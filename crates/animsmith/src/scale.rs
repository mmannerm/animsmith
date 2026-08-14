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
//! A stdout that cannot take the record — a closed pipe, a full filesystem —
//! changes no row of that table: the write failure is diagnosed on stderr and
//! the outcome's exit code stands. By the time stdout is written the pair is
//! already published or the refusal is already a fact about the asset, so
//! raising it would report an operator error for work that was done, and
//! would turn `scale … --format json | head` on a refused asset into exit
//! `2`. A record that refuses to *serialize* is the opposite case and does
//! exit `2`: that record would be false, and [`Finite`] exists to stop it.
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
    destination_identity, emit, input_identity, parent_or_current, publish_pair, read_digest,
    require_writable_destination, serialize_record, sha256_hex,
};
use crate::{Format, render};
use animsmith_core::scale::{
    ScaleError, ScaleOperation, ScalePlan, ScaleProof, ScaleRequest, ScaleTolerancePolicy,
    plan_scale,
};
use animsmith_core::{DocumentShapeError, InputIdentity, ToolInfo};
use animsmith_gltf::{
    GltfCapabilityManifest, GltfCapabilityViolation, GltfContainerKind, GltfRawJsonDifference,
    GltfRawJsonDifferenceKind, GltfRawJsonDifferenceSummary, GltfScaleArtifact,
    GltfScaleArtifactProof, GltfScalePreflightError, GltfScaleRewriteError, GltfScaleSource,
    capability_facts, preflight_scale_source_bytes, prove_rewritten_artifact,
    prove_rewritten_rest_bind, rewrite_linear_units, rewrite_rest_bind,
};
use serde::ser::Error as _;
use serde::{Serialize, Serializer};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const SCALE_EVIDENCE_SCHEMA_VERSION: u32 = 3;
pub(crate) const SCALE_EVIDENCE_SCHEMA_ID: &str = "urn:animsmith:schema:scale-evidence:3";

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

/// One residual, and whether the proof that reports it actually compared
/// anything.
///
/// [`ScaleProof`] reports `0.0` for a residual nothing walked, which §D.6
/// calls a false record: "a record stating residual 0.0 for a claim nothing
/// checked". So the residual is never published flat. `max` is `null`
/// exactly when `evaluated` is false, and a consumer that reads only `max`
/// therefore sees an absence rather than a fabricated zero.
#[derive(Debug, Clone, Copy, Serialize)]
struct Residual {
    evaluated: bool,
    max: Option<Finite>,
}

impl Residual {
    /// Publish `max` iff the proof's own count says it compared something.
    ///
    /// Takes the comparison count rather than a `bool` so the only way to
    /// publish a residual is to hold the number of comparisons that produced
    /// it: there is no constructor a caller could hand a predicate of its
    /// own.
    fn measured(comparisons: usize, max: f64) -> Self {
        let evaluated = comparisons > 0;
        Self {
            evaluated,
            max: evaluated.then_some(Finite(max)),
        }
    }
}

/// Every residual [`ScaleProof`] reports, each carrying whether the proof
/// compared anything to produce it.
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
    f32_rounding_ulps: u32,
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
            f32_rounding_ulps: policy.f32_rounding_ulps,
        }
    }
}

/// Both observed-factor witnesses and the divergence between them, per §D.6.
///
/// The two witnesses are measured from deliberately different state — the raw
/// source projection's parent chain and the normalized skeleton's. Those two
/// chains are validated to describe the same tree, but nothing reconciles the
/// two *readings*: each composes its own stored transforms, which is why both
/// are recorded. The divergence is reported, never enforced;
/// `divergence_ceiling` is what the design expects of it.
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
    scale_animation: bool,
    inverse_binds: bool,
    base_mesh_positions: bool,
}

