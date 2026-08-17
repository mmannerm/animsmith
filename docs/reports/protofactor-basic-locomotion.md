# Animation pack evaluation: Protofactor Basic Locomotion Animset

> Technical verdict: **Usable with conditions**
>
> Evaluation completeness: **partial** — exhaustive AnimSmith 0.2.1 analysis and a Unity 6000.5.8f1 import/playable probe; no Unreal Engine, Godot, Bevy, target-character, or visual blend pass.
>
> Confidence: **medium**
>
> Evaluation date: **2026-08-16**
>
> Report format: **1**
>
> Detailed evidence: [Protofactor Basic Locomotion evidence appendix](protofactor-basic-locomotion-evidence.md)

## Technical decision

This is a strong **third-person locomotion source pack**, not a drop-in controller. It provides in-place and root-motion 8-way walk, run, and crouch rings, idles, and varied turns. Unity imports all 177 individual humanoid clips. Twelve files have negative-time keys, the raw rings are not phase-aligned, and 22/24 clearly cyclic in-place ring members retain strict closure or seam-derivative failures after anchoring, risking wrap pops or foot pulses.

It still requires deliberate controller setup and target-character visual acceptance testing.

AnimSmith 0.2.1 can slice the 12 files and phase-align the three **in-place** rings, but cannot repair loop endpoints/tangents. Its gait-anchor transform resamples accumulating root translation, so use runtime phase offsets or artist-aligned exports for root-motion rings.

The supplied archive was evaluated as authorized input. Current license and historical-provenance boundaries are appendix evidence, not pack defects.

## Capability coverage

### Complete core

- Six 8-way walk, run, and crouch rings: in-place and root-motion (48 files).
- Fourteen standing, crouched, cover, and grenade-aim idle/hold motions.
- Standing/crouched 90°/180° turns, pivots, and U-turns, generally paired. These are **locomotion transitions**, not idles.
- Seventy evidenced in-place/root-motion motion pairs overall. Every pair shares duration and skeleton; 68/70 also share frame count.

### Partial supporting gameplay

- Basic jump/fall/landing coverage without an authored state chain, contacts, or interruption policy.
- Left/right 1 m obstacles without a broader vault, mantle, ledge, or climb family.
- Cover and grenade motions without a tested mask, additive base, socket, IK, or event contract.

### Absent

- Parkour, climbing, broad vaulting, melee, firearm handling, hit reactions, deaths, paired interactions, additive aim offsets, and first-person arms.

Best suited to grounded third-person controllers, not combat-complete, first-person, or traversal-heavy games.

## Runtime sets and authored motion

Each gait shares one cycle duration. Speeds are measured root-motion magnitudes; paired in-place members measure 0 m/s. File stems omit `Humanoid@` and `.fbx`. Directions follow the table order around a 2D blend. `unknown` means loop metadata is absent, not false.

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

Shared durations and nearby directional speeds support these sets. Use measured magnitudes or deliberate project normalization for controller thresholds.

## Integration recipe

