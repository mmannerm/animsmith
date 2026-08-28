# Animation pack evaluation: Protofactor Basic Locomotion Animset

> Technical verdict: **Usable with conditions**
>
> Evaluation completeness: **partial** — complete AnimSmith 0.7.0 baseline, contracts, remediation, addressability, and bounded advice plus a dated Unity probe; no target-character, visual-blend, or current candidate engine pass.
>
> Confidence: **medium**
>
> Evaluation date: **2026-08-26**
>
> Current evaluator: **AnimSmith 0.7.0**
>
> Report format: **2**
>
> Detailed evidence: [Protofactor Basic Locomotion evidence appendix](protofactor-basic-locomotion-evidence.md)

## Technical decision

This is a strong **third-person locomotion source pack**, not a drop-in controller: in-place and root-motion 8-way walk/run/crouch rings, idles, and varied turns. Unity imports all 177 humanoid clips. Twelve files have negative-time keys, raw rings are not phase-aligned, and 22/24 cyclic in-place members fail strict closure or seam checks (AP-002), risking wrap pops.

Directional root-motion speeds span 1.35–1.49× per gait, with diagonals faster than forward (AP-014). Hands measure about 0.01 scale across all 179 FBXs (AP-011); verify weapon/socket compensation in engine.

AnimSmith 0.7.0 slices the 12 negative-time files cleanly and anchors all 24 in-place ring members via a measured vertical yaw heading axis (AP-003). The current GLB candidates stay unpromoted pending a target-character visual gate.

Root trajectory is sampled on all 179 clips as a grid fact, not continuous-curve or extraction proof (AP-012). Movement ownership remains a controller decision.

## Capability coverage

### Complete core

- Six 8-way walk/run/crouch rings: in-place and root-motion (48 files).
- Fourteen standing, crouched, cover, and grenade-aim idle/hold motions.
- Standing/crouched 90°/180° turns, pivots, and U-turns, generally paired: **transitions**, not idles.
- Seventy evidenced in-place/root-motion pairs; every pair shares duration and skeleton, 68/70 also frame count.

### Partial supporting gameplay

- Basic jump/fall/landing without an authored state chain, contacts, or interruption policy.
- Left/right 1 m obstacles without a broader vault, mantle, or climb family.
- Cover and grenade motions without a tested mask, additive base, socket, IK, or event contract.

### Absent

- Parkour, climbing, vaulting, melee, firearms, hit reactions, deaths, paired interactions, aim offsets, and first-person arms.

## Runtime sets and authored motion

Each gait shares one duration. Speeds are measured root-motion magnitudes; in-place partners measure 0 m/s. File stems omit `Humanoid@` and `.fbx`. `unknown` means loop metadata is absent, not false.

