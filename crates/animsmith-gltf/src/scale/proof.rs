//! Artifact-level proof: what the in-memory candidate proof cannot see.
//!
//! [`animsmith_core::scale::prove_scale`] runs on a normalized
//! [`animsmith_core::Document`], so it can only prove claims about domains
//! that model represents. This layer proves the rest, directly against the
//! emitted container bytes:
//!
//! 1. every raw source payload the normalized document does not model
//!    (materials, images, textures, samplers, `TANGENT`/`COLOR_n`, secondary
//!    influences, extension payloads, names, `asset`);
//! 2. byte preservation of every buffer byte outside the converted accessor
//!    ranges;
//! 3. array identities — every array length and every index-valued field;
//! 4. determinism — rewriting the same source twice yields identical bytes;
//! 5. container integrity — GLB header and chunk framing, 4-byte padding, and
//!    declared buffer lengths;
//! 6. `min`/`max` consistency — the rewritten bounds still bound the
//!    rewritten data;
//! 7. single-narrowing agreement — every converted `f32` is bit-identical to
//!    the one-step narrowing of `before * q`.
//!
//! The in-memory layer is kept, not replaced: `SkinMatrix` (`W * B`),
//! `Trajectory`, `CubicInterior` and `Bounds` residuals live there, and
//! re-deriving them from raw bytes would be duplicate math with a second
//! chance to be wrong.
//!
//! Every expected location is derived here by this module's own scan of the
//! raw JSON, not by calling the rewriter's domain table. A proof that asks
//! the implementation which locations it decided to change cannot fail when
//! the implementation decides wrongly.

use super::bytes::{self, AccessorSpan};
use super::rules::AccessorRule;
use super::{GltfScaleArtifact, GltfScaleRewriteError};
use crate::capability::{GltfContainerKind, GltfScaleSource, raw_json_bytes};
use crate::{LoadError, load_bytes, resolve_buffers};
use animsmith_core::scale::{
    ScaleCandidate, ScaleOperation, ScalePlan, ScaleProof, ScaleTolerancePolicy, prove_scale,
};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

const GLB_JSON_CHUNK: u32 = 0x4e4f_534a;
const GLB_BIN_CHUNK: u32 = 0x004e_4942;

/// Observed artifact-level evidence from [`prove_rewritten_artifact`].
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct GltfScaleArtifactProof {
    /// The in-memory candidate proof, run on the reloaded artifact.
    pub core: ScaleProof,
    /// Maximum `abs(after - before * q)` across every converted raw element,
    /// in JSON and in buffer payloads.
    pub length_factor_residual: f64,
    /// Maximum `abs(after - before)` across every dimensionless element that
    /// lives *inside* a converted range — a `MAT4` linear part, a `matrix`
    /// node's 3x3 and homogeneous component, and the unconverted entries of a
    /// converted accessor's `min`/`max`. Everything outside a converted range
    /// is proved byte-identical instead.
    pub dimensionless_residual: f64,
    /// Number of maximal buffer byte ranges outside the converted accessor
    /// ranges that were verified byte-identical to the source.
    pub preserved_byte_ranges: usize,
    /// Number of unique accessors converted.
    pub rewritten_accessor_count: usize,
}

