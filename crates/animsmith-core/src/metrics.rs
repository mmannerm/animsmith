//! Locomotion clip metrics: loop-seam ratio, gait phase, root-motion
//! speed. Ported from a production game pipeline's reference
//! implementation
//! (verified there against Blender pose-matrix FK to <0.01×) — the
//! algorithms are kept semantically identical so the numbers reproduce.

use crate::model::{Clip, Document, Property, Track};
use crate::profile::{ResolvedRoles, Role};
use crate::sample::{PoseGrid, sample_clip};
use glam::Vec3;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

/// Below this per-frame foot move (metres), a clip has no real stride
/// (idle / block / stationary action) and the seam ratio would be a
/// divide-by-noise, so no ratio is reported.
pub const MIN_STRIDE_STEP_M: f64 = 0.02;

/// Lazily sampled metric pose grids for one document.
///
/// The check, measurement, and report pipelines all judge the same
/// uniform metric grid. Sharing this owner lets callers run checks and
/// then emit measurements or reports without sampling the same clip
/// twice.
///
/// The cache uses `Rc` and `RefCell`, so it is intentionally neither
/// `Send` nor `Sync`. Create one owner per document on each worker thread,
/// then share it by reference among consumers on that thread.
#[derive(Debug)]
pub struct MetricGrids<'a> {
    doc: &'a Document,
    grids: RefCell<BTreeMap<usize, Rc<PoseGrid>>>,
}

impl<'a> MetricGrids<'a> {
    /// Create a lazy metric-grid cache for `doc`.
    pub fn new(doc: &'a Document) -> Self {
        Self {
            doc,
            grids: RefCell::new(BTreeMap::new()),
        }
    }

    /// The document these grids sample.
    pub fn document(&self) -> &'a Document {
        self.doc
    }

    /// The metric pose grid for clip `clip_index`, computed once and
    /// shared. Returns `None` for an out-of-range index, non-positive
    /// duration, or fewer than three keys on the longest track.
    pub fn grid(&self, clip_index: usize) -> Option<Rc<PoseGrid>> {
        let clip = self.doc.clips.get(clip_index)?;
        let frames = metric_frame_count(clip)?;
        Some(
            self.grids
                .borrow_mut()
                .entry(clip_index)
                .or_insert_with(|| Rc::new(sample_clip(&self.doc.skeleton, clip, frames)))
                .clone(),
        )
    }
}

/// Foot-cycle metrics for one sampled clip.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct FootCycleMetrics {
    /// Wrap discontinuity of the feet (relative to hips) over the max of
    /// the two seam-adjacent in-clip steps. ≈1.0 for a clean cyclic
    /// loop; well above 1 for a seam pop. `None` when the clip has no
    /// real stride.
    pub loop_seam_ratio: Option<f64>,
    /// Cycle position `[0,1)` of the trough of the fundamental harmonic
    /// of the left-minus-right foot-height signal — a stride-phase
    /// anchor encoding handedness + cycle alignment. `None` when a side
    /// is missing.
    pub gait_phase: Option<f64>,
    /// Peak-to-peak swing of the L−R foot-height signal (metres); near
    /// zero means no detectable alternation and the phase is noise.
    pub lr_amplitude_m: f64,
}

