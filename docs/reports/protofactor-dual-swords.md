# Animation pack evaluation: Protofactor Dual Swords Animset

> Technical verdict: **Usable with conditions**
>
> Evaluation completeness: **partial** — 0.3.0 baseline/contracts, 0.3.1-bound remediation at `674396f`, and Unity 6000.5.8f1 probes; no output-engine, visual, target-character, or non-Unity pass.
>
> Confidence: **medium**
>
> Evaluation date: **2026-08-17**
>
> Report format: **1**
>
> Detailed evidence: [Protofactor Dual Swords evidence appendix](protofactor-dual-swords-evidence.md)

## Technical decision

Use all 186 individual clips as a full-body dual-sword combat mode. They share one 56-bone structure, import as Unity Humanoid, and sampled paths execute unchanged in Unity.

The b7 baseline exposes unreliable loops, gait-phase disagreement, root-motion unknowns, and dense constant tracks. The 0.3.1-bound `674396f` revision emits 24 in-place gait-anchor candidates and reduces walk/run/crouch phase spreads from 0.709/0.673/0.618 to 0.053/0.135/0.059. Mechanical verification passes, but the GLBs lack engine, visual, and trajectory acceptance. Constant-track pruning also remains experimental.

Replace loop policy, retain offsets for raw/residual phase, and review every RM action's translation/yaw. Artists must accept grips, arcs, hit/cancel windows, contacts, transitions, and deformation. The headless mask is execution evidence only; full-body attacks remain default.

## Capability coverage

### Complete core

- Paired IP/RM walk, run, crouch, and five-speed forward locomotion families.
- Eleven attacks, nineteen combos, spins, parries, blocks, reactions/deaths, equipment transitions, idles, and one sword prop.

### Partial supporting gameplay

- Attacks lack accepted hit/contact/cancel and root-motion contracts.
- Upper-body composition executes headlessly; pelvis motion, support, grips, arcs, and visual continuity remain open.

### Absent

- General starts/stops/turns, airborne and traversal content, paired-character interactions, additive aim, first-person content, and authored motion-matching metadata.

## Runtime sets and authored motion

Coordinates are `(right,forward)`; speeds are measured RM magnitudes. IP/RM counterparts have nearly identical pair phase, but directions within each ring are mutually out of phase.

