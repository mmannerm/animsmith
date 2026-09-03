//! Locomotion clip metrics: loop-seam ratio, gait phase, root-motion
//! speed, and sampled root trajectory. The loop-seam, gait, and speed
//! metrics were ported from a production game pipeline's reference
//! implementation
//! (verified there against Blender pose-matrix FK to <0.01×) — the
//! algorithms are kept semantically identical so the numbers reproduce.

use crate::model::{Clip, Document, Property, Track};
use crate::profile::{ResolvedRoles, Role};
use crate::sample::{PoseGrid, sample_clip};
use glam::{DQuat, DVec3, Quat, Vec3};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

/// Below this per-frame foot move (metres), a clip has no real stride
/// (idle / block / stationary action) and the seam ratio would be a
/// divide-by-noise, so no ratio is reported.
pub const MIN_STRIDE_STEP_M: f64 = 0.02;

/// Half-turn tolerance used to refuse direction-ambiguous adjacent yaw steps
/// and to preserve the unwrapped sign when canonicalizing multi-step endpoint
/// results near ±180 degrees.
pub const ROOT_YAW_HALF_TURN_AMBIGUITY_DEG: f64 = 1.0e-4;

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
    /// real stride (see [`Self::has_real_stride`]), or when a real
    /// stride exists but `seam / neighbour_step` is not finite. The
    /// whole-clip position finiteness gate above only bounds individual
    /// coordinates, not the `f32` squared-length arithmetic used to turn
    /// two positions into a distance; a per-axis delta near `f32::MAX`
    /// overflows that squaring to infinity even though every input
    /// position was finite. That is the only known route to this case —
    /// it requires magnitudes far outside any real animation.
    pub loop_seam_ratio: Option<f64>,
    /// Whether the seam-adjacent neighbour step met the configured
    /// minimum stride threshold, i.e. whether the clip has a real
    /// stride to normalize the seam against. `false` means
    /// [`Self::loop_seam_ratio`]'s absence is an expected "no subject"
    /// (a planted/idle clip), not a derivation failure.
    pub has_real_stride: bool,
    /// Cycle position `[0,1)` of the trough of the fundamental harmonic of the
    /// left-minus-right foot-height signal — a stride-phase anchor encoding
    /// handedness + cycle alignment. `None` when a side is missing, the
    /// sampled signal has exact zero peak-to-peak swing, or the harmonic fit
    /// fails.
    pub gait_phase: Option<f64>,
    /// Peak-to-peak swing of the L−R foot-height signal (metres); near
    /// zero means no detectable alternation and the phase is noise.
    pub lr_amplitude_m: f64,
}

/// Why a clip does or does not carry a stride anchor.
///
/// This is the vocabulary the gait checks report coverage in, and the one a
/// presentation reads to say why a member of a declared group has no anchor.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum GaitPhaseOutcome {
    /// One side's foot roles did not resolve, so there is no left-minus-right
    /// signal to fit a phase to.
    MissingBilateralFootRoles,
    /// The left-minus-right signal has exactly zero peak-to-peak swing, so
    /// its phase would be noise rather than a stride.
    NoFootHeightSwing,
    /// The stride anchor, in cycle fraction `[0, 1)`.
    Measured(f64),
    /// The harmonic fit yielded no finite phase.
    Unavailable,
}

impl FootCycleMetrics {
    /// Why this clip does or does not carry a stride anchor.
    pub fn gait_phase_outcome(&self, roles: &ResolvedRoles) -> GaitPhaseOutcome {
        GaitPhaseOutcome::classify(
            self.gait_phase,
            self.lr_amplitude_m,
            bilateral_foot_roles(roles),
        )
    }
}

/// Whether both sides resolve a foot or toe role, which is what a
/// left-minus-right signal needs.
fn bilateral_foot_roles(roles: &ResolvedRoles) -> bool {
    let side = |foot, toe| roles.get(foot).is_some() || roles.get(toe).is_some();
    side(Role::LeftFoot, Role::LeftToe) && side(Role::RightFoot, Role::RightToe)
}

/// Whether the rig resolves the roles any gait measurement needs: hips plus
/// at least one foot. This is the prerequisite the gait checks report as a
/// coverage gap when it fails, and the one a presentation states as the
/// reason no member of a group could be measured.
pub fn gait_roles_resolved(roles: &ResolvedRoles) -> bool {
    let has_foot = [
        Role::LeftFoot,
        Role::LeftToe,
        Role::RightFoot,
        Role::RightToe,
    ]
    .iter()
    .any(|&role| roles.get(role).is_some());
    roles.get(Role::Hips).is_some() && has_foot
}

impl GaitPhaseOutcome {
    fn classify(gait_phase: Option<f64>, lr_amplitude_m: f64, has_bilateral_roles: bool) -> Self {
        match gait_phase {
            _ if !has_bilateral_roles => Self::MissingBilateralFootRoles,
            _ if lr_amplitude_m == 0.0 => Self::NoFootHeightSwing,
            Some(phase) => Self::Measured(phase),
            None => Self::Unavailable,
        }
    }
}

/// Model-space loop-continuity measurements for one skeleton bone.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct BoneLoopContinuityMetrics {
    /// Last-sample to first-sample model-space position distance (metres).
    pub position_delta_m: f64,
    /// Shortest-path model-space rotation difference (degrees).
    pub rotation_delta_deg: f64,
    /// Difference between the model-space linear velocities immediately
    /// before and after the wrap (metres per second).
    pub seam_velocity_delta_mps: f64,
    /// Difference between the model-space angular velocities immediately
    /// before and after the wrap (degrees per second).
    pub seam_angular_velocity_delta_degps: f64,
}

/// Sampled model-space translation and yaw for a selected Root/Hips bone.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct RootTrajectoryMetrics {
    /// Translation facts when every selected-bone position is finite.
    pub translation: Option<RootTranslationMetrics>,
    /// Yaw facts when a fixed, deterministic horizontal heading witness remains
    /// usable across the complete sampled trajectory.
    pub yaw: Option<RootYawMetrics>,
}

/// Sampled model-space translation facts for a selected Root/Hips bone.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct RootTranslationMetrics {
    /// Endpoint displacement along canonical model-space +X, in metres.
    pub horizontal_displacement_x_m: f64,
    /// Endpoint displacement along canonical model-space +Z, in metres.
    pub horizontal_displacement_z_m: f64,
    /// Sum of sampled model-space XZ step lengths, in metres.
    pub horizontal_travel_m: f64,
    /// Signed endpoint displacement along canonical model-space +Y, in metres.
    pub vertical_displacement_m: f64,
    /// Minimum signed +Y displacement from the initial sample, in metres.
    pub vertical_min_displacement_m: f64,
    /// Maximum signed +Y displacement from the initial sample, in metres.
    pub vertical_max_displacement_m: f64,
}

/// Signed sampled yaw facts for a selected Root/Hips bone.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct RootYawMetrics {
    /// Fixed local basis axis used as the horizontal heading witness.
    pub heading_axis: RootYawHeadingAxis,
    /// Shortest signed endpoint-equivalent yaw in `[-180, 180]` degrees.
    /// An exact half turn retains the sign of [`Self::unwrapped_yaw_deg`].
    pub net_yaw_deg: f64,
    /// Signed first-to-last heading change after deterministic wrap crossing
    /// unwrapping. Unlike endpoint orientation alone, a sampled full turn is
    /// retained as approximately `+360` or `-360` degrees.
    pub unwrapped_yaw_deg: f64,
    /// Sum of absolute sampled unwrapped heading steps. This retains reversing
    /// yaw motion that cancels in [`Self::unwrapped_yaw_deg`].
    pub yaw_travel_deg: f64,
}

/// One local orientation witness retained for a complete model-space yaw
/// measurement. Order is policy: conventional `+Z` wins an exact tie,
/// followed by the common Z-up-source `+Y` convention and finally `+X`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RootYawHeadingAxis {
    /// Positive local Z axis.
    PositiveZ,
    /// Positive local Y axis.
    PositiveY,
    /// Positive local X axis.
    PositiveX,
}

