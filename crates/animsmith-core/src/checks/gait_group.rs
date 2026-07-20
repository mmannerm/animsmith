//! `gait-group` — clips in a declared directional blend ring must share
//! a gait phase (the stride anchor from the L−R foot-height
//! fundamental). If their cycles don't align, runtime blends between
//! them skate the feet. Members with too little L/R alternation are
//! excluded from the spread (their phase is noise). Unmeasurable work is
//! represented as a typed coverage gap, so the group's coherence is never
//! silently reported as verified.

use crate::check::{Check, CheckCtx};
use crate::checks::gait_gap;
use crate::evaluation::{
    Applicability, CheckOutput, CoverageGap, CoverageGapCode, EvaluationScope,
};
use crate::finding::{Finding, Severity};
use crate::metrics::{MIN_STRIDE_STEP_M, circular_phase_spread, foot_cycle_metrics};

pub struct GaitGroup;

impl Check for GaitGroup {
    fn id(&self) -> &'static str {
        "gait-group"
    }

    fn applicability(&self, ctx: &CheckCtx) -> Applicability {
        if ctx.config.gait_groups.is_empty() {
            Applicability::NotApplicable
        } else {
            Applicability::Applicable
        }
    }

    fn evaluate(&self, ctx: &CheckCtx) -> CheckOutput {
        let mut findings = Vec::new();
        let mut coverage = run_content(ctx, &mut findings);
        coverage
            .evaluated_scopes
            .insert(0, EvaluationScope::new("member_existence"));
        CheckOutput::from_coverage(findings, coverage.evaluated_scopes, coverage.gaps)
    }
}

#[derive(Default)]
struct GaitCoverage {
    evaluated_scopes: Vec<EvaluationScope>,
    gaps: Vec<CoverageGap>,
}

/// Run the content-evaluation portions of gait-group. Member existence always
/// runs; every group reports whether phase coherence ran or why it did not.
fn run_content(ctx: &CheckCtx, out: &mut Vec<Finding>) -> GaitCoverage {
    let roles_gap = gait_gap(ctx.roles);
    let mut coverage = GaitCoverage::default();

    for (group_name, group) in &ctx.config.gait_groups {
        let mut measured: Vec<(&str, f64)> = Vec::new();
        let mut existing_members = 0usize;
        for clip_name in &group.clips {
            let Some(index) = ctx.doc.clips.iter().position(|c| &c.name == clip_name) else {
                out.push(
                    Finding::new(
                        "gait-group",
                        Severity::Error,
                        format!("gait group '{group_name}' member not found in file"),
                    )
                    .clip(clip_name.clone()),
                );
                continue;
            };
            existing_members += 1;
            if roles_gap.is_some() {
                continue;
            }
            let gait = ctx
                .grid(index)
                .and_then(|grid| foot_cycle_metrics(&grid, ctx.roles, MIN_STRIDE_STEP_M));
            let Some(metrics) = gait else {
                coverage.gaps.push(
                    CoverageGap::new(
                        CoverageGapCode::MEASUREMENT_UNAVAILABLE,
                        "gait phase could not be measured",
                    )
                    .scope(EvaluationScope::new("phase_measurement").subject(clip_name)),
                );
                continue;
            };
            if metrics.lr_amplitude_m < group.min_lr_amplitude_m {
                coverage.gaps.push(
                    CoverageGap::new(
                        CoverageGapCode::MEASUREMENT_UNAVAILABLE,
                        format!(
                            "left/right gait amplitude {:.3} m is below the {:.3} m evidence floor",
                            metrics.lr_amplitude_m, group.min_lr_amplitude_m
                        ),
                    )
                    .scope(EvaluationScope::new("phase_measurement").subject(clip_name)),
                );
                continue;
            }
            if let Some(phase) = metrics.gait_phase {
                measured.push((clip_name, phase));
            } else {
                coverage.gaps.push(
                    CoverageGap::new(
                        CoverageGapCode::MEASUREMENT_UNAVAILABLE,
                        "gait phase could not be fitted from the sampled cycle",
                    )
                    .scope(EvaluationScope::new("phase_measurement").subject(clip_name)),
                );
            }
        }

        let phase_scope = EvaluationScope::new("phase_coherence").subject(group_name.clone());
        if existing_members > 0
            && let Some(gap) = &roles_gap
        {
            coverage.gaps.push(gap.clone().scope(phase_scope));
            continue;
        }

        if measured.len() >= 2 {
            let phases: Vec<f64> = measured.iter().map(|(_, p)| *p).collect();
            let spread = circular_phase_spread(&phases);
            if spread > group.max_gait_phase_spread {
                let listing = measured
                    .iter()
                    .map(|(n, p)| format!("{n}={p:.2}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                out.push(
                    Finding::new(
                        "gait-group",
                        Severity::Error,
                        format!(
                            "gait group '{group_name}': stride-anchor phases spread by \
                             {spread:.2} cycle (cap {cap:.2}) — directional blends \
                             between these clips will skate or pop. Measured: [{listing}]",
                            cap = group.max_gait_phase_spread,
                        ),
                    )
                    .measured(spread)
                    .expected(group.max_gait_phase_spread),
                );
            }
            coverage.evaluated_scopes.push(phase_scope.clone());
        }

        if !group.clips.is_empty() && (measured.len() < 2 || measured.len() < group.clips.len()) {
            let (code, message) = if measured.len() < 2 {
                (
                    CoverageGapCode::INSUFFICIENT_MEASURABLE_MEMBERS,
                    format!(
                        "gait group '{group_name}' has {} measurable phase member(s); at least two are required",
                        measured.len()
                    ),
                )
            } else {
                (
                    CoverageGapCode::MEMBERS_NOT_EVALUATED,
                    format!(
                        "gait group '{group_name}' evaluated {} of {} configured member(s)",
                        measured.len(),
                        group.clips.len()
                    ),
                )
            };
            coverage
                .gaps
                .push(CoverageGap::new(code, message).scope(phase_scope));
        }
    }
    coverage
}
