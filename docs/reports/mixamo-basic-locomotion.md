# Animation pack evaluation: Mixamo Basic Locomotion

> Technical verdict: **Restricted use**
>
> Evaluation completeness: **partial** — all 12 extracted FBX files have current mechanical evidence; vendor mapping, runtime, visual, contact, retarget, license, and cross-pack acceptance remain unavailable.
>
> Confidence: **medium**
>
> Evaluation date: **2026-09-16**
>
> Current evaluator: **AnimSmith 0.14.0**
>
> Report format: **3**
>
> Detailed evidence: [Evidence appendix](mixamo-basic-locomotion-evidence.md)

## Technical decision

For a controller-driven (kinematic) prototype, start with three-direction unarmed walking using the [selected members](#runtime-sets-and-authored-motion). Their source mechanics were measured, but the proposed blend has no accepted loop, foot-contact, transition, target-character, or target-engine result. Use one controller for movement and yaw; decide ownership separately for any additional root-motion-directory state.

**First experiment:** Import the selected in-place files on the target character, play them individually, then test their blend through speed or direction changes, starts, stops, and interrupted transitions. Review any endpoint warning on other named files before admission: retain an intentional hold after visual review, or request source repair when the intended endpoint differs. The [evidence appendix](mixamo-basic-locomotion-evidence.md#mechanical-baseline) retains the exact source counts, warning totals, skeleton counts, and profile results. The [adoption guide](../commercial-pack-evaluations.md) compares this limited pilot with broader library and authoring routes.

## Capability coverage

This is a full-body humanoid controller hypothesis for visible characters. Third-person gameplay-camera appearance and target-character deformation remain visually untested. Dedicated first-person arms/viewmodel content and close-camera suitability were not established.

### Content present

| Candidate ingredients in the evaluated files | Current use boundary |
|---|---|
| Walk forward and strafe left/right are named and measured. Idle, jump and turns also appear by filename; their controller roles remain unclassified. | Check the named states and their transitions in the target game before treating this as a complete controller. |

### Content gaps and unknowns

**Other delivered motion leads, filename only:** `idle.fbx`, `jump.fbx`, `left turn 90.fbx`, and `right turn 90.fbx`. These names occur in the inspected source or issue inventory, but their intended controller roles and gameplay behavior have not been classified or tested. Inspect each source and its boundaries before adding a state; see the [retained source evidence](mixamo-basic-locomotion-evidence.md#animsmith-remediation-evidence). A zero canonical role count does not establish that these motions are absent.

The legacy count of 10 motions has no verified mapping to these 12 files. Classify the other named motions before adding them to a controller.

### Evaluation still needed

- Loop/phase policy, target-engine graph, contact and visual review, target-character retargeting, performance and cross-pack acceptance remain open.

## Runtime sets and authored motion

| Set | Controller use | Adoption decision | Exact members |
|---|---|---|---|
| `hypothesis/kinematic-walk-3way` | Three-direction unarmed walk; controller owns XZ translation, yaw and collision | The three selected in-place walks have no reported mechanical finding. Prototype with controller-owned movement and project-defined speed; configure loop and phase only after contact review. Four other root-motion-directory states need per-file ownership; idle and left turn also have endpoint warnings. No AnimSmith repair was tested. | [Member timings, coordinates and contracts](mixamo-basic-locomotion-evidence.md#exact-runtime-members) |

The selected files are grouped by name and measurement. AnimSmith measured phase (cycles) as forward `0.4841`, left `0.4905`, and right `0.4587`; their measured phase spread is `0.0318` cycles. These phase numbers do not establish matching foot contacts or a usable blend. Loop intent, contact alignment, and gameplay use remain `not-evaluated`.

## Integration recipe

1. **Members/topology:** Use the exact members and coordinates for `hypothesis/kinematic-walk-3way` in the linked appendix as a proposed directional blend. Add other directions or in-place/root-motion alternatives only after checking their own files.
2. **Timing/synchronization:** Keep source timing and leave loop settings undecided. Compare normalized phase and visible foot contacts in the target engine before enabling a continuous blend.
3. **State ownership:** These selected in-place files supply pose; one kinematic controller supplies XZ translation, yaw, and collision. Root-motion use, where animation supplies travel, requires separate per-file checks.
4. **Composition constraints:** Begin with full-body states and transitions. Masks, additive motion, IK, sockets, and layering need separate tests.
5. **Acceptance gate:** In the chosen engine, test the selected blend at center and axes, rapid input changes, starts and stops, foot contacts, interrupted transitions, and target-character deformation.

## Technical issue register

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| MIX-OWN-001 | moderate | Scope: `Basic_Locomotion_Pack_-_root-motion/idle.fbx`; `Basic_Locomotion_Pack_-_root-motion/jump.fbx`; `Basic_Locomotion_Pack_-_root-motion/left turn 90.fbx`; `Basic_Locomotion_Pack_-_root-motion/right turn 90.fbx`; reproduce: lint each file with the current archive-level animation-owned XZ declaration; impact: applying that policy can leave these states stationary while gameplay expects displacement. Guidance: not applicable. | engine-config | Declare these files controller-owned XZ unless project evidence establishes authored displacement. Accept when per-file declared lint has no ownership error and one target-controller playback applies translation/yaw/collision exactly once. | No generic rewrite is justified because the directory label does not establish per-file intent. | `observed-animsmith`; 4 current findings; no runtime acceptance; unresolved. |
| MIX-TIME-001 | moderate | Scope: `Basic_Locomotion_Pack_-_root-motion/idle.fbx`; `Basic_Locomotion_Pack_-_root-motion/left turn 90.fbx`; reproduce: empty-baseline `duration-sanity`; impact: shorter channels clamp-hold at the end and can create a brief frozen joint or transition/loop pop. [Guidance](../game-ready-clips.md#the-readiness-ladder). | unknown | Inspect the intended endpoint and slowed target-engine boundary playback first. If the hold is intentional and visually acceptable, document and retain it. Otherwise ask the source author to align the shorter channels to the intended endpoint, rerun `duration-sanity`, and review boundary playback. Accept only when the intended hold or corrected boundary is visually approved and the remaining mechanical warning is recorded accurately. | Deterministic endpoint conformance is plausible only after the intended endpoint and protected channels are declared. | `observed-animsmith`; 2 current source files; visual intent not evaluated; unresolved. |
| MIX-CONTENT-001 | note | Scope: `Basic_Locomotion_Pack_-_in-place/X Bot.fbx`; `Basic_Locomotion_Pack_-_root-motion/X Bot.fbx`; reproduce: empty-baseline `duration-sanity` reports `Take 001` with no tracks; impact: automatic clip registration can expose an empty state, although these may be reference-character files. Guidance: not applicable. | unknown | Classify each file as reference-only or intended animation before import. Accept when reference files are excluded from animation registration or the author supplies the intended tracks. | No tool should invent missing motion. | `observed-file` and `observed-animsmith`; semantic purpose unavailable; unresolved. |

## Engine status

| Runtime | Evidence level | Technical result | Remaining gate |
|---|---|---|---|
| Unity unspecified | not-evaluated | No current import, graph, playback, or visual evidence | Test the proposed controller and full-body graph on the target character |
| Unreal Engine unspecified | not-evaluated | No current import, retarget, Blend Space, or playback evidence | Test the exact retarget and controller path |
| Godot unspecified | not-evaluated | No current import, AnimationTree, or playback evidence | Test the exact Skeleton3D and controller path |
| Bevy unspecified | not-evaluated | No current loader, graph, or playback evidence | Test exact animation targets and controller composition |

## Fit and limitations

Best fit is basic unarmed locomotion prototyping where a developer can own movement, define thresholds, and test the exact target character. It is a poor fit for drop-in root-motion, network, motion-matching, layered-animation, or production-ready claims.

Cross-pack compatibility is unknown. Shared Mixamo role resolution and a common 66-bone count on motion-named files do not prove identical hierarchy, rest pose, scale, retargeting, blending, contacts, or style.

## Changes between AnimSmith versions

AnimSmith 0.14.0 — The current evaluation reran 996 inspect, measure, and lint commands across the collection. This constituent admitted 12/12 files; baseline and declared-contract findings were revalidated, and the selected controller set was proposed from current files. Its clip roles remain unverified.

AnimSmith 0.10.0 — Retained historical mechanical counts agree with the current rerun; clip mapping and engine acceptance remain open. AnimSmith 0.7.0 evidence remains superseded.

## Evidence status

The current tests covered 12 FBX source files individually. The legacy metadata lists 10 motions, but its clip-to-file mapping is unverified for the 12 files. The [evidence appendix](mixamo-basic-locomotion-evidence.md) records the source integrity check, evaluator revision, command results, and digests. See the [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder).

## Sources

- Fresh external AnimSmith 0.14.0 command ledger and summary; licensed source and outputs remain external.
- [AnimSmith game-ready clip guidance](../game-ready-clips.md).
