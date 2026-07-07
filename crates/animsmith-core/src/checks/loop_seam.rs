//! `loop-seam` — the position wrap discontinuity of a looping clip's
//! feet (relative to hips), normalized by the seam-adjacent in-clip
//! steps. A clean cyclic clip wraps by ≈ one locally-normal step
//! (ratio ≈ 1); a clip whose cut drops the loop closure pops well above
//! that. Judged only on clips declared `loop = true`; the ratio itself
//! is always available via `measure`.

use crate::check::{Check, CheckCtx, Readiness};
use crate::checks::gait_readiness;
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

    fn readiness(&self, ctx: &CheckCtx) -> Readiness {
        let any_loop = ctx
            .clip_expectations()
            .iter()
            .any(|e| e.looping == Some(true));
        if any_loop {
            gait_readiness(ctx.roles)
        } else {
            Readiness::Idle
        }
    }

    fn run(&self, ctx: &CheckCtx, out: &mut Vec<Finding>) {
        let settings = ctx.config.check_settings(self.id());
        let max_ratio = settings.max_ratio.unwrap_or(DEFAULT_MAX_RATIO);
        let min_stride_step_m = ctx.config.loop_seam_min_stride_step_m();

        for (index, clip) in ctx.doc.clips.iter().enumerate() {
            if ctx.expectations(index).looping != Some(true) {
                continue;
            }
            let Some(grid) = ctx.grid(index) else {
                continue; // too short for a cycle; duration-sanity owns degenerate clips
            };
            // Roles resolve (readiness gate); a `None` here means a
            // degenerate clip, which duration-sanity owns.
            let Some(metrics) = foot_cycle_metrics(&grid, ctx.roles, min_stride_step_m) else {
                continue;
            };
            let Some(ratio) = metrics.loop_seam_ratio else {
                continue; // no real stride: idle-like loop, nothing to divide by
            };
            if ratio > max_ratio {
                out.push(
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
    }
}
