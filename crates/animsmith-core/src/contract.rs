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

use crate::diff::MetricDelta;
use crate::evaluation::{
    Applicability, CheckEvaluation, CheckEvaluationGapRef, CheckEvaluationValidationInput,
    ConfigurationState, EvaluationState, SelectionState, validate_and_derive_check_evaluation,
};
use crate::measure::{
    Aabb, AssetMeasurements, ClipMeasurements, ImageMeasurements, LinearTransformClassification,
    LinearTransformMeasurements, MaterialDefinitionMeasurements, MeasurementAvailability,
    SkeletonNodeLocalRestMeasurements, SkeletonRestWorldMatrixUnavailableReason,
    SkinDerivedMatrixMeasurements, SkinDerivedMatrixUnavailableReason, TextureMeasurements,
    assess_inverse_bind, measure_linear_transform, summarize_skin_bind_linear,
};
use crate::metrics::canonical_net_yaw_deg;
use crate::model::{
    DecodedImageColorType, MaterialResourceCoverage, SourceInverseBindAccessorStatus,
    SourceSkeletonCoverage,
};
use crate::prediction::{
    EnginePredictionFacetStateV1, PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE,
    PREDICTION_V1_MAX_FACETS_PER_FILE, PREDICTION_V1_MAX_TOTAL_TEXT_BYTES_PER_FILE,
    PredictionContractError, PredictionDecodeError, PredictionProvenanceV1,
    decode_engine_prediction_v1, decode_prediction_provenance_v1,
    validate_measurement_references_batch,
};
use crate::profile::ResolvedRoles;
use crate::{Document, Severity};

