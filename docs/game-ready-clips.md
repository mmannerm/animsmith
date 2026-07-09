# Game-ready animation clips

What animsmith checks for, and why exported clips fail at runtime.

A skeletal animation clip can pass every format validator and still
break the game it ships in: the loop pops, the character glides, feet
skate through blends, a limb stays T-posed. Those are not file-format
errors — the file is spec-conformant — they are *content* problems that
only surface after the slowest step of the pipeline: engine import, a
bake, a playtest.

This document describes the characteristics that make a clip
game-engine friendly, organized by the runtime symptom you see when a
characteristic is violated. Each section names the symptom, explains
the mechanics behind it, and maps it to the animsmith checks, repairs,
and configuration that address it. If you want runnable commands, each
symptom links into the [examples cookbook](../examples/README.md); if
you want the reasoning behind the tool itself — why it exists and what
it is worth to your team — see [why animsmith](why-animsmith.md).

## A valid file is not a usable clip

Format validators — Khronos glTF-Validator being the canonical one —
check *spec conformance*: accessor validity, buffer bounds, quaternion
norms at the container level. They have no concept of a loop, a gait,
or root motion. A clip whose walk cycle was cut a quarter-stride short
is perfectly valid glTF; it will also visibly pop every second of
gameplay.

The characteristics below fall into two groups, and animsmith treats
them differently:

- **Mechanical characteristics** hold for every clip, with no knowledge
  of your project: finite values, monotonic key times, unit
  quaternions, consistent durations. animsmith checks these out of the
  box, and repairs the losslessly-repairable ones.
