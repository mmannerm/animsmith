# Static assets: bounds, transforms, and textures

This guide is for artists, technical artists, engine integrators, and build
engineers handing a prop or environment asset from a DCC tool to a game. It
explains four questions that look similar in a viewport but need different
answers in an asset pipeline:

1. Which bounding box are we measuring?
2. Is placement stored in mesh vertices or in a node hierarchy?
3. Did the normal map survive conversion as a normal map?
4. Should missing texture links be preserved, repaired declaratively, or sent
   back to the artist?

animsmith reports or performs only the part it can prove. It does not certify
an engine's import settings, generate artistic material content, or replace a
runtime smoke test.

## Start with the handoff problem

| What you observe | Likely source | Start with |
|---|---|---|
| The prop is the right shape but has the wrong size or placement after import. | Object or parent transforms survived as scene hierarchy instead of being applied to vertices. | `animsmith measure`; compare definition, instance, and scene bounds. |
| Culling, framing, or placement code sees a different box than the DCC viewport. | The code and artist are discussing different coordinate domains. | `animsmith measure --format json`. |
| The surface silhouette is right but grooves and bevels look flat. | The normal image or material slot was lost, or the engine imported it as color data. | Ordinary `convert`, then inspect the material in the engine. |
| The model is gray or flat because exported material links are incomplete. | External image paths were not portable or the exporter omitted them. | `convert --material-texture-recipe`. |
| You need to know which material slot uses which source image before an engine import. | A mesh view alone hides material-to-texture-to-image sharing and image encoding facts. | `animsmith measure --format json`. |
| A consumer requires identity transforms and mesh-local final geometry. | The consumer does not retain or intentionally does not evaluate the source node hierarchy. | `convert --bake-static-mesh-transforms`. |

## Which bounding box do you mean?

