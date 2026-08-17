# Animation pack evaluation: Protofactor Ultimate Animation Collection (partial: Basic Locomotion + Sword & Shield)

> Technical verdict: **Usable with conditions**
>
> Evaluation completeness: **partial** — only Basic Locomotion and Sword & Shield are evaluated; Unity co-install and headless composition passed, but cross-pack visual acceptance and the other collection packs are absent.
>
> Confidence: **medium**
>
> Evaluation date: **2026-08-17**
>
> Report format: **1**
>
> Detailed evidence: [partial collection evidence appendix](protofactor-ultimate-animation-collection-evidence.md)

## Technical decision

Adopt these two constituents as **two full-body controller modes**: [Basic Locomotion](protofactor-basic-locomotion.md) for unarmed exploration and [Sword & Shield](protofactor-sword-and-shield.md) for armed combat. Switch at the draw/put-away transition rather than expecting one universal locomotion graph.

This pairing is technically strong: the standard files share the same 56-bone skeleton signature, all 25 overlapping package paths are byte-identical, and Unity 6000.5.8f1 co-imported both packages without a collision. Basic contributes turns, pivots, jump/fall/landing, limited obstacles, cover, and grenade actions; Sword contributes combat locomotion, melee/defense, reactions, death/recovery, equipment transitions, and props.

Two cross-pack full-body blends and three upper-body-mask graphs evaluated without exceptions. This proves graph execution, not style, foot planting, pelvis continuity, grip, weapon contact, or deformation. Use full-body combat by default. Treat masking as a candidate for stationary/light upper-body actions only; kicks and displacement-bearing attacks remain full-body.

Both packs still need loop and gait-phase curation. Quarantine the malformed Sword Crouch FR RM file. This verdict applies only to the two evaluated constituents, not the full collection or its value; the appendix names all 21 excluded local archive labels.

## Capability coverage

### Complete core

- Broad grounded unarmed and armed locomotion with IP/RM variants, idles, turns, pivots, and state handoff clips.
- Sword/shield attacks, combos, defense, reactions, deaths/recovery, equipment, and props.
- One shared Unity Humanoid actor/asset path with exact byte and standard-skeleton compatibility evidence.

### Partial supporting gameplay

- Basic jumps, cover, grenade, and 1 m obstacles without a complete armed counterpart or authored cross-pack transitions.
- Upper-body composition with headless Unity evidence but no visual/contact acceptance.
- Root-motion operation without per-action yaw policy, networking, or interruption proof.

### Absent

- Firearms, first-person arms, broad traversal/climbing, paired-character interactions, additive aim offsets, motion-matching annotations, and evaluation of the 21 excluded packs.

## Runtime sets and authored motion

These are collection-owned cross-pack sets; constituent locomotion sets remain in their linked reports.

| Set/profile | Role or coordinate | Exact members | Variant/type | Timing or motion | Runtime contract |
|---|---|---|---|---|---|
| Armed-state handoff | ordered | `protofactor-basic-locomotion:Humanoid@IdleUnarmed.FBX`; `protofactor-sword-and-shield:Humanoid@DrawWeapons1S&S.fbx`; `protofactor-sword-and-shield:Humanoid@IdleCombatS&S.fbx`; `protofactor-sword-and-shield:Humanoid@PutBackWeapons1S&S.fbx` | set_type=transition-chain | N/A | transition=at-end; state=unarmed-to-armed |
| Walk + sword attack mask candidate | layered | `protofactor-basic-locomotion:Humanoid@WalkForwardUnarmed2.fbx`; `protofactor-sword-and-shield:Humanoid@SwordAttack1S&S.fbx` | set_type=mask-composition | N/A | mask=upper-body-candidate; movement=basic-base; state=armed-overlay |
| Run + heavy block mask candidate | layered | `protofactor-basic-locomotion:Humanoid@RunForward2Unarmed.fbx`; `protofactor-sword-and-shield:Humanoid@BlockHeavy1S&S.fbx` | set_type=mask-composition | N/A | mask=upper-body-candidate; movement=basic-base; state=armed-overlay |

## Integration recipe

1. **Members/topology:** `topology=two-full-body-state-machines`; use Basic IP/RM graphs for unarmed exploration and Sword IP/RM graphs for armed combat; preserve constituent coordinates and quarantine Sword Crouch FR RM.
2. **Timing/synchronization:** `sync=per-constituent-runtime-offsets`; curate loops and phase offsets inside each pack, then crossfade only at reviewed equipment/idle boundaries.
3. **State ownership:** `owner=active-full-body-state`; the active controller mode owns movement, with controller ownership for IP and animation ownership for individually validated RM clips.
4. **Composition constraints:** `composition=full-body-combat-default`; attach the Sword props to their tested hands; mask only reviewed upper-body actions, never kicks or displacement-bearing attacks.
5. **Acceptance gate:** `gate=combined-target-character-visual-review`; test equip transitions, both locomotion graphs, cross-pack blends, masks, contacts, prop grips, retarget deformation, root extraction, and build behavior.

