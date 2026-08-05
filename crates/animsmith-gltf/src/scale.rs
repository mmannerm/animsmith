//! Preservation-safe whole-document linear-unit rewriting of raw glTF/GLB
//! bytes (DESIGN.md Appendix D §D.2, implementation slice 3 of §D.8).
//!
//! [`rewrite_linear_units`] converts every length in a captured
//! [`GltfScaleSource`] by a caller-declared finite `factor > 0`. It operates
//! on the source's own JSON tree and its own resolved buffer bytes, and it
//! **never routes through [`crate::write`]**: that path rebuilds a normalized
//! [`animsmith_core::Document`] and would silently drop every source payload
//! the conversion exists to preserve. The factor is never inferred — not from
//! bounds, character height, names, inverse binds, or asset category.
//!
//! # Composition
//!
//! ```no_run
//! # fn convert(path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
//! use animsmith_core::scale::{
//!     ScaleCandidate, ScaleOperation, ScaleRequest, plan_scale, prove_scale,
//! };
//! use animsmith_gltf::{
//!     capability_facts, load_bytes, preflight_scale_source, prove_rewritten_artifact,
//!     rewrite_linear_units,
//! };
//!
//! let factor = 0.01;
//! let source = preflight_scale_source(path)?;
//! let facts = capability_facts(source.manifest());
//! let plan = plan_scale(&ScaleRequest {
//!     operation: ScaleOperation::WholeDocumentLinearUnits { factor },
//!     document: source.document(),
//!     capability: &facts,
//! })?;
//! let artifact = rewrite_linear_units(&source, factor)?;
//! let reloaded = load_bytes(path, artifact.bytes())?;
//! let core = prove_scale(
//!     source.document(),
//!     &ScaleCandidate::from_document(reloaded),
//!     &plan,
//! )?;
//! let artifact_proof = prove_rewritten_artifact(&source, &artifact, &plan)?;
//! # let _ = (core, artifact_proof);
//! # Ok(())
//! # }
//! ```
//!
//! # What is preserved, and what is not
//!
//! Buffer bytes outside the converted accessor ranges are preserved exactly.
//! Every array index, every index-valued field, every material, image,
//! texture, sampler, name, and `asset` member is preserved exactly. No byte
//! length changes, because the conversion is an in-place `f32` mapping:
//! `/buffers/*/byteLength`, every `bufferViews` field, and every accessor
//! `count`/`byteOffset`/`componentType`/`type` are untouched.
//!
//! **JSON key order and float spelling are not preserved.** `serde_json` is
//! built here without `preserve_order`, so its object map is a `BTreeMap` and
//! re-serializing sorts members lexically and re-renders floats through
//! `ryu` (`1.0000000` becomes `1.0`). Numeric *values* survive exactly — an
//! integer `2` stays `2`. This is an accepted tradeoff: the output is
//! deterministic, which is the criterion, though it is not a minimal textual
//! diff. Enabling `preserve_order` is deliberately not done — it would pull
//! `indexmap` into the dependency graph and reorder [`crate::write`]'s output,
//! moving existing golden tests for an unrelated reason.
//!
//! Converted numbers are narrowed to `f32` exactly once and written back as
//! the shortest decimal that round-trips that `f32`, so the JSON and the
//! buffer payloads live in the same numeric regime the glTF schema declares
//! and the artifact reloads without a second rounding.
//!
//! Writing the `f32`'s full `f64` widening instead would be both longer and
//! *less* stable: `serde_json`'s number parser is not correctly rounded, so
//! `-0.029999999329447746` (the exact widening of `-0.03f32`) reads back as
//! `-0.029999999329447743`. Both narrow to the same `f32`, which is why this
//! is a formatting choice rather than a correctness one — but the short form
//! keeps the emitted diff readable and survives its own round trip.

mod bytes;
mod container;
mod proof;
mod rules;

use crate::capability::{
    GltfCapabilityManifest, GltfCapabilityViolation, GltfCapabilityViolationKind,
    GltfContainerKind, GltfScaleSource,
};
use crate::{LoadError, WriteError};
use animsmith_core::scale::{
    ScaleCapabilityCoverage, ScaleCapabilityFacts, ScaleError, ScaleOperation, ScaleRequest,
    plan_scale,
};
use bytes::{AccessorSpan, ComponentExtrema};
use rules::{AccessorRule, JsonArrayRule};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

