# Animation pack evaluation: Protofactor Campfire

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
> Detailed evidence: [Protofactor Campfire evidence](protofactor-campfire-evidence.md)

## Technical decision

Official AnimSmith 0.10.0 loads all 29 delivered FBXs. The untouched baseline has 3,664 `constant-track` notes across 27 animated files and no errors. Contracts cover 25 motion files: 17 pass and 8 fail; findings are one loop-closure, eight rotational loop-seam, and six velocity loop-seam errors. One current constant-track candidate was generated and mechanically verified as part of the combined 17-candidate remediation rerun; it remains unpromoted. These mechanical conditions do not establish prop contact, loops, or gameplay acceptance.

## Capability coverage

### Complete core

- The delivered files contain idle, posture-transition, fire-lighting, food, and log-toss filename families.

### Partial supporting gameplay

- Current contract evidence identifies loop continuity gates; the one external pruning candidate has no contact or attachment acceptance.

### Absent

- No current locomotion, masking, IK, retarget, or engine acceptance result exists.

## Runtime sets and authored motion

No important runtime sets were identified.

## Integration recipe

1. **Members/topology:** `topology=not-evaluated`; declare posture chains after clip review.
2. **Timing/synchronization:** `sync=not-evaluated`; resolve failed loop contracts first.
3. **State ownership:** `owner=not-evaluated`; motion ownership is not measured for this interaction pack.
4. **Composition constraints:** `composition=full-body`; do not approve masks or prop contact.
5. **Acceptance gate:** `gate=engine-and-visual-review`; test transitions, contacts, props, and build behavior.

## Technical issue register

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| CF-010 | major | [Loop continuity findings](../game-ready-clips.md#the-readiness-ladder) can produce wrap pulses. | artist-author | Review intended loops and correct/re-export motion where needed. | Current declared checks identify the gate; no artistic repair is implied. | `observed-animsmith`; 8 contract failures. |

## Engine status

| Runtime | Evidence level | Technical result | Remaining gate |
|---|---|---|---|
| Unity unspecified | not-evaluated | No current import or playback. | Import, contacts, visual, and build tests. |
| Unreal Engine unspecified | not-evaluated | No current import or playback. | Import, retarget, contacts, and build tests. |
| Godot unspecified | not-evaluated | No current conversion or playback. | Conversion/import and graph tests. |
| Bevy unspecified | not-evaluated | No current handoff or runtime test. | glTF handoff and runtime test. |

## Fit and limitations

Use only after loop, prop, contact, and visual gates are accepted. Cross-pack compatibility remains untested.

## Changes between AnimSmith versions

AnimSmith 0.10.0 — official release revalidated all 29 FBXs and 25 declared contracts, then ran one constant-track pruning candidate in the combined remediation pass. Earlier 0.7.0 evaluator and any engine or offline evidence are historical only.

## Evidence status

Current evidence uses the official release and the [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder); commercial artifacts remain external.

## Sources

- Protofactor, [Animset: Campfire](https://protofactor.biz/product/animset-campfire/) — product context.
- AnimSmith, [CLI reference](../cli.md) and [game-ready clips](../game-ready-clips.md) — evaluation boundaries.
