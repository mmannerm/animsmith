//! `sync-group` — timing and endpoint compatibility for clips sampled at the
//! same absolute time by a runtime blend group.

use crate::check::{Check, CheckCtx};
use crate::checks::loop_closure::effective_caps;
use crate::evaluation::{
    Applicability, CheckOutput, CoverageGap, CoverageGapCode, EvaluationScope, EvaluationScopeCode,
};
use crate::finding::{Finding, MemberMeasurement, Severity};
use crate::measure::{measure_frame_grid, measure_loop_endpoint_mode};

/// Compare timing representations for declared same-time groups.
pub struct SyncGroupCheck;

impl Check for SyncGroupCheck {
    fn id(&self) -> &'static str {
        "sync-group"
    }

    fn applicability(&self, ctx: &CheckCtx) -> Applicability {
        if ctx.config.sync_groups.is_empty() {
            Applicability::NotApplicable
        } else {
            Applicability::Applicable
        }
    }

    fn evaluate(&self, ctx: &CheckCtx) -> CheckOutput {
        let mut findings = Vec::new();
        let mut evaluated_scopes =
            vec![EvaluationScope::new(EvaluationScopeCode::MEMBER_EXISTENCE)];
        let mut gaps = Vec::new();

        for (name, group) in &ctx.config.sync_groups {
            let mut rows = Vec::with_capacity(group.clips.len());
            let mut members = Vec::new();
            let mut missing = Vec::new();
            for member in &group.clips {
                let Some(index) = ctx.doc.clips.iter().position(|clip| &clip.name == member) else {
                    rows.push(
                        MemberMeasurement::new(member).measurement("availability", "missing"),
                    );
                    missing.push(member);
                    continue;
                };
                let clip = &ctx.doc.clips[index];
                let expectations = ctx.expectations(index);
                let duration_s = clip.duration_s.is_finite().then_some(clip.duration_s);
                let frame_count = clip
                    .tracks
                    .iter()
                    .map(|track| track.key_count())
                    .max()
                    .unwrap_or(0) as f64;
                evaluated_scopes.push(
                    EvaluationScope::new(EvaluationScopeCode::SYNC_MEMBER_MEASUREMENT)
                        .subject(member),
                );
                let mut row = MemberMeasurement::new(member)
                    .measurement("availability", "present")
                    .measurement("duration_s", clip.duration_s)
                    .measurement("frame_count", frame_count);

                let fps = expectations.fps;
                let frame_grid = measure_frame_grid(clip, fps);
                match frame_grid {
                    Some(grid) => {
                        row = row
                            .measurement("fps", grid.fps)
                            .measurement("frame_intervals", grid.frame_intervals as f64);
                    }
                    None => {
                        let why = match fps {
                            Some(fps) if !fps.is_finite() || fps <= 0.0 => "invalid_fps",
                            Some(_) => "off_grid",
                            None => "undeclared_fps",
                        };
                        row = row.measurement("frame_grid", why);
                    }
                }

                let (position_cap, rotation_cap) = effective_caps(ctx.config, expectations);
                let endpoint = (expectations.looping == Some(true))
                    .then(|| {
                        measure_loop_endpoint_mode(
                            clip,
                            ctx.grid(index).as_deref(),
                            position_cap,
                            rotation_cap,
                        )
                    })
                    .flatten();
                if let Some(mode) = endpoint {
                    row = row.measurement("loop_endpoint_mode", mode.as_str());
                } else {
                    row = row.measurement("loop_endpoint_mode", "unavailable");
                }
                rows.push(row);
                members.push(MemberEvidence {
                    duration_s,
                    frame_count,
                    frame_grid,
                    endpoint,
                });
            }

            let missing_count = missing.len();
            for member in missing {
                findings.push(
                    Finding::new(
                        self.id(),
                        Severity::Error,
                        format!("sync group '{name}' member not found in file"),
                    )
                    .clip(member.clone())
                    .members(rows.clone()),
                );
            }

            let scope = EvaluationScope::new(EvaluationScopeCode::SYNC_COMPATIBILITY).subject(name);
            if missing_count > 0 {
                gaps.push(
                    CoverageGap::new(
                        CoverageGapCode::MEMBERS_NOT_EVALUATED,
                        format!(
                            "sync group '{name}' has {missing_count} configured member(s) absent from the file"
                        ),
                    )
                    .scope(scope.clone()),
                );
            }
            let present = members.len();
            if present < 2 {
                gaps.push(
                    CoverageGap::new(
                        CoverageGapCode::INSUFFICIENT_MEASURABLE_MEMBERS,
                        format!("sync group '{name}' has {present} present member(s); at least two are required"),
                    )
                    .scope(scope),
                );
                continue;
            }

            let duration_values: Vec<_> = members
                .iter()
                .filter_map(|member| member.duration_s)
                .collect();
            if duration_values.len() == members.len() {
                compare_spread(
                    self.id(),
                    name,
                    "duration",
                    duration_values,
                    group.max_duration_delta_s,
                    &rows,
                    &mut findings,
                );
            } else {
                gaps.push(
                    CoverageGap::new(
                        CoverageGapCode::MEASUREMENT_UNAVAILABLE,
                        format!(
                            "sync group '{name}' has {} of {} members with finite durations",
                            duration_values.len(),
                            members.len()
                        ),
                    )
                    .scope(scope.clone()),
                );
            }
            let frame_counts = members.iter().map(|member| member.frame_count).collect();
            compare_spread(
                self.id(),
                name,
                "frame count",
                frame_counts,
                f64::from(group.max_frame_count_delta),
                &rows,
                &mut findings,
            );

            let grids: Vec<_> = members
                .iter()
                .filter_map(|member| member.frame_grid)
                .collect();
            if grids.len() == members.len() {
                compare_spread(
                    self.id(),
                    name,
                    "FPS",
                    grids.iter().map(|grid| grid.fps).collect(),
                    group.max_fps_delta,
                    &rows,
                    &mut findings,
                );
                // An equal fps grid and duration necessarily has the same frame intervals;
                // report it independently because runtimes commonly surface this as frames.
                compare_spread(
                    self.id(),
                    name,
                    "frame intervals",
                    grids
                        .iter()
                        .map(|grid| grid.frame_intervals as f64)
                        .collect(),
                    f64::from(group.max_frame_count_delta),
                    &rows,
                    &mut findings,
                );
            } else {
                gaps.push(
                    CoverageGap::new(
                        CoverageGapCode::SYNC_FRAME_GRID_UNAVAILABLE,
                        format!("sync group '{name}' has {} of {} members with usable declared frame grids", grids.len(), members.len()),
                    )
                    .scope(scope.clone()),
                );
            }

            let endpoints: Vec<_> = members
                .iter()
                .filter_map(|member| member.endpoint)
                .collect();
            if endpoints.len() == members.len() {
                if endpoints.windows(2).any(|pair| pair[0] != pair[1]) {
                    findings.push(
                        Finding::new(
                            self.id(),
                            Severity::Error,
                            format!("sync group '{name}' has incompatible loop endpoint modes"),
                        )
                        .measured("mixed")
                        .expected("one shared endpoint mode")
                        .members(rows.clone()),
                    );
                }
            } else {
                gaps.push(
                    CoverageGap::new(
                        CoverageGapCode::MEASUREMENT_UNAVAILABLE,
                        format!("sync group '{name}' has {} of {} members with usable loop endpoint evidence", endpoints.len(), members.len()),
                    )
                    .scope(scope.clone()),
                );
            }
            evaluated_scopes.push(scope);
        }
        CheckOutput::from_coverage(findings, evaluated_scopes, gaps)
    }
}

#[derive(Clone, Copy)]
struct MemberEvidence {
    duration_s: Option<f64>,
    frame_count: f64,
    frame_grid: Option<crate::measure::FrameGridMeasurement>,
    endpoint: Option<crate::measure::LoopEndpointMode>,
}

fn compare_spread(
    check_id: &'static str,
    group: &str,
    label: &str,
    values: Vec<f64>,
    cap: f64,
    rows: &[MemberMeasurement],
    findings: &mut Vec<Finding>,
) {
    let Some(min) = values.iter().copied().reduce(f64::min) else {
        return;
    };
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let spread = max - min;
    if !spread.is_finite() {
        return;
    }
    if spread > cap {
        findings.push(
            Finding::new(
                check_id,
                Severity::Error,
                format!("sync group '{group}' {label} spread {spread:.6} exceeds cap {cap:.6}"),
            )
            .measured(spread)
            .expected(cap)
            .members(rows.to_vec()),
        );
    }
}
