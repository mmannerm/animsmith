# Animation pack evidence appendix: Protofactor Dual Swords Animset

> Companion report: [technical evaluation](protofactor-dual-swords.md)
>
> Evidence status: **partial** — exhaustive 0.4.0 baseline/contract/gait-remediation captured on one released evaluator for the first time on this pack; retained 2026-08-17 Unity 6000.5.8f1 probe plus new 0.4.0 engine-profile advice/refusal/addressability evidence; transformed-clip visual/engine acceptance and three full engine imports remain unevaluated.
>
> Evaluation date: **2026-08-21**
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
| Target engines | Unity 6000.3 (0.4.0 import-advice) and Unity 6000.5.8f1 (retained 2026-08-17 probe); Unreal 5.8 and Godot 4.7 profile advice (typed refusal); Bevy 0.19.0 addressability prediction on a generated GLB |
| Target rigs/packs | Supplied Protof-Actor, Basic Locomotion, Sword & Shield, and the evaluated collection subset |
| Source manifest | `logical-assets-inventory.json`; SHA-256 `d7a3e3d88ba5f93c04d20d9ba8c316e667a5e27a132ef8fa33ebda5339d1e535`; re-inventoried 2026-08-21 with 0 added, 0 removed, 0 content changed across all 189 FBXs |
| Evaluation manifest | `evidence/evaluation-manifest.json`; SHA-256 `57e90445c1cb11f80506c5d551c2426ef45bf421a38e45dab3fcd928c79fbd21`; schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1`; taxonomy/profile-set version 1; refreshed on AnimSmith 0.4.0, 2026-08-21 |
| Acquisition/license provenance | `user-stated`: local archive was downloaded from Protofactor.biz as part of the Ultimate Animation Collection. Current EULA permits protected released real-time applications and modification while restricting redistribution/resale; no receipt, download date, historical EULA, or local constituent revision was retained. Technical due diligence only, not legal advice. |

This refresh captures baseline, contracts, and gait remediation on one frozen evaluator, AnimSmith 0.4.0 (tag `v0.4.0`, revision `6b37ad636b198ef8ff47fadbf6a3a51eb1a27c8e`, binary SHA-256 `fd1eee57407aa02db88763d144389a7f5104204c40ddfbb28eb5885ca8cd54c6`, output schema v10 / measurements schema v15), captured 2026-08-21. This is the first time this pack's contracts and gait evidence come from the same evaluator: the published contracts previously sat at `b7c215ba259b87b4b4e46567452a037a34be7308` and were never rerun on the pre-release `674396f` gait build. Source identity re-verifies byte-identical to the published manifest: archive SHA-256 `465eea80e3039c0b70f06784e4dd4cec19a0e62012cebf35a0b435a02826afed` and Unitypackage SHA-256 `296a121c779b5972335b6e9ffcbdb491fe06b462dcff07f60475abaee5fb7d81` both re-verify, so this is a pure evaluator-version refresh with no change in source bytes.

The current product page, observed 2026-08-17, advertises USD 24.99, 185 animations, 79 root-motion and 106 in-place files, Unity Humanoid, Unity 2019.4+, and no native UE4 package. The local delivery has 186 individual files and 76 `_RM` labels, so the current listing does not identify the local constituent revision or explain either count difference. This vendor-page observation was not re-checked on 2026-08-21 and stays dated 2026-08-17.

The evaluation manifest uses schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1`.

### Evidence coverage

