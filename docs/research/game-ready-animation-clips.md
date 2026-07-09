# Game-Ready Animation Clips: Research Notes for Animsmith

Date: 2026-07-09

Scope note: this is a dated research artifact, not the current animsmith
product contract. The user-facing explainer of what animsmith checks and
why is [`docs/game-ready-clips.md`](../game-ready-clips.md). Before
converting any recommendation into an issue, reconcile it against
`DESIGN.md`, the shipped CLI docs, the stable check ids, and any newer
documentation that has merged since this note was written. The first
such reconciliation is the release roadmap at
[`docs/ROADMAP.md`](../ROADMAP.md).

## Executive Summary

A game-ready animation clip is not just an animation that looks good in a DCC tool. It is a clip whose data, rig assumptions, root motion behavior, loop boundaries, contact quality, metadata, compression tolerance, and import settings are predictable enough for a real-time engine and a production pipeline.

The engine documentation points to the same practical contract from different angles:

- Unity treats clips as isolated units of motion, then exposes import-time decisions for clip ranges, looping, root transform rotation and position, compression, custom curves, events, masks, and retargeting warnings.
- Unreal treats an Animation Sequence as skeletal transform data tied to a Skeleton, with import choices for frame range, sample rate, skeleton matching, custom attributes, root motion, notifies/curves, and compression.
- Godot exposes importer-level decisions for animation optimization, saving animations as separate files, timeline slicing, and retargeting through bone maps/rest-pose alignment.
- Bevy is glTF-forward and code-driven: clips commonly arrive as named glTF animations, are loaded through asset labels, and become `AnimationClip`s driven by `AnimationPlayer`, `AnimationGraph`, transitions, events, masks, and target IDs.
- glTF constrains animation data at the format level: node hierarchies must be valid trees, animated nodes must use TRS rather than matrix transforms, times and accessors must be valid, and interpolation must be supported correctly.

That suggests animsmith's core value proposition:

> Animsmith should be a repeatable animation-readiness gate and repair assistant: it turns ambiguous "this animation feels broken in the engine" feedback into measurable checks, actionable reports, CI-friendly failures, and safe transforms for common engine import workflows.

The best first product surface is not a full DCC replacement. It is a clip audit, measurement, normalization, and reporting tool for teams that receive raw mocap, marketplace assets, outsourced FBX files, glTF characters, or DCC exports and need them to become engine-safe assets.

## Current Animsmith Reconciliation

This research was written from engine and pipeline needs inward. The
existing animsmith implementation already covers part of that surface.
The useful planning question is therefore not "what should animsmith
build from scratch?" but "which gaps remain after mapping the research
against the current product?"

Shipped or already documented capabilities include:

- `animsmith lint` as the CI-friendly audit command, with project config,
  stable exit codes, text output, and JSON output.
- `animsmith measure` for raw measurements without judgment.
- Versioned JSON output with `schema_version`.
- A self-contained HTML `report` command.
- Stable kebab-case check ids such as `time-monotonic`, `nan`,
  `quat-norm`, `quat-flip`, `duration-sanity`, `scale-keys`,
  `constant-track`, `loop-seam`, `gait-group`, `root-motion-speed`,
  `missing-bones`, `frozen-bone`, `in-place`, `foot-slide`, `fps`, and
  `bind-pose`.
- Safe mechanical repairs through `fix`, including quaternion
  normalization and hemisphere normalization.
- Mechanical transforms such as slicing, hold extension, gait anchoring,
  and fps-aware transforms.
- `diff` for comparing meaningful metric changes across asset revisions.

Genuinely new or underdeveloped areas surfaced by this research include:

- Engine profiles distinct from rig profiles: Unity, Unreal, Godot, Bevy,
  and generic glTF/runtime behavior.
- Engine import prediction and optional engine readback/smoke tests.
- Bevy-specific asset addressability: `GltfAssetLabel` manifests, named
  animation inventories, target-id reports, and RON animation graph
  templates.
- Event, curve, notify, contact, and gameplay-window metadata checks.
- Duplicate loop-endpoint detection and safe removal.
- Blend entry/exit pose checks against declared transition families.
- Retarget bone-map, bone-length, and rest-pose risk diagnostics.
- Runtime-facing sidecars, including contact events and engine import
  manifests.
- Markdown report output, if a lightweight non-HTML artifact is useful.

The check-like names later in this document are conceptual categories,
not proposed public ids. Existing shipped check ids are public contract:
renaming or replacing them with dotted taxonomy names would require a
separate pre-1.0 design decision.

The transform ideas also need to respect the current design guardrail:
animsmith may rewrite clips only in ways whose correctness its own checks
can verify. Retargeting, rest-pose rewriting, procedural foot locking,
motion warping, stride normalization, and other artistic motion edits
should be treated as ADR-level scope proposals unless they can be reduced
to mechanical, measurable, reversible operations.

## What "Game-Ready" Means

An animation clip is game-ready when it satisfies four contracts at once.

### 1. Runtime Contract

The engine can import, store, sample, blend, compress, retarget, and play the clip without surprises.

