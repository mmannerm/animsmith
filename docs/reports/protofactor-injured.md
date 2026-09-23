# Animation pack evaluation: Protofactor Injured Animset

> Technical verdict: **Usable with conditions**
>
> Evaluation completeness: **partial** — all source files and declared checks were reviewed; engine playback and visual quality remain untested.
>
> Confidence: **medium**
>
> Evaluation date: **2026-09-16**
>
> Current evaluator: **AnimSmith 0.14.0**
>
> Report format: **3**
>
> Detailed evidence: [Protofactor Injured evidence](protofactor-injured-evidence.md)

## Technical decision

**Prototype decision:** Try a separate style-local speed state for each injury style. Configure speed and healthy-state transitions; foot contacts and character fit remain unknown.

**Camera scope:** Full-body humanoid animations; appearance on your target character and camera is untested. Dedicated first-person arms/viewmodel use is unverified.

This animset is one of eight locally evaluated parts of [Ultimate Animation Collection](protofactor-ultimate-animation-collection.md). The local asset revision is unknown; the collection report separates evaluated files from the dated vendor listing.

**Content in evaluated inventory:** Seven idle/walk/run injury styles. Each A–G style has an evaluated three-file candidate; filenames do not establish a severity order or recovery path. These are identified motions; gameplay behavior has not been approved.

**Use as seven separate injury-style locomotion candidates.** Each proposed style has idle/walk/run members; there is no evidence that A–G are interchangeable directions or severity levels. Keep the selected style coherent, let the kinematic controller own motion, and check walk/run contact timing and transitions to healthy locomotion.

`observed-animsmith`: the official 0.14.0 evaluator inspected and measured all 72 FBXs; 28/70 declared-contract files pass and 42 fail per output format. The findings below separate source problems, project settings and untested gameplay. Transformed outputs still need gameplay and visual checks.

## Capability coverage

### Content present

Filename-classified A–G styles each supply idle, walk and run candidates. Each style is a distinct three-member speed-state hypothesis.

### Content gaps and unknowns

The letters do not classify severity or an ordered progression. Recovery and healthy-state handoffs have no accepted set classification here; this does not establish absence from the wider source.

### Evaluation still needed

Test style choice, speed calibration, planted-foot behavior, cross-style and healthy-state transitions on the target character. Engine, retarget and visual acceptance remain open.

## Runtime sets and authored motion

Use each row as a separate blend or sequence proposal. The linked appendix names its exact clips, timings and measurements; controller playback and contact quality still need testing.

