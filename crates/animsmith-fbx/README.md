# animsmith-fbx

## Overview

`animsmith-fbx` loads FBX files into `animsmith-core`'s `Document`
model through the official `ufbx` bindings. It isolates the FBX parser
and bundled C build from the rest of the workspace; `animsmith-core`
stays file-format independent.

The loader normalizes FBX scenes to glTF-style conventions at parse
time: right-handed +Y-up axes, metres, transform-adjust space
conversion, helper nodes for geometric transforms, and scale-compensated
inheritance where needed. Animation stacks are baked into linear TRS
tracks so downstream checks operate on a plain skeleton-and-clip model.

## Usage

```toml
[dependencies]
animsmith-core = "0.1"
animsmith-fbx = "0.1"
```

```rust,no_run
fn lint_fbx(
    path: &std::path::Path,
) -> Result<Vec<animsmith_core::Finding>, Box<dyn std::error::Error>> {
    let doc = animsmith_fbx::load(path)?;
    let roles = animsmith_core::detect_profile(&doc.skeleton).unwrap_or_default();
    let config = animsmith_core::Config::default();
    let grids = animsmith_core::MetricGrids::new(&doc);
    let ctx = animsmith_core::CheckCtx::new(&grids, &roles, &config);

    Ok(animsmith_core::run_checks(
        &ctx,
        &animsmith_core::all_checks(),
    ))
}
```

Use this crate directly when your Rust pipeline accepts FBX input. If
you only ingest glTF/GLB, depend on `animsmith-gltf` instead and avoid
the ufbx C build.

## Feature Flags

This crate has no public feature flags. In the
`animsmith` CLI, FBX input and the `convert` command are behind the
default `fbx` feature and are omitted by `--no-default-features`. The
workspace MSRV is Rust 1.88.

## More Detail

- [API reference on docs.rs after publication](https://docs.rs/animsmith-fbx)
- [Embedding animsmith in a pipeline](https://github.com/mmannerm/animsmith/blob/main/docs/embedding.md)
- [Raw asset to game-ready pipeline scenarios](https://github.com/mmannerm/animsmith/blob/main/docs/pipeline-scenarios.md)
- [CLI feature flags](https://github.com/mmannerm/animsmith/blob/main/docs/cli.md#feature-flags)
- [Workspace design](https://github.com/mmannerm/animsmith/blob/main/DESIGN.md)

## License

Licensed under either the MIT license or the Apache License, Version
2.0, at your option.

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in animsmith by you is licensed as MIT OR
Apache-2.0, without any additional terms or conditions.
