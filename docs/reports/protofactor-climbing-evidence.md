# Animation pack evidence appendix: Protofactor Climbing

> Companion report: [Protofactor Climbing](protofactor-climbing.md)
>
> Evidence status: **partial** — exhaustive file/AnimSmith 0.4.0 coverage, measured vertical/yaw root trajectory, retained Unity 6000.5.8f1 headless evidence, and new Unreal/Godot import-advice attempts; visual traversal, engine root-motion extraction, target-character, and full Unreal/Godot/Bevy passes are absent.
>
> Evaluation date: **2026-08-21**
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
| Target engines | Unity 6000.5.8f1 observed headless (retained 2026-08-17); Unity 6000.3, Unreal Engine 5.8, and Godot 4.7 AnimSmith import-advice probed (2026-08-21); Bevy 0.19.0 documentation-only, no generated glTF/GLB candidate |
| Target rigs/packs | Supplied Protof-Actor; Basic Locomotion, Sword & Shield, Campfire, and Injured selective compatibility |
| Source manifest | `climbing/source-archive-inventory.json`; RAR SHA-256 `4b353c3ded36889ab29096b7d0c04e54859f6dc380fa41e5ebeb925b74241101`; re-inventoried 2026-08-21 under AnimSmith 0.4.0, byte-identical to the published manifest (0 added, 0 removed, 0 changed) |
| Evaluation manifest | `climbing/evidence/evaluation-manifest.json`; SHA-256 `b3807b89f30fb4656446d1e21f41d7405a414025356dd250d9c4a6d212ef3c2f`; schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1`; taxonomy/profile-set version 1; unchanged by the 2026-08-21 refresh |
| Acquisition/license provenance | User states the local archive was downloaded from Protofactor; current [vendor EULA](https://protofactor.biz/end-user-license-agreement/) reviewed 2026-08-17. No receipt or local revision record was evaluated; no legal opinion. |

### Evidence coverage

| Surface | Offered/delivered | Evaluated | Findings | Not evaluated and why |
|---|---:|---:|---:|---|
| Animation files | 75 individual plus 1 combined | 76 | 41 individual contract failures | Dynamic visual quality and combined-take segmentation |
| Rigs/export variants | 3 observed structures | 3 | Standard 56; outlier/combined/actor 58 | Target-character deformation |
| AnimSmith baseline | 77 FBXs | 77 | 9,011 constant-track notes | Default lint lacks traversal intent |
| Declared contracts | 75 individual files | 75 | 34 clean; 41 failing; 8,753 notes | Contacts and engine root-motion extraction proof |
| Offline visual reports | 75 possible | 3 | Reports render skeleton, metrics, and findings | Dynamic visual acceptance |
| Engine import/playback | 75 individual motions | 74 clips imported; 5 sampled | Required samples pass; outlier has no clip | Controller, in-Editor displacement/yaw acceptance, compression, build |
| Blend/mask/retarget | 1 cross-pack mixer | 1 mixer | Execution passes | Visual blend and target rig |

### Claim legend

`observed-file` means derived from delivered files/metadata; `observed-animsmith` means reproduced with the named evaluator; `observed-engine` means the headless Unity probe; `inferred` marks semantic grouping. None of these labels means gameplay acceptance.

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
| Wall-climb 8-way | directional-blend | 8 IP plus 8 RM | Common 1.333 s duration and direction names; high confidence | All 16 strict-loop failures; vertical/yaw now measured per clip (`root_trajectory`); visual/engine-extraction-open |
| Ladder up/down | directional-blend | 2 IP plus 2 RM | Common 1.200 s duration and paired labels; high confidence | All 4 strict-loop failures; vertical/yaw now measured per clip; visual/engine-extraction-open |
| Wall-jump 4-way | directional-blend | 4 IP plus 4 RM | Direction family; high confidence | Discrete-action candidate; all 8 delivered loops fail; wall-jump family vertical range -1.850..+1.918 m (max abs 1.903 m, `Humanoid@WallJumpUp_RM.fbx`) |
| Obstacle alternatives | other | 4 IP plus 4 RM | Height/side choices; high confidence | Discrete actions; all 8 delivered loops fail; obstacle family vertical range -0.043..+2.000 m (`Humanoid@ClimbUp2MetersObstacleUnarmed_RM.FBX` +2.000 m) |
| Fall-and-land chain | transition-chain | 5 logical motions | Apex/fall/landing labels; medium confidence | Outlier excluded; fall/land family vertical range 0.000 m throughout, as expected; visual transition-open |
| Opposite-wall jump chain | transition-chain | 6 logical motions, 2 RM companions | Mirrored prepare/hold/jump labels; medium confidence | Contact/facing-open; vertical/yaw now measured per clip (wall family up to 180° net yaw on turn-to-face members) |

### Pipeline-stage coverage

| Stage | Coverage state | Evidence / remaining gate |
|---|---|---|
| Acquire | `evaluated-clean` | Local vendor archive identified and hashed; transaction record absent. |
| Preserve raw | `evaluated-clean` | RAR and Unitypackage retained unchanged outside the repository. |
| Inspect | `evaluated-finding` | Every FBX inspected/measured/linted; loop, rig, and optimization findings retained. |
| Segment | `partially-evaluated` | Individual files used; combined take not promoted. |
| Root motion | `evaluated-finding` | 28 named IP/RM pairs found; AnimSmith 0.4.0 `root_trajectory` now measures horizontal, vertical, and yaw per clip (76/77), closing the prior horizontal-only gap; engine root-motion extraction and environment/contact gates remain open. |
| Conform | `partially-evaluated` | Standard family and Unity shared Avatar work; one outlier and target rigs open. |
| Validate | `partially-evaluated` | Mechanical contracts, headless Unity, and Unreal/Godot import-advice attempts complete; environment/visual gates open. |
| Optimize | `evaluated-finding` | One pruning candidate verified mechanically but not accepted semantically. |
| Export | `partially-evaluated` | Sample GLB export reopens; Unity native delivery tested; other engines open. |
| Gate/report | `evaluated-clean` | Manifest and parser-validated report pair retained. |

### Readiness evidence by clip set

| Role or runtime set | File-ready / clip-ready | Set-ready / rig-use | Runtime / acceptance boundary |
|---|---|---|---|
| Wall/ladder cycles | Files readable; paired families/durations reconcile | Standard skeleton; every declared loop has strict seam findings | Unity samples execute; vertical/yaw now measured per clip; engine root-motion extraction and contact quality open |
| Obstacles/wall jumps | IP/RM pairs readable | Use discrete actions; delivered loop semantics rejected | Environment height, facing, contacts, cancellation open; vertical measured up to +2.000 m/+1.903 m exemplars |
| Fall/land | Five standard candidates readable | Outlier excluded; transitions inferred | Unity standard sample executes; vertical measured 0.000 m, matching the no-net-climb expectation; physics/landing windows open |

### Validation-profile status

| Validation profile | Selection | Result / next evidence |
|---|---|---|
| Marketplace intake | `selected` — `vendor-intended` | Archive/listing/EULA reviewed; listing and local counts disagree. |
| Blended locomotion | `not-selected` | No ground-locomotion set. |
| Root-motion controller | `selected` — `vendor-intended` | 28 named RM pairs; vertical/yaw now measured (`root_trajectory`, 76/77 clips) as sampled regression facts; engine root-motion extraction and authority acceptance remain open. |
| State-machine transitions | `selected` — `observed-pack-capability` | Entry, exit, prepare, fall, and landing families; visual boundaries open. |
| Layered upper body/weapons | `not-selected` | Full-body contact baseline is safer. |
| Traversal/environment | `selected` — `vendor-intended` | Main purpose; vertical/yaw trajectory now measured per clip; environment matrix absent. |
| Contact actions/interactions | `selected` — `observed-pack-capability` | Wall/ladder/ledge contacts implied; events/IK absent. |
| Retargeted/customizable characters | `selected` — `evaluator-selected-generic-scenario` | Unity source actor works; target rig absent. |
| Motion matching/search | `not-selected` | No database/contact annotation contract. |
| Networked movement | `not-selected` | No authority/rollback contract. |
| Runtime performance | `selected` — `evaluator-selected-generic-scenario` | Constant-track pruning sampled; runtime profiling absent. |

## Pack inventory and content evidence

The logical delivery has 179 regular files and 77 FBXs: 75 individual motions, one combined take, and one actor. The current product page advertises 69 animations, 18 root-motion, and 51 in-place, while the local archive has 75 individual motion files, 28 `_RM` files, and 47 non-RM files. This report does not assume which revision is newer.

Seventy-four individual files share skeleton signature `2b6fe49d5ae6` with 56 bones. `Humanoid@FallingUnarmed.FBX` instead has signature `3da84463466a` and 58 bones. Unity exposes no AnimationClip for that file, whereas its standard-family `Humanoid@Falling.fbx` counterpart imports. AnimSmith 0.4.0's re-inventory reproduces this exclusion unchanged; a newer tool classification alone does not invent a missing clip.

## Mechanical baseline

| Finding/check | Affected scope | Potential impact | Evidence |
|---|---|---|---|
| Default file safety | 77/77 FBXs | No NaN, time-order, quaternion, duration, scale, or bind-pose error | `observed-animsmith`; all baseline commands exit 0 |
| Constant tracks | 9,011 notes in 77 files; 8,753 in 75 individuals | Export bloat and retarget evaluation cost | Baseline/contract summaries |
| Declared loop closure | 26 errors across 22 files | Position discontinuity at wrap | Contract summary |
| Declared loop rotation seam | 41/43 loop-declared files | Angular pulse at wrap | Contract summary |
| Declared loop velocity seam | 39/43 | Velocity pulse at wrap | Contract summary |
| Semantic loop mismatch | 16 obstacle/wall-jump files are loop-declared | Repeated one-shot traversal | Filename/metadata reconciliation |
| Loop-seam applicability/evaluation (0.4.0) | 75 individual contracts: loop-seam applicable 43 / not_applicable 32; evaluation complete 34 / not_evaluated 41 | Stationary and no-stride clips are now correctly recorded `not_evaluated` instead of a mislabelled failure; per-file exit codes are unchanged from 0.3.0 (34 clean / 41 fail) | `observed-animsmith`; contract `loop_seam_applicability`/`loop_seam_evaluation` |
| gait-group check applicability (0.4.0) | 75/75 individual contracts `not_applicable` | No in-place cyclic locomotion ring exists in this traversal pack, so no gait anchoring ran; a correct not-applicable, not a refusal or a failure | `observed-animsmith`; contract `gait_group_applicability` |
| Root trajectory availability (0.4.0) | 76/77 clips measured (1 not_applicable) | 24 clips move more than 1 cm horizontally, 51 report stationary, 4 carry more than 1° of net yaw — replaces the prior horizontal-only blind spot with a measured fact; closes issue #408 | `observed-animsmith`; 0.4.0 baseline `root_trajectory`/`root_trajectory_availability` (measurements v15) |
| Root trajectory by family (0.4.0) | Per-family sampled metric-grid data, grouped by delivered-name family | Vertical ranges: ladder (n=14) -1.500..+1.500 m; obstacle (n=8) -0.043..+2.000 m; wall (n=38) -1.950..+2.000 m, up to 180° net yaw; wall-jump (n=8) -1.850..+1.918 m; fall/land (n=5) 0.000 m throughout | `observed-animsmith`; named exemplars: `Humanoid@ClimbUp2MetersObstacleUnarmed_RM.FBX` +2.000 m, `Humanoid@ExitWallTop_RM.fbx` +2.000 m, `Humanoid@EnterWallTop_RM.fbx` -1.950 m / -180° yaw, `Humanoid@WallJumpUp_RM.fbx` +1.903 m, `Humanoid@EnterLadderTopUnarmed_RM.FBX` -1.500 m, `Humanoid@ExitLadderTopUnarmed_RM.FBX` +1.500 m; sampled metric-grid regression, not continuous-curve extrema or engine root-motion extraction proof |

## AnimSmith remediation evidence

| Source issue | Operation/declarations | Result | Independent verification | Remaining caveat |
|---|---|---|---|---|
| Constant tracks in `Humanoid@WallClimbUp.fbx` | `transform --prune-constant-tracks` with its declared contract | Exit 0; FBX 563,984 bytes to GLB 71,288 bytes (2026-08-17) | Output inspect/measure and fix dry-run exit 0; diff detects change | Lint retains the source seam; equivalence unproven, so output not adopted. Bounded by open issue #401 (re-run under 0.4.0, produced a new candidate; verified open 2026-08-21). |

## Engine procedures and evidence

| Runtime | Version | Procedure | Observed result | Remaining gate |
|---|---|---|---|---|
| Unity | 6000.5.8f1 (retained 2026-08-17); `unity-humanoid` revision 1 / 6000.3 import-advice (2026-08-21) | Merge five authorized package reconstructions outside the repo; inventory importers/clips; sample five standard clips; assert the expected outlier; mix Basic walk to a climb state. Separately, run `generate import-advice` under the frozen `unity-humanoid` / revision 1 / `6000.3` / `fbx-model-importer` profile on every individual clip. | 74/75 individual clips import; 5/5 required samples and mixer pass; outlier exposes no clip (retained, unchanged). Import-advice: `available`, exit 0; each clip's `lockRootRotation`/`lockRootHeightY`/`lockRootPositionXZ` is derived from the delivered `.fbx.meta` (`useFileUnits:1`; `lockRoot*` absent on every meta, so Unity's serialization default of `false` applies, mapping to `extract`) — an assumption about the Unity serialization default, not observed 6000.5.8f1 importer behavior. | Visual contacts/displacement, controller, target rig, compression, build. |
| Unreal Engine | 5.8 | Official root-motion/animation documentation review; run `generate import-advice` under the `unreal` / revision 1 / `5.8` profile on every individual clip (2026-08-21). | Typed refusal `profile_settings_unmodeled`, exit 1: profile revision 1 has no modeled Unreal setting vocabulary. Not import-evaluated; runtime capabilities do not prove pack import. | FBX import/retarget, root lock, motion warping, contacts, build. |
| Godot | 4.7 | Official AnimationTree documentation review; run `generate import-advice` under the `godot` / revision 1 / `4.7` profile on every individual clip (2026-08-21). | Typed refusal `profile_settings_unmodeled`, exit 1: profile revision 1 has no modeled Godot setting vocabulary. Not import-evaluated. | Conversion/import, root extraction, controller, contacts, export. |
| Bevy | 0.19.0 | Official animation-mask example review; checked for a `generate addressability` candidate (2026-08-21). | Not evaluated: no generated glTF/GLB candidate exists for this pack because no gait-anchored in-place ring exists to seed one (`gait-group` not_applicable on all 75 individual contracts), so there was nothing to inventory. This is a coverage gap, not an observed Bevy failure. | glTF conversion, mapping, root policy, contacts, profiling. |

## Rig, masking, and compatibility evidence

| Pack/rig/set pair | Skeleton/retarget | Scale/axes | Root policy | Timing/blend | Overall evidence |
|---|---|---|---|---|---|
| Climbing standard motions to supplied actor | 74 standard files share 56-bone signature; Unity Humanoid succeeds | Metadata scale 1 | Paired RM/IP choice required | Five samples execute | Direct Unity candidate; target/contact untested |
| Climbing outlier | Distinct 58-bone signature | Metadata scale 1 | Unknown/no exposed clip | Cannot sample | Exclude pending author clarification |
| Climbing to Basic Locomotion | Standard signatures align; 25 shared paths byte-identical | Shared actor/assets identical | Basic approach then traversal handoff | One headless mixer executes | Co-install candidate; entry pose/feet unaccepted |
| Climbing to Sword/Campfire/Injured | Standard signature shared | All pairwise overlaps identical, zero conflicts | Full-body handoff default | Five-pack Unity import succeeds | Technical co-existence, not artistic compatibility |

## Limitations and unknowns

1. No dynamic visual review, environment geometry matrix, target retarget, contact/IK/event authoring, compression comparison, cancellation test, network correction, or player build was completed.
2. AnimSmith 0.4.0 measures vertical and yaw root displacement per clip (`root_trajectory`), but as sampled regression facts on the shared uniform metric grid — not continuous-curve extrema and not proof of what any engine's own root-motion extraction produces from the same file; environment alignment and hand/foot contact quality remain unverified.
3. Screenshots of three offline reports prove rendering at frame zero, not motion quality.
4. Unreal Engine and Godot returned typed `profile_settings_unmodeled` refusals from `generate import-advice` (their profile revisions model no settings yet); Bevy had no generated candidate to evaluate; none of the three received an actual engine import/playback pass.
5. Commercial files, derived motion outputs, screenshots, and the generated Unity project remain outside the repository and CI.
6. The 0.4.0 availability recount for `loop_seam` applicability/evaluation and `gait-group` clarifies which facts are `not_applicable` versus `unavailable`/`not_evaluated`; file-level contract pass/fail counts are unchanged from 0.3.0 (34 clean / 41 fail).
7. Measured `root_trajectory` (0.4.0) is a sampled regression fact from the shared uniform metric grid, not continuous-curve or engine-extraction proof; it does not by itself decide RM-vs-code movement-ownership axes for a game controller.

## Reproduction

Source RAR: 142,764,600 bytes, SHA-256 `4b353c3ded36889ab29096b7d0c04e54859f6dc380fa41e5ebeb925b74241101`. Extracted Unitypackage: 142,850,966 bytes, SHA-256 `4ca22fe57d8b322e91cf73a043880fc156dd2a71c1bf9f0b58d42b433731d2a1`.

2026-08-17 baseline evaluator (historical): `animsmith 0.3.0 (v0.3.0-30-gaabac28)`; revision `aabac28edf2719db236068339f1208bbf156d0bb`; binary SHA-256 `2fb43d210b5448fb2cd642946cc46df0cbb34595a48821b22a28daf7c1938f77`.

Run baseline commands for every FBX. For each individual file, apply the humanoid rig, Unity-derived loop declaration, and in-place only to the non-RM side of a real pair; avoid a generic horizontal threshold for vertical traversal — use the measured `root_trajectory` vertical/yaw facts instead. Generate and inspect three risk-selected offline reports. Prune one sample, then inspect, measure, lint, diff, and fix dry-run it. Import all five evaluated packs into Unity and run the retained headless probe.

Portable evidence digests (2026-08-17, historical): baseline `0325119190ccddbe272c74b94808853438488d21dde53b3ce3e56c1d3461800c`; contract `5a95252f8046ca1471022327140331c6233cb376c89de6aa91805b3427de6d6e`; catalog `3c752cc75f73ea6589916e62a35b52aa2f6a004e7af3c39c84970ef6a4744419`; remediation `c1a34efc74d287a5b3334e9ec708b0498286342f42761656bb11049dab3737d`; Basic comparison `96f1ffe158139600495281b57e6a9c37d61720bc9029feb23590681a8e163e5d`; combined Unity probe `d2b6d1b0af14c2c77dca3c2cc4aa892d6e507f3cf8b9bb50bfdb4ef78d407afa`.

2026-08-21 refresh evaluator: `animsmith 0.4.0`, tag `v0.4.0`; revision `6b37ad636b198ef8ff47fadbf6a3a51eb1a27c8e`; binary SHA-256 `fd1eee57407aa02db88763d144389a7f5104204c40ddfbb28eb5885ca8cd54c6`; output schema v10; measurements schema v15. Re-ran the source inventory against the same archive: byte-identical to the published manifest (0 added, 0 removed, 0 changed across 77 FBXs); archive SHA-256 `4b353c3ded36889ab29096b7d0c04e54859f6dc380fa41e5ebeb925b74241101` re-verifies. Re-ran `inspect`, `measure --format json`, and `lint --format json` for both the untouched baseline and the retained declared contract on every FBX; findings, exit codes, and constant-track/loop-closure/loop-seam counts reproduce the 2026-08-17 numbers exactly, with the newly available `loop_seam_ratio`, `gait.phase_availability`, `gait-group` applicability, and `root_trajectory` facts now populated. Ran `generate import-advice` under the frozen `unity-humanoid` (Unity 6000.3), `unreal` (5.8), and `godot` (4.7) profiles on every individual clip: `unity-humanoid` returned `available` (exit 0); `unreal` and `godot` returned the typed `profile_settings_unmodeled` refusal (exit 1). Checked for a Bevy `generate addressability` candidate; none exists because this pack has no gait-anchored in-place ring, so Bevy 0.19.0 is recorded not-evaluated rather than failed. Re-ran the `--prune-constant-tracks` trial on `Humanoid@WallClimbUp.fbx` (source not modified; bounded by open issue #401, verified open 2026-08-21). The retained Unity 6000.5.8f1 headless probe (2026-08-17) was not rerun because the source is byte-identical; it keeps its original date and attribution.

## Sources

- Local authorized source archive, extracted Unity metadata, and bundled animation list — private evidence identified above, accessed 2026-08-17.
- Protofactor, [Animset: Climbing](https://protofactor.biz/product/animset-climbing/), [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/), and [EULA](https://protofactor.biz/end-user-license-agreement/) — current listing/license context, accessed 2026-08-17.
- Unity Asset Store, [Ultimate Animation Collection](https://marketplace.unity.com/packages/3d/animations/ultimate-animation-collection-195459) — current collection version 1.65, release date 2026-08-16, Single Entity listing, and original Unity 6000.5.1; not local constituent identity.
- Unity, [Root Motion](https://docs.unity3d.com/6000.5/Documentation/Manual/RootMotion.html) and [looping clips](https://docs.unity3d.com/6000.5/Documentation/Manual/LoopingAnimationClips.html); Epic Games, [root motion](https://dev.epicgames.com/documentation/unreal-engine/root-motion-in-unreal-engine?application_version=5.8); Godot, [AnimationTree](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html); Bevy, [Animation Masks](https://bevy.org/examples/animation/animation-masks/) — capability context only, accessed 2026-08-17.
- AnimSmith, [Unity 6000.3 animation profile](../engine-profile-unity.md), [Unreal Engine 5.8 animation profile](../engine-profile-unreal.md), [Godot 4.7 animation profile](../engine-profile-godot.md), and [Bevy 0.19.0 animation profile](../engine-profile-bevy.md) — modeled import-advice profile facts for the 2026-08-21 refresh, accessed 2026-08-21.