This includes:

- Valid file structure and buffers.
- Supported animation channels and interpolation modes.
- Predictable sample rate or key distribution.
- Finite values, normalized rotations, sane scale, and no hidden shears.
- Reasonable key counts and memory footprint.
- No unnecessary animated scale or zero-value curves unless intentionally used.

### 2. Character Controller Contract

The clip's spatial behavior matches the gameplay system that will drive it.

This includes:

- Clear in-place vs root-motion classification.
- Expected root bone, root node, or motion extraction source.
- Horizontal, vertical, and rotational root motion treated consistently.
- No foot sliding during planted phases beyond project tolerance.
- No ground penetration or unintended floating.
- Movement distance, stride, speed, and phase aligned with locomotion design.

### 3. Retargeting Contract

The clip can be used on the intended rigs without silent degradation.

This includes:

- Compatible skeleton hierarchy, bone names, and bone count policy.
- Known rest pose, bind pose, unit scale, and orientation.
- Bone lengths and default rotations within tolerance.
- Humanoid/quadruped/creature profile mapping where applicable.
- No tracks that the target engine will discard without the user noticing.

### 4. Pipeline Contract

Artists, technical animators, engineers, and build systems can understand the asset state.

This includes:

- Stable naming and clip slicing.
- Actionable warnings, not vague pass/fail output.
- JSON/HTML/Markdown reports for CI and reviews.
- Profile-specific thresholds per engine/project.
- A clear path from "failed" to "fix in DCC" or "auto-transform safely".

## Engine Observations

### Unity

Unity's Animation tab makes the production concerns explicit. The Manual describes Animation Clips as the smallest building blocks of Unity animation and lets imported FBX data become selectable clips. The importer includes asset-level controls such as import animation, baking IK/simulation to FK keyframes, resampling Euler curves to quaternions, and animation compression. It also exposes clip-level controls for frame ranges, Loop Time, Loop Pose, Cycle Offset, root transform rotation, root transform position Y, root transform position XZ, curves, events, and Avatar masks.

Root motion is treated as a first-class import decision. Unity computes a root transform projection from the body transform for humanoid clips, and lets the user decide whether rotation, vertical position, and horizontal position are baked into the pose or stored as root motion. Unity also warns that import messages can indicate that source data was discarded or that retargeted animation may not exactly match the source.

Implication for animsmith:

- Checks should report whether a clip is an in-place candidate, root-motion candidate, or mixed/ambiguous.
- Loop validation should not only compare first and last local poses. It should compare root-space pose continuity, root displacement continuity, and phase/contact continuity.
- Reports should distinguish "engine will import" from "engine will drop or reinterpret data".
- Compression readiness should be measurable before import: key counts, redundant tracks, high-frequency jitter, rotation/position/scale error estimates.

Primary references:

- Unity Manual, Animation tab: https://docs.unity3d.com/Manual/class-AnimationClip.html
- Unity Manual, How Root Motion works: https://docs.unity3d.com/Manual/RootMotion.html

### Unreal Engine

Unreal's FBX animation pipeline says the FBX pipeline is the route for getting skeletal animations from DCC tools into Unreal, and notes that the UE FBX importer uses FBX 2020.2. The Animation Sequence docs define an Animation Sequence as keyframes for a Skeletal Mesh's Skeleton, with position, rotation, and scale sampled over time. Sequences are tied to Skeletons, which allows sharing across meshes using the same Skeleton.

Unreal's import settings expose the same pipeline concerns in engine terms: animation length, frame import range, sample rate, skeleton selection, importing custom attributes, bone tracks, redundant-key removal, and preservation of local transforms. Unreal also flags a practical issue: animations with non-whole end frame values do not import correctly. Root motion requires a skeleton with a root bone and root-bone motion data, then must be enabled per Animation Sequence or Montage. Unreal also separates root motion extraction modes and documents a performance cost when root motion requires animation graph work on the game thread.

Implication for animsmith:

- An Unreal profile should check FBX version where detectable, whole-frame end ranges, root bone presence, root-bone motion, skeleton name/hierarchy compatibility, track scale usage, and custom attribute/event curves.
- Root motion checks should include "root motion exists but is not safely extractable" and "root motion exists but looks like accidental drift".
- Performance/readiness checks should classify clips by compression risk and redundant track count.

Primary references:

- Unreal Engine, FBX Animation Pipeline: https://dev.epicgames.com/documentation/en-us/unreal-engine/fbx-animation-pipeline-in-unreal-engine
- Unreal Engine, Animation Sequences: https://dev.epicgames.com/documentation/en-us/unreal-engine/animation-sequences-in-unreal-engine
- Unreal Engine, Root Motion: https://dev.epicgames.com/documentation/en-us/unreal-engine/root-motion-in-unreal-engine

### Godot

Godot's 3D scene import docs expose two relevant concepts for animsmith. First, the advanced import settings include animation optimization, saving animations to external files, and slicing one source timeline into multiple named animations with start/end frames and loop flags. Second, Godot's retargeting documentation emphasizes that bone names alone are insufficient: bone rest transforms and hierarchy matter, and models exported from different DCC tools can have different rest rotations.

