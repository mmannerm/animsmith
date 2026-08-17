//! Preservation-safe whole-document linear-unit rewriting of raw glTF/GLB
//! bytes (DESIGN.md Appendix D §D.2 and ownership boundaries in §D.8).
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
//! Correctly rounded JSON parsing is part of this preservation boundary:
//! `serde_json`'s `float_roundtrip` feature guarantees that `ryu`'s shortest
//! finite `f64` spelling reparses to the same value. Converted numbers are
//! narrowed to `f32` exactly once and written back as the shortest decimal
//! that round-trips that `f32`, so the JSON and buffer payloads live in the
//! same numeric regime the glTF schema declares. Writing the `f32`'s full
//! `f64` widening would be longer without carrying more model information.

mod bytes;
mod container;
mod plan;
mod proof;
mod rest_bind;
mod rest_bind_proof;
mod rules;

use crate::capability::{
    GltfCapabilityManifest, GltfCapabilityViolation, GltfCapabilityViolationKind,
    GltfContainerKind, GltfScaleSource, NodeTransformFault, node_transform_faults,
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
pub use rest_bind::rewrite_rest_bind;
pub use rest_bind_proof::prove_rewritten_rest_bind;

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
/// Four preflight violation kinds are **not** re-derivable here at all,
/// because the manifest records neither resolved byte ranges, nor images, nor
/// what a node authored beyond its rest kind:
/// `OverlappingAccessorRanges`, `ImagePayloadOverlap`,
/// `ConflictingNodeTransform` and `NonAffineNodeMatrix`. A manifest evidencing
/// one of those can still project to supported facts. That is safe rather than
/// merely tolerated: a [`GltfScaleSource`] exists only where
/// [`crate::preflight_scale_source_bytes`] found no violation at all, and
/// [`rewrite_linear_units`] re-checks each of the four itself. The preflight's
/// own violation list remains the authority, and [`rewrite_linear_units`]
/// independently re-resolves every accessor it touches rather than trusting
/// these flags.
pub fn capability_facts(manifest: &GltfCapabilityManifest) -> ScaleCapabilityFacts {
    let violations = manifest_violations(manifest);
    capability_facts_from_violations(manifest, &violations)
}

fn capability_facts_from_violations(
    manifest: &GltfCapabilityManifest,
    violations: &[GltfCapabilityViolation],
) -> ScaleCapabilityFacts {
    let mut facts = ScaleCapabilityFacts::default();
    facts.coverage = ScaleCapabilityCoverage::Complete;
    if manifest
        .buffers
        .iter()
        .any(|buffer| buffer.source_kind == crate::capability::GltfBufferSourceKind::External)
    {
        facts.coverage = ScaleCapabilityCoverage::Unavailable;
    }
    for violation in violations {
        record_violation(&mut facts, violation.kind);
    }
    facts.morphs_present = manifest
        .primitives
        .iter()
        .any(|primitive| primitive.morph_target_count > 0);
    facts.morph_weights_present = !manifest.morph_weight_locations.is_empty();
    facts.whole_document_morphs_preservable = (facts.morphs_present || facts.morph_weights_present)
        && manifest
            .primitives
            .iter()
            .all(|primitive| primitive.unsupported_morph_locations.is_empty());
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
        | Kind::OverlappingAccessorRanges
        | Kind::ImagePayloadOverlap => facts.unsafe_accessor_layout_present = true,
        // The typed glTF parse honours `matrix` and silently ignores the TRS
        // members beside it, and decomposes `matrix` to TRS while dropping a
        // projective last row: in both shapes the source carries transform
        // members the normalized model does not represent.
        Kind::ConflictingNodeTransform | Kind::NonAffineNodeMatrix | Kind::AnimatedMatrixNode => {
            facts.unknown_source_members_present = true;
        }
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
    // Animation and node counts are independently controlled by the source.
    // Index matrix-authored nodes once instead of rescanning every node for
    // every channel, which would make this untrusted-input pass quadratic.
    let matrix_nodes = manifest
        .nodes
        .iter()
        .filter_map(|node| {
            (node.rest_kind == crate::capability::GltfNodeRestKind::Matrix)
                .then_some(node.node_index)
        })
        .collect::<BTreeSet<_>>();
    for channel in &manifest.animation_channels {
        if matrix_nodes.contains(&channel.target_node_index) {
            add(
                Kind::AnimatedMatrixNode,
                format!(
                    "/animations/{}/channels/{}/target",
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
        for location in &primitive.unsupported_morph_locations {
            add(Kind::MorphTarget, location.clone());
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

fn require_scale_capability(
    manifest: &GltfCapabilityManifest,
    operation: ScaleOperation,
) -> Result<ScaleCapabilityFacts, GltfScaleRewriteError> {
    let mut violations = manifest_violations(manifest);
    let facts = capability_facts_from_violations(manifest, &violations);
    if facts.is_supported_for(operation) {
        return Ok(facts);
    }
    if matches!(operation, ScaleOperation::RestBindUniformScale { .. }) {
        for primitive in &manifest.primitives {
            if primitive.morph_target_count > 0 {
                violations.push(GltfCapabilityViolation {
                    kind: GltfCapabilityViolationKind::MorphTarget,
                    location: format!(
                        "/meshes/{}/primitives/{}/targets",
                        primitive.mesh_index, primitive.primitive_index
                    ),
                });
            }
        }
        violations.extend(
            manifest
                .morph_weight_locations
                .iter()
                .cloned()
                .map(|location| GltfCapabilityViolation {
                    kind: GltfCapabilityViolationKind::MorphWeights,
                    location,
                }),
        );
        violations.sort_by(|left, right| {
            (left.kind, left.location.as_str()).cmp(&(right.kind, right.location.as_str()))
        });
        violations.dedup();
    }
    let count = violations.len();
    Err(GltfScaleRewriteError::Capability { violations, count })
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
        out.extend(primitive.morph_position_accessors.iter().copied());
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
    affected_source_nodes: Vec<usize>,
    affected_source_skins: Vec<usize>,
    declared_factor: f64,
    operation: ScaleOperation,
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
    ///
    /// [`prove_rewritten_artifact`] checks this list against its own
    /// independent scan, so it is evidence rather than an unverified label.
    pub fn rewritten_json_pointers(&self) -> &[String] {
        &self.rewritten_json_pointers
    }

    /// Buffer indices whose data URI was re-encoded, ascending. Empty for a
    /// GLB whose only buffer is the BIN chunk.
    pub fn reencoded_buffers(&self) -> &[usize] {
        &self.reencoded_buffers
    }

    /// The affected closure as **source-node array indices**, ascending.
    ///
    /// Reported in the raw glTF index space the operation's own selectors use
    /// — `/nodes/{i}` — not as normalized [`animsmith_core::BoneId`]s. A
    /// producer recording which identities an artifact affected must name
    /// them in the space the request named, and a consumer holding the
    /// original file can resolve these directly against its `nodes` array.
    /// [`animsmith_core::scale::ScalePlan::affected_nodes`] reports the same
    /// closure in the normalized space; the frontend already proves the two
    /// describe one tree before it writes a byte.
    ///
    /// For [`ScaleOperation::RestBindUniformScale`] this is the closed
    /// connected hierarchy of DESIGN.md Appendix D §D.2. For
    /// [`ScaleOperation::WholeDocumentLinearUnits`] it is every node the
    /// source declares, that operation's closure being the whole document.
    pub fn affected_source_nodes(&self) -> &[usize] {
        &self.affected_source_nodes
    }

    /// The affected skins as **source-skin array indices**, ascending, in the
    /// same raw index space as [`Self::affected_source_nodes`].
    ///
    /// For [`ScaleOperation::RestBindUniformScale`] a skin is affected when at
    /// least one of its joints lies inside the closure — which is exactly the
    /// condition under which its `inverseBindMatrices` accessor is rebased in
    /// at least one slot. A skin straddling the closure boundary is listed:
    /// it *is* affected, in the slots whose joints are. For
    /// [`ScaleOperation::WholeDocumentLinearUnits`] it is every skin the
    /// source declares.
    pub fn affected_source_skins(&self) -> &[usize] {
        &self.affected_source_skins
    }

    /// The factor the caller declared: the conversion factor `q` for a
    /// whole-document conversion, the expected common factor `s` for a
    /// rest/bind reparameterization.
    pub fn declared_factor(&self) -> f64 {
        self.declared_factor
    }

    /// The operation that produced this artifact, echoed with the selectors
    /// the caller declared.
    ///
    /// Reported so a proof can refuse to check a rest/bind artifact against a
    /// whole-document plan, or the reverse: the two operations write
    /// different domains, and a factor alone does not distinguish them.
    pub fn operation(&self) -> ScaleOperation {
        self.operation
    }
}

// --- Errors -----------------------------------------------------------------

/// The structural relationship of one raw JSON difference to the source.
///
/// Values are intentionally not retained: proof diagnostics identify where
/// preservation failed without copying potentially sensitive source payloads
/// into logs or machine-readable output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum GltfRawJsonDifferenceKind {
    /// The artifact declares a member the source did not.
    ArtifactAdded,
    /// The source declares a member the artifact removed.
    ArtifactRemoved,
    /// Both sides declare the location, but its value or shape changed.
    ValueChanged,
}

impl std::fmt::Display for GltfRawJsonDifferenceKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::ArtifactAdded => "artifact-added",
            Self::ArtifactRemoved => "artifact-removed",
            Self::ValueChanged => "value-changed",
        })
    }
}

/// One value-free raw JSON difference found by an artifact preservation proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GltfRawJsonDifference {
    /// RFC 6901 JSON pointer of a deterministic difference root.
    ///
    /// Object members and equal-length array elements are walked recursively.
    /// Unequal arrays are reported once at their array root because their
    /// element identities no longer pair one-to-one.
    pub pointer: String,
    /// How the artifact differs from the source at [`Self::pointer`].
    pub kind: GltfRawJsonDifferenceKind,
}

/// Bounded raw JSON diagnostics for an artifact preservation proof failure.
///
/// [`Self::differences`] contains at most sixteen entries. The full count is
/// the retained length plus [`Self::omitted`], the exact number not retained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GltfRawJsonDifferenceSummary {
    /// Deterministic prefix of differences, ordered by the JSON tree walk.
    pub differences: Vec<GltfRawJsonDifference>,
    /// Exact number of differences not retained in [`Self::differences`].
    pub omitted: usize,
}

