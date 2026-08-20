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
also inspect rigs, measure clips, generate an offline HTML report,
convert DCC exports, compare re-exports, and byte-surgically fix safe
mechanical problems.

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
animsmith-core = "0.3"
animsmith-gltf = "0.3"
# Optional:
animsmith-fbx = "0.3"
animsmith-report = "0.3"
```

Published library API documentation uses these stable docs.rs URLs:

- [animsmith-core](https://docs.rs/animsmith-core)
- [animsmith-gltf](https://docs.rs/animsmith-gltf)
- [animsmith-fbx](https://docs.rs/animsmith-fbx)
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
$ animsmith measure clip.glb
$ animsmith inspect clip.glb
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

Exit codes are `0` for runs with no failing findings (warnings, notes, and
coverage gaps may remain), `1` for error findings, and `2` for operator
errors. `--deny-warnings` promotes warnings to a failing run.

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
`animsmith-report` when you want to generate the standalone HTML report.
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
| `in-place` | error | declared in-place vs root-motion mode must match measured travel |
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

[checks.rest-world-scale]
# Each exact name or `*` glob must resolve to exactly one source node.
node_selectors = ["weapon_socket", "ik_*_target"]
expected_uniform_scale = 1.0
uniform_scale_tolerance = 0.0001

[clips."run_*"]
loop = true
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

Duration-pin values must be finite and positive; their tolerances must be
finite and non-negative. Invalid pins are explicit `duration-sanity` errors,
not silently ignored contracts.

`rest-world-scale` is quiet until `node_selectors` is nonempty. Each selector
uses a deterministic `*` glob and must resolve exactly once; a miss or multiple
matches is a typed coverage gap rather than a guessed node. The check compares
the selected node's inherited rest-world uniform factor with
`expected_uniform_scale` (default `1.0`) using the inclusive
`uniform_scale_tolerance` (default `0.0001`). Non-uniform, sheared, reflected,
and singular transforms are distinct findings. This policy never infers units
from mesh bounds or asset height.

The source hierarchy is loader-projected evidence: exact authored node members
for glTF, but documented metre/Y-up, adjusted and inheritance-compensated ufbx
state for FBX. An FBX result must not be read as the raw FBX transform stack.

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

The [documentation index](https://github.com/mmannerm/animsmith/tree/main/docs)
lists every guide and reference by task. Four useful next stops:

- [Game-ready clips guide](https://github.com/mmannerm/animsmith/blob/main/docs/game-ready-clips.md)
  — why each check exists, failure mode by failure mode.
- [Static asset workflow guide](https://github.com/mmannerm/animsmith/blob/main/docs/static-asset-workflows.md)
  — bounds and transform domains, normal maps, static baking, texture recipes,
  and what still needs target-engine validation.
- [Scaling glTF safely](https://github.com/mmannerm/animsmith/blob/main/docs/scale.md)
  — choose the right scale operation and follow its exact-source rewrite,
  reload, proof, and paired evidence workflow.
- [Examples cookbook](https://github.com/mmannerm/animsmith/tree/main/examples)
  — runnable, copy-into-your-project workflows.

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