## Technical issue register

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| UC-001 | major | A full-body armed/unarmed [skeleton-compatible handoff](../game-ready-clips.md#files-disagree-about-skeleton-or-clip-identity) can still snap in pose or style. | engine-config | Crossfade at reviewed draw/put-away boundaries on the target character. | Cross-pack declared-set diagnostics can help; style invention cannot. | Exact skeleton/assets and headless mixing pass; visual result open. |
| UC-002 | major | [Upper-body masks](../game-ready-clips.md#feet-skate-when-clips-blend) may break pelvis balance, weapon contact, or lower-body support. | engine-config | Default to full-body; promote masks clip by clip after contact/IK review. | Mask contracts/diagnostics are feasible; contact repair needs authored intent. | Three Unity mask graphs execute; no visual gate. |
| UC-003 | major | Both packs have raw [gait-phase and loop-seam](../game-ready-clips.md#the-loop-pops) findings, so switching packs does not remove skating/wrap risk. | engine-config | Curate loop intent and runtime offsets independently per graph. | Safe basis support is tracked by [#426](https://github.com/mmannerm/animsmith/issues/426). | Exhaustive constituent contracts; no full visual rings. |
| UC-004 | moderate | Basic traversal/jump/turn clips lack authored armed equivalents, causing [state-identity or pose discontinuity](../game-ready-clips.md#files-disagree-about-skeleton-or-clip-identity). | artist-author | Restrict armed movement, temporarily use reviewed Basic clips, or author combat variants. | Tools cannot invent weapon-aware body mechanics. | Content comparison; cross-state visuals open. |
| UC-005 | blocker | Sword Crouch FR RM has a [malformed hierarchy](../game-ready-clips.md#files-disagree-about-skeleton-or-clip-identity), leaving the combined armed RM ring incomplete. | artist-author | Quarantine it; use IP/controller motion or another direction. | Safe synthesis is unavailable without author evidence. | AnimSmith two-node file; Unity no clip. |
| UC-006 | moderate | Only two of the advertised collection's many constituents are evaluated, so collection-wide [identity and compatibility](../game-ready-clips.md#files-disagree-about-skeleton-or-clip-identity) are unknown. | unknown | Treat this as a partial rollup and add packs incrementally. | Versioned rollup manifests can automate reconciliation. | Scope boundary, not a defect in either evaluated pack. |

## Engine status

| Runtime | Evidence level | Technical result | Remaining gate |
|---|---|---|---|
| Unity 6000.5.8f1 | Combined co-import + headless Playables/masks | **Conditional pass:** 309 clips total, 308 humanMotion; shared files do not conflict; two cross-pack blends and three masks execute. | Visual controller, root motion, contacts, target retarget, compression, player build. |
| Unreal Engine | Documentation only | **Not evaluated.** Root Motion, Blend Spaces, Sync Groups, and layered blends can express the design. | Import both packs, retarget, graphs, contacts, build. |
| Godot | Documentation only | **Not evaluated.** AnimationTree supports blend spaces, filters, one-shots, sync, and root-motion extraction. | Conversion/import, retarget, graphs, contacts, export. |
| Bevy | Documentation only | **Not evaluated.** AnimationGraph masks exist; conversion and retargeting remain material gaps. | glTF conversion, target mapping, graphs, root motion, performance. |

## Fit and limitations

Best fit: a third-person action RPG or melee prototype with explicit unarmed/armed states and a compatible humanoid character.

Poor fit without more content/work: first-person, traversal-heavy, firearm, fighting-game contact, motion-matching, or network-root-motion systems. A visually seamless universal locomotion/mask graph is not proven.

Cross-pack evidence is stronger than role-name similarity: assets/signatures match and Unity composition executes. It remains below acceptance-ready until a target-character visual pass.

## Evidence status

The partial v1 rollup manifest covers 194 logical motions and 309 individual files from exactly two constituents. Basic was evaluated with AnimSmith 0.3.0 at `3857fe130c227918e09473b2e1e307f61867439e`; Sword and the rollup use 0.3.0 at `c11f135ece5e980e6c98861a52a715a28a424ff9`. The [readiness ladder](../game-ready-clips.md#the-readiness-ladder) and [appendix](protofactor-ultimate-animation-collection-evidence.md) retain boundaries and reproduction evidence.

## Sources

- [Basic Locomotion report](protofactor-basic-locomotion.md) and [evidence](protofactor-basic-locomotion-evidence.md).
- [Sword & Shield report](protofactor-sword-and-shield.md) and [evidence](protofactor-sword-and-shield-evidence.md).
- Protofactor, [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/) and [End User License Agreement](https://protofactor.biz/end-user-license-agreement/).
- Unity, [Avatar Mask](https://docs.unity3d.com/es/current/Manual/class-AvatarMask.html); Epic Games, [Layered Animations](https://dev.epicgames.com/documentation/unreal-engine/using-layered-animations-in-unreal-engine?lang=en-US); Godot, [AnimationTree](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html); Bevy, [Animation Masks](https://bevy.org/examples/animation/animation-masks/).
