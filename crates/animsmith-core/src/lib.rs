//! Engine-agnostic animation linting primitives for Rust pipelines.
//!
//! This crate is the embedding boundary for animsmith. It owns the core
//! data model ([`Document`], [`Skeleton`], [`Clip`], [`Track`]), rig-role
//! resolution ([`detect_profile`], [`ResolvedRoles::from_names`]),
//! typed configuration ([`Config`]), measurement generation
//! ([`measure::measure_document`]), versioned result envelopes
//! ([`contract::MeasureEnvelope`], [`contract::LintEnvelope`]), measurement diffs
//! ([`diff::diff_measurements`]), structured findings ([`Finding`]), and
//! check execution ([`CheckCtx`], [`all_checks`], [`evaluate_checks`]).
//! The [`source_facts`] module owns the bounded, format-neutral V1 vocabulary
//! that format loaders bind to the exact primary bytes in an immutable
//! [`LoadedSource`]. A mutable normalized [`Document`] does not reconstruct
//! importer-sensitive source declarations; consuming the wrapper as a document
//! deliberately discards those facts. The separate [`dependency_closure`]
//! sidecar records bounded, same-load primary/external content identities over
//! the raw resource-declaration domain; format crates own rooted I/O while core
//! owns its validated value and canonical digest contract. The borrowing facts
//! view reuses the canonical [`model::SourceSkeletonAssets`] table and remains
//! separate from scale's operation-specific capability and proof ledgers.
//! The opt-in [`bake_static_mesh_transforms`] operation canonicalizes supported
//! unanimated, unskinned mesh scenes into identity-root geometry and returns
//! deterministic producer evidence.
//! The opt-in [`transform::prune_constant_tracks`] helper removes only
//! interpolation-aware constant-track candidates whose sampled local and
//! model-space pose evidence remains within its documented tolerances.
//! The [`scale`] module owns the format-neutral plan/proof contracts for the
//! two distinct DESIGN.md Appendix D scale operations —
//! [`scale::ScaleOperation::WholeDocumentLinearUnits`] and
//! [`scale::ScaleOperation::RestBindUniformScale`] — through pure, fail-closed
//! [`scale::plan_scale`] and independent [`scale::prove_scale`]. A format
//! frontend owns exact source rewriting and hands the reloaded emitted
//! document back through [`scale::ScaleCandidate::from_document`]; core does
//! not expose a production candidate builder, decide selectors, publish
//! artifacts, or write files.
//! The [`animsmith-gltf`] and [`animsmith-fbx`] loader crates translate file
//! formats into this model; their docs.rs pages continue the library path for
//! format-specific loading and, for glTF, writing.
//!
//! The [embedding guide] explains crate selection and integration
//! boundaries. The [pipeline scenario guide] shows where an embedded gate
//! fits in marketplace intake, mocap cleanup, outsourced acceptance, and CI.
//! A [runnable example] exercises the complete library flow.
//!
//! [embedding guide]: https://github.com/mmannerm/animsmith/blob/main/docs/embedding.md
//! [pipeline scenario guide]: https://github.com/mmannerm/animsmith/blob/main/docs/pipeline-scenarios.md
//! [runnable example]: https://github.com/mmannerm/animsmith/blob/main/crates/animsmith/examples/embed.rs
//! [`animsmith-gltf`]: https://docs.rs/animsmith-gltf
//! [`animsmith-fbx`]: https://docs.rs/animsmith-fbx
//!
//! # Quick start
//!
//! After a format crate has loaded a [`Document`], resolve rig roles, build
//! a [`Config`] from the host pipeline's contract, and share one
//! [`MetricGrids`] between measurements, checks, and optional report
//! generation:
//!
//! ```
//! use animsmith_core::{
//!     CheckCtx, CheckSelection, Config, Document, MetricGrids, all_checks,
//!     evaluate_checks, resolve_configured_roles,
//! };
//! use animsmith_core::measure::measure_document;
//!
//! let doc = Document::default();
//! let config = Config::default();
//! config.validate()?;
//! let roles = resolve_configured_roles(&doc.skeleton, &config.rig);
//! let grids = MetricGrids::new(&doc);
//!
//! let measurements = measure_document(&grids, &roles, &config);
//! let ctx = CheckCtx::new(&grids, &roles, &config);
//! let results = evaluate_checks(&ctx, &all_checks(), CheckSelection::All)?;
//!
//! assert!(measurements.is_empty());
//! assert!(results.iter().all(|result| result.findings().is_empty()));
//! # Ok::<(), animsmith_core::EvaluationError>(())
//! ```
//!
//! [`CheckCtx::new`] consumes already-resolved roles; it does not interpret
//! [`Config::rig`] automatically. Frontends may use [`detect_profile`],
//! [`resolve_configured_roles`] for the same named-profile plus inline-override
//! policy as the CLI. Missing prerequisites are represented as typed coverage
//! gaps rather than false findings.
//!
//! # API status
//!
//! The Rust API is pre-1.0 and may still change before the first stable
//! release. The intended extension points are the data model,
//! configuration types, measurement and diff APIs, rig-profile APIs, the
//! [`Check`] trait for custom checks, and the check catalog functions
//! re-exported from this crate root. Built-in check ids, CLI exit-code
//! semantics, and the shared versioned JSON envelope/schema ids are treated
//! as the most stable automation contracts. The [`contract`] module owns the
//! same envelope types and immutable identities for CLI and embedded
//! producers. The scene-asset
//! structs in [`model`] and the pipeline-mechanical helpers in
//! [`transform`] and [`static_bake`] are public so the loader, writer, and CLI crates can
//! share the same model, but they are less settled than the
//! measurement/check embedding flow while the crate is pre-1.0. Metric
//! formulas and individual Rust symbols are still subject to pre-1.0
//! refinement.
//!
//! Public APIs that return [`Result`] document their `# Errors` cases.
//! Index-based accessors and transform helpers that rely on
//! loader-established invariants document their `# Panics` contracts.
//! Loader-valid documents from the format crates should flow through
//! checking, sampling, and measurement without panicking on untrusted
//! input.

