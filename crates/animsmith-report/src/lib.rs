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
//! doc: &animsmith_core::Document,
//! roles: &animsmith_core::ResolvedRoles,
//! checks: &[animsmith_core::CheckEvaluation],
//! config: &animsmith_core::Config,
//! ) -> std::io::Result<()> {
//! let grids = animsmith_core::MetricGrids::new(doc);
//! let inputs = animsmith_report::ReportInputs::new(&grids, roles, checks, config);
//! let html = animsmith_report::render(inputs);
//! std::fs::write("report.html", html)
//! }
//! ```
//!
//! # Sharing a report without the motion
//!
//! [`ReportOptions::evidence_only`] leaves the sampled pose grid out of both
//! report forms. The grid *is* the motion: it is the model-space joint
//! position of every bone on every judged frame, so a full report of a
//! licensed clip carries that clip. An evidence-only report keeps the
//! findings, coverage gaps and engine predictions — [`render_comparison`]
//! also keeping both sides' input identities, and [`render`] its profile and
//! source path when the document has one — and can therefore be attached to
//! an issue, published, or sent to a vendor where the source asset itself may
//! not go (see the
//! [licensed-asset policy]).
//!
//! The boundary is the pose grid, and it is worth stating exactly. [`render`]
//! draws its charts here, on the Rust side, so an evidence-only single-clip
//! report keeps them: they retain the root's X/Z path, the two foot-height
//! series relative to the hips plus their difference, and — for each declared
//! gait group the document holds a member of — one left-minus-right
//! foot-height series per member with its measured stride anchor, and nothing
//! else per bone. A [`ReportInputs::clip`] filter scopes the pose grid and the
//! per-clip charts to that clip; a group figure it belongs to still draws
//! every member, because a group compared against one of its own members is
//! not a comparison. [`render_comparison`]'s panels are viewer drawings made from the pose
//! grid, so an evidence-only comparison replaces every one of them — both
//! trajectory panels, both gait panels, and the shared root chart — with the
//! omission notice, and disables the controls that would drive them: the
//! shared-phase transport and the pose overlay. Findings, gaps, predictions,
//! identities, clip metadata, and contexts stay as the full comparison embeds
//! them. What neither form
//! carries is the per-frame grid a viewer could re-export as animation.
//!
//! [licensed-asset policy]: https://github.com/mmannerm/animsmith/blob/main/DEVELOPMENT.md#golden-tests
//!
//! # Deep links and embedding
//!
//! Both documents read their URL fragment as `&`-separated `key=value` pairs
//! and never write it back: neither viewer, nor the runtime they share,
//! contains an assignment to `location.hash`. `theme=light|dark` pins the
//! palette that otherwise follows `prefers-color-scheme`, `embed=1` hides the
//! running title and the interaction hint and nothing else, so the document
//! fits an `<iframe>` with its evidence in place, and `frame=N` scrubs. [`render`]'s
//! document also takes `clip=NAME` and `finding=INDEX`; [`render_comparison`]'s
//! addresses a finding through the `#finding-<side>-<anchor>` links its own
//! panels carry.
//!
//! A key that is absent leaves that state alone. A key that is present is
//! applied as far as the document allows: a syntactically valid frame beyond
//! the clip is clamped to its last frame, and a clip the document does not
//! contain selects the first one. A value the parser cannot read restores that
//! state's default instead — an unparsable frame restores frame 0, an
//! unparsable clip the first clip, and an unaddressable finding index clears
//! the selection. Unknown keys and malformed pairs are ignored, and no
//! fragment changes the findings, coverage gaps, predictions, or charts the
//! document carries.
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

use animsmith_core::metrics::{
    GaitMemberPhase, GaitPhaseEvidence, GaitPhaseOutcome, MetricGrids,
    circular_phase_center_spread, circular_phase_distance, gait_cycle_samples, gait_member_phase,
    gait_phase_evidence, metric_frame_count, wrap_unit_phase,
};
use animsmith_core::profile::{ResolvedRoles, Role};
use animsmith_core::sample::PoseGrid;
use animsmith_core::stance_support::{StanceSideV1, resolve_stance_support_v1};
use animsmith_core::{
    CheckEvaluation, Config, GaitGroup, LoadedSource, PredictionProvenanceV1,
    SourceObservationStateV1, SourceSetCoverageStateV1,
};
use base64::Engine as _;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};
use std::rc::Rc;

/// The colour authority both generated documents resolve through — every
/// colour literal either one carries belongs to this set: dark by default,
/// light under `prefers-color-scheme`, and either one pinned by a `#theme=`
/// fragment.
const TOKENS_CSS: &str = include_str!("../assets/tokens.css");
/// Surfaces both documents share: the page ground, evidence rows, the
/// omission notice, and the `#embed=1` chrome rules.
const BASE_CSS: &str = include_str!("../assets/report-base.css");
/// Pure helpers shared by both viewers. The runtime's fallback palette is a
/// placeholder here and is filled in from [`TOKENS_CSS`] at render time, so
/// the stylesheet stays the only place a token value is written.
const SHARED_JS: &str = include_str!("../assets/shared.js");
/// Replaced by the dark token object before the runtime is emitted.
const DARK_TOKEN_PLACEHOLDER: &str = "\"__ANIMSMITH_DARK_TOKENS__\"";
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

