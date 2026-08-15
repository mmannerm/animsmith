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
//! The public vocabulary and entrypoints continue to resolve through this
//! facade. Private implementation modules own numeric leaves, validation,
//! planning/replay, reference construction, and proof respectively; proof's
//! residual recorder is nested under proof so its paired maximum/count state
//! cannot be mutated outside that implementation boundary.
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
    AffineDomainViolation, BoneId, Document, DocumentShapeError, Interpolation, Property,
    SourceInverseBindAccessorStatus, SourceSkeletonCoverage,
};
#[cfg(test)]
use crate::model::{
    Clip, MeshInstanceShapeViolation, Skeleton, TrackValues, Transform, mat4_is_finite,
};
#[cfg(test)]
use glam::{Mat3, Mat4, Vec4};
#[cfg(test)]
use std::collections::BTreeMap;
use std::collections::BTreeSet;

mod numeric;
mod planning;
mod proof;
mod reference;
mod validation;

pub use planning::plan_scale;
use planning::validate_plan_document_inventory;
pub use proof::{ScaleProof, ScaleProofResidual, prove_scale};
pub use reference::ScaleCandidate;

#[cfg(test)]
use proof::{
    BoundsAccumulator, SkinSlot, accumulate_skinned_bounds, check_residual, check_sampling_budget,
    observed_factor_from_source, per_sample_work_units, skin_influence_magnitude, world_at_time,
};

#[cfg(test)]
use numeric::{
    column_operand_magnitude, largest_entry, mat4_abs, product_operand_magnitude,
    scale_translation_only, translation_composition_rounding_base,
};
#[cfg(test)]
use planning::classify_affine;
#[cfg(any(test, feature = "fixtures"))]
pub(crate) use reference::build_scale_candidate;
#[cfg(test)]
use reference::{build_rest_bind, build_whole_document};
#[cfg(test)]
use validation::{
    WorldBonePose, WorldPose, affected_skin_instance_indices, child_translation_rounding_magnitude,
    instance_bind, rest_world_pose, source_node_index_map, validate_scale_input,
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

    fn field_rows(&self) -> &[ScaleFieldPlan] {
        &self.ledger.field_rows
    }

    fn obligations(&self) -> &[ScaleProofObligation] {
        &self.ledger.obligations
    }
}

#[cfg(test)]
mod tests;
