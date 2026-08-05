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
PBR materials with embedded base-color, normal, metallic-roughness, and occlusion textures — in the same
single call, matching `animsmith-fbx`. Consumers that judge only
animation ignore `assets`; `measure` reports mesh-level measurements
from it and `convert` carries it through.

For glTF/GLB measurement, the loader also provides a source-resource sidecar:
source-order material definitions, semantic texture bindings, texture-to-image
identity, and bounded image metadata. Its complete core binding domain is
`base_color`, `normal`, `metallic_roughness`, `occlusion`, then `emissive`;
extension-defined texture slots are outside that claim. It distinguishes
declared MIME from the container detected in source bytes and decoded
color/channel data; malformed or unsupported image data remains explicit
unavailable evidence rather than a repair request. This sidecar is descriptive
only and is not a writer or conversion preservation, image acceptance,
resize/transcode, or recipe-authority promise.

The glTF source-skeleton sidecar likewise preserves source node and skin
indices independently from the parent-before-child sampling skeleton. It keeps
authored local rest representations, skin-slot joint order, inverse-bind
accessor availability, and source-node skin attachments so measurement can
report facts without choosing a retargeting profile or inferring absent binds.

For measurement, the loader also records whether each primitive declares
secondary `JOINTS_n` or `WEIGHTS_n` attributes. Mesh measurements retain an
individual-primitive mismatch signal when complementary sides occur on
different primitives. This is presence metadata only: secondary values are not
evaluated as skinning influences or preserved by the writer. See the
machine-readable output contract for the exact boundary.

## Raw Scale Preflight

`preflight_scale_source` and `preflight_scale_source_bytes` are read-only
foundations for the scale operations specified in `DESIGN.md` Appendix D. A
normalized `Document` cannot prove that a source lacked morph targets,
cameras, custom extension data, secondary attributes, or another domain the
current model does not retain. Treating an empty normalized field as proof of
absence would let a future rewrite silently drop or incorrectly scale authored
data.

The preflight therefore inspects the original glTF JSON and accessor layouts,
including GPU-instancing declarations, and builds a deterministic typed
manifest before deciding support. A fully covered source returns that manifest
with the exact captured top-level and resolved buffer bytes. Unsupported
domains return the complete inventory alongside all source-indexed violations
before a scale plan, candidate, or output exists. This API does not convert
units, alter rest/bind data, infer a scale factor, or expose a mutation method.

## Whole-Document Linear-Unit Rewrite

`rewrite_linear_units` converts every length in a preflighted source by a
caller-declared finite factor `q > 0`, and never infers that factor.
`capability_facts` projects a `GltfCapabilityManifest` down to the
format-neutral `ScaleCapabilityFacts` that `animsmith_core::scale::plan_scale`
consumes, and `prove_rewritten_artifact` checks the emitted bytes against the
claims a normalized `Document` cannot carry.

The rewrite operates on the source's own JSON tree and buffer bytes and never
routes through the normalized writer, so buffer bytes outside the converted
accessor ranges, every array index, and every unmodeled source payload survive
exactly. Node rest translations, node matrix translation columns, mesh
`POSITION`, translation sampler outputs (including both `CUBICSPLINE`
tangents), per-skin inverse-bind translation columns, and the corresponding
accessor `min`/`max` are converted; rotations, scales, normals, UVs, weights,
key times, and morph weights are not. Each accessor is converted once per
*unique* accessor index, so a `POSITION` shared by several primitives scales by
`q`, never `q^2`. JSON object key order and float spelling are not preserved:
output is deterministic, but it is not a minimal textual diff.

Camera, light, and extension length fields have no registered handler in this
slice, and the preflight rejects those domains outright. Rest/bind
reparameterization, morph `POSITION` deltas, and CLI/evidence publication
remain separate dependency-ordered implementation slices.

## Install

```toml
[dependencies]
animsmith-core = "0.1"
animsmith-gltf = "0.1"
```

The compiling load/check and repair examples live in the crate-level API
documentation.

## Feature Flags

This crate has no public feature flags. In the `animsmith` CLI, glTF
inspect/measure/lint/transform/fix/diff support is always available,
including in `--no-default-features` builds. The workspace MSRV is Rust
1.88.

## More Detail

- [API reference on docs.rs](https://docs.rs/animsmith-gltf)
- [Embedding guide](https://github.com/mmannerm/animsmith/blob/main/docs/embedding.md)
- [Raw asset to game-ready pipeline scenarios](https://github.com/mmannerm/animsmith/blob/main/docs/pipeline-scenarios.md)
- [CLI reference](https://github.com/mmannerm/animsmith/blob/main/docs/cli.md)
- [Workspace design](https://github.com/mmannerm/animsmith/blob/main/DESIGN.md)

## License

Licensed under either the MIT license or the Apache License, Version
2.0, at your option.

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in animsmith by you is licensed as MIT OR
Apache-2.0, without any additional terms or conditions.
