//! `fps` — keyframes of a clip with a declared frame rate must sit on
//! that rate's time grid, and the duration must span a whole number of
//! frames. Off-grid keys mean a resample or retiming step drifted; a
//! fractional frame count means a slice cut mid-frame.

use crate::check::{Check, CheckCtx};
use crate::evaluation::{
    Applicability, CheckOutput, CoverageGap, CoverageGapCode, EvaluationScope, EvaluationScopeCode,
};
use crate::finding::{Finding, Severity};

/// Allowed distance from the frame grid, in frames.
pub const GRID_TOLERANCE_FRAMES: f64 = 0.1;

pub struct Fps;

impl Check for Fps {
    fn id(&self) -> &'static str {
        "fps"
    }

    fn applicability(&self, ctx: &CheckCtx) -> Applicability {
        if ctx
            .clip_expectations()
            .iter()
            .any(|expectations| expectations.fps.is_some())
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
        for (index, clip) in ctx.doc.clips.iter().enumerate() {
            let Some(fps) = ctx.expectations(index).fps else {
                continue;
            };
            if !fps.is_finite() || fps <= 0.0 {
                gaps.push(
                    CoverageGap::new(
                        CoverageGapCode::INVALID_DECLARED_FPS,
                        format!("clip declares a non-positive or non-finite frame rate ({fps})"),
                    )
                    .scope(
                        EvaluationScope::new(EvaluationScopeCode::FRAME_GRID).subject(&clip.name),
                    ),
                );
                continue;
            }
            evaluated_scopes
                .push(EvaluationScope::new(EvaluationScopeCode::FRAME_GRID).subject(&clip.name));
            let frames = clip.duration_s * fps;
            if (frames - frames.round()).abs() > GRID_TOLERANCE_FRAMES {
                findings.push(
                    Finding::new(
                        self.id(),
                        Severity::Warning,
                        format!(
                            "duration {:.4}s is {frames:.2} frames at {fps} fps — not a \
                             whole frame count; a slice cut mid-frame?",
                            clip.duration_s
                        ),
                    )
                    .clip(&clip.name)
                    .measured(frames)
                    .expected(frames.round()),
                );
            }
            // Worst off-grid key across all tracks.
            let mut worst: Option<(f64, f32, &'static str)> = None;
            for track in &clip.tracks {
                for &t in &track.times {
                    let pos = t as f64 * fps;
                    let err = (pos - pos.round()).abs();
                    if err > GRID_TOLERANCE_FRAMES && worst.is_none_or(|(w, _, _)| err > w) {
                        worst = Some((err, t, track.property.as_str()));
                    }
                }
            }
            if let Some((err, t, property)) = worst {
                findings.push(
                    Finding::new(
                        self.id(),
                        Severity::Warning,
                        format!(
                            "{property} key at {t:.4}s sits {err:.2} frames off the \
                             {fps} fps grid (worst offender) — resampling drift?"
                        ),
                    )
                    .clip(&clip.name)
                    .time(t)
                    .measured(err),
                );
            }
        }
        CheckOutput::from_coverage(findings, evaluated_scopes, gaps)
    }
}
