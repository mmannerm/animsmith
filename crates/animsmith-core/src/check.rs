//! The check abstraction, its execution context, and the built-in
//! check sets.

use crate::config::{ClipExpectations, Config};
use crate::evaluation::CheckOutput;
use crate::finding::{Finding, Severity};
use crate::metrics::MetricGrids;
use crate::model::Document;
use crate::profile::ResolvedRoles;
use crate::sample::PoseGrid;
use std::rc::Rc;

/// Everything a check may consume: the document, the resolved rig
/// roles, the configuration, and shared metric [`PoseGrid`] samples.
#[derive(Debug)]
pub struct CheckCtx<'a> {
    /// Document being checked.
    pub doc: &'a Document,
    /// Resolved rig roles for semantic checks.
    pub roles: &'a ResolvedRoles,
    /// Effective configuration for this run.
    pub config: &'a Config,
    grids: &'a MetricGrids<'a>,
    /// Effective per-clip expectations, resolved once and aligned to
    /// `doc.clips`. Resolving them means overlaying every matching glob
    /// entry (see [`Config::expectations_for`]); caching here keeps that
    /// off the per-check hot loop, which otherwise re-resolved the same
    /// clip once per check that reads expectations.
    expectations: Vec<ClipExpectations>,
}

impl<'a> CheckCtx<'a> {
    /// Build a check context that shares metric pose grids with
    /// measurement or report generation.
    ///
    /// `roles` must already reflect any [`Config::rig`](crate::Config::rig)
    /// profile and inline overrides; constructing a context does not resolve
    /// that declarative configuration.
    pub fn new(grids: &'a MetricGrids<'a>, roles: &'a ResolvedRoles, config: &'a Config) -> Self {
        let doc = grids.document();
        let expectations = doc
            .clips
            .iter()
            .map(|c| config.expectations_for(&c.name))
            .collect();
        Self {
            doc,
            roles,
            config,
            grids,
            expectations,
        }
    }

    /// The metric pose grid for clip `clip_index`, computed once and
    /// shared. `None` for clips too short to carry a cycle.
    pub fn grid(&self, clip_index: usize) -> Option<Rc<PoseGrid>> {
        self.grids.grid(clip_index)
    }

    /// Effective expectations for clip `clip_index` (resolved once in
    /// [`CheckCtx::new`]). Index into `doc.clips`.
    ///
    /// # Panics
    ///
    /// Panics if `clip_index` is outside the document's clip range.
    pub fn expectations(&self, clip_index: usize) -> &ClipExpectations {
        &self.expectations[clip_index]
    }

    /// Per-clip expectations in `doc.clips` order — for the readiness
    /// predicates that scan every clip for pending work.
    pub fn clip_expectations(&self) -> &[ClipExpectations] {
        &self.expectations
    }
}

/// Whether a check can run against a document — decided by the runner
/// *before* `run`, so requirement diagnostics are emitted in one place
/// and never subject to per-check severity overrides.
pub enum Readiness {
    /// Requirements met; run the check.
    Ready,
    /// The check has pending work (relevant expectations are declared)
    /// but a prerequisite — a rig role — is unresolved. The runner
    /// emits one standardized skip-note carrying `reason` at `Note`
    /// severity, exempt from overrides. `reason` states what is needed.
    Skipped(String),
    /// No pending work for this document/config; stay silent.
    Idle,
}

/// A lint check that can inspect a document and emit structured
/// [`Finding`] values.
///
/// Custom embedders may implement this trait and pass their checks to
/// [`run_checks`] alongside, or instead of, [`all_checks`]. Implementors
/// should keep `run` panic-free for loader-valid documents; use
/// [`Check::readiness`] to describe missing rig roles or configuration
/// prerequisites.
pub trait Check {
    /// Stable identifier, e.g. `"loop-seam"`. Used in config, JSON
    /// output, and `--select`.
    fn id(&self) -> &'static str;