impl DomainRewritesRecord {
    /// Project the operation onto the five immutable evidence fields.
    ///
    /// Scale evidence v1-v3 froze these operation-level booleans. They describe
    /// the two fixed public operation contracts, not the payload present in one
    /// document and not an input to candidate construction or proof. Keeping
    /// the total mapping private preserves those schemas without recreating a
    /// public boolean bag beside the typed per-field ledger.
    fn from_operation(operation: ScaleOperation) -> Result<Self, String> {
        Ok(match operation {
            ScaleOperation::WholeDocumentLinearUnits { .. } => Self {
                rest_hierarchy: true,
                translation_animation: true,
                scale_animation: false,
                inverse_binds: true,
                base_mesh_positions: true,
            },
            ScaleOperation::RestBindUniformScale { .. } => Self {
                rest_hierarchy: true,
                translation_animation: true,
                scale_animation: true,
                inverse_binds: true,
                base_mesh_positions: false,
            },
            _ => {
                return Err(
                    "the scale operation has no scale-evidence v3 domain projection".to_owned(),
                );
            }
        })
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
    /// Bounded raw-JSON evidence for an artifact-preservation refusal.
    ///
    /// `null` says that this refusal did not compare raw JSON locations. It
    /// is deliberately separate from [`Self::violations`]: capability
    /// violations describe unsupported source domains, while these entries
    /// describe a failed preservation claim after rewriting.
    artifact_proof_differences: Option<ArtifactProofDifferencesRecord>,
}

/// A bounded raw-JSON difference sample from an artifact proof.
#[derive(Debug, Clone, Serialize)]
struct ArtifactProofDifferencesRecord {
    omitted: usize,
    items: Vec<ArtifactProofDifferenceRecord>,
}

impl From<GltfRawJsonDifferenceSummary> for ArtifactProofDifferencesRecord {
    fn from(summary: GltfRawJsonDifferenceSummary) -> Self {
        Self {
            omitted: summary.omitted,
            items: summary.differences.into_iter().map(Into::into).collect(),
        }
    }
}

/// One raw-JSON location the artifact proof found different.
#[derive(Debug, Clone, Serialize)]
struct ArtifactProofDifferenceRecord {
    location: String,
    kind: ArtifactProofDifferenceKindRecord,
}

impl From<GltfRawJsonDifference> for ArtifactProofDifferenceRecord {
    fn from(difference: GltfRawJsonDifference) -> Self {
        Self {
            location: difference.pointer,
            kind: difference.kind.into(),
        }
    }
}

/// Stable machine identity for a raw-JSON difference direction.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum ArtifactProofDifferenceKindRecord {
    ArtifactAdded,
    ArtifactRemoved,
    ValueChanged,
}

impl From<GltfRawJsonDifferenceKind> for ArtifactProofDifferenceKindRecord {
    fn from(kind: GltfRawJsonDifferenceKind) -> Self {
        match kind {
            GltfRawJsonDifferenceKind::ArtifactAdded => Self::ArtifactAdded,
            GltfRawJsonDifferenceKind::ArtifactRemoved => Self::ArtifactRemoved,
            GltfRawJsonDifferenceKind::ValueChanged => Self::ValueChanged,
            // This immutable v2 contract has exactly these three values. A
            // future frontend kind requires a new evidence identity, rather
            // than silently serializing a record v2 consumers cannot read.
            _ => unreachable!(
                "a new raw JSON difference kind requires a new scale-evidence schema version"
            ),
        }
    }
}

/// The immutable versioned scale-evidence contract, `scale-evidence:3`.
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
        artifact_proof_differences: None,
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
        ScaleError::PlanDocumentMismatch { .. } => "plan-document-mismatch",
        ScaleError::IncompleteClosure { .. } => "incomplete-closure",
        ScaleError::UnsupportedUnskinnedGeometry { .. } => "unsupported-unskinned-geometry",
        ScaleError::InvalidAffineDomain { .. } => "invalid-affine-domain",
        ScaleError::FactorMismatch { .. } => "factor-mismatch",
        ScaleError::MixedFactor { .. } => "mixed-factor",
        ScaleError::ProofResidualExceeded { .. } => "proof-residual-exceeded",
        ScaleError::ProofSamplingBudgetExceeded { .. } => "proof-sampling-budget-exceeded",
        ScaleError::MissingProofEvidence { .. } => "missing-proof-evidence",
        ScaleError::InvalidDocumentShape(error) => document_shape_error_kind(error),
        ScaleError::MissingInverseBind { .. } => "missing-inverse-bind",
        ScaleError::InvalidMeshPrimitive { .. } => "invalid-mesh-primitive",
        ScaleError::NegativeSkinWeight { .. } => "negative-skin-weight",
        ScaleError::InvalidSkinnedPrimitive { .. } => "invalid-skinned-primitive",
        ScaleError::CandidateStructureMismatch { .. } => "candidate-structure-mismatch",
        _ => "unclassified-scale-error",
    }
}

