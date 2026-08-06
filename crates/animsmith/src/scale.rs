//! The `animsmith scale` producer: two atomic, evidence-emitting scale
//! operations over raw glTF/GLB bytes (DESIGN.md Appendix D §D.6/§D.7,
//! implementation slice 5 of §D.8).
//!
//! # What one invocation does
//!
//! Read the input once; preflight it; project its raw capability manifest
//! onto format-neutral facts; plan; rewrite the source's own bytes; reload
//! and prove the candidate; write both members of the pair as temporaries;
//! read the artifact temp back and check its digest against the bytes that
//! were proved; then publish the artifact and its evidence as one pair.
//! Any refusal publishes **nothing** and leaves a prior pair byte-identical.
//!
//! # Outcomes, streams, and exit codes
//!
//! | Outcome | Published | `--format json` on stdout | Exit |
//! |---|---|---|---|
//! | Success | the pair, atomically | the record, `outcome: "published"` | 0 |
//! | Refusal | nothing | the record, `outcome: "rejected"` | 1 |
//! | Operator error | nothing | — (prose on stderr) | 2 |
//!
//! The split is by *what the failure is a property of*, not by where in the
//! pipeline it was raised.
//!
//! Exit `1` is a refusal that is a property of the **input asset**: bytes
//! that do not parse as the glTF/GLB the extension declares, an unsupported
//! source domain, any shared planning or proof rejection, any frontend
//! rewrite or artifact-proof rejection, and a read-back digest mismatch.
//!
//! Exit `2` is a property of the **invocation** or of the operator's
//! filesystem: a declared factor that is not finite and positive (rejected
//! before any member of the document is consulted), a missing or unopenable
//! input, a wrong extension, a container the extension disagrees with, two
//! arguments naming one file, a missing output directory, or a publication or
//! rollback I/O failure.
//!
//! This follows `lint --format json`, which prints its machine-readable
//! result to stdout and exits `1` when the asset has a problem; it
//! deliberately diverges from `assemble`, which maps every failure to `2`.
//!
//! # Determinism
//!
//! The evidence record carries no timestamp and no canonicalized path: the
//! operator's declared paths are recorded verbatim, and the canonical forms
//! exist only for the three-way distinctness check. Every collection on the
//! evidence path is ordered. Every `f64` is serialized through [`Finite`],
//! which fails serialization rather than let `serde_json` render a `NaN` or
//! an infinity as `null` — a false record, which DESIGN.md §D.6 forbids more
//! strongly than a missing one.
//!
//! # Cost
//!
//! One invocation plans twice (once here for the plan proof consumes, once
//! inside the rewriter, which plans internally so no caller can hand it a
//! plan built against a different document) and rewrites twice (the second
//! inside the artifact proof's determinism claim), then runs the full
//! sampled proof. On a production-sized rig this is seconds, not
//! milliseconds. The raw capability manifest is per-accessor and
//! per-channel, so the evidence record for such a rig is large; §D.6 asks
//! for the raw manifest and a digest of one is not a manifest.

use crate::publish::{
    destination_identity, input_identity, parent_or_current, publish_pair, read_digest,
    require_writable_destination, sha256_hex,
};
use crate::{Format, render};
use animsmith_core::scale::{
    ScaleDomainRewrites, ScaleError, ScaleOperation, ScalePlan, ScaleProof, ScaleProofObligations,
    ScaleRequest, ScaleTolerancePolicy, plan_scale,
};
use animsmith_core::{Document, InputIdentity, ToolInfo};
use animsmith_gltf::{
    GltfCapabilityManifest, GltfCapabilityViolation, GltfContainerKind, GltfScaleArtifact,
    GltfScaleArtifactProof, GltfScalePreflightError, GltfScaleRewriteError, GltfScaleSource,
    capability_facts, preflight_scale_source_bytes, prove_rewritten_artifact,
    prove_rewritten_rest_bind, rewrite_linear_units, rewrite_rest_bind,
};
use serde::ser::Error as _;
use serde::{Serialize, Serializer};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const SCALE_EVIDENCE_SCHEMA_VERSION: u32 = 1;
pub(crate) const SCALE_EVIDENCE_SCHEMA_ID: &str = "urn:animsmith:schema:scale-evidence:1";

// --- Request ---------------------------------------------------------------

/// The operation and its required selectors, exactly as DESIGN.md Appendix D
/// §D.7 states them. There is no inferred factor, no implicit first
/// skin/root, no config key, and no per-run tolerance flag.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Operation {
    /// Whole-document linear-unit conversion by a declared factor.
    WholeDocument {
        /// Declared conversion factor `q`.
        factor: f64,
    },
    /// Rest/bind hierarchy reparameterization at a declared source skin and
    /// source root node.
    RestBind {
        /// Source-skin array index.
        source_skin_index: usize,
        /// Source-node array index of the scaled ancestor.
        source_root_node_index: usize,
        /// Declared expected common factor `s`.
        expected_factor: f64,
    },
}

impl Operation {
    fn core(self) -> ScaleOperation {
        match self {
            Self::WholeDocument { factor } => ScaleOperation::WholeDocumentLinearUnits { factor },
            Self::RestBind {
                source_skin_index,
                source_root_node_index,
                expected_factor,
            } => ScaleOperation::RestBindUniformScale {
                source_skin_index,
                source_root_node_index,
                expected_factor,
            },
        }
    }

    fn declared_factor(self) -> f64 {
        match self {
            Self::WholeDocument { factor } => factor,
            Self::RestBind {
                expected_factor, ..
            } => expected_factor,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::WholeDocument { .. } => "whole-document",
            Self::RestBind { .. } => "rest-bind",
        }
    }
}

/// One complete `scale` invocation.
#[derive(Debug, Clone)]
pub(crate) struct Request {
    /// Selected operation and its declared selectors.
    pub(crate) operation: Operation,
    /// Input path, exactly as the operator wrote it.
    pub(crate) input: PathBuf,
    /// Artifact destination, exactly as the operator wrote it.
    pub(crate) output: PathBuf,
    /// Evidence destination, exactly as the operator wrote it.
    pub(crate) evidence: PathBuf,
    /// Whether stdout carries the record or a human summary.
    pub(crate) format: Format,
}

