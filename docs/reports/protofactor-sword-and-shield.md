# Animation pack evaluation: Protofactor Sword & Shield Animset

> Technical verdict: **Usable with conditions**
>
> Evaluation completeness: **partial** — exhaustive AnimSmith 0.3.0 analysis and Unity 6000.5.8f1 co-import/playable probes; no full visual controller, target character, Unreal Engine, Godot, or Bevy pass.
>
> Confidence: **medium**
>
> Evaluation date: **2026-08-17**
>
> Report format: **1**
>
> Detailed evidence: [Protofactor Sword & Shield evidence appendix](protofactor-sword-and-shield-evidence.md)

## Technical decision

Use the individual FBXs as a **third-person sword-and-shield combat pack**, after quarantining `Humanoid@CrouchForwardRightS&S_RM.fbx` and replacing the delivered loop policy. Unity imported 131/132 individual humanoid clips; the quarantined two-node file produces no AnimationClip. Other representative gameplay families sampled successfully.

The 28 locomotion motions supply IP/RM variants, but raw gait phases differ by 0.726–0.807 cycles. AnimSmith 0.3.0 safely refuses all 24 IP gait-anchor trials on this rig's unmeasurable horizontal root basis. Duplicate-endpoint removal fixes WalkForward closure, not seam derivatives. Use runtime offsets or artist-aligned exports; refusal is not a fix.

Unity metadata marks 118/132 files as loops, including 52 attack, defense, reaction, and taunt files that should start as one-shots. AnimSmith reports the consequences and can prune experimentally, but cannot reconstruct hierarchy, establish contacts, or infer every RM action's translation/yaw intent.

Unity attached both props at plausible hand-local scale; grip, contacts, deformation, and quality remain unproved.

## Capability coverage

### Complete core

- Eight-direction walk, run, and crouch combat locomotion in IP/RM variants, except quarantined Crouch FR RM.
- Melee, defense, reaction/death, equipment, idle, and taunt families.
- Props and a Unity Humanoid actor; standard clips share one 56-bone signature.

### Partial supporting gameplay

- Equipment and death/recovery chains without events, interruption rules, or visual crossfade acceptance.
- Three Unity mask graphs evaluated, but pelvis ownership, kicks, contacts, IK, and prop alignment are unaccepted.
- RM action displacement is measured; low-displacement/yaw semantics remain unresolved.

### Absent

- General starts/stops/turns, jumps, traversal, climbing, firearm/first-person content, paired-character interactions, additive aim offsets, and authored motion-matching metadata.

## Runtime sets and authored motion

Coordinates are `(right,forward)`; speeds are measured RM magnitudes. Crouch FR RM is catalogued but quarantined.

