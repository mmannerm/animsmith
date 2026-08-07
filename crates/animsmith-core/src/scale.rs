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
//! residual maxima against the fixed [`ScaleTolerancePolicy::APPENDIX_D_V2`]
//! tolerance identity.
//!
//! Those residuals are the producer evidence record of §D.6, which is why
//! two properties of this module are contracts rather than implementation
//! details. Every claim in [`ScaleProofObligations`] is declared only when
//! the planned document carries the evidence for it, and a declared claim
//! whose evidence is missing at proof time is
//! [`ScaleError::MissingProofEvidence`] rather than a zero residual — a
//! record asserting `0.0` for something nothing checked would be false, not
//! merely incomplete. And the two independent observed-factor witnesses
//! §D.6 asks for are both recorded, together with
//! [`ScaleProof::observed_factor_divergence`] between them, so the record
//! states the relationship between its own two measurements instead of
//! leaving a consumer to guess which to trust.

use crate::model::{
    BoneId, Clip, Document, Interpolation, MeshInstance, Primitive, Property, Skeleton,
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
/// instance, [`ScaleTolerancePolicy::APPENDIX_D_V2`]: a policy change is a
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
    ///
    /// This is the normative input band: it is what an operator's declared
    /// factor is judged against, and
    /// [`Self::postcondition_unit_scale_residual`] is derived from it so that
    /// a plan this band accepts is guaranteed to produce a candidate that
    /// satisfies the unit-scale postcondition.
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
    /// Maximum postcondition unit-scale residual, measured **per axis**
    /// (L-infinity) as `max(|scale_axis - 1|)` — not as an L2 norm over the
    /// three axes.
    ///
    /// The norm is normative, and it is the same dimensionless per-axis
    /// relative quantity [`Self::common_factor`] and [`Self::equal_axis`]
    /// measure, so the input band and this postcondition are directly
    /// commensurable (DESIGN.md Appendix D §D.1). This value is *derived*
    /// from [`Self::common_factor`] rather than declared independently: see
    /// [`Self::APPENDIX_D_V2`] for the composition argument and
    /// [`Self::UNIT_SCALE_BANDS`] for the multiplier.
    pub postcondition_unit_scale_residual: f64,
    /// Maximum sampled proof work [`prove_scale`] will perform, in
    /// per-sample-time work units.
    ///
    /// Total work is `sample_time_count * per_sample_work_units`, where the
    /// per-sample cost counts every pass the sampled obligations actually
    /// make — bones, skin slots, and skinned vertices, each once per document
    /// side. See the private `per_sample_work_units` for the exact formula
    /// and for why the slot term cannot be folded into either of the other
    /// two. A
    /// document above this budget is refused with
    /// [`ScaleError::ProofSamplingBudgetExceeded`] *before* any sampling
    /// runs; proof never silently samples a subset.
    ///
    /// This is part of the versioned policy identity, not a per-run flag —
    /// DESIGN.md Appendix D §D.6/§D.7 forbid per-run tolerance knobs, and a
    /// budget that changed per run would make two evidence records carrying
    /// the same policy id describe different amounts of checking.
    pub proof_sample_work_budget: u64,
}

impl ScaleTolerancePolicy {
    /// How many [`Self::common_factor`] bands
    /// [`Self::postcondition_unit_scale_residual`] is derived from.
    ///
    /// Three of them are analytic and one is float headroom; see
    /// [`Self::APPENDIX_D_V2`].
    pub const UNIT_SCALE_BANDS: f64 = 4.0;

    /// The only supported tolerance policy: DESIGN.md Appendix D, version 2.
    ///
    /// Version 2 supersedes the `appendix-d-v1` identity. Two things changed
    /// meaning, which is why it is a new identity rather than a retune:
    ///
    /// 1. [`Self::postcondition_unit_scale_residual`] is now a per-axis
    ///    (L-infinity) residual derived from [`Self::common_factor`], instead
    ///    of an independently declared `1e-5` L2 norm over three axes. Under
    ///    v1 the two were incommensurable, and a source whose observed factor
    ///    had relative error `e` produced a postcondition residual of
    ///    `sqrt(3) * e`, so every `e` in `(5.77e-6, 1e-5]` was accepted by
    ///    [`plan_scale`] and then rejected by [`prove_scale`].
    /// 2. [`Self::proof_sample_work_budget`] is new, and bounds the sampled
    ///    proof work a document may demand.
    ///
    /// `postcondition_unit_scale_residual` is
    /// `UNIT_SCALE_BANDS * common_factor = 4e-5`, rounded up to the next
    /// power of two, `2^-14 = 6.103515625e-5`. That value is also
    /// `512 * 2^-23`, and so lies on the binary32 mantissa grid the
    /// composed-scale measurement lives on. Landing on that grid is what
    /// makes §D.1's inclusive "at most" reachable for this obligation: the
    /// measured residual near unit magnitude is always an integer multiple of
    /// `2^-23`, so a bound off that grid could never be met with equality and
    /// would be an exclusive bound wearing an inclusive name.
    ///
    /// The four bands are:
    ///
    /// - one for [`ScaleError::FactorMismatch`], which binds the domain's
    ///   observed common factor `s_0` to the caller's declared factor
    ///   `s_declared`;
    /// - one for [`ScaleError::MixedFactor`], which binds each affected node's
    ///   observed factor `s_i` to `s_0`;
    /// - one for [`AffineDomainViolation::NonUniformScale`], which binds each
    ///   individual *axis* of node `i` to `s_i`; and
    /// - one reserved as headroom for the `f32` world-matrix composition and
    ///   decomposition that produces the measured composed scale.
    ///
    /// The first three compose, and the third is easy to miss: `s_i` is the
    /// *average* of node `i`'s three world axis lengths (the affine
    /// classifier returns that average), while the postcondition measures an
    /// individual
    /// axis, and the equal-axis check permits each axis its own further band
    /// away from that average. The candidate's composed scale on axis `k` of
    /// node `i` is `axis_ik / s_declared`, and each of the three bands is
    /// stated relative to `max` of its operands, so each contributes at most
    /// `c / (1 - c)` when re-expressed relative to the smaller one. The
    /// analytic worst case is therefore `(1 - c)^-3 - 1 = 3.00006e-5` for
    /// `c = 1e-5`.
    ///
    /// Three bands rounded up (`2^-15 = 3.0517578125e-5`) would leave that
    /// worst case only `4` binary32 ulps of room — `2^-15 - 3.00006e-5 =
    /// 5.17e-7 = 4.34 * 2^-23` — which is not headroom for a float
    /// measurement, it is a rounding artefact. A fourth band makes the
    /// reserved-headroom claim above true rather than aspirational, and it
    /// does not blunt the obligation: every build defect this check exists to
    /// catch — a dropped rebase, a factor applied twice, a stale no-op — is
    /// `>= 1e-3`, so `6.1e-5` still leaves better than a `16x` detection
    /// margin.
    pub const APPENDIX_D_V2: Self = Self {
        id: "appendix-d-v2",
        relative_orthogonality: 1e-5,
        equal_axis: 1e-5,
        common_factor: 1e-5,
        singular_determinant_relative: 1e-6,
        scalar_absolute: 1e-6,
        scalar_relative: 1e-5,
        rotation_residual_radians: 1e-5,
        // 2^-14, exactly: four `common_factor` bands rounded up onto the
        // binary32 mantissa grid (`= 512 * 2^-23`).
        postcondition_unit_scale_residual: 6.103_515_625e-5,
        // A 200-bone rig carrying a 100k-vertex skinned mesh costs
        // `2 * 200 + 3 * 200 + 2 * 100_000 = 201_000` units per sample time
        // and so admits `1_990` of them; the same rig with a 10k-vertex mesh
        // costs `21_000` and admits `19_047`. A 100k-key track on the
        // 100k-vertex rig demands `2.01e10` and is refused.
        //
        // The value is a wall-time ceiling expressed in work units, measured
        // rather than assumed. Release timings of `prove_scale` at this
        // ceiling: `1.54s` for the vertex-dominated shape (10k vertices, one
        // slot, 19_992 sample times) and `4.77s` for the slot-dominated one
        // (200 instances of 100 repeated slots, one vertex, 6_620 sample
        // times) — the same total work costs about three times as much when
        // it is slot work, so the pathological shape is what sets the
        // ceiling. Timings are linear in the charge across four doublings
        // (`1e8` to `8e8`) in both shapes, which is the evidence that the
        // charge is a proxy for real work rather than a number.
        //
        // `1e8` — the first value this policy carried — was too tight, and
        // the arithmetic that justified it was the pre-both-sides charge. A
        // 200-bone rig with a 100k-vertex skinned mesh and a 30-second clip
        // at 30 fps costs `900 * 201_000 = 180_900_000`: a plausible
        // production asset, refused with no way for a caller to opt into the
        // work. At `4e8` it is admitted with `2.2x` headroom (about 66
        // seconds of animation on that rig) and proves in well under a
        // second, while the worst-shaped document the budget still admits
        // stays inside five.
        proof_sample_work_budget: 400_000_000,
    };