#![warn(missing_docs)]

pub mod assembly;
pub mod check;
mod checks;
pub mod config;
pub mod contract;
pub mod dependency_closure;
pub mod diff;
pub mod evaluation;
pub mod finding;
#[cfg(feature = "fixtures")]
pub mod fixtures;
pub mod measure;
pub mod metrics;
pub mod model;
pub mod profile;
pub mod sample;
pub mod scale;
pub mod skinned_canonical;
pub mod source_facts;
pub mod static_bake;
pub mod transform;

pub use check::{Check, CheckCtx, all_checks, mechanical_checks};
pub use config::{
    ClipExpectations, Config, ConfigValidationError, GaitGroup, MovementOwner, Pinned,
    RuntimeNodeSelectorResolution, RuntimeNodeSelectors, RuntimeNodesConfig, SeveritySetting,
    SyncGroup, TimeComplementSettings,
};
pub use contract::{
    DiffEnvelope, InputIdentity, LintEnvelope, LintFileReport, MEASUREMENTS_SCHEMA_ID,
    MEASUREMENTS_SCHEMA_VERSION, MeasureEnvelope, MeasureFileReport, MeasurementContract,
    MeasurementContractError, MeasurementFileError, MeasurementReportError, MeasurementReportFile,
    MeasurementReportInput, OUTPUT_SCHEMA_ID, OUTPUT_SCHEMA_VERSION, RigInfo, RigInfoError,
    ToolInfo, ToolSource, sha256_hex,
};
pub use dependency_closure::{
    DEPENDENCY_CLOSURE_BUDGET_V1_ID, DEPENDENCY_CLOSURE_V1_ID,
    DEPENDENCY_CLOSURE_V1_MAX_DEDUP_PROBES, DEPENDENCY_CLOSURE_V1_MAX_EXTERNAL_RESOURCES,
    DEPENDENCY_CLOSURE_V1_MAX_KEY_BYTES, DEPENDENCY_CLOSURE_V1_MAX_NORMALIZATION_BYTES,
    DEPENDENCY_CLOSURE_V1_MAX_PATH_COMPONENTS, DEPENDENCY_CLOSURE_V1_MAX_REFERENCES,
    DEPENDENCY_CLOSURE_V1_MAX_RESOURCE_BYTES, DEPENDENCY_CLOSURE_V1_MAX_TOTAL_RESOURCE_BYTES,
    DependencyClosureBuilderV1, DependencyClosureCoverageReasonV1, DependencyClosureCoverageV1,
    DependencyClosureError, DependencyClosureIdentityV1, DependencyClosureReferenceV1,
    DependencyClosureV1, DependencyClosureWorkV1, DependencyReferenceTargetV1,
    DependencyResourceKeyV1, DependencyResourcePurposeV1, DependencyResourceRefusalReasonV1,
    DependencyResourceUnavailableReasonV1, ExternalResourceIdentityV1, ResourceClosureBudgetV1,
    ResourceKeySyntaxV1,
};
pub use evaluation::{
    Applicability, BUILTIN_COVERAGE_GAP_CODES, BUILTIN_EVALUATION_SCOPE_CODES, CheckEvaluation,
    CheckOutput, CheckSelection, ConfigurationState, CoverageGap, CoverageGapCode, EvaluationError,
    EvaluationScope, EvaluationScopeCode, EvaluationState, SelectionState, evaluate_checks,
};
pub use finding::{Finding, MemberMeasurement, Severity, Value};
/// Re-export of the exact `glam` version used by animsmith's public math
/// types, so embedders can construct [`Transform`] values without a
/// cross-version type mismatch.
pub use glam;
pub use metrics::MetricGrids;
pub use model::{
    AdditionalInfluenceSet, AffineDomainViolation, Bone, BoneId, Clip, DecodedImageColorType,
    Document, DocumentShapeError, ImageContainerFormat, ImageSourceKind, ImageUnavailableReason,
    Interpolation, MaterialResourceAssets, MaterialResourceCoverage, MaterialTextureSlot,
    MeshInstanceShapeViolation, Property, Skeleton, SourceImageAsset, SourceImageInspection,
    SourceInfo, SourceInverseBindAccessor, SourceInverseBindAccessorStatus, SourceMaterialAsset,
    SourceMaterialTextureBinding, SourceNodeAsset, SourceNodeLocalRest, SourceProjectionViolation,
    SourceSkeletonAssets, SourceSkeletonCoverage, SourceSkinAsset, SourceSkinAttachment,
    SourceTextureAsset, Track, TrackShapeViolation, TrackValues, Transform,
    validate_document_shape,
};
pub use profile::{
    ResolvedRoles, RigProfile, Role, builtin_profiles, detect_profile, resolve_configured_roles,
};
pub use sample::{PoseGrid, TrackSample, default_frame_count, sample_clip, sample_track};
pub use scale::{
    ProofResidualKind, ScaleBoneRestField, ScaleCandidate, ScaleCapabilityCoverage,
    ScaleCapabilityFacts, ScaleError, ScaleFieldDisposition, ScaleFieldPlan, ScaleFieldTarget,
    ScaleOperation, ScalePayloadShapeRow, ScalePlan, ScalePlanLedger, ScaleProjectedRole,
    ScaleProof, ScaleProofObligation, ScaleProofResidual, ScaleRequest, ScaleRewriteRule,
    ScaleSourceNodeKind, ScaleSourceRestField, ScaleSourceTopologyRow, ScaleTolerancePolicy,
    plan_scale, prove_scale,
};
pub use skinned_canonical::{
    SkinnedBindPoseCanonicalization, SkinnedBindPoseCanonicalizationError,
    SkinnedBindPoseCanonicalizationOptions, SkinnedBindPosePlacement,
    canonicalize_skinned_bind_pose,
};
pub use source_facts::{
    LoadedSource, RAW_SOURCE_FACTS_V1_ID, RAW_SOURCE_V1_MAX_CLIPS, RAW_SOURCE_V1_MAX_OBSERVATIONS,
    RAW_SOURCE_V1_MAX_RESOURCE_REFERENCES, RAW_SOURCE_V1_MAX_TEXT_BYTES,
    RAW_SOURCE_V1_MAX_TOTAL_TEXT_BYTES, RAW_SOURCE_V1_MAX_TRAVERSAL_DEPTH, RawSourceFactsBuilderV1,
    RawSourceFactsV1, SourceAxisV1, SourceChannelFactV1, SourceChannelPropertyV1, SourceClipFactV1,
    SourceComponentMaskV1, SourceConstructFactV1, SourceConstructKindV1, SourceCoordinateBasisV1,
    SourceFactDomainV1, SourceFactSetV1, SourceFactsError, SourceFactsViewV1, SourceFormatV1,
    SourceFramesPerSecondV1, SourceHandednessV1, SourceInterpolationV1, SourceLinearUnitV1,
    SourceLoaderDispositionV1, SourceLogicalLocatorV1, SourceObservationStateV1,
    SourceObservationV1, SourceProjectionWorkV1, SourceProvenanceKindV1, SourceProvenanceV1,
    SourceRelativeLocatorV1, SourceResourceKindV1, SourceResourceLocatorV1,
    SourceResourceReferenceV1, SourceSetCoverageStateV1, SourceSetCoverageV1, SourceTargetKindV1,
    SourceTargetV1, SourceTextV1, SourceTimeRangeV1, SourceUnavailableReasonV1,
};
pub use static_bake::{
    StaticMeshBake, StaticMeshBakeError, StaticMeshBakeEvidence, StaticMeshBakeInstanceEvidence,
    bake_static_mesh_transforms,
};
