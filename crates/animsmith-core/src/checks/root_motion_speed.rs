//! `root-motion-speed` — a clip's declared locomotion speed must match
//! its measured horizontal root displacement over its duration.
//! Runtimes scale playback by the declared speed to keep foot plants
//! locked to world velocity; a stale pin plays the clip visibly too
//! fast or too slow.

use crate::check::{Check, CheckCtx};
use crate::finding::{Finding, Severity};
use crate::metrics::root_motion_speed_mps;

/// A declared speed with a measurement under this (m/s) is a stray pin:
/// the clip carries no meaningful root motion at all.
pub const STRAY_PIN_FLOOR_MPS: f64 = 0.5;

pub struct RootMotionSpeed;

impl Check for RootMotionSpeed {
    fn id(&self) -> &'static str {
        "root-motion-speed"
    }

    fn run(&self, ctx: &CheckCtx, out: &mut Vec<Finding>) {
        let mut missing_roles_noted = false;
        for (index, clip) in ctx.doc.clips.iter().enumerate() {
            let Some(pin) = ctx.config.expectations_for(&clip.name).speed_mps else {
                continue;
            };
            let measured = ctx
                .grid(index)
                .and_then(|grid| root_motion_speed_mps(&grid, ctx.roles));
            let Some(measured) = measured else {
                if !missing_roles_noted {
                    missing_roles_noted = true;
                    out.push(
                        Finding::new(
                            self.id(),
                            Severity::Note,
                            format!(
                                "skipped: root/hips role not resolved (rig profile '{}')",
                                ctx.roles.profile
                            ),
                        )
                        .clip(&clip.name),
                    );
                }
                continue;
            };
            if measured < STRAY_PIN_FLOOR_MPS && pin.value >= STRAY_PIN_FLOOR_MPS {
                out.push(
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
                out.push(
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
    }
}