// --- Finite-guarded numbers -------------------------------------------------

/// An `f64` that refuses to serialize unless it is finite.
///
/// `serde_json` renders `NaN` and both infinities as `null` without
/// complaint. In a residual field that reads as "this claim was checked and
/// came out unmeasurable", which is a *false* record rather than a missing
/// one. Every number on the evidence path is wrapped here, so the guard is a
/// property of the record's type rather than of a validation pass someone
/// has to remember to run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Finite(pub(crate) f64);

impl Serialize for Finite {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if !self.0.is_finite() {
            return Err(S::Error::custom(format!(
                "refusing to publish non-finite scale evidence value {}",
                self.0
            )));
        }
        serializer.serialize_f64(self.0)
    }
}

// --- Evidence record --------------------------------------------------------

/// Whether this record describes a published pair or a refusal.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
enum Outcome {
    Published,
    Rejected,
}

/// The operation and its declared selectors, discriminated by `kind`.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum OperationRecord {
    WholeDocument {
        declared_factor: Finite,
    },
    RestBind {
        declared_factor: Finite,
        source_skin_index: usize,
        source_root_node_index: usize,
    },
}

impl From<Operation> for OperationRecord {
    fn from(operation: Operation) -> Self {
        match operation {
            Operation::WholeDocument { factor } => Self::WholeDocument {
                declared_factor: Finite(factor),
            },
            Operation::RestBind {
                source_skin_index,
                source_root_node_index,
                expected_factor,
            } => Self::RestBind {
                declared_factor: Finite(expected_factor),
                source_skin_index,
                source_root_node_index,
            },
        }
    }
}

/// The operator's declared paths, verbatim. Canonical host paths are used
/// for the distinctness check and are deliberately never serialized.
#[derive(Debug, Clone, Serialize)]
struct PathsRecord {
    input: String,
    output: String,
    evidence: String,
}

/// One residual, and whether the proof that reports it actually evaluated
/// anything.
///
/// [`ScaleProof`] reports `0.0` for an obligation the plan does not require,
/// which §D.6 calls a false record: "a record stating residual 0.0 for a
/// claim nothing checked". So the residual is never published flat. `max` is
/// `null` exactly when `evaluated` is false, and a consumer that reads only
/// `max` therefore sees an absence rather than a fabricated zero.
#[derive(Debug, Clone, Copy, Serialize)]
struct Residual {
    evaluated: bool,
    max: Option<Finite>,
}

impl Residual {
    fn new(evaluated: bool, max: f64) -> Self {
        Self {
            evaluated,
            max: evaluated.then_some(Finite(max)),
        }
    }
}

/// Every residual [`ScaleProof`] reports, each gated by the evidence its
/// obligation reads.
#[derive(Debug, Clone, Copy, Serialize)]
struct ResidualsRecord {
    rest_translation: Residual,
    rest_rotation: Residual,
    unit_scale: Residual,
    transform_only_affine: Residual,
    track_value: Residual,
    mesh_position: Residual,
    key_translation: Residual,
    cubic_interior: Residual,
    trajectory: Residual,
    skin_matrix: Residual,
    bounds: Residual,
    unaffected_inverse_bind: Residual,
}

/// The fixed tolerance policy, recorded by identity and in full.
///
/// §D.7 forbids per-run tolerance flags: the command uses the policy version
/// recorded here, and a future policy change requires a new identity.
#[derive(Debug, Clone, Copy, Serialize)]
struct ToleranceRecord {
    policy_id: &'static str,
    relative_orthogonality: Finite,
    equal_axis: Finite,
    common_factor: Finite,
    singular_determinant_relative: Finite,
    scalar_absolute: Finite,
    scalar_relative: Finite,
    rotation_residual_radians: Finite,
    postcondition_unit_scale_residual: Finite,
    proof_sample_work_budget: u64,
}

impl From<ScaleTolerancePolicy> for ToleranceRecord {
    fn from(policy: ScaleTolerancePolicy) -> Self {
        Self {
            policy_id: policy.id,
            relative_orthogonality: Finite(policy.relative_orthogonality),
            equal_axis: Finite(policy.equal_axis),
            common_factor: Finite(policy.common_factor),
            singular_determinant_relative: Finite(policy.singular_determinant_relative),
            scalar_absolute: Finite(policy.scalar_absolute),
            scalar_relative: Finite(policy.scalar_relative),
            rotation_residual_radians: Finite(policy.rotation_residual_radians),
            postcondition_unit_scale_residual: Finite(policy.postcondition_unit_scale_residual),
            proof_sample_work_budget: policy.proof_sample_work_budget,
        }
    }
}

/// Both observed-factor witnesses and the divergence between them, per §D.6.
///
/// The two witnesses are measured from deliberately different state — the raw
/// source projection's parent chain and the normalized skeleton's — and
/// nothing reconciles them, which is why both are recorded. The divergence is
/// reported, never enforced; `divergence_ceiling` is what the design expects
/// of it.
#[derive(Debug, Clone, Copy, Serialize)]
struct FactorsRecord {
    declared: Finite,
    planned_observed: Finite,
    proved_observed: Finite,
    divergence: Finite,
    divergence_ceiling: Finite,
}

/// Affected identities, in the raw source index space the selectors use.
#[derive(Debug, Clone, Serialize)]
struct AffectedRecord {
    source_nodes: Vec<usize>,
    source_skins: Vec<usize>,
    /// How many closure members carry no skin — the transform-only children
    /// of §D.2/§D.3 whose full world affine the proof probes. A count rather
    /// than identities: the frontend reports the closure in source-node
    /// space, but this subset is only distinguished in normalized bone space,
    /// and mixing the two in one record would invite a consumer to read a
    /// bone id as a node index.
    transform_only_attachment_count: usize,
}

