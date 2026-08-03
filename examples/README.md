# animsmith examples

A cookbook of runnable workflows. Each section is a self-contained task
you can copy into your own project — several double as CI/acceptance
gates. Each workflow addresses a runtime failure described in the
[game-ready clips guide](../docs/game-ready-clips.md), which explains
*why* the checks behind these commands exist. For the larger process
that turns raw source assets into engine-facing outputs, see the
[pipeline scenario guide](../docs/pipeline-scenarios.md).

Commands that reference [`examples/assets/`](assets/) run
against small assets committed there, so you can follow along from a
source checkout with no downloads; the assets are procedurally
generated (see [their README](assets/README.md) for
provenance and how to regenerate them). The conversion and reporting
section operates on assets you supply, using placeholder filenames —
`export.fbx`, `old.glb` — for your own exports and baselines.

Transcripts are real command output. Long finding messages are elided as
`...` and the JSON envelope is shown abridged; everything else is
verbatim, including the exit-code annotations in `# comments`.

## Running the commands

Examples use the installed CLI form, `animsmith <command>`. From a
source checkout, prefix each command with `cargo run -p animsmith --`:

```console
animsmith lint examples/assets/clip.glb
cargo run -p animsmith -- lint examples/assets/clip.glb   # source checkout
```

Two examples need the default build's feature-gated commands (`report`,
`convert`); they are marked **default features only**. Everything else
works in the pure-Rust `--no-default-features` build too.

## Exit codes

Every example relies on the same convention, so scripts can gate on it:

| Code | Meaning |
|---:|---|
| 0 | No failing findings; warnings, notes, and coverage gaps may remain. |
| 1 | A failing finding, a significant `diff`, or pending `fix --dry-run` repairs. |
| 2 | Operator error: unreadable input, bad config, bad flags. |

---

## 1. A first CLI gate

