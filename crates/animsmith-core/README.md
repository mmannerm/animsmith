# animsmith-core

## Overview

`animsmith-core` is the embedding boundary for animsmith. It contains
the engine-agnostic data model, rig-role resolution, configuration
types, animation and mesh measurements, sampler, findings, and built-in
check catalog. It does not know about filesystems or file formats; pair
it with a loader crate such as `animsmith-gltf` or `animsmith-fbx` at the
edge of your pipeline.

Use it when a Rust asset pipeline already owns its sidecar format,
contract storage, and gate policy, but wants animsmith's animation
measurements and lint findings inside that pipeline instead of through
the CLI.

## Usage

```toml
[dependencies]
animsmith-core = "0.1"
animsmith-gltf = "0.1"
```

```rust,no_run
fn lint_document(doc: &animsmith_core::Document) -> Vec<animsmith_core::Finding> {
    let roles = animsmith_core::detect_profile(&doc.skeleton).unwrap_or_default();
    let config = animsmith_core::Config::default();
    let grids = animsmith_core::MetricGrids::new(doc);
    let ctx = animsmith_core::CheckCtx::new(&grids, &roles, &config);

    animsmith_core::run_checks(&ctx, &animsmith_core::all_checks())
}
```

Typical embedding flow:

1. Load a `Document` with `animsmith-gltf` or `animsmith-fbx`.
2. Resolve rig roles with `detect_profile` or explicit
   `ResolvedRoles::from_names`.
3. Build `Config` from your own contract format.
4. Create `MetricGrids`, call `measure::measure_document` if you need
   raw numbers, then build `CheckCtx::new` before `all_checks` and
   `run_checks`.
5. Map `Finding` severities into your pipeline's gate/reporting system.

## Feature Flags

This crate has no public feature flags. The workspace MSRV is Rust
1.88.

The crate re-exports `glam` as `animsmith_core::glam` because public
model types use `glam` vectors, quaternions, and matrices. The Rust API
is pre-1.0 and experimental; the most stable contracts are check ids,
exit-code conventions in the CLI, and the versioned JSON envelope.

## More Detail

- [Embedding animsmith in a pipeline](https://github.com/mmannerm/animsmith/blob/main/docs/embedding.md)
- [Workspace design](https://github.com/mmannerm/animsmith/blob/main/DESIGN.md)
- [CLI crate and examples](https://github.com/mmannerm/animsmith/tree/main/crates/animsmith)

## License

Licensed under either the MIT license or the Apache License, Version
2.0, at your option.

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in animsmith by you is licensed as MIT OR
Apache-2.0, without any additional terms or conditions.
