# Animation pack evaluation: Protofactor Sword & Shield Animset

> Technical verdict: **Usable with conditions**
>
> Evaluation completeness: **partial** — AnimSmith 0.4.0 re-run (baseline, contracts, gait-anchor) on a byte-identical source, retained 2026-08-17 Unity probes, and new engine-advisory checks; no visual controller, target character, or non-Unity pass.
>
> Confidence: **medium**
>
> Evaluation date: **2026-08-21**
>
> Report format: **1**
>
> Detailed evidence: [Protofactor Sword & Shield evidence appendix](protofactor-sword-and-shield-evidence.md)

## Technical decision

Use the individual FBXs as a **third-person sword-and-shield combat pack**, after quarantining `Humanoid@CrouchForwardRightS&S_RM.fbx` and replacing the delivered loop policy. Unity imports 131/132 humanoid clips; the quarantined two-node file yields no AnimationClip.

The 28 locomotion motions supply IP/RM variants. 0.3.0 refused all 24 IP gait-anchor trials on an unmeasurable root basis; 0.4.0 resolves that, measures a vertical (`positive_y`) heading, and anchors all 24, cutting spread from 0.723/0.661/0.697 to 0.060/0.137/0.052. Candidates are unpromoted — no Humanoid-retarget or visual import — keep offsets until gated. Duplicate-endpoint removal fixes only WalkForward closure, not seam derivatives.

Unity metadata marks 118/132 files as loops, including 52 attack, defense, reaction, and taunt files that should start as one-shots. AnimSmith reports the consequences and can prune experimentally, but cannot reconstruct hierarchy, contacts, or every RM action's translation/yaw intent. Both props attach at plausible hand-local scale; grip, contacts, deformation, and quality remain unproved.

## Capability coverage

### Complete core

- Eight-direction walk/run/crouch combat locomotion in IP/RM variants, except quarantined Crouch FR RM.
- Melee, defense, reaction/death, equipment, idle, and taunt families.
- Props and a Unity Humanoid actor share one 56-bone signature.

### Partial supporting gameplay

- Equipment and death/recovery chains without events, interruption, or crossfade acceptance.
- Three Unity mask graphs evaluated; pelvis, kicks, contacts, IK, and prop alignment remain unaccepted.
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

Speed ratios are 1.48× walk, 1.14× run, and 1.13× across seven valid crouch RM members; preserve velocity or tune playback. Circular spreads fall 0.723/0.661/0.697 → 0.060/0.137/0.052 (walk/run/crouch) under 0.4.0's unpromoted anchor trial; use offsets or review until gated.

## Integration recipe

1. **Members/topology:** `topology=separate-2d-blends`; build separate IP/RM graphs at the table coordinates, omit quarantined Crouch FR RM, and use the two equipment chains.
2. **Timing/synchronization:** `sync=runtime-phase-offsets`; loop reviewed locomotion/idle/dead-hold members only; treat attacks, blocks, parries, reactions, taunts, equipment, and death/recovery as one-shots until confirmed.
3. **State ownership:** `owner=validate-per-axis`; controller owns IP translation; validate RM ownership per axis, since sampled RM clips bake root rotation.
4. **Composition constraints:** `composition=full-body-combat-default`; attach sword right-hand and shield left-hand, keep kicks/displacement attacks full-body, and promote masks only after pelvis/contact/IK/grip review.
5. **Acceptance gate:** `gate=target-character-visual-review`; exercise complete rings, wraps, transitions, actions, contacts, root extraction, deformation, and crossfades.