impl RootYawHeadingAxis {
    /// Stable display label for the selected local heading witness.
    pub const fn label(self) -> &'static str {
        match self {
            Self::PositiveZ => "+Z",
            Self::PositiveY => "+Y",
            Self::PositiveX => "+X",
        }
    }
}

/// Model-space horizontal `(x, z)` projection of one local unit basis axis.
pub(crate) fn horizontal_heading(rotation: DQuat, axis: RootYawHeadingAxis) -> (f64, f64) {
    let local_axis = match axis {
        RootYawHeadingAxis::PositiveZ => DVec3::Z,
        RootYawHeadingAxis::PositiveY => DVec3::Y,
        RootYawHeadingAxis::PositiveX => DVec3::X,
    };
    let heading = rotation.mul_vec3(local_axis);
    (heading.x, heading.z)
}

/// Select the best-conditioned local heading witness at sample zero. Exact
/// ties retain the declared `+Z`, `+Y`, `+X` priority because replacement is
/// strict rather than `>=`.
pub(crate) fn select_horizontal_heading_axis(rotation: DQuat) -> RootYawHeadingAxis {
    let mut selected = RootYawHeadingAxis::PositiveZ;
    let (x, z) = horizontal_heading(rotation, selected);
    let mut best_length = x.hypot(z);
    for candidate in [RootYawHeadingAxis::PositiveY, RootYawHeadingAxis::PositiveX] {
        let (x, z) = horizontal_heading(rotation, candidate);
        let length = x.hypot(z);
        if length > best_length {
            selected = candidate;
            best_length = length;
        }
    }
    selected
}

/// Convert signed unwrapped yaw to its canonical endpoint-equivalent value.
/// Endpoint-equivalent results within
/// [`ROOT_YAW_HALF_TURN_AMBIGUITY_DEG`] of a half turn retain the sign of the
/// unwrapped result so analytic `+180` and `-180` trajectories remain
/// distinguishable despite binary32 quaternion roundoff.
pub(crate) fn canonical_net_yaw_deg(unwrapped_yaw_deg: f64) -> f64 {
    let mut net = (unwrapped_yaw_deg + 180.0).rem_euclid(360.0) - 180.0;
    if (net.abs() - 180.0).abs() <= ROOT_YAW_HALF_TURN_AMBIGUITY_DEG {
        net = 180.0f64.copysign(unwrapped_yaw_deg);
    }
    net
}

/// Measure model-space translation and sampled yaw for `bone`.
///
/// AnimSmith's metric domain is right-handed, +Y-up metres; horizontal
/// displacement is therefore the signed X/Z endpoint vector and vertical
/// evidence is signed +Y displacement from sample zero. The existing uniform
/// [`PoseGrid`] is authoritative. Yaw chooses at sample zero whichever local
/// `+Z`, `+Y`, or `+X` axis has the greatest horizontal projection, retains
/// that witness for every sample, and unwraps crossings at ±180 degrees.
/// Positive yaw increases `atan2(x, z)`; for a +Z-aligned witness this rotates
/// +Z toward +X, the positive right-handed direction around normalized +Y. A multi-step
/// result within [`ROOT_YAW_HALF_TURN_AMBIGUITY_DEG`] of a half turn is
/// canonicalized to signed ±180 using the unwrapped sign.
/// Exact 180-degree adjacent steps are ambiguous and make only yaw unavailable.
///
/// Returns `None` when the selected bone is outside the grid or fewer than two
/// samples exist. Translation and yaw are derived independently: non-finite
/// positions set [`RootTrajectoryMetrics::translation`] to `None`, while
/// rotation or heading failures set [`RootTrajectoryMetrics::yaw`] to `None`.
pub fn root_trajectory_metrics(grid: &PoseGrid, bone: usize) -> Option<RootTrajectoryMetrics> {
    let frames = grid.frame_count();
    if frames < 2 || bone >= grid.bone_count() {
        return None;
    }

    let first = grid.model_position(0, bone);
    let mut translation_valid = first.is_finite();
    let first_x = f64::from(first.x);
    let first_y = f64::from(first.y);
    let first_z = f64::from(first.z);
    let mut last_x = first_x;
    let mut last_y = first_y;
    let mut last_z = first_z;
    let mut previous_x = first_x;
    let mut previous_z = first_z;
    let mut horizontal_travel_m = 0.0f64;
    let mut vertical_min_displacement_m = 0.0f64;
    let mut vertical_max_displacement_m = 0.0f64;

    let mut yaw_valid = true;
    let mut heading_axis = None;
    let mut first_heading_deg: Option<f64> = None;
    let mut previous_heading_deg: Option<f64> = None;
    let mut winding_turns = 0i64;
    let mut yaw_travel_deg = 0.0f64;

    for frame in 0..frames {
        let position = grid.model_position(frame, bone);
        if !position.is_finite() {
            translation_valid = false;
        } else if translation_valid {
            last_x = f64::from(position.x);
            last_y = f64::from(position.y);
            last_z = f64::from(position.z);
            if frame > 0 {
                horizontal_travel_m += (last_x - previous_x).hypot(last_z - previous_z);
            }
            previous_x = last_x;
            previous_z = last_z;
            let vertical = last_y - first_y;
            vertical_min_displacement_m = vertical_min_displacement_m.min(vertical);
            vertical_max_displacement_m = vertical_max_displacement_m.max(vertical);
        }

        if !yaw_valid {
            continue;
        }
        let rotation = grid.model_rotation(frame, bone);
        let length_squared = rotation.length_squared();
        if !rotation.is_finite() || !length_squared.is_finite() || length_squared == 0.0 {
            yaw_valid = false;
            continue;
        }
        let rotation = rotation.as_dquat().normalize();
        let axis = *heading_axis.get_or_insert_with(|| select_horizontal_heading_axis(rotation));
        let (heading_x, heading_z) = horizontal_heading(rotation, axis);
        let horizontal_length = heading_x.hypot(heading_z);
        if !horizontal_length.is_finite() || horizontal_length <= f64::from(f32::EPSILON) {
            yaw_valid = false;
            continue;
        }
        let heading_deg = heading_x.atan2(heading_z).to_degrees();
        if let Some(previous) = previous_heading_deg {
            let raw_delta = heading_deg - previous;
            if (raw_delta.abs() - 180.0).abs() <= ROOT_YAW_HALF_TURN_AMBIGUITY_DEG {
                yaw_valid = false;
                continue;
            }
            if raw_delta > 180.0 {
                winding_turns -= 1;
                yaw_travel_deg += (raw_delta - 360.0).abs();
            } else if raw_delta < -180.0 {
                winding_turns += 1;
                yaw_travel_deg += (raw_delta + 360.0).abs();
            } else {
                yaw_travel_deg += raw_delta.abs();
            }
        } else {
            first_heading_deg = Some(heading_deg);
        }
        previous_heading_deg = Some(heading_deg);
    }

    // `PoseGrid` positions are binary32 and the grid cannot exceed
    // `usize::MAX` frames, so finite samples cannot overflow these widened
    // binary64 endpoint, extrema, or accumulated-travel calculations. Keep
    // the final filter as a fail-closed boundary if either representation
    // changes; model-space FK overflow is rejected in the loop above.
    let translation = translation_valid
        .then_some(RootTranslationMetrics {
            horizontal_displacement_x_m: last_x - first_x,
            horizontal_displacement_z_m: last_z - first_z,
            horizontal_travel_m,
            vertical_displacement_m: last_y - first_y,
            vertical_min_displacement_m,
            vertical_max_displacement_m,
        })
        .filter(|translation| {
            [
                translation.horizontal_displacement_x_m,
                translation.horizontal_displacement_z_m,
                translation.horizontal_travel_m,
                translation.vertical_displacement_m,
                translation.vertical_min_displacement_m,
                translation.vertical_max_displacement_m,
            ]
            .into_iter()
            .all(f64::is_finite)
        });

    let yaw = yaw_valid.then(|| {
        let unwrapped_yaw_deg = previous_heading_deg.expect("non-empty pose grid")
            - first_heading_deg.expect("non-empty pose grid")
            + winding_turns as f64 * 360.0;
        RootYawMetrics {
            heading_axis: heading_axis.expect("non-empty pose grid"),
            net_yaw_deg: canonical_net_yaw_deg(unwrapped_yaw_deg),
            unwrapped_yaw_deg,
            yaw_travel_deg,
        }
    });
    let yaw = yaw.filter(|yaw| {
        yaw.net_yaw_deg.is_finite()
            && yaw.unwrapped_yaw_deg.is_finite()
            && yaw.yaw_travel_deg.is_finite()
    });

    Some(RootTrajectoryMetrics { translation, yaw })
}

