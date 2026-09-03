# animsmith-core

> **Pre-1.0:** Breaking changes are expected between minor releases. Pin
> dependency versions and review the release notes before upgrading.

## Overview

`animsmith-core` is animsmith's engine-agnostic library crate. This
README is a compact crates.io and repository index; the crate-root
rustdoc owns the embedding flow, API status, extension points, and
panic/error contracts.

## Raw Source Facts and Dependency Closure

The core crate owns a bounded, format-neutral V1 vocabulary for importer-
sensitive source evidence. Format crates can return an immutable
`LoadedSource` that binds those facts and a bounded dependency closure to the
exact primary bytes while lending read-only access to the normalized
`Document`; `into_document()` deliberately discards both sidecars. The facts
reuse the existing `SourceSkeletonAssets`
projection instead of copying source node/skin authority. They describe
AnimSmith loader evidence and availability, not engine support policy,
target-importer policy, or a scale operation's private proof ledger. Complete,
partial, and unavailable row coverage is explicit; a partial prefix proves
presence only. V1 limits projection to 65,536 enumerable rows,
4,096 clips/takes, 4,096 resource declarations, 4,096 bytes per retained
source string, 8 MiB of retained source strings, and traversal depth 128.
Budget N+1 preserves the deterministic prefix and marks the affected set
partial without turning a successful legacy load into an error.

`DependencyClosureV1` maps that raw declaration prefix to the primary input,
safe normalized external keys with exact byte identities, or typed
refusal/unavailability. Each reference also carries its kind-derived,
format-neutral loader-essential, nonessential, or target-only purpose without
making a target-engine support claim. Only complete raw coverage with an
identity for every declaration produces a closure identity. Core performs no filesystem I/O;
format loaders capture sidecar bytes once from a trusted root while loading,
never from the process working directory. V1 bounds capture to 4,096
declarations, 1,024 distinct external keys, 4,096 bytes and 128 components per
key, 8 MiB aggregate normalization input, 64 MiB per external resource, and
256 MiB in aggregate. Unsafe spellings and host paths are not retained.

## Shared runtime-node policy

`Config::runtime_nodes` is the engine-neutral selector authority for
attachments, sockets, IK targets, and other runtime-facing source nodes.
`rest-world-scale` consumes it; its older per-check `node_selectors` field is a
compatibility alias and cannot be declared at the same time. An absent field
or empty list means no policy. This added public field is an intentional
pre-1.0 struct-literal break: exhaustive `Config` literals must add it or use
`..Config::default()`.

## Install

```toml
[dependencies]
animsmith-core = "0.12"
animsmith-gltf = "0.12"
```

## Feature Flags

- `fixtures` (off by default) — exposes `animsmith_core::fixtures`, the
  analytic walk-cycle and scale-reference builders shared with animsmith's own
  tests and example-asset generator. Adds no dependency: the walk-cycle builder
  takes its sine as a parameter, while the scale-reference builder is test
  support rather than a production rewrite path. Internal to the animsmith
  workspace and **not** part of the crate's stable API; downstream code should
  not depend on it.

The workspace MSRV is Rust 1.88.

## Additional Skin-Influence Evidence

`Primitive::additional_influence_sets` carries independent presence metadata
for secondary glTF `JOINTS_n` and `WEIGHTS_n` attributes. The measurement
contract aggregates that metadata per source mesh and retains whether either
side was unpaired on an individual primitive, so complementary declarations on
different primitives are not reported as a clean pair. It intentionally does
not retain secondary per-vertex payloads or change the primary four-influence
skinning semantics.

## Material and Image Measurement Evidence

