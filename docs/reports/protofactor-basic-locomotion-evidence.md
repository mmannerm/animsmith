# Animation pack evidence appendix: Protofactor Basic Locomotion Animset

> Companion report: [Protofactor Basic Locomotion report](protofactor-basic-locomotion.md)
>
> Evidence status: **partial** — the official evaluator loaded the delivered FBXs and produced current baseline evidence.
>
> Evaluation date: **2026-09-16**
>
> Current evaluator: **AnimSmith 0.14.0**
>
> Report format: **3**

This appendix preserves current evidence only. The [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder) remains authoritative.

Evaluation manifest schema: `urn:animsmith:skill:animation-pack-evaluation-manifest:1`.

## Evaluation scope and provenance

| Field | Value |
|---|---|
| Pack/edition | Protofactor Basic Locomotion Animset; local constituent revision unknown |
| Vendor/source | [Protofactor Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/) |
| Delivered scope | Authorized local commercial delivery; 179 preserved FBXs |
| Target use | Engine-neutral third-person locomotion intake |
| Target engines | Unity 6000.5.8f1 source import/sample probe; other engines not evaluated |
| Target rigs/packs | Shared Protof-Actor character used for named current cross-pack probes; no separate project target character supplied |
| Source manifest | External scrubbed inventory, SHA-256 `c6cc4d541fa2cb8e4f3e14c283d5b925f83957db35bfa079309f250cdaf101ba` |
| Evaluation manifest | `urn:animsmith:skill:animation-pack-evaluation-manifest:1`; current binding unavailable because current output binding was not rendered for this retained report format |
| Acquisition/license provenance | Commercial source was authorized locally; no transaction or redistribution conclusion is made |

### Evidence coverage

| Surface | Offered/delivered | Evaluated | Findings | Not evaluated and why |
|---|---:|---:|---:|---|
| Animation files | 179 FBXs | 179 | baseline completed | mechanical and contract evidence captured |
| Rigs/export variants | Unknown | 0 | 0 | Not evaluated in this run |
| AnimSmith baseline | 179 | 179 | current findings recorded | `inspect`, JSON `measure`, JSON `lint`, and Markdown `lint` completed per file |
| Declared contracts | 177 motion-labelled files | 177 | 58 pass; 119 fail per output format | Mechanical contract results are current; historical taxonomy was not reconstructed; current generic scenarios are separately identified; artistic acceptance remains open |
| Offline visual reports | 0 | 0 | 0 | Not evaluated in this run |
| Engine import/playback | 4 runtimes | 1 | 177 source Humanoid clips; 6 sample/3 mixer executions | No pose assertions, visual acceptance, controller, build or candidate-output test |
| Blend/mask/retarget | Named source pairs | Six Basic/melee mixer schedules | Finite sampled poses/fixed owner | Mask, separate target-rig retarget and visual/contact acceptance remain untested |

### Claim legend

Current consequential claims use `observed-file`, `observed-animsmith`, `inferred`, `documentation-stated`, or `not-evaluated` as defined by the assessment taxonomy.

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

New evaluator-selected generic scenarios: current source bytes establish the exact members, take names, durations, and measured roots below. Names suggest gameplay roles; topology and semantic intent are hypotheses requiring clip review. These are not reconstructed historical manifests or measured collection-output sets. Every listed member uses `Take 001`; full identities are in the external selected-set ledger.

| Runtime set | Type | Members/variants | Grouping evidence | Validation status |
|---|---|---|---|---|
| Walk eight directions | `directional-blend` | `Humanoid@WalkForwardUnarmed2.fbx`, `Humanoid@WalkForwardLeftUnarmed.fbx`, `Humanoid@WalkLeftUnarmed.fbx`, `Humanoid@WalkBackwardsLeftUnarmed.fbx`, `Humanoid@WalkBackwardsUnarmed.fbx`, `Humanoid@WalkBackwardsRightUnarmed.fbx`, `Humanoid@WalkRightUnarmed.fbx`, `Humanoid@WalkForwardRightUnarmed.fbx`; `Take 001` | New evaluator-selected scenario; observed bytes/timing, inferred gameplay roles | Mechanical measurements current; set/engine/visual acceptance open |
| Run eight directions | `directional-blend` | `Humanoid@RunForward2Unarmed.fbx`, `Humanoid@RunForwardLeftUnarmed.fbx`, `Humanoid@RunLeftUnarmed.fbx`, `Humanoid@RunBackwardsLeftUnarmed.fbx`, `Humanoid@RunBackwardsUnarmed.fbx`, `Humanoid@RunBackwardsRightUnarmed.fbx`, `Humanoid@RunRightUnarmed.fbx`, `Humanoid@RunForwardRightUnarmed.fbx`; `Take 001` | New evaluator-selected scenario; observed bytes/timing, inferred gameplay roles | Mechanical measurements current; set/engine/visual acceptance open |
| Crouch eight directions | `directional-blend` | `Humanoid@CrouchForwardUnarmed.fbx`, `Humanoid@CrouchForwardLeftUnarmed.fbx`, `Humanoid@CrouchLeftUnarmed.fbx`, `Humanoid@CrouchBackwardsLeftUnarmed.fbx`, `Humanoid@CrouchBackwardsUnarmed.fbx`, `Humanoid@CrouchBackwardsRightUnarmed.fbx`, `Humanoid@CrouchRightUnarmed.fbx`, `Humanoid@CrouchForwardRightUnarmed.fbx`; `Take 001` | New evaluator-selected scenario; observed bytes/timing, inferred gameplay roles | Mechanical measurements current; set/engine/visual acceptance open |

