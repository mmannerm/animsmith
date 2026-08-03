//! `loop-closure` — per-bone model-space C0 pose closure across the wrap.
//! Position and rotation are judged independently on clips declared
//! `loop = true`; no rig roles or locomotion stride are required.

use crate::check::{Check, CheckCtx};
use crate::evaluation::{
    Applicability, CheckOutput, CoverageGap, CoverageGapCode, EvaluationScope, EvaluationScopeCode,
};
use crate::finding::{Finding, Severity};
use crate::metrics::loop_continuity_metrics;

/// Default maximum last-to-first model-space position delta: 1 cm.
pub const DEFAULT_MAX_POSITION_DELTA_M: f64 = 0.01;
/// Default maximum shortest-path model-space rotation delta: 1 degree.
pub const DEFAULT_MAX_ROTATION_DELTA_DEG: f64 = 1.0;

pub struct LoopClosure;

impl Check for LoopClosure {
    fn id(&self) -> &'static str {
        "loop-closure"
    }

    fn applicability(&self, ctx: &CheckCtx) -> Applicability {
        if ctx
            .clip_expectations()
            .iter()
            .any(|expectations| expectations.looping == Some(true))
        {
            Applicability::Applicable
        } else {
            Applicability::NotApplicable
        }
    }

    fn evaluate(&self, ctx: &CheckCtx) -> CheckOutput {
        let mut findings = Vec::new();
        let mut evaluated_scopes = Vec::new();
        let mut gaps = Vec::new();
        let settings = ctx.config.check_settings(self.id());
        let max_position_delta_m = settings
            .max_position_delta_m
            .unwrap_or(DEFAULT_MAX_POSITION_DELTA_M);
        let max_rotation_delta_deg = settings
            .max_rotation_delta_deg
            .unwrap_or(DEFAULT_MAX_ROTATION_DELTA_DEG);

        for (clip_index, clip) in ctx.doc.clips.iter().enumerate() {
            if ctx.expectations(clip_index).looping != Some(true) {
                continue;
            }
            let scope = EvaluationScope::new(EvaluationScopeCode::LOOP_CLOSURE).subject(&clip.name);
            let Some(metrics) = ctx
                .grid(clip_index)
                .as_deref()
                .and_then(loop_continuity_metrics)
            else {
                gaps.push(
                    CoverageGap::new(
                        CoverageGapCode::MEASUREMENT_UNAVAILABLE,
                        "clip has no usable per-bone loop-continuity sample",
                    )
                    .scope(scope),
                );
                continue;
            };
            evaluated_scopes.push(scope);

            let max_position = metrics
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.position_delta_m.total_cmp(&b.position_delta_m));
            if let Some((bone_index, metric)) = max_position
                && metric.position_delta_m > max_position_delta_m
            {
                let bone = &ctx.doc.skeleton.bones[bone_index].name;
                findings.push(
                    Finding::new(
                        self.id(),
                        Severity::Error,
                        format!(
                            "loop does not close in position: bone '{bone}' is {:.4} m from its first-frame model-space position (cap {:.4} m)",
                            metric.position_delta_m, max_position_delta_m
                        ),
                    )
                    .clip(&clip.name)
                    .bone(bone)
                    .time(clip.duration_s as f32)
                    .measured(metric.position_delta_m)
                    .expected(max_position_delta_m),
                );
            }

            let max_rotation = metrics
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.rotation_delta_deg.total_cmp(&b.rotation_delta_deg));
            if let Some((bone_index, metric)) = max_rotation
                && metric.rotation_delta_deg > max_rotation_delta_deg
            {
                let bone = &ctx.doc.skeleton.bones[bone_index].name;
                findings.push(
                    Finding::new(
                        self.id(),
                        Severity::Error,
                        format!(
                            "loop does not close in rotation: bone '{bone}' is {:.3} degrees from its first-frame model-space rotation (cap {:.3} degrees)",
                            metric.rotation_delta_deg, max_rotation_delta_deg
                        ),
                    )
                    .clip(&clip.name)
                    .bone(bone)
                    .time(clip.duration_s as f32)
                    .measured(metric.rotation_delta_deg)
                    .expected(max_rotation_delta_deg),
                );
            }
        }

        CheckOutput::from_coverage(findings, evaluated_scopes, gaps)
    }
}
