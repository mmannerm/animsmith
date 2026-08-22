# Animation pack evidence appendix: Protofactor 2-Handed Melee Weapon Animset

> Companion report: [technical evaluation](protofactor-two-handed-melee.md)
>
> Evidence status: **partial** — exhaustive 0.4.0 baseline/contracts/remediation on one frozen evaluator (default and explicit-role passes), retained Unity 6000.5.8f1 eight-pack evidence, new unity-humanoid/unreal/godot/bevy import-advice/addressability probes, a corrected observed Unity root-lock policy, and a new-project GLB import test; transformed-clip visual acceptance and three engine editors/runtimes remain unevaluated.
>
> Evaluation date: **2026-08-21**
>
> Report format: **1**

This appendix preserves the detailed evidence behind the concise report. The [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder) remains authoritative.

## Evaluation scope and provenance

| Field | Value |
|---|---|
| Pack/edition | Local `Animset@2HandedMeleeWeapon_PACKAGE.unitypackage`; constituent revision is not declared |
| Vendor/source | Protofactor; [current 2-Handed Melee product page](https://protofactor.biz/product/animset-2-handed-melee-weapon/) |
| Delivered scope | Full local RAR → one Unitypackage → 123 FBXs: 120 individual motions, one combined animation FBX, one skinned actor, and one sword prop; Unity metadata/materials/textures included |
| Target use | Game-engine use; third-person two-handed combat and combined use with the seven previously evaluated constituents |
| Target engines | Unity 6000.5.8f1 observed (retained 2026-08-17); `unity-humanoid` rev 1/6000.3, `unreal` rev 1/5.8, `godot` rev 1/4.7, and `bevy` rev 1/0.19.0 import-advice/addressability probes (2026-08-21) |
| Target rigs/packs | Supplied Protof-Actor, Basic Locomotion, Sword & Shield, and the evaluated collection subset |
| Source manifest | `logical-assets-inventory.json`; SHA-256 `3e21bdc9d8f8bb463fdef8eb7760551bf733d28678c7d2abd093e620e226b347` (re-verified 2026-08-21) |
| Evaluation manifest | `evidence/evaluation-manifest.json`; SHA-256 `0121fd1d73e46646c6fd585954bd2fab7744c51f6bac86c6d0ac6504108abd82`; schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1`; taxonomy/profile-set version 1 |
| Acquisition/license provenance | `user-stated`: local archive was downloaded from Protofactor.biz as part of the Ultimate Animation Collection. Current EULA permits protected released real-time applications and modification while restricting redistribution/resale; no receipt, download date, historical EULA, or local constituent revision was retained. Technical due diligence only, not legal advice. |

**Evaluator identity:** AnimSmith `0.4.0`, tag `v0.4.0`, revision `6b37ad636b198ef8ff47fadbf6a3a51eb1a27c8e`, binary SHA-256 `fd1eee57407aa02db88763d144389a7f5104204c40ddfbb28eb5885ca8cd54c6`, output schema v10 nesting measurements schema v15, captured 2026-08-21. This single frozen evaluator now produces the complete baseline, contract, and remediation evidence in this appendix, replacing the earlier mixed story of a `b7c215b` baseline/contracts run plus a `674396f` gait-only pre-release pass; those earlier results are retained below only as dated historical comparison.

**Source identity re-verification:** a 2026-08-21 inventory re-run reproduces the published manifest exactly — 0 added, 0 removed, 0 changed across all 123 FBXs. Archive, package, and logical-manifest digests all re-verify unchanged (see Reproduction). The explicit `[rig.roles]` config digest is also unchanged: `667799ff3e6ccbe29306fe70bce0fb85bb5686215387259b0ca8d63694d5a9cd`. This is a pure evaluator-version refresh, not a new source revision.

The current product page, observed 2026-08-17, advertises USD 19.99, 118 animations, 48 root-motion and 70 in-place files, Unity Humanoid, Unity 2018.4.2+, and no native UE4 package. The local delivery has 120 individual files and 48 `_RM` labels. The 118-file listing count equals the observed Humanoid subset, while the other two local files are Generic; that numerical alignment does not prove revision identity or vendor intent.

The evaluation manifest uses schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1`.

### Evidence coverage

| Surface | Offered/delivered | Evaluated | Findings | Not evaluated and why |
|---|---:|---:|---:|---|
| Animation files | 122 animation-bearing FBXs | 122, default and explicit-role passes | 120 individual plus combined take and actor | Continuous artistic review of all motion |
| Rigs/export variants | 4 observed structures | 4 | Dominant 58-bone, two 56-bone outliers, actor/combined 58-bone variant, prop | Target-character deformation and non-Unity retarget |
| AnimSmith baseline | 123 FBXs | 123, both passes | All commands exit 0; root trajectory 4/122 (default) vs. 122/122 (explicit); 17,010 constant-track notes identical in both passes | Artistic intent and contacts |
| Declared contracts | 120 individual files | 120, both passes | 13 pass, 107 fail identically in both passes; loop-seam findings 0 files (default) vs. 4 files (explicit) | Human loop intent for every action |
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
| Walk combat 8-way | `directional-blend` | 8 IP/RM direction pairs | Exact directional names; common 1.333 s duration | Raw circular spread 0.7111863; released 0.4.0 anchoring reduces it to 0.0693366 (matches pre-release `674396f` to 7 decimals); runtime visual blend not evaluated |
| Run combat 8-way | `directional-blend` | 8 IP/RM direction pairs | Exact directional names; 0.533–0.567 s duration | Raw circular spread 0.6024028; released 0.4.0 anchoring reduces it to 0.1429141 (matches pre-release `674396f` to 7 decimals); runtime visual blend not evaluated |
| Crouch combat 8-way | `directional-blend` | 8 IP/RM direction pairs | Exact directional names; common 1.667 s duration | Raw circular spread 0.5773874; released 0.4.0 anchoring reduces it to 0.0537579 (matches pre-release `674396f` to 7 decimals); runtime visual blend not evaluated |
| Normal forward speed | `speed-blend` | Walk/run/sprint IP/RM pairs | Exact gait names and measured speeds | Measured; thresholds/controller policy unaccepted |
| Draw/combat/put-away | `transition-chain` | Draw, `IdleCombatA`, put-back | Delivered names imply order | Headless members execute; crossfades not evaluated |
| Dodge forward/back | `other` | Forward/back IP/RM pairs | Exact action/direction names | Measured; contacts and interruption not evaluated |
| Parry 3-way | `other` | Front/left/right IP files | Exact directional names | Timing measured; weapon contact not evaluated |

### Pipeline-stage coverage

| Stage | Coverage state | Evidence / remaining gate |
|---|---|---|
| Acquire | `evaluated-clean` | User-authorized local vendor archive retained outside the repository with a digest |
| Preserve raw | `evaluated-clean` | Untouched RAR and extracted Unitypackage retained separately from generated evidence |
| Inspect | `evaluated-finding` | Every FBX inspected, measured, and linted; explicit role-map difference retained |
| Segment | `partially-evaluated` | Individual FBXs are runtime sources; combined-take segmentation not promoted |
| Root motion | `evaluated-finding` | All labeled variants inventoried and measured; explicit-role pass adds 45/121 clips moving >1 cm, 76 stationary, 0 with >1° yaw; per-axis ownership declaration remains unavailable |
| Conform | `evaluated-finding` | Skeleton signatures, role-resolution gap, twist bones, and Unity rig exceptions recorded |
| Validate | `partially-evaluated` | Mechanical/contract work exhaustive; visual combat, masks, contacts, and transitions remain |
| Optimize | `evaluated-finding` | Twenty-four current gait candidates and one prune candidate exported; runtime/equivalence acceptance remains open |
| Export | `partially-evaluated` | Generated GLBs are evidence only, not adopted production candidates; a new-project GLB import test now confirms all 24 gait candidates load as Generic clips (see Engine procedures and evidence) |
| Gate/report | `evaluated-clean` | Manifest and linked report pair parser-validated |

### Readiness evidence by clip set

| Role or runtime set | File-ready / clip-ready | Set-ready / rig-use | Runtime / acceptance boundary |
|---|---|---|---|
| 118 dominant clips | Core mechanical checks complete on 0.4.0; explicit role config required for gait/root/loop-seam coverage (default: 4/122 root trajectory, 0 loop-seam findings; explicit: 122/122, 4 loop-seam findings); declared loop policy fails broadly in both passes | Common 58-bone signature with two forearm-twist bones | Unity Humanoid import and seven samples pass (retained); import-advice available; visual/target-rig acceptance open |
| Two block clips | FBX-readable, but no explicit Unity clip definition or Humanoid import | Collection 56-bone signature, different from pack majority | Quarantined; corrected author exports required |
| Three locomotion rings | Durations and RM speeds measured; strict loop findings remain | Raw circular spreads 0.711/0.602/0.577; released 0.4.0 anchoring under explicit-role contracts reduces them to 0.069/0.143/0.054, matching pre-release `674396f` to 7 decimals | Keep RM raw; transformed IP engine/visual acceptance and residual offsets remain; candidates unpromoted |
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

All numbers below are from released AnimSmith `0.4.0` (`6b37ad636b1`), captured 2026-08-21, unless a row is explicitly marked historical.

**Default profile vs. explicit `[rig.roles]` config — the complete #437 evidence, run separately and reported side by side:**

| Fact | Default profile | Explicit `[rig.roles]` config |
|---|---|---|
| `root_trajectory` availability | 4/122 measured, 118 not_applicable | 122/122 measured |
| `gait` availability | 3 measured / 118 not_applicable / 1 unavailable | 121 measured / 0 not_applicable / 1 unavailable |
| `speed_mps` availability | 3 measured / 118 not_applicable | 121 measured |
| `loop_seam_ratio` availability | 1 measured / 120 not_applicable | 65 measured / 56 not_applicable |
| Contract loop-seam evaluation | 0 complete / 120 not_evaluated | 58 complete / 62 not_evaluated |
| Contract loop-seam findings | 0 files | 4 files |
| Lint notes (mechanical) | 17,016 | 17,016 (identical) |

Two conclusions follow. First, the published "4 normalized loop-seam" contract finding is reachable only under the explicit-role pass; the default pass finds none. Second, because the mechanical lint-note count is byte-for-byte identical between the two passes, the role map changes evaluator *coverage*, not the asset — it is repeatable evaluation/project configuration, not a source-asset repair. Issue [#437](https://github.com/mmannerm/animsmith/issues/437) (case-tolerant fail-closed profile aliasing) is directly re-verified `OPEN` via `gh issue view 437` on 2026-08-21; the case-resolution gap stays visible and this must not be described as an asset repair. The role map also does not unlock everything: `root-motion-speed` and `foot-slide` stay `not_applicable` on all 120 individual files in both passes.

The 3 clips that do resolve gait/speed under the default profile use a fixed local heading witness of `yaw_heading_axis = positive_y`; the explicit-role pass measures the full 121-clip set under the same axis convention, so the heading basis itself is stable across both passes — only coverage changes.

| Finding/check | Affected scope | Potential impact | Evidence |
|---|---|---|---|
| `nan`, `quat-norm`, `quat-flip`, `scale-keys`, `non-uniform-scale` | 123/123 baseline FBXs evaluated; no findings | No defect established at these mechanical gates | `observed-animsmith`; all JSON/Markdown commands exit 0 |
| `duration-sanity` + `time-monotonic` | `Humanoid@GetHitLeftHeavy2HandMelee_RM.fbx` | Two forearm-twist tracks first-key at 0.033 s; channel ends span 1.700–1.733 s, so the shorter tracks clamp-hold | `observed-animsmith`; one `duration-sanity` warning, six `time-monotonic` notes; same finding published for 0.3.0, now with this extra non-gating detail; all lint exits 0 |
| `constant-track` | 122 animation-bearing files; 17,010 notes | Export size/evaluation overhead; pruning may alter sparse-track semantics | `observed-animsmith`; identical count to published 0.3.0 and to the explicit-role pass |
| Default rig-role resolution | 118/120 individual motions unresolved; two 56-bone files resolve built-in humanoid roles | Root/gait/loop-seam measurements are absent in an out-of-box run | `observed-animsmith`; dominant names differ by capitalization and twist bones; see comparison table above |
| Explicit rig-role resolution | 118/118 dominant files resolved as `custom` | Restores gait/root/loop-seam evidence without changing source bytes | `observed-animsmith`; config SHA-256 `667799ff3e6ccbe29306fe70bce0fb85bb5686215387259b0ca8d63694d5a9cd` (unchanged since 0.3.0) |
| Declared loop contracts | 120 individual files; 13 pass, 107 fail, identically in both passes | Pose/velocity wraps, false loops, or intentionally strict policy failures | 48 loop-closure files, 4 normalized-seam files (explicit-role pass only), 106 rotation-seam files, 101 velocity-seam files |
| Directional set phase (circular spread) | 24 core IP files across 3 gait families | Blending unaligned contacts can skate or pulse once per cycle | Raw: Crouch 0.5773874, Run 0.6024028, Walk 0.7111863. Measurable only under the explicit-role config; the default profile cannot resolve this pack's 58-bone roles |
| Directional RM speed | 24 core RM files | Equal input magnitude may produce direction-dependent travel | Ratios: walk 1.35×, run 1.22×, crouch 1.30× |

The base mechanical family ran exhaustively in both passes. Contract checks ran only where declarations made them applicable; unavailable gait/loop-seam roles under the default profile are a coverage gap, not a pass. Explicit mapping restores measurement for the 118-file majority without changing source bytes, while the untouched-baseline limitation remains part of the out-of-box result.

## AnimSmith remediation evidence

| Source issue | Operation/declarations | Result | Independent verification | Remaining caveat |
|---|---|---|---|---|
| Capitalized dominant roles | Explicit `[rig.roles]` mapping for root, hips, spine, head, hands, feet, and toes, run under released 0.4.0 | Restores full coverage vs. the default pass (root trajectory 122/122, gait 121/122, loop-seam findings 4 files); no asset output | Identical 17,016-note mechanical lint count between the default and explicit-role passes is the independent proof of a configuration-only effect (see comparison table above) | Out-of-box profile still unresolved; [#437](https://github.com/mmannerm/animsmith/issues/437) (fail-closed case-tolerant aliasing) stays OPEN; must not be described as an asset repair |
| Directional gait phase | `transform --gait-anchor` on 24 core IP files under explicit-role contracts (`config/contracts-configured`), released 0.4.0 | 24/24 exit 0 and emit GLBs | Inspect/measure/fix dry-run 24/24 exit 0; post-anchor circular spreads Crouch 0.0537579, Run 0.1429141, Walk 0.0693366 match the pre-release `674396f` after-values to seven decimal places; lint/diff 24/24 exit 1 for remaining contracts/semantic rewrites | Only IP transformed; no Unity Humanoid-retarget import ran this session, though all 24 were staged in a separate new-project GLB import test and each loaded as one Generic clip (see Engine procedures and evidence); no visual/contact or trajectory acceptance; residual offsets remain; candidates unpromoted |
| Constant tracks | `transform --prune-constant-tracks` on `Humanoid@IdleCombatA2HandMelee.fbx`, released 0.4.0 | Candidate GLB produced; transform exit 0 | Inspect/measure/fix dry-run exit 0; diff/lint remain nonzero as expected | Bounded by open [#401](https://github.com/mmannerm/animsmith/issues/401); no engine playback, property-equivalence proof, or production adoption |
| Root trajectory (explicit roles) | `measure` under the explicit-role config on all clips, released 0.4.0 | 45/121 clips move >1 cm horizontally, 76 stationary, 0 with >1° yaw travel | Sampled `MetricGrids` regression facts, not continuous-curve or engine-extraction proof | Do not declare movement-ownership axes from measured travel; per-axis intent needs an explicit `movement_owner_*` declaration ([#466](https://github.com/mmannerm/animsmith/issues/466)) |

Pre-release `674396f` (0.3.1-bound) first implemented the merged [#426](https://github.com/mmannerm/animsmith/issues/426) vertical-root-axis gait-anchor basis policy and emitted all 24 IP candidates; that run, together with the `b7c215b` baseline/contracts, is retained here only as dated historical comparison (see Reproduction). Released 0.4.0 reproduces the same result to seven decimal places in circular-spread terms, confirming the release preserves 0.3.1 gait behavior. No RM file was transformed in either run. The historical `674396f` GLBs were never imported. The new 0.4.0 GLBs were not imported as Unity Humanoid clips this session (the retained eight-pack project still has no GLB importer), but all 24 were staged in a separate new Unity 6000.5.8f1 project with `com.unity.cloud.gltfast` 6.9.0 and each loaded as exactly one Generic AnimationClip (see Engine procedures and evidence), so the lower spreads remain mechanical and load-only evidence rather than set-ready, retargeted, or visual acceptance. All generated candidates remain outside the repository with the commercial inputs.

## Engine procedures and evidence

| Runtime | Version | Procedure | Observed result | Remaining gate |
|---|---|---|---|---|
| Unity (retained, dated 2026-08-17) | 6000.5.8f1 | Materialize all eight evaluated Unitypackages into one external project; inventory importers/clips; execute Playables samples, two cross-pack mixers, one AvatarMask graph, expected rig-outlier checks, and sword attachment | 118/120 individuals import as Humanoid; seven samples, both mixers, mask, and prop execute; both Generic block clips fail the Humanoid precondition as expected | A separate new-project GLB import test now covers gait-candidate loading (see below); visual motion/contact/twist review, full graphs, root extraction, target-character retarget of the GLB candidates, compression, build remain open |
| Unity `unity-humanoid` (0.4.0, 2026-08-21; corrected) | rev 1 / 6000.3 | `generate import-advice` regenerated on a delivered dominant-rig FBX under the `unity-humanoid` profile. The published reading was an unverified assumption — that an absent `lockRootRotation`/`lockRootHeightY`/`lockRootPositionXZ` key takes Unity's serialized default of `false`, mapping to `extract` — and it is now corrected: a 2026-08-21 headless Unity 6000.5.8f1 probe read `ModelImporterClipAnimation` directly off a 120-clip cross-pack sample (15 clips from each of the eight evaluated packs, 10 in-place + 5 `_RM` for this pack) | Direct observation falsifies the earlier assumption. Across the 120-clip sample, in-place clips (84) show `lockRootRotation` true 84/84, `lockRootHeightY` true 84/84, `lockRootPositionXZ` true 83/84; root-motion (`_RM`) clips (36) show `lockRootRotation` true 36/36, `lockRootHeightY` true 28/36, `lockRootPositionXZ` true only 5/36 — the delivered policy is **bake** (`true`), not extract, and it is per-variant/axis-specific: XZ is the discriminator. This pack's own 15 sampled clips mostly reproduce that split: all 10 in-place clips observed true/true/true; of the 5 sampled `_RM` clips, `lockRootRotation` is true 5/5, `lockRootHeightY` true 4/5, and `lockRootPositionXZ` true only 1/5 (extracted on the other 4) — the same XZ-as-discriminator pattern with one exception. Regenerated import-advice now projects `lock_root_rotation`=true, `lock_root_height_y`=true, `lock_root_position_xz`=true for in-place clips and false for root-motion clips, matching observation on the sampled majority. This corroborates, but does not by itself decide, the integration recipe's `owner=validate-per-axis` in-place/root-motion split; no per-axis `movement_owner_*` value is declared from it | Confirm the corrected projection against the remaining 105 delivered clips outside the 15-clip sample, including the one sampled `_RM` exception; visual/controller acceptance of the baked-root-motion result remains open |
| Unity (GLB import test) | 6000.5.8f1, new project, `com.unity.cloud.gltfast` 6.9.0 (0.4.0, 2026-08-21) | Staged all 134 AnimSmith 0.4.0 gait-anchored GLB candidates from all eight evaluated packs — including all 24 of this pack's own current gait-anchor candidates — into a brand-new Unity 6000.5.8f1 project, since Unity has no native GLB importer; the retained eight-pack project above was not modified or rerun | 134/134 candidates produced assets and exactly one AnimationClip each, all non-legacy and non-empty (2-Handed contributed 24/24). glTFast imports glTF animation as **Generic** and reconstructs no Humanoid Avatar: this proves the candidates load and yield a well-formed clip, not that the Humanoid retarget path these clips need works, and it is not visual or gameplay acceptance. Candidates remain unpromoted | Supersedes the earlier blanket "Unity project has no GLB importer" blocker — the importer had to be added to a separate project; Humanoid retarget and visual/gameplay acceptance of the 24 candidates remain open |
| Unreal | rev 1 / 5.8 | `generate import-advice` under the `unreal` profile on the same FBX | Typed refusal `profile_settings_unmodeled`, exit 1 — AnimSmith declines to model Unreal V1 settings; this is not an Unreal Editor test | FBX import, IK Rig/Skeleton mapping including twists, Blend Spaces, montages, contacts, build |
| Godot | rev 1 / 4.7 | `generate import-advice` under the `godot` profile on the same FBX | Typed refusal `profile_settings_unmodeled`, exit 1 — same AnimSmith modeling refusal, not a Godot editor test | FBX-to-supported route, Skeleton3D mapping, blend spaces, filters, root motion, export |
| Bevy | rev 1 / 0.19.0 | `generate addressability` under the `bevy` profile on a generated GLB candidate | Exit 0: 1 animation row, coverage `complete`, predicted selector `Animation0`, facet `available`, 0 findings — inventory/selector prediction only | Actual glTF/GLB conversion of the source FBX, retarget strategy, graph/root policy wiring, runtime load, performance |

The Unity 6000.5.8f1 probe is headless and retained unchanged from 2026-08-17, justified by the byte-identical re-verified source; it establishes import and graph execution only, not visible playback quality, contacts, foot plants, two-hand alignment, mask usefulness, retarget deformation, or shipping-build behavior. The 2026-08-21 import-advice/addressability rows are AnimSmith-side capability queries against profile revision 1 tuples: they show what the tool predicts or declines to model, not an actual Unreal/Godot editor import or a Bevy runtime load. The historical `674396f` gait GLBs were never imported. The new 0.4.0 gait GLBs were not imported into the retained eight-pack Unity project (still no GLB importer configured there), but all 24 were staged into a separate new Unity 6000.5.8f1 project with `com.unity.cloud.gltfast` 6.9.0 and each loaded as one Generic AnimationClip — see the Engine procedures table above for the full 134-candidate result and its limits.

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
3. Unity testing was headless; the historical `674396f` gait-anchor GLBs were never imported, and the new 0.4.0 GLBs were not imported into the retained eight-pack project (still no GLB importer there), though a separate new Unity project with `com.unity.cloud.gltfast` 6.9.0 confirmed all 24 load as Generic clips (see Engine procedures and evidence). Complete graphs, root extraction, contacts, Humanoid retarget of the GLB candidates, compression, and builds remain open.
4. Unreal Engine, Godot, and Bevy have no editor/runtime evaluation; the 2026-08-21 rows are AnimSmith import-advice/addressability capability queries only.
5. The two block clips need corrected runtime-ready exports or substitution.
6. The heavy-left-hit RM channel-span anomaly needs target-rig visual review.
7. RM yaw, vertical displacement, weapon arcs, hit events, IK, cancels, and two-handed grip were not accepted.
8. Current public pages/EULA do not prove the local revision, transaction date, or historical terms.
9. The published `unity-humanoid` import-advice previously assumed absent `.fbx.meta` keys take Unity's serialized default (`extract`); a 2026-08-21 headless Unity 6000.5.8f1 probe of `ModelImporterClipAnimation` on a 120-clip cross-pack sample (including 15 of this pack's own clips) falsifies that assumption and shows the delivered policy is bake (`true`), extracting root-motion XZ on most but not all sampled `_RM` clips. The sample covers 15 of this pack's 120 individual clips, not all of them.
10. The explicit `[rig.roles]` config changes evaluator coverage, not source bytes; [#437](https://github.com/mmannerm/animsmith/issues/437) (case-tolerant fail-closed aliasing) remains open, so out-of-box role resolution is still a gap.
11. The integration recipe's `owner=validate-per-axis` step directs the reader to validate root-motion ownership axis by axis rather than assume it. The observed Unity importer locks bake root rotation on every sampled root-motion clip, so animation cannot be assumed to own root-motion yaw. The step is not a per-axis `movement_owner_xz` / `movement_owner_y` / `movement_owner_yaw` declaration, and no such declaration is derived from measured travel in this refresh. Measured root displacement and yaw are recorded as sampled facts only; choosing the per-axis owner remains a project and engine decision. The 2026-08-21 observed Unity root-lock policy (Engine procedures and evidence) independently corroborates this split on the sampled clips — but that is corroborating engine evidence for the recipe, not a licence to declare per-axis `movement_owner_*` values, and none is declared here.
12. The new-project GLB import test (134/134 candidates, 24 from this pack) proves glTFast produces one well-formed Generic AnimationClip per candidate; it does not test this pack's Humanoid retarget path and is not visual or gameplay acceptance.

## Reproduction

**Current evaluator (2026-08-21):** AnimSmith `0.4.0`, tag `v0.4.0`, revision `6b37ad636b198ef8ff47fadbf6a3a51eb1a27c8e`, binary SHA-256 `fd1eee57407aa02db88763d144389a7f5104204c40ddfbb28eb5885ca8cd54c6`, output schema v10 nesting measurements schema v15. This one frozen binary produced the complete default-pass, explicit-role-pass, contract, gait-anchor, prune, and engine-profile evidence above.

Source identity (re-verified byte-identical 2026-08-21): RAR SHA-256 `dc067fc8233e51df5a16606758b586a1ec18896076212f76551538c92ca2ff04`; Unitypackage SHA-256 `3cf6c5359c8845768afa098b79972679a01d80b59a6c7e94d0858d6b405f7054`; logical manifest SHA-256 `3e21bdc9d8f8bb463fdef8eb7760551bf733d28678c7d2abd093e620e226b347`. Inventory re-run: 0 added, 0 removed, 0 changed across 123 FBXs. Explicit-role config digest, unchanged: `667799ff3e6ccbe29306fe70bce0fb85bb5686215387259b0ca8d63694d5a9cd`.

**Historical (674396f, pre-release 0.3.1-bound):** `animsmith 0.3.0 (v0.3.0-39-g674396f)`, revision `674396f0f53b10c4344e7315a5756fe5ef71b469`, binary SHA-256 `7744b71580e04d80f9e5738efce76e0295323ccb3150fa57b0ad9b37c5ff1513`. **Historical baseline/contracts:** captured at `b7c215ba259b87b4b4e46567452a037a34be7308`. Both are retained only for dated comparison; the 2026-08-21 run above supersedes them for current baseline, contract, and remediation conclusions.

**Build reproducibility note:** a 2026-08-21 rebuild of the pinned `v0.4.0` commit produced a binary with a different SHA-256, `1e53013bbe3224557a8783eafeb818f4ef9d74666590cbaa8c18ef48c5b7d6fa`, than the digest recorded above; the build is not byte-reproducible. Both builds emit byte-identical advice artifacts (verified by `diff`), so the regenerated Unity import-advice and the corrected root-lock reading in this refresh are attributable to tag `v0.4.0` / commit `6b37ad636b198ef8ff47fadbf6a3a51eb1a27c8e`, not to this specific recorded binary digest.

```text
animsmith inspect --config config/baseline.animsmith.toml <input.fbx>
animsmith measure --config config/baseline.animsmith.toml --format json <input.fbx>
animsmith measure --config config/explicit-roles.animsmith.toml --format json <input.fbx>
animsmith lint --config config/contracts-configured/<file>.animsmith.toml --format json <input.fbx>
animsmith report --config <config> <input.fbx> --output <report.html>
animsmith transform --config <config> <input.fbx> -o <output.glb> --gait-anchor
animsmith transform --config <config> <input.fbx> -o <output.glb> --prune-constant-tracks
animsmith --config <unity-humanoid-profile>.animsmith.toml generate import-advice <input.fbx> --format json
animsmith --config <unreal-profile>.animsmith.toml generate import-advice <input.fbx> --format json
animsmith --config <godot-profile>.animsmith.toml generate import-advice <input.fbx> --format json
animsmith --config <bevy-profile>.animsmith.toml generate addressability <input.glb> --format json
```

Historical retained summaries (674396f/b7c215b, superseded by the 2026-08-21 run above): historical untouched baseline `af085492f41888def42cf3220d770c0c49d8f1334c714f1a56b1d7b9c7e4b7cb`; explicit-role baseline `b9bd00ecf243c75c66d275c2f40ab7cfacc053d85dae9bca530ee5f96c8317dc`; contracts `b2069595be2a6b6e9b4e4f411f2dcdfd35b448b25e1fb683503aabb50457e91d`; historical refusal-era remediation `ffc1cef7bdaba6a4d4937be4abbf537a23faa11db3ee34c893d5bd53eff6df57`; current remediation commands `65d6a098d67478e6ce4af1c758e48b3b737e96b10a8df5e5444f861147cfcb5e`; current combined summary `118116c9173df4e3e782cdfe3b712deb9fb14cec23c8e0e75cd484e8156d7f4b`; Unity probe `1c147ff6683833ba28c1db210d58aee65140ac232311f370782c28c3925ae62d`. The 2026-08-21 measure/lint JSON, gait-anchor/prune-constant-tracks GLBs, and import-advice/addressability JSON are retained in the private evaluation workspace outside the repository; this appendix publishes only the aggregate facts and digests above. A headless Unity 6000.5.8f1 probe additionally read `ModelImporterClipAnimation` over a 120-clip cross-pack sample (15 from this pack) to correct the assumed root-lock defaults, and a separate new Unity 6000.5.8f1 project with `com.unity.cloud.gltfast` 6.9.0 staged all 134 gait-anchor GLB candidates (24 from this pack) to confirm each imports as exactly one Generic AnimationClip; the retained eight-pack project was not modified.

### Evaluator currency: AnimSmith 0.4.1

AnimSmith 0.4.1 (tag `v0.4.1`, commit `46e4adfc14947d2afbf433386b0ab9857ea935aa`,
changelog-dated 2026-08-22) was released after this evidence was captured. The
evidence in this appendix remains attributable to 0.4.0, which produced it;
relabelling it would be false attribution. 0.4.1 was instead verified equivalent
for this collection before that decision was made:

| Comparison | Scope | Result |
|---|---|---|
| Baseline `measure`/`lint` content and exit codes | 918 delivered FBXs, all eight packs | 0 files differ |
| Declared-contract `lint` | 177 per-clip contracts | 0 differ |
| `generate import-advice` payload | Unity profile | identical |
| Gait anchoring | 24-member ring | 24/24 anchored; circular spreads identical to seven decimals |
| Generated GLB candidates | 24 | motion payload byte-identical; only the glTF `asset.generator` string differs |
| Contract versions | — | unchanged at output v10 / measurements v15 |

The tool-identity block is excluded from those comparisons because it necessarily
differs between releases. 0.4.1 fixes [#502](https://github.com/mmannerm/animsmith/issues/502),
which affects the `scale rest-bind` admission path this evaluation never invoked,
and [#503](https://github.com/mmannerm/animsmith/issues/503), a diagnostics defect
this evaluation reported: 0.4.0 emits `missing required engine setting
BakeAxisConversion` while 0.4.1 emits the accepted key `bake_axis_conversion`.
Neither fix changes a measurement here. Issue and release state are
time-sensitive; re-query them before reuse.


## Sources

- Local source archive and extracted Unity metadata — private authorized input identified above, accessed 2026-08-17; re-verified byte-identical 2026-08-21.
- Protofactor, [2-Handed Melee Weapon](https://protofactor.biz/product/animset-2-handed-melee-weapon/), [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/), and [End User License Agreement](https://protofactor.biz/end-user-license-agreement/) — current listing/license context, not local revision proof.
- Unity, [Avatar Mask](https://docs.unity3d.com/6000.0/Documentation/Manual/class-AvatarMask.html), [Animation Layers](https://docs.unity3d.com/6000.0/Documentation/Manual/AnimationLayers.html), and [Root Motion](https://docs.unity3d.com/6000.5/Documentation/Manual/RootMotion.html) — runtime capabilities.
- Epic Games, [Root Motion](https://dev.epicgames.com/documentation/unreal-engine/root-motion-in-unreal-engine?application_version=5.7) and [Layered Animations](https://dev.epicgames.com/documentation/unreal-engine/using-layered-animations-in-unreal-engine?lang=en-US); Godot, [AnimationTree](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html); Bevy, [Animation Masks](https://bevy.org/examples/animation/animation-masks/) — documentation-only capability.
- AnimSmith issues [#401](https://github.com/mmannerm/animsmith/issues/401) (property-scoped pruning, open), [#402](https://github.com/mmannerm/animsmith/issues/402) (per-clip channel coverage, shipped), [#408](https://github.com/mmannerm/animsmith/issues/408) (root displacement/yaw measurement, shipped), [#411](https://github.com/mmannerm/animsmith/issues/411) (cross-member speed/stride checks, open), [#426](https://github.com/mmannerm/animsmith/issues/426) (vertical-root-axis gait-anchor, shipped), [#437](https://github.com/mmannerm/animsmith/issues/437) (case-tolerant fail-closed role aliasing, OPEN), and [#466](https://github.com/mmannerm/animsmith/issues/466) (declarative per-axis movement ownership, shipped) — issue status verified live 2026-08-21.
