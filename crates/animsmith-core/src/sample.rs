//! The sampled layer: what a game runtime sees. A [`PoseGrid`] is a
//! uniform time grid over `[0, duration]` sampled with glTF-spec
//! interpolation semantics (lerp for T/S, shortest-path slerp for R,
//! STEP hold, cubic-spline Hermite; clamp at the ends), then FK'd to
//! model space.
//!
//! For clips declared looping, the wrap pair is `(last frame, frame 0)`
//! — the seam definition every loop check shares.
//!
//! FK composes every skeleton node, including each root node's own local
//! transform. Metrics that need a body-relative frame derive it from
//! resolved roles such as hips and feet.

use crate::model::{Clip, Interpolation, Skeleton, Track, TrackValues, Transform};
use glam::{Mat4, Quat, Vec3};

/// Maximum retained buffer payload for one sampled grid or metric-grid cache.
///
/// This engineering admission ceiling includes sample times and all three
/// pose arrays. It excludes allocation metadata, the document, other owners,
/// and consumer scratch/output storage; it is not a process-memory guarantee.
pub const MAX_SAMPLING_BYTES: usize = 256 * 1024 * 1024;
/// Maximum sampled frames for one grid or cumulatively for one metric cache.
pub const MAX_SAMPLING_FRAMES: usize = 1_000_000;
/// Maximum frame/track pairs for one grid or cumulatively for one metric cache.
///
/// Each pair may search its track timeline; this is not an elapsed-time bound.
pub const MAX_SAMPLING_TRACK_EVALUATIONS: usize = 8_000_000;

/// A sampled grid was refused before it could provide usable evidence.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SamplingError {
    /// A dimension product, retained byte count, or cache sum was unrepresentable.
    #[error("sampling size arithmetic overflow")]
    SizeOverflow,
    /// A fixed per-grid or cumulative cache admission ceiling was exceeded.
    #[error("sampling {resource} budget exceeded: required {required}, limit {limit}")]
    LimitExceeded {
        /// The bounded quantity and whether it belongs to the grid or cache.
        resource: &'static str,
        /// Required count or bytes.
        required: usize,
        /// Inclusive admission ceiling.
        limit: usize,
    },
    /// The allocator could not reserve an admitted buffer.
    #[error("sampling could not allocate {elements} elements for {buffer}")]
    AllocationFailed {
        /// The buffer whose reservation failed.
        buffer: &'static str,
        /// Requested element count.
        elements: usize,
    },
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SamplingLimits {
    pub(crate) bytes: usize,
    pub(crate) frames: usize,
    pub(crate) track_evaluations: usize,
}

impl SamplingLimits {
    pub(crate) const DEFAULT: Self = Self {
        bytes: MAX_SAMPLING_BYTES,
        frames: MAX_SAMPLING_FRAMES,
        track_evaluations: MAX_SAMPLING_TRACK_EVALUATIONS,
    };
}

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct SamplingCost {
    frames: usize,
    pose_cells: usize,
    track_evaluations: usize,
    bytes: usize,
}

impl SamplingCost {
    pub(crate) fn checked(
        frames: usize,
        bones: usize,
        tracks: usize,
    ) -> Result<Self, SamplingError> {
        let pose_cells = frames
            .checked_mul(bones)
            .ok_or(SamplingError::SizeOverflow)?;
        let track_evaluations = frames
            .checked_mul(tracks)
            .ok_or(SamplingError::SizeOverflow)?;
        let bytes_per_cell = size_of::<Transform>() + size_of::<Mat4>() + size_of::<Quat>();
        let bytes = pose_cells
            .checked_mul(bytes_per_cell)
            .and_then(|poses| {
                frames
                    .checked_mul(size_of::<f32>())
                    .and_then(|times| poses.checked_add(times))
            })
            .filter(|&bytes| bytes <= isize::MAX as usize)
            .ok_or(SamplingError::SizeOverflow)?;
        Ok(Self {
            frames,
            pose_cells,
            track_evaluations,
            bytes,
        })
    }

