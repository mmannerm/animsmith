# Animation pack evaluation: Protofactor Campfire

> Technical verdict: **Usable with conditions**
>
> Evaluation completeness: **partial** — all current file and contract checks ran; no current engine or visual acceptance ran.
>
> Confidence: **medium**
>
> Evaluation date: **2026-09-16**
>
> Current evaluator: **AnimSmith 0.14.0**
>
> Report format: **2**
>
> Detailed evidence: [Protofactor Campfire evidence](protofactor-campfire-evidence.md)

## Technical decision

**Use as a full-body interaction state family, not a locomotion blend tree.** The candidate kneel/sit/lie and grill sequences give a developer concrete states to prototype. The game still owns interaction anchors, interruption rules and props. A one-shot failing a deliberately looping contract is a policy mismatch, not automatically an artist defect.

`observed-animsmith`: the official 0.14.0 evaluator inspected and measured all 29 FBXs; 17/25 declared-contract files pass and 8 fail per output format. Conditions below distinguish source findings, evaluator policy and untested gameplay. No remediation candidate is promoted.

## Capability coverage

### Complete core

- No complete gameplay core has been validated on a target game controller. Candidate content and integration scope are listed below; completed file checks do not establish gameplay completeness.

### Partial supporting gameplay

- Selected kneel/sit/lie and grill sequences provide interaction-state candidates. Prop attachment, entry/exit continuity, interruption and contact behavior remain open.

### Absent

- No current locomotion, masking, IK, retarget, or engine acceptance result exists.

## Runtime sets and authored motion

New evaluator-selected generic scenarios: current source bytes establish the exact members, take names, durations, and measured roots below. Names suggest gameplay roles; topology and semantic intent are hypotheses requiring clip review. These are not reconstructed historical manifests or measured collection-output sets. Every listed member uses `Take 001`; full identities are in the external selected-set ledger.

| Set/profile | Role or coordinate | Exact members | Variant/type | Timing or motion | Runtime contract |
|---|---|---|---|---|---|
| Kneel and sit sequence | Candidate member 1 | `Humanoid@StandToKneelCampfire.fbx::Take 001` | set_type=transition-chain | duration=4.500 s | loop=unknown; movement=controller; contact=not-evaluated |
| Kneel and sit sequence | Candidate member 2 | `Humanoid@IdleKneelCampfire.fbx::Take 001` | set_type=transition-chain | duration=2.167 s | loop=unknown; movement=controller; contact=not-evaluated |
| Kneel and sit sequence | Candidate member 3 | `Humanoid@KneelToSitCampfire.fbx::Take 001` | set_type=transition-chain | duration=4.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Kneel and sit sequence | Candidate member 4 | `Humanoid@IdleSitCampfire.fbx::Take 001` | set_type=transition-chain | duration=2.667 s | loop=unknown; movement=controller; contact=not-evaluated |
| Sit and lie sequence | Candidate member 1 | `Humanoid@IdleSitCampfire.fbx::Take 001` | set_type=transition-chain | duration=2.667 s | loop=unknown; movement=controller; contact=not-evaluated |
| Sit and lie sequence | Candidate member 2 | `Humanoid@IdleSitToIdleLayDownCampfire.fbx::Take 001` | set_type=transition-chain | duration=4.000 s | loop=unknown; movement=controller; contact=not-evaluated |
| Sit and lie sequence | Candidate member 3 | `Humanoid@IdleLayDownCampfire.fbx::Take 001` | set_type=transition-chain | duration=1.967 s | loop=unknown; movement=controller; contact=not-evaluated |
| Sit and lie sequence | Candidate member 4 | `Humanoid@IdleLayDownToIdleSitCampfire.fbx::Take 001` | set_type=transition-chain | duration=2.667 s | loop=unknown; movement=controller; contact=not-evaluated |
| Grill interaction sequence | Candidate member 1 | `Humanoid@IdleKneelCampfire.fbx::Take 001` | set_type=transition-chain | duration=2.167 s | loop=unknown; movement=controller; contact=not-evaluated |
| Grill interaction sequence | Candidate member 2 | `Humanoid@IdleKneelToIdleGrillSkewerCampfire.fbx::Take 001` | set_type=transition-chain | duration=3.667 s | loop=unknown; movement=controller; contact=not-evaluated |
| Grill interaction sequence | Candidate member 3 | `Humanoid@IdleGrillSkewerCampfire.fbx::Take 001` | set_type=transition-chain | duration=3.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Grill interaction sequence | Candidate member 4 | `Humanoid@KneelEatSkewerCampfire.fbx::Take 001` | set_type=transition-chain | duration=11.667 s | loop=unknown; movement=controller; contact=not-evaluated |

