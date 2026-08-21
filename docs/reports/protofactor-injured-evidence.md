# Animation pack evidence appendix: Protofactor Injured

> Companion report: [Protofactor Injured](protofactor-injured.md)
>
> Evidence status: **partial** — exhaustive file/AnimSmith coverage, Unity 6000.5.8f1 headless evidence, and 0.4.0 gait-anchor plus per-engine advice/addressability checks; visual loops, blends, masks, target-character, and full three-engine (Unreal/Godot/Bevy) passes remain absent.
>
> Evaluation date: **2026-08-21**
>
> Report format: **1**

This appendix uses the [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder) without redefining it.

## Evaluation scope and provenance

| Field | Value |
|---|---|
| Pack/edition | Protofactor Injured constituent from a locally held Ultimate Animation Collection archive; local revision unknown |
| Vendor/source | Protofactor [Injured product](https://protofactor.biz/product/animset-injured/) and [collection](https://protofactor.biz/product/ultimate-animation-collection/) pages |
| Delivered scope | RAR to one Unitypackage to 171 logical files; 72 FBXs: 70 individual motions, one combined take, one actor |
| Target use | Game-engine injured locomotion/posture states combined with Basic Locomotion, Sword & Shield, and contextual packs |
| Target engines | Unity 6000.5.8f1 observed (retained 2026-08-17); 0.4.0 (2026-08-21) evaluator advice/addressability probes against `unity-humanoid` (Unity 6000.3), `unreal` (5.8), `godot` (4.7), and `bevy` (0.19.0), all revision 1 |
| Target rigs/packs | Supplied Protof-Actor; Basic Locomotion, Sword & Shield, Campfire, and Climbing selective compatibility |
| Source manifest | `injured/source-archive-inventory.json`; RAR SHA-256 `b459ab3a39a15aa2e499c633f661616449bfc281836858ad9525014184aa9878` |
| Evaluation manifest | `injured/evidence/evaluation-manifest.json`; SHA-256 `ad98ac7639c997a6d7a3eabb7552b2bbb06ab1c797013cf84cb86e764a3159f5`; schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1`; taxonomy/profile-set version 1 |
| Acquisition/license provenance | User states the local archive was downloaded from Protofactor; current [vendor EULA](https://protofactor.biz/end-user-license-agreement/) reviewed 2026-08-17 (not re-reviewed this refresh). No receipt or local revision record was evaluated; no legal opinion. |

0.4.0 re-inventoried the source archive on 2026-08-21 and reproduced the published manifest exactly: 0 added, 0 removed, 0 content changed across 72 FBXs; the source RAR digest and evaluation-manifest digest above are unchanged and re-verify. This refresh is evaluator-version-only — the source identity is untouched.

### Evidence coverage

| Surface | Offered/delivered | Evaluated | Findings | Not evaluated and why |
|---|---:|---:|---:|---|
| Animation files | 70 individual plus 1 combined | 71 | 42 individual contract failures | Dynamic visual quality and combined segmentation |
| Rigs/export variants | 2 observed structures | 2 | Standard 56; combined/actor 58 | Target-character deformation |
| AnimSmith baseline | 72 FBXs | 72 | 9,915 constant-track notes; unchanged from 0.3.0 | Default lint lacks gait intent |
| Declared contracts | 70 individual files | 70 | 28 clean; 42 failing; 9,644 notes (unchanged); loop-seam applicability 48/22, evaluation 25/45 complete (0.4.0) | Visual loop/blend/transition quality |
| Gait measurements | 14 IP/RM pairs | 14 | Matching pair phase/duration; style speeds/phase vary; heading axis `positive_y` on 71/72 clips (0.4.0) | Contact-side/stride semantics |
| Gait-anchor candidates | 14 IP gait files | 14 attempted | 0.4.0: all 14 anchor, exit 0 (circular spread 0.554→0.110 run; 0.603→0.051 walk), delivered by [issue #426](https://github.com/mmannerm/animsmith/issues/426) (closed 2026-08-18); 0.3.0 (2026-08-17, historical): all safely refused under the [issue #407](https://github.com/mmannerm/animsmith/issues/407) (closed 2026-08-17) fail-closed policy, no output | Unpromoted: no engine import this session |
| Engine import/playback | 70 individual clips | 70 imported; 6 sampled | Required samples pass; 0.4.0 adds `unity-humanoid`/`unreal`/`godot`/`bevy` advice-level probes (see below) | Controller, compression, player build |
| Blend/mask/retarget | 2 mixers; 1 mask graph | 3 graph executions | Execution passes | Visual blend/mask and target rig |

### Claim legend

`observed-file` means derived from delivered files/metadata; `observed-animsmith` means reproduced with the named evaluator; `observed-engine` means the headless Unity probe; `inferred` marks semantic grouping. None means gameplay acceptance.

## Evaluation manifest and taxonomy

The retained evaluation manifest uses schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1` and taxonomy/profile-set version 1.

### Canonical clip-role inventory

| Canonical primary role | Logical motions | Delivered files | Evidence boundary |
|---|---:|---:|---|
| `idle-pose` | 21 | 21 | Seven standing, kneeling, and sitting styles |
| `continuous-locomotion` | 14 | 28 | Seven walk and seven run IP/RM pairs |
| `locomotion-transition` | 21 | 21 | Sit/kneel entry and sit return files |
| `airborne` | 0 | 0 | Absent |
| `traversal` | 0 | 0 | Absent |
| `action-interaction` | 0 | 0 | Absent |
| `reaction-death` | 0 | 0 | Absent |
| `emote-cinematic` | 0 | 0 | Absent |
| `other-unknown` | 0 | 0 | None |
| **Total** | **56** | **70** | Validated manifest SHA-256 `ad98ac7639c997a6d7a3eabb7552b2bbb06ab1c797013cf84cb86e764a3159f5` |

### Runtime-set inventory

| Runtime set | Type | Members/variants | Grouping evidence | Validation status |
|---|---|---|---|---|
| Style A locomotion | speed-blend | idle plus walk/run IP/RM; 5 files | Letter family and paired evidence; medium confidence | Threshold 0.540/2.021 m/s; loops visual-open |
| Style B locomotion | speed-blend | idle plus walk/run IP/RM; 5 files | Letter family and paired evidence; medium confidence | 0.541/2.060 m/s; Run IP loop inferred; visual-open |
| Style C locomotion | speed-blend | idle plus walk/run IP/RM; 5 files | Letter family and paired evidence; medium confidence | 0.494/1.748 m/s; blend phase 0.153 cycles |
| Style D locomotion | speed-blend | idle plus walk/run IP/RM; 5 files | Letter family and paired evidence; medium confidence | 0.504/1.821 m/s; blend phase 0.105 cycles |
| Style E locomotion | speed-blend | idle plus walk/run IP/RM; 5 files | Letter family and paired evidence; medium confidence | 0.733/2.005 m/s; blend phase 0.181 cycles |
| Style F locomotion | speed-blend | idle plus walk/run IP/RM; 5 files | Letter family and paired evidence; medium confidence | 0.692/2.005 m/s; blend phase 0.170 cycles |
| Style G locomotion | speed-blend | idle plus walk/run IP/RM; 5 files | Letter family and paired evidence; medium confidence | 0.519/2.127 m/s; blend phase 0.075 cycles |
| Style A posture chain | transition-chain | 6 single files | Standing/sit/kneel labels; high confidence | No kneel exit; transitions visual-open |
| Style B posture chain | transition-chain | 6 single files | Standing/sit/kneel labels; high confidence | No kneel exit; transitions visual-open |
| Style C posture chain | transition-chain | 6 single files | Standing/sit/kneel labels; high confidence | No kneel exit; transitions visual-open |
| Style D posture chain | transition-chain | 6 single files | Standing/sit/kneel labels; high confidence | No kneel exit; transitions visual-open |
| Style E posture chain | transition-chain | 6 single files | Standing/sit/kneel labels; high confidence | No kneel exit; transitions visual-open |
| Style F posture chain | transition-chain | 6 single files | Standing/sit/kneel labels; high confidence | No kneel exit; transitions visual-open |
| Style G posture chain | transition-chain | 6 single files | Standing/sit/kneel labels; high confidence | No kneel exit; transitions visual-open |

### Pipeline-stage coverage

| Stage | Coverage state | Evidence / remaining gate |
|---|---|---|
| Acquire | `evaluated-clean` | Local vendor archive identified and hashed; transaction record absent. |
| Preserve raw | `evaluated-clean` | RAR and Unitypackage retained unchanged outside the repository. |
| Inspect | `evaluated-finding` | Every FBX inspected/measured/linted; seam and optimization findings retained. |
| Segment | `partially-evaluated` | Individual files used; combined take not promoted. |
| Root motion | `evaluated-finding` | 14 IP/RM pairs measured; heading axis resolves to `positive_y` on 71/72 clips (0.4.0); all 14 gait-anchor attempts now succeed, exit 0, unpromoted, via delivered [issue #426](https://github.com/mmannerm/animsmith/issues/426) (closed 2026-08-18). The 2026-08-17 refusal under the [issue #407](https://github.com/mmannerm/animsmith/issues/407) (closed 2026-08-17) fail-closed policy is retained as historical. Root displacement/yaw is measured per [issue #408](https://github.com/mmannerm/animsmith/issues/408) (closed 2026-08-20). |
| Conform | `partially-evaluated` | Standard skeleton and Unity shared Avatar work; target rigs open. |
| Validate | `partially-evaluated` | Mechanical contracts and headless Unity complete; visual gameplay open. |
| Optimize | `evaluated-finding` | One pruning candidate verified mechanically but not accepted semantically. |
| Export | `partially-evaluated` | Sample GLB export reopens; Unity native delivery tested; other engines open. |
| Gate/report | `evaluated-clean` | Manifest and parser-validated report pair retained. |

### Readiness evidence by clip set

| Role or runtime set | File-ready / clip-ready | Set-ready / rig-use | Runtime / acceptance boundary |
|---|---|---|---|
| Seven locomotion styles | 14 IP/RM pairs readable and duration/phase paired | Standard skeleton; thresholds differ; all explicit loops fail | Unity samples/mixers execute; 0.4.0 gait-anchor candidates unpromoted (no engine import this session); gait blends and wraps open |
| Standing idles | Seven files readable | 5/7 strict seam failures | Mask graph executes; full-body/upper-body visual choice open |
| Sit/kneel chains | 35 posture/transition files readable | Style letters coherent; kneel exits absent | Unity samples execute; endpoints/recovery/cancellation open |

### Validation-profile status

| Validation profile | Selection | Result / next evidence |
|---|---|---|
| Marketplace intake | `selected` — `vendor-intended` | Archive/listing/EULA reviewed; listing and local RM counts disagree. |
| Blended locomotion | `selected` — `vendor-intended` | Seven speed sets measured; phase/threshold visual gates open. |
| Root-motion controller | `selected` — `observed-pack-capability` | 14 named pairs; 0.4.0 gait-anchor succeeds unpromoted (delivered [issue #426](https://github.com/mmannerm/animsmith/issues/426), closed 2026-08-18). Root trajectory sampled 72/72 (15 move >1 cm; 56 stationary; 0 with yaw travel >1°) via [issue #408](https://github.com/mmannerm/animsmith/issues/408) (closed 2026-08-20) as a shared-metric-grid regression check, not continuous-curve or engine-extraction proof — no movement-ownership axis is declared from it. Authority and in-engine displacement remain open. |
| State-machine transitions | `selected` — `observed-pack-capability` | Sit/kneel families; kneel return and visual endpoints open. |
| Layered upper body/weapons | `selected` — `evaluator-selected-generic-scenario` | One mask and Sword handoff graph executes; visual pose open. |
| Traversal/environment | `not-selected` | No traversal set. |
| Contact actions/interactions | `not-selected` | No contact-action set. |
| Retargeted/customizable characters | `selected` — `evaluator-selected-generic-scenario` | Unity source actor works; target rig absent. |
| Motion matching/search | `not-selected` | No database/contact annotation contract. |
| Networked movement | `not-selected` | No authority/rollback contract. |
| Runtime performance | `selected` — `evaluator-selected-generic-scenario` | Constant-track pruning sampled; runtime profiling absent. |

## Pack inventory and content evidence

The delivery has 171 regular files and 72 FBXs: 70 individual motions, one combined take, and one actor. Individual files form seven A–G styles with 14 logical gaits delivered as IP/RM pairs, 21 idles, and 21 transitions. The current product page says 70 animations, zero root motion, and 70 in-place; the local archive has 70 individual files including 14 `_RM` files, and its bundled list also identifies RM entries.

All 70 individual motions share skeleton signature `2b6fe49d5ae6` with 56 bones. The combined take and actor use the collection's shared 58-bone structure.

## Mechanical baseline

| Finding/check | Affected scope | Potential impact | Evidence |
|---|---|---|---|
| Default file safety | 72/72 FBXs | No NaN, time-order, quaternion, duration, scale, or bind-pose error | `observed-animsmith`; all baseline commands exit 0 |
| Constant tracks | 9,915 notes in 72 files; 9,644 in 70 individuals | Export bloat and retarget evaluation cost | Baseline/contract summaries; per-(bone, property) coverage granularity delivered by [issue #402](https://github.com/mmannerm/animsmith/issues/402) (closed 2026-08-20) |
| Declared loop closure | 15/48 loop files | Position discontinuity at wrap | Contract summary |
| Declared loop rotation seam | 42/48 | Angular pulse at wrap | Contract summary |
| Declared loop velocity seam | 31/48 | Velocity pulse at wrap | Contract summary |
| Contract result | 42/70 files fail | Every explicit locomotion loop and 15 idles affected; counts unchanged from published 0.3.0 | Per-file JSON/Markdown agreement |
| Loop-seam applicability (0.4.0) | 48/70 applicable; 22/70 not applicable | Separates no-stride clips (mostly idles) from locomotion, ahead of the raw pass/fail split above | `contract.loop_seam_applicability` |
| Loop-seam evaluation completeness (0.4.0) | 25/70 complete; 45/70 not evaluated | No-stride clips are now recorded as not evaluated rather than mislabelled failures; the 42-file findings count above is otherwise unchanged | `contract.loop_seam_evaluation` |

## AnimSmith remediation evidence

| Source issue | Operation/declarations | Result | Independent verification | Remaining caveat |
|---|---|---|---|---|
| Gait phase normalization (0.3.0, 2026-08-17, historical) | `transform --gait-anchor` on 14 IP walk/run files | All exit 2 with no output: root lacks finite horizontal forward axis, enforced by the [issue #407](https://github.com/mmannerm/animsmith/issues/407) fail-closed policy (closed 2026-08-17) | Output-absence and diagnostic retained | Superseded by the 0.4.0 result below; safe refusal produced no candidate. |
| Gait phase normalization (0.4.0, 2026-08-21) | `transform --gait-anchor` on the same 14 IP walk/run files, now measuring heading axis `positive_y` | All 14 anchor: exit 0, via vertical-forward-axis support delivered by [issue #426](https://github.com/mmannerm/animsmith/issues/426) (closed 2026-08-18). Circular ring spread: Run 0.554071→0.109841 (7 members); Walk 0.602542→0.051348 (7 members) | Per-file exit codes and measurements retained; root trajectory sampled 72/72 (via [issue #408](https://github.com/mmannerm/animsmith/issues/408), closed 2026-08-20) shows 0 clips with yaw travel >1° (a grid regression check, not continuous-curve or engine-extraction proof) | Candidates unpromoted: no engine import performed this session; still needs visual/engine acceptance before adoption. |
| Constant tracks in Walk A | `transform --prune-constant-tracks` with declared contract | Exit 0; FBX 658,816 bytes to GLB 45,152 bytes | Output inspect/measure and fix dry-run exit 0; diff detects change | Lint retains source seam; equivalence unproven, so output not adopted; still bounded by [issue #401](https://github.com/mmannerm/animsmith/issues/401). |

## Engine procedures and evidence

| Runtime | Version | Procedure | Observed result | Remaining gate |
|---|---|---|---|---|
| Unity | 6000.5.8f1 (retained, 2026-08-17) | Merge five authorized package reconstructions outside repo; inventory; sample six Injured clips; mix Basic and Sword into Injured; build one Basic-lower/Injured-upper mask graph. | 70/70 individual Humanoid clips import; 6/6 samples, both mixers, and mask graph pass. Byte-identical source justifies keeping this dated evidence current. | Visual loops/blends/mask, controller, target rig, compression, build. |
| Unity | 6000.3, `unity-humanoid` revision 1, `fbx-model-importer` (0.4.0, 2026-08-21) | `generate import-advice` against declarations derived from every delivered `.fbx.meta`: `useFileUnits: 1`; no delivered meta declares `lockRootHeightY`/`lockRootRotationY`/`lockRootPositionXZ`, so each is assumed to take Unity's serialized default of `false`, which the profile projects to `extract` for root rotation, root Y, and root XZ. | Available, exit 0, for all clips. This is 6000.3 import-advice derived from shipped metadata, not observed Unity 6000.5.8f1 import/playback behavior. | Confirm the assumed `lockRoot*` defaults against an actual 6000.3 project; visual/controller acceptance. |
| Unreal Engine | 5.8, `unreal` revision 1 (0.4.0, 2026-08-21) | `generate import-advice` attempt against the profile's modeled settings. | Typed refusal `profile_settings_unmodeled`, exit 1: profile revision 1 does not yet model Unreal 5.8 settings. | FBX import/retarget, blend spaces, markers, root authority, masks, build; still no capability evidence beyond the refusal. |
| Godot | 4.7, `godot` revision 1 (0.4.0, 2026-08-21) | `generate import-advice` attempt against the profile's modeled settings. | Typed refusal `profile_settings_unmodeled`, exit 1: profile revision 1 does not yet model Godot 4.7 settings. | Conversion/import, retarget, phase policy, filters, export; still no capability evidence beyond the refusal. |
| Bevy | 0.19.0, `bevy` revision 1 (0.4.0, 2026-08-21) | `generate addressability` on one generated GLB candidate from this session. | Exit 0: 1 animation row, coverage complete, predicted selector `Animation0`, facet available, 0 findings. | glTF conversion of the delivered source, mapping, phase policy, masks, profiling; this is inventory/selector prediction only, not an import or playback test. |

## Rig, masking, and compatibility evidence

| Pack/rig/set pair | Skeleton/retarget | Scale/axes | Root policy | Timing/blend | Overall evidence |
|---|---|---|---|---|---|
| Injured standard motions to supplied actor | All 70 share 56-bone signature; Unity Humanoid succeeds | Metadata scale 1 | Paired RM/IP choice | Six samples execute | Direct Unity candidate; target deformation untested |
| Injured IP/RM pairs | Same skeleton and duration; phase nearly identical within pair | Same scale | Exclusive movement authority | Walk/run phase differs by style | Strong pairing evidence; loop/blend visual-open |
| Injured to Basic Locomotion | Same standard signature; 25 shared paths byte-identical | Shared assets identical | Basic or Injured movement owns lower body | Mixer and upper/lower mask graph execute | Co-install candidate; visual style/mask open |
| Injured to Sword/other packs | Standard signature shared | All pairwise overlaps identical, zero conflicts | Full-body handoff default | Sword mixer and five-pack import succeed | Technical co-existence, not artistic compatibility |

## Limitations and unknowns

1. No dynamic visual review, target retarget, loop/blend/mask acceptance, transition endpoints, compression comparison, network correction, or player build was completed.
2. Gait phase is a mechanical proxy; contact side, stride semantics, and gameplay feel remain unmeasured.
3. Screenshots of three offline reports prove rendered frame-zero diagnostics, not motion quality.
4. Unreal Engine and Godot remain documentation-only beyond 0.4.0's typed `profile_settings_unmodeled` refusal; Bevy remains documentation-only beyond one generated-GLB addressability inventory/selector prediction. None of the three is import- or playback-tested.
5. Commercial files, derived motion outputs, screenshots, and the generated Unity project remain outside the repository and CI.
6. Root trajectory (72/72 clips; 15 move >1 cm; 0 with yaw travel >1°), measured per [issue #408](https://github.com/mmannerm/animsmith/issues/408) (closed 2026-08-20), is a sampled regression check on AnimSmith's shared metric grid, not continuous-curve or engine-extraction proof; it must not be read as declaring which axis owns movement.

## Reproduction

Source RAR: 157,809,050 bytes, SHA-256 `b459ab3a39a15aa2e499c633f661616449bfc281836858ad9525014184aa9878`; re-verified byte-identical on 2026-08-21. Extracted Unitypackage: 161,663,801 bytes, SHA-256 `3227c487fd1a2f1bc69e569171a3e5fae3f6a062dffb65f829248802046aaa09`.

Historical evaluator (0.3.0, 2026-08-17, superseded): `animsmith 0.3.0 (v0.3.0-30-gaabac28)`; revision `aabac28edf2719db236068339f1208bbf156d0bb`; binary SHA-256 `2fb43d210b5448fb2cd642946cc46df0cbb34595a48821b22a28daf7c1938f77`.

Current evaluator (0.4.0, 2026-08-21): `animsmith 0.4.0 (v0.4.0)`; revision `6b37ad636b198ef8ff47fadbf6a3a51eb1a27c8e`; binary SHA-256 `fd1eee57407aa02db88763d144389a7f5104204c40ddfbb28eb5885ca8cd54c6`; output schema v10; measurements schema v15.

Run baseline commands on every FBX. For every individual file, apply the humanoid rig, Unity-derived loop declaration, and in-place only to the non-RM side of a true pair. Measure each gait, compute pair/family phase evidence, and attempt gait anchoring without promoting refused candidates. Generate and inspect three risk-selected offline reports. Prune one sample and run inspect/measure/lint/diff/fix dry-run. Import all five evaluated packs into Unity and execute the retained probe. For the 0.4.0 refresh (2026-08-21): re-inventory the source archive and confirm the manifest reproduces exactly; re-run baseline and contract lint; re-run `transform --gait-anchor` on the 14 selected IP gaits and retain the succeeding output without promotion; re-run the constant-track prune trial on `Humanoid@WalkInjuredA.fbx`; and run `generate import-advice`/`generate addressability` across the `unity-humanoid`, `unreal`, `godot`, and `bevy` profiles without further engine import this session.

Portable evidence digests (0.3.0, 2026-08-17, historical): baseline `7d3653df78ed84a4213a0c5e2b0d65a61cd7696704edb58d29ad96f398e82dc0`; contract `a59a242483aec788c6cc928096101072d12752128134fd3d54b7d062637d321e`; catalog `08acae7a14c6717877afd255c4eafaf4c147224b2f3ccbd4ead25429498a6d43`; remediation `9cb1afa636de97a57a8ef9f73361955356684c10bb17e0ea8744195768c08df9`; Basic comparison `576ecaac0918834ab840199e2d9b4e555c1c6130e9f0dde185376dc624e1a57`; combined Unity probe `d2b6d1b0af14c2c77dca3c2cc4aa892d6e507f3cf8b9bb50bfdb4ef78d407afa`. 0.4.0 portable evidence digests were not captured this session.

## Sources

- Local authorized source archive, extracted Unity metadata, and bundled animation list — private evidence identified above, accessed 2026-08-17 and re-verified byte-identical 2026-08-21.
- Protofactor, [Animset: Injured](https://protofactor.biz/product/animset-injured/), [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/), and [EULA](https://protofactor.biz/end-user-license-agreement/) — current context, accessed 2026-08-17.
- Unity Asset Store, [Ultimate Animation Collection](https://marketplace.unity.com/packages/3d/animations/ultimate-animation-collection-195459) — current collection version 1.65, release date 2026-08-16, Single Entity listing, and original Unity 6000.5.1; not local constituent identity.
- Unity, [Animation Blend Trees](https://docs.unity3d.com/6000.5/Documentation/Manual/class-BlendTree.html), [Root Motion](https://docs.unity3d.com/6000.5/Documentation/Manual/RootMotion.html), and [looping clips](https://docs.unity3d.com/6000.5/Documentation/Manual/LoopingAnimationClips.html); Epic Games, [animation system](https://dev.epicgames.com/documentation/unreal-engine/skeletal-mesh-animation-system-in-unreal-engine?application_version=5.7); Godot, [AnimationTree](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html); Bevy, [Animation Masks](https://bevy.org/examples/animation/animation-masks/) — capability context only, accessed 2026-08-17.
- AnimSmith issue tracker, verified live 2026-08-21. Open: [#401](https://github.com/mmannerm/animsmith/issues/401) (constant-track pruning equivalence) and [#411](https://github.com/mmannerm/animsmith/issues/411) (declared-set speed/stride coherence). Closed and delivered in this release: [#407](https://github.com/mmannerm/animsmith/issues/407) (2026-08-17, fail-closed gait-anchor safety policy — the source of the 0.3.0 refusal), [#426](https://github.com/mmannerm/animsmith/issues/426) (2026-08-18, gait-anchor support for in-place rigs with a vertical root forward axis — the source of the 0.4.0 14/14 result), [#402](https://github.com/mmannerm/animsmith/issues/402) (2026-08-20, per-(bone, property) channel coverage), and [#408](https://github.com/mmannerm/animsmith/issues/408) (2026-08-20, root displacement/accumulated-yaw measurement).
