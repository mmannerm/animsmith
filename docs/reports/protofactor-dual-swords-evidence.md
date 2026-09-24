# Animation pack evidence appendix: Protofactor Dual Swords Animset

> Companion report: [technical evaluation](protofactor-dual-swords.md)
>
> Evidence status: **partial** — current mechanical, declaration, runtime-group, and remediation evidence completed; engine and human acceptance did not.
>
> Evaluation date: **2026-09-16**
>
> Current evaluator: **AnimSmith 0.14.0**
>
> Report format: **3**

The [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder) remains authoritative.

## Evaluation scope and provenance

| Field | Value |
|---|---|
| Pack/edition | Authorized local Protofactor constituent; revision unknown |
| Vendor/source | Protofactor product context; local revision not established by the current listing |
| Delivered scope | 189 retained FBX candidates; source bytes unchanged during replay |
| Target use | Broad game-engine technical intake; no target controller supplied |
| Target engines | Unity 6000.5.8f1 bounded source/controller probe; Unreal, Godot, and Bevy not evaluated |
| Target rigs/packs | Delivered rigs; Basic Locomotion compatibility not freshly evaluated |
| Source manifest | `external:protofactor-dual-swords/source-inventory.json`; SHA-256 `6686db08c5d264823473332b0c35aa55e068e1d01595797bfc8710da6ea81a7e` |
| Evaluation manifest | Retained `urn:animsmith:skill:animation-pack-evaluation-manifest:1`; current memberships treated as hypotheses and remeasured |
| Acquisition/license provenance | Authorized local commercial input; no legal conclusion |

### Evidence coverage

| Surface | Offered/delivered | Evaluated | Findings | Not evaluated and why |
|---|---:|---:|---:|---|
| Animation files | 189 | 189 | 188 animation-bearing | Engine semantics not implied |
| Rigs/export variants | 189 | 189 | 186 individual-motion files share the 56-bone `2b6fe49d5ae6` skeleton | Retarget/reference-pose behavior not run |
| AnimSmith baseline | 189 | 189 | All inspect/measure/two lint commands exit 0; No baseline warnings or errors. | N/A |
| Declared contracts | 186 | 186 | 24 pass, 162 fail in each format | Declaration intent needs project/vendor authority |
| Offline visual reports | 0 | 0 | 0 | Not run |
| Engine import/playback | 3 named Unity sources | 3 | Fresh Humanoid import and finite standalone sampling passed | Visual/contact, authored-controller, and player-build behavior not evaluated |
| Blend/mask/retarget | Three named source Playables mixers | Three fixed-root schedules | Finite sampled poses for named cross-pack pairs | No full blend tree, mask, separate target-rig retarget, visual or contact acceptance |

### Claim legend

`observed-file` identifies source structure, `observed-animsmith` identifies current tool output, `inferred` identifies bounded hypotheses, and `not-evaluated` marks open gates.

## Evaluation manifest and taxonomy

Current evaluation manifest schema: `urn:animsmith:skill:animation-pack-evaluation-manifest:1`.

### Canonical clip-role inventory

| Canonical primary role | Logical motions | Delivered files | Evidence boundary |
|---|---:|---:|---|
| `idle-pose` | 6 | 6 | `observed-file`; evaluator classification, not vendor semantics |
| `continuous-locomotion` | 70 | 70 | `observed-file`; evaluator classification, not vendor semantics |
| `locomotion-transition` | 0 | 0 | `observed-file`; evaluator classification, not vendor semantics |
| `airborne` | 0 | 0 | `observed-file`; evaluator classification, not vendor semantics |
| `traversal` | 0 | 0 | `observed-file`; evaluator classification, not vendor semantics |
| `action-interaction` | 86 | 86 | `observed-file`; evaluator classification, not vendor semantics |
| `reaction-death` | 24 | 24 | `observed-file`; evaluator classification, not vendor semantics |
| `emote-cinematic` | 0 | 0 | `observed-file`; evaluator classification, not vendor semantics |
| `other-unknown` | 0 | 0 | `observed-file`; evaluator classification, not vendor semantics |
| **Total** | **186** | **186** | Current exact-file catalog |