/// Measure the foot cycle of a clip from its pose grid. Requires the
/// Hips role and at least one foot role; returns `None` otherwise (the
/// caller decides which typed coverage gap represents the missing metric).
///
/// The grid must span `[0, duration]` — the wrap pair is
/// `(last frame, frame 0)`. Grids under 3 frames carry no cycle.
///
/// # Panics
///
/// Panics if `roles` contains bone indices outside `grid`. Role
/// resolutions produced by this crate are tied to the same skeleton that
/// produced the grid; embedders that hand-build roles must preserve that
/// relationship.
pub fn foot_cycle_metrics(
    grid: &PoseGrid,
    roles: &ResolvedRoles,
    min_stride_step_m: f64,
) -> Option<FootCycleMetrics> {
    if grid.frame_count() < 3 {
        return None;
    }
    let hips = roles.get(Role::Hips)?;
    let left: Vec<usize> = [Role::LeftFoot, Role::LeftToe]
        .iter()
        .filter_map(|&r| roles.get(r))
        .collect();
    let right: Vec<usize> = [Role::RightFoot, Role::RightToe]
        .iter()
        .filter_map(|&r| roles.get(r))
        .collect();
    let feet: Vec<usize> = left.iter().chain(right.iter()).copied().collect();
    if feet.is_empty() {
        return None;
    }

    let frames = grid.frame_count();
    // Feet relative to hips: cancels the in-place root so we measure
    // the leg cycle, not body travel.
    let rel = |frame: usize, bone: usize| -> Vec3 {
        grid.model_position(frame, bone) - grid.model_position(frame, hips)
    };
    if (0..frames).any(|frame| {
        !grid.model_position(frame, hips).is_finite()
            || feet.iter().any(|&foot| !rel(frame, foot).is_finite())
    }) {
        return None;
    }

    // Loop seam: the wrap chord vs its NEIGHBOURING in-clip steps (the
    // step into the last frame and the step out of the first) — local
    // continuity, because stride speed varies legitimately inside a
    // cycle and the wrap may sit at an arbitrary cycle position. A real
    // pop is discontinuous against its immediate neighbours too.
    let max_foot_dist = |a: usize, b: usize| -> f64 {
        feet.iter()
            .map(|&f| (rel(a, f) - rel(b, f)).length() as f64)
            .fold(0.0, f64::max)
    };
    let seam = max_foot_dist(frames - 1, 0);
    let step_first = max_foot_dist(1, 0);
    let step_last = max_foot_dist(frames - 1, frames - 2);
    let neighbour_step = step_first.max(step_last);
    let loop_seam_ratio = if neighbour_step > 0.0 && neighbour_step >= min_stride_step_m {
        let ratio = seam / neighbour_step;
        ratio.is_finite().then_some(ratio)
    } else {
        None
    };

    // Gait phase: fundamental-harmonic trough of the L−R foot-height
    // signal over one cycle (the duplicate wrap frame excluded). The
    // difference cancels common-mode pelvis bob and encodes handedness
    // plus a stable cycle anchor.
    let cycle = if frames > 3 { frames - 1 } else { frames };
    let mut gait_phase = None;
    let mut lr_amplitude_m = 0.0f64;
    if !left.is_empty() && !right.is_empty() {
        let avg_height = |frame: usize, bones: &[usize]| -> f64 {
            bones.iter().map(|&b| rel(frame, b).y as f64).sum::<f64>() / bones.len() as f64
        };
        let diff: Vec<f64> = (0..cycle)
            .map(|f| avg_height(f, &left) - avg_height(f, &right))
            .collect();
        let max = diff.iter().copied().fold(f64::MIN, f64::max);
        let min = diff.iter().copied().fold(f64::MAX, f64::min);
        lr_amplitude_m = max - min;
        gait_phase = fundamental_trough_phase(&diff);
    }

    Some(FootCycleMetrics {
        loop_seam_ratio,
        gait_phase,
        lr_amplitude_m,
    })
}

/// Normalized cycle position `[0,1)` of the minimum of the signal's
/// first Fourier harmonic. Robust to plateaus and per-frame noise: the
/// minimum of `A·cos(2π·t/N − φ)` sits at `t/N = (φ/2π + 0.5) mod 1`.
pub fn fundamental_trough_phase(signal: &[f64]) -> Option<f64> {
    let n = signal.len();
    if n < 2 || signal.iter().any(|value| !value.is_finite()) {
        return None;
    }
    let mut re = 0.0f64;
    let mut im = 0.0f64;
    for (k, y) in signal.iter().enumerate() {
        let angle = std::f64::consts::TAU * k as f64 / n as f64;
        re += y * angle.cos();
        im += y * angle.sin();
    }
    let phi = im.atan2(re);
    let phase = (phi / std::f64::consts::TAU + 0.5).rem_euclid(1.0);
    phase.is_finite().then_some(phase)
}