## Technical issue register

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| SS-001 | blocker | [Malformed hierarchy](../game-ready-clips.md#files-disagree-about-skeleton-or-clip-identity) leaves Crouch FR RM without a clip. | artist-author | Quarantine it; use IP/controller motion or another direction. | Safe hierarchy invention is not possible without author evidence. | Two-node hierarchy; Unity no AnimationClip. |
| SS-002 | major | [Incorrect loop declarations](../game-ready-clips.md#the-loop-pops) can replay attacks/reactions and expose hard wraps. | engine-config | Override the 52 obvious one-shot flags; review every remaining loop. | Metadata-aware audits are feasible; intent isn't universally inferable. | 118 delivered loop flags; 113 strict seam failures. |
| SS-003 | major | [Gait-phase disagreement](../game-ready-clips.md#feet-skate-when-clips-blend) risks foot skating across all rings. | animsmith-current-declared | Engine-test the 24 anchored IP candidates before adoption; keep offsets meanwhile. | [#426](https://github.com/mmannerm/animsmith/issues/426) is implemented; RM anchoring remains open. | Spreads 0.723/0.661/0.697 → 0.060/0.137/0.052 (walk/run/crouch); 24/24 anchor; unpromoted. |
| SS-004 | major | [Loop seam derivatives](../game-ready-clips.md#the-loop-pops) remain after the WalkForward duplicate endpoint is dropped. | artist-author | Repair tangents/endpoints or accept a reviewed blend. | Duplicate removal is current; tangent invention is unsafe. | Closure fixed; linear/angular seam failures remain. |
| SS-005 | moderate | [Copied-avatar metadata](../game-ready-clips.md#files-disagree-about-skeleton-or-clip-identity) records a stale pelvis warning in 131 files. | engine-config | Keep the supplied Avatar; verify target-version reimport. | Better diagnostics are feasible; vendor should repair metadata. | Unity imports 131; malformed file fails. |
| SS-006 | moderate | [Low-displacement RM actions](../game-ready-clips.md#the-character-glides-or-runs-in-place) may use root yaw or short lunges, so a speed-only rule can misassign movement owner. | engine-config | Inspect displacement and yaw per action before enabling root motion. | Displacement/yaw evidence is tracked by [#408](https://github.com/mmannerm/animsmith/issues/408). | Fourteen declared RM actions below the provisional threshold. |
| SS-007 | moderate | [Constant tracks](../game-ready-clips.md#the-file-is-bloated-or-the-retargeter-chokes) are dense; unproved pruning can change sparse-track behavior. | animsmith-current-declared | Retain source tracks until runtime/equivalence gates pass. | Tracked by [#401](https://github.com/mmannerm/animsmith/issues/401) and [#402](https://github.com/mmannerm/animsmith/issues/402). | 16,808 notes; three sample prunes only. |
| SS-008 | moderate | The combined FBX has [extra hierarchy and scale animation](../game-ready-clips.md#why-scale-animation-deserves-its-own-review), weakening atomic use. | artist-author | Use individual FBXs. | Boundaries/scale intent need author evidence. | 60 bones; one scale-key warning. |

## Engine status

| Runtime | Evidence level | Technical result | Remaining gate |
|---|---|---|---|
| Unity 6000.5.8f1 | Co-import probes (2026-08-17, retained); 0.4.0 `import-advice` (6000.3) | **Conditional pass:** 131/132 humanoid clips; 8/9 samples, 3/3 blends, 3/3 masks, both props. 0.4.0 advice (exit 0) matches **observed** locks: IP bakes; RM mostly extracts XZ (31/36). | Visual controller, contacts, root motion, retargeting, compression, build. |
| Unreal Engine | Documentation; 0.4.0 `import-advice` | **Not evaluated.** Typed refusal (exit 1); no native UE package. | FBX import, retarget, complete graphs, contacts, build. |
| Godot | Documentation; 0.4.0 `import-advice` | **Not evaluated.** Typed refusal (exit 1). | Import/conversion, retarget, graph, contacts, build. |
| Bevy | 0.4.0 `addressability`, generated GLB | **Advisory pass:** exit 0; selector `Animation0`, 0 findings; inventory only. | glTF loading, targets, graph wiring, playback, root motion, performance. |

## Fit and limitations

Best fit: third-person action RPGs or melee prototypes with a full-body armed state and compatible humanoid rig.

Poor fit: first-person, traversal-heavy, motion-matching, or contact-critical games without authoring. Networking, hit windows, IK, and style remain unevaluated.

Combine with Basic Locomotion as a full-body armed/unarmed state switch; the partial [Ultimate Animation Collection rollup](protofactor-ultimate-animation-collection.md) owns that cross-pack conclusion.

## Evidence status

All 136 FBXs were inspected; the v1 manifest covers 87 logical motions and 132 files. A 2026-08-21 re-inventory reproduces the manifest under AnimSmith `0.4.0` (`6b37ad636b198ef8ff47fadbf6a3a51eb1a27c8e`), re-running baseline/contract/gait-anchor/remediation on the byte-identical source; 2026-08-17 `0.3.0` Unity probes remain historical. The [readiness ladder](../game-ready-clips.md#the-readiness-ladder) and [appendix](protofactor-sword-and-shield-evidence.md) define remaining boundaries.

## Sources

- Protofactor, [Sword & Shield product page](https://protofactor.biz/product/animset-sword-shield/), [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/), and [End User License Agreement](https://protofactor.biz/end-user-license-agreement/).
- Unity, [Avatar Mask](https://docs.unity3d.com/es/current/Manual/class-AvatarMask.html), [imported clip masks](https://docs.unity3d.com/es/current/Manual/AnimationMaskOnImportedClips.html), and [loop optimization](https://docs.unity3d.com/es/current/Manual/LoopingAnimationClips.html).
- Epic Games, [Root Motion](https://dev.epicgames.com/documentation/unreal-engine/root-motion-in-unreal-engine?application_version=5.7) and [Layered Animations](https://dev.epicgames.com/documentation/unreal-engine/using-layered-animations-in-unreal-engine?lang=en-US).
- Godot, [Using AnimationTree](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html); Bevy, [Animation Masks](https://bevy.org/examples/animation/animation-masks/).
