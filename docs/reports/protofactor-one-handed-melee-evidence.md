# Animation pack evidence appendix: Protofactor 1-Handed Melee Weapon Animset

> Companion report: [technical evaluation](protofactor-one-handed-melee.md)
>
> Evidence status: **partial** — exact AnimSmith 0.7.0 baseline, contracts, remediation verification, addressability, and bounded advice plus a dated Unity 6000.5.8f1 observation; transformed-clip visual acceptance and engine-editor/runtime passes remain unevaluated.
>
> Evaluation date: **2026-08-26**
>
> Current evaluator: **AnimSmith 0.7.0**
>
> Report format: **2**

This appendix preserves the detailed evidence behind the concise report. The [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder) remains authoritative.

## Evaluation scope and provenance

| Field | Value |
|---|---|
| Pack/edition | Local `Animset@1HandedMeleeWeapon_PACKAGE.unitypackage`; constituent revision is not declared |
| Vendor/source | Protofactor; [current 1-Handed Melee product page](https://protofactor.biz/product/animset-1-handed-melee-weapon/) |
| Delivered scope | Full local RAR → one Unitypackage → 113 FBXs: 110 individual motions, one combined animation FBX, one skinned actor, and one bludgeon prop; Unity metadata/materials/textures included |
| Target use | Game-engine use; third-person one-handed combat and combination with the other seven evaluated constituents |
| Target engines | Unity 6000.5.8f1 observed (retained, 2026-08-17) plus the current evaluation `unity-humanoid`/`unreal`/`godot`/`bevy` engine-profile advice (2026-08-21); no Unreal/Godot/Bevy import |
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
| Root motion | `evaluated-finding` | Paired labels, horizontal speed, and) per-clip root-trajectory measured on 112/112 clips; movement-ownership axis intent stays open and is never inferred from measured travel. |
| Conform | `evaluated-finding` | 56-bone majority plus two Unity Generic block exceptions. |
| Validate | `partially-evaluated` | Mechanical contracts complete on the current evaluator (loop-seam 73/110 evaluated; 37 no-stride/stationary clips `not_evaluated`); visual combat acceptance open. |
| Optimize | `evaluated-finding` | Twenty-four current gait candidates and one pruning candidate generated; runtime/equivalence acceptance remains open. |
| Export | `partially-evaluated` | Generated GLBs are evidence only; a new-project GLB import test confirms all 24 gait candidates load as Generic clips (see Engine procedures and evidence), but native Humanoid-retarget engine exports are not accepted. |
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
| Default mechanical pass with constant-track notes | 113/113 FBXs complete the baseline commands; 13,629 notes in 112 animated files, all lint exits 0, no error-severity findings | Export bloat; no default hard blocker. | `observed-animsmith`; current baseline summary. |
| Declared loop closure/derivatives | 87/110 contracts fail; 52 closure, 87 rotation, 87 velocity file failures | Pops/pulses if delivered loop flags are trusted. | `observed-animsmith`; current contract summary (110 linted, 23 exit 0), exactly reproducing published 87/110 failures. |
| Loop-seam availability | 93/110 applicable, 17 not_applicable; 73/110 evaluation complete, 37 `not_evaluated` | Most not-evaluated results are no-stride/stationary clips. | `observed-animsmith`; current availability counts. |
| Unity Generic block files | 2/110 | Required block reaction/idle cannot share the Humanoid combat graph. | `observed-engine`; four-outlier probe includes both, unchanged. |
| Directional gait phase | Three IP/RM rings | Same-time blends can skate despite paired IP/RM agreement. | Current raw phase spreads are 0.554–0.734. |
| Root trajectory | 112/112 clips | Enables per-clip movement/yaw review without inventing an ownership axis. | `observed-animsmith`; 39 moving >1 cm, 72 stationary, 0 with >1° yaw, `heading_axis`=`positive_y` on 111/111 — sampled shared-grid regression facts, not extraction proof. |
| RM speed variation | Walk 1.94×; run 1.11×; crouch 1.52× | Direction-dependent controller velocity. | `observed-animsmith`; primary table. |

## AnimSmith remediation evidence

