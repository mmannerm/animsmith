# Animation pack evidence appendix: Protofactor One-Handed Melee Animset

> Companion report: [technical evaluation](protofactor-one-handed-melee.md)
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
| Delivered scope | 113 retained FBX candidates; source bytes unchanged during replay |
| Target use | Broad game-engine technical intake; no target controller supplied |
| Target engines | Unity 6000.5.8f1 bounded source/controller probe; Unreal, Godot, and Bevy not evaluated |
| Target rigs/packs | Delivered rigs; Basic Locomotion compatibility not freshly evaluated |
| Source manifest | `external:protofactor-one-handed-melee/source-inventory.json`; SHA-256 `d70f848f52c11918d90f02f460455cc98b20cacda292cc5b7ae71f733027a5a7` |
| Evaluation manifest | Retained `urn:animsmith:skill:animation-pack-evaluation-manifest:1`; current memberships treated as hypotheses and remeasured |
| Acquisition/license provenance | Authorized local commercial input; no legal conclusion |

### Evidence coverage

| Surface | Offered/delivered | Evaluated | Findings | Not evaluated and why |
|---|---:|---:|---:|---|
| Animation files | 113 | 113 | 112 animation-bearing | Engine semantics not implied |
| Rigs/export variants | 113 | 113 | 109 individual-motion/pack files share the 56-bone `2b6fe49d5ae6` skeleton; `Humanoid@Blocked1hMelee.fbx` is a separate 73-bone variant | Retarget/reference-pose behavior not run |
| AnimSmith baseline | 113 | 113 | All inspect/measure/two lint commands exit 0; No baseline warnings or errors. | N/A |
| Declared contracts | 110 | 110 | 23 pass, 87 fail in each format | Declaration intent needs project/vendor authority |
| Offline visual reports | 0 | 0 | 0 | Not run |
| Engine import/playback | 3 named Unity sources | 3 | Fresh Humanoid import and finite standalone sampling passed | Visual/contact, authored-controller, and player-build behavior not evaluated |
| Blend/mask/retarget | Unknown | 0 | 0 | No target graph, rig, or visual review |

### Claim legend

`observed-file` identifies source structure, `observed-animsmith` identifies current tool output, `inferred` identifies bounded hypotheses, and `not-evaluated` marks open gates.

## Evaluation manifest and taxonomy

Current evaluation manifest schema: `urn:animsmith:skill:animation-pack-evaluation-manifest:1`.

### Canonical clip-role inventory

| Canonical primary role | Logical motions | Delivered files | Evidence boundary |
|---|---:|---:|---|
| `idle-pose` | 6 | 6 | `observed-file`; evaluator classification, not vendor semantics |
| `continuous-locomotion` | 54 | 54 | `observed-file`; evaluator classification, not vendor semantics |
| `locomotion-transition` | 0 | 0 | `observed-file`; evaluator classification, not vendor semantics |
| `airborne` | 5 | 5 | `observed-file`; evaluator classification, not vendor semantics |
| `traversal` | 0 | 0 | `observed-file`; evaluator classification, not vendor semantics |
| `action-interaction` | 20 | 20 | `observed-file`; evaluator classification, not vendor semantics |
| `reaction-death` | 25 | 25 | `observed-file`; evaluator classification, not vendor semantics |
| `emote-cinematic` | 0 | 0 | `observed-file`; evaluator classification, not vendor semantics |
| `other-unknown` | 0 | 0 | `observed-file`; evaluator classification, not vendor semantics |
| **Total** | **110** | **110** | Current exact-file catalog |

### Runtime-set inventory

