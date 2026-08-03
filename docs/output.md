# Machine-Readable Output

animsmith's native JSON is the stable source of truth for pipeline adapters.
Text and Markdown lint output are presentation views over the same evaluation
results. The HTML report remains a sampled-motion view with content findings;
future machine serializers should project the JSON contract.

## Contract identities

Validation and comparison JSON commands emit output contract v3 with the immutable protocol
identity `urn:animsmith:schema:output:3`. The retrievable schema is
[`output-v3.schema.json`](schemas/output-v3.schema.json); its repository URL
is a retrieval location, not the protocol identity.

Measurement evidence is nested and independently versioned as
`urn:animsmith:schema:measurements:8`. Its retrievable schema is
[`measurements-v8.schema.json`](schemas/measurements-v8.schema.json). Version 8
adds per-bone angular seam-velocity evidence; a future
measurement-definition change can therefore bump that contract without
redesigning the outer result envelope.

`convert --format json` is deliberately a separate conversion-evidence
contract, not another command in the output-v3 envelope. Its immutable
identity is `urn:animsmith:schema:conversion-evidence:2`; its retrievable
schema is
[`conversion-evidence-v2.schema.json`](schemas/conversion-evidence-v2.schema.json).
This lets producers pin conversion provenance independently of measurement
and lint evidence.

`assemble` writes a separate character-assembly-evidence v1 document to its
required `--evidence` path. Its immutable identity is
`urn:animsmith:schema:character-assembly-evidence:1`; its retrievable schema is
[`character-assembly-evidence-v1.schema.json`](schemas/character-assembly-evidence-v1.schema.json).
The paired GLB and evidence are prepared before publication, so an operator
failure emits neither new destination and restores any prior pair.

Conversion evidence v1 remains a historical immutable contract at
`urn:animsmith:schema:conversion-evidence:1`. The current CLI emits v2
exclusively; regenerate v1 evidence when a v2 consumer is required.

[`Output-v2`](schemas/output-v2.schema.json) remains a historical immutable
contract. The current CLI emits and
`diff` reads output-v3; regenerate a v2 report with the current
`animsmith measure --format json` before passing it to `diff`.

## Common envelope

```json
{
  "schema_version": 3,
  "schema": "urn:animsmith:schema:output:3",
  "tool": {
    "name": "animsmith",
    "version": "0.1.0",
    "source": {
      "revision": "0123456789abcdef0123456789abcdef01234567",
      "dirty": false
    }
  },
  "command": "measure",
  "summary": { "files": 1 },
  "files": []
}
```

`tool.version` is the package's plain semantic version. Source revision and
dirty state are separate fields so automation never has to parse a decorated
version string. Packaged source records its Cargo VCS revision and leaves
`dirty` as `null`; builds without trustworthy VCS metadata may leave both
fields `null`.

Every `measure` and `lint` file record also includes `input`, with the exact
primary-file byte count and lowercase SHA-256 digest of the bytes parsed for
that row. Retain this identity with the JSON evidence when a pipeline promotes
or publishes an asset: it proves which primary payload the recorded result
describes. For multi-file invocations, rows stay in argument order and each
has its own independently calculated identity.

The identity covers only the named primary input file. In particular, for a
text `.gltf`, external buffers and images loaded beside it are not included in
the digest or byte count. Pipelines that need complete dependency provenance
must retain and identify those resources separately.

Operator failures do not emit a JSON envelope. They exit 2, write a diagnostic
to stderr, and leave stdout empty. Content findings exit 1 at the configured
threshold; coverage gaps are evidence and are nonblocking by default.

## `convert`

`convert --format json` emits one conversion-evidence v2 document. It records
the input and output paths, the requested options, and counts derived from the
written artifact. It is producer evidence: consumers should use the stable
field names and schema identity rather than parsing the text write summary.

