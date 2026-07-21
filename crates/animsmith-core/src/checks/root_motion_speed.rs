//! `root-motion-speed` — a clip's declared locomotion speed must match
//! its measured horizontal root displacement over its duration.
//! Runtimes scale playback by the declared speed to keep foot plants
//! locked to world velocity; a stale pin plays the clip visibly too
//! fast or too slow.

use crate::check::{Check, CheckCtx};
use crate::checks::root_motion_gap;
use crate::evaluation::{
    Applicability, CheckOutput, CoverageGap, CoverageGapCode, EvaluationScope, EvaluationScopeCode,
};
use crate::finding::{Finding, Severity};
use crate::metrics::root_motion_speed_mps;

/// A declared speed with a measurement under this (m/s) is a stray pin:
/// the clip carries no meaningful root motion at all.
pub const STRAY_PIN_FLOOR_MPS: f64 = 0.5;

pub struct RootMotionSpeed;

impl RootMotionSpeed {
    /// Clips whose declared `speed_mps` this check judges (root-motion
    /// clips — treadmill speeds belong to `foot-slide`).
    fn has_pending_work(ctx: &CheckCtx) -> bool {
        ctx.clip_expectations()
            .iter()
            .any(|e| e.speed_mps.is_some() && e.in_place != Some(true))
    }
}

impl Check for RootMotionSpeed {
    fn id(&self) -> &'static str {
        "root-motion-speed"
    }

    fn applicability(&self, ctx: &CheckCtx) -> Applicability {
        if Self::has_pending_work(ctx) {
            Applicability::Applicable
        } else {
            Applicability::NotApplicable
        }
    }

    fn evaluate(&self, ctx: &CheckCtx) -> CheckOutput {
        let mut findings = Vec::new();
        let mut evaluated_scopes = Vec::new();
        let mut gaps = Vec::new();
        for (index, clip) in ctx.doc.clips.iter().enumerate() {
            let expectations = ctx.expectations(index);
            let Some(pin) = expectations.speed_mps else {
                continue;
            };
            if expectations.in_place == Some(true) {
                // A treadmill clip's declared speed describes the
                // stance sweep, not root displacement — `foot-slide`
                // validates it there.
                continue;
            }
            let scope =
                EvaluationScope::new(EvaluationScopeCode::ROOT_MOTION_SPEED).subject(&clip.name);
            if let Some(gap) = root_motion_gap(ctx.roles) {
                gaps.push(gap.scope(scope));
                continue;
            }
            let measured = ctx
                .grid(index)
                .and_then(|grid| root_motion_speed_mps(&grid, ctx.roles));
            let Some(measured) = measured else {
                gaps.push(
                    CoverageGap::new(
                        CoverageGapCode::MEASUREMENT_UNAVAILABLE,
                        "root-motion speed could not be measured",
                    )
                    .scope(scope),
                );
                continue;
            };
            evaluated_scopes.push(scope);
            if measured < STRAY_PIN_FLOOR_MPS && pin.value >= STRAY_PIN_FLOOR_MPS {
                findings.push(
                    Finding::new(
                        self.id(),
                        Severity::Error,
                        format!(
                            "declared speed {:.2} m/s but the clip carries almost no \
                             root motion ({measured:.2} m/s) — stray pin, or an \
                             in-place clip declared as root-motion",
                            pin.value
                        ),
                    )
                    .clip(&clip.name)
                    .measured(measured)
                    .expected(pin.value),
                );
                continue;
            }
            if (measured - pin.value).abs() > pin.tolerance {
                findings.push(
                    Finding::new(
                        self.id(),
                        Severity::Error,
                        format!(
                            "measured root-motion speed {measured:.2} m/s disagrees with \
                             the declared {:.2} ± {:.2} m/s — playback scaled by this pin \
                             will slide or moonwalk",
                            pin.value, pin.tolerance
                        ),
                    )
                    .clip(&clip.name)
                    .measured(measured)
                    .expected(pin.value),
                );
            }
        }
        CheckOutput::from_coverage(findings, evaluated_scopes, gaps)
    }
}
