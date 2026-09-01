//! [`render`] turns [`animsmith_core::MetricGrids`],
//! [`animsmith_core::ResolvedRoles`], typed check evaluations, and optional
//! engine-prediction provenance into a self-contained HTML report.
//! The viewer is driven by the same [`animsmith_core::PoseGrid`] samples
//! the checks judged.
//!
//! The returned HTML is self-contained: CSS, JavaScript, findings, charts,
//! and sampled pose data are embedded in the string. There is no runtime
//! CDN dependency and no JavaScript-side resampling of the clip.
//!
//! # Quick start
//!
//! ```no_run
//! fn write_report(
//!     doc: &animsmith_core::Document,
//!     roles: &animsmith_core::ResolvedRoles,
//!     checks: &[animsmith_core::CheckEvaluation],
//! ) -> std::io::Result<()> {
//!     let grids = animsmith_core::MetricGrids::new(doc);
//!     let html = animsmith_report::render(&grids, roles, checks, None, None);
//!     std::fs::write("report.html", html)
//! }
//! ```
//!
//! # Build and API status
//!
//! The library crate has no public feature flags and supports the workspace
//! MSRV, Rust 1.88. Its Rust API is pre-1.0; see `animsmith-core`'s crate-level
//! API status for the shared stability boundary.
//!
//! See the GitHub [embedding guide] for composing this crate with checks and
//! the [pipeline scenario guide] for CI and outsourced-acceptance reporting
//! workflows.
//!
//! [embedding guide]: https://github.com/mmannerm/animsmith/blob/main/docs/embedding.md
//! [pipeline scenario guide]: https://github.com/mmannerm/animsmith/blob/main/docs/pipeline-scenarios.md
//!
#![warn(missing_docs)]

use animsmith_core::metrics::{MetricGrids, metric_frame_count};
use animsmith_core::profile::{ResolvedRoles, Role};
use animsmith_core::sample::PoseGrid;
use animsmith_core::stance_support::{StanceSideV1, resolve_stance_support_v1};
use animsmith_core::{
    CheckEvaluation, Config, LoadedSource, PredictionProvenanceV1, SourceObservationStateV1,
    SourceSetCoverageStateV1,
};
use base64::Engine as _;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};

/// The one colour authority for every generated document: dark by default,
/// light under `prefers-color-scheme`, and either one pinned by a
/// `#theme=` fragment.
const TOKENS_CSS: &str = include_str!("../assets/tokens.css");
/// Pure helpers shared by both viewers (design-token palette).
const SHARED_JS: &str = include_str!("../assets/shared.js");
const VIEWER_JS: &str = include_str!("../assets/viewer.js");
const VIEWER_CSS: &str = include_str!("../assets/viewer.css");
const COMPARISON_VIEWER_JS: &str = include_str!("../assets/comparison.js");
const COMPARISON_CSS: &str = include_str!("../assets/comparison.css");

/// Maximum pose-data bytes embedded by one side of a comparison report.
///
/// The bound is checked before the renderer allocates its binary pose buffer.
pub const MAX_COMPARISON_POSE_BYTES: usize = 32 * 1024 * 1024;
const MAX_COMPARISON_JSON_BYTES: usize = 48 * 1024 * 1024;
const MAX_COMPARISON_INPUT_TEXT_BYTES: usize = 1024 * 1024;
const MAX_COMPARISON_FINDINGS_PER_SIDE: usize = 4096;
const MAX_COMPARISON_GAPS_PER_SIDE: usize = 4096;
const MAX_COMPARISON_PREDICTION_FACETS_PER_SIDE: usize = 4096;
const MAX_COMPARISON_CONTEXT_ROWS_PER_SIDE: usize = 8192;
const MAX_COMPARISON_REPORT_TEXT_BYTES_PER_SIDE: usize = 4 * 1024 * 1024;
// Four f64 spellings, their keys and JSON punctuation fit well below this
// allowance for one stance run. The same bound also covers every seam,
// structural, gait, or stance object shell; variable authored strings are
// counted separately by `ReportTextCounter`.
const MAX_COMPARISON_CONTEXT_WIRE_BYTES_PER_ROW: usize = 512;

#[derive(Debug, Clone, Copy)]
struct SideReportPreflight {
    text_bytes: usize,
    context_rows: usize,
}

/// Validated bounded correspondence used before sampling or check evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComparisonPreflight {
    /// Selected before clip index in its document.
    pub before_clip_index: usize,
    /// Selected after clip index in its document.
    pub after_clip_index: usize,
    /// Exact before metric frame count.
    pub before_frames: usize,
    /// Exact after metric frame count.
    pub after_frames: usize,
}

/// One explicit input to [`render_comparison`].
#[derive(Clone, Copy)]
pub struct ComparisonSide<'a> {
    /// Exact loader authority for the normalized document and its complete
    /// rooted dependency closure.
    pub source: &'a LoadedSource,
    /// Metric pose grids computed from this side's loaded document.
    pub grids: &'a MetricGrids<'a>,
    /// Resolved roles for this side.
    pub roles: &'a ResolvedRoles,
    /// Typed check evaluations for this side.
    pub checks: &'a [CheckEvaluation],
    /// Exact configuration used to produce `checks`.
    ///
    /// The comparison reuses its effective `foot-slide` contact threshold
    /// when projecting typed stance scopes into sampled support runs.
    pub config: &'a Config,
    /// Optional engine-prediction provenance for this side.
    pub prediction_provenance: Option<&'a PredictionProvenanceV1>,
    /// Exact, caller-declared clip name to compare.
    pub clip: &'a str,
}

/// Refusal returned before a comparison report is published.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ComparisonError {
    /// The declared clip was absent from one input.
    #[error("{side} clip {clip:?} was not found")]
    ClipNotFound {
        /// Input side that did not contain the clip.
        side: &'static str,
        /// Explicit clip correspondence member that was absent.
        clip: String,
    },
    /// A document had more than one clip with the declared name.
    #[error("{side} clip {clip:?} is ambiguous")]
    AmbiguousClip {
        /// Input side containing duplicate clip names.
        side: &'static str,
        /// Explicit clip correspondence member that was ambiguous.
        clip: String,
    },
    /// A skeleton needs a unique bone name before cross-document mapping is safe.
    #[error("{side} skeleton has duplicate bone name {name:?}")]
    DuplicateBoneName {
        /// Input side containing the duplicate.
        side: &'static str,
        /// Duplicate bone name.
        name: String,
    },
    /// A skeleton violated its parent-before-child representation invariant.
    #[error("{side} skeleton has an invalid parent for bone {bone:?}")]
    InvalidHierarchy {
        /// Input side with the malformed hierarchy.
        side: &'static str,
        /// Bone that named a missing or later parent.
        bone: String,
    },
    /// The inputs do not have the same named skeleton.
    #[error("skeleton correspondence refused: {detail}")]
    IncompatibleSkeleton {
        /// Stable, operator-readable reason for refusal.
        detail: String,
    },
    /// The declared clip cannot supply its existing metric sample grid.
    #[error("{side} clip {clip:?} has no available metric sample grid")]
    UnavailableSampleGrid {
        /// Input side lacking the grid.
        side: &'static str,
        /// Declared clip that lacked the grid.
        clip: String,
    },
    /// Embedding one side's exact sampled poses would exceed the fixed budget.
    #[error("{side} pose grid requires {bytes} bytes, above the {limit}-byte comparison limit")]
    PoseWorkExceeded {
        /// Input side exceeding the limit.
        side: &'static str,
        /// Exact requested byte count, or `u128::MAX` on arithmetic overflow.
        bytes: u128,
        /// Fixed comparison budget.
        limit: usize,
    },
    /// Serializing comparison JSON would exceed the fixed report budget.
    #[error("comparison report JSON exceeds the {limit}-byte limit")]
    ReportWorkExceeded {
        /// Fixed comparison JSON budget.
        limit: usize,
    },
    /// Input names would exceed the fixed pre-evaluation text budget.
    #[error("{side} comparison input text requires {bytes} bytes, above the {limit}-byte limit")]
    InputTextWorkExceeded {
        /// Input side exceeding the bound.
        side: &'static str,
        /// Counted UTF-8 bytes.
        bytes: usize,
        /// Fixed input text budget.
        limit: usize,
    },
    /// The metric grids were not built from the supplied loader authority.
    #[error("{side} metric grids do not belong to the supplied source authority")]
    AuthorityDocumentMismatch {
        /// Input side whose authority and grids differed.
        side: &'static str,
    },
    /// A side lacked the exact complete dependency-closure identity required
    /// to distinguish immutable comparison authorities.
    #[error("{side} input has no complete dependency-closure identity")]
    IncompleteDependencyClosure {
        /// Input side whose rooted closure was incomplete.
        side: &'static str,
    },
    /// Before and after named the same complete immutable input authority.
    #[error("before and after have the same complete dependency-closure identity")]
    IdenticalAuthorities,
    /// The exact authored/parser clip-name authority was incomplete.
    #[error("{side} authored clip-name inventory is incomplete")]
    IncompleteAuthoredClipInventory {
        /// Input side whose raw source clip inventory was incomplete.
        side: &'static str,
    },
    /// A selected authored/parser clip name was ambiguous even if the loader
    /// disambiguated normalized document names.
    #[error("{side} authored clip {clip:?} is ambiguous")]
    AmbiguousAuthoredClip {
        /// Input side containing the duplicate authored name.
        side: &'static str,
        /// Explicit selected clip spelling.
        clip: String,
    },
    /// One selected side exceeded a fixed report row budget before report
    /// values were allocated.
    #[error("{side} comparison {kind} requires {found} rows, above the {limit}-row limit")]
    ReportRowsExceeded {
        /// Input side exceeding the bound.
        side: &'static str,
        /// Bounded row domain.
        kind: &'static str,
        /// Observed rows, capped at the N+1 witness.
        found: usize,
        /// Fixed row limit.
        limit: usize,
    },
    /// One selected side exceeded the fixed aggregate report-text budget.
    #[error("{side} comparison report text exceeds the {limit}-byte limit")]
    ReportTextWorkExceeded {
        /// Input side exceeding the bound.
        side: &'static str,
        /// Fixed aggregate text limit.
        limit: usize,
    },
    /// Prediction evidence was not bound to the exact side authority supplied
    /// to the public comparison boundary.
    #[error("{side} prediction authority is invalid: {detail}")]
    PredictionAuthorityMismatch {
        /// Input side whose prediction evidence was inconsistent.
        side: &'static str,
        /// Stable operator-readable mismatch reason.
        detail: &'static str,
    },
}

