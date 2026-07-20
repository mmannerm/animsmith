//! `missing-bones` — bones the clip is declared to animate must exist
//! in the skeleton and carry at least one keyframed track. Catches clip
//! slices that accidentally drop a channel (leaving a limb static) and
//! exports against the wrong rig.

use crate::check::{Check, CheckCtx};
use crate::evaluation::{Applicability, CheckOutput};
use crate::finding::{Finding, Severity};

pub struct MissingBones;

impl Check for MissingBones {
    fn id(&self) -> &'static str {
        "missing-bones"
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
                    findings.push(
                        Finding::new(
                            self.id(),
                            Severity::Error,
                            "required bone does not exist in the skeleton",
                        )
                        .clip(&clip.name)
                        .bone(bone_name.clone()),
                    );
                    continue;
                };
                let animated = clip
                    .tracks
                    .iter()
                    .any(|t| t.bone == bone_id && t.key_count() > 0);
                if !animated {
                    findings.push(
                        Finding::new(
                            self.id(),
                            Severity::Error,
                            "required bone carries no keyframes in this clip",
                        )
                        .clip(&clip.name)
                        .bone(bone_name.clone()),
                    );
                }
            }
        }
        CheckOutput::complete(findings)
    }
}
