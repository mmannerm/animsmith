# Animation pack evidence appendix: Protofactor Injured

> Companion report: [Protofactor Injured](protofactor-injured.md)
>
> Evidence status: **partial** — exact AnimSmith 0.7.0 baseline, contracts, remediation verification, addressability, and bounded advice plus a dated Unity 6000.5.8f1 observation; visual loops, blends, masks, target-character, and engine-editor/runtime passes remain absent.
>
> Evaluation date: **2026-08-26**
>
> Current evaluator: **AnimSmith 0.7.0**
>
> Report format: **2**

This appendix uses the [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder) without redefining it.

## Evaluation scope and provenance

| Field | Value |
|---|---|
| Pack/edition | Protofactor Injured constituent from a locally held Ultimate Animation Collection archive; local revision unknown |
| Vendor/source | Protofactor [Injured product](https://protofactor.biz/product/animset-injured/) and [collection](https://protofactor.biz/product/ultimate-animation-collection/) pages |
| Delivered scope | RAR to one Unitypackage to 171 logical files; 72 FBXs: 70 individual motions, one combined take, one actor |
| Target use | Game-engine injured locomotion/posture states combined with Basic Locomotion, Sword & Shield, and contextual packs |
| Target engines | Dated Unity 6000.5.8f1 observation; current Unity Humanoid revision-1, Unreal revision-2, and Godot revision-2 settings projections; Bevy revision-3 rich addressability |
| Target rigs/packs | Supplied Protof-Actor; Basic Locomotion, Sword & Shield, Campfire, and Climbing selective compatibility |
| Source manifest | `injured/source-archive-inventory.json`; RAR SHA-256 `b459ab3a39a15aa2e499c633f661616449bfc281836858ad9525014184aa9878` |
| Evaluation manifest | `injured/evidence/evaluation-manifest.json`; SHA-256 `ad98ac7639c997a6d7a3eabb7552b2bbb06ab1c797013cf84cb86e764a3159f5`; schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1`; taxonomy/profile-set version 1 |
| Acquisition/license provenance | User states the local archive was downloaded from Protofactor; current [vendor EULA](https://protofactor.biz/end-user-license-agreement/) reviewed 2026-08-17 (not re-reviewed this refresh). No receipt or local revision record was evaluated; no legal opinion. |

### Evidence coverage

| Surface | Offered/delivered | Evaluated | Findings | Not evaluated and why |
|---|---:|---:|---:|---|
| Animation files | 70 individual plus 1 combined | 71 | 42 individual contract failures | Dynamic visual quality and combined segmentation |
| Rigs/export variants | 2 observed structures | 2 | Standard 56; combined/actor 58 | Target-character deformation |
| AnimSmith baseline | 72 FBXs | 72 | 9,915 constant-track notes; baseline commands complete | Default lint lacks gait intent |
| Declared contracts | 70 individual files | 70 | 28 clean; 42 failing; 9,644 notes (unchanged); loop-seam applicability 48/22, evaluation 25/45 complete (current) | Visual loop/blend/transition quality |
| Gait measurements | 14 IP/RM pairs | 14 | Matching pair phase/duration; style speeds/phase vary; heading axis `positive_y` on 71/72 clips (current) | Contact-side/stride semantics |
| Gait-anchor candidates | 14 IP gait files | 14 attempted | All 14 anchor at exit 0; circular spread falls 0.554→0.110 for run and 0.603→0.051 for walk | Unpromoted: no Humanoid-retarget engine import; all 14 load only as Generic clips in the separate GLB test |
| Engine import/playback | 70 individual clips | 70 imported; 6 sampled | Required samples pass; current `unity-humanoid`/`unreal`/`godot`/`bevy` advice projections are listed below | Controller, compression, player build |
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
| Root motion | `evaluated-finding` | 14 IP/RM pairs measured; heading axis resolves to `positive_y` on 71/72 clips; all 14 current gait-anchor attempts succeed and remain unpromoted. Root displacement/yaw is measured on all 72 clips. |
| Conform | `partially-evaluated` | Standard skeleton and Unity shared Avatar work; target rigs open. |
| Validate | `partially-evaluated` | Mechanical contracts and headless Unity complete; visual gameplay open. |
| Optimize | `evaluated-finding` | One pruning candidate verified mechanically but not accepted semantically. |
| Export | `partially-evaluated` | Sample GLB export reopens; Unity native delivery tested; a separate new-project GLB import test confirms all 14 gait candidates load as Generic clips (see Engine procedures and evidence); Humanoid retarget and other engines remain open. |
| Gate/report | `evaluated-clean` | Manifest and parser-validated report pair retained. |

### Readiness evidence by clip set

| Role or runtime set | File-ready / clip-ready | Set-ready / rig-use | Runtime / acceptance boundary |
|---|---|---|---|
| Seven locomotion styles | 14 IP/RM pairs readable and duration/phase paired | Standard skeleton; thresholds differ; all explicit loops fail | Unity samples/mixers execute; current gait-anchor candidates unpromoted (no Humanoid-retarget engine import this session; all 14 loaded as Generic clips in the new-project GLB import test); gait blends and wraps open |
| Standing idles | Seven files readable | 5/7 strict seam failures | Mask graph executes; full-body/upper-body visual choice open |
| Sit/kneel chains | 35 posture/transition files readable | Style letters coherent; kneel exits absent | Unity samples execute; endpoints/recovery/cancellation open |

### Validation-profile status

| Validation profile | Selection | Result / next evidence |
|---|---|---|
| Marketplace intake | `selected` — `vendor-intended` | Archive/listing/EULA reviewed; listing and local RM counts disagree. |
| Blended locomotion | `selected` — `vendor-intended` | Seven speed sets measured; phase/threshold visual gates open. |
| Root-motion controller | `selected` — `observed-pack-capability` | 14 named pairs; current gait anchoring succeeds but candidates remain unpromoted. Root trajectory is sampled on 72/72 clips (15 move >1 cm; 56 stationary; 0 with yaw travel >1°); it is not continuous-curve or engine-extraction proof and declares no movement-ownership axis. |
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
| Constant tracks | 9,915 notes in 72 files; 9,644 in 70 individuals | Export bloat and retarget evaluation cost | Current baseline/contract summaries and per-(bone, property) coverage |
| Declared loop closure | 15/48 loop files | Position discontinuity at wrap | Contract summary |
| Declared loop rotation seam | 42/48 | Angular pulse at wrap | Contract summary |
| Declared loop velocity seam | 31/48 | Velocity pulse at wrap | Contract summary |
| Contract result | 42/70 files fail | Every explicit locomotion loop and 15 idles affected; counts unchanged from the current baseline | Per-file JSON/Markdown agreement |
| Loop-seam applicability (current) | 48/70 applicable; 22/70 not applicable | Separates no-stride clips (mostly idles) from locomotion, ahead of the raw pass/fail split above | `contract.loop_seam_applicability` |
| Loop-seam evaluation completeness (current) | 25/70 complete; 45/70 not evaluated | No-stride clips account for most not-evaluated results; the 42-file findings count applies to the evaluated scope | `contract.loop_seam_evaluation` |

## AnimSmith remediation evidence

| Source issue | Operation/declarations | Result | Independent verification | Remaining caveat |
|---|---|---|---|---|
| Gait phase normalization | Current `transform --gait-anchor` on 14 IP walk/run files with heading axis `positive_y` | All 14 anchor at exit 0. Circular spread: Run 0.554071→0.109841; Walk 0.602542→0.051348 | Per-file checks complete; sampled root trajectory shows 0 clips with yaw travel >1° | All 14 load as Generic clips, but Humanoid-retarget, visual, and gameplay acceptance remain open. |
| Constant tracks in Walk A | `transform --prune-constant-tracks` with declared contract | Exit 0; FBX 658,816 bytes to GLB 45,152 bytes | Output inspect/measure and fix dry-run exit 0; diff detects change | Lint retains source seam; equivalence unproven, so output not adopted; still bounded by [issue #401](https://github.com/mmannerm/animsmith/issues/401). |

## Engine procedures and evidence

| Runtime | Version | Procedure | Observed result | Remaining gate |
|---|---|---|---|---|
| Unity | 6000.5.8f1 (retained, 2026-08-17) | Merge five authorized package reconstructions outside repo; inventory; sample six Injured clips; mix Basic and Sword into Injured; build one Basic-lower/Injured-upper mask graph. | 70/70 individual Humanoid clips import; 6/6 samples, both mixers, and mask graph pass. Byte-identical source justifies keeping this dated evidence current. | Visual loops/blends/mask, controller, target rig, compression, build. |
| Unity | Dated Unity 6000.5.8f1 headless import/Playables observation plus current Unity Humanoid revision-1 projection | Import and representative graph execution succeeded for the delivered FBXs; current settings projection is available | Visual playback, target retarget, contacts, full graphs, compression, and build remain open |
| Unity GLB import | 6000.5.8f1 with glTFast 6.9.0 | Load current candidates in a disposable project | Every tested candidate produces one Generic clip | Humanoid retarget, playback, and visual acceptance remain open |
| Unreal Engine | 5.8, `unreal` revision 2 | Current settings projection | Available; no engine process ran | FBX import/retarget, blend spaces, markers, root authority, masks, and build. |
| Godot | 4.7, `godot` revision 2 | Current settings projection | Available; no engine process ran | Conversion/import, retarget, phase policy, filters, and export. |
| Bevy | 0.19.0, `bevy` revision 3 (current evaluation; Unity observation 2026-08-21) | `generate addressability` on one generated GLB candidate from this session. | Exit 0: 1 animation row, coverage complete, predicted selector `Animation0`, facet available, 0 findings. | glTF conversion of the delivered source, mapping, phase policy, masks, profiling; this is inventory/selector prediction only, not an import or playback test. |

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
4. Current Unreal revision-2 and Godot revision-2 settings projections are available, but neither engine received an import or playback test.
5. Commercial files, derived motion outputs, screenshots, and the generated Unity project remain outside the repository and CI.
6. Root trajectory on 72/72 clips (15 move >1 cm; 0 with yaw travel >1°) is sampled-grid evidence, not continuous-curve or engine-extraction proof, and does not declare movement ownership.
7. The corrected Unity root-lock observation (Engine procedures and evidence) is a 120-clip cross-pack sample, not exhaustive per pack; the 15 sampled Injured clips are all in-place idle/posture files, so this pack's own 14 `_RM` gait files were not directly observed, though the sampled pattern is consistent with the aggregate in-place/root-motion split.
8. The new-project GLB import test (134/134 candidates, 14 from this pack) proves glTFast produces one well-formed Generic AnimationClip per candidate; it does not test this pack's Humanoid retarget path and is not visual or gameplay acceptance.

## Changes between AnimSmith versions

| Evaluator | Change from the preceding evaluated state |
|---|---|
| AnimSmith 0.7.0 | Revalidated the 72-FBX baseline, 70 declared contracts, 14 gait candidates, pruning trial, and current engine projections under output v17 / measurements v16. Current loop applicability no longer treats no-stride clips as failed work. |
| AnimSmith 0.4.1 | Reproduced the evaluated 0.4.0 measurements and transforms for this corpus; unrelated release fixes did not change the pack conclusion. |
| AnimSmith 0.4.0 | Added root-trajectory, channel-coverage, and profile evidence and produced the gait candidates now retained only as unpromoted mechanical evidence. |
| AnimSmith 0.3.0 | Established the initial baseline, contract, and dated Unity evidence. Those evaluator results are superseded. |

## Reproduction

### Current AnimSmith reproduction (2026-08-26)

The unchanged corpus was rerun with `animsmith 0.7.0`, exact tag `v0.7.0`, commit `461ac8a4f6bb368eb8637471a796f13eeb647140`, binary SHA-256 `01a501999c91d93abfb32b1f48241fccc70914fac27c9a650c31df44262578d8`, output schema v17, and measurements schema v16.

| Current external evidence | SHA-256 | Result |
|---|---|---|
| Source inventory | `27937e9a4d2eed8c57147931d32c88905057f407b2e29882f31086135536c068` | 72 FBXs; source unchanged |
| Exhaustive baseline | `70990cdf38a3f2d9f1a28caba319bc8ac32c2d8c620fafdf15c8869dfa619788` | 72/72 complete |
| Declared contracts | `aea4184b500dd0825369e656b9e56aecedd038187bdee966af91cc2f39c08c83` | 70 files; 28 pass / 42 fail |
| Remediation | `1f90528096026ed186a467395f1205f739ea93a7d41858183f8675bccbebb51c` | 14 gait candidates plus one pruning candidate completed |
| 0.7 supplemental projections | `94a88103ee12883ba07dc2d4c5e5f5804ec24f53e50bfe5ffdfdfef9f50cc14a` | 15 addressability V1 + rich V2 pairs; exact-profile advice available |

The current projections do not evaluate injury-style blending, runtime target survival, retarget deformation, or visual acceptance.

## Sources

- Local authorized source archive, extracted Unity metadata, and bundled animation list — private evidence identified above, accessed 2026-08-17 and re-verified byte-identical 2026-08-21.
- Protofactor, [Animset: Injured](https://protofactor.biz/product/animset-injured/), [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/), and [EULA](https://protofactor.biz/end-user-license-agreement/) — current context, accessed 2026-08-17.
- Unity Asset Store, [Ultimate Animation Collection](https://marketplace.unity.com/packages/3d/animations/ultimate-animation-collection-195459) — current collection version 1.65, release date 2026-08-16, Single Entity listing, and original Unity 6000.5.1; not local constituent identity.
- Unity, [Animation Blend Trees](https://docs.unity3d.com/6000.5/Documentation/Manual/class-BlendTree.html), [Root Motion](https://docs.unity3d.com/6000.5/Documentation/Manual/RootMotion.html), and [looping clips](https://docs.unity3d.com/6000.5/Documentation/Manual/LoopingAnimationClips.html); Epic Games, [animation system](https://dev.epicgames.com/documentation/unreal-engine/skeletal-mesh-animation-system-in-unreal-engine?application_version=5.7); Godot, [AnimationTree](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html); Bevy, [Animation Masks](https://bevy.org/examples/animation/animation-masks/) — capability context only, accessed 2026-08-17.
