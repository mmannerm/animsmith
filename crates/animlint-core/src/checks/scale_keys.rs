//! `scale-keys` — scale animation on a skeletal clip is usually an
//! export accident (a stray keyframe, a unit-conversion bake) and many
//! engine rigs ignore or mishandle it. Presence is a warning;
//! non-uniform scale (which most runtimes and retargeters actively
//! break on) is called out separately.

use super::tracks;
use crate::check::{Check, CheckCtx};
use crate::finding::{Finding, Severity};
use crate::model::Property;

/// Relative component spread beyond which a scale key counts as
/// non-uniform.
pub const NON_UNIFORM_TOLERANCE: f32 = 1e-4;

/// Deviation from 1.0 beyond which scale keys count as actually
/// scaling (an all-ones track is merely bloat; `constant-track` owns
/// that).
pub const UNIT_TOLERANCE: f32 = 1e-4;

pub struct ScaleKeys;

impl Check for ScaleKeys {
    fn id(&self) -> &'static str {
        "scale-keys"
    }

    fn run(&self, ctx: &CheckCtx, out: &mut Vec<Finding>) {
        let doc = ctx.doc;
        for (clip, bone, track) in tracks(doc) {
            if track.property != Property::Scale {
                continue;
            }
            let mut scaling = false;
            let mut non_uniform_at: Option<usize> = None;
            for k in 0..track.key_count() {
                let Some(v) = track.key_vec3(k) else { continue };
                if !v.is_finite() {
                    continue;
                }
                if (v - glam::Vec3::ONE).abs().max_element() > UNIT_TOLERANCE {
                    scaling = true;
                }
                if (v.max_element() - v.min_element()).abs() > NON_UNIFORM_TOLERANCE
                    && non_uniform_at.is_none()
                {
                    non_uniform_at = Some(k);
                }
            }
            if scaling {
                out.push(
                    Finding::new(
                        self.id(),
                        Severity::Warning,
                        "scale animation present — verify it is intentional; many rigs \
                         and retargeters mishandle animated scale",
                    )
                    .clip(clip)
                    .bone(bone),
                );
            }
            if let Some(k) = non_uniform_at {
                out.push(
                    Finding::new(
                        self.id(),
                        Severity::Warning,
                        format!("non-uniform scale key (first at key {k})"),
                    )
                    .clip(clip)
                    .bone(bone)
                    .time(track.times[k]),
                );
            }
        }
    }
}
