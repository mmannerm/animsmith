# Animation pack evidence appendix: Protofactor Dual Swords Animset

> Companion report: [technical evaluation](protofactor-dual-swords.md)
>
> Evidence status: **partial** — current source inventory, serial mechanical baseline, declared contracts, and bounded remediation completed; engine and artistic acceptance did not.
>
> Evaluation date: **2026-09-01**
>
> Current evaluator: **AnimSmith 0.10.0**
>
> Report format: **2**

Current evidence boundary; the [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder) remains authoritative.

## Evaluation scope and provenance

| Field | Value |
|---|---|
| Pack/edition | Local licensed constituent; revision unknown |
| Vendor/source | Protofactor current product listing |
| Delivered scope | 189 FBX input candidates |
| Target use | Engine use; no target controller supplied |
| Target engines | Not evaluated |
| Target rigs/packs | Not evaluated |
| Source manifest | External source inventory, SHA-256 `6686db08c5d264823473332b0c35aa55e068e1d01595797bfc8710da6ea81a7e` |
| Evaluation manifest | `urn:animsmith:skill:animation-pack-evaluation-manifest:1` |
| Acquisition/license provenance | Authorized local commercial input; no legal advice |

### Evidence coverage

| Surface | Offered/delivered | Evaluated | Findings | Not evaluated and why |
|---|---:|---:|---:|---|
| Animation files | 189 | 189 | Mechanical baseline completed | N/A |
| Rigs/export variants | 188 animation-bearing candidates | partial | Mechanical inspection only | Runtime compatibility not run |
| AnimSmith baseline | 189 | 189 | 756 baseline commands completed successfully; constant-track notes and strict loop contracts require review. | N/A |
| Declared contracts | 186 selected motion inputs | 186 | 186 declared-contract inputs: 24 clean and 162 non-clean results in each output format. | Engine behavior is out of scope |
| Remediation candidates | 25 | 25 | All transforms exit 0 | Engine/visual review pending |
| Engine import/playback | Unknown | 0 | 0 | Not run |
| Blend/mask/retarget | Unknown | 0 | 0 | Not run |

### Claim legend

Uses `observed-file`, `observed-animsmith`, and `not-evaluated`.

## Evaluation manifest and taxonomy

The current manifest follows `urn:animsmith:skill:animation-pack-evaluation-manifest:1`. File inventory is current; gameplay classification is not promoted without engine acceptance.

### Canonical clip-role inventory

| Canonical primary role | Logical motions | Delivered files | Evidence boundary |
|---|---:|---:|---|
| `idle-pose` | 0 | 0 | runtime classification withheld |
| `continuous-locomotion` | 0 | 0 | runtime classification withheld |
| `locomotion-transition` | 0 | 0 | runtime classification withheld |
| `airborne` | 0 | 0 | runtime classification withheld |
| `traversal` | 0 | 0 | runtime classification withheld |
| `action-interaction` | 0 | 0 | runtime classification withheld |
| `reaction-death` | 0 | 0 | runtime classification withheld |
| `emote-cinematic` | 0 | 0 | runtime classification withheld |
| `other-unknown` | 0 | 0 | runtime classification withheld |
| **Total** | **0** | **0** | Current runtime classification withheld |

### Runtime-set inventory

No runtime sets were identified.

### Pipeline-stage coverage

| Stage | Coverage state | Evidence / remaining gate |
|---|---|---|
| Acquire | `evaluated-clean` | External source retained |
| Preserve raw | `evaluated-clean` | New external evidence root |
| Inspect | `evaluated-clean` | Current baseline completed |
| Segment | `not-evaluated` | Runtime grouping not accepted |
| Root motion | `not-evaluated` | No engine controller test |
| Conform | `partially-evaluated` | Bounded candidate transforms only |
| Validate | `partially-evaluated` | Baseline and declared contracts complete |
| Optimize | `not-evaluated` | No production optimization review |
| Export | `not-evaluated` | No target-engine export accepted |
| Gate/report | `partially-evaluated` | Engine and artistic gates remain |

### Readiness evidence by clip set

