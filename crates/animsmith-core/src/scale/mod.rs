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
//! factor. An internal reference builder constructs analytic candidates for
//! fixtures and calibration; production format frontends instead rewrite
//! exact source bytes and wrap the emitted reload with
//! [`ScaleCandidate::from_document`]. [`prove_scale`] independently re-derives
//! the plan's claims from the source and candidate documents and reports the observed
//! residual maxima against the fixed [`ScaleTolerancePolicy::APPENDIX_D_V6`]
//! tolerance identity.
//!
//! Those residuals are the producer evidence record of §D.6, which is why
//! two properties of this module are contracts rather than implementation
//! details. Every typed [`ScaleProofObligation`] is declared only when
//! the planned document carries the evidence for it. Candidate construction
//! and proof re-derive that structural inventory and report a stale plan as
//! [`ScaleError::PlanDocumentMismatch`]; a counterpart missing inside an
//! inventory-matched walk is [`ScaleError::MissingProofEvidence`]. Neither
//! case becomes a zero residual — a record asserting `0.0` for something
//! nothing checked would be false, not merely incomplete. And the two
//! independent observed-factor witnesses
//! §D.6 asks for are both recorded, together with
//! [`ScaleProof::observed_factor_divergence`] between them, so the record
//! states the relationship between its own two measurements instead of
//! leaving a consumer to guess which to trust.

use crate::model::{
    AffineDomainViolation, BoneId, Clip, Document, DocumentShapeError, Interpolation,
    MeshInstanceShapeViolation, Primitive, Property, Skeleton, SourceInverseBindAccessorStatus,
    SourceNodeAsset, SourceNodeLocalRest, SourceSkeletonCoverage, TrackValues, Transform,
    affine_axis_lengths, average_affine_axis_length, mat4_is_finite,
};
use crate::sample::{TrackSample, sample_track};
use glam::{DMat3, DMat4, DVec3, Mat3, Mat4, Quat, Vec3, Vec4};
use std::collections::{BTreeMap, BTreeSet};

mod numeric;
mod planning;
mod reference;
mod validation;

pub use planning::plan_scale;
pub use reference::ScaleCandidate;

use numeric::{
    column_operand_magnitude, mat4_abs, matrix_magnitude, matrix_residual,
    product_operand_magnitude, scale_translation_only,
};
#[cfg(test)]
use numeric::{largest_entry, translation_composition_rounding_base};
#[cfg(test)]
use planning::classify_affine;
use planning::{check_factor_narrows, validate_plan_document_inventory};
#[cfg(any(test, feature = "fixtures"))]
pub(crate) use reference::build_scale_candidate;
#[cfg(test)]
use reference::{build_rest_bind, build_whole_document};
use validation::{
    WorldBonePose, WorldPose, affected_skin_instance_indices, child_translation_rounding_magnitude,
    instance_bind, local_rest_matrix, rest_world_pose, source_node_index_map, stored_instance_bind,
    validate_candidate_structure, validate_scale_input,
};
#[cfg(test)]
use validation::{
    affected_skin_classification_steps, derive_rest_bind_plan_domain,
    reset_affected_skin_classification_steps, resolve_rest_bind_skin, rest_bind_affected_closure,
    source_world_matrix, world_rests,
};

// --- Tolerance policy ----------------------------------------------------

/// Fixed Appendix D tolerance identity and thresholds. Classification and
/// proof share this one versioned policy and compute in `f64`, narrowing
/// only at the writer model boundary. There is exactly one supported
/// instance, [`ScaleTolerancePolicy::APPENDIX_D_V6`]: a policy change is a
/// new policy identity, not a runtime knob.
///
/// The superseded v5 identity is deliberately not retained as an alias:
///
/// ```compile_fail
/// use animsmith_core::ScaleTolerancePolicy;
///
/// let _ = ScaleTolerancePolicy::APPENDIX_D_V5;
/// ```
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
    /// [`Self::APPENDIX_D_V6`] for the composition argument and
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
    /// How many binary32 ulps of *operand* magnitude an obligation that
    /// compares `f32`-rounded arithmetic may deviate by, on top of
    /// [`Self::scalar_absolute`] and [`Self::scalar_relative`].
    ///
    /// The term this multiplies is **absolute**, not relative: it is
    /// `f32_rounding_ulps * magnitude * f32::EPSILON` where `magnitude` is
    /// the largest quantity the compared arithmetic passed through, not the
    /// quantity being compared. Where the compared value *is* that largest
    /// quantity the term adds `4 * 2^-23 = 4.77e-7` of it — twenty times
    /// less than [`Self::scalar_relative`] already allows — so it cannot
    /// loosen the obligations it applies to in their own regime.
    ///
    /// It exists for the regime where the two diverge. A rotation can make
    /// the compared quantity orders of magnitude smaller than the operands
    /// it was computed from while it still carries those operands' absolute
    /// rounding error: a bound component near zero on a mesh 4000 units
    /// across, a near-identity `W * B` whose translation column cancelled two
    /// 3190-magnitude terms, or a world translation whose parent chain
    /// cancelled two of them one composition earlier. A purely relative band
    /// is then derived from the small number and the error from the large
    /// one, and [`prove_scale`] refuses a correct candidate that
    /// [`plan_scale`] accepted. See [`Self::APPENDIX_D_V6`] for the
    /// measurement this count comes from and DESIGN.md Appendix D §D.1 for
    /// which magnitude each obligation takes it from.
    ///
    /// The count is only as meaningful as that magnitude. Two revisions of
    /// this policy have now found the *base* wrong rather than the count too
    /// small — first the skinned extent alone, which missed the `W * B`
    /// composition, then `abs(W) * abs(B)` alone, which missed what `W`'s own
    /// parent chain had already cancelled — and in both the measured excess
    /// was hundreds of thousands of ulps, not a factor of two. A residual
    /// above this count is evidence about the base before it is evidence
    /// about the count.
    pub f32_rounding_ulps: u32,
}

impl ScaleTolerancePolicy {
    /// How many [`Self::common_factor`] bands
    /// [`Self::postcondition_unit_scale_residual`] is derived from.
    ///
    /// Three of them are analytic and one is float headroom; see
    /// [`Self::APPENDIX_D_V6`].
    pub const UNIT_SCALE_BANDS: f64 = 4.0;

    /// The only supported tolerance policy: DESIGN.md Appendix D, version 6.
    ///
    /// Version 6 supersedes `appendix-d-v5`, which superseded v4, v3, v2 and
    /// v1. Each identity change is a change of *meaning*, not a retune:
    ///
    /// 1. [`Self::postcondition_unit_scale_residual`] is a per-axis
    ///    (L-infinity) residual derived from [`Self::common_factor`], instead
    ///    of v1's independently declared `1e-5` L2 norm over three axes. Under
    ///    v1 the two were incommensurable, and a source whose observed factor
    ///    had relative error `e` produced a postcondition residual of
    ///    `sqrt(3) * e`, so every `e` in `(5.77e-6, 1e-5]` was accepted by
    ///    [`plan_scale`] and then rejected by [`prove_scale`].
    /// 2. [`Self::proof_sample_work_budget`] bounds the sampled proof work a
    ///    document may demand.
    /// 3. [`Self::f32_rounding_ulps`] is new in v3, and adds an absolute
    ///    `f32`-rounding term to the five obligations that compare
    ///    `f32`-rounded arithmetic against a base that a rotation can make
    ///    arbitrarily smaller than the operands the arithmetic ran on —
    ///    [`ProofResidualKind::Bounds`], [`ProofResidualKind::SkinMatrix`],
    ///    [`ProofResidualKind::UnaffectedInverseBind`],
    ///    [`ProofResidualKind::RestTranslation`], and
    ///    [`ProofResidualKind::Trajectory`]. Without it [`plan_scale`] accepts
    ///    and [`prove_scale`] refuses a correct candidate whenever
    ///    `magnitude / component` is large.
    /// 4. v4 widens finite non-negative weight normalization and accumulation
    ///    to binary64, makes the Bounds magnitude a weight-proportional
    ///    combination of each influence's transform and slot-composition
    ///    provenance, and removes v3's blended-point L2 stage. Bounds residuals
    ///    are per axis, and the normalized blend is already bounded by those
    ///    weighted operands.
    /// 5. v5 makes parent-chain translation provenance additive per composed
    ///    link, in binary64, instead of taking a depth-independent maximum.
    ///    Each spatial row carries the new local contribution plus a parent
    ///    term capped by `contribution / EPSILON`. This provisions the smaller
    ///    of one parent-scale ulp and losing the entire contribution, so zero
    ///    and underflowed descendants cannot charge the same translated parent
    ///    repeatedly. Only the three spatial output rows participate; the
    ///    affine homogeneous row contributes zero under this cap. The same
    ///    recurrence constructs rest and sampled poses, and its result reaches
    ///    RestTranslation, Trajectory, SkinMatrix and Bounds through their
    ///    existing consumers.
    /// 6. v6 changes only the association of the shared affine axis-length
    ///    mean: the three finite widened lengths are sorted ascending before
    ///    the ordinary sum and division by three. This removes authored-column
    ///    order from the classifier, planning, and proof witness without
    ///    changing any numeric threshold, the v5 parent-chain provenance
    ///    recurrence, or the evidence schema.
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
    pub const APPENDIX_D_V6: Self = Self {
        id: "appendix-d-v6",
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
        // The value is a wall-time ceiling expressed in work units. Historical
        // v3 measurements were linear in the charge across four doublings
        // (`1e8` to `8e8`) in both named shapes, which establishes that the
        // charge is a proxy for real work rather than an arbitrary count. The
        // shapes were 200 instances of a 99-joint skin list with one vertex
        // each, and one instance of that list with a 10_000-vertex primitive,
        // each with as many sample times as `4e8` admits.
        //
        // On one developer machine under v3 those shapes measured `6.7s` and
        // `3.3s`. They are neither bounds nor v4 measurements: v4 removes the
        // per-vertex L2 stage and widens weighted accumulation to binary64, so
        // it deliberately does not carry the old ordering or attribution
        // forward. The budget remains a conservative work limit because the
        // charged passes and their cardinalities did not change. See DESIGN.md
        // Appendix D §D.1 for the historical measurements and this boundary.
        //
        // `1e8` — the first value this policy carried — was too tight, and
        // the arithmetic that justified it was the pre-both-sides charge. A
        // 200-bone rig with a 100k-vertex skinned mesh and a 30-second clip
        // at 30 fps costs `900 * 201_000 = 180_900_000`: a plausible
        // production asset, refused with no way for a caller to opt into the
        // work. At `4e8` it is admitted with `2.2x` headroom — about 66
        // seconds of animation on that rig.
        proof_sample_work_budget: 400_000_000,
        // Measured, not assumed, and the measurement is **checked in**:
        // `calibrate_f32_rounding_ulps` builds and proves 360_000 correct
        // candidates over 144 cells and asserts every figure below. Run it with
        //
        //   cargo test -p animsmith-core --release --lib \
        //       calibrate_f32_rounding_ulps -- --ignored --nocapture
        //
        // The cells are the cross product of nine operations — rest/bind at
        // root scale `3190`, and whole-document conversion at
        // `{1e-4, 0.01, 0.1, 1.5, 7.3, 100, 3190, 1e6}`, so both directions of
        // the factor — four slot compositions — analytic binds, where
        // `abs(W * B)` is `1`, and composed slots at
        // `abs(W * B) = {1e-3, 1, 1e3}` — two blends, and two weight profiles:
        // balanced, and a mismatched profile that gives each vertex's larger
        // production influence base a log-uniform weight in `[1e-20, 1e-2]`
        // while the smaller gets `1`. The latter realizes 274_670 mismatched
        // vertices, in both slot orientations. Joint locals and vertex
        // positions are drawn log-uniformly over six and eight decades in
        // random directions, every joint carries a random rotation, and half
        // of every cell's trials carry a parent chain that cancels.
        //
        // The quantity is `residual / (magnitude * 2^-23)`: the raw ulp count,
        // *not* net of the scalar band that is paid first. It therefore
        // overstates what this count is asked for, by the whole scalar band, so
        // a worst case under `4` measured this way is under `4` however the two
        // terms are split.
        //
        // The shallow population uses runtime trigonometry, so its maxima are
        // asserted inside broad cross-platform bounds rather than as exact
        // literals; no correct candidate is refused in any cell.
        //
        // A separate deep phase proves 80 correct animated candidates through
        // depth 512, including a literal 192-link closed loop. It forms each
        // demand from the residual and provenance of the same comparison,
        // rather than dividing two unrelated global maxima. Worst raw demands
        // are `0.715` for RestTranslation and Trajectory, `0.143` for
        // SkinMatrix, and `0.149` for Bounds, with no refusal.
        //
        // `UnaffectedInverseBind` demands `0`, and always will: its two sides
        // are the identical `f32` expression on identical stored inputs.
        //
        // What this replaced, and why it is a test now. Earlier revisions of
        // this comment quoted maxima from sweeps that were never checked in —
        // 2_390_000 candidates in four populations, then 5760 whole-document
        // conversions per factor — and one of those claims was false. It said
        // its population carried "binds that are not the rest pose, and blends
        // that cancel" with a worst `Bounds` demand of `0.92`. The shape it did
        // not carry is a composed slot with `abs(W * B) != 1`, and against the
        // base this policy shipped at the time that shape refuses correct
        // candidates in fifteen of the pre-v4 sweep's seventy-two cells and
        // demands `47.7`. A figure a reader cannot re-derive is a figure nobody
        // can check.
        //
        // `4` is the next power of two above every figure above, **measured
        // over that population**. It is not an analytic bound. An earlier
        // revision of this comment said it was — "the analytic worst case for
        // the arithmetic involved, since composing `W * B` accumulates a
        // four-term inner product per entry" — and that argument covers one
        // composition, not a chain of them. v5 instead sums every link's
        // spatial-row translation rounding base into binary64 provenance,
        // capping the carried-parent term by the new contribution's size.
        // That recurrence is explicit and monotone; retaining the count of
        // four remains an empirical decision over the named shallow and deep
        // populations.
        //
        // Under the base this policy carried before the parent chain was
        // folded into it, a correct candidate demanded up to `41` — see
        // [`translation_composition_rounding_base`], and
        // `a_parent_chain_whose_translations_cancel_still_proves_its_skin`
        // for a rig that demands `524288`. Dropping the chain from the base
        // today refuses correct candidates in fifty-six of the sweep's cells at
        // up to `62.6` ulps of what remains, and narrowing the vertex stage to
        // `abs(p)` alone refuses in fifteen at up to `47.7`: the pattern every
        // time is a wrong magnitude, not a count that is too small.
        //
        // The detection cost is **not** bounded, and stating it as
        // `4.77e-7` of the compared quantity would be wrong. `4 * 2^-23` is
        // `4.77e-7` of *the magnitude the arithmetic ran on*, which equals
        // the compared quantity only when the two coincide. Where
        // cancellation made the compared quantity small, the term is that
        // same fraction of the larger operand, and so is
        // `4.77e-7 * (operand magnitude / compared magnitude)` of the
        // quantity actually being compared — a ratio with no upper bound.
        //
        // On this module's own
        // `a_joint_far_from_the_geometry_it_carries_still_proves_its_bounds`
        // fixture the term is `4.44` against a `W * B` of magnitude `1.0`:
        // `443 %` of the compared quantity's own magnitude. Measured on that
        // rig, the largest inverse-bind `x` shift still *accepted* is
        // `4.09375` units; the smallest refused is the next binary32 above
        // it. A regenerated bind wrong by four units is accepted. The bracket
        // is pinned by
        // `the_far_joint_rig_admits_a_four_unit_bind_shift_and_refuses_the_next_one_up`,
        // because a floor quoted here and nowhere held to drifts — an earlier
        // revision of this comment said `4.09`, which is on the accepted side
        // of the real floor.
        //
        // Folding the parent chain into that magnitude does not move this
        // number: on that fixture `abs(W) * abs(B)` already reads `6.38e6`
        // against the chain's `3.19e6`, so the `max` is unchanged and the
        // floor is still `4.09375` units. The chain widens the base only where
        // a chain actually cancelled, in proportion to what it cancelled, and
        // leaves it untouched everywhere else. Buying the same admissions by
        // raising the count instead would have cost the whole factor on
        // *every* slot, including the ones that lost nothing:
        // `a_parent_chain_whose_translations_cancel_still_proves_its_skin`
        // needs `524288` ulps of the base without the chain and `0.08` of it
        // with, and no count between those two is a policy anyone could
        // defend.
        //
        // So for a rig whose joints sit `k` times further from the origin
        // than the geometry they carry, `SkinMatrix` and `Bounds` lose
        // discriminating power in proportion to `k`. That is a property of
        // composing `W * B` from `f32` stored values, not of this policy:
        // the stored inverse bind's translation column is only accurate to
        // its own ulp, and composing it against `W` amplifies that
        // quantization by `W`'s linear part into a product the cancellation
        // has made near-identity. Composing in `f64` does not remove it —
        // measured over a 30_000-candidate rest/bind population, an `f64`
        // composition moves the worst skin residual from `2.50` to `2.06`
        // ulps and the worst bounds residual from `1.68` to `0.90`, leaving
        // the worst residual at `86 %` of the compared product's own
        // magnitude against a `1e-5` relative band. The term is covering
        // input quantization, which no amount of proof-side precision can
        // undo.
        f32_rounding_ulps: 4,
    };

