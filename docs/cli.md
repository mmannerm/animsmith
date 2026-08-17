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
animsmith report <file> -o <report.html> [--clip name]
animsmith transform <file> -o <out.glb> [--clip name] [--slice START:END] [--hold-extend SECONDS] [--gait-anchor] [--drop-duplicate-loop-endpoint] [--prune-constant-tracks] [--fps N]
animsmith fix <file> (-o <out.glb>|--in-place|--dry-run) [--repair id[,id]]
animsmith convert <in.fbx|in.glb|in.gltf> -o <out.glb|out.gltf> [--material-texture-recipe recipe.toml] [--animation-only|--bake-static-mesh-transforms] [--format text|json]
animsmith assemble <recipe.toml> -o <out.glb> --evidence <out.json> [--format text|json]
animsmith scale whole-document <in.glb|in.gltf> -o <out.glb|out.gltf> --factor N --evidence <out.json> [--format text|json]
animsmith scale rest-bind <in.glb|in.gltf> -o <out.glb|out.gltf> --source-skin-index N --source-root-node-index N --expected-factor N --evidence <out.json> [--format text|json]
animsmith diff <before> <after> [--format text|json]
```

`inspect` is the human-readable discovery view for exact asset-authored names.
It inventories clips, bones, materials, and mesh-instance nodes, including each
instance's mesh, skin status, and primitive/material context. Use those names
when authoring `assemble` or material texture recipes.

`--config animsmith.toml` is global. Without it, the CLI auto-loads
`./animsmith.toml` when present and otherwise uses built-in defaults.

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
first/final headings plus counted full-turn crossings, avoiding error growth
with the admitted segment count. Four f32 successors at each inclusive cap
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
[scale workflow](scale.md) owns the operation-choice, exact-source rewrite,
proof, publication, and support-boundary walkthrough; this page remains the
installed command/flag/exit reference.

Every numeric and source-identity argument is required. Nothing is inferred
from mesh bounds, character height, joint lengths, inverse-bind magnitude,
filename, or asset category; there is no implicit first skin or root, no
`animsmith.toml` key, no plan file, no in-place mode, and no per-run tolerance
flag — the tolerance policy is fixed and its identity is recorded in the
evidence. Input, output, and evidence paths must all be distinct, and the
output must keep the input's container extension because the rewrite operates
on the source's own bytes.

Accepted inputs are self-contained glTF/GLB. See the
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
| 0 | No failing findings: clean, warnings-only, notes-only, or coverage gaps only. |
| 1 | At least one failing finding, a significant `diff`, pending repairs under `fix --dry-run`, or a `scale`, `convert`, or `assemble` refusal that is a property of source asset bytes. |
| 2 | Operator/tool error: unopenable input, bad config, unsupported format, or invalid flags. |

The code reports what the run *did*, never how well it could report it. This
holds for **every stdout presentation**: parser-rendered help/version, text,
Markdown, and every `--format json` path (`measure`, `lint`, `diff`, `convert`,
`assemble`, `scale`). If
stdout cannot accept the result — a closed pipe or full filesystem — the
checked write never panics, a best-effort checked diagnostic goes to stderr,
and the stdout-bearing path's already-established code stands. Thus
`lint … --format text | head` still
exits `1` for findings it found, `inspect … | head` still exits `0` for an
inspection it completed, and `scale` still exits `1` for a refusal or `0` for
a published pair. Stderr may itself be closed; losing both streams is still
not a panic. Raising the stdout failure instead would report an operator error
for work that was actually done and make exit semantics depend on presentation
format. JSON serialization failure remains exit `2` because the CLI could not
form a truthful record; delivery failure after rendering is only reporting.
Other operator errors occur before stdout reporting, remain stderr-only, and
retain exit `2`.

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
full outcome vocabulary.

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
The current recipe/evidence pair is v4. Recipe v4 may include an optional
`[rest_bind_scale]` block whose source skin index, source root node index, and
expected factor are all required. The block is glTF/GLB-only and validates the
base plus every clip's complete capability inventory and versioned skeleton
basis before remapping any keys. It cannot be combined with
`canonicalize_skin`, `ground_and_center`, or `remove_nodes`, because those
operations change the proved basis. Set `prune_constant_tracks = true` to
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
closure after animation transforms; any surviving track, mesh-instance, or
skin reference refuses the operation.
It performs no material, texture, or mesh garbage collection. Recipe/evidence
v1 through v3 remain immutable historical contracts; v3 rejects
`rest_bind_scale` as unknown.
The recipe identity is
`urn:animsmith:schema:character-assembly-recipe:4`; see
[`character-assembly-recipe-v4.schema.json`](schemas/character-assembly-recipe-v4.schema.json).

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

## Machine Output

`measure`, `lint`, and `diff` support `--format json`. The native JSON
contract is the source of truth and is versioned with `schema_version`.
See [output.md](output.md) and
[`output-v7.schema.json`](schemas/output-v7.schema.json). Nested measurement
evidence has its own
[`measurements-v13.schema.json`](schemas/measurements-v13.schema.json) contract.
Output-v6 and earlier reports, including reports carrying measurements v12,
are historical contracts; regenerate a current output-v7 report from the
original asset with the current CLI before using `diff`.

`rest-world-scale` is quiet until its config supplies `node_selectors`.
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

`assemble` writes evidence v4 to its required `--evidence` path, with immutable
identity `urn:animsmith:schema:character-assembly-evidence:4`; see
[`character-assembly-evidence-v4.schema.json`](schemas/character-assembly-evidence-v4.schema.json).
`assemble --format json` prints the same record to stdout — the identical bytes
the evidence file receives, serialized once — in place of the default `text`
publication summary. An asset refusal publishes neither member and exits `1`;
JSON emits `producer-refusal:1` on stdout, while text emits its stable kind on
stderr. Operator errors remain exit `2`, stderr-only. A failed stdout delivery
does not change an already-established success or refusal code.

`scale` writes scale evidence v4 to its required `--evidence` path, with
immutable identity `urn:animsmith:schema:scale-evidence:4`; see
[output.md](output.md) and
[`scale-evidence-v4.schema.json`](schemas/scale-evidence-v4.schema.json). The
same record is what `scale --format json` prints to stdout, for a refusal as
well as for a published pair. For an `artifact-proof-failed` refusal from the
exact-preservation walk, `rejection.artifact_proof_differences` names up to 16
raw JSON locations the walk found different; `omitted` counts locations beyond
that fixed cap, and the full count is `items.length + omitted`. The field is
`null` for other artifact-proof claims and for capability refusals, whose
`violations` array retains its existing meaning.

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
not cover external buffer or image files, so retain dependency provenance
separately when that scope matters.

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
