//! Format-neutral scale plan and proof contracts (DESIGN.md Appendix D).
//!
//! This module owns the two distinct scale operations Appendix D defines —
//! [`ScaleOperation::WholeDocumentLinearUnits`] and
//! [`ScaleOperation::RestBindUniformScale`] — plus their shared pure
//! planning, candidate construction, and proof layer. It deliberately
//! consumes and returns only format-neutral facts: an already-loaded
//! [`Document`] and a [`ScaleCapabilityFacts`] projection that a format
//! frontend (for example `animsmith-gltf`'s raw capability preflight)
//! builds from its own source-specific inventory. This module does not
//! accept paths, glTF/ufbx types, config parsers, or publication policy,
//! and it does not itself decide CLI selectors, evidence schemas, or
//! artifact/evidence publication — those are producer concerns layered on
//! top.
//!
//! [`ScaleOperation::RestBindUniformScale`] selects by raw, format-neutral
//! source identity — `source_skin_index` and `source_root_node_index` — not
//! by normalized [`crate::model::BoneId`] or mesh-instance ordinal.
//! Resolving those selectors, and classifying the affected domain's affine
//! shape, walks [`crate::model::SceneAssets::source_skeleton`]: the only
//! place a full (possibly sheared) authored local matrix survives, since
//! [`crate::model::Bone::rest`] is a lossy TRS decomposition that can never
//! look sheared even when the source was.
//!
//! [`plan_scale`] is pure and fail-closed: it never mutates its input and
//! returns a typed [`ScaleError`] for every unsupported affine domain,
//! incomplete closure, incomplete capability, invalid selector, invalid
//! factor, or affected scale-animation track. [`build_scale_candidate`]
//! builds a new [`ScaleCandidate`] document from an accepted [`ScalePlan`];
//! because it only ever reads its `&Document` input, a failure cannot leave
//! the caller's source document mutated — the half-built candidate is
//! simply dropped. [`prove_scale`] independently re-derives the plan's
//! claims from the source and candidate documents and reports the observed
//! residual maxima against the fixed [`ScaleTolerancePolicy::APPENDIX_D_V1`]
//! tolerance identity.

use crate::model::{
    BoneId, Clip, Document, Interpolation, MeshInstance, Property, Skeleton,
    SourceInverseBindAccessorStatus, SourceNodeAsset, SourceNodeLocalRest, SourceSkeletonCoverage,
    SourceSkinAsset, Track, TrackValues, Transform, mat4_is_finite, world_rest_matrices,
};
use crate::sample::{TrackSample, sample_track};
use glam::{Mat3, Mat4, Quat, Vec3, Vec4};
use std::collections::{BTreeMap, BTreeSet};

// --- Tolerance policy ----------------------------------------------------

/// Fixed Appendix D tolerance identity and thresholds. Classification and
/// proof share this one versioned policy and compute in `f64`, narrowing
/// only at the writer model boundary. There is exactly one supported
/// instance, [`ScaleTolerancePolicy::APPENDIX_D_V1`]: a policy change is a
/// new policy identity, not a runtime knob.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct ScaleTolerancePolicy {
    /// Stable policy identity recorded in producer evidence.
    pub id: &'static str,
    /// Relative orthogonality tolerance for rejecting shear.
    pub relative_orthogonality: f64,
    /// Relative tolerance for equal-length affine columns (uniform scale).
    pub equal_axis: f64,
    /// Relative tolerance for one common factor across an affected domain.
    pub common_factor: f64,
    /// `abs(det) <= singular_determinant_relative * product(axis_lengths)`
    /// classifies a linear part as singular.
    pub singular_determinant_relative: f64,
    /// Absolute term of the scalar/vector comparison tolerance.
    pub scalar_absolute: f64,
    /// Relative term of the scalar/vector comparison tolerance.
    pub scalar_relative: f64,
    /// Maximum shortest-path rotation residual, in radians.
    pub rotation_residual_radians: f64,
    /// Maximum postcondition unit-scale residual.
    pub postcondition_unit_scale_residual: f64,
}

impl ScaleTolerancePolicy {
    /// The only supported tolerance policy: DESIGN.md Appendix D, version 1.
    pub const APPENDIX_D_V1: Self = Self {
        id: "appendix-d-v1",
        relative_orthogonality: 1e-5,
        equal_axis: 1e-5,
        common_factor: 1e-5,
        singular_determinant_relative: 1e-6,
        scalar_absolute: 1e-6,
        scalar_relative: 1e-5,
        rotation_residual_radians: 1e-5,
        postcondition_unit_scale_residual: 1e-5,
    };

    /// `abs_error <= scalar_absolute + scalar_relative * max(abs(before), abs(after))`.
    ///
    /// Every proof call site must pass the actual before/after magnitudes of
    /// the specific residual being checked — never a proxy such as the
    /// plan's declared factor — so a residual near a large coordinate gets a
    /// correspondingly looser absolute tolerance than one near a small
    /// coordinate.
    pub fn scalar_tolerance(&self, before: f64, after: f64) -> f64 {
        self.scalar_absolute + self.scalar_relative * before.abs().max(after.abs())
    }

    /// `abs(a - b) <= tolerance * max(abs(a), abs(b))`.
    ///
    /// Genuinely relative, per DESIGN.md Appendix D §D.1: the orthogonality,
    /// equal-axis, and common-factor tolerances are declared *relative*
    /// `1e-5`, so the comparison base is the operands' own magnitude and
    /// nothing else. Flooring that base at `1.0` — as an earlier revision did
    /// — silently converts these into absolute tolerances for every operand
    /// below unit magnitude, which is exactly the regime these operations
    /// exist for: at a common factor of `0.01` a `1.0` floor accepts `1e-3`
    /// relative error, `100x` the declared policy, and lets `plan_scale`
    /// accept a plan whose candidate then fails its own unit-scale
    /// postcondition.
    ///
    /// The only floor is [`Self::scalar_absolute`], which exists solely so
    /// the degenerate `a == b == 0` case compares equal rather than dividing
    /// a zero tolerance into a zero difference.
    fn relative(&self, tolerance: f64, a: f64, b: f64) -> bool {
        (a - b).abs() <= tolerance * a.abs().max(b.abs()).max(self.scalar_absolute)
    }
}

// --- Capability projection ------------------------------------------------

/// Whether a format-neutral capability projection covers the whole source.
///
/// A projection built from an incomplete or partially-inspected source must
/// report [`ScaleCapabilityCoverage::Unavailable`]: an absent flag is not
/// evidence the underlying domain is absent from the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScaleCapabilityCoverage {
    /// The projection cannot vouch for the complete source domain.
    #[default]
    Unavailable,
    /// Every documented domain in the source was inspected.
    Complete,
}

/// Format-neutral capability facts a frontend projects from its raw source
/// inventory before any scale plan or candidate exists.
///
/// This is deliberately coarser than a format's own raw capability
/// manifest (for example `animsmith_gltf::GltfCapabilityManifest`): it only
/// carries the flags this module's planning needs to fail closed on an
/// unsupported domain, per DESIGN.md Appendix D §D.4. A frontend projects
/// its richer, format-specific manifest down to these flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct ScaleCapabilityFacts {
    /// Whether the projection covers the complete source domain.
    pub coverage: ScaleCapabilityCoverage,
    /// A morph target is present.
    pub morphs_present: bool,
    /// Static or animated morph weights are present.
    pub morph_weights_present: bool,
    /// A camera is present.
    pub cameras_present: bool,
    /// A punctual light is present.
    pub lights_present: bool,
    /// GPU-instancing data is present.
    pub instancing_present: bool,
    /// An extension is not covered by a registered length-field handler.
    pub unregistered_extensions_present: bool,
    /// Non-null application-specific extras are present.
    pub extras_present: bool,
    /// A JSON/source member outside the modeled schema was ignored.
    pub unknown_source_members_present: bool,
    /// A non-triangle-list primitive is present.
    pub non_triangle_primitives_present: bool,
    /// A vertex attribute outside the normalized writer subset is present.
    pub unsupported_vertex_attributes_present: bool,
    /// A secondary skin-influence set is present.
    pub secondary_skin_influences_present: bool,
    /// An inverse-bind accessor is missing, empty, mismatched, or unreadable.
    pub inverse_bind_issues_present: bool,
    /// A scale-bearing source layout cannot be safely bounded or rewritten.
    pub unsafe_accessor_layout_present: bool,
    /// An external (non-embedded) resource is referenced.
    pub external_resources_present: bool,
}

impl ScaleCapabilityFacts {
    /// A capability projection declaring complete coverage and no
    /// unsupported domain — the only facts planning accepts.
    pub fn is_supported(&self) -> bool {
        self.coverage == ScaleCapabilityCoverage::Complete
            && !self.morphs_present
            && !self.morph_weights_present
            && !self.cameras_present
            && !self.lights_present
            && !self.instancing_present
            && !self.unregistered_extensions_present
            && !self.extras_present
            && !self.unknown_source_members_present
            && !self.non_triangle_primitives_present
            && !self.unsupported_vertex_attributes_present
            && !self.secondary_skin_influences_present
            && !self.inverse_bind_issues_present
            && !self.unsafe_accessor_layout_present
            && !self.external_resources_present
    }
}

// --- Operation and request -------------------------------------------------

/// The two distinct scale operations DESIGN.md Appendix D §D.1 defines.
///
/// Neither variant infers its factor or applicability from mesh bounds,
/// character height, joint lengths, inverse-bind magnitude, filename, or an
/// asset category. The caller names the operation and declares or accepts
/// the exact factor [`plan_scale`] validates.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum ScaleOperation {
    /// Whole-document linear-unit conversion: every represented length is
    /// converted by the declared finite positive `factor`.
    WholeDocumentLinearUnits {
        /// Declared finite positive conversion factor `q`.
        factor: f64,
    },
    /// Rest/bind hierarchy reparameterization: removes one compensating
    /// inherited scale from a restricted skinned hierarchy.
    ///
    /// Both selectors are raw, format-neutral source identity — a source
    /// node/skin array index, per DESIGN.md Appendix D §D.7 — not a
    /// normalized [`BoneId`] or mesh-instance ordinal. [`plan_scale`]
    /// resolves them through [`crate::model::SceneAssets::source_skeleton`].
    RestBindUniformScale {
        /// Stable source-skin-array index selecting the skin whose joints
        /// anchor the affected domain.
        source_skin_index: usize,
        /// Stable source-node-array index of the scaled ancestor root.
        source_root_node_index: usize,
        /// Caller-declared expected common factor `s`. Planning measures
        /// the source's observed rest-world factor and rejects a mismatch
        /// rather than inferring `s` from geometry.
        expected_factor: f64,
    },
}

/// Pure planning input: the operation, the document to plan against, and a
/// format-neutral capability projection of the raw source.
#[derive(Debug, Clone, Copy)]
pub struct ScaleRequest<'a> {
    /// Selected operation and its declared parameters.
    pub operation: ScaleOperation,
    /// Document to plan against.
    pub document: &'a Document,
    /// Format-neutral capability projection of the raw source.
    pub capability: &'a ScaleCapabilityFacts,
}

// --- Errors ----------------------------------------------------------------

/// Stable machine-readable reason an affine linear part failed
/// classification against the fixed tolerance policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AffineDomainViolation {
    /// The linear part's axis lengths are not equal (non-uniform scale).
    NonUniformScale,
    /// The linear part's axes are not mutually orthogonal (shear).
    Sheared,
    /// The linear part has a negative determinant (reflection).
    Reflected,
    /// The linear part is singular or near-singular.
    Singular,
    /// The linear part contains a non-finite component.
    NonFinite,
}

/// Typed, fail-closed rejection from [`plan_scale`], [`build_scale_candidate`],
/// or [`prove_scale`].
#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum ScaleError {
    /// The whole-document conversion factor is not finite and positive.
    #[error("scale factor must be finite and positive, got {factor}")]
    InvalidFactor {
        /// The rejected factor.
        factor: f64,
    },
    /// The declared rest/bind expected factor is not finite and positive.
    #[error("rest/bind expected factor must be finite and positive, got {factor}")]
    InvalidExpectedFactor {
        /// The rejected factor.
        factor: f64,
    },
    /// A declared factor (or a factor derived from it, such as the rest/bind
    /// reciprocal `1 / expected_factor`) is finite and positive in `f64` but
    /// has no usable `f32` image at the writer model boundary: it either
    /// overflows to infinity or flushes a nonzero factor to zero.
    ///
    /// This is deliberately a distinct variant from
    /// [`ScaleError::InvalidFactor`] / [`ScaleError::InvalidExpectedFactor`],
    /// whose message would be an outright lie here — `1e-50` *is* finite and
    /// positive; what it is not is representable once the model narrows to
    /// `f32`. Rejecting it at plan time is what stops a build from silently
    /// multiplying every translation, mesh `POSITION`, and inverse-bind
    /// translation by `0.0f32` and handing the annihilated document to a
    /// proof that then signs off on it, because `0 == 0 * 0` within any
    /// tolerance.
    #[error(
        "factor {factor} (derived from declared factor {declared}) is not representable at the f32 writer model boundary: it narrows to {narrowed}"
    )]
    FactorNotRepresentable {
        /// The declared factor the caller supplied.
        declared: f64,
        /// The declared factor, or the reciprocal derived from it, that
        /// failed to narrow. Equal to `declared` when the declared factor
        /// itself failed.
        factor: f64,
        /// The unusable `f32` image of `factor`.
        narrowed: f32,
    },
    /// `source_root_node_index` is not a source node in the document's
    /// source skeleton.
    #[error(
        "source root node index {source_root_node_index} is not a source node in the document's source skeleton"
    )]
    InvalidRootSelector {
        /// The rejected source-node index.
        source_root_node_index: usize,
    },
    /// `source_skin_index` is not a skin in the document's source skeleton,
    /// or the skin declares no joints.
    #[error(
        "source skin index {source_skin_index} is not a skin in the document's source skeleton, or has no joints"
    )]
    InvalidSkinSelector {
        /// The rejected source-skin index.
        source_skin_index: usize,
    },
    /// The capability projection is unavailable or declares an unsupported
    /// domain.
    #[error("capability projection is incomplete or declares unsupported domain(s)")]
    IncompleteCapability,
    /// `document.assets.source_skeleton` does not declare complete coverage.
    #[error(
        "document.assets.source_skeleton coverage is not complete: rest/bind planning requires a format-neutral source-node/source-skin projection"
    )]
    IncompleteSourceSkeleton,
    /// A source node in the affected closure did not normalize to a
    /// document skeleton bone.
    #[error("source node {source_node_index} did not normalize to a document skeleton bone")]
    SourceNodeNotNormalized {
        /// The unnormalized source-node index.
        source_node_index: usize,
    },
    /// A raw source-node rest transform is non-finite.
    #[error("source node {source_node_index} has a non-finite raw rest transform")]
    NonFiniteSourceTransform {
        /// The source node with the non-finite transform.
        source_node_index: usize,
    },
    /// A skeleton rest transform or accumulated world matrix is non-finite.
    #[error("node {node} has a non-finite rest transform")]
    NonFiniteTransform {
        /// The node with the non-finite transform.
        node: BoneId,
    },
    /// The skeleton is not in parent-before-child order.
    #[error("node {node} has invalid parent {parent}")]
    InvalidParent {
        /// The node with an invalid parent.
        node: BoneId,
        /// The invalid parent index.
        parent: BoneId,
    },
    /// A plan or document reference a bone index outside
    /// `document.skeleton.bones` for the document actually supplied.
    ///
    /// This guards every boundary where a [`ScalePlan`] built from one
    /// document could be replayed against a different one: [`ScalePlan`]
    /// has no public constructor other than [`plan_scale`], but
    /// [`build_scale_candidate`] and [`prove_scale`] each take the document
    /// to operate on as a separate argument and must not trust that it
    /// still matches the plan's shape.
    #[error("bone index {index} is out of range for this document")]
    BoneIndexOutOfRange {
        /// The out-of-range index.
        index: usize,
    },
    /// The affected closure could not be completed.
    #[error("affected domain closure is not complete: {reason}")]
    IncompleteClosure {
        /// Stable machine-readable reason.
        reason: &'static str,
    },
    /// Unskinned geometry is attached inside the affected closure.
    #[error("node {node} carries unskinned geometry inside the affected closure")]
    UnsupportedUnskinnedGeometry {
        /// The node carrying unskinned geometry.
        node: BoneId,
    },
    /// A node's rest-world linear part is outside the supported affine class.
    #[error(
        "node {node} rest-world linear part is not orientation-preserving positive uniform scale ({reason:?})"
    )]
    InvalidAffineDomain {
        /// The rejected node.
        node: BoneId,
        /// Stable machine-readable violation kind.
        reason: AffineDomainViolation,
    },
    /// The declared `expected_factor` does not match the source's observed
    /// common factor.
    #[error("declared expected factor {expected} does not match observed source factor {observed}")]
    FactorMismatch {
        /// Declared expected factor.
        expected: f64,
        /// Observed source factor.
        observed: f64,
    },
    /// One node's effective factor differs from the domain's common factor.
    #[error("node {node} effective factor {observed} differs from common factor {expected}")]
    MixedFactor {
        /// The domain's common factor.
        expected: f64,
        /// The node's observed factor.
        observed: f64,
        /// The node with the mismatched factor.
        node: BoneId,
    },
    /// A scale-animation track targets an affected node.
    #[error("clip {clip_index} animates scale on affected node {node}")]
    AffectedScaleAnimation {
        /// Index into `document.clips` of the offending clip.
        clip_index: usize,
        /// Affected node targeted by the scale track.
        node: BoneId,
    },
    /// A proof residual exceeded the fixed tolerance policy.
    #[error("proof residual {observed} for {kind:?} exceeds tolerance {tolerance}")]
    ProofResidualExceeded {
        /// Which proof obligation failed.
        kind: ProofResidualKind,
        /// Observed residual.
        observed: f64,
        /// Tolerance the residual exceeded.
        tolerance: f64,
    },
    /// [`ScalePlan::proof_obligations`] declared a claim provable, but
    /// [`prove_scale`] could not find the evidence to check it (for example
    /// a clip or track present in `source` with no counterpart in
    /// `candidate`). This is a distinct failure from
    /// [`ScaleError::ProofResidualExceeded`]: the claim was never checked at
    /// all, so proof must fail rather than silently report a zero residual.
    #[error("proof obligation {kind:?} could not find expected evidence ({detail})")]
    MissingProofEvidence {
        /// Which proof obligation was left unchecked.
        kind: ProofResidualKind,
        /// Stable machine-readable reason.
        detail: &'static str,
    },
    /// `document.assets.source_skeleton.nodes` declares the same
    /// `source_node_index` more than once. This projection is never
    /// deduplicated by last-write-wins or first-match: a duplicate is a
    /// malformed source and must reject.
    #[error("source skeleton declares duplicate source node index {source_node_index}")]
    DuplicateSourceNodeIndex {
        /// The duplicated source-node index.
        source_node_index: usize,
    },
    /// `document.assets.source_skeleton.skins` declares the same
    /// `source_skin_index` more than once.
    #[error("source skeleton declares duplicate source skin index {source_skin_index}")]
    DuplicateSourceSkinIndex {
        /// The duplicated source-skin index.
        source_skin_index: usize,
    },
    /// One clip declares two tracks for the same `(bone, property)` pair.
    /// Every clip-track lookup in this module pairs source and candidate
    /// tracks by index, which is only sound when track identity within a
    /// clip is unique — so a duplicate is rejected rather than silently
    /// paired with the first (or last) match.
    #[error("clip {clip_index} declares duplicate {property:?} tracks for node {node}")]
    DuplicateClipTrack {
        /// Index into `document.clips` of the offending clip.
        clip_index: usize,
        /// The duplicated target node.
        node: BoneId,
        /// The duplicated property.
        property: Property,
    },
    /// A track's shape is malformed: an out-of-range bone, empty or
    /// non-finite keyframe times, a value count that disagrees with
    /// `times.len()` and `interpolation`, a `TrackValues` variant that
    /// disagrees with `property`, or a non-finite value.
    #[error("clip {clip_index} track for node {node} has an invalid shape ({reason})")]
    InvalidTrackShape {
        /// Index into `document.clips` of the offending clip.
        clip_index: usize,
        /// The track's target node.
        node: BoneId,
        /// Stable machine-readable reason.
        reason: &'static str,
    },
    /// A mesh instance is malformed: an out-of-range `mesh` or `skin_joints`
    /// entry, a non-empty `skin_ibms` whose length disagrees with
    /// `skin_joints`, or a non-finite inverse-bind matrix.
    #[error("mesh instance {instance_index} is invalid ({reason})")]
    InvalidMeshInstance {
        /// Index into `document.assets.instances` of the offending instance.
        instance_index: usize,
        /// Stable machine-readable reason.
        reason: &'static str,
    },
    /// No inverse-bind evidence exists for a skin joint: the owning mesh
    /// instance declares an empty `skin_ibms` (falling back to the bone's
    /// own [`crate::model::Bone::inverse_bind`]) and that bone also has no
    /// inverse-bind matrix. Identity is never substituted for genuinely
    /// missing evidence — only for a source skin whose complete-coverage
    /// [`crate::model::SourceSkinAsset::inverse_bind_accessor`] proves the
    /// format-defined identity default with
    /// [`crate::model::SourceInverseBindAccessorStatus::Absent`] (checked
    /// internally by this module's private inverse-bind resolution).
    #[error("no inverse-bind evidence for skin joint {node}")]
    MissingInverseBind {
        /// The joint with no inverse-bind evidence.
        node: BoneId,
    },
    /// A mesh primitive is malformed independently of any skin: a non-finite
    /// base `POSITION`.
    ///
    /// Checked at every public entry point, on the candidate as well as the
    /// input, because base `POSITION` is a rewritten domain: without it a
    /// whole-document build with an overflowing factor returns a document
    /// full of non-finite vertices as `Ok`.
    #[error("mesh {mesh_index} primitive {primitive_index} is invalid ({reason})")]
    InvalidMeshPrimitive {
        /// Index into `document.assets.meshes` of the offending mesh.
        mesh_index: usize,
        /// Index into that mesh's `primitives` of the offending primitive.
        primitive_index: usize,
        /// Stable machine-readable reason.
        reason: &'static str,
    },
    /// A skinned primitive is malformed: `joints`/`weights` shorter than
    /// `positions`, a non-finite position or weight, a joint-influence slot
    /// outside the owning instance's `skin_joints`, or a non-finite skinned
    /// result.
    #[error("instance {instance_index} primitive {primitive_index} is invalid ({reason})")]
    InvalidSkinnedPrimitive {
        /// Index into `document.assets.instances` of the owning instance.
        instance_index: usize,
        /// Index into the owning mesh's `primitives` of the offending
        /// primitive.
        primitive_index: usize,
        /// Stable machine-readable reason.
        reason: &'static str,
    },
    /// `candidate`'s clip/track/instance/mesh/primitive structure does not
    /// match `source`'s: a missing or extra clip, track, instance, mesh, or
    /// primitive, or a track whose identity, interpolation, times, or value
    /// shape disagrees with its source counterpart. Proof pairs source and
    /// candidate structure by index, which requires this parity to hold —
    /// an extra or missing structure is never silently ignored.
    #[error("candidate document structure does not match source ({reason})")]
    CandidateStructureMismatch {
        /// Stable machine-readable reason.
        reason: &'static str,
    },
}

/// Which proof obligation produced a [`ScaleError::ProofResidualExceeded`]
/// or [`ScaleError::MissingProofEvidence`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProofResidualKind {
    /// Rest-world translation residual.
    RestTranslation,
    /// Rest-world rotation residual.
    RestRotation,
    /// Postcondition unit-scale residual.
    UnitScale,
    /// Transform-only attachment full-affine residual (an off-origin point
    /// transformed through the expected and actual world matrix), per
    /// DESIGN.md Appendix D §D.2/§D.6.
    TransformOnlyAffine,
    /// Per-element animation-track value residual, checked directly against
    /// each domain's analytic expectation: a rewritten translation element
    /// (value *or* cubic tangent) against `before * multiplier`, and every
    /// retained rotation/scale element against `before` itself.
    ///
    /// Distinct from [`Self::KeyTranslation`], which samples the *composed*
    /// track at key times: sampling proves what an evaluator would read, but
    /// only a direct element comparison proves that the domains this plan
    /// declares untouched really are untouched.
    TrackValue,
    /// Base mesh `POSITION` residual, per vertex, against this operation's
    /// analytic expectation (`before * q` for whole-document conversion,
    /// `before` for rest/bind reparameterization).
    MeshPosition,
    /// Keyframe-time translation residual.
    KeyTranslation,
    /// Cubic-segment interior-time translation residual.
    CubicInterior,
    /// Sampled world-space trajectory residual.
    Trajectory,
    /// Skin-matrix (`W * B`) residual.
    SkinMatrix,
    /// Skinned mesh bounds residual.
    Bounds,
}

// --- Plan --------------------------------------------------------------

/// Which model domains one [`ScalePlan`] rewrites, matching the DESIGN.md
/// Appendix D §D.4 domain table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ScaleDomainRewrites {
    /// Node-local rest translations (and, for rest/bind, local scales) are
    /// rewritten.
    pub rest_hierarchy: bool,
    /// Translation animation values and cubic tangents are rewritten.
    pub translation_animation: bool,
    /// Per-bone and per-instance inverse bind matrices are rewritten.
    pub inverse_binds: bool,
    /// Base mesh `POSITION` values are rewritten.
    pub base_mesh_positions: bool,
}

/// Which claims [`prove_scale`] must independently verify for one plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ScaleProofObligations {
    /// Prove rest-world translation/orientation facts for affected nodes.
    pub prove_rest: bool,
    /// Prove unit composed scale at every affected node (rest/bind only).
    pub prove_unit_scale_postcondition: bool,
    /// Prove the complete expected world affine of every transform-only
    /// attachment via an off-origin point, so a translation/rotation-only
    /// check cannot mistake a no-op for a correct rebase.
    pub prove_transform_only_affine: bool,
    /// Prove every keyframe time of an affected translation track.
    pub prove_keys: bool,
    /// Prove bounded interior times of cubic-spline translation segments.
    pub prove_cubic_interiors: bool,
    /// Prove sampled world-space trajectories.
    pub prove_trajectories: bool,
    /// Prove the skin equation `W_i(t) * B_i` for affected skins, at rest
    /// and at every declared key/cubic-interior sample time.
    pub prove_skin: bool,
    /// Prove skinned mesh bounds, at rest and at every declared
    /// key/cubic-interior sample time.
    ///
    /// Declared only when the planned document actually carries a skinned
    /// mesh instance with a joint inside the affected closure — an
    /// obligation this module cannot check is never asserted. Once declared
    /// it is binding: proving against a document with no such instance is
    /// [`ScaleError::MissingProofEvidence`], not a silent zero residual.
    ///
    /// Base `POSITION` preservation does not depend on this flag; it is
    /// proved per primitive regardless (see
    /// [`ProofResidualKind::MeshPosition`]).
    pub prove_bounds: bool,
}

/// Pure, typed plan returned by [`plan_scale`].
///
/// Planning never mutates its input document; it only inspects it. Building
/// a candidate from an accepted plan is a distinct, separately fallible step
/// ([`build_scale_candidate`]).
///
/// Every field is private: a [`ScalePlan`] can only be produced by
/// [`plan_scale`], so an external caller cannot hand-construct or mutate one
/// into a state whose `affected_nodes` disagree with `operation`'s
/// selectors. Read plan contents through the accessor methods.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct ScalePlan {
    operation: ScaleOperation,
    tolerance_policy: ScaleTolerancePolicy,
    affected_nodes: Vec<BoneId>,
    transform_only_attachments: Vec<BoneId>,
    common_factor: f64,
    domain_rewrites: ScaleDomainRewrites,
    proof_obligations: ScaleProofObligations,
}

impl ScalePlan {
    /// Echoed operation and its declared parameters.
    pub fn operation(&self) -> ScaleOperation {
        self.operation
    }

    /// The fixed tolerance policy this plan and its proof share.
    pub fn tolerance_policy(&self) -> ScaleTolerancePolicy {
        self.tolerance_policy
    }

    /// Affected node closure, in ascending bone-id order.
    ///
    /// For [`ScaleOperation::WholeDocumentLinearUnits`] this is every node
    /// in the document. For [`ScaleOperation::RestBindUniformScale`] this is
    /// the closed connected hierarchy of DESIGN.md Appendix D §D.2: the
    /// scaled ancestor, every selected skin joint and the paths between
    /// them, and every descendant transform-only attachment.
    pub fn affected_nodes(&self) -> &[BoneId] {
        &self.affected_nodes
    }

    /// Descendant nodes in [`Self::affected_nodes`] that carry no skin —
    /// the "transform-only child" case of DESIGN.md Appendix D §D.2/§D.3.
    /// Always empty for [`ScaleOperation::WholeDocumentLinearUnits`].
    pub fn transform_only_attachments(&self) -> &[BoneId] {
        &self.transform_only_attachments
    }

    /// The one common factor `s` (or `q` for whole-document conversion)
    /// applied across [`Self::affected_nodes`].
    pub fn common_factor(&self) -> f64 {
        self.common_factor
    }

    /// Which model domains this plan rewrites.
    pub fn domain_rewrites(&self) -> ScaleDomainRewrites {
        self.domain_rewrites
    }

    /// Which claims proof must independently verify.
    pub fn proof_obligations(&self) -> ScaleProofObligations {
        self.proof_obligations
    }

    fn affected_set(&self) -> BTreeSet<BoneId> {
        self.affected_nodes.iter().copied().collect()
    }

    fn is_whole_document(&self) -> bool {
        matches!(
            self.operation,
            ScaleOperation::WholeDocumentLinearUnits { .. }
        )
    }
}

/// Plan the caller-selected [`ScaleOperation`] against `request.document`.
///
/// Pure and fail-closed: this function only reads `request.document` and
/// `request.capability`. It never returns a plan for a factor that was not
/// declared or an affine domain outside the initial supported class.
///
/// # Errors
///
/// Returns a typed [`ScaleError`] for every unsupported factor, selector,
/// capability gap, affine domain, closure incompleteness, factor mismatch,
/// or affected scale-animation track.
pub fn plan_scale(request: &ScaleRequest<'_>) -> Result<ScalePlan, ScaleError> {
    if !request.capability.is_supported() {
        return Err(ScaleError::IncompleteCapability);
    }
    validate_document_shape(request.document)?;
    match request.operation {
        ScaleOperation::WholeDocumentLinearUnits { factor } => {
            plan_whole_document(request.document, factor)
        }
        ScaleOperation::RestBindUniformScale {
            source_skin_index,
            source_root_node_index,
            expected_factor,
        } => plan_rest_bind(
            request.document,
            source_skin_index,
            source_root_node_index,
            expected_factor,
        ),
    }
}

/// Validate structural invariants every public entry point in this module
/// must trust before it reads or rewrites `document`: unique source-node and
/// source-skin identity, unique and well-shaped clip tracks, and in-range,
/// finite mesh-instance data. Called at the boundary of [`plan_scale`],
/// [`build_scale_candidate`], and [`prove_scale`] so a malformed public
/// [`Document`] fails closed with a typed [`ScaleError`] instead of being
/// silently deduplicated (last-write-wins), paired with the wrong structure,
/// or defaulted.
///
/// Per-vertex primitive skinning shape (`joints`/`weights` parallel to
/// `positions`) is deliberately not checked here: it is validated directly
/// where it is walked, in [`skinned_bounds`], since only the affected
/// instances' geometry needs inspecting there.
fn validate_document_shape(document: &Document) -> Result<(), ScaleError> {
    // Validated for its own sake, not for the returned matrices: this is
    // what catches a non-finite rest transform or a parent index that is
    // not strictly earlier than its child before any planning, building, or
    // sampling ever indexes into the skeleton.
    world_rests(&document.skeleton)?;
    validate_source_skeleton_identity(document)?;
    validate_clip_tracks(document)?;
    validate_scene_assets(document)?;
    Ok(())
}