/// Presentation choices shared by [`render`] and [`render_comparison`].
///
/// Clip selection is an input rather than an option: the single-clip form
/// takes its `clip_filter` argument and each [`ComparisonSide`] declares its
/// own clip.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReportOptions {
    /// Omit the sampled pose grid from the embedded data and mark the report
    /// `evidence_only`.
    ///
    /// The grid is the motion — every bone's model-space position on every
    /// judged frame — so a full report of a licensed clip carries that clip.
    /// With this set the document renders a notice where each pose view would
    /// be and playback is disabled, while findings, coverage gaps and engine
    /// predictions are unchanged — as are the comparison's per-side input
    /// identities and the single-clip report's profile and source path, when
    /// it has one — so the evidence can be shared where the source asset cannot.
    ///
    /// [`render`]'s charts are drawn here and survive, retaining root X/Z and
    /// foot heights relative to the hips: the omission is the pose grid
    /// itself, not every number derived from it. [`render_comparison`]'s
    /// panels are viewer drawings from that grid, so they are replaced by the
    /// notice instead. A comparison rendered this way also stops being bounded
    /// by [`MAX_COMPARISON_POSE_BYTES`], which limits what a document embeds
    /// rather than what it may describe.
    pub evidence_only: bool,
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
    options: ReportOptions,
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
    // The pose budget bounds what the document will embed. An evidence-only
    // report embeds no poses, so the pair it can describe is not limited by
    // a grid it will never carry; every other bound still applies.
    if !options.evidence_only {
        comparison_pose_bytes(before_frames, before.skeleton.bones.len(), "before")?;
        comparison_pose_bytes(after_frames, after.skeleton.bones.len(), "after")?;
    }
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
    options: ReportOptions,
) -> Result<ComparisonPreflight, ComparisonError> {
    let preflight = preflight_comparison(
        before.document(),
        before_clip_name,
        after.document(),
        after_clip_name,
        options,
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
///
/// The two judged poses are drawn side by side, and one toggle in the shared
/// phase bar draws them in a single pane instead: the before skeleton solid,
/// the after one dashed over it, at the same two selected frames and through
/// the same shared camera.  The two-pane layout is the default, and the
/// overlay stands down while a loop-seam context — one side's own two
/// endpoint frames — is selected.
pub fn render_comparison(
    before: ComparisonSide<'_>,
    after: ComparisonSide<'_>,
    options: ReportOptions,
) -> Result<String, ComparisonError> {
    let before_doc = before.grids.document();
    let after_doc = after.grids.document();
    validate_side_authority(before, "before")?;
    validate_side_authority(after, "after")?;
    let preflight = preflight_comparison_sources(
        before.source,
        before.clip,
        after.source,
        after.clip,
        options,
    )?;
    let before_report = preflight_side_report_work(before, before.clip, "before")?;
    let after_report = preflight_side_report_work(after, after.clip, "after")?;
    preflight_report_allocation(
        before_doc,
        after_doc,
        preflight,
        before_report,
        after_report,
        options,
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
        options,
    )?;
    let after_side = comparison_side_json(
        after,
        after_clip.name.as_str(),
        after_clip.duration_s,
        after_grid.as_ref(),
        "after",
        options,
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
        "evidence_only": options.evidence_only,
        "before": before_side,
        "after": after_side,
    });
    let data = bounded_json(&data)?;
    // A `</script>`-bearing string inside data cannot terminate this element.
    let data = data.replace('<', "\\u003c");
    let before_clip_anchor = semantic_anchor("clip", before.clip);
    let after_clip_anchor = semantic_anchor("clip", after.clip);
    let before_pose = pose_surface("before-gl", options.evidence_only);
    let after_pose = pose_surface("after-gl", options.evidence_only);
    let shared_pose = pose_panel(
        "comparison-root-path",
        COMPARISON_ROOT_VIEW_BOX,
        options.evidence_only,
    );
    let before_trails = pose_panel(
        "before-path",
        COMPARISON_SIDE_VIEW_BOX,
        options.evidence_only,
    );
    let after_trails = pose_panel(
        "after-path",
        COMPARISON_SIDE_VIEW_BOX,
        options.evidence_only,
    );
    let before_gait = pose_panel(
        "before-gait",
        COMPARISON_SIDE_VIEW_BOX,
        options.evidence_only,
    );
    let after_gait = pose_panel(
        "after-gait",
        COMPARISON_SIDE_VIEW_BOX,
        options.evidence_only,
    );
    let shared_js = shared_runtime();
    // Every comparison panel is drawn from the pose grid, so an
    // evidence-only document has no shared phase left to scrub, play, or
    // overlay: all three controls are disabled in the markup itself rather
    // than left to do nothing when they are used.
    let disabled_without_poses = if options.evidence_only {
        " disabled"
    } else {
        ""
    };
    Ok(format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>animsmith — visual comparison</title><style>{TOKENS_CSS}{BASE_CSS}{COMPARISON_CSS}</style></head>\n\
         <body><header><h1>animsmith visual comparison</h1></header>\n\
         <section class=\"disclosure\"><p id=\"mapping\"></p>\n\
         <p class=\"warning\">This comparison presents checked evidence only. An absent finding is not artistic, gameplay, or engine acceptance.</p></section>\n\
         <section class=\"sync\"><button id=\"play\" aria-label=\"Play the shared phase\"{disabled_without_poses}>▶</button><label>Shared phase <input id=\"scrub\" type=\"range\" min=\"0\" max=\"1000\" value=\"0\"{disabled_without_poses}></label><label><input type=\"checkbox\" id=\"overlay\"{disabled_without_poses}> Overlay after on before</label><span id=\"times\"></span></section>\n\
         <main><section class=\"side\" id=\"before-panel\"><span id=\"before-{before_clip_anchor}\"></span><h2 id=\"clip-before\">Before</h2><p id=\"before-identity\"></p><h3>Judged pose at the shared phase</h3>{before_pose}<p id=\"before-pose-context\" class=\"context-label\"></p></section>\n\
         <section class=\"side\" id=\"after-panel\"><span id=\"after-{after_clip_anchor}\"></span><h2 id=\"clip-after\">After</h2><p id=\"after-identity\"></p><h3>Judged pose at the shared phase</h3>{after_pose}<p id=\"after-pose-context\" class=\"context-label\"></p></section>\n\
         <section class=\"side shared-chart\" id=\"root-panel\"><h2 id=\"root-panel-title\">Root path, before over after</h2>{shared_pose}</section>\n\
         <section class=\"side\" id=\"before-evidence\"><h2>Before evidence</h2><h3>Role trajectories</h3>{before_trails}<h3>Gait and sampled stance</h3>{before_gait}<h3>Acceptance context</h3><ul id=\"before-contexts\"></ul><h3>Findings</h3><ul id=\"before-findings\"></ul><h3>Coverage gaps</h3><ul id=\"before-gaps\"></ul><h3>Prediction provenance</h3><pre id=\"before-predictions\"></pre></section>\n\
         <section class=\"side\" id=\"after-evidence\"><h2>After evidence</h2><h3>Role trajectories</h3>{after_trails}<h3>Gait and sampled stance</h3>{after_gait}<h3>Acceptance context</h3><ul id=\"after-contexts\"></ul><h3>Findings</h3><ul id=\"after-findings\"></ul><h3>Coverage gaps</h3><ul id=\"after-gaps\"></ul><h3>Prediction provenance</h3><pre id=\"after-predictions\"></pre></section></main>\n\
         <script>{shared_js}</script><script type=\"application/json\" id=\"comparison-report-data\">{data}</script><script>{COMPARISON_VIEWER_JS}</script></body></html>\n"
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
    options: ReportOptions,
) -> Result<(), ComparisonError> {
    // The pose budget and the base64 it would occupy in the JSON both bound
    // an embedded grid. An evidence-only document embeds none, so neither
    // applies to it; every other allowance below still does.
    let base64_bytes = if options.evidence_only {
        0
    } else {
        let before_pose = comparison_pose_bytes(
            preflight.before_frames,
            before.skeleton.bones.len(),
            "before",
        )?;
        let after_pose =
            comparison_pose_bytes(preflight.after_frames, after.skeleton.bones.len(), "after")?;
        [before_pose, after_pose]
            .into_iter()
            .try_fold(0u128, |total, bytes| {
                let encoded = bytes.checked_add(2)?.checked_div(3)?.checked_mul(4)?;
                total.checked_add(encoded)
            })
            .unwrap_or(u128::MAX)
    };
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
    options: ReportOptions,
) -> Result<Value, ComparisonError> {
    let frames = grid.frame_count();
    let bones = side.grids.document().skeleton.bones.len();
    let mut positions = Vec::new();
    if !options.evidence_only {
        let bytes = comparison_pose_bytes(frames, bones, side_name)?;
        positions.reserve_exact(bytes as usize);
        for frame in 0..frames {
            for bone in 0..bones {
                let point = grid.model_position(frame, bone);
                positions.extend_from_slice(&point.x.to_le_bytes());
                positions.extend_from_slice(&point.y.to_le_bytes());
                positions.extend_from_slice(&point.z.to_le_bytes());
            }
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
    let mut clip = json!({"anchor":semantic_anchor("clip", clip_name),"name":clip_name,"duration":duration_s,"frames":frames,"times":grid.times,"trails":trails});
    if let Some(encoded) = encoded_positions(options, || positions) {
        clip["positions"] = json!(encoded);
    }
    let primary = side.source.dependency_closure().primary_input();
    let closure_identity = side
        .source
        .dependency_closure()
        .identity()
        .expect("comparison authority preflight requires a complete closure");
    // The words come from the Rust side for both documents, so a caption
    // cannot say one thing in a single-clip report and another in a
    // comparison of the same clip. The viewer supplies only the measured
    // numbers around them.
    let contract = clip_contract(side.checks, clip_name);
    Ok(json!({
        "identity": {"sha256": primary.sha256(), "bytes": primary.bytes()},
        "dependency_closure_identity": closure_identity,
        "guidance": {
            "root_path": contract.root_path_guidance(SHARED_PATH_MARKS),
            "gait": contract.gait_guidance(true),
        },
        "clip": clip,
        "contexts": comparison_contexts(side, clip_name, grid, &anchored_findings),
        "findings":findings,"gaps":gaps,"prediction_provenance":side.prediction_provenance,"predictions":predictions,
    }))
}

/// The sampled pose grid as base64, or nothing at all for an evidence-only
/// report. The key is then absent rather than empty, so a consumer cannot
/// mistake an omitted grid for a zero-length take.
fn encoded_positions(
    options: ReportOptions,
    positions: impl FnOnce() -> Vec<u8>,
) -> Option<String> {
    (!options.evidence_only).then(|| base64::engine::general_purpose::STANDARD.encode(positions()))
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

/// The shared runtime with its fallback palette resolved from the
/// stylesheet's own dark values.
fn shared_runtime() -> String {
    SHARED_JS.replace(DARK_TOKEN_PLACEHOLDER, &dark_token_object())
}

/// The `--name: value` declarations of the bare `:root` block of
/// [`TOKENS_CSS`], as a JS object. Parsing our own asset keeps one authority
/// for the palette; a malformed block yields an empty object rather than a
/// panic, and the emitted document is asserted against the stylesheet.
fn dark_token_object() -> String {
    let mut tokens = serde_json::Map::new();
    if let Some(block) = TOKENS_CSS
        .split_once(":root {")
        .and_then(|(_, rest)| rest.split_once('}'))
        .map(|(block, _)| block)
    {
        for declaration in block.split(';') {
            let Some((name, value)) = declaration.split_once(':') else {
                continue;
            };
            let (name, value) = (name.trim(), value.trim());
            if let Some(token) = name.strip_prefix("--") {
                tokens.insert(token.to_owned(), Value::String(value.to_owned()));
            }
        }
    }
    Value::Object(tokens).to_string()
}

/// The shared root panel's drawing area, in the user units its viewer maps
/// into. It is exactly the region drawn: the panel used to reserve a strip
/// below for a caption, which is now the HTML paragraph beside it, so the
/// strip was empty space in the largest panel of the document.
///
/// `assets/comparison.js` maps into these boxes, and
/// `tests/render.rs::the_comparison_viewer_maps_into_the_panels_this_document_emits`
/// holds the two copies of the numbers together.
const COMPARISON_ROOT_VIEW_BOX: &str = "0 0 720 180";
/// The per-side panels' drawing area, for both the trajectory and gait panels.
const COMPARISON_SIDE_VIEW_BOX: &str = "0 0 360 180";

/// Shown where a pose view would be when the sampled grid was deliberately
/// left out of the document.
const POSE_OMITTED_NOTICE: &str = "Pose playback omitted: evidence-only report";

/// The pose view for one panel: a canvas, or the notice that replaces it in
/// an evidence-only report. Emitting the difference here rather than in the
/// viewer keeps it in the document itself, with no canvas to flash first.
fn pose_surface(id: &str, evidence_only: bool) -> String {
    if evidence_only {
        format!("<p class=\"notice\" id=\"{id}-notice\">{POSE_OMITTED_NOTICE}</p>")
    } else {
        format!("<canvas id=\"{id}\"></canvas>")
    }
}

/// A comparison panel drawn from the sampled poses: its `<svg>`, or the same
/// notice when the document carries no grid to draw from. Unlike the
/// single-clip charts, which the Rust side renders once and an evidence-only
/// report keeps, these panels exist only as viewer drawings.
fn pose_panel(id: &str, view_box: &str, evidence_only: bool) -> String {
    if evidence_only {
        pose_surface(id, true)
    } else {
        // The panel's caption is an HTML paragraph beside the drawing, not
        // text inside it: SVG never wraps, so a caption drawn into the
        // picture is cut at the panel edge on a narrow column, while the
        // browser reflows this one for free at whatever width the reader
        // has. The viewer fills it in as it draws.
        format!(
            "<svg id=\"{id}\" viewBox=\"{view_box}\"></svg>\
             <p id=\"{id}-caption\" class=\"context-label\"></p>"
        )
    }
}

/// Everything [`render`] reads, in one value.
///
/// The single-clip report grew a configuration and a clip selection beside
/// its grids, roles, checks and provenance, and seven positional arguments
/// is a call nobody can read. This mirrors [`ComparisonSide`], which the
/// comparison has always taken for the same reason.
#[derive(Debug, Clone, Copy)]
pub struct ReportInputs<'a> {
    /// Metric pose grids computed from the loaded document.
    pub grids: &'a MetricGrids<'a>,
    /// Resolved rig roles.
    pub roles: &'a ResolvedRoles,
    /// Typed check evaluations for this document.
    pub checks: &'a [CheckEvaluation],
    /// Exact configuration the checks were evaluated under.
    ///
    /// A clean gait group emits no finding carrying its membership or its
    /// cap, so the declarations themselves are what lets the report draw a
    /// group's members against each other; the report reads nothing else
    /// out of it.
    pub config: &'a Config,
    /// Optional engine-prediction provenance.
    pub prediction_provenance: Option<&'a PredictionProvenanceV1>,
    /// Restricts the report to one clip name when present. The gait-group
    /// figures a selected clip belongs to are still drawn from every member,
    /// because the group is what that clip is judged against.
    pub clip: Option<&'a str>,
    /// Presentation choices — see [`ReportOptions::evidence_only`] for a
    /// report that omits the sampled motion.
    pub options: ReportOptions,
}

impl<'a> ReportInputs<'a> {
    /// The inputs a report of every clip in `grids` needs, with no
    /// configuration, provenance or clip selection.
    pub fn new(
        grids: &'a MetricGrids<'a>,
        roles: &'a ResolvedRoles,
        checks: &'a [CheckEvaluation],
        config: &'a Config,
    ) -> Self {
        Self {
            grids,
            roles,
            checks,
            config,
            prediction_provenance: None,
            clip: None,
            options: ReportOptions {
                evidence_only: false,
            },
        }
    }
}

/// Render report HTML from shared metric pose grids.
///
/// The function performs no filesystem I/O and cannot report write errors;
/// callers choose where to store or serve the returned self-contained HTML
/// string.
pub fn render(inputs: ReportInputs<'_>) -> String {
    let ReportInputs {
        grids,
        roles,
        checks,
        config,
        prediction_provenance,
        clip: clip_filter,
        options,
    } = inputs;
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
    // The cross-clip figures come first: a group figure is evidence about
    // its members rather than about one clip, so it opens the Charts column
    // rather than sitting under whichever clip happens to be shown.
    let (mut charts_html, group_members) = gait_group_charts(grids, roles, config, clip_filter);
    for (clip_index, clip) in doc.clips.iter().enumerate() {
        if clip_filter.is_some_and(|f| f != clip.name) {
            continue;
        }
        let Some(grid) = grids.grid(clip_index) else {
            continue;
        };
        let frames = grid.frame_count();
        let nb = doc.skeleton.bones.len();
        let sampled_positions = || {
            let mut positions = Vec::with_capacity(frames * nb * 3 * 4);
            for f in 0..frames {
                for b in 0..nb {
                    let p = grid.model_position(f, b);
                    positions.extend_from_slice(&p.x.to_le_bytes());
                    positions.extend_from_slice(&p.y.to_le_bytes());
                    positions.extend_from_slice(&p.z.to_le_bytes());
                }
            }
            positions
        };
        let trails: Value = trail_roles
            .iter()
            .filter_map(|&(role, name)| roles.get(role).map(|id| (name.to_string(), json!(id))))
            .collect::<serde_json::Map<_, _>>()
            .into();
        let mut clip_json = json!({
            "name": clip.name,
            "duration": clip.duration_s,
            "frames": frames,
            // Samples one stride cycle spans, which is the axis a gait-group
            // figure draws its members on. Equal to `frames - 1` for every
            // grid that repeats its first sample at the wrap, and to
            // `frames` for a three-frame grid that has no duplicate.
            "cycle": gait_cycle_samples(frames),
            "trails": trails,
        });
        if let Some(encoded) = encoded_positions(options, sampled_positions) {
            clip_json["positions"] = json!(encoded);
        }
        clips_json.push(clip_json);
        charts_html.push_str(&clip_charts(
            &clip.name,
            grid.as_ref(),
            roles,
            &clip_contract(checks, &clip.name),
        ));
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
        "evidence_only": options.evidence_only,
        "bones": bones,
        // The declared membership of every group this document draws, joined
        // to its figure by `data-group`. The viewer shows a group figure only
        // while one of these clips is selected.
        "groups": group_members,
        "clips": clips_json,
        "findings": findings_json,
        "gaps": gaps_json,
        "prediction_provenance": prediction_provenance,
        "predictions": predictions_json,
    });

    let pose = pose_surface("gl", options.evidence_only);
    let shared_js = shared_runtime();
    // The scrub still moves the chart playhead without a pose grid, so only
    // playback itself is disabled.
    let play_state = if options.evidence_only {
        " disabled"
    } else {
        ""
    };
    let hint = if options.evidence_only {
        "sampled poses were omitted · findings, coverage, and charts are the evidence \
         this report carries"
    } else {
        "drag to orbit · wheel to zoom · frames shown are exactly the grid the checks judged"
    };
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
         <title>animsmith — {title}</title>\n<style>{TOKENS_CSS}{BASE_CSS}{VIEWER_CSS}</style>\n</head>\n<body>\n\
         <header><h1>animsmith report</h1><span id=\"file\"></span></header>\n\
         <main>\n\
         <section id=\"viewer-panel\">\n\
           <div id=\"controls\">\n\
             <select id=\"clip-select\"></select>\n\
             <button id=\"play\" aria-label=\"Play the clip\"{play_state}>▶</button>\n\
             <input type=\"range\" id=\"scrub\" min=\"0\" value=\"0\" step=\"1\">\n\
             <span id=\"time\"></span>\n\
           </div>\n\
           {pose}\n\
           <p class=\"hint\">{hint}</p>\n\
         </section>\n\
         <section id=\"side\">\n\
           <h2>Findings</h2>\n<ul id=\"findings\"></ul>\n\
           <h2>Coverage gaps</h2>\n<ul id=\"gaps\"></ul>\n\
           <h2>Engine predictions</h2>\n<ul id=\"predictions\"></ul>\n\
           <h2>Charts</h2>\n<div id=\"charts\">{charts_html}</div>\n\
         </section>\n\
         </main>\n\
         <script>{shared_js}</script>\n\
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

/// SVG metric charts for one clip: the gait signal (L/R foot heights and
/// their difference) and the top-down root path.
///
/// The charts are Rust-rendered and legible on their own: each carries a
/// `<title>`, a legend, axis labels with units, `role="img"`, and an
/// `aria-label`, and takes its paint from stable series classes rather than
/// per-element attributes, so a `<figure>` lifted out of the report keeps its
/// meaning under an injected copy of the report tokens. The `data-*` hooks and
/// the `.playhead`/`.pathdot` elements the viewer syncs are part of that
/// contract too.
fn clip_charts(
    clip_name: &str,
    grid: &PoseGrid,
    roles: &ResolvedRoles,
    contract: &ClipContract,
) -> String {
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
            "foot height relative to hips",
            &contract.gait_guidance(false),
            &[
                Series {
                    class: "series-left",
                    label: "L foot",
                    axis: Side::Left,
                    values: &l,
                },
                Series {
                    class: "series-right",
                    label: "R foot",
                    axis: Side::Left,
                    values: &r,
                },
                Series {
                    class: "series-diff",
                    label: "L−R",
                    axis: Side::Right,
                    values: &d,
                },
            ],
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
        out.push_str(&path_chart(
            clip_name,
            "root path (top-down)",
            &xs,
            &zs,
            &contract.root_path_guidance(SINGLE_PATH_MARKS),
        ));
    }
    out
}

/// Colour classes a group figure's members cycle through, in declared order.
///
/// Six is the whole set of series colours the design tokens define, so it is
/// also how many members one figure can tell apart — and therefore the bound
/// on the sampled series it allocates. The caption still names every declared
/// member, drawn or not.
const GROUP_SERIES_CLASSES: [&str; 6] = [
    "series-member-0",
    "series-member-1",
    "series-member-2",
    "series-member-3",
    "series-member-4",
    "series-member-5",
];
/// The stride-anchor mark belonging to each member's series, in the same
/// colour, so a mark is read against its own curve.
const GROUP_ANCHOR_CLASSES: [&str; 6] = [
    "anchor-member-0",
    "anchor-member-1",
    "anchor-member-2",
    "anchor-member-3",
    "anchor-member-4",
    "anchor-member-5",
];
/// Height of a stride-anchor mark, measured up from the plot floor.
const ANCHOR_MARK_H: f64 = 14.0;
/// `data-kind` of a gait-group figure, which is also the id of the check the
/// figure is evidence for.
const GAIT_GROUP_KIND: &str = "gait-group";
/// Title of a gait-group figure.
const GAIT_GROUP_TITLE: &str = "L−R foot height by stride phase";

/// One declared member of a gait group, as this document has it.
struct GroupMember<'a> {
    name: &'a str,
    phase: GaitMemberPhase,
    /// The member's left-minus-right foot height, sampled on its own metric
    /// grid, and the samples one cycle of it spans. Empty for a member this
    /// figure does not draw.
    series: Vec<f64>,
    cycle_samples: usize,
}

/// What the caption puts after a member's `name=`.
fn member_caption(phase: GaitMemberPhase) -> String {
    match phase {
        GaitMemberPhase::Absent => "not in file".to_owned(),
        GaitMemberPhase::RolesUnresolved => "roles unresolved".to_owned(),
        GaitMemberPhase::NoFootCycle => "no sampled foot cycle".to_owned(),
        GaitMemberPhase::NoAnchor(GaitPhaseOutcome::MissingBilateralFootRoles) => {
            "no bilateral foot roles".to_owned()
        }
        GaitMemberPhase::NoAnchor(GaitPhaseOutcome::NoFootHeightSwing) => {
            "no left/right swing".to_owned()
        }
        GaitMemberPhase::NoAnchor(_) => "phase not measurable".to_owned(),
        GaitMemberPhase::BelowFloor { floor_m, .. } => {
            format!("below the {floor_m:.3} m amplitude floor")
        }
        GaitMemberPhase::Measured(phase) => format!("{phase:.2}"),
        // The classification is `#[non_exhaustive]`: a state this figure has
        // not been taught to name is still named as unmeasured rather than
        // presented as an anchor.
        _ => "phase not measurable".to_owned(),
    }
}

/// One figure per declared gait group this document holds a member of.
///
/// Under a clip filter only the groups containing that clip are drawn, since
/// those are the ones the selected clip is judged against; every member of
/// such a group is still drawn, because a group compared against one of its
/// own members is not a comparison.
///
/// The number of figures follows the configuration, which the run's author
/// wrote. What the file supplies — the clips, their frame counts, their
/// samples — reaches a figure only through the six members it draws.
fn gait_group_charts(
    grids: &MetricGrids<'_>,
    roles: &ResolvedRoles,
    config: &Config,
    clip_filter: Option<&str>,
) -> (String, Vec<Value>) {
    let doc = grids.document();
    let drawable = |group: &GaitGroup| {
        group.clips.iter().any(|name| {
            clip_filter.is_none_or(|selected| selected == name)
                && doc.clips.iter().any(|clip| &clip.name == name)
        })
    };
    // Groups overlap, so one clip can be a member of several. Its evidence
    // is a property of the clip, not of the group reading it, and is
    // measured once here.
    let mut evidence: BTreeMap<usize, Option<Rc<GaitPhaseEvidence>>> = BTreeMap::new();
    let mut html = String::new();
    let mut membership = Vec::new();
    for (name, group) in config
        .gait_groups
        .iter()
        .filter(|(_, group)| drawable(group))
    {
        html.push_str(&gait_group_chart(name, group, grids, roles, &mut evidence));
        membership.push(json!({ "name": name, "members": group.clips }));
    }
    (html, membership)
}

/// One declared gait group's members on a single figure.
///
/// Every member is drawn against one unshifted normalized source-phase axis:
/// sample `k` of an `n`-sample cycle sits at `k / n`, which is where the
/// stride anchor measured on that same cycle is marked. Shifting each member
/// to its own anchor would draw one curve several times over and erase the
/// disagreement the figure exists to show, so the anchors are marked where
/// they were measured instead, and the group's cap is drawn as a band around
/// their circular mean.
///
/// No member is named as the offender against an invented reference. The
/// check judges a set's circular spread and names no canonical member, so
/// this figure states the same fact its band draws: which measured anchors
/// lie outside the cap either side of the mean.
fn gait_group_chart<'a>(
    group_name: &str,
    group: &'a GaitGroup,
    grids: &MetricGrids<'_>,
    roles: &ResolvedRoles,
    evidence: &mut BTreeMap<usize, Option<Rc<GaitPhaseEvidence>>>,
) -> String {
    let doc = grids.document();
    let mut members: Vec<GroupMember<'a>> = Vec::with_capacity(group.clips.len());
    for clip_name in &group.clips {
        // The member bound is applied here, before a sampled series is
        // cloned for it: a member's state is a handful of numbers, its
        // series is one per judged frame.
        let has_colour = members.len() < GROUP_SERIES_CLASSES.len();
        let index = doc.clips.iter().position(|clip| &clip.name == clip_name);
        let measured = index.and_then(|index| {
            evidence
                .entry(index)
                .or_insert_with(|| {
                    grids
                        .grid(index)
                        .and_then(|grid| gait_phase_evidence(&grid, roles))
                        .map(Rc::new)
                })
                .clone()
        });
        // One classifier serves this figure and the `gait-group` check, so
        // the caption cannot present a member as measured where the check
        // recorded a coverage gap.
        let phase = gait_member_phase(
            roles,
            index.is_some(),
            measured.as_deref(),
            group.min_lr_amplitude_m,
        );
        let (series, cycle_samples) = match measured.filter(|_| has_colour) {
            Some(measured) => (measured.lr_foot_height_m.clone(), measured.cycle_samples),
            None => (Vec::new(), 0),
        };
        members.push(GroupMember {
            name: clip_name,
            phase,
            series,
            cycle_samples,
        });
    }

    // The spread and the centre come from every measured member — exactly
    // the set `gait-group` measures — and from one pass over them.
    let anchors: Vec<f64> = members
        .iter()
        .filter_map(|member| member.phase.anchor())
        .collect();
    let band = (anchors.len() >= 2)
        .then(|| circular_phase_center_spread(&anchors))
        .flatten();
    let guidance = gait_group_guidance(&members, group, band);

    let drawn: Vec<(usize, &GroupMember<'_>)> = members
        .iter()
        .take(GROUP_SERIES_CLASSES.len())
        .enumerate()
        .filter(|(_, member)| !member.series.is_empty())
        .collect();
    let series: Vec<Series<'_>> = drawn
        .iter()
        .map(|(index, member)| Series {
            class: GROUP_SERIES_CLASSES[*index],
            label: member.name,
            axis: Side::Left,
            values: &member.series,
        })
        .collect();
    let legend: Vec<(Swatch, &'static str, String)> = series
        .iter()
        .map(|entry| (Swatch::Line, entry.class, entry.label.to_owned()))
        .collect();
    let subject = Subject::Group(group_name);

    let Some(range) = axis_range(&series, Side::Left) else {
        // Every member is absent, unsampled, or non-finite throughout. The
        // caption still names each one and its state, the way the root path
        // reports a trajectory it cannot plot.
        return Chart {
            subject,
            kind: GAIT_GROUP_KIND,
            title: GAIT_GROUP_TITLE,
            description: format!(
                "{GAIT_GROUP_TITLE}: unavailable — no member of this group has a finite \
                 sampled foot height; findings and coverage remain listed"
            ),
            guidance,
            legend,
            axis: vec![AxisLabel {
                x: W / 2.0,
                y: PAD_TOP + PLOT_H / 2.0,
                anchor: Anchor::Middle,
                text: "no member has a plottable foot-height sample".to_owned(),
            }],
            plot_hooks: false,
            body: String::new(),
            trailer: String::new(),
        }
        .render();
    };

    let plot = Plot::for_legend(legend.len());
    let (min, max) = range;
    let x = |phase: f64| PAD_LEFT + PLOT_W * phase;
    let y = |value: f64| {
        if is_flat(range) {
            plot.top + PLOT_H / 2.0
        } else {
            plot.bottom - PLOT_H * (value - min) / (max - min)
        }
    };

    let mut body = String::new();
    // The tolerance goes down first, so every curve and every mark reads on
    // top of it rather than under it.
    if let Some((centre, _)) = band {
        body.push_str(&phase_band(centre, group.max_gait_phase_spread, plot));
    }
    for (entry, (_, member)) in series.iter().zip(&drawn) {
        let d = polyline(entry.values.len(), |sample| {
            let value = entry.values[sample];
            value
                .is_finite()
                .then(|| (x(source_phase(sample, member.cycle_samples)), y(value)))
        });
        if d.is_empty() {
            continue;
        }
        body.push_str(&format!(
            "<path class=\"{}\" d=\"{d}\" fill=\"none\"/>",
            entry.class
        ));
    }
    body.push_str(&format!(
        "<line class=\"playhead\" x1=\"{PAD_LEFT}\" x2=\"{PAD_LEFT}\" y1=\"{:.1}\" y2=\"{:.1}\"/>",
        plot.top, plot.bottom
    ));
    // The anchors last: a mark a curve crossed would be the one thing in the
    // picture a reader could not check the caption against.
    for (index, member) in &drawn {
        let Some(phase) = member.phase.anchor() else {
            continue;
        };
        let at = x(phase);
        body.push_str(&format!(
            "<line class=\"{}\" x1=\"{at:.1}\" x2=\"{at:.1}\" y1=\"{:.1}\" y2=\"{:.1}\"/>",
            GROUP_ANCHOR_CLASSES[*index],
            plot.bottom,
            plot.bottom - ANCHOR_MARK_H
        ));
    }

    let mut axis = Vec::with_capacity(4);
    if is_flat(range) {
        axis.push(AxisLabel {
            x: 2.0,
            y: plot.top + PLOT_H / 2.0 + 3.0,
            anchor: Anchor::Start,
            text: format!("flat {min:.2} {UNIT}"),
        });
    } else {
        axis.push(AxisLabel {
            x: 2.0,
            y: plot.top + 3.0,
            anchor: Anchor::Start,
            text: format!("{max:.2} {UNIT}"),
        });
        axis.push(AxisLabel {
            x: 2.0,
            y: plot.bottom,
            anchor: Anchor::Start,
            text: format!("{min:.2} {UNIT}"),
        });
    }
    axis.push(AxisLabel {
        x: PAD_LEFT,
        y: plot.height - 4.0,
        anchor: Anchor::Start,
        text: "phase 0".to_owned(),
    });
    axis.push(AxisLabel {
        x: W - PAD_RIGHT,
        y: plot.height - 4.0,
        anchor: Anchor::End,
        text: "phase 1".to_owned(),
    });

    let names = series
        .iter()
        .map(|entry| entry.label)
        .collect::<Vec<_>>()
        .join(", ");
    let described = if is_flat(range) {
        format!("{names} flat at {min:.2} {UNIT}")
    } else {
        format!("{names} {min:.2} to {max:.2} {UNIT}")
    };
    Chart {
        subject,
        kind: GAIT_GROUP_KIND,
        title: GAIT_GROUP_TITLE,
        description: format!(
            "{GAIT_GROUP_TITLE} over one normalized stride cycle, phase 0 to 1: {described}"
        ),
        guidance,
        legend,
        axis,
        plot_hooks: true,
        body,
        trailer: String::new(),
    }
    .render()
}

/// Where a member's sample sits on the shared source-phase axis: sample `k`
/// of a `cycle`-sample stride cycle at `k / cycle`, which is exactly where a
/// phase measured on that cycle is marked. Members sampled at different
/// frame counts are therefore read against each other without any of them
/// moving, and a member whose grid repeats its first sample at the wrap
/// closes on the right-hand edge.
fn source_phase(sample: usize, cycle_samples: usize) -> f64 {
    sample as f64 / cycle_samples.max(1) as f64
}

/// The tolerance band: the group's cap either side of the circular mean of
/// its measured anchors, as one rectangle or two where it crosses the 0/1
/// seam of the cycle.
fn phase_band(centre: f64, cap: f64, plot: Plot) -> String {
    let rect = |from: f64, to: f64| {
        format!(
            "<rect class=\"phase-band\" x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" \
             height=\"{PLOT_H}\"/>",
            PAD_LEFT + PLOT_W * from,
            plot.top,
            PLOT_W * (to - from)
        )
    };
    // Half a cycle either side is the whole ring: every phase is inside. The
    // configuration boundary refuses a cap outside `[0, 0.5]`.
    if cap >= 0.5 {
        return rect(0.0, 1.0);
    }
    let low = wrap_unit_phase(centre - cap);
    let high = wrap_unit_phase(centre + cap);
    if low <= high {
        rect(low, high)
    } else {
        format!("{}{}", rect(0.0, high), rect(low, 1.0))
    }
}

/// What to look for in a gait-group figure: every declared member with its
/// anchor or the state that stopped one being measured, the measured spread
/// against the declared cap, and which anchors the drawn band leaves out.
fn gait_group_guidance(
    members: &[GroupMember<'_>],
    group: &GaitGroup,
    band: Option<(f64, f64)>,
) -> String {
    let cap = group.max_gait_phase_spread;
    let listing = members
        .iter()
        .map(|member| format!("{}={}", member.name, member_caption(member.phase)))
        .collect::<Vec<_>>()
        .join(", ");
    let mut clauses = vec![
        "every member is drawn on one unshifted phase axis, so a member that enters the \
         stride elsewhere is the same curve moved along it"
            .to_owned(),
        format!("anchors {listing}"),
    ];
    match band {
        Some((centre, spread)) => {
            clauses.push(if spread > cap {
                format!("they spread {spread:.2} cycle against the {cap:.2} cap")
            } else {
                format!("they spread {spread:.2} cycle, within the {cap:.2} cap")
            });
            let outside: Vec<&str> = members
                .iter()
                .filter(|member| {
                    member
                        .phase
                        .anchor()
                        .is_some_and(|phase| circular_phase_distance(phase, centre) > cap)
                })
                .map(|member| member.name)
                .collect();
            clauses.push(format!(
                "the band is that cap either side of the circular mean of the measured \
                 anchors, and {}",
                match outside.as_slice() {
                    [] => "no member's anchor lies outside it".to_owned(),
                    [one] => format!("{one} lies outside it"),
                    many => format!("{} lie outside it", many.join(", ")),
                }
            ));
        }
        None => clauses.push(format!(
            "fewer than two members have a measured anchor, so no tolerance band is drawn \
             and the {cap:.2} cap is not evaluated here"
        )),
    }
    let beyond = members.len().saturating_sub(GROUP_SERIES_CLASSES.len());
    if beyond > 0 {
        clauses.push(format!(
            "{beyond} declared member(s) past the first {} are named here but not drawn",
            GROUP_SERIES_CLASSES.len()
        ));
    }
    format!("what to look for: {}", clauses.join("; "))
}

/// Which of a line chart's two value axes a series is plotted against,
/// and which gutter a label is aligned to.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    Left,
    Right,
}

/// One plotted series: the stable class the stylesheet and any chart
/// extractor targets, its legend label, the value axis it is scaled
/// against, and the samples.
struct Series<'a> {
    class: &'static str,
    label: &'a str,
    axis: Side,
    values: &'a [f64],
}

/// Where a text label is anchored horizontally.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Anchor {
    Start,
    Middle,
    End,
}

impl Anchor {
    /// The SVG attribute, omitted for the `start` default.
    fn attribute(self) -> &'static str {
        match self {
            Anchor::Start => "",
            Anchor::Middle => " text-anchor=\"middle\"",
            Anchor::End => " text-anchor=\"end\"",
        }
    }
}

