# Animation pack evaluation: Mixamo Female Basic Locomotion

> Technical verdict: **Restricted use**
>
> Evaluation completeness: **partial** — all 20 extracted FBX files have current mechanical evidence; vendor mapping, runtime, visual, contact, retarget, license, and cross-pack acceptance remain unavailable.
>
> Confidence: **medium**
>
> Evaluation date: **2026-09-16**
>
> Current evaluator: **AnimSmith 0.14.0**
>
> Report format: **3**
>
> Detailed evidence: [Evidence appendix](mixamo-female-basic-locomotion-evidence.md)

## Technical decision

For a prototype, start with a female walk/run speed blend using the selected files in the [runtime set](#runtime-sets-and-authored-motion). Let the kinematic controller own XZ movement, yaw, and collision; the selected roots are effectively stationary. This is an evaluator-proposed set (`hypothesis/kinematic-speed`), with no accepted loop, blend, foot-contact, or target-engine result. Keep it out of production until the [issue gates](#technical-issue-register) and [engine review](#engine-status) pass.

All 20 delivered FBX files loaded under AnimSmith 0.14.0 and the untouched baseline produced 0 errors, 5 `duration-sanity` warnings, and 2843 non-gating `constant-track` notes. Applying one animation-owned movement rule across the archive produced 6 errors where files in the root-motion directory remain effectively stationary. Decide movement ownership per file before using those states; a directory name alone does not establish authored displacement. Two `X Bot.fbx` files use a 68-bone reference skeleton; the other 18 files use 66 bones, and every file resolves the nine-role Mixamo profile.

## Capability coverage

This is a full-body humanoid controller hypothesis for visible characters. Third-person gameplay-camera appearance and target-character deformation remain visually untested. Dedicated first-person arms/viewmodel content and close-camera suitability were not established.

### Content present

| Candidate ingredients in the evaluated files | Current use boundary |
|---|---|
| Walk and run are named and measured. Strafe walking, idle, jump and turns appear by filename; their controller roles remain unclassified. | Check the named states and their transitions in the target game before treating this as a complete controller. |

### Content gaps and unknowns

The legacy count of 18 motions has no verified mapping to these 20 files. Classify the other named motions before adding them to a controller.

### Evaluation still needed

- Loop/phase policy, target-engine graph, contact and visual review, target-character retargeting, performance and cross-pack acceptance remain open.

## Runtime sets and authored motion

| Set | Controller use | Adoption decision | Exact members |
|---|---|---|---|
| `hypothesis/kinematic-speed` | Walk/run speed blend; controller owns XZ translation, yaw and collision | The selected walk/run pair has no reported mechanical finding. Prototype with controller-owned movement and project-defined speed thresholds. Six other root-motion-directory states need per-file ownership; left strafe walk has a source endpoint warning. No AnimSmith repair was tested. | [Member timings, coordinates and contracts](mixamo-female-basic-locomotion-evidence.md#exact-runtime-members) |

The selected files are grouped by name and measurement. AnimSmith measured phase (cycles) as walk `0.4913` and run `0.4289`; their measured phase spread is `0.0624` cycles. These phase numbers do not establish matching foot contacts or a usable blend. Loop intent, contact alignment, and gameplay use remain `not-evaluated`.

## Integration recipe

1. **Members/topology:** Use the exact members and coordinates for `hypothesis/kinematic-speed` in the linked appendix as a proposed directional blend. Add other directions or in-place/root-motion alternatives only after checking their own files.
2. **Timing/synchronization:** Keep source timing and leave loop settings undecided. Compare normalized phase and visible foot contacts in the target engine before enabling a continuous blend.
3. **State ownership:** These selected in-place files supply pose; one kinematic controller supplies XZ translation, yaw, and collision. Root-motion use, where animation supplies travel, requires separate per-file checks.
4. **Composition constraints:** Begin with full-body states and transitions. Masks, additive motion, IK, sockets, and layering need separate tests.
5. **Acceptance gate:** In the chosen engine, test the selected blend at center and axes, rapid input changes, starts and stops, foot contacts, interrupted transitions, and target-character deformation.

## Technical issue register

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| MIX-OWN-001 | moderate | Scope: `Female_Basic_Locomotion_Pack_-_root-motion/idle.fbx`; `Female_Basic_Locomotion_Pack_-_root-motion/jump.fbx`; `Female_Basic_Locomotion_Pack_-_root-motion/left turn (2).fbx`; `Female_Basic_Locomotion_Pack_-_root-motion/left turn.fbx`; `Female_Basic_Locomotion_Pack_-_root-motion/right turn (2).fbx`; `Female_Basic_Locomotion_Pack_-_root-motion/right turn.fbx`; reproduce: lint each file with the current archive-level animation-owned XZ declaration; impact: applying that policy can leave these states stationary while gameplay expects displacement. Guidance: not applicable. | engine-config | Declare these files controller-owned XZ unless project evidence establishes authored displacement. Accept when per-file declared lint has no ownership error and one target-controller playback applies translation/yaw/collision exactly once. | No generic rewrite is justified because the directory label does not establish per-file intent. | `observed-animsmith`; 6 current findings; no runtime acceptance; unresolved. |
| MIX-TIME-001 | moderate | Scope: `Female_Basic_Locomotion_Pack_-_in-place/left strafe walk.fbx`; `Female_Basic_Locomotion_Pack_-_root-motion/left strafe walk.fbx`; `Female_Basic_Locomotion_Pack_-_root-motion/left turn (2).fbx`; reproduce: empty-baseline `duration-sanity`; impact: shorter channels clamp-hold at the end and can create a brief frozen joint or transition/loop pop. [Guidance](../game-ready-clips.md#the-readiness-ladder). | artist-author | Align shorter keyed channels to the intended endpoint without changing the intended terminal pose or trajectory. Accept when `duration-sanity` is clean and slowed target-engine boundary playback shows no hold or pop. | Deterministic endpoint conformance is plausible only after the intended endpoint and protected channels are declared. | `observed-animsmith`; 3 current source files; visual intent not evaluated; unresolved. |
| MIX-CONTENT-001 | note | Scope: `Female_Basic_Locomotion_Pack_-_in-place/X Bot.fbx`; `Female_Basic_Locomotion_Pack_-_root-motion/X Bot.fbx`; reproduce: empty-baseline `duration-sanity` reports `Take 001` with no tracks; impact: automatic clip registration can expose an empty state, although these may be reference-character files. Guidance: not applicable. | unknown | Classify each file as reference-only or intended animation before import. Accept when reference files are excluded from animation registration or the author supplies the intended tracks. | No tool should invent missing motion. | `observed-file` and `observed-animsmith`; semantic purpose unavailable; unresolved. |

## Engine status

| Runtime | Evidence level | Technical result | Remaining gate |
|---|---|---|---|
| Unity unspecified | not-evaluated | No current import, graph, playback, or visual evidence | Test the proposed controller and full-body graph on the target character |
| Unreal Engine unspecified | not-evaluated | No current import, retarget, Blend Space, or playback evidence | Test the exact retarget and controller path |
| Godot unspecified | not-evaluated | No current import, AnimationTree, or playback evidence | Test the exact Skeleton3D and controller path |
| Bevy unspecified | not-evaluated | No current loader, graph, or playback evidence | Test exact animation targets and controller composition |

## Fit and limitations

Best fit is basic female locomotion prototyping where a developer can own movement, define thresholds, and test the exact target character. It is a poor fit for drop-in root-motion, network, motion-matching, layered-animation, or production-ready claims.

Cross-pack compatibility is unknown. Shared Mixamo role resolution and a common 66-bone count on motion-named files do not prove identical hierarchy, rest pose, scale, retargeting, blending, contacts, or style.

## Changes between AnimSmith versions

AnimSmith 0.14.0 — The current evaluation reran 996 inspect, measure, and lint commands across the collection. This constituent admitted 20/20 files; baseline and declared-contract findings were revalidated, and the selected controller set was proposed from current files. Its clip roles remain unverified.

AnimSmith 0.10.0 — Retained historical mechanical counts agree with the current rerun; clip mapping and engine acceptance remain open. AnimSmith 0.7.0 evidence remains superseded.

## Evidence status

The current tests covered 20 FBX source files individually. The legacy metadata lists 18 motions, but its clip-to-file mapping is unverified for the 20 files. The [evidence appendix](mixamo-female-basic-locomotion-evidence.md) records the source integrity check, evaluator revision, command results, and digests. See the [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder).

## Sources

- Fresh external AnimSmith 0.14.0 command ledger and summary; licensed source and outputs remain external.
- [AnimSmith game-ready clip guidance](../game-ready-clips.md).