Implication for animsmith:

- A Godot profile should validate that a single source timeline can be sliced into named clips and that those slices have frame-accurate boundaries.
- Retarget-readiness checks should include rest-pose/bone-rest diagnostics, not just bone-name mapping.
- Reports should include "will share animation correctly" vs "imports but retargeting will likely distort".

Primary references:

- Godot, Advanced Import Settings, Animation options: https://docs.godotengine.org/en/stable/tutorials/assets_pipeline/importing_3d_scenes/advanced_import_settings.html
- Godot, Retargeting 3D Skeletons: https://docs.godotengine.org/en/stable/tutorials/assets_pipeline/retargeting_3d_skeletons.html

### Bevy

Bevy's animation path is different from Unity, Unreal, and Godot because the main engine-facing workflow is not an editor import inspector. In Bevy 0.19, the glTF plugin loads glTF 2.0 assets, exposes specific parts through `GltfAssetLabel`, and creates Bevy-side representations such as scenes, nodes, skins, animations, and named animations. The animation crate defines `AnimationClip` as curves mapped to `AnimationTargetId`s, `AnimationPlayer` as the playback controller, `AnimationGraph` as the blending surface, plus transitions, repeat modes, masks, and animation events.

The official examples show the practical contract: a skinned glTF scene can spawn an animation player, clips can be selected by numeric or named glTF animation labels, and an `AnimationGraphHandle` must be attached to drive playback. Bevy also supports serialized animation graphs in RON and runtime graph construction. That means Bevy-readiness is partly data quality and partly asset-addressability: the clip must be valid glTF, but it also needs stable names/indices, usable target IDs, predictable scene hierarchy, and graph/mask/event metadata that Rust systems can consume.

The glTF loader docs also matter for asset policy. Bevy documents supported and unsupported Khronos extensions; for animation-related planning, `KHR_animation_pointer` is not supported in the referenced Bevy 0.19 docs. The docs also warn that misspelled labels can be ignored when using string labels, recommending typed `GltfAssetLabel` for correctness. That makes name stability and manifest generation useful animsmith features for Bevy teams.

Implication for animsmith:

- A Bevy profile should start from strict glTF validation, then add Bevy-specific checks for named animations, asset labels, scene/default scene availability, skins, animation target IDs, and graph-readiness.
- Reports should include Bevy code-facing identifiers: scene labels, animation labels/indices, named animation keys, target IDs, skeleton root candidates, and whether an `AnimationPlayer`/graph setup can be inferred from the asset.
- Animsmith should warn when clips depend on unsupported glTF extensions or data paths that Bevy will not load into ordinary `AnimationClip` curves.
- For Bevy projects, sidecars are especially valuable: generated RON animation graph templates, Rust-friendly metadata JSON, animation event/contact sidecars, and import manifests can reduce fragile hardcoded indices.

Primary references:

- Bevy docs.rs, `bevy::animation`: https://docs.rs/bevy/latest/bevy/animation/index.html
- Bevy docs.rs, `bevy::gltf`: https://docs.rs/bevy/latest/bevy/gltf/index.html
- Bevy example, Animated Mesh: https://bevy.org/examples/animation/animated-mesh/
- Bevy example, Animation Graph: https://bevy.org/examples/animation/animation-graph/
- Bevy example, Animated Mesh Control: https://bevy.org/examples/animation/animated-mesh-control/

### glTF

glTF gives animsmith a precise low-level validation target. A glTF node hierarchy must be a set of disjoint strict trees, with no cycles and at most one parent per node. Animated nodes must use translation, rotation, and scale properties rather than matrix transforms. TRS is composed in T * R * S order. Animation interpolation modes define how values are evaluated between keyframes, with special handling for quaternion rotation interpolation.

Implication for animsmith:

- glTF validation should include structural checks before semantic animation checks.
- Animated matrix transforms should be reported as a format-level incompatibility.
- Rotation tracks should be checked for normalized quaternions and interpolation-safe continuity.
- Accessor bounds, finite values, monotonic timestamps, and channel target validity should be hard errors.

Primary reference:

- Khronos glTF 2.0 Specification: https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html

## What Should Be Verified

This section is written as a check catalog animsmith can gradually implement.

### Format and Importability

Verify:

- File parses cleanly.
- All referenced buffers/images/accessors exist.
- Node hierarchy has no cycles.
- Animation channels point to valid nodes and supported target paths.
- Animated transforms use supported TRS representation.
- Timestamps are monotonic and finite.
- Rotation values are valid quaternions.
- No NaN, infinity, denormal-heavy data, or extreme coordinate values.
- Declared duration matches actual key times.
- File extensions, embedded/external buffers, and path references are portable.

Why it matters:

Bad file structure creates hard import failures. Slightly malformed animation data can create engine-specific behavior that is hard to diagnose after import.

Animsmith output:

- Hard errors for broken references and unsupported animated transform representation.
- Warnings for unusual but technically legal data, such as animated scale on many bones.
- A "portable asset" score for pathing and external dependency stability.

