# Animation pack evidence appendix: Protofactor Ultimate Animation Collection (partial: five packs)

> Companion report: [partial collection technical evaluation](protofactor-ultimate-animation-collection.md)
>
> Evidence status: **partial** — five constituent manifests, all pairwise shared-file comparisons, and one five-pack Unity probe are retained; 18 listed constituents and visual acceptance are absent.
>
> Evaluation date: **2026-08-17**
>
> Report format: **1**

This collection appendix links rather than replaces the [Basic Locomotion](protofactor-basic-locomotion-evidence.md), [Sword & Shield](protofactor-sword-and-shield-evidence.md), [Campfire](protofactor-campfire-evidence.md), [Climbing](protofactor-climbing-evidence.md), and [Injured](protofactor-injured-evidence.md) evidence. The [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder) remains authoritative.

## Evaluation scope and provenance

| Field | Value |
|---|---|
| Pack/edition | Protofactor Ultimate Animation Collection; partial rollup of five locally held constituents; local collection/constituent revisions unknown |
| Vendor/source | Protofactor [current collection page](https://protofactor.biz/product/ultimate-animation-collection/) and constituent pages |
| Delivered scope | Five RARs to five Unitypackages; 493 FBXs total, including 479 individual motion files; 18 listed constituents not extracted/evaluated in this wave |
| Target use | One third-person project with unarmed, armed, injured, camp, and traversal gameplay modes |
| Target engines | Unity 6000.5.8f1 observed; Unreal Engine 5.7, Godot stable, and Bevy documentation-only |
| Target rigs/packs | Shared Protof-Actor family and all five evaluated constituents; no project character |
| Source manifest | Five separately hashed constituent manifests plus all ten pairwise logical-delivery comparisons |
| Evaluation manifest | `evidence/ultimate-collection-five-pack-partial-evaluation-manifest.json`; SHA-256 `f5121a0b74b75bf48ca774807758883accfaced4e0f15517799def724b9cd1c6`; schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1`; taxonomy/profile-set version 1 |
| Acquisition/license provenance | User states the local archives were downloaded from Protofactor as collection content. Current [vendor EULA](https://protofactor.biz/end-user-license-agreement/) reviewed 2026-08-17; receipts, historical terms, and exact revisions unavailable; no legal opinion. |

Evaluated: **Basic Locomotion, Sword & Shield, Campfire, Climbing, and Injured**. The 18 current/local labels excluded are **1-Handed Melee Weapon, 2-Handed Gun, 2-Handed Melee Weapon, Assault Rifle, Bazooka, Bow & Arrow, Combat Bare Fists, Creature, Crowd, Double Guns, Dual Swords, Fencing, Hostage, Minigun, Push & Pull Cube, Shotgun, Wizard, and Zombie**. Names were inventoried only; contents and compatibility were not evaluated.

### Evidence coverage

| Surface | Offered/delivered | Evaluated | Findings | Not evaluated and why |
|---|---:|---:|---:|---|
| Animation/model files | 493 FBXs | 493 via constituent runs | 479 individual files; known strict-time/seam/rig findings | 18 constituent archives and exhaustive artistic review |
| Canonical inventory | 479 individual files | 322 logical motions, 64 manifest set records | Five namespaced validated manifests | Visual semantic correctness |
| Shared logical paths | 10 pack pairs × 25 overlaps | 250 comparisons | 250 byte-identical; zero conflicts | Unevaluated constituents |
| Engine import/playback | Five reconstructed Unity deliveries | Five in one project | 22/22 required checks pass; one expected outlier fails | Visual controller, compression, player build |
| Cross-pack composition | 4 current mixers; 1 current mask; 2 props | 7 checks | All required checks execute | Style, contacts, IK, offsets, deformation |
| Other engines | Three runtime documentation sets | Documentation only | Capabilities exist | Pack import/retarget/runtime behavior |

### Claim legend

`user-stated`, `vendor-stated`, `observed-file`, `observed-animsmith`, `observed-report`, `observed-engine`, `documentation-stated`, `inferred`, and `not-evaluated` keep acquisition, computation, execution, and inference separate.

## Evaluation manifest and taxonomy

The retained rollup uses schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1` and taxonomy/profile-set version 1.

### Canonical clip-role inventory

| Canonical primary role | Logical motions | Delivered files | Evidence boundary |
|---|---:|---:|---|
| `idle-pose` | 55 | 55 | General, combat, injury, camp, and traversal holds |
| `continuous-locomotion` | 76 | 152 | Basic, Sword, and Injured IP/RM families |
| `locomotion-transition` | 66 | 96 | Turns/pivots, posture, and preparation boundaries |
| `airborne` | 15 | 19 | Basic plus Climbing fall/land families |
| `traversal` | 32 | 62 | Basic obstacle and broad Climbing content |
| `action-interaction` | 55 | 68 | Combat, equipment, camp, cover, and grenade actions |
| `reaction-death` | 20 | 24 | Sword constituent only |
| `emote-cinematic` | 3 | 3 | Sword taunts only |
| `other-unknown` | 0 | 0 | None |
| **Total** | **322** | **479** | Validated five-pack rollup SHA-256 `f5121a0b74b75bf48ca774807758883accfaced4e0f15517799def724b9cd1c6` |

### Runtime-set inventory

| Runtime set | Type | Members/variants | Grouping evidence | Validation status |
|---|---|---|---|---|
| Armed-state handoff | transition-chain | Basic idle plus Sword draw/idle/put-back | Existing two-pack technical evidence; medium confidence | Files/import known; transition visual-open |
| Walk + sword attack mask candidate | mask-composition | Basic walk plus Sword attack | Existing Unity Humanoid mask execution; low confidence | Pelvis/grip/contact visual-open |
| Run + heavy block mask candidate | mask-composition | Basic run plus Sword block | Existing Unity Humanoid mask execution; low confidence | Support/shield visual-open |
| Basic-to-Campfire posture | transition-chain | Basic idle plus Campfire stand-to-kneel/idle | Current Unity mixer and exact standard rig; low confidence | No authored cross-pack entry; visual-open |
| Basic-to-Climbing entry | transition-chain | Basic jump apex plus Climbing fall-to-wall/hold | Current Unity mixer and state semantics; low confidence | Wall/contact/root visual-open |
| Basic-to-Injured state | transition-chain | Basic walk plus Injured A walk/idle | Current Unity mixer and standard rig; low confidence | No authored injury onset; visual-open |
| Walk + injured torso mask candidate | mask-composition | Basic walk plus Injured A idle | Current Unity Humanoid mask execution; low confidence | Injury readability/pelvis visual-open |
| Sword-to-Injured state | transition-chain | Sword combat idle plus Injured A idle | Current Unity mixer and standard rig; low confidence | Weapon policy/transition visual-open |
| Constituent-owned sets | other | 56 manifest records: 10 Basic, 16 Sword, 6 Campfire, 10 Climbing, and 14 Injured | Five constituent manifests | Exact contracts remain in linked reports |

The count above is the versioned manifest-record count. Four Climbing IP/RM families each use separate in-place and root-motion records in the manifest but one paired row in the Climbing appendix, so the 56 constituent records render there as 52 user-facing rows. Adding the eight cross-pack rows produces 64 manifest records and 60 rendered rows.

### Pipeline-stage coverage

| Stage | Coverage state | Evidence / remaining gate |
|---|---|---|
| Acquire | `partially-evaluated` | Five local archives/current pages captured; 18 constituents and historical transaction evidence absent. |
| Preserve raw | `evaluated-clean` | Each evaluated source archive remains immutable and separately hashed outside the repository. |
| Inspect | `evaluated-finding` | All 493 evaluated FBXs inspected through constituent runs. |
| Segment | `partially-evaluated` | Individual files preferred; combined takes not promoted. |
| Root motion | `evaluated-finding` | IP/RM families inventoried; vertical/yaw evidence and outliers remain bounded. |
| Conform | `partially-evaluated` | Standard 56-bone family and shared Unity Avatars co-exist; target/other engines open. |
| Validate | `evaluated-finding` | Constituent contracts and five-pack Unity graph probe complete; visuals open. |
| Optimize | `partially-evaluated` | Bounded constituent trials only; no optimized collection promoted. |
| Export | `partially-evaluated` | Native Unity assets co-imported; other-engine exports not accepted. |
| Gate/report | `partially-evaluated` | Five report pairs and this rollup retained; 18 constituents and visual gates open. |

### Readiness evidence by clip set

| Role or runtime set | File-ready / clip-ready | Set-ready / rig-use | Runtime / acceptance boundary |
|---|---|---|---|
| Basic unarmed mode | 177 individual clips; known declared slicing/loop policy | Broad controller foundation | Unity evidence retained; visual controller open |
| Sword armed mode | 132 files; one RM outlier quarantined | Combat/equipment/props; shared rig | Mix/masks/props execute; contacts open |
| Campfire contextual mode | 25 motions and 2 props readable | Full-body posture chains; missing implied props/events | Mixer/props execute; contacts/loops open |
| Climbing traversal mode | 74 standard clips plus one excluded outlier | IP/RM traversal families; vertical/root/contact gaps | Samples/mixer execute; environment matrix open |
| Injured mode | 70 clips; 14 IP/RM gait pairs | Seven style-specific speed/posture sets | Mix/mask execute; phase/loop/transitions open |
| Cross-pack candidates | Exact members and shared rig evidence | Eight collection-owned sets | Graph execution only; target visual gate required |
| Collection as sold | Five of 23 current constituents classified | Remaining relationships unknown | No collection-wide readiness/value conclusion |

### Validation-profile status

| Validation profile | Selection | Result / next evidence |
|---|---|---|
| Marketplace intake | `selected` — `user-required` | Five-pack intake complete; collection coverage/provenance partial. |
| Blended locomotion | `selected` — `observed-pack-capability` | Constituent sets measured; cross-mode visual gates open. |
| Root-motion controller | `selected` — `observed-pack-capability` | Paired conventions measured; vertical/yaw/authority/network behavior open. |
| State-machine transitions | `selected` — `observed-pack-capability` | Five-mode recipe and cross-pack candidates defined; visuals open. |
| Layered upper body/weapons | `selected` — `user-required` | Masks execute; composition/contact acceptance open. |
| Traversal/environment | `selected` — `observed-pack-capability` | Climbing plus Basic obstacles; environment matrix absent. |
| Contact actions/interactions | `selected` — `observed-pack-capability` | Sword/Campfire/Climbing imply contacts; events/IK/targets open. |
| Retargeted/customizable characters | `selected` — `vendor-intended` | Shared source Avatar works; project character and other engines open. |
| Motion matching/search | `not-selected` | No database contract. |
| Networked movement | `not-selected` | No authority/prediction/rollback contract. |
| Runtime performance | `selected` — `evaluator-selected-generic-scenario` | Constant-track opportunities sampled; runtime equivalence/budget open. |

## Pack inventory and content evidence

The five manifests namespace 479 individual files into 322 logical motions and 64 runtime-set records: 56 constituent-owned plus eight collection-owned. Basic supplies general locomotion/transitions; Sword armed combat/equipment; Campfire rest/interactions; Climbing traversal; Injured seven hurt movement/posture styles.

Every pairwise logical-delivery comparison found exactly 25 overlapping paths, all byte-identical. Shared material/actor assets therefore co-install without same-path byte conflicts in this snapshot. Standard motion files share the current 56-bone signature `2b6fe49d5ae6`; specialist, actor/combined, prop, and malformed/outlier structures remain separately classified.

## Mechanical baseline

| Finding/check | Affected scope | Potential impact | Evidence |
|---|---|---|---|
| Shared paths | 250/250 pairwise overlaps identical | Evaluated packs do not silently overwrite different shared content | Ten SHA-256 comparisons |
| Standard skeleton | Standard motion families in all five packs | Strong co-install/state-switch prerequisite | Current order-independent 56-bone signature |
| Basic raw timing | 12 files | Strict pipelines may reject/clamp without declared slicing | Constituent report |
| Rig/import outliers | Sword Crouch FR RM; Climbing FallingUnarmed | Specific RM/airborne paths unusable | AnimSmith plus Unity evidence |
| Loop/phase policy | All five constituents | Wrap pulses, skating, or wrong semantic repetition | Exhaustive constituent contracts |
| Constant tracks | Broadly present | Optimization opportunity and retarget overhead | Constituent summaries and bounded transforms |

## AnimSmith remediation evidence

| Source issue | Operation/declarations | Result | Independent verification | Remaining caveat |
|---|---|---|---|---|
| Basic negative-time keys | Declared slice to Unity clip ranges | 12/12 candidate outputs remove strict-time errors | Post-transform checks | Visual/engine acceptance still required |
| Gait phase across packs | Gait-anchor trials on Basic, Sword, and Injured IP clips | Safe refusal where root horizontal basis is unmeasurable; no outputs | Command/output-absence records | Runtime offsets or artist export; [#426](https://github.com/mmannerm/animsmith/issues/426) |
| Constant tracks | Representative pruning in all five packs | Smaller GLBs reopen and retain declared findings | Inspect/measure/lint/diff/fix dry-run | Runtime equivalence unproven; no candidate promoted |
| Rig/import outliers | No safe repair | Both files quarantined/excluded | AnimSmith and Unity agree | Artist/vendor source required |

No trajectory-accumulating root translation or yaw was cyclically reordered. Current safety refusals preserve source behavior but do not make the sets acceptance-ready.

## Engine procedures and evidence

| Runtime | Version | Procedure | Observed result | Remaining gate |
|---|---|---|---|---|
| Unity | 6000.5.8f1 | Merge five authorized package reconstructions in a disposable external project; inventory importers/clips; sample 15 new contextual clips; run 4 cross-pack mixers, 1 mask, 2 props, and expected outlier assertion. | 22/22 required checks pass; expected outlier fails separately; all packs/shared assets co-import. | Visual states, root motion, contacts, target retarget, compression, player build. |
| Unreal Engine | 5.7 | Official root-motion/animation/layering documentation review only. | Not evaluated. | Five-pack import/retarget, graphs, contacts, build. |
| Godot | stable | Official AnimationTree documentation review only. | Not evaluated. | Conversion/import, retarget, graphs, root, export. |
| Bevy | unspecified | Official AnimationGraph mask example review only. | Not evaluated. | glTF conversion, target mapping, graphs, root, performance. |

## Rig, masking, and compatibility evidence

| Pack/rig/set pair | Skeleton/retarget | Scale/axes | Root policy | Timing/blend | Overall evidence |
|---|---|---|---|---|---|
| All standard motion families | Shared 56-bone signature and Unity Humanoid basis | Shared actor/assets identical | Constituent IP/RM policies | Five-pack co-import succeeds | Strong technical co-existence; visual style open |
| Basic hub to four modes | Standard signature | Shared basis | Active mode owns root/pelvis | Campfire, Climbing, Injured mixers plus prior Sword handoff | Implementable candidates; transition visuals open |
| Basic locomotion with Sword/Injured overlays | Humanoid masks execute | Shared basis | Basic owns lower/root | Three retained mask candidates | Prototype only; pelvis/contact/readability open |
| Sword/Campfire props | Shared hand/world nodes available | Headless ratios/instantiation plausible | No movement effect in probe | Static attachment/instantiation | Grip, orientation, contacts, missing props open |
| Five packs to project character | No target supplied | Unknown | Project-specific | Not evaluated | Unknown |
| Five-pack rollup to remaining collection | No artifacts evaluated | Unknown | Unknown | Unknown | No compatibility claim |

## Limitations and unknowns

1. This rollup covers five of 23 current constituents and cannot establish overall collection quality, duplication, compatibility, or value.
2. No target character, production controller, geometry suite, camera, contact specification, networking policy, platform, or performance budget was supplied.
3. Headless Unity proves import and graph execution, not foot planting, style, mask seams, grip/contact, deformation, compression, or build behavior.
4. Unreal Engine, Godot, and Bevy remain documentation-only.
5. Public listings/EULA do not establish local archive revisions or historical purchase terms.
6. Commercial assets, derived motion outputs, screenshots, manifests with local paths, and generated projects remain outside the repository and CI.

## Reproduction

Source-manifest SHA-256 values: Basic `3cc3922dc7b4b06db59643f366eab2844f4490334868ea5a2c26bd1926000cd4`; Sword `b9a5317dcd0ed0a4d46e3c9144cbfa3430ab473354cdf9901c796b8875287d02`; Campfire `11e67cd944ad2058d130eea06f557b41b1ba36e0ed14bbc3289d704d99bf962e`; Climbing `b3807b89f30fb4656446d1e21f41d7405a414025356dd250d9c4a6d212ef3c2f`; Injured `ad98ac7639c997a6d7a3eabb7552b2bbb06ab1c797013cf84cb86e764a3159f5`.

Rollup manifest SHA-256: `f5121a0b74b75bf48ca774807758883accfaced4e0f15517799def724b9cd1c6`. Combined Unity probe SHA-256: `d2b6d1b0af14c2c77dca3c2cc4aa892d6e507f3cf8b9bb50bfdb4ef78d407afa`.

The rollup was built with AnimSmith 0.3.0 at revision `aabac28edf2719db236068339f1208bbf156d0bb` from the five validated constituent manifests. Basic and Sword retain their original evaluator revisions; Campfire, Climbing, and Injured used this revision and binary SHA-256 `2fb43d210b5448fb2cd642946cc46df0cbb34595a48821b22a28daf7c1938f77`.

Rebuild the namespaced manifest, verify all pairwise overlaps, validate its schema, then reconstruct the five authorized Unity deliveries outside the repository and run the retained probe. Re-run constituent contracts when their source, evaluator, declared policy, or target runtime changes.

## Sources

- Constituent evidence: [Basic Locomotion](protofactor-basic-locomotion-evidence.md), [Sword & Shield](protofactor-sword-and-shield-evidence.md), [Campfire](protofactor-campfire-evidence.md), [Climbing](protofactor-climbing-evidence.md), and [Injured](protofactor-injured-evidence.md).
- Protofactor, [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/) and [End User License Agreement](https://protofactor.biz/end-user-license-agreement/) — current context, accessed 2026-08-17.
- Unity Asset Store, [Ultimate Animation Collection](https://marketplace.unity.com/packages/3d/animations/ultimate-animation-collection-195459) — current version 1.65, released 2026-08-16, listed at USD 259.99, accessed 2026-08-17.
- Unity, [Avatar Mask](https://docs.unity3d.com/6000.0/Documentation/Manual/class-AvatarMask.html), [Animation Layers](https://docs.unity3d.com/6000.0/Documentation/Manual/AnimationLayers.html), and [Root Motion](https://docs.unity3d.com/6000.5/Documentation/Manual/RootMotion.html); Epic Games, [Root Motion](https://dev.epicgames.com/documentation/unreal-engine/root-motion-in-unreal-engine?application_version=5.7) and [Layered Animations](https://dev.epicgames.com/documentation/unreal-engine/using-layered-animations-in-unreal-engine?lang=en-US); Godot, [AnimationTree](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html); Bevy, [Animation Masks](https://bevy.org/examples/animation/animation-masks/) — capabilities only.
