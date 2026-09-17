# Animation pack evidence appendix: Mixamo Locomotion Collection

> Companion report: [Technical report](mixamo-locomotion-collection.md)
>
> Evidence status: **partial** — exhaustive source mechanics for nine constituents; no current collection semantics, engine, visual, retarget, contact, or artistic acceptance.
>
> Evaluation date: **2026-09-16**
>
> Current evaluator: **AnimSmith 0.14.0**
>
> Report format: **2**

This appendix contains scrubbed rollup facts. The [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder) remains authoritative. The retained collection manifest schema is `urn:animsmith:skill:animation-pack-evaluation-manifest:1`; it does not provide current semantic membership authority.

## Evaluation scope and provenance

| Field | Value |
|---|---|
| Pack/edition | Mixamo Locomotion Collection; nine locally available constituents; revisions unknown |
| Vendor/source | Vendor identity observed in metadata; current listing URLs not retained |
| Delivered scope | 249 extracted FBXs: 92 in-place-directory and 157 root-motion-directory files |
| Target use | Engine-neutral intake plus new evaluator-proposed controller/state hypotheses |
| Target engines | Not evaluated |
| Target rigs/packs | Nine source pools; no target character or proven pairwise combination |
| Source manifest | Fresh collection inventory SHA-256 `bc8543a8e1c7cf439030c07a2aead5db183d610633f637c1979dbce71a4dce66`; identical before/after |
| Evaluation manifest | `mixamo-locomotion-collection.manifest.json`; SHA-256 `9601129c4237e5b331fd7de8c6c705c7bb8dcd702a219d31c250a8ce57b77cf1`; legacy schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1` |
| Acquisition/license provenance | Extracted files locally authorized; original ZIPs and controlling license evidence unavailable; not legal advice |

### Evidence coverage

| Surface | Offered/delivered | Evaluated | Findings | Not evaluated and why |
|---|---:|---:|---:|---|
| Physical FBX files | 249 | 249 | 0 baseline errors; 50 warnings | Vendor 231-motion mapping unavailable |
| Rigs/export variants | 249 | 249 | 231 66-bone motion files; 18 68-bone `X Bot.fbx`; Mixamo roles resolved | Pairwise hierarchy/rest/retarget unavailable |
| AnimSmith baseline | 249 | 249 | 50 warnings; 35405 notes | — |
| Declared contracts | 249 | 249 | 52 ownership errors; 50 warnings | Per-file project intent unavailable |
| Constituent hypotheses | 9 | 9 static proposals | Exact measured members | Runtime/visual acceptance unavailable |
| Cross-pack hypotheses | 1 | 1 static proposal | Full-body handoff only | Pairwise engine/artistic evidence unavailable |
| Engine import/playback | 4 common runtimes | 0 | 0 | No current engine project ran |

### Claim legend

Consequential claims are `observed-file`, `observed-animsmith`, `inferred`, or `not-evaluated`.

## Evaluation manifest and taxonomy

### Canonical clip-role inventory

| Canonical primary role | Logical motions | Delivered files | Evidence boundary |
|---|---:|---:|---|
| `idle-pose` | 0 | 0 | Not authoritatively classified |
| `continuous-locomotion` | 0 | 0 | Filename hypotheses stay outside canonical counts |
| `locomotion-transition` | 0 | 0 | Not authoritatively classified |
| `airborne` | 0 | 0 | Not authoritatively classified |
| `traversal` | 0 | 0 | Not authoritatively classified |
| `action-interaction` | 0 | 0 | Not authoritatively classified |
| `reaction-death` | 0 | 0 | Not authoritatively classified |
| `emote-cinematic` | 0 | 0 | Not authoritatively classified |
| `other-unknown` | 249 | 249 | One opaque current source unit per delivered FBX |
| **Total** | **249** | **249** | No vendor mapping inferred |

### Runtime-set inventory

| Runtime set | Type | Members/variants | Grouping evidence | Validation status |
|---|---|---|---|---|
| `hypothesis/full-body-unarmed-to-pistol` | transition-chain | `mixamo-basic-locomotion::Basic_Locomotion_Pack_-_in-place/walking.fbx`; `mixamo-pistol-handgun-locomotion::Pistol-Handgun_Locomotion_Pack_-_in-place/pistol run.fbx` | New evaluator-selected namespaced full-body state proposal; no historical set recovered | `not-evaluated` pairwise/runtime/artistic acceptance |

### Pipeline-stage coverage

| Stage | Coverage state | Evidence / remaining gate |
|---|---|---|
| Acquire | `partially-evaluated` | Extracted sources present; original ZIP/license unavailable |
| Preserve raw | `evaluated-clean` | 249-file inventory matched before/after |
| Inspect | `evaluated-clean` | All sources loaded and resolved Mixamo roles |
| Segment | `not-evaluated` | Vendor 231-to-249 mapping unavailable |
| Root motion | `partially-evaluated` | Archive declarations ran; per-file XZ/yaw policy unavailable |
| Conform | `not-evaluated` | No outputs produced |
| Validate | `partially-evaluated` | Mechanical/current contracts complete; runtime/artistic gates open |
| Optimize | `not-evaluated` | Constant-track notes not promoted |
| Export | `not-evaluated` | No engine-facing export ran |
| Gate/report | `partially-evaluated` | Source-only reports complete; adoption gates remain |

### Readiness evidence by clip set

| Role or runtime set | File-ready / clip-ready | Set-ready / rig-use | Runtime / acceptance boundary |
|---|---|---|---|
| Nine opaque corpora | 0 baseline errors; 50 warnings | Roles resolve; per-file semantics and retarget use unavailable | No current engine/visual/gameplay acceptance |
| Nine constituent hypotheses | Exact members measured | New controller proposals only | Blend, phase, transition, contact, deformation tests required |
| `hypothesis/full-body-unarmed-to-pistol` | Two exact in-place members measured | Pairwise skeleton/rest/scale unproven | Full-body transition plus technical/artistic acceptance required |

### Validation-profile status

| Validation profile | Selection | Result / next evidence |
|---|---|---|
| Marketplace intake | `selected` — `observed-pack-capability` | Exhaustive source mechanics complete |
| Blended locomotion | `selected` — `evaluator-selected-generic-scenario` | Nine static hypotheses; runtime acceptance open |
| Root-motion controller | `selected` — `observed-pack-capability` | 52 ownership conflicts need per-file policy |
| State-machine transitions | `selected` — `evaluator-selected-generic-scenario` | Full-body cross-pack handoff proposed; not tested |
| Layered upper body/weapons | `selected` — `evaluator-selected-generic-scenario` | Full-body baseline only; layering remains unavailable |
| Traversal/environment | `not-selected` | No target environment contract |
| Contact actions/interactions | `not-selected` | No target contact contract |
| Retargeted/customizable characters | `not-selected` | No target character/retargeter supplied |
| Motion matching/search | `not-selected` | No database contract supplied |
| Networked movement | `not-selected` | No replication/rollback contract supplied |
| Runtime performance | `not-selected` | No target runtime/hardware supplied |

## Pack inventory and content evidence

| Constituent | FBXs | Legacy metadata | Baseline | Declared XZ | New hypothesis |
|---|---:|---|---|---|---|
| [Mixamo Basic Locomotion](mixamo-basic-locomotion.md) | 12 | 10 unlinked metadata motions | 4 warnings / 1712 notes | 4 ownership errors | `hypothesis/kinematic-walk-3way` (`not-evaluated`) |
| [Mixamo Female Basic Locomotion](mixamo-female-basic-locomotion.md) | 20 | 18 unlinked metadata motions | 5 warnings / 2843 notes | 6 ownership errors | `hypothesis/kinematic-speed` (`not-evaluated`) |
| [Mixamo Female Locomotion](mixamo-female-locomotion.md) | 18 | 16 unlinked metadata motions | 4 warnings / 2559 notes | 4 ownership errors | `hypothesis/kinematic-speed` (`not-evaluated`) |
| [Mixamo Locomotion](mixamo-locomotion.md) | 20 | 18 unlinked metadata motions | 5 warnings / 2852 notes | 6 ownership errors | `hypothesis/kinematic-speed` (`not-evaluated`) |
| [Mixamo Longbow Locomotion](mixamo-longbow-locomotion.md) | 22 | 20 unlinked metadata motions | 3 warnings / 3126 notes | 3 ownership errors | `hypothesis/kinematic-run-4way` (`not-evaluated`) |
| [Mixamo Magic Locomotion](mixamo-magic-locomotion.md) | 27 | 25 unlinked metadata motions | 6 warnings / 3837 notes | 4 ownership errors | `hypothesis/kinematic-run-4way` (`not-evaluated`) |
| [Mixamo Male Locomotion](mixamo-male-locomotion.md) | 18 | 16 unlinked metadata motions | 5 warnings / 2564 notes | 4 ownership errors | `hypothesis/kinematic-speed` (`not-evaluated`) |
| [Mixamo Pistol-Handgun Locomotion](mixamo-pistol-handgun-locomotion.md) | 29 | 27 unlinked metadata motions | 7 warnings / 4122 notes | 5 ownership errors | `hypothesis/kinematic-run-axis` (`not-evaluated`) |
| [Mixamo Rifle 8-Way Locomotion](mixamo-rifle-8-way-locomotion.md) | 83 | 81 unlinked metadata motions | 11 warnings / 11790 notes | 16 ownership errors | `hypothesis/kinematic-run-8way` (`not-evaluated`) |

The 231 metadata motion count is not mapped to the 249 files. Exact current paths remain opaque canonical units. New hypotheses use only named current files and do not claim vendor intent or recovered historical membership.

## Mechanical baseline

| Finding/check | Affected scope | Potential impact | Evidence |
|---|---|---|---|
| No baseline errors | 249/249 files | File-readable prerequisite only | Current ledger |
| `duration-sanity` | 50 findings | Empty reference takes or endpoint clamp-hold | Exact paths in constituent appendices |
| `constant-track` | 35405 notes | Optimization hint; no visual defect or safe rewrite inferred | Current ledger |
| Skeleton counts | 231 files at 66 bones; 18 `X Bot.fbx` at 68 | Reference files need classification; common counts do not prove compatibility | Current inspect output |

## AnimSmith remediation evidence

| Source issue | Operation/declarations | Result | Independent verification | Remaining caveat |
|---|---|---|---|---|
| MIX-OWN-001 | Archive-level XZ declarations | 52 stationary-root errors | Current per-file lint/measure; no output produced | Per-file intent/controller acceptance unavailable; residual unresolved |
| MIX-XPACK-001 | No transform or retarget run | No candidate produced | Exact source identities only | Pairwise technical/artistic acceptance unavailable; residual unresolved |

## Engine procedures and evidence

| Runtime | Version | Procedure | Observed result | Remaining gate |
|---|---|---|---|---|
| Unity | unspecified | Not run | `not-evaluated` | Exact pairwise full-body state graph |
| Unreal Engine | unspecified | Not run | `not-evaluated` | Exact retarget/state graph |
| Godot | unspecified | Not run | `not-evaluated` | Exact Skeleton3D/AnimationTree graph |
| Bevy | unspecified | Not run | `not-evaluated` | Exact target identity/animation graph |

## Rig, masking, and compatibility evidence

| Pack/rig/set pair | Skeleton/retarget | Scale/axes | Root policy | Timing/blend | Overall evidence |
|---|---|---|---|---|---|
| Basic / Pistol proposed handoff | 66-bone counts and Mixamo roles on selected files; hierarchy/rest/deformation not compared | Not evaluated | Controller ownership proposed | Full-body transition not tested | `not-evaluated` technical/contact/artistic acceptance |
| All other constituent pairs | Common role labels/counts are insufficient | Not evaluated | Per-file ownership unavailable | No pairwise test | `unknown` |

## Limitations and unknowns

1. Vendor mapping, loop/phase intent, target character, engines, retargeting, contacts, deformation, performance, networking, and artistic style remain unavailable.
2. Cross-pack hypotheses are new evaluator scenarios; no historical membership or compatibility conclusion was reconstructed.
3. Original ZIP payloads and controlling license evidence were unavailable; hashing covers extracted sources.

## Changes between AnimSmith versions

AnimSmith 0.14.0 — Fresh source-only matrix admitted 249/249 files, revalidated mechanical/declared counts, and added bounded new controller/state hypotheses without claiming runtime acceptance.

AnimSmith 0.10.0 — Retained historical totals do not supply collection semantics. AnimSmith 0.7.0 evidence remains superseded.

## Reproduction

Official evaluator preflight: [release archive](https://github.com/mmannerm/animsmith/releases/download/v0.14.0/animsmith-v0.14.0-x86_64-unknown-linux-gnu.tar.gz), archive SHA-256 `4ecf79436f9123c779edb004050da3010b44a2b392e3031facb227d7734fc33e`; exact member `animsmith-v0.14.0-x86_64-unknown-linux-gnu/animsmith`, binary SHA-256 `c2b9649bc74f8a7e5b5d7361a6feaa93941b2327cd7eee82235024bd4f61e366`; tag `v0.14.0`, peeled commit `e8321ad40be5ef6f162b31f085819c039175c3c9`; working-tree state: N/A (official archive). Compiled features: `fbx, report`. Version and required command help plus representative FBX admission passed before evaluation. Preflight: `external:animsmith-0.14.0-report-refresh/preflight.json`, SHA-256 `7034b5043de54e7b30064230db18605d1f96d0ea1477f7b8c56ce817cc73521a`.

Official binary SHA-256 `c2b9649bc74f8a7e5b5d7361a6feaa93941b2327cd7eee82235024bd4f61e366`, version `0.14.0`, source revision `e8321ad40be5ef6f162b31f085819c039175c3c9`, dirty `false`, features `fbx,report`. Version/help/required-command preflight exited 0. The four-worker read-only matrix ran 996 commands with no timeouts: inspect, measure, empty-baseline lint, and variant-declaration lint per FBX. The 52 nonzero exits are the declared-contract errors reported above; every loader/inspect/measure/baseline invocation exited 0.

Source inventory SHA-256 `bc8543a8e1c7cf439030c07a2aead5db183d610633f637c1979dbce71a4dce66` covers 249 files / 119,754,377 bytes and matched before/after. Ledger SHA-256 `62b0a7b7a1bb47c817f6b005a988077a44dc8d942bb26343a5f7278a15582272`; summary `4dd01ee9f1398fa200355c82206c8fd24d80d0dea1f8965aee8b03ba400cfc7c`; report data `a8d8e1bd3707b58e24f7d08f4ea7038c0f316254fa289e4f6439284dbcbc388f`. Full outputs, exact private paths, and licensed sources remain external.

## Sources

- Nine linked current constituent pairs plus fresh external source inventory, command ledger, summary, and outputs.
- [AnimSmith game-ready clip guidance](../game-ready-clips.md).
