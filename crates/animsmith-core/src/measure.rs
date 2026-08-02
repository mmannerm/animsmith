//! Measurements: the raw per-clip metric map that `measure` emits and
//! `lint` judges. Kept separate from findings so pipelines (e.g. a
//! bake's measured sidecar) can pin their own contracts to the numbers.

use crate::config::Config;
use crate::metrics::{MetricGrids, foot_cycle_metrics, root_motion_speed_mps, rotation_range_deg};
use crate::model::{Document, MeshAsset};
use crate::profile::ResolvedRoles;
use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Rotation ranges below this are not recorded (matches the incubating
/// pipeline's convention).
pub const MIN_RECORDED_ROTATION_DEG: f64 = 0.1;

/// Axis-aligned bounding box of a mesh's positions, in scene units
/// (metres, Y-up — the converted space every loader hands over).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Aabb {
    /// Minimum XYZ corner.
    pub min: [f32; 3],
    /// Maximum XYZ corner.
    pub max: [f32; 3],
}

/// Static base-geometry measurements of one source mesh definition.
///
/// Vertex data is read as authored: indexed meshes count their unique
/// vertices, while unindexed meshes count every triangle corner. The geometry
/// AABB is in the definition's primitive coordinate system and excludes node
/// transforms, morph targets, skinning, animation, and runtime placement.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MeshDefinitionMeasurements {
    /// Stable index of the mesh definition in the source format.
    pub mesh_index: usize,
    /// Mesh name.
    pub name: String,
    /// Total position count across the mesh's primitives.
    pub vertex_count: u32,
    /// Bounding box over every finite base `POSITION`; `None` when none exist.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry_aabb: Option<Aabb>,
    /// Highest number of non-zero skin influences on any single vertex
    /// (`0` for an unskinned mesh).
    pub max_joints_per_vertex: u32,
    /// Min/max of the per-vertex skin-weight sums (≈1.0 for a
    /// well-formed skin); `None` for an unskinned mesh.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight_sum_min: Option<f64>,
    /// Maximum finite per-vertex skin-weight sum; `None` for an
    /// unskinned mesh.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight_sum_max: Option<f64>,
}

/// Why a static node-instance AABB could not be emitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum StaticNodeAabbUnavailableReason {
    /// The referenced definition has no finite base positions.
    NoFinitePositions,
    /// The instance is skinned, whose node transform is not a static rendered
    /// bound under glTF semantics; bind-pose skinning is a separate domain.
    SkinnedDeformationExcluded,
    /// The default/rest world transform or a transformed point was non-finite.
    NonFiniteTransform,
}

/// Static base-geometry bounds for one mesh-bearing source node.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct NodeInstanceMeasurements {
    /// Stable index of the mesh-bearing node in the source format.
    pub node_index: usize,
    /// Node name for display; identity comes from [`Self::node_index`].
    pub node_name: String,
    /// Stable source mesh-definition index referenced by this node.
    pub mesh_index: usize,
    /// Tight AABB after applying the node's default/rest world transform to
    /// every finite base position. Deformation and runtime placement are
    /// excluded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub static_node_world_aabb: Option<Aabb>,
    /// Present exactly when [`Self::static_node_world_aabb`] is unavailable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub static_node_world_aabb_unavailable_reason: Option<StaticNodeAabbUnavailableReason>,
}

/// Static aggregate bounds for one declared source scene.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SceneMeasurements {
    /// Stable index of the scene in the source format.
    pub scene_index: usize,
    /// Authored scene name, when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Number of mesh-bearing node instances reachable from the scene roots.
    pub instance_count: usize,
    /// Union of every available static node-instance AABB in this scene.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub static_scene_world_aabb: Option<Aabb>,
    /// Reachable instances excluded because their static node AABB was
    /// unavailable. A non-zero value means the scene aggregate is partial.
    pub excluded_instance_count: usize,
}

/// Static scene-asset evidence nested beside clip measurements.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AssetMeasurements {
    /// Source mesh definitions, including definitions with no node instance.
    pub mesh_definitions: Vec<MeshDefinitionMeasurements>,
    /// Mesh-bearing source nodes, including nodes outside every scene.
    pub node_instances: Vec<NodeInstanceMeasurements>,
    /// Every declared source scene in source order.
    pub scenes: Vec<SceneMeasurements>,
    /// Stable source scene index selected as the default, when declared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_scene_index: Option<usize>,
}