struct RawJsonDifferenceSuffix<'a>(Option<&'a GltfRawJsonDifferenceSummary>);

impl std::fmt::Display for RawJsonDifferenceSuffix<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Some(summary) = self.0 else {
            return Ok(());
        };
        formatter.write_str("; raw JSON differences: ")?;
        for (index, difference) in summary.differences.iter().enumerate() {
            if index > 0 {
                formatter.write_str(", ")?;
            }
            write!(formatter, "{} ({})", difference.pointer, difference.kind)?;
        }
        if summary.omitted > 0 {
            write!(formatter, "; {} omitted", summary.omitted)?;
        }
        Ok(())
    }
}

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
    /// A node declares `matrix` alongside a TRS member. glTF 2.0 forbids the
    /// combination, and the `gltf` crate accepts it, so the conversion would
    /// otherwise emit two rewrites for one node's single transform.
    #[error("{location} declares a TRS member alongside matrix")]
    ConflictingNodeTransform {
        /// JSON pointer of the TRS member that conflicts with `matrix`.
        location: String,
    },
    /// A node `matrix` is not TRS-decomposable: its last row is not
    /// `(0, 0, 0, 1)`, so it carries a projective component that transforms
    /// as `1/q` rather than staying dimensionless under `U M U^-1`.
    #[error("{location} is {value}, so the node matrix is not TRS-decomposable")]
    NonAffineNodeMatrix {
        /// JSON pointer of the offending matrix entry.
        location: String,
        /// The authored value.
        value: f64,
        /// The only value glTF 2.0 permits there.
        expected: f64,
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
    /// Two logical uses of one accessor demand different rest/bind factors,
    /// so no single rewrite of that accessor can satisfy both.
    ///
    /// This is a fail-closed domain #280 does not produce. Its aliasing guard
    /// is a two-value classification — scale-bearing versus dimensionless —
    /// and fires only on the cross; the scale-bearing/scale-bearing cross is
    /// *accepted*, correctly, because a whole-document conversion multiplies
    /// every such use by the same `q`. Under a rest/bind reparameterization
    /// the multiplier differs per node and per skin slot, so the same sharing
    /// makes "rebase affected translation animation values" and "preserve
    /// declared unaffected payloads" simultaneously unsatisfiable. The
    /// manifest is clean and the file is valid glTF; it is the *plan* that
    /// makes the sharing unsatisfiable.
    ///
    /// Splitting the accessor is not the remedy: it would change the
    /// `accessors` and `bufferViews` array lengths and destroy the array
    /// identities the artifact proof pins. Both claimants are named so the
    /// source can be fixed instead.
    #[error(
        "accessor {accessor_index} element {element} must scale by {first_factor} for {first_location} and by {second_factor} for {second_location}"
    )]
    ConflictingRestBindFactor {
        /// The contested accessor index.
        accessor_index: usize,
        /// First element index at which the two claims disagree.
        element: usize,
        /// JSON pointer of the first use to claim this accessor.
        first_location: String,
        /// The factor that use demands at `element`.
        first_factor: f64,
        /// JSON pointer of the use that disagreed.
        second_location: String,
        /// The factor that use demands at `element`.
        second_factor: f64,
    },
    /// The affected closure derived from the raw node hierarchy is not the
    /// closure [`animsmith_core::scale::plan_scale`] planned.
    ///
    /// The plan walks `SourceNodeAsset::parent_source_node_index`; this crate
    /// walks `/nodes/*/children`. `animsmith_core` requires the projection to
    /// agree with the normalized skeleton, but it never sees the raw child
    /// arrays, so a projection that contradicts the JSON it was derived from
    /// plans, builds and proves cleanly there.
    #[error(
        "the plan's affected closure {planned:?} is not the closure {derived:?} derived from the raw node hierarchy"
    )]
    ClosureMismatch {
        /// Plan closure, as source-node indices in ascending order.
        planned: Vec<usize>,
        /// Raw-hierarchy closure, as source-node indices in ascending order.
        derived: Vec<usize>,
    },
    /// A node's parent in the normalized skeleton is not its parent in the
    /// raw node hierarchy, so the two disagree about which nodes inherit the
    /// factor being removed.
    #[error(
        "source node {source_node_index} has a different parent in the skeleton than in the raw hierarchy"
    )]
    ParentChainDisagreement {
        /// The source node whose two parent links disagree.
        source_node_index: usize,
    },
    /// Two source nodes claim the same normalized [`animsmith_core::BoneId`],
    /// so the plan's bone-keyed closure cannot be resolved back to a unique
    /// source node to rewrite.
    #[error("two source nodes both normalized to bone {bone}")]
    AmbiguousSourceNodeProjection {
        /// The contested bone.
        bone: animsmith_core::BoneId,
    },
    /// The raw node hierarchy cannot support the requested closure.
    #[error("source node hierarchy is unusable: {reason}")]
    UnusableSourceHierarchy {
        /// Stable machine-readable reason.
        reason: &'static str,
    },
    /// An artifact-level proof claim failed.
    #[error(
        "artifact proof claim {claim:?} observed {observed}, tolerance {tolerance}{diagnostics}",
        diagnostics = RawJsonDifferenceSuffix(.raw_json_differences.as_ref())
    )]
    ArtifactProofFailed {
        /// Stable machine-readable claim identity.
        claim: &'static str,
        /// Observed residual, count, or difference.
        observed: f64,
        /// The bound it exceeded.
        tolerance: f64,
        /// Bounded, value-free locations for a raw JSON preservation failure.
        ///
        /// Every other artifact proof claim carries `None` because its
        /// existing typed fields already identify the failed obligation.
        raw_json_differences: Option<GltfRawJsonDifferenceSummary>,
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
/// drives the byte rewrite's semantic membership. The glTF adapter separately
/// validates raw topology, aliases, ranges, container fields, and payloads the
/// normalized document does not model, then maps the typed rows onto them.
///
/// # Errors
///
/// Returns [`GltfScaleRewriteError::Capability`] for a manifest declaring an
/// unpreservable domain, [`GltfScaleRewriteError::Plan`] for an invalid or
/// unrepresentable factor or a malformed source document,
/// [`GltfScaleRewriteError::UnhandledLengthField`] for a length field with no
/// registered handler, [`GltfScaleRewriteError::ConflictingNodeTransform`] for
/// a node declaring `matrix` alongside a TRS member,
/// [`GltfScaleRewriteError::NonAffineNodeMatrix`] for a node `matrix` that is
/// not TRS-decomposable, [`GltfScaleRewriteError::ConflictingRewriteRule`] when
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
    let operation = ScaleOperation::WholeDocumentLinearUnits { factor };
    let facts = require_scale_capability(source.manifest(), operation)?;
    let plan = plan_scale(&ScaleRequest {
        operation,
        document: source.document(),
        capability: &facts,
    })?;

    rewrite_linear_units_plan(source, &plan)
}

