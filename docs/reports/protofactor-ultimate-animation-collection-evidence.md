# Animation pack evidence appendix: Protofactor Ultimate Animation Collection (partial: Basic Locomotion + Sword & Shield)

> Companion report: [partial collection technical evaluation](protofactor-ultimate-animation-collection.md)
>
> Evidence status: **partial** — two constituent packs, a combined manifest, exact shared-file comparison, and one Unity combined-project probe; remaining packs and visual acceptance are absent.
>
> Evaluation date: **2026-08-17**
>
> Report format: **1**

This is a collection-level appendix, not a substitute for the [Basic Locomotion](protofactor-basic-locomotion-evidence.md) or [Sword & Shield](protofactor-sword-and-shield-evidence.md) constituent evidence. The [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder) remains authoritative.

## Evaluation scope and provenance

| Field | Value |
|---|---|
| Pack/edition | Protofactor Ultimate Animation Collection, partial rollup of locally held Basic Locomotion and Sword & Shield constituents; local collection/constituent revision unknown |
| Vendor/source | Protofactor; [current Ultimate Animation Collection product page](https://protofactor.biz/product/ultimate-animation-collection/) |
| Delivered scope | Two separate local RARs → two Unitypackages → 315 FBXs total, including 309 individual motion files; no other constituent was extracted or evaluated |
| Target use | Combined game-engine use in one third-person unarmed/armed controller |
| Target engines | Unity 6000.5.8f1 observed; Unreal Engine, Godot, and Bevy documentation-only |
| Target rigs/packs | Supplied Protof-Actor, Basic Locomotion, and Sword & Shield; no project character |
| Source manifest | Constituent logical manifests and exact cross-pack shared-path digest comparison; see reproduction |
| Evaluation manifest | `evidence/ultimate-collection-partial-evaluation-manifest.json`; SHA-256 `571ce1a3710620939c948d842ef4f638927fb41106a7c6f0f541320040f97374`; taxonomy/profile-set version 1 |
| Acquisition/license provenance | `user-stated`: both archives were downloaded from Protofactor.biz as collection content. Current EULA permits protected released real-time applications and modification while restricting redistribution/resale. Receipts, download dates, historical EULA, and exact local collection revision are unavailable; not legal advice. |

The rollup manifest uses schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1`. Current collection listing facts describe today's SKU, not the local artifact or transaction. A technical verdict for these two packs must not be presented as a verdict for every collection constituent.

Evaluated constituents are **Basic Locomotion** and **Sword & Shield**. The 21 local archive labels explicitly excluded from this rollup are **1-Handed Melee Weapon, 2-Handed Gun, 2-Handed Melee Weapon, Assault Rifle, Bazooka, Bow & Arrow, Campfire, Climbing, Combat Bare Fists, Creature, Crowd, Double Guns, Dual Swords, Fencing, Hostage, Injured, Minigun, Push & Pull Cube, Shotgun, Wizard, and Zombie**. Their archive names were inventoried only; their contents, revisions, capabilities, and compatibility were not evaluated.

### Evidence coverage

| Surface | Offered/delivered | Evaluated | Findings | Not evaluated and why |
|---|---:|---:|---:|---|
| Animation files | 315 FBXs | 315 | 309 individual motions; 12 Basic strict time failures; one Sword malformed motion; combined files discouraged | Other collection packs and exhaustive artistic review |
| Rigs/export variants | 5 distinct observed skeleton structures | 5 | Standard 56-bone and actor 58-bone signatures match across packs; specialist/combined/malformed structures differ | Target-character deformation and other engines |
| AnimSmith baseline | 315 FBXs | 315 | Complete constituent summaries retained | Collection-wide semantic contracts beyond two packs |
| Declared contracts | 309 individual files | 309 | Basic 58/177 pass; Sword 17/132 pass under delivered/inferred declarations | Human correction of all loop/action intent |
| Offline visual reports | 309 possible | 17 representative | Static skeleton/pose evidence; malformed Sword file independently visible | Continuous motion/contact/style review |
| Engine import/playback | 2 Unitypackages | 2 | Co-import succeeds; 309 AnimationClips, 308 humanMotion | Other engines, visual controller, player build |
| Blend/mask/retarget | 5 cross-pack graph tests | 5 | 2/2 full-body blends and 3/3 Humanoid masks execute | Visual style, contacts, IK, target-character retarget |

### Claim legend

Evidence labels are `user-stated`, `vendor-stated`, `observed-file`, `observed-animsmith`, `observed-report`, `observed-engine`, `documentation-stated`, `inferred`, and `not-evaluated`.

## Evaluation manifest and taxonomy

### Canonical clip-role inventory

| Canonical primary role | Logical motions | Delivered files | Evidence boundary |
|---|---:|---:|---|
| `idle-pose` | 20 | 20 | Basic general/context holds plus Sword combat/crouch holds. |
| `continuous-locomotion` | 62 | 124 | Basic and combat locomotion; both have IP/RM families and phase findings. |
| `locomotion-transition` | 30 | 60 | Basic turns, pivots, U-turns, and cover boundaries; mostly unarmed. |
| `airborne` | 9 | 13 | Basic jump/fall/landing only. |
| `traversal` | 2 | 4 | Basic left/right 1 m obstacles only. |
| `action-interaction` | 48 | 61 | Basic cover/grenade plus Sword melee/defense/equipment. |
| `reaction-death` | 20 | 24 | Sword only. |
| `emote-cinematic` | 3 | 3 | Sword taunts only. |
| `other-unknown` | 0 | 0 | No individual motion remained unclassified. |
| **Total** | **194** | **309** | Namespaced validated rollup; 107 Basic plus 87 Sword logical motions. |

### Runtime-set inventory

| Runtime set | Type | Members/variants | Grouping evidence | Validation status |
|---|---|---|---|---|
| Armed-state handoff | transition-chain | Basic IdleUnarmed → Sword Draw1 → Sword IdleCombat → Sword PutBack1 | Exact standard signature, shared actor/assets, state semantics; medium confidence | Members/co-import sampled; transition not visually run. |
| Walk + sword attack mask candidate | mask-composition | Basic WalkForwardUnarmed2 IP + SwordAttack1 | User-required scenario, Unity Humanoid mapping; low confidence | Headless mask graph passes; pelvis/contact/grip visual gate open. |
| Run + heavy block mask candidate | mask-composition | Basic RunForward2Unarmed IP + BlockHeavy1 | User-required scenario, Unity Humanoid mapping; low confidence | Headless mask graph passes; shield/contact visual gate open. |
| Basic directional constituent sets | directional-blend | Six IP/RM 8-way rings plus sprint direction candidates | Constituent manifest and report | Keep policy in Basic report; not duplicated here. |
| Basic speed constituent sets | speed-blend | IP/RM forward speed candidates | Constituent manifest and report | Candidate thresholds remain untested. |
| Sword directional constituent sets | directional-blend | Six IP/RM combat rings | Constituent manifest and report | Keep policy in Sword report; Crouch FR RM quarantined. |
| Sword speed constituent sets | speed-blend | Four walk/run speed candidates | Constituent manifest and report | Candidate thresholds remain untested. |
| Sword transition constituent sets | transition-chain | Four death chains and two equipment chains | Constituent manifest and report | Events/crossfades remain untested. |

The first three names match the collection primary report. Namespaced physical members and contracts are retained there; constituent tables own their per-direction timing and speed evidence.

### Pipeline-stage coverage

| Stage | Coverage state | Evidence / remaining gate |
|---|---|---|
| Acquire | `partially-evaluated` | Two local archives/current pages captured; other constituents and historical transaction evidence absent. |
| Preserve raw | `evaluated-clean` | Both source archives remain immutable and separately hashed. |
| Inspect | `evaluated-finding` | All 315 FBXs inspected; 309 individual motions catalogued. |
| Segment | `partially-evaluated` | Individual files preferred; both combined FBXs lack authoritative reusable segmentation. |
| Root motion | `evaluated-finding` | Both packs have IP/RM families; yaw semantics and one malformed Sword RM file remain. |
| Conform | `partially-evaluated` | Unity co-import/shared Avatar works for 308/309 human motions; other engines/target rigs open. |
| Validate | `evaluated-finding` | Constituent contracts and combined Unity probe complete; visual acceptance open. |
| Optimize | `partially-evaluated` | Bounded constituent trials only; no optimized collection promoted. |
| Export | `partially-evaluated` | Native Unity packages co-imported; no collection export for other engines. |
| Gate/report | `partially-evaluated` | Two report pairs and this partial rollup retained; remaining collection scope open. |

### Readiness evidence by clip set

| Role or runtime set | File-ready / clip-ready | Set-ready / rig-use | Runtime / acceptance boundary |
|---|---|---|---|
| Basic unarmed state | All 177 clips import after known mechanical policy; source findings retained. | Broad locomotion/transition coverage; 56/73-bone boundaries and phase findings. | Unity samples pass; full visual controller remains open. |
| Sword armed state | 131/132 human clips; Crouch FR RM quarantined; loop policy requires override. | Standard 56-bone signature matches Basic; props supplied. | Unity samples pass; contacts and visual combat remain open. |
| Armed-state handoff | Exact members readable and skeleton-compatible. | Shared actor/assets; no authored Basic↔Sword transition pair. | Implementable full-body crossfade; target-character visual acceptance required. |
| Mask candidates | Exact members import and Humanoid masks execute. | Basic owns locomotion; pelvis/root/action ownership must be curated. | Prototype only until contact, IK, grip, and deformation pass. |
| Collection as sold | Two constituents classified. | Other pack relationships unknown. | No collection-wide readiness or value conclusion. |

### Validation-profile status

| Validation profile | Selection | Result / next evidence |
|---|---|---|
| Marketplace intake | `selected` — `user-required` | Two-pack intake complete; collection coverage/provenance partial. |
| Blended locomotion | `selected` — `observed-pack-capability` | Constituent rings measured; cross-state transition visual gate open. |
| Root-motion controller | `selected` — `observed-pack-capability` | Both conventions measured; yaw, ownership, interruption, and network behavior open. |
| State-machine transitions | `selected` — `observed-pack-capability` | Two-mode recipe defined; equipment handoff not visually accepted. |
| Layered upper body/weapons | `selected` — `user-required` | Three masks execute; composition/contact acceptance open. |
| Traversal/environment | `selected` — `observed-pack-capability` | Basic provides limited content; armed variants/environment controller open. |
| Contact actions/interactions | `selected` — `observed-pack-capability` | Sword actions/props exist; contacts, events, IK, and targets open. |
| Retargeted/customizable characters | `selected` — `vendor-intended` | Shared source Avatar works; project-character and other engines open. |
| Motion matching/search | `not-selected` | No database contract. |
| Networked movement | `not-selected` | No authority/prediction/rollback contract. |
| Runtime performance | `selected` — `evaluator-selected-generic-scenario` | Constant-track opportunity measured; runtime equivalence/budget open. |

## Pack inventory and content evidence

The two evaluated constituents contain 309 individual motion files: 177 Basic and 132 Sword. Their validated manifests namespace every physical file and combine to 194 logical motions. The rollup retains 29 sets: 10 Basic, 16 Sword, and three collection-owned cross-pack sets.

The exact delivery comparison covers all relative paths in both logical package trees. Twenty-five paths overlap and all 25 contents are byte-identical; there are no same-path byte conflicts. Shared files include the Protof-Actor, materials, textures, and metadata. Standard Basic and Sword clips share skeleton signature `8ea3a291222d`; their shared actor uses `968a9e957f8c`.

Coverage is complementary rather than redundant. Basic owns general movement, turns/pivots, airborne, minimal traversal, cover, and grenade context. Sword owns armed locomotion, melee/defense, reactions/deaths, equipment, taunts, and weapon/shield props. The main overlap is locomotion, which should be treated as unarmed versus armed style states, not merged blindly into one blend space.

## Mechanical baseline

| Finding/check | Affected scope | Potential impact | Evidence |
|---|---|---|---|
| Cross-pack shared paths | 25/25 byte-identical | Co-install does not silently overwrite different shared content in this pair. | SHA-256 path comparison. |
| Standard skeleton | Basic 136 standard files; Sword 131 individual files | Strong direct state-switch and Humanoid retarget prerequisite. | Same 56-bone signature `8ea3a291222d`. |
| Basic time monotonicity | 12/177 files fail raw strict timing | Strict pipelines may reject/clamp; current declared slicing fixes them. | Constituent exhaustive result. |
| Sword malformed hierarchy | 1/132 files | Armed RM crouch ring lacks one member. | Two-node FBX; Unity no AnimationClip. |
| Loop/phase policy | Both packs | Raw normalized-time blends and wraps can skate/pop; metadata over-labels one-shots. | Constituent contract/set measurements. |
| Constant tracks | Every constituent animation file has notes | Optimization opportunity and sparse-track risk. | Complete AnimSmith summaries; sample transforms only. |

## AnimSmith remediation evidence

| Source issue | Operation/declarations | Result | Independent verification | Remaining caveat |
|---|---|---|---|---|
| Basic negative-time keys | Declared `--slice` to Unity clip ranges | 12/12 candidate outputs remove time-monotonic errors | Post-transform inspect/measure/lint | Target-engine/visual acceptance still required. |
| Basic and Sword gait phase | `--gait-anchor` on 48 IP ring trials | 0/48 current outputs; safe refusal on unmeasurable horizontal root basis | Command records show no outputs | Runtime offsets or artist exports; [#426](https://github.com/mmannerm/animsmith/issues/426). |
| Sword duplicate endpoint | Drop WalkForward duplicate endpoint | Closure removed | Post-lint | Seam derivatives remain. |
| Constant tracks | Sample pruning in both packs | Notes removed in generated GLBs | Diff/post-lint retained | Semantic/runtime equivalence unproved; do not promote. |
| Sword malformed hierarchy | No safe operation | Quarantined | AnimSmith and Unity independently agree | Artist/vendor source required. |

No trajectory-accumulating root translation or yaw was cyclically resampled. Current safety refusals preserve source behavior but do not make the gaits set-ready.

## Engine procedures and evidence

| Runtime | Version | Procedure | Observed result | Remaining gate |
|---|---|---|---|---|
| Unity | 6000.5.8f1 | Create disposable project; import Sword then Basic; inventory all importers/clips; sample representatives; evaluate cross-pack mixers and Humanoid masks; attach Sword props. | Both imports exit 0; no shared-file collision; 309 clips/308 humanMotion; 2/2 cross-pack blends and 3/3 masks execute; props attach. | Visual state machines, contacts, root motion, target retarget, compression/build. |
| Unreal Engine | unspecified | Review official Root Motion, Blend Space/Sync Group, Blend Mask, and layered-animation capabilities. | Documentation supports the design; neither package imported. | FBX import/retarget, graphs, contacts, build. |
| Godot | stable | Review AnimationTree blend/filter/one-shot/sync/root-motion procedures. | Documentation supports the design; neither package imported. | Conversion/import, retarget, graphs, root, export. |
| Bevy | unspecified | Review AnimationGraph mask example and current retargeting limitation. | Mask graph capability exists; FBX conversion/retarget path not established. | glTF conversion, target mapping, graphs, root, performance. |

## Rig, masking, and compatibility evidence

| Pack/rig/set pair | Skeleton/retarget | Scale/axes | Root policy | Timing/blend | Overall evidence |
|---|---|---|---|---|---|
| Basic ↔ Sword standard clips | Same 56-bone signature and Unity Humanoid source path | Shared actor/assets byte-identical | Both use IP/`_RM`; yaw incomplete | Two cross-pack full-body blends execute | Strong technical candidate; visual/style acceptance open. |
| Basic actor ↔ Sword props | Shared actor identical | Sword/shield local-identity height ratios 0.429/0.376 | No movement effect in attachment probe | Static attachment only | Plausible Unity setup; grip/orientation/contact open. |
| Basic locomotion ↔ Sword upper action | Humanoid mask mapping succeeds | Shared rig basis | Basic should own root/pelvis; action policy clip-specific | Three mask graphs execute | Prototype only; full-body default recommended. |
| Basic traversal/turn ↔ armed Sword state | Skeleton-compatible | Shared basis | Active state owns movement | No authored armed counterpart or visual test | Content gap; artist/engine decision required. |
| Both packs ↔ project character | No target supplied | Unknown | Project-specific | Not evaluated | Unknown. |
| Two-pack rollup ↔ remaining collection | No artifacts evaluated | Unknown | Unknown | Unknown | No compatibility claim. |

## Limitations and unknowns

1. This rollup covers two constituents only; it does not estimate the quality, compatibility, duplication, or value of the remaining Ultimate Animation Collection.
2. No target character, controller, camera, combat/contact specification, platform, performance budget, networking policy, or artistic acceptance bar was supplied.
3. Unity headless evaluation proves import and graph execution, not foot planting, style continuity, mask seams, prop grip, hit timing, contacts, IK, deformation, compression, or build behavior.
4. Unreal Engine, Godot, and Bevy remain documentation-only. Bevy also requires a viable FBX-to-glTF/retarget route.
5. Constituent loop declarations include obvious one-shot-like clips, so raw contract failure totals cannot be read as visually bad loop counts.
6. Cross-pack masks are candidates, not recommendations for kicks, lunges, pelvis-driven attacks, or other displacement-bearing actions.
7. Current public listings/EULA do not establish local archive revisions or historical purchase terms.

## Reproduction

- Basic source RAR SHA-256: `6f821f56f84339ea1eb6fcaa97e3c70d4a38dd84c413012847f026748dff185f`.
- Basic evaluation manifest SHA-256: `3cc3922dc7b4b06db59643f366eab2844f4490334868ea5a2c26bd1926000cd4`.
- Sword source RAR SHA-256: `4402f20ba681ec83cf01f60b8dfb69b59435b48408030a5fbb4f3454f64840d7`.
- Sword evaluation manifest SHA-256: `b9a5317dcd0ed0a4d46e3c9144cbfa3430ab473354cdf9901c796b8875287d02`.
- Partial rollup manifest SHA-256: `571ce1a3710620939c948d842ef4f638927fb41106a7c6f0f541320040f97374`.
- Exact cross-pack comparison SHA-256: `346e254927a65de26307a5e82da29f70d642c69cea3347b840fb0761e32a4142`.
- Combined Unity probe SHA-256: `c4310bedddfd27e06696207e8bb1c4076039126c467ed4964aba067c8524c392`.
- Evaluators: AnimSmith 0.3.0 at Basic revision `3857fe130c227918e09473b2e1e307f61867439e` and Sword/rollup revision `c11f135ece5e980e6c98861a52a715a28a424ff9`.

The combined Unity procedure imports Sword first and Basic second, then runs one Editor probe over both asset roots. The probe inventories clips/Avatars, samples nine Sword representatives, evaluates three full-body mixes (two cross-pack), evaluates three cross-pack Humanoid masks, and attaches both props. Licensed inputs and generated projects remain local; only portable identities and conclusions are committed.

## Sources

- [Protofactor Basic Locomotion technical report](protofactor-basic-locomotion.md) and [evidence appendix](protofactor-basic-locomotion-evidence.md).
- [Protofactor Sword & Shield technical report](protofactor-sword-and-shield.md) and [evidence appendix](protofactor-sword-and-shield-evidence.md).
- Protofactor, [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/) — current collection description/constituents, accessed 2026-08-17.
- Protofactor, [End User License Agreement](https://protofactor.biz/end-user-license-agreement/) — current protected-use/modification/redistribution terms, not historical transaction evidence, accessed 2026-08-17.
- Unity, [Avatar Mask](https://docs.unity3d.com/es/current/Manual/class-AvatarMask.html), [Animation Layers](https://docs.unity3d.com/6000.0/Documentation/Manual/AnimationLayers.html), and [Root Motion](https://docs.unity3d.com/6000.0/Documentation/Manual/RootMotion.html) — runtime capabilities only.
- Epic Games, [Root Motion](https://dev.epicgames.com/documentation/unreal-engine/root-motion-in-unreal-engine?application_version=5.7) and [Layered Animations](https://dev.epicgames.com/documentation/unreal-engine/using-layered-animations-in-unreal-engine?lang=en-US) — runtime capabilities only.
- Godot, [Using AnimationTree](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html); Bevy, [Animation Masks](https://bevy.org/examples/animation/animation-masks/) and [retargeting issue #15612](https://github.com/bevyengine/bevy/issues/15612) — capability and limitation context only.
- AnimSmith [#426](https://github.com/mmannerm/animsmith/issues/426) — follow-up for in-place rigs whose root basis is unsuitable for current gait anchoring.
