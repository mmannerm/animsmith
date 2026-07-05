//! Measurements: the raw per-clip metric map that `measure` emits and
//! `lint` judges. Kept separate from findings so pipelines (e.g. a
//! bake's measured sidecar) can pin their own contracts to the numbers.

use crate::config::Config;
use crate::metrics::{foot_cycle_metrics, metric_frame_count, root_motion_speed_mps};
use crate::model::{Document, Property, SceneAssets};
use crate::profile::ResolvedRoles;
use crate::sample::sample_clip;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Rotation ranges below this are not recorded (matches the incubating
/// pipeline's convention).
pub const MIN_RECORDED_ROTATION_DEG: f64 = 0.1;

/// Axis-aligned bounding box of a mesh's positions, in scene units
/// (metres, Y-up — the converted space every loader hands over).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Aabb {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

/// Static (animation-independent) measurements of one mesh carried in
/// [`SceneAssets`]. Emitted by `measure` when the input carried geometry
/// (both the FBX and glTF loaders fill `SceneAssets`). Vertex data is
/// read as authored — indexed meshes count their unique vertices,
/// unindexed meshes count every triangle corner.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MeshMeasurements {
    pub name: String,
    /// Total position count across the mesh's primitives.
    pub vertex_count: u32,
    /// Bounding box over every primitive position; `None` for a mesh
    /// with no positions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aabb: Option<Aabb>,
    /// Highest number of non-zero skin influences on any single vertex
    /// (`0` for an unskinned mesh).
    pub max_joints_per_vertex: u32,
    /// Min/max of the per-vertex skin-weight sums (≈1.0 for a
    /// well-formed skin); `None` for an unskinned mesh.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight_sum_min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight_sum_max: Option<f64>,
}

