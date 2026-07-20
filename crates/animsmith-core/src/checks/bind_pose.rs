//! `bind-pose` — a clip whose first frame deviates wildly from the
//! skeleton's rest pose was almost certainly authored against a
//! different bind (wrong seed rig, wrong export skeleton). Small
//! deviations are normal — few clips start exactly at rest — so only a
//! large mean deviation across the animated bones fires.
//!
//! (Rest-vs-inverse-bind disagreement is deferred: IBMs live in mesh
//! space, which needs per-mesh space handling to compare fairly.)

use crate::check::{Check, CheckCtx};
use crate::evaluation::{
    Applicability, CheckOutput, CoverageGap, CoverageGapCode, EvaluationScope,
};
use crate::finding::{Finding, Severity};
use crate::model::Property;

/// Mean first-frame rotation deviation (degrees, across bones with
/// rotation tracks) above which the clip likely targets another bind.
pub const DEFAULT_MAX_MEAN_REST_DELTA_DEG: f64 = 45.0;

pub struct BindPose;

impl Check for BindPose {
    fn id(&self) -> &'static str {
        "bind-pose"
    }

    fn applicability(&self, ctx: &CheckCtx) -> Applicability {
        if ctx.doc.clips.is_empty() {
            Applicability::NotApplicable
        } else {
            Applicability::Applicable
        }
    }

    fn evaluate(&self, ctx: &CheckCtx) -> CheckOutput {
        let mut findings = Vec::new();
        let mut evaluated_scopes = Vec::new();
        let mut gaps = Vec::new();
        let cap = ctx
            .config
            .check_settings(self.id())
            .max_mean_rest_delta_deg
            .unwrap_or(DEFAULT_MAX_MEAN_REST_DELTA_DEG);
        for clip in &ctx.doc.clips {
            let mut total_deg = 0.0f64;
            let mut counted = 0usize;
            let mut worst: Option<(f64, &str)> = None;
            for track in &clip.tracks {
                if track.property != Property::Rotation || track.key_count() == 0 {
                    continue;
                }
                let Some(bone) = ctx.doc.skeleton.bones.get(track.bone) else {
                    continue;
                };
                let Some(first) = track.key_quat(0) else {
                    continue;
                };
                if !first.is_finite() || first.length_squared() == 0.0 {
                    continue;
                }
                let deg = bone
                    .rest
                    .rotation
                    .normalize()
                    .angle_between(first.normalize())
                    .to_degrees() as f64;
                total_deg += deg;
                counted += 1;
                if worst.is_none_or(|(w, _)| deg > w) {
                    worst = Some((deg, bone.name.as_str()));
                }
            }
            if counted < 3 {
                gaps.push(
                    CoverageGap::new(
                        CoverageGapCode::custom("insufficient_rotation_evidence"),
                        format!(
                            "only {counted} usable first-frame rotation track(s); at least three are required"
                        ),
                    )
                    .scope(EvaluationScope::new("first_frame_rest_delta").subject(&clip.name)),
                );
                continue;
            }
            evaluated_scopes
                .push(EvaluationScope::new("first_frame_rest_delta").subject(&clip.name));
            let mean = total_deg / counted as f64;
            if mean > cap {
                let (worst_deg, worst_bone) = worst.expect("counted > 0");
                findings.push(
                    Finding::new(
                        self.id(),
                        Severity::Warning,
                        format!(
                            "first frame deviates from the rest pose by {mean:.0}° on \
                             average across {counted} bones (worst {worst_bone}: \
                             {worst_deg:.0}°) — authored against a different bind?"
                        ),
                    )
                    .clip(&clip.name)
                    .time(0.0)
                    .measured(mean)
                    .expected(cap),
                );
            }
        }
        match (evaluated_scopes.is_empty(), gaps.is_empty()) {
            (_, true) => CheckOutput::complete_scoped(findings, evaluated_scopes),
            (true, false) => CheckOutput::not_evaluated(gaps),
            (false, false) => CheckOutput::partial(findings, evaluated_scopes, gaps),
        }
    }
}
