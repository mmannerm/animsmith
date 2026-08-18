# Animation pack evaluation: Protofactor 1-Handed Melee Weapon Animset

> Technical verdict: **Usable with conditions**
>
> Evaluation completeness: **partial** — 0.3.0 baseline/contracts, 0.3.1-bound gait remediation, and Unity 6000.5.8f1 co-import probes; no visual controller, target character, Unreal Engine, Godot, or Bevy pass.
>
> Confidence: **medium**
>
> Evaluation date: **2026-08-17**
>
> Report format: **1**
>
> Detailed evidence: [Protofactor 1-Handed Melee evidence appendix](protofactor-one-handed-melee-evidence.md)

## Technical decision

Use the 108 Unity-Humanoid gameplay clips as a full-body one-handed combat mode, after quarantining `Humanoid@Blocked1hMelee.fbx` and `Humanoid@IdleBlock1hMelee.fbx`. The remaining sampled paths execute in Unity unchanged.

AnimSmith measures the 56-bone majority without setup and exposes loop, phase, speed, and track risks. At `674396f`, gait anchoring produces 24 IP candidates and reduces walk/run/crouch phase spreads from 0.554/0.734/0.714 to 0.064/0.108/0.039. They remain experimental: RM was untouched and Unity, visual, and trajectory acceptance were not performed. Constant-track pruning also exports but remains unaccepted.

Replace the delivered loop policy, preserve per-direction speeds, and retain runtime offsets until generated candidates pass visual and engine acceptance. Visually author/accept grip, contacts, hit windows, equipment visibility, and transitions. The headless mask pass is only a candidate: displacement-bearing attacks should remain full-body by default.

## Capability coverage

### Complete core

- Paired IP/RM walk, run, crouch, and held-weapon forward speed families.
- One-handed attacks/combos, reactions/deaths, airborne states, equipment transitions, combat idles, and one bludgeon prop.

### Partial supporting gameplay

- Blocking loses two delivered Generic clips; attacks lack events, cancellation rules, contact/IK proof, and visual acceptance.
- Upper-body composition executes headlessly, but pelvis torque, support, grip, and weapon arcs are unaccepted.

### Absent

- General starts/stops/turns, traversal, paired-character interactions, additive aim, first-person content, and authored motion-matching metadata.

## Runtime sets and authored motion

Coordinates are `(right,forward)`; speeds are measured RM magnitudes. IP/RM counterparts have nearly identical pair phases, but each ring is mutually out of phase.

