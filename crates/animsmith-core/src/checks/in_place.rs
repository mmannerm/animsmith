//! `in-place` — a clip's declared travel mode must match its measured
//! root motion. In-place clips drive entity velocity from gameplay and
//! expect no baked travel; root-motion clips are the opposite. A
//! mismatch makes the character glide or run in place at runtime.

use crate::check::{Check, CheckCtx, Readiness};
use crate::checks::root_motion_readiness;
use crate::finding::{Finding, Severity};
use crate::metrics::root_motion_speed_mps;

/// Measured horizontal root speed above this counts as travelling.
pub const TRAVEL_THRESHOLD_MPS: f64 = 0.5;

pub struct InPlace;

impl Check for InPlace {
    fn id(&self) -> &'static str {
        "in-place"
    }

    fn readiness(&self, ctx: &CheckCtx) -> Readiness {
        let any = ctx
            .doc
            .clips
            .iter()
            .any(|c| ctx.config.expectations_for(&c.name).in_place.is_some());
        if any {
            root_motion_readiness(ctx.roles)
        } else {
            Readiness::Idle
        }
    }

    fn run(&self, ctx: &CheckCtx, out: &mut Vec<Finding>) {
        for (index, clip) in ctx.doc.clips.iter().enumerate() {
            let Some(expected) = ctx.config.expectations_for(&clip.name).in_place else {
                continue;
            };
            let measured = ctx
                .grid(index)
                .and_then(|grid| root_motion_speed_mps(&grid, ctx.roles));
            let Some(speed) = measured else {
                continue; // roles resolve (readiness gate); degenerate clip
            };
            let travels = speed >= TRAVEL_THRESHOLD_MPS;
            if expected && travels {
                out.push(
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
            } else if !expected && !travels {
                out.push(
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
    }
}
