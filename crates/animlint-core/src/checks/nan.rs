//! `nan` — NaN/Inf anywhere in key times or values. Always an error:
//! a single non-finite value poisons interpolation and, in most
//! engines, the whole pose.

use super::tracks;
use crate::check::Check;
use crate::finding::{Finding, Severity};
use crate::model::{Document, TrackValues};

pub struct Nan;

impl Check for Nan {
    fn id(&self) -> &'static str {
        "nan"
    }

    fn run(&self, doc: &Document, out: &mut Vec<Finding>) {
        for (clip, bone, track) in tracks(doc) {
            if let Some(k) = track.times.iter().position(|t| !t.is_finite()) {
                out.push(
                    Finding::new(
                        self.id(),
                        Severity::Error,
                        format!(
                            "non-finite key time in {} track (key {k})",
                            track.property.as_str()
                        ),
                    )
                    .clip(clip)
                    .bone(bone),
                );
            }
            let bad_value = match &track.values {
                TrackValues::Vec3s(v) => v.iter().position(|x| !x.is_finite()),
                TrackValues::Quats(v) => v.iter().position(|q| !q.is_finite()),
            };
            if let Some(i) = bad_value {
                out.push(
                    Finding::new(
                        self.id(),
                        Severity::Error,
                        format!(
                            "non-finite value in {} track (element {i})",
                            track.property.as_str()
                        ),
                    )
                    .clip(clip)
                    .bone(bone),
                );
            }
        }
    }
}
