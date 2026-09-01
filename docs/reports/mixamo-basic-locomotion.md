# Animation pack evaluation: Mixamo Basic Locomotion

> Technical verdict: **Insufficient technical evidence**
>
> Evaluation completeness: **partial** — file inspection does not establish license, runtime, visual, contact, retarget, or cross-pack behavior.
>
> Confidence: **medium**
>
> Evaluation date: **2026-09-01**
>
> Current evaluator: **AnimSmith 0.10.0**
>
> Report format: **2**
>
> Detailed evidence: [Evidence appendix](mixamo-basic-locomotion-evidence.md)

## Technical decision

This constituent is one observed archive pair in the locally held collection.
AnimSmith 0.10.0 mechanically inspected, measured, and linted all 12 extracted FBX files. The untouched baseline has 0 errors, 4 warnings, and 1,712 non-gating constant-track notes; these are file-ready evidence only, not gameplay or engine acceptance. The declared in-place XZ contract has 0 errors and 1 duration warning; the declared root-motion contract has 4 stationary-root errors and 3 duration warnings. No source bytes were changed. [The readiness ladder](../game-ready-clips.md#the-readiness-ladder) remains the governing boundary.

## Capability coverage

### Complete core

- Fresh source inventory, parsing, inspection, measurement, and baseline mechanical lint were completed for all delivered files with the declared current evaluator.

### Partial supporting gameplay

- The in-place XZ declaration has 0 errors but 1 duration warning; 4 root-motion-labelled files have stationary roots and that declaration has 3 duration warnings. Per-clip movement ownership is still not established.

### Absent

- No license terms, target engine import, blend graph, retargeting, contact, visual, performance, or production acceptance evidence was supplied.

## Runtime sets and authored motion

No important runtime sets were identified.

## Integration recipe

1. **Members/topology:** `topology=not-evaluated`; do not construct a blend set from archive labels alone.
2. **Timing/synchronization:** `sync=not-evaluated`; no per-member loop or phase contract was established.
3. **State ownership:** `owner=per-clip-declaration`; set XZ, vertical, and yaw ownership only in the target project.
4. **Composition constraints:** `composition=full-body-default`; do not promote masks, additive layers, or contacts without runtime evidence.
5. **Acceptance gate:** `gate=target-engine-visual-test`; import, playback, blend, retarget, and visual/contact testing remain required.

## Technical issue register

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| MIX-OWN-001 | moderate | Four root-motion-labelled files have stationary roots under a package-wide animation-owned XZ declaration, so that policy can make characters run in place. Guidance: not applicable. | engine-config | Declare ownership per clip after target-controller review. | Not a generic rewrite; it needs semantic/project intent. | observed-animsmith; current contract result |

## Engine status

| Runtime | Evidence level | Technical result | Remaining gate |
|---|---|---|---|
| Unity unspecified | not-evaluated | No import or playback evidence | Test exact importer and controller |
| Unreal Engine unspecified | not-evaluated | No import or playback evidence | Test exact importer and retarget path |
| Godot unspecified | not-evaluated | No import or playback evidence | Test exact importer and AnimationTree |
| Bevy unspecified | not-evaluated | No import or playback evidence | Test exact loader and graph |

## Fit and limitations

Best fit is a controlled evaluation intake where a project can supply per-clip intent and test its own controller. It is a poor fit for immediate drop-in compatibility claims.
Cross-pack compatibility is unknown and must be tested against the exact intended constituents and target rig.

## Changes between AnimSmith versions

AnimSmith 0.10.0 — Fresh inventory, baseline, declared-contract, offline-report, and FBX `fix --dry-run` passes used the official `v0.10.0` artifact. The baseline remains error-free; the revalidated root-motion declaration finds 4 stationary-root errors. Output schema changed from v17 to v19 and measurements schema from v16 to v18. AnimSmith 0.7.0 evidence is retained historical evidence only.

## Evidence status

12 delivered FBX files and 10 manifest-declared motions were evaluated mechanically with AnimSmith 0.10.0 at revision `db91d8dda3326f97f581d4d62104d928caec383f`. Fresh external inventories and scrubbed SHA-256 identities are retained outside the repository. The companion appendix records the schema and open evidence gates.

## Sources

- Delivered archive manifests and AnimSmith 0.10.0 output, retained externally as scrubbed evidence.
