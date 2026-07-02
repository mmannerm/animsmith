//! The check abstraction and the built-in check sets.
//!
//! Checks are deliberately tiny: one `id`, one `run` over a
//! [`Document`]. Configuration (severity overrides, tolerances, per-clip
//! expectations) arrives with rig profiles in M1; until then each check
//! carries sensible defaults documented on its type.

use crate::finding::Finding;
use crate::model::Document;

pub trait Check {
    /// Stable identifier, e.g. `"loop-seam"`. Used in config, JSON
    /// output, and `--select`.
    fn id(&self) -> &'static str;

    fn run(&self, doc: &Document, out: &mut Vec<Finding>);
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

pub fn run_checks(doc: &Document, checks: &[Box<dyn Check>]) -> Vec<Finding> {
    let mut out = Vec::new();
    for check in checks {
        check.run(doc, &mut out);
    }
    out
}
