# Animation pack evaluation: Protofactor Ultimate Animation Collection (partial: eight packs)

> Technical verdict: **Usable with conditions**
>
> Evaluation completeness: **partial** — Basic Locomotion, Sword & Shield, Campfire, Climbing, Injured, 1-Handed Melee, 2-Handed Melee, and Dual Swords are evaluated; eight-pack Unity co-install evidence passes, while visual acceptance and 15 currently listed constituents are absent.
>
> Confidence: **medium**
>
> Evaluation date: **2026-08-17**
>
> Report format: **1**
>
> Detailed evidence: [partial collection evidence appendix](protofactor-ultimate-animation-collection-evidence.md)

## Technical decision

Adopt the eight evaluated constituents as explicit full-body gameplay modes around [Basic Locomotion](protofactor-basic-locomotion.md): [Sword & Shield](protofactor-sword-and-shield.md), [1-Handed Melee](protofactor-one-handed-melee.md), [2-Handed Melee](protofactor-two-handed-melee.md), and [Dual Swords](protofactor-dual-swords.md) for distinct armed stances; [Injured](protofactor-injured.md) for seven injury styles; [Campfire](protofactor-campfire.md) for rest/interactions; and [Climbing](protofactor-climbing.md) for environment traversal. Use authored or reviewed state handoffs; do not merge same-role clips into one universal graph merely because their rigs retarget in one engine.

Technical co-existence is strong. The rollup contains 582 logical motions and 895 individual files. Each of the 28 pack pairs has exactly 25 overlapping package paths, all byte-identical, with no conflicts. Most standard motions share the same 56-bone structure; the dominant 2-Handed family adds left/right forearm twists and uses capitalized bone identifiers. One Unity 6000.5.8f1 project imports all eight packs. The latest melee probe passes 33/33 required sampling, mixing, masking, and prop checks, while four expected Generic-rig failures are kept separate; the earlier contextual probe retains its 22/22 result.

AnimSmith's 0.3.1 gait fix materially improves the three new melee packs' selected in-place rings: 72/72 gait-anchor transforms emit candidates. Circular phase spread falls from 0.554/0.734/0.714 to 0.063903/0.108198/0.039432 for 1-Handed walk/run/crouch; 0.711/0.602/0.580 to 0.069337/0.142914/0.053758 for 2-Handed; and 0.709/0.673/0.618 to 0.052993/0.135051/0.058715 for Dual Swords. Root-motion members were not transformed. The GLB candidates remain external and unpromoted because the Unity project has no GLB importer, and no visual or independent trajectory acceptance ran.

This does not make the collection acceptance-ready. Residual seam/phase issues remain, weapon contacts and action timing need gameplay validation, Climbing needs environment and vertical-root validation, Campfire needs prop/contact authoring, and six files are quarantined or excluded. The 15 unevaluated constituents prevent any collection-wide quality or value verdict.

## Capability coverage

### Complete core

- Broad unarmed locomotion, four armed melee styles, injured locomotion/postures, camp states/interactions, and wall/ladder/obstacle traversal are present.
- Eight packs co-install without conflicting shared assets; accepted motion families use either the common 56-bone rig or the Unity-Humanoid-compatible 58-bone 2-Handed twist variant.

### Partial supporting gameplay

- Full-body transitions between modes and selected upper-body masks execute headlessly, but none is visually accepted.
- Props exist for all four weapon modes and part of Campfire; contacts, events, IK, displacement authority, and cancellation remain game-owned.
- Current AnimSmith produces lower-spread gait candidates for the three new melee packs' 72 selected IP ring members; residual runtime offsets and engine/visual acceptance remain required.

### Absent

- Fifteen currently listed constituents remain unevaluated, including firearms, Fencing, Combat Bare Fists, crowd/hostage, creature, wizard, and zombie content.
- No first-person, motion-matching database, network authority/rollback, target-character, or non-Unity engine acceptance.

## Runtime sets and authored motion

These are collection-owned cross-pack candidates; constituent timing/directional sets remain in their linked reports.

