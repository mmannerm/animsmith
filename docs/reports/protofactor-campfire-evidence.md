# Animation pack evidence appendix: Protofactor Campfire

> Companion report: [Protofactor Campfire](protofactor-campfire.md)
>
> Evidence status: **partial** — exhaustive file/AnimSmith coverage and Unity 6000.5.8f1 headless evidence; visual contact, target-character, and three-engine passes are absent.
>
> Evaluation date: **2026-08-17**
>
> Report format: **1**

This appendix uses the [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder) without redefining it.

## Evaluation scope and provenance

| Field | Value |
|---|---|
| Pack/edition | Protofactor Campfire constituent from a locally held Ultimate Animation Collection archive; local revision unknown |
| Vendor/source | Protofactor [Campfire product](https://protofactor.biz/product/animset-campfire/) and [collection](https://protofactor.biz/product/ultimate-animation-collection/) pages |
| Delivered scope | RAR to one Unitypackage to 114 logical files; 29 FBXs: 25 individual motions, one combined take, one actor, campfire prop, skewer prop |
| Target use | Game-engine camp/rest state machine, contextual actions, props, and combination with evaluated collection packs |
| Target engines | Unity 6000.5.8f1 observed; Unreal Engine 5.7, Godot stable, and Bevy documentation-only |
| Target rigs/packs | Supplied Protof-Actor; Basic Locomotion, Sword & Shield, Climbing, and Injured selective compatibility |
| Source manifest | `campfire/source-archive-inventory.json`; RAR SHA-256 `bed86be7f91fdd46b376fce4b1a00c88372a3f703e0fe9077925712d8af8e8e9` |
| Evaluation manifest | `campfire/evidence/evaluation-manifest.json`; SHA-256 `11e67cd944ad2058d130eea06f557b41b1ba36e0ed14bbc3289d704d99bf962e`; schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1`; taxonomy/profile-set version 1 |
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
| Root motion | `not-applicable` | No root-motion-labelled constituent motion files. |
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

## AnimSmith remediation evidence

| Source issue | Operation/declarations | Result | Independent verification | Remaining caveat |
|---|---|---|---|---|
| Constant tracks in IdleKneel | `transform --prune-constant-tracks` with its declared contract | Exit 0; FBX 815,008 bytes to GLB 53,628 bytes | Output inspect/measure exit 0; fix dry-run exit 0; diff detects intentional change | Lint still reports the original rotation seam; runtime equivalence not proven, so output not adopted. |
| Loop/action semantics | Contract `loop=true` only where Unity metadata says so | Eight files fail and seventeen pass | JSON and Markdown agree for all 25 | Detection does not decide whether metadata or motion is wrong. |

## Engine procedures and evidence

| Runtime | Version | Procedure | Observed result | Remaining gate |
|---|---|---|---|---|
| Unity | 6000.5.8f1 | Merge five authorized Unitypackage reconstructions into a disposable project; inventory importers/clips; sample four Campfire clips on shared actor; mix Basic walk to StandToKneel; attach skewer; instantiate campfire. | 25/25 individual Humanoid clips import; 4/4 samples, mixer, and both prop checks pass. Skewer/actor height ratio 0.487; world campfire height 0.286 Unity units. | Visual offsets, contacts, loops, controller, compression, target rig, build. |
| Unreal Engine | 5.7 | Official documentation review only. | Not evaluated; runtime capabilities do not prove pack import. | FBX import/retarget, state machine, events, contacts, build. |
| Godot | stable | Official AnimationTree documentation review only. | Not evaluated. | Conversion/import, retarget, graph, contacts, export. |
| Bevy | unspecified | Official example review only. | Not evaluated; glTF-centric route remains project work. | Conversion, mapping, graph, contacts, profiling. |

## Rig, masking, and compatibility evidence

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

## Reproduction

Source RAR: 150,047,944 bytes, SHA-256 `bed86be7f91fdd46b376fce4b1a00c88372a3f703e0fe9077925712d8af8e8e9`. Extracted Unitypackage: 150,181,063 bytes, SHA-256 `9cfd965420a31f0702f7e2d8f886037011c29b33efe8b1da757dfa7750cc4c7a`.

Evaluator: `animsmith 0.3.0 (v0.3.0-30-gaabac28)`; revision `aabac28edf2719db236068339f1208bbf156d0bb`; binary SHA-256 `2fb43d210b5448fb2cd642946cc46df0cbb34595a48821b22a28daf7c1938f77`.

Run `inspect`, `measure --format json`, `lint --format json`, and `lint --format markdown` on every FBX with the humanoid baseline. For each individual file, apply the retained rig profile and Unity-derived loop declaration; declare in-place only for an actual paired non-RM member. Generate three risk-selected offline reports and inspect rendered screenshots. Run the pruning trial, then inspect, measure, lint, diff, and fix dry-run the candidate. Finally import all five evaluated packs into Unity and execute the retained headless probe.

Portable evidence digests: baseline `f9797cfd04dddac8b366a474dceac08dd968a95c52874398c014c81a1b2f9992`; contract `b9a858bcfce12ef799b06a91242054b8d0aa4a6f257660a41f0393bf20d1e7d2`; catalog `480ded14c195158d8768512e764c442cf14cf1fd04584bd27dfe24fd857ca1b9`; remediation `0e72dade266ad288c5ce2db068370d7563f0437def4177448783ea5bc9644b2e`; Basic comparison `0f6b0f6588822b1d309a6162de615c3de174bce92f6bfa6edc222a4467795903`; combined Unity probe `d2b6d1b0af14c2c77dca3c2cc4aa892d6e507f3cf8b9bb50bfdb4ef78d407afa`.

## Sources

- Local authorized source archive, extracted Unity metadata, and bundled animation list — private evidence identified above, accessed 2026-08-17.
- Protofactor, [Animset: Campfire](https://protofactor.biz/product/animset-campfire/), [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/), and [EULA](https://protofactor.biz/end-user-license-agreement/) — current listing/license context, accessed 2026-08-17.
- Unity Asset Store, [Ultimate Animation Collection](https://marketplace.unity.com/packages/3d/animations/ultimate-animation-collection-195459) — current collection version 1.65, release date 2026-08-16, Single Entity listing, and original Unity 6000.5.1; not local constituent identity.
- Unity, [Animation Blend Trees](https://docs.unity3d.com/6000.5/Documentation/Manual/class-BlendTree.html), [Root Motion](https://docs.unity3d.com/6000.5/Documentation/Manual/RootMotion.html), and [looping clips](https://docs.unity3d.com/6000.5/Documentation/Manual/LoopingAnimationClips.html); Epic Games, [animation system](https://dev.epicgames.com/documentation/unreal-engine/skeletal-mesh-animation-system-in-unreal-engine?application_version=5.7); Godot, [AnimationTree](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html); Bevy, [Animation Masks](https://bevy.org/examples/animation/animation-masks/) — capability context only, accessed 2026-08-17.
