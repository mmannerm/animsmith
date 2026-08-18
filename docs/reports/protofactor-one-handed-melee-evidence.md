# Animation pack evidence appendix: Protofactor 1-Handed Melee Weapon Animset

> Companion report: [technical evaluation](protofactor-one-handed-melee.md)
>
> Evidence status: **partial** — exhaustive 0.3.0 baseline/contracts, refreshed 0.3.1-bound IP gait candidates, and a Unity 6000.5.8f1 eight-pack probe; transformed-clip visual acceptance and three engines remain unevaluated.
>
> Evaluation date: **2026-08-17**
>
> Report format: **1**

This appendix preserves the detailed evidence behind the concise report. The [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder) remains authoritative.

## Evaluation scope and provenance

| Field | Value |
|---|---|
| Pack/edition | Local `Animset@1HandedMeleeWeapon_PACKAGE.unitypackage`; constituent revision is not declared |
| Vendor/source | Protofactor; [current 1-Handed Melee product page](https://protofactor.biz/product/animset-1-handed-melee-weapon/) |
| Delivered scope | Full local RAR → one Unitypackage → 113 FBXs: 110 individual motions, one combined animation FBX, one skinned actor, and one bludgeon prop; Unity metadata/materials/textures included |
| Target use | Game-engine use; third-person one-handed combat and combined use with the seven previously evaluated constituents |
| Target engines | Unity 6000.5.8f1 observed; Unreal Engine, Godot, and Bevy documentation-only |
| Target rigs/packs | Supplied Protof-Actor, Basic Locomotion, Sword & Shield, and the evaluated collection subset |
| Source manifest | `logical-assets-inventory.json`; SHA-256 `aa36db891581ef7fb6e35cfff781958ca67afc1f43c403327ad648832c483f17` |
| Evaluation manifest | `evidence/evaluation-manifest.json`; SHA-256 `f4a76e505dea5eab9afbff0602f6091f08bfa053e0d1956922da193fc375c5c8`; schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1`; taxonomy/profile-set version 1 |
| Acquisition/license provenance | `user-stated`: local archive was downloaded from Protofactor.biz as part of the Ultimate Animation Collection. Current EULA permits protected released real-time applications and modification while restricting redistribution/resale; no receipt, download date, historical EULA, or local constituent revision was retained. Technical due diligence only, not legal advice. |

The current product page, observed 2026-08-17, advertises USD 19.99, 99 animations, 38 root-motion and 61 in-place files, Unity Humanoid, Epic-skeleton scale/retarget intent, Unity 2018.4.2+, and no native UE4 package. The local delivery has 110 individual files and 38 `_RM` labels, so the current listing does not identify the local constituent revision or explain the 11-file difference.

The evaluation manifest uses schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1`.

### Evidence coverage

| Surface | Offered/delivered | Evaluated | Findings | Not evaluated and why |
|---|---:|---:|---:|---|
| Animation files | 111 animation-bearing FBXs | 111 | 110 individual plus one combined take; prop is static | Continuous artistic review of all motion |
| Rigs/export variants | 4 observed structures | 4 | 109 individual files share the 56-bone signature; one 73-bone outlier; actor/combined 58; prop 3 | Target-character deformation and non-Unity retarget |
| AnimSmith baseline | 113 FBXs | 113 | All commands exit 0; 13,629 constant-track notes | Artistic intent and contacts |
| Declared contracts | 110 individual files | 110 | 23 pass, 87 fail under delivered/inferred declarations | Human loop intent for every action |
| Offline visual reports | 110 possible | 5 risk-selected | Reports generated for locomotion, attack, and both outliers | Motion/contact/deformation acceptance |
| Engine import/playback | 110 individual files in Unity | 110 | 108 Humanoid; two Generic outliers; six required samples pass | Full graph, visual root motion, compression, build |
| Blend/mask/retarget | 3 collection graphs + prop | 3 + 1 | Basic and Sword mixers, Basic mask, and prop attachment execute | Visual blending, grip, target retarget, IK |

### Claim legend

Evidence labels follow the versioned taxonomy: `user-stated`, `observed-file`, `observed-animsmith`, `observed-report`, `observed-engine`, `vendor-stated`, `documentation-stated`, `inferred`, and `not-evaluated`.

## Evaluation manifest and taxonomy

### Canonical clip-role inventory

| Canonical primary role | Logical motions | Delivered files | Evidence boundary |
|---|---:|---:|---|
| `idle-pose` | 6 | 6 | Filename/clip inspection; IdleBlock is not Unity Humanoid. |
| `continuous-locomotion` | 27 | 54 | Paired filenames, timing, horizontal speed, and gait measured. |
| `locomotion-transition` | 0 | 0 | No promoted locomotion-transition role. |
| `airborne` | 5 | 5 | Falling/apex/landing names; no jump-controller acceptance. |
| `traversal` | 0 | 0 | Not delivered. |
| `action-interaction` | 13 | 20 | Combat/equipment semantics inferred; contacts/events absent. |
| `reaction-death` | 21 | 25 | Reaction/death naming; Blocked outlier is Generic. |
| `emote-cinematic` | 0 | 0 | Not classified. |
| `other-unknown` | 0 | 0 | None after bounded classification. |
| **Total** | **72** | **110** | Validated v1 manifest identified above. |

### Runtime-set inventory

| Runtime set | Type | Members/variants | Grouping evidence | Validation status |
|---|---|---|---|---|
| Walk combat 8-way | directional-blend | 8 IP + 8 RM | Exact names, common duration, measured directions/speeds; see [primary table](protofactor-one-handed-melee.md#runtime-sets-and-authored-motion). | Mechanically measured; phase/visual acceptance open. |
| Run 8-way | directional-blend | 8 IP + 8 RM | Exact names and measured speed; backward duration differs. | Mechanically measured; phase/visual acceptance open. |
| Crouch combat 8-way | directional-blend | 8 IP + 8 RM | Exact names, common duration, measured speeds. | Mechanically measured; phase/visual acceptance open. |
| Hold forward speed | speed-blend | walk/run/sprint IP + RM | Filename semantics and measured 0.787/3.117/7.500 m/s RM speeds. | Threshold candidate; visual blend open. |
| Draw/combat/put-away | transition-chain | 3 single files | Delivered equipment verbs and combat idle. | Unity draw sample passes; endpoint/visibility timing open. |
| Heavy-hit 4-way | other | 4 IP + 4 RM | Directional names and paired files. | Discrete-selection candidate; contact/owner open. |

### Pipeline-stage coverage

| Stage | Coverage state | Evidence / remaining gate |
|---|---|---|
| Acquire | `evaluated-clean` | Local archive identified and hashed outside the repository. |
| Preserve raw | `evaluated-clean` | RAR and Unitypackage retained unchanged. |
| Inspect | `evaluated-finding` | Every FBX inspected/measured/linted. |
| Segment | `partially-evaluated` | Individual FBXs used; combined take not promoted. |
| Root motion | `evaluated-finding` | Paired labels and horizontal speed measured; yaw intent open. |
| Conform | `evaluated-finding` | 56-bone majority plus two Unity Generic block exceptions. |
| Validate | `partially-evaluated` | Mechanical contracts complete; visual combat acceptance open. |
| Optimize | `evaluated-finding` | Twenty-four current gait candidates and one pruning candidate generated; runtime/equivalence acceptance remains open. |
| Export | `partially-evaluated` | Generated GLBs are evidence only; native engine exports are not accepted. |
| Gate/report | `evaluated-clean` | Manifest and report pair use parser validation. |

### Readiness evidence by clip set

| Role or runtime set | File-ready / clip-ready | Set-ready / rig-use | Runtime / acceptance boundary |
|---|---|---|---|
| Three locomotion rings | 48/48 parse; delivered loop contracts expose seam failures. | Raw spreads 0.554/0.734/0.714; current IP candidates 0.064/0.108/0.039. | Keep RM raw; transformed IP engine/visual acceptance and residual offsets remain. |
| Hold speed chain | 6/6 parse and measure. | Same rig; large intended speed range. | Thresholds and blend quality need controller review. |
| Non-locomotion gameplay | 56 idle, airborne, action, and reaction files mechanically analyzed. | Two block clips are not Unity Humanoid; contacts/events absent. | Full-body default; visual gameplay acceptance open. |
| Prop and masks | Static prop imports at plausible scale; mask graph executes. | Right-hand identity attachment only. | Grip/orientation, pelvis torque, IK, and hit arcs open. |

### Validation-profile status

| Validation profile | Selection | Result / next evidence |
|---|---|---|
| Marketplace intake | `selected` — `vendor-intended` | Full local archive inventoried; current listing differs from local count. |
| Blended locomotion | `selected` — `observed-pack-capability` | Three rings and speed chain measured; phase/visual gates open. |
| Root-motion controller | `selected` — `observed-pack-capability` | RM speed measured; yaw and action ownership open. |
| State-machine transitions | `selected` — `observed-pack-capability` | Equipment and combat states identified; interruption/events open. |
| Layered upper body/weapons | `selected` — `evaluator-selected-generic-scenario` | One Unity mask executes; full-body remains default. |
| Traversal/environment | `not-selected` | No traversal set in this bounded pack. |
| Contact actions/interactions | `selected` — `observed-pack-capability` | Attacks/reactions present; contact and hit windows open. |
| Retargeted/customizable characters | `selected` — `evaluator-selected-generic-scenario` | Supplied avatar only; target character absent. |
| Motion matching/search | `not-selected` | No database/search target supplied. |
| Networked movement | `not-selected` | No authority/rollback contract supplied. |
| Runtime performance | `selected` — `evaluator-selected-generic-scenario` | Constant tracks diagnosed; runtime profiling absent. |

## Pack inventory and content evidence

The Unitypackage materializes 270 collection-relative files. It contains one pack-specific animation folder, shared Protof-Actor content, one bludgeon prop, metadata, materials, and textures. The 110 individual motion files collapse to 72 logical motions because 38 have `_RM` counterparts. All 25 paths overlapping each other evaluated pack are byte-identical in the eight-pack comparison.

## Mechanical baseline

| Finding/check | Affected scope | Potential impact | Evidence |
|---|---|---|---|
| Default mechanical pass with constant-track notes | 113/113 FBXs complete the baseline commands; 13,629 notes in 112 animated files | Export bloat; no default hard blocker. | `observed-animsmith`; baseline summary. |
| Declared loop closure/derivatives | 87/110 contracts fail; 52 closure, 87 rotation, 87 velocity file failures | Pops/pulses if delivered loop flags are trusted. | `observed-animsmith`; contract summary. |
| Unity Generic block files | 2/110 | Required block reaction/idle cannot share the Humanoid combat graph. | `observed-engine`; four-outlier probe includes both. |
| Directional gait phase | Three IP/RM rings | Same-time blends can skate despite paired IP/RM agreement. | Historical `b7c215b` baseline; raw phase spreads 0.554–0.734. |
| RM speed variation | Walk 1.94×; run 1.11×; crouch 1.52× | Direction-dependent controller velocity. | `observed-animsmith`; primary table. |

## AnimSmith remediation evidence

| Source issue | Operation/declarations | Result | Independent verification | Remaining caveat |
|---|---|---|---|---|
| Missing semantic loop/in-place context | Per-file `[clips."Take 001"]` declarations derived from Unity loop metadata and observed IP/RM pairs | 23 pass; 87 fail, exposing raw loop policy. | JSON and Markdown results agree for all 110. | Delivered one-shot loop intent still needs curation. |
| Ring phase disagreement | Current `transform --gait-anchor` on 24 core IP files | 24/24 exit 0 and emit GLBs. | Inspect/measure/fix dry-run 24/24 exit 0; post spreads walk 0.0639032, run 0.1081981, crouch 0.0394317; lint/diff 24/24 exit 1 for remaining contracts/semantic rewrites. | Only IP transformed; no Unity GLB importer, visual/contact, or trajectory acceptance; residual offsets remain. |
| Dense constant tracks | `transform --prune-constant-tracks` on `Humanoid@IdleCombat1hMelee.fbx` | GLB produced. | Inspect/measure and fix dry-run exit 0; diff/lint retain expected semantic differences/findings. | Runtime equivalence and sparse transition behavior unproved. |

The earlier `b7c215b` heading-basis refusals are historical only. Revision `674396f` implements the merged [#426](https://github.com/mmannerm/animsmith/issues/426) basis policy and emits all 24 IP candidates after its bounded translation/yaw safety checks. No RM file was transformed, and the Unity project had no GLB importer, so lower spreads are mechanical evidence rather than set-ready or visual acceptance. All generated candidates remain outside the repository with the commercial inputs.

## Engine procedures and evidence

| Runtime | Version | Procedure | Observed result | Remaining gate |
|---|---|---|---|---|
| Unity | 6000.5.8f1 | Fresh eight-pack project; import all delivered models; inventory importer/avatar/clip state; sample six clips; run Basic/Sword mixers, a Humanoid upper-body mask, and right-hand prop attachment. | 108/110 individual Humanoid clips; six samples, two mixers, mask, and prop pass; both expected Generic outliers fail. | Add a GLB importer or convert candidates, then test gait outputs; visual graph, contacts, root motion, target retarget, compression/build. |
| Unreal Engine | unspecified | Documentation review for Root Motion, Blend Spaces, montages, and layered animation. | Capability documented; pack not imported. | Import/retarget/graphs/contacts/build. |
| Godot | stable | Documentation review for AnimationTree blend spaces, filters, one-shots, and root motion. | Capability documented; pack not imported. | Conversion/import, retarget, graphs, contacts/export. |
| Bevy | unspecified | Documentation review for AnimationGraph masks. | Mask capability documented; pack not imported. | FBX→glTF, retarget, graphs, root motion/performance. |

## Rig, masking, and compatibility evidence

| Pack/rig/set pair | Skeleton/retarget | Scale/axes | Root policy | Timing/blend | Overall evidence |
|---|---|---|---|---|---|
| 1-Hand majority ↔ supplied actor | 109 files use collection 56-bone structure; Unity accepts 108 individual clips as Humanoid | Prop/actor height ratio 0.437 at local identity | IP/RM convention present | Representative playback passes | Strong candidate; two block exceptions and visuals open. |
| 1-Hand ↔ Basic Locomotion | Majority structure matches; shared paths identical | Shared avatar assets identical | Full-body state owns movement | Idle mixer and attack mask execute | Technical candidate; visual state/mask acceptance open. |
| 1-Hand ↔ Sword & Shield | Majority structure matches; shared paths identical | Props differ; grips unreviewed | One active weapon mode owns root | Idle mixer executes | Prefer full-body weapon-mode switch. |
| 1-Hand attack ↔ Basic mask | Unity Humanoid mapping accepted | Right-hand prop attaches | Basic base owns movement | Headless mask executes | Prototype only; pelvis/contact/arc open. |
| Pack ↔ project character | No target character | Not evaluated | Project policy unknown | Not evaluated | Unknown. |

## Limitations and unknowns

1. No target character, camera, controller, quality bar, combat design, hit-window specification, or networking policy was supplied.
2. Headless Unity proves import/execution, not pose quality, feet, deformation, grip, weapon contacts, root behavior, or perceived timing.
3. The 24 refreshed gait-anchor GLBs were not imported because the Unity project had no GLB importer; Unreal Engine, Godot, and Bevy remain documentation-only.
4. Delivered loop metadata is not reliable author intent; strict failures do not mean every flagged clip visibly fails in-game.
5. `_RM` is naming evidence; yaw and short action displacement need independent review.
6. Current public pages/EULA do not prove the local revision, transaction date, or historical terms.

## Reproduction

Source identity: RAR SHA-256 `c2f96f012eed84671dd3261017cc1cfd58b991b030f43bf4e9844c9366f1776e`; Unitypackage SHA-256 `e773abaedd2b78d75288aa20a62dfe1c9eb2c9fb0a66e163ce5c8dcf8236ac24`. Gait remediation used pre-release 0.3.1 code: `animsmith 0.3.0 (v0.3.0-39-g674396f)`, revision `674396f0f53b10c4344e7315a5756fe5ef71b469`, binary SHA-256 `7744b71580e04d80f9e5738efce76e0295323ccb3150fa57b0ad9b37c5ff1513`. Baseline and contracts remain captured at `b7c215ba259b87b4b4e46567452a037a34be7308`.

```text
animsmith inspect --config config/baseline.animsmith.toml <input.fbx>
animsmith measure --config config/baseline.animsmith.toml --format json <input.fbx>
animsmith lint --config config/contracts/<file>.animsmith.toml --format json <input.fbx>
animsmith report --config <config> <input.fbx> --output <report.html>
animsmith transform --config <config> <input.fbx> -o <output.glb> --gait-anchor
animsmith transform --config <config> <input.fbx> -o <output.glb> --prune-constant-tracks
```

Retained summaries: historical baseline `505cb1c323bba8c259eaf0f88651bbfaa75dd1273563740817cf7f99910c30c0`; contracts `e97e2d44bd79d4d3c3e5fde3cfbe715b6f3bbbcda4bdef8053b17503e007e749`; historical refusal-era remediation `3f7283e68b3d53109ed48c71309d4f13d5cebc3d8d971bd2ecd0422ea6e734d2`; current remediation commands `6f8717ec84797cbb89d830e7098baf529fdc6f37eb1b877775a0f6c5d514c5d6`; current combined summary `118116c9173df4e3e782cdfe3b712deb9fb14cec23c8e0e75cd484e8156d7f4b`; Unity probe `1c147ff6683833ba28c1db210d58aee65140ac232311f370782c28c3925ae62d`.

## Sources

- Local source archive and extracted Unity metadata — private authorized input identified above, accessed 2026-08-17.
- Protofactor, [1-Handed Melee Weapon](https://protofactor.biz/product/animset-1-handed-melee-weapon/), [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/), and [End User License Agreement](https://protofactor.biz/end-user-license-agreement/) — current listing/license context, not local revision proof.
- Unity, [Avatar Mask](https://docs.unity3d.com/6000.0/Documentation/Manual/class-AvatarMask.html), [Animation Layers](https://docs.unity3d.com/6000.0/Documentation/Manual/AnimationLayers.html), and [Root Motion](https://docs.unity3d.com/6000.5/Documentation/Manual/RootMotion.html) — runtime capabilities.
- Epic Games, [Root Motion](https://dev.epicgames.com/documentation/unreal-engine/root-motion-in-unreal-engine?application_version=5.7) and [Layered Animations](https://dev.epicgames.com/documentation/unreal-engine/using-layered-animations-in-unreal-engine?lang=en-US); Godot, [AnimationTree](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html); Bevy, [Animation Masks](https://bevy.org/examples/animation/animation-masks/) — documentation-only capability.
- AnimSmith issues [#401](https://github.com/mmannerm/animsmith/issues/401), [#402](https://github.com/mmannerm/animsmith/issues/402), [#408](https://github.com/mmannerm/animsmith/issues/408), and [#411](https://github.com/mmannerm/animsmith/issues/411) — optimization, root, and speed follow-up; merged [#426](https://github.com/mmannerm/animsmith/issues/426) — delivered vertical gait-heading support used by the current trial.