/// Apply one already-compiled core scale plan to the raw glTF source.
///
/// This is the shared writer boundary used when a caller will prove the
/// artifact with the same immutable plan. The operation-specific public
/// convenience functions remain available and compile a plan before
/// delegating here.
///
/// # Errors
///
/// Returns the same capability, plan-replay, raw-layout, aliasing,
/// representability, and container-write errors as the corresponding
/// operation-specific writer. A plan for another document or a plan whose
/// operation cannot be represented by this glTF boundary is rejected before
/// a byte is written.
pub fn rewrite_scale_plan(
    source: &GltfScaleSource,
    plan: &animsmith_core::scale::ScalePlan,
) -> Result<GltfScaleArtifact, GltfScaleRewriteError> {
    require_scale_capability(source.manifest(), plan.operation())?;
    match plan.operation() {
        ScaleOperation::WholeDocumentLinearUnits { .. } => rewrite_linear_units_plan(source, plan),
        ScaleOperation::RestBindUniformScale { .. } => {
            rest_bind::rewrite_rest_bind_plan(source, plan)
        }
        _ => Err(plan::plan_mismatch("gltf_operation_plan_mismatch")),
    }
}

fn rewrite_linear_units_plan(
    source: &GltfScaleSource,
    plan: &animsmith_core::scale::ScalePlan,
) -> Result<GltfScaleArtifact, GltfScaleRewriteError> {
    let manifest = source.manifest();
    let ScaleOperation::WholeDocumentLinearUnits { factor } = plan.operation() else {
        return Err(plan::plan_mismatch("gltf_operation_plan_mismatch"));
    };
    let gltf_plan = plan::GltfScalePlan::new(source, plan)?;

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
    reject_out_of_contract_nodes(root)?;

    let accessor_rules = rules::collect_accessor_rules(&gltf_plan, factor != 1.0)?;
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
    for (pointer, rule) in rules::collect_json_rewrites(&gltf_plan, factor != 1.0)? {
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
        // This operation's closure is the whole document, so every declared
        // node and skin is affected. Counted from the raw arrays rather than
        // from the manifest's own vectors: the manifest inventories the same
        // arrays, but the closure is a claim about the source JSON and is
        // read from it.
        affected_source_nodes: gltf_plan.affected_source_nodes(false),
        affected_source_skins: (0..raw_array_len(root, "skins")).collect(),
        declared_factor: factor,
        operation: plan.operation(),
    })
}

