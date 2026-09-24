# Animation pack evaluation: Protofactor Basic Locomotion Animset

> Technical verdict: **Usable with conditions**
>
> Evaluation completeness: **partial** — all source files and declared checks were reviewed, and a limited Unity source import/mixer ran; gameplay and visual quality remain untested.
>
> Confidence: **medium**
>
> Evaluation date: **2026-09-16**
>
> Current evaluator: **AnimSmith 0.14.0**
>
> Report format: **3**
>
> Detailed evidence: [Protofactor Basic Locomotion evidence](protofactor-basic-locomotion-evidence.md)

## Technical decision

**Prototype decision:** Build separate eight-direction walk, run and crouch blends using the in-place motions, with the controller moving the character. Review loop and foot-contact timing before relying on blended travel. The phase-alignment trial still needs in-game checks.

**Camera scope:** Full-body humanoid animations; appearance on your target character and camera is untested. Dedicated first-person arms/viewmodel use is unverified.

This animset is one of eight locally evaluated parts of [Ultimate Animation Collection](protofactor-ultimate-animation-collection.md). The local asset revision is unknown; the collection report separates evaluated files from the dated vendor listing.

**Content in evaluated inventory:** Ground movement: walk, run, crouch. Cover, turns, jumps and throws appear in evaluated filenames; starts/stops and transitions are not classified into accepted sets. These are identified motions; gameplay behavior has not been approved.

Keep cover, jumps, throws and turns outside the three gait blends; each needs its own state and movement rules. Equal duration within a direction set helps synchronization but does not prove matching foot contacts or smooth crossfades.

`observed-animsmith`: the official 0.14.0 evaluator inspected and measured all 179 FBXs; 58/177 declared-contract files pass and 119 fail per output format. The findings below separate source problems, project settings and untested gameplay. Transformed outputs still need gameplay and visual checks.

## Capability coverage

### Content present

Filename-classified candidates include three complete eight-direction in-place walk, run and crouch rings. The wider evaluated inventory also names cover, turn, jump and throw motions; these are not members of the three selected gait rings.