`AssetMeasurements` records source-order material definitions, semantic
texture bindings, texture-to-image identities, and bounded image inspection
metadata when a loader supplies a source-resource sidecar. Its explicit
`material_resource_coverage` distinguishes complete glTF/GLB evidence from an
unavailable source-resource view in another loader. For glTF/GLB, complete is
scoped to the documented core slots `base_color`, `normal`,
`metallic_roughness`, `occlusion`, and `emissive`; extension-defined texture
slots are not implied. Image records preserve
declared MIME separately from a detected container and decoded dimensions,
channel count, and color type; unavailable images instead carry a reason.
This is descriptive source evidence, not an image acceptance, repair, resize,
transcode, writer-preservation, conversion, or material-recipe policy.

Mesh-definition evidence also includes optional mesh-local finite-position
bounds and an arithmetic vertex centroid. The centroid is descriptive source
geometry evidence, not a center of mass, placement policy, or repaired pivot.

## Skeleton Rest-Pose Evidence

When a loader supplies source skeleton identity, `AssetMeasurements` also
records source-order nodes, per-skin joint lists and inverse-bind declaration
state, and finite derived bind-domain matrices with typed unavailability. glTF
retains exact accessor values; FBX records documented ufbx-normalized cluster
bind projections and does not claim raw FBX payload preservation.
Node-local TRS/matrix data, rest-world transforms, and mesh-local bind data
remain distinct coordinate domains. This is generic evidence only: embedders
choose required joints, comparison tolerances, canonical roots, and any
retargeting or delivery policy.

## Static Mesh Transform Baking

Embedders can opt into
`animsmith_core::bake_static_mesh_transforms` to bake accumulated static
rest transforms into mesh-local positions and inverse-transpose normalized
normals. The operation returns a canonical identity-root document plus
deterministic per-instance evidence. It validates the input fields the static
operation consumes before constructing output, while raw source-projection
evidence that it discards is intentionally irrelevant. It fails closed for
animation, skinning, ambiguous mesh
instancing, malformed data, reflections, and ill-conditioned transforms.
Model-supported material factors and embedded base-color, normal, metallic-roughness, and occlusion textures
are preserved.

## Character Assembly Helpers

`animsmith_core::assembly` provides exact-name clip remapping onto an
authoritative skeleton, named-bone track stripping, optional rest-pose channel
completion for all or an explicit base-bone selection, deterministic quaternion hemisphere cleanup, and endpoint-key
removal. It also plans and transactionally applies exact-name node-subtree
projection while refusing live animation, mesh, and skin references. It rejects ambiguous or missing referenced names rather than guessing
at a retargeting relationship. For a final-pose hold, use the existing
`animsmith_core::transform::hold_extend` helper.

For recipe-v4 rest/bind integration, `animsmith_core::scale` also builds and
compares versioned assembly basis records before remapping. Those records bind
named parent topology, target paths, rests and orientations, helper layout,
coordinate convention, effective target factors, explicit operation selectors,
and factor. The same compiled plan supplies the pre-remap translation and
`CUBICSPLINE` tangent factors; assembly does not own a second scale algorithm.

## Foot-Cycle Map Planning

