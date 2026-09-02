# First lint in 60 seconds

Two synthetic clips from this repository are enough to see what a finding
looks like, repair one, and watch a declared contract catch a popped loop.
The commands run from a checkout of the repository. With an installed binary
and no checkout, download the files they name from
[`examples/assets/`](../examples/assets/README.md) and
[`examples/walk.animsmith.toml`](../examples/walk.animsmith.toml) and adjust
the paths.

## 1. Lint a clip that flickers

Mechanical checks need no configuration. This clip carries one non-unit
rotation key and one hemisphere flip, the two representation defects that make
a pose flicker or spin the long way round:

```console
$ animsmith lint examples/assets/clip-dirty.glb
examples/assets/clip-dirty.glb:
  error[quat-norm] clip 'swing' bone 'spine' @0.500s: non-unit rotation key (worst at key 2) (measured 1.0500, expected 1.0000)
  warning[quat-flip] clip 'swing' bone 'spine' @0.750s: 2 hemisphere flip(s) between adjacent rotation keys (first between keys 2 and 3) — engines without neighborhood correction will spin the long way (measured 2.0000)
  coverage[bind-pose] insufficient_rotation_evidence ×1 (scopes: first_frame_rest_delta; subjects: swing): only 1 usable first-frame rotation track(s); at least three are required
1 error(s), 1 warning(s), 0 note(s), 1 coverage gap(s), 0 available prediction facet(s), 0 required-unavailable prediction facet(s)   # exits 1
```

Every finding names the check, the clip, the bone and the time, then the
measured value against the expected one. The `coverage` line is not a
finding: a check that lacked the evidence it needs (here, at least three
rotation tracks) says so instead of guessing, and never fails the run.

## 2. Repair what is mechanically safe

Both defects are lossless to repair, so `fix` can do it. `--dry-run` reports
the pending repairs and exits 1 without writing anything; `-o` writes the
repaired copy.

```console
$ animsmith fix examples/assets/clip-dirty.glb --dry-run
  would fix[quat-norm] clip 'swing' bone 'spine': 1 key(s) unit-normalized
1 key(s) would be fixed across 1 track(s) -> no output written
  would fix[quat-flip] clip 'swing' bone 'spine': 1 key(s) hemisphere-normalized
1 key(s) would be fixed across 1 track(s) -> no output written   # exits 1

$ animsmith fix examples/assets/clip-dirty.glb -o fixed.glb
  fixed[quat-norm] clip 'swing' bone 'spine': 1 key(s) unit-normalized
1 key(s) fixed across 1 track(s) -> fixed.glb
  fixed[quat-flip] clip 'swing' bone 'spine': 1 key(s) hemisphere-normalized
1 key(s) fixed across 1 track(s) -> fixed.glb   # exits 0

$ animsmith lint fixed.glb
fixed.glb:
  coverage[bind-pose] insufficient_rotation_evidence ×1 (scopes: first_frame_rest_delta; subjects: swing): only 1 usable first-frame rotation track(s); at least three are required
0 error(s), 0 warning(s), 0 note(s), 1 coverage gap(s), 0 available prediction facet(s), 0 required-unavailable prediction facet(s)   # exits 0
```

The repair changes only the bytes of the keys it names; meshes, skins and
every other accessor pass through byte-identical, and `animsmith diff` reports
no measurement moved.

## 3. Lint against a contract

Whether a clip loops is your knowledge, not the file's. Without a declaration,
this walk cycle lints clean:

```console
$ animsmith lint examples/assets/walk-dirty.glb
examples/assets/walk-dirty.glb:
  coverage[bind-pose] insufficient_rotation_evidence ×1 (scopes: first_frame_rest_delta; subjects: walk): only 0 usable first-frame rotation track(s); at least three are required
0 error(s), 0 warning(s), 0 note(s), 1 coverage gap(s), 0 available prediction facet(s), 0 required-unavailable prediction facet(s)   # exits 0
```

Declare it as a loop through a project config and the seam checks arm:

```console
$ animsmith --config examples/walk.animsmith.toml lint examples/assets/walk-dirty.glb
examples/assets/walk-dirty.glb:
  error[loop-closure] clip 'walk' bone 'foot_r' @1.000s: loop does not close in position: bone 'foot_r' is 0.1581 m from its first-frame model-space position (cap 0.0100 m) (measured 0.1581, expected 0.0100)
  error[loop-seam] clip 'walk' @1.000s: loop seam pops: wrap discontinuity is 6.82× the neighbouring in-clip step (cap 1.60) — the clip does not close its cycle (measured 6.8152, expected 1.6000)
  error[loop-seam-vel] clip 'walk' bone 'foot_r' @1.000s: loop velocity changes at the seam: bone 'foot_r' differs by 0.7972 m/s between the incoming and outgoing model-space velocities (cap 0.1000 m/s) (measured 0.7972, expected 0.1000)
  coverage[bind-pose] insufficient_rotation_evidence ×1 (scopes: first_frame_rest_delta; subjects: walk): only 0 usable first-frame rotation track(s); at least three are required
3 error(s), 0 warning(s), 0 note(s), 1 coverage gap(s), 0 available prediction facet(s), 0 required-unavailable prediction facet(s)   # exits 1
```

The clip was cut a quarter-cycle short, so the right foot is 16 cm from where
the loop needs it to be. The [config file](../examples/walk.animsmith.toml)
is short and commented; most of the loop caps it sets are the defaults, and it
says where it departs from them — a stricter `0.5` degree per-clip rotation cap
(default `1.0`) and a `1.6` loop-seam ratio (default `1.5`).

## 4. Read the exit code

`0` means no failing findings (warnings, notes and coverage gaps may remain),
`1` means failing findings, `2` means an operator error such as a missing
file. `--deny-warnings` promotes warnings to failures for a stricter gate.

Next: [your first report](first-report.md) shows the same findings as
skeleton playback and charts. When something specific looks wrong in the
engine, start from the [symptoms](symptoms/README.md).