/// Return the shortest-path model-space rotation vector from `from` to `to`.
///
/// The left-relative step (`to * from⁻¹`) expresses the angular direction in
/// model space. Canonicalizing the quaternion hemisphere makes the result
/// invariant to the equivalent `q`/`-q` representation. At exactly 180
/// degrees, where `w` cannot choose a hemisphere, the first non-zero vector
/// component breaks the tie deterministically.
fn shortest_path_model_rotation_vector(from: Quat, to: Quat) -> Option<Vec3> {
    let mut step = to * from.conjugate();
    if !step.is_finite() {
        return None;
    }

    let [x, y, z, w] = step.to_array();
    if w < 0.0 || (w == 0.0 && (x < 0.0 || (x == 0.0 && (y < 0.0 || (y == 0.0 && z < 0.0))))) {
        step = -step;
    }

    let vector = step.xyz();
    let sin_half_angle = vector.length();
    if !sin_half_angle.is_finite() {
        return None;
    }
    if sin_half_angle == 0.0 {
        return Some(Vec3::ZERO);
    }

    let angle_rad = 2.0 * sin_half_angle.atan2(step.w);
    let rotation_vector = vector * (angle_rad / sin_half_angle);
    rotation_vector.is_finite().then_some(rotation_vector)
}

/// Measure C0 pose closure plus C1 linear- and angular-velocity continuity
/// independently for every bone.
///
/// The grid spans `[0, duration]`, including both endpoints. C1 continuity is
/// therefore the difference between the in-clip step entering the last sample
/// and the in-clip step leaving frame 0. Treating the last-to-first endpoint
/// chord as a velocity would assign zero velocity to a perfectly closed loop.
///
/// Returns `None` when the shared grid has fewer than three frames, has no
/// bones, or has an unusable seam-adjacent time step. A row is `None` only
/// when that bone's seam-adjacent model-space evidence is unusable; one bad
/// bone never suppresses finite evidence for another bone.
pub fn loop_continuity_metrics(grid: &PoseGrid) -> Option<Vec<Option<BoneLoopContinuityMetrics>>> {
    let frames = grid.frame_count();
    if frames < 3 || grid.bone_count() == 0 {
        return None;
    }

    let first_dt = f64::from(grid.times[1] - grid.times[0]);
    let last_dt = f64::from(grid.times[frames - 1] - grid.times[frames - 2]);
    if !first_dt.is_finite() || !last_dt.is_finite() || first_dt <= 0.0 || last_dt <= 0.0 {
        return None;
    }

    Some(
        (0..grid.bone_count())
            .map(|bone| {
                let first = grid.model_position(0, bone);
                let next = grid.model_position(1, bone);
                let previous = grid.model_position(frames - 2, bone);
                let last = grid.model_position(frames - 1, bone);
                if [first, next, previous, last]
                    .iter()
                    .any(|position| !position.is_finite())
                {
                    return None;
                }

                let rotations = [
                    grid.model_rotation(0, bone),
                    grid.model_rotation(1, bone),
                    grid.model_rotation(frames - 2, bone),
                    grid.model_rotation(frames - 1, bone),
                ];
                if rotations.iter().any(|rotation| {
                    !rotation.is_finite()
                        || !rotation.length_squared().is_finite()
                        || rotation.length_squared() == 0.0
                }) {
                    return None;
                }
                let [
                    first_rotation,
                    next_rotation,
                    previous_rotation,
                    last_rotation,
                ] = rotations.map(Quat::normalize);
                let delta = first_rotation.conjugate() * last_rotation;
                let [x, y, z, w] = delta.to_array();
                let sin_half_angle = Vec3::new(x, y, z).length();
                let rotation_delta_deg =
                    f64::from(2.0 * sin_half_angle.atan2(w.abs()).to_degrees());
                let position_delta_m = f64::from((last - first).length());
                let outgoing_velocity = (next - first) / first_dt as f32;
                let incoming_velocity = (last - previous) / last_dt as f32;
                let seam_velocity_delta_mps =
                    f64::from((outgoing_velocity - incoming_velocity).length());
                let outgoing_angular_velocity =
                    shortest_path_model_rotation_vector(first_rotation, next_rotation)?
                        / first_dt as f32;
                let incoming_angular_velocity =
                    shortest_path_model_rotation_vector(previous_rotation, last_rotation)?
                        / last_dt as f32;
                let seam_angular_velocity_delta_degps = f64::from(
                    (outgoing_angular_velocity - incoming_angular_velocity)
                        .length()
                        .to_degrees(),
                );

                if !position_delta_m.is_finite()
                    || !rotation_delta_deg.is_finite()
                    || !seam_velocity_delta_mps.is_finite()
                    || !seam_angular_velocity_delta_degps.is_finite()
                {
                    return None;
                }
                Some(BoneLoopContinuityMetrics {
                    position_delta_m,
                    rotation_delta_deg,
                    seam_velocity_delta_mps,
                    seam_angular_velocity_delta_degps,
                })
            })
            .collect(),
    )
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
    let GaitBones { hips, left, right } = gait_bones(grid, roles)?;
    let feet: Vec<usize> = left.iter().chain(right.iter()).copied().collect();
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
    let has_real_stride = neighbour_step > 0.0 && neighbour_step >= min_stride_step_m;
    let loop_seam_ratio = if has_real_stride {
        let ratio = seam / neighbour_step;
        ratio.is_finite().then_some(ratio)
    } else {
        None
    };

    // Gait phase: fundamental-harmonic trough of the L−R foot-height
    // signal over one cycle (the duplicate wrap frame excluded). The
    // difference cancels common-mode pelvis bob and encodes handedness
    // plus a stable cycle anchor. One function forms that signal and fits
    // it, so a caller that draws the curve draws what was measured.
    let evidence = lr_evidence(grid, hips, &left, &right);
    Some(FootCycleMetrics {
        loop_seam_ratio,
        has_real_stride,
        gait_phase: match evidence.outcome {
            GaitPhaseOutcome::Measured(phase) => Some(phase),
            _ => None,
        },
        lr_amplitude_m: evidence.lr_amplitude_m,
    })
}

/// The bones a gait measurement reads, once the grid is known to carry a
/// cycle it can be read from.
struct GaitBones {
    hips: usize,
    left: Vec<usize>,
    right: Vec<usize>,
}

/// Resolve the bones a gait measurement needs from `grid` and `roles`, or
/// `None` where the clip carries no readable foot cycle at all: under three
/// frames, no hips, no foot on either side, or a non-finite sample anywhere
/// in the hips-relative feet.
///
/// This is the single gate [`foot_cycle_metrics`] and [`gait_phase_evidence`]
/// share, so a caller cannot see a phase the other would refuse to measure.
fn gait_bones(grid: &PoseGrid, roles: &ResolvedRoles) -> Option<GaitBones> {
    if grid.frame_count() < 3 {
        return None;
    }
    let hips = roles.get(Role::Hips)?;
    let side =
        |foot, toe| -> Vec<usize> { [foot, toe].iter().filter_map(|&r| roles.get(r)).collect() };
    let left = side(Role::LeftFoot, Role::LeftToe);
    let right = side(Role::RightFoot, Role::RightToe);
    let feet: Vec<usize> = left.iter().chain(right.iter()).copied().collect();
    if feet.is_empty() {
        return None;
    }
    let finite = (0..grid.frame_count()).all(|frame| {
        let hips_position = grid.model_position(frame, hips);
        hips_position.is_finite()
            && feet
                .iter()
                .all(|&foot| (grid.model_position(frame, foot) - hips_position).is_finite())
    });
    finite.then_some(GaitBones { hips, left, right })
}

