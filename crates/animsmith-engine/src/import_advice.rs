//! Bounded, versioned engine-import advice for one loaded source document.
//!
//! V1 projects only settings already materialized by the frozen engine
//! profile. It never reconstructs authored frame coordinates, predicts
//! importer output, or turns measurements into undeclared project policy.

use crate::{
    BakeOrExtract, EngineProfile, ResolvedClipSettings, ResolvedProfile, SettingId, SettingValue,
    project_prediction_provenance_v1,
};
use animsmith_core::measure::{
    ClipMeasurements, FrameGridMeasurement, LoopEndpointMode, MeasurementAvailability,
};
use animsmith_core::{
    Config, InputIdentity, LoadedSource, MetricGrids, MovementOwner, PredictionProvenanceV1,
    RawSourceSetCoverageStateV1, SourceObservationStateV1, SourceSetCoverageStateV1, SourceTextV1,
    SourceUnavailableReasonV1, ToolInfo, resolve_configured_roles,
};
use serde::de::{Error as _, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::value::RawValue;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io::Read;
use std::marker::PhantomData;

/// Immutable standalone engine-import-advice contract identity.
pub const ENGINE_IMPORT_ADVICE_V1_ID: &str = "urn:animsmith:schema:engine-import-advice:1";
/// Standalone engine-import-advice schema version.
pub const ENGINE_IMPORT_ADVICE_SCHEMA_VERSION: u32 = 1;
/// Stable command spelling carried by the standalone contract.
pub const ENGINE_IMPORT_ADVICE_COMMAND: &str = "generate-import-advice";
/// Maximum serialized bytes accepted by the strict V1 reader.
pub const ENGINE_IMPORT_ADVICE_V1_MAX_REPORT_BYTES: u64 = 256 * 1024 * 1024;

const MAX_CLIPS: usize = animsmith_core::RAW_SOURCE_V1_MAX_CLIPS;
const MAX_TEXT_BYTES: usize = animsmith_core::RAW_SOURCE_V1_MAX_TEXT_BYTES;
const IDENTITY_DOMAIN: &str = "urn:animsmith:engine-import-advice-preimage:1";
const UNITY_GENERIC_FACTS: (&str, u64) = (
    "97afc05a02f7f9a946c66945cb84669a8a67d4dae7bf642486b94f1de3a17dd4",
    3097,
);
const UNITY_HUMANOID_FACTS: (&str, u64) = (
    "43f53df9f26ca3a1248972566029609bcd6b63194cbca399789444622680a12a",
    2847,
);
const UNREAL_FACTS: (&str, u64) = (
    "e44ca461aee46312b8265446f08338b988b96abeab0f8f502f560da5f1cdf759",
    2169,
);
const GODOT_FACTS: (&str, u64) = (
    "e9c8316d1655c487b60dd35bbfc70289952c5fa12f4718f0be09c7e9a00fbe87",
    1166,
);

/// Domain-separated canonical identity of one import-advice payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EngineImportAdviceIdentityV1(InputIdentity);

impl EngineImportAdviceIdentityV1 {
    /// SHA-256 plus canonical-preimage byte count.
    pub const fn input_identity(&self) -> &InputIdentity {
        &self.0
    }
}

/// Whether requested advice could be emitted without guessing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineImportAdviceStateV1 {
    /// Every V1 suggestion applicable to this profile was projected.
    Available,
    /// Advice was quarantined because required authority was unavailable.
    Refused,
}

/// Exact reason why V1 emitted no clip suggestions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineImportAdviceRefusalReasonV1 {
    /// This profile revision exposes no V1 setting vocabulary.
    ProfileSettingsUnmodeled,
    /// The raw source clip/take inventory was not exhaustive.
    RawClipInventoryIncomplete,
    /// A raw row lacked its normalized document index.
    ClipIdentityUnavailable,
    /// Raw rows, normalized clips, measurements, or settings disagreed.
    ClipIdentityMismatch,
    /// A required normalized measurement row was missing or malformed.
    MeasurementUnavailable,
}

/// Stable raw-source reason retained for a source-name observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineImportAdviceSourceUnavailableReasonV1 {
    /// The source declaration was malformed.
    Malformed,
    /// The loader discarded the value.
    Discarded,
    /// Normalization removed the original value.
    NormalizedAway,
    /// Baking removed the original value.
    BakedAway,
    /// The loader does not model the source domain.
    LoaderUnsupported,
    /// The bounded projection stopped at its deterministic limit.
    ProjectionBudgetExceeded,
    /// The parser did not expose the value.
    ParserUnavailable,
}

/// Bounded source animation/take name evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum EngineImportAdviceSourceNameV1 {
    /// The source name was observed exactly.
    Observed {
        /// Bounded source spelling.
        value: String,
    },
    /// Complete evidence proved that no source name exists.
    ProvenAbsent,
    /// The source name could not be established.
    Unavailable {
        /// Stable source-evidence reason.
        reason: EngineImportAdviceSourceUnavailableReasonV1,
    },
}

/// Project-declared owner of one world-movement component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineImportAdviceMovementOwnerV1 {
    /// Gameplay/controller motion owns the component.
    Gameplay,
    /// Extracted animation root motion owns the component.
    Animation,
}

/// Exact engine-neutral evidence retained beside one suggestion row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineImportAdviceClipEvidenceV1 {
    duration_s: f64,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_present_non_null"
    )]
    speed_mps: Option<f64>,
    speed_mps_availability: MeasurementAvailability,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_present_non_null"
    )]
    loop_endpoint_mode: Option<LoopEndpointMode>,
    loop_endpoint_mode_availability: MeasurementAvailability,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_present_non_null"
    )]
    frame_grid: Option<FrameGridMeasurement>,
    frame_grid_availability: MeasurementAvailability,
    #[serde(
        rename = "loop",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_present_non_null"
    )]
    looping: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_present_non_null"
    )]
    movement_owner_xz: Option<EngineImportAdviceMovementOwnerV1>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_present_non_null"
    )]
    movement_owner_y: Option<EngineImportAdviceMovementOwnerV1>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_present_non_null"
    )]
    movement_owner_yaw: Option<EngineImportAdviceMovementOwnerV1>,
}

impl EngineImportAdviceClipEvidenceV1 {
    /// Measured normalized clip duration in seconds.
    pub const fn duration_s(&self) -> f64 {
        self.duration_s
    }
    /// Measured horizontal speed, when available.
    pub const fn speed_mps(&self) -> Option<f64> {
        self.speed_mps
    }
    /// Availability of [`Self::speed_mps`].
    pub const fn speed_mps_availability(&self) -> MeasurementAvailability {
        self.speed_mps_availability
    }
    /// Measured endpoint convention for an explicitly declared loop.
    pub const fn loop_endpoint_mode(&self) -> Option<LoopEndpointMode> {
        self.loop_endpoint_mode
    }
    /// Availability of [`Self::loop_endpoint_mode`].
    pub const fn loop_endpoint_mode_availability(&self) -> MeasurementAvailability {
        self.loop_endpoint_mode_availability
    }
    /// Validated declared-FPS grid, when measured.
    pub const fn frame_grid(&self) -> Option<FrameGridMeasurement> {
        self.frame_grid
    }
    /// Availability of [`Self::frame_grid`].
    pub const fn frame_grid_availability(&self) -> MeasurementAvailability {
        self.frame_grid_availability
    }
    /// Explicit project loop intent.
    pub const fn looping(&self) -> Option<bool> {
        self.looping
    }
    /// Explicit horizontal movement owner.
    pub const fn movement_owner_xz(&self) -> Option<EngineImportAdviceMovementOwnerV1> {
        self.movement_owner_xz
    }
    /// Explicit vertical movement owner.
    pub const fn movement_owner_y(&self) -> Option<EngineImportAdviceMovementOwnerV1> {
        self.movement_owner_y
    }
    /// Explicit yaw movement owner.
    pub const fn movement_owner_yaw(&self) -> Option<EngineImportAdviceMovementOwnerV1> {
        self.movement_owner_yaw
    }

