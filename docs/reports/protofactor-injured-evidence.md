# Animation pack evidence appendix: Protofactor Injured Animset

> Companion report: [Protofactor Injured report](protofactor-injured.md)
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
| Declared contracts | 70 motion-labelled files | 70 | 28 pass; 42 fail per output format | Mechanical contract results are current; historical taxonomy was not reconstructed; current generic scenarios are separately identified; artistic acceptance remains open |
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

New evaluator-selected generic scenarios: current source bytes establish the exact members, take names, durations, and measured roots below. Names suggest gameplay roles; topology and semantic intent are hypotheses requiring clip review. These are not reconstructed historical manifests or measured collection-output sets. Every listed member uses `Take 001`; full identities are in the external selected-set ledger.

| Runtime set | Type | Members/variants | Grouping evidence | Validation status |
|---|---|---|---|---|
| Injury style A speed | `speed-blend` | `Humanoid@IdleInjuredA.fbx`, `Humanoid@WalkInjuredA.fbx`, `Humanoid@RunInjuredA.fbx`; `Take 001` | New evaluator-selected scenario; observed bytes/timing, inferred gameplay roles | Mechanical measurements current; set/engine/visual acceptance open |
| Injury style B speed | `speed-blend` | `Humanoid@IdleInjuredB.fbx`, `Humanoid@WalkInjuredB.fbx`, `Humanoid@RunInjuredB.fbx`; `Take 001` | New evaluator-selected scenario; observed bytes/timing, inferred gameplay roles | Mechanical measurements current; set/engine/visual acceptance open |
| Injury style C speed | `speed-blend` | `Humanoid@IdleInjuredC.fbx`, `Humanoid@WalkInjuredC.fbx`, `Humanoid@RunInjuredC.fbx`; `Take 001` | New evaluator-selected scenario; observed bytes/timing, inferred gameplay roles | Mechanical measurements current; set/engine/visual acceptance open |
| Injury style D speed | `speed-blend` | `Humanoid@IdleInjuredD.fbx`, `Humanoid@WalkInjuredD.fbx`, `Humanoid@RunInjuredD.fbx`; `Take 001` | New evaluator-selected scenario; observed bytes/timing, inferred gameplay roles | Mechanical measurements current; set/engine/visual acceptance open |
| Injury style E speed | `speed-blend` | `Humanoid@IdleInjuredE.fbx`, `Humanoid@WalkInjuredE.fbx`, `Humanoid@RunInjuredE.fbx`; `Take 001` | New evaluator-selected scenario; observed bytes/timing, inferred gameplay roles | Mechanical measurements current; set/engine/visual acceptance open |
| Injury style F speed | `speed-blend` | `Humanoid@IdleInjuredF.fbx`, `Humanoid@WalkInjuredF.fbx`, `Humanoid@RunInjuredF.fbx`; `Take 001` | New evaluator-selected scenario; observed bytes/timing, inferred gameplay roles | Mechanical measurements current; set/engine/visual acceptance open |
| Injury style G speed | `speed-blend` | `Humanoid@IdleInjuredG.fbx`, `Humanoid@WalkInjuredG.fbx`, `Humanoid@RunInjuredG.fbx`; `Take 001` | New evaluator-selected scenario; observed bytes/timing, inferred gameplay roles | Mechanical measurements current; set/engine/visual acceptance open |

### Exact runtime members

Current source members and measurements for the selected runtime scenarios. Selection is an evaluator hypothesis; source identity and measured values are retained below.

