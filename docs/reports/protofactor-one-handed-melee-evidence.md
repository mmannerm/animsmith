# Animation pack evidence appendix: Protofactor 1-Handed Melee Weapon Animset

> Companion report: [technical evaluation](protofactor-one-handed-melee.md)
>
> Evidence status: **partial** — exhaustive AnimSmith `v0.4.0` baseline, contract, and gait/pruning remediation on one frozen released evaluator, plus a retained (dated) Unity 6000.5.8f1 eight-pack probe and new 0.4.0 engine-profile advice/refusal evidence; transformed-clip visual acceptance and full Unreal/Godot/Bevy import remain unevaluated.
>
> Evaluation date: **2026-08-21**
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
| Target engines | Unity 6000.5.8f1 observed (retained, 2026-08-17) plus 0.4.0 `unity-humanoid`/`unreal`/`godot`/`bevy` engine-profile advice (2026-08-21); no Unreal/Godot/Bevy import |
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
| Root motion | `evaluated-finding` | Paired labels, horizontal speed, and (new in 0.4.0, shipped/closed [#408](https://github.com/mmannerm/animsmith/issues/408)) per-clip root-trajectory measured on 112/112 clips; movement-ownership axis intent stays open and is never inferred from measured travel. |
| Conform | `evaluated-finding` | 56-bone majority plus two Unity Generic block exceptions. |
| Validate | `partially-evaluated` | Mechanical contracts complete on 0.4.0 (loop-seam 73/110 evaluated; 37 no-stride/stationary clips correctly `not_evaluated` rather than mislabelled); visual combat acceptance open. |
| Optimize | `evaluated-finding` | Twenty-four 0.4.0 gait candidates (reproducing pre-release `674396f` to seven decimal places) and one pruning candidate generated; runtime/equivalence acceptance remains open. |
| Export | `partially-evaluated` | Generated GLBs are evidence only; native engine exports are not accepted. |
| Gate/report | `evaluated-clean` | Manifest and report pair use parser validation. |

### Readiness evidence by clip set

| Role or runtime set | File-ready / clip-ready | Set-ready / rig-use | Runtime / acceptance boundary |
|---|---|---|---|
| Three locomotion rings | 48/48 parse; delivered loop contracts expose seam failures. | Raw spreads 0.554/0.734/0.714; 0.4.0 IP candidates 0.064/0.108/0.039 (reproduces `674396f` to 7 decimals). | Keep RM raw; transformed IP engine/visual acceptance and residual offsets remain. |
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
| Default mechanical pass with constant-track notes | 113/113 FBXs complete the baseline commands; 13,629 notes in 112 animated files, all lint exits 0, no error-severity findings | Export bloat; no default hard blocker. | `observed-animsmith`; 0.4.0 baseline summary, identical to published 0.3.0. |
| Declared loop closure/derivatives | 87/110 contracts fail; 52 closure, 87 rotation, 87 velocity file failures | Pops/pulses if delivered loop flags are trusted. | `observed-animsmith`; 0.4.0 contract summary (110 linted, 23 exit 0), exactly reproducing published 87/110 failures. |
| Loop-seam availability | 93/110 applicable, 17 not_applicable; 73/110 evaluation complete, 37 correctly `not_evaluated` | No-stride/stationary clips no longer mislabelled pass/fail. | `observed-animsmith`; 0.4.0 availability recount. |
| Unity Generic block files | 2/110 | Required block reaction/idle cannot share the Humanoid combat graph. | `observed-engine`; four-outlier probe includes both, unchanged. |
| Directional gait phase | Three IP/RM rings | Same-time blends can skate despite paired IP/RM agreement. | 0.4.0 baseline reproduces the historical `b7c215b` result; raw phase spreads 0.554–0.734. |
| Root trajectory (new in 0.4.0) | 112/112 clips | Enables per-clip movement/yaw review without inventing an ownership axis. | `observed-animsmith`; 39 moving >1 cm, 72 stationary, 0 with >1° yaw, `heading_axis`=`positive_y` on 111/111 — sampled shared-grid regression facts, not extraction proof. |
| RM speed variation | Walk 1.94×; run 1.11×; crouch 1.52× | Direction-dependent controller velocity. | `observed-animsmith`; primary table. |

## AnimSmith remediation evidence

| Source issue | Operation/declarations | Result | Independent verification | Remaining caveat |
|---|---|---|---|---|
| Missing semantic loop/in-place context | Per-file `[clips."Take 001"]` declarations derived from Unity loop metadata and observed IP/RM pairs | 23 pass; 87 fail, exposing raw loop policy — reproduced exactly on 0.4.0. | JSON and Markdown results agree for all 110. | Delivered one-shot loop intent still needs curation. |
| Ring phase disagreement | Released `v0.4.0` `transform --gait-anchor` on 24 core IP files | 24/24 exit 0 and emit GLBs. | Inspect/measure/fix dry-run 24/24 exit 0; post circular spreads Crouch 0.7135886→0.0394317, Run 0.7341757→0.1081981, Walk 0.5537969→0.0639032 — match the pre-release `674396f` after-values to seven decimal places; lint/diff 24/24 exit 1 for remaining contracts/semantic rewrites. | Only IP transformed; no engine import was available this session (the retained Unity project has no GLB importer); no visual/contact or trajectory acceptance; residual offsets remain. |
| Dense constant tracks | `transform --prune-constant-tracks` on `Humanoid@IdleCombat1hMelee.fbx` | One candidate GLB produced; source never modified. | Inspect/measure and fix dry-run exit 0; diff/lint retain expected semantic differences/findings. | Bounded by open [#401](https://github.com/mmannerm/animsmith/issues/401); runtime equivalence and sparse transition behavior unproved. |

The earlier `b7c215b` heading-basis refusals remain historical only. Pre-release `674396f` first implemented the merged and now-closed [#426](https://github.com/mmannerm/animsmith/issues/426) vertical-heading-basis policy. Released **AnimSmith `v0.4.0`** (commit `6b37ad636b198ef8ff47fadbf6a3a51eb1a27c8e`, binary SHA-256 `fd1eee57407aa02db88763d144389a7f5104204c40ddfbb28eb5885ca8cd54c6`, captured 2026-08-21) reproduces all 24 IP candidates and their post-anchor circular phase spreads to seven decimal places, so the gait result no longer rests on an unreleased build. No RM file was transformed, and no engine import was available this session, so lower spreads remain mechanical evidence rather than set-ready or visual acceptance. All generated candidates remain outside the repository with the commercial inputs.

## Engine procedures and evidence

| Runtime | Version | Procedure | Observed result | Remaining gate |
|---|---|---|---|---|
| Unity | 6000.5.8f1 (retained, 2026-08-17) | Fresh eight-pack project; import all delivered models; inventory importer/avatar/clip state; sample six clips; run Basic/Sword mixers, a Humanoid upper-body mask, and right-hand prop attachment. | 108/110 individual Humanoid clips; six samples, two mixers, mask, and prop pass; both expected Generic outliers fail. Kept at its original date/attribution because the source is byte-identical. | Add a GLB importer or convert candidates, then test gait outputs; visual graph, contacts, root motion, target retarget, compression/build. |
| Unity | `unity-humanoid` rev 1 / 6000.3 (new, 2026-08-21) | `generate import-advice` under the exact revision-1 `unity-humanoid`/Unity 6000.3/`fbx-model-importer` profile against delivered `.fbx.meta` for every clip. | Exit 0, advice available. No delivered `.meta` sets `lockRootRotation`/`lockRootHeightY`/`lockRootPositionXZ`; AnimSmith assumes an absent key takes Unity's serialized default (`false`) and derives root rotation/Y/XZ=`extract` for every clip. This is a 6000.3 advice-only assumption, not observed 6000.5.8f1 importer behavior. | Confirm against actual 6000.5.8f1 (or 6000.3) importer behavior; author explicit per-clip bake/extract settings distinguishing idles from locomotion. |
| Unreal Engine | `unreal` rev 1 / 5.8 | `generate import-advice` under the exact revision-1 `unreal`/UE 5.8/`fbx-importer` profile. | Typed refusal `profile_settings_unmodeled` (exit 1): engine settings are not yet modeled by this profile; no pack import attempted. | Import/retarget/graphs/contacts/build once settings are modeled. |
| Godot | `godot` rev 1 / 4.7 | `generate import-advice` under the exact revision-1 `godot`/Godot 4.7/`resource-importer-scene` profile. | Typed refusal `profile_settings_unmodeled` (exit 1): engine settings are not yet modeled by this profile; no pack import attempted. | Conversion/import, retarget, graphs, contacts/export once settings are modeled. |
| Bevy | `bevy` rev 1 / 0.19.0 | `generate addressability` under the exact revision-1 `bevy`/0.19.0/`gltf-asset-loader` profile against a generated GLB candidate. | Exit 0: one animation row, complete source coverage, predicted selector `Animation0`, facet state available, 0 findings — inventory/selector prediction only. | FBX→glTF conversion, retarget path, graph wiring, root motion, actual Bevy load/playback. |

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
3. The 24 gait-anchor GLBs were not imported because no engine import was available this session and the retained Unity project has no GLB importer; Unreal Engine and Godot's 0.4.0 profiles return typed `profile_settings_unmodeled` refusals rather than import, and Bevy's addressability pass only predicts a selector on a generated GLB — none is playback or visual acceptance.
4. Delivered loop metadata is not reliable author intent; strict failures do not mean every flagged clip visibly fails in-game.
5. `_RM` is naming evidence; per-clip root-trajectory measurement (new in 0.4.0) covers horizontal travel and yaw on 112/112 clips, but these are sampled regression facts on the shared metric grid, not continuous-curve or engine root-motion extraction proof — never declare a movement-ownership axis from measured travel alone.
6. The 0.4.0 Unity `unity-humanoid` import-advice derives root rotation/Y/XZ=`extract` for every clip because no delivered `.fbx.meta` sets the lock flags; this assumes an absent key takes Unity's serialized default, targets 6000.3, and is not observed 6000.5.8f1 behavior.
7. Current public pages/EULA do not prove the local revision, transaction date, or historical terms.

## Reproduction

Source identity: RAR SHA-256 `c2f96f012eed84671dd3261017cc1cfd58b991b030f43bf4e9844c9366f1776e`; Unitypackage SHA-256 `e773abaedd2b78d75288aa20a62dfe1c9eb2c9fb0a66e163ce5c8dcf8236ac24`; logical manifest SHA-256 `aa36db891581ef7fb6e35cfff781958ca67afc1f43c403327ad648832c483f17`. A 2026-08-21 re-inventory reproduces the published manifest exactly: 0 added, 0 removed, 0 changed across all 113 FBXs.

This 0.4.0 refresh runs the baseline, contract pass, and gait/pruning remediation on one frozen released evaluator: `animsmith 0.4.0`, tag `v0.4.0`, revision `6b37ad636b198ef8ff47fadbf6a3a51eb1a27c8e`, binary SHA-256 `fd1eee57407aa02db88763d144389a7f5104204c40ddfbb28eb5885ca8cd54c6`, captured 2026-08-21, output schema v10 / measurements schema v15. This replaces the earlier split between the `b7c215ba259b87b4b4e46567452a037a34be7308` baseline/contract capture and the pre-release-0.3.1 gait-only pass (`animsmith 0.3.0 (v0.3.0-39-g674396f)`, revision `674396f0f53b10c4344e7315a5756fe5ef71b469`, binary SHA-256 `7744b71580e04d80f9e5738efce76e0295323ccb3150fa57b0ad9b37c5ff1513`); that mixed identity is retained here only as historical comparison evidence, since the 24 IP gait candidates reproduce its post-anchor circular phase spreads to seven decimal places.

```text
animsmith inspect --config config/baseline.animsmith.toml <input.fbx>
animsmith measure --config config/baseline.animsmith.toml --format json <input.fbx>
animsmith lint --config config/contracts/<file>.animsmith.toml --format json <input.fbx>
animsmith report --config <config> <input.fbx> --output <report.html>
animsmith transform --config <config> <input.fbx> -o <output.glb> --gait-anchor
animsmith transform --config <config> <input.fbx> -o <output.glb> --prune-constant-tracks
```

Retained summaries (historical, from the superseded mixed-version evaluation): baseline `505cb1c323bba8c259eaf0f88651bbfaa75dd1273563740817cf7f99910c30c0`; contracts `e97e2d44bd79d4d3c3e5fde3cfbe715b6f3bbbcda4bdef8053b17503e007e749`; refusal-era remediation `3f7283e68b3d53109ed48c71309d4f13d5cebc3d8d971bd2ecd0422ea6e734d2`; pre-release remediation commands `6f8717ec84797cbb89d830e7098baf529fdc6f37eb1b877775a0f6c5d514c5d6`; pre-release combined summary `118116c9173df4e3e782cdfe3b712deb9fb14cec23c8e0e75cd484e8156d7f4b`. Unity probe (retained, dated 2026-08-17, unchanged because the source is byte-identical): `1c147ff6683833ba28c1db210d58aee65140ac232311f370782c28c3925ae62d`.

## Sources

- Local source archive and extracted Unity metadata — private authorized input identified above, accessed 2026-08-17; re-inventoried 2026-08-21.
- Protofactor, [1-Handed Melee Weapon](https://protofactor.biz/product/animset-1-handed-melee-weapon/), [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/), and [End User License Agreement](https://protofactor.biz/end-user-license-agreement/) — current listing/license context, not local revision proof.
- Unity, [Avatar Mask](https://docs.unity3d.com/6000.0/Documentation/Manual/class-AvatarMask.html), [Animation Layers](https://docs.unity3d.com/6000.0/Documentation/Manual/AnimationLayers.html), and [Root Motion](https://docs.unity3d.com/6000.5/Documentation/Manual/RootMotion.html) — runtime capabilities.
- Epic Games, [Root Motion](https://dev.epicgames.com/documentation/unreal-engine/root-motion-in-unreal-engine?application_version=5.7) and [Layered Animations](https://dev.epicgames.com/documentation/unreal-engine/using-layered-animations-in-unreal-engine?lang=en-US); Godot, [AnimationTree](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html); Bevy, [Animation Masks](https://bevy.org/examples/animation/animation-masks/) — documentation-only capability.
- AnimSmith issues, verified live 2026-08-21: [#401](https://github.com/mmannerm/animsmith/issues/401) (open) — property-scoped constant-track pruning; [#411](https://github.com/mmannerm/animsmith/issues/411) (open) — cross-set root-speed/stride coherence; [#402](https://github.com/mmannerm/animsmith/issues/402) (closed) — shipped per-clip channel-coverage measurement; [#408](https://github.com/mmannerm/animsmith/issues/408) (closed) — shipped per-clip root displacement/yaw measurement; [#426](https://github.com/mmannerm/animsmith/issues/426) (closed) — shipped vertical gait-heading-basis support, now part of released 0.4.0's gait-anchor trial.
