# Animation pack evidence appendix: Protofactor Two-Handed Melee Animset

> Companion report: [technical evaluation](protofactor-two-handed-melee.md)
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
| Delivered scope | 123 retained FBX candidates; source bytes unchanged during replay |
| Target use | Broad game-engine technical intake; no target controller supplied |
| Target engines | Unity 6000.5.8f1 bounded source/controller probe; Unreal, Godot, and Bevy not evaluated |
| Target rigs/packs | Delivered rigs; Basic Locomotion compatibility not freshly evaluated |
| Source manifest | `external:protofactor-two-handed-melee/source-inventory.json`; SHA-256 `1fc1189eb8ccb299fe26ff51bce96d932b7b2dab5c71a0c6adab477937ad7bc7` |
| Evaluation manifest | Retained `urn:animsmith:skill:animation-pack-evaluation-manifest:1`; current memberships treated as hypotheses and remeasured |
| Acquisition/license provenance | Authorized local commercial input; no legal conclusion |

### Evidence coverage

| Surface | Offered/delivered | Evaluated | Findings | Not evaluated and why |
|---|---:|---:|---:|---|
| Animation files | 123 | 123 | 122 animation-bearing | Engine semantics not implied |
| Rigs/export variants | 123 | 123 | 118 individual-motion files share the 58-bone `3da84463466a` skeleton; `Humanoid@Blocked2HandMelee.fbx` and `Humanoid@IdleBlock2HandMelee.fbx` use the 56-bone `2b6fe49d5ae6` variant | Retarget/reference-pose behavior not run |
| AnimSmith baseline | 123 | 123 | All inspect/measure/two lint commands exit 0; One duration-sanity warning on `Humanoid@GetHitLeftHeavy2HandMelee_RM.fbx`; six time-monotonic notes. | N/A |
| Declared contracts | 120 | 120 | 13 pass, 107 fail in each format | Declaration intent needs project/vendor authority |
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
| `idle-pose` | 8 | 8 | `observed-file`; evaluator classification, not vendor semantics |
| `continuous-locomotion` | 56 | 56 | `observed-file`; evaluator classification, not vendor semantics |
| `locomotion-transition` | 0 | 0 | `observed-file`; evaluator classification, not vendor semantics |
| `airborne` | 5 | 5 | `observed-file`; evaluator classification, not vendor semantics |
| `traversal` | 0 | 0 | `observed-file`; evaluator classification, not vendor semantics |
| `action-interaction` | 34 | 34 | `observed-file`; evaluator classification, not vendor semantics |
| `reaction-death` | 17 | 17 | `observed-file`; evaluator classification, not vendor semantics |
| `emote-cinematic` | 0 | 0 | `observed-file`; evaluator classification, not vendor semantics |
| `other-unknown` | 0 | 0 | `observed-file`; evaluator classification, not vendor semantics |
| **Total** | **120** | **120** | Current exact-file catalog |

### Runtime-set inventory

