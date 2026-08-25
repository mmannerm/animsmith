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
//! not expose a production candidate builder, choose named versus indexed
//! selectors, publish artifacts, or write files. Core does own the
//! format-neutral mapping from an already chosen exact named assembly selector
//! to its source root and fully governed skin through
//! [`scale::resolve_assembly_scale_named_selector`].
//! The [`animsmith-gltf`] and [`animsmith-fbx`] loader crates translate file
//! formats into this model; their docs.rs pages continue the library path for
//! format-specific loading and, for glTF, writing.
//!
//! The [embedding guide] explains crate selection and integration
//! boundaries. The [pipeline scenario guide] shows where an embedded gate
//! fits in marketplace intake, mocap cleanup, outsourced acceptance, and CI.
//! A [runnable example] exercises the complete library flow.
//!
//! The directional-speed policy V1 API freezes source-basis vectors as
//! orientation witnesses for raw collection-output V3 +X/+Z endpoint
//! displacement. Basis magnitudes are nonsemantic; evaluation uses
//! unit axes for heading and the raw evidence identity for binding.
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
mod bounded_deserialize;
pub mod check;
mod checks;
pub mod collection;
pub mod config;
pub mod contact_fragment;
pub mod contract;
pub mod dependency_closure;
pub mod diff;
pub mod directional_speed_evaluation;
pub mod directional_speed_policy;
pub mod engine_contract;
pub mod evaluation;
pub mod finding;
#[cfg(feature = "fixtures")]
pub mod fixtures;
pub mod measure;
pub mod metrics;
pub mod model;
pub mod prediction;
pub mod profile;
pub mod sample;
pub mod scale;
pub mod skinned_canonical;
pub mod source_facts;
pub mod stance_support;
pub mod static_bake;
pub mod transform;
pub mod transition_family;
/// Strict core-only transition-pose evaluation and skeleton-basis identity.
pub mod transition_pose_evaluation;

