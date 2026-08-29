# Animation pack evidence appendix: Protofactor Basic Locomotion Animset

> Companion report: [technical evaluation](protofactor-basic-locomotion.md)
>
> Evidence status: **partial** — exact AnimSmith 0.7.0 baseline, contracts, remediation verification, addressability, and bounded advice plus a dated Unity 6000.5.8f1 observation; target-character visual acceptance and Humanoid retarget of the candidates remain unevaluated.
>
> Evaluation date: **2026-08-26**
>
> Current evaluator: **AnimSmith 0.7.0**
>
> Report format: **2**

This appendix preserves the evidence behind the concise technical report. It is intentionally exhaustive about manifests, pipeline stages, readiness, validation profiles, commands, and unknowns. The [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder) remains authoritative.

The current evaluation uses **AnimSmith 0.7.0** (tag `v0.7.0`, repository
revision `461ac8a4f6bb368eb8637471a796f13eeb647140`, binary SHA-256
`01a501999c91d93abfb32b1f48241fccc70914fac27c9a650c31df44262578d8`),
output schema v17, and measurements schema v16. The source inventory contains
179 FBXs and is unchanged. Current evidence covers the complete mechanical and
declared-contract passes, 39 remediation candidates, engine-settings
projections, and sealed source addressability. Dated Unity observations remain
explicitly labelled because no current engine session was run.

## Evaluation scope and provenance

