//! `loop-seam-vel` — per-bone model-space linear-velocity continuity across
//! the wrap. Judged only on clips declared `loop = true`; no rig roles or
//! locomotion stride are required.

use crate::check::{Check, CheckCtx};
use crate::evaluation::{
    Applicability, CheckOutput, CoverageGap, CoverageGapCode, EvaluationScope, EvaluationScopeCode,
};
use crate::finding::{Finding, Severity};
use crate::metrics::loop_continuity_metrics;

use super::exceeds_f32_cap;

/// Default maximum difference between the velocities entering and leaving
/// the seam: 0.1 metres per second.
pub const DEFAULT_MAX_VELOCITY_DELTA_MPS: f64 = 0.1;

pub struct LoopSeamVelocity;

impl Check for LoopSeamVelocity {
    fn id(&self) -> &'static str {
        "loop-seam-vel"
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
        let max_velocity_delta_mps = ctx
            .config
            .check_settings(self.id())
            .max_velocity_delta_mps
            .unwrap_or(DEFAULT_MAX_VELOCITY_DELTA_MPS);

        for (clip_index, clip) in ctx.doc.clips.iter().enumerate() {
            if ctx.expectations(clip_index).looping != Some(true) {
                continue;
            }
            let scope =
                EvaluationScope::new(EvaluationScopeCode::LOOP_SEAM_VELOCITY).subject(&clip.name);
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

            let max_velocity = metrics.iter().enumerate().max_by(|(_, a), (_, b)| {
                a.seam_velocity_delta_mps
                    .total_cmp(&b.seam_velocity_delta_mps)
            });
            if let Some((bone_index, metric)) = max_velocity
                && exceeds_f32_cap(metric.seam_velocity_delta_mps, max_velocity_delta_mps)
            {
                let bone = &ctx.doc.skeleton.bones[bone_index].name;
                findings.push(
                    Finding::new(
                        self.id(),
                        Severity::Error,
                        format!(
                            "loop velocity changes at the seam: bone '{bone}' differs by {:.4} m/s between the incoming and outgoing model-space velocities (cap {:.4} m/s)",
                            metric.seam_velocity_delta_mps, max_velocity_delta_mps
                        ),
                    )
                    .clip(&clip.name)
                    .bone(bone)
                    .time(clip.duration_s as f32)
                    .measured(metric.seam_velocity_delta_mps)
                    .expected(max_velocity_delta_mps),
                );
            }
        }

        CheckOutput::from_coverage(findings, evaluated_scopes, gaps)
    }
}