| Set/profile | Role or coordinate | Exact members | Variant/type | Timing or motion | Runtime contract |
|---|---|---|---|---|---|
| Walk combat 8-way | F `(0,1)` | IP `Humanoid@WalkForwardDualSwords.fbx`; RM `Humanoid@WalkForwardDualSwords_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.765 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Walk combat 8-way | FL `(-0.707,0.707)` | IP `Humanoid@WalkForwardLeftDualSwords.fbx`; RM `Humanoid@WalkForwardLeftDualSwords_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.740 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Walk combat 8-way | L `(-1,0)` | IP `Humanoid@WalkLeftDualSwords.fbx`; RM `Humanoid@WalkLeftDualSwords_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.738 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Walk combat 8-way | BL `(-0.707,-0.707)` | IP `Humanoid@WalkBackwardsLeftDualSwords.fbx`; RM `Humanoid@WalkBackwardsLeftDualSwords_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.717 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Walk combat 8-way | B `(0,-1)` | IP `Humanoid@WalkBackwardsDualSwords.fbx`; RM `Humanoid@WalkBackwardsDualSwords_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.692 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Walk combat 8-way | BR `(0.707,-0.707)` | IP `Humanoid@WalkBackwardsRightDualSwords.fbx`; RM `Humanoid@WalkBackwardsRightDualSwords_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.701 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Walk combat 8-way | R `(1,0)` | IP `Humanoid@WalkRightDualSwords.fbx`; RM `Humanoid@WalkRightDualSwords_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.745 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Walk combat 8-way | FR `(0.707,0.707)` | IP `Humanoid@WalkForwardRightDualSwords.fbx`; RM `Humanoid@WalkForwardRightDualSwords_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.728 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Run combat 8-way | F `(0,1)` | IP `Humanoid@RunForwardDualSwords.fbx`; RM `Humanoid@RunForwardDualSwords_RM.fbx` | variant=paired-ip-rm | duration=0.600 s; rm_speed=2.500 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Run combat 8-way | FL `(-0.707,0.707)` | IP `Humanoid@RunForwardLeftDualSwords.fbx`; RM `Humanoid@RunForwardLeftDualSwords_RM.fbx` | variant=paired-ip-rm | duration=0.600 s; rm_speed=2.475 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Run combat 8-way | L `(-1,0)` | IP `Humanoid@RunLeftDualSwords.fbx`; RM `Humanoid@RunLeftDualSwords_RM.fbx` | variant=paired-ip-rm | duration=0.600 s; rm_speed=2.500 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Run combat 8-way | BL `(-0.707,-0.707)` | IP `Humanoid@RunBackwardsLeftDualSwords.fbx`; RM `Humanoid@RunBackwardsLeftDualSwords_RM.fbx` | variant=paired-ip-rm | duration=0.600 s; rm_speed=2.475 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Run combat 8-way | B `(0,-1)` | IP `Humanoid@RunBackwardsDualSwords.fbx`; RM `Humanoid@RunBackwardsDualSwords_RM.fbx` | variant=paired-ip-rm | duration=0.600 s; rm_speed=2.500 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Run combat 8-way | BR `(0.707,-0.707)` | IP `Humanoid@RunBackwardsRightDualSwords.fbx`; RM `Humanoid@RunBackwardsRightDualSwords_RM.fbx` | variant=paired-ip-rm | duration=0.600 s; rm_speed=2.475 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Run combat 8-way | R `(1,0)` | IP `Humanoid@RunRightDualSwords.fbx`; RM `Humanoid@RunRightDualSwords_RM.fbx` | variant=paired-ip-rm | duration=0.600 s; rm_speed=2.500 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Run combat 8-way | FR `(0.707,0.707)` | IP `Humanoid@RunForwardRightDualSwords.fbx`; RM `Humanoid@RunForwardRightDualSwords_RM.fbx` | variant=paired-ip-rm | duration=0.600 s; rm_speed=2.475 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Crouch combat 8-way | F `(0,1)` | IP `Humanoid@CrouchForwardDualSwords.fbx`; RM `Humanoid@CrouchForwardDualSwords_RM.fbx` | variant=paired-ip-rm | duration=1.667 s; rm_speed=0.732 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Crouch combat 8-way | FL `(-0.707,0.707)` | IP `Humanoid@CrouchForwardLeftDualSwords.fbx`; RM `Humanoid@CrouchForwardLeftDualSwords_RM.fbx` | variant=paired-ip-rm | duration=1.667 s; rm_speed=0.730 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Crouch combat 8-way | L `(-1,0)` | IP `Humanoid@CrouchLeftDualSwords.fbx`; RM `Humanoid@CrouchLeftDualSwords_RM.fbx` | variant=paired-ip-rm | duration=1.667 s; rm_speed=0.732 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Crouch combat 8-way | BL `(-0.707,-0.707)` | IP `Humanoid@CrouchBackwardsLeftDualSwords.fbx`; RM `Humanoid@CrouchBackwardsLeftDualSwords_RM.fbx` | variant=paired-ip-rm | duration=1.667 s; rm_speed=0.730 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Crouch combat 8-way | B `(0,-1)` | IP `Humanoid@CrouchBackwardsDualSwords.fbx`; RM `Humanoid@CrouchBackwardsDualSwords_RM.fbx` | variant=paired-ip-rm | duration=1.667 s; rm_speed=0.732 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Crouch combat 8-way | BR `(0.707,-0.707)` | IP `Humanoid@CrouchBackwardsRightDualSwords.fbx`; RM `Humanoid@CrouchBackwardsRightDualSwords_RM.fbx` | variant=paired-ip-rm | duration=1.667 s; rm_speed=0.730 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Crouch combat 8-way | R `(1,0)` | IP `Humanoid@CrouchRightDualSwords.fbx`; RM `Humanoid@CrouchRightDualSwords_RM.fbx` | variant=paired-ip-rm | duration=1.667 s; rm_speed=0.732 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Crouch combat 8-way | FR `(0.707,0.707)` | IP `Humanoid@CrouchForwardRightDualSwords.fbx`; RM `Humanoid@CrouchForwardRightDualSwords_RM.fbx` | variant=paired-ip-rm | duration=1.667 s; rm_speed=0.730 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Forward speed alternatives | walk-1 | IP `Humanoid@Walk1DualSwords.fbx`; RM `Humanoid@Walk1DualSwords_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.847 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Forward speed alternatives | walk-2 | IP `Humanoid@Walk2DualSwords.fbx`; RM `Humanoid@Walk2DualSwords_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.927 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Forward speed alternatives | jog | IP `Humanoid@JogDualSwords.fbx`; RM `Humanoid@JogDualSwords_RM.fbx` | variant=paired-ip-rm | duration=0.833 s; rm_speed=2.246 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Forward speed alternatives | jog-fast | IP `Humanoid@JogFastDualSwords.fbx`; RM `Humanoid@JogFastDualSwords_RM.fbx` | variant=paired-ip-rm | duration=0.833 s; rm_speed=2.580 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Forward speed alternatives | run-fast | IP `Humanoid@RunFastDualSwords.fbx`; RM `Humanoid@RunFastDualSwords_RM.fbx` | variant=paired-ip-rm | duration=0.567 s; rm_speed=4.588 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Draw/combat/put-away | ordered | `Humanoid@DrawDualSwords.fbx`; `Humanoid@IdleCombatDualSwords.fbx`; `Humanoid@PutBackDualSwords.fbx` | set_type=transition-chain | N/A | transition=at-end; state=armed-combat |

