//! Pipeline-mechanical clip transforms, ported from the incubating
//! bake's Python: frame-window slicing, hold-extension, and gait-anchor
//! rotation. Scope rule (DESIGN.md §1): animsmith may rewrite a clip
//! only in ways whose correctness its own checks can verify.

use crate::metrics::foot_cycle_metrics;
use crate::model::{Clip, Interpolation, Skeleton, Track, TrackValues};
use crate::profile::ResolvedRoles;
use crate::sample::{TrackSample, sample_clip, sample_track};

/// Keep only the keys inside `[start, end]` seconds (with a half-frame
/// epsilon at `fps` absorbing float drift from earlier retimings) and
/// retime them so the window starts at 0. Cubic tangent triplets move
/// with their keys. The clip duration becomes `end - start`.
pub fn slice(clip: &mut Clip, start_s: f64, end_s: f64, fps: f64) {
    let eps = (0.5 / fps) as f32;
    let (start, end) = (start_s as f32, end_s as f32);
    for track in &mut clip.tracks {
        let keep: Vec<usize> = (0..track.key_count())
            .filter(|&k| track.times[k] >= start - eps && track.times[k] <= end + eps)
            .collect();
        track.times = keep
            .iter()
            .map(|&k| (track.times[k] - start).max(0.0))
            .collect();
        let per_key = match track.interpolation {
            Interpolation::CubicSpline => 3,
            _ => 1,
        };
        match &mut track.values {
            TrackValues::Vec3s(v) => {
                let old = std::mem::take(v);
                *v = keep
                    .iter()
                    .flat_map(|&k| old[k * per_key..(k + 1) * per_key].to_vec())
                    .collect();
            }
            TrackValues::Quats(v) => {
                let old = std::mem::take(v);
                *v = keep
                    .iter()
                    .flat_map(|&k| old[k * per_key..(k + 1) * per_key].to_vec())
                    .collect();
            }
        }
    }
    clip.duration_s = (end_s - start_s).max(0.0);
    clip.tracks.retain(|t| t.key_count() > 0);
}

/// Append one key per track duplicating its final value `hold_s`
/// seconds after its last key (a linear hold — charge/block poses).
/// The clip duration extends to the longest held end.
pub fn hold_extend(clip: &mut Clip, hold_s: f64) {
    for track in &mut clip.tracks {
        let Some(&last) = track.times.last() else {
            continue;
        };
        let key = track.key_count() - 1;
        track.times.push(last + hold_s as f32);
        match &mut track.values {
            TrackValues::Vec3s(v) => {
                let value = v[track.interpolation.value_index_static(key)];
                match track.interpolation {
                    Interpolation::CubicSpline => {
                        // Zero tangents: a flat Hermite hold. Also zero
                        // the previous key's out-tangent so the hold
                        // segment stays flat.
                        v[key * 3 + 2] = glam::Vec3::ZERO;
                        v.extend_from_slice(&[glam::Vec3::ZERO, value, glam::Vec3::ZERO]);
                    }
                    _ => v.push(value),
                }
            }
            TrackValues::Quats(v) => {
                let value = v[track.interpolation.value_index_static(key)];
                match track.interpolation {
                    Interpolation::CubicSpline => {
                        v[key * 3 + 2] = glam::Quat::from_xyzw(0.0, 0.0, 0.0, 0.0);
                        v.extend_from_slice(&[
                            glam::Quat::from_xyzw(0.0, 0.0, 0.0, 0.0),
                            value,
                            glam::Quat::from_xyzw(0.0, 0.0, 0.0, 0.0),
                        ]);
                    }
                    _ => v.push(value),
                }
            }
        }
        clip.duration_s = clip.duration_s.max((last + hold_s as f32) as f64);
    }
}

/// Outcome of [`align_gait_anchor`].
#[derive(Debug, Clone)]
pub struct GaitAlignOutcome {
    /// The measured stride-anchor phase before rotation.
    pub phase_before: f64,
    /// The phase after rotation (should sit near 0).
    pub phase_after: f64,
    /// Loop-seam ratio after rotation (the chosen candidate's wrap).
    pub seam_after: Option<f64>,
    /// The whole-frame offset (−1/0/+1) that produced the cleanest wrap.
    pub frame_offset: i32,
}

