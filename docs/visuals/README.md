# Documentation visuals

Every picture in the customer documentation is produced by AnimSmith
itself from a committed synthetic fixture, or drawn by hand. Nothing here
comes from a licensed or third-party asset.

This page is provenance for the directory it sits in, read where a
contributor finds it: it is not a documentation-site chapter, and the
site publishes only the visuals themselves.

## Generated files

`crates/animsmith/examples/gen_docs_visuals.rs` runs the `report` command
on the committed fixtures in [`examples/assets/`](../../examples/assets/README.md)
with the committed configs beside them, then cuts the standalone charts
out of the rendered reports. Regenerate them with:

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
| `foot-slide-after.report.html` | `report-comparison-after.glb` | [`report-comparison.animsmith.toml`](../../examples/report-comparison.animsmith.toml) |
| `foot-slide.comparison.html` | both `report-comparison-*.glb`, clip `acceptance-matrix` | [`report-comparison.animsmith.toml`](../../examples/report-comparison.animsmith.toml) |
| `walk-dirty.foot-height.svg` | foot-height figure of `walk-dirty.report.html` | |
| `walk.foot-height.svg` | foot-height figure of `walk.report.html` | |
| `walk.root-path.svg` | root-path figure of `walk.report.html` | |
| `walk-travel.root-path.svg` | root-path figure of `walk-travel.report.html` | |
| `run-ring.run-forward.foot-height.svg` | foot-height figure of clip `run_forward` in `run-ring.report.html` | |
| `run-ring.run-left.foot-height.svg` | foot-height figure of clip `run_left` in `run-ring.report.html` | |
| `foot-slide-before.foot-height.svg` | foot-height figure of `foot-slide-before.report.html` | |
| `foot-slide-after.foot-height.svg` | foot-height figure of `foot-slide-after.report.html` | |

The reports are rendered from the fixture directory, so each one names
its input by basename: no checkout path, no timestamp, and no absolute
path is embedded, and two machines produce the same bytes.

An extracted chart is the report's own `<svg>` element with three
changes: the SVG namespace a standalone document needs, the series
colours inlined as a `<style>` (light values, dark under
`prefers-color-scheme`, both taken from the report's design tokens),
and the playhead removed, because a still picture has no frame
selection. Charts are embedded with `<img>`; whole reports are embedded
with `<iframe>` and always followed by a plain link, because GitHub
renders the link and strips the frame.

A report of more than one clip carries one figure per clip, so a chart
cut out of it names the clip as well as the figure kind; an ambiguous
selector is an error rather than a silent first match.

A chart earns a file only where a still picture carries the symptom on
its own. `walk-travel.glb` is why the root-path figures are committed:
it is the one fixture whose root actually travels, so its path reads as
a line against the stationary dot the in-place `walk.glb` draws. The
remaining reports are embedded whole, because what they show is the
findings list or the judged pose grid rather than one curve.

## Hand-authored files

`icons/` holds one hand-drawn mark per symptom page — a few shapes and a
CSS animation each, with a `prefers-reduced-motion` still frame and
light/dark colours. They illustrate a symptom; they are not evidence,
and no measurement is taken from them.

## Releases

The reports are regenerated at release time together with the example
assets they are rendered from — see [RELEASING.md](../../RELEASING.md).