fn document_shape_error_kind(error: &DocumentShapeError) -> &'static str {
    match error {
        DocumentShapeError::NonFiniteSkeletonRest { .. }
        | DocumentShapeError::NonFiniteBoneInverseBind { .. } => "non-finite-transform",
        DocumentShapeError::InvalidSkeletonParent { .. } => "invalid-parent",
        DocumentShapeError::DuplicateSourceNodeIndex { .. } => "duplicate-source-node-index",
        DocumentShapeError::DuplicateSourceSkinIndex { .. } => "duplicate-source-skin-index",
        DocumentShapeError::SourceProjection { .. } => "parent-chain-disagreement",
        DocumentShapeError::DuplicateClipTrack { .. } => "duplicate-clip-track",
        DocumentShapeError::TrackShape { .. } => "invalid-track-shape",
        DocumentShapeError::MeshInstanceShape { .. } => "invalid-mesh-instance",
        _ => "unclassified-document-shape-error",
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
/// [`ScaleError::FactorNotRepresentable`] stays a refusal, and this is
/// settled rather than pending. The classification this function performs is
/// on the *variant*, and the variant is shared by three provenances that do
/// not classify alike: the operator's declared factor, the reciprocal
/// `1 / expected_factor` that the rest/bind plan derives, and the occurrences
/// the frontend rewrite raises against the asset's own magnitudes. Moving the
/// variant to `Failure::Operator` would therefore report a derived or
/// asset-side failure as a bad invocation — the mirror image of the mistake
/// the paragraph above avoids, and the more damaging direction, because it
/// suppresses the record entirely. Splitting it correctly needs
/// provenance-specific variants in [`ScaleError`], not a change to this
/// match.
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
    let (kind, violations, artifact_proof_differences) = match error {
        GltfScaleRewriteError::Capability { violations, .. } => {
            ("unsupported-source-domain", violations, None)
        }
        GltfScaleRewriteError::Plan(error) => (scale_error_kind(&error), Vec::new(), None),
        GltfScaleRewriteError::Load(_) => ("unreadable-source", Vec::new(), None),
        GltfScaleRewriteError::Write(_) => ("unwritable-container", Vec::new(), None),
        GltfScaleRewriteError::UnhandledLengthField { .. } => {
            ("unhandled-length-field", Vec::new(), None)
        }
        GltfScaleRewriteError::ConflictingRewriteRule { .. } => {
            ("conflicting-rewrite-rule", Vec::new(), None)
        }
        GltfScaleRewriteError::UnrewritableAccessor { .. } => {
            ("unrewritable-accessor", Vec::new(), None)
        }
        GltfScaleRewriteError::ConflictingNodeTransform { .. } => {
            ("conflicting-node-transform", Vec::new(), None)
        }
        GltfScaleRewriteError::NonAffineNodeMatrix { .. } => {
            ("non-affine-node-matrix", Vec::new(), None)
        }
        GltfScaleRewriteError::ImagePayloadOverlap { .. } => {
            ("image-payload-overlap", Vec::new(), None)
        }
        GltfScaleRewriteError::UnreassemblableContainer { .. } => {
            ("unreassemblable-container", Vec::new(), None)
        }
        GltfScaleRewriteError::ValueNotRepresentable { .. } => {
            ("value-not-representable", Vec::new(), None)
        }
        GltfScaleRewriteError::ConflictingRestBindFactor { .. } => {
            ("conflicting-rest-bind-factor", Vec::new(), None)
        }
        GltfScaleRewriteError::ClosureMismatch { .. } => ("closure-mismatch", Vec::new(), None),
        GltfScaleRewriteError::ParentChainDisagreement { .. } => {
            ("parent-chain-disagreement", Vec::new(), None)
        }
        GltfScaleRewriteError::AmbiguousSourceNodeProjection { .. } => {
            ("ambiguous-source-node-projection", Vec::new(), None)
        }
        GltfScaleRewriteError::UnusableSourceHierarchy { .. } => {
            ("unusable-source-hierarchy", Vec::new(), None)
        }
        GltfScaleRewriteError::ArtifactProofFailed {
            raw_json_differences,
            ..
        } => (
            "artifact-proof-failed",
            Vec::new(),
            raw_json_differences.map(Into::into),
        ),
        _ => ("unclassified-rewrite-error", Vec::new(), None),
    };
    Failure::Refusal(RejectionRecord {
        stage,
        kind,
        detail,
        violations,
        artifact_proof_differences,
    })
}