```json
{
  "schema_version": 2,
  "schema": "urn:animsmith:schema:conversion-evidence:2",
  "tool": {
    "name": "animsmith",
    "version": "0.1.0",
    "source": { "revision": null, "dirty": null }
  },
  "command": "convert",
  "input": "prop.fbx",
  "output": "prop.glb",
  "options": {
    "animation_only": false,
    "bake_static_mesh_transforms": true,
    "material_texture_recipe": null
  },
  "artifact": {
    "nodes": 2,
    "animations": 0,
    "meshes": 1,
    "primitive_positions": 3,
    "materials": 1,
    "clips_without_writable_tracks": 0
  },
  "static_mesh_bake": {
    "entries": [
      {
        "source_node_index": 4,
        "source_node_name": "prop",
        "source_mesh_ordinal": 0,
        "source_mesh_index": 7,
        "source_mesh_name": "prop_mesh",
        "output_node_index": 1,
        "output_mesh_index": 0,
        "world_transform": [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1],
        "linear_determinant": 1,
        "primitive_count": 1,
        "position_count": 3,
        "normal_count": 3
      }
    ]
  }
}
```

`static_mesh_bake` is absent unless
`--bake-static-mesh-transforms` was requested. Its entries are in deterministic
source-node order. `source_node_index`, `source_mesh_ordinal`, and
`source_mesh_index` identify the source record; names are display data and
need not be unique. `world_transform` is the 16-element column-major rest
world matrix applied to the source positions. `linear_determinant` records the
accepted transform's linear determinant. The output node and mesh indices are
indices in the generated artifact.

The static bake is opt-in and conflicts with `--animation-only`. It only
accepts unanimated, unskinned, singly-instanced static geometry with finite,
non-reflecting, non-singular (including near-singular) transforms. It bakes
positions and inverse-transpose normalized normals into a canonical
identity-root output while retaining indices, UVs, model-supported material
assignments, and embedded base-color and normal textures. Unsupported input is
an operator error, not partial evidence. Repeated same-platform conversion
with the same input and options emits a byte-identical artifact.

When `options.material_texture_recipe` is a path, the top-level
`material_texture_recipe` object is required. When it is `null`, that object is
prohibited. This separates ordinary linked or embedded texture conversion from
recipe provenance. The object records the recipe identity, declared recipe path
and optional root, dimension cap, locked processor policy, and deterministic
consumed/emitted image records:

```json
{
  "schema_version": 1,
  "schema": "urn:animsmith:schema:material-texture-recipe:1",
  "path": "recipes/materials.toml",
  "texture_root": "textures",
  "max_dimension": 1024,
  "processor": {
    "image_crate": "image@0.25.10",
    "png_crate": "png@0.18.1",
    "jpeg_crate": "zune-jpeg@0.5.15",
    "base_color_algorithm": "sRGB-to-linear premultiplied-alpha Lanczos3",
    "normal_algorithm": "tangent-vector Triangle renormalize +Z fallback",
    "data_algorithm": "linear-channel Triangle",
    "output_encoding": "PNG RGBA8 compression=Best filter=NoFilter"
  },
  "consumed_inputs": [
    { "material_index": 0, "material_name": "surface", "slot": "base_color", "declared_path": "surface-base.png", "mime": "image/png", "dimensions": [2048, 1024] }
  ],
  "emitted_textures": [
    { "material_index": 0, "material_name": "surface", "slot": "base_color", "declared_path": "surface-base.png", "mime": "image/png", "dimensions": [1024, 512], "resized": true, "emitted_bytes": 1234 }
  ]
}
```

Each array has the required BaseColor and normal pair plus any declared
metallic-roughness and occlusion slots for every recipe material. Records are
ordered by source-material index, then BaseColor, normal, metallic-roughness,
and occlusion; recipe declaration order does not affect them.
`material_index` is the source-material identity; `material_name` is display
data. `emitted_bytes` is the encoded byte count. See [material texture
recipes](material-texture-recipes.md) for containment and image semantics.

## `assemble`

`assemble` writes character-assembly evidence v1 beside its GLB. The evidence
binds the effective recipe and its SHA-256, every base/clip/recipe/texture input
and digest, selected source takes and windows, exact track-operation counts,
removed named-bone translation deltas, mesh and skin canonicalization, tool
identity, and the final artifact digest and counts. Paths remain
operator-declared; canonical host paths used for containment checks are not
serialized.

The normative recipe and evidence contracts are
[`character-assembly-recipe-v1.schema.json`](schemas/character-assembly-recipe-v1.schema.json)
and
[`character-assembly-evidence-v1.schema.json`](schemas/character-assembly-evidence-v1.schema.json).
See [multi-source character assembly](character-assembly.md) for operation and
consumer-boundary semantics.

## `measure` and `lint`