### Exact runtime members

Current source members and measurements for the selected runtime scenarios. Selection is an evaluator hypothesis; source identity and measured values are retained below.

| Set/profile | Role or coordinate | Exact members | Variant/type | Timing or motion | Runtime contract |
|---|---|---|---|---|---|
| Walk eight directions | Proposed forward (0,1) | `Humanoid@WalkForwardUnarmed2.fbx::Take 001` | set_type=directional-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Walk eight directions | Proposed forward-left (-1,1) | `Humanoid@WalkForwardLeftUnarmed.fbx::Take 001` | set_type=directional-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Walk eight directions | Proposed left (-1,0) | `Humanoid@WalkLeftUnarmed.fbx::Take 001` | set_type=directional-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Walk eight directions | Proposed back-left (-1,-1) | `Humanoid@WalkBackwardsLeftUnarmed.fbx::Take 001` | set_type=directional-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Walk eight directions | Proposed back (0,-1) | `Humanoid@WalkBackwardsUnarmed.fbx::Take 001` | set_type=directional-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Walk eight directions | Proposed back-right (1,-1) | `Humanoid@WalkBackwardsRightUnarmed.fbx::Take 001` | set_type=directional-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Walk eight directions | Proposed right (1,0) | `Humanoid@WalkRightUnarmed.fbx::Take 001` | set_type=directional-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Walk eight directions | Proposed forward-right (1,1) | `Humanoid@WalkForwardRightUnarmed.fbx::Take 001` | set_type=directional-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Run eight directions | Proposed forward (0,1) | `Humanoid@RunForward2Unarmed.fbx::Take 001` | set_type=directional-blend | duration=0.667 s | loop=unknown; movement=controller; contact=not-evaluated |
| Run eight directions | Proposed forward-left (-1,1) | `Humanoid@RunForwardLeftUnarmed.fbx::Take 001` | set_type=directional-blend | duration=0.667 s | loop=unknown; movement=controller; contact=not-evaluated |
| Run eight directions | Proposed left (-1,0) | `Humanoid@RunLeftUnarmed.fbx::Take 001` | set_type=directional-blend | duration=0.667 s | loop=unknown; movement=controller; contact=not-evaluated |
| Run eight directions | Proposed back-left (-1,-1) | `Humanoid@RunBackwardsLeftUnarmed.fbx::Take 001` | set_type=directional-blend | duration=0.667 s | loop=unknown; movement=controller; contact=not-evaluated |
| Run eight directions | Proposed back (0,-1) | `Humanoid@RunBackwardsUnarmed.fbx::Take 001` | set_type=directional-blend | duration=0.667 s | loop=unknown; movement=controller; contact=not-evaluated |
| Run eight directions | Proposed back-right (1,-1) | `Humanoid@RunBackwardsRightUnarmed.fbx::Take 001` | set_type=directional-blend | duration=0.667 s | loop=unknown; movement=controller; contact=not-evaluated |
| Run eight directions | Proposed right (1,0) | `Humanoid@RunRightUnarmed.fbx::Take 001` | set_type=directional-blend | duration=0.667 s | loop=unknown; movement=controller; contact=not-evaluated |
| Run eight directions | Proposed forward-right (1,1) | `Humanoid@RunForwardRightUnarmed.fbx::Take 001` | set_type=directional-blend | duration=0.667 s | loop=unknown; movement=controller; contact=not-evaluated |
| Crouch eight directions | Proposed forward (0,1) | `Humanoid@CrouchForwardUnarmed.fbx::Take 001` | set_type=directional-blend | duration=1.500 s | loop=unknown; movement=controller; contact=not-evaluated |
| Crouch eight directions | Proposed forward-left (-1,1) | `Humanoid@CrouchForwardLeftUnarmed.fbx::Take 001` | set_type=directional-blend | duration=1.500 s | loop=unknown; movement=controller; contact=not-evaluated |
| Crouch eight directions | Proposed left (-1,0) | `Humanoid@CrouchLeftUnarmed.fbx::Take 001` | set_type=directional-blend | duration=1.500 s | loop=unknown; movement=controller; contact=not-evaluated |
| Crouch eight directions | Proposed back-left (-1,-1) | `Humanoid@CrouchBackwardsLeftUnarmed.fbx::Take 001` | set_type=directional-blend | duration=1.500 s | loop=unknown; movement=controller; contact=not-evaluated |
| Crouch eight directions | Proposed back (0,-1) | `Humanoid@CrouchBackwardsUnarmed.fbx::Take 001` | set_type=directional-blend | duration=1.500 s | loop=unknown; movement=controller; contact=not-evaluated |
| Crouch eight directions | Proposed back-right (1,-1) | `Humanoid@CrouchBackwardsRightUnarmed.fbx::Take 001` | set_type=directional-blend | duration=1.500 s | loop=unknown; movement=controller; contact=not-evaluated |
| Crouch eight directions | Proposed right (1,0) | `Humanoid@CrouchRightUnarmed.fbx::Take 001` | set_type=directional-blend | duration=1.500 s | loop=unknown; movement=controller; contact=not-evaluated |
| Crouch eight directions | Proposed forward-right (1,1) | `Humanoid@CrouchForwardRightUnarmed.fbx::Take 001` | set_type=directional-blend | duration=1.500 s | loop=unknown; movement=controller; contact=not-evaluated |

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
| Export | `partially-evaluated` | Candidates were written as external GLBs only; no transformed-output engine handoff was selected |
| Gate/report | `partially-evaluated` | Current baseline, remediation, and boundary recorded |

