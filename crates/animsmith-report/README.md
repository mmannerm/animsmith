# animsmith-report

`animsmith-report` renders animsmith findings into a single offline HTML
report. It is the report-generation crate used by the CLI's `report`
command: callers provide the loaded `Document`, resolved rig roles, and
findings; the crate returns self-contained HTML.

The report embeds the pose-grid frames computed on the Rust side and
plays back exactly those frames in a small hand-written WebGL viewer.
There is no CDN, no three.js dependency, and no JavaScript resampling;
when a finding names a frame, the viewer scrubs to that judged frame.

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
    let html = animsmith_report::render(doc, roles, findings, None);
    std::fs::write("report.html", html)
}
```

This crate has no public feature flags. In the `animsmith` CLI, the
HTML report command is controlled by the default `report` feature and is
omitted by `--no-default-features`.

More detail:

- [CLI report command](https://github.com/mmannerm/animsmith/blob/main/docs/cli.md#commands)
- [Embedding animsmith in a pipeline](https://github.com/mmannerm/animsmith/blob/main/docs/embedding.md)
- [Workspace design](https://github.com/mmannerm/animsmith/blob/main/DESIGN.md)
