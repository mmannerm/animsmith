# Animation pack evidence appendix: Mixamo Pistol-Handgun Locomotion

> Companion report: [Technical report](mixamo-pistol-handgun-locomotion.md)
>
> Evidence status: **partial** — exhaustive current source mechanics and archive-level declarations; no current engine, visual, contact, retarget, license, or artistic acceptance.
>
> Evaluation date: **2026-09-16**
>
> Current evaluator: **AnimSmith 0.14.0**
>
> Report format: **3**

This appendix contains scrubbed facts only. The [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder) remains authoritative. The evaluation manifest schema is `urn:animsmith:skill:animation-pack-evaluation-manifest:1`; it is retained as opaque historical classification input, not current source-to-vendor mapping authority.

## Evaluation scope and provenance

| Field | Value |
|---|---|
| Pack/edition | Mixamo Pistol-Handgun Locomotion; local revision unknown |
| Vendor/source | Vendor identity observed in delivered metadata; listing URL not retained |
| Delivered scope | 29 extracted FBX files: 8 in-place-directory and 21 root-motion-directory files |
| Target use | Engine-neutral intake plus a new evaluator-proposed kinematic-controller scenario |
| Target engines | Not evaluated |
| Target rigs/packs | Mixamo profile resolved; no target character or cross-pack runtime supplied |
| Source manifest | Fresh 249-file collection inventory SHA-256 `bc8543a8e1c7cf439030c07a2aead5db183d610633f637c1979dbce71a4dce66`; identical before/after evaluation |
| Evaluation manifest | `mixamo-pistol-handgun-locomotion.manifest.json`; SHA-256 `5fbc91e885d4dbdacf25a67c26833a0c9f6c48b410555b67c5cd0843e7bbd47b`; legacy schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1` |
| Acquisition/license provenance | Extracted files are locally authorized; original ZIPs and controlling license evidence were unavailable; not legal advice |

The legacy metadata count of 27 motions is not linked authoritatively to these 29 files. Current conclusions use exact source paths and fresh measurements only.

### Evidence coverage

| Surface | Offered/delivered | Evaluated | Findings | Not evaluated and why |
|---|---:|---:|---:|---|
| Animation/source files | 29 | 29 | 0 baseline errors; 7 warnings | Vendor motion mapping unavailable |
| Rigs/export variants | 2 skeleton counts | 29 | two 68-bone `X Bot.fbx`; 27 66-bone files; Mixamo roles resolved | Retarget/deformation unavailable |
| AnimSmith baseline | 29 | 29 | 7 warnings; 4122 notes | — |
| Declared contracts | 29 | 29 | 5 ownership errors; 7 warnings | Per-file project intent unavailable |
| Evaluator scenario | 1 | 1 static proposal | 2 members with measured gait phase | Loop, synchronization, contact, engine, and visual acceptance unavailable |
| Engine import/playback | 4 common runtimes | 0 | 0 | No current engine project ran |
| Blend/mask/retarget | 1 proposed set | 0 runtime tests | 0 | Target graph/character unavailable |

### Claim legend

Consequential claims are `observed-file`, `observed-animsmith`, `inferred`, or `not-evaluated` under the assessment taxonomy.

## Evaluation manifest and taxonomy

### Canonical clip-role inventory

| Canonical primary role | Logical motions | Delivered files | Evidence boundary |
|---|---:|---:|---|
| `idle-pose` | 0 | 0 | Not authoritatively classified |
| `continuous-locomotion` | 0 | 0 | Filename-based hypotheses stay outside the canonical role count |
| `locomotion-transition` | 0 | 0 | Not authoritatively classified |
| `airborne` | 0 | 0 | Not authoritatively classified |
| `traversal` | 0 | 0 | Not authoritatively classified |
| `action-interaction` | 0 | 0 | Not authoritatively classified |
| `reaction-death` | 0 | 0 | Not authoritatively classified |
| `emote-cinematic` | 0 | 0 | Not authoritatively classified |
| `other-unknown` | 29 | 29 | One opaque current source unit per delivered FBX |
| **Total** | **29** | **29** | Current exact source paths; no vendor mapping inferred |

### Exact runtime members

| Set/profile | Role or coordinate | Exact members | Variant/type | Timing or motion | Runtime contract |
|---|---|---|---|---|---|
| `hypothesis/kinematic-run-axis` | forward=(0,1) | `Pistol-Handgun_Locomotion_Pack_-_in-place/pistol run.fbx` | variant=in-place | duration=0.500 s; frames=16 frames | loop=unknown; sync=not-evaluated; movement=controller-xz-yaw; playback=project-defined |
| `hypothesis/kinematic-run-axis` | back=(0,-1) | `Pistol-Handgun_Locomotion_Pack_-_in-place/pistol run backward.fbx` | variant=in-place | duration=0.533 s; frames=17 frames | loop=unknown; sync=not-evaluated; movement=controller-xz-yaw; playback=project-defined |

The table above preserves the measured member detail from the companion report; its proposed controller still requires target-engine acceptance.

### Runtime-set inventory

| Runtime set | Type | Members/variants | Grouping evidence | Validation status |
|---|---|---|---|---|
| `hypothesis/kinematic-run-axis` | directional-blend | `Pistol-Handgun_Locomotion_Pack_-_in-place/pistol run.fbx`; `Pistol-Handgun_Locomotion_Pack_-_in-place/pistol run backward.fbx`; all `in-place` | new evaluator hypothesis from current source names; lateral semantics remain unavailable; exact durations and frames are in the primary table | `not-evaluated` runtime acceptance |

### Pipeline-stage coverage

| Stage | Coverage state | Evidence / remaining gate |
|---|---|---|
| Acquire | `partially-evaluated` | Extracted source present; original ZIP/license unavailable |
| Preserve raw | `evaluated-clean` | Fresh inventory matched byte-for-byte before/after the read-only matrix |
| Inspect | `evaluated-clean` | All 29 FBXs admitted and resolved the Mixamo profile |
| Segment | `not-evaluated` | Vendor motion-to-file mapping unavailable |
| Root motion | `partially-evaluated` | Archive-level XZ declarations ran; per-file ownership and yaw remain project decisions |
| Conform | `not-evaluated` | No source rewrite was authorized or produced |
| Validate | `partially-evaluated` | Mechanical and declared checks complete; runtime/artistic gates open |
| Optimize | `not-evaluated` | Constant-track notes were not promoted to a rewrite |
| Export | `not-evaluated` | No engine-facing export ran |
| Gate/report | `partially-evaluated` | Source-only decision report complete; adoption gates remain |

### Readiness evidence by clip set

| Role or runtime set | File-ready / clip-ready | Set-ready / rig-use | Runtime / acceptance boundary |
|---|---|---|---|
| Opaque delivered corpus | 0 errors; 7 warnings; all sources readable | Mixamo roles resolved; vendor semantics and retarget use unavailable | No current engine, visual, contact, or gameplay acceptance |
| `hypothesis/kinematic-run-axis` | Exact members and gait phases measured; loop intent unknown | New evaluator hypothesis; controller ownership proposed | Target-engine blend, phase/contact acceptance, transition, and deformation test required |

### Validation-profile status

| Validation profile | Selection | Result / next evidence |
|---|---|---|
| Marketplace intake | `selected` — `observed-pack-capability` | Exhaustive source mechanics complete; acquisition/license boundary remains |
| Blended locomotion | `selected` — `evaluator-selected-generic-scenario` | Exact static proposal and gait-phase measurements recorded; runtime blend and phase/contact acceptance required |
| Root-motion controller | `selected` — `observed-pack-capability` | 5 archive-level ownership conflicts require per-file policy |
| State-machine transitions | `selected` — `evaluator-selected-generic-scenario` | Full-body transition policy proposed; no authored/runtime transition tested |
| Layered upper body/weapons | `not-selected` | No mask/additive contract or layer evidence |
| Traversal/environment | `not-selected` | No target traversal/environment contract |
| Contact actions/interactions | `not-selected` | No contact target or gameplay contract |
| Retargeted/customizable characters | `not-selected` | No target rig or retargeter supplied |
| Motion matching/search | `not-selected` | No database/annotation contract supplied |
| Networked movement | `not-selected` | No replication/rollback contract supplied |
| Runtime performance | `not-selected` | No target hardware/runtime project supplied |

## Pack inventory and content evidence

The fresh inventory contains 29 FBX files. Both directory variants include `X Bot.fbx` with a 68-bone skeleton and an empty `Take 001`; the 27 motion-named files use 66 bones. All sources resolved the same nine Mixamo roles. This establishes static structure, not identical hierarchy/rest pose or retarget compatibility. Constant-track findings are optimization hints only.

The current scenario is intended to be useful without claiming old authority: `hypothesis/kinematic-run-axis` names 2 exact in-place-directory files, records current durations/frame counts, assigns movement to a hypothetical kinematic controller, and leaves loop intent, gait-phase acceptance, transitions, and runtime acceptance open. AnimSmith measured phase (cycles) as forward `0.3981` and back `0.6458`; the normalized circular-range spread is `0.2477` cycles, defined as `1 - largest cyclic gap` across member phases on the unit cycle. These are `observed-animsmith` measurements; `sync=not-evaluated`, and no defect, contact acceptance, or synchronization acceptance follows from the spread alone.

## Mechanical baseline

| Finding/check | Affected scope | Potential impact | Evidence |
|---|---|---|---|
| No baseline errors | 29/29 files | File-readable mechanical prerequisite only | `observed-animsmith`; current ledger |
| `duration-sanity` | 7 findings in 7 files | Empty reference take or shorter channels clamp-held at clip end | `Pistol-Handgun_Locomotion_Pack_-_in-place/X Bot.fbx`; `Pistol-Handgun_Locomotion_Pack_-_root-motion/X Bot.fbx`; `Pistol-Handgun_Locomotion_Pack_-_root-motion/pistol idle.fbx`; `Pistol-Handgun_Locomotion_Pack_-_root-motion/pistol jump.fbx`; `Pistol-Handgun_Locomotion_Pack_-_root-motion/pistol kneel to stand.fbx`; `Pistol-Handgun_Locomotion_Pack_-_root-motion/pistol kneeling idle.fbx`; `Pistol-Handgun_Locomotion_Pack_-_root-motion/pistol stand to kneel.fbx` |
| `constant-track` | 4122 notes | Possible storage/evaluation overhead; no visual defect or safe rewrite inferred | `observed-animsmith` |
| Mixamo role resolution | 29/29 files | Enables measurement; does not prove target-character retargeting | `observed-animsmith` |

## AnimSmith remediation evidence

| Source issue | Operation/declarations | Result | Independent verification | Remaining caveat |
|---|---|---|---|---|
| MIX-OWN-001 | Archive-level in-place and animation-owned XZ declarations | 5 stationary-root errors on `Pistol-Handgun_Locomotion_Pack_-_root-motion/pistol idle.fbx`; `Pistol-Handgun_Locomotion_Pack_-_root-motion/pistol jump.fbx`; `Pistol-Handgun_Locomotion_Pack_-_root-motion/pistol kneel to stand.fbx`; `Pistol-Handgun_Locomotion_Pack_-_root-motion/pistol kneeling idle.fbx`; `Pistol-Handgun_Locomotion_Pack_-_root-motion/pistol stand to kneel.fbx` | Current per-file lint and measurement; no output produced | Per-file intent and controller acceptance unavailable; residual unresolved |
| MIX-TIME-001 | No transform run | Candidate not produced | Current baseline identifies exact channel-end ranges | Artist intent and visual boundary acceptance unavailable; residual unresolved |
| MIX-CONTENT-001 | No transform run | Candidate not produced | Both exact `X Bot.fbx` files retain 68 bones and an empty take | Reference-vs-animation intent unavailable; residual unresolved |

No command availability, configuration, or generated hypothesis is reported as a source repair.

## Engine procedures and evidence

| Runtime | Version | Procedure | Observed result | Remaining gate |
|---|---|---|---|---|
| Unity | unspecified | Not run | `not-evaluated` | Exact import/controller/blend/visual test |
| Unreal Engine | unspecified | Not run | `not-evaluated` | Exact import/retarget/Blend Space test |
| Godot | unspecified | Not run | `not-evaluated` | Exact Skeleton3D/AnimationTree test |
| Bevy | unspecified | Not run | `not-evaluated` | Exact target identity/graph/controller test |

## Rig, masking, and compatibility evidence

| Pack/rig/set pair | Skeleton/retarget | Scale/axes | Root policy | Timing/blend | Overall evidence |
|---|---|---|---|---|---|
| `hypothesis/kinematic-run-axis` / target character | Mixamo roles resolve; target deformation not tested | Not evaluated | Controller XZ/yaw/collision proposed | Exact durations and gait phases measured; synchronization/blend acceptance not tested | `not-evaluated` beyond static source evidence |
| This pack / any other Mixamo constituent | Common role labels and 66-bone count observed on motion files | Not evaluated | Per-file ownership unavailable | No pairwise transition/blend test | `unknown`; no cross-pack claim |

## Limitations and unknowns

1. Vendor mapping from 27 metadata motions to 29 files is unavailable and was not reconstructed from names.
2. Loop intent, gait-phase acceptance, contacts, target character, deformation, engine graph, visual quality, root/yaw policy, and cross-pack behavior remain untested.
3. Original ZIP payloads and controlling license evidence were unavailable; current hashing covers extracted source only.

## Changes between AnimSmith versions

AnimSmith 0.14.0 — Fresh current-source matrix admitted 29/29 FBXs and revalidated baseline and archive-level declarations. New controller topology is explicitly evaluator-proposed and remains runtime-unaccepted.

AnimSmith 0.10.0 — Retained historical counts happen to agree; no prior semantic membership or engine claim was promoted. AnimSmith 0.7.0 evidence remains superseded.

## Reproduction

Official evaluator preflight: [release archive](https://github.com/mmannerm/animsmith/releases/download/v0.14.0/animsmith-v0.14.0-x86_64-unknown-linux-gnu.tar.gz), archive SHA-256 `4ecf79436f9123c779edb004050da3010b44a2b392e3031facb227d7734fc33e`; exact member `animsmith-v0.14.0-x86_64-unknown-linux-gnu/animsmith`, binary SHA-256 `c2b9649bc74f8a7e5b5d7361a6feaa93941b2327cd7eee82235024bd4f61e366`; tag `v0.14.0`, peeled commit `e8321ad40be5ef6f162b31f085819c039175c3c9`; working-tree state: N/A (official archive). Compiled features: `fbx, report`. Version and required command help plus representative FBX admission passed before evaluation. Preflight: `external:animsmith-0.14.0-report-refresh/preflight.json`, SHA-256 `7034b5043de54e7b30064230db18605d1f96d0ea1477f7b8c56ce817cc73521a`.

The current official binary SHA-256 is `c2b9649bc74f8a7e5b5d7361a6feaa93941b2327cd7eee82235024bd4f61e366`, version `0.14.0`, source revision `e8321ad40be5ef6f162b31f085819c039175c3c9`, dirty `false`, features `fbx,report`. Version, top-level help, and `inspect`, `measure`, and `lint` help all exited 0 before the matrix. Representative FBX admission and the exhaustive run then completed.

The read-only four-worker matrix ran `inspect`, JSON `measure`, empty-config JSON `lint`, and variant-declaration JSON `lint` for every source: 116 commands for this constituent and 996 collection-wide. Source inventory SHA-256 `bc8543a8e1c7cf439030c07a2aead5db183d610633f637c1979dbce71a4dce66` covers 249 files / 119,754,377 bytes and matched before/after. Current ledger SHA-256 is `62b0a7b7a1bb47c817f6b005a988077a44dc8d942bb26343a5f7278a15582272`; summary `4dd01ee9f1398fa200355c82206c8fd24d80d0dea1f8965aee8b03ba400cfc7c`; report data `a8d8e1bd3707b58e24f7d08f4ea7038c0f316254fa289e4f6439284dbcbc388f`. Baseline config digests are `e3b0c442...` for the shared empty file or `cf70223...` for the explicit Mixamo baseline; in-place declaration digest `acb638c0...`; root-motion declaration digest `567dd18f...`. Full digests, private paths, outputs, and licensed sources remain external.

## Sources

- Fresh external source inventory, command ledger, current summary, and exact per-command output.
- [AnimSmith game-ready clip guidance](../game-ready-clips.md).