/// Which model domains this plan rewrote, matching the §D.4 domain table.
#[derive(Debug, Clone, Copy, Serialize)]
struct DomainRewritesRecord {
    rest_hierarchy: bool,
    translation_animation: bool,
    inverse_binds: bool,
    base_mesh_positions: bool,
}

impl From<ScaleDomainRewrites> for DomainRewritesRecord {
    fn from(rewrites: ScaleDomainRewrites) -> Self {
        Self {
            rest_hierarchy: rewrites.rest_hierarchy,
            translation_animation: rewrites.translation_animation,
            inverse_binds: rewrites.inverse_binds,
            base_mesh_positions: rewrites.base_mesh_positions,
        }
    }
}

/// The artifact-level claims the glTF frontend re-derived independently.
#[derive(Debug, Clone, Copy, Serialize)]
struct ArtifactProofRecord {
    length_factor_residual: Finite,
    dimensionless_residual: Finite,
    preserved_byte_ranges: usize,
    rewritten_accessor_count: usize,
}

/// Proof coverage and results.
#[derive(Debug, Clone, Serialize)]
struct ProofRecord {
    sample_time_count: usize,
    residuals: ResidualsRecord,
    artifact: ArtifactProofRecord,
    /// Whether the bytes read back from the published temp digest to the same
    /// value as the artifact bytes that were proved.
    ///
    /// This is the producer's own third check, and it is the only artifact
    /// claim left for it: [`prove_rewritten_artifact`] and
    /// [`prove_rewritten_rest_bind`] each already reload the candidate and
    /// re-run the core proof, and each already re-runs the whole rewrite to
    /// byte-compare for determinism. What neither can see is the write path,
    /// so that is what this closes — serialization, a short write, a
    /// truncation, or a temp clobbered between write and rename.
    ///
    /// It is a digest comparison rather than a re-proof of the reloaded file
    /// because [`GltfScaleArtifact`] has no public constructor from bytes,
    /// and adding one would let a caller assert artifact-level claims about
    /// bytes it did not produce.
    read_back_digest_matches: bool,
}

/// The published artifact's identity and the exact locations it changed.
#[derive(Debug, Clone, Serialize)]
struct ArtifactRecord {
    container: GltfContainerKind,
    #[serde(flatten)]
    identity: InputIdentity,
    rewritten_accessors: Vec<usize>,
    rewritten_json_pointers: Vec<String>,
    reencoded_buffers: Vec<usize>,
}

/// Everything a published run adds to the record.
#[derive(Debug, Clone, Serialize)]
struct PublishedRecord {
    tolerance: ToleranceRecord,
    factors: FactorsRecord,
    affected: AffectedRecord,
    domain_rewrites: DomainRewritesRecord,
    proof: ProofRecord,
    artifact: ArtifactRecord,
}

/// Where in the pipeline a refusal was raised.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum Stage {
    /// The raw capability preflight refused the source outright.
    Preflight,
    /// Shared planning refused the request.
    Plan,
    /// The glTF frontend refused to rewrite the source bytes.
    Rewrite,
    /// Proof of the rewritten artifact failed.
    Proof,
    /// The bytes read back from the prepared temp were not the bytes proved.
    ReadBack,
}

/// One typed rejection.
#[derive(Debug, Clone, Serialize)]
struct RejectionRecord {
    stage: Stage,
    /// Stable machine identity for the refusal, in the same kebab-case space
    /// across every stage.
    kind: &'static str,
    /// The typed error's own rendering. Prose for an operator; `kind` is what
    /// a consumer branches on.
    detail: String,
    /// Typed capability violations, when the refusal was a capability one.
    /// Empty otherwise — never absent, so a consumer never has to
    /// distinguish "no violations" from "field not written".
    violations: Vec<GltfCapabilityViolation>,
}

/// The immutable versioned scale-evidence contract, `scale-evidence:1`.
///
/// One schema serves both outcomes, discriminated by `outcome`: a published
/// run carries `result` and a `null` `rejection`, a refused run the reverse.
/// A refused run's record is printed, never published — §D.6's publication is
/// an artifact/evidence *pair*, and a refusal has no artifact.
#[derive(Debug, Clone, Serialize)]
struct ScaleEvidence<'a> {
    schema_version: u32,
    schema: &'static str,
    tool: ToolInfo,
    command: &'static str,
    outcome: Outcome,
    operation: OperationRecord,
    paths: PathsRecord,
    input: InputIdentity,
    /// The raw #280 capability manifest, present whenever the preflight got
    /// far enough to inventory one. §D.6 asks for the raw manifest, and a
    /// digest of one is not a manifest.
    capability: Option<&'a GltfCapabilityManifest>,
    result: Option<PublishedRecord>,
    rejection: Option<RejectionRecord>,
}

// --- Failure classification -------------------------------------------------

/// Everything that can stop a run short of publication.
enum Failure {
    /// Exit 2: the operator's invocation or filesystem, not the asset.
    Operator(String),
    /// Exit 1: a property of the input asset.
    Refusal(RejectionRecord),
}

impl From<String> for Failure {
    fn from(message: String) -> Self {
        Self::Operator(message)
    }
}

fn refuse(stage: Stage, kind: &'static str, detail: String) -> Failure {
    Failure::Refusal(RejectionRecord {
        stage,
        kind,
        detail,
        violations: Vec::new(),
    })
}

