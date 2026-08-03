//! `duplicate-loop-endpoint` — detect a redundant repeated closing pose in a
//! declared loop's authored keys.
//!
//! This intentionally reports only the strict, mechanically removable subset
//! of the endpoint modes tracked by #22. It does not classify arbitrary open
//! or non-closing loops and does not sample or infer missing endpoint keys.

use crate::check::{Check, CheckCtx};
use crate::evaluation::{
    Applicability, CheckOutput, CoverageGap, CoverageGapCode, EvaluationScope, EvaluationScopeCode,
};
use crate::finding::{Finding, Severity};
use crate::transform::analyze_duplicate_loop_endpoint;

/// Warn when a declared loop has one or more mechanically removable repeated
/// closing keys on every authored track.
pub struct DuplicateLoopEndpoint;

impl Check for DuplicateLoopEndpoint {
    fn id(&self) -> &'static str {
        "duplicate-loop-endpoint"
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

        for (clip_index, clip) in ctx.doc.clips.iter().enumerate() {
            if ctx.expectations(clip_index).looping != Some(true) {
                continue;
            }
            let scope = EvaluationScope::new(EvaluationScopeCode::DUPLICATE_LOOP_ENDPOINT)
                .subject(&clip.name);
            match analyze_duplicate_loop_endpoint(clip) {
                Ok(candidate) => {
                    evaluated_scopes.push(scope);
                    let Some(outcome) = candidate else {
                        continue;
                    };
                    let delta = |value: Option<f32>, suffix: &str| {
                        value
                            .map(|value| format!("{value:.6}{suffix}"))
                            .unwrap_or_else(|| "n/a".into())
                    };
                    findings.push(
                        Finding::new(
                            self.id(),
                            Severity::Warning,
                            format!(
                                "declared loop repeats its first pose at the authored endpoint: \
                                 {} redundant closing key(s) per track; \
                                 `animsmith transform --drop-duplicate-loop-endpoint` can trim \
                                 {:.6}s -> {:.6}s into an open-cycle representation \
                                 (endpoint deltas: translation {}, rotation {}, scale {})",
                                outcome.removed_keys_per_track,
                                outcome.duration_before_s,
                                outcome.duration_after_s,
                                delta(outcome.max_translation_endpoint_delta_m, " m"),
                                delta(outcome.max_rotation_endpoint_delta_rad, " rad"),
                                delta(outcome.max_scale_endpoint_delta, ""),
                            ),
                        )
                        .clip(&clip.name)
                        .time(outcome.duration_before_s as f32)
                        .measured(outcome.removed_keys_per_track as f64)
                        .expected(0.0),
                    );
                }
                Err(error) => gaps.push(
                    CoverageGap::new(
                        CoverageGapCode::MEASUREMENT_UNAVAILABLE,
                        format!("authored endpoint analysis unavailable: {error}"),
                    )
                    .scope(scope),
                ),
            }
        }

        CheckOutput::from_coverage(findings, evaluated_scopes, gaps)
    }
}