/// Length of a top-level glTF array member, zero when it is absent or is not
/// an array.
fn raw_array_len(root: &Map<String, Value>, key: &str) -> usize {
    root.get(key).and_then(Value::as_array).map_or(0, Vec::len)
}

/// Reject a node whose transform is outside the glTF 2.0 contract.
///
/// The classification itself lives in
/// [`crate::capability::node_transform_faults`], which is also what #280's
/// preflight reports, so the gate and this guard cannot disagree about which
/// nodes are out of contract. This guard is kept as defence in depth: it is
/// the layer that must hold if the preflight is ever relaxed, and it names
/// the consequence for *this* operation, which is that
/// [`rules::collect_json_rewrites`] would emit one rewrite for `matrix` and
/// one for the conflicting TRS member — converting a single node's transform
/// twice under two different rules — and that `M' = U M U^-1` leaves the last
/// row alone, which is only the converted transform when that row is affine.
///
/// Its call site above is test-protected: `rewrite_linear_units` is driven
/// past a synthetic gate by `capability::scale_source_past_the_gate`, so
/// deleting the call fails a test rather than passing silently.
fn reject_out_of_contract_nodes(root: &Map<String, Value>) -> Result<(), GltfScaleRewriteError> {
    let Some(nodes) = root.get("nodes").and_then(Value::as_array) else {
        return Ok(());
    };
    let Some(fault) = node_transform_faults(nodes).into_iter().next() else {
        return Ok(());
    };
    let location = fault.location();
    Err(match fault {
        NodeTransformFault::TrsBesideMatrix { .. } => {
            GltfScaleRewriteError::ConflictingNodeTransform { location }
        }
        NodeTransformFault::ProjectiveMatrixEntry {
            value, expected, ..
        } => GltfScaleRewriteError::NonAffineNodeMatrix {
            location,
            value,
            expected,
        },
        // A `matrix` entry that is not a number fails the typed glTF parse
        // before a source reaches here, so shape errors keep one owner.
        NodeTransformFault::UnreadableMatrixEntry { .. } => {
            LoadError::Malformed(format!("{location} is not a number")).into()
        }
    })
}