Both commands put evidence under `files[].measurements`:

```json
{
  "schema_version": 8,
  "schema": "urn:animsmith:schema:measurements:8",
  "clips": {},
  "mesh_definitions": [],
  "node_instances": [],
  "scenes": [],
  "skeleton_source_coverage": "unavailable",
  "skeleton_nodes": [],
  "skins": [],
  "material_resource_coverage": "complete",
  "material_definitions": [],
  "textures": [],
  "images": []
}
```

`clips` maps clip names to duration, frame count, animated bones, rotation
ranges, optional per-bone loop continuity, and optional role-dependent gait,
foot-seam, and speed metrics.

`loop_continuity.bones[]` is present when a clip has at least three samples and
the seam-adjacent model-space evidence is finite. Rows stay in skeleton order
and carry both `bone_index` and `bone_name`; the numeric index is identity,
while the name is display context and need not be unique. Each row reports:

- `position_delta_m`: last-to-first model-space position distance (C0);
- `rotation_delta_deg`: last-to-first shortest-path model-space rotation
  difference (C0);
- `seam_velocity_delta_mps`: difference between the model-space linear
  velocities entering the last sample and leaving frame 0 (C1).
- `seam_angular_velocity_delta_degps`: shortest-path model-space angular
  velocity difference, in degrees per second, between those same incoming and
  outgoing steps (rotational C1).

The velocity comparison deliberately uses the two in-clip steps adjacent to
the wrap. The uniform grid contains both `t=0` and `t=duration`, so treating
the duplicate last-to-first endpoint chord as a velocity would report zero on
a perfectly closed loop. Model-space values include ancestor motion; the
rotation chain is composed independently of scale rather than decomposed from
a potentially sheared matrix. These measurements need no rig profile and are
emitted for measurable clips whether or not project configuration declares
them as loops. The `loop-closure`, `loop-seam-vel`, and `loop-seam-rot` checks
judge them only where `[clips.<name>] loop = true`.

`mesh_definitions` contains one record per source mesh definition. Its
`geometry_aabb` reduces finite primitive `POSITION` values in the mesh's own
coordinates; it is independent of every node and scene. When finite positions
exist, optional `geometry_centroid` is their arithmetic mean in that same
mesh-local coordinate domain. It is not the AABB midpoint and does not weight
vertices by triangle area, volume, or skin influence. Both fields omit
non-finite positions and are absent when no finite positions remain. Vertex
and skin influence statistics are properties of that same definition.

Indexed primitives contribute each base `POSITION` accessor element once,
regardless of how many times the index stream references it. Unindexed
primitives contribute each base position-stream element. These are the same
counting semantics as `vertex_count`. In text output, the value appears as
`geometry centroid (x, y, z)` beside the geometry bounding-box size, or as
`geometry centroid unavailable` when no finite position exists. JSON carries
the optional three-number `geometry_centroid` array.

Each mesh definition has `additional_influence_sets`, an always-present array
of authored secondary glTF skin-attribute sets discovered across its
primitives. Each entry has a numeric `set_index` of at least 1 plus independent
`joints_present` and `weights_present` booleans. The accompanying
`joints_without_weights_present` and `weights_without_joints_present` booleans
preserve unpaired declarations on individual primitives, even when the mesh
also has the complementary side on another primitive. Entries are strictly
ascending by `set_index` and appear at most once. The array is empty when no
secondary set was authored.
It excludes set 0: `max_joints_per_vertex` and the weight-sum extrema retain
their paired primary `JOINTS_0` / `WEIGHTS_0` semantics and do not incorporate
additional sets.

This is source-presence evidence, not secondary-skinning evaluation. animsmith
does not decode the additional per-vertex values into the core skinning model,
include them in weight statistics, repair unpaired sets, or preserve their
payloads when writing a converted asset. Keep the raw source when this evidence
matters; a consuming pipeline decides whether a non-empty or unpaired set is
acceptable.

Material and image evidence is deliberately separate from mesh definitions.
`material_resource_coverage` is `"complete"` for glTF/GLB input and
`"unavailable"` when the loader cannot provide the source-resource sidecar.
When coverage is complete, `material_definitions`, `textures`, and `images`
are source-indexed records in ascending source order. A material definition
has its optional display `name` and zero or more bindings
`{ "slot", "texture_index" }`. The supported slot vocabulary, in stable
semantic order, is `base_color`, `normal`, `metallic_roughness`, and
`occlusion`. A texture record has `texture_index`, optional `name`, and its
`image_index`. This preserves shared images and textures without duplicating
metadata per material slot.

