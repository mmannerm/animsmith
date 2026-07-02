//! `gait-group` — clips in a declared directional blend ring must share
//! a gait phase (the stride anchor from the L−R foot-height
//! fundamental). If their cycles don't align, runtime blends between
//! them skate the feet. Members with too little L/R alternation are
//! excluded from the spread (their phase is noise); a member whose gait
//! cannot be measured at all is an error, so the group's coherence is
//! never silently unverified.

use crate::check::{Check, CheckCtx};
use crate::finding::{Finding, Severity};
use crate::metrics::{circular_phase_spread, foot_cycle_metrics};

pub struct GaitGroup;

impl Check for GaitGroup {
    fn id(&self) -> &'static str {
        "gait-group"
    }

    fn run(&self, ctx: &CheckCtx, out: &mut Vec<Finding>) {
        for (group_name, group) in &ctx.config.groups {
            let mut measured: Vec<(&str, f64)> = Vec::new();
            for clip_name in &group.clips {
                let Some(index) = ctx.doc.clips.iter().position(|c| &c.name == clip_name) else {
                    out.push(
                        Finding::new(
                            self.id(),
                            Severity::Error,
                            format!("gait group '{group_name}' member not found in file"),
                        )
                        .clip(clip_name.clone()),
                    );
                    continue;
                };
                let gait = ctx
                    .grid(index)
                    .and_then(|grid| foot_cycle_metrics(&grid, ctx.roles));
                let Some(metrics) = gait else {
                    out.push(
                        Finding::new(
                            self.id(),
                            Severity::Error,
                            format!(
                                "gait group '{group_name}' member has no measurable gait \
                                 (hips/foot roles unresolved or clip too short) — the \
                                 group's coherence is unverified"
                            ),
                        )
                        .clip(clip_name.clone()),
                    );
                    continue;
                };
                if metrics.lr_amplitude_m < group.min_lr_amplitude_m {
                    continue; // phase-confidence floor: too little alternation
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
                        self.id(),
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
    }
}
