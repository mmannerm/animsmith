//! `missing-bones` — bones the clip is declared to animate must exist
//! in the skeleton and carry at least one keyframed track. Catches clip
//! slices that accidentally drop a channel (leaving a limb static) and
//! exports against the wrong rig.

use crate::check::{Check, CheckCtx};
use crate::finding::{Finding, Severity};

pub struct MissingBones;

impl Check for MissingBones {
    fn id(&self) -> &'static str {
        "missing-bones"
    }

    fn run(&self, ctx: &CheckCtx, out: &mut Vec<Finding>) {
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
                    out.push(
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
                    out.push(
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
    }
}
