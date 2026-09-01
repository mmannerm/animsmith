# Animation pack evidence appendix: Protofactor Injured Animset

> Companion report: [Protofactor Injured report](protofactor-injured.md)
>
> Evidence status: **partial** — the official evaluator loaded the delivered FBXs and produced current baseline evidence.
>
> Evaluation date: **2026-09-01**
>
> Current evaluator: **AnimSmith 0.10.0**
>
> Report format: **2**

The [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder) remains authoritative.

Evaluation manifest schema: `urn:animsmith:skill:animation-pack-evaluation-manifest:1`.

## Evaluation scope and provenance

| Field | Value |
|---|---|
| Pack/edition | Protofactor Injured Animset; local constituent revision unknown |
| Vendor/source | [Protofactor Animset: Injured](https://protofactor.biz/product/animset-injured/) |
| Delivered scope | Authorized local commercial delivery; 72 FBXs |
| Target use | Engine-neutral injured-state intake |
| Target engines | Unity, Unreal Engine, Godot, and Bevy; not evaluated |
| Target rigs/packs | No target character or current cross-pack run supplied |
| Source manifest | External scrubbed inventory, SHA-256 `e60ef65d2964a9eaa497cb6f8ee898542f88a67719625633cfee64ad7816e7cf` |
| Evaluation manifest | Unavailable: current output binding was not rendered for this retained report format |
| Acquisition/license provenance | Authorized commercial bytes; no transaction or redistribution conclusion |

### Evidence coverage

| Surface | Offered/delivered | Evaluated | Findings | Not evaluated and why |
|---|---:|---:|---:|---|
| Animation files | 72 FBXs | 72 | baseline completed | mechanical and contract evidence captured |
| Rigs/export variants | Unknown | 0 | 0 | Not evaluated in this run |
| AnimSmith baseline | 72 | 72 | current findings recorded | `inspect`, JSON `measure`, JSON `lint`, and Markdown `lint` completed per file |
| Declared contracts | 70 motion-labelled files | 70 | 28 pass; 42 fail per output format | Mechanical contract results are current; semantic taxonomy, engine, and artistic acceptance were not evaluated |
| Offline visual reports | 0 | 0 | 0 | Not evaluated in this run |
| Engine import/playback | 4 runtimes | 0 | 0 | Deferred |
| Blend/mask/retarget | Unknown | 0 | 0 | Measurements completed, but no target rig or blend/mask acceptance test ran |

### Claim legend

Current claims are `observed-file`, `observed-animsmith`, `documentation-stated`, or `not-evaluated`.

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
| **Total** | **0** | **0** | 72 delivered FBXs remain unclassified |

### Runtime-set inventory

No runtime sets were identified.

### Pipeline-stage coverage

| Stage | Coverage state | Evidence / remaining gate |
|---|---|---|
| Acquire | `partially-evaluated` | Authorized local inventory |
| Preserve raw | `evaluated-clean` | Source unchanged |
| Inspect | `evaluated-finding` | 72 baseline attempts completed |
| Segment | `not-evaluated` | No separate segmentation trial selected |
| Root motion | `not-evaluated` | Measurements completed; no root-motion policy or controller acceptance was evaluated |
| Conform | `not-evaluated` | No conformance trial selected |
| Validate | `evaluated-finding` | Lint completed for every FBX |
| Optimize | `not-evaluated` | No transform trial |
| Export | `not-evaluated` | No export or handoff trial selected |
| Gate/report | `partially-evaluated` | Current baseline recorded |

### Readiness evidence by clip set

| Role or runtime set | File-ready / clip-ready | Set-ready / rig-use | Runtime / acceptance boundary |
|---|---|---|---|
| Entire delivered corpus | evaluated-finding | not-evaluated | not-evaluated |

### Validation-profile status

| Validation profile | Selection | Result / next evidence |
|---|---|---|
| Marketplace intake | `selected` — `evaluator-selected-generic-scenario` | Inventory and current baseline captured |
| Blended locomotion | `selected` — `evaluator-selected-generic-scenario` | Measurements completed; selected IP/RM ownership, blend policy, and engine acceptance remain |
| Root-motion controller | `selected` — `evaluator-selected-generic-scenario` | Measurements completed; controller trajectory policy and engine acceptance remain |
| State-machine transitions | `selected` — `evaluator-selected-generic-scenario` | Requires contract-qualified clip selection and engine test |
| Layered upper body/weapons | `not-selected` | No current action scope |
| Traversal/environment | `not-selected` | No traversal scope |
| Contact actions/interactions | `not-selected` | No current contact scope |
| Retargeted/customizable characters | `not-selected` | No target rig supplied |
| Motion matching/search | `not-selected` | No database scope |
| Networked movement | `not-selected` | No controller scope |
| Runtime performance | `not-selected` | No runtime build |

## Pack inventory and content evidence

The scrubbed inventory records 171 regular files totaling 217,461,232 bytes; 72 are FBX candidates. Filename labels are not semantic, speed, loop, or blend evidence.

## Mechanical baseline

| Finding/check | Affected scope | Potential impact | Evidence |
|---|---|---|---|
| Baseline mechanical health | 72/72 FBXs | Mechanical, rig, timing, and lint assessment completed | `observed-animsmith`; 288 commands completed |
| Declared-contract results | 70/70 motion-labelled files | 28 pass and 42 fail per output format; not an engine or artistic acceptance result | `observed-animsmith` |

## AnimSmith remediation evidence

| Source issue | Operation/declarations | Result | Independent verification | Remaining caveat |
|---|---|---|---|---|
| Current source loading | No repair or transform applicable | Loaded successfully; no transform trial was selected | Exhaustive official baseline | No generated output was selected |

## Engine procedures and evidence

| Runtime | Version | Procedure | Observed result | Remaining gate |
|---|---|---|---|---|
| Unity | unspecified | None in current run | not evaluated | Disposable import, blend, mask, and visual test |
| Unreal Engine | unspecified | None in current run | not evaluated | Disposable import, retarget, graph, and build test |
| Godot | unspecified | None in current run | not evaluated | Disposable conversion/import and graph test |
| Bevy | unspecified | None in current run | not evaluated | Selected export handoff and runtime test |

## Rig, masking, and compatibility evidence

| Pack/rig/set pair | Skeleton/retarget | Scale/axes | Root policy | Timing/blend | Overall evidence |
|---|---|---|---|---|---|
| Injured/current evaluator | not evaluated | not evaluated | not evaluated | not evaluated | Current baseline evidence |

## Limitations and unknowns

1. No current engine, visual, contact, retarget, or artistic test ran.
2. Commercial sources and derivatives remain external.

## Changes between AnimSmith versions

AnimSmith 0.10.0 — exhaustive current baseline reran all 72 FBXs with the verified official release. Earlier 0.7.0 evidence is superseded historical evidence only.

## Reproduction

Evaluator: official tag `v0.10.0`, peeled source commit `db91d8dda3326f97f581d4d62104d928caec383f`, binary SHA-256 `2052ce64eda53d5037b305561dd0287209719d743b0a4051552e197fbfe4a387`. Bounded version/help/capability capture and representative FBX admission succeeded before the exhaustive batch. Command manifest SHA-256 `bb8c0800fd141e01092fa90ff7613fef0beeba3a01c0356db1e04252f6a69b2d`; every baseline invocation completed with preserved command output.

## Sources

- Protofactor, [Animset: Injured](https://protofactor.biz/product/animset-injured/) — product context.
- AnimSmith, [CLI reference](../cli.md) and [game-ready clips](../game-ready-clips.md) — evaluator boundary.