An image record has `image_index`, optional `name`, a `source_kind` of
`"embedded"`, `"data_uri"`, or `"external"`, and optional declared and
decoded metadata. `declared_mime_type` is the source's declared MIME type;
it is source-authored text, not proof of the payload. `detected_container` is
the byte-detected `png` or `jpeg` container. `decoded_color_type` is one of
`l8`, `la8`, `rgb8`, `rgba8`, `l16`, `la16`, `rgb16`, or `rgba16`; its
`channel_count` is respectively 1, 2, 3, or 4, while `width` and `height` are
decoded pixel dimensions. Thus MIME describes a declared media label,
container describes encoded bytes, and color type/channel count describe the
decoded pixel representation. These facts can differ without contradiction.
Available images always have decoded metadata and no `unavailable_reason`.
An unavailable image has a stable `unavailable_reason` and no decoded
dimensions, color type, or channel count; a recognizable corrupt PNG/JPEG may
still report its detected container. Reasons are `source_unavailable`,
`invalid_data_uri`, `unsupported_container`, `decode_failed`, and
`resource_limit`. This is inventory evidence, not an image
acceptance decision: animsmith does not repair, resize, transcode, or judge
color-space, normal-map, engine-import, or artistic suitability here.

The records describe what the loader observed, not authority for later writes
or conversion recipes. A source-resource sidecar is not a promise that a
writer preserves every image payload, and a material-texture recipe remains a
separate explicit conversion input. Preserve the raw source if those source
details must be retained.

`node_instances` contains one record per mesh-bearing node. `node_index` and
`mesh_index` are stable source indices, so names need not be unique. Its
`static_node_world_aabb` transforms the definition's finite positions by that
node's default/rest world transform before reducing. It includes rotation and
negative or non-uniform scale, but explicitly excludes animation, skin
deformation, morph deformation, and runtime world placement. A missing box is
explained by `static_node_world_aabb_unavailable_reason`: no finite positions,
excluded skinned deformation, or a non-finite effective transform. The
producer must emit that reason exactly when the corresponding static box is
unavailable.

`scenes` contains every declared source scene. Each record counts its reachable
mesh-bearing nodes in `instance_count` and unions their available static
node-instance boxes in `static_scene_world_aabb`; `excluded_instance_count`
makes partial coverage explicit. A node can contribute to more than one scene.
Mesh-bearing nodes not reachable from a declared scene remain node instances
but contribute to no scene aggregate. `default_scene_index` is present only
when the source names a default scene; its absence does not select scene zero.

`skeleton_source_coverage` separates a loader that cannot expose source-node
and source-skin identity from a source that genuinely contains no nodes or
skins. When it is `"unavailable"`, both `skeleton_nodes` and `skins` are
empty. When it is `"complete"`, `skeleton_nodes` is the source node table in
ascending `node_index` order. `node_index`, `parent_node_index`, skin index,
and joint index are source identities; names are display data and need not be
unique. `scene_root_indices` lists the source scenes that directly declare the
node as a root, in ascending source-scene order. A joint can name any source
node as its parent, including a non-joint helper node.
Structurally inconsistent source identity tables, such as a missing parent or
parent cycle, downgrade the whole skeleton source domain to `"unavailable"`
instead of publishing a self-contradictory complete table.

Every source node records its authored `local_rest`, tagged as `"trs"`,
`"matrix"`, or `"unavailable"`. A TRS has `translation_m`,
`rotation_xyzw`, and `scale`; the quaternion is `[x, y, z, w]`. A matrix is a
16-element column-major local transform. `rest_world_matrix` is a separately
derived, column-major default/rest world matrix, or its typed unavailable
reason. An unavailable local representation carries its stable `reason`. The
local representation is never silently decomposed or replaced by
the world transform. A declared scene is membership only, not a transform
domain: `scene_root_indices` is membership evidence and adds no transform. A
transformed node selected as a scene root retains its own local rest record,
and its ancestors determine the derived world matrix.