/// Independently re-derive and check every artifact-level claim.
///
/// # Errors
///
/// Returns [`GltfScaleRewriteError::ArtifactProofFailed`] for the first
/// failed claim, [`GltfScaleRewriteError::Plan`] when the reloaded candidate
/// fails the shared core proof, and [`GltfScaleRewriteError::Load`] when the
/// artifact cannot be re-read.
pub fn prove_rewritten_artifact(
    source: &GltfScaleSource,
    artifact: &GltfScaleArtifact,
    plan: &ScalePlan,
) -> Result<GltfScaleArtifactProof, GltfScaleRewriteError> {
    let tolerance = plan.tolerance_policy();
    let ScaleOperation::WholeDocumentLinearUnits { factor } = plan.operation() else {
        return Err(failed(
            "plan declares a whole-document unit conversion",
            1.0,
            0.0,
        ));
    };
    if factor != artifact.declared_factor() {
        return Err(failed(
            "plan factor equals the artifact's declared factor",
            (factor - artifact.declared_factor()).abs(),
            0.0,
        ));
    }

    let reloaded = load_bytes(Path::new("scale-artifact"), artifact.bytes())?;
    let core = prove_scale(
        source.document(),
        &ScaleCandidate::from_document(reloaded),
        plan,
    )
    .map_err(GltfScaleRewriteError::Plan)?;

    let (container, json_bytes) = raw_json_bytes(artifact.bytes())?;
    if container != artifact.container() {
        return Err(failed("artifact container kind is unchanged", 1.0, 0.0));
    }
    let artifact_json: Value = serde_json::from_slice(json_bytes)
        .map_err(|error| LoadError::Malformed(format!("artifact JSON is invalid: {error}")))?;
    let source_root = object(source.raw_json())?;
    let artifact_root = object(&artifact_json)?;
    let artifact_gltf = gltf::Gltf::from_slice(artifact.bytes()).map_err(LoadError::Gltf)?;
    let artifact_buffers = resolve_buffers(&artifact_gltf, None)?;

    check_container_integrity(artifact, artifact_root, &artifact_buffers)?;
    check_array_identities(source_root, artifact_root)?;

    let mut proof = GltfScaleArtifactProof {
        core,
        length_factor_residual: 0.0,
        dimensionless_residual: 0.0,
        preserved_byte_ranges: 0,
        rewritten_accessor_count: 0,
    };

    let mut converted_pointers = BTreeSet::new();
    check_node_transforms(
        source_root,
        artifact_root,
        factor,
        &tolerance,
        &mut converted_pointers,
        &mut proof,
    )?;

    let expected = scale_bearing_accessors(source_root);
    if artifact.rewritten_accessors() != expected.keys().copied().collect::<Vec<_>>() {
        return Err(failed(
            "artifact reports exactly the accessors this proof independently derives",
            artifact.rewritten_accessors().len() as f64,
            expected.len() as f64,
        ));
    }
    proof.rewritten_accessor_count = expected.len();

    let mut spans = Vec::with_capacity(expected.len());
    for (&accessor_index, &rule) in &expected {
        let span =
            bytes::accessor_span(source_root, source.resolved_buffers(), accessor_index, rule)?;
        let artifact_span =
            bytes::accessor_span(artifact_root, &artifact_buffers, accessor_index, rule)?;
        if span != artifact_span {
            return Err(failed(
                "converted accessors keep their source byte layout",
                artifact_span.start as f64,
                span.start as f64,
            ));
        }
        spans.push((span, rule));
    }

    check_converted_payloads(
        source.resolved_buffers(),
        &artifact_buffers,
        &spans,
        factor,
        &tolerance,
        &mut proof,
    )?;
    check_accessor_bounds(
        source_root,
        artifact_root,
        &artifact_buffers,
        &spans,
        factor,
        &tolerance,
        &mut converted_pointers,
        &mut proof,
    )?;

    let mut allowed = converted_pointers;
    for &buffer_index in artifact.reencoded_buffers() {
        allowed.insert(format!("/buffers/{buffer_index}/uri"));
    }
    let mut differences = Vec::new();
    collect_json_differences(
        source.raw_json(),
        &artifact_json,
        "",
        &allowed,
        &mut differences,
    );
    if !differences.is_empty() {
        return Err(failed(
            "every raw JSON location outside the converted set is preserved exactly",
            differences.len() as f64,
            0.0,
        ));
    }

    proof.preserved_byte_ranges =
        check_preserved_bytes(source.resolved_buffers(), &artifact_buffers, &spans)?;

    let repeat = super::rewrite_linear_units(source, factor)?;
    if repeat.bytes() != artifact.bytes() {
        return Err(failed(
            "rewriting the same source twice yields identical bytes",
            repeat.bytes().len() as f64,
            artifact.bytes().len() as f64,
        ));
    }
    Ok(proof)
}

// --- Claims ---------------------------------------------------------------