    pub(crate) fn checked_add(self, other: Self) -> Result<Self, SamplingError> {
        Ok(Self {
            frames: self
                .frames
                .checked_add(other.frames)
                .ok_or(SamplingError::SizeOverflow)?,
            pose_cells: self
                .pose_cells
                .checked_add(other.pose_cells)
                .ok_or(SamplingError::SizeOverflow)?,
            track_evaluations: self
                .track_evaluations
                .checked_add(other.track_evaluations)
                .ok_or(SamplingError::SizeOverflow)?,
            bytes: self
                .bytes
                .checked_add(other.bytes)
                .ok_or(SamplingError::SizeOverflow)?,
        })
    }

    pub(crate) fn admit(self, limits: SamplingLimits, cache: bool) -> Result<(), SamplingError> {
        for (resource, required, limit) in [
            (
                if cache { "cache frames" } else { "grid frames" },
                self.frames,
                limits.frames,
            ),
            (
                if cache {
                    "cache retained bytes"
                } else {
                    "grid retained bytes"
                },
                self.bytes,
                limits.bytes,
            ),
            (
                if cache {
                    "cache track evaluations"
                } else {
                    "grid track evaluations"
                },
                self.track_evaluations,
                limits.track_evaluations,
            ),
        ] {
            if required > limit {
                return Err(SamplingError::LimitExceeded {
                    resource,
                    required,
                    limit,
                });
            }
        }
        Ok(())
    }
}

fn reserved<T>(elements: usize, buffer: &'static str) -> Result<Vec<T>, SamplingError> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(elements)
        .map_err(|_| SamplingError::AllocationFailed { buffer, elements })?;
    Ok(result)
}

/// Model-space and local poses for every (frame, bone) of one clip.
#[derive(Debug)]
pub struct PoseGrid {
    /// Uniform sample times, `times[0] == 0`, `times[last] == duration`.
    pub times: Vec<f32>,
    bone_count: usize,
    /// Frame-major: `local[frame * bone_count + bone]`.
    local: Vec<Transform>,
    model: Vec<Mat4>,
    model_rotation: Vec<Quat>,
}

impl PoseGrid {
    fn index(&self, frame: usize, bone: usize) -> usize {
        assert!(
            frame < self.frame_count(),
            "frame index {frame} outside PoseGrid frame count {}",
            self.frame_count()
        );
        assert!(
            bone < self.bone_count,
            "bone index {bone} outside PoseGrid bone count {}",
            self.bone_count
        );
        frame * self.bone_count + bone
    }

    /// Number of sampled frames.
    pub fn frame_count(&self) -> usize {
        self.times.len()
    }

    /// Number of bones sampled per frame.
    pub fn bone_count(&self) -> usize {
        self.bone_count
    }

    /// Local-space transform at `frame` and `bone`.
    ///
    /// # Panics
    ///
    /// Panics if either index is outside the grid bounds.
    pub fn local(&self, frame: usize, bone: usize) -> Transform {
        self.local[self.index(frame, bone)]
    }

    /// Model-space transform at `frame` and `bone`.
    ///
    /// # Panics
    ///
    /// Panics if either index is outside the grid bounds.
    pub fn model(&self, frame: usize, bone: usize) -> Mat4 {
        self.model[self.index(frame, bone)]
    }

    /// Model-space joint position.
    ///
    /// # Panics
    ///
    /// Panics if either index is outside the grid bounds.
    pub fn model_position(&self, frame: usize, bone: usize) -> Vec3 {
        self.model(frame, bone).w_axis.truncate()
    }

    /// Model-space joint rotation, composed from the local rotation chain
    /// independently of scale. This avoids extracting an ambiguous rotation
    /// from a model matrix that contains non-uniform scale or shear.
    ///
    /// # Panics
    ///
    /// Panics if either index is outside the grid bounds.
    pub fn model_rotation(&self, frame: usize, bone: usize) -> Quat {
        self.model_rotation[self.index(frame, bone)]
    }
}

/// Default uniform-grid resolution for a clip: the maximum keyframe count
/// across its tracks, with a minimum of 2. Irregular authored key times can
/// still fall between these uniform samples.
pub fn default_frame_count(clip: &Clip) -> usize {
    clip.tracks
        .iter()
        .map(Track::key_count)
        .max()
        .unwrap_or(2)
        .max(2)
}

