# animsmith-core

## Overview

`animsmith-core` is animsmith's engine-agnostic library crate. This
README is a compact crates.io and repository index; the crate-root
rustdoc owns the embedding flow, API status, extension points, and
panic/error contracts.

## Install

```toml
[dependencies]
animsmith-core = "0.1"
animsmith-gltf = "0.1"
```

## Feature Flags

- `fixtures` (off by default) — exposes `animsmith_core::fixtures`, the
  analytic-clip fixture builders shared with animsmith's own tests and
  example-asset generator. Adds no dependency (the builders take their
  sine as a parameter). Internal to the animsmith workspace and **not**
  part of the crate's stable API; downstream code should not depend on
  it.

The workspace MSRV is Rust 1.88.

## Additional Skin-Influence Evidence

`Primitive::additional_influence_sets` carries independent presence metadata
for secondary glTF `JOINTS_n` and `WEIGHTS_n` attributes. The measurement
contract aggregates that metadata per source mesh and retains whether either
side was unpaired on an individual primitive, so complementary declarations on
different primitives are not reported as a clean pair. It intentionally does
not retain secondary per-vertex payloads or change the primary four-influence
skinning semantics.

## Static Mesh Transform Baking

Embedders can opt into
`animsmith_core::bake_static_mesh_transforms` to bake accumulated static
rest transforms into mesh-local positions and inverse-transpose normalized
normals. The operation returns a canonical identity-root document plus
deterministic per-instance evidence. It validates the complete input before
constructing output and fails closed for animation, skinning, ambiguous mesh
instancing, malformed data, reflections, and ill-conditioned transforms.
Model-supported material factors and embedded base-color, normal, metallic-roughness, and occlusion textures
are preserved.

## Character Assembly Helpers

`animsmith_core::assembly` provides exact-name clip remapping onto an
authoritative skeleton, named-bone track stripping, optional rest-pose channel
completion for all or an explicit base-bone selection, deterministic quaternion hemisphere cleanup, and endpoint-key
removal. It rejects ambiguous or missing referenced names rather than guessing
at a retargeting relationship. For a final-pose hold, use the existing
`animsmith_core::transform::hold_extend` helper.

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
