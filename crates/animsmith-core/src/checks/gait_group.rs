//! `gait-group` — clips in a declared directional blend ring must share
//! a gait phase (the stride anchor from the L−R foot-height
//! fundamental). If their cycles don't align, runtime blends between
//! them skate the feet. Members with too little L/R alternation are
//! excluded from the spread (their phase is noise); a member whose gait
//! cannot be measured at all is an error, so the group's coherence is
//! never silently unverified.

use crate::check::{Check, CheckCtx, Readiness};
use crate::checks::gait_readiness;
use crate::evaluation::{CheckOutput, CoverageGap, EvaluationScope};
use crate::finding::{Finding, Severity};
use crate::metrics::{MIN_STRIDE_STEP_M, circular_phase_spread, foot_cycle_metrics};

pub struct GaitGroup;

impl Check for GaitGroup {
    fn id(&self) -> &'static str {
        "gait-group"
    }

    // gait-group is always Ready when groups exist: member existence is
    // config validation that needs no rig roles, so it must run even on
    // an unresolved rig. The role-dependent metric work inside `run` is
    // gated separately, emitting one exempt skip-note.
    fn readiness(&self, ctx: &CheckCtx) -> Readiness {
        if ctx.config.gait_groups.is_empty() {
            Readiness::Idle
        } else {
            Readiness::Ready
        }
    }

    fn run(&self, ctx: &CheckCtx, out: &mut Vec<Finding>) {
        if let Some(gap) = run_content(ctx, out) {
            out.push(
                Finding::new(
                    self.id(),
                    Severity::Note,
                    format!("skipped: {}", gap.message),
                )
                .as_diagnostic(),
            );
        }
    }

    fn evaluate(&self, ctx: &CheckCtx) -> CheckOutput {
        let mut findings = Vec::new();
        let gap = run_content(ctx, &mut findings);
        let mut output = CheckOutput {
            findings,
            evaluated_scopes: vec![EvaluationScope::new("member_existence")],
            gaps: Vec::new(),
        };
        if let Some(gap) = gap {
            output
                .gaps
                .push(gap.scope(EvaluationScope::new("phase_coherence")));
        } else {
            output
                .evaluated_scopes
                .push(EvaluationScope::new("phase_coherence"));
        }
        output
    }
}

/// Run the content-evaluation portions of gait-group. Member existence always
/// runs; unresolved roles become one typed gap for phase-coherence work.
fn run_content(ctx: &CheckCtx, out: &mut Vec<Finding>) -> Option<CoverageGap> {
    let roles_gap = match gait_readiness(ctx.roles) {
        Readiness::Ready => None,
        Readiness::Skipped(gap) => Some(gap),
        Readiness::Idle => None,
    };

    for (group_name, group) in &ctx.config.gait_groups {
        let mut measured: Vec<(&str, f64)> = Vec::new();
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
            if roles_gap.is_some() {
                continue;
            }
            let gait = ctx
                .grid(index)
                .and_then(|grid| foot_cycle_metrics(&grid, ctx.roles, MIN_STRIDE_STEP_M));
            let Some(metrics) = gait else {
                out.push(
                    Finding::new(
                        "gait-group",
                        Severity::Error,
                        format!(
                            "gait group '{group_name}' member has no measurable gait \
                             (clip too short) — the group's coherence is unverified"
                        ),
                    )
                    .clip(clip_name.clone()),
                );
                continue;
            };
            if metrics.lr_amplitude_m < group.min_lr_amplitude_m {
                continue;
            }
            if let Some(phase) = metrics.gait_phase {
                measured.push((clip_name, phase));
            }
        }
        if measured.len() < 2 {
            continue;
        }
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
    }
    roles_gap
}
