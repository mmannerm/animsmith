//! `scale-keys` — scale animation on a skeletal clip is usually an
//! export accident (a stray keyframe, a unit-conversion bake) and many
//! engine rigs ignore or mishandle it. Presence is a warning;
//! non-uniform scale (which most runtimes and retargeters actively
//! break on) is called out separately.

use super::tracks;
use super::vec3_trajectory::analyze;
use crate::check::{Check, CheckCtx};
use crate::evaluation::CheckOutput;
use crate::finding::{Finding, Severity};
use crate::model::Property;

/// Component range beyond which scale animation is temporally varying.
pub const TEMPORAL_VARIATION_TOLERANCE: f32 = 1e-4;

pub struct ScaleKeys;

impl Check for ScaleKeys {
    fn id(&self) -> &'static str {
        "scale-keys"
    }

    fn evaluate(&self, ctx: &CheckCtx) -> Result<CheckOutput, crate::EvaluationError> {
        let mut findings = Vec::new();
        let doc = ctx.doc;
        for (clip, bone, track) in tracks(doc) {
            if track.property != Property::Scale {
                continue;
            }
            let Some(trajectory) = analyze(track) else {
                continue;
            };
            if trajectory.varies(TEMPORAL_VARIATION_TOLERANCE) {
                findings.push(
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
        }
        Ok(CheckOutput::from_coverage(findings, Vec::new(), Vec::new()))
    }
}