| Set/profile | Role or coordinate | Exact members | Variant/type | Timing or motion | Runtime contract |
|---|---|---|---|---|---|
| Walk combat 8-way | F `(0,1)` | IP `Humanoid@WalkForwardS&S.fbx`; RM `Humanoid@WalkForwardS&S_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.807 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Walk combat 8-way | FL `(-0.707,0.707)` | IP `Humanoid@WalkForwardLeftS&S.fbx`; RM `Humanoid@WalkForwardLeftS&S_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.902 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Walk combat 8-way | L `(-1,0)` | IP `Humanoid@WalkLeftS&S.fbx`; RM `Humanoid@WalkLeftS&S_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.899 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Walk combat 8-way | BL `(-0.707,-0.707)` | IP `Humanoid@WalkBackwardsLeftS&S.fbx`; RM `Humanoid@WalkBackwardsLeftS&S_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.750 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Walk combat 8-way | B `(0,-1)` | IP `Humanoid@WalkBackwardsS&S.fbx`; RM `Humanoid@WalkBackwardsS&S_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.835 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Walk combat 8-way | BR `(0.707,-0.707)` | IP `Humanoid@WalkBackwardsRightS&S.fbx`; RM `Humanoid@WalkBackwardsRightS&S_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.974 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Walk combat 8-way | R `(1,0)` | IP `Humanoid@WalkRightS&S.fbx`; RM `Humanoid@WalkRightS&S_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.926 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Walk combat 8-way | FR `(0.707,0.707)` | IP `Humanoid@WalkForwardRightS&S.fbx`; RM `Humanoid@WalkForwardRightS&S_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=1.107 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Run combat 8-way | F `(0,1)` | IP `Humanoid@RunForwardS&S.fbx`; RM `Humanoid@RunForwardS&S_RM.fbx` | variant=paired-ip-rm | duration=0.600 s; rm_speed=2.851 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Run combat 8-way | FL `(-0.707,0.707)` | IP `Humanoid@RunForwardLeftS&S.fbx`; RM `Humanoid@RunForwardLeftS&S_RM.fbx` | variant=paired-ip-rm | duration=0.600 s; rm_speed=3.063 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Run combat 8-way | L `(-1,0)` | IP `Humanoid@RunLeftS&S.fbx`; RM `Humanoid@RunLeftS&S_RM.fbx` | variant=paired-ip-rm | duration=0.600 s; rm_speed=3.189 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Run combat 8-way | BL `(-0.707,-0.707)` | IP `Humanoid@RunBackwardsLeftS&S.fbx`; RM `Humanoid@RunBackwardsLeftS&S_RM.fbx` | variant=paired-ip-rm | duration=0.600 s; rm_speed=2.944 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Run combat 8-way | B `(0,-1)` | IP `Humanoid@RunBackwardsS&S.fbx`; RM `Humanoid@RunBackwardsS&S_RM.fbx` | variant=paired-ip-rm | duration=0.600 s; rm_speed=2.904 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Run combat 8-way | BR `(0.707,-0.707)` | IP `Humanoid@RunBackwardsRightS&S.fbx`; RM `Humanoid@RunBackwardsRightS&S_RM.fbx` | variant=paired-ip-rm | duration=0.600 s; rm_speed=2.793 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Run combat 8-way | R `(1,0)` | IP `Humanoid@RunRightS&S.fbx`; RM `Humanoid@RunRightS&S_RM.fbx` | variant=paired-ip-rm | duration=0.600 s; rm_speed=2.955 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Run combat 8-way | FR `(0.707,0.707)` | IP `Humanoid@RunForwardRightS&S.fbx`; RM `Humanoid@RunForwardRightS&S_RM.fbx` | variant=paired-ip-rm | duration=0.600 s; rm_speed=3.021 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Crouch combat 8-way | F `(0,1)` | IP `Humanoid@CrouchForwardS&S.fbx`; RM `Humanoid@CrouchForwardS&S_RM.fbx` | variant=paired-ip-rm | duration=1.667 s; rm_speed=0.789 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Crouch combat 8-way | FL `(-0.707,0.707)` | IP `Humanoid@CrouchForwardLeftS&S.fbx`; RM `Humanoid@CrouchForwardLeftS&S_RM.fbx` | variant=paired-ip-rm | duration=1.667 s; rm_speed=0.750 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Crouch combat 8-way | L `(-1,0)` | IP `Humanoid@CrouchLeftS&S.fbx`; RM `Humanoid@CrouchLeftS&S_RM.fbx` | variant=paired-ip-rm | duration=1.667 s; rm_speed=0.731 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Crouch combat 8-way | BL `(-0.707,-0.707)` | IP `Humanoid@CrouchBackwardsLeftS&S.fbx`; RM `Humanoid@CrouchBackwardsLeftS&S_RM.fbx` | variant=paired-ip-rm | duration=1.667 s; rm_speed=0.701 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Crouch combat 8-way | B `(0,-1)` | IP `Humanoid@CrouchBackwardsS&S.fbx`; RM `Humanoid@CrouchBackwardsS&S_RM.fbx` | variant=paired-ip-rm | duration=1.667 s; rm_speed=0.775 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Crouch combat 8-way | BR `(0.707,-0.707)` | IP `Humanoid@CrouchBackwardsRightS&S.fbx`; RM `Humanoid@CrouchBackwardsRightS&S_RM.fbx` | variant=paired-ip-rm | duration=1.667 s; rm_speed=0.767 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Crouch combat 8-way | R `(1,0)` | IP `Humanoid@CrouchRightS&S.fbx`; RM `Humanoid@CrouchRightS&S_RM.fbx` | variant=paired-ip-rm | duration=1.667 s; rm_speed=0.744 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Crouch combat 8-way | FR IP `(0.707,0.707)` | `Humanoid@CrouchForwardRightS&S.fbx` | variant=in-place | duration=1.667 s | loop=true; sync=gait-phase |
| Crouch combat 8-way | FR RM quarantine | `Humanoid@CrouchForwardRightS&S_RM.fbx` | set_type=other | N/A | state=quarantined; movement=unavailable |
| Draw/combat/put-away 1 | ordered | `Humanoid@DrawWeapons1S&S.fbx`; `Humanoid@IdleCombatS&S.fbx`; `Humanoid@PutBackWeapons1S&S.fbx` | set_type=transition-chain | N/A | transition=at-end; state=armed-combat |
| Draw/combat/put-away 2 | ordered | `Humanoid@DrawWeapons2S&S.fbx`; `Humanoid@IdleCombatS&S.fbx`; `Humanoid@PutBackWeapons2S&S.fbx` | set_type=transition-chain | N/A | transition=at-end; state=armed-combat |