    /// The expected ceiling on [`ScaleProof::observed_factor_divergence`]:
    /// [`Self::common_factor`] plus
    /// [`Self::postcondition_unit_scale_residual`], `7.103515625e-5` under
    /// [`Self::APPENDIX_D_V2`].
    ///
    /// [`ScalePlan::observed_factor`] and [`ScaleProof::observed_factor`] are
    /// two independent witnesses of the same quantity, measured from
    /// genuinely different state — the raw source projection composed through
    /// `parent_source_node_index`, and the normalized skeleton composed
    /// through `world_rest_matrices`. Their independence is the point, and it
    /// is why they are not equal. This is how far apart the design expects
    /// them to be, and the sum is where it comes from:
    ///
    /// - planning binds its witness to the caller's declared factor within
    ///   [`Self::common_factor`], or refuses with
    ///   [`ScaleError::FactorMismatch`]; and
    /// - for a candidate [`build_scale_candidate`] produced from the source
    ///   under proof, that candidate's composed root scale is the proof
    ///   witness divided by the declared factor, so the unit-scale
    ///   postcondition binds the proof witness to the declared factor within
    ///   [`Self::postcondition_unit_scale_residual`].
    ///
    /// The two bands are not stated the same way, and the sum is a ceiling
    /// only up to that difference. Planning's is relative to the `max` of its
    /// two operands, exactly as this divergence is. The postcondition's is not
    /// a relative band on the two witnesses at all: it is an absolute
    /// L-infinity deviation from `1` on the *candidate's* composed scale, and
    /// that candidate's scale is the proof witness rebased by the declared
    /// factor — so what it bounds is `|proved - declared|` as a fraction of
    /// the declared factor, not as a fraction of `max(planned, proved)`.
    ///
    /// **Reported, not enforced, and expected rather than proved.** Nothing
    /// refuses a document for exceeding this. The second step above holds for
    /// a candidate this module built from the source it is being proved
    /// against, which [`prove_scale`] deliberately does not require, and it
    /// costs the binary32 rounding of the rebase on the way — so the sum is
    /// the ceiling the design guarantees, not a bound proved to the last ulp.
    /// A divergence beyond it means the source's two parent chains disagree
    /// with each other, which nothing here validates.
    ///
    /// Derived from two bands this policy already declares rather than
    /// introduced as a third, so it adds no tolerance and no policy identity —
    /// and a consumer of the evidence record does not have to sum two
    /// separate policy fields to know what the recorded divergence means.
    pub fn observed_factor_divergence_ceiling(&self) -> f64 {
        self.common_factor + self.postcondition_unit_scale_residual
    }

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
    /// There is **no** floor on the comparison base — not `1.0`, and not
    /// [`Self::scalar_absolute`] either. A `scalar_absolute` floor is a
    /// smaller version of the same defect and breaks the same closure
    /// property, just further down: below `1e-6` the band stops tracking the
    /// operands and freezes at the constant `1e-5 * 1e-6 = 1e-11`, which is a
    /// *relative* band of `1e-11 / abs(s)` and therefore widens without limit
    /// as `s` shrinks. It crosses
    /// [`Self::postcondition_unit_scale_residual`] at
    /// `s = 1e-11 / 2^-14 = 1e-11 * 16384 = 1.6384e-7` (`3.2768e-7` against
    /// the tighter `2^-15` bound an earlier revision declared — halving the
    /// postcondition bound doubles the crossing point, because the crossing
    /// point is inversely proportional to it), so every declared factor
    /// below that had a band of accepted plans whose candidates then failed
    /// the unit-scale postcondition — at `s = 1e-9` the band admits `1e-2`
    /// relative error, `1000x` the declared policy.
    ///
    /// Nothing needs the floor for the degenerate `a == b == 0` case either:
    /// that compares `0.0 <= 0.0`, which holds. (Both call sites have already
    /// proved their operands strictly positive in any case — a declared
    /// factor by [`plan_rest_bind`]'s range check, an observed one by
    /// [`classify_affine`].)
    fn relative(&self, tolerance: f64, a: f64, b: f64) -> bool {
        (a - b).abs() <= tolerance * a.abs().max(b.abs())
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
    /// The document's sampled proof work exceeds
    /// [`ScaleTolerancePolicy::proof_sample_work_budget`].
    ///
    /// Raised by [`prove_scale`] *before* any sample time is evaluated, so a
    /// document whose key count and vertex count multiply out beyond the
    /// versioned policy's budget is refused outright rather than proved
    /// against a silently truncated subset of its sample times. The budget is
    /// a property of the policy identity recorded in evidence, not a per-run
    /// flag.
    #[error(
        "proof sampling work {work} ({sample_times} sample times x {per_sample_cost} work units) exceeds the {policy_id} budget {budget}"
    )]
    ProofSamplingBudgetExceeded {
        /// The tolerance-policy identity whose budget was exceeded.
        policy_id: &'static str,
        /// Distinct sample times the plan's obligations would evaluate,
        /// summed over every clip.
        sample_times: u64,
        /// Work units one sample time costs: `bone_count` plus the vertex
        /// count of every skinned instance inside the affected closure.
        per_sample_cost: u64,
        /// `sample_times * per_sample_cost`, saturating.
        work: u64,
        /// The policy's [`ScaleTolerancePolicy::proof_sample_work_budget`].
        budget: u64,
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
    /// A mesh instance is malformed: an out-of-range `node`, `mesh` or
    /// `skin_joints` entry, a non-empty `skin_ibms` whose length disagrees
    /// with `skin_joints`, or a non-finite inverse-bind matrix.
    ///
    /// `node` is range-checked for the same reason `mesh` is: it names a
    /// bone the rest of the pipeline resolves without a second bounds test
    /// — the glTF writer attaches the instance there, and measurement reads
    /// that bone's world matrix — so an index past the end of the skeleton
    /// is a malformed document rather than a residual.
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
    /// primitive, a track whose identity, interpolation, times, or value
    /// shape disagrees with its source counterpart, or a mesh instance whose
    /// identity — the node it hangs off, the source node it came from, the
    /// mesh it draws, or the joints it binds — disagrees with its source
    /// counterpart. Proof pairs source and candidate structure by index,
    /// which requires this parity to hold — an extra, missing or relocated
    /// structure is never silently ignored.
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
    /// Stored inverse-bind residual for a skin slot *outside* the affected
    /// closure — a skin neither operation touches, whose binds must therefore
    /// come through unchanged.
    ///
    /// Slots inside the closure are covered, more strongly, by
    /// [`Self::SkinMatrix`]: that obligation compares the composed `W * B`,
    /// which is what actually deforms a vertex. Outside the closure there is
    /// no rebase to compose against, so the stored arrays are compared
    /// directly.
    UnaffectedInverseBind,
    /// The factor [`prove_scale`] re-derived from the documents it was given.
    /// Only ever reported as [`ScaleError::MissingProofEvidence`]: it names a
    /// source whose scaled root the proof could not resolve, never a residual.
    ObservedFactor,
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
///
/// **Every flag is evidence-gated.** [`plan_scale`] declares an obligation
/// only when the planned document carries the payload that obligation reads,
/// and [`prove_scale`] refuses with [`ScaleError::MissingProofEvidence`],
/// naming the obligation, when a declared one's evidence is not in the
/// documents it was handed. Neither half is optional: a declared obligation
/// with nothing to iterate would report a `0.0` residual, and those residuals
/// are what producer evidence records (DESIGN.md Appendix D §D.6). A record
/// stating "residual 0.0" for a claim nothing checked is a false record, not
/// a missing one, so an unprovable claim is never declared and a vanished one
/// is never quietly downgraded to success.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ScaleProofObligations {
    /// Prove rest-world translation/orientation facts for affected nodes.
    ///
    /// Evidence: a non-empty affected closure. Only
    /// [`ScaleOperation::WholeDocumentLinearUnits`] can plan an empty one (a
    /// document with no bones); a rest/bind closure always contains its
    /// scaled root.
    pub prove_rest: bool,
    /// Prove unit composed scale at every affected node (rest/bind only).
    ///
    /// Nested inside [`Self::prove_rest`]'s walk, and shares its evidence.
    pub prove_unit_scale_postcondition: bool,
    /// Prove the complete expected world affine of every transform-only
    /// attachment via an off-origin point, so a translation/rotation-only
    /// check cannot mistake a no-op for a correct rebase.
    ///
    /// Evidence: a non-empty [`ScalePlan::transform_only_attachments`]. §D.6
    /// justifies this obligation as the one ensuring "a no-op cannot pass",
    /// which an empty probe loop reporting `0.0` would not do.
    pub prove_transform_only_affine: bool,
    /// Prove every keyframe time of an affected translation track.
    ///
    /// Evidence: some clip declares a translation track on an affected bone.
    pub prove_keys: bool,
    /// Prove bounded interior times of cubic-spline translation segments.
    ///
    /// Evidence: some clip declares both an affected cubic-spline track with
    /// at least two key times — the only source of an interior time — and an
    /// affected translation track for the comparison at that time to read.
    pub prove_cubic_interiors: bool,
    /// Prove sampled world-space trajectories.
    ///
    /// Evidence: some clip declares any track on an affected bone, which is
    /// what yields a sample time to pose both skeletons at.
    pub prove_trajectories: bool,
    /// Prove the skin equation `W_i(t) * B_i` for affected skins, at rest
    /// and at every declared key/cubic-interior sample time.
    ///
    /// Evidence: a skinned mesh instance with a joint inside the affected
    /// closure — the same instance walk [`Self::prove_bounds`] needs, and so
    /// the same predicate.
    ///
    /// **This flag arms two walks, not one.** [`prove_scale`] evaluates the
    /// skin equation once at the **rest** pose, outside the sampled loop and
    /// outside the sampling budget, and again at each sample time; both go
    /// through the same `check_skin_and_bounds`, which has exactly those two
    /// call sites. So a skinned document with no clips at all still proves
    /// this obligation — it reports `sample_time_count == 0` and a
    /// [`ScaleProof::skin_matrix_comparisons`] above zero — and a corrupted
    /// bind in such a document is caught. Reading this flag as arming only
    /// the sampled loop is what led #317 to be filed against a defect that
    /// does not exist.
    ///
    /// Its evidence predicate is likewise independent of the three
    /// clip-driven flags': a document with a skinned instance and no clips
    /// declares this one and none of them.
    /// `a_skin_only_plan_still_evaluates_its_sampled_obligations` pins the
    /// shape where it arms the sampled loop alone, and
    /// `an_unanimated_skinned_document_still_compares_its_skin_at_rest` pins
    /// the rest-pose walk on a document with no sample times at all, so
    /// neither half of the coverage can be lost silently.
    pub prove_skin: bool,
    /// Prove skinned mesh bounds, at rest and at every declared
    /// key/cubic-interior sample time.
    ///
    /// Evidence: as [`Self::prove_skin`].
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
    observed_factor: f64,
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
    ///
    /// This is always the factor the *caller declared*, never the one
    /// measured from the source: [`build_scale_candidate`] applies exactly
    /// this value, and [`prove_scale`] states every analytic expectation in
    /// terms of it. [`Self::observed_factor`] reports the measured
    /// counterpart, and the two are separate numbers on purpose — DESIGN.md
    /// Appendix D §D.6 requires producer evidence to record both.
    pub fn common_factor(&self) -> f64 {
        self.common_factor
    }

    /// The factor this plan *observed* in the source, as distinct from the
    /// caller-declared [`Self::common_factor`] the build applies.
    ///
    /// For [`ScaleOperation::RestBindUniformScale`] this is the rest-world
    /// uniform factor measured at the scaled root of DESIGN.md Appendix D
    /// §D.2 — the average of its rest-world linear part's three column
    /// lengths, the same quantity the domain classification returns. It is
    /// within [`ScaleTolerancePolicy::common_factor`] of
    /// [`Self::common_factor`] (planning rejects it otherwise with
    /// [`ScaleError::FactorMismatch`]) but is generally not equal to it: a
    /// source authored at `0.010_000_02` is accepted against a declared
    /// `0.01`, and both numbers belong in evidence.
    ///
    /// For [`ScaleOperation::WholeDocumentLinearUnits`] this equals
    /// [`Self::common_factor`] exactly, because there is nothing to measure.
    /// That operation's factor is *declared*, not observed: §D.1 states that
    /// a whole-document conversion "changes physical size", is "appropriate
    /// only when the source was authored in a different linear unit", and
    /// that neither operation "may infer its factor or applicability from
    /// mesh bounds, character height, joint lengths, inverse-bind magnitude,
    /// filename, or an asset category". A source authored in centimetres and
    /// one authored in metres are numerically identical documents, so no
    /// measurement of either could distinguish them; the declared factor is
    /// the only fact there is, and reporting it here keeps the evidence
    /// contract uniform across the two operations rather than leaving a hole
    /// a consumer would have to special-case.
    pub fn observed_factor(&self) -> f64 {
        self.observed_factor
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
/// instance with an out-of-range `node`, `mesh` or `skin_joints` entry, a
/// non-empty `skin_ibms` whose length disagrees with `skin_joints`, a
/// non-finite `skin_ibms` matrix, or a bone with a non-finite
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
        // `node` is the bone the instance hangs off, and nothing downstream
        // range-checks it again: the glTF writer emits it as a node index and
        // measurement reads that bone's world matrix. Checked here, alongside
        // `mesh` and `skin_joints`, so the same pass covers every index this
        // instance names.
        if instance.node >= bone_count {
            return Err(ScaleError::InvalidMeshInstance {
                instance_index,
                reason: "node_index_out_of_range",
            });
        }
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

/// Reject `candidate`'s bone/clip/track/instance/mesh/primitive structure when
/// it does not match `source`'s: every proof comparison in this module pairs
/// source and candidate structure positionally (by bone/clip/track/instance/
/// mesh index), which is only sound once the two document shapes are known to
/// agree — an extra or missing structure is never silently ignored.
fn validate_candidate_structure(source: &Document, candidate: &Document) -> Result<(), ScaleError> {
    // Bone-count parity is checked first, and it is the clause the sampling
    // budget rests on. [`sample_time_obligations`] poses *both* skeletons at
    // every sample time, but [`per_sample_work_units`] can only measure the
    // source — it is called before any candidate is walked, and
    // [`ScaleCandidate::from_document`] is public, so the candidate's bone
    // count is caller-supplied. Charging `PROOF_SIDES * bones(source)` is
    // therefore only an honest charge once the two counts are known equal:
    // without this clause a two-bone source paired with a candidate padded to
    // 60_000 identity bones was charged 36_000 units — 0.009% of the budget —
    // and proved in 3.71s release, more than twice the wall time of the
    // vertex-dominated document the budget scores at 100%. Parity is the fix
    // rather than charging both
    // sides because it is the same class of claim as the mesh and skin-joint
    // clauses below ("unchanged skeleton/mesh/skin identity", DESIGN.md
    // Appendix D §D.6), and it makes the single-side charge correct by
    // construction rather than merely conservative.
    if source.skeleton.bones.len() != candidate.skeleton.bones.len() {
        return Err(ScaleError::CandidateStructureMismatch {
            reason: "bone_count_mismatch",
        });
    }
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
    // Mesh assignment and skin-joint identity are what DESIGN.md Appendix D
    // §D.6 calls "unchanged mesh/material/skin identity": neither operation
    // rewrites which mesh an instance draws or which joints it is bound to.
    // Checking it here rather than inferring it from a residual is also what
    // lets the skin and bounds obligations share one instance/vertex walk —
    // the two sides are then known to have the same slots, the same mesh, and
    // therefore the same per-primitive vertex counts.
    //
    // `node` and `source_node_index` are the *placement* half of that same
    // identity, and neither is reachable from any residual. `node` is what
    // attaches the instance to a bone — holding the skeleton fixed, it is what
    // positions an unskinned prop, so moving one from a bone outside the
    // affected closure onto a rebased one relocates it in world space while
    // every residual this module measures stays exactly zero.
    // `source_node_index` is what [`instance_source_skin`] matches attachments
    // against, so re-pointing it changes whether a missing bind resolves to
    // glTF's format-defined identity default or is refused as missing
    // evidence.
    //
    // What these two clauses prove is therefore instance identity *against a
    // fixed skeleton*, not world placement. A bone outside the affected
    // closure has neither its `rest` nor its `parent` compared anywhere in
    // this module, so the same prop can still be relocated by rewriting the
    // bone it names, with every residual zero. Issue #325 tracks that gap.
    // Comparing them positionally also fixes instance *order*: two instances
    // identical in every payload but attached to different nodes cannot be
    // swapped without one of these two clauses firing.
    for (instance, candidate_instance) in source
        .assets
        .instances
        .iter()
        .zip(candidate.assets.instances.iter())
    {
        if instance.node != candidate_instance.node {
            return Err(ScaleError::CandidateStructureMismatch {
                reason: "instance_node_mismatch",
            });
        }
        if instance.source_node_index != candidate_instance.source_node_index {
            return Err(ScaleError::CandidateStructureMismatch {
                reason: "instance_source_node_index_mismatch",
            });
        }
        if instance.mesh != candidate_instance.mesh {
            return Err(ScaleError::CandidateStructureMismatch {
                reason: "instance_mesh_mismatch",
            });
        }
        if instance.skin_joints != candidate_instance.skin_joints {
            return Err(ScaleError::CandidateStructureMismatch {
                reason: "instance_skin_joints_mismatch",
            });
        }
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
    let affected: BTreeSet<BoneId> = affected_nodes.iter().copied().collect();
    let skinned = has_skinned_evidence(document, &affected);
    let sampled = sampled_evidence(document, &affected);
    let boned = !affected_nodes.is_empty();
    Ok(ScalePlan {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor },
        tolerance_policy: ScaleTolerancePolicy::APPENDIX_D_V2,
        affected_nodes,
        transform_only_attachments: Vec::new(),
        common_factor: factor,
        // Declared, not measured — see [`ScalePlan::observed_factor`].
        observed_factor: factor,
        domain_rewrites: ScaleDomainRewrites {
            rest_hierarchy: true,
            translation_animation: true,
            inverse_binds: true,
            base_mesh_positions: true,
        },
        // Every flag below is the evidence predicate for its own obligation,
        // never an unconditional `true`: a declared obligation whose payload
        // the document does not carry would report a `0.0` residual for a
        // claim nothing checked, and those residuals are the evidence record
        // (DESIGN.md Appendix D §D.6).
        proof_obligations: ScaleProofObligations {
            // The rest obligation walks the affected closure, which for this
            // operation is the whole skeleton — empty for a document with no
            // bones at all.
            prove_rest: boned,
            prove_unit_scale_postcondition: false,
            prove_transform_only_affine: false,
            prove_keys: sampled.key_translations,
            prove_cubic_interiors: sampled.cubic_interiors,
            prove_trajectories: sampled.sample_times,
            prove_skin: skinned,
            prove_bounds: skinned,
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

    let tol = ScaleTolerancePolicy::APPENDIX_D_V2;
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
    // Read at the scaled root, which DESIGN.md Appendix D §D.6 names, and not
    // at whichever affected node happens to sort first. The two are *not*
    // interchangeable: this closure and every factor above it were walked in
    // source-node space through `parent_source_node_index`, while the
    // "parent strictly earlier than child" ordering `world_rests` enforces is
    // a fact about `Skeleton::parent`. Nothing compares the two chains —
    // `validate_source_skeleton_identity` checks only that the projection's
    // indices are unique — so a document whose projection inverts a skeleton
    // parent link puts the scaled root somewhere other than the lowest
    // affected bone id, with a different factor. Pinned by
    // `the_observed_factor_is_read_at_the_scaled_root_not_the_lowest_affected_bone`.
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

    let affected: BTreeSet<BoneId> = affected_nodes.iter().copied().collect();
    let skinned = has_skinned_evidence(document, &affected);
    let sampled = sampled_evidence(document, &affected);
    let attached = !transform_only_attachments.is_empty();

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
        // The measured source fact, kept alongside the declared factor the
        // build applies rather than discarded once it has been validated
        // against it (DESIGN.md Appendix D §D.6, "declared and observed
        // factors").
        observed_factor: observed_common,
        domain_rewrites: ScaleDomainRewrites {
            rest_hierarchy: true,
            translation_animation: true,
            inverse_binds: true,
            base_mesh_positions: false,
        },
        // As in [`plan_whole_document`], every flag is its obligation's own
        // evidence predicate. `prove_rest` and the unit-scale postcondition
        // it nests are declared unconditionally here because this closure is
        // never empty: it is seeded from `source_root_node_index`, and
        // `bone_of_source` maps that node to a bone or fails outright, so the
        // scaled root is always in `affected_nodes`.
        proof_obligations: ScaleProofObligations {
            prove_rest: true,
            prove_unit_scale_postcondition: true,
            prove_transform_only_affine: attached,
            prove_keys: sampled.key_translations,
            prove_cubic_interiors: sampled.cubic_interiors,
            prove_trajectories: sampled.sample_times,
            prove_skin: skinned,
            prove_bounds: skinned,
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
///
/// The two helpers below name the quantity this function returns, so the
/// observed factor [`prove_scale`] re-derives is the *same* quantity by
/// definition rather than by coincidence — see [`observed_factor_from_source`].
fn classify_affine(linear: Mat3, tol: &ScaleTolerancePolicy) -> Result<f64, AffineDomainViolation> {
    if !linear.is_finite() {
        return Err(AffineDomainViolation::NonFinite);
    }
    let columns = [
        linear.x_axis.as_dvec3(),
        linear.y_axis.as_dvec3(),
        linear.z_axis.as_dvec3(),
    ];
    let lengths = axis_lengths(linear);
    if lengths.iter().any(|length| !length.is_finite()) {
        return Err(AffineDomainViolation::NonFinite);
    }
    let average = axis_length_average(lengths);
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
    //
    // The base is `max(average, length)`, not `average`, and the comparison
    // is `>`, not `>=`; both are pinned exactly on the boundary by
    // `the_equal_axis_band_is_relative_to_the_longer_axis_and_admits_its_own_edge`.
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
    // `>`, inclusive, like every other comparison against a policy bound, and
    // pinned exactly on the boundary — like the equal-axis band above — by
    // `an_orthogonality_dot_exactly_on_its_tolerance_is_accepted_and_the_next_one_up_is_not`.
    //
    // An earlier revision of this comment claimed no binary32 basis could
    // reach this boundary exactly, on a three-square argument that required
    // `relative_orthogonality` to be the rational `1/100000` and
    // `average * average` to be exact. It is neither. `1e-5` in binary64 is
    // `5_902_958_103_587_057 * 2^-69`, whose numerator is odd, and both
    // `average` and the two products below it are rounded — so nothing here
    // is an identity between exact rationals and no such argument applies.
    // Boundary bases are in fact easy to come by: sweeping the fixture's
    // shape outward from the identity hits one two binary32 steps in.
    if dot01.abs() > orthogonality_tolerance
        || dot02.abs() > orthogonality_tolerance
        || dot12.abs() > orthogonality_tolerance
    {
        return Err(AffineDomainViolation::Sheared);
    }
    // `<` rather than `<=`, and the two are indistinguishable here: a zero
    // determinant can never reach this line. `determinant` is finite by the
    // guard above, `lengths` are finite and non-negative, so `axis_product` is
    // non-negative and never `NaN`; `singular_determinant_relative` is `1e-6`
    // — this function is only ever called with `APPENDIX_D_V2` — so the
    // singular threshold is non-negative, and `(±0.0).abs() <= threshold`
    // holds for every non-negative threshold. Both signed zeros are therefore
    // already `Singular`, and on every determinant that does arrive here
    // `< 0.0` and `<= 0.0` agree. No fixture can separate them; a test that
    // appeared to would be asserting on a policy no caller can supply.
    if determinant < 0.0 {
        return Err(AffineDomainViolation::Reflected);
    }
    Ok(average)
}

/// The three column lengths of a linear part, widened to `f64` first, per
/// DESIGN.md Appendix D §D.1.
fn axis_lengths(linear: Mat3) -> [f64; 3] {
    [
        linear.x_axis.as_dvec3().length(),
        linear.y_axis.as_dvec3().length(),
        linear.z_axis.as_dvec3().length(),
    ]
}

/// The uniform factor a basis with those column lengths carries: their
/// arithmetic mean.
///
/// Shared by [`classify_affine`] and [`observed_factor_from_source`] so that
/// "the observed factor" denotes one quantity across planning and proof.
/// Meaningful only for a basis whose axes are already known equal within
/// [`ScaleTolerancePolicy::equal_axis`]; on any other basis it is just an
/// average and claims nothing.
fn axis_length_average(lengths: [f64; 3]) -> f64 {
    (lengths[0] + lengths[1] + lengths[2]) / 3.0
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
    /// Wrap a candidate `document` that a format frontend produced by
    /// rewriting its own exact source bytes, so it can be handed to
    /// [`prove_scale`].
    ///
    /// DESIGN.md Appendix D §D.8 assigns "exact source rewriting" to the
    /// format frontend, which necessarily produces candidates this module did
    /// not build: `animsmith_gltf`'s whole-document linear-unit rewrite
    /// operates on raw glTF JSON and buffer bytes and then reloads the
    /// artifact. Without this constructor that reloaded [`Document`] could
    /// never reach [`prove_scale`], and the artifact-level proof D.6 requires
    /// would have no in-memory layer to sit on top of.
    ///
    /// This constructor asserts nothing about `document`. It does not need
    /// to: [`prove_scale`] already re-validates both documents it is given
    /// and re-derives every claim from them, so this type carries no safety
    /// obligation that [`prove_scale`] does not independently redo.
    pub fn from_document(document: Document) -> Self {
        Self { document }
    }

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
/// [`ScalePlan::tolerance_policy`], each paired with the number of
/// comparisons that produced it.
///
/// **Read the count before the residual.** A maximum alone cannot
/// distinguish "compared, no deviation" from "nothing to compare": both read
/// `0.0`, because every maximum starts there and is raised only by a loop
/// that may have zero iterations. So a field for an obligation the plan does
/// not require (see [`ScaleProofObligations`]) reports `0.0` — and reports a
/// companion count of `0` that says so. A `0.0` with a count above zero is a
/// measurement; a `0.0` with a count of zero is an absence, which DESIGN.md
/// Appendix D §D.6 requires an evidence record to publish as an absence
/// rather than as a checked zero.
///
/// The counts are measurements, not obligation flags, and are not derived
/// from any: each is written beside its maximum, by the single private
/// recording method every comparison in [`prove_scale`] funnels through.
/// No comparison can raise a residual without being counted, and
/// no count can rise without a comparison having been checked against the
/// tolerance policy. That also covers the residuals no obligation flag gates
/// — [`Self::track_value_residual`], [`Self::mesh_position_residual`] and
/// [`Self::unaffected_inverse_bind_residual`], which are proved
/// unconditionally — for which no flag could serve as a proxy.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct ScaleProof {
    /// The tolerance policy every residual below was checked against.
    pub tolerance_policy: ScaleTolerancePolicy,
    /// Maximum rest-world translation residual across affected nodes.
    pub rest_translation_residual: f64,
    /// Comparisons behind [`Self::rest_translation_residual`]: one per
    /// affected node.
    pub rest_translation_comparisons: usize,
    /// Maximum rest-world rotation residual, in radians, directly comparable
    /// to [`ScaleTolerancePolicy::rotation_residual_radians`].
    ///
    /// Measured as a double-cover-aware quaternion chord length
    /// `|q1 - q2| = 2 * sin(theta / 4)` and reported as the angle
    /// `theta = 4 * asin(chord / 2)` that chord represents, so this value and
    /// the tolerance it is checked against carry the same unit.
    pub rest_rotation_residual: f64,
    /// Comparisons behind [`Self::rest_rotation_residual`]: one per affected
    /// node.
    pub rest_rotation_comparisons: usize,
    /// Maximum postcondition unit-scale residual (rest/bind only).
    pub unit_scale_residual: f64,
    /// Comparisons behind [`Self::unit_scale_residual`]: one per affected
    /// node, and zero unless the plan declares the postcondition.
    pub unit_scale_comparisons: usize,
    /// Maximum transform-only attachment full-affine residual (rest/bind
    /// only).
    pub transform_only_affine_residual: f64,
    /// Comparisons behind [`Self::transform_only_affine_residual`]: one
    /// probe point per transform-only attachment.
    pub transform_only_affine_comparisons: usize,
    /// Maximum per-element animation-track value residual, across rewritten
    /// translation elements and every retained rotation/scale element.
    pub track_value_residual: f64,
    /// Comparisons behind [`Self::track_value_residual`]: one per track
    /// element of every clip, so a document with no clips — or with clips
    /// whose tracks carry no values — reports zero.
    pub track_value_comparisons: usize,
    /// Maximum per-vertex base mesh `POSITION` residual.
    pub mesh_position_residual: f64,
    /// Comparisons behind [`Self::mesh_position_residual`]: one per vertex
    /// of every mesh primitive, so a document with no mesh positions reports
    /// zero.
    pub mesh_position_comparisons: usize,
    /// Maximum residual at any affected-track keyframe time.
    pub key_translation_residual: f64,
    /// Comparisons behind [`Self::key_translation_residual`]: one per
    /// affected translation track per key time.
    pub key_translation_comparisons: usize,
    /// Maximum residual at any bounded cubic-segment interior time.
    pub cubic_interior_residual: f64,
    /// Comparisons behind [`Self::cubic_interior_residual`]: one per
    /// affected translation track per cubic-segment interior time.
    pub cubic_interior_comparisons: usize,
    /// Maximum sampled world-space trajectory residual.
    pub trajectory_residual: f64,
    /// Comparisons behind [`Self::trajectory_residual`]: one per affected
    /// node per sample time.
    pub trajectory_comparisons: usize,
    /// Maximum skin-matrix (`W * B`) component residual, across rest and
    /// every sampled key/cubic-interior time.
    pub skin_matrix_residual: f64,
    /// Comparisons behind [`Self::skin_matrix_residual`]: one per skin slot
    /// of every affected skinned instance, at rest and again at every sample
    /// time — so a skinned document with no clips still reports a count
    /// above zero, from the rest-pose walk alone.
    pub skin_matrix_comparisons: usize,
    /// Maximum skinned mesh bounds residual, across rest and every sampled
    /// key/cubic-interior time.
    pub bounds_residual: f64,
    /// Comparisons behind [`Self::bounds_residual`]: six per evaluated pose
    /// (three axes of each of the two extreme corners), at rest and again at
    /// every sample time.
    pub bounds_comparisons: usize,
    /// Maximum stored inverse-bind residual over every skin slot outside the
    /// affected closure (see [`ProofResidualKind::UnaffectedInverseBind`]).
    pub unaffected_inverse_bind_residual: f64,
    /// Comparisons behind [`Self::unaffected_inverse_bind_residual`]: one
    /// per skin slot of every instance outside the affected closure that
    /// stores a bind on both sides. A document whose every skin is inside
    /// the closure reports zero, as does a slot neither side records — which
    /// §D.6 places outside this proof's scope rather than proving.
    pub unaffected_inverse_bind_comparisons: usize,
    /// The operation's observed factor, re-derived by this proof from the
    /// documents it was handed rather than copied from
    /// [`ScalePlan::observed_factor`], so evidence does not depend on
    /// planning having recorded it.
    ///
    /// **Reported, not checked.** This is a measurement, not an obligation:
    /// nothing here compares it against [`ScalePlan::common_factor`]. The
    /// declared/observed agreement is the *input* contract §D.1 states, and
    /// planning already enforces it ([`ScaleError::FactorMismatch`]); what
    /// binds the candidate is the postcondition
    /// ([`ProofResidualKind::UnitScale`]) that §D.1 derives from that band.
    ///
    /// Measured from `source`, not from `candidate`. That is a choice, not an
    /// impossibility: a rest/bind candidate does record what its source
    /// measured. `build_rest_bind` rebases an affected node's local scale by
    /// `s_parent / s_node` with `s_node` the *declared* factor, and the
    /// affected closure never contains the scaled root's parent, so `s_parent`
    /// is one there and the candidate's composed root scale is exactly
    /// `s_observed / s_declared` — unit only when the two agree, which the
    /// input band admits without requiring. The candidate route is real and it
    /// is accurate: `plan.common_factor() * axis_length_average(axis_lengths(
    /// candidate root world linear))` recovers the measurement to a relative
    /// error below `2^-24`, the half-ulp of the one binary32 rounding that
    /// round trip costs at unit magnitude. Swept over all 215 binary32 values
    /// the `1e-5` band admits around a declared `0.01`, the worst is
    /// `5.9604613e-8`; over this module's own `0.01`-factor fixtures it is
    /// `4.84e-8`, at a source root of `0.010_000_099`.
    ///
    /// It is not the route taken, for four reasons:
    ///
    /// - **Independence.** [`prove_scale`] does not require `candidate` to
    ///   have come from [`build_scale_candidate`]; checking one that did not
    ///   is the reason it exists. Reading the reported measurement off the
    ///   artifact under test would let that artifact pick its own evidence
    ///   value. The observed factor is a fact about the *input*, so the input
    ///   is where it is measured.
    /// - **Precision.** The candidate route divides by the declared factor in
    ///   `f32` and multiplies back in `f64`, so it reports a rounded
    ///   neighbour of the source measurement rather than the measurement.
    /// - **Sign.** The nearest already-published proxy,
    ///   [`Self::unit_scale_residual`], is an absolute value, so
    ///   `common_factor * (1 + unit_scale_residual)` reconstructs
    ///   `s_observed` only when the observed factor is the *larger* of the
    ///   two and reflects it about the declared factor when it is not.
    /// - **Attribution.** That residual is also a maximum over every affected
    ///   node rather than a value read at the scaled root, so it does not
    ///   name the node §D.6 defines the observed factor at.
    ///
    /// See [`ScalePlan::observed_factor`] for how each operation defines it;
    /// for a whole-document conversion this is the declared factor, there
    /// being nothing to measure — and there the candidate route genuinely
    /// does not exist, because `build_whole_document` rewrites translations
    /// only and leaves every composed scale exactly as it found it.
    pub observed_factor: f64,
    /// The observed factor [`plan_scale`] measured, copied from
    /// [`ScalePlan::observed_factor`].
    ///
    /// The record carries both witnesses because they are measured from
    /// genuinely different state and neither is derivable from the other:
    /// this one from the raw source projection (`SourceNodeAsset::local_rest`
    /// composed through `parent_source_node_index`),
    /// [`Self::observed_factor`] from the normalized skeleton (`Bone::rest`
    /// composed through `world_rest_matrices`). That independence is the
    /// property DESIGN.md Appendix D §D.6 wants of a second witness, and it
    /// is why the two are generally not equal. Carrying only one of them
    /// would leave a reader unable to tell which was reported; carrying both
    /// with no stated relationship would leave them unable to tell which to
    /// trust, which is what [`Self::observed_factor_divergence`] answers.
    ///
    /// For [`ScaleOperation::WholeDocumentLinearUnits`] both are the declared
    /// factor, there being nothing to measure, and the divergence is exactly
    /// zero.
    pub planned_observed_factor: f64,
    /// How far apart the two observed factors are:
    /// `abs(planned - proved) / max(abs(planned), abs(proved))`.
    ///
    /// Recorded explicitly rather than left for a consumer to compute, so the
    /// evidence record states the relationship between its own two witnesses
    /// instead of presenting two numbers that both answer to "the observed
    /// factor". Compare it against
    /// [`ScaleTolerancePolicy::observed_factor_divergence_ceiling`], which is
    /// how far apart the design expects them to be and why — a consumer does
    /// not have to re-derive that ceiling by summing two separate policy
    /// fields.
    ///
    /// **Reported, not checked.** Nothing refuses a document for exceeding
    /// the ceiling; see that method for what the ceiling does and does not
    /// guarantee. A divergence beyond it means the source's raw projection
    /// parent chain and its skeleton parent chain disagree — nothing in this
    /// module reconciles the two, by construction — and that is a fact about
    /// the input worth surfacing, not a residual this proof owns.
    pub observed_factor_divergence: f64,
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
///
/// Three of the claims checked here are not gated by
/// [`ScaleProofObligations`]: the per-element animation-track values
/// ([`ProofResidualKind::TrackValue`]), base mesh `POSITION`
/// ([`ProofResidualKind::MeshPosition`]) and the stored inverse binds of
/// skins outside the affected closure
/// ([`ProofResidualKind::UnaffectedInverseBind`]). The latter two are about
/// payloads the plan declares it does *not* rewrite, where an obligation
/// flag would make "unaffected" opt-out; the first compares every element of
/// every track against its own domain's analytic expectation, rewritten or
/// retained, and so is owed by every plan. None of the three admits an
/// obligation flag as a proxy for having run — see [`ScaleProof`], whose
/// comparison counts report what each of them actually walked.
///
/// [`ScaleProof::observed_factor`] is re-derived here from `source` rather
/// than copied from [`ScalePlan::observed_factor`]; it is reported as
/// evidence and is not itself an obligation. Both witnesses and the
/// divergence between them are recorded
/// ([`ScaleProof::planned_observed_factor`],
/// [`ScaleProof::observed_factor_divergence`]); none of the three is checked
/// against a band here.
pub fn prove_scale(
    source: &Document,
    candidate: &ScaleCandidate,
    plan: &ScalePlan,
) -> Result<ScaleProof, ScaleError> {
    // The unit-scale postcondition is evaluated inside the `prove_rest` walk
    // — it reads the same affected node's candidate world matrix — so a plan
    // declaring it without `prove_rest` claims a postcondition nothing would
    // walk. Refused here rather than proved vacuously, for the same reason
    // every other declared-but-unevidenced obligation is: a `0.0` residual
    // for a claim nothing checked is a false record (DESIGN.md Appendix D
    // §D.6), and `prove_unit_scale_postcondition`'s own rustdoc states the
    // nesting as normative rather than incidental.
    //
    // This is a property of the plan alone, so it is checked before either
    // document is validated: no document can make the combination coherent,
    // and a caller holding such a plan should be told about it rather than
    // about whichever document defect happens to be found first. Neither
    // planner emits it — `plan_whole_document` leaves the postcondition off,
    // `plan_rest_bind` sets both — and `ScalePlan`'s fields are private, so
    // this closes a latent gap rather than a reachable one.
    if plan.proof_obligations.prove_unit_scale_postcondition && !plan.proof_obligations.prove_rest {
        return Err(ScaleError::MissingProofEvidence {
            kind: ProofResidualKind::UnitScale,
            detail: "unit_scale_postcondition_without_rest",
        });
    }
    let candidate = candidate.document();
    validate_document_shape(source)?;
    validate_document_shape(candidate)?;
    validate_candidate_structure(source, candidate)?;
    let tol = plan.tolerance_policy;
    let affected = plan.affected_set();
    let source_worlds = world_rests(&source.skeleton)?;
    let candidate_worlds = world_rests(&candidate.skeleton)?;

    let observed_factor = observed_factor_from_source(source, &source_worlds, plan)?;
    let mut proof = ScaleProof {
        tolerance_policy: tol,
        rest_translation_residual: 0.0,
        rest_translation_comparisons: 0,
        rest_rotation_residual: 0.0,
        rest_rotation_comparisons: 0,
        unit_scale_residual: 0.0,
        unit_scale_comparisons: 0,
        transform_only_affine_residual: 0.0,
        transform_only_affine_comparisons: 0,
        track_value_residual: 0.0,
        track_value_comparisons: 0,
        mesh_position_residual: 0.0,
        mesh_position_comparisons: 0,
        key_translation_residual: 0.0,
        key_translation_comparisons: 0,
        cubic_interior_residual: 0.0,
        cubic_interior_comparisons: 0,
        trajectory_residual: 0.0,
        trajectory_comparisons: 0,
        skin_matrix_residual: 0.0,
        skin_matrix_comparisons: 0,
        bounds_residual: 0.0,
        bounds_comparisons: 0,
        unaffected_inverse_bind_residual: 0.0,
        unaffected_inverse_bind_comparisons: 0,
        observed_factor,
        planned_observed_factor: plan.observed_factor,
        observed_factor_divergence: relative_divergence(plan.observed_factor, observed_factor),
        sample_time_count: 0,
    };

    check_candidate_values(source, candidate, &affected, plan, &tol, &mut proof)?;

    if plan.proof_obligations.prove_rest {
        // The affected closure is this obligation's whole evidence base, and
        // a plan only declares it when the closure is non-empty.
        if plan.affected_nodes.is_empty() {
            return Err(ScaleError::MissingProofEvidence {
                kind: ProofResidualKind::RestTranslation,
                detail: "empty_affected_closure",
            });
        }
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
                &mut proof,
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
            record_and_check(
                ProofResidualKind::RestRotation,
                rotation_residual,
                tol.rotation_residual_radians,
                &mut proof,
            )?;
            if plan.proof_obligations.prove_unit_scale_postcondition {
                let (after_scale, ..) = after.to_scale_rotation_translation();
                // Per-axis (L-infinity), per
                // [`ScaleTolerancePolicy::postcondition_unit_scale_residual`]:
                // "unit composed scale for every affected node" (DESIGN.md
                // Appendix D §D.6) is a per-axis claim, and measuring it
                // per-axis is what makes it commensurable with the scalar
                // relative common-factor band that gates the input. An L2
                // norm over the three axes reports `sqrt(3)` times the same
                // defect and therefore rejected candidates the very same
                // policy's input band had just accepted.
                let residual = (after_scale.x as f64 - 1.0)
                    .abs()
                    .max((after_scale.y as f64 - 1.0).abs())
                    .max((after_scale.z as f64 - 1.0).abs());
                record_and_check(
                    ProofResidualKind::UnitScale,
                    residual,
                    tol.postcondition_unit_scale_residual,
                    &mut proof,
                )?;
            }
        }
    }

    if plan.proof_obligations.prove_transform_only_affine {
        // A plan only declares this obligation when its closure actually
        // carried a transform-only attachment, so an empty list here means
        // the plan and the documents under proof disagree. Failing closed
        // matches `prove_bounds`: iterating an empty list instead would
        // report a `0.0` residual for §D.6's "a no-op cannot pass" claim
        // without ever transforming a probe point.
        if plan.transform_only_attachments.is_empty() {
            return Err(ScaleError::MissingProofEvidence {
                kind: ProofResidualKind::TransformOnlyAffine,
                detail: "no_transform_only_attachment",
            });
        }
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
                &mut proof,
            )?;
        }
    }

    check_skin_and_bounds(
        source,
        candidate,
        &source_worlds,
        &candidate_worlds,
        &affected,
        plan,
        &tol,
        &mut proof,
    )?;
    check_unaffected_instance_binds(source, candidate, &affected, plan, &tol, &mut proof)?;

    let any_sampled_obligation = plan.proof_obligations.prove_keys
        || plan.proof_obligations.prove_cubic_interiors
        || plan.proof_obligations.prove_trajectories
        || plan.proof_obligations.prove_skin
        || plan.proof_obligations.prove_bounds;
    if any_sampled_obligation {
        // The clip-driven obligations are declared only when the planned
        // document carried the tracks they read (see [`SampledEvidence`]), so
        // a `source` that carries none is not the document this plan was
        // computed against. Each obligation is refused separately and names
        // itself, because "no clips at all" and "clips with no affected
        // cubic segment" are different gaps and a caller repairing one should
        // not be told about the other.
        let evidence = sampled_evidence(source, &affected);
        if plan.proof_obligations.prove_keys && !evidence.key_translations {
            return Err(ScaleError::MissingProofEvidence {
                kind: ProofResidualKind::KeyTranslation,
                detail: "no_affected_translation_track",
            });
        }
        if plan.proof_obligations.prove_cubic_interiors && !evidence.cubic_interiors {
            return Err(ScaleError::MissingProofEvidence {
                kind: ProofResidualKind::CubicInterior,
                detail: "no_affected_cubic_interior_time",
            });
        }
        if plan.proof_obligations.prove_trajectories && !evidence.sample_times {
            return Err(ScaleError::MissingProofEvidence {
                kind: ProofResidualKind::Trajectory,
                detail: "no_affected_sample_time",
            });
        }
        // Harvested once, up front, for two reasons: the budget below has to
        // know the total sample count *before* the first sample is evaluated,
        // and re-harvesting per clip inside the loop would sort and dedup the
        // same key times twice.
        let mut clip_times = Vec::with_capacity(source.clips.len());
        let mut sample_times: u64 = 0;
        for clip in &source.clips {
            let times = clip_sample_times(clip, &affected);
            sample_times = sample_times
                .saturating_add(times.0.len() as u64)
                .saturating_add(times.1.len() as u64);
            clip_times.push(times);
        }
        let per_sample_cost = per_sample_work_units(source, &affected, plan);
        check_sampling_budget(&tol, sample_times, per_sample_cost)?;

        for (clip_index, clip) in source.clips.iter().enumerate() {
            let candidate_clip =
                candidate
                    .clips
                    .get(clip_index)
                    .ok_or(ScaleError::MissingProofEvidence {
                        kind: ProofResidualKind::KeyTranslation,
                        detail: "candidate_clip_missing",
                    })?;
            let (key_times, interior_times) = &clip_times[clip_index];
            for &t in key_times {
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
                        &mut proof,
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
            for &t in interior_times {
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
                        &mut proof,
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

/// Every obligation one sample time owes, evaluated against **one** pair of
/// world-matrix arrays.
///
/// The trajectory, skin, and bounds obligations all need the same source and
/// candidate poses at `t`. Deriving them once here — rather than letting each
/// obligation call [`world_at_time`] for itself, which recomputed forward
/// kinematics up to three times per sample per document — is what keeps the
/// proof's cost linear in the sample count rather than a fixed multiple of
/// it, and is a precondition for the sampling budget in [`prove_scale`] being
/// a meaningful bound on real work.
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
    if !plan.proof_obligations.prove_trajectories
        && !plan.proof_obligations.prove_skin
        && !plan.proof_obligations.prove_bounds
    {
        return Ok(());
    }
    let source_worlds = world_at_time(&source.skeleton, source_clip, t)?;
    let candidate_worlds = world_at_time(&candidate.skeleton, candidate_clip, t)?;
    if plan.proof_obligations.prove_trajectories {
        check_trajectory_residual_at(
            &source_worlds,
            &candidate_worlds,
            &plan.affected_nodes,
            plan,
            tol,
            proof,
        )?;
    }
    check_skin_and_bounds(
        source,
        candidate,
        &source_worlds,
        &candidate_worlds,
        affected,
        plan,
        tol,
        proof,
    )
}

/// The two document sides — source and candidate — every sampled obligation
/// walks. [`sample_time_obligations`] poses both skeletons, and
/// [`check_skin_and_bounds`] resolves both slot palettes and skins both
/// vertex arrays, so every term of [`per_sample_work_units`] is charged twice.
const PROOF_SIDES: u64 = 2;

/// Refuse a document whose total sampled work exceeds
/// [`ScaleTolerancePolicy::proof_sample_work_budget`], before the first
/// sample time is evaluated.
///
/// A free function rather than an inline comparison in [`prove_scale`] so
/// that the boundary itself is directly testable on synthetic numbers. The
/// budget is a ceiling the document may *reach*: the comparison is `>`, not
/// `>=`, exactly as [`check_residual`]'s is, and for the same reason —
/// DESIGN.md Appendix D §D.1 states every policy quantity as an inclusive
/// "at most". Pinning that end to end would mean a document that then costs
/// `1e8` work units to prove; pinning it here costs nothing and asserts the
/// same thing.
///
/// # Errors
///
/// Returns [`ScaleError::ProofSamplingBudgetExceeded`] carrying both factors
/// and the product, so the caller can see which of the two is oversized.
fn check_sampling_budget(
    tol: &ScaleTolerancePolicy,
    sample_times: u64,
    per_sample_cost: u64,
) -> Result<(), ScaleError> {
    let work = sample_times.saturating_mul(per_sample_cost);
    if work > tol.proof_sample_work_budget {
        return Err(ScaleError::ProofSamplingBudgetExceeded {
            policy_id: tol.id,
            sample_times,
            per_sample_cost,
            work,
            budget: tol.proof_sample_work_budget,
        });
    }
    Ok(())
}

/// Work units one sample time costs, for
/// [`ScaleTolerancePolicy::proof_sample_work_budget`].
///
/// The charge is what [`sample_time_obligations`] and
/// [`check_skin_and_bounds`] actually perform at one sample time, term by
/// term. Everything they walk, they walk for **both** document sides, so
/// every term below carries the [`PROOF_SIDES`] factor:
///
/// - one forward-kinematics pass over the skeleton per side, owed by every
///   sampled obligation — hence `2 * bone_count`, always charged. Only the
///   *source* skeleton is measured here, which is sound only because
///   [`validate_candidate_structure`] has already rejected a candidate whose
///   bone count differs; see the note on its `bone_count_mismatch` clause for
///   what an unchecked candidate skeleton cost;
/// - per affected skinned instance, one `world * inverse_bind` product per
///   [`MeshInstance::skin_joints`] slot per side, plus one residual
///   comparison per slot when the skin obligation is declared; and
/// - per affected skinned instance, every vertex of **every** primitive of
///   its mesh per side, when the bounds obligation is declared.
///
/// The slot term is charged explicitly because nothing bounds it and nothing
/// else stands in for it. An earlier revision charged only `bone_count +
/// vertices` on the claim that slot work "cannot exceed the bone count",
/// which is false twice over: [`validate_scene_assets`] only range-checks
/// joint ids, so `skin_joints` may repeat a joint and be arbitrarily long,
/// and the instance count is unbounded, so the total is
/// `sum over instances of len(skin_joints)` with no relation to
/// `bone_count` at all. A legal 400-instance document with 300 slots each and
/// one vertex per instance was charged `120_600` while performing `36_000_000`
/// slot matrix products — a `299x` undercount, and unbounded in general.
///
/// This bounds the *sampled* work, which is what grows with the document's
/// key count. [`prove_scale`] additionally evaluates the rest pose once,
/// outside the sampled loop and outside this budget; that is one extra pose
/// of the same shape, not a term that scales with anything.
fn per_sample_work_units(
    document: &Document,
    affected: &BTreeSet<BoneId>,
    plan: &ScalePlan,
) -> u64 {
    let mut units = PROOF_SIDES.saturating_mul(document.skeleton.bones.len() as u64);
    let prove_skin = plan.proof_obligations.prove_skin;
    let prove_bounds = plan.proof_obligations.prove_bounds;
    if !prove_skin && !prove_bounds {
        return units;
    }
    for instance in &document.assets.instances {
        if !instance
            .skin_joints
            .iter()
            .any(|joint| affected.contains(joint))
        {
            continue;
        }
        let slots = instance.skin_joints.len() as u64;
        units = units.saturating_add(PROOF_SIDES.saturating_mul(slots));
        if prove_skin {
            units = units.saturating_add(slots);
        }
        if !prove_bounds {
            continue;
        }
        let Some(mesh) = document.assets.meshes.get(instance.mesh) else {
            continue;
        };
        for primitive in &mesh.primitives {
            units =
                units.saturating_add(PROOF_SIDES.saturating_mul(primitive.positions.len() as u64));
        }
    }
    units
}

/// The inverse bind `document` *stores* for one skin slot, in this module's
/// precedence order — the instance's own per-slot array first, then the
/// bone convenience value — or `None` when it stores neither.
///
/// Deliberately stops short of [`instance_bind`]'s last step, the
/// format-defined identity default a complete-coverage source skin with an
/// [`SourceInverseBindAccessorStatus::Absent`] accessor licenses. That step
/// answers "what bind does this slot *have*", which is the right question
/// when composing `W * B`; this one answers "what bind does this document
/// *record*", which is the question [`check_unaffected_instance_binds`] is
/// asking.
///
/// The two questions genuinely disagree on one input — an evidence-free slot
/// under an `Absent` accessor, where the *effective* bind is identity and the
/// *recorded* one is nothing — and asking the recorded question there refuses
/// a candidate that only changed representation. That is a known and
/// deliberate narrowness, not an oversight; see the trap note on
/// [`check_unaffected_instance_binds`] for why it is left standing and what
/// replacing it would take.
///
/// The out-of-range branch is unreachable for a document that passed
/// [`validate_scene_assets`], which requires a non-empty `skin_ibms` to be
/// exactly as long as `skin_joints`; it is kept because this function is
/// total over its arguments rather than over its current call sites.
fn stored_instance_bind(
    document: &Document,
    instance: &MeshInstance,
    slot: usize,
    joint: BoneId,
) -> Result<Option<Mat4>, ScaleError> {
    if !instance.skin_ibms.is_empty() {
        return instance
            .skin_ibms
            .get(slot)
            .copied()
            .map(Some)
            .ok_or(ScaleError::BoneIndexOutOfRange { index: joint });
    }
    let bone = document
        .skeleton
        .bones
        .get(joint)
        .ok_or(ScaleError::BoneIndexOutOfRange { index: joint })?;
    Ok(bone.inverse_bind)
}

/// Resolve one skin joint's inverse-bind matrix per the documented
/// [`MeshInstance::skin_ibms`] contract: use the instance's own matrix when
/// present, else fall back to the bone's [`crate::model::Bone::inverse_bind`]
/// — the only fallback this model contract genuinely represents
/// ([`stored_instance_bind`]).
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
    if let Some(stored) = stored_instance_bind(document, instance, slot, joint)? {
        return Ok(stored);
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
///
/// The guard is `!observed.is_finite()` rather than `observed.is_nan()`, and
/// the difference is narrower than it looks: `+inf > tolerance` is true, so
/// the comparison alone already rejects a positive infinity. Only a
/// *negative* non-finite residual needs the wider guard, and no caller in
/// this module can produce one — every `observed` here is an `abs()`, a
/// `length()`, or a `max` fold over those. The wider spelling is kept because
/// this function's contract is the fail-closed one stated above rather than
/// "whatever today's callers happen to pass", and it is pinned by
/// `a_non_finite_residual_fails_closed_instead_of_comparing_false`.
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

/// One residual's maximum and comparison count, borrowed **together** so a
/// comparison site that reaches one has already reached the other.
///
/// [`ScaleProof`] publishes the two as separate flat fields, because that is
/// what a `#[non_exhaustive]` additive change permits and what the evidence
/// schema reads. Pairing them here, and recording through [`Self::record`]
/// rather than through either reference, makes "a maximum is never raised
/// without its count" the single intended route to a residual field — not an
/// enforced one: Rust privacy is module-scoped and this module is one file,
/// so `*tally.max` or `*tally.comparisons` written on their own still
/// compile. What enforces the convention is the hand-derived count vectors
/// the tests state, starting with
/// `a_whole_document_proof_counts_and_measures_every_comparison_it_makes`:
/// they name every residual's count as a fact, so a comparison that skips
/// its count and a count that skips its comparison each move them.
struct ResidualTally<'a> {
    max: &'a mut f64,
    comparisons: &'a mut usize,
}

impl ResidualTally<'_> {
    /// Record one comparison of `observed`.
    fn record(&mut self, observed: f64) {
        *self.max = self.max.max(observed);
        *self.comparisons += 1;
    }
}

impl ScaleProof {
    /// The maximum/count pair this residual kind reports into.
    ///
    /// The single mapping from a [`ProofResidualKind`] to the fields it
    /// writes. Every comparison site names its kind and nothing else, so a
    /// site cannot report one kind's residual into another kind's field —
    /// which was previously possible wherever a `&mut f64` and a `kind` were
    /// passed as independent arguments.
    ///
    /// [`ProofResidualKind::ObservedFactor`] is not a residual — it names a
    /// source whose scaled root could not be resolved, and is only ever
    /// reported as [`ScaleError::MissingProofEvidence`] — so it has no pair
    /// and no comparison site reaches here with it.
    fn tally(&mut self, kind: ProofResidualKind) -> Option<ResidualTally<'_>> {
        let (max, comparisons) = match kind {
            ProofResidualKind::RestTranslation => (
                &mut self.rest_translation_residual,
                &mut self.rest_translation_comparisons,
            ),
            ProofResidualKind::RestRotation => (
                &mut self.rest_rotation_residual,
                &mut self.rest_rotation_comparisons,
            ),
            ProofResidualKind::UnitScale => (
                &mut self.unit_scale_residual,
                &mut self.unit_scale_comparisons,
            ),
            ProofResidualKind::TransformOnlyAffine => (
                &mut self.transform_only_affine_residual,
                &mut self.transform_only_affine_comparisons,
            ),
            ProofResidualKind::TrackValue => (
                &mut self.track_value_residual,
                &mut self.track_value_comparisons,
            ),
            ProofResidualKind::MeshPosition => (
                &mut self.mesh_position_residual,
                &mut self.mesh_position_comparisons,
            ),
            ProofResidualKind::KeyTranslation => (
                &mut self.key_translation_residual,
                &mut self.key_translation_comparisons,
            ),
            ProofResidualKind::CubicInterior => (
                &mut self.cubic_interior_residual,
                &mut self.cubic_interior_comparisons,
            ),
            ProofResidualKind::Trajectory => (
                &mut self.trajectory_residual,
                &mut self.trajectory_comparisons,
            ),
            ProofResidualKind::SkinMatrix => (
                &mut self.skin_matrix_residual,
                &mut self.skin_matrix_comparisons,
            ),
            ProofResidualKind::Bounds => (&mut self.bounds_residual, &mut self.bounds_comparisons),
            ProofResidualKind::UnaffectedInverseBind => (
                &mut self.unaffected_inverse_bind_residual,
                &mut self.unaffected_inverse_bind_comparisons,
            ),
            ProofResidualKind::ObservedFactor => return None,
        };
        Some(ResidualTally { max, comparisons })
    }
}

/// Record one comparison of `observed` for `kind` and check it against
/// `tolerance`.
///
/// The single point at which a residual maximum moves. Recording and
/// checking here — rather than at each of the twelve obligations' loops —
/// is what makes [`ScaleProof`]'s counts describe exactly the comparisons
/// its maxima were taken over.
fn record_and_check(
    kind: ProofResidualKind,
    observed: f64,
    tolerance: f64,
    proof: &mut ScaleProof,
) -> Result<(), ScaleError> {
    if let Some(mut tally) = proof.tally(kind) {
        tally.record(observed);
    }
    check_residual(kind, observed, tolerance)
}

/// Record `observed` for `kind` and check it against the
/// before/after-derived tolerance for this specific comparison — never a
/// proxy such as the plan's declared factor.
fn check_and_track(
    kind: ProofResidualKind,
    observed: f64,
    before: f64,
    after: f64,
    tol: &ScaleTolerancePolicy,
    proof: &mut ScaleProof,
) -> Result<(), ScaleError> {
    record_and_check(kind, observed, tol.scalar_tolerance(before, after), proof)
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
                            proof,
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
                            proof,
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
                    proof,
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
    proof: &mut ScaleProof,
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
            proof,
        )?;
    }
    Ok(())
}

/// Sampled world-space trajectory residual for one already-derived pose pair
/// (see [`sample_time_obligations`], which owns the single [`world_at_time`]
/// evaluation these matrices come from).
fn check_trajectory_residual_at(
    source_worlds: &[Mat4],
    candidate_worlds: &[Mat4],
    affected_nodes: &[BoneId],
    plan: &ScalePlan,
    tol: &ScaleTolerancePolicy,
    proof: &mut ScaleProof,
) -> Result<(), ScaleError> {
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
            proof,
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

/// `abs(planned - proved) / max(abs(planned), abs(proved))` — the divergence
/// between the two observed-factor witnesses, on the same comparison base
/// [`ScaleTolerancePolicy::relative`] uses.
///
/// Sharing that base is what makes the reported number commensurable with
/// [`ScaleTolerancePolicy::observed_factor_divergence_ceiling`], which is a
/// sum of two bands stated on it. The one division this spelling costs is
/// what a predicate would not pay, so the two are not bit-identical near the
/// ceiling; nothing here compares against it, so nothing depends on that.
///
/// No floor on the base, for [`ScaleTolerancePolicy::relative`]'s reasons,
/// and none is needed: `planned` is strictly positive under both operations —
/// a declared factor [`plan_scale`] range-checked, or an observed one
/// [`classify_affine`] proved non-singular — so the base is at least
/// `planned` and never zero. `proved` carries no such guarantee: it is read
/// from whichever `source` [`prove_scale`] was handed, and a degenerate one
/// measuring exactly zero there reports a divergence of one rather than a
/// division by zero.
fn relative_divergence(planned: f64, proved: f64) -> f64 {
    (planned - proved).abs() / planned.abs().max(proved.abs())
}

/// Re-derive this operation's observed factor from `source`, without reading
/// [`ScalePlan::observed_factor`].
///
/// For [`ScaleOperation::RestBindUniformScale`] the scaled root is resolved
/// the same way [`plan_rest_bind`] resolves it — by `source_root_node_index`
/// through `source`'s own source-node projection — and its factor is then
/// measured from the *normalized* skeleton's rest-world matrix rather than
/// from the raw projection planning classified. That makes this a genuinely
/// second witness: it reads different stored data, through a different
/// composition path, and it is computed from whichever `source` this call was
/// handed, which [`prove_scale`] does not require to be the document the plan
/// came from.
///
/// It is deliberately *not* a re-run of [`classify_affine`]. Re-classifying
/// here would add a fresh rejection path — a proof source outside the
/// supported affine class would fail with a domain error rather than the
/// residual that actually matters — for no gain: whether the source is in the
/// class is a planning question, already answered, and the quantity wanted
/// here is only the factor.
///
/// # Errors
///
/// [`ScaleError::MissingProofEvidence`] when the plan's scaled root has no
/// projection in `source` to measure, and [`ScaleError::BoneIndexOutOfRange`]
/// when it projects to a bone `source` does not have. Neither is silently
/// reported as a zero factor.
fn observed_factor_from_source(
    source: &Document,
    source_worlds: &[Mat4],
    plan: &ScalePlan,
) -> Result<f64, ScaleError> {
    let ScaleOperation::RestBindUniformScale {
        source_root_node_index,
        ..
    } = plan.operation
    else {
        // Whole-document conversion declares its factor rather than observing
        // it; see [`ScalePlan::observed_factor`].
        return Ok(plan.common_factor);
    };
    let bone = source_node_index_map(source)
        .get(&source_root_node_index)
        .and_then(|asset| asset.bone)
        .ok_or(ScaleError::MissingProofEvidence {
            kind: ProofResidualKind::ObservedFactor,
            detail: "scaled_root_not_projected",
        })?;
    let world = *source_worlds
        .get(bone)
        .ok_or(ScaleError::BoneIndexOutOfRange { index: bone })?;
    Ok(axis_length_average(axis_lengths(Mat3::from_mat4(world))))
}

/// Prove that inverse-bind evidence *outside* the affected closure came
/// through unchanged.
///
/// [`check_skin_and_bounds`] skips an instance with no joint in the affected
/// closure entirely, and nothing else looked at one either — so a candidate
/// that rewrote an unrelated skin's `skin_ibms` proved `Ok`. Reachable
/// through the public API, since [`build_scale_candidate`] and
/// [`prove_scale`] do not require their two documents to be the same one.
///
/// Three cases, kept distinct on purpose (issue #296):
///
/// - both sides store a bind for the slot — compared directly, as the
///   arrays they are;
/// - exactly one side stores one — [`ScaleError::MissingProofEvidence`]. A
///   candidate that dropped an array (falling back to a different bind) or
///   materialized one where the source had none has changed the skin, and
///   there is nothing to compare it against;
/// - neither side stores one — nothing to compare, and nothing is claimed.
///
/// That third case is what makes this fail-*closed* rather than
/// fail-*everything*. Resolving both sides through [`instance_bind`] instead
/// would have been the obvious implementation and would newly reject a
/// document carrying an unrelated skin with no bind evidence at all
/// (`MissingInverseBind`) — a document the operation genuinely does not
/// touch. So the resolution here stops at what each document *stores*
/// ([`stored_instance_bind`]) and reports the evidence-free slot as out of
/// scope rather than as proven.
///
/// Scope, stated exactly: this compares the bind each side resolves *for a
/// slot*, in the module's own precedence order — the instance array first,
/// then the bone convenience value. A [`crate::model::Bone::inverse_bind`]
/// that is shadowed by a non-empty `skin_ibms` is not authority for any slot
/// and is not compared here, and neither is
/// [`crate::model::SourceSkinAsset::inverse_bind_accessor`], which is
/// read-side evidence about the input accessor rather than a bind either
/// planning or proof consumes.
///
/// The skip is `any`, not `all`, and that is load-bearing rather than
/// idiomatic. An instance with *some* joint in the affected closure belongs to
/// [`check_skin_and_bounds`], which checks `W * B` on both sides and expects
/// exactly the rewrite [`build_rest_bind`] performs on those slots. Holding
/// such an instance to "binds unchanged" as well rejects this module's own
/// output — pinned by
/// `a_partially_affected_skin_stays_with_the_skin_obligation_that_owns_it`.
///
/// # Known trap: a semantics-preserving representation change is refused
///
/// The one-stored-side rule is stated over *stored* evidence, and there is a
/// case where the two sides store different things and mean the same thing.
/// A complete-coverage source skin whose
/// [`crate::model::SourceSkinAsset::inverse_bind_accessor`] is
/// [`SourceInverseBindAccessorStatus::Absent`] licenses the format-defined
/// identity default, so [`instance_bind`] resolves a slot with no stored
/// evidence to [`Mat4::IDENTITY`]. A candidate that materializes that default
/// as an explicit `[IDENTITY]` array has changed nothing about the effective
/// bind — and is exactly what [`build_rest_bind`] does for an *affected*
/// instance, deliberately — yet is refused here as
/// `MissingProofEvidence { source_slot_bind_missing }`, and the converse as
/// `candidate_slot_bind_missing`.
///
/// This is not reachable through the shipped pipeline: the only producer,
/// [`build_rest_bind`], materializes an array only for an instance with an
/// affected joint, and this obligation skips those. It is a trap for a future
/// rest/bind frontend that normalizes bind representation on the way out.
///
/// It is left refusing rather than relaxed, for two reasons. First, the
/// refusal is the behaviour DESIGN.md §D.6 now states outright ("a slot
/// exactly one side records is a rewritten skin and is refused"), so
/// admitting the representation change is a contract amendment rather than a
/// bug fix. Second, the narrow patch — resolving through [`instance_bind`]
/// only in the one-sided rows — leaves the three rows incoherent, because the
/// neither-stored row would then be the only one that does *not* consult the
/// format default it is entirely about. The coherent alternative is a
/// differently-shaped rule: resolve both sides through [`instance_bind`] and
/// treat [`ScaleError::MissingInverseBind`] on *both* sides as the
/// out-of-scope case, which preserves the fail-closed property the third row
/// exists for while comparing effective binds throughout. That is a design
/// change and belongs in its own issue.
///
/// Unconditional, like the base `POSITION` comparison in
/// [`check_candidate_values`] and for the same reason: it is a structural
/// claim about payloads the plan declares *unaffected*, so there is no
/// obligation flag that could switch it off without also making the plan's
/// "unaffected" claim unfalsifiable.
fn check_unaffected_instance_binds(
    source: &Document,
    candidate: &Document,
    affected: &BTreeSet<BoneId>,
    plan: &ScalePlan,
    tol: &ScaleTolerancePolicy,
    proof: &mut ScaleProof,
) -> Result<(), ScaleError> {
    // Zipped rather than indexed: `validate_candidate_structure` has already
    // proved the two instance lists have equal length and pairwise equal
    // `skin_joints`, so pairing them positionally needs no fallible lookup
    // and cannot silently drop a trailing instance.
    for (instance, candidate_instance) in source
        .assets
        .instances
        .iter()
        .zip(candidate.assets.instances.iter())
    {
        if instance
            .skin_joints
            .iter()
            .any(|joint| affected.contains(joint))
        {
            continue;
        }
        for (slot, &joint) in instance.skin_joints.iter().enumerate() {
            let before = stored_instance_bind(source, instance, slot, joint)?;
            let after = stored_instance_bind(candidate, candidate_instance, slot, joint)?;
            let (before, after) = match (before, after) {
                (None, None) => continue,
                (Some(_), None) => {
                    return Err(ScaleError::MissingProofEvidence {
                        kind: ProofResidualKind::UnaffectedInverseBind,
                        detail: "candidate_slot_bind_missing",
                    });
                }
                (None, Some(_)) => {
                    return Err(ScaleError::MissingProofEvidence {
                        kind: ProofResidualKind::UnaffectedInverseBind,
                        detail: "source_slot_bind_missing",
                    });
                }
                (Some(before), Some(after)) => (before, after),
            };
            // The same analytic expectation [`check_skin_and_bounds`] states
            // for an affected slot's inverse bind: whole-document conversion
            // conjugates every retained matrix as `U B U^-1`, which for a
            // uniform `U` scales the translation column and leaves the linear
            // part alone; rest/bind reparameterization does not touch a skin
            // outside its closure at all.
            //
            // For a whole-document plan and the document it was planned
            // against the conversion branch is unreachable, and provably so:
            // `plan_whole_document` marks every bone affected, and
            // `validate_scene_assets` rejects a `skin_joints` entry outside
            // the bone range, so an instance that reaches here at all has an
            // empty `skin_joints` and this loop has no iterations. It is
            // reachable — and pinned by
            // `a_whole_document_plan_replayed_against_an_extra_bone_still_holds_its_binds_to_the_factor`
            // — only when the plan is replayed against a document with bones
            // its affected set never covered, which this module guards
            // everywhere else rather than assuming away.
            let expected = if plan.is_whole_document() {
                scale_translation_only(before, plan.common_factor as f32)
            } else {
                before
            };
            let residual = matrix_residual(expected, after);
            check_and_track(
                ProofResidualKind::UnaffectedInverseBind,
                residual,
                matrix_magnitude(expected),
                matrix_magnitude(after),
                tol,
                proof,
            )?;
        }
    }
    Ok(())
}

/// The skin-equation and skinned-bounds obligations, evaluated in **one**
/// walk over the affected skinned instances.
///
/// Both obligations need the same three things per instance: the instance's
/// joint world matrices, its resolved inverse binds, and — for bounds — its
/// vertices. Splitting them across two entry points meant resolving the binds
/// twice and, worse, walking every vertex twice for bounds alone (once for
/// the source document, once for the candidate). This walks each vertex once
/// and skins it through both sides.
///
/// [`validate_candidate_structure`] has already established that paired
/// instances agree on `mesh` and `skin_joints` and that paired meshes agree
/// on primitive and vertex counts, so the two sides are known to have the
/// same slots and the same vertices to walk.
#[allow(clippy::too_many_arguments)]
fn check_skin_and_bounds(
    source: &Document,
    candidate: &Document,
    source_worlds: &[Mat4],
    candidate_worlds: &[Mat4],
    affected: &BTreeSet<BoneId>,
    plan: &ScalePlan,
    tol: &ScaleTolerancePolicy,
    proof: &mut ScaleProof,
) -> Result<(), ScaleError> {
    let prove_skin = plan.proof_obligations.prove_skin;
    let prove_bounds = plan.proof_obligations.prove_bounds;
    if !prove_skin && !prove_bounds {
        return Ok(());
    }
    // A plan only declares these two when its source actually carried a
    // skinned instance touching the affected closure, so reaching here with
    // none means the document handed to `prove_scale` is not the one the plan
    // was computed against. Failing closed here matches how every other
    // missing-counterpart branch behaves; returning `Ok` instead would report
    // a `0.0` skin-matrix or bounds residual for a claim that was never
    // checked.
    if (prove_skin || prove_bounds) && !has_skinned_evidence(source, affected) {
        return Err(ScaleError::MissingProofEvidence {
            // One gap, reported once, named for whichever of the two the plan
            // declared — and for `prove_bounds` when it declared both, which
            // every planner-built plan does. This gate was `prove_bounds`'s
            // alone (#288) before `prove_skin` joined it, and what a
            // planner-built plan reports here must not move underneath a
            // caller matching on it.
            kind: if prove_bounds {
                ProofResidualKind::Bounds
            } else {
                ProofResidualKind::SkinMatrix
            },
            detail: "no_skinned_instance_in_affected_closure",
        });
    }

    let mut source_bounds = BoundsAccumulator::default();
    let mut candidate_bounds = BoundsAccumulator::default();

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

        // Resolved once and reused by both obligations.
        let mut source_slots = Vec::with_capacity(instance.skin_joints.len());
        let mut candidate_slots = Vec::with_capacity(instance.skin_joints.len());
        for (slot, &joint) in instance.skin_joints.iter().enumerate() {
            let before_world = *source_worlds
                .get(joint)
                .ok_or(ScaleError::BoneIndexOutOfRange { index: joint })?;
            let after_world = *candidate_worlds
                .get(joint)
                .ok_or(ScaleError::BoneIndexOutOfRange { index: joint })?;
            let before_ibm = instance_bind(source, instance, slot, joint)?;
            let after_ibm = instance_bind(candidate, candidate_instance, slot, joint)?;
            source_slots.push(before_world * before_ibm);
            candidate_slots.push(after_world * after_ibm);
        }

        if prove_skin {
            for (before, after) in source_slots.iter().zip(candidate_slots.iter()) {
                // Whole-document conversion scales every affine's translation
                // by the declared factor while leaving its linear part
                // unchanged (the same `U M U^-1` conjugation as any other
                // retained matrix); rest/bind reparameterization analytically
                // preserves the skin equation exactly.
                let expected = if plan.is_whole_document() {
                    scale_translation_only(*before, plan.common_factor as f32)
                } else {
                    *before
                };
                let residual = matrix_residual(expected, *after);
                check_and_track(
                    ProofResidualKind::SkinMatrix,
                    residual,
                    matrix_magnitude(expected),
                    matrix_magnitude(*after),
                    tol,
                    proof,
                )?;
            }
        }

        if prove_bounds {
            let mesh =
                source
                    .assets
                    .meshes
                    .get(instance.mesh)
                    .ok_or(ScaleError::InvalidMeshInstance {
                        instance_index,
                        reason: "mesh_index_out_of_range",
                    })?;
            let candidate_mesh = candidate.assets.meshes.get(candidate_instance.mesh).ok_or(
                ScaleError::InvalidMeshInstance {
                    instance_index,
                    reason: "mesh_index_out_of_range",
                },
            )?;
            for (primitive_index, (primitive, candidate_primitive)) in mesh
                .primitives
                .iter()
                .zip(candidate_mesh.primitives.iter())
                .enumerate()
            {
                accumulate_skinned_bounds(
                    instance_index,
                    primitive_index,
                    primitive,
                    &source_slots,
                    &mut source_bounds,
                )?;
                accumulate_skinned_bounds(
                    instance_index,
                    primitive_index,
                    candidate_primitive,
                    &candidate_slots,
                    &mut candidate_bounds,
                )?;
            }
        }
    }

    if !prove_bounds {
        return Ok(());
    }
    let (before_min, before_max) =
        source_bounds
            .finish()
            .ok_or(ScaleError::MissingProofEvidence {
                kind: ProofResidualKind::Bounds,
                detail: "source_bounds_missing",
            })?;
    let (after_min, after_max) =
        candidate_bounds
            .finish()
            .ok_or(ScaleError::MissingProofEvidence {
                kind: ProofResidualKind::Bounds,
                detail: "candidate_bounds_missing",
            })?;
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
            check_and_track(ProofResidualKind::Bounds, residual, expected, a, tol, proof)?;
        }
    }
    Ok(())
}

/// Whether `document` carries at least one skinned mesh instance with a
/// joint inside `affected` — the only evidence the skin-equation and bounds
/// obligations can be checked against.
fn has_skinned_evidence(document: &Document, affected: &BTreeSet<BoneId>) -> bool {
    document.assets.instances.iter().any(|instance| {
        instance
            .skin_joints
            .iter()
            .any(|joint| affected.contains(joint))
    })
}

/// Which of the clip-driven obligations `document` carries evidence for
/// inside `affected`.
///
/// Each field is exactly the condition under which that obligation's residual
/// loop reads at least one payload, so a plan that declares the obligation is
/// declaring something [`prove_scale`] will actually check, and the residual
/// it reports is a measurement rather than the zero an empty loop leaves
/// behind. Computed the same way on both sides of the plan/proof boundary:
/// [`plan_scale`] uses it to decide what to declare, and [`prove_scale`] uses
/// it to fail closed when a declared obligation's evidence is not in the
/// document it was handed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct SampledEvidence {
    /// Some clip declares a translation track on an affected bone. That
    /// track is both what puts key times into [`clip_sample_times`] and the
    /// only payload [`check_track_value_residual`] compares, so it is the
    /// whole evidence base for `prove_keys`.
    key_translations: bool,
    /// Some clip declares *both* an affected cubic-spline track with at least
    /// two key times — which is what produces an interior time at all — and
    /// an affected translation track, which is what the comparison at that
    /// interior time reads. The two need not be the same track, but they must
    /// be in the same clip: interior times are harvested per clip and
    /// compared against that clip's tracks.
    cubic_interiors: bool,
    /// Some clip yields at least one sample time for an affected bone, of
    /// either kind. The trajectory obligation compares composed world
    /// matrices rather than track payloads, so any affected track's key times
    /// are evidence for it.
    sample_times: bool,
}

/// Measure [`SampledEvidence`] over `document`'s clips.
fn sampled_evidence(document: &Document, affected: &BTreeSet<BoneId>) -> SampledEvidence {
    let mut evidence = SampledEvidence::default();
    for clip in &document.clips {
        let mut translations = false;
        let mut cubic_segments = false;
        for track in &clip.tracks {
            if !affected.contains(&track.bone) || track.times.is_empty() {
                continue;
            }
            evidence.sample_times = true;
            translations |= track.property == Property::Translation;
            cubic_segments |=
                track.interpolation == Interpolation::CubicSpline && track.times.len() >= 2;
        }
        evidence.key_translations |= translations;
        evidence.cubic_interiors |= translations && cubic_segments;
    }
    evidence
}

/// Running skinned-bounds extremes for one document side.
///
/// `touched` distinguishes "every relevant vertex was unweighted" from "the
/// bounds happen to be at the origin": the former has no bounds evidence at
/// all and must be reported as missing, not as a zero residual.
struct BoundsAccumulator {
    min: Vec3,
    max: Vec3,
    touched: bool,
}

impl Default for BoundsAccumulator {
    fn default() -> Self {
        Self {
            min: Vec3::splat(f32::INFINITY),
            max: Vec3::splat(f32::NEG_INFINITY),
            touched: false,
        }
    }
}

impl BoundsAccumulator {
    fn finish(self) -> Option<(Vec3, Vec3)> {
        self.touched.then_some((self.min, self.max))
    }
}

/// Skin one primitive's vertices through `slots` (already-composed
/// `W_i * B_i` per skin slot) and fold them into `bounds`, rejecting rather
/// than skipping every malformed input along the way: a primitive whose
/// per-vertex `joints`/`weights` are not exactly parallel to `positions`, a
/// non-finite position or weight, a joint-influence slot outside the
/// instance's `skin_joints`, or a non-finite skinned result. A vertex whose
/// four weights are all zero is legitimately unweighted (not malformed) and
/// is excluded from bounds.
fn accumulate_skinned_bounds(
    instance_index: usize,
    primitive_index: usize,
    primitive: &Primitive,
    slots: &[Mat4],
    bounds: &mut BoundsAccumulator,
) -> Result<(), ScaleError> {
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
            let Some(&skin_matrix) = slots.get(joints[slot_index] as usize) else {
                return Err(ScaleError::InvalidSkinnedPrimitive {
                    instance_index,
                    primitive_index,
                    reason: "joint_influence_slot_out_of_range",
                });
            };
            skinned += weight * skin_matrix.transform_point3(position);
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
            bounds.min = bounds.min.min(skinned);
            bounds.max = bounds.max.max(skinned);
            bounds.touched = true;
        }
    }
    Ok(())
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
    fn a_candidate_wrapped_from_an_external_document_is_proved_rather_than_trusted() {
        let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        let capability = complete_capability();
        let plan = plan_scale(&ScaleRequest {
            operation: ScaleOperation::WholeDocumentLinearUnits { factor: 0.01 },
            document: &doc,
            capability: &capability,
        })
        .unwrap();

        // Converted here by hand, exactly as a format frontend's reloaded
        // artifact arrives: never through `build_scale_candidate`.
        let mut converted = doc.clone();
        for bone in &mut converted.skeleton.bones {
            bone.rest.translation *= 0.01;
        }
        for node in &mut converted.assets.source_skeleton.nodes {
            node.local_rest = match &node.local_rest {
                SourceNodeLocalRest::Trs {
                    translation,
                    rotation,
                    scale,
                } => SourceNodeLocalRest::Trs {
                    translation: *translation * 0.01,
                    rotation: *rotation,
                    scale: *scale,
                },
                SourceNodeLocalRest::Matrix(matrix) => {
                    SourceNodeLocalRest::Matrix(scale_translation_only(*matrix, 0.01))
                }
            };
        }
        for mesh in &mut converted.assets.meshes {
            for primitive in &mut mesh.primitives {
                for position in &mut primitive.positions {
                    *position *= 0.01;
                }
            }
        }
        for instance in &mut converted.assets.instances {
            for inverse_bind in &mut instance.skin_ibms {
                *inverse_bind = scale_translation_only(*inverse_bind, 0.01);
            }
        }
        let proof = prove_scale(&doc, &ScaleCandidate::from_document(converted), &plan).unwrap();
        assert!(proof.rest_translation_residual < 1e-6);
        assert!(proof.mesh_position_residual < 1e-6);

        // The constructor asserts nothing, and does not need to: the same
        // wrapper around an *unconverted* document is rejected by proof.
        let error = prove_scale(&doc, &ScaleCandidate::from_document(doc.clone()), &plan)
            .expect_err("an unconverted candidate must not prove");
        assert!(
            matches!(
                error,
                ScaleError::ProofResidualExceeded {
                    kind: ProofResidualKind::MeshPosition,
                    ..
                }
            ),
            "expected a MeshPosition residual, got {error:?}"
        );
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
        // The observed factor is measured *at the scaled root*, and here that
        // is bone 1, not bone 0. Bone 0's rest-world linear part is the
        // identity, so an implementation that measured a fixed bone zero — or
        // the first affected bone by any rule other than "the plan's resolved
        // scaled root" — would report `1.0`, a hundredfold error, rather than
        // `0.01f32` widened to `f64`. Every other observed-factor fixture in
        // this module puts its scaled root at bone 0 and cannot separate the
        // two.
        assert_eq!(plan.observed_factor(), NEAR_UNIT_OBSERVED_FACTOR);

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
        // Re-derived independently of the plan, and at the same bone 1.
        assert_eq!(proof.observed_factor, NEAR_UNIT_OBSERVED_FACTOR);
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

    /// A rig whose scaled root sits under an ancestor that is itself scaled
    /// and stays *outside* the closure, so the root's local scale and its
    /// composed rest-world scale are different numbers:
    ///
    /// ```text
    ///   bone 0  local scale 2      world scale 2      (boundary parent)
    ///   bone 1  local scale 0.005  world scale 0.01   (scaled root)
    ///   bone 2  local scale 1      world scale 0.01   (the skin's joint)
    /// ```
    ///
    /// Every step is exact in binary32: `0.005f32` is `0.01f32 / 2` exactly
    /// (halving is exact for a normal number), so `2 * 0.005f32 == 0.01f32`
    /// and the root's composed linear part is `0.01f32 * I` with no rounding
    /// at all. The observed factor is therefore the same
    /// [`NEAR_UNIT_OBSERVED_FACTOR`] every `0.01f32` fixture here reports,
    /// while the root's *local* scale widens to `0.004999999888241291` —
    /// half of it.
    fn scaled_ancestor_rig() -> Vec<RigNode> {
        vec![
            RigNode {
                parent: None,
                source_node_index: 0,
                translation: Vec3::new(5.0, 0.0, 0.0),
                rotation: Quat::IDENTITY,
                scale: Vec3::splat(2.0),
            },
            RigNode {
                parent: Some(0),
                source_node_index: 1,
                translation: Vec3::new(0.0, 2.0, 0.0),
                rotation: Quat::IDENTITY,
                scale: Vec3::splat(0.005),
            },
            rig(Some(1), 2, Vec3::new(0.0, 100.0, 0.0)),
        ]
    }

    fn scaled_ancestor_document() -> Document {
        // `B = inverse(W_rest(bone 2))` for `W = T(5, 5, 0) * scale(0.01)`:
        // a linear part of `100 * I` and a translation column of
        // `100 * (-5, -5, 0)`.
        let ibm = Mat4::from_scale_rotation_translation(
            Vec3::splat(100.0),
            Quat::IDENTITY,
            Vec3::new(-500.0, -500.0, 0.0),
        );
        rig_document(&scaled_ancestor_rig(), &[2], 0, ibm)
    }

    #[test]
    fn the_observed_factor_is_the_scaled_roots_composed_scale_not_its_local_one() {
        // DESIGN.md Appendix D §D.1 classifies the affine domain from each
        // node's *rest-world* linear part, and §D.6's observed factor is that
        // measurement at the scaled root. Every other fixture in this module
        // puts the scaled root directly under an unscaled parent, where the
        // local and composed scales coincide and an implementation that read
        // `Bone::rest.scale` — or the raw projection's local `Trs` scale —
        // is indistinguishable from one that composed the world matrix. Here
        // they differ by exactly the factor two the boundary parent carries.
        let doc = scaled_ancestor_document();
        let capability = complete_capability();
        let plan = plan_scale(&ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 1,
                expected_factor: 0.01,
            },
            document: &doc,
            capability: &capability,
        })
        .unwrap();
        // The scaled ancestor is the boundary parent and stays out.
        assert_eq!(plan.affected_nodes(), &[1, 2]);
        assert_eq!(plan.common_factor(), 0.01);
        assert_eq!(plan.observed_factor(), NEAR_UNIT_OBSERVED_FACTOR);
        // The local scale a local-only measurement would have reported, named
        // here so the two are separated by an exact comparison rather than by
        // a tolerance: it is half the composed factor.
        assert_eq!(
            f64::from(doc.skeleton.bones[1].rest.scale.x),
            NEAR_UNIT_OBSERVED_FACTOR / 2.0
        );

        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        // `C_parent = I` at the boundary, `C_1 = scale(1 / 0.01f32)`, and
        // `1.0f32 / 0.01f32` is exactly `100.0`, so the root's local scale
        // becomes `0.005f32 * 100` — which rounds to exactly `0.5`, leaving
        // the composed root scale `2 * 0.5 = 1` with a zero residual.
        assert_eq!(
            candidate.document().skeleton.bones[1].rest.scale,
            Vec3::splat(0.5)
        );
        let proof = prove_scale(&doc, &candidate, &plan).unwrap();
        assert_eq!(proof.unit_scale_residual, 0.0);
        assert_eq!(proof.observed_factor, NEAR_UNIT_OBSERVED_FACTOR);
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
    fn the_equal_axis_band_is_relative_to_the_longer_axis_and_admits_its_own_edge() {
        // `classify_affine` rejects an axis whose length is farther from the
        // three-axis average than `equal_axis * average.max(length)`. Two
        // things about that comparison are untested by every other fixture,
        // because they all sit far from the edge with the long axis on the
        // same side: the base is the `max` and not the `average`, and the
        // comparison is `>` and not `>=`.
        //
        // One fixture pins both. All three axis lengths are exact in binary32
        // and the average is exact in binary64:
        //
        //   lengths = (99998.5, 99998.5, 100000.0)
        //   average = 299997.0 / 3           = 99999.0   (exact)
        //   longest axis deviation           = 100000.0 - 99999.0 = 1.0
        //   1e-5 * max(average, 100000.0)    = 1e-5 * 100000.0 = 1.0 exactly
        //   1e-5 * average                   = 1e-5 * 99999.0  = 0.99999
        //
        // so `1.0 > 1.0` is false and the basis classifies as uniform, while
        // an `average` base (`1.0 > 0.99999`) or a `>=` comparison
        // (`1.0 >= 1.0`) both reject it as `NonUniformScale`. The two short
        // axes are `0.5` from the average against the same `0.99999`, so
        // neither of them decides anything here.
        //
        // `1e-5 * 100000.0` is exactly `1.0` in binary64: `fl(1e-5)` exceeds
        // `1e-5` by `8.18e-22`, so the product exceeds one by `8.18e-17`,
        // inside the `1.11e-16` half-ulp at one.
        let nodes = vec![
            RigNode {
                parent: None,
                source_node_index: 0,
                translation: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                scale: Vec3::new(99_998.5, 99_998.5, 100_000.0),
            },
            rig(Some(0), 1, Vec3::ZERO),
        ];
        let doc = rig_document(&nodes, &[1], 0, Mat4::IDENTITY);
        let capability = complete_capability();
        let plan = plan_scale(&declared_factor_request(&doc, &capability, 99_999.0))
            .expect("an axis exactly on the equal-axis edge is uniform");
        // The declared factor `99_999.0` is exactly the axis-length average
        // `classify_affine` returns, so the plan is accepted with a zero
        // factor residual and nothing downstream of the equal-axis check can
        // be what admitted it. That the average differs from the longest axis
        // at all is what makes the `max` base observable here.
        assert_eq!(plan.common_factor(), 99_999.0);

        // One binary32 ulp of the long axis past the edge. At `100_000.0` that
        // ulp is `2^-7 = 0.0078125`, so the length becomes `100000.0078125`,
        // the average `99999.0026041666...`, and the deviation
        // `1.0052083333...` against a `1.000000078125` band.
        let nodes = vec![
            RigNode {
                parent: None,
                source_node_index: 0,
                translation: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                scale: Vec3::new(99_998.5, 99_998.5, 100_000.0 + 0.007_812_5),
            },
            rig(Some(0), 1, Vec3::ZERO),
        ];
        let doc = rig_document(&nodes, &[1], 0, Mat4::IDENTITY);
        assert!(matches!(
            plan_scale(&declared_factor_request(&doc, &capability, 99_999.0)).unwrap_err(),
            ScaleError::InvalidAffineDomain {
                reason: AffineDomainViolation::NonUniformScale,
                ..
            }
        ));
    }

    #[test]
    fn an_orthogonality_dot_exactly_on_its_tolerance_is_accepted_and_the_next_one_up_is_not() {
        // `classify_affine` rejects shear on `dot.abs() > relative_orthogonality
        // * average * average`. Distinguishing that `>` from `>=` needs a
        // binary32 basis whose binary64 column dot lands *exactly* on the
        // tolerance. This is that basis. It was found by search, but the
        // arithmetic below is what makes it checkable without rerunning one:
        // every literal is an exact dyadic written as `mantissa * 2^-k`, so
        // the binary32 bits are readable straight off the page.
        //
        //   col0 = (1,  y0, 0)     y0 = 13_826_165 * 2^-69
        //   col1 = (x1, 1,  0)     x1 = 10_995_118 * 2^-40
        //   col2 = (0,  0,  c)     c  =  8_388_610 * 2^-23
        //
        // Two columns lie in the xy-plane and the third is the z-axis, so
        // `dot02` and `dot12` are exactly zero and only `dot01` decides.
        //
        //   dot01 = 1*x1 + y0*1 + 0*0 = x1 + y0
        //
        // and that sum is *exact* in binary64: `x1` is a multiple of `2^-40`,
        // `y0` a multiple of `2^-69`, and the sum is just under `2^-16`, so it
        // needs 53 bits and gets them. `y0` is small enough that `y0 * y0`
        // vanishes against one, so `col0`'s length is exactly `1.0` and moving
        // `y0` does not move the tolerance at all — it moves only `dot01`, and
        // one binary32 step of `y0` is `2^-69`, which is exactly one binary64
        // ulp of `dot01` in its `[2^-17, 2^-16)` binade. The three cases below
        // are therefore three consecutive representable dot products against
        // one fixed tolerance.
        //
        // Accepting the middle one and rejecting the next proves the middle
        // one *is* the tolerance rather than merely below it: acceptance gives
        // `dot <= tolerance`, rejection of `dot + 1ulp` gives
        // `tolerance < dot + 1ulp`, and the only binary64 in `[dot, dot+1ulp)`
        // is `dot` itself. So `>=` would reject the middle case, and does.
        let c = 8_388_610.0_f32 * 2.0_f32.powi(-23);
        let x1 = 10_995_118.0_f32 * 2.0_f32.powi(-40);
        let y0 = 13_826_165.0_f32 * 2.0_f32.powi(-69);
        let y0_up = 13_826_166.0_f32 * 2.0_f32.powi(-69);
        let y0_down = 13_826_164.0_f32 * 2.0_f32.powi(-69);
        // The fixture's own claim, checked before it is relied on: the three
        // dot products are consecutive binary64 values.
        let dot = f64::from(x1) + f64::from(y0);
        assert_eq!(
            (f64::from(x1) + f64::from(y0_up)).to_bits(),
            dot.to_bits() + 1
        );
        assert_eq!(
            (f64::from(x1) + f64::from(y0_down)).to_bits(),
            dot.to_bits() - 1
        );

        let basis = |y: f32| {
            Mat3::from_cols(
                Vec3::new(1.0, y, 0.0),
                Vec3::new(x1, 1.0, 0.0),
                Vec3::new(0.0, 0.0, c),
            )
        };
        let tol = ScaleTolerancePolicy::APPENDIX_D_V2;
        assert!(classify_affine(basis(y0_down), &tol).is_ok());
        assert!(classify_affine(basis(y0), &tol).is_ok());
        assert_eq!(
            classify_affine(basis(y0_up), &tol),
            Err(AffineDomainViolation::Sheared)
        );
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
        // floored comparison base, and the two fixtures below are the two
        // *adjacent* binary32 values that straddle the band edge, so the
        // boundary itself is pinned rather than merely bracketed:
        //
        //   accept: 0.010_000_099_f32 == 0.010000099427998065948486328125
        //           |s - 0.01|            = 9.9427998065948e-8
        //           1e-5 * max(s, 0.01)   = 1.0000099428e-7  -> inside
        //   reject: 0.010_000_1_f32   == 0.01000010035932064056396484375
        //           |s - 0.01|            = 1.0035932064056e-7
        //           1e-5 * max(s, 0.01)   = 1.0000100359e-7  -> outside
        //
        // The two literals differ by exactly one binary32 ulp at this
        // magnitude (`2^-30 = 9.31322574615478515625e-10`), so no
        // representable factor sits between them.
        for (scale, should_accept) in [(0.010_000_099_f32, true), (0.010_000_1_f32, false)] {
            let doc = noisy_factor_document(scale);
            let capability = complete_capability();
            let request = noisy_factor_request(&doc, &capability);
            let planned = plan_scale(&request);
            assert_eq!(
                planned.is_ok(),
                should_accept,
                "scale {scale} accepted={should_accept}"
            );
            if !should_accept {
                assert!(matches!(
                    planned.unwrap_err(),
                    ScaleError::FactorMismatch { .. }
                ));
            }
        }
    }

    /// A request whose declared factor is a free `f64`, so a fixture can put
    /// the observed and declared magnitudes either way round at the band edge
    /// — the `f32` root scale alone cannot, because one binary32 ulp is far
    /// wider than the window the two comparison bases disagree over.
    fn declared_factor_request<'a>(
        document: &'a Document,
        capability: &'a ScaleCapabilityFacts,
        expected_factor: f64,
    ) -> ScaleRequest<'a> {
        ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor,
            },
            document,
            capability,
        }
    }

    #[test]
    fn the_common_factor_band_is_relative_to_the_larger_operand_whichever_it_is() {
        // DESIGN.md Appendix D §D.1 states the band as
        // `abs(a - b) <= c * max(abs(a), abs(b))` — the base is the `max`, not
        // whichever operand `relative` happens to receive first. The existing
        // boundary fixtures all pass the *larger* operand first (an observed
        // factor slightly above the declared one), so a base of `abs(a)`
        // agrees with `max` on every one of them and the `max` is untested.
        //
        // Both rows below sit exactly on the band edge, in the two operand
        // orders. `relative` is called as `relative(c, observed, declared)`,
        // and the observed common factor of a root whose rest scale is a
        // uniform `s` is `s` itself — `classify_affine` averages three column
        // lengths that are all exactly `s`. Every value is exact in binary64:
        //
        //   1e-5 * 100000.0 == 1.0            (exactly; the f64 product of
        //                                      fl(1e-5) and 1e5 rounds to 1)
        //   abs(99999.0 - 100000.0) == 1.0    (exactly)
        //
        //   smaller first: a = 99999,  b = 100000
        //                  max base  -> 1.0 <= 1e-5 * 100000 = 1.0   accept
        //                  abs(a) base -> 1.0 <= 1e-5 * 99999 = 0.99999
        //                                                            reject
        //   larger first:  a = 100000, b = 99999
        //                  max base  -> 1.0 <= 1e-5 * 100000 = 1.0   accept
        //
        // The second row is the `max` in its already-covered order, but both
        // rows make the *inclusive* `<=` load-bearing: the residual is exactly
        // the bound, so a strict `<` rejects both of them.
        let capability = complete_capability();
        for (observed, declared) in [(99_999.0f32, 100_000.0f64), (100_000.0f32, 99_999.0f64)] {
            let doc = noisy_factor_document(observed);
            let request = declared_factor_request(&doc, &capability, declared);
            let plan = plan_scale(&request).unwrap_or_else(|error| {
                panic!("observed {observed} declared {declared} must plan, got {error:?}")
            });
            // The plan carries the *declared* factor, so this also records
            // that the two really did differ by the full band width rather
            // than the fixture having collapsed them onto one value.
            assert_eq!(plan.common_factor(), declared);
            assert_eq!((f64::from(observed) - declared).abs(), 1.0);
        }

        // One `f64` ulp of the declared factor past the edge, in the
        // smaller-first order, so the row above is a boundary and not merely
        // a wide band. One ulp at `100_000.0` is `2^-36 =
        // 1.4551915228366852e-11`, so the residual becomes `1 + 2^-36` while
        // the base grows by only `1e-5 * 2^-36 = 1.455e-16`, which rounds the
        // tolerance up to `1 + 2^-52`:
        //
        //   1 + 2^-36 = 1.0000000000145519  >  1 + 2^-52 = 1.0000000000000002
        let doc = noisy_factor_document(99_999.0);
        let outside =
            declared_factor_request(&doc, &capability, f64::from_bits(100_000f64.to_bits() + 1));
        assert!(matches!(
            plan_scale(&outside).unwrap_err(),
            ScaleError::FactorMismatch { .. }
        ));
    }

    #[test]
    fn a_noisy_factor_plan_scale_accepts_still_satisfies_its_own_proof_postcondition() {
        // The accept side of the band above is only meaningful if the plan
        // it produces is actually buildable and provable. With the earlier
        // `1.0`-floored comparison base this was false: `plan_scale`
        // accepted a factor `8e-6` relative off, whose candidate then failed
        // its own `UnitScale` postcondition at `1.386e-3` against `1e-5`.
        //
        // Arithmetic for the value below, all exact in binary32:
        //
        //   s              = 0.010_000_02_f32 = 0.0100000202655792236328125
        //   1.0f32/0.01f32 = 100.0 exactly, so the rebased root local scale
        //                    is fl(s * 100) = 1.00000202655792236328125
        //                                   = 1 + 17 * 2^-23
        //   per-axis (L-infinity) unit-scale residual
        //                  = 17 * 2^-23 = 2.02655792236328125e-6
        //
        // An L2 norm over the three axes would report `sqrt(3)` times that
        // (`3.51e-6`), which is why the equality below is exact rather than a
        // one-sided bound: it pins the *norm*, not just the magnitude.
        let doc = noisy_factor_document(0.010_000_02);
        let capability = complete_capability();
        let request = noisy_factor_request(&doc, &capability);
        let plan = plan_scale(&request).unwrap();
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let proof = prove_scale(&doc, &candidate, &plan).unwrap();
        assert_eq!(
            proof.unit_scale_residual,
            17.0 * 2f64.powi(-23),
            "unit scale residual {}",
            proof.unit_scale_residual
        );
    }

    // --- Declared vs observed factor (DESIGN.md Appendix D §D.6) ----------

    /// `0.010_000_02_f32`, exactly, as a `f64`.
    ///
    /// The rest-world linear part of `noisy_factor_document(0.010_000_02)`'s
    /// root is `scale(s) * I`, whose three column lengths are each exactly
    /// `s`: `s` needs 24 significant bits, so `s * s` needs at most 48 and is
    /// exact in binary64, and `sqrt` of an exact square is exact. Their mean
    /// is `s` again — `3 * s` needs at most 26 bits and is exact, and its
    /// quotient by three is the representable `s` — so the observed factor is
    /// this literal and not a rounded neighbour of it.
    ///
    /// Spelled as the shortest decimal that round-trips to that binary64
    /// value, because `clippy::excessive_precision` rejects the full exact
    /// expansion. The exact value is `0.0100000202655792236328125`.
    const NOISY_OBSERVED_FACTOR: f64 = 0.010_000_020_265_579_224;

    /// `0.01_f32`, exactly, as a `f64` — by the same argument as
    /// [`NOISY_OBSERVED_FACTOR`], and spelled the same way. The exact value
    /// is `0.00999999977648258209228515625`.
    const NEAR_UNIT_OBSERVED_FACTOR: f64 = 0.009_999_999_776_482_582;

    #[test]
    fn a_rest_bind_plan_and_its_proof_both_report_the_observed_factor_and_the_declared_one() {
        // DESIGN.md Appendix D §D.6 requires producer evidence to record "the
        // operation kind, declared **and observed** factors". Planning
        // measures the observed factor to validate the declared one against
        // it, so the two are distinct numbers and both are reachable.
        //
        // The fixture's root rest scale is inside the `1e-5` common-factor
        // band but nowhere near equal to the declared `0.01`, so an
        // implementation that reported the declared factor under either name
        // is separated from this one by an exact comparison:
        //
        //   s                   = 0.0100000202655792236328125
        //   abs(s - 0.01)       = 2.02655792236328125e-8
        //   1e-5 * max(s, 0.01) = 1.0000020265579e-7   -> inside the band
        let doc = noisy_factor_document(0.010_000_02);
        let capability = complete_capability();
        let plan = plan_scale(&noisy_factor_request(&doc, &capability)).unwrap();
        assert_eq!(plan.common_factor(), 0.01);
        assert_eq!(plan.observed_factor(), NOISY_OBSERVED_FACTOR);

        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let proof = prove_scale(&doc, &candidate, &plan).unwrap();
        assert_eq!(proof.observed_factor, NOISY_OBSERVED_FACTOR);
    }

    #[test]
    fn the_proof_re_derives_the_observed_factor_from_its_own_source_not_from_the_plan() {
        // The point of §D.6's "observed" column is that proof evidence stands
        // on its own: `prove_scale` must measure the factor from the document
        // it was handed rather than echo what planning recorded. Nothing
        // requires that document to be the one the plan came from — the
        // module says so outright — so planning one source and proving
        // another separates a re-derivation from a copy, and nothing else
        // does: for any single document the two numbers agree.
        //
        // Both roots sit inside the band around the declared `0.01`, so both
        // plan and both prove:
        //
        //   planned root 0.010_000_02_f32 = 0.0100000202655792236328125
        //   proved  root 0.01_f32         = 0.00999999977648258209228515625
        //
        // The proved candidate's root composed scale is exactly one: the
        // build's rebase multiplier is `1.0f32 / 0.01f32 == 100.0` exactly,
        // and `fl(0.01f32 * 100)` is `0.999999977648258209228515625` rounded
        // to binary32 — a `2.235e-8` error against a `2^-25 = 2.98e-8`
        // half-ulp below one, so it rounds to `1.0` and the unit-scale
        // postcondition is met with a zero residual.
        let planned = noisy_factor_document(0.010_000_02);
        let proved = noisy_factor_document(0.01);
        let capability = complete_capability();
        let plan = plan_scale(&noisy_factor_request(&planned, &capability)).unwrap();
        assert_eq!(plan.observed_factor(), NOISY_OBSERVED_FACTOR);

        let candidate = build_scale_candidate(&proved, &plan).unwrap();
        let proof = prove_scale(&proved, &candidate, &plan).unwrap();
        assert_eq!(proof.observed_factor, NEAR_UNIT_OBSERVED_FACTOR);
        assert_eq!(proof.unit_scale_residual, 0.0);
    }

    // --- Reconciling the two observed factors (issue #306) ----------------

    #[test]
    fn the_evidence_record_carries_both_observed_factors_and_the_divergence_between_them() {
        // §D.6's evidence record carries two numbers that both answer to "the
        // observed factor", measured from genuinely different state. The
        // record therefore also carries how far apart they are, so a consumer
        // neither mistakes one for the other nor has to derive the
        // relationship itself.
        let capability = complete_capability();

        // A document whose two chains agree — the ordinary case — puts both
        // witnesses on the same number, and the divergence is a *measured*
        // zero rather than an unset field.
        let consistent = noisy_factor_document(0.010_000_02);
        let plan = plan_scale(&noisy_factor_request(&consistent, &capability)).unwrap();
        let candidate = build_scale_candidate(&consistent, &plan).unwrap();
        let proof = prove_scale(&consistent, &candidate, &plan).unwrap();
        assert_eq!(proof.planned_observed_factor, NOISY_OBSERVED_FACTOR);
        assert_eq!(proof.observed_factor, NOISY_OBSERVED_FACTOR);
        assert_eq!(proof.observed_factor_divergence, 0.0);

        // Planning one source and proving another separates the two
        // witnesses, exactly as `the_proof_re_derives_the_observed_factor
        // _from_its_own_source_not_from_the_plan` does — and this proof
        // *succeeds*, so the divergence below is a measurement taken from a
        // complete evidence record rather than a number read off a failure.
        //
        //   planned 0.010_000_02_f32 = 10_737_440 * 2^-30
        //   proved  0.01_f32         = 10_737_418 * 2^-30
        //   difference               =         22 * 2^-30 (exact: equal
        //                                      exponents, and the result is
        //                                      representable)
        //   divergence = 22 / 10_737_440 = 11 / 5_368_720
        //              = 2.0489055119283555e-6
        let proved = noisy_factor_document(0.01);
        let candidate = build_scale_candidate(&proved, &plan).unwrap();
        let proof = prove_scale(&proved, &candidate, &plan).unwrap();
        assert_eq!(proof.planned_observed_factor, NOISY_OBSERVED_FACTOR);
        assert_eq!(proof.observed_factor, NEAR_UNIT_OBSERVED_FACTOR);
        assert_eq!(proof.observed_factor_divergence, 11.0 / 5_368_720.0);
        // Nowhere near the ceiling: this pair separates the two witnesses,
        // it does not saturate their bands. The fixture below does that.
        assert!(
            proof.observed_factor_divergence
                < plan.tolerance_policy().observed_factor_divergence_ceiling()
        );

        // The same two documents with their roles swapped, so the *proved*
        // witness is now the larger of the pair. The comparison base is the
        // `max` of the two, exactly as `ScaleTolerancePolicy::relative`'s is
        // and for the same reason, so the divergence is the same number in
        // both directions — which a base of "whichever operand came first"
        // would not be. Without this direction that `max` is untested: every
        // other fixture here observes the planned witness as the larger one.
        let swapped_plan = plan_scale(&noisy_factor_request(&proved, &capability)).unwrap();
        let swapped_candidate = build_scale_candidate(&consistent, &swapped_plan).unwrap();
        let swapped = prove_scale(&consistent, &swapped_candidate, &swapped_plan).unwrap();
        assert_eq!(swapped.planned_observed_factor, NEAR_UNIT_OBSERVED_FACTOR);
        assert_eq!(swapped.observed_factor, NOISY_OBSERVED_FACTOR);
        assert_eq!(swapped.observed_factor_divergence, 11.0 / 5_368_720.0);
    }

    #[test]
    fn the_divergence_ceiling_is_the_sum_of_the_two_bands_that_produce_it() {
        // Stated from the two bands rather than read back off the policy, so
        // a ceiling derived from any other pair of fields — the equal-axis
        // band, the scalar relative term, a multiple of either — fails here.
        // The sum is exact: `2^-14` is a power of two well above `fl(1e-5)`'s
        // last bit, and the rounded sum is the same binary64 value the
        // decimal literal denotes.
        let policy = ScaleTolerancePolicy::APPENDIX_D_V2;
        assert_eq!(policy.common_factor, 1e-5);
        assert_eq!(policy.postcondition_unit_scale_residual, 2f64.powi(-14));
        assert_eq!(
            policy.observed_factor_divergence_ceiling(),
            1e-5 + 2f64.powi(-14)
        );
        assert_eq!(
            policy.observed_factor_divergence_ceiling(),
            7.103_515_625e-5
        );
    }

    #[test]
    fn a_pair_that_nearly_spends_both_bands_leaves_barely_any_ceiling_headroom() {
        // Named for what it pins and no wider: *this* pair's divergence and
        // *this* pair's headroom. The general form — "saturating both bands
        // keeps a divergence inside their sum" — is false, and
        // `a_document_whose_skeleton_and_projection_disagree_is_proved_not
        // _refused` below is the counterexample: the rebase's binary32
        // rounding can put a divergence over the sum with both witnesses
        // still honouring their own band. What the two tests together say is
        // that the ceiling is tight in the direction this one measures and
        // not a proved bound in the direction that one does.
        //
        // The ceiling claim of §D.6, exercised at the far end of both bands
        // it is the sum of, on a pair of documents that *proves*:
        //
        //   planned root 0.010_000_099_f32 = 10_737_525 * 2^-30
        //     |s - 0.01| = 9.9427998e-8 against 1e-5 * s = 1.00000994e-7,
        //     so planning's band is 99.4% used;
        //   proved  root 0.009_999_4_f32   = 10_736_774 * 2^-30
        //     the rebase multiplier `1.0f32 / 0.01f32` is 100.0 exactly, and
        //     fl32(10_736_774 * 2^-30 * 100) = 1 - 1007 * 2^-24, so the
        //     unit-scale residual is 1007 * 2^-24 = 6.002187728881836e-5
        //     against the 512 * 2^-23 bound, 98.3% used.
        //
        //   divergence = (10_737_525 - 10_736_774) / 10_737_525
        //              = 751 / 10_737_525 = 6.994162993799782e-5
        //   ceiling    = 1e-5 + 2^-14     = 7.103515625e-5
        //   headroom   =                    1.0935263120021840e-6
        let planned = noisy_factor_document(0.010_000_099);
        let proved = noisy_factor_document(0.009_999_4);
        let capability = complete_capability();
        let plan = plan_scale(&noisy_factor_request(&planned, &capability)).unwrap();
        let candidate = build_scale_candidate(&proved, &plan).unwrap();
        let proof = prove_scale(&proved, &candidate, &plan).unwrap();

        assert_eq!(proof.unit_scale_residual, 1007.0 * 2f64.powi(-24));
        assert_eq!(proof.observed_factor_divergence, 751.0 / 10_737_525.0);
        let ceiling = plan.tolerance_policy().observed_factor_divergence_ceiling();
        assert!(
            proof.observed_factor_divergence < ceiling,
            "divergence {} exceeds ceiling {ceiling}",
            proof.observed_factor_divergence
        );
        // Both bands are nearly spent, so the margin left is small — which is
        // what makes this a test of the ceiling rather than of a fixture that
        // never approached it.
        assert!(ceiling - proof.observed_factor_divergence < 1.1e-6);
    }

    #[test]
    fn a_document_whose_skeleton_and_projection_disagree_is_proved_not_refused() {
        // Issue #306's own construction: one document whose normalized
        // skeleton and raw source projection disagree about the scaled root's
        // rest scale. Every other divergence fixture here separates the two
        // witnesses by planning document A and proving document B, which is a
        // property of the pair rather than of a document; this skews
        // `skeleton.bones[0].rest.scale` alone and leaves the projection
        // untouched, so the disagreement is inside the single document both
        // witnesses are read from.
        //
        //   projection root `0.009_999_9_f32`   = 10_737_311 * 2^-30
        //     planning's witness, measured through `parent_source_node_index`.
        //     |s - 0.01| = 9.987503290197208e-8 against the band
        //     1e-5 * max(s, 0.01) = 1e-7, so 99.9% of it is used and planning
        //     accepts.
        //   skeleton root   `0.010_000_611_f32` = 10_738_074 * 2^-30
        //     proof's witness, measured through `world_rest_matrices`, and
        //     also what `build_scale_candidate` rebases. The multiplier
        //     `1.0f32 / 0.01f32` is `100.0` exactly (as in the fixture above),
        //     and fl32(10_738_074 * 2^-30 * 100) = 1 + 512 * 2^-23, so the
        //     unit-scale residual is exactly `2^-14` — the bound itself,
        //     admitted because every policy quantity is an inclusive "at
        //     most".
        //
        //   divergence = (10_738_074 - 10_737_311) / 10_738_074
        //              = 763 / 10_738_074 = 7.105557290813977e-5
        //   ceiling    = 1e-5 + 2^-14     = 7.103515625e-5
        //
        // So the divergence *exceeds* the ceiling while each witness honours
        // its own band — planning's with room to spare, the postcondition's
        // to the last ulp — because the rebase rounds to binary32 on the way
        // and the pre-rounding ratio, 65_576/1_073_741_824 = 6.10723e-5, is
        // itself already past `2^-14`. This is the counterexample that
        // justifies the chosen behaviour: the divergence is *recorded* and the
        // proof succeeds. Refusing at the ceiling would refuse this document.
        let mut doc = noisy_factor_document(0.009_999_9);
        doc.skeleton.bones[0].rest.scale = Vec3::splat(0.010_000_611);
        let capability = complete_capability();
        let plan = plan_scale(&noisy_factor_request(&doc, &capability)).unwrap();
        assert_eq!(plan.observed_factor(), 0.009_999_900_124_967_098);

        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let proof = prove_scale(&doc, &candidate, &plan).unwrap();
        assert_eq!(proof.observed_factor, 0.010_000_610_724_091_53);
        assert_eq!(proof.unit_scale_residual, 2f64.powi(-14));
        assert_eq!(proof.observed_factor_divergence, 763.0 / 10_738_074.0);

        let ceiling = plan.tolerance_policy().observed_factor_divergence_ceiling();
        assert!(
            proof.observed_factor_divergence > ceiling,
            "divergence {} did not exceed ceiling {ceiling}",
            proof.observed_factor_divergence
        );
    }

    #[test]
    fn a_whole_document_conversion_reports_one_factor_under_both_names() {
        // That operation declares its factor rather than observing one
        // (§D.1: nothing may infer it from the document), so both witnesses
        // are the declared factor and the divergence is structurally zero.
        // A consumer reading the record does not have to special-case the
        // operation to learn that.
        //
        // The zero here is the weakest of the three assertions — an
        // implementation that reported a constant zero divergence would
        // satisfy it, and the two rest/bind fixtures above are what refuse
        // that. What this one pins is the *factors*: `payload_document`'s rig
        // is unit-scaled throughout, so an implementation that measured a
        // whole-document conversion's observed factor from the document
        // instead of declaring it would report `1.0` under one name, `0.01`
        // under the other, and a divergence of `0.99`.
        let doc = payload_document();
        let capability = complete_capability();
        let plan = whole_document_plan(&doc, &capability);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let proof = prove_scale(&doc, &candidate, &plan).unwrap();
        assert_eq!(plan.common_factor(), 0.01);
        assert_eq!(proof.planned_observed_factor, 0.01);
        assert_eq!(proof.observed_factor, 0.01);
        assert_eq!(proof.observed_factor_divergence, 0.0);
    }

    /// The factor the *lowest affected bone* of
    /// [`contradictory_parent_chain_document`] carries, as distinct from the
    /// factor at its scaled root.
    ///
    /// That node's projected rest world is `0.01_f32 * (1 + 2^-18)` evaluated
    /// in binary32. `0.01_f32` is `10_737_418 * 2^-30` and `1 + 2^-18` is
    /// `262_145 * 2^-18`, so the exact product is `2_814_760_441_610 * 2^-48`,
    /// whose 42 significant bits round to the 24-bit `10_737_459 * 2^-30`
    /// (the exact quotient is `10_737_458.96875`, so it rounds up). Column
    /// lengths and their mean are exact on a diagonal basis by the argument
    /// in [`NOISY_OBSERVED_FACTOR`].
    ///
    /// Exactly `0.010000037960708141326904296875`; spelled, like its
    /// neighbours, as the shortest decimal that round-trips.
    const CONTRADICTORY_CHILD_OBSERVED_FACTOR: f64 = 0.010_000_037_960_708_141;

    /// A document whose two parent hierarchies contradict each other.
    ///
    /// `Skeleton::parent` says bone 0 parents bone 1. The source-node
    /// projection's `parent_source_node_index` says node 1 (bone 1) parents
    /// node 0 (bone 0). Nothing in this module reconciles them: the affected
    /// closure and every factor planning classifies are walked in *source
    /// node* space, while the "a parent is strictly earlier than its child"
    /// ordering [`crate::model::world_rest_matrices`] enforces is a fact about
    /// `Skeleton::parent` alone, and `validate_source_skeleton_identity`
    /// checks only that the projection's indices are unique. So "the scaled
    /// root" and "the lowest affected bone id" are not the same node here.
    ///
    /// Each hierarchy carries its own local rests, and both projected factors
    /// sit inside the `1e-5` common-factor band around the declared `0.01`, so
    /// the document plans, builds, and proves either way:
    ///
    ///   projection root  node 1 -> bone 1  local `0.01_f32`
    ///     world `0.01_f32`                              = the scaled root
    ///   projection child node 0 -> bone 0  local `1 + 2^-18`
    ///     world `fl32(0.01_f32 * (1 + 2^-18))`          = the lowest bone id
    ///
    /// whose relative separation is `2^-18 = 3.81e-6`, inside the `1e-5` band
    /// the mixed-factor check applies and far outside the exact equality the
    /// two assertions below make.
    ///
    /// The skeleton keeps the ordinary shape — bone 0 the root at `0.01`,
    /// bone 1 its unit-scaled child — so the rest/bind rebase and its
    /// unit-scale postcondition behave exactly as they do everywhere else.
    fn contradictory_parent_chain_document() -> Document {
        let mut doc = rig_document(
            &[
                RigNode {
                    parent: None,
                    source_node_index: 0,
                    translation: Vec3::ZERO,
                    rotation: Quat::IDENTITY,
                    scale: Vec3::splat(0.01),
                },
                rig(Some(0), 1, Vec3::ZERO),
            ],
            &[0],
            0,
            Mat4::IDENTITY,
        );
        let nodes = &mut doc.assets.source_skeleton.nodes;
        nodes[0].parent_source_node_index = Some(1);
        nodes[0].scene_root_indices = Vec::new();
        nodes[0].local_rest = SourceNodeLocalRest::Trs {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::splat(1.0 + 2f32.powi(-18)),
        };
        nodes[1].parent_source_node_index = None;
        nodes[1].scene_root_indices = vec![0];
        nodes[1].local_rest = SourceNodeLocalRest::Trs {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::splat(0.01),
        };
        doc
    }

    #[test]
    fn the_observed_factor_is_read_at_the_scaled_root_not_the_lowest_affected_bone() {
        // §D.6 defines the observed factor *at the scaled root*, and on every
        // other fixture in this module the scaled root also happens to be the
        // lowest bone id in the closure — so reading either one reports the
        // same number and the distinction is invisible.
        //
        // It is not a theorem that they agree. The two facts that look like
        // they compose into one are about different hierarchies: the closure
        // is a source-node-space walk, and the id ordering `world_rests`
        // enforces constrains `Skeleton::parent`. `contradictory_parent_chain_document`
        // is the document where they disagree, and it is an ordinary
        // fully-proving document otherwise.
        let doc = contradictory_parent_chain_document();
        let capability = complete_capability();
        let plan = plan_scale(&ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 1,
                expected_factor: 0.01,
            },
            document: &doc,
            capability: &capability,
        })
        .unwrap();
        // Bone 1 is the scaled root; bone 0 is the lowest affected id.
        assert_eq!(plan.affected_nodes(), &[0, 1]);
        assert_eq!(plan.observed_factor(), NEAR_UNIT_OBSERVED_FACTOR);

        // The other node's factor, read through the public API rather than
        // asserted about: planning the same document *at* bone 0 makes it its
        // own scaled root, and its closure is the joint alone.
        let at_child = plan_scale(&ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 0.01,
            },
            document: &doc,
            capability: &capability,
        })
        .unwrap();
        assert_eq!(at_child.affected_nodes(), &[0]);
        assert_eq!(
            at_child.observed_factor(),
            CONTRADICTORY_CHILD_OBSERVED_FACTOR
        );
        assert_ne!(
            CONTRADICTORY_CHILD_OBSERVED_FACTOR,
            NEAR_UNIT_OBSERVED_FACTOR
        );

        // And the whole document proves, so the separation above is not an
        // artifact of a fixture that planning would have rejected downstream.
        // Proof measures the *normalized skeleton* — bone 0 at `0.01` and its
        // unit-scaled child bone 1 — so its own root world is `0.01_f32` too.
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let proof = prove_scale(&doc, &candidate, &plan).unwrap();
        assert_eq!(proof.observed_factor, NEAR_UNIT_OBSERVED_FACTOR);
        assert_eq!(proof.unit_scale_residual, 0.0);

        // And the two witnesses of §D.6 *agree exactly* here, so the recorded
        // divergence is zero. The projection root is node 1 with local rest
        // `0.01_f32` and no parent, and the skeleton path to bone 1 is
        // `0.01_f32` at bone 0 composed with bone 1's unit rest — the same
        // number by two routes. What the contradiction moves is *which node*
        // the scaled root is, not what is measured there.
        //
        // Documentary rather than adversarial: the substantive claim is that
        // the two witnesses land on the same number by two different routes,
        // and the zero divergence follows. What separates a computed
        // divergence from a constant one is
        // `the_evidence_record_carries_both_observed_factors_and_the
        // _divergence_between_them`.
        assert_eq!(proof.planned_observed_factor, proof.observed_factor);
        assert_eq!(proof.planned_observed_factor, NEAR_UNIT_OBSERVED_FACTOR);
        assert_eq!(proof.observed_factor_divergence, 0.0);

        // That zero belongs to the *(document, scaled root)* pair above, not
        // to the document: the very same fixture, planned at its other node,
        // separates the two witnesses. Planning reads the projection chain and
        // observes bone 0's `fl32(0.01 * (1 + 2^-18))`; proof reads the
        // skeleton, where bone 0 is the root at `0.01_f32` and its own scaled
        // root, and observes `0.01_f32`.
        //
        //   planned 10_737_459 * 2^-30   (`CONTRADICTORY_CHILD_OBSERVED_FACTOR`)
        //   proved  10_737_418 * 2^-30   (`NEAR_UNIT_OBSERVED_FACTOR`)
        //   divergence = 41 / 10_737_459 = 3.8184080609760655e-6
        //
        // The conclusion the zero was cited for still holds, and now for a
        // stated reason rather than by accident: `3.8e-6` is two orders below
        // the `7.1e-5` ceiling, so a bound on the divergence would not detect
        // this chain contradiction at either root. Detecting it needs the
        // chain-agreement check #309 asks for.
        let at_child_candidate = build_scale_candidate(&doc, &at_child).unwrap();
        let at_child_proof = prove_scale(&doc, &at_child_candidate, &at_child).unwrap();
        assert_eq!(
            at_child_proof.planned_observed_factor,
            CONTRADICTORY_CHILD_OBSERVED_FACTOR
        );
        assert_eq!(at_child_proof.observed_factor, NEAR_UNIT_OBSERVED_FACTOR);
        assert_eq!(
            at_child_proof.observed_factor_divergence,
            41.0 / 10_737_459.0
        );
        assert!(
            at_child_proof.observed_factor_divergence
                < plan.tolerance_policy().observed_factor_divergence_ceiling() / 10.0
        );
    }

    /// `noisy_factor_document`'s rig with a per-axis root rest scale.
    ///
    /// Only ever a *proof* source: [`prove_scale`] deliberately does not
    /// re-run [`classify_affine`], so nothing on that path bounds how far
    /// apart the scaled root's three axes are, and the test below asserts
    /// that planning this same document is refused.
    fn per_axis_factor_document(scale: Vec3) -> Document {
        let nodes = vec![
            RigNode {
                parent: None,
                source_node_index: 0,
                translation: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                scale,
            },
            rig(Some(0), 1, Vec3::ZERO),
        ];
        rig_document(&nodes, &[1], 0, Mat4::IDENTITY)
    }

    #[test]
    fn the_observed_factor_is_the_mean_of_the_scaled_roots_axes_not_its_first_one() {
        // `observed_factor_from_source` reports
        // `axis_length_average(axis_lengths(..))` — the same pair of helpers
        // `classify_affine` returns its factor through, which is what makes
        // the two "the same quantity by definition rather than by
        // coincidence". On a basis whose axes are equal within
        // `equal_axis`, though, the mean and any single axis agree to within
        // that band, and every other proof-source fixture here has a
        // *uniform* scaled root — so nothing separates the mean from, say,
        // its first column.
        //
        // Something has to. Proof does not re-classify its source, by design,
        // so the axis spread of the document `prove_scale` is handed is
        // bounded by nothing at all: the only downstream constraint is the
        // unit-scale postcondition on the rebased *candidate*, which admits
        // `postcondition_unit_scale_residual = 2^-14 = 6.10e-5` of it — six
        // times the `1e-5` equal-axis band planning would have applied. The
        // fixture uses half of what the postcondition allows.
        //
        // The fixture puts the root's axes a fixed `328` ulps either side of
        // `0.01_f32`, which is `10_737_418 * 2^-30`:
        //
        //   x  (10_737_418 + 328) * 2^-30      y  (10_737_418 - 328) * 2^-30
        //   z   10_737_418        * 2^-30
        //
        // Each is exact in binary32 (`10_737_746 < 2^24`), so each column
        // length is exact, and their sum telescopes to `3 * 10_737_418 *
        // 2^-30` — 26 bits, exact in binary64 — whose third is `0.01_f32`
        // again. The mean is therefore exactly `NEAR_UNIT_OBSERVED_FACTOR`
        // while no individual axis is, and `328 / 10_737_418 = 3.05e-5` of
        // spread stays inside the postcondition band the rebase must meet.
        let step = 328.0 * 2f32.powi(-30);
        let proved = per_axis_factor_document(Vec3::new(0.01 + step, 0.01 - step, 0.01));
        let root = proved.skeleton.bones[0].rest.scale;
        let ulps = 328.0 * 2f64.powi(-30);
        assert_eq!(f64::from(root.x), NEAR_UNIT_OBSERVED_FACTOR + ulps);
        assert_eq!(f64::from(root.y), NEAR_UNIT_OBSERVED_FACTOR - ulps);
        assert_eq!(f64::from(root.z), NEAR_UNIT_OBSERVED_FACTOR);

        // Planning refuses it — `3.05e-5` of spread is three times the
        // `equal_axis` band — which is exactly why the spread has to arrive
        // through the proof source rather than through a plan.
        let capability = complete_capability();
        assert_eq!(
            plan_scale(&noisy_factor_request(&proved, &capability)).unwrap_err(),
            ScaleError::InvalidAffineDomain {
                node: 0,
                reason: AffineDomainViolation::NonUniformScale,
            }
        );

        let planned = noisy_factor_document(0.01);
        let plan = plan_scale(&noisy_factor_request(&planned, &capability)).unwrap();
        let candidate = build_scale_candidate(&proved, &plan).unwrap();
        let proof = prove_scale(&proved, &candidate, &plan).unwrap();
        assert_eq!(proof.observed_factor, NEAR_UNIT_OBSERVED_FACTOR);

        // The rebase multiplier is the exact `1.0_f32 / 0.01_f32 == 100`, so
        // the candidate's root axes are `fl32(x * 100)` and `fl32(y * 100)`:
        // `1_073_774_600 * 2^-30` rounds to `1 + 2^-15`, and `1_073_709_000 *
        // 2^-30` rounds to `(2^24 - 513) * 2^-24`. The larger deviation from
        // one is the second, `513 * 2^-24`, and the postcondition admits it
        // with room to spare.
        assert_eq!(proof.unit_scale_residual, 513.0 * 2f64.powi(-24));
        assert!(
            proof.unit_scale_residual <= plan.tolerance_policy().postcondition_unit_scale_residual
        );
    }

    #[test]
    fn a_whole_document_plan_reports_its_declared_factor_as_the_observed_one() {
        // §D.1: a whole-document conversion's factor is declared, never
        // inferred from the document. Two documents authored in different
        // linear units are numerically identical, so there is nothing to
        // measure and the declared factor is the whole of the evidence.
        //
        // The fixture makes that a real assertion rather than a tautology:
        // `unit_rig`'s root has rest scale one, so an implementation that
        // measured the rest-world factor here — as the rest/bind operation
        // does — would report `1.0`, not the declared `0.01`.
        let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        let capability = complete_capability();
        let plan = plan_scale(&ScaleRequest {
            operation: ScaleOperation::WholeDocumentLinearUnits { factor: 0.01 },
            document: &doc,
            capability: &capability,
        })
        .unwrap();
        assert_eq!(plan.common_factor(), 0.01);
        assert_eq!(plan.observed_factor(), 0.01);

        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let proof = prove_scale(&doc, &candidate, &plan).unwrap();
        assert_eq!(proof.observed_factor, 0.01);
    }

    #[test]
    fn a_rest_bind_proof_whose_source_cannot_locate_the_scaled_root_reports_missing_evidence() {
        // The observed factor is re-derived through the proof source's own
        // source-node projection. A source that does not carry one for the
        // plan's scaled root cannot be measured, and reporting `0.0` — or the
        // plan's recorded value — would be evidence for a measurement that
        // never happened.
        let doc = compensated_document();
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

        let mut unprojected = doc.clone();
        unprojected.assets.source_skeleton.nodes[0].bone = None;
        assert_eq!(
            prove_scale(&unprojected, &candidate, &plan).unwrap_err(),
            ScaleError::MissingProofEvidence {
                kind: ProofResidualKind::ObservedFactor,
                detail: "scaled_root_not_projected",
            }
        );
    }

    #[test]
    fn plan_scale_accepting_a_common_factor_implies_its_candidate_proves() {
        // The closure property DESIGN.md Appendix D §D.1 now states outright:
        // a factor `plan_scale` accepts always yields a candidate that
        // satisfies the unit-scale postcondition. Before the v2 policy the
        // implication was false over a whole band, because the input check
        // was a scalar relative comparison and the postcondition was an L2
        // norm over three axes: every relative error `e` in `(5.77e-6, 1e-5]`
        // was accepted and then rejected at `sqrt(3) * e > 1e-5`.
        //
        // Each fixture is a binary32 value chosen so `fl(s * 100)` lands
        // exactly on a mantissa grid point `1 + n * 2^-23`, which is exactly
        // what the rebased root's composed scale becomes (`1.0f32 / 0.01f32`
        // is `100.0` exactly). The expected residual is therefore the literal
        // `n * 2^-23`, hand-computable and independent of this module:
        //
        //   s                   exact value of s              n   n * 2^-23
        //   0.010_000_02        0.0100000202655792236328125   17  2.02655792236328125e-6
        //   0.010_000_061       0.0100000612437725067138671875 51 6.07967376708984375e-6
        //   0.010_000_08        0.0100000798702239990234375   67  7.98702239990234375e-6
        //   0.010_000_099       0.010000099427998065948486328125
        //                                                     83  9.89437103271484375e-6
        //
        // The last three all sit in the previously-broken band: under the v1
        // L2 residual they measured 1.053e-5, 1.383e-5 and 1.714e-5 against a
        // `1e-5` bound and failed proof outright.
        let capability = complete_capability();
        for (scale, expected_residual) in [
            (0.010_000_02_f32, 17.0 * 2f64.powi(-23)),
            (0.010_000_061_f32, 51.0 * 2f64.powi(-23)),
            (0.010_000_08_f32, 67.0 * 2f64.powi(-23)),
            (0.010_000_099_f32, 83.0 * 2f64.powi(-23)),
        ] {
            let doc = noisy_factor_document(scale);
            let request = noisy_factor_request(&doc, &capability);
            let plan = plan_scale(&request)
                .unwrap_or_else(|error| panic!("scale {scale} must plan, got {error:?}"));
            let candidate = build_scale_candidate(&doc, &plan)
                .unwrap_or_else(|error| panic!("scale {scale} must build, got {error:?}"));
            let proof = prove_scale(&doc, &candidate, &plan)
                .unwrap_or_else(|error| panic!("scale {scale} must prove, got {error:?}"));
            assert_eq!(
                proof.unit_scale_residual, expected_residual,
                "scale {scale} unit-scale residual"
            );
            assert!(
                proof.unit_scale_residual
                    <= plan.tolerance_policy().postcondition_unit_scale_residual,
                "scale {scale} residual {} exceeds the declared bound",
                proof.unit_scale_residual
            );
        }
    }

    #[test]
    fn the_common_factor_band_stays_relative_below_the_scalar_absolute_floor() {
        // The band's comparison base carries no floor at all. An earlier
        // revision floored it at `scalar_absolute = 1e-6`, which is the same
        // defect as the `1.0` floor above, one thousandth the size: below
        // `1e-6` the band stopped tracking its operands and froze at the
        // constant `1e-5 * 1e-6 = 1e-11`, a *relative* band of `1e-11 / s`
        // that widens without limit as `s` shrinks and crosses the `2^-14`
        // postcondition bound at `s = 1e-11 / 2^-14 = 1e-11 * 16384 =
        // 1.6384e-7`. Every declared factor below that had a band of plans
        // `plan_scale` accepted and `prove_scale` then refused — the closure
        // property §D.1 states as a theorem, false over a whole regime.
        // (Measured with the floor restored: the largest declared factor that
        // still breaks closure is `1.629e-7` and the smallest clean one is
        // `1.710e-7`, bracketing `1.6384e-7`.)
        //
        // `2^-23 = 1.1920928955078125e-7` is well inside that regime and is
        // exactly representable, as is its reciprocal `2^23`, so every step
        // below is exact in binary32:
        //
        //   s          = 2^-23 * (1 + n * 2^-23)
        //   candidate root local scale
        //              = s * (1 / 2^-23) = 1 + n * 2^-23
        //   residual   = n * 2^-23
        //
        // and the band accepts exactly while `n * 2^-23 <= 1e-5 * (1 + n *
        // 2^-23)`, i.e. `n <= 83`:
        //
        //   n = 83: 9.89437103271484375e-6 <= 1.0000098944e-5  -> accept
        //   n = 84: 1.001358032226562e-5   >  1.0000100136e-5  -> reject
        //
        // That is the *same* boundary the `0.01` fixtures above hit, six
        // orders of magnitude away, which is what "relative" means. Under the
        // `1e-6` floor the band here admitted every `n` up to 703, and
        // `n = 600` — the third row below — planned, built, and then failed
        // its own postcondition at `600 * 2^-23 = 7.152557373046875e-5`.
        let declared = 2f64.powi(-23);
        let unit = 2f32.powi(-23);
        let capability = complete_capability();
        for n in [83u32, 84, 600] {
            let doc = noisy_factor_document(unit * (1.0 + n as f32 * unit));
            let request = ScaleRequest {
                operation: ScaleOperation::RestBindUniformScale {
                    source_skin_index: 0,
                    source_root_node_index: 0,
                    expected_factor: declared,
                },
                document: &doc,
                capability: &capability,
            };
            if n > 83 {
                assert!(
                    matches!(plan_scale(&request), Err(ScaleError::FactorMismatch { .. })),
                    "n = {n} must not be accepted"
                );
                continue;
            }
            let plan = plan_scale(&request)
                .unwrap_or_else(|error| panic!("n = {n} must plan, got {error:?}"));
            let candidate = build_scale_candidate(&doc, &plan)
                .unwrap_or_else(|error| panic!("n = {n} must build, got {error:?}"));
            let proof = prove_scale(&doc, &candidate, &plan)
                .unwrap_or_else(|error| panic!("n = {n} must prove, got {error:?}"));
            assert_eq!(proof.unit_scale_residual, f64::from(n) * 2f64.powi(-23));
        }
    }

    #[test]
    fn a_plan_loading_all_three_analytic_bands_still_proves() {
        // Every other closure fixture is a single-node rig, which can only
        // load the `FactorMismatch` band. This one loads all three bands the
        // §D.1 derivation composes, at 76.3% of the declared `1e-5` each:
        //
        //   u = 2^-17 = 7.62939453125e-6
        //   root local scale  = 0.5 * (1 + u)      -> s_0 = 0.5 * (1 + u)
        //   child local scale = (1 + u/2, 1 + u/2, 1 + 2u)
        //
        // Every product below is exact in binary32 (the discarded terms are
        // `2^-36` and `2^-34` against a `2^-24` ulp), so the child's world
        // axis lengths are
        //
        //   x = y = 0.5 * (1 + 1.5u)      z = 0.5 * (1 + 3u)
        //   average A = (2x + z) / 3 = 0.5 * (1 + 2u)
        //
        // and the three bands are:
        //
        //   FactorMismatch     |s_0 - 0.5| = 0.5u    vs 1e-5 * 0.5(1 + u)
        //   MixedFactor        |A - s_0|   = 0.5u    vs 1e-5 * 0.5(1 + 2u)
        //   NonUniformScale    |z - A|     = 0.5u    vs 1e-5 * 0.5(1 + 3u)
        //
        // all of which accept, since `u = 7.63e-6 < 1e-5`. The third band is
        // the one the two-band derivation missed: `classify_affine` returns
        // the *average* of the three axis lengths, while the postcondition
        // measures an individual *axis*, and `equal_axis` permits each axis
        // its own further band away from that average.
        //
        // The candidate's root local scale is `0.5 * (1 + u) * 2 = 1 + u`
        // exactly, so its composed axes are `2x`, `2x`, `2z`, and with
        // `u = 64 * 2^-23`:
        //
        //   node 0 residual = u    =  64 * 2^-23
        //   node 1 residual = 3u   = 192 * 2^-23 = 2.288818359375e-5
        //
        // `192 * 2^-23` is above `(1 - c)^-2 - 1 = 2.00003e-5`, so this rig
        // is a *counterexample* to the two-band worst case the policy used to
        // claim — it is not reachable by composing only the declared-factor
        // and mixed-factor bands.
        let u = 2f32.powi(-17);
        let nodes = vec![
            RigNode {
                parent: None,
                source_node_index: 0,
                translation: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                scale: Vec3::splat(0.5 * (1.0 + u)),
            },
            RigNode {
                parent: Some(0),
                source_node_index: 1,
                translation: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                scale: Vec3::new(1.0 + u * 0.5, 1.0 + u * 0.5, 1.0 + u * 2.0),
            },
        ];
        let doc = rig_document(&nodes, &[1], 0, Mat4::IDENTITY);
        let capability = complete_capability();
        let plan = plan_scale(&ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 0.5,
            },
            document: &doc,
            capability: &capability,
        })
        .unwrap();
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let proof = prove_scale(&doc, &candidate, &plan).unwrap();
        assert_eq!(proof.unit_scale_residual, 192.0 * 2f64.powi(-23));
        let policy = plan.tolerance_policy();
        assert!(proof.unit_scale_residual <= policy.postcondition_unit_scale_residual);
        let two_bands = (1.0 - policy.common_factor).powi(-2) - 1.0;
        assert!(
            proof.unit_scale_residual > two_bands,
            "residual {} does not exceed the two-band figure {two_bands}",
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

    #[test]
    fn the_reported_rest_rotation_residual_is_the_maximum_not_the_last_node_seen() {
        // `ScaleProof::rest_rotation_residual` is a *maximum* over the
        // affected nodes — so a proof that reported the last affected node's
        // residual instead of the largest would publish a smaller number than
        // it observed. #284 will freeze this as a published §D.6 evidence
        // field, which is exactly what makes
        // accept/reject-unchanged-but-record-false a defect. Like
        // `unit_scale_residual`, this one is checked against a fixed policy
        // bound rather than a before/after-derived tolerance, so it reaches
        // the shared fold through `record_and_check` rather than
        // `check_and_track`.
        //
        // An earlier revision left this unpinned on the grounds that no build
        // path writes `rest.rotation`, so the residual is always zero. That
        // reasoning is about the *builder*; the obligation exists to catch
        // candidates the builder did not produce, and this file hand-builds
        // `ScaleCandidate { document }` in several places, including the two
        // rotation tests above. The build path has nothing to do with it.
        //
        // The rig puts the only nonzero residual on the *first* of two inert
        // leaves, so the last node folded reports exactly zero:
        //
        //   bone 0  root, no rotation error                  residual 0
        //   bone 1  skinned joint, no rotation error          residual 0
        //   bone 2  inert leaf, rotated                       residual theta
        //   bone 3  inert leaf, left identity                 residual 0
        //
        // A rotation of `theta` about X is `(sin(theta/2), 0, 0,
        // cos(theta/2))`; at `theta = 2^-17` the cosine is within `2^-36` of
        // one and rounds to exactly `1.0f32`, so the authored quaternion is
        // the literal `(2^-18, 0, 0, 1)`. Against an identity source rotation
        // the double-cover-aware chord is `|(2^-18, 0, 0, 0)| = 2^-18`
        // exactly, and the reported angle is
        //
        //   4 * asin(2^-18 / 2) = 4 * asin(2^-19) = 2^-17 = 7.62939453125e-6
        //
        // to within `2^-58` (the cubic term of `asin`), which is far inside
        // the `1e-15` band asserted below and comfortably under the `1e-5`
        // policy bound, so the candidate proves rather than being rejected.
        let nodes = vec![
            rig(None, 0, Vec3::ZERO),
            rig(Some(0), 1, Vec3::new(0.0, 1.0, 0.0)),
            rig(Some(0), 2, Vec3::new(3.0, 0.0, 0.0)),
            rig(Some(0), 3, Vec3::new(0.0, 0.0, 3.0)),
        ];
        let doc = rig_document(&nodes, &[1], 0, Mat4::IDENTITY);
        let capability = complete_capability();
        let plan = whole_document_plan(&doc, &capability);
        // The order the maximum is folded in: bone 3 is last.
        assert_eq!(plan.affected_nodes(), &[0, 1, 2, 3]);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();

        // Bones 2 and 3 are leaves carrying no skin slot, no mesh vertex and
        // no track, so no descendant origin, skin palette or sampled pose
        // moves and the rest-rotation obligation is the only one that can see
        // either of them.
        let mut broken = candidate.document().clone();
        broken.skeleton.bones[2].rest.rotation = Quat::from_xyzw(2f32.powi(-18), 0.0, 0.0, 1.0);
        assert_eq!(broken.skeleton.bones[3].rest.rotation, Quat::IDENTITY);
        let broken = ScaleCandidate { document: broken };

        let proof = prove_scale(&doc, &broken, &plan).unwrap();
        assert!(
            (proof.rest_rotation_residual - 2f64.powi(-17)).abs() < 1e-15,
            "residual {} is not the 2^-17 radian angle bone 2 carries",
            proof.rest_rotation_residual
        );
        // Stated as its own inequality so the assertion above cannot be read
        // as "whatever the last node happened to produce": bone 3's residual,
        // the one a last-node fold would publish, is exactly zero.
        assert!(proof.rest_rotation_residual > 0.0);
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
        //
        // `-inf` is in the list because it is the *only* case that separates
        // the `!observed.is_finite()` the guard is written with from the
        // narrower `observed.is_nan()`: `+inf > tolerance` is true, so the
        // comparison already rejects a positive infinity with or without a
        // guard, and a `NaN`-only guard would still pass every other case
        // here. See `check_residual`'s own note for why the wider spelling is
        // kept even though no caller in this module can hand it a negative
        // residual.
        for (observed, tolerance) in [
            (f64::NAN, 1.0),
            (f64::INFINITY, 1.0),
            (f64::NEG_INFINITY, 1.0),
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

    /// The column pair a single shear term is aimed at.
    #[derive(Clone, Copy, Debug)]
    enum ShearPair {
        /// `dot01` — `columns[0] · columns[1]`.
        XY,
        /// `dot02` — `columns[0] · columns[2]`.
        XZ,
        /// `dot12` — `columns[1] · columns[2]`.
        YZ,
    }

    /// A basis carrying exactly one shear term `s`, in the named column pair.
    ///
    /// Two columns are the unit axes they name; the third adds `s` in one
    /// foreign axis:
    ///
    /// ```text
    ///   XY: (1, 0, 0) (s, 1, 0) (0, 0, 1)   dot01 = s, dot02 = 0, dot12 = 0
    ///   XZ: (1, 0, 0) (0, 1, 0) (s, 0, 1)   dot02 = s, dot01 = 0, dot12 = 0
    ///   YZ: (1, 0, 0) (0, 1, 0) (0, s, 1)   dot12 = s, dot01 = 0, dot02 = 0
    /// ```
    ///
    /// Each zero above is structural — every term of those two dot products
    /// has a literal `0.0` factor — so the pairs a fixture is *not* aiming at
    /// stay exactly zero for every `s`, and only the named comparison can
    /// decide.
    fn single_shear_basis(pair: ShearPair, s: f32) -> Mat3 {
        match pair {
            ShearPair::XY => Mat3::from_cols(
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(s, 1.0, 0.0),
                Vec3::new(0.0, 0.0, 1.0),
            ),
            ShearPair::XZ => Mat3::from_cols(
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(s, 0.0, 1.0),
            ),
            ShearPair::YZ => Mat3::from_cols(
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(0.0, s, 1.0),
            ),
        }
    }

    /// Drive [`single_shear_basis`] through `classify_affine` at both signs of
    /// both magnitudes, asserting `Sheared` outside the band and acceptance
    /// inside it.
    ///
    /// The shape is chosen so that *no other check in `classify_affine` can
    /// fire*, at either magnitude and either sign. With `s = ±2^-15`:
    ///
    /// * **NonFinite** — every entry is a finite literal.
    /// * **Singular** — the sheared column is `unit_axis + s * other_axis`,
    ///   and `other_axis` is one of the two columns left untouched, so the
    ///   triple product is unchanged from the identity's: `determinant` is
    ///   exactly `1.0` for all three pairs. `axis_product` is
    ///   `sqrt(1 + 2^-30) < 1.000000001`, so the singular threshold is
    ///   `1e-6 * that`, six orders below `1.0`.
    /// * **NonUniformScale** — the length multiset is
    ///   `(1, 1, sqrt(1 + 2^-30))` for all three pairs, i.e.
    ///   `(1, 1, 1.000000000465661…)`. The average is `1.000000000155220…`
    ///   and the largest deviation is `2/3 * 2^-31 = 3.10e-10`, against a
    ///   band of `1e-5 * max(average, length) > 1e-5` — four and a half
    ///   orders of headroom.
    /// * **Reflected** — the determinant is `+1.0`, so even deleting the
    ///   orthogonality check outright yields `Ok`, never `Reflected`.
    ///
    /// That leaves the orthogonality comparison as the only possible source
    /// of a `Sheared` verdict. It fires because
    /// `|s| = 2^-15 = 3.0517578125e-5` and the tolerance is
    /// `1e-5 * average^2 = 1.0000000003…e-5` — the shear is 3.05x the bound.
    /// At `s = ±2^-17 = ±7.62939453125e-6` the same arithmetic gives
    /// `0.76x` the bound (lengths `(1, 1, sqrt(1 + 2^-34))`, tolerance
    /// `1.0000000000…e-5`), so the in-band cases must be accepted: the
    /// rejections above are the shear magnitude talking and not the shape.
    fn assert_only_this_column_pair_decides_shear(pair: ShearPair) {
        let tol = ScaleTolerancePolicy::APPENDIX_D_V2;
        // Written as exact dyadics rather than decimal literals, so the
        // binary32 bits are readable straight off the page: `2^-15` and
        // `2^-17`.
        let out_of_band_magnitude = 2.0_f32.powi(-15);
        let in_band_magnitude = 2.0_f32.powi(-17);
        for sign in [1.0_f32, -1.0] {
            let out_of_band = sign * out_of_band_magnitude;
            assert_eq!(
                classify_affine(single_shear_basis(pair, out_of_band), &tol),
                Err(AffineDomainViolation::Sheared),
                "{pair:?} shear {out_of_band} was not rejected as sheared"
            );
            let in_band = sign * in_band_magnitude;
            assert!(
                classify_affine(single_shear_basis(pair, in_band), &tol).is_ok(),
                "{pair:?} shear {in_band} was rejected, so the shape and not \
                 the magnitude is what the out-of-band case rejects on"
            );
        }
    }

    #[test]
    fn shear_isolated_to_the_x_y_column_pair_is_sheared_at_either_sign() {
        // Pins `dot01`'s own clause *and* its `.abs()`: with the absolute
        // value dropped, `dot01 = -2^-15` is not `> tolerance` and the other
        // two dot products are structurally zero, so the basis is accepted.
        assert_only_this_column_pair_decides_shear(ShearPair::XY);
    }

    #[test]
    fn shear_isolated_to_the_x_z_column_pair_is_sheared_at_either_sign() {
        // Pins `dot02`'s clause and its `.abs()`. No other fixture isolates
        // `x·z`: the two column pairs either side of it stay exactly zero
        // here, so deleting this clause accepts the basis outright.
        assert_only_this_column_pair_decides_shear(ShearPair::XZ);
    }

    #[test]
    fn shear_isolated_to_the_y_z_column_pair_is_sheared_at_either_sign() {
        // Pins `dot12`'s clause and its `.abs()`.
        // `a_shear_only_f64_can_see_is_still_classified_as_sheared` already
        // reaches this comparison, but through a dense basis in which all
        // three dot products are nonzero; this one isolates it, so the three
        // pairs are covered on equal terms.
        assert_only_this_column_pair_decides_shear(ShearPair::YZ);
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
        // The skin equation reads the same instance walk and is gated on the
        // same evidence: with no skinned instance there is no `W_i * B_i` to
        // evaluate either, and a declared `prove_skin` would report a `0.0`
        // skin-matrix residual for a claim nothing checked.
        assert!(!plan.proof_obligations().prove_skin);
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
        // Replay the plan against a document pair that no longer carries the
        // skinned instance its bounds obligation was declared from. Both
        // sides lose it, so what fails is the bounds obligation for want of
        // evidence, not the source/candidate skin-identity parity check.
        let mut unskinned = doc.clone();
        unskinned.assets.instances[0].skin_joints.clear();
        unskinned.assets.instances[0].skin_ibms.clear();
        let mut unskinned_candidate = candidate.into_document();
        unskinned_candidate.assets.instances[0].skin_joints.clear();
        unskinned_candidate.assets.instances[0].skin_ibms.clear();
        let candidate = ScaleCandidate {
            document: unskinned_candidate,
        };
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

    // --- Evidence gating, per obligation (issue #302) ---------------------

    /// `multi_joint_document` with its two affected translation tracks
    /// replaced by one affected *rotation* track.
    ///
    /// That clip still yields sample times — so the trajectory obligation has
    /// evidence — while carrying no translation payload for the key
    /// obligation to compare and no cubic segment to take an interior of.
    /// It is the one document shape that separates `prove_trajectories` from
    /// its two siblings, which is what makes each of the three refusals below
    /// reachable on its own.
    fn rotation_only_clip_document() -> Document {
        let mut doc = multi_joint_document();
        doc.clips[0].tracks = vec![Track {
            bone: 1,
            property: Property::Rotation,
            interpolation: Interpolation::Linear,
            times: vec![0.0, 1.0],
            values: TrackValues::Quats(vec![Quat::IDENTITY, Quat::IDENTITY]),
        }];
        doc
    }

    /// Strip every clip from a source and a candidate together, so the pair
    /// still satisfies `validate_candidate_structure` and what fails is the
    /// missing obligation evidence rather than clip-count parity.
    fn without_clips(source: &Document, candidate: ScaleCandidate) -> (Document, ScaleCandidate) {
        let mut source = source.clone();
        source.clips.clear();
        let mut document = candidate.into_document();
        document.clips.clear();
        (source, ScaleCandidate { document })
    }

    #[test]
    fn the_clip_driven_obligations_are_declared_only_by_the_tracks_that_evidence_them() {
        let capability = complete_capability();

        // No clips at all: none of the three has anything to read.
        let unanimated = compensated_document();
        let unanimated_plan = plan_scale(&ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 0.01,
            },
            document: &unanimated,
            capability: &capability,
        })
        .unwrap();
        assert!(unanimated.clips.is_empty());
        let obligations = unanimated_plan.proof_obligations();
        assert!(!obligations.prove_keys);
        assert!(!obligations.prove_cubic_interiors);
        assert!(!obligations.prove_trajectories);

        // An affected rotation track: a sample time exists, a translation
        // payload does not.
        let rotation_only = rotation_only_clip_document();
        let obligations = multi_joint_plan(&rotation_only, &capability).proof_obligations();
        assert!(!obligations.prove_keys);
        assert!(!obligations.prove_cubic_interiors);
        assert!(obligations.prove_trajectories);

        // An affected *linear* translation track: keys and trajectories have
        // evidence; a linear segment has no interior time to bound.
        let mut linear_only = multi_joint_document();
        linear_only.clips[0].tracks.truncate(1);
        assert_eq!(
            linear_only.clips[0].tracks[0].interpolation,
            Interpolation::Linear
        );
        let obligations = multi_joint_plan(&linear_only, &capability).proof_obligations();
        assert!(obligations.prove_keys);
        assert!(!obligations.prove_cubic_interiors);
        assert!(obligations.prove_trajectories);

        // And the unmodified fixture, which carries both track kinds, still
        // declares all three — so the three negatives above are the evidence
        // gate and not a planner that stopped declaring them.
        let animated = multi_joint_document();
        let obligations = multi_joint_plan(&animated, &capability).proof_obligations();
        assert!(obligations.prove_keys);
        assert!(obligations.prove_cubic_interiors);
        assert!(obligations.prove_trajectories);
    }

    #[test]
    fn a_declared_key_obligation_with_no_affected_translation_track_is_missing_not_vacuous() {
        let doc = multi_joint_document();
        let capability = complete_capability();
        let plan = multi_joint_plan(&doc, &capability);
        assert!(plan.proof_obligations().prove_keys);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        prove_scale(&doc, &candidate, &plan).unwrap();

        let (clipless, clipless_candidate) = without_clips(&doc, candidate);
        let error = prove_scale(&clipless, &clipless_candidate, &plan).unwrap_err();
        let ScaleError::MissingProofEvidence { kind, detail } = error else {
            panic!("expected missing key evidence, got {error:?}");
        };
        assert_eq!(kind, ProofResidualKind::KeyTranslation);
        assert_eq!(detail, "no_affected_translation_track");
    }

    #[test]
    fn a_declared_cubic_obligation_with_no_affected_cubic_segment_is_missing_not_vacuous() {
        let doc = multi_joint_document();
        let capability = complete_capability();
        let plan = multi_joint_plan(&doc, &capability);
        assert!(plan.proof_obligations().prove_cubic_interiors);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();

        // Drop only the cubic track, from both sides. The linear translation
        // track survives, so the key obligation still has its evidence and
        // the refusal below is the cubic one specifically — the two are
        // reported separately on purpose.
        let mut source = doc.clone();
        source.clips[0].tracks.truncate(1);
        let mut document = candidate.into_document();
        document.clips[0].tracks.truncate(1);
        let error = prove_scale(&source, &ScaleCandidate { document }, &plan).unwrap_err();
        let ScaleError::MissingProofEvidence { kind, detail } = error else {
            panic!("expected missing cubic evidence, got {error:?}");
        };
        assert_eq!(kind, ProofResidualKind::CubicInterior);
        assert_eq!(detail, "no_affected_cubic_interior_time");
    }

    #[test]
    fn a_declared_trajectory_obligation_with_no_sample_time_is_missing_not_vacuous() {
        // `prove_trajectories` is the widest of the three, so its refusal is
        // only reachable from a plan that declares it *without* the other
        // two — otherwise the key or cubic refusal fires first and this one
        // would never be observed.
        let doc = rotation_only_clip_document();
        let capability = complete_capability();
        let plan = multi_joint_plan(&doc, &capability);
        let obligations = plan.proof_obligations();
        assert!(obligations.prove_trajectories);
        assert!(!obligations.prove_keys);
        assert!(!obligations.prove_cubic_interiors);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        prove_scale(&doc, &candidate, &plan).unwrap();

        let (clipless, clipless_candidate) = without_clips(&doc, candidate);
        let error = prove_scale(&clipless, &clipless_candidate, &plan).unwrap_err();
        let ScaleError::MissingProofEvidence { kind, detail } = error else {
            panic!("expected missing trajectory evidence, got {error:?}");
        };
        assert_eq!(kind, ProofResidualKind::Trajectory);
        assert_eq!(detail, "no_affected_sample_time");
    }

    /// A two-key `CUBICSPLINE` *rotation* track on bone 2, with zero
    /// tangents so the interpolation is well conditioned and the pose it
    /// produces is not what is under test.
    ///
    /// What matters is only that it is a cubic segment carrying no
    /// translation payload: it is what produces an interior time, and it is
    /// not what the comparison at that time reads.
    fn cubic_rotation_track() -> Track {
        let zero = Quat::from_xyzw(0.0, 0.0, 0.0, 0.0);
        Track {
            bone: 2,
            property: Property::Rotation,
            interpolation: Interpolation::CubicSpline,
            times: vec![0.0, 1.0],
            values: TrackValues::Quats(vec![
                zero,                       // in-tangent @0
                Quat::IDENTITY,             // value @0
                zero,                       // out-tangent @0
                zero,                       // in-tangent @1
                Quat::from_rotation_z(0.5), // value @1
                zero,                       // out-tangent @1
            ]),
        }
    }

    /// The same linear translation track `multi_joint_document` carries on
    /// bone 1, so the fixtures below differ from that document only where
    /// they mean to.
    fn linear_translation_track() -> Track {
        Track {
            bone: 1,
            property: Property::Translation,
            interpolation: Interpolation::Linear,
            times: vec![0.0, 1.0],
            values: TrackValues::Vec3s(vec![
                Vec3::new(0.0, 100.0, 0.0),
                Vec3::new(0.0, 200.0, 0.0),
            ]),
        }
    }

    #[test]
    fn the_cubic_obligations_two_halves_must_meet_inside_one_clip_and_neither_need_be_the_other() {
        // The cubic obligation is the only one whose evidence is a
        // *conjunction*: an interior time has to exist, and something the
        // comparison reads has to exist at it. Interior times are harvested
        // per clip (`clip_sample_times`) and compared against that clip's
        // tracks (`check_track_value_residual`), so the two halves must meet
        // inside one clip — and neither half has to be the other's track.
        //
        // `multi_joint_document` cannot show either fact: its cubic track *is*
        // a translation track, in the same clip as another translation track,
        // so "per clip", "document-wide", "cubic segment alone" and "the cubic
        // must be the translation track" all report `true` on it alike.
        let capability = complete_capability();

        // Split across two clips: clip 0 has the payload and no cubic
        // segment, clip 1 has the cubic segment and no payload. Neither clip
        // has both, so there is no cubic-interior evidence — while the
        // document as a whole carries one of each, which is exactly what a
        // document-wide conjunction would (wrongly) accept, and what dropping
        // the translation half of the conjunction would accept too.
        let mut split = multi_joint_document();
        split.clips[0].tracks = vec![linear_translation_track()];
        split.clips.push(Clip {
            name: "cubic_rotation".into(),
            duration_s: 1.0,
            tracks: vec![cubic_rotation_track()],
        });
        let split_plan = multi_joint_plan(&split, &capability);
        let obligations = split_plan.proof_obligations();
        assert!(obligations.prove_keys);
        assert!(!obligations.prove_cubic_interiors);
        assert!(obligations.prove_trajectories);
        // And it is an otherwise ordinary document, so the negative above is
        // the conjunction and not a document proof would have refused anyway.
        // Under a document-wide conjunction this same call returns `Ok` with
        // `cubic_interior_residual` published as `0.0` from zero comparisons —
        // issue #302's falsehood exactly.
        let split_candidate = build_scale_candidate(&split, &split_plan).unwrap();
        let proof = prove_scale(&split, &split_candidate, &split_plan).unwrap();
        assert_eq!(proof.cubic_interior_residual, 0.0);

        // The same two tracks in one clip: now the obligation has both halves,
        // and the cubic track supplying the interior time is a *rotation*
        // track while the translation read at that time is a different track
        // on a different bone. Requiring the cubic segment to be the
        // translation track itself would refuse this legal document.
        let mut shared = multi_joint_document();
        shared.clips[0].tracks = vec![linear_translation_track(), cubic_rotation_track()];
        let shared_plan = multi_joint_plan(&shared, &capability);
        let obligations = shared_plan.proof_obligations();
        assert!(obligations.prove_keys);
        assert!(obligations.prove_cubic_interiors);
        assert!(obligations.prove_trajectories);
        let shared_candidate = build_scale_candidate(&shared, &shared_plan).unwrap();
        // The declared obligation is honoured: `0.5` is the rotation track's
        // interior, and the translation track is read there.
        prove_scale(&shared, &shared_candidate, &shared_plan).unwrap();
    }

    #[test]
    fn a_single_key_cubic_track_is_not_a_cubic_segment() {
        // `times.len() >= 2` is what makes the track a *segment*:
        // `clip_sample_times` takes interiors from `times.windows(2)`, so one
        // key yields none. A one-key cubic alongside a translation track has
        // every other mark of cubic evidence — cubic interpolation, an
        // affected bone, a translation payload in the same clip — and still
        // produces nothing for the obligation to read.
        let mut doc = multi_joint_document();
        doc.clips[0].tracks = vec![
            linear_translation_track(),
            Track {
                bone: 2,
                property: Property::Translation,
                interpolation: Interpolation::CubicSpline,
                times: vec![0.0],
                values: TrackValues::Vec3s(vec![
                    Vec3::ZERO,                 // in-tangent @0
                    Vec3::new(0.0, 100.0, 0.0), // value @0
                    Vec3::ZERO,                 // out-tangent @0
                ]),
            },
        ];
        let capability = complete_capability();
        let plan = multi_joint_plan(&doc, &capability);
        let obligations = plan.proof_obligations();
        assert!(obligations.prove_keys);
        assert!(!obligations.prove_cubic_interiors);
        assert!(obligations.prove_trajectories);

        // Declaring it on this document would publish a `0.0` cubic residual
        // from an interior-time loop with nothing to iterate.
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let proof = prove_scale(&doc, &candidate, &plan).unwrap();
        assert_eq!(proof.cubic_interior_residual, 0.0);
    }

    #[test]
    fn a_track_on_an_unaffected_bone_is_evidence_for_nothing() {
        // Every other fixture here animates a bone that is *in* the closure,
        // so the `affected` filter in `sampled_evidence` never decides
        // anything. `contradictory_parent_chain_document` planned at source
        // node 0 is the exception available: that node is a leaf in the
        // projection hierarchy the closure is walked in, so the closure is one
        // bone of the document's two and the other is free to animate.
        let mut doc = contradictory_parent_chain_document();
        doc.clips.push(Clip {
            name: "unaffected".into(),
            duration_s: 1.0,
            tracks: vec![Track {
                bone: 1,
                property: Property::Translation,
                interpolation: Interpolation::CubicSpline,
                times: vec![0.0, 1.0],
                values: TrackValues::Vec3s(vec![
                    Vec3::ZERO,
                    Vec3::new(0.0, 1.0, 0.0),
                    Vec3::ZERO,
                    Vec3::ZERO,
                    Vec3::new(0.0, 2.0, 0.0),
                    Vec3::ZERO,
                ]),
            }],
        });
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
        assert_eq!(plan.affected_nodes(), &[0]);
        assert_eq!(doc.skeleton.bones.len(), 2);
        // A translation track, cubic, with two keys — every property the
        // three flags read — on the one bone the closure does not contain.
        let obligations = plan.proof_obligations();
        assert!(!obligations.prove_keys);
        assert!(!obligations.prove_cubic_interiors);
        assert!(!obligations.prove_trajectories);

        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        prove_scale(&doc, &candidate, &plan).unwrap();
    }

    #[test]
    fn the_proof_side_key_gate_reads_the_key_evidence_and_not_a_neighbouring_field() {
        // The three proof-side gates differ only in which `SampledEvidence`
        // field each reads, and `a_declared_key_obligation_with_no_affected
        // _translation_track_is_missing_not_vacuous` removes every clip, which
        // takes all three fields false together — so the key gate reading
        // `sample_times` instead of `key_translations` would behave
        // identically there.
        //
        // A rotation-only clip separates them: `sample_times` is true and
        // `key_translations` is false. The plan is built from a document whose
        // only track is a *linear* translation, so it declares `prove_keys`
        // without `prove_cubic_interiors` and the key gate is the first one
        // reached.
        let mut linear_only = multi_joint_document();
        linear_only.clips[0].tracks = vec![linear_translation_track()];
        let capability = complete_capability();
        let plan = multi_joint_plan(&linear_only, &capability);
        let obligations = plan.proof_obligations();
        assert!(obligations.prove_keys);
        assert!(!obligations.prove_cubic_interiors);
        assert!(obligations.prove_trajectories);

        // The proof source keeps a clip — so it still has sample times — and
        // carries no translation payload for the key obligation to read.
        let rotation_only = rotation_only_clip_document();
        let candidate = build_scale_candidate(&rotation_only, &plan).unwrap();
        let error = prove_scale(&rotation_only, &candidate, &plan).unwrap_err();
        let ScaleError::MissingProofEvidence { kind, detail } = error else {
            panic!("expected missing key evidence, got {error:?}");
        };
        assert_eq!(kind, ProofResidualKind::KeyTranslation);
        assert_eq!(detail, "no_affected_translation_track");
    }

    #[test]
    fn an_unskinned_rest_bind_document_declares_neither_skin_nor_bounds() {
        // Both flags are `has_skinned_evidence` on the rest/bind planner too,
        // and nothing here was covering that: every rest/bind skin or bounds
        // test uses a document that *does* carry a skinned instance, and the
        // `has_skinned_evidence` negatives all run on the whole-document
        // planner (`an_unskinned_document_does_not_declare_a_bounds
        // _obligation_it_cannot_check`).
        //
        // A rest/bind rebase does not need a mesh instance: the operation is
        // resolved from `source_skeleton.skins`, so a document that declares a
        // skin but instantiates no mesh through it is legal and must prove.
        // Declaring either flag unconditionally here turns it into a refusal.
        let mut doc = multi_joint_document();
        doc.assets.instances.clear();
        let capability = complete_capability();
        let plan = multi_joint_plan(&doc, &capability);
        let obligations = plan.proof_obligations();
        assert!(!obligations.prove_skin);
        assert!(!obligations.prove_bounds);
        // The clip-driven obligations are unaffected, so this is the skinned
        // evidence gate and not a planner that stopped declaring anything.
        assert!(obligations.prove_keys);
        assert!(obligations.prove_trajectories);

        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let proof = prove_scale(&doc, &candidate, &plan).unwrap();
        // The document is sampled, so a declared skin or bounds obligation
        // here would have been reached rather than skipped for want of a
        // sample time: what the two flags turn on is the instance walk alone.
        assert!(proof.sample_time_count > 0);
    }

    #[test]
    fn a_closure_with_no_transform_only_attachment_does_not_declare_the_affine_obligation() {
        let capability = complete_capability();

        // Every affected node of `multi_joint_document` is either the scaled
        // root or a selected skin joint, so there is no attachment to probe.
        let bare = multi_joint_document();
        let bare_plan = multi_joint_plan(&bare, &capability);
        assert!(bare_plan.transform_only_attachments().is_empty());
        assert!(!bare_plan.proof_obligations().prove_transform_only_affine);

        // The same operation on a closure that does carry one declares it,
        // so the negative above is the evidence gate rather than an
        // obligation that stopped being declared at all.
        let attached = compensated_document();
        let attached_plan = plan_scale(&ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 0.01,
            },
            document: &attached,
            capability: &capability,
        })
        .unwrap();
        assert_eq!(attached_plan.transform_only_attachments(), &[2]);
        assert!(
            attached_plan
                .proof_obligations()
                .prove_transform_only_affine
        );
    }

    #[test]
    fn a_declared_transform_only_obligation_with_no_attachment_is_missing_not_vacuous() {
        // The attachment list lives in the plan, so this obligation's
        // evidence cannot vanish from a *document*: the fail-open state
        // §D.6's "a no-op cannot pass" claim has to be protected against is a
        // plan that declares the obligation over an empty list, which is
        // exactly what `plan_rest_bind` produced unconditionally before this
        // gate. Reconstructed here the same way the skin-only and bounds-only
        // plans below are.
        let doc = compensated_document();
        let capability = complete_capability();
        let planned = plan_scale(&ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 0.01,
            },
            document: &doc,
            capability: &capability,
        })
        .unwrap();
        let candidate = build_scale_candidate(&doc, &planned).unwrap();
        prove_scale(&doc, &candidate, &planned).unwrap();

        let mut obligations = planned.proof_obligations();
        assert!(obligations.prove_transform_only_affine);
        obligations.prove_skin = false;
        obligations.prove_bounds = false;
        let unattached = ScalePlan {
            operation: planned.operation(),
            tolerance_policy: planned.tolerance_policy(),
            affected_nodes: planned.affected_nodes().to_vec(),
            transform_only_attachments: Vec::new(),
            common_factor: planned.common_factor(),
            observed_factor: planned.observed_factor(),
            domain_rewrites: planned.domain_rewrites(),
            proof_obligations: obligations,
        };
        let error = prove_scale(&doc, &candidate, &unattached).unwrap_err();
        let ScaleError::MissingProofEvidence { kind, detail } = error else {
            panic!("expected missing transform-only evidence, got {error:?}");
        };
        assert_eq!(kind, ProofResidualKind::TransformOnlyAffine);
        assert_eq!(detail, "no_transform_only_attachment");
    }

    #[test]
    fn a_declared_skin_obligation_with_no_skinned_evidence_is_missing_not_vacuous() {
        // The mirror of `a_declared_bounds_obligation_with_no_skinned
        // _evidence_is_missing_not_vacuous`. Both flags read the same
        // instance walk, so the gate reports one gap once and names it for
        // `prove_bounds` whenever that is declared; the skin-only plan is the
        // shape where `prove_skin` is the one left holding the claim.
        let doc = multi_joint_document();
        let capability = complete_capability();
        let planned = multi_joint_plan(&doc, &capability);
        let candidate = build_scale_candidate(&doc, &planned).unwrap();

        let skin_only = ScalePlan {
            operation: planned.operation(),
            tolerance_policy: planned.tolerance_policy(),
            affected_nodes: planned.affected_nodes().to_vec(),
            transform_only_attachments: Vec::new(),
            common_factor: planned.common_factor(),
            observed_factor: planned.observed_factor(),
            domain_rewrites: planned.domain_rewrites(),
            proof_obligations: ScaleProofObligations {
                prove_rest: false,
                prove_unit_scale_postcondition: false,
                prove_transform_only_affine: false,
                prove_keys: false,
                prove_cubic_interiors: false,
                prove_trajectories: false,
                prove_skin: true,
                prove_bounds: false,
            },
        };
        prove_scale(&doc, &candidate, &skin_only).unwrap();

        let mut unskinned = doc.clone();
        unskinned.assets.instances[0].skin_joints.clear();
        unskinned.assets.instances[0].skin_ibms.clear();
        let mut document = candidate.into_document();
        document.assets.instances[0].skin_joints.clear();
        document.assets.instances[0].skin_ibms.clear();
        let error = prove_scale(&unskinned, &ScaleCandidate { document }, &skin_only).unwrap_err();
        let ScaleError::MissingProofEvidence { kind, detail } = error else {
            panic!("expected missing skin evidence, got {error:?}");
        };
        assert_eq!(kind, ProofResidualKind::SkinMatrix);
        assert_eq!(detail, "no_skinned_instance_in_affected_closure");
    }

    #[test]
    fn a_boneless_document_declares_no_rest_obligation_and_a_declared_one_is_missing() {
        // A whole-document conversion's affected closure is the whole
        // skeleton, so a document with no bones plans an empty one — the only
        // way either planner reaches that state. `prove_rest` walking an
        // empty closure would report a `0.0` rest-translation residual for
        // nodes that do not exist.
        let doc = Document::default();
        assert!(doc.skeleton.bones.is_empty());
        let capability = complete_capability();
        let plan = whole_document_plan(&doc, &capability);
        assert!(plan.affected_nodes().is_empty());
        assert!(!plan.proof_obligations().prove_rest);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        prove_scale(&doc, &candidate, &plan).unwrap();

        let declared = ScalePlan {
            operation: plan.operation(),
            tolerance_policy: plan.tolerance_policy(),
            affected_nodes: Vec::new(),
            transform_only_attachments: Vec::new(),
            common_factor: plan.common_factor(),
            observed_factor: plan.observed_factor(),
            domain_rewrites: plan.domain_rewrites(),
            proof_obligations: ScaleProofObligations {
                prove_rest: true,
                ..plan.proof_obligations()
            },
        };
        let error = prove_scale(&doc, &candidate, &declared).unwrap_err();
        let ScaleError::MissingProofEvidence { kind, detail } = error else {
            panic!("expected missing rest evidence, got {error:?}");
        };
        assert_eq!(kind, ProofResidualKind::RestTranslation);
        assert_eq!(detail, "empty_affected_closure");
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
        // Hand-written joint palettes, so nothing here is recomputed by the
        // code under test. At these poses `multi_joint_document`'s two skin
        // slots compose to
        //
        //     M1 = W1 * B1 = [0.01 I | (0, 2, 0)] * [100 I | (0, -100, 0)]
        //                  = [I | 0.01 * (0, -100, 0) + (0, 2, 0)]
        //                  = [I | (0, 1, 0)]
        //     M2 = W2 * B2 = [0.01 I | (0, 5, 0)] * [100 I | (0, -200, 0)]
        //                  = [I | 0.01 * (0, -200, 0) + (0, 5, 0)]
        //                  = [I | (0, 3, 0)]
        //
        // and the four vertices skin to
        //
        //     (1, 0, 0)  -> M1                                 = ( 1,  1, 0)
        //     (-1, 0, 2) -> M2                                 = (-1,  3, 2)
        //     (0, 2, 0)  -> 0.25 * (0, 3, 0) + 0.75 * (0, 5, 0) = ( 0, 4.5, 0)
        //     (0, -3, 0) -> (0.4 * (0, -2, 0) + 0.4 * (0, 0, 0)) / 0.8
        //                                                       = ( 0, -1, 0)
        let doc = multi_joint_document();
        let slots = [
            Mat4::from_translation(Vec3::new(0.0, 1.0, 0.0)),
            Mat4::from_translation(Vec3::new(0.0, 3.0, 0.0)),
        ];
        let mut accumulator = BoundsAccumulator::default();
        accumulate_skinned_bounds(
            0,
            0,
            &doc.assets.meshes[0].primitives[0],
            &slots,
            &mut accumulator,
        )
        .unwrap();
        let (min, max) = accumulator
            .finish()
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

    /// One vertex, one slot, one influence — the smallest primitive that
    /// exercises [`accumulate_skinned_bounds`]'s per-influence path.
    fn one_influence_primitive(weight: f32) -> Primitive {
        Primitive {
            positions: vec![Vec3::new(1.0, 0.0, 0.0)],
            joints: vec![[0, 0, 0, 0]],
            weights: vec![[weight, 0.0, 0.0, 0.0]],
            ..Primitive::default()
        }
    }

    #[test]
    fn an_infinite_skin_weight_is_rejected_as_a_weight_not_as_a_skinned_result() {
        // The guard is `!weight.is_finite()`, not `weight.is_nan()`, and the
        // two differ exactly on the infinities. An infinite weight is not
        // merely a `NaN` in disguise: it survives `weight != 0.0`, and
        // `inf * (1, 0, 0)` is `(inf, NaN, NaN)` because `inf * 0` is `NaN`,
        // so a `NaN`-only guard would let the vertex through and the failure
        // would surface downstream as `non_finite_result` — blaming the skin
        // equation for a malformed input attribute. The reason string is the
        // assertion, not just the fact of an error.
        let slots = [Mat4::IDENTITY];
        let mut accumulator = BoundsAccumulator::default();
        assert_eq!(
            accumulate_skinned_bounds(
                7,
                3,
                &one_influence_primitive(f32::INFINITY),
                &slots,
                &mut accumulator,
            ),
            Err(ScaleError::InvalidSkinnedPrimitive {
                instance_index: 7,
                primitive_index: 3,
                reason: "non_finite_weight",
            })
        );
        // `NaN` names the same reason, so the guard is one guard and not two
        // behaviours that happen to share a spelling.
        assert_eq!(
            accumulate_skinned_bounds(
                7,
                3,
                &one_influence_primitive(f32::NAN),
                &slots,
                &mut accumulator,
            ),
            Err(ScaleError::InvalidSkinnedPrimitive {
                instance_index: 7,
                primitive_index: 3,
                reason: "non_finite_weight",
            })
        );
    }

    #[test]
    fn a_vertex_whose_influences_sum_negative_is_left_out_of_the_bounds() {
        // The normalisation guard is `weight_sum > 0.0`, not
        // `weight_sum != 0.0`. Nothing upstream constrains authored weights to
        // be non-negative — `validate_scene_assets` range-checks joint ids,
        // not weights — so a negative sum is reachable, and dividing by it
        // negates every normalised influence: this vertex would be folded into
        // the bounds at `(1, 10, 0)`, the *reflection* of where its single
        // influence actually places it.
        //
        //   slot 0 translates by (0, 10, 0), so the influence puts the vertex
        //   (1, 0, 0) at (1, 10, 0);
        //   weight -0.5   ->  skinned    = -0.5 * (1, 10, 0) = (-0.5, -5, 0)
        //                     weight_sum = -0.5
        //   -0.5 > 0 is false, so the vertex contributes nothing and the
        //   accumulator is never touched;
        //   `!= 0.0` instead divides: (-0.5, -5, 0) / -0.5 = (1, 10, 0).
        let slots = [Mat4::from_translation(Vec3::new(0.0, 10.0, 0.0))];
        let mut accumulator = BoundsAccumulator::default();
        accumulate_skinned_bounds(
            0,
            0,
            &one_influence_primitive(-0.5),
            &slots,
            &mut accumulator,
        )
        .expect("a negative weight is not malformed, only unusable as a blend");
        assert_eq!(accumulator.finish(), None);

        // The same primitive with the sign flipped does land in the bounds, so
        // the emptiness above is the sign's doing and not the fixture failing
        // to reach the accumulator at all.
        let mut accumulator = BoundsAccumulator::default();
        accumulate_skinned_bounds(
            0,
            0,
            &one_influence_primitive(0.5),
            &slots,
            &mut accumulator,
        )
        .unwrap();
        assert_eq!(
            accumulator.finish(),
            Some((Vec3::new(1.0, 10.0, 0.0), Vec3::new(1.0, 10.0, 0.0)))
        );
    }

    #[test]
    fn the_fourth_skin_influence_of_a_vertex_is_walked_like_the_first_three() {
        // glTF's `JOINTS_0`/`WEIGHTS_0` carry exactly four influences per
        // vertex, and `accumulate_skinned_bounds` walks `0..4`. No other
        // fixture in this file gives a vertex a nonzero *fourth* influence, so
        // a walk that stopped at three would still pass every one of them —
        // while silently dropping a legal influence from the bounds both sides
        // of the proof are measured on.
        //
        // Dropping it symmetrically is invisible: the same truncated walk runs
        // over the source and the candidate, so an equally-wrong pair of
        // bounds still agree. The defect below is therefore one only the
        // fourth slot carries — a candidate that rebound slot 3 to slot 0's
        // joint. `validate_candidate_structure` compares mesh identity, skin
        // joints and per-primitive vertex counts, but not per-vertex joint
        // indices, and the skin-matrix obligation compares `W * B` per
        // `skin_joints` slot, which this leaves untouched: the bounds walk is
        // the only thing that can see it.
        //
        // Every joint is a direct child of the root, so with a whole-document
        // factor of `0.01` the four rest-world skin matrices (identity binds)
        // are pure translations `0.01 * (0, k, 0)` for `k = 1..4`, and the one
        // vertex is evenly blended across all four:
        //
        //   source skinned    = 0.25 * (W1 + W2 + W3 + W4) * p
        //   candidate skinned = 0.25 * (W1 + W2 + W3 + W1) * p
        //   difference        = 0.25 * 0.01 * (0, 1 - 4, 0) = (0, -0.0075, 0)
        //
        // against a bounds tolerance of `1e-6 + 1e-5 * O(1)`, so the
        // rejection is four orders of magnitude clear of the band.
        let nodes = vec![
            rig(None, 0, Vec3::ZERO),
            rig(Some(0), 1, Vec3::new(0.0, 1.0, 0.0)),
            rig(Some(0), 2, Vec3::new(0.0, 2.0, 0.0)),
            rig(Some(0), 3, Vec3::new(0.0, 3.0, 0.0)),
            rig(Some(0), 4, Vec3::new(0.0, 4.0, 0.0)),
        ];
        let mut doc = rig_document(&nodes, &[1, 2, 3, 4], 0, Mat4::IDENTITY);
        doc.assets.meshes[0].primitives[0] = Primitive {
            positions: vec![Vec3::new(1.0, 0.0, 0.0)],
            joints: vec![[0, 1, 2, 3]],
            weights: vec![[0.25, 0.25, 0.25, 0.25]],
            ..Primitive::default()
        };
        assert_eq!(doc.assets.instances[0].skin_joints, vec![1, 2, 3, 4]);
        let capability = complete_capability();
        let plan = whole_document_plan(&doc, &capability);
        assert!(plan.proof_obligations().prove_bounds);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        // The undoctored candidate proves, so the rejection below is the
        // rebound influence and nothing about the fixture.
        prove_scale(&doc, &candidate, &plan).unwrap();

        let mut broken = candidate.document().clone();
        broken.assets.meshes[0].primitives[0].joints[0] = [0, 1, 2, 0];
        let broken = ScaleCandidate { document: broken };
        assert!(matches!(
            prove_scale(&doc, &broken, &plan).unwrap_err(),
            ScaleError::ProofResidualExceeded {
                kind: ProofResidualKind::Bounds,
                ..
            }
        ));
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
    fn the_appendix_d_v2_tolerance_identity_is_pinned_through_plan_and_proof() {
        // DESIGN.md Appendix D §D.1/§D.6: producers record this identity and
        // these thresholds in evidence, so a change to either is a new policy
        // identity rather than a silent retune.
        fn assert_appendix_d_v2(policy: ScaleTolerancePolicy) {
            assert_eq!(policy.id, "appendix-d-v2");
            assert_eq!(policy.relative_orthogonality, 1e-5);
            assert_eq!(policy.equal_axis, 1e-5);
            assert_eq!(policy.common_factor, 1e-5);
            assert_eq!(policy.singular_determinant_relative, 1e-6);
            assert_eq!(policy.scalar_absolute, 1e-6);
            assert_eq!(policy.scalar_relative, 1e-5);
            assert_eq!(policy.rotation_residual_radians, 1e-5);
            // `2^-14` exactly: the v2 unit-scale bound, on the binary32
            // mantissa grid the composed-scale measurement lives on.
            assert_eq!(policy.postcondition_unit_scale_residual, 6.103_515_625e-5);
            assert_eq!(policy.proof_sample_work_budget, 400_000_000);
            // `abs_error <= 1e-6 + 1e-5 * max(abs(before), abs(after))`, at a
            // hand-computed operand pair.
            assert!((policy.scalar_tolerance(0.0, 100.0) - 0.001_001).abs() < 1e-12);
            // The unit-scale bound is *derived* from the common-factor band,
            // not declared independently: four bands is `4e-5`, and the
            // declared bound is the next power of two at or above it, which
            // is also an exact multiple of `2^-23` and therefore a value the
            // binary32 composed-scale measurement can equal. Pinning every
            // half of that sentence keeps a future retune of either number
            // from silently reopening the window §D.1 closes.
            let bands = ScaleTolerancePolicy::UNIT_SCALE_BANDS * policy.common_factor;
            assert!((bands - 4e-5).abs() < 1e-20, "four bands {bands}");
            assert!(policy.postcondition_unit_scale_residual >= bands);
            assert_eq!(policy.postcondition_unit_scale_residual, 2f64.powi(-14));
            assert_eq!(
                policy.postcondition_unit_scale_residual,
                512.0 * 2f64.powi(-23)
            );
            // Rounding onto that grid is a rounding, not an open-ended
            // loosening: the declared bound is the *next* power of two at or
            // above four bands, so halving it must fall below them. That
            // pins "rounded up to the next power of two" exactly, rather
            // than merely bounding the result from one side.
            assert!(policy.postcondition_unit_scale_residual / 2.0 < bands);
            // Three of the four bands are analytic — the declared-factor
            // match, the mixed-factor match, and the equal-axis match, each
            // contributing at most `c / (1 - c)` — so the composed analytic
            // worst case is `(1 - c)^-3 - 1`. The declared bound has to sit
            // strictly above it with a whole band left over, which is what
            // makes the fourth band genuine float headroom.
            let c = policy.common_factor;
            // `(1 - x)^-3 - 1 = 3x + 6x^2 + 10x^3 + ...`, so at `x = 1e-5`
            // the first two terms are `3.00006e-5` and everything after them
            // is below `1.1e-14`.
            let analytic_worst_case = (1.0 - c).powi(-3) - 1.0;
            assert!(
                (analytic_worst_case - 3.00006e-5).abs() < 1e-13,
                "three composed bands {analytic_worst_case}"
            );
            assert!(
                policy.postcondition_unit_scale_residual - analytic_worst_case > c,
                "headroom {} is under one band",
                policy.postcondition_unit_scale_residual - analytic_worst_case
            );
        }

        let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        let capability = complete_capability();

        let whole_document = whole_document_plan(&doc, &capability);
        assert_appendix_d_v2(whole_document.tolerance_policy());
        let candidate = build_scale_candidate(&doc, &whole_document).unwrap();
        let proof = prove_scale(&doc, &candidate, &whole_document).unwrap();
        assert_appendix_d_v2(proof.tolerance_policy);

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
        assert_appendix_d_v2(rest_bind.tolerance_policy());
        let candidate = build_scale_candidate(&doc, &rest_bind).unwrap();
        let proof = prove_scale(&doc, &candidate, &rest_bind).unwrap();
        assert_appendix_d_v2(proof.tolerance_policy);
    }

    // --- Tolerance boundary and reported maxima ----------------------------

    #[test]
    fn a_residual_exactly_at_its_tolerance_is_accepted_and_the_next_one_up_is_not() {
        // DESIGN.md Appendix D §D.1 states every residual is "at most" its
        // bound, i.e. the bound is *inclusive*. `check_residual` therefore
        // rejects on `observed > tolerance`, and nothing else in the module
        // distinguishes that from `observed >= tolerance` unless a comparison
        // actually lands on the bound. `next_up` is the immediately larger
        // `f64`, so the accept/reject pair below straddles the bound with no
        // representable value in between.
        let bound = ScaleTolerancePolicy::APPENDIX_D_V2.postcondition_unit_scale_residual;
        assert_eq!(
            check_residual(ProofResidualKind::UnitScale, bound, bound),
            Ok(())
        );
        let above = f64::from_bits(bound.to_bits() + 1);
        assert_eq!(
            check_residual(ProofResidualKind::UnitScale, above, bound),
            Err(ScaleError::ProofResidualExceeded {
                kind: ProofResidualKind::UnitScale,
                observed: above,
                tolerance: bound,
            })
        );
    }

    #[test]
    fn a_unit_scale_residual_exactly_on_the_policy_bound_still_proves() {
        // The same inclusive boundary, reached end to end rather than by
        // calling the comparison directly. This is the reason the v2 policy
        // states its unit-scale bound as `2^-14`: the measured residual is
        // `after_scale - 1` for a binary32 `after_scale`, hence always an
        // integer multiple of `2^-23` near unit magnitude, and `2^-14` is
        // `512 * 2^-23`. A bound off that grid could never be *met*, only
        // undershot, and the inclusive/exclusive distinction would be
        // untestable.
        //
        // The source is a rig whose whole affected domain has an exact
        // rest-world factor of `0.5`, so `plan_scale` accepts with a zero
        // factor residual and every other obligation below is analytically
        // exact:
        //
        //   W0 = W1 = diag(0.5),  B1 = diag(2),  W1 * B1 = I
        //
        // The candidate is then hand-built to sit exactly on the bound: its
        // root local scale is `1 + 2^-14` (exactly representable), and its
        // inverse bind is `diag(1 - 2^-14)`. Their product rounds to exactly
        // `1.0` in binary32 — `(1 + 2^-14)(1 - 2^-14) = 1 - 2^-28`, and
        // `2^-28` is far below the `2^-24` half-ulp below one — so the skin
        // equation and the skinned bounds both still hold exactly while the
        // composed scale is off by exactly the declared bound.
        let nodes = vec![
            RigNode {
                parent: None,
                source_node_index: 0,
                translation: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                scale: Vec3::splat(0.5),
            },
            rig(Some(0), 1, Vec3::ZERO),
        ];
        let doc = rig_document(&nodes, &[1], 0, Mat4::from_scale(Vec3::splat(2.0)));
        let capability = complete_capability();
        let plan = plan_scale(&ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 0.5,
            },
            document: &doc,
            capability: &capability,
        })
        .unwrap();
        assert!(plan.proof_obligations().prove_unit_scale_postcondition);

        let bound = plan.tolerance_policy().postcondition_unit_scale_residual;
        let mut boundary = build_scale_candidate(&doc, &plan).unwrap().into_document();
        boundary.skeleton.bones[0].rest.scale = Vec3::splat(1.0 + 2.0f32.powi(-14));
        boundary.assets.instances[0].skin_ibms[0] =
            Mat4::from_scale(Vec3::splat(1.0 - 2.0f32.powi(-14)));
        let boundary = ScaleCandidate { document: boundary };

        let proof = prove_scale(&doc, &boundary, &plan).unwrap();
        assert_eq!(proof.unit_scale_residual, bound);
        assert_eq!(proof.unit_scale_residual, 2f64.powi(-14));
    }

    #[test]
    fn a_successful_proof_reports_the_residual_maxima_it_actually_observed() {
        // `ScaleProof`'s residual fields are exactly what DESIGN.md Appendix
        // D §D.6 requires producer evidence to serialize, so a proof that
        // succeeded while reporting `0.0` for a residual that was really
        // `2.2e-10` would publish a false record. Accept/reject is unaffected
        // by that confusion, so only an assertion on a *nonzero* maximum from
        // a *successful* proof can catch it.
        //
        // Every value below is the same single quantity, hand-computed:
        // the whole-document builder narrows the declared `0.01` to binary32
        // before multiplying, while proof forms its expectation in `f64`.
        //
        //   0.01_f32 = 10737418 * 2^-30 = 0.00999999977648258209228515625
        //   0.01_f64 = 0.010000000000000000208166817117216851...
        //   residual = 0.01_f64 - 0.01_f32 = 2.2351741811588166e-10
        //
        // and the tolerance it passes is `1e-6 + 1e-5 * 0.01 = 1.0001e-6`.
        //
        //   rest translation: child rest `(0, 1, 0)` -> `(0, 0.01_f32, 0)`
        //   mesh position:    vertex `(1, 0, 0)` -> `(0.01_f32, 0, 0)`
        //   bounds:           the one weighted vertex skins to `(1, 1, 0)`
        //                     in the source and `(0.01_f32, 0.01_f32, 0)` in
        //                     the candidate, and min == max
        let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        let capability = complete_capability();
        let plan = whole_document_plan(&doc, &capability);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let proof = prove_scale(&doc, &candidate, &plan).unwrap();

        let expected = 0.01_f64 - (0.01_f32 as f64);
        assert_eq!(expected, 2.235_174_181_158_816_6e-10);
        assert_eq!(proof.rest_translation_residual, expected);
        assert_eq!(proof.mesh_position_residual, expected);
        assert_eq!(proof.bounds_residual, expected);
        // The skin equation, by contrast, is genuinely exact here: proof
        // forms its expectation with the same binary32 `scale_translation_only`
        // the builder used, so there is nothing to round. Pinning it at zero
        // keeps the three nonzero maxima above attributable.
        assert_eq!(proof.skin_matrix_residual, 0.0);
    }

    #[test]
    fn the_reported_unit_scale_residual_is_the_maximum_not_the_last_node_seen() {
        // `ScaleProof::unit_scale_residual` is a *maximum* over the affected
        // nodes, and #284 will freeze it as a published evidence field, so a
        // proof that reported the last node's residual instead of the largest
        // would publish a smaller number than it observed — accept/reject
        // unchanged, record false. This one and `rest_rotation_residual` are
        // the two checked against a fixed policy bound rather than a
        // before/after-derived tolerance, so they reach the shared fold
        // through `record_and_check` rather than `check_and_track`; for each
        // of them only a fixture whose maximum is *not* at the last affected
        // node can tell a maximum from a last-write — see
        // `the_reported_rest_rotation_residual_is_the_maximum_not_the_last_node_seen`
        // for the other.
        //
        // `plan.affected_nodes` is ascending `BoneId` order, so the rig puts
        // the larger residual on node 0. With `u = 2^-17 = 64 * 2^-23` and a
        // declared factor of `0.5`, every product below is exact in binary32:
        //
        //   root local scale  = 0.5 * (1 + u)          -> s_0 = 0.5 * (1 + u)
        //   child local scale = 1 - u/2
        //   child world       = 0.5 * (1 + u) * (1 - u/2)
        //                     = 0.5 * (1 + u/2)        (the `2^-36` term is
        //                                               below the `2^-24` ulp)
        //
        // The mixed-factor band sees `|A_1 - s_0| = 0.5 * u/2` against
        // `1e-5 * 0.5 * (1 + u)`, so it accepts. The candidate's root local
        // scale is `0.5 * (1 + u) * 2 = 1 + u`, hence
        //
        //   node 0 residual = u   = 64 * 2^-23 = 7.62939453125e-6
        //   node 1 residual = u/2 = 32 * 2^-23 = 3.814697265625e-6
        let u = 2f32.powi(-17);
        let nodes = vec![
            RigNode {
                parent: None,
                source_node_index: 0,
                translation: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                scale: Vec3::splat(0.5 * (1.0 + u)),
            },
            RigNode {
                parent: Some(0),
                source_node_index: 1,
                translation: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                scale: Vec3::splat(1.0 - u * 0.5),
            },
        ];
        let doc = rig_document(&nodes, &[1], 0, Mat4::IDENTITY);
        let capability = complete_capability();
        let plan = plan_scale(&ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 0.5,
            },
            document: &doc,
            capability: &capability,
        })
        .unwrap();
        // The order the maximum is folded in: node 0 first, node 1 last.
        assert_eq!(plan.affected_nodes(), &[0, 1]);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let proof = prove_scale(&doc, &candidate, &plan).unwrap();
        assert_eq!(proof.unit_scale_residual, 64.0 * 2f64.powi(-23));
        // Stated as its own inequality so the assertion above cannot be read
        // as "whatever the last node happened to produce".
        assert!(proof.unit_scale_residual > 32.0 * 2f64.powi(-23));
    }

    #[test]
    fn a_skin_only_plan_still_evaluates_its_sampled_obligations() {
        // `prove_skin` is one of the flags that arms `prove_scale`'s sampled
        // loop. Under every plan the two planners construct it is co-declared
        // with `prove_keys`, `prove_cubic_interiors` and `prove_trajectories`,
        // so dropping it from that trigger changes nothing observable — see
        // the note on `ScaleProofObligations::prove_skin`. This synthetic plan
        // is the shape where it is load-bearing on its own: with every other
        // sampled flag off, the sample-coverage `prove_scale` reports is
        // entirely `prove_skin`'s doing.
        let doc = multi_joint_document();
        let capability = complete_capability();
        let planned = multi_joint_plan(&doc, &capability);
        let candidate = build_scale_candidate(&doc, &planned).unwrap();

        let skin_only = ScalePlan {
            operation: planned.operation(),
            tolerance_policy: planned.tolerance_policy(),
            affected_nodes: planned.affected_nodes().to_vec(),
            transform_only_attachments: Vec::new(),
            common_factor: planned.common_factor(),
            observed_factor: planned.observed_factor(),
            domain_rewrites: planned.domain_rewrites(),
            proof_obligations: ScaleProofObligations {
                prove_rest: false,
                prove_unit_scale_postcondition: false,
                prove_transform_only_affine: false,
                prove_keys: false,
                prove_cubic_interiors: false,
                prove_trajectories: false,
                prove_skin: true,
                prove_bounds: false,
            },
        };

        let proof = prove_scale(&doc, &candidate, &skin_only).unwrap();
        // `multi_joint_document`'s clip has key times `{0.0, 1.0}` shared by
        // both tracks plus the analytic midpoint `0.5` of the single cubic
        // segment: three sample times, all of them harvested for `prove_skin`
        // alone.
        assert_eq!(proof.sample_time_count, 3);
        // Nothing else was claimed, so nothing else may be reported as
        // checked.
        assert_eq!(proof.key_translation_residual, 0.0);
        assert_eq!(proof.cubic_interior_residual, 0.0);
        assert_eq!(proof.trajectory_residual, 0.0);
        assert_eq!(proof.bounds_residual, 0.0);
    }

    #[test]
    fn a_bounds_only_plan_still_evaluates_its_sampled_bounds() {
        // The mirror of the skin-only plan above, for `prove_bounds`. Both of
        // the guards this refactor introduced — `sample_time_obligations`'s
        // early return and `check_skin_and_bounds`'s — test `prove_skin ||
        // prove_bounds`, and under every plan the two planners construct the
        // two flags are co-declared, so dropping `prove_bounds` from either
        // guard changes nothing observable on a planner-built plan. This
        // synthetic plan is the shape where it is load-bearing on its own:
        // with `prove_skin` off, a dropped `prove_bounds` clause turns the
        // whole obligation into a silent no-op that still reports a `0.0`
        // bounds residual.
        //
        // The defect is `a_reweighted_vertex_is_named_by_the_bounds
        // _obligation`'s: per-vertex weights are the one rewritten payload no
        // other obligation reads, and at rest both joint palettes are the
        // identity, so it is invisible until the clip drives the two joints
        // apart — which is what makes it reach the *sampled* bounds walk and
        // not merely the rest-pose one.
        let doc = multi_joint_document();
        let capability = complete_capability();
        let planned = multi_joint_plan(&doc, &capability);
        let candidate = build_scale_candidate(&doc, &planned).unwrap();

        let bounds_only = ScalePlan {
            operation: planned.operation(),
            tolerance_policy: planned.tolerance_policy(),
            affected_nodes: planned.affected_nodes().to_vec(),
            transform_only_attachments: Vec::new(),
            common_factor: planned.common_factor(),
            observed_factor: planned.observed_factor(),
            domain_rewrites: planned.domain_rewrites(),
            proof_obligations: ScaleProofObligations {
                prove_rest: false,
                prove_unit_scale_postcondition: false,
                prove_transform_only_affine: false,
                prove_keys: false,
                prove_cubic_interiors: false,
                prove_trajectories: false,
                prove_skin: false,
                prove_bounds: true,
            },
        };

        // The untouched candidate still proves, and the three sample times
        // are entirely `prove_bounds`'s doing.
        let proof = prove_scale(&doc, &candidate, &bounds_only).unwrap();
        assert_eq!(proof.sample_time_count, 3);
        assert_eq!(proof.skin_matrix_residual, 0.0);

        let mut broken = candidate.document().clone();
        broken.assets.meshes[0].primitives[0].weights[2] = [0.75, 0.25, 0.0, 0.0];
        let broken = ScaleCandidate { document: broken };
        assert!(matches!(
            prove_scale(&doc, &broken, &bounds_only).unwrap_err(),
            ScaleError::ProofResidualExceeded {
                kind: ProofResidualKind::Bounds,
                ..
            }
        ));
    }

    #[test]
    fn each_clip_is_proved_against_its_own_sample_times() {
        // `prove_scale` harvests each clip's sample times once, up front, into
        // an index-parallel cache, then reads `clip_times[clip_index]` inside
        // the per-clip loop. That index is new in this revision — the times
        // used to be derived inside the loop, where they could not be wrong —
        // and reading clip 0's times while proving clip 1 is invisible to
        // every fixture whose clips share a timeline.
        //
        // The two clips here deliberately do not. Both drive bone 1, which
        // together with bone 2 skins the single vertex `(1, 0, 0)` at
        // `0.5 / 0.5`:
        //
        //   clip 0  times {0, 1}     bone 1 at the origin throughout
        //   clip 1  times {0, 1, 2}  bone 1 at the origin at 0 and 1,
        //                            and at (0, 10, 0) at 2
        //
        // At every time in clip 0 — and at clip 1's first two — both joint
        // palettes are the identity, so the vertex skins to `(1, 0, 0)`
        // whatever its weights are. Only at `t = 2` do the palettes differ:
        //
        //   source     0.5 * (1, 10, 0) + 0.5 * (1, 0, 0) = (1, 5, 0)
        //   candidate  1.0 * (1, 10, 0)                   = (1, 10, 0)
        //
        // a `5.0` bounds residual against a `1e-6 + 1e-5 * 10` tolerance.
        // Sampling clip 1 at `{0, 1}` clamps its track to its first key and
        // never reaches that pose, so the defect goes unseen and the proof
        // succeeds.
        let nodes = vec![
            rig(None, 0, Vec3::ZERO),
            rig(Some(0), 1, Vec3::ZERO),
            rig(Some(0), 2, Vec3::ZERO),
        ];
        let mut doc = rig_document(&nodes, &[1, 2], 0, Mat4::IDENTITY);
        doc.assets.meshes[0].primitives[0] = Primitive {
            positions: vec![Vec3::new(1.0, 0.0, 0.0)],
            joints: vec![[0, 1, 0, 0]],
            weights: vec![[0.5, 0.5, 0.0, 0.0]],
            ..Primitive::default()
        };
        let clip = |name: &str, times: Vec<f32>, values: Vec<Vec3>| Clip {
            name: name.into(),
            duration_s: f64::from(*times.last().expect("at least one key time")),
            tracks: vec![Track {
                bone: 1,
                property: Property::Translation,
                interpolation: Interpolation::Linear,
                times,
                values: TrackValues::Vec3s(values),
            }],
        };
        doc.clips = vec![
            clip("still", vec![0.0, 1.0], vec![Vec3::ZERO; 2]),
            clip(
                "lift",
                vec![0.0, 1.0, 2.0],
                vec![Vec3::ZERO, Vec3::ZERO, Vec3::new(0.0, 10.0, 0.0)],
            ),
        ];

        let capability = complete_capability();
        let plan = plan_scale(&ScaleRequest {
            operation: ScaleOperation::WholeDocumentLinearUnits { factor: 1.0 },
            document: &doc,
            capability: &capability,
        })
        .unwrap();
        let candidate = build_scale_candidate(&doc, &plan).unwrap();

        // Two key times from the first clip and three from the second, not
        // twice the first clip's two.
        let proof = prove_scale(&doc, &candidate, &plan).unwrap();
        assert_eq!(proof.sample_time_count, 5);

        let mut broken = candidate.document().clone();
        broken.assets.meshes[0].primitives[0].weights[0] = [1.0, 0.0, 0.0, 0.0];
        let broken = ScaleCandidate { document: broken };
        assert!(matches!(
            prove_scale(&doc, &broken, &plan).unwrap_err(),
            ScaleError::ProofResidualExceeded {
                kind: ProofResidualKind::Bounds,
                ..
            }
        ));
    }

    // --- Sampling budget ---------------------------------------------------

    /// A whole-document fixture sized in sample times and skinned vertices,
    /// so a test can name the exact `sample_times * per_sample_work_units`
    /// work its document demands.
    ///
    /// Two bones, one skinned instance with one slot, one primitive, and one
    /// `Linear` translation track — which contributes `times.len()` key times
    /// and no cubic-segment interiors. Its per-sample cost is therefore
    /// `2 * 2 bones + (2 + 1) * 1 slot + 2 * vertices = 7 + 2 * vertices`.
    fn budget_document(key_times: usize, vertices: usize) -> Document {
        let mut doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        doc.assets.meshes[0].primitives[0] = Primitive {
            positions: (0..vertices)
                .map(|v| Vec3::new(v as f32 * 0.5, 1.0, 0.0))
                .collect(),
            joints: vec![[0, 0, 0, 0]; vertices],
            weights: vec![[1.0, 0.0, 0.0, 0.0]; vertices],
            ..Primitive::default()
        };
        let times: Vec<f32> = (0..key_times).map(|i| i as f32 / 1000.0).collect();
        doc.clips.push(Clip {
            name: "clip".into(),
            duration_s: f64::from(*times.last().expect("at least one key time")),
            tracks: vec![Track {
                bone: 1,
                property: Property::Translation,
                interpolation: Interpolation::Linear,
                times,
                values: TrackValues::Vec3s(vec![Vec3::new(0.0, 1.0, 0.0); key_times]),
            }],
        });
        doc
    }

    #[test]
    fn a_document_above_the_sampling_budget_is_refused_with_a_typed_error() {
        // Hand-computed against
        // `ScaleTolerancePolicy::proof_sample_work_budget`:
        //
        //   per-sample cost = 2 sides * 2 bones                  =     4
        //                   + (2 sides + 1 skin residual) * 1 slot =    3
        //                   + 2 sides * 1000 skinned vertices    = 2_000
        //                                                          -------
        //                                                            2_007
        //   sample times    = 200_000 keys, no cubic segments
        //   work            = 200_000 * 2_007 = 401_400_000
        //   budget          =                   400_000_000
        //
        // The refusal is typed and total: proof never silently samples a
        // subset, and it is raised before the first sample time is evaluated.
        let doc = budget_document(200_000, 1_000);
        let capability = complete_capability();
        let plan = whole_document_plan(&doc, &capability);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        assert_eq!(
            prove_scale(&doc, &candidate, &plan).unwrap_err(),
            ScaleError::ProofSamplingBudgetExceeded {
                policy_id: "appendix-d-v2",
                sample_times: 200_000,
                per_sample_cost: 2_007,
                work: 401_400_000,
                budget: 400_000_000,
            }
        );
    }

    #[test]
    fn a_representative_in_budget_document_still_proves() {
        // The same shape at production proportions — a 240-key clip over a
        // 2000-vertex skinned mesh — costs
        // `240 * (4 + 3 + 2 * 2000) = 240 * 4_007 = 961_680` work units, two
        // orders of magnitude inside the budget, and proves.
        let doc = budget_document(240, 2_000);
        let capability = complete_capability();
        let plan = whole_document_plan(&doc, &capability);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        // The vertices really are walked: both obligations that charge for
        // them are declared.
        assert!(plan.proof_obligations().prove_skin);
        assert!(plan.proof_obligations().prove_bounds);
        let proof = prove_scale(&doc, &candidate, &plan).unwrap();
        assert_eq!(proof.sample_time_count, 240);
    }

    /// A synthetic plan carrying only the two obligations
    /// [`per_sample_work_units`] branches on, so a work-unit test can vary
    /// them independently of what either planner happens to declare.
    fn work_unit_plan(prove_skin: bool, prove_bounds: bool) -> ScalePlan {
        ScalePlan {
            operation: ScaleOperation::WholeDocumentLinearUnits { factor: 1.0 },
            tolerance_policy: ScaleTolerancePolicy::APPENDIX_D_V2,
            affected_nodes: vec![1, 2],
            transform_only_attachments: Vec::new(),
            common_factor: 1.0,
            observed_factor: 1.0,
            domain_rewrites: ScaleDomainRewrites {
                rest_hierarchy: false,
                translation_animation: false,
                inverse_binds: false,
                base_mesh_positions: false,
            },
            proof_obligations: ScaleProofObligations {
                prove_rest: false,
                prove_unit_scale_postcondition: false,
                prove_transform_only_affine: false,
                prove_keys: false,
                prove_cubic_interiors: false,
                prove_trajectories: false,
                prove_skin,
                prove_bounds,
            },
        }
    }

    /// Three bones; one mesh whose **two** primitives carry 5 and 7 vertices;
    /// three instances, one of which repeats a joint across slots and one of
    /// which is skinned entirely outside the affected closure `{1, 2}`.
    fn work_unit_document() -> Document {
        let nodes = vec![
            rig(None, 0, Vec3::ZERO),
            rig(Some(0), 1, Vec3::new(0.0, 1.0, 0.0)),
            rig(Some(1), 2, Vec3::new(0.0, 1.0, 0.0)),
        ];
        let mut doc = rig_document(&nodes, &[1, 2], 0, Mat4::IDENTITY);
        let primitive = |vertices: usize| Primitive {
            positions: vec![Vec3::new(1.0, 0.0, 0.0); vertices],
            joints: vec![[0, 0, 0, 0]; vertices],
            weights: vec![[1.0, 0.0, 0.0, 0.0]; vertices],
            ..Primitive::default()
        };
        doc.assets.meshes[0].primitives = vec![primitive(5), primitive(7)];
        let instance = |skin_joints: Vec<BoneId>| MeshInstance {
            source_node_index: 2,
            node: 1,
            mesh: 0,
            skin_ibms: vec![Mat4::IDENTITY; skin_joints.len()],
            skin_joints,
        };
        doc.assets.instances = vec![
            // Three slots, two of which name the *same* joint: legal input,
            // and the shape that makes "slot work cannot exceed the bone
            // count" false.
            instance(vec![1, 1, 2]),
            instance(vec![2]),
            // Outside the affected closure, so neither obligation walks it.
            instance(vec![0]),
        ];
        doc
    }

    #[test]
    fn per_sample_work_units_charges_every_slot_primitive_and_document_side() {
        // Hand-computed, term by term, against what `check_skin_and_bounds`
        // actually performs at one sample time. The affected closure is
        // `{1, 2}`, so the third instance is skipped entirely.
        //
        //   bones      2 sides * 3 bones                            =  6
        //   instance 0 3 slots: 2 sides                             =  6
        //              3 slots: 1 skin residual each                =  3
        //              (5 + 7) vertices * 2 sides                   = 24
        //   instance 1 1 slot:  2 sides                             =  2
        //              1 slot:  1 skin residual                     =  1
        //              (5 + 7) vertices * 2 sides                   = 24
        //
        // The undercount this replaces charged `bones + vertices` — `3 + 24`,
        // once — and dropped the slot term entirely on the false claim that
        // it "cannot exceed the bone count": instance 0 alone owes 9 units of
        // slot work against a 3-bone skeleton.
        let doc = work_unit_document();
        let affected: BTreeSet<BoneId> = [1, 2].into_iter().collect();
        for (prove_skin, prove_bounds, expected) in [
            (true, true, 6 + 6 + 3 + 24 + 2 + 1 + 24),
            (true, false, 6 + 6 + 3 + 2 + 1),
            (false, true, 6 + 6 + 24 + 2 + 24),
            // Neither obligation walks an instance, so only the two
            // forward-kinematics passes remain.
            (false, false, 6),
        ] {
            let plan = work_unit_plan(prove_skin, prove_bounds);
            assert_eq!(
                per_sample_work_units(&doc, &affected, &plan),
                expected,
                "prove_skin={prove_skin} prove_bounds={prove_bounds}"
            );
        }
        // The four literals above, restated as the totals they must be, so a
        // reader does not have to re-add them.
        assert_eq!(
            per_sample_work_units(&doc, &affected, &work_unit_plan(true, true)),
            66
        );
        assert_eq!(
            per_sample_work_units(&doc, &affected, &work_unit_plan(true, false)),
            18
        );
        assert_eq!(
            per_sample_work_units(&doc, &affected, &work_unit_plan(false, true)),
            62
        );
    }

    #[test]
    fn a_document_whose_skin_slots_dominate_its_work_is_refused() {
        // The shape the old `bones + vertices` charge undercounted without
        // bound: many instances, many slots each, almost no vertices. Nothing
        // in `validate_scene_assets` limits either count — it only range-checks
        // joint ids — so `skin_joints` may repeat a joint and an instance list
        // may be arbitrarily long.
        //
        //   per-sample cost = 2 sides * 2 bones                     =      4
        //                   + 50 instances * 100 slots * 2 sides    = 10_000
        //                   + 50 instances * 100 slots * 1 residual =  5_000
        //                   + 50 instances * 1 vertex * 2 sides     =    100
        //                                                             -------
        //                                                             15_104
        //   sample times    = 26_484 keys, no cubic segments
        //   work            = 26_484 * 15_104 = 400_014_336
        //   budget          =                   400_000_000
        //
        // `26_484` is the *first* key count this document cannot afford:
        // `26_483 * 15_104 = 399_999_232` is inside the budget. The old charge
        // read the same document as `2 bones + 50 vertices = 52` units per
        // sample — `1_377_168` work, 0.34% of the budget — while proof went on
        // to perform `26_484 * 50 * 100 * 2 = 264_840_000` slot matrix
        // products.
        let mut doc = budget_document(26_484, 1);
        let instance = MeshInstance {
            source_node_index: 1,
            node: 1,
            mesh: 0,
            skin_joints: vec![1; 100],
            skin_ibms: vec![Mat4::IDENTITY; 100],
        };
        doc.assets.instances = vec![instance; 50];
        let capability = complete_capability();
        let plan = whole_document_plan(&doc, &capability);
        assert!(plan.proof_obligations().prove_skin);
        assert!(plan.proof_obligations().prove_bounds);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        assert_eq!(
            prove_scale(&doc, &candidate, &plan).unwrap_err(),
            ScaleError::ProofSamplingBudgetExceeded {
                policy_id: "appendix-d-v2",
                sample_times: 26_484,
                per_sample_cost: 15_104,
                work: 400_014_336,
                budget: 400_000_000,
            }
        );
    }

    #[test]
    fn cubic_segment_interior_times_count_toward_the_sampling_budget() {
        // `prove_scale` evaluates every cubic segment's analytic interior in
        // addition to its keys, so the budget has to count both. A
        // `CubicSpline` track with `k` keys contributes `k` key times and
        // `k - 1` interiors.
        //
        //   per-sample cost = 2 sides * 2 bones                     =      4
        //                   + 1 slot * (2 sides + 1 skin residual)  =      3
        //                   + 2 sides * 10_000 vertices             = 20_000
        //                                                             -------
        //                                                             20_007
        //   sample times    = 9_998 keys + 9_997 interiors = 19_995
        //   work            = 19_995 * 20_007 = 400_039_965
        //   budget          =                   400_000_000
        //
        // Counting keys alone would report `9_998` sample times here — and
        // `9_998 * 20_007 = 200_029_986`, comfortably inside the budget, so
        // the document would be sampled rather than refused.
        let mut doc = budget_document(9_998, 10_000);
        let keys = doc.clips[0].tracks[0].times.len();
        assert_eq!(keys, 9_998);
        doc.clips[0].tracks[0].interpolation = Interpolation::CubicSpline;
        doc.clips[0].tracks[0].values =
            TrackValues::Vec3s(vec![Vec3::new(0.0, 1.0, 0.0); keys * 3]);
        let capability = complete_capability();
        let plan = whole_document_plan(&doc, &capability);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        assert_eq!(
            prove_scale(&doc, &candidate, &plan).unwrap_err(),
            ScaleError::ProofSamplingBudgetExceeded {
                policy_id: "appendix-d-v2",
                sample_times: 19_995,
                per_sample_cost: 20_007,
                work: 400_039_965,
                budget: 400_000_000,
            }
        );
    }

    #[test]
    fn the_sampling_budget_is_a_ceiling_a_document_may_reach() {
        // DESIGN.md Appendix D §D.1 states every policy quantity as an
        // inclusive "at most", so the budget comparison is `>` and a document
        // whose work lands exactly on the budget is sampled, not refused.
        // Nothing else in the module distinguishes `>` from `>=` unless a
        // document's work lands exactly on the ceiling, and a document that
        // does costs `4e8` work units to prove. Pinned here on synthetic
        // factors instead: `check_sampling_budget` takes the two numbers and
        // the policy, and nothing about the comparison depends on where they
        // came from.
        //
        // `400_000_000 = 20_000 * 20_000`, so the exact-ceiling pair is
        // literal, and `20_001 * 20_000 = 400_020_000` is the next
        // representable step up in `sample_times` — one more sample time of
        // the same document.
        let tol = ScaleTolerancePolicy::APPENDIX_D_V2;
        assert_eq!(tol.proof_sample_work_budget, 400_000_000);
        assert_eq!(check_sampling_budget(&tol, 20_000, 20_000), Ok(()));
        assert_eq!(
            check_sampling_budget(&tol, 20_001, 20_000),
            Err(ScaleError::ProofSamplingBudgetExceeded {
                policy_id: "appendix-d-v2",
                sample_times: 20_001,
                per_sample_cost: 20_000,
                work: 400_020_000,
                budget: 400_000_000,
            })
        );
        // One unit over, reached from the other factor, so neither operand's
        // role is assumed: `400_000_001 = 400_000_001 * 1`.
        assert_eq!(
            check_sampling_budget(&tol, 400_000_001, 1),
            Err(ScaleError::ProofSamplingBudgetExceeded {
                policy_id: "appendix-d-v2",
                sample_times: 400_000_001,
                per_sample_cost: 1,
                work: 400_000_001,
                budget: 400_000_000,
            })
        );
    }

    #[test]
    fn an_overflowing_work_product_saturates_instead_of_wrapping_under_the_budget() {
        // The budget is `sample_times * per_sample_cost` in `u64`, and neither
        // factor is bounded by anything but the document. `saturating_mul` is
        // what makes the overflow fail *closed*: a wrapping product of two
        // enormous factors lands on a small number that sails under the
        // ceiling, which is precisely the refusal this check exists to make.
        //
        // `2^63 * 2 = 2^64`, which is `0` when wrapped — the most hostile case
        // there is, because zero passes any ceiling. Synthetic factors, for
        // the same reason `the_sampling_budget_is_a_ceiling_a_document_may_reach`
        // uses them: `check_sampling_budget` takes the two numbers and the
        // policy, and nothing about the arithmetic depends on where they came
        // from.
        let tol = ScaleTolerancePolicy::APPENDIX_D_V2;
        let sample_times = 9_223_372_036_854_775_808; // 2^63
        assert_eq!(
            check_sampling_budget(&tol, sample_times, 2),
            Err(ScaleError::ProofSamplingBudgetExceeded {
                policy_id: "appendix-d-v2",
                sample_times,
                per_sample_cost: 2,
                work: u64::MAX,
                budget: 400_000_000,
            })
        );
    }

    #[test]
    fn duplicate_key_and_interior_times_across_tracks_are_charged_and_sampled_once() {
        // `clip_sample_times` harvests every affected track's key times into
        // one list and every cubic segment's interior into another, and both
        // are sorted and deduplicated. Two tracks on affected bones sharing a
        // time do not make proof evaluate that time twice, and — because
        // `sample_times` is one of the two factors `check_sampling_budget`
        // multiplies — must not be charged twice either.
        //
        // Two `CubicSpline` tracks, on two different affected bones, carrying
        // the *same* four key times (cloned, so they agree bit for bit):
        //
        //   keys      = {0, 0.001, 0.002, 0.003}                     = 4
        //   interiors = {0.0005, 0.0015, 0.0025}                     = 3
        //                                                              ---
        //   sample times                                                7
        //
        // Undeduplicated the same clip yields `8` keys and `6` interiors, so
        // dropping either `dedup` moves the count off `7`: `11` without the
        // key dedup, `10` without the interior dedup, `14` without both. No
        // interior coincides with a key, so the two lists never overlap and
        // `7` is not an accident of one list absorbing the other.
        let mut doc = budget_document(4, 1);
        let track = &mut doc.clips[0].tracks[0];
        assert_eq!(track.bone, 1);
        track.interpolation = Interpolation::CubicSpline;
        track.values = TrackValues::Vec3s(vec![Vec3::new(0.0, 1.0, 0.0); 4 * 3]);
        let mut twin = track.clone();
        twin.bone = 0;
        doc.clips[0].tracks.push(twin);

        let capability = complete_capability();
        let plan = whole_document_plan(&doc, &capability);
        // Both bones the two tracks target are inside the affected domain, so
        // both tracks really are harvested and the duplication is real.
        assert_eq!(plan.affected_nodes(), &[0, 1]);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let proof = prove_scale(&doc, &candidate, &plan).unwrap();
        assert_eq!(proof.sample_time_count, 7);
    }

    #[test]
    fn a_candidate_skeleton_may_not_carry_bones_the_budget_never_charged() {
        // `per_sample_work_units` measures the *source* skeleton, but
        // `sample_time_obligations` calls `world_at_time` on **both**, and
        // `ScaleCandidate::from_document` is public — so without the
        // `bone_count_mismatch` parity clause the candidate's bone count is
        // caller-supplied work that nothing charges for.
        //
        // Measured before the clause existed, on exactly this shape: the
        // source's charge is
        //
        //   per-sample cost = 2 sides * 2 bones                       =    4
        //                   + (2 sides + 1 skin residual) * 1 slot    =    3
        //                   + 2 sides * 1 skinned vertex              =    2
        //                                                                ----
        //                                                                   9
        //   work            = 4_000 keys * 9 = 36_000
        //   budget          =                  400_000_000
        //
        // — 0.009% of the budget — while proof posed a 60_002-bone candidate
        // skeleton 4_000 times and took `3.71s` in release, more than twice
        // the wall time of a vertex-dominated document the budget scores at
        // 100%. The refusal below is raised before any of that work starts.
        let doc = budget_document(4_000, 1);
        let capability = complete_capability();
        let plan = whole_document_plan(&doc, &capability);
        let affected = plan.affected_set();
        assert_eq!(per_sample_work_units(&doc, &affected, &plan), 9);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();

        let mut padded = candidate.document().clone();
        let root = padded.skeleton.bones[0].clone();
        padded
            .skeleton
            .bones
            .extend(std::iter::repeat_n(root, 60_000));
        assert_eq!(padded.skeleton.bones.len(), 60_002);
        let padded = ScaleCandidate { document: padded };
        assert_eq!(
            prove_scale(&doc, &padded, &plan).unwrap_err(),
            ScaleError::CandidateStructureMismatch {
                reason: "bone_count_mismatch",
            }
        );
        // The unpadded candidate still proves, so the rejection is the bone
        // count and not something the padding happened to break.
        prove_scale(&doc, &candidate, &plan).unwrap();
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

    // --- Skins outside the affected closure (issue #296) -------------------

    /// Append a second, entirely unrelated skinned instance to `doc`: a new
    /// root bone that is the only joint of a new skin, drawing the mesh the
    /// document already has.
    ///
    /// The new bone is a *root*, so no rest/bind closure rooted at bone 0 can
    /// reach it: the closure is the scaled root, the selected skin's joints,
    /// the ancestor paths between them, and their descendants, and this bone
    /// is none of those. `check_skin_and_bounds` therefore skips its instance
    /// outright, which is exactly the gap #296 names.
    ///
    /// `bind` is what the instance *stores* per slot: `Some` writes a
    /// one-element `skin_ibms`, `None` leaves it empty so the slot falls back
    /// to the bone convenience value — which `Bone::inverse_bind` also leaves
    /// unset, making the skin evidence-free on this side.
    fn push_unrelated_skin(doc: &mut Document, bind: Option<Mat4>) -> BoneId {
        let bone = doc.skeleton.bones.len();
        let source_node_index = doc
            .assets
            .source_skeleton
            .nodes
            .iter()
            .map(|node| node.source_node_index)
            .max()
            .expect("the base document projects at least one node")
            + 1;
        doc.skeleton.bones.push(Bone {
            name: "unrelated".into(),
            parent: None,
            rest: Transform {
                translation: Vec3::new(5.0, 0.0, 0.0),
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
            },
            inverse_bind: None,
        });
        doc.assets.source_skeleton.nodes.push(SourceNodeAsset {
            source_node_index,
            name: None,
            parent_source_node_index: None,
            scene_root_indices: vec![0],
            local_rest: SourceNodeLocalRest::Trs {
                translation: Vec3::new(5.0, 0.0, 0.0),
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
            },
            bone: Some(bone),
        });
        let source_skin_index = doc
            .assets
            .source_skeleton
            .skins
            .iter()
            .map(|skin| skin.source_skin_index)
            .max()
            .expect("the base document projects at least one skin")
            + 1;
        doc.assets.source_skeleton.skins.push(SourceSkinAsset {
            source_skin_index,
            name: None,
            skeleton_root_source_node_index: None,
            joint_source_node_indices: vec![source_node_index],
            inverse_bind_accessor: SourceInverseBindAccessor::default(),
            attachments: Vec::new(),
        });
        doc.assets.instances.push(MeshInstance {
            source_node_index,
            node: bone,
            mesh: 0,
            skin_joints: vec![bone],
            skin_ibms: bind.into_iter().collect(),
        });
        bone
    }

    /// `compensated_document` plus that unrelated skin — a rest/bind source
    /// whose closure is `{0, 1, 2}` and whose second instance is bound to
    /// bone 3 alone.
    fn compensated_document_with_unrelated_skin(bind: Option<Mat4>) -> Document {
        let mut doc = compensated_document();
        assert_eq!(push_unrelated_skin(&mut doc, bind), 3);
        doc
    }

    fn compensated_rest_bind_plan(doc: &Document, capability: &ScaleCapabilityFacts) -> ScalePlan {
        plan_scale(&ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 0.01,
            },
            document: doc,
            capability,
        })
        .unwrap()
    }

    #[test]
    fn a_rewritten_unaffected_skins_inverse_binds_are_named_by_their_own_obligation() {
        // The #296 defect, exactly: bone 3 is in no closure, so the skin and
        // bounds obligations skip its instance entirely and a candidate that
        // rewrote its binds proved `Ok`.
        let doc =
            compensated_document_with_unrelated_skin(Some(Mat4::from_scale(Vec3::splat(2.0))));
        let capability = complete_capability();
        let plan = compensated_rest_bind_plan(&doc, &capability);
        assert_eq!(plan.affected_nodes(), &[0, 1, 2]);

        // The honest candidate leaves the unrelated skin alone, and the
        // obligation proves that rather than skipping it.
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        assert_eq!(
            candidate.document().assets.instances[1].skin_ibms,
            doc.assets.instances[1].skin_ibms
        );
        let proof = prove_scale(&doc, &candidate, &plan).unwrap();
        assert_eq!(proof.unaffected_inverse_bind_residual, 0.0);

        // Doctored: `scale(2)` becomes `scale(3)`. Both are diagonal, so the
        // largest component difference — which is what `matrix_residual`
        // reports — is `abs(2 - 3) = 1` on each of the three axis diagonals,
        // against a `1e-6 + 1e-5 * 3 = 3.1e-5` scalar tolerance.
        let mut broken = candidate.document().clone();
        broken.assets.instances[1].skin_ibms[0] = Mat4::from_scale(Vec3::splat(3.0));
        let error = prove_scale(&doc, &ScaleCandidate { document: broken }, &plan).unwrap_err();
        let ScaleError::ProofResidualExceeded {
            kind: ProofResidualKind::UnaffectedInverseBind,
            observed,
            ..
        } = error
        else {
            panic!("expected an unaffected-inverse-bind residual, got {error:?}");
        };
        assert_eq!(observed, 1.0);
    }

    #[test]
    fn a_successful_unaffected_bind_proof_reports_the_residual_maximum_it_observed() {
        // Every other assertion on `unaffected_inverse_bind_residual` in this
        // module reads `0.0`, which is the field's initialized value: none of
        // them can tell a proof that measured and folded a residual from one
        // that never wrote the field at all. #284 will serialize this number,
        // so a permanently-zero field is a permanently-wrong record.
        //
        // The candidate's bind is perturbed *inside* the tolerance this
        // comparison derives for itself, so the proof succeeds and the
        // reported maximum is the perturbation:
        //
        //   source bind    diag(2, 2, 2) with translation column (64, 0, 0)
        //   candidate bind diag(2 + 2^-18, 2 + 2^-17, 2), same translation
        //
        // Both perturbations are exact in binary32 (`2.0` is `2^1`, whose ulp
        // is `2^-22`), `matrix_residual` is an L-infinity fold over the
        // sixteen components, and the larger of the two is `2^-17`. The
        // tolerance is `1e-6 + 1e-5 * max(64, 64) = 6.41e-4`, roughly `84x`
        // the residual, so this is a pass with headroom and not a boundary
        // case. The smaller perturbation is there so the reported number is a
        // genuine maximum rather than the only candidate for one.
        //
        // The translation column is what makes this the rest/bind fixture
        // that separates the two expectations this obligation can state. A
        // bind with a zero translation column is a fixed point of
        // `scale_translation_only`, so on one the whole-document conversion
        // expectation and the rest/bind "unchanged" expectation are the same
        // matrix and the `is_whole_document` branch is unobservable. At `64`
        // the conversion expectation would be `0.64` and this proof would
        // fail instead of reporting a residual.
        let bind = |scale: Vec3| {
            let mut bind = Mat4::from_scale(scale);
            bind.w_axis.x = 64.0;
            bind
        };
        let doc = compensated_document_with_unrelated_skin(Some(bind(Vec3::splat(2.0))));
        let capability = complete_capability();
        let plan = compensated_rest_bind_plan(&doc, &capability);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();

        let mut nudged = candidate.document().clone();
        nudged.assets.instances[1].skin_ibms[0] =
            bind(Vec3::new(2.0 + 2f32.powi(-18), 2.0 + 2f32.powi(-17), 2.0));
        let proof = prove_scale(&doc, &ScaleCandidate { document: nudged }, &plan).unwrap();
        assert_eq!(proof.unaffected_inverse_bind_residual, 2f64.powi(-17));
    }

    #[test]
    fn a_partially_affected_skin_stays_with_the_skin_obligation_that_owns_it() {
        // The skip predicate is `any`, not `all`, and the difference is not
        // cosmetic. An instance with *some* joint in the closure is the skin
        // obligation's, which checks `W * B` on both sides; holding its binds
        // to "unchanged" as well would refuse the honest candidate this very
        // module builds, because `build_rest_bind` rewrites exactly the slots
        // whose joint is affected. That is the fail-closed regression class
        // #296 was deferred from #288 to avoid, re-introduced from the other
        // end.
        //
        // Slot 0 is bone 3 (outside every closure), slot 1 is bone 1 (inside
        // it), so this instance is partially affected and no other fixture
        // here is.
        let unaffected_bind = Mat4::from_scale(Vec3::splat(2.0));
        let mut doc = compensated_document_with_unrelated_skin(Some(unaffected_bind));
        let affected_bind = doc.assets.instances[0].skin_ibms[0];
        doc.assets.instances[1].skin_joints = vec![3, 1];
        doc.assets.instances[1].skin_ibms = vec![unaffected_bind, affected_bind];

        let capability = complete_capability();
        let plan = compensated_rest_bind_plan(&doc, &capability);
        assert_eq!(plan.affected_nodes(), &[0, 1, 2]);

        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let rebased = &candidate.document().assets.instances[1].skin_ibms;
        // The unaffected slot came through byte-identical; the affected one
        // was conjugated by `scale(s)` and did not.
        assert_eq!(rebased[0], unaffected_bind);
        assert_ne!(rebased[1], affected_bind);

        // The builder's own output proves. Under an `all` predicate this
        // instance is no longer skipped, slot 1 is compared against the
        // source bind it was deliberately rebased away from, and this call
        // returns `ProofResidualExceeded { UnaffectedInverseBind }`.
        let proof = prove_scale(&doc, &candidate, &plan).unwrap();
        assert_eq!(proof.unaffected_inverse_bind_residual, 0.0);
    }

    #[test]
    fn a_slot_carrying_both_an_array_and_a_bone_bind_is_compared_through_the_array() {
        // The precedence `stored_instance_bind` documents — the per-instance
        // array first, then the bone convenience value — became normative in
        // this change's §D.6 amendment ("in the resolution order the model
        // defines"). No other fixture gives one slot *both*, so reversing the
        // two orders is invisible: the array-only fixtures resolve through
        // the array either way, and the bone-only fixture through the bone.
        //
        // Here the unrelated slot carries an array bind of `scale(2)` and a
        // bone bind of `scale(4)`, and the two directions are separated by
        // doctoring exactly one of them in the candidate.
        let array_bind = Mat4::from_scale(Vec3::splat(2.0));
        let bone_bind = Mat4::from_scale(Vec3::splat(4.0));
        let mut doc = compensated_document_with_unrelated_skin(Some(array_bind));
        doc.skeleton.bones[3].inverse_bind = Some(bone_bind);
        let capability = complete_capability();
        let plan = compensated_rest_bind_plan(&doc, &capability);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();

        // The array is the authority, so a rewritten array is caught even
        // though the shadowed bone bind still matches. `scale(2)` against
        // `scale(3)` differs by `1` on each of three diagonal components,
        // against a `1e-6 + 1e-5 * 3 = 3.1e-5` tolerance.
        let mut rewritten_array = candidate.document().clone();
        rewritten_array.assets.instances[1].skin_ibms[0] = Mat4::from_scale(Vec3::splat(3.0));
        let error = prove_scale(
            &doc,
            &ScaleCandidate {
                document: rewritten_array,
            },
            &plan,
        )
        .unwrap_err();
        let ScaleError::ProofResidualExceeded {
            kind: ProofResidualKind::UnaffectedInverseBind,
            observed,
            ..
        } = error
        else {
            panic!("expected an unaffected-inverse-bind residual, got {error:?}");
        };
        assert_eq!(observed, 1.0);

        // And the converse, which is the half a one-directional test misses:
        // a bone bind shadowed by a non-empty array is not authority for this
        // slot, so rewriting it changes nothing this obligation claims. Under
        // the reversed precedence this call fails instead.
        let mut rewritten_bone = candidate.document().clone();
        rewritten_bone.skeleton.bones[3].inverse_bind = Some(Mat4::from_scale(Vec3::splat(5.0)));
        let proof = prove_scale(
            &doc,
            &ScaleCandidate {
                document: rewritten_bone,
            },
            &plan,
        )
        .unwrap();
        assert_eq!(proof.unaffected_inverse_bind_residual, 0.0);
    }

    #[test]
    fn an_unaffected_skin_resolved_through_its_bones_is_proved_the_same_way() {
        // The instance stores no array, so both sides resolve the slot
        // through `Bone::inverse_bind` — the module's documented fallback.
        // That evidence is compared too: "the instance stored nothing" is not
        // the same as "there is nothing to compare".
        let mut doc = compensated_document_with_unrelated_skin(None);
        doc.skeleton.bones[3].inverse_bind = Some(Mat4::from_scale(Vec3::splat(2.0)));
        let capability = complete_capability();
        let plan = compensated_rest_bind_plan(&doc, &capability);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let proof = prove_scale(&doc, &candidate, &plan).unwrap();
        assert_eq!(proof.unaffected_inverse_bind_residual, 0.0);

        // Same arithmetic as the instance-array case above: a diagonal
        // `scale(2)` doctored to `scale(3)` differs by `1` per axis.
        let mut broken = candidate.document().clone();
        broken.skeleton.bones[3].inverse_bind = Some(Mat4::from_scale(Vec3::splat(3.0)));
        let error = prove_scale(&doc, &ScaleCandidate { document: broken }, &plan).unwrap_err();
        let ScaleError::ProofResidualExceeded {
            kind: ProofResidualKind::UnaffectedInverseBind,
            observed,
            ..
        } = error
        else {
            panic!("expected an unaffected-inverse-bind residual, got {error:?}");
        };
        assert_eq!(observed, 1.0);
    }

    #[test]
    fn an_unaffected_skin_with_no_bind_evidence_on_either_side_still_proves() {
        // The reason #296 was deferred out of #288: resolving both sides
        // through `instance_bind` would reject this document with
        // `MissingInverseBind`, a fail-closed regression on a skin the
        // operation genuinely does not touch. Nothing is claimed about this
        // slot, and nothing needs to be — but the document must still prove.
        let doc = compensated_document_with_unrelated_skin(None);
        assert!(doc.assets.instances[1].skin_ibms.is_empty());
        assert!(doc.skeleton.bones[3].inverse_bind.is_none());
        let capability = complete_capability();
        let plan = compensated_rest_bind_plan(&doc, &capability);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let proof = prove_scale(&doc, &candidate, &plan).unwrap();
        // The count, not the residual, is what this test turns on. A slot
        // neither side records is out of the proof's scope, and DESIGN.md
        // Appendix D §D.6 requires that to read as an absence — a count of
        // zero. A zero *residual* is also what comparing two identities
        // would publish, so it cannot tell the two apart on its own.
        assert_eq!(proof.unaffected_inverse_bind_comparisons, 0);
        assert_eq!(proof.unaffected_inverse_bind_residual, 0.0);
    }

    #[test]
    fn an_unaffected_skin_whose_bind_evidence_appears_on_only_one_side_is_missing_not_proven() {
        // Between "unchanged, and I can prove it" and "no evidence either
        // way" sits a third case: one side records a bind and the other does
        // not. That is a rewritten skin — the candidate either dropped an
        // array, silently changing which bind the slot resolves to, or
        // materialized one the source never had — and it is reported as
        // missing evidence rather than passed for want of a comparison.
        let capability = complete_capability();

        let doc =
            compensated_document_with_unrelated_skin(Some(Mat4::from_scale(Vec3::splat(2.0))));
        let plan = compensated_rest_bind_plan(&doc, &capability);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let mut dropped = candidate.document().clone();
        dropped.assets.instances[1].skin_ibms.clear();
        assert_eq!(
            prove_scale(&doc, &ScaleCandidate { document: dropped }, &plan).unwrap_err(),
            ScaleError::MissingProofEvidence {
                kind: ProofResidualKind::UnaffectedInverseBind,
                detail: "candidate_slot_bind_missing",
            }
        );

        let bare = compensated_document_with_unrelated_skin(None);
        let bare_plan = compensated_rest_bind_plan(&bare, &capability);
        let bare_candidate = build_scale_candidate(&bare, &bare_plan).unwrap();
        let mut invented = bare_candidate.document().clone();
        invented.assets.instances[1].skin_ibms = vec![Mat4::from_scale(Vec3::splat(2.0))];
        assert_eq!(
            prove_scale(&bare, &ScaleCandidate { document: invented }, &bare_plan).unwrap_err(),
            ScaleError::MissingProofEvidence {
                kind: ProofResidualKind::UnaffectedInverseBind,
                detail: "source_slot_bind_missing",
            }
        );
    }

    #[test]
    fn a_whole_document_plan_replayed_against_an_extra_bone_still_holds_its_binds_to_the_factor() {
        // For a whole-document plan and the document it was planned against,
        // no instance can reach the unaffected-bind obligation with a slot to
        // check: `plan_whole_document` marks every bone affected and
        // `validate_scene_assets` rejects an out-of-range `skin_joints`
        // entry, so an unaffected instance has no joints at all.
        //
        // Replaying the plan against a *different* document — which
        // `prove_scale` explicitly permits, and guards against everywhere
        // else — does reach it. The expectation there is the conversion's,
        // not "unchanged": `B' = U B U^-1` scales the translation column by
        // `q` and leaves the linear part alone.
        //
        //   source bind translation (100, 0, 0), q = 0.01
        //   candidate bind translation (1, 0, 0) = the built candidate's
        //   expected translation (1, 0, 0), residual 0
        //
        // A zero residual is also this field's *initialized* value, so the
        // honest run below cannot by itself tell "checked, and it matched"
        // from "skipped": deleting this obligation's whole-document branch
        // leaves every assertion in it standing. The second run therefore
        // perturbs the candidate's translation *inside* the tolerance this
        // comparison derives for itself and pins the reported maximum:
        //
        //   candidate bind translation (1 + 2^-18, 0, 0)
        //   residual   2^-18 = 3.8147e-6, exact: `1 + 2^-18` is exact in
        //              binary32 (`ulp(1) = 2^-23`) and so is the difference
        //   tolerance  1e-6 + 1e-5 * max(1, 1 + 2^-18) = 1.1000038e-5,
        //              roughly `2.9x` the residual — a pass with headroom.
        let capability = complete_capability();
        let planned = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        let plan = plan_scale(&ScaleRequest {
            operation: ScaleOperation::WholeDocumentLinearUnits { factor: 0.01 },
            document: &planned,
            capability: &capability,
        })
        .unwrap();
        assert_eq!(plan.affected_nodes(), &[0, 1]);

        let mut wider = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        assert_eq!(
            push_unrelated_skin(
                &mut wider,
                Some(Mat4::from_translation(Vec3::new(100.0, 0.0, 0.0)))
            ),
            2
        );
        let candidate = build_scale_candidate(&wider, &plan).unwrap();
        assert_eq!(
            candidate.document().assets.instances[1].skin_ibms[0],
            Mat4::from_translation(Vec3::new(1.0, 0.0, 0.0))
        );
        let proof = prove_scale(&wider, &candidate, &plan).unwrap();
        assert_eq!(proof.unaffected_inverse_bind_residual, 0.0);

        let mut nudged = candidate.document().clone();
        nudged.assets.instances[1].skin_ibms[0] =
            Mat4::from_translation(Vec3::new(1.0 + 2f32.powi(-18), 0.0, 0.0));
        let proof = prove_scale(&wider, &ScaleCandidate { document: nudged }, &plan).unwrap();
        assert_eq!(proof.unaffected_inverse_bind_residual, 2f64.powi(-18));
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

        let cases: [StructureMismatchCase; 13] = [
            // The extra bone is a parentless copy of the root, so it passes
            // `validate_document_shape` (which runs first) and reaches the
            // parity clause rather than a shape guard. This is the row the
            // sampling budget depends on: `per_sample_work_units` measures
            // only the source skeleton while `sample_time_obligations` poses
            // both, so an unrejected candidate skeleton is unbilled work.
            ("bone_count_mismatch", |d| {
                let root = d.skeleton.bones[0].clone();
                d.skeleton.bones.push(root);
            }),
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
            // The placement half of "unchanged mesh/material/skin identity":
            // the instance is re-parented onto the root, and re-pointed at
            // the root's source node, one field at a time. Bone 0 and source
            // node 0 both exist, so neither doctoring can be caught by a
            // range check standing in for the parity clause.
            ("instance_node_mismatch", |d| d.assets.instances[0].node = 0),
            ("instance_source_node_index_mismatch", |d| {
                d.assets.instances[0].source_node_index = 0
            }),
            // "Unchanged mesh/material/skin identity" (DESIGN.md Appendix D
            // §D.6). The extra mesh keeps `instance.mesh = 1` in range, so
            // this reaches the parity check rather than the document-shape
            // guard; the mesh-count clause is checked *after* it, so the
            // reason below is still attributable to the instance clause.
            ("instance_mesh_mismatch", |d| {
                let extra = d.assets.meshes[0].clone();
                d.assets.meshes.push(extra);
                d.assets.instances[0].mesh = 1;
            }),
            ("instance_skin_joints_mismatch", |d| {
                d.assets.instances[0].skin_joints = vec![0];
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

    // --- Mesh-instance placement identity (issue #307) --------------------

    #[test]
    fn a_candidate_that_relocates_a_mesh_instance_is_refused_field_by_field() {
        // Placement is the half of "unchanged mesh/material/skin identity"
        // no residual can reach. The rig is three bones deliberately: the one
        // instance hangs off the *middle* one and names source node 1, so
        // every field can be doctored both down (to 0) and up (to 2) while
        // staying in range. An ordering comparison rather than a parity one
        // would refuse one direction and let the other through.
        let doc = rig_document(
            &[
                rig(None, 0, Vec3::ZERO),
                rig(Some(0), 1, Vec3::new(0.0, 1.0, 0.0)),
                rig(Some(1), 2, Vec3::new(0.0, 1.0, 0.0)),
            ],
            &[1],
            0,
            Mat4::IDENTITY,
        );
        assert_eq!(doc.skeleton.bones.len(), 3);
        assert_eq!(doc.assets.instances[0].node, 1);
        assert_eq!(doc.assets.instances[0].source_node_index, 1);
        let capability = complete_capability();
        let plan = whole_document_plan(&doc, &capability);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        // The honest candidate proves, so each refusal below is attributable
        // to its doctoring rather than to a fixture that never proved.
        prove_scale(&doc, &candidate, &plan).unwrap();

        let refuse = |doctor: &dyn Fn(&mut MeshInstance)| {
            let mut broken = candidate.document().clone();
            doctor(&mut broken.assets.instances[0]);
            prove_scale(&doc, &ScaleCandidate { document: broken }, &plan).unwrap_err()
        };

        // `node` alone: the bone the instance hangs off, which is what the
        // glTF writer attaches it to and what measurement poses it with.
        // Both directions, because either one alone is a relocation.
        for doctored in [0, 2] {
            assert_eq!(
                refuse(&|instance| instance.node = doctored),
                ScaleError::CandidateStructureMismatch {
                    reason: "instance_node_mismatch"
                },
                "moving the instance onto bone {doctored} must be refused"
            );
        }
        // `source_node_index` alone: what `instance_source_skin` matches a
        // source skin's attachments against, and so what decides whether a
        // missing bind is glTF's format-defined identity default. Again both
        // directions.
        for doctored in [0, 2] {
            assert_eq!(
                refuse(&|instance| instance.source_node_index = doctored),
                ScaleError::CandidateStructureMismatch {
                    reason: "instance_source_node_index_mismatch"
                },
                "re-pointing the instance at source node {doctored} must be refused"
            );
        }
        // Both together — the whole-document construction that used to
        // return `Ok(0.0)`. The `node` clause is checked first, so that is
        // the reason reported.
        assert_eq!(
            refuse(&|instance| {
                instance.node = 0;
                instance.source_node_index = 0;
            }),
            ScaleError::CandidateStructureMismatch {
                reason: "instance_node_mismatch"
            }
        );
    }

    #[test]
    fn a_candidate_that_swaps_two_payload_identical_instances_is_refused() {
        // Two instances drawing the same mesh, bound to the same joints,
        // with the same stored binds, differing only in where they hang.
        // Swapping them leaves every payload comparison this module makes —
        // mesh positions, skin matrices, bounds, stored binds — reading
        // exactly the values it read before, so only positional identity
        // can catch it.
        let mut doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        let twin = MeshInstance {
            source_node_index: 0,
            node: 0,
            ..doc.assets.instances[0].clone()
        };
        doc.assets.instances.push(twin);
        let (first, second) = (&doc.assets.instances[0], &doc.assets.instances[1]);
        assert_eq!(first.mesh, second.mesh);
        assert_eq!(first.skin_joints, second.skin_joints);
        assert_eq!(first.skin_ibms, second.skin_ibms);
        assert_ne!(first.node, second.node);

        let capability = complete_capability();
        let plan = whole_document_plan(&doc, &capability);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        prove_scale(&doc, &candidate, &plan).unwrap();

        let mut swapped = candidate.document().clone();
        swapped.assets.instances.swap(0, 1);
        assert_eq!(
            prove_scale(&doc, &ScaleCandidate { document: swapped }, &plan).unwrap_err(),
            ScaleError::CandidateStructureMismatch {
                reason: "instance_node_mismatch"
            }
        );
    }

    /// `compensated_document` plus an **unskinned** prop hanging off a new
    /// parentless bone at world `x = 5`, outside the affected closure.
    ///
    /// Unskinned is what makes this the sharp fixture. Skinning ignores an
    /// instance's node transform — a skinned instance deforms identically
    /// wherever it is attached — so only for an unskinned prop does `node`
    /// alone decide where its geometry lands, given the bone it names.
    fn compensated_document_with_unskinned_prop() -> (Document, BoneId) {
        let mut doc = compensated_document();
        let bone = doc.skeleton.bones.len();
        let source_node_index = doc
            .assets
            .source_skeleton
            .nodes
            .iter()
            .map(|node| node.source_node_index)
            .max()
            .expect("the base document projects at least one node")
            + 1;
        let rest = Transform {
            translation: Vec3::new(5.0, 0.0, 0.0),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        };
        doc.skeleton.bones.push(Bone {
            name: "prop".into(),
            parent: None,
            rest,
            inverse_bind: None,
        });
        doc.assets.source_skeleton.nodes.push(SourceNodeAsset {
            source_node_index,
            name: None,
            parent_source_node_index: None,
            scene_root_indices: vec![0],
            local_rest: SourceNodeLocalRest::Trs {
                translation: rest.translation,
                rotation: rest.rotation,
                scale: rest.scale,
            },
            bone: Some(bone),
        });
        doc.assets.meshes.push(MeshAsset {
            name: "prop".into(),
            source_mesh_index: 1,
            primitives: vec![Primitive {
                positions: vec![Vec3::new(1.0, 0.0, 0.0)],
                ..Primitive::default()
            }],
        });
        let mesh = doc.assets.meshes.len() - 1;
        doc.assets.instances.push(MeshInstance {
            source_node_index,
            node: bone,
            mesh,
            skin_joints: Vec::new(),
            skin_ibms: Vec::new(),
        });
        (doc, bone)
    }

    #[test]
    fn a_rest_bind_candidate_that_relocates_an_unskinned_prop_is_refused() {
        let (doc, prop) = compensated_document_with_unskinned_prop();
        assert_eq!(prop, 3);
        let capability = complete_capability();
        let plan = compensated_rest_bind_plan(&doc, &capability);
        // Bone 3 is a second scene root, so it is outside the closure and
        // untouched; bone 2 is the transform-only attachment *inside* it,
        // rebased from x = 1 to x = 0.01. Moving the prop from one to the
        // other is a real relocation, from world x = 5 onto a rebased joint.
        assert_eq!(plan.affected_nodes(), &[0, 1, 2]);
        assert_eq!(doc.assets.instances[1].node, prop);
        assert!(doc.assets.instances[1].skin_joints.is_empty());

        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let proof = prove_scale(&doc, &candidate, &plan).unwrap();
        // Nothing the proof measures reads the prop's node, so the relocated
        // candidate below produces exactly these residuals: every one of them
        // zero, on a document whose prop has moved five units.
        assert_eq!(proof.rest_translation_residual, 0.0);
        assert_eq!(proof.rest_rotation_residual, 0.0);
        assert_eq!(proof.mesh_position_residual, 0.0);
        assert_eq!(proof.unaffected_inverse_bind_residual, 0.0);

        let mut relocated = candidate.document().clone();
        relocated.assets.instances[1].node = 2;
        assert_eq!(
            relocated.assets.instances[1].source_node_index,
            candidate.document().assets.instances[1].source_node_index,
            "only the node placing the prop may differ"
        );
        assert_eq!(
            prove_scale(
                &doc,
                &ScaleCandidate {
                    document: relocated
                },
                &plan
            )
            .unwrap_err(),
            ScaleError::CandidateStructureMismatch {
                reason: "instance_node_mismatch"
            }
        );
    }

    #[test]
    fn a_mesh_instance_node_outside_the_skeleton_is_refused_on_either_document() {
        // `node` is resolved downstream without a second bounds test, so an
        // index past the end of the skeleton has to be refused here or it
        // reaches the writer. Doctored on the *second* of two instances, so
        // the check is proved to walk the whole list and to report the slot
        // that actually carries the bad index rather than a fixed 0.
        let mut doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        let twin = MeshInstance {
            source_node_index: 0,
            node: 0,
            ..doc.assets.instances[0].clone()
        };
        doc.assets.instances.push(twin);
        let capability = complete_capability();
        let plan = whole_document_plan(&doc, &capability);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let expected = ScaleError::InvalidMeshInstance {
            instance_index: 1,
            reason: "node_index_out_of_range",
        };

        let mut broken_source = doc.clone();
        broken_source.assets.instances[1].node = doc.skeleton.bones.len();
        assert_eq!(
            plan_scale(&ScaleRequest {
                operation: ScaleOperation::WholeDocumentLinearUnits { factor: 0.01 },
                document: &broken_source,
                capability: &capability,
            })
            .unwrap_err(),
            expected
        );
        assert_eq!(
            prove_scale(&broken_source, &candidate, &plan).unwrap_err(),
            expected
        );

        let mut broken_candidate = candidate.document().clone();
        broken_candidate.assets.instances[1].node = doc.skeleton.bones.len();
        assert_eq!(
            prove_scale(
                &doc,
                &ScaleCandidate {
                    document: broken_candidate
                },
                &plan
            )
            .unwrap_err(),
            expected
        );
    }

    // --- Per-residual comparison counts (issue #319) ----------------------

    /// Every residual's comparison count, named, in the order [`ScaleProof`]
    /// declares them.
    ///
    /// Named rather than a bare `[usize; 12]` so a failing vector says which
    /// residual moved, and taken as a whole rather than one field per
    /// assertion so a document's coverage is stated as one hand-derived fact
    /// — including the zeros, which are the half a per-field assertion tends
    /// to leave out.
    fn comparison_counts(proof: &ScaleProof) -> [(&'static str, usize); 12] {
        [
            ("rest_translation", proof.rest_translation_comparisons),
            ("rest_rotation", proof.rest_rotation_comparisons),
            ("unit_scale", proof.unit_scale_comparisons),
            (
                "transform_only_affine",
                proof.transform_only_affine_comparisons,
            ),
            ("track_value", proof.track_value_comparisons),
            ("mesh_position", proof.mesh_position_comparisons),
            ("key_translation", proof.key_translation_comparisons),
            ("cubic_interior", proof.cubic_interior_comparisons),
            ("trajectory", proof.trajectory_comparisons),
            ("skin_matrix", proof.skin_matrix_comparisons),
            ("bounds", proof.bounds_comparisons),
            (
                "unaffected_inverse_bind",
                proof.unaffected_inverse_bind_comparisons,
            ),
        ]
    }

    /// The one binary32 rounding a `0.01` factor costs at a coordinate of
    /// `1.0`, which is the largest source magnitude in the fixtures below
    /// whose product with `0.01` is not exactly representable:
    ///
    /// ```text
    /// f32(0.01) = 0.00999999977648258209228515625
    /// f64(0.01) = 0.010000000000000000208166817117216851...
    /// difference  2.2351741811588166e-10
    /// ```
    ///
    /// Every builder in this module narrows to `f32` exactly once per
    /// element, and every proof-side expectation is formed in `f64` from the
    /// unrounded source, so this is the residual a *correct* candidate
    /// carries — not a defect, and not something a rest/bind plan can
    /// produce, whose expectation for these domains is `before` exactly.
    const NARROWING_AT_ONE: f64 = 2.2351741811588166e-10;

    /// `payload_document` plus an affected **cubic** translation track, so
    /// one whole-document plan declares all five sampled obligations at once
    /// and every domain a whole-document conversion rewrites is measured.
    ///
    /// The track is deliberately constant with zero tangents: at `u = 0.5`
    /// the glTF Hermite basis is `h00 = h01 = 0.5` and `h10 = h11 = 0`, so
    /// the interior sample is `0.5 * p + 0.5 * p = p` **exactly** on both
    /// sides, in `f32`, and the interior-time residual is the same single
    /// narrowing as the key-time one rather than an accumulation of spline
    /// arithmetic this test would have to re-derive.
    fn sampled_payload_document() -> Document {
        let mut doc = payload_document();
        doc.clips[0].tracks.push(Track {
            bone: 1,
            property: Property::Translation,
            interpolation: Interpolation::CubicSpline,
            times: vec![0.0, 1.0],
            values: TrackValues::Vec3s(vec![
                Vec3::ZERO,               // in-tangent @0
                Vec3::new(0.0, 1.0, 0.0), // value @0
                Vec3::ZERO,               // out-tangent @0
                Vec3::ZERO,               // in-tangent @1
                Vec3::new(0.0, 1.0, 0.0), // value @1
                Vec3::ZERO,               // out-tangent @1
            ]),
        });
        doc
    }

    #[test]
    fn a_whole_document_proof_counts_and_measures_every_comparison_it_makes() {
        let doc = sampled_payload_document();
        let capability = complete_capability();
        let plan = whole_document_plan(&doc, &capability);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let proof = prove_scale(&doc, &candidate, &plan).unwrap();

        // Two bones, three mesh vertices, one skin slot; a rotation track and
        // a cubic translation track sharing key times `{0, 1}`, whose single
        // cubic segment contributes the interior time `0.5`.
        assert_eq!(proof.sample_time_count, 3);
        assert_eq!(
            comparison_counts(&proof),
            [
                // One per affected node, and a whole-document closure is
                // every bone.
                ("rest_translation", 2),
                ("rest_rotation", 2),
                // A whole-document plan declares neither the rest/bind
                // postcondition nor a transform-only attachment, so both
                // report the `0.0` that only a count distinguishes from a
                // measured one.
                ("unit_scale", 0),
                ("transform_only_affine", 0),
                // Two rotation values, plus the cubic track's six elements
                // (two values and four tangents).
                ("track_value", 8),
                ("mesh_position", 3),
                // One affected translation track, read at each of the two
                // key times and at the one interior time.
                ("key_translation", 2),
                ("cubic_interior", 1),
                // Both affected nodes, posed at each of the three sample
                // times.
                ("trajectory", 6),
                // One skin slot, at rest and at each of the three sample
                // times. The rest pose is what makes this four rather than
                // three.
                ("skin_matrix", 4),
                // Six per pose: three axes of each of the two extreme
                // corners.
                ("bounds", 24),
                // The document's only skinned instance is bound to bone 1,
                // which a whole-document closure contains.
                ("unaffected_inverse_bind", 0),
            ]
        );

        // Hand-computed residuals, from the fixture's own coordinates. Each
        // is the narrowing above at the magnitude that comparison reads.
        //
        // Bone 1 sits at `(0, 1, 0)`; the candidate holds `(0, f32(0.01), 0)`
        // against an `f64` expectation of `(0, 0.01, 0)`, and a vector with
        // one non-zero component has that component's magnitude for a length.
        assert_eq!(proof.rest_translation_residual, NARROWING_AT_ONE);
        // The same node, posed through the clip: its own rotation track does
        // not move its translation column, and the translation track samples
        // to the same `(0, 1, 0)` at every one of the three times.
        assert_eq!(proof.trajectory_residual, NARROWING_AT_ONE);
        // The track's own elements: the four tangents are `0` (exact under
        // any factor) and both values are `(0, 1, 0)`.
        assert_eq!(proof.track_value_residual, NARROWING_AT_ONE);
        assert_eq!(proof.key_translation_residual, NARROWING_AT_ONE);
        assert_eq!(proof.cubic_interior_residual, NARROWING_AT_ONE);
        // `mesh_position` is a per-vertex L2 norm, and this rig's extreme
        // vertices are `(1, 1, 1)` and `(-1, -1, -1)`: three components each
        // carrying the same narrowing, so `sqrt(3 * e^2)`.
        assert_eq!(proof.mesh_position_residual, 3.871435245533232e-10);
        // Bounds are per axis, and the skinned maximum is at `y = 2`: the
        // vertex `(1, 1, 1)` skinned through `W = translate(0, 1, 0)`. Both
        // `0.02` and `2 * f32(0.01)` are exact doublings of their `y = 1`
        // counterparts, so the residual there is exactly twice the
        // narrowing — which is also what makes this a maximum over the six
        // axis comparisons rather than the only non-zero one.
        assert_eq!(proof.bounds_residual, 2.0 * NARROWING_AT_ONE);
        // A *measured* zero, and the reason the counts exist. The skin
        // equation's expectation is `scale_translation_only(W * B, f32(q))`
        // and the candidate composes `W' * B'` out of the same two `f32`
        // products, so the two sides agree bit for bit — an exact zero that
        // `skin_matrix_comparisons = 4` distinguishes from the four residuals
        // above it that nothing walked.
        assert_eq!(proof.skin_matrix_residual, 0.0);
        // Likewise exact, and structurally so: neither builder writes
        // `Bone::rest.rotation`, and `quat_equality_residual` of a
        // bit-identical pair is `0.0` by construction. No correct candidate
        // can make this one non-zero; `a_genuinely_rewritten_rest_rotation
        // _still_fails_proof` covers the incorrect ones.
        assert_eq!(proof.rest_rotation_residual, 0.0);
    }

    #[test]
    fn an_unanimated_skinned_document_still_compares_its_skin_at_rest() {
        // The permanent pin for what #317 was closed over: `prove_skin` and
        // `prove_bounds` arm a **rest-pose** walk as well as the sampled
        // loop, so a skinned document with no clips at all still compares its
        // skin matrices and its bounds. Gating those two obligations on the
        // presence of sample times — which that issue proposed — would leave
        // this document's only check unperformed while still publishing a
        // `0.0` for it.
        let doc = compensated_document();
        assert!(doc.clips.is_empty());
        let capability = complete_capability();
        let plan = compensated_rest_bind_plan(&doc, &capability);
        assert_eq!(plan.affected_nodes(), &[0, 1, 2]);
        assert_eq!(plan.transform_only_attachments(), &[2]);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let proof = prove_scale(&doc, &candidate, &plan).unwrap();

        assert_eq!(proof.sample_time_count, 0);
        assert_eq!(
            comparison_counts(&proof),
            [
                ("rest_translation", 3),
                ("rest_rotation", 3),
                // Rest/bind declares the postcondition, over the same three
                // nodes.
                ("unit_scale", 3),
                // One probe point through node 2's expected world affine.
                ("transform_only_affine", 1),
                // No clips, so nothing to compare element-wise and no sample
                // time to pose either skeleton at.
                ("track_value", 0),
                ("mesh_position", 1),
                ("key_translation", 0),
                ("cubic_interior", 0),
                ("trajectory", 0),
                // The whole point: one skin slot and one bounding box,
                // compared at rest, with no sample time in the document.
                ("skin_matrix", 1),
                ("bounds", 6),
                // The document's only skin is inside the closure.
                ("unaffected_inverse_bind", 0),
            ]
        );
    }

    #[test]
    fn a_rest_bind_proof_counts_the_comparisons_its_own_obligations_walk() {
        let doc = multi_joint_document();
        let capability = complete_capability();
        let plan = multi_joint_plan(&doc, &capability);
        assert_eq!(plan.affected_nodes(), &[0, 1, 2]);
        // Both non-root nodes are skin joints, so the closure holds no
        // transform-only attachment and that obligation is not declared.
        assert!(plan.transform_only_attachments().is_empty());
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let proof = prove_scale(&doc, &candidate, &plan).unwrap();

        // Key times `{0, 1}` shared by both tracks, plus the cubic track's
        // one interior time `0.5`.
        assert_eq!(proof.sample_time_count, 3);
        assert_eq!(
            comparison_counts(&proof),
            [
                ("rest_translation", 3),
                ("rest_rotation", 3),
                ("unit_scale", 3),
                ("transform_only_affine", 0),
                // Two linear translation values plus six cubic elements.
                ("track_value", 8),
                ("mesh_position", 4),
                // Two affected translation tracks, at each of the two key
                // times and at the one interior time.
                ("key_translation", 4),
                ("cubic_interior", 2),
                // Three affected nodes at each of the three sample times.
                ("trajectory", 9),
                // Two skin slots, at rest and at each of the three sample
                // times.
                ("skin_matrix", 8),
                ("bounds", 24),
                ("unaffected_inverse_bind", 0),
            ]
        );
    }

    #[test]
    fn a_clipless_plan_compares_no_track_value_while_still_comparing_its_mesh() {
        // The `track_value` half of the pair no obligation flag can gate.
        // This document carries a mesh and a skin but no clip, so the
        // per-element track comparison has nothing to walk while every other
        // unconditional comparison still does — which is what separates
        // `track_value`'s count from `mesh_position`'s rather than leaving
        // both to move together.
        let doc = rig_document(&unit_rig(), &[1], 0, Mat4::IDENTITY);
        assert!(doc.clips.is_empty());
        let capability = complete_capability();
        let plan = whole_document_plan(&doc, &capability);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let proof = prove_scale(&doc, &candidate, &plan).unwrap();

        assert_eq!(proof.sample_time_count, 0);
        assert_eq!(
            comparison_counts(&proof),
            [
                ("rest_translation", 2),
                ("rest_rotation", 2),
                ("unit_scale", 0),
                ("transform_only_affine", 0),
                ("track_value", 0),
                ("mesh_position", 1),
                ("key_translation", 0),
                ("cubic_interior", 0),
                ("trajectory", 0),
                ("skin_matrix", 1),
                ("bounds", 6),
                ("unaffected_inverse_bind", 0),
            ]
        );
    }

    #[test]
    fn a_meshless_plan_compares_no_mesh_position_while_still_comparing_its_tracks() {
        // The other half: clips but no mesh. The two documents together are
        // what make `track_value` and `mesh_position` independently
        // observable — on every other fixture in this module they are either
        // both walked or both empty.
        let mut doc = payload_document();
        doc.assets.meshes.clear();
        doc.assets.instances.clear();
        let capability = complete_capability();
        let plan = whole_document_plan(&doc, &capability);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let proof = prove_scale(&doc, &candidate, &plan).unwrap();

        assert_eq!(proof.sample_time_count, 2);
        assert_eq!(
            comparison_counts(&proof),
            [
                ("rest_translation", 2),
                ("rest_rotation", 2),
                ("unit_scale", 0),
                ("transform_only_affine", 0),
                // The rotation track's two values.
                ("track_value", 2),
                ("mesh_position", 0),
                // A rotation track evidences trajectories but neither key
                // translations nor cubic interiors.
                ("key_translation", 0),
                ("cubic_interior", 0),
                ("trajectory", 4),
                // With the instance gone there is no skinned evidence, so
                // neither obligation is declared.
                ("skin_matrix", 0),
                ("bounds", 0),
                ("unaffected_inverse_bind", 0),
            ]
        );
    }

    #[test]
    fn a_skin_outside_the_closure_is_the_only_source_of_an_unaffected_bind_comparison() {
        // The third residual no obligation flag gates, and the one whose
        // producer-side predicate used to hand-mirror `stored_instance_bind`'s
        // private resolution order across a crate boundary. Counting at the
        // point of comparison is what removes that mirror: this document's
        // second instance is bound to bone 3, outside the closure, and stores
        // a bind on both sides.
        let doc =
            compensated_document_with_unrelated_skin(Some(Mat4::from_scale(Vec3::splat(2.0))));
        let capability = complete_capability();
        let plan = compensated_rest_bind_plan(&doc, &capability);
        assert_eq!(plan.affected_nodes(), &[0, 1, 2]);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let proof = prove_scale(&doc, &candidate, &plan).unwrap();

        // One slot of one instance outside the closure.
        assert_eq!(proof.unaffected_inverse_bind_comparisons, 1);
        // A measured zero: rest/bind must leave an unrelated skin's stored
        // bind exactly as it found it, so the correct candidate's residual
        // here is `0.0` — indistinguishable, without the count, from the `0.0`
        // `compensated_document` reports for having no such skin at all.
        assert_eq!(proof.unaffected_inverse_bind_residual, 0.0);
        // The extra bone is outside the closure, so the rest walk is
        // unchanged, and the extra instance is not the skin obligation's.
        assert_eq!(proof.rest_translation_comparisons, 3);
        assert_eq!(proof.skin_matrix_comparisons, 1);
    }

    #[test]
    fn an_unskinned_document_compares_neither_skin_matrices_nor_bounds() {
        let doc = unskinned_document();
        let capability = complete_capability();
        let plan = whole_document_plan(&doc, &capability);
        assert!(!plan.proof_obligations().prove_skin);
        assert!(!plan.proof_obligations().prove_bounds);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let proof = prove_scale(&doc, &candidate, &plan).unwrap();

        assert_eq!(
            comparison_counts(&proof),
            [
                ("rest_translation", 1),
                ("rest_rotation", 1),
                ("unit_scale", 0),
                ("transform_only_affine", 0),
                ("track_value", 0),
                // The unskinned instance's base positions are still proved
                // directly, which is the claim `prove_bounds` cannot make.
                ("mesh_position", 1),
                ("key_translation", 0),
                ("cubic_interior", 0),
                ("trajectory", 0),
                ("skin_matrix", 0),
                ("bounds", 0),
                ("unaffected_inverse_bind", 0),
            ]
        );
    }

    #[test]
    fn a_boneless_whole_document_plan_compares_nothing_at_all() {
        // The zero row of the matrix, all twelve at once: an empty document
        // plans an empty closure, declares no obligation, and carries no
        // payload for the three ungated comparisons to walk. Every residual
        // it publishes is `0.0`, and every count says why.
        let doc = Document::default();
        let capability = complete_capability();
        let plan = whole_document_plan(&doc, &capability);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();
        let proof = prove_scale(&doc, &candidate, &plan).unwrap();

        assert_eq!(proof.sample_time_count, 0);
        assert_eq!(
            comparison_counts(&proof).map(|(_, count)| count),
            [0usize; 12]
        );
    }

    // --- The unit-scale postcondition's own gate (issue #316) -------------

    #[test]
    fn a_plan_declaring_the_unit_scale_postcondition_without_the_rest_walk_is_refused() {
        // The postcondition is evaluated inside the `prove_rest` walk, so
        // this combination would prove `Ok` and publish `unit_scale_residual:
        // 0.0` having compared nothing. Not reachable through `plan_scale` —
        // `ScalePlan`'s fields are private and neither planner emits it — so
        // it is constructed here, in-crate, exactly as the boneless
        // rest-obligation case above is.
        let doc = compensated_document();
        let capability = complete_capability();
        let plan = compensated_rest_bind_plan(&doc, &capability);
        assert!(plan.proof_obligations().prove_rest);
        assert!(plan.proof_obligations().prove_unit_scale_postcondition);
        let candidate = build_scale_candidate(&doc, &plan).unwrap();

        let declared = ScalePlan {
            operation: plan.operation(),
            tolerance_policy: plan.tolerance_policy(),
            affected_nodes: plan.affected_nodes().to_vec(),
            transform_only_attachments: plan.transform_only_attachments().to_vec(),
            common_factor: plan.common_factor(),
            observed_factor: plan.observed_factor(),
            domain_rewrites: plan.domain_rewrites(),
            proof_obligations: ScaleProofObligations {
                prove_rest: false,
                prove_unit_scale_postcondition: true,
                ..plan.proof_obligations()
            },
        };
        let error = prove_scale(&doc, &candidate, &declared).unwrap_err();
        let ScaleError::MissingProofEvidence { kind, detail } = error else {
            panic!("expected a missing unit-scale gate, got {error:?}");
        };
        assert_eq!(kind, ProofResidualKind::UnitScale);
        assert_eq!(detail, "unit_scale_postcondition_without_rest");

        // The guard reads both flags, not just the postcondition: the same
        // plan with `prove_rest` restored is the one `plan_scale` emits and
        // still proves.
        prove_scale(&doc, &candidate, &plan).unwrap();
    }
}
