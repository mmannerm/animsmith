//! `quat-norm` — rotation keys must be unit quaternions. Engines
//! renormalize inconsistently (or not at all); a non-unit key skews
//! blend weights and skinning.

use super::tracks;
use crate::check::{Check, CheckCtx};
use crate::evaluation::CheckOutput;
use crate::finding::{Finding, Severity};
use crate::model::Property;

/// Allowed |q| deviation from 1. glTF-Validator uses ~5e-4 at the
/// container level; we stay slightly looser to tolerate f32 exporters.
pub const NORM_TOLERANCE: f32 = 1e-3;

pub struct QuatNorm;

impl Check for QuatNorm {
    fn id(&self) -> &'static str {
        "quat-norm"
    }

    fn evaluate(&self, ctx: &CheckCtx) -> CheckOutput {
        let mut findings = Vec::new();
        let doc = ctx.doc;
        for (clip, bone, track) in tracks(doc) {
            if track.property != Property::Rotation {
                continue;
            }
            // Worst key wins; tangent elements of cubic tracks are not
            // quaternions and are skipped.
            let mut worst: Option<(usize, f32)> = None;
            for k in 0..track.key_count() {
                let Some(q) = track.key_quat(k) else { continue };
                if !q.is_finite() {
                    continue; // the nan check owns this
                }
                let len = q.length();
                if (len - 1.0).abs() > NORM_TOLERANCE
                    && worst.is_none_or(|(_, w): (usize, f32)| (len - 1.0).abs() > (w - 1.0).abs())
                {
                    worst = Some((k, len));
                }
            }
            if let Some((k, len)) = worst {
                findings.push(
                    Finding::new(
                        self.id(),
                        Severity::Error,
                        format!("non-unit rotation key (worst at key {k})"),
                    )
                    .clip(clip)
                    .bone(bone)
                    .time(track.times[k])
                    .measured(len as f64)
                    .expected(1.0f64),
                );
            }
        }
        CheckOutput::from_coverage(findings, Vec::new(), Vec::new())
    }
}
