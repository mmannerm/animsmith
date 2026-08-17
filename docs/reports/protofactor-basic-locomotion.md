# Animation pack evaluation: Protofactor Basic Locomotion Animset (local Ultimate Animation Collection archive)

> Evaluation status: Partial — full archive, canonical-role/runtime-set, and mechanical evaluation completed; common-engine runtime evaluation is deferred, and the attempted Unity import was blocked by an unavailable Editor license.
>
> Overall recommendation: Prototype only
>
> Confidence: Medium
>
> Evaluation date: 2026-08-16

## Executive decision

### Decision

**Prototype only, medium confidence.** The 177 per-motion files are a strong, mechanically readable source pool, and current AnimSmith can remove the 12 negative-time failures and align the three tested in-place directional rings. Production approval is blocked by unresolved loop semantics/seams, incomplete set- and rig-use contracts, and no successful runtime import or playback in Unity, Unreal Engine, Godot, or Bevy.

### Canonical clip-role inventory

The version-1 canonical taxonomy separates what a motion does from pack-specific context and runtime relationships. The 177 files represent **107 logical motions**: 70 motions have an evidenced in-place/root-motion pairing (140 files), while 37 have one delivered variant. All pairs share skeleton and duration, 68/70 share frame count, all in-place partners measure zero horizontal root speed, and pairwise gait phase differs by at most 0.0053. This supports counterpart grouping but does **not** prove root-track-only equivalence.

| Canonical primary role | Logical motions | Delivered files | Material tags or variants | Classification evidence |
|---|---:|---:|---|---|
| `idle-pose` | 14 | 14 | Standing/crouched, cover holds, grenade aim hold; all single variants | Names and Unity metadata observed; role inferred |
| `continuous-locomotion` | 34 | 68 | 34 in-place + 34 root-motion; walk/run/sprint/crouch/cover-strafe | Names, paired timing/skeletons, zero-speed in-place measurements |
| `locomotion-transition` | 30 | 60 | 30 in-place + 30 root-motion; turns/pivots and cover entry/exit | Names and counterpart evidence; exact transition chains not declared |
| `airborne` | 9 | 13 | 4 paired takeoffs + 5 single fall/apex/landing motions | Names and delivered variants; chain order inferred |
| `traversal` | 2 | 4 | Left/right 1 m obstacle motions, each paired | Names and paired delivery; environment contract absent |
| `action-interaction` | 18 | 18 | Cover peek transitions and grenade aim/throw actions; single variants | Names/metadata observed; body scope and contact semantics unknown |
| `reaction-death` | 0 | 0 | Not delivered | Exhaustive manifest |
| `emote-cinematic` | 0 | 0 | Not delivered | Exhaustive manifest |
| `other-unknown` | 0 | 0 | No per-motion file remained unclassified | Exhaustive manifest; combined take excluded from per-motion total |
| **Total** | **107** | **177** | **70 in-place + 70 root-motion + 37 single files** | Validated `evaluation-manifest.json` |

### Runtime-set inventory

Runtime sets are recorded separately because they are many-to-many relationships, not clip categories. The manifest retains ten candidate sets; it does not manufacture transition, mask, or contact sets where member order or composition is not authoritative.

| Runtime set | Type | Members/variants | Intended relationship | Grouping evidence | Validation status |
|---|---|---|---|---|---|
| Walk/run/crouch × in-place/root-motion | 6 `directional-blend` sets | 8 files each | Six 8-way locomotion rings | Medium confidence: filenames, common skeleton/timing, phase measurements | **Finding/partial:** all six raw phase spreads exceed 0.15; three in-place sets were anchored; no engine blend test |
| Forward walk/run/fast-run × in-place/root-motion | 2 `speed-blend` sets | 3 files each | Forward speed interpolation | Low confidence: naming and skeleton only | **Not evaluated:** no speed-group declarations or runtime test |
| Sprint direction × in-place/root-motion | 2 `directional-blend` sets | 3 files each | Forward/left/right sprint interpolation | Low confidence: naming and skeleton only | **Not evaluated:** no gait/sync declaration or runtime test |

### Pipeline-stage coverage

Pipeline stages describe completed work and remaining decisions; they are not readiness grades. A completed inspection can still produce a finding.

| Stage | Coverage state | Pack result or required decision | Evidence / next gate |
|---|---|---|---|
| Acquire | `partially-evaluated` | Local commercial archive is available and hashed; user states it was downloaded from Protofactor.biz, but the exact edition, receipt, and historical license revision are unavailable | Confirm purchase/license record |
| Preserve raw | `evaluated-clean` | Immutable RAR retained; extraction and generated outputs are separate | Source and output hashes retained |
| Inspect | `evaluated-finding` | All 179 FBX files inspect/measure/lint; 12 have negative-time failures and three skeleton signatures exist | 0.2.1 exhaustive evidence |
| Segment | `partially-evaluated` | 177 per-motion files are atomic candidates; the combined take lacks a complete range manifest | Ignore combined take or obtain authoritative segmentation |
| Root motion | `partially-evaluated` | 70 counterpart pairs measured; translation pairing is strong | Establish yaw intent, extraction, and controller ownership |
| Conform | `partially-evaluated` | Slicing and gait anchoring trialed; remaining seams, retargeting, deformation, and attachments unresolved | Engine and artist review |
| Validate | `partially-evaluated` | Exhaustive mechanical and provisional semantic checks completed | Selected profiles and runtime behavior remain incomplete |
| Optimize | `partially-evaluated` | Constant-track opportunities measured; pruning trialed but not approved | Prove property coverage, transition equivalence, and runtime benefit |
| Export | `not-evaluated` | No production engine-facing export/import route completed | Choose runtime and test exact handoff |
| Gate/report | `evaluated-clean` | Commands, configs, manifests, digests, and validated report retained | Re-run after any declaration or generated-asset change |

