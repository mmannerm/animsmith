# animsmith

A workbench for skeletal animation clips. It answers the question every
game team answers by hand today — **does this clip have
game-engine-friendly characteristics?** — and then helps you make it
so. Lint broken quaternions, degenerate durations, popped loop seams,
gait-phase drift, and export bloat; convert straight from DCC exports;
and fix safe mechanical problems in place.

glTF-Validator checks spec conformance; animsmith judges — and forges —
game-semantics *content*: loop seams, gait phase, root-motion speed,
track hygiene, and other properties that decide whether an animation is
usable in a game runtime.

**Status: pre-1.0, publishing candidate.** glTF/GLB **and FBX** input (via [ufbx](https://github.com/ufbx/ufbx));
mechanical + locomotion-semantics check sets; rig profiles
(mixamo / ue-mannequin / humanoid + auto-detect); `animsmith.toml`
config with per-clip expectations and gait groups; subcommands
`inspect` / `measure` / `lint` / `report` / `transform` / `fix` /
`convert` / `diff`.
The CLI and crates are tested on Linux, macOS, and Windows.
The loop-seam and gait algorithms are golden-tested against the
production numbers of the pipeline they were extracted from. See
[DESIGN.md](https://github.com/mmannerm/animsmith/blob/main/DESIGN.md)
for the full design and roadmap. `fix`
(quaternion hemisphere normalization) patches only the offending
animation bytes, so meshes, skins, and textures pass through
byte-identical. Next up are the remaining hard semantic checks and
additional machine-readable output formats.

## Install

For the CLI:

```console
$ cargo install animsmith
```

The default install includes FBX input and HTML reports. For a pure-Rust
glTF-only binary with no C build step on Linux, macOS, or Windows:

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

## CLI or Library?

Use the `animsmith` binary when you want a local tool, CI gate, or
artist-facing report. Use `animsmith-core` when you already have a Rust
pipeline and want to run the same measurements and checks inside your
own gate. Pair `animsmith-core` with exactly the loader crates you need:
`animsmith-gltf` for glTF/GLB and `animsmith-fbx` for FBX. The CLI crate
is not the embedding API; it is just one frontend over the same core.

## Quickstart

```console
$ animsmith lint clip.glb
clip.glb:
  warning[quat-flip] clip 'walk' bone 'hips' @0.533s: 1 hemisphere flip(s) ...
  note[constant-track] clip 'walk' bone 'ik_target': scale track has 90 keys but never moves — export bloat
0 error(s), 1 warning(s), 1 note(s)

$ animsmith lint export.fbx           # lint a DCC export directly
$ animsmith measure clip.glb          # machine-readable measurements
$ animsmith inspect clip.glb          # skeleton + clips + detected rig profile
$ animsmith report clip.glb -o report.html   # offline HTML: 3D playback + charts
$ animsmith convert export.fbx -o clip.glb   # skeleton+animation glTF
$ animsmith diff old.glb new.glb      # did the re-export change what matters?
$ animsmith fix clip.glb -o fixed.glb # repair quat flips, byte-surgically
$ animsmith fix clip.glb --dry-run    # inspect repairs without writing
```

From a source checkout, prefix the same commands with
`cargo run -p animsmith --`.

Exit codes: `0` clean/warnings-only, `1` error findings (`--deny-warnings`
promotes), `2` operator error.

The HTML report is a single self-contained file (no CDN, works offline,
attach it to a PR): a small hand-written WebGL viewer plays back **the
exact pose-grid frames the checks judged** — no re-sampling in JS —
with foot/root trails, metric charts synced to the scrubber, and a
clickable findings list. FBX support bundles the ufbx C library; build
with `--no-default-features` for a pure-Rust glTF-only binary. In that
build, glTF inspect/measure/lint/transform/fix/diff stay available; the
HTML `report` command requires the `report` feature and the FBX-oriented
`convert` command requires the `fbx` feature.

More documentation:

- [CLI reference](https://github.com/mmannerm/animsmith/blob/main/docs/cli.md)
- [Embedding API](https://github.com/mmannerm/animsmith/blob/main/docs/embedding.md)
- [Machine-readable output](https://github.com/mmannerm/animsmith/blob/main/docs/output.md)

## Checks

Mechanical (always on, no config needed):

| id | severity | what |
|---|---|---|
| `nan` | error | NaN/Inf in key times or values |
| `time-monotonic` | error | non-increasing/negative key times; late first key (note) |
| `quat-norm` | error | non-unit rotation keys |
| `quat-flip` | warning | adjacent keys on opposite hemispheres (long-way slerp) |
| `duration-sanity` | error/warning | degenerate duration; channels ending at different times; empty clips |
| `scale-keys` | warning | animated scale present; non-uniform scale |
| `constant-track` | note | multi-key tracks that never move (export bloat) |

Semantic (driven by declared expectations + rig roles):

| id | severity | what |
|---|---|---|
| `loop-seam` | error | a declared loop's feet-relative-to-hips wrap discontinuity vs its neighbouring in-clip steps |
| `gait-group` | error | stride-phase (L−R foot-height fundamental) spread across a declared directional blend ring |
| `root-motion-speed` | error | measured horizontal root travel vs the declared `speed_mps` pin; stray pins on stationary clips |
| `missing-bones` | error | declared `animates_bones` absent from the skeleton or carrying no keys |
| `frozen-bone` | error | a required bone whose rotation never exceeds the floor (T-posed limb, wrong-source slice) |

## Configuration

`animsmith.toml` (auto-loaded from the working directory, or `--config`):

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

See the
[worked config](https://github.com/mmannerm/animsmith/blob/main/examples/character.animsmith.toml)
for a real contract-style example.
Checks whose rig roles don't resolve are skipped with a note — never a
false failure. `--select`, `--allow`, and `[checks.*] severity`
(including `"off"`) control what runs and how hard it fails.

## Workspace

- [`animsmith-core`](https://github.com/mmannerm/animsmith/tree/main/crates/animsmith-core) — engine-agnostic data model,
  game-runtime-like sampler (`PoseGrid`), measurements, checks. What
  pipelines embed.
- [`animsmith-gltf`](https://github.com/mmannerm/animsmith/tree/main/crates/animsmith-gltf) — glTF/GLB ingestion + the
  glTF writer behind `convert`.
- [`animsmith-fbx`](https://github.com/mmannerm/animsmith/tree/main/crates/animsmith-fbx) — FBX ingestion via ufbx
  (isolates the C build; optional).
- [`animsmith-report`](https://github.com/mmannerm/animsmith/tree/main/crates/animsmith-report) — the self-contained
  HTML report.
- [`animsmith`](https://github.com/mmannerm/animsmith/tree/main/crates/animsmith) — the CLI.

## Contributing

All merges go through PRs with Conventional Commits (CI enforces both);
merged `feat`/`fix` commits auto-publish a GitHub Release. Agent
workflow, architecture invariants, and the audit gate live in
[.agent-instructions/shared.md](https://github.com/mmannerm/animsmith/blob/main/.agent-instructions/shared.md).

## License

MIT OR Apache-2.0.
