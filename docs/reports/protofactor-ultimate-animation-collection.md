# Animation pack evaluation: Protofactor Ultimate Animation Collection (partial: eight packs)

> Technical verdict: **Usable with conditions**
>
> Evaluation completeness: **partial** — Basic Locomotion, Sword & Shield, Campfire, Climbing, Injured, 1-Handed Melee, 2-Handed Melee, and Dual Swords are evaluated; eight-pack Unity co-install evidence passes, while visual acceptance and 15 currently listed constituents are absent.
>
> Confidence: **medium**
>
> Evaluation date: **2026-08-21**
>
> Report format: **1**
>
> Detailed evidence: [partial collection evidence appendix](protofactor-ultimate-animation-collection-evidence.md)

## Technical decision

Adopt the eight evaluated constituents as explicit full-body gameplay modes around [Basic Locomotion](protofactor-basic-locomotion.md): [Sword & Shield](protofactor-sword-and-shield.md), [1-Handed Melee](protofactor-one-handed-melee.md), [2-Handed Melee](protofactor-two-handed-melee.md), and [Dual Swords](protofactor-dual-swords.md) for distinct armed stances; [Injured](protofactor-injured.md) for seven injury styles; [Campfire](protofactor-campfire.md) for rest/interactions; and [Climbing](protofactor-climbing.md) for environment traversal. Use authored or reviewed state handoffs; do not merge same-role clips into one universal graph merely because their rigs retarget in one engine.

Technical co-existence is strong. The rollup contains 582 logical motions and 895 individual files. Each of the 28 pack pairs has exactly 25 overlapping package paths, all byte-identical, with no conflicts. Most standard motions share the same 56-bone structure; the dominant 2-Handed family adds left/right forearm twists and uses capitalized bone identifiers. One Unity 6000.5.8f1 project imports all eight packs. The latest melee probe passes 33/33 required sampling, mixing, masking, and prop checks, while four expected Generic-rig failures are kept separate; the earlier contextual probe retains its 22/22 result.

AnimSmith 0.4.0 extends the gait fix to the whole collection. Every clip in all eight packs resolves a vertical `positive_y` root heading, the exact condition that made 0.3.0 refuse anchoring, so all 134 selected in-place ring members across the six gait-bearing packs now anchor and none refuse. Basic Locomotion, Sword & Shield, and Injured move from 62 combined refusals to 24, 24, and 14 successes, and the three melee packs reproduce their earlier pre-release spreads to seven decimal places. Every ring's circular phase spread falls below 0.14, with per-pack figures in the appendix. Campfire and Climbing have no in-place cyclic ring, which 0.4.0 records as not applicable rather than as a failure. Root-motion members were not transformed. All 159 generated candidates remain external and unpromoted: no engine import or visual acceptance ran.

