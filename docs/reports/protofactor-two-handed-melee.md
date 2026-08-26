# Animation pack evaluation: Protofactor 2-Handed Melee Weapon Animset

> Technical verdict: **Usable with conditions**
>
> Evaluation completeness: **partial** — complete 0.7.0 baseline, explicit-role contracts, remediation, addressability, and bounded advice plus retained Unity evidence; no visual controller, target character, or current engine-editor import/playback.
>
> Confidence: **medium**
>
> Evaluation date: **2026-08-26**
>
> Report format: **1**
>
> Detailed evidence: [Protofactor 2-Handed Melee evidence appendix](protofactor-two-handed-melee-evidence.md)

## Technical decision

Use 118 Unity-Humanoid clips as a full-body two-handed combat mode, after quarantining `Humanoid@Blocked2HandMelee.fbx` and `Humanoid@IdleBlock2HandMelee.fbx`. Seven pack samples, two cross-pack mixers, one mask, and the sword attachment execute unchanged; headless execution is not visual/contact acceptance.

AnimSmith 0.4.0 (`6b37ad636b1`) reproduces the published manifest byte-for-byte — a pure evaluator refresh. The default profile resolves almost nothing here (root trajectory 4/122, gait 3/122); the unchanged explicit `[rig.roles]` config resolves it fully (122/122; gait 121/122). Identical lint-note counts in both passes prove the map is repeatable configuration, not an asset repair — [#437](https://github.com/mmannerm/animsmith/issues/437) stays open (live 2026-08-21).

Gait anchoring under the same contracts reproduces `674396f`'s circular phase spreads to seven decimals (~0.58–0.71 raw → ~0.05–0.14 anchored), confirming the release preserves 0.3.1 gait behavior; candidates stay unpromoted, no Humanoid-retarget or visual import.

Replace loop policy, retain runtime offsets for raw RM and residual IP phase, and review grip, contacts, hit windows, transitions, root-motion ownership, and one heavy-hit RM timing anomaly.

## Capability coverage

### Complete core

- Paired IP/RM walk, run, crouch, normal-forward speed, and forward/back dodge families.
- Attacks/combos, parries, reactions/deaths, airborne states, equipment transitions, idles, taunts, and one sword prop.

### Partial supporting gameplay

- Blocking loses two Generic clips; events, cancel policy, contact/IK/visual acceptance missing.
- Upper-body composition executes headlessly, but grip, pelvis motion, weapon arcs, and mask quality remain open.

### Absent

- Starts/stops/turns, traversal, paired-character interactions, additive aim, first-person content, and motion-matching metadata.

## Runtime sets and authored motion

Coordinates are `(right,forward)`; speeds are measured RM magnitudes.

| Set/profile | Role or coordinate | Exact members | Variant/type | Timing or motion | Runtime contract |
|---|---|---|---|---|---|
| Walk combat 8-way | F `(0,1)` | IP `Humanoid@WalkForwardCombat2HandMelee.fbx`; RM `Humanoid@WalkForwardCombat2HandMelee_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.945 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Walk combat 8-way | FL `(-0.707,0.707)` | IP `Humanoid@WalkForwardLeftCombat2HandMelee.fbx`; RM `Humanoid@WalkForwardLeftCombat2HandMelee_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.901 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Walk combat 8-way | L `(-1,0)` | IP `Humanoid@WalkLeftCombat2HandMelee.fbx`; RM `Humanoid@WalkLeftCombat2HandMelee_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.837 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Walk combat 8-way | BL `(-0.707,-0.707)` | IP `Humanoid@WalkBackwardsLeftCombat2HandMelee.fbx`; RM `Humanoid@WalkBackwardsLeftCombat2HandMelee_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.973 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Walk combat 8-way | B `(0,-1)` | IP `Humanoid@WalkBackwardsCombat2HandMelee.fbx`; RM `Humanoid@WalkBackwardsCombat2HandMelee_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.904 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Walk combat 8-way | BR `(0.707,-0.707)` | IP `Humanoid@WalkBackwardsRightCombat2HandMelee.fbx`; RM `Humanoid@WalkBackwardsRightCombat2HandMelee_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.810 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Walk combat 8-way | R `(1,0)` | IP `Humanoid@WalkRightCombat2HandMelee.fbx`; RM `Humanoid@WalkRightCombat2HandMelee_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.896 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Walk combat 8-way | FR `(0.707,0.707)` | IP `Humanoid@WalkForwardRightCombat2HandMelee.fbx`; RM `Humanoid@WalkForwardRightCombat2HandMelee_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=1.092 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Run combat 8-way | F `(0,1)` | IP `Humanoid@RunForwardCombat2HandMelee.fbx`; RM `Humanoid@RunForwardCombat2HandMelee_RM.fbx` | variant=paired-ip-rm | duration=0.533 s; rm_speed=2.444 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Run combat 8-way | FL `(-0.707,0.707)` | IP `Humanoid@RunForwardLeftCombat2HandMelee.fbx`; RM `Humanoid@RunForwardLeftCombat2HandMelee_RM.fbx` | variant=paired-ip-rm | duration=0.567 s; rm_speed=2.402 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Run combat 8-way | L `(-1,0)` | IP `Humanoid@RunLeftCombat2HandMelee.fbx`; RM `Humanoid@RunLeftCombat2HandMelee_RM.fbx` | variant=paired-ip-rm | duration=0.533 s; rm_speed=2.690 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Run combat 8-way | BL `(-0.707,-0.707)` | IP `Humanoid@RunBackwardsLeftCombat2HandMelee.fbx`; RM `Humanoid@RunBackwardsLeftCombat2HandMelee_RM.fbx` | variant=paired-ip-rm | duration=0.567 s; rm_speed=2.613 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Run combat 8-way | B `(0,-1)` | IP `Humanoid@RunBackwardsCombat2HandMelee.fbx`; RM `Humanoid@RunBackwardsCombat2HandMelee_RM.fbx` | variant=paired-ip-rm | duration=0.533 s; rm_speed=2.202 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Run combat 8-way | BR `(0.707,-0.707)` | IP `Humanoid@RunBackwardsRightCombat2HandMelee.fbx`; RM `Humanoid@RunBackwardsRightCombat2HandMelee_RM.fbx` | variant=paired-ip-rm | duration=0.567 s; rm_speed=2.293 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Run combat 8-way | R `(1,0)` | IP `Humanoid@RunRightCombat2HandMelee.fbx`; RM `Humanoid@RunRightCombat2HandMelee_RM.fbx` | variant=paired-ip-rm | duration=0.567 s; rm_speed=2.508 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Run combat 8-way | FR `(0.707,0.707)` | IP `Humanoid@RunForwardRightCombat2HandMelee.fbx`; RM `Humanoid@RunForwardRightCombat2HandMelee_RM.fbx` | variant=paired-ip-rm | duration=0.567 s; rm_speed=2.514 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Crouch combat 8-way | F `(0,1)` | IP `Humanoid@CrouchForward2HandMelee.fbx`; RM `Humanoid@CrouchForward2HandMelee_RM.fbx` | variant=paired-ip-rm | duration=1.667 s; rm_speed=0.689 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Crouch combat 8-way | FL `(-0.707,0.707)` | IP `Humanoid@CrouchForwardLeft2HandMelee.fbx`; RM `Humanoid@CrouchForwardLeft2HandMelee_RM.fbx` | variant=paired-ip-rm | duration=1.667 s; rm_speed=0.728 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Crouch combat 8-way | L `(-1,0)` | IP `Humanoid@CrouchLeft2HandMelee.fbx`; RM `Humanoid@CrouchLeft2HandMelee_RM.fbx` | variant=paired-ip-rm | duration=1.667 s; rm_speed=0.762 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Crouch combat 8-way | BL `(-0.707,-0.707)` | IP `Humanoid@CrouchBackwardsLeft2HandMelee.fbx`; RM `Humanoid@CrouchBackwardsLeft2HandMelee_RM.fbx` | variant=paired-ip-rm | duration=1.667 s; rm_speed=0.649 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Crouch combat 8-way | B `(0,-1)` | IP `Humanoid@CrouchBackwards2HandMelee.fbx`; RM `Humanoid@CrouchBackwards2HandMelee_RM.fbx` | variant=paired-ip-rm | duration=1.667 s; rm_speed=0.642 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Crouch combat 8-way | BR `(0.707,-0.707)` | IP `Humanoid@CrouchBackwardsRight2HandMelee.fbx`; RM `Humanoid@CrouchBackwardsRight2HandMelee_RM.fbx` | variant=paired-ip-rm | duration=1.667 s; rm_speed=0.704 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Crouch combat 8-way | R `(1,0)` | IP `Humanoid@CrouchRight2HandMelee.fbx`; RM `Humanoid@CrouchRight2HandMelee_RM.fbx` | variant=paired-ip-rm | duration=1.667 s; rm_speed=0.698 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Crouch combat 8-way | FR `(0.707,0.707)` | IP `Humanoid@CrouchForwardRight2HandMelee.fbx`; RM `Humanoid@CrouchForwardRight2HandMelee_RM.fbx` | variant=paired-ip-rm | duration=1.667 s; rm_speed=0.835 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Normal forward speed | walk | IP `Humanoid@WalkForwardNormal2HandMelee.fbx`; RM `Humanoid@WalkForwardNormal2HandMelee_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.956 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Normal forward speed | run | IP `Humanoid@RunNormal2HandMelee.fbx`; RM `Humanoid@RunNormal2HandMelee_RM.fbx` | variant=paired-ip-rm | duration=0.667 s; rm_speed=2.805 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Normal forward speed | sprint | IP `Humanoid@Sprint2HandMelee.fbx`; RM `Humanoid@Sprint2HandMelee_RM.fbx` | variant=paired-ip-rm | duration=0.500 s; rm_speed=6.000 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Draw/combat/put-away | ordered | `Humanoid@Draw2HandMelee.fbx`; `Humanoid@IdleCombatA2HandMelee.fbx`; `Humanoid@PutBack2HandMelee.fbx` | set_type=transition-chain | N/A | transition=at-end; state=armed-combat |

RM speed ratios are 1.35× walk, 1.22× run, 1.30× crouch, and 6.27× across the normal-forward speed family; preserve authored velocity or tune playback deliberately, never normalize silently. Circular phase spreads reproduce historical `674396f` values on released 0.4.0 (see Technical decision); residual offsets and acceptance remain.

## Integration recipe

1. **Members/topology:** `topology=separate-ip-rm-combat-graphs`; build the table's directional/speed graphs; exclude both block outliers.
2. **Timing/synchronization:** `sync=validated-ip-anchor-plus-offsets`; loop reviewed locomotion/idles/holds; use raw clips with runtime offsets until IP candidates pass engine/visual gates (never RM); keep actions/recoveries one-shot.
3. **State ownership:** `owner=validate-per-axis`; controller owns IP translation; validate RM ownership per axis, since sampled RM clips bake root rotation.
4. **Composition constraints:** `composition=full-body-two-handed-default`; attach the sword right-handed; promote masks only after pelvis, grip, contact, and twist-bone review.
5. **Acceptance gate:** `gate=target-character-combat-review`; test rings, wraps, equipment, combat actions, root extraction, masks, contacts, deformation, compression, and builds.

## Technical issue register

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| TH-001 | blocker | [Rig/import disagreement](../game-ready-clips.md#files-disagree-about-skeleton-or-clip-identity) excludes both block clips from Unity Humanoid. | artist-author | Quarantine or substitute pending corrected exports. | Detection can improve; metadata/intent cannot be invented. | Unity imports both as Generic; 56-bone structure. |
| TH-002 | major | Default roles miss 118 capitalized clips, hiding gait/root/loop-seam coverage. Guidance: [rig identity](../game-ready-clips.md#files-disagree-about-skeleton-or-clip-identity). | animsmith-future-candidate | Use the explicit-role config throughout. | Fail-closed case-tolerant aliasing tracked by [#437](https://github.com/mmannerm/animsmith/issues/437). | Explicit config unlocks 4 loop-seam findings (0 default); identical 17,016-note lint count proves configuration. |
| TH-003 | major | [Incorrect loop declarations](../game-ready-clips.md#the-loop-pops) flag 44 obvious one-shots, risking repeats/hard wraps. | engine-config | Override them; review remaining candidates. | Metadata-aware auditing is feasible; universal intent inference is not. | 108/120 loop flags; 107 contract failures. |
| TH-004 | major | [Gait-phase disagreement](../game-ready-clips.md#feet-skate-when-clips-blend) risks foot skating. | animsmith-current-declared | Trial the 24 IP candidates, validate, and offset residual phase; keep RM raw. | Vertical-root-axis gait-anchor shipped via [#426](https://github.com/mmannerm/animsmith/issues/426) (merged); RM trajectory-preservation untested. | 0.4.0 reproduces circular spreads 0.577–0.711 → 0.054–0.143 to 7 decimals vs. `674396f`; no engine/visual acceptance. |
| TH-005 | major | [Directional RM speed variation](../game-ready-clips.md#directional-blend-members-travel-at-different-speeds) can make input magnitude change with direction. | engine-config | Preserve velocity per member or tune playback. | Cross-member checks tracked by [#411](https://github.com/mmannerm/animsmith/issues/411). | Walk 1.35×; crouch 1.30×; run 1.22×. |
| TH-006 | moderate | [Unequal channel spans](../game-ready-clips.md#the-clip-is-the-wrong-length-or-freezes-at-the-end) can clamp-hold forearm twists in `Humanoid@GetHitLeftHeavy2HandMelee_RM.fbx`. | artist-author | Review/correct timing on the target rig. | Declared normalization is plausible only with twist/contact proof. | One warning; six notes; unchanged since 0.3.0. |
| TH-007 | moderate | [RM action ownership](../game-ready-clips.md#the-character-glides-or-runs-in-place) is unclear, risking doubled or missing motion. | engine-config | Declare per-axis `movement_owner_*` intent; do not infer it from travel. | [#408](https://github.com/mmannerm/animsmith/issues/408)/[#466](https://github.com/mmannerm/animsmith/issues/466) shipped; needs a human/engine decision. | 45/121 clips move >1 cm, 76 stationary, 0 with >1° yaw — sampled, not proof. |
| TH-008 | moderate | [Dense constant tracks](../game-ready-clips.md#the-file-is-bloated-or-the-retargeter-chokes) may waste memory; pruning lacks equivalence proof. | animsmith-current-declared | Retain sources pending runtime/equivalence gates. | `(bone, property)` coverage shipped via [#402](https://github.com/mmannerm/animsmith/issues/402); pruning stays open via [#401](https://github.com/mmannerm/animsmith/issues/401). | 16,747 notes; one readable export candidate. |

## Engine status

| Runtime | Evidence level | Technical result | Remaining gate |
|---|---|---|---|
| Unity 6000.5.8f1 / 6000.3 | Retained co-import (2026-08-17) + rev 1 advice (2026-08-21) | **Conditional pass (retained):** 118/120 clips Humanoid; 7 samples/2 mixers/mask/sword pass. New: advice available (exit 0), now **observed**-derived; appendix. | Visual controller, contacts, grip, root motion, retargeting, compression, build. |
| Unreal Engine 5.8 | Advice probe | **Not evaluated in-editor.** `unreal` rev 1 refuses (`profile_settings_unmodeled`, exit 1); not an editor test. | FBX import, retarget/twist mapping, graphs, contacts, build. |
| Godot 4.7 | Advice probe | **Not evaluated in-editor.** `godot` rev 1: same refusal (exit 1). | Import/conversion, retarget, graphs, contacts, export. |
| Bevy 0.19.0 | Addressability probe | **Not evaluated at runtime.** Generated-GLB addressability exits 0: 1 animation, selector `Animation0` — prediction only. | glTF conversion, retarget path, graph, root motion, performance. |

## Fit and limitations

Best fit: third-person action RPGs needing a full-body two-handed state with configurable hit timing, contacts, and ownership.

Poor fit: blocking-critical gameplay without substitutes, first-person, traversal-heavy, motion-matching, or network-root-motion without further authoring.

The dominant rig adds forearm twists to the collection's 56-bone majority; Unity Humanoid mixers with Basic Locomotion and Sword & Shield execute. Use full-body handoffs until style, grip, twist deformation, and contacts pass visual review. The [partial collection rollup](protofactor-ultimate-animation-collection.md) owns the cross-pack conclusion.

## AnimSmith 0.7.0 refresh (2026-08-26)

Exact `v0.7.0` reproduced 123 FBXs, 120 explicit-role contracts (13/107 pass/fail), and 25 candidates under output v17 / measurements v16. Projections are not engine or artistic acceptance.

## Evidence status

The [readiness ladder](../game-ready-clips.md#the-readiness-ladder) and [appendix](protofactor-two-handed-melee-evidence.md) retain current and historical provenance plus the explicit role map.

## Sources

- Protofactor, [2-Handed Melee Weapon product page](https://protofactor.biz/product/animset-2-handed-melee-weapon/), [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/), and [End User License Agreement](https://protofactor.biz/end-user-license-agreement/).
- Unity, [Avatar Mask](https://docs.unity3d.com/6000.0/Documentation/Manual/class-AvatarMask.html), [Animation Layers](https://docs.unity3d.com/6000.0/Documentation/Manual/AnimationLayers.html), and [Root Motion](https://docs.unity3d.com/6000.5/Documentation/Manual/RootMotion.html).
- Epic Games, [Root Motion](https://dev.epicgames.com/documentation/unreal-engine/root-motion-in-unreal-engine?application_version=5.7) and [Layered Animations](https://dev.epicgames.com/documentation/unreal-engine/using-layered-animations-in-unreal-engine?lang=en-US); Godot, [AnimationTree](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html); Bevy, [Animation Masks](https://bevy.org/examples/animation/animation-masks/).