When a clip enters CI, the first question is not just whether the file
parses — it is whether the motion is safe to ship. A gate built from
`inspect`, `measure`, and `lint` catches the gap described in
[a valid file is not a usable clip](../docs/game-ready-clips.md#a-valid-file-is-not-a-usable-clip):
valid containers can still hide content problems that only show up
after import, bake, or playtest. `inspect` summarizes structure;
`measure` reports metrics without judgment; `lint` runs the checks and
sets the exit code.

```console
$ animsmith inspect examples/assets/clip.glb
examples/assets/clip.glb
rig profile: none detected
skeleton: 2 bones
  root
    spine
materials: 0
mesh instances: 0
clips: 1
  swing: 1.000s, 1 tracks, 5 keys max

$ animsmith lint examples/assets/clip.glb
examples/assets/clip.glb:
  coverage[bind-pose] first_frame_rest_delta 'swing': insufficient_rotation_evidence: ...
0 error(s), 0 warning(s), 0 note(s), 1 coverage gap(s)   # exits 0
```

A defective asset produces findings and a non-zero exit:

```console
$ animsmith lint examples/assets/clip-dirty.glb
examples/assets/clip-dirty.glb:
  error[quat-norm] clip 'swing' bone 'spine' @0.500s: non-unit rotation key ...
  warning[quat-flip] clip 'swing' bone 'spine' @0.750s: 2 hemisphere flip(s) ...
1 error(s), 1 warning(s), 0 note(s), 0 coverage gap(s)   # exits 1
```

Warnings alone keep the exit code at 0. Use `--deny-warnings` when CI
should fail on warnings too:

```console
$ animsmith lint --deny-warnings examples/assets/clip-dirty.glb   # exits 1
```

For machine consumption, `--format json` emits the v2 result envelope
(see [output.md](../docs/output.md)). This `jq` projection keeps the example
short while showing where content findings and independently versioned
measurement evidence live:

```console
$ animsmith lint --format json examples/assets/clip-dirty.glb | jq \
    '{schema_version, schema, command,
      check: (.files[0].checks[] | select(.check_id == "quat-norm")),
      measurements: (.files[0].measurements | {schema_version, schema})}'
{
  "schema_version": 2,
  "schema": "urn:animsmith:schema:output:2",
  "command": "lint",
  "check": {
    "check_id": "quat-norm",
    "selection": "selected",
    "configuration": "enabled",
    "applicability": "applicable",
    "evaluation": "complete",
    "findings": [
      { "check_id": "quat-norm", "severity": "error", "clip": "swing",
        "bone": "spine", "time_s": 0.5, "measured": 1.05, "expected": 1.0,
        "message": "non-unit rotation key (worst at key 2)" }
    ]
  },
  "measurements": {
    "schema_version": 7,
    "schema": "urn:animsmith:schema:measurements:7",
    "clips": {},
    "mesh_definitions": [],
    "node_instances": [],
    "scenes": [],
    "skeleton_source_coverage": "unavailable",
    "skeleton_nodes": [],
    "skins": [],
    "material_resource_coverage": "complete",
    "material_definitions": [],
    "textures": [],
    "images": []
  }
}
```

### As a CI gate

Lint every clip and fail the job on any error (add `--deny-warnings` to
also fail on warnings):

```yaml
# .github/workflows/animate.yml
- run: cargo install animsmith --no-default-features
- run: animsmith lint --deny-warnings clips/*.glb
```

The step's exit code is the gate: 1 fails the job, 0 passes it.

---

## 2. Repairing an asset

Use repair when a clip plays as a sudden flicker, spin, or explosion,
but the authored motion is still recoverable. The symptoms in
[the pose flickers, spins, or explodes](../docs/game-ready-clips.md#the-pose-flickers-spins-or-explodes)
are often quaternion representation problems, so `fix --dry-run` lets a
gate report the exact lossless repairs before writing anything.

`quat-norm` and `quat-flip` are not just checks — they are lossless,
idempotent repairs. `fix --dry-run` is the check mode: it reports what
it *would* repair and exits 1 if anything is pending, writing nothing.

```console
$ animsmith fix examples/assets/clip-dirty.glb --dry-run
  would fix[quat-norm] clip 'swing' bone 'spine': 1 key(s) unit-normalized
1 key(s) would be fixed across 1 track(s) -> no output written
  would fix[quat-flip] clip 'swing' bone 'spine': 1 key(s) hemisphere-normalized
1 key(s) would be fixed across 1 track(s) -> no output written   # exits 1
```

Write the repaired asset with `-o` (or `--in-place`), then confirm it
lints clean:

```console
$ animsmith fix examples/assets/clip-dirty.glb -o fixed.glb
  fixed[quat-norm] clip 'swing' bone 'spine': 1 key(s) unit-normalized
1 key(s) fixed across 1 track(s) -> fixed.glb
  fixed[quat-flip] clip 'swing' bone 'spine': 1 key(s) hemisphere-normalized
1 key(s) fixed across 1 track(s) -> fixed.glb

$ animsmith lint fixed.glb
fixed.glb:
  coverage[bind-pose] first_frame_rest_delta 'swing': insufficient_rotation_evidence: ...
0 error(s), 0 warning(s), 0 note(s), 1 coverage gap(s)   # exits 0
```

Because the repairs are lossless, `diff` confirms no measurement moved —
the fix changed representation, not motion:

```console
$ animsmith diff examples/assets/clip-dirty.glb fixed.glb
no significant movement
0 significant change(s)                      # exits 0
```

Pin an exact repair set with `--repair id[,id]` (`animsmith fix --help`
lists the ids). Repairs that cannot be applied byte-surgically — data-URI
`.gltf` buffers, cubic tracks, quantized rotations — are reported as
`skipped[...]` and do not fail the check. Gate on `lint` when detection
alone should fail CI.

---

## 3. Editing a clip

Use editing when the animation content is right but the pipeline cut is
wrong: a capture has junk at the head, a one-shot needs a final hold, or
a loop's stride anchor lands in the wrong place. Those are the cases in
[the clip is the wrong length or freezes at the end](../docs/game-ready-clips.md#the-clip-is-the-wrong-length-or-freezes-at-the-end)
and [the loop pops](../docs/game-ready-clips.md#the-loop-pops), where a
mechanical transform can make the clip conform without touching
geometry.

`transform` applies mechanical pipeline edits — slice a window, hold the
final pose, re-anchor a gait cycle, or remove an eligible duplicate loop
endpoint. Geometry passes through unchanged.

Slice a sub-window (retimed to start at 0):

```console
$ animsmith transform examples/assets/clip.glb -o sliced.glb --slice 0.5:1.0
  sliced 'swing' to [0.5:1]s (3 keys max)
wrote sliced.glb (2 node(s), 1 clip(s), 0 mesh(es) / 0 position(s), 0 material(s))

$ animsmith diff examples/assets/clip.glb sliced.glb
  swing duration_s: moved 1.0000 -> 0.5000
  swing frame_count: moved 5.0000 -> 3.0000
  swing bone_rotation_range_deg[spine]: moved 22.9183 -> 11.4591
3 significant change(s)                       # exits 1
```

Extend the final pose (useful for hold frames at the end of a one-shot):

```console
$ animsmith transform examples/assets/clip.glb -o held.glb --hold-extend 0.5
  hold-extended 'swing' by 0.5s
wrote held.glb (2 node(s), 1 clip(s), 0 mesh(es) / 0 position(s), 0 material(s))
```

Other transforms: `--gait-anchor` rotates a cyclic clip so its stride
anchor lands at t=0 (needs resolvable hips + feet roles), and `--fps N`
sets the grid used for retiming. See
[cli.md](../docs/cli.md#commands) for the full flag list.

For a loop exported with frame 0 copied again at the inclusive final frame,
use `--drop-duplicate-loop-endpoint` only after the default warning identifies
the strict authored-key case:

```console
$ animsmith transform cycle.glb -o cycle-open.glb --drop-duplicate-loop-endpoint
```

The transform removes the same closing key count from every channel and re-pins
the duration; it does not repair a stationary hold, root-travel/nonclosing
clip, mismatched timelines, endpoint tangents, or retargeting damage. The open
cycle is for engines that wrap over duration, so its inclusive
`loop-closure` result is expected to change.

---

## 4. A project contract config

Use a contract config when the failure depends on what your game expects,
not on file validity alone. [The character glides or runs in
place](../docs/game-ready-clips.md#the-character-glides-or-runs-in-place),
[feet skate when clips blend](../docs/game-ready-clips.md#feet-skate-when-clips-blend),
and [feet slide within one
clip](../docs/game-ready-clips.md#feet-slide-within-one-clip) all need
declared locomotion or blend assumptions before animsmith can judge
them.

Mechanical checks run with no config. Contract-aware checks run only for the
expectations you declare. `duplicate-loop-endpoint`, `loop-closure`, and
`loop-seam-vel` need only `loop = true`; role-dependent checks such as
`loop-seam`, `gait-group`, `root-motion-speed`, `in-place`, and `foot-slide`
also need resolvable rig roles. Without those roles they report a typed coverage
gap rather than guess, so a config that pins a `[rig] profile` (or inline
`[rig.roles]`) is what makes them fire.

`examples/assets/walk.glb` is a committed rig for this: a hips + two-foot
skeleton with a one-second walk cycle. Its bone names resolve a built-in
profile, so `inspect` binds the rig with no config at all:

```console
$ animsmith inspect examples/assets/walk.glb
examples/assets/walk.glb
rig profile: ue-mannequin (3 roles)
  hips         -> pelvis
  left_foot    -> foot_l
  right_foot   -> foot_r
skeleton: 3 bones
  pelvis
    foot_l
    foot_r
materials: 0
mesh instances: 0
clips: 1
  walk: 1.000s, 2 tracks, 33 keys max
```

`measure` reports the semantic metrics the checks judge — per-bone C0 pose
closure and C1 seam velocity, the feet-relative loop-seam ratio (≈ 0 here,
since this cycle returns its feet exactly), gait phase, and L/R foot amplitude:

```console
$ animsmith measure examples/assets/walk.glb          # --format json
{
  "schema_version": 2,
  "schema": "urn:animsmith:schema:output:2",
  "tool": { "name": "animsmith", "version": "0.1.0",
            "source": { "revision": null, "dirty": null } },
  "command": "measure",
  "summary": { "files": 1 },
  "files": [
    {
      "path": "examples/assets/walk.glb",
      "rig": { "profile": "ue-mannequin", "resolved_roles": {
        "hips": "pelvis", "left_foot": "foot_l", "right_foot": "foot_r" } },
      "measurements": {
        "schema_version": 7,
        "schema": "urn:animsmith:schema:measurements:7",
        "clips": { "walk": {
          "duration_s": 1.0, "frame_count": 33,
          "animated_bones": ["foot_l", "foot_r"],
          "bone_rotation_range_deg": {},
          "loop_continuity": { "bones": [
            { "bone_index": 0, "bone_name": "pelvis",
              "position_delta_m": 0.0, "rotation_delta_deg": 0.0,
              "seam_velocity_delta_mps": 0.0 },
            { "bone_index": 1, "bone_name": "foot_l",
              "position_delta_m": 3.7e-17, "rotation_delta_deg": 0.0,
              "seam_velocity_delta_mps": 0.0 },
            { "bone_index": 2, "bone_name": "foot_r",
              "position_delta_m": 3.7e-17, "rotation_delta_deg": 0.0,
              "seam_velocity_delta_mps": 0.0 }
          ] },
          "loop_seam_ratio": 1.2e-15,
          "gait": { "phase": 0.75, "lr_amplitude_m": 0.2 },
          "speed_mps": 0.0
        } },
        "mesh_definitions": [],
        "node_instances": [],
        "scenes": [],
        "skeleton_source_coverage": "unavailable",
        "skeleton_nodes": [],
        "skins": [],
        "material_resource_coverage": "complete",
        "material_definitions": [],
        "textures": [],
        "images": []
      }
    }
  ]
}
```

[`examples/walk.animsmith.toml`](walk.animsmith.toml) is the
contract: it declares the clip a loop (which arms all three loop checks) and
in-place, and caps their tolerances. Against the clean rig every semantic check
passes:

```console
$ animsmith lint --config examples/walk.animsmith.toml examples/assets/walk.glb
examples/assets/walk.glb:
  coverage[bind-pose] first_frame_rest_delta 'walk': insufficient_rotation_evidence: ...
0 error(s), 0 warning(s), 0 note(s), 1 coverage gap(s)   # exits 0
```

`examples/assets/walk-dirty.glb` is the same rig with the clip cut a
quarter-cycle short, so the feet never return to their first-frame pose —
the classic popped loop seam. The same contract catches it:

```console
$ animsmith lint --config examples/walk.animsmith.toml examples/assets/walk-dirty.glb
examples/assets/walk-dirty.glb:
  error[loop-closure] clip 'walk' bone 'foot_r' @1.000s: loop does not close
    in position: bone 'foot_r' is 0.1581 m from its first-frame model-space
    position (cap 0.0100 m) (measured 0.1581, expected 0.0100)
  error[loop-seam] clip 'walk' @1.000s: loop seam pops: wrap discontinuity
    is 6.82× the neighbouring in-clip step (cap 1.60) — the clip does not
    close its cycle (measured 6.8152, expected 1.6000)
  error[loop-seam-vel] clip 'walk' bone 'foot_r' @1.000s: loop velocity
    changes at the seam: bone 'foot_r' differs by 0.7972 m/s between the
    incoming and outgoing model-space velocities (cap 0.1000 m/s)
3 error(s), 0 warning(s), 0 note(s), 1 coverage gap(s)  # exits 1
```

The contract is load-bearing: a bare `animsmith lint examples/assets/walk-dirty.glb`
(no config) reports no findings — with no `loop = true` declared, the three
loop checks are explicitly not applicable. Semantic checks
enforce *your* declared expectations, not a guess.

### Scaling up to a full character

[`examples/character.animsmith.toml`](character.animsmith.toml)
is the full game-character shape the small walk contract grows into:

```toml
[rig]
profile = "auto"            # or mixamo / ue-mannequin / humanoid

[checks.loop-seam]
max_ratio = 1.6             # per-check tuning
[checks.loop-closure]
max_position_delta_m = 0.01
max_rotation_delta_deg = 1.0
[checks.loop-seam-vel]
max_velocity_delta_mps = 0.1
[checks.frozen-bone]
min_rotation_deg = 0.5
[checks.quat-flip]
severity = "note"           # demote while an upstream fix lands

[clips."run_*"]             # glob: every run clip loops
loop = true
[clips.run_forward]
duration_s = { value = 1.033, tolerance = 0.02 } # authored clip-length contract
speed_mps = { value = 3.1, tolerance = 0.25 }   # root-motion contract

[gait_groups.run-ring]      # a directional blend ring
clips = ["run_forward", "run_backward", "run_left", "run_right"]
max_gait_phase_spread = 0.15   # members must stride in phase
min_lr_amplitude_m = 0.03      # exclude near-idle members
```

A `gait_groups` block is the payoff for a real character: it holds every
clip in a directional blend ring to the same stride phase, so runtime
blends between them don't skate. `animsmith.toml` is auto-loaded from the
working directory, so committing one next to your assets makes every bare
`animsmith lint` enforce the contract.

### Steering a run without a config

You can also shape a run from the command line. `--select` restricts the
run set, `--allow` suppresses findings, and `[checks.<id>] severity`
(including `"off"`) reshapes how hard each check fails:

```console
$ animsmith lint --select quat-norm examples/assets/clip-dirty.glb   # only quat-norm
$ animsmith lint --allow quat-flip examples/assets/clip-dirty.glb    # hide quat-flip
```

Demote a check while an upstream fix is pending (a `[checks.quat-flip]`
`severity = "note"` override turns the warning into a note):

```console
$ cat demote.toml
[checks.quat-flip]
severity = "note"

$ animsmith lint --config demote.toml examples/assets/clip-dirty.glb
  error[quat-norm] clip 'swing' bone 'spine' @0.500s: non-unit rotation key ...
  note[quat-flip] clip 'swing' bone 'spine' @0.750s: 2 hemisphere flip(s) ...
1 error(s), 0 warning(s), 1 note(s), 0 coverage gap(s)   # exits 1
```

See the [README configuration section](../README.md#configuration) for
the full key reference.

---

## 5. Converting exports and generating reports _(default features only)_

Use this workflow when a DCC or marketplace export reaches your importer
but brings bloated constant tracks, stray scale keys, or a rig the
retargeter cannot trust. That is the hygiene side of [the file is
bloated, or the retargeter
chokes](../docs/game-ready-clips.md#the-file-is-bloated-or-the-retargeter-chokes)
and the rig-contract side of [a limb is T-posed, or a bone never
moves](../docs/game-ready-clips.md#a-limb-is-t-posed-or-a-bone-never-moves):
convert to glTF, measure/lint/report, then compare against the previous
asset before committing the migration.

`convert` normalizes an FBX (or glTF) export into a clean glTF, and
`report` renders a self-contained HTML report with skeleton playback and
metric charts. Both are in the default build; a `--no-default-features`
binary omits them.

```console
$ animsmith convert export.fbx -o clip.glb
$ animsmith measure clip.glb
$ animsmith report clip.glb -o report.html
$ animsmith diff old.glb clip.glb
```

When one authoritative skinned base and its animation takes live in separate
files, use the versioned [`character-assembly.toml`](character-assembly.toml)
recipe instead. `assemble` produces one GLB and evidence pair; source-package
extraction and project publication stay outside animsmith.

```console
$ animsmith assemble examples/character-assembly.toml \
    -o character.glb --evidence character.assembly.json
```

### Getting a test asset

We do not ship third-party assets. To try this on a real rig:

- **Mixamo** — free with an Adobe ID and royalty-free for personal and
  commercial use. Download a character + animation as FBX, then
  `convert` it. Mixamo is also a built-in rig profile, so
  `[rig] profile = "mixamo"` resolves its roles for the semantic checks.
  Check Adobe's current terms before redistributing any downloaded
  asset; the safe path is to keep them out of your repo. The
  [Mixamo tutorial](../docs/mixamo-tutorial.md) takes one download
  end-to-end, from this convert step through a contract config.
- **CC0 / public-domain sources** for assets you want to commit.
- Or **generate your own** — see the
  [asset generator](../crates/animsmith/examples/gen_example_assets.rs)
  this repo uses for its own fixtures.

---

## 6. Embedding animsmith as a library gate

Use embedding when the team already has an importer or build pipeline and
wants animsmith's checks as a library step instead of a separate shell
command. The pipeline-library use case is the payoff in
[why animsmith](../docs/why-animsmith.md): load your
asset once, map your own contract into `Config`, and surface findings in
the same gate that owns the rest of your asset rules.

Pipelines can skip the CLI and drive the check catalog directly: load a
document, resolve rig roles, build a `Config` from your own contract
format, measure, run the checks, and map findings to your gate.

The runnable walkthrough is
[`crates/animsmith/examples/embed.rs`](../crates/animsmith/examples/embed.rs),
paired with [embedding.md](../docs/embedding.md):

```console
$ cargo run -p animsmith --example embed
```

It exits 1 on purpose — the example declares a deliberately wrong
expectation to demonstrate a failing gate, not an accidental error.

---

## Feature matrix

| Example | Needs |
|---|---|
| 1 first gate, 2 repair, 3 transform, 4 config | any build (incl. `--no-default-features`) |
| 5 convert / report | default features (`fbx`, `report`) |
| 6 embedding | library crates |

## Asset policy

Assets committed to this repo are procedurally generated or CC0, with
provenance recorded. Third-party assets (Mixamo and similar) are used
via documented download steps, not checked-in bytes or download scripts,
unless their terms clearly permit redistribution.
