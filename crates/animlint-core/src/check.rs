//! The check abstraction, its execution context, and the built-in
//! check sets.

use crate::config::Config;
use crate::finding::Finding;
use crate::model::Document;
use crate::profile::ResolvedRoles;
use crate::sample::{PoseGrid, sample_clip};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

/// Everything a check may consume: the document, the resolved rig
/// roles, the configuration, and a lazy per-clip [`PoseGrid`] cache
/// shared across checks.
pub struct CheckCtx<'a> {
    pub doc: &'a Document,
    pub roles: &'a ResolvedRoles,
    pub config: &'a Config,
    grids: RefCell<BTreeMap<usize, Rc<PoseGrid>>>,
}

impl<'a> CheckCtx<'a> {
    pub fn new(doc: &'a Document, roles: &'a ResolvedRoles, config: &'a Config) -> Self {
        Self {
            doc,
            roles,
            config,
            grids: RefCell::new(BTreeMap::new()),
        }
    }

    /// The metric pose grid for clip `clip_index`, computed once and
    /// shared. `None` for clips too short to carry a cycle.
    pub fn grid(&self, clip_index: usize) -> Option<Rc<PoseGrid>> {
        let clip = self.doc.clips.get(clip_index)?;
        let frames = crate::metrics::metric_frame_count(clip)?;
        Some(
            self.grids
                .borrow_mut()
                .entry(clip_index)
                .or_insert_with(|| Rc::new(sample_clip(&self.doc.skeleton, clip, frames)))
                .clone(),
        )
    }
}

pub trait Check {
    /// Stable identifier, e.g. `"loop-seam"`. Used in config, JSON
    /// output, and `--select`.
    fn id(&self) -> &'static str;

    fn run(&self, ctx: &CheckCtx, out: &mut Vec<Finding>);
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
    checks
}

/// Run `checks`, applying per-check severity overrides from the config
/// (`severity = "off"` drops a check's findings entirely).
pub fn run_checks(ctx: &CheckCtx, checks: &[Box<dyn Check>]) -> Vec<Finding> {
    let mut out = Vec::new();
    for check in checks {
        let mut findings = Vec::new();
        check.run(ctx, &mut findings);
        match ctx.config.check_settings(check.id()).severity {
            None => out.append(&mut findings),
            Some(setting) => {
                // as_severity() is None for "off": drop the findings.
                if let Some(severity) = setting.as_severity() {
                    for mut f in findings {
                        f.severity = severity;
                        out.push(f);
                    }
                }
            }
        }
    }
    out
}
