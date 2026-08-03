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
animsmith transform <file> -o <out.glb> [--clip name] [--slice START:END] [--hold-extend SECONDS] [--gait-anchor] [--fps N]
animsmith fix <file> (-o <out.glb>|--in-place|--dry-run) [--repair id[,id]]
animsmith convert <in.fbx|in.glb|in.gltf> -o <out.glb|out.gltf> [--material-texture-recipe recipe.toml] [--animation-only|--bake-static-mesh-transforms] [--format text|json]
animsmith assemble <recipe.toml> -o <out.glb> --evidence <out.json>
animsmith diff <before> <after> [--format text|json]
```

`inspect` is the human-readable discovery view for exact asset-authored names.
It inventories clips, bones, materials, and mesh-instance nodes, including each
instance's mesh, skin status, and primitive/material context. Use those names
when authoring `assemble` or material texture recipes.

`--config animsmith.toml` is global. Without it, the CLI auto-loads
`./animsmith.toml` when present and otherwise uses built-in defaults.

## Exit Codes

| Code | Meaning |
|---:|---|
| 0 | No failing findings: clean, warnings-only, notes-only, or coverage gaps only. |
| 1 | At least one failing finding, a significant `diff`, or pending repairs under `fix --dry-run`. |
| 2 | Operator/tool error: unreadable input, bad config, unsupported format, or invalid flags. |

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

## Feature Flags

The default binary enables `fbx` and `report`.

```console
cargo install animsmith
cargo install animsmith --no-default-features
```

The no-default-features build has no C toolchain dependency and keeps the
glTF-only workflow: `inspect`, `measure`, `lint`, `transform`, `fix`, and
`diff`. The HTML `report` command is controlled by the `report` feature.
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
[`output-v2.schema.json`](schemas/output-v2.schema.json). Nested measurement
evidence has its own
[`measurements-v4.schema.json`](schemas/measurements-v4.schema.json) contract.
Alpha-era v1 and preview reports are not retained; regenerate them before
using `diff`.

`convert --format json` emits conversion evidence v2, with immutable identity
`urn:animsmith:schema:conversion-evidence:2`; see [output.md](output.md) and
[`conversion-evidence-v2.schema.json`](schemas/conversion-evidence-v2.schema.json).
It records the requested options, counts from the written artifact, exact
static-mesh transforms when requested, and recipe provenance when a material
texture recipe is used. `text` is the default human-readable write summary.

`assemble` writes evidence v1 to its required `--evidence` path, with immutable
identity `urn:animsmith:schema:character-assembly-evidence:1`; see
[`character-assembly-evidence-v1.schema.json`](schemas/character-assembly-evidence-v1.schema.json).

Machine-readable lint rejects `--allow` so it cannot erase evidence. The flag
remains a presentation and exit-policy convenience for text and Markdown.

Native JSON is deliberately shaped so serializers can be added later
without redesigning the checks: SARIF for code scanning, GitLab Code
Quality/CodeClimate for MR widgets, JUnit XML for CI dashboards, and CSV
for ad-hoc analysis.

Human-readable command output keeps asset-derived names, messages, and paths
on visibly ordered lines: terminal controls, Unicode line separators, and
bidirectional formatting characters are rendered as visible escapes. Markdown
also flattens line separators and neutralizes table/code-span delimiters before
the text is pasted into a trusted review comment. JSON retains the original
strings as machine data.

`measure --format json` also inventories glTF/GLB material definitions,
texture-to-image links, and image metadata. This lets a pipeline distinguish a
normal slot from a BaseColor slot and identify shared source resources without
parsing the asset a second time. The nested measurement contract reports
`material_resource_coverage: "complete"` for glTF/GLB and `"unavailable"`
when a loader has no equivalent source-resource view. This is descriptive
evidence only: it does not accept, repair, resize, transcode, or otherwise fix
an image, and it does not promise that subsequent conversion or writing keeps
every source payload. See [machine-readable output](output.md#measure-and-lint)
for MIME, detected-container, decoded-color, and unavailable-image semantics.

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