    /// The expected ceiling on [`ScaleProof::observed_factor_divergence`]:
    /// [`Self::common_factor`] plus
    /// [`Self::postcondition_unit_scale_residual`], `7.103515625e-5` under
    /// [`Self::APPENDIX_D_V6`].
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
    /// - for a candidate the internal reference builder produced from the
    ///   source
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
    /// A divergence beyond it means the two witnesses were composed from state
    /// that does not agree — most often differing *stored* transforms, since
    /// [`crate::model::SourceNodeAsset::local_rest`] and
    /// [`crate::model::Bone::rest`] are separately stored descriptions of the
    /// same rest pose. It is not evidence of disagreeing parent chains: under
    /// [`crate::model::SourceSkeletonCoverage::Complete`] coverage the two
    /// chains are required to describe the same tree, and every entry point in
    /// this module refuses a document where they do not.
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

    /// [`Self::scalar_tolerance`] plus [`Self::f32_rounding_ulps`] binary32
    /// ulps of `magnitude`.
    ///
    /// `magnitude` is the largest quantity the compared `f32` arithmetic
    /// passed through — never the quantity being compared, which is what
    /// `before`/`after` already carry. The two coincide for a comparison
    /// whose operands are its own magnitude, and diverge without limit for
    /// one whose result was made small by cancellation; DESIGN.md Appendix D
    /// §D.1 names the magnitude each obligation takes.
    ///
    /// The added term is absolute in `magnitude` and so cannot widen a
    /// comparison relative to its own operands: at `magnitude ==
    /// max(before, after)` it is `f32_rounding_ulps * 2^-23 = 4.77e-7` of
    /// them, against the `1e-5` [`Self::scalar_relative`] already allows.
    pub fn f32_rounded_tolerance(&self, before: f64, after: f64, magnitude: f64) -> f64 {
        self.scalar_tolerance(before, after)
            + f64::from(self.f32_rounding_ulps) * magnitude.abs() * f64::from(f32::EPSILON)
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
    /// factor by `planning::plan_rest_bind`'s range check, an observed one by
    /// [`planning::classify_affine`].)
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

/// Typed, fail-closed rejection from [`plan_scale`], reference candidate
/// construction, or [`prove_scale`].
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
    /// A selected root, selected skin joint, or terminal affected source row
    /// did not normalize to a document skeleton bone. Unprojected rows are
    /// otherwise accepted only when they are strict connectors between
    /// projected rows.
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
    /// A transform composed during scale planning, candidate construction,
    /// or proof is non-finite. Entry-time skeleton rest failures are reported
    /// through [`ScaleError::InvalidDocumentShape`].
    #[error("node {node} has a non-finite rest transform")]
    NonFiniteTransform {
        /// The node with the non-finite transform.
        node: BoneId,
    },
    /// A runtime scale walk encountered a parent that cannot be resolved.
    /// Entry-time skeleton topology failures are reported through
    /// [`ScaleError::InvalidDocumentShape`].
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
    /// reference candidate construction and [`prove_scale`] each take the
    /// document to operate on as a separate argument and must not trust that
    /// it still matches the plan's shape.
    #[error("bone index {index} is out of range for this document")]
    BoneIndexOutOfRange {
        /// The out-of-range index.
        index: usize,
    },
    /// A plan replayed against a document derives a different write or proof
    /// inventory than it did when planned.
    ///
    /// Plans may be reused across numerically different documents, but only
    /// while re-deriving the supplied source's structural planning inventory
    /// selects the same complete domain. Otherwise a stale affected-node list
    /// or evidence flag could leave newly introduced payload outside every
    /// proof walk.
    #[error("plan does not describe the supplied document: {reason}")]
    PlanDocumentMismatch {
        /// Stable machine-readable mismatch kind.
        reason: &'static str,
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
    /// The plan's typed obligation ledger declared a claim provable, but
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
    /// The shared model shape required by strict mutating operations is
    /// malformed. [`Document`] is publicly mutable, so planning, building,
    /// and proof validate each supplied snapshot independently.
    #[error(transparent)]
    InvalidDocumentShape(#[from] DocumentShapeError),
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
    /// A primary skin-weight attribute contains a finite negative value.
    ///
    /// Skin weights are coefficients of a convex blend, never signed affine
    /// coefficients. Refusing this at the shared scale-input boundary keeps
    /// planning, candidate construction, and proof on that one semantic
    /// domain and gives evidence consumers a stable kind without requiring
    /// them to parse a [`ScaleError::InvalidMeshPrimitive`] reason string.
    #[error(
        "mesh {mesh_index} primitive {primitive_index} vertex {vertex_index} primary skin influence {influence_index} has a negative weight"
    )]
    NegativeSkinWeight {
        /// Index into `document.assets.meshes`.
        mesh_index: usize,
        /// Index into that mesh's `primitives`.
        primitive_index: usize,
        /// Vertex carrying the rejected weight tuple.
        vertex_index: usize,
        /// Component within the primary four-influence tuple.
        influence_index: usize,
    },
    /// A skinned primitive is malformed: `joints`/`weights` shorter than
    /// `positions`, a non-finite position or weight, a joint-influence slot
    /// outside the owning instance's `skin_joints`, or a skinned result that
    /// is not finite.
    ///
    /// The last case reports two distinct `reason`s. A skinned position that
    /// left the `f32` range is `"skinned_magnitude_overflow"`: the document's
    /// geometry does not fit the arithmetic this proof runs in. A `NaN` is
    /// `"non_finite_result"`: an input that survived every finiteness check
    /// above is degenerate in some other way. Both fail closed; neither is
    /// bounded by a magnitude domain, because skinning accumulates a dot
    /// product per axis and where that overflows depends on the rotation
    /// rather than on the magnitude of the result.
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
    /// `candidate`'s skeleton/source-projection, clip/track/instance/mesh/
    /// primitive structure does not match `source`'s, or an exact unchanged
    /// semantic value differs. This includes a changed parent or source-node
    /// projection, a changed world-rest affine outside a rest/bind closure, a
    /// missing or extra clip, track, instance, mesh, or primitive, a track
    /// whose identity, interpolation, times, or value shape disagrees with
    /// its source counterpart, or a mesh instance whose identity — the node
    /// it hangs off, the source node it came from, the mesh it draws, or the
    /// joints it binds — disagrees with its source counterpart. Proof pairs
    /// source and candidate structure by identity or index, which requires
    /// this parity to hold. For rest/bind this also covers an admitted static
    /// connector local that changed bits or a projected successor whose raw
    /// local is not the independently derived bridged rebase. An extra,
    /// missing, re-parented, relocated, or otherwise rewritten unchanged
    /// value is never silently ignored.
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

/// The structural semantic operation a rewritten field receives.
///
/// No variant stores a resolved factor or expected value. Candidate
/// construction and proof independently resolve the selected operation's
/// numeric arithmetic from the field identity and topology.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ScaleRewriteRule {
    /// A whole-document linear-unit length field.
    WholeDocumentLength,
    /// A rest/bind field governed by the target node's parent-basis factor.
    RestBindParentBasis,
    /// A rest/bind field governed by the local `s_parent / s_node` rebase.
    RestBindLocalScale,
    /// A rest/bind inverse bind governed by its joint's node-basis factor.
    RestBindNodeBasis,
    /// A projected source-local rest, optionally bridged through connectors.
    RestBindSourceLocal {
        /// The immediate connector tail below the projected parent, if any.
        connector_tail: Option<usize>,
    },
}

/// Whether a modeled field is preserved exactly or analytically rewritten.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ScaleFieldDisposition {
    /// The field is outside the core builder's write set.
    ///
    /// This is ownership, not a universal normalized-artifact equality
    /// promise: format frontends may independently re-derive normalized
    /// bones, binds, tracks, or meshes within the established residual
    /// policy. Authored raw source-local fields copied by the core builder are
    /// additionally checked bit-exact by [`prove_scale`].
    PreserveExact,
    /// The field is in the write set and receives the stated semantic rule.
    Rewrite(ScaleRewriteRule),
}

/// One normalized bone-rest field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ScaleBoneRestField {
    /// Local translation.
    Translation,
    /// Local rotation.
    Rotation,
    /// Local scale.
    Scale,
}

/// One authored source-node rest field or component group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ScaleSourceRestField {
    /// TRS translation.
    Translation,
    /// TRS rotation.
    Rotation,
    /// TRS scale.
    Scale,
    /// Matrix linear columns.
    MatrixLinear,
    /// Matrix translation column.
    MatrixTranslation,
    /// Matrix homogeneous row.
    MatrixHomogeneous,
}

/// The exact container-level target of one field disposition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ScaleFieldTarget {
    /// One normalized bone-rest field.
    BoneRest {
        /// Normalized bone identity.
        bone: BoneId,
        /// Rest field.
        field: ScaleBoneRestField,
    },
    /// One authored source-node rest field.
    SourceNodeRest {
        /// Raw source-node identity.
        source_node_index: usize,
        /// Rest field or component group.
        field: ScaleSourceRestField,
    },
    /// One animation track's stored values.
    AnimationValues {
        /// Clip position in the normalized document.
        clip_index: usize,
        /// Track position inside the clip.
        track_index: usize,
        /// Target normalized bone.
        bone: BoneId,
        /// Animated property.
        property: Property,
    },
    /// One bone convenience inverse bind.
    BoneInverseBind {
        /// Normalized bone identity.
        bone: BoneId,
    },
    /// One logical instance inverse-bind slot.
    InstanceInverseBind {
        /// Instance position in the normalized document.
        instance_index: usize,
        /// Slot position inside the instance skin.
        slot: usize,
        /// Joint named by the slot.
        joint: BoneId,
    },
    /// One primitive's complete base-position array.
    MeshPositions {
        /// Mesh position in the normalized document.
        mesh_index: usize,
        /// Primitive position inside the mesh.
        primitive_index: usize,
    },
    /// One primitive's preserved normal array.
    MeshNormals {
        /// Mesh position in the normalized document.
        mesh_index: usize,
        /// Primitive position inside the mesh.
        primitive_index: usize,
    },
}

/// One exact semantic field row in a compiled scale plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ScaleFieldPlan {
    target: ScaleFieldTarget,
    disposition: ScaleFieldDisposition,
    element_count: usize,
}

impl ScaleFieldPlan {
    /// The exact container-level field target.
    pub fn target(&self) -> ScaleFieldTarget {
        self.target
    }

    /// Whether and how the target is rewritten.
    pub fn disposition(&self) -> ScaleFieldDisposition {
        self.disposition
    }

    /// Number of stored elements covered by the container row.
    pub fn element_count(&self) -> usize {
        self.element_count
    }
}