/// Validate explicit correspondence and all sampling work before evaluating checks.
///
/// This boundary intentionally does not ask [`MetricGrids`] for either grid.
/// Callers use it after loading the two immutable documents and before running
/// checks, then pass the same documents to [`render_comparison`].
pub fn preflight_comparison(
    before: &animsmith_core::Document,
    before_clip_name: &str,
    after: &animsmith_core::Document,
    after_clip_name: &str,
) -> Result<ComparisonPreflight, ComparisonError> {
    validate_skeletons(before, after)?;
    input_text_bytes(before, "before")?;
    input_text_bytes(after, "after")?;
    let before_clip = select_clip(before, before_clip_name, "before")?;
    let after_clip = select_clip(after, after_clip_name, "after")?;
    let before_frames = metric_frame_count(before_clip.1).ok_or_else(|| {
        ComparisonError::UnavailableSampleGrid {
            side: "before",
            clip: before_clip_name.to_owned(),
        }
    })?;
    let after_frames =
        metric_frame_count(after_clip.1).ok_or_else(|| ComparisonError::UnavailableSampleGrid {
            side: "after",
            clip: after_clip_name.to_owned(),
        })?;
    comparison_pose_bytes(before_frames, before.skeleton.bones.len(), "before")?;
    comparison_pose_bytes(after_frames, after.skeleton.bones.len(), "after")?;
    Ok(ComparisonPreflight {
        before_clip_index: before_clip.0,
        after_clip_index: after_clip.0,
        before_frames,
        after_frames,
    })
}

/// Validate loader-owned immutable authorities before check evaluation.
///
/// In addition to normalized document correspondence, this requires complete
/// rooted dependency closures, refuses identical before/after authorities,
/// and checks duplicate authored/parser clip names through the exact raw
/// source projection rather than the loader's disambiguated display names.
pub fn preflight_comparison_sources(
    before: &LoadedSource,
    before_clip_name: &str,
    after: &LoadedSource,
    after_clip_name: &str,
) -> Result<ComparisonPreflight, ComparisonError> {
    let preflight = preflight_comparison(
        before.document(),
        before_clip_name,
        after.document(),
        after_clip_name,
    )?;
    let before_identity = complete_source_closure_identity(before, "before")?;
    let after_identity = complete_source_closure_identity(after, "after")?;
    if before_identity == after_identity {
        return Err(ComparisonError::IdenticalAuthorities);
    }
    validate_source_authored_clip_name(before, preflight.before_clip_index, "before")?;
    validate_source_authored_clip_name(after, preflight.after_clip_index, "after")?;
    Ok(preflight)
}

/// Render a self-contained, synchronized before/after HTML diagnostic.
///
/// The caller declares the clip correspondence by supplying exactly one clip
/// name for each side.  The renderer refuses duplicate bone names, mismatched
/// named parent hierarchies, absent/ambiguous clips, unavailable metric grids,
/// or pose/report work beyond its fixed budgets.  It deliberately uses
/// normalized frame phase for unequal durations and labels both source times;
/// it does not infer an authored time warp.
pub fn render_comparison(
    before: ComparisonSide<'_>,
    after: ComparisonSide<'_>,
) -> Result<String, ComparisonError> {
    let before_doc = before.grids.document();
    let after_doc = after.grids.document();
    validate_side_authority(before, "before")?;
    validate_side_authority(after, "after")?;
    let preflight =
        preflight_comparison_sources(before.source, before.clip, after.source, after.clip)?;
    let before_report = preflight_side_report_work(before, before.clip, "before")?;
    let after_report = preflight_side_report_work(after, after.clip, "after")?;
    preflight_report_allocation(
        before_doc,
        after_doc,
        preflight,
        before_report,
        after_report,
    )?;
    let before_clip = &before_doc.clips[preflight.before_clip_index];
    let after_clip = &after_doc.clips[preflight.after_clip_index];
    let before_grid = before
        .grids
        .grid(preflight.before_clip_index)
        .ok_or_else(|| ComparisonError::UnavailableSampleGrid {
            side: "before",
            clip: before.clip.to_owned(),
        })?;
    let after_grid = after
        .grids
        .grid(preflight.after_clip_index)
        .ok_or_else(|| ComparisonError::UnavailableSampleGrid {
            side: "after",
            clip: after.clip.to_owned(),
        })?;

    let bones = comparison_bones(before_doc);
    let before_side = comparison_side_json(
        before,
        before_clip.name.as_str(),
        before_clip.duration_s,
        before_grid.as_ref(),
        "before",
    )?;
    let after_side = comparison_side_json(
        after,
        after_clip.name.as_str(),
        after_clip.duration_s,
        after_grid.as_ref(),
        "after",
    )?;
    let data = json!({
        "kind": "animsmith-comparison-v1",
        "correspondence": {
            "kind": "explicit_clip_names",
            "before_clip": before.clip,
            "after_clip": after.clip,
            "mapping": "normalized_phase",
            "disclosure": "Panels synchronize by normalized sampled-frame phase. Source times remain separate; this is not an authored time warp.",
        },
        "bones": bones,
        "before": before_side,
        "after": after_side,
    });
    let data = bounded_json(&data)?;
    // A `</script>`-bearing string inside data cannot terminate this element.
    let data = data.replace('<', "\\u003c");
    let before_clip_anchor = semantic_anchor("clip", before.clip);
    let after_clip_anchor = semantic_anchor("clip", after.clip);
    Ok(format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>animsmith — visual comparison</title><style>{TOKENS_CSS}{COMPARISON_CSS}</style></head>\n\
         <body><header><h1>animsmith visual comparison</h1><p id=\"mapping\"></p>\n\
         <p class=\"warning\">This comparison presents checked evidence only. An absent finding is not artistic, gameplay, or engine acceptance.</p></header>\n\
         <section class=\"sync\"><label>Shared phase <input id=\"scrub\" type=\"range\" min=\"0\" max=\"1000\" value=\"0\"></label><span id=\"times\"></span></section>\n\
         <section class=\"shared-chart\"><h2>Before/after root trajectory</h2><svg id=\"comparison-root-path\" viewBox=\"0 0 720 220\"></svg></section>\n\
         <main><section class=\"side\" id=\"before-panel\"><span id=\"before-{before_clip_anchor}\"></span><h2 id=\"clip-before\">Before</h2><p id=\"before-identity\"></p><canvas id=\"before-gl\"></canvas><p id=\"before-pose-context\" class=\"context-label\"></p><h3>Role trajectories</h3><svg id=\"before-path\" viewBox=\"0 0 360 180\"></svg><h3>Gait and sampled stance</h3><svg id=\"before-gait\" viewBox=\"0 0 360 180\"></svg><h3>Acceptance context</h3><ul id=\"before-contexts\"></ul><h3>Findings</h3><ul id=\"before-findings\"></ul><h3>Coverage gaps</h3><ul id=\"before-gaps\"></ul><h3>Prediction provenance</h3><pre id=\"before-predictions\"></pre></section>\n\
         <section class=\"side\" id=\"after-panel\"><span id=\"after-{after_clip_anchor}\"></span><h2 id=\"clip-after\">After</h2><p id=\"after-identity\"></p><canvas id=\"after-gl\"></canvas><p id=\"after-pose-context\" class=\"context-label\"></p><h3>Role trajectories</h3><svg id=\"after-path\" viewBox=\"0 0 360 180\"></svg><h3>Gait and sampled stance</h3><svg id=\"after-gait\" viewBox=\"0 0 360 180\"></svg><h3>Acceptance context</h3><ul id=\"after-contexts\"></ul><h3>Findings</h3><ul id=\"after-findings\"></ul><h3>Coverage gaps</h3><ul id=\"after-gaps\"></ul><h3>Prediction provenance</h3><pre id=\"after-predictions\"></pre></section></main>\n\
         <script>{SHARED_JS}</script><script type=\"application/json\" id=\"comparison-report-data\">{data}</script><script>{COMPARISON_VIEWER_JS}</script></body></html>\n"
    ))
}