### Readiness ladder by clip set

`File-ready` below means AnimSmith's mechanical checks ran: `nan`, `time-monotonic`, `quat-norm`, `quat-flip`, `duration-sanity`, `scale-keys`, `non-uniform-scale`, and `constant-track`. A constant-track note is hygiene evidence, not a playback failure. `Clip-ready` uses the provisional Unity loop flags and `_RM` naming declarations; bad declarations can create findings even when a one-shot is usable as a one-shot.

#### File-ready and clip-ready

| Primary role / runtime set | File-ready: mechanical | Clip-ready: declared semantics |
|---|---|---|
| `idle-pose` | **Clean with hygiene notes:** 14/14 have no mechanical error; all have constant tracks. | **Finding:** 6/14 fail provisional loop contracts, including six angular seam and two linear/closure failures. Likely impact: recurring twitch or pulse where looping is intended. |
| `continuous-locomotion` | **Clean with hygiene notes:** 68/68; all have constant tracks. | **Finding:** 64/68 fail provisional contracts; all 64 loop-marked files fail angular/linear seam continuity, 34 fail closure, and two low-displacement `_RM` strafes need yaw review. Likely impact: wrap pop, foot pulse, or controller disagreement. |
| `locomotion-transition` | **Finding untouched:** 10/60 have 30 negative-time errors; declared slicing removes them. | **Partial/finding:** 36/60 fail provisional contracts, but 25 loop flags are suspect for one-shot turns/transitions; 12 low-translation `_RM` turns need yaw semantics. |
| `airborne` | **Clean with hygiene notes:** 13/13. | **Partial/finding:** only the loop-marked falling clip fails seam continuity; takeoff/landing trajectory and contacts remain undeclared. |
| `traversal` | **Clean with hygiene notes:** 4/4. | **Finding under provisional metadata:** all four obstacle files are loop-marked and fail seams; these likely need one-shot reclassification plus environment/root alignment tests. |
| `action-interaction` | **Finding untouched:** 2/18 have six negative-time errors; declared slicing removes them. | **Partial/finding:** eight loop-marked grenade actions fail seams and are likely one-shots; body scope, masks, hand contacts, and events were not declared. |
| Zero-count roles | `reaction-death`, `emote-cinematic`, and `other-unknown` are `not-applicable` to file checks. | Absence is content coverage, not a file defect. |

#### Set-ready and rig/use

| Primary role / runtime set | Set-ready: sync/blend prerequisites | Rig/use prerequisites | Practical result |
|---|---|---|---|
| Six 8-way directional sets | **Finding:** matched timing, but raw phase spread is 0.463–0.716. Likely impact: blended feet occupy different stride moments and may skate. Three in-place rings anchor to 0.050–0.094; three root-motion rings remain untouched. | 56-bone roles resolve; target deformation, contacts, and runtime sync are untested. | **Best prototype subset after current AnimSmith**, pending loop and engine blend review. |
| Four 3-member speed/sprint sets | **Not evaluated:** low-confidence filename groupings; no speed, gait, or sync contracts. | 56-bone skeleton; controller speeds and phase policy unknown. | **Candidates only**, not verified blend sets. |
| `locomotion-transition` | **Not evaluated as chains:** turns and cover transitions have no authoritative ordering or crossfade contract. | Mixed 56/73-bone signatures; root yaw, retargeting, and controller ownership unknown. | **Prototype as curated one-shots** after slicing/reclassification. |
| `airborne` + `traversal` | **Not evaluated as chains:** takeoff → fall → landing and obstacle sequences were not composed. | Ground height, collision, root trajectory, and contacts unknown. | **Prototype only** with controller/environment integration. |
| `action-interaction` + `idle-pose` | **Not evaluated for masks/composition:** grenade aim → throw → recovery and locomotion overlays were not tested. | 56/73-bone variants, baked full-body tracks, no additive base; attachment scale unavailable. | **Full-body prototype actions only**; layered weapon readiness is unknown. |

Across all roles, AnimSmith 0.2.1 found no `nan`, quaternion, duration-sanity, animated-scale, or non-uniform-scale error. Every file has constant tracks. That does not make pruning a default fix: 0.2.1 explicitly distinguishes standalone motion equivalence from runtime transition coverage.

### Tooling frontier

