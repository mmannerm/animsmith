# Animation pack evidence appendix: Protofactor Sword and Shield Animset

> Companion report: [technical evaluation](protofactor-sword-and-shield.md)
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
| Delivered scope | 136 FBX input candidates |
| Target use | Engine use; no target controller supplied |
| Target engines | Not evaluated |
| Target rigs/packs | Not evaluated |
| Source manifest | External source inventory, SHA-256 `e4dc4740bf35ff2812e81ff78970fc6737e62e8022664643c14b5cb8fdf2e4b8` |
| Evaluation manifest | `urn:animsmith:skill:animation-pack-evaluation-manifest:1` |
| Acquisition/license provenance | Authorized local commercial input; no legal advice |

### Evidence coverage

| Surface | Offered/delivered | Evaluated | Findings | Not evaluated and why |
|---|---:|---:|---:|---|
| Animation files | 136 | 136 | Mechanical baseline completed | N/A |
| Rigs/export variants | 132 animation-bearing candidates | partial | Mechanical inspection only | Runtime compatibility not run |
| AnimSmith baseline | 136 | 136 | 544 baseline commands completed successfully; strict contracts require loop and blend review. | N/A |
| Declared contracts | 132 selected motion inputs | 132 | 132 declared-contract inputs: 34 clean and 230 non-clean command results. | Engine behavior is out of scope |
| Remediation candidates | 28 | 28 | 24 gait-anchor + 3 prune-constant-tracks + 1 duplicate-loop-endpoint removal; all transforms exit 0 | External candidates remain unpromoted; engine/visual review pending |
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

Current inventory found 136 FBX candidates, including 132 animation-bearing candidates, 132 individual-motion candidates, and 0 combined-take candidates. This establishes source presence and mechanical coverage only.

## Mechanical baseline

| Finding/check | Affected scope | Potential impact | Evidence |
|---|---|---|---|
| Baseline commands | 136/136 | Inputs are mechanically readable by the current evaluator | `observed-animsmith`; 544 baseline commands completed successfully; strict contracts require loop and blend review. |
| Declared contracts | 132 selected inputs | Non-clean strict contract results require clip-level admission | `observed-animsmith`; 132 declared-contract inputs: 34 clean and 230 non-clean command results. |

## AnimSmith remediation evidence

| Source issue | Operation/declarations | Result | Independent verification | Remaining caveat |
|---|---|---|---|---|
| Mechanical cleanup and gait candidates | 24 gait-anchor, 3 prune-constant-tracks, 1 duplicate-loop-endpoint removal | 28/28 transform commands exit 0 | Post-output inspect/measure/diff/config-lint recorded in `external:protofactor-melee-remediation-0.10.0/public-evidence-projection.json` (SHA-256 `4004862e2f2f7f3719e796c2cedb9d3c264eb943588d5cb2ae78baf917737ac7`) | External candidates remain unpromoted; no engine, visual, or artistic acceptance |

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

Official release archive URL `https://github.com/mmannerm/animsmith/releases/download/v0.10.0/animsmith-v0.10.0-x86_64-unknown-linux-gnu.tar.gz`; archive SHA-256 `8de4f97949fbc61fc3aec1d5f22272735ffe06937a0fea5c998cb3e0f639c662`; member `animsmith-v0.10.0-x86_64-unknown-linux-gnu/animsmith`; evaluator binary SHA-256 `2052ce64eda53d5037b305561dd0287209719d743b0a4051552e197fbfe4a387`; tag `v0.10.0`, peeled commit `db91d8dda3326f97f581d4d62104d928caec383f`; working tree `N/A`; compiled features `fbx/report`. Expected command/help surface and representative FBX admission succeeded before the exhaustive serial batch. Current operation trace and scrubbed preflight are recorded at `external:protofactor-melee-remediation-0.10.0/public-evidence-projection.json` (SHA-256 `4004862e2f2f7f3719e796c2cedb9d3c264eb943588d5cb2ae78baf917737ac7`). No licensed asset, derivative, private path, or output file is published.

## Sources

- Protofactor product listing and [End User License Agreement](https://protofactor.biz/end-user-license-agreement/) — context only, not local-revision proof.