pub use check::{Check, CheckCtx, all_checks, mechanical_checks};
pub use collection::{
    COLLECTION_MANIFEST_V1_BUDGET_ID, COLLECTION_MANIFEST_V1_ID,
    COLLECTION_MANIFEST_V1_MAX_AGGREGATE_MEMBERS, COLLECTION_MANIFEST_V1_MAX_AGGREGATE_WORK,
    COLLECTION_MANIFEST_V1_MAX_CLIPS, COLLECTION_MANIFEST_V1_MAX_IDENTIFIER_BYTES,
    COLLECTION_MANIFEST_V1_MAX_MANIFEST_BYTES, COLLECTION_MANIFEST_V1_MAX_RUNTIME_SETS,
    COLLECTION_MANIFEST_V1_MAX_SOURCES, COLLECTION_MANIFEST_V1_MAX_TAKE_NAME_BYTES,
    COLLECTION_MANIFEST_V1_SCHEMA_VERSION, CollectionClipV1, CollectionDigestPinV1, CollectionIdV1,
    CollectionLogicalIdV1, CollectionManifestBudgetV1, CollectionManifestError,
    CollectionManifestV1, CollectionRuntimeSetKindV1, CollectionRuntimeSetV1,
    CollectionSourceKeyV1, CollectionSourceV1,
};
pub use config::{
    ClipExpectations, Config, ConfigValidationError, GaitGroup, MovementOwner, Pinned,
    RuntimeNodeSelectorResolution, RuntimeNodeSelectors, RuntimeNodesConfig, SeveritySetting,
    SyncGroup, TimeComplementSettings,
};
pub use contact_fragment::{
    CONTACT_FRAGMENT_V1_ID, CONTACT_FRAGMENT_V1_MAX_CANONICAL_BYTES, CONTACT_FRAGMENT_V1_MAX_DEPTH,
    CONTACT_FRAGMENT_V1_MAX_EVENTS, CONTACT_FRAGMENT_V1_MAX_EXTENSION_PAYLOAD_BYTES,
    CONTACT_FRAGMENT_V1_MAX_EXTENSION_PAYLOAD_DEPTH, CONTACT_FRAGMENT_V1_MAX_EXTENSIONS,
    CONTACT_FRAGMENT_V1_MAX_IDENTIFIER_BYTES, CONTACT_FRAGMENT_V1_MAX_SAFE_INTEGER,
    CONTACT_FRAGMENT_V1_MAX_SOURCE_BYTES, CONTACT_FRAGMENT_V1_MAX_TEXT_BYTES,
    CONTACT_FRAGMENT_V1_SCHEMA_VERSION, ContactClipReferenceV1, ContactEventKindV1, ContactEventV1,
    ContactEventWindowV1, ContactExtensionV1, ContactFragmentError, ContactFragmentV1,
    ContactPhaseV1, ContactProducerV1, ContactRoleV1,
};
pub use contract::{
    DiffEnvelope, InputIdentity, LintEnvelope, LintFileReport, MEASUREMENTS_SCHEMA_ID,
    MEASUREMENTS_SCHEMA_VERSION, MeasureEnvelope, MeasureFileReport, MeasurementContract,
    MeasurementContractError, MeasurementFileError, MeasurementReportError, MeasurementReportFile,
    MeasurementReportInput, MeasurementReportReadError, OUTPUT_SCHEMA_ID, OUTPUT_SCHEMA_VERSION,
    OUTPUT_V10_SCHEMA_ID, OUTPUT_V11_MAX_CHECKS_PER_FILE, OUTPUT_V11_MAX_FILES,
    OUTPUT_V11_MAX_REPORT_BYTES, OutputContractError, RigInfo, RigInfoError, ToolInfo, ToolSource,
    sha256_hex,
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
pub use directional_speed_evaluation::{
    COLLECTION_DIRECTIONAL_SPEED_EVALUATION_V1_ID,
    COLLECTION_DIRECTIONAL_SPEED_EVALUATION_V1_SCHEMA_VERSION,
    CollectionDirectionalSpeedEvaluationControlError, CollectionDirectionalSpeedEvaluationV1,
    CollectionDirectionalSpeedEvidenceMemberV1, CollectionDirectionalSpeedEvidenceV1,
    CollectionDirectionalSpeedFindingV1, CollectionDirectionalSpeedLifecycleV1,
    CollectionDirectionalSpeedNotEvaluatedReasonV1, evaluate_collection_directional_speed_v1,
};
pub use directional_speed_policy::{
    COLLECTION_DIRECTIONAL_SPEED_EVIDENCE_V1_MAX_BYTES, COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_ID,
    COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_MAX_AXIS_COSINE,
    COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_MAX_BYTES,
    COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_MAX_COMPONENT,
    COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_MAX_DIRECTION_TOLERANCE_DEG,
    COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_MAX_MEMBERS,
    COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_MAX_SCALAR,
    COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_SCHEMA_VERSION,
    CollectionDirectionalSpeedDiagonalBehaviorV1, CollectionDirectionalSpeedManifestIdentityV1,
    CollectionDirectionalSpeedMemberV1, CollectionDirectionalSpeedModeV1,
    CollectionDirectionalSpeedPolicyError, CollectionDirectionalSpeedPolicyV1,
    CollectionDirectionalSpeedSourceBasisV1,
};
pub use engine_contract::{
    ENGINE_CONTRACT_V1_MAX_AGGREGATE_ROWS, ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
    ENGINE_CONTRACT_V1_MAX_TEXT_BYTES, ENGINE_CONTRACT_V1_MAX_TOTAL_TEXT_BYTES,
    ENGINE_PROFILE_FACTS_V1_ID, EngineAnimationAddressabilityV1, EngineBakeOrExtractV1,
    EngineClipSettingsV1, EngineContractError, EngineConversionControlV1, EngineCoordinateBasisV1,
    EngineDefaultStatusV1, EngineFactIdV1, EngineFactStateV1, EngineFactValueV1,
    EngineForwardAxisV1, EngineHandednessV1, EngineImportHandlingV1, EngineLinearUnitV1,
    EnginePrimarySourceV1, EngineProfileFactV1, EngineProfileSelectionV1,
    EngineRootMotionAddressabilityV1, EngineSettingApplicabilityV1, EngineSettingDescriptorV1,
    EngineSettingDomainV1, EngineSettingIdV1, EngineSettingRowV1, EngineSettingScopeV1,
    EngineSettingValueV1, EngineTargetAddressabilityV1, EngineUpAxisV1,
    RESOLVED_ENGINE_SETTINGS_V1_ID, ResolvedEngineProfileV1, ResolvedEngineSettingsV1,
};
pub use evaluation::{
    Applicability, BUILTIN_COVERAGE_GAP_CODES, BUILTIN_EVALUATION_SCOPE_CODES, CheckEvaluation,
    CheckOutput, CheckSelection, ConfigurationState, CoverageGap, CoverageGapCode, EvaluationError,
    EvaluationScope, EvaluationScopeCode, EvaluationState, SelectionState, evaluate_checks,
    lint_requires_failure,
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
pub use prediction::{
    ENGINE_PREDICTION_V1_ID, EnginePredictionBasisV1, EnginePredictionFacetStateV1,
    EnginePredictionFacetV1, EnginePredictionV1, FinitePredictionNumberV1, MeasurementPointerV1,
    PREDICTION_PROVENANCE_V1_ID, PREDICTION_V1_MAX_AGGREGATE_PROVENANCE_ROWS,
    PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FACET, PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE,
    PREDICTION_V1_MAX_FACETS_PER_FILE, PREDICTION_V1_MAX_MEASUREMENT_POINTER_COMPONENTS,
    PREDICTION_V1_MAX_REASONS_PER_FACET, PREDICTION_V1_MAX_TEXT_BYTES,
    PREDICTION_V1_MAX_TOTAL_TEXT_BYTES_PER_FILE, PredictionBasisIdentityV1,
    PredictionBasisReferenceV1, PredictionContractError, PredictionProvenanceIdentityV1,
    PredictionProvenanceV1, PredictionScalarV1, PredictionUnavailableReasonV1, RawSourceAxisV1,
    RawSourceBasisReferenceV1, RawSourceBindingV1, RawSourceCoordinateBasisV1,
    RawSourceDispositionV1, RawSourceDomainV1, RawSourceFieldIdV1, RawSourceKeyV1,
    RawSourceObservationStateWireV1, RawSourceObservationWireV1, RawSourceProjectionWorkWireV1,
    RawSourceProvenanceKindV1, RawSourceProvenanceV1, RawSourceSetCoverageStateV1,
    RawSourceSetCoverageV1, RawSourceUnavailableReasonV1, ResolvedSettingLocationV1,
    SourceSkeletonRowKindV1,
};
pub use profile::{
    ResolutionOutcome, ResolvedRoles, RigProfile, Role, RoleResolutionPolicy, builtin_profiles,
    detect_profile, detect_profile_detailed, resolve_configured_roles, resolve_named,
    resolve_named_detailed,
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
pub use stance_support::{
    ResolvedStanceSupportV1, StanceSideV1, StanceSupportRunV1, resolve_stance_support_v1,
};
pub use static_bake::{
    StaticMeshBake, StaticMeshBakeError, StaticMeshBakeEvidence, StaticMeshBakeInstanceEvidence,
    bake_static_mesh_transforms,
};
pub use transition_family::{
    CollectionTransitionFamilyMemberV1, CollectionTransitionFamilyV1,
    DocumentTransitionFamilyMemberV1, DocumentTransitionFamilyV1, TRANSITION_FAMILY_V1_ID,
    TRANSITION_FAMILY_V1_MAX_AGGREGATE_MEMBERS, TRANSITION_FAMILY_V1_MAX_DEPTH,
    TRANSITION_FAMILY_V1_MAX_DOCUMENT_FAMILY_ID_BYTES, TRANSITION_FAMILY_V1_MAX_FAMILIES,
    TRANSITION_FAMILY_V1_MAX_MEMBERS_PER_FAMILY, TRANSITION_FAMILY_V1_MAX_NORMALIZED_BYTES,
    TRANSITION_FAMILY_V1_MAX_SOURCE_BYTES, TRANSITION_FAMILY_V1_MAX_STRING_BYTES,
    TRANSITION_FAMILY_V1_SCHEMA_VERSION, TransitionFamilyBasisV1, TransitionFamilyBoundaryV1,
    TransitionFamilyDeclarationInputV1, TransitionFamilyDeclarationV1, TransitionFamilyError,
    TransitionFamilyManifestIdentityV1, TransitionFamilyTolerancesV1,
};
pub use transition_pose_evaluation::{
    CollectionTransitionPoseMemberInputV1, SkeletonBasisBoneV1, SkeletonBasisError,
    SkeletonBasisV1, TRANSITION_POSE_EVALUATION_V1_ID,
    TRANSITION_POSE_EVALUATION_V1_MAX_AGGREGATE_COMPARISONS,
    TRANSITION_POSE_EVALUATION_V1_MAX_AGGREGATE_OFFENDERS,
    TRANSITION_POSE_EVALUATION_V1_MAX_AGGREGATE_PAIR_BOUNDARIES,
    TRANSITION_POSE_EVALUATION_V1_MAX_BASIS_TEXT_BYTES, TRANSITION_POSE_EVALUATION_V1_MAX_BONES,
    TRANSITION_POSE_EVALUATION_V1_MAX_DOCUMENT_CLIPS,
    TRANSITION_POSE_EVALUATION_V1_MAX_FAMILY_PAIR_BOUNDARIES,
    TRANSITION_POSE_EVALUATION_V1_MAX_RAW_TRACK_ROWS_PER_CLIP,
    TRANSITION_POSE_EVALUATION_V1_MAX_RESULT_BYTES,
    TRANSITION_POSE_EVALUATION_V1_MAX_ROTATION_OFFENDERS,
    TRANSITION_POSE_EVALUATION_V1_MAX_SELECTED_TRACK_ELEMENTS,
    TRANSITION_POSE_EVALUATION_V1_MAX_SELECTED_TRACKS_PER_CLIP,
    TRANSITION_POSE_EVALUATION_V1_MAX_TRANSLATION_OFFENDERS,
    TRANSITION_POSE_EVALUATION_V1_SCHEMA_VERSION, TransitionPoseDecisionV1,
    TransitionPoseEvaluationControlError, TransitionPoseEvaluationV1,
    TransitionPoseFamilyEvaluationV1, TransitionPoseMemberV1, TransitionPosePairEvaluationV1,
    TransitionPoseReasonV1, TransitionPoseRotationOffenderV1, TransitionPoseStatusV1,
    TransitionPoseTranslationOffenderV1, evaluate_collection_transition_poses_v1,
    evaluate_document_transition_poses_v1,
};