/// GLB header/chunk framing and declared buffer lengths.
fn check_container_integrity(
    artifact: &GltfScaleArtifact,
    artifact_root: &Map<String, Value>,
    artifact_buffers: &[Vec<u8>],
) -> Result<(), GltfScaleRewriteError> {
    if artifact.container() == GltfContainerKind::Glb {
        let bytes = artifact.bytes();
        let word = |offset: usize| -> Result<u32, GltfScaleRewriteError> {
            bytes
                .get(offset..offset + 4)
                .and_then(|slice| slice.try_into().ok())
                .map(u32::from_le_bytes)
                .ok_or_else(|| failed("GLB container is long enough for its own framing", 0.0, 1.0))
        };
        if &bytes[0..4.min(bytes.len())] != b"glTF" || word(4)? != 2 {
            return Err(failed("GLB declares magic 'glTF' and version 2", 0.0, 1.0));
        }
        if word(8)? as usize != bytes.len() {
            return Err(failed(
                "GLB total length equals the emitted byte count",
                f64::from(word(8)?),
                bytes.len() as f64,
            ));
        }
        let json_len = word(12)? as usize;
        if word(16)? != GLB_JSON_CHUNK || !json_len.is_multiple_of(4) {
            return Err(failed(
                "GLB JSON chunk is typed and 4-byte padded",
                0.0,
                1.0,
            ));
        }
        let mut offset = 20 + json_len;
        let mut framed = offset;
        if offset < bytes.len() {
            let bin_len = word(offset)? as usize;
            if word(offset + 4)? != GLB_BIN_CHUNK || !bin_len.is_multiple_of(4) {
                return Err(failed("GLB BIN chunk is typed and 4-byte padded", 0.0, 1.0));
            }
            offset += 8;
            framed = offset + bin_len;
        }
        if framed != bytes.len() {
            return Err(failed(
                "GLB chunk lengths account for every emitted byte",
                framed as f64,
                bytes.len() as f64,
            ));
        }
    }
    let declared = artifact_root
        .get("buffers")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    for (buffer_index, buffer) in declared.iter().enumerate() {
        let byte_length = buffer
            .get("byteLength")
            .and_then(Value::as_u64)
            .unwrap_or_default() as usize;
        let resolved = artifact_buffers.get(buffer_index).map_or(0, Vec::len);
        if resolved < byte_length {
            return Err(failed(
                "every declared buffer byteLength is backed by resolved bytes",
                resolved as f64,
                byte_length as f64,
            ));
        }
    }
    Ok(())
}

/// Every top-level array keeps its length, so every index-valued field in the
/// document still names the same element.
fn check_array_identities(
    source_root: &Map<String, Value>,
    artifact_root: &Map<String, Value>,
) -> Result<(), GltfScaleRewriteError> {
    const ARRAYS: &[&str] = &[
        "accessors",
        "animations",
        "bufferViews",
        "buffers",
        "cameras",
        "images",
        "materials",
        "meshes",
        "nodes",
        "samplers",
        "scenes",
        "skins",
        "textures",
    ];
    for key in ARRAYS {
        let length =
            |root: &Map<String, Value>| root.get(*key).and_then(Value::as_array).map(Vec::len);
        if length(source_root) != length(artifact_root) {
            return Err(failed(
                "every top-level array keeps its source length",
                length(artifact_root).unwrap_or_default() as f64,
                length(source_root).unwrap_or_default() as f64,
            ));
        }
    }
    Ok(())
}

/// Node `translation` scales; a node `matrix` scales exactly its translation
/// column and preserves its 3x3 and homogeneous component.
fn check_node_transforms(
    source_root: &Map<String, Value>,
    artifact_root: &Map<String, Value>,
    factor: f64,
    tolerance: &ScaleTolerancePolicy,
    converted_pointers: &mut BTreeSet<String>,
    proof: &mut GltfScaleArtifactProof,
) -> Result<(), GltfScaleRewriteError> {
    let source_nodes = source_root
        .get("nodes")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let artifact_nodes = artifact_root
        .get("nodes")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    for (node_index, (before, after)) in source_nodes.iter().zip(artifact_nodes).enumerate() {
        for (member, length, scales) in [
            ("translation", 3usize, &[0usize, 1, 2] as &[usize]),
            ("matrix", 16, &[12, 13, 14]),
        ] {
            let Some(source_values) = before.get(member).and_then(Value::as_array) else {
                continue;
            };
            let pointer = format!("/nodes/{node_index}/{member}");
            let artifact_values = after
                .get(member)
                .and_then(Value::as_array)
                .filter(|values| values.len() == length && source_values.len() == length)
                .ok_or_else(|| {
                    failed(
                        "a converted node transform keeps its authored arity",
                        0.0,
                        length as f64,
                    )
                })?;
            for component in 0..length {
                let before = numeric(&source_values[component], &pointer)?;
                let after = numeric(&artifact_values[component], &pointer)?;
                if scales.contains(&component) {
                    track_length(before, after, factor, tolerance, proof)?;
                } else {
                    track_dimensionless(before, after, tolerance, proof)?;
                }
            }
            converted_pointers.insert(pointer);
        }
    }
    Ok(())
}