pub use proof::{GltfScaleArtifactProof, prove_rewritten_artifact};

// --- Capability projection --------------------------------------------------

/// Project a raw glTF capability manifest onto the format-neutral
/// [`ScaleCapabilityFacts`] that [`animsmith_core::scale::plan_scale`]
/// consumes.
///
/// Every flag is derived by re-deriving the #280 violations the manifest
/// itself evidences and then folding them through one exhaustive
/// [`GltfCapabilityViolationKind`] map, so a new violation kind is a
/// compile error here rather than a silently unmapped domain.
///
/// # Coverage
///
/// `coverage` is [`ScaleCapabilityCoverage::Complete`] exactly when no source
/// buffer is external. That is not a proxy: `preflight_scale_source_bytes`
/// inventories the complete raw JSON unconditionally, and the single
/// inspection it skips — accessor layout, which needs resolved buffer bytes —
/// is skipped exactly when a buffer is external. Such a manifest also carries
/// `external_resources_present`, so it fails
/// [`ScaleCapabilityFacts::is_supported`] twice over.
///
/// # Authority
///
/// The manifest is a projection, not the boundary. Two domains it does not
/// record are static morph weights on `/nodes/*/weights` and
/// `/meshes/*/weights`; both are only meaningful alongside morph targets,
/// which the manifest does record, so a schema-valid source carrying them
/// still reports `morphs_present`. The preflight's own violation list remains
/// the authority, and [`rewrite_linear_units`] independently re-resolves every
/// accessor it touches rather than trusting these flags.
pub fn capability_facts(manifest: &GltfCapabilityManifest) -> ScaleCapabilityFacts {
    let mut facts = ScaleCapabilityFacts::default();
    facts.coverage = ScaleCapabilityCoverage::Complete;
    if manifest
        .buffers
        .iter()
        .any(|buffer| buffer.source_kind == crate::capability::GltfBufferSourceKind::External)
    {
        facts.coverage = ScaleCapabilityCoverage::Unavailable;
    }
    for violation in manifest_violations(manifest) {
        record_violation(&mut facts, violation.kind);
    }
    facts
}

/// The one exhaustive violation-kind to capability-flag map.
///
/// Deliberately written without a wildcard arm: [`GltfCapabilityViolationKind`]
/// is `#[non_exhaustive]` only for downstream crates, so a kind added here
/// fails to compile until it is classified.
fn record_violation(facts: &mut ScaleCapabilityFacts, kind: GltfCapabilityViolationKind) {
    use GltfCapabilityViolationKind as Kind;
    match kind {
        Kind::ExternalResource => facts.external_resources_present = true,
        Kind::MorphTarget => facts.morphs_present = true,
        Kind::MorphWeights => facts.morph_weights_present = true,
        Kind::Camera => facts.cameras_present = true,
        Kind::Light => facts.lights_present = true,
        Kind::Instancing => facts.instancing_present = true,
        Kind::ExtensionDeclaration | Kind::ExtensionPayload => {
            facts.unregistered_extensions_present = true;
        }
        Kind::Extras => facts.extras_present = true,
        Kind::UnknownJsonMember => facts.unknown_source_members_present = true,
        Kind::NonTrianglePrimitive => facts.non_triangle_primitives_present = true,
        Kind::UnsupportedVertexAttribute => facts.unsupported_vertex_attributes_present = true,
        Kind::SecondarySkinInfluences => facts.secondary_skin_influences_present = true,
        Kind::MissingInverseBinds
        | Kind::EmptyInverseBindAccessor
        | Kind::InverseBindCountMismatch
        | Kind::UnreadableInverseBinds => facts.inverse_bind_issues_present = true,
        Kind::UnsafeAccessorLayout
        | Kind::ConflictingAccessorUse
        | Kind::OverlappingAccessorRanges => facts.unsafe_accessor_layout_present = true,
    }
}