| Set/profile | Role or coordinate | Exact members | Variant/type | Timing or motion | Runtime contract |
|---|---|---|---|---|---|
| Injury style A speed | Proposed idle | `Humanoid@IdleInjuredA.fbx::Take 001` | set_type=speed-blend | duration=2.000 s | loop=unknown; movement=controller; contact=not-evaluated |
| Injury style A speed | Proposed walk | `Humanoid@WalkInjuredA.fbx::Take 001` | set_type=speed-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Injury style A speed | Proposed run | `Humanoid@RunInjuredA.fbx::Take 001` | set_type=speed-blend | duration=0.800 s | loop=unknown; movement=controller; contact=not-evaluated |
| Injury style B speed | Proposed idle | `Humanoid@IdleInjuredB.fbx::Take 001` | set_type=speed-blend | duration=2.000 s | loop=unknown; movement=controller; contact=not-evaluated |
| Injury style B speed | Proposed walk | `Humanoid@WalkInjuredB.fbx::Take 001` | set_type=speed-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Injury style B speed | Proposed run | `Humanoid@RunInjuredB.fbx::Take 001` | set_type=speed-blend | duration=0.800 s | loop=unknown; movement=controller; contact=not-evaluated |
| Injury style C speed | Proposed idle | `Humanoid@IdleInjuredC.fbx::Take 001` | set_type=speed-blend | duration=2.000 s | loop=unknown; movement=controller; contact=not-evaluated |
| Injury style C speed | Proposed walk | `Humanoid@WalkInjuredC.fbx::Take 001` | set_type=speed-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Injury style C speed | Proposed run | `Humanoid@RunInjuredC.fbx::Take 001` | set_type=speed-blend | duration=0.700 s | loop=unknown; movement=controller; contact=not-evaluated |
| Injury style D speed | Proposed idle | `Humanoid@IdleInjuredD.fbx::Take 001` | set_type=speed-blend | duration=1.667 s | loop=unknown; movement=controller; contact=not-evaluated |
| Injury style D speed | Proposed walk | `Humanoid@WalkInjuredD.fbx::Take 001` | set_type=speed-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Injury style D speed | Proposed run | `Humanoid@RunInjuredD.fbx::Take 001` | set_type=speed-blend | duration=0.700 s | loop=unknown; movement=controller; contact=not-evaluated |
| Injury style E speed | Proposed idle | `Humanoid@IdleInjuredE.fbx::Take 001` | set_type=speed-blend | duration=2.000 s | loop=unknown; movement=controller; contact=not-evaluated |
| Injury style E speed | Proposed walk | `Humanoid@WalkInjuredE.fbx::Take 001` | set_type=speed-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Injury style E speed | Proposed run | `Humanoid@RunInjuredE.fbx::Take 001` | set_type=speed-blend | duration=0.800 s | loop=unknown; movement=controller; contact=not-evaluated |
| Injury style F speed | Proposed idle | `Humanoid@IdleInjuredF.fbx::Take 001` | set_type=speed-blend | duration=2.000 s | loop=unknown; movement=controller; contact=not-evaluated |
| Injury style F speed | Proposed walk | `Humanoid@WalkInjuredF.fbx::Take 001` | set_type=speed-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Injury style F speed | Proposed run | `Humanoid@RunInjuredF.fbx::Take 001` | set_type=speed-blend | duration=0.800 s | loop=unknown; movement=controller; contact=not-evaluated |
| Injury style G speed | Proposed idle | `Humanoid@IdleInjuredG.fbx::Take 001` | set_type=speed-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Injury style G speed | Proposed walk | `Humanoid@WalkInjuredG.fbx::Take 001` | set_type=speed-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Injury style G speed | Proposed run | `Humanoid@RunInjuredG.fbx::Take 001` | set_type=speed-blend | duration=0.800 s | loop=unknown; movement=controller; contact=not-evaluated |

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
| Named current generic scenarios above | Exact sources/takes measured; declared lint conditions remain | Candidate topology inferred; no set readiness approval | Target controller, contacts and artistic acceptance not evaluated |

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
| baseline: `constant-track:note` | 72 files; 9915 findings | Policy-dependent; inspect the affected take before assigning artist responsibility | `observed-animsmith`; exact affected files below |
| contracts: `constant-track:note` | 70 files; 9644 findings | Policy-dependent; inspect the affected take before assigning artist responsibility | `observed-animsmith`; exact affected files below |
| contracts: `loop-seam-vel:error` | 31 files; 31 findings | Policy-dependent; inspect the affected take before assigning artist responsibility | `observed-animsmith`; exact affected files below |
| contracts: `loop-seam-rot:error` | 42 files; 42 findings | Policy-dependent; inspect the affected take before assigning artist responsibility | `observed-animsmith`; exact affected files below |
| contracts: `loop-closure:error` | 15 files; 15 findings | Policy-dependent; inspect the affected take before assigning artist responsibility | `observed-animsmith`; exact affected files below |

Per-file contract results: 28 pass and 42 fail of 70; JSON and Markdown agree. Loop declarations are evaluation hypotheses, not proof that every action should repeat.