#[derive(Debug, Clone, Copy)]
struct Bounds {
    min: [f32; 3],
    max: [f32; 3],
    any: bool,
}

impl Default for Bounds {
    fn default() -> Self {
        Self {
            min: [f32::INFINITY; 3],
            max: [f32::NEG_INFINITY; 3],
            any: false,
        }
    }
}

impl Bounds {
    fn include(&mut self, point: Vec3) -> bool {
        let point = point.to_array();
        if !point.iter().all(|value| value.is_finite()) {
            return false;
        }
        self.any = true;
        for ((min, max), value) in self.min.iter_mut().zip(&mut self.max).zip(point) {
            *min = min.min(value);
            *max = max.max(value);
        }
        true
    }

    fn include_aabb(&mut self, aabb: Aabb) {
        self.any = true;
        for ((min, max), (aabb_min, aabb_max)) in self
            .min
            .iter_mut()
            .zip(&mut self.max)
            .zip(aabb.min.into_iter().zip(aabb.max))
        {
            *min = min.min(aabb_min);
            *max = max.max(aabb_max);
        }
    }

    fn finish(self) -> Option<Aabb> {
        self.any.then_some(Aabb {
            min: self.min,
            max: self.max,
        })
    }
}

fn measure_mesh_definition(mesh: &MeshAsset) -> MeshDefinitionMeasurements {
    let mut vertex_count = 0u32;
    let mut bounds = Bounds::default();
    let mut max_joints_per_vertex = 0u32;
    let mut weight_sum_min = f64::INFINITY;
    let mut weight_sum_max = f64::NEG_INFINITY;
    let mut any_finite_weight = false;

    for primitive in &mesh.primitives {
        vertex_count = vertex_count.saturating_add(primitive.positions.len() as u32);
        for &position in &primitive.positions {
            // Non-finite geometry remains visible to the `nan` check but must
            // never leak a JSON-invalid bound.
            bounds.include(position);
        }
        for weights in &primitive.weights {
            let influences = weights.iter().filter(|&&weight| weight > 0.0).count() as u32;
            max_joints_per_vertex = max_joints_per_vertex.max(influences);
            let sum: f64 = weights.iter().map(|&weight| f64::from(weight)).sum();
            if sum.is_finite() {
                any_finite_weight = true;
                weight_sum_min = weight_sum_min.min(sum);
                weight_sum_max = weight_sum_max.max(sum);
            }
        }
    }

    MeshDefinitionMeasurements {
        mesh_index: mesh.source_mesh_index,
        name: mesh.name.clone(),
        vertex_count,
        geometry_aabb: bounds.finish(),
        max_joints_per_vertex,
        weight_sum_min: any_finite_weight.then_some(weight_sum_min),
        weight_sum_max: any_finite_weight.then_some(weight_sum_max),
    }
}

fn matrix_is_finite(matrix: Mat4) -> bool {
    matrix
        .to_cols_array()
        .into_iter()
        .all(|component| component.is_finite())
}

fn static_world_matrices(doc: &Document) -> Vec<Option<Mat4>> {
    let mut worlds = Vec::with_capacity(doc.skeleton.bones.len());
    for bone in &doc.skeleton.bones {
        let local = bone.rest.to_mat4();
        let world = match bone.parent {
            Some(parent) => worlds
                .get(parent)
                .copied()
                .flatten()
                .map(|parent_world| parent_world * local),
            None => Some(local),
        }
        .filter(|matrix| matrix_is_finite(*matrix));
        worlds.push(world);
    }
    worlds
}

fn transformed_definition_aabb(
    mesh: &MeshAsset,
    world: Mat4,
) -> Result<Aabb, StaticNodeAabbUnavailableReason> {
    let mut bounds = Bounds::default();
    let mut any_finite_source = false;
    for primitive in &mesh.primitives {
        for &position in &primitive.positions {
            if !position.is_finite() {
                continue;
            }
            any_finite_source = true;
            if !bounds.include(world.transform_point3(position)) {
                return Err(StaticNodeAabbUnavailableReason::NonFiniteTransform);
            }
        }
    }
    if !any_finite_source {
        return Err(StaticNodeAabbUnavailableReason::NoFinitePositions);
    }
    bounds
        .finish()
        .ok_or(StaticNodeAabbUnavailableReason::NonFiniteTransform)
}