/// Stable machine identity for one shared planning or proof rejection.
///
/// Written without a wildcard over the variants this build knows, so a new
/// [`ScaleError`] arrives here as an explicit decision rather than as
/// `"scale-error"`. The trailing arm exists only because [`ScaleError`] is
/// `#[non_exhaustive]` in this crate's compilation of it; it is not a
/// catch-all for variants that already exist.
fn scale_error_kind(error: &ScaleError) -> &'static str {
    match error {
        ScaleError::InvalidFactor { .. } => "invalid-factor",
        ScaleError::InvalidExpectedFactor { .. } => "invalid-expected-factor",
        ScaleError::FactorNotRepresentable { .. } => "factor-not-representable",
        ScaleError::InvalidRootSelector { .. } => "invalid-root-selector",
        ScaleError::InvalidSkinSelector { .. } => "invalid-skin-selector",
        ScaleError::IncompleteCapability => "incomplete-capability",
        ScaleError::IncompleteSourceSkeleton => "incomplete-source-skeleton",
        ScaleError::SourceNodeNotNormalized { .. } => "source-node-not-normalized",
        ScaleError::NonFiniteSourceTransform { .. } => "non-finite-source-transform",
        ScaleError::NonFiniteTransform { .. } => "non-finite-transform",
        ScaleError::InvalidParent { .. } => "invalid-parent",
        ScaleError::BoneIndexOutOfRange { .. } => "bone-index-out-of-range",
        ScaleError::IncompleteClosure { .. } => "incomplete-closure",
        ScaleError::UnsupportedUnskinnedGeometry { .. } => "unsupported-unskinned-geometry",
        ScaleError::InvalidAffineDomain { .. } => "invalid-affine-domain",
        ScaleError::FactorMismatch { .. } => "factor-mismatch",
        ScaleError::MixedFactor { .. } => "mixed-factor",
        ScaleError::AffectedScaleAnimation { .. } => "affected-scale-animation",
        ScaleError::ProofResidualExceeded { .. } => "proof-residual-exceeded",
        ScaleError::ProofSamplingBudgetExceeded { .. } => "proof-sampling-budget-exceeded",
        ScaleError::MissingProofEvidence { .. } => "missing-proof-evidence",
        ScaleError::DuplicateSourceNodeIndex { .. } => "duplicate-source-node-index",
        ScaleError::DuplicateSourceSkinIndex { .. } => "duplicate-source-skin-index",
        ScaleError::DuplicateClipTrack { .. } => "duplicate-clip-track",
        ScaleError::InvalidTrackShape { .. } => "invalid-track-shape",
        ScaleError::InvalidMeshInstance { .. } => "invalid-mesh-instance",
        ScaleError::MissingInverseBind { .. } => "missing-inverse-bind",
        ScaleError::InvalidMeshPrimitive { .. } => "invalid-mesh-primitive",
        ScaleError::InvalidSkinnedPrimitive { .. } => "invalid-skinned-primitive",
        ScaleError::CandidateStructureMismatch { .. } => "candidate-structure-mismatch",
        _ => "unclassified-scale-error",
    }
}

/// Classify one shared planning rejection.
///
/// Almost every [`ScaleError`] planning raises is a fact about the input
/// asset — an unusable selector, a hierarchy the operations do not accept, a
/// factor the source disagrees with — and refuses with exit `1`. The two
/// exceptions are the declared factor's own validity: a factor that is not
/// finite and positive is rejected before a single member of the document is
/// consulted, so it is a property of the *invocation*, and reporting it as a
/// refusal would tell an operator the asset was examined and found wanting
/// when it was not.
///
/// [`ScaleError::FactorNotRepresentable`] deliberately stays a refusal: it is
/// raised for the reciprocal `1 / expected_factor` as well as for the
/// declared factor, and it is also reachable from the frontend rewrite, where
/// this classification does not apply.
fn plan_failure(error: ScaleError) -> Failure {
    match error {
        ScaleError::InvalidFactor { .. } | ScaleError::InvalidExpectedFactor { .. } => {
            Failure::Operator(error.to_string())
        }
        error => refuse(Stage::Plan, scale_error_kind(&error), error.to_string()),
    }
}

/// Classify one frontend rewrite or artifact-proof rejection.
///
/// Every variant is a fact about the input asset — none of them names the
/// operator's filesystem — so all of them refuse with exit `1`. `Load` here
/// is the *artifact* or the source's own JSON failing to parse, not a
/// missing file: the producer reads the input's bytes itself, and a file that
/// cannot be opened at all is the operator error raised before the frontend
/// is reached.
fn rewrite_failure(stage: Stage, error: GltfScaleRewriteError) -> Failure {
    let detail = error.to_string();
    let (kind, violations) = match error {
        GltfScaleRewriteError::Capability { violations, .. } => {
            ("unsupported-source-domain", violations)
        }
        GltfScaleRewriteError::Plan(error) => (scale_error_kind(&error), Vec::new()),
        GltfScaleRewriteError::Load(_) => ("unreadable-source", Vec::new()),
        GltfScaleRewriteError::Write(_) => ("unwritable-container", Vec::new()),
        GltfScaleRewriteError::UnhandledLengthField { .. } => {
            ("unhandled-length-field", Vec::new())
        }
        GltfScaleRewriteError::ConflictingRewriteRule { .. } => {
            ("conflicting-rewrite-rule", Vec::new())
        }
        GltfScaleRewriteError::UnrewritableAccessor { .. } => ("unrewritable-accessor", Vec::new()),
        GltfScaleRewriteError::ConflictingNodeTransform { .. } => {
            ("conflicting-node-transform", Vec::new())
        }
        GltfScaleRewriteError::NonAffineNodeMatrix { .. } => ("non-affine-node-matrix", Vec::new()),
        GltfScaleRewriteError::ImagePayloadOverlap { .. } => ("image-payload-overlap", Vec::new()),
        GltfScaleRewriteError::UnreassemblableContainer { .. } => {
            ("unreassemblable-container", Vec::new())
        }
        GltfScaleRewriteError::ValueNotRepresentable { .. } => {
            ("value-not-representable", Vec::new())
        }
        GltfScaleRewriteError::ConflictingRestBindFactor { .. } => {
            ("conflicting-rest-bind-factor", Vec::new())
        }
        GltfScaleRewriteError::ClosureMismatch { .. } => ("closure-mismatch", Vec::new()),
        GltfScaleRewriteError::ParentChainDisagreement { .. } => {
            ("parent-chain-disagreement", Vec::new())
        }
        GltfScaleRewriteError::AmbiguousSourceNodeProjection { .. } => {
            ("ambiguous-source-node-projection", Vec::new())
        }
        GltfScaleRewriteError::UnusableSourceHierarchy { .. } => {
            ("unusable-source-hierarchy", Vec::new())
        }
        GltfScaleRewriteError::ArtifactProofFailed { .. } => ("artifact-proof-failed", Vec::new()),
        _ => ("unclassified-rewrite-error", Vec::new()),
    };
    Failure::Refusal(RejectionRecord {
        stage,
        kind,
        detail,
        violations,
    })
}

