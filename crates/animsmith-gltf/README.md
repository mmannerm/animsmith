# animsmith-gltf

> **Pre-1.0:** Breaking changes are expected between minor releases. Pin
> dependency versions and review the release notes before upgrading.

## Overview

`animsmith-gltf` loads `.gltf` and `.glb` files into
`animsmith-core`'s `Document` model. `load_source` and `load_source_bytes`
add an immutable, bounded raw-source-facts view bound to the exact primary
bytes; the longstanding `load` and `load_bytes` functions remain the
document-only convenience surface. It is the glTF/GLB boundary for
embedding animsmith in a Rust pipeline: this crate handles container
ingestion, while `animsmith-core` owns checks, measurements, config, and
findings.

Path loading roots external resources at the source file's parent. Captured
byte loading deliberately has no ambient-current-directory fallback: use
`load_bytes_with_resource_root` or `load_source_bytes_with_resource_root`
when the input declares a safe relative external buffer or image. Unsafe
spellings are refused before any open. The caller-supplied root is a capability
rather than source evidence; rooted loading accepts only safe normalized
relative keys, rejects a symlinked final root and every locator-derived
symlink component before opening them, and never includes host paths in the
closure identity or source-controlled diagnostics. Ancestors of the explicitly
supplied root remain part of the caller's capability path.

Values are preserved as authored. The loader does not renormalize
quaternions, resample tracks, or clean data on the way in, so the
mechanical checks judge the same animation data that shipped in the
file. Buffers support GLB BIN chunks, `data:` URIs, and sibling external
files; unsafe external-buffer paths are rejected.

The raw facts identify JSON glTF versus GLB from the container bytes, record
glTF's format-defined metre unit and signed coordinate basis, prove that glTF
declares no FPS or take range, and retain bounded source-order animation,
channel/accessor, extension, and resource-declaration evidence. Resource
locators are classified without opening or hashing additional dependencies;
unsafe, remote, malformed, data-URI, and oversized spellings are redacted.
The facts are importer evidence, not target-engine support policy. The
`LoadedSource` companion dependency closure records the exact primary identity
plus each source-order buffer/image declaration's primary, captured external,
refused, or unavailable mapping. It hashes each accepted external key from
the same bounded read used by the loader and deduplicates aliases. Its fixed
limits cover declarations, normalized keys, open/hash bytes, and alias probes;
an N+1 limit produces typed partial evidence rather than resuming capture.
Because V1 does not inspect extension payloads for additional locators, any
declared extension also keeps dependency-closure coverage partial.

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
Integer `TEXCOORD_0` and `WEIGHTS_0` accessors must also declare
`normalized: true`; otherwise the upstream reader would rescale their values
despite the missing declaration, so the loader returns a located
`LoadError::PrimitiveEncoding` instead. Normalized `UNSIGNED_BYTE` and
`UNSIGNED_SHORT` attributes keep their decoded float values, as do `FLOAT`
attributes.

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
reparameterization uses the same compiled-plan adapter described below.
Whole-document conversion handles supported raw glTF `POSITION` morph deltas
in the exact-source writer and artifact proof without adding them to the
shared normalized model. Static JSON weights retain their numeric values and
animated weight accessor payloads remain byte-exact; unsupported morph
semantics and every rest/bind morph payload remain refused.

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

Character-assembly recipe v5 reuses these same preflight, plan-taking rewrite,
reload, and proof boundaries. When its optional rest/bind block is active, the
base and every separately supplied clip must be glTF/GLB and pass complete raw
capability preflight before any keys are remapped. FBX assembly remains
available when the block is omitted; the block itself does not infer support
from the normalized document.

Recipe v6 leaves that immutable v5 contract unchanged and additionally admits
the narrow inventory-complete FBX rest/bind subset. Each admitted FBX input is
captured and normalized/baked by the FBX frontend, serialized into a private
GLB stage, then passed through these same glTF plan-taking rewrite and proof
boundaries. Its evidence identifies the FBX capability inventory and private
stage explicitly; it does not claim raw FBX span or authored-curve preservation.

Recipe v7 leaves v6 immutable and replaces its unobservable cross-file source
indices with one exact `root_node_name`. Every captured base and clip must map
that name to exactly one normalized source node and exactly one source skin
whose joint set contains it. The resulting per-input indices enter the same
plan-taking writer, private FBX stage, and proof boundaries; v7 evidence records
both the declared name and every resolved name/index tuple.

[scale workflow]: https://github.com/mmannerm/animsmith/blob/main/docs/scale.md
[embedding guide]: https://github.com/mmannerm/animsmith/blob/main/docs/embedding.md#scale-plan-and-proof-contracts

## Install

```toml
[dependencies]
animsmith-core = "0.3"
animsmith-gltf = "0.3"
```

The compiling load/check and repair examples live in the crate-level API
documentation.

## Feature Flags

This crate has no public feature flags. In the `animsmith` CLI, glTF
inspect/measure/lint/transform/fix/diff support is always available,
including in `--no-default-features` builds. The workspace MSRV is
Rust 1.88.

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