/// Reject a `document.assets.source_skeleton` that declares the same
/// `source_node_index` or `source_skin_index` more than once, rather than
/// letting a later `BTreeMap`-keyed projection silently keep the last (or
/// first) duplicate.
fn validate_source_skeleton_identity(document: &Document) -> Result<(), ScaleError> {
    let mut seen_nodes = BTreeSet::new();
    for node in &document.assets.source_skeleton.nodes {
        if !seen_nodes.insert(node.source_node_index) {
            return Err(ScaleError::DuplicateSourceNodeIndex {
                source_node_index: node.source_node_index,
            });
        }
    }
    let mut seen_skins = BTreeSet::new();
    for skin in &document.assets.source_skeleton.skins {
        if !seen_skins.insert(skin.source_skin_index) {
            return Err(ScaleError::DuplicateSourceSkinIndex {
                source_skin_index: skin.source_skin_index,
            });
        }
    }
    Ok(())
}

/// Reject a clip that declares two tracks for the same `(bone, property)`,
/// an out-of-range track bone, or a track whose `times`/`values` shape is
/// malformed. Every proof pairing in this module matches source and
/// candidate tracks positionally, which is only sound once identity within
/// a clip is known to be unique.
fn validate_clip_tracks(document: &Document) -> Result<(), ScaleError> {
    let bone_count = document.skeleton.bones.len();
    for (clip_index, clip) in document.clips.iter().enumerate() {
        let mut seen: Vec<(BoneId, Property)> = Vec::with_capacity(clip.tracks.len());
        for track in &clip.tracks {
            if track.bone >= bone_count {
                return Err(ScaleError::InvalidTrackShape {
                    clip_index,
                    node: track.bone,
                    reason: "bone_index_out_of_range",
                });
            }
            let key = (track.bone, track.property);
            if seen.contains(&key) {
                return Err(ScaleError::DuplicateClipTrack {
                    clip_index,
                    node: track.bone,
                    property: track.property,
                });
            }
            seen.push(key);
            validate_track_value_shape(clip_index, track)?;
        }
    }
    Ok(())
}

fn validate_track_value_shape(clip_index: usize, track: &Track) -> Result<(), ScaleError> {
    if track.times.is_empty() {
        return Err(ScaleError::InvalidTrackShape {
            clip_index,
            node: track.bone,
            reason: "empty_times",
        });
    }
    if track.times.iter().any(|time| !time.is_finite()) {
        return Err(ScaleError::InvalidTrackShape {
            clip_index,
            node: track.bone,
            reason: "non_finite_time",
        });
    }
    if track.times.windows(2).any(|w| w[0] >= w[1]) {
        return Err(ScaleError::InvalidTrackShape {
            clip_index,
            node: track.bone,
            reason: "times_not_strictly_increasing",
        });
    }
    let expected_values = match track.interpolation {
        Interpolation::CubicSpline => track.times.len() * 3,
        Interpolation::Linear | Interpolation::Step => track.times.len(),
    };
    if track.values.len() != expected_values {
        return Err(ScaleError::InvalidTrackShape {
            clip_index,
            node: track.bone,
            reason: "value_count_mismatch",
        });
    }
    let variant_matches_property = matches!(
        (&track.values, track.property),
        (
            TrackValues::Vec3s(_),
            Property::Translation | Property::Scale
        ) | (TrackValues::Quats(_), Property::Rotation)
    );
    if !variant_matches_property {
        return Err(ScaleError::InvalidTrackShape {
            clip_index,
            node: track.bone,
            reason: "value_type_mismatches_property",
        });
    }
    let finite = match &track.values {
        TrackValues::Vec3s(values) => values.iter().all(|value| value.is_finite()),
        TrackValues::Quats(values) => values.iter().all(|value| value.is_finite()),
    };
    if !finite {
        return Err(ScaleError::InvalidTrackShape {
            clip_index,
            node: track.bone,
            reason: "non_finite_value",
        });
    }
    Ok(())
}

/// Reject a mesh primitive with a non-finite base `POSITION`, a mesh
/// instance with an out-of-range `mesh` or `skin_joints` entry, a non-empty
/// `skin_ibms` whose length disagrees with `skin_joints`, a non-finite
/// `skin_ibms` matrix, or a bone with a non-finite
/// [`crate::model::Bone::inverse_bind`].
fn validate_scene_assets(document: &Document) -> Result<(), ScaleError> {
    let bone_count = document.skeleton.bones.len();
    let mesh_count = document.assets.meshes.len();
    for (mesh_index, mesh) in document.assets.meshes.iter().enumerate() {
        for (primitive_index, primitive) in mesh.primitives.iter().enumerate() {
            if primitive
                .positions
                .iter()
                .any(|position| !position.is_finite())
            {
                return Err(ScaleError::InvalidMeshPrimitive {
                    mesh_index,
                    primitive_index,
                    reason: "non_finite_position",
                });
            }
        }
    }
    for (instance_index, instance) in document.assets.instances.iter().enumerate() {
        if instance.mesh >= mesh_count {
            return Err(ScaleError::InvalidMeshInstance {
                instance_index,
                reason: "mesh_index_out_of_range",
            });
        }
        if instance
            .skin_joints
            .iter()
            .any(|&joint| joint >= bone_count)
        {
            return Err(ScaleError::InvalidMeshInstance {
                instance_index,
                reason: "skin_joint_out_of_range",
            });
        }
        if !instance.skin_ibms.is_empty() && instance.skin_ibms.len() != instance.skin_joints.len()
        {
            return Err(ScaleError::InvalidMeshInstance {
                instance_index,
                reason: "skin_ibm_count_mismatch",
            });
        }
        if instance.skin_ibms.iter().any(|ibm| !mat4_is_finite(*ibm)) {
            return Err(ScaleError::InvalidMeshInstance {
                instance_index,
                reason: "non_finite_inverse_bind",
            });
        }
    }
    for (node, bone) in document.skeleton.bones.iter().enumerate() {
        if let Some(inverse_bind) = bone.inverse_bind
            && !mat4_is_finite(inverse_bind)
        {
            return Err(ScaleError::NonFiniteTransform { node });
        }
    }
    Ok(())
}

/// Reject `candidate`'s clip/track/instance/mesh/primitive structure when it
/// does not match `source`'s: every proof comparison in this module pairs
/// source and candidate structure positionally (by clip/track/instance/mesh
/// index), which is only sound once the two document shapes are known to
/// agree — an extra or missing structure is never silently ignored.
fn validate_candidate_structure(source: &Document, candidate: &Document) -> Result<(), ScaleError> {
    if source.clips.len() != candidate.clips.len() {
        return Err(ScaleError::CandidateStructureMismatch {
            reason: "clip_count_mismatch",
        });
    }
    for (source_clip, candidate_clip) in source.clips.iter().zip(candidate.clips.iter()) {
        if source_clip.tracks.len() != candidate_clip.tracks.len() {
            return Err(ScaleError::CandidateStructureMismatch {
                reason: "track_count_mismatch",
            });
        }
        for (source_track, candidate_track) in
            source_clip.tracks.iter().zip(candidate_clip.tracks.iter())
        {
            if source_track.bone != candidate_track.bone
                || source_track.property != candidate_track.property
                || source_track.interpolation != candidate_track.interpolation
                || source_track.times != candidate_track.times
                || source_track.values.len() != candidate_track.values.len()
            {
                return Err(ScaleError::CandidateStructureMismatch {
                    reason: "track_shape_mismatch",
                });
            }
        }
    }
    if source.assets.instances.len() != candidate.assets.instances.len() {
        return Err(ScaleError::CandidateStructureMismatch {
            reason: "instance_count_mismatch",
        });
    }
    if source.assets.meshes.len() != candidate.assets.meshes.len() {
        return Err(ScaleError::CandidateStructureMismatch {
            reason: "mesh_count_mismatch",
        });
    }
    for (source_mesh, candidate_mesh) in source
        .assets
        .meshes
        .iter()
        .zip(candidate.assets.meshes.iter())
    {
        if source_mesh.primitives.len() != candidate_mesh.primitives.len() {
            return Err(ScaleError::CandidateStructureMismatch {
                reason: "primitive_count_mismatch",
            });
        }
        for (source_primitive, candidate_primitive) in source_mesh
            .primitives
            .iter()
            .zip(candidate_mesh.primitives.iter())
        {
            if source_primitive.positions.len() != candidate_primitive.positions.len() {
                return Err(ScaleError::CandidateStructureMismatch {
                    reason: "primitive_vertex_count_mismatch",
                });
            }
        }
    }
    Ok(())
}

/// Reject an `f64` factor whose `f32` image cannot carry it.
///
/// Every rewrite in this module narrows to `f32` at the model boundary
/// (`Vec3`/`Mat4` are `f32`), so a factor that only exists in `f64` is not a
/// conversion this operation can perform. Both failure modes are silent
/// without this check: overflow to `±inf` produces a document full of
/// `NaN`/`inf` that only proof catches, and underflow of a nonzero factor to
/// `0.0f32` annihilates every length in the document while every proof
/// residual stays exactly zero.
///
/// Checked at *plan* time, for both the declared factor and the reciprocal
/// `1 / factor` the rest/bind basis correction derives from it, so no
/// unrepresentable factor ever reaches a builder.
fn check_factor_narrows(declared: f64, factor: f64) -> Result<f32, ScaleError> {
    let narrowed = factor as f32;
    if !narrowed.is_finite() || (narrowed == 0.0 && factor != 0.0) {
        return Err(ScaleError::FactorNotRepresentable {
            declared,
            factor,
            narrowed,
        });
    }
    Ok(narrowed)
}

fn plan_whole_document(document: &Document, factor: f64) -> Result<ScalePlan, ScaleError> {
    if !factor.is_finite() || factor <= 0.0 {
        return Err(ScaleError::InvalidFactor { factor });
    }
    check_factor_narrows(factor, factor)?;
    let affected_nodes: Vec<BoneId> = (0..document.skeleton.bones.len()).collect();
    let prove_bounds = has_skinned_evidence(document, &affected_nodes.iter().copied().collect());
    Ok(ScalePlan {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor },
        tolerance_policy: ScaleTolerancePolicy::APPENDIX_D_V1,
        affected_nodes,
        transform_only_attachments: Vec::new(),
        common_factor: factor,
        domain_rewrites: ScaleDomainRewrites {
            rest_hierarchy: true,
            translation_animation: true,
            inverse_binds: true,
            base_mesh_positions: true,
        },
        proof_obligations: ScaleProofObligations {
            prove_rest: true,
            prove_unit_scale_postcondition: false,
            prove_transform_only_affine: false,
            prove_keys: true,
            prove_cubic_interiors: true,
            prove_trajectories: true,
            prove_skin: true,
            prove_bounds,
        },
    })
}

fn plan_rest_bind(
    document: &Document,
    source_skin_index: usize,
    source_root_node_index: usize,
    expected_factor: f64,
) -> Result<ScalePlan, ScaleError> {
    if !expected_factor.is_finite() || expected_factor <= 0.0 {
        return Err(ScaleError::InvalidExpectedFactor {
            factor: expected_factor,
        });
    }
    // Both directions: the builder narrows `expected_factor` itself, and the
    // proof's basis correction `C = scale(1 / s)` narrows its reciprocal.
    check_factor_narrows(expected_factor, expected_factor)?;
    check_factor_narrows(expected_factor, 1.0 / expected_factor)?;
    if document.assets.source_skeleton.coverage != SourceSkeletonCoverage::Complete {
        return Err(ScaleError::IncompleteSourceSkeleton);
    }
    let skin = resolve_rest_bind_skin(document, source_skin_index)?;
    let by_source_index = source_node_index_map(document);
    if !by_source_index.contains_key(&source_root_node_index) {
        return Err(ScaleError::InvalidRootSelector {
            source_root_node_index,
        });
    }

    let domain =
        rest_bind_affected_closure(document, &by_source_index, skin, source_root_node_index)?;

    let mut bone_of_source: BTreeMap<usize, BoneId> = BTreeMap::new();
    for &source in &domain {
        let asset = by_source_index[&source];
        let bone = asset.bone.ok_or(ScaleError::SourceNodeNotNormalized {
            source_node_index: source,
        })?;
        bone_of_source.insert(source, bone);
    }
    let scaled_root_bone = bone_of_source[&source_root_node_index];

    let tol = ScaleTolerancePolicy::APPENDIX_D_V1;
    let mut world_cache: BTreeMap<usize, Mat4> = BTreeMap::new();
    let mut node_factor: BTreeMap<BoneId, f64> = BTreeMap::new();
    for &source in &domain {
        let world = source_world_matrix(source, &by_source_index, &mut world_cache)?;
        let bone = bone_of_source[&source];
        let linear = Mat3::from_mat4(world);
        let factor = classify_affine(linear, &tol)
            .map_err(|reason| ScaleError::InvalidAffineDomain { node: bone, reason })?;
        node_factor.insert(bone, factor);
    }
    let observed_common = node_factor[&scaled_root_bone];
    if !tol.relative(tol.common_factor, observed_common, expected_factor) {
        return Err(ScaleError::FactorMismatch {
            expected: expected_factor,
            observed: observed_common,
        });
    }
    for (&bone, &factor) in &node_factor {
        if bone == scaled_root_bone {
            continue;
        }
        if !tol.relative(tol.common_factor, factor, observed_common) {
            return Err(ScaleError::MixedFactor {
                expected: observed_common,
                observed: factor,
                node: bone,
            });
        }
    }

    let affected_nodes: Vec<BoneId> = {
        let set: BTreeSet<BoneId> = bone_of_source.values().copied().collect();
        set.into_iter().collect()
    };
    let joint_bones: BTreeSet<BoneId> = skin
        .joint_source_node_indices
        .iter()
        .map(|joint| bone_of_source[joint])
        .collect();
    let transform_only_attachments: Vec<BoneId> = affected_nodes
        .iter()
        .copied()
        .filter(|&bone| bone != scaled_root_bone && !joint_bones.contains(&bone))
        .collect();

    for (clip_index, clip) in document.clips.iter().enumerate() {
        for track in &clip.tracks {
            if track.property == Property::Scale && affected_nodes.contains(&track.bone) {
                return Err(ScaleError::AffectedScaleAnimation {
                    clip_index,
                    node: track.bone,
                });
            }
        }
    }

    let prove_bounds = has_skinned_evidence(document, &affected_nodes.iter().copied().collect());

    Ok(ScalePlan {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index,
            source_root_node_index,
            expected_factor,
        },
        tolerance_policy: tol,
        affected_nodes,
        transform_only_attachments,
        common_factor: expected_factor,
        domain_rewrites: ScaleDomainRewrites {
            rest_hierarchy: true,
            translation_animation: true,
            inverse_binds: true,
            base_mesh_positions: false,
        },
        proof_obligations: ScaleProofObligations {
            prove_rest: true,
            prove_unit_scale_postcondition: true,
            prove_transform_only_affine: true,
            prove_keys: true,
            prove_cubic_interiors: true,
            prove_trajectories: true,
            prove_skin: true,
            prove_bounds,
        },
    })
}

/// Resolve `source_skin_index` against
/// `document.assets.source_skeleton.skins` by
/// [`SourceSkinAsset::source_skin_index`] — never by raw array position,
/// since a loader's source-skin indices need not be dense or contiguous.
fn resolve_rest_bind_skin(
    document: &Document,
    source_skin_index: usize,
) -> Result<&SourceSkinAsset, ScaleError> {
    let skin = document
        .assets
        .source_skeleton
        .skins
        .iter()
        .find(|skin| skin.source_skin_index == source_skin_index)
        .ok_or(ScaleError::InvalidSkinSelector { source_skin_index })?;
    if skin.joint_source_node_indices.is_empty() {
        return Err(ScaleError::InvalidSkinSelector { source_skin_index });
    }
    Ok(skin)
}

fn source_node_index_map(document: &Document) -> BTreeMap<usize, &SourceNodeAsset> {
    document
        .assets
        .source_skeleton
        .nodes
        .iter()
        .map(|node| (node.source_node_index, node))
        .collect()
}

/// Compute the closed connected hierarchy of DESIGN.md Appendix D §D.2 in
/// raw source-node identity space: the scaled ancestor, every selected skin
/// joint and the paths between them, and every descendant whose attachment
/// transform would otherwise inherit the common factor.
fn rest_bind_affected_closure(
    document: &Document,
    by_source_index: &BTreeMap<usize, &SourceNodeAsset>,
    skin: &SourceSkinAsset,
    source_root_node_index: usize,
) -> Result<BTreeSet<usize>, ScaleError> {
    // Built up front so every insertion below — root, joint, ancestor-path
    // helper, or later descendant — is checked against the same evidence,
    // rather than only the descendants a later BFS happens to visit.
    let unskinned_attachment: BTreeSet<usize> = document
        .assets
        .instances
        .iter()
        .filter(|instance| instance.skin_joints.is_empty())
        .map(|instance| instance.source_node_index)
        .collect();
    let reject_if_unskinned = |source_node_index: usize| -> Result<(), ScaleError> {
        if !unskinned_attachment.contains(&source_node_index) {
            return Ok(());
        }
        let bone = by_source_index
            .get(&source_node_index)
            .and_then(|asset| asset.bone)
            .ok_or(ScaleError::SourceNodeNotNormalized { source_node_index })?;
        Err(ScaleError::UnsupportedUnskinnedGeometry { node: bone })
    };

    let mut domain = BTreeSet::new();
    reject_if_unskinned(source_root_node_index)?;
    domain.insert(source_root_node_index);
    for &joint in &skin.joint_source_node_indices {
        if !by_source_index.contains_key(&joint) {
            return Err(ScaleError::IncompleteClosure {
                reason: "skin_joint_source_node_missing",
            });
        }
        reject_if_unskinned(joint)?;
        let mut cursor = joint;
        domain.insert(cursor);
        // Bound the ancestor walk by the known node count: a well-formed
        // source has each node at most one hop closer to the root, but this
        // module is format-neutral and must not assume an upstream loader
        // already rejected a cyclic or forward-referencing parent chain.
        let mut steps = 0usize;
        loop {
            if cursor == source_root_node_index {
                break;
            }
            steps += 1;
            if steps > by_source_index.len() {
                return Err(ScaleError::IncompleteClosure {
                    reason: "cyclic_or_unbounded_source_parent_chain",
                });
            }
            // `cursor` was itself reached via a parent link from a node
            // already confirmed present, but the parent it names need not
            // be: a dangling `parent_source_node_index` must fail closed
            // with a typed error rather than panic on this index.
            let asset = *by_source_index
                .get(&cursor)
                .ok_or(ScaleError::IncompleteClosure {
                    reason: "dangling_source_parent_node_index",
                })?;
            match asset.parent_source_node_index {
                Some(parent) => {
                    reject_if_unskinned(parent)?;
                    domain.insert(parent);
                    cursor = parent;
                }
                None => {
                    return Err(ScaleError::IncompleteClosure {
                        reason: "joint_not_descendant_of_scaled_root",
                    });
                }
            }
        }
    }

    let mut children: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for node in &document.assets.source_skeleton.nodes {
        if let Some(parent) = node.parent_source_node_index {
            children
                .entry(parent)
                .or_default()
                .push(node.source_node_index);
        }
    }
    // Joint ownership spans every declared skin, not just the selected
    // instance: a descendant claimed as a joint by a different skin closes
    // the domain rather than being silently absorbed.
    let joint_owner: BTreeMap<usize, usize> = document
        .assets
        .source_skeleton
        .skins
        .iter()
        .flat_map(|skin| {
            skin.joint_source_node_indices
                .iter()
                .map(move |&joint| (joint, skin.source_skin_index))
        })
        .collect();

    let mut queue: Vec<usize> = domain.iter().copied().collect();
    while let Some(node) = queue.pop() {
        for &child in children.get(&node).into_iter().flatten() {
            if domain.contains(&child) {
                continue;
            }
            if let Some(&owner) = joint_owner.get(&child)
                && owner != skin.source_skin_index
            {
                return Err(ScaleError::IncompleteClosure {
                    reason: "descendant_joint_of_another_skin",
                });
            }
            reject_if_unskinned(child)?;
            domain.insert(child);
            queue.push(child);
        }
    }
    Ok(domain)
}

/// Compose `start`'s raw rest-world matrix from
/// `document.assets.source_skeleton.nodes`, walking the full
/// `parent_source_node_index` ancestor chain (not stopping at any affected
/// closure boundary) so classification sees the node's true rest-world
/// linear part.
///
/// Iterative and cache/visited-guarded rather than naive recursion: a
/// malformed cyclic or self-referencing parent chain errors instead of
/// looping or overflowing the stack.
fn source_world_matrix(
    start: usize,
    by_source_index: &BTreeMap<usize, &SourceNodeAsset>,
    cache: &mut BTreeMap<usize, Mat4>,
) -> Result<Mat4, ScaleError> {
    if let Some(&world) = cache.get(&start) {
        return Ok(world);
    }
    let mut chain = Vec::new();
    let mut cursor = start;
    let mut visited = BTreeSet::new();
    let base_world = loop {
        if let Some(&world) = cache.get(&cursor) {
            break world;
        }
        if !visited.insert(cursor) {
            return Err(ScaleError::IncompleteClosure {
                reason: "cyclic_source_parent_chain",
            });
        }
        let asset = *by_source_index
            .get(&cursor)
            .ok_or(ScaleError::IncompleteClosure {
                reason: "missing_source_node",
            })?;
        chain.push((cursor, asset));
        match asset.parent_source_node_index {
            Some(parent) => cursor = parent,
            None => break Mat4::IDENTITY,
        }
    };
    let mut world = base_world;
    for (node, asset) in chain.into_iter().rev() {
        let local = local_rest_matrix(&asset.local_rest);
        if !mat4_is_finite(local) {
            return Err(ScaleError::NonFiniteSourceTransform {
                source_node_index: node,
            });
        }
        world *= local;
        if !mat4_is_finite(world) {
            return Err(ScaleError::NonFiniteSourceTransform {
                source_node_index: node,
            });
        }
        cache.insert(node, world);
    }
    Ok(world)
}

/// Compose a raw authored local-rest matrix, preserving shear: the `Trs`
/// variant round-trips through `Mat4::from_scale_rotation_translation`
/// (necessarily orthogonal/uniform-representable), while `Matrix` is used
/// as-is — the only representation that can carry a literal shear term.
fn local_rest_matrix(rest: &SourceNodeLocalRest) -> Mat4 {
    match rest {
        SourceNodeLocalRest::Trs {
            translation,
            rotation,
            scale,
        } => Mat4::from_scale_rotation_translation(*scale, *rotation, *translation),
        SourceNodeLocalRest::Matrix(matrix) => *matrix,
    }
}

/// Classify one rest-world linear part against the fixed tolerance policy,
/// returning its common uniform factor.
///
/// Every component is widened to `f64` *first* and every derived quantity —
/// column lengths, determinant, and column dot products — is then evaluated
/// entirely in `f64`, per DESIGN.md Appendix D §D.1 ("Classification and
/// proof share one versioned tolerance policy and compute in `f64`,
/// narrowing only at the writer model boundary"). Evaluating in `f32` and
/// casting the result afterwards is not the same thing: an `f32` column dot
/// product of three near-unit axes carries roughly `1e-7` of cancellation
/// error against a `1e-5` relative threshold scaled by `average^2`, which at
/// small factors is enough to flip a genuinely sheared basis to accepted (a
/// ULP sweep finds pairs whose `f32` dot is `-9.98e-6` and whose `f64` dot is
/// `-1.00e-5`, either side of the threshold).
fn classify_affine(linear: Mat3, tol: &ScaleTolerancePolicy) -> Result<f64, AffineDomainViolation> {
    if !linear.is_finite() {
        return Err(AffineDomainViolation::NonFinite);
    }
    let columns = [
        linear.x_axis.as_dvec3(),
        linear.y_axis.as_dvec3(),
        linear.z_axis.as_dvec3(),
    ];
    let lengths = [
        columns[0].length(),
        columns[1].length(),
        columns[2].length(),
    ];
    if lengths.iter().any(|length| !length.is_finite()) {
        return Err(AffineDomainViolation::NonFinite);
    }
    let average = (lengths[0] + lengths[1] + lengths[2]) / 3.0;
    if average <= 0.0 {
        return Err(AffineDomainViolation::Singular);
    }
    // Determinant/singularity is checked before the uniform-axis and
    // orthogonality checks: a rigid orthogonal basis with equal-length axes
    // can never itself be near-singular (its determinant is forced to
    // `±average^3`), so a degenerate matrix that also happens to have
    // unequal or non-orthogonal axes must still classify as singular first,
    // matching DESIGN.md Appendix D's independent violation fixtures.
    //
    // Expanded as the scalar triple product of the already-widened columns
    // rather than through `Mat3::determinant`, which would evaluate the whole
    // expansion in `f32`.
    let determinant = columns[2].dot(columns[0].cross(columns[1]));
    if !determinant.is_finite() {
        return Err(AffineDomainViolation::NonFinite);
    }
    let axis_product = lengths[0] * lengths[1] * lengths[2];
    if determinant.abs() <= tol.singular_determinant_relative * axis_product {
        return Err(AffineDomainViolation::Singular);
    }
    // Relative to the axis magnitudes themselves — `average` is already
    // proven positive above, so no floor is needed and none may be used: a
    // `1.0` floor would make this an absolute `1e-5` test for every rig
    // authored below unit magnitude, accepting `(0.01, 0.01, 0.010005)` as
    // uniform.
    if lengths
        .iter()
        .any(|&length| (length - average).abs() > tol.equal_axis * average.max(length))
    {
        return Err(AffineDomainViolation::NonUniformScale);
    }
    let dot01 = columns[0].dot(columns[1]);
    let dot02 = columns[0].dot(columns[2]);
    let dot12 = columns[1].dot(columns[2]);
    let orthogonality_tolerance = tol.relative_orthogonality * average * average;
    if dot01.abs() > orthogonality_tolerance
        || dot02.abs() > orthogonality_tolerance
        || dot12.abs() > orthogonality_tolerance
    {
        return Err(AffineDomainViolation::Sheared);
    }
    if determinant < 0.0 {
        return Err(AffineDomainViolation::Reflected);
    }
    Ok(average)
}

/// Compose every [`Bone::rest`](crate::model::Bone::rest) local transform in
/// `skeleton` into a parent-before-child world matrix, delegating to the
/// shared helper [`crate::model::world_rest_matrices`] and mapping its
/// structural error into this module's [`ScaleError`].
fn world_rests(skeleton: &Skeleton) -> Result<Vec<Mat4>, ScaleError> {
    world_rest_matrices(skeleton).map_err(|error| match error {
        crate::model::WorldMatrixError::NonFiniteTransform { node } => {
            ScaleError::NonFiniteTransform { node }
        }
        crate::model::WorldMatrixError::InvalidParent { node, parent } => {
            ScaleError::InvalidParent { node, parent }
        }
    })
}

// --- Candidate construction -------------------------------------------------

/// A candidate document built from an accepted [`ScalePlan`].
///
/// This type deliberately has no mutation method: [`build_scale_candidate`]
/// is the only way to produce one, and it never partially mutates the
/// caller's source document — a failure simply drops the half-built
/// candidate before this type is constructed.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ScaleCandidate {
    document: Document,
}

impl ScaleCandidate {
    /// The candidate document.
    pub fn document(&self) -> &Document {
        &self.document
    }

    /// Consume this candidate, taking ownership of the document.
    pub fn into_document(self) -> Document {
        self.document
    }
}

/// Build a candidate document from an accepted [`ScalePlan`], without
/// mutating `document`.
///
/// `document` need not be the exact document `plan` was computed against —
/// every index [`ScalePlan`] carries is independently re-validated against
/// `document` rather than trusted, so a stale or mismatched plan produces a
/// typed error instead of a panic or a silently wrong candidate.
///
/// # Errors
///
/// Returns [`ScaleError::AffectedScaleAnimation`] if a scale track targets
/// an affected node in `document` (including one added after `plan` was
/// computed), [`ScaleError::BoneIndexOutOfRange`] if an affected node in
/// `plan` is out of range for `document`, [`ScaleError::MissingInverseBind`]
/// if an affected skin slot has no inverse-bind evidence to conjugate, or
/// any document-shape error — checked on the *candidate* as
/// well as the input, so a build can never hand back a structurally invalid
/// or non-finite document as `Ok`.
pub fn build_scale_candidate(
    document: &Document,
    plan: &ScalePlan,
) -> Result<ScaleCandidate, ScaleError> {
    validate_document_shape(document)?;
    let candidate = match plan.operation {
        ScaleOperation::WholeDocumentLinearUnits { factor } => {
            build_whole_document(document, factor)?
        }
        ScaleOperation::RestBindUniformScale { .. } => build_rest_bind(document, plan)?,
    };
    // The same fail-closed shape check the input had to pass, re-run on the
    // output: a builder is the one place in this module that writes numbers,
    // so it must not be the one place that returns unvalidated ones. Without
    // this, an overflowing or annihilating factor produces a candidate whose
    // only remaining defence is `prove_scale`, which a caller is free not to
    // run.
    validate_document_shape(&candidate)?;
    Ok(ScaleCandidate {
        document: candidate,
    })
}

fn build_whole_document(document: &Document, factor: f64) -> Result<Document, ScaleError> {
    let q = check_factor_narrows(factor, factor)?;
    let mut candidate = document.clone();
    for bone in &mut candidate.skeleton.bones {
        bone.rest.translation *= q;
        if let Some(inverse_bind) = &mut bone.inverse_bind {
            *inverse_bind = scale_translation_only(*inverse_bind, q);
        }
    }
    // The raw source-node projection is rewritten alongside the normalized
    // skeleton so the candidate's own evidence stays truthful. Leaving it
    // stale is not merely cosmetic: `plan_rest_bind` classifies the affine
    // domain from `source_skeleton`, so a candidate carrying its pre-scale
    // projection re-plans as though it were never converted and
    // double-applies the factor. `M' = U M U^-1` for a uniform `U` scales the
    // translation column and leaves the linear part alone, which for a `Trs`
    // node is exactly "multiply the translation, keep rotation and scale".
    for node in &mut candidate.assets.source_skeleton.nodes {
        node.local_rest = match &node.local_rest {
            SourceNodeLocalRest::Trs {
                translation,
                rotation,
                scale,
            } => SourceNodeLocalRest::Trs {
                translation: *translation * q,
                rotation: *rotation,
                scale: *scale,
            },
            SourceNodeLocalRest::Matrix(matrix) => {
                SourceNodeLocalRest::Matrix(scale_translation_only(*matrix, q))
            }
        };
    }
    for clip in &mut candidate.clips {
        for track in &mut clip.tracks {
            if track.property != Property::Translation {
                continue;
            }
            if let TrackValues::Vec3s(values) = &mut track.values {
                for value in values.iter_mut() {
                    *value *= q;
                }
            }
        }
    }
    for mesh in &mut candidate.assets.meshes {
        for primitive in &mut mesh.primitives {
            for position in &mut primitive.positions {
                *position *= q;
            }
        }
    }
    for instance in &mut candidate.assets.instances {
        for inverse_bind in &mut instance.skin_ibms {
            *inverse_bind = scale_translation_only(*inverse_bind, q);
        }
    }
    Ok(candidate)
}