// --- Evidence-gated residuals -----------------------------------------------

/// Which residuals this plan's obligations, and the source it was planned
/// against, actually caused proof to evaluate.
///
/// Nine of the twelve are gated by [`ScaleProofObligations`], whose flags
/// [`plan_scale`] already sets from the evidence each obligation reads. The
/// other three — `track_value`, `mesh_position` and `unaffected_inverse_bind`
/// — are proved unconditionally by `prove_scale`, because they are claims
/// about payloads the plan declares it does *not* rewrite and an obligation
/// flag would make "unaffected" unfalsifiable. Their loops can still have
/// zero iterations, so their evidence predicate is re-derived here from the
/// source document. Issues #316, #317 and #319 refine these predicates; the
/// `{evaluated, max}` shape is what makes that a refinement rather than a
/// contract change.
fn residuals(source: &Document, plan: &ScalePlan, proof: &ScaleProof) -> ResidualsRecord {
    let ScaleProofObligations {
        prove_rest,
        prove_unit_scale_postcondition,
        prove_transform_only_affine,
        prove_keys,
        prove_cubic_interiors,
        prove_trajectories,
        prove_skin,
        prove_bounds,
        ..
    } = plan.proof_obligations();
    ResidualsRecord {
        rest_translation: Residual::new(prove_rest, proof.rest_translation_residual),
        rest_rotation: Residual::new(prove_rest, proof.rest_rotation_residual),
        unit_scale: Residual::new(prove_unit_scale_postcondition, proof.unit_scale_residual),
        transform_only_affine: Residual::new(
            prove_transform_only_affine,
            proof.transform_only_affine_residual,
        ),
        track_value: Residual::new(has_track_values(source), proof.track_value_residual),
        mesh_position: Residual::new(has_mesh_positions(source), proof.mesh_position_residual),
        key_translation: Residual::new(prove_keys, proof.key_translation_residual),
        cubic_interior: Residual::new(prove_cubic_interiors, proof.cubic_interior_residual),
        trajectory: Residual::new(prove_trajectories, proof.trajectory_residual),
        skin_matrix: Residual::new(prove_skin, proof.skin_matrix_residual),
        bounds: Residual::new(prove_bounds, proof.bounds_residual),
        unaffected_inverse_bind: Residual::new(
            has_unaffected_stored_binds(source, plan),
            proof.unaffected_inverse_bind_residual,
        ),
    }
}

/// Whether any clip track carries a value element for the per-element
/// comparison to read.
fn has_track_values(source: &Document) -> bool {
    use animsmith_core::model::TrackValues;
    source.clips.iter().any(|clip| {
        clip.tracks.iter().any(|track| match &track.values {
            TrackValues::Vec3s(values) => !values.is_empty(),
            TrackValues::Quats(values) => !values.is_empty(),
        })
    })
}

/// Whether any mesh primitive carries a base `POSITION` to compare.
fn has_mesh_positions(source: &Document) -> bool {
    source
        .assets
        .meshes
        .iter()
        .flat_map(|mesh| &mesh.primitives)
        .any(|primitive| !primitive.positions.is_empty())
}

/// Whether any skin slot *outside* the affected closure has stored
/// inverse-bind evidence on both the instance and the bone fallback the
/// model contract defines.
///
/// Mirrors the resolution order `prove_scale` uses for that obligation: the
/// instance's own `skin_ibms` when non-empty, else the bone's own
/// `inverse_bind`. An instance with any affected joint is skipped there, so
/// it is skipped here.
fn has_unaffected_stored_binds(source: &Document, plan: &ScalePlan) -> bool {
    let affected: std::collections::BTreeSet<usize> =
        plan.affected_nodes().iter().copied().collect();
    source.assets.instances.iter().any(|instance| {
        if instance
            .skin_joints
            .iter()
            .any(|joint| affected.contains(joint))
        {
            return false;
        }
        instance
            .skin_joints
            .iter()
            .enumerate()
            .any(|(slot, joint)| {
                if !instance.skin_ibms.is_empty() {
                    return instance.skin_ibms.get(slot).is_some();
                }
                source
                    .skeleton
                    .bones
                    .get(*joint)
                    .is_some_and(|bone| bone.inverse_bind.is_some())
            })
    })
}

// --- Argument validation ----------------------------------------------------

/// The container an extension declares, or an operator error.
fn declared_container(path: &Path, role: &str) -> Result<GltfContainerKind, String> {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default()
        .as_str()
    {
        "glb" => Ok(GltfContainerKind::Glb),
        "gltf" => Ok(GltfContainerKind::Gltf),
        "" => Err(format!(
            "scale {role} {} has no extension: scale reads and writes .glb or .gltf",
            path.display()
        )),
        _ => Err(format!(
            "scale {role} {} is not .glb or .gltf: scale operates on self-contained glTF/GLB only",
            path.display()
        )),
    }
}

fn container_name(container: GltfContainerKind) -> &'static str {
    match container {
        GltfContainerKind::Glb => "glb",
        GltfContainerKind::Gltf => "gltf",
    }
}