/// Every converted `f32` in a buffer payload is the single-step narrowing of
/// `before * q`, and every component the rule leaves alone is bit-identical.
fn check_converted_payloads(
    source_buffers: &[Vec<u8>],
    artifact_buffers: &[Vec<u8>],
    spans: &[(AccessorSpan, AccessorRule)],
    factor: f64,
    tolerance: &ScaleTolerancePolicy,
    proof: &mut GltfScaleArtifactProof,
) -> Result<(), GltfScaleRewriteError> {
    for &(span, rule) in spans {
        let before = bytes::read_span(source_buffers, span);
        let after = bytes::read_span(artifact_buffers, span);
        if before.len() != after.len() {
            return Err(failed(
                "a converted accessor keeps its element count",
                after.len() as f64,
                before.len() as f64,
            ));
        }
        for (index, (&before, &after)) in before.iter().zip(&after).enumerate() {
            if rule.scales_component(index % span.components) {
                let expected = f64::from(before) * factor;
                if after.to_bits() != (expected as f32).to_bits() {
                    return Err(failed(
                        "every converted element is the single narrowing of before * q",
                        f64::from(after),
                        expected,
                    ));
                }
                track_length(
                    f64::from(before),
                    f64::from(after),
                    factor,
                    tolerance,
                    proof,
                )?;
            } else if after.to_bits() != before.to_bits() {
                return Err(failed(
                    "a converted accessor's dimensionless components are bit-identical",
                    f64::from(after),
                    f64::from(before),
                ));
            }
        }
    }
    Ok(())
}

/// A converted accessor's `min`/`max` scale by `q` and still bound the
/// rewritten data.
#[allow(clippy::too_many_arguments)]
fn check_accessor_bounds(
    source_root: &Map<String, Value>,
    artifact_root: &Map<String, Value>,
    artifact_buffers: &[Vec<u8>],
    spans: &[(AccessorSpan, AccessorRule)],
    factor: f64,
    tolerance: &ScaleTolerancePolicy,
    converted_pointers: &mut BTreeSet<String>,
    proof: &mut GltfScaleArtifactProof,
) -> Result<(), GltfScaleRewriteError> {
    for &(span, rule) in spans {
        let payload = bytes::read_span(artifact_buffers, span);
        for (member, is_min) in [("min", true), ("max", false)] {
            let pointer = format!("/accessors/{}/{member}", span.accessor_index);
            let Some(source_bounds) = source_root
                .get("accessors")
                .and_then(Value::as_array)
                .and_then(|accessors| accessors.get(span.accessor_index))
                .and_then(|accessor| accessor.get(member))
                .and_then(Value::as_array)
            else {
                continue;
            };
            let artifact_bounds = artifact_root
                .get("accessors")
                .and_then(Value::as_array)
                .and_then(|accessors| accessors.get(span.accessor_index))
                .and_then(|accessor| accessor.get(member))
                .and_then(Value::as_array)
                .filter(|bounds| bounds.len() == source_bounds.len())
                .ok_or_else(|| {
                    failed(
                        "a converted accessor keeps its authored bound arity",
                        0.0,
                        source_bounds.len() as f64,
                    )
                })?;
            for component in 0..source_bounds.len() {
                let before = numeric(&source_bounds[component], &pointer)?;
                let after = numeric(&artifact_bounds[component], &pointer)?;
                if !rule.scales_component(component) {
                    track_dimensionless(before, after, tolerance, proof)?;
                    continue;
                }
                // A converted bound tracks `before * q` within the shared
                // tolerance. It is deliberately not required to be exactly
                // `before * q` or wider: narrowing `before * q` to `f32`
                // rounds in either direction, and the binding obligation is
                // the next one — that the emitted bound bounds the emitted
                // bytes.
                track_length(before, after, factor, tolerance, proof)?;
                let observed = payload
                    .iter()
                    .skip(component)
                    .step_by(span.components)
                    .copied()
                    .fold(
                        if is_min {
                            f32::INFINITY
                        } else {
                            f32::NEG_INFINITY
                        },
                        |accumulator, value| {
                            if is_min {
                                accumulator.min(value)
                            } else {
                                accumulator.max(value)
                            }
                        },
                    );
                // Compared in `f32`, the model glTF actually declares for
                // `min`/`max`: a JSON number is `f64` in transit but a float
                // in the schema, and `serde_json`'s parser is not correctly
                // rounded, so an `f64` comparison here would fail on a
                // last-place artefact of re-reading a value that is exact in
                // the model that matters.
                let declared = after as f32;
                let bounds_data = if is_min {
                    declared <= observed
                } else {
                    declared >= observed
                };
                if !bounds_data {
                    return Err(failed(
                        "a converted bound still bounds the converted payload",
                        f64::from(declared),
                        f64::from(observed),
                    ));
                }
            }
            converted_pointers.insert(pointer);
        }
    }
    Ok(())
}

