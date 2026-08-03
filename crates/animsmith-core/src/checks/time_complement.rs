//! `time-complement` — detect declared same-time / absolute-sync members
//! whose gait phases are substantially more similar after reflecting one
//! member's normalized cycle time.

use crate::check::{Check, CheckCtx};
use crate::checks::gait_gap;
use crate::config::TimeComplementSettings;
use crate::evaluation::{
    Applicability, CheckOutput, CoverageGap, CoverageGapCode, EvaluationScope, EvaluationScopeCode,
};
use crate::finding::{Finding, MemberMeasurement, Severity};
use crate::metrics::{MIN_STRIDE_STEP_M, foot_cycle_metrics};
use std::collections::BTreeSet;

/// Compare phase similarity for pairs declared in same-time sync groups.
pub struct TimeComplement;

impl Check for TimeComplement {
    fn id(&self) -> &'static str {
        "time-complement"
    }

    fn applicability(&self, ctx: &CheckCtx) -> Applicability {
        if ctx
            .config
            .sync_groups
            .values()
            .any(|group| group.time_complement.is_some())
        {
            Applicability::Applicable
        } else {
            Applicability::NotApplicable
        }
    }

    fn evaluate(&self, ctx: &CheckCtx) -> CheckOutput {
        let mut findings = Vec::new();
        let mut evaluated_scopes =
            vec![EvaluationScope::new(EvaluationScopeCode::MEMBER_EXISTENCE)];
        let mut gaps = Vec::new();
        let roles_gap = gait_gap(ctx.roles);

        for (group_name, group) in &ctx.config.sync_groups {
            let Some(settings) = &group.time_complement else {
                continue;
            };
            let scope =
                EvaluationScope::new(EvaluationScopeCode::PHASE_COHERENCE).subject(group_name);
            let mut measured = Vec::new();
            let mut present = 0usize;
            let mut missing = 0usize;
            let mut configured_members = BTreeSet::new();

            for member in &group.clips {
                // Treat repeated declarations as one member. In particular,
                // never turn `["walk", "walk"]` into a self-comparison.
                if !configured_members.insert(member.as_str()) {
                    continue;
                }
                let Some(index) = ctx.doc.clips.iter().position(|clip| &clip.name == member) else {
                    missing += 1;
                    continue;
                };
                present += 1;
                if roles_gap.is_some() {
                    continue;
                }

                let measurement_scope =
                    EvaluationScope::new(EvaluationScopeCode::PHASE_MEASUREMENT).subject(member);
                let Some(metrics) = ctx
                    .grid(index)
                    .and_then(|grid| foot_cycle_metrics(&grid, ctx.roles, MIN_STRIDE_STEP_M))
                else {
                    gaps.push(
                        CoverageGap::new(
                            CoverageGapCode::MEASUREMENT_UNAVAILABLE,
                            "gait phase could not be measured for time-complement comparison",
                        )
                        .scope(measurement_scope),
                    );
                    continue;
                };
                if metrics.lr_amplitude_m < settings.min_lr_amplitude_m {
                    gaps.push(
                        CoverageGap::new(
                            CoverageGapCode::MEASUREMENT_UNAVAILABLE,
                            format!(
                                "left/right gait amplitude {:.3} m is below the {:.3} m time-complement evidence floor",
                                metrics.lr_amplitude_m, settings.min_lr_amplitude_m
                            ),
                        )
                        .scope(measurement_scope),
                    );
                    continue;
                }
                let Some(phase) = metrics.gait_phase else {
                    gaps.push(
                        CoverageGap::new(
                            CoverageGapCode::MEASUREMENT_UNAVAILABLE,
                            "gait phase could not be fitted for time-complement comparison",
                        )
                        .scope(measurement_scope),
                    );
                    continue;
                };
                measured.push(MemberPhase {
                    name: member,
                    phase,
                    lr_amplitude_m: metrics.lr_amplitude_m,
                });
            }

            if present > 0
                && let Some(gap) = &roles_gap
            {
                if missing > 0 {
                    gaps.push(
                        CoverageGap::new(
                            CoverageGapCode::MEMBERS_NOT_EVALUATED,
                            format!(
                                "same-time sync group '{group_name}' has {missing} configured member(s) absent from the file"
                            ),
                        )
                        .scope(scope.clone()),
                    );
                }
                gaps.push(gap.clone().scope(scope));
                continue;
            }

            let mut compared_pairs = 0usize;
            for (index, left) in measured.iter().enumerate() {
                for right in &measured[index + 1..] {
                    compared_pairs += 1;
                    let same_time_similarity = similarity(left.phase, right.phase);
                    let reflected_time_similarity = similarity(left.phase, 1.0 - right.phase);
                    let reflected_time_advantage = reflected_time_similarity - same_time_similarity;
                    if reflected_time_advantage > settings.min_reflected_time_advantage {
                        findings.push(time_complement_finding(
                            group_name,
                            settings,
                            left,
                            right,
                            same_time_similarity,
                            reflected_time_similarity,
                            reflected_time_advantage,
                        ));
                    }
                }
            }
            if compared_pairs > 0 {
                evaluated_scopes.push(scope.clone());
            }

            if missing > 0 {
                gaps.push(
                    CoverageGap::new(
                        CoverageGapCode::MEMBERS_NOT_EVALUATED,
                        format!(
                            "same-time sync group '{group_name}' has {missing} configured member(s) absent from the file"
                        ),
                    )
                    .scope(scope.clone()),
                );
            }
            if measured.len() < 2 {
                gaps.push(
                    CoverageGap::new(
                        CoverageGapCode::INSUFFICIENT_MEASURABLE_MEMBERS,
                        format!(
                            "same-time sync group '{group_name}' has {} measurable phase member(s); at least two are required",
                            measured.len()
                        ),
                    )
                    .scope(scope),
                );
            } else if missing == 0 && measured.len() < configured_members.len() {
                gaps.push(
                    CoverageGap::new(
                        CoverageGapCode::MEMBERS_NOT_EVALUATED,
                        format!(
                            "same-time sync group '{group_name}' evaluated {} of {} configured member(s)",
                            measured.len(),
                            configured_members.len()
                        ),
                    )
                    .scope(scope),
                );
            }
        }

        CheckOutput::from_coverage(findings, evaluated_scopes, gaps)
    }
}