| Set/profile | Role or coordinate | Exact members | Variant/type | Timing or motion | Runtime contract |
|---|---|---|---|---|---|
| Walk | F `(0,1)` | IP `WalkForwardUnarmed2`; RM `WalkForwardUnarmed2_RM` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.854 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Walk | FL `(-0.707,0.707)` | IP `WalkForwardLeftUnarmed`; RM `WalkForwardLeftUnarmed_RM` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.975 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Walk | L `(-1,0)` | IP `WalkLeftUnarmed`; RM `WalkLeftUnarmed_RM` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.715 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Walk | BL `(-0.707,-0.707)` | IP `WalkBackwardsLeftUnarmed`; RM `WalkBackwardsLeftUnarmed_RM` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.797 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Walk | B `(0,-1)` | IP `WalkBackwardsUnarmed`; RM `WalkBackwardsUnarmed_RM` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.763 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Walk | BR `(0.707,-0.707)` | IP `WalkBackwardsRightUnarmed`; RM `WalkBackwardsRightUnarmed_RM` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.837 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Walk | R `(1,0)` | IP `WalkRightUnarmed`; RM `WalkRightUnarmed_RM` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.655 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Walk | FR `(0.707,0.707)` | IP `WalkForwardRightUnarmed`; RM `WalkForwardRightUnarmed_RM` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.814 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Run | F `(0,1)` | IP `RunForward2Unarmed`; RM `RunForward2Unarmed_RM` | variant=paired-ip-rm | duration=0.667 s; rm_speed=3.010 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Run | FL `(-0.707,0.707)` | IP `RunForwardLeftUnarmed`; RM `RunForwardLeftUnarmed_RM` | variant=paired-ip-rm | duration=0.667 s; rm_speed=3.048 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Run | L `(-1,0)` | IP `RunLeftUnarmed`; RM `RunLeftUnarmed_RM` | variant=paired-ip-rm | duration=0.667 s; rm_speed=2.900 m/s | loop_ip=unknown; loop_rm=unknown; sync=gait-phase |
| Run | BL `(-0.707,-0.707)` | IP `RunBackwardsLeftUnarmed`; RM `RunBackwardsLeftUnarmed_RM` | variant=paired-ip-rm | duration=0.667 s; rm_speed=2.636 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Run | B `(0,-1)` | IP `RunBackwardsUnarmed`; RM `RunBackwardsUnarmed_RM` | variant=paired-ip-rm | duration=0.667 s; rm_speed=2.100 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Run | BR `(0.707,-0.707)` | IP `RunBackwardsRightUnarmed`; RM `RunBackwardsRightUnarmed_RM` | variant=paired-ip-rm | duration=0.667 s; rm_speed=2.623 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Run | R `(1,0)` | IP `RunRightUnarmed`; RM `RunRightUnarmed_RM` | variant=paired-ip-rm | duration=0.667 s; rm_speed=2.889 m/s | loop_ip=unknown; loop_rm=unknown; sync=gait-phase |
| Run | FR `(0.707,0.707)` | IP `RunForwardRightUnarmed`; RM `RunForwardRightUnarmed_RM` | variant=paired-ip-rm | duration=0.667 s; rm_speed=2.875 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Crouch | F `(0,1)` | IP `CrouchForwardUnarmed`; RM `CrouchForwardUnarmed_RM` | variant=paired-ip-rm | duration=1.500 s; rm_speed=0.760 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Crouch | FL `(-0.707,0.707)` | IP `CrouchForwardLeftUnarmed`; RM `CrouchForwardLeftUnarmed_RM` | variant=paired-ip-rm | duration=1.500 s; rm_speed=0.914 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Crouch | L `(-1,0)` | IP `CrouchLeftUnarmed`; RM `CrouchLeftUnarmed_RM` | variant=paired-ip-rm | duration=1.500 s; rm_speed=0.841 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Crouch | BL `(-0.707,-0.707)` | IP `CrouchBackwardsLeftUnarmed`; RM `CrouchBackwardsLeftUnarmed_RM` | variant=paired-ip-rm | duration=1.500 s; rm_speed=0.820 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Crouch | B `(0,-1)` | IP `CrouchBackwardsUnarmed`; RM `CrouchBackwardsUnarmed_RM` | variant=paired-ip-rm | duration=1.500 s; rm_speed=0.683 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Crouch | BR `(0.707,-0.707)` | IP `CrouchBackwardsRightUnarmed`; RM `CrouchBackwardsRightUnarmed_RM` | variant=paired-ip-rm | duration=1.500 s; rm_speed=0.792 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Crouch | R `(1,0)` | IP `CrouchRightUnarmed`; RM `CrouchRightUnarmed_RM` | variant=paired-ip-rm | duration=1.500 s; rm_speed=0.839 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |
| Crouch | FR `(0.707,0.707)` | IP `CrouchForwardRightUnarmed`; RM `CrouchForwardRightUnarmed_RM` | variant=paired-ip-rm | duration=1.500 s; rm_speed=0.922 m/s | loop_ip=true; loop_rm=true; sync=gait-phase |

Equal durations make these stride-length differences: walk spans 0.655–0.975 m/s (1.49×), run 2.100–3.048 m/s (1.45×), and crouch 0.683–0.922 m/s (1.35×), each with a diagonal faster than forward. Not necessarily defective, but one gait-wide speed may cause foot slide or travel errors; preserve per-direction velocities/thresholds, tune playback, or request artist re-timing.