### Readiness evidence by clip set

| Role or runtime set | File-ready / clip-ready | Set-ready / rig-use | Runtime / acceptance boundary |
|---|---|---|---|
| Entire delivered corpus | evaluated-finding | not-evaluated | not-evaluated |
| Named current generic scenarios above | Exact sources/takes measured; declared lint conditions remain | Candidate topology inferred; no set readiness approval | Target controller, contacts and artistic acceptance not evaluated |

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
| baseline: `constant-track:note` | 179 files; 24186 findings | Policy-dependent; inspect the affected take before assigning artist responsibility | `observed-animsmith`; exact affected files below |
| baseline: `time-monotonic:error` | 12 files; 36 findings | Policy-dependent; inspect the affected take before assigning artist responsibility | `observed-animsmith`; exact affected files below |
| contracts: `constant-track:note` | 177 files; 23922 findings | Policy-dependent; inspect the affected take before assigning artist responsibility | `observed-animsmith`; exact affected files below |
| contracts: `loop-seam-vel:error` | 104 files; 104 findings | Policy-dependent; inspect the affected take before assigning artist responsibility | `observed-animsmith`; exact affected files below |
| contracts: `loop-seam-rot:error` | 108 files; 108 findings | Policy-dependent; inspect the affected take before assigning artist responsibility | `observed-animsmith`; exact affected files below |
| contracts: `loop-closure:error` | 58 files; 84 findings | Policy-dependent; inspect the affected take before assigning artist responsibility | `observed-animsmith`; exact affected files below |
| contracts: `in-place:error` | 14 files; 14 findings | Policy-dependent; inspect the affected take before assigning artist responsibility | `observed-animsmith`; exact affected files below |
| contracts: `time-monotonic:error` | 12 files; 36 findings | Policy-dependent; inspect the affected take before assigning artist responsibility | `observed-animsmith`; exact affected files below |
| contracts: `loop-seam:error` | 8 files; 8 findings | Policy-dependent; inspect the affected take before assigning artist responsibility | `observed-animsmith`; exact affected files below |

Per-file contract results: 58 pass and 119 fail of 177; JSON and Markdown agree. Loop declarations are evaluation hypotheses, not proof that every action should repeat.

