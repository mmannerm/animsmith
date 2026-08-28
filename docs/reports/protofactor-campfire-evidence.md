# Animation pack evidence appendix: Protofactor Campfire

> Companion report: [Protofactor Campfire](protofactor-campfire.md)
>
> Evidence status: **partial** — exact AnimSmith 0.7.0 baseline, contracts, pruning verification, addressability, and bounded advice plus a dated Unity 6000.5.8f1 observation; visual contact, target-character, and engine-editor/runtime passes remain absent.
>
> Evaluation date: **2026-08-26**
>
> Current evaluator: **AnimSmith 0.7.0**
>
> Report format: **2**

This appendix uses the [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder) without redefining it.

## Evaluation scope and provenance

| Field | Value |
|---|---|
| Pack/edition | Protofactor Campfire constituent from a locally held Ultimate Animation Collection archive; local revision unknown |
| Vendor/source | Protofactor [Campfire product](https://protofactor.biz/product/animset-campfire/) and [collection](https://protofactor.biz/product/ultimate-animation-collection/) pages |
| Delivered scope | RAR to one Unitypackage to 114 logical files; 29 FBXs: 25 individual motions, one combined take, one actor, campfire prop, skewer prop |
| Target use | Game-engine camp/rest state machine, contextual actions, props, and combination with evaluated collection packs |
| Target engines | Unity 6000.5.8f1 observed headless (retained 2026-08-17); Unity 6000.3, Unreal Engine 5.8, and Godot 4.7 AnimSmith import-advice probed (2026-08-21); Bevy 0.19.0 documentation-only, no generated glTF/GLB candidate |
| Target rigs/packs | Supplied Protof-Actor; Basic Locomotion, Sword & Shield, Climbing, and Injured selective compatibility |
| Source manifest | `campfire/source-archive-inventory.json`; RAR SHA-256 `bed86be7f91fdd46b376fce4b1a00c88372a3f703e0fe9077925712d8af8e8e9`; re-inventoried 2026-08-21 under AnimSmith 0.7.0, byte-identical to the published manifest (0 added, 0 removed, 0 changed) |
| Evaluation manifest | `campfire/evidence/evaluation-manifest.json`; SHA-256 `11e67cd944ad2058d130eea06f557b41b1ba36e0ed14bbc3289d704d99bf962e`; schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1`; taxonomy/profile-set version 1; unchanged by the 2026-08-21 refresh |
| Acquisition/license provenance | User states the local archive was downloaded from Protofactor; current [vendor EULA](https://protofactor.biz/end-user-license-agreement/) reviewed 2026-08-17. No receipt or local revision record was evaluated; no legal opinion. |

### Evidence coverage

| Surface | Offered/delivered | Evaluated | Findings | Not evaluated and why |
|---|---:|---:|---:|---|
| Animation files | 25 individual plus 1 combined | 26 | 8 individual contract failures | Dynamic visual quality and combined-take segmentation |
| Rigs/export variants | 4 observed signatures | 4 | Standard 56; combined/actor 58; campfire 3; skewer 19 bones | Target-character deformation |
| AnimSmith baseline | 29 FBXs | 29 | 3,664 constant-track notes in 27 animation-bearing files | No semantic intent in default lint |
| Declared contracts | 25 individual files | 25 | 8 failing files; 3,394 notes | Contact/events and artistic endpoints |
| Offline visual reports | 25 possible | 3 | Reports render skeleton, metrics, and findings | Dynamic visual acceptance |
| Engine import/playback | 25 individual clips | 25 imported; 4 sampled | Required samples pass | Controller, compression, player build |
| Blend/mask/retarget | 1 cross-pack mixer; 2 props | 1 mixer; 2 props | Execution/instantiation pass | Visual blend, prop orientation/contact, target rig |

### Claim legend

`observed-file` means derived from delivered files/metadata; `observed-animsmith` means reproduced with the named evaluator; `observed-engine` means the headless Unity probe; `inferred` marks semantic grouping. None of these labels means gameplay acceptance.

## Evaluation manifest and taxonomy

The retained evaluation manifest uses schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1` and taxonomy/profile-set version 1.

### Canonical clip-role inventory

| Canonical primary role | Logical motions | Delivered files | Evidence boundary |
|---|---:|---:|---|
| `idle-pose` | 7 | 7 | Filename/state semantics; all stationary |
| `continuous-locomotion` | 0 | 0 | Absent |
| `locomotion-transition` | 11 | 11 | Posture-state transitions inferred from exact filenames |
| `airborne` | 0 | 0 | Absent |
| `traversal` | 0 | 0 | Absent |
| `action-interaction` | 7 | 7 | Lighting, skewer, and log actions; contacts unaccepted |
| `reaction-death` | 0 | 0 | Absent |
| `emote-cinematic` | 0 | 0 | Absent |
| `other-unknown` | 0 | 0 | None |
| **Total** | **25** | **25** | Validated manifest SHA-256 `11e67cd944ad2058d130eea06f557b41b1ba36e0ed14bbc3289d704d99bf962e` |

### Runtime-set inventory

| Runtime set | Type | Members/variants | Grouping evidence | Validation status |
|---|---|---|---|---|
| Stand-kneel-sit-rest chain | transition-chain | 7 single files | Exact reciprocal/state labels; medium confidence | Mechanical complete; endpoints visual-open |
| Sit-lie chain | transition-chain | 4 single files | Reciprocal sit/lie labels; high confidence | Detailed members in primary; visual-open |
| Stand-sleep chain | transition-chain | 3 single files | Stand/sleep labels; high confidence | Mechanical complete; visual-open |
| Kneel-grill chain | transition-chain | 4 single files | Kneel/grill/skewer family; medium confidence | Detailed members in primary; contact-open |
| Fire-lighting alternatives | other | 4 single files | Four named techniques; high confidence | Detailed members in primary; use discretely |
| Log-toss alternatives | other | 2 single files | Sitting/kneeling alternatives; medium confidence | Missing log/contact gate |

### Pipeline-stage coverage

| Stage | Coverage state | Evidence / remaining gate |
|---|---|---|
| Acquire | `evaluated-clean` | Local vendor archive identified and hashed; transaction record absent. |
| Preserve raw | `evaluated-clean` | RAR and Unitypackage retained unchanged outside the repository. |
| Inspect | `evaluated-finding` | Every FBX inspected/measured/linted; declared loop and constant-track findings retained. |
| Segment | `partially-evaluated` | Individual files used; combined take not promoted. |
| Root motion | `evaluated-clean` | No root-motion-labelled constituent motion files; AnimSmith 0.7.0 `root_trajectory` measures 27/27 clips: 0 move more than 1 cm horizontally and 0 exceed 1° of yaw travel. Sampled regression facts, not continuous-curve or engine-extraction proof; do not derive movement-ownership axes from this alone. |
| Conform | `partially-evaluated` | Standard skeleton and Unity shared Avatar work; target rigs/other engines open. |
| Validate | `partially-evaluated` | Mechanical contracts and headless Unity complete; visual gameplay open. |
| Optimize | `evaluated-finding` | One pruning candidate verified mechanically but not accepted semantically. |
| Export | `partially-evaluated` | Sample GLB export reopens; Unity native delivery tested; other engines open. |
| Gate/report | `evaluated-clean` | Manifest and parser-validated report pair retained. |

### Readiness evidence by clip set

| Role or runtime set | File-ready / clip-ready | Set-ready / rig-use | Runtime / acceptance boundary |
|---|---|---|---|
| Stationary idles | Files readable; 7 delivered loops; 2 strict seam failures | Standard skeleton; no additive/mask contract | Unity imports; inspect true wraps visually |
| Posture chains | Eleven one-shots mechanically readable | Exact state graph inferred; list labels disagree | Unity samples pass; endpoints/cancellation unaccepted |
| Lighting/log/skewer actions | Seven long actions readable | Campfire/skewer supplied; five implied props absent | Events, offsets, contacts, and fire timing unaccepted |

### Validation-profile status

| Validation profile | Selection | Result / next evidence |
|---|---|---|
| Marketplace intake | `selected` — `vendor-intended` | Archive, current listing, and EULA reviewed; revision/receipt absent. |
| Blended locomotion | `not-selected` | No locomotion set. |
| Root-motion controller | `not-selected` | No RM-labelled motions. |
| State-machine transitions | `selected` — `observed-pack-capability` | Eleven transition files; visual endpoints open. |
| Layered upper body/weapons | `not-selected` | Full-body posture/contact baseline is safer. |
| Traversal/environment | `not-selected` | No traversal set. |
| Contact actions/interactions | `selected` — `observed-pack-capability` | Seven actions and two props; contacts/events open. |
| Retargeted/customizable characters | `selected` — `evaluator-selected-generic-scenario` | Unity source actor works; target rig absent. |
| Motion matching/search | `not-selected` | No database contract. |
| Networked movement | `not-selected` | No authority/rollback contract. |
| Runtime performance | `selected` — `evaluator-selected-generic-scenario` | Constant-track pruning sampled; runtime profiling absent. |

## Pack inventory and content evidence

The logical delivery has 114 regular files: 29 FBXs, 63 metadata files, 19 textures/materials, one prefab, and one animation list. The individual motions reconcile to the vendor's current 25-animation count. The combined list also has 25 entries but renames files, including `StandToIdleKneelCampfire` versus delivered `StandToKneelCampfire` and `EatSkewerCampfire` versus `KneelEatSkewerCampfire`; file-scoped identifiers remain authoritative.

All 25 individual motions share skeleton signature `2b6fe49d5ae6` with 56 bones, identical to the standard Basic/Sword/Climbing/Injured family. The combined take and actor use the shared 58-bone actor structure. The props are separate models and are not motion-set members.

## Mechanical baseline

| Finding/check | Affected scope | Potential impact | Evidence |
|---|---|---|---|
| Default file safety | 29/29 FBXs | No NaN, time-order, quaternion, duration, scale, or bind-pose error | `observed-animsmith`; all baseline commands exit 0 |
| Constant tracks | 3,664 notes in 27 animation-bearing files; 3,394 in 25 individuals | Export bloat and retarget evaluation cost | Baseline/contract summaries |
| Declared loop closure | 2 errors in 1/14 loop-declared files | Pose displacement at wrap if kept cyclic | Contract summary |
| Declared loop rotation seam | 8/14 | Once-per-cycle angular pulse | Contract summary |
| Declared loop velocity seam | 6/14 | Once-per-cycle velocity pulse | Contract summary |
| Loop semantics | 7 one-shot-like actions/transitions marked loop | Repeated interaction or snap/restart | Filename/metadata reconciliation |
| Loop-seam ratio availability (current) | 27 clips: 0 measured, 26 not_applicable, 1 unavailable | Stationary clips with no real stride are explicitly `not_applicable`, not pass or fail | `observed-animsmith`; current baseline `loop_seam_ratio` and `loop_seam_ratio_availability` |
| Gait/phase availability (current) | 26/27 clips report `gait.phase_availability: measured`; contract `gait-group` is `not_applicable` on all 25 individual contracts | No in-place cyclic ring exists, so no gait anchoring ran; a correct not-applicable, not a refusal or a failure, and it does not certify prop/contact quality | `observed-animsmith`; current baseline `gait_phase_availability` and contract `gait_group_applicability` |
| Root trajectory (current) | 27/27 clips measured | 0 clips move more than 1 cm horizontally, 26 report stationary, 0 exceed 1° of yaw travel — positive measured confirmation of no root motion; sampled regression facts from the shared metric grid, not continuous-curve or engine-extraction proof | `observed-animsmith`; current baseline `root_trajectory` (measurements v16); |
| Per-bone channel coverage (current) | `bone_channels` available on measured clips | Confirms canonical per-bone translation/rotation/scale track presence; narrows a composition/prop-mask risk discussion but does not by itself prove a visually acceptable engine mask | `observed-animsmith`; current measurements v16; |

## AnimSmith remediation evidence

| Source issue | Operation/declarations | Result | Independent verification | Remaining caveat |
|---|---|---|---|---|
| Constant tracks in IdleKneel | `transform --prune-constant-tracks` with its declared contract | Exit 0; FBX 815,008 bytes to GLB 53,628 bytes | Output inspect/measure exit 0; fix dry-run exit 0; diff detects intentional change | Lint still reports the original rotation seam; runtime equivalence not proven, so output not adopted. Bounded by open issue #401. |
| Loop/action semantics | Contract `loop=true` only where Unity metadata says so | Eight files fail and seventeen pass | JSON and Markdown agree for all 25 | Detection does not decide whether metadata or motion is wrong. |

## Engine procedures and evidence

| Runtime | Version | Procedure | Observed result | Remaining gate |
|---|---|---|---|---|
| Unity Humanoid projection | Revision 1 / Unity 6000.3 | Current `generate import-advice` projection | Available; bounded settings projection only | Verify importer settings on the target project and character |
| Unreal Engine | 5.8 | Official documentation review; run `generate import-advice` under the `unreal` / revision 2 / `5.8` / `fbx-importer` profile on every individual clip (2026-08-21). | Current revision-2 settings projection is available; no engine process ran. | FBX import/retarget, state machine, events, contacts, build. |
| Godot | 4.7 | Official AnimationTree documentation review; run `generate import-advice` under the `godot` / revision 2 / `4.7` / `resource-importer-scene` profile on every individual clip (2026-08-21). | Current revision-2 settings projection is available; no engine process ran. | Conversion/import, retarget, graph, contacts, export. |
| Bevy | 0.19.0 | Official example review; checked for a `generate addressability` candidate (2026-08-21). | Not evaluated: no generated glTF/GLB candidate exists for this stationary pack because no gait-anchored in-place ring exists to seed one, so there was nothing to inventory. This is a coverage gap, not an observed Bevy failure. glTF-centric route remains project work. A collection-wide headless Unity glTFast import of 134 GLB candidates ran 2026-08-21 (see GLB candidate import below), but none of those candidates came from this pack; Bevy addressability stays not-evaluated for Campfire. | Conversion, mapping, graph, contacts, profiling. |

### Unity headless candidate probe (2026-08-21 correction)

A direct Unity 6000.5.8f1 headless sample of 120 clips observed baked root rotation, baked XZ for nearly every in-place clip, and extracted XZ for most root-motion clips. This is sampled importer evidence, not visual or gameplay acceptance.

| Variant | Clips | `lockRootRotation` true | `lockRootHeightY` true | `lockRootPositionXZ` true |
|---|---:|---:|---:|---:|
| In-place (non-`_RM`) | 84 | 84 | 84 | 83 |
| Root-motion (`_RM`) | 36 | 36 | 28 | 5 |

Aggregate across the sample: 120/120 clip definitions inspected, 120/120 `lockRootRotation` true, 112/120 `lockRootHeightY` true, 88/120 `lockRootPositionXZ` true. The observed policy is per-variant and axis-specific: rotation is baked throughout, while XZ is baked for nearly every in-place clip and extracted for most root-motion clips. This does not certify prop or contact quality.

### GLB candidate import into Unity (2026-08-21) — collection-level context, not a pack result

All 134/134 AnimSmith 0.7.0 gait-anchored GLB candidates across the eight-pack collection were staged into a separate, **new** Unity 6000.5.8f1 project using `com.unity.cloud.gltfast` 6.9.0, because Unity has no native GLB importer; the retained five-pack project above was not modified. Result: 134/134 files staged produced assets, 134/134 produced exactly one Unity `AnimationClip`, and every clip is non-legacy and non-empty. **Campfire has no in-place gait ring (see Pipeline-stage coverage above) and therefore contributed none of the 134 candidates** — this result is reported here only as collection-level context, not as a Campfire pack result, and it does not change Bevy addressability, which stays not-evaluated for this pack.

**Limit, stated plainly, for the candidates that do exist:** glTFast imports glTF animation as a **Generic** clip and does not reconstruct a Humanoid Avatar. The 134/134 result proves those candidates load and yield one well-formed clip in Unity; it does **not** test the Humanoid retarget path the source packs actually use, and it is not a visual or gameplay acceptance test.

## Rig, masking, and compatibility evidence

Current measurements v16 include canonical per-bone `bone_channels`. This narrows which joints a hypothetical mask could omit, but channel presence alone does not prove a visually acceptable engine mask; the composition and contact gates below remain open.

| Pack/rig/set pair | Skeleton/retarget | Scale/axes | Root policy | Timing/blend | Overall evidence |
|---|---|---|---|---|---|
| Campfire standard motions to supplied actor | 25 files share 56-bone signature; Unity Humanoid import succeeds | Metadata scale 1; props instantiate plausibly | Stationary full-body | Four samples execute | Direct Unity candidate; target deformation/contact untested |
| Campfire to Basic Locomotion | Same standard signature; 25 shared paths byte-identical | Shared actor/material assets identical | Basic owns approach; Campfire owns posture | One headless mixer executes | Strong co-install candidate; transition style/feet unaccepted |
| Campfire skewer action | Shared right-hand bone exists | Local-identity attachment ratio 0.487 | Stationary | Graph not contact-tested | Attachment works mechanically; grip/orientation/contact open |
| Campfire to other evaluated packs | Standard 56-bone signature shared | Every pair has 25 identical overlaps and zero conflicts | Full-body state handoff default | Five-pack Unity import succeeds | Technical co-existence, not artistic compatibility |

## Limitations and unknowns

1. No dynamic visual review, target-character retarget, contact/IK/event authoring, compression comparison, cancellation test, or player build was completed.
2. Screenshots of three AnimSmith offline reports confirmed rendered skeleton/metric/finding views, not motion quality.
3. Unreal Engine, Godot, and Bevy remain documentation-only.
4. Current vendor pages and EULA do not prove the local archive revision or transaction entitlement.
5. Commercial files, derived motion outputs, screenshots, and the generated Unity project remain outside the repository and CI.
6. The current availability recount for `loop_seam_ratio`, `gait.phase_availability`, and `gait-group` clarifies which stationary-pack facts are `not_applicable` versus `unavailable`; it is not a cleaner pass on prop/contact acceptance, which remains unevaluated.
7. Measured `root_trajectory` is a sampled regression fact from the shared uniform metric grid, not continuous-curve or engine-extraction proof; it does not by itself decide movement-ownership axes for a game controller.
8. A direct Unity 6000.5.8f1 headless sample of 120 clips observed baked root rotation, baked XZ for nearly every in-place clip, and extracted XZ for most root-motion clips. This is sampled importer evidence, not visual or gameplay acceptance.
9. The 134/134 collection candidate Unity import is context only: Campfire contributed no gait candidates. It proves nothing about this pack and does not establish Humanoid retarget.

## Changes between AnimSmith versions

| Evaluator | Change from the preceding evaluated state |
|---|---|
| AnimSmith 0.7.0 | Revalidated the 29-FBX baseline, 25 declared contracts, pruning trial, and current engine projections under output v17 / measurements v16; no generated gait candidate exists for this stationary pack. |
| AnimSmith 0.4.1 | Reproduced the evaluated 0.4.0 results for this corpus; unrelated release fixes did not change the pack conclusion. |
| AnimSmith 0.4.0 | Added root-trajectory and channel-coverage facts and current-at-the-time profile evidence without changing the baseline findings. |
| AnimSmith 0.3.0 | Established the initial mechanical, contract, and dated Unity evidence. Those evaluator results are superseded. |

## Reproduction

### Current AnimSmith reproduction (2026-08-26)

The same source corpus was re-inventoried and rerun with `animsmith 0.7.0`, exact tag `v0.7.0`, commit `461ac8a4f6bb368eb8637471a796f13eeb647140`, binary SHA-256 `01a501999c91d93abfb32b1f48241fccc70914fac27c9a650c31df44262578d8`, output schema v17, and measurements schema v16. The current results below are attributable to this evaluator; dated engine observations remain labelled with their capture dates.

| Current external evidence | SHA-256 | Result |
|---|---|---|
| Source inventory | `2f7e61f5c5d667272a2a67d756ccd96b9c9e1dc60b73e38613f83a9396a29ba4` | 29 FBXs; source unchanged |
| Exhaustive baseline command envelope | `7d46f1744efbb0a521059912bab11bc75a464ce48b489ab8c07b8854724fd908` | 29/29 complete |
| Declared-contract command envelope | `1f0e00639590daba19c8638045bfc5445d03ccea71acba240b2584bd145bfe3c` | 25 files; 17 pass / 8 fail |
| Remediation command envelope | `c77c17c296b1536a2fa39c75f63c86dd0233b75b3c3f4e9c5dbef3bc41ec2199` | Pruning candidate completed and verified |
| 0.7 supplemental projections | `d696122f26d75c94767dd4713f4aac8dfd54227faac529f3f390874f112adffa` | Addressability V1 + rich V2; Unity v1, Unreal v2, Godot v2 advice available |

The pruning candidate has source-addressability coverage for Bevy. It does not make the stationary/contact pack Bevy-ready: runtime target survival, graph wiring, props, contacts, and visual acceptance remain untested.

## Sources

- Local authorized source archive, extracted Unity metadata, and bundled animation list — private evidence identified above, accessed 2026-08-17.
- Protofactor, [Animset: Campfire](https://protofactor.biz/product/animset-campfire/), [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/), and [EULA](https://protofactor.biz/end-user-license-agreement/) — current listing/license context, accessed 2026-08-17.
- Unity Asset Store, [Ultimate Animation Collection](https://marketplace.unity.com/packages/3d/animations/ultimate-animation-collection-195459) — current collection version 1.65, release date 2026-08-16, Single Entity listing, and original Unity 6000.5.1; not local constituent identity.
- Unity, [Animation Blend Trees](https://docs.unity3d.com/6000.5/Documentation/Manual/class-BlendTree.html), [Root Motion](https://docs.unity3d.com/6000.5/Documentation/Manual/RootMotion.html), and [looping clips](https://docs.unity3d.com/6000.5/Documentation/Manual/LoopingAnimationClips.html); Epic Games, [animation system](https://dev.epicgames.com/documentation/unreal-engine/skeletal-mesh-animation-system-in-unreal-engine?application_version=5.8); Godot, [AnimationTree](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html); Bevy, [Animation Masks](https://bevy.org/examples/animation/animation-masks/) — capability context only, accessed 2026-08-17.
- AnimSmith, [Unity 6000.3 animation profile](../engine-profile-unity.md), [Unreal Engine 5.8 animation profile](../engine-profile-unreal.md), [Godot 4.7 animation profile](../engine-profile-godot.md), and [Bevy 0.19.0 animation profile](../engine-profile-bevy.md) — modeled import-advice profile facts for the 2026-08-21 refresh, accessed 2026-08-21.