    fn validate(&self) -> Result<(), EngineImportAdviceError> {
        if !self.duration_s.is_finite()
            || self.duration_s < 0.0
            || self
                .speed_mps
                .is_some_and(|value| !value.is_finite() || value < 0.0)
        {
            return Err(EngineImportAdviceError::InvalidMeasurement);
        }
        validate_optional(self.speed_mps.is_some(), self.speed_mps_availability)?;
        validate_optional(
            self.loop_endpoint_mode.is_some(),
            self.loop_endpoint_mode_availability,
        )?;
        let loop_endpoint_applicability_matches = match self.looping {
            Some(true) => {
                self.loop_endpoint_mode_availability != MeasurementAvailability::NotApplicable
            }
            Some(false) | None => {
                self.loop_endpoint_mode_availability == MeasurementAvailability::NotApplicable
            }
        };
        if !loop_endpoint_applicability_matches {
            return Err(EngineImportAdviceError::InvalidMeasurement);
        }
        validate_optional(self.frame_grid.is_some(), self.frame_grid_availability)?;
        if self.frame_grid.is_some_and(|grid| {
            !grid.fps.is_finite() || grid.fps <= 0.0 || grid.frame_intervals == 0
        }) {
            return Err(EngineImportAdviceError::InvalidMeasurement);
        }
        Ok(())
    }
}

/// One file-scoped normalized clip and its source/intent/measurement evidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineImportAdviceClipV1 {
    source_clip_index: u64,
    normalized_clip_index: u64,
    normalized_clip_name: String,
    source_name: EngineImportAdviceSourceNameV1,
    evidence: EngineImportAdviceClipEvidenceV1,
}

impl EngineImportAdviceClipV1 {
    /// Stable source animation/take index.
    pub const fn source_clip_index(&self) -> u64 {
        self.source_clip_index
    }
    /// Exact normalized document clip index.
    pub const fn normalized_clip_index(&self) -> u64 {
        self.normalized_clip_index
    }
    /// Normalized document clip name.
    pub fn normalized_clip_name(&self) -> &str {
        &self.normalized_clip_name
    }
    /// Source name observation, distinct from the normalized name.
    pub const fn source_name(&self) -> &EngineImportAdviceSourceNameV1 {
        &self.source_name
    }
    /// Project-intent and measurement evidence.
    pub const fn evidence(&self) -> &EngineImportAdviceClipEvidenceV1 {
        &self.evidence
    }

    fn validate(&self, ordinal: usize) -> Result<(), EngineImportAdviceError> {
        if self.source_clip_index != ordinal as u64
            || self.normalized_clip_index >= MAX_CLIPS as u64
            || self.normalized_clip_name.is_empty()
            || self.normalized_clip_name.len() > MAX_TEXT_BYTES
        {
            return Err(EngineImportAdviceError::InvalidClipIdentity);
        }
        if let EngineImportAdviceSourceNameV1::Observed { value } = &self.source_name
            && (value.is_empty() || value.len() > MAX_TEXT_BYTES)
        {
            return Err(EngineImportAdviceError::InvalidClipIdentity);
        }
        self.evidence.validate()
    }
}

/// Exact Unity document-level importer suggestions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnityDocumentAdviceV1 {
    convert_units: bool,
    bake_axis_conversion: bool,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_present_non_null"
    )]
    root_motion_source: Option<String>,
}

impl UnityDocumentAdviceV1 {
    /// Unity Model Importer Convert Units value.
    pub const fn convert_units(&self) -> bool {
        self.convert_units
    }
    /// Unity Model Importer Bake Axis Conversion value.
    pub const fn bake_axis_conversion(&self) -> bool {
        self.bake_axis_conversion
    }
    /// Unity Generic exact motion-node path, absent for Humanoid.
    pub fn root_motion_source(&self) -> Option<&str> {
        self.root_motion_source.as_deref()
    }
}

/// Exact Unity per-clip importer suggestions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnityClipAdviceV1 {
    normalized_clip_index: u64,
    lock_root_rotation: bool,
    lock_root_height_y: bool,
    lock_root_position_xz: bool,
}

impl UnityClipAdviceV1 {
    /// Normalized clip index linked to the root evidence row.
    pub const fn normalized_clip_index(&self) -> u64 {
        self.normalized_clip_index
    }
    /// Unity `ModelImporterClipAnimation.lockRootRotation`.
    pub const fn lock_root_rotation(&self) -> bool {
        self.lock_root_rotation
    }
    /// Unity `ModelImporterClipAnimation.lockRootHeightY`.
    pub const fn lock_root_height_y(&self) -> bool {
        self.lock_root_height_y
    }
    /// Unity `ModelImporterClipAnimation.lockRootPositionXZ`.
    pub const fn lock_root_position_xz(&self) -> bool {
        self.lock_root_position_xz
    }
}

/// Profile-specific V1 advice payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "engine", rename_all = "kebab-case", deny_unknown_fields)]
pub enum EngineImportAdvicePayloadV1 {
    /// Unity Generic 6000.3 materialized settings.
    UnityGeneric {
        /// Document-level importer suggestions.
        document: UnityDocumentAdviceV1,
        /// Per-clip suggestions in normalized-index order.
        #[serde(deserialize_with = "deserialize_unity_clips")]
        clips: Vec<UnityClipAdviceV1>,
    },
    /// Unity Humanoid 6000.3 materialized settings.
    UnityHumanoid {
        /// Document-level importer suggestions.
        document: UnityDocumentAdviceV1,
        /// Per-clip suggestions in normalized-index order.
        #[serde(deserialize_with = "deserialize_unity_clips")]
        clips: Vec<UnityClipAdviceV1>,
    },
    /// Unreal 5.8 revision 1 has no modeled advice settings.
    Unreal,
    /// Godot 4.7 revision 1 has no modeled advice settings.
    Godot,
}

impl EngineImportAdvicePayloadV1 {
    /// Unity document suggestions, absent for unmodeled profiles.
    pub const fn unity_document(&self) -> Option<&UnityDocumentAdviceV1> {
        match self {
            Self::UnityGeneric { document, .. } | Self::UnityHumanoid { document, .. } => {
                Some(document)
            }
            Self::Unreal | Self::Godot => None,
        }
    }
    /// Unity clip suggestions, empty for unmodeled profiles.
    pub fn unity_clips(&self) -> &[UnityClipAdviceV1] {
        match self {
            Self::UnityGeneric { clips, .. } | Self::UnityHumanoid { clips, .. } => clips,
            Self::Unreal | Self::Godot => &[],
        }
    }
}

