# Multi-source character assembly

`animsmith assemble` combines one authoritative skinned base with animation
takes from separate FBX or glTF inputs. Use it when a delivery spreads clips
across individual files or long master timelines but the runtime needs one GLB.

```console
animsmith assemble character-assembly.toml \
  -o character.glb \
  --evidence character.assembly.json
```

The command is available in default builds with the `fbx` feature. It always
writes a GLB and versioned JSON evidence as one publication pair. It prepares
both files before replacing either destination and restores prior outputs if
the second replacement fails.

This rollback contract covers every error returned by the command. No portable
filesystem primitive can make two independently named destinations
power-loss-atomic, so after abrupt process or machine termination the consumer
must verify the evidence artifact digest before publication.

## Boundary

Assembly owns format-generic scene, skin, animation, material, and GLB work.
The consuming project still owns source-package extraction, gameplay naming
and acceptance policy, cache/generation policy, provenance beyond assembly
evidence, and publication. DCC automation is outside this command.

## Recipe

Start from [`examples/character-assembly.toml`](../examples/character-assembly.toml):

```toml
schema_version = 3
schema = "urn:animsmith:schema:character-assembly-recipe:3"
input_root = "inputs"
base_input = "base-character.fbx"
mesh_instances = ["character-mesh"]
remove_nodes = ["sword-placeholder"]
complete_tracks = true
canonicalize_skin = true
ground_and_center = true
prune_constant_tracks = false
fps = 30.0

[[clips]]
name = "idle"
input = "idle.fbx"
take = "Take 001"
frame_window = [1, 61]
drop_closing_endpoint = true
strip_bones = ["motion-root"]
```

All input paths are relative to `input_root`, which is itself relative to the
recipe. Absolute paths, parent traversal, symlink components, and paths that
escape the canonical root are rejected. Output paths are command arguments so
the recipe remains relocatable.

The input root is an operator-controlled snapshot: do not mutate or replace its
entries concurrently with assembly. Symlink checks and reads are separate
portable filesystem operations, so the command does not claim to defend
against an actor racing the input tree while it runs.

### Discover exact recipe names

Inspect the base before writing the recipe:

```console
animsmith inspect inputs/base-character.fbx
```

Copy the quoted name after `node` in the `mesh instances` section exactly into
`mesh_instances`; quoted names use TOML-compatible escapes. Each entry also
shows its source node, referenced mesh,
skinned status, and the material used by every primitive. The top-level
`materials` section lists the exact, case-sensitive names required by a
[material texture recipe](material-texture-recipes.md). Duplicate skeleton node
or material names are marked `ambiguous`; assembly and material recipe
validation reject ambiguous authored names instead of guessing. Multiple mesh
instances attached to one uniquely named node are not ambiguous and are
selected together. `inspect` only reports the authored scene—it does not rename
nodes or materials, merge duplicates, or repair the asset.

The base supplies the authoritative skeleton, rest pose, meshes, skins,
materials, and textures. `mesh_instances` names the exact base nodes whose
mesh instances survive; omitting it retains every instance. An explicit node
name must be unique; every mesh instance attached to that selected node
survives. Unreferenced mesh and material definitions are pruned.

Each `[[clips]]` entry selects one exact source `take`, renames it, and remaps
its tracks to the base skeleton by exact unique bone name. Missing referenced
bones and ambiguous names fail rather than retarget motion. An entry may then
apply:

- `frame_window = [START, END]`: one-based, inclusive source frame endpoints at
  `fps`, or `time_window = [START, END]` in seconds;
- `drop_closing_endpoint = true`: remove one final key from every channel;
- `hold_frames = N`: duplicate the final pose after `N / fps` seconds;
- `gait_anchor = true`: explicitly declare an in-place cyclic gait and run the
  measured gait-anchor transform using the selected `animsmith.toml` rig
  configuration. Assembly refuses before publication when the Root role (or
  Hips fallback) has missing/non-finite trajectory evidence, more than 1 cm of
  horizontal endpoint displacement, or more than 1° of yaw accumulation; no
  interior step is subtracted as an allowance. Every nonconstant channel the
  operation would rotate must contain exactly one key at each declared `fps`
  whole-frame sample over the clip duration, at the exact representable f32
  `key / fps` time and period endpoint. Sparse, differently framed, duplicate,
  or off-grid evidence refuses. The complete skeleton, resolved roles, and
  track shapes are validated first; declared-frame and maximum-authored-key
  work share an inclusive 1,000,000 frame-by-bone sampling budget;
- `strip_bones = [...]`: remove every TRS track for named base bones.

`complete_tracks = true` fills absent TRS channels from the base rest pose for
the union of selected skin joints and nodes targeted by any emitted clip.
Per-clip `strip_bones` entries stay excluded. Rotation key values are
unit-normalized and made hemisphere-consistent without changing cubic tangent
sign relationships.

`canonicalize_skin = true` bakes each skin's common geometry-to-world transform
into positions and normals and rewrites inverse binds consistently.
`ground_and_center = true` additionally places the bind-pose geometry on Y=0
and centers it in X/Z while translating skeleton roots and root-translation
animation tracks by the same offset. Inconsistent skin transforms, singular
normal transforms, or shared mesh definitions that require incompatible bakes
are rejected.