/// Reject any two of the three paths naming one file.
///
/// The input and the two destinations are resolved *differently*, because the
/// operations that reach them differ: the input is read, which follows a
/// symlinked final component, and the destinations are renamed over, which
/// replaces one. Resolving the input the destination way is what lets
/// `latest.glb -> store/rig.glb` be handed in as the input while
/// `store/rig.glb` is named as the output, with publication then destroying
/// the source asset the run just read. See [`input_identity`] and
/// [`destination_identity`].
///
/// This one check is the whole guard: `scale` deliberately does not also
/// reject symlinked paths outright the way `assemble`'s `reject_symlink_path`
/// does. That guard exists to keep a recipe's declared inputs inside the
/// recipe's own directory, a containment property `scale` does not have —
/// its three paths are the operator's own, unconstrained, and a symlinked
/// input that aliases nothing is a legitimate invocation this command has no
/// reason to refuse.
///
/// The canonical forms exist only for this comparison; §D.6's evidence keeps
/// the operator's declared path verbatim, so nothing computed here is
/// serialized.
fn require_distinct_paths(request: &Request) -> Result<(), String> {
    let input = input_identity(&request.input)?;
    let output = destination_identity(&request.output)?;
    let evidence = destination_identity(&request.evidence)?;
    for (first, second, first_label, second_label) in [
        (&input, &output, "input", "output"),
        (&input, &evidence, "input", "evidence"),
        (&output, &evidence, "output", "evidence"),
    ] {
        if first == second {
            return Err(format!(
                "scale {first_label} and {second_label} must be different paths, but both resolve to {}",
                first.display()
            ));
        }
    }
    Ok(())
}

// --- The producer -----------------------------------------------------------

/// Run one complete `scale` invocation.
///
/// # Errors
///
/// Returns an operator error (exit `2`) for a bad invocation, an input that
/// cannot be opened, a destination that cannot be prepared, or a publication
/// or rollback failure. A refusal that is a property of the input asset is
/// not an error here: it prints its record and returns exit `1`.
pub(crate) fn run(request: &Request, tool: ToolInfo) -> Result<ExitCode, String> {
    let input_container = declared_container(&request.input, "input")?;
    let output_container = declared_container(&request.output, "output")?;
    if input_container != output_container {
        return Err(format!(
            "scale output must keep the source container: {} is .{}, {} is .{}",
            request.input.display(),
            container_name(input_container),
            request.output.display(),
            container_name(output_container),
        ));
    }
    require_writable_destination(&request.output)?;
    require_writable_destination(&request.evidence)?;
    require_distinct_paths(request)?;

    let input_bytes = fs::read(&request.input)
        .map_err(|error| format!("cannot read {}: {error}", request.input.display()))?;
    let identity = InputIdentity::from_bytes(&input_bytes);
    let paths = PathsRecord {
        input: request.input.display().to_string(),
        output: request.output.display().to_string(),
        evidence: request.evidence.display().to_string(),
    };

    // A refused preflight owns the only manifest there will ever be for this
    // input, so it is bound out of the error and the record borrows it.
    // Nothing fabricates a manifest: a refusal with no inventory publishes
    // `capability: null`, which is an absence, not an empty document.
    let source = match preflight_scale_source_bytes(&request.input, &input_bytes) {
        Ok(source) => source,
        // The bytes are in hand — `fs::read` already succeeded — so this is
        // the *asset* failing to parse, not the operator's filesystem. It
        // refuses with exit `1` for the same reason every other malformed
        // input does: nothing about the invocation was wrong, and telling an
        // operator to check their command line sends them to the wrong place.
        // No manifest exists: the inventory is built from a document that
        // never loaded, and `capability: null` is that absence.
        Err(GltfScalePreflightError::Load(error)) => {
            let rejection = RejectionRecord {
                stage: Stage::Preflight,
                kind: "unreadable-source",
                detail: error.to_string(),
                violations: Vec::new(),
            };
            return emit_rejection(request, tool, &paths, identity, None, rejection);
        }
        Err(GltfScalePreflightError::Unsupported {
            manifest,
            violations,
            count,
        }) => {
            let rejection = RejectionRecord {
                stage: Stage::Preflight,
                kind: "unsupported-source-domain",
                detail: format!(
                    "glTF scale preflight rejected {count} unsupported source domain(s)"
                ),
                violations,
            };
            return emit_rejection(request, tool, &paths, identity, Some(&manifest), rejection);
        }
        // `GltfScalePreflightError` is `#[non_exhaustive]`, so this arm is
        // required. It classifies an unknown future preflight rejection as a
        // refusal rather than an operator error, which is the safe
        // direction: nothing is published either way, and calling a fact
        // about the asset an invocation mistake sends an operator looking at
        // the wrong thing.
        Err(other) => {
            let rejection = RejectionRecord {
                stage: Stage::Preflight,
                kind: "unclassified-preflight-error",
                detail: other.to_string(),
                violations: Vec::new(),
            };
            return emit_rejection(request, tool, &paths, identity, None, rejection);
        }
    };

    if source.manifest().container != input_container {
        return Err(format!(
            "{} is a {} container but its extension declares .{}",
            request.input.display(),
            container_name(source.manifest().container),
            container_name(input_container),
        ));
    }

    match produce(request, &source) {
        Ok(produced) => publish(request, tool, &paths, identity, &source, produced),
        Err(Failure::Operator(message)) => Err(message),
        Err(Failure::Refusal(rejection)) => emit_rejection(
            request,
            tool,
            &paths,
            identity,
            Some(source.manifest()),
            rejection,
        ),
    }
}

/// The plan, artifact and proof one successful run produced.
struct Produced {
    plan: ScalePlan,
    artifact: GltfScaleArtifact,
    proof: GltfScaleArtifactProof,
    artifact_temp: tempfile::TempPath,
    artifact_identity: InputIdentity,
}

