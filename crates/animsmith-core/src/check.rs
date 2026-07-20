//! The check abstraction, its execution context, and the built-in
//! check sets.

use crate::config::{ClipExpectations, Config};
use crate::evaluation::{Applicability, CheckOutput};
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

    /// Per-clip expectations in `doc.clips` order, used by cheap
    /// applicability predicates that scan for declared work.
    pub fn clip_expectations(&self) -> &[ClipExpectations] {
        &self.expectations
    }
}

/// A lint check that can inspect a document and emit typed evaluation
/// coverage plus structured content findings.
///
/// Custom embedders may implement this trait and pass their checks to
/// [`crate::evaluate_checks`] alongside, or instead of, [`all_checks`].
/// Implementors should keep both methods panic-free for loader-valid
/// documents. Applicability describes whether declared work exists;
/// unavailable prerequisites or measurements belong in typed coverage gaps
/// returned from [`Check::evaluate`].
pub trait Check {
    /// Stable identifier, e.g. `"loop-seam"`. Used in config, JSON
    /// output, and `--select`.
    fn id(&self) -> &'static str;

    /// Whether this document and configuration declare work for the check.
    ///
    /// The runner calls this cheap predicate even for disabled or unselected
    /// checks so applicability remains an independent result dimension. It
    /// must not perform the check's substantive evaluation.
    fn applicability(&self, _ctx: &CheckCtx) -> Applicability {
        Applicability::Applicable
    }

    /// Evaluate every modelled work unit, returning content findings and
    /// explicit coverage. Missing prerequisites are gaps, never findings.
    fn evaluate(&self, ctx: &CheckCtx) -> CheckOutput;
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