fn select_clip<'a>(
    doc: &'a animsmith_core::Document,
    name: &str,
    side: &'static str,
) -> Result<(usize, &'a animsmith_core::Clip), ComparisonError> {
    let mut matches = doc
        .clips
        .iter()
        .enumerate()
        .filter(|(_, clip)| clip.name == name);
    let Some(found) = matches.next() else {
        return Err(ComparisonError::ClipNotFound {
            side,
            clip: name.to_owned(),
        });
    };
    if matches.next().is_some() {
        return Err(ComparisonError::AmbiguousClip {
            side,
            clip: name.to_owned(),
        });
    }
    Ok(found)
}

fn validate_side_authority(
    side: ComparisonSide<'_>,
    side_name: &'static str,
) -> Result<(), ComparisonError> {
    if !std::ptr::eq(side.source.document(), side.grids.document()) {
        return Err(ComparisonError::AuthorityDocumentMismatch { side: side_name });
    }
    Ok(())
}

fn complete_source_closure_identity<'a>(
    source: &'a LoadedSource,
    side_name: &'static str,
) -> Result<&'a animsmith_core::DependencyClosureIdentityV1, ComparisonError> {
    let closure = source.dependency_closure();
    if !closure.coverage().is_complete() {
        return Err(ComparisonError::IncompleteDependencyClosure { side: side_name });
    }
    closure
        .identity()
        .ok_or(ComparisonError::IncompleteDependencyClosure { side: side_name })
}

fn validate_source_authored_clip_name(
    source: &LoadedSource,
    normalized_clip_index: usize,
    side_name: &'static str,
) -> Result<(), ComparisonError> {
    let facts = source.source_facts();
    let clips = facts.clips();
    if clips.coverage().state() != SourceSetCoverageStateV1::Complete {
        return Err(ComparisonError::IncompleteAuthoredClipInventory { side: side_name });
    }
    let selected = clips.rows().iter().find(|row| {
        matches!(
            row.normalized_clip_index().state(),
            SourceObservationStateV1::Observed(index) if *index == normalized_clip_index
        )
    });
    let Some(SourceObservationStateV1::Observed(selected_name)) =
        selected.map(|row| row.source_name().state())
    else {
        // An unnamed source clip is still uniquely and exactly bound through
        // its source index -> normalized index mapping.
        return Ok(());
    };
    let matches = clips
        .rows()
        .iter()
        .filter(|row| {
            matches!(
                row.source_name().state(),
                SourceObservationStateV1::Observed(name) if name == selected_name
            )
        })
        .take(2)
        .count();
    if matches > 1 {
        return Err(ComparisonError::AmbiguousAuthoredClip {
            side: side_name,
            clip: selected_name.as_str().to_owned(),
        });
    }
    Ok(())
}

fn scope_applies(scope: Option<&animsmith_core::EvaluationScope>, clip_name: &str) -> bool {
    scope.is_none_or(|scope| {
        scope
            .subject
            .as_deref()
            .is_none_or(|subject| subject == clip_name)
    })
}

fn bounded_row_count<I>(
    rows: I,
    side: &'static str,
    kind: &'static str,
    limit: usize,
) -> Result<usize, ComparisonError>
where
    I: IntoIterator,
{
    let found = rows.into_iter().take(limit.saturating_add(1)).count();
    if found > limit {
        return Err(ComparisonError::ReportRowsExceeded {
            side,
            kind,
            found,
            limit,
        });
    }
    Ok(found)
}

struct ReportTextCounter {
    bytes: usize,
}