The untested acceptance below applies to each complete set. [Named Unity source
samples and mixers](#engine-procedures-and-evidence) ran for a smaller subset;
they do not establish full-set playback or visual quality.

| Runtime set | Type | Members/variants | Grouping evidence | Validation status |
|---|---|---|---|---|
| `crouch-combat-8-way` | directional-blend | 16 exact files; 8 IP, 8 RM | Current exact-file measurements; grouping is an evaluator-defined hypothesis seeded by retained catalog declarations | 0/16 files pass the retained declaration; whole-set engine and visual acceptance not evaluated |
| `run-combat-8-way` | directional-blend | 16 exact files; 8 IP, 8 RM | Current exact-file measurements; grouping is an evaluator-defined hypothesis seeded by retained catalog declarations | 0/16 files pass the retained declaration; whole-set engine and visual acceptance not evaluated |
| `walk-combat-8-way` | directional-blend | 16 exact files; 8 IP, 8 RM | Current exact-file measurements; grouping is an evaluator-defined hypothesis seeded by retained catalog declarations | 0/16 files pass the retained declaration; whole-set engine and visual acceptance not evaluated |
| `normal-forward-speed` | speed-blend | 6 exact files; 3 IP, 3 RM | Current exact-file measurements; grouping is an evaluator-defined hypothesis seeded by retained catalog declarations | 0/6 files pass the retained declaration; whole-set engine and visual acceptance not evaluated |
| `draw-combat-put-away` | transition-chain | 3 exact files; 3 IP, 0 RM | Current exact-file measurements; grouping is an evaluator-defined hypothesis seeded by retained catalog declarations | 2/3 files pass the retained declaration; whole-set engine and visual acceptance not evaluated |
| `dodge-forward-back` | other | 4 exact files; 2 IP, 2 RM | Current exact-file measurements; grouping is an evaluator-defined hypothesis seeded by retained catalog declarations | 0/4 files pass the retained declaration; whole-set engine and visual acceptance not evaluated |
| `parry-3-way` | other | 3 exact files; 3 IP, 0 RM | Current exact-file measurements; grouping is an evaluator-defined hypothesis seeded by retained catalog declarations | 0/3 files pass the retained declaration; whole-set engine and visual acceptance not evaluated |
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
| `crouch-combat-8-way` | eight-way directional gait | `Humanoid@CrouchForward2HandMelee.fbx`; `Humanoid@CrouchForward2HandMelee_RM.fbx`; `Humanoid@CrouchForwardLeft2HandMelee.fbx`; `Humanoid@CrouchForwardLeft2HandMelee_RM.fbx`; `Humanoid@CrouchLeft2HandMelee.fbx`; `Humanoid@CrouchLeft2HandMelee_RM.fbx`; `Humanoid@CrouchBackwardsLeft2HandMelee.fbx`; `Humanoid@CrouchBackwardsLeft2HandMelee_RM.fbx`; `Humanoid@CrouchBackwards2HandMelee.fbx`; `Humanoid@CrouchBackwards2HandMelee_RM.fbx`; `Humanoid@CrouchBackwardsRight2HandMelee.fbx`; `Humanoid@CrouchBackwardsRight2HandMelee_RM.fbx`; `Humanoid@CrouchRight2HandMelee.fbx`; `Humanoid@CrouchRight2HandMelee_RM.fbx`; `Humanoid@CrouchForwardRight2HandMelee.fbx`; `Humanoid@CrouchForwardRight2HandMelee_RM.fbx` | set_type=directional-blend | duration=1.667 s; rm_speed=0.642 m/s | loop_ip=true; loop_rm=true; sync=not-evaluated |
| `run-combat-8-way` | eight-way directional gait | `Humanoid@RunForwardCombat2HandMelee.fbx`; `Humanoid@RunForwardCombat2HandMelee_RM.fbx`; `Humanoid@RunForwardLeftCombat2HandMelee.fbx`; `Humanoid@RunForwardLeftCombat2HandMelee_RM.fbx`; `Humanoid@RunLeftCombat2HandMelee.fbx`; `Humanoid@RunLeftCombat2HandMelee_RM.fbx`; `Humanoid@RunBackwardsLeftCombat2HandMelee.fbx`; `Humanoid@RunBackwardsLeftCombat2HandMelee_RM.fbx`; `Humanoid@RunBackwardsCombat2HandMelee.fbx`; `Humanoid@RunBackwardsCombat2HandMelee_RM.fbx`; `Humanoid@RunBackwardsRightCombat2HandMelee.fbx`; `Humanoid@RunBackwardsRightCombat2HandMelee_RM.fbx`; `Humanoid@RunRightCombat2HandMelee.fbx`; `Humanoid@RunRightCombat2HandMelee_RM.fbx`; `Humanoid@RunForwardRightCombat2HandMelee.fbx`; `Humanoid@RunForwardRightCombat2HandMelee_RM.fbx` | set_type=directional-blend | duration=0.533 s; rm_speed=2.202 m/s | loop_ip=true; loop_rm=true; sync=not-evaluated |
| `walk-combat-8-way` | eight-way directional gait | `Humanoid@WalkForwardCombat2HandMelee.fbx`; `Humanoid@WalkForwardCombat2HandMelee_RM.fbx`; `Humanoid@WalkForwardLeftCombat2HandMelee.fbx`; `Humanoid@WalkForwardLeftCombat2HandMelee_RM.fbx`; `Humanoid@WalkLeftCombat2HandMelee.fbx`; `Humanoid@WalkLeftCombat2HandMelee_RM.fbx`; `Humanoid@WalkBackwardsLeftCombat2HandMelee.fbx`; `Humanoid@WalkBackwardsLeftCombat2HandMelee_RM.fbx`; `Humanoid@WalkBackwardsCombat2HandMelee.fbx`; `Humanoid@WalkBackwardsCombat2HandMelee_RM.fbx`; `Humanoid@WalkBackwardsRightCombat2HandMelee.fbx`; `Humanoid@WalkBackwardsRightCombat2HandMelee_RM.fbx`; `Humanoid@WalkRightCombat2HandMelee.fbx`; `Humanoid@WalkRightCombat2HandMelee_RM.fbx`; `Humanoid@WalkForwardRightCombat2HandMelee.fbx`; `Humanoid@WalkForwardRightCombat2HandMelee_RM.fbx` | set_type=directional-blend | duration=1.333 s; rm_speed=0.810 m/s | loop_ip=true; loop_rm=true; sync=not-evaluated |

The grouped table gives minimum duration and RM speed. Run durations span 0.533–0.567 s. Measured RM speed spans are `crouch-combat-8-way` 0.642–0.835 m/s (ratio 0.769); `run-combat-8-way` 2.202–2.690 m/s (ratio 0.819); `walk-combat-8-way` 0.810–1.092 m/s (ratio 0.742). Preserve authored variation unless the project declares a normalization policy; diagonals and cardinals require the full blend test. Measured source in-place phase spreads (cycles, eight measured members each) are `crouch-combat-8-way` 0.5774; `run-combat-8-way` 0.6024; `walk-combat-8-way` 0.7112. Phase spread here is the minimum covering arc in cycles: sort phases in `[0,1)`, include the wraparound gap, then subtract the largest gap from 1. It is not `max_circular_deviation_from_mean`; neither measure alone proves support-foot or visual compatibility. Keep `sync=not-evaluated` until the selected synchronization and contact policy is tested. The current catalog also measures `normal-forward-speed`, `draw-combat-put-away`, `dodge-forward-back`, and `parry-3-way` as evaluator-defined hypotheses; they remain candidate groupings.

### Exact runtime members

Each row is one selectable eight-member variant. Numeric duration and RM-speed table values are the measured minima, not a uniform value for every member. The retained current clip catalog measures both `run-combat-8-way-in-place` and `run-combat-8-way-root-motion` at 0.533–0.567 s across their respective eight files; their combined group has the same duration range. RM-speed ranges remain in the grouped measurement paragraph above. Continuous-loop declarations remain unaccepted.

| Set/profile | Role or coordinate | Exact members | Variant/type | Timing or motion | Runtime contract |
|---|---|---|---|---|---|
| `crouch-combat-8-way-in-place` | eight-way directional gait | `Humanoid@CrouchForward2HandMelee.fbx`; `Humanoid@CrouchForwardLeft2HandMelee.fbx`; `Humanoid@CrouchLeft2HandMelee.fbx`; `Humanoid@CrouchBackwardsLeft2HandMelee.fbx`; `Humanoid@CrouchBackwards2HandMelee.fbx`; `Humanoid@CrouchBackwardsRight2HandMelee.fbx`; `Humanoid@CrouchRight2HandMelee.fbx`; `Humanoid@CrouchForwardRight2HandMelee.fbx` | variant=in-place | duration=1.667 s | loop=true; sync=not-evaluated |
| `crouch-combat-8-way-root-motion` | eight-way directional gait | `Humanoid@CrouchForward2HandMelee_RM.fbx`; `Humanoid@CrouchForwardLeft2HandMelee_RM.fbx`; `Humanoid@CrouchLeft2HandMelee_RM.fbx`; `Humanoid@CrouchBackwardsLeft2HandMelee_RM.fbx`; `Humanoid@CrouchBackwards2HandMelee_RM.fbx`; `Humanoid@CrouchBackwardsRight2HandMelee_RM.fbx`; `Humanoid@CrouchRight2HandMelee_RM.fbx`; `Humanoid@CrouchForwardRight2HandMelee_RM.fbx` | variant=root-motion | duration=1.667 s; rm_speed=0.642 m/s | loop=true; sync=not-evaluated |
| `run-combat-8-way-in-place` | eight-way directional gait | `Humanoid@RunForwardCombat2HandMelee.fbx`; `Humanoid@RunForwardLeftCombat2HandMelee.fbx`; `Humanoid@RunLeftCombat2HandMelee.fbx`; `Humanoid@RunBackwardsLeftCombat2HandMelee.fbx`; `Humanoid@RunBackwardsCombat2HandMelee.fbx`; `Humanoid@RunBackwardsRightCombat2HandMelee.fbx`; `Humanoid@RunRightCombat2HandMelee.fbx`; `Humanoid@RunForwardRightCombat2HandMelee.fbx` | variant=in-place | duration=0.533 s | loop=true; sync=not-evaluated |
| `run-combat-8-way-root-motion` | eight-way directional gait | `Humanoid@RunForwardCombat2HandMelee_RM.fbx`; `Humanoid@RunForwardLeftCombat2HandMelee_RM.fbx`; `Humanoid@RunLeftCombat2HandMelee_RM.fbx`; `Humanoid@RunBackwardsLeftCombat2HandMelee_RM.fbx`; `Humanoid@RunBackwardsCombat2HandMelee_RM.fbx`; `Humanoid@RunBackwardsRightCombat2HandMelee_RM.fbx`; `Humanoid@RunRightCombat2HandMelee_RM.fbx`; `Humanoid@RunForwardRightCombat2HandMelee_RM.fbx` | variant=root-motion | duration=0.533 s; rm_speed=2.202 m/s | loop=true; sync=not-evaluated |
| `walk-combat-8-way-in-place` | eight-way directional gait | `Humanoid@WalkForwardCombat2HandMelee.fbx`; `Humanoid@WalkForwardLeftCombat2HandMelee.fbx`; `Humanoid@WalkLeftCombat2HandMelee.fbx`; `Humanoid@WalkBackwardsLeftCombat2HandMelee.fbx`; `Humanoid@WalkBackwardsCombat2HandMelee.fbx`; `Humanoid@WalkBackwardsRightCombat2HandMelee.fbx`; `Humanoid@WalkRightCombat2HandMelee.fbx`; `Humanoid@WalkForwardRightCombat2HandMelee.fbx` | variant=in-place | duration=1.333 s | loop=true; sync=not-evaluated |
| `walk-combat-8-way-root-motion` | eight-way directional gait | `Humanoid@WalkForwardCombat2HandMelee_RM.fbx`; `Humanoid@WalkForwardLeftCombat2HandMelee_RM.fbx`; `Humanoid@WalkLeftCombat2HandMelee_RM.fbx`; `Humanoid@WalkBackwardsLeftCombat2HandMelee_RM.fbx`; `Humanoid@WalkBackwardsCombat2HandMelee_RM.fbx`; `Humanoid@WalkBackwardsRightCombat2HandMelee_RM.fbx`; `Humanoid@WalkRightCombat2HandMelee_RM.fbx`; `Humanoid@WalkForwardRightCombat2HandMelee_RM.fbx` | variant=root-motion | duration=1.333 s; rm_speed=0.810 m/s | loop=true; sync=not-evaluated |

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

The current inventory contains 123 FBXs: 122 animation-bearing files, 120 individual-motion files, and 1 combined take. 118 individual-motion files share the 58-bone `3da84463466a` skeleton; `Humanoid@Blocked2HandMelee.fbx` and `Humanoid@IdleBlock2HandMelee.fbx` use the 56-bone `2b6fe49d5ae6` variant. Counts describe physical current inputs, not runtime acceptance.

### Current in-place gait phase evidence

Phase spread here is the minimum covering arc in cycles: sort phases in `[0,1)`, include the wraparound gap, then subtract the largest gap from 1. It is not `max_circular_deviation_from_mean`; neither measure alone proves support-foot or visual compatibility. These are `observed-animsmith` source measurements for the exact hypotheses above; synchronization/contact acceptance remains open.

| Current hypothesis | Exact source member (`Take 001`) | Measured phase (cycles) |
|---|---|---|
| `crouch-combat-8-way` | `Humanoid@CrouchForward2HandMelee.fbx` | 0.136668 |
| `crouch-combat-8-way` | `Humanoid@CrouchForwardLeft2HandMelee.fbx` | 0.162019 |
| `crouch-combat-8-way` | `Humanoid@CrouchLeft2HandMelee.fbx` | 0.213006 |
| `crouch-combat-8-way` | `Humanoid@CrouchBackwardsLeft2HandMelee.fbx` | 0.783259 |
| `crouch-combat-8-way` | `Humanoid@CrouchBackwards2HandMelee.fbx` | 0.857985 |
| `crouch-combat-8-way` | `Humanoid@CrouchBackwardsRight2HandMelee.fbx` | 0.840786 |
| `crouch-combat-8-way` | `Humanoid@CrouchRight2HandMelee.fbx` | 0.635619 |
| `crouch-combat-8-way` | `Humanoid@CrouchForwardRight2HandMelee.fbx` | 0.145788 |
| `run-combat-8-way` | `Humanoid@RunForwardCombat2HandMelee.fbx` | 0.117562 |
| `run-combat-8-way` | `Humanoid@RunForwardLeftCombat2HandMelee.fbx` | 0.100797 |
| `run-combat-8-way` | `Humanoid@RunLeftCombat2HandMelee.fbx` | 0.239777 |
| `run-combat-8-way` | `Humanoid@RunBackwardsLeftCombat2HandMelee.fbx` | 0.792904 |
| `run-combat-8-way` | `Humanoid@RunBackwardsCombat2HandMelee.fbx` | 0.688387 |
| `run-combat-8-way` | `Humanoid@RunBackwardsRightCombat2HandMelee.fbx` | 0.710328 |
| `run-combat-8-way` | `Humanoid@RunRightCombat2HandMelee.fbx` | 0.637374 |
| `run-combat-8-way` | `Humanoid@RunForwardRightCombat2HandMelee.fbx` | 0.151982 |
| `walk-combat-8-way` | `Humanoid@WalkForwardCombat2HandMelee.fbx` | 0.139942 |
| `walk-combat-8-way` | `Humanoid@WalkForwardLeftCombat2HandMelee.fbx` | 0.150506 |
| `walk-combat-8-way` | `Humanoid@WalkLeftCombat2HandMelee.fbx` | 0.382751 |
| `walk-combat-8-way` | `Humanoid@WalkBackwardsLeftCombat2HandMelee.fbx` | 0.304546 |
| `walk-combat-8-way` | `Humanoid@WalkBackwardsCombat2HandMelee.fbx` | 0.851128 |
| `walk-combat-8-way` | `Humanoid@WalkBackwardsRightCombat2HandMelee.fbx` | 0.694238 |
| `walk-combat-8-way` | `Humanoid@WalkRightCombat2HandMelee.fbx` | 0.678060 |
| `walk-combat-8-way` | `Humanoid@WalkForwardRightCombat2HandMelee.fbx` | 0.653921 |

## Mechanical baseline

| Finding/check | Affected scope | Potential impact | Evidence |
|---|---|---|---|
| Current readability | 123/123 inputs | Enables mechanical intake | `observed-animsmith`; all four commands per input exit 0 |
| Constant tracks | 17010 notes across 122 animation-bearing files | Storage/evaluation noise; not a gameplay defect by itself | `observed-animsmith`; baseline ledger SHA-256 `ddb5a9e2e0cdc97bd4b768fcf8700da32f4a22358babd50a101416a14c9737d5` |
| Retained declarations | 107/120 non-clean | Blocks admission under those exact declarations | `observed-animsmith`; contract ledger SHA-256 `0438c30a7e86b46ca0be07b02baf0e7145a22125fc6f6f8f8eacfef2047a5267` |
| Structural/timing warnings | Named scope above | Requires bounded author/project decision | `observed-file` and `observed-animsmith` |

## AnimSmith remediation evidence

| Source issue | Operation/declarations | Result | Independent verification | Remaining caveat |
|---|---|---|---|---|
| `PF2-01` gait continuity | `transform --gait-anchor` on 24 selected in-place gait files; retained per-file configs | 24 outputs produced; selected inputs have no accumulating root translation/yaw | 24 inspect and measure commands exit 0; output hashes and diffs captured | Post-config lint remains non-clean; no RM trajectory was transformed; visual/contact residual unresolved |
| Constant-track notes | `transform --prune-constant-tracks` on `Humanoid@IdleCombatA2HandMelee.fbx` | Output candidate(s) produced | Inspect, measure, diff, fix dry-run, and config lint captured | 25/25 post-transform config lints exit 1. Candidate adoption and engine/visual acceptance remain open |

Fresh remediation ledger SHA-256 `0f15f4171bf799b5e4bef91251f94dbfaaed8d6f4341a59b05be52f8e1b61430`. Generated motion stays outside Git.

## Engine procedures and evidence

| Runtime | Version | Procedure | Observed result | Remaining gate |
|---|---|---|---|---|
| Unity | 6000.5.8f1 | Fresh named Humanoid source import and standalone sampling: IdleCombatA2HandMelee, WalkForwardCombat2HandMelee, WalkForwardCombat2HandMelee_RM; named Playables mixers: basic-idle-to-twohand-idle, sword-shield-idle-to-twohand-idle, basic-walk-ip-to-twohand-walk-ip. See [collection source/controller probe](protofactor-ultimate-animation-collection-evidence.md#rig-masking-and-compatibility-evidence). | Bounded source/controller feasibility for listed sources only; the collection’s 26/26 is a shared aggregate, not this pack’s result | Rendered motion, contact, deformation, full blend, retargeting, authored AnimatorController, player build and performance remain untested |
| Unreal Engine | not evaluated | No current procedure executed | `not-evaluated` | Import, retarget, root, blend, and visual review |
| Godot | not evaluated | No current procedure executed | `not-evaluated` | Convert/import, graph, root, contact, and visual review |
| Bevy | not evaluated | No current procedure executed | `not-evaluated` | Convert/load, graph, root, performance, and visual review |

## Rig, masking, and compatibility evidence

| Pack/rig/set pair | Skeleton/retarget | Scale/axes | Root policy | Timing/blend | Overall evidence |
|---|---|---|---|---|---|
| Two-Handed Melee internal gait sets | Current signatures measured; no retarget | No engine scale/axis test | IP/RM files measured; owner not selected | Current duration/speed/phase recorded; full blend absent | Mechanical only |
| Two Handed Melee plus Basic Locomotion | Named Humanoid sources imported on shared `SK_Protof-Actor` | Unity import scale/axis accepted for named sources only | Gameplay-owned root fixed for IP/idle; animation-owned root finite for tested RM where applicable | Named Playables transitions finite; visual blend not inspected | Bounded source/controller feasibility, not compatibility acceptance |

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

Preflight verified version/help for required commands and representative GLB/FBX admission before replay; scrubbed locator `external:0.14.0-report-refresh/preflight.json`, SHA-256 `7034b5043de54e7b30064230db18605d1f96d0ea1477f7b8c56ce817cc73521a`. The read-only helper replayed the retained exact `inspect`, `measure`, and two `lint` invocations with four workers, recorded argv/exit/stdout/stderr and source/config hashes, and did not mutate inputs. Baseline, contract, and remediation ledger digests are `ddb5a9e2e0cdc97bd4b768fcf8700da32f4a22358babd50a101416a14c9737d5`, `0438c30a7e86b46ca0be07b02baf0e7145a22125fc6f6f8f8eacfef2047a5267`, and `0f15f4171bf799b5e4bef91251f94dbfaaed8d6f4341a59b05be52f8e1b61430`. Licensed paths, sources, and derivative outputs are excluded from Git and this report.

Fresh Unity evidence: editor `6000.5.8f1`, `Unity.exe` SHA-256 `cdc0eeca135394cde79a632aa998aca14b521b58afe8c2750b009aff608be906`; probe SHA-256 `28cd00897dfd739e2a7bcfa193adab03de3591a1009802946305127b67d1bfb6`; result schema `animsmith-unity-crosspack-probe-v1`, result SHA-256 `f067bca91f182b65fab408a1c97b297377a8f361b03d59e809e236aba666652c`; log SHA-256 `0429bcb9cc7a7492c6637eb799566463ba915243d0a110b4d76cbe75590c8819`; 26/26 aggregate required checks passed with 53 nonempty Humanoid bones per sample. This report uses only the named applicable subset above.

## Sources

- Protofactor product context and [End User License Agreement](https://protofactor.biz/end-user-license-agreement/) — context only; local revision remains unknown.
