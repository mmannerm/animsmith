# Animation pack evidence appendix: Protofactor Basic Locomotion Animset

> Companion report: [Protofactor Basic Locomotion report](protofactor-basic-locomotion.md)
>
> Evidence status: **partial** — the official evaluator loaded the delivered FBXs and produced current baseline evidence.
>
> Evaluation date: **2026-09-01**
>
> Current evaluator: **AnimSmith 0.10.0**
>
> Report format: **2**

This appendix preserves current evidence only. The [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder) remains authoritative.

Evaluation manifest schema: `urn:animsmith:skill:animation-pack-evaluation-manifest:1`.

## Evaluation scope and provenance

| Field | Value |
|---|---|
| Pack/edition | Protofactor Basic Locomotion Animset; local constituent revision unknown |
| Vendor/source | [Protofactor Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/) |
| Delivered scope | Authorized local commercial delivery; 179 preserved FBXs |
| Target use | Engine-neutral third-person locomotion intake |
| Target engines | Unity, Unreal Engine, Godot, and Bevy; not evaluated in this run |
| Target rigs/packs | No target character or current cross-pack run supplied |
| Source manifest | External scrubbed inventory, SHA-256 `c6cc4d541fa2cb8e4f3e14c283d5b925f83957db35bfa079309f250cdaf101ba` |
| Evaluation manifest | `urn:animsmith:skill:animation-pack-evaluation-manifest:1`; current binding unavailable because current output binding was not rendered for this retained report format |
| Acquisition/license provenance | Commercial source was authorized locally; no transaction or redistribution conclusion is made |

### Evidence coverage

| Surface | Offered/delivered | Evaluated | Findings | Not evaluated and why |
|---|---:|---:|---:|---|
| Animation files | 179 FBXs | 179 | baseline completed | mechanical and contract evidence captured |
| Rigs/export variants | Unknown | 0 | 0 | Not evaluated in this run |
| AnimSmith baseline | 179 | 179 | current findings recorded | `inspect`, JSON `measure`, JSON `lint`, and Markdown `lint` completed per file |
| Declared contracts | 177 motion-labelled files | 177 | 58 pass; 119 fail per output format | Mechanical contract results are current; semantic taxonomy, engine, and artistic acceptance were not evaluated |
| Offline visual reports | 0 | 0 | 0 | Not evaluated in this run |
| Engine import/playback | 4 runtimes | 0 | 0 | No engine import or playback run selected |
| Blend/mask/retarget | Unknown | 0 | 0 | Measurements completed, but no target rig or blend/mask acceptance test ran |

### Claim legend

Current consequential claims use `observed-file`, `observed-animsmith`, `documentation-stated`, or `not-evaluated` as defined by the assessment taxonomy.

## Evaluation manifest and taxonomy

### Canonical clip-role inventory

| Canonical primary role | Logical motions | Delivered files | Evidence boundary |
|---|---:|---:|---|
| `idle-pose` | 0 | 0 | Classification was not regenerated or accepted |
| `continuous-locomotion` | 0 | 0 | Classification was not regenerated or accepted |
| `locomotion-transition` | 0 | 0 | Classification was not regenerated or accepted |
| `airborne` | 0 | 0 | Classification was not regenerated or accepted |
| `traversal` | 0 | 0 | Classification was not regenerated or accepted |
| `action-interaction` | 0 | 0 | Classification was not regenerated or accepted |
| `reaction-death` | 0 | 0 | Classification was not regenerated or accepted |
| `emote-cinematic` | 0 | 0 | Classification was not regenerated or accepted |
| `other-unknown` | 0 | 0 | Classification was not regenerated or accepted |
| **Total** | **0** | **0** | 179 delivered FBXs remain unclassified |

### Runtime-set inventory

No runtime sets were identified.

### Pipeline-stage coverage

| Stage | Coverage state | Evidence / remaining gate |
|---|---|---|
| Acquire | `partially-evaluated` | Authorized local bytes inventory only |
| Preserve raw | `evaluated-clean` | Raw commercial source was not modified |
| Inspect | `evaluated-finding` | 179 baseline attempts completed |
| Segment | `not-evaluated` | No separate segmentation trial selected |
| Root motion | `not-evaluated` | Measurements completed; no root-motion policy or controller acceptance was evaluated |
| Conform | `not-evaluated` | No target rig, retarget trial, or conformance policy |
| Validate | `evaluated-finding` | Default lint completed for every FBX |
| Optimize | `not-evaluated` | No source change authorized |
| Export | `not-evaluated` | No export handoff selected |
| Gate/report | `partially-evaluated` | Current baseline and boundary recorded |

