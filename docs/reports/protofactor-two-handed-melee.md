# Animation pack evaluation: Protofactor Two-Handed Melee Animset

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
> Detailed evidence: [Protofactor Two-Handed Melee Animset evidence appendix](protofactor-two-handed-melee-evidence.md)

## Technical decision

AnimSmith 0.14.0 reads all 123 delivered FBX candidates. The untouched baseline has no errors; it reports 17010 constant-track notes, one duration warning. The retained per-file declarations are stricter: 13/120 motion files pass and 107 fail, chiefly because loop treatment is unresolved. Named one-shots must be re-declared before their seam failures are attributed to the source. Intended continuous gait files still need source or loop-policy correction. One heavy-hit RM file has unequal channel end times.

All 25 fresh transform candidates were produced outside the repository and passed inspect/measure. All candidates remain non-clean under their retained configs. None is adopted, so residual severity remains unresolved. Developer decision: run a bounded full-body target-engine pilot only after correcting one-shot loop declarations and obtaining clean contracts for one named gait set; require blend, foot-contact, root-owner, and visual acceptance before production use.

## Capability coverage

### Complete core

- No complete gameplay core has been validated on a target game controller. Candidate content and integration scope are listed below; completed file checks do not establish gameplay completeness.

### Partial supporting gameplay

- Directional IP/RM gait families, equipment transitions, and attacks are present; current set membership is a fresh evaluator-defined hypothesis and every named gait set remains contract-non-clean.
- Airborne content is present but incomplete for a full traversal graph; additive aim, paired interactions, and first-person use are not established.

### Absent

- Unity 6000.5.8f1 has bounded source/controller feasibility for `IdleCombatA2HandMelee`, `WalkForwardCombat2HandMelee`, and `WalkForwardCombat2HandMelee_RM` and the named Playables transitions; visual/contact, authored-controller, deformation, player-build, retarget, mask/layer, performance, network movement, and artistic evaluation remain absent.

## Runtime sets and authored motion

These current hypotheses use exact source identities and fresh 0.14.0 measurements. They do not restore any unavailable historical membership authority.

| Set/profile | Role or coordinate | Exact members | Variant/type | Timing or motion | Runtime contract |
|---|---|---|---|---|---|
| `crouch-combat-8-way` | eight-way directional gait | `Humanoid@CrouchForward2HandMelee.fbx`; `Humanoid@CrouchForward2HandMelee_RM.fbx`; `Humanoid@CrouchForwardLeft2HandMelee.fbx`; `Humanoid@CrouchForwardLeft2HandMelee_RM.fbx`; `Humanoid@CrouchLeft2HandMelee.fbx`; `Humanoid@CrouchLeft2HandMelee_RM.fbx`; `Humanoid@CrouchBackwardsLeft2HandMelee.fbx`; `Humanoid@CrouchBackwardsLeft2HandMelee_RM.fbx`; `Humanoid@CrouchBackwards2HandMelee.fbx`; `Humanoid@CrouchBackwards2HandMelee_RM.fbx`; `Humanoid@CrouchBackwardsRight2HandMelee.fbx`; `Humanoid@CrouchBackwardsRight2HandMelee_RM.fbx`; `Humanoid@CrouchRight2HandMelee.fbx`; `Humanoid@CrouchRight2HandMelee_RM.fbx`; `Humanoid@CrouchForwardRight2HandMelee.fbx`; `Humanoid@CrouchForwardRight2HandMelee_RM.fbx` | set_type=directional-blend | duration=1.667 s; rm_speed=0.642 m/s | loop_ip=true; loop_rm=true; sync=not-evaluated |
| `run-combat-8-way` | eight-way directional gait | `Humanoid@RunForwardCombat2HandMelee.fbx`; `Humanoid@RunForwardCombat2HandMelee_RM.fbx`; `Humanoid@RunForwardLeftCombat2HandMelee.fbx`; `Humanoid@RunForwardLeftCombat2HandMelee_RM.fbx`; `Humanoid@RunLeftCombat2HandMelee.fbx`; `Humanoid@RunLeftCombat2HandMelee_RM.fbx`; `Humanoid@RunBackwardsLeftCombat2HandMelee.fbx`; `Humanoid@RunBackwardsLeftCombat2HandMelee_RM.fbx`; `Humanoid@RunBackwardsCombat2HandMelee.fbx`; `Humanoid@RunBackwardsCombat2HandMelee_RM.fbx`; `Humanoid@RunBackwardsRightCombat2HandMelee.fbx`; `Humanoid@RunBackwardsRightCombat2HandMelee_RM.fbx`; `Humanoid@RunRightCombat2HandMelee.fbx`; `Humanoid@RunRightCombat2HandMelee_RM.fbx`; `Humanoid@RunForwardRightCombat2HandMelee.fbx`; `Humanoid@RunForwardRightCombat2HandMelee_RM.fbx` | set_type=directional-blend | duration=0.533 s; rm_speed=2.202 m/s | loop_ip=true; loop_rm=true; sync=not-evaluated |
| `walk-combat-8-way` | eight-way directional gait | `Humanoid@WalkForwardCombat2HandMelee.fbx`; `Humanoid@WalkForwardCombat2HandMelee_RM.fbx`; `Humanoid@WalkForwardLeftCombat2HandMelee.fbx`; `Humanoid@WalkForwardLeftCombat2HandMelee_RM.fbx`; `Humanoid@WalkLeftCombat2HandMelee.fbx`; `Humanoid@WalkLeftCombat2HandMelee_RM.fbx`; `Humanoid@WalkBackwardsLeftCombat2HandMelee.fbx`; `Humanoid@WalkBackwardsLeftCombat2HandMelee_RM.fbx`; `Humanoid@WalkBackwardsCombat2HandMelee.fbx`; `Humanoid@WalkBackwardsCombat2HandMelee_RM.fbx`; `Humanoid@WalkBackwardsRightCombat2HandMelee.fbx`; `Humanoid@WalkBackwardsRightCombat2HandMelee_RM.fbx`; `Humanoid@WalkRightCombat2HandMelee.fbx`; `Humanoid@WalkRightCombat2HandMelee_RM.fbx`; `Humanoid@WalkForwardRightCombat2HandMelee.fbx`; `Humanoid@WalkForwardRightCombat2HandMelee_RM.fbx` | set_type=directional-blend | duration=1.333 s; rm_speed=0.810 m/s | loop_ip=true; loop_rm=true; sync=not-evaluated |

