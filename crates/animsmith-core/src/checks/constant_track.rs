//! `constant-track` — a multi-key track whose values never move is
//! export bloat (unbaked rig channels, "key everything" exports). Note
//! severity: harmless at runtime, wasteful on disk and in blends.
//! "Unexpectedly constant" (a bone that *should* move) is the
//! `frozen-bone` check and needs per-clip expectations (M1).

use super::tracks;
use super::vec3_trajectory::analyze;
use crate::check::{Check, CheckCtx};
use crate::evaluation::CheckOutput;
use crate::finding::{Finding, Severity};
use crate::model::TrackValues;

/// Positional/scale spread (in source units) below which a track is
/// constant.
pub const VEC3_TOLERANCE: f32 = 1e-4;

/// Rotation deviation below which a track is constant (radians;
/// ~0.06°).
pub const QUAT_TOLERANCE_RAD: f32 = 1e-3;

pub struct ConstantTrack;

impl Check for ConstantTrack {
    fn id(&self) -> &'static str {
        "constant-track"
    }

    fn evaluate(&self, ctx: &CheckCtx) -> CheckOutput {
        let mut findings = Vec::new();
        let doc = ctx.doc;
        for (clip, bone, track) in tracks(doc) {
            if track.key_count() < 2 {
                continue; // a single-key track is a compact pin, not bloat
            }
            let constant = match &track.values {
                TrackValues::Vec3s(_) => {
                    analyze(track).is_some_and(|trajectory| !trajectory.varies(VEC3_TOLERANCE))
                }
                TrackValues::Quats(_) => {
                    let Some(first) = track.key_quat(0) else {
                        continue;
                    };
                    if !first.is_finite() || first.length_squared() == 0.0 {
                        continue; // nan/quat-norm own broken data
                    }
                    let keyed_values_constant = (1..track.key_count()).all(|k| {
                        track.key_quat(k).is_some_and(|q| {
                            q.is_finite()
                                && first.normalize().angle_between(q.normalize())
                                    <= QUAT_TOLERANCE_RAD
                        })
                    });
                    keyed_values_constant
                        && (track.interpolation != crate::model::Interpolation::CubicSpline
                            || cubic_quaternion_tangents_are_zero(track))
                }
            };
            if constant {
                findings.push(
                    Finding::new(
                        self.id(),
                        Severity::Note,
                        format!(
                            "{} track has {} keys but never moves — export bloat",
                            track.property.as_str(),
                            track.key_count()
                        ),
                    )
                    .clip(clip)
                    .bone(bone),
                );
            }
        }
        CheckOutput::from_coverage(findings, Vec::new(), Vec::new())
    }
}

/// Cubic quaternion key values can agree while their tangents still generate
/// a changing component-wise Hermite curve. Treat that representation as
/// redundant only for a genuinely flat hold.
fn cubic_quaternion_tangents_are_zero(track: &crate::model::Track) -> bool {
    let TrackValues::Quats(values) = &track.values else {
        return false;
    };
    let zero = glam::Quat::from_xyzw(0.0, 0.0, 0.0, 0.0);
    (0..track.key_count())
        .all(|key| values.get(3 * key) == Some(&zero) && values.get(3 * key + 2) == Some(&zero))
}
