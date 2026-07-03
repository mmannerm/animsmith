# animsmith

A workbench for skeletal animation clips. It answers the question every
game team answers by hand today — **does this clip have
game-engine-friendly characteristics?** — and then helps you make it
so. Lint broken quaternions, degenerate durations, popped loop seams,
gait-phase drift, and export bloat; convert straight from DCC exports;
and (roadmap) fix the mechanical problems in place.

glTF-Validator checks spec conformance; animsmith judges — and forges —
*content*. Nothing open-source did game-semantics clip validation
before this.

**Status: M2.** glTF/GLB **and FBX** input (via [ufbx](https://github.com/ufbx/ufbx));
mechanical + locomotion-semantics check sets; rig profiles
(mixamo / ue-mannequin / rauta-humanoid + auto-detect); `animsmith.toml`
config with per-clip expectations and gait groups; subcommands
`inspect` / `measure` / `lint` / `convert` / `report` / `diff`.
The loop-seam and gait algorithms are golden-tested against the
production numbers of the pipeline they were extracted from. See
[DESIGN.md](DESIGN.md) for the full design and roadmap: next up are the
remaining transform features — frame-range slice/trim + hold-extend,
gait-anchor rotation, and full mesh/skin conversion (a maintained
replacement for the archived FBX2glTF) — plus foot-slide and bind-pose
checks. `fix` (quaternion hemisphere normalization) already ships: it
patches only the offending animation bytes, so meshes, skins, and
textures pass through byte-identical.

## Quickstart

```console
$ cargo run -p animsmith -- lint clip.glb
clip.glb:
  warning[quat-flip] clip 'walk' bone 'hips' @0.533s: 1 hemisphere flip(s) ...
  note[constant-track] clip 'walk' bone 'ik_target': scale track has 90 keys but never moves — export bloat
0 error(s), 1 warning(s), 1 note(s)

$ cargo run -p animsmith -- lint export.fbx           # lint a DCC export directly
$ cargo run -p animsmith -- measure clip.glb          # machine-readable measurements
$ cargo run -p animsmith -- inspect clip.glb          # skeleton + clips + detected rig profile
$ cargo run -p animsmith -- report clip.glb -o report.html   # offline HTML: 3D playback + charts
$ cargo run -p animsmith -- convert export.fbx -o clip.glb   # skeleton+animation glTF
$ cargo run -p animsmith -- diff old.glb new.glb      # did the re-export change what matters?
$ cargo run -p animsmith -- fix clip.glb -o fixed.glb # repair quat hemisphere flips, byte-surgically
```

Exit codes: `0` clean/warnings-only, `1` error findings (`--deny-warnings`
promotes), `2` operator error.

The HTML report is a single self-contained file (no CDN, works offline,
attach it to a PR): a small hand-written WebGL viewer plays back **the
exact pose-grid frames the checks judged** — no re-sampling in JS —
with foot/root trails, metric charts synced to the scrubber, and a
clickable findings list. FBX support bundles the ufbx C library; build
with `--no-default-features` for a pure-Rust glTF-only binary.

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
profile = "auto"            # or mixamo / ue-mannequin / rauta-humanoid, or inline [rig.roles]

[checks.loop-seam]
max_ratio = 1.6

[clips."run_*"]
loop = true

[clips.run_forward]
speed_mps = { value = 3.1, tolerance = 0.25 }

[groups.run-ring]
clips = ["run_forward", "run_backward", "run_left", "run_right"]
max_gait_phase_spread = 0.15
min_lr_amplitude_m = 0.03
```

See [examples/rauta.animsmith.toml](examples/rauta.animsmith.toml) for a
real config mirroring the incubating project's animation contract.
Checks whose rig roles don't resolve are skipped with a note — never a
false failure. `--select`, `--allow`, and `[checks.*] severity`
(including `"off"`) control what runs and how hard it fails.

## Workspace

- [`animsmith-core`](crates/animsmith-core) — engine-agnostic data model,
  game-runtime-like sampler (`PoseGrid`), measurements, checks. What
  pipelines embed.
- [`animsmith-gltf`](crates/animsmith-gltf) — glTF/GLB ingestion + the
  glTF writer behind `convert`.
- [`animsmith-fbx`](crates/animsmith-fbx) — FBX ingestion via ufbx
  (isolates the C build; optional).
- [`animsmith-report`](crates/animsmith-report) — the self-contained
  HTML report.
- [`animsmith`](crates/animsmith) — the CLI.

## License

MIT OR Apache-2.0.