/// One numeric-value-free payload-shape row used by stale-plan replay.
///
/// Rows include empty containers and structural identities/counts, but never
/// key times or stored floating-point values, so intentional numeric replay
/// against an identically shaped document remains supported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ScalePayloadShapeRow {
    /// Top-level normalized collection counts.
    Document {
        /// Skeleton bone count.
        bone_count: usize,
        /// Authoritative source-skeleton node count; zero under unavailable
        /// coverage.
        source_node_count: usize,
        /// Whether the source projection claims complete coverage.
        source_coverage: SourceSkeletonCoverage,
        /// Clip count.
        clip_count: usize,
        /// Mesh-instance count.
        instance_count: usize,
        /// Mesh count.
        mesh_count: usize,
    },
    /// One normalized topology row.
    Bone {
        /// Normalized bone identity.
        bone: BoneId,
        /// Normalized parent identity.
        parent: Option<BoneId>,
    },
    /// One source skin's complete structural inventory.
    SourceSkin {
        /// Raw source-skin identity.
        source_skin_index: usize,
        /// Explicit source skeleton root.
        skeleton_root_source_node_index: Option<usize>,
        /// Declared joint count.
        joint_count: usize,
        /// Attachment count.
        attachment_count: usize,
        /// Inverse-bind accessor status.
        inverse_bind_status: SourceInverseBindAccessorStatus,
        /// Declared inverse-bind accessor count.
        inverse_bind_declared_count: Option<usize>,
        /// Number of readable matrices retained.
        inverse_bind_matrix_count: usize,
    },
    /// One ordered source-skin joint identity.
    SourceSkinJoint {
        /// Raw source-skin identity.
        source_skin_index: usize,
        /// Joint slot.
        slot: usize,
        /// Raw source-node identity.
        source_node_index: usize,
    },
    /// One ordered source-skin attachment identity.
    SourceSkinAttachment {
        /// Raw source-skin identity.
        source_skin_index: usize,
        /// Attachment position.
        attachment_index: usize,
        /// Raw attachment node identity.
        source_node_index: usize,
        /// Raw mesh identity, when declared.
        source_mesh_index: Option<usize>,
    },
    /// One clip, including an empty clip.
    Clip {
        /// Clip position.
        clip_index: usize,
        /// Track count.
        track_count: usize,
    },
    /// One track's structural identity and arities.
    Track {
        /// Clip position.
        clip_index: usize,
        /// Track position.
        track_index: usize,
        /// Target bone.
        bone: BoneId,
        /// Animated property.
        property: Property,
        /// Interpolation mode.
        interpolation: Interpolation,
        /// Number of key times, without storing their numeric values.
        key_count: usize,
        /// Number of stored value elements.
        value_count: usize,
    },
    /// One mesh instance, including an unskinned instance.
    Instance {
        /// Instance position.
        instance_index: usize,
        /// Normalized attachment node.
        node: BoneId,
        /// Raw source-node attachment identity.
        source_node_index: usize,
        /// Mesh identity.
        mesh: usize,
        /// Logical joint-slot count.
        joint_count: usize,
        /// Stored instance inverse-bind count.
        inverse_bind_count: usize,
    },
    /// One logical joint slot, preserving slot order and identity.
    InstanceJoint {
        /// Instance position.
        instance_index: usize,
        /// Slot position.
        slot: usize,
        /// Joint identity.
        joint: BoneId,
    },
    /// One mesh, including an empty mesh.
    Mesh {
        /// Mesh position.
        mesh_index: usize,
        /// Stable source mesh identity.
        source_mesh_index: usize,
        /// Primitive count.
        primitive_count: usize,
    },
    /// One primitive's modeled shape.
    Primitive {
        /// Mesh position.
        mesh_index: usize,
        /// Primitive position.
        primitive_index: usize,
        /// Base-position count.
        position_count: usize,
        /// Preserved normal count.
        normal_count: usize,
        /// Primary joint-tuple count read by skin/bounds proof.
        joint_count: usize,
        /// Primary weight-tuple count read by skin/bounds proof.
        weight_count: usize,
    },
}

/// One typed proof claim kind derived from the plan's validated inventory.
///
/// Exact members are not duplicated here: inspect [`ScalePlan::affected_nodes`],
/// [`ScalePlan::transform_only_attachments`], and the field, payload, and
/// topology rows exposed by [`ScalePlan::ledger`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ScaleProofObligation {
    /// Preserve normalized parents and complete source projection topology.
    ExactTopology,
    /// Preserve clip, track, instance, skin, mesh, and primitive identities.
    ExactPayloadIdentity,
    /// Preserve exact world rest for the nodes outside a rest/bind closure.
    ExactUnchangedWorldRest,
    /// Prove affected rest-world translation and orientation.
    RestWorld,
    /// Prove affected rest-world facts and the nested unit-scale postcondition.
    RestWorldAndUnitScale,
    /// Probe complete expected affines of transform-only attachments.
    TransformOnlyAffine,
    /// Compare all rewritten and preserved animation values.
    TrackValues,
    /// Compare all rewritten and preserved base positions.
    MeshPositions,
    /// Compare affected translation tracks at their key times.
    KeyTranslations,
    /// Compare affected translation tracks at bounded cubic interior times.
    CubicInteriors,
    /// Compare sampled world-space trajectories.
    Trajectories,
    /// Run the one shared affected-skin walk producing skin and bounds results.
    SkinAndBounds,
    /// Check rewritten inverse-bind slots.
    AffectedInverseBinds,
    /// Check preserved inverse-bind slots outside the closure.
    UnaffectedInverseBinds,
    /// Preserve connector locals and check bridged projected successors.
    ExactConnectorProjection,
}

/// A projected source row's role in a rest/bind topology.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ScaleProjectedRole {
    /// Selected scaled root.
    Root,
    /// Selected skin joint other than the root.
    Joint,
    /// Affected non-joint attachment or path node.
    TransformOnly,
}

/// The typed kind of one canonical rest/bind source-topology row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ScaleSourceNodeKind {
    /// A source node projected into the normalized skeleton.
    Projected {
        /// Normalized bone identity.
        bone: BoneId,
        /// Root, joint, or transform-only role.
        role: ScaleProjectedRole,
        /// Nearest projected parent in source identity space.
        projected_parent: Option<usize>,
        /// Immediate connector tail below that projected parent, if any.
        incoming_connector_tail: Option<usize>,
    },
    /// A static unprojected connector preserved exactly.
    Connector,
    /// A source row outside a rest/bind domain, or any row in a
    /// whole-document plan where connector roles are not applicable.
    OutsideDomain {
        /// Normalized projection identity, if one exists.
        bone: Option<BoneId>,
    },
}

/// One row in the canonical source-keyed rest/bind topology.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ScaleSourceTopologyRow {
    source_node_index: usize,
    parent_source_node_index: Option<usize>,
    kind: ScaleSourceNodeKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ScaleLedger {
    field_rows: Vec<ScaleFieldPlan>,
    payload_shapes: Vec<ScalePayloadShapeRow>,
    obligations: Vec<ScaleProofObligation>,
}

#[derive(Debug, Clone, PartialEq)]
struct WholeDocumentParams {
    factor: f64,
}

#[derive(Debug, Clone, PartialEq)]
struct RestBindParams {
    source_skin_index: usize,
    source_root_node_index: usize,
    expected_factor: f64,
    transform_only_attachments: Vec<BoneId>,
}

#[derive(Debug, Clone, PartialEq)]
enum ScaleCompiledPlan {
    WholeDocument(WholeDocumentParams),
    RestBind(RestBindParams),
}

/// Read-only view of one compiled plan's exact domain, field, topology, and
/// proof-obligation ledger.
///
/// The view has no constructor and cannot be converted back into a
/// [`ScalePlan`]; only [`plan_scale`] can compile an authoritative ledger.
#[derive(Debug, Clone, Copy)]
pub struct ScalePlanLedger<'a> {
    plan: &'a ScalePlan,
}

impl<'a> ScalePlanLedger<'a> {
    fn ledger(self) -> &'a ScaleLedger {
        &self.plan.ledger
    }

    /// Exact container-level modeled field rows in deterministic source order.
    pub fn field_rows(self) -> std::slice::Iter<'a, ScaleFieldPlan> {
        self.ledger().field_rows.iter()
    }

    /// Numeric-value-free payload-shape rows, including empty containers.
    pub fn payload_shapes(self) -> std::slice::Iter<'a, ScalePayloadShapeRow> {
        self.ledger().payload_shapes.iter()
    }

    /// Typed proof obligations derived from the same field and payload inventory.
    pub fn obligations(self) -> std::slice::Iter<'a, ScaleProofObligation> {
        self.ledger().obligations.iter()
    }

    /// Canonical source-keyed topology for the complete modeled projection.
    pub fn source_topology(self) -> std::slice::Iter<'a, ScaleSourceTopologyRow> {
        self.plan.source_topology.iter()
    }
}

impl ScaleSourceTopologyRow {
    /// Raw source-node identity.
    pub fn source_node_index(&self) -> usize {
        self.source_node_index
    }

    /// Authoritative raw parent identity.
    pub fn parent_source_node_index(&self) -> Option<usize> {
        self.parent_source_node_index
    }

    /// Whether this row is projected or is a preserved connector.
    pub fn kind(&self) -> ScaleSourceNodeKind {
        self.kind
    }
}

/// Pure, typed plan returned by [`plan_scale`].
///
/// Planning never mutates its input document; it only inspects it. Reference
/// candidate construction from an accepted plan is a distinct, separately
/// fallible fixture step.
///
/// Every field is private: a [`ScalePlan`] can only be produced by
/// [`plan_scale`], so an external caller cannot hand-construct or mutate one
/// into a state whose `affected_nodes` disagree with `operation`'s
/// selectors. Read plan contents through the accessor methods.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct ScalePlan {
    tolerance_policy: ScaleTolerancePolicy,
    observed_factor: f64,
    affected_nodes: Vec<BoneId>,
    source_topology: Vec<ScaleSourceTopologyRow>,
    ledger: ScaleLedger,
    compiled: ScaleCompiledPlan,
}

impl ScalePlan {
    /// Echoed operation and its declared parameters.
    pub fn operation(&self) -> ScaleOperation {
        match &self.compiled {
            ScaleCompiledPlan::WholeDocument(plan) => ScaleOperation::WholeDocumentLinearUnits {
                factor: plan.factor,
            },
            ScaleCompiledPlan::RestBind(plan) => ScaleOperation::RestBindUniformScale {
                source_skin_index: plan.source_skin_index,
                source_root_node_index: plan.source_root_node_index,
                expected_factor: plan.expected_factor,
            },
        }
    }

    /// The fixed tolerance policy this plan and its proof share.
    pub fn tolerance_policy(&self) -> ScaleTolerancePolicy {
        self.tolerance_policy
    }

    /// Affected normalized-node closure, in ascending bone-id order.
    ///
    /// For [`ScaleOperation::WholeDocumentLinearUnits`] this is every node
    /// in the document. For [`ScaleOperation::RestBindUniformScale`] this is
    /// the closed connected hierarchy of DESIGN.md Appendix D §D.2: the
    /// scaled ancestor, every selected skin joint and the normalized paths
    /// between them, and every descendant transform-only attachment. Raw
    /// source-only connector rows on those paths are not normalized nodes
    /// and therefore do not appear in this list.
    pub fn affected_nodes(&self) -> &[BoneId] {
        &self.affected_nodes
    }

    /// Descendant nodes in [`Self::affected_nodes`] that carry no skin —
    /// the "transform-only child" case of DESIGN.md Appendix D §D.2/§D.3.
    /// Always empty for [`ScaleOperation::WholeDocumentLinearUnits`].
    pub fn transform_only_attachments(&self) -> &[BoneId] {
        match &self.compiled {
            ScaleCompiledPlan::WholeDocument(_) => &[],
            ScaleCompiledPlan::RestBind(plan) => &plan.transform_only_attachments,
        }
    }

    /// The one common factor `s` (or `q` for whole-document conversion)
    /// applied across [`Self::affected_nodes`].
    ///
    /// This is always the factor the *caller declared*, never the one
    /// measured from the source: reference construction applies exactly this
    /// value, and [`prove_scale`] states every analytic expectation in terms
    /// of it. [`Self::observed_factor`] reports the measured
    /// counterpart, and the two are separate numbers on purpose — DESIGN.md
    /// Appendix D §D.6 requires producer evidence to record both.
    pub fn common_factor(&self) -> f64 {
        match &self.compiled {
            ScaleCompiledPlan::WholeDocument(plan) => plan.factor,
            ScaleCompiledPlan::RestBind(plan) => plan.expected_factor,
        }
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

    /// Validate this plan's complete structural inventory against `document`.
    ///
    /// This re-derives and exactly compares the affected domain, canonical
    /// source topology, transform-only attachments, payload shapes, field
    /// dispositions, and proof obligations. Numeric source values are not
    /// compared, so a document with the same structural ledger remains a
    /// valid replay source, but the replay document must still satisfy the
    /// finite-value and nonnegative-weight scale-input requirements.
    ///
    /// # Errors
    ///
    /// Returns [`ScaleError::PlanDocumentMismatch`] when the re-derived
    /// inventory differs, or the corresponding planning/input error when
    /// `document` cannot produce a valid inventory for this operation.
    pub fn validate_document_inventory(&self, document: &Document) -> Result<(), ScaleError> {
        validate_plan_document_inventory(document, self)
    }

    /// Inspect the exact read-only topology, field, and obligation ledger.
    pub fn ledger(&self) -> ScalePlanLedger<'_> {
        ScalePlanLedger { plan: self }
    }

    fn affected_set(&self) -> BTreeSet<BoneId> {
        self.affected_nodes().iter().copied().collect()
    }

    fn is_whole_document(&self) -> bool {
        matches!(self.compiled, ScaleCompiledPlan::WholeDocument(_))
    }

    fn field_rows(&self) -> &[ScaleFieldPlan] {
        &self.ledger.field_rows
    }

    fn obligations(&self) -> &[ScaleProofObligation] {
        &self.ledger.obligations
    }

    fn has_obligation(&self, expected: ScaleProofObligation) -> bool {
        self.obligations().contains(&expected)
    }

    fn rest_obligation(&self) -> Option<(&[BoneId], bool)> {
        if self.has_obligation(ScaleProofObligation::RestWorldAndUnitScale) {
            Some((self.affected_nodes(), true))
        } else if self.has_obligation(ScaleProofObligation::RestWorld) {
            Some((self.affected_nodes(), false))
        } else {
            None
        }
    }

    fn transform_only_nodes(&self) -> Option<&[BoneId]> {
        self.has_obligation(ScaleProofObligation::TransformOnlyAffine)
            .then(|| self.transform_only_attachments())
    }

    fn has_key_translations(&self) -> bool {
        self.has_obligation(ScaleProofObligation::KeyTranslations)
    }

    fn has_cubic_interiors(&self) -> bool {
        self.has_obligation(ScaleProofObligation::CubicInteriors)
    }

    fn trajectory_nodes(&self) -> Option<&[BoneId]> {
        self.has_obligation(ScaleProofObligation::Trajectories)
            .then(|| self.affected_nodes())
    }

    fn has_skin_and_bounds(&self) -> bool {
        self.has_obligation(ScaleProofObligation::SkinAndBounds)
    }

    fn has_unaffected_binds(&self) -> bool {
        self.has_obligation(ScaleProofObligation::UnaffectedInverseBinds)
    }
}

