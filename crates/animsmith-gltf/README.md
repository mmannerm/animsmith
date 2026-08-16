# animsmith-gltf

> **Pre-1.0:** Breaking changes are expected between minor releases. Pin
> dependency versions and review the release notes before upgrading.

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

Primitive accessors must be fully readable within their declared dense and
sparse buffer views and the external, data-URI, or GLB bytes that actually
resolve. A short `POSITION`, index, or modeled attribute is a located
`LoadError::PrimitiveAccessorLayout`, never an empty-vector fallback; this
keeps authored empty geometry distinct from geometry the loader could not
read. Unreadable inverse binds remain explicit source-skeleton evidence rather
than a load refusal because that sidecar models their availability directly.

Animation sampler accessors are checked against the reader selected by their
slot and target property before decoding. Key times require `SCALAR`/`FLOAT`,
translation and scale require `VEC3`/`FLOAT`, and rotation retains all five
decodable glTF component encodings as `VEC4`, while morph-weight output retains
the same five as `SCALAR`; mismatches return a located `LoadError` rather than
panicking or reinterpreting same-sized bytes.

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

Callers that already compiled a core scale plan can use `rewrite_scale_plan`
to pass that same immutable plan through raw rewrite and artifact proof. The
operation-specific writer functions remain convenience wrappers that compile
and delegate to the same plan-taking path.

The rewrite operates on the source's own JSON tree and buffer bytes and never
routes through the normalized writer, so buffer bytes outside the converted
accessor ranges, every array index, and every unmodeled source payload survive
exactly. Node rest translations, node matrix translation columns, mesh
`POSITION`, translation sampler outputs (including both `CUBICSPLINE`
tangents), per-skin inverse-bind translation columns, and the corresponding
accessor `min`/`max` are converted when the declared factor changes lengths;
rotations, scales, normals, UVs, weights, key times, and morph weights are not.
Each accessor is converted once per
*unique* accessor index, so a `POSITION` shared by several primitives scales by
`q`, never `q^2`. JSON object key order and float spelling are not preserved:
output is deterministic, but it is not a minimal textual diff.
At factor one, the compiled plan marks length-bearing node fields, accessor
payloads, and authored bounds `PreserveExact`; the raw writer excludes them
from its write set. Parsed JSON numeric values therefore avoid a needless
`f32` narrowing, and accessor bytes remain authored, although ordinary JSON
reserialization can still canonicalize lexical spelling.

Camera, light, and extension length fields have no registered handler in the
shipped writer, and the preflight rejects those domains outright. Rest/bind
reparameterization uses the same compiled-plan adapter described below. Morph
`POSITION` deltas remain refused because the shared model cannot yet represent
and prove their write domain.

## Rest/Bind Hierarchy Reparameterization

`rewrite_rest_bind` removes one compensating inherited positive uniform scale
from a caller-selected raw source skin/root hierarchy while preserving its
world joint translations and orientations, sampled trajectories, and skinned
geometry. It derives node, animation, and inverse-bind multipliers from the
compiled plan's canonical raw topology and edits the source JSON and buffers
directly. Authored node rotations are outside this scale/length write set, so
artifact proof requires their parsed JSON values to remain exact.

`rewrite_scale_plan` is the common plan-taking writer boundary for both scale
operations. `prove_rewritten_artifact` and `prove_rewritten_rest_bind` reload
and prove the emitted container with independently derived component and
numeric expectations. Writer and proof share numeric-free raw identity and
shape bindings only. The [scale workflow] describes the supported source
boundary and coordinated CLI transaction; the [embedding guide] shows how to carry
one core plan through the adapter.

[scale workflow]: https://github.com/mmannerm/animsmith/blob/main/docs/scale.md
[embedding guide]: https://github.com/mmannerm/animsmith/blob/main/docs/embedding.md#scale-plan-and-proof-contracts

## Install

```toml
[dependencies]
animsmith-core = "0.2"
animsmith-gltf = "0.2"
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
- [Scale workflow](https://github.com/mmannerm/animsmith/blob/main/docs/scale.md)
- [Raw asset to game-ready pipeline scenarios](https://github.com/mmannerm/animsmith/blob/main/docs/pipeline-scenarios.md)
- [CLI reference](https://github.com/mmannerm/animsmith/blob/main/docs/cli.md)
- [Workspace design](https://github.com/mmannerm/animsmith/blob/main/DESIGN.md)

## License

Licensed under either the MIT license or the Apache License, Version
2.0, at your option.

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in animsmith by you is licensed as MIT OR
Apache-2.0, without any additional terms or conditions.