fn build_rest_bind(document: &Document, plan: &ScalePlan) -> Result<Document, ScaleError> {
    let affected = plan.affected_set();
    let s = check_factor_narrows(plan.common_factor, plan.common_factor)?;
    let parent_factor = |node: BoneId| -> Result<f32, ScaleError> {
        let bone = document
            .skeleton
            .bones
            .get(node)
            .ok_or(ScaleError::BoneIndexOutOfRange { index: node })?;
        Ok(match bone.parent {
            Some(parent) if affected.contains(&parent) => s,
            _ => 1.0,
        })
    };
    let node_factor = |node: BoneId| -> f32 { if affected.contains(&node) { s } else { 1.0 } };

    let mut candidate = document.clone();
    if plan.domain_rewrites.rest_hierarchy {
        for (node, bone) in candidate.skeleton.bones.iter_mut().enumerate() {
            if !affected.contains(&node) {
                continue;
            }
            let s_parent = parent_factor(node)?;
            let s_node = node_factor(node);
            bone.rest.translation *= s_parent;
            bone.rest.scale *= s_parent / s_node;
            if plan.domain_rewrites.inverse_binds
                && let Some(inverse_bind) = &mut bone.inverse_bind
            {
                *inverse_bind = scale_rows(*inverse_bind, s_node);
            }
        }
        // The raw source-node projection is rebased alongside the normalized
        // skeleton, applying the same `L' = C_parent^-1 * L * C_i` of
        // DESIGN.md Appendix D §D.2 to the authored local rest. Without this
        // the candidate keeps a projection describing the *pre*-rebase
        // hierarchy while still asserting
        // `SourceSkeletonCoverage::Complete`, and since `plan_rest_bind`
        // classifies the affine domain from exactly that projection, the
        // candidate re-plans as though it had never been rebased and the
        // factor is applied a second time. Rewriting it (rather than
        // downgrading coverage to `Unavailable`) also keeps the source
        // inverse-bind-accessor evidence that `instance_bind` reads under
        // complete coverage available.
        for node in &mut candidate.assets.source_skeleton.nodes {
            let Some(bone) = node.bone.filter(|bone| affected.contains(bone)) else {
                continue;
            };
            let s_parent = parent_factor(bone)?;
            let s_node = node_factor(bone);
            node.local_rest = match &node.local_rest {
                SourceNodeLocalRest::Trs {
                    translation,
                    rotation,
                    scale,
                } => SourceNodeLocalRest::Trs {
                    translation: *translation * s_parent,
                    rotation: *rotation,
                    scale: *scale * (s_parent / s_node),
                },
                SourceNodeLocalRest::Matrix(matrix) => {
                    SourceNodeLocalRest::Matrix(rebase_matrix(*matrix, s_parent, s_node))
                }
            };
        }
    }
    if plan.domain_rewrites.translation_animation {
        for (clip_index, clip) in candidate.clips.iter_mut().enumerate() {
            for track in &mut clip.tracks {
                if !affected.contains(&track.bone) {
                    continue;
                }
                match track.property {
                    Property::Translation => {
                        let s_parent = parent_factor(track.bone)?;
                        if let TrackValues::Vec3s(values) = &mut track.values {
                            for value in values.iter_mut() {
                                *value *= s_parent;
                            }
                        }
                    }
                    Property::Scale => {
                        return Err(ScaleError::AffectedScaleAnimation {
                            clip_index,
                            node: track.bone,
                        });
                    }
                    Property::Rotation => {}
                }
            }
        }
    }
    if plan.domain_rewrites.inverse_binds {
        // Every affected instance's binds are resolved through the same
        // documented fallback chain `prove_scale` uses — instance array,
        // else the bone convenience value, else the source skin's
        // format-defined identity default — and the conjugated result is
        // written back as an explicit per-slot array.
        //
        // Materializing rather than rewriting in place is what closes the
        // fail-open case where a skin legitimately omits its inverse-bind
        // accessor (glTF's "no `inverseBindMatrices`, every joint's bind is
        // identity"): such an instance has an *empty* `skin_ibms` and every
        // bone has `inverse_bind == None`, so an in-place rewrite touches
        // nothing at all and silently emits `W' * B' = W * C * I != W * B`.
        // Writing the conjugated default into the candidate keeps the
        // operation total and makes the candidate self-describing, at the
        // cost of turning an implicit format default into an explicit array
        // — which is a fact about the *output*, not a claim about the source
        // bytes, and is exactly what the writer would have to emit anyway
        // now that the bind is no longer identity.
        //
        // A slot with genuinely no evidence still fails closed here with
        // `MissingInverseBind`, at build time rather than as an opaque
        // `SkinMatrix` residual at proof time.
        for (source_instance, instance) in document
            .assets
            .instances
            .iter()
            .zip(candidate.assets.instances.iter_mut())
        {
            if !source_instance
                .skin_joints
                .iter()
                .any(|joint| affected.contains(joint))
            {
                continue;
            }
            let mut rebased = Vec::with_capacity(source_instance.skin_joints.len());
            for (slot, &joint) in source_instance.skin_joints.iter().enumerate() {
                let before = instance_bind(document, source_instance, slot, joint)?;
                rebased.push(scale_rows(before, node_factor(joint)));
            }
            instance.skin_ibms = rebased;
        }
    }
    Ok(candidate)
}

/// `B' = U B U^-1` for a uniform `U = scale(q)`: the translation column
/// scales by `q`; the linear part is unchanged.
fn scale_translation_only(matrix: Mat4, q: f32) -> Mat4 {
    let mut matrix = matrix;
    matrix.w_axis.x *= q;
    matrix.w_axis.y *= q;
    matrix.w_axis.z *= q;
    matrix
}

/// `scale(k) * M`: every output row (x/y/z, not the homogeneous row) scales
/// by `k`, which is what left-multiplying by a uniform scale does to both
/// the linear part and the translation column.
fn scale_rows(matrix: Mat4, k: f32) -> Mat4 {
    let scale_column = |c: Vec4| Vec4::new(c.x * k, c.y * k, c.z * k, c.w);
    Mat4::from_cols(
        scale_column(matrix.x_axis),
        scale_column(matrix.y_axis),
        scale_column(matrix.z_axis),
        scale_column(matrix.w_axis),
    )
}

/// `L' = scale(s_parent) * L * scale(1 / s_node)`: the rest/bind local
/// rebase of DESIGN.md Appendix D §D.2, applied to a raw authored matrix
/// that may carry terms a TRS decomposition cannot represent.
///
/// Left-multiplying by a uniform scale scales the output rows (that is
/// [`scale_rows`]); right-multiplying by `scale(1 / s_node)` scales the three
/// linear columns in full, translation column untouched.
fn rebase_matrix(matrix: Mat4, s_parent: f32, s_node: f32) -> Mat4 {
    let scaled = scale_rows(matrix, s_parent);
    let inverse_node = 1.0 / s_node;
    Mat4::from_cols(
        scaled.x_axis * inverse_node,
        scaled.y_axis * inverse_node,
        scaled.z_axis * inverse_node,
        scaled.w_axis,
    )
}

// --- Proof -------------------------------------------------------------

/// Observed residual maxima from [`prove_scale`], reported against
/// [`ScalePlan::tolerance_policy`].
///
/// A field for an obligation the plan does not require (see
/// [`ScaleProofObligations`]) reports `0.0`.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct ScaleProof {
    /// The tolerance policy every residual below was checked against.
    pub tolerance_policy: ScaleTolerancePolicy,
    /// Maximum rest-world translation residual across affected nodes.
    pub rest_translation_residual: f64,
    /// Maximum rest-world rotation residual, in radians, directly comparable
    /// to [`ScaleTolerancePolicy::rotation_residual_radians`].
    ///
    /// Measured as a double-cover-aware quaternion chord length
    /// `|q1 - q2| = 2 * sin(theta / 4)` and reported as the angle
    /// `theta = 4 * asin(chord / 2)` that chord represents, so this value and
    /// the tolerance it is checked against carry the same unit.
    pub rest_rotation_residual: f64,
    /// Maximum postcondition unit-scale residual (rest/bind only).
    pub unit_scale_residual: f64,
    /// Maximum transform-only attachment full-affine residual (rest/bind
    /// only).
    pub transform_only_affine_residual: f64,
    /// Maximum per-element animation-track value residual, across rewritten
    /// translation elements and every retained rotation/scale element.
    pub track_value_residual: f64,
    /// Maximum per-vertex base mesh `POSITION` residual.
    pub mesh_position_residual: f64,
    /// Maximum residual at any affected-track keyframe time.
    pub key_translation_residual: f64,
    /// Maximum residual at any bounded cubic-segment interior time.
    pub cubic_interior_residual: f64,
    /// Maximum sampled world-space trajectory residual.
    pub trajectory_residual: f64,
    /// Maximum skin-matrix (`W * B`) component residual, across rest and
    /// every sampled key/cubic-interior time.
    pub skin_matrix_residual: f64,
    /// Maximum skinned mesh bounds residual, across rest and every sampled
    /// key/cubic-interior time.
    pub bounds_residual: f64,
    /// Number of distinct times sampled across all clips.
    pub sample_time_count: usize,
}

/// Independently re-derive and check every claim [`ScalePlan`] makes.
///
/// Proof runs on the in-memory candidate, re-deriving world matrices,
/// sampled trajectories, skin matrices, and bounds from `source` and
/// `candidate` rather than trusting how they were built. Every comparison
/// uses [`ScaleTolerancePolicy::scalar_tolerance`] computed from that
/// comparison's own actual before/after magnitudes, never a proxy such as
/// the plan's declared factor. Neither `source` nor `candidate` need be the
/// exact documents `plan` was computed against: every index is
/// independently re-validated.
///
/// # Errors
///
/// Returns [`ScaleError::ProofResidualExceeded`] for the first residual that
/// exceeds [`ScalePlan::tolerance_policy`], or
/// [`ScaleError::MissingProofEvidence`] if an obligation the plan declares
/// provable has no counterpart evidence in `candidate`.
pub fn prove_scale(
    source: &Document,
    candidate: &ScaleCandidate,
    plan: &ScalePlan,
) -> Result<ScaleProof, ScaleError> {
    let candidate = candidate.document();
    validate_document_shape(source)?;
    validate_document_shape(candidate)?;
    validate_candidate_structure(source, candidate)?;
    let tol = plan.tolerance_policy;
    let affected = plan.affected_set();
    let source_worlds = world_rests(&source.skeleton)?;
    let candidate_worlds = world_rests(&candidate.skeleton)?;

    let mut proof = ScaleProof {
        tolerance_policy: tol,
        rest_translation_residual: 0.0,
        rest_rotation_residual: 0.0,
        unit_scale_residual: 0.0,
        transform_only_affine_residual: 0.0,
        track_value_residual: 0.0,
        mesh_position_residual: 0.0,
        key_translation_residual: 0.0,
        cubic_interior_residual: 0.0,
        trajectory_residual: 0.0,
        skin_matrix_residual: 0.0,
        bounds_residual: 0.0,
        sample_time_count: 0,
    };

    check_candidate_values(source, candidate, &affected, plan, &tol, &mut proof)?;

    if plan.proof_obligations.prove_rest {
        for &node in &plan.affected_nodes {
            let before = *source_worlds
                .get(node)
                .ok_or(ScaleError::BoneIndexOutOfRange { index: node })?;
            let after = *candidate_worlds
                .get(node)
                .ok_or(ScaleError::BoneIndexOutOfRange { index: node })?;
            let (translation_residual, before_mag, after_mag) =
                rest_node_residual(before, after, plan.is_whole_document(), plan.common_factor);
            check_and_track(
                ProofResidualKind::RestTranslation,
                translation_residual,
                before_mag,
                after_mag,
                &tol,
                &mut proof.rest_translation_residual,
            )?;
            // Both operations leave every node's *local* rotation field
            // byte-identical (translation and scale are the only rewritten
            // channels), so proving world orientation preservation by
            // comparing local rotations directly avoids a lossy matrix
            // decomposition and, by composition, implies preserved world
            // orientation for every node in the chain.
            //
            // This is therefore an *equality* test on a field no build path
            // writes, not an angle measurement — and it must not be spelled
            // as one. `Quat::angle_between` does not normalize its operands,
            // so an authored quaternion with `|q| = 1 - eps` (routine in
            // glTF, and which `invariant-9` forbids the loader from
            // renormalizing) reports roughly `2 * sqrt(4 * eps)` against
            // itself: the perfectly ordinary key `[0, 0.7071067, 0,
            // 0.7071067]` measures `1.2e-3` against an identical copy, `120x`
            // the `1e-5` tolerance, and a correct candidate for a real rig is
            // rejected as `RestRotation`.
            let source_rotation = source
                .skeleton
                .bones
                .get(node)
                .ok_or(ScaleError::BoneIndexOutOfRange { index: node })?
                .rest
                .rotation;
            let candidate_rotation = candidate
                .skeleton
                .bones
                .get(node)
                .ok_or(ScaleError::BoneIndexOutOfRange { index: node })?
                .rest
                .rotation;
            // [`quat_equality_residual`] answers in *chord* space, and this
            // obligation's declared bound is an angle, so the chord is
            // converted to the angle it represents before it is either
            // reported or compared (see [`quat_residual_radians`]).
            let rotation_residual =
                quat_residual_radians(quat_equality_residual(source_rotation, candidate_rotation));
            proof.rest_rotation_residual = proof.rest_rotation_residual.max(rotation_residual);
            check_residual(
                ProofResidualKind::RestRotation,
                rotation_residual,
                tol.rotation_residual_radians,
            )?;
            if plan.proof_obligations.prove_unit_scale_postcondition {
                let (after_scale, ..) = after.to_scale_rotation_translation();
                let residual = ((after_scale.x as f64 - 1.0).powi(2)
                    + (after_scale.y as f64 - 1.0).powi(2)
                    + (after_scale.z as f64 - 1.0).powi(2))
                .sqrt();
                proof.unit_scale_residual = proof.unit_scale_residual.max(residual);
                check_residual(
                    ProofResidualKind::UnitScale,
                    residual,
                    tol.postcondition_unit_scale_residual,
                )?;
            }
        }
    }

    if plan.proof_obligations.prove_transform_only_affine {
        // scale(1/s): the analytically expected basis correction `C_i` for
        // every node inside the affected domain (DESIGN.md Appendix D §D.2).
        let correction = Mat4::from_scale(Vec3::splat((1.0 / plan.common_factor) as f32));
        // A fixed off-origin local probe point: transforming it through the
        // complete expected/actual world affine — rather than decomposing
        // to translation/rotation and checking only those — is what makes a
        // no-op (or any build that drops the linear-scale channel) provably
        // fail this check.
        let probe = Vec3::ONE;
        for &node in &plan.transform_only_attachments {
            let before = *source_worlds
                .get(node)
                .ok_or(ScaleError::BoneIndexOutOfRange { index: node })?;
            let after = *candidate_worlds
                .get(node)
                .ok_or(ScaleError::BoneIndexOutOfRange { index: node })?;
            let expected_point = (before * correction).transform_point3(probe).as_dvec3();
            let actual_point = after.transform_point3(probe).as_dvec3();
            let residual = (actual_point - expected_point).length();
            check_and_track(
                ProofResidualKind::TransformOnlyAffine,
                residual,
                expected_point.length(),
                actual_point.length(),
                &tol,
                &mut proof.transform_only_affine_residual,
            )?;
        }
    }

    if plan.proof_obligations.prove_skin {
        check_skin_matrices(
            source,
            candidate,
            &source_worlds,
            &candidate_worlds,
            &affected,
            plan,
            &tol,
            &mut proof.skin_matrix_residual,
        )?;
    }
    if plan.proof_obligations.prove_bounds {
        check_bounds(
            source,
            candidate,
            &source_worlds,
            &candidate_worlds,
            &affected,
            plan,
            &tol,
            &mut proof.bounds_residual,
        )?;
    }

    let any_sampled_obligation = plan.proof_obligations.prove_keys
        || plan.proof_obligations.prove_cubic_interiors
        || plan.proof_obligations.prove_trajectories
        || plan.proof_obligations.prove_skin
        || plan.proof_obligations.prove_bounds;
    if any_sampled_obligation {
        for (clip_index, clip) in source.clips.iter().enumerate() {
            let candidate_clip =
                candidate
                    .clips
                    .get(clip_index)
                    .ok_or(ScaleError::MissingProofEvidence {
                        kind: ProofResidualKind::KeyTranslation,
                        detail: "candidate_clip_missing",
                    })?;
            let (key_times, interior_times) = clip_sample_times(clip, &affected);
            for &t in &key_times {
                proof.sample_time_count += 1;
                if plan.proof_obligations.prove_keys {
                    check_track_value_residual(
                        ProofResidualKind::KeyTranslation,
                        source,
                        clip,
                        candidate_clip,
                        &affected,
                        t,
                        plan,
                        &tol,
                        &mut proof.key_translation_residual,
                    )?;
                }
                sample_time_obligations(
                    source,
                    candidate,
                    clip,
                    candidate_clip,
                    t,
                    &affected,
                    plan,
                    &tol,
                    &mut proof,
                )?;
            }
            for &t in &interior_times {
                proof.sample_time_count += 1;
                if plan.proof_obligations.prove_cubic_interiors {
                    check_track_value_residual(
                        ProofResidualKind::CubicInterior,
                        source,
                        clip,
                        candidate_clip,
                        &affected,
                        t,
                        plan,
                        &tol,
                        &mut proof.cubic_interior_residual,
                    )?;
                }
                sample_time_obligations(
                    source,
                    candidate,
                    clip,
                    candidate_clip,
                    t,
                    &affected,
                    plan,
                    &tol,
                    &mut proof,
                )?;
            }
        }
    }

    Ok(proof)
}

#[allow(clippy::too_many_arguments)]
fn sample_time_obligations(
    source: &Document,
    candidate: &Document,
    source_clip: &Clip,
    candidate_clip: &Clip,
    t: f32,
    affected: &BTreeSet<BoneId>,
    plan: &ScalePlan,
    tol: &ScaleTolerancePolicy,
    proof: &mut ScaleProof,
) -> Result<(), ScaleError> {
    if plan.proof_obligations.prove_trajectories {
        check_trajectory_residual_at(
            source,
            candidate,
            source_clip,
            candidate_clip,
            &plan.affected_nodes,
            t,
            plan,
            tol,
            &mut proof.trajectory_residual,
        )?;
    }
    if plan.proof_obligations.prove_skin {
        let source_worlds = world_at_time(&source.skeleton, source_clip, t)?;
        let candidate_worlds = world_at_time(&candidate.skeleton, candidate_clip, t)?;
        check_skin_matrices(
            source,
            candidate,
            &source_worlds,
            &candidate_worlds,
            affected,
            plan,
            tol,
            &mut proof.skin_matrix_residual,
        )?;
    }
    if plan.proof_obligations.prove_bounds {
        let source_worlds = world_at_time(&source.skeleton, source_clip, t)?;
        let candidate_worlds = world_at_time(&candidate.skeleton, candidate_clip, t)?;
        check_bounds(
            source,
            candidate,
            &source_worlds,
            &candidate_worlds,
            affected,
            plan,
            tol,
            &mut proof.bounds_residual,
        )?;
    }
    Ok(())
}

/// Resolve one skin joint's inverse-bind matrix per the documented
/// [`MeshInstance::skin_ibms`] contract: use the instance's own matrix when
/// present, else fall back to the bone's [`crate::model::Bone::inverse_bind`]
/// — the only fallback this model contract genuinely represents.
///
/// A bone with neither is missing evidence in general, and rejects with
/// [`ScaleError::MissingInverseBind`] rather than substituting
/// [`Mat4::IDENTITY`] to mask a partial or malformed bind array — *unless*
/// the source's own complete-coverage evidence proves this is the
/// format-defined identity default rather than an unavailable or malformed
/// accessor (for example, glTF permits a skin to omit
/// `inverseBindMatrices` entirely, in which case every joint's inverse-bind
/// matrix is defined to be identity). [`instance_source_skin`] only returns
/// that skin evidence when `document.assets.source_skeleton.coverage` is
/// complete, so an incomplete or absent source-skeleton projection still
/// rejects here rather than silently defaulting to identity.
fn instance_bind(
    document: &Document,
    instance: &MeshInstance,
    slot: usize,
    joint: BoneId,
) -> Result<Mat4, ScaleError> {
    if !instance.skin_ibms.is_empty() {
        return instance
            .skin_ibms
            .get(slot)
            .copied()
            .ok_or(ScaleError::BoneIndexOutOfRange { index: joint });
    }
    let bone = document
        .skeleton
        .bones
        .get(joint)
        .ok_or(ScaleError::BoneIndexOutOfRange { index: joint })?;
    if let Some(inverse_bind) = bone.inverse_bind {
        return Ok(inverse_bind);
    }
    if instance_source_skin(document, instance).is_some_and(|skin| {
        skin.inverse_bind_accessor.status == SourceInverseBindAccessorStatus::Absent
    }) {
        return Ok(Mat4::IDENTITY);
    }
    Err(ScaleError::MissingInverseBind { node: joint })
}

/// Resolve the source skin that attaches at `instance`'s source node,
/// per [`crate::model::SourceSkinAttachment::source_node_index`].
///
/// Returns `None` when `document.assets.source_skeleton.coverage` is not
/// [`SourceSkeletonCoverage::Complete`]: an incomplete or unprojected source
/// table cannot vouch for the accessor evidence it does not carry, so an
/// absent flag there must not be read as proof of a format-defined default.
fn instance_source_skin<'a>(
    document: &'a Document,
    instance: &MeshInstance,
) -> Option<&'a SourceSkinAsset> {
    if document.assets.source_skeleton.coverage != SourceSkeletonCoverage::Complete {
        return None;
    }
    document.assets.source_skeleton.skins.iter().find(|skin| {
        skin.attachments
            .iter()
            .any(|attachment| attachment.source_node_index == instance.source_node_index)
    })
}

/// Fail closed on any residual that is not provably within `tolerance`.
///
/// The non-finite guard is load-bearing, not defensive noise: `NaN > x` is
/// `false` for every `x`, so a bare `observed > tolerance` reports a `NaN`
/// residual — the exact signature of a candidate built with an overflowing
/// factor, or of a comparison against a non-finite source value — as a pass.
/// A `NaN` tolerance (from a non-finite before/after magnitude) fails the
/// same way and is rejected for the same reason.
fn check_residual(
    kind: ProofResidualKind,
    observed: f64,
    tolerance: f64,
) -> Result<(), ScaleError> {
    if !observed.is_finite() || !tolerance.is_finite() || observed > tolerance {
        return Err(ScaleError::ProofResidualExceeded {
            kind,
            observed,
            tolerance,
        });
    }
    Ok(())
}

/// Track `observed` into `running_max` and check it against the
/// before/after-derived tolerance for this specific comparison — never a
/// proxy such as the plan's declared factor.
fn check_and_track(
    kind: ProofResidualKind,
    observed: f64,
    before: f64,
    after: f64,
    tol: &ScaleTolerancePolicy,
    running_max: &mut f64,
) -> Result<(), ScaleError> {
    *running_max = running_max.max(observed);
    check_residual(kind, observed, tol.scalar_tolerance(before, after))
}

/// Rest-world translation residual for one node, plus the expected/actual
/// translation magnitudes the caller uses to derive this comparison's own
/// tolerance.
///
/// This deliberately does not also report a rotation residual: extracting a
/// rotation via [`Mat4::to_scale_rotation_translation`] out of a world
/// matrix whose linear part mixes a small uniform scale with an actual
/// rotation is numerically fragile in `f32`, and unnecessary here — both
/// operations leave every node's *local* rotation field untouched, so
/// callers that need a rotation residual should compare
/// [`crate::model::Bone::rest`]`.rotation` directly (see [`prove_scale`]),
/// which is both exact and, by composition, implies preserved world
/// orientation.
fn rest_node_residual(
    before: Mat4,
    after: Mat4,
    whole_document: bool,
    factor: f64,
) -> (f64, f64, f64) {
    let (_, _, before_translation) = before.to_scale_rotation_translation();
    let (_, _, after_translation) = after.to_scale_rotation_translation();
    let expected_translation = if whole_document {
        before_translation.as_dvec3() * factor
    } else {
        before_translation.as_dvec3()
    };
    let actual_translation = after_translation.as_dvec3();
    let translation_residual = (actual_translation - expected_translation).length();
    (
        translation_residual,
        expected_translation.length(),
        actual_translation.length(),
    )
}

/// The multiplier this plan analytically expects a given node's translation
/// values to have been rewritten by.
///
/// Whole-document conversion multiplies every translation by the declared
/// factor. Rest/bind reparameterization multiplies by the target node's
/// *parent-basis* factor: the domain's common factor when the node's parent
/// is itself affected, the unaffected boundary factor of one otherwise.
fn translation_multiplier(
    document: &Document,
    node: BoneId,
    affected: &BTreeSet<BoneId>,
    plan: &ScalePlan,
) -> f64 {
    if plan.is_whole_document() {
        return plan.common_factor;
    }
    if !affected.contains(&node) {
        return 1.0;
    }
    match document
        .skeleton
        .bones
        .get(node)
        .and_then(|bone| bone.parent)
    {
        Some(parent) if affected.contains(&parent) => plan.common_factor,
        _ => 1.0,
    }
}

/// Prove every retained per-element payload directly, not merely its shape.
///
/// [`validate_candidate_structure`] establishes that source and candidate
/// agree on clip/track/instance/mesh/primitive *counts* and on each track's
/// `(bone, property, interpolation, times)` identity — but it never looks
/// inside `values` or `positions`. Both are reachable through this module's
/// public API without any structural mismatch: [`build_scale_candidate`] and
/// [`prove_scale`] each take their document as a separate argument and do
/// not require it to be the same one, so a candidate built from a doctored
/// document can be proved against the real source. Without a direct
/// comparison a rotation key rewritten from `0.1` to `2.5` radians, or an
/// interior mesh vertex moved anywhere at all, passes proof: the sampled
/// obligations only look at translation, world *joint* transforms, and the
/// bounding box's extreme vertices.
///
/// So every element of every domain is checked here against its analytic
/// expectation: rewritten domains against `before * multiplier` and
/// non-rewritten domains against `before` itself. Comparison is by
/// [`ScaleTolerancePolicy::scalar_tolerance`], never exact float equality,
/// which DESIGN.md Appendix D §D.1 forbids.
fn check_candidate_values(
    source: &Document,
    candidate: &Document,
    affected: &BTreeSet<BoneId>,
    plan: &ScalePlan,
    tol: &ScaleTolerancePolicy,
    proof: &mut ScaleProof,
) -> Result<(), ScaleError> {
    for (source_clip, candidate_clip) in source.clips.iter().zip(candidate.clips.iter()) {
        // Paired positionally: `validate_candidate_structure` already proved
        // each pair agrees on `(bone, property, interpolation, times)` and on
        // value count.
        for (track, candidate_track) in source_clip.tracks.iter().zip(candidate_clip.tracks.iter())
        {
            match (&track.values, &candidate_track.values) {
                (TrackValues::Vec3s(before), TrackValues::Vec3s(after)) => {
                    // Scale tracks are dimensionless and never rewritten by
                    // either operation (an affected one is refused outright
                    // at plan time), so their multiplier is one.
                    let multiplier = match track.property {
                        Property::Translation if plan.domain_rewrites.translation_animation => {
                            translation_multiplier(source, track.bone, affected, plan)
                        }
                        _ => 1.0,
                    };
                    for (before, after) in before.iter().zip(after.iter()) {
                        let expected = before.as_dvec3() * multiplier;
                        let actual = after.as_dvec3();
                        let residual = (actual - expected).length();
                        check_and_track(
                            ProofResidualKind::TrackValue,
                            residual,
                            expected.length(),
                            actual.length(),
                            tol,
                            &mut proof.track_value_residual,
                        )?;
                    }
                }
                (TrackValues::Quats(before), TrackValues::Quats(after)) => {
                    for (before, after) in before.iter().zip(after.iter()) {
                        let residual = quat_equality_residual(*before, *after);
                        check_and_track(
                            ProofResidualKind::TrackValue,
                            residual,
                            before.length() as f64,
                            after.length() as f64,
                            tol,
                            &mut proof.track_value_residual,
                        )?;
                    }
                }
                _ => {
                    return Err(ScaleError::CandidateStructureMismatch {
                        reason: "track_value_variant_mismatch",
                    });
                }
            }
        }
    }

    // Base `POSITION` is proved per primitive, directly. Proving it only
    // through skinned bounds is not equivalent: bounds see one extreme
    // vertex per axis, skip every unskinned instance, and — before this —
    // reported success with a zero residual for a document that has no
    // skinned instance at all, leaving a declared `base_mesh_positions`
    // rewrite compared against nothing.
    let position_multiplier =
        if plan.domain_rewrites.base_mesh_positions && plan.is_whole_document() {
            plan.common_factor
        } else {
            1.0
        };
    for (source_mesh, candidate_mesh) in source
        .assets
        .meshes
        .iter()
        .zip(candidate.assets.meshes.iter())
    {
        for (source_primitive, candidate_primitive) in source_mesh
            .primitives
            .iter()
            .zip(candidate_mesh.primitives.iter())
        {
            for (before, after) in source_primitive
                .positions
                .iter()
                .zip(candidate_primitive.positions.iter())
            {
                let expected = before.as_dvec3() * position_multiplier;
                let actual = after.as_dvec3();
                let residual = (actual - expected).length();
                check_and_track(
                    ProofResidualKind::MeshPosition,
                    residual,
                    expected.length(),
                    actual.length(),
                    tol,
                    &mut proof.mesh_position_residual,
                )?;
            }
        }
    }
    Ok(())
}

/// Double-cover-aware component distance between two quaternion *values*,
/// computed in `f64`.
///
/// `q` and `-q` denote the same rotation, so the residual is the smaller of
/// the two component distances. Deliberately not an angle: nothing here
/// normalizes, divides, or takes an inverse cosine, so an authored
/// quaternion whose magnitude is not exactly one — which `invariant-9`
/// requires loaders to preserve — compares equal to an untouched copy of
/// itself at exactly `0.0` rather than at a magnitude-dependent artefact.
fn quat_equality_residual(before: Quat, after: Quat) -> f64 {
    let before = before.as_dquat();
    let after = after.as_dquat();
    (before - after).length().min((before + after).length())
}

/// Convert the chord length [`quat_equality_residual`] reports into the
/// shortest-path rotation angle it represents, in radians.
///
/// For unit quaternions `q1 . q2 = cos(theta / 2)`, so
/// `|q1 - q2|^2 = 2 - 2 * cos(theta / 2) = 4 * sin(theta / 4)^2` and the
/// chord is `2 * sin(theta / 4)`, which is `theta / 2` to first order.
/// Comparing that chord directly against
/// [`ScaleTolerancePolicy::rotation_residual_radians`] therefore accepted
/// *twice* the declared angle: a genuine `2e-5 rad` error measured
/// `9.99e-6` against a `1e-5` policy and passed. Inverting the relation
/// gives `theta = 4 * asin(chord / 2)`.
///
/// Converting here — rather than comparing in chord space, or renaming the
/// reported field to say "chord" — is the choice that keeps the public
/// evidence contract honest. DESIGN.md Appendix D §D.1 declares the bound as
/// "shortest-path rotation residual is at most `1e-5` radians", §D.6 requires
/// evidence to publish the tolerance policy *and* the observed residuals
/// together, and [`ScaleProof::rest_rotation_residual`] is headed for the
/// immutable evidence format. A chord-valued residual sitting next to a
/// radian-valued policy in that record would hand every reader the same
/// factor-of-two misreading this conversion removes. Converting once, up
/// front, also keeps the comparison, the tracked maximum, and
/// [`ScaleError::ProofResidualExceeded`]'s `observed`/`tolerance` pair all in
/// one unit.
///
/// The conversion is monotone over the whole reachable chord range, so it
/// changes which residuals are accepted only by the intended factor of two,
/// never by re-ordering them. The clamp covers a chord above `2`, which no
/// pair of unit quaternions can produce (the double-cover minimum is at most
/// `sqrt(2)`) but an authored non-unit value can: saturating at `2 * pi`
/// fails closed on such a pair instead of reporting a `NaN` that only the
/// non-finite guard in [`check_residual`] would catch.
fn quat_residual_radians(chord: f64) -> f64 {
    4.0 * (chord / 2.0).min(1.0).asin()
}

fn matrix_residual(before: Mat4, after: Mat4) -> f64 {
    before
        .to_cols_array()
        .into_iter()
        .zip(after.to_cols_array())
        .map(|(b, a)| (b as f64 - a as f64).abs())
        .fold(0.0, f64::max)
}