/// Horizontal (XZ-plane) root displacement over the clip, divided by
/// duration. Uses the Root role, falling back to Hips (clips without a
/// dedicated root bone carry travel on the hips).
///
/// # Panics
///
/// Panics if the resolved Root or Hips bone id is outside `grid`.
pub fn root_motion_speed_mps(grid: &PoseGrid, roles: &ResolvedRoles) -> Option<f64> {
    let bone = roles.get(Role::Root).or_else(|| roles.get(Role::Hips))?;
    let frames = grid.frame_count();
    if frames < 2 {
        return None;
    }
    let duration = *grid.times.last()? as f64;
    if duration <= 0.0 {
        return None;
    }
    let a = grid.model_position(0, bone);
    let b = grid.model_position(frames - 1, bone);
    let dx = (b.x - a.x) as f64;
    let dz = (b.z - a.z) as f64;
    let speed = dx.hypot(dz) / duration;
    speed.is_finite().then_some(speed)
}

/// Maximum angular deviation (degrees) of a rotation track from its
/// first keyed rotation.
pub fn rotation_range_deg(track: &Track) -> Option<f64> {
    if track.property != Property::Rotation {
        return None;
    }
    let first = track.key_quat(0)?;
    if !first.is_finite() || first.length_squared() == 0.0 {
        return None;
    }
    let first = first.normalize();
    let mut max_deg = 0.0f64;
    for k in 1..track.key_count() {
        if let Some(q) = track.key_quat(k)
            && q.is_finite()
            && q.length_squared() > 0.0
        {
            let deg = first.angle_between(q.normalize()).to_degrees() as f64;
            if deg.is_finite() {
                max_deg = max_deg.max(deg);
            }
        }
    }
    Some(max_deg)
}

/// Maximum circular distance (in cycle fraction, `[0, 0.5]`) of a set of
/// normalized phases from their circular mean. Phases live on a ring, so
/// a naive max−min would over-report a cluster straddling the 0/1 wrap.
pub fn circular_phase_spread(phases: &[f64]) -> f64 {
    use std::f64::consts::{PI, TAU};
    let (mut sin_sum, mut cos_sum) = (0.0f64, 0.0f64);
    for p in phases {
        sin_sum += (p * TAU).sin();
        cos_sum += (p * TAU).cos();
    }
    let mean = sin_sum.atan2(cos_sum);
    let mut max_dev = 0.0f64;
    for p in phases {
        let mut d = (p * TAU - mean).abs() % TAU;
        if d > PI {
            d = TAU - d;
        }
        max_dev = max_dev.max(d / TAU);
    }
    max_dev
}