    /// Whether the check's prerequisites are met. The default is
    /// [`Readiness::Ready`] — mechanical checks need no rig or config.
    /// Role-dependent checks override this to declare their needs so
    /// the runner, not the check, owns skip-note emission.
    fn readiness(&self, _ctx: &CheckCtx) -> Readiness {
        Readiness::Ready
    }

    /// Execute the check and append any findings to `out`.
    fn run(&self, ctx: &CheckCtx, out: &mut Vec<Finding>);

    /// Evaluate the check for the provisional v2 result model.
    ///
    /// The default treats [`Check::run`] as one complete work unit. Checks
    /// with independently executable sub-work override this to report typed
    /// gaps and completed scopes without encoding them as findings. The v2
    /// runner converts any legacy diagnostic findings into
    /// `legacy_diagnostic` gaps before classifying coverage.
    fn evaluate(&self, ctx: &CheckCtx) -> CheckOutput {
        let mut findings = Vec::new();
        self.run(ctx, &mut findings);
        CheckOutput::complete(findings)
    }
}

/// The mechanical P0 checks: no rig profile, no config required.
pub fn mechanical_checks() -> Vec<Box<dyn Check>> {
    vec![
        Box::new(crate::checks::nan::Nan),
        Box::new(crate::checks::time_monotonic::TimeMonotonic),
        Box::new(crate::checks::quat_norm::QuatNorm),
        Box::new(crate::checks::quat_flip::QuatFlip),
        Box::new(crate::checks::duration_sanity::DurationSanity),
        Box::new(crate::checks::scale_keys::ScaleKeys),
        Box::new(crate::checks::constant_track::ConstantTrack),
    ]
}

/// The full built-in catalog: mechanical + semantic checks.
pub fn all_checks() -> Vec<Box<dyn Check>> {
    let mut checks = mechanical_checks();
    checks.push(Box::new(crate::checks::missing_bones::MissingBones));
    checks.push(Box::new(crate::checks::frozen_bone::FrozenBone));
    checks.push(Box::new(crate::checks::loop_seam::LoopSeam));
    checks.push(Box::new(crate::checks::root_motion_speed::RootMotionSpeed));
    checks.push(Box::new(crate::checks::gait_group::GaitGroup));
    checks.push(Box::new(crate::checks::in_place::InPlace));
    checks.push(Box::new(crate::checks::fps::Fps));
    checks.push(Box::new(crate::checks::bind_pose::BindPose));
    checks.push(Box::new(crate::checks::foot_slide::FootSlide));
    checks
}

/// Run `checks`, honouring per-check severity settings:
///
/// - `severity = "off"` removes the check from the run set — it never
///   executes (no wasted sampling, no discarded findings).
/// - Any other override replaces the severity of the check's
///   *violations*. Diagnostics — the requirement skip-notes emitted by
///   the runner (via [`Check::readiness`]) and any a check marks with
///   [`Finding::as_diagnostic`] — are exempt, so declaring `severity =
///   "error"` never turns a "roles unresolved" note into a false
///   failure.
pub fn run_checks(ctx: &CheckCtx, checks: &[Box<dyn Check>]) -> Vec<Finding> {
    use crate::config::SeveritySetting;

    let mut out = Vec::new();
    for check in checks {
        let setting = ctx.config.check_settings(check.id()).severity;
        if setting == Some(SeveritySetting::Off) {
            continue; // off removes the check from the run set
        }
        match check.readiness(ctx) {
            Readiness::Idle => {}
            Readiness::Skipped(reason) => {
                out.push(
                    Finding::new(check.id(), Severity::Note, format!("skipped: {reason}"))
                        .as_diagnostic(),
                );
            }
            Readiness::Ready => {
                let mut findings = Vec::new();
                check.run(ctx, &mut findings);
                if let Some(severity) = setting.and_then(SeveritySetting::as_severity) {
                    for f in &mut findings {
                        if !f.diagnostic {
                            f.severity = severity;
                        }
                    }
                }
                out.append(&mut findings);
            }
        }
    }
    out
}
