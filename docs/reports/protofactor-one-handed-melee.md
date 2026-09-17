# Animation pack evaluation: Protofactor One-Handed Melee Animset

> Technical verdict: **Usable with conditions**
>
> Evaluation completeness: **partial** — current mechanical, declared-contract, runtime-group, and remediation checks are complete, with bounded Unity source/mixer probes; visual, contact, retargeting, and target game-controller acceptance remain open.
>
> Confidence: **medium**
>
> Evaluation date: **2026-09-16**
>
> Current evaluator: **AnimSmith 0.14.0**
>
> Report format: **2**
>
> Detailed evidence: [Protofactor One-Handed Melee Animset evidence appendix](protofactor-one-handed-melee-evidence.md)

## Technical decision

AnimSmith 0.14.0 reads all 113 delivered FBX candidates. The untouched baseline has no errors; it reports 13629 constant-track notes. The retained per-file declarations are stricter: 23/110 motion files pass and 87 fail, chiefly because loop treatment is unresolved. Named one-shots must be re-declared before their seam failures are attributed to the source. Intended continuous gait files still need source or loop-policy correction.

All 25 fresh transform candidates were produced outside the repository and passed inspect/measure. All candidates remain non-clean under their retained configs. None is adopted, so residual severity remains unresolved. Developer decision: run a bounded full-body target-engine pilot only after correcting one-shot loop declarations and obtaining clean contracts for one named gait set; require blend, foot-contact, root-owner, and visual acceptance before production use.

## Capability coverage

### Complete core

- No complete gameplay core has been validated on a target game controller. Candidate content and integration scope are listed below; completed file checks do not establish gameplay completeness.

### Partial supporting gameplay

- Directional IP/RM gait families, equipment transitions, and attacks are present; current set membership is a fresh evaluator-defined hypothesis and every named gait set remains contract-non-clean.
- Airborne content is present but incomplete for a full traversal graph; additive aim, paired interactions, and first-person use are not established.

### Absent

- Unity 6000.5.8f1 has bounded source/controller feasibility for `IdleCombat1hMelee`, `WalkForwardCombat1hMelee`, and `WalkForwardCombat1hMelee_RM` and the named Playables transitions; visual/contact, authored-controller, deformation, player-build, retarget, mask/layer, performance, network movement, and artistic evaluation remain absent.

## Runtime sets and authored motion

These current hypotheses use exact source identities and fresh 0.14.0 measurements. They do not restore any unavailable historical membership authority.