/// Independently compose one proof-side connector product.
///
/// This intentionally does not call the reference writer's connector-product
/// cache: the raw source-projection check must not certify a writer bug by
/// deriving its expectation through the writer's own product cache.
fn proof_connector_product(
    connector_tail: usize,
    by_source_index: &BTreeMap<usize, &SourceNodeAsset>,
    connector_product_by_tail: &mut BTreeMap<usize, DMat4>,
) -> Result<DMat4, ScaleError> {
    let mut suffix = Vec::new();
    let mut visited = BTreeSet::new();
    let mut cursor = connector_tail;
    let mut product = loop {
        if let Some(&cached) = connector_product_by_tail.get(&cursor) {
            break cached;
        }
        if !visited.insert(cursor) {
            return Err(ScaleError::IncompleteClosure {
                reason: "cyclic_connector_source_parent_chain",
            });
        }
        let node = by_source_index
            .get(&cursor)
            .ok_or(ScaleError::IncompleteClosure {
                reason: "dangling_connector_source_node_index",
            })?;
        if node.bone.is_some() {
            break DMat4::IDENTITY;
        }
        suffix.push(cursor);
        cursor = node
            .parent_source_node_index
            .ok_or(ScaleError::IncompleteClosure {
                reason: "connector_without_projected_ancestor",
            })?;
    };
    while let Some(source) = suffix.pop() {
        let node = by_source_index
            .get(&source)
            .ok_or(ScaleError::IncompleteClosure {
                reason: "dangling_connector_source_node_index",
            })?;
        product *= local_rest_matrix(&node.local_rest).as_dmat4();
        connector_product_by_tail.insert(source, product);
    }
    connector_product_by_tail
        .get(&connector_tail)
        .copied()
        .ok_or(ScaleError::IncompleteClosure {
            reason: "empty_connector_bridge",
        })
}

/// Independently derive the exact f32 local that a bridged projected source
/// row must carry in the candidate.
fn proof_expected_bridged_source_local(
    local_rest: &SourceNodeLocalRest,
    connector: DMat4,
    s_parent: f32,
    s_node: f32,
    bone: BoneId,
) -> Result<SourceNodeLocalRest, ScaleError> {
    if s_parent == 1.0 && s_node == 1.0 {
        return Ok(local_rest.clone());
    }
    let inverse = DMat3::from_cols(
        connector.x_axis.truncate(),
        connector.y_axis.truncate(),
        connector.z_axis.truncate(),
    )
    .inverse();
    let offset = inverse * (connector.w_axis.truncate() * (f64::from(s_parent) - 1.0));
    if !inverse.x_axis.is_finite()
        || !inverse.y_axis.is_finite()
        || !inverse.z_axis.is_finite()
        || !offset.is_finite()
    {
        return Err(ScaleError::NonFiniteTransform { node: bone });
    }
    let expected = match local_rest {
        SourceNodeLocalRest::Trs {
            translation,
            rotation,
            scale,
        } => SourceNodeLocalRest::Trs {
            translation: (translation.as_dvec3() * f64::from(s_parent) + offset).as_vec3(),
            rotation: *rotation,
            scale: (scale.as_dvec3() * (f64::from(s_parent) / f64::from(s_node))).as_vec3(),
        },
        SourceNodeLocalRest::Matrix(matrix) => {
            let ratio = f64::from(s_parent) / f64::from(s_node);
            let rebase_linear_column = |column: Vec4| {
                (column.truncate().as_dvec3() * ratio)
                    .as_vec3()
                    .extend(column.w)
            };
            let translation =
                (matrix.w_axis.truncate().as_dvec3() * f64::from(s_parent) + offset).as_vec3();
            SourceNodeLocalRest::Matrix(Mat4::from_cols(
                rebase_linear_column(matrix.x_axis),
                rebase_linear_column(matrix.y_axis),
                rebase_linear_column(matrix.z_axis),
                translation.extend(matrix.w_axis.w),
            ))
        }
    };
    if !mat4_is_finite(local_rest_matrix(&expected)) {
        return Err(ScaleError::NonFiniteTransform { node: bone });
    }
    Ok(expected)
}

fn vec3_bits_equal(left: Vec3, right: Vec3) -> bool {
    left.to_array().map(f32::to_bits) == right.to_array().map(f32::to_bits)
}

fn quat_bits_equal(left: Quat, right: Quat) -> bool {
    left.to_array().map(f32::to_bits) == right.to_array().map(f32::to_bits)
}

fn source_rest_field_bits_equal(
    left: &SourceNodeLocalRest,
    right: &SourceNodeLocalRest,
    field: ScaleSourceRestField,
) -> bool {
    match (left, right, field) {
        (
            SourceNodeLocalRest::Trs {
                translation: left, ..
            },
            SourceNodeLocalRest::Trs {
                translation: right, ..
            },
            ScaleSourceRestField::Translation,
        )
        | (
            SourceNodeLocalRest::Trs { scale: left, .. },
            SourceNodeLocalRest::Trs { scale: right, .. },
            ScaleSourceRestField::Scale,
        ) => vec3_bits_equal(*left, *right),
        (
            SourceNodeLocalRest::Trs { rotation: left, .. },
            SourceNodeLocalRest::Trs {
                rotation: right, ..
            },
            ScaleSourceRestField::Rotation,
        ) => quat_bits_equal(*left, *right),
        (
            SourceNodeLocalRest::Matrix(left),
            SourceNodeLocalRest::Matrix(right),
            ScaleSourceRestField::MatrixLinear,
        ) => {
            vec3_bits_equal(left.x_axis.truncate(), right.x_axis.truncate())
                && vec3_bits_equal(left.y_axis.truncate(), right.y_axis.truncate())
                && vec3_bits_equal(left.z_axis.truncate(), right.z_axis.truncate())
        }
        (
            SourceNodeLocalRest::Matrix(left),
            SourceNodeLocalRest::Matrix(right),
            ScaleSourceRestField::MatrixTranslation,
        ) => vec3_bits_equal(left.w_axis.truncate(), right.w_axis.truncate()),
        (
            SourceNodeLocalRest::Matrix(left),
            SourceNodeLocalRest::Matrix(right),
            ScaleSourceRestField::MatrixHomogeneous,
        ) => {
            [left.x_axis.w, left.y_axis.w, left.z_axis.w, left.w_axis.w].map(f32::to_bits)
                == [
                    right.x_axis.w,
                    right.y_axis.w,
                    right.z_axis.w,
                    right.w_axis.w,
                ]
                .map(f32::to_bits)
        }
        _ => false,
    }
}

fn f32_values_within_scale_tolerance<const N: usize>(
    left: [f32; N],
    right: [f32; N],
    tolerance: &ScaleTolerancePolicy,
) -> bool {
    left.into_iter().zip(right).all(|(left, right)| {
        let left = f64::from(left);
        let right = f64::from(right);
        let residual = (left - right).abs();
        let limit = tolerance.scalar_tolerance(left, right);
        residual.is_finite() && limit.is_finite() && residual <= limit
    })
}

fn rewritten_source_rest_field_within_tolerance(
    expected: &SourceNodeLocalRest,
    actual: &SourceNodeLocalRest,
    field: ScaleSourceRestField,
    tolerance: &ScaleTolerancePolicy,
) -> bool {
    match (expected, actual, field) {
        (
            SourceNodeLocalRest::Trs {
                translation: expected,
                ..
            },
            SourceNodeLocalRest::Trs {
                translation: actual,
                ..
            },
            ScaleSourceRestField::Translation,
        )
        | (
            SourceNodeLocalRest::Trs {
                scale: expected, ..
            },
            SourceNodeLocalRest::Trs { scale: actual, .. },
            ScaleSourceRestField::Scale,
        ) => f32_values_within_scale_tolerance(expected.to_array(), actual.to_array(), tolerance),
        (
            SourceNodeLocalRest::Matrix(expected),
            SourceNodeLocalRest::Matrix(actual),
            ScaleSourceRestField::MatrixLinear,
        ) => f32_values_within_scale_tolerance(
            [
                expected.x_axis.x,
                expected.x_axis.y,
                expected.x_axis.z,
                expected.y_axis.x,
                expected.y_axis.y,
                expected.y_axis.z,
                expected.z_axis.x,
                expected.z_axis.y,
                expected.z_axis.z,
            ],
            [
                actual.x_axis.x,
                actual.x_axis.y,
                actual.x_axis.z,
                actual.y_axis.x,
                actual.y_axis.y,
                actual.y_axis.z,
                actual.z_axis.x,
                actual.z_axis.y,
                actual.z_axis.z,
            ],
            tolerance,
        ),
        (
            SourceNodeLocalRest::Matrix(expected),
            SourceNodeLocalRest::Matrix(actual),
            ScaleSourceRestField::MatrixTranslation,
        ) => f32_values_within_scale_tolerance(
            expected.w_axis.truncate().to_array(),
            actual.w_axis.truncate().to_array(),
            tolerance,
        ),
        // Rewrites never own rotation or the matrix homogeneous row. Keep
        // these impossible combinations fail-closed instead of silently
        // assigning them a numeric policy.
        _ => false,
    }
}

/// Independently derive the raw authored local expected by one rewrite row.
///
/// This is deliberately proof-owned arithmetic. In particular, direct rows
/// spell out their `f32` association here instead of calling the builder's
/// rebase helpers, while connector rows use the separately implemented
/// proof-side connector product and widened affine derivation.
fn proof_expected_rewritten_source_local(
    source: &Document,
    plan: &ScalePlan,
    affected: &BTreeSet<BoneId>,
    source_node: &SourceNodeAsset,
    rule: ScaleRewriteRule,
    connector_products: &mut BTreeMap<usize, DMat4>,
    source_nodes: &BTreeMap<usize, &SourceNodeAsset>,
) -> Result<SourceNodeLocalRest, ScaleError> {
    match rule {
        ScaleRewriteRule::WholeDocumentLength => {
            let q = check_factor_narrows(plan.common_factor(), plan.common_factor())?;
            Ok(match &source_node.local_rest {
                SourceNodeLocalRest::Trs {
                    translation,
                    rotation,
                    scale,
                } => SourceNodeLocalRest::Trs {
                    translation: Vec3::new(translation.x * q, translation.y * q, translation.z * q),
                    rotation: *rotation,
                    scale: *scale,
                },
                SourceNodeLocalRest::Matrix(matrix) => {
                    SourceNodeLocalRest::Matrix(Mat4::from_cols(
                        matrix.x_axis,
                        matrix.y_axis,
                        matrix.z_axis,
                        Vec4::new(
                            matrix.w_axis.x * q,
                            matrix.w_axis.y * q,
                            matrix.w_axis.z * q,
                            matrix.w_axis.w,
                        ),
                    ))
                }
            })
        }
        ScaleRewriteRule::RestBindSourceLocal { connector_tail } => {
            let bone = source_node
                .bone
                .ok_or(ScaleError::SourceNodeNotNormalized {
                    source_node_index: source_node.source_node_index,
                })?;
            let parent = source
                .skeleton
                .bones
                .get(bone)
                .ok_or(ScaleError::BoneIndexOutOfRange { index: bone })?
                .parent;
            let s = check_factor_narrows(plan.common_factor(), plan.common_factor())?;
            let s_parent = if parent.is_some_and(|parent| affected.contains(&parent)) {
                s
            } else {
                1.0
            };
            let s_node = if affected.contains(&bone) { s } else { 1.0 };
            if let Some(connector_tail) = connector_tail {
                let connector =
                    proof_connector_product(connector_tail, source_nodes, connector_products)?;
                return proof_expected_bridged_source_local(
                    &source_node.local_rest,
                    connector,
                    s_parent,
                    s_node,
                    bone,
                );
            }
            Ok(match &source_node.local_rest {
                SourceNodeLocalRest::Trs {
                    translation,
                    rotation,
                    scale,
                } => SourceNodeLocalRest::Trs {
                    translation: Vec3::new(
                        translation.x * s_parent,
                        translation.y * s_parent,
                        translation.z * s_parent,
                    ),
                    rotation: *rotation,
                    scale: Vec3::new(
                        scale.x * (s_parent / s_node),
                        scale.y * (s_parent / s_node),
                        scale.z * (s_parent / s_node),
                    ),
                },
                SourceNodeLocalRest::Matrix(matrix) => {
                    let inverse_node = 1.0 / s_node;
                    let linear = |column: Vec4| {
                        Vec4::new(
                            column.x * s_parent * inverse_node,
                            column.y * s_parent * inverse_node,
                            column.z * s_parent * inverse_node,
                            column.w * inverse_node,
                        )
                    };
                    SourceNodeLocalRest::Matrix(Mat4::from_cols(
                        linear(matrix.x_axis),
                        linear(matrix.y_axis),
                        linear(matrix.z_axis),
                        Vec4::new(
                            matrix.w_axis.x * s_parent,
                            matrix.w_axis.y * s_parent,
                            matrix.w_axis.z * s_parent,
                            matrix.w_axis.w,
                        ),
                    ))
                }
            })
        }
        ScaleRewriteRule::RestBindParentBasis
        | ScaleRewriteRule::RestBindLocalScale
        | ScaleRewriteRule::RestBindNodeBasis => Err(ScaleError::PlanDocumentMismatch {
            reason: "invalid_source_local_rewrite_rule",
        }),
    }
}

/// Discharge rewritten authored source-local rows against a proof-owned
/// analytic expectation. Direct raw format adapters may narrow an authored
/// value before or after applying the same factor, so those rows use the
/// published scale tolerance. Connector-bridged successors remain bit-exact:
/// that core-only path has no second frontend narrowing boundary.
fn check_rewritten_source_field_dispositions(
    source: &Document,
    candidate: &Document,
    plan: &ScalePlan,
    affected: &BTreeSet<BoneId>,
    tolerance: &ScaleTolerancePolicy,
    discharged: &mut BTreeSet<usize>,
) -> Result<(), ScaleError> {
    let source_nodes = source_node_index_map(source);
    let candidate_nodes = source_node_index_map(candidate);
    let mut connector_products = BTreeMap::new();
    for (row_index, row) in plan.field_rows().iter().enumerate() {
        let (
            ScaleFieldTarget::SourceNodeRest {
                source_node_index,
                field,
            },
            ScaleFieldDisposition::Rewrite(rule),
        ) = (row.target, row.disposition)
        else {
            continue;
        };
        let before =
            source_nodes
                .get(&source_node_index)
                .ok_or(ScaleError::CandidateStructureMismatch {
                    reason: "rewritten_source_node_missing",
                })?;
        let after = candidate_nodes.get(&source_node_index).ok_or(
            ScaleError::CandidateStructureMismatch {
                reason: "rewritten_source_node_missing",
            },
        )?;
        let expected = proof_expected_rewritten_source_local(
            source,
            plan,
            affected,
            before,
            rule,
            &mut connector_products,
            &source_nodes,
        )?;
        let bridged = matches!(
            rule,
            ScaleRewriteRule::RestBindSourceLocal {
                connector_tail: Some(_)
            }
        );
        let matches = if bridged {
            source_rest_field_bits_equal(&expected, &after.local_rest, field)
        } else {
            rewritten_source_rest_field_within_tolerance(
                &expected,
                &after.local_rest,
                field,
                tolerance,
            )
        };
        if !matches {
            return Err(ScaleError::CandidateStructureMismatch {
                reason: if bridged {
                    "bridged_source_local_mismatch"
                } else {
                    "field_disposition_mismatch"
                },
            });
        }
        mark_field_row_discharged(discharged, row_index)?;
    }
    Ok(())
}