| Set | Controller use | Adoption decision | Exact members |
|---|---|---|---|
| Injury style A speed | Style-local idle/walk/run; controller travel | Style-local prototype. RunInjuredA has loop seam findings; review true cycles and foot contacts before speed blending ([IN-LOOP](#technical-issue-register)). | [3 exact members](protofactor-injured-evidence.md#exact-runtime-members) |
| Injury style B speed | Style-local idle/walk/run; controller travel | Style-local prototype. Selected injury loops mostly fail their declared checks; review this style’s loop intent and contacts before speed blending. | [3 exact members](protofactor-injured-evidence.md#exact-runtime-members) |
| Injury style C speed | Style-local idle/walk/run; controller travel | Style-local prototype. Selected injury loops mostly fail their declared checks; review this style’s loop intent and contacts before speed blending. | [3 exact members](protofactor-injured-evidence.md#exact-runtime-members) |
| Injury style D speed | Style-local idle/walk/run; controller travel | Style-local prototype. Selected injury loops mostly fail their declared checks; review this style’s loop intent and contacts before speed blending. | [3 exact members](protofactor-injured-evidence.md#exact-runtime-members) |
| Injury style E speed | Style-local idle/walk/run; controller travel | Style-local prototype. Selected injury loops mostly fail their declared checks; review this style’s loop intent and contacts before speed blending. | [3 exact members](protofactor-injured-evidence.md#exact-runtime-members) |
| Injury style F speed | Style-local idle/walk/run; controller travel | Style-local prototype. Selected injury loops mostly fail their declared checks; review this style’s loop intent and contacts before speed blending. | [3 exact members](protofactor-injured-evidence.md#exact-runtime-members) |
| Injury style G speed | Style-local idle/walk/run; controller travel | Style-local prototype. Selected injury loops mostly fail their declared checks; review this style’s loop intent and contacts before speed blending. | [3 exact members](protofactor-injured-evidence.md#exact-runtime-members) |

## Integration recipe

1. **Members/topology:** Create one idle/walk/run tree per chosen style A–G. Treat style changes as explicit state transitions; do not distribute injury letters around a direction tree.
2. **Timing/synchronization:** Establish intended cyclic playback and support-contact timing before choosing synchronization. Walks measure 1.333 s; runs measure 0.700 or 0.800 s. Match support-foot timing between moving clips; an almost-stationary idle gait phase is not a useful contact anchor. `Humanoid@RunInjuredB.fbx` passes a non-looping declaration, which does not establish it as a clean repeating run.
3. **State ownership:** Selected in-place roots are stationary. Tune speed thresholds to stride/contact behavior, not their measured zero root speed. Keep root-motion alternatives under a separate movement policy.
4. **Composition constraints:** Preserve the limp and compensating torso pose. Upper-body masks can erase the injury expression; validate any weapon layer separately.
5. **Acceptance gate:** Test start/stop, velocity changes, healthy-to-injured transitions and interruptions on the target character.

## Technical issue register

Severity describes impact as delivered for the stated use. Residual status is explicit; untested risks are not confirmed artist defects.

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| IN-LOOP | major | Twenty of the 21 selected idle/walk/run members fail their declared contracts. `Humanoid@RunInjuredA.fbx` has loop-closure/seam findings; repeated playback and speed blending need review. [Readiness guidance](../game-ready-clips.md#the-readiness-ladder). | artist-author | Correct intended loop endpoints and support-foot timing. Fourteen gait-anchor candidates were generated; thirteen still fail declared lint. Residual adoption remains unresolved; no candidates are promoted and thirteen retain contract findings. | Current anchoring changes phase; it is not proof of a clean seam or preserved limp. | Exact selected files, `Take 001`, current source/output lint; `observed-animsmith`. |
| IN-STYLE | minor | Mixing lettered styles as if they were directions or scalar severity levels has no established semantic basis. [Readiness guidance](../game-ready-clips.md#the-readiness-ladder). | engine-config | Choose a style and keep idle/walk/run coherent; explicitly review any style transition. Residual: project decision, not proven vendor defect. | Explicit set declarations make the choice reproducible. | Names and timing observed; artistic relationship `not-evaluated`. |
| IN-TRACK | minor | Dense constant tracks may add storage without useful motion. [Readiness guidance](../game-ready-clips.md#the-readiness-ladder). | animsmith-current-declared | Pruning reduced tracks in WalkInjuredA, but its loop check still fails. Compare required channels and engine playback before using that output. Unresolved. | Current pruning exists; performance gain remains unmeasured. | Fresh prune output and diff; `observed-animsmith`. |

## Engine status

| Runtime | Evidence level | Technical result | Remaining gate |
|---|---|---|---|
| Unity unspecified | not-evaluated | No current import or playback. | Blend, mask, visual, and build tests. |
| Unreal Engine unspecified | not-evaluated | No current import or playback. | Retarget, graph, and build tests. |
| Godot unspecified | not-evaluated | No current conversion or playback. | Conversion/import and graph tests. |
| Bevy unspecified | not-evaluated | No current handoff or runtime test. | glTF handoff and runtime test. |

## Fit and limitations

Combine with Basic Locomotion as a healthy/injured state switch, with speed and stance changes handled deliberately. Combining with weapon packs requires checking whether a grip/aim layer conflicts with the injured torso and supported limb. No cross-pack contact, masking or artistic match is approved.

These are proposed integration decisions (`inferred`), with target-controller, contact and artistic acceptance still `not-evaluated`.

## Changes between AnimSmith versions

AnimSmith 0.14.0 — reran 72 input baselines, 70 per-file declared contracts and 15 remediation trials using the official release. Added current exact-member generic scenarios and developer/artist actions. AnimSmith 0.10.0 — superseded historical evidence; none of its generated outputs or engine results is relabelled as a fresh run.

## Evidence status

Current evidence uses the official release and the [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder); commercial artifacts remain external.

## Sources

- Protofactor, [Animset: Injured](https://protofactor.biz/product/animset-injured/) — product context.
- AnimSmith, [CLI reference](../cli.md) and [game-ready clips](../game-ready-clips.md) — evaluation boundaries.
