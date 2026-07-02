//! `time-monotonic` — key times must be strictly increasing (glTF
//! requires it; engines misbehave without it), non-negative, and the
//! first key should sit at (or very near) t=0: a late first key means
//! the engine clamp-holds an unauthored pose for the gap.

use super::tracks;
use crate::check::Check;
use crate::finding::{Finding, Severity};
use crate::model::Document;

/// A first key later than this is flagged (half a frame at 30 fps).
pub const FIRST_KEY_SLACK_S: f32 = 0.017;

pub struct TimeMonotonic;

impl Check for TimeMonotonic {
    fn id(&self) -> &'static str {
        "time-monotonic"
    }

    fn run(&self, doc: &Document, out: &mut Vec<Finding>) {
        for (clip, bone, track) in tracks(doc) {
            let times = &track.times;
            if times.is_empty() {
                continue;
            }
            if times[0] < 0.0 {
                out.push(
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
                out.push(
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
                out.push(
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
    }
}