The grouped table gives minimum duration and RM speed. Run durations span 0.533–0.567 s. Measured RM speed spans are `crouch-combat-8-way` 0.642–0.835 m/s (ratio 0.769); `run-combat-8-way` 2.202–2.690 m/s (ratio 0.819); `walk-combat-8-way` 0.810–1.092 m/s (ratio 0.742). Preserve authored variation unless the project declares a normalization policy; diagonals and cardinals require the full blend test. Measured source in-place phase spreads (cycles, eight measured members each) are `crouch-combat-8-way` 0.5774; `run-combat-8-way` 0.6024; `walk-combat-8-way` 0.7112. Phase spread here is the minimum covering arc in cycles: sort phases in `[0,1)`, include the wraparound gap, then subtract the largest gap from 1. It is not `max_circular_deviation_from_mean`; neither measure alone proves support-foot or visual compatibility. Keep `sync=not-evaluated` until the selected synchronization and contact policy is tested. The current catalog also measures `normal-forward-speed`, `draw-combat-put-away`, `dodge-forward-back`, and `parry-3-way` as evaluator-defined hypotheses; they remain candidate groupings.

## Integration recipe

1. **Members/topology:** `topology=eight-way-cartesian`; use only the exact members in one named gait row and map forward/cardinal/diagonal coordinates explicitly.
2. **Timing/synchronization:** `sync=not-evaluated`; loop only confirmed continuous states, keep named one-shots at `loop=false`, and require phase/contact review before synchronized blending.
3. **State ownership:** `owner=project-controller`; choose IP controller translation or RM animation translation per state, with one owner for translation, yaw, and collision.
4. **Composition constraints:** `composition=full-body-handoff`; keep weapon combat states namespaced to this pack and require separate mask/socket/IK evidence before layering.
5. **Acceptance gate:** `gate=engine-visual-contact`; require clean intended contracts plus target-engine import, full blend-space playback, foot/weapon contact, and artist review.