`skins` is a source-skin table in ascending `skin_index` order. Its `joints`
are in declared skin-slot order; each joint row owns its `joint_index`,
`node_index`, `joint_bind_to_mesh`, and `mesh_bind_world` observations.
`skeleton_root_node_index` is present only when the source explicitly declares
one. `inverse_bind_accessor` reports
whether the inverse-bind accessor was absent, readable, empty, count-mismatched,
or unreadable. A readable finite accessor retains its raw column-major matrices
in slot order. Absent, malformed, and non-finite inverse-bind evidence is not
inferred from node-local rest data.

Each skin's `attachments` names every source node that declares use of that
skin, in source-node order. In each joint row, `joint_bind_to_mesh` is
`inverse(inverse_bind_matrix)`, so it maps joint bind-local coordinates into
the mesh-local bind domain declared by that skin; `mesh_bind_world` is
`joint_rest_world * inverse_bind_matrix`, mapping that mesh-local bind domain
into world bind coordinates when the authored rest and bind poses agree. These
remain per-joint observations rather than claims that rows agree. Attachments
are identity evidence, not an extra transform folded into either calculation.
Each derived field is either a finite matrix or a typed unavailable reason.
These are descriptive calculations only. animsmith does not decide whether a
consumer requires a joint, whether a skin/rest comparison is close enough,
which root is canonical, or whether an unavailable matrix is acceptable.

These facts help an artist or engine integration diagnose common handoff
questions without reparsing the source: whether a joint is nested below a
helper, whether a transformed scene root participates, whether two mesh nodes
reuse a skin, and whether inverse-bind data is missing or singular. The node
and skin terminology follows the [glTF 2.0 specification](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html).
They do not select a required bone list, validate an engine's tolerance,
retarget, normalize, repair, or rewrite the asset; those remain consuming
project policy.

Lint adds exactly one `files[].checks[]` record for every built-in catalog
check. Each record keeps these dimensions independent:

- `selection`: `selected` or `unselected`;
- `configuration`: `enabled` or `disabled`; a check is disabled when its
  severity is `off` or its built-in policy is opt-in and no enabling severity
  was configured;
- `applicability`: `applicable` or `not_applicable`;
- `evaluation`: `complete`, `partial`, or `not_evaluated`;
- content `findings`;
- completed `evaluated_scopes` and typed coverage `gaps`.

Gap and scope `code` fields are the machine contract; `message` is display
text and must never be parsed. Disabled, unselected, and not-applicable checks
are not artificial gaps. A partial check has at least one completed scope and
at least one gap. Applicable work that completed nothing has a gap and no
content findings. A scope can appear as completed and also be named by a gap
when a group-level calculation covered some but not all members.

Built-in gap codes are:

| Gap code | Meaning | Emitted by |
|---|---|---|
| `roles_unresolved` | Required semantic rig roles were not resolved. | `loop-seam`, `root-motion-speed`, `in-place`, `foot-slide`, `gait-group` |
| `measurement_unavailable` | A required numeric measurement could not be produced or did not meet its evidence floor. | `loop-closure`, `duplicate-loop-endpoint`, `loop-seam`, `loop-seam-vel`, `loop-seam-rot`, `root-motion-speed`, `in-place`, `foot-slide`, `gait-group` |
| `insufficient_measurable_members` | Fewer than two gait-group members produced usable phases. | `gait-group` |
| `members_not_evaluated` | Some configured gait-group members did not produce usable phases. | `gait-group` |
| `invalid_declared_fps` | A declared frame rate was zero, negative, or non-finite. | `fps` |
| `insufficient_rotation_evidence` | Too few usable rotation tracks existed for a bind-pose comparison. | `bind-pose` |

Built-in completed/gap scope codes are:

