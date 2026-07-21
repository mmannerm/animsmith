//! `foot-slide` — during stance (the foot near its lowest height), a
//! locomotion clip's foot must move consistently with the clip's
//! declared travel: at `speed_mps` relative to the character for an
//! in-place (treadmill) clip, or planted in the world for a
//! root-motion clip. Deviation is the skate that runtime IK and blend
//! band-aids exist to hide.
//!
//! The research-grade check of the catalog (DESIGN.md §12): contact
//! detection is heuristic, so it ships as a warning with generous
//! defaults; judged only on clips that declare `speed_mps`.

use crate::check::{Check, CheckCtx};
use crate::checks::root_motion_gap;
use crate::evaluation::{
    Applicability, CheckOutput, CoverageGap, CoverageGapCode, EvaluationScope, EvaluationScopeCode,
};
use crate::finding::{Finding, Severity};
use crate::metrics::root_motion_speed_mps;
use crate::profile::Role;

/// A foot within this height of its per-clip minimum is in contact.
pub const DEFAULT_CONTACT_HEIGHT_M: f64 = 0.03;

/// Allowed deviation of stance-foot speed from the expected travel.
pub const DEFAULT_MAX_SLIDE_MPS: f64 = 0.3;

pub struct FootSlide;

impl Check for FootSlide {
    fn id(&self) -> &'static str {
        "foot-slide"
    }

    fn applicability(&self, ctx: &CheckCtx) -> Applicability {
        // Foot-slide needs the travel mode (root/hips) to know whether
        // a planted or sweeping foot is correct; individual missing
        // feet are handled per-foot in `evaluate`.
        if ctx
            .clip_expectations()
            .iter()
            .any(|expectations| expectations.speed_mps.is_some())
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
        let contact_height = settings
            .contact_height_m
            .unwrap_or(DEFAULT_CONTACT_HEIGHT_M);
        let max_slide = settings.max_slide_mps.unwrap_or(DEFAULT_MAX_SLIDE_MPS);

        for (index, clip) in ctx.doc.clips.iter().enumerate() {
            let Some(pin) = ctx.expectations(index).speed_mps else {
                continue;
            };
            if let Some(gap) = root_motion_gap(ctx.roles) {
                gaps.push(gap.scope(
                    EvaluationScope::new(EvaluationScopeCode::FOOT_STANCE).subject(&clip.name),
                ));
                continue;
            }
            let Some(grid) = ctx.grid(index) else {
                gaps.push(
                    CoverageGap::new(
                        CoverageGapCode::MEASUREMENT_UNAVAILABLE,
                        "clip is too short to sample foot stance",
                    )
                    .scope(
                        EvaluationScope::new(EvaluationScopeCode::FOOT_STANCE).subject(&clip.name),
                    ),
                );
                continue;
            };
            let Some(root_speed) = root_motion_speed_mps(&grid, ctx.roles) else {
                gaps.push(
                    CoverageGap::new(
                        CoverageGapCode::MEASUREMENT_UNAVAILABLE,
                        "root-motion speed could not be measured",
                    )
                    .scope(
                        EvaluationScope::new(EvaluationScopeCode::FOOT_STANCE).subject(&clip.name),
                    ),
                );
                continue;
            };
            // Treadmill clip: the stance foot must sweep backward at the
            // declared speed. Root-motion clip: it must stay planted.
            let expected_speed = if root_speed >= 0.5 { 0.0 } else { pin.value };

            // Foot first, toe as fallback — matching `foot_cycle_metrics`
            // so a rig that resolves only toe roles is still judged (#57).
            for (side_roles, label) in [
                ([Role::LeftFoot, Role::LeftToe], "left"),
                ([Role::RightFoot, Role::RightToe], "right"),
            ] {
                let scope = if label == "left" {
                    EvaluationScope::new(EvaluationScopeCode::LEFT_FOOT_STANCE)
                } else {
                    EvaluationScope::new(EvaluationScopeCode::RIGHT_FOOT_STANCE)
                }
                .subject(&clip.name);
                let Some(foot) = side_roles.iter().find_map(|&r| ctx.roles.get(r)) else {
                    gaps.push(
                        CoverageGap::new(
                            CoverageGapCode::ROLES_UNRESOLVED,
                            format!("{label} foot/toe role not resolved"),
                        )
                        .scope(scope),
                    );
                    continue;
                };
                let frames = grid.frame_count();
                evaluated_scopes.push(scope);
                let heights: Vec<f64> = (0..frames)
                    .map(|f| grid.model_position(f, foot).y as f64)
                    .collect();
                let ground = heights.iter().copied().fold(f64::MAX, f64::min);
                let mut worst: Option<(f64, usize)> = None;
                for f in 1..frames {
                    if heights[f] > ground + contact_height
                        || heights[f - 1] > ground + contact_height
                    {
                        continue; // not a stance step
                    }
                    let dt = (grid.times[f] - grid.times[f - 1]) as f64;
                    if dt <= 0.0 {
                        continue;
                    }
                    let a = grid.model_position(f - 1, foot);
                    let b = grid.model_position(f, foot);
                    let dx = (b.x - a.x) as f64;
                    let dz = (b.z - a.z) as f64;
                    let speed = dx.hypot(dz) / dt;
                    let slide = (speed - expected_speed).abs();
                    if slide > max_slide && worst.is_none_or(|(w, _)| slide > w) {
                        worst = Some((slide, f));
                    }
                }
                if let Some((slide, frame)) = worst {
                    findings.push(
                        Finding::new(
                            self.id(),
                            Severity::Warning,
                            format!(
                                "{label} foot skates during stance: speed deviates \
                                 {slide:.2} m/s from the expected {expected_speed:.2} m/s \
                                 (cap {max_slide:.2}) — foot plants will slip at runtime"
                            ),
                        )
                        .clip(&clip.name)
                        .bone(ctx.doc.skeleton.bones[foot].name.clone())
                        .time(grid.times[frame])
                        .measured(slide)
                        .expected(max_slide),
                    );
                }
            }
        }
        CheckOutput::from_coverage(findings, evaluated_scopes, gaps)
    }
}
