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
| `foot-slide-before.report.html` | `report-comparison-before.glb` | [`report-comparison.animsmith.toml`](../../examples/report-comparison.animsmith.toml) |
| `foot-slide-after.report.html` | `report-comparison-after.glb` | [`report-comparison.animsmith.toml`](../../examples/report-comparison.animsmith.toml) |
| `foot-slide.comparison.html` | both `report-comparison-*.glb`, clip `acceptance-matrix` | [`report-comparison.animsmith.toml`](../../examples/report-comparison.animsmith.toml) |
| `walk-dirty.foot-height.svg` | foot-height figure of `walk-dirty.report.html` | |
| `walk.foot-height.svg` | foot-height figure of `walk.report.html` | |
| `foot-slide-before.foot-height.svg` | foot-height figure of `foot-slide-before.report.html` | |
| `foot-slide-after.foot-height.svg` | foot-height figure of `foot-slide-after.report.html` | |

The reports are rendered from the fixture directory, so each one names
its input by basename: no checkout path, no timestamp, and no absolute
path is embedded, and two machines produce the same bytes.

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

Only the foot-height figure is committed as a chart. Both walk fixtures
declare `movement_owner_xz = "gameplay"`, so their root-path figures are
the same stationary dot; a root-path chart earns a file once a fixture
whose root actually travels needs one.

## Hand-authored files

`icons/loop-pops.svg` and `icons/feet-slide.svg` are drawn here — a few
shapes and a CSS animation each, with a `prefers-reduced-motion` still
frame, an id that scopes their rules, and the same token-with-fallback
colours the generated charts use. They illustrate a symptom; they are not
evidence, and no measurement is taken from them.

## Releases

The reports are regenerated at release time together with the example
assets they are rendered from — see [RELEASING.md](../../RELEASING.md).
