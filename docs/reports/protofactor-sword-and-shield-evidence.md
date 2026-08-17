# Animation pack evidence appendix: Protofactor Sword & Shield Animset

> Companion report: [technical evaluation](protofactor-sword-and-shield.md)
>
> Evidence status: **partial** — exhaustive file/AnimSmith coverage and a Unity 6000.5.8f1 combined-project probe; visual acceptance and three engines remain unevaluated.
>
> Evaluation date: **2026-08-17**
>
> Report format: **1**

This appendix preserves the detailed evidence behind the concise report. The [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder) remains authoritative.

## Evaluation scope and provenance

| Field | Value |
|---|---|
| Pack/edition | Local `Animset@Sword&Shield_PACKAGE.unitypackage`; constituent revision is not declared |
| Vendor/source | Protofactor; [current Sword & Shield product page](https://protofactor.biz/product/animset-sword-shield/) |
| Delivered scope | Full local RAR → one Unitypackage → 136 FBXs: 132 individual motions, one combined animation FBX, one skinned actor, sword prop, and shield prop; Unity metadata/materials/textures included |
| Target use | Game-engine use; generic third-person sword-and-shield controller and combined use with Basic Locomotion |
| Target engines | Unity 6000.5.8f1 observed; Unreal Engine, Godot, and Bevy documentation-only |
| Target rigs/packs | Supplied Protof-Actor and the separately evaluated Protofactor Basic Locomotion pack |
| Source manifest | `evidence/logical-assets-inventory.json`; SHA-256 `a5f52b3e12bab1a4859c31e7e3b7223a806ec48eed8abd406366494ef6c111a6` |
| Evaluation manifest | `evidence/evaluation-manifest.json`; SHA-256 `b9a5317dcd0ed0a4d46e3c9144cbfa3430ab473354cdf9901c796b8875287d02`; taxonomy/profile-set version 1 |
| Acquisition/license provenance | `user-stated`: local archive was downloaded from Protofactor.biz as part of the Ultimate Animation Collection. Current EULA permits protected released real-time applications and modification while restricting redistribution/resale; no receipt, download date, historical EULA, or local constituent revision was retained. Technical due diligence only, not legal advice. |

The current product page, observed 2026-08-17, advertises USD 24.99, 132 animations, 45 root-motion and 87 in-place files, Unity Humanoid, Epic-skeleton scale/retarget intent, Unity 2019.4+, and no native UE4 package. The local individual-file counts match 132/45/87, but the page does not prove the local revision or purchase terms. The collection page is collection context, not constituent identity.

The evaluation manifest uses schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1`.

### Evidence coverage

| Surface | Offered/delivered | Evaluated | Findings | Not evaluated and why |
|---|---:|---:|---:|---|
| Animation files | 134 animation-bearing FBXs | 134 | 133 readable humanoid/combined motions; one two-node malformed motion | Continuous artistic review of all motion |
| Rigs/export variants | 4 observed skeleton structures | 4 | 131 individual motions share 56 bones; malformed 2; combined 60; actor 58 | Target-character deformation and non-Unity retarget |
| AnimSmith baseline | 136 FBXs | 136 | 134 animated inputs completed; 17,078 constant-track notes; one combined scale-key warning | Props have no animation clips |
| Declared contracts | 132 individual motion files | 132 | 17 pass, 115 fail under delivered/inferred declarations | Human author intent for every loop/action |
| Offline visual reports | 132 possible | 8 representative | Seven coherent frame-zero skeletons; malformed file shows only root | Motion, contacts, loop wrap, deformation |
| Engine import/playback | 133 Sword motion FBXs in Unity | 133 | 132 clips total; 131 humanMotion; malformed file has no clip; 8/9 representative samples passed | Visual playback, compression, player build |
| Blend/mask/retarget | 8 composition/attachment probes | 8 | 3/3 blends, 3/3 masks, and 2/2 attachments pass | Visual masks, target rig, IK/contact accuracy |

### Claim legend

Evidence labels are `user-stated`, `vendor-stated`, `observed-file`, `observed-animsmith`, `observed-report`, `observed-engine`, `documentation-stated`, `inferred`, and `not-evaluated`, as defined by the versioned assessment taxonomy.

## Evaluation manifest and taxonomy

### Canonical clip-role inventory

| Canonical primary role | Logical motions | Delivered files | Evidence boundary |
|---|---:|---:|---|
| `idle-pose` | 6 | 6 | Combat/crouch idle and ready/look/breathe states. |
| `continuous-locomotion` | 28 | 56 | Walk, run, crouch directions and normal/fast candidates; 28 IP/RM pairs, one RM malformed. |
| `locomotion-transition` | 0 | 0 | General locomotion transitions absent. |
| `airborne` | 0 | 0 | Jump/fall/landing absent. |
| `traversal` | 0 | 0 | Environment-aligned traversal absent. |
| `action-interaction` | 30 | 43 | Attacks/combos, defense, and equipment transitions; 13 action IP/RM pairs. |
| `reaction-death` | 20 | 24 | Four heavy-hit IP/RM pairs plus light hits and death/downed/recovery singles. |
| `emote-cinematic` | 3 | 3 | Three taunts. |
| `other-unknown` | 0 | 0 | No individual motion remained unclassified. |
| **Total** | **87** | **132** | Validated v1 manifest: 45 paired motions and 42 singles. |

### Runtime-set inventory

Exact locomotion members, durations, speeds, coordinates, and contracts are retained in the companion [runtime-set table](protofactor-sword-and-shield.md#runtime-sets-and-authored-motion).

| Runtime set | Type | Members/variants | Grouping evidence | Validation status |
|---|---|---|---|---|
| Walk combat 8-way | directional-blend | Eight paired IP/RM directions | Common duration, signature, names, measured speed/phase; high grouping confidence | Complete measurements; raw phase/loop findings; Unity representative only. |
| Run combat 8-way | directional-blend | Eight paired IP/RM directions | Common duration, signature, names, measured speed/phase; high grouping confidence | Complete measurements; raw phase/loop findings; Unity representative only. |
| Crouch combat 8-way | directional-blend | Eight IP and eight labeled RM directions | Common duration/names; low confidence because FR RM has only two nodes | Seven valid RM directions; FR RM quarantined. |
| Draw/combat/put-away 1 | transition-chain | Draw1 → IdleCombat → PutBack1 | Vendor names and file observation; high grouping confidence | Members import; transition/events/visual crossfades untested. |
| Draw/combat/put-away 2 | transition-chain | Draw2 → IdleCombat → PutBack2 | Vendor names and file observation; high grouping confidence | Members import; transition/events/visual crossfades untested. |
| Walk normal 1/2 | speed-blend | Two IP and two RM candidates | Naming only; medium confidence | Candidate; speed topology/visual result not tested. |
| Run normal/fast | speed-blend | Two IP and two RM candidates | Naming and measured motion; medium confidence | Candidate; thresholds/visual result not tested. |
| Death/downed/recover, four directions | transition-chain | Death → Dead hold → GetBackUp | Directional names and loop metadata; high grouping confidence | Members import; events, interruption, and recovery pose continuity untested. |

### Pipeline-stage coverage

| Stage | Coverage state | Evidence / remaining gate |
|---|---|---|
| Acquire | `partially-evaluated` | Archive identity and current vendor/EULA pages captured; historical transaction/revision unavailable. |
| Preserve raw | `evaluated-clean` | RAR SHA and lossless Unitypackage/logical inventories retained; generated outputs separate. |
| Inspect | `evaluated-finding` | All 136 FBXs inspected; one advertised motion has a two-node hierarchy. |
| Segment | `partially-evaluated` | 132 individual files are usable boundaries; combined FBX not authoritatively segmented. |
| Root motion | `evaluated-finding` | Forty-five labeled variants measured; malformed member and action displacement/yaw intent remain. |
| Conform | `partially-evaluated` | Unity Humanoid path succeeds for 131 individual clips; other engines/target rigs absent. |
| Validate | `evaluated-finding` | Exhaustive contracts, set measurements, Unity combined probe, and limited static reports. |
| Optimize | `partially-evaluated` | Duplicate-endpoint and prune trials only; no output promoted. |
| Export | `partially-evaluated` | Sample GLBs emitted for verification, not accepted engine assets. |
| Gate/report | `partially-evaluated` | Unity headless gates and report pair complete; visual and other engines open. |

### Readiness evidence by clip set

| Role or runtime set | File-ready / clip-ready | Set-ready / rig-use | Runtime / acceptance boundary |
|---|---|---|---|
| Combat locomotion (56 files) | 55 normal humanoid files; Crouch FR RM quarantined; delivered loops have strict seam findings. | Common signature/timing except quarantine; phase spreads 0.726–0.807; 24 anchors refused. | Unity representative graphs only; complete visual rings/root ownership open. |
| Actions/defense/equipment (43) | Files readable; 39 are delivered as loops although most are one-shots; 13 RM pairs. | Props supplied; no hit events, contacts, IK, or additive contract. | Unity samples/masks/attachments execute; visual/contact gameplay open. |
| Reactions/death (24) | Files readable; four heavy pairs; downed holds and recovery singles. | Four inferred transition chains; no pose/contact continuity gate. | State timing, interruption, ragdoll handoff, visual result open. |
| Idles/taunts (9) | Files readable; delivered loop metadata present. | Same standard signature; no sync or mask contract. | Unity idle sampled; loop/taunt visual acceptance open. |
| Combined FBX | Readable but 60-bone signature and one animated scale track. | Segmentation not authoritative. | Do not use as the runtime source. |

### Validation-profile status

| Validation profile | Selection | Result / next evidence |
|---|---|---|
| Marketplace intake | `selected` — `user-required` | File/tool intake complete; historical provenance partial. |
| Blended locomotion | `selected` — `observed-pack-capability` | Three directional families measured; phase and quarantine findings; full visual graphs open. |
| Root-motion controller | `selected` — `observed-pack-capability` | Forty-five labels measured; action yaw/ownership and full runtime extraction open. |
| State-machine transitions | `selected` — `observed-pack-capability` | Equipment/death chains inferred; events, interruptions, and visual crossfades open. |
| Layered upper body/weapons | `selected` — `user-required` | Three Unity mask graphs run; pelvis, contact, kick, IK, and grip acceptance open. |
| Traversal/environment | `not-applicable` | No advertised environment-aligned traversal family. |
| Contact actions/interactions | `selected` — `observed-pack-capability` | Weapon/shield/kick content exists; contact timing and target interaction open. |
| Retargeted/customizable characters | `selected` — `vendor-intended` | Supplied Unity Avatar works for 131 clips; project-character and non-Unity retarget open. |
| Motion matching/search | `not-selected` | No target database contract. |
| Networked movement | `not-selected` | No authority/prediction/rollback contract. |
| Runtime performance | `selected` — `evaluator-selected-generic-scenario` | Constant tracks measured; equivalence, memory, CPU, and build size open. |

## Pack inventory and content evidence

The immutable source is `<authorized-local-source>/Animset@Sword&Shield_ASSET.rar`, 174,727,929 bytes, SHA-256 `4402f20ba681ec83cf01f60b8dfb69b59435b48408030a5fbb4f3454f64840d7`. It contains `Animset@Sword&Shield_PACKAGE.unitypackage`, 180,663,737 bytes, SHA-256 `dde20f36bfceca17370fcb511b238c9b0598de7657b214c6d67188939bd5dcf7`.

The reconstructed Unity delivery has 313 regular files: 136 FBXs and 162 metadata files plus materials, textures, and the vendor list. The motion directory has 132 individual files plus `Protof-Actor@Sword&ShieldAnimset.fbx`. The other FBXs are the actor, sword, and shield. The vendor list and current listing both reconcile to 132 individual files: 45 `_RM` and 87 non-RM.

Every normal individual file exposes embedded clip `Take 001`; meaningful identity comes from case-sensitive filenames and Unity metadata. The vendor list contains non-authoritative spellings/casing such as `RunFrowardRight`, `ParryHight2`, `swordAttack2`, and `3hitCombo1`; exact report members use delivered filenames.

Unity metadata defines 133 clip entries: 118 loop true and 15 false including the actor. Fifty-two loop-true individual files are obvious one-shot-like candidates by role: 27 attacks, 10 defense actions, 12 reactions, and three taunts. Of the 132 individual motions, all except the malformed RM file produce AnimationClips. Stored source-avatar warnings appear in 131 metadata files, but Unity resolves and imports them; only the malformed file emits an active hierarchy error during the tested import.

## Mechanical baseline

| Finding/check | Affected scope | Potential impact | Evidence |
|---|---|---|---|
| Default inspect/measure/lint completed | 134 animation-bearing FBXs | Establishes readable baseline, not gameplay readiness. | `observed-animsmith`; all commands exit 0 under empty config. |
| Malformed Crouch FR RM hierarchy | 1/132 individual files | No humanoid clip; missing directional RM member. | Two bones, 27,680 bytes; blank skeleton report; Unity no AnimationClip. |
| Standard skeleton signature | 131/132 individual files | Strong within-pack interchange prerequisite. | 56 bones, signature `8ea3a291222d`. |
| Constant tracks | 17,078 baseline notes; 16,808 contract notes | Size/runtime opportunity but unsafe to prune without semantics. | Complete AnimSmith aggregation. |
| Scale keys | Combined FBX only | Unexpected scale animation can complicate retarget/composition. | Bone `SM_1HandedSwordPropIdle2`. |
| Delivered loop contracts | 118 individual files applicable; 113 seam-velocity and 113 seam-rotation failures; 55 closure findings in 48 files | Can pop/pulse when truly cyclic; many results instead expose wrong one-shot declarations. | Contract lint derived from delivered Unity loopTime. |
| Root-motion threshold | 14 labeled action RM files below 0.5 m/s | Translation-only classification may reject short/yaw actions incorrectly. | Measured speed; yaw intent not established. |

## AnimSmith remediation evidence

| Source issue | Operation/declarations | Result | Independent verification | Remaining caveat |
|---|---|---|---|---|
| Raw locomotion phase | `transform --gait-anchor` on 24 IP ring members | 0/24 outputs; safe exit 2 because root has no finite horizontal forward axis at sample 0 | Command records confirm no output files | Use runtime offsets or artist exports; [#426](https://github.com/mmannerm/animsmith/issues/426). |
| WalkForward duplicate endpoint | `transform --drop-duplicate-loop-endpoint` | Output emitted; endpoint closure removed | Post-lint no longer reports loop-closure | Linear and angular seam-derivative errors remain. |
| Dense constant tracks | `transform --prune-constant-tracks` on WalkForward, SwordAttack1, and combined FBX | Three outputs emitted; constant notes removed | Source/output `diff` reports many deltas and format/node differences | Semantic/runtime equivalence not proven; do not promote. |
| Combined scale animation | Current diagnostics only | Warning remains | Post-lint retains scale-key finding | Artist/vendor must establish and repair intent. |
| Malformed RM hierarchy | No safe current operation | No output attempted | Unity and offline report independently confirm absence | Hierarchy/animation must come from artist/vendor evidence. |

No gait transform was applied to accumulating root translation or yaw. AnimSmith 0.3.0's refusal is the correct safety result under the unmeasurable basis; cyclic resampling must not reorder root trajectory without independent displacement and yaw proof.

## Engine procedures and evidence

| Runtime | Version | Procedure | Observed result | Remaining gate |
|---|---|---|---|---|
| Unity | 6000.5.8f1 | Create disposable project; import Sword package, then Basic package; inventory importers/clips; sample representative Playables; mix full-body pairs; apply Humanoid upper-body masks; attach props at hand local identity. | Both imports exit 0; 131/132 Sword human clips; 8/9 samples, 3/3 blends, 3/3 masks, 2/2 attachments pass; only quarantined file fails. | Visual controller, contacts, root motion, target rig, compression, player build. |
| Unreal Engine | unspecified | Documentation review for Root Motion, Blend Spaces/Sync Groups, Blend Masks, and layered animations. | Capability documented; pack not imported; vendor states Epic-scaled but not Epic-rigged and supplies no UE4 files. | Import/retarget/graphs/contact/build. |
| Godot | stable | Documentation review for AnimationTree BlendSpace2D, filters, OneShot, sync, and root-motion API. | Capability documented; pack not imported. | Conversion/import, skeleton mapping, graphs, root, contacts, export. |
| Bevy | unspecified | Documentation review for AnimationGraph masks; inspect current retarget limitation. | Layer masks documented; FBX route and mature retarget path not established. | Convert to glTF, retarget, graph, root motion, performance/build. |

## Rig, masking, and compatibility evidence

| Pack/rig/set pair | Skeleton/retarget | Scale/axes | Root policy | Timing/blend | Overall evidence |
|---|---|---|---|---|---|
| Sword standard clips ↔ supplied actor | 131 files share 56-bone signature; Unity source Avatar valid | Unity local-identity prop height ratios: sword 0.429, shield 0.376 of actor | IP/RM naming present; action yaw incomplete | Representative playback/masks pass | Direct Unity candidate; visual deformation/grips untested. |
| Sword locomotion IP ↔ RM | 27 valid same-signature pairs plus one malformed RM pair | Common import scale | Controller vs animation ownership required | Durations match; phase/speed vary | Conditional; quarantine one member and configure per direction. |
| Sword ↔ Basic Locomotion | Standard 56-bone signature identical; shared actor signature identical | 25 overlapping relative files are byte-identical | Both use IP/`_RM` convention; yaw still unproved | Combined Unity project: 2 cross-pack full-body blends and 3 cross-pack masks pass; 1 Sword-internal blend also passes | Strong technical co-install/graph candidate; style/contact/visual transition untested. |
| Sword action ↔ Basic locomotion mask | Unity Humanoid mapping accepted | Props attach; orientation/contact unreviewed | Basic base owns movement; action root/pelvis must be excluded or reviewed | Three headless mask graphs pass | Prototype candidate only; kicks and displacement-bearing attacks remain full-body. |
| Pack ↔ project character | No target character | Not evaluated | Project policy unknown | Not evaluated | Unknown. |

The shared-path comparison found 25 overlaps and zero conflicts; every overlap is byte-identical. This includes the actor, materials, textures, and relevant metadata. Compatibility does not follow from names alone: the exact digests, skeleton signatures, and combined Unity probe provide the evidence.

## Limitations and unknowns

1. No project character, camera, controller, quality bar, platform, networking policy, combat design, or hit-window specification was supplied.
2. Headless Unity graph evaluation proves import and execution, not motion quality, foot planting, deformation, mask seams, prop orientation, weapon/shield contacts, or perceived timing.
3. Unreal Engine, Godot, and Bevy remain documentation-only; Bevy also needs an FBX-to-glTF and retarget path.
4. Delivered loop metadata is not reliable author intent for one-shots; the strict failure count is not a count of 113 visibly bad gameplay cycles.
5. `_RM` is vendor/naming evidence. Speed does not characterize yaw-only or short-displacement root semantics.
6. AnimSmith reports were inspected at frame zero only; no visual motion acceptance was performed.
7. Pruned GLBs are experimental and not production candidates because semantic equivalence was not established.
8. Current public pages and EULA do not prove the local artifact's revision, transaction date, or historical terms.
9. Only Basic Locomotion was tested cross-pack; the rest of the Ultimate Animation Collection is outside this evaluation.

## Reproduction

Source identity: RAR SHA-256 `4402f20ba681ec83cf01f60b8dfb69b59435b48408030a5fbb4f3454f64840d7`; Unitypackage SHA-256 `dde20f36bfceca17370fcb511b238c9b0598de7657b214c6d67188939bd5dcf7`. Evaluator: `animsmith 0.3.0 (v0.3.0-23-gc11f135)`, repository revision `c11f135ece5e980e6c98861a52a715a28a424ff9`, binary SHA-256 `2fbf038dab62e380f15d709fbed8be58bbec5d9c06a3dfd02a7adec2eba619b2`.

```text
# Exhaustive baseline: 136 FBXs; inspect/measure/lint JSON and Markdown
animsmith inspect --config config/baseline.animsmith.toml <input.fbx>
animsmith measure --config config/baseline.animsmith.toml --format json <input.fbx>
animsmith lint --config config/baseline.animsmith.toml --format json <input.fbx>

# Per-motion contracts: 132 files; 17 exit 0, 115 exit 1
animsmith lint --config config/contracts/<file>.animsmith.toml --format json <input.fbx>

# Bounded remediation, followed by inspect/measure/lint/diff
animsmith transform --config <config> <input.fbx> -o <output.glb> --gait-anchor
animsmith transform --config <config> <input.fbx> -o <output.glb> --drop-duplicate-loop-endpoint
animsmith transform --config <config> <input.fbx> -o <output.glb> --prune-constant-tracks
```

Retained portable artifacts include: baseline summary SHA-256 `5aec24f63aad108179406ced3a8df42055d55961f530124f6487d6835b1dc3b1`; contract summary `a62c87b94e11a84ba238420b4b3f0462ca8e14e004b703b7ebb2a528afe74701`; clip catalog `a980252db9eb48dbddeae27ca150820a3f02c3cf9f25b4e2d3489488c659a60c`; remediation record `65802bf6980ec6105c8a1d254adb4d7183379cb61ba98ca53d1a47ae32fe9438`; combined Unity probe `c4310bedddfd27e06696207e8bb1c4076039126c467ed4964aba067c8524c392`; Basic cross-pack comparison `346e254927a65de26307a5e82da29f70d642c69cea3347b840fb0761e32a4142`.

## Sources

- Local source archive and extracted Unity metadata — private authorized input identified above, accessed 2026-08-17.
- Protofactor, [Animset: Sword & Shield](https://protofactor.biz/product/animset-sword-shield/) — current price, counts, formats, rig/engine statements, accessed 2026-08-17.
- Protofactor, [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/) — current collection context, accessed 2026-08-17.
- Protofactor, [End User License Agreement](https://protofactor.biz/end-user-license-agreement/) — current protected-application, modification, transfer, and redistribution terms; not historical transaction evidence, accessed 2026-08-17.
- Unity, [Avatar Mask](https://docs.unity3d.com/es/current/Manual/class-AvatarMask.html), [imported clip masks](https://docs.unity3d.com/es/current/Manual/AnimationMaskOnImportedClips.html), [Animation Layers](https://docs.unity3d.com/6000.0/Documentation/Manual/AnimationLayers.html), [Root Motion](https://docs.unity3d.com/6000.0/Documentation/Manual/RootMotion.html), and [loop optimization](https://docs.unity3d.com/es/current/Manual/LoopingAnimationClips.html) — runtime capabilities only.
- Epic Games, [Root Motion](https://dev.epicgames.com/documentation/unreal-engine/root-motion-in-unreal-engine?application_version=5.7), [Blend Masks](https://dev.epicgames.com/documentation/unreal-engine/blend-masks-and-blend-profiles-in-unreal-engine?lang=en-US), and [Layered Animations](https://dev.epicgames.com/documentation/unreal-engine/using-layered-animations-in-unreal-engine?lang=en-US) — runtime capabilities only.
- Godot, [Using AnimationTree](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html) — blend, filter, sync, one-shot, and root-motion capabilities only.
- Bevy, [Animation Masks](https://bevy.org/examples/animation/animation-masks/) and [retargeting issue #15612](https://github.com/bevyengine/bevy/issues/15612) — current masking example and retargeting limitation context.
- AnimSmith issues [#401](https://github.com/mmannerm/animsmith/issues/401), [#402](https://github.com/mmannerm/animsmith/issues/402), [#408](https://github.com/mmannerm/animsmith/issues/408), and [#426](https://github.com/mmannerm/animsmith/issues/426) — optimization evidence, root displacement/yaw, and gait-basis follow-up.
