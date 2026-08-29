# Animation pack evidence appendix: Protofactor Dual Swords Animset

> Companion report: [technical evaluation](protofactor-dual-swords.md)
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
| Pack/edition | Local `Animset@DualSwords_PACKAGE.unitypackage`; constituent revision is not declared |
| Vendor/source | Protofactor; [current Dual Swords product page](https://protofactor.biz/product/animset-dual-swords/) |
| Delivered scope | Full local RAR → one Unitypackage → 189 FBXs: 186 individual motions, one combined animation FBX, one skinned actor, and one sword prop; animation list, Unity metadata, materials, and textures included |
| Target use | Game-engine use; third-person dual-wield combat and combination with the other seven evaluated constituents |
| Target engines | Dated Unity 6000.5.8f1 observation; current Unity Humanoid revision-1, Unreal revision-2, and Godot revision-2 settings projections; Bevy revision-3 rich addressability |
| Target rigs/packs | Supplied Protof-Actor, Basic Locomotion, Sword & Shield, and the evaluated collection subset |
| Source manifest | `logical-assets-inventory.json`; SHA-256 `d7a3e3d88ba5f93c04d20d9ba8c316e667a5e27a132ef8fa33ebda5339d1e535`; re-inventoried 2026-08-21 with 0 added, 0 removed, 0 content changed across all 189 FBXs |
| Evaluation manifest | `evidence/evaluation-manifest.json`; SHA-256 `57e90445c1cb11f80506c5d551c2426ef45bf421a38e45dab3fcd928c79fbd21`; schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1`; taxonomy/profile-set version 1; refreshed on AnimSmith 0.7.0, 2026-08-21 |
| Acquisition/license provenance | `user-stated`: local archive was downloaded from Protofactor.biz as part of the Ultimate Animation Collection. Current EULA permits protected released real-time applications and modification while restricting redistribution/resale; no receipt, download date, historical EULA, or local constituent revision was retained. Technical due diligence only, not legal advice. |

The current evaluator is AnimSmith 0.7.0, tag `v0.7.0`, commit `461ac8a4f6bb368eb8637471a796f13eeb647140`, binary SHA-256 `01a501999c91d93abfb32b1f48241fccc70914fac27c9a650c31df44262578d8`, output schema v17, and measurements schema v16, captured 2026-08-26. It produced the complete baseline, contract, remediation, and projection evidence in this appendix.

The product-page evidence dated 2026-08-17 advertises USD 24.99, 185 animations, 79 root-motion and 106 in-place files, Unity Humanoid, Unity 2019.4+, and no native UE4 package. The local delivery has 186 individual files and 76 `_RM` labels, so the listing does not identify the local constituent revision or explain either count difference.

The evaluation manifest uses schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1`.

### Evidence coverage

| Surface | Offered/delivered | Evaluated | Findings | Not evaluated and why |
|---|---:|---:|---:|---|
| Animation files | 187 animation FBXs | 187 | 186 individual plus one combined take; actor and prop are support assets | Continuous artistic review of all motion |
| Rigs/export variants | 3 observed structures | 3 | Individual files share one 56-bone signature; actor/combined 58; prop 3 | Target-character deformation and non-Unity retarget |
| AnimSmith baseline | 189 FBXs | 189 | the current evaluation commands exit 0 (189/189); 25,426 constant-track notes across 188 animated files | Artistic intent and contacts |
| Declared contracts | 186 individual files | 186 | Current: 24 exit 0, 162 with findings — matches the published 24/162 exactly; first time contracts and gait share one evaluator | Human loop intent for every action |
| Offline visual reports | 186 possible | 4 risk-selected | Reports generated for idle, locomotion, attack, and combo | Reports were not used as artistic acceptance |
| Engine import/playback | 186 individual files in Unity; 189 files through current profile advice/addressability | 186 + 189 | Current projections are available for Unity Humanoid revision 1, Unreal revision 2, Godot revision 2, and Bevy revision 3; these are not engine execution evidence. | Full graph, visual root motion, compression, build; real Unreal/Godot/Bevy import |
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
| Walk combat 8-way | directional-blend | 8 IP + 8 RM | Exact names, common duration, directions, and speeds; see [primary table](protofactor-dual-swords.md#runtime-sets-and-authored-motion). | Retained current IP candidate circular spread 0.7086162 → 0.0529930; engine/visual acceptance open. |
| Run combat 8-way | directional-blend | 8 IP + 8 RM | Exact names, common duration, and near-uniform speed. | Retained current IP candidate circular spread 0.6731805 → 0.1350506 (7-decimal match); engine/visual acceptance open. |
| Crouch combat 8-way | directional-blend | 8 IP + 8 RM | Exact names, common duration, and near-uniform speed. | Retained current IP candidate circular spread 0.6184063 → 0.0587151 (7-decimal match); engine/visual acceptance open. |
| Forward speed alternatives | speed-blend | walk-1/walk-2/jog/jog-fast/run-fast IP + RM | Filename order and measured 0.847–4.588 m/s RM speeds. | current; threshold candidate; visual blend open. |
| Draw/combat/put-away | transition-chain | 3 single files | Delivered equipment verbs and combat idle. | Retained 2026-08-17 Unity draw sample passes; endpoint/visibility timing open. |
| Combo alternatives | other | 19 IP/RM logical choices, 38 files | Numbered two- through five-hit filenames and paired files. | Inventory candidate; hit/branch/cancel contract absent. |
| Single-attack alternatives | other | 11 IP/RM logical choices, 22 files | Numbered attack filenames and paired files. | Discrete-action candidate; hit/contact contract absent. |

### Pipeline-stage coverage

| Stage | Coverage state | Evidence / remaining gate |
|---|---|---|
| Acquire | `evaluated-clean` | Local archive identified and hashed outside the repository. |
| Preserve raw | `evaluated-clean` | RAR and Unitypackage retained unchanged; re-verified byte-identical 2026-08-21. |
| Inspect | `evaluated-finding` | Every FBX inspected/measured/linted on AnimSmith 0.7.0. |
| Segment | `partially-evaluated` | Individual FBXs used; combined take not promoted. |
| Root motion | `evaluated-finding` | Per-clip root displacement/yaw measured on 188/188 clips (76 moving, 111 stationary, 0 with >1° yaw); action-ownership intent stays open. |
| Conform | `evaluated-clean` | All individual files share one 56-bone structure and import as Unity Humanoid. |
| Validate | `partially-evaluated` | Mechanical contracts complete on the current evaluator; visual combat acceptance open. |
| Optimize | `evaluated-finding` | The current evaluator generated 24 gait candidates and one pruning candidate; both remain unpromoted for Humanoid retarget. All 24 gait candidates loaded as Generic clips in a separate Unity GLB import test. |
| Export | `partially-evaluated` | Current GLBs inspect/measure cleanly and load as Generic clips in a separate new-project import test, but lack Humanoid-retarget or visual acceptance. |
| Gate/report | `evaluated-clean` | Manifest and report pair use parser validation. |

### Readiness evidence by clip set

| Role or runtime set | File-ready / clip-ready | Set-ready / rig-use | Runtime / acceptance boundary |
|---|---|---|---|
| Three locomotion rings | 48/48 parse; delivered loop contracts expose seam failures. | Current circular spreads: Walk 0.7086162→0.0529930, Run 0.6731805→0.1350506, Crouch 0.6184063→0.0587151. | Dated raw Unity samples pass; all 24 IP candidates load as Generic clips, but Humanoid-retarget and visual acceptance remain open. |
| Forward speed alternatives | 10/10 parse and measure. | Same rig; measured 5.42× speed range. | Thresholds and blend quality need controller review. |
| Combat/equipment/actions | 110 non-locomotion files mechanically analyzed. | Common rig; broad dual-wield vocabulary; contacts/events absent. | Full-body default; visual gameplay acceptance open. |
| Prop and mask | Static prop imports at plausible scale (retained); mask graph executes (retained). | Identity attachments to both hands only. Per-bone `bone_channels` coverage narrows attachment risk on the two-prop composition without proving a working mask or attachment. | Grip/orientation, pelvis torque, IK, and hit arcs open. |

### Validation-profile status

| Validation profile | Selection | Result / next evidence |
|---|---|---|
| Marketplace intake | `selected` — `vendor-intended` | Full local archive re-inventoried 2026-08-21; 0 added/removed/changed; current listing still differs from local counts. |
| Blended locomotion | `selected` — `observed-pack-capability` | Retained current evidence reproduces the IP candidate ring spreads to 7 decimals; transformed-engine/visual gates open. |
| Root-motion controller | `selected` — `observed-pack-capability` | Root trajectory measured 188/188 clips (76 moving, 111 stationary, 0 with >1° yaw; `heading_axis` `positive_y` on 187); action ownership open. |
| State-machine transitions | `selected` — `observed-pack-capability` | Equipment and combat states identified; interruption/events open. |
| Layered upper body/weapons | `selected` — `evaluator-selected-generic-scenario` | Retained 2026-08-17 Unity mask executes; the current evaluation per-bone channel coverage narrows attachment risk but does not prove a working mask; full-body remains default. |
| Traversal/environment | `not-selected` | No traversal set in this bounded pack. |
| Contact actions/interactions | `selected` — `observed-pack-capability` | Attacks/parries/blocks present; contact and hit windows open. |
| Retargeted/customizable characters | `selected` — `evaluator-selected-generic-scenario` | Supplied avatar only; target character absent. |
| Motion matching/search | `not-selected` | No database/search target supplied. |
| Networked movement | `not-selected` | No authority/rollback contract supplied. |
| Runtime performance | `selected` — `evaluator-selected-generic-scenario` | Constant tracks diagnosed (current); runtime profiling absent. |

## Pack inventory and content evidence

The Unitypackage materializes 419 collection-relative files: 189 FBXs, 215 Unity metadata files, 11 files in PNG format, three materials, and one animation list. The 186 individual files collapse to 112 logical motions: 74 motions have two IP/RM-labelled files and 38 have one file. All individual motions share skeleton signature `2b6fe49d5ae6` with 56 bones. The combined take and skinned actor share a separate 58-bone export structure; the static sword prop has three nodes. Each of the seven pairwise comparisons found 25 overlapping package paths, all byte-identical.

The pack composes as two swords, one attached per hand, over the shared 56-bone rig. Current per-bone `bone_channels` evidence confirms authored motion on hand and attachment-relevant bones. It narrows mask/socket risk but does not prove a working mask, correct socket transform, or visually accepted attachment.

## Mechanical baseline

| Finding/check | Affected scope | Potential impact | Evidence |
|---|---|---|---|
| Default mechanical pass with constant-track notes | 189/189 FBXs complete the inspect/measure/lint baseline; 25,426 notes in 188 animated files | Export bloat; no default hard blocker. | `observed-animsmith`; current baseline summary, reproduced exactly. |
| Standard mechanical family | `nan`, `time-monotonic`, `quat-norm`, `quat-flip`, `duration-sanity`, `scale-keys`, and `non-uniform-scale` complete on all 189 FBXs; all 189 lint exits are 0 | No reported corrupt samples, time order, quaternion, duration, or animated-scale blocker; no error-severity finding at any scope. | `observed-animsmith`; exhaustive current baseline. |
| Declared loop closure/seam contracts | 186 files linted; 24 exit 0, 162 with findings (matches the published 24/162 exactly); `loop-closure` 67 files (71 findings), `loop-seam-rot` 162 files/findings, `loop-seam-vel` 161 files/findings, coarse `loop-seam` 0 files with findings | Pops/pulses if delivered loop flags are trusted; the coarse `loop-seam` check alone would hide the rotation/velocity seam failures the finer checks expose. | `observed-animsmith`; current contract summary. |
| Loop-seam and in-place applicability/evaluation granularity | `loop_seam_ratio` applicable 168/186, not_applicable 18/186; `loop_seam_evaluation` complete 106, not_evaluated 80; `in_place` applicable 74, not_applicable 112 | No-stride and stationary clips are recorded `not_evaluated` rather than a mislabeled pass or fail — a consumer must not read `not_evaluated` as either. | `observed-animsmith`; measurements v16 availability fields. |
| Directional gait phase | Three IP/RM rings; 24/24 current IP candidates | Raw same-time blends can skate; residuals still require offsets, and no Humanoid-retarget engine import ran (all 24 load as Generic clips instead). | `observed-animsmith`; current raw and candidate summaries. |
| RM action/reaction ownership | Root trajectory measured 188/188 clips: 76 move >1 cm horizontally, 111 stationary, 0 with yaw travel >1°; `heading_axis` is `positive_y` on 187 clips | Measured travel does not establish controller-versus-animation ownership; each RM action/reaction needs explicit per-axis review. These are sampled-grid facts, not continuous-curve or engine-extraction proof. | `observed-animsmith`; measurements v16 `root_trajectory` fields. |

`constant-nonunit-scale` remained disabled, while checks requiring a declared frame rate, required bones, root-speed range, gait/sync group, or other semantic policy were inactive until applicable declarations existed. Their absence is not a pass.

## AnimSmith remediation evidence

| Source issue | Operation/declarations | Result | Independent verification | Remaining caveat |
|---|---|---|---|---|
| Missing semantic loop/in-place context | Per-file `[clips."Take 001"]` declarations derived from Unity loop metadata and observed IP/RM pairs | 24 pass; 162 fail, exposing raw loop policy — reproduced exactly on the current evaluator. | JSON and Markdown results agree for all 186. | Delivered one-shot loop intent still needs curation. |
| Ring phase disagreement | Retained `transform --gait-anchor` on 24 core IP files, AnimSmith 0.7.0 | 24/24 exit 0 and emit GLBs. Circular spread (smallest arc containing the ring): Crouch 0.6184063 → 0.0587151; Run 0.6731805 → 0.1350506; Walk 0.7086162 → 0.0529930. | Inspect, measure, and fix dry-run pass 24/24; lint/diff exit 1 with expected findings/changes. | No RM transform and no Unity Humanoid-retarget import ran that session, though all 24 were staged in a separate new-project GLB import test and each loaded as one Generic clip (see Engine procedures and evidence); visual and trajectory acceptance are open. Candidates remain unpromoted. |
| Dense constant tracks | `transform --prune-constant-tracks` on `Humanoid@IdleCombatDualSwords.fbx`, AnimSmith 0.7.0 | GLB produced; source never modified. | Inspect/measure and fix dry-run exit 0; diff/lint retain expected semantic differences/findings. | Runtime equivalence and sparse transition behavior unproved; bounded by open [#401](https://github.com/mmannerm/animsmith/issues/401). Candidate remains unpromoted. |

Only in-place members were transformed. The current evaluator measures `heading_axis` as `positive_y`, emits all 24 selected candidates, and leaves RM-labelled inputs untouched. Candidates remain unpromoted pending Humanoid-retarget and visual acceptance.

## Engine procedures and evidence

| Runtime | Version | Procedure | Observed result | Remaining gate |
|---|---|---|---|---|
| Unity | Dated Unity 6000.5.8f1 headless import/Playables observation plus current Unity Humanoid revision-1 projection | Import and representative graph execution succeeded for the delivered FBXs; current settings projection is available | Visual playback, target retarget, contacts, full graphs, compression, and build remain open |
| Unity | 6000.5.8f1 (retained 2026-08-17) | Fresh eight-pack project; import all models; inventory importer/avatar/clip state; sample seven clips; run Basic/Sword mixers, a Humanoid upper-body mask, and right/left-hand prop attachments. | 186/186 individual Humanoid clips; seven samples, two mixers, mask, and both attachments pass. | A separate new-project GLB import test covers gait-candidate loading (see below); visual graph, full rings/actions, contacts, Humanoid retarget of the GLB candidates, compression/build remain open. |
| Unity GLB import | 6000.5.8f1 with glTFast 6.9.0 | Load current candidates in a disposable project | Every tested candidate produces one Generic clip | Humanoid retarget, playback, and visual acceptance remain open |
| Unreal Engine | 5.8 (current advice) | `generate import-advice` for the `unreal` revision 2 / `fbx-importer` profile. | Current revision-2 settings projection is available; no engine process ran. | FBX import, retarget, graphs, contacts, build; a future profile revision would need a modeled Unreal setting vocabulary. |
| Godot | 4.7 (current advice) | `generate import-advice` for the `godot` revision 2 / `resource-importer-scene` profile. | Current projections are available for Unity Humanoid revision 2, Unreal revision 2, Godot revision 2, and Bevy revision 3; these are not engine execution evidence. | Conversion/import, retarget, graphs, contacts/export; a future profile revision would need a modeled Godot setting vocabulary. |
| Bevy | 0.19.0 (current addressability) | `animsmith --config <bevy.animsmith.toml> generate addressability <candidate.glb>` for the `bevy` revision 3 / `gltf-asset-loader` profile, run against one generated remediation-trial GLB candidate. | Exit 0; one animation row; coverage complete; predicted selector `Animation0`; facet `available`; 0 findings. This is inventory/selector prediction only — it does not prove a Bevy runtime load, that animation loading was enabled, or that graph wiring is usable. | FBX→glTF pipeline, retarget path, graph, root motion, performance; an actual Bevy runtime load. |

## Rig, masking, and compatibility evidence

| Pack/rig/set pair | Skeleton/retarget | Scale/axes | Root policy | Timing/blend | Overall evidence |
|---|---|---|---|---|---|
| Dual Swords ↔ supplied actor | All 186 individual clips use the collection 56-bone structure and import as Unity Humanoid (retained) | Sword/actor height ratio 0.429 at local identity | IP/RM convention present | Representative execution passes (retained) | Strong candidate; grips and visuals open. |
| Dual Swords ↔ Basic Locomotion | Exact 56-bone structure; shared paths identical | Shared avatar assets identical | Full-body state owns movement | Idle mixer and attack mask execute (retained) | Technical candidate; visual state/mask acceptance open. |
| Dual Swords ↔ Sword & Shield | Exact 56-bone structure; shared paths identical | Props/grips differ and are unreviewed | One active weapon mode owns root | Idle mixer executes (retained) | Prefer full-body weapon-mode switch. |
| Dual attack ↔ Basic mask | Unity Humanoid mapping accepted | Sword attaches to both hands; the current evaluation per-bone channel coverage on the two-prop composition narrows but does not close attachment risk | Basic base owns movement | Headless mask executes (retained) | Prototype only; pelvis/contact/arcs open. |
| Pack ↔ project character | No target character | Not evaluated | Project policy unknown | Not evaluated | Unknown. |

## Limitations and unknowns

1. No target character, camera, controller, quality bar, combat design, hit-window specification, or networking policy was supplied.
2. Headless Unity proves import/execution, not pose quality, feet, deformation, grips, weapon contacts, root behavior, or perceived timing.
3. All current gait and pruning candidates load as Generic clips in a separate Unity project with glTFast. Humanoid retarget, playback, and visual acceptance remain open.
4. Delivered loop metadata is not reliable author intent; strict failures do not mean every flagged clip visibly fails in-game.
5. `_RM` and the measured horizontal-travel/stationary split are naming and sampled-grid evidence, not proof of movement-ownership axes; each RM action/reaction still needs an independent per-clip review.
6. Per-bone `bone_channels` presence narrows mask/attachment risk on the two-prop composition but does not prove a working mask, socket, or attachment.
7. No visual or artistic acceptance evidence is available.
8. Public-page and EULA evidence is dated 2026-08-17 and does not prove the local revision, transaction date, or historical terms.
9. A direct Unity 6000.5.8f1 headless sample of 120 clips observed baked root rotation, baked XZ for nearly every in-place clip, and extracted XZ for most root-motion clips. This is sampled importer evidence, not visual or gameplay acceptance.
10. The integration recipe's `owner=validate-per-axis` step directs the reader to validate root-motion ownership axis by axis rather than assume it. The observed Unity importer locks bake root rotation on every sampled root-motion clip, so animation cannot be assumed to own root-motion yaw. The step is not a per-axis `movement_owner_xz` / `movement_owner_y` / `movement_owner_yaw` declaration, and no such declaration is derived from measured travel in this refresh. Measured root displacement and yaw are recorded as sampled facts only; choosing the per-axis owner remains a project and engine decision. The 2026-08-21 observed Unity root-lock policy (Engine procedures and evidence) independently corroborates this split on the sampled clips — but that is corroborating engine evidence for the recipe, not a licence to declare per-axis `movement_owner_*` values, and none is declared here.
11. The new-project GLB import test (134/134 candidates, 24 from this pack) proves glTFast produces one well-formed Generic AnimationClip per candidate; it does not test this pack's Humanoid retarget path and is not visual or gameplay acceptance.

## Changes between AnimSmith versions

| Evaluator | Change from the preceding evaluated state |
|---|---|
| AnimSmith 0.7.0 | Revalidated the 189-FBX baseline, 186 declared contracts, 24 gait candidates, pruning trial, and current engine projections under output v17 / measurements v16. |
| AnimSmith 0.4.1 | Reproduced the evaluated 0.4.0 results for this corpus; unrelated release fixes did not change the pack conclusion. |
| AnimSmith 0.4.0 | Consolidated baseline, contract, and gait evidence on one released evaluator and retained the same post-anchor phase results. |
| AnimSmith 0.3.x | Established the initial baseline and first gait-remediation trial. Those evaluator results are superseded. |

## Reproduction

### Current AnimSmith reproduction (2026-08-26)

The unchanged corpus was rerun with `animsmith 0.7.0`, exact tag `v0.7.0`, commit `461ac8a4f6bb368eb8637471a796f13eeb647140`, binary SHA-256 `01a501999c91d93abfb32b1f48241fccc70914fac27c9a650c31df44262578d8`, output schema v17, and measurements schema v16.

| Current external evidence | SHA-256 | Result |
|---|---|---|
| Baseline command envelope | `4434feed1f9dd7b26d0b3151064e3843c57bc67363488f8d1d8293e8a663af6b` | 189 FBXs; all commands complete |
| Declared contracts | `feb394d9bf66c0b58a1c53406fb69f39119e1be9d9c5ddba1a1ba07e816a380e` | 186 files; 24 pass / 162 fail |
| Remediation | `074018e7ad446330c3d10bc3f0b5664bcbbc0fce10b10bc0fd4f5f35c8ad4653` | 25 candidates completed and verified |
| 0.7 supplemental projections | `c24514d418fc808ea6aa5efabb2b256ca62ca370302756d8ae492ed068b96eb5` | 25 addressability V1 + rich V2 pairs; exact-profile advice available |
| Refreshed legacy manifest | `614aed097308a1928b62b2b1a90b8f4b0bca991cf25d04a273e622d6fcccd018` | Valid schema; 112 logical motions |

The current projections do not evaluate weapon contact, runtime graph wiring, target survival, retarget deformation, or visual acceptance.

## Sources

- Local source archive and extracted Unity metadata — private authorized input identified above, accessed 2026-08-17, re-verified 2026-08-21.
- Protofactor, [Dual Swords](https://protofactor.biz/product/animset-dual-swords/), [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/), and [End User License Agreement](https://protofactor.biz/end-user-license-agreement/) — current listing/license context, not local revision proof.
- Unity, [Avatar Mask](https://docs.unity3d.com/6000.0/Documentation/Manual/class-AvatarMask.html), [Animation Layers](https://docs.unity3d.com/6000.0/Documentation/Manual/AnimationLayers.html), and [Root Motion](https://docs.unity3d.com/6000.5/Documentation/Manual/RootMotion.html) — runtime capabilities.
- Epic Games, [Root Motion](https://dev.epicgames.com/documentation/unreal-engine/root-motion-in-unreal-engine?application_version=5.7) and [Layered Animations](https://dev.epicgames.com/documentation/unreal-engine/using-layered-animations-in-unreal-engine?lang=en-US); Godot, [AnimationTree](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html); Bevy, [Animation Masks](https://bevy.org/examples/animation/animation-masks/) — documentation-only capability.