#[derive(Debug, Clone, Copy, Default)]
struct NodeAggregate {
    bounds: Bounds,
    instance_count: usize,
    excluded_instance_count: usize,
}

impl NodeAggregate {
    fn include(&mut self, other: Self) {
        if let Some(aabb) = other.bounds.finish() {
            self.bounds.include_aabb(aabb);
        }
        self.instance_count = self.instance_count.saturating_add(other.instance_count);
        self.excluded_instance_count = self
            .excluded_instance_count
            .saturating_add(other.excluded_instance_count);
    }
}

/// Measure source mesh definitions, their default/rest node instances, and
/// every declared scene without sampling animation or deformation.
///
/// World transforms are composed once in parent-before-child order. Scene
/// aggregates are then derived from reverse-order subtree summaries, so a file
/// with many scenes cannot force work proportional to scenes × all nodes.
/// Non-finite geometry never reaches JSON; non-finite effective transforms and
/// skinned instances are represented by typed unavailability reasons.
pub fn measure_assets(doc: &Document) -> AssetMeasurements {
    let mesh_definitions = doc
        .assets
        .meshes
        .iter()
        .map(measure_mesh_definition)
        .collect::<Vec<_>>();
    let worlds = static_world_matrices(doc);
    let mut node_aggregates = vec![NodeAggregate::default(); doc.skeleton.bones.len()];
    let mut node_instances = Vec::with_capacity(doc.assets.instances.len());

    for instance in &doc.assets.instances {
        let Some(mesh) = doc.assets.meshes.get(instance.mesh) else {
            continue;
        };
        let bounds = if !instance.skin_joints.is_empty() {
            Err(StaticNodeAabbUnavailableReason::SkinnedDeformationExcluded)
        } else {
            match worlds.get(instance.node).copied().flatten() {
                Some(world) => transformed_definition_aabb(mesh, world),
                None => Err(StaticNodeAabbUnavailableReason::NonFiniteTransform),
            }
        };
        let (static_node_world_aabb, unavailable) = match bounds {
            Ok(aabb) => (Some(aabb), None),
            Err(reason) => (None, Some(reason)),
        };
        let node_name = doc
            .skeleton
            .bones
            .get(instance.node)
            .map(|bone| bone.name.clone())
            .unwrap_or_else(|| format!("node-{}", instance.source_node_index));
        let measurement = NodeInstanceMeasurements {
            node_index: instance.source_node_index,
            node_name,
            mesh_index: mesh.source_mesh_index,
            static_node_world_aabb,
            static_node_world_aabb_unavailable_reason: unavailable,
        };
        if let Some(aggregate) = node_aggregates.get_mut(instance.node) {
            aggregate.instance_count = aggregate.instance_count.saturating_add(1);
            match measurement.static_node_world_aabb {
                Some(aabb) => aggregate.bounds.include_aabb(aabb),
                None => {
                    aggregate.excluded_instance_count =
                        aggregate.excluded_instance_count.saturating_add(1);
                }
            }
        }
        node_instances.push(measurement);
    }

    // Parent-before-child is a loader invariant. Walking in reverse lets each
    // subtree contribute once to its parent and then to any scene that names
    // the root, avoiding a scenes × nodes traversal.
    for node in (0..doc.skeleton.bones.len()).rev() {
        let Some(parent) = doc.skeleton.bones[node].parent else {
            continue;
        };
        let child = node_aggregates[node];
        if let Some(parent_aggregate) = node_aggregates.get_mut(parent) {
            parent_aggregate.include(child);
        }
    }

    let scenes = doc
        .assets
        .scenes
        .iter()
        .map(|scene| {
            let mut aggregate = NodeAggregate::default();
            for &root in &scene.roots {
                if let Some(root_aggregate) = node_aggregates.get(root).copied() {
                    aggregate.include(root_aggregate);
                }
            }
            SceneMeasurements {
                scene_index: scene.source_scene_index,
                name: scene.name.clone(),
                instance_count: aggregate.instance_count,
                static_scene_world_aabb: aggregate.bounds.finish(),
                excluded_instance_count: aggregate.excluded_instance_count,
            }
        })
        .collect();

    AssetMeasurements {
        mesh_definitions,
        node_instances,
        scenes,
        default_scene_index: doc.assets.default_scene,
    }
}

