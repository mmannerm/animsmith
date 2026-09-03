# Documentation visuals

Every picture in the customer documentation is produced by AnimSmith
itself from a committed synthetic fixture, or drawn by hand. Nothing here
comes from a licensed or third-party asset.

This page is provenance for the directory it sits in, read where a
contributor finds it: it is not a documentation-site chapter, and the
site publishes only the visuals themselves.

## Generated files

`crates/animsmith/examples/gen_docs_visuals.rs` runs one ordered list of
`animsmith` invocations over the committed fixtures in
[`examples/assets/`](../../examples/assets/README.md) with the committed
configs beside them, then cuts the standalone charts out of the rendered
reports. Regenerate them with:

```console
$ cargo run -p animsmith --example gen_docs_visuals
```

`crates/animsmith/tests/docs_visuals.rs` regenerates into a temporary
directory and compares byte for byte, so a committed file can never drift
from the tool that made it. The manifest and the chart extractor live in
one place — `animsmith-testkit`'s `docs_visuals` module — which the
generator and that test both drive.

| File | Fixture | Config |
| --- | --- | --- |
| `walk-dirty.report.html` | `walk-dirty.glb` | [`walk.animsmith.toml`](../../examples/walk.animsmith.toml) |
| `walk.report.html` | `walk.glb` | [`walk.animsmith.toml`](../../examples/walk.animsmith.toml) |
| `clip-dirty.report.html` | `clip-dirty.glb` | none — the mechanical checks need no contract |
| `walk-short-channel.report.html` | `walk-short-channel.glb` | none — the mechanical checks need no contract |
| `walk-travel.report.html` | `walk-travel.glb` | [`walk-travel-in-place.animsmith.toml`](../../examples/walk-travel-in-place.animsmith.toml) |
| `run-ring.report.html` | `run-ring.glb` | [`run-ring.animsmith.toml`](../../examples/run-ring.animsmith.toml) |
| `walk-frozen-arm.report.html` | `walk-frozen-arm.glb` | [`walk-frozen-arm.animsmith.toml`](../../examples/walk-frozen-arm.animsmith.toml) |
| `walk-scaled.report.html` | `walk-scaled.glb` | none — the mechanical checks need no contract |
| `foot-slide-before.report.html` | `report-comparison-before.glb` | [`report-comparison.animsmith.toml`](../../examples/report-comparison.animsmith.toml) |
| `foot-slide.comparison.html` | both `report-comparison-*.glb`, clip `acceptance-matrix` | [`report-comparison.animsmith.toml`](../../examples/report-comparison.animsmith.toml) |
| `clip-dirty.fix.comparison.html` | `clip-dirty.glb` against `animsmith fix clip-dirty.glb`'s own output, clip `swing` | none — the mechanical checks need no contract |
| `walk-dirty.foot-height.svg` | foot-height figure of `walk-dirty.report.html` | |
| `walk.foot-height.svg` | foot-height figure of `walk.report.html` | |
| `walk.root-path.svg` | root-path figure of `walk.report.html` | |
| `walk-travel.root-path.svg` | root-path figure of `walk-travel.report.html` | |
| `run-ring.gait-group.svg` | gait-group figure of group `run-ring` in `run-ring.report.html` | |

The reports are rendered from the fixture directory, so each one names
its input by basename: no checkout path, no timestamp, and no absolute
path is embedded, and two machines produce the same bytes.

A report may also need an input no fixture holds — the output of the very
command the page it illustrates is about. Each invocation in the manifest
therefore names the file it writes, and one that names `{scratch}/…` writes
an intermediate rather than a committed visual; run order is the whole
dependency rule, so a later invocation reads what an earlier one wrote.
`clip-dirty.fix.comparison.html` is the one report built that way today: its
after side is what `animsmith fix clip-dirty.glb` writes, not a hand-authored
clean clip that resembles it. A comparison document records each side by
content identity, so the committed bytes carry that output's digest and no
path — `a_prepared_report_does_not_depend_on_where_its_input_was_written` in
`crates/animsmith/tests/docs_visuals.rs` renders it into two differently named
scratch directories and compares the results.

An extracted chart is the report's own `<svg>` element with four
changes: the SVG namespace a standalone document needs, an id that keeps
its styling to itself when a page inlines several pictures, the series
colours inlined as a `<style>`, and the playhead removed, because a still
picture has no frame selection.

Every colour in a committed picture resolves through the documentation
theme's own `--as-*` token, with the report's design token as the
fallback (light first, dark under `prefers-color-scheme`). The site build
inlines these drawings into the pages that show them, so the page's own
theme paints them — including an explicit light or dark choice that
differs from the operating system, which `prefers-color-scheme` alone
gets wrong — while GitHub and a standalone open use the fallbacks.

Pages reference a chart with `<img>` and a whole report with `<iframe>`
followed by a plain link, because GitHub renders the link and strips the
frame. Building the site rewrites both: an `<img>` naming a tracked
picture here becomes the picture itself, and a frame becomes a
site-absolute path, which is the one spelling that resolves both on the
chapter page and on mdBook's aggregated print page.

A report carries one figure per clip and, where the run declares gait
groups, one per group as well, so a chart cut out of a report of more than
one names its group as well as the figure kind; an ambiguous selector is an
error rather than a silent first match. A group figure carries every member
on one shared phase axis, which is one picture where two per-clip charts side
by side used to be — and the reason no committed chart is cut per clip any
more.

A visual earns its bytes only where a page shows it: every generated file
here is embedded, linked or pictured on a page under `docs/`, or is the report
a committed chart is cut from — `walk.report.html` is the second kind.
`every_committed_visual_is_shown_on_a_page_or_is_a_charts_source` holds the
directory to that, so a rendered document no reader can reach loses its
manifest entry rather than sitting here.

A chart earns a file only where a still picture carries the symptom on
its own. `walk-travel.glb` is why the root-path figures are committed:
it is the one fixture whose root actually travels, so its path reads as
a line against the stationary dot the in-place `walk.glb` draws. The
remaining reports are embedded whole, because what they show is the
findings list or the judged pose grid rather than one curve.

## Hand-authored files

`icons/` holds one hand-drawn mark per symptom page — a few shapes and a
CSS animation each, with a `prefers-reduced-motion` still frame, an id that
scopes their rules, and the same token-with-fallback colours the generated
charts use. They illustrate a symptom; they are not evidence, and no
measurement is taken from them.

| Drawing | Page it opens |
| --- | --- |
| `icons/pose-flickers.svg` | [the pose flickers, spins, or explodes](../symptoms/pose-flickers.md) |
| `icons/wrong-length.svg` | [the clip is the wrong length or freezes at the end](../symptoms/wrong-length.md) |
| `icons/loop-pops.svg` | [the loop pops](../symptoms/loop-pops.md) |
| `icons/character-glides.svg` | [the character glides or runs in place](../symptoms/character-glides.md) |
| `icons/blend-skate.svg` | [feet skate when clips blend](../symptoms/blend-skate.md) |
| `icons/feet-slide.svg` | [feet slide within a clip](../symptoms/feet-slide.md) |
| `icons/limb-frozen.svg` | [a limb is T-posed, or a bone never moves](../symptoms/limb-frozen.md) |
| `icons/identity-mismatch.svg` | [files disagree about skeleton or clip identity](../symptoms/identity-mismatch.md) |
| `icons/file-bloat.svg` | [the file is bloated, or the retargeter chokes](../symptoms/file-bloat.md) |

## Releases

The reports are regenerated at release time together with the example
assets they are rendered from — see [RELEASING.md](../../RELEASING.md).
