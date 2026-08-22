# Animation pack evidence appendix: Protofactor Basic Locomotion Animset

> Companion report: [technical evaluation](protofactor-basic-locomotion.md)
>
> Evidence status: **partial** — exhaustive file and AnimSmith 0.4.0 coverage, a retained Unity 6000.5.8f1 headless probe, 0.4.0 advice-only engine profiles for Unity/Unreal/Godot/Bevy now corrected by a direct Unity 6000.5.8f1 observation of import-advice root-lock declarations, and a headless Unity glTFast import of all 134 collection-wide gait-anchored GLB candidates (including this pack's 24); target-character visual acceptance and Humanoid retarget of the new candidates remain unevaluated.
>
> Evaluation date: **2026-08-21**
>
> Report format: **1**

This appendix preserves the evidence behind the concise technical report. It is intentionally exhaustive about manifests, pipeline stages, readiness, validation profiles, commands, and unknowns. The [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder) remains authoritative.

**2026-08-21 evaluator refresh.** This pass reran the pack under **AnimSmith 0.4.0** (tag `v0.4.0`, repository revision `6b37ad636b198ef8ff47fadbf6a3a51eb1a27c8e`, binary SHA-256 `fd1eee57407aa02db88763d144389a7f5104204c40ddfbb28eb5885ca8cd54c6`), captured 2026-08-21 on Linux WSL2 x86_64 with rustc 1.97.1, command envelope schema v10 and measurements schema v15. A fresh `inventory_pack.py` reconciliation over the same logical-asset root reproduced the published manifest **exactly**: 0 paths added, 0 paths removed, 0 content changed, 179 FBX, logical-manifest SHA-256 `5bec4f741c39f232c79f4c841fc0eb580589f3868b614610cb6ff15a59a0b34b` unchanged from the Reproduction section below. The archive SHA-256 recorded there (`6f821f56f84339ea1eb6fcaa97e3c70d4a38dd84c413012847f026748dff185f`) re-verified, and the retained Unitypackage payload was independently rehashed live and also matched its recorded digest. **This is therefore a pure evaluator-version refresh, not an asset-revision evaluation** — every fact below that is unchanged from 0.3.0 is a stability result, not a re-confirmation of a different artifact.

What changed under 0.4.0: gait anchoring now succeeds where 0.3.0 refused (see AnimSmith remediation evidence below); root trajectory is now measured on all 179 clips including a yaw heading axis; loop-seam contract scoring now separates applicability from evaluability; and four exact engine profiles (Unity `unity-humanoid`, Unreal, Godot, Bevy) produced advice/refusal/addressability evidence (see Engine procedures and evidence below). What stayed the same: the mechanical baseline (24,186 constant-track notes, 36 time-monotonic errors across 12 files), the contract pass/fail split (58 exit 0 / 119 with findings across 177 files), the manifest taxonomy, and the retained Unity 6000.5.8f1 import/Playables probe, which keeps its original 2026-08-17 date and attribution because the source is byte-identical.

## Evaluation scope and provenance

| Field | Value |
|---|---|
| Pack | Local `Animset@BasicLocomotion_PACKAGE.unitypackage`; edition/version not declared in the archive |
| Vendor/source | Protofactor; [current Basic Locomotion product page](https://protofactor.biz/product/animset-basic-locomotion/) |
| Access | Locally held commercial archive inside “Protofactor Ultimate Animation Collection”; user states it was downloaded from Protofactor.biz |
| Price observed | Current Basic Locomotion page: USD 14.99; current [Protofactor Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/): USD 259.99; current [Ultimate Animation Collection Unity listing](https://marketplace.unity.com/packages/3d/animations/ultimate-animation-collection-195459): USD 259.99. That collection listing reports version 1.65 released 2026-08-16. Observed 2026-08-16; none proves the local artifact's edition or purchase terms. |
| Delivered scope | Full local RAR → one Unitypackage → 179 FBX files, including 177 per-motion FBX files, one combined animation FBX, and one skinned reference FBX; materials/textures and Unity metadata also delivered |
| Target game/use | Game-engine use only; no specific game, camera, character, controller, platform, networking model, or quality bar supplied |
| Target engines | Broad matrix includes Unity, Unreal Engine, Godot, and Bevy. Unity 6000.5.8f1 import and headless Playables probes completed (2026-08-17, retained). 0.4.0 (2026-08-21) added exact engine-profile advice: Unity `unity-humanoid` import-advice `available`; Unreal and Godot both return a typed refusal `profile_settings_unmodeled`; Bevy addressability inventories a generated GLB candidate and predicts its selector. These profiles are advice/refusal/inventory evidence only — not a new import, retarget, or visual/runtime acceptance pass. |
| Target rigs/packs | Delivered Protof-Actor reference only; no project character or other animation pack supplied |
| License evidence | `user-stated`: the archive was downloaded from Protofactor.biz. No license document, receipt, download date, or transaction record is retained with the archive. The current [Protofactor EULA](https://protofactor.biz/end-user-license-agreement/) permits one license owner to use and modify assets in protected published real-time applications while restricting transfer, raw/derived asset resale, and redistribution. The historical terms remain unverified; this is technical due diligence, not legal advice. |
| Source manifest | `<evaluation-workspace>/evidence/logical-asset-manifest.json` |
| Evaluation manifest | `evidence/animsmith-0.3.0/evaluation-manifest.json`; schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1`; taxonomy/profile-set version 1; SHA-256 `3cc3922dc7b4b06db59643f366eab2844f4490334868ea5a2c26bd1926000cd4`. A parallel `evidence/animsmith-0.4.0/` evidence tree retains the same validated taxonomy: manifest content is unchanged because source identity did not change. |

The current vendor Basic Locomotion page advertises 34 animations (12 root-motion and 22 in-place), whereas this local archive contains 177 per-motion files and 70 `_RM` files. The current product page therefore cannot be treated as the manifest for this artifact. The local content was evaluated as an edition-unknown artifact, not as a verified copy of today's SKU. The current Protofactor collection page says the collection contains 23 animsets and more than 2,300 animations, including Basic Locomotion; that is useful collection-scope context but not proof of which constituent packs or versions are present in the local archive.

### Evidence coverage

| Surface | Offered/delivered | Evaluated | Findings | Not evaluated and why |
|---|---:|---:|---:|---|
| Animation files | 179 FBX | 179 | 179 readable; 12 strict time failures; 3 skeleton signatures — identical under 0.4.0 | Continuous visual playback and artistic quality for all files |
| Distinct rigs/export variants | 3 signatures | 3 | 56-bone standard (136 files), 73-bone cover/grenade (41), 58-bone reference/combined (2); complete loader-projected hierarchy/rest evidence | Deformation and Avatar retarget quality in engine |
| AnimSmith default lint | 179 | 179 | 167 exit 0; 12 exit 1; 24,186 constant-track notes — 0.4.0 reproduces exactly | Default lint lacks game semantics for most checks |
| AnimSmith contract lint | 177 per-motion files | 177 | 58 exit 0; 119 exit 1 under declarations derived from Unity metadata/filename policy — 0.4.0 reproduces exactly | Contracts excluded the reference and unsliced combined source; 0.4.0 also corrects loop-seam scoring so 93 no-stride/stationary files are recorded not-evaluated (of 111 applicable) instead of a misleading pass/fail |
| Offline visual reports | 179 possible | 9 representative | Coherent static midposes; expected stationary/translating roots; combined-file report is not usable as one gameplay clip | Motion/contact/loop quality cannot be proven from static samples |
| Engine imports/profiles | 1 native Unity route + 4 exact engine-profile probes (0.4.0) | 1 import completed (retained); 4/4 profiles run | 179 FBXs processed; 177 humanoid clips available; combined FBX copied-avatar hierarchy mismatch. 0.4.0: `unity-humanoid` advice `available` exit 0; Unreal/Godot typed refusal `profile_settings_unmodeled` exit 1; Bevy addressability exit 0 on a generated GLB (1 clip, selector `Animation0`, 0 findings) | Visual playback, compression, package conflicts, player build; profile advice is not import/load/playback proof |
| Blend/mask/retarget tests | 3 directional rings measured offline | 6 representative Unity samples and 3 two-clip blends; both hand scales measured in 179/179 FBXs; 24/24 in-place ring members gait-anchored (0.4.0) | All headless Playables checks passed; raw cross-file phase mismatch and 0.01 hand rest-world scale remain; 0.4.0 anchors all 24 in-place members (circular spread ~0.05–0.09) but the candidates are unpromoted | Full 8-way blend spaces, AvatarMask, additive, crossfade, prop attachment, deformation, target-rig retarget, and any engine test of the new candidates |

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
| Walk | directional-blend | IP/RM 8-way rings | Common skeleton/timing, direction names, measured phase; medium confidence | Both raw rings have gait-phase findings; 0.4.0 anchors the in-place ring (spread 0.6598→0.0724) but the candidate is unpromoted; Unity sampled/blended one representative pair only. |
| Run | directional-blend | IP/RM 8-way rings | Common skeleton/timing, direction names, measured phase; medium confidence | Both raw rings have gait-phase findings; 0.4.0 anchors the in-place ring (spread 0.4630→0.0938) but the candidate is unpromoted; Unity sampled/blended one representative pair only. |
| Crouch | directional-blend | IP/RM 8-way rings | Common skeleton/timing, direction names, measured phase; medium confidence | Both raw rings have gait-phase findings; 0.4.0 anchors the in-place ring (spread 0.7156→0.0502) but the candidate is unpromoted; Unity sampled/blended one representative pair only. |
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
| Root motion | `partially-evaluated` | 70 counterpart pairs measured; 0.4.0 adds root_trajectory sampling on all 179 clips (71 move >1 cm, 107 stationary, 21 show >1° yaw; yaw `heading_axis` `positive_y` on 178/178) — a sampled grid fact, not continuous-curve or engine-extraction proof; controller ownership remains incomplete. |
| Conform | `partially-evaluated` | Slicing succeeded (36→0 time-monotonic errors). 0.3.0 safely refused all 24 in-place gait-anchor trials because the selected root had no finite horizontal forward axis; 0.4.0 measures that axis as vertical (`positive_y`) and anchors all 24 (spread ~0.05–0.09), but the resulting GLB candidates are unpromoted — no Humanoid-retarget or visual acceptance import. Root-motion (RM) anchoring was not attempted. |
| Validate | `partially-evaluated` | Exhaustive mechanical and provisional semantic checks reproduce exactly under 0.4.0; one engine import (retained) plus four advice/refusal/addressability engine profiles (0.4.0); no visual acceptance. 0.4.0 also separates loop-seam applicability (111/66) from evaluability (84 complete/93 not_evaluated). |
| Optimize | `partially-evaluated` | Constant-track pruning trialed but not approved. |
| Export | `partially-evaluated` | Unity imported native package; no generated production export or player build. |
| Gate/report | `evaluated-clean` | Commands, manifests, digests, primary report, and appendix retained. |

### Readiness evidence by clip set

Use the repository's [six-level readiness ladder](../game-ready-clips.md#the-readiness-ladder); this table reports evidence at those levels rather than redefining them.

| Role or set | File-ready / clip-ready | Set-ready / rig-use | Runtime / acceptance boundary |
|---|---|---|---|
| Idles/holds (14) | 14/14 mechanically clean with constant-track notes; six provisional loop declarations have seam findings, including two with linear/closure findings. | No loop-intent or mask contract. | Unity import covered; visual looping and layered use untested. |
| Continuous locomotion (68) | 68/68 mechanically clean; the 24 clearly cyclic in-place ring files are the defensible loop subset, and 22 have raw strict closure/seam-derivative findings. | Six 8-way sets share timing; all six raw rings fail phase target; 0.4.0 now emits 24/24 anchored in-place candidates for this rig (unpromoted). | Unity sampled six representatives and blended three pairs (retained), not full rings, visual contacts, or the new candidates. |
| Locomotion transitions (60) | 10/60 have negative-time errors until sliced; many of the 25 loop flags are semantically suspect for one-shots. | No authoritative transition chains; 56/73-bone boundaries occur. | Curated one-shots only until crossfade/interruption tests. |
| Airborne/traversal (17) | Mechanically clean; four obstacle files and falling are provisionally loop-marked despite likely one-shot/hold semantics. | No trajectory/contact/environment chain. | Controller and environment integration untested. |
| Actions/interactions (18) | 2/18 have negative-time errors until sliced; eight likely one-shot grenade actions are loop-marked. | Full-body tracks; no additive/mask/contact contract. | Unity import only; prop, IK, recovery, and visual result untested. |
| Three in-place 8-way rings | Raw phase spreads 0.660/0.463/0.716; 0.3.0 refused 24/24 anchor attempts before output; 0.4.0 anchors 24/24 (spreads 0.0724/0.0938/0.0502). | Candidates are unpromoted — no Humanoid-retarget or visual acceptance import; keep runtime offsets or artist-aligned exports as the shipped fallback; 0.2.1 aligned spreads remain historical only. | Full blend spaces and loop wraps need visual target-character review of the new candidates. |
| Three root-motion 8-way rings | Raw phase spreads exceed 0.15. | AnimSmith's fail-closed trajectory policy still prevents an unsafe cyclic rewrite; no root-motion (RM) gait-anchor trial was attempted under 0.4.0 either. | Runtime phase offsets or artist/root-preserving tooling required. |

### Validation-profile status

| Validation profile | Selection | Result / next evidence |
|---|---|---|
| Marketplace intake | `selected` — `user-required` | File/tool intake complete; provenance partial. |
| Blended locomotion | `selected` — `observed-pack-capability` | Six phase findings; 0.4.0 anchors all 24 in-place candidates (0.3.0 refused all 24) but they are unpromoted; full runtime rings and visual gate outstanding. |
| Root-motion controller | `selected` — `observed-pack-capability` | Pair translation measured; 0.4.0 adds root-trajectory sampling (71/107/21 split) and a positive_y heading axis; extraction proof and controller ownership outstanding. |
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

Untouched **Unity import and headless evaluation** completed in Unity 6000.5.8f1 on 2026-08-17 and is retained unchanged in this 0.4.0 refresh because the source archive is byte-identical. The package imported into a disposable project with exit 0. Unity processed 179 FBXs overall; the motion directory contains 178 FBXs (177 individual clips plus the combined take), all configured as Humanoid with valid source-avatar references. Six representative in-place/root-motion clips evaluated through `AnimationClipPlayable`, and three representative walk/run/crouch pairs evaluated through a 50/50 `AnimationMixerPlayable`. All nine checks completed without exceptions.

Unity logged one material pack finding: `Protof-Actor@BasicLocomotionAnimset.fbx`, the combined take, reports a copied-avatar hierarchy mismatch for the Hips transform. The 177 individual motion clips remain the recommended source route. Headless evaluation establishes importer and Playables compatibility, not visual motion or blend quality.

Untouched **offline** loading is strong: AnimSmith inspected and measured all 179 FBX files. Nine representative offline HTML reports were rendered at frame 0 and an injected midpoint and visually reviewed. Idle, walk, root-motion walk, side run, jump, landing, cover, and grenade samples showed coherent static humanoid poses without gross explosions. Root-motion walk showed a trajectory while its in-place partner remained stationary. The negative-time cover sample surfaced its errors. The 319.667-second combined take produced a visually dense, incoherent trajectory/transition report as expected for many actions presented as one clip.

Neither the static reports nor the headless Unity probe proves artistic motion quality, planted contacts, loop smoothness, full blend-space behavior, Avatar masks, target-mesh deformation, engine attachment behavior, controller response, compression behavior, or player-build correctness.

### Untouched AnimSmith findings

All rows below were re-run under AnimSmith 0.4.0 on 2026-08-21 unless noted; the first eight rows reproduce their 0.3.0 counts exactly (a stability result, not a new finding). Rows 9–11 are new 0.4.0 measurements not available under 0.3.0.

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
| Root trajectory now measured on every clip (0.4.0) | 179/179 clips; 71 move more than 1 cm horizontally, 107 are stationary (≤1 cm), 21 carry more than 1° of yaw travel | Sampled-grid regression facts, not continuous-curve or engine root-motion-extraction proof; **no movement-ownership axis is inferred from these numbers** | observed-animsmith (0.4.0) |
| Yaw heading axis resolves vertical (0.4.0) | `heading_axis` = `positive_y` on 178/178 measured clips | Recorded cause of the 0.3.0 horizontal-forward-axis gait-anchor refusal; the axis itself was not previously exposed | observed-animsmith (0.4.0) |
| Loop-seam availability now separates applicability from evaluability (0.4.0) | Contract pass: 111/177 files seam-applicable (66 not_applicable); of the applicable files, 84 complete / 93 not_evaluated | No-stride/stationary clips are now recorded as not evaluated instead of being mislabelled pass or fail | observed-animsmith (0.4.0) |

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

**0.4.0 (current, 2026-08-21).**

| Field | Value |
|---|---|
| AnimSmith version | `0.4.0` (tag `v0.4.0`) |
| Repository revision | `6b37ad636b198ef8ff47fadbf6a3a51eb1a27c8e` |
| Binary | SHA-256 `fd1eee57407aa02db88763d144389a7f5104204c40ddfbb28eb5885ca8cd54c6` |
| Platform | Linux WSL2 x86_64; rustc 1.97.1 |
| Output schemas | Command envelope v10; measurements v15 |
| Evidence directory | `<evaluation-workspace>/evidence/animsmith-0.4.0` |
| Rebuild reproducibility (2026-08-21) | Rebuilding tag `v0.4.0` at the same commit `6b37ad636b198ef8ff47fadbf6a3a51eb1a27c8e` produced a binary with a **different** SHA-256, `1e53013bbe3224557a8783eafeb818f4ef9d74666590cbaa8c18ef48c5b7d6fa`, than the digest recorded above — the build is **not byte-reproducible**. Both builds emit byte-identical import-advice artifacts, verified by `diff`. The Unity headless-probe correction and GLB candidate-import evidence added below (2026-08-21) are therefore attributable to the tag and commit, not to the originally recorded binary digest. |

**0.3.0 (historical, 2026-08-17).**

| Field | Value |
|---|---|
| AnimSmith version | `animsmith 0.3.0 (v0.3.0-17-g3857fe1)` |
| Repository commit | `3857fe130c227918e09473b2e1e307f61867439e` |
| Invocation | `<animsmith-checkout>/target/release/animsmith`; SHA-256 `a273f260d118de7de20e83d5c72c009540a63d63af352a4a6dd3cf97e62fbd5d` |
| Available commands/features | `inspect`, `measure`, `lint`, `report`, `transform`, `fix`, `convert`, `assemble`, `scale`, `diff`; current `fix` is glTF/GLB-only quaternion normalization/sign continuity |
| Baseline config and digest | `<evaluation-workspace>/config/baseline.animsmith.toml`; SHA-256 `612df8cc230c9e80b14373ef40336038b9fe308c8327f1529cbdb70612b9cc59` |
| Contract config and digest | 177 per-file configs under `config/contracts-0.3.0`; path-independent sorted-content aggregate SHA-256 `a1b0907976f8a3b6e56b682595e6a9aa35b3e733fe076c16b6aacd1d5a7a3024`; declarations are byte-identical to the 0.2.1 pass |
| Evidence directory | `<evaluation-workspace>/evidence/animsmith-0.3.0` |
| Output schemas | Command envelope v7; measurements v13 |

Build note: the 0.3.0 checkout was built with `RUSTC_WRAPPER= cargo build --release -p animsmith`; the full baseline, contract, and remediation command sets were rerun at that revision. Baseline and contract counts were unchanged from 0.2.1 at that time; hierarchy/rest evidence and gait-anchor behavior changed then. Under the current 0.4.0 refresh, baseline and contract counts reproduce 0.3.0 exactly (see Untouched AnimSmith findings above); gait-anchor behavior changed again — see Current-tool remediation trial below.

### Current-tool remediation trial

**0.4.0 (current, 2026-08-21).**

| Source issue | Operation and declarations | Result | Verification | Effort | Remaining caveat |
|---|---|---|---|---|---|
| Negative-time keys in 12 files | `transform --slice 0:<Unity lastFrame/30> --fps 30`; range derives from each delivered Unity clip declaration | 12/12 transforms succeeded; all 36 time-monotonic errors removed | Same 12 files re-verified inspect/measure/lint clean | Small, repeatable preprocessing | Honest trade-off: `diff` against the 0.3.0 sliced output shows `frame_count` 102→101 and loop-seam derivative deltas shifting at the sliced boundary (for example, one bone's seam angular velocity moves 18.5→38.2 deg/s). These are one-shot cover/grenade transitions, not loops, but the shift is real. |
| Misaligned 8-way gait phases | `transform --gait-anchor` on the same 24 **in-place** walk/run/crouch files using per-file loop/in-place/humanoid declarations | **24/24 transforms succeeded** (0/24 under 0.3.0). AnimSmith now measures a vertical yaw heading axis (`positive_y`) and anchors on it. Circular gait-phase spread (smallest arc containing all ring members): Crouch 0.7156245→0.0501911, Run 0.4630161→0.0938395, Walk 0.6597812→0.0724415 | Anchored outputs and spread measurements retained for every member; source baseline/contract evidence is unchanged | Automated; no manual DCC work for this step | **The 24 GLB candidates are unpromoted: no Humanoid-retarget or visual acceptance import.** Root-motion (RM) gait-anchor was not attempted. Treat the candidates as build-time offsets or artist exports until visually gated. |
| Baked constant tracks | `transform --prune-constant-tracks` on the same standard walk, cover clip, and combined file | 3/3 transforms succeeded, reproducing the 0.3.0 byte ratios | Not re-verified beyond reproduction; still bounded by open issue [#401](https://github.com/mmannerm/animsmith/issues/401) | Small to run, high proof burden | Do not adopt from this trial. Dense transition coverage and semantic equivalence are not proven. |

**0.3.0 (historical, 2026-08-17).**

| Source issue | Operation and declarations | Result | Verification | Effort | Remaining caveat |
|---|---|---|---|---|---|
| Negative-time keys in 12 files | `transform --slice 0:<Unity lastFrame/30> --fps 30`; range derives from each delivered Unity clip declaration | 12/12 transforms succeeded; all 36 time errors removed | 12 inspect/measure and fix dry-runs exit 0; 10 contract lints exit 0; 2 grenade files retain only loop-seam errors; `diff` exit 1 is expected for intentional one-frame trim | Small, repeatable preprocessing | Requires trusting the delivered per-file Unity frame range; transformed output is GLB. |
| Misaligned 8-way gait phases | `transform --gait-anchor` on 24 **in-place** walk/run/crouch files using per-file loop/in-place/humanoid declarations | 0/24 transforms succeeded; all exit 2 and emit no output because selected Root `root` has no finite horizontal forward axis at sample 0 | Refusal text and absence of outputs retained for every member; source baseline/contract evidence remains unchanged | No usable current transform; runtime/DCC work remains | Safe refusal prevents an unproved rewrite but does not align the rings. The source root's local +Z is vertical under its rest basis; [#426](https://github.com/mmannerm/animsmith/issues/426) tracks coordinate-basis-safe support. |
| Baked constant tracks | `transform --prune-constant-tracks` on a standard walk, cover clip, and combined file | 3/3 transforms succeeded; output/source byte ratios 12.3%, 8.7%, and 41.8% | All outputs inspect/measure/lint and fix dry-run exit 0; all `diff` runs exit 1 with large/index-sensitive measurement deltas | Small to run, high proof burden | Do not adopt from this trial. Dense transition coverage and semantic equivalence are not proven. |

The slice operation remains a **current declared transform** under both versions. 0.3.0 required an explicit in-place gait policy and independently checked the selected root/Hips trajectory before gait anchoring; that fail-closed safety from [#407](https://github.com/mmannerm/animsmith/issues/407) reached a stricter heading-basis refusal for this pack because the selected root's transformed local +Z is vertical under its rest basis. 0.4.0 resolves that specific limitation by measuring the vertical heading axis directly and anchoring against it — the coordinate-basis-safe support that [#426](https://github.com/mmannerm/animsmith/issues/426) tracked. The 24 successful 0.2.1 outputs and their post-anchor spreads remain separate historical evidence; the 0.4.0 candidates are a new, unpromoted result, not a re-adoption of the 0.2.1 output. A root-motion-preserving cyclic rebase for the RM variant is still plausible only with independently re-derived displacement and yaw proof. AnimSmith also does not repair genuine loop pose/velocity seams, retarget a rig, create additive motion, fix contacts, or author missing animation.

Current public issues [#401](https://github.com/mmannerm/animsmith/issues/401) and [#402](https://github.com/mmannerm/animsmith/issues/402) document why pruning requires property-scoped policy and emitted `(bone, property)` coverage. File-scoped identity and grouping are tracked by [#409](https://github.com/mmannerm/animsmith/issues/409). No public issue is being claimed for proprietary Unitypackage extraction.

### Before/after conclusion

Current AnimSmith (0.4.0) makes the 12 strict-time failures mechanically usable under a declared frame-range policy, exposes complete loader-projected hierarchy/rest evidence including the hand-scale warning, and now anchors all 24 in-place gait-ring members via a measured vertical heading axis where 0.3.0 refused all 24. It does **not** turn the pack into a production-certified asset automatically: the 24 anchored candidates are unpromoted pending an engine/visual gate, RM-variant anchoring was not attempted, and the 22/24 raw in-place loop failures still require semantic review, engine transition policy, or artist correction. The pruning trial still demonstrates potential storage reduction but fails the proof bar and is excluded from the recommended pipeline.

## Engine procedures and evidence

### Import configuration

Native Unity delivery was selected because the source is a Unitypackage and all metadata declares Unity Humanoid animation. Unity `6000.5.8f1 (5cb7df797b7d)` on Windows 11 created a disposable project, imported the package, and exited 0. The import performed 199 asset imports and exposed 177 humanoid animation clips. The retained probe reports 178 Humanoid model importers in the motion directory, 178 valid source-avatar references, 177 human-motion clips, 6/6 representative sampling passes, and 3/3 representative blend passes.

Remaining Unity import gates are: retargeting each skeleton signature to the actual project character; reviewed loop/root-transform settings; before/after import of the 12 sliced clips; compression effects on contacts, fingers, prop bones, and seams; full blend controllers; masks/IK; and a player build.

### Runtime playback and root motion

Headless Playables sampling and pair mixing are evaluated; visual playback and controller behavior are not. The file-level pair inventory is favorable: all 70 `_RM` files have a same-skeleton non-RM partner, durations match for every pair, and frame counts match for 68. `GoOutOfCoverRightStanding_RM` is one frame longer than its partner; `IdleTakeCoverCrouchingToIdleStanding_RM` is one frame shorter.

The current root-motion speed contract is incomplete for turns. Four crouch turns, two run U-turns, four 90°/180° turns, two walk U-turns, and two cover strafes are `_RM`-labeled but fall below the default 0.5 m/s horizontal-speed threshold. These may contain rotational root motion or intentional low displacement. Validate translation and yaw separately in Unity rather than treating the check as a definitive defect.

Test root motion against controller collision, slopes/steps, capsule reconciliation, animation interruption, and the project's networking/rollback policy. For in-place motion, verify authored foot speed against controller speed; this evaluation did not derive stride-matched gameplay velocities.

### Exact engine-profile evidence (0.4.0)

AnimSmith 0.4.0 adds exact, per-engine advice/refusal/addressability profiles, run 2026-08-21 and independent of the retained Unity 6000.5.8f1 import above. These profiles are static-metadata predictions or typed refusals, not import, retarget, or playback evidence.

| Profile | Engine/version | Subcommand | Result | Notes |
|---|---|---|---|---|
| `unity-humanoid` | Unity 6000.3, revision 1 | `import-advice` | `available`, exit 0 | Declarations were originally derived from delivered `.fbx.meta`: `useFileUnits: 1` on all metas; `lockRootRotation`/`lockRootHeightY`/`lockRootPositionXZ` are absent on all 918 collection metas, so the profile read Unity's serialization default (`false` ⇒ `extract`) for each. **Assumption stated explicitly at the time: an absent `.meta` key takes the Unity serialization default.** That was a stated assumption, not an observation, and it is now falsified — see "Unity headless candidate probe (2026-08-21 correction)" immediately below. This row's `available`/exit 0 result is unaffected; only the assumed default-`false`/`extract` reading was wrong. |
| `unreal` | Unreal Engine 5.8, revision 1 | `import-advice` | typed refusal `profile_settings_unmodeled`, exit 1 | No import was attempted; the profile itself declines to model this engine's settings surface yet. |
| `godot` | Godot 4.7, revision 1 | `import-advice` | typed refusal `profile_settings_unmodeled`, exit 1 | Same refusal shape as Unreal; no import attempted. |
| `bevy` | Bevy 0.19.0, revision 1 | `addressability` | exit 0 on a generated GLB candidate | 1 animation row, coverage complete, predicted selector `Animation0`, facet state `available`, 0 findings. Proves inventory/selector prediction only — not loading, targets, graph wiring, or playback. |

### Unity headless candidate probe (2026-08-21 correction)

The `unity-humanoid` advisory row above stated an explicit assumption: because `lockRootRotation`, `lockRootHeightY`, and `lockRootPositionXZ` are absent from every delivered `.fbx.meta`, the profile read Unity's serialization default of `false` for each key and projected `extract` for every clip. **That assumption is falsified by direct observation.** Unity `6000.5.8f1` was run headless (`-batchmode -nographics -quit -executeMethod CandidateProbe.Run`) in a **new**, disposable project — the retained eight-pack project described under Untouched import and playback above was not modified — reading `ModelImporterClipAnimation` on the delivered files together with their delivered `.meta`, across a 120-clip sample spanning all eight collection packs (including 24 of this pack's own root/root-adjacent files):

| Variant | Clips | `lockRootRotation` true | `lockRootHeightY` true | `lockRootPositionXZ` true |
|---|---:|---:|---:|---:|
| In-place (non-`_RM`) | 84 | 84 | 84 | 83 |
| Root-motion (`_RM`) | 36 | 36 | 28 | 5 |

Aggregate across the sample: 120/120 clip definitions inspected, 120/120 `lockRootRotation` true, 112/120 `lockRootHeightY` true, 88/120 `lockRootPositionXZ` true. The delivered importer policy is therefore **bake**, not extract, and it is per-variant and axis-specific: `lockRootPositionXZ` is the discriminator — baked (`true`) for essentially all in-place clips and mostly extracted (`false`) for root-motion clips — a coherent authored root-motion policy, not an oversight or a random default.

Two in-place files read `false` on all three flags in an earlier 24-file pass: `Humanoid@RunLeftUnarmed.fbx` and `Humanoid@RunRightUnarmed.fbx`. Both have no explicit clip definition in their `.meta` — consistent with the 15 per-motion files noted under Delivery and organization above that lack explicit `clipAnimations` metadata — so Unity falls back to `defaultClipAnimations` for these two specifically. That is a separate, file-identity cause and does not contradict the observed per-variant bake/extract policy for the other 118 sampled clips.

This observation supersedes the stated default-value assumption in the `unity-humanoid` advisory row above; it does not change that row's `available`/exit 0 result, only the projected lock values. Both the original assumption and this correction are retained here for provenance: the assumption was reasonable given the delivered `.meta` alone, and only a direct Unity engine observation could falsify it.

### GLB candidate import into Unity (2026-08-21)

All 134/134 AnimSmith 0.4.0 gait-anchored GLB candidates across the eight-pack collection — including this pack's 24 anchored in-place walk/run/crouch candidates (see AnimSmith remediation evidence above) — were staged into a separate, **new** Unity 6000.5.8f1 project using `com.unity.cloud.gltfast` 6.9.0, because Unity has no native GLB importer; the retained eight-pack project above was not modified. Result: 134/134 files staged produced assets, 134/134 produced exactly one Unity `AnimationClip`, and every clip is non-legacy and non-empty.

**Limit, stated plainly:** glTFast imports glTF animation as a **Generic** clip and does not reconstruct a Humanoid Avatar. This proves the candidates load and yield one well-formed clip in Unity; it does **not** test the Humanoid retarget path this pack actually uses, and it is not a visual or gameplay acceptance test. The 24 gait-anchored candidates for this pack therefore remain **unpromoted**, unchanged from the AnimSmith remediation evidence above.

### Performance and packaging

No engine import size, runtime memory, decompression CPU, build size, or platform performance was evaluated. Source FBX files carry many constant channels. AnimSmith can remove many of them in sampled GLBs, but byte reduction across FBX→GLB is not a runtime-performance measurement and the trial lacks an acceptable equivalence proof. Retain the untouched coverage until a target-runtime measurement and channel-coverage gate justify pruning.

## Blending, masking, and gameplay caveats

### Locomotion, sync, and transitions

The in-place walk, run, and crouch directional rings each contain eight files with equal duration and frame count within that ring, which is a good blend-space prerequisite. Their raw gait phases are not same-time aligned: minimum circular spread is 0.660 for walk, 0.463 for run, and 0.716 for crouch, compared with a common 0.15 alignment target. Direct blending at normalized time therefore risks mixing unlike foot phases.

AnimSmith 0.3.0 refused all 24 in-place anchoring attempts before output because this source rig has no finite horizontal forward axis under the then-current root-basis rule. AnimSmith 0.4.0 resolves that specific limitation: it measures the rig's yaw heading axis directly (`positive_y` on 178/178 clips) and anchors on it, so all 24 in-place candidates now succeed — Crouch 0.7156245→0.0501911, Run 0.4630161→0.0938395, Walk 0.6597812→0.0724415, all comfortably under the 0.15 alignment target. These 24 GLB candidates are new 0.4.0 output; they are unpromoted because no Humanoid-retarget or visual acceptance import, and are not the same artifacts as the 0.2.1 outputs, which remain a separate historical comparison (spreads 0.072/0.094/0.050). Twenty-two of the 24 raw in-place clips still fail at least one strict loop closure or seam derivative check — anchoring does not repair loop seams. Use the 0.4.0 candidates only after a visual/engine gate, or fall back to runtime phase offsets or artist-aligned exports; then test the complete Unity 2D blend space at cardinal, diagonal, and intermediate weights, including phase wrap, accelerations, stops, turns, and transitions to idle/jump.

The contract pass derives loop status from delivered Unity metadata. Since that metadata also marks grenade throws, falling, obstacle passes, and turns as loops, a human must first decide which clips are actually cyclic. Otherwise the failure count mixes content defects with incorrect declarations.

### Upper/lower-body masking and additive use

Runtime masking is not evaluated. The files carry full-body baked transform tracks and the Unity metadata's explicit clips have `hasAdditiveReferencePose: 0`; there are no dedicated upper-body-only or additive files. Unity Humanoid AvatarMasks may allow a grenade or aim clip to override the upper body while locomotion drives the lower body, but the mask boundary, spine continuity, pelvis ownership, arm reach, hand/prop alignment, and root behavior must be tested.

Recommended default: keep pelvis/root and legs in the locomotion layer, begin the action mask above a project-chosen spine boundary, and test several blend weights and transition times. If the action meaningfully shifts pelvis, center of mass, or supporting feet, a pure upper-body mask will likely look wrong and a full-body authored transition is preferable. No IK target bones are evident in the standard 56-bone hierarchy; the 73-bone cover/grenade variant adds cover/rocket/nub nodes but does not establish a portable hand/foot IK contract.

### Game-type caveats

| Game/system context | Suitability | Caveat or required work | Evidence |
|---|---|---|---|
| Third-person action prototype | Good candidate | Complete engine/character test; curate loops and transitions | observed-file, observed-report, inferred |
| Controller-driven in-place locomotion | Good candidate with conditions | 0.4.0 anchors this rig's 24 in-place candidates (0.3.0 refused); validate them in engine/visually before use, or apply runtime/DCC phase alignment; tune controller speed; inspect foot slide and seam wraps | observed-animsmith |
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
| 8-way in-place walk ring | 56-bone standard | In-place by filename; speed below 0.5 m/s | Equal duration/frames; raw phase spread 0.6597812; 0.3.0 anchor refused, 0.4.0 anchors to 0.0724415 | Not engine/visually tested | Anchored candidate, unpromoted | observed-animsmith |
| 8-way in-place run ring | 56-bone standard | In-place by filename; speed below 0.5 m/s | Equal duration/frames; raw phase spread 0.4630161; 0.3.0 anchor refused, 0.4.0 anchors to 0.0938395 | Not engine/visually tested | Anchored candidate, unpromoted | observed-animsmith |
| 8-way in-place crouch ring | 56-bone standard | In-place by filename; speed below 0.5 m/s | Equal duration/frames; raw phase spread 0.7156245; 0.3.0 anchor refused, 0.4.0 anchors to 0.0501911 | Not engine/visually tested | Anchored candidate, unpromoted | observed-animsmith |
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
2. Unity package import and headless sampling/pair-blending were evaluated (retained, 2026-08-17), but no visual full-ring controller, target-character retarget, masks, root-motion controller, compression comparison, or player build was run. 0.4.0 added exact advice/refusal/addressability profiles for Unity, Unreal, Godot, and Bevy (2026-08-21), but these are static-metadata predictions, not import, retarget, or playback evidence — Unreal and Godot remain refused, and Bevy's addressability probe only inventories a generated GLB candidate.
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
13. The 24 gait-anchored GLB candidates produced under 0.4.0 loaded only as Generic glTF clips, with no Humanoid-retarget or visual review; treat them as unpromoted pending that gate.
14. The integration recipe's `owner=validate-per-axis` step directs the reader to validate root-motion ownership axis by axis rather than assume it. The observed Unity importer locks bake root rotation on every sampled root-motion clip, so animation cannot be assumed to own root-motion yaw. The step is not a per-axis `movement_owner_xz` / `movement_owner_y` / `movement_owner_yaw` declaration, and no such declaration is derived from measured travel in this refresh. Measured root displacement and yaw are recorded as sampled facts only; choosing the per-axis owner remains a project and engine decision.
15. A 2026-08-21 direct Unity 6000.5.8f1 headless probe falsified the `unity-humanoid` advisory's stated default-`false`/`extract` assumption for root-lock declarations (see Unity headless candidate probe above): the observed delivered policy is `bake` for in-place clips and per-axis `bake`/`extract` (XZ is the discriminator) for root-motion clips. The probe is still headless-import evidence over a 120-clip cross-pack sample, not continuous visual or gameplay acceptance, and it does not by itself validate the `unity-humanoid` profile's other advice fields.
16. The 134/134 GLB-candidate Unity import (2026-08-21, including this pack's 24 candidates) proves glTFast produces one well-formed Generic `AnimationClip` per candidate in a fresh project; it does not exercise the Humanoid retarget path this pack uses, and it does not promote the candidates. A same-commit rebuild of AnimSmith `v0.4.0` produced a differently-hashed binary (SHA-256 `1e53013bbe3224557a8783eafeb818f4ef9d74666590cbaa8c18ef48c5b7d6fa`, versus the recorded `fd1eee57407aa02db88763d144389a7f5104204c40ddfbb28eb5885ca8cd54c6`) — the build is not byte-reproducible — but both builds emit byte-identical import-advice artifacts, so this appendix's regenerated Unity evidence is attributable to the tag and commit, not to the originally recorded binary digest.

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
- **0.4.0 re-verification (2026-08-21):** a fresh `inventory_pack.py` reconciliation over the same logical-asset root reproduced this manifest exactly — 0 paths added, 0 removed, 0 content changed, 179 FBX, logical manifest SHA-256 unchanged. The RAR SHA-256 above re-verified, and the retained Unitypackage payload was independently rehashed live and matched its recorded digest.

### Evaluation manifest

- Schema: `urn:animsmith:skill:animation-pack-evaluation-manifest:1`
- Clip taxonomy version: `1`
- Validation-profile-set version: `1`
- Validated manifest: `evidence/animsmith-0.3.0/evaluation-manifest.json`
- Manifest SHA-256: `3cc3922dc7b4b06db59643f366eab2844f4490334868ea5a2c26bd1926000cd4`
- Version-migration script: `evidence/animsmith-0.3.0/migrate-evaluation-manifest.py`; SHA-256 `0ee38f1ec5906b2bc9158d7d6fe07e8747faf57e95df3ae62aa252b5a3ba0fb5`
- 0.4.0 evidence tree: `evidence/animsmith-0.4.0/`; retains the same validated taxonomy and manifest content unchanged, because source identity did not change (no new manifest digest was generated to replace the one above)

The manifest maps all 177 physical files to 107 logical motions and every logical motion to exactly one canonical role. It retains ten candidate runtime sets, all eleven profile-selection decisions, and all ten pipeline-stage coverage states. The validator recomputes role/file totals and checks all cross-references.

### AnimSmith commands and outcomes

**0.4.0 (current, 2026-08-21):**

```text
# Build/version capture
RUSTC_WRAPPER= cargo build --release -p animsmith
target/release/animsmith --version
# animsmith 0.4.0 (v0.4.0), revision 6b37ad636b198ef8ff47fadbf6a3a51eb1a27c8e

# Source-identity reconciliation
inventory_pack.py --pack basic-locomotion
# 0 paths added, 0 removed, 0 content changed; 179 FBX; manifest digest matches published

# Exhaustive baseline pattern, repeated for all 179 FBX files
animsmith inspect --config config/baseline.animsmith.toml <input.fbx>
animsmith measure --config config/baseline.animsmith.toml --format json <input.fbx>
animsmith lint --config config/baseline.animsmith.toml --format json <input.fbx>
# output schema v10 / measurements v15; inspect/measure: 179 exit 0; lint: 167 exit 0, 12 exit 1
# constant-track 24186 notes, time-monotonic 36 errors/12 files -- identical to 0.3.0
# root_trajectory measured 179/179; yaw heading_axis = positive_y on 178/178 clips

# Per-motion contract pattern, repeated for 177 files
animsmith lint --config config/contracts-0.4.0/<file>.animsmith.toml --format json <input.fbx>
# 58 exit 0; 119 with findings; loop-seam applicability 111 applicable/66 not_applicable,
# evaluability 84 complete/93 not_evaluated

# Negative-time remediation pattern, repeated for 12 files (unchanged range policy)
animsmith transform --config <per-file-config> <input.fbx> -o <output.glb> \
  --slice 0:<unity-last-frame-divided-by-30> --fps 30
# 12/12 transform exit 0; time-monotonic 36 -> 0
# diff vs the 0.3.0 sliced output shows frame_count 102 -> 101 and loop-seam derivative
# deltas shifting at the sliced boundary (e.g. one bone's seam angular velocity 18.5 -> 38.2 deg/s)

# Directional-ring remediation pattern, repeated for the same 24 in-place clips
animsmith transform --config <per-file-config> <input.fbx> -o <output.glb> --gait-anchor
# 24/24 transform exit 0 (0/24 under 0.3.0); yaw heading axis measured positive_y and anchored on
# circular gait-phase spread (smallest arc containing all ring members, not max-minus-min):
#   Crouch 0.7156245 -> 0.0501911; Run 0.4630161 -> 0.0938395; Walk 0.6597812 -> 0.0724415
# candidates written to generated/remediation-0.4.0/gait-anchor/; UNPROMOTED, no Humanoid-retarget or visual acceptance import

# Exact engine-profile evidence
animsmith engine-profile unity-humanoid import-advice <input.fbx>
# available, exit 0 (declarations derived from delivered .fbx.meta defaults)
animsmith engine-profile unreal import-advice <input.fbx>
animsmith engine-profile godot import-advice <input.fbx>
# both: typed refusal profile_settings_unmodeled, exit 1
animsmith engine-profile bevy addressability <generated.glb>
# exit 0; 1 animation row, selector Animation0, 0 findings

# Experimental only; not approved for shipment
animsmith transform --config config/baseline.animsmith.toml <input.fbx> \
  -o <output.glb> --prune-constant-tracks
# 3/3 transform exit 0; reproduces 0.3.0 byte ratios; still bounded by open #401
```

**0.3.0 (historical, 2026-08-17):**

```text
# Build/version capture
RUSTC_WRAPPER= cargo build --release -p animsmith
target/release/animsmith --version
# animsmith 0.3.0 (v0.3.0-17-g3857fe1)

# Exhaustive baseline pattern, repeated for all 179 FBX files
animsmith inspect --config config/baseline.animsmith.toml <input.fbx>
animsmith measure --config config/baseline.animsmith.toml --format json <input.fbx>
animsmith lint --config config/baseline.animsmith.toml --format json <input.fbx>
animsmith lint --config config/baseline.animsmith.toml --format markdown <input.fbx>
# inspect/measure: 179 exit 0; lint: 167 exit 0, 12 exit 1

# Per-motion contract pattern, repeated for 177 files
animsmith lint --config config/contracts-0.3.0/<file>.animsmith.toml --format json <input.fbx>
# 58 exit 0; 119 exit 1 under delivered/inferred declarations

# Loader-projected hand rest-world scale, repeated for all 179 FBX files
animsmith lint --config config/hand-rest-scale.animsmith.toml --format json <input.fbx>
# 179/179 evaluations complete; 358 hand warnings; scale approximately 0.01

# Negative-time remediation pattern, repeated for 12 files
animsmith transform --config <per-file-config> <input.fbx> -o <output.glb> \
  --slice 0:<unity-last-frame-divided-by-30> --fps 30
# 12/12 transform exit 0; no remaining time-monotonic errors

# Directional-ring remediation pattern, repeated for 24 in-place clips
animsmith transform --config <per-file-config> <input.fbx> -o <output.glb> --gait-anchor
# 0/24 transform exit 0; every trial safely refuses before output because the
# selected root has no finite horizontal forward axis under the current basis rule

# Experimental only; not approved for shipment
animsmith transform --config config/baseline.animsmith.toml <input.fbx> \
  -o <output.glb> --prune-constant-tracks
# 3/3 transform exit 0; 3/3 diff exit 1; semantic equivalence not established

# Every transformed output was inspected/measured/linted and checked with:
animsmith diff --config <config> --format json <source.fbx> <output.glb>
animsmith fix --config <config> --dry-run <output.glb>
```

The retained runners are `evidence/run_baseline.py`, `evidence/run_contract.py`, `evidence/run_remediation.py`, `evidence/run_targeted_lint.py`, and their summarizers. Command argv, exit codes, stdout, and stderr are retained under versioned `evidence/animsmith-*` directories, including a parallel `evidence/animsmith-0.4.0/` tree for the current pass.

### Engine procedure

Completed procedure:

1. Invoke Unity 6000.5.8f1 in batch mode with `-createProject` on a disposable evaluation path.
2. Import the extracted Unitypackage with `-importPackage`.
3. Capture the Editor log, import counts, warnings, and exit code.
4. Add a local-only Editor probe that inventories the motion FBXs, importer/avatar/clip metadata, and representative clips.
5. Evaluate six representative in-place/root-motion clips with `AnimationClipPlayable` and three walk/run/crouch pairs with `AnimationMixerPlayable`.

Observed result: package import exit 0 and probe exit 0. Unity exposed 177 human-motion clips and valid source-avatar references for all 178 motion-directory FBX importers. Six sampling and three pair-blend checks passed. The combined all-in-one FBX logged a copied-avatar hierarchy mismatch; the individual files remain usable. Next build a visual scene containing the source and target characters, complete 8-way controllers and transitions, root-motion toggles, AvatarMask layers, compression variants, and profiler/player-build measurements.

**0.4.0 exact engine-profile procedure (2026-08-21):** for each of the four profiles, invoke the profile's advice/addressability subcommand against the delivered `.fbx.meta` declarations (Unity, Unreal, Godot) or a generated GLB candidate (Bevy) and capture argv, exit code, and JSON result. No project was created, no package was imported, and no engine process executed for Unreal, Godot, or Bevy — these are metadata-only predictions or typed refusals. Unity's `unity-humanoid` advice is likewise metadata-only and separate from the retained import above.

### Evidence artifacts

| Artifact | Purpose | Digest or identity |
|---|---|---|
| `evidence/logical-asset-manifest.json` | Reconstructed logical asset inventory and hashes | SHA-256 `5bec4f741c39f232c79f4c841fc0eb580589f3868b614610cb6ff15a59a0b34b` |
| `evidence/animsmith-0.3.0/evaluation-manifest.json` | Validated v1 canonical roles, delivered variants, runtime sets, profile selection, pipeline coverage, and per-file evidence | SHA-256 `3cc3922dc7b4b06db59643f366eab2844f4490334868ea5a2c26bd1926000cd4` |
| `evidence/animsmith-0.3.0/migrate-evaluation-manifest.py` | Reproducibly preserves unchanged taxonomy while versioning evaluator and changed stage evidence | SHA-256 `0ee38f1ec5906b2bc9158d7d6fe07e8747faf57e95df3ae62aa252b5a3ba0fb5` |
| `evidence/animsmith-0.3.0/baseline-summary.json` | Exhaustive default inspect/measure/lint aggregation; identical digest to 0.2.1 | SHA-256 `85c97c726c112efca5b1b3aa143f2b6c951917bc9de02fd544dd5a652090d75e` |
| `evidence/animsmith-0.3.0/baseline/command-results.json` | 0.3.0 baseline argv, exits, and per-command evidence paths | SHA-256 `853066da58caa300be6d765b0d771bf9c684a39777e76dbd29f1a0fd1f699dd2` |
| `evidence/animsmith-0.3.0/contract/command-results.json` | 0.3.0 per-motion contract argv, exits, configs, and evidence paths | SHA-256 `d7e7e39d67eb4e4f8b8165a676d36314ac6c0e18bbb1e7fce1f36676c1ecd1ef` |
| `evidence/animsmith-0.3.0/remediation-batch/command-results.json` | 0.3.0 slice, gait-anchor refusal, and prune trial/verification records | SHA-256 `f15878121775534531227d61c0d0d5a1959e2e39381b43b2d96ec3b64d37f41c` |
| `evidence/animsmith-0.3.0/hand-rest-scale/command-results.json` | Exhaustive declared hand rest-world-scale lint argv, exits, and evidence paths | SHA-256 `c98ecd56c5f8d991712aedf8db194a4081efff1f4957a0773fe0f0063fd78a29` |
| `evidence/run_targeted_lint.py` | Deterministic exhaustive runner used for the newly available hand-scale check | SHA-256 `39547f8856e5a1030505ead9a0e384d03472913ee851d145eefa315b90303c23` |
| `evidence/unity-meta-summary.json` | Unity importer/clip metadata aggregation | SHA-256 `a73b5162632d2e8d40dae8971440d1555b434956d4c5e482e26383c29ea04458` |
| `evidence/report-screenshots/representative-midpoint-contact-sheet.png` | Visual QA contact sheet for nine offline reports | SHA-256 `e8ec17b71042f101bb1816141f2b187fcfb2f9de899289d9a04d98f38c0cfc1c` |
| `evidence/unity-6000.5.8f1-import.log` | Licensed Unity package import, asset counts, and combined-FBX rig warning | SHA-256 `914136cdb3458d6353d5258c9aeb5d1878097f8fe9bb96aafe22f01bb75e9ea4` |
| `generated/unity-6000.5.8f1-project/Assets/Editor/AnimationPackProbe.cs` | Local-only reproducible importer and Playables probe | SHA-256 `4471b9fc9b2c0b1cd334bac654fd0b35257b9f558403d9c2be418eb03620b351` |
| `evidence/unity-6000.5.8f1-probe.json` | Importer/clip inventory and representative Playables results | SHA-256 `e8128312b4db544c354c95c397a85fa68155adec1423eba3c22a413053f4fbb9` |
| `evidence/unity-6000.5.8f1-probe.log` | Headless probe execution log | SHA-256 `7e69b26f6482197046e3f365e15a0bb57e49efd9b9d047b5eef1d12defc5a9ce` |
| AnimSmith binary (0.3.0, historical) | Exact 0.3.0 evaluator executable | SHA-256 `a273f260d118de7de20e83d5c72c009540a63d63af352a4a6dd3cf97e62fbd5d` |
| AnimSmith binary (0.4.0, current) | Exact 0.4.0 evaluator executable, tag `v0.4.0`, revision `6b37ad636b198ef8ff47fadbf6a3a51eb1a27c8e` | SHA-256 `fd1eee57407aa02db88763d144389a7f5104204c40ddfbb28eb5885ca8cd54c6` |
| `evidence/animsmith-0.4.0/` | 0.4.0 baseline, contract, remediation, and engine-profile command results, argv, exits, and evidence paths (parallel structure to the 0.3.0 tree above); specific per-file digests were not re-captured in this appendix | Not individually re-hashed here; see the 0.4.0 command reproduction above |

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

- Local source archive and extracted Unity metadata — private local artifact identified above, accessed 2026-08-16.
- Protofactor, [Animset: Basic Locomotion](https://protofactor.biz/product/animset-basic-locomotion/) — current product description, counts, formats, Unity compatibility, and price, accessed 2026-08-16.
- Protofactor, [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/) — current collection price, constituent-pack list, and advertised aggregate count, accessed 2026-08-16.
- Protofactor, [End User License Agreement](https://protofactor.biz/end-user-license-agreement/) — current one-owner, protected-real-time-application, modification, transfer, and redistribution terms; not evidence of the local transaction's governing terms, accessed 2026-08-16.
- Unity Asset Store, [Ultimate Animation Collection](https://marketplace.unity.com/packages/3d/animations/ultimate-animation-collection-195459) — current collection version/date, price, license tier, and original Unity version, accessed 2026-08-16.
- AnimSmith [v0.4.0 release](https://github.com/mmannerm/animsmith/releases/tag/v0.4.0) — repository revision `6b37ad636b198ef8ff47fadbf6a3a51eb1a27c8e`, binary SHA-256 `fd1eee57407aa02db88763d144389a7f5104204c40ddfbb28eb5885ca8cd54c6`, accessed 2026-08-21.
- AnimSmith public issue [#165](https://github.com/mmannerm/animsmith/issues/165) — current roadmap guardrails for automatic animation rewrites, accessed 2026-08-16.
- AnimSmith public issue [#401](https://github.com/mmannerm/animsmith/issues/401) — constant-track pruning property-scoped policy limitation; verified open, accessed 2026-08-21.
- AnimSmith public issue [#402](https://github.com/mmannerm/animsmith/issues/402) — emitted channel-coverage limitations, accessed 2026-08-16.
- AnimSmith public issue [#411](https://github.com/mmannerm/animsmith/issues/411) — declared-set speed/lint evidence; verified open, accessed 2026-08-21.
- AnimSmith public issue [#407](https://github.com/mmannerm/animsmith/issues/407) — shipped 0.3.0 fail-closed gait-anchor trajectory policy, accessed 2026-08-17.
- AnimSmith public issue [#426](https://github.com/mmannerm/animsmith/issues/426) — follow-up for in-place rigs whose root local forward axis is vertical, filed 2026-08-17; 0.4.0 now measures that axis directly and anchors on it (see AnimSmith remediation evidence above).
- Unity 6.5 Manual, [Animation Blend Trees](https://docs.unity3d.com/6000.5/Documentation/Manual/class-BlendTree.html) — normalized-time/contact alignment context only; no pack result, accessed 2026-08-16.
- Epic Games, [Animation Sync Groups in Unreal Engine](https://dev.epicgames.com/documentation/unreal-engine/animation-sync-groups-in-unreal-engine) — cycle/foot-placement synchronization context only; no pack result, accessed 2026-08-16.
- Godot Engine stable documentation, [Using AnimationTree](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html) — blend-space, sync-mode, filtering, and missing-track context only; no pack result, accessed 2026-08-16.
- Bevy, [Animation Graph example](https://bevy.org/examples/animation/animation-graph/) — weighted graph blending context only; no pack result, accessed 2026-08-16.