/// Standalone producer envelope for `generate import-advice`.
#[derive(Debug, Clone, Serialize)]
pub struct EngineImportAdviceV1 {
    schema_version: u32,
    schema: &'static str,
    tool: ToolInfo,
    command: &'static str,
    identity: EngineImportAdviceIdentityV1,
    prediction_provenance: PredictionProvenanceV1,
    state: EngineImportAdviceStateV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    refusal_reason: Option<EngineImportAdviceRefusalReasonV1>,
    clips: Vec<EngineImportAdviceClipV1>,
    payload: EngineImportAdvicePayloadV1,
}

/// Import-advice producer or strict-reader contract failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum EngineImportAdviceError {
    /// The selected profile is outside Unity/Unreal/Godot V1 advice.
    #[error("generate import-advice requires an exact Unity, Unreal, or Godot V1 profile")]
    UnsupportedProfile,
    /// Engine-neutral project intent was not a valid core configuration.
    #[error("invalid engine-import-advice configuration: {0}")]
    InvalidConfig(String),
    /// The standalone header was not the immutable V1 shape.
    #[error("invalid engine-import-advice V1 header")]
    WrongHeader,
    /// Producer metadata violated the shared tool shape.
    #[error("invalid engine-import-advice producer identity")]
    InvalidTool,
    /// Shared prediction provenance was invalid.
    #[error("invalid prediction provenance: {0}")]
    InvalidProvenance(String),
    /// Exact profile tuple/facts did not match the payload variant.
    #[error("engine-import-advice profile does not match its payload")]
    ProfileMismatch,
    /// Advice state, reason, or row presence contradicted the lifecycle.
    #[error("invalid engine-import-advice lifecycle")]
    InvalidLifecycle,
    /// A source/normalized clip identity was malformed or duplicated.
    #[error("invalid engine-import-advice clip identity")]
    InvalidClipIdentity,
    /// A retained measurement value/status pair was malformed.
    #[error("invalid engine-import-advice measurement")]
    InvalidMeasurement,
    /// Materialized Unity settings did not match the advice projection.
    #[error("engine-import-advice Unity settings do not match prediction provenance")]
    UnitySettingsMismatch,
    /// Too many clip rows were present.
    #[error("engine-import-advice has {found} clips; limit is {limit}")]
    TooManyClips {
        /// First rejected row count.
        found: usize,
        /// Immutable V1 limit.
        limit: usize,
    },
    /// Canonical advice identity did not match the payload.
    #[error("engine-import-advice identity mismatch")]
    IdentityMismatch,
}

/// Bounded-reader transport/shape failure.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum EngineImportAdviceReadError {
    /// Reading serialized bytes failed.
    #[error("cannot read engine-import-advice document: {source}")]
    Io {
        /// Underlying read failure.
        #[source]
        source: std::io::Error,
    },
    /// Serialized input exceeded the fixed pre-decode bound.
    #[error("engine-import-advice document exceeds {limit} bytes")]
    ReportTooLarge {
        /// Immutable V1 byte limit.
        limit: u64,
    },
    /// JSON shape or field vocabulary was invalid.
    #[error("invalid engine-import-advice JSON: {source}")]
    InvalidJson {
        /// Strict JSON/serde failure.
        #[source]
        source: serde_json::Error,
    },
}

impl EngineImportAdviceV1 {
    /// Whether an immutable profile record has an exact V1 advice mapping.
    ///
    /// This static check requires no asset I/O. It is intentionally false for
    /// Bevy and for any future profile revision until that exact record gains
    /// a separately reviewed advice mapping.
    pub fn supports_profile(profile: &EngineProfile) -> bool {
        let selection = profile.selection();
        let facts = profile.facts_identity();
        [
            (
                "unity-generic",
                1,
                "6000.3",
                "fbx-model-importer",
                UNITY_GENERIC_FACTS,
            ),
            (
                "unity-humanoid",
                1,
                "6000.3",
                "fbx-model-importer",
                UNITY_HUMANOID_FACTS,
            ),
            ("unreal", 1, "5.8", "fbx-importer", UNREAL_FACTS),
            ("godot", 1, "4.7", "resource-importer-scene", GODOT_FACTS),
        ]
        .into_iter()
        .any(|(family, revision, version, importer, identity)| {
            selection.family() == family
                && selection.profile_revision() == revision
                && selection.engine_version() == version
                && selection.importer() == importer
                && facts.sha256() == identity.0
                && facts.bytes() == identity.1
        })
    }

    /// Construct one bounded result from same-load source, profile, intent,
    /// and measurement evidence. Measurements and configured rig roles are
    /// derived inside this boundary from `source`, so a caller cannot attach
    /// a map computed from another normalized document.
    ///
    /// Exact Unreal and Godot V1 profiles produce a typed refusal because
    /// those immutable revisions expose no setting vocabulary.
    ///
    /// # Errors
    ///
    /// Returns [`EngineImportAdviceError`] for an unsupported profile,
    /// invalid same-load provenance, malformed materialized Unity settings,
    /// or a contract invariant.
    pub fn from_source(
        tool: ToolInfo,
        source: &LoadedSource,
        profile: &ResolvedProfile,
        config: &Config,
    ) -> Result<Self, EngineImportAdviceError> {
        config
            .validate()
            .map_err(|error| EngineImportAdviceError::InvalidConfig(error.to_string()))?;
        if !Self::supports_profile(profile.profile()) {
            return Err(EngineImportAdviceError::UnsupportedProfile);
        }
        let prediction_provenance = project_prediction_provenance_v1(profile, source)
            .map_err(|error| EngineImportAdviceError::InvalidProvenance(error.to_string()))?;
        let family = profile.profile().selection().family();
        let (payload, state, refusal_reason, clips) = match family {
            "unreal" => (
                EngineImportAdvicePayloadV1::Unreal,
                EngineImportAdviceStateV1::Refused,
                Some(EngineImportAdviceRefusalReasonV1::ProfileSettingsUnmodeled),
                Vec::new(),
            ),
            "godot" => (
                EngineImportAdvicePayloadV1::Godot,
                EngineImportAdviceStateV1::Refused,
                Some(EngineImportAdviceRefusalReasonV1::ProfileSettingsUnmodeled),
                Vec::new(),
            ),
            "unity-generic" | "unity-humanoid" => {
                let generic = family == "unity-generic";
                let document = unity_document(profile, generic)?;
                let roles = resolve_configured_roles(&source.document().skeleton, &config.rig);
                let grids = MetricGrids::new(source.document());
                let measurements =
                    animsmith_core::measure::measure_document(&grids, &roles, config);
                match build_clip_evidence(source, &measurements, config) {
                    Ok(clips) => match unity_clip_advice(profile, source.document(), &clips) {
                        Ok(unity_clips) => {
                            let payload = if generic {
                                EngineImportAdvicePayloadV1::UnityGeneric {
                                    document,
                                    clips: unity_clips,
                                }
                            } else {
                                EngineImportAdvicePayloadV1::UnityHumanoid {
                                    document,
                                    clips: unity_clips,
                                }
                            };
                            (payload, EngineImportAdviceStateV1::Available, None, clips)
                        }
                        Err(EngineImportAdviceError::UnitySettingsMismatch) => {
                            let payload = if generic {
                                EngineImportAdvicePayloadV1::UnityGeneric {
                                    document,
                                    clips: Vec::new(),
                                }
                            } else {
                                EngineImportAdvicePayloadV1::UnityHumanoid {
                                    document,
                                    clips: Vec::new(),
                                }
                            };
                            (
                                payload,
                                EngineImportAdviceStateV1::Refused,
                                Some(EngineImportAdviceRefusalReasonV1::ClipIdentityMismatch),
                                Vec::new(),
                            )
                        }
                        Err(error) => return Err(error),
                    },
                    Err(reason) => {
                        let payload = if generic {
                            EngineImportAdvicePayloadV1::UnityGeneric {
                                document,
                                clips: Vec::new(),
                            }
                        } else {
                            EngineImportAdvicePayloadV1::UnityHumanoid {
                                document,
                                clips: Vec::new(),
                            }
                        };
                        (
                            payload,
                            EngineImportAdviceStateV1::Refused,
                            Some(reason),
                            Vec::new(),
                        )
                    }
                }
            }
            _ => return Err(EngineImportAdviceError::UnsupportedProfile),
        };
        let mut report = Self {
            schema_version: ENGINE_IMPORT_ADVICE_SCHEMA_VERSION,
            schema: ENGINE_IMPORT_ADVICE_V1_ID,
            tool,
            command: ENGINE_IMPORT_ADVICE_COMMAND,
            identity: EngineImportAdviceIdentityV1(InputIdentity::from_bytes(&[])),
            prediction_provenance,
            state,
            refusal_reason,
            clips,
            payload,
        };
        report.validate_without_identity()?;
        report.identity = EngineImportAdviceIdentityV1(report.computed_identity());
        Ok(report)
    }