| Set/profile | Role or coordinate | Exact members | Variant/type | Timing or motion | Runtime contract |
|---|---|---|---|---|---|
| Walk combat 8-way | F `(0,1)` | IP `Humanoid@WalkForwardCombat1hMelee.fbx`; RM `Humanoid@WalkForwardCombat1hMelee_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.951 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Walk combat 8-way | FL `(-0.707,0.707)` | IP `Humanoid@WalkForwardLeftCombat1hMelee.fbx`; RM `Humanoid@WalkForwardLeftCombat1hMelee_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.834 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Walk combat 8-way | L `(-1,0)` | IP `Humanoid@WalkLeftCombat1hMelee.fbx`; RM `Humanoid@WalkLeftCombat1hMelee_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.491 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Walk combat 8-way | BL `(-0.707,-0.707)` | IP `Humanoid@WalkBackwardsLeftCombat1hMelee.fbx`; RM `Humanoid@WalkBackwardsLeftCombat1hMelee_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.739 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Walk combat 8-way | B `(0,-1)` | IP `Humanoid@WalkBackwardsCombat1hMelee.fbx`; RM `Humanoid@WalkBackwardsCombat1hMelee_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.797 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Walk combat 8-way | BR `(0.707,-0.707)` | IP `Humanoid@WalkBackwardsRightCombat1hMelee.fbx`; RM `Humanoid@WalkBackwardsRightCombat1hMelee_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.837 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Walk combat 8-way | R `(1,0)` | IP `Humanoid@WalkRightCombat1hMelee.fbx`; RM `Humanoid@WalkRightCombat1hMelee_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.518 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Walk combat 8-way | FR `(0.707,0.707)` | IP `Humanoid@WalkForwardRightCombat1hMelee.fbx`; RM `Humanoid@WalkForwardRightCombat1hMelee_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.701 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Run 8-way | F `(0,1)` | IP `Humanoid@RunForward1hMelee.fbx`; RM `Humanoid@RunForward1hMelee_RM.fbx` | variant=paired-ip-rm | duration=0.667 s; rm_speed=1.905 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Run 8-way | FL `(-0.707,0.707)` | IP `Humanoid@RunForwardLeft1hMelee.fbx`; RM `Humanoid@RunForwardLeft1hMelee_RM.fbx` | variant=paired-ip-rm | duration=0.667 s; rm_speed=1.909 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Run 8-way | L `(-1,0)` | IP `Humanoid@RunLeft1hMelee.fbx`; RM `Humanoid@RunLeft1hMelee_RM.fbx` | variant=paired-ip-rm | duration=0.667 s; rm_speed=1.915 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Run 8-way | BL `(-0.707,-0.707)` | IP `Humanoid@RunBackwardsLeft1hMelee.fbx`; RM `Humanoid@RunBackwardsLeft1hMelee_RM.fbx` | variant=paired-ip-rm | duration=0.667 s; rm_speed=1.909 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Run 8-way | B `(0,-1)` | IP `Humanoid@RunBackwards1hMelee.fbx`; RM `Humanoid@RunBackwards1hMelee_RM.fbx` | variant=paired-ip-rm | duration=0.600 s; rm_speed=2.117 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Run 8-way | BR `(0.707,-0.707)` | IP `Humanoid@RunBackwardsRight1hMelee.fbx`; RM `Humanoid@RunBackwardsRight1hMelee_RM.fbx` | variant=paired-ip-rm | duration=0.667 s; rm_speed=1.909 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Run 8-way | R `(1,0)` | IP `Humanoid@RunRight1hMelee.fbx`; RM `Humanoid@RunRight1hMelee_RM.fbx` | variant=paired-ip-rm | duration=0.667 s; rm_speed=1.908 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Run 8-way | FR `(0.707,0.707)` | IP `Humanoid@RunForwardRight1hMelee.fbx`; RM `Humanoid@RunForwardRight1hMelee_RM.fbx` | variant=paired-ip-rm | duration=0.667 s; rm_speed=1.909 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Crouch combat 8-way | F `(0,1)` | IP `Humanoid@CrouchForward1hMelee.fbx`; RM `Humanoid@CrouchForward1hMelee_RM.fbx` | variant=paired-ip-rm | duration=1.667 s; rm_speed=0.578 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Crouch combat 8-way | FL `(-0.707,0.707)` | IP `Humanoid@CrouchForwardLeft1hMelee.fbx`; RM `Humanoid@CrouchForwardLeft1hMelee_RM.fbx` | variant=paired-ip-rm | duration=1.667 s; rm_speed=0.668 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Crouch combat 8-way | L `(-1,0)` | IP `Humanoid@CrouchLeft1hMelee.fbx`; RM `Humanoid@CrouchLeft1hMelee_RM.fbx` | variant=paired-ip-rm | duration=1.667 s; rm_speed=0.480 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Crouch combat 8-way | BL `(-0.707,-0.707)` | IP `Humanoid@CrouchBackwardsLeft1hMelee.fbx`; RM `Humanoid@CrouchBackwardsLeft1hMelee_RM.fbx` | variant=paired-ip-rm | duration=1.667 s; rm_speed=0.660 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Crouch combat 8-way | B `(0,-1)` | IP `Humanoid@CrouchBackwards1hMelee.fbx`; RM `Humanoid@CrouchBackwards1hMelee_RM.fbx` | variant=paired-ip-rm | duration=1.667 s; rm_speed=0.649 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Crouch combat 8-way | BR `(0.707,-0.707)` | IP `Humanoid@CrouchBackwardsRight1hMelee.fbx`; RM `Humanoid@CrouchBackwardsRight1hMelee_RM.fbx` | variant=paired-ip-rm | duration=1.667 s; rm_speed=0.622 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Crouch combat 8-way | R `(1,0)` | IP `Humanoid@CrouchRight1hMelee.fbx`; RM `Humanoid@CrouchRight1hMelee_RM.fbx` | variant=paired-ip-rm | duration=1.667 s; rm_speed=0.480 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Crouch combat 8-way | FR `(0.707,0.707)` | IP `Humanoid@CrouchForwardRight1hMelee.fbx`; RM `Humanoid@CrouchForwardRight1hMelee_RM.fbx` | variant=paired-ip-rm | duration=1.667 s; rm_speed=0.730 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Hold forward speed | walk | IP `Humanoid@WalkForwardHold1hMelee.fbx`; RM `Humanoid@WalkForwardHold1hMelee_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.787 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Hold forward speed | run | IP `Humanoid@RunHold1hMelee.fbx`; RM `Humanoid@RunHold1hMelee_RM.fbx` | variant=paired-ip-rm | duration=0.600 s; rm_speed=3.117 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Hold forward speed | sprint | IP `Humanoid@SprintHold1hMelee.fbx`; RM `Humanoid@SprintHold1hMelee_RM.fbx` | variant=paired-ip-rm | duration=0.400 s; rm_speed=7.500 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Draw/combat/put-away | ordered | `Humanoid@DrawWeapon1hMelee.fbx`; `Humanoid@IdleCombat1hMelee.fbx`; `Humanoid@PutBackWeapon1hMelee.fbx` | set_type=transition-chain | N/A | transition=at-end; state=armed-combat |

RM speed ratios are 1.94× walk, 1.11× run, 1.52× crouch, and 9.52× across the intended hold speed chain. Preserve directional velocity or tune playback; do not normalize silently. Raw walk/run/crouch phase spreads are 0.554/0.734/0.714; current IP candidates reduce them to 0.064/0.108/0.039 but still require acceptance and residual offsets.

## Integration recipe

1. **Members/topology:** `topology=separate-ip-rm-combat-graphs`; create the three directional graphs and hold speed chain from the exact table members, and exclude both quarantined block clips.
2. **Timing/synchronization:** `sync=validated-ip-anchor-plus-offsets`; loop reviewed locomotion/idles/holds; use raw clips with runtime offsets until the IP candidates pass engine/visual gates, never apply this evidence to RM, and keep actions/recoveries one-shot.
3. **State ownership:** `owner=split-by-movement-variant`; the controller owns IP translation/yaw, animation owns accepted RM translation/yaw, and every RM action gets an explicit owner decision.
4. **Composition constraints:** `composition=full-body-combat-default`; attach the bludgeon to the right hand and promote upper-body masks only after pelvis, contact, grip, and target-character review.
5. **Acceptance gate:** `gate=target-character-combat-review`; test complete rings, wraps, draw/put-away, attacks, hits, contacts, root extraction, masks, deformation, compression, and builds.

## Technical issue register

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| OH-001 | blocker | [Rig/import disagreement](../game-ready-clips.md#files-disagree-about-skeleton-or-clip-identity) leaves both block clips outside the Unity Humanoid runtime; one also has a 73-bone hierarchy. | artist-author | Quarantine both and use another reaction/idle until corrected exports exist. | Detection can improve; author intent and Humanoid metadata cannot be safely invented. | Unity imports both as Generic; AnimSmith distinguishes the 73-bone outlier. |
| OH-002 | major | [Incorrect loop declarations](../game-ready-clips.md#the-loop-pops) mark 30 obvious attacks, combos, hits, and recoveries as loops, risking repeated actions and hard wraps. | engine-config | Override one-shot-like flags and review remaining loop candidates. | A metadata/role-aware audit is feasible; universal intent inference is not. | 93/110 delivered loop flags; 87 declared-loop contract failures. |
| OH-003 | major | [Gait-phase disagreement](../game-ready-clips.md#feet-skate-when-clips-blend) risks foot skating. | animsmith-current-declared | Trial the 24 IP candidates, then validate and offset residual phase; keep RM raw. | IP support shipped via [#426](https://github.com/mmannerm/animsmith/issues/426); trajectory-preserving RM was not tested. | Spreads fall 0.554–0.734 → 0.039–0.108; no engine/visual acceptance. |
| OH-004 | major | [Directional RM speed variation](../game-ready-clips.md#directional-blend-members-travel-at-different-speeds) can make input magnitude change with direction. | engine-config | Preserve velocity per member or tune playback against controller policy. | Cross-member policy checks are tracked by [#411](https://github.com/mmannerm/animsmith/issues/411). | Walk 1.94×; crouch 1.52×; run 1.11×. |
| OH-005 | moderate | [RM action ownership](../game-ready-clips.md#the-character-glides-or-runs-in-place) is not established by `_RM` or horizontal speed alone, risking doubled or missing displacement/yaw. | engine-config | Inspect displacement and yaw per attack/reaction before enabling root motion. | Independent translation/yaw evidence is tracked by [#408](https://github.com/mmannerm/animsmith/issues/408). | Eleven RM action/reaction files; yaw not summarized. |
| OH-006 | moderate | [Dense constant tracks](../game-ready-clips.md#the-file-is-bloated-or-the-retargeter-chokes) may waste memory; unproved pruning can change sparse-track behavior. | animsmith-current-declared | Keep sources until runtime/equivalence gates pass. | Current pruning works mechanically; stronger proof remains tracked by [#401](https://github.com/mmannerm/animsmith/issues/401) and [#402](https://github.com/mmannerm/animsmith/issues/402). | 13,360 contract notes; one verified export candidate. |

## Engine status

| Runtime | Evidence level | Technical result | Remaining gate |
|---|---|---|---|
| Unity 6000.5.8f1 | Eight-pack co-import + headless Playables/mask/attachment | **Conditional pass:** 108/110 individual clips are Humanoid; all six required samples, two cross-pack mixers, one mask, and the prop check pass. | Visual controller, contacts, grip, root motion, retargeting, compression, build. |
| Unreal Engine | Documentation only | **Not evaluated.** Root Motion, Blend Spaces, montages, and layered blends can express the policy; no native UE package is delivered. | FBX import, retarget, graphs, contacts, build. |
| Godot | Documentation only | **Not evaluated.** AnimationTree can express blend spaces, one-shots, filters, and root extraction. | Import/conversion, retarget, graphs, contacts, export. |
| Bevy | Documentation only | **Not evaluated.** AnimationGraph masks exist; FBX conversion and retargeting remain project work. | glTF conversion, retarget path, graph, root motion, performance. |

## Fit and limitations

Best fit: third-person action RPGs or melee prototypes with a dedicated full-body one-handed armed state and capacity to configure contacts, events, and controller ownership.

Poor fit: blocking-critical gameplay without substitute clips, first-person, traversal-heavy, motion-matching, or network-root-motion systems without further authoring.

The majority 56-bone structure matches Basic Locomotion, Sword & Shield, and Dual Swords; all shared package paths are byte-identical. Unity mixers and a Basic-locomotion mask execute, but use full-body state handoffs between weapon modes until style, pose, grip, and contact are visually accepted. The [partial collection rollup](protofactor-ultimate-animation-collection.md) owns the cross-pack conclusion.

## Evidence status

All 113 FBXs were analyzed; the v1 manifest covers 72 logical motions and 110 individual files. Baseline/contracts remain captured at `b7c215b`; remediation was refreshed with AnimSmith `0.3.0` at `674396f0f53b10c4344e7315a5756fe5ef71b469`. The [readiness ladder](../game-ready-clips.md#the-readiness-ladder) and [appendix](protofactor-one-handed-melee-evidence.md) define remaining boundaries. The local archive came from the user's Protofactor Ultimate collection download; current pages do not prove its constituent revision or historical terms.

## Sources

- Protofactor, [1-Handed Melee Weapon product page](https://protofactor.biz/product/animset-1-handed-melee-weapon/), [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/), and [End User License Agreement](https://protofactor.biz/end-user-license-agreement/).
- Unity, [Avatar Mask](https://docs.unity3d.com/6000.0/Documentation/Manual/class-AvatarMask.html), [Animation Layers](https://docs.unity3d.com/6000.0/Documentation/Manual/AnimationLayers.html), and [Root Motion](https://docs.unity3d.com/6000.5/Documentation/Manual/RootMotion.html).
- Epic Games, [Root Motion](https://dev.epicgames.com/documentation/unreal-engine/root-motion-in-unreal-engine?application_version=5.7) and [Layered Animations](https://dev.epicgames.com/documentation/unreal-engine/using-layered-animations-in-unreal-engine?lang=en-US); Godot, [AnimationTree](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html); Bevy, [Animation Masks](https://bevy.org/examples/animation/animation-masks/).
