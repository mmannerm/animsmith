//! `required-bones` — skeleton names declared as required must occur exactly
//! once, independent of whether any clip animates them. This supports static
//! runtime sockets, IK targets, and mask bones without overloading the
//! per-clip motion contract of `missing-bones` and `frozen-bone`.

use std::collections::BTreeSet;

use crate::check::{Check, CheckCtx};
use crate::evaluation::{
    Applicability, CheckOutput, CoverageGap, CoverageGapCode, EvaluationScope, EvaluationScopeCode,
};
use crate::finding::{Finding, Severity};

pub struct RequiredBones;

impl Check for RequiredBones {
    fn id(&self) -> &'static str {
        "required-bones"
    }

    fn applicability(&self, ctx: &CheckCtx) -> Applicability {
        if ctx
            .config
            .rig
            .required_bones
            .as_ref()
            .is_some_and(|bones| !bones.is_empty())
        {
            Applicability::Applicable
        } else {
            Applicability::NotApplicable
        }
    }

    fn evaluate(&self, ctx: &CheckCtx) -> CheckOutput {
        let required = ctx
            .config
            .rig
            .required_bones
            .as_deref()
            .expect("applicability only permits a non-empty required_bones declaration");
        let scope = EvaluationScope::new(EvaluationScopeCode::REQUIRED_BONE_PRESENCE);

        if ctx.doc.skeleton.bones.is_empty() {
            return CheckOutput::from_coverage(
                Vec::new(),
                Vec::new(),
                vec![CoverageGap::new(
                    CoverageGapCode::SKELETON_UNAVAILABLE,
                    "required skeleton bones cannot be checked because the file has no usable skeleton",
                )
                .scope(scope)],
            );
        }

        // A repeated declaration is one requirement, not repeated work: emit
        // at most one stable result for each first-declared name.
        let mut seen = BTreeSet::new();
        let mut findings = Vec::new();
        for bone_name in required {
            if !seen.insert(bone_name) {
                continue;
            }
            let matches = ctx
                .doc
                .skeleton
                .bones
                .iter()
                .filter(|bone| bone.name == *bone_name)
                .count();
            match matches {
                1 => {}
                0 => findings.push(
                    Finding::new(
                        self.id(),
                        Severity::Error,
                        "required bone does not exist in the skeleton",
                    )
                    .bone(bone_name.clone())
                    .measured(0.0)
                    .expected(1.0),
                ),
                count => findings.push(
                    Finding::new(
                        self.id(),
                        Severity::Error,
                        format!(
                            "required bone name is ambiguous: {count} skeleton bones share this name"
                        ),
                    )
                    .bone(bone_name.clone())
                    .measured(count as f64)
                    .expected(1.0),
                ),
            }
        }

        CheckOutput::from_coverage(findings, vec![scope], Vec::new())
    }
}
