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
//! 3. array identities — every array length and every index-valued field,
//!    plus honest reporting: the accessor indices and JSON pointers the
//!    artifact says it rewrote are exactly the ones this module's own scan
//!    derives;
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
use super::{
    GltfRawJsonDifference, GltfRawJsonDifferenceKind, GltfRawJsonDifferenceSummary,
    GltfScaleArtifact, GltfScaleRewriteError,
};
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
const MAX_RAW_JSON_DIFFERENCES: usize = 16;

/// Observed artifact-level evidence from [`prove_rewritten_artifact`] or
/// [`super::prove_rewritten_rest_bind`].
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct GltfScaleArtifactProof {
    /// The in-memory candidate proof, run on the reloaded artifact.
    pub core: ScaleProof,
    /// Maximum `abs(after - before * m)` across every rewritten raw element,
    /// in JSON and in buffer payloads, where `m` is the multiplier that
    /// element's domain analytically requires: the declared `q` for a
    /// whole-document conversion, and `s_parent`, `s_parent / s_i` or `s_i`
    /// per DESIGN.md Appendix D §D.2 for a rest/bind reparameterization.
    pub length_factor_residual: f64,
    /// Maximum `abs(after - before)` across every element that lives *inside*
    /// a rewritten range and must nevertheless come through unchanged — a
    /// whole-document `MAT4` linear part, a `matrix` node's homogeneous row,
    /// an unaffected joint's inverse-bind slot, and the untouched entries of
    /// a rewritten accessor's `min`/`max`. Everything outside a rewritten
    /// range is proved byte-identical instead.
    pub dimensionless_residual: f64,
    /// Number of maximal buffer byte ranges outside the rewritten accessor
    /// ranges that were verified byte-identical to the source.
    pub preserved_byte_ranges: usize,
    /// Number of unique accessors rewritten.
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

    // The artifact's own report of what it changed is evidence, so it is
    // checked rather than trusted: `converted_pointers` was accumulated by
    // this module's independent scan, and it is exactly the set the rewriter
    // is allowed to have touched.
    if artifact.rewritten_json_pointers() != converted_pointers.iter().cloned().collect::<Vec<_>>()
    {
        return Err(failed(
            "artifact reports exactly the JSON pointers this proof independently derives",
            artifact.rewritten_json_pointers().len() as f64,
            converted_pointers.len() as f64,
        ));
    }

    let mut allowed = converted_pointers;
    for &buffer_index in artifact.reencoded_buffers() {
        allowed.insert(format!("/buffers/{buffer_index}/uri"));
    }
    check_preserved_json(
        source.raw_json(),
        &artifact_json,
        &allowed,
        "every raw JSON location outside the converted set is preserved exactly",
    )?;

    let converted_spans: Vec<AccessorSpan> = spans.iter().map(|(span, _)| *span).collect();
    proof.preserved_byte_ranges = check_preserved_bytes(
        source.resolved_buffers(),
        &artifact_buffers,
        &converted_spans,
    )?;

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
pub(super) fn check_container_integrity(
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
pub(super) fn check_array_identities(
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
pub(super) fn check_preserved_bytes(
    source_buffers: &[Vec<u8>],
    artifact_buffers: &[Vec<u8>],
    spans: &[AccessorSpan],
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
            .filter(|span| span.buffer == buffer_index)
            .map(|span| (span.start, span.end))
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
            // `max` rather than a plain assignment. Unreachable through
            // `prove_rewritten_artifact` — `converted` is sorted and #280
            // rejects a source whose scale-bearing accessor ranges overlap,
            // so no later span can end before the cursor — but a nested span
            // must never rewind the cursor and re-compare bytes that were
            // already accounted for. Pinned by
            // `a_nested_converted_range_does_not_rewind_the_preserved_byte_cursor`.
            cursor = cursor.max(end);
        }
    }
    Ok(preserved)
}

#[derive(Debug, Default)]
struct RawJsonDifferenceCollector {
    differences: Vec<GltfRawJsonDifference>,
    total: usize,
}

impl RawJsonDifferenceCollector {
    fn record(&mut self, pointer: String, kind: GltfRawJsonDifferenceKind) {
        self.total += 1;
        if self.differences.len() < MAX_RAW_JSON_DIFFERENCES {
            self.differences
                .push(GltfRawJsonDifference { pointer, kind });
        }
    }

    fn finish(self) -> GltfRawJsonDifferenceSummary {
        let omitted = self.total - self.differences.len();
        GltfRawJsonDifferenceSummary {
            differences: self.differences,
            omitted,
        }
    }
}

/// Check that every raw JSON location outside `allowed` is preserved.
pub(super) fn check_preserved_json(
    before: &Value,
    after: &Value,
    allowed: &BTreeSet<String>,
    claim: &'static str,
) -> Result<(), GltfScaleRewriteError> {
    let mut collector = RawJsonDifferenceCollector::default();
    collect_json_differences(before, after, "", allowed, &mut collector);
    if collector.total == 0 {
        return Ok(());
    }
    let total = collector.total;
    let summary = collector.finish();
    Err(GltfScaleRewriteError::ArtifactProofFailed {
        claim,
        observed: total as f64,
        tolerance: 0.0,
        raw_json_differences: Some(summary),
    })
}

/// Collect every JSON location where the artifact differs from the source,
/// skipping the pointers the conversion is allowed to change.
fn collect_json_differences(
    before: &Value,
    after: &Value,
    pointer: &str,
    allowed: &BTreeSet<String>,
    out: &mut RawJsonDifferenceCollector,
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
                    // A member only one side declares is a difference unless
                    // the caller's own scan already accounted for it. The
                    // rest/bind reparameterization materializes exactly one
                    // such member — the closure root's `scale`, whose glTF
                    // default `[1, 1, 1]` is not fixed under `* 1/s` — and its
                    // caller records that pointer only after checking the
                    // materialized value against that default. Skipping the
                    // `allowed` test here would make an added member
                    // unreportable *and* unchecked; the whole-document
                    // conversion adds no member at all, so nothing there
                    // changes either way.
                    _ if allowed.contains(&child) => {}
                    (None, Some(_)) => {
                        out.record(child, GltfRawJsonDifferenceKind::ArtifactAdded);
                    }
                    (Some(_), None) => {
                        out.record(child, GltfRawJsonDifferenceKind::ArtifactRemoved);
                    }
                    (None, None) => unreachable!("a key came from at least one object"),
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
        _ => out.record(pointer.to_owned(), GltfRawJsonDifferenceKind::ValueChanged),
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

pub(super) fn track_length(
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
        return Err(failed(
            "every converted length differs from the source by exactly the declared factor",
            residual,
            bound,
        ));
    }
    Ok(())
}

pub(super) fn track_dimensionless(
    before: f64,
    after: f64,
    tolerance: &ScaleTolerancePolicy,
    proof: &mut GltfScaleArtifactProof,
) -> Result<(), GltfScaleRewriteError> {
    let residual = (after - before).abs();
    proof.dimensionless_residual = proof.dimensionless_residual.max(residual);
    let bound = tolerance.scalar_tolerance(before, after);
    if residual > bound {
        return Err(failed(
            "every dimensionless value inside a converted range is invariant",
            residual,
            bound,
        ));
    }
    Ok(())
}

pub(super) fn failed(claim: &'static str, observed: f64, tolerance: f64) -> GltfScaleRewriteError {
    GltfScaleRewriteError::ArtifactProofFailed {
        claim,
        observed,
        tolerance,
        raw_json_differences: None,
    }
}

pub(super) fn object(value: &Value) -> Result<&Map<String, Value>, GltfScaleRewriteError> {
    value
        .as_object()
        .ok_or_else(|| LoadError::Malformed("top-level glTF JSON is not an object".into()).into())
}

pub(super) fn numeric(value: &Value, location: &str) -> Result<f64, GltfScaleRewriteError> {
    value
        .as_f64()
        .ok_or_else(|| LoadError::Malformed(format!("{location} is not a number")).into())
}

#[cfg(test)]
mod tests {
    //! Per-claim negative tests.
    //!
    //! Each test corrupts exactly one thing about an otherwise valid artifact
    //! and asserts the exact claim string that must catch it. These live here
    //! rather than in `tests/scale_rewrite.rs` because
    //! [`super::super::GltfScaleArtifact`]'s fields are private, and two of
    //! the claims — the accessor and JSON-pointer cross-checks — can only be
    //! falsified by making the artifact's own report disagree with its bytes.
    //!
    //! The fixture is a `.gltf`, not a GLB, so an artifact is pure JSON and a
    //! corruption is a `serde_json` edit plus a base64 round trip. No test
    //! below asserts a value this crate produced: every expectation is a
    //! claim string, and every payload literal is exact under a factor of
    //! four.

    use super::*;
    use crate::preflight_scale_source_bytes;
    use animsmith_core::scale::{ScaleOperation, ScaleRequest, plan_scale};
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use serde_json::json;

    /// Byte offsets inside the fixture's single buffer.
    mod offsets {
        pub const POSITION: usize = 0; // 36 bytes, converted
        pub const INVERSE_BIND: usize = 36; // 64 bytes, converted
        pub const JOINTS: usize = 100; // 24 bytes, preserved
        pub const WEIGHTS: usize = 124; // 48 bytes, preserved
        pub const SPARE: usize = 172; // 4 bytes, reached by no bufferView
        pub const LENGTH: usize = 176;
    }

    const FACTOR: f64 = 4.0;

    const POSITIONS: [f32; 9] = [1.0, 2.0, -3.0, 0.5, -0.25, 4.0, 2.0, 0.0, 1.5];
    const INVERSE_BIND: [f32; 16] = [
        1.0, 0.0, 0.0, 0.0, //
        0.0, 1.0, 0.0, 0.0, //
        0.0, 0.0, 1.0, 0.0, //
        -1.0, 2.0, -0.5, 1.0,
    ];

    fn data_uri(bytes: &[u8]) -> String {
        format!(
            "data:application/octet-stream;base64,{}",
            STANDARD.encode(bytes)
        )
    }

    fn fixture_buffer() -> Vec<u8> {
        let mut buffer = vec![0u8; offsets::LENGTH];
        for (index, value) in POSITIONS.iter().enumerate() {
            let at = offsets::POSITION + index * 4;
            buffer[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
        for (index, value) in INVERSE_BIND.iter().enumerate() {
            let at = offsets::INVERSE_BIND + index * 4;
            buffer[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
        // One joint, full weight on it, for each of the three vertices.
        for vertex in 0..3 {
            let at = offsets::WEIGHTS + vertex * 16;
            buffer[at..at + 4].copy_from_slice(&1.0f32.to_le_bytes());
        }
        buffer
    }

    /// A skinned source whose buffer carries four bytes no `bufferView`
    /// reaches, so a byte can be flipped outside every converted range
    /// without disturbing anything the normalized `Document` models.
    fn fixture_json(buffer: &[u8]) -> Value {
        json!({
            "asset": { "version": "2.0" },
            "buffers": [{ "uri": data_uri(buffer), "byteLength": offsets::LENGTH }],
            "bufferViews": [
                { "buffer": 0, "byteOffset": offsets::POSITION, "byteLength": 36 },
                { "buffer": 0, "byteOffset": offsets::INVERSE_BIND, "byteLength": 64 },
                { "buffer": 0, "byteOffset": offsets::JOINTS, "byteLength": 24 },
                { "buffer": 0, "byteOffset": offsets::WEIGHTS, "byteLength": 48 }
            ],
            "accessors": [
                { "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3",
                  "min": [0.5, -0.25, -3.0], "max": [2.0, 2.0, 4.0] },
                { "bufferView": 1, "componentType": 5126, "count": 1, "type": "MAT4" },
                { "bufferView": 2, "componentType": 5123, "count": 3, "type": "VEC4" },
                { "bufferView": 3, "componentType": 5126, "count": 3, "type": "VEC4" }
            ],
            "materials": [{ "name": "surface" }],
            "meshes": [{ "primitives": [{
                "attributes": { "POSITION": 0, "JOINTS_0": 2, "WEIGHTS_0": 3 },
                "material": 0
            }] }],
            "nodes": [{ "name": "joint" }, { "name": "holder", "mesh": 0, "skin": 0 }],
            "scenes": [{ "nodes": [0, 1] }],
            "scene": 0,
            "skins": [{ "joints": [0], "skeleton": 0, "inverseBindMatrices": 1 }]
        })
    }

    fn fixture() -> (GltfScaleSource, GltfScaleArtifact, ScalePlan) {
        let value = fixture_json(&fixture_buffer());
        let bytes = serde_json::to_vec(&value).expect("fixture serializes");
        let source = preflight_scale_source_bytes(Path::new("proof-fixture.gltf"), &bytes)
            .expect("the fixture preflights cleanly");
        let plan = plan_scale(&ScaleRequest {
            operation: ScaleOperation::WholeDocumentLinearUnits { factor: FACTOR },
            document: source.document(),
            capability: &super::super::capability_facts(source.manifest()),
        })
        .expect("plan");
        let artifact = super::super::rewrite_linear_units(&source, FACTOR).expect("rewrite");
        (source, artifact, plan)
    }

    /// The artifact's JSON, as a mutable tree.
    fn artifact_value(artifact: &GltfScaleArtifact) -> Value {
        serde_json::from_slice(artifact.bytes()).expect("a .gltf artifact is JSON")
    }

    fn put_artifact_value(artifact: &mut GltfScaleArtifact, value: &Value) {
        artifact.bytes = serde_json::to_vec(value).expect("corrupted fixture serializes");
    }

    fn artifact_buffer(value: &Value) -> Vec<u8> {
        let uri = value["buffers"][0]["uri"].as_str().expect("data URI");
        STANDARD
            .decode(uri.split_once("base64,").expect("base64 data URI").1)
            .expect("valid base64")
    }

    fn put_artifact_buffer(value: &mut Value, bytes: &[u8]) {
        value["buffers"][0]["uri"] = json!(data_uri(bytes));
    }

    /// Assert that proving `artifact` fails with exactly `expected`.
    fn expect_claim(
        source: &GltfScaleSource,
        artifact: &GltfScaleArtifact,
        plan: &ScalePlan,
        expected: &str,
    ) {
        match prove_rewritten_artifact(source, artifact, plan) {
            Err(GltfScaleRewriteError::ArtifactProofFailed {
                claim,
                raw_json_differences,
                ..
            }) => {
                assert_eq!(claim, expected);
                assert_eq!(
                    raw_json_differences, None,
                    "ordinary proof claims do not carry raw JSON diagnostics"
                );
            }
            other => panic!("expected the claim {expected:?} to fail, got {other:?}"),
        }
    }

    #[test]
    fn the_uncorrupted_fixture_proves_and_reports_its_evidence() {
        // Without this, every negative below could be passing for the wrong
        // reason: a fixture that never proves cannot show which claim caught
        // which corruption.
        let (source, artifact, plan) = fixture();
        let proof = prove_rewritten_artifact(&source, &artifact, &plan).expect("artifact proof");
        assert_eq!(proof.rewritten_accessor_count, 2, "POSITION and the IBM");
        assert_eq!(proof.length_factor_residual, 0.0);
        assert_eq!(proof.dimensionless_residual, 0.0);
        // 0..36 and 36..100 are converted, so 100..104 is the only preserved
        // complement range.
        assert_eq!(proof.preserved_byte_ranges, 1);
        assert_eq!(artifact.rewritten_accessors(), [0, 1]);
        assert_eq!(
            artifact.rewritten_json_pointers(),
            ["/accessors/0/max", "/accessors/0/min"]
        );
    }

    #[test]
    fn a_flipped_byte_outside_every_converted_range_fails_byte_preservation() {
        let (source, mut artifact, plan) = fixture();
        let mut value = artifact_value(&artifact);
        let mut buffer = artifact_buffer(&value);
        buffer[offsets::SPARE] ^= 0xff;
        put_artifact_buffer(&mut value, &buffer);
        put_artifact_value(&mut artifact, &value);
        expect_claim(
            &source,
            &artifact,
            &plan,
            "buffer bytes outside the converted ranges are preserved",
        );
    }

    #[test]
    fn a_dimensionless_component_inside_a_converted_range_must_be_bit_identical() {
        // The inverse bind's 3x3 entry 1 is `+0.0`; setting its sign bit
        // makes it `-0.0`, which is numerically identical and so invisible to
        // every residual — only the bit comparison catches it.
        let (source, mut artifact, plan) = fixture();
        let mut value = artifact_value(&artifact);
        let mut buffer = artifact_buffer(&value);
        const SIGN_BYTE: usize = offsets::INVERSE_BIND + 4 + 3;
        buffer[SIGN_BYTE] |= 0x80;
        put_artifact_buffer(&mut value, &buffer);
        put_artifact_value(&mut artifact, &value);
        expect_claim(
            &source,
            &artifact,
            &plan,
            "a converted accessor's dimensionless components are bit-identical",
        );
    }

    #[test]
    fn an_under_reported_converted_accessor_fails_the_accessor_cross_check() {
        let (source, mut artifact, plan) = fixture();
        artifact.rewritten_accessors.pop();
        expect_claim(
            &source,
            &artifact,
            &plan,
            "artifact reports exactly the accessors this proof independently derives",
        );
    }

    #[test]
    fn an_under_reported_rewritten_json_pointer_fails_the_pointer_cross_check() {
        let (source, mut artifact, plan) = fixture();
        artifact.rewritten_json_pointers.pop();
        expect_claim(
            &source,
            &artifact,
            &plan,
            "artifact reports exactly the JSON pointers this proof independently derives",
        );
    }

    #[test]
    fn added_and_removed_preserved_json_locations_keep_whole_document_direction() {
        let (source, mut artifact, plan) = fixture();
        let mut value = artifact_value(&artifact);
        let material = value["materials"][0]
            .as_object_mut()
            .expect("fixture material is an object");
        material.remove("name");
        material.insert("replacement".into(), json!(true));
        put_artifact_value(&mut artifact, &value);
        let error = prove_rewritten_artifact(&source, &artifact, &plan)
            .expect_err("changed preserved JSON must fail");
        assert_eq!(
            error.to_string(),
            "artifact proof claim \"every raw JSON location outside the converted set is preserved exactly\" observed 2, tolerance 0; raw JSON differences: /materials/0/name (artifact-removed), /materials/0/replacement (artifact-added)"
        );
        match error {
            GltfScaleRewriteError::ArtifactProofFailed {
                claim,
                observed,
                tolerance,
                raw_json_differences: Some(summary),
            } => {
                assert_eq!(
                    claim,
                    "every raw JSON location outside the converted set is preserved exactly"
                );
                assert_eq!(observed, 2.0);
                assert_eq!(tolerance, 0.0);
                assert_eq!(
                    summary,
                    GltfRawJsonDifferenceSummary {
                        differences: vec![
                            GltfRawJsonDifference {
                                pointer: "/materials/0/name".into(),
                                kind: GltfRawJsonDifferenceKind::ArtifactRemoved,
                            },
                            GltfRawJsonDifference {
                                pointer: "/materials/0/replacement".into(),
                                kind: GltfRawJsonDifferenceKind::ArtifactAdded,
                            },
                        ],
                        omitted: 0,
                    }
                );
            }
            other => panic!("expected located JSON diagnostics, got {other:?}"),
        }
    }

    #[test]
    fn json_difference_collection_is_typed_escaped_ordered_and_allowlisted() {
        let before = json!({
            "a~/b~/c": 1,
            "allowed": "source secret",
            "allowedly": 1,
            "removed": true,
            "value": 1
        });
        let after = json!({
            "a~/b~/c": 2,
            "added": true,
            "allowed": "artifact secret",
            "allowedly": 2,
            "value": 2
        });
        let allowed = BTreeSet::from(["/allowed".to_owned()]);
        let mut collector = RawJsonDifferenceCollector::default();
        collect_json_differences(&before, &after, "", &allowed, &mut collector);
        assert_eq!(
            collector.finish(),
            GltfRawJsonDifferenceSummary {
                differences: vec![
                    GltfRawJsonDifference {
                        pointer: "/added".into(),
                        kind: GltfRawJsonDifferenceKind::ArtifactAdded,
                    },
                    GltfRawJsonDifference {
                        pointer: "/allowedly".into(),
                        kind: GltfRawJsonDifferenceKind::ValueChanged,
                    },
                    GltfRawJsonDifference {
                        pointer: "/a~0~1b~0~1c".into(),
                        kind: GltfRawJsonDifferenceKind::ValueChanged,
                    },
                    GltfRawJsonDifference {
                        pointer: "/removed".into(),
                        kind: GltfRawJsonDifferenceKind::ArtifactRemoved,
                    },
                    GltfRawJsonDifference {
                        pointer: "/value".into(),
                        kind: GltfRawJsonDifferenceKind::ValueChanged,
                    },
                ],
                omitted: 0,
            }
        );
    }

    #[test]
    fn json_difference_collection_caps_storage_but_counts_every_difference() {
        let mut before = Map::new();
        let mut after = Map::new();
        for index in 0..20 {
            let key = format!("key-{index:02}");
            match index % 3 {
                0 => {
                    before.insert(key.clone(), json!(0));
                    after.insert(key, json!(1));
                }
                1 => {
                    after.insert(key, json!(1));
                }
                _ => {
                    before.insert(key, json!(0));
                }
            }
        }
        let mut collector = RawJsonDifferenceCollector::default();
        collect_json_differences(
            &Value::Object(before),
            &Value::Object(after),
            "",
            &BTreeSet::new(),
            &mut collector,
        );
        let summary = collector.finish();
        assert_eq!(summary.differences.len() + summary.omitted, 20);
        assert_eq!(summary.omitted, 4);
        assert_eq!(
            summary.differences,
            (0..MAX_RAW_JSON_DIFFERENCES)
                .map(|index| GltfRawJsonDifference {
                    pointer: format!("/key-{index:02}"),
                    kind: match index % 3 {
                        0 => GltfRawJsonDifferenceKind::ValueChanged,
                        1 => GltfRawJsonDifferenceKind::ArtifactAdded,
                        _ => GltfRawJsonDifferenceKind::ArtifactRemoved,
                    },
                })
                .collect::<Vec<_>>()
        );
        let display = super::super::RawJsonDifferenceSuffix(Some(&summary)).to_string();
        assert!(display.ends_with("; 4 omitted"));
        assert!(display.contains("/key-15 (value-changed)"));
        assert!(
            !display.contains("/key-16"),
            "omitted pointers stay omitted"
        );
    }

    #[test]
    fn unequal_arrays_report_one_value_change_at_the_array_root() {
        for (before, after) in [
            (json!({ "nodes": [1] }), json!({ "nodes": [1, 2] })),
            (json!({ "nodes": [1, 2] }), json!({ "nodes": [1] })),
        ] {
            let mut collector = RawJsonDifferenceCollector::default();
            collect_json_differences(&before, &after, "", &BTreeSet::new(), &mut collector);
            assert_eq!(
                collector.finish(),
                GltfRawJsonDifferenceSummary {
                    differences: vec![GltfRawJsonDifference {
                        pointer: "/nodes".into(),
                        kind: GltfRawJsonDifferenceKind::ValueChanged,
                    }],
                    omitted: 0,
                }
            );
        }
    }

    #[test]
    fn a_changed_top_level_array_length_fails_array_identity() {
        let (source, mut artifact, plan) = fixture();
        let mut value = artifact_value(&artifact);
        value["materials"]
            .as_array_mut()
            .expect("materials array")
            .push(json!({ "name": "smuggled" }));
        put_artifact_value(&mut artifact, &value);
        expect_claim(
            &source,
            &artifact,
            &plan,
            "every top-level array keeps its source length",
        );
    }

    #[test]
    fn a_resolved_buffer_that_grew_fails_the_buffer_length_claim() {
        // `byteLength` is left alone, so the growth is invisible to the JSON
        // comparison and only the resolved-bytes claim can see it.
        let (source, mut artifact, plan) = fixture();
        let mut value = artifact_value(&artifact);
        let mut buffer = artifact_buffer(&value);
        buffer.extend_from_slice(&[0u8; 4]);
        put_artifact_buffer(&mut value, &buffer);
        put_artifact_value(&mut artifact, &value);
        expect_claim(
            &source,
            &artifact,
            &plan,
            "every resolved buffer keeps its source byte length",
        );
    }

    #[test]
    fn a_declared_buffer_length_without_backing_bytes_fails_container_integrity() {
        let (source, mut artifact, plan) = fixture();
        let mut value = artifact_value(&artifact);
        value["buffers"][0]["byteLength"] = json!(offsets::LENGTH + 4);
        put_artifact_value(&mut artifact, &value);
        expect_claim(
            &source,
            &artifact,
            &plan,
            "every declared buffer byteLength is backed by resolved bytes",
        );
    }

    #[test]
    fn a_flipped_container_kind_fails_the_container_claim() {
        let (source, mut artifact, plan) = fixture();
        artifact.container = GltfContainerKind::Glb;
        expect_claim(
            &source,
            &artifact,
            &plan,
            "artifact container kind is unchanged",
        );
    }

    #[test]
    fn a_bound_that_no_longer_bounds_the_converted_payload_fails() {
        // One ULP above the converted minimum: far inside the shared scalar
        // tolerance, so every residual still passes and only the bounding
        // obligation itself can reject it.
        let (source, mut artifact, plan) = fixture();
        let mut value = artifact_value(&artifact);
        value["accessors"][0]["min"][0] = json!(2.0000002f32 as f64);
        put_artifact_value(&mut artifact, &value);
        expect_claim(
            &source,
            &artifact,
            &plan,
            "a converted bound still bounds the converted payload",
        );
    }

    #[test]
    fn bytes_that_are_not_the_rewriters_own_output_fail_the_determinism_claim() {
        // Same JSON value, different bytes. Every claim that reads the
        // artifact through `serde_json` still passes, so nothing but the
        // byte-for-byte repeat comparison can notice.
        let (source, mut artifact, plan) = fixture();
        let value = artifact_value(&artifact);
        artifact.bytes = serde_json::to_vec_pretty(&value).expect("pretty serializes");
        expect_claim(
            &source,
            &artifact,
            &plan,
            "rewriting the same source twice yields identical bytes",
        );
    }

    #[test]
    fn a_nested_converted_range_does_not_rewind_the_preserved_byte_cursor() {
        // `check_preserved_bytes` is exercised directly here because #280
        // rejects a source whose scale-bearing accessor ranges overlap, so
        // `prove_rewritten_artifact` can never hand it a nested pair. The
        // guard that makes the walk safe anyway is `cursor.max(end)`: without
        // it the inner span rewinds the cursor to 8 and bytes 8..16 — which
        // lie *inside* the outer converted range and are legitimately allowed
        // to differ — get compared as if they were preserved.
        let mut before = vec![0u8; 20];
        let mut after = vec![0u8; 20];
        before[10] = 1;
        after[10] = 2;
        let span = |start: usize, end: usize| AccessorSpan {
            accessor_index: 0,
            buffer: 0,
            start,
            end,
            components: 1,
        };
        let spans = [span(0, 16), span(4, 8)];
        let preserved = check_preserved_bytes(&[before], &[after], &spans)
            .expect("only 16..20 lies outside the converted ranges");
        assert_eq!(preserved, 1);
    }
}
