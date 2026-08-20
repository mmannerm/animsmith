//! `in-place` — a clip's declared horizontal movement owner must match its
//! measured root motion. Gameplay-owned XZ motion expects no baked travel;
//! animation-owned XZ motion expects root travel. A mismatch makes the
//! character glide or run in place at runtime.

use crate::check::{Check, CheckCtx};
use crate::checks::root_motion_gap;
use crate::config::MovementOwner;
use crate::evaluation::{
    Applicability, CheckOutput, CoverageGap, CoverageGapCode, EvaluationScope, EvaluationScopeCode,
};
use crate::finding::{Finding, Severity};
use crate::metrics::root_motion_speed_mps;

/// Measured horizontal root speed above this counts as travelling.
pub const TRAVEL_THRESHOLD_MPS: f64 = 0.5;

pub struct InPlace;

impl Check for InPlace {
    fn id(&self) -> &'static str {
        "in-place"
    }

    fn applicability(&self, ctx: &CheckCtx) -> Applicability {
        if ctx
            .clip_expectations()
            .iter()
            .any(|expectations| expectations.movement_owner_xz.is_some())
        {
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
            let Some(owner) = ctx.expectations(index).movement_owner_xz else {
                continue;
            };
            let expected_in_place = owner == MovementOwner::Gameplay;
            let scope = EvaluationScope::new(EvaluationScopeCode::TRAVEL_MODE).subject(&clip.name);
            if let Some(gap) = root_motion_gap(ctx.roles) {
                gaps.push(gap.scope(scope));
                continue;
            }
            let measured = ctx
                .grid(index)
                .and_then(|grid| root_motion_speed_mps(&grid, ctx.roles));
            let Some(speed) = measured else {
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
            let travels = speed >= TRAVEL_THRESHOLD_MPS;
            if expected_in_place && travels {
                findings.push(
                    Finding::new(
                        self.id(),
                        Severity::Error,
                        format!(
                            "declared in-place but the root travels at {speed:.2} m/s — \
                             the character will glide at runtime"
                        ),
                    )
                    .clip(&clip.name)
                    .measured(speed)
                    .expected(0.0f64),
                );
            } else if owner == MovementOwner::Animation && !travels {
                findings.push(
                    Finding::new(
                        self.id(),
                        Severity::Error,
                        format!(
                            "declared root-motion but the root is stationary \
                             ({speed:.2} m/s) — the character will run in place"
                        ),
                    )
                    .clip(&clip.name)
                    .measured(speed),
                );
            }
        }
        CheckOutput::from_coverage(findings, evaluated_scopes, gaps)
    }
}
