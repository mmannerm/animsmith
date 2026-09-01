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
| Conform | `partially-evaluated` | Twelve declared slices and 24 declared in-place gait anchors generated external candidates; no target-rig or engine acceptance was run |
| Validate | `evaluated-finding` | Default lint completed for every FBX |
| Optimize | `partially-evaluated` | Three external constant-track candidates were generated; source was unchanged and runtime equivalence remains unproved |
| Export | `partially-evaluated` | Candidates were written as external GLBs only; no engine handoff was selected |
| Gate/report | `partially-evaluated` | Current baseline, remediation, and boundary recorded |

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
| 36 negative-time findings in 12 inputs | Declared `transform --slice` windows at 30 FPS | 12/12 generated candidates; the 36 `time-monotonic` errors are absent after slicing | Every output passed `inspect`, JSON `measure`, and `fix --dry-run`; `diff` exited 1 for the intended change | Remaining loop-seam findings; candidates are unpromoted and no engine/visual result exists |
| In-place locomotion phase disagreement | Declared `transform --gait-anchor` on 24 selected in-place members | 24/24 generated candidates; circular spreads changed 0.7156245→0.0501911, 0.4630161→0.0938395, and 0.6597812→0.0724415 | Every output passed `inspect`, JSON `measure`, and `fix --dry-run`; `diff` exited 1 for the intended change | Post-output declared lint findings remain; no root-motion, engine, visual, or gameplay acceptance is claimed |
| Dense constant tracks | Declared `transform --prune-constant-tracks` on three representatives | 3/3 generated candidates | Every output passed `inspect`, JSON `measure`, and `fix --dry-run`; `diff` exited 1 for the intended change | Semantic/runtime equivalence is unproved; candidates are unpromoted |

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

AnimSmith 0.10.0 — official baseline reran the corpus with the verified release, then produced the current 39-candidate remediation evidence. Earlier 0.7.0 evaluator and any engine/offline results are superseded historical evidence and are not used by the current report.

## Reproduction

Evaluator archive: [official Linux release archive](https://github.com/mmannerm/animsmith/releases/download/v0.10.0/animsmith-v0.10.0-x86_64-unknown-linux-gnu.tar.gz), SHA-256 `8de4f97949fbc61fc3aec1d5f22272735ffe06937a0fea5c998cb3e0f639c662`; member `animsmith-v0.10.0-x86_64-unknown-linux-gnu/animsmith`, tag `v0.10.0`, peeled commit `db91d8dda3326f97f581d4d62104d928caec383f`, binary SHA-256 `2052ce64eda53d5037b305561dd0287209719d743b0a4051552e197fbfe4a387`. Working tree: N/A (official archive). The binary had the expected `fbx` and `report` feature surface; version, top-level help, and help for `inspect`, `measure`, `lint`, `transform`, `diff`, and `fix` all succeeded, as did representative FBX admission. Current outputs are output schema v19 / measurements schema v18. The safe external evidence locators are `external:protofactor-remediation-0.10.0/basic-locomotion/remediation-v0.10.0-final/preflight.json` (SHA-256 `4fdb2d20e3067c47342cac542d28e7cd5e3272f01843a302827fff5c94af86c9`), `remediation-results.json` (`523cca49b496690cf47e7301a72e3839e893721b4acc7e9430e670c830480ea3`), and `public-summary.json` (`0574ba377f74d2063cfba8d630f15f595d862ca2bc5ed217984dc8eb2128d466`). Each of the 179 source files received `inspect`, JSON `measure`, JSON `lint`, and Markdown `lint`; all remediation command output and digests remain external.

## Sources

- Protofactor, [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/) — product context.
- AnimSmith, [CLI reference](../cli.md) and [game-ready clips](../game-ready-clips.md) — current evaluator and readiness boundary.