    /// Canonical advice identity.
    pub const fn identity(&self) -> &EngineImportAdviceIdentityV1 {
        &self.identity
    }
    /// Exact shared profile/settings/source/closure provenance.
    pub const fn prediction_provenance(&self) -> &PredictionProvenanceV1 {
        &self.prediction_provenance
    }
    /// Advice availability state.
    pub const fn state(&self) -> EngineImportAdviceStateV1 {
        self.state
    }
    /// Refusal reason, present exactly when state is refused.
    pub const fn refusal_reason(&self) -> Option<EngineImportAdviceRefusalReasonV1> {
        self.refusal_reason
    }
    /// File-scoped clip evidence, empty for a refused result.
    pub fn clips(&self) -> &[EngineImportAdviceClipV1] {
        &self.clips
    }
    /// Profile-specific setting projection or unmodeled payload.
    pub const fn payload(&self) -> &EngineImportAdvicePayloadV1 {
        &self.payload
    }

    fn validate_without_identity(&self) -> Result<(), EngineImportAdviceError> {
        if self.schema_version != ENGINE_IMPORT_ADVICE_SCHEMA_VERSION
            || self.schema != ENGINE_IMPORT_ADVICE_V1_ID
            || self.command != ENGINE_IMPORT_ADVICE_COMMAND
        {
            return Err(EngineImportAdviceError::WrongHeader);
        }
        self.prediction_provenance
            .validate()
            .map_err(|error| EngineImportAdviceError::InvalidProvenance(error.to_string()))?;
        validate_profile(&self.prediction_provenance, &self.payload)?;
        if self.clips.len() > MAX_CLIPS {
            return Err(EngineImportAdviceError::TooManyClips {
                found: self.clips.len(),
                limit: MAX_CLIPS,
            });
        }
        let mut normalized = BTreeSet::new();
        for (ordinal, clip) in self.clips.iter().enumerate() {
            clip.validate(ordinal)?;
            if !normalized.insert(clip.normalized_clip_index) {
                return Err(EngineImportAdviceError::InvalidClipIdentity);
            }
        }
        if normalized
            .iter()
            .copied()
            .ne((0..self.clips.len()).map(|index| index as u64))
        {
            return Err(EngineImportAdviceError::InvalidClipIdentity);
        }
        let raw_clips_complete = self
            .prediction_provenance
            .raw_source()
            .clips_coverage()
            .state()
            == RawSourceSetCoverageStateV1::Complete;
        match (self.state, self.refusal_reason) {
            (EngineImportAdviceStateV1::Available, None) => {
                if !raw_clips_complete
                    || matches!(
                        self.payload,
                        EngineImportAdvicePayloadV1::Unreal | EngineImportAdvicePayloadV1::Godot
                    )
                    || self.payload.unity_clips().len() != self.clips.len()
                {
                    return Err(EngineImportAdviceError::InvalidLifecycle);
                }
                for (normalized_index, advice) in self.payload.unity_clips().iter().enumerate() {
                    if advice.normalized_clip_index != normalized_index as u64 {
                        return Err(EngineImportAdviceError::InvalidClipIdentity);
                    }
                }
                validate_unity_settings_against_provenance(
                    &self.prediction_provenance,
                    &self.clips,
                    &self.payload,
                )?;
            }
            (EngineImportAdviceStateV1::Refused, Some(reason)) => {
                if !self.clips.is_empty() || !self.payload.unity_clips().is_empty() {
                    return Err(EngineImportAdviceError::InvalidLifecycle);
                }
                let unmodeled = matches!(
                    self.payload,
                    EngineImportAdvicePayloadV1::Unreal | EngineImportAdvicePayloadV1::Godot
                );
                if (reason == EngineImportAdviceRefusalReasonV1::ProfileSettingsUnmodeled)
                    != unmodeled
                {
                    return Err(EngineImportAdviceError::InvalidLifecycle);
                }
                if !unmodeled {
                    let coverage_matches_reason = match reason {
                        EngineImportAdviceRefusalReasonV1::RawClipInventoryIncomplete => {
                            !raw_clips_complete
                        }
                        EngineImportAdviceRefusalReasonV1::ClipIdentityUnavailable
                        | EngineImportAdviceRefusalReasonV1::ClipIdentityMismatch
                        | EngineImportAdviceRefusalReasonV1::MeasurementUnavailable => {
                            raw_clips_complete
                        }
                        EngineImportAdviceRefusalReasonV1::ProfileSettingsUnmodeled => false,
                    };
                    if !coverage_matches_reason {
                        return Err(EngineImportAdviceError::InvalidLifecycle);
                    }
                    validate_unity_document_against_provenance(
                        &self.prediction_provenance,
                        &self.payload,
                    )?;
                }
            }
            _ => return Err(EngineImportAdviceError::InvalidLifecycle),
        }
        Ok(())
    }

    fn computed_identity(&self) -> InputIdentity {
        let mut encoder = AdviceEncoder::new(IDENTITY_DOMAIN);
        encode_identity(
            &mut encoder,
            self.prediction_provenance.identity().input_identity(),
        );
        encoder.token(match self.state {
            EngineImportAdviceStateV1::Available => "available",
            EngineImportAdviceStateV1::Refused => "refused",
        });
        encoder.bool(self.refusal_reason.is_some());
        if let Some(reason) = self.refusal_reason {
            encoder.token(refusal_reason_name(reason));
        }
        encoder.count(self.clips.len());
        for clip in &self.clips {
            encode_clip(&mut encoder, clip);
        }
        encode_payload(&mut encoder, &self.payload);
        InputIdentity::from_bytes(&encoder.bytes)
    }
}