## Technical issue register

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| PF2-01 | major | Scope: `Humanoid@RunForwardCombat2HandMelee.fbx`, `Humanoid@RunForwardCombat2HandMelee_RM.fbx`, and all members of `run-combat-8-way`; reproduce: lint each exact file with its retained per-file config; impact: every named gait hypothesis has 0 contract-clean members and cannot be admitted as a production blend set. [Guidance](../game-ready-clips.md#the-readiness-ladder) | artist-author | Action: re-export intended continuous cycles with matching pose and velocity boundaries, or document a non-loop policy; acceptance: all exact members pass the intended loop contract and a full blend-space visual/contact review; residual: unresolved | AnimSmith can validate and compare candidates, but cannot infer intended contact phase or approve visible motion. | `observed-animsmith`; current exact-file evidence, high confidence mechanically |
| PF2-02 | moderate | Scope: `Humanoid@AttackA2HandMelee.fbx` and other attacks, reactions, parries, or emotes currently declared as loops; reproduce: run the retained config and observe loop seam errors; impact: a project declaration can reject valid one-shots or cause unintended replay seams. [Guidance](../game-ready-clips.md#the-readiness-ladder) | engine-config | Action: set `loop=false` for confirmed one-shots and retain looping only for vendor/project-authorized continuous states; acceptance: revised declarations lint clean and state transitions play once; residual: unresolved | A future classifier may suggest intent, but project or vendor authority remains required. | `observed-animsmith`; declaration defect distinguished from source-animation quality |
| PF2-03 | moderate | Scope: `Humanoid@GetHitLeftHeavy2HandMelee_RM.fbx`, channels ending at 1.700–1.733 s; reproduce: baseline lint with the retained baseline config; impact: shorter channels clamp-hold for the final frame interval. [Guidance](../game-ready-clips.md#the-readiness-ladder) | artist-author | Action: align channel endpoints or document the hold; acceptance: duration-sanity passes and the heavy-hit contact/recovery is visually approved; residual: unresolved | Current lint detects the mismatch; safe semantic repair still needs author intent. | `observed-animsmith`; one warning plus six time-monotonic notes |

## Engine status

| Runtime | Evidence level | Technical result | Remaining gate |
|---|---|---|---|
| Unity 6000.5.8f1 | source-controller-feasibility | Fresh import and standalone sampling passed for `IdleCombatA2HandMelee`, `WalkForwardCombat2HandMelee`, and `WalkForwardCombat2HandMelee_RM`; `basic-idle-to-twohand-idle`, `sword-shield-idle-to-twohand-idle`, and `basic-walk-ip-to-twohand-walk-ip` produced finite mixer poses with fixed gameplay-owned root; the RM walk produced finite 1.159724 owner translation with animation-owned root. | Inspect rendered motion, contacts, deformation, authored AnimatorController behavior, and a player build; do not generalize beyond named sources. |
| Unreal Engine unspecified | not-evaluated | No current import or retarget result. | Import, retarget, root-motion, blend, and visual tests. |
| Godot unspecified | not-evaluated | No current conversion or playback result. | Convert/import and test animation graph, roots, contacts, and visuals. |
| Bevy unspecified | not-evaluated | No current conversion or playback result. | Convert/load and test graph, root ownership, performance, and visuals. |

## Fit and limitations

Best fit is a full-body combat prototype whose controller can explicitly select one gait variant and keep pack-local state names. It is a poor fit for immediate production admission, motion matching, layered combat, or retargeted characters without the missing engine and human gates.

The fresh Unity probe establishes finite source playback and the named cross-pack transitions for `IdleCombatA2HandMelee`, `WalkForwardCombat2HandMelee`, and `WalkForwardCombat2HandMelee_RM`; it does not establish visual quality, contact, phase, retargeting, or broad pack compatibility. Keep states namespaced and retain explicit root ownership.

## Changes between AnimSmith versions

AnimSmith 0.14.0 — revalidated the unchanged retained source inventory, per-file declarations, measured runtime hypotheses, and every previously recommended bounded transform. Current candidates remain unadopted. AnimSmith 0.10.0 — retained command ledgers and classification material supplied replay controls; its conclusions are superseded for current behavior.

## Evidence status

Current evidence covers 123 physical FBXs and 120 logical individual-motion files with official AnimSmith 0.14.0, report format 2, and the retained evaluation-manifest schema. A bounded Unity source/controller probe is current; visual/artistic, contact, retarget, authored-controller, and player-build gates remain open. See the [readiness ladder](../game-ready-clips.md#the-readiness-ladder) and the [evidence appendix](protofactor-two-handed-melee-evidence.md). Licensed source and generated candidates remain outside Git.

## Sources

- Protofactor product context and [End User License Agreement](https://protofactor.biz/end-user-license-agreement/) — vendor context only; it does not identify the local constituent revision.
