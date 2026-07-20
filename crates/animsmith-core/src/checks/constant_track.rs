//! `constant-track` — a multi-key track whose values never move is
//! export bloat (unbaked rig channels, "key everything" exports). Note
//! severity: harmless at runtime, wasteful on disk and in blends.
//! "Unexpectedly constant" (a bone that *should* move) is the
//! `frozen-bone` check and needs per-clip expectations (M1).

use super::tracks;
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
                    let Some(first) = track.key_vec3(0) else {
                        continue;
                    };
                    (1..track.key_count()).all(|k| {
                        track
                            .key_vec3(k)
                            .is_some_and(|v| (v - first).abs().max_element() <= VEC3_TOLERANCE)
                    })
                }
                TrackValues::Quats(_) => {
                    let Some(first) = track.key_quat(0) else {
                        continue;
                    };
                    if !first.is_finite() || first.length_squared() == 0.0 {
                        continue; // nan/quat-norm own broken data
                    }
                    (1..track.key_count()).all(|k| {
                        track.key_quat(k).is_some_and(|q| {
                            q.is_finite()
                                && first.normalize().angle_between(q.normalize())
                                    <= QUAT_TOLERANCE_RAD
                        })
                    })
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
        CheckOutput::complete(findings)
    }
}