/// Role-dependent gait metrics for one clip.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct GaitMeasurement {
    /// Stride-anchor phase in `[0,1)`; see
    /// [`crate::metrics::FootCycleMetrics::gait_phase`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<f64>,
    /// Peak-to-peak L−R foot-height swing (metres).
    pub lr_amplitude_m: f64,
}

/// Measurements for one clip in the `measure` output map.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ClipMeasurements {
    /// Clip duration in seconds.
    pub duration_s: f64,
    /// Keyframe count of the longest channel. This also selects the uniform
    /// metric-grid resolution, but it is not an authored frame-rate value.
    pub frame_count: u32,
    /// Bones with at least one keyframed channel, sorted.
    pub animated_bones: Vec<String>,
    /// Max rotation deviation (degrees) of each bone from its first
    /// keyed rotation. Bones under [`MIN_RECORDED_ROTATION_DEG`] are
    /// omitted.
    pub bone_rotation_range_deg: BTreeMap<String, f64>,
    /// Loop wrap discontinuity ratio; needs hips + foot roles and a
    /// real stride. See [`crate::metrics::FootCycleMetrics`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loop_seam_ratio: Option<f64>,
    /// Gait stride anchor; needs a left and a right foot role.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gait: Option<GaitMeasurement>,
    /// Horizontal root displacement ÷ duration (m/s); needs the Root
    /// (or Hips) role.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed_mps: Option<f64>,
}