| Source issue | Operation/declarations | Result | Independent verification | Remaining caveat |
|---|---|---|---|---|
| Missing semantic loop/in-place context | Per-file `[clips."Take 001"]` declarations derived from Unity loop metadata and observed IP/RM pairs | 23 pass; 87 fail, exposing raw loop policy — reproduced exactly on the current evaluator. | JSON and Markdown results agree for all 110. | Delivered one-shot loop intent still needs curation. |
| Ring phase disagreement | Current `transform --gait-anchor` on 24 core IP files | 24/24 exit 0 and emit GLBs. | Inspect/measure/fix dry-run 24/24 exit 0; post circular spreads Crouch 0.0394317, Run 0.1081981, Walk 0.0639032; lint/diff retain remaining contract findings and intentional rewrite deltas. | Only IP transformed; all 24 load as Generic clips, but Humanoid-retarget, visual/contact, and trajectory acceptance remain open. |
| Dense constant tracks | `transform --prune-constant-tracks` on `Humanoid@IdleCombat1hMelee.fbx` | One candidate GLB produced; source never modified. | Inspect/measure and fix dry-run exit 0; diff/lint retain expected semantic differences/findings. | Bounded by open [#401](https://github.com/mmannerm/animsmith/issues/401); runtime equivalence and sparse transition behavior unproved. |

Only in-place members were transformed. The current evaluator measures the rig heading basis as `positive_y`, emits every selected gait candidate, and leaves RM-labelled inputs untouched. Candidates remain unpromoted pending engine and visual acceptance.

## Engine procedures and evidence

| Runtime | Version | Procedure | Observed result | Remaining gate |
|---|---|---|---|---|
| Unity | 6000.5.8f1 (retained, 2026-08-17) | Fresh eight-pack project; import all delivered models; inventory importer/avatar/clip state; sample six clips; run Basic/Sword mixers, a Humanoid upper-body mask, and right-hand prop attachment. | 108/110 individual Humanoid clips; six samples, two mixers, mask, and prop pass; both expected Generic outliers fail. The observation remains dated; its source hash matches the evaluated artifact. | A separate new-project GLB import test covers gait-candidate loading (see below); visual graph, contacts, Humanoid retarget of the GLB candidates, target retarget, compression/build remain open. |
| Unity | Dated Unity 6000.5.8f1 headless import/Playables observation plus current Unity Humanoid revision-1 projection | Import and representative graph execution succeeded for the delivered FBXs; current settings projection is available | Visual playback, target retarget, contacts, full graphs, compression, and build remain open |
| Unity GLB import | 6000.5.8f1 with glTFast 6.9.0 | Load current candidates in a disposable project | Every tested candidate produces one Generic clip | Humanoid retarget, playback, and visual acceptance remain open |
| Unreal Engine | `unreal` rev 2 / 5.8 | `generate import-advice` under the exact revision-2 `unreal`/UE 5.8/`fbx-importer` profile. | Current revision-2 settings projection is available; no engine process ran. | Import/retarget/graphs/contacts/build once settings are modeled. |
| Godot | `godot` rev 2 / 4.7 | `generate import-advice` under the exact revision-2 `godot`/Godot 4.7/`resource-importer-scene` profile. | Current revision-2 settings projection is available; no engine process ran. | Conversion/import, retarget, graphs, contacts/export once settings are modeled. |
| Bevy | `bevy` rev 3 / 0.19.0 | Current rich addressability against a generated GLB candidate | Available with 64-bit target UUIDs; sealed inventory only | Target survival, retarget path, graph wiring, root motion, actual Bevy load/playback. |

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
3. Current Unreal revision-2 and Godot revision-2 settings projections are available, but neither engine received an import or playback test.
4. Delivered loop metadata is not reliable author intent; strict failures do not mean every flagged clip visibly fails in-game.
5. `_RM` is naming evidence; per-clip root-trajectory measurement covers horizontal travel and yaw on 112/112 clips, but these are sampled regression facts on the shared metric grid, not continuous-curve or engine root-motion extraction proof — never declare a movement-ownership axis from measured travel alone.
6. A direct Unity 6000.5.8f1 headless sample of 120 clips observed baked root rotation, baked XZ for nearly every in-place clip, and extracted XZ for most root-motion clips. This is sampled importer evidence, not visual or gameplay acceptance.
7. Current public pages/EULA do not prove the local revision, transaction date, or historical terms.
8. The integration recipe's `owner=validate-per-axis` step directs the reader to validate root-motion ownership axis by axis rather than assume it. The observed Unity importer locks bake root rotation on every sampled root-motion clip, so animation cannot be assumed to own root-motion yaw. The step is not a per-axis `movement_owner_xz` / `movement_owner_y` / `movement_owner_yaw` declaration, and no such declaration is derived from measured travel in this refresh. Measured root displacement and yaw are recorded as sampled facts only; choosing the per-axis owner remains a project and engine decision. The 2026-08-21 observed Unity root-lock policy (Engine procedures and evidence) independently corroborates this split — this pack's own sampled in-place clips all bake `lockRootPositionXZ` while its sampled `_RM` clips all extract it — but that is corroborating engine evidence for the recipe, not a licence to declare per-axis `movement_owner_*` values, and none is declared here.
9. The new-project GLB import test (134/134 candidates, 24 from this pack) proves glTFast produces one well-formed Generic AnimationClip per candidate; it does not test this pack's Humanoid retarget path and is not visual or gameplay acceptance.

## Changes between AnimSmith versions

| Evaluator | Change from the preceding evaluated state |
|---|---|
| AnimSmith 0.7.0 | Revalidated the 113-FBX baseline, 110 declared contracts, 24 gait candidates, pruning trial, and current engine projections under output v17 / measurements v16. |
| AnimSmith 0.4.1 | Reproduced the evaluated 0.4.0 results for this corpus; unrelated release fixes did not change the pack conclusion. |
| AnimSmith 0.4.0 | Consolidated baseline, contract, and gait evidence on one released evaluator and retained the same post-anchor phase results. |
| AnimSmith 0.3.x | Established the initial baseline and first gait-remediation trial. Those evaluator results are superseded. |

## Reproduction

### Current AnimSmith reproduction (2026-08-26)

The unchanged corpus was rerun with `animsmith 0.7.0`, exact tag `v0.7.0`, commit `461ac8a4f6bb368eb8637471a796f13eeb647140`, binary SHA-256 `01a501999c91d93abfb32b1f48241fccc70914fac27c9a650c31df44262578d8`, output schema v17, and measurements schema v16.

| Current external evidence | SHA-256 | Result |
|---|---|---|
| Baseline command envelope | `94a35ceaa33ded273c01ff002d4959fc60fcaf8a77d9b561ae88519a6694a475` | 113 FBXs; all commands complete |
| Declared contracts | `3cf7c419a46224a94514f7fbf6b753f71ad4abaa09e6ca63506a9b53745f7fca` | 110 files; 23 pass / 87 fail |
| Remediation | `a4fa16a295b6f9eedde392bb1f88b4f1d1b1b54f965e400df3e3765a4bdd5b08` | 25 candidates completed and verified |
| 0.7 supplemental projections | `846a2456f3f5d01c39fd18d15807cad18c465ac06eeaad242977c8055d31b477` | 25 addressability V1 + rich V2 pairs; exact-profile advice available |
| Refreshed legacy manifest | `71846e7671a298010c3e09876a237ff765297822ae1fcd6b145f1c3a17672c81` | Valid schema; 72 logical motions |

The current projections do not evaluate weapon contact, runtime graph wiring, target survival, retarget deformation, or visual acceptance.

## Sources

- Local source archive and extracted Unity metadata — private authorized input identified above, accessed 2026-08-17; re-inventoried 2026-08-21.
- Protofactor, [1-Handed Melee Weapon](https://protofactor.biz/product/animset-1-handed-melee-weapon/), [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/), and [End User License Agreement](https://protofactor.biz/end-user-license-agreement/) — current listing/license context, not local revision proof.
- Unity, [Avatar Mask](https://docs.unity3d.com/6000.0/Documentation/Manual/class-AvatarMask.html), [Animation Layers](https://docs.unity3d.com/6000.0/Documentation/Manual/AnimationLayers.html), and [Root Motion](https://docs.unity3d.com/6000.5/Documentation/Manual/RootMotion.html) — runtime capabilities.
- Epic Games, [Root Motion](https://dev.epicgames.com/documentation/unreal-engine/root-motion-in-unreal-engine?application_version=5.7) and [Layered Animations](https://dev.epicgames.com/documentation/unreal-engine/using-layered-animations-in-unreal-engine?lang=en-US); Godot, [AnimationTree](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html); Bevy, [Animation Masks](https://bevy.org/examples/animation/animation-masks/) — documentation-only capability.