/// Buffer bytes outside the converted ranges are identical, counted as
/// maximal preserved ranges.
fn check_preserved_bytes(
    source_buffers: &[Vec<u8>],
    artifact_buffers: &[Vec<u8>],
    spans: &[(AccessorSpan, AccessorRule)],
) -> Result<usize, GltfScaleRewriteError> {
    if source_buffers.len() != artifact_buffers.len() {
        return Err(failed(
            "the artifact declares the same number of resolved buffers",
            artifact_buffers.len() as f64,
            source_buffers.len() as f64,
        ));
    }
    let mut preserved = 0usize;
    for (buffer_index, (before, after)) in source_buffers.iter().zip(artifact_buffers).enumerate() {
        if before.len() != after.len() {
            return Err(failed(
                "every resolved buffer keeps its source byte length",
                after.len() as f64,
                before.len() as f64,
            ));
        }
        let mut converted: Vec<(usize, usize)> = spans
            .iter()
            .filter(|(span, _)| span.buffer == buffer_index)
            .map(|(span, _)| (span.start, span.end))
            .collect();
        converted.sort_unstable();
        let mut cursor = 0usize;
        for (start, end) in converted.into_iter().chain([(before.len(), before.len())]) {
            if cursor < start {
                if before[cursor..start] != after[cursor..start] {
                    return Err(failed(
                        "buffer bytes outside the converted ranges are preserved",
                        cursor as f64,
                        start as f64,
                    ));
                }
                preserved += 1;
            }
            cursor = cursor.max(end);
        }
    }
    Ok(preserved)
}

/// Collect every JSON location where the artifact differs from the source,
/// skipping the pointers the conversion is allowed to change.
fn collect_json_differences(
    before: &Value,
    after: &Value,
    pointer: &str,
    allowed: &BTreeSet<String>,
    out: &mut Vec<String>,
) {
    if allowed.contains(pointer) {
        return;
    }
    match (before, after) {
        (Value::Object(before), Value::Object(after)) => {
            let keys: BTreeSet<&String> = before.keys().chain(after.keys()).collect();
            for key in keys {
                let child = format!("{pointer}/{}", key.replace('~', "~0").replace('/', "~1"));
                match (before.get(key), after.get(key)) {
                    (Some(before), Some(after)) => {
                        collect_json_differences(before, after, &child, allowed, out);
                    }
                    _ => out.push(child),
                }
            }
        }
        (Value::Array(before), Value::Array(after)) if before.len() == after.len() => {
            for (index, (before, after)) in before.iter().zip(after).enumerate() {
                collect_json_differences(
                    before,
                    after,
                    &format!("{pointer}/{index}"),
                    allowed,
                    out,
                );
            }
        }
        (before, after) if before == after => {}
        _ => out.push(pointer.to_owned()),
    }
}

