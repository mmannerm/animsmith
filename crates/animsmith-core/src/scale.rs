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
use glam::{Mat3, Mat4, Vec3, Vec4};
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

    fn relative(&self, tolerance: f64, a: f64, b: f64) -> bool {
        (a - b).abs() <= tolerance * a.abs().max(b.abs()).max(1.0)
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

/// Reject a mesh instance with an out-of-range `mesh` or `skin_joints`
/// entry, a non-empty `skin_ibms` whose length disagrees with
/// `skin_joints`, a non-finite `skin_ibms` matrix, or a bone with a
/// non-finite [`crate::model::Bone::inverse_bind`].
fn validate_scene_assets(document: &Document) -> Result<(), ScaleError> {
    let bone_count = document.skeleton.bones.len();
    let mesh_count = document.assets.meshes.len();
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

fn plan_whole_document(document: &Document, factor: f64) -> Result<ScalePlan, ScaleError> {
    if !factor.is_finite() || factor <= 0.0 {
        return Err(ScaleError::InvalidFactor { factor });
    }
    Ok(ScalePlan {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor },
        tolerance_policy: ScaleTolerancePolicy::APPENDIX_D_V1,
        affected_nodes: (0..document.skeleton.bones.len()).collect(),
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
            prove_bounds: true,
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
            prove_bounds: true,
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

fn classify_affine(linear: Mat3, tol: &ScaleTolerancePolicy) -> Result<f64, AffineDomainViolation> {
    let columns = [linear.x_axis, linear.y_axis, linear.z_axis];
    if columns.iter().any(|column| !column.is_finite()) {
        return Err(AffineDomainViolation::NonFinite);
    }
    let lengths = [
        columns[0].length() as f64,
        columns[1].length() as f64,
        columns[2].length() as f64,
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
    let determinant = linear.determinant() as f64;
    if !determinant.is_finite() {
        return Err(AffineDomainViolation::NonFinite);
    }
    let axis_product = lengths[0] * lengths[1] * lengths[2];
    if determinant.abs() <= tol.singular_determinant_relative * axis_product {
        return Err(AffineDomainViolation::Singular);
    }
    if lengths
        .iter()
        .any(|&length| (length - average).abs() > tol.equal_axis * average.max(length).max(1.0))
    {
        return Err(AffineDomainViolation::NonUniformScale);
    }
    let dot01 = columns[0].dot(columns[1]) as f64;
    let dot02 = columns[0].dot(columns[2]) as f64;
    let dot12 = columns[1].dot(columns[2]) as f64;
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
/// computed), or [`ScaleError::BoneIndexOutOfRange`] if an affected node in
/// `plan` is out of range for `document`.
pub fn build_scale_candidate(
    document: &Document,
    plan: &ScalePlan,
) -> Result<ScaleCandidate, ScaleError> {
    validate_document_shape(document)?;
    let candidate = match plan.operation {
        ScaleOperation::WholeDocumentLinearUnits { factor } => {
            build_whole_document(document, factor)
        }
        ScaleOperation::RestBindUniformScale { .. } => build_rest_bind(document, plan)?,
    };
    Ok(ScaleCandidate {
        document: candidate,
    })
}

fn build_whole_document(document: &Document, factor: f64) -> Document {
    let q = factor as f32;
    let mut candidate = document.clone();
    for bone in &mut candidate.skeleton.bones {
        bone.rest.translation *= q;
        if let Some(inverse_bind) = &mut bone.inverse_bind {
            *inverse_bind = scale_translation_only(*inverse_bind, q);
        }
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
    candidate
}

fn build_rest_bind(document: &Document, plan: &ScalePlan) -> Result<Document, ScaleError> {
    let affected = plan.affected_set();
    let s = plan.common_factor as f32;
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
        for instance in &mut candidate.assets.instances {
            for (slot, &joint) in instance.skin_joints.iter().enumerate() {
                if !affected.contains(&joint) {
                    continue;
                }
                if let Some(inverse_bind) = instance.skin_ibms.get_mut(slot) {
                    *inverse_bind = scale_rows(*inverse_bind, node_factor(joint));
                }
            }
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
    /// Maximum rest-world rotation residual, in radians.
    pub rest_rotation_residual: f64,
    /// Maximum postcondition unit-scale residual (rest/bind only).
    pub unit_scale_residual: f64,
    /// Maximum transform-only attachment full-affine residual (rest/bind
    /// only).
    pub transform_only_affine_residual: f64,
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
        key_translation_residual: 0.0,
        cubic_interior_residual: 0.0,
        trajectory_residual: 0.0,
        skin_matrix_residual: 0.0,
        bounds_residual: 0.0,
        sample_time_count: 0,
    };

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
            // comparing local rotations directly is both exact (no lossy
            // matrix decomposition) and, by composition, implies preserved
            // world orientation for every node in the chain.
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
            let rotation_residual = source_rotation.angle_between(candidate_rotation) as f64;
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

fn check_residual(
    kind: ProofResidualKind,
    observed: f64,
    tolerance: f64,
) -> Result<(), ScaleError> {
    if observed > tolerance {
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

fn clip_sample_times(clip: &Clip, affected: &BTreeSet<BoneId>) -> (Vec<f32>, Vec<f32>) {
    let mut keys = Vec::new();
    let mut interiors = Vec::new();
    for track in &clip.tracks {
        if track.property != Property::Translation || !affected.contains(&track.bone) {
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
        // Whole-document multiplies every affected translation track by the
        // declared factor uniformly. Rest/bind multiplies by the target
        // node's parent-basis factor: the domain's common factor if the
        // node's parent is itself affected, else the unaffected boundary
        // factor of one.
        let multiplier = if plan.is_whole_document() {
            plan.common_factor
        } else {
            match source
                .skeleton
                .bones
                .get(track.bone)
                .and_then(|bone| bone.parent)
            {
                Some(parent) if affected.contains(&parent) => plan.common_factor,
                _ => 1.0,
            }
        };
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
    let has_relevant_instance = source.assets.instances.iter().any(|instance| {
        !instance.skin_joints.is_empty()
            && instance
                .skin_joints
                .iter()
                .any(|joint| affected.contains(joint))
    });
    if !has_relevant_instance {
        return Ok(());
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

    #[test]
    fn noisy_but_within_tolerance_factor_is_accepted_and_just_outside_is_not() {
        // DESIGN.md Appendix D §D.3 case 4: a noisy value such as
        // `100.000015` is accepted when within the declared tolerance.
        // `ScaleTolerancePolicy::relative`'s comparison base is floored at
        // `1.0`, so at this factor's magnitude (~0.01) the effective
        // absolute threshold is `common_factor` itself (`1e-5`): pick
        // deltas either side of that floored threshold rather than a
        // fraction of 0.01.
        for (scale, should_accept) in [(0.010008_f32, true), (0.01002_f32, false)] {
            let nodes = vec![rig(None, 0, Vec3::ZERO), rig(Some(0), 1, Vec3::ZERO)];
            let mut doc = rig_document(&nodes, &[1], 0, Mat4::IDENTITY);
            doc.assets.source_skeleton.nodes[0].local_rest = trs_scale(Vec3::splat(scale));
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
            let result = plan_scale(&request);
            assert_eq!(
                result.is_ok(),
                should_accept,
                "scale {scale} accepted={should_accept}"
            );
        }
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
        assert!(matches!(
            plan_scale(&request).unwrap_err(),
            ScaleError::IncompleteClosure { .. }
        ));
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
        assert!(matches!(
            plan_scale(&request).unwrap_err(),
            ScaleError::IncompleteClosure { .. }
        ));
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
        let bad_tracks = [
            // Out-of-range bone.
            Track {
                bone: 99,
                property: Property::Translation,
                interpolation: Interpolation::Linear,
                times: vec![0.0],
                values: TrackValues::Vec3s(vec![Vec3::ZERO]),
            },
            // Empty times.
            Track {
                bone: 1,
                property: Property::Translation,
                interpolation: Interpolation::Linear,
                times: vec![],
                values: TrackValues::Vec3s(vec![]),
            },
            // Non-finite time.
            Track {
                bone: 1,
                property: Property::Translation,
                interpolation: Interpolation::Linear,
                times: vec![f32::NAN],
                values: TrackValues::Vec3s(vec![Vec3::ZERO]),
            },
            // Non-ascending times.
            Track {
                bone: 1,
                property: Property::Translation,
                interpolation: Interpolation::Linear,
                times: vec![1.0, 0.0],
                values: TrackValues::Vec3s(vec![Vec3::ZERO, Vec3::ONE]),
            },
            // Value count disagrees with times/interpolation.
            Track {
                bone: 1,
                property: Property::Translation,
                interpolation: Interpolation::Linear,
                times: vec![0.0, 1.0],
                values: TrackValues::Vec3s(vec![Vec3::ZERO]),
            },
            // Cubic-spline value count not `3 * times.len()`.
            Track {
                bone: 1,
                property: Property::Translation,
                interpolation: Interpolation::CubicSpline,
                times: vec![0.0, 1.0],
                values: TrackValues::Vec3s(vec![Vec3::ZERO; 4]),
            },
            // Non-finite value.
            Track {
                bone: 1,
                property: Property::Translation,
                interpolation: Interpolation::Linear,
                times: vec![0.0],
                values: TrackValues::Vec3s(vec![Vec3::new(f32::NAN, 0.0, 0.0)]),
            },
        ];
        for track in bad_tracks {
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
            assert!(matches!(
                plan_scale(&request).unwrap_err(),
                ScaleError::InvalidTrackShape { .. }
            ));
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
        assert!(matches!(
            prove_scale(&malformed, &candidate, &plan).unwrap_err(),
            ScaleError::InvalidSkinnedPrimitive {
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
}
