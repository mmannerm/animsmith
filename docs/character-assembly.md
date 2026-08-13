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
schema_version = 2
schema = "urn:animsmith:schema:character-assembly-recipe:2"
input_root = "inputs"
base_input = "base-character.fbx"
mesh_instances = ["character-mesh"]
complete_tracks = true
canonicalize_skin = true
ground_and_center = true
prune_constant_tracks = true
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
- `gait_anchor = true`: run the existing measured gait-anchor transform using
  the selected `animsmith.toml` rig configuration;
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
other assembly transforms. The carve-out uses the effective output clip's
`animates_bones` exact names; it never uses `required_bones`, so declared motion
evidence is preserved without retaining unrelated tracks. Set it to `false` or
omit it to retain every otherwise eligible track.

An optional `material_texture_recipe` reuses the exact material-name recipe
boundary described in [Material texture recipes](material-texture-recipes.md),
including BaseColor, normal, metallic-roughness, and occlusion slots.

The normative current recipe schema is
[`character-assembly-recipe-v2.schema.json`](schemas/character-assembly-recipe-v2.schema.json).
Recipe v1 remains an immutable historical contract. To migrate, change
`schema_version` and `schema` to v2, then choose `prune_constant_tracks`; its
default is `false`, so an otherwise unchanged v1 recipe keeps its behavior.

## Evidence and determinism

The current evidence identity is
`urn:animsmith:schema:character-assembly-evidence:2`. It records the effective
recipe, recipe and input SHA-256 digests, the selected configuration file's
declared path and digest (or an explicit built-in-defaults marker), selected takes and windows, exact
source/base bone remap names and indices, and track
operation counts, start/end/delta facts for named translation tracks removed by
`strip_bones`, mesh selection, canonicalization flags, tool identity, and the
final artifact digest and counts. When pruning is enabled, each clip records
the exact removed tracks in `pruned_constant_tracks`: the index in the
completed, normalized output clip immediately before pruning (retaining its
pre-prune authored order), exact output bone name and index, TRS property,
interpolation, and key count.
The array is empty when pruning is disabled or no track is removed. See
[`character-assembly-evidence-v2.schema.json`](schemas/character-assembly-evidence-v2.schema.json).

Evidence v1 remains an immutable historical contract and does not describe
pruning. Consumers migrating from v1 must accept evidence v2 and verify the
recorded track list against the effective output clip; do not infer the list
from `required_bones`.

Given identical recipe bytes, input and config bytes, tool build, paths, and
platform, repeated runs emit byte-identical GLB and evidence. Evidence keeps
operator-declared paths rather than leaking canonical host paths.
