# animsmith

[![CI](https://github.com/mmannerm/animsmith/actions/workflows/ci.yml/badge.svg)](https://github.com/mmannerm/animsmith/actions/workflows/ci.yml)
[![Coverage](https://codecov.io/gh/mmannerm/animsmith/branch/main/graph/badge.svg)](https://codecov.io/gh/mmannerm/animsmith)
[![OpenSSF Scorecard](https://api.securityscorecards.dev/projects/github.com/mmannerm/animsmith/badge)](https://scorecard.dev/viewer/?uri=github.com/mmannerm/animsmith)

A linter and workbench for skeletal animation clips. animsmith turns a
question game teams answer by hand — is this exported animation ready for
our game? — into measured, repeatable evidence: it checks what the file and
your declared contract can establish, and reports what it could not evaluate.

glTF-Validator checks spec conformance. animsmith checks content semantics:
loop seams, gait phase, root-motion speed, track hygiene, and the other
properties that break clips only after engine import. It reads glTF/GLB and
FBX, and can also inspect rigs, measure clips, generate an HTML report or a
versioned JSON inventory, lint declared multi-file collections, convert DCC
exports, compare re-exports, and byte-surgically repair the defects that are
safe to repair.

Something already looks wrong in the engine? Start from the
[symptom pages](https://mmannerm.github.io/animsmith/docs/symptoms/): what
you see, what animsmith measured, and who fixes it. Evaluating animsmith for
your team? Start with
[why animsmith](https://github.com/mmannerm/animsmith/blob/main/docs/why-animsmith.md).

> **Pre-1.0:** Breaking changes are expected between minor releases. Pin
> dependency and CLI versions, and review the release notes before upgrading.

The CLI and crates are tested on Linux, macOS, and Windows. The Rust API is
still experimental, while the most stable automation contracts are check ids,
exit codes, and the versioned JSON envelope.

## Install

Download prebuilt CLI archives from
[GitHub Releases](https://github.com/mmannerm/animsmith/releases/latest). The
supported platform archive names are listed in the
[CLI guide](https://github.com/mmannerm/animsmith/blob/main/docs/cli.md#install).
Each archive includes the `animsmith` binary, README, license files, and
third-party notices, with a matching `.sha256` file so CI or installer scripts
can verify the download.

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
animsmith-core = "0.12"
animsmith-gltf = "0.12"
# Optional:
animsmith-fbx = "0.12"
animsmith-engine = "0.12"
animsmith-report = "0.12"
```

Published API documentation lives on docs.rs:
[animsmith-core](https://docs.rs/animsmith-core),
[animsmith-gltf](https://docs.rs/animsmith-gltf),
[animsmith-fbx](https://docs.rs/animsmith-fbx),
[animsmith-engine](https://docs.rs/animsmith-engine), and
[animsmith-report](https://docs.rs/animsmith-report); the bin-only
[`animsmith` package page](https://docs.rs/animsmith) carries the CLI's
release, source, and feature metadata. Use the binary for a
local tool, CI gate, or artist-facing report; use the library crates when you
already have a Rust pipeline and want the same checks inside your own gate —
the [embedding guide](https://mmannerm.github.io/animsmith/docs/embedding.html)
shows how.

## Quickstart

```console
$ animsmith lint clip.glb
clip.glb:
  warning[quat-flip] clip 'walk' bone 'hips' @0.533s: 1 hemisphere flip(s) ...
  note[constant-track] clip 'walk' bone 'ik_target': scale track has 90 keys but never moves
0 error(s), 1 warning(s), 1 note(s), 0 coverage gap(s)
```

Exit codes are `0` for a run with no failing findings and no
`required_prediction_unavailable` engine facets (warnings, notes, and ordinary
coverage gaps may remain), `1` for failing findings or required-unavailable
prediction work, and `2` for operator errors. `--deny-warnings` promotes
warnings to a failing run; severity and `--allow` never suppress an emitted
required-unavailable facet.

The remaining subcommands follow the same shape and are listed with every
flag, format, and exit code in the
[CLI reference](https://mmannerm.github.io/animsmith/docs/cli.html), and
runnable end-to-end recipes are in the
[examples cookbook](https://mmannerm.github.io/animsmith/examples/).

The HTML report is a single self-contained file with no CDN dependency. It
plays back the exact pose-grid frames judged by the checks, with foot/root
trails, metric charts, and a clickable findings list. It follows the reader's
light or dark system theme, and a URL fragment can pin the theme, embed it in
a page, or deep-link a clip, frame, or finding. `--evidence-only` leaves the
sampled poses out so the report can be shared where the source motion cannot.

## Checks

Mechanical checks need no project config and run on every file:

| id | severity | what |
|---|---|---|
| `nan` | error | NaN or Inf in keys |
| `time-monotonic` | error | key times must move forward |
| `quat-norm` | error | rotation keys must be unit |
| `quat-flip` | warning | adjacent keys on opposite hemispheres |
| `duration-sanity` | error/warning | degenerate duration or mismatched channels |
| `scale-keys` | warning | scale that changes over time |
| `non-uniform-scale` | warning | axes differ somewhere on trajectory |
| `constant-nonunit-scale` | off (opt-in) | constant scale away from one |
| `constant-track` | note | multi-key track that never moves |

Contract-aware checks use declared expectations and, where needed, rig roles:

| id | severity | what |
|---|---|---|
| `fps` | warning | keys must sit on grid |
| `duplicate-loop-endpoint` | warning | loop repeats its first pose |
| `loop-closure` | error | loop must close in pose |
| `loop-seam` | error | feet-relative wrap discontinuity in loops |
| `loop-seam-vel` | error | linear velocity jumps at wrap |
| `loop-seam-rot` | error | angular velocity jumps at wrap |
| `in-place` | error | declared XZ owner versus travel |
| `gait-group` | error | stride phase spread across ring |
| `sync-group` | error | same-time members share timing surfaces |
| `time-complement` | warning | pair aligns better under reflection |
| `root-motion-speed` | error | measured travel versus declared speed |
| `foot-slide` | warning | stance foot skates against speed |
| `missing-bones` | error | declared animated bone is missing |
| `required-bones` | error | declared rig bone must exist |
| `rest-world-scale` | warning | selected node's inherited rest scale |
| `frozen-bone` | error | required bone never actually rotates |
| `bind-pose` | warning | first frame far from rest |

A check that cannot resolve the roles or evidence it needs reports a typed,
nonblocking coverage gap instead of guessing. The current per-check reference —
defaults, prerequisites, config keys, gap semantics, and remediation
boundaries — is the
[built-in check reference](https://mmannerm.github.io/animsmith/docs/built-in-checks.html).

## Configuration

`animsmith.toml` is auto-loaded from the working directory, or passed with
`--config`. A minimal contract declares the rig and what each clip promises:

```toml
[rig]
profile = "auto"            # or mixamo / ue-mannequin / humanoid, or inline [rig.roles]

[clips.walk]
loop = true
movement_owner_xz = "gameplay"

[checks.loop-closure]
max_position_delta_m = 0.01
```

Every table, key, default, precedence rule, validation domain, glob, rig
profile, and engine setting is in the
[configuration reference](https://mmannerm.github.io/animsmith/docs/configuration-reference.html),
and
[`examples/character.animsmith.toml`](https://github.com/mmannerm/animsmith/blob/main/examples/character.animsmith.toml)
is a worked whole-character contract.

An optional exact `[engine]` profile adds bounded importer predictions without
changing what `animsmith measure` reports. It never claims that an engine
loaded, spawned, retargeted, or played anything: the tuple, the settings it
models, and its coverage boundary belong to the profile page for
[Unity](https://mmannerm.github.io/animsmith/docs/engine-profile-unity.html),
[Unreal](https://mmannerm.github.io/animsmith/docs/engine-profile-unreal.html),
[Godot](https://mmannerm.github.io/animsmith/docs/engine-profile-godot.html),
[Bevy](https://mmannerm.github.io/animsmith/docs/engine-profile-bevy.html), or
a [custom glTF runtime](https://mmannerm.github.io/animsmith/docs/engine-profile-gltf-runtime.html).
The separately versioned producer documents — `urn:animsmith:schema:gltf-addressability:2`
and `urn:animsmith:schema:engine-import-advice:2` — are specified in
[machine-readable output](https://mmannerm.github.io/animsmith/docs/output.html).

## Documentation

The [documentation site](https://mmannerm.github.io/animsmith/) lists every
guide and reference by task. Its root is the latest released documentation;
[development documentation](https://mmannerm.github.io/animsmith/dev/) tracks
the default development branch.

- [Symptoms](https://mmannerm.github.io/animsmith/docs/symptoms/) — start from
  what you see in the engine.
- [What game-ready means](https://mmannerm.github.io/animsmith/docs/game-ready-clips.html)
  — the readiness ladder, and who owns each level.
- [For artists](https://mmannerm.github.io/animsmith/docs/animation-author-workflow.html)
  and [for game developers](https://mmannerm.github.io/animsmith/docs/game-developer-intake-workflow.html)
  — the two end-to-end workflows.
- [CLI reference](https://mmannerm.github.io/animsmith/docs/cli.html) and
  [Rust embedding guide](https://mmannerm.github.io/animsmith/docs/embedding.html)
  — commands, flags, exit codes, and library-crate boundaries.

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