| Field | Value |
|---|---|
| Pack | Local `Animset@BasicLocomotion_PACKAGE.unitypackage`; edition/version not declared in the archive |
| Vendor/source | Protofactor; [current Basic Locomotion product page](https://protofactor.biz/product/animset-basic-locomotion/) |
| Access | Locally held commercial archive inside “Protofactor Ultimate Animation Collection”; user states it was downloaded from Protofactor.biz |
| Price observed | Current Basic Locomotion page: USD 14.99; current [Protofactor Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/): USD 259.99; current [Ultimate Animation Collection Unity listing](https://marketplace.unity.com/packages/3d/animations/ultimate-animation-collection-195459): USD 259.99. That collection listing reports version 1.65 released 2026-08-16. Observed 2026-08-16; none proves the local artifact's edition or purchase terms. |
| Delivered scope | Full local RAR → one Unitypackage → 179 FBX files, including 177 per-motion FBX files, one combined animation FBX, and one skinned reference FBX; materials/textures and Unity metadata also delivered |
| Target game/use | Game-engine use only; no specific game, camera, character, controller, platform, networking model, or quality bar supplied |
| Target engines | Unity 6000.5.8f1 has dated import and headless Playables observations. Current AnimSmith projections are available for Unity Humanoid revision 1, Unreal revision 2, and Godot revision 2; Bevy revision-3 rich addressability is available for generated GLB candidates. Projections are not import, retarget, playback, or visual-acceptance evidence. |
| Target rigs/packs | Delivered Protof-Actor reference only; no project character or other animation pack supplied |
| License evidence | `user-stated`: the archive was downloaded from Protofactor.biz. No license document, receipt, download date, or transaction record is retained with the archive. The current [Protofactor EULA](https://protofactor.biz/end-user-license-agreement/) permits one license owner to use and modify assets in protected published real-time applications while restricting transfer, raw/derived asset resale, and redistribution. The historical terms remain unverified; this is technical due diligence, not legal advice. |
| Source manifest | `<evaluation-workspace>/evidence/logical-asset-manifest.json` |
| Evaluation manifest | Schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1`; external validated taxonomy and profile-selection evidence. Current command envelopes and digests are listed under Reproduction. The immutable V1 manifest is supporting taxonomy evidence, not current 0.7 command output. |

The current vendor Basic Locomotion page advertises 34 animations (12 root-motion and 22 in-place), whereas this local archive contains 177 per-motion files and 70 `_RM` files. The current product page therefore cannot be treated as the manifest for this artifact. The local content was evaluated as an edition-unknown artifact, not as a verified copy of today's SKU. The current Protofactor collection page says the collection contains 23 animsets and more than 2,300 animations, including Basic Locomotion; that is useful collection-scope context but not proof of which constituent packs or versions are present in the local archive.

### Evidence coverage

| Surface | Offered/delivered | Evaluated | Findings | Not evaluated and why |
|---|---:|---:|---:|---|
| Animation files | 179 FBX | 179 | 179 readable; 12 strict time failures; 3 skeleton signatures — identical under the current evaluator | Continuous visual playback and artistic quality for all files |
| Distinct rigs/export variants | 3 signatures | 3 | 56-bone standard (136 files), 73-bone cover/grenade (41), 58-bone reference/combined (2); complete loader-projected hierarchy/rest evidence | Deformation and Avatar retarget quality in engine |
| AnimSmith default lint | 179 | 179 | 167 exit 0; 12 exit 1; 24,186 constant-track notes — the current evaluation reproduces exactly | Default lint lacks game semantics for most checks |
| AnimSmith contract lint | 177 per-motion files | 177 | 58 exit 0; 119 exit 1 under declarations derived from Unity metadata/filename policy — the current evaluation reproduces exactly | Contracts excluded the reference and unsliced combined source; the current evaluation also corrects loop-seam scoring so 93 no-stride/stationary files are recorded not-evaluated (of 111 applicable) instead of a misleading pass/fail |
| Offline visual reports | 179 possible | 9 representative | Coherent static midposes; expected stationary/translating roots; combined-file report is not usable as one gameplay clip | Motion/contact/loop quality cannot be proven from static samples |
| Engine imports/profiles | 1 native Unity route + 4 exact engine-profile probes (current) | 1 import completed (retained); 4/4 profiles run | Current projections are available for Unity Humanoid revision 1, Unreal revision 2, Godot revision 2, and Bevy revision 3; these are not engine execution evidence. | Visual playback, compression, package conflicts, player build; profile advice is not import/load/playback proof |
| Blend/mask/retarget tests | 3 directional rings measured offline | 6 representative Unity samples and 3 two-clip blends; both hand scales measured in 179/179 FBXs; 24/24 in-place ring members gait-anchored (current) | All headless Playables checks passed; raw cross-file phase mismatch and 0.01 hand rest-world scale remain; the current evaluation anchors all 24 in-place members (circular spread ~0.05–0.09) but the candidates are unpromoted | Full 8-way blend spaces, AvatarMask, additive, crossfade, prop attachment, deformation, target-rig retarget, and any engine test of the new candidates |

### Claim legend

Use: `user-stated`, `observed-file`, `observed-animsmith`, `observed-report`,
`observed-engine`, `vendor-stated`, `documentation-stated`, `inferred`, and
`not-evaluated`.

## Evaluation manifest and taxonomy

Manifest schema: `urn:animsmith:skill:animation-pack-evaluation-manifest:1`.

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
| Walk | directional-blend | IP/RM 8-way rings | Common skeleton/timing, direction names, measured phase; medium confidence | Both raw rings have gait-phase findings; the current evaluation anchors the in-place ring (spread 0.6598→0.0724) but the candidate is unpromoted; Unity sampled/blended one representative pair only. |
| Run | directional-blend | IP/RM 8-way rings | Common skeleton/timing, direction names, measured phase; medium confidence | Both raw rings have gait-phase findings; the current evaluation anchors the in-place ring (spread 0.4630→0.0938) but the candidate is unpromoted; Unity sampled/blended one representative pair only. |
| Crouch | directional-blend | IP/RM 8-way rings | Common skeleton/timing, direction names, measured phase; medium confidence | Both raw rings have gait-phase findings; the current evaluation anchors the in-place ring (spread 0.7156→0.0502) but the candidate is unpromoted; Unity sampled/blended one representative pair only. |
| Forward walk/run/fast-run | speed-blend | Two IP/RM sets; three speeds each | Naming and common skeleton; low confidence | Candidate speed blends; no declared set or runtime test. |
| Sprint forward/left/right | directional-blend | Two IP/RM sets; three directions each | Naming and common skeleton; low confidence | Candidate directional blends; no declared set or runtime test. |

The companion report's [runtime-set table](protofactor-basic-locomotion.md#runtime-sets-and-authored-motion) retains the exact 24 paired members, cycle durations, per-direction root speeds, and cross-member speed ratios; this appendix preserves grouping and validation evidence without duplicating that table.

### Pipeline-stage coverage

| Stage | Coverage state | Evidence / remaining gate |
|---|---|---|
| Acquire | `partially-evaluated` | Local archive and current vendor/license URLs identified; historical transaction/edition not retained. |
| Preserve raw | `evaluated-clean` | Immutable RAR retained; extraction and generated outputs are separate. |
| Inspect | `evaluated-finding` | All 179 FBXs inspect/measure/lint; 12 negative-time files, three skeleton signatures, complete loader-projected hierarchy/rest evidence, and 0.01 hand scales. |
| Segment | `partially-evaluated` | 177 atomic per-motion files; combined take has no complete authoritative ranges. |
| Root motion | `partially-evaluated` | 70 counterpart pairs measured; `root_trajectory` sampling covers all 179 clips (71 move >1 cm, 107 stationary, 21 show >1° yaw; yaw `heading_axis` `positive_y` on 178/178) — a sampled grid fact, not continuous-curve or engine-extraction proof; controller ownership remains incomplete. |
| Conform | `partially-evaluated` | Current slicing succeeds (36→0 time-monotonic errors), and gait anchoring measures the heading axis as vertical (`positive_y`) and anchors all 24 selected in-place clips (spread ~0.05–0.09). The GLB candidates are unpromoted pending Humanoid-retarget and visual acceptance; RM anchoring was not attempted. |
| Validate | `partially-evaluated` | Exhaustive mechanical and declared semantic checks complete under the current evaluator; one dated Unity import plus four current projection/addressability profiles; no visual acceptance. Loop-seam applicability is 111/66 and evaluability is 84 complete/93 not_evaluated. |
| Optimize | `partially-evaluated` | Constant-track pruning trialed but not approved. |
| Export | `partially-evaluated` | Unity imported native package; no generated production export or player build. |
| Gate/report | `evaluated-clean` | Commands, manifests, digests, primary report, and appendix retained. |

### Readiness evidence by clip set

Use the repository's [six-level readiness ladder](../game-ready-clips.md#the-readiness-ladder); this table reports evidence at those levels rather than redefining them.

| Role or set | File-ready / clip-ready | Set-ready / rig-use | Runtime / acceptance boundary |
|---|---|---|---|
| Idles/holds (14) | 14/14 mechanically clean with constant-track notes; six provisional loop declarations have seam findings, including two with linear/closure findings. | No loop-intent or mask contract. | Unity import covered; visual looping and layered use untested. |
| Continuous locomotion (68) | 68/68 mechanically clean; the 24 clearly cyclic in-place ring files are the defensible loop subset, and 22 have raw strict closure/seam-derivative findings. | Six 8-way sets share timing; all six raw rings fail phase target; the current evaluation emits 24/24 anchored in-place candidates for this rig (unpromoted). | Unity sampled six representatives and blended three pairs (retained), not full rings, visual contacts, or the current candidates. |
| Locomotion transitions (60) | 10/60 have negative-time errors until sliced; many of the 25 loop flags are semantically suspect for one-shots. | No authoritative transition chains; 56/73-bone boundaries occur. | Curated one-shots only until crossfade/interruption tests. |
| Airborne/traversal (17) | Mechanically clean; four obstacle files and falling are provisionally loop-marked despite likely one-shot/hold semantics. | No trajectory/contact/environment chain. | Controller and environment integration untested. |
| Actions/interactions (18) | 2/18 have negative-time errors until sliced; eight likely one-shot grenade actions are loop-marked. | Full-body tracks; no additive/mask/contact contract. | Unity import only; prop, IK, recovery, and visual result untested. |
| Three in-place 8-way rings | Raw phase spreads 0.660/0.463/0.716; current gait anchoring succeeds for 24/24 members (spreads 0.0724/0.0938/0.0502). | Candidates are unpromoted pending Humanoid-retarget and visual acceptance; keep runtime offsets or artist-aligned exports as the shipping fallback. | Full blend spaces and loop wraps need visual target-character review. |
| Three root-motion 8-way rings | Raw phase spreads exceed 0.15. | AnimSmith's fail-closed trajectory policy still prevents an unsafe cyclic rewrite; no root-motion (RM) gait-anchor trial was attempted under the current evaluator either. | Runtime phase offsets or artist/root-preserving tooling required. |

### Validation-profile status

| Validation profile | Selection | Result / next evidence |
|---|---|---|
| Marketplace intake | `selected` — `user-required` | File/tool intake complete; provenance partial. |
| Blended locomotion | `selected` — `observed-pack-capability` | Six raw phase findings; current gait anchoring succeeds for all 24 in-place candidates, which remain unpromoted pending full runtime-ring and visual gates. |
| Root-motion controller | `selected` — `observed-pack-capability` | Pair translation and root trajectory are measured (71 moving/107 stationary/21 yawing), with a `positive_y` heading axis; extraction proof and controller ownership outstanding. |
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
- One per-motion filename contains `Standingt`. Separately, the bundled combined-take animation list—not the exact per-motion FBX identifiers in the primary report—contains `SpintTurnRight`, `WalkForwadRight`<!-- vendor-id -->, `runForward2`, and `Runbackwards`. These source disagreements increase automation cost.

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

Untouched **Unity import and headless evaluation** completed in Unity 6000.5.8f1 on 2026-08-17 and is retained unchanged in this current evaluation because the source archive is byte-identical. The package imported into a disposable project with exit 0. Unity processed 179 FBXs overall; the motion directory contains 178 FBXs (177 individual clips plus the combined take), all configured as Humanoid with valid source-avatar references. Six representative in-place/root-motion clips evaluated through `AnimationClipPlayable`, and three representative walk/run/crouch pairs evaluated through a 50/50 `AnimationMixerPlayable`. All nine checks completed without exceptions.

Unity logged one material pack finding: `Protof-Actor@BasicLocomotionAnimset.fbx`, the combined take, reports a copied-avatar hierarchy mismatch for the Hips transform. The 177 individual motion clips remain the recommended source route. Headless evaluation establishes importer and Playables compatibility, not visual motion or blend quality.

Untouched **offline** loading is strong: AnimSmith inspected and measured all 179 FBX files. Nine representative offline HTML reports were rendered at frame 0 and an injected midpoint and visually reviewed. Idle, walk, root-motion walk, side run, jump, landing, cover, and grenade samples showed coherent static humanoid poses without gross explosions. Root-motion walk showed a trajectory while its in-place partner remained stationary. The negative-time cover sample surfaced its errors. The 319.667-second combined take produced a visually dense, incoherent trajectory/transition report as expected for many actions presented as one clip.

Neither the static reports nor the headless Unity probe proves artistic motion quality, planted contacts, loop smoothness, full blend-space behavior, Avatar masks, target-mesh deformation, engine attachment behavior, controller response, compression behavior, or player-build correctness.

### Untouched AnimSmith findings

All rows below are current AnimSmith 0.7.0 results captured on 2026-08-26.

| Finding or coverage gap | Affected scope | User-visible effect | Evidence |
|---|---|---|---|
| Non-monotonic negative-time keys at −0.033333335 s on translation/rotation/scale of `root_CoverUnarmedAnimset` | 12 files; 36 errors | Strict import/processing may reject or mishandle pre-roll; timelines do not start cleanly at zero | observed-animsmith: `baseline-summary.json` |
| Baked constant tracks | 179 files; 24,186 notes; 99–192 per file, median 137 | Source bloat and dense channel coverage; runtime cost unmeasured | observed-animsmith |
| Unity-declared loop closure failures | 58 of 111 declared loops; 84 errors | Possible pose pops or contact mismatch at wrap | observed-animsmith; declaration partly observed-file |
| Unity-declared loop seam derivative failures | Of 111 declared loops, 104 files fail linear seam velocity and 108 fail angular seam velocity | Possible visible speed/rotation discontinuity at wrap; many declarations appear semantically questionable | observed-animsmith |
| Root-motion declaration reports stationary root | 14 of 70 `_RM` files | Current check under-reports rotation-only or low-displacement root motion | observed-animsmith; tool/check limitation, not automatically a pack defect |
| Missing semantic coverage under baseline config | 16 of 26 selected checks commonly non-applicable | Exit 0 without explicit declarations does not establish game readiness | observed-animsmith |
| Generic embedded clip identity | 179 files expose `Take 001` | Shared configs cannot naturally express distinct cross-file clip contracts by embedded name | observed-file, observed-animsmith |
| Hand rest-world scale differs from a 1.0 attachment policy | Both exact hand nodes in 179/179 FBXs; 358 warnings; measured 0.0099999966–0.0100000017 | An uncompensated prop/socket may inherit approximately 0.01 scale and appear about 100× too small | observed-animsmith; loader-projected evidence, engine attachment untested |
| Root trajectory measured on every clip | 179/179 clips; 71 move more than 1 cm horizontally, 107 are stationary (≤1 cm), 21 carry more than 1° of yaw travel | Sampled-grid regression facts, not continuous-curve or engine root-motion-extraction proof; **no movement-ownership axis is inferred from these numbers** | observed-animsmith |
| Yaw heading axis resolves vertical | `heading_axis` = `positive_y` on 178/178 measured clips | Supplies the current gait-anchor heading basis | observed-animsmith |
| Loop-seam availability separates applicability from evaluability | Contract pass: 111/177 files seam-applicable (66 not_applicable); of the applicable files, 84 complete / 93 not_evaluated | No-stride/stationary clips are recorded as not evaluated instead of being mislabelled pass or fail | observed-animsmith |

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

### Current remediation results

| Source issue | Current operation and declarations | Current result | Verification | Remaining caveat |
|---|---|---|---|---|
| Negative-time keys in 12 files | `transform --slice 0:<Unity lastFrame/30> --fps 30`; range derives from each delivered Unity clip declaration | 12/12 transforms succeeded; all 36 time-monotonic errors were removed | Current inspect, measure, lint, and fix dry-run checks completed | The declared frame-range policy removes one boundary frame; review transition timing in-engine. |
| Misaligned 8-way gait phases | `transform --gait-anchor` on 24 in-place walk/run/crouch files with per-file loop, in-place, and humanoid declarations | 24/24 transforms succeeded. Circular phase spread: Crouch 0.7156245→0.0501911, Run 0.4630161→0.0938395, Walk 0.6597812→0.0724415 | Current candidates and measurements were captured for every member | Candidates remain unpromoted until Humanoid-retarget and visual acceptance; RM anchoring was not attempted. |
| Baked constant tracks | `transform --prune-constant-tracks` on representative walk, cover, and combined files | 3/3 candidates emitted | Mechanical verification completed | Runtime equivalence and transition behavior are not proven; do not adopt this optimization without project-specific testing. |

The current evaluator measures the vertical yaw heading axis as `positive_y`, so
all selected in-place rings can be anchored without changing root-motion
members. AnimSmith does not repair genuine loop pose or velocity seams, retarget
a rig, author additive motion, fix contacts, or choose project movement
ownership.

### Current conclusion

The declared slice makes the 12 strict-time failures mechanically usable and
all 24 in-place gait-ring candidates meet the 0.15 phase-alignment target. That
does not make the assets production-certified: the candidates still need a
Humanoid-retarget and visual gate, 22/24 raw in-place clips retain at least one
strict loop closure or seam finding, and the pruning trial remains unsuitable
for adoption without runtime-equivalence evidence.

## Engine procedures and evidence

### Import configuration

Native Unity delivery was selected because the source is a Unitypackage and all metadata declares Unity Humanoid animation. Unity `6000.5.8f1 (5cb7df797b7d)` on Windows 11 created a disposable project, imported the package, and exited 0. The import performed 199 asset imports and exposed 177 humanoid animation clips. The retained probe reports 178 Humanoid model importers in the motion directory, 178 valid source-avatar references, 177 human-motion clips, 6/6 representative sampling passes, and 3/3 representative blend passes.

Remaining Unity import gates are: retargeting each skeleton signature to the actual project character; reviewed loop/root-transform settings; before/after import of the 12 sliced clips; compression effects on contacts, fingers, prop bones, and seams; full blend controllers; masks/IK; and a player build.

### Runtime playback and root motion

Headless Playables sampling and pair mixing are evaluated; visual playback and controller behavior are not. The file-level pair inventory is favorable: all 70 `_RM` files have a same-skeleton non-RM partner, durations match for every pair, and frame counts match for 68. `GoOutOfCoverRightStanding_RM` is one frame longer than its partner; `IdleTakeCoverCrouchingToIdleStanding_RM` is one frame shorter.

The current root-motion speed contract is incomplete for turns. Four crouch turns, two run U-turns, four 90°/180° turns, two walk U-turns, and two cover strafes are `_RM`-labeled but fall below the default 0.5 m/s horizontal-speed threshold. These may contain rotational root motion or intentional low displacement. Validate translation and yaw separately in Unity rather than treating the check as a definitive defect.

Test root motion against controller collision, slopes/steps, capsule reconciliation, animation interruption, and the project's networking/rollback policy. For in-place motion, verify authored foot speed against controller speed; this evaluation did not derive stride-matched gameplay velocities.

### Current engine-profile evidence

The current projections are static settings or sealed-addressability evidence,
not engine import, retarget, playback, or visual acceptance.

| Profile | Engine/version | Current projection | Remaining gate |
|---|---|---|---|
| `unity-humanoid` | Unity 6000.3, revision 1 | Import advice available | Verify the projected importer settings on the target character and project. |
| `unreal` | Unreal Engine 5.8, revision 2 | Import advice available | Run an Unreal import, retarget, graph, and playback test. |
| `godot` | Godot 4.7, revision 2 | Import advice available | Run a Godot conversion/import, retarget, graph, and playback test. |
| `bevy` | Bevy 0.19.0, revision 3 | Rich addressability available with 64-bit target UUIDs | Verify target survival, graph wiring, and playback in the declared loader environment. |

### Unity headless importer observation (2026-08-21)

A disposable Unity 6000.5.8f1 project inspected
`ModelImporterClipAnimation` for a 120-clip sample spanning all eight collection
packs, including 24 root/root-adjacent files from this pack:

| Variant | Clips | `lockRootRotation` true | `lockRootHeightY` true | `lockRootPositionXZ` true |
|---|---:|---:|---:|---:|
| In-place (non-`_RM`) | 84 | 84 | 84 | 83 |
| Root-motion (`_RM`) | 36 | 36 | 28 | 5 |

The observed importer policy is per-variant and axis-specific. Rotation is baked
for every sampled clip; XZ is baked for nearly every in-place clip and extracted
for most root-motion clips. Two in-place files without explicit clip definitions
use Unity's `defaultClipAnimations`. This is a dated headless-import observation,
not continuous visual or gameplay acceptance.

### GLB candidate import into Unity (2026-08-21)

All 134/134 AnimSmith 0.7.0 gait-anchored GLB candidates across the eight-pack collection — including this pack's 24 anchored in-place walk/run/crouch candidates (see AnimSmith remediation evidence above) — were staged into a separate, **new** Unity 6000.5.8f1 project using `com.unity.cloud.gltfast` 6.9.0, because Unity has no native GLB importer; the retained eight-pack project above was not modified. Result: 134/134 files staged produced assets, 134/134 produced exactly one Unity `AnimationClip`, and every clip is non-legacy and non-empty.

**Limit, stated plainly:** glTFast imports glTF animation as a **Generic** clip and does not reconstruct a Humanoid Avatar. This proves the candidates load and yield one well-formed clip in Unity; it does **not** test the Humanoid retarget path this pack actually uses, and it is not a visual or gameplay acceptance test. The 24 gait-anchored candidates for this pack therefore remain **unpromoted**, unchanged from the AnimSmith remediation evidence above.

### Performance and packaging

No engine import size, runtime memory, decompression CPU, build size, or platform performance was evaluated. Source FBX files carry many constant channels. AnimSmith can remove many of them in sampled GLBs, but byte reduction across FBX→GLB is not a runtime-performance measurement and the trial lacks an acceptable equivalence proof. Retain the untouched coverage until a target-runtime measurement and channel-coverage gate justify pruning.

## Blending, masking, and gameplay caveats

### Locomotion, sync, and transitions

The in-place walk, run, and crouch directional rings each contain eight files
with equal duration and frame count within the ring. Raw circular phase spreads
are 0.660 for walk, 0.463 for run, and 0.716 for crouch, so direct normalized-time
blending risks mixing unlike foot phases. Current gait anchoring succeeds for all
24 members and reduces those spreads to 0.0724415, 0.0938395, and 0.0501911.

The candidates remain unpromoted pending Humanoid-retarget and visual acceptance,
and 22/24 raw clips still fail at least one strict loop closure or seam derivative
check. Anchoring does not repair loop seams. Test cardinal, diagonal, and
intermediate weights, phase wrap, acceleration, stops, turns, and idle/jump
transitions. Review the delivered Unity loop declarations first because they
also mark one-shots such as grenade throws and obstacle passes as loops.

### Upper/lower-body masking and additive use

Runtime masking is not evaluated. The files carry full-body baked transform tracks and the Unity metadata's explicit clips have `hasAdditiveReferencePose: 0`; there are no dedicated upper-body-only or additive files. Unity Humanoid AvatarMasks may allow a grenade or aim clip to override the upper body while locomotion drives the lower body, but the mask boundary, spine continuity, pelvis ownership, arm reach, hand/prop alignment, and root behavior must be tested.

Recommended default: keep pelvis/root and legs in the locomotion layer, begin the action mask above a project-chosen spine boundary, and test several blend weights and transition times. If the action meaningfully shifts pelvis, center of mass, or supporting feet, a pure upper-body mask will likely look wrong and a full-body authored transition is preferable. No IK target bones are evident in the standard 56-bone hierarchy; the 73-bone cover/grenade variant adds cover/rocket/nub nodes but does not establish a portable hand/foot IK contract.

### Game-type caveats

| Game/system context | Suitability | Caveat or required work | Evidence |
|---|---|---|---|
| Third-person action prototype | Good candidate | Complete engine/character test; curate loops and transitions | observed-file, observed-report, inferred |
| Controller-driven in-place locomotion | Good candidate with conditions | Current gait anchoring emits 24 in-place candidates; validate them in engine and visually before use, or apply runtime/DCC phase alignment; tune controller speed; inspect foot slide and seam wraps | observed-animsmith |
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
| 8-way in-place walk ring | 56-bone standard | In-place by filename; speed below 0.5 m/s | Equal duration/frames; current anchoring reduces phase spread 0.6597812→0.0724415 | Not engine/visually tested | Anchored candidate, unpromoted | observed-animsmith |
| 8-way in-place run ring | 56-bone standard | In-place by filename; speed below 0.5 m/s | Equal duration/frames; current anchoring reduces phase spread 0.4630161→0.0938395 | Not engine/visually tested | Anchored candidate, unpromoted | observed-animsmith |
| 8-way in-place crouch ring | 56-bone standard | In-place by filename; speed below 0.5 m/s | Equal duration/frames; current anchoring reduces phase spread 0.7156245→0.0501911 | Not engine/visually tested | Anchored candidate, unpromoted | observed-animsmith |
| Standard locomotion ↔ cover/grenade clips | 56-bone vs 73-bone signatures | Mixed; filename policy | Per-transition timing not evaluated | Humanoid retarget/masks not tested | Engine-config candidate, not exact-skeleton direct | observed-animsmith, inferred |
| Per-motion files ↔ combined take | 56/73-bone motions vs 58-bone combined/reference | Mixed in combined source | Combined segmentation incomplete/inconsistent | Not tested | Prefer per-motion assets; combined source unknown | observed-file, observed-animsmith |

### Cross-pack or target-rig compatibility

| Pack/rig pair | Skeleton/retarget | Scale/axes | Root policy | Timing/blend | Style/semantics | Overall | Evidence |
|---|---|---|---|---|---|---|---|
| Pack ↔ supplied Protof-Actor | Unity metadata references the actor Avatar for 177 per-motion files | File import flags use scale 1; loader-projected hand rest-world scale is about 0.01; attachment behavior untested | `_RM`/in-place convention present | Runtime not tested | Intended source actor | Engine-config candidate | observed-file, observed-animsmith |
| Pack ↔ project character | No character supplied; Humanoid is promising but insufficient | Not evaluated | Project policy unknown | Not evaluated | Not evaluated | Unknown | not-evaluated |
| Pack ↔ another animation pack | No comparison pack supplied | Not evaluated | Not evaluated | Not evaluated | Not evaluated | Unknown | not-evaluated |

A meaningful future cross-pack report should compare at least: humanoid role mapping and rest pose, scale/axes, root ownership and translation/yaw conventions, sample rates and loop semantics, gait/contact phase, action semantics, style, AvatarMask/additive assumptions, and runtime blends on the same target character.

## Limitations and unknowns

1. No target game, engine project, character, controller, camera, platform, frame budget, networking policy, or artistic quality bar was supplied; suitability conclusions are deliberately generic.
2. A dated Unity package import and headless sampling/pair-blending pass exists, but no visual full-ring controller, target-character retarget, masks, root-motion controller, compression comparison, or player build was run. Current Unity, Unreal, and Godot settings projections and Bevy addressability are tool-side evidence only, not engine execution.
3. Static report samples and headless Playables evaluation cannot establish motion quality, planted contacts, deformation, loop perceptibility, full blend-space quality, masking, or compression behavior.
4. Root-motion classification partly relies on the `_RM` filename convention; speed-only checks do not characterize rotational or low-displacement root motion.
5. Contract loop declarations come from Unity metadata, which appears to over-label one-shots. Counts should not be interpreted as 108 visually bad gameplay cycles.
6. Cross-file phase spread was aggregated from per-file AnimSmith gait measurements because the current embedded clip identities/config model cannot directly express these groups across files.
7. AnimSmith's complete FBX source hierarchy/rest data is loader-projected evidence after metre/Y-up adjustment and inheritance compensation, not a raw FBX transform-stack dump. The 0.01 hand-scale finding still needs a real engine attachment test.
8. The constant-pruning sample is not approved: source/output format differs, `diff` reports many deltas, and emitted per-property coverage is not currently available from the evaluator.
9. No malware/security audit beyond archive path/structure inspection was performed; no executables or scripts were found in the logical pack content.
10. No current vendor download was acquired, so the local artifact cannot be equated to the 2026 product listing.
11. No full artistic review of all 177 motions was conducted; nine representative offline reports were sampled.
12. Root trajectory (movement over 1 cm, stationary, yaw over 1°) and the `positive_y` heading axis are sampled-grid regression facts on 179/179 clips, not continuous-curve or engine root-motion-extraction proof; no movement-ownership axis (which side owns XZ/Y/yaw translation) is inferred from them.
13. The 24 gait-anchored GLB candidates produced under the current evaluator loaded only as Generic glTF clips, with no Humanoid-retarget or visual review; treat them as unpromoted pending that gate.
14. The integration recipe's `owner=validate-per-axis` step directs the reader to validate root-motion ownership axis by axis rather than assume it. The observed Unity importer locks bake root rotation on every sampled root-motion clip, so animation cannot be assumed to own root-motion yaw. The step is not a per-axis `movement_owner_xz` / `movement_owner_y` / `movement_owner_yaw` declaration, and no such declaration is derived from measured travel in this refresh. Measured root displacement and yaw are recorded as sampled facts only; choosing the per-axis owner remains a project and engine decision.
15. A direct Unity 6000.5.8f1 headless sample of 120 clips observed baked root rotation, baked XZ for nearly every in-place clip, and extracted XZ for most root-motion clips. This is sampled importer evidence, not visual or gameplay acceptance.
16. The 134/134 candidate Unity import, including this pack's 24 candidates, proves glTFast produces one Generic `AnimationClip` per candidate. It does not exercise Humanoid retarget or promote the candidates.

## Changes between AnimSmith versions

| Evaluator | Change from the preceding evaluated state |
|---|---|
| AnimSmith 0.7.0 | Revalidated the 179-FBX baseline, 177 declared contracts, 12 slice candidates, 24 gait candidates, 3 pruning trials, and current engine projections under output v17 / measurements v16. Current conclusions are in the ordinary sections above. |
| AnimSmith 0.4.1 | Reproduced the evaluated 0.4.0 measurements and transforms for this corpus; its unrelated rest-bind and diagnostic fixes did not change the pack conclusion. |
| AnimSmith 0.4.0 | Added the root-trajectory, channel-coverage, and engine-profile evidence used to refine the integration recipe; unsafe RM gait anchoring remained refused. |
| AnimSmith 0.3.x | Established the initial baseline and declared-remediation trials. Those results are superseded by the current evaluation. |

## Reproduction

### Current AnimSmith reproduction (2026-08-26)

The same source corpus was re-inventoried and rerun with `animsmith 0.7.0`, exact tag `v0.7.0`, commit `461ac8a4f6bb368eb8637471a796f13eeb647140`, binary SHA-256 `01a501999c91d93abfb32b1f48241fccc70914fac27c9a650c31df44262578d8`, output schema v17, and measurements schema v16. The current results below are attributable to this evaluator; dated engine observations remain labelled with their capture dates.

| Current external evidence | SHA-256 | Result |
|---|---|---|
| Source inventory | `c6cc4d541fa2cb8e4f3e14c283d5b925f83957db35bfa079309f250cdaf101ba` | 179 FBXs; source unchanged |
| Exhaustive baseline command envelope | `723ca37c489d34d525ab5c9ea681508cb273f5886da338c1aa9dfc6d79d74b2b` | 179 inspected/measured; current findings recorded |
| Declared-contract command envelope | `6b643ead6c923997e8df740e81ef85d0772c10ab71a01301bd8d3e8e7cd77c1f` | 177 files; 58 pass / 119 fail |
| Remediation command envelope | `a4ccc86f5932b1459102f382457f24ac2915d6d6d1d9a4b233002318eb9d6ba5` | 39/39 transforms completed and verified |
| 0.7 supplemental projections | `1f804804fb8a623cd9c435c9e231da0a4ccf0a2287f1281b6b43f0098847aa95` | 39 addressability V1 + rich V2 pairs; Unity v1, Unreal v2, Godot v2 advice available |

Rich Bevy addressability used the exact revision-3 `0.19.0` / `gltf-asset-loader` tuple with a declared bare extension-handler environment, animation feature enabled, animations loaded, and 64-bit target UUIDs. Advice used the preserved Unity Humanoid revision-1 settings vocabulary and the current bounded Unreal/Godot revision-2 projection vocabularies. These documents predict declared settings and sealed source addressability only; they do not execute an engine or prove runtime survival.

## Sources

- Local source archive and extracted Unity metadata — private local artifact identified above, accessed 2026-08-16.
- Protofactor, [Animset: Basic Locomotion](https://protofactor.biz/product/animset-basic-locomotion/) — current product description, counts, formats, Unity compatibility, and price, accessed 2026-08-16.
- Protofactor, [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/) — current collection price, constituent-pack list, and advertised aggregate count, accessed 2026-08-16.
- Protofactor, [End User License Agreement](https://protofactor.biz/end-user-license-agreement/) — current one-owner, protected-real-time-application, modification, transfer, and redistribution terms; not evidence of the local transaction's governing terms, accessed 2026-08-16.
- Unity Asset Store, [Ultimate Animation Collection](https://marketplace.unity.com/packages/3d/animations/ultimate-animation-collection-195459) — current collection version/date, price, license tier, and original Unity version, accessed 2026-08-16.
- AnimSmith [v0.7.0 release](https://github.com/mmannerm/animsmith/releases/tag/v0.7.0) — repository revision `461ac8a4f6bb368eb8637471a796f13eeb647140`, binary SHA-256 `01a501999c91d93abfb32b1f48241fccc70914fac27c9a650c31df44262578d8`, accessed 2026-08-26.
- AnimSmith public issue [#165](https://github.com/mmannerm/animsmith/issues/165) — current roadmap guardrails for automatic animation rewrites, accessed 2026-08-16.
- AnimSmith public issue [#401](https://github.com/mmannerm/animsmith/issues/401) — constant-track pruning property-scoped policy limitation; verified open, accessed 2026-08-21.
- Unity 6.5 Manual, [Animation Blend Trees](https://docs.unity3d.com/6000.5/Documentation/Manual/class-BlendTree.html) — normalized-time/contact alignment context only; no pack result, accessed 2026-08-16.
- Epic Games, [Animation Sync Groups in Unreal Engine](https://dev.epicgames.com/documentation/unreal-engine/animation-sync-groups-in-unreal-engine) — cycle/foot-placement synchronization context only; no pack result, accessed 2026-08-16.
- Godot Engine stable documentation, [Using AnimationTree](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html) — blend-space, sync-mode, filtering, and missing-track context only; no pack result, accessed 2026-08-16.
- Bevy, [Animation Graph example](https://bevy.org/examples/animation/animation-graph/) — weighted graph blending context only; no pack result, accessed 2026-08-16.