// --- Measured residual coverage ----------------------------------------------

/// Which residuals `prove_scale` actually evaluated, read from the
/// comparison counts it publishes.
///
/// **Measured, not re-derived.** Each `evaluated` is `count > 0` for the
/// count [`ScaleProof`] writes at the point of comparison, beside the
/// maximum that same comparison raises. Nothing here inspects the
/// plan's obligations or the source's payloads: an obligation flag is at
/// best a proxy for "the loop ran", exact for the nine obligations
/// `plan_scale` gates on evidence and unavailable for the three
/// `prove_scale` proves unconditionally — and a source-side predicate for
/// those three would have to restate, across a crate boundary and enforced
/// by nothing, private resolution rules that belong to `prove_scale`.
///
/// The published `{evaluated, max}` shape is unchanged; only the source of
/// `evaluated` is. `max` is still `null` exactly when nothing was compared,
/// which is what keeps §D.6's "a record stating residual 0.0 for a claim
/// nothing checked" out of the record.
fn residuals(proof: &ScaleProof) -> ResidualsRecord {
    ResidualsRecord {
        rest_translation: Residual::measured(
            proof.rest_translation_comparisons,
            proof.rest_translation_residual,
        ),
        rest_rotation: Residual::measured(
            proof.rest_rotation_comparisons,
            proof.rest_rotation_residual,
        ),
        unit_scale: Residual::measured(proof.unit_scale_comparisons, proof.unit_scale_residual),
        transform_only_affine: Residual::measured(
            proof.transform_only_affine_comparisons,
            proof.transform_only_affine_residual,
        ),
        track_value: Residual::measured(proof.track_value_comparisons, proof.track_value_residual),
        mesh_position: Residual::measured(
            proof.mesh_position_comparisons,
            proof.mesh_position_residual,
        ),
        key_translation: Residual::measured(
            proof.key_translation_comparisons,
            proof.key_translation_residual,
        ),
        cubic_interior: Residual::measured(
            proof.cubic_interior_comparisons,
            proof.cubic_interior_residual,
        ),
        trajectory: Residual::measured(proof.trajectory_comparisons, proof.trajectory_residual),
        skin_matrix: Residual::measured(proof.skin_matrix_comparisons, proof.skin_matrix_residual),
        bounds: Residual::measured(proof.bounds_comparisons, proof.bounds_residual),
        unaffected_inverse_bind: Residual::measured(
            proof.unaffected_inverse_bind_comparisons,
            proof.unaffected_inverse_bind_residual,
        ),
    }
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
                artifact_proof_differences: None,
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
                artifact_proof_differences: None,
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
                artifact_proof_differences: None,
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
            domain_rewrites: DomainRewritesRecord::from_operation(plan.operation())?,
            proof: ProofRecord {
                sample_time_count: proof.core.sample_time_count,
                residuals: residuals(&proof.core),
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
        // The very bytes the evidence file received, not a second rendering
        // of the same record. A stdout that cannot take them is diagnosed
        // rather than raised: the pair is on disk, and a run that published
        // it does not report an operator error.
        Format::Json => emit(&evidence_bytes),
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
        // Serialized once and emitted. A value `Finite` refuses is still an
        // operator error — it means this record would be *false*, and no
        // exit code should stand behind that. A stdout that cannot take a
        // record that serialized fine is only a reporting failure, so the
        // refusal keeps exit `1` rather than inverting into `2`.
        Format::Json => emit(&serialize_record(&record)?),
        Format::Text => eprint!("{summary}"),
    }
    Ok(ExitCode::from(crate::EXIT_FINDINGS))
}