fn build_clip_evidence(
    source: &LoadedSource,
    measurements: &BTreeMap<String, ClipMeasurements>,
    config: &Config,
) -> Result<Vec<EngineImportAdviceClipV1>, EngineImportAdviceRefusalReasonV1> {
    let raw = source.source_facts().clips();
    if raw.coverage().state() != SourceSetCoverageStateV1::Complete {
        return Err(EngineImportAdviceRefusalReasonV1::RawClipInventoryIncomplete);
    }
    let document = source.document();
    if raw.rows().len() != document.clips.len() || measurements.len() != document.clips.len() {
        return Err(EngineImportAdviceRefusalReasonV1::ClipIdentityMismatch);
    }
    let mut normalized = BTreeSet::new();
    let mut rows = Vec::with_capacity(raw.rows().len());
    for (ordinal, raw_clip) in raw.rows().iter().enumerate() {
        if raw_clip.source_clip_index() != ordinal {
            return Err(EngineImportAdviceRefusalReasonV1::ClipIdentityMismatch);
        }
        let normalized_index = match raw_clip.normalized_clip_index().state() {
            SourceObservationStateV1::Observed(value) => *value,
            SourceObservationStateV1::ProvenAbsent | SourceObservationStateV1::Unavailable(_) => {
                return Err(EngineImportAdviceRefusalReasonV1::ClipIdentityUnavailable);
            }
        };
        if normalized_index >= document.clips.len() || !normalized.insert(normalized_index) {
            return Err(EngineImportAdviceRefusalReasonV1::ClipIdentityMismatch);
        }
        let clip = &document.clips[normalized_index];
        let Some(measurement) = measurements.get(&clip.name) else {
            return Err(EngineImportAdviceRefusalReasonV1::MeasurementUnavailable);
        };
        let expectations = config.expectations_for(&clip.name);
        let evidence = EngineImportAdviceClipEvidenceV1 {
            duration_s: measurement.duration_s,
            speed_mps: measurement.speed_mps,
            speed_mps_availability: measurement.speed_mps_availability,
            loop_endpoint_mode: measurement.loop_endpoint_mode,
            loop_endpoint_mode_availability: measurement.loop_endpoint_mode_availability,
            frame_grid: measurement.frame_grid,
            frame_grid_availability: measurement.frame_grid_availability,
            looping: expectations.looping,
            movement_owner_xz: expectations.movement_owner_xz.map(movement_owner),
            movement_owner_y: expectations.movement_owner_y.map(movement_owner),
            movement_owner_yaw: expectations.movement_owner_yaw.map(movement_owner),
        };
        if evidence.validate().is_err() {
            return Err(EngineImportAdviceRefusalReasonV1::MeasurementUnavailable);
        }
        rows.push(EngineImportAdviceClipV1 {
            source_clip_index: ordinal as u64,
            normalized_clip_index: normalized_index as u64,
            normalized_clip_name: clip.name.clone(),
            source_name: source_name(raw_clip.source_name().state()),
            evidence,
        });
    }
    if normalized.len() != document.clips.len() {
        return Err(EngineImportAdviceRefusalReasonV1::ClipIdentityMismatch);
    }
    Ok(rows)
}

fn unity_document(
    profile: &ResolvedProfile,
    generic: bool,
) -> Result<UnityDocumentAdviceV1, EngineImportAdviceError> {
    let convert_units = bool_setting(profile.document_settings().get(&SettingId::ConvertUnits))?;
    let bake_axis_conversion = bool_setting(
        profile
            .document_settings()
            .get(&SettingId::BakeAxisConversion),
    )?;
    let root_motion_source = if generic {
        match profile
            .document_settings()
            .get(&SettingId::RootMotionSource)
        {
            Some(SettingValue::SourceTransformPath(value)) => Some(value.clone()),
            _ => return Err(EngineImportAdviceError::UnitySettingsMismatch),
        }
    } else {
        None
    };
    Ok(UnityDocumentAdviceV1 {
        convert_units,
        bake_axis_conversion,
        root_motion_source,
    })
}

