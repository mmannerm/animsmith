# Animation pack evidence appendix: Protofactor Ultimate Animation Collection (partial: eight packs)

> Companion report: [partial collection technical evaluation](protofactor-ultimate-animation-collection.md)
>
> Evidence status: **partial** — exact AnimSmith 0.7.0 mechanical reruns and bounded projections cover eight constituents; all 28 pairwise shared-file comparisons and dated Unity observations remain available; 15 listed constituents and visual acceptance are absent.
>
> Evaluation date: **2026-08-26**
>
> Current evaluator: **AnimSmith 0.7.0**
>
> Report format: **2**

This collection appendix links rather than replaces the [Basic Locomotion](protofactor-basic-locomotion-evidence.md), [Sword & Shield](protofactor-sword-and-shield-evidence.md), [Campfire](protofactor-campfire-evidence.md), [Climbing](protofactor-climbing-evidence.md), [Injured](protofactor-injured-evidence.md), [1-Handed Melee](protofactor-one-handed-melee-evidence.md), [2-Handed Melee](protofactor-two-handed-melee-evidence.md), and [Dual Swords](protofactor-dual-swords-evidence.md) evidence. The [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder) remains authoritative.

## Evaluation scope and provenance

| Field | Value |
|---|---|
| Pack/edition | Protofactor Ultimate Animation Collection; partial rollup of eight locally held constituents; local collection/constituent revisions unknown |
| Vendor/source | Protofactor [current collection page](https://protofactor.biz/product/ultimate-animation-collection/) and constituent pages |
| Delivered scope | Eight RARs to eight Unitypackages; 918 FBXs total, including 895 individual motion files; 15 listed constituents not extracted/evaluated in this wave |
| Target use | One third-person project with unarmed, four weapon-specific armed, injured, camp, and traversal gameplay modes |
| Target engines | Unity 6000.5.8f1 observed; Unreal Engine 5.7, Godot stable, and Bevy documentation-only |
| Target rigs/packs | Protof-Actor family across all eight evaluated constituents, including the 2-Handed 58-bone twist variant; no project character |
| Source manifest | Eight separately hashed constituent manifests plus all 28 pairwise logical-delivery comparisons |
| Evaluation manifest | `evidence/ultimate-collection-eight-pack-partial-evaluation-manifest.json`; SHA-256 `b3ac8ec9b2bfde35edbf5b240f51d0a875d2213c1b35b8ce7b101635b691b309`; schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1`; taxonomy/profile-set version 1 |
| Acquisition/license provenance | User states the local archives were downloaded from Protofactor as collection content. Current [vendor EULA](https://protofactor.biz/end-user-license-agreement/) reviewed 2026-08-17; receipts, historical terms, and exact revisions unavailable; no legal opinion. |

Evaluated: **Basic Locomotion, Sword & Shield, Campfire, Climbing, Injured, 1-Handed Melee Weapon, 2-Handed Melee Weapon, and Dual Swords**. The 15 current/local labels excluded are **2-Handed Gun, Assault Rifle, Bazooka, Bow & Arrow, Combat Bare Fists, Creature, Crowd, Double Guns, Fencing, Hostage, Minigun, Push & Pull Cube, Shotgun, Wizard, and Zombie**. Names were inventoried only; contents and compatibility were not evaluated.

### Evidence coverage

| Surface | Offered/delivered | Evaluated | Findings | Not evaluated and why |
|---|---:|---:|---:|---|
| Animation/model files | 918 FBXs | 918 via constituent runs | 895 individual files; known strict-time/seam/rig findings | 15 constituent archives and exhaustive artistic review |
| Canonical inventory | 895 individual files | 582 logical motions, 90 manifest set records | Eight namespaced validated manifests | Visual semantic correctness |
| Shared logical paths | 28 pack pairs × 25 overlaps | 700 comparisons | 700 byte-identical; zero conflicts | Unevaluated constituents |
| Engine import/playback | Eight reconstructed Unity deliveries | Eight in one project | Melee 33/33 required passes plus four expected Generic failures; contextual 22/22 retained | Visual controller, compression, player build |
| Cross-pack composition | 6 mixers, 3 masks, 4 prop attachments | Dated Unity observations | All required checks execute | Style, contacts, IK, offsets, deformation |
| Other engines | Three runtime documentation sets | Documentation only | Capabilities exist | Pack import/retarget/runtime behavior |

### Claim legend

`user-stated`, `vendor-stated`, `observed-file`, `observed-animsmith`, `observed-report`, `observed-engine`, `documentation-stated`, `inferred`, and `not-evaluated` keep acquisition, computation, execution, and inference separate.

## Evaluation manifest and taxonomy

The retained rollup uses schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1` and taxonomy/profile-set version 1.

### Canonical clip-role inventory

| Canonical primary role | Logical motions | Delivered files | Evidence boundary |
|---|---:|---:|---|
| `idle-pose` | 75 | 75 | General, weapon-combat, injury, camp, and traversal holds |
| `continuous-locomotion` | 166 | 332 | Basic, armed, and Injured IP/RM families |
| `locomotion-transition` | 66 | 96 | Turns/pivots, posture, and preparation boundaries |
| `airborne` | 25 | 29 | Basic, Climbing, and armed fall/land families |
| `traversal` | 32 | 62 | Basic obstacle and broad Climbing content |
| `action-interaction` | 141 | 208 | Four weapon styles, equipment, camp, cover, and grenade actions |
| `reaction-death` | 74 | 90 | Armed hit, block, downed, and death families |
| `emote-cinematic` | 3 | 3 | Sword taunts only |
| `other-unknown` | 0 | 0 | None |
| **Total** | **582** | **895** | Validated eight-pack rollup SHA-256 `b3ac8ec9b2bfde35edbf5b240f51d0a875d2213c1b35b8ce7b101635b691b309` |

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
| Basic-to-1-Handed armed state | transition-chain | Basic idle plus 1-Handed draw/combat idle/put-back | Eight-pack Unity mixer and shared Humanoid path; low confidence | Weapon/contact transition visual-open |
| Basic-to-2-Handed armed state | transition-chain | Basic idle plus 2-Handed draw/combat idle/put-back | Eight-pack Unity mixer and Humanoid retarget path; low confidence | Twist-rig deformation and transition visual-open |
| Basic-to-Dual-Swords armed state | transition-chain | Basic idle plus Dual draw/combat idle/put-back | Eight-pack Unity mixer and shared standard rig; low confidence | Dual-grip/contact transition visual-open |
| Walk + 1-Handed attack mask candidate | mask-composition | Basic walk plus 1-Handed Attack A | Eight-pack Unity Humanoid mask execution; low confidence | Pelvis/weapon/contact visual-open |
| Walk + 2-Handed attack mask candidate | mask-composition | Basic walk plus 2-Handed Attack A | Eight-pack Unity Humanoid mask execution; low confidence | Two-hand alignment/twist deformation visual-open |
| Walk + Dual-Swords attack mask candidate | mask-composition | Basic walk plus Dual Attack 1 | Eight-pack Unity Humanoid mask execution; low confidence | Bilateral grip/pelvis/contact visual-open |
| Constituent-owned sets | other | 76 manifest records: 10 Basic, 16 Sword, 6 Campfire, 10 Climbing, 14 Injured, 6 1-Handed, 7 2-Handed, and 7 Dual Swords | Eight constituent manifests | Exact contracts remain in linked reports |

The count above is the versioned manifest-record count: 76 constituent records plus 14 namespaced cross-pack records produce 90 total. Constituent reports may render paired IP/RM relationships differently; their linked evidence remains authoritative for exact member measurements.

### Pipeline-stage coverage

| Stage | Coverage state | Evidence / remaining gate |
|---|---|---|
| Acquire | `partially-evaluated` | Eight local archives/current pages captured; 15 constituents and historical transaction evidence absent. |
| Preserve raw | `evaluated-clean` | Each evaluated source archive remains immutable and separately hashed outside the repository. |
| Inspect | `evaluated-finding` | All 918 evaluated FBXs inspected through constituent runs. |
| Segment | `partially-evaluated` | Individual files preferred; combined takes not promoted. |
| Root motion | `evaluated-finding` | IP/RM families inventoried; vertical/yaw evidence and outliers remain bounded. |
| Conform | `partially-evaluated` | Standard 56-bone family, 58-bone 2-Handed twist variant, and shared Unity Avatars co-exist; outliers and target/other engines open. |
| Validate | `evaluated-finding` | Constituent contracts and combined Unity graph probes ran; visuals open. |
| Optimize | `partially-evaluated` | AnimSmith 0.7.0 emits 159 candidates across the collection: 134 gait-anchor, 12 slice, 12 prune-constant-tracks, and 1 drop-duplicate-loop-endpoint. Pruning stays bounded by open [#401](https://github.com/mmannerm/animsmith/issues/401); no generated collection output was promoted. |
| Export | `partially-evaluated` | Native Unity assets co-imported; other-engine exports not accepted. |
| Gate/report | `partially-evaluated` | Eight report pairs and this rollup are versioned; 15 constituents and visual gates remain. |

### Readiness evidence by clip set

| Role or runtime set | File-ready / clip-ready | Set-ready / rig-use | Runtime / acceptance boundary |
|---|---|---|---|
| Basic unarmed mode | 177 individual clips; known declared slicing/loop policy | Broad controller foundation | Unity evidence retained; visual controller open |
| Sword armed mode | 132 files; one RM outlier quarantined | Combat/equipment/props; shared rig | Mix/masks/props execute; contacts open |
| Campfire contextual mode | 25 motions and 2 props readable | Full-body posture chains; missing implied props/events | Mixer/props execute; contacts/loops open |
| Climbing traversal mode | 74 standard clips plus one excluded outlier | IP/RM traversal families; vertical/root/contact gaps | Samples/mixer execute; environment matrix open |
| Injured mode | 70 clips; 14 IP/RM gait pairs | Seven style-specific speed/posture sets | Mix/mask execute; phase/loop/transitions open |
| 1-Handed mode | 110 individual clips; 108 import as Humanoid, two Generic block files quarantined | Common 56-bone family except one separate block structure; 24 IP gait candidates reduce three ring spreads | Raw samples/mix/mask/prop execute; generated GLBs and combat contacts open |
| 2-Handed mode | 120 individual clips; 118 dominant 58-bone Humanoid files, two Generic block files quarantined | Explicit AnimSmith roles recover measurement; 24 IP gait candidates reduce three ring spreads; Unity retargets raw twist variant | Raw samples/mix/mask/prop execute; generated GLBs, deformation, and two-hand contacts open |
| Dual-Swords mode | 186 individual clips; all import as Humanoid | Common 56-bone family; 24 IP gait candidates reduce three ring spreads | Raw samples/mix/mask/two prop attachments execute; generated GLBs and bilateral contacts open |
| Cross-pack candidates | Exact members, digest, skeleton, and Humanoid evidence | Fourteen collection-owned sets | Graph execution only; target visual gate required |
| Collection as sold | Eight of 23 current constituents classified | Remaining relationships unknown | No collection-wide readiness/value conclusion |

### Validation-profile status

| Validation profile | Selection | Result / next evidence |
|---|---|---|
| Marketplace intake | `selected` — `user-required` | Eight-pack intake complete; collection coverage/provenance partial. |
| Blended locomotion | `selected` — `observed-pack-capability` | Constituent sets measured; cross-mode visual gates open. |
| Root-motion controller | `selected` — `observed-pack-capability` | Paired conventions measured; vertical/yaw/authority/network behavior open. |
| State-machine transitions | `selected` — `observed-pack-capability` | Eight-mode recipe and cross-pack candidates defined; visuals open. |
| Layered upper body/weapons | `selected` — `user-required` | Masks execute; composition/contact acceptance open. |
| Traversal/environment | `selected` — `observed-pack-capability` | Climbing plus Basic obstacles; environment matrix absent. |
| Contact actions/interactions | `selected` — `observed-pack-capability` | Four melee packs plus Campfire/Climbing imply contacts; events/IK/targets open. |
| Retargeted/customizable characters | `selected` — `vendor-intended` | Shared source Avatar and 2-Handed Humanoid retarget execute; project character and other engines open. |
| Motion matching/search | `not-selected` | No database contract. |
| Networked movement | `not-selected` | No authority/prediction/rollback contract. |
| Runtime performance | `selected` — `evaluator-selected-generic-scenario` | Constant-track opportunities sampled; runtime equivalence/budget open. |

## Pack inventory and content evidence

The eight manifests namespace 895 individual files into 582 logical motions and 90 runtime-set records: 76 constituent-owned plus 14 collection-owned. Basic supplies general locomotion/transitions; Sword, 1-Handed, 2-Handed, and Dual Swords supply distinct combat/equipment modes; Campfire supplies rest/interactions; Climbing supplies traversal; Injured supplies seven hurt movement/posture styles.

Every pairwise logical-delivery comparison found exactly 25 overlapping paths, all byte-identical. Shared material/actor assets therefore co-install without same-path byte conflicts in this snapshot. Accepted standard motion files across seven packs, including 1-Handed, use the 56-bone signature `2b6fe49d5ae6`. The 118 accepted 2-Handed files use a 58-bone signature `3da84463466a`, adding left/right forearm twists and capitalized `Humanoid_` names; Unity Humanoid graph execution bridges that structural difference, but target-character deformation remains open. Specialist, actor/combined, prop, and malformed/Generic outlier structures remain separately classified.

## Mechanical baseline

| Finding/check | Affected scope | Potential impact | Evidence |
|---|---|---|---|
| Shared paths | 700/700 pairwise overlaps identical | Evaluated packs do not silently overwrite different shared content | Twenty-eight SHA-256 comparisons |
| Standard skeleton | Accepted standard motion families across seven packs, including 1-Handed | Strong co-install/state-switch prerequisite | Current order-independent 56-bone signature |
| 2-Handed twist skeleton | 118 accepted files | Adds two forearm-twist targets; target deformation may differ | 58-bone signature, built-in case-tolerant role resolution, and Unity Humanoid execution |
| Basic raw timing | 12 files | Strict pipelines may reject/clamp without declared slicing | Constituent report |
| Rig/import outliers | Sword Crouch FR RM; Climbing FallingUnarmed; four 1-/2-Handed Generic block files | Specific RM, airborne, or blocking paths unusable | AnimSmith plus Unity evidence |
| Default role resolution | 118 dominant 2-Handed files | Unique case-only aliases resolve without project overrides; ambiguity would refuse | Current structured output retains exact delivered names and `ascii-case-insensitive` provenance |
| Loop/phase policy | All eight constituents | Wrap pulses, skating, or wrong semantic repetition | Exhaustive constituent contracts |
| Constant tracks | Broadly present | Optimization opportunity and retarget overhead | Constituent summaries and bounded transforms |

## AnimSmith remediation evidence

| Source issue | Operation/declarations | Result | Independent verification | Remaining caveat |
|---|---|---|---|---|
| Basic negative-time keys | Declared slice to Unity clip ranges | 12/12 candidate outputs remove strict-time errors | Post-transform checks | Visual/engine acceptance still required |
| Gait phase across packs | Current evaluation of every selected IP ring member | All 134 transforms across the six gait-bearing packs emit GLB candidates. Circular walk/run/crouch spreads: Basic 0.6598/0.4630/0.7156→0.072442/0.093840/0.050191; Sword & Shield 0.7231/0.6605/0.6974→0.059938/0.137277/0.052440; Injured walk/run 0.6025/0.5541→0.051348/0.109841; 1-Handed 0.5538/0.7342/0.7136→0.063903/0.108198/0.039432; 2-Handed 0.7112/0.6024/0.5774→0.069337/0.142914/0.053758; Dual Swords 0.7086/0.6732/0.6184→0.052993/0.135051/0.058715. | All 134 candidates re-read under `inspect`; spread uses the smallest containing arc. | Only IP members transformed; Campfire and Climbing have no in-place ring. Runtime offsets, engine import, visual review, and trajectory acceptance remain open. |
| 2-Handed role resolution | Built-in humanoid profile on the capitalized 58-bone family | Resolves role-backed measurements on all 118 accepted files without modifying assets or requiring a role override | Exact delivered names and per-role case-resolution policies | Target-rig retarget/deformation quality remains untested |
| Constant tracks | Representative pruning in all eight packs | Smaller GLBs reopen and retain declared findings | Inspect/measure/lint/diff/fix dry-run | Runtime equivalence unproven; no candidate promoted |
| Rig/import outliers | No safe repair | Six files quarantined/excluded | AnimSmith and Unity agree | Artist/vendor source required |

Only manifest-selected in-place ring members were transformed; no root-motion member was cyclically reordered. Current candidates remain unpromoted pending engine import, visual review, and residual phase policy.

## Engine procedures and evidence

| Runtime | Version | Procedure | Observed result | Remaining gate |
|---|---|---|---|---|
| Unity | 6000.5.8f1 | Merge eight authorized package reconstructions in one disposable external project. Contextual checks cover 15 samples, 4 mixers, 1 mask, and 2 props; melee checks cover 20 samples, 6 mixers, 3 masks, 4 prop attachments, and 4 expected Generic assertions. | Contextual 22/22 and melee 33/33 required checks pass. Four expected Generic files fail Humanoid playback separately; all eight packs and shared assets co-import. | Visual states, root motion, contacts, target retarget, compression, player build. |
| Unreal Engine | 5.7 | Official root-motion/animation/layering documentation review only. | Not evaluated. | Eight-pack import/retarget, graphs, contacts, build. |
| Godot | stable | Official AnimationTree documentation review only. | Not evaluated. | Conversion/import, retarget, graphs, root, export. |
| Bevy | unspecified | Official AnimationGraph mask example review only. | Not evaluated. | glTF conversion, target mapping, graphs, root, performance. |

## Rig, masking, and compatibility evidence

| Pack/rig/set pair | Skeleton/retarget | Scale/axes | Root policy | Timing/blend | Overall evidence |
|---|---|---|---|---|---|
| Standard 56-bone motion families | Identical signature across seven packs' accepted standard files | Shared actor/assets identical | Constituent IP/RM policies | Eight-pack co-import succeeds | Strong technical co-existence; visual style open |
| 2-Handed 58-bone family to other modes | Forearm twists and capitalized identifiers differ; Unity Humanoid mixers execute | Shared actor scale; target axes/deformation not visually checked | Active mode owns root/pelvis | Basic and Sword idle mixers pass headlessly | Engine-config path observed; target-character deformation open |
| Basic hub to seven modes | Standard signature or Unity Humanoid retarget path | Shared basis | Active mode owns root/pelvis | Seven mode handoffs have retained graph evidence | Implementable candidates; transition visuals open |
| Basic locomotion with armed/Injured overlays | Humanoid masks execute | Shared basis | Basic owns lower/root | Six Sword/1-Handed/2-Handed/Dual-Swords/Injured mask candidates retained | Prototype only; pelvis/contact/readability open |
| Armed/Campfire props | Shared hand/world nodes available | Headless ratios/instantiation plausible | No movement effect in probe | Static attachment/instantiation | Grip, orientation, contacts, missing props open |
| Eight packs to project character | No target supplied | Unknown | Project-specific | Not evaluated | Unknown |
| Eight-pack rollup to remaining collection | No artifacts evaluated | Unknown | Unknown | Unknown | No compatibility claim |

## Limitations and unknowns

1. This rollup covers eight of 23 current constituents and cannot establish overall collection quality, duplication, compatibility, or value.
2. No target character, production controller, geometry suite, camera, contact specification, networking policy, platform, or performance budget was supplied.
3. Headless Unity proves import and graph execution, not foot planting, style, mask seams, grip/contact, 2-Handed twist deformation, compression, or build behavior.
4. Unreal Engine, Godot, and Bevy remain documentation-only.
5. Public listings/EULA do not establish local archive revisions or historical purchase terms.
6. The current gait outputs are GLB files, while the disposable Unity project has no GLB importer; no generated candidate was engine-imported, visually reviewed, or independently accepted.
7. Commercial assets, derived motion outputs, screenshots, manifests with local paths, and generated projects remain outside the repository and CI.

## Changes between AnimSmith versions

| Evaluator | Change from the preceding evaluated state |
|---|---|
| AnimSmith 0.7.0 | Revalidated all eight evaluated constituents under output v17 / measurements v16, retained 159 unpromoted candidates, and added current engine projections. The 2-Handed default-profile role gap is resolved through fail-closed case-only aliases. |
| AnimSmith 0.4.1 | Reproduced the evaluated 0.4.0 measurements and transforms across the retained collection scope; unrelated release fixes did not change the rollup conclusion. |
| AnimSmith 0.4.0 | Added consolidated constituent and profile evidence while retaining the eight-of-23 partial scope and the same engine/artistic gates. |
| AnimSmith 0.3.x | Established the original constituent baselines, collection comparisons, dated Unity probes, and first gait trials. Those evaluator results are superseded. |

## Reproduction

### Current AnimSmith reproduction (2026-08-26)

Every one of the eight evaluated constituents was re-inventoried or reconciled and rerun under `animsmith 0.7.0`, exact tag `v0.7.0`, commit `461ac8a4f6bb368eb8637471a796f13eeb647140`, binary SHA-256 `01a501999c91d93abfb32b1f48241fccc70914fac27c9a650c31df44262578d8`, output schema v17, and measurements schema v16. The constituent appendices bind their individual baseline, contract, remediation, manifest, and supplemental-projection digests.

All 159 current GLB remediation candidates emitted immutable addressability V1 and Bevy revision-3 rich addressability V2 with 64-bit target UUIDs. Representative Unity Humanoid revision-1, Unreal revision-2, and Godot revision-2 advice projections were available for every constituent. This is sealed addressability and version-pinned settings evidence only; it does not replace the dated Unity import observations or fill the Unreal/Godot/Bevy execution, retarget, visual, contact, gameplay, or artistic gates.

The collection is partial at eight of 23 constituents. The bound evidence contains 28 pairwise shared-path comparisons, all byte-identical, and the current source inventories match that comparison scope.

## Sources

- Constituent evidence: [Basic Locomotion](protofactor-basic-locomotion-evidence.md), [Sword & Shield](protofactor-sword-and-shield-evidence.md), [Campfire](protofactor-campfire-evidence.md), [Climbing](protofactor-climbing-evidence.md), [Injured](protofactor-injured-evidence.md), [1-Handed Melee](protofactor-one-handed-melee-evidence.md), [2-Handed Melee](protofactor-two-handed-melee-evidence.md), and [Dual Swords](protofactor-dual-swords-evidence.md).
- Protofactor, [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/) and [End User License Agreement](https://protofactor.biz/end-user-license-agreement/) — current context, accessed 2026-08-17.
- Unity Asset Store, [Ultimate Animation Collection](https://marketplace.unity.com/packages/3d/animations/ultimate-animation-collection-195459) — current version 1.65, released 2026-08-16, listed at USD 259.99, accessed 2026-08-17.
- Unity, [Avatar Mask](https://docs.unity3d.com/6000.0/Documentation/Manual/class-AvatarMask.html), [Animation Layers](https://docs.unity3d.com/6000.0/Documentation/Manual/AnimationLayers.html), and [Root Motion](https://docs.unity3d.com/6000.5/Documentation/Manual/RootMotion.html); Epic Games, [Root Motion](https://dev.epicgames.com/documentation/unreal-engine/root-motion-in-unreal-engine?application_version=5.7) and [Layered Animations](https://dev.epicgames.com/documentation/unreal-engine/using-layered-animations-in-unreal-engine?lang=en-US); Godot, [AnimationTree](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html); Bevy, [Animation Masks](https://bevy.org/examples/animation/animation-masks/) — capabilities only.