`animsmith_core::foot_cycle` owns the strict format-neutral V1 declaration and
pure map planner for a manifest-declared in-place gait ring. It binds canonical
contact fragments to exact manifest clip witnesses, consumes independently
measured typed Root/Hips evidence bound to the same artifact, dependency
closure, and collection source/take for the inclusive 0.01 m / 1 degree in-place
gate while retaining signed endpoint X/Z and signed accumulated yaw facts,
validates the known stance-detector provenance and alternating bilateral
support topology under one exact ring-wide detector threshold, bounds aggregate
canonicalization work, and returns one slope-bounded, endpoint-preserving contact
`time_warp` operation per member. The reference member's authored boundary
phases are canonical; the planner never guesses correspondence or rotates phase
zero. Its declaration and returned plan retain one required identity-bound
proof policy with no defaults or cross-member merge. The CLI crate owns the
bounded TOML reader. Asset loading, root-motion
measurement, animation track mutation, transformed-fragment proof, and
generation-directory publication are deliberately not part of this planner slice.
`animsmith_core::time_warp_clip_v1` is the next pure boundary: after a host has
bound a member plan to the selected source, it produces a validated cloned
LINEAR/STEP clip candidate and conservatively retains only representation-exact
CUBICSPLINE tracks. This Clip-only seam validates track-local shape; its host
must have already bound every track bone index to the selected, validated
skeleton. `animsmith_core::preflight_time_warp_clip_v1` exposes the same
validation path with exact per-candidate name/track/key/value/storage-byte
counts and a conservative V1 work charge, without allocating the candidate, so
a host can enforce a checked aggregate batch budget first. The CLI crate has a
private source-binding adapter that applies that batch preflight, prepares those
candidates, and uses
`transform_contact_support_detector_extension_time_warp_v1` for the exact known
stance extension, but it defers `transform_contact_fragment_v1` until a later
serializer can supply exact captured output artifact and closure identities. It
adds no CLI command and performs no proof or publication. See the
[collection contracts](https://github.com/mmannerm/animsmith/blob/main/docs/collection-contracts.md#foot-cycle-parameterization-v1-18-planner-slice).

## Constant-Track Pruning

`animsmith_core::transform::prune_constant_tracks` is an opt-in mechanical
edit for redundant multi-key TRS tracks. It shares the built-in
`constant-track` classifier, tests cumulative removals on the original clip's
sample grid, and returns authored-order removed and retained records. Callers
provide any bone IDs whose authored channels must remain; the function also
refuses changes that alter sampled local TRS or model-space position/rotation,
cannot be sampled safely, or would leave the clip without a writable track.

## Skinned Bind-Pose Canonicalization

`animsmith_core::canonicalize_skinned_bind_pose` prepares an unanimated,
skinned character base for a right-handed, Y-up metre delivery space. The
caller declares the source-to-target affine coordinate transform; the operation
rejects reflections, shear, non-uniform unit conversion, malformed skin data,
and inverse binds that disagree with the input rest pose. It emits one identity
scene root, private bind-world mesh copies with inverse-transpose normalized
normals, remapped joints, and regenerated inverse bind matrices. Optional
ground-and-centre placement uses the complete converted bind-pose bounds in a
deterministic source-node order.

## Scale Planning and Proof

`animsmith_core::scale` is the format-neutral boundary for two explicitly
selected operations: whole-document linear-unit conversion and rest/bind
hierarchy reparameterization. `plan_scale` compiles one immutable typed ledger
from a validated `Document` and format capability facts. A format frontend
rewrites its exact source representation, reloads those bytes, wraps the result
with `ScaleCandidate::from_document`, and calls `prove_scale`, which derives
its expectations independently from the writer.

Core deliberately exposes no production candidate builder and performs no file
I/O or publication. The analytic builder under the non-default `fixtures`
feature is test/calibration support only. See the
[scale workflow](https://github.com/mmannerm/animsmith/blob/main/docs/scale.md),
[embedding guide](https://github.com/mmannerm/animsmith/blob/main/docs/embedding.md#scale-plan-and-proof-contracts),
and Appendix D of the
[workspace design](https://github.com/mmannerm/animsmith/blob/main/DESIGN.md#appendix-d--decision-record-skinned-restbind-scale-canonicalization).

## More Details

- [API reference on docs.rs](https://docs.rs/animsmith-core)
- [Embedding guide](https://github.com/mmannerm/animsmith/blob/main/docs/embedding.md)
- [Raw asset to game-ready pipeline scenarios](https://github.com/mmannerm/animsmith/blob/main/docs/pipeline-scenarios.md)
- [Workspace design](https://github.com/mmannerm/animsmith/blob/main/DESIGN.md)
- [CLI crate and examples](https://github.com/mmannerm/animsmith/tree/main/crates/animsmith)

## License

Licensed under either the MIT license or the Apache License, Version
2.0, at your option.

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in animsmith by you is licensed as MIT OR
Apache-2.0, without any additional terms or conditions.