### Skeleton and Rig Compatibility

Verify:

- Expected root bone or root node exists.
- Skeleton hierarchy matches project profile.
- Bone names match a configured naming map or known profile.
- Bone parent-child relationships match target skeleton.
- Rest pose and bind pose are available and consistent.
- Bone lengths and default rotations are within tolerance.
- No duplicate bone names where the engine/profile requires uniqueness.
- No unnecessary mesh nodes inside bone hierarchy for profiles that treat those specially.
- No unsupported helper/control bones unless explicitly ignored.

Why it matters:

Retargeting often fails as "almost correct". The clip imports, but shoulders twist, feet rotate, hips drift, or scale changes leak into the pose. Engine docs consistently make skeleton compatibility central to sharing animations.

Animsmith output:

- Bone map report: matched, missing, duplicate, extra, parent mismatch, rest-pose mismatch.
- Retarget risk level: low, medium, high.
- DCC-oriented fix hints: rename, map, remove helper tracks, rebake rest pose, export skeleton only.

### Clip Segmentation, Naming, and Timing

Verify:

- Clip has stable name, action name, or slice definition.
- Start/end frames are explicit and whole-frame when the target profile requires it.
- Duration is above a useful minimum.
- Start/end frame selection does not duplicate a pose in a way that causes a visible hitch.
- Source timeline can be split into named clips without overlap mistakes.
- Sample rate is expected or intentionally variable-keyed.
- For looping clips, loop flag/policy is present or inferable.

Why it matters:

Many "bad clips" are bad cuts. A walk loop that includes both frame 0 and the repeated final frame may pause. An attack clip with a fractional end frame may import wrong in Unreal. A single mocap timeline without slices is not directly usable by designers.

Animsmith output:

- Clip table: name, source take, start, end, duration, frame count, sample rate.
- Slice validation: gaps, overlaps, fractional boundaries, duplicate endpoint risk.
- Suggested split definitions for DCC/export/import tools.

### Root Motion and In-Place Policy

Verify:

- Root track exists when required.
- Root node/bone is unambiguous.
- Root translation and rotation are intentional, not tiny drift.
- Horizontal XZ root motion matches gameplay expectations.
- Vertical Y root motion is either baked into pose or extracted according to profile.
- Root rotation is either baked into pose or extracted according to profile.
- In-place clips have near-zero root displacement and near-zero accumulated drift.
- Root-motion clips have coherent trajectory, speed, and final displacement.
- Root path does not jump unexpectedly at clip boundaries.
- Root lock policy can be expressed for target engine.

Why it matters:

Root motion is where animation and gameplay meet. Unity, Unreal, and Godot all expose controls that decide whether motion moves the character object, stays in the pose, or is discarded. The same raw data can be correct for one gameplay model and wrong for another.

Animsmith output:

- Classification: in-place, root-motion, vertical-motion, rotational-root, ambiguous, broken.
- Metrics: displacement, facing delta, average speed, peak speed, vertical travel, path curvature.
- Policy diff: what changes if the clip is imported as Unity in-place, Unity root-motion, Unreal root-motion, or generic glTF playback.

### Loop Quality

Verify:

- First and last poses match within tolerance after accounting for root-space projection.
- Root displacement per cycle is expected.
- Velocity at loop boundary is continuous.
- Foot/contact phase is continuous.
- Upper-body additive layers do not pop at wrap.
- Quaternion continuity does not flip at wrap.
- Loop contains a complete gait cycle where expected.
- Loop has no hidden endpoint hold.

Why it matters:

Engine loop toggles do not make a bad loop good. They repeat the data. A good loop must preserve pose, velocity, root-space relation, and contacts.

Animsmith output:

- Loop score with component deltas: root position, root rotation, skeleton pose, key contact bones, velocity.
- Suggested trim offsets, such as "remove duplicated final frame" or "start at opposite foot contact".
- Optional transform: distribute small loop-pose correction across the clip, similar in spirit to an import-time loop-pose adjustment.

### Contact, Grounding, and Foot Slide

Verify:

- Feet, toes, hands, weapon tips, or configured contact bones respect contact windows.
- Planted foot horizontal velocity remains below threshold.
- Planted foot vertical position is near floor or contact plane.
- Feet do not penetrate the ground beyond tolerance.
- Feet do not float during expected support phase.
- Contact events align with visual contact frames.
- Stride length and cadence are plausible for the character scale.
- Character root speed matches observed foot motion.

Why it matters:

Foot slide is one of the fastest ways for a game animation to feel ungrounded. It also exposes root-motion/in-place mismatch, scale mismatch, bad retargeting, and bad trimming.

Animsmith output:

- Foot-slide report per foot/toe/contact bone.
- Contact timeline and inferred support phases.
- Warnings for "root speed inconsistent with stride" and "floor plane appears offset".
- Optional event generation: footstep left/right markers and contact windows.

### Blend and State-Machine Friendliness

Verify:

- Entry/exit poses are compatible with expected transitions.
- Idle/start/stop/turn clips have appropriate neutral or authored transition frames.
- Locomotion clips expose phase for blend trees or motion matching.
- Directional clips use consistent forward axis and speed labels.
- Additive clips have a correct reference pose/frame.
- Attack/ability clips expose anticipation, active, recovery, and cancel windows if configured.
- Facial/upper-body overlays do not animate lower-body tracks unless intended.

Why it matters:

A single clip can look good in isolation and still be awkward in a state machine. Designers and animation engineers need phase, event, and transition metadata, not just poses.

Animsmith output:

- Phase metrics for loops.
- Entry/exit pose deltas against configured transition families.
- Metadata coverage report: events, curves, notifies, gameplay windows.
- Suggested tags: locomotion, idle, start, stop, turn, additive, attack, hit-react.

### Runtime Performance and Compression

Verify:

- Total key count and key density.
- Per-track key count.
- Redundant keys.
- Tracks with constant values.
- Animated scale track count.
- Curves with all-zero values.
- High-frequency jitter that compression will preserve or amplify.
- Estimated error under key reduction.
- Memory footprint estimate before and after cleanup.

Why it matters:

Unity and Unreal both expose key reduction/compression as part of normal animation import. Compression is not just an engine-side detail; bad raw data makes compression less effective and more visually risky.

Animsmith output:

- Key-count budget pass/fail.
- Redundant track report.
- Compression-risk score.
- Suggested safe transforms: drop constant scale tracks, remove zero curves, reduce keys under configured tolerance, resample only where needed.

### Metadata, Events, Curves, and Gameplay Signals

Verify:

- Clip has required events for its type.
- Gameplay windows are present and ordered.
- Curves use expected names and value ranges.
- Footstep/contact events align with inferred contact frames.
- Root motion/in-place tags match measured behavior.
- Custom attributes are supported by target profile.
- Events are not outside clip range after trimming.

Why it matters:

Gameplay often depends on animation metadata: footsteps, hit frames, invulnerability windows, VFX triggers, audio, IK weights, look-at masks, turn curves, stride warping, and blend weights. Raw assets usually do not arrive with this information in a project-specific form.

Animsmith output:

- Metadata completeness report.
- Event alignment warnings.
- Exportable event/curve sidecars for engines that cannot preserve the source representation.

### Asset Addressability and Runtime Wiring

Verify:

- Each clip can be addressed by stable name, index, label, or manifest entry.
- Named animations and numeric animation indices match the target runtime's expected paths.
- Scene/default scene labels are present for runtimes that spawn animated scenes from asset labels.
- Animation target identifiers can be mapped back to bones/entities after import.
- Animation graph, blend tree, mask, or state-machine inputs can refer to the intended clips without hardcoded guesswork.
- Event/contact sidecars use stable clip IDs and frame/time coordinates after trimming or resampling.
- Required engine/runtime features are supported by the asset loader, such as glTF extensions and animation channel types.

Why it matters:

This is especially important for Bevy and custom runtimes. A clip can be valid glTF and still be awkward to use if the project has to rely on fragile numeric indices, unnamed clips, missing default scenes, unsupported extensions, or target IDs that are hard to connect to gameplay code.

Animsmith output:

- Bevy/glTF asset manifest: scenes, animations, named animations, skins, target IDs, root candidates, and labels.
- Runtime wiring warnings: missing default scene, unnamed animation, unsupported extension, ambiguous animation target, graph/mask metadata missing.
- Optional generated templates: Bevy `GltfAssetLabel` manifest, RON animation graph skeleton, and event/contact sidecar.

## Common Asset Process: Raw to Game-Ready

A practical game animation pipeline often looks like this:

1. Acquire source asset

   Sources include mocap capture, marketplace packs, outsourced DCC files, Mixamo-style libraries, procedural generation, animation retarget output, or internal Maya/Blender exports.

2. Normalize and preserve source

   Keep raw source immutable. Record source author, license, DCC package, export settings, units, skeleton profile, and intended target engine.

3. Parse and inspect

   Validate file structure, skeleton, clips, track counts, root motion, sample rates, durations, and metadata. This is where animsmith should operate first.

4. Segment clips

   Split source takes or long timelines into named gameplay clips. Trim endpoints. Decide loop flags. Remove accidental duplicate final frames. Record start/end frames in versioned config.

5. Decide root motion policy

   For each clip, choose in-place, root-motion XZ, vertical root motion, rotational root motion, or baked pose. This should be explicit in config and checked by animsmith.

6. Retarget and conform

   Map to project skeleton, apply rest-pose corrections, remove helper/control tracks, normalize scale, and rebake transforms where needed.

7. Validate motion semantics

   Check loop continuity, foot contacts, root trajectory, stride, phase, floor contact, event windows, and blend entry/exit.

8. Optimize

   Remove redundant keys/tracks, compress or prepare for engine compression, remove unsupported curves, and resample only when it improves portability.

9. Export engine-facing artifacts

   Export FBX, glTF/GLB, engine-specific sidecars, event JSON, import presets, or generated clips.