## Integration recipe

1. **Members/topology:** `topology=separate-2d-blends`; build separate in-place/root-motion graphs at the table coordinates. Treat each RM speed as authored evidence, not one gait-wide speed.
2. **Timing/synchronization:** `loop=per-member-table`; `sync=gait-phase`. Slice the 12 affected files; 0.7.0 anchors all 24 in-place members (AP-003), but keep those candidates ungated — use runtime phase offsets or artist exports as fallback. Enable loops after wrap review; Run L/R remain unknown.
3. **State ownership:** `owner=validate-per-axis`; controller owns in-place translation; validate RM ownership per axis, since sampled RM clips bake root rotation. Preserve trajectories when offsetting.
4. **Composition constraints:** `composition=separate-gaits-and-full-body-actions`; never mix movement variants. Treat grenade/cover as full-body until masks, additive bases, sockets, IK, and interruptions are tested.
5. **Acceptance gate:** `gate=target-character-visual-review`; test contacts, wrap, and travel in all eight directions. Phase alignment does not repair [foot skating](../game-ready-clips.md#feet-skate-when-clips-blend) or [loop seams](../game-ready-clips.md#the-loop-pops).

## Technical issue register

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| AP-001 | major | [Negative-time keys](../game-ready-clips.md#the-clip-is-the-wrong-length-or-freezes-at-the-end) in 12 files can fail strict pipelines or clamp a pose. | animsmith-current-declared | Slice to the Unity range, 30 fps. | Batch the declared transform. | Verified: 36/36 errors removed. |
| AP-002 | major | [Loop seams](../game-ready-clips.md#the-loop-pops) fail on 22/24 raw cyclic IP clips, risking pops/pulses. | artist-author | Correct endpoints/tangents or review an engine blend. | Generic invention is unsafe. | Exhaustive; visual impact untested. |
| AP-003 | major | [Gait-phase disagreement](../game-ready-clips.md#feet-skate-when-clips-blend) may cause blend skating; 0.7.0 anchors all 24 members (spread ~0.05–0.09) via a vertical yaw axis. | engine-config | Validate the 24 candidates in engine/visually; keep runtime/artist exports as fallback. | Basis-safe anchoring is available; a rebase still needs visual proof. | 24/24 anchored; unpromoted, no Humanoid-retarget or visual import. |
| AP-004 | moderate | [Three skeleton signatures](../game-ready-clips.md#files-disagree-about-skeleton-or-clip-identity) block exact interchange; may change transitions. | engine-config | Use the supplied Avatar; test 56/73-bone boundaries. | Diagnostics plausible; retargeting is not. | Avatar references valid; deformation untested. |
| AP-005 | moderate | [Constant tracks](../game-ready-clips.md#the-file-is-bloated-or-the-retargeter-chokes) may affect sparse-track resets/transitions if pruned. | animsmith-current-declared | Retain until runtime/equivalence tests justify pruning. | Per-bone channel coverage is available; [#401](https://github.com/mmannerm/animsmith/issues/401) limits broad pruning adoption. | Runtime cost unknown. |
| AP-006 | moderate | Every FBX uses `Take 001`; [cross-file set identity](../game-ready-clips.md#files-disagree-about-skeleton-or-clip-identity) needs file-scoped identities plus a manifest. | engine-config | Keep the published file-scoped IDs and author a current collection manifest from source authority. | Current collection manifests and runtime sets support this layout; this report's legacy manifest is not a current collection binding. | 179/179 FBXs observed. |
| AP-009 | moderate | The combined FBX has a [copied-avatar hierarchy mismatch](../game-ready-clips.md#files-disagree-about-skeleton-or-clip-identity) and no authoritative segmentation. | artist-author | Use the 177 individual FBXs. | Tools cannot invent boundaries. | Unity 6000.5.8f1. |
| AP-011 | moderate | [Attachment scale](../game-ready-clips.md#attachment-nodes-and-inherited-rest-world-scale) is about 0.01 at both hands; an uncompensated weapon may be 100× too small. | engine-config | Test sockets/prop compensation on the target rig. | Source-node evidence available. | 358 warnings; engine untested. |
| AP-012 | moderate | [`_RM` speed](../game-ready-clips.md#the-character-glides-or-runs-in-place) does not characterize root yaw; turns may be ignored or doubled. | engine-config | Inspect yaw, extraction, and movement ownership. | Per-clip displacement and yaw evidence are available; continuous-curve extraction proof is not. | 71/179 move >1 cm, 107 stationary, 21 show >1° yaw (sampled grid); controller untested. |
| AP-013 | major | [RM gait-phase disagreement](../game-ready-clips.md#feet-skate-when-clips-blend) may skate; resampling must preserve translation/yaw. | engine-config | Keep trajectories; use runtime/artist exports. | Current safety policy refuses unsafe rewrites; rebase needs separate proof. | Publishes no unsafe output. |
| AP-014 | major | [Directional speed/stride variation](../game-ready-clips.md#directional-blend-members-travel-at-different-speeds) spans 1.35–1.49×; one gait-wide speed can cause travel or slide. | engine-config | Preserve per-direction speeds/thresholds, tune playback, or request artist re-timing. | Current declared-set policies can lint a supplied controller contract; auto re-timing remains unsafe. | Exact measurements; no current collection-policy binding or visual test. |

## Engine status

| Runtime | Evidence level | Technical result | Remaining gate |
|---|---|---|---|
| Unity 6000.5.8f1 | Observed import + Playables; 0.7.0 advice | **Conditional pass:** 177 clips imported; 6/6 samples and 3/3 blends passed; the combined FBX hit AP-009. Advice is `available` and matches observed locks: IP bakes, while 31/36 RM clips extract XZ. | Visual blends, root motion, masks, retargeting, build. |
| Unreal Engine | Documentation only, 0.7.0 advice (UE 5.8) | **Not evaluated in-engine.** Current revision-2 settings projection is available; no engine process ran. | Import, retarget, blends, root motion, layers. |
| Godot | Documentation only, 0.7.0 advice (4.7) | **Not evaluated in-engine.** Current revision-2 settings projection is available; no engine process ran. | Import/retarget, reset, root motion, masks. |
| Bevy | Documentation only, 0.7.0 addressability probe | **Not evaluated.** Addressability exit 0 on a generated GLB: 1 clip, selector `Animation0`, 0 findings; selector prediction only. | Conversion, graph, root motion, playback. |

The Unity probe proves headless mixing only; 0.7.0 advice is a metadata prediction only.

## Fit and limitations

Best fit: grounded third-person controllers needing broad locomotion and action placeholders.

Caveats: loop polish, phase/root ownership, retargeting, weapon layers, and contacts remain project work; incomplete for traversal, combat, reaction/death, first-person, or paired-interaction systems. Cross-pack compatibility remains untested.

## Changes between AnimSmith versions

AnimSmith 0.7.0 reproduced the 179-FBX baseline and 177 contracts (58 pass / 119 fail). Its 39 candidates include V1 and rich V2 addressability. Projections do not prove engine acceptance.

AnimSmith 0.4.0 added the successful 12-file slicing and 24-member in-place gait anchoring used by the current recipe. AnimSmith 0.3.0 refused those gait trials, and the 0.2.1 candidates are superseded.

## Evidence status

Current evidence is the exact 0.7.0 rerun plus the dated Unity 6000.5.8f1 observation. See the [readiness ladder](../game-ready-clips.md#the-readiness-ladder) and [appendix](protofactor-basic-locomotion-evidence.md).

## Sources

- Protofactor, [product page](https://protofactor.biz/product/animset-basic-locomotion/) and [EULA](https://protofactor.biz/end-user-license-agreement/).
- Unity, [Blend Trees](https://docs.unity3d.com/6000.5/Documentation/Manual/class-BlendTree.html).
- Epic Games, [Sync Groups](https://dev.epicgames.com/documentation/unreal-engine/animation-sync-groups-in-unreal-engine).
- Godot, [AnimationTree](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html).
- Bevy, [Animation Graph example](https://bevy.org/examples/animation/animation-graph/).
