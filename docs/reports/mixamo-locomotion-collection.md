# Animation pack evaluation: Mixamo Locomotion Collection

> Technical verdict: **Insufficient technical evidence**
>
> Evaluation completeness: **partial** — this is a documentation rollup of nine current constituent evaluations, not a new collection-level runtime, visual, or compatibility evaluation.
>
> Confidence: **medium**
>
> Evaluation date: **2026-09-01**
>
> Current evaluator: **AnimSmith 0.10.0**
>
> Report format: **2**
>
> Detailed evidence: [Evidence appendix](mixamo-locomotion-collection-evidence.md)

## Technical decision

This partial rollup aggregates the current evidence for [Basic](mixamo-basic-locomotion.md), [Female Basic](mixamo-female-basic-locomotion.md), [Female](mixamo-female-locomotion.md), [Locomotion](mixamo-locomotion.md), [Longbow](mixamo-longbow-locomotion.md), [Magic](mixamo-magic-locomotion.md), [Male](mixamo-male-locomotion.md), [Pistol/Handgun](mixamo-pistol-handgun-locomotion.md), and [Rifle 8-Way](mixamo-rifle-8-way-locomotion.md). It makes no new collection-level classification, runtime-set, engine, visual, remediation, retarget, or compatibility claim.

The nine fresh baselines admitted and mechanically checked all 249 extracted FBX files: 0 errors, 50 `duration-sanity` warnings, and 35,405 non-gating `constant-track` notes. The declared in-place XZ controls had 0 errors and 13 warnings. The declared root-motion XZ controls had 52 stationary-root errors and 37 warnings. These archive-level declarations are not per-clip movement ownership, and their finding counts must not be pooled with the empty-baseline result. No source bytes were changed. [The readiness ladder](../game-ready-clips.md#the-readiness-ladder) remains the governing boundary.

## Capability coverage

### Complete core

- Current constituent inventory, parsing, inspection, measurement, and empty-baseline mechanical lint cover all 249 delivered FBX files.

### Partial supporting gameplay

- Archive-level in-place and root-motion declarations were exercised, but their labels do not establish per-clip XZ, vertical, or yaw ownership.

### Absent

- No collection-level source-to-manifest member mapping, gameplay classification, runtime-set definition, engine import/playback, blend, retarget, contact, visual, performance, production, or cross-pack compatibility evidence was freshly evaluated.

## Runtime sets and authored motion

No important runtime sets were identified.

## Integration recipe

1. **Members/topology:** `topology=not-evaluated`; do not construct a collection blend set from archive labels or this rollup.
2. **Timing/synchronization:** `sync=not-evaluated`; no per-member loop, timing, or phase contract was refreshed at collection scope.
3. **State ownership:** `owner=per-clip-declaration`; assign XZ, vertical, and yaw ownership only after target-project review.
4. **Composition constraints:** `composition=full-body-default`; do not promote masks, additive layers, contacts, or retargeting from constituent mechanical evidence.
5. **Acceptance gate:** `gate=target-engine-visual-test`; exact-engine import, playback, blend, retarget, and visual/contact testing remain required.

## Technical issue register

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| MIX-OWN-001 | moderate | A root-motion declaration finds 52 stationary-root files, so package-wide animation-owned XZ can make a character run in place. Guidance: not applicable. | unknown | Declare movement ownership per clip after target-controller review. | Not a generic rewrite; it needs semantic/project intent. | observed-animsmith; partial |

## Engine status

| Runtime | Evidence level | Technical result | Remaining gate |
|---|---|---|---|
| Unity unspecified | not-evaluated | No fresh collection import or playback evidence | Test exact importer and controller |
| Unreal Engine unspecified | not-evaluated | No fresh collection import or playback evidence | Test exact importer and retarget path |
| Godot unspecified | not-evaluated | No fresh collection import or playback evidence | Test exact importer and AnimationTree |
| Bevy unspecified | not-evaluated | No fresh collection import or playback evidence | Test exact loader and graph |

## Fit and limitations

Best fit is a controlled intake where the consuming project supplies per-clip intent and validates its controller. It is a poor fit for immediate drop-in collection compatibility claims.

Cross-pack compatibility remains unknown: this refresh did not compare overlapping paths or skeleton/reference-rig identity, and did not test blend, retarget, mask, contact, or visual behavior.

## Changes between AnimSmith versions

AnimSmith 0.10.0 — Fresh constituent inventory, baseline, archive-variant declaration, and preflight evidence revalidated and superseded the prior collection summary. The official evaluator now records output schema v19 and measurements schema v18.

AnimSmith 0.7.0 — Retained only as historical collection evidence; its output schema v17, measurements schema v16, and prior evidence digests are superseded by the current constituent evidence.

## Evidence status

The included nine constituents total 249 delivered FBX files and 231 manifest-declared motions. Current evidence used AnimSmith 0.10.0 at peeled source commit `db91d8dda3326f97f581d4d62104d928caec383f`; source archives, inventories, command outputs, configs, and scrubbed SHA-256 identities remain external. The companion appendix records the exact evaluator provenance and the evidence boundary.

## Sources

- The nine linked constituent reports and their current external AnimSmith 0.10.0 evidence; no licensed payloads are published.
