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

## Importer-sensitive source facts

`load_source` and `load_source_bytes` return an immutable `LoadedSource` that
binds the normalized `Document` to bounded source evidence from the same ufbx
parse and the SHA-256/byte count of the exact primary bytes. The legacy `load`
and `load_bytes` APIs remain available when only the normalized document is
needed; consuming a `LoadedSource` as a document deliberately discards its
source sidecar.

The V1 FBX projection records ufbx's effective source unit, signed coordinate
basis, finite frame rate, animation-stack identity and parser-resolved time
range, raw layer/property/component presence, aggregate custom or unmodeled
domains, and texture/video/cache resource declarations. Unit and basis facts
come from `scene.settings.unit_meters` and `scene.settings.axes`;
`OriginalUnitScaleFactor` and `OriginalUpAxis` remain advisory and are not
promoted into effective evidence. Stack ranges remain separate from baked
clips whose start may be trimmed to zero.

Animation property rows come from ufbx layer bindings, not baked `Document`
tracks. Local translation, rotation, and scale bindings are marked `Baked`;
authored interpolation, keys, and tangents are explicitly unavailable after
the bake boundary. Other source properties remain unsupported instead of being
reconstructed from normalized samples.

Resource rows retain only bounded safe relative spellings, preferring ufbx's
`relative_filename` and then its parser filename field. Values that are not
safe relative declarations are classified without retaining their spelling;
the parser-resolved absolute-path field contributes only a boolean declaration
presence marker for a redacted `Absolute` row. It is never retained, emitted,
normalized, turned into a host path, or opened. Texture and video declarations
that refer to the same spelling remain separate reference rows.

## Dependency closure and external resources

Each `LoadedSource` also carries the core-owned V1 dependency closure. Path
loaders use the primary file's parent as their trusted resource root. The
byte-only loaders deliberately perform no external resource I/O; callers that
have separately captured bytes and a trusted root must use
`load_source_bytes_with_resource_root`,
`load_scale_source_bytes_with_resource_root`, or
`load_bytes_with_resource_root`.

ufbx external-file loading is disabled. The FBX loader processes the retained
texture, video, and cache declaration prefix once in deterministic order.
Accepted relative declarations are normalized as parser-relative keys and each
distinct key is opened and hashed once below the supplied root. The root itself
and every key-derived component/final target must be non-symlink; unsafe,
missing, unreadable, or budget-exceeded declarations become typed closure
outcomes without host path or error text. The shared V1 limits cap one external
read at 64 MiB, distinct captured bytes at 256 MiB, and distinct keys at 1,024;
the first N+1 boundary retains only its deterministic prefix. ufbx's
`texture_files` list is a deduplicated view derived from the represented
texture rows, so it does not independently make closure coverage partial.
Audio clips remain conservatively unmodeled because they have no V1 raw
resource row.

The exact captured bytes are reused for optional PNG/JPEG `TextureAsset`s;
there is no post-load reread or path fallback. A separate 256 MiB FBX asset
materialization ceiling prevents material aliases from multiplying retained
texture vectors. An optional texture past that ceiling is absent from the
normalized document while the closure identity remains based on the one
captured resource.

Projection limits bound only this added evidence walk and retained rows/text.
V1 retains at most 65,536 observation rows, 4,096 clip identities, 4,096
resource declarations, 4,096 bytes per source text, and 8 MiB of aggregate
text; the shared traversal-depth ceiling is 128. The FBX resource projection
merges only the texture, video, and cache typed lists, so unrelated scene
elements do not add to that evidence walk. At N+1 the deterministic prefix is
marked partial while loading still succeeds; the limits do not certify ufbx or
animation baking as globally memory-bounded.

## Scale capability inventory

