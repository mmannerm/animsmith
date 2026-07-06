# animsmith-gltf

## Overview

`animsmith-gltf` loads `.gltf` and `.glb` files into
`animsmith-core`'s `Document` model. It is the glTF/GLB boundary for
embedding animsmith in a Rust pipeline: this crate handles container
ingestion, while `animsmith-core` owns checks, measurements, config, and
findings.

Values are preserved as authored. The loader does not renormalize
quaternions, resample tracks, or clean data on the way in, so the
mechanical checks judge the same animation data that shipped in the
file. Buffers support GLB BIN chunks, `data:` URIs, and sibling external
files; unsafe external-buffer paths are rejected.

`load` also fills `Document::assets` with the file's geometry — meshes
(triangle lists), skins (joints + inverse bind matrices), and
factor-only materials with embedded base-color textures — in the same
single call, matching `animsmith-fbx`. Consumers that judge only
animation ignore `assets`; `measure` reports mesh-level measurements
from it and `convert` carries it through.

## Usage

```toml
[dependencies]
animsmith-core = "0.1"
animsmith-gltf = "0.1"
```

```rust,no_run
fn lint_clip(
    path: &std::path::Path,
) -> Result<Vec<animsmith_core::Finding>, Box<dyn std::error::Error>> {
    let doc = animsmith_gltf::load(path)?;
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

## Feature Flags

This crate has no public feature flags. In the `animsmith` CLI, glTF
inspect/measure/lint/transform/fix/diff support is always available,
including in `--no-default-features` builds. The workspace MSRV is Rust
1.88.

## More Detail

- [Embedding animsmith in a pipeline](https://github.com/mmannerm/animsmith/blob/main/docs/embedding.md)
- [CLI reference](https://github.com/mmannerm/animsmith/blob/main/docs/cli.md)
- [Workspace design](https://github.com/mmannerm/animsmith/blob/main/DESIGN.md)

## License

Licensed under either the MIT license or the Apache License, Version
2.0, at your option.

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in animsmith by you is licensed as MIT OR
Apache-2.0, without any additional terms or conditions.
