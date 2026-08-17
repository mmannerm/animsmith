# Engine, blending, and compatibility checks

Use these checks only for game-engine suitability. Adapt them to the named
engine, version, character, controller, platform, and game type.

## Contents

- [Evidence levels](#evidence-levels)
- [Target-engine import](#target-engine-import)
- [Playback and root motion](#playback-and-root-motion)
- [Blending and transitions](#blending-and-transitions)
- [Upper and lower body masking](#upper-and-lower-body-masking)
- [Additive animation, IK, and attachments](#additive-animation-ik-and-attachments)
- [Rig and retargeting](#rig-and-retargeting)
- [Compatibility matrix](#compatibility-matrix)
- [Game-type caveats](#game-type-caveats)
- [Performance and production](#performance-and-production)

## Evidence levels

Keep these levels distinct in the report:

1. static file structure;
2. AnimSmith inspection, measurement, lint, or offline report;
3. successful engine import;
4. isolated engine playback;
5. blend graph, mask, retarget, and root-motion tests;
6. representative gameplay under the target controller and camera;
7. platform/performance and artistic acceptance.

Success at one level does not imply success at a later level. If an engine is
not installed or a licensed pack cannot be imported, report levels 3-7 as not
evaluated.

## Target-engine import

Use a disposable project and the exact target engine version. Preserve the
engine log and importer settings. Use official documentation for that version.

When the request is engine-general rather than scoped to one runtime, use at
least these rows and keep their results independent:

| Runtime | Minimum researched/prototyped surface |
|---|---|
| Unity | Model/Avatar import, clip segmentation and looping, root motion, Blend Trees and normalized phase, Animator layers/AvatarMask, compression |
| Unreal Engine | FBX animation import, Skeleton/IK Rig retarget path, root motion, Blend Spaces and Sync Groups/markers, layered blends, compression |
| Godot | glTF/scene animation import, Skeleton3D mapping, root motion, AnimationTree blend spaces/sync modes/filters, track reset behavior |
| Bevy | glTF animation loading, animation target identity, AnimationGraph blending/transitions, masked/additive capability actually exposed by the tested version, asset/runtime cost |

For every cell, record `documentation-only`, `prototype-observed`,
`not-evaluated`, or `deferred`. Never copy a setting or repair capability from
one engine into another engine's conclusion. A runtime may mask or compensate
for a source defect; distinguish that from repairing the source artifact.

Check:

- which delivered format imports and which route the vendor recommends;
- import warnings, dropped or renamed clips, take segmentation, duplicate
  assets, and embedded dependencies;
- unit scale, coordinate/forward axis, handedness conversion, scene/root
  transforms, bone scale, and visible character size;
- skeleton hierarchy, bone count and names, rest/reference pose, bone axes,
  leaf/end bones, root and motion-root layout, and mesh/skin binding;
- avatar/humanoid mapping or equivalent, retarget base pose, translation
  retarget policy, and unmapped required bones;
- clip duration, sample rate, interpolation, looping flag, first/last-frame
  convention, root lock, and root-motion extraction settings;
- animation curves, events/notifies, morph/facial tracks, finger tracks,
  additive metadata, and custom attributes promised by the pack;
- material/texture or demo-character dependencies only when relevant to judging
  the animation delivery;
- default compression, resampling, key reduction, and whether these change the
  visible or measured result.

Record an untouched-default import before trying vendor presets or corrective
settings. Then record the minimal settings-only path separately.

## Playback and root motion

Test representative and risk-selected clips at normal speed and slowed down.
Inspect from the gameplay camera and close enough to see contacts.

Check:

- pose stability, deformation, limb orientation, scale changes, and frozen or
  missing body parts;
- loop pose and velocity continuity;
- foot and hand contacts, sliding, penetration, ground height, and vertical
  root behavior;
- in-place versus authored root motion, displacement direction, yaw, speed,
  root-motion extraction, and controller double-application;
- starts, stops, pivots, landings, recovery, and transitions into and out of
  the pack's main loops;
- deterministic behavior under the intended playback-rate and compression
  settings.

For networked games, state who owns movement, how root motion is replicated or
reconciled, and whether the evaluation actually tested that path. Do not turn a
single-player playback result into a network-readiness claim.

Before recommending cyclic gait anchoring, inspect what the captured AnimSmith
version resamples. A time rotation that is appropriate for an in-place cycle
can reorder root motion that accumulates translation or yaw and break its
world-space trajectory. Recommend it for root-motion clips only when the
operation explicitly preserves/rebases the trajectory and an independent
before/after proof re-derives displacement and yaw. Otherwise retain the raw
root motion and use runtime phase offsets or artist-authored alignment.

For every current-version gait-anchor trial, record whether the operation
produced output or refused, the selected movement policy, and whether its root
heading basis was measurable for the source rig. A fail-closed refusal prevents
an unsafe rewrite but does not align the set. If an older evaluator produced an
output that the current evaluator refuses, keep the older measurement clearly
historical and base the current controller recommendation on the refusal.

## Blending and transitions

Test actual runtime blends; static pose similarity is insufficient.

For locomotion blend spaces or directional sets, check:

- common skeleton and retarget path;
- consistent root-motion policy, units, speed interpretation, orientation, and
  ground reference;
- compatible duration, sampling, endpoint convention, and sync markers or gait
  phase;
- left/right contact phase, stride length, stance width, vertical root motion,
  and turning behavior;
- interpolation at center, axes, diagonals, and rapid direction changes;
- time-reflected pairs or backwards playback assumptions;
- starts, stops, pivots, accelerations, and gaps the pack does not supply.

For every measured ring, report the minimum/maximum root-speed ratio and compare
forward, cardinal, and diagonal members. Equal duration plus unequal speed
means unequal travel per cycle. Do not label that automatically defective:
state whether the runtime preserves authored per-direction velocity, normalizes
input, scales playback/controller motion, or requires artist re-timing, and
name the likely foot-slide or diagonal-speed consequence of a mismatch.

For state transitions and crossfades, check pose and velocity discontinuity,
foot plant loss, hand/weapon discontinuity, body-height changes, and whether a
special authored transition is required. Record blend duration and sync mode.

Motion matching, pose search, and distance matching need appropriate metadata
and a sufficiently varied database. Treat general locomotion-loop quality as a
prerequisite, not proof that the pack is suitable for those systems.

Turn important blend evidence into an implementation recipe in the primary
report. Name every member, give 1D thresholds or 2D coordinates, record measured
cycle duration and root-motion speed, identify which members loop, state the
phase/sync policy, and say whether animation or the gameplay controller owns
translation and yaw. Keep in-place and root-motion variants in separate graphs
unless the runtime policy explicitly explains how they interoperate.

## Upper and lower body masking

Test masks with representative locomotion plus upper-body actions such as aim,
reload, attack, cast, carry, or interact.

Check:

- a stable hierarchy split and a suitable spine/pelvis boundary;
- whether upper-body clips key the root, pelvis, legs, IK targets, or scale and
  therefore override locomotion unexpectedly;
- whether lower-body clips animate spine/arms in ways that fight the overlay;
- shoulder, clavicle, spine, neck, and weapon continuity at the mask boundary;
- additive versus override semantics and the correct reference pose;
- hand placement, two-handed props, recoil propagation, and look/aim layers;
- mask behavior after retargeting to each target skeleton.

`constant-track` or track inventory can reveal unnecessary keyed regions, but
only an engine layer test establishes useful masking. Track pruning is not a
substitute for deciding which body motion belongs in a clip.

## Additive animation, IK, and attachments

Verify whether clips advertised as additive carry an explicit base/reference
pose and whether the target engine imports them with the intended additive
mode. Test zero weight, full weight, and composition with the expected base.

Check IK and contact assumptions:

- IK/helper bone presence, hierarchy, keyed state, and naming;
- hand and foot targets, pole vectors, twist bones, and target-engine rig
  mapping;
- weapon/prop sockets, grip orientation, two-hand alignment, and scale;
- whether contact correction is embedded, expected from runtime IK, or absent;
- whether stripping helper tracks or bones would break downstream rigs.

Missing semantic rig elements may be project-config or author issues. Do not
invent them during generic preprocessing.

## Rig and retargeting

Do not infer compatibility from `humanoid`, `Mixamo`, `UE mannequin`, shared
bone names, or equal bone counts alone.

Compare:

- hierarchy and parent relationships;
- rest/reference pose and joint orientations;
- bone axes, roll, local transforms, scale, and reflections;
- proportions and translation-bearing tracks;
- root, pelvis/hips, motion root, and scene-root semantics;
- twist, roll, IK, facial, finger, cloth, weapon, and attachment bones;
- animation target identity and duplicate/ambiguous names;
- retarget profile completeness and chain mapping;
- deformation on the actual target mesh, especially shoulders, hips, hands,
  feet, and extreme poses.

Run at least an idle, locomotion extreme, contact-heavy action, and wide-range
pose through the intended retargeter. Record engine configuration separately
from any artist-authored base-pose or chain correction.

AnimSmith assembly may remap exact unique bone names onto an authoritative
base in versions that expose that command. Treat this as deterministic
assembly, not semantic or deformation-aware retargeting.

## Compatibility matrix

Create separate matrices for within-pack sets and cross-pack pairs. Select
pairs by actual planned use; avoid an unreadable all-by-all table when packs
contain hundreds of clips.

Use these row/column dimensions:

| Dimension | Questions |
|---|---|
| Skeleton identity | Can clips target the same skeleton directly? Are names unique and hierarchy-equivalent? |
| Retargeting | Does the named engine retargeter map every required chain and preserve deformation? |
| Scale and axes | Do unit, forward/up axes, root orientation, and rest-world scale agree? |
| Root motion | Do both use compatible roots, displacement, yaw, ground, and controller policy? |
| Timing and sync | Do duration, rate, endpoint convention, contact phase, and sync markers align? |
| Blending | Do runtime crossfades and blend-space interpolation preserve pose, velocity, and contacts? |
| Masking/additive | Do key coverage, hierarchy split, and reference pose compose correctly? |
| Style | Do stance, energy, silhouette, cadence, and contact intent belong together artistically? |
| Gameplay semantics | Do naming and apparent motion represent the same action, direction, weapon, and state? |
| Pipeline | Can one reproducible conversion/config/retarget path process both packs? |

Assign one result per important pairing:

- `direct`: same verified target and successful runtime blend;
- `engine-config`: verified with engine mapping/settings only;
- `animsmith-current`: verified after a captured current AnimSmith operation;
- `artist-required`: retarget, cleanup, transition, or style work remains;
- `incompatible`: a stated required use failed and no acceptable remediation is
  available;
- `unknown`: no pairwise runtime evidence or insufficient source access.

Include evidence and caveats beside the result. A category is not a grade;
`engine-config` can be an excellent purchase outcome when the setup is stable
and documented.

## Game-type caveats

Discuss only relevant rows, but consider:

- first-person: arm rig, camera-relative motion, weapon alignment, recoil,
  hand contacts, and full-body shadow/body needs;
- close third-person: foot/hand contact visibility, turns, transitions, combat
  readability, slopes, and camera-facing artifacts;
- distant crowds or strategy: silhouette, loop variety, cost, LOD, and repeated
  cadence may matter more than finger/contact detail;
- networked action: authoritative movement, root-motion replication, rollback,
  deterministic timing, and transition interruption;
- motion matching: database breadth, pose/trajectory coverage, consistent
  contacts, and required metadata;
- melee/contact combat: weapon arcs, hit timing, root displacement, paired
  interactions, cancels, and recovery;
- traversal/platforming: takeoff, apex, landing, ledge/root alignment, and
  controller/environment coupling;
- stylized games: exaggeration and cadence compatibility may dominate
  anatomical realism;
- customizable characters: proportion range, retarget deformation, twist
  coverage, accessories, and attachment stability.

State both best-fit and poor-fit uses. Avoid penalizing a focused pack for not
covering unrelated genres.

## Performance and production

Measure or record, where available:

- file and imported asset size;
- clip, track, key, bone, mesh, and animation count;
- constant tracks, scale tracks, resampling, and compression behavior;
- runtime memory and evaluation cost on target hardware;
- naming consistency, versioning, source files, documentation, update history,
  vendor support, and reproducible import settings;
- whether generated outputs and evidence can be rebuilt from preserved source.

Do not equate smaller files with a better pack. Report optimization changes
beside motion, deformation, and runtime verification.