/// Sample `clip` on a uniform `frames`-sample grid and FK to model space.
///
/// Sampling is intentionally tolerant and storage-driven rather than a shape
/// validation step. Tracks with an empty timeline or a target bone outside
/// `skeleton` are skipped. `TrackValues::Vec3s` writes translation or scale
/// according to [`Track::property`], ignores the rotation property, and uses
/// [`Vec3::ZERO`] for missing values (so a missing scale value is zero, not
/// the transform scale default of one). `TrackValues::Quats` writes rotation
/// regardless of [`Track::property`] and uses [`Quat::IDENTITY`] for missing
/// values. This tolerant behaviour is distinct from strict operations, which
/// should call [`crate::validate_document_shape`] at their own boundary.
///
/// # Errors
///
/// Returns [`SamplingError`] before sampling if checked dimensions exceed the
/// fixed frame, buffer-payload or track-evaluation ceiling, or a buffer cannot
/// be reserved. Requests of zero or one frame still sample two endpoints.
/// Independent calls have independent budgets; this does not bound a caller's
/// repeated search or the storage of several grids retained by that caller.
///
/// # Panics
///
/// Panics if a skeleton bone's parent index is outside
/// [`Skeleton::bones`]. Loader crates also order parents before children;
/// hand-built documents must preserve both invariants for correct FK.
pub fn sample_clip(
    skeleton: &Skeleton,
    clip: &Clip,
    frames: usize,
) -> Result<PoseGrid, SamplingError> {
    let frames = frames.max(2);
    let cost = SamplingCost::checked(frames, skeleton.bones.len(), clip.tracks.len())?;
    cost.admit(SamplingLimits::DEFAULT, false)?;
    let duration = clip.duration_s as f32;
    let mut times = reserved(frames, "sample times")?;
    times.extend((0..frames).map(|i| duration * i as f32 / (frames - 1) as f32));

    sample_clip_with_times(skeleton, clip, times, cost)
}

/// Sample a clip at caller-provided times and FK to model space.
///
/// Strict transforms use this internal boundary after validating their own
/// authored grid. Keeping those exact binary32 times avoids synthesizing a
/// nearby uniform time through a second floating-point formula.
pub(crate) fn sample_clip_at_times(
    skeleton: &Skeleton,
    clip: &Clip,
    times: &[f32],
) -> Result<PoseGrid, SamplingError> {
    let cost = SamplingCost::checked(times.len(), skeleton.bones.len(), clip.tracks.len())?;
    cost.admit(SamplingLimits::DEFAULT, false)?;
    let mut owned_times = reserved(times.len(), "sample times")?;
    owned_times.extend_from_slice(times);
    sample_clip_with_times(skeleton, clip, owned_times, cost)
}

fn sample_clip_with_times(
    skeleton: &Skeleton,
    clip: &Clip,
    times: Vec<f32>,
    cost: SamplingCost,
) -> Result<PoseGrid, SamplingError> {
    let frames = times.len();
    let nb = skeleton.bones.len();

    let mut local = reserved(cost.pose_cells, "local poses")?;
    local.resize(cost.pose_cells, Transform::IDENTITY);
    let mut model = reserved(cost.pose_cells, "model poses")?;
    model.resize(cost.pose_cells, Mat4::IDENTITY);
    let mut model_rotation = reserved(cost.pose_cells, "model rotations")?;
    model_rotation.resize(cost.pose_cells, Quat::IDENTITY);
    for f in 0..frames {
        for (b, bone) in skeleton.bones.iter().enumerate() {
            local[f * nb + b] = bone.rest;
        }
    }

    for track in &clip.tracks {
        if track.times.is_empty() || track.bone >= nb {
            continue;
        }
        for (f, &t) in times.iter().enumerate() {
            let slot = &mut local[f * nb + track.bone];
            match &track.values {
                TrackValues::Vec3s(_) => {
                    let v = sample_vec3(track, t);
                    match track.property {
                        crate::model::Property::Translation => slot.translation = v,
                        crate::model::Property::Scale => slot.scale = v,
                        crate::model::Property::Rotation => {}
                    }
                }
                TrackValues::Quats(_) => slot.rotation = sample_quat(track, t),
            }
        }
    }

    for f in 0..frames {
        for (b, bone) in skeleton.bones.iter().enumerate() {
            let m = local[f * nb + b].to_mat4();
            model[f * nb + b] = match bone.parent {
                Some(p) => model[f * nb + p] * m,
                None => m,
            };
            let rotation = local[f * nb + b].rotation;
            model_rotation[f * nb + b] = match bone.parent {
                Some(p) => model_rotation[f * nb + p] * rotation,
                None => rotation,
            };
        }
    }

    Ok(PoseGrid {
        times,
        bone_count: nb,
        local,
        model,
        model_rotation,
    })
}