/// Reject an `image` whose buffer view shares bytes with a converted accessor.
///
/// An empty image view is skipped, matching
/// [`crate::capability::image_payload_ranges`], which admits a range only when
/// `start < end`. Both walkers compare half-open ranges, under which an empty
/// range shares no byte with anything — including a range it sits inside — so
/// skipping it is the same answer the overlap test would give, not a
/// relaxation of it. Without the skip the predicate below degenerates for
/// `start == end` into "the view's offset lies strictly inside the span", and
/// the gate would accept a source this guard refuses. (`byteLength: 0` is
/// schema-invalid — glTF 2.0 gives `bufferView.byteLength` `minimum: 1` — so
/// this decides an unreachable case rather than a supported one; it is pinned
/// because the two walkers must not drift, not because the shape is expected.)
///
/// Its call site above is test-protected the same way
/// [`reject_out_of_contract_nodes`] is.
fn reject_image_payload_overlap(
    root: &Map<String, Value>,
    manifest: &GltfCapabilityManifest,
    spans: &[(AccessorSpan, AccessorRule)],
) -> Result<(), GltfScaleRewriteError> {
    reject_image_payload_overlap_spans(root, manifest, spans.iter().map(|(span, _)| *span))
}

/// [`reject_image_payload_overlap`] over a bare span sequence, so the
/// rest/bind rewrite — whose spans carry per-slot claims rather than
/// [`AccessorRule`]s — shares one implementation with the whole-document
/// conversion instead of growing a second one.
fn reject_image_payload_overlap_spans(
    root: &Map<String, Value>,
    manifest: &GltfCapabilityManifest,
    spans: impl Iterator<Item = AccessorSpan> + Clone,
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
        if start >= end {
            continue;
        }
        for span in spans.clone() {
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
    rewrite_accessor_bounds_with(
        json,
        accessor_index,
        &|component| rule.scales_component(component),
        Some(factor),
        observed,
    )
}

/// [`rewrite_accessor_bounds`] for a rewrite whose per-element factors need
/// not agree.
///
/// `factor` is the single multiplier every element of this accessor shares,
/// when there is one. `None` means the accessor's elements were rebased by
/// *different* factors — one `inverseBindMatrices` accessor whose joints
/// straddle the affected closure — and an authored bound then has no single
/// conversion at all. In that case the emitted bound is the observed extremum
/// of the rewritten payload: still deterministic, still sufficient, and the
/// only choice that cannot claim a bound the data does not satisfy. It can
/// tighten a loose authored bound, which is a fact about a document that
/// declares `min`/`max` on a partially-rebased matrix accessor, not a general
/// behaviour — for a single shared factor the authored bound is preserved
/// exactly as before.
fn rewrite_accessor_bounds_with(
    json: &mut Value,
    accessor_index: usize,
    scales_component: &dyn Fn(usize) -> bool,
    factor: Option<f64>,
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
            if !scales_component(component) {
                continue;
            }
            let location = format!("{pointer}/{component}");
            let before = entry
                .as_f64()
                .ok_or_else(|| LoadError::Malformed(format!("{location} is not a number")))?;
            let converted = match factor {
                Some(factor) => bytes::narrow(before * factor, &location)?,
                None if is_min => observed.min[component],
                None => observed.max[component],
            };
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
                unsupported_morph_locations: Vec::new(),
            }],
            morph_weight_locations: Vec::new(),
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

    // --- Defence in depth -------------------------------------------------
    //
    // #280's preflight now refuses both out-of-contract node transforms
    // (#301) and image payloads aliasing a converted accessor (#300), so no
    // `GltfScaleSource` carrying either can be built through the public API
    // and these guards are unreachable from an integration test. They are
    // deliberately kept — they are the layer that must hold if the gate is
    // relaxed — so they are exercised directly here instead.
    //
    // Two things need proving, and they are not the same thing. That each
    // guard *classifies* correctly is proved by calling it directly. That
    // each guard is still *wired into* `rewrite_linear_units` is proved by
    // `capability::scale_source_past_the_gate`, the `cfg(test)`-only seam
    // that builds a `GltfScaleSource` from bytes the gate would refuse —
    // the synthetic relaxation the guards exist for. Without it, deleting a
    // guard's call site changes no observable behaviour and no test fails.

    /// The identity node `matrix`, column-major.
    const IDENTITY_MATRIX: [f64; 16] = [
        1.0, 0.0, 0.0, 0.0, //
        0.0, 1.0, 0.0, 0.0, //
        0.0, 0.0, 1.0, 0.0, //
        0.0, 0.0, 0.0, 1.0,
    ];

    fn nodes_root(nodes: Value) -> Map<String, Value> {
        serde_json::json!({ "nodes": nodes })
            .as_object()
            .expect("literal JSON object")
            .clone()
    }

    #[test]
    fn the_rewriter_guard_still_refuses_a_matrix_beside_a_trs_member() {
        for (member, member_value) in [
            ("translation", serde_json::json!([1.5, -2.0, 0.25])),
            ("rotation", serde_json::json!([0.0, 0.0, 0.0, 1.0])),
            ("scale", serde_json::json!([2.0, 2.0, 2.0])),
        ] {
            let mut node = serde_json::json!({ "matrix": Vec::from(IDENTITY_MATRIX) });
            node[member] = member_value;
            match reject_out_of_contract_nodes(&nodes_root(serde_json::json!([node]))) {
                Err(GltfScaleRewriteError::ConflictingNodeTransform { location }) => {
                    assert_eq!(location, format!("/nodes/0/{member}"));
                }
                other => panic!("matrix + {member} must be refused, got {other:?}"),
            }
        }
    }

    #[test]
    fn the_rewriter_guard_still_refuses_a_projective_node_matrix() {
        for (component, authored, expected) in [
            (3usize, 0.5f64, 0.0f64),
            (7, -1.0, 0.0),
            (11, 2.0, 0.0),
            (15, 2.0, 1.0),
        ] {
            let mut matrix = IDENTITY_MATRIX;
            matrix[component] = authored;
            let node = serde_json::json!({ "matrix": Vec::from(matrix) });
            match reject_out_of_contract_nodes(&nodes_root(serde_json::json!([node]))) {
                Err(GltfScaleRewriteError::NonAffineNodeMatrix {
                    location,
                    value,
                    expected: reported,
                }) => {
                    assert_eq!(location, format!("/nodes/0/matrix/{component}"));
                    assert_eq!(value, authored);
                    assert_eq!(reported, expected);
                }
                other => panic!("matrix[{component}] = {authored} must be refused, got {other:?}"),
            }
        }
    }

    #[test]
    fn the_rewriter_guard_accepts_an_affine_matrix_with_a_translation_column() {
        let mut matrix = IDENTITY_MATRIX;
        matrix[12] = 1.5;
        matrix[13] = -2.0;
        matrix[14] = 0.25;
        let node = serde_json::json!({ "matrix": Vec::from(matrix) });
        reject_out_of_contract_nodes(&nodes_root(serde_json::json!([node])))
            .expect("an affine matrix with a translation column is in contract");
        reject_out_of_contract_nodes(&nodes_root(serde_json::json!([{
            "translation": [1.5, -2.0, 0.25],
            "rotation": [0.0, 0.0, 0.0, 1.0],
            "scale": [2.0, 2.0, 2.0]
        }])))
        .expect("TRS without matrix declares no conflict");
    }

    #[test]
    fn a_non_numeric_matrix_entry_stays_a_malformed_source_rather_than_a_contract_fault() {
        let mut matrix: Vec<Value> = Vec::from(IDENTITY_MATRIX)
            .into_iter()
            .map(Value::from)
            .collect();
        matrix[15] = Value::from("1.0");
        let node = serde_json::json!({ "matrix": matrix });
        match reject_out_of_contract_nodes(&nodes_root(serde_json::json!([node]))) {
            Err(GltfScaleRewriteError::Load(_)) => {}
            other => panic!("a non-numeric matrix entry is malformed, got {other:?}"),
        }
    }

    /// [`reject_image_payload_overlap`] for one image view and one converted
    /// accessor span, both in buffer 0.
    ///
    /// The image sits on `bufferView 2`, behind two decoy views that share no
    /// byte with any span, so a guard reading a fixed view rather than the
    /// indexed one answers from a range that is never the image's.
    fn image_overlap(image: (u64, u64), span: (usize, usize)) -> Result<(), GltfScaleRewriteError> {
        let root = serde_json::json!({ "images": [{ "bufferView": 2, "mimeType": "image/png" }] })
            .as_object()
            .expect("literal JSON object")
            .clone();
        let decoy = |buffer_view_index| GltfBufferViewCapability {
            buffer_view_index,
            buffer_index: 1,
            byte_offset: 0,
            byte_length: 4096,
            byte_stride: None,
        };
        let mut manifest = manifest();
        manifest.buffer_views = vec![
            decoy(0),
            decoy(1),
            GltfBufferViewCapability {
                buffer_view_index: 2,
                buffer_index: 0,
                byte_offset: image.0,
                byte_length: image.1,
                byte_stride: None,
            },
        ];
        let spans = vec![(
            AccessorSpan {
                accessor_index: 0,
                buffer: 0,
                start: span.0,
                end: span.1,
                components: 3,
            },
            AccessorRule::AllComponents,
        )];
        reject_image_payload_overlap(&root, &manifest, &spans)
    }

    #[test]
    fn the_rewriter_guard_still_refuses_an_image_payload_over_a_converted_span() {
        for (name, image, span) in [
            (
                "image runs one byte into the span",
                (0u64, 13u64),
                (12usize, 48usize),
            ),
            ("span runs one byte into the image", (35, 13), (0, 36)),
        ] {
            match image_overlap(image, span) {
                Err(GltfScaleRewriteError::ImagePayloadOverlap {
                    location,
                    accessor_index,
                }) => {
                    assert_eq!(location, "/images/0/bufferView", "{name}");
                    assert_eq!(accessor_index, 0, "{name}");
                }
                other => panic!("{name}: expected ImagePayloadOverlap, got {other:?}"),
            }
        }
    }

    #[test]
    fn the_rewriter_guard_accepts_an_image_payload_adjacent_to_a_converted_span() {
        // Both ranges are half-open, so touching endpoints share no byte.
        for (name, image, span) in [
            (
                "image ends where the span begins",
                (0u64, 12u64),
                (12usize, 48usize),
            ),
            ("image begins where the span ends", (36, 12), (0, 36)),
        ] {
            image_overlap(image, span)
                .unwrap_or_else(|error| panic!("{name}: adjacency is not an overlap: {error:?}"));
        }
    }

    #[test]
    fn an_empty_image_view_inside_a_converted_span_is_not_an_overlap() {
        // A `byteLength: 0` view covers no byte, so under the half-open
        // comparison it aliases nothing — not even a span it sits inside.
        // `capability::image_payload_ranges` drops the same shape before
        // comparing, and the two walkers must give one answer: without the
        // skip this predicate degenerates for `start == end` into "the
        // offset lies strictly inside the span", and the gate would accept
        // what this guard refused.
        for (name, image, span) in [
            (
                "empty view inside the span",
                (12u64, 0u64),
                (0usize, 36usize),
            ),
            ("empty view at the span's start", (0, 0), (0, 36)),
            ("empty view at the span's end", (36, 0), (0, 36)),
        ] {
            image_overlap(image, span)
                .unwrap_or_else(|error| panic!("{name}: an empty view aliases nothing: {error:?}"));
        }
    }

    // --- The guards are still wired into `rewrite_linear_units` -----------

    /// A [`GltfScaleSource`] built from `value` past the preflight gate.
    fn past_the_gate(name: &str, value: Value) -> crate::GltfScaleSource {
        let bytes = serde_json::to_vec(&value).expect("literal JSON serializes");
        crate::capability::scale_source_past_the_gate(std::path::Path::new(name), &bytes)
            .unwrap_or_else(|error| panic!("{name} must still load past the gate: {error:?}"))
    }

    /// One 96-byte buffer, a `POSITION` accessor on `bufferView 1`, and one
    /// image on `bufferView 2` at a caller-chosen range.
    ///
    /// The image is deliberately **not** on `bufferView 0`: a guard reading
    /// the first view instead of the indexed one would answer from the unused
    /// decoy view at index 0, which shares no byte with `POSITION`.
    fn image_and_position_document(image_offset: usize, image_length: usize) -> Value {
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        serde_json::json!({
            "asset": { "version": "2.0" },
            "buffers": [{
                "uri": format!(
                    "data:application/octet-stream;base64,{}",
                    STANDARD.encode([0u8; 96])
                ),
                "byteLength": 96
            }],
            "bufferViews": [
                { "buffer": 0, "byteOffset": 48, "byteLength": 12 },
                { "buffer": 0, "byteOffset": 0, "byteLength": 36 },
                { "buffer": 0, "byteOffset": image_offset, "byteLength": image_length }
            ],
            "accessors": [{
                "bufferView": 1, "componentType": 5126, "count": 3, "type": "VEC3",
                "min": [0, 0, 0], "max": [0, 0, 0]
            }],
            "images": [{ "bufferView": 2, "mimeType": "image/png" }],
            "meshes": [{ "primitives": [{ "attributes": { "POSITION": 0 } }] }]
        })
    }

    #[test]
    fn rewrite_linear_units_still_calls_the_node_transform_guard() {
        // Deleting `reject_out_of_contract_nodes(root)?` from
        // `rewrite_linear_units` must fail a test. It cannot fail one through
        // the public API, because the gate refuses every source that would
        // reach it; the seam supplies the relaxation the guard exists for.
        let mut node = serde_json::json!({ "matrix": Vec::from(IDENTITY_MATRIX) });
        node["translation"] = serde_json::json!([1.5, -2.0, 0.25]);
        let source = past_the_gate(
            "matrix-plus-trs.gltf",
            serde_json::json!({ "asset": { "version": "2.0" }, "nodes": [node] }),
        );
        match rewrite_linear_units(&source, 4.0) {
            Err(GltfScaleRewriteError::ConflictingNodeTransform { location }) => {
                assert_eq!(location, "/nodes/0/translation");
            }
            other => panic!("the wired guard must refuse matrix + translation, got {other:?}"),
        }

        // The projective arm is reached through the same call site, and
        // without it `U M U^-1` would silently emit an unconverted last row.
        let mut matrix = IDENTITY_MATRIX;
        matrix[15] = 2.0;
        let source = past_the_gate(
            "projective-matrix.gltf",
            serde_json::json!({
                "asset": { "version": "2.0" },
                "nodes": [{ "matrix": Vec::from(matrix) }]
            }),
        );
        match rewrite_linear_units(&source, 4.0) {
            Err(GltfScaleRewriteError::NonAffineNodeMatrix {
                location,
                value,
                expected,
            }) => {
                assert_eq!(location, "/nodes/0/matrix/15");
                assert_eq!(value, 2.0);
                assert_eq!(expected, 1.0);
            }
            other => panic!("the wired guard must refuse a projective matrix, got {other:?}"),
        }
    }

    #[test]
    fn rewrite_linear_units_still_calls_the_image_payload_guard() {
        // Deleting `reject_image_payload_overlap(root, manifest, &spans)?`
        // must fail a test. Without it the conversion runs to completion and
        // writes converted `f32`s over bytes the image reads.
        let source = past_the_gate(
            "image-overlap.gltf",
            // `POSITION` is [0, 36); the image view is [12, 24).
            image_and_position_document(12, 12),
        );
        match rewrite_linear_units(&source, 2.0) {
            Err(GltfScaleRewriteError::ImagePayloadOverlap {
                location,
                accessor_index,
            }) => {
                assert_eq!(location, "/images/0/bufferView");
                assert_eq!(accessor_index, 0);
            }
            other => panic!("the wired guard must refuse an aliased image, got {other:?}"),
        }
    }

    #[test]
    fn the_wired_image_guard_still_accepts_a_disjoint_image_view() {
        // The seam must not make every source refusable: the same document
        // with the image moved clear of `POSITION` converts. This is what
        // keeps the two tests above from passing for the wrong reason.
        let source = past_the_gate(
            "image-disjoint.gltf",
            // `POSITION` is [0, 36); the image view is [36, 48).
            image_and_position_document(36, 12),
        );
        rewrite_linear_units(&source, 2.0)
            .expect("an image view disjoint from every converted span converts");
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
        manifest.morph_weight_locations = vec!["/animations/0/channels/0/target/path".to_owned()];
        let facts = capability_facts(&manifest);
        assert!(facts.morph_weights_present);
        assert!(!facts.is_supported());
    }

    #[test]
    fn an_animated_matrix_node_is_rederived_from_manifest_identity() {
        use crate::capability::{
            GltfAnimationChannelCapability, GltfNodeCapability, GltfNodeRestKind,
        };
        let mut manifest = manifest();
        manifest.nodes = vec![GltfNodeCapability {
            node_index: 9,
            rest_kind: GltfNodeRestKind::Matrix,
            mesh_index: None,
            skin_index: None,
        }];
        manifest.animation_channels = vec![GltfAnimationChannelCapability {
            animation_index: 3,
            channel_index: 4,
            target_node_index: 9,
            target_path: "scale".to_owned(),
            interpolation: "STEP".to_owned(),
            input_accessor_index: 5,
            output_accessor_index: 6,
        }];
        assert_eq!(
            manifest_violations(&manifest),
            vec![GltfCapabilityViolation {
                kind: GltfCapabilityViolationKind::AnimatedMatrixNode,
                location: "/animations/3/channels/4/target".to_owned(),
            }]
        );
        let facts = capability_facts(&manifest);
        assert!(facts.unknown_source_members_present);
        assert!(!facts.is_supported());
    }
}
