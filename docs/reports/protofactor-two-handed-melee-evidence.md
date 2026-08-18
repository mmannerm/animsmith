# Animation pack evidence appendix: Protofactor 2-Handed Melee Weapon Animset

> Companion report: [technical evaluation](protofactor-two-handed-melee.md)
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
| Pack/edition | Local `Animset@2HandedMeleeWeapon_PACKAGE.unitypackage`; constituent revision is not declared |
| Vendor/source | Protofactor; [current 2-Handed Melee product page](https://protofactor.biz/product/animset-2-handed-melee-weapon/) |
| Delivered scope | Full local RAR → one Unitypackage → 123 FBXs: 120 individual motions, one combined animation FBX, one skinned actor, and one sword prop; Unity metadata/materials/textures included |
| Target use | Game-engine use; third-person two-handed combat and combined use with the seven previously evaluated constituents |
| Target engines | Unity 6000.5.8f1 observed; Unreal Engine, Godot, and Bevy documentation-only |
| Target rigs/packs | Supplied Protof-Actor, Basic Locomotion, Sword & Shield, and the evaluated collection subset |
| Source manifest | `logical-assets-inventory.json`; SHA-256 `3e21bdc9d8f8bb463fdef8eb7760551bf733d28678c7d2abd093e620e226b347` |
| Evaluation manifest | `evidence/evaluation-manifest.json`; SHA-256 `0121fd1d73e46646c6fd585954bd2fab7744c51f6bac86c6d0ac6504108abd82`; schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1`; taxonomy/profile-set version 1 |
| Acquisition/license provenance | `user-stated`: local archive was downloaded from Protofactor.biz as part of the Ultimate Animation Collection. Current EULA permits protected released real-time applications and modification while restricting redistribution/resale; no receipt, download date, historical EULA, or local constituent revision was retained. Technical due diligence only, not legal advice. |

The current product page, observed 2026-08-17, advertises USD 19.99, 118 animations, 48 root-motion and 70 in-place files, Unity Humanoid, Unity 2018.4.2+, and no native UE4 package. The local delivery has 120 individual files and 48 `_RM` labels. The 118-file listing count equals the observed Humanoid subset, while the other two local files are Generic; that numerical alignment does not prove revision identity or vendor intent.

The evaluation manifest uses schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1`.

### Evidence coverage

| Surface | Offered/delivered | Evaluated | Findings | Not evaluated and why |
|---|---:|---:|---:|---|
| Animation files | 122 animation-bearing FBXs | 122 | 120 individual plus combined take and actor | Continuous artistic review of all motion |
| Rigs/export variants | 4 observed structures | 4 | Dominant 58-bone, two 56-bone outliers, actor/combined 58-bone variant, prop | Target-character deformation and non-Unity retarget |
| AnimSmith baseline | 123 FBXs | 123 | All commands exit 0; timing and 17,010 constant-track findings | Artistic intent and contacts |
| Declared contracts | 120 individual files | 120 | 13 pass, 107 fail under delivered/inferred declarations | Human loop intent for every action |
| Offline visual reports | 122 possible | 5 generated; 0 visually accepted | No visual conclusions claimed | Continuous playback/contact/deformation review |
| Engine import/playback | 120 individual files | 120 imported; 7 sampled | 118 Humanoid; 2 expected Generic failures | Visual playback of the full pack and builds |
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
| Walk combat 8-way | `directional-blend` | 8 IP/RM direction pairs | Exact directional names; common 1.333 s duration | Raw measured; eight current IP candidates reduce spread to 0.069337; runtime visual blend not evaluated |
| Run combat 8-way | `directional-blend` | 8 IP/RM direction pairs | Exact directional names; 0.533–0.567 s duration | Raw measured; eight current IP candidates reduce spread to 0.142914; runtime visual blend not evaluated |
| Crouch combat 8-way | `directional-blend` | 8 IP/RM direction pairs | Exact directional names; common 1.667 s duration | Raw measured; eight current IP candidates reduce spread to 0.053758; runtime visual blend not evaluated |
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
| Root motion | `evaluated-finding` | All labeled variants inventoried and measured; per-action yaw remains unavailable |
| Conform | `evaluated-finding` | Skeleton signatures, role-resolution gap, twist bones, and Unity rig exceptions recorded |
| Validate | `partially-evaluated` | Mechanical/contract work exhaustive; visual combat, masks, contacts, and transitions remain |
| Optimize | `evaluated-finding` | Twenty-four current gait candidates and one prune candidate exported; runtime/equivalence acceptance remains open |
| Export | `partially-evaluated` | Generated GLBs are evidence only, not adopted production candidates |
| Gate/report | `evaluated-clean` | Manifest and linked report pair parser-validated |

### Readiness evidence by clip set

| Role or runtime set | File-ready / clip-ready | Set-ready / rig-use | Runtime / acceptance boundary |
|---|---|---|---|
| 118 dominant clips | Core mechanical checks complete; explicit role config required; declared loop policy fails broadly | Common 58-bone signature with two forearm-twist bones | Unity Humanoid import and seven samples pass; visual/target-rig acceptance open |
| Two block clips | FBX-readable, but no explicit Unity clip definition or Humanoid import | Collection 56-bone signature, different from pack majority | Quarantined; corrected author exports required |
| Three locomotion rings | Durations and RM speeds measured; strict loop findings remain | Raw spreads 0.711/0.602/0.580; current IP candidates 0.069/0.143/0.054 | Keep RM raw; transformed IP engine/visual acceptance and residual offsets remain |
| Normal forward speed | Three paired gaits measured; 6.27× speed range | Shared dominant skeleton | Controller thresholds, playback scaling, and foot contacts open |
| Actions/reactions | File-ready except one timing-span warning; delivered loop intent unreliable | Full-body use; two-handed contact and yaw semantics unknown | Headless samples only; hit windows, grip, cancels, and visual quality open |
| Draw/combat/put-away | All members readable; two chain members pass declared contracts | Shared skeleton and right-hand attachment | State transitions and visible equipment continuity open |

### Validation-profile status

| Validation profile | Selection | Result / next evidence |
|---|---|---|
| Marketplace intake | `selected` — `vendor-intended` | Full local delivery inventoried, hashed, and mechanically evaluated |
| Blended locomotion | `selected` — `observed-pack-capability` | Three rings and one speed family measured; visual blend acceptance remains |
| Root-motion controller | `selected` — `observed-pack-capability` | RM speed measured; yaw/action ownership and engine extraction remain |
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

| Finding/check | Affected scope | Potential impact | Evidence |
|---|---|---|---|
| `nan`, `quat-norm`, `quat-flip`, `scale-keys`, `non-uniform-scale` | 123/123 baseline FBXs evaluated; no findings | No defect established at these mechanical gates | `observed-animsmith`; all JSON/Markdown commands exit 0 |
| `duration-sanity` + `time-monotonic` | `Humanoid@GetHitLeftHeavy2HandMelee_RM.fbx` | Two forearm-twist tracks start at 0.033 s; channel ends span 1.700–1.733 s, so shorter tracks clamp-hold | `observed-animsmith`; one warning, six notes |
| `constant-track` | 122 animation-bearing files; 17,010 baseline notes | Export size/evaluation overhead; pruning may alter sparse-track semantics | `observed-animsmith`; note severity |
| Default rig-role resolution | 118/120 individual motions unresolved; two 56-bone files resolve built-in humanoid roles | Root/gait measurements are absent in an out-of-box run | `observed-animsmith`; dominant names differ by capitalization and twist bones |
| Explicit rig-role resolution | 118/118 dominant files resolved as `custom` | Restores gait/root evidence without changing source bytes | `observed-animsmith`; config SHA-256 `667799ff3e6ccbe29306fe70bce0fb85bb5686215387259b0ca8d63694d5a9cd` |
| Declared loop contracts | 120 individual files; 13 pass, 107 fail | Pose/velocity wraps, false loops, or intentionally strict policy failures | 48 closure, 4 normalized seam, 106 rotation-seam, 101 velocity-seam files |
| Directional set phase | 48 core files | Blending unaligned contacts can skate or pulse once per cycle | Historical `b7c215b` baseline: IP/RM spreads walk 0.711, run 0.602, crouch 0.577–0.580 |
| Directional RM speed | 24 core RM files | Equal input magnitude may produce direction-dependent travel | Ratios: walk 1.35×, run 1.22×, crouch 1.30× |

The base mechanical family ran exhaustively. Contract checks ran only where declarations made them applicable; unavailable gait roles under the default profile are a coverage gap, not a pass. Explicit mapping restored measurement for the 118-file majority, while the untouched-baseline limitation remains part of the out-of-box result.

## AnimSmith remediation evidence

| Source issue | Operation/declarations | Result | Independent verification | Remaining caveat |
|---|---|---|---|---|
| Capitalized dominant roles | Explicit `[rig.roles]` mapping for root, hips, spine, head, hands, feet, and toes | 118/118 dominant files resolve as `custom`; no asset output | Config digest plus configured baseline SHA-256 `b9bd00ecf243c75c66d275c2f40ab7cfacc053d85dae9bca530ee5f96c8317dc` | Out-of-box profile still unresolved; future detection must refuse ambiguous matches; see [#437](https://github.com/mmannerm/animsmith/issues/437) |
| Directional gait phase | Current `transform --gait-anchor` on 24 core IP files with explicit roles/contracts | 24/24 exit 0 and emit GLBs | Inspect/measure/fix dry-run 24/24 exit 0; post spreads walk 0.0693366, run 0.1429141, crouch 0.0537579; lint/diff 24/24 exit 1 for remaining contracts/semantic rewrites | Only IP transformed; no Unity GLB importer, visual/contact, or trajectory acceptance; residual offsets remain |
| Constant tracks | `transform --prune-constant-tracks` on `Humanoid@IdleCombatA2HandMelee.fbx` | Candidate GLB produced; transform exit 0 | Inspect/measure/fix dry-run exit 0; diff/lint remain nonzero as expected | No engine playback, property-equivalence proof, or production adoption |

The earlier `b7c215b` heading-basis refusals are historical only. Revision `674396f` implements the merged [#426](https://github.com/mmannerm/animsmith/issues/426) basis policy and emits all 24 IP candidates. No RM file was transformed, and the Unity project had no GLB importer, so the lower spreads are mechanical evidence rather than set-ready or visual acceptance. All generated candidates remain outside the repository with the commercial inputs.

## Engine procedures and evidence

| Runtime | Version | Procedure | Observed result | Remaining gate |
|---|---|---|---|---|
| Unity | 6000.5.8f1 | Materialize all eight evaluated Unitypackages into one external project; inventory importers/clips; execute Playables samples, two cross-pack mixers, one AvatarMask graph, expected rig-outlier checks, and sword attachment | 118/120 individuals import as Humanoid; seven samples, both mixers, mask, and prop execute; both Generic block clips fail the Humanoid precondition as expected | Add a GLB importer or convert candidates, then test gait outputs; visual motion/contact/twist review, full graphs, root extraction, target-character retarget, compression, build |
| Unreal Engine | Not installed | Review official FBX/root-motion/layered-animation capability; no import performed | `not-evaluated`; documentation does not prove this pack | FBX import, IK Rig/Skeleton mapping including twists, Blend Spaces, montages, contacts, build |
| Godot | Not installed | Review AnimationTree capability; no import performed | `not-evaluated`; documentation does not prove this pack | FBX-to-supported route, Skeleton3D mapping, blend spaces, filters, root motion, export |
| Bevy | Not installed | Review glTF animation graph/mask capability; no conversion/import performed | `not-evaluated`; documentation does not prove this pack | Conversion, target identities, retarget strategy, graph/root policy, performance |

The Unity probe is headless. It establishes import and graph execution only, not visible playback quality, contacts, foot plants, two-hand alignment, mask usefulness, retarget deformation, or shipping-build behavior.

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
3. Unity testing was headless; the 24 refreshed gait-anchor GLBs were not imported because the project had no GLB importer. Complete graphs, root extraction, contacts, compression, and builds remain open.
4. Unreal Engine, Godot, and Bevy were documentation-only; no pack import was observed.
5. The two block clips need corrected runtime-ready exports or substitution.
6. The heavy-left-hit RM channel-span anomaly needs target-rig visual review.
7. RM yaw, vertical displacement, weapon arcs, hit events, IK, cancels, and two-handed grip were not accepted.
8. Current public pages/EULA do not prove the local revision, transaction date, or historical terms.

## Reproduction

Source identity: RAR SHA-256 `dc067fc8233e51df5a16606758b586a1ec18896076212f76551538c92ca2ff04`; Unitypackage SHA-256 `3cf6c5359c8845768afa098b79972679a01d80b59a6c7e94d0858d6b405f7054`. Gait remediation used pre-release 0.3.1 code: `animsmith 0.3.0 (v0.3.0-39-g674396f)`, revision `674396f0f53b10c4344e7315a5756fe5ef71b469`, binary SHA-256 `7744b71580e04d80f9e5738efce76e0295323ccb3150fa57b0ad9b37c5ff1513`. Baseline and contracts remain captured at `b7c215ba259b87b4b4e46567452a037a34be7308`.

```text
animsmith inspect --config config/baseline.animsmith.toml <input.fbx>
animsmith measure --config config/baseline.animsmith.toml --format json <input.fbx>
animsmith measure --config config/explicit-roles.animsmith.toml --format json <input.fbx>
animsmith lint --config config/contracts-configured/<file>.animsmith.toml --format json <input.fbx>
animsmith report --config <config> <input.fbx> --output <report.html>
animsmith transform --config <config> <input.fbx> -o <output.glb> --gait-anchor
animsmith transform --config <config> <input.fbx> -o <output.glb> --prune-constant-tracks
```

Retained summaries: historical untouched baseline `af085492f41888def42cf3220d770c0c49d8f1334c714f1a56b1d7b9c7e4b7cb`; explicit-role baseline `b9bd00ecf243c75c66d275c2f40ab7cfacc053d85dae9bca530ee5f96c8317dc`; contracts `b2069595be2a6b6e9b4e4f411f2dcdfd35b448b25e1fb683503aabb50457e91d`; historical refusal-era remediation `ffc1cef7bdaba6a4d4937be4abbf537a23faa11db3ee34c893d5bd53eff6df57`; current remediation commands `65d6a098d67478e6ce4af1c758e48b3b737e96b10a8df5e5444f861147cfcb5e`; current combined summary `118116c9173df4e3e782cdfe3b712deb9fb14cec23c8e0e75cd484e8156d7f4b`; Unity probe `1c147ff6683833ba28c1db210d58aee65140ac232311f370782c28c3925ae62d`.

## Sources

- Local source archive and extracted Unity metadata — private authorized input identified above, accessed 2026-08-17.
- Protofactor, [2-Handed Melee Weapon](https://protofactor.biz/product/animset-2-handed-melee-weapon/), [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/), and [End User License Agreement](https://protofactor.biz/end-user-license-agreement/) — current listing/license context, not local revision proof.
- Unity, [Avatar Mask](https://docs.unity3d.com/6000.0/Documentation/Manual/class-AvatarMask.html), [Animation Layers](https://docs.unity3d.com/6000.0/Documentation/Manual/AnimationLayers.html), and [Root Motion](https://docs.unity3d.com/6000.5/Documentation/Manual/RootMotion.html) — runtime capabilities.
- Epic Games, [Root Motion](https://dev.epicgames.com/documentation/unreal-engine/root-motion-in-unreal-engine?application_version=5.7) and [Layered Animations](https://dev.epicgames.com/documentation/unreal-engine/using-layered-animations-in-unreal-engine?lang=en-US); Godot, [AnimationTree](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html); Bevy, [Animation Masks](https://bevy.org/examples/animation/animation-masks/) — documentation-only capability.
- AnimSmith issues [#401](https://github.com/mmannerm/animsmith/issues/401), [#402](https://github.com/mmannerm/animsmith/issues/402), [#408](https://github.com/mmannerm/animsmith/issues/408), [#411](https://github.com/mmannerm/animsmith/issues/411), [#426](https://github.com/mmannerm/animsmith/issues/426), and [#437](https://github.com/mmannerm/animsmith/issues/437) — optimization, root, speed, gait, and role-profile follow-up.