/// The stride-anchor evidence one clip yields.
///
/// The signal, the samples a cycle spans, the fit's outcome and the swing an
/// amplitude floor is judged against all come from one measurement, so a
/// drawn curve, a drawn anchor and a reported coverage gap describe the same
/// thing rather than three re-derivations of it.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct GaitPhaseEvidence {
    /// Left-minus-right foot height relative to the hips, in metres, one
    /// value per sampled frame of the clip's metric grid. Empty when the rig
    /// resolves a foot on one side only, which is a left-minus-right signal
    /// with no right-hand term.
    pub lr_foot_height_m: Vec<f64>,
    /// Samples one stride cycle spans, so sample `k` of
    /// [`Self::lr_foot_height_m`] sits at cycle position `k / cycle_samples`
    /// and a phase `p` sits at sample `p * cycle_samples`. A grid over three
    /// frames excludes its duplicate wrap sample from the cycle; a
    /// three-frame grid has no duplicate to exclude.
    pub cycle_samples: usize,
    /// Why this clip does or does not carry a stride anchor.
    pub outcome: GaitPhaseOutcome,
    /// Peak-to-peak swing of the signal over one cycle (metres).
    pub lr_amplitude_m: f64,
}

/// Measure one clip's stride-anchor evidence from its metric grid.
///
/// `None` where the clip carries no readable foot cycle at all — the same
/// refusal [`foot_cycle_metrics`] makes, through the same gate.
///
/// # Panics
///
/// Panics if `roles` contains bone indices outside `grid`, exactly as
/// [`foot_cycle_metrics`] does.
pub fn gait_phase_evidence(grid: &PoseGrid, roles: &ResolvedRoles) -> Option<GaitPhaseEvidence> {
    let GaitBones { hips, left, right } = gait_bones(grid, roles)?;
    Some(lr_evidence(grid, hips, &left, &right))
}

/// Samples one stride cycle spans on a `frames`-sample metric grid.
///
/// A grid over three frames repeats its first sample at the wrap, and that
/// duplicate is not part of the cycle; a three-frame grid is too short to
/// carry one. Sample `k` therefore sits at cycle position `k / cycle`, which
/// is where a phase measured on that cycle can be drawn against it.
pub fn gait_cycle_samples(frames: usize) -> usize {
    if frames > 3 { frames - 1 } else { frames }
}

/// Form the left-minus-right foot-height signal and fit its stride anchor.
fn lr_evidence(grid: &PoseGrid, hips: usize, left: &[usize], right: &[usize]) -> GaitPhaseEvidence {
    let frames = grid.frame_count();
    let cycle_samples = gait_cycle_samples(frames);
    if left.is_empty() || right.is_empty() {
        return GaitPhaseEvidence {
            lr_foot_height_m: Vec::new(),
            cycle_samples,
            outcome: GaitPhaseOutcome::MissingBilateralFootRoles,
            lr_amplitude_m: 0.0,
        };
    }
    let mean_height = |frame: usize, bones: &[usize]| -> f64 {
        let hips_position = grid.model_position(frame, hips);
        bones
            .iter()
            .map(|&bone| (grid.model_position(frame, bone) - hips_position).y as f64)
            .sum::<f64>()
            / bones.len() as f64
    };
    let lr_foot_height_m: Vec<f64> = (0..frames)
        .map(|frame| mean_height(frame, left) - mean_height(frame, right))
        .collect();
    // The amplitude and the fit read one cycle; the series keeps every
    // sampled frame, because a caller drawing the curve draws the clip.
    let cycle = &lr_foot_height_m[..cycle_samples];
    let max = cycle.iter().copied().fold(f64::MIN, f64::max);
    let min = cycle.iter().copied().fold(f64::MAX, f64::min);
    let lr_amplitude_m = max - min;
    let phase = (lr_amplitude_m > 0.0)
        .then(|| fundamental_trough_phase(cycle))
        .flatten();
    GaitPhaseEvidence {
        lr_foot_height_m,
        cycle_samples,
        outcome: GaitPhaseOutcome::classify(phase, lr_amplitude_m, true),
        lr_amplitude_m,
    }
}

/// How one declared member of a gait group stands, classified once for every
/// consumer.
///
/// The `gait-group` check turns these into findings and coverage gaps; a
/// report draws the measured ones and names the rest. Deriving them twice is
/// what lets a picture claim a member was measured where the check recorded
/// a gap.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum GaitMemberPhase {
    /// The document holds no clip of this member's name.
    Absent,
    /// The rig resolved no hips-plus-foot pair, so no member of any group
    /// has a phase to measure (see [`gait_roles_resolved`]).
    RolesUnresolved,
    /// The clip is present but carries no readable foot cycle.
    NoFootCycle,
    /// The foot cycle was read and yielded no anchor, with the reason.
    NoAnchor(GaitPhaseOutcome),
    /// The swing is under the group's evidence floor, where a phase is noise.
    BelowFloor {
        /// Peak-to-peak swing measured (metres).
        amplitude_m: f64,
        /// The floor it is under (metres).
        floor_m: f64,
    },
    /// The measured stride anchor, in cycle fraction `[0, 1)`.
    Measured(f64),
}

impl GaitMemberPhase {
    /// The anchor a member contributes to its group's spread, if any.
    pub fn anchor(self) -> Option<f64> {
        match self {
            GaitMemberPhase::Measured(phase) => Some(phase),
            _ => None,
        }
    }
}