**loop-seam-vel:error**: `Humanoid@IdleInjuredA.fbx`, `Humanoid@IdleInjuredD.fbx`, `Humanoid@IdleInjuredF.fbx`, `Humanoid@IdleSitInjuredE.fbx`, `Humanoid@RunInjuredA.fbx`, `Humanoid@RunInjuredA_RM.fbx`, `Humanoid@RunInjuredB_RM.fbx`, `Humanoid@RunInjuredC.fbx`, `Humanoid@RunInjuredC_RM.fbx`, `Humanoid@RunInjuredD.fbx`, `Humanoid@RunInjuredD_RM.fbx`, `Humanoid@RunInjuredE.fbx`, `Humanoid@RunInjuredE_RM.fbx`, `Humanoid@RunInjuredF.fbx`, `Humanoid@RunInjuredF_RM.fbx`, `Humanoid@RunInjuredG.fbx`, `Humanoid@RunInjuredG_RM.fbx`, `Humanoid@WalkInjuredA.fbx`, `Humanoid@WalkInjuredA_RM.fbx`, `Humanoid@WalkInjuredB.fbx`, `Humanoid@WalkInjuredB_RM.fbx`, `Humanoid@WalkInjuredC.fbx`, `Humanoid@WalkInjuredC_RM.fbx`, `Humanoid@WalkInjuredD.fbx`, `Humanoid@WalkInjuredD_RM.fbx`, `Humanoid@WalkInjuredE.fbx`, `Humanoid@WalkInjuredE_RM.fbx`, `Humanoid@WalkInjuredF.fbx`, `Humanoid@WalkInjuredF_RM.fbx`, `Humanoid@WalkInjuredG.fbx`, `Humanoid@WalkInjuredG_RM.fbx`.

**loop-seam-rot:error**: `Humanoid@IdleInjuredA.fbx`, `Humanoid@IdleInjuredB.fbx`, `Humanoid@IdleInjuredC.fbx`, `Humanoid@IdleInjuredD.fbx`, `Humanoid@IdleInjuredE.fbx`, `Humanoid@IdleInjuredF.fbx`, `Humanoid@IdleInjuredG.fbx`, `Humanoid@IdleKneelInjuredA.fbx`, `Humanoid@IdleKneelInjuredC.fbx`, `Humanoid@IdleKneelInjuredD.fbx`, `Humanoid@IdleKneelInjuredE.fbx`, `Humanoid@IdleSitInjuredB.fbx`, `Humanoid@IdleSitInjuredD.fbx`, `Humanoid@IdleSitInjuredE.fbx`, `Humanoid@IdleSitInjuredF.fbx`, `Humanoid@RunInjuredA.fbx`, `Humanoid@RunInjuredA_RM.fbx`, `Humanoid@RunInjuredB_RM.fbx`, `Humanoid@RunInjuredC.fbx`, `Humanoid@RunInjuredC_RM.fbx`, `Humanoid@RunInjuredD.fbx`, `Humanoid@RunInjuredD_RM.fbx`, `Humanoid@RunInjuredE.fbx`, `Humanoid@RunInjuredE_RM.fbx`, `Humanoid@RunInjuredF.fbx`, `Humanoid@RunInjuredF_RM.fbx`, `Humanoid@RunInjuredG.fbx`, `Humanoid@RunInjuredG_RM.fbx`, `Humanoid@WalkInjuredA.fbx`, `Humanoid@WalkInjuredA_RM.fbx`, `Humanoid@WalkInjuredB.fbx`, `Humanoid@WalkInjuredB_RM.fbx`, `Humanoid@WalkInjuredC.fbx`, `Humanoid@WalkInjuredC_RM.fbx`, `Humanoid@WalkInjuredD.fbx`, `Humanoid@WalkInjuredD_RM.fbx`, `Humanoid@WalkInjuredE.fbx`, `Humanoid@WalkInjuredE_RM.fbx`, `Humanoid@WalkInjuredF.fbx`, `Humanoid@WalkInjuredF_RM.fbx`, `Humanoid@WalkInjuredG.fbx`, `Humanoid@WalkInjuredG_RM.fbx`.

**loop-closure:error**: `Humanoid@RunInjuredA.fbx`, `Humanoid@RunInjuredA_RM.fbx`, `Humanoid@RunInjuredB_RM.fbx`, `Humanoid@RunInjuredC_RM.fbx`, `Humanoid@RunInjuredD_RM.fbx`, `Humanoid@RunInjuredE_RM.fbx`, `Humanoid@RunInjuredF_RM.fbx`, `Humanoid@RunInjuredG_RM.fbx`, `Humanoid@WalkInjuredA_RM.fbx`, `Humanoid@WalkInjuredB_RM.fbx`, `Humanoid@WalkInjuredC_RM.fbx`, `Humanoid@WalkInjuredD_RM.fbx`, `Humanoid@WalkInjuredE_RM.fbx`, `Humanoid@WalkInjuredF_RM.fbx`, `Humanoid@WalkInjuredG_RM.fbx`.