/// Re-derive, from the manifest alone, the #280 violations it evidences.
fn manifest_violations(manifest: &GltfCapabilityManifest) -> Vec<GltfCapabilityViolation> {
    use GltfCapabilityViolationKind as Kind;
    let mut out = Vec::new();
    let mut add = |kind: Kind, location: String| {
        out.push(GltfCapabilityViolation { kind, location });
    };

    for location in &manifest.external_resource_locations {
        add(Kind::ExternalResource, location.clone());
    }
    for location in &manifest.extras_locations {
        add(Kind::Extras, location.clone());
    }
    for location in &manifest.unknown_member_locations {
        add(Kind::UnknownJsonMember, location.clone());
    }
    for name in &manifest.extensions {
        add(
            match name.as_str() {
                "KHR_lights_punctual" => Kind::Light,
                "EXT_mesh_gpu_instancing" => Kind::Instancing,
                _ => Kind::ExtensionDeclaration,
            },
            format!("/extensionsUsed:{name}"),
        );
    }
    for location in &manifest.extension_locations {
        add(Kind::ExtensionPayload, location.clone());
    }
    if manifest.camera_count > 0 {
        add(Kind::Camera, "/cameras".to_owned());
    }
    for instancing in &manifest.instancing {
        add(
            Kind::Instancing,
            format!(
                "/nodes/{}/extensions/EXT_mesh_gpu_instancing",
                instancing.node_index
            ),
        );
    }
    for channel in &manifest.animation_channels {
        if channel.target_path == "weights" {
            add(
                Kind::MorphWeights,
                format!(
                    "/animations/{}/channels/{}/target/path",
                    channel.animation_index, channel.channel_index
                ),
            );
        }
    }
    for primitive in &manifest.primitives {
        let base = format!(
            "/meshes/{}/primitives/{}",
            primitive.mesh_index, primitive.primitive_index
        );
        if primitive.morph_target_count > 0 {
            add(Kind::MorphTarget, format!("{base}/targets"));
        }
        if primitive.mode != 4 {
            add(Kind::NonTrianglePrimitive, format!("{base}/mode"));
        }
        for attribute in &primitive.attributes {
            let semantic = attribute.semantic.as_str();
            let location = format!("{base}/attributes/{semantic}");
            if is_secondary_influence(semantic) {
                add(Kind::SecondarySkinInfluences, location);
            } else if !matches!(
                semantic,
                "POSITION" | "NORMAL" | "TEXCOORD_0" | "JOINTS_0" | "WEIGHTS_0"
            ) {
                add(Kind::UnsupportedVertexAttribute, location);
            }
        }
    }
    for skin in &manifest.skins {
        let location = format!("/skins/{}/inverseBindMatrices", skin.skin_index);
        let accessor = skin
            .inverse_bind_accessor_index
            .and_then(|index| manifest.accessors.get(index));
        match (skin.inverse_bind_accessor_index, skin.inverse_bind_count) {
            (None, _) => add(Kind::MissingInverseBinds, location),
            (Some(_), Some(0)) => add(Kind::EmptyInverseBindAccessor, location),
            (Some(_), Some(count)) if count != skin.joint_count as u64 => {
                add(Kind::InverseBindCountMismatch, location);
            }
            (Some(_), _)
                if !accessor.is_some_and(|accessor| {
                    accessor.buffer_view_index.is_some()
                        && accessor.component_type == 5126
                        && accessor.accessor_type == "MAT4"
                        && !accessor.sparse
                }) =>
            {
                add(Kind::UnreadableInverseBinds, location);
            }
            _ => {}
        }
    }
    for accessor_index in scale_bearing_accessors(manifest) {
        let Some(accessor) = manifest.accessors.get(accessor_index) else {
            add(
                Kind::UnsafeAccessorLayout,
                format!("/accessors/{accessor_index}"),
            );
            continue;
        };
        let element_size =
            rules::components_per_element(&accessor.accessor_type).map(|components| components * 4);
        let stride = accessor
            .buffer_view_index
            .and_then(|index| manifest.buffer_views.get(index))
            .and_then(|view| view.byte_stride);
        if accessor.sparse
            || accessor.normalized
            || accessor.component_type != 5126
            || accessor.buffer_view_index.is_none()
            || accessor.count == 0
            || element_size.is_none()
            || stride.is_some_and(|stride| Some(stride as usize) != element_size)
        {
            add(
                Kind::UnsafeAccessorLayout,
                format!("/accessors/{accessor_index}"),
            );
        }
    }
    out
}