/// Discharge preserve-exact authored source-local rows after semantic
/// residuals succeed. These are raw fields both core builders copy directly;
/// normalized fields may be independently re-derived by a format frontend
/// and remain governed by the existing versioned residual policy.
fn check_preserved_field_dispositions(
    source: &Document,
    candidate: &Document,
    plan: &ScalePlan,
    discharged: &mut BTreeSet<usize>,
) -> Result<(), ScaleError> {
    let source_nodes = source_node_index_map(source);
    let candidate_nodes = source_node_index_map(candidate);
    let mut connector_sources = BTreeSet::new();
    let mut bridged_successors = BTreeSet::new();
    for row in plan.ledger().source_topology() {
        match row.kind() {
            ScaleSourceNodeKind::Connector => {
                connector_sources.insert(row.source_node_index());
            }
            ScaleSourceNodeKind::Projected {
                incoming_connector_tail: Some(_),
                ..
            } => {
                bridged_successors.insert(row.source_node_index());
            }
            ScaleSourceNodeKind::Projected { .. } | ScaleSourceNodeKind::OutsideDomain { .. } => {}
        }
    }
    for (row_index, row) in plan.field_rows().iter().enumerate() {
        let (
            ScaleFieldTarget::SourceNodeRest {
                source_node_index,
                field,
            },
            ScaleFieldDisposition::PreserveExact,
        ) = (row.target, row.disposition)
        else {
            continue;
        };
        let exact = match (
            source_nodes.get(&source_node_index),
            candidate_nodes.get(&source_node_index),
        ) {
            (Some(before), Some(after)) => {
                source_rest_field_bits_equal(&before.local_rest, &after.local_rest, field)
            }
            // Unavailable coverage carries no authoritative raw-row identity;
            // the compiler emits no source field rows for it.
            (None, None) => true,
            _ => false,
        };
        if !exact {
            return Err(ScaleError::CandidateStructureMismatch {
                reason: if connector_sources.contains(&source_node_index) {
                    "connector_source_local_mismatch"
                } else if bridged_successors.contains(&source_node_index) {
                    "bridged_source_local_mismatch"
                } else {
                    "field_disposition_mismatch"
                },
            });
        }
        mark_field_row_discharged(discharged, row_index)?;
    }
    Ok(())
}

fn mark_field_row_discharged(
    discharged: &mut BTreeSet<usize>,
    row_index: usize,
) -> Result<(), ScaleError> {
    if !discharged.insert(row_index) {
        return Err(ScaleError::PlanDocumentMismatch {
            reason: "field_row_discharged_twice",
        });
    }
    Ok(())
}

fn finish_field_row_discharge(
    plan: &ScalePlan,
    discharged: &BTreeSet<usize>,
) -> Result<(), ScaleError> {
    let expected: BTreeSet<_> = (0..plan.field_rows().len()).collect();
    if *discharged != expected {
        return Err(ScaleError::PlanDocumentMismatch {
            reason: "field_row_not_discharged",
        });
    }
    Ok(())
}

// --- Proof -------------------------------------------------------------

mod proof_residual {
    /// One proof claim's maximum residual and the comparisons behind it.
    ///
    /// A maximum of `0.0` can mean either an exact measurement or that no
    /// comparison was made. [`Self::evaluated`] distinguishes those cases.
    /// The two measurements are intentionally one read-only value so a
    /// consumer cannot pair one claim's maximum with another claim's count.
    #[derive(Debug, Clone, Copy, PartialEq)]
    #[non_exhaustive]
    pub struct ScaleProofResidual {
        max: f64,
        comparisons: usize,
    }

    impl ScaleProofResidual {
        /// The maximum residual observed across this claim's comparisons.
        #[must_use]
        pub fn max(self) -> f64 {
            self.max
        }

        /// The number of comparisons behind [`Self::max`].
        #[must_use]
        pub fn comparisons(self) -> usize {
            self.comparisons
        }

        /// Whether the proof evaluated this claim at least once.
        #[must_use]
        pub fn evaluated(self) -> bool {
            self.comparisons != 0
        }

        pub(super) const EMPTY: Self = Self {
            max: 0.0,
            comparisons: 0,
        };

        pub(super) fn record(&mut self, observed: f64) {
            self.max = self.max.max(observed);
            self.comparisons += 1;
        }
    }

    #[cfg(doctest)]
    mod api_contract {
        /// Compile-fail coverage for the removed split API. Each field stays in
        /// its own compilation unit so restoring one cannot be masked by a
        /// different missing field.
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.rest_translation_residual;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.rest_translation_comparisons;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.rest_rotation_residual;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.rest_rotation_comparisons;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.unit_scale_residual;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.unit_scale_comparisons;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.transform_only_affine_residual;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.transform_only_affine_comparisons;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.track_value_residual;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.track_value_comparisons;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.mesh_position_residual;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.mesh_position_comparisons;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.key_translation_residual;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.key_translation_comparisons;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.cubic_interior_residual;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.cubic_interior_comparisons;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.trajectory_residual;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.trajectory_comparisons;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.skin_matrix_residual;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.skin_matrix_comparisons;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.bounds_residual;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.bounds_comparisons;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.unaffected_inverse_bind_residual;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProof;
        ///
        /// fn removed(proof: ScaleProof) {
        ///     let _ = proof.unaffected_inverse_bind_comparisons;
        /// }
        /// ```
        ///
        /// Direct construction and member-by-member mutation are also unavailable:
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProofResidual;
        ///
        /// let _ = ScaleProofResidual {
        ///     max: 0.0,
        ///     comparisons: 1,
        /// };
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProofResidual;
        ///
        /// fn replace_max(mut residual: ScaleProofResidual) {
        ///     residual.max = 1.0;
        /// }
        /// ```
        ///
        /// ```compile_fail
        /// use animsmith_core::ScaleProofResidual;
        ///
        /// fn replace_count(mut residual: ScaleProofResidual) {
        ///     residual.comparisons = 1;
        /// }
        /// ```

        struct RemovedSplitFields;
    }
}

pub use proof_residual::ScaleProofResidual;

/// Observed residual maxima from [`prove_scale`], reported against
/// [`ScalePlan::tolerance_policy`], each paired with the number of
/// comparisons that produced it.
///
/// **Read the paired value, not just its maximum.** A maximum alone cannot
/// distinguish "compared, no deviation" from "nothing to compare": both read
/// `0.0`, because every maximum starts there and is raised only by a loop
/// that may have zero iterations. So a field for an obligation the plan does
/// not require (see [`ScaleProofObligation`]) reports a
/// [`ScaleProofResidual`] whose maximum and count are both zero. A `0.0`
/// maximum with a count above zero is a measurement; a zero count is an
/// absence, which DESIGN.md Appendix D §D.6 requires an evidence record to
/// publish as an absence rather than as a checked zero.
///
/// The counts are measurements, not proxies derived from obligation presence:
/// each is stored with its maximum by the single private recording method
/// every comparison in [`prove_scale`] funnels through. The pair's fields are
/// private, so no producer can combine one claim's count with another claim's
/// maximum.
/// No comparison can raise a residual without being counted, and
/// no count can rise without a comparison having been checked against the
/// tolerance policy. A [`ScaleProofObligation`] may declare a proof walk, or
/// name a row-driven claim discharged through the canonical field inventory;
/// neither role substitutes for the comparison count recorded here.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct ScaleProof {
    /// The tolerance policy every residual below was checked against.
    pub tolerance_policy: ScaleTolerancePolicy,
    /// Maximum rest-world translation residual, with one comparison per
    /// affected node.
    pub rest_translation: ScaleProofResidual,
    /// Maximum rest-world rotation residual, in radians, directly comparable
    /// to [`ScaleTolerancePolicy::rotation_residual_radians`].
    ///
    /// Measured as a double-cover-aware quaternion chord length
    /// `|q1 - q2| = 2 * sin(theta / 4)` and reported as the angle
    /// `theta = 4 * asin(chord / 2)` that chord represents, so this value and
    /// the tolerance it is checked against carry the same unit.
    /// Its count is one per affected node.
    pub rest_rotation: ScaleProofResidual,
    /// Maximum postcondition unit-scale residual, with one comparison per
    /// affected node when the rest/bind plan declares the postcondition.
    pub unit_scale: ScaleProofResidual,
    /// Maximum transform-only attachment full-affine residual (rest/bind
    /// only), with one probe-point comparison per attachment.
    pub transform_only_affine: ScaleProofResidual,
    /// Maximum per-element animation-track value residual, across rewritten
    /// translation elements and every retained rotation/scale element. Its
    /// count is one per track element across every clip.
    pub track_value: ScaleProofResidual,
    /// Maximum per-vertex base mesh `POSITION` residual, with one comparison
    /// per vertex.
    pub mesh_position: ScaleProofResidual,
    /// Maximum residual at any affected-track keyframe time, with one
    /// comparison per affected translation track per key time.
    pub key_translation: ScaleProofResidual,
    /// Maximum residual at any bounded cubic-segment interior time, with one
    /// comparison per affected translation track per interior time.
    pub cubic_interior: ScaleProofResidual,
    /// Maximum sampled world-space trajectory residual, with one comparison
    /// per affected node per sample time.
    pub trajectory: ScaleProofResidual,
    /// Maximum skin-matrix (`W * B`) component residual, across rest and
    /// every sampled key/cubic-interior time. Its count is one per skin slot
    /// of every affected skinned instance at each evaluated pose.
    pub skin_matrix: ScaleProofResidual,
    /// Maximum skinned mesh bounds residual, across rest and every sampled
    /// key/cubic-interior time. Its count is six per evaluated pose.
    pub bounds: ScaleProofResidual,
    /// Maximum stored inverse-bind residual over every skin slot outside the
    /// affected closure (see [`ProofResidualKind::UnaffectedInverseBind`]).
    /// Its count is one per stored slot compared outside the closure and zero
    /// unless the plan declares
    /// [`ScaleProofObligation::UnaffectedInverseBinds`].
    pub unaffected_inverse_bind: ScaleProofResidual,
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
    /// is accurate: `plan.common_factor() * average_affine_axis_length(
    /// affine_axis_lengths(candidate root world linear))` recovers the
    /// measurement to a relative error below `2^-24`, the half-ulp of the one
    /// binary32 rounding that round trip costs at unit magnitude. Swept over
    /// all 215 binary32 values
    /// the `1e-5` band admits around a declared `0.01`, the worst is
    /// `5.9604613e-8`; over this module's own `0.01`-factor fixtures it is
    /// `4.84e-8`, at a source root of `0.010_000_099`.
    ///
    /// It is not the route taken, for four reasons:
    ///
    /// - **Independence.** [`prove_scale`] does not require `candidate` to
    ///   have come from reference construction; checking one that did not
    ///   is the reason it exists. Reading the reported measurement off the
    ///   artifact under test would let that artifact pick its own evidence
    ///   value. The observed factor is a fact about the *input*, so the input
    ///   is where it is measured.
    /// - **Precision.** The candidate route divides by the declared factor in
    ///   `f32` and multiplies back in `f64`, so it reports a rounded
    ///   neighbour of the source measurement rather than the measurement.
    /// - **Sign.** The nearest already-published proxy,
    ///   [`Self::unit_scale`]'s `max()`, is an absolute value, so
    ///   `common_factor * (1 + unit_scale.max())` reconstructs
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
    /// guarantee. The two *chains* the witnesses compose through are already
    /// required to agree: under
    /// [`crate::model::SourceSkeletonCoverage::Complete`] coverage a document
    /// whose projection and skeleton describe different trees is refused
    /// before either witness is taken. What nothing reconciles is the two
    /// *readings* — [`crate::model::SourceNodeAsset::local_rest`] and
    /// [`crate::model::Bone::rest`] stay separately stored and separately
    /// composed, which is why both witnesses exist at all. A divergence
    /// beyond the ceiling is therefore a fact about how far apart the input's
    /// two stored descriptions of one rest pose are, worth surfacing, not a
    /// residual this proof owns.
    pub observed_factor_divergence: f64,
    /// Number of distinct times sampled across all clips.
    pub sample_time_count: usize,
    // Calibration-only raw f32-rounding demand maxima. These are intentionally
    // private and test-only: evidence publishes residuals and comparison
    // counts, while the ignored calibration needs the per-comparison
    // `observed / (base * ulp)` maximum that the proof actually checked.
    // Keeping it on the test build's proof makes calibration consume the
    // production comparison instead of recomputing poses, slots, bounds, or
    // bases, without changing the release proof layout or runtime work.
    #[cfg(test)]
    rest_translation_f32_rounding_demand: f64,
    #[cfg(test)]
    trajectory_f32_rounding_demand: f64,
    #[cfg(test)]
    skin_matrix_f32_rounding_demand: f64,
    #[cfg(test)]
    bounds_f32_rounding_demand: f64,
    #[cfg(test)]
    unaffected_inverse_bind_f32_rounding_demand: f64,
}

