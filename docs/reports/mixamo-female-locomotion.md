# Animation pack evaluation: Mixamo Female Locomotion

> Technical verdict: **Restricted use**
>
> Evaluation completeness: **partial** — all 18 extracted FBX files have current mechanical evidence; vendor mapping, runtime, visual, contact, retarget, license, and cross-pack acceptance remain unavailable.
>
> Confidence: **medium**
>
> Evaluation date: **2026-09-16**
>
> Current evaluator: **AnimSmith 0.14.0**
>
> Report format: **3**
>
> Detailed evidence: [Evidence appendix](mixamo-female-locomotion-evidence.md)

## Technical decision

Developer decision: admit this pack only to an isolated female locomotion prototyping pilot. Use the explicitly new `hypothesis/kinematic-speed` proposal with a kinematic controller owning XZ translation, yaw, and collision; selected members measured at no more than 0.000000 m/s root speed. Do not promote the proposal, loops, phase policy, root-motion use, or shipment until the issue acceptance gates and target-engine blend/contact review pass. No source bytes were changed. [The readiness ladder](../game-ready-clips.md#the-readiness-ladder) remains the boundary.

All 18 delivered FBX files loaded under AnimSmith 0.14.0 and the untouched baseline produced 0 errors, 4 `duration-sanity` warnings, and 2559 non-gating `constant-track` notes. The current archive-level contracts produced 4 errors where files in the root-motion directory remain effectively stationary. That is a movement-ownership decision for the consuming project, not proof of defective animation. Two `X Bot.fbx` files use a 68-bone reference skeleton; the other 16 files use 66 bones, and every file resolves the nine-role Mixamo profile.

## Capability coverage

This is a full-body humanoid controller hypothesis for visible characters. Third-person gameplay-camera appearance and target-character deformation remain visually untested. Dedicated first-person arms/viewmodel content and close-camera suitability were not established.

### Content present

| Candidate ingredients in the evaluated files | Current use boundary |
|---|---|
| Walk and run are named and measured. Strafe walking, idle, jump and turns appear by filename; their controller roles remain unclassified. | Filename-based candidates and current measurements; these do not establish a complete accepted gameplay controller. |

### Content gaps and unknowns

No authoritative 16-motion-to-18-file mapping was established. The remaining motion-named files have no accepted canonical role classification; this does not show that gameplay content is absent.

### Evaluation still needed

- `hypothesis/kinematic-speed` is a new evaluator-selected controller hypothesis over exact current files and measurements. It is not vendor intent, a recovered historical set, or runtime evidence.
- Loop/phase policy, target-engine graph, contact and visual review, target-character retargeting, performance and cross-pack acceptance remain open.

## Runtime sets and authored motion

| Set | Controller use | Adoption decision | Exact members |
|---|---|---|---|
| `hypothesis/kinematic-speed` | Walk/run speed blend; controller owns XZ translation, yaw and collision | The selected walk/run pair has no reported source finding. Prototype with controller-owned movement and project-defined speed thresholds. Four other root-motion-directory states need per-file ownership; left strafe walk has a source endpoint warning. No AnimSmith repair was tried. | [Member timings, coordinates and contracts](mixamo-female-locomotion-evidence.md#exact-runtime-members) |

This set is a current evaluator hypothesis based on exact filenames plus measurements. AnimSmith measured phase (cycles) as walk `0.4913` and run `0.4289`; their normalized circular-range spread is `0.0624` cycles (`1 - largest cyclic gap` across member phases on the unit cycle). This is mechanical evidence, not a synchronization verdict: loop intent, foot-contact correspondence, phase acceptance, and gameplay use remain `not-evaluated`; `sync=not-evaluated`.

## Integration recipe

1. **Members/topology:** `topology=speed-blend`; use only the exact `hypothesis/kinematic-speed` members and coordinates in the linked appendix; do not infer missing directions or IP/RM pairs.
2. **Timing/synchronization:** `sync=not-evaluated`; keep each measured duration and phase as evidence, leave loop flags unknown, and test normalized phase plus foot contacts before enabling continuous blends.
3. **State ownership:** `owner=controller-xz-yaw-collision`; animation supplies pose while the kinematic controller applies translation, yaw, and collision once.
4. **Composition constraints:** `composition=full-body-only`; use full-body states and transitions; no mask, additive, IK, socket, or layered result is established.
5. **Acceptance gate:** `gate=engine-visual-contact`; test exact members at center/axes, rapid input changes, starts/stops, foot contacts, transition interruption, and target-character deformation in the chosen engine.

## Technical issue register

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| MIX-OWN-001 | moderate | Scope: `Female_Locomotion_Pack_-_root-motion/idle.fbx`; `Female_Locomotion_Pack_-_root-motion/jump.fbx`; `Female_Locomotion_Pack_-_root-motion/left turn.fbx`; `Female_Locomotion_Pack_-_root-motion/right turn.fbx`; reproduce: lint each file with the current archive-level animation-owned XZ declaration; impact: applying that policy can leave these states stationary while gameplay expects displacement. Guidance: not applicable. | engine-config | Action: declare these files controller-owned XZ unless project evidence establishes authored displacement; acceptance: per-file declared lint has no ownership error and one target-controller playback applies translation/yaw/collision exactly once; residual: unresolved. | No generic rewrite is justified because the directory label does not establish per-file intent. | `observed-animsmith`; 4 current findings; no runtime acceptance. |
| MIX-TIME-001 | moderate | Scope: `Female_Locomotion_Pack_-_in-place/left strafe walk.fbx`; `Female_Locomotion_Pack_-_root-motion/left strafe walk.fbx`; reproduce: empty-baseline `duration-sanity`; impact: shorter channels clamp-hold at the end and can create a brief frozen joint or transition/loop pop. [Guidance](../game-ready-clips.md#the-readiness-ladder). | artist-author | Action: align shorter keyed channels to the intended endpoint without changing the intended terminal pose or trajectory; acceptance: `duration-sanity` is clean and slowed target-engine boundary playback shows no hold or pop; residual: unresolved. | Deterministic endpoint conformance is plausible only after the intended endpoint and protected channels are declared. | `observed-animsmith`; 2 current source files; visual intent not evaluated. |
| MIX-CONTENT-001 | note | Scope: `Female_Locomotion_Pack_-_in-place/X Bot.fbx`; `Female_Locomotion_Pack_-_root-motion/X Bot.fbx`; reproduce: empty-baseline `duration-sanity` reports `Take 001` with no tracks; impact: automatic clip registration can expose an empty state, although these may be reference-character files. Guidance: not applicable. | unknown | Action: classify each file as reference-only or intended animation before import; acceptance: reference files are excluded from animation registration or the author supplies the intended tracks; residual: unresolved. | No tool should invent missing motion. | `observed-file` and `observed-animsmith`; semantic purpose unavailable. |

## Engine status

| Runtime | Evidence level | Technical result | Remaining gate |
|---|---|---|---|
| Unity unspecified | not-evaluated | No current import, graph, playback, or visual evidence | Test the proposed controller and full-body graph on the target character |
| Unreal Engine unspecified | not-evaluated | No current import, retarget, Blend Space, or playback evidence | Test the exact retarget and controller path |
| Godot unspecified | not-evaluated | No current import, AnimationTree, or playback evidence | Test the exact Skeleton3D and controller path |
| Bevy unspecified | not-evaluated | No current loader, graph, or playback evidence | Test exact animation targets and controller composition |

## Fit and limitations

Best fit is female locomotion prototyping where a developer can own movement, define thresholds, and test the exact target character. It is a poor fit for drop-in root-motion, network, motion-matching, layered-animation, or production-ready claims.

Cross-pack compatibility is unknown. Shared Mixamo role resolution and a common 66-bone count on motion-named files do not prove identical hierarchy, rest pose, scale, retargeting, blending, contacts, or style.

## Changes between AnimSmith versions

AnimSmith 0.14.0 — Freshly hashed source reran 996 collection-wide inspect/measure/lint commands with four workers. This constituent admitted 18/18 files; baseline and declared-contract finding counts were revalidated, and a new explicitly evaluator-owned controller hypothesis was added without recovering old semantic authority.

AnimSmith 0.10.0 — Retained historical mechanical counts agree with the current rerun but do not supply current runtime sets, source mappings, or engine acceptance. AnimSmith 0.7.0 evidence remains superseded.

## Evidence status

Current evidence covers 18 physical FBX files as opaque source units under evaluator revision `e8321ad40be5ef6f162b31f085819c039175c3c9`. The legacy manifest metadata says 16 motions but does not map them authoritatively to the 18 files. The immutable source inventory matched before and after evaluation; current external ledger and summary digests are recorded in the appendix. See the [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder).

## Sources

- Fresh external AnimSmith 0.14.0 command ledger and summary; licensed source and outputs remain external.
- [AnimSmith game-ready clip guidance](../game-ready-clips.md).