fn unity_clip_advice(
    profile: &ResolvedProfile,
    document: &animsmith_core::Document,
    clips: &[EngineImportAdviceClipV1],
) -> Result<Vec<UnityClipAdviceV1>, EngineImportAdviceError> {
    let settings = settings_by_document_index(profile.clip_settings(), document)?;
    let mut advice = clips
        .iter()
        .map(|clip| {
            let row = settings
                .get(clip.normalized_clip_index as usize)
                .ok_or(EngineImportAdviceError::UnitySettingsMismatch)?;
            Ok(UnityClipAdviceV1 {
                normalized_clip_index: clip.normalized_clip_index,
                lock_root_rotation: bake_setting(row.settings().get(&SettingId::RootRotation))?,
                lock_root_height_y: bake_setting(row.settings().get(&SettingId::RootPositionY))?,
                lock_root_position_xz: bake_setting(
                    row.settings().get(&SettingId::RootPositionXz),
                )?,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    advice.sort_by_key(|clip| clip.normalized_clip_index);
    Ok(advice)
}

fn settings_by_document_index<'a>(
    settings: &'a [ResolvedClipSettings],
    document: &animsmith_core::Document,
) -> Result<Vec<&'a ResolvedClipSettings>, EngineImportAdviceError> {
    if settings.len() != document.clips.len() {
        return Err(EngineImportAdviceError::UnitySettingsMismatch);
    }
    let mut used = vec![false; settings.len()];
    document
        .clips
        .iter()
        .map(|clip| {
            let Some((index, row)) = settings
                .iter()
                .enumerate()
                .find(|(index, row)| !used[*index] && row.clip_name() == clip.name)
            else {
                return Err(EngineImportAdviceError::UnitySettingsMismatch);
            };
            used[index] = true;
            Ok(row)
        })
        .collect()
}

fn bool_setting(value: Option<&SettingValue>) -> Result<bool, EngineImportAdviceError> {
    match value {
        Some(SettingValue::Boolean(value)) => Ok(*value),
        _ => Err(EngineImportAdviceError::UnitySettingsMismatch),
    }
}

fn bake_setting(value: Option<&SettingValue>) -> Result<bool, EngineImportAdviceError> {
    match value {
        Some(SettingValue::BakeOrExtract(BakeOrExtract::Bake)) => Ok(true),
        Some(SettingValue::BakeOrExtract(BakeOrExtract::Extract)) => Ok(false),
        _ => Err(EngineImportAdviceError::UnitySettingsMismatch),
    }
}

fn movement_owner(value: MovementOwner) -> EngineImportAdviceMovementOwnerV1 {
    match value {
        MovementOwner::Gameplay => EngineImportAdviceMovementOwnerV1::Gameplay,
        MovementOwner::Animation => EngineImportAdviceMovementOwnerV1::Animation,
    }
}

fn source_name(state: &SourceObservationStateV1<SourceTextV1>) -> EngineImportAdviceSourceNameV1 {
    match state {
        SourceObservationStateV1::Observed(value) => EngineImportAdviceSourceNameV1::Observed {
            value: value.as_str().to_owned(),
        },
        SourceObservationStateV1::ProvenAbsent => EngineImportAdviceSourceNameV1::ProvenAbsent,
        SourceObservationStateV1::Unavailable(reason) => {
            EngineImportAdviceSourceNameV1::Unavailable {
                reason: source_reason(*reason),
            }
        }
    }
}

fn source_reason(reason: SourceUnavailableReasonV1) -> EngineImportAdviceSourceUnavailableReasonV1 {
    match reason {
        SourceUnavailableReasonV1::Malformed => {
            EngineImportAdviceSourceUnavailableReasonV1::Malformed
        }
        SourceUnavailableReasonV1::Discarded => {
            EngineImportAdviceSourceUnavailableReasonV1::Discarded
        }
        SourceUnavailableReasonV1::NormalizedAway => {
            EngineImportAdviceSourceUnavailableReasonV1::NormalizedAway
        }
        SourceUnavailableReasonV1::BakedAway => {
            EngineImportAdviceSourceUnavailableReasonV1::BakedAway
        }
        SourceUnavailableReasonV1::LoaderUnsupported => {
            EngineImportAdviceSourceUnavailableReasonV1::LoaderUnsupported
        }
        SourceUnavailableReasonV1::ProjectionBudgetExceeded => {
            EngineImportAdviceSourceUnavailableReasonV1::ProjectionBudgetExceeded
        }
        SourceUnavailableReasonV1::ParserUnavailable => {
            EngineImportAdviceSourceUnavailableReasonV1::ParserUnavailable
        }
    }
}

fn validate_optional(
    present: bool,
    availability: MeasurementAvailability,
) -> Result<(), EngineImportAdviceError> {
    if matches!(availability, MeasurementAvailability::Measured) != present {
        return Err(EngineImportAdviceError::InvalidMeasurement);
    }
    Ok(())
}

fn validate_unity_document_against_provenance(
    provenance: &PredictionProvenanceV1,
    payload: &EngineImportAdvicePayloadV1,
) -> Result<(), EngineImportAdviceError> {
    use animsmith_core::{EngineSettingIdV1 as Id, EngineSettingValueV1 as Value};
    let Some(document) = payload.unity_document() else {
        return Err(EngineImportAdviceError::UnitySettingsMismatch);
    };
    let settings = provenance.settings();
    if settings.document_setting(Id::ConvertUnits) != Some(&Value::Boolean(document.convert_units))
        || settings.document_setting(Id::BakeAxisConversion)
            != Some(&Value::Boolean(document.bake_axis_conversion))
    {
        return Err(EngineImportAdviceError::UnitySettingsMismatch);
    }
    let generic = matches!(payload, EngineImportAdvicePayloadV1::UnityGeneric { .. });
    match (
        generic,
        document.root_motion_source.as_ref(),
        settings.document_setting(Id::RootMotionSource),
    ) {
        (true, Some(expected), Some(Value::SourceTransformPath(actual))) if expected == actual => {
            Ok(())
        }
        (false, None, None) => Ok(()),
        _ => Err(EngineImportAdviceError::UnitySettingsMismatch),
    }
}

fn validate_unity_settings_against_provenance(
    provenance: &PredictionProvenanceV1,
    clips: &[EngineImportAdviceClipV1],
    payload: &EngineImportAdvicePayloadV1,
) -> Result<(), EngineImportAdviceError> {
    use animsmith_core::{
        EngineBakeOrExtractV1 as Bake, EngineSettingIdV1 as Id, EngineSettingValueV1 as Value,
    };
    validate_unity_document_against_provenance(provenance, payload)?;
    let settings = provenance.settings();
    if settings.clips().len() != clips.len() {
        return Err(EngineImportAdviceError::UnitySettingsMismatch);
    }
    let mut used = vec![false; settings.clips().len()];
    for advice in payload.unity_clips() {
        let Some(clip) = clips
            .iter()
            .find(|clip| clip.normalized_clip_index == advice.normalized_clip_index)
        else {
            return Err(EngineImportAdviceError::UnitySettingsMismatch);
        };
        let Some((index, row)) = settings
            .clips()
            .iter()
            .enumerate()
            .find(|(index, row)| !used[*index] && row.clip_name() == clip.normalized_clip_name)
        else {
            return Err(EngineImportAdviceError::UnitySettingsMismatch);
        };
        used[index] = true;
        for (id, baked) in [
            (Id::RootRotation, advice.lock_root_rotation),
            (Id::RootPositionY, advice.lock_root_height_y),
            (Id::RootPositionXz, advice.lock_root_position_xz),
        ] {
            let expected = Value::BakeOrExtract(if baked { Bake::Bake } else { Bake::Extract });
            if row.setting(id) != Some(&expected) {
                return Err(EngineImportAdviceError::UnitySettingsMismatch);
            }
        }
    }
    Ok(())
}

fn validate_profile(
    provenance: &PredictionProvenanceV1,
    payload: &EngineImportAdvicePayloadV1,
) -> Result<(), EngineImportAdviceError> {
    let selection = provenance.profile().selection();
    let facts = provenance.profile().facts_identity();
    let (family, revision, version, importer, identity) = match payload {
        EngineImportAdvicePayloadV1::UnityGeneric { .. } => (
            "unity-generic",
            1,
            "6000.3",
            "fbx-model-importer",
            UNITY_GENERIC_FACTS,
        ),
        EngineImportAdvicePayloadV1::UnityHumanoid { .. } => (
            "unity-humanoid",
            1,
            "6000.3",
            "fbx-model-importer",
            UNITY_HUMANOID_FACTS,
        ),
        EngineImportAdvicePayloadV1::Unreal => ("unreal", 1, "5.8", "fbx-importer", UNREAL_FACTS),
        EngineImportAdvicePayloadV1::Godot => {
            ("godot", 1, "4.7", "resource-importer-scene", GODOT_FACTS)
        }
    };
    if selection.family() != family
        || selection.profile_revision() != revision
        || selection.engine_version() != version
        || selection.importer() != importer
        || facts.sha256() != identity.0
        || facts.bytes() != identity.1
    {
        return Err(EngineImportAdviceError::ProfileMismatch);
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolSourceInputV1 {
    #[serde(default, deserialize_with = "deserialize_required_nullable")]
    revision: RequiredNullable<String>,
    #[serde(default, deserialize_with = "deserialize_required_nullable")]
    dirty: RequiredNullable<bool>,
}

#[derive(Debug, Default)]
enum RequiredNullable<T> {
    #[default]
    Missing,
    Present(Option<T>),
}

fn deserialize_required_nullable<'de, D, T>(
    deserializer: D,
) -> Result<RequiredNullable<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(RequiredNullable::Present)
}

fn deserialize_present_non_null<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolInputV1 {
    name: String,
    version: String,
    source: ToolSourceInputV1,
}

/// Staged bounded reader input for the standalone advice contract.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineImportAdviceInput {
    schema_version: u32,
    schema: String,
    tool: ToolInputV1,
    command: String,
    identity: EngineImportAdviceIdentityV1,
    prediction_provenance: Box<RawValue>,
    state: EngineImportAdviceStateV1,
    #[serde(default, deserialize_with = "deserialize_present_non_null")]
    refusal_reason: Option<EngineImportAdviceRefusalReasonV1>,
    #[serde(deserialize_with = "deserialize_clips")]
    clips: Vec<EngineImportAdviceClipV1>,
    payload: EngineImportAdvicePayloadV1,
}

impl EngineImportAdviceInput {
    /// Read at most 256 MiB before any JSON decoding.
    ///
    /// # Errors
    ///
    /// Returns a typed I/O, size, or JSON-shape error.
    pub fn read_from(reader: impl Read) -> Result<Self, EngineImportAdviceReadError> {
        Self::read_from_with_limit(reader, ENGINE_IMPORT_ADVICE_V1_MAX_REPORT_BYTES)
    }

    fn read_from_with_limit(
        reader: impl Read,
        limit: u64,
    ) -> Result<Self, EngineImportAdviceReadError> {
        let mut bytes = Vec::new();
        reader
            .take(limit.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|source| EngineImportAdviceReadError::Io { source })?;
        if bytes.len() as u64 > limit {
            return Err(EngineImportAdviceReadError::ReportTooLarge { limit });
        }
        serde_json::from_slice(&bytes)
            .map_err(|source| EngineImportAdviceReadError::InvalidJson { source })
    }

    /// Validate identities, lifecycle, exact profile, and setting cross-links.
    ///
    /// # Errors
    ///
    /// Returns [`EngineImportAdviceError`] for a semantic contradiction.
    pub fn into_report(self) -> Result<EngineImportAdviceReadbackV1, EngineImportAdviceError> {
        if self.schema_version != ENGINE_IMPORT_ADVICE_SCHEMA_VERSION
            || self.schema != ENGINE_IMPORT_ADVICE_V1_ID
            || self.command != ENGINE_IMPORT_ADVICE_COMMAND
        {
            return Err(EngineImportAdviceError::WrongHeader);
        }
        validate_tool(&self.tool)?;
        let prediction_provenance: PredictionProvenanceV1 =
            serde_json::from_str(self.prediction_provenance.get())
                .map_err(|error| EngineImportAdviceError::InvalidProvenance(error.to_string()))?;
        let report = EngineImportAdviceReadbackV1 {
            tool: EngineImportAdviceToolReadbackV1 {
                name: self.tool.name,
                version: self.tool.version,
                revision: required_nullable_value(self.tool.source.revision),
                dirty: required_nullable_value(self.tool.source.dirty),
            },
            identity: self.identity,
            prediction_provenance,
            state: self.state,
            refusal_reason: self.refusal_reason,
            clips: self.clips,
            payload: self.payload,
        };
        report.validate()?;
        Ok(report)
    }
}

/// Strict read-side representation of one import-advice document.
#[derive(Debug, Clone)]
pub struct EngineImportAdviceReadbackV1 {
    tool: EngineImportAdviceToolReadbackV1,
    identity: EngineImportAdviceIdentityV1,
    prediction_provenance: PredictionProvenanceV1,
    state: EngineImportAdviceStateV1,
    refusal_reason: Option<EngineImportAdviceRefusalReasonV1>,
    clips: Vec<EngineImportAdviceClipV1>,
    payload: EngineImportAdvicePayloadV1,
}

impl EngineImportAdviceReadbackV1 {
    /// Validated producer identity.
    pub const fn tool(&self) -> &EngineImportAdviceToolReadbackV1 {
        &self.tool
    }
    /// Canonical advice identity.
    pub const fn identity(&self) -> &EngineImportAdviceIdentityV1 {
        &self.identity
    }
    /// Shared exact provenance.
    pub const fn prediction_provenance(&self) -> &PredictionProvenanceV1 {
        &self.prediction_provenance
    }
    /// Advice state.
    pub const fn state(&self) -> EngineImportAdviceStateV1 {
        self.state
    }
    /// Typed refusal reason.
    pub const fn refusal_reason(&self) -> Option<EngineImportAdviceRefusalReasonV1> {
        self.refusal_reason
    }
    /// Validated clip evidence.
    pub fn clips(&self) -> &[EngineImportAdviceClipV1] {
        &self.clips
    }
    /// Validated profile-specific payload.
    pub const fn payload(&self) -> &EngineImportAdvicePayloadV1 {
        &self.payload
    }

    fn validate(&self) -> Result<(), EngineImportAdviceError> {
        let surrogate = EngineImportAdviceV1 {
            schema_version: ENGINE_IMPORT_ADVICE_SCHEMA_VERSION,
            schema: ENGINE_IMPORT_ADVICE_V1_ID,
            tool: ToolInfo::animsmith(animsmith_core::ToolSource::new(None, None)),
            command: ENGINE_IMPORT_ADVICE_COMMAND,
            identity: self.identity.clone(),
            prediction_provenance: self.prediction_provenance.clone(),
            state: self.state,
            refusal_reason: self.refusal_reason,
            clips: self.clips.clone(),
            payload: self.payload.clone(),
        };
        surrogate.validate_without_identity()?;
        if surrogate.computed_identity() != self.identity.0 {
            return Err(EngineImportAdviceError::IdentityMismatch);
        }
        Ok(())
    }
}

/// Validated producer metadata from strict readback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineImportAdviceToolReadbackV1 {
    name: String,
    version: String,
    revision: Option<String>,
    dirty: Option<bool>,
}