/// The swatch a legend entry draws beside its label.
///
/// A plotted series is a line, and a top-down track's two ends are the same
/// hollow circle and filled square the plot marks them with, so the legend
/// says what a shape in the picture means rather than only what a colour does.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Swatch {
    Line,
    Start,
    End,
}

/// One axis label, placed in a chart's gutters.
struct AxisLabel {
    x: f64,
    y: f64,
    anchor: Anchor,
    text: String,
}

/// What a figure is about.
///
/// A clip figure belongs to one clip of the document and is shown only while
/// that clip is selected; a group figure belongs to a declared gait group and
/// is shown whichever member the reader is scrubbing, because comparing the
/// members is the whole of what it says. The subject names the figure in its
/// caption and is the attribute the viewer and the documentation site's chart
/// extractor select it by.
#[derive(Clone, Copy)]
enum Subject<'a> {
    /// One clip of the rendered document.
    Clip(&'a str),
    /// One declared gait group, drawn from its members.
    Group(&'a str),
}

impl<'a> Subject<'a> {
    /// The name a caption and an `aria-label` open with.
    fn name(self) -> &'a str {
        match self {
            Subject::Clip(name) | Subject::Group(name) => name,
        }
    }

    /// The `data-*` attribute this figure is selected by.
    ///
    /// A group figure carries no `data-clip`, because it belongs to no single
    /// clip. Its membership is not an attribute either: a group's members are
    /// authored clip names, and any separator that packs several of them into
    /// one attribute is a name a document may legitimately contain — a member
    /// called `run,left` in a comma-separated list is read as two members,
    /// neither of them itself, and its own figure disappears while it is
    /// selected. The membership travels in the embedded JSON instead, where
    /// a name is a string, and `data-group` is the key that joins the two.
    fn attribute(self) -> String {
        match self {
            Subject::Clip(name) => format!("data-clip=\"{}\"", esc(name)),
            Subject::Group(name) => format!("data-group=\"{}\"", esc(name)),
        }
    }
}

