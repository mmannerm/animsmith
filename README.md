# animsmith

[![CI](https://github.com/mmannerm/animsmith/actions/workflows/ci.yml/badge.svg)](https://github.com/mmannerm/animsmith/actions/workflows/ci.yml)
[![Coverage](https://codecov.io/gh/mmannerm/animsmith/branch/main/graph/badge.svg)](https://codecov.io/gh/mmannerm/animsmith)
[![OpenSSF Scorecard](https://api.securityscorecards.dev/projects/github.com/mmannerm/animsmith/badge)](https://scorecard.dev/viewer/?uri=github.com/mmannerm/animsmith)

A linter and workbench for skeletal animation clips. animsmith turns a
question game teams answer by hand — is this exported animation ready
for our game? — into measured, repeatable evidence: it checks what the
file and your declared contract can establish, and reports what it
could not evaluate.

animsmith checks glTF/GLB and FBX clips for broken quaternions,
degenerate durations, popped loop seams, gait-phase drift, root-motion
contract drift, export bloat, and other game-semantics problems. It can
also inspect rigs, measure clips, generate an offline HTML report or a
versioned glTF animation-addressability inventory or engine import advice,
lint declared multi-file clip collections, convert DCC exports,
compare re-exports, and byte-surgically fix safe mechanical problems.

glTF-Validator checks spec conformance. animsmith checks content
semantics: loop seams, gait phase, root-motion speed, track hygiene, and
other properties that commonly break clips only after engine import.
What "game-ready" means here is staged evidence, not certification of
an unspecified runtime: the
[game-ready clips guide](https://github.com/mmannerm/animsmith/blob/main/docs/game-ready-clips.md#the-readiness-ladder)
defines the readiness ladder — what animsmith validates, what needs
your declarations, and what stays with your engine and team — and
walks each runtime failure mode with the check, repair, and config
that covers it.
Evaluating animsmith for your team? Start with
[why animsmith](https://github.com/mmannerm/animsmith/blob/main/docs/why-animsmith.md)
— what it is, why it exists, and what it is worth by role.

> **Pre-1.0:** Breaking changes are expected between minor releases. Pin
> dependency and CLI versions, and review the release notes before upgrading.

The CLI and crates are tested on Linux, macOS, and Windows. The Rust API is
still experimental, while the most stable automation contracts are check ids,
exit codes, and the versioned JSON envelope.

## Install

Download prebuilt CLI archives from
[GitHub Releases](https://github.com/mmannerm/animsmith/releases/latest):

The supported platform archive names are listed in the
[CLI guide](https://github.com/mmannerm/animsmith/blob/main/docs/cli.md#install).

Each archive includes the `animsmith` binary, README, license files, and
third-party notices. Matching `.sha256` files are published alongside the
archives so CI or installer scripts can verify downloads.

Or install from crates.io with Cargo:

```console
$ cargo install animsmith
```

The default install includes FBX input and HTML reports. For a pure-Rust
glTF-only binary with no C build step:

```console
$ cargo install animsmith --no-default-features
```

For Rust pipelines, depend on the crates you need:

```toml
[dependencies]
animsmith-core = "0.8"
animsmith-gltf = "0.8"
# Optional:
animsmith-fbx = "0.8"
animsmith-engine = "0.8"
animsmith-report = "0.8"
```

Published library API documentation uses these stable docs.rs URLs:

- [animsmith-core](https://docs.rs/animsmith-core)
- [animsmith-gltf](https://docs.rs/animsmith-gltf)
- [animsmith-fbx](https://docs.rs/animsmith-fbx)
- [animsmith-engine](https://docs.rs/animsmith-engine)
- [animsmith-report](https://docs.rs/animsmith-report)

The bin-only [`animsmith` package page](https://docs.rs/animsmith) exposes
release, source, and feature metadata. CLI usage is documented in the
[CLI guide](https://github.com/mmannerm/animsmith/blob/main/docs/cli.md).

## Quickstart

```console
$ animsmith lint clip.glb
clip.glb:
  warning[quat-flip] clip 'walk' bone 'hips' @0.533s: 1 hemisphere flip(s) ...
  note[constant-track] clip 'walk' bone 'ik_target': scale track has 90 keys but never moves
0 error(s), 1 warning(s), 1 note(s), 0 coverage gap(s)

$ animsmith lint export.fbx
$ animsmith collection lint collection.toml --format json
$ animsmith collection evaluate-directional-speed --policy directional-speed.toml --evidence collection-output.json --format json
$ animsmith measure clip.glb
$ animsmith inspect clip.glb
$ animsmith generate addressability clip.glb
$ animsmith --config unity.animsmith.toml generate import-advice export.fbx
$ animsmith report clip.glb -o report.html
$ animsmith convert export.fbx -o clip.glb
$ animsmith convert prop.fbx -o prop.glb --bake-static-mesh-transforms
$ animsmith convert prop.fbx -o prop.glb --material-texture-recipe materials.toml
$ animsmith assemble character-assembly.toml -o character.glb --evidence character.assembly.json
$ animsmith scale whole-document centimetres.glb -o metres.glb --factor 0.01 --evidence metres.scale.json
$ animsmith scale rest-bind character.glb -o canonical.glb --source-skin-index 0 --source-root-node-index 3 --expected-factor 0.01 --evidence canonical.scale.json
$ animsmith diff old.glb new.glb
$ animsmith fix clip.glb -o fixed.glb
$ animsmith fix clip.glb --dry-run
$ animsmith transform clip.glb -o compact.glb --prune-constant-tracks
```

Exit codes are `0` for runs with no failing findings and no
`required_prediction_unavailable` engine facets (warnings, notes, and ordinary
coverage gaps may remain), `1` for failing findings or required-unavailable
prediction work, and `2` for operator errors. `--deny-warnings` promotes
warnings to a failing run; severity and `--allow` never suppress an emitted
required-unavailable facet.

The HTML report is a single self-contained file with no CDN dependency.
It plays back the exact pose-grid frames judged by the checks, with
foot/root trails, metric charts, and a clickable findings list.

## CLI Or Library?

Use the `animsmith` binary when you want a local tool, CI gate, or
artist-facing report. Use `animsmith-core` when you already have a Rust
pipeline and want to run the same measurements and checks inside your
own gate — the
[embedding guide](https://github.com/mmannerm/animsmith/blob/main/docs/embedding.md)
shows how.

Pair `animsmith-core` with the format crates you need:
`animsmith-gltf` for glTF/GLB and `animsmith-fbx` for FBX. Add
`animsmith-engine` for the strict built-in consumer-profile registry and
resolver, and `animsmith-report` when you want to generate the standalone HTML report.
The CLI crate is not the library API; it is one frontend over the same core.

## Checks

Mechanical checks:

| id | severity | what |
|---|---|---|
| `nan` | error | NaN/Inf in key times or values |
| `time-monotonic` | error | non-increasing or negative key times; late first key notes |
| `quat-norm` | error | non-unit rotation keys |
| `quat-flip` | warning | adjacent rotation keys on opposite hemispheres |
| `duration-sanity` | error/warning | degenerate or unexpectedly changed duration, empty clips, or mismatched channel ends |
| `scale-keys` | warning | interpolation-aware temporal scale variation |
| `non-uniform-scale` | warning | non-uniform scale anywhere on the evaluated trajectory |
| `constant-nonunit-scale` | off (opt-in) | constant non-unit scale channels, including single-key pins |
| `constant-track` | note | multi-key tracks that never move |

These checks need no rig roles. Default-on entries run without project config;
`duration-sanity` can additionally enforce a declared per-clip duration, and
opt-in policy signals remain visible but disabled.

Contract-aware checks use declared expectations and, where needed, rig roles:

| id | severity | what |
|---|---|---|
| `fps` | warning | duration and keys must land on the declared frame grid |
| `duplicate-loop-endpoint` | warning | a declared loop repeats its first authored pose at a mechanically removable final endpoint |
| `loop-closure` | error | maximum per-bone model-space position and rotation mismatch in declared loops |
| `loop-seam` | error | feet-relative-to-hips wrap discontinuity in declared loops |
| `loop-seam-vel` | error | maximum per-bone model-space linear-velocity change across a declared loop wrap |
| `loop-seam-rot` | error | maximum per-bone model-space angular-velocity change across a declared loop wrap |
| `in-place` | error | declared XZ movement owner must match measured horizontal travel |
| `gait-group` | error | stride-phase spread across a declared directional blend ring |
| `sync-group` | error | same-time blend members must share duration, frame grid, and endpoint convention |
| `time-complement` | warning | same-time blend pairs whose gait phase aligns substantially better under reflected time |
| `root-motion-speed` | error | measured horizontal root travel vs a declared speed pin |
| `foot-slide` | warning | stance feet must move consistently with declared travel |
| `missing-bones` | error | declared animated bones missing from the skeleton or carrying no keys |
| `required-bones` | error | declared rig bones missing from the skeleton, even when no clip is expected to animate them |
| `rest-world-scale` | warning | selected source nodes have an unexpected effective rest-world scale or affine class |
| `frozen-bone` | error | required bones whose rotation never exceeds the configured floor |
| `bind-pose` | warning | first frame deviating too far from the skeleton rest pose |

`duplicate-loop-endpoint`, `loop-closure`, `loop-seam-vel`, and `loop-seam-rot` need no rig
roles or locomotion stride. `duplicate-loop-endpoint` is default-on but applies
only when `[clips.<name>] loop = true`: it warns only for the strict authored-key
case where every track shares one finite, strictly increasing timeline, has
matching endpoint values (vector components within `1e-5`, sign-invariant
quaternion angle within `1e-4` radians), and contains real interior motion. Use
`transform --drop-duplicate-loop-endpoint` to make
that eligible clip an open cycle; it removes the same terminal key count from
every channel and re-pins its duration. It deliberately does not preserve a
green `loop-closure` result, because that inclusive endpoint check expects the
repeated final sample. The complete endpoint-mode classifier remains [#22](https://github.com/mmannerm/animsmith/issues/22).
Checks that do require semantic roles report a
typed, nonblocking coverage gap when those roles cannot be resolved rather
than guessing or manufacturing a content finding.

## Configuration

`animsmith.toml` is auto-loaded from the working directory, or passed
with `--config`:

The complete table/key reference, defaults, precedence, validation domains,
and engine-profile settings live in the [configuration reference](https://github.com/mmannerm/animsmith/blob/main/docs/configuration-reference.md).

```toml
[rig]
profile = "auto"            # or mixamo / ue-mannequin / humanoid, or inline [rig.roles]
# Structural rig contract: sockets, IK targets, and mask bones may be static.
required_bones = ["root", "weapon_socket", "ik_hand_l"]

[checks.loop-seam]
max_ratio = 1.6

[checks.loop-closure]
max_position_delta_m = 0.01
max_rotation_delta_deg = 1.0

[checks.loop-seam-vel]
max_velocity_delta_mps = 0.1

[checks.loop-seam-rot]
max_angular_velocity_delta_degps = 5.0

[runtime_nodes]
# Shared attachment/socket/IK policy. Each exact name or `*` glob must resolve
# to exactly one source node.
selectors = ["weapon_socket", "ik_*_target"]

[checks.rest-world-scale]
expected_uniform_scale = 1.0
uniform_scale_tolerance = 0.0001

[clips."run_*"]
loop = true
# Project intent is independent per world-motion component.
movement_owner_xz = "animation"
movement_owner_y = "gameplay"
movement_owner_yaw = "animation"
# Clip/glob caps override these global defaults only for matching clips.
max_loop_position_delta_m = 0.04
max_loop_rotation_delta_deg = 2.0
max_loop_velocity_delta_mps = 0.2
max_loop_angular_velocity_delta_degps = 200.0

[clips.run_forward]
# Exact entries win over matching globs, field by field.
max_loop_rotation_delta_deg = 0.5
duration_s = { value = 1.033, tolerance = 0.02 }
speed_mps = { value = 3.1, tolerance = 0.25 }

[gait_groups.run-ring]
clips = ["run_forward", "run_backward", "run_left", "run_right"]
max_gait_phase_spread = 0.15
min_lr_amplitude_m = 0.03

[sync_groups.run-ring]
clips = ["run_forward", "run_backward", "run_left", "run_right"]
# Same-time runtime sampling needs compatible duration, declared FPS grid, and
# loop endpoint convention; tune the declared timing tolerance for your engine.
max_duration_delta_s = 0.001
max_frame_count_delta = 0
max_fps_delta = 0.01

[sync_groups.run-ring.time_complement]
# Compare every unordered pair in the group. Similarity is derived from the
# left-minus-right foot-height gait phase; higher scores are closer.
min_reflected_time_advantage = 0.25
min_lr_amplitude_m = 0.03
```

Built-in profiles match exact names first, then only one unique ASCII
case-insensitive candidate; multiple candidates remain a typed ambiguity.
`[rig.roles]` is authoritative and exact: use it for a deliberately chosen
bone name, not as a case-insensitive alias.

Duration-pin values must be finite and positive; their tolerances must be
finite and non-negative. Invalid pins are explicit `duration-sanity` errors,
not silently ignored contracts.

`movement_owner_xz`, `movement_owner_y`, and `movement_owner_yaw` each accept
`"gameplay"` when the entity/controller owns that component or `"animation"`
when extracted root motion owns it. Omitted axes remain unspecified; animsmith
never infers one axis from another, a filename, measured displacement, or an
engine profile. The legacy `in_place` boolean remains an XZ-only input alias
(true is gameplay, false is animation). One selector entry cannot declare
both spellings. Alias and canonical declarations in different glob/exact
layers are normalized first, then use the same field-by-field precedence as
the other clip expectations.

`rest-world-scale` is quiet until `[runtime_nodes].selectors` is nonempty. The
legacy `[checks.rest-world-scale].node_selectors` spelling remains an alias, but
declaring both is a configuration error. Each selector uses a deterministic
`*` glob and must resolve exactly once; a miss or multiple matches is a typed
coverage gap rather than a guessed node. The check compares
the selected node's inherited rest-world uniform factor with
`expected_uniform_scale` (default `1.0`) using the inclusive
`uniform_scale_tolerance` (default `0.0001`). Non-uniform, sheared, reflected,
and singular transforms are distinct findings. This policy never infers units
from mesh bounds or asset height.

The source hierarchy is loader-projected evidence: exact authored node members
for glTF, but documented metre/Y-up, adjusted and inheritance-compensated ufbx
state for FBX. An FBX result must not be read as the raw FBX transform stack.

An optional exact `[engine]` profile adds bounded importer predictions without
changing `measure`. For `bevy` revision 1 / `0.19.0` /
`gltf-asset-loader`, `engine-addressability` reports the canonical
`GltfAssetLabel::Animation(i)` display selector `Animation{i}` for every
completely inventoried source animation. The index is type-safe and
version-pinned but can change when the source animation order changes. This is
selector evidence, not proof that Bevy loaded the asset, retained its targets,
or connected an animation graph. See the runnable
[Bevy example](https://github.com/mmannerm/animsmith/blob/main/examples/README.md#predicting-a-bevy-animation-selector).

For `bevy` revision 2 with the same exact engine/importer tuple,
`engine-unit-scale` emits machine-readable importer predictions for the glTF
metre-to-world-length mapping, loader-created scene entities, raw-inventory
mesh primitive children, and configured runtime-node selectors. The profile
requires an exact supported extension-handler environment and the
`bevy_animation` feature state. It does not claim that arbitrary application
world state is metre-authored, include the caller-owned `WorldAssetRoot`, or
authorize content rewriting. See
[the Bevy profile](https://github.com/mmannerm/animsmith/blob/main/docs/engine-profile-bevy.md).

The current successor is Bevy profile revision 3, still pinned to
0.19.0 and `gltf-asset-loader`. Its narrow `engine-track-support` slice covers
only the feature-gated animation path: `bevy_animation` and `load_animations`
are resolved with their explicit/default origins, and a disabled feature gate
takes precedence over `load_animations`. It inventories raw source animation
and channel rows from the same load, then emits only negative gate outcomes:
when a gate drops content, the affected animation/channel rows are reported as
available negative facets; when both gates allow loading, runtime survival is
required-unavailable because AnimSmith does not run Bevy. Complete-empty input
is not applicable, while partial or unavailable inventory produces exactly one
subjectless unsuppressible inventory facet and no retained-prefix prediction.
This slice does not model extensions, other animation constructs,
positive runtime survival, or a content finding for a dropped row. Its V5
provenance and output-v19 contract are current; output-v17, output-v16, revisions 1 and 2,
and output-v15 remain preserved and readable.

The rich glTF addressability producer is the separate immutable
`urn:animsmith:schema:gltf-addressability:2` contract. It preserves the V1
animation inventory and adds independently covered, bounded same-load scenes,
nodes, skins, attachments, scene paths, default-scene routing, named-map
winners, and unique animation targets. Complete empty coverage proves absence;
partial prefixes and unavailable domains do not. It retains explicit
`skin.skeleton` source evidence but makes no inferred-root claim, and does not
claim scene-instantiated `SkinnedMesh` attachment.

Its optional exact Bevy adapter uses the existing single
`engine-addressability` lifecycle and `Animation{i}` primitive. The separately
versioned rule bundle is pinned to Bevy `v0.19.0`, commit
`c6f634ca9f406d68ba5109d921247b654cb42c10`, `bevy_gltf 0.19.0`, locked
`gltf 1.4.1`, and the loader/label/path/target-id/feature/root-`Cargo.lock`
sources. It models
`Scene{i}` and an optional `Gltf.default_scene` route only; there is no
`DefaultScene` label or fabricated `Scene0`. It eagerly projects
`Skin{i}/InverseBindMatrices` for every source skin (identity fallback when
absent) and projects `Skin{i}` when any source node references it. Named maps
are separate source-order last-write-wins projections.

Exact target UUIDs require an explicitly declared 32- or 64-bit pointer width
because Bevy hashes path-segment lengths using its target pointer width; width
is never inferred from the host. Unreachable, multiply reachable, colliding,
incomplete, or feature-disabled targets are required-unavailable, not guessed. Its explicit
`target_coverage` projection distinguishes complete (including empty) target
domains from incomplete raw/animation evidence and `target_domain_truncated`;
other rich projection budget failures use `projection_bounds_exceeded`. V2 bounds each
domain at 4,096 rows, references at 65,536, path segments at 1,024 bytes and
256 segments, retained text at 1 MiB, and staged reports at 256 MiB. It is
prediction evidence only and never certifies Bevy loading, spawning, target
survival, graph wiring, or playback. See
[`gltf-addressability-v2.schema.json`](https://github.com/mmannerm/animsmith/blob/main/docs/schemas/gltf-addressability-v2.schema.json).

The exact Unity Generic revision-2 tuple uses profile id unity-generic,
revision `2`, engine version `6000.3`, and importer id fbx-model-importer for
FBX. Its required closed settings are
`animation_type = "generic"`, `avatar_setup = "create_from_this_model"`,
`import_animation = true`, and the document-scoped
`root_motion_source`, plus per-clip `root_rotation`, `root_position_y`, and
`root_position_xz` values of `"bake"` or `"extract"`. The preserved Unity
Humanoid revision-1 profile is a separate contract; `root_motion_source` is not
applicable to it.

For this Generic tuple, `engine-root-motion` compares each explicitly declared
`movement_owner_xz`, `movement_owner_y`, and `movement_owner_yaw` with its
corresponding importer setting. It reports typed `RootMotionRouting` results
(gameplay + `baked_into_pose`, or animation + `stored_as_root_motion`) and
ordinary error findings for conflicts. Required-unavailable facets cover
missing/ambiguous paths, a path that does not identify the explicit Root role,
incomplete raw path or intent coverage, settings overflow, and unavailable
axis measurements. Hips is never a fallback for this engine rule. Measurement
availability is required, but displacement/yaw magnitude never controls
applicability or routing; this is not a travel threshold.

The configured FBX path is case-sensitive and byte-exact: `/` is the only
unescaped separator; empty, `.`, `..`, backslash, control, and Unicode-format
segments are rejected, with limits of 1,024 bytes per segment, 4,096 bytes per
path, and 256 segments. The loader records same-byte raw source identities,
parent chains, and names; implicit and generated helper nodes cannot match,
and incomplete coverage cannot prove `NoMatch`. V6 provenance and output-v19
carry this prediction. AnimSmith does not execute Unity, read back imported
assets, play runtime clips, or certify engine behavior. Repository and CI use
only self-authored synthetic fixtures for this slice.

`generate import-advice` is the separate engine-setting projection path. With
the preserved revision-1 Unity 6000.3 Generic or Humanoid profile, it emits
only the documented importer properties already materialized by config and
binds them to same-load source, closure, intent, and measurement evidence.
The current Generic revision-2 profile is the root-motion lint contract;
it does not add import-advice fields. Unreal 5.8 and Godot 4.7
revision 1 refuse with typed `profile_settings_unmodeled` evidence rather than
inventing settings. V1 never guesses frame-number ranges, sample rates, unit
conversion, or root-motion behavior. See the runnable
[import-advice example](https://github.com/mmannerm/animsmith/blob/main/examples/README.md#generating-engine-import-advice).

Revision 2 adds the same command's narrow document-setting projection for the
exact `godot/2/4.7/resource-importer-scene` tuple (glTF JSON or GLB)
and `unreal/2/5.8/fbx-importer` tuple (FBX). Godot projects only
`animation/fps` (1..120, default 30) and `animation/trimming` (default false);
Unreal requires explicit `sample_rate` as `default_30`, `source_determined`,
or `custom_hz(1..48000)`. These are exact parameter projections, not importer
execution, readback, runtime, or game-ready claims. The V2 document is
versioned separately as `urn:animsmith:schema:engine-import-advice:2`.

The four loop-continuity caps may also be declared under a clip name or
`*`-glob: `max_loop_position_delta_m`, `max_loop_rotation_delta_deg`, and
`max_loop_velocity_delta_mps`, and `max_loop_angular_velocity_delta_degps`.
Each is a finite, non-negative cap that overrides only its corresponding global
`[checks.loop-closure]`, `[checks.loop-seam-vel]`, or `[checks.loop-seam-rot]`
default. Matching globs layer in lexical key order;
an exact clip entry wins field by field. This lets an idle family remain strict
without forcing root-motion locomotion to use the same position budget.

`--select`, `--allow`, and `[checks.*] severity` including `"off"`
control what runs and how hard it fails. Assigning "note", "warn", or "error"
also enables an opt-in check such as `constant-nonunit-scale`. See the
[worked config](https://github.com/mmannerm/animsmith/blob/main/examples/character.animsmith.toml)
for a contract-style example.

## More Documentation

The [documentation site](https://mmannerm.github.io/animsmith/)
lists every guide and reference by task. The site root is the latest released
documentation; [development documentation](https://mmannerm.github.io/animsmith/dev/)
tracks the default development branch. Four useful next stops:

- [Game-ready clips guide](https://mmannerm.github.io/animsmith/docs/game-ready-clips.html)
  — why each check exists, failure mode by failure mode.
- [Engine profile guides](https://mmannerm.github.io/animsmith/docs/)
  — exact Unity, Unreal, Godot, Bevy, and generic glTF importer boundaries,
  settings, scale guidance, and downstream responsibilities.
- [Static asset workflow guide](https://mmannerm.github.io/animsmith/docs/static-asset-workflows.html)
  — bounds and transform domains, normal maps, static baking, texture recipes,
  and what still needs target-engine validation.
- [Scaling glTF safely](https://mmannerm.github.io/animsmith/docs/scale.html)
  — choose the right scale operation and follow its exact-source rewrite,
  reload, proof, and paired evidence workflow.
- [Examples cookbook](https://mmannerm.github.io/animsmith/examples/)
  — runnable, copy-into-your-project workflows.
- [CLI reference](https://mmannerm.github.io/animsmith/docs/cli.html)
  — commands, flags, output formats, and exit codes.
- [Rust embedding guide](https://mmannerm.github.io/animsmith/docs/embedding.html)
  — library-crate boundaries and the embedded gate flow.

## Contributing

See the
[contributor guide](https://github.com/mmannerm/animsmith/blob/main/CONTRIBUTING.md)
for the contribution flow and the
[development setup](https://github.com/mmannerm/animsmith/blob/main/DEVELOPMENT.md)
for toolchain, gates, and test commands. Questions and bug reports:
[SUPPORT.md](https://github.com/mmannerm/animsmith/blob/main/SUPPORT.md);
vulnerabilities:
[SECURITY.md](https://github.com/mmannerm/animsmith/blob/main/SECURITY.md).

## License

MIT OR Apache-2.0. See
[THIRD-PARTY.md](https://github.com/mmannerm/animsmith/blob/main/THIRD-PARTY.md)
for dependency notices.