**loop-seam-vel:error**: `Humanoid@CrouchBackwardsLeftUnarmed.fbx`, `Humanoid@CrouchBackwardsLeftUnarmed_RM.fbx`, `Humanoid@CrouchBackwardsRightUnarmed.fbx`, `Humanoid@CrouchBackwardsRightUnarmed_RM.fbx`, `Humanoid@CrouchBackwardsUnarmed.fbx`, `Humanoid@CrouchBackwardsUnarmed_RM.fbx`, `Humanoid@CrouchForwardLeftUnarmed.fbx`, `Humanoid@CrouchForwardLeftUnarmed_RM.fbx`, `Humanoid@CrouchForwardRightUnarmed.fbx`, `Humanoid@CrouchForwardRightUnarmed_RM.fbx`, `Humanoid@CrouchForwardUnarmed.fbx`, `Humanoid@CrouchForwardUnarmed_RM.fbx`, `Humanoid@CrouchIdleLookAround1Unarmed.fbx`, `Humanoid@CrouchIdleLookAround2Unarmed.fbx`, `Humanoid@CrouchLeftUnarmed.fbx`, `Humanoid@CrouchLeftUnarmed_RM.fbx`, `Humanoid@CrouchRightUnarmed.fbx`, `Humanoid@CrouchRightUnarmed_RM.fbx`, `Humanoid@CrouchThrowGrenadeUnarmed1.fbx`, `Humanoid@CrouchThrowGrenadeUnarmed2.fbx`, `Humanoid@CrouchTurn180RightUnarmed.fbx`, `Humanoid@CrouchTurn180RightUnarmed_RM.fbx`, `Humanoid@CrouchTurn90LeftUnarmed.fbx`, `Humanoid@CrouchTurn90LeftUnarmed_RM.fbx`, `Humanoid@CrouchTurn90RightUnarmed.fbx`, `Humanoid@CrouchTurn90RightUnarmed_RM.fbx`, `Humanoid@FallingUnarmed.FBX`, `Humanoid@Pass1MeterObstacleLeftUnarmed.FBX`, `Humanoid@Pass1MeterObstacleLeftUnarmed_RM.FBX`, `Humanoid@Pass1MeterObstacleRightUnarmed.FBX`, `Humanoid@Pass1MeterObstacleRightUnarmed_RM.FBX`, `Humanoid@RunBackwardsLeftUnarmed.fbx`, `Humanoid@RunBackwardsLeftUnarmed_RM.fbx`, `Humanoid@RunBackwardsRightUnarmed.fbx`, `Humanoid@RunBackwardsRightUnarmed_RM.fbx`, `Humanoid@RunBackwardsUnarmed.fbx`, `Humanoid@RunBackwardsUnarmed_RM.fbx`, `Humanoid@RunFastForwardUnarmed.fbx`, `Humanoid@RunFastForwardUnarmed_RM.fbx`, `Humanoid@RunFastTurnLeftUnarmed.fbx`, `Humanoid@RunFastTurnLeftUnarmed_RM.fbx`, `Humanoid@RunFastTurnRightUnarmed.fbx`, `Humanoid@RunFastTurnRightUnarmed_RM.fbx`, `Humanoid@RunForward2Unarmed.fbx`, `Humanoid@RunForward2Unarmed_RM.fbx`, `Humanoid@RunForwardLeftUnarmed.fbx`, `Humanoid@RunForwardLeftUnarmed_RM.fbx`, `Humanoid@RunForwardRightUnarmed.fbx`, `Humanoid@RunForwardRightUnarmed_RM.fbx`, `Humanoid@RunForwardUnarmed.FBX`, `Humanoid@RunForwardUnarmed_RM.FBX`, `Humanoid@RunUTurnLeftUnarmed.fbx`, `Humanoid@RunUTurnLeftUnarmed_RM.fbx`, `Humanoid@RunUTurnRightUnarmed.fbx`, `Humanoid@RunUTurnRightUnarmed_RM.fbx`, `Humanoid@SprintForwardLeftUnarmed.fbx`, `Humanoid@SprintForwardLeftUnarmed_RM.fbx`, `Humanoid@SprintForwardRightUnarmed.fbx`, `Humanoid@SprintForwardRightUnarmed_RM.fbx`, `Humanoid@SprintForwardUnarmed.FBX`, `Humanoid@SprintForwardUnarmed_RM.FBX`, `Humanoid@StrafeLeftTakeCoverCrouchingUnarmed.fbx`, `Humanoid@StrafeLeftTakeCoverCrouchingUnarmed_RM.fbx`, `Humanoid@StrafeLeftTakeCoverStandingUnarmed.fbx`, `Humanoid@StrafeLeftTakeCoverStandingUnarmed_RM.fbx`, `Humanoid@StrafeRightTakeCoverCrouchingUnarmed.fbx`, `Humanoid@StrafeRightTakeCoverCrouchingUnarmed_RM.fbx`, `Humanoid@StrafeRightTakeCoverStandingUnarmed.fbx`, `Humanoid@StrafeRightTakeCoverStandingUnarmed_RM.fbx`, `Humanoid@ThrowGrenadeLeftUnderCoverCrouching.fbx`, `Humanoid@ThrowGrenadeLeftUnderCoverStanding.fbx`, `Humanoid@ThrowGrenadeRightUnderCoverCrouching.fbx`, `Humanoid@ThrowGrenadeRightUnderCoverStanding.fbx`, `Humanoid@ThrowGrenadeUnarmed1.fbx`, `Humanoid@ThrowGrenadeUnarmed2.fbx`, `Humanoid@Turn180LeftUnarmed.fbx`, `Humanoid@Turn180LeftUnarmed_RM.fbx`, `Humanoid@Turn180RightUnarmed.fbx`, `Humanoid@Turn180RightUnarmed_RM.fbx`, `Humanoid@Turn90LeftUnarmed.fbx`, `Humanoid@Turn90LeftUnarmed_RM.fbx`, `Humanoid@Turn90RightUnarmed.fbx`, `Humanoid@Turn90RightUnarmed_RM.fbx`, `Humanoid@WalkBackwardsLeftUnarmed.fbx`, `Humanoid@WalkBackwardsLeftUnarmed_RM.fbx`, `Humanoid@WalkBackwardsRightUnarmed.fbx`, `Humanoid@WalkBackwardsRightUnarmed_RM.fbx`, `Humanoid@WalkBackwardsUnarmed.fbx`, `Humanoid@WalkBackwardsUnarmed_RM.fbx`, `Humanoid@WalkForwardLeftUnarmed.fbx`, `Humanoid@WalkForwardLeftUnarmed_RM.fbx`, `Humanoid@WalkForwardRightUnarmed.fbx`, `Humanoid@WalkForwardRightUnarmed_RM.fbx`, `Humanoid@WalkForwardUnarmed.FBX`, `Humanoid@WalkForwardUnarmed2.fbx`, `Humanoid@WalkForwardUnarmed2_RM.fbx`, `Humanoid@WalkForwardUnarmed_RM.FBX`, `Humanoid@WalkLeftUnarmed.fbx`, `Humanoid@WalkLeftUnarmed_RM.fbx`, `Humanoid@WalkRightUnarmed.fbx`, `Humanoid@WalkRightUnarmed_RM.fbx`, `Humanoid@WalkUTurnLeftUnarmed_RM.fbx`, `Humanoid@WalkUTurnRightUnarmed.fbx`, `Humanoid@WalkUTurnRightUnarmed_RM.fbx`.