#[cfg(test)]
mod tests {
    use super::*;
    use animsmith_core::{
        MeshInstanceShapeViolation, Property, SourceProjectionViolation, TrackShapeViolation,
    };

    #[test]
    fn shared_document_shape_errors_keep_specific_evidence_kinds() {
        let cases = [
            (
                DocumentShapeError::NonFiniteSkeletonRest { node: 0 },
                "non-finite-transform",
            ),
            (
                DocumentShapeError::InvalidSkeletonParent { node: 1, parent: 2 },
                "invalid-parent",
            ),
            (
                DocumentShapeError::DuplicateSourceNodeIndex {
                    source_node_index: 3,
                },
                "duplicate-source-node-index",
            ),
            (
                DocumentShapeError::DuplicateSourceSkinIndex {
                    source_skin_index: 4,
                },
                "duplicate-source-skin-index",
            ),
            (
                DocumentShapeError::SourceProjection {
                    source_node_index: 5,
                    violation: SourceProjectionViolation::ParentSourceNodeMissing,
                },
                "parent-chain-disagreement",
            ),
            (
                DocumentShapeError::DuplicateClipTrack {
                    clip_index: 6,
                    node: 7,
                    property: Property::Translation,
                },
                "duplicate-clip-track",
            ),
            (
                DocumentShapeError::TrackShape {
                    clip_index: 8,
                    node: 9,
                    violation: TrackShapeViolation::EmptyTimes,
                },
                "invalid-track-shape",
            ),
            (
                DocumentShapeError::MeshInstanceShape {
                    instance_index: 10,
                    violation: MeshInstanceShapeViolation::MeshIndexOutOfRange,
                },
                "invalid-mesh-instance",
            ),
            (
                DocumentShapeError::NonFiniteBoneInverseBind { node: 11 },
                "non-finite-transform",
            ),
        ];

        for (error, expected) in cases {
            assert_eq!(
                scale_error_kind(&ScaleError::InvalidDocumentShape(error)),
                expected
            );
        }

        for violation in [
            SourceProjectionViolation::ProjectedBoneOutOfRange,
            SourceProjectionViolation::TwoSourceNodesProjectToOneBone,
            SourceProjectionViolation::ParentSourceNodeMissing,
            SourceProjectionViolation::CyclicUnprojectedSourceParentChain,
            SourceProjectionViolation::NearestProjectedParentMismatch,
            SourceProjectionViolation::ProjectedBoneHasUnprojectedSkeletonChild,
        ] {
            assert_eq!(
                scale_error_kind(&ScaleError::InvalidDocumentShape(
                    DocumentShapeError::SourceProjection {
                        source_node_index: 0,
                        violation,
                    },
                )),
                "parent-chain-disagreement",
                "source projection violation {violation}"
            );
        }

        for violation in [
            TrackShapeViolation::BoneIndexOutOfRange,
            TrackShapeViolation::EmptyTimes,
            TrackShapeViolation::NonFiniteTime,
            TrackShapeViolation::TimesNotStrictlyIncreasing,
            TrackShapeViolation::ValueCountMismatch,
            TrackShapeViolation::ValueTypeMismatchesProperty,
            TrackShapeViolation::NonFiniteValue,
        ] {
            assert_eq!(
                scale_error_kind(&ScaleError::InvalidDocumentShape(
                    DocumentShapeError::TrackShape {
                        clip_index: 0,
                        node: 0,
                        violation,
                    },
                )),
                "invalid-track-shape",
                "track-shape violation {violation}"
            );
        }

        for violation in [
            MeshInstanceShapeViolation::NodeIndexOutOfRange,
            MeshInstanceShapeViolation::MeshIndexOutOfRange,
            MeshInstanceShapeViolation::SkinJointOutOfRange,
            MeshInstanceShapeViolation::SkinInverseBindCountMismatch,
            MeshInstanceShapeViolation::NonFiniteSkinInverseBind,
        ] {
            assert_eq!(
                scale_error_kind(&ScaleError::InvalidDocumentShape(
                    DocumentShapeError::MeshInstanceShape {
                        instance_index: 0,
                        violation,
                    },
                )),
                "invalid-mesh-instance",
                "mesh-instance violation {violation}"
            );
        }
    }

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
        // One comparison that found `0.25`, and a residual nothing compared.
        let evaluated = Residual::measured(1, 0.25);
        let unevaluated = Residual::measured(0, 0.0);
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
    fn artifact_proof_difference_diagnostics_preserve_locations_and_kinds() {
        let summary = GltfRawJsonDifferenceSummary {
            omitted: 1,
            differences: vec![
                GltfRawJsonDifference {
                    pointer: "/nodes/1/translation/0".to_owned(),
                    kind: GltfRawJsonDifferenceKind::ValueChanged,
                },
                GltfRawJsonDifference {
                    pointer: "/nodes/2".to_owned(),
                    kind: GltfRawJsonDifferenceKind::ArtifactAdded,
                },
                GltfRawJsonDifference {
                    pointer: "/nodes/3".to_owned(),
                    kind: GltfRawJsonDifferenceKind::ArtifactRemoved,
                },
            ],
        };

        assert_eq!(
            serde_json::to_string(&ArtifactProofDifferencesRecord::from(summary)).unwrap(),
            r#"{"omitted":1,"items":[{"location":"/nodes/1/translation/0","kind":"value_changed"},{"location":"/nodes/2","kind":"artifact_added"},{"location":"/nodes/3","kind":"artifact_removed"}]}"#
        );
    }