fn matrix_magnitude(matrix: Mat4) -> f64 {
    matrix
        .to_cols_array()
        .into_iter()
        .fold(0.0f64, |largest, component| {
            largest.max(component.abs() as f64)
        })
}

/// Harvest the times every sampled obligation is evaluated at: every key
/// time of every animated track on an affected bone, plus the analytic
/// mid-segment interior of each cubic segment.
///
/// Deliberately *not* restricted to translation tracks. The sampled
/// obligations these times feed — trajectories, the skin equation, and
/// bounds — depend on a node's complete animated pose, so a clip that
/// animates an affected joint's rotation but not its translation would
/// otherwise yield zero sample times and make every sampled obligation
/// vacuously true while still reporting success.
fn clip_sample_times(clip: &Clip, affected: &BTreeSet<BoneId>) -> (Vec<f32>, Vec<f32>) {
    let mut keys = Vec::new();
    let mut interiors = Vec::new();
    for track in &clip.tracks {
        if !affected.contains(&track.bone) {
            continue;
        }
        keys.extend_from_slice(&track.times);
        if track.interpolation == Interpolation::CubicSpline {
            for window in track.times.windows(2) {
                interiors.push((window[0] + window[1]) * 0.5);
            }
        }
    }
    keys.sort_by(f32::total_cmp);
    keys.dedup();
    interiors.sort_by(f32::total_cmp);
    interiors.dedup();
    (keys, interiors)
}

