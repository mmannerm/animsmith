//! The sampled layer: what a game runtime sees. A [`PoseGrid`] is a
//! uniform time grid over `[0, duration]` sampled with glTF-spec
//! interpolation semantics (lerp for T/S, shortest-path slerp for R,
//! STEP hold, cubic-spline Hermite; clamp at the ends), then FK'd to
//! model space.
//!
//! For clips declared looping, the wrap pair is `(last frame, frame 0)`
//! — the seam definition every loop check shares.
//!
//! FK currently includes root bones' own transforms; excluding an
//! asset-centering scene root is handled at the rig-profile level
//! (`Role::Root`, M1).

use crate::model::{Clip, Interpolation, Skeleton, Track, TrackValues, Transform};
use glam::{Mat4, Quat, Vec3};

/// Model-space and local poses for every (frame, bone) of one clip.
#[derive(Debug)]
pub struct PoseGrid {
    /// Uniform sample times, `times[0] == 0`, `times[last] == duration`.
    pub times: Vec<f32>,
    bone_count: usize,
    /// Frame-major: `local[frame * bone_count + bone]`.
    local: Vec<Transform>,
    model: Vec<Mat4>,
}

impl PoseGrid {
    pub fn frame_count(&self) -> usize {
        self.times.len()
    }

    pub fn bone_count(&self) -> usize {
        self.bone_count
    }

    pub fn local(&self, frame: usize, bone: usize) -> Transform {
        self.local[frame * self.bone_count + bone]
    }

    pub fn model(&self, frame: usize, bone: usize) -> Mat4 {
        self.model[frame * self.bone_count + bone]
    }

    /// Model-space joint position.
    pub fn model_position(&self, frame: usize, bone: usize) -> Vec3 {
        self.model(frame, bone).w_axis.truncate()
    }
}

/// Default grid resolution for a clip: the max keyframe count across its
/// tracks (minimum 2), so no authored key falls between samples.
pub fn default_frame_count(clip: &Clip) -> usize {
    clip.tracks
        .iter()
        .map(Track::key_count)
        .max()
        .unwrap_or(2)
        .max(2)
}

/// Sample `clip` on a uniform `frames`-sample grid and FK to model space.
pub fn sample_clip(skeleton: &Skeleton, clip: &Clip, frames: usize) -> PoseGrid {
    let frames = frames.max(2);
    let nb = skeleton.bones.len();
    let duration = clip.duration_s as f32;
    let times: Vec<f32> = (0..frames)
        .map(|i| duration * i as f32 / (frames - 1) as f32)
        .collect();

    let mut local = vec![Transform::IDENTITY; frames * nb];
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

    let mut model = vec![Mat4::IDENTITY; frames * nb];
    for f in 0..frames {
        for (b, bone) in skeleton.bones.iter().enumerate() {
            let m = local[f * nb + b].to_mat4();
            model[f * nb + b] = match bone.parent {
                Some(p) => model[f * nb + p] * m,
                None => m,
            };
        }
    }

    PoseGrid {
        times,
        bone_count: nb,
        local,
        model,
    }
}

/// One track's sampled value at a time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrackSample {
    Vec3(Vec3),
    Quat(Quat),
}

/// Sample a single track at `t` with the same semantics the grid uses
/// (clamp at ends, glTF interpolation, shortest-path slerp).
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

/// Clamped value fetch: loaders enforce times/values length agreement,
/// but `Document`s are plain data an embedder can build by hand — an
/// inconsistent track samples as the type default, never a panic.
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
    use crate::model::Property;

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
}
