# Animation pack evidence appendix: Protofactor Climbing Animset

> Companion report: [Protofactor Climbing report](protofactor-climbing.md)
>
> Evidence status: **partial** — the official evaluator loaded the delivered FBXs and produced current baseline evidence.
>
> Evaluation date: **2026-09-16**
>
> Current evaluator: **AnimSmith 0.14.0**
>
> Report format: **3**

The [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder) remains authoritative.

Evaluation manifest schema: `urn:animsmith:skill:animation-pack-evaluation-manifest:1`.

## Evaluation scope and provenance

| Field | Value |
|---|---|
| Pack/edition | Protofactor Climbing Animset; local constituent revision unknown |
| Vendor/source | [Protofactor Animset: Climbing](https://protofactor.biz/product/animset-climbing/) |
| Delivered scope | Authorized local commercial delivery; 77 FBXs |
| Target use | Engine-neutral traversal intake |
| Target engines | Unity, Unreal Engine, Godot, and Bevy; not evaluated |
| Target rigs/packs | No target character or current cross-pack run supplied |
| Source manifest | External scrubbed inventory, SHA-256 `9646d3aa9057fbfe58d612642d1a8c6b72039a5c0f6fec8ee60f27d0fb66c12a` |
| Evaluation manifest | Unavailable: current output binding was not rendered for this retained report format |
| Acquisition/license provenance | Authorized commercial bytes; no transaction or redistribution conclusion |

### Evidence coverage

| Surface | Offered/delivered | Evaluated | Findings | Not evaluated and why |
|---|---:|---:|---:|---|
| Animation files | 77 FBXs | 77 | baseline completed | mechanical and contract evidence captured |
| Rigs/export variants | Unknown | 0 | 0 | Not evaluated in this run |
| AnimSmith baseline | 77 | 77 | current findings recorded | `inspect`, JSON `measure`, JSON `lint`, and Markdown `lint` completed per file |
| Declared contracts | 75 motion-labelled files | 75 | 34 pass; 41 fail per output format | Mechanical contract results are current; historical taxonomy was not reconstructed; current generic scenarios are separately identified; artistic acceptance remains open |
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
| **Total** | **0** | **0** | 77 delivered FBXs remain unclassified |

### Runtime-set inventory

New evaluator-selected generic scenarios: current source bytes establish the exact members, take names, durations, and measured roots below. Names suggest gameplay roles; topology and semantic intent are hypotheses requiring clip review. These are not reconstructed historical manifests or measured collection-output sets. Every listed member uses `Take 001`; full identities are in the external selected-set ledger.

| Runtime set | Type | Members/variants | Grouping evidence | Validation status |
|---|---|---|---|---|
| Wall eight directions | `directional-blend` | `Humanoid@WallClimbUp.fbx`, `Humanoid@WallClimbUpLeft.fbx`, `Humanoid@WallClimbLeft.fbx`, `Humanoid@WallClimbDownLeft.fbx`, `Humanoid@WallClimbDown.fbx`, `Humanoid@WallClimbDownRight.fbx`, `Humanoid@WallClimbRight.fbx`, `Humanoid@WallClimbUpRight.fbx`; `Take 001` | New evaluator-selected scenario; observed bytes/timing, inferred gameplay roles | Mechanical measurements current; set/engine/visual acceptance open |
| Ladder up and down | `directional-blend` | `Humanoid@ClimbUpLadder.fbx`, `Humanoid@ClimbDownLadder.fbx`; `Take 001` | New evaluator-selected scenario; observed bytes/timing, inferred gameplay roles | Mechanical measurements current; set/engine/visual acceptance open |

### Exact runtime members

Current source members and measurements for the selected runtime scenarios. Selection is an evaluator hypothesis; source identity and measured values are retained below.

| Set/profile | Role or coordinate | Exact members | Variant/type | Timing or motion | Runtime contract |
|---|---|---|---|---|---|
| Wall eight directions | Proposed up (0,1) | `Humanoid@WallClimbUp.fbx::Take 001` | set_type=directional-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Wall eight directions | Proposed up-left (-1,1) | `Humanoid@WallClimbUpLeft.fbx::Take 001` | set_type=directional-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Wall eight directions | Proposed left (-1,0) | `Humanoid@WallClimbLeft.fbx::Take 001` | set_type=directional-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Wall eight directions | Proposed down-left (-1,-1) | `Humanoid@WallClimbDownLeft.fbx::Take 001` | set_type=directional-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Wall eight directions | Proposed down (0,-1) | `Humanoid@WallClimbDown.fbx::Take 001` | set_type=directional-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Wall eight directions | Proposed down-right (1,-1) | `Humanoid@WallClimbDownRight.fbx::Take 001` | set_type=directional-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Wall eight directions | Proposed right (1,0) | `Humanoid@WallClimbRight.fbx::Take 001` | set_type=directional-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Wall eight directions | Proposed up-right (1,1) | `Humanoid@WallClimbUpRight.fbx::Take 001` | set_type=directional-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Ladder up and down | Proposed up (0,1) | `Humanoid@ClimbUpLadder.fbx::Take 001` | set_type=directional-blend | duration=1.200 s | loop=unknown; movement=controller; contact=not-evaluated |
| Ladder up and down | Proposed down | `Humanoid@ClimbDownLadder.fbx::Take 001` | set_type=directional-blend | duration=1.200 s | loop=unknown; movement=controller; contact=not-evaluated |

### Pipeline-stage coverage

| Stage | Coverage state | Evidence / remaining gate |
|---|---|---|
| Acquire | `partially-evaluated` | Authorized local inventory |
| Preserve raw | `evaluated-clean` | Source unchanged |
| Inspect | `evaluated-finding` | 77 baseline attempts completed |
| Segment | `not-evaluated` | No separate segmentation trial selected |
| Root motion | `not-evaluated` | Measurements completed; no root-motion policy or controller acceptance was evaluated |
| Conform | `not-evaluated` | No target rig or conformance policy was selected |
| Validate | `evaluated-finding` | Lint completed for every FBX |
| Optimize | `partially-evaluated` | One external constant-track candidate was generated; source was unchanged and runtime equivalence remains unproved |
| Export | `partially-evaluated` | Candidate was written as an external GLB only; no engine handoff was selected |
| Gate/report | `partially-evaluated` | Current baseline, remediation, and boundary recorded |

### Readiness evidence by clip set

| Role or runtime set | File-ready / clip-ready | Set-ready / rig-use | Runtime / acceptance boundary |
|---|---|---|---|
| Entire delivered corpus | evaluated-finding | not-evaluated | not-evaluated |
| Named current generic scenarios above | Exact sources/takes measured; declared lint conditions remain | Candidate topology inferred; no set readiness approval | Target controller, contacts and artistic acceptance not evaluated |

### Validation-profile status

| Validation profile | Selection | Result / next evidence |
|---|---|---|
| Marketplace intake | `selected` — `evaluator-selected-generic-scenario` | Inventory and current baseline captured |
| Blended locomotion | `not-selected` | No current locomotion scope |
| Root-motion controller | `selected` — `evaluator-selected-generic-scenario` | Measurements completed; controller trajectory policy and engine acceptance remain |
| State-machine transitions | `selected` — `evaluator-selected-generic-scenario` | Requires contract-qualified clip selection and engine test |
| Layered upper body/weapons | `not-selected` | No current action scope |
| Traversal/environment | `selected` — `evaluator-selected-generic-scenario` | Requires traversal and contact review |
| Contact actions/interactions | `selected` — `evaluator-selected-generic-scenario` | Requires obstacle/ladder evidence |
| Retargeted/customizable characters | `not-selected` | No target rig supplied |
| Motion matching/search | `not-selected` | No database scope |
| Networked movement | `not-selected` | No controller scope |
| Runtime performance | `not-selected` | No runtime build |

## Pack inventory and content evidence

The scrubbed inventory records 179 regular files totaling 194,505,327 bytes; 77 are FBX candidates. Filename labels are not semantic, contact, or root-motion evidence.

## Mechanical baseline

| Finding/check | Affected scope | Potential impact | Evidence |
|---|---|---|---|
| baseline: `constant-track:note` | 77 files; 9011 findings | Policy-dependent; inspect the affected take before assigning artist responsibility | `observed-animsmith`; exact affected files below |
| contracts: `constant-track:note` | 75 files; 8753 findings | Policy-dependent; inspect the affected take before assigning artist responsibility | `observed-animsmith`; exact affected files below |
| contracts: `loop-seam-vel:error` | 39 files; 39 findings | Policy-dependent; inspect the affected take before assigning artist responsibility | `observed-animsmith`; exact affected files below |
| contracts: `loop-seam-rot:error` | 41 files; 41 findings | Policy-dependent; inspect the affected take before assigning artist responsibility | `observed-animsmith`; exact affected files below |
| contracts: `loop-closure:error` | 22 files; 26 findings | Policy-dependent; inspect the affected take before assigning artist responsibility | `observed-animsmith`; exact affected files below |

Per-file contract results: 34 pass and 41 fail of 75; JSON and Markdown agree. Loop declarations are evaluation hypotheses, not proof that every action should repeat.

**loop-seam-vel:error**: `Humanoid@ClimbDownLadder.fbx`, `Humanoid@ClimbDownLadder_RM.fbx`, `Humanoid@ClimbUp1MeterObstacleUnarmed.FBX`, `Humanoid@ClimbUp1MeterObstacleUnarmed_RM.FBX`, `Humanoid@ClimbUp2MetersObstacleUnarmed.FBX`, `Humanoid@ClimbUp2MetersObstacleUnarmed_RM.FBX`, `Humanoid@ClimbUpHalfMeterObstacleLeftUnarmed.fbx`, `Humanoid@ClimbUpHalfMeterObstacleLeftUnarmed_RM.fbx`, `Humanoid@ClimbUpHalfMeterObstacleRightUnarmed.fbx`, `Humanoid@ClimbUpHalfMeterObstacleRightUnarmed_RM.fbx`, `Humanoid@ClimbUpLadder.fbx`, `Humanoid@ClimbUpLadder_RM.fbx`, `Humanoid@FallingUnarmed.FBX`, `Humanoid@IdleLookAroundWallClimb.fbx`, `Humanoid@IdleWallClimb.fbx`, `Humanoid@WallClimbDown.fbx`, `Humanoid@WallClimbDown_RM.fbx`, `Humanoid@WallClimbDownLeft.fbx`, `Humanoid@WallClimbDownLeft_RM.fbx`, `Humanoid@WallClimbDownRight.fbx`, `Humanoid@WallClimbDownRight_RM.fbx`, `Humanoid@WallClimbLeft.fbx`, `Humanoid@WallClimbLeft_RM.fbx`, `Humanoid@WallClimbRight.fbx`, `Humanoid@WallClimbRight_RM.fbx`, `Humanoid@WallClimbUp.fbx`, `Humanoid@WallClimbUp_RM.fbx`, `Humanoid@WallClimbUpLeft.fbx`, `Humanoid@WallClimbUpLeft_RM.fbx`, `Humanoid@WallClimbUpRight.fbx`, `Humanoid@WallClimbUpRight_RM.fbx`, `Humanoid@WallJumpDown.fbx`, `Humanoid@WallJumpDown_RM.fbx`, `Humanoid@WallJumpLeft.fbx`, `Humanoid@WallJumpLeft_RM.fbx`, `Humanoid@WallJumpRight.fbx`, `Humanoid@WallJumpRight_RM.fbx`, `Humanoid@WallJumpUp.fbx`, `Humanoid@WallJumpUp_RM.fbx`.

**loop-seam-rot:error**: `Humanoid@ClimbDownLadder.fbx`, `Humanoid@ClimbDownLadder_RM.fbx`, `Humanoid@ClimbUp1MeterObstacleUnarmed.FBX`, `Humanoid@ClimbUp1MeterObstacleUnarmed_RM.FBX`, `Humanoid@ClimbUp2MetersObstacleUnarmed.FBX`, `Humanoid@ClimbUp2MetersObstacleUnarmed_RM.FBX`, `Humanoid@ClimbUpHalfMeterObstacleLeftUnarmed.fbx`, `Humanoid@ClimbUpHalfMeterObstacleLeftUnarmed_RM.fbx`, `Humanoid@ClimbUpHalfMeterObstacleRightUnarmed.fbx`, `Humanoid@ClimbUpHalfMeterObstacleRightUnarmed_RM.fbx`, `Humanoid@ClimbUpLadder.fbx`, `Humanoid@ClimbUpLadder_RM.fbx`, `Humanoid@FallingUnarmed.FBX`, `Humanoid@IdleLookAroundWallClimb.fbx`, `Humanoid@IdlePrepareJumpOppositeWallLeft.fbx`, `Humanoid@IdlePrepareJumpOppositeWallRight.fbx`, `Humanoid@IdleWallClimb.fbx`, `Humanoid@WallClimbDown.fbx`, `Humanoid@WallClimbDown_RM.fbx`, `Humanoid@WallClimbDownLeft.fbx`, `Humanoid@WallClimbDownLeft_RM.fbx`, `Humanoid@WallClimbDownRight.fbx`, `Humanoid@WallClimbDownRight_RM.fbx`, `Humanoid@WallClimbLeft.fbx`, `Humanoid@WallClimbLeft_RM.fbx`, `Humanoid@WallClimbRight.fbx`, `Humanoid@WallClimbRight_RM.fbx`, `Humanoid@WallClimbUp.fbx`, `Humanoid@WallClimbUp_RM.fbx`, `Humanoid@WallClimbUpLeft.fbx`, `Humanoid@WallClimbUpLeft_RM.fbx`, `Humanoid@WallClimbUpRight.fbx`, `Humanoid@WallClimbUpRight_RM.fbx`, `Humanoid@WallJumpDown.fbx`, `Humanoid@WallJumpDown_RM.fbx`, `Humanoid@WallJumpLeft.fbx`, `Humanoid@WallJumpLeft_RM.fbx`, `Humanoid@WallJumpRight.fbx`, `Humanoid@WallJumpRight_RM.fbx`, `Humanoid@WallJumpUp.fbx`, `Humanoid@WallJumpUp_RM.fbx`.

**loop-closure:error**: `Humanoid@ClimbDownLadder_RM.fbx`, `Humanoid@ClimbUp1MeterObstacleUnarmed.FBX`, `Humanoid@ClimbUp1MeterObstacleUnarmed_RM.FBX`, `Humanoid@ClimbUp2MetersObstacleUnarmed_RM.FBX`, `Humanoid@ClimbUpHalfMeterObstacleLeftUnarmed.fbx`, `Humanoid@ClimbUpHalfMeterObstacleLeftUnarmed_RM.fbx`, `Humanoid@ClimbUpHalfMeterObstacleRightUnarmed.fbx`, `Humanoid@ClimbUpHalfMeterObstacleRightUnarmed_RM.fbx`, `Humanoid@ClimbUpLadder_RM.fbx`, `Humanoid@WallClimbDown_RM.fbx`, `Humanoid@WallClimbDownLeft_RM.fbx`, `Humanoid@WallClimbDownRight_RM.fbx`, `Humanoid@WallClimbLeft.fbx`, `Humanoid@WallClimbLeft_RM.fbx`, `Humanoid@WallClimbRight_RM.fbx`, `Humanoid@WallClimbUp_RM.fbx`, `Humanoid@WallClimbUpLeft_RM.fbx`, `Humanoid@WallClimbUpRight_RM.fbx`, `Humanoid@WallJumpDown_RM.fbx`, `Humanoid@WallJumpLeft_RM.fbx`, `Humanoid@WallJumpRight_RM.fbx`, `Humanoid@WallJumpUp_RM.fbx`.

## AnimSmith remediation evidence

| Source issue | Operation/declarations | Result | Independent verification | Remaining caveat |
|---|---|---|---|---|
| `prune-constant-tracks` trial | Explicit original source/config; `transform --prune-constant-tracks` | 1 candidates emitted; 0 pass selected output lint | Every candidate inspected, measured, linted, diffed and checked with `fix --dry-run`; detailed exit records external | Unpromoted; no candidate engine/contact/visual acceptance |

## Engine procedures and evidence

| Runtime | Version | Procedure | Observed result | Remaining gate |
|---|---|---|---|---|
| Unity | unspecified | None in current run | not evaluated | Disposable import, traversal, contact, and visual test |
| Unreal Engine | unspecified | None in current run | not evaluated | Disposable import, retarget, IK, contact, and build test |
| Godot | unspecified | None in current run | not evaluated | Disposable conversion/import and graph test |
| Bevy | unspecified | None in current run | not evaluated | Selected export handoff and runtime test |

## Rig, masking, and compatibility evidence

| Pack/rig/set pair | Skeleton/retarget | Scale/axes | Root policy | Timing/blend | Overall evidence |
|---|---|---|---|---|---|
| Climbing/current evaluator | not evaluated | not evaluated | not evaluated | not evaluated | Current baseline evidence |

## Limitations and unknowns

1. No current engine, visual, contact, retarget, or artistic test ran.
2. Commercial sources and derivatives remain external.

## Changes between AnimSmith versions

AnimSmith 0.14.0 — reran 77 input baselines, 75 per-file declared contracts and one remediation trial using the official release. Added current exact-member generic scenarios and developer/artist actions. AnimSmith 0.10.0 — superseded historical evidence; none of its generated outputs or engine results is relabelled as a fresh run.

## Reproduction

Official evaluator preflight: [release archive](https://github.com/mmannerm/animsmith/releases/download/v0.14.0/animsmith-v0.14.0-x86_64-unknown-linux-gnu.tar.gz), archive SHA-256 `4ecf79436f9123c779edb004050da3010b44a2b392e3031facb227d7734fc33e`; exact member `animsmith-v0.14.0-x86_64-unknown-linux-gnu/animsmith`, binary SHA-256 `c2b9649bc74f8a7e5b5d7361a6feaa93941b2327cd7eee82235024bd4f61e366`; tag `v0.14.0`, peeled commit `e8321ad40be5ef6f162b31f085819c039175c3c9`; working-tree state: N/A (official archive). Compiled features: `fbx, report`. Version and required command help plus representative FBX admission passed before evaluation. Preflight: `external:animsmith-0.14.0-report-refresh/preflight.json`, SHA-256 `7034b5043de54e7b30064230db18605d1f96d0ea1477f7b8c56ce817cc73521a`.

Evaluator: [official 0.14.0 Linux release](https://github.com/mmannerm/animsmith/releases/tag/v0.14.0), tag commit `e8321ad40be5ef6f162b31f085819c039175c3c9`; archive SHA-256 `4ecf79436f9123c779edb004050da3010b44a2b392e3031facb227d7734fc33e`; binary SHA-256 `c2b9649bc74f8a7e5b5d7361a6feaa93941b2327cd7eee82235024bd4f61e366`, features `fbx, report`. Output schema v19; measurements schema v18. Version/help and representative admission passed before corpus work. Sources/configurations were hashed before and after read-only runs.

External evidence prefix: `external:animsmith-0.14.0-report-refresh/evaluations/protofactor-climbing/`. Commands record exact source/config arguments, exits and raw-output paths. Commercial source and generated motion remain external. Reproduction requires the authorized delivery and captured configuration; generic one-liners below do not substitute for those declarations.

- `baseline/command-results.json`, SHA-256 `a0b70aa8d224845797d296567e56d430ec4a9b4f18179f1a4ba23c921338c519`.
- `contracts/command-results.json`, SHA-256 `c736fb5f2f79b8044d9a2fc51707470eccabeda81a6a9c878fd6b2847d8c2046`.
- `remediation/command-results.json`, SHA-256 `dca43e1b41a0561de9cd395f266cf5aeab5cb239a20ae8c407d1753f33c3387f`.
- `selected-runtime-sets.json`, SHA-256 `f2a494df4894caff8640995b88aff8a0a7d435f9d231db0f421ede4df182dccf`.
- `current-summary.json`, SHA-256 `f047b5c288f4ba004f4635e6319f232b2616f5a0c3ee492ddbef7a527b1155ef`.

```sh
animsmith inspect --config <captured-config> <authorized-source>
animsmith measure --config <captured-config> --format json <authorized-source>
animsmith lint --config <captured-config> --format json <authorized-source>
```

The exact per-trial transform, slice range, configuration and post-check commands are in the remediation ledger. Current scenario sets are new evaluator choices; historical structured-model migration remains outside this refresh.

## Sources

- Protofactor, [Animset: Climbing](https://protofactor.biz/product/animset-climbing/) — product context.
- AnimSmith, [CLI reference](../cli.md) and [game-ready clips](../game-ready-clips.md) — evaluator boundary.