impl Write for ReportTextCounter {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.bytes = self
            .bytes
            .checked_add(data.len())
            .ok_or_else(|| io::Error::other("comparison report text limit"))?;
        if self.bytes > MAX_COMPARISON_REPORT_TEXT_BYTES_PER_SIDE {
            return Err(io::Error::other("comparison report text limit"));
        }
        Ok(data.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn preflight_side_report_work(
    side: ComparisonSide<'_>,
    clip_name: &str,
    side_name: &'static str,
) -> Result<SideReportPreflight, ComparisonError> {
    validate_prediction_authority(side, side_name)?;
    let selected_findings = side
        .checks
        .iter()
        .flat_map(CheckEvaluation::findings)
        .filter(|finding| finding.clip.as_deref().is_none_or(|clip| clip == clip_name));
    bounded_row_count(
        selected_findings.clone(),
        side_name,
        "findings",
        MAX_COMPARISON_FINDINGS_PER_SIDE,
    )?;
    let selected_gaps = side.checks.iter().flat_map(|check| {
        check
            .gaps()
            .iter()
            .filter(|gap| scope_applies(gap.scope.as_ref(), clip_name))
    });
    bounded_row_count(
        selected_gaps.clone(),
        side_name,
        "coverage gaps",
        MAX_COMPARISON_GAPS_PER_SIDE,
    )?;
    let selected_facets = side
        .checks
        .iter()
        .filter_map(CheckEvaluation::engine_prediction)
        .flat_map(|prediction| prediction.facets())
        .filter(|facet| scope_applies(Some(facet.scope()), clip_name));
    bounded_row_count(
        selected_facets.clone(),
        side_name,
        "prediction facets",
        MAX_COMPARISON_PREDICTION_FACETS_PER_SIDE,
    )?;

    let clip_index = select_clip(side.grids.document(), clip_name, side_name)?.0;
    let frames = metric_frame_count(&side.grids.document().clips[clip_index]).unwrap_or(0);
    let finding_contexts = selected_findings
        .clone()
        .filter(|finding| {
            matches!(
                finding.check_id,
                "loop-closure"
                    | "loop-seam"
                    | "loop-seam-vel"
                    | "loop-seam-rot"
                    | "constant-track"
                    | "quat-flip"
                    | "quat-norm"
            )
        })
        .count();
    let has_stance_context = side.checks.iter().any(|check| {
        check.check_id() == "foot-slide"
            && check.evaluated_scopes().iter().any(|scope| {
                matches!(
                    scope.code.as_str(),
                    "left_foot_stance" | "right_foot_stance"
                ) && scope.subject.as_deref() == Some(clip_name)
            })
    });
    // One retained stance run can begin or end at every sampled frame; add
    // the two side rows themselves. Finding-derived seam/structural contexts
    // are counted exactly from their typed check authorities.
    let stance_contexts = if has_stance_context {
        frames.saturating_add(2)
    } else {
        0
    };
    let has_gait_context = side.roles.get(Role::Hips).is_some()
        && (side.roles.get(Role::LeftFoot).is_some() || side.roles.get(Role::LeftToe).is_some())
        && (side.roles.get(Role::RightFoot).is_some() || side.roles.get(Role::RightToe).is_some());
    let context_upper_bound = stance_contexts
        .saturating_add(finding_contexts)
        .saturating_add(usize::from(has_gait_context));
    if context_upper_bound > MAX_COMPARISON_CONTEXT_ROWS_PER_SIDE {
        return Err(ComparisonError::ReportRowsExceeded {
            side: side_name,
            kind: "diagnostic contexts",
            found: MAX_COMPARISON_CONTEXT_ROWS_PER_SIDE.saturating_add(1),
            limit: MAX_COMPARISON_CONTEXT_ROWS_PER_SIDE,
        });
    }

    let mut counter = ReportTextCounter { bytes: 0 };
    macro_rules! count_wire {
        ($value:expr) => {
            serde_json::to_writer(&mut counter, $value).map_err(|_| {
                ComparisonError::ReportTextWorkExceeded {
                    side: side_name,
                    limit: MAX_COMPARISON_REPORT_TEXT_BYTES_PER_SIDE,
                }
            })?
        };
    }
    // A finding is serialized once as a finding row and may duplicate all of
    // its authored text into both seam and structural context projections.
    // Counting all three copies is conservative without first allocating the
    // eventual serde Values or inspecting check-specific strings.
    for finding in selected_findings {
        count_wire!(finding);
        count_wire!(finding);
        count_wire!(finding);
    }
    for check in side.checks {
        for gap in check
            .gaps()
            .iter()
            .filter(|gap| scope_applies(gap.scope.as_ref(), clip_name))
        {
            count_wire!(&check.check_id());
            count_wire!(gap);
        }
        if let Some(prediction) = check.engine_prediction()
            && prediction
                .facets()
                .iter()
                .any(|facet| scope_applies(Some(facet.scope()), clip_name))
        {
            // Count the whole attachment rather than just selected facets:
            // this includes every repeated check id, identity, basis string,
            // unavailable reason and provenance-shaped field conservatively.
            count_wire!(&check.check_id());
            count_wire!(prediction);
        }
    }
    // The selected names are repeated in correspondence, side headers,
    // anchors, selected scopes, and contextual rows. This intentionally
    // overcounts rather than allocating the final wire tree to discover the
    // exact repetition count.
    for _ in 0..16 {
        count_wire!(&clip_name);
    }
    if let Some(provenance) = side.prediction_provenance {
        count_wire!(provenance);
    }
    Ok(SideReportPreflight {
        text_bytes: counter.bytes,
        context_rows: context_upper_bound,
    })
}

fn validate_prediction_authority(
    side: ComparisonSide<'_>,
    side_name: &'static str,
) -> Result<(), ComparisonError> {
    let predictions = side
        .checks
        .iter()
        .filter_map(CheckEvaluation::engine_prediction)
        .collect::<Vec<_>>();
    let Some(provenance) = side.prediction_provenance else {
        if predictions.is_empty() {
            return Ok(());
        }
        return Err(ComparisonError::PredictionAuthorityMismatch {
            side: side_name,
            detail: "prediction attachment has no supplied provenance",
        });
    };
    if provenance.dependency_closure() != side.source.dependency_closure() {
        return Err(ComparisonError::PredictionAuthorityMismatch {
            side: side_name,
            detail: "provenance dependency closure differs from the loaded source",
        });
    }
    if predictions
        .iter()
        .any(|prediction| prediction.provenance_identity() != provenance.identity())
    {
        return Err(ComparisonError::PredictionAuthorityMismatch {
            side: side_name,
            detail: "prediction attachment identity differs from supplied provenance",
        });
    }
    Ok(())
}

fn preflight_report_allocation(
    before: &animsmith_core::Document,
    after: &animsmith_core::Document,
    preflight: ComparisonPreflight,
    before_report: SideReportPreflight,
    after_report: SideReportPreflight,
) -> Result<(), ComparisonError> {
    let before_pose = comparison_pose_bytes(
        preflight.before_frames,
        before.skeleton.bones.len(),
        "before",
    )?;
    let after_pose =
        comparison_pose_bytes(preflight.after_frames, after.skeleton.bones.len(), "after")?;
    let base64_bytes = [before_pose, after_pose]
        .into_iter()
        .try_fold(0u128, |total, bytes| {
            let encoded = bytes.checked_add(2)?.checked_div(3)?.checked_mul(4)?;
            total.checked_add(encoded)
        })
        .unwrap_or(u128::MAX);
    // JSON f64 spellings are at most 24 bytes for finite values in serde's
    // shortest-roundtrip representation. Six bytes per source-name byte is
    // the worst JSON Unicode/control escape expansion. The fixed allowance
    // covers keys, identities, arrays, roles, and the embedded contract shell.
    let time_bytes = (preflight.before_frames as u128)
        .saturating_add(preflight.after_frames as u128)
        .saturating_mul(24);
    let bone_name_bytes = before
        .skeleton
        .bones
        .iter()
        .chain(after.skeleton.bones.iter())
        .map(|bone| bone.name.len() as u128)
        .sum::<u128>()
        .saturating_mul(6);
    let estimate = base64_bytes
        .saturating_add(time_bytes)
        .saturating_add(bone_name_bytes)
        .saturating_add(before_report.text_bytes as u128)
        .saturating_add(after_report.text_bytes as u128)
        .saturating_add(
            (before_report.context_rows as u128)
                .saturating_add(after_report.context_rows as u128)
                .saturating_mul(MAX_COMPARISON_CONTEXT_WIRE_BYTES_PER_ROW as u128),
        )
        .saturating_add(64 * 1024);
    require_report_estimate(estimate)
}

fn require_report_estimate(estimate: u128) -> Result<(), ComparisonError> {
    if estimate > MAX_COMPARISON_JSON_BYTES as u128 {
        return Err(ComparisonError::ReportWorkExceeded {
            limit: MAX_COMPARISON_JSON_BYTES,
        });
    }
    Ok(())
}

fn validate_skeletons(
    before: &animsmith_core::Document,
    after: &animsmith_core::Document,
) -> Result<(), ComparisonError> {
    let before_names = skeleton_names(&before.skeleton, "before")?;
    let after_names = skeleton_names(&after.skeleton, "after")?;
    if before_names.is_empty() {
        return Err(ComparisonError::IncompatibleSkeleton {
            detail: "skeleton has no bones".into(),
        });
    }
    if before_names.len() != after_names.len() {
        return Err(ComparisonError::IncompatibleSkeleton {
            detail: "bone counts differ".into(),
        });
    }
    for (name, before_index) in &before_names {
        let Some(after_index) = after_names.get(name) else {
            return Err(ComparisonError::IncompatibleSkeleton {
                detail: format!("bone {name:?} is absent after"),
            });
        };
        // The offline viewer uses one shared bone-index array for both pose
        // buffers. Refuse instead of silently drawing an after pose under the
        // before bone's label when two otherwise equivalent hierarchies were
        // serialized in different orders.
        if before_index != after_index {
            return Err(ComparisonError::IncompatibleSkeleton {
                detail: format!("bone {name:?} has a different index"),
            });
        }
        let before_parent = before.skeleton.bones[*before_index]
            .parent
            .map(|p| before.skeleton.bones[p].name.as_str());
        let after_parent = after.skeleton.bones[*after_index]
            .parent
            .map(|p| after.skeleton.bones[p].name.as_str());
        if before_parent != after_parent {
            return Err(ComparisonError::IncompatibleSkeleton {
                detail: format!("bone {name:?} has a different parent"),
            });
        }
    }
    Ok(())
}

fn skeleton_names<'a>(
    skeleton: &'a animsmith_core::Skeleton,
    side: &'static str,
) -> Result<BTreeMap<&'a str, usize>, ComparisonError> {
    let mut names = BTreeMap::new();
    for (index, bone) in skeleton.bones.iter().enumerate() {
        if bone.parent.is_some_and(|parent| parent >= index) {
            return Err(ComparisonError::InvalidHierarchy {
                side,
                bone: bone.name.clone(),
            });
        }
        if names.insert(bone.name.as_str(), index).is_some() {
            return Err(ComparisonError::DuplicateBoneName {
                side,
                name: bone.name.clone(),
            });
        }
    }
    Ok(names)
}

fn input_text_bytes(
    doc: &animsmith_core::Document,
    side: &'static str,
) -> Result<(), ComparisonError> {
    let bytes = doc
        .skeleton
        .bones
        .iter()
        .map(|bone| bone.name.len())
        .chain(doc.clips.iter().map(|clip| clip.name.len()))
        .try_fold(0usize, |total, bytes| total.checked_add(bytes))
        .unwrap_or(usize::MAX);
    if bytes > MAX_COMPARISON_INPUT_TEXT_BYTES {
        return Err(ComparisonError::InputTextWorkExceeded {
            side,
            bytes,
            limit: MAX_COMPARISON_INPUT_TEXT_BYTES,
        });
    }
    Ok(())
}

fn comparison_bones(doc: &animsmith_core::Document) -> Vec<Value> {
    doc.skeleton
        .bones
        .iter()
        .map(|bone| {
            json!({
                "name": bone.name,
                "parent": bone.parent.map(|parent| parent as i64).unwrap_or(-1),
            })
        })
        .collect()
}

