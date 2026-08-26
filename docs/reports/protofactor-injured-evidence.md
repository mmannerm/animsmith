# Animation pack evidence appendix: Protofactor Injured

> Companion report: [Protofactor Injured](protofactor-injured.md)
>
> Evidence status: **partial** — exhaustive file/AnimSmith coverage, Unity 6000.5.8f1 headless evidence, 0.4.0 gait-anchor plus per-engine advice/addressability checks, a corrected observed Unity root-lock policy, and a new-project GLB import test; visual loops, blends, masks, target-character, and full three-engine (Unreal/Godot/Bevy) passes remain absent.
>
> Evaluation date: **2026-08-26**
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
| Gait-anchor candidates | 14 IP gait files | 14 attempted | 0.4.0: all 14 anchor, exit 0 (circular spread 0.554→0.110 run; 0.603→0.051 walk), delivered by [issue #426](https://github.com/mmannerm/animsmith/issues/426) (closed 2026-08-18); 0.3.0 (2026-08-17, historical): all safely refused under the [issue #407](https://github.com/mmannerm/animsmith/issues/407) (closed 2026-08-17) fail-closed policy, no output | Unpromoted: no Humanoid-retarget engine import this session; all 14 were staged in a separate new-project GLB import test (Generic clips only; see Engine procedures and evidence) |
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
| Export | `partially-evaluated` | Sample GLB export reopens; Unity native delivery tested; a separate new-project GLB import test now confirms all 14 gait candidates load as Generic clips (see Engine procedures and evidence); Humanoid retarget and other engines remain open. |
| Gate/report | `evaluated-clean` | Manifest and parser-validated report pair retained. |

### Readiness evidence by clip set

| Role or runtime set | File-ready / clip-ready | Set-ready / rig-use | Runtime / acceptance boundary |
|---|---|---|---|
| Seven locomotion styles | 14 IP/RM pairs readable and duration/phase paired | Standard skeleton; thresholds differ; all explicit loops fail | Unity samples/mixers execute; 0.4.0 gait-anchor candidates unpromoted (no Humanoid-retarget engine import this session; all 14 loaded as Generic clips in the new-project GLB import test); gait blends and wraps open |
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
| Gait phase normalization (0.4.0, 2026-08-21) | `transform --gait-anchor` on the same 14 IP walk/run files, now measuring heading axis `positive_y` | All 14 anchor: exit 0, via vertical-forward-axis support delivered by [issue #426](https://github.com/mmannerm/animsmith/issues/426) (closed 2026-08-18). Circular ring spread: Run 0.554071→0.109841 (7 members); Walk 0.602542→0.051348 (7 members) | Per-file exit codes and measurements retained; root trajectory sampled 72/72 (via [issue #408](https://github.com/mmannerm/animsmith/issues/408), closed 2026-08-20) shows 0 clips with yaw travel >1° (a grid regression check, not continuous-curve or engine-extraction proof) | Candidates remain unpromoted for Humanoid retarget: no Unity Humanoid import ran this session. All 14 were staged in a separate new Unity 6000.5.8f1 project with `com.unity.cloud.gltfast` 6.9.0 and each produced exactly one Generic AnimationClip (see Engine procedures and evidence); still needs visual/Humanoid-retarget/engine acceptance before adoption. |
| Constant tracks in Walk A | `transform --prune-constant-tracks` with declared contract | Exit 0; FBX 658,816 bytes to GLB 45,152 bytes | Output inspect/measure and fix dry-run exit 0; diff detects change | Lint retains source seam; equivalence unproven, so output not adopted; still bounded by [issue #401](https://github.com/mmannerm/animsmith/issues/401). |

## Engine procedures and evidence

| Runtime | Version | Procedure | Observed result | Remaining gate |
|---|---|---|---|---|
| Unity | 6000.5.8f1 (retained, 2026-08-17) | Merge five authorized package reconstructions outside repo; inventory; sample six Injured clips; mix Basic and Sword into Injured; build one Basic-lower/Injured-upper mask graph. | 70/70 individual Humanoid clips import; 6/6 samples, both mixers, and mask graph pass. Byte-identical source justifies keeping this dated evidence current. | Visual loops/blends/mask, controller, target rig, compression, build. |
| Unity | 6000.3, `unity-humanoid` revision 1, `fbx-model-importer` (0.4.0, 2026-08-21; corrected) | `generate import-advice` regenerated against every delivered `.fbx.meta`. The published reading was an unverified assumption — that an absent `lockRootRotation`/`lockRootHeightY`/`lockRootPositionXZ` key takes Unity's serialized default of `false`, mapping to `extract` — and it is now corrected: a 2026-08-21 headless Unity 6000.5.8f1 probe read `ModelImporterClipAnimation` directly off a 120-clip cross-pack sample (15 clips from each of the eight evaluated packs, including 15 in-place Injured files: `GoToKneelInjured{A–G}`, `GoToSitInjured{A–G}`, `IdleInjuredA`). | Direct observation falsifies the earlier assumption. Across the 120-clip sample, in-place clips (84) show `lockRootRotation` true 84/84, `lockRootHeightY` true 84/84, `lockRootPositionXZ` true 83/84; root-motion (`_RM`) clips (36) show `lockRootRotation` true 36/36, `lockRootHeightY` true 28/36, `lockRootPositionXZ` true only 5/36. The delivered policy is **bake** (`true`), not extract, and it is per-variant and axis-specific: XZ is the discriminator, baked for in-place and mostly extracted for root motion — a coherent authored root-motion policy. All 15 sampled Injured clips are in-place and observed true/true/true, consistent with the in-place row; none of this pack's own 14 `_RM` gait files were in the sampled 120. Regenerated import-advice now projects `lock_root_rotation`=true, `lock_root_height_y`=true, `lock_root_position_xz`=true for in-place clips and false for root-motion clips, matching observation. | Confirm the corrected projection against this pack's own 14 `_RM` gait files specifically (none were in the cross-pack sample); visual/controller acceptance of the baked-root-motion result remains open. |
| Unity (GLB import test) | 6000.5.8f1, new project, `com.unity.cloud.gltfast` 6.9.0 (0.4.0, 2026-08-21) | Staged all 134 AnimSmith 0.4.0 gait-anchored GLB candidates from all eight evaluated packs — including all 14 of this pack's own current gait-anchor candidates — into a brand-new Unity 6000.5.8f1 project, since Unity has no native GLB importer; the retained eight-pack project above was not modified or rerun. | 134/134 candidates produced assets and exactly one AnimationClip each, all non-legacy and non-empty (Injured contributed 14/14). glTFast imports glTF animation as **Generic** and reconstructs no Humanoid Avatar: this proves the candidates load and yield a well-formed clip, not that the Humanoid retarget path these clips need works, and it is not visual or gameplay acceptance. Candidates remain unpromoted. | Supersedes the earlier blanket "Unity project has no GLB importer" blocker — the importer had to be added to a separate project; Humanoid retarget and visual/gameplay acceptance of the 14 candidates remain open. |
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
7. The corrected Unity root-lock observation (Engine procedures and evidence) is a 120-clip cross-pack sample, not exhaustive per pack; the 15 sampled Injured clips are all in-place idle/posture files, so this pack's own 14 `_RM` gait files were not directly observed, though the sampled pattern is consistent with the aggregate in-place/root-motion split.
8. The new-project GLB import test (134/134 candidates, 14 from this pack) proves glTFast produces one well-formed Generic AnimationClip per candidate; it does not test this pack's Humanoid retarget path and is not visual or gameplay acceptance.

## Reproduction

Source RAR: 157,809,050 bytes, SHA-256 `b459ab3a39a15aa2e499c633f661616449bfc281836858ad9525014184aa9878`; re-verified byte-identical on 2026-08-21. Extracted Unitypackage: 161,663,801 bytes, SHA-256 `3227c487fd1a2f1bc69e569171a3e5fae3f6a062dffb65f829248802046aaa09`.

Historical evaluator (0.3.0, 2026-08-17, superseded): `animsmith 0.3.0 (v0.3.0-30-gaabac28)`; revision `aabac28edf2719db236068339f1208bbf156d0bb`; binary SHA-256 `2fb43d210b5448fb2cd642946cc46df0cbb34595a48821b22a28daf7c1938f77`.

Current evaluator (0.4.0, 2026-08-21): `animsmith 0.4.0 (v0.4.0)`; revision `6b37ad636b198ef8ff47fadbf6a3a51eb1a27c8e`; binary SHA-256 `fd1eee57407aa02db88763d144389a7f5104204c40ddfbb28eb5885ca8cd54c6`; output schema v10; measurements schema v15.

A 2026-08-21 rebuild of this pinned commit (tag `v0.4.0`) produced a binary with a different SHA-256, `1e53013bbe3224557a8783eafeb818f4ef9d74666590cbaa8c18ef48c5b7d6fa`, than the digest recorded above; the build is not byte-reproducible. Both builds emit byte-identical advice artifacts (verified by `diff`), so the regenerated Unity import-advice and the corrected root-lock reading in this refresh are attributable to tag `v0.4.0` / commit `6b37ad636b198ef8ff47fadbf6a3a51eb1a27c8e`, not to this specific recorded binary digest.

Run baseline commands on every FBX. For every individual file, apply the humanoid rig, Unity-derived loop declaration, and in-place only to the non-RM side of a true pair. Measure each gait, compute pair/family phase evidence, and attempt gait anchoring without promoting refused candidates. Generate and inspect three risk-selected offline reports. Prune one sample and run inspect/measure/lint/diff/fix dry-run. Import all five evaluated packs into Unity and execute the retained probe. For the 0.4.0 refresh (2026-08-21): re-inventory the source archive and confirm the manifest reproduces exactly; re-run baseline and contract lint; re-run `transform --gait-anchor` on the 14 selected IP gaits and retain the succeeding output without promotion; re-run the constant-track prune trial on `Humanoid@WalkInjuredA.fbx`; run `generate import-advice`/`generate addressability` across the `unity-humanoid`, `unreal`, `godot`, and `bevy` profiles; run a headless Unity 6000.5.8f1 probe of `ModelImporterClipAnimation` over a 120-clip cross-pack sample to correct the assumed root-lock defaults; and stage all 134 gait-anchor GLB candidates (14 from this pack) into a separate new Unity 6000.5.8f1 project with `com.unity.cloud.gltfast` 6.9.0 to confirm each imports as exactly one Generic AnimationClip, without modifying the retained eight-pack project.

Portable evidence digests (0.3.0, 2026-08-17, historical): baseline `7d3653df78ed84a4213a0c5e2b0d65a61cd7696704edb58d29ad96f398e82dc0`; contract `a59a242483aec788c6cc928096101072d12752128134fd3d54b7d062637d321e`; catalog `08acae7a14c6717877afd255c4eafaf4c147224b2f3ccbd4ead25429498a6d43`; remediation `9cb1afa636de97a57a8ef9f73361955356684c10bb17e0ea8744195768c08df9`; Basic comparison `576ecaac0918834ab840199e2d9b4e555c1c6130e9f0dde185376dc624e1a57`; combined Unity probe `d2b6d1b0af14c2c77dca3c2cc4aa892d6e507f3cf8b9bb50bfdb4ef78d407afa`. 0.4.0 portable evidence digests were not captured this session.

### Current evaluator: AnimSmith 0.7.0 (2026-08-26)

The unchanged corpus was rerun with `animsmith 0.7.0`, exact tag `v0.7.0`, commit `461ac8a4f6bb368eb8637471a796f13eeb647140`, binary SHA-256 `01a501999c91d93abfb32b1f48241fccc70914fac27c9a650c31df44262578d8`, output schema v17, and measurements schema v16.

| Retained external evidence | SHA-256 | Result |
|---|---|---|
| Source inventory | `27937e9a4d2eed8c57147931d32c88905057f407b2e29882f31086135536c068` | 72 FBXs; source unchanged |
| Exhaustive baseline | `70990cdf38a3f2d9f1a28caba319bc8ac32c2d8c620fafdf15c8869dfa619788` | 72/72 complete |
| Declared contracts | `aea4184b500dd0825369e656b9e56aecedd038187bdee966af91cc2f39c08c83` | 70 files; 28 pass / 42 fail |
| Remediation | `1f90528096026ed186a467395f1205f739ea93a7d41858183f8675bccbebb51c` | 14 gait candidates plus one pruning candidate completed |
| 0.7 supplemental projections | `94a88103ee12883ba07dc2d4c5e5f5804ec24f53e50bfe5ffdfdfef9f50cc14a` | 15 addressability V1 + rich V2 pairs; exact-profile advice available |

The new projections do not evaluate injury-style blending, runtime target survival, retarget deformation, or visual acceptance.

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

- Local authorized source archive, extracted Unity metadata, and bundled animation list — private evidence identified above, accessed 2026-08-17 and re-verified byte-identical 2026-08-21.
- Protofactor, [Animset: Injured](https://protofactor.biz/product/animset-injured/), [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/), and [EULA](https://protofactor.biz/end-user-license-agreement/) — current context, accessed 2026-08-17.
- Unity Asset Store, [Ultimate Animation Collection](https://marketplace.unity.com/packages/3d/animations/ultimate-animation-collection-195459) — current collection version 1.65, release date 2026-08-16, Single Entity listing, and original Unity 6000.5.1; not local constituent identity.
- Unity, [Animation Blend Trees](https://docs.unity3d.com/6000.5/Documentation/Manual/class-BlendTree.html), [Root Motion](https://docs.unity3d.com/6000.5/Documentation/Manual/RootMotion.html), and [looping clips](https://docs.unity3d.com/6000.5/Documentation/Manual/LoopingAnimationClips.html); Epic Games, [animation system](https://dev.epicgames.com/documentation/unreal-engine/skeletal-mesh-animation-system-in-unreal-engine?application_version=5.7); Godot, [AnimationTree](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html); Bevy, [Animation Masks](https://bevy.org/examples/animation/animation-masks/) — capability context only, accessed 2026-08-17.
- AnimSmith issue tracker, verified live 2026-08-21. Open: [#401](https://github.com/mmannerm/animsmith/issues/401) (constant-track pruning equivalence) and [#411](https://github.com/mmannerm/animsmith/issues/411) (declared-set speed/stride coherence). Closed and delivered in this release: [#407](https://github.com/mmannerm/animsmith/issues/407) (2026-08-17, fail-closed gait-anchor safety policy — the source of the 0.3.0 refusal), [#426](https://github.com/mmannerm/animsmith/issues/426) (2026-08-18, gait-anchor support for in-place rigs with a vertical root forward axis — the source of the 0.4.0 14/14 result), [#402](https://github.com/mmannerm/animsmith/issues/402) (2026-08-20, per-(bone, property) channel coverage), and [#408](https://github.com/mmannerm/animsmith/issues/408) (2026-08-20, root displacement/accumulated-yaw measurement).
