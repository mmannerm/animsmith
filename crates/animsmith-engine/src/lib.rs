//! Strict, immutable engine-import profile facts and deterministic settings
//! resolution for AnimSmith.
//!
//! The V1 registry has exactly five version-pinned profiles. [`resolve_static`]
//! validates a profile tuple and every declared setting before asset I/O;
//! [`StaticResolution::resolve_input`] then checks the authoritative
//! [`animsmith_core::SourceFormatV1`] and materializes per-clip settings.
//! Unknown engine behavior remains [`FactState::Unknown`]. This crate performs
//! no filesystem access, parses no TOML, and imports no format crate. Its
//! one-way [`project_prediction_provenance_v1`] adapter publishes the already
//! resolved profile/settings and same-load core source evidence. The borrowed
//! [`EngineAddressabilityCheck`] uses that evidence for the single frozen Bevy
//! 0.19.0 source-animation index-selector prediction; other engine behavior
//! remains outside this crate unless a version-pinned rule is added.
//! [`BevyAnimationAssetLabelV1`] is the one bounded selector formatter used by
//! that check, while [`build_bevy_animation_addressability_adapter_v1`] runs
//! the same check lifecycle once and packages its unchanged evaluation for the
//! standalone glTF animation-addressability contract.
//! [`EngineImportAdviceV1`] independently projects exact materialized Unity
//! importer settings beside same-load provenance and bounded clip evidence;
//! profiles without modeled settings produce a typed refusal rather than a
//! guessed preset.
//!
//! # Example
//!
//! ```
//! use animsmith_core::SourceFormatV1;
//! use animsmith_engine::{
//!     BakeOrExtract, EngineDeclaration, ProfileSelection, SettingValue,
//!     resolve_static,
//! };
//!
//! let mut declaration = EngineDeclaration {
//!     selection: Some(ProfileSelection::new(
//!         "unity-generic", 1, "6000.3", "fbx-model-importer",
//!     )),
//!     document_settings: Some(Default::default()),
//!     ..EngineDeclaration::default()
//! };
//! let document = declaration.document_settings.as_mut().unwrap();
//! document.insert("convert_units".into(), SettingValue::Boolean(true));
//! document.insert("bake_axis_conversion".into(), SettingValue::Boolean(true));
//! document.insert(
//!     "root_motion_source".into(),
//!     SettingValue::SourceTransformPath("Reference/Root".into()),
//! );
//! declaration.clip_settings.insert(
//!     "walk_*".into(),
//!     [
//!         ("root_rotation".into(), SettingValue::BakeOrExtract(BakeOrExtract::Extract)),
//!         ("root_position_y".into(), SettingValue::BakeOrExtract(BakeOrExtract::Bake)),
//!         ("root_position_xz".into(), SettingValue::BakeOrExtract(BakeOrExtract::Extract)),
//!     ].into(),
//! );
//! let static_profile = resolve_static(declaration)?.unwrap();
//! let resolved = static_profile.resolve_input(SourceFormatV1::Fbx, &["walk_forward".into()])?;
//! assert_eq!(resolved.clip_settings().len(), 1);
//! # Ok::<(), animsmith_engine::ResolutionError>(())
//! ```
//!
//! The crate has no public feature flags and supports the workspace MSRV,
//! Rust 1.88. Its Rust API is pre-1.0.

#![warn(missing_docs)]

mod addressability;
mod canonical;
mod clip_boundary;
mod error;
mod import_advice;
mod prediction;
mod provenance;
mod registry;
mod resolver;
mod root_motion;
mod track_support;
mod types;
mod unit_scale;

pub use canonical::{project_engine_profile_v2, project_resolved_engine_settings_v3};