This does not make the collection acceptance-ready. Residual seam/phase issues remain, weapon contacts and action timing need gameplay validation, Climbing has measured vertical and yaw root facts but still needs environment and contact validation, Campfire needs prop/contact authoring, and six files are quarantined or excluded. The 15 unevaluated constituents prevent any collection-wide quality or value verdict.

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
| UC-003 | major | Raw loop and gait-phase findings span all eight packs, so combining packs can compound skating or wrap pulses; [loop guidance](../game-ready-clips.md#the-loop-pops). | engine-config | Visually review and import the 134 IP gait candidates across the six gait-bearing packs, apply runtime offsets for residual phase, and retain raw RM. | Delivered [#426](https://github.com/mmannerm/animsmith/issues/426) extends anchoring to vertical-forward-axis rigs; [#411](https://github.com/mmannerm/animsmith/issues/411) still covers set speed/stride evidence. | 134/134 transforms succeed and every candidate re-reads cleanly; spreads improve materially, but Unity and visual acceptance stay open. |
| UC-004 | major | Climbing has measured vertical and yaw displacement but no environment-contact proof, so traversal can still stall, double, or misalign against geometry. Guidance: not applicable. | engine-config | Keep root authority exclusive and validate every traversal family against real game geometry. | Delivered [#408](https://github.com/mmannerm/animsmith/issues/408) supplies per-clip root displacement and accumulated yaw; contact and alignment intent stay outside the tool. | Ladder +/-1.500 m, obstacle up to +2.000 m, wall -1.950..+2.000 m, and a -180 deg turn on `EnterWallTop_RM` are measured; headless samples/mixer pass only and no contact gate ran. |
| UC-005 | blocker | Sword Crouch FR RM, Climbing `FallingUnarmed`, and four 1-/2-Handed block files are rig/import outliers, leaving those specific runtime paths unusable; [identity guidance](../game-ready-clips.md#files-disagree-about-skeleton-or-clip-identity). | artist-author | Quarantine all six and use documented alternatives. | Detection can improve; missing, malformed, or Generic-authored content cannot be safely synthesized. | AnimSmith and Unity evidence agree; four new Generic files fail expected Humanoid assertions. |
| UC-006 | moderate | Fifteen current constituents remain unevaluated, so collection-wide compatibility, coverage, and value are unknown. Guidance: not applicable. | unknown | Keep this a partial rollup and add category waves with selective cross-pack tests. | Versioned rollup/report automation is tracked by [#427](https://github.com/mmannerm/animsmith/issues/427). | Explicit scope boundary. |
| UC-007 | moderate | AnimSmith's default humanoid role profile does not resolve the capitalized 2-Handed bone names, hiding gait-role measurements unless configured; [rig guidance](../game-ready-clips.md#files-disagree-about-skeleton-or-clip-identity). | animsmith-future-candidate | Use the retained explicit `[rig.roles]` mapping for the 118 accepted 58-bone files. | Case-tolerant aliases are plausible only with deterministic ambiguity checks and fail-closed proof; tracked by [#437](https://github.com/mmannerm/animsmith/issues/437). | Under 0.4.0 the default profile measures root trajectory on 4/122 clips against 122/122 with the explicit map, while lint notes stay identical at 17,016 - configuration, not an asset repair. |

## Engine status

| Runtime | Evidence level | Technical result | Remaining gate |
|---|---|---|---|
| Unity 6000.5.8f1 | Eight-pack co-import + headless Playables/masks/props | **Conditional pass:** the melee wave passes 33/33 required checks and four expected Generic outliers fail separately; the earlier contextual wave retains 22/22 required passes. Shared assets do not conflict. | Visual controllers, root motion, contacts, target retarget, compression, player build. |
| Unreal Engine | Exact profile validation, documentation otherwise | **Not evaluated for import.** The exact `unreal` revision 1 / Unreal 5.8 profile validates the source and returns the typed refusal `profile_settings_unmodeled` (exit 1) because its setting vocabulary is unmodelled. Root Motion, Blend Spaces, Sync Groups, montages, and layered blends can express the design. | Import eight packs, retarget, graphs, contacts, build. |
| Godot | Exact profile validation, documentation otherwise | **Not evaluated for import.** The exact `godot` revision 1 / Godot 4.7 profile validates the source and returns the typed refusal `profile_settings_unmodeled` (exit 1). AnimationTree supports blend spaces, filters, one-shots, sync, and root-motion extraction. | Conversion/import, retarget, graphs, contacts, export. |
| Bevy | Addressability inventory on generated GLB candidates | **Not evaluated for runtime.** The exact `bevy` revision 1 / Bevy 0.19.0 profile inventories each generated candidate and predicts the canonical `Animation0` selector with no findings. That is selector and inventory evidence only - not asset loading, target identity, graph wiring, masking, or playback. | glTF conversion for delivered sources, target mapping, graphs, root motion, performance. |

## Fit and limitations

Best fit: a third-person action RPG, survival game, or prototype with explicit unarmed, weapon-specific armed, injured, camp, and traversal states and capacity for controller/contact authoring.

Poor fit without further packs/work: firearm-heavy, first-person, creature/zombie, crowd, motion-matching, fighting-game contact, or network-root-motion systems. Fencing-specific reach and stance behavior also remains unevaluated. A seamless universal locomotion/mask graph is not proven.

The eight packs are technically compatible enough to justify a shared project and staged visual evaluation. The next collection work should remain category-based; compare a category to these eight only where a real gameplay handoff, mask, prop, or replacement decision exists.

## Evidence status

The partial rollup manifest covers 582 logical motions, 895 individual files, and 90 runtime-set records: 76 constituent-owned plus 14 cross-pack. Every constituent now uses one frozen evaluator, AnimSmith 0.4.0 at `6b37ad636b198ef8ff47fadbf6a3a51eb1a27c8e`, replacing the earlier mix of 0.3.0 revisions and a pre-release gait-only build; all evidence is output v10 with measurements v15. Each source identity re-inventoried byte-identical, so the 28-pair shared-path comparison is retained from its original run, not re-attributed. Mechanical counts reproduce the 0.3.0 baseline exactly, so these changes are evaluator semantics, not asset drift. The [readiness ladder](../game-ready-clips.md#the-readiness-ladder) and [appendix](protofactor-ultimate-animation-collection-evidence.md) retain exact scope, revisions, and reproduction evidence.

## Sources

- Constituent reports: [Basic Locomotion](protofactor-basic-locomotion.md), [Sword & Shield](protofactor-sword-and-shield.md), [Campfire](protofactor-campfire.md), [Climbing](protofactor-climbing.md), [Injured](protofactor-injured.md), [1-Handed Melee](protofactor-one-handed-melee.md), [2-Handed Melee](protofactor-two-handed-melee.md), and [Dual Swords](protofactor-dual-swords.md).
- Protofactor, [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/) and [End User License Agreement](https://protofactor.biz/end-user-license-agreement/).
- Unity, [Avatar Mask](https://docs.unity3d.com/6000.0/Documentation/Manual/class-AvatarMask.html) and [Root Motion](https://docs.unity3d.com/6000.5/Documentation/Manual/RootMotion.html); Epic Games, [Layered Animations](https://dev.epicgames.com/documentation/unreal-engine/using-layered-animations-in-unreal-engine?lang=en-US); Godot, [AnimationTree](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html); Bevy, [Animation Masks](https://bevy.org/examples/animation/animation-masks/).