**Other source motions worth inspecting:** The measured inventory includes `Humanoid@SprintForwardUnarmed.FBX`, `Humanoid@SprintForwardLeftUnarmed.fbx`, `Humanoid@SprintForwardRightUnarmed.fbx`, `Humanoid@RunFastForwardUnarmed.fbx`, `Humanoid@RunFastTurnLeftUnarmed.fbx`, `Humanoid@RunFastTurnRightUnarmed.fbx`, `Humanoid@RunUTurnLeftUnarmed.fbx`, `Humanoid@RunUTurnRightUnarmed.fbx`, and `Humanoid@Turn90LeftUnarmed.fbx` / `Humanoid@Turn90RightUnarmed.fbx` / `Humanoid@Turn180LeftUnarmed.fbx` / `Humanoid@Turn180RightUnarmed.fbx` (plus listed root-motion variants). These are [filename candidates in the retained baseline](protofactor-basic-locomotion-evidence.md#mechanical-baseline), outside the three selected gait rings. Sprint speed, planted turns, start/stop bindings and controller behavior remain unclassified and untested.

### Content gaps and unknowns

Start/stop sequences and foot-contact markers are not classified into accepted runtime sets here. Their status is an evaluation gap, not a claim that the delivered pack lacks them.

### Evaluation still needed

Test loop/contact correspondence, starts/stops, reversals, crouch changes and target-character playback. The bounded Unity source probe does not accept a controller, visual quality or first-person presentation.

## Runtime sets and authored motion

Use each row as a separate blend or sequence proposal. The linked appendix names its exact clips, timings and measurements; controller playback and contact quality still need testing.

| Set | Controller use | Adoption decision | Exact members |
|---|---|---|---|
| Walk eight directions | Eight-direction controller gait | Prototype with controller travel. Loop/contact policy needs review; phase-alignment output still needs in-game checks (22/24 candidates retain lint across all three rings). | [8 exact members](protofactor-basic-locomotion-evidence.md#exact-runtime-members) |
| Run eight directions | Eight-direction controller gait | Prototype with controller travel. Verify faster contact timing; phase-alignment output still needs in-game checks (22/24 candidates retain lint across all three rings). | [8 exact members](protofactor-basic-locomotion-evidence.md#exact-runtime-members) |
| Crouch eight directions | Eight-direction controller gait | Prototype with controller travel. Configure stance changes; phase-alignment output still needs in-game checks (22/24 candidates retain lint across all three rings). | [8 exact members](protofactor-basic-locomotion-evidence.md#exact-runtime-members) |

## Integration recipe

1. **Members/topology:** Create separate walk, run, and crouch two-parameter trees from the appendix members. Use local horizontal velocity for direction; handle standing/crouching and cover as states.
2. **Timing/synchronization:** Establish the project or vendor loop/contact policy before choosing synchronized blending. The phase-alignment trial reduces measured phase spread; test the declared support-foot correspondence, loop closure, and intermediate weights before adoption.
3. **State ownership:** The selected in-place roots have no measured travel. Let the kinematic controller own translation/collision and choose playback speed from observed stride/contact tests; zero root speed is not a usable speed threshold. Give root-motion actions an explicit separate movement policy.
4. **Composition constraints:** Preserve authored lower-body/torso coupling initially. Weapon layers need an explicit spine mask, reference-pose convention, and planted-foot test.
5. **Acceptance gate:** Test stops, reversals, diagonal transitions, slopes, and crouch changes on the target character. Source mixer execution does not validate exported candidates.

## Technical issue register

The phase spreads below measure how far the selected gait cycles differ; they do not establish matching support feet or visual blending. The [evidence appendix](protofactor-basic-locomotion-evidence.md) gives the calculation and source phases.

Severity describes impact as delivered for the stated use. Residual status is explicit; untested risks are not confirmed artist defects.

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| BL-TIME | major | Twelve cover/throw files contain 36 negative-time findings; an importer can trim or interpret their pre-roll differently. [Readiness guidance](../game-ready-clips.md#the-readiness-ladder). | artist-author | Re-export the intended start/range or review the twelve slice trials. Slicing clears all 36 time findings; ten outputs pass their full declared checks and two still have other findings. Source files are unchanged. Check contacts and engine playback before using any output. | Explicit range slicing exists; it does not recover missing artistic intent. | Baseline affected-file list and slice verification; `observed-animsmith`. |
| BL-PHASE | note | Evaluator-selected walk/run/crouch rings have phase spreads 0.6598/0.4630/0.7156 cycles. Their contact correspondence and intended synchronization are unconfirmed; the numbers alone do not establish an artist defect. [Readiness guidance](../game-ready-clips.md#the-readiness-ladder). | unknown | Obtain project or vendor loop/contact intent, then test a declared phase/contact tolerance over the exact rings. Anchored spreads are 0.0724/0.0938/0.0502 cycles in the same order, but 22 of 24 candidates retain lint findings. Request artist cleanup only if accepted intent and playback establish a source-motion problem. Residual adoption unresolved; no output approved for gameplay use. | Current declared gait anchoring reduces measured phase disagreement; it cannot choose intended contacts or approve blending. | Exact sets and source/output phases `observed-animsmith`; synchronization/contact acceptance `not-evaluated`. |
| BL-ROOT | major | Fourteen files fail the declared in-place policy; a controller that also moves the character may double-apply displacement. [Readiness guidance](../game-ready-clips.md#the-readiness-ladder). | engine-config | Choose who moves the character in each state, then inspect the fourteen measured trajectories and test displacement/collision. A filename without an RM suffix does not prove a stationary root. Unresolved. | Root measurements identify the conflict; the project must choose movement ownership. | Declared contract ledger lists exact files; `observed-animsmith`. |
| BL-TRACK | minor | Constant tracks inflate storage; note counts alone do not establish runtime cost. [Readiness guidance](../game-ready-clips.md#the-readiness-ladder). | animsmith-current-declared | Three pruning outputs pass the original checks. Compare required tracks, masks and engine playback before using them; equivalence is unverified. | Current pruning is available; no measured frame-time improvement is claimed. | Three representative outputs; `observed-animsmith`. |

## Engine status

| Runtime | Evidence level | Technical result | Remaining gate |
|---|---|---|---|
| Unity 6000.5.8f1 | observed-engine | Source inventory: 177 Humanoid clips; 6/6 sample and 3/3 representative mixer executions; the run mixer used `RunForwardUnarmed.FBX`, not the selected ring’s `RunForward2Unarmed.fbx`. A separate named cross-pack subset has finite-pose/fixed-root assertions (26 total checks across five packs). | Inventory probe is execution-only and does not validate the complete selected rings; cross-pack probe adds finite poses, not visual/contact quality. Controller, candidate-output and build acceptance remain open. |
| Unreal Engine unspecified | not-evaluated | No current import or playback run. | Import, retarget, graph, and build tests. |
| Godot unspecified | not-evaluated | No current conversion, import, or playback run. | Conversion/import, graph, and export tests. |
| Bevy unspecified | not-evaluated | No current glTF handoff or runtime run. | Conversion, addressability, runtime, and performance tests. |

## Fit and limitations

Basic + Injured is a plausible state switch between healthy and injured locomotion; preserve each injury style and calibrate speed separately. Basic + melee packs is a plausible full-body armed/unarmed state change, with stance and grip continuity still untested. Basic + Climbing/Campfire needs an explicit exit from ground locomotion, environment alignment, and a return state. Fresh Unity checks support the named Basic-to-melee idle/walk combinations as technically executable with a fixed owner root; see the [collection appendix](protofactor-ultimate-animation-collection-evidence.md#rig-masking-and-compatibility-evidence). They do not approve unrestricted blending or upper-body overlays.

These are proposed integration decisions (`inferred`), with target-controller, contact and artistic acceptance still `not-evaluated`.

## Changes between AnimSmith versions

AnimSmith 0.14.0 — reran 179 input baselines, 177 per-file declared contracts and 39 remediation trials using the official release. Added current exact-member generic scenarios and developer/artist actions. AnimSmith 0.10.0 — superseded historical evidence; none of its generated outputs or engine results is relabelled as a fresh run.

## Evidence status

Current evidence uses the official 0.14.0 binary and the [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder). Commercial sources and derivatives remain external.

## Sources

- Protofactor, [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/) — collection-level product context.
- AnimSmith, [game-ready clips](../game-ready-clips.md) and [CLI reference](../cli.md) — readiness and command boundaries.
- Unity 6 documentation: [Blend Trees](https://docs.unity3d.com/6000.0/Documentation/Manual/class-BlendTree.html), [Root Motion](https://docs.unity3d.com/6000.0/Documentation/Manual/RootMotion.html), and [Animation Layers](https://docs.unity3d.com/6000.0/Documentation/Manual/AnimationLayers.html) — engine concepts only, not evidence of pack quality.