fn comparison_side_json(
    side: ComparisonSide<'_>,
    clip_name: &str,
    duration_s: f64,
    grid: &PoseGrid,
    side_name: &'static str,
) -> Result<Value, ComparisonError> {
    let frames = grid.frame_count();
    let bones = side.grids.document().skeleton.bones.len();
    let bytes = comparison_pose_bytes(frames, bones, side_name)?;
    let mut positions = Vec::with_capacity(bytes as usize);
    for frame in 0..frames {
        for bone in 0..bones {
            let point = grid.model_position(frame, bone);
            positions.extend_from_slice(&point.x.to_le_bytes());
            positions.extend_from_slice(&point.y.to_le_bytes());
            positions.extend_from_slice(&point.z.to_le_bytes());
        }
    }
    let trails: Value = [
        (Role::Root, "root"),
        (Role::Hips, "hips"),
        (Role::LeftFoot, "left_foot"),
        (Role::RightFoot, "right_foot"),
    ]
    .iter()
    .filter_map(|(role, name)| {
        side.roles
            .get(*role)
            .map(|index| (name.to_string(), json!(index)))
    })
    .collect::<serde_json::Map<_, _>>()
    .into();
    let anchored_findings = side
        .checks
        .iter()
        .flat_map(CheckEvaluation::findings)
        .filter(|finding| finding.clip.as_deref().is_none_or(|clip| clip == clip_name))
        .enumerate()
        .map(|(ordinal, finding)| (finding, finding_anchor(side_name, ordinal, finding)))
        .collect::<Vec<_>>();
    let findings = anchored_findings
        .iter()
        .map(|(finding, anchor)| {
            let subject_bone = finding_subject_bone(side, finding);
            json!({"anchor":anchor,"check":finding.check_id,"severity":finding.severity.to_string(),"clip":finding.clip,"bone":finding.bone,"node":finding.node,"subject_bone":subject_bone,"time":finding.time_s,"measured":finding.measured,"expected":finding.expected,"members":finding.members,"prediction_scope":finding.prediction_scope,"message":finding.message})
        })
        .collect::<Vec<_>>();
    let gaps = side.checks.iter().flat_map(|check| check.gaps().iter().map(move |gap| (check.check_id(), gap)))
        .filter(|(_, gap)| scope_applies(gap.scope.as_ref(), clip_name))
        .map(|(check_id, gap)| json!({"check_id":check_id,"code":gap.code,"message":gap.message,"scope":gap.scope}))
        .collect::<Vec<_>>();
    let predictions = side
        .checks
        .iter()
        .filter_map(|check| {
            let prediction = check.engine_prediction()?;
            let facets = prediction
                .facets()
                .iter()
                .filter(|facet| scope_applies(Some(facet.scope()), clip_name))
                .collect::<Vec<_>>();
            (!facets.is_empty()).then(|| {
                json!({
                    "check_id": check.check_id(),
                    "prediction": {
                        "schema": prediction.contract_id(),
                        "provenance_identity": prediction.provenance_identity(),
                        "facets": facets,
                    }
                })
            })
        })
        .collect::<Vec<_>>();
    let primary = side.source.dependency_closure().primary_input();
    let closure_identity = side
        .source
        .dependency_closure()
        .identity()
        .expect("comparison authority preflight requires a complete closure");
    Ok(json!({
        "identity": {"sha256": primary.sha256(), "bytes": primary.bytes()},
        "dependency_closure_identity": closure_identity,
        "clip": {"anchor":semantic_anchor("clip", clip_name),"name":clip_name,"duration":duration_s,"frames":frames,"times":grid.times,"positions":base64::engine::general_purpose::STANDARD.encode(positions),"trails":trails},
        "contexts": comparison_contexts(side, clip_name, grid, &anchored_findings),
        "findings":findings,"gaps":gaps,"prediction_provenance":side.prediction_provenance,"predictions":predictions,
    }))
}

fn comparison_contexts(
    side: ComparisonSide<'_>,
    clip_name: &str,
    grid: &PoseGrid,
    anchored_findings: &[(&animsmith_core::Finding, String)],
) -> Value {
    let left_gait_role = side
        .roles
        .get(Role::LeftFoot)
        .map(|bone| (Role::LeftFoot, bone))
        .or_else(|| {
            side.roles
                .get(Role::LeftToe)
                .map(|bone| (Role::LeftToe, bone))
        });
    let right_gait_role = side
        .roles
        .get(Role::RightFoot)
        .map(|bone| (Role::RightFoot, bone))
        .or_else(|| {
            side.roles
                .get(Role::RightToe)
                .map(|bone| (Role::RightToe, bone))
        });
    let gait = match (side.roles.get(Role::Hips), left_gait_role, right_gait_role) {
        (Some(hips), Some((left_role, left)), Some((right_role, right))) => json!({
            "source": "exact sampled pose-grid model-space selected foot/toe heights relative to hips",
            "hips": hips,
            "left": left,
            "left_role": left_role.as_str(),
            "right": right,
            "right_role": right_role.as_str(),
        }),
        _ => Value::Null,
    };

    let contact_height_m = side
        .config
        .check_settings("foot-slide")
        .contact_height_m
        .unwrap_or(animsmith_core::DEFAULT_CONTACT_HEIGHT_M);
    let stances = [
        (StanceSideV1::Left, "left", "left_foot_stance"),
        (StanceSideV1::Right, "right", "right_foot_stance"),
    ]
    .into_iter()
    .filter(|(_, _, scope_code)| {
        side.checks.iter().any(|check| {
            check.check_id() == "foot-slide"
                && check.evaluated_scopes().iter().any(|scope| {
                    scope.code.as_str() == *scope_code
                        && scope.subject.as_deref() == Some(clip_name)
                })
        })
    })
    .filter_map(|(stance_side, label, scope_code)| {
        let stance = resolve_stance_support_v1(grid, side.roles, stance_side, contact_height_m)?;
        let bone = stance.bone();
        let bone_name = side
            .grids
            .document()
            .skeleton
            .bones
            .get(bone)?
            .name
            .as_str();
        let runs = stance
            .retained_runs()
            .map(|run| {
                json!({
                    "start_frame": run.start_frame,
                    "end_frame": run.end_frame,
                    "start_s": grid.times[run.start_frame],
                    "end_s": grid.times[run.end_frame],
                })
            })
            .collect::<Vec<_>>();
        Some(json!({
            "source": "typed foot-slide evaluated scope plus shared V1 sampled stance classifier",
            "scope": scope_code,
            "side": label,
            "selected_role": stance.role().as_str(),
            "bone": bone,
            "bone_name": bone_name,
            "contact_height_m": contact_height_m,
            "runs": runs,
        }))
    })
    .collect::<Vec<_>>();

    let mut seams = Vec::new();
    let mut structural = Vec::new();
    for (finding, anchor) in anchored_findings
        .iter()
        .filter(|(finding, _)| finding.clip.as_deref() == Some(clip_name))
    {
        if matches!(
            finding.check_id,
            "loop-closure" | "loop-seam" | "loop-seam-vel" | "loop-seam-rot"
        ) {
            let subject_bone = finding_subject_bone(side, finding);
            seams.push(json!({
                "source": "typed seam finding on exact sampled endpoint poses",
                "finding_anchor": anchor,
                "check": finding.check_id,
                "first_frame": 0,
                "last_frame": grid.frame_count() - 1,
                "first_s": grid.times[0],
                "last_s": grid.times[grid.frame_count() - 1],
                "subject_bone": subject_bone,
                "subject_bone_name": finding.bone,
            }));
        }
        if matches!(
            finding.check_id,
            "constant-track" | "quat-flip" | "quat-norm"
        ) {
            structural.push(json!({
                "source": "typed check finding; no visual pose difference is implied",
                "finding_anchor": anchor,
                "check": finding.check_id,
                "evidence_kind": "structural",
                "subject_bone_name": finding.bone,
                "label": "structural evidence — poses may look unchanged",
            }));
        }
    }

    json!({
        "gait": gait,
        "stances": stances,
        "seams": seams,
        "structural": structural,
    })
}

fn finding_subject_bone(
    side: ComparisonSide<'_>,
    finding: &animsmith_core::Finding,
) -> Option<usize> {
    if let Some(name) = finding.bone.as_ref() {
        return side
            .grids
            .document()
            .skeleton
            .bones
            .iter()
            .position(|bone| bone.name == *name);
    }
    let target = finding.node.as_deref()?;
    let source = side.source.source_facts().source_skeleton();
    let by_index = source
        .nodes
        .iter()
        .map(|node| (node.source_node_index, node))
        .collect::<BTreeMap<_, _>>();
    source.nodes.iter().find_map(|node| {
        (source_node_path(node.source_node_index, &by_index) == target)
            .then_some(node.bone)
            .flatten()
    })
}

fn source_node_path(
    source_node_index: usize,
    by_index: &BTreeMap<usize, &animsmith_core::SourceNodeAsset>,
) -> String {
    let mut components = Vec::new();
    let mut current = Some(source_node_index);
    let mut visited = BTreeSet::new();
    while let Some(index) = current {
        if !visited.insert(index) {
            break;
        }
        let Some(node) = by_index.get(&index) else {
            break;
        };
        components.push(format!(
            "#{}({})",
            node.source_node_index,
            node.name.as_deref().unwrap_or("<unnamed>")
        ));
        current = node.parent_source_node_index;
    }
    components.reverse();
    components.join("/")
}

fn finding_anchor(side: &'static str, ordinal: usize, finding: &animsmith_core::Finding) -> String {
    let material = format!(
        "{side}\u{1f}{ordinal}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{:?}\u{1f}{}",
        finding.check_id,
        finding.clip.as_deref().unwrap_or(""),
        finding.bone.as_deref().unwrap_or(""),
        finding.node.as_deref().unwrap_or(""),
        finding.time_s,
        finding.message,
    );
    format!(
        "finding-{}",
        &animsmith_core::sha256_hex(material.as_bytes())[..16]
    )
}

fn semantic_anchor(kind: &str, material: &str) -> String {
    format!(
        "{kind}-{}",
        &animsmith_core::sha256_hex(material.as_bytes())[..16]
    )
}

fn comparison_pose_bytes(
    frames: usize,
    bones: usize,
    side: &'static str,
) -> Result<u128, ComparisonError> {
    let bytes = (frames as u128)
        .saturating_mul(bones as u128)
        .saturating_mul(3)
        .saturating_mul(4);
    if bytes > MAX_COMPARISON_POSE_BYTES as u128 {
        return Err(ComparisonError::PoseWorkExceeded {
            side,
            bytes,
            limit: MAX_COMPARISON_POSE_BYTES,
        });
    }
    Ok(bytes)
}

