# Animation pack evidence appendix: Protofactor Basic Locomotion Animset

> Companion report: [technical evaluation](protofactor-basic-locomotion.md)
>
> Evidence status: **partial** — exhaustive file and AnimSmith coverage plus a Unity 6000.5.8f1 headless probe; other runtimes and visual acceptance remain unevaluated.
>
> Evaluation date: **2026-08-16**
>
> Report format: **1**

This appendix preserves the evidence behind the concise technical report. It is intentionally exhaustive about manifests, pipeline stages, readiness, validation profiles, commands, and unknowns. The [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder) remains authoritative.

## Evaluation scope and provenance

| Field | Value |
|---|---|
| Pack | Local `Animset@BasicLocomotion_PACKAGE.unitypackage`; edition/version not declared in the archive |
| Vendor/source | Protofactor; [current Basic Locomotion product page](https://protofactor.biz/product/animset-basic-locomotion/) |
| Access | Locally held commercial archive inside “Protofactor Ultimate Animation Collection”; user states it was downloaded from Protofactor.biz |
| Price observed | Current Basic Locomotion page: USD 14.99; current [Protofactor Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/): USD 259.99; current [Ultimate Animation Collection Unity listing](https://marketplace.unity.com/packages/3d/animations/ultimate-animation-collection-195459): USD 259.99. That collection listing reports version 1.65 released 2026-08-16. Observed 2026-08-16; none proves the local artifact's edition or purchase terms. |
| Delivered scope | Full local RAR → one Unitypackage → 179 FBX files, including 177 per-motion FBX files, one combined animation FBX, and one skinned reference FBX; materials/textures and Unity metadata also delivered |
| Target game/use | Game-engine use only; no specific game, camera, character, controller, platform, networking model, or quality bar supplied |
| Target engines | Broad matrix includes Unity, Unreal Engine, Godot, and Bevy. Unity 6000.5.8f1 import and headless Playables probes completed; the other engines and visual/runtime acceptance remain deferred. |
| Target rigs/packs | Delivered Protof-Actor reference only; no project character or other animation pack supplied |
| License evidence | `user-stated`: the archive was downloaded from Protofactor.biz. No license document, receipt, download date, or transaction record is retained with the archive. The current [Protofactor EULA](https://protofactor.biz/end-user-license-agreement/) permits one license owner to use and modify assets in protected published real-time applications while restricting transfer, raw/derived asset resale, and redistribution. The historical terms remain unverified; this is technical due diligence, not legal advice. |
| Source manifest | `<evaluation-workspace>/evidence/logical-asset-manifest.json` |
| Evaluation manifest | `evidence/animsmith-0.2.1/evaluation-manifest.json`; schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1`; taxonomy/profile-set version 1; SHA-256 `a0a2249b0f29f1f60ac582bc053891c4fc98417c8531c2fca1d8ee510772e143` |

The current vendor Basic Locomotion page advertises 34 animations (12 root-motion and 22 in-place), whereas this local archive contains 177 per-motion files and 70 `_RM` files. The current product page therefore cannot be treated as the manifest for this artifact. The local content was evaluated as an edition-unknown artifact, not as a verified copy of today's SKU. The current Protofactor collection page says the collection contains 23 animsets and more than 2,300 animations, including Basic Locomotion; that is useful collection-scope context but not proof of which constituent packs or versions are present in the local archive.

### Evidence coverage

| Surface | Offered/delivered | Evaluated | Findings | Not evaluated and why |
|---|---:|---:|---:|---|
| Animation files | 179 FBX | 179 | 179 readable; 12 strict time failures; 3 skeleton signatures | Continuous visual playback and artistic quality for all files |
| Distinct rigs/export variants | 3 signatures | 3 | 56-bone standard (136 files), 73-bone cover/grenade (41), 58-bone reference/combined (2) | Deformation and Avatar retarget quality in engine |
| AnimSmith default lint | 179 | 179 | 167 exit 0; 12 exit 1; 24,186 constant-track notes | Default lint lacks game semantics for most checks |
| AnimSmith contract lint | 177 per-motion files | 177 | 58 exit 0; 119 exit 1 under declarations derived from Unity metadata/filename policy | Contracts excluded the reference and unsliced combined source |
| Offline visual reports | 179 possible | 9 representative | Coherent static midposes; expected stationary/translating roots; combined-file report is not usable as one gameplay clip | Motion/contact/loop quality cannot be proven from static samples |
| Engine imports | 1 native Unity route | 1 completed | 179 FBXs processed; 177 humanoid clips available; combined FBX copied-avatar hierarchy mismatch | Visual playback, compression, package conflicts, and player build |
| Blend/mask/retarget tests | 3 directional rings measured offline | 6 representative Unity samples and 3 two-clip blends | All headless Playables checks passed; raw cross-file phase mismatch remains | Full 8-way blend spaces, AvatarMask, additive, crossfade, deformation, target-rig retarget |

### Claim legend

Use: `user-stated`, `observed-file`, `observed-animsmith`, `observed-report`,
`observed-engine`, `vendor-stated`, `documentation-stated`, `inferred`, and
`not-evaluated`.

## Evaluation manifest and taxonomy

### Canonical clip-role inventory

The validated manifest maps every per-motion file to exactly one primary role. In-place/root-motion counterparts are grouped as one logical motion only where skeleton, duration, naming, and measured behavior support the pairing.

| Canonical primary role | Logical motions | Delivered files | Evidence boundary |
|---|---:|---:|---|
| `idle-pose` | 14 | 14 | Standing/crouched, cover, and grenade-aim holds; stationary 90°/180° turns are excluded and classified as transitions. |
| `continuous-locomotion` | 34 | 68 | Walk/run/sprint/crouch/cover-strafe counterparts. |
| `locomotion-transition` | 30 | 60 | Stationary turns, moving pivots/U-turns, and cover entry/exit counterparts. |
| `airborne` | 9 | 13 | Four paired takeoffs plus five fall/apex/landing singles. |
| `traversal` | 2 | 4 | Left/right 1 m obstacle counterparts. |
| `action-interaction` | 18 | 18 | Cover peek and grenade aim/throw actions. |
| `reaction-death` | 0 | 0 | Absent from delivered per-motion files. |
| `emote-cinematic` | 0 | 0 | Absent from delivered per-motion files. |
| `other-unknown` | 0 | 0 | No per-motion file remained unclassified. |
| **Total** | **107** | **177** | 70 paired motions (140 files) and 37 single variants. |

### Runtime-set inventory

Runtime relationships remain separate from roles because a clip may participate in more than one set.

| Runtime set | Type | Members/variants | Grouping evidence | Validation status |
|---|---|---|---|---|
| Walk | directional-blend | IP/RM 8-way rings | Common skeleton/timing, direction names, measured phase; medium confidence | Both raw rings have gait-phase findings; in-place ring transformed; Unity sampled/blended one representative pair only. |
| Run | directional-blend | IP/RM 8-way rings | Common skeleton/timing, direction names, measured phase; medium confidence | Both raw rings have gait-phase findings; in-place ring transformed; Unity sampled/blended one representative pair only. |
| Crouch | directional-blend | IP/RM 8-way rings | Common skeleton/timing, direction names, measured phase; medium confidence | Both raw rings have gait-phase findings; in-place ring transformed; Unity sampled/blended one representative pair only. |
| Forward walk/run/fast-run | speed-blend | Two IP/RM sets; three speeds each | Naming and common skeleton; low confidence | Candidate speed blends; no declared set or runtime test. |
| Sprint forward/left/right | directional-blend | Two IP/RM sets; three directions each | Naming and common skeleton; low confidence | Candidate directional blends; no declared set or runtime test. |

The companion report's [runtime-set table](protofactor-basic-locomotion.md#runtime-sets-and-authored-motion) retains the exact 24 paired members, cycle durations, per-direction root speeds, and cross-member speed ratios; this appendix preserves grouping and validation evidence without duplicating that table.

### Pipeline-stage coverage

| Stage | Coverage state | Evidence / remaining gate |
|---|---|---|
| Acquire | `partially-evaluated` | Local archive and current vendor/license URLs identified; historical transaction/edition not retained. |
| Preserve raw | `evaluated-clean` | Immutable RAR retained; extraction and generated outputs are separate. |
| Inspect | `evaluated-finding` | All 179 FBXs inspect/measure/lint; 12 negative-time files and three skeleton signatures. |
| Segment | `partially-evaluated` | 177 atomic per-motion files; combined take has no complete authoritative ranges. |
| Root motion | `partially-evaluated` | 70 counterpart pairs measured; yaw/extraction/controller behavior incomplete. |
| Conform | `partially-evaluated` | Slicing and in-place gait anchoring trialed; the evaluator deliberately did not run root-motion anchoring because 0.2.1 would reorder accumulating translation/yaw. |
| Validate | `partially-evaluated` | Exhaustive mechanical and provisional semantic checks; one engine, no visual acceptance. |
| Optimize | `partially-evaluated` | Constant-track pruning trialed but not approved. |
| Export | `partially-evaluated` | Unity imported native package; no generated production export or player build. |
| Gate/report | `evaluated-clean` | Commands, manifests, digests, primary report, and appendix retained. |

### Readiness evidence by clip set

Use the repository's [six-level readiness ladder](../game-ready-clips.md#the-readiness-ladder); this table reports evidence at those levels rather than redefining them.

| Role or set | File-ready / clip-ready | Set-ready / rig-use | Runtime / acceptance boundary |
|---|---|---|---|
| Idles/holds (14) | 14/14 mechanically clean with constant-track notes; six provisional loop declarations have seam findings, including two with linear/closure findings. | No loop-intent or mask contract. | Unity import covered; visual looping and layered use untested. |
| Continuous locomotion (68) | 68/68 mechanically clean; the 24 clearly cyclic in-place ring files are the defensible loop subset, and 22 still have strict closure/seam-derivative findings after anchoring. | Six 8-way sets share timing; all six raw rings fail phase target. | Unity sampled six representatives and blended three pairs, not full rings or visual contacts. |
| Locomotion transitions (60) | 10/60 have negative-time errors until sliced; many of the 25 loop flags are semantically suspect for one-shots. | No authoritative transition chains; 56/73-bone boundaries occur. | Curated one-shots only until crossfade/interruption tests. |
| Airborne/traversal (17) | Mechanically clean; four obstacle files and falling are provisionally loop-marked despite likely one-shot/hold semantics. | No trajectory/contact/environment chain. | Controller and environment integration untested. |
| Actions/interactions (18) | 2/18 have negative-time errors until sliced; eight likely one-shot grenade actions are loop-marked. | Full-body tracks; no additive/mask/contact contract. | Unity import only; prop, IK, recovery, and visual result untested. |
| Three in-place 8-way rings | Raw phase spreads 0.660/0.463/0.716; anchored spreads 0.072/0.094/0.050. | Current transform is an in-place candidate only. | Full blend spaces and loop wraps need visual target-character review. |
| Three root-motion 8-way rings | Raw phase spreads exceed 0.15. | AnimSmith 0.2.1 gait anchoring resamples accumulating translation and is not approved. | Runtime phase offsets or artist/root-preserving tooling required. |

### Validation-profile status

| Validation profile | Selection | Result / next evidence |
|---|---|---|
| Marketplace intake | `selected` — `user-required` | File/tool intake complete; provenance partial. |
| Blended locomotion | `selected` — `observed-pack-capability` | Six phase findings; three in-place rings transformed; full runtime rings outstanding. |
| Root-motion controller | `selected` — `observed-pack-capability` | Pair translation measured; yaw, extraction, and controller ownership outstanding. |
| State-machine transitions | `selected` — `observed-pack-capability` | Members exist; authoritative chains, crossfades, interruption, and recovery outstanding. |
| Layered upper body/weapons | `selected` — `evaluator-selected-generic-scenario` | Full-body grenade content exists; masks, additive base, sockets, IK, and scale outstanding. |
| Traversal/environment | `selected` — `observed-pack-capability` | Files inspected; controller/environment composition outstanding. |
| Contact actions/interactions | `selected` — `observed-pack-capability` | Grenade/cover content exists; contact/release events and prop alignment outstanding. |
| Retargeted/customizable characters | `selected` — `observed-pack-capability` | Unity source-avatar references valid; target deformation outstanding. |
| Motion matching/search | `not-selected` | No target database contract. |
| Networked movement | `not-selected` | No authority/prediction/rollback contract. |
| Runtime performance | `selected` — `evaluator-selected-generic-scenario` | Constant tracks measured; target memory/CPU/build-size outstanding. |

## Pack inventory and content evidence

### Delivery and organization

The immutable source is `<authorized-local-source>/Animset@BasicLocomotion_ASSET.rar` (174,531,814 bytes; SHA-256 `6f821f56f84339ea1eb6fcaa97e3c70d4a38dd84c413012847f026748dff185f`). It contains one 178,333,789-byte Unitypackage dated 2023-04-25 internally. Extraction occurred only in the separate evaluation workspace; no source bytes were changed. Licensed source and derived evidence remain local and are identified here by portable labels and digests rather than machine-specific paths.

The reconstructed Unity logical tree contains 179 FBX payloads, 7 PNG textures, 2 material files, 1 animation-list text file, and 196 `.meta` files. Of the FBX files, 177 are named per motion, one is a combined animation take, and one is a skinned reference actor. Every imported animation FBX has Unity `animationType: 3` (Humanoid). The 177 per-motion assets copy the supplied reference Avatar; the combined file uses older metadata.

Organization is usable but not clean enough to be its own production contract:

- Every FBX exposes the generic embedded take name `Take 001`; meaningful names live in filenames and, for 164 files, Unity clip metadata.
- Fifteen per-motion files lack explicit `clipAnimations` metadata even though their FBX take is readable.
- Unity metadata is mixed across serialized versions 19301, 20300, and 23.
- The combined FBX is 9,591 frames / 319.667 seconds by measurement. The bundled animation list ends at frame 6,785, and the combined-file `.meta` declares only frames 0–2,211. The remaining ranges have no complete authoritative segmentation manifest.
- One per-motion filename contains `Standingt`. Separately, the bundled combined-take animation list—not the exact per-motion FBX identifiers in the primary report—contains `SpintTurnRight`, `WalkForwadRight`, `runForward2`, and `Runbackwards`. These source disagreements increase automation cost.

The 48 runtime-ring identifiers in the primary report were reconciled exactly, with case preserved, against the retained logical manifest; none was silently corrected from the animation-list spelling.

### Animation/gameplay coverage

| Family | Delivered clips/variants | Intended use | Material gaps for this game | Evidence |
|---|---|---|---|---|
| Idle/locomotion | Idle variants; 8-way walk, run, and crouch rings; sprint/fast-run; in-place and many `_RM` pairs | General third-person locomotion and blend trees | Raw ring phases are misaligned; loop seams and target speeds require runtime validation | observed-file, observed-animsmith |
| Starts/stops/pivots/transitions | 90°/180° turns, U-turns, forward turns, cover entry/exit/peek/strafe transitions | Direction changes and contextual cover transitions | No clearly named general locomotion start/stop family; coverage is naming-inferred | observed-file, inferred |
| Jump/traversal | Jump-to-apex, walk/run takeoffs by foot, falling, light/medium/heavy landing, 1 m obstacle passes | Basic airborne state machine and low obstacle traversal | No apex-to-land matching, trajectory, contact, ledge, vault-height, or controller test | observed-file, not-evaluated |
| Combat/actions/interactions | Grenade aim/idle/throws plus cover-specific throws | Simple grenade action and cover gameplay | No melee, firearms, hit reactions, deaths, paired interactions, or contact-event metadata | observed-file |
| Additive/aim/masked layers | Grenade aim/action poses are present as full-body clips | Possible override layer after authoring/configuration | No additive reference pose; no arm-only files; masks and IK were not tested | observed-file, not-evaluated |
| Reactions/death/other | Look-around, scratch/yawn, breathing | Ambient variation | Reactions/death are outside delivered scope | observed-file |

## Mechanical baseline

The untouched baseline below separates format/mechanical evidence from declared clip and set semantics.

### Untouched import and playback

Untouched **Unity import and headless evaluation** completed in Unity 6000.5.8f1. The package imported into a disposable project with exit 0. Unity processed 179 FBXs overall; the motion directory contains 178 FBXs (177 individual clips plus the combined take), all configured as Humanoid with valid source-avatar references. Six representative in-place/root-motion clips evaluated through `AnimationClipPlayable`, and three representative walk/run/crouch pairs evaluated through a 50/50 `AnimationMixerPlayable`. All nine checks completed without exceptions.

Unity logged one material pack finding: `Protof-Actor@BasicLocomotionAnimset.fbx`, the combined take, reports a copied-avatar hierarchy mismatch for the Hips transform. The 177 individual motion clips remain the recommended source route. Headless evaluation establishes importer and Playables compatibility, not visual motion or blend quality.

Untouched **offline** loading is strong: AnimSmith inspected and measured all 179 FBX files. Nine representative offline HTML reports were rendered at frame 0 and an injected midpoint and visually reviewed. Idle, walk, root-motion walk, side run, jump, landing, cover, and grenade samples showed coherent static humanoid poses without gross explosions. Root-motion walk showed a trajectory while its in-place partner remained stationary. The negative-time cover sample surfaced its errors. The 319.667-second combined take produced a visually dense, incoherent trajectory/transition report as expected for many actions presented as one clip.

Neither the static reports nor the headless Unity probe proves artistic motion quality, planted contacts, loop smoothness, full blend-space behavior, Avatar masks, target-mesh deformation, attachment scale, controller response, compression behavior, or player-build correctness.

### Untouched AnimSmith findings

| Finding or coverage gap | Affected scope | User-visible effect | Evidence |
|---|---|---|---|
| Non-monotonic negative-time keys at −0.033333335 s on translation/rotation/scale of `root_CoverUnarmedAnimset` | 12 files; 36 errors | Strict import/processing may reject or mishandle pre-roll; timelines do not start cleanly at zero | observed-animsmith: `baseline-summary.json` |
| Baked constant tracks | 179 files; 24,186 notes; 99–192 per file, median 137 | Source bloat and dense channel coverage; runtime cost unmeasured | observed-animsmith |
| Unity-declared loop closure failures | 58 of 111 declared loops; 84 errors | Possible pose pops or contact mismatch at wrap | observed-animsmith; declaration partly observed-file |
| Unity-declared loop seam derivative failures | Of 111 declared loops, 104 files fail linear seam velocity and 108 fail angular seam velocity | Possible visible speed/rotation discontinuity at wrap; many declarations appear semantically questionable | observed-animsmith |
| Root-motion declaration reports stationary root | 14 of 70 `_RM` files | Current check under-reports rotation-only or low-displacement root motion | observed-animsmith; tool/check limitation, not automatically a pack defect |
| Missing semantic coverage under baseline config | 16 of 26 selected checks commonly non-applicable | Exit 0 without explicit declarations does not establish game readiness | observed-animsmith |
| Generic embedded clip identity | 179 files expose `Take 001` | Shared configs cannot naturally express distinct cross-file clip contracts by embedded name | observed-file, observed-animsmith |

The 12 negative-time files are:

- `GoBackToCoverLeftStanding` (in-place and `_RM`)
- `GoBackToCoverRightCrouching` (in-place and `_RM`)
- `GoBackToCoverRightStanding` (in-place and `_RM`)
- `GoOutOfCoverRightStanding_RM`
- `IdleStandingToTakeCoverCrouching` (in-place and `_RM`)
- `IdleTakeCoverCrouchingToIdleStanding` (in-place only)
- `ThrowGrenadeLeftUnderCoverStanding`
- `ThrowGrenadeRightUnderCoverStanding`

## AnimSmith remediation evidence

### Captured evaluator

| Field | Value |
|---|---|
| AnimSmith version | `animsmith 0.2.1 (v0.2.0-4-gb6d0f9a)` |
| Repository commit | `b6d0f9a5b06d8e5f907fbb87dc6d07ec55525b47` |
| Invocation | `<animsmith-checkout>/target/debug/animsmith`; SHA-256 `ac4a41527888778a3cdacc77401a014ca479a0bfe58fa53b5107b7a9cb6159f5` |
| Available commands/features | `inspect`, `measure`, `lint`, `report`, `transform`, `fix`, `convert`, `assemble`, `scale`, `diff`; current `fix` is glTF/GLB-only quaternion normalization/sign continuity |
| Baseline config and digest | `<evaluation-workspace>/config/baseline.animsmith.toml`; SHA-256 `612df8cc230c9e80b14373ef40336038b9fe308c8327f1529cbdb70612b9cc59` |
| Contract config and digest | 177 per-file configs under `config/contracts-0.2.1`; path-independent sorted-content aggregate SHA-256 `a1b0907976f8a3b6e56b682595e6a9aa35b3e733fe076c16b6aacd1d5a7a3024` |
| Evidence directory | `<evaluation-workspace>/evidence/animsmith-0.2.1` |

Build note: the repository-configured `sccache` wrapper stalled on `rustc -vV` during the original pass; the documented `RUSTC_WRAPPER=` fallback was used to build the pinned evaluator. The full baseline, contract, and remediation command sets were rerun after rebasing to 0.2.1 and reproduced the prior findings.

### Current-tool remediation trial

| Source issue | Operation and declarations | Result | Verification | Effort | Remaining caveat |
|---|---|---|---|---|---|
| Negative-time keys in 12 files | `transform --slice 0:<Unity lastFrame/30> --fps 30`; range derives from each delivered Unity clip declaration | 12/12 transforms succeeded; all 36 time errors removed | 12 inspect/measure and fix dry-runs exit 0; 10 contract lints exit 0; 2 grenade files retain only loop-seam errors; `diff` exit 1 is expected for intentional one-frame trim | Small, repeatable preprocessing | Requires trusting the delivered per-file Unity frame range; transformed output is GLB. |
| Misaligned 8-way gait phases | `transform --gait-anchor` on 24 **in-place** walk/run/crouch files using per-file loop/in-place/humanoid declarations | 24/24 transforms succeeded; phase spreads reduced to walk 0.072, run 0.094, crouch 0.050 | All inspect/measure and fix dry-runs exit 0; only 2/24 strict contract lints exit 0; remaining errors are loop closure/velocity/angular seam findings | Small/medium automated step plus review | In-place phase alignment improves, but endpoints and blend quality remain unresolved. Do not apply this operation to accumulating root motion. |
| Baked constant tracks | `transform --prune-constant-tracks` on a standard walk, cover clip, and combined file | 3/3 transforms succeeded; output/source byte ratios 12.3%, 8.7%, and 41.8% | All outputs inspect/measure/lint and fix dry-run exit 0; all `diff` runs exit 1 with large/index-sensitive measurement deltas | Small to run, high proof burden | Do not adopt from this trial. Dense transition coverage and semantic equivalence are not proven. |

The slice and in-place gait-anchor operations are **current declared transforms**: their behavior depends on explicit clip range or gait semantics rather than an inferred artistic rewrite. In AnimSmith 0.2.1, gait anchoring cyclically resamples every nonconstant channel, including accumulating root translation or yaw. That can break a root-motion trajectory, so the transform is refused for `_RM` recommendations in this report. A future root-motion-preserving cyclic rebase is plausible only with independently re-derived displacement and yaw proof. AnimSmith also does not repair genuine loop pose/velocity seams, retarget a rig, create additive motion, fix contacts, or author missing animation.

Current public issues [#401](https://github.com/mmannerm/animsmith/issues/401) and [#402](https://github.com/mmannerm/animsmith/issues/402) document why pruning requires property-scoped policy and emitted `(bone, property)` coverage. No matching public issue was found on 2026-08-16 for cross-file clip identity/group contracts or Unitypackage ingestion; those are potential ideas, not roadmap commitments.

### Before/after conclusion

Current AnimSmith makes the 12 strict-time failures mechanically usable under a declared frame-range policy and makes the three core **in-place** directional rings phase-compatible by measurement. It does **not** make the root-motion rings safe to transform or turn the pack into a production-certified asset automatically. The post-anchor loop failures still require semantic reclassification, engine transition policy, or artist correction. The pruning trial demonstrates potential storage reduction but fails the proof bar and is excluded from the recommended pipeline.

## Engine procedures and evidence

### Import configuration

Native Unity delivery was selected because the source is a Unitypackage and all metadata declares Unity Humanoid animation. Unity `6000.5.8f1 (5cb7df797b7d)` on Windows 11 created a disposable project, imported the package, and exited 0. The import performed 199 asset imports and exposed 177 humanoid animation clips. The retained probe reports 178 Humanoid model importers in the motion directory, 178 valid source-avatar references, 177 human-motion clips, 6/6 representative sampling passes, and 3/3 representative blend passes.

Remaining Unity import gates are: retargeting each skeleton signature to the actual project character; reviewed loop/root-transform settings; before/after import of the 12 sliced clips; compression effects on contacts, fingers, prop bones, and seams; full blend controllers; masks/IK; and a player build.

### Runtime playback and root motion

Headless Playables sampling and pair mixing are evaluated; visual playback and controller behavior are not. The file-level pair inventory is favorable: all 70 `_RM` files have a same-skeleton non-RM partner, durations match for every pair, and frame counts match for 68. `GoOutOfCoverRightStanding_RM` is one frame longer than its partner; `IdleTakeCoverCrouchingToIdleStanding_RM` is one frame shorter.

The current root-motion speed contract is incomplete for turns. Four crouch turns, two run U-turns, four 90°/180° turns, two walk U-turns, and two cover strafes are `_RM`-labeled but fall below the default 0.5 m/s horizontal-speed threshold. These may contain rotational root motion or intentional low displacement. Validate translation and yaw separately in Unity rather than treating the check as a definitive defect.

Test root motion against controller collision, slopes/steps, capsule reconciliation, animation interruption, and the project's networking/rollback policy. For in-place motion, verify authored foot speed against controller speed; this evaluation did not derive stride-matched gameplay velocities.

### Performance and packaging

No engine import size, runtime memory, decompression CPU, build size, or platform performance was evaluated. Source FBX files carry many constant channels. AnimSmith can remove many of them in sampled GLBs, but byte reduction across FBX→GLB is not a runtime-performance measurement and the trial lacks an acceptable equivalence proof. Retain the untouched coverage until a target-runtime measurement and channel-coverage gate justify pruning.

## Blending, masking, and gameplay caveats

### Locomotion, sync, and transitions

The in-place walk, run, and crouch directional rings each contain eight files with equal duration and frame count within that ring, which is a good blend-space prerequisite. Their raw gait phases are not same-time aligned: minimum circular spread is 0.660 for walk, 0.463 for run, and 0.716 for crouch, compared with a common 0.15 alignment target. Direct blending at normalized time therefore risks mixing unlike foot phases.

Current AnimSmith gait anchoring reduces those spreads to 0.072, 0.094, and 0.050. This supports using the outputs as a blend-tree candidate, not as proof of clean blends. Twenty-two of the 24 anchored clips still fail at least one strict loop closure or seam derivative check. Unity must test the complete 2D blend space at cardinal, diagonal, and intermediate weights, including phase wrap, accelerations, stops, turns, and transitions to idle/jump.

The contract pass derives loop status from delivered Unity metadata. Since that metadata also marks grenade throws, falling, obstacle passes, and turns as loops, a human must first decide which clips are actually cyclic. Otherwise the failure count mixes content defects with incorrect declarations.

### Upper/lower-body masking and additive use

Runtime masking is not evaluated. The files carry full-body baked transform tracks and the Unity metadata's explicit clips have `hasAdditiveReferencePose: 0`; there are no dedicated upper-body-only or additive files. Unity Humanoid AvatarMasks may allow a grenade or aim clip to override the upper body while locomotion drives the lower body, but the mask boundary, spine continuity, pelvis ownership, arm reach, hand/prop alignment, and root behavior must be tested.

Recommended default: keep pelvis/root and legs in the locomotion layer, begin the action mask above a project-chosen spine boundary, and test several blend weights and transition times. If the action meaningfully shifts pelvis, center of mass, or supporting feet, a pure upper-body mask will likely look wrong and a full-body authored transition is preferable. No IK target bones are evident in the standard 56-bone hierarchy; the 73-bone cover/grenade variant adds cover/rocket/nub nodes but does not establish a portable hand/foot IK contract.

### Game-type caveats

| Game/system context | Suitability | Caveat or required work | Evidence |
|---|---|---|---|
| Third-person action prototype | Good candidate | Complete engine/character test; curate loops and transitions | observed-file, observed-report, inferred |
| Controller-driven in-place locomotion | Good candidate with conditions | Phase-anchor rings; tune controller speed; inspect foot slide and seam wraps | observed-animsmith |
| Root-motion locomotion | Candidate with conditions | Validate translation/yaw extraction, one-frame pair mismatches, collision, interruption, and networking | observed-animsmith, not-evaluated |
| Cover gameplay | Candidate with conditions | Uses 73-bone variant; 10 of the 12 negative-time files are cover-related; verify Avatar and cover geometry | observed-file, observed-animsmith |
| Upper-body grenade overlay | Prototype only | Full-body, non-additive files; mask/IK/prop alignment untested | observed-file, not-evaluated |
| Motion matching/distance matching | Poor fit without substantial processing | No trajectory/contact/phase database or distance annotations were delivered or tested | observed-file |
| Networked rollback/root motion | Unknown/high risk | Determinism, authority, correction, and interruption not evaluated | not-evaluated |
| First-person arms | Poor fit | No arm-only rig or first-person camera framing | observed-file |
| Close cinematic camera | Unknown | Static samples are plausible, but motion, fingers, contacts, deformation, and compression need high-quality review | observed-report, not-evaluated |

## Rig, masking, and compatibility evidence

### Within-pack sets

| Clip set/pair | Skeleton | Root motion | Timing/sync | Runtime blend/mask | Result | Evidence |
|---|---|---|---|---|---|---|
| 70 `_RM`/non-RM pairs | Same signature within every pair | Explicit filename convention; behavior partly measured | Durations match 70/70; frames match 68/70 | Not tested | Direct candidate with engine configuration | observed-file, observed-animsmith |
| 8-way in-place walk ring | 56-bone standard | In-place by filename; speed below 0.5 m/s | Equal duration/frames; raw phase spread 0.660, anchored 0.072 | Not tested | AnimSmith-current preprocessing candidate | observed-animsmith |
| 8-way in-place run ring | 56-bone standard | In-place by filename; speed below 0.5 m/s | Equal duration/frames; raw phase spread 0.463, anchored 0.094 | Not tested | AnimSmith-current preprocessing candidate | observed-animsmith |
| 8-way in-place crouch ring | 56-bone standard | In-place by filename; speed below 0.5 m/s | Equal duration/frames; raw phase spread 0.716, anchored 0.050 | Not tested | AnimSmith-current preprocessing candidate | observed-animsmith |
| Standard locomotion ↔ cover/grenade clips | 56-bone vs 73-bone signatures | Mixed; filename policy | Per-transition timing not evaluated | Humanoid retarget/masks not tested | Engine-config candidate, not exact-skeleton direct | observed-animsmith, inferred |
| Per-motion files ↔ combined take | 56/73-bone motions vs 58-bone combined/reference | Mixed in combined source | Combined segmentation incomplete/inconsistent | Not tested | Prefer per-motion assets; combined source unknown | observed-file, observed-animsmith |

### Cross-pack or target-rig compatibility

| Pack/rig pair | Skeleton/retarget | Scale/axes | Root policy | Timing/blend | Style/semantics | Overall | Evidence |
|---|---|---|---|---|---|---|---|
| Pack ↔ supplied Protof-Actor | Unity metadata references the actor Avatar for 177 per-motion files | File import flags use scale 1 and file units/scale; engine result unknown | `_RM`/in-place convention present | Runtime not tested | Intended source actor | Engine-config candidate | observed-file |
| Pack ↔ project character | No character supplied; Humanoid is promising but insufficient | Not evaluated | Project policy unknown | Not evaluated | Not evaluated | Unknown | not-evaluated |
| Pack ↔ another animation pack | No comparison pack supplied | Not evaluated | Not evaluated | Not evaluated | Not evaluated | Unknown | not-evaluated |

A meaningful future cross-pack report should compare at least: humanoid role mapping and rest pose, scale/axes, root ownership and translation/yaw conventions, sample rates and loop semantics, gait/contact phase, action semantics, style, AvatarMask/additive assumptions, and runtime blends on the same target character.

## Limitations and unknowns

1. No target game, engine project, character, controller, camera, platform, frame budget, networking policy, or artistic quality bar was supplied; suitability conclusions are deliberately generic.
2. Unity package import and headless sampling/pair-blending were evaluated, but no visual full-ring controller, target-character retarget, masks, root-motion controller, compression comparison, or player build was run. Unreal Engine, Godot, and Bevy remain documentation-only.
3. Static report samples and headless Playables evaluation cannot establish motion quality, planted contacts, deformation, loop perceptibility, full blend-space quality, masking, or compression behavior.
4. Root-motion classification partly relies on the `_RM` filename convention; speed-only checks do not characterize rotational or low-displacement root motion.
5. Contract loop declarations come from Unity metadata, which appears to over-label one-shots. Counts should not be interpreted as 108 visually bad gameplay cycles.
6. Cross-file phase spread was aggregated from per-file AnimSmith gait measurements because the current embedded clip identities/config model cannot directly express these groups across files.
7. The constant-pruning sample is not approved: source/output format differs, `diff` reports many deltas, and emitted per-property coverage is not currently available from the evaluator.
8. No malware/security audit beyond archive path/structure inspection was performed; no executables or scripts were found in the logical pack content.
9. No current vendor download was acquired, so the local artifact cannot be equated to the 2026 product listing.
10. No full artistic review of all 177 motions was conducted; nine representative offline reports were sampled.

## Reproduction

### Source identity

- Source RAR: `<authorized-local-source>/Animset@BasicLocomotion_ASSET.rar`
- RAR size: 174,531,814 bytes
- RAR SHA-256: `6f821f56f84339ea1eb6fcaa97e3c70d4a38dd84c413012847f026748dff185f`
- Archive member: `Animset@BasicLocomotion_PACKAGE.unitypackage`
- Unitypackage size: 178,333,789 bytes
- Evaluation workspace: `<evaluation-workspace>`
- Logical manifest SHA-256: `5bec4f741c39f232c79f4c841fc0eb580589f3868b614610cb6ff15a59a0b34b`
- Exclusions: no other Ultimate Animation Collection packs were extracted or evaluated; no licensed source bytes are stored in the AnimSmith repository.

### Evaluation manifest

- Schema: `urn:animsmith:skill:animation-pack-evaluation-manifest:1`
- Clip taxonomy version: `1`
- Validation-profile-set version: `1`
- Validated manifest: `evidence/animsmith-0.2.1/evaluation-manifest.json`
- Manifest SHA-256: `a0a2249b0f29f1f60ac582bc053891c4fc98417c8531c2fca1d8ee510772e143`
- Derivation script: `evidence/animsmith-0.2.1/build-evaluation-manifest.py`; SHA-256 `28b48e2109c0aa6c6e9b65c759e2c0b5442c390c67e7759c250ae84781d40545`

The manifest maps all 177 physical files to 107 logical motions and every logical motion to exactly one canonical role. It retains ten candidate runtime sets, all eleven profile-selection decisions, and all ten pipeline-stage coverage states. The validator recomputes role/file totals and checks all cross-references.

### AnimSmith commands and outcomes

```text
# Build/version capture (repository-configured sccache was bypassed after it stalled)
RUSTC_WRAPPER= cargo build --bin animsmith
target/debug/animsmith --version
# animsmith 0.2.1 (v0.2.0-4-gb6d0f9a)

# Exhaustive baseline pattern, repeated for all 179 FBX files
animsmith inspect --config config/baseline.animsmith.toml <input.fbx>
animsmith measure --config config/baseline.animsmith.toml --format json <input.fbx>
animsmith lint --config config/baseline.animsmith.toml --format json <input.fbx>
animsmith lint --config config/baseline.animsmith.toml --format markdown <input.fbx>
# inspect/measure: 179 exit 0; lint: 167 exit 0, 12 exit 1

# Per-motion contract pattern, repeated for 177 files
animsmith lint --config config/contracts/<file>.animsmith.toml --format json <input.fbx>
# 58 exit 0; 119 exit 1 under delivered/inferred declarations

# Negative-time remediation pattern, repeated for 12 files
animsmith transform --config <per-file-config> <input.fbx> -o <output.glb> \
  --slice 0:<unity-last-frame-divided-by-30> --fps 30
# 12/12 transform exit 0; no remaining time-monotonic errors

# Directional-ring remediation pattern, repeated for 24 in-place clips
animsmith transform --config <per-file-config> <input.fbx> -o <output.glb> --gait-anchor
# 24/24 transform exit 0; circular phase spread <= 0.094 per ring

# Experimental only; not approved for shipment
animsmith transform --config config/baseline.animsmith.toml <input.fbx> \
  -o <output.glb> --prune-constant-tracks
# 3/3 transform exit 0; 3/3 diff exit 1; semantic equivalence not established

# Every transformed output was inspected/measured/linted and checked with:
animsmith diff --config <config> --format json <source.fbx> <output.glb>
animsmith fix --config <config> --dry-run <output.glb>
```

The retained runners are `evidence/run_baseline.py`, `evidence/run_contract.py`, `evidence/run_remediation.py`, and their summarizers. Command argv, exit codes, stdout, and stderr are retained under `evidence/animsmith`.

### Engine procedure

Completed procedure:

1. Invoke Unity 6000.5.8f1 in batch mode with `-createProject` on a disposable evaluation path.
2. Import the extracted Unitypackage with `-importPackage`.
3. Capture the Editor log, import counts, warnings, and exit code.
4. Add a local-only Editor probe that inventories the motion FBXs, importer/avatar/clip metadata, and representative clips.
5. Evaluate six representative in-place/root-motion clips with `AnimationClipPlayable` and three walk/run/crouch pairs with `AnimationMixerPlayable`.

Observed result: package import exit 0 and probe exit 0. Unity exposed 177 human-motion clips and valid source-avatar references for all 178 motion-directory FBX importers. Six sampling and three pair-blend checks passed. The combined all-in-one FBX logged a copied-avatar hierarchy mismatch; the individual files remain usable. Next build a visual scene containing the source and target characters, complete 8-way controllers and transitions, root-motion toggles, AvatarMask layers, compression variants, and profiler/player-build measurements.

### Evidence artifacts

| Artifact | Purpose | Digest or identity |
|---|---|---|
| `evidence/logical-asset-manifest.json` | Reconstructed logical asset inventory and hashes | SHA-256 `5bec4f741c39f232c79f4c841fc0eb580589f3868b614610cb6ff15a59a0b34b` |
| `evidence/animsmith-0.2.1/evaluation-manifest.json` | Validated v1 canonical roles, delivered variants, runtime sets, profile selection, pipeline coverage, and per-file evidence | SHA-256 `a0a2249b0f29f1f60ac582bc053891c4fc98417c8531c2fca1d8ee510772e143` |
| `evidence/animsmith-0.2.1/build-evaluation-manifest.py` | Reproducible migration from retained per-file/category evidence to the v1 evaluation manifest | SHA-256 `28b48e2109c0aa6c6e9b65c759e2c0b5442c390c67e7759c250ae84781d40545` |
| `evidence/animsmith-0.2.1/clip-category-manifest.json` | Legacy eight-bucket source used to derive the canonical-role manifest; retained for auditability | SHA-256 `089835067e5a4b957599aaf2bfba3d2425a5b65667532dbde9b332dbaeb59751` |
| `evidence/animsmith-0.2.1/baseline-summary.json` | Exhaustive default inspect/measure/lint aggregation | SHA-256 `85c97c726c112efca5b1b3aa143f2b6c951917bc9de02fd544dd5a652090d75e` |
| `evidence/animsmith-0.2.1/baseline/command-results.json` | 0.2.1 baseline argv, exits, and per-command evidence paths | SHA-256 `dbc6fca3099e3e3711694b901c065bb3f2dc733a3d17eda00dcb345eefd3f690` |
| `evidence/animsmith-0.2.1/contract/command-results.json` | 0.2.1 per-motion contract argv, exits, configs, and evidence paths | SHA-256 `872ec43dc450fa7e8092caec3e8590cbe61d2c3b4e0a3f048e778e860c3a39e9` |
| `evidence/animsmith-0.2.1/remediation-batch/command-results.json` | 0.2.1 slice, gait-anchor, and prune trial/verification records | SHA-256 `c4c93efe8d8ffdd9c9de2fb9ec0265486e43ebee62867f2d06ab3faeb90fd031` |
| `evidence/unity-meta-summary.json` | Unity importer/clip metadata aggregation | SHA-256 `a73b5162632d2e8d40dae8971440d1555b434956d4c5e482e26383c29ea04458` |
| `evidence/report-screenshots/representative-midpoint-contact-sheet.png` | Visual QA contact sheet for nine offline reports | SHA-256 `e8ec17b71042f101bb1816141f2b187fcfb2f9de899289d9a04d98f38c0cfc1c` |
| `evidence/unity-6000.5.8f1-import.log` | Licensed Unity package import, asset counts, and combined-FBX rig warning | SHA-256 `914136cdb3458d6353d5258c9aeb5d1878097f8fe9bb96aafe22f01bb75e9ea4` |
| `generated/unity-6000.5.8f1-project/Assets/Editor/AnimationPackProbe.cs` | Local-only reproducible importer and Playables probe | SHA-256 `4471b9fc9b2c0b1cd334bac654fd0b35257b9f558403d9c2be418eb03620b351` |
| `evidence/unity-6000.5.8f1-probe.json` | Importer/clip inventory and representative Playables results | SHA-256 `e8128312b4db544c354c95c397a85fa68155adec1423eba3c22a413053f4fbb9` |
| `evidence/unity-6000.5.8f1-probe.log` | Headless probe execution log | SHA-256 `7e69b26f6482197046e3f365e15a0bb57e49efd9b9d047b5eef1d12defc5a9ce` |
| AnimSmith binary | Exact 0.2.1 evaluator executable | SHA-256 `ac4a41527888778a3cdacc77401a014ca479a0bfe58fa53b5107b7a9cb6159f5` |

## Sources

- Local source archive and extracted Unity metadata — private local artifact identified above, accessed 2026-08-16.
- Protofactor, [Animset: Basic Locomotion](https://protofactor.biz/product/animset-basic-locomotion/) — current product description, counts, formats, Unity compatibility, and price, accessed 2026-08-16.
- Protofactor, [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/) — current collection price, constituent-pack list, and advertised aggregate count, accessed 2026-08-16.
- Protofactor, [End User License Agreement](https://protofactor.biz/end-user-license-agreement/) — current one-owner, protected-real-time-application, modification, transfer, and redistribution terms; not evidence of the local transaction's governing terms, accessed 2026-08-16.
- Unity Asset Store, [Ultimate Animation Collection](https://marketplace.unity.com/packages/3d/animations/ultimate-animation-collection-195459) — current collection version/date, price, license tier, and original Unity version, accessed 2026-08-16.
- AnimSmith public issue [#165](https://github.com/mmannerm/animsmith/issues/165) — current roadmap guardrails for automatic animation rewrites, accessed 2026-08-16.
- AnimSmith public issues [#401](https://github.com/mmannerm/animsmith/issues/401) and [#402](https://github.com/mmannerm/animsmith/issues/402) — current constant-track pruning and emitted channel-coverage limitations, accessed 2026-08-16.
- Unity 6.5 Manual, [Animation Blend Trees](https://docs.unity3d.com/6000.5/Documentation/Manual/class-BlendTree.html) — normalized-time/contact alignment context only; no pack result, accessed 2026-08-16.
- Epic Games, [Animation Sync Groups in Unreal Engine](https://dev.epicgames.com/documentation/unreal-engine/animation-sync-groups-in-unreal-engine) — cycle/foot-placement synchronization context only; no pack result, accessed 2026-08-16.
- Godot Engine stable documentation, [Using AnimationTree](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html) — blend-space, sync-mode, filtering, and missing-track context only; no pack result, accessed 2026-08-16.
- Bevy, [Animation Graph example](https://bevy.org/examples/animation/animation-graph/) — weighted graph blending context only; no pack result, accessed 2026-08-16.
