# Animation pack evidence appendix: Protofactor Sword and Shield Animset

> Companion report: [technical evaluation](protofactor-sword-and-shield.md)
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
| Delivered scope | 136 retained FBX candidates; source bytes unchanged during replay |
| Target use | Broad game-engine technical intake; no target controller supplied |
| Target engines | Unity 6000.5.8f1 bounded source/controller probe; Unreal, Godot, and Bevy not evaluated |
| Target rigs/packs | Delivered rigs; Basic Locomotion compatibility not freshly evaluated |
| Source manifest | `external:protofactor-sword-and-shield/source-inventory.json`; SHA-256 `e4dc4740bf35ff2812e81ff78970fc6737e62e8022664643c14b5cb8fdf2e4b8` |
| Evaluation manifest | Retained `urn:animsmith:skill:animation-pack-evaluation-manifest:1`; current memberships treated as hypotheses and remeasured |
| Acquisition/license provenance | Authorized local commercial input; no legal conclusion |

### Evidence coverage

| Surface | Offered/delivered | Evaluated | Findings | Not evaluated and why |
|---|---:|---:|---:|---|
| Animation files | 136 | 136 | 134 animation-bearing | Engine semantics not implied |
| Rigs/export variants | 136 | 136 | 131 individual-motion files share the 56-bone `2b6fe49d5ae6` skeleton; `Humanoid@CrouchForwardRightS&S_RM.fbx` exposes only a 2-bone `646f12b4559c` skeleton | Retarget/reference-pose behavior not run |
| AnimSmith baseline | 136 | 136 | All inspect/measure/two lint commands exit 0; One scale-keys warning on `Protof-Actor@Sword&ShieldAnimset.fbx`. | N/A |
| Declared contracts | 132 | 132 | 17 pass, 115 fail in each format | Declaration intent needs project/vendor authority |
| Offline visual reports | 0 | 0 | 0 | Not run |
| Engine import/playback | 1 named Unity sources | 1 | Fresh Humanoid import and finite standalone sampling passed | Visual/contact, authored-controller, and player-build behavior not evaluated |
| Blend/mask/retarget | Three named source Playables mixers | Three fixed-root schedules | Finite sampled poses for named cross-pack pairs | No full blend tree, mask, separate target-rig retarget, visual or contact acceptance |

### Claim legend

`observed-file` identifies source structure, `observed-animsmith` identifies current tool output, `inferred` identifies bounded hypotheses, and `not-evaluated` marks open gates.

## Evaluation manifest and taxonomy

Current evaluation manifest schema: `urn:animsmith:skill:animation-pack-evaluation-manifest:1`.

### Canonical clip-role inventory

| Canonical primary role | Logical motions | Delivered files | Evidence boundary |
|---|---:|---:|---|
| `idle-pose` | 6 | 6 | `observed-file`; evaluator classification, not vendor semantics |
| `continuous-locomotion` | 56 | 56 | `observed-file`; evaluator classification, not vendor semantics |
| `locomotion-transition` | 0 | 0 | `observed-file`; evaluator classification, not vendor semantics |
| `airborne` | 0 | 0 | `observed-file`; evaluator classification, not vendor semantics |
| `traversal` | 0 | 0 | `observed-file`; evaluator classification, not vendor semantics |
| `action-interaction` | 46 | 46 | `observed-file`; evaluator classification, not vendor semantics |
| `reaction-death` | 24 | 24 | `observed-file`; evaluator classification, not vendor semantics |
| `emote-cinematic` | 0 | 0 | `observed-file`; evaluator classification, not vendor semantics |
| `other-unknown` | 0 | 0 | `observed-file`; evaluator classification, not vendor semantics |
| **Total** | **132** | **132** | Current exact-file catalog |

### Runtime-set inventory