### Readiness evidence by clip set

| Role or runtime set | File-ready / clip-ready | Set-ready / rig-use | Runtime / acceptance boundary |
|---|---|---|---|
| Entire delivered corpus | evaluated-finding | not-evaluated | not-evaluated |

### Validation-profile status

| Validation profile | Selection | Result / next evidence |
|---|---|---|
| Marketplace intake | `selected` — `evaluator-selected-generic-scenario` | Source inventory and current baseline captured |
| Blended locomotion | `selected` — `evaluator-selected-generic-scenario` | Measurements completed; selected IP/RM ownership, blend policy, and engine acceptance remain |
| Root-motion controller | `selected` — `evaluator-selected-generic-scenario` | Measurements completed; controller trajectory policy and engine acceptance remain |
| State-machine transitions | `selected` — `evaluator-selected-generic-scenario` | Requires contract-qualified clip selection and engine test |
| Layered upper body/weapons | `not-selected` | No current action scope |
| Traversal/environment | `not-selected` | No current action scope |
| Contact actions/interactions | `not-selected` | No current action scope |
| Retargeted/customizable characters | `not-selected` | No target character supplied |
| Motion matching/search | `not-selected` | No motion database scope |
| Networked movement | `not-selected` | No controller scope |
| Runtime performance | `not-selected` | No runtime build |

## Pack inventory and content evidence

The scrubbed inventory records 385 regular files totaling 296,205,291 bytes; 179 are FBX AnimSmith-input candidates. File names indicate locomotion, cover, airborne, turn, and action families, but names are not current semantic proof.

## Mechanical baseline

| Finding/check | Affected scope | Potential impact | Evidence |
|---|---|---|---|
| Baseline mechanical health | 179/179 FBXs | Mechanical, rig, timing, and lint assessment completed | `observed-animsmith`; 716 commands completed |
| Declared-contract results | 177/177 motion-labelled files | 58 pass and 119 fail per output format; not an engine or artistic acceptance result | `observed-animsmith` |

## AnimSmith remediation evidence

| Source issue | Operation/declarations | Result | Independent verification | Remaining caveat |
|---|---|---|---|---|
| Current source loading | No transform or repair trial is applicable | Loaded successfully; no transform trial was selected | Official baseline is exhaustive | No generated output was selected |

## Engine procedures and evidence

| Runtime | Version | Procedure | Observed result | Remaining gate |
|---|---|---|---|---|
| Unity | unspecified | None in current run | not evaluated | Disposable import, controller, and visual test |
| Unreal Engine | unspecified | None in current run | not evaluated | Disposable import, retarget, graph, and visual test |
| Godot | unspecified | None in current run | not evaluated | Disposable conversion/import and graph test |
| Bevy | unspecified | None in current run | not evaluated | Selected export handoff, runtime, and performance test |

## Rig, masking, and compatibility evidence

| Pack/rig/set pair | Skeleton/retarget | Scale/axes | Root policy | Timing/blend | Overall evidence |
|---|---|---|---|---|---|
| Basic Locomotion/current evaluator | not evaluated | not evaluated | not evaluated | not evaluated | Current baseline evidence |

## Limitations and unknowns

1. No current engine, visual, contact, retarget, or artistic test ran.
2. Commercial sources and outputs remain external and are not published.

## Changes between AnimSmith versions

AnimSmith 0.10.0 — official baseline reran the corpus with the verified release. Earlier 0.7.0 results are superseded historical evidence and are not used by the current report.

## Reproduction

Evaluator: official tag `v0.10.0`, peeled source commit `db91d8dda3326f97f581d4d62104d928caec383f`, binary SHA-256 `2052ce64eda53d5037b305561dd0287209719d743b0a4051552e197fbfe4a387`. Bounded version/help/capability capture and representative FBX admission succeeded before the exhaustive batch. The scrubbed command manifest digest is `37303fc996d73b11da8abc0980505e862cc00dac905dfdb8c1fe3f7517624b4c`; each of the 179 files received `inspect`, JSON `measure`, JSON `lint`, and Markdown `lint`, all completed with preserved output.

## Sources

- Protofactor, [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/) — product context.
- AnimSmith, [CLI reference](../cli.md) and [game-ready clips](../game-ready-clips.md) — current evaluator and readiness boundary.
