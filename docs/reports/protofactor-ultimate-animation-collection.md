# Animation pack evaluation: Protofactor Ultimate Animation Collection (partial: five packs)

> Technical verdict: **Usable with conditions**
>
> Evaluation completeness: **partial** — Basic Locomotion, Sword & Shield, Campfire, Climbing, and Injured are evaluated; five-pack Unity co-install/composition passes, while visual acceptance and 18 currently listed constituents are absent.
>
> Confidence: **medium**
>
> Evaluation date: **2026-08-17**
>
> Report format: **1**
>
> Detailed evidence: [partial collection evidence appendix](protofactor-ultimate-animation-collection-evidence.md)

## Technical decision

Adopt the five evaluated constituents as explicit full-body gameplay modes around [Basic Locomotion](protofactor-basic-locomotion.md): [Sword & Shield](protofactor-sword-and-shield.md) for armed combat, [Injured](protofactor-injured.md) for seven injury styles, [Campfire](protofactor-campfire.md) for rest/interactions, and [Climbing](protofactor-climbing.md) for environment traversal. Use authored or reviewed state handoffs; do not merge same-role clips into one universal graph merely because their rigs match.

Technical co-existence is strong. The rollup contains 322 logical motions and 479 individual files. Each of the ten pack pairs has exactly 25 overlapping package paths, all byte-identical, with no conflicts. Standard motions share the same 56-bone structure, and a single Unity 6000.5.8f1 project passes all 22 required sampling, mixing, masking, and prop checks.

This does not make the collection acceptance-ready. Constituent seam/phase issues remain, Climbing needs environment and vertical-root validation, Campfire needs prop/contact authoring, and two files are quarantined or excluded. The 18 unevaluated constituents prevent any collection-wide quality or value verdict.

## Capability coverage

### Complete core

- Broad unarmed locomotion, armed sword/shield combat, injured locomotion/postures, camp states/interactions, and wall/ladder/obstacle traversal are present.
- Five packs share a Unity Humanoid actor family and co-install without conflicting shared assets in the evaluated delivery.

### Partial supporting gameplay

- Full-body transitions between modes and selected upper-body masks execute headlessly, but none is visually accepted.
- Props exist for Sword and part of Campfire; contacts, events, IK, displacement authority, and cancellation remain game-owned.

### Absent

- Eighteen currently listed constituents remain unevaluated, including firearms, other melee/fantasy, crowd/hostage, creature, wizard, and zombie content.
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

## Integration recipe

1. **Members/topology:** `topology=five-full-body-modes`; preserve each constituent's sets, use Basic as the default movement hub, and quarantine both documented outliers.
2. **Timing/synchronization:** `sync=per-mode-contracts`; curate loops/phases inside each pack, then crossfade only at reviewed equipment, posture, injury, or environment boundaries.
3. **State ownership:** `owner=active-gameplay-mode`; one mode owns root/pelvis/movement at a time, with exclusive code or animation displacement per clip.
4. **Composition constraints:** `composition=full-body-default`; promote masks clip-by-clip only after pelvis, support, weapon/prop contact, and target-character review.
5. **Acceptance gate:** `gate=five-mode-target-character-matrix`; test constituent graphs, every cross-pack set above, props, contacts, root authority, retargeting, cancellation, compression, and builds.

