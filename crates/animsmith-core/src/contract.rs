//! Versioned JSON result-contract types shared by CLI and embedded producers.
//!
//! The CLI is one producer of these envelopes. Embedded pipelines can use the
//! same constructors and immutable protocol identities without duplicating the
//! wire shape or hard-coding URNs.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::diff::MetricDelta;
use crate::evaluation::{
    Applicability, CheckEvaluation, ConfigurationState, EvaluationState, SelectionState,
};
use crate::measure::{Aabb, AssetMeasurements, ClipMeasurements};
use crate::profile::ResolvedRoles;
use crate::{Document, Severity};

/// Current outer result-envelope version.
pub const OUTPUT_SCHEMA_VERSION: u32 = 2;
/// Immutable identity of the current outer result envelope.
pub const OUTPUT_SCHEMA_ID: &str = "urn:animsmith:schema:output:2";
/// Current nested measurement-contract version.
pub const MEASUREMENTS_SCHEMA_VERSION: u32 = 3;
/// Immutable identity of the current nested measurement contract.
pub const MEASUREMENTS_SCHEMA_ID: &str = "urn:animsmith:schema:measurements:3";

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
    /// envelope constructed through this API remains within output v2.
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
    for (clip_name, clip) in clips {
        finite(clip.duration_s, format!("clips[{clip_name:?}].duration_s"))?;
        for (bone, value) in &clip.bone_rotation_range_deg {
            finite(
                *value,
                format!("clips[{clip_name:?}].bone_rotation_range_deg[{bone:?}]"),
            )?;
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
    Ok(())
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
    measurements: Option<MeasurementPayloadInput>,
}

#[derive(Debug, Deserialize)]
struct MeasurementPayloadInput {
    schema_version: Option<u32>,
    schema: Option<String>,
    clips: Option<BTreeMap<String, ClipMeasurements>>,
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
    measurements: MeasurementContract,
}

impl MeasurementReportFile {
    /// Source path recorded by the producing report.
    pub fn path(&self) -> &str {
        &self.path
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
    /// The outer envelope does not carry the immutable v2 identity.
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
                Ok(MeasurementReportFile { path, measurements })
            })
            .collect()
    }
}

#[cfg(test)]
mod measurement_report_input_tests {
    use super::*;

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
                    measurements: Some(MeasurementPayloadInput {
                        schema_version: Some(MEASUREMENTS_SCHEMA_VERSION),
                        schema: Some(MEASUREMENTS_SCHEMA_ID.into()),
                        clips: Some(clips),
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
            bone_rotation_range_deg: BTreeMap::new(),
            loop_seam_ratio: None,
            gait: None,
            speed_mps: None,
        };
        let invalid_mesh = || crate::measure::MeshDefinitionMeasurements {
            mesh_index: 0,
            name: "mesh".into(),
            vertex_count: 1,
            geometry_aabb: None,
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
    rig: RigInfo,
    measurements: MeasurementContract,
}

impl FileEvidence {
    fn new(path: impl Into<String>, rig: RigInfo, measurements: MeasurementContract) -> Self {
        Self {
            path: path.into(),
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
    pub fn new(path: impl Into<String>, rig: RigInfo, measurements: MeasurementContract) -> Self {
        Self {
            evidence: FileEvidence::new(path, rig, measurements),
        }
    }

    /// Display path supplied by the producer.
    pub fn path(&self) -> &str {
        &self.evidence.path
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
        rig: RigInfo,
        checks: Vec<CheckEvaluation>,
        measurements: MeasurementContract,
    ) -> Self {
        Self {
            evidence: FileEvidence::new(path, rig, measurements),
            checks,
        }
    }

    /// Display path supplied by the producer.
    pub fn path(&self) -> &str {
        &self.evidence.path
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