| Set/profile | Role or coordinate | Exact members | Variant/type | Timing or motion | Runtime contract |
|---|---|---|---|---|---|
| Armed-state handoff | ordered | `basic:Humanoid@IdleUnarmed.FBX`; `sword:Humanoid@DrawWeapons1S&S.fbx`; `sword:Humanoid@IdleCombatS&S.fbx`; `sword:Humanoid@PutBackWeapons1S&S.fbx` | set_type=transition-chain | N/A | transition=at-end; state=unarmed-to-armed |
| Walk + sword attack mask candidate | layered | `basic:Humanoid@WalkForwardUnarmed2.fbx`; `sword:Humanoid@SwordAttack1S&S.fbx` | set_type=mask-composition | N/A | mask=upper-body-candidate; movement=basic-base |
| Run + heavy block mask candidate | layered | `basic:Humanoid@RunForward2Unarmed.fbx`; `sword:Humanoid@BlockHeavy1S&S.fbx` | set_type=mask-composition | N/A | mask=upper-body-candidate; movement=basic-base |
| Basic-to-Campfire posture | ordered | `basic:Humanoid@IdleUnarmed.FBX`; `campfire:Humanoid@StandToKneelCampfire.fbx`; `campfire:Humanoid@IdleKneelCampfire.fbx` | set_type=transition-chain | N/A | transition=reviewed-crossfade; state=campfire |
| Basic-to-Climbing entry | ordered | `basic:Humanoid@JumpToApexUnarmed.FBX`; `climbing:Humanoid@FallingToEnterWall.fbx`; `climbing:Humanoid@IdleWallClimb.fbx` | set_type=transition-chain | N/A | transition=contact-window; state=climbing |
| Basic-to-Injured state | ordered | `basic:Humanoid@WalkForwardUnarmed2.fbx`; `injured:Humanoid@WalkInjuredA.fbx`; `injured:Humanoid@IdleInjuredA.fbx` | set_type=transition-chain | N/A | transition=reviewed-crossfade; state=injured |
| Walk + injured torso mask candidate | layered | `basic:Humanoid@WalkForwardUnarmed2.fbx`; `injured:Humanoid@IdleInjuredA.fbx` | set_type=mask-composition | N/A | mask=upper-body-candidate; movement=basic-base |
| Sword-to-Injured state | ordered | `sword:Humanoid@IdleCombatS&S.fbx`; `injured:Humanoid@IdleInjuredA.fbx` | set_type=transition-chain | N/A | transition=weapon-policy; state=injured |
| Basic-to-1-Handed armed state | ordered | `basic:Humanoid@IdleUnarmed.FBX`; `one:Humanoid@DrawWeapon1hMelee.fbx`; `one:Humanoid@IdleCombat1hMelee.fbx`; `one:Humanoid@PutBackWeapon1hMelee.fbx` | set_type=transition-chain | N/A | transition=at-end; state=unarmed-to-one-handed |
| Basic-to-2-Handed armed state | ordered | `basic:Humanoid@IdleUnarmed.FBX`; `two:Humanoid@Draw2HandMelee.fbx`; `two:Humanoid@IdleCombatA2HandMelee.fbx`; `two:Humanoid@PutBack2HandMelee.fbx` | set_type=transition-chain | N/A | transition=at-end; state=unarmed-to-two-handed |
| Basic-to-Dual-Swords armed state | ordered | `basic:Humanoid@IdleUnarmed.FBX`; `dual:Humanoid@DrawDualSwords.fbx`; `dual:Humanoid@IdleCombatDualSwords.fbx`; `dual:Humanoid@PutBackDualSwords.fbx` | set_type=transition-chain | N/A | transition=at-end; state=unarmed-to-dual-swords |
| Walk + 1-Handed attack mask candidate | layered | `basic:Humanoid@WalkForwardUnarmed2.fbx`; `one:Humanoid@AttackA1hMelee.fbx` | set_type=mask-composition | N/A | mask=upper-body-candidate; movement=basic-base |
| Walk + 2-Handed attack mask candidate | layered | `basic:Humanoid@WalkForwardUnarmed2.fbx`; `two:Humanoid@AttackA2HandMelee.fbx` | set_type=mask-composition | N/A | mask=upper-body-candidate; movement=basic-base |
| Walk + Dual-Swords attack mask candidate | layered | `basic:Humanoid@WalkForwardUnarmed2.fbx`; `dual:Humanoid@Attack1DualSwords.fbx` | set_type=mask-composition | N/A | mask=upper-body-candidate; movement=basic-base |

