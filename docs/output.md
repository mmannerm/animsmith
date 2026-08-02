# Machine-Readable Output

animsmith's native JSON is the stable source of truth for pipeline adapters.
Text and Markdown lint output are presentation views over the same evaluation
results. The HTML report remains a sampled-motion view with content findings;
future machine serializers should project the JSON contract.

## Contract identities

Every JSON command emits output contract v2 with the immutable protocol
identity `urn:animsmith:schema:output:2`. The retrievable schema is
[`output-v2.schema.json`](schemas/output-v2.schema.json); its repository URL
is a retrieval location, not the protocol identity.

Measurement evidence is nested and independently versioned as
`urn:animsmith:schema:measurements:2`. Its retrievable schema is
[`measurements-v2.schema.json`](schemas/measurements-v2.schema.json). A future
measurement-definition change can therefore bump that contract without
redesigning the outer result envelope.

`convert --format json` is deliberately a separate conversion-evidence
contract, not another command in the output-v2 envelope. Its immutable
identity is `urn:animsmith:schema:conversion-evidence:2`; its retrievable
schema is
[`conversion-evidence-v2.schema.json`](schemas/conversion-evidence-v2.schema.json).
This lets producers pin conversion provenance independently of measurement
and lint evidence.

Conversion evidence v1 remains a historical immutable contract at
`urn:animsmith:schema:conversion-evidence:1`. The current CLI emits v2
exclusively; regenerate v1 evidence when a v2 consumer is required.

The project is alpha, so the final v2 cutover intentionally does not read or
emit earlier v1 or preview reports. Regenerate old reports with the current
`animsmith measure --format json` before passing them to `diff`.

## Common envelope

```json
{
  "schema_version": 2,
  "schema": "urn:animsmith:schema:output:2",
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

Each array has the exact BaseColor and normal pair for every recipe material:
two records per material. Records are ordered by source-material index, then
BaseColor before normal; recipe declaration order does not affect them.
`material_index` is the source-material identity; `material_name` is display
data. `emitted_bytes` is the encoded byte count. See [material texture
recipes](material-texture-recipes.md) for containment and image semantics.

## `measure` and `lint`

Both commands put evidence under `files[].measurements`:

```json
{
  "schema_version": 2,
  "schema": "urn:animsmith:schema:measurements:2",
  "clips": {},
  "mesh_definitions": [],
  "node_instances": [],
  "scenes": []
}
```

`clips` maps clip names to duration, frame count, animated bones, rotation
ranges, and optional role-dependent gait, seam, and speed metrics.

`mesh_definitions` contains one record per source mesh definition. Its
`geometry_aabb` reduces finite primitive `POSITION` values in the mesh's own
coordinates; it is independent of every node and scene. Vertex and skin
influence statistics are properties of that same definition.

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

Lint adds exactly one `files[].checks[]` record for every built-in catalog
check. Each record keeps these dimensions independent:

- `selection`: `selected` or `unselected`;
- `configuration`: `enabled` or `disabled`;
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
| `measurement_unavailable` | A required numeric measurement could not be produced or did not meet its evidence floor. | `loop-seam`, `root-motion-speed`, `in-place`, `foot-slide`, `gait-group` |
| `insufficient_measurable_members` | Fewer than two gait-group members produced usable phases. | `gait-group` |
| `members_not_evaluated` | Some configured gait-group members did not produce usable phases. | `gait-group` |
| `invalid_declared_fps` | A declared frame rate was zero, negative, or non-finite. | `fps` |
| `insufficient_rotation_evidence` | Too few usable rotation tracks existed for a bind-pose comparison. | `bind-pose` |

Built-in completed/gap scope codes are:

| Scope code | Work unit | Emitted by |
|---|---|---|
| `member_existence` | Configured gait-group members were checked for existence. | `gait-group` |
| `phase_measurement` | One named clip's gait phase was measured or lacked usable evidence. | `gait-group` |
| `phase_coherence` | One named gait group's measurable phases were compared. | `gait-group` |
| `loop_seam` | One named clip's positional loop seam was measured. | `loop-seam` |
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

Numeric equality in the JSON contract means equality of decoded JSON numbers,
not byte-for-byte lexical spelling. For example, `1`, `1.0`, and `1e0` denote
the same numeric value to a conforming adapter.

## `diff`

`diff --format json` uses the same output v2 header and emits `inputs`, a
delta count, and structured metric deltas:

```json
{
  "schema_version": 2,
  "schema": "urn:animsmith:schema:output:2",
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

`diff` accepts asset files or one-file v2 `measure`/`lint` reports carrying
measurement contract v2. Multi-file reports and unsupported contract versions
are rejected as operator errors. Before extracting the clip metrics it uses,
`diff` validates the complete measurement record, including mesh evidence, and
rejects malformed or non-finite payload values.
