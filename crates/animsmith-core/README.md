# animsmith-core

`animsmith-core` is the embedding boundary for animsmith. It contains
the engine-agnostic data model, rig-role resolution, configuration
types, measurements, sampler, findings, and built-in check catalog. It
does not know about filesystems or file formats; pair it with a loader
crate such as `animsmith-gltf` or `animsmith-fbx` at the edge of your
pipeline.

Use it when a Rust asset pipeline already owns its sidecar format,
contract storage, and gate policy, but wants animsmith's animation
measurements and lint findings inside that pipeline instead of through
the CLI.

```toml
[dependencies]
animsmith-core = "0.1"
animsmith-gltf = "0.1"
```

Typical embedding flow:

1. Load a `Document` with `animsmith-gltf` or `animsmith-fbx`.
2. Resolve rig roles with `detect_profile` or explicit
   `ResolvedRoles::from_names`.
3. Build `Config` from your own contract format.
4. Call `measure::measure_document`, then `CheckCtx::new`,
   `all_checks`, and `run_checks`.
5. Map `Finding` severities into your pipeline's gate/reporting system.

The crate re-exports `glam` as `animsmith_core::glam` because public
model types use `glam` vectors, quaternions, and matrices. The Rust API
is pre-1.0 and experimental; the most stable contracts are check ids,
exit-code conventions in the CLI, and the versioned JSON envelope.

More detail:

- [Embedding animsmith in a pipeline](https://github.com/mmannerm/animsmith/blob/main/docs/embedding.md)
- [Workspace design](https://github.com/mmannerm/animsmith/blob/main/DESIGN.md)
- [CLI crate and examples](https://github.com/mmannerm/animsmith/tree/main/crates/animsmith)