| Scope code | Work unit | Emitted by |
|---|---|---|
| `loop_closure` | One named clip's per-bone model-space pose closure was measured. | `loop-closure` |
| `duplicate_loop_endpoint` | One named clip's authored tracks were analyzed for redundant closing endpoint keys. | `duplicate-loop-endpoint` |
| `member_existence` | Configured gait-group members were checked for existence. | `gait-group` |
| `phase_measurement` | One named clip's gait phase was measured or lacked usable evidence. | `gait-group` |
| `phase_coherence` | One named gait group's measurable phases were compared. | `gait-group` |
| `loop_seam` | One named clip's positional loop seam was measured. | `loop-seam` |
| `loop_seam_velocity` | One named clip's per-bone model-space seam velocity continuity was measured. | `loop-seam-vel` |
| `loop_seam_rotation` | One named clip's per-bone model-space angular seam velocity continuity was measured. | `loop-seam-rot` |
| `root_motion_speed` | One named clip's root-motion speed was measured. | `root-motion-speed` |
| `travel_mode` | One named clip's in-place/root-motion declaration was judged. | `in-place` |
| `foot_stance` | Whole-clip prerequisites for stance analysis were evaluated. | `foot-slide` |
| `left_foot_stance` | The named clip's left foot/toe stance was evaluated. | `foot-slide` |
| `right_foot_stance` | The named clip's right foot/toe stance was evaluated. | `foot-slide` |
| `frame_grid` | The named clip's declared frame grid was evaluated. | `fps` |
| `first_frame_rest_delta` | The named clip's first-frame/rest-pose rotation evidence was evaluated. | `bind-pose` |

The built-in gap and scope declarations in `animsmith_core` are authoritative
for each code's identity, meaning, and allowed emitting check ids. Runtime
evaluation rejects a built-in code from an undeclared emitter, and the output
contract test derives this reference inventory from those same declarations.
The public code slices let consumers enumerate or allow-list animsmith's
built-in vocabulary; the meaning/emitter registry remains an implementation
detail. Custom checks may add namespaced gap codes and their own namespaced
scope vocabulary.

`summary.checks` reports a `total` and four independent partitions. Each of
`selection`, `configuration`, `applicability`, and `evaluation` sums to that
same total. `summary.checks.gaps` counts typed gaps, while
`summary.findings` counts content findings by severity.

`lint --format json` deliberately rejects `--allow` so machine evidence is
never deleted. `--allow` remains available for text and Markdown presentation
and their exit policy. Text and Markdown render coverage gaps separately from
findings and group repeated gaps by `(check_id, code)` for readability. Group
counts still reflect every underlying per-scope JSON gap.

## Findings and numeric values

Findings carry `check_id`, `severity`, optional `clip`, `bone`, `time_s`,
`measured`, and `expected` fields, plus a human message. Treat `check_id` and
the structured fields as automation data; treat `message` as display text.
The nested `check_id` intentionally repeats its owning check record so a
finding stays self-describing when extracted or consumed through the embedded
API; the evaluator rejects mismatched parent/child ids.
For `loop-closure` and `loop-seam-vel`, `expected` is the effective cap for
that finding's clip after exact-name and glob expectations are resolved, with
the corresponding global check setting or built-in default as fallback.

Numeric equality in the JSON contract means equality of decoded JSON numbers,
not byte-for-byte lexical spelling. For example, `1`, `1.0`, and `1e0` denote
the same numeric value to a conforming adapter.

## `diff`

`diff --format json` uses the same output v3 header and emits `inputs`, a
delta count, and structured metric deltas:

```json
{
  "schema_version": 3,
  "schema": "urn:animsmith:schema:output:3",
  "tool": {
    "name": "animsmith",
    "version": "0.1.0",
    "source": { "revision": null, "dirty": null }
  },
  "command": "diff",
  "inputs": { "before": "old.glb", "after": "new.glb" },
  "summary": { "deltas": 1 },
  "deltas": [
    { "clip": "walk", "metric": "speed_mps", "before": 1.0, "after": 1.2, "note": "moved" }
  ]
}
```

`diff` accepts asset files or one-file v3 `measure`/`lint` reports carrying
measurement contract v8. Multi-file reports and unsupported contract versions
are rejected as operator errors. Before extracting the clip metrics it uses,
`diff` validates the complete measurement record, including mesh evidence, and
rejects malformed or non-finite payload values.

Loop-continuity rows compare by `bone_index`. Re-export noise at or below
0.001 m for `position_delta_m`, 0.1 degree for `rotation_delta_deg`, and
0.01 m/s for `seam_velocity_delta_mps` is silent; the 0.5 degree/s floor
applies to `seam_angular_velocity_delta_degps`. Larger changes produce metric
paths such as `loop_continuity.bones[12].rotation_delta_deg`. These are diff
significance floors, not the lint caps configured under
`[checks.loop-closure]`, `[checks.loop-seam-vel]`, and `[checks.loop-seam-rot]`.
