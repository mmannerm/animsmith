# animsmith-fbx

`animsmith-fbx` loads FBX files into `animsmith-core`'s `Document`
model through the official `ufbx` bindings. It isolates the FBX parser
and bundled C build from the rest of the workspace; `animsmith-core`
stays file-format independent.

The loader normalizes FBX scenes to glTF-style conventions at parse
time: right-handed +Y-up axes, metres, transform-adjust space
conversion, helper nodes for geometric transforms, and scale-compensated
inheritance where needed. Animation stacks are baked into linear TRS
tracks so downstream checks operate on a plain skeleton-and-clip model.

```toml
[dependencies]
animsmith-core = "0.1"
animsmith-fbx = "0.1"
```

Use this crate directly when your Rust pipeline accepts FBX input. If
you only ingest glTF/GLB, depend on `animsmith-gltf` instead and avoid
the ufbx C build. This crate has no public feature flags; in the
`animsmith` CLI, FBX input and the `convert` command are behind the
default `fbx` feature and are omitted by `--no-default-features`.

More detail:

- [Embedding animsmith in a pipeline](https://github.com/mmannerm/animsmith/blob/main/docs/embedding.md)
- [CLI feature flags](https://github.com/mmannerm/animsmith/blob/main/docs/cli.md#feature-flags)
- [Workspace design](https://github.com/mmannerm/animsmith/blob/main/DESIGN.md)
