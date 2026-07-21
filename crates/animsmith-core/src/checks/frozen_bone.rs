//! `frozen-bone` — a required bone that carries keyframes but whose
//! rotation never exceeds a floor is frozen: a T-posed limb, a
//! wrong-source slice, or a masked-out channel that the presence-only
//! `missing-bones` check would pass. Real motion moves required bones
//! tens of degrees; the default 1° floor catches truly static bones
//! without flagging subtle idle sway.

use crate::check::{Check, CheckCtx};
use crate::evaluation::{Applicability, CheckOutput};
use crate::finding::{Finding, Severity};
use crate::metrics::rotation_range_deg;

pub const DEFAULT_MIN_ROTATION_DEG: f64 = 1.0;

pub struct FrozenBone;

impl Check for FrozenBone {
    fn id(&self) -> &'static str {
        "frozen-bone"
    }

    fn applicability(&self, ctx: &CheckCtx) -> Applicability {
        if ctx
            .clip_expectations()
            .iter()
            .any(|expectations| expectations.animates_bones.is_some())
        {
            Applicability::Applicable
        } else {
            Applicability::NotApplicable
        }
    }

    fn evaluate(&self, ctx: &CheckCtx) -> CheckOutput {
        let mut findings = Vec::new();
        let floor = ctx
            .config
            .check_settings(self.id())
            .min_rotation_deg
            .unwrap_or(DEFAULT_MIN_ROTATION_DEG);

        for (index, clip) in ctx.doc.clips.iter().enumerate() {
            let Some(required) = ctx.expectations(index).animates_bones.as_deref() else {
                continue;
            };
            for bone_name in required {
                let Some(bone_id) = ctx
                    .doc
                    .skeleton
                    .bones
                    .iter()
                    .position(|b| &b.name == bone_name)
                else {
                    continue; // missing-bones owns absent bones
                };
                // Max angular deviation from the first key across the
                // bone's rotation tracks; a bone with no rotation track
                // at all is also frozen rotation-wise, but that reads as
                // missing-bones territory only if it has no tracks —
                // with translation-only tracks it belongs here.
                let mut has_any_track = false;
                let mut max_deg = 0.0f64;
                for track in clip.tracks.iter().filter(|t| t.bone == bone_id) {
                    has_any_track = true;
                    if let Some(deg) = rotation_range_deg(track) {
                        max_deg = max_deg.max(deg);
                    }
                }
                if has_any_track && max_deg < floor {
                    findings.push(
                        Finding::new(
                            self.id(),
                            Severity::Error,
                            format!(
                                "required bone rotates only {max_deg:.2}° over the clip \
                                 (floor {floor:.2}°) — frozen/T-posed limb or a \
                                 wrong-source slice"
                            ),
                        )
                        .clip(&clip.name)
                        .bone(bone_name.clone())
                        .measured(max_deg)
                        .expected(floor),
                    );
                }
            }
        }
        CheckOutput::from_coverage(findings, Vec::new(), Vec::new())
    }
}