impl EngineImportAdviceToolReadbackV1 {
    /// Producer name.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Producer semantic version.
    pub fn version(&self) -> &str {
        &self.version
    }
    /// Full source revision, when established.
    pub fn revision(&self) -> Option<&str> {
        self.revision.as_deref()
    }
    /// Dirty-worktree observation, when established.
    pub const fn dirty(&self) -> Option<bool> {
        self.dirty
    }
}

struct BoundedSequenceVisitor<T> {
    element: PhantomData<fn() -> T>,
}

impl<'de, T> Visitor<'de> for BoundedSequenceVisitor<T>
where
    T: Deserialize<'de>,
{
    type Value = Vec<T>;
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "a sequence with at most {MAX_CLIPS} clip rows")
    }
    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::with_capacity(sequence.size_hint().unwrap_or(0).min(MAX_CLIPS));
        while values.len() < MAX_CLIPS {
            let Some(value) = sequence.next_element()? else {
                return Ok(values);
            };
            values.push(value);
        }
        if sequence.next_element::<serde::de::IgnoredAny>()?.is_some() {
            return Err(A::Error::custom(EngineImportAdviceError::TooManyClips {
                found: MAX_CLIPS + 1,
                limit: MAX_CLIPS,
            }));
        }
        Ok(values)
    }
}

fn deserialize_bounded_clips<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    deserializer.deserialize_seq(BoundedSequenceVisitor {
        element: PhantomData,
    })
}
fn deserialize_clips<'de, D>(deserializer: D) -> Result<Vec<EngineImportAdviceClipV1>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_clips(deserializer)
}
fn deserialize_unity_clips<'de, D>(deserializer: D) -> Result<Vec<UnityClipAdviceV1>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_clips(deserializer)
}

fn validate_tool(tool: &ToolInputV1) -> Result<(), EngineImportAdviceError> {
    if tool.name != "animsmith"
        || tool.name.len() > MAX_TEXT_BYTES
        || tool.version.len() > MAX_TEXT_BYTES
        || !is_schema_semver(&tool.version)
        || matches!(tool.source.revision, RequiredNullable::Missing)
        || matches!(tool.source.dirty, RequiredNullable::Missing)
        || matches!(
            &tool.source.revision,
            RequiredNullable::Present(Some(revision))
                if revision.len() != 40
                    || !revision.bytes().all(|byte| byte.is_ascii_hexdigit())
        )
    {
        return Err(EngineImportAdviceError::InvalidTool);
    }
    Ok(())
}

fn required_nullable_value<T>(value: RequiredNullable<T>) -> Option<T> {
    match value {
        RequiredNullable::Present(value) => value,
        RequiredNullable::Missing => unreachable!("validated required nullable field"),
    }
}

