# animlint

A linter for skeletal animation clips. It answers the question every
game team answers by hand today: **does this clip have
game-engine-friendly characteristics?** Broken quaternions, degenerate
durations, export bloat — and, as the catalog grows, loop closure, gait
phase, root-motion sanity, and foot slide.

glTF-Validator checks spec conformance; animlint judges *content*.
Nothing open-source did game-semantics clip validation before this.

**Status: M0 (walking skeleton).** glTF/GLB input, the mechanical check
set, `inspect` / `measure` / `lint`. See [DESIGN.md](DESIGN.md) for the
full design, check catalog, and roadmap (FBX via ufbx, self-contained
HTML reports with a 3D viewer, loop/gait/foot-slide checks).

## Quickstart

```console
$ cargo run -p animlint -- lint clip.glb
clip.glb:
  warning[quat-flip] clip 'walk' bone 'hips' @0.533s: 1 hemisphere flip(s) ...
  note[constant-track] clip 'walk' bone 'ik_target': scale track has 90 keys but never moves — export bloat
0 error(s), 1 warning(s), 1 note(s)

$ cargo run -p animlint -- measure clip.glb          # machine-readable measurements
$ cargo run -p animlint -- inspect clip.glb          # skeleton + clip summary
```

Exit codes: `0` clean/warnings-only, `1` error findings (`--deny-warnings`
promotes), `2` operator error.

## Checks (M0)

| id | severity | what |
|---|---|---|
| `nan` | error | NaN/Inf in key times or values |
| `time-monotonic` | error | non-increasing/negative key times; late first key (note) |
| `quat-norm` | error | non-unit rotation keys |
| `quat-flip` | warning | adjacent keys on opposite hemispheres (long-way slerp) |
| `duration-sanity` | error/warning | degenerate duration; channels ending at different times; empty clips |
| `scale-keys` | warning | animated scale present; non-uniform scale |
| `constant-track` | note | multi-key tracks that never move (export bloat) |

## Workspace

- [`animlint-core`](crates/animlint-core) — engine-agnostic data model,
  game-runtime-like sampler (`PoseGrid`), measurements, checks. What
  pipelines embed.
- [`animlint-gltf`](crates/animlint-gltf) — glTF/GLB ingestion.
- [`animlint`](crates/animlint) — the CLI.

## License

MIT OR Apache-2.0.
