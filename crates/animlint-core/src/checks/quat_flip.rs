//! `quat-flip` — adjacent rotation keys on opposite hemispheres
//! (`dot < 0`). Engines that slerp without neighborhood correction take
//! the long way around: a visible 360°-minus-θ spin between two keys.
//! Exporters should emit hemisphere-consistent keys.

use super::tracks;
use crate::check::{Check, CheckCtx};
use crate::finding::{Finding, Severity};
use crate::model::Property;

pub struct QuatFlip;

impl Check for QuatFlip {
    fn id(&self) -> &'static str {
        "quat-flip"
    }

    fn run(&self, ctx: &CheckCtx, out: &mut Vec<Finding>) {
        let doc = ctx.doc;
        for (clip, bone, track) in tracks(doc) {
            if track.property != Property::Rotation {
                continue;
            }
            let mut first: Option<usize> = None;
            let mut count = 0usize;
            for k in 1..track.key_count() {
                let (Some(a), Some(b)) = (track.key_quat(k - 1), track.key_quat(k)) else {
                    continue;
                };
                if !a.is_finite() || !b.is_finite() {
                    continue;
                }
                if a.dot(b) < 0.0 {
                    count += 1;
                    first.get_or_insert(k);
                }
            }
            if let Some(k) = first {
                out.push(
                    Finding::new(
                        self.id(),
                        Severity::Warning,
                        format!(
                            "{count} hemisphere flip(s) between adjacent rotation keys \
                             (first between keys {} and {k}) — engines without \
                             neighborhood correction will spin the long way",
                            k - 1
                        ),
                    )
                    .clip(clip)
                    .bone(bone)
                    .time(track.times[k])
                    .measured(count as f64),
                );
            }
        }
    }
}
