# Animation pack evidence appendix: Protofactor Climbing

> Companion report: [Protofactor Climbing](protofactor-climbing.md)
>
> Evidence status: **partial** — exhaustive file/AnimSmith coverage and Unity 6000.5.8f1 headless evidence; visual traversal, vertical/yaw displacement, target-character, and three-engine passes are absent.
>
> Evaluation date: **2026-08-17**
>
> Report format: **1**

This appendix uses the [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder) without redefining it.

## Evaluation scope and provenance

| Field | Value |
|---|---|
| Pack/edition | Protofactor Climbing constituent from a locally held Ultimate Animation Collection archive; local revision unknown |
| Vendor/source | Protofactor [Climbing product](https://protofactor.biz/product/animset-climbing/) and [collection](https://protofactor.biz/product/ultimate-animation-collection/) pages |
| Delivered scope | RAR to one Unitypackage to 179 logical files; 77 FBXs: 75 individual motions, one combined take, one actor |
| Target use | Game-engine wall, ladder, obstacle, airborne, and landing traversal combined with evaluated collection packs |
| Target engines | Unity 6000.5.8f1 observed; Unreal Engine 5.7, Godot stable, and Bevy documentation-only |
| Target rigs/packs | Supplied Protof-Actor; Basic Locomotion, Sword & Shield, Campfire, and Injured selective compatibility |
| Source manifest | `climbing/source-archive-inventory.json`; RAR SHA-256 `4b353c3ded36889ab29096b7d0c04e54859f6dc380fa41e5ebeb925b74241101` |
| Evaluation manifest | `climbing/evidence/evaluation-manifest.json`; SHA-256 `b3807b89f30fb4656446d1e21f41d7405a414025356dd250d9c4a6d212ef3c2f`; schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1`; taxonomy/profile-set version 1 |
| Acquisition/license provenance | User states the local archive was downloaded from Protofactor; current [vendor EULA](https://protofactor.biz/end-user-license-agreement/) reviewed 2026-08-17. No receipt or local revision record was evaluated; no legal opinion. |

### Evidence coverage

| Surface | Offered/delivered | Evaluated | Findings | Not evaluated and why |
|---|---:|---:|---:|---|
| Animation files | 75 individual plus 1 combined | 76 | 41 individual contract failures | Dynamic visual quality and combined-take segmentation |
| Rigs/export variants | 3 observed structures | 3 | Standard 56; outlier/combined/actor 58 | Target-character deformation |
| AnimSmith baseline | 77 FBXs | 77 | 9,011 constant-track notes | Default lint lacks traversal intent |
| Declared contracts | 75 individual files | 75 | 34 clean; 41 failing; 8,753 notes | Vertical/yaw displacement and contacts |
| Offline visual reports | 75 possible | 3 | Reports render skeleton, metrics, and findings | Dynamic visual acceptance |
| Engine import/playback | 75 individual motions | 74 clips imported; 5 sampled | Required samples pass; outlier has no clip | Controller, displacement, compression, build |
| Blend/mask/retarget | 1 cross-pack mixer | 1 mixer | Execution passes | Visual blend and target rig |

### Claim legend

`observed-file` means derived from delivered files/metadata; `observed-animsmith` means reproduced with the named evaluator; `observed-engine` means the headless Unity probe; `inferred` marks semantic grouping. None means gameplay acceptance.

## Evaluation manifest and taxonomy

The retained evaluation manifest uses schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1` and taxonomy/profile-set version 1.

### Canonical clip-role inventory

| Canonical primary role | Logical motions | Delivered files | Evidence boundary |
|---|---:|---:|---|
| `idle-pose` | 7 | 7 | Wall/ladder/preparation holds; stationary by files |
| `continuous-locomotion` | 0 | 0 | Absent |
| `locomotion-transition` | 4 | 4 | Prepare/return transitions |
| `airborne` | 6 | 6 | Fall, apex, and three landings; one outlier |
| `traversal` | 30 | 58 | 28 IP/RM pairs plus two unpaired motions |
| `action-interaction` | 0 | 0 | Absent |
| `reaction-death` | 0 | 0 | Absent |
| `emote-cinematic` | 0 | 0 | Absent |
| `other-unknown` | 0 | 0 | None |
| **Total** | **47** | **75** | Validated manifest SHA-256 `b3807b89f30fb4656446d1e21f41d7405a414025356dd250d9c4a6d212ef3c2f` |

### Runtime-set inventory

| Runtime set | Type | Members/variants | Grouping evidence | Validation status |
|---|---|---|---|---|
| Wall-climb 8-way | directional-blend | 8 IP plus 8 RM | Common 1.333 s duration and direction names; high confidence | All 16 strict-loop failures; visual/displacement-open |
| Ladder up/down | directional-blend | 2 IP plus 2 RM | Common 1.200 s duration and paired labels; high confidence | All 4 strict-loop failures; vertical displacement-open |
| Wall-jump 4-way | directional-blend | 4 IP plus 4 RM | Direction family; high confidence | Discrete-action candidate; all 8 delivered loops fail |
| Obstacle alternatives | other | 4 IP plus 4 RM | Height/side choices; high confidence | Discrete actions; all 8 delivered loops fail |
| Fall-and-land chain | transition-chain | 5 logical motions | Apex/fall/landing labels; medium confidence | Outlier excluded; visual transition-open |
| Opposite-wall jump chain | transition-chain | 6 logical motions, 2 RM companions | Mirrored prepare/hold/jump labels; medium confidence | Contact/facing/displacement-open |

### Pipeline-stage coverage

| Stage | Coverage state | Evidence / remaining gate |
|---|---|---|
| Acquire | `evaluated-clean` | Local vendor archive identified and hashed; transaction record absent. |
| Preserve raw | `evaluated-clean` | RAR and Unitypackage retained unchanged outside the repository. |
| Inspect | `evaluated-finding` | Every FBX inspected/measured/linted; loop, rig, and optimization findings retained. |
| Segment | `partially-evaluated` | Individual files used; combined take not promoted. |
| Root motion | `evaluated-finding` | 28 named IP/RM pairs found; horizontal-only measurement is insufficient for traversal. |
| Conform | `partially-evaluated` | Standard family and Unity shared Avatar work; one outlier and target rigs open. |
| Validate | `partially-evaluated` | Mechanical contracts and headless Unity complete; environment/visual gates open. |
| Optimize | `evaluated-finding` | One pruning candidate verified mechanically but not accepted semantically. |
| Export | `partially-evaluated` | Sample GLB export reopens; Unity native delivery tested; other engines open. |
| Gate/report | `evaluated-clean` | Manifest and parser-validated report pair retained. |

### Readiness evidence by clip set

| Role or runtime set | File-ready / clip-ready | Set-ready / rig-use | Runtime / acceptance boundary |
|---|---|---|---|
| Wall/ladder cycles | Files readable; paired families/durations reconcile | Standard skeleton; every declared loop has strict seam findings | Unity samples execute; vertical root/contact quality open |
| Obstacles/wall jumps | IP/RM pairs readable | Use discrete actions; delivered loop semantics rejected | Environment height, facing, contacts, cancellation open |
| Fall/land | Five standard candidates readable | Outlier excluded; transitions inferred | Unity standard sample executes; physics/landing windows open |

### Validation-profile status

| Validation profile | Selection | Result / next evidence |
|---|---|---|
| Marketplace intake | `selected` — `vendor-intended` | Archive/listing/EULA reviewed; listing and local counts disagree. |
| Blended locomotion | `not-selected` | No ground-locomotion set. |
| Root-motion controller | `selected` — `vendor-intended` | 28 named RM pairs; vertical/yaw measurement and engine authority open. |
| State-machine transitions | `selected` — `observed-pack-capability` | Entry, exit, prepare, fall, and landing families; visual boundaries open. |
| Layered upper body/weapons | `not-selected` | Full-body contact baseline is safer. |
| Traversal/environment | `selected` — `vendor-intended` | Main purpose; environment matrix absent. |
| Contact actions/interactions | `selected` — `observed-pack-capability` | Wall/ladder/ledge contacts implied; events/IK absent. |
| Retargeted/customizable characters | `selected` — `evaluator-selected-generic-scenario` | Unity source actor works; target rig absent. |
| Motion matching/search | `not-selected` | No database/contact annotation contract. |
| Networked movement | `not-selected` | No authority/rollback contract. |
| Runtime performance | `selected` — `evaluator-selected-generic-scenario` | Constant-track pruning sampled; runtime profiling absent. |

## Pack inventory and content evidence

The logical delivery has 179 regular files and 77 FBXs: 75 individual motions, one combined take, and one actor. The current product page advertises 69 animations, 18 root-motion, and 51 in-place, while the local archive has 75 individual motion files, 28 `_RM` files, and 47 non-RM files. This report does not assume which revision is newer.

Seventy-four individual files share skeleton signature `2b6fe49d5ae6` with 56 bones. `Humanoid@FallingUnarmed.FBX` instead has signature `3da84463466a` and 58 bones. Unity exposes no AnimationClip for that file, whereas its standard-family `Humanoid@Falling.fbx` counterpart imports.

## Mechanical baseline

| Finding/check | Affected scope | Potential impact | Evidence |
|---|---|---|---|
| Default file safety | 77/77 FBXs | No NaN, time-order, quaternion, duration, scale, or bind-pose error | `observed-animsmith`; all baseline commands exit 0 |
| Constant tracks | 9,011 notes in 77 files; 8,753 in 75 individuals | Export bloat and retarget evaluation cost | Baseline/contract summaries |
| Declared loop closure | 26 errors across 22 files | Position discontinuity at wrap | Contract summary |
| Declared loop rotation seam | 41/43 loop-declared files | Angular pulse at wrap | Contract summary |
| Declared loop velocity seam | 39/43 | Velocity pulse at wrap | Contract summary |
| Semantic loop mismatch | 16 obstacle/wall-jump files are loop-declared | Repeated one-shot traversal | Filename/metadata reconciliation |

## AnimSmith remediation evidence

| Source issue | Operation/declarations | Result | Independent verification | Remaining caveat |
|---|---|---|---|---|
| Constant tracks in WallClimbUp | `transform --prune-constant-tracks` with its declared contract | Exit 0; FBX 563,984 bytes to GLB 71,288 bytes | Output inspect/measure and fix dry-run exit 0; diff detects change | Lint retains the source seam; equivalence unproven, so output not adopted. |
| Vertical/yaw displacement | Measure current RM clips | Horizontal speeds are available; up/down may report 0 | Paired inventory and reports retained | [Issue #408](https://github.com/mmannerm/animsmith/issues/408) is needed for complete displacement evidence. |

## Engine procedures and evidence

| Runtime | Version | Procedure | Observed result | Remaining gate |
|---|---|---|---|---|
| Unity | 6000.5.8f1 | Merge five authorized package reconstructions outside the repo; inventory importers/clips; sample five standard clips; assert the expected outlier; mix Basic walk to a climb state. | 74/75 individual clips import; 5/5 required samples and mixer pass; outlier exposes no clip. | Visual contacts/displacement, controller, target rig, compression, build. |
| Unreal Engine | 5.7 | Official root-motion/animation documentation review only. | Not evaluated. | FBX import/retarget, root lock, motion warping, contacts, build. |
| Godot | stable | Official AnimationTree documentation review only. | Not evaluated. | Conversion/import, root extraction, controller, contacts, export. |
| Bevy | unspecified | Official animation-mask example review only. | Not evaluated. | glTF conversion, mapping, root policy, contacts, profiling. |

## Rig, masking, and compatibility evidence

| Pack/rig/set pair | Skeleton/retarget | Scale/axes | Root policy | Timing/blend | Overall evidence |
|---|---|---|---|---|---|
| Climbing standard motions to supplied actor | 74 standard files share 56-bone signature; Unity Humanoid succeeds | Metadata scale 1 | Paired RM/IP choice required | Five samples execute | Direct Unity candidate; target/contact untested |
| Climbing outlier | Distinct 58-bone signature | Metadata scale 1 | Unknown/no exposed clip | Cannot sample | Exclude pending author clarification |
| Climbing to Basic Locomotion | Standard signatures align; 25 shared paths byte-identical | Shared actor/assets identical | Basic approach then traversal handoff | One headless mixer executes | Co-install candidate; entry pose/feet unaccepted |
| Climbing to Sword/Campfire/Injured | Standard signature shared | All pairwise overlaps identical, zero conflicts | Full-body handoff default | Five-pack Unity import succeeds | Technical co-existence, not artistic compatibility |

## Limitations and unknowns

1. No dynamic visual review, environment geometry matrix, target retarget, contact/IK/event authoring, compression comparison, cancellation test, network correction, or player build was completed.
2. AnimSmith 0.3.0 horizontal speed does not characterize vertical/yaw root displacement.
3. Screenshots of three offline reports prove rendering at frame zero, not motion quality.
4. Unreal Engine, Godot, and Bevy remain documentation-only.
5. Commercial files, derived motion outputs, screenshots, and the generated Unity project remain outside the repository and CI.

## Reproduction

Source RAR: 142,764,600 bytes, SHA-256 `4b353c3ded36889ab29096b7d0c04e54859f6dc380fa41e5ebeb925b74241101`. Extracted Unitypackage: 142,850,966 bytes, SHA-256 `4ca22fe57d8b322e91cf73a043880fc156dd2a71c1bf9f0b58d42b433731d2a1`.

Evaluator: `animsmith 0.3.0 (v0.3.0-30-gaabac28)`; revision `aabac28edf2719db236068339f1208bbf156d0bb`; binary SHA-256 `2fb43d210b5448fb2cd642946cc46df0cbb34595a48821b22a28daf7c1938f77`.

Run baseline commands for every FBX. For each individual file, apply the humanoid rig, Unity-derived loop declaration, and in-place only to the non-RM side of a real pair; avoid a generic horizontal threshold for vertical traversal. Generate and inspect three risk-selected offline reports. Prune one sample, then inspect, measure, lint, diff, and fix dry-run it. Import all five evaluated packs into Unity and run the retained headless probe.

Portable evidence digests: baseline `0325119190ccddbe272c74b94808853438488d21dde53b3ce3e56c1d3461800c`; contract `5a95252f8046ca1471022327140331c6233cb376c89de6aa91805b3427de6d6e`; catalog `3c752cc75f73ea6589916e62a35b52aa2f6a004e7af3c39c84970ef6a4744419`; remediation `c1a34efc74d287a5b3334e9ec708b0498286342f42761656bb11049dab3737d`; Basic comparison `96f1ffe158139600495281b57e6a9c37d61720bc9029feb23590681a8e163e5d`; combined Unity probe `d2b6d1b0af14c2c77dca3c2cc4aa892d6e507f3cf8b9bb50bfdb4ef78d407afa`.

## Sources

- Local authorized source archive, extracted Unity metadata, and bundled animation list — private evidence identified above, accessed 2026-08-17.
- Protofactor, [Animset: Climbing](https://protofactor.biz/product/animset-climbing/), [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/), and [EULA](https://protofactor.biz/end-user-license-agreement/) — current context, accessed 2026-08-17.
- Unity Asset Store, [Ultimate Animation Collection](https://marketplace.unity.com/packages/3d/animations/ultimate-animation-collection-195459) — current collection version 1.65, release date 2026-08-16, Single Entity listing, and original Unity 6000.5.1; not local constituent identity.
- Unity, [Root Motion](https://docs.unity3d.com/6000.5/Documentation/Manual/RootMotion.html) and [looping clips](https://docs.unity3d.com/6000.5/Documentation/Manual/LoopingAnimationClips.html); Epic Games, [root motion](https://dev.epicgames.com/documentation/unreal-engine/root-motion-in-unreal-engine?application_version=5.7); Godot, [AnimationTree](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html); Bevy, [Animation Masks](https://bevy.org/examples/animation/animation-masks/) — capability context only, accessed 2026-08-17.