## Technical issue register

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| UC-001 | major | Skeleton-compatible mode changes can still snap in pose, style, foot plant, or prop state; [identity guidance](../game-ready-clips.md#files-disagree-about-skeleton-or-clip-identity). | engine-config | Crossfade only at reviewed full-body boundaries and author missing handoffs. | Cross-pack set diagnostics can help; style invention cannot. | Five mixers execute; visuals open. |
| UC-002 | major | Upper-body masks may remove injured posture or break pelvis balance, weapon contact, and support; [blend guidance](../game-ready-clips.md#feet-skate-when-clips-blend). | engine-config | Default to full-body and promote masks member by member. | Mask/contact diagnostics are future candidates; artistic intent remains required. | Four historical/current mask graphs execute; no visual gate. |
| UC-003 | major | Raw loop and gait-phase findings span all five packs, so combining packs can compound skating or wrap pulses; [loop guidance](../game-ready-clips.md#the-loop-pops). | engine-config | Apply constituent loop policies and per-set thresholds/sync markers. | Current checks help; [#426](https://github.com/mmannerm/animsmith/issues/426) and [#411](https://github.com/mmannerm/animsmith/issues/411) cover remaining tooling gaps. | Exhaustive constituent contracts; visual rings open. |
| UC-004 | major | Climbing lacks vertical/yaw displacement and environment-contact proof, risking stalled, doubled, or geometry-misaligned traversal. Guidance: not applicable. | animsmith-future-candidate | Keep root authority exclusive and validate against game geometry. | [#408](https://github.com/mmannerm/animsmith/issues/408) tracks displacement evidence; contacts still need game intent. | Headless samples/mixer pass only. |
| UC-005 | blocker | Sword Crouch FR RM and Climbing `FallingUnarmed` are rig/import outliers, leaving those specific runtime paths unusable; [identity guidance](../game-ready-clips.md#files-disagree-about-skeleton-or-clip-identity). | artist-author | Quarantine both and use documented alternatives. | Detection can improve; missing/malformed authored content cannot be safely synthesized. | AnimSmith and Unity evidence agree. |
| UC-006 | moderate | Eighteen current constituents remain unevaluated, so collection-wide compatibility, coverage, and value are unknown. Guidance: not applicable. | unknown | Keep this a partial rollup and add category waves with selective cross-pack tests. | Versioned rollup/report automation is tracked by [#427](https://github.com/mmannerm/animsmith/issues/427). | Explicit scope boundary. |

## Engine status

| Runtime | Evidence level | Technical result | Remaining gate |
|---|---|---|---|
| Unity 6000.5.8f1 | Five-pack co-import + headless Playables/masks/props | **Conditional pass:** all 22 required checks pass; the expected Climbing outlier fails separately; shared assets do not conflict. | Visual controllers, root motion, contacts, target retarget, compression, player build. |
| Unreal Engine | Documentation only | **Not evaluated.** Root Motion, Blend Spaces, Sync Groups, montages, and layered blends can express the design. | Import five packs, retarget, graphs, contacts, build. |
| Godot | Documentation only | **Not evaluated.** AnimationTree supports blend spaces, filters, one-shots, sync, and root-motion extraction. | Conversion/import, retarget, graphs, contacts, export. |
| Bevy | Documentation only | **Not evaluated.** AnimationGraph masks exist; FBX conversion and retargeting remain project work. | glTF conversion, target mapping, graphs, root motion, performance. |

## Fit and limitations

Best fit: a third-person action RPG, survival game, or prototype with explicit unarmed, armed, injured, camp, and traversal states and capacity for controller/contact authoring.

Poor fit without further packs/work: firearm-heavy, first-person, creature/zombie, crowd, motion-matching, fighting-game contact, or network-root-motion systems. A seamless universal locomotion/mask graph is not proven.

The five packs are technically compatible enough to justify a shared project and staged visual evaluation. The next collection work should remain category-based; compare a category to these five only where a real gameplay handoff, mask, prop, or replacement decision exists.

## Evidence status

The partial rollup manifest covers 322 logical motions, 479 individual files, and 64 runtime sets from five constituents. Contextual packs and the rollup use AnimSmith 0.3.0 at `aabac28edf2719db236068339f1208bbf156d0bb`; Basic and Sword retain their separately versioned evidence. The [readiness ladder](../game-ready-clips.md#the-readiness-ladder) and [appendix](protofactor-ultimate-animation-collection-evidence.md) retain exact scope, revisions, and reproduction evidence.

## Sources

- Constituent reports: [Basic Locomotion](protofactor-basic-locomotion.md), [Sword & Shield](protofactor-sword-and-shield.md), [Campfire](protofactor-campfire.md), [Climbing](protofactor-climbing.md), and [Injured](protofactor-injured.md).
- Protofactor, [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/) and [End User License Agreement](https://protofactor.biz/end-user-license-agreement/).
- Unity, [Avatar Mask](https://docs.unity3d.com/6000.0/Documentation/Manual/class-AvatarMask.html) and [Root Motion](https://docs.unity3d.com/6000.5/Documentation/Manual/RootMotion.html); Epic Games, [Layered Animations](https://dev.epicgames.com/documentation/unreal-engine/using-layered-animations-in-unreal-engine?lang=en-US); Godot, [AnimationTree](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html); Bevy, [Animation Masks](https://bevy.org/examples/animation/animation-masks/).
