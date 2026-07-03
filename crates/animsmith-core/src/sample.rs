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
#[non_exhaustive]
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
fn segment(times: &[f32], t: f32) -> (usize, usize, f32) {
    let n = times.len();
    if t <= times[0] || n == 1 {
        return (0, 0, 0.0);
    }
    if t >= times[n - 1] {
        return (n - 1, n - 1, 0.0);
    }
    let k1 = times.partition_point(|&k| k <= t);
    let k0 = k1 - 1;
    let dt = times[k1] - times[k0];
    let u = if dt > 0.0 { (t - times[k0]) / dt } else { 0.0 };
    (k0, k1, u)
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
    let v0 = vals[track.value_index(k0)];
    if k0 == k1 {
        return v0;
    }
    let v1 = vals[track.value_index(k1)];
    match track.interpolation {
        Interpolation::Step => v0,
        Interpolation::Linear => v0.lerp(v1, u),
        Interpolation::CubicSpline => {
            let dt = track.times[k1] - track.times[k0];
            // out-tangent of k0, in-tangent of k1, scaled by dt per spec.
            let m0 = vals[3 * k0 + 2] * dt;
            let m1 = vals[3 * k1] * dt;
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
    let q0 = vals[track.value_index(k0)];
    if k0 == k1 {
        return q0.normalize();
    }
    let q1 = vals[track.value_index(k1)];
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
            let m0 = vals[3 * k0 + 2].to_array();
            let m1 = vals[3 * k1].to_array();
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