**loop-seam-rot:error**: `Humanoid@CrouchBackwardsLeftUnarmed.fbx`, `Humanoid@CrouchBackwardsLeftUnarmed_RM.fbx`, `Humanoid@CrouchBackwardsRightUnarmed.fbx`, `Humanoid@CrouchBackwardsRightUnarmed_RM.fbx`, `Humanoid@CrouchBackwardsUnarmed.fbx`, `Humanoid@CrouchBackwardsUnarmed_RM.fbx`, `Humanoid@CrouchForwardLeftUnarmed.fbx`, `Humanoid@CrouchForwardLeftUnarmed_RM.fbx`, `Humanoid@CrouchForwardRightUnarmed.fbx`, `Humanoid@CrouchForwardRightUnarmed_RM.fbx`, `Humanoid@CrouchForwardUnarmed.fbx`, `Humanoid@CrouchForwardUnarmed_RM.fbx`, `Humanoid@CrouchIdleBreathe1Unarmed.fbx`, `Humanoid@CrouchIdleBreathe2Unarmed.fbx`, `Humanoid@CrouchIdleLookAround1Unarmed.fbx`, `Humanoid@CrouchIdleLookAround2Unarmed.fbx`, `Humanoid@CrouchLeftUnarmed.fbx`, `Humanoid@CrouchLeftUnarmed_RM.fbx`, `Humanoid@CrouchRightUnarmed.fbx`, `Humanoid@CrouchRightUnarmed_RM.fbx`, `Humanoid@CrouchThrowGrenadeUnarmed1.fbx`, `Humanoid@CrouchThrowGrenadeUnarmed2.fbx`, `Humanoid@CrouchTurn180RightUnarmed.fbx`, `Humanoid@CrouchTurn180RightUnarmed_RM.fbx`, `Humanoid@CrouchTurn90LeftUnarmed.fbx`, `Humanoid@CrouchTurn90LeftUnarmed_RM.fbx`, `Humanoid@CrouchTurn90RightUnarmed.fbx`, `Humanoid@CrouchTurn90RightUnarmed_RM.fbx`, `Humanoid@FallingUnarmed.FBX`, `Humanoid@IdleLookAroundScratchYawnUnarmed.fbx`, `Humanoid@IdleLookAroundUnarmed.FBX`, `Humanoid@Pass1MeterObstacleLeftUnarmed.FBX`, `Humanoid@Pass1MeterObstacleLeftUnarmed_RM.FBX`, `Humanoid@Pass1MeterObstacleRightUnarmed.FBX`, `Humanoid@Pass1MeterObstacleRightUnarmed_RM.FBX`, `Humanoid@RunBackwardsLeftUnarmed.fbx`, `Humanoid@RunBackwardsLeftUnarmed_RM.fbx`, `Humanoid@RunBackwardsRightUnarmed.fbx`, `Humanoid@RunBackwardsRightUnarmed_RM.fbx`, `Humanoid@RunBackwardsUnarmed.fbx`, `Humanoid@RunBackwardsUnarmed_RM.fbx`, `Humanoid@RunFastForwardUnarmed.fbx`, `Humanoid@RunFastForwardUnarmed_RM.fbx`, `Humanoid@RunFastTurnLeftUnarmed.fbx`, `Humanoid@RunFastTurnLeftUnarmed_RM.fbx`, `Humanoid@RunFastTurnRightUnarmed.fbx`, `Humanoid@RunFastTurnRightUnarmed_RM.fbx`, `Humanoid@RunForward2Unarmed.fbx`, `Humanoid@RunForward2Unarmed_RM.fbx`, `Humanoid@RunForwardLeftUnarmed.fbx`, `Humanoid@RunForwardLeftUnarmed_RM.fbx`, `Humanoid@RunForwardRightUnarmed.fbx`, `Humanoid@RunForwardRightUnarmed_RM.fbx`, `Humanoid@RunForwardUnarmed.FBX`, `Humanoid@RunForwardUnarmed_RM.FBX`, `Humanoid@RunUTurnLeftUnarmed.fbx`, `Humanoid@RunUTurnLeftUnarmed_RM.fbx`, `Humanoid@RunUTurnRightUnarmed.fbx`, `Humanoid@RunUTurnRightUnarmed_RM.fbx`, `Humanoid@SprintForwardLeftUnarmed.fbx`, `Humanoid@SprintForwardLeftUnarmed_RM.fbx`, `Humanoid@SprintForwardRightUnarmed.fbx`, `Humanoid@SprintForwardRightUnarmed_RM.fbx`, `Humanoid@SprintForwardUnarmed.FBX`, `Humanoid@SprintForwardUnarmed_RM.FBX`, `Humanoid@StrafeLeftTakeCoverCrouchingUnarmed.fbx`, `Humanoid@StrafeLeftTakeCoverCrouchingUnarmed_RM.fbx`, `Humanoid@StrafeLeftTakeCoverStandingUnarmed.fbx`, `Humanoid@StrafeLeftTakeCoverStandingUnarmed_RM.fbx`, `Humanoid@StrafeRightTakeCoverCrouchingUnarmed.fbx`, `Humanoid@StrafeRightTakeCoverCrouchingUnarmed_RM.fbx`, `Humanoid@StrafeRightTakeCoverStandingUnarmed.fbx`, `Humanoid@StrafeRightTakeCoverStandingUnarmed_RM.fbx`, `Humanoid@ThrowGrenadeLeftUnderCoverCrouching.fbx`, `Humanoid@ThrowGrenadeLeftUnderCoverStanding.fbx`, `Humanoid@ThrowGrenadeRightUnderCoverCrouching.fbx`, `Humanoid@ThrowGrenadeRightUnderCoverStanding.fbx`, `Humanoid@ThrowGrenadeUnarmed1.fbx`, `Humanoid@ThrowGrenadeUnarmed2.fbx`, `Humanoid@Turn180LeftUnarmed.fbx`, `Humanoid@Turn180LeftUnarmed_RM.fbx`, `Humanoid@Turn180RightUnarmed.fbx`, `Humanoid@Turn180RightUnarmed_RM.fbx`, `Humanoid@Turn90LeftUnarmed.fbx`, `Humanoid@Turn90LeftUnarmed_RM.fbx`, `Humanoid@Turn90RightUnarmed.fbx`, `Humanoid@Turn90RightUnarmed_RM.fbx`, `Humanoid@WalkBackwardsLeftUnarmed.fbx`, `Humanoid@WalkBackwardsLeftUnarmed_RM.fbx`, `Humanoid@WalkBackwardsRightUnarmed.fbx`, `Humanoid@WalkBackwardsRightUnarmed_RM.fbx`, `Humanoid@WalkBackwardsUnarmed.fbx`, `Humanoid@WalkBackwardsUnarmed_RM.fbx`, `Humanoid@WalkForwardLeftUnarmed.fbx`, `Humanoid@WalkForwardLeftUnarmed_RM.fbx`, `Humanoid@WalkForwardRightUnarmed.fbx`, `Humanoid@WalkForwardRightUnarmed_RM.fbx`, `Humanoid@WalkForwardUnarmed.FBX`, `Humanoid@WalkForwardUnarmed2.fbx`, `Humanoid@WalkForwardUnarmed2_RM.fbx`, `Humanoid@WalkForwardUnarmed_RM.FBX`, `Humanoid@WalkLeftUnarmed.fbx`, `Humanoid@WalkLeftUnarmed_RM.fbx`, `Humanoid@WalkRightUnarmed.fbx`, `Humanoid@WalkRightUnarmed_RM.fbx`, `Humanoid@WalkUTurnLeftUnarmed_RM.fbx`, `Humanoid@WalkUTurnRightUnarmed.fbx`, `Humanoid@WalkUTurnRightUnarmed_RM.fbx`.