fn is_secondary_influence(semantic: &str) -> bool {
    semantic
        .strip_prefix("JOINTS_")
        .or_else(|| semantic.strip_prefix("WEIGHTS_"))
        .and_then(|index| index.parse::<u32>().ok())
        .is_some_and(|index| index >= 1)
}

/// Accessor indices the conversion would have to rewrite, from the manifest.
fn scale_bearing_accessors(manifest: &GltfCapabilityManifest) -> BTreeSet<usize> {
    let mut out = BTreeSet::new();
    for primitive in &manifest.primitives {
        for attribute in &primitive.attributes {
            if attribute.semantic == "POSITION" {
                out.insert(attribute.accessor_index);
            }
        }
    }
    for skin in &manifest.skins {
        out.extend(skin.inverse_bind_accessor_index);
    }
    for channel in &manifest.animation_channels {
        if channel.target_path == "translation" {
            out.insert(channel.output_accessor_index);
        }
    }
    out
}

// --- Artifact ---------------------------------------------------------------

/// A rewritten glTF/GLB container and the exact locations that changed.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct GltfScaleArtifact {
    container: GltfContainerKind,
    bytes: Vec<u8>,
    rewritten_accessors: Vec<usize>,
    rewritten_json_pointers: Vec<String>,
    reencoded_buffers: Vec<usize>,
    declared_factor: f64,
}

impl GltfScaleArtifact {
    /// The rewritten container bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The container kind, unchanged from the source.
    pub fn container(&self) -> GltfContainerKind {
        self.container
    }

    /// Source accessor indices whose payload was rewritten, ascending and
    /// without repeats. One entry per **unique** accessor index, however many
    /// logical uses reach it.
    pub fn rewritten_accessors(&self) -> &[usize] {
        &self.rewritten_accessors
    }

    /// JSON pointers whose value was rewritten, in lexical order.
    ///
    /// Buffer URIs re-encoded during container reassembly are not domain
    /// rewrites and are reported by [`Self::reencoded_buffers`] instead.
    pub fn rewritten_json_pointers(&self) -> &[String] {
        &self.rewritten_json_pointers
    }

    /// Buffer indices whose data URI was re-encoded, ascending. Empty for a
    /// GLB whose only buffer is the BIN chunk.
    pub fn reencoded_buffers(&self) -> &[usize] {
        &self.reencoded_buffers
    }

    /// The factor the caller declared.
    pub fn declared_factor(&self) -> f64 {
        self.declared_factor
    }
}

// --- Errors -----------------------------------------------------------------