| Runtime set | Type | Members/variants | Grouping evidence | Validation status |
|---|---|---|---|---|
| `crouch-combat-8-way` | directional-blend | 16 exact files; 8 IP, 8 RM | Current exact-file measurements; grouping is an evaluator-defined hypothesis seeded by retained catalog declarations | 0/16 files pass the retained declaration; engine and visual gates not evaluated |
| `run-8-way` | directional-blend | 16 exact files; 8 IP, 8 RM | Current exact-file measurements; grouping is an evaluator-defined hypothesis seeded by retained catalog declarations | 0/16 files pass the retained declaration; engine and visual gates not evaluated |
| `walk-combat-8-way` | directional-blend | 16 exact files; 8 IP, 8 RM | Current exact-file measurements; grouping is an evaluator-defined hypothesis seeded by retained catalog declarations | 0/16 files pass the retained declaration; engine and visual gates not evaluated |
| `hold-forward-speed` | speed-blend | 6 exact files; 3 IP, 3 RM | Current exact-file measurements; grouping is an evaluator-defined hypothesis seeded by retained catalog declarations | 0/6 files pass the retained declaration; engine and visual gates not evaluated |
| `draw-combat-put-away` | transition-chain | 3 exact files; 3 IP, 0 RM | Current exact-file measurements; grouping is an evaluator-defined hypothesis seeded by retained catalog declarations | 2/3 files pass the retained declaration; engine and visual gates not evaluated |
| `heavy-hit-4-way` | other | 8 exact files; 4 IP, 4 RM | Current exact-file measurements; grouping is an evaluator-defined hypothesis seeded by retained catalog declarations | 0/8 files pass the retained declaration; engine and visual gates not evaluated |
| `crouch-combat-8-way-in-place` | directional-blend | 8 exact IP files | Selectable variant of the measured `crouch-combat-8-way` source family | Retained gait contracts non-clean; contact/engine acceptance open |
| `crouch-combat-8-way-root-motion` | directional-blend | 8 exact RM files | Selectable variant of the measured `crouch-combat-8-way` source family | Retained gait contracts non-clean; contact/engine acceptance open |
| `run-8-way-in-place` | directional-blend | 8 exact IP files | Selectable variant of the measured `run-8-way` source family | Retained gait contracts non-clean; contact/engine acceptance open |
| `run-8-way-root-motion` | directional-blend | 8 exact RM files | Selectable variant of the measured `run-8-way` source family | Retained gait contracts non-clean; contact/engine acceptance open |
| `walk-combat-8-way-in-place` | directional-blend | 8 exact IP files | Selectable variant of the measured `walk-combat-8-way` source family | Retained gait contracts non-clean; contact/engine acceptance open |
| `walk-combat-8-way-root-motion` | directional-blend | 8 exact RM files | Selectable variant of the measured `walk-combat-8-way` source family | Retained gait contracts non-clean; contact/engine acceptance open |

