# Animation pack evaluation: Protofactor Basic Locomotion Animset

> Technical verdict: **Usable with conditions**
>
> Evaluation completeness: **partial** — current mechanical and declared-contract evidence is exhaustive, with a fresh bounded Unity source import/mixer probe; visual and gameplay acceptance remain open.
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

**Use and evidence boundary:** Full-body humanoid content for a third-person prototype context. Target-character and gameplay-camera appearance have not been accepted; dedicated first-person arms/viewmodel suitability is not established.

The 2026-09-17 vendor listing places this animset in Ultimate Animation Collection, which advertises 24 animsets. This report evaluates one of eight locally evaluated constituents; sixteen advertised constituents, including Female Basic Locomotion, were not evaluated. The local asset revision is unknown, so current listing membership is a scope reference, not proof of local contents.

**Content in evaluated inventory:** Ground movement: walk, run, crouch. Cover, turns, jumps and throws appear in evaluated filenames; starts/stops and transitions are not classified into accepted sets. This describes candidate content, not accepted gameplay behavior.

**Adoption route:** Use the three in-place rings to prototype controller-owned travel. Configure root ownership and loop/contact policy; the declared gait-anchor trial is an AnimSmith mechanical candidate, not an adopted output.

**For a kinematic character: a useful locomotion foundation, conditional on phase and loop cleanup.** Start with the three in-place direction sets below. Do not put every delivered movement into one blend tree: cover, jumps, throws, and turns need state-specific selection and ownership. Equal duration within each ring helps synchronization but does not prove foot contacts or crossfades.

`observed-animsmith`: the official 0.14.0 evaluator inspected and measured all 179 FBXs; 58/177 declared-contract files pass and 119 fail per output format. Conditions below distinguish source findings, evaluator policy and untested gameplay. No remediation candidate is promoted.

## Capability coverage

### Content present

Filename-classified candidates include three complete eight-direction in-place walk, run and crouch rings. The wider evaluated inventory also names cover, turn, jump and throw motions; these are not members of the three selected gait rings.

### Content gaps and unknowns

Start/stop sequences and foot-contact markers are not classified into accepted runtime sets here. Their status is an evaluation gap, not a claim that the delivered pack lacks them.

### Evaluation still needed

Test loop/contact correspondence, starts/stops, reversals, crouch changes and target-character playback. The bounded Unity source probe does not accept a controller, visual quality or first-person presentation.

## Runtime sets and authored motion

These are evaluator-selected source candidates, not accepted controller states. Follow a single row for each blend or chain; the appendix preserves file names, coordinates, timings and measurements.

| Set | Controller use | Adoption decision | Exact members |
|---|---|---|---|
| Walk eight directions | Eight-direction controller gait | Source prototype with controller travel. Loop/contact policy needs review; phase-anchor trial remains unadopted (22/24 candidates retain lint across all three rings). | [8 exact members](protofactor-basic-locomotion-evidence.md#exact-runtime-members) |
| Run eight directions | Eight-direction controller gait | Source prototype with controller travel. Verify faster contact timing; phase-anchor trial remains unadopted (22/24 candidates retain lint across all three rings). | [8 exact members](protofactor-basic-locomotion-evidence.md#exact-runtime-members) |
| Crouch eight directions | Eight-direction controller gait | Source prototype with controller travel. Configure stance changes; phase-anchor trial remains unadopted (22/24 candidates retain lint across all three rings). | [8 exact members](protofactor-basic-locomotion-evidence.md#exact-runtime-members) |

## Integration recipe

1. **Members/topology:** `topology=directional-blend`; create separate walk, run, and crouch two-parameter trees from the appendix members. Use local horizontal velocity for direction; handle standing/crouching and cover as states.
2. **Timing/synchronization:** `sync=not-evaluated`; establish the project or vendor loop/contact policy before choosing synchronized blending. Current unpromoted gait-anchor candidates reduce measured phase spread; test the declared support-foot correspondence, loop closure, and intermediate weights before adoption.
3. **State ownership:** `owner=controller`; the selected in-place roots have no measured travel. Let the kinematic controller own translation/collision and choose playback speed from observed stride/contact tests; zero root speed is not a usable speed threshold. Give RM actions an explicit separate movement policy.
4. **Composition constraints:** `composition=full-body`; preserve authored lower-body/torso coupling initially. Weapon layers need an explicit spine mask, reference-pose convention, and planted-foot test.
5. **Acceptance gate:** `gate=engine-and-visual-review`; test stops, reversals, diagonal transitions, slopes, and crouch changes on the target character. Source mixer execution does not validate exported candidates.

## Technical issue register

Phase spread here is the minimum covering arc in cycles: sort phases in `[0,1)`, include the wraparound gap, then subtract the largest gap from 1. It is not `max_circular_deviation_from_mean`; neither measure alone proves support-foot or visual compatibility.

Severity describes impact as delivered for the stated use. Residual status is explicit; untested risks are not confirmed artist defects.

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| BL-TIME | major | Twelve cover/throw files contain 36 negative-time findings; an importer can trim or interpret their pre-roll differently. [Readiness guidance](../game-ready-clips.md#the-readiness-ladder). | artist-author | Re-export deliberate clip start/range, or review the twelve declared slice candidates. All time findings disappear; ten candidates also pass their declared lint. Sources remain unchanged. Candidate time-ordering residual is mechanically cleared in all twelve; two retain other lint findings. All twelve remain unpromoted pending contact/engine review. | Explicit range slicing exists; it does not recover missing artistic intent. | Baseline affected-file list and slice verification; `observed-animsmith`. |
| BL-PHASE | note | Evaluator-selected walk/run/crouch rings have phase spreads 0.6598/0.4630/0.7156 cycles. Their contact correspondence and intended synchronization are unconfirmed; the numbers alone do not establish an artist defect. [Readiness guidance](../game-ready-clips.md#the-readiness-ladder). | unknown | Obtain project or vendor loop/contact intent, then test a declared phase/contact tolerance over the exact rings. Anchored spreads are 0.0724/0.0938/0.0502 cycles in the same order, but 22 of 24 candidates retain lint findings. Request artist cleanup only if accepted intent and playback establish a source-motion problem. Residual adoption unresolved; no output promoted. | Current declared gait anchoring reduces measured phase disagreement; it cannot choose intended contacts or approve blending. | Exact sets and source/output phases `observed-animsmith`; synchronization/contact acceptance `not-evaluated`. |
| BL-ROOT | major | Fourteen files fail the declared in-place policy; a controller that also moves the character may double-apply displacement. [Readiness guidance](../game-ready-clips.md#the-readiness-ladder). | engine-config | Select a documented root policy per state and inspect those measured trajectories. Do not infer in-place from missing RM suffix alone. Accept only after displacement/collision tests; residual unresolved. | Root measurements identify the conflict; the project must choose movement ownership. | Declared contract ledger lists exact files; `observed-animsmith`. |
| BL-TRACK | minor | Constant tracks inflate storage; note counts alone do not establish runtime cost. [Readiness guidance](../game-ready-clips.md#the-readiness-ladder). | animsmith-current-declared | Three prune candidates pass baseline lint. Accept only after required tracks, masks and engine playback remain equivalent; residual unverified. | Current pruning is available; no measured frame-time improvement is claimed. | Three representative outputs; `observed-animsmith`. |

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
