//! Versioned JSON result-contract types shared by CLI and embedded producers.
//!
//! The CLI is one producer of these envelopes. Embedded pipelines can use the
//! same constructors and immutable protocol identities without duplicating the
//! wire shape or hard-coding URNs.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use glam::Mat4;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::diff::MetricDelta;
use crate::evaluation::{
    Applicability, CheckEvaluation, ConfigurationState, EvaluationState, SelectionState,
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
use crate::profile::ResolvedRoles;
use crate::{Document, Severity};

/// Current outer result-envelope version.
pub const OUTPUT_SCHEMA_VERSION: u32 = 9;
/// Immutable identity of the current outer result envelope.
pub const OUTPUT_SCHEMA_ID: &str = "urn:animsmith:schema:output:9";
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
    /// envelope constructed through this API remains within output v9.
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
    let mut hex = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
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
/// measurement contract. Unknown fields remain forward-compatible, while all
/// protocol identities and command constraints are validated by
/// [`MeasurementReportInput::into_files`].
#[derive(Debug, Deserialize)]
pub struct MeasurementReportInput {
    schema_version: Option<u32>,
    schema: Option<String>,
    command: Option<String>,
    files: Option<Vec<MeasurementFileInput>>,
}

#[derive(Debug, Deserialize)]
struct MeasurementFileInput {
    path: Option<String>,
    input: Option<InputIdentityInput>,
    measurements: Option<MeasurementPayloadInput>,
}

#[derive(Debug, Deserialize)]
struct InputIdentityInput {
    sha256: Option<String>,
    bytes: Option<u64>,
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
    /// The outer envelope omitted its file array.
    #[error("report envelope has no `files` array")]
    MissingFiles,
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

/// One measurement-report file record failed validation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum MeasurementFileError {
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

impl MeasurementReportInput {
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
        match self.command.as_deref() {
            Some("measure" | "lint") => {}
            Some(command) => {
                return Err(MeasurementReportError::UnsupportedCommand {
                    command: command.to_owned(),
                });
            }
            None => return Err(MeasurementReportError::MissingCommand),
        }
        let files = self.files.ok_or(MeasurementReportError::MissingFiles)?;
        files
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
                Ok(MeasurementReportFile {
                    path,
                    input: InputIdentity { sha256, bytes },
                    measurements,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod measurement_report_input_tests {
    use super::*;

    #[test]
    fn v11_nested_version_is_rejected_before_current_shape_decode() {
        let report: MeasurementReportInput = serde_json::from_value(serde_json::json!({
            "schema_version": OUTPUT_SCHEMA_VERSION,
            "schema": OUTPUT_SCHEMA_ID,
            "command": "measure",
            "files": [{
                "path": "measurements-v11.json",
                "input": { "sha256": "0".repeat(64), "bytes": 0 },
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
    fn recovered_payloads_run_measurement_contract_validation() {
        // Exercise the last-resort contract guard with private NaN inputs that
        // JSON cannot encode. Public-boundary tests separately cover finite
        // deserializer values that overflow while narrowing into f32 mesh
        // bounds. Together they prove no input route can bypass
        // MeasurementContract::new.
        let file =
            |path: &str,
             clips: BTreeMap<String, ClipMeasurements>,
             mesh_definitions: Vec<crate::measure::MeshDefinitionMeasurements>| {
                MeasurementFileInput {
                    path: Some(path.into()),
                    input: Some(InputIdentityInput {
                        sha256: Some("0".repeat(64)),
                        bytes: Some(0),
                    }),
                    measurements: Some(MeasurementPayloadInput {
                        schema_version: Some(MEASUREMENTS_SCHEMA_VERSION),
                        schema: Some(MEASUREMENTS_SCHEMA_ID.into()),
                        clips: Some(clips),
                        material_resource_coverage: Some(MaterialResourceCoverage::Unavailable),
                        material_definitions: Some(Vec::new()),
                        textures: Some(Vec::new()),
                        images: Some(Vec::new()),
                        skeleton_source_coverage: Some(SourceSkeletonCoverage::Unavailable),
                        skeleton_nodes: Some(Vec::new()),
                        skins: Some(Vec::new()),
                        mesh_definitions: Some(mesh_definitions),
                        node_instances: Some(Vec::new()),
                        scenes: Some(Vec::new()),
                        default_scene_index: None,
                    }),
                }
            };
        let report = |files| MeasurementReportInput {
            schema_version: Some(OUTPUT_SCHEMA_VERSION),
            schema: Some(OUTPUT_SCHEMA_ID.into()),
            command: Some("measure".into()),
            files: Some(files),
        };
        let invalid_clip = || ClipMeasurements {
            duration_s: f64::NAN,
            frame_count: 1,
            animated_bones: Vec::new(),
            bone_channels: Vec::new(),
            bone_rotation_range_deg: BTreeMap::new(),
            loop_continuity: None,
            loop_continuity_availability: MeasurementAvailability::NotApplicable,
            loop_endpoint_mode: None,
            loop_endpoint_mode_availability: MeasurementAvailability::NotApplicable,
            frame_grid: None,
            frame_grid_availability: MeasurementAvailability::NotApplicable,
            loop_seam_ratio: None,
            loop_seam_ratio_availability: MeasurementAvailability::NotApplicable,
            gait: None,
            gait_availability: MeasurementAvailability::NotApplicable,
            root_trajectory: None,
            root_trajectory_availability: MeasurementAvailability::NotApplicable,
            speed_mps: None,
            speed_mps_availability: MeasurementAvailability::NotApplicable,
        };
        let invalid_mesh = || crate::measure::MeshDefinitionMeasurements {
            mesh_index: 0,
            name: "mesh".into(),
            vertex_count: 1,
            geometry_aabb: None,
            geometry_centroid: None,
            max_joints_per_vertex: 1,
            weight_sum_min: Some(f64::NAN),
            weight_sum_max: Some(1.0),
            additional_influence_sets: Vec::new(),
        };
        let valid = || file("valid.glb", BTreeMap::new(), Vec::new());
        let cases = [
            (
                report(vec![file(
                    "invalid-clip.glb",
                    BTreeMap::from([("walk".into(), invalid_clip())]),
                    Vec::new(),
                )]),
                MeasurementReportError::File {
                    file_index: 0,
                    source: MeasurementFileError::InvalidMeasurements {
                        source: MeasurementContractError::NonFiniteValue {
                            path: "clips[\"walk\"].duration_s".into(),
                        },
                    },
                },
                "files[0] has invalid measurements: measurement value clips[\"walk\"].duration_s must be finite",
                0,
            ),
            (
                report(vec![file(
                    "invalid-mesh.glb",
                    BTreeMap::new(),
                    vec![invalid_mesh()],
                )]),
                MeasurementReportError::File {
                    file_index: 0,
                    source: MeasurementFileError::InvalidMeasurements {
                        source: MeasurementContractError::NonFiniteValue {
                            path: "mesh_definitions[0].weight_sum_min".into(),
                        },
                    },
                },
                "files[0] has invalid measurements: measurement value mesh_definitions[0].weight_sum_min must be finite",
                0,
            ),
            (
                report(vec![
                    valid(),
                    file(
                        "invalid-clip.glb",
                        BTreeMap::from([("walk".into(), invalid_clip())]),
                        Vec::new(),
                    ),
                ]),
                MeasurementReportError::File {
                    file_index: 1,
                    source: MeasurementFileError::InvalidMeasurements {
                        source: MeasurementContractError::NonFiniteValue {
                            path: "clips[\"walk\"].duration_s".into(),
                        },
                    },
                },
                "files[1] has invalid measurements: measurement value clips[\"walk\"].duration_s must be finite",
                1,
            ),
            (
                report(vec![
                    valid(),
                    file("invalid-mesh.glb", BTreeMap::new(), vec![invalid_mesh()]),
                ]),
                MeasurementReportError::File {
                    file_index: 1,
                    source: MeasurementFileError::InvalidMeasurements {
                        source: MeasurementContractError::NonFiniteValue {
                            path: "mesh_definitions[0].weight_sum_min".into(),
                        },
                    },
                },
                "files[1] has invalid measurements: measurement value mesh_definitions[0].weight_sum_min must be finite",
                1,
            ),
        ];

        for (input, expected, expected_display, expected_file_index) in cases {
            let error = input
                .into_files()
                .expect_err("recovered evidence must be validated");
            assert_eq!(error, expected);
            assert_eq!(error.file_index(), Some(expected_file_index));
            assert_eq!(error.to_string(), expected_display);
        }
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
    checks: Vec<CheckEvaluation>,
}

impl LintFileReport {
    /// Construct a lint file report with one record per catalog check.
    pub fn new(
        path: impl Into<String>,
        input: InputIdentity,
        rig: RigInfo,
        checks: Vec<CheckEvaluation>,
        measurements: MeasurementContract,
    ) -> Self {
        Self {
            evidence: FileEvidence::new(path, input, rig, measurements),
            checks,
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

    /// Check records in catalog order.
    pub fn checks(&self) -> &[CheckEvaluation] {
        &self.checks
    }

    /// Nested measurement evidence.
    pub fn measurements(&self) -> &MeasurementContract {
        &self.evidence.measurements
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
    pub fn new(tool: ToolInfo, files: Vec<MeasureFileReport>) -> Self {
        Self {
            header: EnvelopeHeader::new(tool, "measure"),
            summary: MeasureSummary { files: files.len() },
            files,
        }
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
    pub fn new(tool: ToolInfo, files: Vec<LintFileReport>) -> Self {
        let mut findings = FindingSummary::default();
        let mut checks = CheckSummary::default();
        for file in &files {
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
            }
        }
        Self {
            header: EnvelopeHeader::new(tool, "lint"),
            summary: LintSummary {
                files: files.len(),
                findings,
                checks,
            },
            files,
        }
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