pub use addressability::{
    GLTF_ANIMATION_ADDRESSABILITY_COMMAND, GLTF_ANIMATION_ADDRESSABILITY_SCHEMA_VERSION,
    GLTF_ANIMATION_ADDRESSABILITY_V1_ID, GLTF_ANIMATION_ADDRESSABILITY_V1_MAX_REPORT_BYTES,
    GltfAnimationAddressabilityAnimationSetV1, GltfAnimationAddressabilityAnimationV1,
    GltfAnimationAddressabilityBevyAdapterV1, GltfAnimationAddressabilityBevyReadbackV1,
    GltfAnimationAddressabilityChannelSetV1, GltfAnimationAddressabilityChannelV1,
    GltfAnimationAddressabilityCheckReadbackV1, GltfAnimationAddressabilityError,
    GltfAnimationAddressabilityIdentityV1, GltfAnimationAddressabilityInput,
    GltfAnimationAddressabilityInventoryV1, GltfAnimationAddressabilityReadError,
    GltfAnimationAddressabilityReadbackV1, GltfAnimationAddressabilityToolReadbackV1,
    GltfAnimationAddressabilityV1, GltfAnimationChannelPropertyV1, GltfAnimationCoverageStateV1,
    GltfAnimationCoverageV1, GltfAnimationObservationV1, GltfAnimationTargetKindV1,
    GltfAnimationTargetV1, GltfAnimationUnavailableReasonV1,
};
pub use clip_boundary::{ENGINE_CLIP_BOUNDARY_CHECK_ID, EngineClipBoundaryCheck};
pub use error::{
    InvalidSettingReason, PredictionRuleError, RegistryValidationError, ResolutionError,
    SettingLocation,
};
pub use import_advice::{
    ENGINE_IMPORT_ADVICE_COMMAND, ENGINE_IMPORT_ADVICE_SCHEMA_VERSION, ENGINE_IMPORT_ADVICE_V1_ID,
    ENGINE_IMPORT_ADVICE_V1_MAX_REPORT_BYTES, ENGINE_IMPORT_ADVICE_V2_ID,
    ENGINE_IMPORT_ADVICE_V2_MAX_REPORT_BYTES, ENGINE_IMPORT_ADVICE_V2_SCHEMA_VERSION,
    EngineImportAdviceClipEvidenceV1, EngineImportAdviceClipV1, EngineImportAdviceError,
    EngineImportAdviceIdentityV1, EngineImportAdviceIdentityV2, EngineImportAdviceInput,
    EngineImportAdviceInputV2, EngineImportAdviceMovementOwnerV1, EngineImportAdvicePayloadV1,
    EngineImportAdviceProjectionFieldV2, EngineImportAdviceProjectionV2,
    EngineImportAdviceProjectionValueV2, EngineImportAdviceReadError, EngineImportAdviceReadbackV1,
    EngineImportAdviceReadbackV2, EngineImportAdviceRefusalReasonV1,
    EngineImportAdviceRefusalReasonV2, EngineImportAdviceSourceNameV1,
    EngineImportAdviceSourceUnavailableReasonV1, EngineImportAdviceStateV1,
    EngineImportAdviceStateV2, EngineImportAdviceToolReadbackV1, EngineImportAdviceV1,
    EngineImportAdviceV2, UnityClipAdviceV1, UnityDocumentAdviceV1,
};
pub use prediction::{
    BevyAnimationAssetLabelError, BevyAnimationAssetLabelV1, ENGINE_ADDRESSABILITY_CHECK_ID,
    ENGINE_CHECK_IDS_V1, ENGINE_CHECK_IDS_V2, EngineAddressabilityCheck,
    EngineAddressabilityCheckV2, EngineAddressabilityCheckV3,
    GltfAnimationAddressabilityAdapterError, build_bevy_animation_addressability_adapter_v1,
};
pub use provenance::{
    PredictionProvenanceProjectionError, project_prediction_provenance_v1,
    project_prediction_provenance_v2, project_prediction_provenance_v3,
    project_prediction_provenance_v4, project_prediction_provenance_v5,
    project_prediction_provenance_v6,
};
pub use registry::{
    RegistryValidationErrorV2, profiles_v1, profiles_v2, validate_registry_v1, validate_registry_v2,
};
pub use resolver::{
    RESOLVED_ENGINE_SETTINGS_V2_MAX_CLIPS, ResolutionErrorV2, ResolvedClipCoverageReasonV2,
    ResolvedClipCoverageV2, ResolvedClipSettings, ResolvedClipSettingsV2,
    ResolvedEngineSettingsWorkV2, ResolvedProfile, ResolvedProfileSettingsV2, ResolvedProfileV2,
    SOURCE_TRANSFORM_PATH_MAX_BYTES, StaticResolution, StaticResolutionV2, lookup_profile,
    lookup_profile_v2, resolve_static, resolve_static_v2,
};
pub use root_motion::{ENGINE_ROOT_MOTION_CHECK_ID, EngineRootMotionCheck};
pub use track_support::{ENGINE_TRACK_SUPPORT_CHECK_ID, EngineTrackSupportCheck};
pub use types::{
    AnimationAddressability, BakeOrExtract, BevyGltfHandlerEnvironmentV2, BevyLoadMeshesStateV2,
    ConversionControl, CoordinateBasis, DefaultStatus, EngineDeclaration, EngineDeclarationV2,
    EngineProfile, EngineProfileV2, FactId, FactState, FactValue, ForwardAxis, Handedness,
    ImportHandling, LinearUnit, PrimarySource, PrimarySourceV2, ProfileFact, ProfileSelection,
    ResolvedSettingOriginV2, ResolvedSettingV2, RootMotionAddressability, SettingApplicability,
    SettingDefaultV2, SettingDescriptor, SettingDescriptorV2, SettingDomain, SettingDomainV2,
    SettingId, SettingIdV2, SettingMap, SettingMapV2, SettingScope, SettingValue, SettingValueV2,
    TargetAddressability, UnityAnimationTypeV2, UnityAvatarSetupV2, UnrealSampleRateV2, UpAxis,
};
pub use unit_scale::{ENGINE_UNIT_SCALE_CHECK_ID, EngineUnitScaleCheck};

/// Runtime kind of one closed [`SettingValue`] variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SettingValueKind {
    /// Boolean value.
    Boolean,
    /// Bake-or-extract value.
    BakeOrExtract,
    /// Source-transform path value.
    SourceTransformPath,
}
