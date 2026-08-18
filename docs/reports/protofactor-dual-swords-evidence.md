# Animation pack evidence appendix: Protofactor Dual Swords Animset

> Companion report: [technical evaluation](protofactor-dual-swords.md)
>
> Evidence status: **partial** — exhaustive 0.3.0 baseline/contracts, 0.3.1-bound gait remediation at `674396f`, and a Unity 6000.5.8f1 probe; transformed-engine/visual acceptance and three engines remain unevaluated.
>
> Evaluation date: **2026-08-17**
>
> Report format: **1**

This appendix preserves the detailed evidence behind the concise report. The [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder) remains authoritative.

## Evaluation scope and provenance

| Field | Value |
|---|---|
| Pack/edition | Local `Animset@DualSwords_PACKAGE.unitypackage`; constituent revision is not declared |
| Vendor/source | Protofactor; [current Dual Swords product page](https://protofactor.biz/product/animset-dual-swords/) |
| Delivered scope | Full local RAR → one Unitypackage → 189 FBXs: 186 individual motions, one combined animation FBX, one skinned actor, and one sword prop; animation list, Unity metadata, materials, and textures included |
| Target use | Game-engine use; third-person dual-wield combat and combined use with the seven previously evaluated constituents |
| Target engines | Unity 6000.5.8f1 observed; Unreal Engine, Godot, and Bevy documentation-only |
| Target rigs/packs | Supplied Protof-Actor, Basic Locomotion, Sword & Shield, and the evaluated collection subset |
| Source manifest | `logical-assets-inventory.json`; SHA-256 `d7a3e3d88ba5f93c04d20d9ba8c316e667a5e27a132ef8fa33ebda5339d1e535` |
| Evaluation manifest | `evidence/evaluation-manifest.json`; SHA-256 `57e90445c1cb11f80506c5d551c2426ef45bf421a38e45dab3fcd928c79fbd21`; schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1`; taxonomy/profile-set version 1; baseline evaluator b7 |
| Acquisition/license provenance | `user-stated`: local archive was downloaded from Protofactor.biz as part of the Ultimate Animation Collection. Current EULA permits protected released real-time applications and modification while restricting redistribution/resale; no receipt, download date, historical EULA, or local constituent revision was retained. Technical due diligence only, not legal advice. |

The current product page, observed 2026-08-17, advertises USD 24.99, 185 animations, 79 root-motion and 106 in-place files, Unity Humanoid, Unity 2019.4+, and no native UE4 package. The local delivery has 186 individual files and 76 `_RM` labels, so the current listing does not identify the local constituent revision or explain either count difference.

The evaluation manifest uses schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1`.

### Evidence coverage

| Surface | Offered/delivered | Evaluated | Findings | Not evaluated and why |
|---|---:|---:|---:|---|
| Animation files | 187 animation FBXs | 187 | 186 individual plus one combined take; actor and prop are support assets | Continuous artistic review of all motion |
| Rigs/export variants | 3 observed structures | 3 | Individual files share one 56-bone signature; actor/combined 58; prop 3 | Target-character deformation and non-Unity retarget |
| AnimSmith baseline | 189 FBXs | 189 | b7 commands exit 0; 25,426 constant-track notes | Not rerun at 674396f; artistic intent and contacts |
| Declared contracts | 186 individual files | 186 | b7: 24 pass, 162 fail under delivered/inferred declarations | Not rerun at 674396f; human loop intent for every action |
| Offline visual reports | 186 possible | 4 risk-selected | Reports generated for idle, locomotion, attack, and combo | Reports were not used as artistic acceptance |
| Engine import/playback | 186 individual files in Unity | 186 | All Humanoid; seven required samples pass | Full graph, visual root motion, compression, build |
| Blend/mask/retarget | 3 collection graphs + 2 prop attachments | 3 + 2 | Basic and Sword mixers, Basic mask, and both hand attachments execute | Visual blending, grips, target retarget, IK |

### Claim legend

Evidence labels follow the versioned taxonomy: `user-stated`, `observed-file`, `observed-animsmith`, `observed-report`, `observed-engine`, `vendor-stated`, `documentation-stated`, `inferred`, and `not-evaluated`.

## Evaluation manifest and taxonomy

### Canonical clip-role inventory

| Canonical primary role | Logical motions | Delivered files | Evidence boundary |
|---|---:|---:|---|
| `idle-pose` | 6 | 6 | Filename/clip inspection; Unity Humanoid import observed. |
| `continuous-locomotion` | 35 | 70 | Paired filenames, timing, horizontal speed, and gait measured. |
| `locomotion-transition` | 0 | 0 | No promoted locomotion-transition role. |
| `airborne` | 0 | 0 | Not delivered. |
| `traversal` | 0 | 0 | Not delivered. |
| `action-interaction` | 51 | 86 | Combat/equipment semantics inferred; contacts/events absent. |
| `reaction-death` | 20 | 24 | Reaction/death naming; gameplay timing unaccepted. |
| `emote-cinematic` | 0 | 0 | Taunts were conservatively retained with combat actions in this bounded manifest. |
| `other-unknown` | 0 | 0 | None after bounded classification. |
| **Total** | **112** | **186** | Validated v1 manifest identified above. |

### Runtime-set inventory

| Runtime set | Type | Members/variants | Grouping evidence | Validation status |
|---|---|---|---|---|
| Walk combat 8-way | directional-blend | 8 IP + 8 RM | Exact names, common duration, directions, and speeds; see [primary table](protofactor-dual-swords.md#runtime-sets-and-authored-motion). | Raw measured; current IP candidate spread 0.053; engine/visual acceptance open. |
| Run combat 8-way | directional-blend | 8 IP + 8 RM | Exact names, common duration, and near-uniform speed. | Raw measured; current IP candidate spread 0.135; engine/visual acceptance open. |
| Crouch combat 8-way | directional-blend | 8 IP + 8 RM | Exact names, common duration, and near-uniform speed. | Raw measured; current IP candidate spread 0.059; engine/visual acceptance open. |
| Forward speed alternatives | speed-blend | walk-1/walk-2/jog/jog-fast/run-fast IP + RM | Filename order and measured 0.847–4.588 m/s RM speeds. | Threshold candidate; visual blend open. |
| Draw/combat/put-away | transition-chain | 3 single files | Delivered equipment verbs and combat idle. | Unity draw sample passes; endpoint/visibility timing open. |
| Combo alternatives | other | 19 IP/RM logical choices, 38 files | Numbered two- through five-hit filenames and paired files. | Inventory candidate; hit/branch/cancel contract absent. |
| Single-attack alternatives | other | 11 IP/RM logical choices, 22 files | Numbered attack filenames and paired files. | Discrete-action candidate; hit/contact contract absent. |

### Pipeline-stage coverage

| Stage | Coverage state | Evidence / remaining gate |
|---|---|---|
| Acquire | `evaluated-clean` | Local archive identified and hashed outside the repository. |
| Preserve raw | `evaluated-clean` | RAR and Unitypackage retained unchanged. |
| Inspect | `evaluated-finding` | Every FBX inspected/measured/linted. |
| Segment | `partially-evaluated` | Individual FBXs used; combined take not promoted. |
| Root motion | `evaluated-finding` | Paired labels and horizontal speed measured; yaw intent open. |
| Conform | `evaluated-clean` | All individual files share one 56-bone structure and import as Unity Humanoid. |
| Validate | `partially-evaluated` | Mechanical contracts complete; visual combat acceptance open. |
| Optimize | `evaluated-finding` | 24 current gait candidates and one pruning candidate generated; runtime equivalence open. |
| Export | `partially-evaluated` | Current GLBs inspect/measure cleanly but lack engine/visual acceptance. |
| Gate/report | `evaluated-clean` | Manifest and report pair use parser validation. |

### Readiness evidence by clip set

| Role or runtime set | File-ready / clip-ready | Set-ready / rig-use | Runtime / acceptance boundary |
|---|---|---|---|
| Three locomotion rings | 48/48 parse; delivered loop contracts expose seam failures. | Raw spreads 0.618–0.709; current IP candidates 0.053–0.135. | Raw Unity samples pass; transformed engine/feet/root/visual gates open. |
| Forward speed alternatives | 10/10 parse and measure. | Same rig; measured 5.42× speed range. | Thresholds and blend quality need controller review. |
| Combat/equipment/actions | 110 non-locomotion files mechanically analyzed. | Common rig; broad dual-wield vocabulary; contacts/events absent. | Full-body default; visual gameplay acceptance open. |
| Prop and mask | Static prop imports at plausible scale; mask graph executes. | Identity attachments to both hands only. | Grip/orientation, pelvis torque, IK, and hit arcs open. |

### Validation-profile status

| Validation profile | Selection | Result / next evidence |
|---|---|---|
| Marketplace intake | `selected` — `vendor-intended` | Full local archive inventoried; current listing differs from local counts. |
| Blended locomotion | `selected` — `observed-pack-capability` | Current IP candidates reduce ring phase spreads; transformed-engine/visual gates open. |
| Root-motion controller | `selected` — `observed-pack-capability` | RM speed measured; yaw and action ownership open. |
| State-machine transitions | `selected` — `observed-pack-capability` | Equipment and combat states identified; interruption/events open. |
| Layered upper body/weapons | `selected` — `evaluator-selected-generic-scenario` | One Unity mask executes; full-body remains default. |
| Traversal/environment | `not-selected` | No traversal set in this bounded pack. |
| Contact actions/interactions | `selected` — `observed-pack-capability` | Attacks/parries/blocks present; contact and hit windows open. |
| Retargeted/customizable characters | `selected` — `evaluator-selected-generic-scenario` | Supplied avatar only; target character absent. |
| Motion matching/search | `not-selected` | No database/search target supplied. |
| Networked movement | `not-selected` | No authority/rollback contract supplied. |
| Runtime performance | `selected` — `evaluator-selected-generic-scenario` | Constant tracks diagnosed; runtime profiling absent. |

## Pack inventory and content evidence

The Unitypackage materializes 419 collection-relative files: 189 FBXs, 215 Unity metadata files, 11 files in PNG format, three materials, and one animation list. The 186 individual files collapse to 112 logical motions: 74 motions have two IP/RM-labelled files and 38 have one file. All individual motions share skeleton signature `2b6fe49d5ae6` with 56 bones. The combined take and skinned actor share a separate 58-bone export structure; the static sword prop has three nodes. Each of the seven pairwise comparisons found 25 overlapping package paths, all byte-identical.

## Mechanical baseline

| Finding/check | Affected scope | Potential impact | Evidence |
|---|---|---|---|
| Default mechanical pass with constant-track notes | 189/189 FBXs complete the inspect/measure/lint baseline; 25,426 notes in 188 animated files | Export bloat; no default hard blocker. | `observed-animsmith`; baseline summary. |
| Standard mechanical family | `nan`, `time-monotonic`, `quat-norm`, `quat-flip`, `duration-sanity`, `scale-keys`, and `non-uniform-scale` complete on all 189 FBXs | No reported corrupt samples, time order, quaternion, duration, or animated-scale blocker. | `observed-animsmith`; exhaustive baseline. |
| Declared loop closure/derivatives | 162/186 contracts fail; 67 closure, 162 rotation, 161 velocity file failures | Pops/pulses if delivered loop flags are trusted. | `observed-animsmith`; contract summary. |
| Directional gait phase | Three IP/RM rings; current transform covers IP only | Raw same-time blends can skate; current IP residuals still require offsets. | `observed-animsmith`; raw and current summaries. |
| RM action/reaction ambiguity | 39 RM action/reaction files; 12 below 0.1 m/s | A label alone cannot select animation/controller movement or establish yaw. | `observed-animsmith`; measured horizontal speeds. |

`constant-nonunit-scale` remained disabled, while checks requiring a declared frame rate, required bones, root-speed range, gait/sync group, or other semantic policy were inactive until applicable declarations existed. Their absence is not a pass.

## AnimSmith remediation evidence

| Source issue | Operation/declarations | Result | Independent verification | Remaining caveat |
|---|---|---|---|---|
| Missing semantic loop/in-place context | Per-file `[clips."Take 001"]` declarations derived from Unity loop metadata and observed IP/RM pairs | 24 pass; 162 fail, exposing raw loop policy. | JSON and Markdown results agree for all 186. | Delivered one-shot loop intent still needs curation. |
| Ring phase disagreement | Current `transform --gait-anchor` on 24 core IP files | 24/24 exit 0 and emit GLBs; walk/run/crouch spreads fall from 0.709/0.673/0.618 to 0.053/0.135/0.059. | Inspect, measure, and fix dry-run pass 24/24; lint/diff exit 1 with expected findings/changes. | No RM transform, Unity GLB importer, visual, or trajectory acceptance; retain residual offsets. |
| Dense constant tracks | `transform --prune-constant-tracks` on `Humanoid@IdleCombatDualSwords.fbx` | GLB produced. | Inspect/measure and fix dry-run exit 0; diff/lint retain expected semantic differences/findings. | Runtime equivalence and sparse transition behavior unproved. |

No gait-anchor trial touched RM-labelled input. The earlier b7 revision refused these inputs; that refusal is historical and no longer the current remediation result. The current candidates remain experimental because output generation and mechanical verification do not establish engine playback, trajectory, contacts, or visual quality.

## Engine procedures and evidence

| Runtime | Version | Procedure | Observed result | Remaining gate |
|---|---|---|---|---|
| Unity | 6000.5.8f1 | Fresh eight-pack project; import all models; inventory importer/avatar/clip state; sample seven clips; run Basic/Sword mixers, a Humanoid upper-body mask, and right/left-hand prop attachments. | 186/186 individual Humanoid clips; seven samples, two mixers, mask, and both attachments pass. | Visual graph, full rings/actions, contacts, root motion, target retarget, compression/build. |
| Unreal Engine | unspecified | Documentation review for Root Motion, Blend Spaces, montages, and layered animation. | Capability documented; pack not imported. | Import/retarget/graphs/contacts/build. |
| Godot | stable | Documentation review for AnimationTree blend spaces, filters, one-shots, and root motion. | Capability documented; pack not imported. | Conversion/import, retarget, graphs, contacts/export. |
| Bevy | unspecified | Documentation review for AnimationGraph masks. | Mask capability documented; pack not imported. | FBX→glTF, retarget, graphs, root motion/performance. |

## Rig, masking, and compatibility evidence

| Pack/rig/set pair | Skeleton/retarget | Scale/axes | Root policy | Timing/blend | Overall evidence |
|---|---|---|---|---|---|
| Dual Swords ↔ supplied actor | All 186 individual clips use the collection 56-bone structure and import as Unity Humanoid | Sword/actor height ratio 0.429 at local identity | IP/RM convention present | Representative execution passes | Strong candidate; grips and visuals open. |
| Dual Swords ↔ Basic Locomotion | Exact 56-bone structure; shared paths identical | Shared avatar assets identical | Full-body state owns movement | Idle mixer and attack mask execute | Technical candidate; visual state/mask acceptance open. |
| Dual Swords ↔ Sword & Shield | Exact 56-bone structure; shared paths identical | Props/grips differ and are unreviewed | One active weapon mode owns root | Idle mixer executes | Prefer full-body weapon-mode switch. |
| Dual attack ↔ Basic mask | Unity Humanoid mapping accepted | Sword attaches to both hands | Basic base owns movement | Headless mask executes | Prototype only; pelvis/contact/arcs open. |
| Pack ↔ project character | No target character | Not evaluated | Project policy unknown | Not evaluated | Unknown. |

## Limitations and unknowns

1. No target character, camera, controller, quality bar, combat design, hit-window specification, or networking policy was supplied.
2. Headless Unity proves import/execution, not pose quality, feet, deformation, grips, weapon contacts, root behavior, or perceived timing.
3. Unreal Engine, Godot, and Bevy remain documentation-only.
4. Delivered loop metadata is not reliable author intent; strict failures do not mean every flagged clip visibly fails in-game.
5. `_RM` is naming evidence; yaw and low-horizontal-speed actions need independent review.
6. Offline HTML reports were generated but not used to claim visual or artistic acceptance.
7. Current public pages/EULA do not prove the local revision, transaction date, or historical terms.

## Reproduction

Source identity: RAR SHA-256 `465eea80e3039c0b70f06784e4dd4cec19a0e62012cebf35a0b435a02826afed`; Unitypackage SHA-256 `296a121c779b5972335b6e9ffcbdb491fe06b462dcff07f60475abaee5fb7d81`. Baseline/contracts evaluator: `animsmith 0.3.0 (v0.3.0-34-gb7c215b)`, revision `b7c215ba259b87b4b4e46567452a037a34be7308`, binary SHA-256 `67bdc22ce1a83feb7312a1ddf251d330b2e8113c10a845b71de1169955ef8609`. Gait remediation used pre-release 0.3.1 code: `animsmith 0.3.0 (v0.3.0-39-g674396f)`, revision `674396f0f53b10c4344e7315a5756fe5ef71b469`, binary SHA-256 `7744b71580e04d80f9e5738efce76e0295323ccb3150fa57b0ad9b37c5ff1513`.

```text
animsmith inspect --config config/baseline.animsmith.toml <input.fbx>
animsmith measure --config config/baseline.animsmith.toml --format json <input.fbx>
animsmith lint --config config/contracts/<file>.animsmith.toml --format json <input.fbx>
animsmith report --config <config> <input.fbx> --output <report.html>
animsmith transform --config <config> <input.fbx> -o <output.glb> --gait-anchor
animsmith transform --config <config> <input.fbx> -o <output.glb> --prune-constant-tracks
```

Retained summaries: b7 baseline `d627e5fd7957b373cdfd5dd4368b1d619d9a990d667dc482f08c67fcb822cde0`; b7 contracts `dea91806cab5c0e85fb8cd344769f26e9f7e36f1954b8770fcc448f48611c7ae`; historical b7 remediation `6610cd4d5382bd203a3d7f92d9b520348e802cbb59285b4b6b1c752bfd10fab1`; current gait commands `16d31a27a961180154afc30613e1ab3e5e4a7cdb8aab94238861947c8e819a15`; current combined summary `118116c9173df4e3e782cdfe3b712deb9fb14cec23c8e0e75cd484e8156d7f4b`; Unity probe `1c147ff6683833ba28c1db210d58aee65140ac232311f370782c28c3925ae62d`.

## Sources

- Local source archive and extracted Unity metadata — private authorized input identified above, accessed 2026-08-17.
- Protofactor, [Dual Swords](https://protofactor.biz/product/animset-dual-swords/), [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/), and [End User License Agreement](https://protofactor.biz/end-user-license-agreement/) — current listing/license context, not local revision proof.
- Unity, [Avatar Mask](https://docs.unity3d.com/6000.0/Documentation/Manual/class-AvatarMask.html), [Animation Layers](https://docs.unity3d.com/6000.0/Documentation/Manual/AnimationLayers.html), and [Root Motion](https://docs.unity3d.com/6000.5/Documentation/Manual/RootMotion.html) — runtime capabilities.
- Epic Games, [Root Motion](https://dev.epicgames.com/documentation/unreal-engine/root-motion-in-unreal-engine?application_version=5.7) and [Layered Animations](https://dev.epicgames.com/documentation/unreal-engine/using-layered-animations-in-unreal-engine?lang=en-US); Godot, [AnimationTree](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html); Bevy, [Animation Masks](https://bevy.org/examples/animation/animation-masks/) — documentation-only capability.
- AnimSmith issues [#401](https://github.com/mmannerm/animsmith/issues/401), [#402](https://github.com/mmannerm/animsmith/issues/402), [#408](https://github.com/mmannerm/animsmith/issues/408), and implemented [#426](https://github.com/mmannerm/animsmith/issues/426) — optimization, root, and gait context.