// --- Independent domain scan ------------------------------------------------

/// Every accessor a whole-document conversion must convert, derived here by
/// this module's own scan of the raw JSON.
///
/// Deliberately a second implementation of the [`super::rules`] domain table:
/// asking the rewriter which accessors it decided to convert would make this
/// proof agree with the rewriter by construction. The map is keyed by unique
/// accessor index for the same reason the rewriter's is — a `POSITION` shared
/// by two primitives is one conversion, and a proof that counted it twice
/// would expect `q^2`.
fn scale_bearing_accessors(root: &Map<String, Value>) -> BTreeMap<usize, AccessorRule> {
    let mut out = BTreeMap::new();
    let index = |value: Option<&Value>| -> Option<usize> {
        value?.as_u64().and_then(|index| index.try_into().ok())
    };
    for mesh in root
        .get("meshes")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
    {
        for primitive in mesh
            .get("primitives")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default()
        {
            if let Some(accessor) = index(
                primitive
                    .get("attributes")
                    .and_then(|attributes| attributes.get("POSITION")),
            ) {
                out.insert(accessor, AccessorRule::AllComponents);
            }
        }
    }
    for skin in root
        .get("skins")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
    {
        if let Some(accessor) = index(skin.get("inverseBindMatrices")) {
            out.insert(accessor, AccessorRule::Mat4TranslationColumn);
        }
    }
    for animation in root
        .get("animations")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
    {
        let samplers = animation
            .get("samplers")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default();
        for channel in animation
            .get("channels")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default()
        {
            let is_translation = channel
                .get("target")
                .and_then(|target| target.get("path"))
                .and_then(Value::as_str)
                == Some("translation");
            if !is_translation {
                continue;
            }
            if let Some(accessor) = index(channel.get("sampler"))
                .and_then(|sampler| samplers.get(sampler))
                .and_then(|sampler| index(sampler.get("output")))
            {
                out.insert(accessor, AccessorRule::AllComponents);
            }
        }
    }
    out
}

// --- Residual bookkeeping ---------------------------------------------------

fn track_length(
    before: f64,
    after: f64,
    factor: f64,
    tolerance: &ScaleTolerancePolicy,
    proof: &mut GltfScaleArtifactProof,
) -> Result<(), GltfScaleRewriteError> {
    let expected = before * factor;
    let residual = (after - expected).abs();
    proof.length_factor_residual = proof.length_factor_residual.max(residual);
    let bound = tolerance.scalar_tolerance(expected, after);
    if residual > bound {
        return Err(GltfScaleRewriteError::ArtifactProofFailed {
            claim: "every converted length differs from the source by exactly the declared factor",
            observed: residual,
            tolerance: bound,
        });
    }
    Ok(())
}

fn track_dimensionless(
    before: f64,
    after: f64,
    tolerance: &ScaleTolerancePolicy,
    proof: &mut GltfScaleArtifactProof,
) -> Result<(), GltfScaleRewriteError> {
    let residual = (after - before).abs();
    proof.dimensionless_residual = proof.dimensionless_residual.max(residual);
    let bound = tolerance.scalar_tolerance(before, after);
    if residual > bound {
        return Err(GltfScaleRewriteError::ArtifactProofFailed {
            claim: "every dimensionless value inside a converted range is invariant",
            observed: residual,
            tolerance: bound,
        });
    }
    Ok(())
}

fn failed(claim: &'static str, observed: f64, tolerance: f64) -> GltfScaleRewriteError {
    GltfScaleRewriteError::ArtifactProofFailed {
        claim,
        observed,
        tolerance,
    }
}

fn object(value: &Value) -> Result<&Map<String, Value>, GltfScaleRewriteError> {
    value
        .as_object()
        .ok_or_else(|| LoadError::Malformed("top-level glTF JSON is not an object".into()).into())
}

fn numeric(value: &Value, location: &str) -> Result<f64, GltfScaleRewriteError> {
    value
        .as_f64()
        .ok_or_else(|| LoadError::Malformed(format!("{location} is not a number")).into())
}
