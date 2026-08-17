# Animation pack evidence appendix: Protofactor Injured

> Companion report: [Protofactor Injured](protofactor-injured.md)
>
> Evidence status: **partial** — exhaustive file/AnimSmith coverage and Unity 6000.5.8f1 headless evidence; visual loops, blends, masks, target-character, and three-engine passes are absent.
>
> Evaluation date: **2026-08-17**
>
> Report format: **1**

This appendix uses the [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder) without redefining it.

## Evaluation scope and provenance

| Field | Value |
|---|---|
| Pack/edition | Protofactor Injured constituent from a locally held Ultimate Animation Collection archive; local revision unknown |
| Vendor/source | Protofactor [Injured product](https://protofactor.biz/product/animset-injured/) and [collection](https://protofactor.biz/product/ultimate-animation-collection/) pages |
| Delivered scope | RAR to one Unitypackage to 171 logical files; 72 FBXs: 70 individual motions, one combined take, one actor |
| Target use | Game-engine injured locomotion/posture states combined with Basic Locomotion, Sword & Shield, and contextual packs |
| Target engines | Unity 6000.5.8f1 observed; Unreal Engine 5.7, Godot stable, and Bevy documentation-only |
| Target rigs/packs | Supplied Protof-Actor; Basic Locomotion, Sword & Shield, Campfire, and Climbing selective compatibility |
| Source manifest | `injured/source-archive-inventory.json`; RAR SHA-256 `b459ab3a39a15aa2e499c633f661616449bfc281836858ad9525014184aa9878` |
| Evaluation manifest | `injured/evidence/evaluation-manifest.json`; SHA-256 `ad98ac7639c997a6d7a3eabb7552b2bbb06ab1c797013cf84cb86e764a3159f5`; schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1`; taxonomy/profile-set version 1 |
| Acquisition/license provenance | User states the local archive was downloaded from Protofactor; current [vendor EULA](https://protofactor.biz/end-user-license-agreement/) reviewed 2026-08-17. No receipt or local revision record was evaluated; no legal opinion. |

### Evidence coverage

| Surface | Offered/delivered | Evaluated | Findings | Not evaluated and why |
|---|---:|---:|---:|---|
| Animation files | 70 individual plus 1 combined | 71 | 42 individual contract failures | Dynamic visual quality and combined segmentation |
| Rigs/export variants | 2 observed structures | 2 | Standard 56; combined/actor 58 | Target-character deformation |
| AnimSmith baseline | 72 FBXs | 72 | 9,915 constant-track notes | Default lint lacks gait intent |
| Declared contracts | 70 individual files | 70 | 28 clean; 42 failing; 9,644 notes | Visual loop/blend/transition quality |
| Gait measurements | 14 IP/RM pairs | 14 | Matching pair phase/duration; style speeds/phase vary | Contact-side/stride semantics |
| Gait-anchor candidates | 14 IP gait files | 14 attempted | All safely refused; no output | Root forward-axis support tracked in issue #426 |
| Engine import/playback | 70 individual clips | 70 imported; 6 sampled | Required samples pass | Controller, compression, player build |
| Blend/mask/retarget | 2 mixers; 1 mask graph | 3 graph executions | Execution passes | Visual blend/mask and target rig |

### Claim legend

`observed-file` means derived from delivered files/metadata; `observed-animsmith` means reproduced with the named evaluator; `observed-engine` means the headless Unity probe; `inferred` marks semantic grouping. None means gameplay acceptance.

## Evaluation manifest and taxonomy

The retained evaluation manifest uses schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1` and taxonomy/profile-set version 1.

### Canonical clip-role inventory

| Canonical primary role | Logical motions | Delivered files | Evidence boundary |
|---|---:|---:|---|
| `idle-pose` | 21 | 21 | Seven standing, kneeling, and sitting styles |
| `continuous-locomotion` | 14 | 28 | Seven walk and seven run IP/RM pairs |
| `locomotion-transition` | 21 | 21 | Sit/kneel entry and sit return files |
| `airborne` | 0 | 0 | Absent |
| `traversal` | 0 | 0 | Absent |
| `action-interaction` | 0 | 0 | Absent |
| `reaction-death` | 0 | 0 | Absent |
| `emote-cinematic` | 0 | 0 | Absent |
| `other-unknown` | 0 | 0 | None |
| **Total** | **56** | **70** | Validated manifest SHA-256 `ad98ac7639c997a6d7a3eabb7552b2bbb06ab1c797013cf84cb86e764a3159f5` |

### Runtime-set inventory

| Runtime set | Type | Members/variants | Grouping evidence | Validation status |
|---|---|---|---|---|
| Style A locomotion | speed-blend | idle plus walk/run IP/RM; 5 files | Letter family and paired evidence; medium confidence | Threshold 0.540/2.021 m/s; loops visual-open |
| Style B locomotion | speed-blend | idle plus walk/run IP/RM; 5 files | Letter family and paired evidence; medium confidence | 0.541/2.060 m/s; Run IP loop inferred; visual-open |
| Style C locomotion | speed-blend | idle plus walk/run IP/RM; 5 files | Letter family and paired evidence; medium confidence | 0.494/1.748 m/s; blend phase 0.153 cycles |
| Style D locomotion | speed-blend | idle plus walk/run IP/RM; 5 files | Letter family and paired evidence; medium confidence | 0.504/1.821 m/s; blend phase 0.105 cycles |
| Style E locomotion | speed-blend | idle plus walk/run IP/RM; 5 files | Letter family and paired evidence; medium confidence | 0.733/2.005 m/s; blend phase 0.181 cycles |
| Style F locomotion | speed-blend | idle plus walk/run IP/RM; 5 files | Letter family and paired evidence; medium confidence | 0.692/2.005 m/s; blend phase 0.170 cycles |
| Style G locomotion | speed-blend | idle plus walk/run IP/RM; 5 files | Letter family and paired evidence; medium confidence | 0.519/2.127 m/s; blend phase 0.075 cycles |
| Style A posture chain | transition-chain | 6 single files | Standing/sit/kneel labels; high confidence | No kneel exit; transitions visual-open |
| Style B posture chain | transition-chain | 6 single files | Standing/sit/kneel labels; high confidence | No kneel exit; transitions visual-open |
| Style C posture chain | transition-chain | 6 single files | Standing/sit/kneel labels; high confidence | No kneel exit; transitions visual-open |
| Style D posture chain | transition-chain | 6 single files | Standing/sit/kneel labels; high confidence | No kneel exit; transitions visual-open |
| Style E posture chain | transition-chain | 6 single files | Standing/sit/kneel labels; high confidence | No kneel exit; transitions visual-open |
| Style F posture chain | transition-chain | 6 single files | Standing/sit/kneel labels; high confidence | No kneel exit; transitions visual-open |
| Style G posture chain | transition-chain | 6 single files | Standing/sit/kneel labels; high confidence | No kneel exit; transitions visual-open |

### Pipeline-stage coverage

| Stage | Coverage state | Evidence / remaining gate |
|---|---|---|
| Acquire | `evaluated-clean` | Local vendor archive identified and hashed; transaction record absent. |
| Preserve raw | `evaluated-clean` | RAR and Unitypackage retained unchanged outside the repository. |
| Inspect | `evaluated-finding` | Every FBX inspected/measured/linted; seam and optimization findings retained. |
| Segment | `partially-evaluated` | Individual files used; combined take not promoted. |
| Root motion | `evaluated-finding` | 14 IP/RM pairs measured; gait-anchor attempts safely refused on root orientation. |
| Conform | `partially-evaluated` | Standard skeleton and Unity shared Avatar work; target rigs open. |
| Validate | `partially-evaluated` | Mechanical contracts and headless Unity complete; visual gameplay open. |
| Optimize | `evaluated-finding` | One pruning candidate verified mechanically but not accepted semantically. |
| Export | `partially-evaluated` | Sample GLB export reopens; Unity native delivery tested; other engines open. |
| Gate/report | `evaluated-clean` | Manifest and parser-validated report pair retained. |

### Readiness evidence by clip set

| Role or runtime set | File-ready / clip-ready | Set-ready / rig-use | Runtime / acceptance boundary |
|---|---|---|---|
| Seven locomotion styles | 14 IP/RM pairs readable and duration/phase paired | Standard skeleton; thresholds differ; all explicit loops fail | Unity samples/mixers execute; gait blends and wraps open |
| Standing idles | Seven files readable | 5/7 strict seam failures | Mask graph executes; full-body/upper-body visual choice open |
| Sit/kneel chains | 35 posture/transition files readable | Style letters coherent; kneel exits absent | Unity samples execute; endpoints/recovery/cancellation open |

### Validation-profile status

| Validation profile | Selection | Result / next evidence |
|---|---|---|
| Marketplace intake | `selected` — `vendor-intended` | Archive/listing/EULA reviewed; listing and local RM counts disagree. |
| Blended locomotion | `selected` — `vendor-intended` | Seven speed sets measured; phase/threshold visual gates open. |
| Root-motion controller | `selected` — `observed-pack-capability` | 14 named pairs; authority and in-engine displacement open. |
| State-machine transitions | `selected` — `observed-pack-capability` | Sit/kneel families; kneel return and visual endpoints open. |
| Layered upper body/weapons | `selected` — `evaluator-selected-generic-scenario` | One mask and Sword handoff graph executes; visual pose open. |
| Traversal/environment | `not-selected` | No traversal set. |
| Contact actions/interactions | `not-selected` | No contact-action set. |
| Retargeted/customizable characters | `selected` — `evaluator-selected-generic-scenario` | Unity source actor works; target rig absent. |
| Motion matching/search | `not-selected` | No database/contact annotation contract. |
| Networked movement | `not-selected` | No authority/rollback contract. |
| Runtime performance | `selected` — `evaluator-selected-generic-scenario` | Constant-track pruning sampled; runtime profiling absent. |

## Pack inventory and content evidence

The delivery has 171 regular files and 72 FBXs: 70 individual motions, one combined take, and one actor. Individual files form seven A–G styles with 14 logical gaits delivered as IP/RM pairs, 21 idles, and 21 transitions. The current product page says 70 animations, zero root motion, and 70 in-place; the local archive has 70 individual files including 14 `_RM` files, and its bundled list also identifies RM entries.

All 70 individual motions share skeleton signature `2b6fe49d5ae6` with 56 bones. The combined take and actor use the collection's shared 58-bone structure.

## Mechanical baseline

| Finding/check | Affected scope | Potential impact | Evidence |
|---|---|---|---|
| Default file safety | 72/72 FBXs | No NaN, time-order, quaternion, duration, scale, or bind-pose error | `observed-animsmith`; all baseline commands exit 0 |
| Constant tracks | 9,915 notes in 72 files; 9,644 in 70 individuals | Export bloat and retarget evaluation cost | Baseline/contract summaries |
| Declared loop closure | 15/48 loop files | Position discontinuity at wrap | Contract summary |
| Declared loop rotation seam | 42/48 | Angular pulse at wrap | Contract summary |
| Declared loop velocity seam | 31/48 | Velocity pulse at wrap | Contract summary |
| Contract result | 42/70 files fail | Every explicit locomotion loop and 15 idles affected | Per-file JSON/Markdown agreement |

## AnimSmith remediation evidence

| Source issue | Operation/declarations | Result | Independent verification | Remaining caveat |
|---|---|---|---|---|
| Gait phase normalization | `transform --align-gait-anchor` on 14 IP walk/run files | All exit 2 with no output: root lacks finite horizontal forward axis | Output-absence and diagnostic retained | Safe refusal; [issue #426](https://github.com/mmannerm/animsmith/issues/426) tracks support. |
| Constant tracks in Walk A | `transform --prune-constant-tracks` with declared contract | Exit 0; FBX 658,816 bytes to GLB 45,152 bytes | Output inspect/measure and fix dry-run exit 0; diff detects change | Lint retains source seam; equivalence unproven, so output not adopted. |

## Engine procedures and evidence

| Runtime | Version | Procedure | Observed result | Remaining gate |
|---|---|---|---|---|
| Unity | 6000.5.8f1 | Merge five authorized package reconstructions outside repo; inventory; sample six Injured clips; mix Basic and Sword into Injured; build one Basic-lower/Injured-upper mask graph. | 70/70 individual Humanoid clips import; 6/6 samples, both mixers, and mask graph pass. | Visual loops/blends/mask, controller, target rig, compression, build. |
| Unreal Engine | 5.7 | Official animation/root-motion/sync documentation review only. | Not evaluated. | FBX import/retarget, blend spaces, markers, root authority, masks, build. |
| Godot | stable | Official AnimationTree documentation review only. | Not evaluated. | Conversion/import, retarget, phase policy, filters, export. |
| Bevy | unspecified | Official animation-mask example review only. | Not evaluated. | glTF conversion, mapping, phase policy, masks, profiling. |

## Rig, masking, and compatibility evidence

| Pack/rig/set pair | Skeleton/retarget | Scale/axes | Root policy | Timing/blend | Overall evidence |
|---|---|---|---|---|---|
| Injured standard motions to supplied actor | All 70 share 56-bone signature; Unity Humanoid succeeds | Metadata scale 1 | Paired RM/IP choice | Six samples execute | Direct Unity candidate; target deformation untested |
| Injured IP/RM pairs | Same skeleton and duration; phase nearly identical within pair | Same scale | Exclusive movement authority | Walk/run phase differs by style | Strong pairing evidence; loop/blend visual-open |
| Injured to Basic Locomotion | Same standard signature; 25 shared paths byte-identical | Shared assets identical | Basic or Injured movement owns lower body | Mixer and upper/lower mask graph execute | Co-install candidate; visual style/mask open |
| Injured to Sword/other packs | Standard signature shared | All pairwise overlaps identical, zero conflicts | Full-body handoff default | Sword mixer and five-pack import succeed | Technical co-existence, not artistic compatibility |

## Limitations and unknowns

1. No dynamic visual review, target retarget, loop/blend/mask acceptance, transition endpoints, compression comparison, network correction, or player build was completed.
2. Gait phase is a mechanical proxy; contact side, stride semantics, and gameplay feel remain unmeasured.
3. Screenshots of three offline reports prove rendered frame-zero diagnostics, not motion quality.
4. Unreal Engine, Godot, and Bevy remain documentation-only.
5. Commercial files, derived motion outputs, screenshots, and the generated Unity project remain outside the repository and CI.

## Reproduction

Source RAR: 157,809,050 bytes, SHA-256 `b459ab3a39a15aa2e499c633f661616449bfc281836858ad9525014184aa9878`. Extracted Unitypackage: 161,663,801 bytes, SHA-256 `3227c487fd1a2f1bc69e569171a3e5fae3f6a062dffb65f829248802046aaa09`.

Evaluator: `animsmith 0.3.0 (v0.3.0-30-gaabac28)`; revision `aabac28edf2719db236068339f1208bbf156d0bb`; binary SHA-256 `2fb43d210b5448fb2cd642946cc46df0cbb34595a48821b22a28daf7c1938f77`.

Run baseline commands on every FBX. For every individual file, apply the humanoid rig, Unity-derived loop declaration, and in-place only to the non-RM side of a true pair. Measure each gait, compute pair/family phase evidence, and attempt gait anchoring without promoting refused candidates. Generate and inspect three risk-selected offline reports. Prune one sample and run inspect/measure/lint/diff/fix dry-run. Import all five evaluated packs into Unity and execute the retained probe.

Portable evidence digests: baseline `7d3653df78ed84a4213a0c5e2b0d65a61cd7696704edb58d29ad96f398e82dc0`; contract `a59a242483aec788c6cc928096101072d12752128134fd3d54b7d062637d321e`; catalog `08acae7a14c6717877afd255c4eafaf4c147224b2f3ccbd4ead25429498a6d43`; remediation `9cb1afa636de97a57a8ef9f73361955356684c10bb17e0ea8744195768c08df9`; Basic comparison `576ecaac0918834ab840199e2d9b4e555c1c6130e9f0dde185376dc624e1a57`; combined Unity probe `d2b6d1b0af14c2c77dca3c2cc4aa892d6e507f3cf8b9bb50bfdb4ef78d407afa`.

## Sources

- Local authorized source archive, extracted Unity metadata, and bundled animation list — private evidence identified above, accessed 2026-08-17.
- Protofactor, [Animset: Injured](https://protofactor.biz/product/animset-injured/), [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/), and [EULA](https://protofactor.biz/end-user-license-agreement/) — current context, accessed 2026-08-17.
- Unity Asset Store, [Ultimate Animation Collection](https://marketplace.unity.com/packages/3d/animations/ultimate-animation-collection-195459) — current collection version 1.65, release date 2026-08-16, Single Entity listing, and original Unity 6000.5.1; not local constituent identity.
- Unity, [Animation Blend Trees](https://docs.unity3d.com/6000.5/Documentation/Manual/class-BlendTree.html), [Root Motion](https://docs.unity3d.com/6000.5/Documentation/Manual/RootMotion.html), and [looping clips](https://docs.unity3d.com/6000.5/Documentation/Manual/LoopingAnimationClips.html); Epic Games, [animation system](https://dev.epicgames.com/documentation/unreal-engine/skeletal-mesh-animation-system-in-unreal-engine?application_version=5.7); Godot, [AnimationTree](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html); Bevy, [Animation Masks](https://bevy.org/examples/animation/animation-masks/) — capability context only, accessed 2026-08-17.