/// Plan, rewrite, prove, and stage the artifact — everything up to but not
/// including publication.
fn produce(request: &Request, source: &GltfScaleSource) -> Result<Produced, Failure> {
    let facts = capability_facts(source.manifest());
    let plan = plan_scale(&ScaleRequest {
        operation: request.operation.core(),
        document: source.document(),
        capability: &facts,
    })
    .map_err(plan_failure)?;

    let artifact = match request.operation {
        Operation::WholeDocument { factor } => rewrite_linear_units(source, factor),
        Operation::RestBind {
            source_skin_index,
            source_root_node_index,
            expected_factor,
        } => rewrite_rest_bind(
            source,
            source_skin_index,
            source_root_node_index,
            expected_factor,
        ),
    }
    .map_err(|error| rewrite_failure(Stage::Rewrite, error))?;

    let proof = match request.operation {
        Operation::WholeDocument { .. } => prove_rewritten_artifact(source, &artifact, &plan),
        Operation::RestBind { .. } => prove_rewritten_rest_bind(source, &artifact, &plan),
    }
    .map_err(|error| rewrite_failure(Stage::Proof, error))?;

    // Staged in the artifact's own destination directory, so publication is
    // a rename within one filesystem.
    let artifact_temp = tempfile::Builder::new()
        .prefix(".animsmith-scale-")
        .suffix(&format!(".{}", container_name(artifact.container())))
        .tempfile_in(parent_or_current(&request.output))
        .map_err(|error| Failure::Operator(format!("cannot create temporary output: {error}")))?
        .into_temp_path();
    fs::write(&artifact_temp, artifact.bytes())
        .map_err(|error| Failure::Operator(format!("cannot write temporary output: {error}")))?;

    // The third artifact check: the bytes that landed are the bytes proved.
    let (read_back_sha256, _) = read_digest(&artifact_temp).map_err(Failure::from)?;
    require_read_back_match(&read_back_sha256, &sha256_hex(artifact.bytes()))?;
    let artifact_identity = InputIdentity::from_bytes(artifact.bytes());

    Ok(Produced {
        plan,
        artifact,
        proof,
        artifact_temp,
        artifact_identity,
    })
}

/// Refuse unless the digest read back from the staged temp is the digest of
/// the bytes that were proved.
///
/// Split out so the refusal it raises is directly testable. The *scenario* it
/// guards — bytes changing between `write` and the read that follows it —
/// cannot be produced from a test that drives the real binary, so what a test
/// can pin is this classification: a mismatch is a `read-back` refusal with a
/// stable kind, not an operator error and not a silent success.
fn require_read_back_match(read_back: &str, proved: &str) -> Result<(), Failure> {
    if read_back == proved {
        return Ok(());
    }
    Err(refuse(
        Stage::ReadBack,
        "read-back-digest-mismatch",
        format!(
            "the staged artifact reads back as {read_back} but the proved bytes digest to {proved}"
        ),
    ))
}

/// Serialize the record, stage it, and publish the pair.
fn publish(
    request: &Request,
    tool: ToolInfo,
    paths: &PathsRecord,
    identity: InputIdentity,
    source: &GltfScaleSource,
    produced: Produced,
) -> Result<ExitCode, String> {
    let Produced {
        plan,
        artifact,
        proof,
        artifact_temp,
        artifact_identity,
    } = produced;
    let policy = plan.tolerance_policy();
    let record = ScaleEvidence {
        schema_version: SCALE_EVIDENCE_SCHEMA_VERSION,
        schema: SCALE_EVIDENCE_SCHEMA_ID,
        tool,
        command: "scale",
        outcome: Outcome::Published,
        operation: request.operation.into(),
        paths: paths.clone(),
        input: identity,
        capability: Some(source.manifest()),
        result: Some(PublishedRecord {
            tolerance: policy.into(),
            factors: FactorsRecord {
                declared: Finite(plan.common_factor()),
                planned_observed: Finite(proof.core.planned_observed_factor),
                proved_observed: Finite(proof.core.observed_factor),
                divergence: Finite(proof.core.observed_factor_divergence),
                divergence_ceiling: Finite(policy.observed_factor_divergence_ceiling()),
            },
            affected: AffectedRecord {
                source_nodes: artifact.affected_source_nodes().to_vec(),
                source_skins: artifact.affected_source_skins().to_vec(),
                transform_only_attachment_count: plan.transform_only_attachments().len(),
            },
            domain_rewrites: plan.domain_rewrites().into(),
            proof: ProofRecord {
                sample_time_count: proof.core.sample_time_count,
                residuals: residuals(source.document(), &plan, &proof.core),
                artifact: ArtifactProofRecord {
                    length_factor_residual: Finite(proof.length_factor_residual),
                    dimensionless_residual: Finite(proof.dimensionless_residual),
                    preserved_byte_ranges: proof.preserved_byte_ranges,
                    rewritten_accessor_count: proof.rewritten_accessor_count,
                },
                read_back_digest_matches: true,
            },
            artifact: ArtifactRecord {
                container: artifact.container(),
                identity: artifact_identity,
                rewritten_accessors: artifact.rewritten_accessors().to_vec(),
                rewritten_json_pointers: artifact.rewritten_json_pointers().to_vec(),
                reencoded_buffers: artifact.reencoded_buffers().to_vec(),
            },
        }),
        rejection: None,
    };
    let evidence_bytes = serialize_record(&record)?;

    let evidence_temp = tempfile::Builder::new()
        .prefix(".animsmith-scale-evidence-")
        .suffix(".json")
        .tempfile_in(parent_or_current(&request.evidence))
        .map_err(|error| format!("cannot create temporary evidence: {error}"))?
        .into_temp_path();
    fs::write(&evidence_temp, &evidence_bytes)
        .map_err(|error| format!("cannot write temporary evidence: {error}"))?;

    publish_pair(
        &artifact_temp,
        &request.output,
        &evidence_temp,
        &request.evidence,
        false,
    )?;

    match request.format {
        Format::Json => render::print_json(&record),
        Format::Text => print!(
            "{}",
            render::render_scale_published(
                &request.output,
                &request.evidence,
                request.operation.label(),
                request.operation.declared_factor(),
                artifact.affected_source_nodes().len(),
                artifact.affected_source_skins().len(),
                proof.core.sample_time_count,
            )
        ),
    }
    Ok(ExitCode::SUCCESS)
}

