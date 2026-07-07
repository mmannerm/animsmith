# animsmith examples

A cookbook of runnable workflows. Each section is a self-contained task
you can copy into your own project — several double as CI/acceptance
gates.

Commands that reference [`examples/assets/`](../examples/assets/) run
against small assets committed there, so you can follow along from a
source checkout with no downloads; the assets are procedurally
generated (see [their README](../examples/assets/README.md) for
provenance and how to regenerate them). Sections that operate on your
own rig use placeholder filenames — `character.glb`, `export.fbx`,
`old.glb` — for assets you supply.

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
| 0 | Clean, or warnings only. |
| 1 | A failing finding, a significant `diff`, or pending `fix --dry-run` repairs. |
| 2 | Operator error: unreadable input, bad config, bad flags. |

---

## 1. A first CLI gate

Look at an asset, measure it, and lint it. `inspect` summarizes
structure; `measure` reports metrics without judgment; `lint` runs the
checks and sets the exit code.

```console
$ animsmith inspect examples/assets/clip.glb
examples/assets/clip.glb
rig profile: none detected
skeleton: 2 bones
  root
    spine
clips: 1
  swing: 1.000s, 1 tracks, 5 keys max

$ animsmith lint examples/assets/clip.glb
examples/assets/clip.glb: clean
0 error(s), 0 warning(s), 0 note(s)          # exits 0
```

A defective asset produces findings and a non-zero exit:

```console
$ animsmith lint examples/assets/clip-dirty.glb
examples/assets/clip-dirty.glb:
  error[quat-norm] clip 'swing' bone 'spine' @0.500s: non-unit rotation key ...
  warning[quat-flip] clip 'swing' bone 'spine' @0.750s: 2 hemisphere flip(s) ...
1 error(s), 1 warning(s), 0 note(s)          # exits 1
```

Warnings alone keep the exit code at 0. Use `--deny-warnings` when CI
should fail on warnings too:

```console
$ animsmith lint --deny-warnings examples/assets/clip-dirty.glb   # exits 1
```

For machine consumption, `--format json` emits a versioned envelope
(see [output.md](output.md)):

```console
$ animsmith lint --format json examples/assets/clip-dirty.glb
{
  "schema_version": 1,
  "command": "lint",
  "summary": { "files": 1, "findings": { "error": 1, "warning": 1, "note": 0 } },
  "files": [
    {
      "path": "examples/assets/clip-dirty.glb",
      "findings": [
        { "check_id": "quat-norm", "severity": "error", "clip": "swing",
          "bone": "spine", "time_s": 0.5, "measured": 1.05, "expected": 1.0,
          "message": "non-unit rotation key (worst at key 2)" },
        { "check_id": "quat-flip", "severity": "warning", "clip": "swing",
          "bone": "spine", "time_s": 0.75, "measured": 2.0,
          "message": "2 hemisphere flip(s) between adjacent rotation keys ..." }
      ]
    }
  ]
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
fixed.glb: clean                             # exits 0
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

`transform` applies mechanical pipeline edits — slice a window, hold the
final pose, re-anchor a gait cycle, or resample the frame rate. Geometry
passes through unchanged.

Slice a sub-window (retimed to start at 0):

```console
$ animsmith transform examples/assets/clip.glb -o sliced.glb --slice 0.5:1.0
  sliced 'swing' to [0.5:1]s (3 keys max)
wrote sliced.glb (1 clip(s) transformed)

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
wrote held.glb (1 clip(s) transformed)
```

Other transforms: `--gait-anchor` rotates a cyclic clip so its stride
anchor lands at t=0 (needs resolvable hips + feet roles), and `--fps N`
sets the grid used for retiming. See
[cli.md](cli.md#commands) for the full flag list.

---

## 4. A project contract config

Mechanical checks run with no config. The semantic checks —
`loop-seam`, `gait-group`, `root-motion-speed`, `frozen-bone`,
`in-place`, `foot-slide` — need declared expectations *and* resolvable
rig roles. Without a resolved rig they skip with a note rather than
guess, so a config that pins a `[rig] profile` (or inline `[rig.roles]`)
is what makes them fire.

[`examples/character.animsmith.toml`](../examples/character.animsmith.toml)
is a worked game-character contract: locomotion loops, typed gait rings,
a seam cap, a frozen-bone floor, and a severity override. Copy its shape
into your game repo and point `lint` at it:

```console
$ animsmith lint --config examples/character.animsmith.toml character.glb
```

Here `character.glb` is your own rig — the contract declares locomotion
clips and gait rings, so it needs a real skeleton to fire the semantic
checks (the committed `clip.glb` is a two-bone toy that resolves no rig).

`animsmith.toml` is auto-loaded from the working directory when present,
so committing one next to your assets makes every bare `animsmith lint`
enforce the contract.

You can also steer a run without a config file. `--select` restricts the
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
1 error(s), 0 warning(s), 1 note(s)          # exits 1
```

See the [README configuration section](../README.md#configuration) for
the full key reference.

---

## 5. Migrating an FBX export _(default features only)_

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

### Getting a test asset

We do not ship third-party assets. To try this on a real rig:

- **Mixamo** — free with an Adobe ID and royalty-free for personal and
  commercial use. Download a character + animation as FBX, then
  `convert` it. Mixamo is also a built-in rig profile, so
  `[rig] profile = "mixamo"` resolves its roles for the semantic checks.
  Check Adobe's current terms before redistributing any downloaded
  asset; the safe path is to keep them out of your repo.
- **CC0 / public-domain sources** for assets you want to commit.
- Or **generate your own** — see the
  [asset generator](../crates/animsmith/examples/gen_example_assets.rs)
  this repo uses for its own fixtures.

---

## 6. Embedding animsmith as a library gate

Pipelines can skip the CLI and drive the check catalog directly: load a
document, resolve rig roles, build a `Config` from your own contract
format, measure, run the checks, and map findings to your gate.

The runnable walkthrough is
[`crates/animsmith/examples/embed.rs`](../crates/animsmith/examples/embed.rs),
paired with [embedding.md](embedding.md):

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