10. Engine import smoke test

   Import into Unity/Unreal/Godot where possible, capture warnings, compare engine-readback metadata, and verify the clip still matches measured expectations.

11. CI gate and report

   Run checks in continuous integration. Attach HTML/Markdown reports to asset review. Track metric diffs across revisions.

## How Animsmith Can Help

Animsmith should position itself as a buildable, scriptable counterpart to engine import inspectors. Engines already show many of the problems, but they show them late, inside editor workflows, and often after engine-specific conversion has already happened. Animsmith can catch and explain those problems before import.

### Core Value Proposition

Animsmith helps teams answer:

- Will this clip import into my target engine?
- Will the engine reinterpret, discard, or compress data in a way that changes motion?
- Is this clip in-place or root-motion, and does that match the gameplay contract?
- Is this loop actually seamless?
- Are the feet grounded and contacts usable?
- Can this clip retarget to our skeleton?
- Are the events, curves, and gameplay windows present?
- Did this asset revision make the clip better or worse?

### Stakeholder Value

For gameplay engineers:

- Fewer movement/controller mismatches.
- Measurable root motion, speed, stride, and phase.
- CI gates for animation regressions.
- JSON output that can feed gameplay tooling.

For animation engineers and technical animators:

- Clear rig, retarget, root, loop, contact, and compression diagnostics.
- Batch validation across libraries.
- Repeatable thresholds per character, engine, and clip family.
- A safer way to prepare clips for blend trees, state machines, and motion matching.

For artists and animators:

- Actionable reports instead of vague "import looked wrong" feedback.
- Visualizable metrics: root path, foot contact, loop error, pose deltas.
- Fix hints that map to DCC actions: trim frames, freeze scale, bake keys, rename bones, remove helper controls, fix rest pose.

For producers and asset pipeline owners:

- Reduced review churn.
- Better outsourced asset acceptance criteria.
- A machine-readable definition of "done".
- Library-wide inventory and quality metrics.

## Recommended Animsmith Requirements

### Product Requirements

Shipped: clip audit command

- `animsmith lint <asset>` already runs structural, skeleton, clip,
  root-motion, loop, contact, and performance checks.
- It supports project config, stable exit codes, text output, and JSON
  output.
- Future work should extend this surface with engine profiles rather
  than introduce a parallel `check` command.

P0/P1: Engine profiles

- Built-in profiles: `generic`, `unity-generic`, `unity-humanoid`, `unreal`, `godot`, `bevy`, `gltf-runtime`.
- Profiles define root policy, unit/axis expectations, skeleton profile, clip boundary rules, loop thresholds, contact bones, and unsupported track behavior.

Shipped: metrics command

- `animsmith measure` already reports raw measurements without judgment.
- Future work should add missing engine- and metadata-oriented metrics:
  asset labels, event windows, transition pose deltas, duplicate endpoint
  hints, import-risk fields, and Bevy target-id summaries.

Shipped/P1: report formats

- The HTML `report` command already provides a human-readable artifact.
- JSON output is already versioned and should remain the machine-readable
  source of truth.
- Markdown output would be new and useful for CI comments, issue filing,
  and lightweight design reviews.
- Any report should keep exposing:
  - "What failed"
  - "Why it matters in engines"
  - "How to fix"
  - "Which profile threshold was used"
  - "Raw metric values"

Shipped/P1: Configuration

- `animsmith.toml` already provides project config for rig profiles,
  checks, clips, thresholds, and expectations.
- Future config work should add engine-profile settings for:
  - skeleton profiles
  - root bones/nodes
  - contact bones
  - floor plane
  - clip families
  - thresholds
  - required metadata/events
  - engine profile

P1: Transform command

- Safe transforms:
  - split/slice clips
  - trim frames
  - remove duplicate endpoint frame
  - normalize quaternion signs
  - drop redundant constant tracks when the result is provably equivalent
  - remove all-zero curves when the target profile permits it
  - generate footstep/contact events

Transforms that alter authored motion need a separate design decision:
root-motion/in-place conversion, retargeting, rest-pose rewriting, floor
alignment, key reduction with error tolerance, motion warping, procedural
foot locking, and stride normalization are valuable ideas but exceed the
current "mechanical and check-verifiable" bar unless narrowed.

P1: Diff command

- Compare raw vs transformed clip and asset revision vs previous revision.
- Report motion deltas, loop score changes, key count changes, foot-slide changes, and retarget risk changes.

P1: Engine-import preset generation

- Generate suggested Unity/Unreal/Godot import settings, Bevy asset manifests, or sidecar instructions:
  - loop flags
  - root motion node/bone
  - sample rate
  - clip ranges
  - compression tolerances
  - event/curve metadata
  - Bevy `GltfAssetLabel` paths and optional animation graph templates

P2: Visual report

- Web/HTML viewer with:
  - root trajectory
  - contact timeline
  - per-bone track summary
  - loop boundary pose delta
  - foot slide heatmap
  - before/after transform comparison

P2: Engine smoke tests

