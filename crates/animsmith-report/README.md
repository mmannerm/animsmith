# animsmith-report

> **Pre-1.0:** Breaking changes are expected between minor releases. Pin
> dependency versions and review the release notes before upgrading.

## Overview

`animsmith-report` renders typed animsmith check evaluations into a single offline HTML
report. It is the report-generation crate used by the CLI's `report`
command: callers provide `MetricGrids` built from the loaded `Document`,
resolved rig roles, check evaluations, and optional prediction provenance;
the crate returns self-contained HTML without flattening predictions into findings.

The report embeds the pose-grid frames computed on the Rust side and
plays back exactly those frames in a small hand-written WebGL viewer.
There is no CDN, no three.js dependency, and no JavaScript resampling;
when a finding names a frame, the viewer scrubs to that judged frame.

`ReportOptions::evidence_only` omits the sampled pose grid from either report
form, for sharing evidence where the motion itself cannot travel; the
[CLI reference](https://github.com/mmannerm/animsmith/blob/main/docs/cli.md#commands)
describes exactly what it keeps and drops.

Every colour in a generated document resolves through one set of design
tokens: dark by default, light under `prefers-color-scheme`, and either one
pinned by a `#theme=light|dark` URL fragment, which one bounded parser reads
along with the `embed`, `clip`, `frame`, and `finding` options. Each chart is
a self-describing `<figure>` — `viewBox`, `role="img"`, an `aria-label` naming
the plotted series and their units, an in-chart legend, and axis labels —
whose paint comes from stable series classes (`series-left`, `series-right`,
`series-diff`, `root-path`, and for a gait group `series-member-0`…`5`,
`anchor-member-0`…`5` and `phase-band`) rather than per-element attributes, so
a figure lifted out of the report keeps its meaning. The `data-kind`,
`data-pad`, and `data-plotw` hooks the playhead uses are part of that
contract, along with the subject a figure names: `data-clip` for the gait and
root-path figures of one clip, `data-group` for the figure a declared gait
group draws its members on. A figure whose legend needs a second row grows its
`viewBox` by that row and moves its plot down, so the plotted rectangle is the
same height in every chart.

That group figure is why `render` takes a `ReportInputs` carrying the
`Config` the checks were evaluated under: a coherent group emits no finding
carrying its membership or its cap, so the declarations are the only authority
for it. Every member is drawn on one unshifted normalized source-phase axis —
the stride cycle its anchor was measured on — each measured anchor is marked at
its own phase, and the group's `max_gait_phase_spread` is drawn as a band
around their circular mean. `animsmith-core` classifies the members
(`gait_phase_evidence` and `gait_member_phase`), so the figure and the
`gait-group` check cannot disagree about which member was measured. The figure
names no canonical member, because the check does not have one: it states
which measured anchors lie outside that band. The document embeds each drawn
group's declared membership under `groups`, joined to its figure by
`data-group`, so a viewer shows the figure only while one of those members is
selected; membership is data rather than a delimited attribute because a clip
name is arbitrary authored text that any separator could split.

`render_comparison` is the deliberately narrow before/after companion. Call
`preflight_comparison_sources` on the two exact `LoadedSource` authorities
before evaluating checks, then pass two explicit clip names and the exact
configuration used for their checks. It refuses identical or incomplete rooted
dependency-closure identities, including duplicate authored names that a loader
disambiguated in its normalized document, as well as
non-identical named/indexed skeleton correspondence, embeds bounded sampled
poses, and writes separate finding, gap, and prediction-provenance records for
each selected clip. The viewer displays both complete closure identities. Its
normalized-phase display labels both source times and does not claim an authored
retime or artistic/engine acceptance.

Its two judged poses are drawn side by side, and one toggle beside the shared
phase draws them in a single pane instead: the before skeleton solid, the after
one dashed over it, at the same two selected frames and through the same shared
camera. The two-pane layout is the default, and the overlay stands down while a
loop-seam context — one side's own two endpoint frames — is selected.

The comparison view reuses typed seam findings and `foot-slide` evaluated
scopes. The latter are projected through the shared sampled-stance classifier
with the run's effective contact threshold, so the report can show endpoint
poses, selected feet, and stance intervals without parsing finding messages.
Root, hips, and bilateral-foot trajectories come from the same exact pose grid;
the combined before/after root chart uses one metres scale. Quaternion and
constant-track findings are labeled structural evidence because redundant
track remediation may leave the visible pose unchanged.

## Install

```toml
[dependencies]
animsmith-core = "0.12"
animsmith-report = "0.12"
```

The compiling render example lives in the crate-level API documentation.
Pass the same `animsmith_core::MetricGrids` used for checks or measurements
to render those sampled frames without resampling the clips.

## Feature Flags

This crate has no public feature flags. In the `animsmith` CLI, the
HTML report command is controlled by the default `report` feature and is
omitted by `--no-default-features`. The workspace MSRV is Rust 1.88.

## More Detail

- [API reference on docs.rs](https://docs.rs/animsmith-report)
- [CLI report command](https://github.com/mmannerm/animsmith/blob/main/docs/cli.md#commands)
- [Embedding guide](https://github.com/mmannerm/animsmith/blob/main/docs/embedding.md)
- [Raw asset to game-ready pipeline scenarios](https://github.com/mmannerm/animsmith/blob/main/docs/pipeline-scenarios.md)
- [Workspace design](https://github.com/mmannerm/animsmith/blob/main/DESIGN.md)

## License

Licensed under either the MIT license or the Apache License, Version
2.0, at your option.

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in animsmith by you is licensed as MIT OR
Apache-2.0, without any additional terms or conditions.
