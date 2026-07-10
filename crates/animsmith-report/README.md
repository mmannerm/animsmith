# animsmith-report

## Overview

`animsmith-report` renders animsmith findings into a single offline HTML
report. It is the report-generation crate used by the CLI's `report`
command: callers provide the loaded `Document`, resolved rig roles, and
findings; the crate returns self-contained HTML.

The report embeds the pose-grid frames computed on the Rust side and
plays back exactly those frames in a small hand-written WebGL viewer.
There is no CDN, no three.js dependency, and no JavaScript resampling;
when a finding names a frame, the viewer scrubs to that judged frame.

## Usage

```toml
[dependencies]
animsmith-core = "0.1"
animsmith-report = "0.1"
```

```rust,no_run
fn write_report(
    doc: &animsmith_core::Document,
    roles: &animsmith_core::ResolvedRoles,
    findings: &[animsmith_core::Finding],
) -> std::io::Result<()> {
    let grids = animsmith_core::MetricGrids::new(doc);
    let html = animsmith_report::render(&grids, roles, findings, None);
    std::fs::write("report.html", html)
}
```

Pass the same `animsmith_core::MetricGrids` used for checks or
measurements to render those sampled frames without resampling the
clips.

## Feature Flags

This crate has no public feature flags. In the `animsmith` CLI, the
HTML report command is controlled by the default `report` feature and is
omitted by `--no-default-features`. The workspace MSRV is Rust 1.88.

## More Detail

- [API reference on docs.rs after publication](https://docs.rs/animsmith-report)
- [CLI report command](https://github.com/mmannerm/animsmith/blob/main/docs/cli.md#commands)
- [Embedding animsmith in a pipeline](https://github.com/mmannerm/animsmith/blob/main/docs/embedding.md)
- [Raw asset to game-ready pipeline scenarios](https://github.com/mmannerm/animsmith/blob/main/docs/pipeline-scenarios.md)
- [Workspace design](https://github.com/mmannerm/animsmith/blob/main/DESIGN.md)

## License

Licensed under either the MIT license or the Apache License, Version
2.0, at your option.

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in animsmith by you is licensed as MIT OR
Apache-2.0, without any additional terms or conditions.