/// Measure every mesh in the document's [`SceneAssets`], in document
/// order. Returns an empty vector when no geometry was loaded (the
/// lint/inspect path and asset-less files), so callers that don't carry
/// assets emit nothing extra. Never panics on hostile geometry: NaN
/// positions are ignored by the min/max reduction and flow to the `nan`
/// check instead.
pub fn measure_meshes(assets: &SceneAssets) -> Vec<MeshMeasurements> {
    assets
        .meshes
        .iter()
        .map(|mesh| {
            let mut vertex_count = 0u32;
            let mut min = [f32::INFINITY; 3];
            let mut max = [f32::NEG_INFINITY; 3];
            let mut any_position = false;
            let mut max_joints_per_vertex = 0u32;
            let mut weight_sum_min = f64::INFINITY;
            let mut weight_sum_max = f64::NEG_INFINITY;
            let mut any_weight = false;

            for prim in &mesh.primitives {
                vertex_count = vertex_count.saturating_add(prim.positions.len() as u32);
                for p in &prim.positions {
                    any_position = true;
                    let a = p.to_array();
                    for i in 0..3 {
                        // f32::min/max return the non-NaN operand, so a
                        // NaN coordinate can't poison the box.
                        min[i] = min[i].min(a[i]);
                        max[i] = max[i].max(a[i]);
                    }
                }
                for w in &prim.weights {
                    any_weight = true;
                    let influences = w.iter().filter(|&&x| x > 0.0).count() as u32;
                    max_joints_per_vertex = max_joints_per_vertex.max(influences);
                    let sum: f64 = w.iter().map(|&x| x as f64).sum();
                    weight_sum_min = weight_sum_min.min(sum);
                    weight_sum_max = weight_sum_max.max(sum);
                }
            }

            MeshMeasurements {
                name: mesh.name.clone(),
                vertex_count,
                aabb: any_position.then_some(Aabb { min, max }),
                max_joints_per_vertex,
                weight_sum_min: any_weight.then_some(weight_sum_min),
                weight_sum_max: any_weight.then_some(weight_sum_max),
            }
        })
        .collect()
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ClipMeasurements {
    pub duration_s: f64,
    /// Keyframe count of the longest channel.
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

/// Measure every clip in the document. Role-dependent metrics
/// (loop seam, gait, root-motion speed) are present only where the
/// roles resolve; pass an empty [`ResolvedRoles`] to skip them.
pub fn measure_document(
    doc: &Document,
    roles: &ResolvedRoles,
    config: &Config,
) -> BTreeMap<String, ClipMeasurements> {
    let min_stride_step_m = config.loop_seam_min_stride_step_m();
    doc.clips
        .iter()
        .map(|clip| {
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

                if track.property == Property::Rotation
                    && let Some(first) = track.key_quat(0)
                    && first.is_finite()
                    && first.length_squared() > 0.0
                {
                    let first = first.normalize();
                    let mut max_deg = 0.0f64;
                    for k in 1..track.key_count() {
                        if let Some(q) = track.key_quat(k)
                            && q.is_finite()
                            && q.length_squared() > 0.0
                        {
                            let deg = first.angle_between(q.normalize()).to_degrees() as f64;
                            max_deg = max_deg.max(deg);
                        }
                    }
                    if max_deg >= MIN_RECORDED_ROTATION_DEG {
                        let entry = rotation_range.entry(bone.name.clone()).or_insert(0.0);
                        *entry = entry.max(max_deg);
                    }
                }
            }

            let grid =
                metric_frame_count(clip).map(|frames| sample_clip(&doc.skeleton, clip, frames));
            let cycle = grid
                .as_ref()
                .and_then(|g| foot_cycle_metrics(g, roles, min_stride_step_m));
            let speed_mps = grid.as_ref().and_then(|g| root_motion_speed_mps(g, roles));

            (
                clip.name.clone(),
                ClipMeasurements {
                    duration_s: clip.duration_s,
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
    use crate::model::{MeshAsset, Primitive};
    use glam::Vec3;

    fn mesh(name: &str, primitives: Vec<Primitive>) -> SceneAssets {
        SceneAssets {
            meshes: vec![MeshAsset {
                name: name.into(),
                node: 0,
                primitives,
                skin_joints: vec![],
                skin_ibms: vec![],
            }],
            materials: vec![],
        }
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
        let m = &measure_meshes(&mesh("body", vec![prim]))[0];

        assert_eq!(m.name, "body");
        assert_eq!(m.vertex_count, 4);
        let aabb = m.aabb.as_ref().expect("positions present");
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
        let m = &measure_meshes(&mesh("prop", vec![prim]))[0];

        assert_eq!(m.vertex_count, 2);
        assert_eq!(m.aabb.as_ref().unwrap().min, [-1.0, -2.0, -3.0]);
        assert_eq!(m.max_joints_per_vertex, 0);
        assert_eq!(m.weight_sum_min, None, "no skin ⇒ no weight-sum");
        assert_eq!(m.weight_sum_max, None);
    }

    #[test]
    fn empty_mesh_reports_no_bbox() {
        let m = &measure_meshes(&mesh("hollow", vec![Primitive::default()]))[0];
        assert_eq!(m.vertex_count, 0);
        assert!(m.aabb.is_none(), "no positions ⇒ no bounding box");
    }

    #[test]
    fn nan_position_does_not_poison_the_bbox() {
        // A hostile NaN coordinate must not crash or corrupt the box —
        // f32::min/max drop it, keeping the finite extent.
        let prim = Primitive {
            positions: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(f32::NAN, 5.0, 0.0),
                Vec3::new(2.0, 0.0, 0.0),
            ],
            ..Primitive::default()
        };
        let m = &measure_meshes(&mesh("nan", vec![prim]))[0];
        let aabb = m.aabb.as_ref().unwrap();
        assert_eq!(aabb.min[0], 0.0);
        assert_eq!(aabb.max[0], 2.0, "NaN ignored, real extent kept");
        assert_eq!(aabb.max[1], 5.0);
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
        let m = &measure_meshes(&mesh("multi", vec![a, b]))[0];
        assert_eq!(m.vertex_count, 8, "3 + 5 corners across two primitives");
    }
}
