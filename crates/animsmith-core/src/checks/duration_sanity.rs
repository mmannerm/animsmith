//! `duration-sanity` — degenerate clip durations and channels within
//! one clip that end at different times (an engine clamp-holds the
//! shorter channels, which usually means a partial export). fps
//! expectations from config arrive with M1.

use crate::check::{Check, CheckCtx};
use crate::finding::{Finding, Severity};

/// Channel end-time spread beyond this is flagged (half a frame at
/// 30 fps).
pub const END_SPREAD_TOLERANCE_S: f32 = 0.017;

pub struct DurationSanity;

impl Check for DurationSanity {
    fn id(&self) -> &'static str {
        "duration-sanity"
    }

    fn run(&self, ctx: &CheckCtx, out: &mut Vec<Finding>) {
        let doc = ctx.doc;
        for clip in &doc.clips {
            if clip.tracks.is_empty() {
                out.push(
                    Finding::new(self.id(), Severity::Warning, "clip has no tracks")
                        .clip(&clip.name),
                );
                continue;
            }
            if clip.duration_s <= 0.0 || !clip.duration_s.is_finite() {
                out.push(
                    Finding::new(
                        self.id(),
                        Severity::Error,
                        format!("degenerate clip duration ({}s)", clip.duration_s),
                    )
                    .clip(&clip.name)
                    .measured(clip.duration_s),
                );
                continue;
            }
            // Single-key tracks are pinned values, not truncated
            // channels — a common bake idiom — so they don't count
            // toward the end spread.
            let ends: Vec<f32> = clip
                .tracks
                .iter()
                .filter(|t| t.key_count() >= 2)
                .map(|t| t.end_time())
                .collect();
            if ends.is_empty() {
                continue;
            }
            let max = ends.iter().copied().fold(f32::MIN, f32::max);
            let min = ends.iter().copied().fold(f32::MAX, f32::min);
            if max - min > END_SPREAD_TOLERANCE_S {
                out.push(
                    Finding::new(
                        self.id(),
                        Severity::Warning,
                        format!(
                            "channels end at different times ({min:.3}s..{max:.3}s) — \
                             shorter channels will be clamp-held"
                        ),
                    )
                    .clip(&clip.name)
                    .measured((max - min) as f64)
                    .expected(0.0f64),
                );
            }
        }
    }
}