- Optional integration to import assets into Unity, Unreal, Godot, or a Bevy runtime harness and capture import warnings.
- Compare engine-side imported clip metadata to animsmith's predictions.

P3: DCC integrations

- Blender/Maya scripts or plugins that open animsmith reports at the failing frame/bone.
- Export profile templates.
- One-click "validate before export".

### Check Requirements

The following are capability categories for planning. They should map
onto existing stable ids where those exist, not replace them.

Shipped or partially shipped categories:

- `format.parse`
- `format.references`
- `animation.time_monotonic`
- `animation.finite_values`
- `animation.quaternion_validity`
- `animation.channel_target_validity`
- `skeleton.root_exists`
- `skeleton.hierarchy_profile`
- `skeleton.rest_pose_presence`
- `clip.duration`
- `clip.boundaries`
- `clip.whole_frame_end` for Unreal profile
- `root.classification`
- `root.drift_in_place`
- `root.motion_presence`
- `loop.pose_delta`
- `loop.root_delta`
- `loop.velocity_delta`
- `contact.foot_slide`
- `contact.floor_penetration`
- `performance.key_count`
- `performance.constant_tracks`
- `performance.animated_scale`

New or underdeveloped categories:

- `retarget.bone_map_quality`
- `retarget.rest_pose_delta`
- `retarget.bone_length_delta`
- `compression.estimated_error`
- `compression.jitter_risk`
- `blend.entry_exit_pose_delta`
- `phase.gait_cycle_consistency`
- `metadata.event_alignment`
- `engine.unity_import_prediction`
- `engine.unreal_import_prediction`
- `engine.godot_import_prediction`
- `engine.bevy_import_prediction`
- `bevy.gltf_asset_labels`
- `bevy.animation_target_ids`
- `bevy.animation_graph_readiness`
- `metadata.required_events`

### Transform Requirements

Safe transforms should be explicit, reversible where practical, and reported as patches to the animation data.

Initial transforms:

- Trim clip to frame/time range.
- Split source timeline into named clips.
- Remove duplicated loop endpoint.
- Normalize quaternion signs to avoid interpolation flips.
- Drop constant tracks below tolerance.
- Drop all-zero curves not required by profile.
- Resample to target sample rate.
- Bake or extract root XZ/Y/rotation according to profile.
- Floor-align root or contact plane where clearly configured.
- Generate contact events from foot/toe analysis.

High-risk transforms should require explicit opt-in:

- Retargeting.
- Rest-pose rewriting.
- Loop-pose correction distribution.
- Key reduction with nonzero error tolerance.
- Procedural foot locking.
- Motion warping or stride normalization.

## Documentation Animsmith Should Cover

### Documentation Landing Page

Explain animsmith in one sentence:

> Animsmith validates, measures, reports, and prepares skeletal animation clips for game-engine import and runtime use.

Then show three concrete examples:

- "Check whether a walk cycle is a good in-place loop for Unity."
- "Verify whether an FBX attack animation has a valid Unreal root bone, whole-frame end, and gameplay events."
- "Generate a Bevy-friendly glTF animation manifest with stable labels, contact events, and graph-ready clip metadata."

### Concepts Guide: What Is Game-Ready?

Cover:

- Game-ready vs visually good.
- Clip, take, sequence, action, slice.
- In-place vs root motion.
- Root bone vs root node vs body/root transform.
- Loop quality.
- Contact and foot slide.
- Retargeting readiness.
- Compression readiness.
- Events, curves, notifies, and gameplay windows.

### Engine Profile Guides

Create separate pages:

- Unity profile
- Unreal profile
- Godot profile
- Bevy profile
- glTF profile
- Generic runtime profile

Each page should include:

- What the engine expects.
- What animsmith checks.
- Common failures.
- How to configure thresholds.
- How to fix failures in DCC or engine import settings.

### Artist-Facing Failure Catalog

For every warning/error, document:

- Symptom in engine.
- What animsmith measured.
- Common cause.
- Fix in Blender/Maya/MotionBuilder where possible.
- Fix by animsmith transform where safe.
- When to ask an animation engineer.

Examples:

- Foot slides during planted phase.
- Loop pops at wrap.
- Root drifts in an in-place idle.
- Attack moves mesh away from capsule.
- Clip imports but retargeted shoulders twist.
- Scale tracks are discarded or cause compression issues.
- Animation has fractional end frame.
- Source timeline contains multiple unnamed clips.

### Engineer-Facing Reference

Cover:

- CLI commands.
- Config schema.
- Engine profile schema.
- JSON report schema.
- Exit codes.
- CI examples.
- Batch mode.
- Diff mode.
- Embedding/library API.

### Pipeline Guide

Show recommended workflows:

- Marketplace pack intake.
- Mocap cleanup gate.
- Outsourced asset acceptance.
- Character skeleton migration.
- Pre-commit asset checks.
- CI checks on animation asset changes.
- Raw vs transformed artifact storage.

## Roadmap Recommendations

### First Milestone: Trustworthy Audit

Goal: animsmith becomes useful before it mutates files.

Deliver:

