# Animation pack evaluation: Protofactor Injured Animset

> Technical verdict: **Usable with conditions**
>
> Evaluation completeness: **partial** — all current file and contract checks ran; no current engine or visual acceptance ran.
>
> Confidence: **medium**
>
> Evaluation date: **2026-09-01**
>
> Current evaluator: **AnimSmith 0.10.0**
>
> Report format: **2**
>
> Detailed evidence: [Protofactor Injured evidence](protofactor-injured-evidence.md)

## Technical decision

Official AnimSmith 0.10.0 loads all 72 delivered FBXs. The untouched baseline has 9,915 `constant-track` notes and no errors. Contracts cover 70 motion files: 28 pass and 42 fail, with 15 loop-closure, 42 rotational loop-seam, and 31 velocity loop-seam errors. No current blend, root-motion, engine, retarget, contact, or visual test establishes acceptance.

## Capability coverage

### Complete core

- The delivery has injured idle, kneel, sit, transition, walk, and run filename families.

### Partial supporting gameplay

- Current contract evidence covers 14 declared in-place files, but loop and blend conditions remain unresolved.

### Absent

- No current recovery, retarget, masking, engine, or artistic acceptance is established.

## Runtime sets and authored motion

No important runtime sets were identified.

## Integration recipe

1. **Members/topology:** `topology=not-evaluated`; declare injured locomotion rings after clip review.
2. **Timing/synchronization:** `sync=not-evaluated`; resolve failed loop contracts first.
3. **State ownership:** `owner=not-evaluated`; declare IP/RM controller ownership.
4. **Composition constraints:** `composition=full-body`; do not approve injury masks.
5. **Acceptance gate:** `gate=engine-and-visual-review`; test blends, retarget, and gameplay behavior.

## Technical issue register

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| IN-010 | major | [Loop continuity findings](../game-ready-clips.md#the-readiness-ladder) can cause visible wrap or transition pulses. | artist-author | Review intended loops and correct/re-export motion. | Current checks identify the gate; no artistic repair is implied. | `observed-animsmith`; 42 contract failures. |

## Engine status

| Runtime | Evidence level | Technical result | Remaining gate |
|---|---|---|---|
| Unity unspecified | not-evaluated | No current import or playback. | Blend, mask, visual, and build tests. |
| Unreal Engine unspecified | not-evaluated | No current import or playback. | Retarget, graph, and build tests. |
| Godot unspecified | not-evaluated | No current conversion or playback. | Conversion/import and graph tests. |
| Bevy unspecified | not-evaluated | No current handoff or runtime test. | glTF handoff and runtime test. |

## Fit and limitations

Use only after loop, blend, retarget, and visual acceptance. Cross-pack compatibility remains untested.

## Changes between AnimSmith versions

AnimSmith 0.10.0 — official release revalidated all 72 FBXs and 70 declared contracts. Earlier 0.7.0 evidence is superseded historical evidence only.

## Evidence status

Current evidence uses the official release and the [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder); commercial artifacts remain external.

## Sources

- Protofactor, [Animset: Injured](https://protofactor.biz/product/animset-injured/) — product context.
- AnimSmith, [CLI reference](../cli.md) and [game-ready clips](../game-ready-clips.md) — evaluation boundaries.