| Surface | Offered/delivered | Evaluated | Findings | Not evaluated and why |
|---|---:|---:|---:|---|
| Animation files | 187 animation FBXs | 187 | 186 individual plus one combined take; actor and prop are support assets | Continuous artistic review of all motion |
| Rigs/export variants | 3 observed structures | 3 | Individual files share one 56-bone signature; actor/combined 58; prop 3 | Target-character deformation and non-Unity retarget |
| AnimSmith baseline | 189 FBXs | 189 | 0.4.0 commands exit 0 (189/189); 25,426 constant-track notes across 188 animated files, identical to the published 0.3.0 count and the collection's largest single-pack total | Artistic intent and contacts |
| Declared contracts | 186 individual files | 186 | 0.4.0: 24 exit 0, 162 with findings — matches the published 24/162 exactly; first time contracts and gait share one evaluator | Human loop intent for every action |
| Offline visual reports | 186 possible | 4 risk-selected | Reports generated for idle, locomotion, attack, and combo | Reports were not used as artistic acceptance |
| Engine import/playback | 186 individual files in Unity; 189 files through 0.4.0 profile advice/addressability | 186 + 189 | Retained: all Humanoid, seven samples pass (2026-08-17). New: `unity-humanoid` advice exit 0; `unreal`/`godot` advice typed-refuse `profile_settings_unmodeled`; `bevy` addressability exit 0 on one generated candidate | Full graph, visual root motion, compression, build; real Unreal/Godot/Bevy import |
| Blend/mask/retarget | 3 collection graphs + 2 prop attachments | 3 + 2 | Basic and Sword mixers, Basic mask, and both hand attachments execute (retained 2026-08-17) | Visual blending, grips, target retarget, IK |

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
| **Total** | **112** | **186** | Validated v1 manifest identified above; reproduced unchanged by the 2026-08-21 re-inventory. |

### Runtime-set inventory