The [exact runtime members](#exact-runtime-members) below record source members and fresh timing/motion for the decision-driving gait sets. No unavailable membership authority is inferred from prose or filenames.

### Grouped gait measurements (retained)

Current source members and measurements for the selected runtime scenarios. Selection is an evaluator hypothesis; source identity and measured values are retained below.

The original grouped gait rows below mix eight in-place and eight root-motion files. The selectable eight-member variants follow this retained source table.

| Set/profile | Role or coordinate | Exact members | Variant/type | Timing or motion | Runtime contract |
|---|---|---|---|---|---|
| `crouch-combat-8-way` | eight-way directional gait | `Humanoid@CrouchForward1hMelee.fbx`; `Humanoid@CrouchForward1hMelee_RM.fbx`; `Humanoid@CrouchForwardLeft1hMelee.fbx`; `Humanoid@CrouchForwardLeft1hMelee_RM.fbx`; `Humanoid@CrouchLeft1hMelee.fbx`; `Humanoid@CrouchLeft1hMelee_RM.fbx`; `Humanoid@CrouchBackwardsLeft1hMelee.fbx`; `Humanoid@CrouchBackwardsLeft1hMelee_RM.fbx`; `Humanoid@CrouchBackwards1hMelee.fbx`; `Humanoid@CrouchBackwards1hMelee_RM.fbx`; `Humanoid@CrouchBackwardsRight1hMelee.fbx`; `Humanoid@CrouchBackwardsRight1hMelee_RM.fbx`; `Humanoid@CrouchRight1hMelee.fbx`; `Humanoid@CrouchRight1hMelee_RM.fbx`; `Humanoid@CrouchForwardRight1hMelee.fbx`; `Humanoid@CrouchForwardRight1hMelee_RM.fbx` | set_type=directional-blend | duration=1.667 s; rm_speed=0.480 m/s | loop_ip=true; loop_rm=true; sync=not-evaluated |
| `run-8-way` | eight-way directional gait | `Humanoid@RunForward1hMelee.fbx`; `Humanoid@RunForward1hMelee_RM.fbx`; `Humanoid@RunForwardLeft1hMelee.fbx`; `Humanoid@RunForwardLeft1hMelee_RM.fbx`; `Humanoid@RunLeft1hMelee.fbx`; `Humanoid@RunLeft1hMelee_RM.fbx`; `Humanoid@RunBackwardsLeft1hMelee.fbx`; `Humanoid@RunBackwardsLeft1hMelee_RM.fbx`; `Humanoid@RunBackwards1hMelee.fbx`; `Humanoid@RunBackwards1hMelee_RM.fbx`; `Humanoid@RunBackwardsRight1hMelee.fbx`; `Humanoid@RunBackwardsRight1hMelee_RM.fbx`; `Humanoid@RunRight1hMelee.fbx`; `Humanoid@RunRight1hMelee_RM.fbx`; `Humanoid@RunForwardRight1hMelee.fbx`; `Humanoid@RunForwardRight1hMelee_RM.fbx` | set_type=directional-blend | duration=0.600 s; rm_speed=1.905 m/s | loop_ip=true; loop_rm=true; sync=not-evaluated |
| `walk-combat-8-way` | eight-way directional gait | `Humanoid@WalkForwardCombat1hMelee.fbx`; `Humanoid@WalkForwardCombat1hMelee_RM.fbx`; `Humanoid@WalkForwardLeftCombat1hMelee.fbx`; `Humanoid@WalkForwardLeftCombat1hMelee_RM.fbx`; `Humanoid@WalkLeftCombat1hMelee.fbx`; `Humanoid@WalkLeftCombat1hMelee_RM.fbx`; `Humanoid@WalkBackwardsLeftCombat1hMelee.fbx`; `Humanoid@WalkBackwardsLeftCombat1hMelee_RM.fbx`; `Humanoid@WalkBackwardsCombat1hMelee.fbx`; `Humanoid@WalkBackwardsCombat1hMelee_RM.fbx`; `Humanoid@WalkBackwardsRightCombat1hMelee.fbx`; `Humanoid@WalkBackwardsRightCombat1hMelee_RM.fbx`; `Humanoid@WalkRightCombat1hMelee.fbx`; `Humanoid@WalkRightCombat1hMelee_RM.fbx`; `Humanoid@WalkForwardRightCombat1hMelee.fbx`; `Humanoid@WalkForwardRightCombat1hMelee_RM.fbx` | set_type=directional-blend | duration=1.333 s; rm_speed=0.491 m/s | loop_ip=true; loop_rm=true; sync=not-evaluated |

The grouped table gives minimum duration and RM speed. Run durations span 0.600–0.667 s. Measured RM speed spans are `crouch-combat-8-way` 0.480–0.730 m/s (ratio 0.657); `run-8-way` 1.905–2.117 m/s (ratio 0.900); `walk-combat-8-way` 0.491–0.951 m/s (ratio 0.516). Preserve authored variation unless the project declares a normalization policy; diagonals and cardinals require the full blend test. Measured source in-place phase spreads (cycles, eight measured members each) are `crouch-combat-8-way` 0.7136; `run-8-way` 0.7342; `walk-combat-8-way` 0.5538. Phase spread here is the minimum covering arc in cycles: sort phases in `[0,1)`, include the wraparound gap, then subtract the largest gap from 1. It is not `max_circular_deviation_from_mean`; neither measure alone proves support-foot or visual compatibility. Keep `sync=not-evaluated` until the selected synchronization and contact policy is tested. The current catalog also measures `hold-forward-speed`, `draw-combat-put-away`, and `heavy-hit-4-way` as evaluator-defined hypotheses; they remain candidate groupings.

### Exact runtime members

Each row is one selectable eight-member variant. Numeric duration and RM-speed table values are the measured minima, not a uniform value for every member. The retained current clip catalog measures both `run-8-way-in-place` and `run-8-way-root-motion` at 0.600–0.667 s across their respective eight files; their combined group has the same duration range. RM-speed ranges remain in the grouped measurement paragraph above. Continuous-loop declarations remain unaccepted.

| Set/profile | Role or coordinate | Exact members | Variant/type | Timing or motion | Runtime contract |
|---|---|---|---|---|---|
| `crouch-combat-8-way-in-place` | eight-way directional gait | `Humanoid@CrouchForward1hMelee.fbx`; `Humanoid@CrouchForwardLeft1hMelee.fbx`; `Humanoid@CrouchLeft1hMelee.fbx`; `Humanoid@CrouchBackwardsLeft1hMelee.fbx`; `Humanoid@CrouchBackwards1hMelee.fbx`; `Humanoid@CrouchBackwardsRight1hMelee.fbx`; `Humanoid@CrouchRight1hMelee.fbx`; `Humanoid@CrouchForwardRight1hMelee.fbx` | variant=in-place | duration=1.667 s | loop=true; sync=not-evaluated |
| `crouch-combat-8-way-root-motion` | eight-way directional gait | `Humanoid@CrouchForward1hMelee_RM.fbx`; `Humanoid@CrouchForwardLeft1hMelee_RM.fbx`; `Humanoid@CrouchLeft1hMelee_RM.fbx`; `Humanoid@CrouchBackwardsLeft1hMelee_RM.fbx`; `Humanoid@CrouchBackwards1hMelee_RM.fbx`; `Humanoid@CrouchBackwardsRight1hMelee_RM.fbx`; `Humanoid@CrouchRight1hMelee_RM.fbx`; `Humanoid@CrouchForwardRight1hMelee_RM.fbx` | variant=root-motion | duration=1.667 s; rm_speed=0.480 m/s | loop=true; sync=not-evaluated |
| `run-8-way-in-place` | eight-way directional gait | `Humanoid@RunForward1hMelee.fbx`; `Humanoid@RunForwardLeft1hMelee.fbx`; `Humanoid@RunLeft1hMelee.fbx`; `Humanoid@RunBackwardsLeft1hMelee.fbx`; `Humanoid@RunBackwards1hMelee.fbx`; `Humanoid@RunBackwardsRight1hMelee.fbx`; `Humanoid@RunRight1hMelee.fbx`; `Humanoid@RunForwardRight1hMelee.fbx` | variant=in-place | duration=0.600 s | loop=true; sync=not-evaluated |
| `run-8-way-root-motion` | eight-way directional gait | `Humanoid@RunForward1hMelee_RM.fbx`; `Humanoid@RunForwardLeft1hMelee_RM.fbx`; `Humanoid@RunLeft1hMelee_RM.fbx`; `Humanoid@RunBackwardsLeft1hMelee_RM.fbx`; `Humanoid@RunBackwards1hMelee_RM.fbx`; `Humanoid@RunBackwardsRight1hMelee_RM.fbx`; `Humanoid@RunRight1hMelee_RM.fbx`; `Humanoid@RunForwardRight1hMelee_RM.fbx` | variant=root-motion | duration=0.600 s; rm_speed=1.905 m/s | loop=true; sync=not-evaluated |
| `walk-combat-8-way-in-place` | eight-way directional gait | `Humanoid@WalkForwardCombat1hMelee.fbx`; `Humanoid@WalkForwardLeftCombat1hMelee.fbx`; `Humanoid@WalkLeftCombat1hMelee.fbx`; `Humanoid@WalkBackwardsLeftCombat1hMelee.fbx`; `Humanoid@WalkBackwardsCombat1hMelee.fbx`; `Humanoid@WalkBackwardsRightCombat1hMelee.fbx`; `Humanoid@WalkRightCombat1hMelee.fbx`; `Humanoid@WalkForwardRightCombat1hMelee.fbx` | variant=in-place | duration=1.333 s | loop=true; sync=not-evaluated |
| `walk-combat-8-way-root-motion` | eight-way directional gait | `Humanoid@WalkForwardCombat1hMelee_RM.fbx`; `Humanoid@WalkForwardLeftCombat1hMelee_RM.fbx`; `Humanoid@WalkLeftCombat1hMelee_RM.fbx`; `Humanoid@WalkBackwardsLeftCombat1hMelee_RM.fbx`; `Humanoid@WalkBackwardsCombat1hMelee_RM.fbx`; `Humanoid@WalkBackwardsRightCombat1hMelee_RM.fbx`; `Humanoid@WalkRightCombat1hMelee_RM.fbx`; `Humanoid@WalkForwardRightCombat1hMelee_RM.fbx` | variant=root-motion | duration=1.333 s; rm_speed=0.491 m/s | loop=true; sync=not-evaluated |

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
| Individual motions | Current evaluator reads and measures exact files | Declaration-clean subset only | Engine and visual behavior not evaluated |
| Directional gait hypotheses | Timing, speed, skeleton, and current errors recorded | 0 contract-clean members in each named gait set | Full blend, phase, root-owner, and contact gates open |
| Transition/action hypotheses | Exact files measured | One-shot/loop intent requires authority | State graph, contact, and artistic gates open |

### Validation-profile status

| Validation profile | Selection | Result / next evidence |
|---|---|---|
| Marketplace intake | `selected` — `vendor-intended` | Current source inventory and exhaustive mechanical intake complete. |
| Blended locomotion | `selected` — `evaluator-selected-generic-scenario` | Named gait hypotheses measured; contracts and full blend review remain open. |
| Root-motion controller | `selected` — `observed-pack-capability` | RM variants measured; controller/collision ownership not evaluated. |
| State-machine transitions | `selected` — `observed-pack-capability` | Transition candidates identified; engine playback not evaluated. |
| Layered upper body/weapons | `not-selected` | No mask/socket/IK authority or test. |
| Traversal/environment | `not-selected` | No target scene or traversal contacts. |
| Contact actions/interactions | `selected` — `observed-pack-capability` | Combat files present; weapon/body contacts not evaluated. |
| Retargeted/customizable characters | `not-selected` | No target rig or reference-pose test. |
| Motion matching/search | `not-selected` | No database construction or query test. |
| Networked movement | `not-selected` | No networking or reconciliation design. |
| Runtime performance | `not-selected` | No target build or benchmark. |

## Pack inventory and content evidence

The current inventory contains 113 FBXs: 112 animation-bearing files, 110 individual-motion files, and 1 combined take. 109 individual-motion/pack files share the 56-bone `2b6fe49d5ae6` skeleton; `Humanoid@Blocked1hMelee.fbx` is a separate 73-bone variant. Counts describe physical current inputs, not runtime acceptance.

### Current in-place gait phase evidence

Phase spread here is the minimum covering arc in cycles: sort phases in `[0,1)`, include the wraparound gap, then subtract the largest gap from 1. It is not `max_circular_deviation_from_mean`; neither measure alone proves support-foot or visual compatibility. These are `observed-animsmith` source measurements for the exact hypotheses above; synchronization/contact acceptance remains open.

| Current hypothesis | Exact source member (`Take 001`) | Measured phase (cycles) |
|---|---|---|
| `crouch-combat-8-way` | `Humanoid@CrouchForward1hMelee.fbx` | 0.093914 |
| `crouch-combat-8-way` | `Humanoid@CrouchForwardLeft1hMelee.fbx` | 0.095760 |
| `crouch-combat-8-way` | `Humanoid@CrouchLeft1hMelee.fbx` | 0.374620 |
| `crouch-combat-8-way` | `Humanoid@CrouchBackwardsLeft1hMelee.fbx` | 0.378580 |
| `crouch-combat-8-way` | `Humanoid@CrouchBackwards1hMelee.fbx` | 0.861504 |
| `crouch-combat-8-way` | `Humanoid@CrouchBackwardsRight1hMelee.fbx` | 0.916710 |
| `crouch-combat-8-way` | `Humanoid@CrouchRight1hMelee.fbx` | 0.919138 |
| `crouch-combat-8-way` | `Humanoid@CrouchForwardRight1hMelee.fbx` | 0.575092 |
| `run-8-way` | `Humanoid@RunForward1hMelee.fbx` | 0.192225 |
| `run-8-way` | `Humanoid@RunForwardLeft1hMelee.fbx` | 0.226898 |
| `run-8-way` | `Humanoid@RunLeft1hMelee.fbx` | 0.299076 |
| `run-8-way` | `Humanoid@RunBackwardsLeft1hMelee.fbx` | 0.358187 |
| `run-8-way` | `Humanoid@RunBackwards1hMelee.fbx` | 0.926401 |
| `run-8-way` | `Humanoid@RunBackwardsRight1hMelee.fbx` | 0.799076 |
| `run-8-way` | `Humanoid@RunRight1hMelee.fbx` | 0.552283 |
| `run-8-way` | `Humanoid@RunForwardRight1hMelee.fbx` | 0.675372 |
| `walk-combat-8-way` | `Humanoid@WalkForwardCombat1hMelee.fbx` | 0.128668 |
| `walk-combat-8-way` | `Humanoid@WalkForwardLeftCombat1hMelee.fbx` | 0.126990 |
| `walk-combat-8-way` | `Humanoid@WalkLeftCombat1hMelee.fbx` | 0.276900 |
| `walk-combat-8-way` | `Humanoid@WalkBackwardsLeftCombat1hMelee.fbx` | 0.837304 |
| `walk-combat-8-way` | `Humanoid@WalkBackwardsCombat1hMelee.fbx` | 0.796839 |
| `walk-combat-8-way` | `Humanoid@WalkBackwardsRightCombat1hMelee.fbx` | 0.878098 |
| `walk-combat-8-way` | `Humanoid@WalkRightCombat1hMelee.fbx` | 0.723103 |
| `walk-combat-8-way` | `Humanoid@WalkForwardRightCombat1hMelee.fbx` | 0.178871 |

## Mechanical baseline

| Finding/check | Affected scope | Potential impact | Evidence |
|---|---|---|---|
| Current readability | 113/113 inputs | Enables mechanical intake | `observed-animsmith`; all four commands per input exit 0 |
| Constant tracks | 13629 notes across 112 animation-bearing files | Storage/evaluation noise; not a gameplay defect by itself | `observed-animsmith`; baseline ledger SHA-256 `2017c5adce72a3164d891bc0eda281ad0583d6285282aa423b5e47b443a591dd` |
| Retained declarations | 87/110 non-clean | Blocks admission under those exact declarations | `observed-animsmith`; contract ledger SHA-256 `57eb976f00807af833e25609b9ca74c019bd321800074ec87855524a84f6bb04` |
| Structural/timing warnings | Named scope above | Requires bounded author/project decision | `observed-file` and `observed-animsmith` |

## AnimSmith remediation evidence

| Source issue | Operation/declarations | Result | Independent verification | Remaining caveat |
|---|---|---|---|---|
| `PF1-01` gait continuity | `transform --gait-anchor` on 24 selected in-place gait files; retained per-file configs | 24 outputs produced; selected inputs have no accumulating root translation/yaw | 24 inspect and measure commands exit 0; output hashes and diffs captured | Post-config lint remains non-clean; no RM trajectory was transformed; visual/contact residual unresolved |
| Constant-track notes | `transform --prune-constant-tracks` on `Humanoid@IdleCombat1hMelee.fbx` | Output candidate(s) produced | Inspect, measure, diff, fix dry-run, and config lint captured | 25/25 post-transform config lints exit 1. Candidate adoption and engine/visual acceptance remain open |

Fresh remediation ledger SHA-256 `b6e30d9afd4d816057d41e58d7627cb39cc272074b491d1a8be4c625fb5901df`. Generated motion stays outside Git.

## Engine procedures and evidence

| Runtime | Version | Procedure | Observed result | Remaining gate |
|---|---|---|---|---|
| Unity | not evaluated | No current pack procedure executed in this report pair | `not-evaluated` | Import, sample, full blend, root, contact, and visual review |
| Unreal Engine | not evaluated | No current procedure executed | `not-evaluated` | Import, retarget, root, blend, and visual review |
| Godot | not evaluated | No current procedure executed | `not-evaluated` | Convert/import, graph, root, contact, and visual review |
| Bevy | not evaluated | No current procedure executed | `not-evaluated` | Convert/load, graph, root, performance, and visual review |

## Rig, masking, and compatibility evidence

| Pack/rig/set pair | Skeleton/retarget | Scale/axes | Root policy | Timing/blend | Overall evidence |
|---|---|---|---|---|---|
| One-Handed Melee internal gait sets | Current signatures measured; no retarget | No engine scale/axis test | IP/RM files measured; owner not selected | Current duration/speed/phase recorded; full blend absent | Mechanical only |
| One Handed Melee plus Basic Locomotion | Named Humanoid sources imported on shared `SK_Protof-Actor` | Unity import scale/axis accepted for named sources only | Gameplay-owned root fixed for IP/idle; animation-owned root finite for tested RM where applicable | Named Playables transitions finite; visual blend not inspected | Bounded source/controller feasibility, not compatibility acceptance |

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

Preflight verified version/help for required commands and representative GLB/FBX admission before replay; scrubbed locator `external:0.14.0-report-refresh/preflight.json`, SHA-256 `7034b5043de54e7b30064230db18605d1f96d0ea1477f7b8c56ce817cc73521a`. The read-only helper replayed the retained exact `inspect`, `measure`, and two `lint` invocations with four workers, recorded argv/exit/stdout/stderr and source/config hashes, and did not mutate inputs. Baseline, contract, and remediation ledger digests are `2017c5adce72a3164d891bc0eda281ad0583d6285282aa423b5e47b443a591dd`, `57eb976f00807af833e25609b9ca74c019bd321800074ec87855524a84f6bb04`, and `b6e30d9afd4d816057d41e58d7627cb39cc272074b491d1a8be4c625fb5901df`. Licensed paths, sources, and derivative outputs are excluded from Git and this report.

Fresh Unity evidence: editor `6000.5.8f1`, `Unity.exe` SHA-256 `cdc0eeca135394cde79a632aa998aca14b521b58afe8c2750b009aff608be906`; probe SHA-256 `28cd00897dfd739e2a7bcfa193adab03de3591a1009802946305127b67d1bfb6`; result schema `animsmith-unity-crosspack-probe-v1`, result SHA-256 `f067bca91f182b65fab408a1c97b297377a8f361b03d59e809e236aba666652c`; log SHA-256 `0429bcb9cc7a7492c6637eb799566463ba915243d0a110b4d76cbe75590c8819`; 26/26 aggregate required checks passed with 53 nonempty Humanoid bones per sample. This report uses only the named applicable subset above.

## Sources

- Protofactor product context and [End User License Agreement](https://protofactor.biz/end-user-license-agreement/) — context only; local revision remains unknown.
