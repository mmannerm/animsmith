# Animation pack evidence appendix: Protofactor Sword & Shield Animset

> Companion report: [technical evaluation](protofactor-sword-and-shield.md)
>
> Evidence status: **partial** — exact AnimSmith 0.7.0 baseline, contracts, remediation verification, addressability, and bounded advice plus a dated Unity 6000.5.8f1 observation; visual acceptance, Humanoid retarget of the candidates, and engine-editor/runtime passes remain unevaluated.
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
| Pack/edition | Local `Animset@Sword&Shield_PACKAGE.unitypackage`; constituent revision is not declared |
| Vendor/source | Protofactor; [current Sword & Shield product page](https://protofactor.biz/product/animset-sword-shield/) |
| Delivered scope | Full local RAR → one Unitypackage → 136 FBXs: 132 individual motions, one combined animation FBX, one skinned actor, sword prop, and shield prop; Unity metadata/materials/textures included |
| Target use | Game-engine use; generic third-person sword-and-shield controller and combined use with Basic Locomotion |
| Target engines | Unity 6000.5.8f1 observed; Unreal Engine, Godot, and Bevy documentation-only |
| Target rigs/packs | Supplied Protof-Actor and the separately evaluated Protofactor Basic Locomotion pack |
| Source manifest | `evidence/logical-assets-inventory.json`; SHA-256 `a5f52b3e12bab1a4859c31e7e3b7223a806ec48eed8abd406366494ef6c111a6` |
| Evaluation manifest | `evidence/evaluation-manifest.json`; SHA-256 `b9a5317dcd0ed0a4d46e3c9144cbfa3430ab473354cdf9901c796b8875287d02`; taxonomy/profile-set version 1 |
| Acquisition/license provenance | `user-stated`: local archive was downloaded from Protofactor.biz as part of the Ultimate Animation Collection. Current EULA permits protected released real-time applications and modification while restricting redistribution/resale; no receipt, download date, historical EULA, or local constituent revision was retained. Technical due diligence only, not legal advice. |
| Current evaluator | AnimSmith 0.7.0, tag `v0.7.0`, commit `461ac8a4f6bb368eb8637471a796f13eeb647140`, binary SHA-256 `01a501999c91d93abfb32b1f48241fccc70914fac27c9a650c31df44262578d8`, output schema v17 / measurements schema v16, captured 2026-08-26. |

The current product page, observed 2026-08-17, advertises USD 24.99, 132 animations, 45 root-motion and 87 in-place files, Unity Humanoid, Epic-skeleton scale/retarget intent, Unity 2019.4+, and no native UE4 package. The local individual-file counts match 132/45/87, but the page does not prove the local revision or purchase terms. The collection page is collection context, not constituent identity.

The evaluation manifest uses schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1`.

The current evaluation re-inventoried all 136 FBXs and ran the baseline, declared-contract, gait-anchor, remediation, and projection passes on the same source. Unity 6000.5.8f1 import, playback, blend, and mask observations are dated 2026-08-17 and are not a current engine rerun.

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
| Combat locomotion (56 files) | 55 normal humanoid files; Crouch FR RM quarantined; delivered loops have strict seam findings. | Common signature/timing except quarantine; current circular phase spreads fall Crouch 0.6974371→0.0524396, Run 0.6605044→0.1372773, Walk 0.7231052→0.0599383; 24/24 anchors succeed and remain unpromoted. | Dated Unity representative graphs only; no candidate Humanoid-retarget or visual import. |
| Actions/defense/equipment (43) | Files readable; 39 are delivered as loops although most are one-shots; 13 RM pairs. | Props supplied; no hit events, contacts, IK, or additive contract. | Unity samples/masks/attachments execute; visual/contact gameplay open. |
| Reactions/death (24) | Files readable; four heavy pairs; downed holds and recovery singles. | Four inferred transition chains; no pose/contact continuity gate. | State timing, interruption, ragdoll handoff, visual result open. |
| Idles/taunts (9) | Files readable; delivered loop metadata present. | Same standard signature; no sync or mask contract. | Unity idle sampled; loop/taunt visual acceptance open. |
| Combined FBX | Readable but 60-bone signature and one animated scale track. | Segmentation not authoritative. | Do not use as the runtime source. |

### Validation-profile status

| Validation profile | Selection | Result / next evidence |
|---|---|---|
| Marketplace intake | `selected` — `user-required` | File/tool intake complete; historical provenance partial. |
| Blended locomotion | `selected` — `observed-pack-capability` | Three directional families measured; quarantine findings; the current evaluation anchors all 24 IP candidates (unpromoted, no Humanoid-retarget or visual acceptance import); full visual graphs open. |
| Root-motion controller | `selected` — `observed-pack-capability` | Forty-five labels measured; the current evaluation additionally measures 45/133 clips moving >1 cm and 0/133 with >1° yaw pack-wide (sampled grid facts, not an ownership determination); action yaw/ownership and full runtime extraction open. |
| State-machine transitions | `selected` — `observed-pack-capability` | Equipment/death chains inferred; events, interruptions, and visual crossfades open. |
| Layered upper body/weapons | `selected` — `user-required` | Three Unity mask graphs run; pelvis, contact, kick, IK, and grip acceptance open. |
| Traversal/environment | `not-applicable` | No advertised environment-aligned traversal family. |
| Contact actions/interactions | `selected` — `observed-pack-capability` | Weapon/shield/kick content exists; contact timing and target interaction open. |
| Retargeted/customizable characters | `selected` — `vendor-intended` | Supplied Unity Avatar works for 131 clips; project-character and non-Unity retarget open. |
| Motion matching/search | `not-selected` | No target database contract. |
| Networked movement | `not-selected` | No authority/prediction/rollback contract. |
| Runtime performance | `selected` — `evaluator-selected-generic-scenario` | Constant tracks measured; equivalence, memory, CPU, and build size open. |

## Pack inventory and content evidence

The immutable source is `<authorized-local-source>/Animset@Sword&Shield_ASSET.rar`, 174,727,929 bytes, SHA-256 `4402f20ba681ec83cf01f60b8dfb69b59435b48408030a5fbb4f3454f64840d7`. It contains `Animset@Sword&Shield_PACKAGE.unitypackage`, 180,663,737 bytes, SHA-256 `dde20f36bfceca17370fcb511b238c9b0598de7657b214c6d67188939bd5dcf7`. The current inventory verifies both digests and 136 FBXs with 0 added, removed, or changed.

The reconstructed Unity delivery has 313 regular files: 136 FBXs and 162 metadata files plus materials, textures, and the vendor list. The motion directory has 132 individual files plus `Protof-Actor@Sword&ShieldAnimset.fbx`. The other FBXs are the actor, sword, and shield. The vendor list and current listing both reconcile to 132 individual files: 45 `_RM` and 87 non-RM.

Every normal individual file exposes embedded clip `Take 001`; meaningful identity comes from case-sensitive filenames and Unity metadata. The vendor list contains non-authoritative spellings/casing such as `RunFrowardRight`, `ParryHight2`<!-- vendor-id -->, `swordAttack2`, and `3hitCombo1`; exact report members use delivered filenames.

Unity metadata defines 133 clip entries: 118 loop true and 15 false including the actor. Fifty-two loop-true individual files are obvious one-shot-like candidates by role: 27 attacks, 10 defense actions, 12 reactions, and three taunts. Of the 132 individual motions, all except the malformed RM file produce AnimationClips. Stored source-avatar warnings appear in 131 metadata files, but Unity resolves and imports them; only the malformed file emits an active hierarchy error during the tested import.

## Mechanical baseline

| Finding/check | Affected scope | Potential impact | Evidence |
|---|---|---|---|
| Default inspect/measure/lint completed | 134 animation-bearing FBXs | Establishes readable baseline, not gameplay readiness. | `observed-animsmith`; all commands exit 0 under empty config. |
| Malformed Crouch FR RM hierarchy | 1/132 individual files | No humanoid clip; missing directional RM member. | Two bones, 27,680 bytes; blank skeleton report; Unity no AnimationClip; re-verified unchanged under the current evaluator. |
| Standard skeleton signature | 131/132 individual files | Strong within-pack interchange prerequisite. | 56 bones, signature `8ea3a291222d`. |
| Constant tracks (current evaluation) | 17,078 baseline notes; 16,808 contract notes | Size/runtime opportunity but unsafe to prune without semantics. | Complete AnimSmith aggregation; all lint exits 0. |
| Scale keys | Combined FBX only | Unexpected scale animation can complicate retarget/composition. | Bone `SM_1HandedSwordPropIdle2`. |
| Delivered loop contracts | 118 individual files applicable; 113 seam-velocity and 113 seam-rotation failures; 55 closure findings in 48 files | Can pop/pulse when truly cyclic; many results instead expose wrong one-shot declarations. | Contract lint derived from delivered Unity loopTime; 132 files linted, 17 exit 0/115 with findings. |
| Loop-seam evaluation completeness (current) | 118/132 loop-seam-applicable files; 74 fully evaluated, 58 not evaluated | No-stride/stationary clips account for most not-evaluated results; the 113 seam-velocity/seam-rotation findings apply to the evaluated scope. | current contract `loop_seam_applicability` (118 applicable/14 not applicable) and `loop_seam_evaluation` (74 complete/58 not evaluated). |
| Root-motion threshold | 14 labeled action RM files below 0.5 m/s | Translation-only classification may reject short/yaw actions incorrectly. | Measured speed; yaw intent not established. |
| Root trajectory (current) | 133/134 clips measured | Sampled-grid translation/yaw facts only; not a continuous-curve or engine-extraction proof, and not itself an ownership determination. | 45/133 move >1 cm horizontally; 87/133 stationary (≤1 cm); 0/133 exceed 1° yaw travel; `heading_axis` resolves `positive_y` on all 132 measured clips. |

## AnimSmith remediation evidence

| Source issue | Operation/declarations | Result | Independent verification | Remaining caveat |
|---|---|---|---|---|
| Raw locomotion phase (current evaluation; Unity observation 2026-08-21) | `transform --gait-anchor` on the same 24 IP ring members, re-run under the current evaluator | 24/24 outputs at exit 0; the current evaluation measures a vertical (`positive_y`) heading and anchors every ring; circular spread falls Crouch 0.6974371→0.0524396, Run 0.6605044→0.1372773, Walk 0.7231052→0.0599383 | Post-anchor inspect/measure/lint retained; source never modified | Candidates unpromoted — current basis-safe anchoring is mechanical evidence, not Humanoid-retarget, engine, or visual acceptance. |
| WalkForward duplicate endpoint | `transform --drop-duplicate-loop-endpoint`, re-run under the current evaluator with the same result | Output emitted; endpoint closure removed; source never modified | Post-lint no longer reports loop-closure | Linear and angular seam-derivative errors remain. |
| Dense constant tracks | `transform --prune-constant-tracks` on WalkForward, SwordAttack1, and the combined FBX, re-run under the current evaluator with the same result | Three outputs emitted; constant notes removed; source never modified | Source/output `diff` reports many deltas and format/node differences | Semantic/runtime equivalence not proven; still bounded by open [#401](https://github.com/mmannerm/animsmith/issues/401); do not promote. |
| Combined scale animation | Current diagnostics only | Warning remains | Post-lint retains scale-key finding | Artist/vendor must establish and repair intent. |
| Malformed RM hierarchy | No safe current operation | No output attempted; re-verified unchanged under the current evaluator | Unity and offline report independently confirm absence | Hierarchy/animation must come from artist/vendor evidence. |

No gait transform touched RM's accumulating root translation or yaw; only the 24 IP ring members were anchored. Cyclic resampling must not reorder root trajectory without independent displacement and yaw proof, and the candidates remain unpromoted pending engine and visual review.

## Engine procedures and evidence

| Runtime | Version | Procedure | Observed result | Remaining gate |
|---|---|---|---|---|
| Unity (retained, captured 2026-08-17) | 6000.5.8f1 | Create disposable project; import Sword package, then Basic package; inventory importers/clips; sample representative Playables; mix full-body pairs; apply Humanoid upper-body masks; attach props at hand local identity. | Both imports exit 0; 131/132 Sword human clips; 8/9 samples, 3/3 blends, 3/3 masks, 2/2 attachments pass; only quarantined file fails. | Visual controller, contacts, root motion, target rig, compression, player build. |
| Unreal Engine | unspecified | Documentation review for Root Motion, Blend Spaces/Sync Groups, Blend Masks, and layered animations. | Capability documented; pack not imported; vendor states Epic-scaled but not Epic-rigged and supplies no UE4 files. | Import/retarget/graphs/contact/build. |
| Godot | stable | Documentation review for AnimationTree BlendSpace2D, filters, OneShot, sync, and root-motion API. | Capability documented; pack not imported. | Conversion/import, skeleton mapping, graphs, root, contacts, export. |
| Bevy | unspecified | Documentation review for AnimationGraph masks; inspect current retarget limitation. | Layer masks documented; FBX route and mature retarget path not established. | Convert to glTF, retarget, graph, root motion, performance/build. |
| Unity Humanoid projection | Revision 1 / Unity 6000.3 | Current `generate import-advice` projection | Available; bounded settings projection only | Verify importer settings on the target project and character |
| Unreal Engine projection | UE 5.8, `unreal` profile revision 2 | Current `generate import-advice` projection | Available; no engine process ran | Import, retarget, graphs, contacts, and build remain open. |
| Godot projection | Godot 4.7, `godot` profile revision 2 | Current `generate import-advice` projection | Available; no engine process ran | Import, retarget, graphs, contacts, and export remain open. |
| Bevy addressability | Bevy 0.19.0, `bevy` profile revision 3 | `generate addressability` on one generated GLB candidate (not a production asset; source never modified). | Exit 0; 1 animation row, coverage complete, predicted selector `Animation0`, facet state `available`, 0 findings. | Inventory/selector prediction only; not glTF loading, targets, graph wiring, or playback. |

### Unity headless importer observation (2026-08-21)

A direct Unity 6000.5.8f1 headless sample of 120 clips observed baked root rotation, baked XZ for nearly every in-place clip, and extracted XZ for most root-motion clips. This is sampled importer evidence, not visual or gameplay acceptance.

| Variant | Clips | `lockRootRotation` true | `lockRootHeightY` true | `lockRootPositionXZ` true |
|---|---:|---:|---:|---:|
| In-place (non-`_RM`) | 84 | 84 | 84 | 83 |
| Root-motion (`_RM`) | 36 | 36 | 28 | 5 |

Aggregate across the sample: 120/120 clip definitions inspected, 120/120 `lockRootRotation` true, 112/120 `lockRootHeightY` true, 88/120 `lockRootPositionXZ` true. The delivered importer policy is per-variant and axis-specific: rotation is baked throughout, while `lockRootPositionXZ` is baked for nearly every in-place clip and extracted for most root-motion clips.

### GLB candidate import into Unity (2026-08-21)

All 134/134 AnimSmith 0.7.0 gait-anchored GLB candidates across the eight-pack collection — including this pack's 24 anchored in-place walk/run/crouch candidates (see AnimSmith remediation evidence above) — were staged into a separate, **new** Unity 6000.5.8f1 project using `com.unity.cloud.gltfast` 6.9.0, because Unity has no native GLB importer; the retained combined-project probe above was not modified. Result: 134/134 files staged produced assets, 134/134 produced exactly one Unity `AnimationClip`, and every clip is non-legacy and non-empty.

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
3. Current Unreal revision-2 and Godot revision-2 settings projections are available, but neither engine received an import or playback test.
4. Delivered loop metadata is not reliable author intent for one-shots; the strict failure count is not a count of 113 visibly bad gameplay cycles.
5. `_RM` is vendor/naming evidence. Speed does not characterize yaw-only or short-displacement root semantics; current pack-wide root-trajectory counts (45 moving/87 stationary/0 with >1° yaw) are sampled-grid facts, not continuous-curve or engine-extraction proof, and do not declare movement ownership.
6. AnimSmith reports were inspected at frame zero only; no visual motion acceptance was performed.
7. Pruned GLBs are experimental and not production candidates because semantic equivalence was not established.
8. Current public pages and EULA do not prove the local artifact's revision, transaction date, or historical terms.
9. Only Basic Locomotion was tested cross-pack; the rest of the Ultimate Animation Collection is outside this evaluation.
10. AnimSmith 0.7.0 reports circular phase spread as the smallest arc containing the ring; it is not directly comparable to a linear max-minus-min figure.
11. A newer AnimSmith classification alone — for example the malformed Crouch FR RM hierarchy — must not be read as inventing a missing clip or asserting author intent beyond what the file evidence shows.
12. Current limitations are described by capability and owner in the issue table; issue state is time-sensitive and should be re-queried before reuse.
13. The integration recipe's `owner=validate-per-axis` step directs the reader to validate root-motion ownership axis by axis rather than assume it. The observed Unity importer locks bake root rotation on every sampled root-motion clip, so animation cannot be assumed to own root-motion yaw. The step is not a per-axis `movement_owner_xz` / `movement_owner_y` / `movement_owner_yaw` declaration, and no such declaration is derived from measured travel in this refresh. Measured root displacement and yaw are recorded as sampled facts only; choosing the per-axis owner remains a project and engine decision.
14. A direct Unity 6000.5.8f1 headless sample of 120 clips observed baked root rotation, baked XZ for nearly every in-place clip, and extracted XZ for most root-motion clips. This is sampled importer evidence, not visual or gameplay acceptance.
15. The 134/134 GLB-candidate Unity import, including this pack's 24 candidates, proves glTFast produces one Generic `AnimationClip` per candidate. It does not exercise the Humanoid retarget path or promote the candidates.

## Changes between AnimSmith versions

| Evaluator | Change from the preceding evaluated state |
|---|---|
| AnimSmith 0.7.0 | Revalidated the 136-FBX baseline, 132 declared contracts, 24 gait candidates, endpoint/pruning trials, and current engine projections under output v17 / measurements v16. |
| AnimSmith 0.4.1 | Reproduced the evaluated 0.4.0 measurements and transforms for this corpus; unrelated release fixes did not change the pack conclusion. |
| AnimSmith 0.4.0 | Added root-trajectory, channel-coverage, and profile evidence and confirmed basis-safe gait anchoring for the vertical-forward-axis rig. |
| AnimSmith 0.3.0 | Established the initial baseline, contract, remediation, and dated Unity evidence. Those evaluator results are superseded. |

## Reproduction

### Current AnimSmith reproduction (2026-08-26)

The same source corpus was re-inventoried and rerun with `animsmith 0.7.0`, exact tag `v0.7.0`, commit `461ac8a4f6bb368eb8637471a796f13eeb647140`, binary SHA-256 `01a501999c91d93abfb32b1f48241fccc70914fac27c9a650c31df44262578d8`, output schema v17, and measurements schema v16.

| Current external evidence | SHA-256 | Result |
|---|---|---|
| Source inventory | `e4dc4740bf35ff2812e81ff78970fc6737e62e8022664643c14b5cb8fdf2e4b8` | 136 FBXs; source unchanged |
| Exhaustive baseline | `7ee7b4063350b006a9831d0b281db56905b76f62176552ad299ddbe0251eb557` | 134 animation-bearing FBXs complete |
| Declared contracts | `4b80d70b3fd1debb96db3e6851fe2d36b6f86903908c2e710dfaf22e2aea4b1b` | 132 files; 17 pass / 115 fail |
| Remediation | `b522372a6a2687eeb8c3e93b8f7c7db979eb64ebaa7c1e681cf0dcb9abb76e6e` | 28 candidates completed and verified |
| 0.7 supplemental projections | `5d812576b84a2f21ccb7aa8351863aee93027424322a88e4fe6ac14db03d8556` | 28 addressability V1 + rich V2 pairs; exact-profile advice available |

The current projections do not evaluate shield/weapon contact, runtime graph wiring, target survival, retarget deformation, or visual acceptance.

## Sources

- Local source archive and extracted Unity metadata — private authorized input identified above, accessed 2026-08-17 and re-inventoried unchanged 2026-08-21.
- Protofactor, [Animset: Sword & Shield](https://protofactor.biz/product/animset-sword-shield/) — current price, counts, formats, rig/engine statements, accessed 2026-08-17.
- Protofactor, [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/) — current collection context, accessed 2026-08-17.
- Protofactor, [End User License Agreement](https://protofactor.biz/end-user-license-agreement/) — current protected-application, modification, transfer, and redistribution terms; not historical transaction evidence, accessed 2026-08-17.
- Unity, [Avatar Mask](https://docs.unity3d.com/es/current/Manual/class-AvatarMask.html), [imported clip masks](https://docs.unity3d.com/es/current/Manual/AnimationMaskOnImportedClips.html), [Animation Layers](https://docs.unity3d.com/6000.0/Documentation/Manual/AnimationLayers.html), [Root Motion](https://docs.unity3d.com/6000.0/Documentation/Manual/RootMotion.html), and [loop optimization](https://docs.unity3d.com/es/current/Manual/LoopingAnimationClips.html) — runtime capabilities only.
- Epic Games, [Root Motion](https://dev.epicgames.com/documentation/unreal-engine/root-motion-in-unreal-engine?application_version=5.7), [Blend Masks](https://dev.epicgames.com/documentation/unreal-engine/blend-masks-and-blend-profiles-in-unreal-engine?lang=en-US), and [Layered Animations](https://dev.epicgames.com/documentation/unreal-engine/using-layered-animations-in-unreal-engine?lang=en-US) — runtime capabilities only.
- Godot, [Using AnimationTree](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html) — blend, filter, sync, one-shot, and root-motion capabilities only.
- Bevy, [Animation Masks](https://bevy.org/examples/animation/animation-masks/) and [retargeting issue #15612](https://github.com/bevyengine/bevy/issues/15612) — current masking example and retargeting limitation context.