/// Print one refusal and report exit `1`. Nothing is published, and any
/// prior pair is untouched because nothing ever moved it.
fn emit_rejection(
    request: &Request,
    tool: ToolInfo,
    paths: &PathsRecord,
    identity: InputIdentity,
    capability: Option<&GltfCapabilityManifest>,
    rejection: RejectionRecord,
) -> Result<ExitCode, String> {
    // Rendered before `rejection` is moved into the record below, which is
    // also why it is built in both formats rather than inside the text arm.
    let summary = render::render_scale_rejected(
        &request.input,
        request.operation.label(),
        rejection.kind,
        &rejection.detail,
    );
    let record = ScaleEvidence {
        schema_version: SCALE_EVIDENCE_SCHEMA_VERSION,
        schema: SCALE_EVIDENCE_SCHEMA_ID,
        tool,
        command: "scale",
        outcome: Outcome::Rejected,
        operation: request.operation.into(),
        paths: paths.clone(),
        input: identity,
        capability,
        result: None,
        rejection: Some(rejection),
    };
    match request.format {
        Format::Json => {
            // Serialized first so a value `Finite` refuses becomes an
            // operator error rather than a panic inside the shared printer.
            serialize_record(&record)?;
            render::print_json(&record);
        }
        Format::Text => eprint!("{summary}"),
    }
    Ok(ExitCode::from(crate::EXIT_FINDINGS))
}

/// Serialize one record as the pretty JSON both the file and stdout carry,
/// newline-terminated.
///
/// # Errors
///
/// Returns an operator error when a value refuses to serialize — which
/// [`Finite`] makes happen for any non-finite number on the evidence path.
/// Nothing has been published at either call site when this fails.
fn serialize_record(record: &ScaleEvidence<'_>) -> Result<Vec<u8>, String> {
    let mut bytes = serde_json::to_vec_pretty(record)
        .map_err(|error| format!("cannot serialize scale evidence: {error}"))?;
    bytes.push(b'\n');
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_non_finite_evidence_value_refuses_to_serialize() {
        // `serde_json` renders NaN and both infinities as `null`, which in a
        // residual field is a false record rather than a missing one.
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let error = serde_json::to_string(&Finite(value))
                .expect_err("a non-finite evidence value must not serialize");
            assert!(
                error.to_string().contains("refusing to publish non-finite"),
                "{error}"
            );
        }
    }

    #[test]
    fn finite_values_including_both_zeros_serialize_unchanged() {
        // The guard must not reject a legitimate residual. `-0.0` is the
        // interesting case: it is finite, and `serde_json` renders it as
        // `-0.0`.
        assert_eq!(serde_json::to_string(&Finite(0.0)).unwrap(), "0.0");
        assert_eq!(serde_json::to_string(&Finite(-0.0)).unwrap(), "-0.0");
        // A value with a long exact decimal expansion, to show the guard is
        // a pass-through and adds no rounding of its own. The literal is
        // written the way `serde_json` spells it rather than in exponent
        // form: the spelling is that crate's, not this record's contract.
        assert_eq!(
            serde_json::to_string(&Finite(7.103515625e-5)).unwrap(),
            "0.00007103515625"
        );
    }

    #[test]
    fn an_unevaluated_residual_publishes_null_rather_than_zero() {
        let evaluated = Residual::new(true, 0.25);
        let unevaluated = Residual::new(false, 0.0);
        assert_eq!(
            serde_json::to_string(&evaluated).unwrap(),
            r#"{"evaluated":true,"max":0.25}"#
        );
        assert_eq!(
            serde_json::to_string(&unevaluated).unwrap(),
            r#"{"evaluated":false,"max":null}"#
        );
    }

    #[test]
    fn the_operation_record_carries_selectors_only_for_rest_bind() {
        let whole = OperationRecord::from(Operation::WholeDocument { factor: 0.01 });
        assert_eq!(
            serde_json::to_string(&whole).unwrap(),
            r#"{"kind":"whole-document","declared_factor":0.01}"#
        );
        let rest_bind = OperationRecord::from(Operation::RestBind {
            source_skin_index: 2,
            source_root_node_index: 3,
            expected_factor: 0.01,
        });
        assert_eq!(
            serde_json::to_string(&rest_bind).unwrap(),
            r#"{"kind":"rest-bind","declared_factor":0.01,"source_skin_index":2,"source_root_node_index":3}"#
        );
    }

    #[test]
    fn a_read_back_digest_mismatch_is_a_refusal_and_not_an_operator_error() {
        let proved = "a".repeat(64);
        assert!(
            require_read_back_match(&proved, &proved).is_ok(),
            "equal digests publish"
        );

        let read_back = "b".repeat(64);
        let Err(Failure::Refusal(rejection)) = require_read_back_match(&read_back, &proved) else {
            panic!("a digest mismatch must be a refusal, not an operator error");
        };
        assert_eq!(rejection.stage, Stage::ReadBack);
        assert_eq!(rejection.kind, "read-back-digest-mismatch");
        assert!(
            rejection.detail.contains(&read_back),
            "{}",
            rejection.detail
        );
        assert!(rejection.detail.contains(&proved), "{}", rejection.detail);
    }

    #[test]
    fn an_extension_that_is_not_gltf_or_glb_is_an_operator_error() {
        assert!(
            declared_container(Path::new("rig.fbx"), "input")
                .unwrap_err()
                .contains("self-contained glTF/GLB only")
        );
        assert!(
            declared_container(Path::new("rig"), "input")
                .unwrap_err()
                .contains("has no extension")
        );
        assert_eq!(
            declared_container(Path::new("rig.GLB"), "input").unwrap(),
            GltfContainerKind::Glb
        );
        assert_eq!(
            declared_container(Path::new("rig.gltf"), "output").unwrap(),
            GltfContainerKind::Gltf
        );
    }
}
