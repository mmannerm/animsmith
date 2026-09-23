# Animation pack evaluation: Protofactor Campfire

> Technical verdict: **Usable with conditions**
>
> Evaluation completeness: **partial** — all source files and declared checks were reviewed; engine playback and visual quality remain untested.
>
> Confidence: **medium**
>
> Evaluation date: **2026-09-16**
>
> Current evaluator: **AnimSmith 0.14.0**
>
> Report format: **3**
>
> Detailed evidence: [Protofactor Campfire evidence](protofactor-campfire-evidence.md)

## Technical decision

**Prototype decision:** Try discrete full-body interactions with project anchors and prop attachment. Configure one-shots and exits; scene contact and interruption remain unknown.

**Camera scope:** Full-body humanoid animations; appearance on your target character and camera is untested. Dedicated first-person arms/viewmodel use is unverified.

This animset is one of eight locally evaluated parts of [Ultimate Animation Collection](protofactor-ultimate-animation-collection.md). The local asset revision is unknown; the collection report separates evaluated files from the dated vendor listing.

**Content in evaluated inventory:** Kneel, sit, lie and grill interactions. The evaluated inventory includes posture changes and prop actions; locomotion coverage is outside this pack. These are identified motions; gameplay behavior has not been approved.

**Use as a full-body interaction state family, not a locomotion blend tree.** The candidate kneel/sit/lie and grill sequences give a developer concrete states to prototype. The game still owns interaction anchors, interruption rules and props. If a one-time action fails a looping check, first correct its project setting before judging the animation.

`observed-animsmith`: the official 0.14.0 evaluator inspected and measured all 29 FBXs; 17/25 declared-contract files pass and 8 fail per output format. The findings below separate source problems, project settings and untested gameplay. Transformed outputs still need gameplay and visual checks.

## Capability coverage

### Content present

Filename-classified kneel/sit/lie transitions, held postures and skewer/grill actions support three proposed interaction chains. Named reverse transitions can be evaluated for exit.

### Content gaps and unknowns

Ground locomotion is outside this interaction selection. Prop attachment, scene anchors and complete interrupt/return topology are unclassified in this evaluation.

### Evaluation still needed

Play entry, hold, prop actions, interruption and exits on the target scene. No current target-engine or visual/contact acceptance exists.

## Runtime sets and authored motion

Use each row as a separate blend or sequence proposal. The linked appendix names its exact clips, timings and measurements; controller playback and contact quality still need testing.

| Set | Controller use | Adoption decision | Exact members |
|---|---|---|---|
| Kneel and sit sequence | Discrete posture/prop chain | Prototype for discrete entry/hold/exit. The kneel hold has a measured rotational seam that pruning did not clear; review its intended loop before repetition ([CF-LOOP](#technical-issue-register)). | [4 exact members](protofactor-campfire-evidence.md#exact-runtime-members) |
| Sit and lie sequence | Discrete posture/prop chain | Prototype after configuring one-shot transitions and explicit return. Pose and seat contact still need scene review. | [4 exact members](protofactor-campfire-evidence.md#exact-runtime-members) |
| Grill interaction sequence | Discrete posture/prop chain | Prototype after prop-anchor and event configuration. If kneel hold repeats, its seam remains; grip/contact is untested. | [4 exact members](protofactor-campfire-evidence.md#exact-runtime-members) |

## Integration recipe

1. **Members/topology:** Choose the kneel/sit or sit/lie chain below. The listed chains cover ingress/hold only. Before returning to ground movement, select/review `Humanoid@IdleSitToIdleKneelCampfire.fbx` then `Humanoid@KneelToStandCampfire.fbx` (both `Take 001`); exit continuity is untested. Trigger one-shots once; fire-lighting alternatives are discrete choices.
2. **Timing/synchronization:** Verify exit/entry poses and event points for reaching, eating and log release. Do not normalize every action to the idle duration.
3. **State ownership:** Enter at an authored interaction anchor, stop ground locomotion, and define how interruption restores control. Inspect root/hip offsets before deciding whether an action may displace the capsule.
4. **Composition constraints:** Validate skewer/log attachment and hand placement against the scene. Upper-body masking may break posture and is not established here.
5. **Acceptance gate:** Play the whole interaction including approach, interrupt and exit, with the intended campfire and seating geometry.

## Technical issue register

Severity describes impact as delivered for the stated use. Residual status is explicit; untested risks are not confirmed artist defects.

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| CF-LOOP | major | The selected idle candidate `Humanoid@IdleKneelCampfire.fbx` has a rotational seam; repeated kneeling may pulse. [Readiness guidance](../game-ready-clips.md#the-readiness-ladder). | artist-author | For genuine resting loops, review endpoint pose/velocity and re-export clean cycles. Pruning IdleKneel does not remove its seam. The measured seam persists; review looping playback before repeating this hold. | Current measurements locate discontinuities; pruning is not a loop repair. | Exact `Take 001`, loop-seam-rot in current contract/output; `observed-animsmith`. |
| CF-POLICY | minor | Lighting and transition actions are also tested as loops; wrap failures do not establish defects when those actions are used once. [Readiness guidance](../game-ready-clips.md#the-readiness-ladder). | engine-config | Mark reviewed one-shots non-looping and test their intended start/end transitions. Residual: unresolved configuration choice, not proven bad animation. | Explicit per-clip declarations already support this distinction. | For example `Humanoid@IdleKneelToIdleGrillSkewerCampfire.fbx`; `observed-animsmith`, role `inferred`. |
| CF-PROP | major | Hand/prop placement and release timing have no current contact test; visible penetration or floating objects would block a finished interaction. [Readiness guidance](../game-ready-clips.md#the-readiness-ladder). | engine-config | Set scene and prop anchors, then test grill, eat and log events on the chosen character. If contacts fail, give the artist the clip, time and scene position. Contact quality remains unknown. | Generic contact evidence could help; automatic prop choreography is not established. | Integration risk, `not-evaluated`; not a confirmed vendor defect. |

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