Speed ratios are 1.48× walk, 1.14× run, and 1.13× across seven valid crouch RM members. Preserve directional velocity or tune playback. Phase spreads are 0.770, 0.726, and at least 0.807, so use offsets and visual review.

## Integration recipe

1. **Members/topology:** `topology=separate-2d-blends`; create separate IP and RM graphs at the table coordinates, omit the quarantined Crouch FR RM member, and use the two named equipment chains.
2. **Timing/synchronization:** `sync=runtime-phase-offsets`; loop reviewed locomotion, idle, and dead-hold members only; configure attacks, blocks, parries, reactions, taunts, equipment, death, and recovery as one-shots until author intent is confirmed.
3. **State ownership:** `owner=split-by-movement-variant`; the controller owns IP translation/yaw, animation owns validated RM translation/yaw, and each action gets an explicit movement-owner decision.
4. **Composition constraints:** `composition=full-body-combat-default`; attach sword to the right hand and shield to the left, keep kicks and displacement-bearing attacks full-body, and promote upper-body masks only after pelvis, contact, IK, and grip review.
5. **Acceptance gate:** `gate=target-character-visual-review`; exercise complete rings, wraps, transitions, actions, contacts, root extraction, deformation, and crossfades.

## Technical issue register

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| SS-001 | blocker | [Malformed hierarchy](../game-ready-clips.md#files-disagree-about-skeleton-or-clip-identity) leaves Crouch FR RM without a humanoid clip. | artist-author | Quarantine it and use IP/controller motion or another direction. | Safe hierarchy invention is not possible without author evidence. | AnimSmith two-node hierarchy; Unity no AnimationClip. |
| SS-002 | major | [Incorrect loop declarations](../game-ready-clips.md#the-loop-pops) can replay attacks/reactions and expose hard wraps. | engine-config | Override 52 obvious one-shot-like flags and review every remaining loop. | A metadata-aware declaration audit is feasible; intent cannot be inferred universally. | 118 delivered loop flags; 113 strict seam failures. |
| SS-003 | major | [Gait-phase disagreement](../game-ready-clips.md#feet-skate-when-clips-blend) risks foot skating across all rings. | engine-config | Use runtime offsets or artist-aligned exports. | Basis-safe support is tracked by [#426](https://github.com/mmannerm/animsmith/issues/426). | Phase spreads 0.726–0.807; 24/24 safe refusals. |
| SS-004 | major | [Loop seam derivatives](../game-ready-clips.md#the-loop-pops) remain after the WalkForward duplicate endpoint is dropped. | artist-author | Repair tangents/endpoints or accept a reviewed engine blend. | Duplicate removal is current; generic tangent invention is unsafe. | Sample closure fixed; linear/angular seam failures remain. |
| SS-005 | moderate | [Copied-avatar metadata](../game-ready-clips.md#files-disagree-about-skeleton-or-clip-identity) records a stale pelvis warning in 131 files. | engine-config | Keep the supplied Avatar; verify target-version reimport. | Better diagnostics are feasible; vendor should repair metadata. | Unity imported 131; malformed file failed. |
| SS-006 | moderate | [Low-displacement RM actions](../game-ready-clips.md#the-character-glides-or-runs-in-place) may actually use root yaw or short lunges, so a speed-only rule can assign the wrong movement owner. | engine-config | Inspect displacement and yaw per action before enabling root motion. | Independent displacement/yaw evidence is tracked by [#408](https://github.com/mmannerm/animsmith/issues/408). | Fourteen declared RM actions below the provisional threshold. |
| SS-007 | moderate | [Constant tracks](../game-ready-clips.md#the-file-is-bloated-or-the-retargeter-chokes) are dense; unproved pruning can change sparse-track behavior. | animsmith-current-declared | Retain source tracks until runtime/equivalence gates pass. | Property evidence remains tracked by [#401](https://github.com/mmannerm/animsmith/issues/401) and [#402](https://github.com/mmannerm/animsmith/issues/402). | 16,808 notes; three sample prunes only. |
| SS-008 | moderate | The combined FBX has [extra hierarchy and scale animation](../game-ready-clips.md#why-scale-animation-deserves-its-own-review), weakening atomic use. | artist-author | Use individual FBXs. | Boundaries and scale intent need author evidence. | 60 bones; one scale-key warning. |

## Engine status

| Runtime | Evidence level | Technical result | Remaining gate |
|---|---|---|---|
| Unity 6000.5.8f1 | Co-import + headless Playables/masks/attachments | **Conditional pass:** 131/132 humanoid clips; 8/9 samples, 3/3 blends, 3/3 masks, both props evaluated. | Visual controller, contacts, root motion, retargeting, compression, build. |
| Unreal Engine | Documentation only | **Not evaluated.** Root Motion, Blend Spaces, Sync Groups, and layered blends can express the policy; vendor supplies no native UE package. | FBX import, retarget, complete graphs, contacts, build. |
| Godot | Documentation only | **Not evaluated.** AnimationTree supports BlendSpace2D, filters, one-shots, and root-motion extraction. | Import/conversion, retarget, graph, contacts, build. |
| Bevy | Documentation only | **Not evaluated.** AnimationGraph masks can express layers, but FBX conversion and retargeting remain project work. | glTF conversion, retarget path, graph, root motion, performance. |

## Fit and limitations

Best fit: third-person action RPGs or melee prototypes using a full-body armed state and compatible humanoid rig.

Poor fit: first-person, traversal-heavy, motion-matching, or contact-critical games without authoring. Networking, hit windows, IK, paired opponents, and style remain unevaluated.

For combined use with Basic Locomotion, prefer a full-body state switch: Basic for unarmed exploration and Sword & Shield for armed combat. The partial [Ultimate Animation Collection rollup](protofactor-ultimate-animation-collection.md) owns that cross-pack conclusion.

## Evidence status

All 136 FBXs were inspected; the v1 manifest covers 87 logical motions and 132 files. AnimSmith `0.3.0` at `c11f135ece5e980e6c98861a52a715a28a424ff9` ran baseline, contract, and bounded remediation passes. The [readiness ladder](../game-ready-clips.md#the-readiness-ladder) and [appendix](protofactor-sword-and-shield-evidence.md) define remaining boundaries.

## Sources

- Protofactor, [Sword & Shield product page](https://protofactor.biz/product/animset-sword-shield/), [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/), and [End User License Agreement](https://protofactor.biz/end-user-license-agreement/).
- Unity, [Avatar Mask](https://docs.unity3d.com/es/current/Manual/class-AvatarMask.html), [imported clip masks](https://docs.unity3d.com/es/current/Manual/AnimationMaskOnImportedClips.html), and [loop optimization](https://docs.unity3d.com/es/current/Manual/LoopingAnimationClips.html).
- Epic Games, [Root Motion](https://dev.epicgames.com/documentation/unreal-engine/root-motion-in-unreal-engine?application_version=5.7) and [Layered Animations](https://dev.epicgames.com/documentation/unreal-engine/using-layered-animations-in-unreal-engine?lang=en-US).
- Godot, [Using AnimationTree](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html); Bevy, [Animation Masks](https://bevy.org/examples/animation/animation-masks/).