#[allow(clippy::too_many_arguments)]
fn check_track_value_residual(
    kind: ProofResidualKind,
    source: &Document,
    source_clip: &Clip,
    candidate_clip: &Clip,
    affected: &BTreeSet<BoneId>,
    t: f32,
    plan: &ScalePlan,
    tol: &ScaleTolerancePolicy,
    running_max: &mut f64,
) -> Result<(), ScaleError> {
    // Paired positionally, not by a `(bone, property)` lookup:
    // `validate_candidate_structure` already established that `source_clip`
    // and `candidate_clip` have the same track count and each pair agrees on
    // `(bone, property)`, so a positional pairing cannot silently match the
    // wrong duplicate the way a `find` could.
    for (track, candidate_track) in source_clip.tracks.iter().zip(candidate_clip.tracks.iter()) {
        if track.property != Property::Translation || !affected.contains(&track.bone) {
            continue;
        }
        let multiplier = translation_multiplier(source, track.bone, affected, plan);
        let TrackSample::Vec3(before) = sample_track(track, t) else {
            return Err(ScaleError::MissingProofEvidence {
                kind,
                detail: "source_sample_not_vec3",
            });
        };
        let TrackSample::Vec3(after) = sample_track(candidate_track, t) else {
            return Err(ScaleError::MissingProofEvidence {
                kind,
                detail: "candidate_sample_not_vec3",
            });
        };
        let expected = before.as_dvec3() * multiplier;
        let actual = after.as_dvec3();
        let residual = (actual - expected).length();
        check_and_track(
            kind,
            residual,
            expected.length(),
            actual.length(),
            tol,
            running_max,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn check_trajectory_residual_at(
    source: &Document,
    candidate: &Document,
    source_clip: &Clip,
    candidate_clip: &Clip,
    affected_nodes: &[BoneId],
    t: f32,
    plan: &ScalePlan,
    tol: &ScaleTolerancePolicy,
    running_max: &mut f64,
) -> Result<(), ScaleError> {
    let source_worlds = world_at_time(&source.skeleton, source_clip, t)?;
    let candidate_worlds = world_at_time(&candidate.skeleton, candidate_clip, t)?;
    for &node in affected_nodes {
        let before = *source_worlds
            .get(node)
            .ok_or(ScaleError::BoneIndexOutOfRange { index: node })?;
        let after = *candidate_worlds
            .get(node)
            .ok_or(ScaleError::BoneIndexOutOfRange { index: node })?;
        let (translation_residual, before_mag, after_mag) =
            rest_node_residual(before, after, plan.is_whole_document(), plan.common_factor);
        check_and_track(
            ProofResidualKind::Trajectory,
            translation_residual,
            before_mag,
            after_mag,
            tol,
            running_max,
        )?;
    }
    Ok(())
}

/// Sample `clip` at `t` and compose parent-before-child world matrices,
/// validating every input before it is indexed or accumulated: an
/// out-of-range track bone rejects rather than being skipped, a non-finite
/// sampled value or accumulated matrix rejects, and a parent index that is
/// not strictly earlier than its child rejects — the same structural
/// invariant [`world_rest_matrices`](crate::model::world_rest_matrices)
/// enforces for the unanimated rest pose.
fn world_at_time(skeleton: &Skeleton, clip: &Clip, t: f32) -> Result<Vec<Mat4>, ScaleError> {
    let bone_count = skeleton.bones.len();
    let mut locals = vec![Transform::IDENTITY; bone_count];
    for (index, bone) in skeleton.bones.iter().enumerate() {
        locals[index] = bone.rest;
    }
    for track in &clip.tracks {
        if track.bone >= bone_count {
            return Err(ScaleError::BoneIndexOutOfRange { index: track.bone });
        }
        match sample_track(track, t) {
            TrackSample::Vec3(value) => {
                if !value.is_finite() {
                    return Err(ScaleError::NonFiniteTransform { node: track.bone });
                }
                match track.property {
                    Property::Translation => locals[track.bone].translation = value,
                    Property::Scale => locals[track.bone].scale = value,
                    Property::Rotation => {}
                }
            }
            TrackSample::Quat(value) => {
                if !value.is_finite() {
                    return Err(ScaleError::NonFiniteTransform { node: track.bone });
                }
                locals[track.bone].rotation = value;
            }
        }
    }
    let mut worlds = vec![Mat4::IDENTITY; bone_count];
    for (index, bone) in skeleton.bones.iter().enumerate() {
        let local = locals[index].to_mat4();
        if !mat4_is_finite(local) {
            return Err(ScaleError::NonFiniteTransform { node: index });
        }
        worlds[index] = match bone.parent {
            Some(parent) if parent < index => worlds[parent] * local,
            Some(parent) => {
                return Err(ScaleError::InvalidParent {
                    node: index,
                    parent,
                });
            }
            None => local,
        };
        if !mat4_is_finite(worlds[index]) {
            return Err(ScaleError::NonFiniteTransform { node: index });
        }
    }
    Ok(worlds)
}

#[allow(clippy::too_many_arguments)]
fn check_skin_matrices(
    source: &Document,
    candidate: &Document,
    source_worlds: &[Mat4],
    candidate_worlds: &[Mat4],
    affected: &BTreeSet<BoneId>,
    plan: &ScalePlan,
    tol: &ScaleTolerancePolicy,
    running_max: &mut f64,
) -> Result<(), ScaleError> {
    for (instance_index, instance) in source.assets.instances.iter().enumerate() {
        if instance.skin_joints.is_empty()
            || !instance
                .skin_joints
                .iter()
                .any(|joint| affected.contains(joint))
        {
            continue;
        }
        let candidate_instance = candidate.assets.instances.get(instance_index).ok_or(
            ScaleError::MissingProofEvidence {
                kind: ProofResidualKind::SkinMatrix,
                detail: "candidate_instance_missing",
            },
        )?;
        for (slot, &joint) in instance.skin_joints.iter().enumerate() {
            let before_world = *source_worlds
                .get(joint)
                .ok_or(ScaleError::BoneIndexOutOfRange { index: joint })?;
            let after_world = *candidate_worlds
                .get(joint)
                .ok_or(ScaleError::BoneIndexOutOfRange { index: joint })?;
            let before_ibm = instance_bind(source, instance, slot, joint)?;
            let after_ibm = instance_bind(candidate, candidate_instance, slot, joint)?;
            let before = before_world * before_ibm;
            let after = after_world * after_ibm;
            // Whole-document conversion scales every affine's translation by
            // the declared factor while leaving its linear part unchanged
            // (the same `U M U^-1` conjugation as any other retained
            // matrix); rest/bind reparameterization analytically preserves
            // the skin equation exactly.
            let expected = if plan.is_whole_document() {
                scale_translation_only(before, plan.common_factor as f32)
            } else {
                before
            };
            let residual = matrix_residual(expected, after);
            check_and_track(
                ProofResidualKind::SkinMatrix,
                residual,
                matrix_magnitude(expected),
                matrix_magnitude(after),
                tol,
                running_max,
            )?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn check_bounds(
    source: &Document,
    candidate: &Document,
    source_worlds: &[Mat4],
    candidate_worlds: &[Mat4],
    affected: &BTreeSet<BoneId>,
    plan: &ScalePlan,
    tol: &ScaleTolerancePolicy,
    running_max: &mut f64,
) -> Result<(), ScaleError> {
    // A plan only declares `prove_bounds` when its source actually carried a
    // skinned instance touching the affected closure, so reaching this
    // function with none means the document handed to `prove_scale` is not
    // the one the plan was computed against. Failing closed here matches how
    // every other missing-counterpart branch behaves (`source_bounds_missing`
    // below, `candidate_clip_missing` in `prove_scale`); returning `Ok`
    // instead would report a `0.0` bounds residual for a claim that was
    // never checked.
    if !has_skinned_evidence(source, affected) {
        return Err(ScaleError::MissingProofEvidence {
            kind: ProofResidualKind::Bounds,
            detail: "no_skinned_instance_in_affected_closure",
        });
    }
    let (before_min, before_max) = skinned_bounds(source, source_worlds, affected)?.ok_or(
        ScaleError::MissingProofEvidence {
            kind: ProofResidualKind::Bounds,
            detail: "source_bounds_missing",
        },
    )?;
    let (after_min, after_max) = skinned_bounds(candidate, candidate_worlds, affected)?.ok_or(
        ScaleError::MissingProofEvidence {
            kind: ProofResidualKind::Bounds,
            detail: "candidate_bounds_missing",
        },
    )?;
    let q = if plan.is_whole_document() {
        plan.common_factor
    } else {
        1.0
    };
    for (before, after) in [(before_min, after_min), (before_max, after_max)] {
        let before = before.to_array();
        let after = after.to_array();
        for axis in 0..3 {
            let b = before[axis] as f64;
            let a = after[axis] as f64;
            let expected = b * q;
            let residual = (a - expected).abs();
            check_and_track(
                ProofResidualKind::Bounds,
                residual,
                expected,
                a,
                tol,
                running_max,
            )?;
        }
    }
    Ok(())
}

/// Whether `document` carries at least one skinned mesh instance with a
/// joint inside `affected` — the only evidence a bounds obligation can be
/// checked against.
fn has_skinned_evidence(document: &Document, affected: &BTreeSet<BoneId>) -> bool {
    document.assets.instances.iter().any(|instance| {
        instance
            .skin_joints
            .iter()
            .any(|joint| affected.contains(joint))
    })
}

/// Compute skinned bounds for every instance whose `skin_joints` touches
/// `affected`, rejecting rather than skipping every malformed input along
/// the way: an out-of-range `instance.mesh`, a primitive whose per-vertex
/// `joints`/`weights` are not exactly parallel to `positions`, a non-finite
/// position or weight, a joint-influence slot outside the instance's
/// `skin_joints`, or a non-finite skinned result. A vertex whose four
/// weights are all zero is legitimately unweighted (not malformed) and is
/// excluded from bounds as before.
fn skinned_bounds(
    document: &Document,
    worlds: &[Mat4],
    affected: &BTreeSet<BoneId>,
) -> Result<Option<(Vec3, Vec3)>, ScaleError> {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    let mut touched = false;
    for (instance_index, instance) in document.assets.instances.iter().enumerate() {
        if instance.skin_joints.is_empty()
            || !instance
                .skin_joints
                .iter()
                .any(|joint| affected.contains(joint))
        {
            continue;
        }
        let mesh =
            document
                .assets
                .meshes
                .get(instance.mesh)
                .ok_or(ScaleError::InvalidMeshInstance {
                    instance_index,
                    reason: "mesh_index_out_of_range",
                })?;
        let mut joint_matrices = Vec::with_capacity(instance.skin_joints.len());
        let mut inverse_binds = Vec::with_capacity(instance.skin_joints.len());
        for (slot, &joint) in instance.skin_joints.iter().enumerate() {
            let world = *worlds
                .get(joint)
                .ok_or(ScaleError::BoneIndexOutOfRange { index: joint })?;
            joint_matrices.push(world);
            inverse_binds.push(instance_bind(document, instance, slot, joint)?);
        }
        for (primitive_index, primitive) in mesh.primitives.iter().enumerate() {
            if primitive.joints.len() != primitive.positions.len()
                || primitive.weights.len() != primitive.positions.len()
            {
                return Err(ScaleError::InvalidSkinnedPrimitive {
                    instance_index,
                    primitive_index,
                    reason: "joints_or_weights_length_mismatch",
                });
            }
            for (vertex, &position) in primitive.positions.iter().enumerate() {
                if !position.is_finite() {
                    return Err(ScaleError::InvalidSkinnedPrimitive {
                        instance_index,
                        primitive_index,
                        reason: "non_finite_position",
                    });
                }
                let joints = primitive.joints[vertex];
                let weights = primitive.weights[vertex];
                let mut skinned = Vec3::ZERO;
                let mut weight_sum = 0.0f32;
                for slot_index in 0..4 {
                    let weight = weights[slot_index];
                    if weight == 0.0 {
                        continue;
                    }
                    if !weight.is_finite() {
                        return Err(ScaleError::InvalidSkinnedPrimitive {
                            instance_index,
                            primitive_index,
                            reason: "non_finite_weight",
                        });
                    }
                    let slot = joints[slot_index] as usize;
                    let (Some(&joint_matrix), Some(&inverse_bind)) =
                        (joint_matrices.get(slot), inverse_binds.get(slot))
                    else {
                        return Err(ScaleError::InvalidSkinnedPrimitive {
                            instance_index,
                            primitive_index,
                            reason: "joint_influence_slot_out_of_range",
                        });
                    };
                    skinned += weight * (joint_matrix * inverse_bind).transform_point3(position);
                    weight_sum += weight;
                }
                if weight_sum > 0.0 {
                    skinned /= weight_sum;
                    if !skinned.is_finite() {
                        return Err(ScaleError::InvalidSkinnedPrimitive {
                            instance_index,
                            primitive_index,
                            reason: "non_finite_result",
                        });
                    }
                    min = min.min(skinned);
                    max = max.max(skinned);
                    touched = true;
                }
            }
        }
    }
    Ok(touched.then_some((min, max)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        Bone, MeshAsset, MeshInstance, Primitive, SceneAssets, SourceInverseBindAccessor,
        SourceNodeAsset, SourceNodeLocalRest, SourceSkeletonAssets, SourceSkeletonCoverage,
        SourceSkinAsset, SourceSkinAttachment, Track,
    };
    use glam::{Quat, Vec3};

    /// One node in a test rig, in ascending [`BoneId`] order (`nodes[i].bone
    /// == i`): building both the normalized [`Skeleton`] and the format-neutral
    /// `source_skeleton` from one list keeps them consistent by construction.
    struct RigNode {
        parent: Option<BoneId>,
        source_node_index: usize,
        translation: Vec3,
        rotation: Quat,
        scale: Vec3,
    }

    fn rig(parent: Option<BoneId>, source_node_index: usize, translation: Vec3) -> RigNode {
        RigNode {
            parent,
            source_node_index,
            translation,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }

    /// Build a `Document` with a skinned rig from `nodes`, plus a matching
    /// `assets.source_skeleton` projection (possibly using different source
    /// node numbering than bone-id order) and one skin selecting `skin_bones`.
    fn rig_document(
        nodes: &[RigNode],
        skin_bones: &[BoneId],
        skin_source_index: usize,
        ibm: Mat4,
    ) -> Document {
        let bones: Vec<Bone> = nodes
            .iter()
            .enumerate()
            .map(|(id, n)| Bone {
                name: format!("bone{id}"),
                parent: n.parent,
                rest: Transform {
                    translation: n.translation,
                    rotation: n.rotation,
                    scale: n.scale,
                },
                inverse_bind: None,
            })
            .collect();
        let source_nodes: Vec<SourceNodeAsset> = nodes
            .iter()
            .enumerate()
            .map(|(id, n)| SourceNodeAsset {
                source_node_index: n.source_node_index,
                name: None,
                parent_source_node_index: n.parent.map(|p| nodes[p].source_node_index),
                scene_root_indices: if n.parent.is_none() { vec![0] } else { vec![] },
                local_rest: SourceNodeLocalRest::Trs {
                    translation: n.translation,
                    rotation: n.rotation,
                    scale: n.scale,
                },
                bone: Some(id),
            })
            .collect();
        let joint_source_node_indices: Vec<usize> = skin_bones
            .iter()
            .map(|&b| nodes[b].source_node_index)
            .collect();
        let mesh_owner_source_index =
            nodes[*skin_bones.last().expect("at least one joint")].source_node_index;

        Document {
            skeleton: Skeleton { bones },
            clips: Vec::new(),
            assets: SceneAssets {
                meshes: vec![MeshAsset {
                    name: "mesh".into(),
                    source_mesh_index: 0,
                    primitives: vec![Primitive {
                        positions: vec![Vec3::new(1.0, 0.0, 0.0)],
                        joints: vec![[0, 0, 0, 0]],
                        weights: vec![[1.0, 0.0, 0.0, 0.0]],
                        ..Primitive::default()
                    }],
                }],
                instances: vec![MeshInstance {
                    source_node_index: mesh_owner_source_index,
                    node: skin_bones[0],
                    mesh: 0,
                    skin_joints: skin_bones.to_vec(),
                    skin_ibms: vec![ibm; skin_bones.len()],
                }],
                source_skeleton: SourceSkeletonAssets {
                    coverage: SourceSkeletonCoverage::Complete,
                    nodes: source_nodes,
                    skins: vec![SourceSkinAsset {
                        source_skin_index: skin_source_index,
                        name: None,
                        skeleton_root_source_node_index: None,
                        joint_source_node_indices,
                        inverse_bind_accessor: SourceInverseBindAccessor::default(),
                        attachments: Vec::new(),
                    }],
                },
                ..SceneAssets::default()
            },
            source: Default::default(),
        }
    }

    fn complete_capability() -> ScaleCapabilityFacts {
        ScaleCapabilityFacts {
            coverage: ScaleCapabilityCoverage::Complete,
            ..Default::default()
        }
    }

    fn unit_rig() -> Vec<RigNode> {
        vec![
            rig(None, 0, Vec3::ZERO),
            rig(Some(0), 1, Vec3::new(0.0, 1.0, 0.0)),
        ]
    }

    // --- Whole-document conversion ------------------------------------

    #[test]
    fn whole_document_factor_one_is_a_literal_no_op() {
        let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        let capability = complete_capability();
        let request = ScaleRequest {
            operation: ScaleOperation::WholeDocumentLinearUnits { factor: 1.0 },
            document: &doc,
            capability: &capability,
        };
        let plan = plan_scale(&request).unwrap();
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        assert_eq!(
            candidate.document().skeleton.bones[1].rest.translation,
            Vec3::new(0.0, 1.0, 0.0)
        );
        let proof = prove_scale(&doc, &candidate, &plan).unwrap();
        assert!(proof.rest_translation_residual < 1e-9);
        assert!(proof.bounds_residual < 1e-6);
    }

    #[test]
    fn whole_document_conversion_scales_translation_mesh_and_ibm() {
        let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        let capability = complete_capability();
        let request = ScaleRequest {
            operation: ScaleOperation::WholeDocumentLinearUnits { factor: 0.01 },
            document: &doc,
            capability: &capability,
        };
        let plan = plan_scale(&request).unwrap();
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let child = &candidate.document().skeleton.bones[1];
        assert!((child.rest.translation - Vec3::new(0.0, 0.01, 0.0)).length() < 1e-6);
        assert_eq!(child.rest.scale, Vec3::ONE);
        let mesh_position = candidate.document().assets.meshes[0].primitives[0].positions[0];
        assert!((mesh_position - Vec3::new(0.01, 0.0, 0.0)).length() < 1e-6);
        let ibm = candidate.document().assets.instances[0].skin_ibms[0];
        assert!(ibm.w_axis.abs_diff_eq(Vec4::new(0.0, 0.0, 0.0, 1.0), 1e-6));
        let proof = prove_scale(&doc, &candidate, &plan).unwrap();
        assert!(proof.bounds_residual < 1e-6);
    }

    #[test]
    fn whole_document_conversion_scales_translation_track_values_and_cubic_tangents() {
        let mut doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        doc.clips.push(Clip {
            name: "clip".into(),
            duration_s: 1.0,
            tracks: vec![Track {
                bone: 1,
                property: Property::Translation,
                interpolation: Interpolation::CubicSpline,
                times: vec![0.0, 1.0],
                values: TrackValues::Vec3s(vec![
                    Vec3::new(0.0, -1.0, 0.0), // in-tangent @0
                    Vec3::new(0.0, 1.0, 0.0),  // value @0
                    Vec3::new(0.0, 1.0, 0.0),  // out-tangent @0
                    Vec3::new(0.0, -2.0, 0.0), // in-tangent @1
                    Vec3::new(0.0, 2.0, 0.0),  // value @1
                    Vec3::new(0.0, 2.0, 0.0),  // out-tangent @1
                ]),
            }],
        });
        let capability = complete_capability();
        let request = ScaleRequest {
            operation: ScaleOperation::WholeDocumentLinearUnits { factor: 0.01 },
            document: &doc,
            capability: &capability,
        };
        let plan = plan_scale(&request).unwrap();
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let TrackValues::Vec3s(values) = &candidate.document().clips[0].tracks[0].values else {
            panic!("expected vec3 track");
        };
        let expected: Vec<Vec3> = [-1.0, 1.0, 1.0, -2.0, 2.0, 2.0]
            .into_iter()
            .map(|y: f32| Vec3::new(0.0, y * 0.01, 0.0))
            .collect();
        for (value, expected) in values.iter().zip(expected) {
            assert!((*value - expected).length() < 1e-6);
        }
        let proof = prove_scale(&doc, &candidate, &plan).unwrap();
        assert!(proof.cubic_interior_residual < 1e-4);
        assert!(proof.trajectory_residual < 1e-4);
        assert!(proof.sample_time_count > 0);
    }

    // --- Rest/bind selector resolution ----------------------------------

    #[test]
    fn rest_bind_resolves_shuffled_source_selectors_to_the_correct_bone_closure() {
        // Source-node and source-skin numbering deliberately disagrees with
        // bone-id order: this is the fixture that actually exercises source
        // selector resolution rather than assuming source order == bone order.
        let nodes = vec![
            rig(None, 7, Vec3::ZERO),                  // bone 0 (root), source 7
            rig(Some(0), 2, Vec3::new(0.0, 1.0, 0.0)), // bone 1 (joint), source 2
        ];
        let doc = rig_document(&nodes, &[1], 42, Mat4::IDENTITY);
        let capability = complete_capability();
        let request = ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 42,
                source_root_node_index: 7,
                expected_factor: 1.0,
            },
            document: &doc,
            capability: &capability,
        };
        let plan = plan_scale(&request).unwrap();
        assert_eq!(plan.affected_nodes(), &[0, 1]);
    }

    #[test]
    fn rest_bind_factor_one_on_unit_rig_is_a_deterministic_no_op() {
        let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        let capability = complete_capability();
        let request = ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 1.0,
            },
            document: &doc,
            capability: &capability,
        };
        let plan = plan_scale(&request).unwrap();
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        assert_eq!(
            candidate.document().skeleton.bones[1].rest.translation,
            Vec3::new(0.0, 1.0, 0.0)
        );
        let proof = prove_scale(&doc, &candidate, &plan).unwrap();
        assert!(proof.rest_translation_residual < 1e-9);
    }

    #[test]
    fn rest_bind_requesting_a_different_factor_on_unit_rig_rejects_as_factor_mismatch() {
        let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        let capability = complete_capability();
        let request = ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 0.5,
            },
            document: &doc,
            capability: &capability,
        };
        let error = plan_scale(&request).unwrap_err();
        assert!(matches!(error, ScaleError::FactorMismatch { .. }));
    }

    // --- Compensated inherited scale + transform-only attachment --------

    fn compensated_rig() -> Vec<RigNode> {
        vec![
            RigNode {
                parent: None,
                source_node_index: 0,
                translation: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                scale: Vec3::splat(0.01),
            },
            RigNode {
                parent: Some(0),
                source_node_index: 1,
                translation: Vec3::new(0.0, 100.0, 0.0),
                // A non-identity rotation on the path to the transform-only
                // attachment: DESIGN.md Appendix D §D.3 case 2 requires this
                // so a translation+rotation-only proof cannot mistake a
                // no-op for a correct rebase.
                rotation: Quat::from_rotation_y(0.2),
                scale: Vec3::ONE,
            },
            rig(Some(1), 2, Vec3::new(1.0, 0.0, 0.0)),
        ]
    }

    fn compensated_document() -> Document {
        let nodes = compensated_rig();
        let child_world = Mat4::from_scale_rotation_translation(
            nodes[0].scale,
            nodes[1].rotation,
            Vec3::new(0.0, 1.0, 0.0),
        );
        let ibm = child_world.inverse();
        rig_document(&nodes, &[1], 0, ibm)
    }

    #[test]
    fn compensated_inherited_scale_reparameterizes_and_preserves_world_geometry() {
        let doc = compensated_document();
        let capability = complete_capability();
        let request = ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 0.01,
            },
            document: &doc,
            capability: &capability,
        };
        let plan = plan_scale(&request).unwrap();
        assert_eq!(plan.affected_nodes(), &[0, 1, 2]);
        assert_eq!(plan.transform_only_attachments(), &[2]);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let bones = &candidate.document().skeleton.bones;
        assert!((bones[0].rest.scale - Vec3::ONE).length() < 1e-6);
        assert!((bones[1].rest.translation - Vec3::new(0.0, 1.0, 0.0)).length() < 1e-6);
        // Transform-only attachment: rebased from (1,0,0) to (0.01,0,0).
        assert!((bones[2].rest.translation - Vec3::new(0.01, 0.0, 0.0)).length() < 1e-6);

        let proof = prove_scale(&doc, &candidate, &plan).unwrap();
        assert!(proof.rest_translation_residual < 1e-4);
        assert!(proof.unit_scale_residual < 1e-4);
        assert!(proof.transform_only_affine_residual < 1e-4);
        assert!(proof.skin_matrix_residual < 1e-3);
    }

    #[test]
    fn a_stale_no_op_candidate_for_the_transform_only_attachment_fails_proof() {
        let doc = compensated_document();
        let capability = complete_capability();
        let request = ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 0.01,
            },
            document: &doc,
            capability: &capability,
        };
        let plan = plan_scale(&request).unwrap();
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let mut broken = candidate.document().clone();
        // Simulate a builder bug that left the transform-only attachment's
        // local translation un-rebased.
        broken.skeleton.bones[2].rest.translation = Vec3::new(1.0, 0.0, 0.0);
        let broken = ScaleCandidate { document: broken };
        assert!(prove_scale(&doc, &broken, &plan).is_err());
    }

    // --- A closure that genuinely branches -------------------------------

    /// A rest/bind rig whose affected closure *branches*, at two depths.
    ///
    /// Every other rig in this module is a single chain: no domain node has
    /// more than one child, so a descendant walk that followed only each
    /// node's *first* child would still produce the correct closure for all
    /// of them, and the traversal's breadth goes entirely unpinned.
    ///
    /// ```text
    /// bone 0  source 0  parent -  T (0, 0, 0)     S 0.01  scaled root
    /// bone 1  source 1  parent 0  T (0, 100, 0)   S 1     the skin's only joint
    /// bone 2  source 2  parent 0  T (100, 0, 0)   S 1     root's SECOND child
    /// bone 3  source 3  parent 1  T (0, 0, 100)   S 1     joint's first child
    /// bone 4  source 4  parent 1  T (0, 50, 0)    S 1     joint's SECOND child
    /// bone 5  source 5  parent 2  T (0, 0, 50)    S 1     child of bone 2 only
    /// ```
    ///
    /// Bones 2, 4 and 5 are reachable *only* through a second-or-later
    /// child: none of them is a skin joint, and none lies on a joint's
    /// ancestor path, so neither the root insertion nor the joint
    /// ancestor walk can pull them in — only the descendant walk can, and
    /// only if it visits more than one child per node. Bone 5 additionally
    /// hangs below a second child, so it is reachable only after the walk
    /// has already branched once.
    ///
    /// Hand-computed rest-world matrices — linear part `0.01 * I`
    /// throughout, so the whole domain classifies at the common factor
    /// `0.01`:
    ///
    /// ```text
    /// W0 (0, 0, 0)   W1 (0, 1, 0)     W2 (1, 0, 0)
    /// W3 (0, 1, 1)   W4 (0, 1.5, 0)   W5 (1, 0, 0.5)
    /// ```
    fn branching_rig() -> Vec<RigNode> {
        vec![
            RigNode {
                parent: None,
                source_node_index: 0,
                translation: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                scale: Vec3::splat(0.01),
            },
            rig(Some(0), 1, Vec3::new(0.0, 100.0, 0.0)),
            rig(Some(0), 2, Vec3::new(100.0, 0.0, 0.0)),
            rig(Some(1), 3, Vec3::new(0.0, 0.0, 100.0)),
            rig(Some(1), 4, Vec3::new(0.0, 50.0, 0.0)),
            rig(Some(2), 5, Vec3::new(0.0, 0.0, 50.0)),
        ]
    }

    fn branching_document() -> Document {
        // `B1 = inverse(W1) = inverse([0.01 I | (0, 1, 0)]) = [100 I | (0, -100, 0)]`,
        // written as a literal rather than derived from the fixture, so
        // `W1 * B1 = I` is a hand-checked fact of the source.
        let ibm = Mat4::from_cols(
            Vec4::new(100.0, 0.0, 0.0, 0.0),
            Vec4::new(0.0, 100.0, 0.0, 0.0),
            Vec4::new(0.0, 0.0, 100.0, 0.0),
            Vec4::new(0.0, -100.0, 0.0, 1.0),
        );
        let mut doc = rig_document(&branching_rig(), &[1], 0, ibm);
        // Animated on the branch that only a second child reaches: bone 4's
        // world translation is `(0, 1, 0) + 0.01 * value`, so the source
        // trajectory runs `(0, 1.5, 0)` -> `(0, 1.6, 0)`.
        doc.clips.push(Clip {
            name: "clip".into(),
            duration_s: 1.0,
            tracks: vec![Track {
                bone: 4,
                property: Property::Translation,
                interpolation: Interpolation::Linear,
                times: vec![0.0, 1.0],
                values: TrackValues::Vec3s(vec![
                    Vec3::new(0.0, 50.0, 0.0),
                    Vec3::new(0.0, 60.0, 0.0),
                ]),
            }],
        });
        doc
    }

    #[test]
    fn a_branching_affected_closure_pulls_in_every_child_at_every_depth() {
        let doc = branching_document();
        let capability = complete_capability();
        let plan = plan_scale(&ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 0.01,
            },
            document: &doc,
            capability: &capability,
        })
        .unwrap();
        // The complete expected closure, as a literal. A walk that follows
        // only a first child yields `[0, 1, 3]`: bone 2 (the root's second
        // child), bone 4 (the joint's second child) and bone 5 (below bone
        // 2) are all missing.
        assert_eq!(plan.affected_nodes(), &[0, 1, 2, 3, 4, 5]);
        // Everything but the scaled root and the skin's one joint.
        assert_eq!(plan.transform_only_attachments(), &[2, 3, 4, 5]);
        assert_eq!(plan.common_factor(), 0.01);

        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let bones = &candidate.document().skeleton.bones;
        // `L' = C_parent^-1 * L * C_i`: every affected local translation is
        // multiplied by its parent's factor (`0.01` inside the domain, one
        // at the root boundary) and every affected local scale becomes one.
        let expected_local = [
            Vec3::ZERO,
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.0, 0.5, 0.0),
            Vec3::new(0.0, 0.0, 0.5),
        ];
        for (node, expected) in expected_local.iter().enumerate() {
            assert!(
                (bones[node].rest.translation - *expected).length() < 1e-6,
                "bone {node} translation {:?}",
                bones[node].rest.translation
            );
            assert!(
                (bones[node].rest.scale - Vec3::ONE).length() < 1e-6,
                "bone {node} scale {:?}",
                bones[node].rest.scale
            );
        }
        // The animated branch node is rebased by its parent's `0.01` too.
        let TrackValues::Vec3s(values) = &candidate.document().clips[0].tracks[0].values else {
            panic!("expected a vec3 track");
        };
        assert!((values[0] - Vec3::new(0.0, 0.5, 0.0)).length() < 1e-6);
        assert!((values[1] - Vec3::new(0.0, 0.6, 0.0)).length() < 1e-6);
        // `B1' = C^-1 * B1 = scale(0.01) * [100 I | (0, -100, 0)]
        //      = [I | (0, -1, 0)]`.
        let rebased_ibm = candidate.document().assets.instances[0].skin_ibms[0];
        assert!(
            rebased_ibm.abs_diff_eq(
                Mat4::from_cols(
                    Vec4::new(1.0, 0.0, 0.0, 0.0),
                    Vec4::new(0.0, 1.0, 0.0, 0.0),
                    Vec4::new(0.0, 0.0, 1.0, 0.0),
                    Vec4::new(0.0, -1.0, 0.0, 1.0),
                ),
                1e-6
            ),
            "rebased inverse bind {rebased_ibm:?}"
        );

        let proof = prove_scale(&doc, &candidate, &plan).unwrap();
        // Two key times, from the one animated branch node.
        assert_eq!(proof.sample_time_count, 2);
        assert!(proof.rest_translation_residual < 1e-6);
        assert!(proof.unit_scale_residual < 1e-6);
        assert!(proof.transform_only_affine_residual < 1e-6);
        assert!(proof.trajectory_residual < 1e-6);
        assert!(proof.key_translation_residual < 1e-6);
        assert!(proof.skin_matrix_residual < 1e-4);
        assert!(proof.bounds_residual < 1e-6);
    }

    // --- The `C_parent = I` boundary at the top of the domain ------------

    /// A rest/bind rig whose scaled root is *not* the skeleton root.
    ///
    /// DESIGN.md Appendix D §D.2 rebases each affected local matrix as
    /// `L' = C_parent^-1 * L * C_i`, where `C_i = scale(1 / s)` inside the
    /// affected domain and `C_parent = I` at its parent boundary. Every other
    /// fixture here scales the skeleton root itself, with a zero local
    /// translation and no track of its own, so `C_parent = I` and
    /// `C_parent = C_i` are indistinguishable and the boundary rule — the
    /// core of the operation — goes unpinned on both the build and the proof
    /// side.
    ///
    /// This rig makes them differ, in one closure:
    ///
    /// ```text
    /// bone 0   parent -   T (5, 0, 0)     S 1      boundary parent, outside
    /// bone 1   parent 0   T (0, 2, 0)     S 0.01   scaled root, animated
    /// bone 2   parent 1   T (0, 100, 0)   S 1      the skin's only joint
    /// bone 3   parent 2   T (100, 0, 0)   S 1      attachment, depth 1
    /// bone 4   parent 3   T (0, 0, 200)   S 1      attachment, depth 2
    /// ```
    ///
    /// Every rest-world linear part from bone 1 down is `0.01 * I`, so the
    /// domain classifies at the common factor `0.01`; bone 0 contributes a
    /// pure translation and is neither a joint, an ancestor path between
    /// joints, nor a descendant, so it stays outside. The rest-world
    /// translations are bone 1 `(5, 2, 0)`, bone 2 `(5, 3, 0)`, bone 3
    /// `(6, 3, 0)` and bone 4 `(6, 3, 2)`.
    fn parent_boundary_rig() -> Vec<RigNode> {
        vec![
            rig(None, 0, Vec3::new(5.0, 0.0, 0.0)),
            RigNode {
                parent: Some(0),
                source_node_index: 1,
                translation: Vec3::new(0.0, 2.0, 0.0),
                rotation: Quat::IDENTITY,
                scale: Vec3::splat(0.01),
            },
            rig(Some(1), 2, Vec3::new(0.0, 100.0, 0.0)),
            rig(Some(2), 3, Vec3::new(100.0, 0.0, 0.0)),
            rig(Some(3), 4, Vec3::new(0.0, 0.0, 200.0)),
        ]
    }

    fn parent_boundary_document() -> Document {
        // `B = inverse(W_rest(bone 2))` for `W = T(5, 3, 0) * scale(0.01)`:
        // `W^-1 = scale(100) * T(-5, -3, 0)`, that is a linear part of
        // `100 * I` and a translation column of `100 * (-5, -3, 0)`.
        let ibm = Mat4::from_scale_rotation_translation(
            Vec3::splat(100.0),
            Quat::IDENTITY,
            Vec3::new(-500.0, -300.0, 0.0),
        );
        let mut doc = rig_document(&parent_boundary_rig(), &[2], 0, ibm);
        // A translation track on the scaled root *itself*. Its parent is
        // outside the closure, so this track's parent-basis multiplier is the
        // boundary factor of one and both values must survive the rebase
        // byte-for-byte — unlike the descendant tracks every other animated
        // fixture here carries, which are rebased by `s`.
        doc.clips.push(Clip {
            name: "clip".into(),
            duration_s: 1.0,
            tracks: vec![Track {
                bone: 1,
                property: Property::Translation,
                interpolation: Interpolation::Linear,
                times: vec![0.0, 1.0],
                values: TrackValues::Vec3s(vec![
                    Vec3::new(0.0, 2.0, 0.0),
                    Vec3::new(0.0, 4.0, 0.0),
                ]),
            }],
        });
        doc
    }

    #[test]
    fn a_scaled_root_whose_parent_is_outside_the_closure_keeps_its_own_translation_basis() {
        let doc = parent_boundary_document();
        let capability = complete_capability();
        let request = ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 1,
                expected_factor: 0.01,
            },
            document: &doc,
            capability: &capability,
        };
        let plan = plan_scale(&request).unwrap();
        // The closure is the scaled root, its one joint, and *both*
        // attachment levels below it: bone 4 is two hops below the deepest
        // node the joint/ancestor seeding reaches, so a descendant walk that
        // stops after one level drops it. Bone 0 is the boundary parent and
        // must stay out.
        assert_eq!(plan.affected_nodes(), &[1, 2, 3, 4]);
        assert_eq!(plan.transform_only_attachments(), &[3, 4]);
        assert_eq!(plan.common_factor(), 0.01);

        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let bones = &candidate.document().skeleton.bones;
        // The boundary parent is not in the domain and is not rewritten.
        assert_eq!(bones[0].rest.translation, Vec3::new(5.0, 0.0, 0.0));
        assert_eq!(bones[0].rest.scale, Vec3::ONE);
        // Scaled root: `C_parent = I`, so its local translation keeps the
        // basis it was authored in — multiplying it by `s` here would move
        // its world origin from `(5, 2, 0)` to `(5, 0.02, 0)` — while its own
        // `C_i` still corrects its local scale from `0.01` to one.
        assert!((bones[1].rest.translation - Vec3::new(0.0, 2.0, 0.0)).length() < 1e-9);
        // Below the root every parent is itself affected, so
        // `C_parent = scale(1 / s)` and each local translation is rebased by
        // `s = 0.01`: `(0, 100, 0) -> (0, 1, 0)`, `(100, 0, 0) -> (1, 0, 0)`,
        // `(0, 0, 200) -> (0, 0, 2)`.
        assert!((bones[2].rest.translation - Vec3::new(0.0, 1.0, 0.0)).length() < 1e-6);
        assert!((bones[3].rest.translation - Vec3::new(1.0, 0.0, 0.0)).length() < 1e-6);
        assert!((bones[4].rest.translation - Vec3::new(0.0, 0.0, 2.0)).length() < 1e-6);
        for (id, bone) in bones.iter().enumerate().skip(1) {
            assert!(
                (bone.rest.scale - Vec3::ONE).length() < 1e-6,
                "bone {id} local scale {:?}",
                bone.rest.scale
            );
        }

        // The raw source projection is rebased by the same rule, and the
        // scaled root's authored local translation is unchanged there too.
        let source_nodes = &candidate.document().assets.source_skeleton.nodes;
        let expected_projection = [
            (0, Vec3::new(5.0, 0.0, 0.0), Vec3::ONE),
            (1, Vec3::new(0.0, 2.0, 0.0), Vec3::ONE),
            (2, Vec3::new(0.0, 1.0, 0.0), Vec3::ONE),
            (3, Vec3::new(1.0, 0.0, 0.0), Vec3::ONE),
            (4, Vec3::new(0.0, 0.0, 2.0), Vec3::ONE),
        ];
        for (index, expected_translation, expected_scale) in expected_projection {
            let SourceNodeLocalRest::Trs {
                translation, scale, ..
            } = &source_nodes[index].local_rest
            else {
                panic!("expected a trs source rest");
            };
            assert!(
                (*translation - expected_translation).length() < 1e-6,
                "source node {index} translation {translation:?}"
            );
            assert!(
                (*scale - expected_scale).length() < 1e-6,
                "source node {index} scale {scale:?}"
            );
        }

        // The scaled root's own translation track is *not* rebased.
        let TrackValues::Vec3s(values) = &candidate.document().clips[0].tracks[0].values else {
            panic!("expected a vec3 track");
        };
        let expected_values = [Vec3::new(0.0, 2.0, 0.0), Vec3::new(0.0, 4.0, 0.0)];
        for (value, expected) in values.iter().zip(expected_values) {
            assert!((*value - expected).length() < 1e-9, "track value {value:?}");
        }

        // `B' = C^-1 * B = scale(s) * B`: linear `I`, translation column
        // `0.01 * (-500, -300, 0)`.
        let binds = &candidate.document().assets.instances[0].skin_ibms;
        assert_eq!(binds.len(), 1);
        assert!(
            binds[0].abs_diff_eq(Mat4::from_translation(Vec3::new(-5.0, -3.0, 0.0)), 1e-5),
            "rebased bind {:?}",
            binds[0]
        );

        let proof = prove_scale(&doc, &candidate, &plan).unwrap();
        // Two key times, no cubic segment.
        assert_eq!(proof.sample_time_count, 2);
        assert!(proof.rest_translation_residual < 1e-4);
        assert!(proof.unit_scale_residual < 1e-4);
        assert!(proof.transform_only_affine_residual < 1e-4);
        // Exactly zero: the one rewritten track's multiplier is one.
        assert!(proof.track_value_residual < 1e-9);
        assert!(proof.trajectory_residual < 1e-4);
        assert!(proof.skin_matrix_residual < 1e-4);
        assert!(proof.bounds_residual < 1e-4);
    }

    // --- Affine violation classes (rest/bind classification) -----------

    /// Two-node rig used for affine-violation fixtures: `mutate` edits the
    /// root's raw source local-rest matrix, which is what classification
    /// now runs against (not the lossy TRS-decomposed `Bone::rest`).
    fn reject_case(mutate: impl FnOnce(&mut SourceNodeLocalRest)) -> ScaleError {
        let nodes = vec![rig(None, 0, Vec3::ZERO), rig(Some(0), 1, Vec3::ZERO)];
        let mut doc = rig_document(&nodes, &[1], 0, Mat4::IDENTITY);
        let root = &mut doc.assets.source_skeleton.nodes[0].local_rest;
        mutate(root);
        let capability = complete_capability();
        let request = ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 0.01,
            },
            document: &doc,
            capability: &capability,
        };
        plan_scale(&request).unwrap_err()
    }

    fn trs_scale(scale: Vec3) -> SourceNodeLocalRest {
        SourceNodeLocalRest::Trs {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale,
        }
    }

    #[test]
    fn nonuniform_scale_domain_rejects() {
        let error = reject_case(|rest| *rest = trs_scale(Vec3::new(0.01, 0.02, 0.01)));
        assert!(matches!(
            error,
            ScaleError::InvalidAffineDomain {
                reason: AffineDomainViolation::NonUniformScale,
                ..
            }
        ));
    }

    #[test]
    fn literal_shear_via_a_raw_matrix_fixture_rejects() {
        // A TRS-only check can never see this: `SourceNodeLocalRest::Matrix`
        // is the only representation that carries a literal shear term. All
        // three columns keep equal length (so the uniform-axis check alone
        // cannot explain the rejection) but the first two are not
        // orthogonal, isolating the shear violation.
        let angle = 80f32.to_radians();
        let error = reject_case(|rest| {
            *rest = SourceNodeLocalRest::Matrix(Mat4::from_cols_array(&[
                1.0,
                0.0,
                0.0,
                0.0,
                angle.cos(),
                angle.sin(),
                0.0,
                0.0,
                0.0,
                0.0,
                1.0,
                0.0,
                0.0,
                0.0,
                0.0,
                1.0,
            ]));
        });
        assert!(matches!(
            error,
            ScaleError::InvalidAffineDomain {
                reason: AffineDomainViolation::Sheared,
                ..
            }
        ));
    }

    #[test]
    fn reflected_domain_rejects() {
        let error = reject_case(|rest| *rest = trs_scale(Vec3::new(-0.01, 0.01, 0.01)));
        assert!(matches!(
            error,
            ScaleError::InvalidAffineDomain {
                reason: AffineDomainViolation::Reflected,
                ..
            }
        ));
    }

    #[test]
    fn singular_domain_rejects() {
        let error = reject_case(|rest| *rest = trs_scale(Vec3::new(0.0, 0.01, 0.01)));
        assert!(matches!(
            error,
            ScaleError::InvalidAffineDomain {
                reason: AffineDomainViolation::Singular,
                ..
            }
        ));
    }

    #[test]
    fn near_singular_domain_rejects() {
        // Equal-length axes (so the uniform-axis check alone would pass)
        // that are nearly parallel: the determinant collapses toward zero
        // while every column stays unit length.
        let eps = 1e-8f32;
        let error = reject_case(|rest| {
            *rest = SourceNodeLocalRest::Matrix(Mat4::from_cols_array(&[
                1.0,
                0.0,
                0.0,
                0.0,
                eps.cos(),
                eps.sin(),
                0.0,
                0.0,
                0.0,
                0.0,
                1.0,
                0.0,
                0.0,
                0.0,
                0.0,
                1.0,
            ]));
        });
        assert!(matches!(
            error,
            ScaleError::InvalidAffineDomain {
                reason: AffineDomainViolation::Singular,
                ..
            }
        ));
    }

    #[test]
    fn nonfinite_domain_rejects() {
        // A non-finite raw local-rest transform is caught composing raw
        // source-node world matrices, before affine classification ever
        // runs.
        let error = reject_case(|rest| *rest = trs_scale(Vec3::new(f32::NAN, 0.01, 0.01)));
        assert!(matches!(
            error,
            ScaleError::NonFiniteSourceTransform {
                source_node_index: 0
            }
        ));
    }

    #[test]
    fn mixed_factor_within_domain_rejects() {
        let nodes = vec![rig(None, 0, Vec3::ZERO), rig(Some(0), 1, Vec3::ZERO)];
        let mut doc = rig_document(&nodes, &[1], 0, Mat4::IDENTITY);
        doc.assets.source_skeleton.nodes[0].local_rest = trs_scale(Vec3::splat(0.01));
        doc.assets.source_skeleton.nodes[1].local_rest = trs_scale(Vec3::splat(0.02));
        let capability = complete_capability();
        let request = ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 0.01,
            },
            document: &doc,
            capability: &capability,
        };
        assert!(matches!(
            plan_scale(&request).unwrap_err(),
            ScaleError::MixedFactor { .. }
        ));
    }

    /// A two-node rig whose root carries `scale` in *both* the normalized
    /// `Bone::rest` and the raw `source_skeleton` projection, so a plan
    /// accepted from the source projection can be carried through
    /// `build_scale_candidate` and `prove_scale` without the two disagreeing.
    fn noisy_factor_document(scale: f32) -> Document {
        let nodes = vec![
            RigNode {
                parent: None,
                source_node_index: 0,
                translation: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                scale: Vec3::splat(scale),
            },
            rig(Some(0), 1, Vec3::ZERO),
        ];
        rig_document(&nodes, &[1], 0, Mat4::IDENTITY)
    }

    fn noisy_factor_request<'a>(
        document: &'a Document,
        capability: &'a ScaleCapabilityFacts,
    ) -> ScaleRequest<'a> {
        ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 0.01,
            },
            document,
            capability,
        }
    }

    #[test]
    fn noisy_but_within_tolerance_factor_is_accepted_and_just_outside_is_not() {
        // DESIGN.md Appendix D §D.3 case 4: a noisy near-factor is accepted
        // only when its measured residual is within the declared tolerance
        // — which §D.1 declares *relative* `1e-5`. The accept/reject deltas
        // are therefore derived from the factor's own magnitude, not from a
        // floored comparison base:
        //
        //   tolerance = 1e-5 * max(observed, 0.01) = 1.0e-7
        //   0.010000020 -> residual 2.03e-8, inside that band
        //   0.010000500 -> residual 5.00e-7, 5x outside it
        for (scale, should_accept) in [(0.010_000_02_f32, true), (0.010_000_5_f32, false)] {
            let doc = noisy_factor_document(scale);
            let capability = complete_capability();
            let request = noisy_factor_request(&doc, &capability);
            assert_eq!(
                plan_scale(&request).is_ok(),
                should_accept,
                "scale {scale} accepted={should_accept}"
            );
        }
    }

    #[test]
    fn a_noisy_factor_plan_scale_accepts_still_satisfies_its_own_proof_postcondition() {
        // The accept side of the band above is only meaningful if the plan
        // it produces is actually buildable and provable. With the earlier
        // `1.0`-floored comparison base this was false: `plan_scale`
        // accepted a factor `8e-6` relative off, whose candidate then failed
        // its own `UnitScale` postcondition at `1.386e-3` against `1e-5`.
        //
        // Arithmetic for the value below: the root's composed scale after
        // the rebase is `observed / expected = 0.010000020 / 0.01`, so the
        // unit-scale residual is `sqrt(3) * 2.03e-6 = 3.51e-6`, inside the
        // `1e-5` postcondition.
        let doc = noisy_factor_document(0.010_000_02);
        let capability = complete_capability();
        let request = noisy_factor_request(&doc, &capability);
        let plan = plan_scale(&request).unwrap();
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let proof = prove_scale(&doc, &candidate, &plan).unwrap();
        assert!(
            proof.unit_scale_residual < 4e-6,
            "unit scale residual {}",
            proof.unit_scale_residual
        );
    }

    #[test]
    fn equal_axis_uniformity_is_relative_to_the_authored_magnitude() {
        // `(0.01, 0.01, 0.010005)` is `5e-4` relative non-uniform — 50x the
        // declared `1e-5` equal-axis tolerance — and must classify as
        // non-uniform. A comparison base floored at `1.0` would instead read
        // the `5e-6` absolute spread as uniform.
        let error = reject_case(|rest| *rest = trs_scale(Vec3::new(0.01, 0.01, 0.010005)));
        assert!(
            matches!(
                error,
                ScaleError::InvalidAffineDomain {
                    reason: AffineDomainViolation::NonUniformScale,
                    ..
                }
            ),
            "unexpected error {error:?}"
        );
    }

    #[test]
    fn a_tiny_expected_factor_does_not_pass_the_common_factor_check_by_absolute_luck() {
        // Source factor `1e-6` against a declared `1e-30`: the two differ by
        // 24 orders of magnitude, which a comparison base floored at `1.0`
        // read as an absolute difference of `1e-6` and accepted.
        let doc = noisy_factor_document(1e-6);
        let capability = complete_capability();
        let request = ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 1e-30,
            },
            document: &doc,
            capability: &capability,
        };
        assert!(matches!(
            plan_scale(&request).unwrap_err(),
            ScaleError::FactorMismatch { .. }
        ));
    }

    // --- Closure and selector rejections --------------------------------

    #[test]
    fn incomplete_capability_rejects_before_geometry_is_inspected() {
        let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        let capability = ScaleCapabilityFacts::default();
        let request = ScaleRequest {
            operation: ScaleOperation::WholeDocumentLinearUnits { factor: 1.0 },
            document: &doc,
            capability: &capability,
        };
        assert!(matches!(
            plan_scale(&request).unwrap_err(),
            ScaleError::IncompleteCapability
        ));
    }

    #[test]
    fn incomplete_source_skeleton_coverage_rejects() {
        let mut doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        doc.assets.source_skeleton.coverage = SourceSkeletonCoverage::Unavailable;
        let capability = complete_capability();
        let request = ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 1.0,
            },
            document: &doc,
            capability: &capability,
        };
        assert!(matches!(
            plan_scale(&request).unwrap_err(),
            ScaleError::IncompleteSourceSkeleton
        ));
    }

    #[test]
    fn invalid_factor_rejects() {
        let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        let capability = complete_capability();
        for factor in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            let request = ScaleRequest {
                operation: ScaleOperation::WholeDocumentLinearUnits { factor },
                document: &doc,
                capability: &capability,
            };
            assert!(matches!(
                plan_scale(&request).unwrap_err(),
                ScaleError::InvalidFactor { .. }
            ));
        }
    }

    #[test]
    fn invalid_source_selectors_reject_without_panicking() {
        let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        let capability = complete_capability();
        let bad_root = ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 99,
                expected_factor: 0.01,
            },
            document: &doc,
            capability: &capability,
        };
        assert!(matches!(
            plan_scale(&bad_root).unwrap_err(),
            ScaleError::InvalidRootSelector {
                source_root_node_index: 99
            }
        ));
        let bad_skin = ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 99,
                source_root_node_index: 0,
                expected_factor: 0.01,
            },
            document: &doc,
            capability: &capability,
        };
        assert!(matches!(
            plan_scale(&bad_skin).unwrap_err(),
            ScaleError::InvalidSkinSelector {
                source_skin_index: 99
            }
        ));
    }

    #[test]
    fn incomplete_closure_when_a_skin_joint_is_outside_the_scaled_roots_descendants() {
        // Root and an unrelated sibling subtree; the skin's joint (bone 2)
        // is not a descendant of the declared root (bone 0).
        let nodes = vec![
            rig(None, 0, Vec3::ZERO),
            rig(None, 1, Vec3::ZERO),
            rig(Some(1), 2, Vec3::new(0.0, 1.0, 0.0)),
        ];
        let doc = rig_document(&nodes, &[2], 0, Mat4::IDENTITY);
        let capability = complete_capability();
        let request = ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 1.0,
            },
            document: &doc,
            capability: &capability,
        };
        assert_eq!(
            plan_scale(&request).unwrap_err(),
            ScaleError::IncompleteClosure {
                reason: "joint_not_descendant_of_scaled_root"
            }
        );
    }

    #[test]
    fn descendant_unskinned_geometry_inside_the_closure_rejects() {
        let nodes = vec![
            rig(None, 0, Vec3::ZERO),
            rig(Some(0), 1, Vec3::new(0.0, 1.0, 0.0)),
            rig(Some(1), 2, Vec3::new(1.0, 0.0, 0.0)),
        ];
        let mut doc = rig_document(&nodes, &[1], 0, Mat4::IDENTITY);
        // An extra unskinned mesh instance attached at bone 2, a descendant
        // of the affected joint.
        doc.assets.meshes.push(MeshAsset {
            name: "prop".into(),
            source_mesh_index: 1,
            primitives: vec![Primitive {
                positions: vec![Vec3::ZERO],
                joints: vec![[0, 0, 0, 0]],
                weights: vec![[1.0, 0.0, 0.0, 0.0]],
                ..Primitive::default()
            }],
        });
        doc.assets.instances.push(MeshInstance {
            source_node_index: 2,
            node: 2,
            mesh: 1,
            skin_joints: Vec::new(),
            skin_ibms: Vec::new(),
        });
        let capability = complete_capability();
        let request = ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 1.0,
            },
            document: &doc,
            capability: &capability,
        };
        assert!(matches!(
            plan_scale(&request).unwrap_err(),
            ScaleError::UnsupportedUnskinnedGeometry { node: 2 }
        ));
    }

    #[test]
    fn root_attached_unskinned_geometry_rejects() {
        // Root (bone 0) and joint (bone 1); an unskinned mesh instance is
        // attached directly at the selected root itself, not a later
        // descendant, so it must still be rejected.
        let mut doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        doc.assets.meshes.push(MeshAsset {
            name: "prop".into(),
            source_mesh_index: 1,
            primitives: vec![Primitive {
                positions: vec![Vec3::ZERO],
                joints: vec![[0, 0, 0, 0]],
                weights: vec![[1.0, 0.0, 0.0, 0.0]],
                ..Primitive::default()
            }],
        });
        doc.assets.instances.push(MeshInstance {
            source_node_index: 0,
            node: 0,
            mesh: 1,
            skin_joints: Vec::new(),
            skin_ibms: Vec::new(),
        });
        let capability = complete_capability();
        let request = ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 1.0,
            },
            document: &doc,
            capability: &capability,
        };
        assert!(matches!(
            plan_scale(&request).unwrap_err(),
            ScaleError::UnsupportedUnskinnedGeometry { node: 0 }
        ));
    }

    #[test]
    fn ancestor_path_attached_unskinned_geometry_rejects() {
        // Root (bone 0) -> mid (bone 1) -> joint (bone 2); bone 1 is not the
        // root and not a skin joint, it is only reached by walking the
        // joint's ancestor chain, so it must still be rejected.
        let nodes = vec![
            rig(None, 0, Vec3::ZERO),
            rig(Some(0), 1, Vec3::new(0.0, 1.0, 0.0)),
            rig(Some(1), 2, Vec3::new(0.0, 1.0, 0.0)),
        ];
        let mut doc = rig_document(&nodes, &[2], 0, Mat4::IDENTITY);
        doc.assets.meshes.push(MeshAsset {
            name: "prop".into(),
            source_mesh_index: 1,
            primitives: vec![Primitive {
                positions: vec![Vec3::ZERO],
                joints: vec![[0, 0, 0, 0]],
                weights: vec![[1.0, 0.0, 0.0, 0.0]],
                ..Primitive::default()
            }],
        });
        doc.assets.instances.push(MeshInstance {
            source_node_index: 1,
            node: 1,
            mesh: 1,
            skin_joints: Vec::new(),
            skin_ibms: Vec::new(),
        });
        let capability = complete_capability();
        let request = ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 1.0,
            },
            document: &doc,
            capability: &capability,
        };
        assert!(matches!(
            plan_scale(&request).unwrap_err(),
            ScaleError::UnsupportedUnskinnedGeometry { node: 1 }
        ));
    }

    #[test]
    fn dangling_ancestor_source_parent_index_rejects_without_panicking() {
        // Root (bone 0) -> mid (bone 1) -> joint (bone 2); the source
        // skeleton then drops the projection for bone 1 entirely, leaving
        // bone 2's `parent_source_node_index` dangling. Walking the joint's
        // ancestor chain must fail closed with a typed error rather than
        // panic on an unchecked map index.
        let nodes = vec![
            rig(None, 0, Vec3::ZERO),
            rig(Some(0), 1, Vec3::new(0.0, 1.0, 0.0)),
            rig(Some(1), 2, Vec3::new(0.0, 1.0, 0.0)),
        ];
        let mut doc = rig_document(&nodes, &[2], 0, Mat4::IDENTITY);
        doc.assets
            .source_skeleton
            .nodes
            .retain(|node| node.source_node_index != 1);
        let capability = complete_capability();
        let request = ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 1.0,
            },
            document: &doc,
            capability: &capability,
        };
        assert_eq!(
            plan_scale(&request).unwrap_err(),
            ScaleError::IncompleteClosure {
                reason: "dangling_source_parent_node_index"
            }
        );
    }

    // --- Scale-animation refusal ----------------------------------------

    #[test]
    fn scale_track_on_an_affected_node_is_refused_by_planning() {
        let mut doc = compensated_document();
        doc.clips.push(Clip {
            name: "clip".into(),
            duration_s: 1.0,
            tracks: vec![Track {
                bone: 1,
                property: Property::Scale,
                interpolation: Interpolation::Linear,
                times: vec![0.0],
                values: TrackValues::Vec3s(vec![Vec3::ONE]),
            }],
        });
        let capability = complete_capability();
        let request = ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 0.01,
            },
            document: &doc,
            capability: &capability,
        };
        assert!(matches!(
            plan_scale(&request).unwrap_err(),
            ScaleError::AffectedScaleAnimation {
                clip_index: 0,
                node: 1
            }
        ));
    }

    // --- Mid-build failure and source-mutation safety -------------------

    #[test]
    fn build_scale_candidate_rejects_a_scale_track_added_after_planning_without_mutating_the_document()
     {
        let doc = compensated_document();
        let capability = complete_capability();
        let request = ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 0.01,
            },
            document: &doc,
            capability: &capability,
        };
        // Plan against the clean document through the public API.
        let plan = plan_scale(&request).unwrap();

        // Mutate a *different* document after planning: build_scale_candidate
        // must independently reject the now-invalid state rather than trust
        // the plan was computed against what it was handed.
        let mut mutated = doc.clone();
        mutated.clips.push(Clip {
            name: "clip".into(),
            duration_s: 1.0,
            tracks: vec![Track {
                bone: 1,
                property: Property::Scale,
                interpolation: Interpolation::Linear,
                times: vec![0.0],
                values: TrackValues::Vec3s(vec![Vec3::ONE]),
            }],
        });
        let original_translation = mutated.skeleton.bones[0].rest.translation;
        let original_clip_count = mutated.clips.len();

        let error = build_scale_candidate(&mutated, &plan).unwrap_err();
        assert!(matches!(error, ScaleError::AffectedScaleAnimation { .. }));
        assert_eq!(
            mutated.skeleton.bones[0].rest.translation,
            original_translation
        );
        assert_eq!(mutated.clips.len(), original_clip_count);
        assert_eq!(doc.skeleton.bones[0].rest.translation, Vec3::ZERO);
    }

    #[test]
    fn every_rejection_path_leaves_the_source_document_unchanged() {
        let cases: Vec<Box<dyn Fn() -> (Document, ScaleOperation)>> = vec![
            Box::new(|| {
                (
                    rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY),
                    ScaleOperation::WholeDocumentLinearUnits { factor: -1.0 },
                )
            }),
            Box::new(|| {
                (
                    rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY),
                    ScaleOperation::RestBindUniformScale {
                        source_skin_index: 0,
                        source_root_node_index: 0,
                        expected_factor: 0.5,
                    },
                )
            }),
            Box::new(|| {
                (
                    rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY),
                    ScaleOperation::RestBindUniformScale {
                        source_skin_index: 99,
                        source_root_node_index: 0,
                        expected_factor: 1.0,
                    },
                )
            }),
        ];
        for case in cases {
            let (doc, operation) = case();
            let before = doc.skeleton.bones[0].rest.translation;
            let capability = complete_capability();
            let request = ScaleRequest {
                operation,
                document: &doc,
                capability: &capability,
            };
            assert!(plan_scale(&request).is_err());
            assert_eq!(doc.skeleton.bones[0].rest.translation, before);
        }
    }

    // --- Duplicate source-skeleton identity (hardening gap 1) -----------

    #[test]
    fn duplicate_source_node_index_rejects_instead_of_last_write_wins() {
        let mut doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        // Two source nodes both claim `source_node_index == 1`; a
        // `BTreeMap`-keyed projection would silently keep only one.
        let mut duplicate = doc.assets.source_skeleton.nodes[1].clone();
        duplicate.source_node_index = 1;
        doc.assets.source_skeleton.nodes.push(duplicate);
        let capability = complete_capability();
        let request = ScaleRequest {
            operation: ScaleOperation::WholeDocumentLinearUnits { factor: 2.0 },
            document: &doc,
            capability: &capability,
        };
        assert!(matches!(
            plan_scale(&request).unwrap_err(),
            ScaleError::DuplicateSourceNodeIndex {
                source_node_index: 1
            }
        ));
    }

    #[test]
    fn duplicate_source_skin_index_rejects_instead_of_first_match() {
        let mut doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        let duplicate = doc.assets.source_skeleton.skins[0].clone();
        doc.assets.source_skeleton.skins.push(duplicate);
        let capability = complete_capability();
        let request = ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 1.0,
            },
            document: &doc,
            capability: &capability,
        };
        assert!(matches!(
            plan_scale(&request).unwrap_err(),
            ScaleError::DuplicateSourceSkinIndex {
                source_skin_index: 0
            }
        ));
    }

    // --- Duplicate clip tracks (hardening gap 2) -------------------------

    #[test]
    fn duplicate_clip_track_for_the_same_bone_and_property_rejects() {
        let mut doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        let track = Track {
            bone: 1,
            property: Property::Translation,
            interpolation: Interpolation::Linear,
            times: vec![0.0, 1.0],
            values: TrackValues::Vec3s(vec![Vec3::ZERO, Vec3::ONE]),
        };
        doc.clips.push(Clip {
            name: "clip".into(),
            duration_s: 1.0,
            tracks: vec![track.clone(), track],
        });
        let capability = complete_capability();
        let request = ScaleRequest {
            operation: ScaleOperation::WholeDocumentLinearUnits { factor: 2.0 },
            document: &doc,
            capability: &capability,
        };
        assert!(matches!(
            plan_scale(&request).unwrap_err(),
            ScaleError::DuplicateClipTrack {
                clip_index: 0,
                node: 1,
                property: Property::Translation,
            }
        ));
    }

    #[test]
    fn malformed_track_shapes_reject_without_panicking() {
        // Each malformation is paired with the stable reason it must be
        // reported as: a single interchangeable reason string would let a
        // producer's evidence say "invalid shape" without ever saying which
        // shape rule the source broke.
        let bad_tracks = [
            // Out-of-range bone.
            (
                "bone_index_out_of_range",
                Track {
                    bone: 99,
                    property: Property::Translation,
                    interpolation: Interpolation::Linear,
                    times: vec![0.0],
                    values: TrackValues::Vec3s(vec![Vec3::ZERO]),
                },
            ),
            // Empty times.
            (
                "empty_times",
                Track {
                    bone: 1,
                    property: Property::Translation,
                    interpolation: Interpolation::Linear,
                    times: vec![],
                    values: TrackValues::Vec3s(vec![]),
                },
            ),
            // Non-finite time.
            (
                "non_finite_time",
                Track {
                    bone: 1,
                    property: Property::Translation,
                    interpolation: Interpolation::Linear,
                    times: vec![f32::NAN],
                    values: TrackValues::Vec3s(vec![Vec3::ZERO]),
                },
            ),
            // Non-ascending times.
            (
                "times_not_strictly_increasing",
                Track {
                    bone: 1,
                    property: Property::Translation,
                    interpolation: Interpolation::Linear,
                    times: vec![1.0, 0.0],
                    values: TrackValues::Vec3s(vec![Vec3::ZERO, Vec3::ONE]),
                },
            ),
            // Value count disagrees with times/interpolation.
            (
                "value_count_mismatch",
                Track {
                    bone: 1,
                    property: Property::Translation,
                    interpolation: Interpolation::Linear,
                    times: vec![0.0, 1.0],
                    values: TrackValues::Vec3s(vec![Vec3::ZERO]),
                },
            ),
            // Cubic-spline value count not `3 * times.len()`.
            (
                "value_count_mismatch",
                Track {
                    bone: 1,
                    property: Property::Translation,
                    interpolation: Interpolation::CubicSpline,
                    times: vec![0.0, 1.0],
                    values: TrackValues::Vec3s(vec![Vec3::ZERO; 4]),
                },
            ),
            // `TrackValues` variant disagrees with `property`.
            (
                "value_type_mismatches_property",
                Track {
                    bone: 1,
                    property: Property::Translation,
                    interpolation: Interpolation::Linear,
                    times: vec![0.0],
                    values: TrackValues::Quats(vec![Quat::IDENTITY]),
                },
            ),
            // Non-finite value.
            (
                "non_finite_value",
                Track {
                    bone: 1,
                    property: Property::Translation,
                    interpolation: Interpolation::Linear,
                    times: vec![0.0],
                    values: TrackValues::Vec3s(vec![Vec3::new(f32::NAN, 0.0, 0.0)]),
                },
            ),
        ];
        for (expected_reason, track) in bad_tracks {
            let node = track.bone;
            let mut doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
            doc.clips.push(Clip {
                name: "clip".into(),
                duration_s: 1.0,
                tracks: vec![track],
            });
            let capability = complete_capability();
            let request = ScaleRequest {
                operation: ScaleOperation::WholeDocumentLinearUnits { factor: 2.0 },
                document: &doc,
                capability: &capability,
            };
            assert_eq!(
                plan_scale(&request).unwrap_err(),
                ScaleError::InvalidTrackShape {
                    clip_index: 0,
                    node,
                    reason: expected_reason,
                }
            );
        }
    }

    // --- world_at_time hardening (hardening gap 3) -----------------------

    #[test]
    fn out_of_range_track_bone_added_after_planning_rejects_without_panicking() {
        let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        let capability = complete_capability();
        let request = ScaleRequest {
            operation: ScaleOperation::WholeDocumentLinearUnits { factor: 2.0 },
            document: &doc,
            capability: &capability,
        };
        let plan = plan_scale(&request).unwrap();
        let mut mutated = doc.clone();
        mutated.clips.push(Clip {
            name: "clip".into(),
            duration_s: 1.0,
            tracks: vec![Track {
                bone: 99,
                property: Property::Translation,
                interpolation: Interpolation::Linear,
                times: vec![0.0],
                values: TrackValues::Vec3s(vec![Vec3::ZERO]),
            }],
        });
        assert!(matches!(
            build_scale_candidate(&mutated, &plan).unwrap_err(),
            ScaleError::InvalidTrackShape { .. }
        ));
    }

    #[test]
    fn invalid_skeleton_parent_ordering_rejects_without_panicking() {
        // Bone 0's parent is bone 1, which is later in `bones` — violates
        // the documented parent-before-child invariant.
        let doc = Document {
            skeleton: Skeleton {
                bones: vec![
                    Bone {
                        name: "a".into(),
                        parent: Some(1),
                        rest: Transform::IDENTITY,
                        inverse_bind: None,
                    },
                    Bone {
                        name: "b".into(),
                        parent: None,
                        rest: Transform::IDENTITY,
                        inverse_bind: None,
                    },
                ],
            },
            clips: Vec::new(),
            assets: SceneAssets {
                source_skeleton: SourceSkeletonAssets {
                    coverage: SourceSkeletonCoverage::Complete,
                    nodes: vec![
                        SourceNodeAsset {
                            source_node_index: 0,
                            name: None,
                            parent_source_node_index: Some(1),
                            scene_root_indices: vec![],
                            local_rest: SourceNodeLocalRest::Matrix(Mat4::IDENTITY),
                            bone: Some(0),
                        },
                        SourceNodeAsset {
                            source_node_index: 1,
                            name: None,
                            parent_source_node_index: None,
                            scene_root_indices: vec![0],
                            local_rest: SourceNodeLocalRest::Matrix(Mat4::IDENTITY),
                            bone: Some(1),
                        },
                    ],
                    skins: Vec::new(),
                },
                ..SceneAssets::default()
            },
            source: Default::default(),
        };
        let capability = complete_capability();
        let request = ScaleRequest {
            operation: ScaleOperation::WholeDocumentLinearUnits { factor: 2.0 },
            document: &doc,
            capability: &capability,
        };
        assert!(matches!(
            plan_scale(&request).unwrap_err(),
            ScaleError::InvalidParent { node: 0, parent: 1 }
        ));
    }

    // --- skinned_bounds hardening (hardening gap 4) -----------------------

    #[test]
    fn joint_influence_slot_outside_skin_joints_rejects_without_panicking() {
        let doc = compensated_document();
        let capability = complete_capability();
        let request = ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 0.01,
            },
            document: &doc,
            capability: &capability,
        };
        let plan = plan_scale(&request).unwrap();
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let mut malformed = doc.clone();
        // Skin has one joint (slot 0), but this vertex claims influence
        // slot 5.
        malformed.assets.meshes[0].primitives[0].joints[0] = [5, 0, 0, 0];
        assert!(matches!(
            prove_scale(&malformed, &candidate, &plan).unwrap_err(),
            ScaleError::InvalidSkinnedPrimitive {
                reason: "joint_influence_slot_out_of_range",
                ..
            }
        ));
    }

    #[test]
    fn missing_per_vertex_joints_or_weights_in_a_skinned_primitive_rejects() {
        let doc = compensated_document();
        let capability = complete_capability();
        let request = ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 0.01,
            },
            document: &doc,
            capability: &capability,
        };
        let plan = plan_scale(&request).unwrap();
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let mut malformed = doc.clone();
        // One position, but the weights array is empty: no longer parallel
        // to `positions`.
        malformed.assets.meshes[0].primitives[0].weights.clear();
        assert!(matches!(
            prove_scale(&malformed, &candidate, &plan).unwrap_err(),
            ScaleError::InvalidSkinnedPrimitive {
                reason: "joints_or_weights_length_mismatch",
                ..
            }
        ));
    }

    #[test]
    fn non_finite_vertex_position_in_a_skinned_primitive_rejects() {
        let doc = compensated_document();
        let capability = complete_capability();
        let request = ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 0.01,
            },
            document: &doc,
            capability: &capability,
        };
        let plan = plan_scale(&request).unwrap();
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let mut malformed = doc.clone();
        malformed.assets.meshes[0].primitives[0].positions[0] = Vec3::new(f32::NAN, 0.0, 0.0);
        // Base `POSITION` is a rewritten domain, so its finiteness is now a
        // document-shape invariant checked at every entry point rather than
        // something only the skinned-bounds walk happens to notice — which
        // is what makes it hold for the *candidate* a build returns, too.
        assert!(matches!(
            prove_scale(&malformed, &candidate, &plan).unwrap_err(),
            ScaleError::InvalidMeshPrimitive {
                reason: "non_finite_position",
                ..
            }
        ));
    }

    #[test]
    fn missing_inverse_bind_evidence_rejects_instead_of_defaulting_to_identity() {
        let doc = compensated_document();
        let capability = complete_capability();
        let request = ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 0.01,
            },
            document: &doc,
            capability: &capability,
        };
        let plan = plan_scale(&request).unwrap();
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let mut malformed = doc.clone();
        // Empty `skin_ibms` falls back to the joint bone's own
        // `inverse_bind`, which is `None` for every bone this fixture
        // builds: there is genuinely no inverse-bind evidence, so this must
        // reject rather than silently substitute identity.
        malformed.assets.instances[0].skin_ibms.clear();
        assert!(matches!(
            prove_scale(&malformed, &candidate, &plan).unwrap_err(),
            ScaleError::MissingInverseBind { .. }
        ));
    }

    /// Build a single-bone, single-instance `Document` with no `skin_ibms`
    /// and no `Bone::inverse_bind`, so `instance_bind` must fall through to
    /// `document.assets.source_skeleton` evidence — exactly the glTF
    /// "skin declares no `inverseBindMatrices` accessor" shape, where the
    /// format default is an identity inverse-bind matrix per joint.
    fn absent_inverse_bind_document(
        status: SourceInverseBindAccessorStatus,
        coverage: SourceSkeletonCoverage,
    ) -> Document {
        Document {
            skeleton: Skeleton {
                bones: vec![Bone {
                    name: "bone0".into(),
                    parent: None,
                    rest: Transform::IDENTITY,
                    inverse_bind: None,
                }],
            },
            clips: Vec::new(),
            assets: SceneAssets {
                meshes: vec![MeshAsset {
                    name: "mesh".into(),
                    source_mesh_index: 0,
                    primitives: vec![Primitive {
                        positions: vec![Vec3::new(1.0, 0.0, 0.0)],
                        joints: vec![[0, 0, 0, 0]],
                        weights: vec![[1.0, 0.0, 0.0, 0.0]],
                        ..Primitive::default()
                    }],
                }],
                instances: vec![MeshInstance {
                    source_node_index: 0,
                    node: 0,
                    mesh: 0,
                    skin_joints: vec![0],
                    skin_ibms: Vec::new(),
                }],
                source_skeleton: SourceSkeletonAssets {
                    coverage,
                    nodes: vec![SourceNodeAsset {
                        source_node_index: 0,
                        name: None,
                        parent_source_node_index: None,
                        scene_root_indices: vec![0],
                        local_rest: SourceNodeLocalRest::Trs {
                            translation: Vec3::ZERO,
                            rotation: Quat::IDENTITY,
                            scale: Vec3::ONE,
                        },
                        bone: Some(0),
                    }],
                    skins: vec![SourceSkinAsset {
                        source_skin_index: 0,
                        name: None,
                        skeleton_root_source_node_index: None,
                        joint_source_node_indices: vec![0],
                        inverse_bind_accessor: SourceInverseBindAccessor {
                            status,
                            declared_count: None,
                            matrices: Vec::new(),
                        },
                        attachments: vec![SourceSkinAttachment {
                            source_node_index: 0,
                            source_mesh_index: Some(0),
                        }],
                    }],
                },
                ..SceneAssets::default()
            },
            source: Default::default(),
        }
    }

    #[test]
    fn absent_inverse_bind_accessor_with_complete_coverage_resolves_to_identity() {
        let doc = absent_inverse_bind_document(
            SourceInverseBindAccessorStatus::Absent,
            SourceSkeletonCoverage::Complete,
        );
        let instance = &doc.assets.instances[0];
        assert_eq!(instance_bind(&doc, instance, 0, 0), Ok(Mat4::IDENTITY));
    }

    #[test]
    fn malformed_inverse_bind_accessor_status_still_rejects_rather_than_defaulting() {
        for status in [
            SourceInverseBindAccessorStatus::EmptyAccessor,
            SourceInverseBindAccessorStatus::CountMismatch,
            SourceInverseBindAccessorStatus::Unreadable,
        ] {
            let doc = absent_inverse_bind_document(status, SourceSkeletonCoverage::Complete);
            let instance = &doc.assets.instances[0];
            assert!(matches!(
                instance_bind(&doc, instance, 0, 0),
                Err(ScaleError::MissingInverseBind { node: 0 })
            ));
        }
    }

    #[test]
    fn absent_inverse_bind_accessor_with_incomplete_coverage_still_rejects() {
        let doc = absent_inverse_bind_document(
            SourceInverseBindAccessorStatus::Absent,
            SourceSkeletonCoverage::Unavailable,
        );
        let instance = &doc.assets.instances[0];
        assert!(matches!(
            instance_bind(&doc, instance, 0, 0),
            Err(ScaleError::MissingInverseBind { node: 0 })
        ));
    }

    // --- Revalidation at build/prove boundaries (hardening gap 5) -------

    #[test]
    fn build_scale_candidate_rejects_a_duplicate_clip_track_added_after_planning() {
        let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        let capability = complete_capability();
        let request = ScaleRequest {
            operation: ScaleOperation::WholeDocumentLinearUnits { factor: 2.0 },
            document: &doc,
            capability: &capability,
        };
        let plan = plan_scale(&request).unwrap();
        let mut mutated = doc.clone();
        let track = Track {
            bone: 1,
            property: Property::Translation,
            interpolation: Interpolation::Linear,
            times: vec![0.0],
            values: TrackValues::Vec3s(vec![Vec3::ZERO]),
        };
        mutated.clips.push(Clip {
            name: "clip".into(),
            duration_s: 1.0,
            tracks: vec![track.clone(), track],
        });
        assert!(matches!(
            build_scale_candidate(&mutated, &plan).unwrap_err(),
            ScaleError::DuplicateClipTrack { .. }
        ));
    }

    #[test]
    fn prove_scale_rejects_a_malformed_source_document_replayed_against_a_valid_candidate() {
        let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        let capability = complete_capability();
        let request = ScaleRequest {
            operation: ScaleOperation::WholeDocumentLinearUnits { factor: 2.0 },
            document: &doc,
            capability: &capability,
        };
        let plan = plan_scale(&request).unwrap();
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let mut malformed_source = doc.clone();
        malformed_source.assets.instances[0].mesh = 99;
        assert!(matches!(
            prove_scale(&malformed_source, &candidate, &plan).unwrap_err(),
            ScaleError::InvalidMeshInstance {
                reason: "mesh_index_out_of_range",
                ..
            }
        ));
    }

    // --- Candidate proof structure parity (hardening gap 6) -------------

    #[test]
    fn prove_scale_rejects_a_candidate_missing_a_source_clip() {
        let mut doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        doc.clips.push(Clip {
            name: "clip".into(),
            duration_s: 1.0,
            tracks: vec![Track {
                bone: 1,
                property: Property::Translation,
                interpolation: Interpolation::Linear,
                times: vec![0.0],
                values: TrackValues::Vec3s(vec![Vec3::ZERO]),
            }],
        });
        let capability = complete_capability();
        let request = ScaleRequest {
            operation: ScaleOperation::WholeDocumentLinearUnits { factor: 2.0 },
            document: &doc,
            capability: &capability,
        };
        let plan = plan_scale(&request).unwrap();
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let mut dropped = candidate.document().clone();
        dropped.clips.clear();
        let dropped = ScaleCandidate { document: dropped };
        assert!(matches!(
            prove_scale(&doc, &dropped, &plan).unwrap_err(),
            ScaleError::CandidateStructureMismatch {
                reason: "clip_count_mismatch"
            }
        ));
    }

    #[test]
    fn prove_scale_rejects_a_candidate_with_an_extra_track_not_present_in_source() {
        let doc = compensated_document();
        let capability = complete_capability();
        let request = ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 0.01,
            },
            document: &doc,
            capability: &capability,
        };
        let plan = plan_scale(&request).unwrap();
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let mut extended = candidate.document().clone();
        extended.clips.push(Clip {
            name: "extra".into(),
            duration_s: 1.0,
            tracks: vec![Track {
                bone: 0,
                property: Property::Translation,
                interpolation: Interpolation::Linear,
                times: vec![0.0],
                values: TrackValues::Vec3s(vec![Vec3::ZERO]),
            }],
        });
        let extended = ScaleCandidate { document: extended };
        assert!(matches!(
            prove_scale(&doc, &extended, &plan).unwrap_err(),
            ScaleError::CandidateStructureMismatch {
                reason: "clip_count_mismatch"
            }
        ));
    }

    #[test]
    fn prove_scale_rejects_a_candidate_whose_track_times_differ_from_the_source() {
        // `prove_scale` does not require its two documents to be the same
        // one: a caller can build a candidate from one document and prove
        // it against another. Track times are the sampling grid *both*
        // sides are read on, so a candidate that agrees on track identity,
        // count, property and interpolation but disagrees on `times` would
        // have every sampled obligation -- key, cubic interior, trajectory,
        // skin and bounds -- comparing values drawn from different
        // instants. That proves nothing, so it is a structure mismatch
        // rather than a residual.
        let mut doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        doc.clips.push(Clip {
            name: "clip".into(),
            duration_s: 1.0,
            tracks: vec![Track {
                bone: 1,
                property: Property::Translation,
                interpolation: Interpolation::Linear,
                times: vec![0.0, 1.0],
                values: TrackValues::Vec3s(vec![
                    Vec3::new(0.0, 1.0, 0.0),
                    Vec3::new(0.0, 3.0, 0.0),
                ]),
            }],
        });
        let capability = complete_capability();
        // Whole-document conversion by `0.01`.
        let plan = whole_document_plan(&doc, &capability);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();

        // The candidate is otherwise *correct*: both affected translation
        // keys carry exactly `0.01x` the authored value, and it proves.
        let TrackValues::Vec3s(built) = &candidate.document().clips[0].tracks[0].values else {
            panic!("translation track must hold Vec3 values");
        };
        assert!((built[0] - Vec3::new(0.0, 0.01, 0.0)).length() < 1e-9);
        assert!((built[1] - Vec3::new(0.0, 0.03, 0.0)).length() < 1e-9);
        prove_scale(&doc, &candidate, &plan).unwrap();

        // Move the second key from `1.0s` to `2.0s` and change nothing
        // else. Track count, `(bone, property, interpolation)` and the
        // value count all still match the source, so neither a count
        // mismatch nor a value residual can fire ahead of the time check:
        // the disagreeing sampling grid is the only thing left to catch.
        let mut retimed = candidate.document().clone();
        retimed.clips[0].tracks[0].times = vec![0.0, 2.0];
        assert_eq!(
            retimed.clips[0].tracks[0].values.len(),
            doc.clips[0].tracks[0].values.len(),
            "only the sampling grid may differ"
        );
        let retimed = ScaleCandidate { document: retimed };
        assert_eq!(
            prove_scale(&doc, &retimed, &plan).unwrap_err(),
            ScaleError::CandidateStructureMismatch {
                reason: "track_shape_mismatch"
            }
        );
    }

    // --- Authored (unnormalized) rotation equality ----------------------

    #[test]
    fn an_authored_rest_rotation_with_magnitude_below_one_is_not_a_rotation_residual() {
        // A routine authored glTF quaternion (a 45-degree turn about Y) whose
        // stored magnitude is `1 - 4e-8`, which `invariant-9` forbids the
        // loader from renormalizing. Comparing it to an untouched copy of
        // itself as an *angle* reports `6.9e-4`, `69x` the `1e-5` tolerance,
        // and rejects a correct candidate. As the equality test it actually
        // is, the residual is exactly zero.
        let unnormalized = Quat::from_xyzw(0.0, 0.3826834, 0.0, 0.9238795);
        assert!(
            unnormalized.as_dquat().length() < 1.0,
            "fixture quaternion must be shorter than unit length"
        );
        let nodes = vec![
            rig(None, 0, Vec3::ZERO),
            RigNode {
                parent: Some(0),
                source_node_index: 1,
                translation: Vec3::new(0.0, 1.0, 0.0),
                rotation: unnormalized,
                scale: Vec3::ONE,
            },
        ];
        let doc = rig_document(&nodes, &[1], 0, Mat4::IDENTITY);
        let capability = complete_capability();
        let request = ScaleRequest {
            operation: ScaleOperation::WholeDocumentLinearUnits { factor: 0.01 },
            document: &doc,
            capability: &capability,
        };
        let plan = plan_scale(&request).unwrap();
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let proof = prove_scale(&doc, &candidate, &plan).unwrap();
        assert_eq!(proof.rest_rotation_residual, 0.0);
    }

    #[test]
    fn a_genuinely_rewritten_rest_rotation_still_fails_proof() {
        let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        let capability = complete_capability();
        let request = ScaleRequest {
            operation: ScaleOperation::WholeDocumentLinearUnits { factor: 0.01 },
            document: &doc,
            capability: &capability,
        };
        let plan = plan_scale(&request).unwrap();
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let mut broken = candidate.document().clone();
        broken.skeleton.bones[1].rest.rotation = Quat::from_rotation_y(0.5);
        let broken = ScaleCandidate { document: broken };
        assert!(matches!(
            prove_scale(&doc, &broken, &plan).unwrap_err(),
            ScaleError::ProofResidualExceeded {
                kind: ProofResidualKind::RestRotation,
                ..
            }
        ));
    }

    #[test]
    fn a_rest_rotation_error_is_bounded_by_the_declared_angle_not_twice_it() {
        // DESIGN.md Appendix D §D.1 declares "shortest-path rotation residual
        // is at most `1e-5` radians". The residual is *measured* as a
        // double-cover-aware quaternion chord `|q1 - q2| = 2 * sin(theta / 4)`
        // — which is `theta / 2` to first order — so checking that chord
        // against the declared *angle* accepted a genuine `2e-5 rad` rotation
        // error: fail-open by exactly two.
        //
        // Both probes are literal. A rotation of `theta` about Y is
        // `(0, sin(theta / 2), 0, cos(theta / 2))`; at these angles
        // `cos(theta / 2)` is within `6e-11` of one and rounds to exactly
        // `1.0f32`, so the authored value is `(0, theta / 2, 0, 1)` and the
        // angle it represents is `2 * atan2(theta / 2, 1) = theta` to far
        // better than the `1e-9` bands asserted below. The source rotation is
        // the identity, so each doctored quaternion's own angle *is* the
        // residual under test.
        let doc = rig_document(&rest_only_leaf_rig(), &[1], 0, Mat4::IDENTITY);
        assert_eq!(doc.skeleton.bones[2].rest.rotation, Quat::IDENTITY);
        let capability = complete_capability();
        let plan = whole_document_plan(&doc, &capability);
        assert_eq!(plan.tolerance_policy().rotation_residual_radians, 1e-5);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();

        // Bone 2 is a leaf carrying no skin slot, no mesh vertex and no
        // track, so rotating it moves no descendant origin, no skin palette
        // and no sampled pose: the rest-rotation obligation is the only one
        // that can see either probe.
        let rotate_leaf = |half_angle: f32| {
            let mut broken = candidate.document().clone();
            broken.skeleton.bones[2].rest.rotation = Quat::from_xyzw(0.0, half_angle, 0.0, 1.0);
            ScaleCandidate { document: broken }
        };

        // `9e-6 rad`: inside the declared bound, and reported *as* `9e-6`
        // rather than as the `4.5e-6` chord it was measured from — the
        // reported field carries the unit its name and the §D.6 evidence
        // contract promise.
        let inside = rotate_leaf(4.5e-6);
        let proof = prove_scale(&doc, &inside, &plan).unwrap();
        assert!(
            (proof.rest_rotation_residual - 9.0e-6).abs() < 1e-9,
            "residual {} is not the 9e-6 radian angle it measures",
            proof.rest_rotation_residual
        );

        // `1.1e-5 rad`: outside the declared bound, but only a `5.5e-6`
        // chord — the value a chord-against-radians comparison accepted.
        let outside = rotate_leaf(5.5e-6);
        let error = prove_scale(&doc, &outside, &plan).unwrap_err();
        let ScaleError::ProofResidualExceeded {
            kind,
            observed,
            tolerance,
        } = error
        else {
            panic!("expected a residual rejection, got {error:?}");
        };
        assert_eq!(kind, ProofResidualKind::RestRotation);
        assert_eq!(tolerance, 1e-5);
        assert!(
            (observed - 1.1e-5).abs() < 1e-9,
            "observed {observed} is not the 1.1e-5 radian angle it measures"
        );
    }

    #[test]
    fn a_rest_rotation_chord_above_two_saturates_at_two_pi_instead_of_reporting_nan() {
        // `quat_residual_radians` inverts the chord relation
        // `chord = 2 * sin(theta / 4)` as `theta = 4 * asin(chord / 2)`, whose
        // domain runs out at `chord = 2`. No pair of *unit* quaternions can
        // reach that (the double-cover minimum is at most `sqrt(2)`), but an
        // authored non-unit value can — and `invariant-9` forbids the loader
        // from normalizing one away — so the conversion clamps and saturates
        // at `4 * asin(1) = 2 * pi` rather than handing `asin` an
        // out-of-domain argument and reporting `NaN`.
        //
        // The pair fails closed either way: `2 * pi` and `NaN` both exceed the
        // `1e-5` bound (`check_residual` rejects a non-finite observation
        // outright). What is pinned here is therefore the *reported* residual
        // — the value DESIGN.md Appendix D §D.6 requires evidence to publish
        // next to the tolerance policy — not the accept/reject outcome.
        //
        // Both operands are literal. The source leaf rotation is exactly the
        // identity `(0, 0, 0, 1)` and the candidate's is the non-unit
        // `(0, 0, 0, -4)`, so the double-cover-aware chord is
        // `min(|(0, 0, 0, 5)|, |(0, 0, 0, -3)|) = 3`: `chord / 2 = 1.5` is
        // genuinely outside `asin`'s domain, and the clamp is the only thing
        // between this pair and a `NaN`.
        let doc = rig_document(&rest_only_leaf_rig(), &[1], 0, Mat4::IDENTITY);
        assert_eq!(doc.skeleton.bones[2].rest.rotation, Quat::IDENTITY);
        let capability = complete_capability();
        let plan = whole_document_plan(&doc, &capability);
        assert_eq!(plan.tolerance_policy().rotation_residual_radians, 1e-5);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();

        // Bone 2 is a leaf carrying no skin slot, no mesh vertex and no track,
        // so the rest-rotation obligation is the only one that can observe it.
        // A quaternion of the form `(0, 0, 0, w)` also leaves the world matrix
        // derived from it at the identity, so not even the rest-*translation*
        // residual of this node moves: the saturated value below is reported
        // by the rotation obligation and nothing else.
        let mut broken = candidate.document().clone();
        broken.skeleton.bones[2].rest.rotation = Quat::from_xyzw(0.0, 0.0, 0.0, -4.0);
        let broken = ScaleCandidate { document: broken };

        let error = prove_scale(&doc, &broken, &plan).unwrap_err();
        let ScaleError::ProofResidualExceeded {
            kind,
            observed,
            tolerance,
        } = error
        else {
            panic!("expected a residual rejection, got {error:?}");
        };
        assert_eq!(kind, ProofResidualKind::RestRotation);
        assert_eq!(tolerance, 1e-5);
        assert!(
            observed.is_finite(),
            "saturated residual {observed} must never be NaN"
        );
        // Exact, not approximate: `4.0 * asin(1.0)` is `4 * FRAC_PI_2`, and
        // scaling by a power of two is exact, so the saturation value is
        // bit-for-bit `TAU`.
        assert_eq!(observed, std::f64::consts::TAU);
    }

    // --- f32 representability of the declared factor --------------------

    #[test]
    fn a_factor_that_annihilates_or_overflows_at_the_f32_boundary_rejects_at_plan_time() {
        let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        let capability = complete_capability();
        // `1e-50` narrows to `0.0f32`: building would multiply every
        // translation, mesh `POSITION` and inverse-bind translation by zero
        // and every proof residual would still be exactly zero, because
        // `0 == 0 * 0`. `1e40` narrows to `inf`.
        for factor in [1e-50, 1e40] {
            let request = ScaleRequest {
                operation: ScaleOperation::WholeDocumentLinearUnits { factor },
                document: &doc,
                capability: &capability,
            };
            assert!(
                matches!(
                    plan_scale(&request).unwrap_err(),
                    ScaleError::FactorNotRepresentable { .. }
                ),
                "factor {factor} was not rejected"
            );
        }
    }

    #[test]
    fn a_rest_bind_factor_whose_reciprocal_overflows_f32_rejects_at_plan_time() {
        // `1e-40` narrows to a nonzero `f32` subnormal, so the declared
        // factor itself passes; the basis correction `C = scale(1 / s)` the
        // proof derives from it does not.
        let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        let capability = complete_capability();
        let request = ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 1e-40,
            },
            document: &doc,
            capability: &capability,
        };
        assert!(matches!(
            plan_scale(&request).unwrap_err(),
            ScaleError::FactorNotRepresentable {
                declared: 1e-40,
                ..
            }
        ));
    }

    #[test]
    fn a_non_finite_residual_fails_closed_instead_of_comparing_false() {
        // `NaN > tolerance` is `false`, so an unguarded comparison reports a
        // `NaN` residual as a pass.
        for (observed, tolerance) in [
            (f64::NAN, 1.0),
            (f64::INFINITY, 1.0),
            (0.0, f64::NAN),
            (0.0, f64::INFINITY),
        ] {
            assert!(
                check_residual(ProofResidualKind::Bounds, observed, tolerance).is_err(),
                "observed {observed} tolerance {tolerance} passed"
            );
        }
    }

    // --- f64 affine classification --------------------------------------

    #[test]
    fn a_shear_only_f64_can_see_is_still_classified_as_sheared() {
        // Column pair whose `f32` dot product is `-9.98e-6` (inside the
        // `1e-5` threshold) but whose `f64` dot product is `-1.00043e-5`
        // (outside it). Evaluating the classifier's dot products in `f32`
        // and casting afterwards accepts this basis as orthogonal.
        let c0 = Vec3::new(0.12792248, -0.99066633, -0.047073245);
        let c1 = Vec3::new(-0.34637994, -0.00016034879, -0.93809813);
        let c2 = Vec3::new(0.92933476, 0.13630849, -0.3431568);
        assert!((c1.dot(c2) as f64).abs() < 1e-5, "f32 dot is inside band");
        assert!(
            (c1.as_dvec3().dot(c2.as_dvec3())).abs() > 1e-5,
            "f64 dot is outside band"
        );
        let error = reject_case(|rest| {
            *rest = SourceNodeLocalRest::Matrix(Mat4::from_cols(
                c0.extend(0.0),
                c1.extend(0.0),
                c2.extend(0.0),
                Vec4::W,
            ));
        });
        assert!(
            matches!(
                error,
                ScaleError::InvalidAffineDomain {
                    reason: AffineDomainViolation::Sheared,
                    ..
                }
            ),
            "unexpected error {error:?}"
        );
    }

    // --- Per-value proof of every domain --------------------------------

    /// `unit_rig` plus a rotation track and a three-vertex primitive whose
    /// middle vertex is strictly interior to the skinned bounding box — the
    /// two payloads no sampled obligation looks at.
    fn payload_document() -> Document {
        let mut doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        doc.assets.meshes[0].primitives[0] = Primitive {
            positions: vec![
                Vec3::new(1.0, 1.0, 1.0),
                Vec3::ZERO,
                Vec3::new(-1.0, -1.0, -1.0),
            ],
            joints: vec![[0, 0, 0, 0]; 3],
            weights: vec![[1.0, 0.0, 0.0, 0.0]; 3],
            ..Primitive::default()
        };
        doc.clips.push(Clip {
            name: "clip".into(),
            duration_s: 1.0,
            tracks: vec![Track {
                bone: 1,
                property: Property::Rotation,
                interpolation: Interpolation::Linear,
                times: vec![0.0, 1.0],
                values: TrackValues::Quats(vec![
                    Quat::from_rotation_y(0.1),
                    Quat::from_rotation_y(0.1),
                ]),
            }],
        });
        doc
    }

    fn whole_document_plan(document: &Document, capability: &ScaleCapabilityFacts) -> ScalePlan {
        plan_scale(&ScaleRequest {
            operation: ScaleOperation::WholeDocumentLinearUnits { factor: 0.01 },
            document,
            capability,
        })
        .unwrap()
    }

    #[test]
    fn a_rotation_key_rewritten_in_the_candidate_fails_proof() {
        // Reachable through the public API: `build_scale_candidate` and
        // `prove_scale` each take their document separately, so a candidate
        // can be built from a doctored copy and proved against the real
        // source. Rotation values are a domain both operations declare
        // untouched; nothing sampled would notice.
        let doc = payload_document();
        let capability = complete_capability();
        let plan = whole_document_plan(&doc, &capability);
        let mut doctored = doc.clone();
        doctored.clips[0].tracks[0].values =
            TrackValues::Quats(vec![Quat::from_rotation_y(2.5), Quat::from_rotation_y(2.5)]);
        let candidate = build_scale_candidate(&doctored, &plan).unwrap();
        assert!(matches!(
            prove_scale(&doc, &candidate, &plan).unwrap_err(),
            ScaleError::ProofResidualExceeded {
                kind: ProofResidualKind::TrackValue,
                ..
            }
        ));
    }

    #[test]
    fn an_interior_mesh_vertex_moved_in_the_candidate_fails_proof() {
        let doc = payload_document();
        let capability = complete_capability();
        let plan = whole_document_plan(&doc, &capability);
        let mut doctored = doc.clone();
        // Strictly inside the skinned bounding box, so the bounds obligation
        // is blind to it.
        doctored.assets.meshes[0].primitives[0].positions[1] = Vec3::new(0.5, 0.5, 0.5);
        let candidate = build_scale_candidate(&doctored, &plan).unwrap();
        assert!(matches!(
            prove_scale(&doc, &candidate, &plan).unwrap_err(),
            ScaleError::ProofResidualExceeded {
                kind: ProofResidualKind::MeshPosition,
                ..
            }
        ));
    }

    #[test]
    fn an_unsampled_translation_tangent_is_named_by_the_track_value_obligation() {
        // The rotation and mesh-position arms of `check_candidate_values`
        // each already name themselves (see the two tests above); its
        // `Vec3s` arm — every translation value and cubic tangent element —
        // did not, so the kind it reports was free to be any variant.
        //
        // glTF cubic evaluation of the segment `[k0, k1]` reads only the
        // *out*-tangent of `k0` and the *in*-tangent of `k1`. For a two-key
        // track that leaves `values[0]`, the in-tangent at the first key,
        // unread at every key time and every cubic interior time — and so
        // unread by the trajectory, skin and bounds obligations derived from
        // those samples. The direct per-element check is the only obligation
        // that can see this element at all, which is what makes the kind it
        // reports this test's subject rather than an artefact of ordering.
        let mut doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        doc.clips.push(Clip {
            name: "clip".into(),
            duration_s: 1.0,
            tracks: vec![Track {
                bone: 1,
                property: Property::Translation,
                interpolation: Interpolation::CubicSpline,
                times: vec![0.0, 1.0],
                values: TrackValues::Vec3s(vec![
                    Vec3::new(0.0, 500.0, 0.0), // in-tangent @0 — never sampled
                    Vec3::new(0.0, 1.0, 0.0),   // value @0
                    Vec3::ZERO,                 // out-tangent @0 (`m0`)
                    Vec3::ZERO,                 // in-tangent @1 (`m1`)
                    Vec3::new(0.0, 1.0, 0.0),   // value @1
                    Vec3::ZERO,                 // out-tangent @1
                ]),
            }],
        });
        let capability = complete_capability();
        // Whole-document conversion by `0.01`.
        let plan = whole_document_plan(&doc, &capability);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let mut broken = candidate.document().clone();
        let TrackValues::Vec3s(values) = &mut broken.clips[0].tracks[0].values else {
            panic!("expected a vec3 track");
        };
        // A builder that left this one element un-rewritten: the candidate
        // keeps the source's `500` where `0.01 * 500 = 5` is expected.
        values[0] = Vec3::new(0.0, 500.0, 0.0);
        let broken = ScaleCandidate { document: broken };
        let error = prove_scale(&doc, &broken, &plan).unwrap_err();
        let ScaleError::ProofResidualExceeded {
            kind,
            observed,
            tolerance,
        } = error
        else {
            panic!("expected a proof residual, got {error:?}");
        };
        assert_eq!(kind, ProofResidualKind::TrackValue);
        // `|500 - 5| = 495`, against `1e-6 + 1e-5 * 500 = 5.001e-3`.
        assert!((observed - 495.0).abs() < 1e-9, "observed {observed}");
        assert!((tolerance - 5.001e-3).abs() < 1e-9, "tolerance {tolerance}");
    }

    #[test]
    fn an_honest_candidate_proves_every_retained_payload_with_a_zero_residual() {
        let doc = payload_document();
        let capability = complete_capability();
        let plan = whole_document_plan(&doc, &capability);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let proof = prove_scale(&doc, &candidate, &plan).unwrap();
        assert_eq!(proof.track_value_residual, 0.0);
        assert!(proof.mesh_position_residual < 1e-9);
    }

    #[test]
    fn sample_times_are_harvested_from_every_animated_track_not_only_translation() {
        // `payload_document`'s only clip animates rotation. Harvesting sample
        // times from translation tracks alone leaves `sample_time_count` at
        // zero, making every sampled obligation vacuously true.
        let doc = payload_document();
        let capability = complete_capability();
        let plan = whole_document_plan(&doc, &capability);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let proof = prove_scale(&doc, &candidate, &plan).unwrap();
        assert_eq!(proof.sample_time_count, 2);
    }

    // --- Base positions and bounds evidence -----------------------------

    /// One unskinned mesh instance and no skinned instance at all: the
    /// declared `base_mesh_positions` rewrite has no skinned bounds to be
    /// proved through.
    fn unskinned_document() -> Document {
        Document {
            skeleton: Skeleton {
                bones: vec![Bone {
                    name: "bone0".into(),
                    parent: None,
                    rest: Transform::IDENTITY,
                    inverse_bind: None,
                }],
            },
            clips: Vec::new(),
            assets: SceneAssets {
                meshes: vec![MeshAsset {
                    name: "mesh".into(),
                    source_mesh_index: 0,
                    primitives: vec![Primitive {
                        positions: vec![Vec3::new(1.0, 2.0, 3.0)],
                        ..Primitive::default()
                    }],
                }],
                instances: vec![MeshInstance {
                    source_node_index: 0,
                    node: 0,
                    mesh: 0,
                    skin_joints: Vec::new(),
                    skin_ibms: Vec::new(),
                }],
                ..SceneAssets::default()
            },
            source: Default::default(),
        }
    }

    #[test]
    fn an_unskinned_document_does_not_declare_a_bounds_obligation_it_cannot_check() {
        let doc = unskinned_document();
        let capability = complete_capability();
        let plan = whole_document_plan(&doc, &capability);
        assert!(!plan.proof_obligations().prove_bounds);
        assert!(plan.domain_rewrites().base_mesh_positions);
    }

    #[test]
    fn an_unskinned_documents_base_positions_are_proved_directly() {
        let doc = unskinned_document();
        let capability = complete_capability();
        let plan = whole_document_plan(&doc, &capability);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        assert!(
            (candidate.document().assets.meshes[0].primitives[0].positions[0]
                - Vec3::new(0.01, 0.02, 0.03))
            .length()
                < 1e-8
        );
        prove_scale(&doc, &candidate, &plan).unwrap();

        // A candidate that silently skipped the declared rewrite must fail,
        // even though there is no skinned instance and therefore no bounds.
        let mut unrewritten = candidate.document().clone();
        unrewritten.assets.meshes[0].primitives[0].positions[0] = Vec3::new(1.0, 2.0, 3.0);
        let unrewritten = ScaleCandidate {
            document: unrewritten,
        };
        assert!(matches!(
            prove_scale(&doc, &unrewritten, &plan).unwrap_err(),
            ScaleError::ProofResidualExceeded {
                kind: ProofResidualKind::MeshPosition,
                ..
            }
        ));
    }

    #[test]
    fn a_declared_bounds_obligation_with_no_skinned_evidence_is_missing_not_vacuous() {
        let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        let capability = complete_capability();
        let plan = whole_document_plan(&doc, &capability);
        assert!(plan.proof_obligations().prove_bounds);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        // Replay the plan against a document that no longer carries the
        // skinned instance its bounds obligation was declared from.
        let mut unskinned = doc.clone();
        unskinned.assets.instances[0].skin_joints.clear();
        unskinned.assets.instances[0].skin_ibms.clear();
        let error = prove_scale(&unskinned, &candidate, &plan).unwrap_err();
        let ScaleError::MissingProofEvidence { kind, detail } = error else {
            panic!("expected missing bounds evidence, got {error:?}");
        };
        assert_eq!(kind, ProofResidualKind::Bounds);
        // The `detail` is load-bearing, not decoration. Deleting the early
        // `has_skinned_evidence` gate leaves the later `skinned_bounds`
        // fallback returning the same *variant* and the same `kind` from a
        // different cause, so only the reason string distinguishes "the
        // document carries no skinned instance at all" from "it carries one
        // whose vertices produced no box".
        assert_eq!(detail, "no_skinned_instance_in_affected_closure");
    }

    #[test]
    fn a_source_skin_whose_vertices_are_all_unweighted_names_the_missing_source_bounds() {
        // The instance still declares an affected joint, so the early
        // `has_skinned_evidence` gate is satisfied and this reaches the
        // `skinned_bounds` fallback. What is missing is a vertex that
        // actually binds to that joint: a fully unweighted vertex is
        // legitimately excluded from bounds, and with the fixture's only
        // vertex excluded the source yields no box at all.
        let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        let capability = complete_capability();
        let plan = whole_document_plan(&doc, &capability);
        assert!(plan.proof_obligations().prove_bounds);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        prove_scale(&doc, &candidate, &plan).unwrap();

        let mut unweighted = doc.clone();
        unweighted.assets.meshes[0].primitives[0].weights[0] = [0.0; 4];
        assert_eq!(unweighted.assets.instances[0].skin_joints, vec![1]);
        let error = prove_scale(&unweighted, &candidate, &plan).unwrap_err();
        let ScaleError::MissingProofEvidence { kind, detail } = error else {
            panic!("expected missing bounds evidence, got {error:?}");
        };
        assert_eq!(kind, ProofResidualKind::Bounds);
        assert_eq!(detail, "source_bounds_missing");
    }

    #[test]
    fn a_candidate_skin_whose_vertices_are_all_unweighted_names_the_missing_candidate_bounds() {
        // Same shape as the source case, on the other side of the comparison:
        // vertex weights are not part of the structural parity
        // `validate_candidate_structure` enforces, so a candidate can reach
        // the bounds obligation carrying a box-less skin while the source
        // still has one. The two sides must not be reported interchangeably.
        let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        let capability = complete_capability();
        let plan = whole_document_plan(&doc, &capability);
        assert!(plan.proof_obligations().prove_bounds);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();

        let mut unweighted = candidate.document().clone();
        unweighted.assets.meshes[0].primitives[0].weights[0] = [0.0; 4];
        let unweighted = ScaleCandidate {
            document: unweighted,
        };
        let error = prove_scale(&doc, &unweighted, &plan).unwrap_err();
        let ScaleError::MissingProofEvidence { kind, detail } = error else {
            panic!("expected missing bounds evidence, got {error:?}");
        };
        assert_eq!(kind, ProofResidualKind::Bounds);
        assert_eq!(detail, "candidate_bounds_missing");
    }

    // --- Absent inverse-bind accessor through a rest/bind rebase ---------

    #[test]
    fn rest_bind_materializes_the_format_defined_identity_bind_it_must_conjugate() {
        // glTF's legal "skin omits `inverseBindMatrices`, every joint's bind
        // is identity" shape: an empty `skin_ibms` and no bone-level
        // convenience value. Rewriting only what is already stored touches
        // nothing here and silently emits `W' * B' = W * C * I != W * B`.
        let mut doc = compensated_document();
        doc.assets.instances[0].skin_ibms.clear();
        doc.assets.source_skeleton.skins[0].attachments = vec![SourceSkinAttachment {
            source_node_index: doc.assets.instances[0].source_node_index,
            source_mesh_index: Some(0),
        }];
        assert_eq!(
            doc.assets.source_skeleton.skins[0]
                .inverse_bind_accessor
                .status,
            SourceInverseBindAccessorStatus::Absent
        );
        let capability = complete_capability();
        let request = ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 0.01,
            },
            document: &doc,
            capability: &capability,
        };
        let plan = plan_scale(&request).unwrap();
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        // `B' = C^-1 * B = scale(s) * I`, written explicitly so the
        // candidate describes its own bind rather than relying on a format
        // default that is no longer identity.
        let ibms = &candidate.document().assets.instances[0].skin_ibms;
        assert_eq!(ibms.len(), 1);
        assert!(ibms[0].abs_diff_eq(Mat4::from_scale(Vec3::splat(0.01)), 1e-8));
        let proof = prove_scale(&doc, &candidate, &plan).unwrap();
        assert!(proof.skin_matrix_residual < 1e-6);
    }

    // --- Source-skeleton freshness and idempotence -----------------------

    #[test]
    fn rest_bind_rebases_the_raw_source_projection_alongside_the_skeleton() {
        let doc = compensated_document();
        let capability = complete_capability();
        let request = ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 0.01,
            },
            document: &doc,
            capability: &capability,
        };
        let plan = plan_scale(&request).unwrap();
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let SourceNodeLocalRest::Trs { scale, .. } =
            &candidate.document().assets.source_skeleton.nodes[0].local_rest
        else {
            panic!("expected a trs source rest");
        };
        assert!((*scale - Vec3::ONE).length() < 1e-6);

        // The whole point: re-planning the candidate with the identical
        // request must not be accepted and double-apply the factor.
        let replanned = ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 0.01,
            },
            document: candidate.document(),
            capability: &capability,
        };
        assert!(matches!(
            plan_scale(&replanned).unwrap_err(),
            ScaleError::FactorMismatch { .. }
        ));
    }

    /// The same compensated-scale relationship as [`compensated_document`],
    /// with every affected node's authored local rest declared as
    /// [`SourceNodeLocalRest::Matrix`] instead of `Trs` — the variant every
    /// other fixture in this module leaves unexercised, and the only one that
    /// reaches [`rebase_matrix`].
    ///
    /// ```text
    /// bone 0   parent -   scale(0.01)                   scaled root
    /// bone 1   parent 0   T(0, 100, 0) * diag(-1,-1,1)  the skin's joint
    /// bone 2   parent 1   T(0, 0, 50)                   transform-only child
    /// ```
    ///
    /// `diag(-1, -1, 1)` is the proper rotation by `pi` about z, so the
    /// linear parts stay orthogonal with a positive determinant and the
    /// domain classifies at `0.01`. The matching [`Bone::rest`] rotation is
    /// `Quat::from_rotation_z(PI)`, whose `f32` matrix differs from that
    /// literal by under `1e-7`.
    ///
    /// Rest-world facts: bone 1 has linear `0.01 * diag(-1, -1, 1)` and
    /// translation `(0, 1, 0)`; bone 2 adds `0.01 * (0, 0, 50)` for
    /// `(0, 1, 0.5)`.
    fn matrix_projection_document() -> Document {
        let nodes = vec![
            RigNode {
                parent: None,
                source_node_index: 0,
                translation: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                scale: Vec3::splat(0.01),
            },
            RigNode {
                parent: Some(0),
                source_node_index: 1,
                translation: Vec3::new(0.0, 100.0, 0.0),
                rotation: Quat::from_rotation_z(std::f32::consts::PI),
                scale: Vec3::ONE,
            },
            rig(Some(1), 2, Vec3::new(0.0, 0.0, 50.0)),
        ];
        // `B = inverse(W_rest(bone 1))`. With `R = diag(-1, -1, 1) = R^-1`,
        // `W = scale(0.01) * T(0, 100, 0) * R` has linear `0.01 * R` and
        // translation `(0, 1, 0)`, so `W^-1 = R * scale(100) * T(0, -1, 0)`:
        // linear `diag(-100, -100, 100)`, translation column `(0, 100, 0)`.
        let ibm = Mat4::from_cols(
            Vec4::new(-100.0, 0.0, 0.0, 0.0),
            Vec4::new(0.0, -100.0, 0.0, 0.0),
            Vec4::new(0.0, 0.0, 100.0, 0.0),
            Vec4::new(0.0, 100.0, 0.0, 1.0),
        );
        let mut doc = rig_document(&nodes, &[1], 0, ibm);
        let authored = [
            Mat4::from_cols(
                Vec4::new(0.01, 0.0, 0.0, 0.0),
                Vec4::new(0.0, 0.01, 0.0, 0.0),
                Vec4::new(0.0, 0.0, 0.01, 0.0),
                Vec4::new(0.0, 0.0, 0.0, 1.0),
            ),
            Mat4::from_cols(
                Vec4::new(-1.0, 0.0, 0.0, 0.0),
                Vec4::new(0.0, -1.0, 0.0, 0.0),
                Vec4::new(0.0, 0.0, 1.0, 0.0),
                Vec4::new(0.0, 100.0, 0.0, 1.0),
            ),
            Mat4::from_cols(
                Vec4::new(1.0, 0.0, 0.0, 0.0),
                Vec4::new(0.0, 1.0, 0.0, 0.0),
                Vec4::new(0.0, 0.0, 1.0, 0.0),
                Vec4::new(0.0, 0.0, 50.0, 1.0),
            ),
        ];
        for (node, matrix) in doc.assets.source_skeleton.nodes.iter_mut().zip(authored) {
            node.local_rest = SourceNodeLocalRest::Matrix(matrix);
        }
        doc
    }

    #[test]
    fn rest_bind_rebases_a_matrix_declared_source_projection_to_agree_with_the_skeleton() {
        // `rebase_matrix` implements the `SourceNodeLocalRest::Matrix` half of
        // the source-projection rewrite and is unreachable from a `Trs`
        // fixture, so nothing else here executes it. The shipped code is
        // correct — this pins it against the same
        // `L' = scale(s_parent) * L * scale(1 / s_node)` the `Trs` half
        // applies, including the fact that a uniform right-multiply scales
        // the three linear columns and leaves the translation column alone.
        let doc = matrix_projection_document();
        let capability = complete_capability();
        let request = ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 0.01,
            },
            document: &doc,
            capability: &capability,
        };
        let plan = plan_scale(&request).unwrap();
        assert_eq!(plan.affected_nodes(), &[0, 1, 2]);
        assert_eq!(plan.transform_only_attachments(), &[2]);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();

        // Hand-computed rewrites. Bone 0 is the scaled root, so
        // `s_parent = 1` and only its linear columns are divided by
        // `s = 0.01`: `scale(0.01) -> I`. Bones 1 and 2 have an affected
        // parent, so `s_parent = s_node = 0.01`: their linear columns are
        // multiplied and divided by the same factor and come out unchanged,
        // while their translation columns are multiplied by `0.01` alone —
        // `(0, 100, 0) -> (0, 1, 0)` and `(0, 0, 50) -> (0, 0, 0.5)`.
        let expected = [
            Mat4::IDENTITY,
            Mat4::from_cols(
                Vec4::new(-1.0, 0.0, 0.0, 0.0),
                Vec4::new(0.0, -1.0, 0.0, 0.0),
                Vec4::new(0.0, 0.0, 1.0, 0.0),
                Vec4::new(0.0, 1.0, 0.0, 1.0),
            ),
            Mat4::from_cols(
                Vec4::new(1.0, 0.0, 0.0, 0.0),
                Vec4::new(0.0, 1.0, 0.0, 0.0),
                Vec4::new(0.0, 0.0, 1.0, 0.0),
                Vec4::new(0.0, 0.0, 0.5, 1.0),
            ),
        ];
        for (index, expected) in expected.into_iter().enumerate() {
            let SourceNodeLocalRest::Matrix(matrix) =
                &candidate.document().assets.source_skeleton.nodes[index].local_rest
            else {
                panic!("an authored matrix source rest must stay a matrix");
            };
            assert!(
                matrix.abs_diff_eq(expected, 1e-6),
                "source node {index} rebased to {matrix:?}"
            );
            // And the rewritten projection describes the same local transform
            // as the rewritten normalized bone: the two halves of the rest
            // rewrite must not drift apart.
            let rest = candidate.document().skeleton.bones[index].rest;
            let bone_matrix =
                Mat4::from_scale_rotation_translation(rest.scale, rest.rotation, rest.translation);
            assert!(
                matrix.abs_diff_eq(bone_matrix, 1e-6),
                "source node {index} projection {matrix:?} disagrees with bone rest {bone_matrix:?}"
            );
        }

        // `B' = C^-1 * B = scale(s) * B`: linear `diag(-1, -1, 1)`,
        // translation `(0, 1, 0)`.
        let binds = &candidate.document().assets.instances[0].skin_ibms;
        assert!(
            binds[0].abs_diff_eq(
                Mat4::from_cols(
                    Vec4::new(-1.0, 0.0, 0.0, 0.0),
                    Vec4::new(0.0, -1.0, 0.0, 0.0),
                    Vec4::new(0.0, 0.0, 1.0, 0.0),
                    Vec4::new(0.0, 1.0, 0.0, 1.0),
                ),
                1e-5
            ),
            "rebased bind {:?}",
            binds[0]
        );

        let proof = prove_scale(&doc, &candidate, &plan).unwrap();
        assert!(proof.rest_translation_residual < 1e-4);
        assert!(proof.rest_rotation_residual < 1e-9);
        assert!(proof.unit_scale_residual < 1e-4);
        assert!(proof.transform_only_affine_residual < 1e-4);
        assert!(proof.skin_matrix_residual < 1e-4);
    }

    #[test]
    fn whole_document_conversion_rebases_the_raw_source_projection_too() {
        let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        let capability = complete_capability();
        let plan = whole_document_plan(&doc, &capability);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let SourceNodeLocalRest::Trs {
            translation, scale, ..
        } = &candidate.document().assets.source_skeleton.nodes[1].local_rest
        else {
            panic!("expected a trs source rest");
        };
        assert!((*translation - Vec3::new(0.0, 0.01, 0.0)).length() < 1e-8);
        // Dimensionless: a linear-unit conversion never touches it.
        assert_eq!(*scale, Vec3::ONE);
    }

    // --- Per-obligation falsifiability ----------------------------------
    //
    // DESIGN.md Appendix D §D.6 lists the claims proof must establish as
    // *separate* obligations. A suite in which every doctored candidate is
    // caught by whichever obligation happens to run first proves only that
    // *something* is checked, so each test below is built around a candidate
    // whose single defect is visible to one obligation and is asserted to
    // name exactly that [`ProofResidualKind`]. Turning the matching
    // obligation off is what must make each of these tests fail.

    /// Root, one skinned joint, and a leaf attachment carrying no skin, no
    /// mesh instance and no animation track: the only obligation that can
    /// observe that leaf at all is the rest-world translation check.
    fn rest_only_leaf_rig() -> Vec<RigNode> {
        vec![
            rig(None, 0, Vec3::ZERO),
            rig(Some(0), 1, Vec3::new(0.0, 1.0, 0.0)),
            rig(Some(0), 2, Vec3::new(3.0, 0.0, 0.0)),
        ]
    }

    #[test]
    fn an_un_rewritten_rest_translation_is_named_by_the_rest_translation_obligation() {
        let doc = rig_document(&rest_only_leaf_rig(), &[1], 0, Mat4::IDENTITY);
        let capability = complete_capability();
        let plan = whole_document_plan(&doc, &capability);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        // Analytic expectation: `(3, 0, 0) * 0.01`.
        assert!(
            (candidate.document().skeleton.bones[2].rest.translation - Vec3::new(0.03, 0.0, 0.0))
                .length()
                < 1e-8
        );
        let mut broken = candidate.document().clone();
        // A builder that skipped exactly one node's rest translation. The
        // leaf carries no skin slot, no mesh, and no track, so no sampled,
        // skin, or bounds obligation can see it.
        broken.skeleton.bones[2].rest.translation = Vec3::new(3.0, 0.0, 0.0);
        let broken = ScaleCandidate { document: broken };
        assert!(matches!(
            prove_scale(&doc, &broken, &plan).unwrap_err(),
            ScaleError::ProofResidualExceeded {
                kind: ProofResidualKind::RestTranslation,
                ..
            }
        ));
    }

    #[test]
    fn a_rest_translation_error_confined_to_z_is_still_named_by_the_rest_translation_obligation() {
        // The rest-translation residual is a three-component length, and
        // every other fixture's translation error has an x or y term, so a
        // residual that quietly dropped its z term would still be caught
        // everywhere else. This candidate keeps x and y at the analytically
        // expected `(3, 0, 0) * 0.01` and moves z alone; as in the test
        // above, the leaf carries no skin slot, mesh vertex or track, so the
        // rest-translation obligation is the only one that can see it at all.
        let doc = rig_document(&rest_only_leaf_rig(), &[1], 0, Mat4::IDENTITY);
        let capability = complete_capability();
        let plan = whole_document_plan(&doc, &capability);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let mut broken = candidate.document().clone();
        broken.skeleton.bones[2].rest.translation = Vec3::new(0.03, 0.0, 1.0);
        let broken = ScaleCandidate { document: broken };
        let error = prove_scale(&doc, &broken, &plan).unwrap_err();
        assert!(
            matches!(
                error,
                ScaleError::ProofResidualExceeded {
                    kind: ProofResidualKind::RestTranslation,
                    ..
                }
            ),
            "{error:?}"
        );
    }

    #[test]
    fn a_non_unit_composed_scale_on_an_affected_node_is_named_by_the_unit_scale_obligation() {
        let doc = compensated_document();
        let capability = complete_capability();
        let request = ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 0.01,
            },
            document: &doc,
            capability: &capability,
        };
        let plan = plan_scale(&request).unwrap();
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let mut broken = candidate.document().clone();
        // A leaf's own local scale does not move its own world origin and is
        // not the `rest.rotation` field the rotation obligation compares, so
        // the postcondition "unit composed scale for every affected node" is
        // the first obligation that can see it. Composed world scale becomes
        // `(2, 2, 2)`: a residual of `sqrt(3)` against a `1e-5` policy.
        broken.skeleton.bones[2].rest.scale = Vec3::splat(2.0);
        let broken = ScaleCandidate { document: broken };
        assert!(matches!(
            prove_scale(&doc, &broken, &plan).unwrap_err(),
            ScaleError::ProofResidualExceeded {
                kind: ProofResidualKind::UnitScale,
                ..
            }
        ));
    }

    #[test]
    fn a_composed_scale_anomaly_confined_to_z_is_still_named_by_the_unit_scale_obligation() {
        // The postcondition residual sums a per-axis deviation from one, and
        // no other fixture puts a composed-scale anomaly on z alone, so a
        // residual that dropped its z axis would still be caught everywhere
        // else. Here the composed x and y scales stay one and only z becomes
        // two: dropping z reports `0.0` and hands the candidate on to a
        // *different* obligation, which is why this test names the kind
        // rather than merely asserting a rejection.
        let doc = compensated_document();
        let capability = complete_capability();
        let request = ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 0.01,
            },
            document: &doc,
            capability: &capability,
        };
        let plan = plan_scale(&request).unwrap();
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let mut broken = candidate.document().clone();
        broken.skeleton.bones[2].rest.scale = Vec3::new(1.0, 1.0, 2.0);
        let broken = ScaleCandidate { document: broken };
        let error = prove_scale(&doc, &broken, &plan).unwrap_err();
        assert!(
            matches!(
                error,
                ScaleError::ProofResidualExceeded {
                    kind: ProofResidualKind::UnitScale,
                    ..
                }
            ),
            "{error:?}"
        );
    }

    #[test]
    fn a_transform_only_attachment_with_a_correct_origin_but_a_wrong_linear_part_still_fails() {
        // DESIGN.md Appendix D §D.6 requires proving "the analytically
        // expected full world affine of a transform-only attached child ...
        // so a no-op cannot pass". The existing stale-attachment fixture is
        // already caught by the rest-*translation* check, which proves
        // nothing about the linear part. This candidate keeps the
        // attachment's world origin exactly right and its composed scale
        // exactly one — so `RestTranslation`, `RestRotation` and `UnitScale`
        // all pass — while flipping two axes of its linear part. Only
        // transforming an off-origin point through the complete affine can
        // see it.
        //
        // A *magnitude* error in the linear part would be caught first by
        // the unit-scale postcondition; `diag(-1, -1, 1)` is a proper
        // rotation by pi about z, so every column length stays one and the
        // determinant stays positive.
        let doc = compensated_document();
        let capability = complete_capability();
        let request = ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 0.01,
            },
            document: &doc,
            capability: &capability,
        };
        let plan = plan_scale(&request).unwrap();
        assert_eq!(plan.transform_only_attachments(), &[2]);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let mut broken = candidate.document().clone();
        broken.skeleton.bones[2].rest.scale = Vec3::new(-1.0, -1.0, 1.0);
        let broken = ScaleCandidate { document: broken };
        assert!(matches!(
            prove_scale(&doc, &broken, &plan).unwrap_err(),
            ScaleError::ProofResidualExceeded {
                kind: ProofResidualKind::TransformOnlyAffine,
                ..
            }
        ));
    }

    #[test]
    fn an_inverse_bind_whose_linear_part_was_not_conjugated_is_named_by_the_skin_obligation() {
        let mut doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        // The single skinned vertex sits at the geometry origin, so
        // `(W * B) * p` reduces to the translation column of `W * B` and the
        // bounds obligation is analytically blind to a change confined to
        // `B`'s linear part.
        doc.assets.meshes[0].primitives[0].positions[0] = Vec3::ZERO;
        let capability = complete_capability();
        let plan = whole_document_plan(&doc, &capability);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let mut broken = candidate.document().clone();
        broken.assets.instances[0].skin_ibms[0].x_axis = Vec4::new(2.0, 0.0, 0.0, 0.0);
        let broken = ScaleCandidate { document: broken };
        assert!(matches!(
            prove_scale(&doc, &broken, &plan).unwrap_err(),
            ScaleError::ProofResidualExceeded {
                kind: ProofResidualKind::SkinMatrix,
                ..
            }
        ));
    }

    #[test]
    fn a_rest_scale_that_only_shows_up_under_animation_is_named_by_the_trajectory_obligation() {
        // Bone 0 is the only skin joint; bones 1 and 2 are transform-only
        // descendants whose *rest* translations are both zero, so doctoring
        // bone 1's rest scale moves nothing at rest, nothing in the skin
        // equation, and nothing in any stored track value — it only shows up
        // once bone 2's translation track drives it off its parent's origin.
        let nodes = vec![
            rig(None, 0, Vec3::ZERO),
            rig(Some(0), 1, Vec3::ZERO),
            rig(Some(1), 2, Vec3::ZERO),
        ];
        let mut doc = rig_document(&nodes, &[0], 0, Mat4::IDENTITY);
        doc.clips.push(Clip {
            name: "clip".into(),
            duration_s: 1.0,
            tracks: vec![Track {
                bone: 2,
                property: Property::Translation,
                interpolation: Interpolation::Linear,
                times: vec![0.0, 1.0],
                values: TrackValues::Vec3s(vec![
                    Vec3::new(1.0, 0.0, 0.0),
                    Vec3::new(2.0, 0.0, 0.0),
                ]),
            }],
        });
        let capability = complete_capability();
        let plan = whole_document_plan(&doc, &capability);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let mut broken = candidate.document().clone();
        broken.skeleton.bones[1].rest.scale = Vec3::splat(2.0);
        let broken = ScaleCandidate { document: broken };
        assert!(matches!(
            prove_scale(&doc, &broken, &plan).unwrap_err(),
            ScaleError::ProofResidualExceeded {
                kind: ProofResidualKind::Trajectory,
                ..
            }
        ));
    }

    /// A `CUBICSPLINE` translation segment whose two key values are both the
    /// origin and whose out/in tangents are equal and large.
    ///
    /// The two properties this fixture exists for are analytic:
    ///
    /// * at either key time the sampled value is exactly the key value, so a
    ///   *tangent* perturbation is invisible to the key-time obligations;
    /// * at the segment midpoint the Hermite basis contributes
    ///   `h10 * dt * m0 + h11 * dt * m1 = 0.125 * m0 - 0.125 * m1`, which is
    ///   exactly zero while `m0 == m1` — so a perturbation `d` of one
    ///   tangent moves the sampled midpoint by `0.125 * d` away from a zero
    ///   expectation, where the policy tolerance is only `1e-6`.
    ///
    /// A tangent magnitude of `1000` makes the *element-wise* `TrackValue`
    /// tolerance `1e-6 + 1e-5 * 1000 = 1.0001e-2`, so a `1e-3` perturbation
    /// is comfortably inside it and comfortably outside the `1.25e-4`
    /// midpoint residual it produces. That gap is what lets these two tests
    /// isolate the sampled obligations from the element-wise one.
    fn flat_cubic_translation_track() -> Track {
        Track {
            bone: 1,
            property: Property::Translation,
            interpolation: Interpolation::CubicSpline,
            times: vec![0.0, 1.0],
            values: TrackValues::Vec3s(vec![
                Vec3::ZERO,                  // in-tangent @0
                Vec3::ZERO,                  // value @0
                Vec3::new(0.0, 1000.0, 0.0), // out-tangent @0 (`m0`)
                Vec3::new(0.0, 1000.0, 0.0), // in-tangent @1 (`m1`)
                Vec3::ZERO,                  // value @1
                Vec3::ZERO,                  // out-tangent @1
            ]),
        }
    }

    fn identity_conversion_plan(
        document: &Document,
        capability: &ScaleCapabilityFacts,
    ) -> ScalePlan {
        plan_scale(&ScaleRequest {
            operation: ScaleOperation::WholeDocumentLinearUnits { factor: 1.0 },
            document,
            capability,
        })
        .unwrap()
    }

    #[test]
    fn a_cubic_tangent_error_inside_element_tolerance_is_named_by_the_cubic_interior_obligation() {
        let mut doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        doc.clips.push(Clip {
            name: "clip".into(),
            duration_s: 1.0,
            tracks: vec![flat_cubic_translation_track()],
        });
        let capability = complete_capability();
        let plan = identity_conversion_plan(&doc, &capability);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let mut broken = candidate.document().clone();
        let TrackValues::Vec3s(values) = &mut broken.clips[0].tracks[0].values else {
            panic!("expected a vec3 track");
        };
        values[2].y = 1000.001;
        let broken = ScaleCandidate { document: broken };
        assert!(matches!(
            prove_scale(&doc, &broken, &plan).unwrap_err(),
            ScaleError::ProofResidualExceeded {
                kind: ProofResidualKind::CubicInterior,
                ..
            }
        ));
    }

    #[test]
    fn the_same_tangent_error_at_a_harvested_key_time_is_named_by_the_key_obligation() {
        // Identical defect to the test above, but a rotation track keyed at
        // `0.5` promotes the segment midpoint to a *key* time. Key times are
        // proved before cubic interiors, so this candidate is named by
        // `KeyTranslation` — which is otherwise unreachable, since at a
        // segment's own key times the sampled value is the stored value the
        // element-wise `TrackValue` check already owns.
        let mut doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        doc.clips.push(Clip {
            name: "clip".into(),
            duration_s: 1.0,
            tracks: vec![
                flat_cubic_translation_track(),
                Track {
                    bone: 1,
                    property: Property::Rotation,
                    interpolation: Interpolation::Linear,
                    times: vec![0.5],
                    values: TrackValues::Quats(vec![Quat::from_rotation_y(0.3)]),
                },
            ],
        });
        let capability = complete_capability();
        let plan = identity_conversion_plan(&doc, &capability);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let mut broken = candidate.document().clone();
        let TrackValues::Vec3s(values) = &mut broken.clips[0].tracks[0].values else {
            panic!("expected a vec3 track");
        };
        values[2].y = 1000.001;
        let broken = ScaleCandidate { document: broken };
        assert!(matches!(
            prove_scale(&doc, &broken, &plan).unwrap_err(),
            ScaleError::ProofResidualExceeded {
                kind: ProofResidualKind::KeyTranslation,
                ..
            }
        ));
    }

    // --- Multi-joint, multi-vertex, animated rest/bind fixture ------------

    /// The multi-joint half of DESIGN.md Appendix D §D.6's "analytic
    /// one-joint **and multi-joint** fixtures", with the §D.4 translation
    /// animation domain actually populated.
    ///
    /// Two compensated joints (`0.01` at every rest-world linear part), four
    /// vertices — two singly weighted, one genuinely blended `0.25 / 0.75`,
    /// and one whose weights sum to `0.8` so the normalisation step in
    /// [`skinned_bounds`] is exercised — plus a `LINEAR` translation track
    /// on one joint and a `CUBICSPLINE` translation track (with non-zero
    /// tangents) on the other, so key, cubic-interior, trajectory, skin and
    /// bounds obligations all have something to evaluate.
    ///
    /// Analytic facts, all hand-derived:
    ///
    /// ```text
    /// W1(rest) = [0.01 I | (0, 1, 0)]   B1 = [100 I | (0, -100, 0)]
    /// W2(rest) = [0.01 I | (0, 2, 0)]   B2 = [100 I | (0, -200, 0)]
    /// ```
    ///
    /// so `W_i(rest) * B_i == I` for both joints (geometry bind `G = I`).
    fn multi_joint_document() -> Document {
        let nodes = vec![
            RigNode {
                parent: None,
                source_node_index: 0,
                translation: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                scale: Vec3::splat(0.01),
            },
            rig(Some(0), 1, Vec3::new(0.0, 100.0, 0.0)),
            rig(Some(1), 2, Vec3::new(0.0, 100.0, 0.0)),
        ];
        let mut doc = rig_document(&nodes, &[1, 2], 0, Mat4::IDENTITY);
        doc.assets.instances[0].skin_ibms = vec![
            Mat4::from_scale_rotation_translation(
                Vec3::splat(100.0),
                Quat::IDENTITY,
                Vec3::new(0.0, -100.0, 0.0),
            ),
            Mat4::from_scale_rotation_translation(
                Vec3::splat(100.0),
                Quat::IDENTITY,
                Vec3::new(0.0, -200.0, 0.0),
            ),
        ];
        doc.assets.meshes[0].primitives[0] = Primitive {
            positions: vec![
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(-1.0, 0.0, 2.0),
                Vec3::new(0.0, 2.0, 0.0),
                Vec3::new(0.0, -3.0, 0.0),
            ],
            joints: vec![[0, 0, 0, 0], [1, 0, 0, 0], [0, 1, 0, 0], [0, 1, 0, 0]],
            weights: vec![
                [1.0, 0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0, 0.0],
                [0.25, 0.75, 0.0, 0.0],
                // Deliberately sums to `0.8`, not `1.0`.
                [0.4, 0.4, 0.0, 0.0],
            ],
            ..Primitive::default()
        };
        doc.clips.push(Clip {
            name: "clip".into(),
            duration_s: 1.0,
            tracks: vec![
                Track {
                    bone: 1,
                    property: Property::Translation,
                    interpolation: Interpolation::Linear,
                    times: vec![0.0, 1.0],
                    values: TrackValues::Vec3s(vec![
                        Vec3::new(0.0, 100.0, 0.0),
                        Vec3::new(0.0, 200.0, 0.0),
                    ]),
                },
                Track {
                    bone: 2,
                    property: Property::Translation,
                    interpolation: Interpolation::CubicSpline,
                    times: vec![0.0, 1.0],
                    values: TrackValues::Vec3s(vec![
                        Vec3::ZERO,                 // in-tangent @0
                        Vec3::new(0.0, 100.0, 0.0), // value @0
                        Vec3::new(0.0, 60.0, 0.0),  // out-tangent @0
                        Vec3::new(0.0, 60.0, 0.0),  // in-tangent @1
                        Vec3::new(0.0, 300.0, 0.0), // value @1
                        Vec3::ZERO,                 // out-tangent @1
                    ]),
                },
            ],
        });
        doc
    }

    fn multi_joint_plan(document: &Document, capability: &ScaleCapabilityFacts) -> ScalePlan {
        plan_scale(&ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 0.01,
            },
            document,
            capability,
        })
        .unwrap()
    }

    #[test]
    fn skinned_bounds_blends_and_normalises_multi_joint_weights() {
        // Hand-written animated joint worlds, so nothing here is recomputed
        // by the code under test. At these poses the two joint palettes are
        //
        //     M1 = W1 * B1 = [I | (0, 1, 0)]
        //     M2 = W2 * B2 = [I | (0, 3, 0)]
        //
        // and the four vertices skin to
        //
        //     (1, 0, 0)  -> M1                                 = ( 1,  1, 0)
        //     (-1, 0, 2) -> M2                                 = (-1,  3, 2)
        //     (0, 2, 0)  -> 0.25 * (0, 3, 0) + 0.75 * (0, 5, 0) = ( 0, 4.5, 0)
        //     (0, -3, 0) -> (0.4 * (0, -2, 0) + 0.4 * (0, 0, 0)) / 0.8
        //                                                       = ( 0, -1, 0)
        let doc = multi_joint_document();
        let worlds = vec![
            Mat4::from_scale(Vec3::splat(0.01)),
            Mat4::from_scale_rotation_translation(
                Vec3::splat(0.01),
                Quat::IDENTITY,
                Vec3::new(0.0, 2.0, 0.0),
            ),
            Mat4::from_scale_rotation_translation(
                Vec3::splat(0.01),
                Quat::IDENTITY,
                Vec3::new(0.0, 5.0, 0.0),
            ),
        ];
        let affected: BTreeSet<BoneId> = [0, 1, 2].into_iter().collect();
        let (min, max) = skinned_bounds(&doc, &worlds, &affected)
            .unwrap()
            .expect("multi-joint fixture has weighted vertices");
        assert!(
            (min - Vec3::new(-1.0, -1.0, 0.0)).length() < 1e-5,
            "min {min:?}"
        );
        assert!(
            (max - Vec3::new(1.0, 4.5, 2.0)).length() < 1e-5,
            "max {max:?}"
        );
    }

    #[test]
    fn rest_bind_rebases_translation_tracks_and_proves_every_sampled_obligation() {
        let doc = multi_joint_document();
        let capability = complete_capability();
        let plan = multi_joint_plan(&doc, &capability);
        assert_eq!(plan.affected_nodes(), &[0, 1, 2]);
        let obligations = plan.proof_obligations();
        assert!(obligations.prove_keys);
        assert!(obligations.prove_cubic_interiors);
        assert!(obligations.prove_trajectories);
        assert!(obligations.prove_bounds);

        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let clip = &candidate.document().clips[0];

        // Both tracks sit on a node whose parent is itself affected, so the
        // parent-basis multiplier of DESIGN.md Appendix D §D.2 is `s = 0.01`
        // for values *and* both cubic tangents.
        let TrackValues::Vec3s(linear) = &clip.tracks[0].values else {
            panic!("expected a vec3 track");
        };
        let expected_linear = [Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 2.0, 0.0)];
        for (value, expected) in linear.iter().zip(expected_linear) {
            assert!((*value - expected).length() < 1e-6, "{value:?}");
        }
        let TrackValues::Vec3s(cubic) = &clip.tracks[1].values else {
            panic!("expected a vec3 track");
        };
        let expected_cubic = [
            Vec3::ZERO,
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.6, 0.0),
            Vec3::new(0.0, 0.6, 0.0),
            Vec3::new(0.0, 3.0, 0.0),
            Vec3::ZERO,
        ];
        for (value, expected) in cubic.iter().zip(expected_cubic) {
            assert!((*value - expected).length() < 1e-6, "{value:?}");
        }

        // Both rebased binds are hand-derived: `B' = C^-1 * B = scale(s) * B`.
        let binds = &candidate.document().assets.instances[0].skin_ibms;
        assert!(binds[0].abs_diff_eq(Mat4::from_translation(Vec3::new(0.0, -1.0, 0.0)), 1e-5));
        assert!(binds[1].abs_diff_eq(Mat4::from_translation(Vec3::new(0.0, -2.0, 0.0)), 1e-5));

        let proof = prove_scale(&doc, &candidate, &plan).unwrap();
        // Two key times plus the analytic midpoint of the one cubic segment.
        assert_eq!(proof.sample_time_count, 3);
        assert!(proof.key_translation_residual < 1e-4);
        assert!(proof.cubic_interior_residual < 1e-4);
        assert!(proof.trajectory_residual < 1e-4);
        assert!(proof.skin_matrix_residual < 1e-4);
        assert!(proof.bounds_residual < 1e-4);
    }

    #[test]
    fn a_reweighted_vertex_is_named_by_the_bounds_obligation() {
        // Per-vertex skin weights are the one rewritten-document payload no
        // other obligation reads: they do not appear in a track value, a
        // base `POSITION`, a world matrix, or `W * B`. At rest both joint
        // palettes are the identity, so this candidate is only distinguished
        // once the clip drives the two joints apart.
        let doc = multi_joint_document();
        let capability = complete_capability();
        let plan = multi_joint_plan(&doc, &capability);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let mut broken = candidate.document().clone();
        broken.assets.meshes[0].primitives[0].weights[2] = [0.75, 0.25, 0.0, 0.0];
        let broken = ScaleCandidate { document: broken };
        assert!(matches!(
            prove_scale(&doc, &broken, &plan).unwrap_err(),
            ScaleError::ProofResidualExceeded {
                kind: ProofResidualKind::Bounds,
                ..
            }
        ));
    }

    // --- Capability gate ---------------------------------------------------

    /// One row of the capability-gate table: the flag's name and the single
    /// mutation it applies to an otherwise complete projection.
    type CapabilityDomainCase = (&'static str, fn(&mut ScaleCapabilityFacts));

    #[test]
    fn every_unsupported_capability_domain_rejects_planning_on_its_own() {
        // DESIGN.md Appendix D §D.4: every unmodeled domain fails closed.
        // One flag at a time, against an otherwise complete projection, so a
        // dropped clause cannot hide behind a sibling.
        let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        let domains: [CapabilityDomainCase; 14] = [
            ("morphs_present", |f| f.morphs_present = true),
            ("morph_weights_present", |f| f.morph_weights_present = true),
            ("cameras_present", |f| f.cameras_present = true),
            ("lights_present", |f| f.lights_present = true),
            ("instancing_present", |f| f.instancing_present = true),
            ("unregistered_extensions_present", |f| {
                f.unregistered_extensions_present = true
            }),
            ("extras_present", |f| f.extras_present = true),
            ("unknown_source_members_present", |f| {
                f.unknown_source_members_present = true
            }),
            ("non_triangle_primitives_present", |f| {
                f.non_triangle_primitives_present = true
            }),
            ("unsupported_vertex_attributes_present", |f| {
                f.unsupported_vertex_attributes_present = true
            }),
            ("secondary_skin_influences_present", |f| {
                f.secondary_skin_influences_present = true
            }),
            ("inverse_bind_issues_present", |f| {
                f.inverse_bind_issues_present = true
            }),
            ("unsafe_accessor_layout_present", |f| {
                f.unsafe_accessor_layout_present = true
            }),
            ("external_resources_present", |f| {
                f.external_resources_present = true
            }),
        ];
        let operations = [
            ScaleOperation::WholeDocumentLinearUnits { factor: 0.01 },
            ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 1.0,
            },
        ];
        for (name, set_flag) in domains {
            let mut capability = complete_capability();
            set_flag(&mut capability);
            assert!(!capability.is_supported(), "{name} must not be supported");
            for operation in operations {
                let request = ScaleRequest {
                    operation,
                    document: &doc,
                    capability: &capability,
                };
                assert!(
                    matches!(
                        plan_scale(&request).unwrap_err(),
                        ScaleError::IncompleteCapability
                    ),
                    "{name} must reject {operation:?}"
                );
            }
        }
        // The complete projection these were derived from is genuinely
        // supported, so each rejection above is attributable to its own flag.
        assert!(complete_capability().is_supported());
    }

    // --- Tolerance policy identity -----------------------------------------

    #[test]
    fn the_appendix_d_v1_tolerance_identity_is_pinned_through_plan_and_proof() {
        // DESIGN.md Appendix D §D.1/§D.6: producers record this identity and
        // these thresholds in evidence, so a change to either is a new policy
        // identity rather than a silent retune.
        fn assert_appendix_d_v1(policy: ScaleTolerancePolicy) {
            assert_eq!(policy.id, "appendix-d-v1");
            assert_eq!(policy.relative_orthogonality, 1e-5);
            assert_eq!(policy.equal_axis, 1e-5);
            assert_eq!(policy.common_factor, 1e-5);
            assert_eq!(policy.singular_determinant_relative, 1e-6);
            assert_eq!(policy.scalar_absolute, 1e-6);
            assert_eq!(policy.scalar_relative, 1e-5);
            assert_eq!(policy.rotation_residual_radians, 1e-5);
            assert_eq!(policy.postcondition_unit_scale_residual, 1e-5);
            // `abs_error <= 1e-6 + 1e-5 * max(abs(before), abs(after))`, at a
            // hand-computed operand pair.
            assert!((policy.scalar_tolerance(0.0, 100.0) - 0.001_001).abs() < 1e-12);
        }

        let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        let capability = complete_capability();

        let whole_document = whole_document_plan(&doc, &capability);
        assert_appendix_d_v1(whole_document.tolerance_policy());
        let candidate = build_scale_candidate(&doc, &whole_document).unwrap();
        let proof = prove_scale(&doc, &candidate, &whole_document).unwrap();
        assert_appendix_d_v1(proof.tolerance_policy);

        let rest_bind = plan_scale(&ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 1.0,
            },
            document: &doc,
            capability: &capability,
        })
        .unwrap();
        assert_appendix_d_v1(rest_bind.tolerance_policy());
        let candidate = build_scale_candidate(&doc, &rest_bind).unwrap();
        let proof = prove_scale(&doc, &candidate, &rest_bind).unwrap();
        assert_appendix_d_v1(proof.tolerance_policy);
    }

    // --- Invalid declared rest/bind factor ---------------------------------

    #[test]
    fn a_non_positive_or_non_finite_expected_factor_is_invalid_not_a_factor_mismatch() {
        // The unit rig's observed common factor is one, so a request that
        // slipped past this guard would come back as `FactorMismatch` — a
        // materially different claim ("your rig is not what you declared")
        // from "your declared factor is not a factor".
        let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        let capability = complete_capability();
        for factor in [0.0, -1.0, f64::NAN] {
            let request = ScaleRequest {
                operation: ScaleOperation::RestBindUniformScale {
                    source_skin_index: 0,
                    source_root_node_index: 0,
                    expected_factor: factor,
                },
                document: &doc,
                capability: &capability,
            };
            match plan_scale(&request).unwrap_err() {
                ScaleError::InvalidExpectedFactor { factor: rejected } => {
                    assert_eq!(rejected.is_nan(), factor.is_nan(), "{factor}");
                    if !factor.is_nan() {
                        assert_eq!(rejected, factor);
                    }
                }
                other => panic!("expected InvalidExpectedFactor for {factor}, got {other:?}"),
            }
        }
    }

    // --- Non-identity inverse binds ----------------------------------------

    /// `4 * Rz(pi/2)` with a non-zero translation column: a linear part that
    /// is neither identity nor a pure rotation, so `B' = U B U^-1` is a
    /// genuinely different claim from "scale every component".
    const NON_IDENTITY_BIND: Mat4 = Mat4::from_cols(
        Vec4::new(0.0, 4.0, 0.0, 0.0),
        Vec4::new(-4.0, 0.0, 0.0, 0.0),
        Vec4::new(0.0, 0.0, 4.0, 0.0),
        Vec4::new(5.0, -6.0, 7.0, 1.0),
    );

    #[test]
    fn whole_document_conversion_conjugates_a_non_identity_bind_and_the_bone_convenience_value() {
        let mut doc = rig_document(&unit_rig(), &[1], 0, NON_IDENTITY_BIND);
        // Every other fixture leaves this `None`, so the bone-level rewrite
        // branch never executes.
        doc.skeleton.bones[1].inverse_bind = Some(NON_IDENTITY_BIND);
        let capability = complete_capability();
        let plan = whole_document_plan(&doc, &capability);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();

        // `U B U^-1` for a uniform `U = scale(0.01)`: the translation column
        // is multiplied by `q`, the dimensionless linear part is untouched.
        for (label, converted) in [
            (
                "instance",
                candidate.document().assets.instances[0].skin_ibms[0],
            ),
            (
                "bone",
                candidate.document().skeleton.bones[1]
                    .inverse_bind
                    .expect("bone bind is retained"),
            ),
        ] {
            assert_eq!(converted.x_axis, Vec4::new(0.0, 4.0, 0.0, 0.0), "{label}");
            assert_eq!(converted.y_axis, Vec4::new(-4.0, 0.0, 0.0, 0.0), "{label}");
            assert_eq!(converted.z_axis, Vec4::new(0.0, 0.0, 4.0, 0.0), "{label}");
            assert!(
                converted
                    .w_axis
                    .abs_diff_eq(Vec4::new(0.05, -0.06, 0.07, 1.0), 1e-7),
                "{label}: {:?}",
                converted.w_axis
            );
        }
        let proof = prove_scale(&doc, &candidate, &plan).unwrap();
        assert!(proof.skin_matrix_residual < 1e-4);
    }

    #[test]
    fn rest_bind_rewrites_the_bone_convenience_inverse_bind_it_falls_back_to() {
        // `skin_ibms` is empty, so the documented fallback chain resolves the
        // joint's bind through `Bone::inverse_bind` — the value every other
        // fixture leaves `None`.
        let nodes = vec![
            RigNode {
                parent: None,
                source_node_index: 0,
                translation: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                scale: Vec3::splat(0.01),
            },
            rig(Some(0), 1, Vec3::new(0.0, 100.0, 0.0)),
        ];
        let mut doc = rig_document(&nodes, &[1], 0, Mat4::IDENTITY);
        doc.assets.instances[0].skin_ibms.clear();
        // `W1(rest) = [0.01 I | (0, 1, 0)]`, so its exact inverse bind is
        // `[100 I | (0, -100, 0)]`.
        doc.skeleton.bones[1].inverse_bind = Some(Mat4::from_scale_rotation_translation(
            Vec3::splat(100.0),
            Quat::IDENTITY,
            Vec3::new(0.0, -100.0, 0.0),
        ));
        let capability = complete_capability();
        let plan = plan_scale(&ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 0.01,
            },
            document: &doc,
            capability: &capability,
        })
        .unwrap();
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        // `B' = C^-1 * B = scale(0.01) * [100 I | (0, -100, 0)]`.
        let expected = Mat4::from_translation(Vec3::new(0.0, -1.0, 0.0));
        let bone_bind = candidate.document().skeleton.bones[1]
            .inverse_bind
            .expect("bone bind is retained");
        assert!(bone_bind.abs_diff_eq(expected, 1e-5), "{bone_bind:?}");
        let materialized = &candidate.document().assets.instances[0].skin_ibms;
        assert_eq!(materialized.len(), 1);
        assert!(materialized[0].abs_diff_eq(expected, 1e-5));
        let proof = prove_scale(&doc, &candidate, &plan).unwrap();
        assert!(proof.skin_matrix_residual < 1e-4);
    }

    // --- Stable reason strings ---------------------------------------------

    fn rest_bind_reject_reason(document: &Document) -> ScaleError {
        let capability = complete_capability();
        plan_scale(&ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 1.0,
            },
            document,
            capability: &capability,
        })
        .unwrap_err()
    }

    #[test]
    fn a_skin_joint_with_no_source_node_projection_names_its_own_closure_reason() {
        let mut doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        doc.assets.source_skeleton.skins[0]
            .joint_source_node_indices
            .push(99);
        assert_eq!(
            rest_bind_reject_reason(&doc),
            ScaleError::IncompleteClosure {
                reason: "skin_joint_source_node_missing"
            }
        );
    }

    #[test]
    fn a_descendant_claimed_as_a_joint_by_another_skin_names_its_own_closure_reason() {
        let nodes = vec![
            rig(None, 0, Vec3::ZERO),
            rig(Some(0), 1, Vec3::new(0.0, 1.0, 0.0)),
            rig(Some(1), 2, Vec3::new(0.0, 1.0, 0.0)),
        ];
        let mut doc = rig_document(&nodes, &[1], 0, Mat4::IDENTITY);
        doc.assets.source_skeleton.skins.push(SourceSkinAsset {
            source_skin_index: 1,
            name: None,
            skeleton_root_source_node_index: None,
            joint_source_node_indices: vec![2],
            inverse_bind_accessor: SourceInverseBindAccessor::default(),
            attachments: Vec::new(),
        });
        assert_eq!(
            rest_bind_reject_reason(&doc),
            ScaleError::IncompleteClosure {
                reason: "descendant_joint_of_another_skin"
            }
        );
    }

    #[test]
    fn a_joint_ancestor_chain_that_never_reaches_the_root_names_its_own_closure_reason() {
        // Source nodes 1 and 2 name each other as parent, so walking joint
        // 2's ancestor chain toward the declared root never terminates.
        let nodes = vec![
            rig(None, 0, Vec3::ZERO),
            rig(Some(0), 1, Vec3::new(0.0, 1.0, 0.0)),
            rig(Some(1), 2, Vec3::new(0.0, 1.0, 0.0)),
        ];
        let mut doc = rig_document(&nodes, &[2], 0, Mat4::IDENTITY);
        doc.assets.source_skeleton.nodes[1].parent_source_node_index = Some(2);
        assert_eq!(
            rest_bind_reject_reason(&doc),
            ScaleError::IncompleteClosure {
                reason: "cyclic_or_unbounded_source_parent_chain"
            }
        );
    }

    #[test]
    fn a_cyclic_rest_world_parent_chain_names_its_own_closure_reason() {
        // The closure itself completes — joint 1 reaches the declared root
        // in one hop — but composing the root's own rest-world matrix walks
        // *above* the closure and finds the root naming its own descendant
        // as parent.
        let nodes = vec![
            rig(None, 0, Vec3::ZERO),
            rig(Some(0), 1, Vec3::new(0.0, 1.0, 0.0)),
            rig(Some(0), 2, Vec3::new(0.0, 1.0, 0.0)),
        ];
        let mut doc = rig_document(&nodes, &[1], 0, Mat4::IDENTITY);
        doc.assets.source_skeleton.nodes[0].parent_source_node_index = Some(2);
        assert_eq!(
            rest_bind_reject_reason(&doc),
            ScaleError::IncompleteClosure {
                reason: "cyclic_source_parent_chain"
            }
        );
    }

    #[test]
    fn a_rest_world_ancestor_outside_the_projection_names_its_own_closure_reason() {
        // The scaled root declares an ancestor the source-node projection
        // does not carry, so its true rest-world linear part is unknowable.
        let mut doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        doc.assets.source_skeleton.nodes[0].parent_source_node_index = Some(99);
        assert_eq!(
            rest_bind_reject_reason(&doc),
            ScaleError::IncompleteClosure {
                reason: "missing_source_node"
            }
        );
    }

    /// One row of the candidate-structure table: the stable reason the
    /// mismatch must be named by, and the doctoring that produces it.
    type StructureMismatchCase = (&'static str, fn(&mut Document));

    #[test]
    fn every_candidate_structure_mismatch_names_its_own_reason() {
        let mut doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        doc.clips.push(Clip {
            name: "clip".into(),
            duration_s: 1.0,
            tracks: vec![Track {
                bone: 1,
                property: Property::Translation,
                interpolation: Interpolation::Linear,
                times: vec![0.0, 1.0],
                values: TrackValues::Vec3s(vec![Vec3::ZERO, Vec3::ONE]),
            }],
        });
        let capability = complete_capability();
        let plan = whole_document_plan(&doc, &capability);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();

        let cases: [StructureMismatchCase; 8] = [
            ("track_count_mismatch", |d| {
                d.clips[0].tracks.push(Track {
                    bone: 1,
                    property: Property::Rotation,
                    interpolation: Interpolation::Linear,
                    times: vec![0.0],
                    values: TrackValues::Quats(vec![Quat::IDENTITY]),
                })
            }),
            ("track_shape_mismatch", |d| {
                d.clips[0].tracks[0].interpolation = Interpolation::Step
            }),
            // The remaining track-identity clauses, each doctored on its
            // own: a track retargeted to the sibling bone, and one
            // rebranded onto the other `Vec3`-valued channel. Both leave
            // track count, times and value count untouched, so only the
            // clause under test can name the mismatch.
            ("track_shape_mismatch", |d| d.clips[0].tracks[0].bone = 0),
            ("track_shape_mismatch", |d| {
                d.clips[0].tracks[0].property = Property::Scale
            }),
            ("instance_count_mismatch", |d| {
                let extra = d.assets.instances[0].clone();
                d.assets.instances.push(extra);
            }),
            ("mesh_count_mismatch", |d| {
                let extra = d.assets.meshes[0].clone();
                d.assets.meshes.push(extra);
            }),
            ("primitive_count_mismatch", |d| {
                let extra = d.assets.meshes[0].primitives[0].clone();
                d.assets.meshes[0].primitives.push(extra);
            }),
            ("primitive_vertex_count_mismatch", |d| {
                d.assets.meshes[0].primitives[0].positions.push(Vec3::ZERO);
                d.assets.meshes[0].primitives[0].joints.push([0, 0, 0, 0]);
                d.assets.meshes[0].primitives[0]
                    .weights
                    .push([1.0, 0.0, 0.0, 0.0]);
            }),
        ];
        for (expected, doctor) in cases {
            let mut broken = candidate.document().clone();
            doctor(&mut broken);
            let broken = ScaleCandidate { document: broken };
            assert_eq!(
                prove_scale(&doc, &broken, &plan).unwrap_err(),
                ScaleError::CandidateStructureMismatch { reason: expected }
            );
        }
    }
}