/// Rotate a cyclic clip in time so its measured stride anchor (the
/// trough of the L−R foot-height fundamental) lands at clip time 0.
///
/// Semantics ported from the reference bake: the cycle period is
/// `duration + 1/fps` (an open loop's wrap step is a real frame of the
/// stride); the shift is quantized to whole frames so every resample
/// lands on an existing key; each channel keeps its times and gets its
/// output values replaced by the channel sampled at
/// `(t + shift) mod period`; channels with fewer than 3 keys are left
/// alone. Because a ±1-frame shift stays inside phase tolerance but
/// moves *where the wrap lands*, all three candidates are tried and
/// the one with the cleanest wrap (lowest seam ratio) wins.
pub fn align_gait_anchor(
    skeleton: &Skeleton,
    clip: &mut Clip,
    roles: &ResolvedRoles,
    fps: f64,
) -> Result<GaitAlignOutcome, String> {
    let measure = |c: &Clip| -> Option<(f64, Option<f64>, f64)> {
        let frames = crate::metrics::metric_frame_count(c)?;
        let grid = sample_clip(skeleton, c, frames);
        let m = foot_cycle_metrics(&grid, roles)?;
        Some((m.gait_phase?, m.loop_seam_ratio, m.lr_amplitude_m))
    };
    let Some((phase_before, _, amplitude)) = measure(clip) else {
        return Err(
            "no usable stride anchor (hips/foot roles unresolved or clip too short)".into(),
        );
    };
    if amplitude < 0.03 {
        return Err(format!(
            "no usable stride anchor (L−R amplitude {amplitude:.4} m) — a ring clip must \
             alternate its feet for anchor alignment to mean anything"
        ));
    }

    let original = clip.clone();
    let mut best: Option<(f64, GaitAlignOutcome, Clip)> = None;
    for frame_offset in [0i32, -1, 1] {
        let mut candidate = original.clone();
        rotate_values(&mut candidate, phase_before, fps, frame_offset);
        let Some((phase_after, seam_after, _)) = measure(&candidate) else {
            continue;
        };
        // Rank by wrap cleanliness; a missing seam (no stride at the
        // wrap) should not happen on a ring clip — rank it last.
        let rank = seam_after.unwrap_or(f64::MAX);
        if best.as_ref().is_none_or(|(r, _, _)| rank < *r) {
            best = Some((
                rank,
                GaitAlignOutcome {
                    phase_before,
                    phase_after,
                    seam_after,
                    frame_offset,
                },
                candidate,
            ));
        }
    }
    let Some((_, outcome, rotated)) = best else {
        return Err("no rotation candidate was measurable".into());
    };
    *clip = rotated;
    Ok(outcome)
}

/// Replace each dense channel's output values with the channel sampled
/// at `(t + shift) mod period`; times untouched.
fn rotate_values(clip: &mut Clip, phase: f64, fps: f64, frame_offset: i32) {
    let duration = clip
        .tracks
        .iter()
        .map(Track::end_time)
        .fold(0.0f32, f32::max) as f64;
    if duration <= 0.0 {
        return;
    }
    let period = duration + 1.0 / fps;
    let mut shift = ((phase * period * fps).round() + frame_offset as f64) / fps;
    shift = shift.rem_euclid(period);

    for track in &mut clip.tracks {
        if track.key_count() < 3 || track.interpolation == Interpolation::CubicSpline {
            continue; // constants are rotation-invariant; cubic needs resampling
        }
        let sampled: Vec<TrackSample> = track
            .times
            .iter()
            .map(|&t| sample_track(track, ((t as f64 + shift) % period) as f32))
            .collect();
        match &mut track.values {
            TrackValues::Vec3s(v) => {
                for (slot, s) in v.iter_mut().zip(&sampled) {
                    if let TrackSample::Vec3(x) = s {
                        *slot = *x;
                    }
                }
            }
            TrackValues::Quats(v) => {
                for (slot, s) in v.iter_mut().zip(&sampled) {
                    if let TrackSample::Quat(q) = s {
                        *slot = *q;
                    }
                }
            }
        }
    }
}