| Problem and likely impact | Untouched | After captured AnimSmith 0.2.1 | Plausible future generic tooling | Still left for engine / artist / vendor |
|---|---|---|---|---|
| Negative pre-roll in 12 files: strict loaders may reject it or clamp-hold an unintended pose | 36 `time-monotonic` errors | **Resolved mechanically:** declared slice to Unity's frame range removes all 36 errors | File-qualified manifest orchestration could apply the current declared transform automatically. This is deterministic but still removes out-of-range authored data; it is not lossless. | Verify the chosen range after engine import; artist/vendor only if the Unity range is not authoritative. |
| Directional gait phases disagree: same-time blending can produce foot skate | Six raw rings exceed the 0.15 phase-spread target | **Partly resolved:** gait anchoring brings three tested in-place rings to 0.050–0.094 | File-qualified cross-file gait groups would make measurement and batch application more direct. This is non-destructive analysis plus the existing declared cyclic time rotation, not invented motion. | Test all six runtime blend spaces, contacts, and transition behavior; retime/re-author only if phase rotation is insufficient. See [AnimSmith](https://github.com/mmannerm/animsmith/blob/b6d0f9a5b06d8e5f907fbb87dc6d07ec55525b47/docs/game-ready-clips.md#feet-skate-when-clips-blend), [Unity](https://docs.unity3d.com/6000.5/Documentation/Manual/class-BlendTree.html), [Unreal](https://dev.epicgames.com/documentation/unreal-engine/animation-sync-groups-in-unreal-engine), and [Godot](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html). |
| Loop endpoint/velocity discontinuity: visible pop, hitch, or once-per-cycle pulse | Widespread under provisional Unity loop flags | **Diagnosed, not repaired:** gait anchoring changes phase, not arbitrary endpoints/tangents | A general source-preserving fix is **not plausible**: changing endpoint poses or derivatives changes motion and requires intended-cycle judgment. A narrowly declared/provable duplicate-endpoint drop already exists. | First correct loop semantics. Genuine C0/C1 failures require artist/source work or an explicitly accepted engine loop blend. See [AnimSmith's loop explanation](https://github.com/mmannerm/animsmith/blob/b6d0f9a5b06d8e5f907fbb87dc6d07ec55525b47/docs/game-ready-clips.md#the-loop-pops). |
| Constant channels: larger source/evaluation payload, but predictable dense transition coverage | All 177 files contain notes | Experimental pruning shrinks sampled GLBs but is **not approved** for this pack | Property-scoped pruning and emitted `(bone, property)` coverage are plausible, deterministic improvements tracked by [#401](https://github.com/mmannerm/animsmith/issues/401) and [#402](https://github.com/mmannerm/animsmith/issues/402). | Each runtime must define reset/hold semantics and demonstrate transition equivalence and actual performance benefit. |
| Rotation-only/low-displacement root motion is under-characterized: root yaw may be ignored or double-applied | 14 `_RM` files fail a translation-speed interpretation | Existing measurement exposes horizontal speed but does not settle yaw intent | Additional generic root translation/yaw measurements and declarations are deterministic, non-destructive diagnostics. Automatic root-motion conversion is not generally lossless and remains inappropriate without a stronger contract. | Configure/test root extraction and controller ownership per runtime; artist fix if the authored root path is wrong. |
| Weapon/attachment scale is unknown: a weapon could inherit the wrong size or a non-uniform transform | No dedicated socket contract; no animated-scale errors | A hand-node `rest-world-scale` trial on all three skeleton variants produced six `measurement_unavailable` gaps because FBX source-node transform evidence is unavailable | Extending FBX loading to expose source-node rest-world transforms is a plausible format-adapter improvement; the core check already exists and is non-destructive. | Engine socket creation, grip orientation, two-hand alignment, deformation, and target-character scale still require runtime/artist review. |
| Generic embedded name `Take 001`: cross-file set contracts are awkward | 177 files depend on filename/Unity metadata for identity | External per-file configs and aggregation work, but are cumbersome | File-qualified clip identity/group declarations are deterministic, format-neutral, and non-destructive; no matching public issue was found. | Human/vendor must still define intended categories, loop semantics, and gameplay relationships. |

### Validation-profile status

Profiles capture capability-oriented game uses without pretending that one universal game contract exists. `Evaluator-selected` rows are exploratory hypotheses: they may establish caveats or unknowns, but do not penalize the pack for unrelated missing content.

| Validation profile | Selection and activation basis | Result | Evidence boundary / next test |
|---|---|---|---|
| Marketplace intake | `selected` — `user-required` | **Partial/finding:** complete file/tool intake; historical license revision, archive edition, and all engine behavior remain incomplete | Locate the transaction/license record and complete runtime import |
| Blended locomotion | `selected` — `observed-pack-capability` | **Finding/partial:** six 8-way rings have raw phase mismatch; three in-place rings were anchored | Test all six in engine with contacts, loop policy, and transitions |
| Root-motion controller | `selected` — `observed-pack-capability` | **Partial:** 70 counterpart pairs measured; translation pairing is strong | Measure/declare yaw, extraction, controller ownership, and turn behavior |
| State-machine transitions | `selected` — `observed-pack-capability` | **Partial/not evaluated at runtime:** 30 canonical transition motions plus airborne/action boundaries; chains not authoritative | Declare chains and test crossfades, contacts, interruption, and recovery |
| Layered upper body/weapons | `selected` — `evaluator-selected-generic-scenario` | **Not evaluated:** grenade content makes the scenario material, but delivered tracks are full-body and no additive/mask contract exists | Test masks, base pose, spine boundary, sockets, grip, IK, and retargeting |
| Traversal/environment | `selected` — `observed-pack-capability` | **Partial:** airborne and obstacle files inspected; no composed controller/environment test | Test trajectories, alignment, collision, contacts, and handoff |
| Contact actions/interactions | `selected` — `observed-pack-capability` | **Not evaluated at contact level:** grenade/cover motions exist | Test release/contact timing, prop alignment, events, and recovery |
| Retargeted/customizable characters | `selected` — `observed-pack-capability` | **Partial:** three signatures and Humanoid roles identified; no target-mesh deformation test | Run representative idle, extreme locomotion, traversal, and grenade poses through the target retargeter |
| Motion matching/search | `not-selected` | **No suitability result:** no target system or database contract supplied | Select only with trajectory/contact/metadata requirements |
| Networked movement | `not-selected` | **No suitability result:** no authority, prediction, rollback, or replication requirements | Select with a named controller/network model |
| Runtime performance | `selected` — `evaluator-selected-generic-scenario` | **Partial:** constant tracks measured and pruning trialed but not approved | Measure imported memory/cost and prove transition equivalence on target hardware |

### Common-engine status

The complete engine-capability study and prototype matrix is deliberately deferred for the next report iteration. Official documentation below explains why phase/sync prerequisites matter; it does not establish that this pack works in any runtime.

| Runtime | Evidence level | Pack result | Documented context and next evidence |
|---|---|---|---|
| Unity | Attempted import | **Deferred / no pack result:** 6000.3.6f1 stopped before import because no Editor license was available. | [Blend Trees require similar timing and aligned normalized-time contacts](https://docs.unity3d.com/6000.5/Documentation/Manual/class-BlendTree.html). Activate the Editor, then test import, Humanoid retarget, loops, all six rings, root motion, masks, attachments, and compression. |
| Unreal Engine | Documentation context only | **Deferred / not tested.** | [Sync Groups document foot-placement and length constraints](https://dev.epicgames.com/documentation/unreal-engine/animation-sync-groups-in-unreal-engine). Research the current FBX import/retarget/root-motion/layer capabilities, then prototype representative sets. |
| Godot | Documentation context only | **Deferred / not tested.** | [AnimationTree documents blend spaces, cyclic sync, filters, and missing-track reset behavior](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html). Research current import/root-motion behavior, then prototype representative sets. |
| Bevy | Documentation context only | **Deferred / not tested.** | The official [AnimationGraph example](https://bevy.org/examples/animation/animation-graph/) demonstrates weighted clip blending, not pack compatibility. Research current glTF targets, transitions, masks/additive support, and root-motion ownership before prototyping. |

### Best fit

- Unity Humanoid third-person prototypes needing broad general locomotion plus jumps, landings, cover, obstacle, and grenade actions.
- Projects that can choose between controller-driven in-place motion and `_RM` root-motion variants per action.
- Teams willing to maintain an explicit clip manifest, correct loop classifications, preprocess a bounded subset, and validate on their own Avatar and controller.

### Poor fit or material caveats

- A production build expecting import-and-ship behavior with no curation.
- Motion matching/search and networked movement were not selected as evaluation profiles, so this report makes no suitability claim for them. First-person arms and precision paired-contact gameplay likewise require project-specific profiles and runtime evidence.
- Additive aim/recoil or drop-in upper-body overlays: the delivered metadata defines no additive reference pose, the files contain baked full-body channels, and runtime masks were not tested.
- Exact-skeleton pipelines that cannot use Unity Humanoid retargeting: the archive has three skeleton signatures (56, 73, and 58 bones).
- A workflow built around the single combined 319.667-second FBX: the bundled text list covers only part of its range, while its Unity metadata declares one much shorter clip.

### Adoption conditions

1. Locate the Protofactor.biz receipt/download record and the license revision that governed it. Confirm that the current EULA's single-individual license and protected-application distribution terms fit the intended team and release; current web terms are not proof of the historical transaction.
2. Complete the deferred Unity, Unreal Engine, Godot, and Bevy documentation/prototype matrix; prioritize the engine actually intended for production and test on its target character/controller.
3. Slice the 12 negative-time files from the authoritative Unity clip frame range, or obtain corrected sources from the author.
4. Define the true loop set. Remove loop semantics from one-shots, then review and either artist-fix or deliberately crossfade the remaining locomotion seam failures.
5. Phase-align all six 8-way walk/run/crouch rings with AnimSmith gait anchoring or explicit engine phase mapping, then test blend spaces and foot contacts.
6. Use Unity Humanoid Avatar configuration when mixing the standard and cover/grenade skeleton variants; validate deformation and prop/attachment behavior.
7. Keep constant-track pruning disabled for shipment until emitted `(bone, property)` coverage, runtime reset behavior, and transition equivalence can be proved.

## Evaluation scope and evidence

| Field | Value |
|---|---|
| Pack | Local `Animset@BasicLocomotion_PACKAGE.unitypackage`; edition/version not declared in the archive |
| Vendor/source | Protofactor; [current Basic Locomotion product page](https://protofactor.biz/product/animset-basic-locomotion/) |
| Access | Locally held commercial archive inside “Protofactor Ultimate Animation Collection”; user states it was downloaded from Protofactor.biz |
| Price observed | Current Basic Locomotion page: USD 14.99; current [Protofactor Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/): USD 259.99; current [Unity listing](https://marketplace.unity.com/packages/3d/animations/ultimate-animation-collection-195459): USD 249.99 on 2026-08-16. None proves the local artifact's edition or purchase terms. |
| Delivered scope | Full local RAR → one Unitypackage → 179 FBX files, including 177 per-motion FBX files, one combined animation FBX, and one skinned reference FBX; materials/textures and Unity metadata also delivered |
| Target game/use | Game-engine use only; no specific game, camera, character, controller, platform, networking model, or quality bar supplied |
| Target engines | Broad matrix now includes Unity, Unreal Engine, Godot, and Bevy. Unity 6000.3.6f1 was attempted but blocked by activation; the complete four-engine research/prototype pass is deferred. |
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
| Engine imports | 1 native Unity route attempted | 0 completed | Editor identity captured; activation failed before project creation | Importer warnings, Avatar validity, playback, compression, package conflicts |
| Blend/mask/retarget tests | 3 directional rings measured offline | 3 phase sets measured; 0 runtime tests | Raw cross-file phase mismatch; current gait anchor aligned phases | Runtime blend spaces, AvatarMask, additive, crossfade, deformation, target-rig retarget |

### Claim legend

Use: `user-stated`, `observed-file`, `observed-animsmith`, `observed-report`,
`observed-engine`, `vendor-stated`, `documentation-stated`, `inferred`, and
`not-evaluated`.

## Pack inventory and content coverage

### Delivery and organization

The immutable source is `<authorized-local-source>/Animset@BasicLocomotion_ASSET.rar` (174,531,814 bytes; SHA-256 `6f821f56f84339ea1eb6fcaa97e3c70d4a38dd84c413012847f026748dff185f`). It contains one 178,333,789-byte Unitypackage dated 2023-04-25 internally. Extraction occurred only in the separate evaluation workspace; no source bytes were changed. Licensed source and derived evidence remain local and are identified here by portable labels and digests rather than machine-specific paths.

The reconstructed Unity logical tree contains 179 FBX payloads, 7 PNG textures, 2 material files, 1 animation-list text file, and 196 `.meta` files. Of the FBX files, 177 are named per motion, one is a combined animation take, and one is a skinned reference actor. Every imported animation FBX has Unity `animationType: 3` (Humanoid). The 177 per-motion assets copy the supplied reference Avatar; the combined file uses older metadata.

Organization is usable but not clean enough to be its own production contract:

- Every FBX exposes the generic embedded take name `Take 001`; meaningful names live in filenames and, for 164 files, Unity clip metadata.
- Fifteen per-motion files lack explicit `clipAnimations` metadata even though their FBX take is readable.
- Unity metadata is mixed across serialized versions 19301, 20300, and 23.
- The combined FBX is 9,591 frames / 319.667 seconds by measurement. The bundled animation list ends at frame 6,785, and the combined-file `.meta` declares only frames 0–2,211. The remaining ranges have no complete authoritative segmentation manifest.
- Typos such as `Standingt` and `Spint`, a walk filename with `Forward` misspelled, and inconsistent casing in `runForward2` and `Runbackwards` increase the cost of filename-driven automation.

### Animation/gameplay coverage

| Family | Delivered clips/variants | Intended use | Material gaps for this game | Evidence |
|---|---|---|---|---|
| Idle/locomotion | Idle variants; 8-way walk, run, and crouch rings; sprint/fast-run; in-place and many `_RM` pairs | General third-person locomotion and blend trees | Raw ring phases are misaligned; loop seams and target speeds require runtime validation | observed-file, observed-animsmith |
| Starts/stops/pivots/transitions | 90°/180° turns, U-turns, forward turns, cover entry/exit/peek/strafe transitions | Direction changes and contextual cover transitions | No clearly named general locomotion start/stop family; coverage is naming-inferred | observed-file, inferred |
| Jump/traversal | Jump-to-apex, walk/run takeoffs by foot, falling, light/medium/heavy landing, 1 m obstacle passes | Basic airborne state machine and low obstacle traversal | No apex-to-land matching, trajectory, contact, ledge, vault-height, or controller test | observed-file, not-evaluated |
| Combat/actions/interactions | Grenade aim/idle/throws plus cover-specific throws | Simple grenade action and cover gameplay | No melee, firearms, hit reactions, deaths, paired interactions, or contact-event metadata | observed-file |
| Additive/aim/masked layers | Grenade aim/action poses are present as full-body clips | Possible override layer after authoring/configuration | No additive reference pose; no arm-only files; masks and IK were not tested | observed-file, not-evaluated |
| Reactions/death/other | Look-around, scratch/yawn, breathing | Ambient variation | Reactions/death are outside delivered scope | observed-file |

## Out-of-the-box results

### Summary scorecard

| Readiness lane | Verdict | Evidence | Adoption consequence |
|---|---|---|---|
| Acquisition and rights | Unknown | user-stated: downloaded from Protofactor.biz; observed-file: no local license/receipt; documentation-stated: current Protofactor EULA only | Locate the transaction record and historical EULA revision before distributing a game build. |
| Delivery completeness/organization | Conditional | observed-file: full Unitypackage, but incomplete combined-file segmentation and 15 missing explicit clip definitions | Prefer per-motion FBX files and maintain a project manifest. |
| AnimSmith-readable formats | Ready | observed-animsmith: 179/179 inspect and measure success | Mechanical automation can start immediately. |
| Untouched mechanical clip health | Conditional | observed-animsmith: 12/179 files fail monotonic time; others pass default errors | Exclude or slice those 12 before strict-pipeline use. |
| Declared clip semantics | Conditional | observed-file: generic `Take 001`; 111 loop flags include likely one-shots | Curate per-file names, loop status, in-place/root policy, and set membership. |
| Set/sync/locomotion behavior | Conditional | observed-animsmith: matched ring timing but raw phase spread 0.463–0.716 | Phase-align or use engine phase offsets; validate seams and contacts. |
| Rig/rest/bind/retargeting | Conditional | observed-animsmith: humanoid role resolution succeeds; three skeleton signatures | Use Humanoid retargeting and validate the actual character. |
| Root motion/in-place behavior | Conditional | observed-animsmith: 70 complete same-skeleton pairs; no non-RM clip exceeds 0.5 m/s horizontal speed | Rotational/low-displacement root motion is under-characterized by the current speed check. |
| Target-engine import/playback | Unknown | not-evaluated: Unity activation failure | Blocks production recommendation. |
| Masks/additive/IK/attachments | Unknown | observed-file: full-body tracks, no additive reference; not-evaluated runtime masks | Treat actions as full-body overrides until proven otherwise. |
| Performance/runtime footprint | Unknown | observed-file: baked constant tracks; no engine memory/CPU/build-size test | Do not infer runtime cost from source or GLB size. |
| Game/content/artistic fit | Conditional | observed-report: plausible static samples; no stated game or full playback | Prototype against camera, controller, and quality bar. |
| Cross-pack compatibility | Unknown | no external pack/rig supplied | Run pairwise skeleton, scale, root-policy, timing, semantic, and runtime tests. |
| Maintainability/reproducibility | Conditional | observed-file and observed-animsmith: exhaustive manifests/results retained; naming/meta inconsistencies remain | Add a canonical project-side clip manifest and pin the evaluator. |

### Untouched import and playback

Untouched **engine** import/playback is not evaluated. Unity 6000.3.6f1 was invoked in batch mode with a disposable project/import-package route. It exited 198 before creating the project because the installed Editor had no valid license. This is an evaluator-environment blocker, not a pack failure.

Untouched **offline** loading is strong: AnimSmith inspected and measured all 179 FBX files. Nine representative offline HTML reports were rendered at frame 0 and an injected midpoint and visually reviewed. Idle, walk, root-motion walk, side run, jump, landing, cover, and grenade samples showed coherent static humanoid poses without gross explosions. Root-motion walk showed a trajectory while its in-place partner remained stationary. The negative-time cover sample surfaced its errors. The 319.667-second combined take produced a visually dense, incoherent trajectory/transition report as expected for many actions presented as one clip.

These static reports do not prove artistic motion quality, foot contacts, loop smoothness, blend behavior, Avatar masks, mesh deformation, scale, controller response, or engine import correctness.

### Untouched AnimSmith findings

| Finding or coverage gap | Affected scope | User-visible effect | Evidence |
|---|---|---|---|
| Non-monotonic negative-time keys at −0.033333335 s on translation/rotation/scale of `root_CoverUnarmedAnimset` | 12 files; 36 errors | Strict import/processing may reject or mishandle pre-roll; timelines do not start cleanly at zero | observed-animsmith: `baseline-summary.json` |
| Baked constant tracks | 179 files; 24,186 notes; 99–192 per file, median 137 | Source bloat and dense channel coverage; runtime cost unmeasured | observed-animsmith |
| Unity-declared loop closure failures | 58 of 111 declared loops; 84 errors | Possible pose pops or contact mismatch at wrap | observed-animsmith; declaration partly observed-file |
| Unity-declared loop seam velocity/angular-velocity failures | 104/108 of 111 declared loops | Possible visible speed/rotation discontinuity at wrap | observed-animsmith; many loop declarations appear semantically questionable |
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

## AnimSmith results

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
| Misaligned 8-way gait phases | `transform --gait-anchor` on 24 in-place walk/run/crouch files using per-file loop/in-place/humanoid declarations | 24/24 transforms succeeded; phase spreads reduced to walk 0.072, run 0.094, crouch 0.050 | All inspect/measure and fix dry-runs exit 0; only 2/24 strict contract lints exit 0; remaining errors are loop closure/velocity/angular seam findings | Small/medium automated step plus review | Phase alignment is improved, but it does not repair loop endpoints or prove blend quality. |
| Baked constant tracks | `transform --prune-constant-tracks` on a standard walk, cover clip, and combined file | 3/3 transforms succeeded; output/source byte ratios 12.3%, 8.7%, and 41.8% | All outputs inspect/measure/lint and fix dry-run exit 0; all `diff` runs exit 1 with large/index-sensitive measurement deltas | Small to run, high proof burden | Do not adopt from this trial. Dense transition coverage and semantic equivalence are not proven. |

The slice and gait-anchor operations are **current declared transforms**: their behavior depends on explicit clip range or gait semantics rather than an inferred artistic rewrite. AnimSmith does not currently repair genuine loop pose/velocity seams, retarget a rig, create additive motion, fix contacts, or author missing animation.

Current public issues [#401](https://github.com/mmannerm/animsmith/issues/401) and [#402](https://github.com/mmannerm/animsmith/issues/402) document why pruning requires property-scoped policy and emitted `(bone, property)` coverage. No matching public issue was found on 2026-08-16 for cross-file clip identity/group contracts or Unitypackage ingestion; those are potential ideas, not roadmap commitments.

### Before/after conclusion

Current AnimSmith makes the 12 strict-time failures mechanically usable under a declared frame-range policy and makes the three core directional rings phase-compatible by measurement. It does **not** turn the pack into a production-certified asset automatically. The post-anchor loop failures still require semantic reclassification, engine transition policy, or artist correction. The pruning trial demonstrates potential storage reduction but fails the proof bar and is excluded from the recommended pipeline.

## Engine integration

### Import configuration

Native Unity delivery was selected because the source is a Unitypackage and all metadata declares Unity Humanoid animation. The exact installed Editor was Unity `6000.3.6f1 (bbb010bdb8a3)` on Windows 11. Batch-mode `-createProject` plus `-importPackage` was attempted against a disposable path. Unity exited before project creation with `No valid Unity Editor license found. Please activate your license.` No package import, reserialization, or source mutation occurred.

After activation, the minimum import matrix should record:

1. Package import warnings/conflicts and actual logical asset count.
2. Avatar validity for the supplied `SK_Protof-Actor` and a representative file from each skeleton signature.
3. Humanoid retarget to the real target character at project scale.
4. Importer loop/root-transform settings for representative in-place and `_RM` pairs.
5. Default and project compression effects on contacts, fingers, prop bones, and seam behavior.
6. The 12 negative-time sources before and after slicing.

### Runtime playback and root motion

Runtime playback is not evaluated. The file-level pair inventory is favorable: all 70 `_RM` files have a same-skeleton non-RM partner, durations match for every pair, and frame counts match for 68. `GoOutOfCoverRightStanding_RM` is one frame longer than its partner; `IdleTakeCoverCrouchingToIdleStanding_RM` is one frame shorter.

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

## Compatibility

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

## Issue and remediation register

| ID | Severity | Problem and impact | Primary owner | Current workaround | Future AnimSmith potential | Confidence/status |
|---|---|---|---|---|---|---|
| AP-001 | Major | 12 files contain negative-time transform keys, failing strict monotonic-time validation | animsmith-current-declared | Slice `0:lastFrame/30` at 30 fps from delivered Unity clip metadata | Not needed; current declared slice removed all time errors | High; verified on all 12 outputs |
| AP-002 | Major | Many Unity-declared loops fail strict closure/velocity/angular seam checks; genuine cycles may pop | artist-author | First remove false loop declarations; use deliberate engine crossfades where acceptable | Not suitable for automatic artistic correction under current safety policy; loop-pose distribution is ADR-gated in roadmap [#165](https://github.com/mmannerm/animsmith/issues/165) | Medium; mechanical findings exhaustive, visual/runtime impact unknown |
| AP-003 | Major | Walk/run/crouch direction rings are not raw phase-aligned for direct blends | animsmith-current-declared | Run `--gait-anchor` or configure engine phase offsets | Not needed for phase anchoring; current tool achieved ≤0.094 spread | High for measured phase; runtime blend still unknown |
| AP-004 | Moderate | Three skeleton signatures prevent exact-skeleton interchange across all files | engine-config | Unity Humanoid Avatar/retarget configuration | General retargeting is intentionally outside current automatic rewrite scope; diagnostics could improve | High file evidence; engine result unknown |
| AP-005 | Moderate | All files contain many baked constant tracks; possible source/runtime bloat | animsmith-current-declared | Keep tracks for now; measure target runtime and transition/reset behavior before optimizing | Current pruning preserves standalone sampled motion but may make property coverage sparse; [#401](https://github.com/mmannerm/animsmith/issues/401) and [#402](https://github.com/mmannerm/animsmith/issues/402) track property scope and emitted coverage evidence | High that tracks exist; unknown performance impact; trial not approved |
| AP-006 | Moderate | Embedded name `Take 001` in every file and current file-by-file semantics make cross-file set contracts cumbersome | animsmith-future-candidate | Generate per-file configs and aggregate measurements externally, as this evaluation did | A format-neutral file-scoped identity/group contract could help; no matching public issue found, so no commitment | High current ergonomics evidence; future suitability medium |
| AP-007 | Major | User identifies Protofactor.biz as the download source, but local receipt/license/edition provenance is absent and current listings conflict with local counts | vendor-license | Locate the Protofactor purchase/download record and applicable historical license snapshot, then map them to this archive hash | Not suitable; rights cannot be inferred or repaired mechanically | High that local proof is absent; historical terms unknown |
| AP-008 | Major | Unity import/runtime evidence is missing because Editor activation failed | engine-config | Activate Unity and rerun disposable-project import/test matrix | Not suitable; external engine environment | High; observed log, evaluation incomplete |
| AP-009 | Moderate | Combined FBX, bundled animation list, and Unity clip range disagree; unsliced take is not gameplay-ready | vendor-license / artist-author | Use the 177 per-motion files; only slice combined regions backed by an authoritative manifest | Declared slicing can execute known ranges, but cannot invent missing semantic boundaries | High file evidence |
| AP-010 | Moderate | Fifteen files lack explicit Unity clip declarations; 111 loop flags include likely one-shots | engine-config | Maintain a reviewed project-side clip manifest and override importer flags | Generic manifest validation could help; automatic semantic inference is unsafe | High file evidence; intended semantics partly unknown |

## Acquisition and adoption guidance

### Value and expected work

| State | Usable scope | Required tasks | Effort | Owner |
|---|---|---|---|---|
| Untouched | Broad file-readable prototype pool; most per-motion clips | Exclude 12 strict-time failures, avoid combined take, curate loop semantics, configure Humanoid Avatar, test engine | Medium | Developer/technical animator |
| After current AnimSmith | Adds clean-time variants for all 12 and phase-aligned 8-way in-place rings | Review transformed GLBs, retain explicit provenance, resolve remaining seams, perform engine matrix | Medium | Technical animator/developer |
| Target production state | Only clips that pass target-character playback, blending, contact, mask, root-motion, compression, and performance gates | Artist fixes for unacceptable seams/contacts; project integration; rights confirmation; regression manifests | Medium/high, game-dependent | Artist, technical animator, developer, producer/legal |

### Recommendation rationale

If the team already owns this exact archive, it offers unusually broad prototype coverage and has strong mechanical readability. The current tools can resolve the one unambiguous structural defect class and improve directional phase alignment with bounded, reproducible operations. That is enough to justify a prototype evaluation in Unity.

It is not enough to approve production use or a new purchase based solely on this artifact. Although the user identifies Protofactor.biz as the download source, the local archive does not match the current vendor page's advertised counts, the applicable historical license/receipt is not retained with it, runtime engine evidence is absent, and genuine cyclic seams may need source-level work. Do not use the current USD prices as a value calculation for the local edition. If considering a new purchase, obtain the current package manifest/version and license from the storefront and rerun this skill against that delivered artifact.

## Limitations and unknowns

1. No target game, engine project, character, controller, camera, platform, frame budget, networking policy, or artistic quality bar was supplied; suitability conclusions are deliberately generic.
2. Unity import/playback levels were not evaluated because Editor activation failed before project creation. This is the largest confidence limiter.
3. Static report samples cannot establish motion quality, contacts, deformation, loop perceptibility, blending, masking, or compression behavior.
4. Root-motion classification partly relies on the `_RM` filename convention; speed-only checks do not characterize rotational or low-displacement root motion.
5. Contract loop declarations come from Unity metadata, which appears to over-label one-shots. Counts should not be interpreted as 108 visually bad gameplay cycles.
6. Cross-file phase spread was aggregated from per-file AnimSmith gait measurements because the current embedded clip identities/config model cannot directly express these groups across files.
7. The constant-pruning sample is not approved: source/output format differs, `diff` reports many deltas, and emitted per-property coverage is not currently available from the evaluator.
8. No malware/security audit beyond archive path/structure inspection was performed; no executables or scripts were found in the logical pack content.
9. No current vendor download was acquired, so the local artifact cannot be equated to the 2026 product listing.
10. No full artistic review of all 177 motions was conducted; nine representative offline reports were sampled.

## Reproduction appendix

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

Attempted procedure:

1. Invoke Unity 6000.3.6f1 in batch mode with `-createProject` on a disposable evaluation path.
2. Import the extracted Unitypackage with `-importPackage`.
3. Capture the Editor log and exit code.

Observed result: exit 198 before project creation/import because no valid Unity Editor license was found. After activation, rerun this route and then build a scene containing the source actor and target character, an Animator Controller with the 8-way rings and representative transitions, root-motion toggles, AvatarMask layers, and profiler/build measurements.

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
| `evidence/unity-6000.3.6f1-import.log` | Unity identity and activation blocker | SHA-256 `f98e0ae9f29f5cf1c2f37bd85ab71ce5ef906c9eb7884cdce321e0a64bca617b` |
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