struct MemberPhase<'a> {
    name: &'a str,
    phase: f64,
    lr_amplitude_m: f64,
}

fn similarity(left_phase: f64, right_phase: f64) -> f64 {
    (1.0 + (std::f64::consts::TAU * (left_phase - right_phase)).cos()) / 2.0
}

#[allow(clippy::too_many_arguments)]
fn time_complement_finding(
    group_name: &str,
    settings: &TimeComplementSettings,
    left: &MemberPhase<'_>,
    right: &MemberPhase<'_>,
    same_time_similarity: f64,
    reflected_time_similarity: f64,
    reflected_time_advantage: f64,
) -> Finding {
    let pair_measurements = |member: &MemberPhase<'_>| {
        MemberMeasurement::new(member.name)
            .measurement("gait_phase", member.phase)
            .measurement("lr_amplitude_m", member.lr_amplitude_m)
            .measurement("same_time_similarity", same_time_similarity)
            .measurement("reflected_time_similarity", reflected_time_similarity)
            .measurement("reflected_time_advantage", reflected_time_advantage)
    };
    Finding::new(
        "time-complement",
        Severity::Warning,
        format!(
            "same-time / absolute-sync group '{group_name}' pair '{}' and '{}' has reflected-time gait similarity {reflected_time_similarity:.3} versus same-time {same_time_similarity:.3} (advantage {reflected_time_advantage:.3}; threshold {:.3}); this is a sync-compatibility diagnostic for the declared group",
            left.name, right.name, settings.min_reflected_time_advantage,
        ),
    )
    .measured(reflected_time_advantage)
    .expected(settings.min_reflected_time_advantage)
    .members(vec![pair_measurements(left), pair_measurements(right)])
}
