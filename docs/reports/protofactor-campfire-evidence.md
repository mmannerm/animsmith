# Animation pack evidence appendix: Protofactor Campfire

> Companion report: [Protofactor Campfire](protofactor-campfire.md)
>
> Evidence status: **partial** — exhaustive file/AnimSmith coverage, Unity 6000.5.8f1 headless evidence now corrected by a direct Unity 6000.5.8f1 observation of import-advice root-lock declarations, and 0.4.0 `generate import-advice` probes for Unity/Unreal/Godot; visual contact, target-character, and full three-engine import passes remain absent, and Bevy has no generated glTF/GLB candidate to probe. A collection-wide headless Unity glTFast import of all 134 gait-anchored GLB candidates from the other evaluated packs ran 2026-08-21; this pack has no gait ring and contributed none of them.
>
> Evaluation date: **2026-08-21**
>
> Report format: **1**

This appendix uses the [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder) without redefining it.

## Evaluation scope and provenance

| Field | Value |
|---|---|
| Pack/edition | Protofactor Campfire constituent from a locally held Ultimate Animation Collection archive; local revision unknown |
| Vendor/source | Protofactor [Campfire product](https://protofactor.biz/product/animset-campfire/) and [collection](https://protofactor.biz/product/ultimate-animation-collection/) pages |
| Delivered scope | RAR to one Unitypackage to 114 logical files; 29 FBXs: 25 individual motions, one combined take, one actor, campfire prop, skewer prop |
| Target use | Game-engine camp/rest state machine, contextual actions, props, and combination with evaluated collection packs |
| Target engines | Unity 6000.5.8f1 observed headless (retained 2026-08-17); Unity 6000.3, Unreal Engine 5.8, and Godot 4.7 AnimSmith import-advice probed (2026-08-21); Bevy 0.19.0 documentation-only, no generated glTF/GLB candidate |
| Target rigs/packs | Supplied Protof-Actor; Basic Locomotion, Sword & Shield, Climbing, and Injured selective compatibility |
| Source manifest | `campfire/source-archive-inventory.json`; RAR SHA-256 `bed86be7f91fdd46b376fce4b1a00c88372a3f703e0fe9077925712d8af8e8e9`; re-inventoried 2026-08-21 under AnimSmith 0.4.0, byte-identical to the published manifest (0 added, 0 removed, 0 changed) |
| Evaluation manifest | `campfire/evidence/evaluation-manifest.json`; SHA-256 `11e67cd944ad2058d130eea06f557b41b1ba36e0ed14bbc3289d704d99bf962e`; schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1`; taxonomy/profile-set version 1; unchanged by the 2026-08-21 refresh |
| Acquisition/license provenance | User states the local archive was downloaded from Protofactor; current [vendor EULA](https://protofactor.biz/end-user-license-agreement/) reviewed 2026-08-17. No receipt or local revision record was evaluated; no legal opinion. |

### Evidence coverage

| Surface | Offered/delivered | Evaluated | Findings | Not evaluated and why |
|---|---:|---:|---:|---|
| Animation files | 25 individual plus 1 combined | 26 | 8 individual contract failures | Dynamic visual quality and combined-take segmentation |
| Rigs/export variants | 4 observed signatures | 4 | Standard 56; combined/actor 58; campfire 3; skewer 19 bones | Target-character deformation |
| AnimSmith baseline | 29 FBXs | 29 | 3,664 constant-track notes in 27 animation-bearing files | No semantic intent in default lint |
| Declared contracts | 25 individual files | 25 | 8 failing files; 3,394 notes | Contact/events and artistic endpoints |
| Offline visual reports | 25 possible | 3 | Reports render skeleton, metrics, and findings | Dynamic visual acceptance |
| Engine import/playback | 25 individual clips | 25 imported; 4 sampled | Required samples pass | Controller, compression, player build |
| Blend/mask/retarget | 1 cross-pack mixer; 2 props | 1 mixer; 2 props | Execution/instantiation pass | Visual blend, prop orientation/contact, target rig |

### Claim legend

`observed-file` means derived from delivered files/metadata; `observed-animsmith` means reproduced with the named evaluator; `observed-engine` means the headless Unity probe; `inferred` marks semantic grouping. None of these labels means gameplay acceptance.

## Evaluation manifest and taxonomy

The retained evaluation manifest uses schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1` and taxonomy/profile-set version 1.

### Canonical clip-role inventory

| Canonical primary role | Logical motions | Delivered files | Evidence boundary |
|---|---:|---:|---|
| `idle-pose` | 7 | 7 | Filename/state semantics; all stationary |
| `continuous-locomotion` | 0 | 0 | Absent |
| `locomotion-transition` | 11 | 11 | Posture-state transitions inferred from exact filenames |
| `airborne` | 0 | 0 | Absent |
| `traversal` | 0 | 0 | Absent |
| `action-interaction` | 7 | 7 | Lighting, skewer, and log actions; contacts unaccepted |
| `reaction-death` | 0 | 0 | Absent |
| `emote-cinematic` | 0 | 0 | Absent |
| `other-unknown` | 0 | 0 | None |
| **Total** | **25** | **25** | Validated manifest SHA-256 `11e67cd944ad2058d130eea06f557b41b1ba36e0ed14bbc3289d704d99bf962e` |

### Runtime-set inventory

| Runtime set | Type | Members/variants | Grouping evidence | Validation status |
|---|---|---|---|---|
| Stand-kneel-sit-rest chain | transition-chain | 7 single files | Exact reciprocal/state labels; medium confidence | Mechanical complete; endpoints visual-open |
| Sit-lie chain | transition-chain | 4 single files | Reciprocal sit/lie labels; high confidence | Detailed members in primary; visual-open |
| Stand-sleep chain | transition-chain | 3 single files | Stand/sleep labels; high confidence | Mechanical complete; visual-open |
| Kneel-grill chain | transition-chain | 4 single files | Kneel/grill/skewer family; medium confidence | Detailed members in primary; contact-open |
| Fire-lighting alternatives | other | 4 single files | Four named techniques; high confidence | Detailed members in primary; use discretely |
| Log-toss alternatives | other | 2 single files | Sitting/kneeling alternatives; medium confidence | Missing log/contact gate |

### Pipeline-stage coverage

| Stage | Coverage state | Evidence / remaining gate |
|---|---|---|
| Acquire | `evaluated-clean` | Local vendor archive identified and hashed; transaction record absent. |
| Preserve raw | `evaluated-clean` | RAR and Unitypackage retained unchanged outside the repository. |
| Inspect | `evaluated-finding` | Every FBX inspected/measured/linted; declared loop and constant-track findings retained. |
| Segment | `partially-evaluated` | Individual files used; combined take not promoted. |
| Root motion | `evaluated-clean` | No root-motion-labelled constituent motion files; AnimSmith 0.4.0 `root_trajectory` (delivered this release by closed issue #408, "expose root displacement and accumulated yaw per clip", closed 2026-08-20) now measures 27/27 clips and confirms 0 move more than 1 cm horizontally and 0 exceed 1° of yaw travel — a measured confirmation, not merely an absence of a root-motion label. Sampled regression facts, not continuous-curve or engine-extraction proof; do not derive movement-ownership axes from this alone. |
| Conform | `partially-evaluated` | Standard skeleton and Unity shared Avatar work; target rigs/other engines open. |
| Validate | `partially-evaluated` | Mechanical contracts and headless Unity complete; visual gameplay open. |
| Optimize | `evaluated-finding` | One pruning candidate verified mechanically but not accepted semantically. |
| Export | `partially-evaluated` | Sample GLB export reopens; Unity native delivery tested; other engines open. |
| Gate/report | `evaluated-clean` | Manifest and parser-validated report pair retained. |

### Readiness evidence by clip set

| Role or runtime set | File-ready / clip-ready | Set-ready / rig-use | Runtime / acceptance boundary |
|---|---|---|---|
| Stationary idles | Files readable; 7 delivered loops; 2 strict seam failures | Standard skeleton; no additive/mask contract | Unity imports; inspect true wraps visually |
| Posture chains | Eleven one-shots mechanically readable | Exact state graph inferred; list labels disagree | Unity samples pass; endpoints/cancellation unaccepted |
| Lighting/log/skewer actions | Seven long actions readable | Campfire/skewer supplied; five implied props absent | Events, offsets, contacts, and fire timing unaccepted |

### Validation-profile status

| Validation profile | Selection | Result / next evidence |
|---|---|---|
| Marketplace intake | `selected` — `vendor-intended` | Archive, current listing, and EULA reviewed; revision/receipt absent. |
| Blended locomotion | `not-selected` | No locomotion set. |
| Root-motion controller | `not-selected` | No RM-labelled motions. |
| State-machine transitions | `selected` — `observed-pack-capability` | Eleven transition files; visual endpoints open. |
| Layered upper body/weapons | `not-selected` | Full-body posture/contact baseline is safer. |
| Traversal/environment | `not-selected` | No traversal set. |
| Contact actions/interactions | `selected` — `observed-pack-capability` | Seven actions and two props; contacts/events open. |
| Retargeted/customizable characters | `selected` — `evaluator-selected-generic-scenario` | Unity source actor works; target rig absent. |
| Motion matching/search | `not-selected` | No database contract. |
| Networked movement | `not-selected` | No authority/rollback contract. |
| Runtime performance | `selected` — `evaluator-selected-generic-scenario` | Constant-track pruning sampled; runtime profiling absent. |

## Pack inventory and content evidence

The logical delivery has 114 regular files: 29 FBXs, 63 metadata files, 19 textures/materials, one prefab, and one animation list. The individual motions reconcile to the vendor's current 25-animation count. The combined list also has 25 entries but renames files, including `StandToIdleKneelCampfire` versus delivered `StandToKneelCampfire` and `EatSkewerCampfire` versus `KneelEatSkewerCampfire`; file-scoped identifiers remain authoritative.

All 25 individual motions share skeleton signature `2b6fe49d5ae6` with 56 bones, identical to the standard Basic/Sword/Climbing/Injured family. The combined take and actor use the shared 58-bone actor structure. The props are separate models and are not motion-set members.

## Mechanical baseline

| Finding/check | Affected scope | Potential impact | Evidence |
|---|---|---|---|
| Default file safety | 29/29 FBXs | No NaN, time-order, quaternion, duration, scale, or bind-pose error | `observed-animsmith`; all baseline commands exit 0 |
| Constant tracks | 3,664 notes in 27 animation-bearing files; 3,394 in 25 individuals | Export bloat and retarget evaluation cost | Baseline/contract summaries |
| Declared loop closure | 2 errors in 1/14 loop-declared files | Pose displacement at wrap if kept cyclic | Contract summary |
| Declared loop rotation seam | 8/14 | Once-per-cycle angular pulse | Contract summary |
| Declared loop velocity seam | 6/14 | Once-per-cycle velocity pulse | Contract summary |
| Loop semantics | 7 one-shot-like actions/transitions marked loop | Repeated interaction or snap/restart | Filename/metadata reconciliation |
| Loop-seam ratio availability (0.4.0) | 27 clips: 0 measured, 26 not_applicable, 1 unavailable | Every stationary clip correctly has no real stride to normalize a seam against; previously such clips risked reading as an unlabelled pass or failure | `observed-animsmith`; 0.4.0 baseline `loop_seam_ratio` and `loop_seam_ratio_availability` |
| Gait/phase availability (0.4.0) | 26/27 clips report `gait.phase_availability: measured`; contract `gait-group` is `not_applicable` on all 25 individual contracts | No in-place cyclic ring exists, so no gait anchoring ran; a correct not-applicable, not a refusal or a failure, and it does not certify prop/contact quality | `observed-animsmith`; 0.4.0 baseline `gait_phase_availability` and contract `gait_group_applicability` |
| Root trajectory (0.4.0) | 27/27 clips measured | 0 clips move more than 1 cm horizontally, 26 report stationary, 0 exceed 1° of yaw travel — positive measured confirmation of no root motion; sampled regression facts from the shared metric grid, not continuous-curve or engine-extraction proof | `observed-animsmith`; 0.4.0 baseline `root_trajectory` (measurements v15); delivered this release by closed issue #408 (2026-08-20) |
| Per-bone channel coverage (0.4.0) | `bone_channels` available on measured clips | Confirms canonical per-bone translation/rotation/scale track presence; narrows a composition/prop-mask risk discussion but does not by itself prove a visually acceptable engine mask | `observed-animsmith`; 0.4.0 measurements v15; delivered this release by closed issue #402 (2026-08-20) |

## AnimSmith remediation evidence

| Source issue | Operation/declarations | Result | Independent verification | Remaining caveat |
|---|---|---|---|---|
| Constant tracks in IdleKneel | `transform --prune-constant-tracks` with its declared contract | Exit 0; FBX 815,008 bytes to GLB 53,628 bytes | Output inspect/measure exit 0; fix dry-run exit 0; diff detects intentional change | Lint still reports the original rotation seam; runtime equivalence not proven, so output not adopted. Bounded by open issue #401 (re-run under 0.4.0, produced a new candidate; verified open 2026-08-21). |
| Loop/action semantics | Contract `loop=true` only where Unity metadata says so | Eight files fail and seventeen pass | JSON and Markdown agree for all 25 | Detection does not decide whether metadata or motion is wrong. |

## Engine procedures and evidence

| Runtime | Version | Procedure | Observed result | Remaining gate |
|---|---|---|---|---|
| Unity | 6000.5.8f1 (retained 2026-08-17); `unity-humanoid` revision 1 / 6000.3 import-advice (2026-08-21) | Merge five authorized Unitypackage reconstructions into a disposable project; inventory importers/clips; sample four Campfire clips on shared actor; mix Basic walk to StandToKneel; attach skewer; instantiate campfire. Separately, run `generate import-advice` under the frozen `unity-humanoid` / revision 1 / `6000.3` / `fbx-model-importer` profile on every individual clip. | 25/25 individual Humanoid clips import; 4/4 samples, mixer, and both prop checks pass. Skewer/actor height ratio 0.487; world campfire height 0.286 Unity units (all retained, unchanged). Import-advice: `available`, exit 0; each clip's `lockRootRotation`/`lockRootHeightY`/`lockRootPositionXZ` was derived from the delivered `.fbx.meta` (`useFileUnits:1`; `lockRoot*` absent on every meta, so Unity's serialization default of `false` applied, mapping to `extract`) — an assumption about the Unity serialization default, not observed 6000.5.8f1 importer behavior. **That assumption is now falsified by direct observation — see "Unity headless candidate probe (2026-08-21 correction)" below; the `available`/exit 0 result is unaffected, only the projected lock values were wrong.** | Visual offsets, contacts, loops, controller, compression, target rig, build. |
| Unreal Engine | 5.8 | Official documentation review; run `generate import-advice` under the `unreal` / revision 1 / `5.8` / `fbx-importer` profile on every individual clip (2026-08-21). | Typed refusal `profile_settings_unmodeled`, exit 1: profile revision 1 has no modeled Unreal setting vocabulary. Not import-evaluated; runtime capabilities do not prove pack import. | FBX import/retarget, state machine, events, contacts, build. |
| Godot | 4.7 | Official AnimationTree documentation review; run `generate import-advice` under the `godot` / revision 1 / `4.7` / `resource-importer-scene` profile on every individual clip (2026-08-21). | Typed refusal `profile_settings_unmodeled`, exit 1: profile revision 1 has no modeled Godot setting vocabulary. Not import-evaluated. | Conversion/import, retarget, graph, contacts, export. |
| Bevy | 0.19.0 | Official example review; checked for a `generate addressability` candidate (2026-08-21). | Not evaluated: no generated glTF/GLB candidate exists for this stationary pack because no gait-anchored in-place ring exists to seed one, so there was nothing to inventory. This is a coverage gap, not an observed Bevy failure. glTF-centric route remains project work. A collection-wide headless Unity glTFast import of 134 GLB candidates ran 2026-08-21 (see GLB candidate import below), but none of those candidates came from this pack; Bevy addressability stays not-evaluated for Campfire. | Conversion, mapping, graph, contacts, profiling. |

### Unity headless candidate probe (2026-08-21 correction)

The Unity row above stated an explicit assumption: because `lockRootRotation`, `lockRootHeightY`, and `lockRootPositionXZ` are absent from every delivered `.fbx.meta`, the advice read Unity's serialization default of `false` for each key and projected `extract` for every clip. **That assumption is falsified by direct observation.** Unity `6000.5.8f1` was run headless (`-batchmode -nographics -quit -executeMethod CandidateProbe.Run`) in a **new**, disposable project — the retained five-pack project above was not modified — reading `ModelImporterClipAnimation` on the delivered files together with their delivered `.meta`, across a 120-clip sample spanning all eight collection packs, including this pack's own stationary files (for example `Humanoid@FlintstonesLightCampfire.fbx` and `Humanoid@IdleGrillSkewerCampfire.fbx`):

| Variant | Clips | `lockRootRotation` true | `lockRootHeightY` true | `lockRootPositionXZ` true |
|---|---:|---:|---:|---:|
| In-place (non-`_RM`) | 84 | 84 | 84 | 83 |
| Root-motion (`_RM`) | 36 | 36 | 28 | 5 |

Aggregate across the sample: 120/120 clip definitions inspected, 120/120 `lockRootRotation` true, 112/120 `lockRootHeightY` true, 88/120 `lockRootPositionXZ` true. The delivered importer policy is therefore **bake**, not extract, and it is per-variant and axis-specific: `lockRootPositionXZ` is the discriminator — baked (`true`) for essentially all in-place clips (this pack's stationary files are all in-place by construction, with no `_RM` variants) and mostly extracted (`false`) for root-motion clips in other packs. That is a coherent authored root-motion policy, not an oversight. This observation supersedes the stated default-value assumption in the Unity row above; it does not change that row's `available`/exit 0 result, only the projected lock values, and it does not by itself certify prop or contact quality, which remains a visual gate.

### GLB candidate import into Unity (2026-08-21) — collection-level context, not a pack result

All 134/134 AnimSmith 0.4.0 gait-anchored GLB candidates across the eight-pack collection were staged into a separate, **new** Unity 6000.5.8f1 project using `com.unity.cloud.gltfast` 6.9.0, because Unity has no native GLB importer; the retained five-pack project above was not modified. Result: 134/134 files staged produced assets, 134/134 produced exactly one Unity `AnimationClip`, and every clip is non-legacy and non-empty. **Campfire has no in-place gait ring (see Pipeline-stage coverage above) and therefore contributed none of the 134 candidates** — this result is reported here only as collection-level context, not as a Campfire pack result, and it does not change Bevy addressability, which stays not-evaluated for this pack.

**Limit, stated plainly, for the candidates that do exist:** glTFast imports glTF animation as a **Generic** clip and does not reconstruct a Humanoid Avatar. The 134/134 result proves those candidates load and yield one well-formed clip in Unity; it does **not** test the Humanoid retarget path the source packs actually use, and it is not a visual or gameplay acceptance test.

## Rig, masking, and compatibility evidence

AnimSmith 0.4.0 (measurements v15) adds canonical per-bone `bone_channels`, delivered this release by closed issue #402 ("expose per-clip channel coverage at (bone, property) granularity", closed 2026-08-20), confirming translation/rotation/scale track presence per bone index for the pack's clips. This narrows which joints a hypothetical mask could omit, but channel presence alone does not prove a visually acceptable engine mask; the composition and contact gates below are unchanged.

| Pack/rig/set pair | Skeleton/retarget | Scale/axes | Root policy | Timing/blend | Overall evidence |
|---|---|---|---|---|---|
| Campfire standard motions to supplied actor | 25 files share 56-bone signature; Unity Humanoid import succeeds | Metadata scale 1; props instantiate plausibly | Stationary full-body | Four samples execute | Direct Unity candidate; target deformation/contact untested |
| Campfire to Basic Locomotion | Same standard signature; 25 shared paths byte-identical | Shared actor/material assets identical | Basic owns approach; Campfire owns posture | One headless mixer executes | Strong co-install candidate; transition style/feet unaccepted |
| Campfire skewer action | Shared right-hand bone exists | Local-identity attachment ratio 0.487 | Stationary | Graph not contact-tested | Attachment works mechanically; grip/orientation/contact open |
| Campfire to other evaluated packs | Standard 56-bone signature shared | Every pair has 25 identical overlaps and zero conflicts | Full-body state handoff default | Five-pack Unity import succeeds | Technical co-existence, not artistic compatibility |

## Limitations and unknowns

1. No dynamic visual review, target-character retarget, contact/IK/event authoring, compression comparison, cancellation test, or player build was completed.
2. Screenshots of three AnimSmith offline reports confirmed rendered skeleton/metric/finding views, not motion quality.
3. Unreal Engine, Godot, and Bevy remain documentation-only.
4. Current vendor pages and EULA do not prove the local archive revision or transaction entitlement.
5. Commercial files, derived motion outputs, screenshots, and the generated Unity project remain outside the repository and CI.
6. The 0.4.0 availability recount for `loop_seam_ratio`, `gait.phase_availability`, and `gait-group` clarifies which stationary-pack facts are `not_applicable` versus `unavailable`; it is not a cleaner pass on prop/contact acceptance, which remains unevaluated.
7. Measured `root_trajectory` (0.4.0, delivered by closed issue #408) is a sampled regression fact from the shared uniform metric grid, not continuous-curve or engine-extraction proof; it does not by itself decide movement-ownership axes for a game controller.
8. A 2026-08-21 direct Unity 6000.5.8f1 headless probe falsified the Unity import-advice's stated default-`false`/`extract` assumption for root-lock declarations (see Unity headless candidate probe above): the observed delivered policy is `bake` for in-place clips such as this pack's, and per-axis `bake`/`extract` for root-motion clips in other packs. The probe is headless-import evidence over a 120-clip cross-pack sample, not continuous visual or gameplay acceptance, and it does not certify prop/contact quality.
9. The collection-wide 134/134 GLB-candidate Unity import (2026-08-21) is context only for this pack: Campfire has no gait ring and contributed none of the candidates, so it proves nothing about Campfire content specifically, and Bevy addressability stays not-evaluated here. Where candidates do exist elsewhere, glTFast produces only a Generic clip, not a Humanoid retarget test. A same-commit rebuild of AnimSmith `v0.4.0` produced a differently-hashed binary (SHA-256 `1e53013bbe3224557a8783eafeb818f4ef9d74666590cbaa8c18ef48c5b7d6fa`, versus the recorded `fd1eee57407aa02db88763d144389a7f5104204c40ddfbb28eb5885ca8cd54c6`) — the build is not byte-reproducible — but both builds emit byte-identical import-advice artifacts, so this appendix's regenerated Unity evidence is attributable to the tag and commit, not to the originally recorded binary digest.

## Reproduction

Source RAR: 150,047,944 bytes, SHA-256 `bed86be7f91fdd46b376fce4b1a00c88372a3f703e0fe9077925712d8af8e8e9`. Extracted Unitypackage: 150,181,063 bytes, SHA-256 `9cfd965420a31f0702f7e2d8f886037011c29b33efe8b1da757dfa7750cc4c7a`.

2026-08-17 baseline evaluator (historical): `animsmith 0.3.0 (v0.3.0-30-gaabac28)`; revision `aabac28edf2719db236068339f1208bbf156d0bb`; binary SHA-256 `2fb43d210b5448fb2cd642946cc46df0cbb34595a48821b22a28daf7c1938f77`.

Run `inspect`, `measure --format json`, `lint --format json`, and `lint --format markdown` on every FBX with the humanoid baseline. For each individual file, apply the retained rig profile and Unity-derived loop declaration; declare in-place only for an actual paired non-RM member. Generate three risk-selected offline reports and inspect rendered screenshots. Run the pruning trial, then inspect, measure, lint, diff, and fix dry-run the candidate. Finally import all five evaluated packs into Unity and execute the retained headless probe.

Portable evidence digests (2026-08-17, historical): baseline `f9797cfd04dddac8b366a474dceac08dd968a95c52874398c014c81a1b2f9992`; contract `b9a858bcfce12ef799b06a91242054b8d0aa4a6f257660a41f0393bf20d1e7d2`; catalog `480ded14c195158d8768512e764c442cf14cf1fd04584bd27dfe24fd857ca1b9`; remediation `0e72dade266ad288c5ce2db068370d7563f0437def4177448783ea5bc9644b2e`; Basic comparison `0f6b0f6588822b1d309a6162de615c3de174bce92f6bfa6edc222a4467795903`; combined Unity probe `d2b6d1b0af14c2c77dca3c2cc4aa892d6e507f3cf8b9bb50bfdb4ef78d407afa`.

2026-08-21 refresh evaluator: `animsmith 0.4.0`, tag `v0.4.0`; revision `6b37ad636b198ef8ff47fadbf6a3a51eb1a27c8e`; binary SHA-256 `fd1eee57407aa02db88763d144389a7f5104204c40ddfbb28eb5885ca8cd54c6`; output schema v10; measurements schema v15. **Rebuild reproducibility:** rebuilding tag `v0.4.0` at this same commit produced a binary with a *different* SHA-256, `1e53013bbe3224557a8783eafeb818f4ef9d74666590cbaa8c18ef48c5b7d6fa` — the build is not byte-reproducible. Both builds emit byte-identical import-advice artifacts, verified by `diff`, so the Unity headless-probe correction below is attributable to the tag and commit, not to one specific binary digest. Re-ran the source inventory against the same archive: byte-identical to the published manifest (0 added, 0 removed, 0 changed across 29 FBXs); archive SHA-256 `bed86be7f91fdd46b376fce4b1a00c88372a3f703e0fe9077925712d8af8e8e9` re-verifies. Re-ran `inspect`, `measure --format json`, and `lint --format json` for both the untouched baseline and the retained declared contract on every FBX; findings, exit codes, and constant-track/loop-closure/loop-seam counts reproduce the 2026-08-17 numbers exactly, with the newly available `loop_seam_ratio`, `gait.phase_availability`, `gait-group` applicability, `root_trajectory` (delivered by closed issue #408), and `bone_channels` (delivered by closed issue #402) facts now populated. Ran `generate import-advice` under the frozen `unity-humanoid` (Unity 6000.3), `unreal` (5.8), and `godot` (4.7) profiles on every individual clip: `unity-humanoid` returned `available` (exit 0); `unreal` and `godot` returned the typed `profile_settings_unmodeled` refusal (exit 1). Checked for a Bevy `generate addressability` candidate; none exists because this stationary pack has no gait-anchored in-place ring, so Bevy 0.19.0 is recorded not-evaluated rather than failed. Re-ran the `--prune-constant-tracks` trial on `Humanoid@IdleKneelCampfire.fbx` (source not modified; bounded by open issue #401). The retained Unity 6000.5.8f1 headless probe (2026-08-17) was not rerun because the source is byte-identical; it keeps its original date and attribution.

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

- Local authorized source archive, extracted Unity metadata, and bundled animation list — private evidence identified above, accessed 2026-08-17.
- Protofactor, [Animset: Campfire](https://protofactor.biz/product/animset-campfire/), [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/), and [EULA](https://protofactor.biz/end-user-license-agreement/) — current listing/license context, accessed 2026-08-17.
- Unity Asset Store, [Ultimate Animation Collection](https://marketplace.unity.com/packages/3d/animations/ultimate-animation-collection-195459) — current collection version 1.65, release date 2026-08-16, Single Entity listing, and original Unity 6000.5.1; not local constituent identity.
- Unity, [Animation Blend Trees](https://docs.unity3d.com/6000.5/Documentation/Manual/class-BlendTree.html), [Root Motion](https://docs.unity3d.com/6000.5/Documentation/Manual/RootMotion.html), and [looping clips](https://docs.unity3d.com/6000.5/Documentation/Manual/LoopingAnimationClips.html); Epic Games, [animation system](https://dev.epicgames.com/documentation/unreal-engine/skeletal-mesh-animation-system-in-unreal-engine?application_version=5.8); Godot, [AnimationTree](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html); Bevy, [Animation Masks](https://bevy.org/examples/animation/animation-masks/) — capability context only, accessed 2026-08-17.
- AnimSmith, [Unity 6000.3 animation profile](../engine-profile-unity.md), [Unreal Engine 5.8 animation profile](../engine-profile-unreal.md), [Godot 4.7 animation profile](../engine-profile-godot.md), and [Bevy 0.19.0 animation profile](../engine-profile-bevy.md) — modeled import-advice profile facts for the 2026-08-21 refresh, accessed 2026-08-21.
