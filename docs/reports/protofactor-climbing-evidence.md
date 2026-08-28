# Animation pack evidence appendix: Protofactor Climbing

> Companion report: [Protofactor Climbing](protofactor-climbing.md)
>
> Evidence status: **partial** — exact AnimSmith 0.7.0 baseline, contracts, pruning verification, addressability, and bounded advice plus a dated Unity 6000.5.8f1 observation; visual traversal, engine root-motion extraction, target-character, and engine-editor/runtime passes remain absent.
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
| Pack/edition | Protofactor Climbing constituent from a locally held Ultimate Animation Collection archive; local revision unknown |
| Vendor/source | Protofactor [Climbing product](https://protofactor.biz/product/animset-climbing/) and [collection](https://protofactor.biz/product/ultimate-animation-collection/) pages |
| Delivered scope | RAR to one Unitypackage to 179 logical files; 77 FBXs: 75 individual motions, one combined take, one actor |
| Target use | Game-engine wall, ladder, obstacle, airborne, and landing traversal combined with evaluated collection packs |
| Target engines | Unity 6000.5.8f1 observed headless (retained 2026-08-17); Unity 6000.3, Unreal Engine 5.8, and Godot 4.7 AnimSmith import-advice probed (2026-08-21); Bevy 0.19.0 documentation-only, no generated glTF/GLB candidate |
| Target rigs/packs | Supplied Protof-Actor; Basic Locomotion, Sword & Shield, Campfire, and Injured selective compatibility |
| Source manifest | `climbing/source-archive-inventory.json`; RAR SHA-256 `4b353c3ded36889ab29096b7d0c04e54859f6dc380fa41e5ebeb925b74241101`; re-inventoried 2026-08-21 under AnimSmith 0.7.0, byte-identical to the published manifest (0 added, 0 removed, 0 changed) |
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
| Wall-climb 8-way | directional-blend | 8 IP plus 8 RM | Common 1.333 s duration and direction names; high confidence | All 16 strict-loop failures; per-clip `root_trajectory` includes vertical/yaw; visual/engine-extraction-open |
| Ladder up/down | directional-blend | 2 IP plus 2 RM | Common 1.200 s duration and paired labels; high confidence | All 4 strict-loop failures; per-clip evidence includes vertical/yaw; visual/engine-extraction-open |
| Wall-jump 4-way | directional-blend | 4 IP plus 4 RM | Direction family; high confidence | Discrete-action candidate; all 8 delivered loops fail; wall-jump family vertical range -1.850..+1.918 m (max abs 1.903 m, `Humanoid@WallJumpUp_RM.fbx`) |
| Obstacle alternatives | other | 4 IP plus 4 RM | Height/side choices; high confidence | Discrete actions; all 8 delivered loops fail; obstacle family vertical range -0.043..+2.000 m (`Humanoid@ClimbUp2MetersObstacleUnarmed_RM.FBX` +2.000 m) |
| Fall-and-land chain | transition-chain | 5 logical motions | Apex/fall/landing labels; medium confidence | Outlier excluded; fall/land family vertical range 0.000 m throughout, as expected; visual transition-open |
| Opposite-wall jump chain | transition-chain | 6 logical motions, 2 RM companions | Mirrored prepare/hold/jump labels; medium confidence | Contact/facing-open; per-clip evidence includes vertical/yaw (wall family up to 180° net yaw on turn-to-face members) |

### Pipeline-stage coverage

| Stage | Coverage state | Evidence / remaining gate |
|---|---|---|
| Acquire | `evaluated-clean` | Local vendor archive identified and hashed; transaction record absent. |
| Preserve raw | `evaluated-clean` | RAR and Unitypackage retained unchanged outside the repository. |
| Inspect | `evaluated-finding` | Every FBX inspected/measured/linted; loop, rig, and optimization findings retained. |
| Segment | `partially-evaluated` | Individual files used; combined take not promoted. |
| Root motion | `evaluated-finding` | 28 named IP/RM pairs found; current `root_trajectory` evidence covers horizontal, vertical, and yaw motion for 76/77 clips; engine root-motion extraction and environment/contact gates remain open. |
| Conform | `partially-evaluated` | Standard family and Unity shared Avatar work; one outlier and target rigs open. |
| Validate | `partially-evaluated` | Mechanical contracts, headless Unity, and Unreal/Godot import-advice attempts complete; environment/visual gates open. |
| Optimize | `evaluated-finding` | One pruning candidate verified mechanically but not accepted semantically. |
| Export | `partially-evaluated` | Sample GLB export reopens; Unity native delivery tested; other engines open. |
| Gate/report | `evaluated-clean` | Manifest and parser-validated report pair retained. |

### Readiness evidence by clip set

| Role or runtime set | File-ready / clip-ready | Set-ready / rig-use | Runtime / acceptance boundary |
|---|---|---|---|
| Wall/ladder cycles | Files readable; paired families/durations reconcile | Standard skeleton; every declared loop has strict seam findings | Unity samples execute; per-clip evidence includes vertical/yaw; engine root-motion extraction and contact quality open |
| Obstacles/wall jumps | IP/RM pairs readable | Use discrete actions; delivered loop semantics rejected | Environment height, facing, contacts, cancellation open; vertical measured up to +2.000 m/+1.903 m exemplars |
| Fall/land | Five standard candidates readable | Outlier excluded; transitions inferred | Unity standard sample executes; vertical measured 0.000 m, matching the no-net-climb expectation; physics/landing windows open |

### Validation-profile status

| Validation profile | Selection | Result / next evidence |
|---|---|---|
| Marketplace intake | `selected` — `vendor-intended` | Archive/listing/EULA reviewed; listing and local counts disagree. |
| Blended locomotion | `not-selected` | No ground-locomotion set. |
| Root-motion controller | `selected` — `vendor-intended` | 28 named RM pairs; sampled `root_trajectory` evidence includes vertical/yaw for 76/77 clips; engine root-motion extraction and authority acceptance remain open. |
| State-machine transitions | `selected` — `observed-pack-capability` | Entry, exit, prepare, fall, and landing families; visual boundaries open. |
| Layered upper body/weapons | `not-selected` | Full-body contact baseline is safer. |
| Traversal/environment | `selected` — `vendor-intended` | Main purpose; per-clip evidence includes vertical/yaw trajectory; environment matrix absent. |
| Contact actions/interactions | `selected` — `observed-pack-capability` | Wall/ladder/ledge contacts implied; events/IK absent. |
| Retargeted/customizable characters | `selected` — `evaluator-selected-generic-scenario` | Unity source actor works; target rig absent. |
| Motion matching/search | `not-selected` | No database/contact annotation contract. |
| Networked movement | `not-selected` | No authority/rollback contract. |
| Runtime performance | `selected` — `evaluator-selected-generic-scenario` | Constant-track pruning sampled; runtime profiling absent. |

## Pack inventory and content evidence

The logical delivery has 179 regular files and 77 FBXs: 75 individual motions, one combined take, and one actor. The current product page advertises 69 animations, 18 root-motion, and 51 in-place, while the local archive has 75 individual motion files, 28 `_RM` files, and 47 non-RM files. This report does not assume which revision is newer.

Seventy-four individual files share skeleton signature `2b6fe49d5ae6` with 56 bones. `Humanoid@FallingUnarmed.FBX` instead has signature `3da84463466a` and 58 bones. Unity exposes no AnimationClip for that file, whereas its standard-family `Humanoid@Falling.fbx` counterpart imports. AnimSmith 0.7.0's re-inventory reproduces this exclusion unchanged; a newer tool classification alone does not invent a missing clip.

## Mechanical baseline

| Finding/check | Affected scope | Potential impact | Evidence |
|---|---|---|---|
| Default file safety | 77/77 FBXs | No NaN, time-order, quaternion, duration, scale, or bind-pose error | `observed-animsmith`; all baseline commands exit 0 |
| Constant tracks | 9,011 notes in 77 files; 8,753 in 75 individuals | Export bloat and retarget evaluation cost | Baseline/contract summaries |
| Declared loop closure | 26 errors across 22 files | Position discontinuity at wrap | Contract summary |
| Declared loop rotation seam | 41/43 loop-declared files | Angular pulse at wrap | Contract summary |
| Declared loop velocity seam | 39/43 | Velocity pulse at wrap | Contract summary |
| Semantic loop mismatch | 16 obstacle/wall-jump files are loop-declared | Repeated one-shot traversal | Filename/metadata reconciliation |
| Loop-seam applicability/evaluation (current) | 75 individual contracts: loop-seam applicable 43 / not_applicable 32; evaluation complete 34 / not_evaluated 41 | Stationary and no-stride clips account for most `not_evaluated` results; per-file exit codes are 34 clean / 41 findings | `observed-animsmith`; contract `loop_seam_applicability`/`loop_seam_evaluation` |
| gait-group check applicability (current) | 75/75 individual contracts `not_applicable` | No in-place cyclic locomotion ring exists in this traversal pack, so no gait anchoring ran; a correct not-applicable, not a refusal or a failure | `observed-animsmith`; contract `gait_group_applicability` |
| Root trajectory availability (current) | 76/77 clips measured (1 not_applicable) | 24 clips move more than 1 cm horizontally, 51 report stationary, 4 carry more than 1° of net yaw; sampled-grid evidence only | `observed-animsmith`; current baseline `root_trajectory`/`root_trajectory_availability` (measurements v16) |
| Root trajectory by family (current) | Per-family sampled metric-grid data, grouped by delivered-name family | Vertical ranges: ladder (n=14) -1.500..+1.500 m; obstacle (n=8) -0.043..+2.000 m; wall (n=38) -1.950..+2.000 m, up to 180° net yaw; wall-jump (n=8) -1.850..+1.918 m; fall/land (n=5) 0.000 m throughout | `observed-animsmith`; named exemplars: `Humanoid@ClimbUp2MetersObstacleUnarmed_RM.FBX` +2.000 m, `Humanoid@ExitWallTop_RM.fbx` +2.000 m, `Humanoid@EnterWallTop_RM.fbx` -1.950 m / -180° yaw, `Humanoid@WallJumpUp_RM.fbx` +1.903 m, `Humanoid@EnterLadderTopUnarmed_RM.FBX` -1.500 m, `Humanoid@ExitLadderTopUnarmed_RM.FBX` +1.500 m; sampled metric-grid regression, not continuous-curve extrema or engine root-motion extraction proof |

## AnimSmith remediation evidence

| Source issue | Operation/declarations | Result | Independent verification | Remaining caveat |
|---|---|---|---|---|
| Constant tracks in `Humanoid@WallClimbUp.fbx` | `transform --prune-constant-tracks` with its declared contract | Exit 0; FBX 563,984 bytes to GLB 71,288 bytes (2026-08-17) | Output inspect/measure and fix dry-run exit 0; diff detects change | Lint retains the source seam; equivalence unproven, so output not adopted. Bounded by open issue #401. |

## Engine procedures and evidence

| Runtime | Version | Procedure | Observed result | Remaining gate |
|---|---|---|---|---|
| Unity Humanoid projection | Revision 1 / Unity 6000.3 | Current `generate import-advice` projection | Available; bounded settings projection only | Verify importer settings on the target project and character |
| Unreal Engine | 5.8 | Official root-motion/animation documentation review; run `generate import-advice` under the `unreal` / revision 2 / `5.8` profile on every individual clip (2026-08-21). | Current revision-2 settings projection is available; no engine process ran. | FBX import/retarget, root lock, motion warping, contacts, build. |
| Godot | 4.7 | Official AnimationTree documentation review; run `generate import-advice` under the `godot` / revision 2 / `4.7` profile on every individual clip (2026-08-21). | Current revision-2 settings projection is available; no engine process ran. | Conversion/import, root extraction, controller, contacts, export. |
| Bevy | 0.19.0 | Official animation-mask example review; checked for a `generate addressability` candidate (2026-08-21). | Not evaluated: no generated glTF/GLB candidate exists for this pack because no gait-anchored in-place ring exists to seed one (`gait-group` not_applicable on all 75 individual contracts), so there was nothing to inventory. This is a coverage gap, not an observed Bevy failure. A collection-wide headless Unity glTFast import of 134 GLB candidates ran 2026-08-21 (see GLB candidate import below), but none of those candidates came from this pack; Bevy addressability stays not-evaluated for Climbing. | glTF conversion, mapping, root policy, contacts, profiling. |

### Unity headless candidate probe (2026-08-21 correction)

A direct Unity 6000.5.8f1 headless sample of 120 clips observed baked root rotation, baked XZ for nearly every in-place clip, and extracted XZ for most root-motion clips. This is sampled importer evidence, not visual or gameplay acceptance.

| Variant | Clips | `lockRootRotation` true | `lockRootHeightY` true | `lockRootPositionXZ` true |
|---|---:|---:|---:|---:|
| In-place (non-`_RM`) | 84 | 84 | 84 | 83 |
| Root-motion (`_RM`) | 36 | 36 | 28 | 5 |

Aggregate across the sample: 120/120 clip definitions inspected, 120/120 `lockRootRotation` true, 112/120 `lockRootHeightY` true, 88/120 `lockRootPositionXZ` true. The observed policy is per-variant and axis-specific: rotation is baked throughout, while XZ is baked for in-place clips and extracted for most root-motion clips. This does not prove engine root-motion extraction or environment contact.

### GLB candidate import into Unity (2026-08-21) — collection-level context, not a pack result

All 134/134 AnimSmith 0.7.0 gait-anchored GLB candidates across the eight-pack collection were staged into a separate, **new** Unity 6000.5.8f1 project using `com.unity.cloud.gltfast` 6.9.0, because Unity has no native GLB importer; the retained five-pack project above was not modified. Result: 134/134 files staged produced assets, 134/134 produced exactly one Unity `AnimationClip`, and every clip is non-legacy and non-empty. **Climbing has no in-place gait ring (`gait-group` `not_applicable` on all 75 individual contracts) and therefore contributed none of the 134 candidates** — this result is reported here only as collection-level context, not as a Climbing pack result, and it does not change Bevy addressability, which stays not-evaluated for this pack.

**Limit, stated plainly, for the candidates that do exist:** glTFast imports glTF animation as a **Generic** clip and does not reconstruct a Humanoid Avatar. The 134/134 result proves those candidates load and yield one well-formed clip in Unity; it does **not** test the Humanoid retarget path the source packs actually use, and it is not a visual or gameplay acceptance test.

## Rig, masking, and compatibility evidence

| Pack/rig/set pair | Skeleton/retarget | Scale/axes | Root policy | Timing/blend | Overall evidence |
|---|---|---|---|---|---|
| Climbing standard motions to supplied actor | 74 standard files share 56-bone signature; Unity Humanoid succeeds | Metadata scale 1 | Paired RM/IP choice required | Five samples execute | Direct Unity candidate; target/contact untested |
| Climbing outlier | Distinct 58-bone signature | Metadata scale 1 | Unknown/no exposed clip | Cannot sample | Exclude pending author clarification |
| Climbing to Basic Locomotion | Standard signatures align; 25 shared paths byte-identical | Shared actor/assets identical | Basic approach then traversal handoff | One headless mixer executes | Co-install candidate; entry pose/feet unaccepted |
| Climbing to Sword/Campfire/Injured | Standard signature shared | All pairwise overlaps identical, zero conflicts | Full-body handoff default | Five-pack Unity import succeeds | Technical co-existence, not artistic compatibility |

## Limitations and unknowns

1. No dynamic visual review, environment geometry matrix, target retarget, contact/IK/event authoring, compression comparison, cancellation test, network correction, or player build was completed.
2. AnimSmith 0.7.0 measures vertical and yaw root displacement per clip (`root_trajectory`), but as sampled regression facts on the shared uniform metric grid — not continuous-curve extrema and not proof of what any engine's own root-motion extraction produces from the same file; environment alignment and hand/foot contact quality remain unverified.
3. Screenshots of three offline reports prove rendering at frame zero, not motion quality.
4. Current Unreal revision-2 and Godot revision-2 settings projections are available, but neither engine received an import or playback test.
5. Commercial files, derived motion outputs, screenshots, and the generated Unity project remain outside the repository and CI.
6. The current availability recount for `loop_seam` applicability/evaluation and `gait-group` clarifies which facts are `not_applicable` versus `unavailable`/`not_evaluated`; file-level contract pass/fail counts are.
7. Measured `root_trajectory` (current) is a sampled regression fact from the shared uniform metric grid, not continuous-curve or engine-extraction proof; it does not by itself decide RM-vs-code movement-ownership axes for a game controller.
8. A direct Unity 6000.5.8f1 headless sample of 120 clips observed baked root rotation, baked XZ for nearly every in-place clip, and extracted XZ for most root-motion clips. This is sampled importer evidence, not visual or gameplay acceptance.
9. The 134/134 collection candidate Unity import is context only: Climbing contributed no gait candidates. It proves nothing about this pack and does not establish Humanoid retarget.

## Changes between AnimSmith versions

| Evaluator | Change from the preceding evaluated state |
|---|---|
| AnimSmith 0.7.0 | Revalidated the 77-FBX baseline, 75 declared contracts, traversal displacement/yaw evidence, pruning trial, and current engine projections under output v17 / measurements v16. |
| AnimSmith 0.4.1 | Reproduced the evaluated 0.4.0 results for this corpus; unrelated release fixes did not change the pack conclusion. |
| AnimSmith 0.4.0 | Added root-trajectory evidence needed to distinguish vertical traversal from horizontal locomotion and retained the same mechanical findings. |
| AnimSmith 0.3.0 | Established the initial baseline, contract, and dated Unity evidence. Those evaluator results are superseded. |

## Reproduction

### Current AnimSmith reproduction (2026-08-26)

The unchanged corpus was rerun with `animsmith 0.7.0`, exact tag `v0.7.0`, commit `461ac8a4f6bb368eb8637471a796f13eeb647140`, binary SHA-256 `01a501999c91d93abfb32b1f48241fccc70914fac27c9a650c31df44262578d8`, output schema v17, and measurements schema v16.

| Current external evidence | SHA-256 | Result |
|---|---|---|
| Source inventory | `3b7d199c5d19e2b593baf06053bc73ef6e456f1ea39394952cc3b37178141e26` | 77 FBXs; source unchanged |
| Exhaustive baseline | `22c29dc1d8853df56bd94ba9e627fda7db8875efd328729852b5a45d26b3ccf5` | 77/77 complete |
| Declared contracts | `3729e12c28b88237c8f0291e55047f4d0df81d079c744122125f8e1700776d24` | 75 files; 34 pass / 41 fail |
| Remediation | `ca0117478e6f32f04bc27512f20769f551908bbf04cc1e1a056a3810e680d550` | Pruning candidate completed and verified |
| 0.7 supplemental projections | `3b3aa46ff91710d16b1a916e026f70d3dde3187e1417f082cba97603bc045ecd` | Addressability V1 + rich V2; exact-profile advice available |

The current projections do not evaluate ledge topology, contacts, root-motion extraction in an engine, retarget deformation, or visual continuity.

## Sources

- Local authorized source archive, extracted Unity metadata, and bundled animation list — private evidence identified above, accessed 2026-08-17.
- Protofactor, [Animset: Climbing](https://protofactor.biz/product/animset-climbing/), [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/), and [EULA](https://protofactor.biz/end-user-license-agreement/) — current listing/license context, accessed 2026-08-17.
- Unity Asset Store, [Ultimate Animation Collection](https://marketplace.unity.com/packages/3d/animations/ultimate-animation-collection-195459) — current collection version 1.65, release date 2026-08-16, Single Entity listing, and original Unity 6000.5.1; not local constituent identity.
- Unity, [Root Motion](https://docs.unity3d.com/6000.5/Documentation/Manual/RootMotion.html) and [looping clips](https://docs.unity3d.com/6000.5/Documentation/Manual/LoopingAnimationClips.html); Epic Games, [root motion](https://dev.epicgames.com/documentation/unreal-engine/root-motion-in-unreal-engine?application_version=5.8); Godot, [AnimationTree](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html); Bevy, [Animation Masks](https://bevy.org/examples/animation/animation-masks/) — capability context only, accessed 2026-08-17.
- AnimSmith, [Unity 6000.3 animation profile](../engine-profile-unity.md), [Unreal Engine 5.8 animation profile](../engine-profile-unreal.md), [Godot 4.7 animation profile](../engine-profile-godot.md), and [Bevy 0.19.0 animation profile](../engine-profile-bevy.md) — modeled import-advice profile facts for the 2026-08-21 refresh, accessed 2026-08-21.