| Role or runtime set | File-ready / clip-ready | Set-ready / rig-use | Runtime / acceptance boundary |
|---|---|---|---|
| All delivered candidates | Mechanical baseline complete | Not evaluated | No current engine or artistic acceptance |

### Validation-profile status

| Validation profile | Selection | Result / next evidence |
|---|---|---|
| Marketplace intake | `selected` — `vendor-intended` | Inventory and baseline completed. |
| Blended locomotion | `selected` — `evaluator-selected-generic-scenario` | Strict contract findings require targeted review. |
| Root-motion controller | `not-selected` | No controller was supplied. |
| State-machine transitions | `selected` — `evaluator-selected-generic-scenario` | Contract evidence captured; engine validation pending. |
| Layered upper body/weapons | `not-selected` | No mask or attachment test. |
| Traversal/environment | `not-selected` | No target scene. |
| Contact actions/interactions | `not-selected` | No target contacts. |
| Retargeted/customizable characters | `not-selected` | No target rig. |
| Motion matching/search | `not-selected` | No target database. |
| Networked movement | `not-selected` | No target networking design. |
| Runtime performance | `not-selected` | No runtime benchmark. |

## Pack inventory and content evidence

Current inventory found 189 FBX candidates, including 188 animation-bearing candidates, 186 individual-motion candidates, and 1 combined-take candidates. This establishes source presence and mechanical coverage only.

## Mechanical baseline

| Finding/check | Affected scope | Potential impact | Evidence |
|---|---|---|---|
| Baseline commands | 189/189 | Inputs are mechanically readable by the current evaluator | `observed-animsmith`; 756 baseline commands completed successfully; constant-track notes and strict loop contracts require review. |
| Declared contracts | 186 selected inputs | Non-clean strict contract results require clip-level admission | `observed-animsmith`; 186 declared-contract inputs: 24 clean and 162 non-clean results in each output format. |

## AnimSmith remediation evidence

| Source issue | Operation/declarations | Result | Independent verification | Remaining caveat |
|---|---|---|---|---|
| Mechanical cleanup and gait candidates | Bounded external transform trials | 25/25 transform commands exit 0 | Scripted post-transform checks recorded externally | Not engine, visual, or artistic acceptance |

## Engine procedures and evidence

| Runtime | Version | Procedure | Observed result | Remaining gate |
|---|---|---|---|---|
| Unity | Not evaluated | No current import | Not evaluated | Import and visual review |
| Unreal Engine | Not evaluated | No current import | Not evaluated | Import and retarget review |
| Godot | Not evaluated | No current import | Not evaluated | Conversion and runtime review |
| Bevy | Not evaluated | No current import | Not evaluated | Conversion and runtime review |

## Rig, masking, and compatibility evidence

| Pack/rig/set pair | Skeleton/retarget | Scale/axes | Root policy | Timing/blend | Overall evidence |
|---|---|---|---|---|---|
| Delivered pack | Mechanical inspection only | Not evaluated | Not evaluated | Strict contracts pending review | No engine compatibility claim |

## Limitations and unknowns

1. Tool evidence does not replace target-engine import, playback, contact, retargeting, masking, or visual review.
2. Strict declared-contract failures are clip-level admission signals, not a blanket statement about authored quality.
3. Candidate transforms remain external and require project acceptance before use.

## Changes between AnimSmith versions

AnimSmith 0.10.0 — revalidated inventory, baseline, contracts, and bounded remediations with the official FBX-capable evaluator. AnimSmith 0.7.0 — historical evidence is retained only for history and is superseded for current-state conclusions.

## Reproduction

Official tag `v0.10.0`; peeled source commit `db91d8dda3326f97f581d4d62104d928caec383f`; evaluator `animsmith 0.10.0`; binary SHA-256 `2052ce64eda53d5037b305561dd0287209719d743b0a4051552e197fbfe4a387`. Bounded help/capability capture and representative FBX admission succeeded before the exhaustive serial batch. External evidence retains version/help, source inventory, baseline, contract, and remediation records. No licensed asset, derivative, private path, or output file is published.

## Sources

- Protofactor product listing and [End User License Agreement](https://protofactor.biz/end-user-license-agreement/) — context only, not local-revision proof.
