# Animation pack evidence appendix: Protofactor 2-Handed Melee Weapon Animset

> Companion report: [technical evaluation](protofactor-two-handed-melee.md)
>
> Evidence status: **partial** — exact AnimSmith 0.7.0 baseline, declared contracts, remediation verification, addressability, and bounded advice plus a dated Unity 6000.5.8f1 observation; transformed-clip visual acceptance and engine-editor/runtime passes remain unevaluated.
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
| Pack/edition | Local `Animset@2HandedMeleeWeapon_PACKAGE.unitypackage`; constituent revision is not declared |
| Vendor/source | Protofactor; [current 2-Handed Melee product page](https://protofactor.biz/product/animset-2-handed-melee-weapon/) |
| Delivered scope | Full local RAR → one Unitypackage → 123 FBXs: 120 individual motions, one combined animation FBX, one skinned actor, and one sword prop; Unity metadata/materials/textures included |
| Target use | Game-engine use; third-person two-handed combat and combination with the other seven evaluated constituents |
| Target engines | Dated Unity 6000.5.8f1 observation; current Unity Humanoid revision-1, Unreal revision-2, and Godot revision-2 settings projections; Bevy revision-3 rich addressability |
| Target rigs/packs | Supplied Protof-Actor, Basic Locomotion, Sword & Shield, and the evaluated collection subset |
| Source manifest | `logical-assets-inventory.json`; SHA-256 `3e21bdc9d8f8bb463fdef8eb7760551bf733d28678c7d2abd093e620e226b347` (re-verified 2026-08-21) |
| Evaluation manifest | `evidence/evaluation-manifest.json`; SHA-256 `0121fd1d73e46646c6fd585954bd2fab7744c51f6bac86c6d0ac6504108abd82`; schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1`; taxonomy/profile-set version 1 |
| Acquisition/license provenance | `user-stated`: local archive was downloaded from Protofactor.biz as part of the Ultimate Animation Collection. Current EULA permits protected released real-time applications and modification while restricting redistribution/resale; no receipt, download date, historical EULA, or local constituent revision was retained. Technical due diligence only, not legal advice. |

The current evaluator is AnimSmith 0.7.0, tag `v0.7.0`, commit `461ac8a4f6bb368eb8637471a796f13eeb647140`, binary SHA-256 `01a501999c91d93abfb32b1f48241fccc70914fac27c9a650c31df44262578d8`, output schema v17, and measurements schema v16, captured 2026-08-26. It produced the complete baseline, contract, remediation, and projection evidence in this appendix.

The current product page, observed 2026-08-17, advertises USD 19.99, 118 animations, 48 root-motion and 70 in-place files, Unity Humanoid, Unity 2018.4.2+, and no native UE4 package. The local delivery has 120 individual files and 48 `_RM` labels. The 118-file listing count equals the observed Humanoid subset, while the other two local files are Generic; that numerical alignment does not prove revision identity or vendor intent.

The evaluation manifest uses schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1`.

### Evidence coverage

| Surface | Offered/delivered | Evaluated | Findings | Not evaluated and why |
|---|---:|---:|---:|---|
| Animation files | 122 animation-bearing FBXs | 122 | 120 individual plus combined take and actor | Continuous artistic review of all motion |
| Rigs/export variants | 4 observed structures | 4 | Dominant 58-bone, two 56-bone outliers, actor/combined 58-bone variant, prop | Target-character deformation and non-Unity retarget |
| AnimSmith baseline | 123 FBXs | 123 | All commands exit 0; root trajectory 122/122, gait 121/122; 17,010 constant-track notes | Artistic intent and contacts |
| Declared contracts | 120 individual files | 120 | 13 pass, 107 fail; normalized loop-seam findings in 4 files | Human loop intent for every action |
| Offline visual reports | 122 possible | 5 generated; 0 visually accepted | No visual conclusions claimed | Continuous playback/contact/deformation review |
| Engine import/playback | 120 individual files | 120 imported; 7 sampled; 1 import-advice + 1 addressability probe | 118 Humanoid; 2 expected Generic failures; Unity advice available, Unreal/Godot advice refused, Bevy addressability available | Visual playback, editor import, and runtime load for Unreal/Godot/Bevy |
| Blend/mask/retarget | 4 selected probes | 4 headless | 2 mixers, 1 mask, 1 prop execute | Visual blending, contacts, twist deformation, target rigs |

### Claim legend

Claims use the versioned evidence labels from the skill's assessment taxonomy: `user-stated`, `observed-file`, `observed-animsmith`, `observed-report`, `observed-engine`, `vendor-stated`, `documentation-stated`, `inferred`, and `not-evaluated`.

## Evaluation manifest and taxonomy

### Canonical clip-role inventory

| Canonical primary role | Logical motions | Delivered files | Evidence boundary |
|---|---:|---:|---|
| `idle-pose` | 8 | 8 | Filename/metadata classification; includes two block-pose outliers |
| `continuous-locomotion` | 28 | 56 | Direct paired IP/RM names; paired intent does not prove track-only difference |
| `locomotion-transition` | 0 | 0 | None delivered |
| `airborne` | 5 | 5 | Jump, fall, and landing naming evidence |
| `traversal` | 0 | 0 | None delivered |
| `action-interaction` | 22 | 34 | Attacks, parries, dodges, equipment, and taunts; contact intent unverified |
| `reaction-death` | 13 | 17 | Hit/death names; several RM counterparts |
| `emote-cinematic` | 0 | 0 | Taunts classified as gameplay actions in this combat pack |
| `other-unknown` | 0 | 0 | Every individual motion received one role |
| **Total** | **76** | **120** | Validated v1 manifest SHA-256 `0121fd1d73e46646c6fd585954bd2fab7744c51f6bac86c6d0ac6504108abd82` |

### Runtime-set inventory

Exact members and measurements for every promoted set are in the [primary runtime-set table](protofactor-two-handed-melee.md#runtime-sets-and-authored-motion).

| Runtime set | Type | Members/variants | Grouping evidence | Validation status |
|---|---|---|---|---|
| Walk combat 8-way | `directional-blend` | 8 IP/RM direction pairs | Exact directional names; common 1.333 s duration | Raw circular spread 0.7111863; the current evaluator anchoring reduces it to 0.0693366; runtime visual blend not evaluated |
| Run combat 8-way | `directional-blend` | 8 IP/RM direction pairs | Exact directional names; 0.533–0.567 s duration | Raw circular spread 0.6024028; the current evaluator anchoring reduces it to 0.1429141; runtime visual blend not evaluated |
| Crouch combat 8-way | `directional-blend` | 8 IP/RM direction pairs | Exact directional names; common 1.667 s duration | Raw circular spread 0.5773874; the current evaluator anchoring reduces it to 0.0537579; runtime visual blend not evaluated |
| Normal forward speed | `speed-blend` | Walk/run/sprint IP/RM pairs | Exact gait names and measured speeds | Measured; thresholds/controller policy unaccepted |
| Draw/combat/put-away | `transition-chain` | Draw, `IdleCombatA`, put-back | Delivered names imply order | Headless members execute; crossfades not evaluated |
| Dodge forward/back | `other` | Forward/back IP/RM pairs | Exact action/direction names | Measured; contacts and interruption not evaluated |
| Parry 3-way | `other` | Front/left/right IP files | Exact directional names | Timing measured; weapon contact not evaluated |

### Pipeline-stage coverage

| Stage | Coverage state | Evidence / remaining gate |
|---|---|---|
| Acquire | `evaluated-clean` | User-authorized local vendor archive retained outside the repository with a digest |
| Preserve raw | `evaluated-clean` | Untouched RAR and extracted Unitypackage retained separately from generated evidence |
| Inspect | `evaluated-finding` | Every FBX inspected, measured, and linted; exact profile and role-resolution policies retained |
| Segment | `partially-evaluated` | Individual FBXs are runtime sources; combined-take segmentation not promoted |
| Root motion | `evaluated-finding` | All labeled variants inventoried and measured; 45/121 clips move >1 cm, 76 are stationary, and 0 have >1° yaw; ownership still requires an explicit per-axis project declaration |
| Conform | `evaluated-finding` | Skeleton signatures, case-tolerant resolution provenance, twist bones, and Unity rig exceptions recorded |
| Validate | `partially-evaluated` | Mechanical/contract work exhaustive; visual combat, masks, contacts, and transitions remain |
| Optimize | `evaluated-finding` | Twenty-four current gait candidates and one prune candidate exported; runtime/equivalence acceptance remains open |
| Export | `partially-evaluated` | Generated GLBs are evidence only, not adopted production candidates; a new-project GLB import test confirms all 24 gait candidates load as Generic clips (see Engine procedures and evidence) |
| Gate/report | `evaluated-clean` | Manifest and linked report pair parser-validated |

### Readiness evidence by clip set

| Role or runtime set | File-ready / clip-ready | Set-ready / rig-use | Runtime / acceptance boundary |
|---|---|---|---|
| 118 dominant clips | Core mechanical checks complete; the default humanoid profile resolves the capitalized names through unique ASCII-case-insensitive aliases; declared loop policy fails broadly | Common 58-bone signature with two forearm-twist bones | Unity Humanoid import and seven samples pass (retained); import-advice available; visual/target-rig acceptance open |
| Two block clips | FBX-readable, but no explicit Unity clip definition or Humanoid import | Collection 56-bone signature, different from pack majority | Quarantined; corrected author exports required |
| Three locomotion rings | Durations and RM speeds measured; strict loop findings remain | Raw circular spreads 0.711/0.602/0.577; current anchoring reduces them to 0.069/0.143/0.054 | Keep RM raw; transformed IP engine/visual acceptance and residual offsets remain; candidates unpromoted |
| Normal forward speed | Three paired gaits measured; 6.27× speed range | Shared dominant skeleton | Controller thresholds, playback scaling, and foot contacts open |
| Actions/reactions | File-ready except one timing-span warning; delivered loop intent unreliable | Full-body use; two-handed contact and yaw semantics unknown | Headless samples only; hit windows, grip, cancels, and visual quality open |
| Draw/combat/put-away | All members readable; two chain members pass declared contracts | Shared skeleton and right-hand attachment | State transitions and visible equipment continuity open |

### Validation-profile status

| Validation profile | Selection | Result / next evidence |
|---|---|---|
| Marketplace intake | `selected` — `vendor-intended` | Full local delivery inventoried, hashed, and mechanically evaluated |
| Blended locomotion | `selected` — `observed-pack-capability` | Three rings and one speed family measured; visual blend acceptance remains |
| Root-motion controller | `selected` — `observed-pack-capability` | RM speed and root trajectory measured (45/121 moving, 76 stationary, 0 with >1° yaw); per-axis ownership declaration and engine extraction remain |
| State-machine transitions | `selected` — `observed-pack-capability` | Boundaries catalogued; runtime crossfades and interruption remain |
| Layered upper body/weapons | `selected` — `evaluator-selected-generic-scenario` | One headless mask executes; full-body remains default |
| Traversal/environment | `not-selected` | No traversal role; airborne clips alone do not establish traversal |
| Contact actions/interactions | `selected` — `observed-pack-capability` | Contacts, events, weapon arcs, and cancels remain |
| Retargeted/customizable characters | `selected` — `evaluator-selected-generic-scenario` | Unity mixers execute; target meshes/twist deformation remain |
| Motion matching/search | `not-selected` | No target system or authored search metadata |
| Networked movement | `not-selected` | No networking contract or runtime evidence |
| Runtime performance | `selected` — `evaluator-selected-generic-scenario` | Platform memory, compression, and frame cost remain |

## Pack inventory and content evidence

The source RAR contains one Unitypackage. Materialization produced 123 FBXs plus Unity metadata, textures, materials, and support files. The evaluation treats the 120 individual motion files as the runtime inventory; the combined animation FBX and skinned actor are inspected delivery evidence, not substitutes for the individual clips. The static sword prop is not a motion.

The manifest groups 44 IP/RM file pairs and 32 single-file motions into 76 logical motions. A suffix or matched duration supports counterpart intent but does not prove that translation is their only difference. The seven runtime sets are orthogonal to the canonical roles.

The dominant 118-file skeleton has 58 bones, signature `3da84463466a`, capitalized `Humanoid_` names, and left/right forearm-twist bones. `Humanoid@Blocked2HandMelee.fbx` and `Humanoid@IdleBlock2HandMelee.fbx` instead have the collection's 56-bone signature `2b6fe49d5ae6`; Unity metadata imports both as Generic without explicit clip definitions. The actor/combined variant has another 58-bone signature, and the prop has three nodes.

Unity metadata marks 108/120 individual files as loops. A conservative semantic classification identifies 44 obvious one-shot-like flags: 32 action/interaction files and 12 reaction files. This is a starting override list, not complete proof of the other 64 loop declarations.

## Mechanical baseline

All numbers below are current AnimSmith 0.7.0 results captured on 2026-08-26.

The default humanoid profile resolves the dominant rig's case-only bone-name
variants with `ascii-case-insensitive` provenance and retains every exact
delivered name. Root trajectory is measured on 122/122 clips; gait and speed
are measured on 121/122, with one non-motion delivery row unavailable. The
measured local heading witness is `yaw_heading_axis = positive_y`.

| Finding/check | Affected scope | Potential impact | Evidence |
|---|---|---|---|
| `nan`, `quat-norm`, `quat-flip`, `scale-keys`, `non-uniform-scale` | 123/123 baseline FBXs evaluated; no findings | No defect established at these mechanical gates | `observed-animsmith`; all JSON/Markdown commands exit 0 |
| `duration-sanity` + `time-monotonic` | `Humanoid@GetHitLeftHeavy2HandMelee_RM.fbx` | Two forearm-twist tracks first-key at 0.033 s; channel ends span 1.700–1.733 s, so the shorter tracks clamp-hold | `observed-animsmith`; one `duration-sanity` warning, six `time-monotonic` notes; all lint exits 0 |
| `constant-track` | 122 animation-bearing files; 17,010 notes | Export size/evaluation overhead; pruning may alter sparse-track semantics | `observed-animsmith`; all current baseline commands exit 0 |
| Default rig-role resolution | 118/118 dominant individual motions resolve as `humanoid` | Makes gait/root/loop-seam measurements available without project role overrides | `observed-animsmith`; root is exact and the nine capitalized humanoid bindings record `ascii-case-insensitive` provenance |
| Declared loop contracts | 120 individual files; 13 pass, 107 fail | Pose/velocity wraps, false loops, or intentionally strict policy failures | 48 loop-closure files, 4 normalized-seam files, 106 rotation-seam files, 101 velocity-seam files |
| Directional set phase (circular spread) | 24 core IP files across 3 gait families | Blending unaligned contacts can skate or pulse once per cycle | Raw: Crouch 0.5773874, Run 0.6024028, Walk 0.7111863 |
| Directional RM speed | 24 core RM files | Equal input magnitude may produce direction-dependent travel | Ratios: walk 1.35×, run 1.22×, crouch 1.30× |

The base mechanical family ran exhaustively. Contract checks ran only where declarations made them applicable; a not-applicable check is not a pass. The default profile's fail-closed case-tolerant resolution supplies the current role-backed measurement coverage without changing source bytes.

## AnimSmith remediation evidence

| Source issue | Operation/declarations | Result | Independent verification | Remaining caveat |
|---|---|---|---|---|
| Capitalized dominant roles | Current built-in humanoid profile | Resolves all 118 dominant files through unique case-only aliases; no asset output or project override | Structured output retains the exact delivered names plus `exact` or `ascii-case-insensitive` policy per role | Ambiguity remains fail-closed; this evidence does not prove target-rig retarget quality |
| Directional gait phase | Current `transform --gait-anchor` on 24 core IP files under declared contracts | 24/24 exit 0 and emit GLBs | Inspect/measure/fix dry-run 24/24 exit 0; post-anchor circular spreads Crouch 0.0537579, Run 0.1429141, Walk 0.0693366; lint/diff retain remaining contract findings and intentional rewrite deltas | Only IP transformed; all 24 load as Generic clips, but Humanoid-retarget, visual/contact, and trajectory acceptance remain open |
| Constant tracks | `transform --prune-constant-tracks` on `Humanoid@IdleCombatA2HandMelee.fbx`, the current evaluator | Candidate GLB produced; transform exit 0 | Inspect/measure/fix dry-run exit 0; diff/lint remain nonzero as expected | Bounded by open [#401](https://github.com/mmannerm/animsmith/issues/401); no engine playback, property-equivalence proof, or production adoption |
| Root trajectory | Current `measure` under the built-in humanoid profile | 45/121 clips move >1 cm horizontally, 76 stationary, 0 with >1° yaw travel | Sampled `MetricGrids` regression facts, not continuous-curve or engine-extraction proof | Do not infer movement ownership from travel; declare `movement_owner_xz`, `movement_owner_y`, and `movement_owner_yaw` from the project contract |


## Engine procedures and evidence

| Runtime | Version | Procedure | Observed result | Remaining gate |
|---|---|---|---|---|
| Unity (retained, dated 2026-08-17) | 6000.5.8f1 | Materialize all eight evaluated Unitypackages into one external project; inventory importers/clips; execute Playables samples, two cross-pack mixers, one AvatarMask graph, expected rig-outlier checks, and sword attachment | 118/120 individuals import as Humanoid; seven samples, both mixers, mask, and prop execute; both Generic block clips fail the Humanoid precondition as expected | A separate new-project GLB import test covers gait-candidate loading (see below); visual motion/contact/twist review, full graphs, root extraction, target-character retarget of the GLB candidates, compression, build remain open |
| Unity | Dated Unity 6000.5.8f1 headless import/Playables observation plus current Unity Humanoid revision-1 projection | Import and representative graph execution succeeded for the delivered FBXs; current settings projection is available | Visual playback, target retarget, contacts, full graphs, compression, and build remain open |
| Unity GLB import | 6000.5.8f1 with glTFast 6.9.0 | Load current candidates in a disposable project | Every tested candidate produces one Generic clip | Humanoid retarget, playback, and visual acceptance remain open |
| Unreal | rev 2 / 5.8 | `generate import-advice` under the `unreal` profile on the same FBX | Current revision-2 settings projection is available; no engine process ran. | FBX import, IK Rig/Skeleton mapping including twists, Blend Spaces, montages, contacts, build |
| Godot | rev 2 / 4.7 | `generate import-advice` under the `godot` profile on the same FBX | Current revision-2 settings projection is available; no engine process ran. | FBX-to-supported route, Skeleton3D mapping, blend spaces, filters, root motion, export |
| Bevy | rev 3 / 0.19.0 | Current rich addressability on a generated GLB candidate | Available with 64-bit target UUIDs; sealed inventory only | Target survival, retarget strategy, graph/root policy wiring, runtime load, performance |

The dated Unity probe establishes headless import and graph execution only, not visible playback quality, contacts, foot plants, two-hand alignment, mask usefulness, retarget deformation, or shipping-build behavior. Current Unity revision-1, Unreal revision-2, and Godot revision-2 projections are settings evidence only; Bevy revision-3 addressability is sealed inventory only. All 24 gait candidates load as Generic clips, but Humanoid retarget and visual acceptance remain open.

## Rig, masking, and compatibility evidence

| Pack/rig/set pair | Skeleton/retarget | Scale/axes | Root policy | Timing/blend | Overall evidence |
|---|---|---|---|---|---|
| 118 dominant clips with each other | Same exact 58-bone signature | Unity scale 1 metadata; no visible scale review | Paired IP/RM labels and measured horizontal speed | Common per-set durations except 0.533/0.567 run split; phases differ | Structurally direct; runtime blend/contact acceptance unknown |
| Two block clips → dominant runtime | 56-bone structure; Unity Generic versus 58-bone Humanoid majority | No separate scale finding | No explicit Unity clip definition | No mixer test because Humanoid precondition fails | Quarantined; author/vendor correction needed |
| Basic Locomotion idle → 2H combat idle | Unity Humanoid mixer executes | Common project; visual scale unreviewed | Stationary handoff | Headless crossfade only | `observed-engine`; full-body handoff candidate, not visual acceptance |
| Sword & Shield idle → 2H combat idle | Unity Humanoid mixer executes despite 56/58 source hierarchy difference | Common project; visual scale unreviewed | Stationary handoff | Headless crossfade only | `observed-engine`; engine-config path, twist deformation unknown |
| Basic walk + 2H AttackA mask | Unity AvatarMask graph executes | Common project; visual scale unreviewed | Basic controls root/lower body in graph | No pelvis, grip, or contact visual proof | Candidate only; full-body action remains default |
| Supplied sword → right hand | Dominant rig exposes mapped right hand | Prop/character height ratio 0.302 | Not applicable | Static headless attachment | Plausible scale only; grip/orientation/contact unaccepted |
| Eight-pack package coexistence | Pairwise overlapping logical paths are byte-identical | No path-level conflict | Pack-specific policies remain | Co-import succeeds | Positive packaging evidence; not motion/style compatibility |

The dominant rig's two forearm-twist bones distinguish it from the 56-bone Basic Locomotion, Sword & Shield, Dual Swords, and one-handed majority. Unity Humanoid abstraction allowed sampled cross-pack mixers, but shared humanoid status does not prove rest-pose, deformation, hand-contact, or style compatibility on a target character.

## Limitations and unknowns

1. No target character, gameplay controller, camera, combat timing contract, or network model was supplied.
2. Offline reports were generated for five risk-selected files but were not used for visual acceptance.
3. Unity testing was headless. A separate glTFast project confirmed all 24 current gait candidates load as Generic clips; complete graphs, root extraction, contacts, Humanoid retarget, compression, and builds remain open.
4. Unreal Engine, Godot, and Bevy have no editor/runtime evaluation; the 2026-08-21 rows are AnimSmith import-advice/addressability capability queries only.
5. The two block clips need corrected runtime-ready exports or substitution.
6. The heavy-left-hit RM channel-span anomaly needs target-rig visual review.
7. RM yaw, vertical displacement, weapon arcs, hit events, IK, cancels, and two-handed grip were not accepted.
8. Current public pages/EULA do not prove the local revision, transaction date, or historical terms.
9. A direct Unity 6000.5.8f1 headless sample of 120 clips observed baked root rotation, baked XZ for nearly every in-place clip, and extracted XZ for most root-motion clips. This is sampled importer evidence, not visual or gameplay acceptance.
10. Built-in case-tolerant role resolution is limited to unique ASCII-case-only matches and refuses ambiguity; it is not fuzzy retargeting or evidence of target-rig deformation quality.
11. The integration recipe's `owner=validate-per-axis` step directs the reader to validate root-motion ownership axis by axis rather than assume it. The observed Unity importer locks bake root rotation on every sampled root-motion clip, so animation cannot be assumed to own root-motion yaw. The step is not a per-axis `movement_owner_xz` / `movement_owner_y` / `movement_owner_yaw` declaration, and no such declaration is derived from measured travel in this refresh. Measured root displacement and yaw are recorded as sampled facts only; choosing the per-axis owner remains a project and engine decision. The 2026-08-21 observed Unity root-lock policy (Engine procedures and evidence) independently corroborates this split on the sampled clips — but that is corroborating engine evidence for the recipe, not a licence to declare per-axis `movement_owner_*` values, and none is declared here.
12. The new-project GLB import test (134/134 candidates, 24 from this pack) proves glTFast produces one well-formed Generic AnimationClip per candidate; it does not test this pack's Humanoid retarget path and is not visual or gameplay acceptance.

## Changes between AnimSmith versions

| Evaluator | Change from the preceding evaluated state |
|---|---|
| AnimSmith 0.7.0 | Revalidated the 123-FBX baseline, 120 declared contracts, 25 candidates, and current engine projections under output v17 / measurements v16. The built-in humanoid profile now resolves the dominant capitalized rig through unique fail-closed case-only aliases, so the explicit-role workaround is no longer required. |
| AnimSmith 0.4.1 | Reproduced the evaluated 0.4.0 measurements and transforms for this corpus; unrelated release fixes did not change the pack conclusion. |
| AnimSmith 0.4.0 | Required an explicit role map for the dominant rig, then produced the root/gait/loop evidence and candidates retained as historical comparison only. |
| AnimSmith 0.3.x | Established the initial baseline and first gait-remediation trial. Those evaluator results are superseded. |

## Reproduction

### Current AnimSmith reproduction (2026-08-26)

The unchanged corpus was rerun with `animsmith 0.7.0`, exact tag `v0.7.0`, commit `461ac8a4f6bb368eb8637471a796f13eeb647140`, binary SHA-256 `01a501999c91d93abfb32b1f48241fccc70914fac27c9a650c31df44262578d8`, output schema v17, and measurements schema v16. The built-in humanoid profile resolved the dominant 58-bone family without an explicit role override and recorded exact case-resolution provenance.

| Current external evidence | SHA-256 | Result |
|---|---|---|
| Baseline command envelope | `5be8ffa0b9173665830528d0724b663eeee115f9cf3153d750a60122a9ad2c86` | 123 FBXs; all commands complete |
| Declared contracts | `b6556d9b3bb4510dc5f1ab7f48f7a3121eee2855d3d582b1fc69aab7778cd81e` | 120 files; 13 pass / 107 fail |
| Remediation | `9425884f4564b5fac32b695394310e716397843dc8bcd8a7788e19432912f7f8` | 25 candidates completed and verified |
| 0.7 supplemental projections | `a6aac306e5f7bca5e596fead8f054cd4999652229f846cdce0c3f6547e007240` | 25 addressability V1 + rich V2 pairs; exact-profile advice available |
| Refreshed legacy manifest | `993bfc545d0d76e849cabd3cecc714ced3034c8d80fcad6dec14b12b0003d66b` | Valid schema; 76 logical motions |

The current projections do not evaluate weapon contact, runtime graph wiring, target survival, retarget deformation, or visual acceptance.

## Sources

- Local source archive and extracted Unity metadata — private authorized input identified above, accessed 2026-08-17; re-verified byte-identical 2026-08-21.
- Protofactor, [2-Handed Melee Weapon](https://protofactor.biz/product/animset-2-handed-melee-weapon/), [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/), and [End User License Agreement](https://protofactor.biz/end-user-license-agreement/) — current listing/license context, not local revision proof.
- Unity, [Avatar Mask](https://docs.unity3d.com/6000.0/Documentation/Manual/class-AvatarMask.html), [Animation Layers](https://docs.unity3d.com/6000.0/Documentation/Manual/AnimationLayers.html), and [Root Motion](https://docs.unity3d.com/6000.5/Documentation/Manual/RootMotion.html) — runtime capabilities.
- Epic Games, [Root Motion](https://dev.epicgames.com/documentation/unreal-engine/root-motion-in-unreal-engine?application_version=5.7) and [Layered Animations](https://dev.epicgames.com/documentation/unreal-engine/using-layered-animations-in-unreal-engine?lang=en-US); Godot, [AnimationTree](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html); Bevy, [Animation Masks](https://bevy.org/examples/animation/animation-masks/) — documentation-only capability.