## AnimSmith remediation evidence

| Source issue | Operation/declarations | Result | Independent verification | Remaining caveat |
|---|---|---|---|---|
| `prune-constant-tracks` trial | Explicit original source/config; `transform --prune-constant-tracks` | 1 candidates emitted; 0 pass selected output lint | Every candidate inspected, measured, linted, diffed and checked with `fix --dry-run`; detailed exit records external | Unpromoted; no candidate engine/contact/visual acceptance |
| `gait-anchor` trial | Explicit original source/config; `transform --gait-anchor` | 14 candidates emitted; 1 pass selected output lint | Every candidate inspected, measured, linted, diffed and checked with `fix --dry-run`; detailed exit records external | Unpromoted; no candidate engine/contact/visual acceptance |

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

AnimSmith 0.14.0 — reran 72 input baselines, 70 per-file declared contracts and 15 remediation trials using the official release. Added current exact-member generic scenarios and developer/artist actions. AnimSmith 0.10.0 — superseded historical evidence; none of its generated outputs or engine results is relabelled as a fresh run.

## Reproduction

Official evaluator preflight: [release archive](https://github.com/mmannerm/animsmith/releases/download/v0.14.0/animsmith-v0.14.0-x86_64-unknown-linux-gnu.tar.gz), archive SHA-256 `4ecf79436f9123c779edb004050da3010b44a2b392e3031facb227d7734fc33e`; exact member `animsmith-v0.14.0-x86_64-unknown-linux-gnu/animsmith`, binary SHA-256 `c2b9649bc74f8a7e5b5d7361a6feaa93941b2327cd7eee82235024bd4f61e366`; tag `v0.14.0`, peeled commit `e8321ad40be5ef6f162b31f085819c039175c3c9`; working-tree state: N/A (official archive). Compiled features: `fbx, report`. Version and required command help plus representative FBX admission passed before evaluation. Preflight: `external:animsmith-0.14.0-report-refresh/preflight.json`, SHA-256 `7034b5043de54e7b30064230db18605d1f96d0ea1477f7b8c56ce817cc73521a`.

Evaluator: [official 0.14.0 Linux release](https://github.com/mmannerm/animsmith/releases/tag/v0.14.0), tag commit `e8321ad40be5ef6f162b31f085819c039175c3c9`; archive SHA-256 `4ecf79436f9123c779edb004050da3010b44a2b392e3031facb227d7734fc33e`; binary SHA-256 `c2b9649bc74f8a7e5b5d7361a6feaa93941b2327cd7eee82235024bd4f61e366`, features `fbx, report`. Output schema v19; measurements schema v18. Version/help and representative admission passed before corpus work. Sources/configurations were hashed before and after read-only runs.

External evidence prefix: `external:animsmith-0.14.0-report-refresh/evaluations/protofactor-injured/`. Commands record exact source/config arguments, exits and raw-output paths. Commercial source and generated motion remain external. Reproduction requires the authorized delivery and captured configuration; generic one-liners below do not substitute for those declarations.

- `baseline/command-results.json`, SHA-256 `e1c37270f1fd4b496845d2a4bc8748b54008b9d4985e159f307c4f9f1cd99979`.
- `contracts/command-results.json`, SHA-256 `f38a687675764b1f6a6bbb78e931aadfec712396b0ebf74390be6392b1aaeb32`.
- `remediation/command-results.json`, SHA-256 `7e924d579e7089edac6c5bb3bd71fee5929d675f47d1b61b1797d7c0bb44c025`.
- `selected-runtime-sets.json`, SHA-256 `edada9b15d75682712eb5467e105d6df08aa804297ea876e251cd67bd641fd48`.
- `current-summary.json`, SHA-256 `d620da6ff1c1b856936ed587cad7a84801461a2a847b3b06ef73290dc76ea1c0`.

```sh
animsmith inspect --config <captured-config> <authorized-source>
animsmith measure --config <captured-config> --format json <authorized-source>
animsmith lint --config <captured-config> --format json <authorized-source>
```

The exact per-trial transform, slice range, configuration and post-check commands are in the remediation ledger. Current scenario sets are new evaluator choices; historical structured-model migration remains outside this refresh.

## Sources

- Protofactor, [Animset: Injured](https://protofactor.biz/product/animset-injured/) — product context.
- AnimSmith, [CLI reference](../cli.md) and [game-ready clips](../game-ready-clips.md) — evaluator boundary.