- Engine profiles.
- Check catalog.
- JSON and Markdown/HTML reports.
- Metrics inventory.
- Root/in-place classification.
- Loop and foot-slide checks.
- Configurable skeleton/contact profiles.
- CI-ready exit codes.

### Second Milestone: Safe Preparation

Goal: animsmith can fix common mechanical issues whose correctness it can
verify.

Deliver:

- Clip splitting/trimming.
- Duplicate endpoint removal.
- Provably equivalent track pruning.
- Quaternion normalization.
- Fps-aware resampling with measured before/after diff.
- Contact event generation.
- Diff report for every transform.

### Third Milestone: Engine Feedback Loop

Goal: animsmith predicts and verifies real engine import behavior.

Deliver:

- Unity/Unreal/Godot import setting generation.
- Bevy asset manifest and animation graph template generation.
- Optional headless/editor import smoke tests.
- Optional Bevy runtime smoke-test harness.
- Import warning capture.
- Engine-readback comparison.
- Profile-specific docs generated from check metadata.

### Fourth Milestone: Retargeting Diagnostics and Authoring Assistance

Goal: animsmith helps technical animators diagnose retargeting and library
quality problems without silently becoming a DCC retargeter.

Deliver:

- Bone map validation.
- Rest-pose diagnostics.
- Retarget risk scoring.
- Motion library inventory.
- Batch reports and dashboards.
- DCC helper plugins.

Actual retargeting, rest-pose rewriting, and authored-motion repair would
need an explicit design-record update before becoming product scope.

## Product Positioning

Animsmith should not be positioned as "another converter" or "another animation editor". The stronger position is:

> Animsmith is a content quality gate for animation clips. It gives animation teams the same repeatable, reviewable, CI-friendly confidence that code teams expect from tests and linters.

The pitch differs by audience:

- Artists: "See exactly why the clip fails and which frame/bone needs attention."
- Technical animators: "Codify engine import rules and retargeting assumptions once, then batch-apply them."
- Gameplay engineers: "Stop debugging capsule/root/foot-slide problems after import."
- Producers: "Turn outsourced animation acceptance into measurable requirements."
- Tool engineers: "Use a library and JSON schema rather than scraping engine import logs."

## Suggested Issue Backlog

New issues worth filing after dedupe:

- Add duplicate endpoint detection and removal.
- Add engine profile config model distinct from rig profiles.
- Add engine import prediction for Unity/Unreal/Godot/Bevy profiles.
- Add Unity/Unreal/Godot/Bevy profile docs.
- Add Bevy glTF asset-label and named-animation manifest generation.
- Add Bevy animation graph template generation.
- Add generated contact events sidecar.
- Add gameplay event/window metadata checks.
- Add blend entry/exit pose-delta checks against transition families.
- Add retarget bone-map and bone-length diagnostics.
- Add Markdown report output.
- Add engine import preset generation.
- Add headless import smoke test adapters.
- Add Bevy runtime smoke-test harness.
- Add compression error estimator.
- Add batch library dashboard.

Ideas that need ADR or design-record updates before issue filing:

- Root-motion to in-place conversion, or in-place to authored trajectory.
- Retargeting and rest-pose rewriting.
- Procedural foot locking, motion warping, or stride normalization.
- Key reduction with nonzero visual/motion error tolerance.

## Appendix: Source Notes

- Unity's Animation tab documents import-level controls for animation clips, compression, clip ranges, looping, root transform controls, curves, events, masks, root motion node source, import messages, and retargeting warnings.
- Unity's Root Motion guide documents body/root transform behavior, baking rotation/Y/XZ into pose, root transform projection, gravityWeight implications, and loop pose behavior.
- Unreal's FBX Animation Pipeline documents the FBX import path, FBX 2020.2 requirement, animation export/import expectations, skeleton-only animation export, and skeleton selection.
- Unreal's Animation Sequences guide documents skeletal keyframe data, skeleton sharing, import options, whole-frame end frame issue, editing, sharing/retargeting, and compression.
- Unreal's Root Motion guide documents root bone requirements, root motion extraction, root lock options, debug visualization, and runtime/performance implications.
- Godot's Advanced Import Settings document animation optimizer, saving animations to file, and timeline slices.
- Godot's Retargeting 3D Skeletons guide documents bone rest, bone map, skeleton profile, and rest-pose alignment concerns.
- Bevy's animation docs describe `AnimationClip`, `AnimationPlayer`, `AnimationTargetId`, `AnimationGraph`, transitions, repeat modes, masks, and events as the runtime animation surface.
- Bevy's glTF docs describe loading glTF 2.0 assets, using typed `GltfAssetLabel`s for scenes and animations, named asset access, coordinate conversion utilities, skins, and supported/unsupported Khronos extensions.
- Bevy's animation examples show skinned glTF playback, named animation selection, animation graph blending, serialized graph assets, and attaching an animation graph handle for runtime playback.
- Khronos glTF 2.0 documents hierarchy, TRS animation constraints, animation interpolation behavior, skins, accessors, and binary data rules.
