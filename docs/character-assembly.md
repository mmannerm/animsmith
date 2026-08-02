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
schema_version = 1
schema = "urn:animsmith:schema:character-assembly-recipe:1"
input_root = "inputs"
base_input = "base-character.fbx"
mesh_instances = ["character-mesh"]
complete_tracks = true
canonicalize_skin = true
ground_and_center = true
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

The base supplies the authoritative skeleton, rest pose, meshes, skins,
materials, and textures. `mesh_instances` names the exact base nodes whose
mesh instances survive; omitting it retains every instance. Unreferenced mesh
and material definitions are pruned.

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

An optional `material_texture_recipe` reuses the exact material-name recipe
boundary described in [Material texture recipes](material-texture-recipes.md),
including BaseColor, normal, metallic-roughness, and occlusion slots.

The normative recipe schema is
[`character-assembly-recipe-v1.schema.json`](schemas/character-assembly-recipe-v1.schema.json).

## Evidence and determinism

The evidence identity is
`urn:animsmith:schema:character-assembly-evidence:1`. It records the effective
recipe, recipe and input SHA-256 digests, the selected configuration file's
declared path and digest (or an explicit built-in-defaults marker), selected takes and windows, exact
source/base bone remap names and indices, and track
operation counts, start/end/delta facts for named translation tracks removed by
`strip_bones`, mesh selection, canonicalization flags, tool identity, and the
final artifact digest and counts. See
[`character-assembly-evidence-v1.schema.json`](schemas/character-assembly-evidence-v1.schema.json).

Given identical recipe bytes, input and config bytes, tool build, paths, and
platform, repeated runs emit byte-identical GLB and evidence. Evidence keeps
operator-declared paths rather than leaking canonical host paths.