`load_scale_source` and `load_scale_source_bytes` return the normalized
`Document` together with a deterministic `FbxScaleCapabilityInventory` from
the same ufbx parse. The inventory gives every current DESIGN.md Appendix D.4
domain an explicit status and records the ingestion boundary: advisory
`Original*` coordinate fields and target units/axes,
adjusted transforms, helper nodes and inherit-mode compensation, baked takes
and discarded authored curve keys (including unsupported stackless curves),
generated normals, cluster-derived bind
matrices, influence truncation and renormalization, triangulation and exact-bit
welding, omitted point/line faces and zero-face mesh definitions (with stable
source identities), authored face/edge payloads,
uninstanced mesh definitions,
unsupported deformers/payloads, external resources, and stable ufbx source
identities. Shared source geometry remains one normalized mesh definition with
multiple node instances rather than duplicate definitions. Invalid or
unrepresentable influences have an explicit rejected
count. Only successfully projected cluster binds count toward bone-convenience
overwrites. The document also carries the documented source-node/source-skin
identity projection in normalized ufbx order when every joint slot is
representable. A missing cluster bone downgrades that generic projection to
`Unavailable`, and an unreadable bind declaration retains no shifted matrix
prefix. Normalized mesh definitions and source-skin attachments use the same
stable ufbx mesh identity even when an earlier source mesh emits no primitive.
`Complete` is coverage of the adjusted/compensated projection, not a
claim that raw FBX transform members or object payloads were preserved.

`capability_facts` remains the deliberately unsupported generic projection.
`rest_bind_capability_facts` is its narrower companion for `animsmith scale
rest-bind`: it accepts only a complete normalized ufbx inventory whose
scale-bearing domains are proven. The source-aware
`rest_bind_capability_facts_for_source` additionally distinguishes
parser-known texture-file linkage from genuinely unknown source elements. It
may admit bounded texture/video declarations and user-defined properties
because the normalized GLB bridge does not use either as rest/bind state. On
the same-load proof path it may also admit enumerated scale-invariant
conversion-fidelity facts: authored color/tangent/bitangent/UV, face/edge,
crease/subdivision payload omitted by the normalized bridge; influence
truncation, rejection, and renormalization when effective normalized coverage
remains complete; polygon triangulation; and exact-bit welding. Their public
inventory counters and unsupported domain status remain unchanged as evidence.
Missing effective influences, omitted point/line geometry, empty or uninstanced
meshes, unsupported skinning/deformers, incomplete binds or normals, and any
unclassified payload remain fail-closed. The same-load proof path also admits
exactly ufbx's marker, LOD-group, stereo-camera, camera-switcher, and
display-layer typed lists: those records do not supply hierarchy transforms,
skin binds, animation tracks, or geometry to that bridge. Display layers
contain only node membership and editor visibility/freeze/color state. Shader
and binding-table records are admitted on the same basis.
A BindPose is admitted only when it covers every joint of each skin it touches
and its converted rows are finite, unambiguous, and agree component-wise under
the fixed rest/bind tolerance with the converted cluster bind or node
rest-world matrix already consumed by the bridge; no Pose remains required.
Incomplete, ambiguous, non-finite, or mismatching BindPoses remain distinct
fail-closed kinds. All of these rows remain in the raw unmodeled-element
aggregate. Every other unmodeled typed list remains fail-closed, and a refusal
reports the exact nonzero kind counts instead of only the aggregate total. The
immutable
inventory, raw-source facts, and dependency closure still report all admitted
declarations honestly. The inventory-only API remains conservative when it
cannot make that same-load distinction. Refusals identify the exact coverage
domain, semantic row, or counter that failed. The accepted source is staged as
a private GLB, rewritten and proven as the exact emitted GLB, then atomically
published as a `.glb` artifact/evidence pair. Whole-document FBX scaling
remains disabled. No API claims raw FBX bytes, raw object properties, authored
curve keys, camera or marker behavior, material/texture assignment, or source
vertex identity are preserved.

## Install

```toml
[dependencies]
animsmith-core = "0.12"
animsmith-fbx = "0.12"
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
