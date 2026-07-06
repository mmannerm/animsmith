//! `fps` — keyframes of a clip with a declared frame rate must sit on
//! that rate's time grid, and the duration must span a whole number of
//! frames. Off-grid keys mean a resample or retiming step drifted; a
//! fractional frame count means a slice cut mid-frame.

use crate::check::{Check, CheckCtx};
use crate::finding::{Finding, Severity};

/// Allowed distance from the frame grid, in frames.
pub const GRID_TOLERANCE_FRAMES: f64 = 0.1;

pub struct Fps;

impl Check for Fps {
    fn id(&self) -> &'static str {
        "fps"
    }

    fn run(&self, ctx: &CheckCtx, out: &mut Vec<Finding>) {
        for (index, clip) in ctx.doc.clips.iter().enumerate() {
            let Some(fps) = ctx.expectations(index).fps else {
                continue;
            };
            if fps <= 0.0 {
                continue;
            }
            let frames = clip.duration_s * fps;
            if (frames - frames.round()).abs() > GRID_TOLERANCE_FRAMES {
                out.push(
                    Finding::new(
                        self.id(),
                        Severity::Warning,
                        format!(
                            "duration {:.4}s is {frames:.2} frames at {fps} fps — not a \
                             whole frame count; a slice cut mid-frame?",
                            clip.duration_s
                        ),
                    )
                    .clip(&clip.name)
                    .measured(frames)
                    .expected(frames.round()),
                );
            }
            // Worst off-grid key across all tracks.
            let mut worst: Option<(f64, f32, &'static str)> = None;
            for track in &clip.tracks {
                for &t in &track.times {
                    let pos = t as f64 * fps;
                    let err = (pos - pos.round()).abs();
                    if err > GRID_TOLERANCE_FRAMES && worst.is_none_or(|(w, _, _)| err > w) {
                        worst = Some((err, t, track.property.as_str()));
                    }
                }
            }
            if let Some((err, t, property)) = worst {
                out.push(
                    Finding::new(
                        self.id(),
                        Severity::Warning,
                        format!(
                            "{property} key at {t:.4}s sits {err:.2} frames off the \
                             {fps} fps grid (worst offender) — resampling drift?"
                        ),
                    )
                    .clip(&clip.name)
                    .time(t)
                    .measured(err),
                );
            }
        }
    }
}