/// Current outer result-envelope version.
pub const OUTPUT_SCHEMA_VERSION: u32 = 10;
/// Immutable identity of the current outer result envelope.
pub const OUTPUT_SCHEMA_ID: &str = "urn:animsmith:schema:output:10";
/// Maximum serialized bytes accepted by the output-v10 report reader.
pub const OUTPUT_V10_MAX_REPORT_BYTES: u64 = 256 * 1024 * 1024;
/// Maximum file records carried by one output-v10 envelope.
pub const OUTPUT_V10_MAX_FILES: usize = 4_096;
/// Maximum check records carried by one output-v10 lint file.
pub const OUTPUT_V10_MAX_CHECKS_PER_FILE: usize = 4_096;
/// Current nested measurement-contract version.
pub const MEASUREMENTS_SCHEMA_VERSION: u32 = 15;
/// Immutable identity of the current nested measurement contract.
pub const MEASUREMENTS_SCHEMA_ID: &str = "urn:animsmith:schema:measurements:15";

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
    /// envelope constructed through this API remains within output v10.
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RigInfo {
    profile: String,
    resolved_roles: BTreeMap<&'static str, String>,
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
        let resolved_roles = roles
            .iter_with_names()
            .map(|(role, bone, expected_name)| {
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
                Ok((role.as_str(), name.name.clone()))
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            profile: roles.profile.clone(),
            resolved_roles,
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
        validate_measurements(&clips, &assets)?;
        Ok(Self {
            schema_version: MEASUREMENTS_SCHEMA_VERSION,
            schema: MEASUREMENTS_SCHEMA_ID,
            clips,
            assets,
        })
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

fn validate_measurements(
    clips: &BTreeMap<String, ClipMeasurements>,
    assets: &AssetMeasurements,
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
                for (field, value) in [
                    ("position_delta_m", bone.position_delta_m),
                    ("rotation_delta_deg", bone.rotation_delta_deg),
                    ("seam_velocity_delta_mps", bone.seam_velocity_delta_mps),
                    (
                        "seam_angular_velocity_delta_degps",
                        bone.seam_angular_velocity_delta_degps,
                    ),
                ] {
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
    validate_material_resources(assets, &invalid)?;
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
        validate_image_measurement(image, offset, invalid)?;
    }
    Ok(())
}

fn validate_image_measurement(
    image: &ImageMeasurements,
    offset: usize,
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
/// measurement contract while retaining every legitimate output-v10 root
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
    #[serde(rename = "rig")]
    _rig: Box<RawValue>,
    measurements: Option<Box<RawValue>>,
    #[serde(default, deserialize_with = "deserialize_required_nullable")]
    prediction_provenance: RequiredNullable<Box<RawValue>>,
    checks: Option<Vec<Box<RawValue>>>,
}

#[derive(Debug)]
struct MeasurementFileInput {
    path: Option<String>,
    input: Option<InputIdentityInput>,
    measurements: Option<Box<RawValue>>,
    prediction_provenance: RequiredNullable<PredictionProvenanceV1>,
    checks: Option<Vec<PredictionCheckInput>>,
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
    prediction: Option<crate::prediction::EnginePredictionV1>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PredictionFindingInput {
    check_id: String,
    #[serde(rename = "severity")]
    _severity: PredictionSeverityInput,
    #[serde(rename = "clip")]
    _clip: Option<String>,
    #[serde(rename = "bone")]
    _bone: Option<String>,
    #[serde(rename = "node")]
    _node: Option<String>,
    prediction_scope: Option<crate::evaluation::EvaluationScope>,
    #[serde(rename = "time_s")]
    _time_s: Option<f32>,
    #[serde(rename = "measured")]
    _measured: Option<Box<RawValue>>,
    #[serde(rename = "expected")]
    _expected: Option<Box<RawValue>>,
    #[serde(rename = "members")]
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

#[derive(Debug, Deserialize)]
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

fn decode_measurement_payload(
    raw: &RawValue,
) -> Result<MeasurementPayloadInput, serde_json::Error> {
    let mut deserializer = serde_json::Deserializer::from_str(raw.get());
    let payload = MeasurementPayloadInput::deserialize(MeasurementF32NarrowingDeserializer(
        &mut deserializer,
    ))?;
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
    /// A current output-v10 envelope carried a field outside its closed schema.
    #[error("report envelope has unknown field `{field}`")]
    UnknownOutputField {
        /// Lexically first unknown root field.
        field: String,
    },
    /// A current output-v10 envelope omitted its producer metadata.
    #[error("report envelope has no `tool` object")]
    MissingTool,
    /// The outer envelope omitted its file array.
    #[error("report envelope has no `files` array")]
    MissingFiles,
    /// The outer envelope exceeds the immutable file-record bound.
    #[error("report contains {found} files, exceeding the output-v10 limit of {limit}")]
    TooManyFiles {
        /// Supplied file count.
        found: usize,
        /// Immutable output-v10 limit.
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

/// A serialized output-v10 report could not be read within the public bound.
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
    /// The serialized report exceeded the immutable output-v10 byte limit.
    #[error("report exceeds the output-v10 limit of {limit} bytes")]
    ReportTooLarge {
        /// Immutable maximum accepted byte count.
        limit: u64,
    },
    /// The bounded bytes were not valid JSON for the output-v10 read shape.
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
    /// The bounded file record could not be decoded after the outer v10
    /// identity was accepted.
    #[error("has invalid output-v10 file shape: {reason}")]
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
    #[error("contains {found} checks, exceeding the output-v10 limit of {limit}")]
    TooManyChecks {
        /// Supplied check count.
        found: usize,
        /// Immutable output-v10 limit.
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
            measurements: wire.measurements,
            prediction_provenance: RequiredNullable::Missing,
            checks: None,
        });
    }

    if wire
        .checks
        .as_ref()
        .is_some_and(|checks| checks.len() > OUTPUT_V10_MAX_CHECKS_PER_FILE)
    {
        return Err(prediction_file_error(
            file_index,
            MeasurementFileError::TooManyChecks {
                found: wire.checks.as_ref().map_or(0, Vec::len),
                limit: OUTPUT_V10_MAX_CHECKS_PER_FILE,
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
            let provenance = decode_prediction_provenance_v1(raw.get()).map_err(|error| {
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
            })?;
            RequiredNullable::Present(Some(provenance))
        }
    };
    let mut decoded_facets = 0usize;
    let mut decoded_references = 0usize;
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
                let prediction = wire
                    .prediction
                    .map(|raw| {
                        decode_engine_prediction_v1(
                            raw.get(),
                            PREDICTION_V1_MAX_FACETS_PER_FILE.saturating_sub(decoded_facets),
                            PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE
                                .saturating_sub(decoded_references),
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
                    .validate(check_index, provenance_for_checks)
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
        measurements: wire.measurements,
        prediction_provenance,
        checks,
    })
}

fn validate_prediction_phase_file(
    command: &str,
    file_index: usize,
    file: &MeasurementFileInput,
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
            if checks.len() > OUTPUT_V10_MAX_CHECKS_PER_FILE {
                return Err(prediction_file_error(
                    file_index,
                    MeasurementFileError::TooManyChecks {
                        found: checks.len(),
                        limit: OUTPUT_V10_MAX_CHECKS_PER_FILE,
                    },
                ));
            }
            if let Some(provenance) = provenance {
                provenance.validate().map_err(|source| {
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
                .map(PredictionProvenanceV1::retained_text_bytes)
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
        provenance: Option<&PredictionProvenanceV1>,
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
            .flat_map(crate::prediction::EnginePredictionV1::facets)
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
                .is_some_and(crate::prediction::EnginePredictionV1::has_required_unavailable),
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

impl MeasurementReportInput {
    /// Read one report through the immutable output-v10 byte bound before
    /// UTF-8 or JSON parsing.
    ///
    /// The JSON parser receives at most [`OUTPUT_V10_MAX_REPORT_BYTES`] bytes
    /// and retains its recursion limit. This function never performs an
    /// unbounded `read_to_end` or constructs a generic JSON value.
    ///
    /// # Errors
    ///
    /// Returns a typed I/O, N+1 size, or JSON-shape error. Semantic contract
    /// validation remains in [`Self::into_files`].
    pub fn read_from(reader: impl Read) -> Result<Self, MeasurementReportReadError> {
        Self::read_from_with_limit(reader, OUTPUT_V10_MAX_REPORT_BYTES)
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
        match self.schema_version {
            Some(OUTPUT_SCHEMA_VERSION) => {}
            Some(found) => {
                return Err(MeasurementReportError::UnsupportedOutputVersion { found });
            }
            None => return Err(MeasurementReportError::MissingOutputVersion),
        }
        if self.schema.as_deref() != Some(OUTPUT_SCHEMA_ID) {
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
        validate_prediction_summary_presence(command, self.summary.as_ref())?;
        let files = self.files.ok_or(MeasurementReportError::MissingFiles)?;
        if files.len() > OUTPUT_V10_MAX_FILES {
            return Err(MeasurementReportError::TooManyFiles {
                found: files.len(),
                limit: OUTPUT_V10_MAX_FILES,
            });
        }
        let mut available = 0usize;
        let mut unavailable = 0usize;
        let mut decoded_files = Vec::with_capacity(files.len());
        for (file_index, raw) in files.into_iter().enumerate() {
            let file = decode_prediction_phase_file(command, file_index, &raw)?;
            let (file_available, file_unavailable) =
                validate_prediction_phase_file(command, file_index, &file)?;
            available = available
                .checked_add(file_available)
                .ok_or(MeasurementReportError::PredictionFacetSummaryMismatch)?;
            unavailable = unavailable
                .checked_add(file_unavailable)
                .ok_or(MeasurementReportError::PredictionFacetSummaryMismatch)?;
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
                let measurements = decode_measurement_payload(&measurements).map_err(|source| {
                    MeasurementReportError::file(
                        file_index,
                        MeasurementFileError::InvalidMeasurementsShape {
                            reason: source.to_string(),
                        },
                    )
                })?;
                match measurements.schema_version {
                    Some(MEASUREMENTS_SCHEMA_VERSION) => {}
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
                if measurements.schema.as_deref() != Some(MEASUREMENTS_SCHEMA_ID) {
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
                let measurements = MeasurementContract::new(clips, assets).map_err(|source| {
                    MeasurementReportError::file(
                        file_index,
                        MeasurementFileError::InvalidMeasurements { source },
                    )
                })?;
                Ok((
                    MeasurementReportFile {
                        path,
                        input: InputIdentity { sha256, bytes },
                        measurements,
                    },
                    file.checks.unwrap_or_default(),
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;

        // Measurement-dependent basis pointers are deliberately resolved only
        // after every file's complete measurements-v15 contract has passed.
        for (file_index, (file, checks)) in parsed.iter().enumerate() {
            validate_measurement_references_batch(
                &file.measurements,
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
        }
        Ok(parsed.into_iter().map(|(file, _)| file).collect())
    }
}

#[cfg(test)]
mod measurement_report_input_tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::engine_contract::{
        EngineFactIdV1, EngineFactStateV1, EngineFactValueV1, EnginePrimarySourceV1,
        EngineProfileFactV1, EngineProfileSelectionV1, ResolvedEngineProfileV1,
        ResolvedEngineSettingsV1,
    };
    use crate::evaluation::{CheckOutput, EvaluationScope, EvaluationScopeCode};
    use crate::measure::AssetMeasurements;
    use crate::prediction::{
        EnginePredictionBasisV1, EnginePredictionFacetV1, EnginePredictionV1,
        PredictionBasisReferenceV1, PredictionScalarV1, PredictionUnavailableReasonV1,
        RawSourceBindingV1,
    };
    use crate::source_facts::SourceFormatV1;
    use crate::{DependencyClosureV1, Document, ResolvedRoles};

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

    fn prediction_test_provenance() -> PredictionProvenanceV1 {
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
        let settings = ResolvedEngineSettingsV1::new(&profile, vec![], vec![]).unwrap();
        PredictionProvenanceV1::new(profile, SourceFormatV1::Glb, settings, raw, closure).unwrap()
    }

    fn prediction_test_measurements() -> MeasurementContract {
        MeasurementContract::new(BTreeMap::new(), AssetMeasurements::default()).unwrap()
    }

    fn prediction_test_rig() -> RigInfo {
        RigInfo::from_resolved(&Document::default(), &ResolvedRoles::default()).unwrap()
    }

    fn unavailable_facet(
        subject: String,
        basis: EnginePredictionBasisV1,
    ) -> EnginePredictionFacetV1 {
        EnginePredictionFacetV1::required_unavailable(
            EvaluationScope::new(EvaluationScopeCode::custom("test:prediction-limit"))
                .subject(subject),
            basis,
            vec![PredictionUnavailableReasonV1::ProjectIntentUnavailable],
        )
        .unwrap()
    }

    fn unavailable_check(
        check_id: &'static str,
        provenance: &PredictionProvenanceV1,
        facets: Vec<EnginePredictionFacetV1>,
    ) -> CheckEvaluation {
        let prediction = EnginePredictionV1::new(provenance.identity().clone(), facets).unwrap();
        CheckEvaluation::evaluated(
            check_id,
            CheckOutput::from_coverage(Vec::new(), Vec::new(), Vec::new())
                .with_engine_prediction(prediction),
        )
        .unwrap()
    }

    fn lint_file(
        provenance: &PredictionProvenanceV1,
        checks: Vec<CheckEvaluation>,
    ) -> Result<LintFileReport, OutputContractError> {
        LintFileReport::new(
            "limit.glb",
            provenance.raw_source().primary_input().clone(),
            prediction_test_rig(),
            Some(provenance.clone()),
            checks,
            prediction_test_measurements(),
        )
    }

    fn validated_lint_wire(
        provenance: &PredictionProvenanceV1,
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

    fn prediction_with_retained_text(
        provenance: &PredictionProvenanceV1,
        retained_text: usize,
    ) -> EnginePredictionV1 {
        const FIELD_ID_BYTES: usize = 16;
        const MAX_VALUE_BYTES: usize = crate::PREDICTION_V1_MAX_TEXT_BYTES;
        let fixed = "test:prediction-limit".len()
            + PredictionUnavailableReasonV1::ProjectIntentUnavailable
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
        let facet = EnginePredictionFacetV1::required_unavailable(
            EvaluationScope::new(EvaluationScopeCode::custom("test:prediction-limit")),
            basis,
            vec![PredictionUnavailableReasonV1::ProjectIntentUnavailable],
        )
        .unwrap();
        let prediction =
            EnginePredictionV1::new(provenance.identity().clone(), vec![facet]).unwrap();
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
        assert_eq!(
            lint_read_error(wire),
            MeasurementReportError::File {
                file_index: 0,
                source: MeasurementFileError::TooManyPredictionBasisReferences {
                    found: PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE + 1,
                    limit: PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE,
                },
            }
        );
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
                .with_engine_prediction(at_limit_prediction),
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
                .with_engine_prediction(above_limit_prediction),
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
            .expect("outer v10 shape remains valid")
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
                source: MeasurementFileError::InvalidPredictionProvenance {
                    source: PredictionContractError::InvalidSchema {
                        field: "provenance.schema",
                        ..
                    },
                },
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
        let facet = EnginePredictionFacetV1::required_unavailable(
            EvaluationScope::new(EvaluationScopeCode::custom("test:prediction")),
            basis,
            vec![PredictionUnavailableReasonV1::ProjectIntentUnavailable],
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
                source: MeasurementFileError::InvalidPrediction {
                    check_index: 0,
                    source: PredictionContractError::InvalidToken {
                        field: "facet scope code",
                        ..
                    },
                },
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
                        contract: "engine prediction basis v1",
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
        let facet = EnginePredictionFacetV1::required_unavailable(
            EvaluationScope::new(EvaluationScopeCode::custom("test:prediction")),
            basis,
            vec![PredictionUnavailableReasonV1::ProjectIntentUnavailable],
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
}

/// One source file and its measurement-command evidence.
#[derive(Debug, Clone, Serialize)]
pub struct MeasureFileReport {
    #[serde(flatten)]
    evidence: FileEvidence,
}

impl MeasureFileReport {
    /// Construct a measurement-command file report.
    pub fn new(
        path: impl Into<String>,
        input: InputIdentity,
        rig: RigInfo,
        measurements: MeasurementContract,
    ) -> Self {
        Self {
            evidence: FileEvidence::new(path, input, rig, measurements),
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
}

/// One source file and its lint-command evidence.
#[derive(Debug, Clone, Serialize)]
pub struct LintFileReport {
    #[serde(flatten)]
    evidence: FileEvidence,
    prediction_provenance: Option<PredictionProvenanceV1>,
    checks: Vec<CheckEvaluation>,
}

impl LintFileReport {
    /// Construct and validate a lint file report with one record per catalog check.
    ///
    /// # Errors
    ///
    /// Returns [`OutputContractError`] when the file exceeds an output-v10
    /// bound or prediction evidence does not bind to the same file/provenance.
    pub fn new(
        path: impl Into<String>,
        input: InputIdentity,
        rig: RigInfo,
        prediction_provenance: Option<PredictionProvenanceV1>,
        checks: Vec<CheckEvaluation>,
        measurements: MeasurementContract,
    ) -> Result<Self, OutputContractError> {
        let report = Self {
            evidence: FileEvidence::new(path, input, rig, measurements),
            prediction_provenance,
            checks,
        };
        report.validate()?;
        Ok(report)
    }

    /// Display path supplied by the producer.
    pub fn path(&self) -> &str {
        &self.evidence.path
    }

    /// Immutable identity of the source bytes used to produce this record.
    pub fn input(&self) -> &InputIdentity {
        &self.evidence.input
    }

    /// Check records in catalog order.
    pub fn checks(&self) -> &[CheckEvaluation] {
        &self.checks
    }

    /// File-scoped prediction provenance, or `None` for engine-neutral lint.
    pub const fn prediction_provenance(&self) -> Option<&PredictionProvenanceV1> {
        self.prediction_provenance.as_ref()
    }

    /// Nested measurement evidence.
    pub fn measurements(&self) -> &MeasurementContract {
        &self.evidence.measurements
    }

    fn validate(&self) -> Result<(), OutputContractError> {
        if self.checks.len() > OUTPUT_V10_MAX_CHECKS_PER_FILE {
            return Err(OutputContractError::TooManyChecks {
                found: self.checks.len(),
                limit: OUTPUT_V10_MAX_CHECKS_PER_FILE,
            });
        }
        if let Some(provenance) = &self.prediction_provenance {
            provenance.validate()?;
            if provenance.raw_source().primary_input() != &self.evidence.input {
                return Err(OutputContractError::PredictionPrimaryInputMismatch);
            }
        }

        let mut facets = 0usize;
        let mut references = 0usize;
        let mut text = self
            .prediction_provenance
            .as_ref()
            .map(PredictionProvenanceV1::retained_text_bytes)
            .transpose()?
            .unwrap_or(0);
        for check in &self.checks {
            let Some(prediction) = check.engine_prediction() else {
                continue;
            };
            let provenance = self
                .prediction_provenance
                .as_ref()
                .ok_or(OutputContractError::PredictionWithoutProvenance)?;
            prediction.validate_against_provenance(provenance)?;
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
        validate_measurement_references_batch(
            &self.evidence.measurements,
            self.checks
                .iter()
                .enumerate()
                .filter_map(|(check_index, check)| {
                    check
                        .engine_prediction()
                        .map(|prediction| (check_index, prediction))
                }),
        )
        .map_err(|error| OutputContractError::InvalidPrediction(error.source))?;
        if facets > PREDICTION_V1_MAX_FACETS_PER_FILE {
            return Err(OutputContractError::TooManyPredictionFacets {
                found: facets,
                limit: PREDICTION_V1_MAX_FACETS_PER_FILE,
            });
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
        Ok(())
    }
}

/// A producer attempted to construct output outside the immutable v10 contract.
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
    #[error("checked arithmetic overflow while validating output-v10 bounds")]
    ArithmeticOverflow,
    /// Nested prediction evidence violated its contract.
    #[error("invalid prediction evidence: {0}")]
    InvalidPrediction(#[from] PredictionContractError),
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
        if files.len() > OUTPUT_V10_MAX_FILES {
            return Err(OutputContractError::TooManyFiles {
                found: files.len(),
                limit: OUTPUT_V10_MAX_FILES,
            });
        }
        Ok(Self {
            header: EnvelopeHeader::new(tool, "measure"),
            summary: MeasureSummary { files: files.len() },
            files,
        })
    }
}

/// Current lint-command result envelope.
#[derive(Debug, Clone, Serialize)]
pub struct LintEnvelope {
    #[serde(flatten)]
    header: EnvelopeHeader,
    summary: LintSummary,
    files: Vec<LintFileReport>,
}

impl LintEnvelope {
    /// Construct a schema-valid lint envelope and derive its summary from the
    /// supplied check records.
    pub fn new(tool: ToolInfo, files: Vec<LintFileReport>) -> Result<Self, OutputContractError> {
        if files.len() > OUTPUT_V10_MAX_FILES {
            return Err(OutputContractError::TooManyFiles {
                found: files.len(),
                limit: OUTPUT_V10_MAX_FILES,
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
                if let Some(prediction) = check.engine_prediction() {
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
            header: EnvelopeHeader::new(tool, "lint"),
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