| Set/profile | Role or coordinate | Exact members | Variant/type | Timing or motion | Runtime contract |
|---|---|---|---|---|---|
| `crouch-combat-8-way` | eight-way directional gait | `Humanoid@CrouchForward1hMelee.fbx`; `Humanoid@CrouchForward1hMelee_RM.fbx`; `Humanoid@CrouchForwardLeft1hMelee.fbx`; `Humanoid@CrouchForwardLeft1hMelee_RM.fbx`; `Humanoid@CrouchLeft1hMelee.fbx`; `Humanoid@CrouchLeft1hMelee_RM.fbx`; `Humanoid@CrouchBackwardsLeft1hMelee.fbx`; `Humanoid@CrouchBackwardsLeft1hMelee_RM.fbx`; `Humanoid@CrouchBackwards1hMelee.fbx`; `Humanoid@CrouchBackwards1hMelee_RM.fbx`; `Humanoid@CrouchBackwardsRight1hMelee.fbx`; `Humanoid@CrouchBackwardsRight1hMelee_RM.fbx`; `Humanoid@CrouchRight1hMelee.fbx`; `Humanoid@CrouchRight1hMelee_RM.fbx`; `Humanoid@CrouchForwardRight1hMelee.fbx`; `Humanoid@CrouchForwardRight1hMelee_RM.fbx` | set_type=directional-blend | duration=1.667 s; rm_speed=0.480 m/s | loop_ip=true; loop_rm=true; sync=not-evaluated |
| `run-8-way` | eight-way directional gait | `Humanoid@RunForward1hMelee.fbx`; `Humanoid@RunForward1hMelee_RM.fbx`; `Humanoid@RunForwardLeft1hMelee.fbx`; `Humanoid@RunForwardLeft1hMelee_RM.fbx`; `Humanoid@RunLeft1hMelee.fbx`; `Humanoid@RunLeft1hMelee_RM.fbx`; `Humanoid@RunBackwardsLeft1hMelee.fbx`; `Humanoid@RunBackwardsLeft1hMelee_RM.fbx`; `Humanoid@RunBackwards1hMelee.fbx`; `Humanoid@RunBackwards1hMelee_RM.fbx`; `Humanoid@RunBackwardsRight1hMelee.fbx`; `Humanoid@RunBackwardsRight1hMelee_RM.fbx`; `Humanoid@RunRight1hMelee.fbx`; `Humanoid@RunRight1hMelee_RM.fbx`; `Humanoid@RunForwardRight1hMelee.fbx`; `Humanoid@RunForwardRight1hMelee_RM.fbx` | set_type=directional-blend | duration=0.600 s; rm_speed=1.905 m/s | loop_ip=true; loop_rm=true; sync=not-evaluated |
| `walk-combat-8-way` | eight-way directional gait | `Humanoid@WalkForwardCombat1hMelee.fbx`; `Humanoid@WalkForwardCombat1hMelee_RM.fbx`; `Humanoid@WalkForwardLeftCombat1hMelee.fbx`; `Humanoid@WalkForwardLeftCombat1hMelee_RM.fbx`; `Humanoid@WalkLeftCombat1hMelee.fbx`; `Humanoid@WalkLeftCombat1hMelee_RM.fbx`; `Humanoid@WalkBackwardsLeftCombat1hMelee.fbx`; `Humanoid@WalkBackwardsLeftCombat1hMelee_RM.fbx`; `Humanoid@WalkBackwardsCombat1hMelee.fbx`; `Humanoid@WalkBackwardsCombat1hMelee_RM.fbx`; `Humanoid@WalkBackwardsRightCombat1hMelee.fbx`; `Humanoid@WalkBackwardsRightCombat1hMelee_RM.fbx`; `Humanoid@WalkRightCombat1hMelee.fbx`; `Humanoid@WalkRightCombat1hMelee_RM.fbx`; `Humanoid@WalkForwardRightCombat1hMelee.fbx`; `Humanoid@WalkForwardRightCombat1hMelee_RM.fbx` | set_type=directional-blend | duration=1.333 s; rm_speed=0.491 m/s | loop_ip=true; loop_rm=true; sync=not-evaluated |

The grouped table gives minimum duration and RM speed. Run durations span 0.600–0.667 s. Measured RM speed spans are `crouch-combat-8-way` 0.480–0.730 m/s (ratio 0.657); `run-8-way` 1.905–2.117 m/s (ratio 0.900); `walk-combat-8-way` 0.491–0.951 m/s (ratio 0.516). Preserve authored variation unless the project declares a normalization policy; diagonals and cardinals require the full blend test. Measured source in-place phase spreads (cycles, eight measured members each) are `crouch-combat-8-way` 0.7136; `run-8-way` 0.7342; `walk-combat-8-way` 0.5538. Phase spread here is the minimum covering arc in cycles: sort phases in `[0,1)`, include the wraparound gap, then subtract the largest gap from 1. It is not `max_circular_deviation_from_mean`; neither measure alone proves support-foot or visual compatibility. Keep `sync=not-evaluated` until the selected synchronization and contact policy is tested. The current catalog also measures `hold-forward-speed`, `draw-combat-put-away`, and `heavy-hit-4-way` as evaluator-defined hypotheses; they remain candidate groupings.

## Integration recipe

1. **Members/topology:** `topology=eight-way-cartesian`; use only the exact members in one named gait row and map forward/cardinal/diagonal coordinates explicitly.
2. **Timing/synchronization:** `sync=not-evaluated`; loop only confirmed continuous states, keep named one-shots at `loop=false`, and require phase/contact review before synchronized blending.
3. **State ownership:** `owner=project-controller`; choose IP controller translation or RM animation translation per state, with one owner for translation, yaw, and collision.
4. **Composition constraints:** `composition=full-body-handoff`; keep weapon combat states namespaced to this pack and require separate mask/socket/IK evidence before layering.
5. **Acceptance gate:** `gate=engine-visual-contact`; require clean intended contracts plus target-engine import, full blend-space playback, foot/weapon contact, and artist review.

