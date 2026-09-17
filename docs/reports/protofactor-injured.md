# Animation pack evaluation: Protofactor Injured Animset

> Technical verdict: **Usable with conditions**
>
> Evaluation completeness: **partial** — all current file and contract checks ran; no current engine or visual acceptance ran.
>
> Confidence: **medium**
>
> Evaluation date: **2026-09-16**
>
> Current evaluator: **AnimSmith 0.14.0**
>
> Report format: **2**
>
> Detailed evidence: [Protofactor Injured evidence](protofactor-injured-evidence.md)

## Technical decision

**Use as seven separate injury-style locomotion candidates.** Each proposed style has idle/walk/run members; there is no evidence that A–G are interchangeable directions or severity levels. Keep the selected style coherent, let the kinematic controller own motion, and check walk/run contact timing and transitions to healthy locomotion.

`observed-animsmith`: the official 0.14.0 evaluator inspected and measured all 72 FBXs; 28/70 declared-contract files pass and 42 fail per output format. Conditions below distinguish source findings, evaluator policy and untested gameplay. No remediation candidate is promoted.

## Capability coverage

### Complete core

- No complete gameplay core has been validated on a target game controller. Candidate content and integration scope are listed below; completed file checks do not establish gameplay completeness.

### Partial supporting gameplay

- Seven separately selected idle/walk/run styles support proposed injured-locomotion states. Injury expression, speed/phase calibration and healthy-state handoffs remain open.

### Absent

- No current recovery, retarget, masking, engine, or artistic acceptance is established.

## Runtime sets and authored motion

New evaluator-selected generic scenarios: current source bytes establish the exact members, take names, durations, and measured roots below. Names suggest gameplay roles; topology and semantic intent are hypotheses requiring clip review. These are not reconstructed historical manifests or measured collection-output sets. Every listed member uses `Take 001`; full identities are in the external selected-set ledger.

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

## Integration recipe

1. **Members/topology:** `topology=speed-blend`; create one idle/walk/run tree per chosen style A–G. Treat style changes as explicit state transitions; do not distribute injury letters around a direction tree.
2. **Timing/synchronization:** `sync=normalized-phase`; walks measure 1.333 s; runs measure 0.700 or 0.800 s. Match support-foot timing between moving clips; an almost-stationary idle gait phase is not a useful contact anchor. `Humanoid@RunInjuredB.fbx` passes a non-looping declaration, which does not establish it as a clean repeating run.
3. **State ownership:** `owner=controller`; selected non-RM roots are stationary. Tune speed thresholds to stride/contact behavior, not their measured zero root speed. Keep RM alternatives under a separate movement policy.
4. **Composition constraints:** `composition=full-body`; preserve the limp and compensating torso pose. Upper-body masks can erase the injury expression; validate any weapon layer separately.
5. **Acceptance gate:** `gate=engine-and-visual-review`; test start/stop, velocity changes, healthy-to-injured transitions and interruptions on the target character.

## Technical issue register

Severity describes impact as delivered for the stated use. Residual status is explicit; untested risks are not confirmed artist defects.

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| IN-LOOP | major | Twenty of the 21 selected idle/walk/run members fail their declared contracts. `Humanoid@RunInjuredA.fbx` has loop-closure/seam findings; repeated playback and speed blending need review. [Readiness guidance](../game-ready-clips.md#the-readiness-ladder). | artist-author | Correct intended loop endpoints and support-foot timing. Fourteen gait-anchor candidates were generated; thirteen still fail declared lint. Residual adoption remains unresolved; no candidates are promoted and thirteen retain contract findings. | Current anchoring changes phase; it is not proof of a clean seam or preserved limp. | Exact selected files, `Take 001`, current source/output lint; `observed-animsmith`. |
| IN-STYLE | minor | Mixing lettered styles as if they were directions or scalar severity levels has no established semantic basis. [Readiness guidance](../game-ready-clips.md#the-readiness-ladder). | engine-config | Choose a style and keep idle/walk/run coherent; explicitly review any style transition. Residual: project decision, not proven vendor defect. | Explicit set declarations make the choice reproducible. | Names and timing observed; artistic relationship `not-evaluated`. |
| IN-TRACK | minor | Dense constant tracks may add storage without useful motion. [Readiness guidance](../game-ready-clips.md#the-readiness-ladder). | animsmith-current-declared | One WalkInjuredA prune candidate was emitted, but its declared loop lint still fails. Residual: unresolved; validate channel and runtime equivalence before replacing sources. | Current pruning exists; performance gain remains unmeasured. | Fresh prune output and diff; `observed-animsmith`. |

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