### Runtime-set inventory

The untested acceptance below applies to each complete set. [Named Unity source
samples and mixers](#engine-procedures-and-evidence) ran for a smaller subset;
they do not establish full-set playback or visual quality.

| Runtime set | Type | Members/variants | Grouping evidence | Validation status |
|---|---|---|---|---|
| `crouch-combat-8-way` | directional-blend | 16 exact files; 8 IP, 8 RM | Current exact-file measurements; grouping is an evaluator-defined hypothesis seeded by retained catalog declarations | 0/16 files pass the retained declaration; whole-set engine and visual acceptance not evaluated |
| `run-combat-8-way` | directional-blend | 16 exact files; 8 IP, 8 RM | Current exact-file measurements; grouping is an evaluator-defined hypothesis seeded by retained catalog declarations | 0/16 files pass the retained declaration; whole-set engine and visual acceptance not evaluated |
| `walk-combat-8-way` | directional-blend | 16 exact files; 8 IP, 8 RM | Current exact-file measurements; grouping is an evaluator-defined hypothesis seeded by retained catalog declarations | 0/16 files pass the retained declaration; whole-set engine and visual acceptance not evaluated |
| `forward-speed-alternatives` | speed-blend | 10 exact files; 5 IP, 5 RM | Current exact-file measurements; grouping is an evaluator-defined hypothesis seeded by retained catalog declarations | 0/10 files pass the retained declaration; whole-set engine and visual acceptance not evaluated |
| `draw-combat-put-away` | transition-chain | 3 exact files; 3 IP, 0 RM | Current exact-file measurements; grouping is an evaluator-defined hypothesis seeded by retained catalog declarations | 2/3 files pass the retained declaration; whole-set engine and visual acceptance not evaluated |
| `combo-alternatives` | other | 38 exact files; 19 IP, 19 RM | Current exact-file measurements; grouping is an evaluator-defined hypothesis seeded by retained catalog declarations | 1/38 files pass the retained declaration; whole-set engine and visual acceptance not evaluated |
| `single-attack-alternatives` | other | 22 exact files; 11 IP, 11 RM | Current exact-file measurements; grouping is an evaluator-defined hypothesis seeded by retained catalog declarations | 0/22 files pass the retained declaration; whole-set engine and visual acceptance not evaluated |
| `crouch-combat-8-way-in-place` | directional-blend | 8 exact IP files | Selectable variant of the measured `crouch-combat-8-way` source family | Retained gait contracts non-clean; contact/engine acceptance open |
| `crouch-combat-8-way-root-motion` | directional-blend | 8 exact RM files | Selectable variant of the measured `crouch-combat-8-way` source family | Retained gait contracts non-clean; contact/engine acceptance open |
| `run-combat-8-way-in-place` | directional-blend | 8 exact IP files | Selectable variant of the measured `run-combat-8-way` source family | Retained gait contracts non-clean; contact/engine acceptance open |
| `run-combat-8-way-root-motion` | directional-blend | 8 exact RM files | Selectable variant of the measured `run-combat-8-way` source family | Retained gait contracts non-clean; contact/engine acceptance open |
| `walk-combat-8-way-in-place` | directional-blend | 8 exact IP files | Selectable variant of the measured `walk-combat-8-way` source family | Retained gait contracts non-clean; contact/engine acceptance open |
| `walk-combat-8-way-root-motion` | directional-blend | 8 exact RM files | Selectable variant of the measured `walk-combat-8-way` source family | Retained gait contracts non-clean; contact/engine acceptance open |

The [exact runtime members](#exact-runtime-members) below record source members and fresh timing/motion for the decision-driving gait sets. No unavailable membership authority is inferred from prose or filenames.

### Grouped gait measurements (retained)

Current source members and measurements for the selected runtime scenarios. Selection is an evaluator hypothesis; source identity and measured values are retained below.

For these multi-member gait rows, duration and RM speed cells are set minima, not per-clip values. The ranges below retain authored variation; individual speed assignments are not reproduced here.

The original grouped gait rows below mix eight in-place and eight root-motion files. The selectable eight-member variants follow this retained source table.

| Set/profile | Role or coordinate | Exact members | Variant/type | Timing or motion | Runtime contract |
|---|---|---|---|---|---|
| `crouch-combat-8-way` | eight-way directional gait | `Humanoid@CrouchForwardDualSwords.fbx`; `Humanoid@CrouchForwardDualSwords_RM.fbx`; `Humanoid@CrouchForwardLeftDualSwords.fbx`; `Humanoid@CrouchForwardLeftDualSwords_RM.fbx`; `Humanoid@CrouchLeftDualSwords.fbx`; `Humanoid@CrouchLeftDualSwords_RM.fbx`; `Humanoid@CrouchBackwardsLeftDualSwords.fbx`; `Humanoid@CrouchBackwardsLeftDualSwords_RM.fbx`; `Humanoid@CrouchBackwardsDualSwords.fbx`; `Humanoid@CrouchBackwardsDualSwords_RM.fbx`; `Humanoid@CrouchBackwardsRightDualSwords.fbx`; `Humanoid@CrouchBackwardsRightDualSwords_RM.fbx`; `Humanoid@CrouchRightDualSwords.fbx`; `Humanoid@CrouchRightDualSwords_RM.fbx`; `Humanoid@CrouchForwardRightDualSwords.fbx`; `Humanoid@CrouchForwardRightDualSwords_RM.fbx` | set_type=directional-blend | duration=1.667 s; rm_speed=0.730 m/s | loop_ip=true; loop_rm=true; sync=not-evaluated |
| `run-combat-8-way` | eight-way directional gait | `Humanoid@RunForwardDualSwords.fbx`; `Humanoid@RunForwardDualSwords_RM.fbx`; `Humanoid@RunForwardLeftDualSwords.fbx`; `Humanoid@RunForwardLeftDualSwords_RM.fbx`; `Humanoid@RunLeftDualSwords.fbx`; `Humanoid@RunLeftDualSwords_RM.fbx`; `Humanoid@RunBackwardsLeftDualSwords.fbx`; `Humanoid@RunBackwardsLeftDualSwords_RM.fbx`; `Humanoid@RunBackwardsDualSwords.fbx`; `Humanoid@RunBackwardsDualSwords_RM.fbx`; `Humanoid@RunBackwardsRightDualSwords.fbx`; `Humanoid@RunBackwardsRightDualSwords_RM.fbx`; `Humanoid@RunRightDualSwords.fbx`; `Humanoid@RunRightDualSwords_RM.fbx`; `Humanoid@RunForwardRightDualSwords.fbx`; `Humanoid@RunForwardRightDualSwords_RM.fbx` | set_type=directional-blend | duration=0.600 s; rm_speed=2.475 m/s | loop_ip=true; loop_rm=true; sync=not-evaluated |
| `walk-combat-8-way` | eight-way directional gait | `Humanoid@WalkForwardDualSwords.fbx`; `Humanoid@WalkForwardDualSwords_RM.fbx`; `Humanoid@WalkForwardLeftDualSwords.fbx`; `Humanoid@WalkForwardLeftDualSwords_RM.fbx`; `Humanoid@WalkLeftDualSwords.fbx`; `Humanoid@WalkLeftDualSwords_RM.fbx`; `Humanoid@WalkBackwardsLeftDualSwords.fbx`; `Humanoid@WalkBackwardsLeftDualSwords_RM.fbx`; `Humanoid@WalkBackwardsDualSwords.fbx`; `Humanoid@WalkBackwardsDualSwords_RM.fbx`; `Humanoid@WalkBackwardsRightDualSwords.fbx`; `Humanoid@WalkBackwardsRightDualSwords_RM.fbx`; `Humanoid@WalkRightDualSwords.fbx`; `Humanoid@WalkRightDualSwords_RM.fbx`; `Humanoid@WalkForwardRightDualSwords.fbx`; `Humanoid@WalkForwardRightDualSwords_RM.fbx` | set_type=directional-blend | duration=1.333 s; rm_speed=0.692 m/s | loop_ip=true; loop_rm=true; sync=not-evaluated |

Measured RM speed spans are `crouch-combat-8-way` 0.730–0.732 m/s (ratio 0.997); `run-combat-8-way` 2.475–2.500 m/s (ratio 0.990); `walk-combat-8-way` 0.692–0.765 m/s (ratio 0.905). Preserve authored variation unless the project declares a normalization policy; diagonals and cardinals require the full blend test. Measured source in-place phase spreads (cycles, eight measured members each) are `crouch-combat-8-way` 0.6184; `run-combat-8-way` 0.6732; `walk-combat-8-way` 0.7086. Phase spread here is the minimum covering arc in cycles: sort phases in `[0,1)`, include the wraparound gap, then subtract the largest gap from 1. It is not `max_circular_deviation_from_mean`; neither measure alone proves support-foot or visual compatibility. Keep `sync=not-evaluated` until the selected synchronization and contact policy is tested. The current catalog also measures `forward-speed-alternatives`, `draw-combat-put-away`, `combo-alternatives`, and `single-attack-alternatives` as evaluator-defined hypotheses; they remain candidate groupings.

### Exact runtime members

Each row is one selectable eight-member variant. Duration and RM speed ranges are preserved in the grouped measurement paragraph above; numeric table values are the measured minima. Continuous-loop declarations remain unaccepted.

| Set/profile | Role or coordinate | Exact members | Variant/type | Timing or motion | Runtime contract |
|---|---|---|---|---|---|
| `crouch-combat-8-way-in-place` | eight-way directional gait | `Humanoid@CrouchForwardDualSwords.fbx`; `Humanoid@CrouchForwardLeftDualSwords.fbx`; `Humanoid@CrouchLeftDualSwords.fbx`; `Humanoid@CrouchBackwardsLeftDualSwords.fbx`; `Humanoid@CrouchBackwardsDualSwords.fbx`; `Humanoid@CrouchBackwardsRightDualSwords.fbx`; `Humanoid@CrouchRightDualSwords.fbx`; `Humanoid@CrouchForwardRightDualSwords.fbx` | variant=in-place | duration=1.667 s | loop=true; sync=not-evaluated |
| `crouch-combat-8-way-root-motion` | eight-way directional gait | `Humanoid@CrouchForwardDualSwords_RM.fbx`; `Humanoid@CrouchForwardLeftDualSwords_RM.fbx`; `Humanoid@CrouchLeftDualSwords_RM.fbx`; `Humanoid@CrouchBackwardsLeftDualSwords_RM.fbx`; `Humanoid@CrouchBackwardsDualSwords_RM.fbx`; `Humanoid@CrouchBackwardsRightDualSwords_RM.fbx`; `Humanoid@CrouchRightDualSwords_RM.fbx`; `Humanoid@CrouchForwardRightDualSwords_RM.fbx` | variant=root-motion | duration=1.667 s; rm_speed=0.730 m/s | loop=true; sync=not-evaluated |
| `run-combat-8-way-in-place` | eight-way directional gait | `Humanoid@RunForwardDualSwords.fbx`; `Humanoid@RunForwardLeftDualSwords.fbx`; `Humanoid@RunLeftDualSwords.fbx`; `Humanoid@RunBackwardsLeftDualSwords.fbx`; `Humanoid@RunBackwardsDualSwords.fbx`; `Humanoid@RunBackwardsRightDualSwords.fbx`; `Humanoid@RunRightDualSwords.fbx`; `Humanoid@RunForwardRightDualSwords.fbx` | variant=in-place | duration=0.600 s | loop=true; sync=not-evaluated |
| `run-combat-8-way-root-motion` | eight-way directional gait | `Humanoid@RunForwardDualSwords_RM.fbx`; `Humanoid@RunForwardLeftDualSwords_RM.fbx`; `Humanoid@RunLeftDualSwords_RM.fbx`; `Humanoid@RunBackwardsLeftDualSwords_RM.fbx`; `Humanoid@RunBackwardsDualSwords_RM.fbx`; `Humanoid@RunBackwardsRightDualSwords_RM.fbx`; `Humanoid@RunRightDualSwords_RM.fbx`; `Humanoid@RunForwardRightDualSwords_RM.fbx` | variant=root-motion | duration=0.600 s; rm_speed=2.475 m/s | loop=true; sync=not-evaluated |
| `walk-combat-8-way-in-place` | eight-way directional gait | `Humanoid@WalkForwardDualSwords.fbx`; `Humanoid@WalkForwardLeftDualSwords.fbx`; `Humanoid@WalkLeftDualSwords.fbx`; `Humanoid@WalkBackwardsLeftDualSwords.fbx`; `Humanoid@WalkBackwardsDualSwords.fbx`; `Humanoid@WalkBackwardsRightDualSwords.fbx`; `Humanoid@WalkRightDualSwords.fbx`; `Humanoid@WalkForwardRightDualSwords.fbx` | variant=in-place | duration=1.333 s | loop=true; sync=not-evaluated |
| `walk-combat-8-way-root-motion` | eight-way directional gait | `Humanoid@WalkForwardDualSwords_RM.fbx`; `Humanoid@WalkForwardLeftDualSwords_RM.fbx`; `Humanoid@WalkLeftDualSwords_RM.fbx`; `Humanoid@WalkBackwardsLeftDualSwords_RM.fbx`; `Humanoid@WalkBackwardsDualSwords_RM.fbx`; `Humanoid@WalkBackwardsRightDualSwords_RM.fbx`; `Humanoid@WalkRightDualSwords_RM.fbx`; `Humanoid@WalkForwardRightDualSwords_RM.fbx` | variant=root-motion | duration=1.333 s; rm_speed=0.692 m/s | loop=true; sync=not-evaluated |

### Pipeline-stage coverage

| Stage | Coverage state | Evidence / remaining gate |
|---|---|---|
| Acquire | `evaluated-clean` | Authorized retained source inventory |
| Preserve raw | `evaluated-clean` | Replay verified source and config hashes unchanged |
| Inspect | `evaluated-finding` | Exhaustive current inspect/measure/lint; notes and named warnings retained |
| Segment | `partially-evaluated` | Exact individual files measured; combined take not semantically segmented |
| Root motion | `evaluated-finding` | IP/RM measurements captured; controller ownership not evaluated |
| Conform | `evaluated-finding` | Fresh candidates produced but not adopted |
| Validate | `evaluated-finding` | Baseline and retained declarations complete; many declarations fail |
| Optimize | `not-evaluated` | Prune candidates are not production-adopted |
| Export | `partially-evaluated` | Candidate GLBs exist externally; no engine import acceptance |
| Gate/report | `partially-evaluated` | Mechanical report complete; engine and human gates open |

### Readiness evidence by clip set

| Role or runtime set | File-ready / clip-ready | Set-ready / rig-use | Runtime / acceptance boundary |
|---|---|---|---|
| Individual motions | Current evaluator reads and measures exact files | Declaration-clean subset only | Named Unity source imports/samples ran; visual behavior not evaluated |
| Directional gait hypotheses | Timing, speed, skeleton, and current errors recorded | 0 contract-clean members in each named gait set | Named source samples and mixers only; full blend, phase, controller, and contact gates open |
| Transition/action hypotheses | Exact files measured | One-shot/loop intent requires authority | Named source Playables schedules ran; authored state graph, contact, and artistic gates open |

### Validation-profile status

| Validation profile | Selection | Result / next evidence |
|---|---|---|
| Marketplace intake | `selected` — `vendor-intended` | Current source inventory and exhaustive mechanical intake complete. |
| Blended locomotion | `selected` — `evaluator-selected-generic-scenario` | Named gait hypotheses measured; selected source mixer schedules ran in Unity, while contracts and full blend review remain open. |
| Root-motion controller | `selected` — `observed-pack-capability` | RM variants measured; named Unity source root samples ran where applicable, while controller/collision ownership remains untested. |
| State-machine transitions | `selected` — `observed-pack-capability` | Transition candidates identified; named source Playables schedules ran, but authored state transitions and interruptions were not tested. |
| Layered upper body/weapons | `not-selected` | No mask/socket/IK authority or test. |
| Traversal/environment | `not-selected` | No target scene or traversal contacts. |
| Contact actions/interactions | `selected` — `observed-pack-capability` | Combat files present; weapon/body contacts not evaluated. |
| Retargeted/customizable characters | `not-selected` | No target rig or reference-pose test. |
| Motion matching/search | `not-selected` | No database construction or query test. |
| Networked movement | `not-selected` | No networking or reconciliation design. |
| Runtime performance | `not-selected` | No target build or benchmark. |

## Pack inventory and content evidence

The current inventory contains 189 FBXs: 188 animation-bearing files, 186 individual-motion files, and 1 combined take. 186 individual-motion files share the 56-bone `2b6fe49d5ae6` skeleton. Counts describe physical current inputs, not runtime acceptance.

### Current in-place gait phase evidence

Phase spread here is the minimum covering arc in cycles: sort phases in `[0,1)`, include the wraparound gap, then subtract the largest gap from 1. It is not `max_circular_deviation_from_mean`; neither measure alone proves support-foot or visual compatibility. These are `observed-animsmith` source measurements for the exact hypotheses above; synchronization/contact acceptance remains open.

| Current hypothesis | Exact source member (`Take 001`) | Measured phase (cycles) |
|---|---|---|
| `crouch-combat-8-way` | `Humanoid@CrouchForwardDualSwords.fbx` | 0.266199 |
| `crouch-combat-8-way` | `Humanoid@CrouchForwardLeftDualSwords.fbx` | 0.244612 |
| `crouch-combat-8-way` | `Humanoid@CrouchLeftDualSwords.fbx` | 0.325734 |
| `crouch-combat-8-way` | `Humanoid@CrouchBackwardsLeftDualSwords.fbx` | 0.783295 |
| `crouch-combat-8-way` | `Humanoid@CrouchBackwardsDualSwords.fbx` | 0.820047 |
| `crouch-combat-8-way` | `Humanoid@CrouchBackwardsRightDualSwords.fbx` | 0.816853 |
| `crouch-combat-8-way` | `Humanoid@CrouchRightDualSwords.fbx` | 0.707328 |
| `crouch-combat-8-way` | `Humanoid@CrouchForwardRightDualSwords.fbx` | 0.186673 |
| `run-combat-8-way` | `Humanoid@RunForwardDualSwords.fbx` | 0.194700 |
| `run-combat-8-way` | `Humanoid@RunForwardLeftDualSwords.fbx` | 0.265383 |
| `run-combat-8-way` | `Humanoid@RunLeftDualSwords.fbx` | 0.298649 |
| `run-combat-8-way` | `Humanoid@RunBackwardsLeftDualSwords.fbx` | 0.803162 |
| `run-combat-8-way` | `Humanoid@RunBackwardsDualSwords.fbx` | 0.831072 |
| `run-combat-8-way` | `Humanoid@RunBackwardsRightDualSwords.fbx` | 0.723860 |
| `run-combat-8-way` | `Humanoid@RunRightDualSwords.fbx` | 0.625469 |
| `run-combat-8-way` | `Humanoid@RunForwardRightDualSwords.fbx` | 0.112560 |
| `walk-combat-8-way` | `Humanoid@WalkForwardDualSwords.fbx` | 0.163464 |
| `walk-combat-8-way` | `Humanoid@WalkForwardLeftDualSwords.fbx` | 0.142984 |
| `walk-combat-8-way` | `Humanoid@WalkLeftDualSwords.fbx` | 0.324181 |
| `walk-combat-8-way` | `Humanoid@WalkBackwardsLeftDualSwords.fbx` | 0.884376 |
| `walk-combat-8-way` | `Humanoid@WalkBackwardsDualSwords.fbx` | 0.886856 |
| `walk-combat-8-way` | `Humanoid@WalkBackwardsRightDualSwords.fbx` | 0.882356 |
| `walk-combat-8-way` | `Humanoid@WalkRightDualSwords.fbx` | 0.615565 |
| `walk-combat-8-way` | `Humanoid@WalkForwardRightDualSwords.fbx` | 0.134164 |

## Mechanical baseline

| Finding/check | Affected scope | Potential impact | Evidence |
|---|---|---|---|
| Current readability | 189/189 inputs | Enables mechanical intake | `observed-animsmith`; all four commands per input exit 0 |
| Constant tracks | 25426 notes across 188 animation-bearing files | Storage/evaluation noise; not a gameplay defect by itself | `observed-animsmith`; baseline ledger SHA-256 `bc7abef8fb3439801eb2cae1c7a212849af278265a13732e1f31b0ccd5feaa5c` |
| Retained declarations | 162/186 non-clean | Blocks admission under those exact declarations | `observed-animsmith`; contract ledger SHA-256 `a57be47f3eb19c0deee6f78ea700c8ecb40f0a983b54c8789e0bbef59e7a7820` |
| Structural/timing warnings | Named scope above | Requires bounded author/project decision | `observed-file` and `observed-animsmith` |

## AnimSmith remediation evidence

| Source issue | Operation/declarations | Result | Independent verification | Remaining caveat |
|---|---|---|---|---|
| `PFD-01` gait continuity | `transform --gait-anchor` on 24 selected in-place gait files; retained per-file configs | 24 outputs produced; selected inputs have no accumulating root translation/yaw | 24 inspect and measure commands exit 0; output hashes and diffs captured | Post-config lint remains non-clean; no RM trajectory was transformed; visual/contact residual unresolved |
| Constant-track notes | `transform --prune-constant-tracks` on `Humanoid@IdleCombatDualSwords.fbx` | Output candidate(s) produced | Inspect, measure, diff, fix dry-run, and config lint captured | 25/25 post-transform config lints exit 1. Candidate adoption and engine/visual acceptance remain open |

Fresh remediation ledger SHA-256 `b201f90c3f30afd4fbc96c25dee8b99608de4bc3d33998a0a29a7b85e1c19b73`. Generated motion stays outside Git.

## Engine procedures and evidence

| Runtime | Version | Procedure | Observed result | Remaining gate |
|---|---|---|---|---|
| Unity | 6000.5.8f1 | Fresh named Humanoid source import and standalone sampling: IdleCombatDualSwords, WalkForwardDualSwords, WalkForwardDualSwords_RM; named Playables mixers: basic-idle-to-dual-idle, sword-shield-idle-to-dual-idle, basic-walk-ip-to-dual-walk-ip. See [collection source/controller probe](protofactor-ultimate-animation-collection-evidence.md#rig-masking-and-compatibility-evidence). | Bounded source/controller feasibility for listed sources only; the collection’s 26/26 is a shared aggregate, not this pack’s result | Rendered motion, contact, deformation, full blend, retargeting, authored AnimatorController, player build and performance remain untested |
| Unreal Engine | not evaluated | No current procedure executed | `not-evaluated` | Import, retarget, root, blend, and visual review |
| Godot | not evaluated | No current procedure executed | `not-evaluated` | Convert/import, graph, root, contact, and visual review |
| Bevy | not evaluated | No current procedure executed | `not-evaluated` | Convert/load, graph, root, performance, and visual review |

## Rig, masking, and compatibility evidence

| Pack/rig/set pair | Skeleton/retarget | Scale/axes | Root policy | Timing/blend | Overall evidence |
|---|---|---|---|---|---|
| Dual Swords internal gait sets | Current signatures measured; no retarget | No engine scale/axis test | IP/RM files measured; owner not selected | Current duration/speed/phase recorded; full blend absent | Mechanical only |
| Dual Swords plus Basic Locomotion | Named Humanoid sources imported on shared `SK_Protof-Actor` | Unity import scale/axis accepted for named sources only | Gameplay-owned root fixed for IP/idle; animation-owned root finite for tested RM where applicable | Named Playables transitions finite; visual blend not inspected | Bounded source/controller feasibility, not compatibility acceptance |

## Limitations and unknowns

1. Retained configs are replay controls, not proof that every attack, reaction, parry, or emote should loop.
2. Current grouping hypotheses were remeasured from exact files, but vendor/project semantics and the unavailable richer historical membership authority were not reconstructed.
3. Generated candidates remain external and unadopted; command success does not lower residual severity.
4. Unity evidence is limited to named source import, finite headless poses, explicit root ownership, and named Playables mixers; no rendered visual, contact, retarget, mask, authored-controller, player-build, performance, or artistic acceptance was performed.

## Changes between AnimSmith versions

AnimSmith 0.14.0 — current official binary revalidated source hashes, all baseline and declaration commands, named runtime hypotheses, and every retained remediation recipe. AnimSmith 0.10.0 — retained ledgers supplied immutable source/config controls; historical results are superseded for current conclusions.

## Reproduction

Official evaluator preflight: [release archive](https://github.com/mmannerm/animsmith/releases/download/v0.14.0/animsmith-v0.14.0-x86_64-unknown-linux-gnu.tar.gz), archive SHA-256 `4ecf79436f9123c779edb004050da3010b44a2b392e3031facb227d7734fc33e`; exact member `animsmith-v0.14.0-x86_64-unknown-linux-gnu/animsmith`, binary SHA-256 `c2b9649bc74f8a7e5b5d7361a6feaa93941b2327cd7eee82235024bd4f61e366`; tag `v0.14.0`, peeled commit `e8321ad40be5ef6f162b31f085819c039175c3c9`; working-tree state: N/A (official archive). Compiled features: `fbx, report`. Version and required command help plus representative FBX admission passed before evaluation. Preflight: `external:animsmith-0.14.0-report-refresh/preflight.json`, SHA-256 `7034b5043de54e7b30064230db18605d1f96d0ea1477f7b8c56ce817cc73521a`.

Official archive `https://github.com/mmannerm/animsmith/releases/download/v0.14.0/animsmith-v0.14.0-x86_64-unknown-linux-gnu.tar.gz`; archive SHA-256 `4ecf79436f9123c779edb004050da3010b44a2b392e3031facb227d7734fc33e`; member `animsmith-v0.14.0-x86_64-unknown-linux-gnu/animsmith`; binary SHA-256 `c2b9649bc74f8a7e5b5d7361a6feaa93941b2327cd7eee82235024bd4f61e366`; tag `v0.14.0`; commit `e8321ad40be5ef6f162b31f085819c039175c3c9`; working tree `N/A`; features `fbx/report`.

Preflight verified version/help for required commands and representative GLB/FBX admission before replay; scrubbed locator `external:0.14.0-report-refresh/preflight.json`, SHA-256 `7034b5043de54e7b30064230db18605d1f96d0ea1477f7b8c56ce817cc73521a`. The read-only helper replayed the retained exact `inspect`, `measure`, and two `lint` invocations with four workers, recorded argv/exit/stdout/stderr and source/config hashes, and did not mutate inputs. Baseline, contract, and remediation ledger digests are `bc7abef8fb3439801eb2cae1c7a212849af278265a13732e1f31b0ccd5feaa5c`, `a57be47f3eb19c0deee6f78ea700c8ecb40f0a983b54c8789e0bbef59e7a7820`, and `b201f90c3f30afd4fbc96c25dee8b99608de4bc3d33998a0a29a7b85e1c19b73`. Licensed paths, sources, and derivative outputs are excluded from Git and this report.

Fresh Unity evidence: editor `6000.5.8f1`, `Unity.exe` SHA-256 `cdc0eeca135394cde79a632aa998aca14b521b58afe8c2750b009aff608be906`; probe SHA-256 `28cd00897dfd739e2a7bcfa193adab03de3591a1009802946305127b67d1bfb6`; result schema `animsmith-unity-crosspack-probe-v1`, result SHA-256 `f067bca91f182b65fab408a1c97b297377a8f361b03d59e809e236aba666652c`; log SHA-256 `0429bcb9cc7a7492c6637eb799566463ba915243d0a110b4d76cbe75590c8819`; 26/26 aggregate required checks passed with 53 nonempty Humanoid bones per sample. This report uses only the named applicable subset above.

## Sources

- Protofactor product context and [End User License Agreement](https://protofactor.biz/end-user-license-agreement/) — context only; local revision remains unknown.
