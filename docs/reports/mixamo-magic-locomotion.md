# Animation pack evaluation: Mixamo Magic Locomotion

> Technical verdict: **Insufficient technical evidence**
>
> Evaluation completeness: **partial** — file inspection does not establish license, runtime, visual, contact, retarget, or cross-pack behavior.
>
> Confidence: **medium**
>
> Evaluation date: **2026-08-26**
>
> Report format: **1**
>
> Detailed evidence: [Evidence appendix](mixamo-magic-locomotion-evidence.md)

## Technical decision

This constituent is one observed archive pair in the locally held collection.
AnimSmith 0.7.0 mechanically inspected 27 extracted FBX files with no errors in the untouched baseline. The baseline retained 6 warnings and constant-track notes; these are not gameplay or engine acceptance. Variant-level in-place ownership was clean, while a blanket root-motion ownership declaration produced 4 stationary-root findings. No source bytes were changed. [The readiness ladder](../game-ready-clips.md#the-readiness-ladder) remains the governing boundary.

## Capability coverage

### Complete core

- File inventory, parsing, inspection, measurement, and baseline mechanical lint were completed for the delivered files.

### Partial supporting gameplay

- Archive-level in-place and root-motion labels supported only a variant-level XZ contract; per-clip movement ownership is not established.

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
| MIX-OWN-001 | moderate | Archive-level root-motion labeling includes stationary-root files, so a package-wide animation-owned XZ policy can make characters run in place. Guidance: not applicable. | unknown | Declare ownership per clip after target-controller review. | Not a generic rewrite; it needs semantic/project intent. | observed-animsmith; partial |

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

## Evidence status

27 delivered FBX files and 25 manifest-declared motions were evaluated mechanically with AnimSmith 0.7.0 at revision `461ac8a`. The retained external manifests use scrubbed SHA-256 identities only. The companion appendix records the schema and open evidence gates.

## Sources

- Delivered archive manifests and AnimSmith 0.7.0 output, retained externally as scrubbed evidence.
