//! `non-uniform-scale` — unequal scale components are risky for many rigs and
//! retargeters, whether they are animated or held in a constant channel.

use super::tracks;
use super::vec3_trajectory::analyze;
use crate::check::{Check, CheckCtx};
use crate::evaluation::CheckOutput;
use crate::finding::{Finding, Severity};
use crate::model::Property;

/// Relative component spread beyond which a scale trajectory counts as
/// non-uniform.
pub const NON_UNIFORM_TOLERANCE: f32 = 1e-4;

pub struct NonUniformScale;

impl Check for NonUniformScale {
    fn id(&self) -> &'static str {
        "non-uniform-scale"
    }

    fn evaluate(&self, ctx: &CheckCtx) -> CheckOutput {
        let mut findings = Vec::new();
        for (clip, bone, track) in tracks(ctx.doc) {
            if track.property != Property::Scale {
                continue;
            }
            let Some(trajectory) = analyze(track) else {
                continue;
            };
            if trajectory.is_non_uniform(NON_UNIFORM_TOLERANCE) {
                findings.push(
                    Finding::new(
                        self.id(),
                        Severity::Warning,
                        "non-uniform scale present — verify it is intentional; many rigs and \
                         retargeters mishandle non-uniform scale",
                    )
                    .clip(clip)
                    .bone(bone)
                    .time(trajectory.max_pairwise_time()),
                );
            }
        }
        CheckOutput::from_coverage(findings, Vec::new(), Vec::new())
    }
}