1. **Members/topology:** `topology=separate-2d-blends`; build separate in-place and root-motion graphs at the table coordinates.
2. **Timing/synchronization:** `loop=per-member-table`; `sync=gait-phase`. Slice the 12 affected files and anchor only the in-place rings. Enable loops after wrap review; Run L/R remain unknown.
3. **State ownership:** `owner=split-by-movement-variant`; the controller owns in-place translation/yaw; animation owns root-motion translation/yaw. Use runtime phase offsets or artist exports for root-motion rings.
4. **Composition constraints:** `composition=separate-gaits-and-full-body-actions`; never mix movement variants. Treat grenade/cover as full-body until masks, additive bases, sockets, IK, and interruptions are tested.
5. **Acceptance gate:** `gate=target-character-visual-review`; test contacts and wrap. Phase alignment does not repair [foot skating](../game-ready-clips.md#feet-skate-when-clips-blend) or [loop seams](../game-ready-clips.md#the-loop-pops).

## Technical issue register

Owners identify current/future AnimSmith, engine, or artist work.

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| AP-001 | major | [Negative-time keys](../game-ready-clips.md#the-clip-is-the-wrong-length-or-freezes-at-the-end) in 12 files can fail strict pipelines or clamp a pose. | animsmith-current-declared | Slice to the Unity range at 30 fps. | Batch the declared transform. | Verified: 36/36 errors removed. |
| AP-002 | major | [Loop seams](../game-ready-clips.md#the-loop-pops) fail on 22/24 anchored cyclic IP clips, risking pops/pulses. | artist-author | Correct endpoints/tangents or review an engine loop blend. | Generic invention is unsafe. | Exhaustive mechanical result; visual impact untested. |
| AP-003 | major | [Gait-phase disagreement](../game-ready-clips.md#feet-skate-when-clips-blend) can make the three IP rings skate when blended. | animsmith-current-declared | Anchor each declared IP ring. | Existing transform can be batched; contact cleanup remains artistic. | Spreads verified ≤0.094. |
| AP-004 | moderate | [Three skeleton signatures](../game-ready-clips.md#a-limb-is-t-posed-or-a-bone-never-moves) block exact interchange and may change retargeted transitions. | engine-config | Use the supplied Avatar; test 56/73-bone boundaries. | Diagnostics are plausible; deformation-aware retargeting is not. | Avatar references valid; deformation untested. |
| AP-005 | moderate | [Constant tracks](../game-ready-clips.md#the-file-is-bloated-or-the-retargeter-chokes) in every file may affect sparse-track resets/transitions if pruned. | animsmith-current-declared | Retain until runtime and equivalence tests justify pruning. | Property evidence: [#401](https://github.com/mmannerm/animsmith/issues/401), [#402](https://github.com/mmannerm/animsmith/issues/402). | Runtime cost unknown. |
| AP-006 | moderate | Every FBX uses `Take 001`; [cross-file set identity](../game-ready-clips.md#the-readiness-ladder) therefore needs filenames plus a manifest. | animsmith-future-candidate | Keep file-scoped IDs and the manifest. | Deterministic `(file, clip)` grouping is non-destructive and plausible. | 179/179 FBXs observed; no public issue found. |
| AP-009 | moderate | The combined FBX has a [copied-avatar hierarchy mismatch](../game-ready-clips.md#a-limb-is-t-posed-or-a-bone-never-moves) and no authoritative segmentation. | artist-author | Use the 177 individual FBXs. | Tools cannot invent boundaries or hierarchy intent. | Unity 6000.5.8f1. |
| AP-011 | moderate | [Attachment scale](../game-ready-clips.md#attachment-nodes-and-inherited-rest-world-scale) is unavailable, so weapon size/grip/IK is uncertified. | engine-config | Create sockets; test props on the target rig. | Source-node exposure could improve diagnostics. | Animated scale clean; attachments unknown. |
| AP-012 | moderate | [`_RM` speed](../game-ready-clips.md#the-character-glides-or-runs-in-place) does not characterize root yaw; turns may be ignored or doubled. | engine-config | Inspect yaw, extraction, and movement ownership. | Translation/yaw diagnostics are safe; conversion needs a contract. | Yaw/controller untested. |
| AP-013 | major | [RM gait-phase disagreement](../game-ready-clips.md#feet-skate-when-clips-blend) may skate, but 0.2.1 anchoring reorders accumulating translation. | engine-config | Keep trajectories; use runtime offsets or artist exports. | A proved root-preserving cyclic rebase is plausible. | Measured; deliberately untreated. |

## Engine status

| Runtime | Evidence level | Technical result | Remaining gate |
|---|---|---|---|
| Unity 6000.5.8f1 | Import + headless Playables | **Conditional pass:** 177 humanoid clips imported; 6/6 samples and 3/3 pair blends passed. Combined FBX hit AP-009. | Visual/full-ring blends, root motion, masks, retargeting, compression, build. |
| Unreal Engine | Documentation only | **Not evaluated.** Sync Groups and Blend Spaces can express the policy. | Import, retarget, blends, root motion, layers, compression. |
| Godot | Documentation only | **Not evaluated.** AnimationTree can express blends, sync, and filters. | Import/retarget, reset, root motion, masks, build. |
| Bevy | Documentation only | **Not evaluated.** AnimationGraph supports weighted blending. | Conversion, graph, root motion, transitions, performance. |

Unity proves headless evaluation/mixing on the supplied actor, not planted feet, clean loops, or target deformation.

## Fit and limitations

Best fit: grounded third-person controllers needing broad locomotion, turns, and basic cover/grenade/jump/obstacle placeholders.

Caveats: loop polish, phase/root ownership, retargeting, weapon layers, and contacts remain project work. It is incomplete for traversal-, combat-, reaction/death-, first-person, motion-matching, or paired-interaction systems. Cross-pack compatibility remains untested.

## Evidence status

The evaluation covers all 177 individual motion FBXs with AnimSmith 0.2.1 at repository revision `b6d0f9a5b06d8e5f907fbb87dc6d07ec55525b47`, using manifest schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1`. The [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder) defines what the mechanical, clip, set, rig/use, runtime, and acceptance claims mean. Detailed counts, profiles, commands, digests, provenance, and limitations are in the [evidence appendix](protofactor-basic-locomotion-evidence.md).

## Sources

- Protofactor, [Basic Locomotion product page](https://protofactor.biz/product/animset-basic-locomotion/) and [End User License Agreement](https://protofactor.biz/end-user-license-agreement/).
- Unity, [Animation Blend Trees](https://docs.unity3d.com/6000.5/Documentation/Manual/class-BlendTree.html).
- Epic Games, [Animation Sync Groups](https://dev.epicgames.com/documentation/unreal-engine/animation-sync-groups-in-unreal-engine).
- Godot, [Using AnimationTree](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html).
- Bevy, [Animation Graph example](https://bevy.org/examples/animation/animation-graph/).