`prune_constant_tracks = true` removes tracks that the existing core constant-
track predicate proves constant, after completion, quaternion cleanup, and all
other assembly transforms. This is all-property pruning: it can remove
completion-generated `(bone, property)` coverage when the completed value is
constant, so it may undo part of `complete_tracks = true`. The carve-out uses
the effective output clip's `animates_bones` exact names; it never uses
`required_bones`, so declared motion evidence is preserved without retaining
unrelated tracks. The canonical example leaves pruning disabled; opt in only
after reviewing the completed clip and the consumer's transition behavior. A
consumer that does not explicitly reset an omitted property during a transition
can retain the outgoing clip's value, so leave pruning disabled where dense
transition coverage matters until property-scoped selection is available (see
[#401](https://github.com/mmannerm/animsmith/issues/401)). Set it to `false` or
omit it to retain every otherwise eligible track.

`remove_nodes = [...]` selects exact, case-sensitive names from the
post-canonicalization base skeleton and removes each selected node together
with its complete descendant subtree. Every name must resolve exactly once;
an absent or ambiguous name fails rather than being skipped. Overlapping
ancestor and descendant selections are allowed and their union is removed
once, in original parent-before-child node order. Selecting the entire
skeleton is refused. Surviving descendants are never reparented because every
descendant of a removed node belongs to the removal closure.

Removal is planned before track completion, and closure nodes are excluded
from completion targets. The structural projection is applied only after
clip processing and constant-track pruning. It is
refused if a final track still targets the closure, if a mesh instance is
attached to it, or if a selected node remains referenced by a skin joint or
complete source-skin identity. Per-clip `strip_bones` and constant-track
pruning can make an animated decorative subtree removable; `remove_nodes`
never weakens the final-reference refusal. The effective clip's
`animates_bones` configuration still protects named tracks from pruning, so a
protected surviving track causes removal to fail. `[rig] required_bones` is a
lint presence policy, not a node-removal selector or protection list.

Errors are deterministic and no output is published on any refusal. Recipe
schema and value validation precede input loading. After the base's selected
mesh and canonicalization steps, selectors and their descendant closure are
resolved deterministically and the whole-skeleton rule is checked. After all
clip transforms, every surviving track, mesh attachment, skin reference, and
complete source-skin fact is checked before the hierarchy is changed. The
reported refusal identifies the offending stored record; traversal order is
an implementation detail rather than a wire contract.

Accepted projection preserves the original order and parent links of every
surviving node, remaps all surviving node references through one stable map,
and clears optional source-native skeleton projection because it can no longer
describe the authored source completely. Node removal performs no mesh,
material, or texture garbage collection. A mesh instance inside the closure
is refused, so accepted removal cannot newly orphan its resources; the earlier
explicit `mesh_instances` selection and its existing resource pruning remain
the only assembly resource-selection pass.

An optional `material_texture_recipe` reuses the exact material-name recipe
boundary described in [Material texture recipes](material-texture-recipes.md),
including BaseColor, normal, metallic-roughness, and occlusion slots.

The normative current recipe schema is
[`character-assembly-recipe-v3.schema.json`](schemas/character-assembly-recipe-v3.schema.json).
Recipe v1 and v2 remain immutable historical contracts. To migrate from v2,
change `schema_version` and `schema` to v3, then add `remove_nodes` or omit it
to request no node projection. V3 still applies its current shared snapshot validation.

## Evidence and determinism

The current evidence identity is
`urn:animsmith:schema:character-assembly-evidence:3`. It records the effective
recipe, recipe and input SHA-256 digests, the selected configuration file's
declared path and digest (or an explicit built-in-defaults marker), selected takes and windows, exact
source/base bone remap names and indices, and track
operation counts, start/end/delta facts for named translation tracks removed by
`strip_bones`, mesh selection, canonicalization flags, tool identity, and the
final artifact digest and counts. When pruning is enabled, each clip records
the exact removed tracks in `pruned_constant_tracks`: the index in the
completed, normalized output clip immediately before pruning (retaining its
pre-prune authored order), exact bone name and `bone_index` in the
post-canonicalization/pre-node-removal skeleton, TRS property, interpolation,
and key count. `bone_remaps.base_index` uses that same pre-removal coordinate
space. Consumers derive a surviving final index by subtracting earlier
`transforms.removed_nodes[].original_node_index` entries; a pruned track whose
node is later removed intentionally has no final-artifact node index.
The array is empty when pruning is disabled or no track is removed. The
top-level `transforms.removed_nodes` array records every projected node exactly
once in original pre-removal node order: its exact name, original node index,
nullable original parent index, and whether the recipe selected it directly
rather than through an ancestor. It is empty when `remove_nodes` is omitted or
empty. See
[`character-assembly-evidence-v3.schema.json`](schemas/character-assembly-evidence-v3.schema.json).

Evidence v1 and v2 remain immutable historical contracts; v1 does not describe
pruning and neither historical version describes structural node removal.
Consumers migrating to v3 must verify `removed_nodes` against the effective
recipe and output hierarchy rather than inferring removal from track evidence
or lint configuration.

Given identical recipe bytes, input and config bytes, tool build, paths, and
platform, repeated runs emit byte-identical GLB and evidence. Evidence keeps
operator-declared paths rather than leaking canonical host paths.
