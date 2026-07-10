# Mixamo to game-ready: an end-to-end tutorial

Take one free Mixamo download through the whole pipeline: download,
convert to glTF, inspect the rig, lint, fix what is mechanically
fixable, and grow a project contract config from the results.

This is the bring-your-own-asset counterpart to the
[examples cookbook](../examples/README.md): the cookbook runs against
small assets committed to this repo, while this tutorial runs against
a real marketplace rig you download yourself. Each step links into the
[game-ready clips guide](game-ready-clips.md) for *why* it matters
rather than re-explaining.

**About the transcripts.** The command output in this tutorial is
real, captured from a small generated rig that carries the same
`mixamorig:*` bone names, the same nine profile roles, and the same
`mixamo.com` take name as a real download (this repo does not ship
Mixamo's own files — see the
[asset policy](../examples/README.md#asset-policy)). Your download
will show many more bones — fingers, twist joints — and possibly more
findings; the roles, the commands, and the behavior are the same.

<!-- Contributor note, deliberately outside the rendered prose: the
stand-in transcripts and the committed contract are drift-guarded by
crates/animsmith/tests/mixamo_tutorial.rs. The download- and
FBX-specific steps (1-2) are docs-only because testing them would
require committing Mixamo bytes, which the asset policy forbids -
recorded here per issue 68's smoke-test-or-record requirement. -->

**Build requirements.** `convert` (step 2) ships in the default
install — it sits behind the `fbx` feature, which is enabled unless
you opted out. If you installed with `--no-default-features`, use the
default build for this one step (see
[feature flags](cli.md#feature-flags)). Everything from step 3 on
works in any build.

## 1. Download a clip from Mixamo

[Mixamo](https://www.mixamo.com/) is free with an Adobe ID, and Adobe
documents its animations as royalty-free for personal and commercial
projects — see
[Adobe's Mixamo FAQ](https://helpx.adobe.com/creative-cloud/faq/mixamo-faq.html)
for the current terms. That covers *using* the assets in your project;
it does not clearly permit redistributing the downloaded files, so
keep them out of public repos and do not commit them here.

1. Sign in and pick any character.
2. Search the animations for **Walking** and select a walk cycle.
3. Check the **In Place** option. Mixamo's default walks travel
   through the world; In Place keeps the hips over the origin, which
   is the shape most capsule-driven character controllers expect. (If
   your game uses root motion, download without In Place — step 6
   shows how the contract differs.)
4. Download as **FBX Binary** with skin. The skin is optional for
   linting, but keeping it lets `report` render the mesh later.

We recommend keeping the downloaded FBX as your immutable raw source
and treating every animsmith output as a derived artifact — the
[pipeline scenario guide](pipeline-scenarios.md) explains that
raw-vs-generated split.

## 2. Convert to glTF

```console
$ animsmith convert walking.fbx -o walking.glb
wrote walking.glb (65 bones, 1 clip(s), 1 mesh(es) / 21668 corners, 1 material(s))
```

animsmith lints FBX directly, so conversion is not required to run
checks — but glTF/GLB is the native format for the rest of the
pipeline (`fix`, `transform`, and `diff` operate on glTF), and a
convert-once step gives later commands a stable baseline to compare
against. A conversion that succeeds proves the container is
well-formed, not that the motion is usable —
[a valid file is not a usable clip](game-ready-clips.md#a-valid-file-is-not-a-usable-clip)
— which is what steps 4–6 judge. Bone and clip counts vary by
character; the numbers above are illustrative.

## 3. Inspect the rig

```console
$ animsmith inspect walking.glb
walking.glb
rig profile: mixamo (9 roles)
  hips         -> mixamorig:Hips
  spine        -> mixamorig:Spine
  head         -> mixamorig:Head
  left_foot    -> mixamorig:LeftFoot
  right_foot   -> mixamorig:RightFoot
  left_toe     -> mixamorig:LeftToeBase
  right_toe    -> mixamorig:RightToeBase
  left_hand    -> mixamorig:LeftHand
  right_hand   -> mixamorig:RightHand
skeleton: 9 bones
  mixamorig:Hips
    mixamorig:LeftFoot
      mixamorig:LeftToeBase
    mixamorig:RightFoot
      mixamorig:RightToeBase
    mixamorig:Spine
      mixamorig:Head
      mixamorig:LeftHand
      mixamorig:RightHand
clips: 1
  mixamo.com: 1.000s, 2 tracks, 33 keys max
```

These are the same nine roles a real download resolves; your skeleton
list will just be longer (~65 bones — the extras are fingers, twist
joints, and other bones that carry no role).

Two things to read off this output:

- **The `mixamo` profile resolved without any config.** Mixamo is a
  built-in rig profile: it binds the semantic roles from Mixamo's
  `mixamorig:*` bone names, which is what arms the role-based checks
  in step 6. Without a resolved rig, those checks skip rather than
  guess (see
  [from symptom to command](game-ready-clips.md#from-symptom-to-command)).
- **The clip is named `mixamo.com`.** Mixamo names every export's
  animation take `mixamo.com`, and animsmith uses the take name as the
  clip name. Your contract config must address the clip by this name.

> **The `mixamo` profile has no Root role.** Mixamo rigs have no
> dedicated root bone — `mixamorig:Hips` is the top of the hierarchy,
> and any root motion is baked into it. animsmith therefore judges
> `in-place` and `root-motion-speed` on the **Hips track** for Mixamo
> rigs: travel measurements still work, but they describe hip
> movement, not a separate locomotion root. If your engine workflow
> retargets hip motion onto a dedicated root bone (a common Unreal
> step), that happens downstream in your DCC or importer — lint the
> export you actually ship to the engine.

## 4. Lint the mechanical checks

```console
$ animsmith lint walking.glb
walking.glb: clean
0 error(s), 0 warning(s), 0 note(s)
```

With no config, only the mechanical checks run — the ones that need no
declared expectations: NaNs, non-unit or hemisphere-flipped
quaternions, degenerate durations, animated scale, constant tracks.
Mixamo's own exports are generally clean here; the checks earn their
keep on the same asset *after* it has been through your DCC, a
retargeter, or an exporter plugin, where
[flicker-inducing quaternion defects](game-ready-clips.md#the-pose-flickers-spins-or-explodes)
and
[export bloat](game-ready-clips.md#the-file-is-bloated-or-the-retargeter-chokes)
creep in. Findings like `constant-track` notes on a marketplace rig
are normal — a 65-bone skeleton usually carries tracks that never
move.

## 5. Fix what is mechanically fixable

```console
$ animsmith fix walking.glb --dry-run
0 key(s) would be fixed across 0 track(s) -> no output written
0 key(s) would be fixed across 0 track(s) -> no output written
```

(One summary line per default repair — `quat-norm`, then `quat-flip`.)
`fix` applies only repairs that are provably lossless: `quat-norm` and
`quat-flip`, the representation defects behind
[the pose flickers, spins, or explodes](game-ready-clips.md#the-pose-flickers-spins-or-explodes).
That makes it safe to run unconditionally; on a clean export it is a
no-op, as here. When a re-export does pick up defects:

```console
$ animsmith fix walking.glb -o walking-fixed.glb
$ animsmith lint walking-fixed.glb
$ animsmith diff walking.glb walking-fixed.glb
```

The re-lint proves the findings are gone; the `diff` proves the
repair moved no measured motion. The cookbook's
[repair section](../examples/README.md#2-repairing-an-asset) shows the
full transcript of that loop on a committed dirty asset.

## 6. Grow the contract config

The mechanical checks passing does not make the clip game-ready — a
walk whose loop pops or whose feet skate is mechanically pristine (see
[a valid file is not a usable clip](game-ready-clips.md#a-valid-file-is-not-a-usable-clip)).
The semantic checks need *your* expectations declared. Start from what
`measure` reports:

```console
$ animsmith measure --format json walking.glb
{
  "command": "measure",
  "files": [
    {
      "path": "walking.glb",
      "rig": { "profile": "mixamo", "resolved_roles": {
        "hips": "mixamorig:Hips", "spine": "mixamorig:Spine",
        "left_foot": "mixamorig:LeftFoot", "right_foot": "mixamorig:RightFoot" } },
      "measurements": {
        "mixamo.com": {
          "duration_s": 1.0, "frame_count": 33,
          "loop_seam_ratio": 1.2e-15,
          "gait": { "phase": 0.75, "lr_amplitude_m": 0.2 },
          "speed_mps": 0.0
        }
      }
    }
  ]
}
```

(Abridged — all nine roles resolve; head, toes, and hands are elided
here.) The numbers become the contract:
`loop_seam_ratio` near zero says the cycle closes, `speed_mps` of zero
confirms the In Place download, and the gait numbers seed a
[blend-ring group](game-ready-clips.md#feet-skate-when-clips-blend)
once you have more than one direction. Declare what must stay true in
`animsmith.toml`:

```toml
[rig]
profile = "mixamo"

[clips."mixamo.com"]
loop = true
in_place = true

[checks.loop-seam]
max_ratio = 1.6
```

This is the committed
[`examples/mixamo.animsmith.toml`](../examples/mixamo.animsmith.toml)
— it pins the profile rather than trusting auto-detection, declares
the clip a loop (arming `loop-seam`) and in-place (arming `in-place`,
judged on the Hips track per the callout above). Every key, glob
pattern, and severity override is documented in the
[configuration reference](../README.md#configuration). For a
root-motion download, swap the in-place declaration for a speed pin,
using the `speed_mps` that `measure` reported as the starting value:

```toml
[clips."mixamo.com"]
loop = true
in_place = false
speed_mps = { value = 1.2, tolerance = 0.15 }
```

Lint against the contract:

```console
$ animsmith lint --config examples/mixamo.animsmith.toml walking.glb
walking.glb: clean
0 error(s), 0 warning(s), 0 note(s)          # exits 0
```

And this is what a violation looks like — the same contract against a
copy whose cycle was cut short, the classic
[popped loop](game-ready-clips.md#the-loop-pops):

```console
$ animsmith lint --config examples/mixamo.animsmith.toml walking-popped.glb
walking-popped.glb:
  error[loop-seam] clip 'mixamo.com' @1.000s: loop seam pops: wrap discontinuity
    is 6.82× the neighbouring in-clip step (cap 1.60) — the clip does not
    close its cycle (measured 6.8152, expected 1.6000)
1 error(s), 0 warning(s), 0 note(s)          # exits 1
```

From here the contract grows with your project: rename clips as you
import more animations, add `[clips."run_*"]` globs, speed pins, and
`gait_groups` blend rings as the locomotion set fills out — the
cookbook's
[project contract section](../examples/README.md#4-a-project-contract-config)
and the worked
[`character.animsmith.toml`](../examples/character.animsmith.toml)
show that full shape. Commit `animsmith.toml` next to your assets and
every bare `animsmith lint` — local or
[in CI](../examples/README.md#1-a-first-cli-gate) — enforces it.

## Where to go next

The [game-ready clips guide](game-ready-clips.md) explains every
failure mode these checks catch; everything else — the cookbook, the
CLI reference, embedding — is routed from the
[documentation index](README.md).
