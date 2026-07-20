//! `loop-seam` — the position wrap discontinuity of a looping clip's
//! feet (relative to hips), normalized by the seam-adjacent in-clip
//! steps. A clean cyclic clip wraps by ≈ one locally-normal step
//! (ratio ≈ 1); a clip whose cut drops the loop closure pops well above
//! that. Judged only on clips declared `loop = true`; the ratio itself
//! is always available via `measure`.

use crate::check::{Check, CheckCtx};
use crate::checks::gait_gap;
use crate::evaluation::{
    Applicability, CheckOutput, CoverageGap, CoverageGapCode, EvaluationScope,
};
use crate::finding::{Finding, Severity};
use crate::metrics::foot_cycle_metrics;

/// Default ratio cap. A clean loop sits near 1.0; materially above
/// that is a seam pop.
pub const DEFAULT_MAX_RATIO: f64 = 1.5;

pub struct LoopSeam;

impl Check for LoopSeam {
    fn id(&self) -> &'static str {
        "loop-seam"
    }

    fn applicability(&self, ctx: &CheckCtx) -> Applicability {
        if ctx
            .clip_expectations()
            .iter()
            .any(|expectations| expectations.looping == Some(true))
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
        let settings = ctx.config.check_settings(self.id());
        let max_ratio = settings.max_ratio.unwrap_or(DEFAULT_MAX_RATIO);
        let min_stride_step_m = ctx.config.loop_seam_min_stride_step_m();

        for (index, clip) in ctx.doc.clips.iter().enumerate() {
            if ctx.expectations(index).looping != Some(true) {
                continue;
            }
            let scope = EvaluationScope::new("loop_seam").subject(&clip.name);
            if let Some(gap) = gait_gap(ctx.roles) {
                gaps.push(gap.scope(scope));
                continue;
            }
            let Some(grid) = ctx.grid(index) else {
                gaps.push(
                    CoverageGap::new(
                        CoverageGapCode::MEASUREMENT_UNAVAILABLE,
                        "clip is too short to sample a loop cycle",
                    )
                    .scope(scope),
                );
                continue;
            };
            let Some(metrics) = foot_cycle_metrics(&grid, ctx.roles, min_stride_step_m) else {
                gaps.push(
                    CoverageGap::new(
                        CoverageGapCode::MEASUREMENT_UNAVAILABLE,
                        "foot-cycle metrics could not be produced",
                    )
                    .scope(scope),
                );
                continue;
            };
            evaluated_scopes.push(scope);
            let Some(ratio) = metrics.loop_seam_ratio else {
                continue; // stationary loop: the completed judgment has no ratio
            };
            if ratio > max_ratio {
                findings.push(
                    Finding::new(
                        self.id(),
                        Severity::Error,
                        format!(
                            "loop seam pops: wrap discontinuity is {ratio:.2}× the \
                             neighbouring in-clip step (cap {max_ratio:.2}) — the clip \
                             does not close its cycle"
                        ),
                    )
                    .clip(&clip.name)
                    .time(clip.duration_s as f32)
                    .measured(ratio)
                    .expected(max_ratio),
                );
            }
        }
        match (evaluated_scopes.is_empty(), gaps.is_empty()) {
            (_, true) => CheckOutput::complete_scoped(findings, evaluated_scopes),
            (true, false) => CheckOutput::not_evaluated(gaps),
            (false, false) => CheckOutput::partial(findings, evaluated_scopes, gaps),
        }
    }
}