/// Independently re-derive and check every claim [`ScalePlan`] makes.
///
/// Proof runs on the in-memory candidate, re-deriving world matrices,
/// sampled trajectories, skin matrices, and bounds from `source` and
/// `candidate` rather than trusting how they were built. Numerical residuals
/// use [`ScaleTolerancePolicy::scalar_tolerance`] computed from that
/// comparison's own actual before/after magnitudes, never a proxy such as the
/// plan's declared factor. Discrete topology and the complete rest-world
/// affine outside a rest/bind closure are exact unchanged-domain invariants.
/// The world comparison is a semantic placement claim; exact local write-set
/// parity is a separate artifact/ledger obligation. Neither `source` nor
/// `candidate` need be numerically identical to the document `plan` was
/// computed against, but re-deriving `source`'s structural planning inventory
/// must produce the same affected domain and proof obligations.
///
/// # Errors
///
/// Returns [`ScaleError::PlanDocumentMismatch`] when the supplied source
/// derives a different proof inventory, any planning/selector error surfaced
/// while re-deriving that inventory, [`ScaleError::CandidateStructureMismatch`]
/// when an exact source/candidate invariant differs,
/// [`ScaleError::ProofResidualExceeded`] for the first residual that exceeds
/// [`ScalePlan::tolerance_policy`], or [`ScaleError::MissingProofEvidence`] if
/// an obligation the plan declares provable has no counterpart evidence in
/// `candidate`.
///
/// Two of the claims checked here are not gated by
/// [`ScaleProofObligation`]: the per-element animation-track values
/// ([`ProofResidualKind::TrackValue`]), base mesh `POSITION`
/// ([`ProofResidualKind::MeshPosition`]). They compare every element of
/// every track and every base mesh against that element's own domain's
/// analytic expectation — the declared multiplier where the plan rewrites
/// that domain, the retained value where it does not — and so are owed by
/// every plan. Base `POSITION` belongs with the track-value comparison, not
/// with the unaffected binds, which is worth naming because it is easy to get
/// backwards: a whole-document plan *does* rewrite it
/// ([`ScaleRewriteRule::WholeDocumentLength`]), so its comparison is a
/// rewritten-value check, and it is unconditional because skinned bounds
/// would otherwise be its only witness — and they report a zero residual for
/// a document carrying no skinned instance at all. Neither admits
/// an obligation flag as a proxy for having run — see [`ScaleProof`], whose
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
    let candidate = candidate.document();
    validate_plan_document_inventory(source, plan)?;
    validate_scale_input(candidate)?;
    validate_candidate_structure(source, candidate)?;
    let mut discharged_field_rows = BTreeSet::new();
    let tol = plan.tolerance_policy;
    let affected = plan.affected_set();
    let affected_skin_instances = if plan.has_skin_and_bounds() {
        affected_skin_instance_indices(source, &affected)
    } else {
        Vec::new()
    };
    let source_worlds = rest_world_pose(&source.skeleton)?;
    let candidate_worlds = rest_world_pose(&candidate.skeleton)?;

    // Rest/bind rewrites a strict hierarchy domain while promising that every
    // bone outside it keeps the same world rest. This is exact, not a new
    // tolerance: unchanged placement is the operation's semantic invariant,
    // and a relative tolerance would only admit larger and larger displacement
    // as authored coordinates grow. Compare the complete affine so an
    // in-place rotation or scale mutation cannot hide behind an unchanged
    // origin. Exact local-field/write-set parity is intentionally not inferred
    // from equal matrices; that belongs to the explicit artifact/ledger layer.
    //
    // Whole-document conversion has no complement, so the loop is naturally
    // empty there; topology parity still applies because that operation does
    // not rewrite parents either.
    if plan.has_obligation(ScaleProofObligation::ExactUnchangedWorldRest) {
        for node in (0..source.skeleton.bones.len()).filter(|node| !affected.contains(node)) {
            let before = source_worlds.bone(node)?.matrix;
            let after = candidate_worlds.bone(node)?.matrix;
            if before != after {
                return Err(ScaleError::CandidateStructureMismatch {
                    reason: "unaffected_world_rest_mismatch",
                });
            }
        }
    }
    let observed_factor = observed_factor_from_source(source, &source_worlds, plan)?;
    let mut proof = ScaleProof {
        tolerance_policy: tol,
        rest_translation: ScaleProofResidual::EMPTY,
        rest_rotation: ScaleProofResidual::EMPTY,
        unit_scale: ScaleProofResidual::EMPTY,
        transform_only_affine: ScaleProofResidual::EMPTY,
        track_value: ScaleProofResidual::EMPTY,
        mesh_position: ScaleProofResidual::EMPTY,
        key_translation: ScaleProofResidual::EMPTY,
        cubic_interior: ScaleProofResidual::EMPTY,
        trajectory: ScaleProofResidual::EMPTY,
        skin_matrix: ScaleProofResidual::EMPTY,
        bounds: ScaleProofResidual::EMPTY,
        unaffected_inverse_bind: ScaleProofResidual::EMPTY,
        observed_factor,
        planned_observed_factor: plan.observed_factor,
        observed_factor_divergence: relative_divergence(plan.observed_factor, observed_factor),
        sample_time_count: 0,
        #[cfg(test)]
        rest_translation_f32_rounding_demand: 0.0,
        #[cfg(test)]
        trajectory_f32_rounding_demand: 0.0,
        #[cfg(test)]
        skin_matrix_f32_rounding_demand: 0.0,
        #[cfg(test)]
        bounds_f32_rounding_demand: 0.0,
        #[cfg(test)]
        unaffected_inverse_bind_f32_rounding_demand: 0.0,
    };

    check_candidate_values(
        source,
        candidate,
        &affected,
        plan,
        &tol,
        &mut proof,
        &mut discharged_field_rows,
    )?;

    if let Some((rest_nodes, prove_unit_scale)) = plan.rest_obligation() {
        for &node in rest_nodes {
            let before = source_worlds.bone(node)?.matrix;
            let after_pose = candidate_worlds.bone(node)?;
            let after = after_pose.matrix;
            let after_chain = after_pose.translation_rounding_magnitude;
            let (translation_residual, before_mag, after_mag) = rest_node_residual(
                before,
                after,
                plan.is_whole_document(),
                plan.common_factor(),
            );
            // The magnitude is the parent chain's, not the surviving
            // translation's: a joint whose local offset points back along its
            // parent's world translation leaves a world translation the
            // difference of two much larger terms, carrying their rounding
            // error into a comparison whose own operands are small.
            //
            // The *candidate's* chain, not the source's and not the `max` of
            // the two. The residual is measured against the candidate's
            // arithmetic: whole-document conversion scales every translation
            // by the factor and leaves every linear part alone, so the two
            // chains are that factor apart — subject to the candidate's `f32`
            // narrowing of the factor, since the build scales by
            // `factor as f32` while this proof rebases by the `f64` factor, a
            // relative difference of at most `2^-24` (`1.49e-8` at `q = 0.1`,
            // whose `f32` is `0.10000000149011612`) that the rounding term
            // covers many times over — and the source's rounding is rebased by
            // the same factor before it is compared. The
            // residual therefore scales with `after_chain` at either end of
            // the factor range, and under rest/bind the two chains are equal
            // outright. `before_chain` can only ever over-provide — and under
            // a *shrinking* conversion it over-provides by `1/factor`, freezing
            // the band at the source rig's size while the candidate the band
            // is spent on gets smaller without limit.
            //
            // `a_cancelling_chain_under_conversion_holds_rest_translation_to_the_candidate_side`
            // pins that reading the *source* side alone refuses a correct
            // candidate under a growing conversion;
            // `a_shrinking_conversion_holds_rest_translation_to_the_candidate_s_own_chain`
            // pins the opposite direction, where the source side is the larger
            // one and reading it admits a `100x` larger build error; and
            // `the_rest_translation_v6_floor_is_an_adjacent_f32_transition`
            // pins the size of the term from above.
            check_and_track_f32_rounded(
                ProofResidualKind::RestTranslation,
                translation_residual,
                before_mag,
                after_mag,
                after_chain,
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
            if prove_unit_scale {
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
    // The rest-world handler above is the semantic owner of all normalized
    // rest containers. Translation/rotation have direct residuals; scale is
    // intentionally owned at composed-world level (and, for rest/bind, by
    // the unit-scale postcondition) rather than by a new local-value band.
    for (row_index, row) in plan.field_rows().iter().enumerate() {
        if matches!(row.target, ScaleFieldTarget::BoneRest { .. }) {
            mark_field_row_discharged(&mut discharged_field_rows, row_index)?;
        }
    }

    if let Some(transform_only_nodes) = plan.transform_only_nodes() {
        // scale(1/s): the analytically expected basis correction `C_i` for
        // every node inside the affected domain (DESIGN.md Appendix D §D.2).
        let correction = Mat4::from_scale(Vec3::splat((1.0 / plan.common_factor()) as f32));
        // A fixed off-origin local probe point: transforming it through the
        // complete expected/actual world affine — rather than decomposing
        // to translation/rotation and checking only those — is what makes a
        // no-op (or any build that drops the linear-scale channel) provably
        // fail this check.
        let probe = Vec3::ONE;
        for &node in transform_only_nodes {
            let before = source_worlds.bone(node)?.matrix;
            let after = candidate_worlds.bone(node)?.matrix;
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
        &affected_skin_instances,
        plan,
        &tol,
        &mut proof,
    )?;
    if plan.has_unaffected_binds() {
        check_unaffected_instance_binds(source, candidate, &affected, &tol, &mut proof)?;
    }
    // The shared skin walk and unaffected-bind walk are the existing numeric
    // owners for stored slot binds. Bone convenience binds that are
    // unreferenced or shadowed, plus normals that neither scale operation
    // writes, remain ownership-only rows: changing their numeric policy here
    // would alter the accepted set.
    for (row_index, row) in plan.field_rows().iter().enumerate() {
        if matches!(
            row.target,
            ScaleFieldTarget::BoneInverseBind { .. }
                | ScaleFieldTarget::InstanceInverseBind { .. }
                | ScaleFieldTarget::MeshNormals { .. }
        ) {
            mark_field_row_discharged(&mut discharged_field_rows, row_index)?;
        }
    }

    let any_sampled_obligation = plan.has_key_translations()
        || plan.has_cubic_interiors()
        || plan.trajectory_nodes().is_some()
        || plan.has_skin_and_bounds();
    if any_sampled_obligation {
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
        let per_sample_cost = per_sample_work_units(source, &affected_skin_instances);
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
                if plan.has_key_translations() {
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
                    &affected_skin_instances,
                    plan,
                    &tol,
                    &mut proof,
                )?;
            }
            for &t in interior_times {
                proof.sample_time_count += 1;
                if plan.has_cubic_interiors() {
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
                    &affected_skin_instances,
                    plan,
                    &tol,
                    &mut proof,
                )?;
            }
        }
    }

    check_rewritten_source_field_dispositions(
        source,
        candidate,
        plan,
        &affected,
        &tol,
        &mut discharged_field_rows,
    )?;
    check_preserved_field_dispositions(source, candidate, plan, &mut discharged_field_rows)?;
    finish_field_row_discharge(plan, &discharged_field_rows)?;
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
    affected_skin_instances: &[usize],
    plan: &ScalePlan,
    tol: &ScaleTolerancePolicy,
    proof: &mut ScaleProof,
) -> Result<(), ScaleError> {
    if plan.trajectory_nodes().is_none() && !plan.has_skin_and_bounds() {
        return Ok(());
    }
    let source_worlds = world_at_time(&source.skeleton, source_clip, t)?;
    let candidate_worlds = world_at_time(&candidate.skeleton, candidate_clip, t)?;
    if let Some(nodes) = plan.trajectory_nodes() {
        check_trajectory_residual_at(&source_worlds, &candidate_worlds, nodes, plan, tol, proof)?;
    }
    check_skin_and_bounds(
        source,
        candidate,
        &source_worlds,
        &candidate_worlds,
        affected_skin_instances,
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
///   [`crate::model::MeshInstance::skin_joints`] slot per side, plus one residual
///   comparison per slot when the skin obligation is declared; and
/// - per affected skinned instance, every vertex of **every** primitive of
///   its mesh per side, when the bounds obligation is declared.
///
/// The slot term is charged explicitly because nothing bounds it and nothing
/// else stands in for it. An earlier revision charged only `bone_count +
/// vertices` on the claim that slot work "cannot exceed the bone count",
/// which is false twice over: [`validate_scale_input`] only range-checks
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
fn per_sample_work_units(document: &Document, affected_skin_instances: &[usize]) -> u64 {
    let mut units = PROOF_SIDES.saturating_mul(document.skeleton.bones.len() as u64);
    for &instance_index in affected_skin_instances {
        let instance = &document.assets.instances[instance_index];
        let slots = instance.skin_joints.len() as u64;
        units = units.saturating_add(PROOF_SIDES.saturating_mul(slots));
        units = units.saturating_add(slots);
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

impl ScaleProof {
    /// Record the raw ulp demand of one f32-rounded comparison at the exact
    /// point its residual and rounding base meet.
    ///
    /// This is calibration instrumentation, not published evidence. A zero
    /// base and zero residual make no demand on the rounding count; a nonzero
    /// residual with no provenance records infinity so calibration fails
    /// closed instead of silently reporting zero.
    #[cfg(test)]
    fn record_f32_rounding_demand(
        &mut self,
        kind: ProofResidualKind,
        observed: f64,
        magnitude: f64,
    ) {
        let demand = if magnitude > 0.0 {
            observed / (magnitude * f64::from(f32::EPSILON))
        } else if observed == 0.0 {
            0.0
        } else {
            f64::INFINITY
        };
        let slot = match kind {
            ProofResidualKind::RestTranslation => &mut self.rest_translation_f32_rounding_demand,
            ProofResidualKind::Trajectory => &mut self.trajectory_f32_rounding_demand,
            ProofResidualKind::SkinMatrix => &mut self.skin_matrix_f32_rounding_demand,
            ProofResidualKind::Bounds => &mut self.bounds_f32_rounding_demand,
            ProofResidualKind::UnaffectedInverseBind => {
                &mut self.unaffected_inverse_bind_f32_rounding_demand
            }
            _ => unreachable!("only f32-rounded residual kinds record a raw rounding demand"),
        };
        *slot = slot.max(demand);
    }

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
    fn tally(&mut self, kind: ProofResidualKind) -> Option<&mut ScaleProofResidual> {
        let tally = match kind {
            ProofResidualKind::RestTranslation => &mut self.rest_translation,
            ProofResidualKind::RestRotation => &mut self.rest_rotation,
            ProofResidualKind::UnitScale => &mut self.unit_scale,
            ProofResidualKind::TransformOnlyAffine => &mut self.transform_only_affine,
            ProofResidualKind::TrackValue => &mut self.track_value,
            ProofResidualKind::MeshPosition => &mut self.mesh_position,
            ProofResidualKind::KeyTranslation => &mut self.key_translation,
            ProofResidualKind::CubicInterior => &mut self.cubic_interior,
            ProofResidualKind::Trajectory => &mut self.trajectory,
            ProofResidualKind::SkinMatrix => &mut self.skin_matrix,
            ProofResidualKind::Bounds => &mut self.bounds,
            ProofResidualKind::UnaffectedInverseBind => &mut self.unaffected_inverse_bind,
            ProofResidualKind::ObservedFactor => return None,
        };
        Some(tally)
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
    if let Some(tally) = proof.tally(kind) {
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

/// [`check_and_track`] for a residual between two `f32`-rounded quantities,
/// carrying the `magnitude` their arithmetic actually ran on.
///
/// Only the five obligations whose compared quantity can be made arbitrarily
/// smaller than that magnitude by a rotation use this — see
/// [`ScaleTolerancePolicy::f32_rounding_ulps`]. Every other obligation
/// compares a vector length or a matrix entry against its own magnitude,
/// where the two are the same number and the extra term would be noise.
fn check_and_track_f32_rounded(
    kind: ProofResidualKind,
    observed: f64,
    before: f64,
    after: f64,
    magnitude: f64,
    tol: &ScaleTolerancePolicy,
    proof: &mut ScaleProof,
) -> Result<(), ScaleError> {
    #[cfg(test)]
    proof.record_f32_rounding_demand(kind, observed, magnitude);
    record_and_check(
        kind,
        observed,
        tol.f32_rounded_tolerance(before, after, magnitude),
        proof,
    )
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
        return plan.common_factor();
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
        Some(parent) if affected.contains(&parent) => plan.common_factor(),
        _ => 1.0,
    }
}

/// Independently derive the scale-track boundary root the proof expects.
///
/// This deliberately does not call the reference writer's scale-animation
/// multiplier: the builder and proof must derive the selected-root boundary
/// separately, or a wrong builder helper could leave a root scale track
/// unchanged and teach the proof to accept it.
fn proof_scale_animation_root(
    source: &Document,
    plan: &ScalePlan,
) -> Result<Option<BoneId>, ScaleError> {
    let ScaleOperation::RestBindUniformScale {
        source_root_node_index,
        ..
    } = plan.operation()
    else {
        return Ok(None);
    };
    // Unlike the builder's parent/affected-boundary derivation, proof starts
    // from the operation's authored root selector and the source projection.
    // Agreement therefore requires two independent descriptions of which
    // bone owns the only non-unit local scale multiplier.
    let selected_root = source
        .assets
        .source_skeleton
        .nodes
        .iter()
        .find(|asset| asset.source_node_index == source_root_node_index)
        .and_then(|asset| asset.bone)
        .ok_or(ScaleError::PlanDocumentMismatch {
            reason: "selected_root_projection_mismatch",
        })?;
    Ok(Some(selected_root))
}

/// Prove every retained per-element payload directly, not merely its shape.
///
/// [`validate_candidate_structure`] establishes that source and candidate
/// agree on clip/track/instance/mesh/primitive *counts* and on each track's
/// `(bone, property, interpolation, times)` identity — but it never looks
/// inside `values` or `positions`. Both are reachable through this module's
/// public API without any structural mismatch:
/// [`ScaleCandidate::from_document`] accepts an external document
/// independently of the source supplied to [`prove_scale`], so a doctored
/// candidate can be proved against the real source. Without a direct
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
    discharged: &mut BTreeSet<usize>,
) -> Result<(), ScaleError> {
    let proof_scale_root = proof_scale_animation_root(source, plan)?;
    for (row_index, row) in plan.field_rows().iter().enumerate() {
        match row.target {
            ScaleFieldTarget::AnimationValues {
                clip_index,
                track_index,
                bone,
                property,
            } => {
                let track = &source.clips[clip_index].tracks[track_index];
                let candidate_track = &candidate.clips[clip_index].tracks[track_index];
                if track.bone != bone || track.property != property {
                    return Err(ScaleError::PlanDocumentMismatch {
                        reason: "compiled_animation_target_mismatch",
                    });
                }
                match (&track.values, &candidate_track.values) {
                    (TrackValues::Vec3s(before), TrackValues::Vec3s(after)) => {
                        let multiplier = match row.disposition {
                            ScaleFieldDisposition::PreserveExact => 1.0,
                            ScaleFieldDisposition::Rewrite(
                                ScaleRewriteRule::WholeDocumentLength,
                            ) => plan.common_factor(),
                            ScaleFieldDisposition::Rewrite(
                                ScaleRewriteRule::RestBindParentBasis,
                            ) => translation_multiplier(source, bone, affected, plan),
                            ScaleFieldDisposition::Rewrite(
                                ScaleRewriteRule::RestBindLocalScale,
                            ) => {
                                if proof_scale_root == Some(bone) {
                                    1.0 / plan.common_factor()
                                } else {
                                    1.0
                                }
                            }
                            ScaleFieldDisposition::Rewrite(
                                ScaleRewriteRule::RestBindNodeBasis
                                | ScaleRewriteRule::RestBindSourceLocal { .. },
                            ) => {
                                return Err(ScaleError::PlanDocumentMismatch {
                                    reason: "invalid_animation_rewrite_rule",
                                });
                            }
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
                mark_field_row_discharged(discharged, row_index)?;
            }
            ScaleFieldTarget::MeshPositions {
                mesh_index,
                primitive_index,
            } => {
                // Base `POSITION` is proved per primitive, directly. Proving
                // it only through skinned bounds would miss interior vertices
                // and every unskinned instance.
                let source_primitive =
                    &source.assets.meshes[mesh_index].primitives[primitive_index];
                let candidate_primitive =
                    &candidate.assets.meshes[mesh_index].primitives[primitive_index];
                let position_multiplier = match row.disposition {
                    ScaleFieldDisposition::PreserveExact => 1.0,
                    ScaleFieldDisposition::Rewrite(ScaleRewriteRule::WholeDocumentLength) => {
                        plan.common_factor()
                    }
                    ScaleFieldDisposition::Rewrite(_) => {
                        return Err(ScaleError::PlanDocumentMismatch {
                            reason: "invalid_mesh_position_rewrite_rule",
                        });
                    }
                };
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
                mark_field_row_discharged(discharged, row_index)?;
            }
            _ => {}
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
/// together, and [`ScaleProof::rest_rotation`] is headed for the
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
    source_worlds: &WorldPose,
    candidate_worlds: &WorldPose,
    affected_nodes: &[BoneId],
    plan: &ScalePlan,
    tol: &ScaleTolerancePolicy,
    proof: &mut ScaleProof,
) -> Result<(), ScaleError> {
    for &node in affected_nodes {
        let before = source_worlds.bone(node)?.matrix;
        let after_pose = candidate_worlds.bone(node)?;
        let after = after_pose.matrix;
        let after_chain = after_pose.translation_rounding_magnitude;
        let (translation_residual, before_mag, after_mag) = rest_node_residual(
            before,
            after,
            plan.is_whole_document(),
            plan.common_factor(),
        );
        // The same magnitude the unanimated `RestTranslation` comparison
        // takes — the *candidate's* sampled parent chain, read off the sampled
        // pose this residual was composed from rather than the rest pose: the
        // two obligations differ only in which locals the chain ran on, and
        // the argument for reading the candidate side alone is the one stated
        // there.
        //
        // Reaching this term at all needs a rig whose *sampled* parent chain
        // cancels, which a clip over a rest pose that already cancels is the
        // simplest way to build:
        // `a_sampled_pose_whose_parent_chain_cancels_still_proves_its_trajectory`
        // and `the_trajectory_v6_floor_is_an_adjacent_f32_transition`
        // are those fixtures, and
        // `a_shrinking_conversion_holds_trajectory_to_the_candidate_s_own_chain`
        // is the one that separates the candidate's chain from the source's.
        // Without such a rig the only comparison this obligation makes has
        // `chain = 0` on both sides, and every mutation of the term is a no-op
        // on a quantity that is arithmetically absent.
        check_and_track_f32_rounded(
            ProofResidualKind::Trajectory,
            translation_residual,
            before_mag,
            after_mag,
            after_chain,
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
/// invariant [`crate::model::world_rest_matrices`]
/// enforces for the unanimated rest pose.
fn world_at_time(skeleton: &Skeleton, clip: &Clip, t: f32) -> Result<WorldPose, ScaleError> {
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
    let mut bones: Vec<WorldBonePose> = Vec::with_capacity(bone_count);
    for (index, bone) in skeleton.bones.iter().enumerate() {
        let local = locals[index].to_mat4();
        if !mat4_is_finite(local) {
            return Err(ScaleError::NonFiniteTransform { node: index });
        }
        let pose = match bone.parent {
            Some(parent) if parent < index => {
                // Accumulated in the same walk the composition runs in, so
                // the chain costs one `Mat4 * Vec4` per bone rather than a
                // second pass that would have to recompose every local.
                let parent_pose = bones[parent];
                let matrix = parent_pose.matrix * local;
                WorldBonePose {
                    matrix,
                    translation_rounding_magnitude: child_translation_rounding_magnitude(
                        parent_pose,
                        local,
                    ),
                }
            }
            Some(parent) => {
                return Err(ScaleError::InvalidParent {
                    node: index,
                    parent,
                });
            }
            None => WorldBonePose {
                matrix: local,
                translation_rounding_magnitude: 0.0,
            },
        };
        if !mat4_is_finite(pose.matrix) {
            return Err(ScaleError::NonFiniteTransform { node: index });
        }
        bones.push(pose);
    }
    Ok(WorldPose { bones })
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
/// [`planning::classify_affine`] proved non-singular — so the base is at least
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
/// the same way `planning::plan_rest_bind` resolves it — by `source_root_node_index`
/// through `source`'s own source-node projection — and its factor is then
/// measured from the *normalized* skeleton's rest-world matrix rather than
/// from the raw projection planning classified. That makes this a genuinely
/// second witness: it reads different stored data, through a different
/// composition path, and it is computed from whichever `source` this call was
/// handed, which [`prove_scale`] does not require to be the document the plan
/// came from.
///
/// # The scaled root is the minimum [`BoneId`] in the closure
///
/// §D.6 defines the observed factor *at the scaled root*, and "the lowest
/// affected bone id" is the reading a source-node-space walk and a
/// `BoneId`-space walk could otherwise land on separately. Once
/// [`crate::model::validate_document_shape`] holds they are the same node, and no
/// document can distinguish them:
///
/// - every insertion [`validation::rest_bind_affected_closure`] makes is the root itself,
///   a node on the ancestor path from a joint *up to* the root, or a BFS
///   descendant of something already in the set — so the closure lies inside
///   `subtree(root)` in source-node space;
/// - chain agreement is a parent-preserving injection, so it carries
///   `subtree(root)` onto `subtree(bone(root))` in `BoneId` space;
/// - [`crate::model::world_rest_matrices`] refuses a skeleton in which a parent's id is not
///   strictly less than its child's, so every member of `subtree(b)` has an
///   id at least `b`.
///
/// The scaled root is therefore the strict minimum id in the closure, and
/// reading either one reports the same number. That is a consequence of the
/// agreement precondition rather than of this function: without it the two
/// readings genuinely differ, and the difference is a false proof rather than
/// a naming quibble.
///
/// It is deliberately *not* a re-run of [`planning::classify_affine`]. Re-classifying
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
    source_worlds: &WorldPose,
    plan: &ScalePlan,
) -> Result<f64, ScaleError> {
    let ScaleOperation::RestBindUniformScale {
        source_root_node_index,
        ..
    } = plan.operation()
    else {
        // Whole-document conversion declares its factor rather than observing
        // it; see [`ScalePlan::observed_factor`].
        return Ok(plan.common_factor());
    };
    let bone = source_node_index_map(source)
        .get(&source_root_node_index)
        .and_then(|asset| asset.bone)
        .ok_or(ScaleError::MissingProofEvidence {
            kind: ProofResidualKind::ObservedFactor,
            detail: "scaled_root_not_projected",
        })?;
    let world = source_worlds.bone(bone)?.matrix;
    Ok(average_affine_axis_length(affine_axis_lengths(
        Mat3::from_mat4(world),
    )))
}

/// Prove that inverse-bind evidence *outside* the affected closure came
/// through unchanged.
///
/// [`check_skin_and_bounds`] skips an instance with no joint in the affected
/// closure entirely, and nothing else looked at one either — so a candidate
/// that rewrote an unrelated skin's `skin_ibms` proved `Ok`. Reachable
/// through the public API, since [`ScaleCandidate::from_document`] and
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
/// exactly the rewrite the reference rest/bind writer performs on those
/// slots. Holding such an instance to "binds unchanged" as well rejects this
/// module's own output — pinned by
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
/// bind — and is exactly what the reference rest/bind writer does for an
/// *affected* instance, deliberately — yet is refused here as
/// `MissingProofEvidence { source_slot_bind_missing }`, and the converse as
/// `candidate_slot_bind_missing`.
///
/// This is not reachable through the shipped pipeline: the reference
/// rest/bind writer materializes an array only for an instance with an
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
/// Unconditional, like the per-element comparisons in
/// [`check_candidate_values`], though for its own reason: it is a structural
/// claim about payloads the plan declares *unaffected*, so there is no
/// obligation flag that could switch it off without also making the plan's
/// "unaffected" claim unfalsifiable. Base `POSITION` is unconditional on the
/// other ground — a whole-document plan *does* rewrite it, and its only other
/// witness reports zero for a document with no skinned instance.
fn check_unaffected_instance_binds(
    source: &Document,
    candidate: &Document,
    affected: &BTreeSet<BoneId>,
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
            // Any slot reaching this function is outside a rest/bind closure
            // and therefore unchanged. A valid whole-document plan covers
            // every current bone; `validate_plan_document_inventory` rejects
            // stale replay before an added bone could reach this walk.
            let expected = before;
            let residual = matrix_residual(expected, after);
            // Both sides are stored matrices, so the magnitude the
            // comparison rounded against *is* the magnitude being compared:
            // `scale_translation_only` scales a column, it does not cancel
            // two terms the way composing `W * B` does, and a rotation
            // cannot make one of these entries small while its error stays
            // large. The rounding term is passed for the same base the
            // relative band already uses, which is what makes it inert here
            // — measured at `0` ulps across the whole rotation sweep,
            // because a candidate this obligation reads was produced by the
            // identical `f32` expression on the identical stored inputs. It
            // is stated rather than omitted so the policy quantity means one
            // thing across every obligation that compares `f32` matrices.
            let magnitude = matrix_magnitude(expected).max(matrix_magnitude(after));
            check_and_track_f32_rounded(
                ProofResidualKind::UnaffectedInverseBind,
                residual,
                matrix_magnitude(expected),
                matrix_magnitude(after),
                magnitude,
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
    source_worlds: &WorldPose,
    candidate_worlds: &WorldPose,
    affected_skin_instances: &[usize],
    plan: &ScalePlan,
    tol: &ScaleTolerancePolicy,
    proof: &mut ScaleProof,
) -> Result<(), ScaleError> {
    if !plan.has_skin_and_bounds() {
        return Ok(());
    }

    let mut source_bounds = BoundsAccumulator::default();
    let mut candidate_bounds = BoundsAccumulator::default();

    // The factor the source side is rebased by before it is compared, and so
    // the factor its *rounding* is rebased by too. Both obligations below take
    // their comparison base as `candidate.max(q * source)` for this reason —
    // see the note at the skin-matrix call. `1.0` for rest/bind, where the two
    // documents state the same world in the same units and the rebasing is a
    // no-op.
    let q = if plan.is_whole_document() {
        plan.common_factor()
    } else {
        1.0
    };

    for &instance_index in affected_skin_instances {
        let instance = &source.assets.instances[instance_index];
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
            let before_pose = source_worlds.bone(joint)?;
            let after_pose = candidate_worlds.bone(joint)?;
            let before_world = before_pose.matrix;
            let before_chain = before_pose.translation_rounding_magnitude;
            let after_world = after_pose.matrix;
            let after_chain = after_pose.translation_rounding_magnitude;
            let before_ibm = instance_bind(source, instance, slot, joint)?;
            let after_ibm = instance_bind(candidate, candidate_instance, slot, joint)?;
            source_slots.push(SkinSlot::compose(before_world, before_ibm, before_chain));
            candidate_slots.push(SkinSlot::compose(after_world, after_ibm, after_chain));
        }

        for (before, after) in source_slots.iter().zip(candidate_slots.iter()) {
            // Whole-document conversion scales every affine's translation
            // by the declared factor while leaving its linear part
            // unchanged (the same `U M U^-1` conjugation as any other
            // retained matrix); rest/bind reparameterization analytically
            // preserves the skin equation exactly.
            let expected = if plan.is_whole_document() {
                scale_translation_only(before.matrix, plan.common_factor() as f32)
            } else {
                before.matrix
            };
            let residual = matrix_residual(expected, after.matrix);
            // The candidate's own composition magnitude, and the source's
            // *rebased by the factor*. The residual is `|after - q *
            // before|`, so the source operand enters the comparison
            // multiplied by `q` and its rounding is multiplied by `q` with
            // it: a source slot accurate to `k` ulps of its own magnitude
            // contributes `q * k` ulps of that magnitude here. A base that
            // reads the source side unrebased — which `max(before, after)`
            // did — therefore states the source's error in the wrong units
            // by a factor of `q`.
            //
            // Under a *shrinking* conversion that is the whole defect: the
            // unrebased source magnitude is `1/q` times too large, so the
            // band freezes at the source rig's size while the candidate it
            // is spent on keeps shrinking. Measured over the sweep
            // populations below the recovered discriminating power is the
            // factor exactly — `100x` at `0.01`, `10_000x` at `1e-4`.
            //
            // Unlike the parent-chain case this does **not** reduce to the
            // candidate's magnitude alone, because `q * before` is not
            // bounded by `after`: the two magnitudes are a factor apart
            // only in the terms that carry a translation, and both retain
            // an unscaled `O(1)` floor from the composition's linear block
            // and the homogeneous row. Where that floor dominates the
            // source — small joints carrying small geometry — `q * before`
            // exceeds `after` by up to the full factor under a growing
            // conversion.
            //
            // `q * before` is written anyway because it is the bound that
            // can be argued from the operands rather than measured from a
            // population: a source slot accurate to the count's own budget
            // contributes `q` times that budget here, whatever cancels.
            // `a_growing_conversion_provisions_a_rebased_source_magnitude`
            // pins the regime where this rebased source term exceeds the
            // candidate term by orders of magnitude.
            let magnitude = after.rounding_magnitude.max(q * before.rounding_magnitude);
            check_and_track_f32_rounded(
                ProofResidualKind::SkinMatrix,
                residual,
                matrix_magnitude(expected),
                matrix_magnitude(after.matrix),
                magnitude,
                tol,
                proof,
            )?;
        }

        let mesh = source.assets.meshes.get(instance.mesh).ok_or(
            DocumentShapeError::MeshInstanceShape {
                instance_index,
                violation: MeshInstanceShapeViolation::MeshIndexOutOfRange,
            },
        )?;
        let candidate_mesh = candidate.assets.meshes.get(candidate_instance.mesh).ok_or(
            DocumentShapeError::MeshInstanceShape {
                instance_index,
                violation: MeshInstanceShapeViolation::MeshIndexOutOfRange,
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

    let source_bounds_magnitude = source_bounds.rounding_magnitude();
    let candidate_bounds_magnitude = candidate_bounds.rounding_magnitude();
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
    // One magnitude for all six comparisons, never the corner a residual lands
    // on: that corner is not evidence about the arithmetic that produced it. A
    // per-axis extreme is contributed by whichever vertex happened to be
    // furthest along that axis, and three vertices at `(3000, .001, .002)`,
    // `(.001, 3000, .003)` and `(.002, .003, 3000)` build a corner of magnitude
    // `2.4e-3` out of vertices of magnitude `3000` — so a base read off the
    // corner would be a million times smaller than the rounding error the
    // corner carries.
    //
    // The candidate's magnitude against the source's *rebased by the factor*,
    // for the reason the skin-matrix call states in full: the comparison below
    // is `|a - q * b|`, so the source bound's rounding enters it multiplied by
    // `q`. `max(source, candidate)` stated that rounding in the source rig's
    // units and was loose by `1/q` under a shrinking conversion — `100x` at
    // `0.01` and `10_000x` at `1e-4`, both recovered here.
    let magnitude = candidate_bounds_magnitude.max(q * source_bounds_magnitude);
    for (before, after) in [(before_min, after_min), (before_max, after_max)] {
        let before = before.to_array();
        let after = after.to_array();
        for axis in 0..3 {
            let b = before[axis] as f64;
            let a = after[axis] as f64;
            let expected = b * q;
            let residual = (a - expected).abs();
            check_and_track_f32_rounded(
                ProofResidualKind::Bounds,
                residual,
                expected,
                a,
                magnitude,
                tol,
                proof,
            )?;
        }
    }
    Ok(())
}

/// One skin slot's composed `W * B`, together with the magnitude that
/// composition rounded against.
///
/// The two travel together because a caller that has one without the other
/// cannot state a tolerance for anything derived from it: `matrix` is
/// near-identity for a bind-pose slot no matter how far from the origin the
/// joint sits, while `rounding_magnitude` is where the arithmetic actually
/// happened (see [`product_operand_magnitude`]).
#[derive(Debug, Clone, Copy)]
struct SkinSlot {
    matrix: Mat4,
    /// [`mat4_abs`] of [`Self::matrix`], for
    /// [`column_operand_magnitude`]'s per-vertex use.
    ///
    /// Held here rather than taken per vertex because it is constant across
    /// every vertex the slot influences and the loop that reads it is the
    /// hottest in this proof.
    absolute: Mat4,
    rounding_magnitude: f64,
}

impl SkinSlot {
    /// Compose `world * inverse_bind`, carrying the larger of the two
    /// magnitudes the result's error can come from.
    ///
    /// `world_translation_rounding_magnitude` is the accumulated provenance
    /// for this slot's joint. The fixed chain and `W * B` stages retain the
    /// measured policy's maximum here: #337 changes the unbounded number of
    /// links *inside* the incoming chain, not this fixed two-stage envelope.
    /// Both terms remain load-bearing in the calibrated corpus; replacing the
    /// envelope with an analytic componentwise error propagation is a broader
    /// model, not a larger scalar sum hidden in this constructor.
    fn compose(world: Mat4, inverse_bind: Mat4, world_translation_rounding_magnitude: f64) -> Self {
        let matrix = world * inverse_bind;
        Self {
            matrix,
            absolute: mat4_abs(matrix),
            rounding_magnitude: product_operand_magnitude(world, inverse_bind)
                .max(world_translation_rounding_magnitude),
        }
    }
}

/// Running skinned-bounds extremes for one document side, and the largest
/// magnitude the `f32` arithmetic behind them ran on.
///
/// `touched` distinguishes "every relevant vertex was unweighted" from "the
/// bounds happen to be at the origin": the former has no bounds evidence at
/// all and must be reported as missing, not as a zero residual.
///
/// `rounding_magnitude` is what
/// [`ScaleTolerancePolicy::f32_rounding_ulps`] is counted in for
/// [`ProofResidualKind::Bounds`]. For each contributing influence, it is the
/// larger of the magnitude the `W * B * p` transform ran on and the slot's
/// [`SkinSlot::rounding_magnitude`]. The former is
/// [`column_operand_magnitude`] of the composed slot against `p` extended by
/// the homogeneous `1` — the *product* `abs(W * B) * abs(p)` and not either
/// factor alone. The latter carries the two earlier stages: the composition
/// that produced `W * B`, whose translation column may cancel large `W` and
/// `B` terms, and the parent chain that produced `W`, whose translation may
/// already contain cancellation.
///
/// The per-influence magnitudes are combined with the same binary64 weighted
/// average of the stored, non-negative binary32 weights as the skinned point.
/// A tiny influence therefore carries only its proportional arithmetic
/// provenance; taking a plain max would let an arbitrarily small weight on a
/// distant joint widen the whole bound tolerance. The blended point's own
/// per-axis magnitude is consequently already bounded by the weighted
/// transform operands and needs no separate L2 stage. See DESIGN.md Appendix
/// D §D.1.
struct BoundsAccumulator {
    min: Vec3,
    max: Vec3,
    touched: bool,
    rounding_magnitude: f64,
}

impl Default for BoundsAccumulator {
    fn default() -> Self {
        Self {
            min: Vec3::splat(f32::INFINITY),
            max: Vec3::splat(f32::NEG_INFINITY),
            touched: false,
            rounding_magnitude: 0.0,
        }
    }
}

impl BoundsAccumulator {
    fn finish(self) -> Option<(Vec3, Vec3)> {
        self.touched.then_some((self.min, self.max))
    }

    fn rounding_magnitude(&self) -> f64 {
        self.rounding_magnitude
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
    slots: &[SkinSlot],
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
        // Accumulate both the weighted numerator and denominator in binary64,
        // then narrow the normalized point once. Binary32 multiply-then-divide
        // can lose a lone subnormal contribution or overflow a large finite
        // denominator. Precomputing binary32 coefficients is not sufficient:
        // their rounded sum can exceed one and overflow an otherwise finite
        // convex blend at `f32::MAX`.
        let mut weight_sum = 0.0f64;
        for weight in weights {
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
            weight_sum += f64::from(weight);
        }
        if weight_sum == 0.0 {
            continue;
        }

        let mut skinned_numerator = DVec3::ZERO;
        let mut weighted_magnitude = 0.0f64;
        for slot_index in 0..4 {
            let stored_weight = weights[slot_index];
            if stored_weight == 0.0 {
                continue;
            }
            let Some(slot) = slots.get(joints[slot_index] as usize) else {
                return Err(ScaleError::InvalidSkinnedPrimitive {
                    instance_index,
                    primitive_index,
                    reason: "joint_influence_slot_out_of_range",
                });
            };
            let weight = f64::from(stored_weight);
            skinned_numerator += weight * slot.matrix.transform_point3(position).as_dvec3();
            // The magnitude `slot.matrix.transform_point3(position)` runs on,
            // which is `abs(W * B) * abs(p)` and not either factor alone.
            //
            // `abs(p)` alone — which is what this stage read before — names one
            // of the two. It is the right number only while `abs(W * B)` is
            // `1`, which is every rig whose slots are a pure rotation of the
            // bind pose, and that is the whole of what the fixtures below build:
            // `cancelling_blend_document` composes with `HALF_TURN_Z`, so its
            // stage was accidentally exact and no test could see the gap. Give
            // the composed slot a scale of `k` and the transform runs on
            // `k * abs(p)` while the base reads `abs(p)`, short by `k`; the
            // weighted sum over slots can then cancel the result to nothing
            // while every term still carries `k * abs(p)`'s ulp.
            // `two_slots_with_a_scaled_composition_cancel_a_vertex_and_still_prove_its_bounds`
            // is that rig, and it is refused outright — `observed: 1.53e-5`
            // against `tolerance: 8.63e-6` at `k = 16` — without this term.
            //
            // The homogeneous `1` is included because `transform_point3` sums
            // the translation column in with the rest, so that column's entries
            // are terms of the same dot product.
            let influence_magnitude = skin_influence_magnitude(slot, position);
            weighted_magnitude += weight * influence_magnitude;
        }
        let skinned = (skinned_numerator / weight_sum).as_vec3();
        if !skinned.is_finite() {
            // Overflow and `NaN` are different failures and are
            // reported as such: an overflowing skinned position is a
            // document whose geometry leaves the `f32` range this proof
            // computes in, while a `NaN` is a malformed or degenerate
            // input that survived every finiteness check above. No
            // magnitude domain is documented for the former, because the
            // boundary is not a property of any magnitude a document
            // could be checked against ahead of time:
            // `transform_point3` accumulates a dot product whose
            // intermediate terms depend on the rotation, so two rigs
            // whose skinned extents agree can disagree on whether they
            // compose finitely.
            return Err(ScaleError::InvalidSkinnedPrimitive {
                instance_index,
                primitive_index,
                reason: if skinned.is_nan() {
                    "non_finite_result"
                } else {
                    "skinned_magnitude_overflow"
                },
            });
        }
        bounds.min = bounds.min.min(skinned);
        bounds.max = bounds.max.max(skinned);
        let vertex_magnitude = weighted_magnitude / weight_sum;
        bounds.rounding_magnitude = bounds.rounding_magnitude.max(vertex_magnitude);
        bounds.touched = true;
    }
    Ok(())
}

/// The per-axis arithmetic provenance one weighted skin influence carries
/// into a bound. The transform application and the already-composed slot can
/// each dominate by an unbounded ratio, so the influence retains the larger;
/// [`accumulate_skinned_bounds`] then combines influences with the same
/// binary64 weighted average as the skinned point.
fn skin_influence_magnitude(slot: &SkinSlot, position: Vec3) -> f64 {
    column_operand_magnitude(slot.absolute, position.extend(1.0)).max(slot.rounding_magnitude)
}

#[cfg(test)]
mod tests;
