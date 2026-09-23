# Animation pack evidence appendix: Protofactor Campfire

> Companion report: [Protofactor Campfire report](protofactor-campfire.md)
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
| Pack/edition | Protofactor Campfire; local constituent revision unknown |
| Vendor/source | [Protofactor Animset: Campfire](https://protofactor.biz/product/animset-campfire/) |
| Delivered scope | Authorized local commercial delivery; 29 FBXs |
| Target use | Engine-neutral campfire interaction intake |
| Target engines | Unity, Unreal Engine, Godot, and Bevy; not evaluated |
| Target rigs/packs | No target character or current cross-pack run supplied |
| Source manifest | External scrubbed inventory, SHA-256 `122e9fe58cb2c5985d861724a41d709e28cf0f2f48408181e848b69b4d954338` |
| Evaluation manifest | Unavailable: current output binding was not rendered for this retained report format |
| Acquisition/license provenance | Authorized commercial bytes; no transaction or redistribution conclusion |

### Evidence coverage

| Surface | Offered/delivered | Evaluated | Findings | Not evaluated and why |
|---|---:|---:|---:|---|
| Animation files | 29 FBXs | 29 | baseline completed | mechanical and contract evidence captured |
| Rigs/export variants | Unknown | 0 | 0 | Not evaluated in this run |
| AnimSmith baseline | 29 | 29 | current findings recorded | `inspect`, JSON `measure`, JSON `lint`, and Markdown `lint` completed per file |
| Declared contracts | 25 motion-labelled files | 25 | 17 pass; 8 fail per output format | Mechanical contract results are current; historical taxonomy was not reconstructed; current generic scenarios are separately identified; artistic acceptance remains open |
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
| **Total** | **0** | **0** | 29 delivered FBXs remain unclassified |

### Runtime-set inventory

New evaluator-selected generic scenarios: current source bytes establish the exact members, take names, durations, and measured roots below. Names suggest gameplay roles; topology and semantic intent are hypotheses requiring clip review. These are not reconstructed historical manifests or measured collection-output sets. Every listed member uses `Take 001`; full identities are in the external selected-set ledger.

| Runtime set | Type | Members/variants | Grouping evidence | Validation status |
|---|---|---|---|---|
| Kneel and sit sequence | `transition-chain` | `Humanoid@StandToKneelCampfire.fbx`, `Humanoid@IdleKneelCampfire.fbx`, `Humanoid@KneelToSitCampfire.fbx`, `Humanoid@IdleSitCampfire.fbx`; `Take 001` | New evaluator-selected scenario; observed bytes/timing, inferred gameplay roles | Mechanical measurements current; set/engine/visual acceptance open |
| Sit and lie sequence | `transition-chain` | `Humanoid@IdleSitCampfire.fbx`, `Humanoid@IdleSitToIdleLayDownCampfire.fbx`, `Humanoid@IdleLayDownCampfire.fbx`, `Humanoid@IdleLayDownToIdleSitCampfire.fbx`; `Take 001` | New evaluator-selected scenario; observed bytes/timing, inferred gameplay roles | Mechanical measurements current; set/engine/visual acceptance open |
| Grill interaction sequence | `transition-chain` | `Humanoid@IdleKneelCampfire.fbx`, `Humanoid@IdleKneelToIdleGrillSkewerCampfire.fbx`, `Humanoid@IdleGrillSkewerCampfire.fbx`, `Humanoid@KneelEatSkewerCampfire.fbx`; `Take 001` | New evaluator-selected scenario; observed bytes/timing, inferred gameplay roles | Mechanical measurements current; set/engine/visual acceptance open |

### Exact runtime members

Current source members and measurements for the selected runtime scenarios. Selection is an evaluator hypothesis; source identity and measured values are retained below.

| Set/profile | Role or coordinate | Exact members | Variant/type | Timing or motion | Runtime contract |
|---|---|---|---|---|---|
| Kneel and sit sequence | Candidate member 1 | `Humanoid@StandToKneelCampfire.fbx::Take 001` | set_type=transition-chain | duration=4.500 s | loop=unknown; movement=controller; contact=not-evaluated |
| Kneel and sit sequence | Candidate member 2 | `Humanoid@IdleKneelCampfire.fbx::Take 001` | set_type=transition-chain | duration=2.167 s | loop=unknown; movement=controller; contact=not-evaluated |
| Kneel and sit sequence | Candidate member 3 | `Humanoid@KneelToSitCampfire.fbx::Take 001` | set_type=transition-chain | duration=4.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Kneel and sit sequence | Candidate member 4 | `Humanoid@IdleSitCampfire.fbx::Take 001` | set_type=transition-chain | duration=2.667 s | loop=unknown; movement=controller; contact=not-evaluated |
| Sit and lie sequence | Candidate member 1 | `Humanoid@IdleSitCampfire.fbx::Take 001` | set_type=transition-chain | duration=2.667 s | loop=unknown; movement=controller; contact=not-evaluated |
| Sit and lie sequence | Candidate member 2 | `Humanoid@IdleSitToIdleLayDownCampfire.fbx::Take 001` | set_type=transition-chain | duration=4.000 s | loop=unknown; movement=controller; contact=not-evaluated |
| Sit and lie sequence | Candidate member 3 | `Humanoid@IdleLayDownCampfire.fbx::Take 001` | set_type=transition-chain | duration=1.967 s | loop=unknown; movement=controller; contact=not-evaluated |
| Sit and lie sequence | Candidate member 4 | `Humanoid@IdleLayDownToIdleSitCampfire.fbx::Take 001` | set_type=transition-chain | duration=2.667 s | loop=unknown; movement=controller; contact=not-evaluated |
| Grill interaction sequence | Candidate member 1 | `Humanoid@IdleKneelCampfire.fbx::Take 001` | set_type=transition-chain | duration=2.167 s | loop=unknown; movement=controller; contact=not-evaluated |
| Grill interaction sequence | Candidate member 2 | `Humanoid@IdleKneelToIdleGrillSkewerCampfire.fbx::Take 001` | set_type=transition-chain | duration=3.667 s | loop=unknown; movement=controller; contact=not-evaluated |
| Grill interaction sequence | Candidate member 3 | `Humanoid@IdleGrillSkewerCampfire.fbx::Take 001` | set_type=transition-chain | duration=3.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Grill interaction sequence | Candidate member 4 | `Humanoid@KneelEatSkewerCampfire.fbx::Take 001` | set_type=transition-chain | duration=11.667 s | loop=unknown; movement=controller; contact=not-evaluated |

### Pipeline-stage coverage

| Stage | Coverage state | Evidence / remaining gate |
|---|---|---|
| Acquire | `partially-evaluated` | Authorized local inventory |
| Preserve raw | `evaluated-clean` | Source unchanged |
| Inspect | `evaluated-finding` | 29 baseline attempts completed |
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
| Root-motion controller | `not-selected` | No root-motion controller scope |
| State-machine transitions | `selected` — `evaluator-selected-generic-scenario` | Requires contract-qualified clip selection and engine test |
| Layered upper body/weapons | `not-selected` | No current action scope |
| Traversal/environment | `not-selected` | No traversal scope |
| Contact actions/interactions | `selected` — `evaluator-selected-generic-scenario` | Requires declared contact/prop criteria and engine/visual review |
| Retargeted/customizable characters | `not-selected` | No target rig supplied |
| Motion matching/search | `not-selected` | No database scope |
| Networked movement | `not-selected` | No controller scope |
| Runtime performance | `not-selected` | No runtime build |

## Pack inventory and content evidence

The scrubbed inventory records 114 regular files totaling 188,335,953 bytes; 29 are FBX candidates. Filename labels are not semantic or contact evidence.

## Mechanical baseline

| Finding/check | Affected scope | Potential impact | Evidence |
|---|---|---|---|
| baseline: `constant-track:note` | 27 files; 3664 findings | Policy-dependent; inspect the affected take before assigning artist responsibility | `observed-animsmith`; exact affected files below |
| contracts: `constant-track:note` | 25 files; 3394 findings | Policy-dependent; inspect the affected take before assigning artist responsibility | `observed-animsmith`; exact affected files below |
| contracts: `loop-seam-vel:error` | 6 files; 6 findings | Policy-dependent; inspect the affected take before assigning artist responsibility | `observed-animsmith`; exact affected files below |
| contracts: `loop-seam-rot:error` | 8 files; 8 findings | Policy-dependent; inspect the affected take before assigning artist responsibility | `observed-animsmith`; exact affected files below |
| contracts: `loop-closure:error` | 1 files; 2 findings | Policy-dependent; inspect the affected take before assigning artist responsibility | `observed-animsmith`; exact affected files below |

Per-file contract results: 17 pass and 8 fail of 25; JSON and Markdown agree. Loop declarations are evaluation hypotheses, not proof that every action should repeat.

**loop-seam-vel:error**: `Humanoid@FlintstonesLightCampfire.fbx`, `Humanoid@IdleGrillSkewerCampfire.fbx`, `Humanoid@KneelTossLogCampfire.fbx`, `Humanoid@LighterLightCampfire.fbx`, `Humanoid@MatchesLightCampfire.fbx`, `Humanoid@StickLightCampfire.fbx`.

**loop-seam-rot:error**: `Humanoid@FlintstonesLightCampfire.fbx`, `Humanoid@IdleGrillSkewerCampfire.fbx`, `Humanoid@IdleKneelCampfire.fbx`, `Humanoid@IdleKneelToIdleGrillSkewerCampfire.fbx`, `Humanoid@KneelTossLogCampfire.fbx`, `Humanoid@LighterLightCampfire.fbx`, `Humanoid@MatchesLightCampfire.fbx`, `Humanoid@StickLightCampfire.fbx`.

**loop-closure:error**: `Humanoid@IdleKneelToIdleGrillSkewerCampfire.fbx`.

## AnimSmith remediation evidence

| Source issue | Operation/declarations | Result | Independent verification | Remaining caveat |
|---|---|---|---|---|
| `prune-constant-tracks` trial | Explicit original source/config; `transform --prune-constant-tracks` | 1 candidates emitted; 0 pass selected output lint | Every candidate inspected, measured, linted, diffed and checked with `fix --dry-run`; detailed exit records external | Unpromoted; no candidate engine/contact/visual acceptance |

## Engine procedures and evidence

| Runtime | Version | Procedure | Observed result | Remaining gate |
|---|---|---|---|---|
| Unity | unspecified | None in current run | not evaluated | Disposable import, visual interaction, contact, and build test |
| Unreal Engine | unspecified | None in current run | not evaluated | Disposable import, retarget, contact, and build test |
| Godot | unspecified | None in current run | not evaluated | Disposable conversion/import and graph test |
| Bevy | unspecified | None in current run | not evaluated | Selected export handoff and runtime test |

## Rig, masking, and compatibility evidence

| Pack/rig/set pair | Skeleton/retarget | Scale/axes | Root policy | Timing/blend | Overall evidence |
|---|---|---|---|---|---|
| Campfire/current evaluator | not evaluated | not evaluated | not evaluated | not evaluated | Current baseline evidence |

## Limitations and unknowns

1. No current engine, visual, contact, retarget, or artistic test ran.
2. Commercial sources and derivatives remain external.

## Changes between AnimSmith versions

AnimSmith 0.14.0 — reran 29 input baselines, 25 per-file declared contracts and one remediation trial using the official release. Added current exact-member generic scenarios and developer/artist actions. AnimSmith 0.10.0 — superseded historical evidence; none of its generated outputs or engine results is relabelled as a fresh run.

## Reproduction

Official evaluator preflight: [release archive](https://github.com/mmannerm/animsmith/releases/download/v0.14.0/animsmith-v0.14.0-x86_64-unknown-linux-gnu.tar.gz), archive SHA-256 `4ecf79436f9123c779edb004050da3010b44a2b392e3031facb227d7734fc33e`; exact member `animsmith-v0.14.0-x86_64-unknown-linux-gnu/animsmith`, binary SHA-256 `c2b9649bc74f8a7e5b5d7361a6feaa93941b2327cd7eee82235024bd4f61e366`; tag `v0.14.0`, peeled commit `e8321ad40be5ef6f162b31f085819c039175c3c9`; working-tree state: N/A (official archive). Compiled features: `fbx, report`. Version and required command help plus representative FBX admission passed before evaluation. Preflight: `external:animsmith-0.14.0-report-refresh/preflight.json`, SHA-256 `7034b5043de54e7b30064230db18605d1f96d0ea1477f7b8c56ce817cc73521a`.

Evaluator: [official 0.14.0 Linux release](https://github.com/mmannerm/animsmith/releases/tag/v0.14.0), tag commit `e8321ad40be5ef6f162b31f085819c039175c3c9`; archive SHA-256 `4ecf79436f9123c779edb004050da3010b44a2b392e3031facb227d7734fc33e`; binary SHA-256 `c2b9649bc74f8a7e5b5d7361a6feaa93941b2327cd7eee82235024bd4f61e366`, features `fbx, report`. Output schema v19; measurements schema v18. Version/help and representative admission passed before corpus work. Sources/configurations were hashed before and after read-only runs.

External evidence prefix: `external:animsmith-0.14.0-report-refresh/evaluations/protofactor-campfire/`. Commands record exact source/config arguments, exits and raw-output paths. Commercial source and generated motion remain external. Reproduction requires the authorized delivery and captured configuration; generic one-liners below do not substitute for those declarations.

- `baseline/command-results.json`, SHA-256 `9adacb0d5337653263819ab80646744463c5826945f3efa3d2d30b6c967593db`.
- `contracts/command-results.json`, SHA-256 `d98b6436e3401ccea7a3b50aee7b98384df760f9d507f1f1c51cc86cccc4e070`.
- `remediation/command-results.json`, SHA-256 `fc78fb24af301fbfe6ddd543649150556fb4face43e1d093c9bb6445da9c89a9`.
- `selected-runtime-sets.json`, SHA-256 `ae84f5b67615281775432a0713a3e2d813ae16a6f8e699f4802d039f789fba72`.
- `current-summary.json`, SHA-256 `468e0386579f146b795e7b780f119c8b800d7299bfb343f6895e5b710f4b6a5c`.

```sh
animsmith inspect --config <captured-config> <authorized-source>
animsmith measure --config <captured-config> --format json <authorized-source>
animsmith lint --config <captured-config> --format json <authorized-source>
```

The exact per-trial transform, slice range, configuration and post-check commands are in the remediation ledger. Current scenario sets are new evaluator choices; historical structured-model migration remains outside this refresh.

## Sources

- Protofactor, [Animset: Campfire](https://protofactor.biz/product/animset-campfire/) — product context.
- AnimSmith, [CLI reference](../cli.md) and [game-ready clips](../game-ready-clips.md) — evaluator boundary.