/// The shell every chart shares: the `<figure>` and its `<svg>`, the title,
/// the legend, and the axis labels. A caller contributes only the geometry it
/// plots and the words that describe it, so the chart kinds cannot drift
/// apart on the parts a reader — or the documentation site's extractor —
/// depends on.
struct Chart<'a> {
    subject: Subject<'a>,
    kind: &'static str,
    title: &'static str,
    /// Tail of the `aria-label`, after the subject name.
    description: String,
    /// What a reader should look for in this picture, appended to the
    /// visible caption. It is not repeated into the `aria-label`, which
    /// already states the measured ranges the drawing is evidence for.
    guidance: String,
    /// The legend's entries. [`Chart`] lays them out and derives the plot
    /// rectangle from how many rows they need, so a caller cannot draw in a
    /// rectangle the figure does not reserve.
    legend: Vec<(Swatch, &'static str, String)>,
    axis: Vec<AxisLabel>,
    /// Publishes the plot rectangle the viewer's playhead is placed in.
    plot_hooks: bool,
    body: String,
    /// Anything the figure carries outside its `<svg>`.
    trailer: String,
}

impl Chart<'_> {
    fn render(&self) -> String {
        let subject = esc(self.subject.name());
        let caption = format!("{subject} — {}", esc(self.title));
        let axis: String = self
            .axis
            .iter()
            .map(|label| {
                format!(
                    "<text class=\"axis\" x=\"{:.1}\" y=\"{:.1}\"{}>{}</text>",
                    label.x,
                    label.y,
                    label.anchor.attribute(),
                    esc(&label.text)
                )
            })
            .collect();
        let hooks = if self.plot_hooks {
            format!(" data-pad=\"{PAD_LEFT}\" data-plotw=\"{PLOT_W}\"")
        } else {
            String::new()
        };
        let plot = Plot::for_legend(self.legend.len());
        // The caption a reader sees carries the guidance; the `<title>` stays
        // the figure's short name, so an assistive reader hears what the
        // picture is before the sentence about how to read it.
        let figcaption = if self.guidance.is_empty() {
            caption.clone()
        } else {
            format!("{caption} · {}", esc(&self.guidance))
        };
        format!(
            "<figure class=\"chart\" {} data-kind=\"{}\"{hooks}>\
             <figcaption>{figcaption}</figcaption>\
             <svg viewBox=\"0 0 {W} {}\" width=\"100%\" role=\"img\" \
             aria-label=\"{subject} — {}\"><title>{caption}</title>{}{}{axis}</svg>{}</figure>",
            self.subject.attribute(),
            self.kind,
            plot.height,
            esc(&self.description),
            legend_block(&self.legend),
            self.body,
            self.trailer,
        )
    }
}

