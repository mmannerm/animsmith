//! `loop-seam-rot` — per-bone model-space angular-velocity continuity across
//! the wrap. Judged only on clips declared `loop = true`; no rig roles or
//! locomotion stride are required.

use crate::check::{Check, CheckCtx};
use crate::evaluation::{
    Applicability, CheckOutput, CoverageGap, CoverageGapCode, EvaluationScope, EvaluationScopeCode,
};
use crate::finding::{Finding, Severity};
use crate::metrics::loop_continuity_metrics;

use super::exceeds_f32_cap;

/// Default maximum difference between the angular velocities entering and
/// leaving the seam: 5 degrees per second.
pub const DEFAULT_MAX_ANGULAR_VELOCITY_DELTA_DEGPS: f64 = 5.0;

/// Per-bone model-space angular-velocity continuity across a loop seam.
pub struct LoopSeamRotation;

impl Check for LoopSeamRotation {
    fn id(&self) -> &'static str {
        "loop-seam-rot"
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
        let global_max_angular_velocity_delta_degps = ctx
            .config
            .check_settings(self.id())
            .max_angular_velocity_delta_degps
            .unwrap_or(DEFAULT_MAX_ANGULAR_VELOCITY_DELTA_DEGPS);

        for (clip_index, clip) in ctx.doc.clips.iter().enumerate() {
            let expectations = ctx.expectations(clip_index);
            if expectations.looping != Some(true) {
                continue;
            }
            let max_angular_velocity_delta_degps = expectations
                .max_loop_angular_velocity_delta_degps
                .unwrap_or(global_max_angular_velocity_delta_degps);
            let scope =
                EvaluationScope::new(EvaluationScopeCode::LOOP_SEAM_ROTATION).subject(&clip.name);
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

            let max_angular_velocity = metrics.iter().enumerate().max_by(|(_, a), (_, b)| {
                a.seam_angular_velocity_delta_degps
                    .total_cmp(&b.seam_angular_velocity_delta_degps)
            });
            if let Some((bone_index, metric)) = max_angular_velocity
                && exceeds_f32_cap(
                    metric.seam_angular_velocity_delta_degps,
                    max_angular_velocity_delta_degps,
                )
            {
                let bone = &ctx.doc.skeleton.bones[bone_index].name;
                findings.push(
                    Finding::new(
                        self.id(),
                        Severity::Error,
                        format!(
                            "loop angular velocity changes at the seam: bone '{bone}' differs by {:.3} deg/s between the incoming and outgoing model-space angular velocities (cap {:.3} deg/s)",
                            metric.seam_angular_velocity_delta_degps,
                            max_angular_velocity_delta_degps
                        ),
                    )
                    .clip(&clip.name)
                    .bone(bone)
                    .time(clip.duration_s as f32)
                    .measured(metric.seam_angular_velocity_delta_degps)
                    .expected(max_angular_velocity_delta_degps),
                );
            }
        }

        CheckOutput::from_coverage(findings, evaluated_scopes, gaps)
    }
}
