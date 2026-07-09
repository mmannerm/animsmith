# animsmith

[![CI](https://github.com/mmannerm/animsmith/actions/workflows/ci.yml/badge.svg)](https://github.com/mmannerm/animsmith/actions/workflows/ci.yml)
[![Coverage](https://codecov.io/gh/mmannerm/animsmith/branch/main/graph/badge.svg)](https://codecov.io/gh/mmannerm/animsmith)
[![OpenSSF Scorecard](https://api.securityscorecards.dev/projects/github.com/mmannerm/animsmith/badge)](https://scorecard.dev/viewer/?uri=github.com/mmannerm/animsmith)

A linter and workbench for skeletal animation clips. animsmith answers a
question game teams usually answer by hand: does this exported animation
behave like something a game runtime can actually use?

animsmith checks glTF/GLB and FBX clips for broken quaternions,
degenerate durations, popped loop seams, gait-phase drift, root-motion
contract drift, export bloat, and other game-semantics problems. It can
also inspect rigs, measure clips, generate an offline HTML report,
convert DCC exports, compare re-exports, and byte-surgically fix safe
mechanical problems.

glTF-Validator checks spec conformance. animsmith checks content
semantics: loop seams, gait phase, root-motion speed, track hygiene, and
other properties that decide whether an animation is usable in a game
runtime. For the full story of what makes a clip game-ready — each
runtime failure mode, and which check, repair, and config covers it —
see the
[game-ready clips guide](https://github.com/mmannerm/animsmith/blob/main/docs/game-ready-clips.md).

**Status: pre-1.0, publishing candidate.** The CLI and crates are tested
on Linux, macOS, and Windows. The Rust API is still experimental, while
the most stable automation contracts are check ids, exit codes, and the
versioned JSON envelope.

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
animsmith-core = "0.1"
animsmith-gltf = "0.1"
# Optional, only when you ingest FBX:
animsmith-fbx = "0.1"
```

API documentation is published on docs.rs when the crates are published
to crates.io:

- [animsmith-core](https://docs.rs/animsmith-core)
- [animsmith-gltf](https://docs.rs/animsmith-gltf)
- [animsmith-fbx](https://docs.rs/animsmith-fbx)
- [animsmith-report](https://docs.rs/animsmith-report)
- [animsmith](https://docs.rs/animsmith)

## Quickstart

```console
$ animsmith lint clip.glb
clip.glb:
  warning[quat-flip] clip 'walk' bone 'hips' @0.533s: 1 hemisphere flip(s) ...
  note[constant-track] clip 'walk' bone 'ik_target': scale track has 90 keys but never moves
0 error(s), 1 warning(s), 1 note(s)

$ animsmith lint export.fbx
$ animsmith measure clip.glb
$ animsmith inspect clip.glb
$ animsmith report clip.glb -o report.html
$ animsmith convert export.fbx -o clip.glb
$ animsmith diff old.glb new.glb
$ animsmith fix clip.glb -o fixed.glb
$ animsmith fix clip.glb --dry-run
```

Exit codes are `0` for clean or warnings-only runs, `1` for error
findings, and `2` for operator errors. `--deny-warnings` promotes
warnings to a failing run.

The HTML report is a single self-contained file with no CDN dependency.
It plays back the exact pose-grid frames judged by the checks, with
foot/root trails, metric charts, and a clickable findings list.

## CLI Or Library?

Use the `animsmith` binary when you want a local tool, CI gate, or
artist-facing report. Use `animsmith-core` when you already have a Rust
pipeline and want to run the same measurements and checks inside your
own gate.

Pair `animsmith-core` with exactly the loader crates you need:
`animsmith-gltf` for glTF/GLB, `animsmith-fbx` for FBX, and
`animsmith-report` when you want to generate the standalone HTML report.
The CLI crate is not the embedding API; it is one frontend over the same
core.

## Checks

Mechanical checks run without project config:

| id | severity | what |
|---|---|---|
| `nan` | error | NaN/Inf in key times or values |
| `time-monotonic` | error | non-increasing or negative key times; late first key notes |
| `quat-norm` | error | non-unit rotation keys |
| `quat-flip` | warning | adjacent rotation keys on opposite hemispheres |
| `duration-sanity` | error/warning | degenerate duration, empty clips, or mismatched channel ends |
| `scale-keys` | warning | animated scale or non-uniform scale |
| `constant-track` | note | multi-key tracks that never move |

Contract-aware checks use declared expectations and, where needed, rig roles:

| id | severity | what |
|---|---|---|
| `fps` | warning | duration and keys must land on the declared frame grid |
| `loop-seam` | error | feet-relative-to-hips wrap discontinuity in declared loops |
| `in-place` | error | declared in-place vs root-motion mode must match measured travel |
| `gait-group` | error | stride-phase spread across a declared directional blend ring |
| `root-motion-speed` | error | measured horizontal root travel vs a declared speed pin |
| `foot-slide` | warning | stance feet must move consistently with declared travel |
| `missing-bones` | error | declared animated bones missing from the skeleton or carrying no keys |
| `frozen-bone` | error | required bones whose rotation never exceeds the configured floor |
| `bind-pose` | warning | first frame deviating too far from the skeleton rest pose |

Checks whose rig roles cannot be resolved are skipped with a note rather
than guessed.

## Configuration

`animsmith.toml` is auto-loaded from the working directory, or passed
with `--config`:

```toml
[rig]
profile = "auto"            # or mixamo / ue-mannequin / humanoid, or inline [rig.roles]

[checks.loop-seam]
max_ratio = 1.6

[clips."run_*"]
loop = true

[clips.run_forward]
speed_mps = { value = 3.1, tolerance = 0.25 }

[gait_groups.run-ring]
clips = ["run_forward", "run_backward", "run_left", "run_right"]
max_gait_phase_spread = 0.15
min_lr_amplitude_m = 0.03
```

`--select`, `--allow`, and `[checks.*] severity` including `"off"`
control what runs and how hard it fails. See the
[worked config](https://github.com/mmannerm/animsmith/blob/main/examples/character.animsmith.toml)
for a contract-style example.

## More Documentation

- [Game-ready clips guide](https://github.com/mmannerm/animsmith/blob/main/docs/game-ready-clips.md)
- [Examples cookbook](https://github.com/mmannerm/animsmith/tree/main/examples)
- [Documentation index](https://github.com/mmannerm/animsmith/tree/main/docs)
- [CLI reference](https://github.com/mmannerm/animsmith/blob/main/docs/cli.md)
- [Embedding API](https://github.com/mmannerm/animsmith/blob/main/docs/embedding.md)
- [Machine-readable output](https://github.com/mmannerm/animsmith/blob/main/docs/output.md)
- [Architecture and roadmap](https://github.com/mmannerm/animsmith/blob/main/DESIGN.md)
- [Contributor guide](https://github.com/mmannerm/animsmith/blob/main/CONTRIBUTING.md)
- [Development setup](https://github.com/mmannerm/animsmith/blob/main/DEVELOPMENT.md)
- [Release process](https://github.com/mmannerm/animsmith/blob/main/RELEASING.md)
- [Support](https://github.com/mmannerm/animsmith/blob/main/SUPPORT.md)
- [Security policy](https://github.com/mmannerm/animsmith/blob/main/SECURITY.md)

## License

MIT OR Apache-2.0. See
[THIRD-PARTY.md](https://github.com/mmannerm/animsmith/blob/main/THIRD-PARTY.md)
for dependency notices.