**loop-closure:error**: `Humanoid@CrouchBackwardsLeftUnarmed_RM.fbx`, `Humanoid@CrouchBackwardsRightUnarmed_RM.fbx`, `Humanoid@CrouchBackwardsUnarmed_RM.fbx`, `Humanoid@CrouchForwardLeftUnarmed_RM.fbx`, `Humanoid@CrouchForwardRightUnarmed_RM.fbx`, `Humanoid@CrouchForwardUnarmed_RM.fbx`, `Humanoid@CrouchIdleBreathe2Unarmed.fbx`, `Humanoid@CrouchIdleLookAround2Unarmed.fbx`, `Humanoid@CrouchLeftUnarmed_RM.fbx`, `Humanoid@CrouchRightUnarmed_RM.fbx`, `Humanoid@CrouchThrowGrenadeUnarmed1.fbx`, `Humanoid@CrouchThrowGrenadeUnarmed2.fbx`, `Humanoid@CrouchTurn180RightUnarmed_RM.fbx`, `Humanoid@CrouchTurn90LeftUnarmed.fbx`, `Humanoid@CrouchTurn90LeftUnarmed_RM.fbx`, `Humanoid@CrouchTurn90RightUnarmed_RM.fbx`, `Humanoid@Pass1MeterObstacleLeftUnarmed_RM.FBX`, `Humanoid@Pass1MeterObstacleRightUnarmed_RM.FBX`, `Humanoid@RunBackwardsLeftUnarmed_RM.fbx`, `Humanoid@RunBackwardsRightUnarmed_RM.fbx`, `Humanoid@RunBackwardsUnarmed_RM.fbx`, `Humanoid@RunFastForwardUnarmed_RM.fbx`, `Humanoid@RunFastTurnLeftUnarmed_RM.fbx`, `Humanoid@RunFastTurnRightUnarmed_RM.fbx`, `Humanoid@RunForward2Unarmed_RM.fbx`, `Humanoid@RunForwardLeftUnarmed_RM.fbx`, `Humanoid@RunForwardRightUnarmed_RM.fbx`, `Humanoid@RunForwardUnarmed_RM.FBX`, `Humanoid@RunUTurnLeftUnarmed.fbx`, `Humanoid@RunUTurnLeftUnarmed_RM.fbx`, `Humanoid@RunUTurnRightUnarmed.fbx`, `Humanoid@RunUTurnRightUnarmed_RM.fbx`, `Humanoid@SprintForwardLeftUnarmed_RM.fbx`, `Humanoid@SprintForwardRightUnarmed_RM.fbx`, `Humanoid@SprintForwardUnarmed_RM.FBX`, `Humanoid@StrafeLeftTakeCoverCrouchingUnarmed_RM.fbx`, `Humanoid@StrafeLeftTakeCoverStandingUnarmed_RM.fbx`, `Humanoid@StrafeRightTakeCoverCrouchingUnarmed_RM.fbx`, `Humanoid@StrafeRightTakeCoverStandingUnarmed_RM.fbx`, `Humanoid@ThrowGrenadeUnarmed2.fbx`, `Humanoid@Turn180LeftUnarmed_RM.fbx`, `Humanoid@Turn180RightUnarmed_RM.fbx`, `Humanoid@Turn90LeftUnarmed_RM.fbx`, `Humanoid@Turn90RightUnarmed_RM.fbx`, `Humanoid@WalkBackwardsLeftUnarmed_RM.fbx`, `Humanoid@WalkBackwardsRightUnarmed_RM.fbx`, `Humanoid@WalkBackwardsUnarmed_RM.fbx`, `Humanoid@WalkForwardLeftUnarmed.fbx`, `Humanoid@WalkForwardLeftUnarmed_RM.fbx`, `Humanoid@WalkForwardRightUnarmed_RM.fbx`, `Humanoid@WalkForwardUnarmed2.fbx`, `Humanoid@WalkForwardUnarmed2_RM.fbx`, `Humanoid@WalkForwardUnarmed_RM.FBX`, `Humanoid@WalkLeftUnarmed_RM.fbx`, `Humanoid@WalkRightUnarmed_RM.fbx`, `Humanoid@WalkUTurnLeftUnarmed_RM.fbx`, `Humanoid@WalkUTurnRightUnarmed.fbx`, `Humanoid@WalkUTurnRightUnarmed_RM.fbx`.

