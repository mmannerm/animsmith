//! Locomotion clip metrics: loop-seam ratio, gait phase, root-motion
//! speed. Ported from the rauta project's `locomotion_metrics.py`
//! (verified there against Blender pose-matrix FK to <0.01×) — the
//! algorithms are kept semantically identical so the numbers reproduce.

use crate::model::Clip;
use crate::profile::{ResolvedRoles, Role};
use crate::sample::PoseGrid;
use glam::Vec3;

/// Below this per-frame foot move (metres), a clip has no real stride
/// (idle / block / stationary action) and the seam ratio would be a
/// divide-by-noise, so no ratio is reported.
pub const MIN_STRIDE_STEP_M: f64 = 0.02;

/// Foot-cycle metrics for one sampled clip.
#[derive(Debug, Clone, PartialEq)]
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
/// caller decides whether that's a skip-note or nothing).
///
/// The grid must span `[0, duration]` — the wrap pair is
/// `(last frame, frame 0)`. Grids under 3 frames carry no cycle.
pub fn foot_cycle_metrics(grid: &PoseGrid, roles: &ResolvedRoles) -> Option<FootCycleMetrics> {
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
    let loop_seam_ratio = if neighbour_step >= MIN_STRIDE_STEP_M {
        Some(seam / neighbour_step)
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
    if n < 2 {
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
    Some((phi / std::f64::consts::TAU + 0.5).rem_euclid(1.0))
}

/// Horizontal (XZ-plane) root displacement over the clip, divided by
/// duration. Uses the Root role, falling back to Hips (clips without a
/// dedicated root bone carry travel on the hips).
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
    Some(dx.hypot(dz) / duration)
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
