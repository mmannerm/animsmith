# Animation pack evidence appendix: Protofactor Sword & Shield Animset

> Companion report: [technical evaluation](protofactor-sword-and-shield.md)
>
> Evidence status: **partial** — AnimSmith 0.4.0 re-run (baseline, contracts, gait-anchor, remediation) on a byte-identical source, retained 2026-08-17 Unity 6000.5.8f1 combined-project probe, new 0.4.0 engine-advisory checks now corrected by a direct Unity 6000.5.8f1 observation of import-advice root-lock declarations, and a headless Unity glTFast import of all 134 collection-wide gait-anchored GLB candidates (including this pack's 24); visual acceptance, Humanoid retarget of the new candidates, and full non-Unity engine passes remain unevaluated.
>
> Evaluation date: **2026-08-21**
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
| Evaluator refresh | AnimSmith `0.4.0`, tag `v0.4.0`, revision `6b37ad636b198ef8ff47fadbf6a3a51eb1a27c8e`, binary SHA-256 `fd1eee57407aa02db88763d144389a7f5104204c40ddfbb28eb5885ca8cd54c6`, output schema v10 / measurements schema v15, captured 2026-08-21. Re-ran baseline, declared-contract, gait-anchor, and remediation passes on this exact source; a re-inventory reproduced the published manifest exactly (0 added, 0 removed, 0 content changed). |
| Rebuild reproducibility (2026-08-21) | Rebuilding tag `v0.4.0` at the same commit `6b37ad636b198ef8ff47fadbf6a3a51eb1a27c8e` produced a binary with a **different** SHA-256, `1e53013bbe3224557a8783eafeb818f4ef9d74666590cbaa8c18ef48c5b7d6fa`, than the digest recorded above — the build is **not byte-reproducible**. Both builds emit byte-identical import-advice artifacts, verified by `diff`. The Unity headless-probe correction and GLB candidate-import evidence added below (2026-08-21) are therefore attributable to the tag and commit, not to the originally recorded binary digest. |

The current product page, observed 2026-08-17, advertises USD 24.99, 132 animations, 45 root-motion and 87 in-place files, Unity Humanoid, Epic-skeleton scale/retarget intent, Unity 2019.4+, and no native UE4 package. The local individual-file counts match 132/45/87, but the page does not prove the local revision or purchase terms. The collection page is collection context, not constituent identity.

The evaluation manifest uses schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1`.

This appendix's 2026-08-21 evaluation date reflects an AnimSmith `0.4.0` refresh: a 2026-08-21 re-inventory reproduces the published manifest exactly (0 added, 0 removed, 0 content changed), and 0.4.0 re-ran the baseline, declared-contract, gait-anchor, and remediation passes on this byte-identical source. Unity 6000.5.8f1 import/playback/blend/mask evidence is retained unchanged from its original 2026-08-17 capture (justified by the byte-identical source) and is labeled with that date wherever cited; it was not re-captured under 0.4.0.

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
| Combat locomotion (56 files) | 55 normal humanoid files; Crouch FR RM quarantined; delivered loops have strict seam findings. | Common signature/timing except quarantine; 0.4.0 circular phase spreads fall Crouch 0.6974371→0.0524396, Run 0.6605044→0.1372773, Walk 0.7231052→0.0599383; 24/24 anchors succeed at exit 0 (2026-08-21, unpromoted), reversing the 0.3.0 (2026-08-17) 0/24 refusal. | Unity representative graphs only (2026-08-17, retained); complete visual rings/root ownership, and engine review of the anchored candidates, remain open. |
| Actions/defense/equipment (43) | Files readable; 39 are delivered as loops although most are one-shots; 13 RM pairs. | Props supplied; no hit events, contacts, IK, or additive contract. | Unity samples/masks/attachments execute; visual/contact gameplay open. |
| Reactions/death (24) | Files readable; four heavy pairs; downed holds and recovery singles. | Four inferred transition chains; no pose/contact continuity gate. | State timing, interruption, ragdoll handoff, visual result open. |
| Idles/taunts (9) | Files readable; delivered loop metadata present. | Same standard signature; no sync or mask contract. | Unity idle sampled; loop/taunt visual acceptance open. |
| Combined FBX | Readable but 60-bone signature and one animated scale track. | Segmentation not authoritative. | Do not use as the runtime source. |

### Validation-profile status

| Validation profile | Selection | Result / next evidence |
|---|---|---|
| Marketplace intake | `selected` — `user-required` | File/tool intake complete; historical provenance partial. |
| Blended locomotion | `selected` — `observed-pack-capability` | Three directional families measured; quarantine findings; 0.4.0 anchors all 24 IP candidates (unpromoted, no Humanoid-retarget or visual acceptance import); full visual graphs open. |
| Root-motion controller | `selected` — `observed-pack-capability` | Forty-five labels measured; 0.4.0 additionally measures 45/133 clips moving >1 cm and 0/133 with >1° yaw pack-wide (sampled grid facts, not an ownership determination); action yaw/ownership and full runtime extraction open. |
| State-machine transitions | `selected` — `observed-pack-capability` | Equipment/death chains inferred; events, interruptions, and visual crossfades open. |
| Layered upper body/weapons | `selected` — `user-required` | Three Unity mask graphs run; pelvis, contact, kick, IK, and grip acceptance open. |
| Traversal/environment | `not-applicable` | No advertised environment-aligned traversal family. |
| Contact actions/interactions | `selected` — `observed-pack-capability` | Weapon/shield/kick content exists; contact timing and target interaction open. |
| Retargeted/customizable characters | `selected` — `vendor-intended` | Supplied Unity Avatar works for 131 clips; project-character and non-Unity retarget open. |
| Motion matching/search | `not-selected` | No target database contract. |
| Networked movement | `not-selected` | No authority/prediction/rollback contract. |
| Runtime performance | `selected` — `evaluator-selected-generic-scenario` | Constant tracks measured; equivalence, memory, CPU, and build size open. |

## Pack inventory and content evidence

The immutable source is `<authorized-local-source>/Animset@Sword&Shield_ASSET.rar`, 174,727,929 bytes, SHA-256 `4402f20ba681ec83cf01f60b8dfb69b59435b48408030a5fbb4f3454f64840d7`. It contains `Animset@Sword&Shield_PACKAGE.unitypackage`, 180,663,737 bytes, SHA-256 `dde20f36bfceca17370fcb511b238c9b0598de7657b214c6d67188939bd5dcf7`. A 2026-08-21 AnimSmith `0.4.0` re-inventory re-verifies both digests and the retained logical-manifest digest below unchanged, reproducing 136 FBXs with 0 added, 0 removed, and 0 content changed.

The reconstructed Unity delivery has 313 regular files: 136 FBXs and 162 metadata files plus materials, textures, and the vendor list. The motion directory has 132 individual files plus `Protof-Actor@Sword&ShieldAnimset.fbx`. The other FBXs are the actor, sword, and shield. The vendor list and current listing both reconcile to 132 individual files: 45 `_RM` and 87 non-RM.

Every normal individual file exposes embedded clip `Take 001`; meaningful identity comes from case-sensitive filenames and Unity metadata. The vendor list contains non-authoritative spellings/casing such as `RunFrowardRight`, `ParryHight2`<!-- vendor-id -->, `swordAttack2`, and `3hitCombo1`; exact report members use delivered filenames.

Unity metadata defines 133 clip entries: 118 loop true and 15 false including the actor. Fifty-two loop-true individual files are obvious one-shot-like candidates by role: 27 attacks, 10 defense actions, 12 reactions, and three taunts. Of the 132 individual motions, all except the malformed RM file produce AnimationClips. Stored source-avatar warnings appear in 131 metadata files, but Unity resolves and imports them; only the malformed file emits an active hierarchy error during the tested import.

## Mechanical baseline

| Finding/check | Affected scope | Potential impact | Evidence |
|---|---|---|---|
| Default inspect/measure/lint completed | 134 animation-bearing FBXs | Establishes readable baseline, not gameplay readiness. | `observed-animsmith`; all commands exit 0 under empty config. |
| Malformed Crouch FR RM hierarchy | 1/132 individual files | No humanoid clip; missing directional RM member. | Two bones, 27,680 bytes; blank skeleton report; Unity no AnimationClip; re-verified unchanged under 0.4.0. |
| Standard skeleton signature | 131/132 individual files | Strong within-pack interchange prerequisite. | 56 bones, signature `8ea3a291222d`. |
| Constant tracks (0.4.0 re-verified) | 17,078 baseline notes; 16,808 contract notes | Size/runtime opportunity but unsafe to prune without semantics. | Complete AnimSmith aggregation; identical to the published 0.3.0 counts; all lint exits 0. |
| Scale keys | Combined FBX only | Unexpected scale animation can complicate retarget/composition. | Bone `SM_1HandedSwordPropIdle2`. |
| Delivered loop contracts | 118 individual files applicable; 113 seam-velocity and 113 seam-rotation failures; 55 closure findings in 48 files | Can pop/pulse when truly cyclic; many results instead expose wrong one-shot declarations. | Contract lint derived from delivered Unity loopTime; 132 files linted, 17 exit 0/115 with findings, exactly matching the published 0.3.0 pass/fail split. |
| Loop-seam evaluation completeness (0.4.0) | 118/132 loop-seam-applicable files; 74 fully evaluated, 58 not evaluated | 0.4.0 now labels no-stride/stationary clips `not evaluated` instead of silently folding them into pass/fail; the 113 seam-velocity/seam-rotation finding counts above are unchanged by this relabeling. | 0.4.0 contract `loop_seam_applicability` (118 applicable/14 not applicable) and `loop_seam_evaluation` (74 complete/58 not evaluated). |
| Root-motion threshold | 14 labeled action RM files below 0.5 m/s | Translation-only classification may reject short/yaw actions incorrectly. | Measured speed; yaw intent not established. |
| Root trajectory (0.4.0) | 133/134 clips measured | Sampled-grid translation/yaw facts only; not a continuous-curve or engine-extraction proof, and not itself an ownership determination. | 45/133 move >1 cm horizontally; 87/133 stationary (≤1 cm); 0/133 exceed 1° yaw travel; `heading_axis` resolves `positive_y` on all 132 measured clips. |

## AnimSmith remediation evidence

| Source issue | Operation/declarations | Result | Independent verification | Remaining caveat |
|---|---|---|---|---|
| Raw locomotion phase (0.3.0, 2026-08-17, historical) | `transform --gait-anchor` on 24 IP ring members | 0/24 outputs; safe exit 2 because root has no finite horizontal forward axis at sample 0 | Command records confirm no output files | Superseded by the 0.4.0 re-run below; kept as dated historical evidence. |
| Raw locomotion phase (0.4.0, 2026-08-21) | `transform --gait-anchor` on the same 24 IP ring members, re-run under 0.4.0 | 24/24 outputs at exit 0; 0.4.0 measures a vertical (`positive_y`) heading and anchors every ring; circular spread falls Crouch 0.6974371→0.0524396, Run 0.6605044→0.1372773, Walk 0.7231052→0.0599383 | Post-anchor inspect/measure/lint retained; source never modified | Candidates unpromoted — no Humanoid-retarget or visual acceptance import. This lands the basis-safe capability tracked as [#426](https://github.com/mmannerm/animsmith/issues/426); it is not itself an engine or visual acceptance. |
| WalkForward duplicate endpoint | `transform --drop-duplicate-loop-endpoint`, re-run under 0.4.0 with the same result | Output emitted; endpoint closure removed; source never modified | Post-lint no longer reports loop-closure | Linear and angular seam-derivative errors remain. |
| Dense constant tracks | `transform --prune-constant-tracks` on WalkForward, SwordAttack1, and the combined FBX, re-run under 0.4.0 with the same result | Three outputs emitted; constant notes removed; source never modified | Source/output `diff` reports many deltas and format/node differences | Semantic/runtime equivalence not proven; still bounded by open [#401](https://github.com/mmannerm/animsmith/issues/401); do not promote. |
| Combined scale animation | Current diagnostics only | Warning remains | Post-lint retains scale-key finding | Artist/vendor must establish and repair intent. |
| Malformed RM hierarchy | No safe current operation | No output attempted; re-verified unchanged under 0.4.0 | Unity and offline report independently confirm absence | Hierarchy/animation must come from artist/vendor evidence. |

No gait transform touched RM's accumulating root translation or yaw; only the 24 IP ring members were anchored. 0.4.0's basis-safe heading measurement replaces 0.3.0's correct-but-blocking refusal; cyclic resampling still must not reorder root trajectory without independent displacement and yaw proof, and the anchored IP candidates remain unpromoted pending engine and visual review.

## Engine procedures and evidence

| Runtime | Version | Procedure | Observed result | Remaining gate |
|---|---|---|---|---|
| Unity (retained, captured 2026-08-17) | 6000.5.8f1 | Create disposable project; import Sword package, then Basic package; inventory importers/clips; sample representative Playables; mix full-body pairs; apply Humanoid upper-body masks; attach props at hand local identity. | Both imports exit 0; 131/132 Sword human clips; 8/9 samples, 3/3 blends, 3/3 masks, 2/2 attachments pass; only quarantined file fails. | Visual controller, contacts, root motion, target rig, compression, player build. |
| Unreal Engine | unspecified | Documentation review for Root Motion, Blend Spaces/Sync Groups, Blend Masks, and layered animations. | Capability documented; pack not imported; vendor states Epic-scaled but not Epic-rigged and supplies no UE4 files. | Import/retarget/graphs/contact/build. |
| Godot | stable | Documentation review for AnimationTree BlendSpace2D, filters, OneShot, sync, and root-motion API. | Capability documented; pack not imported. | Conversion/import, skeleton mapping, graphs, root, contacts, export. |
| Bevy | unspecified | Documentation review for AnimationGraph masks; inspect current retarget limitation. | Layer masks documented; FBX route and mature retarget path not established. | Convert to glTF, retarget, graph, root motion, performance/build. |
| Unity Humanoid (advisory, 0.4.0, 2026-08-21) | 6000.3, `unity-humanoid` profile revision 1 | `generate import-advice` against the delivered source. AnimSmith derives declarations from the delivered `.fbx.meta` files rather than inventing them: every meta sets `useFileUnits: 1`, and none sets `lockRootRotation`, `lockRootHeightY`, or `lockRootPositionXZ`. | Exit 0; under the stated assumption that an absent meta key takes Unity's serialized default (`false`), advice resolved root rotation and root position to `extract` for every clip. **That was a stated assumption, not an observation, and it is now falsified — see "Unity headless candidate probe (2026-08-21 correction)" below.** | This is 6000.3 profile-revision-1 advice, not observed Unity 6000.5.8f1 import behavior; the default assumption has now been checked against actual import and found wrong (see below). |
| Unreal Engine (advisory, 0.4.0, 2026-08-21) | UE 5.8, `unreal` profile revision 1 | `generate import-advice` against the delivered source. | Typed refusal `profile_settings_unmodeled`; exit 1. | Profile settings are not yet modeled for this engine; no advice was produced. |
| Godot (advisory, 0.4.0, 2026-08-21) | Godot 4.7, `godot` profile revision 1 | `generate import-advice` against the delivered source. | Typed refusal `profile_settings_unmodeled`; exit 1. | Profile settings are not yet modeled for this engine; no advice was produced. |
| Bevy (advisory, 0.4.0, 2026-08-21) | Bevy 0.19.0, `bevy` profile revision 1 | `generate addressability` on one generated GLB candidate (not a production asset; source never modified). | Exit 0; 1 animation row, coverage complete, predicted selector `Animation0`, facet state `available`, 0 findings. | Inventory/selector prediction only; not glTF loading, targets, graph wiring, or playback. |

### Unity headless candidate probe (2026-08-21 correction)

The Unity Humanoid advisory row above stated an explicit assumption: because `lockRootRotation`, `lockRootHeightY`, and `lockRootPositionXZ` are absent from every delivered `.fbx.meta`, the profile read Unity's serialization default of `false` for each key and projected `extract` for every clip. **That assumption is falsified by direct observation.** Unity `6000.5.8f1` was run headless (`-batchmode -nographics -quit -executeMethod CandidateProbe.Run`) in a **new**, disposable project — the retained combined-project probe above was not modified — reading `ModelImporterClipAnimation` on the delivered files together with their delivered `.meta`, across a 120-clip sample spanning all eight collection packs (including files from this pack, whose vendor-list filenames carry the `S&S` suffix, for example `Humanoid@3HitCombo1S&S.fbx`):

| Variant | Clips | `lockRootRotation` true | `lockRootHeightY` true | `lockRootPositionXZ` true |
|---|---:|---:|---:|---:|
| In-place (non-`_RM`) | 84 | 84 | 84 | 83 |
| Root-motion (`_RM`) | 36 | 36 | 28 | 5 |

Aggregate across the sample: 120/120 clip definitions inspected, 120/120 `lockRootRotation` true, 112/120 `lockRootHeightY` true, 88/120 `lockRootPositionXZ` true. The delivered importer policy is therefore **bake**, not extract, and it is per-variant and axis-specific: `lockRootPositionXZ` is the discriminator — baked (`true`) for essentially all in-place clips and mostly extracted (`false`) for root-motion clips — a coherent authored root-motion policy, not an oversight or a random default. This observation supersedes the stated default-value assumption in the advisory row above; it does not change that row's exit-0 result, only the projected lock values.

### GLB candidate import into Unity (2026-08-21)

All 134/134 AnimSmith 0.4.0 gait-anchored GLB candidates across the eight-pack collection — including this pack's 24 anchored in-place walk/run/crouch candidates (see AnimSmith remediation evidence above) — were staged into a separate, **new** Unity 6000.5.8f1 project using `com.unity.cloud.gltfast` 6.9.0, because Unity has no native GLB importer; the retained combined-project probe above was not modified. Result: 134/134 files staged produced assets, 134/134 produced exactly one Unity `AnimationClip`, and every clip is non-legacy and non-empty.

**Limit, stated plainly:** glTFast imports glTF animation as a **Generic** clip and does not reconstruct a Humanoid Avatar. This proves the candidates load and yield one well-formed clip in Unity; it does **not** test the Humanoid retarget path this pack actually uses, and it is not a visual or gameplay acceptance test. The 24 gait-anchored candidates for this pack therefore remain **unpromoted**, unchanged from the AnimSmith remediation evidence above.

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
3. Unreal Engine and Godot remain documentation-only plus a 0.4.0 typed `profile_settings_unmodeled` refusal; neither engine has import-advice or an actual import test. Bevy has only a 0.4.0 addressability advisory on a generated GLB, not glTF loading, retargeting, targets, graph wiring, or playback.
4. Delivered loop metadata is not reliable author intent for one-shots; the strict failure count is not a count of 113 visibly bad gameplay cycles.
5. `_RM` is vendor/naming evidence. Speed does not characterize yaw-only or short-displacement root semantics; the 0.4.0 pack-wide root-trajectory counts (45 moving/87 stationary/0 with >1° yaw) are sampled-grid regression facts, not continuous-curve or engine-extraction proof, and must not be read as declaring which axis owns movement for any action.
6. AnimSmith reports were inspected at frame zero only; no visual motion acceptance was performed.
7. Pruned GLBs are experimental and not production candidates because semantic equivalence was not established.
8. Current public pages and EULA do not prove the local artifact's revision, transaction date, or historical terms.
9. Only Basic Locomotion was tested cross-pack; the rest of the Ultimate Animation Collection is outside this evaluation.
10. AnimSmith 0.4.0's circular phase-spread metric (the smallest arc containing the ring) is the current measurement basis for this refresh; it is not directly comparable to a linear max-minus-min figure.
11. A newer AnimSmith classification alone — for example the malformed Crouch FR RM hierarchy — must not be read as inventing a missing clip or asserting author intent beyond what the file evidence shows.
12. Every issue cited in this report pair was checked against the public tracker on 2026-08-21. Open: [#401](https://github.com/mmannerm/animsmith/issues/401), [#411](https://github.com/mmannerm/animsmith/issues/411), [#427](https://github.com/mmannerm/animsmith/issues/427), [#437](https://github.com/mmannerm/animsmith/issues/437), and [#440](https://github.com/mmannerm/animsmith/issues/440). Closed and delivered in this release: [#407](https://github.com/mmannerm/animsmith/issues/407) (2026-08-17, the fail-closed gait policy behind the 0.3.0 refusal), [#426](https://github.com/mmannerm/animsmith/issues/426) (2026-08-18, vertical-forward-axis gait anchoring, the source of the 24/24 result), [#402](https://github.com/mmannerm/animsmith/issues/402) (2026-08-20, per-(bone, property) channel coverage), and [#408](https://github.com/mmannerm/animsmith/issues/408) (2026-08-20, root displacement and accumulated yaw). Issue state is time-sensitive and should be re-queried before reuse.
13. The integration recipe's `owner=validate-per-axis` step directs the reader to validate root-motion ownership axis by axis rather than assume it. The observed Unity importer locks bake root rotation on every sampled root-motion clip, so animation cannot be assumed to own root-motion yaw. The step is not a per-axis `movement_owner_xz` / `movement_owner_y` / `movement_owner_yaw` declaration, and no such declaration is derived from measured travel in this refresh. Measured root displacement and yaw are recorded as sampled facts only; choosing the per-axis owner remains a project and engine decision.
14. A 2026-08-21 direct Unity 6000.5.8f1 headless probe falsified the Unity Humanoid advisory's stated default-`false`/`extract` assumption for root-lock declarations (see Unity headless candidate probe above): the observed delivered policy is `bake` for in-place clips and per-axis `bake`/`extract` (XZ is the discriminator) for root-motion clips. The probe is headless-import evidence over a 120-clip cross-pack sample, not continuous visual or gameplay acceptance, and it does not by itself validate the `unity-humanoid` profile's other advice fields.
15. The 134/134 GLB-candidate Unity import (2026-08-21, including this pack's 24 candidates) proves glTFast produces one well-formed Generic `AnimationClip` per candidate in a fresh project; it does not exercise the Humanoid retarget path this pack uses, and it does not promote the candidates. A same-commit rebuild of AnimSmith `v0.4.0` produced a differently-hashed binary (SHA-256 `1e53013bbe3224557a8783eafeb818f4ef9d74666590cbaa8c18ef48c5b7d6fa`, versus the recorded `fd1eee57407aa02db88763d144389a7f5104204c40ddfbb28eb5885ca8cd54c6`) — the build is not byte-reproducible — but both builds emit byte-identical import-advice artifacts, so this appendix's regenerated Unity evidence is attributable to the tag and commit, not to the originally recorded binary digest.

## Reproduction

Source identity: RAR SHA-256 `4402f20ba681ec83cf01f60b8dfb69b59435b48408030a5fbb4f3454f64840d7`; Unitypackage SHA-256 `dde20f36bfceca17370fcb511b238c9b0598de7657b214c6d67188939bd5dcf7`. A 2026-08-21 re-inventory reproduces the published manifest exactly (0 added, 0 removed, 0 content changed); the retained logical-manifest digest `a5f52b3e12bab1a4859c31e7e3b7223a806ec48eed8abd406366494ef6c111a6` re-verifies.

Historical evaluator (2026-08-17 baseline/contract/remediation capture and the retained Unity 6000.5.8f1 probes): `animsmith 0.3.0 (v0.3.0-23-gc11f135)`, repository revision `c11f135ece5e980e6c98861a52a715a28a424ff9`, binary SHA-256 `2fbf038dab62e380f15d709fbed8be58bbec5d9c06a3dfd02a7adec2eba619b2`.

Current evaluator (2026-08-21 refresh): `animsmith 0.4.0`, tag `v0.4.0`, repository revision `6b37ad636b198ef8ff47fadbf6a3a51eb1a27c8e`, binary SHA-256 `fd1eee57407aa02db88763d144389a7f5104204c40ddfbb28eb5885ca8cd54c6`; output schema v10, measurements schema v15. A same-commit rebuild produced a differently-hashed binary (SHA-256 `1e53013bbe3224557a8783eafeb818f4ef9d74666590cbaa8c18ef48c5b7d6fa`) with byte-identical import-advice output (see Rebuild reproducibility above); this appendix's 2026-08-21 evidence is attributable to the tag and commit, not to one specific binary digest.

```text
# Exhaustive baseline: 136 FBXs; inspect/measure/lint JSON and Markdown
animsmith inspect --config config/baseline.animsmith.toml <input.fbx>
animsmith measure --config config/baseline.animsmith.toml --format json <input.fbx>
animsmith lint --config config/baseline.animsmith.toml --format json <input.fbx>

# Per-motion contracts: 132 files; 17 exit 0, 115 exit 1
animsmith lint --config config/contracts/<file>.animsmith.toml --format json <input.fbx>

# Bounded remediation, followed by inspect/measure/lint/diff; source never modified
animsmith transform --config <config> <input.fbx> -o <output.glb> --gait-anchor
animsmith transform --config <config> <input.fbx> -o <output.glb> --drop-duplicate-loop-endpoint
animsmith transform --config <config> <input.fbx> -o <output.glb> --prune-constant-tracks

# 0.4.0 engine-profile advisory checks (generated candidates only; source never modified)
animsmith --config config/engine/unity-humanoid.animsmith.toml generate import-advice <input.fbx>
animsmith --config config/engine/unreal.animsmith.toml generate import-advice <input.fbx>
animsmith --config config/engine/godot.animsmith.toml generate import-advice <input.fbx>
animsmith --config config/engine/bevy.animsmith.toml generate addressability <candidate.glb>
```

Retained portable artifacts (0.3.0, 2026-08-17) include: baseline summary SHA-256 `5aec24f63aad108179406ced3a8df42055d55961f530124f6487d6835b1dc3b1`; contract summary `a62c87b94e11a84ba238420b4b3f0462ca8e14e004b703b7ebb2a528afe74701`; clip catalog `a980252db9eb48dbddeae27ca150820a3f02c3cf9f25b4e2d3489488c659a60c`; remediation record `65802bf6980ec6105c8a1d254adb4d7183379cb61ba98ca53d1a47ae32fe9438`; combined Unity probe `c4310bedddfd27e06696207e8bb1c4076039126c467ed4964aba067c8524c392`; Basic cross-pack comparison `346e254927a65de26307a5e82da29f70d642c69cea3347b840fb0761e32a4142`. The 2026-08-21 AnimSmith 0.4.0 baseline and contract passes reproduce these historical totals exactly (17,078 constant-track notes; 17 pass/115 fail contracts), so no new summary digests are published for this refresh.

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

- Local source archive and extracted Unity metadata — private authorized input identified above, accessed 2026-08-17 and re-inventoried unchanged 2026-08-21.
- Protofactor, [Animset: Sword & Shield](https://protofactor.biz/product/animset-sword-shield/) — current price, counts, formats, rig/engine statements, accessed 2026-08-17.
- Protofactor, [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/) — current collection context, accessed 2026-08-17.
- Protofactor, [End User License Agreement](https://protofactor.biz/end-user-license-agreement/) — current protected-application, modification, transfer, and redistribution terms; not historical transaction evidence, accessed 2026-08-17.
- Unity, [Avatar Mask](https://docs.unity3d.com/es/current/Manual/class-AvatarMask.html), [imported clip masks](https://docs.unity3d.com/es/current/Manual/AnimationMaskOnImportedClips.html), [Animation Layers](https://docs.unity3d.com/6000.0/Documentation/Manual/AnimationLayers.html), [Root Motion](https://docs.unity3d.com/6000.0/Documentation/Manual/RootMotion.html), and [loop optimization](https://docs.unity3d.com/es/current/Manual/LoopingAnimationClips.html) — runtime capabilities only.
- Epic Games, [Root Motion](https://dev.epicgames.com/documentation/unreal-engine/root-motion-in-unreal-engine?application_version=5.7), [Blend Masks](https://dev.epicgames.com/documentation/unreal-engine/blend-masks-and-blend-profiles-in-unreal-engine?lang=en-US), and [Layered Animations](https://dev.epicgames.com/documentation/unreal-engine/using-layered-animations-in-unreal-engine?lang=en-US) — runtime capabilities only.
- Godot, [Using AnimationTree](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html) — blend, filter, sync, one-shot, and root-motion capabilities only.
- Bevy, [Animation Masks](https://bevy.org/examples/animation/animation-masks/) and [retargeting issue #15612](https://github.com/bevyengine/bevy/issues/15612) — current masking example and retargeting limitation context.
- AnimSmith issue tracker, queried 2026-08-21. Open: [#401](https://github.com/mmannerm/animsmith/issues/401) (constant-track pruning proof) and [#411](https://github.com/mmannerm/animsmith/issues/411) (declared-set speed/stride coherence). Closed and delivered in 0.4.0: [#402](https://github.com/mmannerm/animsmith/issues/402) (2026-08-20, per-(bone, property) channel coverage), [#408](https://github.com/mmannerm/animsmith/issues/408) (2026-08-20, root displacement and accumulated yaw), [#426](https://github.com/mmannerm/animsmith/issues/426) (2026-08-18, gait anchoring for rigs whose root local forward axis is vertical), and [#407](https://github.com/mmannerm/animsmith/issues/407) (2026-08-17, the fail-closed gait-anchor safety policy).