The untested acceptance below applies to each complete set. [Named Unity source
samples and mixers](#engine-procedures-and-evidence) ran for a smaller subset;
they do not establish full-set playback or visual quality.

| Runtime set | Type | Members/variants | Grouping evidence | Validation status |
|---|---|---|---|---|
| `walk-combat-8-way-in-place` | directional-blend | 8 exact files; 8 IP, 0 RM | Current exact-file measurements; grouping is an evaluator-defined hypothesis seeded by retained catalog declarations | 0/8 files pass the retained declaration; whole-set engine and visual acceptance not evaluated |
| `walk-combat-8-way-root-motion` | directional-blend | 8 exact files; 0 IP, 8 RM | Current exact-file measurements; grouping is an evaluator-defined hypothesis seeded by retained catalog declarations | 0/8 files pass the retained declaration; whole-set engine and visual acceptance not evaluated |
| `run-combat-8-way-in-place` | directional-blend | 8 exact files; 8 IP, 0 RM | Current exact-file measurements; grouping is an evaluator-defined hypothesis seeded by retained catalog declarations | 0/8 files pass the retained declaration; whole-set engine and visual acceptance not evaluated |
| `run-combat-8-way-root-motion` | directional-blend | 8 exact files; 0 IP, 8 RM | Current exact-file measurements; grouping is an evaluator-defined hypothesis seeded by retained catalog declarations | 0/8 files pass the retained declaration; whole-set engine and visual acceptance not evaluated |
| `crouch-combat-8-way-in-place` | directional-blend | 8 exact files; 8 IP, 0 RM | Current exact-file measurements; grouping is an evaluator-defined hypothesis seeded by retained catalog declarations | 0/8 files pass the retained declaration; whole-set engine and visual acceptance not evaluated |
| `crouch-combat-8-way-root-motion` | directional-blend | 8 exact files; 0 IP, 8 RM | Current exact-file measurements; grouping is an evaluator-defined hypothesis seeded by retained catalog declarations | 0/8 files pass the retained declaration; whole-set engine and visual acceptance not evaluated |
| `walk-normal-speed-in-place` | speed-blend | 2 exact files; 2 IP, 0 RM | Current exact-file measurements; grouping is an evaluator-defined hypothesis seeded by retained catalog declarations | 0/2 files pass the retained declaration; whole-set engine and visual acceptance not evaluated |
| `walk-normal-speed-root-motion` | speed-blend | 2 exact files; 0 IP, 2 RM | Current exact-file measurements; grouping is an evaluator-defined hypothesis seeded by retained catalog declarations | 0/2 files pass the retained declaration; whole-set engine and visual acceptance not evaluated |
| `run-normal-fast-in-place` | speed-blend | 2 exact files; 2 IP, 0 RM | Current exact-file measurements; grouping is an evaluator-defined hypothesis seeded by retained catalog declarations | 0/2 files pass the retained declaration; whole-set engine and visual acceptance not evaluated |
| `run-normal-fast-root-motion` | speed-blend | 2 exact files; 0 IP, 2 RM | Current exact-file measurements; grouping is an evaluator-defined hypothesis seeded by retained catalog declarations | 0/2 files pass the retained declaration; whole-set engine and visual acceptance not evaluated |
| `death-downed-recover-front` | transition-chain | 3 exact files; 3 IP, 0 RM | Current exact-file measurements; grouping is an evaluator-defined hypothesis seeded by retained catalog declarations | 3/3 files pass the retained declaration; whole-set engine and visual acceptance not evaluated |
| `death-downed-recover-back` | transition-chain | 3 exact files; 3 IP, 0 RM | Current exact-file measurements; grouping is an evaluator-defined hypothesis seeded by retained catalog declarations | 3/3 files pass the retained declaration; whole-set engine and visual acceptance not evaluated |
| `death-downed-recover-left` | transition-chain | 3 exact files; 3 IP, 0 RM | Current exact-file measurements; grouping is an evaluator-defined hypothesis seeded by retained catalog declarations | 3/3 files pass the retained declaration; whole-set engine and visual acceptance not evaluated |
| `death-downed-recover-right` | transition-chain | 3 exact files; 3 IP, 0 RM | Current exact-file measurements; grouping is an evaluator-defined hypothesis seeded by retained catalog declarations | 3/3 files pass the retained declaration; whole-set engine and visual acceptance not evaluated |
| `draw-combat-put-away-1` | transition-chain | 3 exact files; 3 IP, 0 RM | Current exact-file measurements; grouping is an evaluator-defined hypothesis seeded by retained catalog declarations | 2/3 files pass the retained declaration; whole-set engine and visual acceptance not evaluated |
| `draw-combat-put-away-2` | transition-chain | 3 exact files; 3 IP, 0 RM | Current exact-file measurements; grouping is an evaluator-defined hypothesis seeded by retained catalog declarations | 2/3 files pass the retained declaration; whole-set engine and visual acceptance not evaluated |

The [exact runtime members](#exact-runtime-members) below record source members and fresh timing/motion for the decision-driving gait sets. No historical membership is promoted solely from prose or filenames.

### Exact runtime members

Current source members and measurements for the selected runtime scenarios. Selection is an evaluator hypothesis; source identity and measured values are retained below.

For these multi-member gait rows, duration and RM speed cells are set minima, not per-clip values. The ranges below retain authored variation; individual speed assignments are not reproduced here.

| Set/profile | Role or coordinate | Exact members | Variant/type | Timing or motion | Runtime contract |
|---|---|---|---|---|---|
| `walk-combat-8-way-in-place` | eight-way directional gait | `Humanoid@WalkForwardS&S.fbx`; `Humanoid@WalkForwardLeftS&S.fbx`; `Humanoid@WalkLeftS&S.fbx`; `Humanoid@WalkBackwardsLeftS&S.fbx`; `Humanoid@WalkBackwardsS&S.fbx`; `Humanoid@WalkBackwardsRightS&S.fbx`; `Humanoid@WalkRightS&S.fbx`; `Humanoid@WalkForwardRightS&S.fbx` | variant=in-place | duration=1.333 s | loop=true; sync=not-evaluated |
| `walk-combat-8-way-root-motion` | eight-way directional gait | `Humanoid@WalkForwardS&S_RM.fbx`; `Humanoid@WalkForwardLeftS&S_RM.fbx`; `Humanoid@WalkLeftS&S_RM.fbx`; `Humanoid@WalkBackwardsLeftS&S_RM.fbx`; `Humanoid@WalkBackwardsS&S_RM.fbx`; `Humanoid@WalkBackwardsRightS&S_RM.fbx`; `Humanoid@WalkRightS&S_RM.fbx`; `Humanoid@WalkForwardRightS&S_RM.fbx` | variant=root-motion | duration=1.333 s; rm_speed=0.750 m/s | loop=true; sync=not-evaluated |
| `run-combat-8-way-in-place` | eight-way directional gait | `Humanoid@RunForwardS&S.fbx`; `Humanoid@RunForwardLeftS&S.fbx`; `Humanoid@RunLeftS&S.fbx`; `Humanoid@RunBackwardsLeftS&S.fbx`; `Humanoid@RunBackwardsS&S.fbx`; `Humanoid@RunBackwardsRightS&S.fbx`; `Humanoid@RunRightS&S.fbx`; `Humanoid@RunForwardRightS&S.fbx` | variant=in-place | duration=0.600 s | loop=true; sync=not-evaluated |
| `run-combat-8-way-root-motion` | eight-way directional gait | `Humanoid@RunForwardS&S_RM.fbx`; `Humanoid@RunForwardLeftS&S_RM.fbx`; `Humanoid@RunLeftS&S_RM.fbx`; `Humanoid@RunBackwardsLeftS&S_RM.fbx`; `Humanoid@RunBackwardsS&S_RM.fbx`; `Humanoid@RunBackwardsRightS&S_RM.fbx`; `Humanoid@RunRightS&S_RM.fbx`; `Humanoid@RunForwardRightS&S_RM.fbx` | variant=root-motion | duration=0.600 s; rm_speed=2.793 m/s | loop=true; sync=not-evaluated |
| `crouch-combat-8-way-in-place` | eight-way directional gait | `Humanoid@CrouchForwardS&S.fbx`; `Humanoid@CrouchForwardLeftS&S.fbx`; `Humanoid@CrouchLeftS&S.fbx`; `Humanoid@CrouchBackwardsLeftS&S.fbx`; `Humanoid@CrouchBackwardsS&S.fbx`; `Humanoid@CrouchBackwardsRightS&S.fbx`; `Humanoid@CrouchRightS&S.fbx`; `Humanoid@CrouchForwardRightS&S.fbx` | variant=in-place | duration=1.667 s | loop=true; sync=not-evaluated |
| `crouch-combat-8-way-root-motion` | eight-way directional gait | `Humanoid@CrouchForwardS&S_RM.fbx`; `Humanoid@CrouchForwardLeftS&S_RM.fbx`; `Humanoid@CrouchLeftS&S_RM.fbx`; `Humanoid@CrouchBackwardsLeftS&S_RM.fbx`; `Humanoid@CrouchBackwardsS&S_RM.fbx`; `Humanoid@CrouchBackwardsRightS&S_RM.fbx`; `Humanoid@CrouchRightS&S_RM.fbx`; `Humanoid@CrouchForwardRightS&S_RM.fbx` | variant=root-motion | duration=1.667 s; rm_speed=0.701 m/s | loop=true; sync=not-evaluated |

Measured RM speed spans are walk 0.750–1.107 m/s (ratio 0.677); run 2.793–3.189 m/s (ratio 0.876); crouch 0.701–0.789 m/s (ratio 0.889). Preserve authored variation unless the project declares a normalization policy; diagonals and cardinals require the full blend test. Measured source in-place phase spreads (cycles, eight measured members each) are `walk-combat-8-way-in-place` 0.7231; `run-combat-8-way-in-place` 0.6605; `crouch-combat-8-way-in-place` 0.6974. Phase spread here is the minimum covering arc in cycles: sort phases in `[0,1)`, include the wraparound gap, then subtract the largest gap from 1. It is not `max_circular_deviation_from_mean`; neither measure alone proves support-foot or visual compatibility. Keep `sync=not-evaluated` until the selected synchronization and contact policy is tested. The current catalog also measures paired normal/fast speed hypotheses, four death/downed/recovery chains, and two draw/combat/put-away chains. The transition chains are mechanically stronger than the gait sets, but still lack engine and visual acceptance.

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
| Root-motion controller | `selected` — `observed-pack-capability` | RM variants measured mechanically; no Sword & Shield root-motion walk was sampled standalone in Unity, and controller/collision ownership remains untested. |
| State-machine transitions | `selected` — `observed-pack-capability` | Transition candidates identified; named source Playables schedules ran, but authored state transitions and interruptions were not tested. |
| Layered upper body/weapons | `not-selected` | No mask/socket/IK authority or test. |
| Traversal/environment | `not-selected` | No target scene or traversal contacts. |
| Contact actions/interactions | `selected` — `observed-pack-capability` | Combat files present; weapon/body contacts not evaluated. |
| Retargeted/customizable characters | `not-selected` | No target rig or reference-pose test. |
| Motion matching/search | `not-selected` | No database construction or query test. |
| Networked movement | `not-selected` | No networking or reconciliation design. |
| Runtime performance | `not-selected` | No target build or benchmark. |

## Pack inventory and content evidence

The current inventory contains 136 FBXs: 134 animation-bearing files, 132 individual-motion files, and 1 combined take. 131 individual-motion files share the 56-bone `2b6fe49d5ae6` skeleton; `Humanoid@CrouchForwardRightS&S_RM.fbx` exposes only a 2-bone `646f12b4559c` skeleton. Counts describe physical current inputs, not runtime acceptance.

### Current in-place gait phase evidence

Phase spread here is the minimum covering arc in cycles: sort phases in `[0,1)`, include the wraparound gap, then subtract the largest gap from 1. It is not `max_circular_deviation_from_mean`; neither measure alone proves support-foot or visual compatibility. These are `observed-animsmith` source measurements for the exact hypotheses above; synchronization/contact acceptance remains open.

| Current hypothesis | Exact source member (`Take 001`) | Measured phase (cycles) |
|---|---|---|
| `walk-combat-8-way-in-place` | `Humanoid@WalkForwardS&S.fbx` | 0.149832 |
| `walk-combat-8-way-in-place` | `Humanoid@WalkForwardLeftS&S.fbx` | 0.111639 |
| `walk-combat-8-way-in-place` | `Humanoid@WalkLeftS&S.fbx` | 0.378087 |
| `walk-combat-8-way-in-place` | `Humanoid@WalkBackwardsLeftS&S.fbx` | 0.800623 |
| `walk-combat-8-way-in-place` | `Humanoid@WalkBackwardsS&S.fbx` | 0.839751 |
| `walk-combat-8-way-in-place` | `Humanoid@WalkBackwardsRightS&S.fbx` | 0.881416 |
| `walk-combat-8-way-in-place` | `Humanoid@WalkRightS&S.fbx` | 0.677664 |
| `walk-combat-8-way-in-place` | `Humanoid@WalkForwardRightS&S.fbx` | 0.654982 |
| `run-combat-8-way-in-place` | `Humanoid@RunForwardS&S.fbx` | 0.165717 |
| `run-combat-8-way-in-place` | `Humanoid@RunForwardLeftS&S.fbx` | 0.201005 |
| `run-combat-8-way-in-place` | `Humanoid@RunLeftS&S.fbx` | 0.276155 |
| `run-combat-8-way-in-place` | `Humanoid@RunBackwardsLeftS&S.fbx` | 0.781110 |
| `run-combat-8-way-in-place` | `Humanoid@RunBackwardsS&S.fbx` | 0.864140 |
| `run-combat-8-way-in-place` | `Humanoid@RunBackwardsRightS&S.fbx` | 0.786420 |
| `run-combat-8-way-in-place` | `Humanoid@RunRightS&S.fbx` | 0.615651 |
| `run-combat-8-way-in-place` | `Humanoid@RunForwardRightS&S.fbx` | 0.138593 |
| `crouch-combat-8-way-in-place` | `Humanoid@CrouchForwardS&S.fbx` | 0.087834 |
| `crouch-combat-8-way-in-place` | `Humanoid@CrouchForwardLeftS&S.fbx` | 0.083958 |
| `crouch-combat-8-way-in-place` | `Humanoid@CrouchLeftS&S.fbx` | 0.375552 |
| `crouch-combat-8-way-in-place` | `Humanoid@CrouchBackwardsLeftS&S.fbx` | 0.878684 |
| `crouch-combat-8-way-in-place` | `Humanoid@CrouchBackwardsS&S.fbx` | 0.891297 |
| `crouch-combat-8-way-in-place` | `Humanoid@CrouchBackwardsRightS&S.fbx` | 0.885628 |
| `crouch-combat-8-way-in-place` | `Humanoid@CrouchRightS&S.fbx` | 0.576121 |
| `crouch-combat-8-way-in-place` | `Humanoid@CrouchForwardRightS&S.fbx` | 0.124471 |

## Mechanical baseline

| Finding/check | Affected scope | Potential impact | Evidence |
|---|---|---|---|
| Current readability | 136/136 inputs | Enables mechanical intake | `observed-animsmith`; all four commands per input exit 0 |
| Constant tracks | 17078 notes across 134 animation-bearing files | Storage/evaluation noise; not a gameplay defect by itself | `observed-animsmith`; baseline ledger SHA-256 `9148c23bc266d269cf69f8c1abaf7dc6bdd00ee456d3acc4d53930a3e6893ac7` |
| Retained declarations | 115/132 non-clean | Blocks admission under those exact declarations | `observed-animsmith`; contract ledger SHA-256 `f3256aed915183c2a3923e75da1fad0a76842f472e7283c57bc9d4a43f1d2ead` |
| Structural/timing warnings | Named scope above | Requires bounded author/project decision | `observed-file` and `observed-animsmith` |

## AnimSmith remediation evidence

| Source issue | Operation/declarations | Result | Independent verification | Remaining caveat |
|---|---|---|---|---|
| `PFS-01` gait continuity | `transform --gait-anchor` on 24 selected in-place gait files; retained per-file configs | 24 outputs produced; selected inputs have no accumulating root translation/yaw | 24 inspect and measure commands exit 0; output hashes and diffs captured | Post-config lint remains non-clean; no RM trajectory was transformed; visual/contact residual unresolved |
| Constant-track notes | `transform --prune-constant-tracks` on `Humanoid@SwordAttack1S&S.fbx`, `Humanoid@WalkForwardS&S.fbx`, and `Protof-Actor@Sword&ShieldAnimset.fbx` | Output candidate(s) produced | Inspect, measure, diff, fix dry-run, and config lint captured | 27/28 post-transform config lints exit 1; only the combined-take prune exits 0. Candidate adoption and engine/visual acceptance remain open |
| `PFS-01` endpoint trial | `transform --drop-duplicate-loop-endpoint` on `Humanoid@WalkForwardS&S.fbx` | Output produced | Inspect/measure succeed; diff and config lint remain non-clean | Unadopted; visual loop/contact residual unresolved |

Fresh remediation ledger SHA-256 `bace6c9618d4b4a1ea84b1300a7da0b15aefc0d88421de5e56a7551a8b5cb869`. Generated motion stays outside Git.

## Engine procedures and evidence

| Runtime | Version | Procedure | Observed result | Remaining gate |
|---|---|---|---|---|
| Unity | 6000.5.8f1 | Fresh named Humanoid source import and standalone sampling: IdleCombatS&S; named Playables mixers: sword-shield-idle-to-onehand-idle, sword-shield-idle-to-twohand-idle, sword-shield-idle-to-dual-idle. See [collection source/controller probe](protofactor-ultimate-animation-collection-evidence.md#rig-masking-and-compatibility-evidence). | Bounded source/controller feasibility for listed sources only; the collection’s 26/26 is a shared aggregate, not this pack’s result | Rendered motion, contact, deformation, full blend, retargeting, authored AnimatorController, player build and performance remain untested |
| Unreal Engine | not evaluated | No current procedure executed | `not-evaluated` | Import, retarget, root, blend, and visual review |
| Godot | not evaluated | No current procedure executed | `not-evaluated` | Convert/import, graph, root, contact, and visual review |
| Bevy | not evaluated | No current procedure executed | `not-evaluated` | Convert/load, graph, root, performance, and visual review |

## Rig, masking, and compatibility evidence

| Pack/rig/set pair | Skeleton/retarget | Scale/axes | Root policy | Timing/blend | Overall evidence |
|---|---|---|---|---|---|
| Sword and Shield internal gait sets | Current signatures measured; no retarget | No engine scale/axis test | IP/RM files measured; owner not selected | Current duration/speed/phase recorded; full blend absent | Mechanical only |
| Sword & Shield idle to One-Handed Melee, Two-Handed Melee, and Dual Swords idles | Named Humanoid sources imported on shared `SK_Protof-Actor` | Unity import scale/axis accepted for named sources only | Gameplay-owned root fixed for these idle mixers; no Shield RM sample | Three named Playables mixers produced finite poses; visual blend not inspected | Bounded source feasibility for these pairs only; no Basic-to-Shield handoff test |

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

Preflight verified version/help for required commands and representative GLB/FBX admission before replay; scrubbed locator `external:0.14.0-report-refresh/preflight.json`, SHA-256 `7034b5043de54e7b30064230db18605d1f96d0ea1477f7b8c56ce817cc73521a`. The read-only helper replayed the retained exact `inspect`, `measure`, and two `lint` invocations with four workers, recorded argv/exit/stdout/stderr and source/config hashes, and did not mutate inputs. Baseline, contract, and remediation ledger digests are `9148c23bc266d269cf69f8c1abaf7dc6bdd00ee456d3acc4d53930a3e6893ac7`, `f3256aed915183c2a3923e75da1fad0a76842f472e7283c57bc9d4a43f1d2ead`, and `bace6c9618d4b4a1ea84b1300a7da0b15aefc0d88421de5e56a7551a8b5cb869`. Licensed paths, sources, and derivative outputs are excluded from Git and this report.

Fresh Unity evidence: editor `6000.5.8f1`, `Unity.exe` SHA-256 `cdc0eeca135394cde79a632aa998aca14b521b58afe8c2750b009aff608be906`; probe SHA-256 `28cd00897dfd739e2a7bcfa193adab03de3591a1009802946305127b67d1bfb6`; result schema `animsmith-unity-crosspack-probe-v1`, result SHA-256 `f067bca91f182b65fab408a1c97b297377a8f361b03d59e809e236aba666652c`; log SHA-256 `0429bcb9cc7a7492c6637eb799566463ba915243d0a110b4d76cbe75590c8819`; 26/26 aggregate required checks passed with 53 nonempty Humanoid bones per sample. This report uses only the named applicable subset above.

## Sources

- Protofactor product context and [End User License Agreement](https://protofactor.biz/end-user-license-agreement/) — context only; local revision remains unknown.