    #[test]
    fn artifact_proof_failure_maps_its_differences_into_the_shipped_rejection() {
        let differences = (0..16)
            .map(|index| GltfRawJsonDifference {
                pointer: format!("/nodes/{index}"),
                kind: match index % 3 {
                    0 => GltfRawJsonDifferenceKind::ValueChanged,
                    1 => GltfRawJsonDifferenceKind::ArtifactAdded,
                    _ => GltfRawJsonDifferenceKind::ArtifactRemoved,
                },
            })
            .collect();
        let error = GltfScaleRewriteError::ArtifactProofFailed {
            claim: "preserved raw JSON",
            observed: 20.0,
            tolerance: 0.0,
            raw_json_differences: Some(GltfRawJsonDifferenceSummary {
                omitted: 4,
                differences,
            }),
        };

        let Failure::Refusal(rejection) = rewrite_failure(Stage::Proof, error) else {
            panic!("an artifact proof failure is a typed refusal");
        };
        assert_eq!(rejection.stage, Stage::Proof);
        assert_eq!(rejection.kind, "artifact-proof-failed");
        assert_eq!(rejection.violations, Vec::new());
        assert_eq!(
            rejection.detail,
            "artifact proof claim \"preserved raw JSON\" observed 20, tolerance 0; raw JSON differences: /nodes/0 (value-changed), /nodes/1 (artifact-added), /nodes/2 (artifact-removed), /nodes/3 (value-changed), /nodes/4 (artifact-added), /nodes/5 (artifact-removed), /nodes/6 (value-changed), /nodes/7 (artifact-added), /nodes/8 (artifact-removed), /nodes/9 (value-changed), /nodes/10 (artifact-added), /nodes/11 (artifact-removed), /nodes/12 (value-changed), /nodes/13 (artifact-added), /nodes/14 (artifact-removed), /nodes/15 (value-changed); 4 omitted"
        );
        let expected_items: Vec<_> = (0..16)
            .map(|index| {
                let kind = match index % 3 {
                    0 => "value_changed",
                    1 => "artifact_added",
                    _ => "artifact_removed",
                };
                serde_json::json!({
                    "location": format!("/nodes/{index}"),
                    "kind": kind,
                })
            })
            .collect();
        assert_eq!(
            serde_json::to_value(rejection.artifact_proof_differences).unwrap(),
            serde_json::json!({
                "omitted": 4,
                "items": expected_items,
            })
        );
    }

    #[test]
    fn artifact_proof_failure_without_a_json_walk_maps_to_null_differences() {
        let error = GltfScaleRewriteError::ArtifactProofFailed {
            claim: "an earlier artifact claim",
            observed: 1.0,
            tolerance: 0.0,
            raw_json_differences: None,
        };
        let Failure::Refusal(rejection) = rewrite_failure(Stage::Proof, error) else {
            panic!("an artifact proof failure is a typed refusal");
        };
        assert!(rejection.artifact_proof_differences.is_none());
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