fn is_schema_semver(value: &str) -> bool {
    let without_build = match value.split_once('+') {
        Some((core, build))
            if !build.is_empty()
                && !build.contains('+')
                && build
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-')) =>
        {
            core
        }
        Some(_) => return false,
        None => value,
    };
    let core = match without_build.split_once('-') {
        Some((core, pre))
            if !pre.is_empty()
                && pre
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-')) =>
        {
            core
        }
        Some(_) => return false,
        None => without_build,
    };
    let parts = core.split('.').collect::<Vec<_>>();
    parts.len() == 3
        && parts.iter().all(|part| {
            !part.is_empty()
                && part.bytes().all(|byte| byte.is_ascii_digit())
                && (*part == "0" || !part.starts_with('0'))
        })
}

struct AdviceEncoder {
    bytes: Vec<u8>,
}
impl AdviceEncoder {
    fn new(domain: &str) -> Self {
        let mut value = Self { bytes: Vec::new() };
        value.token(domain);
        value
    }
    fn token(&mut self, value: &str) {
        self.bytes
            .extend_from_slice(&(value.len() as u64).to_le_bytes());
        self.bytes.extend_from_slice(value.as_bytes());
    }
    fn count(&mut self, value: usize) {
        self.u64(value as u64);
    }
    fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }
    fn bool(&mut self, value: bool) {
        self.bytes.push(u8::from(value));
    }
    fn f64(&mut self, value: f64) {
        self.bytes.extend_from_slice(&value.to_bits().to_le_bytes());
    }
}

fn encode_identity(encoder: &mut AdviceEncoder, value: &InputIdentity) {
    encoder.token(value.sha256());
    encoder.u64(value.bytes());
}
fn encode_string_option(encoder: &mut AdviceEncoder, value: Option<&str>) {
    encoder.bool(value.is_some());
    if let Some(value) = value {
        encoder.token(value);
    }
}
fn encode_clip(encoder: &mut AdviceEncoder, clip: &EngineImportAdviceClipV1) {
    encoder.u64(clip.source_clip_index);
    encoder.u64(clip.normalized_clip_index);
    encoder.token(&clip.normalized_clip_name);
    match &clip.source_name {
        EngineImportAdviceSourceNameV1::Observed { value } => {
            encoder.token("observed");
            encoder.token(value);
        }
        EngineImportAdviceSourceNameV1::ProvenAbsent => encoder.token("proven_absent"),
        EngineImportAdviceSourceNameV1::Unavailable { reason } => {
            encoder.token("unavailable");
            encoder.token(source_reason_name(*reason));
        }
    }
    let evidence = &clip.evidence;
    encoder.f64(evidence.duration_s);
    encoder.bool(evidence.speed_mps.is_some());
    if let Some(value) = evidence.speed_mps {
        encoder.f64(value);
    }
    encoder.token(availability_name(evidence.speed_mps_availability));
    encoder.bool(evidence.loop_endpoint_mode.is_some());
    if let Some(value) = evidence.loop_endpoint_mode {
        encoder.token(value.as_str());
    }
    encoder.token(availability_name(evidence.loop_endpoint_mode_availability));
    encoder.bool(evidence.frame_grid.is_some());
    if let Some(value) = evidence.frame_grid {
        encoder.f64(value.fps);
        encoder.u64(u64::from(value.frame_intervals));
    }
    encoder.token(availability_name(evidence.frame_grid_availability));
    encoder.bool(evidence.looping.is_some());
    if let Some(value) = evidence.looping {
        encoder.bool(value);
    }
    for owner in [
        evidence.movement_owner_xz,
        evidence.movement_owner_y,
        evidence.movement_owner_yaw,
    ] {
        encoder.bool(owner.is_some());
        if let Some(owner) = owner {
            encoder.token(owner_name(owner));
        }
    }
}

fn encode_payload(encoder: &mut AdviceEncoder, payload: &EngineImportAdvicePayloadV1) {
    match payload {
        EngineImportAdvicePayloadV1::UnityGeneric { document, clips } => {
            encoder.token("unity-generic");
            encode_unity_payload(encoder, document, clips);
        }
        EngineImportAdvicePayloadV1::UnityHumanoid { document, clips } => {
            encoder.token("unity-humanoid");
            encode_unity_payload(encoder, document, clips);
        }
        EngineImportAdvicePayloadV1::Unreal => encoder.token("unreal"),
        EngineImportAdvicePayloadV1::Godot => encoder.token("godot"),
    }
}
fn encode_unity_payload(
    encoder: &mut AdviceEncoder,
    document: &UnityDocumentAdviceV1,
    clips: &[UnityClipAdviceV1],
) {
    encoder.bool(document.convert_units);
    encoder.bool(document.bake_axis_conversion);
    encode_string_option(encoder, document.root_motion_source.as_deref());
    encoder.count(clips.len());
    for clip in clips {
        encoder.u64(clip.normalized_clip_index);
        encoder.bool(clip.lock_root_rotation);
        encoder.bool(clip.lock_root_height_y);
        encoder.bool(clip.lock_root_position_xz);
    }
}

fn refusal_reason_name(reason: EngineImportAdviceRefusalReasonV1) -> &'static str {
    match reason {
        EngineImportAdviceRefusalReasonV1::ProfileSettingsUnmodeled => "profile_settings_unmodeled",
        EngineImportAdviceRefusalReasonV1::RawClipInventoryIncomplete => {
            "raw_clip_inventory_incomplete"
        }
        EngineImportAdviceRefusalReasonV1::ClipIdentityUnavailable => "clip_identity_unavailable",
        EngineImportAdviceRefusalReasonV1::ClipIdentityMismatch => "clip_identity_mismatch",
        EngineImportAdviceRefusalReasonV1::MeasurementUnavailable => "measurement_unavailable",
    }
}
fn source_reason_name(reason: EngineImportAdviceSourceUnavailableReasonV1) -> &'static str {
    match reason {
        EngineImportAdviceSourceUnavailableReasonV1::Malformed => "malformed",
        EngineImportAdviceSourceUnavailableReasonV1::Discarded => "discarded",
        EngineImportAdviceSourceUnavailableReasonV1::NormalizedAway => "normalized_away",
        EngineImportAdviceSourceUnavailableReasonV1::BakedAway => "baked_away",
        EngineImportAdviceSourceUnavailableReasonV1::LoaderUnsupported => "loader_unsupported",
        EngineImportAdviceSourceUnavailableReasonV1::ProjectionBudgetExceeded => {
            "projection_budget_exceeded"
        }
        EngineImportAdviceSourceUnavailableReasonV1::ParserUnavailable => "parser_unavailable",
    }
}
fn availability_name(value: MeasurementAvailability) -> &'static str {
    match value {
        MeasurementAvailability::Measured => "measured",
        MeasurementAvailability::NotApplicable => "not_applicable",
        MeasurementAvailability::Unavailable => "unavailable",
        _ => "unknown",
    }
}
fn owner_name(value: EngineImportAdviceMovementOwnerV1) -> &'static str {
    match value {
        EngineImportAdviceMovementOwnerV1::Gameplay => "gameplay",
        EngineImportAdviceMovementOwnerV1::Animation => "animation",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn staged_reader_accepts_byte_n_and_rejects_n_plus_one_before_json_decode() {
        let exact = EngineImportAdviceInput::read_from_with_limit(Cursor::new(b"null"), 4);
        assert!(matches!(
            exact,
            Err(EngineImportAdviceReadError::InvalidJson { .. })
        ));
        let over = EngineImportAdviceInput::read_from_with_limit(Cursor::new(b"null "), 4);
        assert!(matches!(
            over,
            Err(EngineImportAdviceReadError::ReportTooLarge { limit: 4 })
        ));
    }
}
