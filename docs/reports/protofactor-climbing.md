# Animation pack evaluation: Protofactor Climbing Animset

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
> Detailed evidence: [Protofactor Climbing evidence](protofactor-climbing-evidence.md)

## Technical decision

Official AnimSmith 0.10.0 loads all 77 delivered FBXs. The untouched baseline has 9,011 `constant-track` notes and no errors. Contracts cover 75 motion files: 34 pass and 41 fail, with 26 loop-closure, 41 rotational loop-seam, and 39 velocity loop-seam errors. No current traversal, contact, root-motion, engine, or visual test establishes runtime acceptance.

## Capability coverage

### Complete core

- The delivery has wall, ladder, obstacle, fall, jump, landing, entry, and exit filename families.

### Partial supporting gameplay

- Mechanical and declared-loop evidence is available, but traversal contacts and controller behavior are untested.

### Absent

- No current IK, retarget, engine, or artistic acceptance is established.

## Runtime sets and authored motion

No important runtime sets were identified.

## Integration recipe

1. **Members/topology:** `topology=not-evaluated`; declare traversal chains after clip review.
2. **Timing/synchronization:** `sync=not-evaluated`; resolve failed loop contracts first.
3. **State ownership:** `owner=not-evaluated`; declare root-motion/controller ownership.
4. **Composition constraints:** `composition=full-body`; do not approve wall or ladder contacts.
5. **Acceptance gate:** `gate=engine-and-visual-review`; test traversal, contacts, IK, and build behavior.

## Technical issue register

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| CL-010 | major | [Loop continuity findings](../game-ready-clips.md#the-readiness-ladder) can cause transition pulses. | artist-author | Review intended loops and correct/re-export motion. | Current checks identify the gate; no artistic repair is implied. | `observed-animsmith`; 41 contract failures. |

## Engine status

| Runtime | Evidence level | Technical result | Remaining gate |
|---|---|---|---|
| Unity unspecified | not-evaluated | No current import or traversal playback. | Controller, contact, visual, and build tests. |
| Unreal Engine unspecified | not-evaluated | No current import or traversal playback. | Retarget, IK, contact, and build tests. |
| Godot unspecified | not-evaluated | No current conversion or playback. | Conversion/import and graph tests. |
| Bevy unspecified | not-evaluated | No current handoff or runtime test. | glTF handoff and runtime test. |

## Fit and limitations

Use only after target-engine traversal and contact acceptance. Cross-pack and artistic compatibility remain untested.

## Changes between AnimSmith versions

AnimSmith 0.10.0 — official release revalidated all 77 FBXs and 75 declared contracts. Earlier 0.7.0 evidence is superseded historical evidence only.

## Evidence status

Current evidence uses the official release and the [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder); commercial artifacts remain external.

## Sources

- Protofactor, [Animset: Climbing](https://protofactor.biz/product/animset-climbing/) — product context.
- AnimSmith, [CLI reference](../cli.md) and [game-ready clips](../game-ready-clips.md) — evaluation boundaries.
