//! `time-monotonic` — key times must be strictly increasing (glTF
//! requires it; engines misbehave without it), non-negative, and the
//! first key should sit at (or very near) t=0: a late first key means
//! the engine clamp-holds an unauthored pose for the gap.

use super::tracks;
use crate::check::{Check, CheckCtx};
use crate::evaluation::CheckOutput;
use crate::finding::{Finding, Severity};

/// A first key later than this is flagged (half a frame at 30 fps).
pub const FIRST_KEY_SLACK_S: f32 = 0.017;

/// Negative first-key times within this of zero are tolerated: frame-
/// range slicing in bake pipelines leaves f32-quantization dust like
/// -1e-6 s, which engines clamp harmlessly.
pub const NEGATIVE_TIME_TOLERANCE_S: f32 = 1e-4;

pub struct TimeMonotonic;

impl Check for TimeMonotonic {
    fn id(&self) -> &'static str {
        "time-monotonic"
    }

    fn evaluate(&self, ctx: &CheckCtx) -> CheckOutput {
        let mut findings = Vec::new();
        let doc = ctx.doc;
        for (clip, bone, track) in tracks(doc) {
            let times = &track.times;
            if times.is_empty() {
                continue;
            }
            if times[0] < -NEGATIVE_TIME_TOLERANCE_S {
                findings.push(
                    Finding::new(
                        self.id(),
                        Severity::Error,
                        format!("negative key time in {} track", track.property.as_str()),
                    )
                    .clip(clip)
                    .bone(bone)
                    .time(times[0])
                    .measured(times[0]),
                );
            }
            if let Some(k) = (1..times.len()).find(|&k| times[k] <= times[k - 1]) {
                findings.push(
                    Finding::new(
                        self.id(),
                        Severity::Error,
                        format!(
                            "key times not strictly increasing in {} track (keys {} and {})",
                            track.property.as_str(),
                            k - 1,
                            k
                        ),
                    )
                    .clip(clip)
                    .bone(bone)
                    .time(times[k]),
                );
            }
            if times[0] > FIRST_KEY_SLACK_S {
                findings.push(
                    Finding::new(
                        self.id(),
                        Severity::Note,
                        format!(
                            "first key of {} track starts at {:.3}s, not 0 — the pose is \
                             clamp-held until then",
                            track.property.as_str(),
                            times[0]
                        ),
                    )
                    .clip(clip)
                    .bone(bone)
                    .time(times[0])
                    .measured(times[0])
                    .expected(0.0f64),
                );
            }
        }
        CheckOutput::from_coverage(findings, Vec::new(), Vec::new())
    }
}