/// One track's sampled value at a time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrackSample {
    /// Sampled translation or scale value.
    Vec3(Vec3),
    /// Sampled rotation value.
    Quat(Quat),
}

/// Sample a single track at `t` with the same semantics the grid uses
/// (clamp at ends, glTF interpolation, shortest-path slerp).
///
/// Hostile or incomplete tracks are tolerated: empty/non-finite timelines
/// choose a clamped segment where possible, and missing values default to zero
/// vectors or the identity quaternion. This function does not validate a
/// [`crate::Document`] because it receives only one track.
pub fn sample_track(track: &Track, t: f32) -> TrackSample {
    match &track.values {
        TrackValues::Vec3s(_) => TrackSample::Vec3(sample_vec3(track, t)),
        TrackValues::Quats(_) => TrackSample::Quat(sample_quat(track, t)),
    }
}

/// Locate the keyframe segment containing `t`: returns `(k0, k1, u)`
/// with `u` in `[0, 1]`. Clamps outside the keyframe range.
///
/// Panic-free on hostile tracks: empty times yield the first segment,
/// and non-finite key times (every comparison false) fall through to
/// the clamped first/last key instead of underflowing — the `nan`
/// check reports them; sampling must merely survive them.
fn segment(times: &[f32], t: f32) -> (usize, usize, f32) {
    let n = times.len();
    if n <= 1 || t <= times[0] {
        return (0, 0, 0.0);
    }
    if t >= times[n - 1] {
        return (n - 1, n - 1, 0.0);
    }
    let k1 = times.partition_point(|&k| k <= t).min(n - 1);
    if k1 == 0 {
        return (0, 0, 0.0);
    }
    let k0 = k1 - 1;
    let dt = times[k1] - times[k0];
    let u = if dt > 0.0 { (t - times[k0]) / dt } else { 0.0 };
    (k0, k1, u)
}

/// Clamped value fetch: loaders enforce times/values length agreement, but
/// `Document`s are plain data an embedder can build by hand — an inconsistent
/// track reads the storage type's default value and never panics.
fn value_at<T: Copy + Default>(vals: &[T], index: usize) -> T {
    vals.get(index).copied().unwrap_or_default()
}

/// glTF cubic-spline Hermite basis at `u`.
fn hermite(u: f32) -> (f32, f32, f32, f32) {
    let u2 = u * u;
    let u3 = u2 * u;
    (
        2.0 * u3 - 3.0 * u2 + 1.0, // h00 (p0)
        u3 - 2.0 * u2 + u,         // h10 (m0)
        -2.0 * u3 + 3.0 * u2,      // h01 (p1)
        u3 - u2,                   // h11 (m1)
    )
}

fn sample_vec3(track: &Track, t: f32) -> Vec3 {
    let TrackValues::Vec3s(vals) = &track.values else {
        return Vec3::ZERO;
    };
    let (k0, k1, u) = segment(&track.times, t);
    let v0 = value_at(vals, track.value_index(k0));
    if k0 == k1 {
        return v0;
    }
    let v1 = value_at(vals, track.value_index(k1));
    match track.interpolation {
        Interpolation::Step => v0,
        Interpolation::Linear => v0.lerp(v1, u),
        Interpolation::CubicSpline => {
            let dt = track.times[k1] - track.times[k0];
            // out-tangent of k0, in-tangent of k1, scaled by dt per spec.
            let m0 = value_at(vals, 3 * k0 + 2) * dt;
            let m1 = value_at(vals, 3 * k1) * dt;
            let (h00, h10, h01, h11) = hermite(u);
            v0 * h00 + m0 * h10 + v1 * h01 + m1 * h11
        }
    }
}

