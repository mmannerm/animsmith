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
| Conform | `partially-evaluated` | Fourteen declared in-place gait anchors generated external candidates; no target-rig or engine acceptance was run |
| Validate | `evaluated-finding` | Lint completed for every FBX |
| Optimize | `partially-evaluated` | One external constant-track candidate was generated; source was unchanged and runtime equivalence remains unproved |
| Export | `partially-evaluated` | Candidates were written as external GLBs only; no engine handoff was selected |
| Gate/report | `partially-evaluated` | Current baseline, remediation, and boundary recorded |

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
| In-place locomotion phase alignment | Declared `transform --gait-anchor` on 14 in-place members | 14/14 external candidates generated in the combined 17-candidate rerun | Transform, `inspect`, and JSON `measure` exited 0; `diff` exited 1 for intended changes | Retained lint findings; no engine, visual, root-motion, or gameplay acceptance is claimed |
| Dense constant tracks | Declared `transform --prune-constant-tracks` on one representative | 1/1 external candidate generated | Transform, `inspect`, and JSON `measure` exited 0; `diff` exited 1 for the intended change | Runtime equivalence is unproved; candidate is unpromoted |

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

AnimSmith 0.10.0 — exhaustive current baseline reran all 72 FBXs with the verified official release, then produced 14 gait-anchor and one pruning candidate in the combined remediation pass. Earlier 0.7.0 evaluator and any engine/offline results are historical only.

## Reproduction

Evaluator archive: [official Linux release archive](https://github.com/mmannerm/animsmith/releases/download/v0.10.0/animsmith-v0.10.0-x86_64-unknown-linux-gnu.tar.gz), SHA-256 `8de4f97949fbc61fc3aec1d5f22272735ffe06937a0fea5c998cb3e0f639c662`; member `animsmith-v0.10.0-x86_64-unknown-linux-gnu/animsmith`, tag `v0.10.0`, peeled commit `db91d8dda3326f97f581d4d62104d928caec383f`, binary SHA-256 `2052ce64eda53d5037b305561dd0287209719d743b0a4051552e197fbfe4a387`. Working tree: N/A (official archive). The binary had the expected `fbx` and `report` feature surface; version, top-level help, and help for `inspect`, `measure`, `lint`, `transform`, `diff`, and `fix` all succeeded, as did representative FBX admission. Current outputs are output schema v19 / measurements schema v18. Safe evidence is external at `external:protofactor-remediation-0.10.0/`: preflight SHA-256 `dde0aa485d392c360736346041558b72f39798de7fdcc443ad7aaba9fc8b445a`, status SHA-256 `d85a1c53d5d1fcb829e0574051f173f6e1c08a409358385f4400940e11652e1f`, and ledger SHA-256 `fdab771891c1ee7d316bab224f0dc2bb65cd286f84b21c9c91cab171f580f5ec`. All transform, `inspect`, and JSON `measure` invocations exited 0; `diff` exited 1 for intentional output changes. Retained lint findings and all candidate payloads remain external.

## Sources

- Protofactor, [Animset: Injured](https://protofactor.biz/product/animset-injured/) — product context.
- AnimSmith, [CLI reference](../cli.md) and [game-ready clips](../game-ready-clips.md) — evaluator boundary.