A mesh definition contains reusable vertex data. A node places an instance of
that mesh into a hierarchy. A scene chooses root nodes and therefore a set of
instances. The [glTF scene and node model](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html#scenes)
and its [transformation rules](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html#transformations)
make those identities separate: a mesh's global transform comes from the node
that references it, and a child inherits its parents' transforms.

That gives one asset several valid axis-aligned bounding boxes (AABBs):

| animsmith record | Coordinate domain | Question it answers |
|---|---|---|
| `mesh_definitions[].geometry_aabb` | Mesh-local source positions | How large is the reusable geometry before any placement? |
| `mesh_definitions[].geometry_centroid` | Mesh-local finite source positions | Where is the average authored vertex position before any placement? |
| `node_instances[].static_node_world_aabb` | Default/rest hierarchy | Where and how large is this particular static instance after its parent and local transforms? |
| `scenes[].static_scene_world_aabb` | One declared source scene | What box encloses the measurable static instances reachable from this scene's roots? |

For example, a unit cube can have a mesh-local box from `(0, 0, 0)` to
`(1, 1, 1)`, while a node with scale `2` and translation `10` along X has a
static node-world box from `(10, 0, 0)` to `(12, 2, 2)`. Neither result is
wrong. Using the first as a placed-world bound, or the second as a reusable
mesh bound, is the bug.

`geometry_centroid` is a separate mesh-local fact: it is the arithmetic mean
of finite decoded base `POSITION` rows, not the center of `geometry_aabb`. An
asymmetric mesh can therefore have a centroid away from its box center. It is
useful for checking authored pivots or coarse placement assumptions, but it is
not a center of mass, surface-area-weighted center, collision origin, or a
runtime placement guarantee.

For indexed geometry, each stored `POSITION` contributes once even when the
index stream references it repeatedly; for unindexed geometry, each stored
position contributes once. Empty or wholly non-finite geometry has no
centroid. animsmith reports this evidence but does not move the pivot, recenter
the mesh, or bake transforms as part of `measure`.

```console
animsmith measure prop.glb
animsmith measure prop.glb --format json > prop.measure.json
```

### Why the distinction matters

- **Artists** can tell whether unexpected size comes from modeled geometry or
  an unapplied object/parent transform.
- **Engine integrators** can choose the domain their importer, preview camera,
  culling system, or placement code actually consumes.
- **Pipeline developers** can compare the same source identities across
  re-exports instead of joining records by non-unique display names.

An unavailable box is not an empty or zero-sized box. Skinned deformation,
non-finite transforms, or a definition without finite positions produce an
explicit unavailability reason. Scene records also expose excluded instance
counts, so a partial static union cannot masquerade as complete coverage.

These are default/rest source-asset measurements. They deliberately exclude
animation, skin and morph deformation, runtime component transforms,
engine-generated collision, and camera- or frame-dependent bounds. Validate
those in the consuming engine.

## Inventory material and image handoffs

Before treating a missing detail as an engine-shader problem, inspect the
source material graph:

```console
animsmith measure prop.glb --format json > prop.measure.json
```

For glTF/GLB, the nested measurement data records source-order material
definitions and their semantic bindings, then texture-to-image references and
image metadata. That exposes, for example, a normal slot that points to the
same image as BaseColor, a texture shared by several materials, a linked
external image, or an image whose declared MIME disagrees with the container
found in its bytes. Successful image inspection reports decoded width, height,
channel count, and color type. It does not decide whether a BaseColor texture
is authored well, whether a normal map has the intended convention, or whether
an engine importer will choose the right settings.

An unavailable image is explicit rather than silently omitted: it has an
unavailability reason and no decoded metadata. A malformed PNG/JPEG may still
identify its container, but is not repaired. Other loaders can report
`material_resource_coverage: "unavailable"`; do not equate that with an empty
material table. This inventory is source evidence only. It neither resizes or
transcodes images nor makes a later writer or conversion recipe authoritative
for every original resource. Keep the raw handoff asset when byte-level source
provenance matters.

## Preserve normal maps as data, not color

A tangent-space normal map stores direction vectors in RGB texels. It changes
lighting without changing the mesh silhouette; Unity's
[normal-map introduction](https://docs.unity3d.com/6000.1/Documentation/Manual/StandardShaderMaterialParameterNormalMap.html)
shows why a low-poly surface can retain grooves, rivets, and other fine detail.
Blender likewise documents that a tangent-space
[Normal Map node](https://docs.blender.org/manual/en/latest/render/shader_nodes/vector/normal_map.html)
expects matching UVs and non-color image data.

Ordinary full-scene conversion preserves available linked or embedded PNG/JPEG
base-color and normal images. A glTF normal texture keeps its
`normalTexture.scale`; an FBX normal map uses glTF's default scale because the
ordinary FBX material does not expose the same scalar. The glTF material model
defines [the normal slot and scale](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html#material-normaltexture)
separately from the
[sRGB BaseColor slot](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html#reference-material-pbrmetallicroughness-basecolortexture).

```console
animsmith convert source.fbx -o source.glb
```

Expected result: a material that had a discoverable normal image still has a
normal texture after conversion, and converting or transforming the glTF again
does not silently replace it with the BaseColor image or discard its strength.
If the original bytes already satisfy the operation, they pass through without
being decoded and re-encoded.

### What preservation does not fix

animsmith does not decide whether an image is artistically correct, convert a
height map into a normal map, flip an engine-specific green-channel convention,
generate missing tangents, or configure the engine's texture importer. For
example, Unity still requires the texture to be imported as a normal map and
documents its expected Y+ convention. A model can therefore contain the right
glTF material link and still look wrong under engine-specific import or shader
settings.

## Bake static placement into geometry

Use `--bake-static-mesh-transforms` when a static consumer needs final placement
in mesh-local vertices beneath identity nodes. A common example is a prop
modeled under rotated or scaled helper objects: it looks right in the DCC and a
hierarchy-aware viewer, but a consumer that extracts only the mesh definition
gets the unplaced shape.

```console
animsmith convert prop.fbx -o prop-baked.glb \
  --bake-static-mesh-transforms
```

The operation accumulates the accepted rest hierarchy into positions, applies
the inverse-transpose transform to normals and normalizes them, then emits a
canonical identity root with identity mesh children. It retains indices, UVs,
supported material assignments, BaseColor textures, and normal textures.
`convert --format json` records the source identity, applied world matrix,
determinant, and output identity for each baked instance.

Conceptually, this is the exchange-format counterpart of applying object
transforms in a DCC. Blender's
[Apply transforms](https://docs.blender.org/manual/en/latest/scene_layout/object/editing/apply.html)
documentation explains the artist-side operation: transfer the current object
transform into its data while keeping the visible result in place. animsmith's
operation is intentionally narrower and fail-closed.

| Before | After |
|---|---|
| Placement may be split across parents and a mesh node. | Accepted placement is in vertex positions. |
| Mesh nodes may have non-identity rest transforms. | Output root and mesh children are identity transforms. |
| A hierarchy-aware viewer is required to reproduce placement. | A consumer of the emitted mesh-local geometry sees the baked placement. |
| Source normals are local to the original mesh. | Normals are inverse-transpose transformed and unit-normalized. |

### Rejection is part of the contract

The bake exits with an asset refusal (`1`) and writes no output instead of guessing
when the source cannot be transformed without violating its preservation
promises. The [CLI reference](cli.md#static-mesh-transform-bake) owns the exact
acceptance and rejection list.

It is not a general scene flattener. It does not bake skins or animation,
duplicate shared instances, repair winding, preserve artistic pivots or helper
hierarchies, choose a project-specific forward axis, or generate collision.
Use the DCC or an engine-aware asset build step when one of those changes is
intentional.

## Attach textures with a recipe

Use `--material-texture-recipe` when the source material names are trustworthy
but image links are missing, non-portable, or subject to a project size cap.
Do not add a recipe merely because conversion supports one: ordinary conversion
already preserves texture links it can discover.

```console
animsmith convert prop.fbx -o prop.glb \
  --material-texture-recipe recipes/prop-materials.toml
```

A typical bad handoff has a material named `surface`, an artist-approved
BaseColor image and tangent-space normal image beside the source, but an FBX
whose external paths point to another workstation. A recipe maps the exact,
case-sensitive source material name to those two images. If either path, image,
or material match is invalid, the whole conversion fails before output.

The successful result has one explicit BaseColor/normal pair for every recipe
entry. Images within `max_dimension` keep their original bytes and MIME type.
Larger images use deterministic, role-aware processing: BaseColor is treated as
color, while a normal map is treated as vector data and renormalized. Conversion
evidence records every consumed and emitted image.

The recipe does not generate textures, invent material matches, pack ORM maps,
add unsupported PBR slots, repair UVs or tangents, change normal-map handedness,
or judge artistic quality. See the
[material texture recipe reference](material-texture-recipes.md) for the exact
TOML schema, containment policy, limits, and processing contract.

## Verify the result in layers

One successful conversion is evidence about the asset operation, not proof of
the whole runtime presentation. Keep the raw source immutable and verify each
layer explicitly:

```console
# 1. Record source bounds and identities.
animsmith measure prop.fbx --format json > prop.before.json

# 2. Convert with only the operations your handoff requires.
animsmith convert prop.fbx -o prop.glb \
  --bake-static-mesh-transforms \
  --material-texture-recipe recipes/prop-materials.toml \
  --format json > prop.conversion.json

# 3. Measure the written artifact, not only the source.
animsmith measure prop.glb --format json > prop.after.json
```

Then inspect the written artifact in the target engine:

- confirm size, forward/up orientation, pivot expectations, and hierarchy;
- confirm the shader uses BaseColor and normal textures in the intended slots;
- confirm the normal texture's import type and channel convention;
- exercise runtime culling, LODs, collision, lighting, and instancing;
- retain the conversion evidence beside the derived artifact so automation can
  explain how it was produced.

That final engine pass stays necessary because animsmith does not know the
consumer's shaders, import presets, physics scale, collision generation, or
runtime placement policy.