## Integration recipe

1. **Members/topology:** `topology=transition-chain`; choose the kneel/sit or sit/lie chain below. The listed chains cover ingress/hold only. Before returning to ground movement, select/review `Humanoid@IdleSitToIdleKneelCampfire.fbx` then `Humanoid@KneelToStandCampfire.fbx` (both `Take 001`); exit continuity is untested. Trigger one-shots once; fire-lighting alternatives are discrete choices.
2. **Timing/synchronization:** `sync=state-transition`; verify exit/entry poses and event points for reaching, eating and log release. Do not normalize every action to the idle duration.
3. **State ownership:** `owner=controller`; enter at an authored interaction anchor, stop ground locomotion, and define how interruption restores control. Inspect root/hip offsets before deciding whether an action may displace the capsule.
4. **Composition constraints:** `composition=full-body`; validate skewer/log attachment and hand placement against the scene. Upper-body masking may break posture and is not established here.
5. **Acceptance gate:** `gate=engine-and-visual-review`; play the whole interaction including approach, interrupt and exit, with the intended campfire and seating geometry.

## Technical issue register

Severity describes impact as delivered for the stated use. Residual status is explicit; untested risks are not confirmed artist defects.

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| CF-LOOP | major | The selected idle candidate `Humanoid@IdleKneelCampfire.fbx` has a rotational seam; repeated kneeling may pulse. [Readiness guidance](../game-ready-clips.md#the-readiness-ladder). | artist-author | For genuine resting loops, review endpoint pose/velocity and re-export clean cycles. Pruning IdleKneel does not remove its seam. Residual: major until accepted looping playback. | Current measurements locate discontinuities; pruning is not a loop repair. | Exact `Take 001`, loop-seam-rot in current contract/output; `observed-animsmith`. |
| CF-POLICY | minor | Lighting and transition actions are also tested as loops; wrap failures do not establish defects when those actions are used once. [Readiness guidance](../game-ready-clips.md#the-readiness-ladder). | engine-config | Mark reviewed one-shots non-looping and test their intended start/end transitions. Residual: unresolved configuration choice, not proven bad animation. | Explicit per-clip declarations already support this distinction. | For example `Humanoid@IdleKneelToIdleGrillSkewerCampfire.fbx`; `observed-animsmith`, role `inferred`. |
| CF-PROP | major | Hand/prop placement and release timing have no current contact test; visible penetration or floating objects would block a finished interaction. [Readiness guidance](../game-ready-clips.md#the-readiness-ladder). | engine-config | Set attachment/interaction anchors and validate grill/eat/log events on the chosen character. Ask the artist for authored contact points only if this reveals missing or inconsistent motion; residual unknown. | Generic contact evidence could help; automatic prop choreography is not established. | Integration risk, `not-evaluated`; not a confirmed vendor defect. |

## Engine status

| Runtime | Evidence level | Technical result | Remaining gate |
|---|---|---|---|
| Unity unspecified | not-evaluated | No current import or playback. | Import, contacts, visual, and build tests. |
| Unreal Engine unspecified | not-evaluated | No current import or playback. | Import, retarget, contacts, and build tests. |
| Godot unspecified | not-evaluated | No current conversion or playback. | Conversion/import and graph tests. |
| Bevy unspecified | not-evaluated | No current handoff or runtime test. | glTF handoff and runtime test. |

## Fit and limitations

Combine with Basic Locomotion through a state transition after the controller reaches a campfire anchor. Do not crossfade arbitrary moving locomotion into a seated loop while keeping translation enabled. Cross-pack entry/exit pose, prop placement, collision and character proportions remain untested.

These are proposed integration decisions (`inferred`), with target-controller, contact and artistic acceptance still `not-evaluated`.

## Changes between AnimSmith versions

AnimSmith 0.14.0 — reran 29 input baselines, 25 per-file declared contracts and one remediation trial using the official release. Added current exact-member generic scenarios and developer/artist actions. AnimSmith 0.10.0 — superseded historical evidence; none of its generated outputs or engine results is relabelled as a fresh run.

## Evidence status

Current evidence uses the official release and the [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder); commercial artifacts remain external.

## Sources

- Protofactor, [Animset: Campfire](https://protofactor.biz/product/animset-campfire/) — product context.
- AnimSmith, [CLI reference](../cli.md) and [game-ready clips](../game-ready-clips.md) — evaluation boundaries.
