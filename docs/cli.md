# animsmith CLI

`animsmith` is designed for artist inner loops, CI gates, and pipeline
automation. It reads glTF/GLB everywhere; the released default build also
reads FBX through the `fbx` feature. The CLI is tested on Linux, macOS,
and Windows.

## Install

Install the released CLI by downloading the archive for your platform from
[GitHub Releases](https://github.com/mmannerm/animsmith/releases/latest):

<!-- release-targets:start -->
| Platform | Archive |
|---|---|
| Linux x86_64 | `animsmith-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz` |
| macOS Apple Silicon | `animsmith-vX.Y.Z-aarch64-apple-darwin.tar.gz` |
| Windows x86_64 | `animsmith-vX.Y.Z-x86_64-pc-windows-msvc.zip` |
<!-- release-targets:end -->

Each archive has a matching `.sha256` checksum file.

You can also install from crates.io with Cargo:

```console
cargo install animsmith
```

From a source checkout, prefix commands with `cargo run -p animsmith --`.

## Help

Every command has generated help:

```console
animsmith --help
animsmith lint --help
animsmith fix --help
animsmith generate addressability --help
animsmith generate contact-fragment --help
```

There are no man pages yet, so `--help` is the canonical installed CLI
reference. The help output reflects compile-time features: a
`--no-default-features` binary omits feature-gated commands such as
`report`, `convert`, and `assemble`.

## Commands

```console
animsmith inspect <file>
animsmith measure <file...> [--format text|json]
animsmith lint <file...> [--format text|json|markdown] [--select id[,id]] [--allow id[,id]] [--deny-warnings]
animsmith evaluate-transition-poses <input.glb|input.gltf|input.fbx> [--config animsmith.toml] --format json
animsmith collection lint <collection.toml> [--format json]
animsmith collection generate-contact-fragment <manifest.toml> --clip <logical-id> -o <out.json> [--format text|json]
animsmith collection evaluate-directional-speed --policy <policy.toml> --evidence <collection-output.json> [--format json]
animsmith collection evaluate-transition-poses <collection.toml> --families <transition-families.toml> --format json
animsmith report <file> -o <report.html> [--clip name]
animsmith transform <file> -o <out.glb> [--clip name] [--slice START:END] [--hold-extend SECONDS] [--gait-anchor] [--drop-duplicate-loop-endpoint] [--prune-constant-tracks] [--fps N]
animsmith fix <file> (-o <out.glb>|--in-place|--dry-run) [--repair id[,id]]
animsmith convert <in.fbx|in.glb|in.gltf> -o <out.glb|out.gltf> [--material-texture-recipe recipe.toml] [--animation-only|--bake-static-mesh-transforms] [--format text|json]
animsmith assemble <recipe.toml> -o <out.glb> --evidence <out.json> [--format text|json]
animsmith scale whole-document <in.glb|in.gltf> -o <out.glb|out.gltf> --factor N --evidence <out.json> [--format text|json]
animsmith scale rest-bind <in.glb|in.gltf> -o <out.glb|out.gltf> --source-skin-index N --source-root-node-index N --expected-factor N --evidence <out.json> [--format text|json]
animsmith generate addressability <in.glb|in.gltf> [--format json|text|markdown]
animsmith generate contact-fragment <in.glb|in.gltf|in.fbx> --clip <take-name> -o <out.json> [--format text|json]
animsmith diff <before> <after> [--format text|json]
```

`inspect` is the human-readable discovery view for exact asset-authored names.
It inventories clips, bones, materials, and mesh-instance nodes, including each
instance's mesh, skin status, and primitive/material context. Use those names
when authoring `assemble` or material texture recipes.

`--config animsmith.toml` is global for document-local commands. Without it, the CLI auto-loads
`./animsmith.toml` when present and otherwise uses built-in defaults.
Collection commands deliberately reject the global spelling. `collection lint`
uses each source config declared in the collection manifest, or exact built-in
defaults when none is declared; it never discovers an ambient
`./animsmith.toml`. `collection evaluate-directional-speed` has no config or
output-path option: its policy and evidence inputs fully declare its boundary.

`evaluate-transition-poses` is a JSON-only, single-result transition-family
contract, not a lint/check stream. It evaluates exact named/indexed takes in
the input against document-local `[transition_families."<id>"]` declarations
and never changes the input. When no config file is selected or found, its
declaration authority is the exact zero-byte TOML input; this is deliberately
the same authority as an explicitly empty config file. An absent or empty
transition-family table emits `no_configured_families` and exits 0. With
families, all complete passes exit 0; findings or any incomplete family emit
the immutable result and exit 1. Invalid config/declaration, input loading,
or output errors emit no result and exit 2. Collection transition-family
reload/evaluation is a separate command boundary.

`transform --gait-anchor` is an explicit declaration that each selected clip
is an in-place cyclic gait. Before rewriting, it samples the configured Root
role (or Hips fallback) and refuses the whole command if trajectory evidence is
missing/non-finite, horizontal accumulation exceeds 1 cm, or yaw accumulation
exceeds 1°. Every nonconstant channel the operation would rotate must contain
exactly one key at each `--fps` whole-frame sample over the clip duration, at
the exact representable f32 `key / fps` time and period endpoint. Sparse,
differently framed, duplicate-time, or off-grid trajectory evidence refuses,
as do duplicate `(bone, property)` channels (including constant channels), so
the phase shift remains a bijective authored-value permutation. Verification
samples the exact admitted f32 key times, and mutation permutes output values
by integer key index rather than resampling. Constant channels are exempt and
cannot influence the declared period or shift. Before sampling, the command
validates the complete skeleton,
resolved metric roles, track targets/cardinalities, and finite evidence. Both
declared-frame and maximum-authored-key work must independently fit these
inclusive 1,000,000-sample bounds: declared frames × skeleton bones, declared
frames × tracks, and maximum authored keys × skeleton bones. The translation
and yaw caps apply directly; no interior step is an allowance. Yaw uses f64
first/final headings plus counted full-turn crossings. At sample zero it
selects the local `+Z`, `+Y`, or `+X` axis with the greatest finite horizontal
projection, in that tie order, and keeps that axis for the complete proof. A
different source-axis convention is therefore accepted without per-sample
axis switching; if the selected axis later has no horizontal projection, the
command refuses. This avoids error growth with the admitted segment count.
Four f32 successors at each inclusive cap
cover only authored endpoint translation/quaternion quantization; other checks
are unchanged. A refusal names the clip and selected bone, does not publish or
replace the requested output, and emits no earlier per-clip success lines:
standalone transform stdout is buffered until all selected clips and the
artifact write succeed. Keep authored root motion unchanged, apply a
runtime phase offset, or use separately designed trajectory-preserving tooling.
The option does not convert root motion to in-place motion.

`scale` is the atomic linear-scale producer. It has two distinct
subcommands because the two operations rewrite different domains and a factor
alone does not identify which one was meant: `whole-document` converts every
represented length by a declared factor (physical size changes; use it only
when the source was authored in a different linear unit), and `rest-bind`
removes one compensating inherited scale from the skinned hierarchy anchored
at a declared source skin and source root node, preserving world joint
translations and orientations, sampled trajectories, and skinned vertex
positions while removing the composed scale. The
[scale workflow](scale.md) owns the operation-choice, glTF raw rewrite or
narrow FBX staging, proof, publication, and support-boundary walkthrough; this page remains the
installed command/flag/exit reference.

Every numeric and source-identity argument is required. Nothing is inferred
from mesh bounds, character height, joint lengths, inverse-bind magnitude,
filename, or asset category; there is no implicit first skin or root, no
`animsmith.toml` key, no plan file, no in-place mode, and no per-run tolerance
flag — the tolerance policy is fixed and its identity is recorded in the
evidence. Input, output, and evidence paths must all be distinct. glTF/GLB
keeps its input container; the narrow FBX `rest-bind` path emits a new `.glb`.

Accepted inputs are self-contained glTF/GLB for both operations, plus a
complete-inventory `.fbx` for narrow `rest-bind` when the default FBX feature
is enabled. See the
[scale workflow](scale.md#supported-source-boundary) for the complete support
boundary and rewrite/reload/proof sequence, and the
[output reference](output.md#scale) for typed refusal
records and deterministic publication details.

`scale` splits its two failure codes by what the failure is a property of, not
by how far the run got. A refusal that is a property of the **input asset**
publishes nothing, leaves any prior pair byte-identical, and exits 1 — with
the typed reason as prose on stderr under `--format text` and as the same
evidence record with `outcome: "rejected"` on stdout under `--format json`.
That includes bytes that do not parse as the glTF/GLB the extension declares,
and a document whose size puts the sampled proof over the policy's work
budget. A failure that is a property of the **invocation** or of the
operator's filesystem exits 2 with prose on stderr: a declared factor that is
not finite and positive, an input that cannot be opened, a wrong extension, a
container the extension disagrees with, two arguments naming one file, or a
missing output directory.

The three paths must name three different files, and each is compared by the
file it actually reaches: the input is resolved through a symbolic link,
because reading follows one, while a destination is not, because publishing
renames *over* a link rather than through it. Passing a symlinked input
alongside a destination naming its target is refused before anything is
written. A symlinked input that aliases neither destination is accepted.

`transform --drop-duplicate-loop-endpoint` is the narrow mechanical transform
for an inclusive DCC cycle export that copied frame 0 to the final frame. It
accepts only a strict authored-key candidate: all channels must have one common
finite, strictly increasing timeline; valid key cardinality; matching first and
last values (vector components within `1e-5`, sign-invariant quaternion angle
within `1e-4` radians);
and actual interior motion. It atomically removes the same complete terminal
key count from every channel (including each cubic-spline key triplet), leaves
retained data unchanged, and re-pins duration to the last retained key. Other
clips are not generalized, retimed, or repaired by this flag. The resulting
open-cycle representation no longer satisfies inclusive `loop-closure`, which
expects a repeated final sample; the complete endpoint-mode classifier remains
[#22](https://github.com/mmannerm/animsmith/issues/22).

`transform --prune-constant-tracks` is an opt-in companion to the default
`constant-track` note. It removes only multi-key translation, rotation, and
scale tracks whose evaluated keyed values are constant: vector components may
vary by at most `1e-4`, and rotations by at most `1e-3` radians under
sign-invariant quaternion comparison. Single-key pins, malformed/non-finite
data, and cubic-spline tangents that create motion above tolerance are not
candidates and remain unchanged. For each candidate, the transform either
removes it or prints a refusal reason: removing it would change sampled local
TRS or model-space position/rotation, it is the clip's last remaining track, sampling is unsafe,
or it targets a bone named by that clip's effective `animates_bones`
expectation. The latter preserves the evidence needed for `missing-bones` and
`frozen-bone`; `[rig] required_bones` does not protect tracks because it is a
skeleton-presence contract.

Pruning runs after any selected slice, hold extension, duplicate-endpoint
removal, and gait anchoring, so the final clip is what is judged. Text output
lists every removed or retained candidate with its original track index, bone,
property, interpolation, key count, and (for retained tracks) reason; no
machine-readable output schema changes. It does not reduce changing keys,
rewrite DCC curves, remove custom curves AnimSmith does not model, decide
whether a non-rest constant pin is artistically intentional, or repair cubic
tangents.

## Exit Codes

| Code | Meaning |
|---:|---|
| 0 | No failing findings and no required-unavailable engine prediction facets; warnings, notes, or ordinary coverage gaps may remain. |
| 1 | At least one failing finding, any `required_prediction_unavailable` facet (including an embedded Bevy addressability evaluation), an incomplete `collection lint` result, an incomplete or not-evaluable directional-speed or transition-pose evaluation, a significant `diff`, pending repairs under `fix --dry-run`, or a `scale`, `convert`, or `assemble` refusal that is a property of source asset bytes. |
| 2 | Operator/tool error: unopenable input, bad config, unsupported format, or invalid flags. |

The code reports what the run *did*, never how well it could report it. This
holds for parser-rendered help/version, text, Markdown, and every `--format json`
path (`measure`, `lint`,
`evaluate-transition-poses`, `collection evaluate-transition-poses`, `collection lint`, `collection evaluate-directional-speed`,
`diff`, `convert`, `assemble`, `scale`, `generate addressability`). If
stdout cannot accept the result — a closed pipe or full filesystem — the
checked write never panics, a best-effort checked diagnostic goes to stderr,
and the stdout-bearing path's already-established code stands **except for
`evaluate-transition-poses` or `collection evaluate-transition-poses`**. These immutable results have no
sidecar or previously established outcome: a failed stdout write is an
operator error, produces no usable result, and exits 2. Thus
`lint … --format text | head` still
exits `1` for findings it found, `inspect … | head` still exits `0` for an
inspection it completed, and `scale` still exits `1` for a refusal or `0` for
a published pair. Stderr may itself be closed; losing both streams is still
not a panic. Raising the stdout failure instead would report an operator error
for work that was actually done and make exit semantics depend on presentation
format. JSON serialization failure remains exit `2` because the CLI could not
form a truthful record; delivery failure after rendering is only reporting for
the ordinary streams. Other operator errors occur before stdout reporting,
remain stderr-only, and retain exit `2`.

For `convert` and `assemble`, classification is by typed provenance, never by
diagnostic wording. The JSON refusal identity is
`urn:animsmith:schema:producer-refusal:1`:

| Class | Examples | Code and streams |
|---|---|---|
| Asset-property refusal | Source bytes do not parse or are structurally unsupported; a valid recipe names a missing/ambiguous take, node, mesh, or material; static bake, canonicalization, gait, scale, proof, or output representability rejects the asset. | `1`; JSON writes one `producer-refusal:1` record to stdout and nothing to stderr, while text writes one escaped typed refusal to stderr and nothing to stdout. |
| Operator error | Invalid flags/config; recipe syntax, schema, or intrinsic values; unsupported extension; missing/unreadable declared file; unsafe, aliased, or unwritable paths; temporary/write/publication/rollback I/O; refusal-record serialization. | `2`; prose on stderr and nothing on stdout in either format. |

A refusal is established before publication. It leaves any prior assembly
artifact/evidence pair and any prior convert artifact byte-identical. Convert
still writes its successful single artifact directly, so this is not a
rollback promise for an operator I/O failure during that write.
Commands that render several related pieces attempt one checked stream when a
single delivery boundary is promised. In particular, all selected `fix`
repair reports and all parts of one conversion summary produce at most one
stdout-failure diagnostic.
Help and version delivery uses clap's own fallible styled writer, so forced
ANSI color remains intact while a closed destination still follows this rule.
Only stdout is affected; nothing about artifact publication changes.

A role-dependent check with missing prerequisites reports a typed coverage
gap and does not fail the run — exit `0` means no failing findings among the
checks that evaluated, not that every declared check evaluated; see
[reading a lint run](game-ready-clips.md#reading-a-lint-run) for the
full outcome vocabulary. A required-unavailable engine prediction is distinct:
it exits `1` and cannot be suppressed by severity or `--allow`.

Use `lint --deny-warnings` when CI should fail on warnings as well as
errors. `fix --dry-run` is the repair check mode: it exits 1 when the
file has repairable defects and 0 otherwise, so CI can gate on "this
asset needs fixing" without writing anything. The exit code
reflects repairs `fix` would actually perform: tracks it cannot patch
(data-URI buffers, cubic `quat-norm` tracks, quantized rotations) are printed as
`skipped[...]` but do not fail the check — gate on `lint` (the
`quat-norm` or `quat-flip` checks) when detection alone should fail CI.

## Unsupported glTF Encodings

AnimSmith reads a narrower slice of glTF than the specification permits. A
mesh accessor it cannot decode exactly as declared is refused with exit `2`
rather than read as something else, so a run never reports numbers derived
from bytes it misread.

Integer `TEXCOORD_0` and `WEIGHTS_0` accessors are accepted only when they
declare `"normalized": true`, as glTF requires. The upstream reader rescales
`UNSIGNED_BYTE` and `UNSIGNED_SHORT` values even when that flag is missing;
AnimSmith refuses the unflagged accessor before decoding so `measure` cannot
present an unauthorized rescaling as authored data. Normalized integer and
`FLOAT` forms retain their decoded values. Re-export the malformed attribute
with the flag, or as `FLOAT`; the refusal identifies its mesh, primitive,
semantic, and accessor.

The common case is **`KHR_mesh_quantization`**: gltfpack and similar mesh
optimizers store `POSITION` and `NORMAL` as `BYTE`/`SHORT` instead of
`FLOAT`, and AnimSmith has no decoder for those. Which message you get
depends on how the file declares the extension:

```console
$ animsmith lint quantized.glb
animsmith: glTF parse error: invalid glTF: extensionsRequired[0] = "KHR_mesh_quantization": Unsupported extension;

$ animsmith lint quantized-declared-used-only.glb
animsmith: mesh 0 primitive 0 POSITION: accessor 0 is VEC3 of SHORT, but the loader reads VEC3 of FLOAT
```

Re-export without quantization — `gltfpack -noq`, or turn off the
quantization option in whichever exporter produced the file — and the same
asset loads. AnimSmith measures rigs and animation, so the unquantized
source is the right input for it; keep the quantized build for shipping.

Two other shapes are refused with the same `mesh N primitive M` message: an
accessor typed for a different element than the slot that references it (a
`VEC3` `TEXCOORD_0`), and one whose buffer layout the reader cannot walk (a
`byteStride` shorter than its own element, a `sparse` block of count 0, or a
dense/sparse byte extent beyond its declared buffer view or the external,
data-URI, or GLB bytes that actually resolved). Short extents are refused
rather than silently loaded as empty positions or indices, so a clean
measurement never substitutes absent geometry for unreadable authored values.
Those are invalid glTF rather than an unsupported feature, and the fix
belongs at the source. The [Khronos
glTF-Validator](https://github.com/KhronosGroup/glTF-Validator) names the
same accessor — `ACCESSOR_SMALL_BYTESTRIDE` for the stride, and the schema's
`minimum: 1` on `sparse.count` for the sparse block — so run it on the file
to see the defect in its own vocabulary. AnimSmith still has to check these
itself, because the `gltf` crate it parses with does not: that crate's JSON
validation bounds `byteStride` to `4..=252` without ever relating it to the
accessor's element, so an unwalkable layout passes parsing and reaches the
reader.

The same layout defect on an **animation sampler** is refused too, naming
the clip and channel instead of a mesh:

```console
$ animsmith lint poisoned-track.gltf
animsmith: clip 'walk' node 3 sampler input: accessor 0 reads its elements from buffer view 0, whose byteOffset 18446744073709551615 plus byteLength 12 is a byte extent that overflows
```

Sampler **element encodings** are checked independently before the selected
reader is constructed. Inputs must be `SCALAR` of `FLOAT`; translation and
scale outputs must be `VEC3` of `FLOAT`; rotation outputs must be `VEC4` and
may use any of glTF's five decodable quaternion component types (`BYTE`,
`UNSIGNED_BYTE`, `SHORT`, `UNSIGNED_SHORT`, or `FLOAT`); morph-weight output
uses those same five component types as `SCALAR`. A mismatched type or
component type is an operator error rather than a panic or a size-coincident
reinterpretation:

```console
$ animsmith lint mistyped-track.gltf
animsmith: animation 0 sampler 0 input for node 0 translation: accessor 0 is VEC3 of FLOAT, but the loader reads SCALAR of FLOAT
```

## Feature Flags

The default binary enables `fbx` and `report`.

```console
cargo install animsmith
cargo install animsmith --no-default-features
```

The no-default-features build has no C toolchain dependency and keeps the
glTF-only workflow: `inspect`, `measure`, `lint`, `transform`, `fix`, `scale`,
and `diff`. `scale` is the minimal build's evidence-emitting producer. The HTML `report` command is controlled by the `report` feature.
`convert` accepts FBX or glTF input (a glTF input is re-emitted,
carrying its geometry) but is compiled only with the `fbx` feature.
`assemble` is gated by the same feature because its maintained boundary accepts
FBX sources as well as glTF.
Full-scene conversion carries factor-only materials plus linked or embedded
PNG/JPEG base-color, normal, metallic-roughness, and occlusion textures. Normal textures retain their glTF
scale; FBX normal maps use glTF's default scale because ordinary FBX materials
do not expose the same scalar. See the
[static asset workflow guide](static-asset-workflows.md#preserve-normal-maps-as-data-not-color)
for why the normal slot matters, common bad handoffs, and the engine-side checks
that conversion cannot perform.

`--material-texture-recipe <PATH>` applies explicit BaseColor, normal, metallic-roughness, and occlusion image
mappings during conversion. It conflicts with `--animation-only`, which removes
materials, and is compatible with `--bake-static-mesh-transforms` and either
`--format` value. Without a recipe, ordinary linked and embedded textures keep
their existing conversion path. See [material texture recipes](material-texture-recipes.md)
for the recipe contract, deterministic processing policy, and path rules.

`assemble` reads a [versioned multi-source character recipe](character-assembly.md),
uses one input as the authoritative skinned base, and exact-name remaps selected
takes from other FBX or glTF inputs. It writes a GLB and assembly-evidence JSON
as a rollback-safe publication pair. The command owns generic asset transforms;
source extraction, project policy, and publication remain consumer concerns.
The current recipe/evidence pair is v7. Recipe v7 may include an optional
`[rest_bind_scale]` block whose exact `root_node_name` and expected factor are
both required. The base must resolve that name to exactly one source node and
exactly one non-empty source skin whose every joint is that node or its
descendant. Every distinct FBX clip input uses an animation-only projection.
A glTF/GLB clip retains its existing successful full rest/bind or meshless
track-only path; role-specific admission additionally selects the track-only
path when only unused geometry, deformation, material, or bind obligations
would fail. Framing, dependency, raw-coverage, named-skeleton, and animation
accessor/layout obligations remain strict. The compatibility domain contains the selected base skin joints,
the root, their named ancestry, and each actual track target with its ancestry.
Unreferenced base-only geometry or attachment descendants are not required in
the clip.
Boundary whitespace is invalid rather than
trimmed. The block accepts
glTF/GLB plus the narrow normalized/baked FBX subset admitted by the existing
rest-bind capability boundary. That boundary may admit user-defined FBX
properties and bounded external texture/video declarations after same-load
evidence proves they are not scale-bearing. It also admits exactly ufbx's
marker, LOD-group, stereo-camera, camera-switcher, and display-layer typed
lists because they cannot supply hierarchy transforms, skin binds, tracks, or
geometry to the normalized rest/bind bridge. Display layers contribute only
node membership and editor visibility/freeze/color state. The boundary admits
shader/binding-table metadata on the same basis. BindPose rows are admitted
only when they cover every joint of each skin they touch and are finite,
unambiguous, and reconcile with the converted
cluster bind or node rest-world matrices already consumed by the bridge; no
Pose remains required. Those rows remain counted in the raw aggregate; every
other unmodeled typed list stays
fail-closed, with exact nonzero kind counts in the refusal. The boundary still
may admit enumerated scale-invariant conversion fidelity—omitted authored
vertex/face/edge metadata, influence projection with complete effective
coverage, triangulation, and exact-bit welding—while retaining the public
inventory evidence. It refuses incomplete resource or
construct coverage, extensions, and every unsupported transform, geometry,
bind, or animation fact. A refusal names the exact failed fact or counter. An
artifact or evidence destination that names
one of the safe source-relative dependency keys retained by the same-load
closure is an operator error even when capture was unavailable or refused;
symlink-mediated dependency keys stop before publication rather than resolving
through the link, and resource-budget truncation stops rather than trusting an
unchecked tail. Publication never replaces a source sidecar. The command
validates the base plus every clip's versioned skeleton basis before
remapping any keys. V7 retains v6's composition with
`canonicalize_skin`, `ground_and_center`, and `remove_nodes`: it applies those
normalized assembly transforms to both the staged source and the rebased clip
reference, then performs one final raw staged-GLB rewrite and proof. Recipe v4
retains its released refusal for that combination. Set `prune_constant_tracks = true` to
remove only tracks proven constant after all other transforms, including track
completion. Because this is all-property pruning, it can remove
completion-generated `(bone, property)` coverage whose completed value is
constant. Effective output clip `animates_bones` exact names are retained,
while `required_bones` is never used as the carve-out. Evidence records every
removed track and is empty when pruning is disabled or removes nothing. Leave
pruning disabled where consumers need dense transition coverage and do not
explicitly reset omitted properties; property-scoped selection is tracked in
[#401](https://github.com/mmannerm/animsmith/issues/401).
Root-level `remove_nodes` exact-names base nodes and removes their descendant
closure after animation transforms; any surviving track, skinned mesh-instance,
or skin reference refuses the operation. With FBX `rest_bind_scale`, an
unskinned mesh instance attached inside that declared closure is excluded from
the private scale stage and removed with the node. It still blocks the scale
plan when the node is not declared for removal.
For every track-only clip input, v7 applies the accepted base plan's named
animation-target factors (including cubic translation tangents) through the
animation-only projection without inventing a bind rewrite. The named root and
selected base skin-joint basis must still match,
as must every actual track target and the named ancestry connecting those
nodes. Unreferenced base-only geometry descendants do not participate in that
clip proof. The projection never applies to the base; every base and full
rest/bind path retains the strict raw capability contract.
It performs no material, texture, or mesh garbage collection. Recipe/evidence
v1 through v6 remain immutable historical contracts; v3 rejects
`rest_bind_scale` as unknown.
The recipe identity is
`urn:animsmith:schema:character-assembly-recipe:7`; see
[`character-assembly-recipe-v7.schema.json`](schemas/character-assembly-recipe-v7.schema.json).

## Static mesh transform bake

`convert --bake-static-mesh-transforms` is an explicit, opt-in conversion
operation for a static mesh whose normalized placement must live directly in
mesh-local geometry. It accumulates each accepted mesh node's rest transform
through its hierarchy into positions, transforms normals with the
inverse-transpose and normalizes them, and writes the result beneath a
canonical identity root. Indices, UVs, model-supported material assignments,
and embedded base-color, normal, metallic-roughness, and occlusion textures are retained. The default
conversion is unchanged; `--bake-static-mesh-transforms` conflicts with
`--animation-only`.

The operation fails with exit code 2 rather than guessing when the input has
any animation track, a skin signal, a mesh definition with no unambiguous
single instance (including shared definitions), malformed or non-finite scene
data, a singular or near-singular transform, or a reflection. Skin baking,
animated-node baking, and reflection handling are outside this operation's
contract. Same-platform runs over the same input and options produce a
byte-identical artifact. The
[static asset workflow guide](static-asset-workflows.md#bake-static-placement-into-geometry)
shows the before/after model, when baking is appropriate, and why this is not a
general scene flattener.

## Repairs

Every repair is safe, lossless, and idempotent — that is the bar for
adding one. Repairs have stable ids so scripts can pin exact behavior:

| Repair id | Behavior |
|---|---|
| `quat-norm` | Unit-normalizes finite, non-zero LINEAR/STEP quaternion keys. This is lossless because scaling a quaternion does not change the represented rotation after normalization. CUBICSPLINE tracks are skipped to preserve tangents. |
| `quat-flip` | Normalizes adjacent quaternion keys to the same hemisphere. This is lossless because `q` and `-q` represent the same rotation. |

By default `fix` runs every repair. `--repair id[,id]` pins an exact
list (`animsmith fix --help` names the valid ids). `fix` writes only
when you explicitly choose a destination; `--dry-run` reports and sets
the exit code without writing:

```console
animsmith fix clip.glb --dry-run
animsmith fix clip.glb -o fixed.glb
animsmith fix clip.glb --in-place
animsmith fix clip.glb --repair quat-norm,quat-flip -o fixed.glb
```

## Engine profiles and importer settings

An optional `[engine]` section selects one exact, versioned importer contract.
There is no generic, automatic, nearest-version, or fallback profile. V1
accepts only these tuples:

| profile | revision | engine version | importer | accepted source |
|---|---:|---|---|---|
| `unity-generic` | 1 | `6000.3` | `fbx-model-importer` | FBX |
| `unity-humanoid` | 1 | `6000.3` | `fbx-model-importer` | FBX |
| `unreal` | 1 | `5.8` | `fbx-importer` | FBX |
| `godot` | 1 | `4.7` | `resource-importer-scene` | glTF, GLB, or FBX |
| `bevy` | 1 | `0.19.0` | `gltf-asset-loader` | glTF or GLB |

The accepted-source column is AnimSmith's V1 profile boundary, not a claim
that the named engine supports no other source formats. An absent `[engine]`
keeps the existing engine-neutral behavior.

Unity exposes the only V1 setting vocabulary. Every applicable setting is
required because the cited Unity 6000.3 documentation does not establish a
default that AnimSmith can safely materialize:

```toml
[engine]
profile = "unity-generic"
profile_revision = 1
engine_version = "6000.3"
importer = "fbx-model-importer"

[engine.settings]
convert_units = true
bake_axis_conversion = true
root_motion_source = "Reference/Root"

[clips."locomotion_*".engine_settings]
root_rotation = "extract"
root_position_y = "bake"
root_position_xz = "extract"

[clips.idle.engine_settings]
root_rotation = "bake"
root_position_y = "bake"
root_position_xz = "bake"
```

`root_motion_source` is an exact document-scoped source-transform path for
Unity Generic. It is not applicable to Unity Humanoid. The three
`bake | extract` choices are clip-scoped for both Unity profiles and resolve
with the normal field-by-field rule: matching globs in lexical order, then an
exact clip name last. Every real clip must resolve all three choices. Unreal,
Godot, and Bevy expose no V1 settings, so supplying any setting is an error.

All statically knowable tuple, setting, value, scope, and applicability errors
are reported before input I/O. Accepted input format and required per-clip
materialization are checked after loading. Profile selection never changes
`measure` values. Output v11 records resolved profile provenance on lint files.

The first production rule is `engine-addressability` for the exact Bevy
revision 1 / 0.19.0 / `gltf-asset-loader` tuple. With complete glTF/GLB source
animation inventory, it emits one available facet whose subject is the exact
Bevy display label `Animation{i}` for each source animation index `i`:

```console
animsmith --config examples/bevy.animsmith.toml lint \
  --select engine-addressability examples/assets/walk.glb
```

Named, unnamed, and duplicate-named animations all use their distinct source
indices; names do not become typed labels. Partial or unavailable source clip
coverage emits one blocking `animation_asset_label_inventory` facet rather
than predictions for the retained prefix. The rule predicts only the canonical
`GltfAssetLabel::Animation(i)` selector spelling. It does not prove Bevy ran,
that animation loading was enabled, that the runtime asset exists, or that its
targets and graph wiring are usable. The selector can change when source
animation order changes.

Current lint uses prediction provenance v2 with bounded 4,096/N+1 settings
coverage. A 4,097th clip is retained as typed partial-settings overflow
evidence, never as a complete prefix.

`generate addressability` packages the same immutable raw-source evidence as a
standalone, animation-only contract. It emits canonical JSON by default:

```console
animsmith generate addressability examples/assets/walk.glb \
  > walk.addressability.json
```

The neutral inventory retains source-order animation and channel indices,
optional non-unique source names, channel targets and accessor indices, the
primary-file identity, and the full dependency closure. Its identity covers
only those neutral fields. Without an engine profile, or with a supported
profile other than the exact Bevy tuple above, the root `bevy` field is null.
With the exact Bevy profile, it embeds same-load prediction provenance and the
unchanged `engine-addressability` evaluation:

```console
animsmith --config examples/bevy.animsmith.toml generate addressability \
  examples/assets/walk.glb --format markdown
```

Names remain metadata and never replace source indices. Partial or unavailable
animation coverage stays visible in the neutral inventory. When the exact Bevy
adapter is active, the existing required-unavailable facet makes the command
exit 1; without that adapter the same neutral evidence exits 0. Non-glTF input,
malformed or unknown profile selections, and an actual 4,097-clip input are
operator errors (exit 2). Text and Markdown are escaped presentation views of
the same validated value and add no conclusions to the JSON contract. This V1
does not inventory scenes, default scenes, skins, target paths or UUIDs, Bevy
named-map winners, or extension support, and it does not certify a runtime
load. Consumers using the strict staged reader must also keep each report at or
below 256 MiB; the reader enforces this byte cap before UTF-8 or JSON decoding.

`generate contact-fragment` publishes canonical `contact-fragment:1` bytes to
`--output`. It selects one exact unique clip, samples the existing
longest-authored-channel metric grid, and records only finite bilateral
model-space stance support. Each retained two-or-more-sample run emits one
normalized support window and one earliest-minimum marker. The source primary
identity and complete dependency closure are bound into the fragment; an
incomplete role, grid, closure, or other prerequisite is a typed exit-1
refusal and leaves any existing output unchanged. `--format json` writes the
same canonical bytes to stdout; text is a presentation summary. The collection
form selects one exact manifest logical id and reloads its declared
source/config rather than consuming collection-output evidence. Neither form
infers physical contact, footsteps, gameplay, IK, or engine behavior.

An output-v14 measure report deliberately has no engine provenance or
loader-owned source format. `diff` also ignores the provenance on lint reports.
When its operands are JSON reports, `diff` validates the complete
version-matched records and compares their decoded version-matched measurements
(historical v15 or current v16); a selected engine profile does not change
that report meaning. When its operands are source assets, the profile is still
resolved against each loader-owned source format before measurement.

Runtime-facing attachment/socket/IK nodes have one shared engine-neutral
policy:

```toml
[runtime_nodes]
selectors = ["weapon_socket", "ik_*_target"]

[checks.rest-world-scale]
expected_uniform_scale = 1.0
uniform_scale_tolerance = 0.0001
```

The older `[checks.rest-world-scale].node_selectors` field remains a
compatibility alias. Declaring both spellings is a configuration error. An
absent selector field or explicit empty list means no runtime-node policy.

## Machine Output

`measure`, `lint`, `collection lint`, `collection evaluate-directional-speed`,
`diff`, `generate addressability`, and
`generate import-advice` support
`--format json`. The native JSON contract is the source of truth and is
versioned with `schema_version`.
See [output.md](output.md) and the current
`urn:animsmith:schema:output:14` [`output-v14.schema.json`](schemas/output-v14.schema.json). Nested measurement
evidence has its own
`urn:animsmith:schema:measurements:16`
[`measurements-v16.schema.json`](schemas/measurements-v16.schema.json) contract.
`urn:animsmith:schema:measurements:15`, `urn:animsmith:schema:output:11`,
`urn:animsmith:schema:output:12`, and `urn:animsmith:schema:output:13` remain
historical immutable contracts. `diff`
retains strict version-matched readers for output-v11/v12 with
measurements-v15 and output-v13/v14 with measurements-v16; output-v10 and earlier
reports require regeneration from the original asset.

`collection lint COLLECTION.toml --format json` emits the separate immutable
`urn:animsmith:schema:collection-output:6` contract. Historical
`urn:animsmith:schema:collection-output:5` and
`urn:animsmith:schema:collection-output:4` remain immutable; the manifest directory is
the control root; safe missing/unreadable sources, rejected readable bytes,
digest/take mismatches, and incomplete runtime-set members remain typed rows
and exit 1 while later safe sources continue. Invalid manifests, unsafe or
nonregular paths, and missing/malformed selected configs exit 2 without an
envelope. Source, clip, and set rows are canonical; member order remains the
manifest order. Each source separately retains its primary input identity and
a complete, partial, or unavailable dependency-closure state. Only a complete
closure identity can establish that source, one of its clips, or a runtime-set
member; partial/unavailable reasons remain typed and make the result incomplete
with exit 1. Each established clip binds an exact source take index/name
to a normalized clip index and carries duplicate-safe indexed measurements.
The nested whole-document lint envelope advances to output v14 and measurements
v16. The strict reader preserves version binding for collection-output-v5 with
output-v13; older historical contracts remain immutable and are not retargeted. Regenerate current collection evidence
before passing it to collection evaluators.
Runtime sets keep `decision: not_evaluated`; they make no blend, controller,
engine, artistic, or gameplay claim. Every declared member carries raw
`root_travel`: its existing `duration_s`, root-trajectory translation
availability, signed horizontal X/Z displacement, sampled horizontal travel,
and speed availability/value. Binding-unavailable rows remain explicit with
unavailable root-travel facts. `evidence.root_travel` reports the count of
fully measured declared members and a complete/incomplete lifecycle; it never
reduces the declared set to measurable rows. Gait-group member rows additionally
carry raw gait-phase availability, and only a fully established, phase-measured
set emits `evidence.gait_phase.phase_spread` with basis
`max_circular_deviation_from_mean`; this preserves existing gait lint
threshold semantics. See
[`collection-output-v6.schema.json`](schemas/collection-output-v6.schema.json).

`collection evaluate-directional-speed --policy POLICY.toml --evidence
COLLECTION-OUTPUT.json --format json` strictly reads a bounded
`collection-directional-speed-policy:1` declaration and a bounded
collection-output V6 or historical V5 document, then writes the separate immutable
`urn:animsmith:schema:collection-directional-speed-evaluation:1` result. The
result binds the exact raw TOML and JSON byte identities and preserves every
declared member in manifest order. Invalid, stale, wrong-kind, unreadable,
malformed, or over-budget inputs exit 2 with no stdout. Incomplete root travel,
zero endpoint displacement, zero ratio reference, numeric-range outcomes, and
declared-policy findings write the result and exit 1; only a complete passing
policy exits 0. It has no text/Markdown presentation, `--output`, subset, or
inference mode. See
[`collection-directional-speed-evaluation-v1.schema.json`](schemas/collection-directional-speed-evaluation-v1.schema.json).

`generate addressability` has a separate immutable contract,
`urn:animsmith:schema:gltf-animation-addressability:1`; see
[output.md](output.md#gltf-animation-addressability) and
[`gltf-animation-addressability-v1.schema.json`](schemas/gltf-animation-addressability-v1.schema.json).
It is not output-v11 and cannot be used as a `diff` measurement operand.

`generate contact-fragment` is also outside output-v11: it writes the strict
`urn:animsmith:schema:contact-fragment:1` sidecar. Its exit-1 JSON refusal
uses `producer-refusal:1`; exit 2 is reserved for CLI, config, path, or
publication control failures.

`generate import-advice` requires an exact `[engine]` selection and all
required settings, then binds that resolved profile to one input and the
same-load raw facts, dependency closure, explicit clip intent, and normalized
measurements:

```console
animsmith --config unity.animsmith.toml generate import-advice export.fbx
animsmith --config unity.animsmith.toml generate import-advice export.fbx \
  --format markdown
```

The default JSON is the separate immutable contract
`urn:animsmith:schema:engine-import-advice:1`; see
[output.md](output.md#engine-import-advice) and
[`engine-import-advice-v1.schema.json`](schemas/engine-import-advice-v1.schema.json).
The strict reader caps the serialized document at 256 MiB before decoding.
Unity 6000.3 Generic/Humanoid projects only the resolved importer settings
modeled by profile revision 1. Unreal 5.8 and Godot 4.7 revision 1 return a
typed `profile_settings_unmodeled` refusal (exit 1). Available advice exits 0;
config, unsupported-profile/input-format, I/O, and serialization errors exit
2. The command never invents authored frame numbers, sampling rates, unit
conversion, or root-motion behavior. Text and Markdown are presentation-only
views of the same validated value.

Measurements v16 retains the v15 clip evidence and adds source-order primitive
geometry plus bounded leading-magic evidence for unsupported nonempty images.
Measurements v15 added canonical per-bone local TRS channel coverage and
sampled Root/Hips trajectory evidence. Root is preferred whenever that role
resolves; Hips is only a typed fallback when Root is unresolved. The
trajectory groups signed endpoint displacement, sampled horizontal travel and
vertical extrema, plus net/unwrapped/travel yaw with independent translation
and yaw availability. These are engine-neutral normalized-model-space
regression facts from the shared uniform metric grid, not continuous-curve or
engine root-motion extraction proof; see [output.md](output.md#measure-and-lint)
for the full coordinate, sampling, and availability contract.

`rest-world-scale` is quiet until its config resolves a nonempty shared or
legacy runtime-node selector policy.
Each exact name or `*` glob must resolve to one source node; findings include
the stable source-node path and ancestry so attachment/import policies can be
traced back to the source projection. For glTF that projection is authored
node state; for FBX it is ufbx-normalized metre/Y-up, adjusted/inheritance-
compensated state rather than the raw FBX transform stack. See the
[selected-node scale workflow](game-ready-clips.md#attachment-nodes-and-inherited-rest-world-scale).

`convert --format json` emits conversion evidence v2, with immutable identity
`urn:animsmith:schema:conversion-evidence:2`; see [output.md](output.md) and
[`conversion-evidence-v2.schema.json`](schemas/conversion-evidence-v2.schema.json).
It records the requested options, counts from the written artifact, exact
static-mesh transforms when requested, and recipe provenance when a material
texture recipe is used. `text` is the default human-readable write summary.
An asset refusal under `--format json` uses the independent immutable
[`producer-refusal-v1.schema.json`](schemas/producer-refusal-v1.schema.json)
identity instead; conversion evidence v2 remains success-only and unchanged.

`assemble` writes evidence v7 to its required `--evidence` path, with immutable
identity `urn:animsmith:schema:character-assembly-evidence:7`; see
[`character-assembly-evidence-v7.schema.json`](schemas/character-assembly-evidence-v7.schema.json).
`assemble --format json` prints the same record to stdout — the identical bytes
the evidence file receives, serialized once — in place of the default `text`
publication summary. An asset refusal publishes neither member and exits `1`;
JSON emits `producer-refusal:1` on stdout, while text emits its stable kind on
stderr. Operator errors remain exit `2`, stderr-only. A failed stdout delivery
does not change an already-established success or refusal code.

`scale` writes scale evidence v4 for glTF/GLB to its required `--evidence`
path, with immutable identity `urn:animsmith:schema:scale-evidence:4`; see
[output.md](output.md) and
[`scale-evidence-v4.schema.json`](schemas/scale-evidence-v4.schema.json). The
same record is what `scale --format json` prints to stdout, for a refusal as
well as for a published pair. For an `artifact-proof-failed` refusal from the
exact-preservation walk, `rejection.artifact_proof_differences` names up to 16
raw JSON locations the walk found different; `omitted` counts locations beyond
that fixed cap, and the full count is `items.length + omitted`. The field is
`null` for other artifact-proof claims and for capability refusals, whose
`violations` array retains its existing meaning.

With the default FBX feature, `scale rest-bind INPUT.fbx -o OUTPUT.glb` is a
separate narrow producer contract. It accepts only the complete normalized
ufbx inventory, emits the immutable
`urn:animsmith:schema:scale-evidence:5` record described by
[`scale-evidence-v5.schema.json`](schemas/scale-evidence-v5.schema.json), and
proves the exact re-encoded GLB after reload. It never writes FBX and makes no
raw-FBX preservation claim. `scale whole-document` remains glTF/GLB-only.

Machine-readable lint rejects `--allow` so it cannot erase evidence. The flag
remains a presentation and exit-policy convenience for text and Markdown.

Native JSON is deliberately shaped so serializers can be added later
without redesigning the checks: SARIF for code scanning, GitLab Code
Quality/CodeClimate for MR widgets, JUnit XML for CI dashboards, and CSV
for ad-hoc analysis.

Each JSON `measure` and `lint` file row records an `input` SHA-256 and byte
count for the exact primary file bytes parsed. Keep that row with retained
review, promotion, or publication evidence; multi-file runs calculate one
identity per argument in order. For `.gltf`, the identity deliberately does
not cover external buffer or image files. Profiled lint embeds the bounded
same-load dependency closure in `prediction_provenance`; other workflows must
retain dependency provenance separately when that scope matters.

Human-readable command output keeps asset-derived names, messages, and paths
on visibly ordered lines: terminal controls, Unicode line separators, and
bidirectional formatting characters are rendered as visible escapes. Markdown
also flattens line separators and neutralizes table/code-span delimiters before
the text is pasted into a trusted review comment. JSON retains the original
strings as machine data.

`measure --format json` also inventories glTF/GLB material definitions,
texture-to-image links, and image metadata. Its exact core source-slot domain
is `base_color`, `normal`, `metallic_roughness`, `occlusion`, then `emissive`.
This lets a pipeline distinguish an emissive-only material from an untextured
one and identify shared source resources without parsing the asset again. The
nested contract reports `material_resource_coverage: "complete"` when that
documented five-slot glTF/GLB domain was inspected and `"unavailable"` when a
loader has no equivalent source-resource view. Complete does not cover
extension-defined texture slots or promise writer/conversion preservation.
This is descriptive evidence only: it does not accept, repair, resize,
transcode, or otherwise fix an image. See
[machine-readable output](output.md#measure-and-lint) for the full boundary.

## CI Comments (`lint --format markdown`)

`lint --format markdown` renders findings as GitHub/GitLab-flavored
Markdown for pasting into a CI comment or asset-review thread. It mirrors
the text output's information — severity, check id, location, measured
and expected values, per-clip grouping, and typed coverage gaps — as tables inside a per-file
collapsible section, with a clean summary for a passing asset:

```console
animsmith lint clip.glb --format markdown >> "$GITHUB_STEP_SUMMARY"
```

A file with findings is rendered as a `<details>` section (expanded for
short lists, collapsed once a file carries more than ten findings so one
noisy asset does not bury the rest of the comment). A file with neither
findings nor gaps collapses to a one-line `✅ Clean` summary. A footer tallies
errors, warnings, notes, and gaps across every input. The exit code is
unchanged from text and JSON — see [Exit Codes](#exit-codes). Repeated gaps
with the same check id and code share one presentation row with a count and
bounded subject list; JSON retains every original scope.

Markdown is presentation-only and carries **no stability guarantees** —
gate automation on `--format json` (see [output.md](output.md)), and
treat the Markdown as display text that may be re-laid-out between
releases.
