# Animation pack evaluation: Protofactor Dual Swords Animset

> Technical verdict: **Usable with conditions**
>
> Evaluation completeness: **partial** — current mechanical and declared-contract evidence completed; engine, visual, contact, and artistic acceptance were not rerun.
>
> Confidence: **medium**
>
> Evaluation date: **2026-09-01**
>
> Current evaluator: **AnimSmith 0.10.0**
>
> Report format: **2**
>
> Detailed evidence: [Protofactor Dual Swords Animset evidence appendix](protofactor-dual-swords-evidence.md)

## Technical decision

The current baseline is mechanically readable with AnimSmith 0.10.0, but this pack is not a blanket game-ready asset. 756 baseline commands completed successfully; constant-track notes and strict loop contracts require review. The 25 bounded remediation candidates comprise 24 gait-anchor and 1 prune-constant-tracks transform; all exited 0 with post-output checks recorded in the external evidence projection. They remain external, unpromoted candidates pending engine, contact, and visual review.

## Capability coverage

### Complete core

- Source inventory and serial AnimSmith baseline completed without publishing licensed inputs or derivatives.
- 189 delivered FBX candidates were inspected, measured, and linted in both output formats.
- Declared contracts and bounded remediation trials were rerun with the current evaluator.

### Partial supporting gameplay

- 186 declared-contract inputs: 24 clean and 162 non-clean results in each output format.
- Candidate transforms are mechanical evidence, not engine or artistic acceptance.

### Absent

- Current target-engine import/playback, blending, masking, retargeting, contact, performance, and artistic review.

## Runtime sets and authored motion

No important runtime sets were identified.

## Integration recipe

1. **Members/topology:** `topology=pack-local`; construct project state graphs only from reviewed imported clips.
2. **Timing/synchronization:** `sync=contract-gated`; use loop and seam findings to gate transitions.
3. **State ownership:** `owner=project-controller`; assign root-motion ownership per state.
4. **Composition constraints:** `composition=full-body-review`; validate masks, props, IK, and attachments.
5. **Acceptance gate:** `gate=engine-visual-contact`; require import, playback, contact, and visual sign-off.

## Technical issue register

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| PF-010 | major | Declared loop and seam contracts do not pass for the full selected corpus; clip-level admission is required. [Guidance](../game-ready-clips.md#the-readiness-ladder). | unknown | Keep non-clean clips out of production runtime sets. | Generic validation and conforming assistance may help. | `observed-animsmith`; current contract evidence. |
| PF-011 | minor | Generated candidates are mechanical signals, not visual acceptance. [Guidance](../game-ready-clips.md#the-readiness-ladder). | unknown | Review deltas and playback. | Generic track cleanup is applicable. | `observed-animsmith`; 25/25 transforms exit 0. |

## Engine status

| Runtime | Evidence level | Technical result | Remaining gate |
|---|---|---|---|
| Unity | not-evaluated | No current import or playback was run. | Disposable-project import and visual controller tests. |
| Unreal Engine | not-evaluated | No current import or playback was run. | Import, retarget, and blend tests. |
| Godot | not-evaluated | No current import or playback was run. | Conversion/import route and runtime tests. |
| Bevy | not-evaluated | No current import or playback was run. | Conversion, loading, graph, and performance tests. |

## Fit and limitations

This is current mechanical evidence, not a guarantee of engine readiness, gameplay suitability, or artistic quality. Licensed source, candidate outputs, and local evidence remain private.

## Changes between AnimSmith versions

AnimSmith 0.10.0 — revalidated current inventory, baseline, declared contracts, and bounded remediation using the official FBX-capable release binary. AnimSmith 0.7.0 — retained historical evidence is superseded for current-state conclusions.

## Evidence status

See the [readiness ladder](../game-ready-clips.md#the-readiness-ladder) and [appendix](protofactor-dual-swords-evidence.md). Mechanical completion is separate from engine and visual acceptance.

## Sources

- Protofactor product listing and [End User License Agreement](https://protofactor.biz/end-user-license-agreement/) — vendor context only, not local-revision proof.