struct BoundedCounter {
    bytes: usize,
}
impl Write for BoundedCounter {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.bytes = self
            .bytes
            .checked_add(data.len())
            .ok_or_else(|| io::Error::other("comparison report limit"))?;
        if self.bytes > MAX_COMPARISON_JSON_BYTES {
            return Err(io::Error::other("comparison report limit"));
        }
        Ok(data.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn bounded_json(data: &Value) -> Result<String, ComparisonError> {
    serde_json::to_writer(BoundedCounter { bytes: 0 }, data).map_err(|_| {
        ComparisonError::ReportWorkExceeded {
            limit: MAX_COMPARISON_JSON_BYTES,
        }
    })?;
    serde_json::to_string(data).map_err(|_| ComparisonError::ReportWorkExceeded {
        limit: MAX_COMPARISON_JSON_BYTES,
    })
}

#[cfg(test)]
mod comparison_tests {
    use super::*;

    #[test]
    fn comparison_pose_budget_refuses_before_allocation() {
        let error = comparison_pose_bytes(MAX_COMPARISON_POSE_BYTES, 2, "before")
            .expect_err("two bones at this frame count exceed the fixed limit");
        assert_eq!(
            error,
            ComparisonError::PoseWorkExceeded {
                side: "before",
                bytes: (MAX_COMPARISON_POSE_BYTES as u128) * 2 * 3 * 4,
                limit: MAX_COMPARISON_POSE_BYTES,
            }
        );
    }

    #[test]
    fn comparison_row_budgets_admit_max_and_refuse_n_plus_one() {
        assert_eq!(
            bounded_row_count(
                0..MAX_COMPARISON_FINDINGS_PER_SIDE,
                "before",
                "findings",
                MAX_COMPARISON_FINDINGS_PER_SIDE,
            )
            .unwrap(),
            MAX_COMPARISON_FINDINGS_PER_SIDE
        );
        assert_eq!(
            bounded_row_count(
                0..MAX_COMPARISON_FINDINGS_PER_SIDE + 1,
                "before",
                "findings",
                MAX_COMPARISON_FINDINGS_PER_SIDE,
            ),
            Err(ComparisonError::ReportRowsExceeded {
                side: "before",
                kind: "findings",
                found: MAX_COMPARISON_FINDINGS_PER_SIDE + 1,
                limit: MAX_COMPARISON_FINDINGS_PER_SIDE,
            })
        );
    }

    #[test]
    fn comparison_text_budget_admits_max_and_refuses_n_plus_one() {
        let mut counter = ReportTextCounter { bytes: 0 };
        assert_eq!(
            counter
                .write(&vec![0; MAX_COMPARISON_REPORT_TEXT_BYTES_PER_SIDE])
                .unwrap(),
            MAX_COMPARISON_REPORT_TEXT_BYTES_PER_SIDE
        );
        assert!(counter.write(&[0]).is_err());
    }

    #[test]
    fn comparison_report_budget_admits_max_and_refuses_n_plus_one() {
        assert_eq!(
            require_report_estimate(MAX_COMPARISON_JSON_BYTES as u128),
            Ok(())
        );
        assert_eq!(
            require_report_estimate(MAX_COMPARISON_JSON_BYTES as u128 + 1),
            Err(ComparisonError::ReportWorkExceeded {
                limit: MAX_COMPARISON_JSON_BYTES,
            })
        );
    }
}

/// Escape untrusted text (clip/bone names, paths from the linted
/// asset) for interpolation into HTML markup and attributes.
fn esc(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Render report HTML from shared metric pose grids.
///
/// `clip_filter` restricts the report to one clip name when present. The
/// function performs no filesystem I/O and cannot report write errors;
/// callers choose where to store or serve the returned self-contained HTML
/// string.
pub fn render(
    grids: &MetricGrids<'_>,
    roles: &ResolvedRoles,
    checks: &[CheckEvaluation],
    prediction_provenance: Option<&PredictionProvenanceV1>,
    clip_filter: Option<&str>,
) -> String {
    let doc = grids.document();
    let bones: Vec<Value> = doc
        .skeleton
        .bones
        .iter()
        .map(|b| json!({ "name": b.name, "parent": b.parent.map(|p| p as i64).unwrap_or(-1) }))
        .collect();

    let trail_roles = [
        (Role::Root, "root"),
        (Role::Hips, "hips"),
        (Role::LeftFoot, "left_foot"),
        (Role::RightFoot, "right_foot"),
    ];

    let mut clips_json: Vec<Value> = Vec::new();
    let mut charts_html = String::new();
    for (clip_index, clip) in doc.clips.iter().enumerate() {
        if clip_filter.is_some_and(|f| f != clip.name) {
            continue;
        }
        let Some(grid) = grids.grid(clip_index) else {
            continue;
        };
        let frames = grid.frame_count();
        let nb = doc.skeleton.bones.len();
        let mut positions = Vec::with_capacity(frames * nb * 3 * 4);
        for f in 0..frames {
            for b in 0..nb {
                let p = grid.model_position(f, b);
                positions.extend_from_slice(&p.x.to_le_bytes());
                positions.extend_from_slice(&p.y.to_le_bytes());
                positions.extend_from_slice(&p.z.to_le_bytes());
            }
        }
        let trails: Value = trail_roles
            .iter()
            .filter_map(|&(role, name)| roles.get(role).map(|id| (name.to_string(), json!(id))))
            .collect::<serde_json::Map<_, _>>()
            .into();
        clips_json.push(json!({
            "name": clip.name,
            "duration": clip.duration_s,
            "frames": frames,
            "positions": base64::engine::general_purpose::STANDARD.encode(&positions),
            "trails": trails,
        }));
        charts_html.push_str(&clip_charts(&clip.name, grid.as_ref(), roles));
    }

    let findings_json: Vec<Value> = checks
        .iter()
        .flat_map(CheckEvaluation::findings)
        .filter(|f| clip_filter.is_none() || f.clip.as_deref() == clip_filter || f.clip.is_none())
        .map(|f| {
            json!({
                "check": f.check_id,
                "severity": f.severity.to_string(),
                "clip": f.clip,
                "bone": f.bone,
                "node": f.node,
                "time": f.time_s,
                "message": f.message,
            })
        })
        .collect();

    let predictions_json: Vec<Value> = checks
        .iter()
        .filter_map(|check| {
            check.engine_prediction().map(|prediction| {
                json!({
                    "check_id": check.check_id(),
                    "prediction": prediction,
                })
            })
        })
        .collect();

    let gaps_json: Vec<Value> = checks
        .iter()
        .flat_map(|check| {
            check.gaps().iter().map(|gap| {
                json!({
                    "check_id": check.check_id(),
                    "code": gap.code,
                    "message": gap.message,
                    "scope": gap.scope,
                })
            })
        })
        .collect();

    let data = json!({
        "file": doc.source.path,
        "profile": roles.profile,
        "bones": bones,
        "clips": clips_json,
        "findings": findings_json,
        "gaps": gaps_json,
        "prediction_provenance": prediction_provenance,
        "predictions": predictions_json,
    });

    let title = esc(doc
        .source
        .path
        .as_deref()
        .and_then(|p| p.rsplit(['/', '\\']).next())
        .unwrap_or("animsmith report"));
    // A `</script>`-bearing string inside the JSON would terminate the
    // data block early; escaping `<` inside JSON strings is lossless.
    let data = data.to_string().replace('<', "\\u003c");

    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>animsmith — {title}</title>\n<style>{TOKENS_CSS}{VIEWER_CSS}</style>\n</head>\n<body>\n\
         <header><h1>animsmith report</h1><span id=\"file\"></span></header>\n\
         <main>\n\
         <section id=\"viewer-panel\">\n\
           <div id=\"controls\">\n\
             <select id=\"clip-select\"></select>\n\
             <button id=\"play\">▶</button>\n\
             <input type=\"range\" id=\"scrub\" min=\"0\" value=\"0\" step=\"1\">\n\
             <span id=\"time\"></span>\n\
           </div>\n\
           <canvas id=\"gl\"></canvas>\n\
           <p class=\"hint\">drag to orbit · wheel to zoom · frames shown are exactly the \
           grid the checks judged</p>\n\
         </section>\n\
         <section id=\"side\">\n\
           <h2>Findings</h2>\n<ul id=\"findings\"></ul>\n\
           <h2>Coverage gaps</h2>\n<ul id=\"gaps\"></ul>\n\
           <h2>Engine predictions</h2>\n<ul id=\"predictions\"></ul>\n\
           <h2>Charts</h2>\n<div id=\"charts\">{charts_html}</div>\n\
         </section>\n\
         </main>\n\
         <script>{SHARED_JS}</script>\n\
         <script type=\"application/json\" id=\"report-data\">{data}</script>\n\
         <script>{VIEWER_JS}</script>\n</body>\n</html>\n"
    )
}

/// Render a collection inventory dashboard from its already-validated,
/// versioned machine authority.
///
/// The dashboard deliberately has no asset loader, policy evaluator, or
/// filesystem access. The CLI constructs and writes the authority first; this
/// presentation embeds exactly that authority as escaped JSON, so filtering
/// cannot change its meaning or turn an incomplete collection into a pass.
pub fn render_collection_dashboard(authority_json: &str) -> String {
    // A `</script>`-bearing untrusted string in the authority must not escape
    // the data element. Escaping `<` in JSON strings is lossless.
    let data = authority_json.replace('<', "\\u003c");
    format!(
        r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>animsmith collection dashboard</title><style>
body{{font:16px system-ui,sans-serif;margin:2rem;max-width:1200px;color:#17202a}}
header,section{{margin-bottom:1.5rem}} label{{margin-right:1rem}}
table{{border-collapse:collapse;width:100%}}th,td{{border:1px solid #ccd;padding:.4rem;text-align:left;vertical-align:top}}
.warning{{background:#fff4d6}} .muted{{color:#57606a}} code{{word-break:break-word}}
</style></head><body>
<header><h1>animsmith collection dashboard</h1><p id="identity"></p><p id="summary" class="muted"></p><p class="warning">This is an inventory of declared evidence, not a quality score or game-ready verdict. Engine loading/playback, retargeting, contacts, visual/artistic quality, and gameplay acceptance remain separate gates.</p></header>
<section><h2>Filters</h2><label>Source <select id="source"><option value="">all</option></select></label><label>Role <select id="role"><option value="">all</option></select></label><label>Runtime set <select id="set"><option value="">all</option></select></label><label>Severity <select id="severity"><option value="">all</option></select></label><label>Outcome <select id="outcome"><option value="">all</option></select></label><label>Availability <select id="availability"><option value="">all</option></select></label><label>Group <select id="group"><option value="source">source</option><option value="roles">role</option><option value="runtime_sets">runtime set</option><option value="severities">severity</option><option value="outcome">outcome</option><option value="availability">availability</option></select></label><p id="groups" class="muted"></p></section>
<section><h2>Declared sources</h2><p id="source-count" class="muted"></p><table><thead><tr><th>Source</th><th>Declared locator</th><th>Observed input identity</th><th>Availability</th><th>Loader</th><th>Dependency closure</th><th>Physical takes</th><th>Logical clips</th><th>Unscoped findings</th><th>Unscoped prediction unavailable</th></tr></thead><tbody id="sources"></tbody></table></section>
<section><h2>Observed physical takes</h2><p class="muted">Source-owned observed takes remain listed independently of logical clip declarations.</p><table><thead><tr><th>Source / source take</th><th>Observed take name</th><th>Normalized clip</th><th>Availability</th><th>Outcome</th><th>Findings / gaps / prediction unavailable</th></tr></thead><tbody id="takes"></tbody></table></section>
<section><h2>Logical clips</h2><p id="count" class="muted"></p><table><thead><tr><th>Logical clip</th><th>Physical take</th><th>Roles</th><th>Availability</th><th>Outcome</th><th>Findings / gaps / prediction unavailable</th><th>Runtime sets</th><th>Per-asset report</th></tr></thead><tbody id="clips"></tbody></table></section>
<section><h2>Runtime sets</h2><div id="sets"></div></section><section><h2>Evaluation authority</h2><pre id="evaluation"></pre></section>
<script type="application/json" id="collection-dashboard-data">{data}</script>
<script>
const d=JSON.parse(document.getElementById('collection-dashboard-data').textContent);
const q=id=>document.getElementById(id),esc=s=>String(s??'').replace(/[&<>\"]/g,c=>({{'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;'}}[c]));
q('identity').textContent=`collection input ${{d.collection_output.sha256}} (${{d.collection_output.bytes}} bytes)`;
q('summary').textContent=`${{d.summary.sources}} sources · ${{d.summary.physical_takes}} physical takes · ${{d.summary.clips}} clips · ${{d.summary.runtime_sets}} runtime sets · ${{d.summary.findings}} findings (${{d.summary.unscoped_findings}} unscoped) · ${{d.summary.coverage_gaps}} gaps · ${{d.summary.prediction_unavailable}} prediction unavailable (${{d.summary.unscoped_prediction_unavailable}} unscoped) · ${{d.summary.with_findings}} with findings · ${{d.summary.evaluated}} evaluated · ${{d.summary.partial}} partial · ${{d.summary.excluded}} excluded · ${{d.summary.unavailable}} unavailable · ${{d.summary.not_evaluated}} not evaluated`;
const clips=d.view.clips,sources=d.view.sources,takes=sources.flatMap(s=>s.takes.map(t=>({{...t,source:s.key}}))),sets=d.view.runtime_sets,facet=(item,key)=>{{const value=item[key];return Array.isArray(value)?(value.length?value:['none']):[value||'none']}},values=(items,key)=>[...new Set(items.flatMap(x=>facet(x,key)))].sort();
q('source-count').textContent=`${{sources.length}} declared sources; sources with zero logical clips remain listed`;
q('sources').innerHTML=sources.map(s=>{{const sourceClips=clips.filter(c=>c.source===s.key).length,input=s.input?`${{esc(s.input.sha256)}} (${{s.input.bytes}} bytes)`:'unavailable',severities=facet(s,'unscoped_severities').map(esc).join(', '),predictionReasons=facet(s,'unscoped_prediction_reasons').map(esc).join(', ');return `<tr><td><code>${{esc(s.key)}}</code></td><td><code>${{esc(s.locator)}}</code></td><td><code>${{input}}</code></td><td>${{esc(s.availability)}}</td><td>${{esc(s.loader)}}</td><td>${{esc(s.dependency_closure)}}</td><td>${{s.takes.length}}</td><td>${{sourceClips}}</td><td>${{s.unscoped_findings}} (${{severities}})</td><td>${{s.unscoped_prediction_unavailable}} (${{predictionReasons}})</td></tr>`}}).join('');
q('takes').innerHTML=takes.map(t=>`<tr><td><code>${{esc(t.source)}} #${{t.source_take_index}}</code></td><td><code>${{esc(t.take_name||'unavailable')}}</code></td><td><code>${{t.normalized_clip_index===undefined?'unavailable':`#${{t.normalized_clip_index}} ${{esc(t.normalized_clip_name)}}`}}</code></td><td>${{esc(t.availability)}}</td><td>${{esc(t.outcome)}}</td><td>${{t.findings}} (${{facet(t,'severities').map(esc).join(', ')}}) / ${{t.coverage_gaps}} / ${{t.prediction_unavailable}}<br><small>coverage ${{t.coverage.complete}} complete / ${{t.coverage.partial}} partial / ${{t.coverage.excluded}} excluded / ${{t.coverage.not_evaluated}} not evaluated</small></td></tr>`).join('');
for(const [id,key,items] of [['source','source',clips],['role','roles',clips],['set','runtime_sets',clips],['severity','severities',clips],['outcome','outcome',clips],['availability','availability',clips]]){{for(const x of values(items,key)){{const o=document.createElement('option');o.value=o.textContent=x;q(id).append(o)}}}}
function selected(id){{return q(id).value}}function matches(c){{return(!selected('source')||c.source===selected('source'))&&(!selected('role')||facet(c,'roles').includes(selected('role')))&&(!selected('set')||facet(c,'runtime_sets').includes(selected('set')))&&(!selected('severity')||facet(c,'severities').includes(selected('severity')))&&(!selected('outcome')||c.outcome===selected('outcome'))&&(!selected('availability')||c.availability===selected('availability'))}}
function draw(){{const shown=clips.filter(matches);q('count').textContent=`showing ${{shown.length}} of ${{clips.length}} declared clips; filters do not change collection completeness`;const group=q('group').value;const counts={{}};for(const c of shown){{for(const key of facet(c,group)){{counts[key]=(counts[key]||0)+1}}}}q('groups').textContent=Object.entries(counts).sort(([a],[b])=>a<b?-1:a>b?1:0).map(([key,count])=>`${{key}}: ${{count}}`).join(' · ')||'no matching declared clips';q('clips').innerHTML=shown.map(c=>`<tr><td><code>${{esc(c.id)}}</code></td><td><code>${{esc(c.source)}} #${{c.take_index}} ${{esc(c.take_name)}}</code></td><td>${{esc(facet(c,'roles').join(', '))}}</td><td>${{esc(c.availability)}}</td><td>${{esc(c.outcome)}}</td><td>${{c.findings}} / ${{c.coverage_gaps}} / ${{c.prediction_unavailable}}<br><small>coverage ${{c.coverage.complete}} complete / ${{c.coverage.partial}} partial / ${{c.coverage.excluded}} excluded / ${{c.coverage.not_evaluated}} not evaluated</small></td><td>${{facet(c,'runtime_sets').map(esc).join('<br>')}}</td><td>${{c.report_link?`<a href="${{esc(c.report_link)}}">open report</a>`:'—'}}</td></tr>`).join('')}}
for(const id of ['source','role','set','severity','outcome','availability','group'])q(id).onchange=draw;draw();
q('sets').innerHTML=sets.map(s=>`<article><h3><code>${{esc(s.id)}}</code> — ${{esc(s.lifecycle)}}</h3><ol>${{s.members.map(m=>`<li><code>${{esc(m)}}</code></li>`).join('')}}</ol><p>${{esc((s.gaps||[]).join(', ')||'no recorded set gaps')}}</p></article>`).join('');
q('evaluation').textContent=JSON.stringify(d.evaluation,null,2);
</script></body></html>"#,
    )
}

/// SVG metric charts for one clip: gait signal (L/R foot heights and
/// their difference) and the top-down root path. Rust-rendered; a JS
/// playhead line is moved across them in sync with the 3D view.
fn clip_charts(clip_name: &str, grid: &PoseGrid, roles: &ResolvedRoles) -> String {
    let mut out = String::new();
    let frames = grid.frame_count();
    let hips = roles.get(Role::Hips);
    let left = roles.get(Role::LeftFoot);
    let right = roles.get(Role::RightFoot);

    if let (Some(hips), Some(left), Some(right)) = (hips, left, right) {
        let rel_y = |f: usize, b: usize| {
            (grid.model_position(f, b).y - grid.model_position(f, hips).y) as f64
        };
        let l: Vec<f64> = (0..frames).map(|f| rel_y(f, left)).collect();
        let r: Vec<f64> = (0..frames).map(|f| rel_y(f, right)).collect();
        let d: Vec<f64> = l.iter().zip(&r).map(|(a, b)| a - b).collect();
        out.push_str(&line_chart(
            clip_name,
            "gait",
            "foot height rel hips (m) — L blue · R orange · L−R grey",
            &[("#7aa2f7", &l), ("#e0af68", &r), ("#9099b2", &d)],
        ));
    }

    let root = roles.get(Role::Root).or(hips);
    if let Some(root) = root {
        let xs: Vec<f64> = (0..frames)
            .map(|f| grid.model_position(f, root).x as f64)
            .collect();
        let zs: Vec<f64> = (0..frames)
            .map(|f| grid.model_position(f, root).z as f64)
            .collect();
        out.push_str(&path_chart(clip_name, "root path (top-down, m)", &xs, &zs));
    }
    out
}

const W: f64 = 360.0;
const H: f64 = 120.0;
const PAD: f64 = 8.0;

fn line_chart(clip: &str, kind: &str, label: &str, series: &[(&str, &Vec<f64>)]) -> String {
    let clip = &esc(clip);
    let all: Vec<f64> = series.iter().flat_map(|(_, v)| v.iter().copied()).collect();
    if all.is_empty() {
        return String::new();
    }
    let min = all.iter().copied().fold(f64::MAX, f64::min);
    let max = all.iter().copied().fold(f64::MIN, f64::max);
    let span = (max - min).max(1e-6);
    let n = series[0].1.len().max(2);
    let x = |i: usize| PAD + (W - 2.0 * PAD) * i as f64 / (n - 1) as f64;
    let y = |v: f64| H - PAD - (H - 2.0 * PAD) * (v - min) / span;
    let mut paths = String::new();
    for (color, values) in series {
        let d: Vec<String> = values
            .iter()
            .enumerate()
            .map(|(i, &v)| format!("{}{:.1},{:.1}", if i == 0 { "M" } else { "L" }, x(i), y(v)))
            .collect();
        paths.push_str(&format!(
            "<path d=\"{}\" fill=\"none\" stroke=\"{color}\" stroke-width=\"1.5\"/>",
            d.join("")
        ));
    }
    format!(
        "<figure class=\"chart\" data-clip=\"{clip}\" data-kind=\"{kind}\" data-pad=\"{PAD}\" \
         data-plotw=\"{}\"><figcaption>{clip} — {label}</figcaption>\
         <svg viewBox=\"0 0 {W} {H}\" width=\"100%\">{paths}\
         <line class=\"playhead\" x1=\"{PAD}\" x2=\"{PAD}\" y1=\"0\" y2=\"{H}\"/></svg></figure>",
        W - 2.0 * PAD
    )
}

fn path_chart(clip: &str, label: &str, xs: &[f64], zs: &[f64]) -> String {
    let clip = &esc(clip);
    if xs.is_empty() {
        return String::new();
    }
    let (min_x, max_x) = (
        xs.iter().copied().fold(f64::MAX, f64::min),
        xs.iter().copied().fold(f64::MIN, f64::max),
    );
    let (min_z, max_z) = (
        zs.iter().copied().fold(f64::MAX, f64::min),
        zs.iter().copied().fold(f64::MIN, f64::max),
    );
    let span = (max_x - min_x).max(max_z - min_z).max(1e-3);
    let x = |v: f64| PAD + (W - 2.0 * PAD) * (v - min_x) / span;
    let y = |v: f64| H - PAD - (H - 2.0 * PAD) * (v - min_z) / span;
    let d: Vec<String> = xs
        .iter()
        .zip(zs)
        .enumerate()
        .map(|(i, (&px, &pz))| {
            format!(
                "{}{:.1},{:.1}",
                if i == 0 { "M" } else { "L" },
                x(px),
                y(pz)
            )
        })
        .collect();
    format!(
        "<figure class=\"chart\" data-clip=\"{clip}\" data-kind=\"rootpath\">\
         <figcaption>{clip} — {label}</figcaption>\
         <svg viewBox=\"0 0 {W} {H}\" width=\"100%\">\
         <path d=\"{}\" fill=\"none\" stroke=\"#9ece6a\" stroke-width=\"1.5\"/>\
         <circle class=\"pathdot\" r=\"3\" cx=\"{:.1}\" cy=\"{:.1}\"/></svg>\
         <template class=\"pathpoints\">{}</template></figure>",
        d.join(""),
        x(xs[0]),
        y(zs[0]),
        xs.iter()
            .zip(zs)
            .map(|(&px, &pz)| format!("{:.1},{:.1}", x(px), y(pz)))
            .collect::<Vec<_>>()
            .join(";")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use animsmith_core::glam::Vec3;
    use animsmith_core::model::{
        Bone, Clip, Document, Interpolation, Property, Skeleton, SourceInfo, Track, TrackValues,
        Transform,
    };
    use animsmith_core::profile::Role;
    use animsmith_core::{CheckCtx, Config};

    fn report_document() -> Document {
        Document {
            skeleton: Skeleton {
                bones: vec![Bone {
                    name: "root".into(),
                    parent: None,
                    rest: Transform::IDENTITY,
                    inverse_bind: None,
                }],
            },
            clips: vec![Clip {
                name: "walk".into(),
                duration_s: 1.0,
                tracks: vec![Track {
                    bone: 0,
                    property: Property::Translation,
                    interpolation: Interpolation::Linear,
                    times: vec![0.0, 0.5, 1.0],
                    values: TrackValues::Vec3s(vec![
                        Vec3::ZERO,
                        Vec3::new(1.0, 0.0, 0.0),
                        Vec3::new(2.0, 0.0, 0.0),
                    ]),
                }],
            }],
            source: SourceInfo {
                path: Some("walk.glb".into()),
                format: Some("gltf".into()),
            },
            ..Document::default()
        }
    }

    fn report_data(html: &str) -> Value {
        let marker = r#"<script type="application/json" id="report-data">"#;
        let (_, tail) = html.split_once(marker).expect("report data marker");
        let (raw, _) = tail.split_once("</script>").expect("report data close");
        serde_json::from_str(raw).expect("report data is JSON")
    }

    #[test]
    fn collection_dashboard_escapes_authority_script_terminators() {
        let authority = r#"{"schema":"x","text":"</script><img src=x>"}"#;
        let html = render_collection_dashboard(authority);
        let marker = r#"<script type="application/json" id="collection-dashboard-data">"#;
        let (_, tail) = html.split_once(marker).expect("dashboard data marker");
        let (raw, _) = tail.split_once("</script>").expect("dashboard data close");
        assert!(!raw.contains("</script>"));
        assert!(raw.contains(r#"\u003c/script>"#));
        assert_eq!(
            serde_json::from_str::<Value>(raw).unwrap()["text"],
            "</script><img src=x>"
        );
    }

    #[test]
    fn shared_grid_render_embeds_clip_data() {
        let doc = report_document();
        let roles = ResolvedRoles::from_names(&doc.skeleton, [(Role::Root, "root".to_string())]);
        let config = Config::default();
        let checks = Vec::new();

        let fresh = render(&MetricGrids::new(&doc), &roles, &checks, None, None);
        let grids = MetricGrids::new(&doc);
        let ctx = CheckCtx::new(&grids, &roles, &config);
        assert!(ctx.grid(0).is_some());
        let shared = render(&grids, &roles, &checks, None, None);

        assert_eq!(fresh, shared);
        assert!(shared.contains(r#"data-kind="rootpath""#));

        let data = report_data(&shared);
        assert_eq!(data["file"], "walk.glb");
        assert_eq!(data["clips"][0]["name"], "walk");
        assert_eq!(data["clips"][0]["frames"], 3);
        assert_eq!(data["clips"][0]["trails"]["root"], 0);

        let positions = data["clips"][0]["positions"]
            .as_str()
            .expect("encoded positions");
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(positions)
            .expect("positions decode");
        assert_eq!(bytes.len(), 3 * 3 * std::mem::size_of::<f32>());
    }
}