/// Classify one declared gait-group member.
///
/// `evidence` is what [`gait_phase_evidence`] returned for the member's clip,
/// or `None` where the document holds no such clip (`present` is `false`) or
/// the clip carries no readable foot cycle.
pub fn gait_member_phase(
    roles: &ResolvedRoles,
    present: bool,
    evidence: Option<&GaitPhaseEvidence>,
    min_lr_amplitude_m: f64,
) -> GaitMemberPhase {
    if !present {
        return GaitMemberPhase::Absent;
    }
    if !gait_roles_resolved(roles) {
        return GaitMemberPhase::RolesUnresolved;
    }
    let Some(evidence) = evidence else {
        return GaitMemberPhase::NoFootCycle;
    };
    match evidence.outcome {
        GaitPhaseOutcome::Measured(_) if evidence.lr_amplitude_m < min_lr_amplitude_m => {
            GaitMemberPhase::BelowFloor {
                amplitude_m: evidence.lr_amplitude_m,
                floor_m: min_lr_amplitude_m,
            }
        }
        GaitPhaseOutcome::Measured(phase) => GaitMemberPhase::Measured(phase),
        outcome => GaitMemberPhase::NoAnchor(outcome),
    }
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
/// duration. Uses the Root role whenever it resolves and falls back to Hips
/// only when Root is unresolved (clips without a dedicated root bone carry
/// travel on the hips). Returns `None` rather than falling back when the
/// selected role index is outside `grid`, the grid is too short, duration is
/// non-positive, or the derived speed is non-finite.
pub fn root_motion_speed_mps(grid: &PoseGrid, roles: &ResolvedRoles) -> Option<f64> {
    let bone = roles.get(Role::Root).or_else(|| roles.get(Role::Hips))?;
    let frames = grid.frame_count();
    if frames < 2 || bone >= grid.bone_count() {
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

/// A cycle position folded into `[0, 1)`.
///
/// `rem_euclid` rounds a hair-negative input's wrap up to exactly one, and
/// the cycle position of a full turn is zero.
pub fn wrap_unit_phase(phase: f64) -> f64 {
    let wrapped = phase.rem_euclid(1.0);
    if wrapped < 1.0 { wrapped } else { 0.0 }
}

/// Circular distance between two normalized phases, in cycle fraction
/// `[0, 0.5]`. Phases live on a ring, so the distance is the shorter of the
/// two arcs between them.
pub fn circular_phase_distance(phase: f64, other: f64) -> f64 {
    let distance = (phase - other).abs() % 1.0;
    distance.min(1.0 - distance)
}

/// The circular mean of a set of normalized phases and the maximum deviation
/// from it, in one pass; `None` for an empty set, which has no mean
/// direction.
///
/// The mean is a cycle position in `[0, 1)` and the deviation a cycle
/// fraction in `[0, 0.5]`. A caller drawing a tolerance around a group of
/// phases needs the centre as well as the spread, and taking them separately
/// computed the resultant twice.
///
/// A set whose vectors cancel keeps whatever direction `atan2` derives from
/// the residual sums — the same direction the spread has always been
/// measured from.
pub fn circular_phase_center_spread(phases: &[f64]) -> Option<(f64, f64)> {
    use std::f64::consts::{PI, TAU};
    if phases.is_empty() {
        return None;
    }
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
    Some((wrap_unit_phase(mean / TAU), max_dev))
}

/// Maximum circular distance (in cycle fraction, `[0, 0.5]`) of a set of
/// normalized phases from their circular mean. Phases live on a ring, so
/// a naive max−min would over-report a cluster straddling the 0/1 wrap.
pub fn circular_phase_spread(phases: &[f64]) -> f64 {
    circular_phase_center_spread(phases).map_or(0.0, |(_, spread)| spread)
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
    use crate::measure::{RootTrajectorySourceRole, measure_document};
    use crate::model::{
        Bone, Clip, Document, Interpolation, Property, Skeleton, Track, TrackValues, Transform,
    };
    use crate::profile::{ResolvedRoles, Role};
    use glam::{EulerRot, Mat3, Quat, Vec3};
    use std::rc::Rc;

    #[test]
    fn gait_phase_outcome_retains_the_defensive_derivation_failure_state() {
        assert_eq!(
            GaitPhaseOutcome::classify(None, 0.01, true),
            GaitPhaseOutcome::Unavailable
        );
        assert_eq!(
            GaitPhaseOutcome::classify(Some(0.25), 0.01, true),
            GaitPhaseOutcome::Measured(0.25)
        );
        assert_eq!(
            GaitPhaseOutcome::classify(None, 0.0, true),
            GaitPhaseOutcome::NoFootHeightSwing
        );
        assert_eq!(
            GaitPhaseOutcome::classify(None, 0.0, false),
            GaitPhaseOutcome::MissingBilateralFootRoles
        );
    }

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
    fn unrelated_dense_track_changes_shared_sampled_trajectory_not_the_analytic_curve() {
        let skeleton = Skeleton {
            bones: vec![
                Bone {
                    name: "root".into(),
                    parent: None,
                    rest: Transform::IDENTITY,
                    inverse_bind: None,
                },
                Bone {
                    name: "unrelated".into(),
                    parent: None,
                    rest: Transform::IDENTITY,
                    inverse_bind: None,
                },
            ],
        };
        let target_tracks = vec![
            Track {
                bone: 0,
                property: Property::Translation,
                interpolation: Interpolation::Linear,
                times: vec![0.0, 0.25, 1.0],
                values: TrackValues::Vec3s(vec![Vec3::ZERO, Vec3::new(1.0, 2.0, 0.0), Vec3::ZERO]),
            },
            Track {
                bone: 0,
                property: Property::Rotation,
                interpolation: Interpolation::Linear,
                times: vec![0.0, 0.25, 1.0],
                values: TrackValues::Quats(
                    [0.0_f32, 170.0, 340.0]
                        .map(|degrees| Quat::from_rotation_y(degrees.to_radians()))
                        .to_vec(),
                ),
            },
        ];
        let document = Document {
            skeleton: skeleton.clone(),
            clips: vec![Clip {
                name: "aliased".into(),
                duration_s: 1.0,
                tracks: target_tracks.clone(),
            }],
            ..Document::default()
        };
        let coarse_grid = MetricGrids::new(&document)
            .grid(0)
            .expect("three-sample grid");
        assert_eq!(coarse_grid.frame_count(), 3);
        let coarse = root_trajectory_metrics(&coarse_grid, 0).expect("coarse trajectory");
        let coarse_translation = coarse.translation.expect("coarse translation");
        let coarse_yaw = coarse.yaw.expect("coarse yaw");

        let analytic_peak_y = 2.0;
        assert!((coarse_translation.horizontal_travel_m - 4.0 / 3.0).abs() < 1.0e-5);
        assert!((coarse_translation.vertical_max_displacement_m - 4.0 / 3.0).abs() < 1.0e-5);
        assert!(coarse_translation.vertical_max_displacement_m < analytic_peak_y);
        assert!((coarse_yaw.net_yaw_deg - -20.0).abs() < 1.0e-4);
        assert!((coarse_yaw.unwrapped_yaw_deg - -20.0).abs() < 1.0e-4);
        assert!((coarse_yaw.yaw_travel_deg - 740.0 / 3.0).abs() < 1.0e-3);

        let mut dense_document = Document {
            skeleton,
            clips: vec![Clip {
                name: "aliased".into(),
                duration_s: 1.0,
                tracks: target_tracks,
            }],
            ..Document::default()
        };
        dense_document.clips[0].tracks.push(Track {
            bone: 1,
            property: Property::Scale,
            interpolation: Interpolation::Linear,
            times: vec![0.0, 0.25, 0.5, 0.75, 1.0],
            values: TrackValues::Vec3s(vec![Vec3::ONE; 5]),
        });
        let dense_grid = MetricGrids::new(&dense_document)
            .grid(0)
            .expect("unrelated track selects five-sample grid");
        assert_eq!(dense_grid.frame_count(), 5);
        let dense = root_trajectory_metrics(&dense_grid, 0).expect("dense trajectory");
        let dense_translation = dense.translation.expect("dense translation");
        let dense_yaw = dense.yaw.expect("dense yaw");

        assert!((dense_translation.horizontal_travel_m - 2.0).abs() < 1.0e-5);
        assert_eq!(
            dense_translation.vertical_max_displacement_m,
            analytic_peak_y
        );
        assert!((dense_yaw.net_yaw_deg - -20.0).abs() < 1.0e-4);
        assert!((dense_yaw.unwrapped_yaw_deg - 340.0).abs() < 1.0e-3);
        assert!((dense_yaw.yaw_travel_deg - 340.0).abs() < 1.0e-3);

        let coarse_roles =
            ResolvedRoles::from_names(&document.skeleton, [(Role::Root, "root".into())]);
        let coarse_measurements = measure_document(
            &MetricGrids::new(&document),
            &coarse_roles,
            &Config::default(),
        );
        let coarse_published = coarse_measurements["aliased"]
            .root_trajectory
            .as_ref()
            .expect("public coarse Root trajectory");
        assert_eq!(coarse_published.source_role, RootTrajectorySourceRole::Root);
        let coarse_published_translation = coarse_published.translation.unwrap();
        let coarse_published_yaw = coarse_published.yaw.unwrap();
        assert!((coarse_published_translation.horizontal_travel_m - 4.0 / 3.0).abs() < 1.0e-5);
        assert!(
            (coarse_published_translation.vertical_max_displacement_m - 4.0 / 3.0).abs() < 1.0e-5
        );
        assert!((coarse_published_yaw.unwrapped_yaw_deg - -20.0).abs() < 1.0e-4);
        assert!((coarse_published_yaw.yaw_travel_deg - 740.0 / 3.0).abs() < 1.0e-3);

        let dense_roles =
            ResolvedRoles::from_names(&dense_document.skeleton, [(Role::Root, "root".into())]);
        let dense_measurements = measure_document(
            &MetricGrids::new(&dense_document),
            &dense_roles,
            &Config::default(),
        );
        let dense_published = dense_measurements["aliased"]
            .root_trajectory
            .as_ref()
            .expect("public dense Root trajectory");
        assert_eq!(dense_published.source_role, RootTrajectorySourceRole::Root);
        let dense_published_translation = dense_published.translation.unwrap();
        let dense_published_yaw = dense_published.yaw.unwrap();
        assert!((dense_published_translation.horizontal_travel_m - 2.0).abs() < 1.0e-5);
        assert_eq!(
            dense_published_translation.vertical_max_displacement_m,
            analytic_peak_y
        );
        assert!((dense_published_yaw.unwrapped_yaw_deg - 340.0).abs() < 1.0e-3);
        assert!((dense_published_yaw.yaw_travel_deg - 340.0).abs() < 1.0e-3);
        assert_ne!(
            coarse_published_translation.horizontal_travel_m,
            dense_published_translation.horizontal_travel_m
        );
        assert_ne!(
            coarse_published_translation.vertical_max_displacement_m,
            dense_published_translation.vertical_max_displacement_m
        );
        assert_ne!(
            coarse_published_yaw.unwrapped_yaw_deg,
            dense_published_yaw.unwrapped_yaw_deg
        );
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

    /// A real stride can still leave `loop_seam_ratio` `None`: the seam
    /// (frame `frames - 1` vs frame `0`) is a single point-to-point
    /// distance, unconstrained by either neighbour step, so it can sit at
    /// a per-axis delta near `f32::MAX` while both neighbour steps stay at
    /// the smallest representable positive `f32` value (comfortably over
    /// the configured floor). `f32` squares that delta while computing the
    /// distance, overflowing to infinity even though every input position
    /// was finite — the ratio then divides out non-finite, not "no
    /// subject". This is the only known route to
    /// [`FootCycleMetrics::loop_seam_ratio`]'s "real stride but
    /// underivable" `None`, and it requires magnitudes far outside any
    /// real animation.
    #[test]
    fn foot_metrics_real_stride_with_seam_beyond_f32_squaring_range_has_no_ratio() {
        let mut doc = document_with_metric_clip();
        doc.skeleton.bones = vec![
            Bone {
                name: "hips".into(),
                parent: None,
                rest: Transform::IDENTITY,
                inverse_bind: None,
            },
            Bone {
                name: "left".into(),
                parent: Some(0),
                rest: Transform::IDENTITY,
                inverse_bind: None,
            },
        ];
        doc.clips[0].tracks = vec![Track {
            bone: 1,
            property: Property::Translation,
            interpolation: Interpolation::Linear,
            times: vec![0.0, 0.25, 0.5, 1.0],
            values: TrackValues::Vec3s(vec![
                Vec3::ZERO,
                Vec3::new(f32::MIN_POSITIVE, 0.0, 0.0),
                Vec3::new(f32::MAX - f32::MIN_POSITIVE, 0.0, 0.0),
                Vec3::new(f32::MAX, 0.0, 0.0),
            ]),
        }];
        let roles = ResolvedRoles::from_names(
            &doc.skeleton,
            [
                (Role::Hips, "hips".to_string()),
                (Role::LeftFoot, "left".to_string()),
            ],
        );
        let grid = MetricGrids::new(&doc).grid(0).expect("metric grid");

        let metrics = foot_cycle_metrics(&grid, &roles, f64::from(f32::MIN_POSITIVE))
            .expect("hips + one foot role with enough frames yields metrics");

        assert!(
            metrics.has_real_stride,
            "the neighbour step met the (tiny) configured floor"
        );
        assert_eq!(
            metrics.loop_seam_ratio, None,
            "the seam distance overflowed f32 squaring to infinity, so the \
             ratio is non-finite despite a real stride"
        );
    }

    fn trajectory_grid_with_rotations(positions: Vec<Vec3>, rotations: Vec<Quat>) -> PoseGrid {
        assert_eq!(positions.len(), rotations.len());
        assert!(positions.len() >= 2);
        let last = positions.len() - 1;
        let times = (0..=last)
            .map(|index| index as f32 / last as f32)
            .collect::<Vec<_>>();
        let skeleton = Skeleton {
            bones: vec![Bone {
                name: "root".into(),
                parent: None,
                rest: Transform::IDENTITY,
                inverse_bind: None,
            }],
        };
        let clip = Clip {
            name: "trajectory".into(),
            duration_s: 1.0,
            tracks: vec![
                Track {
                    bone: 0,
                    property: Property::Translation,
                    interpolation: Interpolation::Linear,
                    times: times.clone(),
                    values: TrackValues::Vec3s(positions),
                },
                Track {
                    bone: 0,
                    property: Property::Rotation,
                    interpolation: Interpolation::Linear,
                    times,
                    values: TrackValues::Quats(rotations),
                },
            ],
        };
        sample_clip(&skeleton, &clip, last + 1)
    }

    fn trajectory_grid(positions: Vec<Vec3>, yaw_degrees: Vec<f32>) -> PoseGrid {
        trajectory_grid_with_rotations(
            positions,
            yaw_degrees
                .into_iter()
                .map(|degrees| Quat::from_rotation_y(degrees.to_radians()))
                .collect(),
        )
    }

    fn assert_stationary_translation(trajectory: &RootTrajectoryMetrics) {
        let translation = trajectory
            .translation
            .expect("finite stationary translation");
        assert_eq!(translation.horizontal_displacement_x_m, 0.0);
        assert_eq!(translation.horizontal_displacement_z_m, 0.0);
        assert_eq!(translation.horizontal_travel_m, 0.0);
        assert_eq!(translation.vertical_displacement_m, 0.0);
        assert_eq!(translation.vertical_min_displacement_m, 0.0);
        assert_eq!(translation.vertical_max_displacement_m, 0.0);
    }

    #[test]
    fn root_trajectory_retains_direction_travel_and_vertical_extrema() {
        let grid = trajectory_grid(
            vec![
                Vec3::ZERO,
                Vec3::new(1.0, 2.0, 0.0),
                Vec3::new(1.0, 1.0, -1.0),
            ],
            vec![0.0, 45.0, 90.0],
        );
        let trajectory = root_trajectory_metrics(&grid, 0).expect("valid selected bone");
        let translation = trajectory.translation.expect("finite translation");

        assert_eq!(translation.horizontal_displacement_x_m, 1.0);
        assert_eq!(translation.horizontal_displacement_z_m, -1.0);
        assert_eq!(translation.horizontal_travel_m, 2.0);
        assert!(
            translation.horizontal_travel_m
                > translation
                    .horizontal_displacement_x_m
                    .hypot(translation.horizontal_displacement_z_m)
        );
        assert_eq!(translation.vertical_displacement_m, 1.0);
        assert_eq!(translation.vertical_min_displacement_m, 0.0);
        assert_eq!(translation.vertical_max_displacement_m, 2.0);
        let yaw = trajectory.yaw.expect("finite yaw");
        assert!((yaw.net_yaw_deg - 90.0).abs() < 1.0e-4);
        assert!((yaw.unwrapped_yaw_deg - 90.0).abs() < 1.0e-4);

        let out_and_back =
            trajectory_grid(vec![Vec3::ZERO, Vec3::X, Vec3::ZERO], vec![0.0, 0.0, 0.0]);
        let translation = root_trajectory_metrics(&out_and_back, 0)
            .unwrap()
            .translation
            .unwrap();
        assert_eq!(translation.horizontal_displacement_x_m, 0.0);
        assert_eq!(translation.horizontal_travel_m, 2.0);

        for (positions, expected_x, expected_z, expected_travel) in [
            (vec![Vec3::ZERO, -Vec3::Z, -Vec3::Z * 2.0], 0.0, -2.0, 2.0),
            (vec![Vec3::ZERO, Vec3::X * 0.5, Vec3::X], 1.0, 0.0, 1.0),
        ] {
            let grid = trajectory_grid(positions, vec![0.0; 3]);
            let translation = root_trajectory_metrics(&grid, 0)
                .unwrap()
                .translation
                .unwrap();
            assert_eq!(translation.horizontal_displacement_x_m, expected_x);
            assert_eq!(translation.horizontal_displacement_z_m, expected_z);
            assert_eq!(translation.horizontal_travel_m, expected_travel);
        }

        for (positions, expected_net, expected_min, expected_max) in [
            (vec![Vec3::ZERO, Vec3::Y, Vec3::Y * 2.0], 2.0, 0.0, 2.0),
            (vec![Vec3::ZERO, -Vec3::Y, -Vec3::Y * 2.0], -2.0, -2.0, 0.0),
            (vec![Vec3::ZERO, Vec3::Y * 2.0, Vec3::ZERO], 0.0, 0.0, 2.0),
        ] {
            let grid = trajectory_grid(positions, vec![0.0; 3]);
            let translation = root_trajectory_metrics(&grid, 0)
                .unwrap()
                .translation
                .unwrap();
            assert_eq!(translation.vertical_displacement_m, expected_net);
            assert_eq!(translation.vertical_min_displacement_m, expected_min);
            assert_eq!(translation.vertical_max_displacement_m, expected_max);
        }
    }

    #[test]
    fn root_yaw_retains_signed_half_and_full_turns_and_reversing_travel() {
        assert_eq!(canonical_net_yaw_deg(179.99995), 180.0);
        assert_eq!(canonical_net_yaw_deg(-179.99995), -180.0);
        let cases = [
            (vec![0.0, 45.0, 90.0], 90.0, 90.0, 90.0),
            (vec![0.0, -45.0, -90.0], -90.0, -90.0, 90.0),
            (vec![0.0, 90.0, 180.0], 180.0, 180.0, 180.0),
            (vec![0.0, -90.0, -180.0], -180.0, -180.0, 180.0),
            (
                vec![0.0, 60.0, 120.0, 179.99995],
                180.0,
                179.99995,
                179.99995,
            ),
            (
                vec![0.0, -60.0, -120.0, -179.99995],
                -180.0,
                -179.99995,
                179.99995,
            ),
            (vec![0.0, 90.0, 180.0, 270.0, 360.0], 0.0, 360.0, 360.0),
            (vec![0.0, -90.0, -180.0, -270.0, -360.0], 0.0, -360.0, 360.0),
            (vec![0.0, 90.0, 0.0], 0.0, 0.0, 180.0),
        ];

        for (angles, expected_net, expected_unwrapped, expected_travel) in cases {
            let grid = trajectory_grid(vec![Vec3::ZERO; angles.len()], angles);
            let trajectory = root_trajectory_metrics(&grid, 0).unwrap();
            assert_stationary_translation(&trajectory);
            let yaw = trajectory.yaw.expect("yaw with sub-half-turn steps");
            assert_eq!(yaw.heading_axis, RootYawHeadingAxis::PositiveZ);
            if expected_net == 180.0 || expected_net == -180.0 {
                assert_eq!(yaw.net_yaw_deg, expected_net, "{yaw:?}");
            } else {
                assert!((yaw.net_yaw_deg - expected_net).abs() < 1.0e-4, "{yaw:?}");
            }
            assert!(
                (yaw.unwrapped_yaw_deg - expected_unwrapped).abs() < 1.0e-4,
                "{yaw:?}"
            );
            assert!(
                (yaw.yaw_travel_deg - expected_travel).abs() < 1.0e-4,
                "{yaw:?}"
            );
        }
    }

    #[test]
    fn root_trajectory_translation_and_yaw_fail_independently() {
        let bad_position = trajectory_grid(
            vec![Vec3::ZERO, Vec3::new(f32::NAN, 0.0, 0.0), Vec3::ZERO],
            vec![0.0, 45.0, 90.0],
        );
        let measured = root_trajectory_metrics(&bad_position, 0).unwrap();
        assert!(measured.translation.is_none());
        assert!(measured.yaw.is_some());

        let bad_rotation = trajectory_grid_with_rotations(
            vec![Vec3::ZERO, Vec3::X, Vec3::X * 2.0],
            vec![
                Quat::IDENTITY,
                Quat::from_xyzw(f32::NAN, 0.0, 0.0, 1.0),
                Quat::IDENTITY,
            ],
        );
        let measured = root_trajectory_metrics(&bad_rotation, 0).unwrap();
        let translation = measured
            .translation
            .expect("translation remains measurable");
        assert_eq!(translation.horizontal_displacement_x_m, 2.0);
        assert_eq!(translation.horizontal_displacement_z_m, 0.0);
        assert_eq!(translation.horizontal_travel_m, 2.0);
        assert_eq!(translation.vertical_displacement_m, 0.0);
        assert_eq!(translation.vertical_min_displacement_m, 0.0);
        assert_eq!(translation.vertical_max_displacement_m, 0.0);
        assert!(measured.yaw.is_none());

        let zero_rotation = trajectory_grid_with_rotations(
            vec![Vec3::ZERO, Vec3::X, Vec3::X * 2.0],
            vec![
                Quat::IDENTITY,
                Quat::from_xyzw(0.0, 0.0, 0.0, 0.0),
                Quat::IDENTITY,
            ],
        );
        let measured = root_trajectory_metrics(&zero_rotation, 0).unwrap();
        let translation = measured
            .translation
            .expect("zero quaternion only makes rotation decomposition unavailable");
        assert_eq!(translation.horizontal_displacement_x_m, 2.0);
        assert_eq!(translation.horizontal_displacement_z_m, 0.0);
        assert_eq!(translation.horizontal_travel_m, 2.0);
        assert_eq!(translation.vertical_displacement_m, 0.0);
        assert_eq!(translation.vertical_min_displacement_m, 0.0);
        assert_eq!(translation.vertical_max_displacement_m, 0.0);
        assert!(measured.yaw.is_none());
    }

    #[test]
    fn root_trajectory_extrema_widen_finite_samples_and_reject_model_overflow() {
        let finite_extremes = trajectory_grid(
            vec![-Vec3::Y * f32::MAX, Vec3::Y * f32::MAX],
            vec![0.0, 0.0],
        );
        let translation = root_trajectory_metrics(&finite_extremes, 0)
            .expect("selected bone exists")
            .translation
            .expect("finite binary32 samples have finite widened extrema");
        let full_binary32_span = 2.0 * f64::from(f32::MAX);
        assert!(full_binary32_span.is_finite());
        assert_eq!(translation.vertical_displacement_m, full_binary32_span);
        assert_eq!(translation.vertical_min_displacement_m, 0.0);
        assert_eq!(translation.vertical_max_displacement_m, full_binary32_span);

        let skeleton = Skeleton {
            bones: vec![
                Bone {
                    name: "ancestor".into(),
                    parent: None,
                    rest: Transform {
                        translation: Vec3::Y * f32::MAX,
                        ..Transform::IDENTITY
                    },
                    inverse_bind: None,
                },
                Bone {
                    name: "root".into(),
                    parent: Some(0),
                    rest: Transform {
                        translation: Vec3::Y * f32::MAX,
                        ..Transform::IDENTITY
                    },
                    inverse_bind: None,
                },
            ],
        };
        let clip = Clip {
            name: "overflow".into(),
            duration_s: 1.0,
            tracks: vec![Track {
                bone: 1,
                property: Property::Rotation,
                interpolation: Interpolation::Linear,
                times: vec![0.0, 1.0],
                values: TrackValues::Quats(vec![Quat::IDENTITY; 2]),
            }],
        };
        let grid = sample_clip(&skeleton, &clip, 2);

        assert!(
            skeleton
                .bones
                .iter()
                .all(|bone| bone.rest.translation.is_finite()
                    && bone.rest.rotation.is_finite()
                    && bone.rest.scale.is_finite())
        );
        assert!(
            !grid.model_position(0, 1).is_finite(),
            "finite local translations overflow while composing model space"
        );
        let trajectory = root_trajectory_metrics(&grid, 1).expect("selected bone exists");
        assert_eq!(trajectory.translation, None);
        let yaw = trajectory
            .yaw
            .expect("translation overflow does not erase finite yaw");
        assert_eq!(yaw.net_yaw_deg, 0.0);
        assert_eq!(yaw.unwrapped_yaw_deg, 0.0);
        assert_eq!(yaw.yaw_travel_deg, 0.0);
    }

    #[test]
    fn root_yaw_publishes_fixed_basis_and_refuses_ambiguous_steps() {
        let in_place_grid =
            trajectory_grid(vec![Vec3::new(3.0, 2.0, -1.0); 3], vec![0.0, 45.0, 90.0]);
        let in_place = root_trajectory_metrics(&in_place_grid, 0).unwrap();
        assert_stationary_translation(&in_place);

        let positive_x_rotation = Quat::from_rotation_x(std::f32::consts::FRAC_PI_6);
        let positive_x_length = {
            let (x, z) = horizontal_heading(
                positive_x_rotation.as_dquat(),
                RootYawHeadingAxis::PositiveX,
            );
            x.hypot(z)
        };
        for other_axis in [RootYawHeadingAxis::PositiveZ, RootYawHeadingAxis::PositiveY] {
            let (x, z) = horizontal_heading(positive_x_rotation.as_dquat(), other_axis);
            assert!(positive_x_length > x.hypot(z));
        }
        let positive_x_grid =
            trajectory_grid_with_rotations(vec![Vec3::ZERO; 3], vec![positive_x_rotation; 3]);
        let positive_x = root_trajectory_metrics(&positive_x_grid, 0).unwrap();
        assert_stationary_translation(&positive_x);
        assert_eq!(
            positive_x.yaw.unwrap().heading_axis,
            RootYawHeadingAxis::PositiveX
        );

        // Analytic tilted basis, built directly from orthonormal columns rather
        // than Euler angles. Local +Y has world heading 30 degrees and a 0.5
        // vertical component; +X and +Z each have a smaller horizontal
        // projection, so +Y is the strict fixed witness.
        let sqrt_three = 3.0_f32.sqrt();
        let positive_y = Vec3::new(sqrt_three / 4.0, 0.5, 0.75);
        let horizontal_perpendicular = Vec3::new(sqrt_three / 2.0, 0.0, -0.5);
        let tilt_tangent = Vec3::new(-0.25, sqrt_three / 2.0, -sqrt_three / 4.0);
        let positive_x =
            (horizontal_perpendicular + tilt_tangent) * std::f32::consts::FRAC_1_SQRT_2;
        let positive_z = positive_x.cross(positive_y);
        let tilted = Quat::from_mat3(&Mat3::from_cols(positive_x, positive_y, positive_z));
        let (heading_x, heading_z) =
            horizontal_heading(tilted.as_dquat(), RootYawHeadingAxis::PositiveY);
        assert!((heading_x.atan2(heading_z).to_degrees() - 30.0).abs() < 1.0e-5);
        let rotations = [0.0_f32, 20.0, 40.0]
            .map(|degrees| Quat::from_axis_angle(Vec3::Y, degrees.to_radians()) * tilted)
            .to_vec();
        let basis_grid = trajectory_grid_with_rotations(vec![Vec3::ZERO; 3], rotations);
        let basis = root_trajectory_metrics(&basis_grid, 0).unwrap();
        assert_stationary_translation(&basis);
        let yaw = basis.yaw.unwrap();
        assert_eq!(yaw.heading_axis, RootYawHeadingAxis::PositiveY);
        assert!((yaw.net_yaw_deg - 40.0).abs() < 1.0e-4);
        assert!((yaw.unwrapped_yaw_deg - 40.0).abs() < 1.0e-4);
        assert!((yaw.yaw_travel_deg - 40.0).abs() < 1.0e-4);

        // A local-Y twist changes a Y-first Euler decomposition but cannot
        // change the retained local +Y witness. This is the concrete case an
        // Euler-component shortcut gets wrong.
        let local_half_turn = Quat::from_xyzw(0.0, 1.0, 0.0, 0.0);
        let fixed_witness_rotations = vec![tilted, tilted * local_half_turn, tilted];
        let first_euler_y = fixed_witness_rotations[0].to_euler(EulerRot::YXZ).0;
        let twisted_euler_y = fixed_witness_rotations[1].to_euler(EulerRot::YXZ).0;
        assert!(
            (twisted_euler_y - first_euler_y).abs() > 1.0,
            "Y-first Euler shortcut must observe a misleading change"
        );
        let expected_heading = horizontal_heading(
            fixed_witness_rotations[0].as_dquat(),
            RootYawHeadingAxis::PositiveY,
        );
        for rotation in &fixed_witness_rotations[1..] {
            let heading = horizontal_heading(rotation.as_dquat(), RootYawHeadingAxis::PositiveY);
            assert!((heading.0 - expected_heading.0).abs() < 1.0e-7);
            assert!((heading.1 - expected_heading.1).abs() < 1.0e-7);
        }

        let stationary_fixed_witness =
            trajectory_grid_with_rotations(vec![Vec3::ZERO; 3], fixed_witness_rotations.clone());
        let misleading_translation_fixed_witness = trajectory_grid_with_rotations(
            vec![Vec3::ZERO, Vec3::X * 100.0, Vec3::Z * 100.0],
            fixed_witness_rotations,
        );
        let stationary = root_trajectory_metrics(&stationary_fixed_witness, 0).unwrap();
        assert_stationary_translation(&stationary);
        let misleading = root_trajectory_metrics(&misleading_translation_fixed_witness, 0)
            .expect("translation content does not select yaw/up");
        assert!(misleading.translation.unwrap().horizontal_travel_m > 200.0);
        let stationary_yaw = stationary.yaw.unwrap();
        let misleading_yaw = misleading.yaw.unwrap();
        assert_eq!(stationary_yaw.heading_axis, RootYawHeadingAxis::PositiveY);
        assert_eq!(misleading_yaw, stationary_yaw);
        assert_eq!(stationary_yaw.net_yaw_deg, 0.0);
        assert_eq!(stationary_yaw.unwrapped_yaw_deg, 0.0);
        assert_eq!(stationary_yaw.yaw_travel_deg, 0.0);

        let becomes_vertical = trajectory_grid_with_rotations(
            vec![Vec3::ZERO; 3],
            vec![
                Quat::IDENTITY,
                Quat::from_rotation_x(std::f32::consts::FRAC_PI_4),
                Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            ],
        );
        assert!(
            root_trajectory_metrics(&becomes_vertical, 0)
                .unwrap()
                .yaw
                .is_none()
        );

        let half_turn_step = trajectory_grid(vec![Vec3::ZERO; 2], vec![0.0, 180.0]);
        assert!(
            root_trajectory_metrics(&half_turn_step, 0)
                .unwrap()
                .yaw
                .is_none()
        );
    }

    #[test]
    fn root_yaw_uses_animated_ancestor_model_rotation_for_fixed_child_local_pose() {
        let skeleton = Skeleton {
            bones: vec![
                Bone {
                    name: "ancestor".into(),
                    parent: None,
                    rest: Transform::IDENTITY,
                    inverse_bind: None,
                },
                Bone {
                    name: "root".into(),
                    parent: Some(0),
                    rest: Transform::IDENTITY,
                    inverse_bind: None,
                },
            ],
        };
        let clip = Clip {
            name: "inherited_yaw".into(),
            duration_s: 1.0,
            tracks: vec![
                Track {
                    bone: 0,
                    property: Property::Rotation,
                    interpolation: Interpolation::Linear,
                    times: vec![0.0, 0.5, 1.0],
                    values: TrackValues::Quats(
                        [0.0_f32, 45.0, 90.0]
                            .map(|degrees| Quat::from_rotation_y(degrees.to_radians()))
                            .to_vec(),
                    ),
                },
                Track {
                    bone: 1,
                    property: Property::Rotation,
                    interpolation: Interpolation::Linear,
                    times: vec![0.0, 0.5, 1.0],
                    values: TrackValues::Quats(vec![Quat::IDENTITY; 3]),
                },
            ],
        };
        let grid = sample_clip(&skeleton, &clip, 3);
        for frame in 0..grid.frame_count() {
            assert_eq!(grid.local(frame, 1).rotation, Quat::IDENTITY);
        }

        let trajectory = root_trajectory_metrics(&grid, 1).expect("child trajectory");
        assert_stationary_translation(&trajectory);
        let yaw = trajectory.yaw.expect("ancestor supplies model-space yaw");
        assert_eq!(yaw.heading_axis, RootYawHeadingAxis::PositiveZ);
        assert!((yaw.net_yaw_deg - 90.0).abs() < 1.0e-4);
        assert!((yaw.unwrapped_yaw_deg - 90.0).abs() < 1.0e-4);
        assert!((yaw.yaw_travel_deg - 90.0).abs() < 1.0e-4);
    }
}
