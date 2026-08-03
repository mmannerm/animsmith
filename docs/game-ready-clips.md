# Game-ready animation clips

What animsmith checks for, and why exported clips fail at runtime.

A skeletal animation clip can pass every format validator and still
break the game it ships in: the loop pops, the character glides, feet
skate through blends, a limb stays T-posed. Those are not file-format
errors — the file is spec-conformant — they are *content* problems that
only surface after the slowest step of the pipeline: engine import, a
bake, a playtest.

This document defines what "game-ready" means here — the
[readiness ladder](#the-readiness-ladder) below stages the evidence
from file-ready data to shipped acceptance — and then describes the
checkable characteristics, organized by the runtime symptom you see
when one is violated. Each symptom section explains the mechanics and
maps them to the animsmith checks, repairs, and configuration that
address them. If you want runnable commands, each symptom links into
the [examples cookbook](../examples/README.md); if you want the
reasoning behind the tool itself — why it exists and what it is worth
to your team — see [why animsmith](why-animsmith.md).

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
  quaternions, consistent durations.
- **Semantic characteristics** are contracts only you can declare:
  *this* clip loops, *this* one is authored in place, *these four*
  form a blend ring — declared in a
  [project config](../examples/README.md#4-a-project-contract-config)
  and resolved through a rig profile.

These two groups are the first two levels of the
[readiness ladder](#the-readiness-ladder), which states what animsmith
does about each.

Two loops benefit. The **artist inner loop** — `animsmith lint
export.fbx` seconds after a DCC export catches "the loop pops" or
"wrong rig" while the DCC session is still open, instead of after
import and bake. And the **CI gate** — the same checks with
machine-readable output and stable exit codes hold every committed
asset to the contract, so a re-export can't silently drift.

## The readiness ladder

"Game-ready" is not one property a tool can certify, because most of
it is relative to a consumer: *your* engine, *your* controllers,
*your* bar for quality. It is a ladder of evidence, and each level
has a different owner. animsmith's job is to make the early levels
checked and repeatable, make the declared middle measurable, and say
plainly what it did not evaluate — not to stamp the whole ladder.

1. **File-ready** — the data is parseable, finite, and mechanically
   valid: no NaN/Inf, monotonic key times, unit quaternions, sane
   durations, clean track hygiene. This is animsmith's primary
   generic coverage: the mechanical checks (`nan`, `time-monotonic`,
   `quat-norm`, `quat-flip`, `duration-sanity`, `scale-keys`,
   `non-uniform-scale`, `constant-track`) run on every file with no
   configuration. The narrower `constant-nonunit-scale` policy signal is
   registered but opt-in. `fix` repairs the two losslessly repairable defect classes
   (`quat-norm`, `quat-flip`).

2. **Clip-ready** — the clip honors what you declared about it: loop
   closure, duration and frame grid, in-place vs root-motion policy,
   required bone motion, structural rig presence, bind-pose consistency. Strong, config-backed
   coverage where a check exists: `fps`, `loop-seam`, `in-place`,
   `root-motion-speed`, `foot-slide`, `missing-bones`, `required-bones`, `frozen-bone`,
   and `bind-pose` judge exactly the expectations you declare — and
   the checks that need rig roles report a typed coverage gap instead
   of guessing when a role cannot be resolved. One member is heuristic:
   `foot-slide` ships as a warning (see
   [feet slide within one clip](#feet-slide-within-one-clip)).

3. **Set-ready** — clips that blend or sync together are compatible
   as a set. Generic measurement and checking where implemented:
   `gait-group` holds a declared directional blend ring to a shared
   stride phase, `measure` supplies the per-clip numbers, and
   `animsmith diff` catches drift between revisions. Set
   compatibility beyond the implemented checks is yours to review.

4. **Rig and use prerequisites** — which bones play which roles on
   the target rig, which bones must exist, which bones a clip must animate, and what each
   clip is for. A shared boundary: you supply the meaning (a rig
   profile or `[rig.roles]`, `[rig] required_bones`, `animates_bones`, per-clip
   expectations), and animsmith resolves roles against the skeleton,
   checks the declarations, and reports the resolved roles it used.
   Nothing at this level can be inferred from the file alone.

5. **Runtime integration** — importer behavior, blend-graph
   topology, animation target IDs, masks, sync and reset behavior,
   and the poses your engine actually evaluates. Consumer-owned:
   animsmith ships no runtime-integration checks, and its
   measurements come from its own documented sampling model — a
   model of engine samplers, not a reproduction of yours.

6. **Gameplay, artistic, and production acceptance** — controller
   feel and timing, readability, visual quality, provenance,
   reproducibility, shipping sign-off. Consumer-owned: reports and
   measurements inform the review; people make the call.

A clean run is evidence, and evidence has scope: it covers the checks
that ran, on the file that ran, against the contract you declared.
Only an actual animsmith run on the actual file establishes that
evidence — nothing transfers from vendor previews, other files in the
pack, or another export's report. And where generic validation touches
a later level, it supplies prerequisites or evidence for that level,
never blanket certification of it: a mechanically pristine,
contract-clean clip can still be rejected by your importer, your blend
graph, or your art director.

### Reading a lint run

One `animsmith lint` run answers five independent questions. Keep
them separate when you automate on the output:

- **Was the check active?** The full catalog is selected by default;
  `--select` narrows the selected set, and `[checks.<id>] severity = "off"`
  disables a check. A built-in opt-in check stays disabled until its severity
  is set to "note", "warn", or "error". Final JSON still records inactive
  checks without executing them.
- **Did it apply here?** Contract-aware checks judge only declared
  expectations. With no `loop = true` clip in the config, `loop-seam`
  has nothing to judge and is recorded as `not_applicable`.
- **Was the work evaluated?** When declared work exists but a prerequisite
  or measurement is missing, the check reports a typed coverage gap. A check
  can also complete part of its work: `gait-group` still validates declared
  ring members when unresolved roles keep it from measuring phase, then
  reports member existence as completed and phase coherence as a gap.
- **What did the evaluated work find?** Content findings at note,
  warning, or error severity, carrying clip, bone, time, and
  measured-vs-expected context.
- **What blocks?** Gate policy is yours, not animsmith's verdict:
  exit `1` on error findings, `--deny-warnings` to promote warnings,
  per-check severity overrides, and presentation-only `--allow` in text or
  Markdown. Coverage gaps never fail a run — exit `0` means no failing
  findings among the work that was evaluated, not that everything
  was evaluated. A gate that requires full coverage must inspect gaps too.

There is deliberately no single "pass" state: a run can complete with
warnings, and it can evaluate some declared work while skipping the
rest. See [machine-readable output](output.md) for the final v3
representation. It models selection, configuration, applicability, and
evaluation independently, keeps content findings separate from typed gaps,
and records completed work scopes. This is evidence about animsmith's checks,
not runtime certification; stricter completeness policy belongs to the
consuming pipeline.

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
- **A valid-looking clip can still be one frame too short.** Export slicing,
  inclusive-vs-exclusive frame ranges, and endpoint removal can produce a
  positive duration whose channels agree but which no longer matches the
  gameplay or animation manifest. Declare
  `duration_s = { value = 1.033, tolerance = 0.02 }` for the clip and
  `duration-sanity` reports the measured and expected seconds when that pin
  is missed. The value must be finite and positive, and the tolerance finite
  and non-negative; invalid pins are errors instead of silently disabling the
  contract. The tolerance absorbs harmless exporter rounding. Linting only
  reports the mismatch; it does not repair or resample the clip. Re-export
  when the authored range is wrong, or use the explicit slice/hold transforms
  below when the source is intentionally being edited.
- **Keys off the frame grid mean a retiming step drifted.** A clip with
  a declared frame rate should keep its keys on that rate's time grid
  and span a whole number of frames. Off-grid keys mean a resample or
  retiming step drifted; a fractional frame count means a slice cut
  mid-frame — and engines care: Unreal, for example, documents that
  [animations with non-whole end frames do not import
  correctly](https://dev.epicgames.com/documentation/en-us/unreal-engine/animation-sequences-in-unreal-engine). The
  `fps` check verifies both once the config declares a rate.

When the wrong length is the *input* problem — a capture with garbage
at the head, a one-shot that should hold its final pose — the
`transform` command does the mechanical edit:
`--slice` cuts a window on the frame grid and retimes it to start at
zero, and `--hold-extend` appends a linear hold of the final pose
(charge and block poses). See [editing a
clip](../examples/README.md#3-editing-a-clip).

## The loop pops

A looping clip needs both pose and motion continuity at the wrap. A pose
offset is a C0 discontinuity: the runtime jumps when it returns to frame 0. A
matching pose with mismatched incoming and outgoing velocity is a C1
discontinuity: it reaches the right point but changes direction or speed
abruptly, producing a hitch or pulse once per cycle. Unity's
[looping-clip guide](https://docs.unity3d.com/Manual/LoopingAnimationClips.html)
shows the same artist-facing start/end match problem and the special treatment
root-motion axes may need.

Animsmith separates four questions:

- `loop-closure` finds the largest last-to-first **model-space position** and
  shortest-path **model-space rotation** delta across all skeleton bones. The
  default caps are 0.01 m and 1 degree.
- `loop-seam-vel` finds the largest difference between the model-space linear
  velocity entering the last sample and leaving frame 0. Its default cap is
  0.1 m/s. A closed triangle-wave trajectory demonstrates the problem: the
  first and last position are identical, but the bone reverses direction at
  the wrap.
- `loop-seam-rot` finds the largest difference between the **shortest-path
  model-space angular velocities** entering the last sample and leaving frame
  0. Its default cap is 5 deg/s. This is rotational C1 continuity: the pose
  can close rotationally (C0) yet still snap in direction or turn rate at the
  wrap when the two seam-adjacent rotation steps disagree.
- `loop-seam` remains the locomotion-specific test. It compares feet relative
  to hips and normalizes by the neighbouring stride step, so it needs resolved
  hips/foot roles and deliberately has no result for a stationary clip.

There is a fourth, narrower warning for an export-mode problem rather than a
pose-continuity judgment: `duplicate-loop-endpoint`. Many DCC workflows export
an inclusive range or bake a cycle by copying frame 0 again at the final frame.
For a declared loop, animsmith recognizes only the mechanically certain subset:
every authored channel has one common finite, strictly increasing timeline and
valid cardinality; first and final vector components match within `1e-5` and
quaternions match within a sign-invariant `1e-4` radians; and the clip has real
interior motion. A
stationary hold is therefore not mistaken for a cycle. The warning is
default-on, needs no rig roles, and is otherwise not applicable unless
`loop = true` is declared.

That duplicate can matter to an engine that loops by advancing over the clip
duration: it evaluates or holds the same pose twice before wrapping, which can
look like a one-frame hitch. `transform --drop-duplicate-loop-endpoint` turns
only an eligible candidate into an open cycle: it atomically removes the same
complete terminal key count from every channel (and cubic-spline key triplets),
preserves retained authored data, and re-pins duration. It does not repair a
nonclosing/root-travel clip, mismatched timelines, a C1 tangent problem,
retargeting damage, or a runtime loop blend. Blender's [Cycles F-curve
modifier](https://docs.blender.org/manual/en/latest/editors/graph_editor/fcurves/modifiers.html#cycles-modifier)
and Unity's [looping-clip guide](https://docs.unity3d.com/Manual/LoopingAnimationClips.html)
are useful places to inspect the authored range and engine import behavior.

An open cycle intentionally does not keep `loop-closure` green: that existing
inclusive check compares a repeated final sample with frame 0. It is not a
general endpoint-mode classifier; [#22](https://github.com/mmannerm/animsmith/issues/22)
continues to own the complete endpoint-mode measurement and sync-group policy.

The first three checks are per-bone and role independent, so idle, guard,
block, aim-offset, facial, and prop loops remain testable without a humanoid
profile or detectable stride. Model-space is intentional: a parent mismatch
can move or rotate many descendants even when their local keys match. JSON
measurements retain a stable `bone_index` plus display `bone_name` for every
row, while findings name only the maximum offending bone for each judged
dimension.

Typical causes are an export range ending one frame early, a cycle modifier or
procedural controller not being baked, copied endpoint keys whose Bezier
tangents do not match, retargeting or resampling that changes one endpoint, and
an engine importer using a different clip range from the DCC. A useful
diagnostic split is:

- nonzero position/rotation delta: repair the endpoint pose or clip range;
- zero pose delta but high velocity delta: repair the endpoint tangents or the
  seam-adjacent keys;
- closed rotational pose but high angular-velocity delta: repair the endpoint
  rotation tangents or the seam-adjacent rotation keys using the intended
  shortest turn;
- many descendants reporting the same displacement: inspect their first
  mismatching ancestor;
- only `loop-seam` failing: inspect locomotion phase and foot/hips-relative
  motion rather than a stationary pose loop.

The normal fix is in the DCC: select the intended complete cycle, bake
procedural motion, make the last pose match the first where appropriate, match
the derivative on both sides, then re-export and rerun `animsmith measure` and
`animsmith lint`.
Blender's [Cycles F-curve
modifier](https://docs.blender.org/manual/en/latest/editors/graph_editor/fcurves/modifiers.html#cycles-modifier)
can preview repeated curves, but the evaluated endpoint and tangents still
need to survive baking and export. Engine-side loop-pose blending can hide a
small mismatch at runtime; it does not make the source measurement close.

Root-motion locomotion is the important exception. Intentional horizontal root
travel does not return to its starting model-space position, and every child
inherits that travel. Tune the global `max_position_delta_m`, or the clip-level
`max_loop_position_delta_m`, to the contract; alternatively disable
`loop-closure` for such a pipeline. Keep using `loop-seam` for feet-relative
locomotion. `loop-seam-vel` and `loop-seam-rot` can still validate constant
inherited linear and angular motion respectively.

When one project contains both stationary idles and root-motion locomotion,
keep the global `[checks.loop-closure]`, `[checks.loop-seam-vel]`, and
`[checks.loop-seam-rot]` caps strict, then put a finite, non-negative
`max_loop_position_delta_m`, `max_loop_rotation_delta_deg`,
`max_loop_velocity_delta_mps`, or
`max_loop_angular_velocity_delta_degps` on the relevant `[clips."run_*"]`
family. A `[clips.run_forward]` entry wins over matching globs for only the
fields it supplies, so a one-off authored clip need not copy every inherited
value. This is a contract choice, not a repair: it does not make a
discontinuous source clip smooth.

There is no general automatic repair. `transform --gait-anchor` can rotate a
locomotion cycle in time to choose a better stride cut, but it does not rewrite
arbitrary bone endpoint poses or tangents. Angular-velocity C1 continuity,
acceleration/jerk continuity, root-motion extraction policy, and runtime blend
settings remain out of scope.

All four checks are judged only on clips declared `loop = true` — whether a
clip is intended to loop is project knowledge — while raw measurements are
always available for measurable clips through `animsmith measure`. Workflow:
[a project contract config](../examples/README.md#4-a-project-contract-config)
shows the same walk cycle passing clean and failing with a popped seam, and why
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

Four related rig-integrity failures, in increasing subtlety:

- **A structural rig bone is absent or ambiguous.** Runtime sockets, IK
  targets, mask bones, and attachment points can be intentionally static, so
  they do not belong in a per-clip motion rule. Put their exact names in
  `[rig] required_bones = ["weapon_socket", "ik_hand_l"]`.
  `required-bones` passes a present static bone, errors for a missing name,
  and refuses to guess if duplicate skeleton names make the declaration
  ambiguous. It also reports an empty or absent skeleton as unable to meet a
  nonempty structural contract. This check does not create bones, rename an
  export, retarget a rig, or validate engine-side socket use: repair the source
  rig in the DCC and re-export.

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

### Why scale animation deserves its own review

A transform scale is a three-component value `(x, y, z)`. A value of
`(1, 1, 1)` preserves the authored size, a uniform value such as `(2, 2, 2)`
doubles every axis, and a non-uniform value such as `(1, 2, 1)` stretches one
axis. The [glTF animation model](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html#animations)
allows a node's scale to be keyframed with STEP, LINEAR, or CUBICSPLINE
interpolation. Blender's
[keyframe guide](https://docs.blender.org/manual/en/latest/animation/keyframes/introduction.html)
explains the artist-facing version of the same idea: keys store property values
and interpolation curves determine every value between them.

Scale animation is not automatically invalid. It may be an intentional
squash-and-stretch effect, visibility technique, or gameplay deformation.
Unreal, for example, explicitly
[supports non-uniform scale animation](https://dev.epicgames.com/documentation/en-us/unreal-engine/non-uniform-scale-animation?application_version=4.27)
and stores scale only for animations that need it. The warning exists because
unintentional scale curves are also a common export artifact and because
runtime consequences are project- and engine-dependent. Unity documents that
[non-uniform parent scale](https://docs.unity3d.com/6000.1/Documentation/Manual/class-Transform.html#non-uniform-scaling)
can skew rotated children and disagree with some collider shapes.

animsmith separates four facts so a team can make that policy decision without
conflating them:

| Check | Literal fact | Typical source | Why review it |
|---|---|---|---|
| `scale-keys` | At least one scale component changes over time after interpolation. | Intentional squash/stretch; constraint or retarget bake; unit-conversion keys; exporter-created curves. | It can change proportions, child placement, blending, physics assumptions, and animation storage. Confirm the motion is intentional in the target engine. |
| `non-uniform-scale` | X, Y, and Z differ somewhere on the evaluated trajectory. | Stretching one bone axis; unapplied object scale; cubic interpolation overshoot between apparently harmless keys. | Parent/child hierarchies, normals, colliders, and engine components may treat non-uniform scale differently from uniform scale. |
| `constant-nonunit-scale` | A scale channel or single-key pin stays away from `(1, 1, 1)`. Disabled by default. | Unit conversion; a deliberately resized character; an unapplied static transform that survived into the rig. | Often harmless, sometimes a pipeline-policy violation. Enable it only when the project expects unit scale in animation channels. |
| `constant-track` | A multi-key track stores repeated values and never changes. | "Key everything" export, baked controls, or importer-generated constant curves. | It is redundant data even when its value is valid. Unity exposes a corresponding importer option to [remove constant scale curves](https://docs.unity3d.com/ScriptReference/ModelImporter-removeConstantScaleCurves.html). |

Examples:

- Keys `(1,1,1) → (1.1,1.1,1.1) → (1,1,1)` trigger `scale-keys` but not
  `non-uniform-scale`: the character grows uniformly and returns.
- A constant `(1,1.2,1)` channel triggers `non-uniform-scale`, and triggers
  `constant-nonunit-scale` only when that opt-in check is enabled. Multiple
  repeated keys also trigger `constant-track`.
- Dense `(1,1,1)` keys trigger `constant-track`, not `scale-keys`; there is no
  temporal scale motion.
- Equal CUBICSPLINE key values can still trigger `scale-keys` when their
  tangents move the curve between keys. Inspect the curve, not only the key
  diamonds.

To opt into a unit-scale policy:

```toml
[checks.constant-nonunit-scale]
severity = "note" # or "warn" / "error" for your project
```

### Fix the source, then verify the exported result

For an unintentional finding, inspect scale channels in the DCC's Graph Editor,
identify whether the curve belongs to a deform bone, control, helper, or object,
and remove or rebake only the unwanted channel. Check exporter options that key
all transforms or resample the FBX transform stack. Blender's
[Apply transforms](https://docs.blender.org/manual/en/latest/scene_layout/object/editing/apply.html)
can move object-level scale into object data before rigging, but its manual
explicitly warns that applying an armature object transform does not rewrite
pose animation curves or constraints. Do not treat `Ctrl-A` as a universal fix
for an already animated rig.

Re-export, rerun `animsmith lint`, and preview the result in the target engine.
The desired end state depends on intent:

- deliberate scale motion remains and is accepted by project policy;
- unnecessary dense keys are removed while the evaluated pose stays the same;
- accidental scale motion or non-uniformity is removed at its authoring source;
- a constant non-unit pin remains only when the rig/import contract requires it.

animsmith does not delete scale tracks, flatten skeletal scale into mesh
geometry, retarget the clip, rewrite cubic tangents, or decide whether an
effect is artistically correct. Those operations can change deformation and
must stay in the DCC or an engine-aware retarget/import pipeline. The checks
turn the exported facts into a reviewable work order; they are not an automatic
scale repair.

---

## From symptom to command

| Symptom | Check(s) | Repair / transform | Config surface | Workflow |
|---|---|---|---|---|
| Pose flickers, spins, or explodes | `nan`, `quat-norm`, `quat-flip`, `time-monotonic` | `fix` (quat repairs, lossless) | — | [First gate](../examples/README.md#1-a-first-cli-gate), [Repair](../examples/README.md#2-repairing-an-asset) |
| Wrong length, freezes at the end | `duration-sanity`, `fps` | `transform --slice`, `--hold-extend` | `[clips.<name>] duration_s`, `fps` | [Editing a clip](../examples/README.md#3-editing-a-clip) |
| The loop pops or pulses at the wrap | `duplicate-loop-endpoint`, `loop-closure`, `loop-seam-vel`, `loop-seam-rot`, `loop-seam` | drop a strict duplicated endpoint with `transform --drop-duplicate-loop-endpoint`; otherwise re-author endpoint pose/tangents; `transform --gait-anchor` only for locomotion phase | `[clips.<name>] loop = true`, `[checks.loop-closure]`, `[checks.loop-seam-vel]`, `[checks.loop-seam-rot]` | [Contract config](../examples/README.md#4-a-project-contract-config) |
| Glides or runs in place | `in-place`, `root-motion-speed` | re-export; `measure` for ground truth | `[clips.<name>] in_place`, `speed_mps` | [Contract config](../examples/README.md#4-a-project-contract-config) |
| Feet skate across blends | `gait-group` | `transform --gait-anchor` | `[gait_groups.<name>]` | [Contract config](../examples/README.md#4-a-project-contract-config) |
| Feet slide within a clip | `foot-slide` | re-author in DCC | `[clips.<name>] speed_mps` | [Contract config](../examples/README.md#4-a-project-contract-config) |
| Missing runtime socket or IK target | `required-bones` | repair source rig / re-export | `[rig] required_bones` | [Structural rig contract](../examples/README.md#keeping-the-exported-rig-shape-stable) |
| T-posed limb, static bone, wrong bind | `missing-bones`, `frozen-bone`, `bind-pose` | re-export | `[clips.<name>] animates_bones`, `[rig]` | [Contract config](../examples/README.md#4-a-project-contract-config) |
| Bloat, retargeter breakage | `constant-track`, `scale-keys`, `non-uniform-scale`, opt-in `constant-nonunit-scale` | re-export with baked/clean channels | `[checks.<id>]` severity | [First gate](../examples/README.md#1-a-first-cli-gate) |

Where the repair column says *re-export*, that is deliberate: animsmith
rewrites a clip only in ways whose correctness its own checks can
verify. Lossless quaternion repairs and mechanical edits (slice,
hold-extend, gait-anchor, duplicate-loop-endpoint removal, FBX→glTF conversion) qualify; artistic
transformation — retargeting, motion editing — is DCC work and stays
out of scope.

The gait and root-motion checks (`loop-seam`, `in-place`,
`root-motion-speed`, `gait-group`, `foot-slide`) additionally need a
resolved rig profile so they know which bones are the hips, feet, and
root. `loop-closure`, `loop-seam-vel`, and `loop-seam-rot` do not. Built-in profiles cover
`mixamo`, `ue-mannequin`, and `humanoid`
rigs; `[rig] profile = "auto"` scores them against your skeleton, and
`[rig.roles]` binds bone names explicitly for everything else. See the
[configuration reference](../README.md#configuration) for every key.

## Why animsmith exists

The positioning case — what animsmith is, why nothing else fills this
role, and what it is worth to each role on a team — lives in
[why animsmith](why-animsmith.md).

Everything else — runnable workflows for the symptoms above, pipeline
scenarios, the CLI reference, embedding, and the dated engine survey
behind this guide's contract — is routed from the
[documentation index](README.md).