| Runtime set | Type | Members/variants | Grouping evidence | Validation status |
|---|---|---|---|---|
| Walk combat 8-way | directional-blend | 8 IP + 8 RM | Exact names, common duration, directions, and speeds; see [primary table](protofactor-dual-swords.md#runtime-sets-and-authored-motion). | 0.4.0-reproduced; current IP candidate circular spread 0.7086162 → 0.0529930 (7-decimal match to the pre-release `674396f` result); engine/visual acceptance open. |
| Run combat 8-way | directional-blend | 8 IP + 8 RM | Exact names, common duration, and near-uniform speed. | 0.4.0-reproduced; current IP candidate circular spread 0.6731805 → 0.1350506 (7-decimal match); engine/visual acceptance open. |
| Crouch combat 8-way | directional-blend | 8 IP + 8 RM | Exact names, common duration, and near-uniform speed. | 0.4.0-reproduced; current IP candidate circular spread 0.6184063 → 0.0587151 (7-decimal match); engine/visual acceptance open. |
| Forward speed alternatives | speed-blend | walk-1/walk-2/jog/jog-fast/run-fast IP + RM | Filename order and measured 0.847–4.588 m/s RM speeds. | 0.4.0-reproduced; threshold candidate; visual blend open. |
| Draw/combat/put-away | transition-chain | 3 single files | Delivered equipment verbs and combat idle. | Retained 2026-08-17 Unity draw sample passes; endpoint/visibility timing open. |
| Combo alternatives | other | 19 IP/RM logical choices, 38 files | Numbered two- through five-hit filenames and paired files. | Inventory candidate; hit/branch/cancel contract absent. |
| Single-attack alternatives | other | 11 IP/RM logical choices, 22 files | Numbered attack filenames and paired files. | Discrete-action candidate; hit/contact contract absent. |

### Pipeline-stage coverage

| Stage | Coverage state | Evidence / remaining gate |
|---|---|---|
| Acquire | `evaluated-clean` | Local archive identified and hashed outside the repository. |
| Preserve raw | `evaluated-clean` | RAR and Unitypackage retained unchanged; re-verified byte-identical 2026-08-21. |
| Inspect | `evaluated-finding` | Every FBX inspected/measured/linted on AnimSmith 0.4.0. |
| Segment | `partially-evaluated` | Individual FBXs used; combined take not promoted. |
| Root motion | `evaluated-finding` | Closed [#408](https://github.com/mmannerm/animsmith/issues/408) delivered per-clip root displacement/yaw; 188/188 clips measured (76 moving, 111 stationary, 0 with >1° yaw); action-ownership intent stays open. |
| Conform | `evaluated-clean` | All individual files share one 56-bone structure and import as Unity Humanoid. |
| Validate | `partially-evaluated` | Mechanical contracts complete on 0.4.0; visual combat acceptance open. |
| Optimize | `evaluated-finding` | 24 current gait candidates (7-decimal reproduction of the pre-release result) and one pruning candidate generated on 0.4.0; both remain unpromoted — no engine import ran this session and the retained Unity project has no GLB importer. |
| Export | `partially-evaluated` | Current GLBs inspect/measure cleanly but lack engine/visual acceptance. |
| Gate/report | `evaluated-clean` | Manifest and report pair use parser validation. |

### Readiness evidence by clip set

| Role or runtime set | File-ready / clip-ready | Set-ready / rig-use | Runtime / acceptance boundary |
|---|---|---|---|
| Three locomotion rings | 48/48 parse; delivered loop contracts expose seam failures (0.4.0-reproduced). | Circular spreads: Walk 0.7086162→0.0529930, Run 0.6731805→0.1350506, Crouch 0.6184063→0.0587151 — 7-decimal match to the pre-release `674396f` result. | Retained 2026-08-17 raw Unity samples pass; no engine import ran this session, the retained Unity project has no GLB importer, and candidates stay unpromoted. |
| Forward speed alternatives | 10/10 parse and measure. | Same rig; measured 5.42× speed range. | Thresholds and blend quality need controller review. |
| Combat/equipment/actions | 110 non-locomotion files mechanically analyzed. | Common rig; broad dual-wield vocabulary; contacts/events absent. | Full-body default; visual gameplay acceptance open. |
| Prop and mask | Static prop imports at plausible scale (retained); mask graph executes (retained). | Identity attachments to both hands only. Per-bone `bone_channels` coverage (0.4.0, closed [#402](https://github.com/mmannerm/animsmith/issues/402)) narrows attachment risk on the two-prop composition without proving a working mask or attachment. | Grip/orientation, pelvis torque, IK, and hit arcs open. |

### Validation-profile status

| Validation profile | Selection | Result / next evidence |
|---|---|---|
| Marketplace intake | `selected` — `vendor-intended` | Full local archive re-inventoried 2026-08-21; 0 added/removed/changed; current listing still differs from local counts. |
| Blended locomotion | `selected` — `observed-pack-capability` | 0.4.0 reproduces the current IP candidate ring spreads to 7 decimals; transformed-engine/visual gates open. |
| Root-motion controller | `selected` — `observed-pack-capability` | Root trajectory measured 188/188 clips (76 moving, 111 stationary, 0 with >1° yaw; `heading_axis` `positive_y` on 187); action ownership open. |
| State-machine transitions | `selected` — `observed-pack-capability` | Equipment and combat states identified; interruption/events open. |
| Layered upper body/weapons | `selected` — `evaluator-selected-generic-scenario` | Retained 2026-08-17 Unity mask executes; 0.4.0 per-bone channel coverage (closed [#402](https://github.com/mmannerm/animsmith/issues/402)) narrows attachment risk but does not prove a working mask; full-body remains default. |
| Traversal/environment | `not-selected` | No traversal set in this bounded pack. |
| Contact actions/interactions | `selected` — `observed-pack-capability` | Attacks/parries/blocks present; contact and hit windows open. |
| Retargeted/customizable characters | `selected` — `evaluator-selected-generic-scenario` | Supplied avatar only; target character absent. |
| Motion matching/search | `not-selected` | No database/search target supplied. |
| Networked movement | `not-selected` | No authority/rollback contract supplied. |
| Runtime performance | `selected` — `evaluator-selected-generic-scenario` | Constant tracks diagnosed (0.4.0-reproduced); runtime profiling absent. |

## Pack inventory and content evidence

The Unitypackage materializes 419 collection-relative files: 189 FBXs, 215 Unity metadata files, 11 files in PNG format, three materials, and one animation list. The 186 individual files collapse to 112 logical motions: 74 motions have two IP/RM-labelled files and 38 have one file. All individual motions share skeleton signature `2b6fe49d5ae6` with 56 bones. The combined take and skinned actor share a separate 58-bone export structure; the static sword prop has three nodes. Each of the seven pairwise comparisons found 25 overlapping package paths, all byte-identical.

The pack composes as two swords, one attached per hand, over the shared 56-bone rig. Measurements v15's canonical per-bone `bone_channels` evidence (translation/rotation/scale track presence per bone index — delivered by closed [#402](https://github.com/mmannerm/animsmith/issues/402)) was inspected for this two-prop composition: presence on the hand/attachment-relevant bones narrows the risk that a mask or socket references a bone with no authored motion, but it does not by itself prove a working engine mask, a correct socket transform, or a visually accepted attachment. That gate stays with the retained headless mask/attachment probe and future visual review.

## Mechanical baseline

| Finding/check | Affected scope | Potential impact | Evidence |
|---|---|---|---|
| Default mechanical pass with constant-track notes | 189/189 FBXs complete the inspect/measure/lint baseline; 25,426 notes in 188 animated files, identical to the published 0.3.0 count and the collection's largest single-pack total | Export bloat; no default hard blocker. | `observed-animsmith`; 0.4.0 baseline summary, reproduced exactly. |
| Standard mechanical family | `nan`, `time-monotonic`, `quat-norm`, `quat-flip`, `duration-sanity`, `scale-keys`, and `non-uniform-scale` complete on all 189 FBXs; all 189 lint exits are 0 | No reported corrupt samples, time order, quaternion, duration, or animated-scale blocker; no error-severity finding at any scope. | `observed-animsmith`; exhaustive 0.4.0 baseline. |
| Declared loop closure/seam contracts | 186 files linted; 24 exit 0, 162 with findings (matches the published 24/162 exactly); `loop-closure` 67 files (71 findings), `loop-seam-rot` 162 files/findings, `loop-seam-vel` 161 files/findings, coarse `loop-seam` 0 files with findings | Pops/pulses if delivered loop flags are trusted; the coarse `loop-seam` check alone would hide the rotation/velocity seam failures the finer checks expose. | `observed-animsmith`; 0.4.0 contract summary. |
| Loop-seam and in-place applicability/evaluation granularity | `loop_seam_ratio` applicable 168/186, not_applicable 18/186; `loop_seam_evaluation` complete 106, not_evaluated 80; `in_place` applicable 74, not_applicable 112 | No-stride and stationary clips are recorded `not_evaluated` rather than a mislabeled pass or fail — a consumer must not read `not_evaluated` as either. | `observed-animsmith`; measurements v15 availability fields. |
| Directional gait phase | Three IP/RM rings; 24/24 current IP candidates | Raw same-time blends can skate; current IP residuals still require offsets and no engine import ran this session. | `observed-animsmith`; raw and 0.4.0 current summaries, see AnimSmith remediation evidence below. |
| RM action/reaction ownership, re-expressed via measured travel | Root trajectory measured 188/188 clips: 76 move >1 cm horizontally, 111 stationary (≤1 cm), 0 with yaw travel >1°; `heading_axis` is `positive_y` on all 187 clips with a measured yaw domain | A clip's measured horizontal-travel or stationary status does not by itself establish which axis the controller versus the animation should own; each RM action/reaction still needs an explicit per-clip ownership review. These are sampled regression facts from the shared uniform metric grid, not continuous-curve or engine root-motion extraction proof. | `observed-animsmith`; measurements v15 `root_trajectory` fields, delivered by closed [#408](https://github.com/mmannerm/animsmith/issues/408). |

`constant-nonunit-scale` remained disabled, while checks requiring a declared frame rate, required bones, root-speed range, gait/sync group, or other semantic policy were inactive until applicable declarations existed. Their absence is not a pass.

## AnimSmith remediation evidence

| Source issue | Operation/declarations | Result | Independent verification | Remaining caveat |
|---|---|---|---|---|
| Missing semantic loop/in-place context | Per-file `[clips."Take 001"]` declarations derived from Unity loop metadata and observed IP/RM pairs | 24 pass; 162 fail, exposing raw loop policy — reproduced exactly on 0.4.0. | JSON and Markdown results agree for all 186. | Delivered one-shot loop intent still needs curation. |
| Ring phase disagreement | Current `transform --gait-anchor` on 24 core IP files, AnimSmith 0.4.0 | 24/24 exit 0 and emit GLBs. Circular spread (smallest arc containing the ring): Crouch 0.6184063 → 0.0587151; Run 0.6731805 → 0.1350506; Walk 0.7086162 → 0.0529930 — each matches the pre-release `674396f` after-value to seven decimal places, confirming the released 0.4.0 evaluator preserves the 0.3.1 gait behavior instead of resting on an unreleased build. | Inspect, measure, and fix dry-run pass 24/24; lint/diff exit 1 with expected findings/changes. | No RM transform, no Unity GLB importer, and no engine import at all ran this session; visual and trajectory acceptance are open. Candidates remain unpromoted. |
| Dense constant tracks | `transform --prune-constant-tracks` on `Humanoid@IdleCombatDualSwords.fbx`, AnimSmith 0.4.0 | GLB produced; source never modified. | Inspect/measure and fix dry-run exit 0; diff/lint retain expected semantic differences/findings. | Runtime equivalence and sparse transition behavior unproved; bounded by open [#401](https://github.com/mmannerm/animsmith/issues/401). Candidate remains unpromoted. |

No gait-anchor trial touched RM-labelled input; every resampled channel is in-place-only, so root translation/yaw accumulation is not a factor for these 24 candidates. The root heading basis is measurable for this rig (`heading_axis` `positive_y`, see Mechanical baseline). The earlier `b7c215b` heading-basis refusals are historical only, and the pre-release `674396f` build that first produced these 24 outputs is superseded: 0.4.0 is the first publicly released evaluator to reproduce that result. Both current candidates remain experimental because output generation and mechanical verification do not establish engine playback, trajectory, contacts, or visual quality, and no engine import ran this session.

## Engine procedures and evidence

| Runtime | Version | Procedure | Observed result | Remaining gate |
|---|---|---|---|---|
| Unity | 6000.3 (0.4.0 advice) | `animsmith --config <unity-humanoid.animsmith.toml> generate import-advice <input.fbx>` for `unity-humanoid` revision 1 / `fbx-model-importer`. Declarations (`convert_units`, `bake_axis_conversion`, and per-clip `root_rotation`/`root_position_y`/`root_position_xz`) were derived from the delivered `.fbx.meta` sidecars: `useFileUnits: 1`, and `lockRootRotation`/`lockRootHeightY`/`lockRootPositionXZ` are absent on every delivered meta, so Unity applies its documented default `false` for each, which AnimSmith's `bake=true`/`extract=false` mapping resolves to `extract` for every clip. This is an operator-verified assumption about the absent keys, not a value AnimSmith read from the meta itself. | Advice available, exit 0, for all sampled clips. This is 6000.3 advice derived from declared settings; it is not observed Unity 6000.5.8f1 import behavior. | Confirm the meta-derived assumption and the advice against an actual 6000.3 or 6000.5 import; visual controller, contacts, grips, root motion, retargeting, compression, build. |
| Unity | 6000.5.8f1 (retained 2026-08-17) | Fresh eight-pack project; import all models; inventory importer/avatar/clip state; sample seven clips; run Basic/Sword mixers, a Humanoid upper-body mask, and right/left-hand prop attachments. Retained unrun this session because the source is confirmed byte-identical. | 186/186 individual Humanoid clips; seven samples, two mixers, mask, and both attachments pass. | Visual graph, full rings/actions, contacts, root motion, target retarget, compression/build; rerun against the 24 current gait candidates once a GLB importer is added. |
| Unreal Engine | 5.8 (0.4.0 advice) | `generate import-advice` for the `unreal` revision 1 / `fbx-importer` profile. | Typed refusal `profile_settings_unmodeled`, exit 1: revision 1 models no Unreal setting vocabulary, so AnimSmith declines rather than guessing. Root Motion, Blend Spaces, montages, and layered blends remain documented capability, not observed. | FBX import, retarget, graphs, contacts, build; a future profile revision would need a modeled Unreal setting vocabulary. |
| Godot | 4.7 (0.4.0 advice) | `generate import-advice` for the `godot` revision 1 / `resource-importer-scene` profile. | Typed refusal `profile_settings_unmodeled`, exit 1, for the same reason as Unreal. AnimationTree blend spaces, one-shots, filters, and root extraction remain documented capability, not observed. | Conversion/import, retarget, graphs, contacts/export; a future profile revision would need a modeled Godot setting vocabulary. |
| Bevy | 0.19.0 (0.4.0 addressability) | `animsmith --config <bevy.animsmith.toml> generate addressability <candidate.glb>` for the `bevy` revision 1 / `gltf-asset-loader` profile, run against one generated remediation-trial GLB candidate. | Exit 0; one animation row; coverage complete; predicted selector `Animation0`; facet `available`; 0 findings. This is inventory/selector prediction only — it does not prove a Bevy runtime load, that animation loading was enabled, or that graph wiring is usable. | FBX→glTF pipeline, retarget path, graph, root motion, performance; an actual Bevy runtime load. |

## Rig, masking, and compatibility evidence

| Pack/rig/set pair | Skeleton/retarget | Scale/axes | Root policy | Timing/blend | Overall evidence |
|---|---|---|---|---|---|
| Dual Swords ↔ supplied actor | All 186 individual clips use the collection 56-bone structure and import as Unity Humanoid (retained) | Sword/actor height ratio 0.429 at local identity | IP/RM convention present | Representative execution passes (retained) | Strong candidate; grips and visuals open. |
| Dual Swords ↔ Basic Locomotion | Exact 56-bone structure; shared paths identical | Shared avatar assets identical | Full-body state owns movement | Idle mixer and attack mask execute (retained) | Technical candidate; visual state/mask acceptance open. |
| Dual Swords ↔ Sword & Shield | Exact 56-bone structure; shared paths identical | Props/grips differ and are unreviewed | One active weapon mode owns root | Idle mixer executes (retained) | Prefer full-body weapon-mode switch. |
| Dual attack ↔ Basic mask | Unity Humanoid mapping accepted | Sword attaches to both hands; 0.4.0 per-bone channel coverage on the two-prop composition narrows but does not close attachment risk | Basic base owns movement | Headless mask executes (retained) | Prototype only; pelvis/contact/arcs open. |
| Pack ↔ project character | No target character | Not evaluated | Project policy unknown | Not evaluated | Unknown. |

## Limitations and unknowns

1. No target character, camera, controller, quality bar, combat design, hit-window specification, or networking policy was supplied.
2. Headless Unity proves import/execution, not pose quality, feet, deformation, grips, weapon contacts, root behavior, or perceived timing.
3. No engine import ran this session; the 24 current gait candidates and the constant-track-pruning candidate remain unpromoted, and the retained Unity project still has no GLB importer. Unreal Engine, Godot, and Bevy remain advice/prediction-only, not real imports.
4. Delivered loop metadata is not reliable author intent; strict failures do not mean every flagged clip visibly fails in-game.
5. `_RM` and the measured horizontal-travel/stationary split are naming and sampled-grid evidence, not proof of movement-ownership axes; each RM action/reaction still needs an independent per-clip review.
6. Per-bone `bone_channels` presence narrows mask/attachment risk on the two-prop composition but does not prove a working mask, socket, or attachment.
7. Offline HTML reports were generated but not used to claim visual or artistic acceptance.
8. Current public pages/EULA do not prove the local revision, transaction date, or historical terms, and were not re-checked on 2026-08-21.
9. The 0.4.0 Unity 6000.3 import-advice rests on an operator-verified assumption about absent `.fbx.meta` keys, not an observed Unity 6000.3 or 6000.5 import.
10. The integration recipe's `owner=split-by-movement-variant` step is a variant-level integration recommendation grounded in the delivered in-place/root-motion pairing, which is observed-file evidence. It is not a per-axis `movement_owner_xz` / `movement_owner_y` / `movement_owner_yaw` declaration, and no such declaration is derived from measured travel in this refresh. Measured root displacement and yaw are recorded as sampled facts only; choosing the per-axis owner remains a project and engine decision.

## Reproduction

Source identity: RAR SHA-256 `465eea80e3039c0b70f06784e4dd4cec19a0e62012cebf35a0b435a02826afed`; Unitypackage SHA-256 `296a121c779b5972335b6e9ffcbdb491fe06b462dcff07f60475abaee5fb7d81`; both re-verified byte-identical on 2026-08-21 (0 added, 0 removed, 0 content changed across 189 FBXs).

Current evaluator (baseline, contracts, and gait remediation): AnimSmith 0.4.0, tag `v0.4.0`, revision `6b37ad636b198ef8ff47fadbf6a3a51eb1a27c8e`, binary SHA-256 `fd1eee57407aa02db88763d144389a7f5104204c40ddfbb28eb5885ca8cd54c6`, output schema v10, measurements schema v15, captured 2026-08-21.

Historical evaluators, retained as dated comparison only: baseline/contracts originally captured on `animsmith 0.3.0 (v0.3.0-34-gb7c215b)`, revision `b7c215ba259b87b4b4e46567452a037a34be7308`, binary SHA-256 `67bdc22ce1a83feb7312a1ddf251d330b2e8113c10a845b71de1169955ef8609`. Gait remediation was first produced on pre-release `animsmith 0.3.0 (v0.3.0-39-g674396f)`, revision `674396f0f53b10c4344e7315a5756fe5ef71b469`, binary SHA-256 `7744b71580e04d80f9e5738efce76e0295323ccb3150fa57b0ad9b37c5ff1513`; 0.4.0 reproduces those after-values to seven decimal places (AnimSmith remediation evidence above).

```text
animsmith inspect --config config/baseline.animsmith.toml <input.fbx>
animsmith measure --config config/baseline.animsmith.toml --format json <input.fbx>
animsmith lint --config config/contracts/<file>.animsmith.toml --format json <input.fbx>
animsmith report --config <config> <input.fbx> --output <report.html>
animsmith transform --config <config> <input.fbx> -o <output.glb> --gait-anchor
animsmith transform --config <config> <input.fbx> -o <output.glb> --prune-constant-tracks
animsmith --config <unity-humanoid.animsmith.toml> generate import-advice <input.fbx>
animsmith --config <unreal.animsmith.toml> generate import-advice <input.fbx>
animsmith --config <godot.animsmith.toml> generate import-advice <input.fbx>
animsmith --config <bevy.animsmith.toml> generate addressability <candidate.glb>
```

Retained historical summaries: b7 baseline `d627e5fd7957b373cdfd5dd4368b1d619d9a990d667dc482f08c67fcb822cde0`; b7 contracts `dea91806cab5c0e85fb8cd344769f26e9f7e36f1954b8770fcc448f48611c7ae`; historical b7 remediation `6610cd4d5382bd203a3d7f92d9b520348e802cbb59285b4b6b1c752bfd10fab1`; pre-release 674396f gait commands `16d31a27a961180154afc30613e1ab3e5e4a7cdb8aab94238861947c8e819a15`; pre-release 674396f combined summary `118116c9173df4e3e782cdfe3b712deb9fb14cec23c8e0e75cd484e8156d7f4b`; retained Unity probe `1c147ff6683833ba28c1db210d58aee65140ac232311f370782c28c3925ae62d`. The 2026-08-21 AnimSmith 0.4.0 baseline, contract, and gait-remediation run reproduced these historical totals and the pre-release gait spreads exactly (see Mechanical baseline and AnimSmith remediation evidence above); its JSON/Markdown outputs and generated GLB candidates are retained with the commercial source outside the repository, and no new digest is asserted here beyond the evaluator binary identity above.

## Sources

- Local source archive and extracted Unity metadata — private authorized input identified above, accessed 2026-08-17, re-verified 2026-08-21.
- Protofactor, [Dual Swords](https://protofactor.biz/product/animset-dual-swords/), [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/), and [End User License Agreement](https://protofactor.biz/end-user-license-agreement/) — current listing/license context, not local revision proof.
- Unity, [Avatar Mask](https://docs.unity3d.com/6000.0/Documentation/Manual/class-AvatarMask.html), [Animation Layers](https://docs.unity3d.com/6000.0/Documentation/Manual/AnimationLayers.html), and [Root Motion](https://docs.unity3d.com/6000.5/Documentation/Manual/RootMotion.html) — runtime capabilities.
- Epic Games, [Root Motion](https://dev.epicgames.com/documentation/unreal-engine/root-motion-in-unreal-engine?application_version=5.7) and [Layered Animations](https://dev.epicgames.com/documentation/unreal-engine/using-layered-animations-in-unreal-engine?lang=en-US); Godot, [AnimationTree](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html); Bevy, [Animation Masks](https://bevy.org/examples/animation/animation-masks/) — documentation-only capability.
- AnimSmith issues: open [#401](https://github.com/mmannerm/animsmith/issues/401) (constant-track pruning equivalence proof); closed [#402](https://github.com/mmannerm/animsmith/issues/402) (per-clip channel coverage), [#407](https://github.com/mmannerm/animsmith/issues/407) (transform fail-closed gait policy), [#408](https://github.com/mmannerm/animsmith/issues/408) (root displacement/accumulated yaw per clip), and [#426](https://github.com/mmannerm/animsmith/issues/426) (vertical root-forward gait anchoring) — verified live 2026-08-21.