- **Semantic characteristics** are contracts only you can declare: *this*
  clip loops, *this* one is authored in place, *these four* form a
  blend ring. animsmith checks these against a
  [project config](../examples/README.md#4-a-project-contract-config)
  plus a resolved rig profile — and skips them with a note, rather than
  guessing, when either is missing.

Two loops benefit. The **artist inner loop** — `animsmith lint
export.fbx` seconds after a DCC export catches "the loop pops" or
"wrong rig" while the DCC session is still open, instead of after
import and bake. And the **CI gate** — the same checks with
machine-readable output and stable exit codes hold every committed
asset to the contract, so a re-export can't silently drift.

---

## The pose flickers, spins, or explodes

Rotation in a clip is stored as quaternions, and engines are strict
about the math even when exporters are not.

- **A non-finite value anywhere poisons everything.** A single NaN or
  Inf in a key time or value poisons interpolation and, in most
  engines, the whole pose — one bad float turns a character into
  visual noise. The `nan` check treats this as an error, always;
  there is no safe automatic repair for a value that carries no
  information.
- **Non-unit quaternions skew skinning.** Rotation keys must be unit
  quaternions. Engines renormalize inconsistently (or not at all); a
  non-unit key skews blend weights and skinning. The `quat-norm` check
  catches it, and `animsmith fix` repairs it losslessly — scaling a
  finite, non-zero quaternion back to unit length preserves the
  rotation it represents.
- **Hemisphere flips spin the long way around.** A quaternion and its
  negation represent the same rotation, but interpolation between them
  does not: adjacent keys on opposite hemispheres (`dot < 0`) make
  engines that slerp without neighborhood correction take the long way
  — a visible 360°-minus-θ spin between two keys. The `quat-flip`
  check catches it; `animsmith fix` repairs it losslessly by negating
  keys until each track is hemisphere-consistent.
- **Key times must move forward.** glTF requires strictly increasing
  key times, and engines misbehave without them. A first key that
  starts late is its own hazard: the engine clamp-holds an unauthored
  pose for the gap. The `time-monotonic` check covers both.

Workflow: [a first CLI gate](../examples/README.md#1-a-first-cli-gate)
detects these; [repairing an
asset](../examples/README.md#2-repairing-an-asset) walks the
`fix --dry-run` → `fix` → verify loop. The repairs are byte-surgical:
meshes, skins, materials, and textures pass through byte-identical.

## The clip is the wrong length or freezes at the end

- **Channels that end at different times mean a partial export.** When
  one bone's track is shorter than the clip, the engine clamp-holds the
  shorter channel — a limb freezes while the rest of the body keeps
  moving. The `duration-sanity` check flags degenerate durations and
  mismatched channel ends.
- **Keys off the frame grid mean a retiming step drifted.** A clip with
  a declared frame rate should keep its keys on that rate's time grid
  and span a whole number of frames. Off-grid keys mean a resample or
  retiming step drifted; a fractional frame count means a slice cut
  mid-frame — and engines care: Unreal, for example, documents that
  animations with non-whole end frames do not import correctly. The
  `fps` check verifies both once the config declares a rate.

When the wrong length is the *input* problem — a capture with garbage
at the head, a one-shot that should hold its final pose — the
`transform` command does the mechanical edit:
`--slice` cuts a window on the frame grid and retimes it to start at
zero, and `--hold-extend` appends a linear hold of the final pose
(charge and block poses). See [editing a
clip](../examples/README.md#3-editing-a-clip).

## The loop pops

A looping clip must end where it began — not just near it. At the wrap
point the runtime jumps from the last frame back to the first, and any
residual offset in the pose becomes a visible hitch, once per cycle,
forever.

The `loop-seam` check measures the wrap discontinuity of the feet
relative to the hips, normalized by the seam-adjacent in-clip steps: a
clean cyclic clip wraps by about one locally-normal step (ratio ≈ 1),
while a clip whose cut dropped the loop closure pops well above that.
It is judged only on clips declared `loop = true` in the config —
whether a clip is *supposed* to loop is a fact about your project, not
the file — though the measured ratio is always available via
`animsmith measure`.

When the seam is broken because the cycle is badly anchored rather
than badly cut, `transform --gait-anchor` rotates the clip in time so the
measured stride anchor lands at t=0, picking the candidate frame with
the lowest seam ratio.

Workflow: [a project contract
config](../examples/README.md#4-a-project-contract-config) shows the
same walk cycle passing clean and failing with a popped seam — and why
an undeclared loop is reported clean.

## The character glides or runs in place

Locomotion clips carry a travel contract between the asset and the
runtime, and nothing inside the file can verify it alone.

- **In-place vs root motion.** An in-place (treadmill) clip expects the
  gameplay code to drive entity velocity; a root-motion clip bakes the
  travel in. A clip that violates its declared mode makes the character
  glide or run in place at runtime. The `in-place` check compares the
  declaration against measured root motion.
- **Declared speed drift.** Runtimes scale playback by a clip's
  declared locomotion speed to keep foot plants locked to world
  velocity; a stale speed pin plays the clip visibly too fast or too
  slow. The `root-motion-speed` check compares the declared `speed_mps`
  against the measured horizontal root displacement. Use
  `animsmith measure` to obtain the ground-truth number before pinning
  it.

Both checks need a resolvable root: they use the rig profile's root
role, falling back to the hips role when no dedicated root bone
exists. That fallback matters in practice — the built-in `mixamo`
profile resolves `mixamorig:*` bone names but has no root role (Mixamo
rigs have no dedicated root bone), so root-motion checks on Mixamo
assets judge the hips track.

## Feet skate when clips blend

A directional locomotion set — run forward, back, left, right — is
blended at runtime, and blending is only seamless when every member
strides in phase. If one cycle's left foot plants at t=0 and another's
at mid-cycle, every blend between them skates the feet.

The `gait-group` check holds a declared blend ring to a shared gait
phase (the stride anchor measured from the left−right foot-height
fundamental). Members with too little left/right alternation are
excluded from the spread — their phase is noise — and a member whose
gait cannot be measured at all is an error, so the group's coherence is
never silently unverified. Declare the ring in config:

```toml
[gait_groups.run-ring]
clips = ["run_forward", "run_backward", "run_left", "run_right"]
max_gait_phase_spread = 0.15
```

`transform --gait-anchor` is the matching repair-by-transform: it
rotates a cyclic clip so its stride anchor lands at t=0, aligning the
set member by member.

## Feet slide within one clip

During stance — the part of the stride where a foot is planted — the
foot must move consistently with the clip's declared travel: at
`speed_mps` relative to the character for an in-place clip, or planted
in the world for a root-motion clip. Deviation is the skate that
runtime IK and blend band-aids exist to hide.

The `foot-slide` check measures stance-phase foot velocity against the
declaration. It is the research-grade check of the catalog: contact
detection is heuristic, so it ships as a warning with generous
defaults, and is judged only on clips that declare `speed_mps`.

## A limb is T-posed, or a bone never moves

Three related rig-integrity failures, in increasing subtlety:

- **A declared bone is missing entirely.** Bones the clip is declared
  to animate (via `animates_bones` in the config) must exist in the
  skeleton and carry at least one keyframed track. The `missing-bones`
  check catches slices that accidentally dropped a channel — leaving a
  limb static — and exports against the wrong rig.
- **A bone has keys but never moves.** A required bone whose rotation
  never exceeds a floor is frozen: a T-posed limb, a wrong-source
  slice, or a masked-out channel that a presence-only check would
  pass. Real motion moves required bones tens of degrees; the
  `frozen-bone` check's default 1° floor catches truly static bones
  without flagging subtle idle sway.
- **The clip was authored against a different bind.** A clip whose
  first frame deviates wildly from the skeleton's rest pose was almost
  certainly authored against a different bind — wrong seed rig, wrong
  export skeleton — and will deform incorrectly when retargeted onto
  this one. Small deviations are normal (few clips start exactly at
  rest); the `bind-pose` check fires only on a large mean deviation
  across the animated bones.

## The file is bloated, or the retargeter chokes

Export hygiene problems rarely break playback outright, which is why
they accumulate:

- **Constant tracks are export bloat.** A multi-key track whose values
  never move comes from unbaked rig channels or "key everything"
  exports. Harmless at runtime, wasteful on disk and in every blend
  the runtime evaluates — the `constant-track` check reports it as a
  note.
- **Scale keys are usually an accident.** Scale animation on a skeletal
  clip is typically an export artifact (a stray keyframe, a
  unit-conversion bake), and many engine rigs ignore or mishandle it —
  so its presence is a warning. Non-uniform scale is worse: most
  runtimes and retargeters actively break on it, and the `scale-keys`
  check calls it out separately.

---

## From symptom to command

| Symptom | Check(s) | Repair / transform | Config surface | Workflow |
|---|---|---|---|---|
| Pose flickers, spins, or explodes | `nan`, `quat-norm`, `quat-flip`, `time-monotonic` | `fix` (quat repairs, lossless) | — | [First gate](../examples/README.md#1-a-first-cli-gate), [Repair](../examples/README.md#2-repairing-an-asset) |
| Wrong length, freezes at the end | `duration-sanity`, `fps` | `transform --slice`, `--hold-extend` | `[clips.<name>] fps` | [Editing a clip](../examples/README.md#3-editing-a-clip) |
| The loop pops | `loop-seam` | `transform --gait-anchor` | `[clips.<name>] loop = true` | [Contract config](../examples/README.md#4-a-project-contract-config) |
| Glides or runs in place | `in-place`, `root-motion-speed` | re-export; `measure` for ground truth | `[clips.<name>] in_place`, `speed_mps` | [Contract config](../examples/README.md#4-a-project-contract-config) |
| Feet skate across blends | `gait-group` | `transform --gait-anchor` | `[gait_groups.<name>]` | [Contract config](../examples/README.md#4-a-project-contract-config) |
| Feet slide within a clip | `foot-slide` | re-author in DCC | `[clips.<name>] speed_mps` | [Contract config](../examples/README.md#4-a-project-contract-config) |
| T-posed limb, static bone, wrong bind | `missing-bones`, `frozen-bone`, `bind-pose` | re-export | `[clips.<name>] animates_bones`, `[rig]` | [Contract config](../examples/README.md#4-a-project-contract-config) |
| Bloat, retargeter breakage | `constant-track`, `scale-keys` | re-export with baked/clean channels | `[checks.<id>]` severity | [First gate](../examples/README.md#1-a-first-cli-gate) |

Where the repair column says *re-export*, that is deliberate: animsmith
rewrites a clip only in ways whose correctness its own checks can
verify. Lossless quaternion repairs and mechanical edits (slice,
hold-extend, gait-anchor, FBX→glTF conversion) qualify; artistic
transformation — retargeting, motion editing — is DCC work and stays
out of scope.

The gait and root-motion checks (`loop-seam`, `in-place`,
`root-motion-speed`, `gait-group`, `foot-slide`) additionally need a
resolved rig profile so they know which bones are the hips, feet, and
root. Built-in profiles cover `mixamo`, `ue-mannequin`, and `humanoid`
rigs; `[rig] profile = "auto"` scores them against your skeleton, and
`[rig.roles]` binds bone names explicitly for everything else. See the
[configuration reference](../README.md#configuration) for every key.

## Why animsmith exists

The positioning case — what animsmith is, why nothing else fills this
role, and what it is worth to each role on a team — lives in
[why animsmith](why-animsmith.md). The short version: nothing
open-source does game-semantics clip validation, format validators
stop at spec conformance, engine importers surface content problems
late with checks re-derived studio by studio, and animsmith packages
the missing checks as a standalone, engine-agnostic, CI-friendly tool.

Where to go next:

- [Pipeline scenario guide](pipeline-scenarios.md) — where these checks
  fit in raw-to-game-ready asset processes.
- [Examples cookbook](../examples/README.md) — runnable workflows for
  every symptom above.
- [CLI reference](cli.md) — every command, flag, and exit code.
- [Embedding guide](embedding.md) — drive the check catalog from Rust
  in your own pipeline.
- [DESIGN.md](../DESIGN.md) — architecture, the full check-catalog
  rationale, and the roadmap.
- [Game-ready animation research](research/game-ready-animation-clips.md)
  — the dated engine survey (Unity, Unreal, Godot, Bevy, glTF) behind
  this guide's contract, with the gaps that inform the roadmap.