## Integration recipe

1. **Members/topology:** `topology=eight-full-body-modes`; preserve each constituent's sets, use Basic as the default movement hub, and quarantine all six documented outliers.
2. **Timing/synchronization:** `sync=per-mode-contracts`; review and import the current IP gait candidates where applicable, apply runtime offsets for residual phase, preserve raw root-motion members, and crossfade only at reviewed equipment, posture, injury, or environment boundaries.
3. **State ownership:** `owner=active-gameplay-mode`; one mode owns root/pelvis/movement at a time, with exclusive code or animation displacement per clip.
4. **Composition constraints:** `composition=full-body-default`; promote masks clip-by-clip only after pelvis, support, weapon/prop contact, and target-character review.
5. **Acceptance gate:** `gate=eight-mode-target-character-matrix`; test constituent graphs, every cross-pack set above, props, contacts, root authority, retargeting, cancellation, compression, and builds.

## Technical issue register

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| UC-001 | major | Skeleton-compatible mode changes can still snap in pose, style, foot plant, or prop state; [identity guidance](../game-ready-clips.md#files-disagree-about-skeleton-or-clip-identity). | engine-config | Crossfade only at reviewed full-body boundaries and author missing handoffs. | Cross-pack set diagnostics can help; style invention cannot. | Retained mixer checks execute; visuals open. |
| UC-002 | major | Upper-body masks may remove injured posture or break pelvis balance, weapon contact, and support; [blend guidance](../game-ready-clips.md#feet-skate-when-clips-blend). | engine-config | Default to full-body and promote masks member by member. | Mask/contact diagnostics are future candidates; artistic intent remains required. | Seven historical/current mask graphs execute; no visual gate. |
| UC-003 | major | Raw loop and gait-phase findings span all eight packs, so combining packs can compound skating or wrap pulses; [loop guidance](../game-ready-clips.md#the-loop-pops). | engine-config | For the three new melee packs, visually review/import the 72 current IP gait candidates and apply runtime offsets for residual phase; retain raw RM. Apply each older constituent's documented policy. | [#426](https://github.com/mmannerm/animsmith/issues/426) is delivered and the current declared transform now emits these IP candidates; [#411](https://github.com/mmannerm/animsmith/issues/411) covers remaining report evidence. | 72/72 transforms and inspect/measure/fix verification succeed; phase spreads improve materially, but Unity/visual acceptance is open. |
| UC-004 | major | Climbing lacks vertical/yaw displacement and environment-contact proof, risking stalled, doubled, or geometry-misaligned traversal. Guidance: not applicable. | animsmith-future-candidate | Keep root authority exclusive and validate against game geometry. | [#408](https://github.com/mmannerm/animsmith/issues/408) tracks displacement evidence; contacts still need game intent. | Headless samples/mixer pass only. |
| UC-005 | blocker | Sword Crouch FR RM, Climbing `FallingUnarmed`, and four 1-/2-Handed block files are rig/import outliers, leaving those specific runtime paths unusable; [identity guidance](../game-ready-clips.md#files-disagree-about-skeleton-or-clip-identity). | artist-author | Quarantine all six and use documented alternatives. | Detection can improve; missing, malformed, or Generic-authored content cannot be safely synthesized. | AnimSmith and Unity evidence agree; four new Generic files fail expected Humanoid assertions. |
| UC-006 | moderate | Fifteen current constituents remain unevaluated, so collection-wide compatibility, coverage, and value are unknown. Guidance: not applicable. | unknown | Keep this a partial rollup and add category waves with selective cross-pack tests. | Versioned rollup/report automation is tracked by [#427](https://github.com/mmannerm/animsmith/issues/427). | Explicit scope boundary. |
| UC-007 | moderate | AnimSmith's default humanoid role profile does not resolve the capitalized 2-Handed bone names, hiding gait-role measurements unless configured; [rig guidance](../game-ready-clips.md#files-disagree-about-skeleton-or-clip-identity). | animsmith-future-candidate | Use the retained explicit `[rig.roles]` mapping for the 118 accepted 58-bone files. | Case-tolerant aliases are plausible only with deterministic ambiguity checks and fail-closed proof; tracked by [#437](https://github.com/mmannerm/animsmith/issues/437). | Explicit mapping restores measurements without changing source assets. |

## Engine status

| Runtime | Evidence level | Technical result | Remaining gate |
|---|---|---|---|
| Unity 6000.5.8f1 | Eight-pack co-import + headless Playables/masks/props | **Conditional pass:** the melee wave passes 33/33 required checks and four expected Generic outliers fail separately; the earlier contextual wave retains 22/22 required passes. Shared assets do not conflict. | Visual controllers, root motion, contacts, target retarget, compression, player build. |
| Unreal Engine | Documentation only | **Not evaluated.** Root Motion, Blend Spaces, Sync Groups, montages, and layered blends can express the design. | Import eight packs, retarget, graphs, contacts, build. |
| Godot | Documentation only | **Not evaluated.** AnimationTree supports blend spaces, filters, one-shots, sync, and root-motion extraction. | Conversion/import, retarget, graphs, contacts, export. |
| Bevy | Documentation only | **Not evaluated.** AnimationGraph masks exist; FBX conversion and retargeting remain project work. | glTF conversion, target mapping, graphs, root motion, performance. |

## Fit and limitations

Best fit: a third-person action RPG, survival game, or prototype with explicit unarmed, weapon-specific armed, injured, camp, and traversal states and capacity for controller/contact authoring.

Poor fit without further packs/work: firearm-heavy, first-person, creature/zombie, crowd, motion-matching, fighting-game contact, or network-root-motion systems. Fencing-specific reach and stance behavior also remains unevaluated. A seamless universal locomotion/mask graph is not proven.

The eight packs are technically compatible enough to justify a shared project and staged visual evaluation. The next collection work should remain category-based; compare a category to these eight only where a real gameplay handoff, mask, prop, or replacement decision exists.

## Evidence status

The partial rollup manifest covers 582 logical motions, 895 individual files, and 90 runtime-set records: 76 constituent-owned plus 14 cross-pack. New melee baselines/contracts use AnimSmith 0.3.0 at `b7c215ba259b87b4b4e46567452a037a34be7308`; their gait refresh uses the 0.3.1-bound revision `674396f0f53b10c4344e7315a5756fe5ef71b469`. Contextual packs use 0.3.0 at `aabac28edf2719db236068339f1208bbf156d0bb`; Basic and Sword retain their separately versioned evidence. The [readiness ladder](../game-ready-clips.md#the-readiness-ladder) and [appendix](protofactor-ultimate-animation-collection-evidence.md) retain exact scope, revisions, and reproduction evidence.

## Sources

- Constituent reports: [Basic Locomotion](protofactor-basic-locomotion.md), [Sword & Shield](protofactor-sword-and-shield.md), [Campfire](protofactor-campfire.md), [Climbing](protofactor-climbing.md), [Injured](protofactor-injured.md), [1-Handed Melee](protofactor-one-handed-melee.md), [2-Handed Melee](protofactor-two-handed-melee.md), and [Dual Swords](protofactor-dual-swords.md).
- Protofactor, [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/) and [End User License Agreement](https://protofactor.biz/end-user-license-agreement/).
- Unity, [Avatar Mask](https://docs.unity3d.com/6000.0/Documentation/Manual/class-AvatarMask.html) and [Root Motion](https://docs.unity3d.com/6000.5/Documentation/Manual/RootMotion.html); Epic Games, [Layered Animations](https://dev.epicgames.com/documentation/unreal-engine/using-layered-animations-in-unreal-engine?lang=en-US); Godot, [AnimationTree](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html); Bevy, [Animation Masks](https://bevy.org/examples/animation/animation-masks/).
