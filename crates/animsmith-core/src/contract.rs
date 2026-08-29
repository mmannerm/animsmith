//! Versioned JSON result-contract types shared by CLI and embedded producers.
//!
//! The CLI is one producer of these envelopes. Embedded pipelines can use the
//! same constructors and immutable protocol identities without duplicating the
//! wire shape or hard-coding URNs.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::io::Read;

use glam::Mat4;
use serde::de::{DeserializeSeed, EnumAccess, MapAccess, SeqAccess, VariantAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::value::RawValue;
use sha2::{Digest, Sha256};

use crate::dependency_closure::DependencyClosureCoverageV1;
use crate::diff::MetricDelta;
use crate::engine_contract::{
    EngineFactIdV1, EngineFactStateV1, EngineFactValueV1, EngineSettingIdV2, EngineSettingValueV2,
};
use crate::evaluation::{
    Applicability, CheckEvaluation, CheckEvaluationGapRef, CheckEvaluationValidationInput,
    ConfigurationState, EvaluationScope, EvaluationState, SelectionState,
    validate_and_derive_check_evaluation,
};
use crate::measure::{
    Aabb, AdditionalInfluenceSetMeasurements, AssetMeasurements, ClipMeasurements,
    ImageMeasurements, LinearTransformClassification, LinearTransformMeasurements,
    MaterialDefinitionMeasurements, MeasurementAvailability, MeshDefinitionMeasurements,
    NodeInstanceMeasurements, PrimitiveMeasurements, SceneMeasurements,
    SkeletonNodeLocalRestMeasurements, SkeletonRestWorldMatrixUnavailableReason,
    SkinDerivedMatrixMeasurements, SkinDerivedMatrixUnavailableReason,
    StaticNodeAabbUnavailableReason, TextureMeasurements, assess_inverse_bind,
    measure_linear_transform, summarize_skin_bind_linear,
};
use crate::metrics::canonical_net_yaw_deg;
use crate::model::{
    DecodedImageColorType, MaterialResourceCoverage, SourceInverseBindAccessorStatus,
    SourceSkeletonCoverage,
};
use crate::prediction::{
    EngineMachineResultV1, EnginePredictionBasisV2, EnginePredictionBasisV4,
    EnginePredictionFacetStateV1, EnginePredictionV1, EnginePredictionV4, EnginePredictionV5,
    EnginePredictionV6, ExactSourceTimingBasisReferenceV1, ExactSourceTimingBindingV1,
    ExactSourceTimingDomainV1, ExactSourceTimingKeyV1, ExactSourceTimingObservationStateWireV1,
    PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE, PREDICTION_V1_MAX_FACETS_PER_FILE,
    PREDICTION_V1_MAX_TOTAL_TEXT_BYTES_PER_FILE, PREDICTION_V2_MAX_CANDIDATE_FACETS_PER_RULE,
    PredictionBasisReferenceV4, PredictionContractError, PredictionDecodeError,
    PredictionProvenanceV4, PredictionProvenanceV5, PredictionProvenanceV6, PredictionScalarV1,
    RawSceneAttachmentBasisDomainV1, RawSceneAttachmentBasisReferenceV1, RawSourceBasisReferenceV1,
    ResolvedSettingLocationV1, TransformScaleDomainV1, TransformScaleResultV1,
    TransformScaleSubjectKindV1, UnitMappingResultV1,
    decode_engine_prediction_v1_with_measurement_schema,
    decode_engine_prediction_v2_with_measurement_schema, decode_engine_prediction_v3,
    decode_engine_prediction_v4, decode_prediction_provenance_v1_with_measurement_schema,
    decode_prediction_provenance_v2_with_measurement_schema, decode_prediction_provenance_v3,
    decode_prediction_provenance_v4, validate_measurement_references_batch,
    validate_measurement_references_batch_v2, validate_measurement_references_batch_v3,
    validate_measurement_references_batch_v4,
};
use crate::profile::ResolvedRoles;
use crate::source_facts::SourceFormatV1;
use crate::{Document, Severity};
use crate::{
    EnginePredictionV2, EnginePredictionV3, ImporterSubjectCreationV1, InventoryCoverageResultV1,
    PredictionBasisReferenceV1, PredictionBasisReferenceV2, PredictionInventoryCoverageStateV1,
    PredictionInventoryDomainV1, PredictionProvenanceV1, PredictionProvenanceV2,
    PredictionProvenanceV3, PredictionUnavailableReasonV2, RawAnimationChannelInventoryV1,
    RawSceneAttachmentCoverageV1, RawSourceDomainV1, RawSourceFieldIdV1, RawSourceKeyV1,
    RawSourceSetCoverageStateV1, ResolvedEngineSettingsCoverageStateV2,
    SourceImportDispositionResultV1, SourceImportDispositionV1, SourceImportSubjectKindV1,
    SourceSkeletonRowKindV1,
};

/// Current outer result-envelope version.
pub const OUTPUT_SCHEMA_VERSION: u32 = 18;
/// Immutable identity of the current outer result envelope.
pub const OUTPUT_SCHEMA_ID: &str = "urn:animsmith:schema:output:18";
/// Immutable output-v17 identity retained for historical V6 prediction evidence.
pub const OUTPUT_V17_SCHEMA_ID: &str = "urn:animsmith:schema:output:17";
/// Schema version of output-v17.
pub const OUTPUT_V17_SCHEMA_VERSION: u32 = 17;
/// Immutable output-v16 identity retained for historical V5 prediction evidence.
pub const OUTPUT_V16_SCHEMA_ID: &str = "urn:animsmith:schema:output:16";
/// Schema version of output-v16.
pub const OUTPUT_V16_SCHEMA_VERSION: u32 = 16;
/// Immutable output-v15 identity retained for historical V4 prediction evidence.
pub const OUTPUT_V15_SCHEMA_ID: &str = "urn:animsmith:schema:output:15";
/// Schema version of output-v15.
pub const OUTPUT_V15_SCHEMA_VERSION: u32 = 15;
/// Immutable output-v10 identity retained by V1 dependent contracts.
pub const OUTPUT_V10_SCHEMA_ID: &str = "urn:animsmith:schema:output:10";
/// Immutable output-v11 identity retained as historical schema evidence.
pub const OUTPUT_V11_SCHEMA_ID: &str = "urn:animsmith:schema:output:11";
/// Immutable output-v11 version retained as historical schema evidence.
pub const OUTPUT_V11_SCHEMA_VERSION: u32 = 11;
/// Immutable identity of the bounded-overflow outer result envelope.
pub const OUTPUT_V12_SCHEMA_ID: &str = "urn:animsmith:schema:output:12";
/// Schema version of output-v12.
pub const OUTPUT_V12_SCHEMA_VERSION: u32 = 12;
/// Immutable output-v13 identity retained as historical V2 prediction evidence.
pub const OUTPUT_V13_SCHEMA_ID: &str = "urn:animsmith:schema:output:13";
/// Schema version of output-v13.
pub const OUTPUT_V13_SCHEMA_VERSION: u32 = 13;
/// Immutable output-v14 identity retained for historical V3 prediction evidence.
pub const OUTPUT_V14_SCHEMA_ID: &str = "urn:animsmith:schema:output:14";
/// Schema version of output-v14.
pub const OUTPUT_V14_SCHEMA_VERSION: u32 = 14;
/// Maximum serialized bytes accepted by the output-v11 report reader.
pub const OUTPUT_V11_MAX_REPORT_BYTES: u64 = 256 * 1024 * 1024;
/// Maximum file records carried by one output-v11 envelope.
pub const OUTPUT_V11_MAX_FILES: usize = 4_096;
/// Maximum check records carried by one output-v11 lint file.
pub const OUTPUT_V11_MAX_CHECKS_PER_FILE: usize = 4_096;
/// Current nested measurement-contract version.
pub const MEASUREMENTS_SCHEMA_VERSION: u32 = 17;
/// Immutable identity of the current nested measurement contract.
pub const MEASUREMENTS_SCHEMA_ID: &str = "urn:animsmith:schema:measurements:17";
/// Immutable measurements-v16 identity retained for output-v13 through output-v17 readers.
pub const MEASUREMENTS_V16_SCHEMA_ID: &str = "urn:animsmith:schema:measurements:16";
/// Immutable measurements-v16 version retained for historical report readers.
pub const MEASUREMENTS_V16_SCHEMA_VERSION: u32 = 16;
/// Immutable measurements-v15 identity retained for output-v11 and output-v12 readers.
pub const MEASUREMENTS_V15_SCHEMA_ID: &str = "urn:animsmith:schema:measurements:15";
/// Immutable measurements-v15 version retained for historical report readers.
pub const MEASUREMENTS_V15_SCHEMA_VERSION: u32 = 15;

/// Source checkout identity for the producing animsmith build.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolSource {
    revision: Option<String>,
    dirty: Option<bool>,
}

impl ToolSource {
    /// Construct source identity from a full Git revision and dirty bit.
    ///
    /// Packaged or otherwise provenance-free builds use `None` for fields they
    /// cannot establish rather than claiming a clean checkout. Revisions that
    /// are not full 40-character hexadecimal Git object ids are dropped so an
    /// envelope constructed through this API remains within output v11.
    pub fn new(revision: Option<String>, dirty: Option<bool>) -> Self {
        let revision = revision.filter(|revision| {
            revision.len() == 40 && revision.bytes().all(|byte| byte.is_ascii_hexdigit())
        });
        Self { revision, dirty }
    }
}

/// Identity of the animsmith producer that emitted an envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolInfo {
    name: &'static str,
    version: &'static str,
    source: ToolSource,
}

impl ToolInfo {
    /// Construct animsmith producer identity from this package's version and
    /// optional source-checkout metadata.
    pub fn animsmith(source: ToolSource) -> Self {
        Self {
            name: "animsmith",
            version: env!("CARGO_PKG_VERSION"),
            source,
        }
    }
}

/// Immutable identity of the bytes used to produce one file report.
///
/// The digest is lowercase hexadecimal SHA-256 so consumers can compare
/// identities without retaining the source bytes themselves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InputIdentity {
    sha256: String,
    bytes: u64,
}

/// Lowercase hexadecimal SHA-256 digest of exactly these bytes.
///
/// The digest type carries no `LowerHex` impl, so every producer of a
/// contract `sha256` field goes through this one formatter rather than
/// re-deriving the encoding.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    sha256_digest_hex(Sha256::digest(bytes).into())
}

fn sha256_digest_hex(digest: [u8; 32]) -> String {
    let mut hex = String::with_capacity(64);
    for byte in digest {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

impl InputIdentity {
    /// Calculate the identity for source bytes.
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            sha256: sha256_hex(bytes),
            bytes: bytes.len() as u64,
        }
    }

    /// Construct an identity from an already-computed SHA-256 digest and exact byte count.
    ///
    /// This keeps streaming and bounded readers on the same lowercase digest
    /// authority without exposing an unchecked string constructor.
    pub fn from_sha256_digest(digest: [u8; 32], bytes: u64) -> Self {
        Self {
            sha256: sha256_digest_hex(digest),
            bytes,
        }
    }

    /// Lowercase hexadecimal SHA-256 digest of the source bytes.
    pub fn sha256(&self) -> &str {
        &self.sha256
    }

    /// Number of source bytes represented by this identity.
    pub fn bytes(&self) -> u64 {
        self.bytes
    }
}

/// Rig profile and resolved semantic-role bindings for one input file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RigInfo {
    profile: String,
    resolution_outcome: String,
    resolved_roles: BTreeMap<String, String>,
    resolved_role_policies: BTreeMap<String, String>,
}

/// Resolved-role evidence did not belong to the supplied document.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum RigInfoError {
    /// A resolved role referenced a bone outside the document's skeleton.
    #[error(
        "resolved role {role:?} references bone {bone}, but the document has {bone_count} bones"
    )]
    InvalidBoneId {
        /// Stable semantic role name.
        role: &'static str,
        /// Invalid bone index carried by the resolution.
        bone: usize,
        /// Number of bones available in the supplied document.
        bone_count: usize,
    },
    /// A valid bone index now names a different bone than the resolution did.
    #[error(
        "resolved role {role:?} expected bone {bone} to be {expected:?}, but the document names it {found:?}"
    )]
    BoneNameMismatch {
        /// Stable semantic role name.
        role: &'static str,
        /// Bone index carried by the resolution.
        bone: usize,
        /// Bone name captured when the role was resolved.
        expected: String,
        /// Bone name at that index in the supplied document.
        found: String,
    },
}

impl RigInfo {
    /// Project resolved roles into their stable role names and source bone
    /// names for the result contract.
    ///
    /// # Errors
    ///
    /// Returns [`RigInfoError`] when `roles` references a bone outside the
    /// supplied document, such as a resolution produced from another
    /// skeleton.
    pub fn from_resolved(doc: &Document, roles: &ResolvedRoles) -> Result<Self, RigInfoError> {
        let resolved = roles
            .iter_with_details()
            .map(|(role, bone, expected_name, policy)| {
                let name = doc
                    .skeleton
                    .bones
                    .get(bone)
                    .ok_or(RigInfoError::InvalidBoneId {
                        role: role.as_str(),
                        bone,
                        bone_count: doc.skeleton.bones.len(),
                    })?;
                if name.name != expected_name {
                    return Err(RigInfoError::BoneNameMismatch {
                        role: role.as_str(),
                        bone,
                        expected: expected_name.to_owned(),
                        found: name.name.clone(),
                    });
                }
                Ok((role.as_str(), (name.name.clone(), policy.as_str())))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        Ok(Self {
            profile: roles.profile.clone(),
            resolution_outcome: roles.outcome().as_str().to_owned(),
            resolved_roles: resolved
                .iter()
                .map(|(&role, (name, _))| (role.to_owned(), name.clone()))
                .collect(),
            resolved_role_policies: resolved
                .into_iter()
                .map(|(role, (_, policy))| (role.to_owned(), policy.to_owned()))
                .collect(),
        })
    }
}

/// Independently versioned measurement payload nested in measure and lint
/// file records.
#[derive(Debug, Clone, Serialize)]
pub struct MeasurementContract {
    schema_version: u32,
    schema: &'static str,
    clips: BTreeMap<String, ClipMeasurements>,
    #[serde(flatten)]
    assets: AssetMeasurements,
}

/// Measurement evidence could not satisfy the current measurement contract.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum MeasurementContractError {
    /// A required or present numeric value was non-finite.
    #[error("measurement value {path} must be finite")]
    NonFiniteValue {
        /// Human-readable location within the measurement contract.
        path: String,
    },
    /// Related measurement fields were structurally inconsistent.
    #[error("measurement structure {path} is invalid: {reason}")]
    InvalidStructure {
        /// Human-readable location within the measurement contract.
        path: String,
        /// Stable explanation of the violated relationship.
        reason: String,
    },
}

impl MeasurementContract {
    /// Construct the current measurement contract.
    ///
    /// # Errors
    ///
    /// Returns [`MeasurementContractError`] when required or present numeric
    /// evidence is non-finite or structurally inconsistent.
    pub fn new(
        clips: BTreeMap<String, ClipMeasurements>,
        assets: AssetMeasurements,
    ) -> Result<Self, MeasurementContractError> {
        validate_measurements(&clips, &assets, MeasurementRevision::V17)?;
        Ok(Self {
            schema_version: MEASUREMENTS_SCHEMA_VERSION,
            schema: MEASUREMENTS_SCHEMA_ID,
            clips,
            assets,
        })
    }

    pub(crate) fn historical_v15(
        clips: BTreeMap<String, ClipMeasurements>,
        assets: AssetMeasurements,
    ) -> Result<Self, MeasurementContractError> {
        validate_measurements(&clips, &assets, MeasurementRevision::V15)?;
        Ok(Self {
            schema_version: MEASUREMENTS_V15_SCHEMA_VERSION,
            schema: MEASUREMENTS_V15_SCHEMA_ID,
            clips,
            assets,
        })
    }

    pub(crate) fn historical_v16(
        clips: BTreeMap<String, ClipMeasurements>,
        assets: AssetMeasurements,
    ) -> Result<Self, MeasurementContractError> {
        validate_measurements(&clips, &assets, MeasurementRevision::V16)?;
        Ok(Self {
            schema_version: MEASUREMENTS_V16_SCHEMA_VERSION,
            schema: MEASUREMENTS_V16_SCHEMA_ID,
            clips,
            assets,
        })
    }

    /// Reconstruct the measurements-v16 evidence view consumed by immutable
    /// prediction contracts embedded in current output. V16 represented loop
    /// continuity as all-or-nothing, so any unavailable V17 bone projects to
    /// an unavailable outer fact; fully measured rows simply omit the V17-only
    /// per-bone availability field.
    fn prediction_v16_projection(&self) -> Result<Self, MeasurementContractError> {
        let mut clips = self.clips.clone();
        for clip in clips.values_mut() {
            let has_unavailable_bone = clip.loop_continuity.as_ref().is_some_and(|continuity| {
                continuity
                    .bones
                    .iter()
                    .any(|bone| bone.availability != MeasurementAvailability::Measured)
            });
            if has_unavailable_bone {
                clip.loop_continuity = None;
                clip.loop_continuity_availability = MeasurementAvailability::Unavailable;
            } else if let Some(continuity) = &mut clip.loop_continuity {
                for bone in &mut continuity.bones {
                    bone.availability_was_present = false;
                }
            }
        }
        Self::historical_v16(clips, self.assets.clone())
    }

    /// Per-clip measurements keyed by clip name.
    pub fn clips(&self) -> &BTreeMap<String, ClipMeasurements> {
        &self.clips
    }

    /// Static source-geometry, node-instance, and declared-scene evidence.
    pub fn assets(&self) -> &AssetMeasurements {
        &self.assets
    }

    /// Consume the contract and return its clip and static asset measurements.
    pub fn into_parts(self) -> (BTreeMap<String, ClipMeasurements>, AssetMeasurements) {
        (self.clips, self.assets)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MeasurementRevision {
    V15,
    V16,
    V17,
}

fn validate_measurements(
    clips: &BTreeMap<String, ClipMeasurements>,
    assets: &AssetMeasurements,
    revision: MeasurementRevision,
) -> Result<(), MeasurementContractError> {
    let finite = |value: f64, path: String| {
        value
            .is_finite()
            .then_some(())
            .ok_or(MeasurementContractError::NonFiniteValue { path })
    };
    let permits_roundoff = |observed: f64, lower_bound: f64| {
        let tolerance = 1.0e-9 * observed.abs().max(lower_bound.abs()).max(1.0);
        observed + tolerance >= lower_bound
    };
    let check_availability = |value_present: bool,
                              availability: MeasurementAvailability,
                              path: String| {
        match (value_present, availability) {
            (true, MeasurementAvailability::Measured) => Ok(()),
            (
                false,
                MeasurementAvailability::NotApplicable | MeasurementAvailability::Unavailable,
            ) => Ok(()),
            _ => Err(MeasurementContractError::InvalidStructure {
                path,
                reason: "value presence must match availability status".into(),
            }),
        }
    };
    for (clip_name, clip) in clips {
        finite(clip.duration_s, format!("clips[{clip_name:?}].duration_s"))?;
        let mut previous_bone_index = None;
        let mut covered_bone_names = BTreeSet::new();
        for (offset, bone) in clip.bone_channels.iter().enumerate() {
            let path = format!("clips[{clip_name:?}].bone_channels[{offset}]");
            if previous_bone_index.is_some_and(|previous| previous >= bone.bone_index) {
                return Err(MeasurementContractError::InvalidStructure {
                    path: format!("{path}.bone_index"),
                    reason: "bone channel entries must use strictly increasing unique bone indices"
                        .into(),
                });
            }
            previous_bone_index = Some(bone.bone_index);
            if bone.properties.is_empty() {
                return Err(MeasurementContractError::InvalidStructure {
                    path: format!("{path}.properties"),
                    reason: "bone channel coverage must contain at least one property".into(),
                });
            }
            if bone
                .properties
                .windows(2)
                .any(|properties| properties[0] >= properties[1])
            {
                return Err(MeasurementContractError::InvalidStructure {
                    path: format!("{path}.properties"),
                    reason:
                        "channel properties must be unique and ordered translation, rotation, scale"
                            .into(),
                });
            }
            covered_bone_names.insert(bone.bone_name.clone());
        }
        let expected_animated_bones: Vec<_> = covered_bone_names.into_iter().collect();
        if clip.animated_bones != expected_animated_bones {
            return Err(MeasurementContractError::InvalidStructure {
                path: format!("clips[{clip_name:?}].animated_bones"),
                reason: "animated_bones must equal the sorted unique bone names in bone_channels"
                    .into(),
            });
        }
        for (bone, value) in &clip.bone_rotation_range_deg {
            if clip.animated_bones.binary_search(bone).is_err() {
                return Err(MeasurementContractError::InvalidStructure {
                    path: format!("clips[{clip_name:?}].bone_rotation_range_deg[{bone:?}]"),
                    reason: "rotation-range bones must be present in animated_bones".into(),
                });
            }
            finite(
                *value,
                format!("clips[{clip_name:?}].bone_rotation_range_deg[{bone:?}]"),
            )?;
        }
        check_availability(
            clip.loop_continuity.is_some(),
            clip.loop_continuity_availability,
            format!("clips[{clip_name:?}].loop_continuity"),
        )?;
        check_availability(
            clip.loop_endpoint_mode.is_some(),
            clip.loop_endpoint_mode_availability,
            format!("clips[{clip_name:?}].loop_endpoint_mode"),
        )?;
        check_availability(
            clip.frame_grid.is_some(),
            clip.frame_grid_availability,
            format!("clips[{clip_name:?}].frame_grid"),
        )?;
        check_availability(
            clip.loop_seam_ratio.is_some(),
            clip.loop_seam_ratio_availability,
            format!("clips[{clip_name:?}].loop_seam_ratio"),
        )?;
        check_availability(
            clip.gait.is_some(),
            clip.gait_availability,
            format!("clips[{clip_name:?}].gait"),
        )?;
        check_availability(
            clip.root_trajectory.is_some(),
            clip.root_trajectory_availability,
            format!("clips[{clip_name:?}].root_trajectory"),
        )?;
        check_availability(
            clip.speed_mps.is_some(),
            clip.speed_mps_availability,
            format!("clips[{clip_name:?}].speed_mps"),
        )?;
        if let Some(gait) = &clip.gait {
            check_availability(
                gait.phase.is_some(),
                gait.phase_availability,
                format!("clips[{clip_name:?}].gait.phase"),
            )?;
        }
        if let Some(trajectory) = &clip.root_trajectory {
            let path = format!("clips[{clip_name:?}].root_trajectory");
            check_availability(
                trajectory.translation.is_some(),
                trajectory.translation_availability,
                format!("{path}.translation"),
            )?;
            check_availability(
                trajectory.yaw.is_some(),
                trajectory.yaw_availability,
                format!("{path}.yaw"),
            )?;
            if trajectory.translation_availability == MeasurementAvailability::NotApplicable {
                return Err(MeasurementContractError::InvalidStructure {
                    path: format!("{path}.translation_availability"),
                    reason:
                        "translation remains applicable when a root-trajectory bone is selected"
                            .into(),
                });
            }
            if trajectory.yaw_availability == MeasurementAvailability::NotApplicable {
                return Err(MeasurementContractError::InvalidStructure {
                    path: format!("{path}.yaw_availability"),
                    reason: "yaw remains applicable when a root-trajectory bone is selected".into(),
                });
            }
            if let Some(translation) = trajectory.translation {
                for (field, value) in [
                    (
                        "horizontal_displacement_x_m",
                        translation.horizontal_displacement_x_m,
                    ),
                    (
                        "horizontal_displacement_z_m",
                        translation.horizontal_displacement_z_m,
                    ),
                    ("horizontal_travel_m", translation.horizontal_travel_m),
                    (
                        "vertical_displacement_m",
                        translation.vertical_displacement_m,
                    ),
                    (
                        "vertical_min_displacement_m",
                        translation.vertical_min_displacement_m,
                    ),
                    (
                        "vertical_max_displacement_m",
                        translation.vertical_max_displacement_m,
                    ),
                ] {
                    finite(value, format!("{path}.translation.{field}"))?;
                }
                if translation.horizontal_travel_m < 0.0 {
                    return Err(MeasurementContractError::InvalidStructure {
                        path: format!("{path}.translation.horizontal_travel_m"),
                        reason: "sampled horizontal travel must be non-negative".into(),
                    });
                }
                let horizontal_displacement_m = translation
                    .horizontal_displacement_x_m
                    .hypot(translation.horizontal_displacement_z_m);
                if !permits_roundoff(translation.horizontal_travel_m, horizontal_displacement_m) {
                    return Err(MeasurementContractError::InvalidStructure {
                        path: format!("{path}.translation.horizontal_travel_m"),
                        reason: "sampled horizontal travel must contain endpoint displacement"
                            .into(),
                    });
                }
                if translation.vertical_min_displacement_m > 0.0
                    || translation.vertical_max_displacement_m < 0.0
                    || translation.vertical_displacement_m < translation.vertical_min_displacement_m
                    || translation.vertical_displacement_m > translation.vertical_max_displacement_m
                {
                    return Err(MeasurementContractError::InvalidStructure {
                        path: format!("{path}.translation"),
                        reason: "vertical extrema must include zero and the endpoint displacement"
                            .into(),
                    });
                }
            }
            if let Some(yaw) = trajectory.yaw {
                finite(yaw.net_yaw_deg, format!("{path}.yaw.net_yaw_deg"))?;
                finite(
                    yaw.unwrapped_yaw_deg,
                    format!("{path}.yaw.unwrapped_yaw_deg"),
                )?;
                finite(yaw.yaw_travel_deg, format!("{path}.yaw.yaw_travel_deg"))?;
                if !(-180.0..=180.0).contains(&yaw.net_yaw_deg) {
                    return Err(MeasurementContractError::InvalidStructure {
                        path: format!("{path}.yaw.net_yaw_deg"),
                        reason: "net yaw must be in the inclusive range [-180, 180]".into(),
                    });
                }
                if yaw.net_yaw_deg != canonical_net_yaw_deg(yaw.unwrapped_yaw_deg) {
                    return Err(MeasurementContractError::InvalidStructure {
                        path: format!("{path}.yaw.net_yaw_deg"),
                        reason: "net yaw must be the canonical endpoint-equivalent unwrapped yaw"
                            .into(),
                    });
                }
                if yaw.yaw_travel_deg < 0.0 {
                    return Err(MeasurementContractError::InvalidStructure {
                        path: format!("{path}.yaw.yaw_travel_deg"),
                        reason: "sampled yaw travel must be non-negative".into(),
                    });
                }
                if !permits_roundoff(yaw.yaw_travel_deg, yaw.unwrapped_yaw_deg.abs()) {
                    return Err(MeasurementContractError::InvalidStructure {
                        path: format!("{path}.yaw.yaw_travel_deg"),
                        reason: "sampled yaw travel must contain signed unwrapped yaw".into(),
                    });
                }
            }
        }
        if let Some(loop_continuity) = &clip.loop_continuity {
            if loop_continuity.bones.is_empty() {
                return Err(MeasurementContractError::InvalidStructure {
                    path: format!("clips[{clip_name:?}].loop_continuity.bones"),
                    reason: "present loop-continuity evidence must contain at least one bone"
                        .into(),
                });
            }
            for (expected_index, bone) in loop_continuity.bones.iter().enumerate() {
                let path = format!("clips[{clip_name:?}].loop_continuity.bones[{expected_index}]");
                if usize::try_from(bone.bone_index) != Ok(expected_index) {
                    return Err(MeasurementContractError::InvalidStructure {
                        path: format!("{path}.bone_index"),
                        reason: format!(
                            "expected skeleton-order index {expected_index}, found {}",
                            bone.bone_index
                        ),
                    });
                }
                if revision == MeasurementRevision::V17 && !bone.availability_was_present {
                    return Err(MeasurementContractError::InvalidStructure {
                        path: format!("{path}.availability"),
                        reason: "measurements-v17 requires explicit per-bone availability".into(),
                    });
                }
                if revision != MeasurementRevision::V17 && bone.availability_was_present {
                    return Err(MeasurementContractError::InvalidStructure {
                        path: format!("{path}.availability"),
                        reason:
                            "per-bone loop-continuity availability is exclusive to measurements-v17"
                                .into(),
                    });
                }
                if revision != MeasurementRevision::V17
                    && bone.availability != MeasurementAvailability::Measured
                {
                    return Err(MeasurementContractError::InvalidStructure {
                        path: format!("{path}.availability"),
                        reason: "historical measurement contracts require every present loop-continuity row to be measured"
                            .into(),
                    });
                }
                if bone.availability == MeasurementAvailability::NotApplicable {
                    return Err(MeasurementContractError::InvalidStructure {
                        path: format!("{path}.availability"),
                        reason: "an existing bone's loop-continuity evidence remains applicable"
                            .into(),
                    });
                }
                let values = [
                    ("position_delta_m", bone.position_delta_m),
                    ("rotation_delta_deg", bone.rotation_delta_deg),
                    ("seam_velocity_delta_mps", bone.seam_velocity_delta_mps),
                    (
                        "seam_angular_velocity_delta_degps",
                        bone.seam_angular_velocity_delta_degps,
                    ),
                ];
                let values_present = values.iter().filter(|(_, value)| value.is_some()).count();
                match (bone.availability, values_present) {
                    (MeasurementAvailability::Measured, 4)
                    | (MeasurementAvailability::Unavailable, 0) => {}
                    _ => {
                        return Err(MeasurementContractError::InvalidStructure {
                            path: path.clone(),
                            reason: "all loop-continuity values must be present exactly when the bone is measured"
                                .into(),
                        });
                    }
                }
                for (field, value) in values {
                    let Some(value) = value else { continue };
                    finite(value, format!("{path}.{field}"))?;
                    if value < 0.0 {
                        return Err(MeasurementContractError::InvalidStructure {
                            path: format!("{path}.{field}"),
                            reason: "loop-continuity deltas must be non-negative".into(),
                        });
                    }
                }
            }
        }
        if let Some(frame_grid) = &clip.frame_grid {
            let path = format!("clips[{clip_name:?}].frame_grid");
            finite(frame_grid.fps, format!("{path}.fps"))?;
            if frame_grid.fps <= 0.0 {
                return Err(MeasurementContractError::InvalidStructure {
                    path: format!("{path}.fps"),
                    reason: "declared frame-grid FPS must be positive".into(),
                });
            }
            if frame_grid.frame_intervals == 0 {
                return Err(MeasurementContractError::InvalidStructure {
                    path: format!("{path}.frame_intervals"),
                    reason: "declared frame-grid evidence must contain at least one interval"
                        .into(),
                });
            }
        }
        if let Some(value) = clip.loop_seam_ratio {
            finite(value, format!("clips[{clip_name:?}].loop_seam_ratio"))?;
        }
        if let Some(gait) = &clip.gait {
            if let Some(value) = gait.phase {
                finite(value, format!("clips[{clip_name:?}].gait.phase"))?;
            }
            finite(
                gait.lr_amplitude_m,
                format!("clips[{clip_name:?}].gait.lr_amplitude_m"),
            )?;
        }
        if let Some(value) = clip.speed_mps {
            finite(value, format!("clips[{clip_name:?}].speed_mps"))?;
        }
    }
    let invalid = |path: String, reason: &str| MeasurementContractError::InvalidStructure {
        path,
        reason: reason.to_owned(),
    };
    let finite_aabb = |aabb: &Aabb, path: &str| {
        for (corner, values) in [("min", aabb.min), ("max", aabb.max)] {
            for (axis, value) in values.into_iter().enumerate() {
                finite(f64::from(value), format!("{path}.{corner}[{axis}]"))?;
            }
        }
        for (axis, (min, max)) in aabb.min.into_iter().zip(aabb.max).enumerate() {
            if min > max {
                return Err(invalid(
                    format!("{path}.min[{axis}]"),
                    "AABB minimum cannot exceed maximum",
                ));
            }
        }
        Ok(())
    };

    let mut mesh_indices = BTreeSet::new();
    for (index, mesh) in assets.mesh_definitions.iter().enumerate() {
        if !mesh_indices.insert(mesh.mesh_index) {
            return Err(invalid(
                format!("mesh_definitions[{index}].mesh_index"),
                "mesh_index must be unique",
            ));
        }
        if let Some(aabb) = &mesh.geometry_aabb {
            finite_aabb(aabb, &format!("mesh_definitions[{index}].geometry_aabb"))?;
        }
        if let Some(centroid) = mesh.geometry_centroid {
            for (axis, value) in centroid.into_iter().enumerate() {
                finite(
                    f64::from(value),
                    format!("mesh_definitions[{index}].geometry_centroid[{axis}]"),
                )?;
            }
        }
        if let Some(value) = mesh.weight_sum_min {
            finite(value, format!("mesh_definitions[{index}].weight_sum_min"))?;
        }
        if let Some(value) = mesh.weight_sum_max {
            finite(value, format!("mesh_definitions[{index}].weight_sum_max"))?;
        }
        if revision == MeasurementRevision::V15 && mesh.vertex_count > u64::from(u32::MAX) {
            return Err(invalid(
                format!("mesh_definitions[{index}].vertex_count"),
                "measurements-v15 vertex_count cannot exceed its historical u32 maximum",
            ));
        }
        match (&mesh.primitives, revision) {
            (None, MeasurementRevision::V16 | MeasurementRevision::V17) => {
                return Err(invalid(
                    format!("mesh_definitions[{index}].primitives"),
                    "measurements-v16 requires per-primitive evidence",
                ));
            }
            (Some(_), MeasurementRevision::V15) => {
                return Err(invalid(
                    format!("mesh_definitions[{index}].primitives"),
                    "measurements-v15 cannot carry per-primitive evidence",
                ));
            }
            (Some(primitives), MeasurementRevision::V16 | MeasurementRevision::V17) => {
                let mut summed_vertex_count = 0u64;
                let mut summed_finite_vertex_count = 0u64;
                let mut aggregate_min = [f32::INFINITY; 3];
                let mut aggregate_max = [f32::NEG_INFINITY; 3];
                let mut weighted_centroid_sum = [0.0f64; 3];
                let mut previous_primitive_index = None;
                for (primitive_offset, primitive) in primitives.iter().enumerate() {
                    let path = format!("mesh_definitions[{index}].primitives[{primitive_offset}]");
                    if previous_primitive_index
                        .is_some_and(|previous| previous >= primitive.primitive_index)
                    {
                        return Err(invalid(
                            format!("{path}.primitive_index"),
                            "primitive_index must be unique and strictly increasing in source order",
                        ));
                    }
                    previous_primitive_index = Some(primitive.primitive_index);
                    if primitive.finite_vertex_count > primitive.vertex_count {
                        return Err(invalid(
                            format!("{path}.finite_vertex_count"),
                            "finite_vertex_count cannot exceed vertex_count",
                        ));
                    }
                    match (
                        primitive.finite_vertex_count,
                        primitive.geometry_aabb.as_ref(),
                        primitive.geometry_centroid,
                    ) {
                        (0, None, None) => {}
                        (1.., Some(aabb), Some(centroid)) => {
                            finite_aabb(aabb, &format!("{path}.geometry_aabb"))?;
                            for (axis, value) in centroid.into_iter().enumerate() {
                                finite(
                                    f64::from(value),
                                    format!("{path}.geometry_centroid[{axis}]"),
                                )?;
                                if value < aabb.min[axis] || value > aabb.max[axis] {
                                    return Err(invalid(
                                        format!("{path}.geometry_centroid[{axis}]"),
                                        "primitive centroid must lie inside its geometry AABB",
                                    ));
                                }
                                aggregate_min[axis] = aggregate_min[axis].min(aabb.min[axis]);
                                aggregate_max[axis] = aggregate_max[axis].max(aabb.max[axis]);
                                weighted_centroid_sum[axis] +=
                                    f64::from(value) * primitive.finite_vertex_count as f64;
                            }
                        }
                        (0, _, _) => {
                            return Err(invalid(
                                path,
                                "a primitive with no finite vertices cannot carry geometry facts",
                            ));
                        }
                        (1.., _, _) => {
                            return Err(invalid(
                                path,
                                "a primitive with finite vertices requires both geometry facts",
                            ));
                        }
                    }
                    if assets.material_resource_coverage == MaterialResourceCoverage::Complete
                        && primitive.material_index.is_some_and(|material_index| {
                            material_index >= assets.material_definitions.len()
                        })
                    {
                        return Err(invalid(
                            format!("{path}.material_index"),
                            "material_index must reference a source material when material resource coverage is complete",
                        ));
                    }
                    summed_vertex_count = summed_vertex_count
                        .checked_add(primitive.vertex_count)
                        .ok_or_else(|| {
                        invalid(
                            format!("mesh_definitions[{index}].vertex_count"),
                            "primitive vertex-count sum overflows u64",
                        )
                    })?;
                    summed_finite_vertex_count = summed_finite_vertex_count
                        .checked_add(primitive.finite_vertex_count)
                        .ok_or_else(|| {
                            invalid(
                                format!("mesh_definitions[{index}].primitives"),
                                "primitive finite-vertex-count sum overflows u64",
                            )
                        })?;
                }
                if summed_vertex_count != mesh.vertex_count {
                    return Err(invalid(
                        format!("mesh_definitions[{index}].vertex_count"),
                        "vertex_count must equal the checked sum of primitive vertex counts",
                    ));
                }
                let expected_aabb = (summed_finite_vertex_count != 0).then_some(Aabb {
                    min: aggregate_min,
                    max: aggregate_max,
                });
                let expected_centroid = (summed_finite_vertex_count != 0).then(|| {
                    let count = summed_finite_vertex_count as f64;
                    weighted_centroid_sum.map(|sum| (sum / count) as f32)
                });
                match (
                    summed_finite_vertex_count,
                    mesh.geometry_aabb.as_ref(),
                    mesh.geometry_centroid,
                ) {
                    (0, None, None) | (1.., Some(_), Some(_)) => {}
                    (0, _, _) => {
                        return Err(invalid(
                            format!("mesh_definitions[{index}]"),
                            "a mesh with no finite primitive vertices cannot carry geometry facts",
                        ));
                    }
                    (1.., _, _) => {
                        return Err(invalid(
                            format!("mesh_definitions[{index}]"),
                            "a mesh with finite primitive vertices requires both geometry facts",
                        ));
                    }
                }
                if mesh.geometry_aabb != expected_aabb {
                    return Err(invalid(
                        format!("mesh_definitions[{index}].geometry_aabb"),
                        "mesh AABB must equal the exact union of primitive AABBs",
                    ));
                }
                if mesh.geometry_centroid != expected_centroid {
                    return Err(invalid(
                        format!("mesh_definitions[{index}].geometry_centroid"),
                        "mesh centroid must equal the finite-count-weighted primitive centroids",
                    ));
                }
            }
            (None, MeasurementRevision::V15) => {}
        }
        let mut previous_set_index = None;
        for (set_offset, set) in mesh.additional_influence_sets.iter().enumerate() {
            let path = format!(
                "mesh_definitions[{index}].additional_influence_sets[{set_offset}].set_index"
            );
            if set.set_index == 0 {
                return Err(invalid(path, "set_index must be at least 1"));
            }
            if !set.joints_present && !set.weights_present {
                return Err(invalid(
                    format!("mesh_definitions[{index}].additional_influence_sets[{set_offset}]"),
                    "an additional influence set must declare joints, weights, or both",
                ));
            }
            if set.joints_without_weights_present && !set.joints_present {
                return Err(invalid(
                    format!(
                        "mesh_definitions[{index}].additional_influence_sets[{set_offset}].joints_without_weights_present"
                    ),
                    "joints_without_weights_present requires joints_present",
                ));
            }
            if set.weights_without_joints_present && !set.weights_present {
                return Err(invalid(
                    format!(
                        "mesh_definitions[{index}].additional_influence_sets[{set_offset}].weights_without_joints_present"
                    ),
                    "weights_without_joints_present requires weights_present",
                ));
            }
            if set.joints_present && !set.weights_present && !set.joints_without_weights_present {
                return Err(invalid(
                    format!(
                        "mesh_definitions[{index}].additional_influence_sets[{set_offset}].joints_without_weights_present"
                    ),
                    "joints_without_weights_present is required when weights_present is false",
                ));
            }
            if set.weights_present && !set.joints_present && !set.weights_without_joints_present {
                return Err(invalid(
                    format!(
                        "mesh_definitions[{index}].additional_influence_sets[{set_offset}].weights_without_joints_present"
                    ),
                    "weights_without_joints_present is required when joints_present is false",
                ));
            }
            if previous_set_index.is_some_and(|previous| previous >= set.set_index) {
                return Err(invalid(
                    path,
                    "set_index values must be strictly increasing and unique",
                ));
            }
            previous_set_index = Some(set.set_index);
        }
    }

    let mut node_indices = BTreeSet::new();
    for (index, instance) in assets.node_instances.iter().enumerate() {
        if !node_indices.insert(instance.node_index) {
            return Err(invalid(
                format!("node_instances[{index}].node_index"),
                "node_index must be unique",
            ));
        }
        if !mesh_indices.contains(&instance.mesh_index) {
            return Err(invalid(
                format!("node_instances[{index}].mesh_index"),
                "mesh_index must reference a mesh definition",
            ));
        }
        match (
            instance.static_node_world_aabb.as_ref(),
            instance.static_node_world_aabb_unavailable_reason,
        ) {
            (Some(aabb), None) => finite_aabb(
                aabb,
                &format!("node_instances[{index}].static_node_world_aabb"),
            )?,
            (None, Some(_)) => {}
            (Some(_), Some(_)) => {
                return Err(invalid(
                    format!("node_instances[{index}]"),
                    "an available static node AABB cannot have an unavailable reason",
                ));
            }
            (None, None) => {
                return Err(invalid(
                    format!("node_instances[{index}]"),
                    "a missing static node AABB requires an unavailable reason",
                ));
            }
        }
    }

    let mut scene_indices = BTreeSet::new();
    for (index, scene) in assets.scenes.iter().enumerate() {
        if !scene_indices.insert(scene.scene_index) {
            return Err(invalid(
                format!("scenes[{index}].scene_index"),
                "scene_index must be unique",
            ));
        }
        if scene.excluded_instance_count > scene.instance_count {
            return Err(invalid(
                format!("scenes[{index}].excluded_instance_count"),
                "excluded_instance_count cannot exceed instance_count",
            ));
        }
        let available = scene.instance_count - scene.excluded_instance_count;
        match (&scene.static_scene_world_aabb, available) {
            (Some(aabb), 1..) => {
                finite_aabb(aabb, &format!("scenes[{index}].static_scene_world_aabb"))?
            }
            (None, 0) => {}
            (Some(_), 0) => {
                return Err(invalid(
                    format!("scenes[{index}].static_scene_world_aabb"),
                    "a scene with no available instances cannot have an AABB",
                ));
            }
            (None, _) => {
                return Err(invalid(
                    format!("scenes[{index}].static_scene_world_aabb"),
                    "a scene with available instances requires an AABB",
                ));
            }
        }
    }
    if let Some(default_scene_index) = assets.default_scene_index
        && !scene_indices.contains(&default_scene_index)
    {
        return Err(invalid(
            "default_scene_index".into(),
            "default_scene_index must reference a declared scene",
        ));
    }
    validate_skeleton_measurements(assets, &invalid)?;
    validate_material_resources(assets, revision, &invalid)?;
    Ok(())
}

fn validate_linear_transform_fields(
    linear: &LinearTransformMeasurements,
    path: &str,
    invalid: &impl Fn(String, &str) -> MeasurementContractError,
) -> Result<(), MeasurementContractError> {
    let numeric_fields_present = linear.axis_lengths.is_some()
        && linear.determinant.is_some()
        && linear.orientation.is_some();
    if linear.classification == LinearTransformClassification::NonFinite {
        if linear.axis_lengths.is_some()
            || linear.determinant.is_some()
            || linear.orientation.is_some()
            || linear.uniform_scale.is_some()
        {
            return Err(invalid(
                path.into(),
                "a non_finite classification cannot carry numeric linear-transform facts",
            ));
        }
        return Ok(());
    }
    if !numeric_fields_present {
        return Err(invalid(
            path.into(),
            "a finite classification requires axis_lengths, determinant, and orientation",
        ));
    }
    for (axis, value) in linear
        .axis_lengths
        .expect("presence checked")
        .into_iter()
        .enumerate()
    {
        if !value.is_finite() {
            return Err(MeasurementContractError::NonFiniteValue {
                path: format!("{path}.axis_lengths[{axis}]"),
            });
        }
        if value < 0.0 {
            return Err(invalid(
                format!("{path}.axis_lengths[{axis}]"),
                "axis lengths must be non-negative",
            ));
        }
    }
    if !linear.determinant.expect("presence checked").is_finite() {
        return Err(MeasurementContractError::NonFiniteValue {
            path: format!("{path}.determinant"),
        });
    }
    if let Some(scale) = linear.uniform_scale {
        if !scale.is_finite() {
            return Err(MeasurementContractError::NonFiniteValue {
                path: format!("{path}.uniform_scale"),
            });
        }
        if scale < 0.0 {
            return Err(invalid(
                format!("{path}.uniform_scale"),
                "uniform scale must be non-negative",
            ));
        }
    }
    Ok(())
}

fn validate_skeleton_measurements(
    assets: &AssetMeasurements,
    invalid: &impl Fn(String, &str) -> MeasurementContractError,
) -> Result<(), MeasurementContractError> {
    if assets.skeleton_source_coverage == SourceSkeletonCoverage::Unavailable {
        if !assets.skeleton_nodes.is_empty() || !assets.skins.is_empty() {
            return Err(invalid(
                "skeleton_source_coverage".into(),
                "unavailable skeleton source coverage requires empty skeleton_nodes and skins arrays",
            ));
        }
        return Ok(());
    }

    let finite_matrix = |matrix: &[f32; 16], path: &str| {
        for (component, value) in matrix.iter().enumerate() {
            if !value.is_finite() {
                return Err(MeasurementContractError::NonFiniteValue {
                    path: format!("{path}[{component}]"),
                });
            }
        }
        Ok(())
    };
    for (offset, node) in assets.skeleton_nodes.iter().enumerate() {
        if node.node_index != offset {
            return Err(invalid(
                format!("skeleton_nodes[{offset}].node_index"),
                "node_index must be contiguous and match source order",
            ));
        }
        match &node.local_rest {
            SkeletonNodeLocalRestMeasurements::Trs {
                translation_parent_space_m,
                rotation_xyzw,
                scale,
            } => {
                for (field, values) in [
                    (
                        "translation_parent_space_m",
                        translation_parent_space_m.as_slice(),
                    ),
                    ("rotation_xyzw", rotation_xyzw.as_slice()),
                    ("scale", scale.as_slice()),
                ] {
                    for (component, value) in values.iter().enumerate() {
                        if !value.is_finite() {
                            return Err(MeasurementContractError::NonFiniteValue {
                                path: format!(
                                    "skeleton_nodes[{offset}].local_rest.{field}[{component}]"
                                ),
                            });
                        }
                    }
                }
            }
            SkeletonNodeLocalRestMeasurements::Matrix { matrix } => finite_matrix(
                matrix,
                &format!("skeleton_nodes[{offset}].local_rest.matrix"),
            )?,
            SkeletonNodeLocalRestMeasurements::Unavailable { .. } => {}
        }
        let node_path = format!("skeleton_nodes[{offset}]");
        validate_linear_transform_fields(
            &node.rest_world_linear,
            &format!("{node_path}.rest_world_linear"),
            invalid,
        )?;
        match (
            node.rest_world_matrix.as_ref(),
            node.rest_world_translation_m.as_ref(),
            node.rest_world_matrix_unavailable_reason,
        ) {
            (Some(matrix), Some(translation), None) => {
                finite_matrix(matrix, &format!("{node_path}.rest_world_matrix"))?;
                for (component, value) in translation.iter().enumerate() {
                    if !value.is_finite() {
                        return Err(MeasurementContractError::NonFiniteValue {
                            path: format!("{node_path}.rest_world_translation_m[{component}]"),
                        });
                    }
                }
                let expected_translation = [matrix[12], matrix[13], matrix[14]];
                if *translation != expected_translation {
                    return Err(invalid(
                        format!("{node_path}.rest_world_translation_m"),
                        "rest_world_translation_m must equal the rest-world matrix translation column",
                    ));
                }
                let expected_linear = measure_linear_transform(Mat4::from_cols_array(matrix));
                if node.rest_world_linear != expected_linear {
                    return Err(invalid(
                        format!("{node_path}.rest_world_linear"),
                        "rest_world_linear must be derived from rest_world_matrix",
                    ));
                }
            }
            (None, None, Some(_)) => {
                if node.rest_world_linear.classification != LinearTransformClassification::NonFinite
                {
                    return Err(invalid(
                        format!("{node_path}.rest_world_linear"),
                        "an unavailable rest-world matrix requires a non_finite linear classification",
                    ));
                }
            }
            (Some(_), Some(_), Some(_)) => {
                return Err(invalid(
                    node_path,
                    "an available rest_world_matrix cannot have an unavailable reason",
                ));
            }
            _ => {
                return Err(invalid(
                    node_path,
                    "rest-world matrix, translation, and unavailable reason fields are inconsistent",
                ));
            }
        }
    }
    for (offset, node) in assets.skeleton_nodes.iter().enumerate() {
        if let Some(parent) = node.parent_node_index
            && parent >= assets.skeleton_nodes.len()
        {
            return Err(invalid(
                format!("skeleton_nodes[{offset}].parent_node_index"),
                "parent_node_index must reference a skeleton node",
            ));
        }
        let mut previous_scene = None;
        for (scene_offset, scene_index) in node.scene_root_indices.iter().enumerate() {
            if !assets
                .scenes
                .iter()
                .any(|scene| scene.scene_index == *scene_index)
            {
                return Err(invalid(
                    format!("skeleton_nodes[{offset}].scene_root_indices[{scene_offset}]"),
                    "scene_root_indices values must reference declared scenes",
                ));
            }
            if previous_scene.is_some_and(|previous| previous >= *scene_index) {
                return Err(invalid(
                    format!("skeleton_nodes[{offset}].scene_root_indices[{scene_offset}]"),
                    "scene_root_indices values must be strictly increasing and unique",
                ));
            }
            previous_scene = Some(*scene_index);
        }
    }
    let mut visits = vec![ParentVisit::Unvisited; assets.skeleton_nodes.len()];
    for start in 0..assets.skeleton_nodes.len() {
        if visits.get(start) != Some(&ParentVisit::Unvisited) {
            continue;
        }
        let mut path = Vec::new();
        let mut current = start;
        loop {
            match visits.get(current).copied().ok_or_else(|| {
                invalid(
                    format!("skeleton_nodes[{current}].parent_node_index"),
                    "parent_node_index must reference a skeleton node",
                )
            })? {
                ParentVisit::Done => break,
                ParentVisit::Visiting => {
                    return Err(invalid(
                        format!("skeleton_nodes[{current}].parent_node_index"),
                        "source node parent graph must be acyclic",
                    ));
                }
                ParentVisit::Unvisited => {
                    *visits.get_mut(current).ok_or_else(|| {
                        invalid(
                            format!("skeleton_nodes[{current}].parent_node_index"),
                            "parent_node_index must reference a skeleton node",
                        )
                    })? = ParentVisit::Visiting;
                    path.push(current);
                    match assets
                        .skeleton_nodes
                        .get(current)
                        .ok_or_else(|| {
                            invalid(
                                format!("skeleton_nodes[{current}].parent_node_index"),
                                "parent_node_index must reference a skeleton node",
                            )
                        })?
                        .parent_node_index
                    {
                        Some(parent) => current = parent,
                        None => break,
                    }
                }
            }
        }
        for node_index in path {
            *visits.get_mut(node_index).ok_or_else(|| {
                invalid(
                    format!("skeleton_nodes[{node_index}].parent_node_index"),
                    "parent_node_index must reference a skeleton node",
                )
            })? = ParentVisit::Done;
        }
    }

    for (offset, node) in assets.skeleton_nodes.iter().enumerate() {
        let local_rest_available = !matches!(
            node.local_rest,
            SkeletonNodeLocalRestMeasurements::Unavailable { .. }
        );
        let path = format!("skeleton_nodes[{offset}]");
        if !local_rest_available {
            if node.rest_world_matrix.is_some()
                || node.rest_world_matrix_unavailable_reason
                    != Some(SkeletonRestWorldMatrixUnavailableReason::NonFiniteLocalRest)
            {
                return Err(invalid(
                    path,
                    "an unavailable local_rest requires a non_finite_local_rest rest-world result",
                ));
            }
            continue;
        }

        let expected_unavailable_reason = if let Some(parent_index) = node.parent_node_index {
            let parent = assets.skeleton_nodes.get(parent_index).ok_or_else(|| {
                invalid(
                    format!("skeleton_nodes[{offset}].parent_node_index"),
                    "parent_node_index must reference a skeleton node",
                )
            })?;
            if parent.rest_world_matrix.is_none() {
                Some(SkeletonRestWorldMatrixUnavailableReason::ParentRestWorldUnavailable)
            } else {
                Some(SkeletonRestWorldMatrixUnavailableReason::NonFiniteWorldMatrix)
            }
        } else {
            None
        };
        match (
            node.rest_world_matrix.is_some(),
            expected_unavailable_reason,
        ) {
            (true, None | Some(SkeletonRestWorldMatrixUnavailableReason::NonFiniteWorldMatrix)) => {
            }
            (false, Some(expected))
                if node.rest_world_matrix_unavailable_reason == Some(expected) => {}
            _ => {
                return Err(invalid(
                    path,
                    "rest-world availability must agree with local rest and parent rest-world evidence",
                ));
            }
        }
    }

    for (offset, skin) in assets.skins.iter().enumerate() {
        if skin.skin_index != offset {
            return Err(invalid(
                format!("skins[{offset}].skin_index"),
                "skin_index must be contiguous and match source order",
            ));
        }
        if let Some(root) = skin.skeleton_root_node_index
            && root >= assets.skeleton_nodes.len()
        {
            return Err(invalid(
                format!("skins[{offset}].skeleton_root_node_index"),
                "skeleton_root_node_index must reference a skeleton node",
            ));
        }
        for (joint_offset, joint) in skin.joints.iter().enumerate() {
            if joint.joint_index != joint_offset {
                return Err(invalid(
                    format!("skins[{offset}].joints[{joint_offset}].joint_index"),
                    "joint_index must be contiguous and match declared skin order",
                ));
            }
            if joint.node_index >= assets.skeleton_nodes.len() {
                return Err(invalid(
                    format!("skins[{offset}].joints[{joint_offset}].node_index"),
                    "joint node_index must reference a skeleton node",
                ));
            }
        }
        match skin.inverse_bind_accessor.status {
            SourceInverseBindAccessorStatus::Absent => {
                if skin.inverse_bind_accessor.declared_count.is_some()
                    || !skin.inverse_bind_accessor.matrices.is_empty()
                {
                    return Err(invalid(
                        format!("skins[{offset}].inverse_bind_accessor"),
                        "an absent inverse-bind declaration has no declared count or matrices",
                    ));
                }
            }
            SourceInverseBindAccessorStatus::EmptyAccessor => {
                if skin.inverse_bind_accessor.declared_count != Some(0)
                    || !skin.inverse_bind_accessor.matrices.is_empty()
                {
                    return Err(invalid(
                        format!("skins[{offset}].inverse_bind_accessor"),
                        "an empty inverse-bind declaration has declared_count 0 and no matrices",
                    ));
                }
            }
            SourceInverseBindAccessorStatus::Available => {
                if skin.inverse_bind_accessor.declared_count
                    != Some(skin.inverse_bind_accessor.matrices.len())
                    || skin.inverse_bind_accessor.matrices.len() < skin.joints.len()
                {
                    return Err(invalid(
                        format!("skins[{offset}].inverse_bind_accessor"),
                        "an available inverse-bind declaration must retain its declared finite matrices and cover every joint",
                    ));
                }
            }
            SourceInverseBindAccessorStatus::CountMismatch => {
                if skin.inverse_bind_accessor.declared_count
                    != Some(skin.inverse_bind_accessor.matrices.len())
                    || skin.inverse_bind_accessor.matrices.len() >= skin.joints.len()
                {
                    return Err(invalid(
                        format!("skins[{offset}].inverse_bind_accessor"),
                        "a count-mismatched inverse-bind declaration retains fewer matrices than joints",
                    ));
                }
            }
            SourceInverseBindAccessorStatus::Unreadable => {
                if skin.inverse_bind_accessor.declared_count.is_none()
                    || !skin.inverse_bind_accessor.matrices.is_empty()
                {
                    return Err(invalid(
                        format!("skins[{offset}].inverse_bind_accessor"),
                        "an unreadable inverse-bind declaration retains its count but cannot serialize matrices",
                    ));
                }
            }
        }
        for (matrix_offset, matrix) in skin.inverse_bind_accessor.matrices.iter().enumerate() {
            finite_matrix(
                matrix,
                &format!("skins[{offset}].inverse_bind_accessor.matrices[{matrix_offset}]"),
            )?;
        }
        for (joint_offset, joint) in skin.joints.iter().enumerate() {
            let expected_source = skin.inverse_bind_accessor.matrices.get(joint_offset);
            let joint_bind_path =
                format!("skins[{offset}].joints[{joint_offset}].joint_bind_to_mesh");
            validate_derived_matrix(
                &joint.joint_bind_to_mesh,
                &joint_bind_path,
                &finite_matrix,
                invalid,
            )?;
            validate_derived_reason_compatibility(
                &joint.joint_bind_to_mesh,
                skin.inverse_bind_accessor.status,
                skin.inverse_bind_accessor.matrices.len(),
                joint_offset,
                &joint_bind_path,
                DerivedMatrixDomain::JointBindToMesh,
                invalid,
            )?;
            validate_derived_source(
                &joint.joint_bind_to_mesh,
                expected_source,
                None,
                &joint_bind_path,
                DerivedMatrixDomain::JointBindToMesh,
                invalid,
            )?;

            let mesh_bind_path = format!("skins[{offset}].joints[{joint_offset}].mesh_bind_world");
            validate_derived_matrix(
                &joint.mesh_bind_world,
                &mesh_bind_path,
                &finite_matrix,
                invalid,
            )?;
            validate_derived_reason_compatibility(
                &joint.mesh_bind_world,
                skin.inverse_bind_accessor.status,
                skin.inverse_bind_accessor.matrices.len(),
                joint_offset,
                &mesh_bind_path,
                DerivedMatrixDomain::MeshBindWorld,
                invalid,
            )?;
            let joint_rest_world_available = assets
                .skeleton_nodes
                .get(joint.node_index)
                .ok_or_else(|| {
                    invalid(
                        format!("skins[{offset}].joints[{joint_offset}].node_index"),
                        "joint node_index must reference a skeleton node",
                    )
                })?
                .rest_world_matrix
                .is_some();
            let joint_rest_world = assets.skeleton_nodes[joint.node_index]
                .rest_world_matrix
                .as_ref();
            validate_mesh_bind_world_reason_compatibility(
                &joint.mesh_bind_world,
                joint_rest_world_available,
                &mesh_bind_path,
                invalid,
            )?;
            validate_derived_source(
                &joint.mesh_bind_world,
                expected_source,
                joint_rest_world,
                &mesh_bind_path,
                DerivedMatrixDomain::MeshBindWorld,
                invalid,
            )?;
        }
        if let Some(scale) = skin.joint_bind_linear_summary.consistent_uniform_scale
            && !scale.is_finite()
        {
            return Err(MeasurementContractError::NonFiniteValue {
                path: format!("skins[{offset}].joint_bind_linear_summary.consistent_uniform_scale"),
            });
        }
        let expected_summary = summarize_skin_bind_linear(&skin.joints);
        if skin.joint_bind_linear_summary != expected_summary {
            return Err(invalid(
                format!("skins[{offset}].joint_bind_linear_summary"),
                "joint-bind linear summary must match the skin joint observations",
            ));
        }
        let mut previous_attachment_node = None;
        for (attachment_offset, attachment) in skin.attachments.iter().enumerate() {
            if attachment.node_index >= assets.skeleton_nodes.len() {
                return Err(invalid(
                    format!("skins[{offset}].attachments[{attachment_offset}].node_index"),
                    "attachment node_index must reference a skeleton node",
                ));
            }
            if previous_attachment_node.is_some_and(|previous| previous >= attachment.node_index) {
                return Err(invalid(
                    format!("skins[{offset}].attachments[{attachment_offset}].node_index"),
                    "attachment node_index values must be strictly increasing and unique",
                ));
            }
            previous_attachment_node = Some(attachment.node_index);
        }
    }
    Ok(())
}

fn validate_derived_reason_compatibility(
    matrix: &SkinDerivedMatrixMeasurements,
    status: SourceInverseBindAccessorStatus,
    readable_matrix_count: usize,
    joint_index: usize,
    path: &str,
    domain: DerivedMatrixDomain,
    invalid: &impl Fn(String, &str) -> MeasurementContractError,
) -> Result<(), MeasurementContractError> {
    let requires_accessor_reason = match status {
        SourceInverseBindAccessorStatus::Absent => {
            Some(SkinDerivedMatrixUnavailableReason::InverseBindAccessorAbsent)
        }
        SourceInverseBindAccessorStatus::EmptyAccessor => {
            Some(SkinDerivedMatrixUnavailableReason::InverseBindAccessorEmpty)
        }
        SourceInverseBindAccessorStatus::Unreadable => {
            Some(SkinDerivedMatrixUnavailableReason::InverseBindAccessorUnreadable)
        }
        SourceInverseBindAccessorStatus::CountMismatch if joint_index >= readable_matrix_count => {
            Some(SkinDerivedMatrixUnavailableReason::InverseBindAccessorCountMismatch)
        }
        SourceInverseBindAccessorStatus::Available
        | SourceInverseBindAccessorStatus::CountMismatch => None,
    };
    if let Some(expected) = requires_accessor_reason {
        if matrix.matrix.is_some() || matrix.unavailable_reason != Some(expected) {
            return Err(invalid(
                path.into(),
                "derived matrices without a usable inverse bind must carry the matching accessor reason",
            ));
        }
    } else {
        match (domain, matrix.unavailable_reason) {
            (
                _,
                Some(
                    SkinDerivedMatrixUnavailableReason::InverseBindAccessorAbsent
                    | SkinDerivedMatrixUnavailableReason::InverseBindAccessorEmpty
                    | SkinDerivedMatrixUnavailableReason::InverseBindAccessorCountMismatch
                    | SkinDerivedMatrixUnavailableReason::InverseBindAccessorUnreadable,
                ),
            ) => {
                return Err(invalid(
                    format!("{path}.unavailable_reason"),
                    "a usable inverse-bind matrix cannot be reported as accessor-unavailable",
                ));
            }
            (
                DerivedMatrixDomain::JointBindToMesh,
                Some(SkinDerivedMatrixUnavailableReason::JointRestWorldUnavailable),
            ) => {
                return Err(invalid(
                    format!("{path}.unavailable_reason"),
                    "joint_bind_to_mesh cannot use a joint-rest-world unavailable reason",
                ));
            }
            (
                DerivedMatrixDomain::MeshBindWorld,
                Some(
                    SkinDerivedMatrixUnavailableReason::InverseBindMatrixNonInvertible
                    | SkinDerivedMatrixUnavailableReason::InverseBindMatrixNonAffine
                    | SkinDerivedMatrixUnavailableReason::InverseBindMatrixIllConditioned,
                ),
            ) => {
                return Err(invalid(
                    format!("{path}.unavailable_reason"),
                    "mesh_bind_world does not require an invertible inverse-bind matrix",
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_mesh_bind_world_reason_compatibility(
    matrix: &SkinDerivedMatrixMeasurements,
    joint_rest_world_available: bool,
    path: &str,
    invalid: &impl Fn(String, &str) -> MeasurementContractError,
) -> Result<(), MeasurementContractError> {
    match matrix.unavailable_reason {
        Some(SkinDerivedMatrixUnavailableReason::JointRestWorldUnavailable)
            if joint_rest_world_available =>
        {
            Err(invalid(
                format!("{path}.unavailable_reason"),
                "an available joint rest-world matrix cannot be reported as unavailable",
            ))
        }
        Some(SkinDerivedMatrixUnavailableReason::NonFiniteDerivedMatrix)
            if !joint_rest_world_available =>
        {
            Err(invalid(
                format!("{path}.unavailable_reason"),
                "a non-finite mesh-bind-world result requires an available joint rest-world matrix",
            ))
        }
        _ => Ok(()),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ParentVisit {
    Unvisited,
    Visiting,
    Done,
}

#[derive(Clone, Copy)]
enum DerivedMatrixDomain {
    JointBindToMesh,
    MeshBindWorld,
}

fn validate_derived_source(
    measurements: &SkinDerivedMatrixMeasurements,
    expected_source: Option<&[f32; 16]>,
    joint_rest_world: Option<&[f32; 16]>,
    path: &str,
    domain: DerivedMatrixDomain,
    invalid: &impl Fn(String, &str) -> MeasurementContractError,
) -> Result<(), MeasurementContractError> {
    if measurements.source_inverse_bind_matrix.as_ref() != expected_source {
        return Err(invalid(
            format!("{path}.source_inverse_bind_matrix"),
            "source_inverse_bind_matrix must equal the retained declaration slot exactly",
        ));
    }
    let Some(source) = expected_source else {
        if measurements.inversion_quality.is_some() {
            return Err(invalid(
                format!("{path}.inversion_quality"),
                "inversion quality requires a readable source inverse-bind matrix",
            ));
        }
        return Ok(());
    };
    let raw = Mat4::from_cols_array(source);
    match domain {
        DerivedMatrixDomain::JointBindToMesh => {
            let assessment = assess_inverse_bind(raw);
            if measurements.inversion_quality != assessment.quality {
                return Err(invalid(
                    format!("{path}.inversion_quality"),
                    "inversion quality must be derived from the source linear 3x3",
                ));
            }
            match assessment.inverse {
                Ok(inverse) => {
                    if measurements.matrix != Some(inverse.to_cols_array())
                        || measurements.unavailable_reason.is_some()
                    {
                        return Err(invalid(
                            path.into(),
                            "a trustworthy source inverse-bind matrix requires its exact inverse",
                        ));
                    }
                }
                Err(reason) => {
                    if measurements.matrix.is_some()
                        || measurements.unavailable_reason != Some(reason)
                    {
                        return Err(invalid(
                            path.into(),
                            "an untrustworthy source inverse-bind matrix requires its derived reason",
                        ));
                    }
                }
            }
        }
        DerivedMatrixDomain::MeshBindWorld => {
            if measurements.inversion_quality.is_some() {
                return Err(invalid(
                    format!("{path}.inversion_quality"),
                    "mesh_bind_world does not invert its source matrix",
                ));
            }
            if let Some(world) = joint_rest_world {
                let expected = Mat4::from_cols_array(world) * raw;
                if expected.to_cols_array().into_iter().all(f32::is_finite) {
                    if measurements.matrix != Some(expected.to_cols_array())
                        || measurements.unavailable_reason.is_some()
                    {
                        return Err(invalid(
                            path.into(),
                            "mesh_bind_world must equal joint_rest_world times the source inverse bind",
                        ));
                    }
                } else if measurements.unavailable_reason
                    != Some(SkinDerivedMatrixUnavailableReason::NonFiniteDerivedMatrix)
                {
                    return Err(invalid(
                        format!("{path}.unavailable_reason"),
                        "a non-finite mesh-bind product requires its typed unavailable reason",
                    ));
                }
            }
        }
    }
    Ok(())
}

fn validate_derived_matrix(
    matrix: &SkinDerivedMatrixMeasurements,
    path: &str,
    finite_matrix: &impl Fn(&[f32; 16], &str) -> Result<(), MeasurementContractError>,
    invalid: &impl Fn(String, &str) -> MeasurementContractError,
) -> Result<(), MeasurementContractError> {
    if let Some(source) = &matrix.source_inverse_bind_matrix {
        finite_matrix(source, &format!("{path}.source_inverse_bind_matrix"))?;
    }
    if let Some(quality) = matrix.inversion_quality {
        let value = quality.reciprocal_condition_number_inf;
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(invalid(
                format!("{path}.inversion_quality.reciprocal_condition_number_inf"),
                "reciprocal condition number must be finite and between zero and one",
            ));
        }
    }
    match (
        &matrix.matrix,
        matrix.linear.as_ref(),
        matrix.unavailable_reason,
    ) {
        (Some(matrix), Some(linear), None) => {
            finite_matrix(matrix, &format!("{path}.matrix"))?;
            validate_linear_transform_fields(linear, &format!("{path}.linear"), invalid)?;
            if *linear != measure_linear_transform(Mat4::from_cols_array(matrix)) {
                return Err(invalid(
                    format!("{path}.linear"),
                    "linear facts must be derived from the available matrix",
                ));
            }
        }
        (None, None, Some(_)) => {}
        (Some(_), Some(_), Some(_)) => {
            return Err(invalid(
                path.into(),
                "an available derived matrix cannot have an unavailable reason",
            ));
        }
        _ => {
            return Err(invalid(
                path.into(),
                "derived matrix, linear facts, and unavailable reason fields are inconsistent",
            ));
        }
    }
    Ok(())
}

fn validate_material_resources(
    assets: &AssetMeasurements,
    revision: MeasurementRevision,
    invalid: &impl Fn(String, &str) -> MeasurementContractError,
) -> Result<(), MeasurementContractError> {
    let absent = assets.material_definitions.is_empty()
        && assets.textures.is_empty()
        && assets.images.is_empty();
    if assets.material_resource_coverage == MaterialResourceCoverage::Unavailable && !absent {
        return Err(invalid(
            "material_resource_coverage".into(),
            "unavailable resource coverage requires empty material, texture, and image arrays",
        ));
    }

    for (offset, material) in assets.material_definitions.iter().enumerate() {
        if material.material_index != offset {
            return Err(invalid(
                format!("material_definitions[{offset}].material_index"),
                "material_index must be contiguous and match source order",
            ));
        }
        let mut previous_slot = None;
        for (binding_offset, binding) in material.texture_bindings.iter().enumerate() {
            if binding.texture_index >= assets.textures.len() {
                return Err(invalid(
                    format!(
                        "material_definitions[{offset}].texture_bindings[{binding_offset}].texture_index"
                    ),
                    "texture_index must reference a source texture",
                ));
            }
            if previous_slot.is_some_and(|previous| previous >= binding.slot) {
                return Err(invalid(
                    format!(
                        "material_definitions[{offset}].texture_bindings[{binding_offset}].slot"
                    ),
                    "texture bindings must be strictly ordered by slot and unique",
                ));
            }
            previous_slot = Some(binding.slot);
        }
    }
    for (offset, texture) in assets.textures.iter().enumerate() {
        if texture.texture_index != offset {
            return Err(invalid(
                format!("textures[{offset}].texture_index"),
                "texture_index must be contiguous and match source order",
            ));
        }
        if texture.image_index >= assets.images.len() {
            return Err(invalid(
                format!("textures[{offset}].image_index"),
                "image_index must reference a source image",
            ));
        }
    }
    for (offset, image) in assets.images.iter().enumerate() {
        validate_image_measurement(image, offset, revision, invalid)?;
    }
    Ok(())
}

fn validate_image_measurement(
    image: &ImageMeasurements,
    offset: usize,
    revision: MeasurementRevision,
    invalid: &impl Fn(String, &str) -> MeasurementContractError,
) -> Result<(), MeasurementContractError> {
    if image.image_index != offset {
        return Err(invalid(
            format!("images[{offset}].image_index"),
            "image_index must be contiguous and match source order",
        ));
    }
    let available = [
        image.width.is_some(),
        image.height.is_some(),
        image.channel_count.is_some(),
        image.decoded_color_type.is_some(),
    ];
    match (
        available.into_iter().all(|value| value),
        image.unavailable_reason,
    ) {
        (true, None) => {
            let (Some(width), Some(height), Some(channel_count), Some(decoded_color_type)) = (
                image.width,
                image.height,
                image.channel_count,
                image.decoded_color_type,
            ) else {
                return Err(invalid(
                    format!("images[{offset}]"),
                    "available image metadata must include width, height, channel_count, and decoded_color_type",
                ));
            };
            if width == 0 || height == 0 {
                return Err(invalid(
                    format!("images[{offset}]"),
                    "available image dimensions must be greater than zero",
                ));
            }
            if channel_count != color_type_channel_count(decoded_color_type) {
                return Err(invalid(
                    format!("images[{offset}].channel_count"),
                    "channel_count must match decoded_color_type",
                ));
            }
            if image.detected_container.is_none() {
                return Err(invalid(
                    format!("images[{offset}].detected_container"),
                    "available image metadata requires a detected_container",
                ));
            }
        }
        (false, Some(_)) if available.into_iter().all(|value| !value) => {}
        (true, Some(_)) => {
            return Err(invalid(
                format!("images[{offset}]"),
                "available image metadata cannot have an unavailable_reason",
            ));
        }
        (false, None) if available.into_iter().all(|value| !value) => {
            return Err(invalid(
                format!("images[{offset}]"),
                "missing image metadata requires an unavailable_reason",
            ));
        }
        (false, _) => {
            return Err(invalid(
                format!("images[{offset}]"),
                "available image metadata must include width, height, channel_count, and decoded_color_type",
            ));
        }
    }
    match image.unavailable_reason {
        Some(crate::model::ImageUnavailableReason::DecodeFailed)
            if image.detected_container.is_none() =>
        {
            return Err(invalid(
                format!("images[{offset}].detected_container"),
                "decode_failed requires a detected_container",
            ));
        }
        Some(
            crate::model::ImageUnavailableReason::SourceUnavailable
            | crate::model::ImageUnavailableReason::InvalidDataUri
            | crate::model::ImageUnavailableReason::UnsupportedContainer,
        ) if image.detected_container.is_some() => {
            return Err(invalid(
                format!("images[{offset}].detected_container"),
                "this unavailable_reason cannot have a detected_container",
            ));
        }
        _ => {}
    }
    match (revision, image.unavailable_reason, &image.leading_magic_hex) {
        (MeasurementRevision::V15, _, Some(_)) => {
            return Err(invalid(
                format!("images[{offset}].leading_magic_hex"),
                "measurements-v15 cannot carry leading-magic evidence",
            ));
        }
        (
            MeasurementRevision::V16 | MeasurementRevision::V17,
            Some(crate::model::ImageUnavailableReason::UnsupportedContainer),
            Some(magic),
        ) => {
            if magic.is_empty()
                || magic.len() > 32
                || !magic.len().is_multiple_of(2)
                || !magic
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
            {
                return Err(invalid(
                    format!("images[{offset}].leading_magic_hex"),
                    "leading_magic_hex must be nonempty lowercase even-length hex for at most 16 bytes",
                ));
            }
        }
        (
            MeasurementRevision::V16 | MeasurementRevision::V17,
            Some(crate::model::ImageUnavailableReason::UnsupportedContainer),
            None,
        )
        | (MeasurementRevision::V15, _, None) => {}
        (MeasurementRevision::V16 | MeasurementRevision::V17, _, Some(_)) => {
            return Err(invalid(
                format!("images[{offset}].leading_magic_hex"),
                "leading_magic_hex is permitted only for unsupported_container",
            ));
        }
        (MeasurementRevision::V16 | MeasurementRevision::V17, _, None) => {}
    }
    Ok(())
}

fn color_type_channel_count(color_type: DecodedImageColorType) -> u8 {
    match color_type {
        DecodedImageColorType::L8 | DecodedImageColorType::L16 => 1,
        DecodedImageColorType::La8 | DecodedImageColorType::La16 => 2,
        DecodedImageColorType::Rgb8 | DecodedImageColorType::Rgb16 => 3,
        DecodedImageColorType::Rgba8 | DecodedImageColorType::Rgba16 => 4,
    }
}

/// Typed read-side subset accepted when a consumer needs measurements from a
/// current `measure` or `lint` report.
///
/// This intentionally models only the fields needed to recover the nested
/// measurement contract while retaining every legitimate output-v11 root
/// field. The frozen schema is closed, while all protocol identities and
/// command constraints are validated by [`MeasurementReportInput::into_files`].
#[derive(Debug)]
pub struct MeasurementReportInput {
    schema_version: Option<u32>,
    schema: Option<String>,
    _tool: Option<Box<RawValue>>,
    command: Option<String>,
    summary: Option<MeasurementReportSummaryInput>,
    files: Option<Vec<Box<RawValue>>>,
    _inputs: Option<Box<RawValue>>,
    _deltas: Option<Box<RawValue>>,
    extra: BTreeMap<String, Box<RawValue>>,
}

impl<'de> Deserialize<'de> for MeasurementReportInput {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct MeasurementReportInputVisitor;

        impl<'de> Visitor<'de> for MeasurementReportInputVisitor {
            type Value = MeasurementReportInput;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("an output report object")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut schema_version = None;
                let mut schema = None;
                let mut tool = None;
                let mut command = None;
                let mut summary = None;
                let mut files = None;
                let mut inputs = None;
                let mut deltas = None;
                let mut extra = BTreeMap::new();
                while let Some(field) = map.next_key::<String>()? {
                    match field.as_str() {
                        "schema_version" => {
                            if schema_version.is_some() {
                                return Err(serde::de::Error::duplicate_field("schema_version"));
                            }
                            schema_version = Some(map.next_value()?);
                        }
                        "schema" => {
                            if schema.is_some() {
                                return Err(serde::de::Error::duplicate_field("schema"));
                            }
                            schema = Some(map.next_value()?);
                        }
                        "tool" => {
                            if tool.is_some() {
                                return Err(serde::de::Error::duplicate_field("tool"));
                            }
                            tool = Some(map.next_value()?);
                        }
                        "command" => {
                            if command.is_some() {
                                return Err(serde::de::Error::duplicate_field("command"));
                            }
                            command = Some(map.next_value()?);
                        }
                        "summary" => {
                            if summary.is_some() {
                                return Err(serde::de::Error::duplicate_field("summary"));
                            }
                            summary = Some(map.next_value()?);
                        }
                        "files" => {
                            if files.is_some() {
                                return Err(serde::de::Error::duplicate_field("files"));
                            }
                            files = Some(map.next_value()?);
                        }
                        "inputs" => {
                            if inputs.is_some() {
                                return Err(serde::de::Error::duplicate_field("inputs"));
                            }
                            inputs = Some(map.next_value()?);
                        }
                        "deltas" => {
                            if deltas.is_some() {
                                return Err(serde::de::Error::duplicate_field("deltas"));
                            }
                            deltas = Some(map.next_value()?);
                        }
                        _ => {
                            extra.insert(field, map.next_value()?);
                        }
                    }
                }
                Ok(MeasurementReportInput {
                    schema_version: schema_version.unwrap_or_default(),
                    schema: schema.unwrap_or_default(),
                    _tool: tool,
                    command: command.unwrap_or_default(),
                    summary: summary.unwrap_or_default(),
                    files: files.unwrap_or_default(),
                    _inputs: inputs.unwrap_or_default(),
                    _deltas: deltas.unwrap_or_default(),
                    extra,
                })
            }
        }

        deserializer.deserialize_map(MeasurementReportInputVisitor)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MeasurementFileWireInput {
    path: Option<String>,
    input: Option<InputIdentityInput>,
    rig: Box<RawValue>,
    measurements: Option<Box<RawValue>>,
    #[serde(default, deserialize_with = "deserialize_required_nullable")]
    prediction_provenance: RequiredNullable<Box<RawValue>>,
    checks: Option<Vec<Box<RawValue>>>,
}

#[derive(Debug)]
struct MeasurementFileInput {
    path: Option<String>,
    input: Option<InputIdentityInput>,
    /// V17 alone retains the strictly decoded rig needed by the shared
    /// root-motion reconstruction hook. Historical readers keep treating rig
    /// as opaque evidence, preserving their released acceptance behavior.
    rig_v17: Option<RigInfo>,
    measurements: Option<Box<RawValue>>,
    prediction_provenance: RequiredNullable<PredictionProvenanceV2>,
    checks: Option<Vec<PredictionCheckInput>>,
    legacy_prediction_provenance: RequiredNullable<PredictionProvenanceV1>,
    legacy_checks: Option<Vec<LegacyPredictionCheckInput>>,
    prediction_provenance_v3: RequiredNullable<PredictionProvenanceV3>,
    checks_v3: Option<Vec<PredictionCheckInputV3>>,
    prediction_provenance_v4: RequiredNullable<PredictionProvenanceV4>,
    checks_v4: Option<Vec<PredictionCheckInputV4>>,
    prediction_provenance_v5: RequiredNullable<PredictionProvenanceV5>,
    checks_v5: Option<Vec<PredictionCheckInputV5>>,
    prediction_provenance_v6: RequiredNullable<PredictionProvenanceV6>,
    checks_v6: Option<Vec<PredictionCheckInputV6>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyPredictionCheckWireV11 {
    check_id: String,
    selection: SelectionState,
    configuration: ConfigurationState,
    applicability: Applicability,
    evaluation: EvaluationState,
    findings: Vec<PredictionFindingInput>,
    #[serde(default)]
    evaluated_scopes: Vec<crate::evaluation::EvaluationScope>,
    #[serde(default)]
    gaps: Vec<PredictionGapInput>,
    prediction: Option<Box<RawValue>>,
}

/// The immutable V11 check attachment.  This deliberately remains a separate
/// internal shape: V11 evidence is validated with its V1 identity and staged
/// decoding rules, never converted into a V2 attachment.
#[derive(Debug)]
struct LegacyPredictionCheckInput {
    check_id: String,
    selection: SelectionState,
    configuration: ConfigurationState,
    applicability: Applicability,
    evaluation: EvaluationState,
    findings: Vec<PredictionFindingInput>,
    evaluated_scopes: Vec<crate::evaluation::EvaluationScope>,
    gaps: Vec<PredictionGapInput>,
    prediction: Option<EnginePredictionV1>,
}

#[derive(Debug, Default)]
enum RequiredNullable<T> {
    #[default]
    Missing,
    Present(Option<T>),
}

impl<T> RequiredNullable<T> {
    fn as_present(&self) -> Option<&T> {
        match self {
            Self::Missing | Self::Present(None) => None,
            Self::Present(Some(value)) => Some(value),
        }
    }
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MeasurementReportSummaryInput {
    #[serde(rename = "files")]
    _files: Option<Box<RawValue>>,
    #[serde(rename = "findings")]
    _findings: Option<Box<RawValue>>,
    #[serde(rename = "checks")]
    _checks: Option<Box<RawValue>>,
    #[serde(rename = "deltas")]
    _deltas: Option<Box<RawValue>>,
    prediction_facets: Option<PredictionFacetSummaryInput>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PredictionFacetSummaryInput {
    available: usize,
    required_prediction_unavailable: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PredictionCheckWireInput {
    check_id: String,
    selection: SelectionState,
    configuration: ConfigurationState,
    applicability: Applicability,
    evaluation: EvaluationState,
    findings: Vec<PredictionFindingInput>,
    #[serde(default)]
    evaluated_scopes: Vec<crate::evaluation::EvaluationScope>,
    #[serde(default)]
    gaps: Vec<PredictionGapInput>,
    prediction: Option<Box<RawValue>>,
}

#[derive(Debug)]
struct PredictionCheckInput {
    check_id: String,
    selection: SelectionState,
    configuration: ConfigurationState,
    applicability: Applicability,
    evaluation: EvaluationState,
    findings: Vec<PredictionFindingInput>,
    evaluated_scopes: Vec<crate::evaluation::EvaluationScope>,
    gaps: Vec<PredictionGapInput>,
    prediction: Option<EnginePredictionV2>,
}

#[derive(Debug)]
struct PredictionCheckInputV3 {
    check_id: String,
    selection: SelectionState,
    configuration: ConfigurationState,
    applicability: Applicability,
    evaluation: EvaluationState,
    findings: Vec<PredictionFindingInput>,
    evaluated_scopes: Vec<crate::evaluation::EvaluationScope>,
    gaps: Vec<PredictionGapInput>,
    prediction: Option<EnginePredictionV3>,
}

#[derive(Debug)]
struct PredictionCheckInputV4 {
    check_id: String,
    selection: SelectionState,
    configuration: ConfigurationState,
    applicability: Applicability,
    evaluation: EvaluationState,
    findings: Vec<PredictionFindingInput>,
    evaluated_scopes: Vec<crate::evaluation::EvaluationScope>,
    gaps: Vec<PredictionGapInput>,
    prediction: Option<EnginePredictionV4>,
}

#[derive(Debug)]
struct PredictionCheckInputV5 {
    check_id: String,
    selection: SelectionState,
    configuration: ConfigurationState,
    applicability: Applicability,
    evaluation: EvaluationState,
    findings: Vec<PredictionFindingInput>,
    evaluated_scopes: Vec<crate::evaluation::EvaluationScope>,
    gaps: Vec<PredictionGapInput>,
    prediction: Option<EnginePredictionV5>,
}

#[derive(Debug)]
struct PredictionCheckInputV6 {
    check_id: String,
    selection: SelectionState,
    configuration: ConfigurationState,
    applicability: Applicability,
    evaluation: EvaluationState,
    findings: Vec<PredictionFindingInput>,
    evaluated_scopes: Vec<crate::evaluation::EvaluationScope>,
    gaps: Vec<PredictionGapInput>,
    prediction: Option<EnginePredictionV6>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PredictionFindingInput {
    check_id: String,
    #[serde(rename = "severity")]
    _severity: PredictionSeverityInput,
    #[serde(rename = "clip", skip_serializing_if = "Option::is_none")]
    _clip: Option<String>,
    #[serde(rename = "bone", skip_serializing_if = "Option::is_none")]
    _bone: Option<String>,
    #[serde(rename = "node", skip_serializing_if = "Option::is_none")]
    _node: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prediction_scope: Option<crate::evaluation::EvaluationScope>,
    #[serde(rename = "time_s", skip_serializing_if = "Option::is_none")]
    _time_s: Option<f32>,
    #[serde(rename = "measured", skip_serializing_if = "Option::is_none")]
    _measured: Option<Box<RawValue>>,
    #[serde(rename = "expected", skip_serializing_if = "Option::is_none")]
    _expected: Option<Box<RawValue>>,
    #[serde(rename = "members", skip_serializing_if = "Option::is_none")]
    _members: Option<Box<RawValue>>,
    #[serde(rename = "message")]
    _message: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PredictionGapInput {
    code: String,
    #[serde(rename = "message")]
    _message: String,
    scope: Option<crate::evaluation::EvaluationScope>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PredictionSeverityInput {
    Error,
    Warning,
    Note,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct InputIdentityInput {
    sha256: Option<String>,
    bytes: Option<u64>,
}

/// Recursively preserves the historical JSON-f64-to-Rust-f32 narrowing path
/// without materializing an unbounded generic JSON value.
///
/// `serde_json` rejects a finite JSON number that exceeds `f32::MAX` when it
/// directly services `deserialize_f32`. Output-v9 readback first retained the
/// number as `f64`, then narrowed it to `f32`; semantic measurement validation
/// consequently reported the resulting infinity as a typed non-finite value.
/// This adapter retains that contract while streaming directly into the bounded
/// typed measurement DTO.
struct MeasurementF32NarrowingDeserializer<D>(D);

macro_rules! delegate_measurement_deserializer {
    ($method:ident $(, $argument:ident: $argument_type:ty)*) => {
        fn $method<V>(
            self,
            $($argument: $argument_type,)*
            visitor: V,
        ) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
        {
            self.0.$method(
                $($argument,)*
                MeasurementF32NarrowingVisitor(visitor),
            )
        }
    };
}

impl<'de, D> Deserializer<'de> for MeasurementF32NarrowingDeserializer<D>
where
    D: Deserializer<'de>,
{
    type Error = D::Error;

    delegate_measurement_deserializer!(deserialize_any);
    delegate_measurement_deserializer!(deserialize_bool);
    delegate_measurement_deserializer!(deserialize_i8);
    delegate_measurement_deserializer!(deserialize_i16);
    delegate_measurement_deserializer!(deserialize_i32);
    delegate_measurement_deserializer!(deserialize_i64);
    delegate_measurement_deserializer!(deserialize_i128);
    delegate_measurement_deserializer!(deserialize_u8);
    delegate_measurement_deserializer!(deserialize_u16);
    delegate_measurement_deserializer!(deserialize_u32);
    delegate_measurement_deserializer!(deserialize_u64);
    delegate_measurement_deserializer!(deserialize_u128);

    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.0
            .deserialize_f64(MeasurementF32NarrowingNumberVisitor(visitor))
    }

    delegate_measurement_deserializer!(deserialize_f64);
    delegate_measurement_deserializer!(deserialize_char);
    delegate_measurement_deserializer!(deserialize_str);
    delegate_measurement_deserializer!(deserialize_string);
    delegate_measurement_deserializer!(deserialize_bytes);
    delegate_measurement_deserializer!(deserialize_byte_buf);
    delegate_measurement_deserializer!(deserialize_option);
    delegate_measurement_deserializer!(deserialize_unit);
    delegate_measurement_deserializer!(deserialize_unit_struct, name: &'static str);
    delegate_measurement_deserializer!(deserialize_newtype_struct, name: &'static str);
    delegate_measurement_deserializer!(deserialize_seq);
    delegate_measurement_deserializer!(deserialize_tuple, len: usize);
    delegate_measurement_deserializer!(
        deserialize_tuple_struct,
        name: &'static str,
        len: usize
    );
    delegate_measurement_deserializer!(deserialize_map);
    delegate_measurement_deserializer!(
        deserialize_struct,
        name: &'static str,
        fields: &'static [&'static str]
    );
    delegate_measurement_deserializer!(
        deserialize_enum,
        name: &'static str,
        variants: &'static [&'static str]
    );
    delegate_measurement_deserializer!(deserialize_identifier);
    delegate_measurement_deserializer!(deserialize_ignored_any);

    fn is_human_readable(&self) -> bool {
        self.0.is_human_readable()
    }
}

struct MeasurementF32NarrowingNumberVisitor<V>(V);

impl<'de, V> Visitor<'de> for MeasurementF32NarrowingNumberVisitor<V>
where
    V: Visitor<'de>,
{
    type Value = V::Value;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.expecting(formatter)
    }

    fn visit_f32<E>(self, value: f32) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.0.visit_f32(value)
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.0.visit_f32(value as f32)
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.0.visit_f32(value as f32)
    }

    fn visit_i128<E>(self, value: i128) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.0.visit_f32(value as f32)
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.0.visit_f32(value as f32)
    }

    fn visit_u128<E>(self, value: u128) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.0.visit_f32(value as f32)
    }
}

struct MeasurementF32NarrowingVisitor<V>(V);

macro_rules! delegate_measurement_visitor {
    ($method:ident, $value_type:ty) => {
        fn $method<E>(self, value: $value_type) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            self.0.$method(value)
        }
    };
}

impl<'de, V> Visitor<'de> for MeasurementF32NarrowingVisitor<V>
where
    V: Visitor<'de>,
{
    type Value = V::Value;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.expecting(formatter)
    }

    delegate_measurement_visitor!(visit_bool, bool);
    delegate_measurement_visitor!(visit_i8, i8);
    delegate_measurement_visitor!(visit_i16, i16);
    delegate_measurement_visitor!(visit_i32, i32);
    delegate_measurement_visitor!(visit_i64, i64);
    delegate_measurement_visitor!(visit_i128, i128);
    delegate_measurement_visitor!(visit_u8, u8);
    delegate_measurement_visitor!(visit_u16, u16);
    delegate_measurement_visitor!(visit_u32, u32);
    delegate_measurement_visitor!(visit_u64, u64);
    delegate_measurement_visitor!(visit_u128, u128);
    delegate_measurement_visitor!(visit_f32, f32);
    delegate_measurement_visitor!(visit_f64, f64);
    delegate_measurement_visitor!(visit_char, char);

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.0.visit_str(value)
    }

    fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.0.visit_borrowed_str(value)
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.0.visit_string(value)
    }

    fn visit_bytes<E>(self, value: &[u8]) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.0.visit_bytes(value)
    }

    fn visit_borrowed_bytes<E>(self, value: &'de [u8]) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.0.visit_borrowed_bytes(value)
    }

    fn visit_byte_buf<E>(self, value: Vec<u8>) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.0.visit_byte_buf(value)
    }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.0.visit_none()
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        self.0
            .visit_some(MeasurementF32NarrowingDeserializer(deserializer))
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.0.visit_unit()
    }

    fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        self.0
            .visit_newtype_struct(MeasurementF32NarrowingDeserializer(deserializer))
    }

    fn visit_seq<A>(self, sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        self.0.visit_seq(MeasurementF32NarrowingSeqAccess(sequence))
    }

    fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        self.0.visit_map(MeasurementF32NarrowingMapAccess(map))
    }

    fn visit_enum<A>(self, data: A) -> Result<Self::Value, A::Error>
    where
        A: EnumAccess<'de>,
    {
        self.0.visit_enum(MeasurementF32NarrowingEnumAccess(data))
    }
}

struct MeasurementF32NarrowingSeed<S>(S);

impl<'de, S> DeserializeSeed<'de> for MeasurementF32NarrowingSeed<S>
where
    S: DeserializeSeed<'de>,
{
    type Value = S::Value;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        self.0
            .deserialize(MeasurementF32NarrowingDeserializer(deserializer))
    }
}

struct MeasurementF32NarrowingSeqAccess<A>(A);

impl<'de, A> SeqAccess<'de> for MeasurementF32NarrowingSeqAccess<A>
where
    A: SeqAccess<'de>,
{
    type Error = A::Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        self.0.next_element_seed(MeasurementF32NarrowingSeed(seed))
    }

    fn size_hint(&self) -> Option<usize> {
        self.0.size_hint()
    }
}

struct MeasurementF32NarrowingMapAccess<A>(A);

impl<'de, A> MapAccess<'de> for MeasurementF32NarrowingMapAccess<A>
where
    A: MapAccess<'de>,
{
    type Error = A::Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: DeserializeSeed<'de>,
    {
        self.0.next_key_seed(MeasurementF32NarrowingSeed(seed))
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        self.0.next_value_seed(MeasurementF32NarrowingSeed(seed))
    }

    fn size_hint(&self) -> Option<usize> {
        self.0.size_hint()
    }
}

struct MeasurementF32NarrowingEnumAccess<A>(A);

impl<'de, A> EnumAccess<'de> for MeasurementF32NarrowingEnumAccess<A>
where
    A: EnumAccess<'de>,
{
    type Error = A::Error;
    type Variant = MeasurementF32NarrowingVariantAccess<A::Variant>;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        let (value, variant) = self.0.variant_seed(MeasurementF32NarrowingSeed(seed))?;
        Ok((value, MeasurementF32NarrowingVariantAccess(variant)))
    }
}

struct MeasurementF32NarrowingVariantAccess<A>(A);

impl<'de, A> VariantAccess<'de> for MeasurementF32NarrowingVariantAccess<A>
where
    A: VariantAccess<'de>,
{
    type Error = A::Error;

    fn unit_variant(self) -> Result<(), Self::Error> {
        self.0.unit_variant()
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        self.0
            .newtype_variant_seed(MeasurementF32NarrowingSeed(seed))
    }

    fn tuple_variant<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.0
            .tuple_variant(len, MeasurementF32NarrowingVisitor(visitor))
    }

    fn struct_variant<V>(
        self,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.0
            .struct_variant(fields, MeasurementF32NarrowingVisitor(visitor))
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum SkeletonNodeMeasurementInput {
    Current(Box<crate::measure::SkeletonNodeMeasurements>),
    Earlier {
        #[serde(rename = "node_index")]
        _node_index: usize,
    },
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum SkinMeasurementInput {
    Current(Box<crate::measure::SkinMeasurements>),
    Earlier {
        #[serde(rename = "skin_index")]
        _skin_index: usize,
    },
}

#[derive(Debug, Deserialize)]
struct MeasurementPayloadInput {
    schema_version: Option<u32>,
    schema: Option<String>,
    clips: Option<BTreeMap<String, ClipMeasurements>>,
    material_resource_coverage: Option<MaterialResourceCoverage>,
    material_definitions: Option<Vec<MaterialDefinitionMeasurements>>,
    textures: Option<Vec<TextureMeasurements>>,
    images: Option<Vec<ImageMeasurements>>,
    skeleton_source_coverage: Option<SourceSkeletonCoverage>,
    skeleton_nodes: Option<Vec<SkeletonNodeMeasurementInput>>,
    skins: Option<Vec<SkinMeasurementInput>>,
    mesh_definitions: Option<Vec<crate::measure::MeshDefinitionMeasurements>>,
    node_instances: Option<Vec<crate::measure::NodeInstanceMeasurements>>,
    scenes: Option<Vec<crate::measure::SceneMeasurements>>,
    default_scene_index: Option<usize>,
}

/// Current measurement payload readback is closed at the root and at every
/// domain introduced or extended by measurements-v16. Historical readers use
/// [`MeasurementPayloadInput`] directly so their accepted JSON shape does not
/// change retroactively.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MeasurementPayloadV16Input {
    schema_version: Option<u32>,
    schema: Option<String>,
    clips: Option<BTreeMap<String, ClipMeasurements>>,
    material_resource_coverage: Option<MaterialResourceCoverage>,
    material_definitions: Option<Vec<MaterialDefinitionMeasurements>>,
    textures: Option<Vec<TextureMeasurements>>,
    images: Option<Vec<ImageMeasurementsV16Input>>,
    skeleton_source_coverage: Option<SourceSkeletonCoverage>,
    skeleton_nodes: Option<Vec<SkeletonNodeMeasurementInput>>,
    skins: Option<Vec<SkinMeasurementInput>>,
    mesh_definitions: Option<Vec<MeshDefinitionMeasurementsV16Input>>,
    node_instances: Option<Vec<NodeInstanceMeasurementsV16Input>>,
    scenes: Option<Vec<SceneMeasurementsV16Input>>,
    default_scene_index: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AabbV16Input {
    min: [f32; 3],
    max: [f32; 3],
}

impl From<AabbV16Input> for Aabb {
    fn from(value: AabbV16Input) -> Self {
        Self {
            min: value.min,
            max: value.max,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PrimitiveMeasurementsV16Input {
    primitive_index: usize,
    #[serde(deserialize_with = "deserialize_required_optional_usize")]
    material_index: Option<usize>,
    vertex_count: u64,
    finite_vertex_count: u64,
    geometry_aabb: Option<AabbV16Input>,
    geometry_centroid: Option<[f32; 3]>,
}

fn deserialize_required_optional_usize<'de, D>(deserializer: D) -> Result<Option<usize>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<usize>::deserialize(deserializer)
}

impl From<PrimitiveMeasurementsV16Input> for PrimitiveMeasurements {
    fn from(value: PrimitiveMeasurementsV16Input) -> Self {
        Self {
            primitive_index: value.primitive_index,
            material_index: value.material_index,
            vertex_count: value.vertex_count,
            finite_vertex_count: value.finite_vertex_count,
            geometry_aabb: value.geometry_aabb.map(Into::into),
            geometry_centroid: value.geometry_centroid,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MeshDefinitionMeasurementsV16Input {
    mesh_index: usize,
    name: String,
    primitives: Option<Vec<PrimitiveMeasurementsV16Input>>,
    vertex_count: u64,
    geometry_aabb: Option<AabbV16Input>,
    geometry_centroid: Option<[f32; 3]>,
    max_joints_per_vertex: u32,
    weight_sum_min: Option<f64>,
    weight_sum_max: Option<f64>,
    additional_influence_sets: Vec<AdditionalInfluenceSetMeasurements>,
}

impl From<MeshDefinitionMeasurementsV16Input> for MeshDefinitionMeasurements {
    fn from(value: MeshDefinitionMeasurementsV16Input) -> Self {
        Self {
            mesh_index: value.mesh_index,
            name: value.name,
            primitives: value
                .primitives
                .map(|primitives| primitives.into_iter().map(Into::into).collect()),
            vertex_count: value.vertex_count,
            geometry_aabb: value.geometry_aabb.map(Into::into),
            geometry_centroid: value.geometry_centroid,
            max_joints_per_vertex: value.max_joints_per_vertex,
            weight_sum_min: value.weight_sum_min,
            weight_sum_max: value.weight_sum_max,
            additional_influence_sets: value.additional_influence_sets,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ImageMeasurementsV16Input {
    image_index: usize,
    name: Option<String>,
    source_kind: crate::model::ImageSourceKind,
    declared_mime_type: Option<String>,
    detected_container: Option<crate::model::ImageContainerFormat>,
    leading_magic_hex: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    channel_count: Option<u8>,
    decoded_color_type: Option<DecodedImageColorType>,
    unavailable_reason: Option<crate::model::ImageUnavailableReason>,
}

impl From<ImageMeasurementsV16Input> for ImageMeasurements {
    fn from(value: ImageMeasurementsV16Input) -> Self {
        Self {
            image_index: value.image_index,
            name: value.name,
            source_kind: value.source_kind,
            declared_mime_type: value.declared_mime_type,
            detected_container: value.detected_container,
            leading_magic_hex: value.leading_magic_hex,
            width: value.width,
            height: value.height,
            channel_count: value.channel_count,
            decoded_color_type: value.decoded_color_type,
            unavailable_reason: value.unavailable_reason,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NodeInstanceMeasurementsV16Input {
    node_index: usize,
    node_name: String,
    mesh_index: usize,
    static_node_world_aabb: Option<AabbV16Input>,
    static_node_world_aabb_unavailable_reason: Option<StaticNodeAabbUnavailableReason>,
}

impl From<NodeInstanceMeasurementsV16Input> for NodeInstanceMeasurements {
    fn from(value: NodeInstanceMeasurementsV16Input) -> Self {
        Self {
            node_index: value.node_index,
            node_name: value.node_name,
            mesh_index: value.mesh_index,
            static_node_world_aabb: value.static_node_world_aabb.map(Into::into),
            static_node_world_aabb_unavailable_reason: value
                .static_node_world_aabb_unavailable_reason,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SceneMeasurementsV16Input {
    scene_index: usize,
    name: Option<String>,
    instance_count: usize,
    static_scene_world_aabb: Option<AabbV16Input>,
    excluded_instance_count: usize,
}

impl From<SceneMeasurementsV16Input> for SceneMeasurements {
    fn from(value: SceneMeasurementsV16Input) -> Self {
        Self {
            scene_index: value.scene_index,
            name: value.name,
            instance_count: value.instance_count,
            static_scene_world_aabb: value.static_scene_world_aabb.map(Into::into),
            excluded_instance_count: value.excluded_instance_count,
        }
    }
}

impl From<MeasurementPayloadV16Input> for MeasurementPayloadInput {
    fn from(value: MeasurementPayloadV16Input) -> Self {
        Self {
            schema_version: value.schema_version,
            schema: value.schema,
            clips: value.clips,
            material_resource_coverage: value.material_resource_coverage,
            material_definitions: value.material_definitions,
            textures: value.textures,
            images: value
                .images
                .map(|images| images.into_iter().map(Into::into).collect()),
            skeleton_source_coverage: value.skeleton_source_coverage,
            skeleton_nodes: value.skeleton_nodes,
            skins: value.skins,
            mesh_definitions: value
                .mesh_definitions
                .map(|meshes| meshes.into_iter().map(Into::into).collect()),
            node_instances: value
                .node_instances
                .map(|instances| instances.into_iter().map(Into::into).collect()),
            scenes: value
                .scenes
                .map(|scenes| scenes.into_iter().map(Into::into).collect()),
            default_scene_index: value.default_scene_index,
        }
    }
}

fn decode_measurement_payload(
    raw: &RawValue,
    strict_v16: bool,
) -> Result<MeasurementPayloadInput, serde_json::Error> {
    let mut deserializer = serde_json::Deserializer::from_str(raw.get());
    let payload = if strict_v16 {
        MeasurementPayloadV16Input::deserialize(MeasurementF32NarrowingDeserializer(
            &mut deserializer,
        ))?
        .into()
    } else {
        MeasurementPayloadInput::deserialize(MeasurementF32NarrowingDeserializer(
            &mut deserializer,
        ))?
    };
    deserializer.end()?;
    Ok(payload)
}

/// One validated file record recovered from a measurement report.
///
/// The record retains its source path and full nested measurement contract so
/// consumers can choose the clip, mesh, and cardinality policies appropriate
/// to their workflow.
#[derive(Debug, Clone)]
pub struct MeasurementReportFile {
    path: String,
    input: InputIdentity,
    measurements: MeasurementContract,
}

impl MeasurementReportFile {
    /// Source path recorded by the producing report.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Immutable identity of the source bytes used to produce this record.
    pub fn input(&self) -> &InputIdentity {
        &self.input
    }

    /// Validated nested measurement contract.
    pub fn measurements(&self) -> &MeasurementContract {
        &self.measurements
    }

    /// Consume this record and return its validated measurement contract.
    pub fn into_measurements(self) -> MeasurementContract {
        self.measurements
    }
}

/// A typed measurement-report subset failed current-contract validation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum MeasurementReportError {
    /// The outer envelope omitted its version.
    #[error("report envelope has no `schema_version`")]
    MissingOutputVersion,
    /// The outer envelope uses an unsupported version.
    #[error("has schema_version {found}; this build reads schema_version {OUTPUT_SCHEMA_VERSION}")]
    UnsupportedOutputVersion {
        /// Version found in the input.
        found: u32,
    },
    /// The outer envelope does not carry the immutable current identity.
    #[error("report envelope does not identify output contract {OUTPUT_SCHEMA_ID}")]
    WrongOutputIdentity,
    /// The outer envelope omitted its command.
    #[error("report envelope has no `command`")]
    MissingCommand,
    /// The outer envelope belongs to a command without file measurements.
    #[error("report command {command:?} does not carry measurement file records")]
    UnsupportedCommand {
        /// Command found in the input.
        command: String,
    },
    /// A current output-v11 envelope carried a field outside its closed schema.
    #[error("report envelope has unknown field `{field}`")]
    UnknownOutputField {
        /// Lexically first unknown root field.
        field: String,
    },
    /// A current output-v11 envelope omitted its producer metadata.
    #[error("report envelope has no `tool` object")]
    MissingTool,
    /// The outer envelope omitted its file array.
    #[error("report envelope has no `files` array")]
    MissingFiles,
    /// The outer envelope exceeds the immutable file-record bound.
    #[error("report contains {found} files, exceeding the output-v11 limit of {limit}")]
    TooManyFiles {
        /// Supplied file count.
        found: usize,
        /// Immutable output-v11 limit.
        limit: usize,
    },
    /// A lint report omitted the derived prediction-facet summary.
    #[error("lint report summary has no `prediction_facets` object")]
    MissingPredictionFacetSummary,
    /// A measure report carried a lint-only prediction-facet summary.
    #[error("measure report summary must not carry `prediction_facets`")]
    UnexpectedPredictionFacetSummary,
    /// Derived prediction-facet totals did not match the lint summary.
    #[error("lint report prediction-facet summary does not match its check records")]
    PredictionFacetSummaryMismatch,
    /// One file record failed validation.
    #[error("files[{file_index}] {source}")]
    File {
        /// Zero-based index of the invalid file record.
        file_index: usize,
        /// Typed record-validation failure.
        #[source]
        source: MeasurementFileError,
    },
}

/// A serialized output-v11 report could not be read within the public bound.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum MeasurementReportReadError {
    /// Reading the bounded input failed.
    #[error("cannot read report: {source}")]
    Io {
        /// Underlying bounded-reader failure.
        #[source]
        source: std::io::Error,
    },
    /// The serialized report exceeded the immutable output-v11 byte limit.
    #[error("report exceeds the output-v11 limit of {limit} bytes")]
    ReportTooLarge {
        /// Immutable maximum accepted byte count.
        limit: u64,
    },
    /// The bounded bytes were not valid JSON for the output-v11 read shape.
    #[error("invalid report JSON: {source}")]
    InvalidJson {
        /// JSON syntax or typed-shape failure.
        #[source]
        source: serde_json::Error,
    },
}

/// One measurement-report file record failed validation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum MeasurementFileError {
    /// The bounded file record could not be decoded after the outer v11
    /// identity was accepted.
    #[error("has invalid output-v11 file shape: {reason}")]
    InvalidFileShape {
        /// Stable serde diagnostic for the malformed nested record.
        reason: String,
    },
    /// The file record omitted its source path.
    #[error("has no `path`")]
    MissingPath,
    /// The file record omitted its source-byte identity.
    #[error("has no `input`")]
    MissingInput,
    /// The source-byte identity omitted its SHA-256 digest.
    #[error("input has no `sha256`")]
    MissingSha256,
    /// The source-byte identity uses a malformed SHA-256 digest.
    #[error("input `sha256` must be 64 lowercase hexadecimal characters")]
    InvalidSha256,
    /// The source-byte identity omitted its byte count.
    #[error("input has no `bytes`")]
    MissingBytes,
    /// The file record omitted its nested measurement contract.
    #[error("has no measurements")]
    MissingMeasurements,
    /// A lint file omitted its required nullable provenance field.
    #[error("has no required `prediction_provenance` field")]
    MissingPredictionProvenance,
    /// A measure file carried lint-only prediction provenance.
    #[error("measure file must not carry `prediction_provenance`")]
    UnexpectedPredictionProvenance,
    /// A lint file omitted its check array.
    #[error("lint file has no `checks` array")]
    MissingChecks,
    /// A measure file carried lint-only check records.
    #[error("measure file must not carry `checks`")]
    UnexpectedChecks,
    /// A lint file exceeded the immutable per-file check bound.
    #[error("contains {found} checks, exceeding the output-v11 limit of {limit}")]
    TooManyChecks {
        /// Supplied check count.
        found: usize,
        /// Immutable output-v11 limit.
        limit: usize,
    },
    /// File and prediction-provenance primary identities differ.
    #[error("prediction provenance primary input does not match file input")]
    PredictionPrimaryInputMismatch,
    /// File-scoped prediction provenance violated its immutable contract.
    #[error("has invalid prediction provenance: {source}")]
    InvalidPredictionProvenance {
        /// Typed nested provenance failure.
        #[source]
        source: PredictionContractError,
    },
    /// The serialized provenance object could not be decoded as the strict V1 wire.
    #[error("has invalid prediction provenance shape: {reason}")]
    InvalidPredictionProvenanceShape {
        /// Stable serde diagnostic for the malformed nested object.
        reason: String,
    },
    /// One check carried a prediction without file provenance.
    #[error("checks[{check_index}] has prediction without non-null file provenance")]
    PredictionWithoutProvenance {
        /// Zero-based check index.
        check_index: usize,
    },
    /// One check's prediction evidence violated its immutable contract.
    #[error("checks[{check_index}] has invalid prediction evidence: {source}")]
    InvalidPrediction {
        /// Zero-based check index.
        check_index: usize,
        /// Typed nested prediction failure.
        #[source]
        source: PredictionContractError,
    },
    /// One serialized check or prediction object could not be decoded as the strict V1 wire.
    #[error("checks[{check_index}] has invalid prediction shape: {reason}")]
    InvalidPredictionShape {
        /// Zero-based check index.
        check_index: usize,
        /// Stable serde diagnostic for the malformed nested object.
        reason: String,
    },
    /// One check's prediction attachment contradicts the sole check lifecycle.
    #[error("checks[{check_index}] has invalid prediction lifecycle: {reason}")]
    InvalidPredictionLifecycle {
        /// Zero-based check index.
        check_index: usize,
        /// Stable relationship failure.
        reason: &'static str,
    },
    /// Aggregate prediction facets exceeded the per-file V1 bound.
    #[error("contains {found} prediction facets, exceeding the V1 limit of {limit}")]
    TooManyPredictionFacets {
        /// Supplied facet count.
        found: usize,
        /// Immutable V1 limit.
        limit: usize,
    },
    /// A decoded V2 budget summary did not coincide with an exhausted shared
    /// file facet budget.
    #[error("facet-budget summary requires exactly {limit} aggregate facets, found {found}")]
    FacetBudgetSummaryWithoutExhaustedFileBudget {
        /// Aggregate facet count.
        found: usize,
        /// Immutable shared file limit.
        limit: usize,
    },
    /// Aggregate prediction basis rows exceeded the per-file V1 bound.
    #[error("contains {found} prediction basis rows, exceeding the V1 limit of {limit}")]
    TooManyPredictionBasisReferences {
        /// Supplied basis-row count.
        found: usize,
        /// Immutable V1 limit.
        limit: usize,
    },
    /// Aggregate prediction/provenance retained text exceeded the per-file bound.
    #[error("retains {found} prediction text bytes, exceeding the V1 limit of {limit}")]
    TooMuchPredictionText {
        /// Supplied UTF-8 byte count.
        found: usize,
        /// Immutable V1 limit.
        limit: usize,
    },
    /// Checked prediction accounting overflowed.
    #[error("prediction bound accounting overflowed")]
    PredictionAccountingOverflow,
    /// The nested measurement contract omitted its version.
    #[error("has no versioned measurement contract")]
    MissingMeasurementVersion,
    /// The nested measurement contract uses an unsupported version.
    #[error(
        "has measurement schema_version {found}; this build reads measurement schema_version {MEASUREMENTS_SCHEMA_VERSION}"
    )]
    UnsupportedMeasurementVersion {
        /// Version found in the nested contract.
        found: u32,
    },
    /// The nested contract does not carry the immutable measurement identity.
    #[error("does not identify measurement contract {MEASUREMENTS_SCHEMA_ID}")]
    WrongMeasurementIdentity,
    /// The nested contract omitted its clip-measurement map.
    #[error("measurement contract has no `clips` map")]
    MissingClips,
    /// The nested contract omitted material resource coverage.
    #[error("measurement contract has no `material_resource_coverage`")]
    MissingMaterialResourceCoverage,
    /// The nested contract omitted its material definition array.
    #[error("measurement contract has no `material_definitions` array")]
    MissingMaterialDefinitions,
    /// The nested contract omitted its texture array.
    #[error("measurement contract has no `textures` array")]
    MissingTextures,
    /// The nested contract omitted its image array.
    #[error("measurement contract has no `images` array")]
    MissingImages,
    /// The nested contract omitted skeleton source coverage.
    #[error("measurement contract has no `skeleton_source_coverage`")]
    MissingSkeletonSourceCoverage,
    /// The nested contract omitted its source skeleton-node array.
    #[error("measurement contract has no `skeleton_nodes` array")]
    MissingSkeletonNodes,
    /// The nested contract omitted its source skin array.
    #[error("measurement contract has no `skins` array")]
    MissingSkins,
    /// The nested contract omitted its mesh-definition array.
    #[error("measurement contract has no `mesh_definitions` array")]
    MissingMeshDefinitions,
    /// The nested contract omitted its node-instance array.
    #[error("measurement contract has no `node_instances` array")]
    MissingNodeInstances,
    /// The nested contract omitted its scene array.
    #[error("measurement contract has no `scenes` array")]
    MissingScenes,
    /// The nested measurements object could not be decoded after prediction-independent
    /// validation completed.
    #[error("has invalid measurements shape: {reason}")]
    InvalidMeasurementsShape {
        /// Stable serde diagnostic for the malformed nested measurements.
        reason: String,
    },
    /// The nested measurement values do not satisfy the current contract.
    #[error("has invalid measurements: {source}")]
    InvalidMeasurements {
        /// Measurement validation failure.
        #[source]
        source: MeasurementContractError,
    },
}

impl MeasurementReportError {
    /// Zero-based file index for an error in one report record.
    ///
    /// Envelope-level errors return `None`.
    pub fn file_index(&self) -> Option<usize> {
        match self {
            Self::File { file_index, .. } => Some(*file_index),
            _ => None,
        }
    }

    fn file(file_index: usize, source: MeasurementFileError) -> Self {
        Self::File { file_index, source }
    }
}

fn prediction_file_error(
    file_index: usize,
    source: MeasurementFileError,
) -> MeasurementReportError {
    MeasurementReportError::file(file_index, source)
}

fn decode_prediction_phase_file(
    command: &str,
    file_index: usize,
    raw: &RawValue,
    expected_measurement_schema: &'static str,
) -> Result<MeasurementFileInput, MeasurementReportError> {
    let wire: MeasurementFileWireInput = serde_json::from_str(raw.get()).map_err(|source| {
        prediction_file_error(
            file_index,
            MeasurementFileError::InvalidFileShape {
                reason: source.to_string(),
            },
        )
    })?;

    if command == "measure" {
        if !matches!(wire.prediction_provenance, RequiredNullable::Missing) {
            return Err(prediction_file_error(
                file_index,
                MeasurementFileError::UnexpectedPredictionProvenance,
            ));
        }
        if wire.checks.is_some() {
            return Err(prediction_file_error(
                file_index,
                MeasurementFileError::UnexpectedChecks,
            ));
        }
        return Ok(MeasurementFileInput {
            path: wire.path,
            input: wire.input,
            rig_v17: None,
            measurements: wire.measurements,
            prediction_provenance: RequiredNullable::Missing,
            checks: None,
            legacy_prediction_provenance: RequiredNullable::Missing,
            legacy_checks: None,
            prediction_provenance_v3: RequiredNullable::Missing,
            checks_v3: None,
            prediction_provenance_v4: RequiredNullable::Missing,
            checks_v4: None,
            prediction_provenance_v5: RequiredNullable::Missing,
            checks_v5: None,
            prediction_provenance_v6: RequiredNullable::Missing,
            checks_v6: None,
        });
    }

    if wire
        .checks
        .as_ref()
        .is_some_and(|checks| checks.len() > OUTPUT_V11_MAX_CHECKS_PER_FILE)
    {
        return Err(prediction_file_error(
            file_index,
            MeasurementFileError::TooManyChecks {
                found: wire.checks.as_ref().map_or(0, Vec::len),
                limit: OUTPUT_V11_MAX_CHECKS_PER_FILE,
            },
        ));
    }

    if matches!(wire.prediction_provenance, RequiredNullable::Missing) {
        return Err(prediction_file_error(
            file_index,
            MeasurementFileError::MissingPredictionProvenance,
        ));
    }

    let prediction_provenance = match wire.prediction_provenance {
        RequiredNullable::Missing => unreachable!("missing provenance was rejected above"),
        RequiredNullable::Present(None) => RequiredNullable::Present(None),
        RequiredNullable::Present(Some(raw)) => {
            let provenance = decode_prediction_provenance_v2_with_measurement_schema(
                raw.get(),
                expected_measurement_schema,
            )
            .map_err(|error| {
                let source = match error {
                    PredictionDecodeError::Shape(source) => {
                        MeasurementFileError::InvalidPredictionProvenanceShape {
                            reason: source.to_string(),
                        }
                    }
                    PredictionDecodeError::Semantic(source) => {
                        MeasurementFileError::InvalidPredictionProvenance { source }
                    }
                    PredictionDecodeError::TooManyFileFacets
                    | PredictionDecodeError::TooManyFileBasisReferences => {
                        unreachable!("provenance never consumes prediction budgets")
                    }
                };
                prediction_file_error(file_index, source)
            })?;
            RequiredNullable::Present(Some(provenance))
        }
    };
    let mut decoded_facets = 0usize;
    let mut decoded_references = 0usize;
    let mut has_facet_budget_summary = false;
    let mut decoded_text = match &prediction_provenance {
        RequiredNullable::Present(Some(provenance)) => {
            provenance.retained_text_bytes().map_err(|source| {
                prediction_file_error(
                    file_index,
                    MeasurementFileError::InvalidPredictionProvenance { source },
                )
            })?
        }
        RequiredNullable::Missing | RequiredNullable::Present(None) => 0,
    };
    let provenance_for_checks = match &prediction_provenance {
        RequiredNullable::Present(provenance) => provenance.as_ref(),
        RequiredNullable::Missing => unreachable!("missing provenance was rejected above"),
    };
    let checks = wire
        .checks
        .map(|raw_checks| {
            let mut checks = Vec::with_capacity(raw_checks.len());
            for (check_index, raw) in raw_checks.into_iter().enumerate() {
                let wire: PredictionCheckWireInput =
                    serde_json::from_str(raw.get()).map_err(|source| {
                        prediction_file_error(
                            file_index,
                            MeasurementFileError::InvalidPredictionShape {
                                check_index,
                                reason: source.to_string(),
                            },
                        )
                    })?;
                if provenance_for_checks.is_none() && wire.prediction.is_some() {
                    return Err(prediction_file_error(
                        file_index,
                        MeasurementFileError::PredictionWithoutProvenance { check_index },
                    ));
                }
                if (wire.selection == SelectionState::Unselected
                    || wire.configuration == ConfigurationState::Disabled
                    || wire.applicability == Applicability::NotApplicable)
                    && wire.prediction.is_some()
                {
                    return Err(prediction_file_error(
                        file_index,
                        MeasurementFileError::InvalidPredictionLifecycle {
                            check_index,
                            reason: "inactive check must have empty output",
                        },
                    ));
                }
                // Allocate the remaining aggregate facet/reference budgets
                // before parsing this attachment.  This prevents an over-limit
                // later prediction from retaining a prefix whose findings can
                // no longer be represented by the file contract.
                let prediction = wire
                    .prediction
                    .map(|raw| {
                        let facet_limit =
                            PREDICTION_V1_MAX_FACETS_PER_FILE.saturating_sub(decoded_facets);
                        let reference_limit = PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE
                            .saturating_sub(decoded_references);
                        decode_engine_prediction_v2_with_measurement_schema(
                            raw.get(),
                            facet_limit,
                            reference_limit,
                            expected_measurement_schema,
                        )
                        .map_err(|error| {
                            let source = match error {
                                PredictionDecodeError::Shape(source) => {
                                    MeasurementFileError::InvalidPredictionShape {
                                        check_index,
                                        reason: source.to_string(),
                                    }
                                }
                                PredictionDecodeError::Semantic(source) => {
                                    MeasurementFileError::InvalidPrediction {
                                        check_index,
                                        source,
                                    }
                                }
                                PredictionDecodeError::TooManyFileFacets => {
                                    MeasurementFileError::TooManyPredictionFacets {
                                        found: PREDICTION_V1_MAX_FACETS_PER_FILE + 1,
                                        limit: PREDICTION_V1_MAX_FACETS_PER_FILE,
                                    }
                                }
                                PredictionDecodeError::TooManyFileBasisReferences => {
                                    MeasurementFileError::TooManyPredictionBasisReferences {
                                        found: PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE + 1,
                                        limit: PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE,
                                    }
                                }
                            };
                            prediction_file_error(file_index, source)
                        })
                    })
                    .transpose()?;
                let check = PredictionCheckInput {
                    check_id: wire.check_id,
                    selection: wire.selection,
                    configuration: wire.configuration,
                    applicability: wire.applicability,
                    evaluation: wire.evaluation,
                    findings: wire.findings,
                    evaluated_scopes: wire.evaluated_scopes,
                    gaps: wire.gaps,
                    prediction,
                };
                check
                    .validate(
                        check_index,
                        provenance_for_checks,
                        expected_measurement_schema,
                    )
                    .map_err(|source| prediction_file_error(file_index, source))?;
                if let Some(prediction) = &check.prediction {
                    has_facet_budget_summary |= prediction.has_facet_budget_summary();
                    decoded_facets = decoded_facets
                        .checked_add(prediction.facets().len())
                        .ok_or_else(|| {
                            prediction_file_error(
                                file_index,
                                MeasurementFileError::PredictionAccountingOverflow,
                            )
                        })?;
                    if decoded_facets > PREDICTION_V1_MAX_FACETS_PER_FILE {
                        return Err(prediction_file_error(
                            file_index,
                            MeasurementFileError::TooManyPredictionFacets {
                                found: decoded_facets,
                                limit: PREDICTION_V1_MAX_FACETS_PER_FILE,
                            },
                        ));
                    }
                    decoded_references = decoded_references
                        .checked_add(prediction.basis_reference_count())
                        .ok_or_else(|| {
                            prediction_file_error(
                                file_index,
                                MeasurementFileError::PredictionAccountingOverflow,
                            )
                        })?;
                    if decoded_references > PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE {
                        return Err(prediction_file_error(
                            file_index,
                            MeasurementFileError::TooManyPredictionBasisReferences {
                                found: decoded_references,
                                limit: PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE,
                            },
                        ));
                    }
                    decoded_text = decoded_text
                        .checked_add(prediction.retained_text_bytes().map_err(|source| {
                            prediction_file_error(
                                file_index,
                                MeasurementFileError::InvalidPrediction {
                                    check_index,
                                    source,
                                },
                            )
                        })?)
                        .ok_or_else(|| {
                            prediction_file_error(
                                file_index,
                                MeasurementFileError::PredictionAccountingOverflow,
                            )
                        })?;
                    if decoded_text > PREDICTION_V1_MAX_TOTAL_TEXT_BYTES_PER_FILE {
                        return Err(prediction_file_error(
                            file_index,
                            MeasurementFileError::TooMuchPredictionText {
                                found: decoded_text,
                                limit: PREDICTION_V1_MAX_TOTAL_TEXT_BYTES_PER_FILE,
                            },
                        ));
                    }
                }
                checks.push(check);
            }
            if has_facet_budget_summary && decoded_facets != PREDICTION_V1_MAX_FACETS_PER_FILE {
                return Err(prediction_file_error(
                    file_index,
                    MeasurementFileError::FacetBudgetSummaryWithoutExhaustedFileBudget {
                        found: decoded_facets,
                        limit: PREDICTION_V1_MAX_FACETS_PER_FILE,
                    },
                ));
            }
            Ok(checks)
        })
        .transpose()?;
    Ok(MeasurementFileInput {
        path: wire.path,
        input: wire.input,
        rig_v17: None,
        measurements: wire.measurements,
        prediction_provenance,
        checks,
        legacy_prediction_provenance: RequiredNullable::Missing,
        legacy_checks: None,
        prediction_provenance_v3: RequiredNullable::Missing,
        checks_v3: None,
        prediction_provenance_v4: RequiredNullable::Missing,
        checks_v4: None,
        prediction_provenance_v5: RequiredNullable::Missing,
        checks_v5: None,
        prediction_provenance_v6: RequiredNullable::Missing,
        checks_v6: None,
    })
}

fn decode_prediction_phase_file_v14(
    command: &str,
    file_index: usize,
    raw: &RawValue,
) -> Result<MeasurementFileInput, MeasurementReportError> {
    let wire: MeasurementFileWireInput = serde_json::from_str(raw.get()).map_err(|source| {
        prediction_file_error(
            file_index,
            MeasurementFileError::InvalidFileShape {
                reason: source.to_string(),
            },
        )
    })?;
    if command == "measure" {
        if !matches!(wire.prediction_provenance, RequiredNullable::Missing) {
            return Err(prediction_file_error(
                file_index,
                MeasurementFileError::UnexpectedPredictionProvenance,
            ));
        }
        if wire.checks.is_some() {
            return Err(prediction_file_error(
                file_index,
                MeasurementFileError::UnexpectedChecks,
            ));
        }
        return Ok(MeasurementFileInput {
            path: wire.path,
            input: wire.input,
            rig_v17: None,
            measurements: wire.measurements,
            prediction_provenance: RequiredNullable::Missing,
            checks: None,
            legacy_prediction_provenance: RequiredNullable::Missing,
            legacy_checks: None,
            prediction_provenance_v3: RequiredNullable::Missing,
            checks_v3: None,
            prediction_provenance_v4: RequiredNullable::Missing,
            checks_v4: None,
            prediction_provenance_v5: RequiredNullable::Missing,
            checks_v5: None,
            prediction_provenance_v6: RequiredNullable::Missing,
            checks_v6: None,
        });
    }
    if wire
        .checks
        .as_ref()
        .is_some_and(|checks| checks.len() > OUTPUT_V11_MAX_CHECKS_PER_FILE)
    {
        return Err(prediction_file_error(
            file_index,
            MeasurementFileError::TooManyChecks {
                found: wire.checks.as_ref().map_or(0, Vec::len),
                limit: OUTPUT_V11_MAX_CHECKS_PER_FILE,
            },
        ));
    }
    if matches!(wire.prediction_provenance, RequiredNullable::Missing) {
        return Err(prediction_file_error(
            file_index,
            MeasurementFileError::MissingPredictionProvenance,
        ));
    }
    let prediction_provenance_v3 = match wire.prediction_provenance {
        RequiredNullable::Missing => unreachable!("missing provenance was rejected above"),
        RequiredNullable::Present(None) => RequiredNullable::Present(None),
        RequiredNullable::Present(Some(raw)) => {
            let provenance = decode_prediction_provenance_v3(raw.get()).map_err(|error| {
                let source = match error {
                    PredictionDecodeError::Shape(source) => {
                        MeasurementFileError::InvalidPredictionProvenanceShape {
                            reason: source.to_string(),
                        }
                    }
                    PredictionDecodeError::Semantic(source) => {
                        MeasurementFileError::InvalidPredictionProvenance { source }
                    }
                    PredictionDecodeError::TooManyFileFacets
                    | PredictionDecodeError::TooManyFileBasisReferences => {
                        unreachable!("provenance never consumes prediction budgets")
                    }
                };
                prediction_file_error(file_index, source)
            })?;
            RequiredNullable::Present(Some(provenance))
        }
    };
    let provenance_for_checks = match &prediction_provenance_v3 {
        RequiredNullable::Present(provenance) => provenance.as_ref(),
        RequiredNullable::Missing => unreachable!("missing provenance was rejected above"),
    };
    let mut decoded_facets = 0usize;
    let mut decoded_references = 0usize;
    let mut has_facet_budget_summary = false;
    let mut decoded_text = provenance_for_checks
        .map(PredictionProvenanceV3::retained_text_bytes)
        .transpose()
        .map_err(|source| {
            prediction_file_error(
                file_index,
                MeasurementFileError::InvalidPredictionProvenance { source },
            )
        })?
        .unwrap_or(0);
    let checks_v3 = wire
        .checks
        .map(|raw_checks| {
            let mut checks = Vec::with_capacity(raw_checks.len());
            for (check_index, raw) in raw_checks.into_iter().enumerate() {
                let wire: PredictionCheckWireInput =
                    serde_json::from_str(raw.get()).map_err(|source| {
                        prediction_file_error(
                            file_index,
                            MeasurementFileError::InvalidPredictionShape {
                                check_index,
                                reason: source.to_string(),
                            },
                        )
                    })?;
                if provenance_for_checks.is_none() && wire.prediction.is_some() {
                    return Err(prediction_file_error(
                        file_index,
                        MeasurementFileError::PredictionWithoutProvenance { check_index },
                    ));
                }
                if (wire.selection == SelectionState::Unselected
                    || wire.configuration == ConfigurationState::Disabled
                    || wire.applicability == Applicability::NotApplicable)
                    && wire.prediction.is_some()
                {
                    return Err(prediction_file_error(
                        file_index,
                        MeasurementFileError::InvalidPredictionLifecycle {
                            check_index,
                            reason: "inactive check must have empty output",
                        },
                    ));
                }
                let prediction = wire
                    .prediction
                    .map(|raw| {
                        decode_engine_prediction_v3(
                            raw.get(),
                            PREDICTION_V1_MAX_FACETS_PER_FILE.saturating_sub(decoded_facets),
                            PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE
                                .saturating_sub(decoded_references),
                        )
                        .map_err(|error| {
                            let source = match error {
                                PredictionDecodeError::Shape(source) => {
                                    MeasurementFileError::InvalidPredictionShape {
                                        check_index,
                                        reason: source.to_string(),
                                    }
                                }
                                PredictionDecodeError::Semantic(source) => {
                                    MeasurementFileError::InvalidPrediction {
                                        check_index,
                                        source,
                                    }
                                }
                                PredictionDecodeError::TooManyFileFacets => {
                                    MeasurementFileError::TooManyPredictionFacets {
                                        found: PREDICTION_V1_MAX_FACETS_PER_FILE + 1,
                                        limit: PREDICTION_V1_MAX_FACETS_PER_FILE,
                                    }
                                }
                                PredictionDecodeError::TooManyFileBasisReferences => {
                                    MeasurementFileError::TooManyPredictionBasisReferences {
                                        found: PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE + 1,
                                        limit: PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE,
                                    }
                                }
                            };
                            prediction_file_error(file_index, source)
                        })
                    })
                    .transpose()?;
                let check = PredictionCheckInputV3 {
                    check_id: wire.check_id,
                    selection: wire.selection,
                    configuration: wire.configuration,
                    applicability: wire.applicability,
                    evaluation: wire.evaluation,
                    findings: wire.findings,
                    evaluated_scopes: wire.evaluated_scopes,
                    gaps: wire.gaps,
                    prediction,
                };
                check
                    .validate(check_index, provenance_for_checks)
                    .map_err(|source| prediction_file_error(file_index, source))?;
                if let Some(prediction) = &check.prediction {
                    has_facet_budget_summary |= prediction.has_facet_budget_summary();
                    decoded_facets = decoded_facets
                        .checked_add(prediction.facets().len())
                        .ok_or_else(|| {
                            prediction_file_error(
                                file_index,
                                MeasurementFileError::PredictionAccountingOverflow,
                            )
                        })?;
                    decoded_references = decoded_references
                        .checked_add(prediction.basis_reference_count())
                        .ok_or_else(|| {
                            prediction_file_error(
                                file_index,
                                MeasurementFileError::PredictionAccountingOverflow,
                            )
                        })?;
                    decoded_text = decoded_text
                        .checked_add(prediction.retained_text_bytes().map_err(|source| {
                            prediction_file_error(
                                file_index,
                                MeasurementFileError::InvalidPrediction {
                                    check_index,
                                    source,
                                },
                            )
                        })?)
                        .ok_or_else(|| {
                            prediction_file_error(
                                file_index,
                                MeasurementFileError::PredictionAccountingOverflow,
                            )
                        })?;
                    if decoded_text > PREDICTION_V1_MAX_TOTAL_TEXT_BYTES_PER_FILE {
                        return Err(prediction_file_error(
                            file_index,
                            MeasurementFileError::TooMuchPredictionText {
                                found: decoded_text,
                                limit: PREDICTION_V1_MAX_TOTAL_TEXT_BYTES_PER_FILE,
                            },
                        ));
                    }
                }
                checks.push(check);
            }
            if has_facet_budget_summary && decoded_facets != PREDICTION_V1_MAX_FACETS_PER_FILE {
                return Err(prediction_file_error(
                    file_index,
                    MeasurementFileError::FacetBudgetSummaryWithoutExhaustedFileBudget {
                        found: decoded_facets,
                        limit: PREDICTION_V1_MAX_FACETS_PER_FILE,
                    },
                ));
            }
            Ok(checks)
        })
        .transpose()?;
    Ok(MeasurementFileInput {
        path: wire.path,
        input: wire.input,
        rig_v17: None,
        measurements: wire.measurements,
        prediction_provenance: RequiredNullable::Missing,
        checks: None,
        legacy_prediction_provenance: RequiredNullable::Missing,
        legacy_checks: None,
        prediction_provenance_v3,
        checks_v3,
        prediction_provenance_v4: RequiredNullable::Missing,
        checks_v4: None,
        prediction_provenance_v5: RequiredNullable::Missing,
        checks_v5: None,
        prediction_provenance_v6: RequiredNullable::Missing,
        checks_v6: None,
    })
}

fn decode_prediction_phase_file_v15(
    command: &str,
    file_index: usize,
    raw: &RawValue,
) -> Result<MeasurementFileInput, MeasurementReportError> {
    #[derive(Deserialize)]
    struct SchemaProbe {
        schema: String,
    }

    let probe: MeasurementFileWireInput = serde_json::from_str(raw.get()).map_err(|source| {
        prediction_file_error(
            file_index,
            MeasurementFileError::InvalidFileShape {
                reason: source.to_string(),
            },
        )
    })?;
    if command == "measure"
        || matches!(probe.prediction_provenance, RequiredNullable::Present(None))
    {
        return decode_prediction_phase_file_v14(command, file_index, raw);
    }
    let RequiredNullable::Present(Some(provenance_raw)) = &probe.prediction_provenance else {
        return decode_prediction_phase_file_v14(command, file_index, raw);
    };
    let schema = serde_json::from_str::<SchemaProbe>(provenance_raw.get()).map_err(|source| {
        prediction_file_error(
            file_index,
            MeasurementFileError::InvalidPredictionProvenanceShape {
                reason: source.to_string(),
            },
        )
    })?;
    if schema.schema == crate::prediction::PREDICTION_PROVENANCE_V3_ID {
        return decode_prediction_phase_file_v14(command, file_index, raw);
    }
    if schema.schema != crate::prediction::PREDICTION_PROVENANCE_V4_ID {
        return Err(prediction_file_error(
            file_index,
            MeasurementFileError::InvalidPredictionProvenance {
                source: PredictionContractError::InvalidSchema {
                    field: "prediction provenance.schema",
                    expected: crate::prediction::PREDICTION_PROVENANCE_V4_ID,
                    found: schema.schema,
                },
            },
        ));
    }

    let MeasurementFileWireInput {
        path,
        input,
        measurements,
        prediction_provenance,
        checks,
        ..
    } = probe;
    if checks
        .as_ref()
        .is_some_and(|checks| checks.len() > OUTPUT_V11_MAX_CHECKS_PER_FILE)
    {
        return Err(prediction_file_error(
            file_index,
            MeasurementFileError::TooManyChecks {
                found: checks.as_ref().map_or(0, Vec::len),
                limit: OUTPUT_V11_MAX_CHECKS_PER_FILE,
            },
        ));
    }
    let RequiredNullable::Present(Some(provenance_raw)) = prediction_provenance else {
        unreachable!("V4 provenance was probed above")
    };
    let provenance = decode_prediction_provenance_v4(provenance_raw.get()).map_err(|error| {
        let source = match error {
            PredictionDecodeError::Shape(source) => {
                MeasurementFileError::InvalidPredictionProvenanceShape {
                    reason: source.to_string(),
                }
            }
            PredictionDecodeError::Semantic(source) => {
                MeasurementFileError::InvalidPredictionProvenance { source }
            }
            PredictionDecodeError::TooManyFileFacets
            | PredictionDecodeError::TooManyFileBasisReferences => unreachable!(),
        };
        prediction_file_error(file_index, source)
    })?;
    let mut decoded_facets = 0usize;
    let mut decoded_references = 0usize;
    let mut decoded_text = provenance.retained_text_bytes().map_err(|source| {
        prediction_file_error(
            file_index,
            MeasurementFileError::InvalidPredictionProvenance { source },
        )
    })?;
    let checks_v4 = checks
        .map(|raw_checks| {
            let mut decoded = Vec::with_capacity(raw_checks.len());
            for (check_index, raw_check) in raw_checks.into_iter().enumerate() {
                let wire: PredictionCheckWireInput = serde_json::from_str(raw_check.get())
                    .map_err(|source| {
                        prediction_file_error(
                            file_index,
                            MeasurementFileError::InvalidPredictionShape {
                                check_index,
                                reason: source.to_string(),
                            },
                        )
                    })?;
                if (wire.selection == SelectionState::Unselected
                    || wire.configuration == ConfigurationState::Disabled
                    || wire.applicability == Applicability::NotApplicable)
                    && wire.prediction.is_some()
                {
                    return Err(prediction_file_error(
                        file_index,
                        MeasurementFileError::InvalidPredictionLifecycle {
                            check_index,
                            reason: "inactive check must have empty output",
                        },
                    ));
                }
                let prediction = wire
                    .prediction
                    .map(|prediction_raw| {
                        decode_engine_prediction_v4(
                            prediction_raw.get(),
                            PREDICTION_V1_MAX_FACETS_PER_FILE.saturating_sub(decoded_facets),
                            PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE
                                .saturating_sub(decoded_references),
                        )
                        .map_err(|error| {
                            let source = match error {
                                PredictionDecodeError::Shape(source) => {
                                    MeasurementFileError::InvalidPredictionShape {
                                        check_index,
                                        reason: source.to_string(),
                                    }
                                }
                                PredictionDecodeError::Semantic(source) => {
                                    MeasurementFileError::InvalidPrediction {
                                        check_index,
                                        source,
                                    }
                                }
                                PredictionDecodeError::TooManyFileFacets => {
                                    MeasurementFileError::TooManyPredictionFacets {
                                        found: PREDICTION_V1_MAX_FACETS_PER_FILE + 1,
                                        limit: PREDICTION_V1_MAX_FACETS_PER_FILE,
                                    }
                                }
                                PredictionDecodeError::TooManyFileBasisReferences => {
                                    MeasurementFileError::TooManyPredictionBasisReferences {
                                        found: PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE + 1,
                                        limit: PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE,
                                    }
                                }
                            };
                            prediction_file_error(file_index, source)
                        })
                    })
                    .transpose()?;
                let check = PredictionCheckInputV4 {
                    check_id: wire.check_id,
                    selection: wire.selection,
                    configuration: wire.configuration,
                    applicability: wire.applicability,
                    evaluation: wire.evaluation,
                    findings: wire.findings,
                    evaluated_scopes: wire.evaluated_scopes,
                    gaps: wire.gaps,
                    prediction,
                };
                check
                    .validate(check_index, Some(&provenance))
                    .map_err(|source| prediction_file_error(file_index, source))?;
                if let Some(prediction) = &check.prediction {
                    decoded_facets = decoded_facets
                        .checked_add(prediction.facets().len())
                        .ok_or_else(|| {
                            prediction_file_error(
                                file_index,
                                MeasurementFileError::PredictionAccountingOverflow,
                            )
                        })?;
                    decoded_references = decoded_references
                        .checked_add(prediction.basis_reference_count())
                        .ok_or_else(|| {
                            prediction_file_error(
                                file_index,
                                MeasurementFileError::PredictionAccountingOverflow,
                            )
                        })?;
                    decoded_text = decoded_text
                        .checked_add(prediction.retained_text_bytes().map_err(|source| {
                            prediction_file_error(
                                file_index,
                                MeasurementFileError::InvalidPrediction {
                                    check_index,
                                    source,
                                },
                            )
                        })?)
                        .ok_or_else(|| {
                            prediction_file_error(
                                file_index,
                                MeasurementFileError::PredictionAccountingOverflow,
                            )
                        })?;
                    if decoded_text > PREDICTION_V1_MAX_TOTAL_TEXT_BYTES_PER_FILE {
                        return Err(prediction_file_error(
                            file_index,
                            MeasurementFileError::TooMuchPredictionText {
                                found: decoded_text,
                                limit: PREDICTION_V1_MAX_TOTAL_TEXT_BYTES_PER_FILE,
                            },
                        ));
                    }
                }
                decoded.push(check);
            }
            Ok(decoded)
        })
        .transpose()?;
    Ok(MeasurementFileInput {
        path,
        input,
        rig_v17: None,
        measurements,
        prediction_provenance: RequiredNullable::Missing,
        checks: None,
        legacy_prediction_provenance: RequiredNullable::Missing,
        legacy_checks: None,
        prediction_provenance_v3: RequiredNullable::Missing,
        checks_v3: None,
        prediction_provenance_v4: RequiredNullable::Present(Some(provenance)),
        checks_v4,
        prediction_provenance_v5: RequiredNullable::Missing,
        checks_v5: None,
        prediction_provenance_v6: RequiredNullable::Missing,
        checks_v6: None,
    })
}

fn decode_prediction_phase_file_v16(
    command: &str,
    file_index: usize,
    raw: &RawValue,
) -> Result<MeasurementFileInput, MeasurementReportError> {
    #[derive(Deserialize)]
    struct SchemaProbe {
        schema: String,
    }

    let probe: MeasurementFileWireInput = serde_json::from_str(raw.get()).map_err(|source| {
        prediction_file_error(
            file_index,
            MeasurementFileError::InvalidFileShape {
                reason: source.to_string(),
            },
        )
    })?;
    if command == "measure"
        || matches!(probe.prediction_provenance, RequiredNullable::Present(None))
    {
        return decode_prediction_phase_file_v15(command, file_index, raw);
    }
    let RequiredNullable::Present(Some(provenance_raw)) = &probe.prediction_provenance else {
        return decode_prediction_phase_file_v15(command, file_index, raw);
    };
    let schema = serde_json::from_str::<SchemaProbe>(provenance_raw.get()).map_err(|source| {
        prediction_file_error(
            file_index,
            MeasurementFileError::InvalidPredictionProvenanceShape {
                reason: source.to_string(),
            },
        )
    })?;
    if schema.schema == crate::prediction::PREDICTION_PROVENANCE_V3_ID {
        return decode_prediction_phase_file_v15(command, file_index, raw);
    }
    if schema.schema != crate::prediction::PREDICTION_PROVENANCE_V5_ID {
        return Err(prediction_file_error(
            file_index,
            MeasurementFileError::InvalidPredictionProvenance {
                source: PredictionContractError::InvalidSchema {
                    field: "prediction provenance.schema",
                    expected: crate::prediction::PREDICTION_PROVENANCE_V5_ID,
                    found: schema.schema,
                },
            },
        ));
    }
    let MeasurementFileWireInput {
        path,
        input,
        measurements,
        prediction_provenance,
        checks,
        ..
    } = probe;
    if checks
        .as_ref()
        .is_some_and(|checks| checks.len() > OUTPUT_V11_MAX_CHECKS_PER_FILE)
    {
        return Err(prediction_file_error(
            file_index,
            MeasurementFileError::TooManyChecks {
                found: checks.as_ref().map_or(0, Vec::len),
                limit: OUTPUT_V11_MAX_CHECKS_PER_FILE,
            },
        ));
    }
    let RequiredNullable::Present(Some(raw_provenance)) = prediction_provenance else {
        unreachable!()
    };
    let provenance: PredictionProvenanceV5 =
        serde_json::from_str(raw_provenance.get()).map_err(|source| {
            prediction_file_error(
                file_index,
                MeasurementFileError::InvalidPredictionProvenanceShape {
                    reason: source.to_string(),
                },
            )
        })?;
    provenance.validate().map_err(|source| {
        prediction_file_error(
            file_index,
            MeasurementFileError::InvalidPredictionProvenance { source },
        )
    })?;
    let mut decoded_facets = 0usize;
    let mut decoded_references = 0usize;
    let mut decoded_text = provenance.retained_text_bytes().map_err(|source| {
        prediction_file_error(
            file_index,
            MeasurementFileError::InvalidPredictionProvenance { source },
        )
    })?;
    let checks_v5 = checks
        .map(|raw_checks| {
            let mut decoded = Vec::with_capacity(raw_checks.len());
            for (check_index, raw_check) in raw_checks.into_iter().enumerate() {
                let wire: PredictionCheckWireInput = serde_json::from_str(raw_check.get())
                    .map_err(|source| {
                        prediction_file_error(
                            file_index,
                            MeasurementFileError::InvalidPredictionShape {
                                check_index,
                                reason: source.to_string(),
                            },
                        )
                    })?;
                if (wire.selection == SelectionState::Unselected
                    || wire.configuration == ConfigurationState::Disabled
                    || wire.applicability == Applicability::NotApplicable)
                    && wire.prediction.is_some()
                {
                    return Err(prediction_file_error(
                        file_index,
                        MeasurementFileError::InvalidPredictionLifecycle {
                            check_index,
                            reason: "inactive check must have empty output",
                        },
                    ));
                }
                let prediction =
                    wire.prediction
                        .map(|raw_prediction| {
                            serde_json::from_str::<EnginePredictionV5>(raw_prediction.get())
                                .map_err(|source| {
                                    prediction_file_error(
                                        file_index,
                                        MeasurementFileError::InvalidPredictionShape {
                                            check_index,
                                            reason: source.to_string(),
                                        },
                                    )
                                })
                        })
                        .transpose()?;
                let check = PredictionCheckInputV5 {
                    check_id: wire.check_id,
                    selection: wire.selection,
                    configuration: wire.configuration,
                    applicability: wire.applicability,
                    evaluation: wire.evaluation,
                    findings: wire.findings,
                    evaluated_scopes: wire.evaluated_scopes,
                    gaps: wire.gaps,
                    prediction,
                };
                check
                    .validate(check_index, Some(&provenance))
                    .map_err(|source| prediction_file_error(file_index, source))?;
                if let Some(prediction) = &check.prediction {
                    decoded_facets = decoded_facets
                        .checked_add(prediction.facets().len())
                        .ok_or_else(|| {
                            prediction_file_error(
                                file_index,
                                MeasurementFileError::PredictionAccountingOverflow,
                            )
                        })?;
                    decoded_references = decoded_references
                        .checked_add(prediction.basis_reference_count())
                        .ok_or_else(|| {
                            prediction_file_error(
                                file_index,
                                MeasurementFileError::PredictionAccountingOverflow,
                            )
                        })?;
                    decoded_text = decoded_text
                        .checked_add(prediction.retained_text_bytes().map_err(|source| {
                            prediction_file_error(
                                file_index,
                                MeasurementFileError::InvalidPrediction {
                                    check_index,
                                    source,
                                },
                            )
                        })?)
                        .ok_or_else(|| {
                            prediction_file_error(
                                file_index,
                                MeasurementFileError::PredictionAccountingOverflow,
                            )
                        })?;
                    if decoded_facets > PREDICTION_V1_MAX_FACETS_PER_FILE {
                        return Err(prediction_file_error(
                            file_index,
                            MeasurementFileError::TooManyPredictionFacets {
                                found: decoded_facets,
                                limit: PREDICTION_V1_MAX_FACETS_PER_FILE,
                            },
                        ));
                    }
                    if decoded_references > PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE {
                        return Err(prediction_file_error(
                            file_index,
                            MeasurementFileError::TooManyPredictionBasisReferences {
                                found: decoded_references,
                                limit: PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE,
                            },
                        ));
                    }
                    if decoded_text > PREDICTION_V1_MAX_TOTAL_TEXT_BYTES_PER_FILE {
                        return Err(prediction_file_error(
                            file_index,
                            MeasurementFileError::TooMuchPredictionText {
                                found: decoded_text,
                                limit: PREDICTION_V1_MAX_TOTAL_TEXT_BYTES_PER_FILE,
                            },
                        ));
                    }
                }
                decoded.push(check);
            }
            Ok(decoded)
        })
        .transpose()?;
    Ok(MeasurementFileInput {
        path,
        input,
        rig_v17: None,
        measurements,
        prediction_provenance: RequiredNullable::Missing,
        checks: None,
        legacy_prediction_provenance: RequiredNullable::Missing,
        legacy_checks: None,
        prediction_provenance_v3: RequiredNullable::Missing,
        checks_v3: None,
        prediction_provenance_v4: RequiredNullable::Missing,
        checks_v4: None,
        prediction_provenance_v5: RequiredNullable::Present(Some(provenance)),
        checks_v5,
        prediction_provenance_v6: RequiredNullable::Missing,
        checks_v6: None,
    })
}

fn decode_prediction_phase_file_v17(
    command: &str,
    file_index: usize,
    raw: &RawValue,
) -> Result<MeasurementFileInput, MeasurementReportError> {
    #[derive(Deserialize)]
    struct SchemaProbe {
        schema: String,
    }
    let probe: MeasurementFileWireInput = serde_json::from_str(raw.get()).map_err(|source| {
        prediction_file_error(
            file_index,
            MeasurementFileError::InvalidFileShape {
                reason: source.to_string(),
            },
        )
    })?;
    if command == "measure"
        || matches!(probe.prediction_provenance, RequiredNullable::Present(None))
    {
        return decode_prediction_phase_file_v16(command, file_index, raw);
    }
    let RequiredNullable::Present(Some(provenance_raw)) = &probe.prediction_provenance else {
        return decode_prediction_phase_file_v16(command, file_index, raw);
    };
    let schema = serde_json::from_str::<SchemaProbe>(provenance_raw.get()).map_err(|source| {
        prediction_file_error(
            file_index,
            MeasurementFileError::InvalidPredictionProvenanceShape {
                reason: source.to_string(),
            },
        )
    })?;
    if matches!(
        schema.schema.as_str(),
        crate::prediction::PREDICTION_PROVENANCE_V3_ID
            | crate::prediction::PREDICTION_PROVENANCE_V5_ID
    ) {
        return decode_prediction_phase_file_v16(command, file_index, raw);
    }
    if schema.schema != crate::prediction::PREDICTION_PROVENANCE_V6_ID {
        return Err(prediction_file_error(
            file_index,
            MeasurementFileError::InvalidPredictionProvenance {
                source: PredictionContractError::InvalidSchema {
                    field: "prediction provenance.schema",
                    expected: crate::prediction::PREDICTION_PROVENANCE_V6_ID,
                    found: schema.schema,
                },
            },
        ));
    }
    let MeasurementFileWireInput {
        path,
        input,
        rig,
        measurements,
        prediction_provenance,
        checks,
    } = probe;
    let rig_v17 = serde_json::from_str::<RigInfo>(rig.get()).map_err(|source| {
        prediction_file_error(
            file_index,
            MeasurementFileError::InvalidFileShape {
                reason: format!("invalid output-v17 rig evidence: {source}"),
            },
        )
    })?;
    if checks
        .as_ref()
        .is_some_and(|checks| checks.len() > OUTPUT_V11_MAX_CHECKS_PER_FILE)
    {
        return Err(prediction_file_error(
            file_index,
            MeasurementFileError::TooManyChecks {
                found: checks.as_ref().map_or(0, Vec::len),
                limit: OUTPUT_V11_MAX_CHECKS_PER_FILE,
            },
        ));
    }
    let RequiredNullable::Present(Some(raw_provenance)) = prediction_provenance else {
        unreachable!()
    };
    let provenance: PredictionProvenanceV6 =
        serde_json::from_str(raw_provenance.get()).map_err(|source| {
            prediction_file_error(
                file_index,
                MeasurementFileError::InvalidPredictionProvenanceShape {
                    reason: source.to_string(),
                },
            )
        })?;
    provenance.validate().map_err(|source| {
        prediction_file_error(
            file_index,
            MeasurementFileError::InvalidPredictionProvenance { source },
        )
    })?;
    let mut decoded_facets = 0usize;
    let mut decoded_references = 0usize;
    let mut decoded_text = provenance.retained_text_bytes().map_err(|source| {
        prediction_file_error(
            file_index,
            MeasurementFileError::InvalidPredictionProvenance { source },
        )
    })?;
    let checks_v6 = checks
        .map(|raw_checks| {
            let mut decoded = Vec::with_capacity(raw_checks.len());
            for (check_index, raw_check) in raw_checks.into_iter().enumerate() {
                let wire: PredictionCheckWireInput = serde_json::from_str(raw_check.get())
                    .map_err(|source| {
                        prediction_file_error(
                            file_index,
                            MeasurementFileError::InvalidPredictionShape {
                                check_index,
                                reason: source.to_string(),
                            },
                        )
                    })?;
                if (wire.selection == SelectionState::Unselected
                    || wire.configuration == ConfigurationState::Disabled
                    || wire.applicability == Applicability::NotApplicable)
                    && wire.prediction.is_some()
                {
                    return Err(prediction_file_error(
                        file_index,
                        MeasurementFileError::InvalidPredictionLifecycle {
                            check_index,
                            reason: "inactive check must have empty output",
                        },
                    ));
                }
                let prediction =
                    wire.prediction
                        .map(|raw_prediction| {
                            serde_json::from_str::<EnginePredictionV6>(raw_prediction.get())
                                .map_err(|source| {
                                    prediction_file_error(
                                        file_index,
                                        MeasurementFileError::InvalidPredictionShape {
                                            check_index,
                                            reason: source.to_string(),
                                        },
                                    )
                                })
                        })
                        .transpose()?;
                let check = PredictionCheckInputV6 {
                    check_id: wire.check_id,
                    selection: wire.selection,
                    configuration: wire.configuration,
                    applicability: wire.applicability,
                    evaluation: wire.evaluation,
                    findings: wire.findings,
                    evaluated_scopes: wire.evaluated_scopes,
                    gaps: wire.gaps,
                    prediction,
                };
                check
                    .validate(check_index, Some(&provenance))
                    .map_err(|source| prediction_file_error(file_index, source))?;
                if let Some(prediction) = &check.prediction {
                    decoded_facets = decoded_facets
                        .checked_add(prediction.facets().len())
                        .ok_or_else(|| {
                            prediction_file_error(
                                file_index,
                                MeasurementFileError::PredictionAccountingOverflow,
                            )
                        })?;
                    decoded_references = decoded_references
                        .checked_add(prediction.basis_reference_count())
                        .ok_or_else(|| {
                            prediction_file_error(
                                file_index,
                                MeasurementFileError::PredictionAccountingOverflow,
                            )
                        })?;
                    decoded_text = decoded_text
                        .checked_add(prediction.retained_text_bytes().map_err(|source| {
                            prediction_file_error(
                                file_index,
                                MeasurementFileError::InvalidPrediction {
                                    check_index,
                                    source,
                                },
                            )
                        })?)
                        .ok_or_else(|| {
                            prediction_file_error(
                                file_index,
                                MeasurementFileError::PredictionAccountingOverflow,
                            )
                        })?;
                    if decoded_facets > PREDICTION_V1_MAX_FACETS_PER_FILE {
                        return Err(prediction_file_error(
                            file_index,
                            MeasurementFileError::TooManyPredictionFacets {
                                found: decoded_facets,
                                limit: PREDICTION_V1_MAX_FACETS_PER_FILE,
                            },
                        ));
                    }
                    if decoded_references > PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE {
                        return Err(prediction_file_error(
                            file_index,
                            MeasurementFileError::TooManyPredictionBasisReferences {
                                found: decoded_references,
                                limit: PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE,
                            },
                        ));
                    }
                    if decoded_text > PREDICTION_V1_MAX_TOTAL_TEXT_BYTES_PER_FILE {
                        return Err(prediction_file_error(
                            file_index,
                            MeasurementFileError::TooMuchPredictionText {
                                found: decoded_text,
                                limit: PREDICTION_V1_MAX_TOTAL_TEXT_BYTES_PER_FILE,
                            },
                        ));
                    }
                }
                decoded.push(check);
            }
            Ok(decoded)
        })
        .transpose()?;
    Ok(MeasurementFileInput {
        path,
        input,
        rig_v17: Some(rig_v17),
        measurements,
        prediction_provenance: RequiredNullable::Missing,
        checks: None,
        legacy_prediction_provenance: RequiredNullable::Missing,
        legacy_checks: None,
        prediction_provenance_v3: RequiredNullable::Missing,
        checks_v3: None,
        prediction_provenance_v4: RequiredNullable::Missing,
        checks_v4: None,
        prediction_provenance_v5: RequiredNullable::Missing,
        checks_v5: None,
        prediction_provenance_v6: RequiredNullable::Present(Some(provenance)),
        checks_v6,
    })
}

/// Decode the immutable output-v11 file envelope with its original V1 staged
/// reader.  Historical evidence remains V1 all the way through validation;
/// accepting it must be neither weaker nor a reinterpretation as V2.
fn decode_legacy_v11_file(
    command: &str,
    file_index: usize,
    raw: &RawValue,
) -> Result<MeasurementFileInput, MeasurementReportError> {
    let wire: MeasurementFileWireInput = serde_json::from_str(raw.get()).map_err(|source| {
        prediction_file_error(
            file_index,
            MeasurementFileError::InvalidFileShape {
                reason: source.to_string(),
            },
        )
    })?;
    if command == "measure" {
        if !matches!(wire.prediction_provenance, RequiredNullable::Missing) {
            return Err(prediction_file_error(
                file_index,
                MeasurementFileError::UnexpectedPredictionProvenance,
            ));
        }
        if wire.checks.is_some() {
            return Err(prediction_file_error(
                file_index,
                MeasurementFileError::UnexpectedChecks,
            ));
        }
        return Ok(MeasurementFileInput {
            path: wire.path,
            input: wire.input,
            rig_v17: None,
            measurements: wire.measurements,
            prediction_provenance: RequiredNullable::Missing,
            checks: None,
            legacy_prediction_provenance: RequiredNullable::Missing,
            legacy_checks: None,
            prediction_provenance_v3: RequiredNullable::Missing,
            checks_v3: None,
            prediction_provenance_v4: RequiredNullable::Missing,
            checks_v4: None,
            prediction_provenance_v5: RequiredNullable::Missing,
            checks_v5: None,
            prediction_provenance_v6: RequiredNullable::Missing,
            checks_v6: None,
        });
    }

    if wire
        .checks
        .as_ref()
        .is_some_and(|checks| checks.len() > OUTPUT_V11_MAX_CHECKS_PER_FILE)
    {
        return Err(prediction_file_error(
            file_index,
            MeasurementFileError::TooManyChecks {
                found: wire.checks.as_ref().map_or(0, Vec::len),
                limit: OUTPUT_V11_MAX_CHECKS_PER_FILE,
            },
        ));
    }
    if matches!(wire.prediction_provenance, RequiredNullable::Missing) {
        return Err(prediction_file_error(
            file_index,
            MeasurementFileError::MissingPredictionProvenance,
        ));
    }
    let legacy_prediction_provenance = match wire.prediction_provenance {
        RequiredNullable::Missing => unreachable!("missing provenance was rejected above"),
        RequiredNullable::Present(None) => RequiredNullable::Present(None),
        RequiredNullable::Present(Some(raw)) => RequiredNullable::Present(Some(
            decode_prediction_provenance_v1_with_measurement_schema(
                raw.get(),
                MEASUREMENTS_V15_SCHEMA_ID,
            )
            .map_err(|error| {
                prediction_file_error(
                    file_index,
                    match error {
                        PredictionDecodeError::Shape(source) => {
                            MeasurementFileError::InvalidPredictionProvenanceShape {
                                reason: source.to_string(),
                            }
                        }
                        PredictionDecodeError::Semantic(source) => {
                            MeasurementFileError::InvalidPredictionProvenance { source }
                        }
                        PredictionDecodeError::TooManyFileFacets
                        | PredictionDecodeError::TooManyFileBasisReferences => {
                            unreachable!("provenance decoding cannot consume prediction budgets")
                        }
                    },
                )
            })?,
        )),
    };
    let mut decoded_facets = 0usize;
    let mut decoded_references = 0usize;
    let mut decoded_text = legacy_prediction_provenance
        .as_present()
        .map(PredictionProvenanceV1::retained_text_bytes)
        .transpose()
        .map_err(|source| {
            prediction_file_error(
                file_index,
                MeasurementFileError::InvalidPredictionProvenance { source },
            )
        })?
        .unwrap_or(0);
    let provenance_for_checks = legacy_prediction_provenance.as_present();
    let legacy_checks = wire
        .checks
        .map(|raw_checks| {
            let mut checks = Vec::with_capacity(raw_checks.len());
            for (check_index, raw) in raw_checks.into_iter().enumerate() {
                let wire: LegacyPredictionCheckWireV11 =
                    serde_json::from_str(raw.get()).map_err(|source| {
                        prediction_file_error(
                            file_index,
                            MeasurementFileError::InvalidPredictionShape {
                                check_index,
                                reason: source.to_string(),
                            },
                        )
                    })?;
                // Preserve the released V11 reader's precedence: these
                // lifecycle violations are decided from the raw attachment
                // presence before a malformed prediction can be decoded.
                if provenance_for_checks.is_none() && wire.prediction.is_some() {
                    return Err(prediction_file_error(
                        file_index,
                        MeasurementFileError::PredictionWithoutProvenance { check_index },
                    ));
                }
                if (wire.selection == SelectionState::Unselected
                    || wire.configuration == ConfigurationState::Disabled
                    || wire.applicability == Applicability::NotApplicable)
                    && wire.prediction.is_some()
                {
                    return Err(prediction_file_error(
                        file_index,
                        MeasurementFileError::InvalidPredictionLifecycle {
                            check_index,
                            reason: "inactive check must have empty output",
                        },
                    ));
                }
                let prediction = wire
                    .prediction
                    .map(|raw| {
                        decode_engine_prediction_v1_with_measurement_schema(
                            raw.get(),
                            PREDICTION_V1_MAX_FACETS_PER_FILE.saturating_sub(decoded_facets),
                            PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE
                                .saturating_sub(decoded_references),
                            MEASUREMENTS_V15_SCHEMA_ID,
                        )
                        .map_err(|error| {
                            prediction_file_error(
                                file_index,
                                match error {
                                    PredictionDecodeError::Shape(source) => {
                                        MeasurementFileError::InvalidPredictionShape {
                                            check_index,
                                            reason: source.to_string(),
                                        }
                                    }
                                    PredictionDecodeError::Semantic(source) => {
                                        MeasurementFileError::InvalidPrediction {
                                            check_index,
                                            source,
                                        }
                                    }
                                    PredictionDecodeError::TooManyFileFacets => {
                                        MeasurementFileError::TooManyPredictionFacets {
                                            found: PREDICTION_V1_MAX_FACETS_PER_FILE + 1,
                                            limit: PREDICTION_V1_MAX_FACETS_PER_FILE,
                                        }
                                    }
                                    PredictionDecodeError::TooManyFileBasisReferences => {
                                        MeasurementFileError::TooManyPredictionBasisReferences {
                                            found: PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE + 1,
                                            limit: PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE,
                                        }
                                    }
                                },
                            )
                        })
                    })
                    .transpose()?;
                let check = LegacyPredictionCheckInput {
                    check_id: wire.check_id,
                    selection: wire.selection,
                    configuration: wire.configuration,
                    applicability: wire.applicability,
                    evaluation: wire.evaluation,
                    findings: wire.findings,
                    evaluated_scopes: wire.evaluated_scopes,
                    gaps: wire.gaps,
                    prediction,
                };
                check
                    .validate(
                        check_index,
                        provenance_for_checks,
                        MEASUREMENTS_V15_SCHEMA_ID,
                    )
                    .map_err(|source| prediction_file_error(file_index, source))?;
                if let Some(prediction) = &check.prediction {
                    decoded_facets = decoded_facets
                        .checked_add(prediction.facets().len())
                        .ok_or_else(|| {
                            prediction_file_error(
                                file_index,
                                MeasurementFileError::PredictionAccountingOverflow,
                            )
                        })?;
                    if decoded_facets > PREDICTION_V1_MAX_FACETS_PER_FILE {
                        return Err(prediction_file_error(
                            file_index,
                            MeasurementFileError::TooManyPredictionFacets {
                                found: decoded_facets,
                                limit: PREDICTION_V1_MAX_FACETS_PER_FILE,
                            },
                        ));
                    }
                    decoded_references = decoded_references
                        .checked_add(prediction.basis_reference_count())
                        .ok_or_else(|| {
                            prediction_file_error(
                                file_index,
                                MeasurementFileError::PredictionAccountingOverflow,
                            )
                        })?;
                    if decoded_references > PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE {
                        return Err(prediction_file_error(
                            file_index,
                            MeasurementFileError::TooManyPredictionBasisReferences {
                                found: decoded_references,
                                limit: PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE,
                            },
                        ));
                    }
                    decoded_text = decoded_text
                        .checked_add(prediction.retained_text_bytes().map_err(|source| {
                            prediction_file_error(
                                file_index,
                                MeasurementFileError::InvalidPrediction {
                                    check_index,
                                    source,
                                },
                            )
                        })?)
                        .ok_or_else(|| {
                            prediction_file_error(
                                file_index,
                                MeasurementFileError::PredictionAccountingOverflow,
                            )
                        })?;
                    if decoded_text > PREDICTION_V1_MAX_TOTAL_TEXT_BYTES_PER_FILE {
                        return Err(prediction_file_error(
                            file_index,
                            MeasurementFileError::TooMuchPredictionText {
                                found: decoded_text,
                                limit: PREDICTION_V1_MAX_TOTAL_TEXT_BYTES_PER_FILE,
                            },
                        ));
                    }
                }
                checks.push(check);
            }
            Ok(checks)
        })
        .transpose()?;
    Ok(MeasurementFileInput {
        path: wire.path,
        input: wire.input,
        rig_v17: None,
        measurements: wire.measurements,
        prediction_provenance: RequiredNullable::Missing,
        checks: None,
        legacy_prediction_provenance,
        legacy_checks,
        prediction_provenance_v3: RequiredNullable::Missing,
        checks_v3: None,
        prediction_provenance_v4: RequiredNullable::Missing,
        checks_v4: None,
        prediction_provenance_v5: RequiredNullable::Missing,
        checks_v5: None,
        prediction_provenance_v6: RequiredNullable::Missing,
        checks_v6: None,
    })
}

fn validate_legacy_v11_prediction_phase_file(
    command: &str,
    file_index: usize,
    file: &MeasurementFileInput,
) -> Result<(usize, usize), MeasurementReportError> {
    match command {
        "measure" => return Ok((0, 0)),
        "lint" => {}
        _ => unreachable!("command was validated before prediction phase"),
    }
    let provenance = match &file.legacy_prediction_provenance {
        RequiredNullable::Missing => {
            return Err(prediction_file_error(
                file_index,
                MeasurementFileError::MissingPredictionProvenance,
            ));
        }
        RequiredNullable::Present(provenance) => provenance.as_ref(),
    };
    let checks = file
        .legacy_checks
        .as_ref()
        .ok_or_else(|| prediction_file_error(file_index, MeasurementFileError::MissingChecks))?;
    if checks.len() > OUTPUT_V11_MAX_CHECKS_PER_FILE {
        return Err(prediction_file_error(
            file_index,
            MeasurementFileError::TooManyChecks {
                found: checks.len(),
                limit: OUTPUT_V11_MAX_CHECKS_PER_FILE,
            },
        ));
    }
    if let Some(provenance) = provenance {
        provenance
            .validate_with_measurement_schema(MEASUREMENTS_V15_SCHEMA_ID)
            .map_err(|source| {
                prediction_file_error(
                    file_index,
                    MeasurementFileError::InvalidPredictionProvenance { source },
                )
            })?;
        let input = file
            .input
            .as_ref()
            .ok_or_else(|| prediction_file_error(file_index, MeasurementFileError::MissingInput))?;
        if input.sha256.as_deref() != Some(provenance.raw_source().primary_input().sha256())
            || input.bytes != Some(provenance.raw_source().primary_input().bytes())
        {
            return Err(prediction_file_error(
                file_index,
                MeasurementFileError::PredictionPrimaryInputMismatch,
            ));
        }
    }
    let mut facets = 0usize;
    let mut references = 0usize;
    let mut text = provenance
        .map(PredictionProvenanceV1::retained_text_bytes)
        .transpose()
        .map_err(|source| {
            prediction_file_error(
                file_index,
                MeasurementFileError::InvalidPredictionProvenance { source },
            )
        })?
        .unwrap_or(0);
    let mut available = 0usize;
    let mut unavailable = 0usize;
    for (check_index, check) in checks.iter().enumerate() {
        if let Some(prediction) = &check.prediction {
            facets = facets
                .checked_add(prediction.facets().len())
                .ok_or_else(|| {
                    prediction_file_error(
                        file_index,
                        MeasurementFileError::PredictionAccountingOverflow,
                    )
                })?;
            references = references
                .checked_add(prediction.basis_reference_count())
                .ok_or_else(|| {
                    prediction_file_error(
                        file_index,
                        MeasurementFileError::PredictionAccountingOverflow,
                    )
                })?;
            text = text
                .checked_add(prediction.retained_text_bytes().map_err(|source| {
                    prediction_file_error(
                        file_index,
                        MeasurementFileError::InvalidPrediction {
                            check_index,
                            source,
                        },
                    )
                })?)
                .ok_or_else(|| {
                    prediction_file_error(
                        file_index,
                        MeasurementFileError::PredictionAccountingOverflow,
                    )
                })?;
            for facet in prediction.facets() {
                match facet.state() {
                    EnginePredictionFacetStateV1::Available => {
                        available = available
                            .checked_add(1)
                            .ok_or(MeasurementReportError::PredictionFacetSummaryMismatch)?;
                    }
                    EnginePredictionFacetStateV1::RequiredPredictionUnavailable => {
                        unavailable = unavailable
                            .checked_add(1)
                            .ok_or(MeasurementReportError::PredictionFacetSummaryMismatch)?;
                    }
                }
            }
        }
    }
    if facets > PREDICTION_V1_MAX_FACETS_PER_FILE {
        return Err(prediction_file_error(
            file_index,
            MeasurementFileError::TooManyPredictionFacets {
                found: facets,
                limit: PREDICTION_V1_MAX_FACETS_PER_FILE,
            },
        ));
    }
    if references > PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE {
        return Err(prediction_file_error(
            file_index,
            MeasurementFileError::TooManyPredictionBasisReferences {
                found: references,
                limit: PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE,
            },
        ));
    }
    if text > PREDICTION_V1_MAX_TOTAL_TEXT_BYTES_PER_FILE {
        return Err(prediction_file_error(
            file_index,
            MeasurementFileError::TooMuchPredictionText {
                found: text,
                limit: PREDICTION_V1_MAX_TOTAL_TEXT_BYTES_PER_FILE,
            },
        ));
    }
    Ok((available, unavailable))
}

impl LegacyPredictionCheckInput {
    fn validate(
        &self,
        check_index: usize,
        provenance: Option<&PredictionProvenanceV1>,
        expected_measurement_schema: &'static str,
    ) -> Result<(), MeasurementFileError> {
        let gap_refs = self
            .gaps
            .iter()
            .map(|gap| CheckEvaluationGapRef {
                code: &gap.code,
                scope: gap.scope.as_ref(),
            })
            .collect::<Vec<_>>();
        let finding_check_ids = self
            .findings
            .iter()
            .map(|finding| finding.check_id.as_str())
            .collect::<Vec<_>>();
        let prediction_scopes = self
            .prediction
            .as_ref()
            .into_iter()
            .flat_map(EnginePredictionV1::facets)
            .map(|facet| facet.scope())
            .collect::<Vec<_>>();
        let derived = validate_and_derive_check_evaluation(CheckEvaluationValidationInput {
            check_id: &self.check_id,
            selection: self.selection,
            configuration: self.configuration,
            applicability: self.applicability,
            finding_check_ids: &finding_check_ids,
            evaluated_scopes: &self.evaluated_scopes,
            gaps: &gap_refs,
            prediction_scopes: &prediction_scopes,
            has_prediction: self.prediction.is_some(),
            prediction_has_required_unavailable: self
                .prediction
                .as_ref()
                .is_some_and(EnginePredictionV1::has_required_unavailable),
        })
        .map_err(|error| MeasurementFileError::InvalidPredictionLifecycle {
            check_index,
            reason: error.reason(),
        })?;
        if self.evaluation != derived {
            return Err(MeasurementFileError::InvalidPredictionLifecycle {
                check_index,
                reason: "evaluation does not match completed and missing prediction work",
            });
        }
        let Some(prediction) = &self.prediction else {
            if self
                .findings
                .iter()
                .any(|finding| finding.prediction_scope.is_some())
            {
                return Err(MeasurementFileError::InvalidPredictionLifecycle {
                    check_index,
                    reason: "finding has prediction_scope without prediction",
                });
            }
            return Ok(());
        };
        let provenance =
            provenance.ok_or(MeasurementFileError::PredictionWithoutProvenance { check_index })?;
        prediction
            .validate_against_provenance_with_measurement_schema(
                provenance,
                expected_measurement_schema,
            )
            .map_err(|source| MeasurementFileError::InvalidPrediction {
                check_index,
                source,
            })?;
        for facet in prediction.facets() {
            let evaluated = self
                .evaluated_scopes
                .iter()
                .filter(|scope| *scope == facet.scope())
                .count();
            let duplicated_gap = self
                .gaps
                .iter()
                .any(|gap| gap.scope.as_ref() == Some(facet.scope()));
            match facet.state() {
                EnginePredictionFacetStateV1::Available if evaluated != 1 => {
                    return Err(MeasurementFileError::InvalidPredictionLifecycle {
                        check_index,
                        reason: "available facet scope must occur exactly once in evaluated_scopes",
                    });
                }
                EnginePredictionFacetStateV1::RequiredPredictionUnavailable
                    if evaluated != 0 || duplicated_gap =>
                {
                    return Err(MeasurementFileError::InvalidPredictionLifecycle {
                        check_index,
                        reason: "required-unavailable facet scope must be absent from evaluated_scopes and gaps",
                    });
                }
                _ => {}
            }
        }
        for finding in &self.findings {
            let Some(scope) = &finding.prediction_scope else {
                return Err(MeasurementFileError::InvalidPredictionLifecycle {
                    check_index,
                    reason: "prediction-backed finding must carry prediction_scope",
                });
            };
            if prediction
                .facets()
                .iter()
                .filter(|facet| {
                    facet.scope() == scope
                        && facet.state() == EnginePredictionFacetStateV1::Available
                })
                .count()
                != 1
            {
                return Err(MeasurementFileError::InvalidPredictionLifecycle {
                    check_index,
                    reason: "finding prediction_scope must name one available facet",
                });
            }
        }
        Ok(())
    }
}

fn validate_prediction_phase_file(
    command: &str,
    file_index: usize,
    file: &MeasurementFileInput,
    expected_measurement_schema: &'static str,
) -> Result<(usize, usize), MeasurementReportError> {
    let mut available = 0usize;
    let mut unavailable = 0usize;
    match command {
        "measure" => {
            if !matches!(file.prediction_provenance, RequiredNullable::Missing) {
                return Err(prediction_file_error(
                    file_index,
                    MeasurementFileError::UnexpectedPredictionProvenance,
                ));
            }
            if file.checks.is_some() {
                return Err(prediction_file_error(
                    file_index,
                    MeasurementFileError::UnexpectedChecks,
                ));
            }
        }
        "lint" => {
            let provenance = match &file.prediction_provenance {
                RequiredNullable::Missing => {
                    return Err(prediction_file_error(
                        file_index,
                        MeasurementFileError::MissingPredictionProvenance,
                    ));
                }
                RequiredNullable::Present(provenance) => provenance.as_ref(),
            };
            let checks = file.checks.as_ref().ok_or_else(|| {
                prediction_file_error(file_index, MeasurementFileError::MissingChecks)
            })?;
            if checks.len() > OUTPUT_V11_MAX_CHECKS_PER_FILE {
                return Err(prediction_file_error(
                    file_index,
                    MeasurementFileError::TooManyChecks {
                        found: checks.len(),
                        limit: OUTPUT_V11_MAX_CHECKS_PER_FILE,
                    },
                ));
            }
            if let Some(provenance) = provenance {
                provenance
                    .validate_with_measurement_schema(expected_measurement_schema)
                    .map_err(|source| {
                        prediction_file_error(
                            file_index,
                            MeasurementFileError::InvalidPredictionProvenance { source },
                        )
                    })?;
                let input = file.input.as_ref().ok_or_else(|| {
                    prediction_file_error(file_index, MeasurementFileError::MissingInput)
                })?;
                if input.sha256.as_deref() != Some(provenance.raw_source().primary_input().sha256())
                    || input.bytes != Some(provenance.raw_source().primary_input().bytes())
                {
                    return Err(prediction_file_error(
                        file_index,
                        MeasurementFileError::PredictionPrimaryInputMismatch,
                    ));
                }
            }

            let mut facets = 0usize;
            let mut references = 0usize;
            let mut text = provenance
                .map(PredictionProvenanceV2::retained_text_bytes)
                .transpose()
                .map_err(|source| {
                    prediction_file_error(
                        file_index,
                        MeasurementFileError::InvalidPredictionProvenance { source },
                    )
                })?
                .unwrap_or(0);
            for (check_index, check) in checks.iter().enumerate() {
                if let Some(prediction) = &check.prediction {
                    facets = facets
                        .checked_add(prediction.facets().len())
                        .ok_or_else(|| {
                            prediction_file_error(
                                file_index,
                                MeasurementFileError::PredictionAccountingOverflow,
                            )
                        })?;
                    references = references
                        .checked_add(prediction.basis_reference_count())
                        .ok_or_else(|| {
                            prediction_file_error(
                                file_index,
                                MeasurementFileError::PredictionAccountingOverflow,
                            )
                        })?;
                    text = text
                        .checked_add(prediction.retained_text_bytes().map_err(|source| {
                            prediction_file_error(
                                file_index,
                                MeasurementFileError::InvalidPrediction {
                                    check_index,
                                    source,
                                },
                            )
                        })?)
                        .ok_or_else(|| {
                            prediction_file_error(
                                file_index,
                                MeasurementFileError::PredictionAccountingOverflow,
                            )
                        })?;
                    for facet in prediction.facets() {
                        match facet.state() {
                            EnginePredictionFacetStateV1::Available => {
                                available = available.checked_add(1).ok_or(
                                    MeasurementReportError::PredictionFacetSummaryMismatch,
                                )?;
                            }
                            EnginePredictionFacetStateV1::RequiredPredictionUnavailable => {
                                unavailable = unavailable.checked_add(1).ok_or(
                                    MeasurementReportError::PredictionFacetSummaryMismatch,
                                )?;
                            }
                        }
                    }
                }
            }
            if facets > PREDICTION_V1_MAX_FACETS_PER_FILE {
                return Err(prediction_file_error(
                    file_index,
                    MeasurementFileError::TooManyPredictionFacets {
                        found: facets,
                        limit: PREDICTION_V1_MAX_FACETS_PER_FILE,
                    },
                ));
            }
            if references > PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE {
                return Err(prediction_file_error(
                    file_index,
                    MeasurementFileError::TooManyPredictionBasisReferences {
                        found: references,
                        limit: PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE,
                    },
                ));
            }
            if text > PREDICTION_V1_MAX_TOTAL_TEXT_BYTES_PER_FILE {
                return Err(prediction_file_error(
                    file_index,
                    MeasurementFileError::TooMuchPredictionText {
                        found: text,
                        limit: PREDICTION_V1_MAX_TOTAL_TEXT_BYTES_PER_FILE,
                    },
                ));
            }
        }
        _ => unreachable!("command was validated before prediction phase"),
    }
    Ok((available, unavailable))
}

fn validate_prediction_phase_file_v14(
    command: &str,
    file_index: usize,
    file: &MeasurementFileInput,
) -> Result<(usize, usize), MeasurementReportError> {
    if command == "measure" {
        if !matches!(file.prediction_provenance_v3, RequiredNullable::Missing) {
            return Err(prediction_file_error(
                file_index,
                MeasurementFileError::UnexpectedPredictionProvenance,
            ));
        }
        if file.checks_v3.is_some() {
            return Err(prediction_file_error(
                file_index,
                MeasurementFileError::UnexpectedChecks,
            ));
        }
        return Ok((0, 0));
    }
    let provenance = match &file.prediction_provenance_v3 {
        RequiredNullable::Missing => {
            return Err(prediction_file_error(
                file_index,
                MeasurementFileError::MissingPredictionProvenance,
            ));
        }
        RequiredNullable::Present(provenance) => provenance.as_ref(),
    };
    let checks = file
        .checks_v3
        .as_ref()
        .ok_or_else(|| prediction_file_error(file_index, MeasurementFileError::MissingChecks))?;
    if let Some(provenance) = provenance {
        provenance.validate().map_err(|source| {
            prediction_file_error(
                file_index,
                MeasurementFileError::InvalidPredictionProvenance { source },
            )
        })?;
        let input = file
            .input
            .as_ref()
            .ok_or_else(|| prediction_file_error(file_index, MeasurementFileError::MissingInput))?;
        if input.sha256.as_deref() != Some(provenance.raw_source().primary_input().sha256())
            || input.bytes != Some(provenance.raw_source().primary_input().bytes())
        {
            return Err(prediction_file_error(
                file_index,
                MeasurementFileError::PredictionPrimaryInputMismatch,
            ));
        }
    }
    let mut available = 0usize;
    let mut unavailable = 0usize;
    let mut facets = 0usize;
    let mut references = 0usize;
    let mut text = provenance
        .map(PredictionProvenanceV3::retained_text_bytes)
        .transpose()
        .map_err(|source| {
            prediction_file_error(
                file_index,
                MeasurementFileError::InvalidPredictionProvenance { source },
            )
        })?
        .unwrap_or(0);
    for (check_index, check) in checks.iter().enumerate() {
        check
            .validate(check_index, provenance)
            .map_err(|source| prediction_file_error(file_index, source))?;
        if let Some(prediction) = &check.prediction {
            facets = facets
                .checked_add(prediction.facets().len())
                .ok_or_else(|| {
                    prediction_file_error(
                        file_index,
                        MeasurementFileError::PredictionAccountingOverflow,
                    )
                })?;
            references = references
                .checked_add(prediction.basis_reference_count())
                .ok_or_else(|| {
                    prediction_file_error(
                        file_index,
                        MeasurementFileError::PredictionAccountingOverflow,
                    )
                })?;
            text = text
                .checked_add(prediction.retained_text_bytes().map_err(|source| {
                    prediction_file_error(
                        file_index,
                        MeasurementFileError::InvalidPrediction {
                            check_index,
                            source,
                        },
                    )
                })?)
                .ok_or_else(|| {
                    prediction_file_error(
                        file_index,
                        MeasurementFileError::PredictionAccountingOverflow,
                    )
                })?;
            for facet in prediction.facets() {
                match facet.state() {
                    EnginePredictionFacetStateV1::Available => available += 1,
                    EnginePredictionFacetStateV1::RequiredPredictionUnavailable => unavailable += 1,
                }
            }
        }
    }
    if facets > PREDICTION_V1_MAX_FACETS_PER_FILE {
        return Err(prediction_file_error(
            file_index,
            MeasurementFileError::TooManyPredictionFacets {
                found: facets,
                limit: PREDICTION_V1_MAX_FACETS_PER_FILE,
            },
        ));
    }
    if references > PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE {
        return Err(prediction_file_error(
            file_index,
            MeasurementFileError::TooManyPredictionBasisReferences {
                found: references,
                limit: PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE,
            },
        ));
    }
    if text > PREDICTION_V1_MAX_TOTAL_TEXT_BYTES_PER_FILE {
        return Err(prediction_file_error(
            file_index,
            MeasurementFileError::TooMuchPredictionText {
                found: text,
                limit: PREDICTION_V1_MAX_TOTAL_TEXT_BYTES_PER_FILE,
            },
        ));
    }
    Ok((available, unavailable))
}

fn validate_prediction_phase_file_v15(
    command: &str,
    file_index: usize,
    file: &MeasurementFileInput,
) -> Result<(usize, usize), MeasurementReportError> {
    if matches!(file.prediction_provenance_v4, RequiredNullable::Missing) {
        return validate_prediction_phase_file_v14(command, file_index, file);
    }
    if command == "measure" {
        return Err(prediction_file_error(
            file_index,
            MeasurementFileError::UnexpectedPredictionProvenance,
        ));
    }
    let provenance = match &file.prediction_provenance_v4 {
        RequiredNullable::Present(provenance) => provenance.as_ref(),
        RequiredNullable::Missing => unreachable!(),
    };
    let checks = file
        .checks_v4
        .as_ref()
        .ok_or_else(|| prediction_file_error(file_index, MeasurementFileError::MissingChecks))?;
    if let Some(provenance) = provenance {
        provenance.validate().map_err(|source| {
            prediction_file_error(
                file_index,
                MeasurementFileError::InvalidPredictionProvenance { source },
            )
        })?;
        let input = file
            .input
            .as_ref()
            .ok_or_else(|| prediction_file_error(file_index, MeasurementFileError::MissingInput))?;
        if input.sha256.as_deref() != Some(provenance.raw_source().primary_input().sha256())
            || input.bytes != Some(provenance.raw_source().primary_input().bytes())
        {
            return Err(prediction_file_error(
                file_index,
                MeasurementFileError::PredictionPrimaryInputMismatch,
            ));
        }
    }
    let mut available = 0usize;
    let mut unavailable = 0usize;
    let mut has_facet_budget_summary = false;
    let mut facets = 0usize;
    let mut references = 0usize;
    let mut text = provenance
        .map(PredictionProvenanceV4::retained_text_bytes)
        .transpose()
        .map_err(|source| {
            prediction_file_error(
                file_index,
                MeasurementFileError::InvalidPredictionProvenance { source },
            )
        })?
        .unwrap_or(0);
    for (check_index, check) in checks.iter().enumerate() {
        check
            .validate(check_index, provenance)
            .map_err(|source| prediction_file_error(file_index, source))?;
        if let Some(prediction) = &check.prediction {
            has_facet_budget_summary |= prediction.has_facet_budget_summary();
            facets = facets
                .checked_add(prediction.facets().len())
                .ok_or_else(|| {
                    prediction_file_error(
                        file_index,
                        MeasurementFileError::PredictionAccountingOverflow,
                    )
                })?;
            references = references
                .checked_add(prediction.basis_reference_count())
                .ok_or_else(|| {
                    prediction_file_error(
                        file_index,
                        MeasurementFileError::PredictionAccountingOverflow,
                    )
                })?;
            text = text
                .checked_add(prediction.retained_text_bytes().map_err(|source| {
                    prediction_file_error(
                        file_index,
                        MeasurementFileError::InvalidPrediction {
                            check_index,
                            source,
                        },
                    )
                })?)
                .ok_or_else(|| {
                    prediction_file_error(
                        file_index,
                        MeasurementFileError::PredictionAccountingOverflow,
                    )
                })?;
            for facet in prediction.facets() {
                match facet.state() {
                    EnginePredictionFacetStateV1::Available => available += 1,
                    EnginePredictionFacetStateV1::RequiredPredictionUnavailable => unavailable += 1,
                }
            }
        }
    }
    if facets > PREDICTION_V1_MAX_FACETS_PER_FILE {
        return Err(prediction_file_error(
            file_index,
            MeasurementFileError::TooManyPredictionFacets {
                found: facets,
                limit: PREDICTION_V1_MAX_FACETS_PER_FILE,
            },
        ));
    }
    if has_facet_budget_summary && facets != PREDICTION_V1_MAX_FACETS_PER_FILE {
        return Err(prediction_file_error(
            file_index,
            MeasurementFileError::FacetBudgetSummaryWithoutExhaustedFileBudget {
                found: facets,
                limit: PREDICTION_V1_MAX_FACETS_PER_FILE,
            },
        ));
    }
    if references > PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE {
        return Err(prediction_file_error(
            file_index,
            MeasurementFileError::TooManyPredictionBasisReferences {
                found: references,
                limit: PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE,
            },
        ));
    }
    if text > PREDICTION_V1_MAX_TOTAL_TEXT_BYTES_PER_FILE {
        return Err(prediction_file_error(
            file_index,
            MeasurementFileError::TooMuchPredictionText {
                found: text,
                limit: PREDICTION_V1_MAX_TOTAL_TEXT_BYTES_PER_FILE,
            },
        ));
    }
    Ok((available, unavailable))
}

fn validate_prediction_phase_file_v16(
    command: &str,
    file_index: usize,
    file: &MeasurementFileInput,
) -> Result<(usize, usize), MeasurementReportError> {
    if matches!(file.prediction_provenance_v5, RequiredNullable::Missing) {
        return validate_prediction_phase_file_v15(command, file_index, file);
    }
    if command == "measure" {
        return Err(prediction_file_error(
            file_index,
            MeasurementFileError::UnexpectedPredictionProvenance,
        ));
    }
    let provenance = match &file.prediction_provenance_v5 {
        RequiredNullable::Present(provenance) => provenance.as_ref(),
        RequiredNullable::Missing => unreachable!(),
    };
    let checks = file
        .checks_v5
        .as_ref()
        .ok_or_else(|| prediction_file_error(file_index, MeasurementFileError::MissingChecks))?;
    if let Some(provenance) = provenance {
        provenance.validate().map_err(|source| {
            prediction_file_error(
                file_index,
                MeasurementFileError::InvalidPredictionProvenance { source },
            )
        })?;
        let input = file
            .input
            .as_ref()
            .ok_or_else(|| prediction_file_error(file_index, MeasurementFileError::MissingInput))?;
        if input.sha256.as_deref()
            != Some(provenance.raw_animation_channels().primary_input().sha256())
            || input.bytes != Some(provenance.raw_animation_channels().primary_input().bytes())
        {
            return Err(prediction_file_error(
                file_index,
                MeasurementFileError::PredictionPrimaryInputMismatch,
            ));
        }
    }
    let mut available = 0usize;
    let mut unavailable = 0usize;
    let mut facets = 0usize;
    let mut references = 0usize;
    let mut has_facet_budget_summary = false;
    let mut text = provenance
        .map(PredictionProvenanceV5::retained_text_bytes)
        .transpose()
        .map_err(|source| {
            prediction_file_error(
                file_index,
                MeasurementFileError::InvalidPredictionProvenance { source },
            )
        })?
        .unwrap_or(0);
    for (check_index, check) in checks.iter().enumerate() {
        check
            .validate(check_index, provenance)
            .map_err(|source| prediction_file_error(file_index, source))?;
        validate_current_engine_track_support_prediction_v5(
            &check.check_id,
            check.selection,
            check.configuration,
            check.applicability,
            check.prediction.as_ref(),
            provenance,
            check.findings.is_empty(),
        )
        .map_err(|source| {
            prediction_file_error(
                file_index,
                MeasurementFileError::InvalidPrediction {
                    check_index,
                    source,
                },
            )
        })?;
        if let Some(prediction) = &check.prediction {
            has_facet_budget_summary |= prediction.base_prediction().has_facet_budget_summary();
            facets = facets
                .checked_add(prediction.facets().len())
                .ok_or_else(|| {
                    prediction_file_error(
                        file_index,
                        MeasurementFileError::PredictionAccountingOverflow,
                    )
                })?;
            references = references
                .checked_add(prediction.basis_reference_count())
                .ok_or_else(|| {
                    prediction_file_error(
                        file_index,
                        MeasurementFileError::PredictionAccountingOverflow,
                    )
                })?;
            text = text
                .checked_add(prediction.retained_text_bytes().map_err(|source| {
                    prediction_file_error(
                        file_index,
                        MeasurementFileError::InvalidPrediction {
                            check_index,
                            source,
                        },
                    )
                })?)
                .ok_or_else(|| {
                    prediction_file_error(
                        file_index,
                        MeasurementFileError::PredictionAccountingOverflow,
                    )
                })?;
            for facet in prediction.facets() {
                match facet.state() {
                    EnginePredictionFacetStateV1::Available => available += 1,
                    EnginePredictionFacetStateV1::RequiredPredictionUnavailable => unavailable += 1,
                }
            }
        }
    }
    if facets > PREDICTION_V1_MAX_FACETS_PER_FILE {
        return Err(prediction_file_error(
            file_index,
            MeasurementFileError::TooManyPredictionFacets {
                found: facets,
                limit: PREDICTION_V1_MAX_FACETS_PER_FILE,
            },
        ));
    }
    if has_facet_budget_summary && facets != PREDICTION_V1_MAX_FACETS_PER_FILE {
        return Err(prediction_file_error(
            file_index,
            MeasurementFileError::FacetBudgetSummaryWithoutExhaustedFileBudget {
                found: facets,
                limit: PREDICTION_V1_MAX_FACETS_PER_FILE,
            },
        ));
    }
    if references > PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE {
        return Err(prediction_file_error(
            file_index,
            MeasurementFileError::TooManyPredictionBasisReferences {
                found: references,
                limit: PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE,
            },
        ));
    }
    if text > PREDICTION_V1_MAX_TOTAL_TEXT_BYTES_PER_FILE {
        return Err(prediction_file_error(
            file_index,
            MeasurementFileError::TooMuchPredictionText {
                found: text,
                limit: PREDICTION_V1_MAX_TOTAL_TEXT_BYTES_PER_FILE,
            },
        ));
    }
    Ok((available, unavailable))
}

fn validate_prediction_phase_file_v17(
    command: &str,
    file_index: usize,
    file: &MeasurementFileInput,
) -> Result<(usize, usize), MeasurementReportError> {
    if matches!(file.prediction_provenance_v6, RequiredNullable::Missing) {
        return validate_prediction_phase_file_v16(command, file_index, file);
    }
    if command == "measure" {
        return Err(prediction_file_error(
            file_index,
            MeasurementFileError::UnexpectedPredictionProvenance,
        ));
    }
    let provenance = match &file.prediction_provenance_v6 {
        RequiredNullable::Present(provenance) => provenance.as_ref(),
        RequiredNullable::Missing => unreachable!(),
    };
    let checks = file
        .checks_v6
        .as_ref()
        .ok_or_else(|| prediction_file_error(file_index, MeasurementFileError::MissingChecks))?;
    if let Some(provenance) = provenance {
        provenance.validate().map_err(|source| {
            prediction_file_error(
                file_index,
                MeasurementFileError::InvalidPredictionProvenance { source },
            )
        })?;
        let input = file
            .input
            .as_ref()
            .ok_or_else(|| prediction_file_error(file_index, MeasurementFileError::MissingInput))?;
        let primary = provenance.raw_transform_paths().primary_input();
        if input.sha256.as_deref() != Some(primary.sha256()) || input.bytes != Some(primary.bytes())
        {
            return Err(prediction_file_error(
                file_index,
                MeasurementFileError::PredictionPrimaryInputMismatch,
            ));
        }
    }
    let mut available = 0usize;
    let mut unavailable = 0usize;
    let mut facets = 0usize;
    let mut references = 0usize;
    let mut has_facet_budget_summary = false;
    let mut text = provenance
        .map(PredictionProvenanceV6::retained_text_bytes)
        .transpose()
        .map_err(|source| {
            prediction_file_error(
                file_index,
                MeasurementFileError::InvalidPredictionProvenance { source },
            )
        })?
        .unwrap_or(0);
    for (check_index, check) in checks.iter().enumerate() {
        check
            .validate(check_index, provenance)
            .map_err(|source| prediction_file_error(file_index, source))?;
        if let Some(prediction) = &check.prediction {
            has_facet_budget_summary |= prediction.base_prediction().has_facet_budget_summary();
            facets = facets
                .checked_add(prediction.facets().len())
                .ok_or_else(|| {
                    prediction_file_error(
                        file_index,
                        MeasurementFileError::PredictionAccountingOverflow,
                    )
                })?;
            references = references
                .checked_add(prediction.basis_reference_count())
                .ok_or_else(|| {
                    prediction_file_error(
                        file_index,
                        MeasurementFileError::PredictionAccountingOverflow,
                    )
                })?;
            text = text
                .checked_add(prediction.retained_text_bytes().map_err(|source| {
                    prediction_file_error(
                        file_index,
                        MeasurementFileError::InvalidPrediction {
                            check_index,
                            source,
                        },
                    )
                })?)
                .ok_or_else(|| {
                    prediction_file_error(
                        file_index,
                        MeasurementFileError::PredictionAccountingOverflow,
                    )
                })?;
            for facet in prediction.facets() {
                match facet.state() {
                    EnginePredictionFacetStateV1::Available => available += 1,
                    EnginePredictionFacetStateV1::RequiredPredictionUnavailable => unavailable += 1,
                }
            }
        }
    }
    if facets > PREDICTION_V1_MAX_FACETS_PER_FILE {
        return Err(prediction_file_error(
            file_index,
            MeasurementFileError::TooManyPredictionFacets {
                found: facets,
                limit: PREDICTION_V1_MAX_FACETS_PER_FILE,
            },
        ));
    }
    if has_facet_budget_summary && facets != PREDICTION_V1_MAX_FACETS_PER_FILE {
        return Err(prediction_file_error(
            file_index,
            MeasurementFileError::FacetBudgetSummaryWithoutExhaustedFileBudget {
                found: facets,
                limit: PREDICTION_V1_MAX_FACETS_PER_FILE,
            },
        ));
    }
    if references > PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE {
        return Err(prediction_file_error(
            file_index,
            MeasurementFileError::TooManyPredictionBasisReferences {
                found: references,
                limit: PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE,
            },
        ));
    }
    if text > PREDICTION_V1_MAX_TOTAL_TEXT_BYTES_PER_FILE {
        return Err(prediction_file_error(
            file_index,
            MeasurementFileError::TooMuchPredictionText {
                found: text,
                limit: PREDICTION_V1_MAX_TOTAL_TEXT_BYTES_PER_FILE,
            },
        ));
    }
    Ok((available, unavailable))
}

fn validate_prediction_summary(
    command: &str,
    summary: Option<&MeasurementReportSummaryInput>,
    available: usize,
    unavailable: usize,
) -> Result<(), MeasurementReportError> {
    let summary = summary.and_then(|summary| summary.prediction_facets.as_ref());
    match (command, summary) {
        ("measure", Some(_)) => Err(MeasurementReportError::UnexpectedPredictionFacetSummary),
        ("measure", None) => Ok(()),
        ("lint", None) => Err(MeasurementReportError::MissingPredictionFacetSummary),
        ("lint", Some(summary))
            if summary.available != available
                || summary.required_prediction_unavailable != unavailable =>
        {
            Err(MeasurementReportError::PredictionFacetSummaryMismatch)
        }
        ("lint", Some(_)) => Ok(()),
        _ => unreachable!("command was validated before prediction summary"),
    }
}

fn validate_prediction_summary_presence(
    command: &str,
    summary: Option<&MeasurementReportSummaryInput>,
) -> Result<(), MeasurementReportError> {
    match (
        command,
        summary.and_then(|summary| summary.prediction_facets.as_ref()),
    ) {
        ("measure", Some(_)) => Err(MeasurementReportError::UnexpectedPredictionFacetSummary),
        ("lint", None) => Err(MeasurementReportError::MissingPredictionFacetSummary),
        ("measure", None) | ("lint", Some(_)) => Ok(()),
        _ => unreachable!("command was validated before prediction summary"),
    }
}

impl PredictionCheckInput {
    fn validate(
        &self,
        check_index: usize,
        provenance: Option<&PredictionProvenanceV2>,
        expected_measurement_schema: &'static str,
    ) -> Result<(), MeasurementFileError> {
        let gap_refs = self
            .gaps
            .iter()
            .map(|gap| CheckEvaluationGapRef {
                code: &gap.code,
                scope: gap.scope.as_ref(),
            })
            .collect::<Vec<_>>();
        let finding_check_ids = self
            .findings
            .iter()
            .map(|finding| finding.check_id.as_str())
            .collect::<Vec<_>>();
        let prediction_scopes = self
            .prediction
            .as_ref()
            .into_iter()
            .flat_map(EnginePredictionV2::facets)
            .map(|facet| facet.scope())
            .collect::<Vec<_>>();
        let derived = validate_and_derive_check_evaluation(CheckEvaluationValidationInput {
            check_id: &self.check_id,
            selection: self.selection,
            configuration: self.configuration,
            applicability: self.applicability,
            finding_check_ids: &finding_check_ids,
            evaluated_scopes: &self.evaluated_scopes,
            gaps: &gap_refs,
            prediction_scopes: &prediction_scopes,
            has_prediction: self.prediction.is_some(),
            prediction_has_required_unavailable: self
                .prediction
                .as_ref()
                .is_some_and(EnginePredictionV2::has_required_unavailable),
        })
        .map_err(|error| MeasurementFileError::InvalidPredictionLifecycle {
            check_index,
            reason: error.reason(),
        })?;
        if self.evaluation != derived {
            return Err(MeasurementFileError::InvalidPredictionLifecycle {
                check_index,
                reason: "evaluation does not match completed and missing prediction work",
            });
        }

        let Some(prediction) = &self.prediction else {
            if self
                .findings
                .iter()
                .any(|finding| finding.prediction_scope.is_some())
            {
                return Err(MeasurementFileError::InvalidPredictionLifecycle {
                    check_index,
                    reason: "finding has prediction_scope without prediction",
                });
            }
            return Ok(());
        };
        let provenance =
            provenance.ok_or(MeasurementFileError::PredictionWithoutProvenance { check_index })?;
        prediction
            .validate_against_provenance_with_measurement_schema(
                provenance,
                expected_measurement_schema,
            )
            .map_err(|source| MeasurementFileError::InvalidPrediction {
                check_index,
                source,
            })?;
        prediction
            .validate_facet_budget_summary_for_check(&self.check_id)
            .map_err(|source| MeasurementFileError::InvalidPrediction {
                check_index,
                source,
            })?;
        validate_current_engine_addressability_prediction_v2(
            &self.check_id,
            prediction,
            provenance,
        )
        .map_err(|source| MeasurementFileError::InvalidPrediction {
            check_index,
            source,
        })?;
        for facet in prediction.facets() {
            let evaluated = self
                .evaluated_scopes
                .iter()
                .filter(|scope| *scope == facet.scope())
                .count();
            let duplicated_gap = self
                .gaps
                .iter()
                .any(|gap| gap.scope.as_ref() == Some(facet.scope()));
            match facet.state() {
                EnginePredictionFacetStateV1::Available if evaluated != 1 => {
                    return Err(MeasurementFileError::InvalidPredictionLifecycle {
                        check_index,
                        reason: "available facet scope must occur exactly once in evaluated_scopes",
                    });
                }
                EnginePredictionFacetStateV1::RequiredPredictionUnavailable
                    if evaluated != 0 || duplicated_gap =>
                {
                    return Err(MeasurementFileError::InvalidPredictionLifecycle {
                        check_index,
                        reason: "required-unavailable facet scope must be absent from evaluated_scopes and gaps",
                    });
                }
                _ => {}
            }
        }
        for finding in &self.findings {
            let Some(scope) = &finding.prediction_scope else {
                return Err(MeasurementFileError::InvalidPredictionLifecycle {
                    check_index,
                    reason: "prediction-backed finding must carry prediction_scope",
                });
            };
            if prediction
                .facets()
                .iter()
                .filter(|facet| {
                    facet.scope() == scope
                        && facet.state() == EnginePredictionFacetStateV1::Available
                })
                .count()
                != 1
            {
                return Err(MeasurementFileError::InvalidPredictionLifecycle {
                    check_index,
                    reason: "finding prediction_scope must name one available facet",
                });
            }
        }
        Ok(())
    }
}

impl PredictionCheckInputV3 {
    fn validate(
        &self,
        check_index: usize,
        provenance: Option<&PredictionProvenanceV3>,
    ) -> Result<(), MeasurementFileError> {
        let gap_refs = self
            .gaps
            .iter()
            .map(|gap| CheckEvaluationGapRef {
                code: &gap.code,
                scope: gap.scope.as_ref(),
            })
            .collect::<Vec<_>>();
        let finding_check_ids = self
            .findings
            .iter()
            .map(|finding| finding.check_id.as_str())
            .collect::<Vec<_>>();
        let prediction_scopes = self
            .prediction
            .as_ref()
            .into_iter()
            .flat_map(EnginePredictionV3::facets)
            .map(|facet| facet.scope())
            .collect::<Vec<_>>();
        let derived = validate_and_derive_check_evaluation(CheckEvaluationValidationInput {
            check_id: &self.check_id,
            selection: self.selection,
            configuration: self.configuration,
            applicability: self.applicability,
            finding_check_ids: &finding_check_ids,
            evaluated_scopes: &self.evaluated_scopes,
            gaps: &gap_refs,
            prediction_scopes: &prediction_scopes,
            has_prediction: self.prediction.is_some(),
            prediction_has_required_unavailable: self
                .prediction
                .as_ref()
                .is_some_and(EnginePredictionV3::has_required_unavailable),
        })
        .map_err(|error| MeasurementFileError::InvalidPredictionLifecycle {
            check_index,
            reason: error.reason(),
        })?;
        if self.evaluation != derived {
            return Err(MeasurementFileError::InvalidPredictionLifecycle {
                check_index,
                reason: "evaluation does not match completed and missing prediction work",
            });
        }
        validate_current_engine_clip_boundary_applicability_v3(
            &self.check_id,
            self.applicability,
            provenance,
        )
        .map_err(|source| MeasurementFileError::InvalidPrediction {
            check_index,
            source,
        })?;
        let Some(prediction) = &self.prediction else {
            if self
                .findings
                .iter()
                .any(|finding| finding.prediction_scope.is_some())
            {
                return Err(MeasurementFileError::InvalidPredictionLifecycle {
                    check_index,
                    reason: "finding has prediction_scope without prediction",
                });
            }
            if self.check_id == "engine-clip-boundary"
                && self.selection == SelectionState::Selected
                && self.configuration == ConfigurationState::Enabled
                && self.applicability == Applicability::Applicable
            {
                return Err(MeasurementFileError::InvalidPrediction {
                    check_index,
                    source: PredictionContractError::EngineClipBoundaryFacetMismatch,
                });
            }
            return Ok(());
        };
        let provenance =
            provenance.ok_or(MeasurementFileError::PredictionWithoutProvenance { check_index })?;
        prediction
            .validate_against_provenance(provenance)
            .map_err(|source| MeasurementFileError::InvalidPrediction {
                check_index,
                source,
            })?;
        prediction
            .validate_facet_budget_summary_for_check(&self.check_id)
            .map_err(|source| MeasurementFileError::InvalidPrediction {
                check_index,
                source,
            })?;
        validate_current_engine_addressability_prediction_v3(
            &self.check_id,
            prediction,
            provenance,
        )
        .map_err(|source| MeasurementFileError::InvalidPrediction {
            check_index,
            source,
        })?;
        for facet in prediction.facets() {
            let evaluated = self
                .evaluated_scopes
                .iter()
                .filter(|scope| *scope == facet.scope())
                .count();
            let duplicated_gap = self
                .gaps
                .iter()
                .any(|gap| gap.scope.as_ref() == Some(facet.scope()));
            match facet.state() {
                EnginePredictionFacetStateV1::Available if evaluated != 1 => {
                    return Err(MeasurementFileError::InvalidPredictionLifecycle {
                        check_index,
                        reason: "available facet scope must occur exactly once in evaluated_scopes",
                    });
                }
                EnginePredictionFacetStateV1::RequiredPredictionUnavailable
                    if evaluated != 0 || duplicated_gap =>
                {
                    return Err(MeasurementFileError::InvalidPredictionLifecycle {
                        check_index,
                        reason: "required-unavailable facet scope must be absent from evaluated_scopes and gaps",
                    });
                }
                _ => {}
            }
        }
        for finding in &self.findings {
            let Some(scope) = &finding.prediction_scope else {
                return Err(MeasurementFileError::InvalidPredictionLifecycle {
                    check_index,
                    reason: "prediction-backed finding must carry prediction_scope",
                });
            };
            if prediction
                .facets()
                .iter()
                .filter(|facet| {
                    facet.scope() == scope
                        && facet.state() == EnginePredictionFacetStateV1::Available
                })
                .count()
                != 1
            {
                return Err(MeasurementFileError::InvalidPredictionLifecycle {
                    check_index,
                    reason: "finding prediction_scope must name one available facet",
                });
            }
        }
        let finding_scopes = self
            .findings
            .iter()
            .filter_map(|finding| finding.prediction_scope.as_ref())
            .collect::<Vec<_>>();
        validate_current_engine_clip_boundary_prediction_v3(
            &self.check_id,
            prediction,
            provenance,
            &self.evaluated_scopes,
            &finding_scopes,
        )
        .map_err(|source| MeasurementFileError::InvalidPrediction {
            check_index,
            source,
        })?;
        Ok(())
    }
}

impl PredictionCheckInputV4 {
    fn validate(
        &self,
        check_index: usize,
        provenance: Option<&PredictionProvenanceV4>,
    ) -> Result<(), MeasurementFileError> {
        let gap_refs = self
            .gaps
            .iter()
            .map(|gap| CheckEvaluationGapRef {
                code: &gap.code,
                scope: gap.scope.as_ref(),
            })
            .collect::<Vec<_>>();
        let finding_check_ids = self
            .findings
            .iter()
            .map(|finding| finding.check_id.as_str())
            .collect::<Vec<_>>();
        let prediction_scopes = self
            .prediction
            .as_ref()
            .into_iter()
            .flat_map(EnginePredictionV4::facets)
            .map(|facet| facet.scope())
            .collect::<Vec<_>>();
        let derived = validate_and_derive_check_evaluation(CheckEvaluationValidationInput {
            check_id: &self.check_id,
            selection: self.selection,
            configuration: self.configuration,
            applicability: self.applicability,
            finding_check_ids: &finding_check_ids,
            evaluated_scopes: &self.evaluated_scopes,
            gaps: &gap_refs,
            prediction_scopes: &prediction_scopes,
            has_prediction: self.prediction.is_some(),
            prediction_has_required_unavailable: self
                .prediction
                .as_ref()
                .is_some_and(EnginePredictionV4::has_required_unavailable),
        })
        .map_err(|error| MeasurementFileError::InvalidPredictionLifecycle {
            check_index,
            reason: error.reason(),
        })?;
        if self.evaluation != derived {
            return Err(MeasurementFileError::InvalidPredictionLifecycle {
                check_index,
                reason: "evaluation does not match completed and missing prediction work",
            });
        }
        let Some(prediction) = &self.prediction else {
            if self
                .findings
                .iter()
                .any(|finding| finding.prediction_scope.is_some())
            {
                return Err(MeasurementFileError::InvalidPredictionLifecycle {
                    check_index,
                    reason: "finding has prediction_scope without prediction",
                });
            }
            return Ok(());
        };
        let provenance =
            provenance.ok_or(MeasurementFileError::PredictionWithoutProvenance { check_index })?;
        prediction
            .validate_against_provenance(provenance)
            .map_err(|source| MeasurementFileError::InvalidPrediction {
                check_index,
                source,
            })?;
        prediction
            .validate_facet_budget_summary_for_check(&self.check_id)
            .map_err(|source| MeasurementFileError::InvalidPrediction {
                check_index,
                source,
            })?;
        for facet in prediction.facets() {
            let evaluated = self
                .evaluated_scopes
                .iter()
                .filter(|scope| *scope == facet.scope())
                .count();
            let duplicated_gap = self
                .gaps
                .iter()
                .any(|gap| gap.scope.as_ref() == Some(facet.scope()));
            match facet.state() {
                EnginePredictionFacetStateV1::Available if evaluated != 1 => {
                    return Err(MeasurementFileError::InvalidPredictionLifecycle {
                        check_index,
                        reason: "available facet scope must occur exactly once in evaluated_scopes",
                    });
                }
                EnginePredictionFacetStateV1::RequiredPredictionUnavailable
                    if evaluated != 0 || duplicated_gap =>
                {
                    return Err(MeasurementFileError::InvalidPredictionLifecycle {
                        check_index,
                        reason: "required-unavailable facet scope must be absent from evaluated_scopes and gaps",
                    });
                }
                _ => {}
            }
        }
        for finding in &self.findings {
            let Some(scope) = &finding.prediction_scope else {
                return Err(MeasurementFileError::InvalidPredictionLifecycle {
                    check_index,
                    reason: "prediction-backed finding must carry prediction_scope",
                });
            };
            if prediction
                .facets()
                .iter()
                .filter(|facet| {
                    facet.scope() == scope
                        && facet.state() == EnginePredictionFacetStateV1::Available
                })
                .count()
                != 1
            {
                return Err(MeasurementFileError::InvalidPredictionLifecycle {
                    check_index,
                    reason: "finding prediction_scope must name one available facet",
                });
            }
        }
        Ok(())
    }
}

impl PredictionCheckInputV5 {
    fn validate(
        &self,
        check_index: usize,
        provenance: Option<&PredictionProvenanceV5>,
    ) -> Result<(), MeasurementFileError> {
        let gap_refs = self
            .gaps
            .iter()
            .map(|gap| CheckEvaluationGapRef {
                code: &gap.code,
                scope: gap.scope.as_ref(),
            })
            .collect::<Vec<_>>();
        let finding_check_ids = self
            .findings
            .iter()
            .map(|finding| finding.check_id.as_str())
            .collect::<Vec<_>>();
        let prediction_scopes = self
            .prediction
            .as_ref()
            .into_iter()
            .flat_map(EnginePredictionV5::facets)
            .map(|facet| facet.scope())
            .collect::<Vec<_>>();
        let derived = validate_and_derive_check_evaluation(CheckEvaluationValidationInput {
            check_id: &self.check_id,
            selection: self.selection,
            configuration: self.configuration,
            applicability: self.applicability,
            finding_check_ids: &finding_check_ids,
            evaluated_scopes: &self.evaluated_scopes,
            gaps: &gap_refs,
            prediction_scopes: &prediction_scopes,
            has_prediction: self.prediction.is_some(),
            prediction_has_required_unavailable: self
                .prediction
                .as_ref()
                .is_some_and(EnginePredictionV5::has_required_unavailable),
        })
        .map_err(|error| MeasurementFileError::InvalidPredictionLifecycle {
            check_index,
            reason: error.reason(),
        })?;
        if self.evaluation != derived {
            return Err(MeasurementFileError::InvalidPredictionLifecycle {
                check_index,
                reason: "evaluation does not match completed and missing prediction work",
            });
        }
        let Some(prediction) = &self.prediction else {
            if self
                .findings
                .iter()
                .any(|finding| finding.prediction_scope.is_some())
            {
                return Err(MeasurementFileError::InvalidPredictionLifecycle {
                    check_index,
                    reason: "finding has prediction_scope without prediction",
                });
            }
            return Ok(());
        };
        let provenance =
            provenance.ok_or(MeasurementFileError::PredictionWithoutProvenance { check_index })?;
        prediction
            .validate_against_provenance(provenance)
            .map_err(|source| MeasurementFileError::InvalidPrediction {
                check_index,
                source,
            })?;
        prediction
            .base_prediction()
            .validate_facet_budget_summary_for_check(&self.check_id)
            .map_err(|source| MeasurementFileError::InvalidPrediction {
                check_index,
                source,
            })?;
        for facet in prediction.facets() {
            let evaluated = self
                .evaluated_scopes
                .iter()
                .filter(|scope| *scope == facet.scope())
                .count();
            let duplicated_gap = self
                .gaps
                .iter()
                .any(|gap| gap.scope.as_ref() == Some(facet.scope()));
            match facet.state() {
                EnginePredictionFacetStateV1::Available if evaluated != 1 => {
                    return Err(MeasurementFileError::InvalidPredictionLifecycle {
                        check_index,
                        reason: "available facet scope must occur exactly once in evaluated_scopes",
                    });
                }
                EnginePredictionFacetStateV1::RequiredPredictionUnavailable
                    if evaluated != 0 || duplicated_gap =>
                {
                    return Err(MeasurementFileError::InvalidPredictionLifecycle {
                        check_index,
                        reason: "required-unavailable facet scope must be absent from evaluated_scopes and gaps",
                    });
                }
                _ => {}
            }
        }
        for finding in &self.findings {
            let Some(scope) = &finding.prediction_scope else {
                return Err(MeasurementFileError::InvalidPredictionLifecycle {
                    check_index,
                    reason: "prediction-backed finding must carry prediction_scope",
                });
            };
            if prediction
                .facets()
                .iter()
                .filter(|facet| {
                    facet.scope() == scope
                        && facet.state() == EnginePredictionFacetStateV1::Available
                })
                .count()
                != 1
            {
                return Err(MeasurementFileError::InvalidPredictionLifecycle {
                    check_index,
                    reason: "finding prediction_scope must name one available facet",
                });
            }
        }
        Ok(())
    }
}

impl PredictionCheckInputV6 {
    fn validate(
        &self,
        check_index: usize,
        provenance: Option<&PredictionProvenanceV6>,
    ) -> Result<(), MeasurementFileError> {
        let gap_refs = self
            .gaps
            .iter()
            .map(|gap| CheckEvaluationGapRef {
                code: &gap.code,
                scope: gap.scope.as_ref(),
            })
            .collect::<Vec<_>>();
        let finding_check_ids = self
            .findings
            .iter()
            .map(|finding| finding.check_id.as_str())
            .collect::<Vec<_>>();
        let prediction_scopes = self
            .prediction
            .as_ref()
            .into_iter()
            .flat_map(EnginePredictionV6::facets)
            .map(|facet| facet.scope())
            .collect::<Vec<_>>();
        let derived = validate_and_derive_check_evaluation(CheckEvaluationValidationInput {
            check_id: &self.check_id,
            selection: self.selection,
            configuration: self.configuration,
            applicability: self.applicability,
            finding_check_ids: &finding_check_ids,
            evaluated_scopes: &self.evaluated_scopes,
            gaps: &gap_refs,
            prediction_scopes: &prediction_scopes,
            has_prediction: self.prediction.is_some(),
            prediction_has_required_unavailable: self
                .prediction
                .as_ref()
                .is_some_and(EnginePredictionV6::has_required_unavailable),
        })
        .map_err(|error| MeasurementFileError::InvalidPredictionLifecycle {
            check_index,
            reason: error.reason(),
        })?;
        if self.evaluation != derived {
            return Err(MeasurementFileError::InvalidPredictionLifecycle {
                check_index,
                reason: "evaluation does not match completed and missing prediction work",
            });
        }
        let Some(prediction) = &self.prediction else {
            if self
                .findings
                .iter()
                .any(|finding| finding.prediction_scope.is_some())
            {
                return Err(MeasurementFileError::InvalidPredictionLifecycle {
                    check_index,
                    reason: "finding has prediction_scope without prediction",
                });
            }
            return Ok(());
        };
        let provenance =
            provenance.ok_or(MeasurementFileError::PredictionWithoutProvenance { check_index })?;
        prediction
            .validate_against_provenance(provenance)
            .map_err(|source| MeasurementFileError::InvalidPrediction {
                check_index,
                source,
            })?;
        prediction
            .base_prediction()
            .validate_facet_budget_summary_for_check(&self.check_id)
            .map_err(|source| MeasurementFileError::InvalidPrediction {
                check_index,
                source,
            })?;
        for facet in prediction.facets() {
            let evaluated = self
                .evaluated_scopes
                .iter()
                .filter(|scope| *scope == facet.scope())
                .count();
            let duplicated_gap = self
                .gaps
                .iter()
                .any(|gap| gap.scope.as_ref() == Some(facet.scope()));
            match facet.state() {
                EnginePredictionFacetStateV1::Available if evaluated != 1 => {
                    return Err(MeasurementFileError::InvalidPredictionLifecycle {
                        check_index,
                        reason: "available facet scope must occur exactly once in evaluated_scopes",
                    });
                }
                EnginePredictionFacetStateV1::RequiredPredictionUnavailable
                    if evaluated != 0 || duplicated_gap =>
                {
                    return Err(MeasurementFileError::InvalidPredictionLifecycle {
                        check_index,
                        reason: "required-unavailable facet scope must be absent from evaluated_scopes and gaps",
                    });
                }
                _ => {}
            }
        }
        for finding in &self.findings {
            let Some(scope) = &finding.prediction_scope else {
                return Err(MeasurementFileError::InvalidPredictionLifecycle {
                    check_index,
                    reason: "prediction-backed finding must carry prediction_scope",
                });
            };
            if prediction
                .facets()
                .iter()
                .filter(|facet| {
                    facet.scope() == scope
                        && facet.state() == EnginePredictionFacetStateV1::Available
                })
                .count()
                != 1
            {
                return Err(MeasurementFileError::InvalidPredictionLifecycle {
                    check_index,
                    reason: "finding prediction_scope must name one available facet",
                });
            }
        }
        Ok(())
    }
}

/// Validate the frozen current-lint addressability inventory contract.  This
/// is deliberately output-facing: standalone V1 engine artifacts retain their
/// historic provenance and reason vocabulary.
fn validate_current_engine_addressability_prediction_v2(
    check_id: &str,
    prediction: &EnginePredictionV2,
    provenance: &PredictionProvenanceV2,
) -> Result<(), PredictionContractError> {
    if check_id != "engine-addressability" {
        return Ok(());
    }
    let raw_partial =
        provenance.raw_source().clips_coverage().state() != RawSourceSetCoverageStateV1::Complete;
    let settings_partial = matches!(
        provenance.settings().clip_coverage().state(),
        ResolvedEngineSettingsCoverageStateV2::Partial
    );
    let inventories = prediction
        .facets()
        .iter()
        .filter(|facet| {
            facet.scope().code.as_str() == "animation_asset_label_inventory"
                && facet.scope().subject.is_none()
        })
        .collect::<Vec<_>>();
    if !raw_partial && !settings_partial {
        if prediction.facets().iter().any(|facet| {
            facet
                .reasons()
                .contains(&PredictionUnavailableReasonV2::ResolvedSettingsOverflow)
        }) {
            return Err(PredictionContractError::EngineAddressabilityInventoryReasonsMismatch);
        }
        let available = prediction
            .facets()
            .iter()
            .filter(|facet| facet.state() == EnginePredictionFacetStateV1::Available)
            .collect::<Vec<_>>();
        let expected_rows = provenance.settings().clips().len();
        if (!prediction.has_facet_budget_summary() && available.len() != expected_rows)
            || (prediction.has_facet_budget_summary() && available.len() >= expected_rows)
        {
            return Err(PredictionContractError::EngineAddressabilityFacetPrefixMismatch);
        }
        let mut seen = vec![false; available.len()];
        for facet in available {
            let Some(ordinal) = facet
                .scope()
                .subject
                .as_deref()
                .and_then(|subject| subject.strip_prefix("Animation"))
                .and_then(|ordinal| ordinal.parse::<usize>().ok())
            else {
                return Err(PredictionContractError::EngineAddressabilityFacetPrefixMismatch);
            };
            if facet.scope().code.as_str() != "animation_asset_label"
                || ordinal >= seen.len()
                || std::mem::replace(&mut seen[ordinal], true)
                || !facet.basis().references().iter().any(|reference| {
                    matches!(
                        reference,
                        PredictionBasisReferenceV1::RawSource { reference }
                            if reference.domain() == RawSourceDomainV1::Clip
                                && matches!(
                                    reference.key(),
                                    RawSourceKeyV1::Clip { source_clip_index }
                                        if *source_clip_index == ordinal as u64
                                )
                                && reference.field().as_str() == "source_name.state"
                    )
                })
            {
                return Err(PredictionContractError::EngineAddressabilityFacetPrefixMismatch);
            }
        }
        if seen.iter().any(|seen| !seen) {
            return Err(PredictionContractError::EngineAddressabilityFacetPrefixMismatch);
        }
        return Ok(());
    }

    let mut expected = Vec::new();
    if raw_partial {
        expected.push(PredictionUnavailableReasonV2::RawSourceIncomplete);
    }
    if settings_partial {
        expected.push(PredictionUnavailableReasonV2::ResolvedSettingsOverflow);
    }
    if inventories.len() != 1 && !(inventories.is_empty() && prediction.has_facet_budget_summary())
    {
        return Err(PredictionContractError::EngineAddressabilityInventoryReasonsMismatch);
    }
    if let Some(inventory) = inventories.first()
        && (inventory.state() != EnginePredictionFacetStateV1::RequiredPredictionUnavailable
            || inventory.reasons() != expected)
    {
        return Err(PredictionContractError::EngineAddressabilityInventoryReasonsMismatch);
    }
    if inventories.is_empty() {
        // The allocator may replace this sole incomplete-inventory candidate
        // with its canonical budget summary.  The enclosing file validator
        // separately proves that the shared 4,096-slot budget was exhausted.
        return Ok(());
    }
    Ok(())
}

/// Validate the frozen V14/V3 addressability inventory without retargeting
/// the immutable V13/V2 contract.
fn validate_current_engine_addressability_prediction_v3(
    check_id: &str,
    prediction: &EnginePredictionV3,
    provenance: &PredictionProvenanceV3,
) -> Result<(), PredictionContractError> {
    if check_id != "engine-addressability" {
        return Ok(());
    }
    let raw_partial =
        provenance.raw_source().clips_coverage().state() != RawSourceSetCoverageStateV1::Complete;
    let settings_partial = matches!(
        provenance.settings().clip_coverage().state(),
        ResolvedEngineSettingsCoverageStateV2::Partial
    );
    let inventories = prediction
        .facets()
        .iter()
        .filter(|facet| {
            facet.scope().code.as_str() == "animation_asset_label_inventory"
                && facet.scope().subject.is_none()
        })
        .collect::<Vec<_>>();
    if !raw_partial && !settings_partial {
        if prediction.facets().iter().any(|facet| {
            facet
                .reasons()
                .contains(&PredictionUnavailableReasonV2::ResolvedSettingsOverflow)
        }) {
            return Err(PredictionContractError::EngineAddressabilityInventoryReasonsMismatch);
        }
        let available = prediction
            .facets()
            .iter()
            .filter(|facet| facet.state() == EnginePredictionFacetStateV1::Available)
            .collect::<Vec<_>>();
        let expected_rows = provenance.settings().clips().len();
        if (!prediction.has_facet_budget_summary() && available.len() != expected_rows)
            || (prediction.has_facet_budget_summary() && available.len() >= expected_rows)
        {
            return Err(PredictionContractError::EngineAddressabilityFacetPrefixMismatch);
        }
        let mut seen = vec![false; available.len()];
        for facet in available {
            let Some(ordinal) = facet
                .scope()
                .subject
                .as_deref()
                .and_then(|subject| subject.strip_prefix("Animation"))
                .and_then(|ordinal| ordinal.parse::<usize>().ok())
            else {
                return Err(PredictionContractError::EngineAddressabilityFacetPrefixMismatch);
            };
            if facet.scope().code.as_str() != "animation_asset_label"
                || ordinal >= seen.len()
                || std::mem::replace(&mut seen[ordinal], true)
                || !facet.basis().references().iter().any(|reference| {
                    matches!(
                        reference,
                        PredictionBasisReferenceV2::V1(PredictionBasisReferenceV1::RawSource { reference })
                            if reference.domain() == RawSourceDomainV1::Clip
                                && matches!(
                                    reference.key(),
                                    RawSourceKeyV1::Clip { source_clip_index }
                                        if *source_clip_index == ordinal as u64
                                )
                                && reference.field().as_str() == "source_name.state"
                    )
                })
            {
                return Err(PredictionContractError::EngineAddressabilityFacetPrefixMismatch);
            }
        }
        if seen.iter().any(|seen| !seen) {
            return Err(PredictionContractError::EngineAddressabilityFacetPrefixMismatch);
        }
        return Ok(());
    }

    let mut expected = Vec::new();
    if raw_partial {
        expected.push(PredictionUnavailableReasonV2::RawSourceIncomplete);
    }
    if settings_partial {
        expected.push(PredictionUnavailableReasonV2::ResolvedSettingsOverflow);
    }
    if inventories.len() != 1 && !(inventories.is_empty() && prediction.has_facet_budget_summary())
    {
        return Err(PredictionContractError::EngineAddressabilityInventoryReasonsMismatch);
    }
    if let Some(inventory) = inventories.first()
        && (inventory.state() != EnginePredictionFacetStateV1::RequiredPredictionUnavailable
            || inventory.reasons() != expected)
    {
        return Err(PredictionContractError::EngineAddressabilityInventoryReasonsMismatch);
    }
    Ok(())
}

const ENGINE_TRACK_SUPPORT_CHECK_ID: &str = "engine-track-support";
const ENGINE_TRACK_SUPPORT_ANIMATION_SCOPE: &str = "engine-track-support:animation";
const ENGINE_TRACK_SUPPORT_CHANNEL_SCOPE: &str = "engine-track-support:animation-channel";
const ENGINE_TRACK_SUPPORT_INVENTORY_SCOPE: &str = "engine-track-support:inventory";
const ENGINE_TRACK_SUPPORT_BUDGET_SCOPE: &str = "engine-track-support:facet-budget";

fn validate_current_engine_track_support_prediction_v5(
    check_id: &str,
    selection_state: SelectionState,
    configuration: ConfigurationState,
    applicability: Applicability,
    prediction: Option<&EnginePredictionV5>,
    provenance: Option<&PredictionProvenanceV5>,
    findings_empty: bool,
) -> Result<(), PredictionContractError> {
    if check_id != ENGINE_TRACK_SUPPORT_CHECK_ID {
        if prediction.is_some_and(|prediction| {
            prediction.facets().iter().any(|facet| {
                matches!(
                    facet.result(),
                    Some(EngineMachineResultV1::SourceImportDisposition(_))
                )
            })
        }) {
            return Err(PredictionContractError::InvalidMachineResult(
                "source-import disposition is confined to engine-track-support",
            ));
        }
        return Ok(());
    }
    let exact_profile = provenance.is_some_and(|provenance| {
        let base = provenance.base();
        let selected = base.profile().selection();
        selected.family() == "bevy"
            && selected.profile_revision() == 3
            && selected.engine_version() == "0.19.0"
            && selected.importer() == "gltf-asset-loader"
            && base.profile().fact_bundle_urn() == "urn:animsmith:engine-profile:bevy:3"
            && base.profile().facts_identity().sha256()
                == "d532b00621bf06a2db2dedf896c19aae2c07b3b1873a1b05beade2252d7a89c5"
            && base.profile().facts_identity().bytes() == 4_849
            && matches!(
                base.source_format(),
                SourceFormatV1::GltfJson | SourceFormatV1::Glb
            )
            && !provenance.raw_animation_channels().is_complete_empty()
    });
    let expected_applicability = if exact_profile {
        Applicability::Applicable
    } else {
        Applicability::NotApplicable
    };
    if applicability != expected_applicability {
        return Err(PredictionContractError::InvalidMachineResult(
            "engine-track-support applicability mismatch",
        ));
    }
    if !exact_profile
        || selection_state != SelectionState::Selected
        || configuration != ConfigurationState::Enabled
    {
        return if prediction.is_none() {
            Ok(())
        } else {
            Err(PredictionContractError::InvalidMachineResult(
                "inactive engine-track-support carried prediction",
            ))
        };
    }
    if !findings_empty {
        return Err(PredictionContractError::InvalidMachineResult(
            "engine-track-support must not emit findings",
        ));
    }
    let provenance = provenance.ok_or(PredictionContractError::ProvenanceIdentityMismatch)?;
    let prediction = prediction.ok_or(PredictionContractError::InvalidMachineResult(
        "applicable engine-track-support check requires prediction",
    ))?;
    let inventory = provenance.raw_animation_channels();
    let mut expected = Vec::new();
    let has_summary = prediction.base_prediction().has_facet_budget_summary();
    let expected_demand = if !inventory.source_coverage_complete() {
        1
    } else {
        inventory.rows().len()
    };
    if inventory.source_coverage_complete() && inventory.candidate_overflow() && !has_summary {
        return Err(PredictionContractError::InvalidMachineResult(
            "saturated engine-track-support demand requires facet-budget summary",
        ));
    }
    let candidate_capacity = if has_summary {
        prediction.facets().len().saturating_sub(1)
    } else {
        expected_demand
    };
    if !inventory.is_complete_empty() && candidate_capacity != 0 {
        if !inventory.source_coverage_complete() {
            expected.push(track_unavailable(
                EvaluationScope::new(crate::evaluation::EvaluationScopeCode::custom(
                    ENGINE_TRACK_SUPPORT_INVENTORY_SCOPE,
                )),
                track_inventory_basis(inventory),
                PredictionUnavailableReasonV2::RawSourceIncomplete,
            ));
        } else {
            for row in inventory.rows().iter().take(candidate_capacity) {
                let animation = row.source_animation_index();
                if let Some(channel) = row.source_channel_index() {
                    expected.push(track_subject_facet(
                        track_scope(
                            ENGINE_TRACK_SUPPORT_CHANNEL_SCOPE,
                            format!("source_animation:{animation}:source_channel:{channel}"),
                        ),
                        track_row_basis(inventory, animation, Some(channel)),
                        SourceImportSubjectKindV1::AnimationChannel,
                        track_gate(provenance),
                    ));
                } else {
                    expected.push(track_subject_facet(
                        track_scope(
                            ENGINE_TRACK_SUPPORT_ANIMATION_SCOPE,
                            format!("source_animation:{animation}"),
                        ),
                        track_row_basis(inventory, animation, None),
                        SourceImportSubjectKindV1::Animation,
                        track_gate(provenance),
                    ));
                }
            }
        }
    }
    if has_summary {
        expected.push(track_unavailable(
            EvaluationScope::new(crate::evaluation::EvaluationScopeCode::custom(
                ENGINE_TRACK_SUPPORT_BUDGET_SCOPE,
            )),
            track_static_basis(),
            PredictionUnavailableReasonV2::FacetBudgetExceeded,
        ));
    }
    let expected = EnginePredictionV4::new(provenance.base().identity().clone(), expected)?
        .facets()
        .to_vec();
    if prediction.facets() != expected {
        return Err(PredictionContractError::InvalidMachineResult(
            "engine-track-support facets do not reconstruct from V5 provenance",
        ));
    }
    Ok(())
}

/// Shared producer/readback hook for the immutable engine-root-motion rule.
///
/// The exact Unity revision-2 reconstruction is filled beside the production
/// rule once its final profile identity and facet grammar are frozen. Keeping
/// this hook at the common boundary prevents the writer and strict reader from
/// acquiring separate acceptance paths in the meantime.
#[allow(clippy::too_many_arguments)]
fn validate_current_engine_root_motion_prediction_v6<F: RootMotionFindingEvidence>(
    check_id: &str,
    selection: SelectionState,
    configuration: ConfigurationState,
    applicability: Applicability,
    prediction: Option<&EnginePredictionV6>,
    provenance: Option<&PredictionProvenanceV6>,
    findings: &[F],
    rig: &RigInfo,
    measurements: &MeasurementContract,
) -> Result<(), PredictionContractError> {
    const CHECK_ID: &str = "engine-root-motion";
    if check_id != CHECK_ID
        && prediction.is_some_and(|prediction| {
            prediction.facets().iter().any(|facet| {
                matches!(
                    facet.result(),
                    Some(EngineMachineResultV1::RootMotionRouting(_))
                )
            })
        })
    {
        return Err(PredictionContractError::InvalidMachineResult(
            "root-motion routing is confined to engine-root-motion",
        ));
    }
    if check_id != CHECK_ID {
        return Ok(());
    }
    let active =
        selection == SelectionState::Selected && configuration == ConfigurationState::Enabled;
    let exact = provenance.is_some_and(root_motion_is_exact_unity_v2);
    let has_work = provenance.is_some_and(root_motion_has_work);
    let expected_applicability = if exact && has_work {
        Applicability::Applicable
    } else {
        Applicability::NotApplicable
    };
    if applicability != expected_applicability {
        return Err(PredictionContractError::InvalidMachineResult(
            "engine-root-motion applicability does not reconstruct from V6 provenance",
        ));
    }
    if !active || expected_applicability == Applicability::NotApplicable {
        if prediction.is_some() || !findings.is_empty() {
            return Err(PredictionContractError::InvalidMachineResult(
                "inactive or inapplicable engine-root-motion must carry no prediction or findings",
            ));
        }
        return Ok(());
    }
    let provenance = provenance.ok_or(PredictionContractError::InvalidMachineResult(
        "applicable engine-root-motion has no V6 provenance",
    ))?;
    let prediction = prediction.ok_or(PredictionContractError::InvalidMachineResult(
        "applicable engine-root-motion has no V6 prediction",
    ))?;
    validate_root_motion_facets_v6(prediction, provenance, findings, rig, measurements)
}

/// An exact profile remains applicable while declaration evidence needed to
/// prove complete-empty intent is incomplete. This keeps the strict reader
/// aligned with the producer: an unvisited ownerless tail is unavailable work,
/// not N/A.
fn root_motion_has_work(provenance: &PredictionProvenanceV6) -> bool {
    let intent = provenance.root_motion_project_intent();
    if intent.clip_coverage() != crate::EngineRootMotionProjectIntentCoverageV1::Complete
        || provenance.base().base().settings().clip_coverage().state()
            != ResolvedEngineSettingsCoverageStateV2::Complete
    {
        return true;
    }
    match intent.declared_axis_candidates() {
        crate::EngineRootMotionProjectIntentCountV1::Exact { count } => count != 0,
        crate::EngineRootMotionProjectIntentCountV1::NPlusOne => true,
    }
}

fn root_motion_is_exact_unity_v2(provenance: &PredictionProvenanceV6) -> bool {
    let profile = provenance.base().base().profile();
    let selection = profile.selection();
    selection.family() == "unity-generic"
        && selection.profile_revision() == 2
        && selection.engine_version() == "6000.3"
        && selection.importer() == "fbx-model-importer"
        && profile.fact_bundle_urn() == "urn:animsmith:engine-profile:unity-generic:2"
        && profile.facts_identity().sha256()
            == "740e1c324a7a5b13efa2d9980fe255a6245d858adec55fb3387614a3ff45274c"
        && profile.facts_identity().bytes() == 2_776
        && provenance.base().base().source_format() == SourceFormatV1::Fbx
        && profile.setting_descriptors().len() == 7
        && profile.primary_sources().len() == 3
        && [
            "unity-fbx-animation-clip-6000.3",
            "unity-fbx-model-importer-6000.3",
            "unity-fbx-motion-node-6000.3",
        ]
        .into_iter()
        .all(|id| profile.source(id).is_some())
}

fn validate_root_motion_facets_v6<F: RootMotionFindingEvidence>(
    prediction: &EnginePredictionV6,
    provenance: &PredictionProvenanceV6,
    findings: &[F],
    rig: &RigInfo,
    measurements: &MeasurementContract,
) -> Result<(), PredictionContractError> {
    const INVENTORY_SCOPE: &str = "engine-root-motion:inventory";
    const AXIS_SCOPE: &str = "engine-root-motion:clip-axis";
    const BUDGET_SCOPE: &str = "engine-root-motion:facet-budget";
    let intent = provenance.root_motion_project_intent();
    let mut atomic = Vec::new();
    if provenance
        .base()
        .base()
        .raw_source()
        .clips_coverage()
        .state()
        != RawSourceSetCoverageStateV1::Complete
        || !matches!(
            provenance.raw_transform_paths().coverage(),
            crate::RawTransformPathCoverageV1::Complete
        )
    {
        atomic.push(PredictionUnavailableReasonV2::RawSourceIncomplete);
    }
    if intent.clip_coverage() != crate::EngineRootMotionProjectIntentCoverageV1::Complete {
        atomic.push(PredictionUnavailableReasonV2::ProjectIntentUnavailable);
    }
    if intent.declared_axis_candidates().overflowed()
        || intent.unmapped_declared_axis_candidates().overflowed()
    {
        atomic.push(PredictionUnavailableReasonV2::custom(
            "animsmith:root_motion_intent_work_budget_exceeded",
        )?);
    }
    if !matches!(
        intent.unmapped_declared_axis_candidates(),
        crate::EngineRootMotionProjectIntentCountV1::Exact { count: 0 }
    ) {
        atomic.push(PredictionUnavailableReasonV2::ProjectIntentUnavailable);
    }
    if provenance.base().base().settings().clip_coverage().state()
        != ResolvedEngineSettingsCoverageStateV2::Complete
    {
        atomic.push(PredictionUnavailableReasonV2::ResolvedSettingsOverflow);
    }
    atomic.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    atomic.dedup();
    if !atomic.is_empty() {
        let facets = prediction.facets();
        let valid_inventory = facets.len() == 1
            && facets[0].scope().code.as_str() == INVENTORY_SCOPE
            && facets[0].scope().subject.is_none()
            && facets[0].state() == EnginePredictionFacetStateV1::RequiredPredictionUnavailable
            && facets[0].reasons() == atomic
            && facets[0].basis() == &root_motion_inventory_basis(provenance)?;
        if !valid_inventory || !findings.is_empty() {
            return Err(PredictionContractError::InvalidMachineResult(
                "engine-root-motion atomic unavailable summary is not canonical",
            ));
        }
        return Ok(());
    }

    let configured_path = provenance
        .base()
        .base()
        .settings()
        .document_setting(EngineSettingIdV2::RootMotionSource)
        .and_then(|row| match row.value() {
            EngineSettingValueV2::SourceTransformPath(path) => {
                crate::RawTransformPathV1::parse(path).ok()
            }
            _ => None,
        });
    let path_resolution = configured_path
        .as_ref()
        .map(|path| provenance.raw_transform_paths().resolve(path));
    let root_name = rig.resolved_roles.get("root").map(String::as_str);
    let mut name_counts = BTreeMap::new();
    for clip in intent.clips() {
        if let Some(name) = clip.normalized_clip_name() {
            *name_counts.entry(name).or_insert(0usize) += 1;
        }
    }
    let mut expected = Vec::new();
    for clip in intent.clips() {
        let Some(name) = clip.normalized_clip_name() else {
            continue;
        };
        for (axis, owner) in [
            (
                crate::RootMotionAxisV1::HorizontalXz,
                clip.movement_owner_xz(),
            ),
            (crate::RootMotionAxisV1::VerticalY, clip.movement_owner_y()),
            (crate::RootMotionAxisV1::Yaw, clip.movement_owner_yaw()),
        ]
        .into_iter()
        .filter_map(|(axis, owner)| owner.map(|owner| (axis, owner)))
        {
            let axis_name = match axis {
                crate::RootMotionAxisV1::HorizontalXz => "horizontal_xz",
                crate::RootMotionAxisV1::VerticalY => "vertical_y",
                crate::RootMotionAxisV1::Yaw => "yaw",
            };
            let scope = EvaluationScope::new(crate::EvaluationScopeCode::custom(AXIS_SCOPE))
                .subject(format!(
                    "source_clip:{:020}:axis:{axis_name}",
                    clip.source_clip_index()
                ));
            let setting_id = match axis {
                crate::RootMotionAxisV1::HorizontalXz => EngineSettingIdV2::RootPositionXz,
                crate::RootMotionAxisV1::VerticalY => EngineSettingIdV2::RootPositionY,
                crate::RootMotionAxisV1::Yaw => EngineSettingIdV2::RootRotation,
            };
            let setting = provenance
                .base()
                .base()
                .settings()
                .clip_row(
                    clip.normalized_clip_index().ok_or(
                        PredictionContractError::InvalidMachineResult(
                            "mapped root-motion intent is missing normalized clip index",
                        ),
                    )?,
                    name,
                )
                .and_then(|row| row.setting(setting_id));
            let measurement = measurements.clips().get(name);
            let reason = if name_counts.get(name).copied().unwrap_or(0) > 1 {
                Some(PredictionUnavailableReasonV2::MeasurementUnavailable)
            } else {
                root_motion_unavailable_reason(
                    path_resolution.as_ref(),
                    intent.resolved_root_bone_index(),
                    root_name,
                    measurement,
                    setting.map(|row| row.value()),
                    axis,
                )?
            };
            let basis = root_motion_candidate_basis(
                provenance,
                clip,
                name,
                name_counts.get(name).copied().unwrap_or(0),
                axis,
                owner,
                configured_path.as_ref(),
                path_resolution.as_ref(),
                setting.map(|row| row.value_origin()),
                measurement,
            )?;
            expected.push((
                scope.clone(),
                name.to_owned(),
                axis,
                owner,
                setting.map(|row| row.value()).cloned(),
                reason,
                basis,
            ));
        }
    }
    let facets = prediction.facets();
    let has_budget = facets
        .last()
        .is_some_and(|facet| facet.scope().code.as_str() == BUDGET_SCOPE);
    let candidates = if has_budget {
        &facets[..facets.len() - 1]
    } else {
        facets
    };
    if candidates.len() > expected.len() || (candidates.len() < expected.len() && !has_budget) {
        return Err(PredictionContractError::InvalidMachineResult(
            "engine-root-motion facet allocation is not a canonical prefix",
        ));
    }
    if has_budget {
        let facet = facets.last().unwrap();
        if facet.state() != EnginePredictionFacetStateV1::RequiredPredictionUnavailable
            || facet.scope().subject.is_some()
            || facet.reasons() != [PredictionUnavailableReasonV2::FacetBudgetExceeded]
            || facet.basis() != &root_motion_static_basis()?
        {
            return Err(PredictionContractError::InvalidMachineResult(
                "engine-root-motion facet-budget summary is invalid",
            ));
        }
    }
    let mut retained_conflicts = Vec::new();
    for (facet, (scope, clip_name, axis, owner, setting, reason, basis)) in
        candidates.iter().zip(expected.iter())
    {
        if facet.scope() != scope || facet.basis() != basis {
            return Err(PredictionContractError::InvalidMachineResult(
                "engine-root-motion scope or basis is not canonical",
            ));
        }
        match reason {
            Some(reason)
                if facet.state() == EnginePredictionFacetStateV1::RequiredPredictionUnavailable
                    && facet.reasons() == std::slice::from_ref(reason) => {}
            None => {
                let disposition = root_motion_disposition(setting.as_ref().unwrap()).unwrap();
                let compatible = matches!(
                    (owner, disposition),
                    (
                        crate::RootMotionProjectOwnerV1::Gameplay,
                        crate::RootMotionImporterDispositionV1::BakedIntoPose
                    ) | (
                        crate::RootMotionProjectOwnerV1::Animation,
                        crate::RootMotionImporterDispositionV1::StoredAsRootMotion
                    )
                );
                let expected_result = crate::RootMotionRoutingResultV1 {
                    axis: *axis,
                    project_owner: *owner,
                    importer_disposition: disposition,
                    compatibility: if compatible {
                        crate::RootMotionCompatibilityV1::Compatible
                    } else {
                        crate::RootMotionCompatibilityV1::Conflict
                    },
                };
                if !matches!(facet.result(), Some(EngineMachineResultV1::RootMotionRouting(result)) if result == &expected_result)
                {
                    return Err(PredictionContractError::InvalidMachineResult(
                        "engine-root-motion result is not canonical",
                    ));
                }
                if !compatible {
                    let axis_label = match axis {
                        crate::RootMotionAxisV1::HorizontalXz => "horizontal XZ",
                        crate::RootMotionAxisV1::VerticalY => "vertical Y",
                        crate::RootMotionAxisV1::Yaw => "yaw",
                    };
                    let owner_label = match owner {
                        crate::RootMotionProjectOwnerV1::Gameplay => "gameplay",
                        crate::RootMotionProjectOwnerV1::Animation => "animation",
                    };
                    let disposition_label = match disposition {
                        crate::RootMotionImporterDispositionV1::BakedIntoPose => {
                            "baked into the pose"
                        }
                        crate::RootMotionImporterDispositionV1::StoredAsRootMotion => {
                            "stored as root motion"
                        }
                    };
                    retained_conflicts.push(serde_json::json!({
                        "check_id": "engine-root-motion",
                        "severity": "error",
                        "clip": clip_name,
                        "prediction_scope": scope,
                        "message": format!("clip {:?} assigns {} movement to {}, but Unity imports that axis as {}", clip_name, axis_label, owner_label, disposition_label),
                    }));
                }
            }
            _ => {
                return Err(PredictionContractError::InvalidMachineResult(
                    "engine-root-motion unavailable facet is not canonical",
                ));
            }
        }
    }
    let actual_findings = findings
        .iter()
        .map(RootMotionFindingEvidence::root_motion_wire_value)
        .collect::<Vec<_>>();
    if actual_findings != retained_conflicts {
        return Err(PredictionContractError::InvalidMachineResult(
            "engine-root-motion conflict findings are not canonical",
        ));
    }
    Ok(())
}

fn root_motion_lift(reference: PredictionBasisReferenceV1) -> PredictionBasisReferenceV4 {
    PredictionBasisReferenceV4::v2(PredictionBasisReferenceV2::v1(reference))
}

fn root_motion_static_basis() -> Result<EnginePredictionBasisV4, PredictionContractError> {
    let mut references = vec![root_motion_lift(PredictionBasisReferenceV1::profile_fact(
        "root_motion_addressability",
    )?)];
    for source in [
        "unity-fbx-model-importer-6000.3",
        "unity-fbx-animation-clip-6000.3",
        "unity-fbx-motion-node-6000.3",
    ] {
        references.push(root_motion_lift(
            PredictionBasisReferenceV1::primary_source(source)?,
        ));
    }
    for setting in [
        EngineSettingIdV2::AnimationType,
        EngineSettingIdV2::AvatarSetup,
        EngineSettingIdV2::ImportAnimation,
        EngineSettingIdV2::RootMotionSource,
    ] {
        references.push(root_motion_lift(
            PredictionBasisReferenceV1::resolved_setting(
                ResolvedSettingLocationV1::Document,
                setting.as_str(),
            )?,
        ));
    }
    EnginePredictionBasisV4::new(references)
}

fn root_motion_inventory_basis(
    provenance: &PredictionProvenanceV6,
) -> Result<EnginePredictionBasisV4, PredictionContractError> {
    let mut references = root_motion_static_basis()?.references().to_vec();
    let path_coverage = match provenance.raw_transform_paths().coverage() {
        crate::RawTransformPathCoverageV1::Complete => "complete",
        crate::RawTransformPathCoverageV1::Partial(_) => "partial",
        crate::RawTransformPathCoverageV1::Unavailable(_) => "unavailable",
    };
    let clip_coverage = match provenance.root_motion_project_intent().clip_coverage() {
        crate::EngineRootMotionProjectIntentCoverageV1::Complete => "complete",
        crate::EngineRootMotionProjectIntentCoverageV1::PartialProjectionBudgetExceeded => {
            "partial_projection_budget_exceeded"
        }
    };
    let count = match provenance
        .root_motion_project_intent()
        .declared_axis_candidates()
    {
        crate::EngineRootMotionProjectIntentCountV1::Exact { count } => {
            PredictionScalarV1::UnsignedInteger { value: count }
        }
        crate::EngineRootMotionProjectIntentCountV1::NPlusOne => {
            PredictionScalarV1::token("n_plus_one")?
        }
    };
    let unmapped_count = match provenance
        .root_motion_project_intent()
        .unmapped_declared_axis_candidates()
    {
        crate::EngineRootMotionProjectIntentCountV1::Exact { count } => {
            PredictionScalarV1::UnsignedInteger { value: count }
        }
        crate::EngineRootMotionProjectIntentCountV1::NPlusOne => {
            PredictionScalarV1::token("n_plus_one")?
        }
    };
    let raw_clip_coverage = match provenance
        .base()
        .base()
        .raw_source()
        .clips_coverage()
        .state()
    {
        RawSourceSetCoverageStateV1::Complete => "complete",
        RawSourceSetCoverageStateV1::Partial => "partial",
        RawSourceSetCoverageStateV1::Unavailable => "unavailable",
    };
    let settings_coverage = match provenance.base().base().settings().clip_coverage().state() {
        ResolvedEngineSettingsCoverageStateV2::Complete => "complete",
        ResolvedEngineSettingsCoverageStateV2::Partial => "partial",
    };
    for (field, value) in [
        (
            "raw_source.clips.coverage",
            PredictionScalarV1::token(raw_clip_coverage)?,
        ),
        (
            "raw_transform_path_inventory.coverage",
            PredictionScalarV1::token(path_coverage)?,
        ),
        (
            "root_motion_project_intent.clip_coverage",
            PredictionScalarV1::token(clip_coverage)?,
        ),
        ("root_motion_project_intent.declared_axis_candidates", count),
        (
            "root_motion_project_intent.unmapped_declared_axis_candidates",
            unmapped_count,
        ),
        (
            "resolved_settings.clips.coverage",
            PredictionScalarV1::token(settings_coverage)?,
        ),
    ] {
        references.push(root_motion_lift(PredictionBasisReferenceV1::project_field(
            field, value,
        )?));
    }
    EnginePredictionBasisV4::new(references)
}

fn root_motion_project_reference(
    field: &'static str,
    value: PredictionScalarV1,
) -> Result<PredictionBasisReferenceV4, PredictionContractError> {
    Ok(root_motion_lift(PredictionBasisReferenceV1::project_field(
        field, value,
    )?))
}

fn root_motion_token(value: &'static str) -> Result<PredictionScalarV1, PredictionContractError> {
    PredictionScalarV1::token(value)
}

#[allow(clippy::too_many_arguments)]
fn root_motion_candidate_basis(
    provenance: &PredictionProvenanceV6,
    clip: &crate::EngineRootMotionClipIntentV1,
    name: &str,
    duplicate_count: usize,
    axis: crate::RootMotionAxisV1,
    owner: crate::RootMotionProjectOwnerV1,
    configured_path: Option<&crate::RawTransformPathV1>,
    path_resolution: Option<&crate::RawTransformPathResolutionV1>,
    setting_origin: Option<crate::EngineSettingValueOriginV3>,
    measurement: Option<&ClipMeasurements>,
) -> Result<EnginePredictionBasisV4, PredictionContractError> {
    let mut refs = root_motion_static_basis()?.references().to_vec();
    let settings = provenance.base().base().settings();
    for (id, field) in [
        (
            EngineSettingIdV2::AnimationType,
            "resolved_setting.document.animation_type.value_origin",
        ),
        (
            EngineSettingIdV2::AvatarSetup,
            "resolved_setting.document.avatar_setup.value_origin",
        ),
        (
            EngineSettingIdV2::ImportAnimation,
            "resolved_setting.document.import_animation.value_origin",
        ),
        (
            EngineSettingIdV2::RootMotionSource,
            "resolved_setting.document.root_motion_source.value_origin",
        ),
    ] {
        let value = settings
            .document_setting(id)
            .map_or(PredictionScalarV1::Null, |row| {
                root_motion_token(match row.value_origin() {
                    crate::EngineSettingValueOriginV3::ExplicitConfig => "explicit_config",
                    crate::EngineSettingValueOriginV3::ProfileDefault => "profile_default",
                })
                .expect("static origin token")
            });
        refs.push(root_motion_project_reference(field, value)?);
    }
    let setting_id = match axis {
        crate::RootMotionAxisV1::HorizontalXz => EngineSettingIdV2::RootPositionXz,
        crate::RootMotionAxisV1::VerticalY => EngineSettingIdV2::RootPositionY,
        crate::RootMotionAxisV1::Yaw => EngineSettingIdV2::RootRotation,
    };
    if let Some(index) = clip.normalized_clip_index() {
        refs.push(root_motion_lift(
            PredictionBasisReferenceV1::resolved_setting(
                ResolvedSettingLocationV1::Clip {
                    clip_ordinal: index,
                    clip_name: name.to_owned(),
                },
                setting_id.as_str(),
            )?,
        ));
    }
    refs.push(root_motion_project_reference(
        "root_motion_project_intent.source_clip_index",
        PredictionScalarV1::UnsignedInteger {
            value: clip.source_clip_index(),
        },
    )?);
    let mapping_state = match clip.normalized_clip_mapping_state() {
        crate::EngineRootMotionClipMappingStateV1::Observed => "observed",
        crate::EngineRootMotionClipMappingStateV1::ProvenAbsent => "proven_absent",
        crate::EngineRootMotionClipMappingStateV1::Unavailable => "unavailable",
    };
    for field in ["normalized_clip_index.state", "normalized_clip_index.value"] {
        if field.ends_with(".value") && clip.normalized_clip_index().is_none() {
            continue;
        }
        let value = if field.ends_with(".state") {
            root_motion_token(mapping_state)?
        } else {
            PredictionScalarV1::UnsignedInteger {
                value: clip.normalized_clip_index().unwrap(),
            }
        };
        let raw: RawSourceBasisReferenceV1 = serde_json::from_value(serde_json::json!({
            "domain": "clip", "key": {"kind": "clip", "source_clip_index": clip.source_clip_index()}, "field": field, "value": value
        })).map_err(|_| PredictionContractError::InvalidMachineResult("failed to reconstruct root-motion raw clip reference"))?;
        refs.push(root_motion_lift(PredictionBasisReferenceV1::raw_source(
            raw,
        )));
    }
    let axis_name = match axis {
        crate::RootMotionAxisV1::HorizontalXz => "horizontal_xz",
        crate::RootMotionAxisV1::VerticalY => "vertical_y",
        crate::RootMotionAxisV1::Yaw => "yaw",
    };
    let owner_name = match owner {
        crate::RootMotionProjectOwnerV1::Gameplay => "gameplay",
        crate::RootMotionProjectOwnerV1::Animation => "animation",
    };
    let origin = setting_origin.map_or(PredictionScalarV1::Null, |origin| {
        root_motion_token(match origin {
            crate::EngineSettingValueOriginV3::ExplicitConfig => "explicit_config",
            crate::EngineSettingValueOriginV3::ProfileDefault => "profile_default",
        })
        .expect("origin token")
    });
    let path_coverage = match provenance.raw_transform_paths().coverage() {
        crate::RawTransformPathCoverageV1::Complete => "complete",
        crate::RawTransformPathCoverageV1::Partial(_) => "partial",
        crate::RawTransformPathCoverageV1::Unavailable(_) => "unavailable",
    };
    let raw_coverage = match provenance
        .base()
        .base()
        .raw_source()
        .clips_coverage()
        .state()
    {
        RawSourceSetCoverageStateV1::Complete => "complete",
        RawSourceSetCoverageStateV1::Partial => "partial",
        RawSourceSetCoverageStateV1::Unavailable => "unavailable",
    };
    let resolution = match path_resolution {
        Some(crate::RawTransformPathResolutionV1::Exact(_)) => "exact",
        Some(crate::RawTransformPathResolutionV1::NoMatch) => "no_match",
        Some(crate::RawTransformPathResolutionV1::Ambiguous { .. }) => "ambiguous",
        Some(crate::RawTransformPathResolutionV1::CoverageIncomplete { .. }) => {
            "coverage_incomplete"
        }
        None => "invalid_configured_path",
    };
    for (field, value) in [
        (
            "root_motion_project_intent.clip_mapping_state",
            root_motion_token(mapping_state)?,
        ),
        (
            "root_motion_project_intent.normalized_clip_index",
            clip.normalized_clip_index()
                .map_or(PredictionScalarV1::Null, |value| {
                    PredictionScalarV1::UnsignedInteger { value }
                }),
        ),
        (
            "root_motion_project_intent.normalized_clip_name",
            PredictionScalarV1::text(name)?,
        ),
        (
            "measurement.clip_name_match_count",
            PredictionScalarV1::UnsignedInteger {
                value: duplicate_count as u64,
            },
        ),
        (
            "measurement.clip_identity_state",
            root_motion_token(if duplicate_count > 1 {
                "duplicate"
            } else {
                "unique"
            })?,
        ),
        (
            "root_motion_project_intent.axis",
            root_motion_token(axis_name)?,
        ),
        (
            "root_motion_project_intent.owner",
            root_motion_token(owner_name)?,
        ),
        ("resolved_setting.clip.value_origin", origin),
        (
            "raw_source.clips.coverage",
            root_motion_token(raw_coverage)?,
        ),
        (
            "raw_transform_path_inventory.coverage",
            root_motion_token(path_coverage)?,
        ),
        (
            "resolved_role.root.bone_index",
            provenance
                .root_motion_project_intent()
                .resolved_root_bone_index()
                .map_or(PredictionScalarV1::Null, |value| {
                    PredictionScalarV1::UnsignedInteger { value }
                }),
        ),
        (
            "root_motion_source.configured_path",
            configured_path.map_or(PredictionScalarV1::Null, |path| {
                PredictionScalarV1::text(path.as_str()).expect("path text")
            }),
        ),
        (
            "root_motion_source.resolution_state",
            root_motion_token(resolution)?,
        ),
    ] {
        refs.push(root_motion_project_reference(field, value)?);
    }
    if let Some(crate::RawTransformPathResolutionV1::Ambiguous { matches }) = path_resolution {
        refs.push(root_motion_project_reference(
            "root_motion_source.match_count",
            PredictionScalarV1::UnsignedInteger {
                value: matches.len() as u64,
            },
        )?);
    }
    if let Some(crate::RawTransformPathResolutionV1::Exact(path_match)) = path_resolution {
        let digest = crate::sha256_hex(
            &serde_json::to_vec(path_match.parent_chain()).expect("parent chain serializes"),
        );
        for (field, value) in [
            (
                "root_motion_source.source_node_index",
                PredictionScalarV1::UnsignedInteger {
                    value: path_match.source_node_index(),
                },
            ),
            (
                "root_motion_source.projected_bone_index",
                path_match
                    .projected_bone_index()
                    .map_or(PredictionScalarV1::Null, |value| {
                        PredictionScalarV1::UnsignedInteger { value }
                    }),
            ),
            (
                "root_motion_source.path",
                PredictionScalarV1::text(path_match.path().as_str())?,
            ),
            ("root_motion_source.node_kind", root_motion_token("source")?),
            (
                "root_motion_source.parent_chain_count",
                PredictionScalarV1::UnsignedInteger {
                    value: path_match.parent_chain().len() as u64,
                },
            ),
            (
                "root_motion_source.parent_chain_sha256",
                PredictionScalarV1::text(digest)?,
            ),
        ] {
            refs.push(root_motion_project_reference(field, value)?);
        }
    }
    if duplicate_count == 1
        && let Some(measurement) = measurement
    {
        let escaped = name.replace('~', "~0").replace('/', "~1");
        let prefix = format!("/measurements/clips/{escaped}/root_trajectory");
        let availability = |value| {
            root_motion_token(match value {
                MeasurementAvailability::Measured => "measured",
                MeasurementAvailability::NotApplicable => "not_applicable",
                MeasurementAvailability::Unavailable => "unavailable",
            })
        };
        let mut add_measurement = |pointer: String, value| -> Result<(), PredictionContractError> {
            refs.push(root_motion_lift(
                PredictionBasisReferenceV1::measurement_v16(
                    crate::MeasurementPointerV1::new(pointer)?,
                    value,
                ),
            ));
            Ok(())
        };
        add_measurement(
            format!("{prefix}_availability"),
            availability(measurement.root_trajectory_availability)?,
        )?;
        if let Some(trajectory) = measurement.root_trajectory.as_ref() {
            add_measurement(
                format!("{prefix}/bone_index"),
                PredictionScalarV1::UnsignedInteger {
                    value: u64::from(trajectory.bone_index),
                },
            )?;
            add_measurement(
                format!("{prefix}/source_role"),
                root_motion_token(trajectory.source_role.as_str())?,
            )?;
            let (suffix, present_field, availability_value, present) = match axis {
                crate::RootMotionAxisV1::HorizontalXz | crate::RootMotionAxisV1::VerticalY => (
                    "translation_availability",
                    "measurement.root_translation_present",
                    trajectory.translation_availability,
                    trajectory.translation.is_some(),
                ),
                crate::RootMotionAxisV1::Yaw => (
                    "yaw_availability",
                    "measurement.root_yaw_present",
                    trajectory.yaw_availability,
                    trajectory.yaw.is_some(),
                ),
            };
            add_measurement(
                format!("{prefix}/{suffix}"),
                availability(availability_value)?,
            )?;
            refs.push(root_motion_project_reference(
                present_field,
                PredictionScalarV1::Boolean { value: present },
            )?);
        }
    }
    EnginePredictionBasisV4::new(refs)
}

trait RootMotionFindingEvidence {
    fn root_motion_wire_value(&self) -> serde_json::Value;
}

impl RootMotionFindingEvidence for crate::Finding {
    fn root_motion_wire_value(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("Finding serializes")
    }
}

impl RootMotionFindingEvidence for PredictionFindingInput {
    fn root_motion_wire_value(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("finding input serializes")
    }
}

fn root_motion_disposition(
    setting: &EngineSettingValueV2,
) -> Option<crate::RootMotionImporterDispositionV1> {
    match setting {
        EngineSettingValueV2::BakeOrExtract(crate::EngineBakeOrExtractV1::Bake) => {
            Some(crate::RootMotionImporterDispositionV1::BakedIntoPose)
        }
        EngineSettingValueV2::BakeOrExtract(crate::EngineBakeOrExtractV1::Extract) => {
            Some(crate::RootMotionImporterDispositionV1::StoredAsRootMotion)
        }
        _ => None,
    }
}

fn root_motion_unavailable_reason(
    path_resolution: Option<&crate::RawTransformPathResolutionV1>,
    resolved_root_bone_index: Option<u64>,
    root_name: Option<&str>,
    measurement: Option<&ClipMeasurements>,
    setting: Option<&EngineSettingValueV2>,
    axis: crate::RootMotionAxisV1,
) -> Result<Option<PredictionUnavailableReasonV2>, PredictionContractError> {
    let path_match = match path_resolution {
        Some(crate::RawTransformPathResolutionV1::Exact(path_match)) => path_match,
        Some(crate::RawTransformPathResolutionV1::NoMatch) | None => {
            return Ok(Some(PredictionUnavailableReasonV2::SourceSelectorNoMatch));
        }
        Some(crate::RawTransformPathResolutionV1::Ambiguous { .. }) => {
            return Ok(Some(PredictionUnavailableReasonV2::SourceSelectorAmbiguous));
        }
        Some(crate::RawTransformPathResolutionV1::CoverageIncomplete { .. }) => {
            return Ok(Some(PredictionUnavailableReasonV2::RawSourceIncomplete));
        }
    };
    if resolved_root_bone_index.is_none()
        || path_match.projected_bone_index() != resolved_root_bone_index
    {
        return Ok(Some(PredictionUnavailableReasonV2::custom(
            "animsmith:root_motion_source_not_explicit_root",
        )?));
    }
    let Some(trajectory) = measurement
        .filter(|measurement| {
            measurement.root_trajectory_availability == MeasurementAvailability::Measured
        })
        .and_then(|measurement| measurement.root_trajectory.as_ref())
    else {
        return Ok(Some(PredictionUnavailableReasonV2::MeasurementUnavailable));
    };
    if trajectory.source_role != crate::measure::RootTrajectorySourceRole::Root
        || root_name != Some(trajectory.bone_name.as_str())
        || resolved_root_bone_index != Some(u64::from(trajectory.bone_index))
    {
        return Ok(Some(PredictionUnavailableReasonV2::custom(
            "animsmith:root_motion_source_not_explicit_root",
        )?));
    }
    let measured = match axis {
        crate::RootMotionAxisV1::HorizontalXz | crate::RootMotionAxisV1::VerticalY => {
            trajectory.translation_availability == MeasurementAvailability::Measured
                && trajectory.translation.is_some()
        }
        crate::RootMotionAxisV1::Yaw => {
            trajectory.yaw_availability == MeasurementAvailability::Measured
                && trajectory.yaw.is_some()
        }
    };
    if !measured {
        return Ok(Some(PredictionUnavailableReasonV2::MeasurementUnavailable));
    }
    if setting.and_then(root_motion_disposition).is_none() {
        return Ok(Some(
            PredictionUnavailableReasonV2::ResolvedSettingsOverflow,
        ));
    }
    Ok(None)
}

fn track_scope(code: &'static str, subject: String) -> EvaluationScope {
    EvaluationScope::new(crate::evaluation::EvaluationScopeCode::custom(code)).subject(subject)
}

fn track_subject_facet(
    scope: EvaluationScope,
    basis: EnginePredictionBasisV4,
    subject_kind: SourceImportSubjectKindV1,
    gate: Option<EngineSettingIdV2>,
) -> crate::EnginePredictionFacetV4 {
    match gate {
        None => track_unavailable(
            scope,
            basis,
            PredictionUnavailableReasonV2::RuntimeAnimationSurvivalUnavailable,
        ),
        Some(controlling_gate) => crate::EnginePredictionFacetV4::available(
            scope,
            basis,
            EngineMachineResultV1::SourceImportDisposition(SourceImportDispositionResultV1 {
                subject_kind,
                disposition: SourceImportDispositionV1::Dropped,
                controlling_gate: Some(controlling_gate),
            }),
        )
        .expect("reconstructed track result is valid"),
    }
}

fn track_unavailable(
    scope: EvaluationScope,
    basis: EnginePredictionBasisV4,
    reason: PredictionUnavailableReasonV2,
) -> crate::EnginePredictionFacetV4 {
    crate::EnginePredictionFacetV4::required_unavailable(scope, basis, vec![reason])
        .expect("reconstructed track unavailable facet is valid")
}

fn track_gate(provenance: &PredictionProvenanceV5) -> Option<EngineSettingIdV2> {
    let settings = provenance.base().settings();
    if matches!(
        settings
            .document_setting(EngineSettingIdV2::BevyAnimationFeature)
            .map(|row| row.value()),
        Some(EngineSettingValueV2::Boolean(false))
    ) {
        Some(EngineSettingIdV2::BevyAnimationFeature)
    } else if matches!(
        settings
            .document_setting(EngineSettingIdV2::LoadAnimations)
            .map(|row| row.value()),
        Some(EngineSettingValueV2::Boolean(false))
    ) {
        Some(EngineSettingIdV2::LoadAnimations)
    } else {
        None
    }
}

fn track_static_basis() -> EnginePredictionBasisV4 {
    let v1 = |reference| PredictionBasisReferenceV4::v2(PredictionBasisReferenceV2::v1(reference));
    EnginePredictionBasisV4::new(vec![
        v1(PredictionBasisReferenceV1::profile_fact("source_import_disposition").unwrap()),
        v1(PredictionBasisReferenceV1::primary_source("bevy-gltf-loader-0.19.0-c6f634ca").unwrap()),
        v1(
            PredictionBasisReferenceV1::primary_source("bevy-feature-manifest-0.19.0-c6f634ca")
                .unwrap(),
        ),
        v1(PredictionBasisReferenceV1::resolved_setting(
            ResolvedSettingLocationV1::Document,
            EngineSettingIdV2::BevyAnimationFeature.as_str(),
        )
        .unwrap()),
        v1(PredictionBasisReferenceV1::resolved_setting(
            ResolvedSettingLocationV1::Document,
            EngineSettingIdV2::LoadAnimations.as_str(),
        )
        .unwrap()),
    ])
    .unwrap()
}

fn track_inventory_basis(inventory: &RawAnimationChannelInventoryV1) -> EnginePredictionBasisV4 {
    let mut references = track_static_basis().references().to_vec();
    references.push(PredictionBasisReferenceV4::v2(
        PredictionBasisReferenceV2::v1(
            PredictionBasisReferenceV1::project_field(
                "raw_animation_channel_inventory.animation_coverage",
                PredictionScalarV1::text(match inventory.animation_coverage().state() {
                    RawSourceSetCoverageStateV1::Complete => "complete",
                    RawSourceSetCoverageStateV1::Partial => "partial",
                    RawSourceSetCoverageStateV1::Unavailable => "unavailable",
                })
                .unwrap(),
            )
            .unwrap(),
        ),
    ));
    references.push(PredictionBasisReferenceV4::v2(
        PredictionBasisReferenceV2::v1(
            PredictionBasisReferenceV1::project_field(
                "raw_animation_channel_inventory.source_coverage_complete",
                PredictionScalarV1::Boolean {
                    value: inventory.source_coverage_complete(),
                },
            )
            .unwrap(),
        ),
    ));
    if let Some(row) = inventory.rows().iter().find(|row| {
        row.channel_coverage()
            .is_some_and(|coverage| coverage.state() != RawSourceSetCoverageStateV1::Complete)
    }) {
        let coverage = row.channel_coverage().unwrap();
        let state = match coverage.state() {
            RawSourceSetCoverageStateV1::Complete => "complete",
            RawSourceSetCoverageStateV1::Partial => "partial",
            RawSourceSetCoverageStateV1::Unavailable => "unavailable",
        };
        references.push(PredictionBasisReferenceV4::v2(
            PredictionBasisReferenceV2::v1(
                PredictionBasisReferenceV1::project_field(
                    "raw_animation_channel_inventory.incomplete_channel_animation_row",
                    PredictionScalarV1::UnsignedInteger {
                        value: row.source_animation_index(),
                    },
                )
                .unwrap(),
            ),
        ));
        references.push(PredictionBasisReferenceV4::v2(
            PredictionBasisReferenceV2::v1(
                PredictionBasisReferenceV1::project_field(
                    "raw_animation_channel_inventory.incomplete_channel_coverage",
                    PredictionScalarV1::text(state).unwrap(),
                )
                .unwrap(),
            ),
        ));
        if let Some(reason) = coverage.reason() {
            let reason = serde_json::to_value(reason).unwrap();
            references.push(PredictionBasisReferenceV4::v2(
                PredictionBasisReferenceV2::v1(
                    PredictionBasisReferenceV1::project_field(
                        "raw_animation_channel_inventory.incomplete_channel_reason",
                        PredictionScalarV1::text(reason.as_str().unwrap()).unwrap(),
                    )
                    .unwrap(),
                ),
            ));
        }
    }
    EnginePredictionBasisV4::new(references).unwrap()
}

fn track_row_basis(
    inventory: &RawAnimationChannelInventoryV1,
    animation: u64,
    channel: Option<u64>,
) -> EnginePredictionBasisV4 {
    let mut references = track_inventory_basis(inventory).references().to_vec();
    references.push(PredictionBasisReferenceV4::v2(
        PredictionBasisReferenceV2::v1(
            PredictionBasisReferenceV1::project_field(
                "raw_animation_channel_inventory.animation_row",
                PredictionScalarV1::UnsignedInteger { value: animation },
            )
            .unwrap(),
        ),
    ));
    if let Some(channel) = channel {
        references.push(PredictionBasisReferenceV4::v2(
            PredictionBasisReferenceV2::v1(
                PredictionBasisReferenceV1::project_field(
                    "raw_animation_channel_inventory.channel_row",
                    PredictionScalarV1::UnsignedInteger { value: channel },
                )
                .unwrap(),
            ),
        ));
    }
    EnginePredictionBasisV4::new(references).unwrap()
}

const ENGINE_UNIT_SCALE_CHECK_ID: &str = "engine-unit-scale";
const ENGINE_UNIT_SCALE_FILE_SCOPE: &str = "engine-unit-scale:file-unit";
const ENGINE_UNIT_SCALE_SCENE_SCOPE: &str = "engine-unit-scale:loader-scene-root";
const ENGINE_UNIT_SCALE_SCENE_INVENTORY_SCOPE: &str = "engine-unit-scale:scene-inventory";
const ENGINE_UNIT_SCALE_MESH_SCOPE: &str = "engine-unit-scale:loader-mesh-primitive";
const ENGINE_UNIT_SCALE_MESH_INVENTORY_SCOPE: &str = "engine-unit-scale:mesh-inventory";
const ENGINE_UNIT_SCALE_SELECTED_SCOPE: &str = "engine-unit-scale:selected-source-node";
const ENGINE_UNIT_SCALE_BUDGET_SCOPE: &str = "engine-unit-scale:facet-budget";
const ENGINE_UNIT_SCALE_SELECTED_REACHABILITY_UNAVAILABLE_REASON: &str =
    "animsmith:selected_node_scene_reachability_unavailable";
const ENGINE_UNIT_SCALE_SELECTED_MAX_REACHABILITY_NODES: usize = 128;

#[derive(Clone, PartialEq, Eq)]
struct ExpectedUnitScaleFacet {
    scope: EvaluationScope,
    result: Option<EngineMachineResultV1>,
    reasons: Vec<PredictionUnavailableReasonV2>,
}

fn unit_scale_scope(code: &'static str, subject: Option<String>) -> EvaluationScope {
    let scope = EvaluationScope::new(crate::evaluation::EvaluationScopeCode::custom(code));
    subject.map_or(scope.clone(), |subject| scope.subject(subject))
}

fn unit_scale_unavailable_reasons(
    reason: PredictionUnavailableReasonV2,
    dependency_complete: bool,
) -> Vec<PredictionUnavailableReasonV2> {
    let mut reasons = vec![reason];
    if !dependency_complete {
        reasons.push(PredictionUnavailableReasonV2::DependencyClosureIncomplete);
    }
    reasons.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    reasons
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum UnitScaleSelectedReachability {
    Reachable(u64, u64),
    Unreachable,
    Unavailable,
    WorkBudgetExceeded,
}

enum UnitScaleSelectedReachabilityPlan {
    Complete {
        node_index: u64,
        scene_witnesses: BTreeMap<u64, (u64, u64)>,
    },
    Refused {
        node_index: u64,
    },
}

enum UnitScaleSelectedReachabilityPlans {
    Complete {
        plans: BTreeMap<String, UnitScaleSelectedReachabilityPlan>,
        selected_facet_count: usize,
    },
    Overflow {
        plans: BTreeMap<String, UnitScaleSelectedReachabilityPlan>,
    },
}

impl UnitScaleSelectedReachabilityPlans {
    fn get(&self, selector: &str) -> Option<&UnitScaleSelectedReachabilityPlan> {
        match self {
            Self::Complete { plans, .. } | Self::Overflow { plans } => plans.get(selector),
        }
    }

    const fn selected_facet_count(&self) -> usize {
        match self {
            Self::Complete {
                selected_facet_count,
                ..
            } => *selected_facet_count,
            Self::Overflow { .. } => PREDICTION_V2_MAX_CANDIDATE_FACETS_PER_RULE + 1,
        }
    }
}

impl UnitScaleSelectedReachabilityPlan {
    const fn node_index(&self) -> u64 {
        match self {
            Self::Complete { node_index, .. } | Self::Refused { node_index } => *node_index,
        }
    }
}

fn unit_scale_selected_reachability_budget_exceeded(work: &mut usize) -> bool {
    *work = work.saturating_add(1);
    *work > PREDICTION_V2_MAX_CANDIDATE_FACETS_PER_RULE
}

fn unit_scale_selected_reachability(
    nodes: &BTreeMap<usize, &crate::measure::SkeletonNodeMeasurements>,
    start: u64,
    roots: &[u64],
    work: &mut usize,
) -> UnitScaleSelectedReachability {
    let Ok(start) = usize::try_from(start) else {
        return UnitScaleSelectedReachability::Unavailable;
    };
    if *work >= PREDICTION_V2_MAX_CANDIDATE_FACETS_PER_RULE {
        return UnitScaleSelectedReachability::WorkBudgetExceeded;
    }
    let roots = roots
        .iter()
        .copied()
        .enumerate()
        .map(|(ordinal, node)| (node, ordinal as u64))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    let mut current = Some(start);
    for _ in 0..ENGINE_UNIT_SCALE_SELECTED_MAX_REACHABILITY_NODES {
        let Some(index) = current else {
            return UnitScaleSelectedReachability::Unreachable;
        };
        if unit_scale_selected_reachability_budget_exceeded(work) {
            return UnitScaleSelectedReachability::WorkBudgetExceeded;
        }
        if let Some(ordinal) = roots.get(&(index as u64)) {
            return UnitScaleSelectedReachability::Reachable(*ordinal, index as u64);
        }
        if !seen.insert(index) {
            return UnitScaleSelectedReachability::Unavailable;
        }
        current = match nodes.get(&index) {
            Some(node) => node.parent_node_index,
            None => return UnitScaleSelectedReachability::Unavailable,
        };
    }
    match current {
        Some(_) => UnitScaleSelectedReachability::Unavailable,
        None => UnitScaleSelectedReachability::Unreachable,
    }
}

fn unit_scale_selected_reachability_plans(
    provenance: &PredictionProvenanceV4,
    measurements: &MeasurementContract,
) -> UnitScaleSelectedReachabilityPlans {
    let assets = measurements.assets();
    let inventory = provenance
        .raw_scene_attachment()
        .inventory()
        .filter(|inventory| {
            assets.skeleton_source_coverage == SourceSkeletonCoverage::Complete
                && inventory.scenes().coverage() == RawSceneAttachmentCoverageV1::Complete
                && inventory.source_skeleton().coverage() == RawSceneAttachmentCoverageV1::Complete
        });
    let nodes = assets
        .skeleton_nodes
        .iter()
        .map(|node| (node.node_index, node))
        .collect::<BTreeMap<_, _>>();
    let mut plans = BTreeMap::new();
    let mut selected_facet_count = 0usize;
    for selector in provenance.rule_inputs().runtime_node_selectors() {
        let Some(inventory) = inventory else {
            selected_facet_count = selected_facet_count.saturating_add(1);
            if selected_facet_count > PREDICTION_V2_MAX_CANDIDATE_FACETS_PER_RULE {
                return UnitScaleSelectedReachabilityPlans::Overflow { plans };
            }
            continue;
        };
        let mut matches = assets.skeleton_nodes.iter().filter(|node| {
            node.name
                .as_deref()
                .is_some_and(|name| crate::config::glob_match(selector, name))
        });
        let Some(node) = matches.next() else {
            selected_facet_count = selected_facet_count.saturating_add(1);
            if selected_facet_count > PREDICTION_V2_MAX_CANDIDATE_FACETS_PER_RULE {
                return UnitScaleSelectedReachabilityPlans::Overflow { plans };
            }
            continue;
        };
        if matches.next().is_some() {
            selected_facet_count = selected_facet_count.saturating_add(1);
            if selected_facet_count > PREDICTION_V2_MAX_CANDIDATE_FACETS_PER_RULE {
                return UnitScaleSelectedReachabilityPlans::Overflow { plans };
            }
            continue;
        }
        let node_index = node.node_index as u64;
        let mut work = 0usize;
        let remaining_capacity =
            PREDICTION_V2_MAX_CANDIDATE_FACETS_PER_RULE.saturating_sub(selected_facet_count);
        let mut would_overflow = false;
        let mut plan = UnitScaleSelectedReachabilityPlan::Complete {
            node_index,
            scene_witnesses: BTreeMap::new(),
        };
        for scene in inventory.scenes().rows() {
            match unit_scale_selected_reachability(
                &nodes,
                node_index,
                scene.root_node_indices(),
                &mut work,
            ) {
                UnitScaleSelectedReachability::Reachable(root_ordinal, root_node_index) => {
                    let UnitScaleSelectedReachabilityPlan::Complete {
                        scene_witnesses, ..
                    } = &mut plan
                    else {
                        unreachable!("a reachable scene cannot follow a refusal");
                    };
                    if scene_witnesses.len() < remaining_capacity {
                        scene_witnesses
                            .insert(scene.source_scene_index(), (root_ordinal, root_node_index));
                    } else {
                        would_overflow = true;
                    }
                }
                UnitScaleSelectedReachability::Unreachable => {}
                UnitScaleSelectedReachability::Unavailable
                | UnitScaleSelectedReachability::WorkBudgetExceeded => {
                    plan = UnitScaleSelectedReachabilityPlan::Refused { node_index };
                    break;
                }
            }
        }
        if matches!(&plan, UnitScaleSelectedReachabilityPlan::Complete { .. }) && would_overflow {
            return UnitScaleSelectedReachabilityPlans::Overflow { plans };
        }
        let facets = match &plan {
            UnitScaleSelectedReachabilityPlan::Complete {
                scene_witnesses, ..
            } => scene_witnesses.len().max(1),
            UnitScaleSelectedReachabilityPlan::Refused { .. } => 1,
        };
        selected_facet_count = selected_facet_count.saturating_add(facets);
        if selected_facet_count > PREDICTION_V2_MAX_CANDIDATE_FACETS_PER_RULE {
            return UnitScaleSelectedReachabilityPlans::Overflow { plans };
        }
        plans.insert(selector.clone(), plan);
    }
    UnitScaleSelectedReachabilityPlans::Complete {
        plans,
        selected_facet_count,
    }
}

fn unit_scale_selected_ancestry_reason(
    nodes: &BTreeMap<usize, &crate::measure::SkeletonNodeMeasurements>,
    start: usize,
) -> Option<PredictionUnavailableReasonV2> {
    let mut current = start;
    let mut seen = BTreeSet::new();
    for _ in 0..128 {
        if !seen.insert(current) {
            return Some(
                PredictionUnavailableReasonV2::custom(
                    "animsmith:selected_node_ancestry_unavailable",
                )
                .expect("static reason is valid"),
            );
        }
        let Some(node) = nodes.get(&current) else {
            return Some(
                PredictionUnavailableReasonV2::custom(
                    "animsmith:selected_node_ancestry_unavailable",
                )
                .expect("static reason is valid"),
            );
        };
        match node.local_rest {
            SkeletonNodeLocalRestMeasurements::Matrix { .. } => {
                return Some(
                    PredictionUnavailableReasonV2::custom(
                        "animsmith:matrix_authored_selected_node_or_ancestry",
                    )
                    .expect("static reason is valid"),
                );
            }
            SkeletonNodeLocalRestMeasurements::Unavailable { .. } => {
                return Some(
                    PredictionUnavailableReasonV2::custom(
                        "animsmith:selected_node_ancestry_unavailable",
                    )
                    .expect("static reason is valid"),
                );
            }
            SkeletonNodeLocalRestMeasurements::Trs { .. } => {}
        }
        let parent = node.parent_node_index?;
        current = parent;
    }
    Some(
        PredictionUnavailableReasonV2::custom("animsmith:selected_node_ancestry_unavailable")
            .expect("static reason is valid"),
    )
}

#[derive(Clone, Copy)]
struct CurrentUnitScaleMeshRow {
    source_scene_index: u64,
    source_root_ordinal: u64,
    root_node_index: u64,
    source_node_index: u64,
    source_mesh_index: u64,
    source_primitive_index: u64,
}

enum CurrentUnitScaleMeshPlan {
    Detailed(Vec<CurrentUnitScaleMeshRow>),
    CompleteEmpty,
    Incomplete,
    JoinOverflow,
}

fn unit_scale_join_budget_exceeded(work: &mut usize) -> bool {
    *work = work.saturating_add(1);
    *work > PREDICTION_V2_MAX_CANDIDATE_FACETS_PER_RULE
}

fn unit_scale_reachable_root_indexed(
    start: u64,
    roots: &BTreeMap<u64, u64>,
    parents: &BTreeMap<u64, Option<u64>>,
    work: &mut usize,
) -> Result<Option<(u64, u64)>, ()> {
    let mut seen = BTreeSet::new();
    let mut current = Some(start);
    while let Some(index) = current {
        if unit_scale_join_budget_exceeded(work) {
            return Err(());
        }
        if let Some(ordinal) = roots.get(&index) {
            return Ok(Some((*ordinal, index)));
        }
        if !seen.insert(index) {
            return Ok(None);
        }
        current = match parents.get(&index) {
            Some(parent) => *parent,
            None => return Ok(None),
        };
    }
    Ok(None)
}

fn current_unit_scale_mesh_plan(
    provenance: &PredictionProvenanceV4,
    measurements: &MeasurementContract,
) -> CurrentUnitScaleMeshPlan {
    let Some(inventory) = provenance.raw_scene_attachment().inventory() else {
        return CurrentUnitScaleMeshPlan::Incomplete;
    };
    if inventory.scenes().coverage() != RawSceneAttachmentCoverageV1::Complete
        || inventory.node_mesh_attachments().coverage() != RawSceneAttachmentCoverageV1::Complete
        || inventory.mesh_primitives().coverage() != RawSceneAttachmentCoverageV1::Complete
        || inventory.source_skeleton().coverage() != RawSceneAttachmentCoverageV1::Complete
        || measurements.assets().skeleton_source_coverage != SourceSkeletonCoverage::Complete
    {
        return CurrentUnitScaleMeshPlan::Incomplete;
    }
    let parents = measurements
        .assets()
        .skeleton_nodes
        .iter()
        .map(|node| {
            (
                node.node_index as u64,
                node.parent_node_index.map(|parent| parent as u64),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let scene_roots = inventory
        .scenes()
        .rows()
        .iter()
        .map(|scene| {
            (
                scene.source_scene_index(),
                scene
                    .root_node_indices()
                    .iter()
                    .copied()
                    .enumerate()
                    .map(|(ordinal, node)| (node, ordinal as u64))
                    .collect::<BTreeMap<_, _>>(),
            )
        })
        .collect::<Vec<_>>();
    let primitives_by_mesh = inventory.mesh_primitives().rows().iter().fold(
        BTreeMap::<u64, Vec<u64>>::new(),
        |mut grouped, primitive| {
            grouped
                .entry(primitive.source_mesh_index())
                .or_default()
                .push(primitive.source_primitive_index());
            grouped
        },
    );
    let mut work = 0usize;
    let mut rows = Vec::new();
    for (source_scene_index, roots) in scene_roots {
        for attachment in inventory.node_mesh_attachments().rows() {
            if unit_scale_join_budget_exceeded(&mut work) {
                return CurrentUnitScaleMeshPlan::JoinOverflow;
            }
            let Ok(reachable) = unit_scale_reachable_root_indexed(
                attachment.source_node_index(),
                &roots,
                &parents,
                &mut work,
            ) else {
                return CurrentUnitScaleMeshPlan::JoinOverflow;
            };
            let Some((source_root_ordinal, root_node_index)) = reachable else {
                continue;
            };
            for &source_primitive_index in primitives_by_mesh
                .get(&attachment.source_mesh_index())
                .into_iter()
                .flatten()
            {
                if unit_scale_join_budget_exceeded(&mut work) {
                    return CurrentUnitScaleMeshPlan::JoinOverflow;
                }
                rows.push(CurrentUnitScaleMeshRow {
                    source_scene_index,
                    source_root_ordinal,
                    root_node_index,
                    source_node_index: attachment.source_node_index(),
                    source_mesh_index: attachment.source_mesh_index(),
                    source_primitive_index,
                });
            }
        }
    }
    if rows.is_empty() {
        CurrentUnitScaleMeshPlan::CompleteEmpty
    } else {
        CurrentUnitScaleMeshPlan::Detailed(rows)
    }
}

fn current_unit_scale_selected_facet_count(
    reachability_plans: &UnitScaleSelectedReachabilityPlans,
) -> usize {
    reachability_plans.selected_facet_count()
}

fn expected_current_engine_unit_scale_facets(
    provenance: &PredictionProvenanceV4,
    measurements: &MeasurementContract,
    mesh_plan: &CurrentUnitScaleMeshPlan,
    candidate_capacity: usize,
    reachability_plans: &UnitScaleSelectedReachabilityPlans,
) -> Option<Vec<ExpectedUnitScaleFacet>> {
    let dependency_complete = matches!(
        provenance.dependency_closure().coverage(),
        DependencyClosureCoverageV1::Complete
    );
    let available = |scope, result| ExpectedUnitScaleFacet {
        scope,
        result: Some(result),
        reasons: vec![],
    };
    let unavailable = |scope, reasons| ExpectedUnitScaleFacet {
        scope,
        result: None,
        reasons,
    };
    let mut expected = Vec::with_capacity(candidate_capacity);
    if expected.len() < candidate_capacity {
        expected.push(if dependency_complete {
            available(
                unit_scale_scope(ENGINE_UNIT_SCALE_FILE_SCOPE, None),
                EngineMachineResultV1::UnitMapping(
                    UnitMappingResultV1::gltf_to_engine_world_length_unit(),
                ),
            )
        } else {
            unavailable(
                unit_scale_scope(ENGINE_UNIT_SCALE_FILE_SCOPE, None),
                vec![PredictionUnavailableReasonV2::DependencyClosureIncomplete],
            )
        });
    }

    let inventory = provenance.raw_scene_attachment().inventory();
    match inventory
        .filter(|inventory| inventory.scenes().coverage() == RawSceneAttachmentCoverageV1::Complete)
    {
        Some(inventory) => {
            for scene in inventory.scenes().rows() {
                if expected.len() >= candidate_capacity {
                    break;
                }
                let scope = unit_scale_scope(
                    ENGINE_UNIT_SCALE_SCENE_SCOPE,
                    Some(format!("source_scene:{}", scene.source_scene_index())),
                );
                expected.push(if dependency_complete {
                    available(
                        scope,
                        EngineMachineResultV1::TransformScale(TransformScaleResultV1 {
                            subject_kind: TransformScaleSubjectKindV1::LoaderSceneEntity,
                            creation: ImporterSubjectCreationV1::Created,
                            domain: TransformScaleDomainV1::Local,
                            classification: Some(LinearTransformClassification::UnitOrthonormal),
                        }),
                    )
                } else {
                    unavailable(
                        scope,
                        vec![PredictionUnavailableReasonV2::DependencyClosureIncomplete],
                    )
                });
            }
        }
        None if expected.len() < candidate_capacity => expected.push(unavailable(
            unit_scale_scope(ENGINE_UNIT_SCALE_SCENE_INVENTORY_SCOPE, None),
            unit_scale_unavailable_reasons(
                PredictionUnavailableReasonV2::RawSourceIncomplete,
                dependency_complete,
            ),
        )),
        None => {}
    }

    let assets = measurements.assets();
    let nodes = assets
        .skeleton_nodes
        .iter()
        .map(|node| (node.node_index, node))
        .collect::<BTreeMap<_, _>>();
    if expected.len() < candidate_capacity {
        match mesh_plan {
            CurrentUnitScaleMeshPlan::Incomplete => expected.push(unavailable(
                unit_scale_scope(ENGINE_UNIT_SCALE_MESH_INVENTORY_SCOPE, None),
                unit_scale_unavailable_reasons(
                    PredictionUnavailableReasonV2::RawSourceIncomplete,
                    dependency_complete,
                ),
            )),
            CurrentUnitScaleMeshPlan::JoinOverflow => expected.push(unavailable(
                unit_scale_scope(ENGINE_UNIT_SCALE_MESH_INVENTORY_SCOPE, None),
                vec![
                    PredictionUnavailableReasonV2::custom(
                        "animsmith:mesh_join_work_budget_exceeded",
                    )
                    .expect("static reason is valid"),
                ],
            )),
            CurrentUnitScaleMeshPlan::CompleteEmpty => expected.push(if dependency_complete {
                available(
                    unit_scale_scope(ENGINE_UNIT_SCALE_MESH_INVENTORY_SCOPE, None),
                    EngineMachineResultV1::InventoryCoverage(InventoryCoverageResultV1 {
                        domain: PredictionInventoryDomainV1::LoaderMeshPrimitiveSubjects,
                        coverage: PredictionInventoryCoverageStateV1::Complete,
                        retained_rows: 0,
                    }),
                )
            } else {
                unavailable(
                    unit_scale_scope(ENGINE_UNIT_SCALE_MESH_INVENTORY_SCOPE, None),
                    vec![PredictionUnavailableReasonV2::DependencyClosureIncomplete],
                )
            }),
            CurrentUnitScaleMeshPlan::Detailed(rows) => {
                let load_meshes = matches!(
                    provenance
                        .settings()
                        .document_setting(EngineSettingIdV2::LoadMeshes)
                        .map(|setting| setting.value()),
                    Some(EngineSettingValueV2::Token(value)) if value == "nonempty"
                );
                for row in rows {
                    if expected.len() >= candidate_capacity {
                        break;
                    }
                    let scope = unit_scale_scope(
                        ENGINE_UNIT_SCALE_MESH_SCOPE,
                        Some(format!(
                            "source_scene:{}:source_node:{}:source_mesh:{}:source_primitive:{}",
                            row.source_scene_index,
                            row.source_node_index,
                            row.source_mesh_index,
                            row.source_primitive_index,
                        )),
                    );
                    expected.push(if dependency_complete {
                        available(
                            scope,
                            EngineMachineResultV1::TransformScale(TransformScaleResultV1 {
                                subject_kind:
                                    TransformScaleSubjectKindV1::LoaderMeshPrimitiveEntity,
                                creation: if load_meshes {
                                    ImporterSubjectCreationV1::Created
                                } else {
                                    ImporterSubjectCreationV1::SuppressedBySetting
                                },
                                domain: TransformScaleDomainV1::Local,
                                classification: load_meshes
                                    .then_some(LinearTransformClassification::UnitOrthonormal),
                            }),
                        )
                    } else {
                        unavailable(
                            scope,
                            vec![PredictionUnavailableReasonV2::DependencyClosureIncomplete],
                        )
                    });
                }
            }
        }
    }

    for selector in provenance.rule_inputs().runtime_node_selectors() {
        if expected.len() >= candidate_capacity {
            break;
        }
        if assets.skeleton_source_coverage != SourceSkeletonCoverage::Complete
            || !inventory.is_some_and(|inventory| {
                inventory.source_skeleton().coverage() == RawSceneAttachmentCoverageV1::Complete
                    && inventory.scenes().coverage() == RawSceneAttachmentCoverageV1::Complete
            })
        {
            expected.push(unavailable(
                unit_scale_scope(
                    ENGINE_UNIT_SCALE_SELECTED_SCOPE,
                    Some(format!("selector:{selector}")),
                ),
                unit_scale_unavailable_reasons(
                    PredictionUnavailableReasonV2::RawSourceIncomplete,
                    dependency_complete,
                ),
            ));
            continue;
        }
        let mut matches = assets.skeleton_nodes.iter().filter(|node| {
            node.name
                .as_deref()
                .is_some_and(|name| crate::config::glob_match(selector, name))
        });
        let first = matches.next();
        let second = matches.next();
        if first.is_none() || second.is_some() {
            expected.push(unavailable(
                unit_scale_scope(
                    ENGINE_UNIT_SCALE_SELECTED_SCOPE,
                    Some(format!("selector:{selector}")),
                ),
                unit_scale_unavailable_reasons(
                    if first.is_none() {
                        PredictionUnavailableReasonV2::SourceSelectorNoMatch
                    } else {
                        PredictionUnavailableReasonV2::SourceSelectorAmbiguous
                    },
                    dependency_complete,
                ),
            ));
            continue;
        }
        let node = first.expect("one selected source node was established");
        let inventory = inventory.expect("selected inventory was proven complete");
        let mut reachable_scenes = Vec::new();
        let plan = reachability_plans.get(selector)?;
        let reachability_unavailable =
            match plan {
                UnitScaleSelectedReachabilityPlan::Refused {
                    node_index: cached_node_index,
                } if *cached_node_index == node.node_index as u64 => true,
                UnitScaleSelectedReachabilityPlan::Complete {
                    node_index: cached_node_index,
                    scene_witnesses,
                } if *cached_node_index == node.node_index as u64 => {
                    reachable_scenes.extend(
                        inventory.scenes().rows().iter().filter(|scene| {
                            scene_witnesses.contains_key(&scene.source_scene_index())
                        }),
                    );
                    false
                }
                UnitScaleSelectedReachabilityPlan::Refused { .. }
                | UnitScaleSelectedReachabilityPlan::Complete { .. } => return None,
            };
        if reachability_unavailable {
            expected.push(unavailable(
                unit_scale_scope(
                    ENGINE_UNIT_SCALE_SELECTED_SCOPE,
                    Some(format!("selector:{selector}")),
                ),
                unit_scale_unavailable_reasons(
                    PredictionUnavailableReasonV2::custom(
                        ENGINE_UNIT_SCALE_SELECTED_REACHABILITY_UNAVAILABLE_REASON,
                    )
                    .expect("static reason is valid"),
                    dependency_complete,
                ),
            ));
            continue;
        }
        let reason = unit_scale_selected_ancestry_reason(&nodes, node.node_index).or_else(|| {
            (node.rest_world_linear.classification == LinearTransformClassification::NonFinite
                || node.rest_world_matrix.is_none())
            .then_some(PredictionUnavailableReasonV2::MeasurementUnavailable)
        });
        let mut reachable_found = false;
        for scene in reachable_scenes {
            reachable_found = true;
            if expected.len() >= candidate_capacity {
                break;
            }
            let scope = unit_scale_scope(
                ENGINE_UNIT_SCALE_SELECTED_SCOPE,
                Some(format!(
                    "selector:{selector}:source_scene:{}:source_node:{}",
                    scene.source_scene_index(),
                    node.node_index,
                )),
            );
            expected.push(if let Some(reason) = reason.clone() {
                unavailable(
                    scope,
                    unit_scale_unavailable_reasons(reason, dependency_complete),
                )
            } else if dependency_complete {
                available(
                    scope,
                    EngineMachineResultV1::TransformScale(TransformScaleResultV1 {
                        subject_kind: TransformScaleSubjectKindV1::SelectedSourceNode,
                        creation: ImporterSubjectCreationV1::Created,
                        domain: TransformScaleDomainV1::LoaderRootToSubject,
                        classification: Some(node.rest_world_linear.classification),
                    }),
                )
            } else {
                unavailable(
                    scope,
                    vec![PredictionUnavailableReasonV2::DependencyClosureIncomplete],
                )
            });
        }
        if !reachable_found && expected.len() < candidate_capacity {
            expected.push(unavailable(
                unit_scale_scope(
                    ENGINE_UNIT_SCALE_SELECTED_SCOPE,
                    Some(format!("selector:{selector}")),
                ),
                unit_scale_unavailable_reasons(
                    PredictionUnavailableReasonV2::custom("animsmith:selected_node_unreachable")
                        .expect("static reason is valid"),
                    dependency_complete,
                ),
            ));
        }
    }
    Some(expected)
}

fn unit_scale_exact_raw_row_references(
    basis: &EnginePredictionBasisV4,
    expected: &[RawSceneAttachmentBasisReferenceV1],
) -> bool {
    let actual = basis
        .references()
        .iter()
        .filter_map(|reference| match reference {
            PredictionBasisReferenceV4::RawSceneAttachment(reference)
                if !matches!(
                    reference,
                    RawSceneAttachmentBasisReferenceV1::Coverage { .. }
                ) =>
            {
                Some(reference)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    actual.len() == expected.len() && actual.iter().all(|reference| expected.contains(reference))
}

fn unit_scale_mesh_scope_keys(subject: &str) -> Option<(u64, u64, u64, u64)> {
    let values = subject
        .strip_prefix("source_scene:")?
        .split(':')
        .collect::<Vec<_>>();
    if values.len() != 7
        || values[1] != "source_node"
        || values[3] != "source_mesh"
        || values[5] != "source_primitive"
    {
        return None;
    }
    Some((
        values[0].parse().ok()?,
        values[2].parse().ok()?,
        values[4].parse().ok()?,
        values[6].parse().ok()?,
    ))
}

fn unit_scale_expected_mesh_raw_rows(
    subject: &str,
    mesh_plan: &CurrentUnitScaleMeshPlan,
) -> Option<Vec<RawSceneAttachmentBasisReferenceV1>> {
    let (source_scene_index, source_node_index, source_mesh_index, source_primitive_index) =
        unit_scale_mesh_scope_keys(subject)?;
    let CurrentUnitScaleMeshPlan::Detailed(rows) = mesh_plan else {
        return None;
    };
    let row = rows.iter().find(|row| {
        row.source_scene_index == source_scene_index
            && row.source_node_index == source_node_index
            && row.source_mesh_index == source_mesh_index
            && row.source_primitive_index == source_primitive_index
    })?;
    Some(vec![
        RawSceneAttachmentBasisReferenceV1::SceneRow { source_scene_index },
        RawSceneAttachmentBasisReferenceV1::SceneRoot {
            source_scene_index,
            source_root_ordinal: row.source_root_ordinal,
            source_node_index: row.root_node_index,
        },
        RawSceneAttachmentBasisReferenceV1::NodeMeshAttachmentRow {
            source_node_index,
            source_mesh_index,
        },
        RawSceneAttachmentBasisReferenceV1::MeshPrimitiveRow {
            source_mesh_index,
            source_primitive_index,
        },
    ])
}

fn unit_scale_selected_scope_keys<'a>(
    subject: &str,
    selectors: &'a [String],
) -> Option<(&'a str, Option<(u64, u64)>)> {
    for selector in selectors {
        let prefix = format!("selector:{selector}");
        if subject == prefix {
            return Some((selector, None));
        }
        let Some(values) = subject
            .strip_prefix(&prefix)
            .and_then(|suffix| suffix.strip_prefix(":source_scene:"))
        else {
            continue;
        };
        let values = values.split(':').collect::<Vec<_>>();
        if values.len() == 3 && values[1] == "source_node" {
            return Some((
                selector,
                Some((values[0].parse().ok()?, values[2].parse().ok()?)),
            ));
        }
    }
    None
}

fn unit_scale_classification_name(value: LinearTransformClassification) -> &'static str {
    match value {
        LinearTransformClassification::UnitOrthonormal => "unit_orthonormal",
        LinearTransformClassification::UniformScaled => "uniform_scaled",
        LinearTransformClassification::NonUniform => "non_uniform",
        LinearTransformClassification::Sheared => "sheared",
        LinearTransformClassification::Reflected => "reflected",
        LinearTransformClassification::Singular => "singular",
        LinearTransformClassification::NonFinite => "non_finite",
    }
}

fn unit_scale_raw_source_node_reference(
    source_index: u64,
    field: &str,
    value: PredictionScalarV1,
) -> Option<RawSourceBasisReferenceV1> {
    RawSourceBasisReferenceV1::from_wire(
        RawSourceDomainV1::SourceNode,
        RawSourceKeyV1::SourceSkeleton {
            row_kind: SourceSkeletonRowKindV1::SourceNode,
            source_index,
        },
        RawSourceFieldIdV1::new(field).ok()?,
        value,
    )
    .ok()
}

fn unit_scale_exact_raw_source_references(
    basis: &EnginePredictionBasisV4,
    expected: &[RawSourceBasisReferenceV1],
) -> bool {
    let actual = basis
        .references()
        .iter()
        .filter_map(|reference| match reference {
            PredictionBasisReferenceV4::V2(PredictionBasisReferenceV2::V1(
                PredictionBasisReferenceV1::RawSource { reference },
            )) => Some(reference),
            _ => None,
        })
        .collect::<Vec<_>>();
    actual.len() == expected.len() && actual.iter().all(|reference| expected.contains(reference))
}

fn unit_scale_selected_authored_kind(
    basis: &EnginePredictionBasisV4,
    source_index: u64,
) -> Option<&str> {
    let mut values = basis.references().iter().filter_map(|reference| {
        let PredictionBasisReferenceV4::V2(PredictionBasisReferenceV2::V1(
            PredictionBasisReferenceV1::RawSource { reference },
        )) = reference
        else {
            return None;
        };
        if reference.domain() != RawSourceDomainV1::SourceNode
            || reference.key()
                != &(RawSourceKeyV1::SourceSkeleton {
                    row_kind: SourceSkeletonRowKindV1::SourceNode,
                    source_index,
                })
            || reference.field().as_str() != "local_rest.kind"
        {
            return None;
        }
        match reference.value() {
            PredictionScalarV1::Token { value } if matches!(value.as_str(), "trs" | "matrix") => {
                Some(value.as_str())
            }
            _ => None,
        }
    });
    let value = values.next()?;
    values.next().is_none().then_some(value)
}

fn unit_scale_selected_expected_reasons(
    primary: Option<PredictionUnavailableReasonV2>,
    dependency_complete: bool,
) -> Vec<PredictionUnavailableReasonV2> {
    match primary {
        Some(reason) => unit_scale_unavailable_reasons(reason, dependency_complete),
        None if dependency_complete => Vec::new(),
        None => vec![PredictionUnavailableReasonV2::DependencyClosureIncomplete],
    }
}

struct CurrentUnitScaleSelectedEvidence {
    raw_source_references: Vec<RawSourceBasisReferenceV1>,
    reasons: Vec<PredictionUnavailableReasonV2>,
}

fn unit_scale_expected_selected_raw_source_references(
    selector: &str,
    keys: Option<(u64, u64)>,
    basis: &EnginePredictionBasisV4,
    provenance: &PredictionProvenanceV4,
    measurements: &MeasurementContract,
    reachability_plans: &UnitScaleSelectedReachabilityPlans,
) -> Option<CurrentUnitScaleSelectedEvidence> {
    let assets = measurements.assets();
    let dependency_complete = matches!(
        provenance.dependency_closure().coverage(),
        DependencyClosureCoverageV1::Complete
    );
    let inventory = provenance.raw_scene_attachment().inventory();
    let inventory_complete = inventory.is_some_and(|inventory| {
        inventory.scenes().coverage() == RawSceneAttachmentCoverageV1::Complete
            && inventory.source_skeleton().coverage() == RawSceneAttachmentCoverageV1::Complete
    });
    if assets.skeleton_source_coverage != SourceSkeletonCoverage::Complete || !inventory_complete {
        return Some(CurrentUnitScaleSelectedEvidence {
            raw_source_references: Vec::new(),
            reasons: unit_scale_selected_expected_reasons(
                Some(PredictionUnavailableReasonV2::RawSourceIncomplete),
                dependency_complete,
            ),
        });
    }
    let mut matches = assets.skeleton_nodes.iter().filter(|node| {
        node.name
            .as_deref()
            .is_some_and(|name| crate::config::glob_match(selector, name))
    });
    let first = matches.next();
    let second = matches.next();
    let name_reference = |node: &crate::measure::SkeletonNodeMeasurements| {
        unit_scale_raw_source_node_reference(
            node.node_index as u64,
            "name",
            node.name.as_ref().map_or(PredictionScalarV1::Null, |name| {
                PredictionScalarV1::text(name).expect("retained measurement text is bounded")
            }),
        )
    };
    if first.is_none() {
        return keys.is_none().then(|| CurrentUnitScaleSelectedEvidence {
            raw_source_references: Vec::new(),
            reasons: unit_scale_selected_expected_reasons(
                Some(PredictionUnavailableReasonV2::SourceSelectorNoMatch),
                dependency_complete,
            ),
        });
    }
    if let Some(second) = second {
        if keys.is_some() {
            return None;
        }
        return Some(CurrentUnitScaleSelectedEvidence {
            raw_source_references: vec![name_reference(first?)?, name_reference(second)?],
            reasons: unit_scale_selected_expected_reasons(
                Some(PredictionUnavailableReasonV2::SourceSelectorAmbiguous),
                dependency_complete,
            ),
        });
    }
    let node = first?;
    if keys.is_some_and(|(_, source_node_index)| source_node_index != node.node_index as u64) {
        return None;
    }
    let mut expected = vec![name_reference(node)?];
    let nodes = assets
        .skeleton_nodes
        .iter()
        .map(|node| (node.node_index, node))
        .collect::<BTreeMap<_, _>>();
    let plan = reachability_plans.get(selector)?;
    let reachability_unavailable = match plan {
        UnitScaleSelectedReachabilityPlan::Refused {
            node_index: cached_node_index,
        } if *cached_node_index == node.node_index as u64 => true,
        UnitScaleSelectedReachabilityPlan::Complete {
            node_index: cached_node_index,
            ..
        } if *cached_node_index == node.node_index as u64 => false,
        UnitScaleSelectedReachabilityPlan::Refused { .. }
        | UnitScaleSelectedReachabilityPlan::Complete { .. } => return None,
    };
    if reachability_unavailable {
        if keys.is_some() {
            return None;
        }
        return Some(CurrentUnitScaleSelectedEvidence {
            raw_source_references: expected,
            reasons: unit_scale_selected_expected_reasons(
                Some(
                    PredictionUnavailableReasonV2::custom(
                        ENGINE_UNIT_SCALE_SELECTED_REACHABILITY_UNAVAILABLE_REASON,
                    )
                    .expect("static reason is valid"),
                ),
                dependency_complete,
            ),
        });
    }
    let mut current = node.node_index;
    let mut seen = BTreeSet::new();
    let mut ancestry_reason = None;
    let mut ancestry_complete = false;
    for _ in 0..128 {
        if !seen.insert(current) {
            ancestry_reason = Some(
                PredictionUnavailableReasonV2::custom(
                    "animsmith:selected_node_ancestry_unavailable",
                )
                .expect("static reason is valid"),
            );
            break;
        }
        let Some(ancestry_node) = nodes.get(&current).copied() else {
            ancestry_reason = Some(
                PredictionUnavailableReasonV2::custom(
                    "animsmith:selected_node_ancestry_unavailable",
                )
                .expect("static reason is valid"),
            );
            break;
        };
        // The normalized measurement deliberately erases whether a non-finite
        // local rest was authored as TRS or as a matrix. The exact same-load
        // raw-source scalar in the basis retains that distinction per row.
        let retained_kind = unit_scale_selected_authored_kind(basis, current as u64)?;
        let local_kind = match ancestry_node.local_rest {
            SkeletonNodeLocalRestMeasurements::Trs { .. } if retained_kind == "trs" => "trs",
            SkeletonNodeLocalRestMeasurements::Matrix { .. } if retained_kind == "matrix" => {
                "matrix"
            }
            SkeletonNodeLocalRestMeasurements::Unavailable { .. } => retained_kind,
            _ => return None,
        };
        expected.push(unit_scale_raw_source_node_reference(
            current as u64,
            "local_rest.kind",
            PredictionScalarV1::token(local_kind).ok()?,
        )?);
        expected.push(unit_scale_raw_source_node_reference(
            current as u64,
            "parent_source_node_index",
            ancestry_node
                .parent_node_index
                .map_or(PredictionScalarV1::Null, |parent| {
                    PredictionScalarV1::UnsignedInteger {
                        value: parent as u64,
                    }
                }),
        )?);
        if local_kind == "matrix" {
            ancestry_reason = Some(
                PredictionUnavailableReasonV2::custom(
                    "animsmith:matrix_authored_selected_node_or_ancestry",
                )
                .expect("static reason is valid"),
            );
            break;
        }
        let Some(parent) = ancestry_node.parent_node_index else {
            ancestry_complete = true;
            break;
        };
        current = parent;
    }
    if ancestry_reason.is_none() && !ancestry_complete {
        ancestry_reason = Some(
            PredictionUnavailableReasonV2::custom("animsmith:selected_node_ancestry_unavailable")
                .expect("static reason is valid"),
        );
    }
    let primary_reason = if keys.is_none() {
        Some(
            PredictionUnavailableReasonV2::custom("animsmith:selected_node_unreachable")
                .expect("static reason is valid"),
        )
    } else {
        ancestry_reason.or_else(|| {
            (node.rest_world_linear.classification == LinearTransformClassification::NonFinite
                || node.rest_world_matrix.is_none())
            .then_some(PredictionUnavailableReasonV2::MeasurementUnavailable)
        })
    };
    Some(CurrentUnitScaleSelectedEvidence {
        raw_source_references: expected,
        reasons: unit_scale_selected_expected_reasons(primary_reason, dependency_complete),
    })
}

fn unit_scale_exact_selected_evidence(
    selector: &str,
    keys: Option<(u64, u64)>,
    reasons: &[PredictionUnavailableReasonV2],
    basis: &EnginePredictionBasisV4,
    provenance: &PredictionProvenanceV4,
    measurements: &MeasurementContract,
    reachability_plans: &UnitScaleSelectedReachabilityPlans,
) -> bool {
    let Some(expected) = unit_scale_expected_selected_raw_source_references(
        selector,
        keys,
        basis,
        provenance,
        measurements,
        reachability_plans,
    ) else {
        return false;
    };
    if reasons != expected.reasons
        || !unit_scale_exact_raw_source_references(basis, &expected.raw_source_references)
    {
        return false;
    }
    let measurement_references = basis
        .references()
        .iter()
        .filter_map(|reference| match reference {
            PredictionBasisReferenceV4::V2(PredictionBasisReferenceV2::V1(
                PredictionBasisReferenceV1::Measurement {
                    schema,
                    pointer,
                    value,
                },
            )) => Some((*schema, pointer.as_str(), value)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let Some((source_scene_index, source_node_index)) = keys else {
        return unit_scale_exact_raw_row_references(basis, &[])
            && measurement_references.is_empty();
    };
    let Some(inventory) = provenance.raw_scene_attachment().inventory() else {
        return false;
    };
    let Some(_scene) = inventory
        .scenes()
        .rows()
        .iter()
        .find(|scene| scene.source_scene_index() == source_scene_index)
    else {
        return false;
    };
    let witness = reachability_plans
        .get(selector)
        .filter(|plan| plan.node_index() == source_node_index)
        .and_then(|plan| match plan {
            UnitScaleSelectedReachabilityPlan::Complete {
                scene_witnesses, ..
            } => scene_witnesses.get(&source_scene_index).copied(),
            UnitScaleSelectedReachabilityPlan::Refused { .. } => None,
        });
    let Some((source_root_ordinal, source_node_index_at_root)) = witness else {
        return false;
    };
    if !unit_scale_exact_raw_row_references(
        basis,
        &[
            RawSceneAttachmentBasisReferenceV1::SceneRow { source_scene_index },
            RawSceneAttachmentBasisReferenceV1::SceneRoot {
                source_scene_index,
                source_root_ordinal,
                source_node_index: source_node_index_at_root,
            },
        ],
    ) {
        return false;
    }
    let Some((ordinal, node)) = measurements
        .assets()
        .skeleton_nodes
        .iter()
        .enumerate()
        .find(|(_, node)| node.node_index as u64 == source_node_index)
    else {
        return measurement_references.is_empty();
    };
    let pointer =
        format!("/measurements/skeleton_nodes/{ordinal}/rest_world_linear/classification");
    matches!(
        measurement_references.as_slice(),
        [(
            MEASUREMENTS_V16_SCHEMA_ID,
            actual_pointer,
            PredictionScalarV1::Token { value },
        )] if *actual_pointer == pointer
            && value == unit_scale_classification_name(node.rest_world_linear.classification)
    )
}

fn validate_current_engine_unit_scale_basis(
    scope: &EvaluationScope,
    basis: &EnginePredictionBasisV4,
    reasons: &[PredictionUnavailableReasonV2],
    provenance: &PredictionProvenanceV4,
    measurements: &MeasurementContract,
    mesh_plan: &CurrentUnitScaleMeshPlan,
    reachability_plans: &UnitScaleSelectedReachabilityPlans,
) -> bool {
    let v1 = |predicate: &dyn Fn(&PredictionBasisReferenceV1) -> bool| {
        basis.references().iter().any(|reference| {
            matches!(
                reference,
                PredictionBasisReferenceV4::V2(PredictionBasisReferenceV2::V1(reference))
                    if predicate(reference)
            )
        })
    };
    let fact = |expected: &str| {
        v1(
            &|reference| matches!(reference, PredictionBasisReferenceV1::ProfileFact { fact_id } if fact_id == expected),
        )
    };
    let setting = |expected: &str| {
        v1(
            &|reference| matches!(reference, PredictionBasisReferenceV1::ResolvedSetting { setting_id, .. } if setting_id == expected),
        )
    };
    let source = |expected: &str| {
        v1(
            &|reference| matches!(reference, PredictionBasisReferenceV1::PrimarySource { source_id } if source_id == expected),
        )
    };
    let raw_coverage = |expected: RawSceneAttachmentBasisDomainV1| {
        basis.references().iter().any(|reference| {
            matches!(
                reference,
                PredictionBasisReferenceV4::RawSceneAttachment(
                    RawSceneAttachmentBasisReferenceV1::Coverage { domain }
                ) if *domain == expected
            )
        })
    };
    let common_transform = source("bevy-gltf-loader-0.19.0-c6f634ca")
        && source("bevy-gltf-coordinate-conversion-0.19.0-c6f634ca")
        && fact("resulting_transform_scale")
        && setting("extension_handler_environment");
    match scope.code.as_str() {
        ENGINE_UNIT_SCALE_FILE_SCOPE => {
            unit_scale_exact_raw_row_references(basis, &[])
                && [
                    "application_world_unit_policy",
                    "importer_scale_conversion",
                    "physical_dimensions_preserved",
                    "source_to_target_unit_mapping",
                    "target_linear_unit",
                ]
                .into_iter()
                .all(fact)
                && [
                    "bevy-gltf-loader-0.19.0-c6f634ca",
                    "bevy-gltf-coordinate-conversion-0.19.0-c6f634ca",
                    "khronos-gltf-2.0-coordinate-units",
                ]
                .into_iter()
                .all(source)
        }
        ENGINE_UNIT_SCALE_SCENE_SCOPE | ENGINE_UNIT_SCALE_SCENE_INVENTORY_SCOPE => {
            let expected_rows = if scope.code.as_str() == ENGINE_UNIT_SCALE_SCENE_INVENTORY_SCOPE {
                Some(Vec::new())
            } else {
                scope
                    .subject
                    .as_deref()
                    .and_then(|subject| subject.strip_prefix("source_scene:"))
                    .and_then(|index| index.parse::<u64>().ok())
                    .filter(|index| {
                        scope.subject.as_deref() == Some(&format!("source_scene:{index}"))
                    })
                    .map(|source_scene_index| {
                        vec![RawSceneAttachmentBasisReferenceV1::SceneRow { source_scene_index }]
                    })
            };
            expected_rows.as_deref().is_some_and(|expected_rows| {
                unit_scale_exact_raw_row_references(basis, expected_rows)
            }) && common_transform
                && setting("rotate_scene_entity")
                && (provenance.raw_scene_attachment().inventory().is_none()
                    || raw_coverage(RawSceneAttachmentBasisDomainV1::Scenes))
        }
        ENGINE_UNIT_SCALE_MESH_SCOPE | ENGINE_UNIT_SCALE_MESH_INVENTORY_SCOPE => {
            let expected_rows = if scope.code.as_str() == ENGINE_UNIT_SCALE_MESH_INVENTORY_SCOPE {
                Some(Vec::new())
            } else {
                scope
                    .subject
                    .as_deref()
                    .and_then(|subject| unit_scale_expected_mesh_raw_rows(subject, mesh_plan))
            };
            expected_rows.as_deref().is_some_and(|expected_rows| {
                unit_scale_exact_raw_row_references(basis, expected_rows)
            }) && common_transform
                && source("bevy-render-asset-usages-0.19.0-c6f634ca")
                && setting("load_meshes")
                && setting("rotate_meshes")
                && (provenance.raw_scene_attachment().inventory().is_none()
                    || [
                        RawSceneAttachmentBasisDomainV1::SourceSkeleton,
                        RawSceneAttachmentBasisDomainV1::Scenes,
                        RawSceneAttachmentBasisDomainV1::NodeMeshAttachments,
                        RawSceneAttachmentBasisDomainV1::MeshPrimitives,
                    ]
                    .into_iter()
                    .all(raw_coverage))
        }
        ENGINE_UNIT_SCALE_SELECTED_SCOPE => {
            let selected = scope.subject.as_deref().and_then(|subject| {
                unit_scale_selected_scope_keys(
                    subject,
                    provenance.rule_inputs().runtime_node_selectors(),
                )
            });
            let selector = selected.map(|(selector, _)| selector);
            common_transform
                && setting("rotate_scene_entity")
                && selected.is_some_and(|(selector, keys)| {
                    unit_scale_exact_selected_evidence(
                        selector,
                        keys,
                        reasons,
                        basis,
                        provenance,
                        measurements,
                        reachability_plans,
                    )
                })
                && selector.is_some_and(|selector| {
                    v1(&|reference| {
                        matches!(
                            reference,
                            PredictionBasisReferenceV1::ProjectField {
                                field_id,
                                value: PredictionScalarV1::Text { value }
                            } if field_id == "runtime_nodes.selector" && value == selector
                        )
                    })
                })
                && (provenance.raw_scene_attachment().inventory().is_none()
                    || (raw_coverage(RawSceneAttachmentBasisDomainV1::SourceSkeleton)
                        && raw_coverage(RawSceneAttachmentBasisDomainV1::Scenes)))
        }
        ENGINE_UNIT_SCALE_BUDGET_SCOPE => {
            unit_scale_exact_raw_row_references(basis, &[])
                && source("bevy-gltf-loader-0.19.0-c6f634ca")
        }
        _ => false,
    }
}

fn validate_current_engine_unit_scale_prediction_v4(
    check_id: &str,
    selection: SelectionState,
    configuration: ConfigurationState,
    applicability: Applicability,
    prediction: Option<&EnginePredictionV4>,
    provenance: Option<&PredictionProvenanceV4>,
    measurements: &MeasurementContract,
) -> Result<(), PredictionContractError> {
    if check_id != ENGINE_UNIT_SCALE_CHECK_ID {
        return Ok(());
    }
    let exact_profile = provenance.is_some_and(|provenance| {
        let selection = provenance.profile().selection();
        selection.family() == "bevy"
            && selection.profile_revision() == 2
            && selection.engine_version() == "0.19.0"
            && selection.importer() == "gltf-asset-loader"
            && provenance.profile().fact_bundle_urn() == "urn:animsmith:engine-profile:bevy:2"
            && provenance.profile().facts_identity().sha256()
                == "bcd663e891b25029ecdf17e942f6fa93f71a8d0598fc8cb02bc7634749c34597"
            && provenance.profile().facts_identity().bytes() == 4_783
            && matches!(
                provenance.source_format(),
                SourceFormatV1::GltfJson | SourceFormatV1::Glb
            )
    });
    validate_current_engine_unit_scale_prediction_common(
        check_id,
        selection,
        configuration,
        applicability,
        prediction,
        provenance,
        measurements,
        exact_profile,
    )
}

fn validate_current_engine_unit_scale_prediction_v5(
    check_id: &str,
    selection: SelectionState,
    configuration: ConfigurationState,
    applicability: Applicability,
    prediction: Option<&EnginePredictionV5>,
    provenance: Option<&PredictionProvenanceV5>,
    measurements: &MeasurementContract,
) -> Result<(), PredictionContractError> {
    let base = provenance.map(PredictionProvenanceV5::base);
    let exact_profile = base.is_some_and(|provenance| {
        let selection = provenance.profile().selection();
        let exact_identity = match selection.profile_revision() {
            2 => {
                provenance.profile().fact_bundle_urn() == "urn:animsmith:engine-profile:bevy:2"
                    && provenance.profile().facts_identity().sha256()
                        == "bcd663e891b25029ecdf17e942f6fa93f71a8d0598fc8cb02bc7634749c34597"
                    && provenance.profile().facts_identity().bytes() == 4_783
            }
            3 => {
                provenance.profile().fact_bundle_urn() == "urn:animsmith:engine-profile:bevy:3"
                    && provenance.profile().facts_identity().sha256()
                        == "d532b00621bf06a2db2dedf896c19aae2c07b3b1873a1b05beade2252d7a89c5"
                    && provenance.profile().facts_identity().bytes() == 4_849
            }
            _ => false,
        };
        selection.family() == "bevy"
            && selection.engine_version() == "0.19.0"
            && selection.importer() == "gltf-asset-loader"
            && exact_identity
            && matches!(
                provenance.source_format(),
                SourceFormatV1::GltfJson | SourceFormatV1::Glb
            )
    });
    validate_current_engine_unit_scale_prediction_common(
        check_id,
        selection,
        configuration,
        applicability,
        prediction.map(EnginePredictionV5::base_prediction),
        base,
        measurements,
        exact_profile,
    )
}

#[allow(clippy::too_many_arguments)]
fn validate_current_engine_unit_scale_prediction_common(
    check_id: &str,
    selection: SelectionState,
    configuration: ConfigurationState,
    applicability: Applicability,
    prediction: Option<&EnginePredictionV4>,
    provenance: Option<&PredictionProvenanceV4>,
    measurements: &MeasurementContract,
    exact_profile: bool,
) -> Result<(), PredictionContractError> {
    if check_id != ENGINE_UNIT_SCALE_CHECK_ID {
        return Ok(());
    }
    if applicability
        != if exact_profile {
            Applicability::Applicable
        } else {
            Applicability::NotApplicable
        }
    {
        return Err(PredictionContractError::EngineUnitScaleFacetMismatch);
    }
    if !exact_profile {
        return if prediction.is_none() {
            Ok(())
        } else {
            Err(PredictionContractError::EngineUnitScaleFacetMismatch)
        };
    }
    if selection != SelectionState::Selected || configuration != ConfigurationState::Enabled {
        return if prediction.is_none() {
            Ok(())
        } else {
            Err(PredictionContractError::EngineUnitScaleFacetMismatch)
        };
    }
    let provenance = provenance.ok_or(PredictionContractError::EngineUnitScaleFacetMismatch)?;
    let prediction = prediction.ok_or(PredictionContractError::EngineUnitScaleFacetMismatch)?;
    let mesh_plan = current_unit_scale_mesh_plan(provenance, measurements);
    let reachability_plans = unit_scale_selected_reachability_plans(provenance, measurements);
    if prediction.facets().iter().any(|facet| {
        !validate_current_engine_unit_scale_basis(
            facet.scope(),
            facet.basis(),
            facet.reasons(),
            provenance,
            measurements,
            &mesh_plan,
            &reachability_plans,
        )
    }) {
        return Err(PredictionContractError::EngineUnitScaleFacetMismatch);
    }
    let has_summary = prediction.facets().iter().any(|facet| {
        facet.scope().code.as_str() == ENGINE_UNIT_SCALE_BUDGET_SCOPE
            && facet.reasons() == [PredictionUnavailableReasonV2::FacetBudgetExceeded]
    });
    let retained = prediction
        .facets()
        .iter()
        .filter(|facet| facet.scope().code.as_str() != ENGINE_UNIT_SCALE_BUDGET_SCOPE)
        .collect::<Vec<_>>();
    let scene_facets = provenance
        .raw_scene_attachment()
        .inventory()
        .filter(|inventory| inventory.scenes().coverage() == RawSceneAttachmentCoverageV1::Complete)
        .map_or(1, |inventory| inventory.scenes().rows().len());
    let mesh_facets = match &mesh_plan {
        CurrentUnitScaleMeshPlan::Detailed(rows) => rows.len(),
        CurrentUnitScaleMeshPlan::CompleteEmpty
        | CurrentUnitScaleMeshPlan::Incomplete
        | CurrentUnitScaleMeshPlan::JoinOverflow => 1,
    };
    let selected_facets = current_unit_scale_selected_facet_count(&reachability_plans);
    let expected_count = 1usize
        .checked_add(scene_facets)
        .and_then(|count| count.checked_add(mesh_facets))
        .and_then(|count| count.checked_add(selected_facets))
        .ok_or(PredictionContractError::EngineUnitScaleFacetMismatch)?;
    if (!has_summary && expected_count > PREDICTION_V2_MAX_CANDIDATE_FACETS_PER_RULE)
        || (has_summary && retained.len() >= expected_count)
    {
        return Err(PredictionContractError::EngineUnitScaleFacetMismatch);
    }
    let candidate_capacity = if has_summary {
        retained.len()
    } else {
        expected_count
    };
    let mut expected_retained = expected_current_engine_unit_scale_facets(
        provenance,
        measurements,
        &mesh_plan,
        candidate_capacity,
        &reachability_plans,
    )
    .ok_or(PredictionContractError::EngineUnitScaleFacetMismatch)?;
    if expected_retained.len() != candidate_capacity {
        return Err(PredictionContractError::EngineUnitScaleFacetMismatch);
    }
    expected_retained.sort_by(|left, right| {
        left.scope
            .code
            .as_str()
            .cmp(right.scope.code.as_str())
            .then_with(|| left.scope.subject.cmp(&right.scope.subject))
    });
    if retained.len() != expected_retained.len()
        || retained
            .iter()
            .zip(&expected_retained)
            .any(|(actual, expected)| {
                actual.scope() != &expected.scope
                    || actual.result() != expected.result.as_ref()
                    || (actual.scope().code.as_str() != ENGINE_UNIT_SCALE_SELECTED_SCOPE
                        && actual.reasons() != expected.reasons)
            })
    {
        return Err(PredictionContractError::EngineUnitScaleFacetMismatch);
    }
    Ok(())
}

const ENGINE_CLIP_BOUNDARY_CHECK_ID: &str = "engine-clip-boundary";
const ENGINE_CLIP_BOUNDARY_SOURCE_ID: &str = "unreal-animation-sequences-5.8";
const ENGINE_CLIP_BOUNDARY_PROFILE_FAMILY: &str = "unreal";
const ENGINE_CLIP_BOUNDARY_PROFILE_REVISION: u32 = 1;
const ENGINE_CLIP_BOUNDARY_ENGINE_VERSION: &str = "5.8";
const ENGINE_CLIP_BOUNDARY_IMPORTER: &str = "fbx-importer";
const ENGINE_CLIP_BOUNDARY_PROFILE_FACTS_SHA256: &str =
    "e44ca461aee46312b8265446f08338b988b96abeab0f8f502f560da5f1cdf759";
const ENGINE_CLIP_BOUNDARY_PROFILE_FACTS_BYTES: u64 = 2_169;

fn current_engine_clip_boundary_profile_matches_v3(provenance: &PredictionProvenanceV3) -> bool {
    let selection = provenance.profile().selection();
    provenance.source_format() == SourceFormatV1::Fbx
        && provenance.raw_source().source_format() == SourceFormatV1::Fbx
        && selection.family() == ENGINE_CLIP_BOUNDARY_PROFILE_FAMILY
        && selection.profile_revision() == ENGINE_CLIP_BOUNDARY_PROFILE_REVISION
        && selection.engine_version() == ENGINE_CLIP_BOUNDARY_ENGINE_VERSION
        && selection.importer() == ENGINE_CLIP_BOUNDARY_IMPORTER
        && provenance.profile().facts_identity().sha256()
            == ENGINE_CLIP_BOUNDARY_PROFILE_FACTS_SHA256
        && provenance.profile().facts_identity().bytes() == ENGINE_CLIP_BOUNDARY_PROFILE_FACTS_BYTES
        && matches!(
            provenance
                .profile()
                .fact(EngineFactIdV1::WholeEndFrameRequired)
                .map(|fact| fact.state()),
            Some(EngineFactStateV1::Known(EngineFactValueV1::Boolean(true)))
        )
        && provenance
            .profile()
            .source(ENGINE_CLIP_BOUNDARY_SOURCE_ID)
            .is_some()
}

fn validate_current_engine_clip_boundary_applicability_v3(
    check_id: &str,
    applicability: Applicability,
    provenance: Option<&PredictionProvenanceV3>,
) -> Result<(), PredictionContractError> {
    if check_id != ENGINE_CLIP_BOUNDARY_CHECK_ID {
        return Ok(());
    }
    let expected = match provenance {
        Some(provenance)
            if current_engine_clip_boundary_profile_matches_v3(provenance)
                && !(provenance.raw_source().clips_coverage().state()
                    == RawSourceSetCoverageStateV1::Complete
                    && provenance
                        .raw_source()
                        .exact_source_timing()
                        .is_some_and(|timing| timing.clips().is_empty())) =>
        {
            Applicability::Applicable
        }
        _ => Applicability::NotApplicable,
    };
    if applicability != expected {
        return Err(PredictionContractError::EngineClipBoundaryFacetMismatch);
    }
    Ok(())
}

/// Re-derive the frozen output-v14 clip-boundary rule from embedded V3
/// provenance. This keeps readback and producer construction from accepting a
/// merely well-shaped facet whose scope, basis, availability, reason, or
/// finding disagrees with the retained exact source timing.
fn validate_current_engine_clip_boundary_prediction_v3(
    check_id: &str,
    prediction: &EnginePredictionV3,
    provenance: &PredictionProvenanceV3,
    evaluated_scopes: &[EvaluationScope],
    finding_scopes: &[&EvaluationScope],
) -> Result<(), PredictionContractError> {
    if check_id != ENGINE_CLIP_BOUNDARY_CHECK_ID {
        return Ok(());
    }

    if !current_engine_clip_boundary_profile_matches_v3(provenance) {
        return Err(PredictionContractError::EngineClipBoundaryFacetMismatch);
    }

    let expected_rows = provenance.settings().clips().len();
    let timing = provenance.raw_source().exact_source_timing();
    if timing.is_some_and(|timing| timing.clips().len() != expected_rows) {
        return Err(PredictionContractError::EngineClipBoundaryFacetMismatch);
    }
    let inventory_incomplete =
        provenance.raw_source().clips_coverage().state() != RawSourceSetCoverageStateV1::Complete;
    let has_budget_summary = prediction.has_facet_budget_summary();
    let mut seen_rows = vec![false; expected_rows];
    let mut row_facets = 0usize;
    let mut inventory_facets = 0usize;
    let mut available_scopes = Vec::new();
    let mut expected_finding_scopes = Vec::new();

    for facet in prediction.facets() {
        if facet.reasons() == [PredictionUnavailableReasonV2::FacetBudgetExceeded] {
            if facet.basis() != &engine_clip_boundary_inventory_basis(timing)? {
                return Err(PredictionContractError::EngineClipBoundaryFacetMismatch);
            }
            continue;
        }
        if facet.scope().code.as_str() == "engine_clip_boundary" {
            let Some(source_clip_index) = facet
                .scope()
                .subject
                .as_deref()
                .and_then(|subject| subject.strip_prefix("source_stack:"))
                .and_then(|index| index.parse::<usize>().ok())
            else {
                return Err(PredictionContractError::EngineClipBoundaryFacetMismatch);
            };
            if source_clip_index >= expected_rows
                || std::mem::replace(&mut seen_rows[source_clip_index], true)
            {
                return Err(PredictionContractError::EngineClipBoundaryFacetMismatch);
            }
            row_facets += 1;
            let expected_basis = engine_clip_boundary_stack_basis(timing, source_clip_index)?;
            if facet.basis() != &expected_basis {
                return Err(PredictionContractError::EngineClipBoundaryFacetMismatch);
            }

            let exact = timing.and_then(|timing| {
                let declared = matches!(
                    timing.declared_time_mode().state(),
                    ExactSourceTimingObservationStateWireV1::Observed(_)
                );
                let period = match timing.frame_period().state() {
                    ExactSourceTimingObservationStateWireV1::Observed(period) => {
                        Some(period.units_per_frame())
                    }
                    _ => None,
                };
                let end = match timing.clips()[source_clip_index]
                    .source_time_range()
                    .state()
                {
                    ExactSourceTimingObservationStateWireV1::Observed(range) => {
                        Some(range.end_units())
                    }
                    _ => None,
                };
                declared.then_some(())?;
                Some((period?, end?))
            });
            match exact {
                Some((period, end)) => {
                    if facet.state() != EnginePredictionFacetStateV1::Available
                        || !facet.reasons().is_empty()
                    {
                        return Err(PredictionContractError::EngineClipBoundaryFacetMismatch);
                    }
                    available_scopes.push(facet.scope());
                    if end.rem_euclid(period) != 0 {
                        expected_finding_scopes.push(facet.scope());
                    }
                }
                None => {
                    let expected_reasons =
                        engine_clip_boundary_unavailable_reasons(timing, source_clip_index)?;
                    if facet.state() != EnginePredictionFacetStateV1::RequiredPredictionUnavailable
                        || facet.reasons() != expected_reasons
                    {
                        return Err(PredictionContractError::EngineClipBoundaryFacetMismatch);
                    }
                }
            }
        } else if facet.scope().code.as_str() == "engine_clip_boundary_inventory"
            && facet.scope().subject.is_none()
        {
            inventory_facets += 1;
            if inventory_facets != 1
                || !inventory_incomplete
                || facet.state() != EnginePredictionFacetStateV1::RequiredPredictionUnavailable
                || facet.reasons() != [PredictionUnavailableReasonV2::RawSourceIncomplete]
                || facet.basis() != &engine_clip_boundary_inventory_basis(timing)?
            {
                return Err(PredictionContractError::EngineClipBoundaryFacetMismatch);
            }
        } else {
            return Err(PredictionContractError::EngineClipBoundaryFacetMismatch);
        }
    }

    if seen_rows[..row_facets].iter().any(|seen| !seen)
        || seen_rows[row_facets..].iter().any(|seen| *seen)
    {
        return Err(PredictionContractError::EngineClipBoundaryFacetMismatch);
    }
    let candidate_facets = row_facets + inventory_facets;
    let expected_demand = expected_rows + usize::from(inventory_incomplete);
    if has_budget_summary {
        if candidate_facets >= expected_demand
            || inventory_facets != usize::from(inventory_incomplete && candidate_facets != 0)
        {
            return Err(PredictionContractError::EngineClipBoundaryFacetMismatch);
        }
    } else if row_facets != expected_rows || inventory_facets != usize::from(inventory_incomplete) {
        return Err(PredictionContractError::EngineClipBoundaryFacetMismatch);
    }
    if evaluated_scopes.len() != available_scopes.len()
        || evaluated_scopes
            .iter()
            .any(|scope| !available_scopes.contains(&scope))
    {
        return Err(PredictionContractError::EngineClipBoundaryFacetMismatch);
    }
    if finding_scopes.len() != expected_finding_scopes.len()
        || expected_finding_scopes.iter().any(|expected| {
            finding_scopes
                .iter()
                .filter(|actual| **actual == *expected)
                .count()
                != 1
        })
    {
        return Err(PredictionContractError::EngineClipBoundaryFindingMismatch);
    }
    Ok(())
}

fn engine_clip_boundary_common_basis()
-> Result<Vec<PredictionBasisReferenceV2>, PredictionContractError> {
    Ok(vec![
        PredictionBasisReferenceV2::v1(PredictionBasisReferenceV1::profile_fact(
            "whole_end_frame_required",
        )?),
        PredictionBasisReferenceV2::v1(PredictionBasisReferenceV1::primary_source(
            ENGINE_CLIP_BOUNDARY_SOURCE_ID,
        )?),
    ])
}

fn engine_clip_boundary_exact_reference(
    binding: &ExactSourceTimingBindingV1,
    domain: ExactSourceTimingDomainV1,
    key: ExactSourceTimingKeyV1,
    field: &'static str,
) -> Result<PredictionBasisReferenceV2, PredictionContractError> {
    Ok(PredictionBasisReferenceV2::exact_source_timing(
        ExactSourceTimingBasisReferenceV1::from_binding(
            domain,
            key,
            RawSourceFieldIdV1::new(field)?,
            binding,
        )?,
    ))
}

fn engine_clip_boundary_stack_basis(
    timing: Option<&ExactSourceTimingBindingV1>,
    source_clip_index: usize,
) -> Result<EnginePredictionBasisV2, PredictionContractError> {
    let mut references = engine_clip_boundary_common_basis()?;
    let Some(timing) = timing else {
        return EnginePredictionBasisV2::new(references);
    };
    let stack_key = ExactSourceTimingKeyV1::Clip {
        source_clip_index: source_clip_index as u64,
    };
    for (domain, key, field) in [
        (
            ExactSourceTimingDomainV1::Document,
            ExactSourceTimingKeyV1::Document,
            "declared_time_mode.state",
        ),
        (
            ExactSourceTimingDomainV1::Document,
            ExactSourceTimingKeyV1::Document,
            "frame_period.state",
        ),
        (
            ExactSourceTimingDomainV1::Clip,
            stack_key.clone(),
            "source_time_range.state",
        ),
    ] {
        references.push(engine_clip_boundary_exact_reference(
            timing, domain, key, field,
        )?);
    }
    if matches!(
        timing.declared_time_mode().state(),
        ExactSourceTimingObservationStateWireV1::Observed(_)
    ) {
        references.push(engine_clip_boundary_exact_reference(
            timing,
            ExactSourceTimingDomainV1::Document,
            ExactSourceTimingKeyV1::Document,
            "declared_time_mode.value.time_mode",
        )?);
    }
    if matches!(
        timing.frame_period().state(),
        ExactSourceTimingObservationStateWireV1::Observed(_)
    ) {
        references.push(engine_clip_boundary_exact_reference(
            timing,
            ExactSourceTimingDomainV1::Document,
            ExactSourceTimingKeyV1::Document,
            "frame_period.value.units_per_frame",
        )?);
    }
    if matches!(
        timing.clips()[source_clip_index]
            .source_time_range()
            .state(),
        ExactSourceTimingObservationStateWireV1::Observed(_)
    ) {
        references.push(engine_clip_boundary_exact_reference(
            timing,
            ExactSourceTimingDomainV1::Clip,
            stack_key,
            "source_time_range.value.end_units",
        )?);
    }
    EnginePredictionBasisV2::new(references)
}

fn engine_clip_boundary_inventory_basis(
    timing: Option<&ExactSourceTimingBindingV1>,
) -> Result<EnginePredictionBasisV2, PredictionContractError> {
    let mut references = engine_clip_boundary_common_basis()?;
    if let Some(timing) = timing {
        for field in ["clip_coverage.state", "clip_coverage.reason"] {
            references.push(engine_clip_boundary_exact_reference(
                timing,
                ExactSourceTimingDomainV1::Document,
                ExactSourceTimingKeyV1::Document,
                field,
            )?);
        }
    }
    EnginePredictionBasisV2::new(references)
}

fn engine_clip_boundary_unavailable_reasons(
    timing: Option<&ExactSourceTimingBindingV1>,
    source_clip_index: usize,
) -> Result<Vec<PredictionUnavailableReasonV2>, PredictionContractError> {
    let Some(timing) = timing else {
        return Ok(vec![PredictionUnavailableReasonV2::custom(
            "animsmith:exact_source_timing_unavailable",
        )?]);
    };
    let mut reasons = Vec::new();
    if !matches!(
        timing.declared_time_mode().state(),
        ExactSourceTimingObservationStateWireV1::Observed(_)
    ) {
        reasons.push(PredictionUnavailableReasonV2::custom(
            "animsmith:source_declared_time_mode_unavailable",
        )?);
    }
    if !matches!(
        timing.frame_period().state(),
        ExactSourceTimingObservationStateWireV1::Observed(_)
    ) {
        reasons.push(PredictionUnavailableReasonV2::custom(
            "animsmith:source_frame_period_unavailable",
        )?);
    }
    if !matches!(
        timing.clips()[source_clip_index]
            .source_time_range()
            .state(),
        ExactSourceTimingObservationStateWireV1::Observed(_)
    ) {
        reasons.push(PredictionUnavailableReasonV2::custom(
            "animsmith:source_clip_time_range_unavailable",
        )?);
    }
    reasons.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    Ok(reasons)
}

impl MeasurementReportInput {
    /// Read one report through the immutable output-v11 byte bound before
    /// UTF-8 or JSON parsing.
    ///
    /// The JSON parser receives at most [`OUTPUT_V11_MAX_REPORT_BYTES`] bytes
    /// and retains its recursion limit. This function never performs an
    /// unbounded `read_to_end` or constructs a generic JSON value.
    ///
    /// # Errors
    ///
    /// Returns a typed I/O, N+1 size, or JSON-shape error. Semantic contract
    /// validation remains in [`Self::into_files`].
    pub fn read_from(reader: impl Read) -> Result<Self, MeasurementReportReadError> {
        Self::read_from_with_limit(reader, OUTPUT_V11_MAX_REPORT_BYTES)
    }

    fn read_from_with_limit(
        reader: impl Read,
        limit: u64,
    ) -> Result<Self, MeasurementReportReadError> {
        let mut bounded = reader.take(limit + 1);
        let mut bytes = Vec::new();
        bounded
            .read_to_end(&mut bytes)
            .map_err(|source| MeasurementReportReadError::Io { source })?;
        if bytes.len() as u64 > limit {
            return Err(MeasurementReportReadError::ReportTooLarge { limit });
        }
        serde_json::from_slice(&bytes)
            .map_err(|source| MeasurementReportReadError::InvalidJson { source })
    }

    /// Number of file records present before nested record validation.
    ///
    /// Returns `None` when the report omitted its file array. Consumers can
    /// retain this count while [`MeasurementReportInput::into_files`] performs
    /// full validation, then apply their own cardinality and error policy.
    pub fn file_count(&self) -> Option<usize> {
        self.files.as_ref().map(Vec::len)
    }

    /// Validate current output/measurement identities and recover every file's
    /// complete measurement record from a `measure` or `lint` report.
    ///
    /// File order is preserved. Empty and multi-file reports are accepted so
    /// callers can apply their own cardinality policy.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a missing or unsupported identity, command,
    /// file shape, nested measurement contract, or measurement payload.
    pub fn into_files(self) -> Result<Vec<MeasurementReportFile>, MeasurementReportError> {
        #[derive(Clone, Copy, PartialEq, Eq)]
        enum ReaderRevision {
            V11,
            V12,
            V13,
            V14,
            V15,
            V16,
            V17,
            V18,
        }

        let revision = match self.schema_version {
            Some(OUTPUT_V11_SCHEMA_VERSION) => ReaderRevision::V11,
            Some(OUTPUT_V12_SCHEMA_VERSION) => ReaderRevision::V12,
            Some(OUTPUT_V13_SCHEMA_VERSION) => ReaderRevision::V13,
            Some(OUTPUT_V14_SCHEMA_VERSION) => ReaderRevision::V14,
            Some(OUTPUT_V15_SCHEMA_VERSION) => ReaderRevision::V15,
            Some(OUTPUT_V16_SCHEMA_VERSION) => ReaderRevision::V16,
            Some(OUTPUT_V17_SCHEMA_VERSION) => ReaderRevision::V17,
            Some(OUTPUT_SCHEMA_VERSION) => ReaderRevision::V18,
            Some(found) => {
                return Err(MeasurementReportError::UnsupportedOutputVersion { found });
            }
            None => return Err(MeasurementReportError::MissingOutputVersion),
        };
        let expected_schema = match revision {
            ReaderRevision::V11 => OUTPUT_V11_SCHEMA_ID,
            ReaderRevision::V12 => OUTPUT_V12_SCHEMA_ID,
            ReaderRevision::V13 => OUTPUT_V13_SCHEMA_ID,
            ReaderRevision::V14 => OUTPUT_V14_SCHEMA_ID,
            ReaderRevision::V15 => OUTPUT_V15_SCHEMA_ID,
            ReaderRevision::V16 => OUTPUT_V16_SCHEMA_ID,
            ReaderRevision::V17 => OUTPUT_V17_SCHEMA_ID,
            ReaderRevision::V18 => OUTPUT_SCHEMA_ID,
        };
        if self.schema.as_deref() != Some(expected_schema) {
            return Err(MeasurementReportError::WrongOutputIdentity);
        }
        let command = match self.command.as_deref() {
            Some(command @ ("measure" | "lint")) => command,
            Some(command) => {
                return Err(MeasurementReportError::UnsupportedCommand {
                    command: command.to_owned(),
                });
            }
            None => return Err(MeasurementReportError::MissingCommand),
        };
        if let Some(field) = self.extra.keys().next() {
            return Err(MeasurementReportError::UnknownOutputField {
                field: field.clone(),
            });
        }
        if self._tool.is_none() {
            return Err(MeasurementReportError::MissingTool);
        }
        // The V11 reader retains the same summary obligation as the released
        // V1 contract; only its prediction attachment identity differs.
        validate_prediction_summary_presence(command, self.summary.as_ref())?;
        let files = self.files.ok_or(MeasurementReportError::MissingFiles)?;
        if files.len() > OUTPUT_V11_MAX_FILES {
            return Err(MeasurementReportError::TooManyFiles {
                found: files.len(),
                limit: OUTPUT_V11_MAX_FILES,
            });
        }
        let mut available = 0usize;
        let mut unavailable = 0usize;
        let mut decoded_files = Vec::with_capacity(files.len());
        for (file_index, raw) in files.into_iter().enumerate() {
            let file = if revision == ReaderRevision::V11 {
                let file = decode_legacy_v11_file(command, file_index, &raw)?;
                let (file_available, file_unavailable) =
                    validate_legacy_v11_prediction_phase_file(command, file_index, &file)?;
                available = available
                    .checked_add(file_available)
                    .ok_or(MeasurementReportError::PredictionFacetSummaryMismatch)?;
                unavailable = unavailable
                    .checked_add(file_unavailable)
                    .ok_or(MeasurementReportError::PredictionFacetSummaryMismatch)?;
                file
            } else if revision == ReaderRevision::V14 {
                let file = decode_prediction_phase_file_v14(command, file_index, &raw)?;
                let (file_available, file_unavailable) =
                    validate_prediction_phase_file_v14(command, file_index, &file)?;
                available = available
                    .checked_add(file_available)
                    .ok_or(MeasurementReportError::PredictionFacetSummaryMismatch)?;
                unavailable = unavailable
                    .checked_add(file_unavailable)
                    .ok_or(MeasurementReportError::PredictionFacetSummaryMismatch)?;
                file
            } else if revision == ReaderRevision::V15 {
                let file = decode_prediction_phase_file_v15(command, file_index, &raw)?;
                let (file_available, file_unavailable) =
                    validate_prediction_phase_file_v15(command, file_index, &file)?;
                available = available
                    .checked_add(file_available)
                    .ok_or(MeasurementReportError::PredictionFacetSummaryMismatch)?;
                unavailable = unavailable
                    .checked_add(file_unavailable)
                    .ok_or(MeasurementReportError::PredictionFacetSummaryMismatch)?;
                file
            } else if revision == ReaderRevision::V16 {
                let file = decode_prediction_phase_file_v16(command, file_index, &raw)?;
                let (file_available, file_unavailable) =
                    validate_prediction_phase_file_v16(command, file_index, &file)?;
                available = available
                    .checked_add(file_available)
                    .ok_or(MeasurementReportError::PredictionFacetSummaryMismatch)?;
                unavailable = unavailable
                    .checked_add(file_unavailable)
                    .ok_or(MeasurementReportError::PredictionFacetSummaryMismatch)?;
                file
            } else if matches!(revision, ReaderRevision::V17 | ReaderRevision::V18) {
                let file = decode_prediction_phase_file_v17(command, file_index, &raw)?;
                let (file_available, file_unavailable) =
                    validate_prediction_phase_file_v17(command, file_index, &file)?;
                available = available
                    .checked_add(file_available)
                    .ok_or(MeasurementReportError::PredictionFacetSummaryMismatch)?;
                unavailable = unavailable
                    .checked_add(file_unavailable)
                    .ok_or(MeasurementReportError::PredictionFacetSummaryMismatch)?;
                file
            } else {
                let expected_measurement_schema = if revision == ReaderRevision::V12 {
                    MEASUREMENTS_V15_SCHEMA_ID
                } else {
                    MEASUREMENTS_V16_SCHEMA_ID
                };
                let file = decode_prediction_phase_file(
                    command,
                    file_index,
                    &raw,
                    expected_measurement_schema,
                )?;
                let (file_available, file_unavailable) = validate_prediction_phase_file(
                    command,
                    file_index,
                    &file,
                    expected_measurement_schema,
                )?;
                available = available
                    .checked_add(file_available)
                    .ok_or(MeasurementReportError::PredictionFacetSummaryMismatch)?;
                unavailable = unavailable
                    .checked_add(file_unavailable)
                    .ok_or(MeasurementReportError::PredictionFacetSummaryMismatch)?;
                file
            };
            decoded_files.push(file);
        }
        validate_prediction_summary(command, self.summary.as_ref(), available, unavailable)?;
        let parsed = decoded_files
            .into_iter()
            .enumerate()
            .map(|(file_index, file)| {
                let path = file.path.ok_or_else(|| {
                    MeasurementReportError::file(file_index, MeasurementFileError::MissingPath)
                })?;
                let input = file.input.ok_or_else(|| {
                    MeasurementReportError::file(file_index, MeasurementFileError::MissingInput)
                })?;
                let sha256 = input.sha256.ok_or_else(|| {
                    MeasurementReportError::file(file_index, MeasurementFileError::MissingSha256)
                })?;
                if sha256.len() != 64
                    || !sha256
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
                {
                    return Err(MeasurementReportError::file(
                        file_index,
                        MeasurementFileError::InvalidSha256,
                    ));
                }
                let bytes = input.bytes.ok_or_else(|| {
                    MeasurementReportError::file(file_index, MeasurementFileError::MissingBytes)
                })?;
                let measurements = file.measurements.ok_or_else(|| {
                    MeasurementReportError::file(
                        file_index,
                        MeasurementFileError::MissingMeasurements,
                    )
                })?;
                let measurements = decode_measurement_payload(
                    &measurements,
                    matches!(
                        revision,
                        ReaderRevision::V13
                            | ReaderRevision::V14
                            | ReaderRevision::V15
                            | ReaderRevision::V16
                            | ReaderRevision::V17
                            | ReaderRevision::V18
                    ),
                )
                .map_err(|source| {
                    MeasurementReportError::file(
                        file_index,
                        MeasurementFileError::InvalidMeasurementsShape {
                            reason: source.to_string(),
                        },
                    )
                })?;
                let (expected_measurement_version, expected_measurement_schema) = match revision {
                    ReaderRevision::V11 | ReaderRevision::V12 => {
                        (MEASUREMENTS_V15_SCHEMA_VERSION, MEASUREMENTS_V15_SCHEMA_ID)
                    }
                    ReaderRevision::V13
                    | ReaderRevision::V14
                    | ReaderRevision::V15
                    | ReaderRevision::V16
                    | ReaderRevision::V17 => {
                        (MEASUREMENTS_V16_SCHEMA_VERSION, MEASUREMENTS_V16_SCHEMA_ID)
                    }
                    ReaderRevision::V18 => (MEASUREMENTS_SCHEMA_VERSION, MEASUREMENTS_SCHEMA_ID),
                };
                match measurements.schema_version {
                    Some(found) if found == expected_measurement_version => {}
                    Some(found) => {
                        return Err(MeasurementReportError::file(
                            file_index,
                            MeasurementFileError::UnsupportedMeasurementVersion { found },
                        ));
                    }
                    None => {
                        return Err(MeasurementReportError::file(
                            file_index,
                            MeasurementFileError::MissingMeasurementVersion,
                        ));
                    }
                }
                if measurements.schema.as_deref() != Some(expected_measurement_schema) {
                    return Err(MeasurementReportError::file(
                        file_index,
                        MeasurementFileError::WrongMeasurementIdentity,
                    ));
                }
                let clips = measurements.clips.ok_or_else(|| {
                    MeasurementReportError::file(file_index, MeasurementFileError::MissingClips)
                })?;
                let material_resource_coverage =
                    measurements.material_resource_coverage.ok_or_else(|| {
                        MeasurementReportError::file(
                            file_index,
                            MeasurementFileError::MissingMaterialResourceCoverage,
                        )
                    })?;
                let material_definitions = measurements.material_definitions.ok_or_else(|| {
                    MeasurementReportError::file(
                        file_index,
                        MeasurementFileError::MissingMaterialDefinitions,
                    )
                })?;
                let textures = measurements.textures.ok_or_else(|| {
                    MeasurementReportError::file(file_index, MeasurementFileError::MissingTextures)
                })?;
                let images = measurements.images.ok_or_else(|| {
                    MeasurementReportError::file(file_index, MeasurementFileError::MissingImages)
                })?;
                let skeleton_source_coverage =
                    measurements.skeleton_source_coverage.ok_or_else(|| {
                        MeasurementReportError::file(
                            file_index,
                            MeasurementFileError::MissingSkeletonSourceCoverage,
                        )
                    })?;
                let skeleton_nodes = measurements.skeleton_nodes.ok_or_else(|| {
                    MeasurementReportError::file(
                        file_index,
                        MeasurementFileError::MissingSkeletonNodes,
                    )
                })?;
                let skeleton_nodes = skeleton_nodes
                    .into_iter()
                    .enumerate()
                    .map(|(offset, node)| match node {
                        SkeletonNodeMeasurementInput::Current(node) => Ok(*node),
                        SkeletonNodeMeasurementInput::Earlier { .. } => {
                            Err(MeasurementReportError::file(
                                file_index,
                                MeasurementFileError::InvalidMeasurements {
                                    source: MeasurementContractError::InvalidStructure {
                                        path: format!("skeleton_nodes[{offset}]"),
                                        reason: "uses a shape from an earlier measurement contract"
                                            .into(),
                                    },
                                },
                            ))
                        }
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let skins = measurements.skins.ok_or_else(|| {
                    MeasurementReportError::file(file_index, MeasurementFileError::MissingSkins)
                })?;
                let skins = skins
                    .into_iter()
                    .enumerate()
                    .map(|(offset, skin)| match skin {
                        SkinMeasurementInput::Current(skin) => Ok(*skin),
                        SkinMeasurementInput::Earlier { .. } => Err(MeasurementReportError::file(
                            file_index,
                            MeasurementFileError::InvalidMeasurements {
                                source: MeasurementContractError::InvalidStructure {
                                    path: format!("skins[{offset}]"),
                                    reason: "uses a shape from an earlier measurement contract"
                                        .into(),
                                },
                            },
                        )),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let mesh_definitions = measurements.mesh_definitions.ok_or_else(|| {
                    MeasurementReportError::file(
                        file_index,
                        MeasurementFileError::MissingMeshDefinitions,
                    )
                })?;
                let node_instances = measurements.node_instances.ok_or_else(|| {
                    MeasurementReportError::file(
                        file_index,
                        MeasurementFileError::MissingNodeInstances,
                    )
                })?;
                let scenes = measurements.scenes.ok_or_else(|| {
                    MeasurementReportError::file(file_index, MeasurementFileError::MissingScenes)
                })?;
                let assets = AssetMeasurements {
                    material_resource_coverage,
                    material_definitions,
                    textures,
                    images,
                    skeleton_source_coverage,
                    skeleton_nodes,
                    skins,
                    mesh_definitions,
                    node_instances,
                    scenes,
                    default_scene_index: measurements.default_scene_index,
                };
                let measurements = match revision {
                    ReaderRevision::V11 | ReaderRevision::V12 => {
                        MeasurementContract::historical_v15(clips, assets)
                    }
                    ReaderRevision::V13
                    | ReaderRevision::V14
                    | ReaderRevision::V15
                    | ReaderRevision::V16
                    | ReaderRevision::V17 => MeasurementContract::historical_v16(clips, assets),
                    ReaderRevision::V18 => MeasurementContract::new(clips, assets),
                }
                .map_err(|source| {
                    MeasurementReportError::file(
                        file_index,
                        MeasurementFileError::InvalidMeasurements { source },
                    )
                })?;
                let prediction_measurements = if revision == ReaderRevision::V18 {
                    measurements.prediction_v16_projection().map_err(|source| {
                        MeasurementReportError::file(
                            file_index,
                            MeasurementFileError::InvalidMeasurements { source },
                        )
                    })?
                } else {
                    measurements.clone()
                };
                if revision == ReaderRevision::V15 {
                    let provenance = file.prediction_provenance_v4.as_present();
                    for (check_index, check) in file
                        .checks_v3
                        .as_deref()
                        .unwrap_or_default()
                        .iter()
                        .enumerate()
                    {
                        validate_current_engine_unit_scale_prediction_v4(
                            &check.check_id,
                            check.selection,
                            check.configuration,
                            check.applicability,
                            None,
                            None,
                            &prediction_measurements,
                        )
                        .map_err(|source| {
                            MeasurementReportError::file(
                                file_index,
                                MeasurementFileError::InvalidPrediction {
                                    check_index,
                                    source,
                                },
                            )
                        })?;
                    }
                    for (check_index, check) in file
                        .checks_v4
                        .as_deref()
                        .unwrap_or_default()
                        .iter()
                        .enumerate()
                    {
                        validate_current_engine_unit_scale_prediction_v4(
                            &check.check_id,
                            check.selection,
                            check.configuration,
                            check.applicability,
                            check.prediction.as_ref(),
                            provenance,
                            &prediction_measurements,
                        )
                        .map_err(|source| {
                            MeasurementReportError::file(
                                file_index,
                                MeasurementFileError::InvalidPrediction {
                                    check_index,
                                    source,
                                },
                            )
                        })?;
                    }
                }
                if matches!(
                    revision,
                    ReaderRevision::V16 | ReaderRevision::V17 | ReaderRevision::V18
                ) && !matches!(file.prediction_provenance_v5, RequiredNullable::Missing)
                {
                    let provenance = file.prediction_provenance_v5.as_present();
                    for (check_index, check) in file
                        .checks_v5
                        .as_deref()
                        .unwrap_or_default()
                        .iter()
                        .enumerate()
                    {
                        validate_current_engine_unit_scale_prediction_v5(
                            &check.check_id,
                            check.selection,
                            check.configuration,
                            check.applicability,
                            check.prediction.as_ref(),
                            provenance,
                            &prediction_measurements,
                        )
                        .map_err(|source| {
                            MeasurementReportError::file(
                                file_index,
                                MeasurementFileError::InvalidPrediction {
                                    check_index,
                                    source,
                                },
                            )
                        })?;
                    }
                }
                if matches!(revision, ReaderRevision::V17 | ReaderRevision::V18)
                    && !matches!(file.prediction_provenance_v6, RequiredNullable::Missing)
                {
                    let provenance = file.prediction_provenance_v6.as_present();
                    let rig = file.rig_v17.as_ref().ok_or_else(|| {
                        MeasurementReportError::file(
                            file_index,
                            MeasurementFileError::InvalidFileShape {
                                reason: "current role-policy rig evidence was not retained".into(),
                            },
                        )
                    })?;
                    for (check_index, check) in file
                        .checks_v6
                        .as_deref()
                        .unwrap_or_default()
                        .iter()
                        .enumerate()
                    {
                        validate_current_engine_root_motion_prediction_v6(
                            &check.check_id,
                            check.selection,
                            check.configuration,
                            check.applicability,
                            check.prediction.as_ref(),
                            provenance,
                            &check.findings,
                            rig,
                            &prediction_measurements,
                        )
                        .map_err(|source| {
                            MeasurementReportError::file(
                                file_index,
                                MeasurementFileError::InvalidPrediction {
                                    check_index,
                                    source,
                                },
                            )
                        })?;
                    }
                }
                Ok((
                    MeasurementReportFile {
                        path,
                        input: InputIdentity { sha256, bytes },
                        measurements,
                    },
                    (
                        file.checks.unwrap_or_default(),
                        file.legacy_checks.unwrap_or_default(),
                        file.checks_v3.unwrap_or_default(),
                        file.checks_v4.unwrap_or_default(),
                        file.checks_v5.unwrap_or_default(),
                        file.checks_v6.unwrap_or_default(),
                    ),
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;

        // Measurement-dependent basis pointers are deliberately resolved only
        // after every file's complete, version-routed measurements contract has passed.
        for (
            file_index,
            (file, (checks, legacy_checks, checks_v3, checks_v4, checks_v5, checks_v6)),
        ) in parsed.iter().enumerate()
        {
            let prediction_measurements = if revision == ReaderRevision::V18 {
                file.measurements
                    .prediction_v16_projection()
                    .map_err(|source| {
                        MeasurementReportError::file(
                            file_index,
                            MeasurementFileError::InvalidMeasurements { source },
                        )
                    })?
            } else {
                file.measurements.clone()
            };
            validate_measurement_references_batch_v4(
                &prediction_measurements,
                checks_v6
                    .iter()
                    .enumerate()
                    .filter_map(|(check_index, check)| {
                        check
                            .prediction
                            .as_ref()
                            .map(|prediction| (check_index, prediction.base_prediction()))
                    }),
            )
            .map_err(|error| {
                MeasurementReportError::file(
                    file_index,
                    MeasurementFileError::InvalidPrediction {
                        check_index: error.prediction_index,
                        source: error.source,
                    },
                )
            })?;
            validate_measurement_references_batch_v4(
                &prediction_measurements,
                checks_v5
                    .iter()
                    .enumerate()
                    .filter_map(|(check_index, check)| {
                        check
                            .prediction
                            .as_ref()
                            .map(|prediction| (check_index, prediction.base_prediction()))
                    }),
            )
            .map_err(|error| {
                MeasurementReportError::file(
                    file_index,
                    MeasurementFileError::InvalidPrediction {
                        check_index: error.prediction_index,
                        source: error.source,
                    },
                )
            })?;
            validate_measurement_references_batch_v4(
                &prediction_measurements,
                checks_v4
                    .iter()
                    .enumerate()
                    .filter_map(|(check_index, check)| {
                        check
                            .prediction
                            .as_ref()
                            .map(|prediction| (check_index, prediction))
                    }),
            )
            .map_err(|error| {
                MeasurementReportError::file(
                    file_index,
                    MeasurementFileError::InvalidPrediction {
                        check_index: error.prediction_index,
                        source: error.source,
                    },
                )
            })?;
            validate_measurement_references_batch_v3(
                &prediction_measurements,
                checks_v3
                    .iter()
                    .enumerate()
                    .filter_map(|(check_index, check)| {
                        check
                            .prediction
                            .as_ref()
                            .map(|prediction| (check_index, prediction))
                    }),
            )
            .map_err(|error| {
                MeasurementReportError::file(
                    file_index,
                    MeasurementFileError::InvalidPrediction {
                        check_index: error.prediction_index,
                        source: error.source,
                    },
                )
            })?;
            validate_measurement_references_batch_v2(
                &prediction_measurements,
                checks
                    .iter()
                    .enumerate()
                    .filter_map(|(check_index, check)| {
                        check
                            .prediction
                            .as_ref()
                            .map(|prediction| (check_index, prediction))
                    }),
            )
            .map_err(|error| {
                MeasurementReportError::file(
                    file_index,
                    MeasurementFileError::InvalidPrediction {
                        check_index: error.prediction_index,
                        source: error.source,
                    },
                )
            })?;
            validate_measurement_references_batch(
                &prediction_measurements,
                legacy_checks
                    .iter()
                    .enumerate()
                    .filter_map(|(check_index, check)| {
                        check
                            .prediction
                            .as_ref()
                            .map(|prediction| (check_index, prediction))
                    }),
            )
            .map_err(|error| {
                MeasurementReportError::file(
                    file_index,
                    MeasurementFileError::InvalidPrediction {
                        check_index: error.prediction_index,
                        source: error.source,
                    },
                )
            })?;
        }
        Ok(parsed.into_iter().map(|(file, _)| file).collect())
    }
}

#[cfg(test)]
mod measurement_report_input_tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::engine_contract::{
        EngineClipSettingsV1, EngineConversionControlV1, EngineCoordinateBasisV1, EngineFactIdV1,
        EngineFactStateV1, EngineFactValueV1, EngineForwardAxisV1, EngineHandednessV1,
        EngineLinearUnitV1, EnginePrimarySourceV1, EngineProfileFactV1, EngineProfileSelectionV1,
        EngineUpAxisV1, ResolvedEngineProfileV1, ResolvedEngineSettingsCoverageV2,
        ResolvedEngineSettingsV1, ResolvedEngineSettingsV2, ResolvedEngineSettingsWorkV2,
    };
    use crate::evaluation::{CheckOutput, EvaluationScope, EvaluationScopeCode};
    use crate::measure::{
        AssetMeasurements, ImageMeasurements, MeshDefinitionMeasurements, PrimitiveMeasurements,
    };
    use crate::prediction::{
        EngineMachineResultV1, EnginePredictionBasisV1, EnginePredictionBasisV2,
        EnginePredictionBasisV4, EnginePredictionFacetV1, EnginePredictionFacetV2,
        EnginePredictionFacetV3, EnginePredictionFacetV4, EnginePredictionV1, EnginePredictionV2,
        EnginePredictionV3, EnginePredictionV4, PredictionBasisReferenceV1,
        PredictionBasisReferenceV2, PredictionBasisReferenceV4, PredictionProvenanceIdentityV4,
        PredictionScalarV1, PredictionUnavailableReasonV1, PredictionUnavailableReasonV2,
        RawSourceBindingV1, RawSourceBindingV2, UnitMappingResultV1,
    };
    use crate::source_facts::SourceFormatV1;
    use crate::{
        DependencyClosureV1, Document, Finding, ImageSourceKind, ImageUnavailableReason,
        MaterialResourceCoverage, ResolvedRoles,
    };

    fn prediction_test_profile() -> ResolvedEngineProfileV1 {
        let all_fact_ids = [
            EngineFactIdV1::AcceptedInputs,
            EngineFactIdV1::AnimationAddressability,
            EngineFactIdV1::AnimationChannelHandling,
            EngineFactIdV1::AnimationTargetAddressability,
            EngineFactIdV1::AxisConversionControl,
            EngineFactIdV1::ConstructHandling,
            EngineFactIdV1::ExactAxisConversion,
            EngineFactIdV1::ExtensionHandling,
            EngineFactIdV1::ResultingHierarchyScale,
            EngineFactIdV1::RootMotionAddressability,
            EngineFactIdV1::TargetCoordinateBasis,
            EngineFactIdV1::TargetLinearUnit,
            EngineFactIdV1::UnitConversionControl,
            EngineFactIdV1::WholeEndFrameRequired,
        ];
        let facts = all_fact_ids
            .into_iter()
            .map(|id| {
                let state = if id == EngineFactIdV1::AcceptedInputs {
                    EngineFactStateV1::Known(EngineFactValueV1::AcceptedFormats(vec![
                        SourceFormatV1::Glb,
                    ]))
                } else {
                    EngineFactStateV1::Unknown
                };
                EngineProfileFactV1::new(id, state)
            })
            .collect();
        ResolvedEngineProfileV1::new(
            EngineProfileSelectionV1::new("test", 1, "1", "test-importer").unwrap(),
            "urn:animsmith:engine-profile:test:1",
            facts,
            vec![],
            vec![
                EnginePrimarySourceV1::new(
                    "test-source",
                    "1",
                    "https://example.invalid/test",
                    "2026-08-20",
                    vec![EngineFactIdV1::AcceptedInputs],
                    vec![],
                )
                .unwrap(),
            ],
        )
        .unwrap()
    }

    fn prediction_test_provenance_v2() -> PredictionProvenanceV2 {
        let raw: RawSourceBindingV1 = serde_json::from_value(serde_json::json!({
            "schema": crate::RAW_SOURCE_FACTS_V1_ID,
            "primary_input": {"sha256": "00".repeat(32), "bytes": 0},
            "source_format": "glb",
            "linear_unit": {
                "state": "observed", "value": 1.0, "disposition": "preserved",
                "provenance": {"kind": "format_defined"}
            },
            "coordinate_basis": {
                "state": "observed",
                "value": {"right": "positive_x", "up": "positive_y", "forward": "positive_z"},
                "disposition": "preserved", "provenance": {"kind": "format_defined"}
            },
            "frames_per_second": {
                "state": "observed", "value": 30.0, "disposition": "preserved",
                "provenance": {"kind": "format_defined"}
            },
            "clips_coverage": {"state": "complete"},
            "constructs_coverage": {"state": "complete"},
            "resources_coverage": {"state": "unavailable", "reason": "parser_unavailable"},
            "source_skeleton_coverage": "unavailable",
            "work": {
                "inspected_rows": 0, "retained_rows": 0,
                "retained_text_bytes": 0, "max_traversal_depth": 0
            }
        }))
        .unwrap();
        let closure = DependencyClosureV1::unavailable(raw.primary_input().clone());
        let profile = prediction_test_profile();
        let settings = ResolvedEngineSettingsV2::new(
            &profile,
            vec![],
            vec![],
            ResolvedEngineSettingsCoverageV2::complete(),
            ResolvedEngineSettingsWorkV2::new(0, 0, 0),
        )
        .unwrap();
        PredictionProvenanceV2::new(profile, SourceFormatV1::Glb, settings, raw, closure).unwrap()
    }

    fn prediction_test_provenance() -> PredictionProvenanceV3 {
        let prior = prediction_test_provenance_v2();
        let raw: RawSourceBindingV2 = serde_json::from_value(serde_json::json!({
            "schema": crate::RAW_SOURCE_FACTS_V2_ID,
            "source_facts": prior.raw_source(),
            "exact_source_timing": null
        }))
        .unwrap();
        PredictionProvenanceV3::new(
            prior.profile().clone(),
            prior.source_format(),
            prior.settings().clone(),
            raw,
            prior.dependency_closure().clone(),
        )
        .unwrap()
    }

    fn partial_engine_provenance() -> PredictionProvenanceV3 {
        let complete = prediction_test_provenance();
        let mut raw_wire = serde_json::to_value(complete.raw_source().source_facts()).unwrap();
        raw_wire["clips_coverage"] = serde_json::json!({
            "state": "partial",
            "reason": "projection_budget_exceeded"
        });
        let raw: RawSourceBindingV2 = serde_json::from_value(serde_json::json!({
            "schema": crate::RAW_SOURCE_FACTS_V2_ID,
            "source_facts": raw_wire,
            "exact_source_timing": null
        }))
        .unwrap();
        let clips = (0..PREDICTION_V1_MAX_FACETS_PER_FILE)
            .map(|index| EngineClipSettingsV1::new(format!("clip-{index:04}"), Vec::new()).unwrap())
            .collect();
        let settings = ResolvedEngineSettingsV2::new(
            complete.profile(),
            Vec::new(),
            clips,
            ResolvedEngineSettingsCoverageV2::actual_clip_rows_exceeded(),
            ResolvedEngineSettingsWorkV2::new(4_097, 4_096, 4_096),
        )
        .unwrap();
        PredictionProvenanceV3::new(
            complete.profile().clone(),
            complete.source_format(),
            settings,
            raw,
            complete.dependency_closure().clone(),
        )
        .unwrap()
    }

    fn prediction_test_measurements() -> MeasurementContract {
        MeasurementContract::new(BTreeMap::new(), AssetMeasurements::default()).unwrap()
    }

    fn prediction_test_measurements_v16() -> MeasurementContract {
        MeasurementContract::historical_v16(BTreeMap::new(), AssetMeasurements::default()).unwrap()
    }

    fn loop_projection_measurements(
        availability: &[MeasurementAvailability],
    ) -> MeasurementContract {
        let bones = availability
            .iter()
            .enumerate()
            .map(|(index, availability)| match availability {
                MeasurementAvailability::Measured => serde_json::json!({
                    "bone_index": index,
                    "bone_name": format!("bone-{index}"),
                    "availability": "measured",
                    "position_delta_m": index as f64,
                    "rotation_delta_deg": index as f64,
                    "seam_velocity_delta_mps": index as f64,
                    "seam_angular_velocity_delta_degps": index as f64,
                }),
                MeasurementAvailability::Unavailable => serde_json::json!({
                    "bone_index": index,
                    "bone_name": format!("bone-{index}"),
                    "availability": "unavailable",
                }),
                MeasurementAvailability::NotApplicable => {
                    panic!("loop-continuity rows are always applicable")
                }
            })
            .collect::<Vec<_>>();
        let clip: ClipMeasurements = serde_json::from_value(serde_json::json!({
            "duration_s": 1.0,
            "frame_count": 3,
            "animated_bones": [],
            "bone_channels": [],
            "bone_rotation_range_deg": {},
            "loop_continuity": { "bones": bones },
            "loop_continuity_availability": "measured",
            "loop_endpoint_mode_availability": "not_applicable",
            "frame_grid_availability": "not_applicable",
            "loop_seam_ratio_availability": "not_applicable",
            "gait_availability": "not_applicable",
            "root_trajectory_availability": "not_applicable",
            "speed_mps_availability": "not_applicable"
        }))
        .unwrap();
        MeasurementContract::new(
            BTreeMap::from([("loop".into(), clip)]),
            AssetMeasurements::default(),
        )
        .unwrap()
    }

    fn measure_wire(measurements: MeasurementContract) -> serde_json::Value {
        let file = MeasureFileReport::new(
            "test.glb",
            InputIdentity::from_bytes(&[]),
            prediction_test_rig(),
            measurements,
        )
        .unwrap();
        let envelope =
            MeasureEnvelope::new(ToolInfo::animsmith(ToolSource::new(None, None)), vec![file])
                .unwrap();
        let wire = serde_json::to_value(envelope).unwrap();
        serde_json::from_value::<MeasurementReportInput>(wire.clone())
            .unwrap()
            .into_files()
            .expect("current measurement fixture reads back");
        wire
    }

    fn primitive_measurement_contract() -> MeasurementContract {
        let mut assets = AssetMeasurements::default();
        assets.mesh_definitions.push(MeshDefinitionMeasurements {
            mesh_index: 0,
            name: "mesh".into(),
            primitives: Some(vec![
                PrimitiveMeasurements {
                    primitive_index: 1,
                    material_index: Some(7),
                    vertex_count: 2,
                    finite_vertex_count: 1,
                    geometry_aabb: Some(Aabb {
                        min: [-2.0, 1.0, 0.0],
                        max: [-2.0, 1.0, 0.0],
                    }),
                    geometry_centroid: Some([-2.0, 1.0, 0.0]),
                },
                PrimitiveMeasurements {
                    primitive_index: 3,
                    material_index: None,
                    vertex_count: 2,
                    finite_vertex_count: 2,
                    geometry_aabb: Some(Aabb {
                        min: [4.0, 3.0, 0.0],
                        max: [6.0, 3.0, 0.0],
                    }),
                    geometry_centroid: Some([5.0, 3.0, 0.0]),
                },
            ]),
            vertex_count: 4,
            geometry_aabb: Some(Aabb {
                min: [-2.0, 1.0, 0.0],
                max: [6.0, 3.0, 0.0],
            }),
            geometry_centroid: Some([8.0 / 3.0, 7.0 / 3.0, 0.0]),
            max_joints_per_vertex: 0,
            weight_sum_min: None,
            weight_sum_max: None,
            additional_influence_sets: Vec::new(),
        });
        MeasurementContract::new(BTreeMap::new(), assets).unwrap()
    }

    fn prediction_test_rig() -> RigInfo {
        RigInfo::from_resolved(&Document::default(), &ResolvedRoles::default()).unwrap()
    }

    fn basis_v2(basis: EnginePredictionBasisV1) -> EnginePredictionBasisV2 {
        EnginePredictionBasisV2::new(
            basis
                .references()
                .iter()
                .cloned()
                .map(PredictionBasisReferenceV2::v1)
                .collect(),
        )
        .unwrap()
    }

    fn unavailable_facet(
        subject: String,
        basis: EnginePredictionBasisV1,
    ) -> EnginePredictionFacetV3 {
        EnginePredictionFacetV3::required_unavailable(
            EvaluationScope::new(EvaluationScopeCode::custom("test:prediction-limit"))
                .subject(subject),
            basis_v2(basis),
            vec![PredictionUnavailableReasonV2::ProjectIntentUnavailable],
        )
        .unwrap()
    }

    fn unavailable_check(
        check_id: &'static str,
        provenance: &PredictionProvenanceV3,
        facets: Vec<EnginePredictionFacetV3>,
    ) -> CheckEvaluation {
        let prediction = EnginePredictionV3::new(provenance.identity().clone(), facets).unwrap();
        CheckEvaluation::evaluated(
            check_id,
            CheckOutput::from_coverage(Vec::new(), Vec::new(), Vec::new())
                .with_engine_prediction_v3(prediction),
        )
        .unwrap()
    }

    fn unavailable_check_v2(
        check_id: &'static str,
        provenance: &PredictionProvenanceV2,
        facets: Vec<EnginePredictionFacetV2>,
    ) -> CheckEvaluation {
        let prediction = EnginePredictionV2::new(provenance.identity().clone(), facets).unwrap();
        CheckEvaluation::evaluated(
            check_id,
            CheckOutput::from_coverage(Vec::new(), Vec::new(), Vec::new())
                .with_engine_prediction_v2(prediction),
        )
        .unwrap()
    }

    fn lint_file(
        provenance: &PredictionProvenanceV3,
        checks: Vec<CheckEvaluation>,
    ) -> Result<LintFileReport, OutputContractError> {
        LintFileReport::new(
            "limit.glb",
            provenance.raw_source().primary_input().clone(),
            prediction_test_rig(),
            Some(provenance.clone()),
            checks,
            prediction_test_measurements_v16(),
        )
    }

    #[test]
    fn output_v15_rejects_mixed_v3_provenance_and_v4_predictions() {
        let provenance = prediction_test_provenance();
        let identity: PredictionProvenanceIdentityV4 =
            serde_json::from_value(serde_json::to_value(provenance.identity()).unwrap()).unwrap();
        let basis = EnginePredictionBasisV4::new(vec![PredictionBasisReferenceV4::v2(
            PredictionBasisReferenceV2::v1(
                PredictionBasisReferenceV1::profile_fact("accepted_inputs").unwrap(),
            ),
        )])
        .unwrap();
        let scope = EvaluationScope::new(EvaluationScopeCode::custom("acme:v4-mixed"));
        let facet = EnginePredictionFacetV4::available(
            scope.clone(),
            basis,
            EngineMachineResultV1::UnitMapping(
                UnitMappingResultV1::gltf_to_engine_world_length_unit(),
            ),
        )
        .unwrap();
        let prediction = EnginePredictionV4::new(identity, vec![facet]).unwrap();
        let check = CheckEvaluation::evaluated(
            "acme-v4-mixed",
            CheckOutput::from_coverage(vec![], vec![scope], vec![])
                .with_engine_prediction_v4(prediction),
        )
        .unwrap();
        assert!(matches!(
            lint_file(&provenance, vec![check]),
            Err(OutputContractError::PredictionRevisionMismatch)
        ));
    }

    #[test]
    fn output_v15_preserves_prediction_without_provenance_error_precedence() {
        let provenance = prediction_test_provenance();
        let check = unavailable_check(
            "acme-v3-without-provenance",
            &provenance,
            vec![unavailable_facet(
                "row".to_owned(),
                EnginePredictionBasisV1::new(Vec::new()).unwrap(),
            )],
        );
        let error = LintFileReport::new(
            "without-provenance.glb",
            provenance.raw_source().primary_input().clone(),
            prediction_test_rig(),
            None,
            vec![check],
            prediction_test_measurements_v16(),
        )
        .unwrap_err();
        assert_eq!(error, OutputContractError::PredictionWithoutProvenance);
    }

    #[test]
    fn output_v15_rejects_active_unit_scale_without_v4_provenance() {
        let check = CheckEvaluation::evaluated(
            ENGINE_UNIT_SCALE_CHECK_ID,
            CheckOutput::from_coverage(Vec::new(), Vec::new(), Vec::new()),
        )
        .unwrap();
        let error = LintFileReport::new_v4(
            "unit-scale-without-provenance.glb",
            InputIdentity::from_bytes(&[]),
            prediction_test_rig(),
            None,
            vec![check],
            prediction_test_measurements_v16(),
        )
        .unwrap_err();
        assert_eq!(
            error,
            OutputContractError::InvalidPrediction(
                PredictionContractError::EngineUnitScaleFacetMismatch
            )
        );
    }

    fn validated_lint_wire(
        provenance: &PredictionProvenanceV3,
        checks: Vec<CheckEvaluation>,
    ) -> serde_json::Value {
        let file = lint_file(provenance, checks).expect("producer accepts exact N");
        let envelope =
            LintEnvelope::new(ToolInfo::animsmith(ToolSource::new(None, None)), vec![file])
                .unwrap();
        let wire = serde_json::to_value(envelope).unwrap();
        let read: MeasurementReportInput = serde_json::from_value(wire.clone()).unwrap();
        read.into_files().expect("reader accepts exact N");
        wire
    }

    fn lint_read_error(wire: serde_json::Value) -> MeasurementReportError {
        let read: MeasurementReportInput = serde_json::from_value(wire).unwrap();
        read.into_files().expect_err("reader must reject N+1")
    }

    fn clip_boundary_profile() -> ResolvedEngineProfileV1 {
        let all_fact_ids = [
            EngineFactIdV1::AcceptedInputs,
            EngineFactIdV1::AnimationAddressability,
            EngineFactIdV1::AnimationChannelHandling,
            EngineFactIdV1::AnimationTargetAddressability,
            EngineFactIdV1::AxisConversionControl,
            EngineFactIdV1::ConstructHandling,
            EngineFactIdV1::ExactAxisConversion,
            EngineFactIdV1::ExtensionHandling,
            EngineFactIdV1::ResultingHierarchyScale,
            EngineFactIdV1::RootMotionAddressability,
            EngineFactIdV1::TargetCoordinateBasis,
            EngineFactIdV1::TargetLinearUnit,
            EngineFactIdV1::UnitConversionControl,
            EngineFactIdV1::WholeEndFrameRequired,
        ];
        let facts = all_fact_ids
            .into_iter()
            .map(|id| {
                let state = match id {
                    EngineFactIdV1::AcceptedInputs => {
                        EngineFactStateV1::Known(EngineFactValueV1::AcceptedFormats(vec![
                            SourceFormatV1::Fbx,
                        ]))
                    }
                    EngineFactIdV1::TargetCoordinateBasis => EngineFactStateV1::Known(
                        EngineFactValueV1::CoordinateBasis(EngineCoordinateBasisV1 {
                            handedness: EngineHandednessV1::Left,
                            up_axis: EngineUpAxisV1::Z,
                            forward_axis: EngineForwardAxisV1::PositiveX,
                        }),
                    ),
                    EngineFactIdV1::TargetLinearUnit => EngineFactStateV1::Known(
                        EngineFactValueV1::LinearUnit(EngineLinearUnitV1::Centimetre),
                    ),
                    EngineFactIdV1::UnitConversionControl
                    | EngineFactIdV1::AxisConversionControl => {
                        EngineFactStateV1::Known(EngineFactValueV1::ConversionControl(
                            EngineConversionControlV1::ImporterOption,
                        ))
                    }
                    EngineFactIdV1::WholeEndFrameRequired => {
                        EngineFactStateV1::Known(EngineFactValueV1::Boolean(true))
                    }
                    _ => EngineFactStateV1::Unknown,
                };
                EngineProfileFactV1::new(id, state)
            })
            .collect();
        ResolvedEngineProfileV1::new(
            EngineProfileSelectionV1::new("unreal", 1, "5.8", "fbx-importer").unwrap(),
            "urn:animsmith:engine-profile:unreal:1",
            facts,
            vec![],
            vec![
                EnginePrimarySourceV1::new(
                    ENGINE_CLIP_BOUNDARY_SOURCE_ID,
                    "5.8",
                    "https://dev.epicgames.com/documentation/en-us/unreal-engine/animation-sequences-in-unreal-engine?application_version=5.8",
                    "2026-08-20",
                    vec![EngineFactIdV1::WholeEndFrameRequired],
                    vec![],
                )
                .unwrap(),
                EnginePrimarySourceV1::new(
                    "unreal-coordinate-system-5.8",
                    "5.8",
                    "https://dev.epicgames.com/documentation/en-us/unreal-engine/coordinate-system-and-spaces-in-unreal-engine?application_version=5.8",
                    "2026-08-20",
                    vec![EngineFactIdV1::TargetCoordinateBasis],
                    vec![],
                )
                .unwrap(),
                EnginePrimarySourceV1::new(
                    "unreal-fbx-import-options-5.8",
                    "5.8",
                    "https://dev.epicgames.com/documentation/en-us/unreal-engine/fbx-import-options-reference-in-unreal-engine?application_version=5.8",
                    "2026-08-20",
                    vec![
                        EngineFactIdV1::AcceptedInputs,
                        EngineFactIdV1::UnitConversionControl,
                        EngineFactIdV1::AxisConversionControl,
                    ],
                    vec![],
                )
                .unwrap(),
                EnginePrimarySourceV1::new(
                    "unreal-units-5.8",
                    "5.8",
                    "https://dev.epicgames.com/documentation/en-us/unreal-engine/units-of-measurement-in-unreal-engine?application_version=5.8",
                    "2026-08-20",
                    vec![EngineFactIdV1::TargetLinearUnit],
                    vec![],
                )
                .unwrap(),
            ],
        )
        .unwrap()
    }

    fn exact_observed(value: serde_json::Value, kind: &'static str) -> serde_json::Value {
        serde_json::json!({
            "state": {"kind": "observed", "value": value},
            "disposition": "preserved",
            "provenance": {"kind": kind}
        })
    }

    fn clip_boundary_raw_wire(unavailable_last: bool) -> serde_json::Value {
        let last_range = if unavailable_last {
            serde_json::json!({
                "state": {"kind": "unavailable", "value": "malformed"},
                "disposition": "baked",
                "provenance": null
            })
        } else {
            exact_observed(
                serde_json::json!({
                    "selection": "primary", "begin_units": 0, "end_units": 9_408_000
                }),
                "parser_projected",
            )
        };
        serde_json::json!({
            "schema": crate::RAW_SOURCE_FACTS_V2_ID,
            "source_facts": {
                "schema": crate::RAW_SOURCE_FACTS_V1_ID,
                "primary_input": {"sha256": "00".repeat(32), "bytes": 0},
                "source_format": "fbx",
                "linear_unit": {
                    "state": "observed", "value": 0.01, "disposition": "preserved",
                    "provenance": {"kind": "format_defined"}
                },
                "coordinate_basis": {
                    "state": "observed",
                    "value": {"right": "positive_x", "up": "positive_y", "forward": "positive_z"},
                    "disposition": "preserved", "provenance": {"kind": "format_defined"}
                },
                "frames_per_second": {
                    "state": "observed", "value": 30.0, "disposition": "preserved",
                    "provenance": {"kind": "format_defined"}
                },
                "clips_coverage": {"state": "complete"},
                "constructs_coverage": {"state": "complete"},
                "resources_coverage": {"state": "unavailable", "reason": "parser_unavailable"},
                "source_skeleton_coverage": "unavailable",
                "work": {
                    "inspected_rows": 3, "retained_rows": 3,
                    "retained_text_bytes": 0, "max_traversal_depth": 0
                }
            },
            "exact_source_timing": {
                "schema": crate::EXACT_SOURCE_TIMING_V1_ID,
                "time_basis": exact_observed(
                    serde_json::json!({"units_per_second": 141_120_000}),
                    "format_defined"
                ),
                "declared_time_mode": exact_observed(
                    serde_json::json!("fps30"), "source_declared"
                ),
                "effective_time_mode": exact_observed(
                    serde_json::json!("fps30"), "parser_projected"
                ),
                "declared_custom_frame_rate": exact_observed(
                    serde_json::json!({"binary64_bits": 30.0_f64.to_bits()}),
                    "source_declared"
                ),
                "frame_period": exact_observed(
                    serde_json::json!({"units_per_frame": 4_704_000}),
                    "derived_from_source"
                ),
                "declared_time_protocol": exact_observed(
                    serde_json::json!("default"), "source_declared"
                ),
                "effective_time_protocol": exact_observed(
                    serde_json::json!("default"), "parser_projected"
                ),
                "clip_coverage": {"state": "complete"},
                "clips": [
                    {
                        "source_clip_index": 0,
                        "source_time_range": exact_observed(
                            serde_json::json!({
                                "selection": "primary", "begin_units": 0,
                                "end_units": 4_704_000
                            }),
                            "parser_projected"
                        )
                    },
                    {
                        "source_clip_index": 1,
                        "source_time_range": exact_observed(
                            serde_json::json!({
                                "selection": "primary", "begin_units": 0,
                                "end_units": 4_704_001
                            }),
                            "parser_projected"
                        )
                    },
                    {"source_clip_index": 2, "source_time_range": last_range}
                ]
            }
        })
    }

    fn clip_boundary_provenance(unavailable_last: bool) -> PredictionProvenanceV3 {
        let raw: RawSourceBindingV2 =
            serde_json::from_value(clip_boundary_raw_wire(unavailable_last)).unwrap();
        let profile = clip_boundary_profile();
        let clips = (0..3)
            .map(|index| EngineClipSettingsV1::new(format!("stack-{index}"), vec![]).unwrap())
            .collect();
        let settings = ResolvedEngineSettingsV2::new(
            &profile,
            vec![],
            clips,
            ResolvedEngineSettingsCoverageV2::complete(),
            ResolvedEngineSettingsWorkV2::new(3, 3, 3),
        )
        .unwrap();
        PredictionProvenanceV3::new(
            profile,
            SourceFormatV1::Fbx,
            settings,
            raw.clone(),
            DependencyClosureV1::unavailable(raw.primary_input().clone()),
        )
        .unwrap()
    }

    fn clip_boundary_provenance_with_settings(
        provenance: &PredictionProvenanceV3,
        profile: ResolvedEngineProfileV1,
        clips: Vec<EngineClipSettingsV1>,
    ) -> PredictionProvenanceV3 {
        let retained = clips.len();
        let settings = ResolvedEngineSettingsV2::new(
            &profile,
            vec![],
            clips,
            ResolvedEngineSettingsCoverageV2::complete(),
            ResolvedEngineSettingsWorkV2::new(retained, retained, retained),
        )
        .unwrap();
        PredictionProvenanceV3::new(
            profile,
            provenance.source_format(),
            settings,
            provenance.raw_source().clone(),
            provenance.dependency_closure().clone(),
        )
        .unwrap()
    }

    fn altered_clip_boundary_source_profile(
        provenance: &PredictionProvenanceV3,
    ) -> ResolvedEngineProfileV1 {
        let sources = provenance
            .profile()
            .primary_sources()
            .iter()
            .map(|source| {
                let url = if source.id() == ENGINE_CLIP_BOUNDARY_SOURCE_ID {
                    format!("{}#altered", source.url())
                } else {
                    source.url().to_owned()
                };
                EnginePrimarySourceV1::new(
                    source.id(),
                    source.target_version(),
                    url,
                    source.verified_on(),
                    source.supported_fact_ids().to_vec(),
                    source.supported_setting_ids().to_vec(),
                )
                .unwrap()
            })
            .collect();
        ResolvedEngineProfileV1::new(
            provenance.profile().selection().clone(),
            provenance.profile().fact_bundle_urn(),
            provenance.profile().facts().to_vec(),
            provenance.profile().setting_descriptors().to_vec(),
            sources,
        )
        .unwrap()
    }

    fn clip_boundary_scope(source_clip_index: usize) -> EvaluationScope {
        EvaluationScope::new(EvaluationScopeCode::ENGINE_CLIP_BOUNDARY)
            .subject(format!("source_stack:{source_clip_index}"))
    }

    struct InapplicableClipBoundaryCheck;

    impl crate::Check for InapplicableClipBoundaryCheck {
        fn id(&self) -> &'static str {
            ENGINE_CLIP_BOUNDARY_CHECK_ID
        }

        fn applicability(&self, _ctx: &crate::CheckCtx<'_>) -> Applicability {
            Applicability::NotApplicable
        }

        fn evaluate(&self, _ctx: &crate::CheckCtx<'_>) -> CheckOutput {
            panic!("an inapplicable check must not be evaluated")
        }
    }

    fn inapplicable_clip_boundary_check() -> CheckEvaluation {
        let document = Document::default();
        let grids = crate::MetricGrids::new(&document);
        let roles = ResolvedRoles::default();
        let config = crate::Config::default();
        let context = crate::CheckCtx::new(&grids, &roles, &config);
        let checks: Vec<Box<dyn crate::Check>> = vec![Box::new(InapplicableClipBoundaryCheck)];
        crate::evaluate_checks(&context, &checks, crate::CheckSelection::All)
            .unwrap()
            .pop()
            .unwrap()
    }

    fn clip_boundary_check(
        provenance: &PredictionProvenanceV3,
        unavailable_last: bool,
        first_basis: Option<EnginePredictionBasisV2>,
    ) -> CheckEvaluation {
        let timing = provenance.raw_source().exact_source_timing();
        let scopes = (0..3).map(clip_boundary_scope).collect::<Vec<_>>();
        let mut facets = Vec::new();
        for (index, scope) in scopes.iter().cloned().enumerate() {
            let basis = if index == 0 {
                first_basis
                    .clone()
                    .unwrap_or_else(|| engine_clip_boundary_stack_basis(timing, index).unwrap())
            } else {
                engine_clip_boundary_stack_basis(timing, index).unwrap()
            };
            if unavailable_last && index == 2 {
                facets.push(
                    EnginePredictionFacetV3::required_unavailable(
                        scope,
                        basis,
                        engine_clip_boundary_unavailable_reasons(timing, index).unwrap(),
                    )
                    .unwrap(),
                );
            } else {
                facets.push(EnginePredictionFacetV3::available(scope, basis).unwrap());
            }
        }
        let prediction = EnginePredictionV3::new(provenance.identity().clone(), facets).unwrap();
        let finding = Finding::new(
            ENGINE_CLIP_BOUNDARY_CHECK_ID,
            Severity::Warning,
            "fractional exact source clip end",
        )
        .prediction_scope(scopes[1].clone());
        let evaluated_scopes = if unavailable_last {
            scopes[..2].to_vec()
        } else {
            scopes
        };
        CheckEvaluation::evaluated(
            ENGINE_CLIP_BOUNDARY_CHECK_ID,
            CheckOutput::from_coverage(vec![finding], evaluated_scopes, vec![])
                .with_engine_prediction_v3(prediction),
        )
        .unwrap()
    }

    fn clip_boundary_lint_wire(unavailable_last: bool) -> serde_json::Value {
        let provenance = clip_boundary_provenance(unavailable_last);
        let check = clip_boundary_check(&provenance, unavailable_last, None);
        let file = LintFileReport::new(
            "test.fbx",
            provenance.raw_source().primary_input().clone(),
            prediction_test_rig(),
            Some(provenance),
            vec![check],
            prediction_test_measurements_v16(),
        )
        .unwrap();
        let envelope =
            LintEnvelope::new(ToolInfo::animsmith(ToolSource::new(None, None)), vec![file])
                .unwrap();
        let wire = serde_json::to_value(envelope).unwrap();
        serde_json::from_value::<MeasurementReportInput>(wire.clone())
            .unwrap()
            .into_files()
            .unwrap();
        wire
    }

    fn assert_clip_boundary_read_error(wire: serde_json::Value, expected: PredictionContractError) {
        assert_eq!(
            lint_read_error(wire),
            MeasurementReportError::File {
                file_index: 0,
                source: MeasurementFileError::InvalidPrediction {
                    check_index: 0,
                    source: expected,
                },
            }
        );
    }

    #[test]
    fn exact_source_raw_source_v2_observed_values_round_trip_and_reject_hostile_mutations() {
        let wire = clip_boundary_raw_wire(false);
        let binding: RawSourceBindingV2 = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(serde_json::to_value(binding).unwrap(), wire);

        let mut invalid_value = wire.clone();
        invalid_value["exact_source_timing"]["frame_period"]["state"]["value"]["units_per_frame"] =
            serde_json::json!(0);
        assert_eq!(
            serde_json::from_value::<RawSourceBindingV2>(invalid_value)
                .unwrap_err()
                .to_string(),
            PredictionContractError::ExactSourceTimingValueMismatch.to_string()
        );

        let mut invalid_coverage = wire.clone();
        invalid_coverage["exact_source_timing"]["clip_coverage"] = serde_json::json!({
            "state": "partial", "reason": "projection_budget_exceeded"
        });
        assert_eq!(
            serde_json::from_value::<RawSourceBindingV2>(invalid_coverage)
                .unwrap_err()
                .to_string(),
            PredictionContractError::ExactSourceTimingCoverageMismatch.to_string()
        );

        let mut invalid_prefix = wire;
        invalid_prefix["exact_source_timing"]["clips"][1]["source_clip_index"] =
            serde_json::json!(2);
        assert_eq!(
            serde_json::from_value::<RawSourceBindingV2>(invalid_prefix)
                .unwrap_err()
                .to_string(),
            PredictionContractError::ExactSourceTimingClipPrefixMismatch.to_string()
        );
    }

    #[test]
    fn clip_boundary_v3_readback_rejects_scope_basis_reason_and_finding_mutations() {
        let wire = clip_boundary_lint_wire(false);

        let mut wrong_scope = wire.clone();
        wrong_scope["files"][0]["checks"][0]["prediction"]["facets"][0]["scope"]["subject"] =
            serde_json::json!("source_stack:9");
        wrong_scope["files"][0]["checks"][0]["evaluated_scopes"][0]["subject"] =
            serde_json::json!("source_stack:9");
        assert_clip_boundary_read_error(
            wrong_scope,
            PredictionContractError::EngineClipBoundaryFacetMismatch,
        );

        let mut wrong_basis = wire.clone();
        wrong_basis["files"][0]["checks"][0]["prediction"]["facets"][0]["basis"] =
            serde_json::to_value(
                EnginePredictionBasisV2::new(engine_clip_boundary_common_basis().unwrap()).unwrap(),
            )
            .unwrap();
        assert_clip_boundary_read_error(
            wrong_basis,
            PredictionContractError::EngineClipBoundaryFacetMismatch,
        );

        let mut missing_finding = wire;
        missing_finding["files"][0]["checks"][0]["findings"] = serde_json::json!([]);
        assert_clip_boundary_read_error(
            missing_finding,
            PredictionContractError::EngineClipBoundaryFindingMismatch,
        );

        let mut wrong_reason = clip_boundary_lint_wire(true);
        wrong_reason["files"][0]["checks"][0]["prediction"]["facets"][2]["reasons"] =
            serde_json::json!(["animsmith:source_frame_period_unavailable"]);
        assert_clip_boundary_read_error(
            wrong_reason,
            PredictionContractError::EngineClipBoundaryFacetMismatch,
        );
    }

    #[test]
    fn clip_boundary_v3_rederives_applicability_for_producer_and_readback() {
        let provenance = clip_boundary_provenance(false);
        assert!(matches!(
            lint_file(&provenance, vec![inapplicable_clip_boundary_check()]),
            Err(OutputContractError::InvalidPrediction(
                PredictionContractError::EngineClipBoundaryFacetMismatch
            ))
        ));

        let mut wire = clip_boundary_lint_wire(false);
        let check = wire["files"][0]["checks"][0].as_object_mut().unwrap();
        check.insert(
            "applicability".to_owned(),
            serde_json::json!("not_applicable"),
        );
        check.insert("evaluation".to_owned(), serde_json::json!("not_evaluated"));
        check.insert("findings".to_owned(), serde_json::json!([]));
        check.remove("evaluated_scopes");
        check.remove("gaps");
        check.remove("prediction");
        assert_clip_boundary_read_error(
            wire,
            PredictionContractError::EngineClipBoundaryFacetMismatch,
        );
    }

    #[test]
    fn clip_boundary_v3_applicability_uses_raw_exact_stack_inventory() {
        let original = clip_boundary_provenance(false);
        let provenance = clip_boundary_provenance_with_settings(
            &original,
            original.profile().clone(),
            Vec::new(),
        );
        assert_eq!(
            provenance
                .raw_source()
                .exact_source_timing()
                .unwrap()
                .clips()
                .len(),
            3
        );
        assert!(provenance.settings().clips().is_empty());
        assert!(matches!(
            lint_file(&provenance, vec![inapplicable_clip_boundary_check()]),
            Err(OutputContractError::InvalidPrediction(
                PredictionContractError::EngineClipBoundaryFacetMismatch
            ))
        ));

        let mut wire = clip_boundary_lint_wire(false);
        wire["files"][0]["prediction_provenance"] = serde_json::to_value(provenance).unwrap();
        let check = wire["files"][0]["checks"][0].as_object_mut().unwrap();
        check.insert(
            "applicability".to_owned(),
            serde_json::json!("not_applicable"),
        );
        check.insert("evaluation".to_owned(), serde_json::json!("not_evaluated"));
        check.insert("findings".to_owned(), serde_json::json!([]));
        check.remove("evaluated_scopes");
        check.remove("gaps");
        check.remove("prediction");
        assert_clip_boundary_read_error(
            wire,
            PredictionContractError::EngineClipBoundaryFacetMismatch,
        );
    }

    #[test]
    fn clip_boundary_v3_binds_the_frozen_unreal_profile_identity() {
        let original = clip_boundary_provenance(false);
        assert_eq!(
            original.profile().facts_identity().sha256(),
            ENGINE_CLIP_BOUNDARY_PROFILE_FACTS_SHA256
        );
        assert_eq!(
            original.profile().facts_identity().bytes(),
            ENGINE_CLIP_BOUNDARY_PROFILE_FACTS_BYTES
        );
        let altered_profile = altered_clip_boundary_source_profile(&original);
        assert_ne!(
            altered_profile.facts_identity(),
            original.profile().facts_identity()
        );
        let altered = clip_boundary_provenance_with_settings(
            &original,
            altered_profile,
            original.settings().clips().to_vec(),
        );
        let altered_check = clip_boundary_check(&altered, false, None);
        assert!(matches!(
            lint_file(&altered, vec![altered_check.clone()]),
            Err(OutputContractError::InvalidPrediction(
                PredictionContractError::EngineClipBoundaryFacetMismatch
            ))
        ));

        let mut wire = clip_boundary_lint_wire(false);
        wire["files"][0]["prediction_provenance"] = serde_json::to_value(altered).unwrap();
        wire["files"][0]["checks"][0] = serde_json::to_value(altered_check).unwrap();
        assert_clip_boundary_read_error(
            wire,
            PredictionContractError::EngineClipBoundaryFacetMismatch,
        );
    }

    #[test]
    fn clip_boundary_v3_producer_rejects_incomplete_exact_basis() {
        let provenance = clip_boundary_provenance(false);
        let incomplete_basis =
            EnginePredictionBasisV2::new(engine_clip_boundary_common_basis().unwrap()).unwrap();
        let check = clip_boundary_check(&provenance, false, Some(incomplete_basis));
        assert!(matches!(
            lint_file(&provenance, vec![check]),
            Err(OutputContractError::InvalidPrediction(
                PredictionContractError::EngineClipBoundaryFacetMismatch
            ))
        ));
    }

    fn prediction_with_retained_text(
        provenance: &PredictionProvenanceV3,
        retained_text: usize,
    ) -> EnginePredictionV3 {
        const FIELD_ID_BYTES: usize = 16;
        const MAX_VALUE_BYTES: usize = crate::PREDICTION_V1_MAX_TEXT_BYTES;
        let fixed = "test:prediction-limit".len()
            + PredictionUnavailableReasonV2::ProjectIntentUnavailable
                .as_str()
                .len();
        let remaining = retained_text.checked_sub(fixed).unwrap();
        let full_row = FIELD_ID_BYTES + MAX_VALUE_BYTES;
        let full_rows = remaining / full_row;
        let remainder = remaining % full_row;
        let (full_rows, tail_lengths) = if remainder == 0 {
            (full_rows, Vec::new())
        } else if remainder >= FIELD_ID_BYTES {
            (full_rows, vec![remainder - FIELD_ID_BYTES])
        } else {
            (
                full_rows - 1,
                vec![0, MAX_VALUE_BYTES - FIELD_ID_BYTES + remainder],
            )
        };
        let mut references = Vec::with_capacity(full_rows + tail_lengths.len());
        for index in 0..full_rows {
            references.push(
                PredictionBasisReferenceV1::project_field(
                    format!("f{index:015}"),
                    PredictionScalarV1::text("x".repeat(MAX_VALUE_BYTES)).unwrap(),
                )
                .unwrap(),
            );
        }
        for length in tail_lengths {
            let index = references.len();
            references.push(
                PredictionBasisReferenceV1::project_field(
                    format!("f{index:015}"),
                    PredictionScalarV1::text("x".repeat(length)).unwrap(),
                )
                .unwrap(),
            );
        }
        let basis = EnginePredictionBasisV1::new(references).unwrap();
        let facet = EnginePredictionFacetV3::required_unavailable(
            EvaluationScope::new(EvaluationScopeCode::custom("test:prediction-limit")),
            basis_v2(basis),
            vec![PredictionUnavailableReasonV2::ProjectIntentUnavailable],
        )
        .unwrap();
        let prediction =
            EnginePredictionV3::new(provenance.identity().clone(), vec![facet]).unwrap();
        assert_eq!(prediction.retained_text_bytes().unwrap(), retained_text);
        prediction
    }

    #[test]
    fn report_reader_enforces_the_byte_cap_before_json_parsing() {
        let bytes = br#"{"schema_version":10,"tool":{}}"#;
        let report =
            MeasurementReportInput::read_from_with_limit(bytes.as_slice(), bytes.len() as u64)
                .expect("exact N must parse");
        assert_eq!(report.schema_version, Some(10));

        assert!(matches!(
            MeasurementReportInput::read_from_with_limit(
                bytes.as_slice(),
                bytes.len() as u64 - 1,
            ),
            Err(MeasurementReportReadError::ReportTooLarge { limit })
                if limit == bytes.len() as u64 - 1
        ));
    }

    #[test]
    fn prediction_facet_file_bound_accepts_n_and_rejects_n_plus_one_on_write_and_read() {
        let provenance = prediction_test_provenance();
        let empty_basis = EnginePredictionBasisV1::new(Vec::new()).unwrap();
        let facets = (0..PREDICTION_V1_MAX_FACETS_PER_FILE)
            .map(|index| unavailable_facet(format!("facet-{index:04}"), empty_basis.clone()))
            .collect();
        let at_limit = unavailable_check("test:facet-limit", &provenance, facets);
        let mut wire = validated_lint_wire(&provenance, vec![at_limit.clone()]);
        let extra = unavailable_check(
            "test:facet-extra",
            &provenance,
            vec![unavailable_facet("facet-extra".into(), empty_basis)],
        );

        assert_eq!(
            lint_file(&provenance, vec![at_limit, extra.clone()]).unwrap_err(),
            OutputContractError::TooManyPredictionFacets {
                found: PREDICTION_V1_MAX_FACETS_PER_FILE + 1,
                limit: PREDICTION_V1_MAX_FACETS_PER_FILE,
            }
        );

        wire["files"][0]["checks"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::to_value(extra).unwrap());
        assert_eq!(
            lint_read_error(wire),
            MeasurementReportError::File {
                file_index: 0,
                source: MeasurementFileError::TooManyPredictionFacets {
                    found: PREDICTION_V1_MAX_FACETS_PER_FILE + 1,
                    limit: PREDICTION_V1_MAX_FACETS_PER_FILE,
                },
            }
        );
    }

    #[test]
    fn v2_budget_summary_is_canonical_and_requires_an_exhausted_file_budget() {
        let provenance = prediction_test_provenance();
        let basis = EnginePredictionBasisV1::new(Vec::new()).unwrap();
        let mut facets = (0..PREDICTION_V1_MAX_FACETS_PER_FILE - 1)
            .map(|index| unavailable_facet(format!("facet-{index:04}"), basis.clone()))
            .collect::<Vec<_>>();
        facets.push(
            EnginePredictionFacetV3::required_unavailable(
                EvaluationScope::new(EvaluationScopeCode::custom("test:budget:facet-budget")),
                basis_v2(basis),
                vec![PredictionUnavailableReasonV2::FacetBudgetExceeded],
            )
            .unwrap(),
        );
        let check = unavailable_check("test:budget", &provenance, facets);
        let wire = validated_lint_wire(&provenance, vec![check]);
        let facets = wire["files"][0]["checks"][0]["prediction"]["facets"]
            .as_array()
            .unwrap();
        let summary_index = facets
            .iter()
            .position(|facet| facet["reasons"] == serde_json::json!(["facet_budget_exceeded"]))
            .unwrap();

        let mut wrong_scope = wire.clone();
        wrong_scope["files"][0]["checks"][0]["prediction"]["facets"][summary_index]["scope"]["code"] =
            serde_json::json!("test:wrong:facet-budget");
        assert!(matches!(
            lint_read_error(wrong_scope),
            MeasurementReportError::File {
                source: MeasurementFileError::InvalidPrediction {
                    source: PredictionContractError::InvalidFacetBudgetSummary,
                    ..
                },
                ..
            }
        ));

        let mut subject = wire.clone();
        subject["files"][0]["checks"][0]["prediction"]["facets"][summary_index]["scope"]["subject"] =
            serde_json::json!("forged");
        assert!(matches!(
            lint_read_error(subject),
            MeasurementReportError::File {
                source: MeasurementFileError::InvalidPrediction {
                    source: PredictionContractError::InvalidFacetBudgetSummary,
                    ..
                },
                ..
            }
        ));

        let mut available = wire.clone();
        available["files"][0]["checks"][0]["prediction"]["facets"][summary_index]["state"] =
            serde_json::json!("available");
        let available_error = lint_read_error(available);
        assert!(
            matches!(
                available_error,
                MeasurementReportError::File {
                    source: MeasurementFileError::InvalidPrediction {
                        source: PredictionContractError::AvailableBasisEmpty,
                        ..
                    },
                    ..
                }
            ),
            "unexpected available mutation: {available_error:?}"
        );

        let mut duplicate = wire.clone();
        let duplicate_summary =
            duplicate["files"][0]["checks"][0]["prediction"]["facets"][summary_index].clone();
        duplicate["files"][0]["checks"][0]["prediction"]["facets"]
            [if summary_index == 0 { 1 } else { 0 }] = duplicate_summary;
        assert!(matches!(
            lint_read_error(duplicate),
            MeasurementReportError::File {
                source: MeasurementFileError::InvalidPrediction {
                    source: PredictionContractError::DuplicateFacetScope,
                    ..
                },
                ..
            }
        ));

        let mut under_full = wire;
        under_full["files"][0]["checks"][0]["prediction"]["facets"]
            .as_array_mut()
            .unwrap()
            .remove(if summary_index == 0 { 1 } else { 0 });
        under_full["summary"]["prediction_facets"]["required_prediction_unavailable"] =
            serde_json::json!(PREDICTION_V1_MAX_FACETS_PER_FILE - 1);
        assert_eq!(
            lint_read_error(under_full),
            MeasurementReportError::File {
                file_index: 0,
                source: MeasurementFileError::FacetBudgetSummaryWithoutExhaustedFileBudget {
                    found: PREDICTION_V1_MAX_FACETS_PER_FILE - 1,
                    limit: PREDICTION_V1_MAX_FACETS_PER_FILE,
                },
            }
        );
    }

    #[test]
    fn partial_engine_inventory_can_be_replaced_by_its_budget_summary() {
        let provenance = partial_engine_provenance();
        let summary = EnginePredictionFacetV3::required_unavailable(
            EvaluationScope::new(EvaluationScopeCode::custom(
                "engine-addressability:facet-budget",
            )),
            basis_v2(EnginePredictionBasisV1::new(Vec::new()).unwrap()),
            vec![PredictionUnavailableReasonV2::FacetBudgetExceeded],
        )
        .unwrap();
        let engine = unavailable_check("engine-addressability", &provenance, vec![summary]);
        let filler_basis = EnginePredictionBasisV1::new(Vec::new()).unwrap();
        let filler = unavailable_check(
            "test:filler",
            &provenance,
            (0..PREDICTION_V1_MAX_FACETS_PER_FILE - 1)
                .map(|index| unavailable_facet(format!("filler-{index:04}"), filler_basis.clone()))
                .collect(),
        );
        // A capacity-zero incomplete inventory is represented by the one
        // canonical summary while other rules consume the retained slots.
        let wire = validated_lint_wire(&provenance, vec![engine, filler]);
        assert_eq!(
            wire["summary"]["prediction_facets"]["required_prediction_unavailable"],
            serde_json::json!(PREDICTION_V1_MAX_FACETS_PER_FILE)
        );
    }

    #[test]
    fn engine_addressability_inventory_reasons_follow_raw_and_settings_coverage() {
        let partial = partial_engine_provenance();
        let basis = EnginePredictionBasisV1::new(Vec::new()).unwrap();
        let inventory = EnginePredictionFacetV3::required_unavailable(
            EvaluationScope::new(EvaluationScopeCode::ANIMATION_ASSET_LABEL_INVENTORY),
            basis_v2(basis.clone()),
            vec![
                PredictionUnavailableReasonV2::RawSourceIncomplete,
                PredictionUnavailableReasonV2::ResolvedSettingsOverflow,
            ],
        )
        .unwrap();
        let check = unavailable_check("engine-addressability", &partial, vec![inventory]);
        let mut wire = validated_lint_wire(&partial, vec![check]);
        wire["files"][0]["checks"][0]["prediction"]["facets"][0]["reasons"] =
            serde_json::json!(["raw_source_incomplete"]);
        assert!(matches!(
            lint_read_error(wire),
            MeasurementReportError::File {
                source: MeasurementFileError::InvalidPrediction {
                    source: PredictionContractError::EngineAddressabilityInventoryReasonsMismatch,
                    ..
                },
                ..
            }
        ));

        let complete = prediction_test_provenance();
        let forged = EnginePredictionFacetV3::required_unavailable(
            EvaluationScope::new(EvaluationScopeCode::ANIMATION_ASSET_LABEL_INVENTORY),
            basis_v2(basis),
            vec![PredictionUnavailableReasonV2::ResolvedSettingsOverflow],
        )
        .unwrap();
        assert!(matches!(
            lint_file(
                &complete,
                vec![unavailable_check(
                    "engine-addressability",
                    &complete,
                    vec![forged]
                )],
            ),
            Err(OutputContractError::InvalidPrediction(
                PredictionContractError::EngineAddressabilityInventoryReasonsMismatch
            ))
        ));
    }

    #[test]
    fn engine_addressability_rejects_a_non_addressability_available_facet_prefix() {
        let mut wire = clip_boundary_lint_wire(false);
        let check = &mut wire["files"][0]["checks"][0];
        check["check_id"] = serde_json::json!("engine-addressability");
        for (index, facet) in check["prediction"]["facets"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .enumerate()
        {
            facet["scope"]["code"] = serde_json::json!("animation_asset_label");
            facet["scope"]["subject"] = serde_json::json!(format!("Animation{index}"));
        }
        for (index, scope) in check["evaluated_scopes"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .enumerate()
        {
            scope["code"] = serde_json::json!("animation_asset_label");
            scope["subject"] = serde_json::json!(format!("Animation{index}"));
        }
        check["findings"][0]["check_id"] = serde_json::json!("engine-addressability");
        check["findings"][0]["prediction_scope"]["code"] =
            serde_json::json!("animation_asset_label");
        check["findings"][0]["prediction_scope"]["subject"] = serde_json::json!("Animation1");

        assert!(matches!(
            lint_read_error(wire),
            MeasurementReportError::File {
                source: MeasurementFileError::InvalidPrediction {
                    source: PredictionContractError::EngineAddressabilityFacetPrefixMismatch,
                    ..
                },
                ..
            }
        ));
    }

    #[test]
    fn prediction_basis_file_bound_accepts_n_and_rejects_n_plus_one_on_write_and_read() {
        let provenance = prediction_test_provenance();
        let basis = EnginePredictionBasisV1::new(
            (0..crate::PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FACET)
                .map(|index| {
                    PredictionBasisReferenceV1::project_field(
                        format!("project.field.{index:04}"),
                        PredictionScalarV1::Null,
                    )
                    .unwrap()
                })
                .collect(),
        )
        .unwrap();
        let facet_count = PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE
            / crate::PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FACET;
        let facets = (0..facet_count)
            .map(|index| unavailable_facet(format!("basis-{index:02}"), basis.clone()))
            .collect();
        let at_limit = unavailable_check("test:basis-limit", &provenance, facets);
        let mut wire = validated_lint_wire(&provenance, vec![at_limit.clone()]);
        let extra_basis = EnginePredictionBasisV1::new(vec![
            PredictionBasisReferenceV1::project_field("project.extra", PredictionScalarV1::Null)
                .unwrap(),
        ])
        .unwrap();
        let extra = unavailable_check(
            "test:basis-extra",
            &provenance,
            vec![unavailable_facet("basis-extra".into(), extra_basis)],
        );

        assert_eq!(
            lint_file(&provenance, vec![at_limit, extra.clone()]).unwrap_err(),
            OutputContractError::TooManyPredictionBasisReferences {
                found: PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE + 1,
                limit: PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE,
            }
        );

        wire["files"][0]["checks"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::to_value(extra).unwrap());
        *wire["files"][0]["checks"]
            .as_array_mut()
            .unwrap()
            .last_mut()
            .unwrap()
            .get_mut("prediction")
            .unwrap()
            .get_mut("facets")
            .and_then(serde_json::Value::as_array_mut)
            .and_then(|facets| facets.first_mut())
            .and_then(|facet| facet.get_mut("basis"))
            .and_then(|basis| basis.get_mut("references"))
            .and_then(serde_json::Value::as_array_mut)
            .and_then(|references| references.first_mut())
            .unwrap() = serde_json::Value::Null;
        assert!(matches!(
            lint_read_error(wire),
            MeasurementReportError::File {
                file_index: 0,
                source: MeasurementFileError::TooManyPredictionBasisReferences { .. },
            }
        ));
    }

    #[test]
    fn prediction_text_file_bound_accepts_n_and_rejects_n_plus_one_on_write_and_read() {
        let provenance = prediction_test_provenance();
        let provenance_text = provenance.retained_text_bytes().unwrap();
        let at_limit_prediction = prediction_with_retained_text(
            &provenance,
            PREDICTION_V1_MAX_TOTAL_TEXT_BYTES_PER_FILE - provenance_text,
        );
        let at_limit = CheckEvaluation::evaluated(
            "test:text-limit",
            CheckOutput::from_coverage(Vec::new(), Vec::new(), Vec::new())
                .with_engine_prediction_v3(at_limit_prediction),
        )
        .unwrap();
        let mut wire = validated_lint_wire(&provenance, vec![at_limit]);

        let above_limit_prediction = prediction_with_retained_text(
            &provenance,
            PREDICTION_V1_MAX_TOTAL_TEXT_BYTES_PER_FILE + 1 - provenance_text,
        );
        let above_limit = CheckEvaluation::evaluated(
            "test:text-limit",
            CheckOutput::from_coverage(Vec::new(), Vec::new(), Vec::new())
                .with_engine_prediction_v3(above_limit_prediction),
        )
        .unwrap();
        let above_limit_wire = serde_json::to_value(&above_limit).unwrap();
        assert_eq!(
            lint_file(&provenance, vec![above_limit]).unwrap_err(),
            OutputContractError::TooMuchPredictionText {
                found: PREDICTION_V1_MAX_TOTAL_TEXT_BYTES_PER_FILE + 1,
                limit: PREDICTION_V1_MAX_TOTAL_TEXT_BYTES_PER_FILE,
            }
        );

        wire["files"][0]["checks"][0] = above_limit_wire;
        assert_eq!(
            lint_read_error(wire),
            MeasurementReportError::File {
                file_index: 0,
                source: MeasurementFileError::TooMuchPredictionText {
                    found: PREDICTION_V1_MAX_TOTAL_TEXT_BYTES_PER_FILE + 1,
                    limit: PREDICTION_V1_MAX_TOTAL_TEXT_BYTES_PER_FILE,
                },
            }
        );
    }

    fn reader_error(wire: serde_json::Value) -> MeasurementReportError {
        serde_json::from_value::<MeasurementReportInput>(wire)
            .expect("outer v11 shape remains valid")
            .into_files()
            .expect_err("mutated report must fail")
    }

    fn empty_check(check_id: &'static str) -> CheckEvaluation {
        CheckEvaluation::evaluated(
            check_id,
            CheckOutput::from_coverage(Vec::new(), Vec::new(), Vec::new()),
        )
        .unwrap()
    }

    #[test]
    fn staged_reader_rejects_unknown_root_file_and_check_fields() {
        let provenance = prediction_test_provenance();
        let wire = validated_lint_wire(&provenance, vec![empty_check("test:reader")]);

        let mut root = wire.clone();
        root["unknown_root"] = serde_json::json!(true);
        let bytes = serde_json::to_vec(&root).unwrap();
        assert_eq!(
            MeasurementReportInput::read_from(bytes.as_slice())
                .expect("unknown root fields are retained through the staged read")
                .into_files()
                .unwrap_err(),
            MeasurementReportError::UnknownOutputField {
                field: "unknown_root".into(),
            }
        );

        let mut missing_tool = wire.clone();
        missing_tool.as_object_mut().unwrap().remove("tool");
        assert_eq!(
            reader_error(missing_tool),
            MeasurementReportError::MissingTool
        );

        let bare = br#"{"walk":true}"#;
        assert_eq!(
            MeasurementReportInput::read_from(bare.as_slice())
                .expect("unknown root fields remain staged until header validation")
                .into_files()
                .unwrap_err(),
            MeasurementReportError::MissingOutputVersion,
        );

        let unsupported = br#"{"schema_version":9,"walk":true}"#;
        assert_eq!(
            MeasurementReportInput::read_from(unsupported.as_slice())
                .expect("unknown root fields remain staged until header validation")
                .into_files()
                .unwrap_err(),
            MeasurementReportError::UnsupportedOutputVersion { found: 9 },
        );

        let mut file = wire.clone();
        file["files"][0]["unknown_file"] = serde_json::json!(true);
        assert!(matches!(
            reader_error(file),
            MeasurementReportError::File {
                file_index: 0,
                source: MeasurementFileError::InvalidFileShape { reason },
            } if reason.contains("unknown field `unknown_file`")
        ));

        let mut check = wire;
        check["files"][0]["checks"][0]["unknown_check"] = serde_json::json!(true);
        assert!(matches!(
            reader_error(check),
            MeasurementReportError::File {
                file_index: 0,
                source: MeasurementFileError::InvalidPredictionShape {
                    check_index: 0,
                    reason,
                },
            } if reason.contains("unknown field `unknown_check`")
        ));

        let provenance = prediction_test_provenance();
        let mut summary = validated_lint_wire(&provenance, vec![empty_check("test:reader")]);
        summary["summary"]["prediction_facets"]["unknown_prediction_total"] = serde_json::json!(0);
        let bytes = serde_json::to_vec(&summary).unwrap();
        assert!(matches!(
            MeasurementReportInput::read_from(bytes.as_slice()).unwrap_err(),
            MeasurementReportReadError::InvalidJson { source }
                if source.to_string().contains("unknown field `unknown_prediction_total`")
        ));

        let provenance = prediction_test_provenance();
        let mut summary = validated_lint_wire(&provenance, vec![empty_check("test:reader")]);
        summary["summary"]["unknown_summary"] = serde_json::json!(0);
        let bytes = serde_json::to_vec(&summary).unwrap();
        assert!(matches!(
            MeasurementReportInput::read_from(bytes.as_slice()).unwrap_err(),
            MeasurementReportReadError::InvalidJson { source }
                if source.to_string().contains("unknown field `unknown_summary`")
        ));
    }

    #[test]
    fn staged_reader_preserves_typed_prediction_semantic_errors() {
        let provenance = prediction_test_provenance();
        let mut provenance_wire = validated_lint_wire(&provenance, Vec::new());
        provenance_wire["files"][0]["prediction_provenance"]["schema"] =
            serde_json::json!("urn:changed");
        assert!(matches!(
            reader_error(provenance_wire),
            MeasurementReportError::File {
                file_index: 0,
                source: MeasurementFileError::InvalidPredictionProvenance { .. },
            }
        ));

        let basis = EnginePredictionBasisV1::new(vec![
            PredictionBasisReferenceV1::project_field(
                "test:project",
                PredictionScalarV1::Boolean { value: true },
            )
            .unwrap(),
        ])
        .unwrap();
        let facet = EnginePredictionFacetV3::required_unavailable(
            EvaluationScope::new(EvaluationScopeCode::custom("test:prediction")),
            basis_v2(basis),
            vec![PredictionUnavailableReasonV2::ProjectIntentUnavailable],
        )
        .unwrap();
        let prediction_wire = validated_lint_wire(
            &provenance,
            vec![unavailable_check("test:reader", &provenance, vec![facet])],
        );

        let mut wrong_emitter = prediction_wire.clone();
        wrong_emitter["files"][0]["checks"][0]["prediction"]["facets"][0]["scope"]["code"] =
            serde_json::json!("member_existence");
        assert!(matches!(
            reader_error(wrong_emitter),
            MeasurementReportError::File {
                file_index: 0,
                source: MeasurementFileError::InvalidPredictionLifecycle {
                    check_index: 0,
                    reason: "prediction facet scope code is invalid for its parent check",
                },
            }
        ));

        let mut empty_scope = prediction_wire.clone();
        empty_scope["files"][0]["checks"][0]["prediction"]["facets"][0]["scope"]["code"] =
            serde_json::json!("");
        assert!(matches!(
            reader_error(empty_scope),
            MeasurementReportError::File {
                file_index: 0,
                source: MeasurementFileError::InvalidPrediction { check_index: 0, .. },
            }
        ));

        let mut prediction_wire = prediction_wire;
        prediction_wire["files"][0]["checks"][0]["prediction"]["facets"][0]["basis"]["identity"]
            ["bytes"] = serde_json::json!(0);
        assert!(matches!(
            reader_error(prediction_wire),
            MeasurementReportError::File {
                file_index: 0,
                source: MeasurementFileError::InvalidPrediction {
                    check_index: 0,
                    source: PredictionContractError::IdentityMismatch {
                        contract: "engine prediction basis v2",
                    },
                },
            }
        ));
    }

    #[test]
    fn staged_reader_uses_the_authoritative_check_lifecycle_without_prediction() {
        let provenance = prediction_test_provenance();
        let base = validated_lint_wire(&provenance, vec![empty_check("test:reader")]);

        for (field, state) in [
            ("selection", "unselected"),
            ("configuration", "disabled"),
            ("applicability", "not_applicable"),
        ] {
            let mut inactive = base.clone();
            inactive["files"][0]["checks"][0][field] = serde_json::json!(state);
            assert!(matches!(
                reader_error(inactive),
                MeasurementReportError::File {
                    file_index: 0,
                    source: MeasurementFileError::InvalidPredictionLifecycle {
                        check_index: 0,
                        reason: "evaluation does not match completed and missing prediction work",
                    },
                }
            ));
        }

        let mut inactive = base.clone();
        inactive["files"][0]["checks"][0]["selection"] = serde_json::json!("unselected");
        inactive["files"][0]["checks"][0]["evaluation"] = serde_json::json!("not_evaluated");
        serde_json::from_value::<MeasurementReportInput>(inactive)
            .unwrap()
            .into_files()
            .expect("empty inactive record is valid");

        let mut not_evaluated = base.clone();
        not_evaluated["files"][0]["checks"][0]["gaps"] = serde_json::json!([{
            "code": "test:missing",
            "message": "missing",
        }]);
        not_evaluated["files"][0]["checks"][0]["evaluation"] = serde_json::json!("not_evaluated");
        serde_json::from_value::<MeasurementReportInput>(not_evaluated.clone())
            .unwrap()
            .into_files()
            .expect("missing-only active record derives not_evaluated");
        not_evaluated["files"][0]["checks"][0]["evaluation"] = serde_json::json!("complete");
        assert!(matches!(
            reader_error(not_evaluated),
            MeasurementReportError::File {
                source: MeasurementFileError::InvalidPredictionLifecycle { check_index: 0, .. },
                ..
            }
        ));

        let mut partial = base.clone();
        partial["files"][0]["checks"][0]["gaps"] = serde_json::json!([{
            "code": "test:missing",
            "message": "missing",
        }]);
        partial["files"][0]["checks"][0]["evaluated_scopes"] =
            serde_json::json!([{ "code": "test:completed" }]);
        partial["files"][0]["checks"][0]["evaluation"] = serde_json::json!("partial");
        serde_json::from_value::<MeasurementReportInput>(partial.clone())
            .unwrap()
            .into_files()
            .expect("mixed active record derives partial");
        partial["files"][0]["checks"][0]["evaluation"] = serde_json::json!("complete");
        assert!(matches!(
            reader_error(partial),
            MeasurementReportError::File {
                source: MeasurementFileError::InvalidPredictionLifecycle { check_index: 0, .. },
                ..
            }
        ));

        let mut wrong_complete = base;
        wrong_complete["files"][0]["checks"][0]["evaluation"] = serde_json::json!("partial");
        assert!(matches!(
            reader_error(wrong_complete),
            MeasurementReportError::File {
                source: MeasurementFileError::InvalidPredictionLifecycle { check_index: 0, .. },
                ..
            }
        ));
    }

    #[test]
    fn staged_reader_rejects_invalid_scope_gap_and_finding_shapes() {
        let provenance = prediction_test_provenance();
        let base = validated_lint_wire(&provenance, vec![empty_check("test:reader")]);

        let mut empty_scope = base.clone();
        empty_scope["files"][0]["checks"][0]["evaluated_scopes"] =
            serde_json::json!([{ "code": "" }]);
        assert!(matches!(
            reader_error(empty_scope),
            MeasurementReportError::File {
                source: MeasurementFileError::InvalidPredictionLifecycle { check_index: 0, .. },
                ..
            }
        ));

        let mut malformed_gap = base.clone();
        malformed_gap["files"][0]["checks"][0]["gaps"] = serde_json::json!([{
            "code": "",
            "message": "missing",
        }]);
        malformed_gap["files"][0]["checks"][0]["evaluation"] = serde_json::json!("not_evaluated");
        assert!(matches!(
            reader_error(malformed_gap),
            MeasurementReportError::File {
                source: MeasurementFileError::InvalidPredictionLifecycle { check_index: 0, .. },
                ..
            }
        ));

        let mut incomplete_finding = base;
        incomplete_finding["files"][0]["checks"][0]["findings"] = serde_json::json!([{
            "check_id": "test:reader",
        }]);
        assert!(matches!(
            reader_error(incomplete_finding),
            MeasurementReportError::File {
                source: MeasurementFileError::InvalidPredictionShape { check_index: 0, .. },
                ..
            }
        ));
    }

    #[test]
    fn staged_reader_stops_at_the_first_files_lifecycle_failure() {
        let provenance = prediction_test_provenance();
        let mut wire = validated_lint_wire(&provenance, vec![empty_check("test:reader")]);
        let mut later_file = wire["files"][0].clone();
        later_file["prediction_provenance"]["schema"] = serde_json::json!("urn:changed");
        wire["files"].as_array_mut().unwrap().push(later_file);
        wire["summary"]["files"] = serde_json::json!(2);
        wire["files"][0]["checks"][0]["evaluation"] = serde_json::json!("partial");

        assert!(matches!(
            reader_error(wire),
            MeasurementReportError::File {
                file_index: 0,
                source: MeasurementFileError::InvalidPredictionLifecycle { check_index: 0, .. },
            }
        ));
    }

    #[test]
    fn staged_reader_stops_at_the_first_checks_lifecycle_failure() {
        let provenance = prediction_test_provenance();
        let basis = EnginePredictionBasisV1::new(vec![
            PredictionBasisReferenceV1::project_field(
                "test:project",
                PredictionScalarV1::Boolean { value: true },
            )
            .unwrap(),
        ])
        .unwrap();
        let facet = EnginePredictionFacetV3::required_unavailable(
            EvaluationScope::new(EvaluationScopeCode::custom("test:prediction")),
            basis_v2(basis),
            vec![PredictionUnavailableReasonV2::ProjectIntentUnavailable],
        )
        .unwrap();
        let mut wire = validated_lint_wire(
            &provenance,
            vec![
                empty_check("test:first"),
                unavailable_check("test:second", &provenance, vec![facet]),
            ],
        );
        wire["files"][0]["checks"][0]["evaluation"] = serde_json::json!("partial");
        wire["files"][0]["checks"][1]["unknown"] = serde_json::json!(true);

        assert!(matches!(
            reader_error(wire),
            MeasurementReportError::File {
                file_index: 0,
                source: MeasurementFileError::InvalidPredictionLifecycle { check_index: 0, .. },
            }
        ));
    }

    #[test]
    fn v11_nested_version_is_rejected_before_current_shape_decode() {
        let report: MeasurementReportInput = serde_json::from_value(serde_json::json!({
            "schema_version": OUTPUT_SCHEMA_VERSION,
            "schema": OUTPUT_SCHEMA_ID,
            "tool": {},
            "command": "measure",
            "files": [{
                "path": "measurements-v11.json",
                "input": { "sha256": "0".repeat(64), "bytes": 0 },
                "rig": {},
                "measurements": {
                    "schema_version": 11,
                    "schema": "urn:animsmith:schema:measurements:11",
                    "skeleton_nodes": [{
                        "node_index": 0,
                        "scene_root_indices": [],
                        "local_rest": {
                            "kind": "trs",
                            "translation_m": [0.0, 0.0, 0.0],
                            "rotation_xyzw": [0.0, 0.0, 0.0, 1.0],
                            "scale": [1.0, 1.0, 1.0]
                        },
                        "rest_world_matrix": [
                            1.0, 0.0, 0.0, 0.0,
                            0.0, 1.0, 0.0, 0.0,
                            0.0, 0.0, 1.0, 0.0,
                            0.0, 0.0, 0.0, 1.0
                        ]
                    }],
                    "skins": [{ "skin_index": 0 }]
                }
            }]
        }))
        .expect("unsupported payload shapes remain decodable for version rejection");

        assert!(matches!(
            report.into_files(),
            Err(MeasurementReportError::File {
                file_index: 0,
                source: MeasurementFileError::UnsupportedMeasurementVersion { found: 11 },
            })
        ));
    }

    #[test]
    fn current_v16_primitive_mutations_fail_closed_on_readback() {
        let wire = measure_wire(primitive_measurement_contract());
        let primitive = &wire["files"][0]["measurements"]["mesh_definitions"][0]["primitives"][0];
        assert_eq!(primitive["primitive_index"], serde_json::json!(1));
        assert_eq!(primitive["material_index"], serde_json::json!(7));
        assert_eq!(
            wire["files"][0]["measurements"]["mesh_definitions"][0]["primitives"][1]["material_index"],
            serde_json::Value::Null
        );

        let mut missing_material = wire.clone();
        missing_material["files"][0]["measurements"]["mesh_definitions"][0]["primitives"][1]
            .as_object_mut()
            .unwrap()
            .remove("material_index");
        assert!(matches!(
            reader_error(missing_material),
            MeasurementReportError::File {
                source: MeasurementFileError::InvalidMeasurementsShape { .. },
                ..
            }
        ));

        let mut mutations = Vec::new();
        let mut missing = wire.clone();
        missing["files"][0]["measurements"]["mesh_definitions"][0]
            .as_object_mut()
            .unwrap()
            .remove("primitives");
        mutations.push(missing);

        let mut duplicate_index = wire.clone();
        duplicate_index["files"][0]["measurements"]["mesh_definitions"][0]["primitives"][1]["primitive_index"] =
            serde_json::json!(1);
        mutations.push(duplicate_index);

        let mut decreasing_index = wire.clone();
        decreasing_index["files"][0]["measurements"]["mesh_definitions"][0]["primitives"][1]["primitive_index"] =
            serde_json::json!(0);
        mutations.push(decreasing_index);

        let mut finite_over = wire.clone();
        finite_over["files"][0]["measurements"]["mesh_definitions"][0]["primitives"][0]["finite_vertex_count"] =
            serde_json::json!(3);
        mutations.push(finite_over);

        let mut zero_with_facts = wire.clone();
        zero_with_facts["files"][0]["measurements"]["mesh_definitions"][0]["primitives"][0]["finite_vertex_count"] =
            serde_json::json!(0);
        mutations.push(zero_with_facts);

        let mut positive_without_centroid = wire.clone();
        positive_without_centroid["files"][0]["measurements"]["mesh_definitions"][0]["primitives"]
            [0]
        .as_object_mut()
        .unwrap()
        .remove("geometry_centroid");
        mutations.push(positive_without_centroid);

        let mut centroid_outside_aabb = wire.clone();
        centroid_outside_aabb["files"][0]["measurements"]["mesh_definitions"][0]["primitives"][0]
            ["geometry_centroid"] = serde_json::json!([-3.0, 1.0, 0.0]);
        mutations.push(centroid_outside_aabb);

        let mut missing_mesh_geometry = wire.clone();
        missing_mesh_geometry["files"][0]["measurements"]["mesh_definitions"][0]
            .as_object_mut()
            .unwrap()
            .remove("geometry_centroid");
        mutations.push(missing_mesh_geometry);

        let mut wrong_mesh_aabb = wire.clone();
        wrong_mesh_aabb["files"][0]["measurements"]["mesh_definitions"][0]["geometry_aabb"]["max"]
            [0] = serde_json::json!(7.0);
        mutations.push(wrong_mesh_aabb);

        let mut wrong_mesh_centroid = wire.clone();
        wrong_mesh_centroid["files"][0]["measurements"]["mesh_definitions"][0]["geometry_centroid"] =
            serde_json::json!([3.0, 7.0 / 3.0, 0.0]);
        mutations.push(wrong_mesh_centroid);

        let mut wrong_sum = wire.clone();
        wrong_sum["files"][0]["measurements"]["mesh_definitions"][0]["vertex_count"] =
            serde_json::json!(3);
        mutations.push(wrong_sum);

        let mut finite_sum_overflow = wire.clone();
        for primitive in
            finite_sum_overflow["files"][0]["measurements"]["mesh_definitions"][0]["primitives"]
                .as_array_mut()
                .unwrap()
        {
            primitive["vertex_count"] = serde_json::json!(u64::MAX);
            primitive["finite_vertex_count"] = serde_json::json!(u64::MAX);
        }
        finite_sum_overflow["files"][0]["measurements"]["mesh_definitions"][0]["vertex_count"] =
            serde_json::json!(u64::MAX);
        mutations.push(finite_sum_overflow);

        for mutation in mutations {
            assert!(matches!(
                reader_error(mutation),
                MeasurementReportError::File {
                    source: MeasurementFileError::InvalidMeasurements { .. },
                    ..
                }
            ));
        }

        let mut unknown_root = wire.clone();
        unknown_root["files"][0]["measurements"]
            .as_object_mut()
            .unwrap()
            .insert("bogus".into(), serde_json::json!(true));
        let mut unknown_mesh = wire.clone();
        unknown_mesh["files"][0]["measurements"]["mesh_definitions"][0]
            .as_object_mut()
            .unwrap()
            .insert("bogus".into(), serde_json::json!(true));
        let mut unknown_primitive = wire.clone();
        unknown_primitive["files"][0]["measurements"]["mesh_definitions"][0]["primitives"][0]
            .as_object_mut()
            .unwrap()
            .insert("bogus".into(), serde_json::json!(true));
        let mut unknown_aabb = wire;
        unknown_aabb["files"][0]["measurements"]["mesh_definitions"][0]["primitives"][0]
            ["geometry_aabb"]
            .as_object_mut()
            .unwrap()
            .insert("bogus".into(), serde_json::json!(true));
        for mutation in [unknown_root, unknown_mesh, unknown_primitive, unknown_aabb] {
            assert!(matches!(
                reader_error(mutation),
                MeasurementReportError::File {
                    source: MeasurementFileError::InvalidMeasurementsShape { reason },
                    ..
                } if reason.contains("unknown field `bogus`")
            ));
        }
    }

    #[test]
    fn current_v16_material_indices_follow_resource_coverage() {
        let unavailable = measure_wire(primitive_measurement_contract());
        serde_json::from_value::<MeasurementReportInput>(unavailable.clone())
            .unwrap()
            .into_files()
            .expect("unavailable inventory may retain a source material index");

        let mut complete_out_of_range = unavailable.clone();
        complete_out_of_range["files"][0]["measurements"]["material_resource_coverage"] =
            serde_json::json!("complete");
        assert!(matches!(
            reader_error(complete_out_of_range),
            MeasurementReportError::File {
                source: MeasurementFileError::InvalidMeasurements { .. },
                ..
            }
        ));

        let mut complete_in_range = unavailable;
        complete_in_range["files"][0]["measurements"]["material_resource_coverage"] =
            serde_json::json!("complete");
        complete_in_range["files"][0]["measurements"]["material_definitions"] = serde_json::json!([{
            "material_index": 0,
            "name": null,
            "texture_bindings": []
        }]);
        complete_in_range["files"][0]["measurements"]["mesh_definitions"][0]["primitives"][0]["material_index"] =
            serde_json::json!(0);
        serde_json::from_value::<MeasurementReportInput>(complete_in_range)
            .unwrap()
            .into_files()
            .expect("complete inventory accepts an in-range primitive material index");
    }

    #[test]
    fn current_v16_leading_magic_is_bounded_reason_specific_hex() {
        let mut assets = AssetMeasurements {
            material_resource_coverage: MaterialResourceCoverage::Complete,
            ..AssetMeasurements::default()
        };
        assets.images.push(ImageMeasurements {
            image_index: 0,
            name: None,
            source_kind: ImageSourceKind::Embedded,
            declared_mime_type: None,
            detected_container: None,
            leading_magic_hex: Some("00ff10".into()),
            width: None,
            height: None,
            channel_count: None,
            decoded_color_type: None,
            unavailable_reason: Some(ImageUnavailableReason::UnsupportedContainer),
        });
        let wire = measure_wire(MeasurementContract::new(BTreeMap::new(), assets).unwrap());

        let mut unknown_image = wire.clone();
        unknown_image["files"][0]["measurements"]["images"][0]
            .as_object_mut()
            .unwrap()
            .insert("bogus".into(), serde_json::json!(true));
        assert!(matches!(
            reader_error(unknown_image),
            MeasurementReportError::File {
                source: MeasurementFileError::InvalidMeasurementsShape { reason },
                ..
            } if reason.contains("unknown field `bogus`")
        ));

        for magic in ["", "0", "0F", "00ff00112233445566778899aabbccdde"] {
            let mut mutation = wire.clone();
            mutation["files"][0]["measurements"]["images"][0]["leading_magic_hex"] =
                serde_json::json!(magic);
            assert!(matches!(
                reader_error(mutation),
                MeasurementReportError::File {
                    source: MeasurementFileError::InvalidMeasurements { .. },
                    ..
                }
            ));
        }

        let mut wrong_reason = wire;
        wrong_reason["files"][0]["measurements"]["images"][0]["unavailable_reason"] =
            serde_json::json!("resource_limit");
        assert!(matches!(
            reader_error(wrong_reason),
            MeasurementReportError::File {
                source: MeasurementFileError::InvalidMeasurements { .. },
                ..
            }
        ));
    }

    #[test]
    fn v12_v15_round_trips_without_inventing_primitive_rows() {
        let current_provenance = prediction_test_provenance_v2();
        let historical_provenance = current_provenance.clone().historical_v15_for_test();
        let basis =
            EnginePredictionBasisV1::new_v16(vec![PredictionBasisReferenceV1::measurement_v16(
                crate::MeasurementPointerV1::new("/measurements/schema_version").unwrap(),
                PredictionScalarV1::UnsignedInteger { value: 15 },
            )])
            .unwrap();
        let facet = EnginePredictionFacetV2::required_unavailable(
            EvaluationScope::new(EvaluationScopeCode::custom("test:v12")),
            basis,
            vec![PredictionUnavailableReasonV2::MeasurementUnavailable],
        )
        .unwrap();
        let current_check =
            unavailable_check_v2("test:v12", &current_provenance, vec![facet.clone()]);
        let prediction =
            EnginePredictionV2::new(current_provenance.identity().clone(), vec![facet])
                .unwrap()
                .historical_v15_for_test(historical_provenance.identity().clone());
        let mut check_wire = serde_json::to_value(current_check).unwrap();
        check_wire["prediction"] = serde_json::to_value(prediction).unwrap();
        let mut historical_assets = AssetMeasurements::default();
        historical_assets
            .mesh_definitions
            .push(MeshDefinitionMeasurements {
                mesh_index: 0,
                name: "legacy-mesh".into(),
                primitives: None,
                vertex_count: 0,
                geometry_aabb: None,
                geometry_centroid: None,
                max_joints_per_vertex: 0,
                weight_sum_min: None,
                weight_sum_max: None,
                additional_influence_sets: Vec::new(),
            });
        let historical_measurements =
            MeasurementContract::historical_v15(BTreeMap::new(), historical_assets).unwrap();
        let wire = serde_json::json!({
            "schema_version": OUTPUT_V12_SCHEMA_VERSION,
            "schema": OUTPUT_V12_SCHEMA_ID,
            "tool": {},
            "command": "lint",
            "summary": {"prediction_facets": {
                "available": 0,
                "required_prediction_unavailable": 1
            }},
            "files": [{
                "path": "historical-v12.glb",
                "input": {"sha256": "00".repeat(32), "bytes": 0},
                "rig": serde_json::to_value(prediction_test_rig()).unwrap(),
                "measurements": serde_json::to_value(historical_measurements).unwrap(),
                "prediction_provenance": serde_json::to_value(historical_provenance).unwrap(),
                "checks": [check_wire]
            }]
        });

        let mut historical_with_unknowns = wire.clone();
        historical_with_unknowns["files"][0]["measurements"]
            .as_object_mut()
            .unwrap()
            .insert("future_root_field".into(), serde_json::json!(true));
        historical_with_unknowns["files"][0]["measurements"]["mesh_definitions"][0]
            .as_object_mut()
            .unwrap()
            .insert("future_mesh_field".into(), serde_json::json!(true));
        serde_json::from_value::<MeasurementReportInput>(historical_with_unknowns)
            .unwrap()
            .into_files()
            .expect("historical measurements-v15 retains permissive unknown-field readback");

        let mut smuggled_primitives = wire.clone();
        smuggled_primitives["files"][0]["measurements"]["mesh_definitions"][0]["primitives"] =
            serde_json::json!([]);
        assert!(matches!(
            serde_json::from_value::<MeasurementReportInput>(smuggled_primitives)
                .unwrap()
                .into_files(),
            Err(MeasurementReportError::File {
                source: MeasurementFileError::InvalidMeasurements { source },
                ..
            }) if source.to_string().contains("measurements-v15 cannot carry per-primitive evidence")
        ));

        let mut smuggled_magic = wire.clone();
        smuggled_magic["files"][0]["measurements"]["material_resource_coverage"] =
            serde_json::json!("complete");
        smuggled_magic["files"][0]["measurements"]["images"] = serde_json::json!([{
            "image_index": 0,
            "name": null,
            "source_kind": "embedded",
            "declared_mime_type": null,
            "detected_container": null,
            "leading_magic_hex": "00",
            "width": null,
            "height": null,
            "channel_count": null,
            "decoded_color_type": null,
            "unavailable_reason": "unsupported_container"
        }]);
        assert!(matches!(
            serde_json::from_value::<MeasurementReportInput>(smuggled_magic)
                .unwrap()
                .into_files(),
            Err(MeasurementReportError::File {
                source: MeasurementFileError::InvalidMeasurements { source },
                ..
            }) if source.to_string().contains("measurements-v15 cannot carry leading-magic evidence")
        ));

        let files = serde_json::from_value::<MeasurementReportInput>(wire.clone())
            .unwrap()
            .into_files()
            .unwrap();
        let readback = serde_json::to_value(files[0].measurements()).unwrap();
        assert_eq!(readback["schema_version"], serde_json::json!(15));
        assert_eq!(
            readback["schema"],
            serde_json::json!(MEASUREMENTS_V15_SCHEMA_ID)
        );
        assert_eq!(readback["mesh_definitions"].as_array().unwrap().len(), 1);
        assert!(readback["mesh_definitions"][0].get("primitives").is_none());

        let mut historical_max = wire.clone();
        historical_max["files"][0]["measurements"]["mesh_definitions"][0]["vertex_count"] =
            serde_json::json!(u32::MAX);
        serde_json::from_value::<MeasurementReportInput>(historical_max)
            .unwrap()
            .into_files()
            .expect("measurements-v15 retains its historical inclusive u32 maximum");

        let mut historical_overflow = wire.clone();
        historical_overflow["files"][0]["measurements"]["mesh_definitions"][0]["vertex_count"] =
            serde_json::json!(u64::from(u32::MAX) + 1);
        assert!(matches!(
            lint_read_error(historical_overflow),
            MeasurementReportError::File {
                source: MeasurementFileError::InvalidMeasurements { .. },
                ..
            }
        ));

        let mut v16_basis = wire;
        v16_basis["files"][0]["checks"][0]["prediction"]["facets"][0]["basis"]["references"][0]["schema"] =
            serde_json::json!(MEASUREMENTS_SCHEMA_ID);
        assert!(matches!(
            lint_read_error(v16_basis),
            MeasurementReportError::File {
                source: MeasurementFileError::InvalidPrediction {
                    source: PredictionContractError::InvalidSchema {
                        field: "basis.measurement.schema",
                        expected: MEASUREMENTS_V15_SCHEMA_ID,
                        ..
                    },
                    ..
                },
                ..
            }
        ));
    }

    #[test]
    fn adjacent_output_revisions_reject_each_others_nested_measurements() {
        let current = measure_wire(prediction_test_measurements());
        let mut v12_with_v17 = current.clone();
        v12_with_v17["schema_version"] = serde_json::json!(OUTPUT_V12_SCHEMA_VERSION);
        v12_with_v17["schema"] = serde_json::json!(OUTPUT_V12_SCHEMA_ID);
        assert!(matches!(
            reader_error(v12_with_v17),
            MeasurementReportError::File {
                source: MeasurementFileError::UnsupportedMeasurementVersion { found: 17 },
                ..
            }
        ));

        let historical =
            MeasurementContract::historical_v15(BTreeMap::new(), AssetMeasurements::default())
                .unwrap();
        let mut v13_with_v15 = current;
        v13_with_v15["files"][0]["measurements"] = serde_json::to_value(historical).unwrap();
        assert!(matches!(
            reader_error(v13_with_v15),
            MeasurementReportError::File {
                source: MeasurementFileError::UnsupportedMeasurementVersion { found: 15 },
                ..
            }
        ));
    }

    #[test]
    fn historical_output_v17_writer_projects_current_measurements_to_v16() {
        let report = LintFileReportV17::new(
            "historical.glb",
            InputIdentity::from_bytes(&[]),
            prediction_test_rig(),
            None,
            Vec::new(),
            prediction_test_measurements(),
        )
        .unwrap();
        assert_eq!(
            report.measurements().schema_version,
            MEASUREMENTS_V16_SCHEMA_VERSION
        );
        assert_eq!(report.measurements().schema, MEASUREMENTS_V16_SCHEMA_ID);
    }

    #[test]
    fn current_v18_writers_refuse_measurements_v16_at_file_and_envelope_boundaries() {
        let mismatch = OutputContractError::CurrentMeasurementContractMismatch {
            output_version: OUTPUT_SCHEMA_VERSION,
            found_version: MEASUREMENTS_V16_SCHEMA_VERSION,
            found_schema: MEASUREMENTS_V16_SCHEMA_ID.into(),
        };
        assert_eq!(
            MeasureFileReport::new(
                "historical.glb",
                InputIdentity::from_bytes(&[]),
                prediction_test_rig(),
                prediction_test_measurements_v16(),
            )
            .unwrap_err(),
            mismatch
        );
        assert_eq!(
            LintFileReportV18::new(
                "historical.glb",
                InputIdentity::from_bytes(&[]),
                prediction_test_rig(),
                None,
                Vec::new(),
                prediction_test_measurements_v16(),
            )
            .unwrap_err(),
            mismatch
        );

        let malformed_measure = MeasureFileReport {
            evidence: FileEvidence::new(
                "historical.glb",
                InputIdentity::from_bytes(&[]),
                prediction_test_rig(),
                prediction_test_measurements_v16(),
            ),
        };
        assert_eq!(
            MeasureEnvelope::new(
                ToolInfo::animsmith(ToolSource::new(None, None)),
                vec![malformed_measure],
            )
            .unwrap_err(),
            mismatch
        );
        let malformed_lint = LintFileReportV18 {
            evidence: FileEvidence::new(
                "historical.glb",
                InputIdentity::from_bytes(&[]),
                prediction_test_rig(),
                prediction_test_measurements_v16(),
            ),
            prediction_provenance: None,
            checks: Vec::new(),
        };
        assert_eq!(
            LintEnvelopeV18::new(
                ToolInfo::animsmith(ToolSource::new(None, None)),
                vec![malformed_lint],
            )
            .unwrap_err(),
            mismatch
        );
    }

    #[test]
    fn v17_to_v16_projection_is_total_and_used_by_prediction_and_historical_writers() {
        let fully_measured = loop_projection_measurements(&[
            MeasurementAvailability::Measured,
            MeasurementAvailability::Measured,
        ]);
        let projected = fully_measured.prediction_v16_projection().unwrap();
        assert_eq!(projected.schema_version, MEASUREMENTS_V16_SCHEMA_VERSION);
        let continuity = projected.clips()["loop"]
            .loop_continuity
            .as_ref()
            .expect("fully measured rows survive projection");
        assert_eq!(continuity.bones.len(), 2);
        let wire = serde_json::to_value(&projected).unwrap();
        assert!(
            wire["clips"]["loop"]["loop_continuity"]["bones"]
                .as_array()
                .unwrap()
                .iter()
                .all(|bone| bone.get("availability").is_none())
        );

        let partial = loop_projection_measurements(&[
            MeasurementAvailability::Measured,
            MeasurementAvailability::Unavailable,
        ]);
        let all_unavailable = loop_projection_measurements(&[
            MeasurementAvailability::Unavailable,
            MeasurementAvailability::Unavailable,
        ]);
        for measurements in [&partial, &all_unavailable] {
            let projected = measurements.prediction_v16_projection().unwrap();
            assert_eq!(
                projected.clips()["loop"].loop_continuity_availability,
                MeasurementAvailability::Unavailable
            );
            assert!(projected.clips()["loop"].loop_continuity.is_none());
        }

        let current_v3 = LintFileReportV18::new(
            "v3.glb",
            InputIdentity::from_bytes(&[]),
            prediction_test_rig(),
            None,
            Vec::new(),
            fully_measured,
        )
        .unwrap();
        let current_v5 = LintFileReportV18::new_v5(
            "v5.glb",
            InputIdentity::from_bytes(&[]),
            prediction_test_rig(),
            None,
            Vec::new(),
            partial.clone(),
        )
        .unwrap();
        let current_v6 = LintFileReportV18::new_v6(
            "v6.glb",
            InputIdentity::from_bytes(&[]),
            prediction_test_rig(),
            None,
            Vec::new(),
            all_unavailable.clone(),
        )
        .unwrap();
        assert!(
            current_v3.measurements().clips()["loop"]
                .loop_continuity
                .is_some()
        );
        assert!(
            current_v5.measurements().clips()["loop"]
                .loop_continuity
                .is_some()
        );
        assert!(
            current_v6.measurements().clips()["loop"]
                .loop_continuity
                .is_some()
        );

        let historical_v15 = LintFileReport::new(
            "v15.glb",
            InputIdentity::from_bytes(&[]),
            prediction_test_rig(),
            None,
            Vec::new(),
            partial.clone(),
        )
        .unwrap();
        let historical_v16 = LintFileReportV16::new(
            "v16.glb",
            InputIdentity::from_bytes(&[]),
            prediction_test_rig(),
            None,
            Vec::new(),
            partial.clone(),
        )
        .unwrap();
        let historical_v17 = LintFileReportV17::new(
            "v17.glb",
            InputIdentity::from_bytes(&[]),
            prediction_test_rig(),
            None,
            Vec::new(),
            partial,
        )
        .unwrap();
        for measurements in [
            historical_v15.measurements(),
            historical_v16.measurements(),
            historical_v17.measurements(),
        ] {
            assert_eq!(measurements.schema_version, MEASUREMENTS_V16_SCHEMA_VERSION);
            assert_eq!(
                measurements.clips()["loop"].loop_continuity_availability,
                MeasurementAvailability::Unavailable
            );
            assert!(measurements.clips()["loop"].loop_continuity.is_none());
        }
    }

    #[test]
    fn legacy_output_v11_reader_dispatch_preserves_measurement_recovery() {
        let provenance = prediction_test_provenance_v2();
        let legacy = PredictionProvenanceV1::new(
            provenance.profile().clone(),
            provenance.source_format(),
            ResolvedEngineSettingsV1::new(provenance.profile(), Vec::new(), Vec::new()).unwrap(),
            provenance.raw_source().clone(),
            provenance.dependency_closure().clone(),
        )
        .unwrap()
        .historical_v15_for_test();
        let legacy_prediction = EnginePredictionV1::new(
            legacy.identity().clone(),
            vec![
                EnginePredictionFacetV1::required_unavailable(
                    EvaluationScope::new(EvaluationScopeCode::custom("test:legacy-v11")),
                    EnginePredictionBasisV1::new(Vec::new()).unwrap(),
                    vec![PredictionUnavailableReasonV1::ProjectIntentUnavailable],
                )
                .unwrap(),
            ],
        )
        .unwrap()
        .historical_v15_for_test(legacy.identity().clone());
        let legacy_measurements =
            MeasurementContract::historical_v15(BTreeMap::new(), AssetMeasurements::default())
                .unwrap();
        // This is an actual V11/V1 wire shape, deliberately constructed
        // without producing a V12 envelope and swapping only its header.
        let wire = serde_json::json!({
            "schema_version": OUTPUT_V11_SCHEMA_VERSION,
            "schema": OUTPUT_V11_SCHEMA_ID,
            "tool": {},
            "command": "lint",
            "summary": {
                "prediction_facets": {
                    "available": 0,
                    "required_prediction_unavailable": 1,
                },
            },
            "files": [{
                "path": "legacy-v11.glb",
                "input": { "sha256": "00".repeat(32), "bytes": 0 },
                "rig": serde_json::to_value(prediction_test_rig()).unwrap(),
                "measurements": serde_json::to_value(legacy_measurements).unwrap(),
                "prediction_provenance": serde_json::to_value(legacy.clone()).unwrap(),
                "checks": [{
                    "check_id": "test:legacy-v11",
                    "selection": "selected",
                    "configuration": "enabled",
                    "applicability": "applicable",
                    "evaluation": "not_evaluated",
                    "findings": [],
                    "evaluated_scopes": [],
                    "gaps": [],
                    "prediction": legacy_prediction,
                }],
            }],
        });
        let report: MeasurementReportInput = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(report.file_count(), Some(1));
        assert_eq!(report.into_files().unwrap().len(), 1);

        let mut bad_provenance = wire.clone();
        bad_provenance["files"][0]["prediction_provenance"]["schema"] =
            serde_json::json!("urn:forged");
        assert!(matches!(
            lint_read_error(bad_provenance),
            MeasurementReportError::File {
                source: MeasurementFileError::InvalidPredictionProvenance { .. },
                ..
            }
        ));

        let mut bad_prediction = wire.clone();
        bad_prediction["files"][0]["checks"] = serde_json::json!([{
            "check_id": "test:legacy-v11",
            "selection": "selected",
            "configuration": "enabled",
            "applicability": "applicable",
            "evaluation": "not_evaluated",
            "findings": [],
            "evaluated_scopes": [],
            "gaps": [],
            "prediction": { "schema": "urn:forged" }
        }]);
        assert!(matches!(
            lint_read_error(bad_prediction),
            MeasurementReportError::File {
                source: MeasurementFileError::InvalidPredictionShape { .. },
                ..
            }
        ));

        let mut missing_provenance_before_malformed_prediction = wire.clone();
        missing_provenance_before_malformed_prediction["files"][0]["prediction_provenance"] =
            serde_json::Value::Null;
        missing_provenance_before_malformed_prediction["files"][0]["checks"][0]["prediction"] =
            serde_json::json!({ "schema": "urn:forged" });
        assert!(matches!(
            lint_read_error(missing_provenance_before_malformed_prediction),
            MeasurementReportError::File {
                source: MeasurementFileError::PredictionWithoutProvenance { check_index: 0 },
                ..
            }
        ));

        let mut inactive_before_malformed_prediction = wire.clone();
        inactive_before_malformed_prediction["files"][0]["checks"][0]["selection"] =
            serde_json::json!("unselected");
        inactive_before_malformed_prediction["files"][0]["checks"][0]["prediction"] =
            serde_json::json!({ "schema": "urn:forged" });
        assert!(matches!(
            lint_read_error(inactive_before_malformed_prediction),
            MeasurementReportError::File {
                source: MeasurementFileError::InvalidPredictionLifecycle { check_index: 0, .. },
                ..
            }
        ));

        // A V11 aggregate overflow is terminal at the check that creates it;
        // a malformed later check must never change that first error.
        let mut overbudget_before_later_malformed = wire.clone();
        let facet =
            overbudget_before_later_malformed["files"][0]["checks"][0]["prediction"]["facets"][0]
                .clone();
        overbudget_before_later_malformed["files"][0]["checks"][0]["prediction"]["facets"] =
            serde_json::Value::Array(
                std::iter::repeat_n(facet, PREDICTION_V1_MAX_FACETS_PER_FILE + 1).collect(),
            );
        overbudget_before_later_malformed["files"][0]["checks"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({ "check_id": 7 }));
        let precedence_error = lint_read_error(overbudget_before_later_malformed);
        assert!(
            matches!(
                precedence_error,
                MeasurementReportError::File {
                    source: MeasurementFileError::InvalidPrediction {
                        source: PredictionContractError::TooManyFacets { .. },
                        ..
                    },
                    ..
                },
            ),
            "{precedence_error:?}"
        );
    }
}

#[derive(Debug, Clone, Serialize)]
struct FileEvidence {
    path: String,
    input: InputIdentity,
    rig: RigInfo,
    measurements: MeasurementContract,
}

impl FileEvidence {
    fn new(
        path: impl Into<String>,
        input: InputIdentity,
        rig: RigInfo,
        measurements: MeasurementContract,
    ) -> Self {
        Self {
            path: path.into(),
            input,
            rig,
            measurements,
        }
    }

    fn historical_v16(
        path: impl Into<String>,
        input: InputIdentity,
        rig: RigInfo,
        measurements: MeasurementContract,
    ) -> Result<Self, OutputContractError> {
        Ok(Self::new(
            path,
            input,
            rig,
            measurements.prediction_v16_projection()?,
        ))
    }
}

/// One source file and its measurement-command evidence.
#[derive(Debug, Clone, Serialize)]
pub struct MeasureFileReport {
    #[serde(flatten)]
    evidence: FileEvidence,
}

impl MeasureFileReport {
    /// Construct a measurement-command file report.
    ///
    /// # Errors
    ///
    /// Returns [`OutputContractError::CurrentMeasurementContractMismatch`] if
    /// the nested evidence is not the exact current measurements-v17 contract.
    pub fn new(
        path: impl Into<String>,
        input: InputIdentity,
        rig: RigInfo,
        measurements: MeasurementContract,
    ) -> Result<Self, OutputContractError> {
        require_measurements_v17(OUTPUT_SCHEMA_VERSION, &measurements)?;
        Ok(Self {
            evidence: FileEvidence::new(path, input, rig, measurements),
        })
    }

    /// Display path supplied by the producer.
    pub fn path(&self) -> &str {
        &self.evidence.path
    }

    /// Immutable identity of the source bytes used to produce this record.
    pub fn input(&self) -> &InputIdentity {
        &self.evidence.input
    }

    /// Nested measurement evidence.
    pub fn measurements(&self) -> &MeasurementContract {
        &self.evidence.measurements
    }
}

/// A producer attempted to construct output outside the immutable v11 contract.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum OutputContractError {
    /// One envelope carried too many file records.
    #[error("output contains {found} files, exceeding the v10 limit of {limit}")]
    TooManyFiles {
        /// Supplied file count.
        found: usize,
        /// Immutable v10 limit.
        limit: usize,
    },
    /// One lint file carried too many check records.
    #[error("lint file contains {found} checks, exceeding the v10 limit of {limit}")]
    TooManyChecks {
        /// Supplied check count.
        found: usize,
        /// Immutable v10 limit.
        limit: usize,
    },
    /// A check carried prediction evidence without its file-scoped authority.
    #[error("engine prediction requires non-null file prediction_provenance")]
    PredictionWithoutProvenance,
    /// A current output-v15 lint record attempted to attach historical prediction evidence.
    #[error("output-v15 lint cannot carry historical engine-prediction evidence")]
    HistoricalPredictionInV2Output,
    /// File provenance and check prediction used different contract revisions.
    #[error(
        "output-v15 prediction provenance and check attachments must use one correlated revision"
    )]
    PredictionRevisionMismatch,
    /// File and provenance primary-input identities differed.
    #[error("prediction provenance primary input does not match the lint file input")]
    PredictionPrimaryInputMismatch,
    /// Aggregate prediction facets exceeded the V1 file limit.
    #[error("lint file contains {found} prediction facets, exceeding the V1 limit of {limit}")]
    TooManyPredictionFacets {
        /// Supplied facet count.
        found: usize,
        /// Immutable V1 limit.
        limit: usize,
    },
    /// A V2 rule emitted a budget summary although the file did not consume
    /// every one of its shared prediction-facet slots.
    #[error("facet-budget summary requires exactly {limit} aggregate facets, found {found}")]
    FacetBudgetSummaryWithoutExhaustedFileBudget {
        /// Aggregate facet count.
        found: usize,
        /// Immutable shared file limit.
        limit: usize,
    },
    /// Aggregate basis references exceeded the V1 file limit.
    #[error(
        "lint file contains {found} prediction basis references, exceeding the V1 limit of {limit}"
    )]
    TooManyPredictionBasisReferences {
        /// Supplied reference count.
        found: usize,
        /// Immutable V1 limit.
        limit: usize,
    },
    /// Aggregate retained provenance/prediction text exceeded the V1 limit.
    #[error("lint file retains {found} prediction text bytes, exceeding the V1 limit of {limit}")]
    TooMuchPredictionText {
        /// Supplied UTF-8 byte count.
        found: usize,
        /// Immutable V1 limit.
        limit: usize,
    },
    /// Checked aggregate accounting overflowed.
    #[error("checked arithmetic overflow while validating output-v11 bounds")]
    ArithmeticOverflow,
    /// Nested prediction evidence violated its contract.
    #[error("invalid prediction evidence: {0}")]
    InvalidPrediction(#[from] PredictionContractError),
    /// Current measurements could not be projected into the immutable V16
    /// evidence view consumed by existing prediction contracts.
    #[error("invalid measurements-v16 prediction projection: {0}")]
    InvalidMeasurementProjection(#[from] MeasurementContractError),
    /// A historical output writer was paired with a different nested
    /// measurement contract.
    #[error(
        "output-v{output_version} requires measurements-v16, found version {found_version} with identity {found_schema}"
    )]
    HistoricalMeasurementContractMismatch {
        /// Historical outer output revision.
        output_version: u32,
        /// Supplied nested measurement revision.
        found_version: u32,
        /// Supplied nested measurement identity.
        found_schema: String,
    },
    /// A current output writer received a non-current nested measurement contract.
    #[error(
        "output-v{output_version} requires measurements-v17, found measurement schema_version {found_version} ({found_schema})"
    )]
    CurrentMeasurementContractMismatch {
        /// Current outer output revision.
        output_version: u32,
        /// Supplied nested measurement revision.
        found_version: u32,
        /// Supplied nested measurement identity.
        found_schema: String,
    },
}

fn require_measurements_v17(
    output_version: u32,
    measurements: &MeasurementContract,
) -> Result<(), OutputContractError> {
    if measurements.schema_version == MEASUREMENTS_SCHEMA_VERSION
        && measurements.schema == MEASUREMENTS_SCHEMA_ID
    {
        Ok(())
    } else {
        Err(OutputContractError::CurrentMeasurementContractMismatch {
            output_version,
            found_version: measurements.schema_version,
            found_schema: measurements.schema.to_owned(),
        })
    }
}

fn require_measurements_v16(
    output_version: u32,
    measurements: &MeasurementContract,
) -> Result<(), OutputContractError> {
    if measurements.schema_version == MEASUREMENTS_V16_SCHEMA_VERSION
        && measurements.schema == MEASUREMENTS_V16_SCHEMA_ID
    {
        Ok(())
    } else {
        Err(OutputContractError::HistoricalMeasurementContractMismatch {
            output_version,
            found_version: measurements.schema_version,
            found_schema: measurements.schema.to_owned(),
        })
    }
}

#[derive(Debug, Clone, Serialize)]
struct EnvelopeHeader {
    schema_version: u32,
    schema: &'static str,
    tool: ToolInfo,
    command: &'static str,
}

impl EnvelopeHeader {
    fn new(tool: ToolInfo, command: &'static str) -> Self {
        Self {
            schema_version: OUTPUT_SCHEMA_VERSION,
            schema: OUTPUT_SCHEMA_ID,
            tool,
            command,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct MeasureSummary {
    files: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
struct FindingSummary {
    error: usize,
    warning: usize,
    note: usize,
}

impl FindingSummary {
    fn add(&mut self, severity: Severity) {
        match severity {
            Severity::Error => self.error += 1,
            Severity::Warning => self.warning += 1,
            Severity::Note => self.note += 1,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
struct SelectionSummary {
    selected: usize,
    unselected: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
struct ConfigurationSummary {
    enabled: usize,
    disabled: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
struct ApplicabilitySummary {
    applicable: usize,
    not_applicable: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
struct EvaluationStateSummary {
    complete: usize,
    partial: usize,
    not_evaluated: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
struct CheckSummary {
    total: usize,
    selection: SelectionSummary,
    configuration: ConfigurationSummary,
    applicability: ApplicabilitySummary,
    evaluation: EvaluationStateSummary,
    gaps: usize,
}

#[derive(Debug, Clone, Serialize)]
struct LintSummary {
    files: usize,
    findings: FindingSummary,
    checks: CheckSummary,
    prediction_facets: PredictionFacetSummary,
}

#[derive(Debug, Clone, Default, Serialize)]
struct PredictionFacetSummary {
    available: usize,
    required_prediction_unavailable: usize,
}

/// Current measure-command result envelope.
#[derive(Debug, Clone, Serialize)]
pub struct MeasureEnvelope {
    #[serde(flatten)]
    header: EnvelopeHeader,
    summary: MeasureSummary,
    files: Vec<MeasureFileReport>,
}

impl MeasureEnvelope {
    /// Construct a schema-valid measurement envelope.
    pub fn new(tool: ToolInfo, files: Vec<MeasureFileReport>) -> Result<Self, OutputContractError> {
        if files.len() > OUTPUT_V11_MAX_FILES {
            return Err(OutputContractError::TooManyFiles {
                found: files.len(),
                limit: OUTPUT_V11_MAX_FILES,
            });
        }
        for file in &files {
            require_measurements_v17(OUTPUT_SCHEMA_VERSION, &file.evidence.measurements)?;
        }
        Ok(Self {
            header: EnvelopeHeader::new(tool, "measure"),
            summary: MeasureSummary { files: files.len() },
            files,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
#[allow(
    clippy::large_enum_variant,
    reason = "the internal correlated revision enum preserves value ownership for both immutable wire types"
)]
enum CurrentPredictionProvenance {
    V3(PredictionProvenanceV3),
    V4(PredictionProvenanceV4),
}

/// Current output-v15 lint file evidence with correlated V3 or V4 prediction provenance.
#[derive(Debug, Clone, Serialize)]
pub struct LintFileReport {
    #[serde(flatten)]
    evidence: FileEvidence,
    prediction_provenance: Option<CurrentPredictionProvenance>,
    checks: Vec<CheckEvaluation>,
}

impl LintFileReport {
    /// Construct a legacy-V3 lint file report inside output-v15.
    pub fn new(
        path: impl Into<String>,
        input: InputIdentity,
        rig: RigInfo,
        prediction_provenance: Option<PredictionProvenanceV3>,
        checks: Vec<CheckEvaluation>,
        measurements: MeasurementContract,
    ) -> Result<Self, OutputContractError> {
        if checks.len() > OUTPUT_V11_MAX_CHECKS_PER_FILE {
            return Err(OutputContractError::TooManyChecks {
                found: checks.len(),
                limit: OUTPUT_V11_MAX_CHECKS_PER_FILE,
            });
        }
        let report = Self {
            evidence: FileEvidence::historical_v16(path, input, rig, measurements)?,
            prediction_provenance: prediction_provenance.map(CurrentPredictionProvenance::V3),
            checks,
        };
        report.validate()?;
        Ok(report)
    }

    /// V3 prediction provenance, or `None` for engine-neutral lint.
    pub const fn prediction_provenance(&self) -> Option<&PredictionProvenanceV3> {
        match self.prediction_provenance.as_ref() {
            Some(CurrentPredictionProvenance::V3(provenance)) => Some(provenance),
            Some(CurrentPredictionProvenance::V4(_)) | None => None,
        }
    }

    /// Construct a result-bearing V4 lint file report.
    pub fn new_v4(
        path: impl Into<String>,
        input: InputIdentity,
        rig: RigInfo,
        prediction_provenance: Option<PredictionProvenanceV4>,
        checks: Vec<CheckEvaluation>,
        measurements: MeasurementContract,
    ) -> Result<Self, OutputContractError> {
        if checks.len() > OUTPUT_V11_MAX_CHECKS_PER_FILE {
            return Err(OutputContractError::TooManyChecks {
                found: checks.len(),
                limit: OUTPUT_V11_MAX_CHECKS_PER_FILE,
            });
        }
        let report = Self {
            evidence: FileEvidence::historical_v16(path, input, rig, measurements)?,
            prediction_provenance: prediction_provenance.map(CurrentPredictionProvenance::V4),
            checks,
        };
        report.validate()?;
        Ok(report)
    }

    /// V4 prediction provenance, when this file uses revision 4.
    pub const fn prediction_provenance_v4(&self) -> Option<&PredictionProvenanceV4> {
        match self.prediction_provenance.as_ref() {
            Some(CurrentPredictionProvenance::V4(provenance)) => Some(provenance),
            Some(CurrentPredictionProvenance::V3(_)) | None => None,
        }
    }

    /// Display path supplied by the producer.
    pub fn path(&self) -> &str {
        &self.evidence.path
    }

    /// Immutable identity of the source bytes used to produce this record.
    pub fn input(&self) -> &InputIdentity {
        &self.evidence.input
    }

    /// Nested measurement evidence.
    pub fn measurements(&self) -> &MeasurementContract {
        &self.evidence.measurements
    }

    /// Catalog-ordered check records.
    pub fn checks(&self) -> &[CheckEvaluation] {
        &self.checks
    }

    fn validate(&self) -> Result<(), OutputContractError> {
        require_measurements_v16(OUTPUT_V15_SCHEMA_VERSION, &self.evidence.measurements)?;
        if let Some(provenance) = &self.prediction_provenance {
            let primary_input = match provenance {
                CurrentPredictionProvenance::V3(provenance) => {
                    provenance.validate()?;
                    provenance.raw_source().primary_input()
                }
                CurrentPredictionProvenance::V4(provenance) => {
                    provenance.validate()?;
                    provenance.raw_source().primary_input()
                }
            };
            if primary_input != &self.evidence.input {
                return Err(OutputContractError::PredictionPrimaryInputMismatch);
            }
        }
        let mut facets = 0usize;
        let mut references = 0usize;
        let mut has_facet_budget_summary = false;
        let mut text = self
            .prediction_provenance
            .as_ref()
            .map(|provenance| match provenance {
                CurrentPredictionProvenance::V3(provenance) => provenance.retained_text_bytes(),
                CurrentPredictionProvenance::V4(provenance) => provenance.retained_text_bytes(),
            })
            .transpose()?
            .unwrap_or(0);
        for check in &self.checks {
            if check.engine_prediction().is_some() || check.engine_prediction_v2().is_some() {
                return Err(OutputContractError::HistoricalPredictionInV2Output);
            }
            let provenance_revision_matches = matches!(
                (
                    &self.prediction_provenance,
                    check.engine_prediction_v3(),
                    check.engine_prediction_v4()
                ),
                (Some(CurrentPredictionProvenance::V3(_)), Some(_), None)
                    | (Some(CurrentPredictionProvenance::V4(_)), None, Some(_))
                    | (_, None, None)
            );
            if !provenance_revision_matches {
                if self.prediction_provenance.is_none()
                    && (check.engine_prediction_v3().is_some()
                        || check.engine_prediction_v4().is_some())
                {
                    return Err(OutputContractError::PredictionWithoutProvenance);
                }
                return Err(OutputContractError::PredictionRevisionMismatch);
            }
            let legacy_v3_provenance = match self.prediction_provenance.as_ref() {
                Some(CurrentPredictionProvenance::V3(provenance)) => Some(provenance),
                Some(CurrentPredictionProvenance::V4(_)) | None => None,
            };
            if !matches!(
                self.prediction_provenance.as_ref(),
                Some(CurrentPredictionProvenance::V4(_))
            ) {
                validate_current_engine_clip_boundary_applicability_v3(
                    check.check_id(),
                    check.applicability(),
                    legacy_v3_provenance,
                )?;
            }
            if check.check_id() == ENGINE_CLIP_BOUNDARY_CHECK_ID
                && check.selection() == SelectionState::Selected
                && check.configuration() == ConfigurationState::Enabled
                && check.applicability() == Applicability::Applicable
                && check.engine_prediction_v3().is_none()
            {
                return Err(OutputContractError::InvalidPrediction(
                    PredictionContractError::EngineClipBoundaryFacetMismatch,
                ));
            }
            let current_v4_provenance = match self.prediction_provenance.as_ref() {
                Some(CurrentPredictionProvenance::V4(provenance)) => Some(provenance),
                Some(CurrentPredictionProvenance::V3(_)) | None => None,
            };
            validate_current_engine_unit_scale_prediction_v4(
                check.check_id(),
                check.selection(),
                check.configuration(),
                check.applicability(),
                check.engine_prediction_v4(),
                current_v4_provenance,
                &self.evidence.measurements,
            )?;
            if let Some(prediction) = check.engine_prediction_v3() {
                let Some(CurrentPredictionProvenance::V3(provenance)) =
                    self.prediction_provenance.as_ref()
                else {
                    return Err(OutputContractError::PredictionRevisionMismatch);
                };
                prediction.validate_against_provenance(provenance)?;
                prediction.validate_for_check(
                    check.check_id(),
                    check.evaluated_scopes(),
                    check.gaps(),
                    check.findings(),
                )?;
                validate_current_engine_addressability_prediction_v3(
                    check.check_id(),
                    prediction,
                    provenance,
                )?;
                let finding_scopes = check
                    .findings()
                    .iter()
                    .filter_map(|finding| finding.prediction_scope.as_ref())
                    .collect::<Vec<_>>();
                validate_current_engine_clip_boundary_prediction_v3(
                    check.check_id(),
                    prediction,
                    provenance,
                    check.evaluated_scopes(),
                    &finding_scopes,
                )?;
                has_facet_budget_summary |= prediction.has_facet_budget_summary();
                facets = facets
                    .checked_add(prediction.facets().len())
                    .ok_or(OutputContractError::ArithmeticOverflow)?;
                references = references
                    .checked_add(prediction.basis_reference_count())
                    .ok_or(OutputContractError::ArithmeticOverflow)?;
                text = text
                    .checked_add(prediction.retained_text_bytes()?)
                    .ok_or(OutputContractError::ArithmeticOverflow)?;
            }
            if let Some(prediction) = check.engine_prediction_v4() {
                let Some(CurrentPredictionProvenance::V4(provenance)) =
                    self.prediction_provenance.as_ref()
                else {
                    return Err(OutputContractError::PredictionRevisionMismatch);
                };
                prediction.validate_against_provenance(provenance)?;
                prediction.validate_for_check(
                    check.check_id(),
                    check.evaluated_scopes(),
                    check.gaps(),
                    check.findings(),
                )?;
                has_facet_budget_summary |= prediction.has_facet_budget_summary();
                facets = facets
                    .checked_add(prediction.facets().len())
                    .ok_or(OutputContractError::ArithmeticOverflow)?;
                references = references
                    .checked_add(prediction.basis_reference_count())
                    .ok_or(OutputContractError::ArithmeticOverflow)?;
                text = text
                    .checked_add(prediction.retained_text_bytes()?)
                    .ok_or(OutputContractError::ArithmeticOverflow)?;
            }
        }
        if facets > PREDICTION_V1_MAX_FACETS_PER_FILE {
            return Err(OutputContractError::TooManyPredictionFacets {
                found: facets,
                limit: PREDICTION_V1_MAX_FACETS_PER_FILE,
            });
        }
        if has_facet_budget_summary && facets != PREDICTION_V1_MAX_FACETS_PER_FILE {
            return Err(
                OutputContractError::FacetBudgetSummaryWithoutExhaustedFileBudget {
                    found: facets,
                    limit: PREDICTION_V1_MAX_FACETS_PER_FILE,
                },
            );
        }
        if references > PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE {
            return Err(OutputContractError::TooManyPredictionBasisReferences {
                found: references,
                limit: PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE,
            });
        }
        if text > PREDICTION_V1_MAX_TOTAL_TEXT_BYTES_PER_FILE {
            return Err(OutputContractError::TooMuchPredictionText {
                found: text,
                limit: PREDICTION_V1_MAX_TOTAL_TEXT_BYTES_PER_FILE,
            });
        }
        validate_measurement_references_batch_v3(
            &self.evidence.measurements,
            self.checks
                .iter()
                .enumerate()
                .filter_map(|(check_index, check)| {
                    check
                        .engine_prediction_v3()
                        .map(|prediction| (check_index, prediction))
                }),
        )
        .map_err(|error| OutputContractError::InvalidPrediction(error.source))?;
        validate_measurement_references_batch_v4(
            &self.evidence.measurements,
            self.checks
                .iter()
                .enumerate()
                .filter_map(|(check_index, check)| {
                    check
                        .engine_prediction_v4()
                        .map(|prediction| (check_index, prediction))
                }),
        )
        .map_err(|error| OutputContractError::InvalidPrediction(error.source))?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
struct EnvelopeHeaderV2 {
    schema_version: u32,
    schema: &'static str,
    tool: ToolInfo,
    command: &'static str,
}

/// Immutable historical output-v15 lint envelope.
#[derive(Debug, Clone, Serialize)]
pub struct LintEnvelope {
    #[serde(flatten)]
    header: EnvelopeHeaderV2,
    summary: LintSummary,
    files: Vec<LintFileReport>,
}

impl LintEnvelope {
    /// Construct a V2 lint envelope and derive its summaries.
    pub fn new(tool: ToolInfo, files: Vec<LintFileReport>) -> Result<Self, OutputContractError> {
        if files.len() > OUTPUT_V11_MAX_FILES {
            return Err(OutputContractError::TooManyFiles {
                found: files.len(),
                limit: OUTPUT_V11_MAX_FILES,
            });
        }
        let mut findings = FindingSummary::default();
        let mut checks = CheckSummary::default();
        let mut prediction_facets = PredictionFacetSummary::default();
        for file in &files {
            file.validate()?;
            for check in file.checks() {
                checks.total += 1;
                for finding in check.findings() {
                    findings.add(finding.severity);
                }
                match check.selection() {
                    SelectionState::Selected => checks.selection.selected += 1,
                    SelectionState::Unselected => checks.selection.unselected += 1,
                }
                match check.configuration() {
                    ConfigurationState::Enabled => checks.configuration.enabled += 1,
                    ConfigurationState::Disabled => checks.configuration.disabled += 1,
                }
                match check.applicability() {
                    Applicability::Applicable => checks.applicability.applicable += 1,
                    Applicability::NotApplicable => checks.applicability.not_applicable += 1,
                }
                match check.evaluation() {
                    EvaluationState::Complete => checks.evaluation.complete += 1,
                    EvaluationState::Partial => checks.evaluation.partial += 1,
                    EvaluationState::NotEvaluated => checks.evaluation.not_evaluated += 1,
                }
                checks.gaps += check.gaps().len();
                if let Some(prediction) = check.engine_prediction_v3() {
                    for facet in prediction.facets() {
                        match facet.state() {
                            EnginePredictionFacetStateV1::Available => {
                                prediction_facets.available += 1;
                            }
                            EnginePredictionFacetStateV1::RequiredPredictionUnavailable => {
                                prediction_facets.required_prediction_unavailable += 1;
                            }
                        }
                    }
                }
                if let Some(prediction) = check.engine_prediction_v4() {
                    for facet in prediction.facets() {
                        match facet.state() {
                            EnginePredictionFacetStateV1::Available => {
                                prediction_facets.available += 1;
                            }
                            EnginePredictionFacetStateV1::RequiredPredictionUnavailable => {
                                prediction_facets.required_prediction_unavailable += 1;
                            }
                        }
                    }
                }
            }
        }
        Ok(Self {
            header: EnvelopeHeaderV2 {
                schema_version: OUTPUT_V15_SCHEMA_VERSION,
                schema: OUTPUT_V15_SCHEMA_ID,
                tool,
                command: "lint",
            },
            summary: LintSummary {
                files: files.len(),
                findings,
                checks,
                prediction_facets,
            },
            files,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
#[allow(clippy::large_enum_variant)]
enum CurrentPredictionProvenanceV16 {
    V3(PredictionProvenanceV3),
    V5(PredictionProvenanceV5),
}

/// Current output-v16 lint file evidence with V3 or V5 prediction provenance.
#[derive(Debug, Clone, Serialize)]
pub struct LintFileReportV16 {
    #[serde(flatten)]
    evidence: FileEvidence,
    prediction_provenance: Option<CurrentPredictionProvenanceV16>,
    checks: Vec<CheckEvaluation>,
}

impl LintFileReportV16 {
    /// Construct a revision-1-profile record whose V3 graph is unchanged.
    pub fn new(
        path: impl Into<String>,
        input: InputIdentity,
        rig: RigInfo,
        prediction_provenance: Option<PredictionProvenanceV3>,
        checks: Vec<CheckEvaluation>,
        measurements: MeasurementContract,
    ) -> Result<Self, OutputContractError> {
        let report = Self {
            evidence: FileEvidence::historical_v16(path, input, rig, measurements)?,
            prediction_provenance: prediction_provenance.map(CurrentPredictionProvenanceV16::V3),
            checks,
        };
        report.validate()?;
        Ok(report)
    }

    /// Construct a revision-2-contract record using V5 provenance and checks.
    pub fn new_v5(
        path: impl Into<String>,
        input: InputIdentity,
        rig: RigInfo,
        prediction_provenance: Option<PredictionProvenanceV5>,
        checks: Vec<CheckEvaluation>,
        measurements: MeasurementContract,
    ) -> Result<Self, OutputContractError> {
        let report = Self {
            evidence: FileEvidence::historical_v16(path, input, rig, measurements)?,
            prediction_provenance: prediction_provenance.map(CurrentPredictionProvenanceV16::V5),
            checks,
        };
        report.validate()?;
        Ok(report)
    }

    /// Display path supplied by the producer.
    pub fn path(&self) -> &str {
        &self.evidence.path
    }
    /// Immutable source-byte identity.
    pub fn input(&self) -> &InputIdentity {
        &self.evidence.input
    }
    /// Nested measurement evidence.
    pub fn measurements(&self) -> &MeasurementContract {
        &self.evidence.measurements
    }
    /// Catalog-ordered check records.
    pub fn checks(&self) -> &[CheckEvaluation] {
        &self.checks
    }
    /// V3 provenance for a revision-1 profile.
    pub const fn prediction_provenance_v3(&self) -> Option<&PredictionProvenanceV3> {
        match self.prediction_provenance.as_ref() {
            Some(CurrentPredictionProvenanceV16::V3(provenance)) => Some(provenance),
            Some(CurrentPredictionProvenanceV16::V5(_)) | None => None,
        }
    }
    /// V5 provenance for a revision-2-contract profile.
    pub const fn prediction_provenance_v5(&self) -> Option<&PredictionProvenanceV5> {
        match self.prediction_provenance.as_ref() {
            Some(CurrentPredictionProvenanceV16::V5(provenance)) => Some(provenance),
            Some(CurrentPredictionProvenanceV16::V3(_)) | None => None,
        }
    }

    fn validate(&self) -> Result<(), OutputContractError> {
        require_measurements_v16(OUTPUT_V16_SCHEMA_VERSION, &self.evidence.measurements)?;
        if self.checks.len() > OUTPUT_V11_MAX_CHECKS_PER_FILE {
            return Err(OutputContractError::TooManyChecks {
                found: self.checks.len(),
                limit: OUTPUT_V11_MAX_CHECKS_PER_FILE,
            });
        }
        if let Some(CurrentPredictionProvenanceV16::V3(provenance)) = &self.prediction_provenance {
            return LintFileReport {
                evidence: self.evidence.clone(),
                prediction_provenance: Some(CurrentPredictionProvenance::V3(provenance.clone())),
                checks: self.checks.clone(),
            }
            .validate();
        }
        let provenance = match &self.prediction_provenance {
            Some(CurrentPredictionProvenanceV16::V5(provenance)) => {
                provenance.validate()?;
                if provenance.base().raw_source().primary_input() != &self.evidence.input {
                    return Err(OutputContractError::PredictionPrimaryInputMismatch);
                }
                Some(provenance)
            }
            Some(CurrentPredictionProvenanceV16::V3(_)) => unreachable!(),
            None => None,
        };
        let mut facets = 0usize;
        let mut references = 0usize;
        let mut has_facet_budget_summary = false;
        let mut text = provenance
            .map(PredictionProvenanceV5::retained_text_bytes)
            .transpose()?
            .unwrap_or(0);
        for check in &self.checks {
            if check.engine_prediction().is_some()
                || check.engine_prediction_v2().is_some()
                || check.engine_prediction_v3().is_some()
                || check.engine_prediction_v4().is_some()
            {
                return Err(OutputContractError::HistoricalPredictionInV2Output);
            }
            let prediction = check.engine_prediction_v5();
            if provenance.is_none() && prediction.is_some() {
                return Err(OutputContractError::PredictionWithoutProvenance);
            }
            if let Some(prediction) = prediction {
                let provenance =
                    provenance.ok_or(OutputContractError::PredictionWithoutProvenance)?;
                prediction.validate_against_provenance(provenance)?;
                prediction.validate_for_check(
                    check.check_id(),
                    check.evaluated_scopes(),
                    check.gaps(),
                    check.findings(),
                )?;
                has_facet_budget_summary |= prediction.base_prediction().has_facet_budget_summary();
                facets = facets
                    .checked_add(prediction.facets().len())
                    .ok_or(OutputContractError::ArithmeticOverflow)?;
                references = references
                    .checked_add(prediction.basis_reference_count())
                    .ok_or(OutputContractError::ArithmeticOverflow)?;
                text = text
                    .checked_add(prediction.retained_text_bytes()?)
                    .ok_or(OutputContractError::ArithmeticOverflow)?;
            }
            validate_current_engine_track_support_prediction_v5(
                check.check_id(),
                check.selection(),
                check.configuration(),
                check.applicability(),
                prediction,
                provenance,
                check.findings().is_empty(),
            )?;
            validate_current_engine_unit_scale_prediction_v5(
                check.check_id(),
                check.selection(),
                check.configuration(),
                check.applicability(),
                prediction,
                provenance,
                &self.evidence.measurements,
            )?;
        }
        if facets > PREDICTION_V1_MAX_FACETS_PER_FILE {
            return Err(OutputContractError::TooManyPredictionFacets {
                found: facets,
                limit: PREDICTION_V1_MAX_FACETS_PER_FILE,
            });
        }
        if has_facet_budget_summary && facets != PREDICTION_V1_MAX_FACETS_PER_FILE {
            return Err(
                OutputContractError::FacetBudgetSummaryWithoutExhaustedFileBudget {
                    found: facets,
                    limit: PREDICTION_V1_MAX_FACETS_PER_FILE,
                },
            );
        }
        if references > PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE {
            return Err(OutputContractError::TooManyPredictionBasisReferences {
                found: references,
                limit: PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE,
            });
        }
        if text > PREDICTION_V1_MAX_TOTAL_TEXT_BYTES_PER_FILE {
            return Err(OutputContractError::TooMuchPredictionText {
                found: text,
                limit: PREDICTION_V1_MAX_TOTAL_TEXT_BYTES_PER_FILE,
            });
        }
        validate_measurement_references_batch_v4(
            &self.evidence.measurements,
            self.checks.iter().enumerate().filter_map(|(index, check)| {
                check
                    .engine_prediction_v5()
                    .map(|prediction| (index, prediction.base_prediction()))
            }),
        )
        .map_err(|error| OutputContractError::InvalidPrediction(error.source))?;
        Ok(())
    }
}

/// Current output-v16 lint envelope.
#[derive(Debug, Clone, Serialize)]
pub struct LintEnvelopeV16 {
    #[serde(flatten)]
    header: EnvelopeHeaderV2,
    summary: LintSummary,
    files: Vec<LintFileReportV16>,
}

impl LintEnvelopeV16 {
    /// Construct a schema-valid V16 lint envelope and derive summaries.
    pub fn new(tool: ToolInfo, files: Vec<LintFileReportV16>) -> Result<Self, OutputContractError> {
        if files.len() > OUTPUT_V11_MAX_FILES {
            return Err(OutputContractError::TooManyFiles {
                found: files.len(),
                limit: OUTPUT_V11_MAX_FILES,
            });
        }
        let mut findings = FindingSummary::default();
        let mut checks = CheckSummary::default();
        let mut prediction_facets = PredictionFacetSummary::default();
        for file in &files {
            file.validate()?;
            for check in file.checks() {
                checks.total += 1;
                for finding in check.findings() {
                    findings.add(finding.severity);
                }
                match check.selection() {
                    SelectionState::Selected => checks.selection.selected += 1,
                    SelectionState::Unselected => checks.selection.unselected += 1,
                }
                match check.configuration() {
                    ConfigurationState::Enabled => checks.configuration.enabled += 1,
                    ConfigurationState::Disabled => checks.configuration.disabled += 1,
                }
                match check.applicability() {
                    Applicability::Applicable => checks.applicability.applicable += 1,
                    Applicability::NotApplicable => checks.applicability.not_applicable += 1,
                }
                match check.evaluation() {
                    EvaluationState::Complete => checks.evaluation.complete += 1,
                    EvaluationState::Partial => checks.evaluation.partial += 1,
                    EvaluationState::NotEvaluated => checks.evaluation.not_evaluated += 1,
                }
                checks.gaps += check.gaps().len();
                for facet in check
                    .engine_prediction_v3()
                    .into_iter()
                    .flat_map(EnginePredictionV3::facets)
                {
                    match facet.state() {
                        EnginePredictionFacetStateV1::Available => prediction_facets.available += 1,
                        EnginePredictionFacetStateV1::RequiredPredictionUnavailable => {
                            prediction_facets.required_prediction_unavailable += 1
                        }
                    }
                }
                for facet in check
                    .engine_prediction_v5()
                    .into_iter()
                    .flat_map(EnginePredictionV5::facets)
                {
                    match facet.state() {
                        EnginePredictionFacetStateV1::Available => prediction_facets.available += 1,
                        EnginePredictionFacetStateV1::RequiredPredictionUnavailable => {
                            prediction_facets.required_prediction_unavailable += 1
                        }
                    }
                }
            }
        }
        Ok(Self {
            header: EnvelopeHeaderV2 {
                schema_version: OUTPUT_V16_SCHEMA_VERSION,
                schema: OUTPUT_V16_SCHEMA_ID,
                tool,
                command: "lint",
            },
            summary: LintSummary {
                files: files.len(),
                findings,
                checks,
                prediction_facets,
            },
            files,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
#[allow(clippy::large_enum_variant)]
enum CurrentPredictionProvenanceV17 {
    V3(PredictionProvenanceV3),
    V5(PredictionProvenanceV5),
    V6(PredictionProvenanceV6),
}

/// Historical output-v17 lint file evidence with immutable V3, V5, or V6 provenance.
#[derive(Debug, Clone, Serialize)]
pub struct LintFileReportV17 {
    #[serde(flatten)]
    evidence: FileEvidence,
    prediction_provenance: Option<CurrentPredictionProvenanceV17>,
    checks: Vec<CheckEvaluation>,
}

impl LintFileReportV17 {
    /// Construct a revision-1-profile record whose V3 graph is unchanged.
    pub fn new(
        path: impl Into<String>,
        input: InputIdentity,
        rig: RigInfo,
        prediction_provenance: Option<PredictionProvenanceV3>,
        checks: Vec<CheckEvaluation>,
        measurements: MeasurementContract,
    ) -> Result<Self, OutputContractError> {
        let report = Self {
            evidence: FileEvidence::historical_v16(path, input, rig, measurements)?,
            prediction_provenance: prediction_provenance.map(CurrentPredictionProvenanceV17::V3),
            checks,
        };
        report.validate()?;
        Ok(report)
    }

    /// Construct a V5 record whose output-v16 graph remains unchanged.
    pub fn new_v5(
        path: impl Into<String>,
        input: InputIdentity,
        rig: RigInfo,
        prediction_provenance: Option<PredictionProvenanceV5>,
        checks: Vec<CheckEvaluation>,
        measurements: MeasurementContract,
    ) -> Result<Self, OutputContractError> {
        let report = Self {
            evidence: FileEvidence::historical_v16(path, input, rig, measurements)?,
            prediction_provenance: prediction_provenance.map(CurrentPredictionProvenanceV17::V5),
            checks,
        };
        report.validate()?;
        Ok(report)
    }

    /// Construct a transform-path-and-intent-bound V6 record.
    pub fn new_v6(
        path: impl Into<String>,
        input: InputIdentity,
        rig: RigInfo,
        prediction_provenance: Option<PredictionProvenanceV6>,
        checks: Vec<CheckEvaluation>,
        measurements: MeasurementContract,
    ) -> Result<Self, OutputContractError> {
        let report = Self {
            evidence: FileEvidence::historical_v16(path, input, rig, measurements)?,
            prediction_provenance: prediction_provenance.map(CurrentPredictionProvenanceV17::V6),
            checks,
        };
        report.validate()?;
        Ok(report)
    }

    /// Display path supplied by the producer.
    pub fn path(&self) -> &str {
        &self.evidence.path
    }
    /// Immutable source-byte identity.
    pub fn input(&self) -> &InputIdentity {
        &self.evidence.input
    }
    /// Nested measurement evidence.
    pub fn measurements(&self) -> &MeasurementContract {
        &self.evidence.measurements
    }
    /// Catalog-ordered check records.
    pub fn checks(&self) -> &[CheckEvaluation] {
        &self.checks
    }
    /// V3 provenance for a revision-1 profile.
    pub const fn prediction_provenance_v3(&self) -> Option<&PredictionProvenanceV3> {
        match self.prediction_provenance.as_ref() {
            Some(CurrentPredictionProvenanceV17::V3(provenance)) => Some(provenance),
            Some(CurrentPredictionProvenanceV17::V5(_) | CurrentPredictionProvenanceV17::V6(_))
            | None => None,
        }
    }
    /// Immutable V5 provenance.
    pub const fn prediction_provenance_v5(&self) -> Option<&PredictionProvenanceV5> {
        match self.prediction_provenance.as_ref() {
            Some(CurrentPredictionProvenanceV17::V5(provenance)) => Some(provenance),
            Some(CurrentPredictionProvenanceV17::V3(_) | CurrentPredictionProvenanceV17::V6(_))
            | None => None,
        }
    }
    /// Current V6 provenance.
    pub const fn prediction_provenance_v6(&self) -> Option<&PredictionProvenanceV6> {
        match self.prediction_provenance.as_ref() {
            Some(CurrentPredictionProvenanceV17::V6(provenance)) => Some(provenance),
            Some(CurrentPredictionProvenanceV17::V3(_) | CurrentPredictionProvenanceV17::V5(_))
            | None => None,
        }
    }

    fn validate(&self) -> Result<(), OutputContractError> {
        require_measurements_v16(OUTPUT_V17_SCHEMA_VERSION, &self.evidence.measurements)?;
        match &self.prediction_provenance {
            Some(CurrentPredictionProvenanceV17::V3(provenance)) => {
                return LintFileReportV16::new(
                    self.evidence.path.clone(),
                    self.evidence.input.clone(),
                    self.evidence.rig.clone(),
                    Some(provenance.clone()),
                    self.checks.clone(),
                    self.evidence.measurements.clone(),
                )
                .map(|_| ());
            }
            Some(CurrentPredictionProvenanceV17::V5(provenance)) => {
                return LintFileReportV16::new_v5(
                    self.evidence.path.clone(),
                    self.evidence.input.clone(),
                    self.evidence.rig.clone(),
                    Some(provenance.clone()),
                    self.checks.clone(),
                    self.evidence.measurements.clone(),
                )
                .map(|_| ());
            }
            Some(CurrentPredictionProvenanceV17::V6(_)) => {}
            None => {
                return LintFileReportV16::new(
                    self.evidence.path.clone(),
                    self.evidence.input.clone(),
                    self.evidence.rig.clone(),
                    None,
                    self.checks.clone(),
                    self.evidence.measurements.clone(),
                )
                .map(|_| ());
            }
        }
        if self.checks.len() > OUTPUT_V11_MAX_CHECKS_PER_FILE {
            return Err(OutputContractError::TooManyChecks {
                found: self.checks.len(),
                limit: OUTPUT_V11_MAX_CHECKS_PER_FILE,
            });
        }
        let provenance = match &self.prediction_provenance {
            Some(CurrentPredictionProvenanceV17::V6(provenance)) => {
                provenance.validate()?;
                if provenance.base().base().raw_source().primary_input() != &self.evidence.input {
                    return Err(OutputContractError::PredictionPrimaryInputMismatch);
                }
                Some(provenance)
            }
            _ => unreachable!(),
        };
        let mut facets = 0usize;
        let mut references = 0usize;
        let mut has_facet_budget_summary = false;
        let mut text = provenance
            .map(PredictionProvenanceV6::retained_text_bytes)
            .transpose()?
            .unwrap_or(0);
        for check in &self.checks {
            if check.engine_prediction().is_some()
                || check.engine_prediction_v2().is_some()
                || check.engine_prediction_v3().is_some()
                || check.engine_prediction_v4().is_some()
                || check.engine_prediction_v5().is_some()
            {
                return Err(OutputContractError::HistoricalPredictionInV2Output);
            }
            let prediction = check.engine_prediction_v6();
            if let Some(prediction) = prediction {
                let provenance =
                    provenance.ok_or(OutputContractError::PredictionWithoutProvenance)?;
                prediction.validate_against_provenance(provenance)?;
                prediction.validate_for_check(
                    check.check_id(),
                    check.evaluated_scopes(),
                    check.gaps(),
                    check.findings(),
                )?;
                has_facet_budget_summary |= prediction.base_prediction().has_facet_budget_summary();
                facets = facets
                    .checked_add(prediction.facets().len())
                    .ok_or(OutputContractError::ArithmeticOverflow)?;
                references = references
                    .checked_add(prediction.basis_reference_count())
                    .ok_or(OutputContractError::ArithmeticOverflow)?;
                text = text
                    .checked_add(prediction.retained_text_bytes()?)
                    .ok_or(OutputContractError::ArithmeticOverflow)?;
            }
            validate_current_engine_root_motion_prediction_v6(
                check.check_id(),
                check.selection(),
                check.configuration(),
                check.applicability(),
                prediction,
                provenance,
                check.findings(),
                &self.evidence.rig,
                &self.evidence.measurements,
            )?;
        }
        if facets > PREDICTION_V1_MAX_FACETS_PER_FILE {
            return Err(OutputContractError::TooManyPredictionFacets {
                found: facets,
                limit: PREDICTION_V1_MAX_FACETS_PER_FILE,
            });
        }
        if has_facet_budget_summary && facets != PREDICTION_V1_MAX_FACETS_PER_FILE {
            return Err(
                OutputContractError::FacetBudgetSummaryWithoutExhaustedFileBudget {
                    found: facets,
                    limit: PREDICTION_V1_MAX_FACETS_PER_FILE,
                },
            );
        }
        if references > PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE {
            return Err(OutputContractError::TooManyPredictionBasisReferences {
                found: references,
                limit: PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE,
            });
        }
        if text > PREDICTION_V1_MAX_TOTAL_TEXT_BYTES_PER_FILE {
            return Err(OutputContractError::TooMuchPredictionText {
                found: text,
                limit: PREDICTION_V1_MAX_TOTAL_TEXT_BYTES_PER_FILE,
            });
        }
        validate_measurement_references_batch_v4(
            &self.evidence.measurements,
            self.checks.iter().enumerate().filter_map(|(index, check)| {
                check
                    .engine_prediction_v6()
                    .map(|prediction| (index, prediction.base_prediction()))
            }),
        )
        .map_err(|error| OutputContractError::InvalidPrediction(error.source))?;
        Ok(())
    }
}

/// Current output-v18 lint-file payload. Immutable prediction contracts are
/// checked against the deterministic measurements-v16 projection while the
/// report retains the complete measurements-v17 evidence.
#[derive(Debug, Clone, Serialize)]
pub struct LintFileReportV18 {
    #[serde(flatten)]
    evidence: FileEvidence,
    prediction_provenance: Option<CurrentPredictionProvenanceV17>,
    checks: Vec<CheckEvaluation>,
}

impl LintFileReportV18 {
    /// Construct a revision-1-profile record whose V3 graph is unchanged.
    pub fn new(
        path: impl Into<String>,
        input: InputIdentity,
        rig: RigInfo,
        prediction_provenance: Option<PredictionProvenanceV3>,
        checks: Vec<CheckEvaluation>,
        measurements: MeasurementContract,
    ) -> Result<Self, OutputContractError> {
        Self::build(
            path,
            input,
            rig,
            prediction_provenance.map(CurrentPredictionProvenanceV17::V3),
            checks,
            measurements,
        )
    }

    /// Construct a V5 record whose immutable prediction graph consumes V16.
    pub fn new_v5(
        path: impl Into<String>,
        input: InputIdentity,
        rig: RigInfo,
        prediction_provenance: Option<PredictionProvenanceV5>,
        checks: Vec<CheckEvaluation>,
        measurements: MeasurementContract,
    ) -> Result<Self, OutputContractError> {
        Self::build(
            path,
            input,
            rig,
            prediction_provenance.map(CurrentPredictionProvenanceV17::V5),
            checks,
            measurements,
        )
    }

    /// Construct a V6 record whose immutable prediction graph consumes V16.
    pub fn new_v6(
        path: impl Into<String>,
        input: InputIdentity,
        rig: RigInfo,
        prediction_provenance: Option<PredictionProvenanceV6>,
        checks: Vec<CheckEvaluation>,
        measurements: MeasurementContract,
    ) -> Result<Self, OutputContractError> {
        Self::build(
            path,
            input,
            rig,
            prediction_provenance.map(CurrentPredictionProvenanceV17::V6),
            checks,
            measurements,
        )
    }

    fn build(
        path: impl Into<String>,
        input: InputIdentity,
        rig: RigInfo,
        prediction_provenance: Option<CurrentPredictionProvenanceV17>,
        checks: Vec<CheckEvaluation>,
        measurements: MeasurementContract,
    ) -> Result<Self, OutputContractError> {
        require_measurements_v17(OUTPUT_SCHEMA_VERSION, &measurements)?;
        let report = Self {
            evidence: FileEvidence::new(path, input, rig, measurements),
            prediction_provenance,
            checks,
        };
        report.historical_prediction_view()?;
        Ok(report)
    }

    /// Display path supplied by the producer.
    pub fn path(&self) -> &str {
        &self.evidence.path
    }

    /// Immutable source-byte identity.
    pub fn input(&self) -> &InputIdentity {
        &self.evidence.input
    }

    /// Nested current measurement evidence.
    pub fn measurements(&self) -> &MeasurementContract {
        &self.evidence.measurements
    }

    /// Catalog-ordered check records.
    pub fn checks(&self) -> &[CheckEvaluation] {
        &self.checks
    }

    /// V3 provenance for a revision-1 profile.
    pub const fn prediction_provenance_v3(&self) -> Option<&PredictionProvenanceV3> {
        match self.prediction_provenance.as_ref() {
            Some(CurrentPredictionProvenanceV17::V3(provenance)) => Some(provenance),
            Some(CurrentPredictionProvenanceV17::V5(_) | CurrentPredictionProvenanceV17::V6(_))
            | None => None,
        }
    }

    /// Immutable V5 provenance.
    pub const fn prediction_provenance_v5(&self) -> Option<&PredictionProvenanceV5> {
        match self.prediction_provenance.as_ref() {
            Some(CurrentPredictionProvenanceV17::V5(provenance)) => Some(provenance),
            Some(CurrentPredictionProvenanceV17::V3(_) | CurrentPredictionProvenanceV17::V6(_))
            | None => None,
        }
    }

    /// Immutable V6 provenance.
    pub const fn prediction_provenance_v6(&self) -> Option<&PredictionProvenanceV6> {
        match self.prediction_provenance.as_ref() {
            Some(CurrentPredictionProvenanceV17::V6(provenance)) => Some(provenance),
            Some(CurrentPredictionProvenanceV17::V3(_) | CurrentPredictionProvenanceV17::V5(_))
            | None => None,
        }
    }

    fn historical_prediction_view(&self) -> Result<LintFileReportV17, OutputContractError> {
        let measurements = self.evidence.measurements.prediction_v16_projection()?;
        let report = LintFileReportV17 {
            evidence: FileEvidence::new(
                self.evidence.path.clone(),
                self.evidence.input.clone(),
                self.evidence.rig.clone(),
                measurements,
            ),
            prediction_provenance: self.prediction_provenance.clone(),
            checks: self.checks.clone(),
        };
        report.validate()?;
        Ok(report)
    }
}

/// Historical output-v17 lint envelope retained for immutable writer tests and
/// the output-v18 prediction projection.
#[derive(Debug, Clone, Serialize)]
pub struct LintEnvelopeV17 {
    #[serde(flatten)]
    header: EnvelopeHeaderV2,
    summary: LintSummary,
    files: Vec<LintFileReportV17>,
}

impl LintEnvelopeV17 {
    /// Construct a schema-valid V17 lint envelope and derive summaries.
    pub fn new(tool: ToolInfo, files: Vec<LintFileReportV17>) -> Result<Self, OutputContractError> {
        if files.len() > OUTPUT_V11_MAX_FILES {
            return Err(OutputContractError::TooManyFiles {
                found: files.len(),
                limit: OUTPUT_V11_MAX_FILES,
            });
        }
        let mut findings = FindingSummary::default();
        let mut checks = CheckSummary::default();
        let mut prediction_facets = PredictionFacetSummary::default();
        for file in &files {
            file.validate()?;
            for check in file.checks() {
                checks.total += 1;
                for finding in check.findings() {
                    findings.add(finding.severity);
                }
                match check.selection() {
                    SelectionState::Selected => checks.selection.selected += 1,
                    SelectionState::Unselected => checks.selection.unselected += 1,
                }
                match check.configuration() {
                    ConfigurationState::Enabled => checks.configuration.enabled += 1,
                    ConfigurationState::Disabled => checks.configuration.disabled += 1,
                }
                match check.applicability() {
                    Applicability::Applicable => checks.applicability.applicable += 1,
                    Applicability::NotApplicable => checks.applicability.not_applicable += 1,
                }
                match check.evaluation() {
                    EvaluationState::Complete => checks.evaluation.complete += 1,
                    EvaluationState::Partial => checks.evaluation.partial += 1,
                    EvaluationState::NotEvaluated => checks.evaluation.not_evaluated += 1,
                }
                checks.gaps += check.gaps().len();
                let mut add_facet = |state| match state {
                    EnginePredictionFacetStateV1::Available => prediction_facets.available += 1,
                    EnginePredictionFacetStateV1::RequiredPredictionUnavailable => {
                        prediction_facets.required_prediction_unavailable += 1
                    }
                };
                for facet in check
                    .engine_prediction_v3()
                    .into_iter()
                    .flat_map(EnginePredictionV3::facets)
                {
                    add_facet(facet.state());
                }
                for facet in check
                    .engine_prediction_v5()
                    .into_iter()
                    .flat_map(EnginePredictionV5::facets)
                {
                    add_facet(facet.state());
                }
                for facet in check
                    .engine_prediction_v6()
                    .into_iter()
                    .flat_map(EnginePredictionV6::facets)
                {
                    add_facet(facet.state());
                }
            }
        }
        Ok(Self {
            header: EnvelopeHeaderV2 {
                schema_version: OUTPUT_V17_SCHEMA_VERSION,
                schema: OUTPUT_V17_SCHEMA_ID,
                tool,
                command: "lint",
            },
            summary: LintSummary {
                files: files.len(),
                findings,
                checks,
                prediction_facets,
            },
            files,
        })
    }
}

/// Current output-v18 lint envelope.
#[derive(Debug, Clone, Serialize)]
pub struct LintEnvelopeV18 {
    #[serde(flatten)]
    header: EnvelopeHeaderV2,
    summary: LintSummary,
    files: Vec<LintFileReportV18>,
}

impl LintEnvelopeV18 {
    /// Construct a schema-valid V18 lint envelope and derive summaries.
    pub fn new(tool: ToolInfo, files: Vec<LintFileReportV18>) -> Result<Self, OutputContractError> {
        if files.len() > OUTPUT_V11_MAX_FILES {
            return Err(OutputContractError::TooManyFiles {
                found: files.len(),
                limit: OUTPUT_V11_MAX_FILES,
            });
        }
        for file in &files {
            require_measurements_v17(OUTPUT_SCHEMA_VERSION, &file.evidence.measurements)?;
        }
        let historical_files = files
            .iter()
            .map(LintFileReportV18::historical_prediction_view)
            .collect::<Result<Vec<_>, _>>()?;
        let historical = LintEnvelopeV17::new(tool.clone(), historical_files)?;
        Ok(Self {
            header: EnvelopeHeaderV2 {
                schema_version: OUTPUT_SCHEMA_VERSION,
                schema: OUTPUT_SCHEMA_ID,
                tool,
                command: "lint",
            },
            summary: historical.summary,
            files,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
struct DiffInputs {
    before: String,
    after: String,
}

#[derive(Debug, Clone, Serialize)]
struct DiffSummary {
    deltas: usize,
}

/// Current diff-command result envelope.
#[derive(Debug, Serialize)]
pub struct DiffEnvelope {
    #[serde(flatten)]
    header: EnvelopeHeader,
    inputs: DiffInputs,
    summary: DiffSummary,
    deltas: Vec<MetricDelta>,
}

impl DiffEnvelope {
    /// Construct a schema-valid diff envelope.
    pub fn new(
        tool: ToolInfo,
        before: impl Into<String>,
        after: impl Into<String>,
        deltas: Vec<MetricDelta>,
    ) -> Self {
        Self {
            header: EnvelopeHeader::new(tool, "diff"),
            inputs: DiffInputs {
                before: before.into(),
                after: after.into(),
            },
            summary: DiffSummary {
                deltas: deltas.len(),
            },
            deltas,
        }
    }
}
