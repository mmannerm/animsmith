//! `constant-nonunit-scale` — a retained non-unit scale channel can matter to
//! pipelines even when it has no temporal variation.

use super::tracks;
use super::vec3_trajectory::analyze;
use crate::check::{Check, CheckCtx};
use crate::evaluation::CheckOutput;
use crate::finding::{Finding, Severity};
use crate::model::Property;

/// Deviation from unit scale beyond which a constant channel is non-unit.
pub const UNIT_TOLERANCE: f32 = 1e-4;

pub struct ConstantNonunitScale;

impl Check for ConstantNonunitScale {
    fn id(&self) -> &'static str {
        "constant-nonunit-scale"
    }

    fn enabled_by_default(&self) -> bool {
        false
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
            if !trajectory.varies(UNIT_TOLERANCE)
                && trajectory.max_unit_deviation() > UNIT_TOLERANCE
            {
                findings.push(
                    Finding::new(
                        self.id(),
                        Severity::Note,
                        "constant non-unit scale channel present",
                    )
                    .clip(clip)
                    .bone(bone),
                );
            }
        }
        CheckOutput::from_coverage(findings, Vec::new(), Vec::new())
    }
}