fn sample_quat(track: &Track, t: f32) -> Quat {
    let TrackValues::Quats(vals) = &track.values else {
        return Quat::IDENTITY;
    };
    let (k0, k1, u) = segment(&track.times, t);
    let q0: Quat = value_at(vals, track.value_index(k0));
    if k0 == k1 {
        return q0.normalize();
    }
    let q1 = value_at(vals, track.value_index(k1));
    match track.interpolation {
        Interpolation::Step => q0.normalize(),
        Interpolation::Linear => {
            // Shortest-path slerp: negate the target when the dot is
            // negative, matching what game runtimes do.
            let q1 = if q0.dot(q1) < 0.0 { -q1 } else { q1 };
            q0.normalize().slerp(q1.normalize(), u)
        }
        Interpolation::CubicSpline => {
            // Per glTF spec: componentwise Hermite on the raw
            // quaternion, then normalize.
            let dt = track.times[k1] - track.times[k0];
            let m0 = value_at(vals, 3 * k0 + 2).to_array();
            let m1 = value_at(vals, 3 * k1).to_array();
            let a0 = q0.to_array();
            let a1 = q1.to_array();
            let (h00, h10, h01, h11) = hermite(u);
            let mut out = [0.0f32; 4];
            for i in 0..4 {
                out[i] = a0[i] * h00 + m0[i] * dt * h10 + a1[i] * h01 + m1[i] * dt * h11;
            }
            Quat::from_array(out).normalize()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Bone, Property};

    fn track(times: Vec<f32>, quats: Vec<Quat>) -> Track {
        Track {
            bone: 0,
            property: Property::Rotation,
            interpolation: Interpolation::Linear,
            times,
            values: TrackValues::Quats(quats),
        }
    }

    fn vec3_track(interpolation: Interpolation, times: Vec<f32>, vals: Vec<Vec3>) -> Track {
        Track {
            bone: 0,
            property: Property::Translation,
            interpolation,
            times,
            values: TrackValues::Vec3s(vals),
        }
    }

    #[test]
    #[should_panic(expected = "outside PoseGrid bone count")]
    fn pose_grid_rejects_bone_index_at_frame_boundary() {
        let skeleton = Skeleton {
            bones: vec![Bone {
                name: "root".into(),
                parent: None,
                rest: Transform::IDENTITY,
                inverse_bind: None,
            }],
        };
        let clip = Clip {
            name: "idle".into(),
            duration_s: 1.0,
            tracks: Vec::new(),
        };
        let grid = sample_clip(&skeleton, &clip, 2).unwrap();

        let _ = grid.local(0, grid.bone_count());
    }

    #[test]
    fn out_of_range_clip_track_is_skipped() {
        let skeleton = Skeleton {
            bones: vec![Bone {
                name: "root".into(),
                parent: None,
                rest: Transform::IDENTITY,
                inverse_bind: None,
            }],
        };
        let clip = Clip {
            name: "hostile".into(),
            duration_s: 1.0,
            tracks: vec![Track {
                bone: 1,
                property: Property::Translation,
                interpolation: Interpolation::Linear,
                times: vec![0.0],
                values: TrackValues::Vec3s(vec![Vec3::splat(42.0)]),
            }],
        };

        let grid = sample_clip(&skeleton, &clip, 2).unwrap();
        assert_eq!(grid.local(0, 0), Transform::IDENTITY);
        assert_eq!(grid.local(1, 0), Transform::IDENTITY);
    }

    /// Issue #24: a NaN first key time made both clamp guards fail,
    /// partition_point return 0, and `k1 - 1` underflow.
    #[test]
    fn nan_first_key_time_samples_without_panicking() {
        let t = track(
            vec![f32::NAN, 0.5, 1.0],
            vec![Quat::IDENTITY, Quat::IDENTITY, Quat::IDENTITY],
        );
        for time in [-1.0, 0.0, 0.25, 0.75, 2.0] {
            let TrackSample::Quat(q) = sample_track(&t, time) else {
                panic!("rotation track samples a quat");
            };
            assert!(q.is_finite() || q.is_nan()); // no panic is the contract
        }
    }

    #[test]
    fn all_nan_times_sample_without_panicking() {
        let t = track(
            vec![f32::NAN, f32::NAN],
            vec![Quat::IDENTITY, Quat::IDENTITY],
        );
        sample_track(&t, 0.5);
    }

    #[test]
    fn empty_track_samples_default_without_panicking() {
        let t = track(vec![], vec![]);
        let TrackSample::Quat(q) = sample_track(&t, 0.5) else {
            panic!("rotation track samples a quat");
        };
        assert_eq!(q, Quat::IDENTITY.normalize());
    }

    /// Issue #24: values shorter than times indexed out of bounds.
    /// Loaders reject such tracks; hand-built documents sample the
    /// type default instead of panicking.
    #[test]
    fn short_values_sample_default_without_panicking() {
        let t = track(vec![0.0, 0.5, 1.0], vec![Quat::IDENTITY, Quat::IDENTITY]);
        sample_track(&t, 0.75); // k1 = 2, values has no index 2
    }

    #[test]
    fn short_vec3_values_sample_default_without_panicking() {
        let t = vec3_track(Interpolation::Linear, vec![0.0, 0.5, 1.0], vec![Vec3::ONE]);
        let TrackSample::Vec3(v) = sample_track(&t, 0.75) else {
            panic!("translation track samples a vec3");
        };
        assert!(v.is_finite());
    }

    /// The cubic tangent fetches (3*k0+2, 3*k1) are the indices most
    /// likely to run off a short buffer.
    #[test]
    fn short_cubic_values_sample_default_without_panicking() {
        let t = vec3_track(
            Interpolation::CubicSpline,
            vec![0.0, 1.0],
            vec![Vec3::ZERO, Vec3::ONE, Vec3::ZERO], // 3 of the 6 a cubic pair needs
        );
        sample_track(&t, 0.5);
    }

    #[test]
    fn sampling_cost_counts_all_three_arrays_and_enforces_each_ceiling() {
        let cost = SamplingCost::checked(3, 2, 4).unwrap();
        assert_eq!(cost.pose_cells, 6);
        assert_eq!(cost.track_evaluations, 12);
        // Three f32 times, six (40-byte TRS + 64-byte matrix + 16-byte quat) cells.
        assert_eq!(cost.bytes, 732);
        let exact = SamplingLimits {
            bytes: 732,
            frames: 3,
            track_evaluations: 12,
        };
        assert!(cost.admit(exact, false).is_ok());
        for limits in [
            SamplingLimits {
                bytes: 731,
                ..exact
            },
            SamplingLimits { frames: 2, ..exact },
            SamplingLimits {
                track_evaluations: 11,
                ..exact
            },
        ] {
            assert!(matches!(
                cost.admit(limits, false),
                Err(SamplingError::LimitExceeded { .. })
            ));
        }
    }

    #[test]
    fn production_sampling_boundaries_are_checked_without_allocating_grids() {
        // Eight-bone grids need 964 bytes/frame. These adjacent frame counts
        // straddle the literal 256 MiB ceiling while staying under the frame cap.
        assert_eq!(
            SamplingCost::checked(278_460, 8, 0).unwrap().bytes,
            268_435_440
        );
        assert!(
            SamplingCost::checked(278_460, 8, 0)
                .unwrap()
                .admit(SamplingLimits::DEFAULT, false)
                .is_ok()
        );
        assert!(
            SamplingCost::checked(278_461, 8, 0)
                .unwrap()
                .admit(SamplingLimits::DEFAULT, false)
                .is_err()
        );
        assert!(
            SamplingCost::checked(1_000_000, 0, 0)
                .unwrap()
                .admit(SamplingLimits::DEFAULT, false)
                .is_ok()
        );
        assert!(
            SamplingCost::checked(1_000_001, 0, 0)
                .unwrap()
                .admit(SamplingLimits::DEFAULT, false)
                .is_err()
        );
        assert!(
            SamplingCost::checked(2, 0, 4_000_000)
                .unwrap()
                .admit(SamplingLimits::DEFAULT, false)
                .is_ok()
        );
        assert!(
            SamplingCost::checked(2, 0, 4_000_001)
                .unwrap()
                .admit(SamplingLimits::DEFAULT, false)
                .is_err()
        );
        for dimensions in [(usize::MAX, 2, 0), (2, 0, usize::MAX), (usize::MAX, 0, 0)] {
            assert_eq!(
                SamplingCost::checked(dimensions.0, dimensions.1, dimensions.2).unwrap_err(),
                SamplingError::SizeOverflow
            );
        }
    }
}