**in-place:error**: `Humanoid@CrouchTurn180LeftUnarmed_RM.fbx`, `Humanoid@CrouchTurn180RightUnarmed_RM.fbx`, `Humanoid@CrouchTurn90LeftUnarmed_RM.fbx`, `Humanoid@CrouchTurn90RightUnarmed_RM.fbx`, `Humanoid@RunUTurnLeftUnarmed_RM.fbx`, `Humanoid@RunUTurnRightUnarmed_RM.fbx`, `Humanoid@StrafeLeftTakeCoverCrouchingUnarmed_RM.fbx`, `Humanoid@StrafeRightTakeCoverCrouchingUnarmed_RM.fbx`, `Humanoid@Turn180LeftUnarmed_RM.fbx`, `Humanoid@Turn180RightUnarmed_RM.fbx`, `Humanoid@Turn90LeftUnarmed_RM.fbx`, `Humanoid@Turn90RightUnarmed_RM.fbx`, `Humanoid@WalkUTurnLeftUnarmed_RM.fbx`, `Humanoid@WalkUTurnRightUnarmed_RM.fbx`.

**time-monotonic:error**: `Humanoid@GoBackToCoverLeftStandingUnarmed.fbx`, `Humanoid@GoBackToCoverLeftStandingUnarmed_RM.fbx`, `Humanoid@GoBackToCoverRightCrouchingUnarmed.fbx`, `Humanoid@GoBackToCoverRightCrouchingUnarmed_RM.fbx`, `Humanoid@GoBackToCoverRightStandingUnarmed.fbx`, `Humanoid@GoBackToCoverRightStandingUnarmed_RM.fbx`, `Humanoid@GoOutOfCoverRightStandingUnarmed_RM.fbx`, `Humanoid@IdleStandingToTakeCoverCrouchingUnarmed.fbx`, `Humanoid@IdleStandingToTakeCoverCrouchingUnarmed_RM.fbx`, `Humanoid@IdleTakeCoverCrouchingToIdleStandingUnarmed.fbx`, `Humanoid@ThrowGrenadeLeftUnderCoverStanding.fbx`, `Humanoid@ThrowGrenadeRightUnderCoverStanding.fbx`.

**loop-seam:error**: `Humanoid@RunFastTurnLeftUnarmed_RM.fbx`, `Humanoid@RunFastTurnRightUnarmed_RM.fbx`, `Humanoid@RunUTurnLeftUnarmed_RM.fbx`, `Humanoid@RunUTurnRightUnarmed_RM.fbx`, `Humanoid@Turn180RightUnarmed_RM.fbx`, `Humanoid@WalkUTurnLeftUnarmed_RM.fbx`, `Humanoid@WalkUTurnRightUnarmed.fbx`, `Humanoid@WalkUTurnRightUnarmed_RM.fbx`.

## AnimSmith remediation evidence

Phase spread here is the minimum covering arc in cycles: sort phases in `[0,1)`, include the wraparound gap, then subtract the largest gap from 1. It is not `max_circular_deviation_from_mean`; neither measure alone proves support-foot or visual compatibility. The primary report compares source and unpromoted output spreads for the same walk/run/crouch member sets.

| Source issue | Operation/declarations | Result | Independent verification | Remaining caveat |
|---|---|---|---|---|
| `gait-anchor` trial | Explicit original source/config; `transform --gait-anchor` | 24 candidates emitted; 2 pass selected output lint | All 36 original time-ordering errors are absent after the twelve slices. Every candidate inspected, measured, linted, diffed and checked with `fix --dry-run`; detailed exit records external | Unpromoted; no candidate engine/contact/visual acceptance |
| `prune-constant-tracks` trial | Explicit original source/config; `transform --prune-constant-tracks` | 3 candidates emitted; 3 pass selected output lint | Every candidate inspected, measured, linted, diffed and checked with `fix --dry-run`; detailed exit records external | Unpromoted; no candidate engine/contact/visual acceptance |
| `slice-negative-time` trial | Explicit original source/config; `transform --slice` | 12 candidates emitted; 10 pass selected output lint | Every candidate inspected, measured, linted, diffed and checked with `fix --dry-run`; detailed exit records external | Unpromoted; no candidate engine/contact/visual acceptance |

## Engine procedures and evidence

| Runtime | Version | Procedure | Observed result | Remaining gate |
|---|---|---|---|---|
| Unity | 6000.5.8f1 | Fresh project from source FBX and retained importer settings; batch AnimationPackProbe.Run | 178 motion FBXs, 177 Humanoid clips, 178 valid source-avatar assignments; 6/6 samples and 3/3 pair mixers execute | No pose assertions; target controller, visual, contacts, build and transformed outputs untested |
| Unreal Engine | unspecified | None in current run | not evaluated | Disposable import, retarget, graph, and visual test |
| Godot | unspecified | None in current run | not evaluated | Disposable conversion/import and graph test |
| Bevy | unspecified | None in current run | not evaluated | Selected export handoff, runtime, and performance test |

The source sample probe selects forward walk/run/crouch and their RM-labelled counterparts; mixers blend forward with forward-left for walk/run/crouch at 0.5/0.5 for 0.1 s. It reports exceptions, not bone-pose or contact assertions. The probe uses `RunForwardUnarmed.FBX`, while the new selected run ring uses `RunForward2Unarmed.fbx`; it does not validate that entire ring. All 179 copied FBX hashes match the current source baseline. Each motion importer references the shared source avatar; zero embedded valid model avatars is not a failure of that explicit source-avatar setup.