/// Measure every clip using shared metric pose grids. Role-dependent
/// metrics (loop seam, gait, root-motion speed) are present only where
/// the roles resolve; pass an empty [`ResolvedRoles`] to skip them.
///
/// This returns clip measurements only. Call [`measure_assets`] separately
/// when the pipeline also needs static scene measurements. Clip names are map
/// keys and therefore must be unique; a later duplicate replaces an earlier
/// entry.
pub fn measure_document(
    grids: &MetricGrids<'_>,
    roles: &ResolvedRoles,
    config: &Config,
) -> BTreeMap<String, ClipMeasurements> {
    let doc = grids.document();
    let min_stride_step_m = config.loop_seam_min_stride_step_m();
    doc.clips
        .iter()
        .enumerate()
        .map(|(clip_index, clip)| {
            let mut animated: BTreeSet<String> = BTreeSet::new();
            let mut rotation_range: BTreeMap<String, f64> = BTreeMap::new();
            let mut frame_count = 0usize;

            for track in &clip.tracks {
                let Some(bone) = doc.skeleton.bones.get(track.bone) else {
                    continue;
                };
                if track.key_count() == 0 {
                    continue;
                }
                animated.insert(bone.name.clone());
                frame_count = frame_count.max(track.key_count());

                if let Some(max_deg) = rotation_range_deg(track)
                    && max_deg >= MIN_RECORDED_ROTATION_DEG
                {
                    let entry = rotation_range.entry(bone.name.clone()).or_insert(0.0);
                    *entry = entry.max(max_deg);
                }
            }

            let grid = grids.grid(clip_index);
            let cycle = grid
                .as_ref()
                .and_then(|g| foot_cycle_metrics(g, roles, min_stride_step_m));
            let speed_mps = grid.as_ref().and_then(|g| root_motion_speed_mps(g, roles));
            let duration_s = if clip.duration_s.is_finite() {
                clip.duration_s
            } else {
                clip.tracks
                    .iter()
                    .flat_map(|track| track.times.iter().copied())
                    .filter(|time| time.is_finite())
                    .map(f64::from)
                    .fold(0.0, f64::max)
            };

            (
                clip.name.clone(),
                ClipMeasurements {
                    duration_s,
                    frame_count: frame_count as u32,
                    animated_bones: animated.into_iter().collect(),
                    bone_rotation_range_deg: rotation_range,
                    loop_seam_ratio: cycle.as_ref().and_then(|c| c.loop_seam_ratio),
                    gait: cycle.map(|c| GaitMeasurement {
                        phase: c.gait_phase,
                        lr_amplitude_m: c.lr_amplitude_m,
                    }),
                    speed_mps,
                },
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        Bone, Clip, Document, Interpolation, MeshAsset, Primitive, Property, SceneAssets, Skeleton,
        Track, TrackValues, Transform,
    };
    use crate::profile::Role;
    use glam::{Quat, Vec3};

    fn mesh(name: &str, primitives: Vec<Primitive>) -> MeshDefinitionMeasurements {
        let doc = Document {
            assets: SceneAssets {
                meshes: vec![MeshAsset {
                    name: name.into(),
                    source_mesh_index: 0,
                    primitives,
                }],
                ..SceneAssets::default()
            },
            ..Document::default()
        };
        measure_assets(&doc).mesh_definitions.remove(0)
    }

    #[test]
    fn skinned_mesh_measures_bbox_joints_and_weight_sums() {
        // Four positions with an analytic AABB of (0,0,0)..(2,3,4).
        let prim = Primitive {
            positions: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(2.0, 0.0, 0.0),
                Vec3::new(0.0, 3.0, 0.0),
                Vec3::new(0.0, 0.0, 4.0),
            ],
            // Influence counts 1, 2, 3, 3 → max 3; weight sums 1.0, 1.0,
            // 1.0, 0.9 → min 0.9, max 1.0.
            weights: vec![
                [1.0, 0.0, 0.0, 0.0],
                [0.5, 0.5, 0.0, 0.0],
                [0.4, 0.3, 0.3, 0.0],
                [0.3, 0.3, 0.3, 0.0],
            ],
            joints: vec![[0, 0, 0, 0]; 4],
            ..Primitive::default()
        };
        let m = mesh("body", vec![prim]);

        assert_eq!(m.name, "body");
        assert_eq!(m.vertex_count, 4);
        let aabb = m.geometry_aabb.as_ref().expect("positions present");
        assert_eq!(aabb.min, [0.0, 0.0, 0.0]);
        assert_eq!(aabb.max, [2.0, 3.0, 4.0]);
        assert_eq!(m.max_joints_per_vertex, 3);
        // f32 weights summed in f64 carry rounding; compare with tolerance.
        assert!((m.weight_sum_min.unwrap() - 0.9).abs() < 1e-6);
        assert!((m.weight_sum_max.unwrap() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn unskinned_mesh_has_bbox_but_no_weight_stats() {
        let prim = Primitive {
            positions: vec![Vec3::new(-1.0, -2.0, -3.0), Vec3::new(1.0, 2.0, 3.0)],
            ..Primitive::default()
        };
        let m = mesh("prop", vec![prim]);

        assert_eq!(m.vertex_count, 2);
        assert_eq!(m.geometry_aabb.as_ref().unwrap().min, [-1.0, -2.0, -3.0]);
        assert_eq!(m.max_joints_per_vertex, 0);
        assert_eq!(m.weight_sum_min, None, "no skin ⇒ no weight-sum");
        assert_eq!(m.weight_sum_max, None);
    }

    #[test]
    fn empty_mesh_reports_no_bbox() {
        let m = mesh("hollow", vec![Primitive::default()]);
        assert_eq!(m.vertex_count, 0);
        assert!(m.geometry_aabb.is_none(), "no positions ⇒ no bounding box");
    }

    #[test]
    fn non_finite_position_is_dropped_from_the_bbox() {
        // A vertex with any non-finite coordinate is garbage geometry:
        // it is dropped whole (not folded per-axis), so the box stays
        // the finite extent — and never emits a non-finite bound.
        let prim = Primitive {
            positions: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(f32::NAN, 5.0, 0.0),
                Vec3::new(f32::INFINITY, 9.0, 0.0),
                Vec3::new(2.0, 3.0, 0.0),
            ],
            ..Primitive::default()
        };
        let m = mesh("nan", vec![prim]);
        let aabb = m.geometry_aabb.as_ref().unwrap();
        // Only the two finite vertices contribute; the NaN/Inf rows drop
        // out, so their 5.0 / 9.0 do NOT reach the box.
        assert_eq!(aabb.min, [0.0, 0.0, 0.0]);
        assert_eq!(aabb.max, [2.0, 3.0, 0.0]);
        assert!(
            aabb.min.iter().chain(&aabb.max).all(|c| c.is_finite()),
            "no non-finite bound is ever emitted"
        );
    }

    #[test]
    fn all_non_finite_positions_yield_no_bbox() {
        // Every vertex non-finite ⇒ no finite contribution ⇒ `aabb` is
        // omitted, not an inf/-inf box that serializes to JSON `null`.
        let prim = Primitive {
            positions: vec![Vec3::splat(f32::NAN), Vec3::splat(f32::INFINITY)],
            ..Primitive::default()
        };
        let m = mesh("allnan", vec![prim]);
        assert_eq!(m.vertex_count, 2, "count still reflects the vertices");
        assert!(
            m.geometry_aabb.is_none(),
            "no finite vertex ⇒ no box (never null bounds)"
        );
    }

    #[test]
    fn non_finite_weight_sum_is_omitted() {
        // A NaN weight makes its sum non-finite; it must not surface as a
        // JSON-null weight-sum bound.
        let prim = Primitive {
            positions: vec![Vec3::ZERO, Vec3::ONE],
            weights: vec![[0.5, 0.5, 0.0, 0.0], [f32::NAN, 0.0, 0.0, 0.0]],
            ..Primitive::default()
        };
        let m = mesh("nanw", vec![prim]);
        // The one finite sum (1.0) is kept; the NaN sum is skipped.
        assert_eq!(m.weight_sum_min, Some(1.0));
        assert_eq!(m.weight_sum_max, Some(1.0));
    }

    #[test]
    fn all_non_finite_weight_sums_yield_no_weight_stats() {
        // Every weight sum non-finite ⇒ no finite contribution ⇒ both
        // bounds omitted, not an inf/-inf pair that serializes to `null`.
        let prim = Primitive {
            positions: vec![Vec3::ZERO, Vec3::ONE],
            weights: vec![[f32::NAN, 0.0, 0.0, 0.0], [f32::INFINITY, 0.0, 0.0, 0.0]],
            ..Primitive::default()
        };
        let m = mesh("allnanw", vec![prim]);
        assert_eq!(m.weight_sum_min, None, "no finite weight sum ⇒ omitted");
        assert_eq!(m.weight_sum_max, None);
        // max_joints_per_vertex still counts the non-zero influences.
        assert_eq!(m.max_joints_per_vertex, 1);
    }

    #[test]
    fn vertex_count_sums_across_primitives() {
        let a = Primitive {
            positions: vec![Vec3::ZERO; 3],
            ..Primitive::default()
        };
        let b = Primitive {
            positions: vec![Vec3::ONE; 5],
            ..Primitive::default()
        };
        let m = mesh("multi", vec![a, b]);
        assert_eq!(m.vertex_count, 8, "3 + 5 corners across two primitives");
    }

    #[test]
    fn non_finite_instance_transform_makes_scene_coverage_partial() {
        let doc = Document {
            skeleton: Skeleton {
                bones: vec![
                    Bone {
                        name: "finite".into(),
                        parent: None,
                        rest: Transform::IDENTITY,
                        inverse_bind: None,
                    },
                    Bone {
                        name: "overflow".into(),
                        parent: Some(0),
                        rest: Transform {
                            scale: Vec3::splat(f32::MAX),
                            ..Transform::IDENTITY
                        },
                        inverse_bind: None,
                    },
                ],
            },
            assets: SceneAssets {
                meshes: vec![MeshAsset {
                    name: "point".into(),
                    source_mesh_index: 4,
                    primitives: vec![Primitive {
                        positions: vec![Vec3::new(2.0, 0.0, 0.0)],
                        ..Primitive::default()
                    }],
                }],
                instances: vec![
                    crate::model::MeshInstance {
                        source_node_index: 10,
                        node: 0,
                        mesh: 0,
                        ..crate::model::MeshInstance::default()
                    },
                    crate::model::MeshInstance {
                        source_node_index: 11,
                        node: 1,
                        mesh: 0,
                        ..crate::model::MeshInstance::default()
                    },
                ],
                scenes: vec![crate::model::SceneAsset {
                    source_scene_index: 3,
                    name: Some("partial".into()),
                    roots: vec![0],
                }],
                default_scene: None,
                ..SceneAssets::default()
            },
            ..Document::default()
        };

        let measured = measure_assets(&doc);
        assert_eq!(measured.default_scene_index, None, "no implicit scene zero");
        assert_eq!(measured.node_instances.len(), 2);
        assert_eq!(
            measured.node_instances[0].static_node_world_aabb,
            Some(Aabb {
                min: [2.0, 0.0, 0.0],
                max: [2.0, 0.0, 0.0],
            })
        );
        assert_eq!(
            measured.node_instances[1].static_node_world_aabb_unavailable_reason,
            Some(StaticNodeAabbUnavailableReason::NonFiniteTransform)
        );
        assert_eq!(measured.scenes[0].instance_count, 2);
        assert_eq!(measured.scenes[0].excluded_instance_count, 1);
        assert_eq!(
            measured.scenes[0].static_scene_world_aabb,
            measured.node_instances[0].static_node_world_aabb,
            "partial aggregate retains the finite instance"
        );
    }

    #[test]
    fn later_duplicate_clip_name_replaces_earlier_measurement() {
        let earlier = Clip {
            name: "duplicate".into(),
            duration_s: 1.0,
            tracks: vec![
                Track {
                    bone: 0,
                    property: Property::Rotation,
                    interpolation: Interpolation::Linear,
                    times: vec![0.0, 0.5, 1.0],
                    values: TrackValues::Quats(vec![
                        Quat::IDENTITY,
                        Quat::from_rotation_x(0.25),
                        Quat::from_rotation_x(0.5),
                    ]),
                },
                Track {
                    bone: 0,
                    property: Property::Translation,
                    interpolation: Interpolation::Linear,
                    times: vec![0.0, 0.5, 1.0],
                    values: TrackValues::Vec3s(vec![Vec3::ZERO, Vec3::Z * 0.5, Vec3::Z]),
                },
                Track {
                    bone: 1,
                    property: Property::Translation,
                    interpolation: Interpolation::Linear,
                    times: vec![0.0, 0.5, 1.0],
                    values: TrackValues::Vec3s(vec![
                        Vec3::new(-0.1, -1.0, 0.0),
                        Vec3::new(-0.1, -0.9, 0.15),
                        Vec3::new(-0.1, -1.0, 0.0),
                    ]),
                },
                Track {
                    bone: 2,
                    property: Property::Translation,
                    interpolation: Interpolation::Linear,
                    times: vec![0.0, 0.5, 1.0],
                    values: TrackValues::Vec3s(vec![
                        Vec3::new(0.1, -1.0, 0.0),
                        Vec3::new(0.1, -1.1, -0.15),
                        Vec3::new(0.1, -1.0, 0.0),
                    ]),
                },
            ],
        };
        let later = Clip {
            name: "duplicate".into(),
            duration_s: 2.0,
            tracks: vec![Track {
                bone: 0,
                property: Property::Translation,
                interpolation: Interpolation::Linear,
                times: vec![0.0, 2.0],
                values: TrackValues::Vec3s(vec![Vec3::ZERO, Vec3::X]),
            }],
        };
        let skeleton = Skeleton {
            bones: vec![
                Bone {
                    name: "hips".into(),
                    parent: None,
                    rest: Transform::IDENTITY,
                    inverse_bind: None,
                },
                Bone {
                    name: "left_foot".into(),
                    parent: Some(0),
                    rest: Transform::IDENTITY,
                    inverse_bind: None,
                },
                Bone {
                    name: "right_foot".into(),
                    parent: Some(0),
                    rest: Transform::IDENTITY,
                    inverse_bind: None,
                },
            ],
        };
        let roles = ResolvedRoles::from_names(
            &skeleton,
            [
                (Role::Hips, "hips".into()),
                (Role::LeftFoot, "left_foot".into()),
                (Role::RightFoot, "right_foot".into()),
            ],
        );
        let earlier_doc = Document {
            skeleton: skeleton.clone(),
            clips: vec![earlier.clone()],
            ..Document::default()
        };
        let earlier_grids = MetricGrids::new(&earlier_doc);
        let earlier_measurement =
            &measure_document(&earlier_grids, &roles, &Config::default())["duplicate"];
        assert!(earlier_measurement.loop_seam_ratio.is_some());
        assert!(earlier_measurement.gait.is_some());
        assert!(earlier_measurement.speed_mps.is_some());

        let doc = Document {
            skeleton,
            clips: vec![earlier, later],
            ..Document::default()
        };
        let grids = MetricGrids::new(&doc);
        let measurements = measure_document(&grids, &roles, &Config::default());

        assert_eq!(
            serde_json::to_value(measurements).expect("duplicate measurements serialize"),
            serde_json::json!({
                "duplicate": {
                    "duration_s": 2.0,
                    "frame_count": 2,
                    "animated_bones": ["hips"],
                    "bone_rotation_range_deg": {},
                }
            })
        );
    }
}
