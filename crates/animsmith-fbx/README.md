# animsmith-fbx

> **Pre-1.0:** Breaking changes are expected between minor releases. Pin
> dependency versions and review the release notes before upgrading.

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
Scene assets carry triangulated meshes, skins, factor-only materials, and
linked or embedded PNG/JPEG base-color and normal textures into the shared
format-independent model.

## Scale capability inventory

`load_scale_source` and `load_scale_source_bytes` return the normalized
`Document` together with a deterministic `FbxScaleCapabilityInventory` from
the same ufbx parse. The inventory gives every current DESIGN.md Appendix D.4
domain an explicit status and records the ingestion boundary: original/target units and axes,
adjusted transforms, helper nodes and inherit-mode compensation, baked takes
and discarded authored curve keys, generated normals, cluster-derived bind
matrices, influence truncation and renormalization, triangulation and exact-bit
welding, unsupported deformers/payloads, external resources, and stable ufbx
source identities. Invalid or unrepresentable influences have an explicit
rejected count. The document also carries the documented source-node/source-skin
identity projection in normalized ufbx order when every joint slot is
representable. A missing cluster bone downgrades that generic projection to
`Unavailable`, and an unreadable bind declaration retains no shifted matrix
prefix. `Complete` is coverage of the adjusted/compensated projection, not a
claim that raw FBX transform members or object payloads were preserved.

`capability_facts` projects the inventory into `animsmith-core`'s
format-neutral scale gate. The projection is deliberately unsupported in this
inventory-only slice: FBX loading has already normalized transform/unit state,
rebuilt geometry, and baked curves, while ufbx exposes no raw payload spans for
artifact-preservation proof. Neither rest/bind nor whole-document scaling is
enabled. No API here claims raw FBX bytes, raw object properties, authored
curve keys, or source vertex identity are preserved.

## Install

```toml
[dependencies]
animsmith-core = "0.2"
animsmith-fbx = "0.2"
```

The compiling load/check example lives in the crate-level API documentation.

Use this crate directly when your Rust pipeline accepts FBX input. If
you only ingest glTF/GLB, depend on `animsmith-gltf` instead and avoid
the ufbx C build.

## Feature Flags

This crate has no public feature flags. In the
`animsmith` CLI, FBX input and the `convert` command are behind the
default `fbx` feature and are omitted by `--no-default-features`. The
workspace MSRV is Rust 1.88.

## More Detail

- [API reference on docs.rs](https://docs.rs/animsmith-fbx)
- [Embedding guide](https://github.com/mmannerm/animsmith/blob/main/docs/embedding.md)
- [Raw asset to game-ready pipeline scenarios](https://github.com/mmannerm/animsmith/blob/main/docs/pipeline-scenarios.md)
- [CLI feature flags](https://github.com/mmannerm/animsmith/blob/main/docs/cli.md#feature-flags)
- [Workspace design](https://github.com/mmannerm/animsmith/blob/main/DESIGN.md)

## License

Licensed under either the MIT license or the Apache License, Version
2.0, at your option.

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in animsmith by you is licensed as MIT OR
Apache-2.0, without any additional terms or conditions.