## Technical issue register

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| PF1-01 | major | Scope: `Humanoid@RunForward1hMelee.fbx`, `Humanoid@RunForward1hMelee_RM.fbx`, and all members of `run-8-way`; reproduce: lint each exact file with its retained per-file config; impact: every named gait hypothesis has 0 contract-clean members and cannot be admitted as a production blend set. [Guidance](../game-ready-clips.md#the-readiness-ladder) | artist-author | Action: re-export intended continuous cycles with matching pose and velocity boundaries, or document a non-loop policy; acceptance: all exact members pass the intended loop contract and a full blend-space visual/contact review; residual: unresolved | AnimSmith can validate and compare candidates, but cannot infer intended contact phase or approve visible motion. | `observed-animsmith`; current exact-file evidence, high confidence mechanically |
| PF1-02 | moderate | Scope: `Humanoid@AttackA1hMelee.fbx` and other attacks, reactions, parries, or emotes currently declared as loops; reproduce: run the retained config and observe loop seam errors; impact: a project declaration can reject valid one-shots or cause unintended replay seams. [Guidance](../game-ready-clips.md#the-readiness-ladder) | engine-config | Action: set `loop=false` for confirmed one-shots and retain looping only for vendor/project-authorized continuous states; acceptance: revised declarations lint clean and state transitions play once; residual: unresolved | A future classifier may suggest intent, but project or vendor authority remains required. | `observed-animsmith`; declaration defect distinguished from source-animation quality |

## Engine status

| Runtime | Evidence level | Technical result | Remaining gate |
|---|---|---|---|
| Unity 6000.5.8f1 | source-controller-feasibility | Fresh import and standalone sampling passed for `IdleCombat1hMelee`, `WalkForwardCombat1hMelee`, and `WalkForwardCombat1hMelee_RM`; `basic-idle-to-onehand-idle`, `sword-shield-idle-to-onehand-idle`, and `basic-walk-ip-to-onehand-walk-ip` produced finite mixer poses with fixed gameplay-owned root; the RM walk produced finite 1.215074 owner translation with animation-owned root. | Inspect rendered motion, contacts, deformation, authored AnimatorController behavior, and a player build; do not generalize beyond named sources. |
| Unreal Engine unspecified | not-evaluated | No current import or retarget result. | Import, retarget, root-motion, blend, and visual tests. |
| Godot unspecified | not-evaluated | No current conversion or playback result. | Convert/import and test animation graph, roots, contacts, and visuals. |
| Bevy unspecified | not-evaluated | No current conversion or playback result. | Convert/load and test graph, root ownership, performance, and visuals. |

## Fit and limitations

Best fit is a full-body combat prototype whose controller can explicitly select one gait variant and keep pack-local state names. It is a poor fit for immediate production admission, motion matching, layered combat, or retargeted characters without the missing engine and human gates.

The fresh Unity probe establishes finite source playback and the named cross-pack transitions for `IdleCombat1hMelee`, `WalkForwardCombat1hMelee`, and `WalkForwardCombat1hMelee_RM`; it does not establish visual quality, contact, phase, retargeting, or broad pack compatibility. Keep states namespaced and retain explicit root ownership.

## Changes between AnimSmith versions

AnimSmith 0.14.0 — revalidated the unchanged retained source inventory, per-file declarations, measured runtime hypotheses, and every previously recommended bounded transform. Current candidates remain unadopted. AnimSmith 0.10.0 — retained command ledgers and classification material supplied replay controls; its conclusions are superseded for current behavior.

## Evidence status

Current evidence covers 113 physical FBXs and 110 logical individual-motion files with official AnimSmith 0.14.0, report format 2, and the retained evaluation-manifest schema. A bounded Unity source/controller probe is current; visual/artistic, contact, retarget, authored-controller, and player-build gates remain open. See the [readiness ladder](../game-ready-clips.md#the-readiness-ladder) and the [evidence appendix](protofactor-one-handed-melee-evidence.md). Licensed source and generated candidates remain outside Git.

## Sources

- Protofactor product context and [End User License Agreement](https://protofactor.biz/end-user-license-agreement/) — vendor context only; it does not identify the local constituent revision.