RM speed ratios are 1.10× walk, 1.01× run, 1.003× crouch, and 5.42× across forward-speed candidates. Raw phase spreads are 0.709/0.673/0.618; current IP candidates reduce them to 0.053/0.135/0.059 but still need runtime offsets and acceptance. Attack/combo groupings remain inventory alternatives because hit, branch, and cancellation contracts are absent.

## Integration recipe

1. **Members/topology:** `topology=separate-ip-rm-combat-graphs`; build the three 8-way graphs and forward-speed graph from the exact table members.
2. **Timing/synchronization:** `sync=runtime-phase-offsets`; offset raw clips and transformed residuals; loop reviewed locomotion/idles and make actions one-shots.
3. **State ownership:** `owner=split-by-movement-variant`; the controller owns IP translation/yaw, animation owns accepted RM translation/yaw, and every RM action gets an explicit owner decision.
4. **Composition constraints:** `composition=full-body-dual-weapon-default`; attach one sword per hand and allow masks only after pelvis, support, grip, arc, and target-character review.
5. **Acceptance gate:** `gate=target-character-combat-review`; test complete rings, wraps, draw/put-away, attacks/combos, blocking/parries, contacts, root extraction, masks, deformation, compression, and builds.

## Technical issue register

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| DS-001 | major | [Incorrect loop declarations](../game-ready-clips.md#the-loop-pops) mark 88 obvious attacks, combos, reactions, and recoveries as loops, risking repeated actions and hard wraps. | engine-config | Override one-shot-like flags and review the remaining loop candidates. | A metadata/role-aware audit is feasible; universal intent inference is not. | 168/186 delivered loop flags; 162 declared-loop contract failures. |
| DS-002 | major | [Gait-phase disagreement](../game-ready-clips.md#feet-skate-when-clips-blend) risks foot skating. | animsmith-current-declared | Retain offsets; engine-test 24 IP candidates before adoption. | [#426](https://github.com/mmannerm/animsmith/issues/426) is implemented; further work requires declared residual policy and proof. | Raw 0.618–0.709 → post 0.053–0.135; 24/24 outputs; engine/visual gate open. |
| DS-003 | moderate | [RM action ownership](../game-ready-clips.md#the-character-glides-or-runs-in-place) is not established by `_RM` or horizontal speed alone, risking doubled or missing displacement/yaw. | engine-config | Inspect translation and yaw per attack/reaction before enabling root motion. | Independent translation/yaw evidence is tracked by [#408](https://github.com/mmannerm/animsmith/issues/408). | 39 RM action/reaction files; 12 measure below 0.1 m/s; yaw not summarized. |
| DS-004 | major | Attacks and combos lack accepted hit, cancel, grip, contact, and event timing, so gameplay may miss, clip, or feel unresponsive. Guidance: not applicable. | artist-author | Author project events and visually review both weapon arcs and contacts. | Tooling can preserve declared metadata, but cannot infer combat intent or correct arcs. | 30 attack/combo choices; headless samples only. |
| DS-005 | moderate | [Dense constant tracks](../game-ready-clips.md#the-file-is-bloated-or-the-retargeter-chokes) may waste memory; unproved pruning can change sparse-track behavior. | animsmith-current-declared | Keep sources until runtime/equivalence gates pass. | Current pruning works mechanically; stronger proof remains tracked by [#401](https://github.com/mmannerm/animsmith/issues/401) and [#402](https://github.com/mmannerm/animsmith/issues/402). | 25,167 contract notes; one verified export candidate. |

## Engine status

| Runtime | Evidence level | Technical result | Remaining gate |
|---|---|---|---|
| Unity 6000.5.8f1 | Eight-pack co-import + headless Playables/mask/attachments | **Conditional pass:** 186/186 individual clips are Humanoid; seven samples, two cross-pack mixers, one mask, and both hand attachments pass. | Visual controller, contacts, grips, root motion, retargeting, compression, build. |
| Unreal Engine | Documentation only | **Not evaluated.** Root Motion, Blend Spaces, montages, and layered blends can express the policy; no native UE package is delivered. | FBX import, retarget, graphs, contacts, build. |
| Godot | Documentation only | **Not evaluated.** AnimationTree can express blend spaces, one-shots, filters, and root extraction. | Import/conversion, retarget, graphs, contacts, export. |
| Bevy | Documentation only | **Not evaluated.** AnimationGraph masks exist; FBX conversion and retargeting remain project work. | glTF conversion, retarget path, graph, root motion, performance. |

## Fit and limitations

Best fit: third-person action RPGs or melee prototypes that want a broad full-body dual-wield combat vocabulary and can supply controller, event, contact, and artistic acceptance work.

Poor fit: first-person, traversal-heavy, motion-matching, or network-root-motion projects without more content and validation; layered combat systems that require attacks to leave the pelvis and legs untouched.

The 56-bone structure matches Basic Locomotion, Sword & Shield, and the 1-Handed majority; every 25-path overlap with each evaluated collection constituent is byte-identical. Unity mixers and a Basic-locomotion mask execute, but use full-body state handoffs between weapon modes until style, pose, grips, and contacts are visually accepted. The [partial collection rollup](protofactor-ultimate-animation-collection.md) owns the cross-pack conclusion.

## Evidence status

All 189 FBXs were analyzed; the v1 manifest covers 112 logical motions and 186 individual files. Baseline/contracts remain the b7 snapshot; current `674396f` reran gait remediation only. The [readiness ladder](../game-ready-clips.md#the-readiness-ladder) and [appendix](protofactor-dual-swords-evidence.md) define the boundary. The local archive came from the user's Protofactor Ultimate collection download; current pages do not prove its constituent revision or historical terms.

## Sources

- Protofactor, [Dual Swords product page](https://protofactor.biz/product/animset-dual-swords/), [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/), and [End User License Agreement](https://protofactor.biz/end-user-license-agreement/).
- Unity, [Avatar Mask](https://docs.unity3d.com/6000.0/Documentation/Manual/class-AvatarMask.html), [Animation Layers](https://docs.unity3d.com/6000.0/Documentation/Manual/AnimationLayers.html), and [Root Motion](https://docs.unity3d.com/6000.5/Documentation/Manual/RootMotion.html).
- Epic Games, [Root Motion](https://dev.epicgames.com/documentation/unreal-engine/root-motion-in-unreal-engine?application_version=5.7) and [Layered Animations](https://dev.epicgames.com/documentation/unreal-engine/using-layered-animations-in-unreal-engine?lang=en-US); Godot, [AnimationTree](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html); Bevy, [Animation Masks](https://bevy.org/examples/animation/animation-masks/).