/// One legend entry drawn at `(x, y)`: its swatch, then its label.
fn legend_entry(swatch: Swatch, class: &str, label: &str, x: f64, y: f64) -> String {
    let mark = match swatch {
        Swatch::Line => format!(
            "<line class=\"{class}\" x1=\"{x:.1}\" x2=\"{:.1}\" y1=\"{y}\" y2=\"{y}\"/>",
            x + 10.0
        ),
        Swatch::Start => format!(
            "<circle class=\"{class}\" cx=\"{:.1}\" cy=\"{y}\" r=\"{PATH_START_R}\"/>",
            x + 5.0
        ),
        Swatch::End => format!(
            "<rect class=\"{class}\" x=\"{:.1}\" y=\"{:.1}\" width=\"{PATH_END_SIDE}\" \
             height=\"{PATH_END_SIDE}\"/>",
            x + 5.0 - PATH_END_SIDE / 2.0,
            y - PATH_END_SIDE / 2.0
        ),
    };
    format!(
        "{mark}<text class=\"legend\" x=\"{:.1}\" y=\"{:.1}\">{}</text>",
        x + 13.0,
        y + 3.0,
        esc(label)
    )
}

/// Every chart's legend, laid out in fixed columns.
///
/// One row of six clip names is wider than the picture, and a legend entry
/// that runs past the `viewBox` edge is simply not drawn — the reader loses
/// the mapping from colour to series, which is the one thing a legend is
/// for. Fixed columns keep every entry inside the plot width whatever names
/// the document supplies, and a name too long for its column is cut with an
/// ellipsis; a group figure's caption carries every member's full name
/// either way. The two- and three-series charts fill one row of it.
fn legend_block(entries: &[(Swatch, &'static str, String)]) -> String {
    let column = PLOT_W / LEGEND_COLUMNS as f64;
    entries
        .iter()
        .enumerate()
        .map(|(index, (swatch, class, label))| {
            legend_entry(
                *swatch,
                class,
                &fit_label(label, column - LEGEND_ENTRY_MARGIN),
                PAD_LEFT + (index % LEGEND_COLUMNS) as f64 * column,
                LEGEND_Y + (index / LEGEND_COLUMNS) as f64 * LEGEND_ROW_H,
            )
        })
        .collect()
}

/// The plot rectangle a chart draws in.
///
/// A legend of more than one row reserves its extra rows above the plot and
/// pushes it down by exactly that, so the plotted rectangle stays [`PLOT_H`]
/// tall in every figure and the figure itself grows instead. [`Chart`]
/// derives this from the entries it was given and a caller derives it from
/// the same count, so the drawing and the `viewBox` cannot disagree.
#[derive(Clone, Copy)]
struct Plot {
    /// Top edge of the plotted rectangle.
    top: f64,
    /// Bottom edge of the plotted rectangle.
    bottom: f64,
    /// Height of the whole figure's `viewBox`.
    height: f64,
}

impl Plot {
    fn for_legend(entries: usize) -> Self {
        let rows = entries.div_ceil(LEGEND_COLUMNS).max(1);
        let reserved = (rows - 1) as f64 * LEGEND_ROW_H;
        Self {
            top: PAD_TOP + reserved,
            bottom: H - PAD_BOTTOM + reserved,
            height: H + reserved,
        }
    }
}

/// `label` cut to what fits `width` viewBox units at the shared chart type
/// scale, with an ellipsis where it was cut.
fn fit_label(label: &str, width: f64) -> String {
    let fits = (width / LEGEND_CHAR_W).floor().max(1.0) as usize;
    if label.chars().count() <= fits {
        return label.to_owned();
    }
    label
        .chars()
        .take(fits.saturating_sub(1))
        .chain(['…'])
        .collect()
}

const W: f64 = 360.0;
const H: f64 = 150.0;
/// Gutters for the y-axis labels, the legend row, and the x-axis labels. The
/// plot rectangle between them is what `data-pad`/`data-plotw` describe.
const PAD_LEFT: f64 = 34.0;
/// As wide as the left gutter: a line chart labels a second value axis
/// there, and the top-down path chart keeps the plot centred.
const PAD_RIGHT: f64 = 34.0;
const PAD_TOP: f64 = 18.0;
const PAD_BOTTOM: f64 = 16.0;
const PLOT_W: f64 = W - PAD_LEFT - PAD_RIGHT;
const PLOT_H: f64 = H - PAD_TOP - PAD_BOTTOM;
/// Every chart plots metres; the unit is stated in the axis labels rather
/// than left to the caption.
const UNIT: &str = "m";
/// Evaluation-scope codes that only a clip declared `loop = true` produces.
const LOOP_SCOPE_CODES: &[&str] = &[
    "loop_closure",
    "duplicate_loop_endpoint",
    "loop_seam",
    "loop_seam_velocity",
    "loop_seam_rotation",
];
/// The scope `root-motion-speed` emits, which it evaluates only for a clip
/// that pins `speed_mps` and does not hand XZ travel to gameplay.
const ROOT_MOTION_SCOPE_CODES: &[&str] = &["root_motion_speed"];
/// The per-foot scopes `foot-slide` emits once it has stance to judge.
const STANCE_SCOPE_CODES: &[&str] = &["left_foot_stance", "right_foot_stance"];
/// The scope `in-place` emits for a clip that declares an XZ movement owner.
const TRAVEL_MODE_SCOPE_CODES: &[&str] = &["travel_mode"];

/// What this run declared and judged about one clip.
///
/// A caption may only say what to look for in a picture when the document
/// says what the clip owes. The report is handed check evaluations rather
/// than the configuration, and that is the better authority anyway: a
/// check's evaluated scope is the unit of work it actually did on this clip,
/// so a `loop_closure` scope naming the clip *is* its `loop = true`
/// declaration, judged.
///
/// Nothing here infers a route, a stride, or a locomotion intent. Authored
/// root motion that curves or turns is a valid clip, and so is an idle or a
/// jump with no alternating stance; a caption that told a reader to expect a
/// straight line or two feet trading plants would be wrong about the file in
/// front of them. Where nothing is declared, the caption says so and names
/// what was judged instead.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct ClipContract {
    /// A loop check selected work on this clip, so `loop = true` is declared.
    looping: bool,
    /// `root-motion-speed` selected work on this clip, so the animation owns
    /// its horizontal travel at a declared speed.
    root_motion: bool,
    /// `in-place` judged this clip against its declared XZ movement owner.
    travel_mode: bool,
    /// `foot-slide` evaluated stance for at least one of this clip's feet.
    stance: bool,
    /// Every check that completed work naming this clip, in id order.
    judged: Vec<&'static str>,
}

/// Derive [`ClipContract`] from the evaluations the report was handed.
///
/// A declaration is visible whether the work completed or hit a coverage
/// gap, because the declaration is what selected the work either way; stance
/// bands and the checks named as having judged the clip come from completed
/// work only, since a gap is precisely the absence of it.
fn clip_contract(checks: &[CheckEvaluation], clip_name: &str) -> ClipContract {
    let mut contract = ClipContract::default();
    for check in checks {
        let gap_scopes = check.gaps().iter().filter_map(|gap| gap.scope.as_ref());
        for scope in check.evaluated_scopes().iter().chain(gap_scopes) {
            if scope.subject.as_deref() != Some(clip_name) {
                continue;
            }
            let code = scope.code.as_str();
            contract.looping |= LOOP_SCOPE_CODES.contains(&code);
            contract.root_motion |= ROOT_MOTION_SCOPE_CODES.contains(&code);
            contract.travel_mode |= TRAVEL_MODE_SCOPE_CODES.contains(&code);
        }
        let evaluated: Vec<&str> = check
            .evaluated_scopes()
            .iter()
            .filter(|scope| scope.subject.as_deref() == Some(clip_name))
            .map(|scope| scope.code.as_str())
            .collect();
        if evaluated.is_empty() {
            continue;
        }
        contract.stance |= evaluated
            .iter()
            .any(|code| STANCE_SCOPE_CODES.contains(code));
        contract.judged.push(check.check_id());
    }
    contract.judged.sort_unstable();
    contract.judged.dedup();
    contract
}

impl ClipContract {
    /// The sentence for a clip that declares nothing the picture could be
    /// read against, naming what was judged instead of prescribing.
    fn undeclared(&self, contracts: &str) -> String {
        if self.judged.is_empty() {
            format!("no {contracts} contract declared and no check judged this clip")
        } else {
            format!(
                "no {contracts} contract declared: the checks judged only {}",
                self.judged.join(", ")
            )
        }
    }

    /// What to look for in a top-down root path, given `marks` describing how
    /// that panel marks the current frame and the track's two ends.
    fn root_path_guidance(&self, marks: &str) -> String {
        let mut clauses = Vec::new();
        if self.looping {
            clauses.push(String::from(
                "this clip is declared a loop, so its root path should end where it began",
            ));
        }
        if self.root_motion {
            clauses.push(String::from(
                "this clip declares animation-owned root travel at a pinned speed, so the path \
                 should keep travelling at that speed; the shape it travels is whatever the clip \
                 authors, and a turn is not a defect",
            ));
        }
        if self.travel_mode && !self.root_motion {
            clauses.push(String::from(
                "this clip declares who owns its horizontal travel and the in-place check judged \
                 the root against that declaration, so the path is the evidence for it",
            ));
        }
        if clauses.is_empty() {
            clauses.push(format!(
                "{}, so the path is shown as measured rather than against an expectation",
                self.undeclared("loop or root-motion")
            ));
        }
        format!("what to look for: {} · {marks}", clauses.join("; "))
    }

    /// What to look for in a foot-height chart. `bands` is true for the
    /// comparison's gait panel, which shades the judged stance intervals; the
    /// single-clip chart plots none, so it does not claim any.
    fn gait_guidance(&self, bands: bool) -> String {
        let mut clauses = Vec::new();
        if self.looping {
            clauses.push(String::from(
                "this clip is declared a loop, so the curves should end where they began",
            ));
        }
        if self.stance {
            clauses.push(String::from(if bands {
                "the shaded bands are the sampled stance intervals the foot-slide check judged, \
                 and a foot that moves horizontally during its band is the slide"
            } else {
                "the foot-slide check judged stance intervals on this clip, and a foot that \
                 moves horizontally during a plant is the slide it reports"
            }));
        }
        if clauses.is_empty() {
            clauses.push(format!(
                "{}, so the heights are shown as measured rather than against an expectation",
                self.undeclared("loop or stance")
            ));
        }
        format!("what to look for: {}", clauses.join("; "))
    }
}

/// How the single-clip root path marks the frame a reader is on and the two
/// ends of the track.
const SINGLE_PATH_MARKS: &str = "the dot is the current frame, the hollow circle where the track \
     starts and the square where it ends";
/// The same for the comparison's shared panel, whose dot is the shared phase
/// and which draws two tracks.
const SHARED_PATH_MARKS: &str = "the dot is the shared phase, the hollow circle where a track \
     starts and the square where it ends";
const LEGEND_Y: f64 = 9.0;
/// Columns [`grid_legend`] lays its entries out in, and with the six series
/// colours a group figure paints, the two rows it can need.
const LEGEND_COLUMNS: usize = 3;
/// Height of one legend row, in viewBox units.
const LEGEND_ROW_H: f64 = 11.0;
/// Room a grid-legend entry gives its swatch and the gap to the next
/// column, before the label may use what is left.
const LEGEND_ENTRY_MARGIN: f64 = 16.0;
/// Average glyph advance at the shared `--chart-type` scale the
/// stylesheets set on chart labels, in viewBox units. Approximate on
/// purpose — it only lays legend entries out left to right, and they have
/// only to stay inside the plot width. The comparison viewer lays its own
/// legends out from the same number.
const LEGEND_CHAR_W: f64 = 4.6;
/// Extent below which a top-down path is reported as stationary rather
/// than plotted: a millimetre, which is finer than any clip an engine
/// distinguishes from standing still.
const STATIC_PATH_M: f64 = 0.001;
/// Radius of the hollow circle marking where a top-down track starts. It
/// stands outside the playhead dot's own radius, so the two are two marks
/// wherever the current frame is the track's first one.
const PATH_START_R: f64 = 6.0;
/// How close a track's two drawn ends have to be before the end mark steps
/// aside: inside this, a square, a ring and the playhead dot land on one
/// coordinate and read as a single blob.
const PATH_MARK_CLEAR: f64 = PATH_START_R + 3.0;
/// How far the end mark steps, toward the middle of the plot so it cannot
/// leave the picture, with a leader back to the coordinate it belongs to.
const PATH_MARK_OFFSET: f64 = 11.0;
/// Side of the filled square marking where a top-down track ends.
///
/// A track drawn as one line says nothing about which way it was walked: a
/// clip that travels out and never returns and a clip that comes back over
/// its own line are the same picture. Two different marks at the two ends
/// tell them apart, and the shapes differ as well as the roles, so the pair
/// survives a reader who cannot separate the two colours.
const PATH_END_SIDE: f64 = 6.0;
/// The per-frame `pathpoints` entry for a frame with no sampled position.
/// It carries no coordinate at all, so a viewer cannot mistake it for one;
/// `assets/viewer.js` hides the playhead dot when it reads this.
const NO_POSITION: &str = "-";

/// The value range of one axis' series, over the samples that are finite.
///
/// A channel can be non-finite for every frame it was sampled at — that is
/// what the `nan` check exists to report — and a derived series inherits
/// it, so an all-NaN right foot makes `L−R` NaN throughout. Folding those
/// in yields a `NaN` range, which then prints `NaN m` in the gutter and
/// plots a path of `NaN` coordinates that no renderer draws. `None` means
/// there is nothing to scale and nothing to plot.
fn axis_range(series: &[Series<'_>], axis: Side) -> Option<(f64, f64)> {
    let values = series
        .iter()
        .filter(|entry| entry.axis == axis)
        .flat_map(|entry| entry.values.iter().copied())
        .filter(|value| value.is_finite());
    values.fold(None, |range, value| {
        Some(match range {
            None => (value, value),
            Some((min, max)) => (min.min(value), max.max(value)),
        })
    })
}

/// Whether a range is a single value: every sample on that axis is the
/// same number, so there is no span to scale across.
fn is_flat((min, max): (f64, f64)) -> bool {
    (max - min).abs() < f64::EPSILON
}

/// The polyline through `count` plotted points, where `at` yields the one
/// at an index or `None` when that index has nothing to plot.
///
/// A gap starts a new subpath. Dropping the unplottable points and joining
/// what is left would draw a straight segment between the samples either
/// side of the hole — a stretch of trajectory the reader is being shown as
/// measured evidence, and which no frame recorded. Empty when nothing is
/// plottable at all.
fn polyline(count: usize, at: impl Fn(usize) -> Option<(f64, f64)>) -> String {
    let mut path = String::new();
    let mut drawing = false;
    for index in 0..count {
        let Some((x, y)) = at(index).filter(|(x, y)| x.is_finite() && y.is_finite()) else {
            drawing = false;
            continue;
        };
        path.push_str(&format!("{}{x:.1},{y:.1}", if drawing { "L" } else { "M" }));
        drawing = true;
    }
    path
}

/// The `(x, z)` root sample at `frame`, when both coordinates are finite.
///
/// A trajectory point needs both: a frame finite in X but not in Z has no
/// position to plot. Taking the two extents separately instead lets a
/// track that alternates between them look plottable when not one frame
/// is — which is a shape a malformed file reaches, not a theoretical one.
fn joint_sample(xs: &[f64], zs: &[f64], frame: usize) -> Option<(f64, f64)> {
    let (x, z) = (*xs.get(frame)?, *zs.get(frame)?);
    (x.is_finite() && z.is_finite()).then_some((x, z))
}

/// The X and Z extents of a set of jointly finite trajectory points.
fn joint_extent(points: &[(f64, f64)]) -> Option<((f64, f64), (f64, f64))> {
    points.iter().fold(None, |extent, &(x, z)| {
        Some(match extent {
            None => ((x, x), (z, z)),
            Some(((min_x, max_x), (min_z, max_z))) => {
                ((min_x.min(x), max_x.max(x)), (min_z.min(z), max_z.max(z)))
            }
        })
    })
}

/// A line chart of one or two independently scaled value axes.
///
/// Series that share an axis share a scale, so they can be read against
/// each other; series on the other axis get their own, so a signal
/// orders of magnitude larger cannot flatten the rest. The gait chart
/// needs exactly that: both feet swing within about ten centimetres of
/// each other a metre below the hips, while their difference swings
/// about zero. On one shared scale the two foot curves collapse into a
/// line at the bottom of the plot — the picture stops showing the thing
/// the reader came for.
fn line_chart(
    clip: &str,
    kind: &'static str,
    title: &'static str,
    guidance: &str,
    series: &[Series<'_>],
) -> String {
    let Some(left) = axis_range(series, Side::Left) else {
        return String::new();
    };
    let right = axis_range(series, Side::Right);
    let frames = series[0].values.len();
    let n = frames.max(2);
    let last_frame = frames.saturating_sub(1);
    let x = |i: usize| PAD_LEFT + PLOT_W * i as f64 / (n - 1) as f64;
    // A series that never changes has no span to scale across. Dividing by
    // a clamped epsilon pins it to whichever gutter its own value sits at —
    // the bottom row — and prints that one value twice, as though it were a
    // range. Two feet exactly in phase make `L−R` identically zero, which
    // is a real clip, so a flat series is centred instead and labelled once.
    let y = |range: (f64, f64), v: f64| {
        let (min, max) = range;
        if is_flat(range) {
            PAD_TOP + PLOT_H / 2.0
        } else {
            H - PAD_BOTTOM - PLOT_H * (v - min) / (max - min)
        }
    };

    let mut body = String::new();
    for entry in series {
        // An axis with no finite sample has no range, so its series is not
        // plotted at all rather than plotted against the other axis'.
        let Some(range) = (match entry.axis {
            Side::Left => Some(left),
            Side::Right => right,
        }) else {
            continue;
        };
        let d = polyline(entry.values.len(), |index| {
            let value = entry.values[index];
            value.is_finite().then(|| (x(index), y(range, value)))
        });
        if d.is_empty() {
            continue;
        }
        body.push_str(&format!(
            "<path class=\"{}\" d=\"{d}\" fill=\"none\"/>",
            entry.class,
        ));
    }
    body.push_str(&format!(
        "<line class=\"playhead\" x1=\"{PAD_LEFT}\" x2=\"{PAD_LEFT}\" y1=\"{PAD_TOP}\" y2=\"{:.1}\"/>",
        H - PAD_BOTTOM
    ));

    // Each axis states its own range in its own gutter, and the legend
    // says which series is read against the right-hand one, so two scales
    // on one plot cannot be mistaken for one.
    let mut axis = Vec::with_capacity(6);
    for (side, (min, max)) in [(Side::Left, Some(left)), (Side::Right, right)]
        .into_iter()
        .filter_map(|(side, range)| range.map(|range| (side, range)))
    {
        let (at, anchor) = match side {
            Side::Left => (2.0, Anchor::Start),
            Side::Right => (W - 2.0, Anchor::End),
        };
        if is_flat((min, max)) {
            axis.push(AxisLabel {
                x: at,
                y: PAD_TOP + PLOT_H / 2.0 + 3.0,
                anchor,
                text: format!("flat {min:.2} {UNIT}"),
            });
            continue;
        }
        axis.push(AxisLabel {
            x: at,
            y: PAD_TOP + 3.0,
            anchor,
            text: format!("{max:.2} {UNIT}"),
        });
        axis.push(AxisLabel {
            x: at,
            y: H - PAD_BOTTOM,
            anchor,
            text: format!("{min:.2} {UNIT}"),
        });
    }
    axis.push(AxisLabel {
        x: PAD_LEFT,
        y: H - 4.0,
        anchor: Anchor::Start,
        text: "frame 0".to_owned(),
    });
    axis.push(AxisLabel {
        x: W - PAD_RIGHT,
        y: H - 4.0,
        anchor: Anchor::End,
        text: format!("frame {last_frame}"),
    });

    let legend_labels: Vec<String> = series
        .iter()
        .map(|entry| match entry.axis {
            Side::Left => entry.label.to_owned(),
            Side::Right => format!("{} (right axis)", entry.label),
        })
        .collect();
    let named = |axis: Side| -> String {
        series
            .iter()
            .filter(|entry| entry.axis == axis)
            .map(|entry| entry.label)
            .collect::<Vec<_>>()
            .join(", ")
    };
    let scale = |axis: Side, range: (f64, f64)| {
        let (min, max) = range;
        if is_flat(range) {
            format!("{} flat at {min:.2} {UNIT}", named(axis))
        } else {
            format!("{} {min:.2} to {max:.2} {UNIT}", named(axis))
        }
    };
    let described = match right {
        // No finite sample on the right axis: its series is named as
        // unplottable rather than left out, so the description still
        // accounts for every series the legend lists.
        None if series.iter().any(|entry| entry.axis == Side::Right) => format!(
            "{}; {} has no finite sample and is not plotted",
            scale(Side::Left, left),
            named(Side::Right)
        ),
        None => scale(Side::Left, left),
        Some(right) => format!(
            "{}; {} on its own right-hand axis",
            scale(Side::Left, left),
            scale(Side::Right, right)
        ),
    };
    Chart {
        subject: Subject::Clip(clip),
        kind,
        title,
        description: format!("{title} over frames 0 to {last_frame}: {described}"),
        guidance: guidance.to_owned(),
        legend: series
            .iter()
            .zip(legend_labels)
            .map(|(entry, label)| (Swatch::Line, entry.class, label))
            .collect(),
        axis,
        plot_hooks: true,
        body,
        trailer: String::new(),
    }
    .render()
}

fn path_chart(clip: &str, title: &'static str, xs: &[f64], zs: &[f64], guidance: &str) -> String {
    if xs.is_empty() {
        return String::new();
    }
    // A position needs a finite X *and* Z on the same frame, so the plot,
    // its extents and its first point all come from the frames that have
    // both. Sampled frames can be non-finite — that is what the `nan` check
    // reports — and taking the two extents per coordinate instead let a
    // track finite in X on one frame and in Z on the next look plottable
    // when no single frame was, leaving no first point to place the dot at.
    // A run with no plottable frame at all says so, the way the comparison
    // viewer's root panel does, rather than folding `f64::MAX`/`f64::MIN`
    // seeds into a negative span that reads as a stationary root parked at
    // "X 179769313486231570000…00 m".
    let plotted: Vec<(f64, f64)> = (0..xs.len())
        .filter_map(|frame| joint_sample(xs, zs, frame))
        .collect();
    let Some(((min_x, max_x), (min_z, max_z))) = joint_extent(&plotted) else {
        return Chart {
            subject: Subject::Clip(clip),
            kind: "rootpath",
            title,
            description: format!(
                "{title}: unavailable — not one of the {} sampled root frames has a finite \
                 X and Z together; findings and coverage remain listed",
                xs.len()
            ),
            guidance: guidance.to_owned(),
            legend: vec![(Swatch::Line, "root-path", "root".to_owned())],
            axis: vec![AxisLabel {
                x: W / 2.0,
                y: PAD_TOP + PLOT_H / 2.0,
                anchor: Anchor::Middle,
                text: "root path unavailable: sampled positions are non-finite".to_owned(),
            }],
            plot_hooks: false,
            body: String::new(),
            trailer: String::new(),
        }
        .render();
    };
    // One metres scale for both axes: a top-down path that is squashed in Z
    // would misdescribe the trajectory it is evidence for.
    let span = (max_x - min_x).max(max_z - min_z).max(1e-3);
    let scale = PLOT_W.min(PLOT_H) / span;
    let center_x = PAD_LEFT + PLOT_W / 2.0;
    let center_y = PAD_TOP + PLOT_H / 2.0;
    let x = |v: f64| center_x + (v - (min_x + max_x) / 2.0) * scale;
    let y = |v: f64| center_y - (v - (min_z + max_z) / 2.0) * scale;
    let d = polyline(xs.len(), |frame| {
        joint_sample(xs, zs, frame).map(|(px, pz)| (x(px), y(pz)))
    });
    // The viewer places the playhead dot by frame index, so this template
    // keeps one entry per frame. A frame with no sampled position carries
    // [`NO_POSITION`] rather than a coordinate, and the viewer hides the dot
    // for it: borrowing the nearest frame that does have one shows the
    // reader a place the clip is not at that moment — for a leading hole, a
    // coordinate it only reaches later.
    let points: Vec<String> = (0..xs.len())
        .map(|frame| match joint_sample(xs, zs, frame) {
            Some((px, pz)) => format!("{:.1},{:.1}", x(px), y(pz)),
            None => NO_POSITION.to_owned(),
        })
        .collect();

    // An in-place clip plots one dot, and an empty square captioned
    // `X 0.00…0.00 m` reads as a chart that failed rather than as the
    // measurement it is. Say what the plot shows in words, and keep both
    // range labels so the unit and the numbers stay where a reader of any
    // other root path already looks for them.
    let mut axis = Vec::with_capacity(3);
    let stationary = (max_x - min_x).max(max_z - min_z) < STATIC_PATH_M;
    if stationary {
        let at_origin = min_x.abs().max(min_z.abs()) < STATIC_PATH_M;
        axis.push(AxisLabel {
            x: W / 2.0,
            y: center_y - 10.0,
            anchor: Anchor::Middle,
            text: if at_origin {
                "root stays at the origin".to_owned()
            } else {
                format!("root stays at X {min_x:.2} {UNIT}, Z {min_z:.2} {UNIT}")
            },
        });
    }
    axis.push(AxisLabel {
        x: 2.0,
        y: H - 4.0,
        anchor: Anchor::Start,
        text: format!("X {min_x:.2}…{max_x:.2} {UNIT}"),
    });
    axis.push(AxisLabel {
        x: W - 2.0,
        y: H - 4.0,
        anchor: Anchor::End,
        text: format!("Z {min_z:.2}…{max_z:.2} {UNIT}"),
    });

    // The size of the thing drawn, in the unit it was measured in: the plot
    // is fitted to its own extent, so a two-centimetre sway and a two-metre
    // stride fill the same square and only this number tells them apart.
    let extent = format!(
        "the path spans {:.3} {UNIT} at its widest",
        (max_x - min_x).max(max_z - min_z)
    );
    // Where the track ends relative to where it began, which is what the two
    // end marks show and what a loop is judged on. `STATIC_PATH_M` is the
    // same millimetre the stationary test uses: closer than that and the two
    // marks coincide, so the words and the picture agree.
    let (start, end) = (plotted[0], plotted[plotted.len() - 1]);
    let gap = ((end.0 - start.0).powi(2) + (end.1 - start.1).powi(2)).sqrt();
    let closure = if gap < STATIC_PATH_M {
        "the track closes on itself".to_owned()
    } else {
        format!("the track ends {gap:.3} {UNIT} from its start")
    };

    // Where the two marks are drawn. A track that comes back to where it
    // began puts its ring, its square and the playhead dot on one
    // coordinate, and a filled square inside a ring inside a dot is one
    // blob: the square steps aside toward the middle of the plot, where it
    // cannot leave the picture, and a leader says which point it belongs to.
    let (start_x, start_y) = (x(start.0), y(start.1));
    let (end_x, end_y) = (x(end.0), y(end.1));
    let apart = ((end_x - start_x).powi(2) + (end_y - start_y).powi(2)).sqrt();
    let (mark_x, mark_y) = if apart < PATH_MARK_CLEAR {
        (
            end_x
                + if end_x > center_x {
                    -PATH_MARK_OFFSET
                } else {
                    PATH_MARK_OFFSET
                },
            end_y
                + if end_y > center_y {
                    -PATH_MARK_OFFSET
                } else {
                    PATH_MARK_OFFSET
                },
        )
    } else {
        (end_x, end_y)
    };
    let leader = if apart < PATH_MARK_CLEAR {
        format!(
            "<line class=\"pathleader\" x1=\"{end_x:.1}\" y1=\"{end_y:.1}\" \
             x2=\"{mark_x:.1}\" y2=\"{mark_y:.1}\"/>"
        )
    } else {
        String::new()
    };

    Chart {
        subject: Subject::Clip(clip),
        kind: "rootpath",
        title,
        description: if stationary {
            format!(
                "{title}: the root does not move, staying at X {min_x:.2} {UNIT}, \
                 Z {min_z:.2} {UNIT} for all {} frames",
                xs.len()
            )
        } else {
            format!(
                "{title}: X {min_x:.2} to {max_x:.2} {UNIT}, Z {min_z:.2} to {max_z:.2} {UNIT}, \
                 {} frames on one uniform scale",
                xs.len()
            )
        },
        guidance: format!("{guidance} · {extent} · {closure}"),
        legend: vec![
            (Swatch::Line, "root-path", "root".to_owned()),
            (Swatch::Start, "pathstart", "start".to_owned()),
            (Swatch::End, "pathend", "end".to_owned()),
        ],
        axis,
        plot_hooks: false,
        // The dot marks the selected frame, so the document opens with it on
        // frame 0 — or hidden, when frame 0 has no position. A static render
        // of this figure (an extracted chart, a document with no script) then
        // shows the same thing the viewer would.
        body: format!(
            "<path class=\"root-path\" d=\"{d}\" fill=\"none\"/>\
             <circle class=\"pathstart\" cx=\"{start_x:.1}\" cy=\"{start_y:.1}\" \
             r=\"{PATH_START_R}\"/>{leader}\
             <rect class=\"pathend\" x=\"{:.1}\" y=\"{:.1}\" width=\"{PATH_END_SIDE}\" \
             height=\"{PATH_END_SIDE}\"/>{}",
            mark_x - PATH_END_SIDE / 2.0,
            mark_y - PATH_END_SIDE / 2.0,
            match joint_sample(xs, zs, 0) {
                Some((px, pz)) => format!(
                    "<circle class=\"pathdot\" r=\"3\" cx=\"{:.1}\" cy=\"{:.1}\"/>",
                    x(px),
                    y(pz)
                ),
                None => "<circle class=\"pathdot\" r=\"3\" display=\"none\"/>".to_owned(),
            }
        ),
        trailer: format!(
            "<template class=\"pathpoints\">{}</template>",
            points.join(";")
        ),
    }
    .render()
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

        let options = ReportOptions::default();
        let grids_once = MetricGrids::new(&doc);
        let fresh = render(ReportInputs {
            options,
            ..ReportInputs::new(&grids_once, &roles, &checks, &config)
        });
        let grids = MetricGrids::new(&doc);
        let ctx = CheckCtx::new(&grids, &roles, &config);
        assert!(ctx.grid(0).is_some());
        let shared = render(ReportInputs {
            options,
            ..ReportInputs::new(&grids, &roles, &checks, &config)
        });

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