/// The metric sampling grid for a clip: uniform, resolution = max key
/// count (mirroring how the runtime loops a clip over `[0, duration]`,
/// wrapping duration→0 at render times unaligned with authored keys).
/// `None` for clips too short to carry a cycle (< 3 keys), matching the
/// reference implementation.
pub fn metric_frame_count(clip: &Clip) -> Option<usize> {
    let n = crate::sample::default_frame_count(clip);
    if clip.duration_s <= 0.0 || n < 3 {
        None
    } else {
        Some(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::CheckCtx;
    use crate::config::Config;
    use crate::measure::measure_document;
    use crate::model::{
        Bone, Clip, Document, Interpolation, Property, Skeleton, Track, TrackValues, Transform,
    };
    use crate::profile::{ResolvedRoles, Role};
    use glam::{Quat, Vec3};
    use std::rc::Rc;

    fn document_with_metric_clip() -> Document {
        Document {
            skeleton: Skeleton {
                bones: vec![Bone {
                    name: "root".into(),
                    parent: None,
                    rest: Transform::IDENTITY,
                    inverse_bind: None,
                }],
            },
            clips: vec![Clip {
                name: "walk".into(),
                duration_s: 1.0,
                tracks: vec![Track {
                    bone: 0,
                    property: Property::Rotation,
                    interpolation: Interpolation::Linear,
                    times: vec![0.0, 0.5, 1.0],
                    values: TrackValues::Quats(vec![
                        Quat::IDENTITY,
                        Quat::from_rotation_y(0.1),
                        Quat::from_rotation_y(0.2),
                    ]),
                }],
            }],
            ..Document::default()
        }
    }

    fn document_with_grid_inputs(duration_s: f64, times: Vec<f32>) -> Document {
        let values = vec![Quat::IDENTITY; times.len()];
        Document {
            skeleton: Skeleton {
                bones: vec![Bone {
                    name: "root".into(),
                    parent: None,
                    rest: Transform::IDENTITY,
                    inverse_bind: None,
                }],
            },
            clips: vec![Clip {
                name: "probe".into(),
                duration_s,
                tracks: vec![Track {
                    bone: 0,
                    property: Property::Rotation,
                    interpolation: Interpolation::Linear,
                    times,
                    values: TrackValues::Quats(values),
                }],
            }],
            ..Document::default()
        }
    }

    #[test]
    fn metric_grids_are_shared_by_checks_and_measurements() {
        let doc = document_with_metric_clip();
        let roles = ResolvedRoles::default();
        let config = Config::default();
        let grids = MetricGrids::new(&doc);

        let ctx = CheckCtx::new(&grids, &roles, &config);
        let from_ctx = ctx.grid(0).expect("metric grid");
        let from_owner = grids.grid(0).expect("same metric grid");
        assert!(Rc::ptr_eq(&from_ctx, &from_owner));

        let measurements = measure_document(&grids, &roles, &config);
        assert!(measurements.contains_key("walk"));
        let fresh_grids = MetricGrids::new(&doc);
        assert_eq!(
            serde_json::to_value(&measurements).expect("shared measurements serialize"),
            serde_json::to_value(measure_document(&fresh_grids, &roles, &config))
                .expect("plain measurements serialize")
        );
    }

    #[test]
    fn grid_returns_none_for_each_documented_invalid_request() {
        let valid = document_with_grid_inputs(1.0, vec![0.0, 0.5, 1.0]);
        let valid_grids = MetricGrids::new(&valid);
        assert!(valid_grids.grid(0).is_some());
        for clip_index in [1, 2, usize::MAX] {
            assert!(valid_grids.grid(clip_index).is_none());
        }

        for duration_s in [0.0, -1.0] {
            let non_positive = document_with_grid_inputs(duration_s, vec![0.0, 0.5, 1.0]);
            assert!(MetricGrids::new(&non_positive).grid(0).is_none());
        }

        for times in [vec![], vec![0.0], vec![0.0, 1.0]] {
            let too_few_keys = document_with_grid_inputs(1.0, times);
            assert!(MetricGrids::new(&too_few_keys).grid(0).is_none());
        }
    }

    #[test]
    fn grid_uses_longest_track_for_resolution() {
        // The first track is too short by itself; the later translation
        // track selects the grid's three-frame resolution.
        let mut doc = document_with_grid_inputs(1.0, vec![0.0, 1.0]);
        doc.clips[0].tracks.push(Track {
            bone: 0,
            property: Property::Translation,
            interpolation: Interpolation::Linear,
            times: vec![0.0, 0.5, 1.0],
            values: TrackValues::Vec3s(vec![Vec3::ZERO, Vec3::X, 2.0 * Vec3::X]),
        });

        let grid = MetricGrids::new(&doc)
            .grid(0)
            .expect("later longest track supplies a metric grid");
        assert_eq!(grid.frame_count(), 3);
    }

    #[test]
    fn foot_metrics_reject_finite_positions_whose_relative_subtraction_overflows() {
        let mut doc = document_with_metric_clip();
        doc.skeleton.bones = vec![
            Bone {
                name: "hips".into(),
                parent: None,
                rest: Transform {
                    translation: Vec3::splat(-f32::MAX),
                    ..Transform::IDENTITY
                },
                inverse_bind: None,
            },
            Bone {
                name: "left".into(),
                parent: None,
                rest: Transform {
                    translation: Vec3::splat(f32::MAX),
                    ..Transform::IDENTITY
                },
                inverse_bind: None,
            },
        ];
        doc.clips[0].tracks[0].bone = 0;
        let roles = ResolvedRoles::from_names(
            &doc.skeleton,
            [
                (Role::Hips, "hips".to_string()),
                (Role::LeftFoot, "left".to_string()),
            ],
        );
        let grid = MetricGrids::new(&doc).grid(0).expect("metric grid");

        assert!(grid.model_position(0, 0).is_finite());
        assert!(grid.model_position(0, 1).is_finite());
        assert!(foot_cycle_metrics(&grid, &roles, MIN_STRIDE_STEP_M).is_none());
    }
}