## Rig, masking, and compatibility evidence

Fresh Unity 6000.5.8f1 source tests passed 26/26 bounded checks: 13 individual Humanoid samples, nine named cross-pack mixer schedules and four RM samples. For the nine mixers, five weights (0, 0.25, 0.5, 0.75, 1) produced finite sampled bone transforms with `applyRootMotion=false` and a fixed Animator owner. This supports technical feasibility of the named source combinations, not visually smooth handoffs or a finished kinematic controller.

Basic-specific pairs use `IdleUnarmed` or `WalkForwardUnarmed2` against the three named melee idle/walk clips; the explicit RM source is `WalkForwardUnarmed2_RM`. The [collection appendix](protofactor-ultimate-animation-collection-evidence.md#rig-masking-and-compatibility-evidence) preserves exact pairs and evidence digests.

No rendered-frame, contact, weapon-grip, deformation, interruption, authored AnimatorController or player-build acceptance ran. All tests use original FBXs on the shared Protof-Actor Humanoid avatar; none accepts the AnimSmith-generated GLB candidates.

## Limitations and unknowns

1. The Basic inventory/sample probe tests source import and graph execution only; the separate cross-pack probe adds finite-pose/root assertions. No visual, contact, target-controller, build, transformed-output or artistic acceptance test ran.
2. Commercial sources and outputs remain external and are not published.

## Changes between AnimSmith versions

AnimSmith 0.14.0 — reran 179 input baselines, 177 per-file declared contracts and 39 remediation trials using the official release. Added current exact-member generic scenarios and developer/artist actions. AnimSmith 0.10.0 — superseded historical evidence; none of its generated outputs or engine results is relabelled as a fresh run.

## Reproduction

Official evaluator preflight: [release archive](https://github.com/mmannerm/animsmith/releases/download/v0.14.0/animsmith-v0.14.0-x86_64-unknown-linux-gnu.tar.gz), archive SHA-256 `4ecf79436f9123c779edb004050da3010b44a2b392e3031facb227d7734fc33e`; exact member `animsmith-v0.14.0-x86_64-unknown-linux-gnu/animsmith`, binary SHA-256 `c2b9649bc74f8a7e5b5d7361a6feaa93941b2327cd7eee82235024bd4f61e366`; tag `v0.14.0`, peeled commit `e8321ad40be5ef6f162b31f085819c039175c3c9`; working-tree state: N/A (official archive). Compiled features: `fbx, report`. Version and required command help plus representative FBX admission passed before evaluation. Preflight: `external:animsmith-0.14.0-report-refresh/preflight.json`, SHA-256 `7034b5043de54e7b30064230db18605d1f96d0ea1477f7b8c56ce817cc73521a`.

Evaluator: [official 0.14.0 Linux release](https://github.com/mmannerm/animsmith/releases/tag/v0.14.0), tag commit `e8321ad40be5ef6f162b31f085819c039175c3c9`; archive SHA-256 `4ecf79436f9123c779edb004050da3010b44a2b392e3031facb227d7734fc33e`; binary SHA-256 `c2b9649bc74f8a7e5b5d7361a6feaa93941b2327cd7eee82235024bd4f61e366`, features `fbx, report`. Output schema v19; measurements schema v18. Version/help and representative admission passed before corpus work. Sources/configurations were hashed before and after read-only runs.

External evidence prefix: `external:animsmith-0.14.0-report-refresh/evaluations/protofactor-basic-locomotion/`. Commands record exact source/config arguments, exits and raw-output paths. Commercial source and generated motion remain external. Reproduction requires the authorized delivery and captured configuration; generic one-liners below do not substitute for those declarations.

- `baseline/command-results.json`, SHA-256 `693b4e0226c9d3b8a62727a210bde82d58fe65f80aa020b44f012554bb6c19b3`.
- `contracts/command-results.json`, SHA-256 `151f4cce9337110156c8886eefbc735034f9360d5adf5682e8f147a1029b5980`.
- `remediation/command-results.json`, SHA-256 `fdf00e9e3ff0d5ce9c40f4fc3a19f62bca879a182084db3491b4c44a3d704bb2`.
- `selected-runtime-sets.json`, SHA-256 `eeb198563d692a400a4acd07bba02edbae85d3e52252db5bffde6d31fd83ebc1`.
- `current-summary.json`, SHA-256 `9fb888b1ee64f82477a9f79a4143cdb67d11487c24ae3454140d8e99fb944917`.

```sh
animsmith inspect --config <captured-config> <authorized-source>
animsmith measure --config <captured-config> --format json <authorized-source>
animsmith lint --config <captured-config> --format json <authorized-source>
```

The exact per-trial transform, slice range, configuration and post-check commands are in the remediation ledger. Current scenario sets are new evaluator choices; historical structured-model migration remains outside this refresh.

Fresh source-engine evidence: `external:animsmith-0.14.0-report-refresh/evaluations/protofactor-basic-locomotion/unity/verification.json`, SHA-256 `78108d62fd10818ac26dd31475ca91b4d7e301805e2f063ede608b0e587c1315`. The adjacent probe, result and editor log are retained externally.

## Sources

- Protofactor, [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/) — product context.
- AnimSmith, [CLI reference](../cli.md) and [game-ready clips](../game-ready-clips.md) — current evaluator and readiness boundary.