/// Typed, fail-closed rejection from [`rewrite_linear_units`] or
/// [`prove_rewritten_artifact`].
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum GltfScaleRewriteError {
    /// The source's raw capability manifest declares a domain the rewrite
    /// cannot preserve or convert.
    #[error("glTF scale rewrite rejected {count} unsupported source domain(s)")]
    Capability {
        /// Deterministically ordered typed violations.
        violations: Vec<GltfCapabilityViolation>,
        /// Number of violations, repeated for stable error rendering.
        count: usize,
    },
    /// Shared core planning or proof rejected the request.
    #[error(transparent)]
    Plan(#[from] ScaleError),
    /// The source or the rewritten artifact could not be read.
    #[error(transparent)]
    Load(#[from] LoadError),
    /// The rewritten container could not be emitted.
    #[error(transparent)]
    Write(#[from] WriteError),
    /// A length-bearing member has no registered field handler, so its value
    /// would be left in the source unit while everything around it moved.
    #[error("no registered length-field handler for {location}")]
    UnhandledLengthField {
        /// JSON pointer of the unconvertible member.
        location: String,
    },
    /// Two logical uses of one accessor disagree on how it converts.
    #[error("accessor {accessor_index} is used with two disagreeing rewrite rules")]
    ConflictingRewriteRule {
        /// The contested accessor index.
        accessor_index: usize,
    },
    /// An accessor selected for rewriting is not the dense `f32` layout the
    /// preflight vouched for.
    #[error("accessor {accessor_index} at {location} is not a rewritable dense f32 accessor")]
    UnrewritableAccessor {
        /// The accessor index.
        accessor_index: usize,
        /// JSON pointer of the accessor.
        location: String,
    },
    /// An `image` reads a buffer view overlapping an accessor the conversion
    /// rewrites, so converting would corrupt the image payload.
    ///
    /// #280's `inspect_accessor_layouts` ranges *accessors* only, so an image
    /// buffer view is invisible to its disjointness proof.
    #[error("{location} reads bytes that overlap rewritten accessor {accessor_index}")]
    ImagePayloadOverlap {
        /// JSON pointer of the offending image.
        location: String,
        /// The accessor whose rewritten range it overlaps.
        accessor_index: usize,
    },
    /// The source container cannot be reassembled without inventing a
    /// buffer-to-chunk mapping.
    #[error("source container cannot be reassembled: {reason}")]
    UnreassemblableContainer {
        /// Stable machine-readable reason.
        reason: &'static str,
    },
    /// One converted element has no usable `f32` image: the product is not
    /// finite, or a nonzero product flushed to zero.
    #[error("converted value {value} at {location} is not representable as f32")]
    ValueNotRepresentable {
        /// Located JSON pointer or accessor element identity.
        location: String,
        /// The `f64` product that could not be narrowed.
        value: f64,
    },
    /// An artifact-level proof claim failed.
    #[error("artifact proof claim {claim:?} observed {observed}, tolerance {tolerance}")]
    ArtifactProofFailed {
        /// Stable machine-readable claim identity.
        claim: &'static str,
        /// Observed residual, count, or difference.
        observed: f64,
        /// The bound it exceeded.
        tolerance: f64,
    },
}

// --- Rewrite ----------------------------------------------------------------

/// Rewrite `source`'s linear units by the caller-declared finite
/// `factor > 0`.
///
/// The factor is validated through
/// [`animsmith_core::scale::plan_scale`], so it carries exactly the shared
/// contract's `InvalidFactor` / `FactorNotRepresentable` boundary, and the
/// source document's shape is validated before a byte is written. The plan
/// itself does not drive the byte rewrite: the rewrite is driven by the raw
/// domain table of DESIGN.md Appendix D §D.2, which sees source payloads the
/// normalized document does not model.
///
/// # Errors
///
/// Returns [`GltfScaleRewriteError::Capability`] for a manifest declaring an
/// unpreservable domain, [`GltfScaleRewriteError::Plan`] for an invalid or
/// unrepresentable factor or a malformed source document,
/// [`GltfScaleRewriteError::UnhandledLengthField`] for a length field with no
/// registered handler, [`GltfScaleRewriteError::ConflictingRewriteRule`] when
/// one accessor is reached by two disagreeing rules,
/// [`GltfScaleRewriteError::UnrewritableAccessor`] for an accessor outside the
/// dense `f32` layout, [`GltfScaleRewriteError::ImagePayloadOverlap`] when an
/// image payload shares bytes with a converted accessor,
/// [`GltfScaleRewriteError::ValueNotRepresentable`] for an element whose
/// converted value has no `f32` image, and
/// [`GltfScaleRewriteError::Write`] when a GLB length field would overflow.
pub fn rewrite_linear_units(
    source: &GltfScaleSource,
    factor: f64,
) -> Result<GltfScaleArtifact, GltfScaleRewriteError> {
    let manifest = source.manifest();
    let facts = capability_facts(manifest);
    if !facts.is_supported() {
        let violations = manifest_violations(manifest);
        let count = violations.len();
        return Err(GltfScaleRewriteError::Capability { violations, count });
    }
    plan_scale(&ScaleRequest {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor },
        document: source.document(),
        capability: &facts,
    })?;

    let root = source
        .raw_json()
        .as_object()
        .ok_or_else(|| LoadError::Malformed("top-level glTF JSON is not an object".into()))?;
    if let Some(location) = rules::unhandled_length_fields(source.raw_json())
        .into_iter()
        .next()
    {
        return Err(GltfScaleRewriteError::UnhandledLengthField { location });
    }

    let accessor_rules = rules::collect_accessor_rules(root).map_err(|accessor_index| {
        GltfScaleRewriteError::ConflictingRewriteRule { accessor_index }
    })?;
    let mut spans = Vec::with_capacity(accessor_rules.len());
    for (&accessor_index, &rule) in &accessor_rules {
        spans.push((
            bytes::accessor_span(root, source.resolved_buffers(), accessor_index, rule)?,
            rule,
        ));
    }
    reject_image_payload_overlap(root, manifest, &spans)?;

    let mut buffers = source.resolved_buffers().to_vec();
    let mut extrema: BTreeMap<usize, ComponentExtrema> = BTreeMap::new();
    let mut modified: BTreeSet<usize> = BTreeSet::new();
    for &(span, rule) in &spans {
        extrema.insert(
            span.accessor_index,
            bytes::scale_span(&mut buffers, span, rule, factor)?,
        );
        modified.insert(span.buffer);
    }

    let mut json = source.raw_json().clone();
    let mut rewritten_json_pointers = Vec::new();
    for (pointer, rule) in rules::collect_json_rewrites(root) {
        rewrite_json_array(&mut json, &pointer, rule, factor)?;
        rewritten_json_pointers.push(pointer);
    }
    for (&accessor_index, &rule) in &accessor_rules {
        let observed = &extrema[&accessor_index];
        rewritten_json_pointers.extend(rewrite_accessor_bounds(
            &mut json,
            accessor_index,
            rule,
            factor,
            observed,
        )?);
    }
    rewritten_json_pointers.sort();

    // A buffer whose bytes never changed keeps its authored data URI, so the
    // only re-encoded buffers are the ones a rewritten accessor lives in.
    let reencoded_buffers = modified
        .iter()
        .copied()
        .filter(|&buffer_index| {
            manifest.buffers.get(buffer_index).is_some_and(|buffer| {
                buffer.source_kind == crate::capability::GltfBufferSourceKind::DataUri
            })
        })
        .collect();
    let out = container::assemble(manifest, &json, &buffers, &modified)?;
    Ok(GltfScaleArtifact {
        container: manifest.container,
        bytes: out,
        rewritten_accessors: accessor_rules.keys().copied().collect(),
        rewritten_json_pointers,
        reencoded_buffers,
        declared_factor: factor,
    })
}

/// Reject an `image` whose buffer view shares bytes with a converted accessor.
fn reject_image_payload_overlap(
    root: &Map<String, Value>,
    manifest: &GltfCapabilityManifest,
    spans: &[(AccessorSpan, AccessorRule)],
) -> Result<(), GltfScaleRewriteError> {
    let Some(images) = root.get("images").and_then(Value::as_array) else {
        return Ok(());
    };
    for (image_index, image) in images.iter().enumerate() {
        let Some(view_index) = image
            .get("bufferView")
            .and_then(Value::as_u64)
            .and_then(|index| usize::try_from(index).ok())
        else {
            continue;
        };
        let Some(view) = manifest.buffer_views.get(view_index) else {
            continue;
        };
        let start = view.byte_offset as usize;
        let end = start.saturating_add(view.byte_length as usize);
        for &(span, _) in spans {
            if span.buffer == view.buffer_index && start < span.end && span.start < end {
                return Err(GltfScaleRewriteError::ImagePayloadOverlap {
                    location: format!("/images/{image_index}/bufferView"),
                    accessor_index: span.accessor_index,
                });
            }
        }
    }
    Ok(())
}

/// Multiply the selected entries of a JSON numeric array in place.
fn rewrite_json_array(
    json: &mut Value,
    pointer: &str,
    rule: JsonArrayRule,
    factor: f64,
) -> Result<(), GltfScaleRewriteError> {
    let target = json
        .pointer_mut(pointer)
        .and_then(Value::as_array_mut)
        .filter(|values| values.len() == rule.expected_len())
        .ok_or_else(|| {
            LoadError::Malformed(format!(
                "{pointer} is not an array of {} numbers",
                rule.expected_len()
            ))
        })?;
    for (component, entry) in target.iter_mut().enumerate() {
        if !rule.scales_component(component) {
            continue;
        }
        let location = format!("{pointer}/{component}");
        let before = entry
            .as_f64()
            .ok_or_else(|| LoadError::Malformed(format!("{location} is not a number")))?;
        *entry = number(bytes::narrow(before * factor, &location)?, &location)?;
    }
    Ok(())
}

/// Convert an accessor's authored `min`/`max`, then reconcile each converted
/// bound against the bytes that were actually written.
///
/// Multiplying an authored bound in `f64` and narrowing can round `min`
/// *up* past the true scaled minimum (or `max` down below the true maximum),
/// which trips a glTF validator's bound check on a document that is otherwise
/// correct. Rather than nudge blindly, the observed per-component extrema of
/// the rewritten payload are folded in: a converted bound that still bounds
/// the data is kept exactly, and one that does not is widened to the observed
/// extreme. That is deterministic, always sufficient (a one-ULP nudge is not),
/// and minimal — it never tightens an authored bound that was already loose.
fn rewrite_accessor_bounds(
    json: &mut Value,
    accessor_index: usize,
    rule: AccessorRule,
    factor: f64,
    observed: &ComponentExtrema,
) -> Result<Vec<String>, GltfScaleRewriteError> {
    let mut rewritten = Vec::new();
    for (member, is_min) in [("min", true), ("max", false)] {
        let pointer = format!("/accessors/{accessor_index}/{member}");
        let Some(bounds) = json.pointer_mut(&pointer).and_then(Value::as_array_mut) else {
            continue;
        };
        if bounds.len() != observed.min.len() {
            return Err(LoadError::Malformed(format!(
                "{pointer} declares {} entries but the accessor has {} components",
                bounds.len(),
                observed.min.len()
            ))
            .into());
        }
        for (component, entry) in bounds.iter_mut().enumerate() {
            if !rule.scales_component(component) {
                continue;
            }
            let location = format!("{pointer}/{component}");
            let before = entry
                .as_f64()
                .ok_or_else(|| LoadError::Malformed(format!("{location} is not a number")))?;
            let converted = bytes::narrow(before * factor, &location)?;
            let reconciled = if is_min {
                converted.min(observed.min[component])
            } else {
                converted.max(observed.max[component])
            };
            *entry = number(reconciled, &location)?;
        }
        rewritten.push(pointer);
    }
    Ok(rewritten)
}

/// Render one converted `f32` as the shortest decimal that round-trips it.
fn number(value: f32, location: &str) -> Result<Value, GltfScaleRewriteError> {
    value
        .to_string()
        .parse::<f64>()
        .ok()
        .and_then(serde_json::Number::from_f64)
        .map(Value::Number)
        .ok_or_else(|| GltfScaleRewriteError::ValueNotRepresentable {
            location: location.to_owned(),
            value: f64::from(value),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::{
        GltfAccessorCapability, GltfAttributeCapability, GltfBufferCapability,
        GltfBufferSourceKind, GltfBufferViewCapability, GltfPrimitiveCapability,
        GltfSkinCapability,
    };

    fn manifest() -> GltfCapabilityManifest {
        GltfCapabilityManifest {
            container: GltfContainerKind::Gltf,
            buffers: vec![GltfBufferCapability {
                buffer_index: 0,
                source_kind: GltfBufferSourceKind::DataUri,
                declared_byte_length: 36,
            }],
            buffer_views: vec![GltfBufferViewCapability {
                buffer_view_index: 0,
                buffer_index: 0,
                byte_offset: 0,
                byte_length: 36,
                byte_stride: None,
            }],
            accessors: vec![GltfAccessorCapability {
                accessor_index: 0,
                buffer_view_index: Some(0),
                byte_offset: 0,
                component_type: 5126,
                accessor_type: "VEC3".to_owned(),
                count: 3,
                normalized: false,
                sparse: false,
            }],
            nodes: Vec::new(),
            animation_channels: Vec::new(),
            primitives: vec![GltfPrimitiveCapability {
                mesh_index: 0,
                primitive_index: 0,
                mode: 4,
                attributes: vec![GltfAttributeCapability {
                    semantic: "POSITION".to_owned(),
                    accessor_index: 0,
                }],
                morph_target_count: 0,
                morph_position_accessors: Vec::new(),
            }],
            instancing: Vec::new(),
            skins: Vec::new(),
            camera_count: 0,
            extensions: Vec::new(),
            extension_locations: Vec::new(),
            external_resource_locations: Vec::new(),
            extras_locations: Vec::new(),
            unknown_member_locations: Vec::new(),
        }
    }

    #[test]
    fn a_clean_manifest_projects_to_complete_supported_facts() {
        let facts = capability_facts(&manifest());
        assert_eq!(facts.coverage, ScaleCapabilityCoverage::Complete);
        assert!(facts.is_supported());
    }

    #[test]
    fn an_external_buffer_makes_coverage_unavailable_as_well_as_unsupported() {
        let mut manifest = manifest();
        manifest.buffers[0].source_kind = GltfBufferSourceKind::External;
        manifest.external_resource_locations = vec!["/buffers/0/uri".to_owned()];
        let facts = capability_facts(&manifest);
        assert_eq!(facts.coverage, ScaleCapabilityCoverage::Unavailable);
        assert!(facts.external_resources_present);
        assert!(!facts.is_supported());
    }

    #[test]
    fn every_unsupported_domain_sets_exactly_its_own_flag() {
        type Case = (
            &'static str,
            Box<dyn Fn(&mut GltfCapabilityManifest)>,
            fn(&ScaleCapabilityFacts) -> bool,
        );
        let cases: Vec<Case> = vec![
            (
                "morph targets",
                Box::new(|m| m.primitives[0].morph_target_count = 2),
                |f| f.morphs_present,
            ),
            ("camera", Box::new(|m| m.camera_count = 1), |f| {
                f.cameras_present
            }),
            (
                "extension",
                Box::new(|m| m.extensions = vec!["ACME_opaque".to_owned()]),
                |f| f.unregistered_extensions_present,
            ),
            (
                "punctual light",
                Box::new(|m| m.extensions = vec!["KHR_lights_punctual".to_owned()]),
                |f| f.lights_present,
            ),
            (
                "extras",
                Box::new(|m| m.extras_locations = vec!["/extras".to_owned()]),
                |f| f.extras_present,
            ),
            (
                "unknown member",
                Box::new(|m| m.unknown_member_locations = vec!["/nope".to_owned()]),
                |f| f.unknown_source_members_present,
            ),
            (
                "non-triangle mode",
                Box::new(|m| m.primitives[0].mode = 1),
                |f| f.non_triangle_primitives_present,
            ),
            (
                "unmodeled attribute",
                Box::new(|m| {
                    m.primitives[0].attributes.push(GltfAttributeCapability {
                        semantic: "TANGENT".to_owned(),
                        accessor_index: 0,
                    })
                }),
                |f| f.unsupported_vertex_attributes_present,
            ),
            (
                "secondary influences",
                Box::new(|m| {
                    m.primitives[0].attributes.push(GltfAttributeCapability {
                        semantic: "JOINTS_1".to_owned(),
                        accessor_index: 0,
                    })
                }),
                |f| f.secondary_skin_influences_present,
            ),
            (
                "missing inverse binds",
                Box::new(|m| {
                    m.skins = vec![GltfSkinCapability {
                        skin_index: 0,
                        joint_count: 1,
                        inverse_bind_accessor_index: None,
                        inverse_bind_count: None,
                    }]
                }),
                |f| f.inverse_bind_issues_present,
            ),
            (
                "interleaved POSITION",
                Box::new(|m| m.buffer_views[0].byte_stride = Some(16)),
                |f| f.unsafe_accessor_layout_present,
            ),
            (
                "normalized POSITION",
                Box::new(|m| m.accessors[0].normalized = true),
                |f| f.unsafe_accessor_layout_present,
            ),
            (
                "sparse POSITION",
                Box::new(|m| m.accessors[0].sparse = true),
                |f| f.unsafe_accessor_layout_present,
            ),
        ];
        for (name, mutate, flag) in cases {
            let mut manifest = manifest();
            mutate(&mut manifest);
            let facts = capability_facts(&manifest);
            assert!(flag(&facts), "{name} did not set its capability flag");
            assert!(!facts.is_supported(), "{name} was still reported supported");
        }
    }

    #[test]
    fn an_animated_weights_channel_is_projected_as_morph_weights() {
        use crate::capability::GltfAnimationChannelCapability;
        let mut manifest = manifest();
        manifest.animation_channels = vec![GltfAnimationChannelCapability {
            animation_index: 0,
            channel_index: 0,
            target_node_index: 0,
            target_path: "weights".to_owned(),
            interpolation: "LINEAR".to_owned(),
            input_accessor_index: 1,
            output_accessor_index: 2,
        }];
        let facts = capability_facts(&manifest);
        assert!(facts.morph_weights_present);
        assert!(!facts.is_supported());
    }
}
