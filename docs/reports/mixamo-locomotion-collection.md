# Animation pack evaluation: Mixamo Locomotion Collection

> Technical verdict: **Restricted use**
>
> Evaluation completeness: **partial** — exhaustive current source mechanics for nine constituents; no current collection binding, vendor mapping, engine, visual, retarget, contact, or cross-pack acceptance.
>
> Confidence: **medium**
>
> Evaluation date: **2026-09-16**
>
> Current evaluator: **AnimSmith 0.14.0**
>
> Report format: **2**
>
> Detailed evidence: [Evidence appendix](mixamo-locomotion-collection-evidence.md)

## Technical decision

All 249 extracted FBX files loaded and completed current inspect, measure, and empty-baseline lint: 0 errors, 50 `duration-sanity` warnings, and 35405 `constant-track` notes. Current archive-level contracts produced 52 stationary-root errors. All sources resolve the Mixamo profile; 231 motion-named files have 66 bones and 18 `X Bot.fbx` reference files have 68.

Developer decision: use the collection only as nine separately admitted source pools for bounded controller prototypes. The collection-level `hypothesis/full-body-unarmed-to-pistol` below is a new evaluator proposal for a full-body state handoff, not proof of compatibility. Do not merge blend trees, share retarget settings, layer weapons, or ship a combined controller until exact pairwise hierarchy/rest/scale checks plus target-engine transition, contact, deformation, and style review pass. No source bytes were changed. [The readiness ladder](../game-ready-clips.md#the-readiness-ladder) governs adoption.

## Capability coverage

### Complete core

- No complete gameplay core has been validated on a target game controller. Candidate content and integration scope are listed below; completed file checks do not establish gameplay completeness.

### Partial supporting gameplay

- Each constituent now has an exact, newly declared kinematic scenario; the rollup adds one conservative full-body cross-pack handoff hypothesis.
- Archive-level variant declarations ran, but 52 files need per-file movement ownership and none has collection-level runtime acceptance.

### Absent

- No authoritative 231-motion-to-249-file mapping, current collection binding, loop/phase contract, cross-pack skeleton identity, engine graph, retarget, visual/contact, performance, or artistic-style acceptance exists.

## Runtime sets and authored motion

| Set/profile | Role or coordinate | Exact members | Variant/type | Timing or motion | Runtime contract |
|---|---|---|---|---|---|
| `hypothesis/full-body-unarmed-to-pistol` | source-state | `mixamo-basic-locomotion::Basic_Locomotion_Pack_-_in-place/walking.fbx` | set_type=transition-chain | duration=1.033 s; frames=32 frames | loop=unknown; transition=full-body; movement=controller-xz-yaw; state=unarmed |
| `hypothesis/full-body-unarmed-to-pistol` | destination-state | `mixamo-pistol-handgun-locomotion::Pistol-Handgun_Locomotion_Pack_-_in-place/pistol run.fbx` | set_type=transition-chain | duration=0.500 s; frames=16 frames | loop=unknown; transition=full-body; movement=controller-xz-yaw; state=pistol |

This is a new evaluator hypothesis. It establishes neither semantic equivalence nor pairwise technical/artistic compatibility.

## Integration recipe

1. **Members/topology:** `topology=full-body-state-handoff`; use only the namespaced unarmed and pistol members above in separate states.
2. **Timing/synchronization:** `transition=unsynced-crossfade-hypothesis`; keep source timing, choose crossfade only in the target engine, and do not infer loop or gait phase.
3. **State ownership:** `owner=controller-xz-yaw-collision`; one kinematic controller owns movement and collision across the state boundary.
4. **Composition constraints:** `composition=no-cross-pack-layering`; start full-body; no upper-body mask, additive, shared IK, socket, or retarget profile is established.
5. **Acceptance gate:** `gate=pairwise-engine-artistic`; compare hierarchy/rest/scale, then test transition interruption, feet, weapon hands, target deformation, and style from the gameplay camera.

## Technical issue register

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| MIX-OWN-001 | moderate | Scope: 52 exact paths listed in the nine linked constituent issue rows; reproduce: archive-level animation-owned XZ lint; impact: a package-wide policy can leave intended movement stationary. Guidance: not applicable. | engine-config | Action: choose ownership per file and state; acceptance: declared lint and one target controller apply movement/yaw/collision exactly once for every admitted state; residual: unresolved. | Generic rewrite is not justified without intent. | `observed-animsmith`; 52 current findings. |
| MIX-XPACK-001 | major | Scope: every proposed cross-pack state/layer boundary; reproduce: no current pairwise hierarchy/rest/scale or runtime transition test exists; impact: a shared controller may pop, deform, slide, misalign weapons, or change style. Guidance: not applicable. | unknown | Action: select exact pairs, compare skeleton/rest/scale, then run full-body target-engine transitions before considering layers; acceptance: technical, contact, deformation, and artistic gates pass per pair; residual: unresolved. | Deterministic comparison can support evidence but cannot decide artistic compatibility. | `not-evaluated`; current source mechanics only. |

## Engine status

| Runtime | Evidence level | Technical result | Remaining gate |
|---|---|---|---|
| Unity unspecified | not-evaluated | No current collection import, controller, or visual test | Test exact pairwise full-body state graph |
| Unreal Engine unspecified | not-evaluated | No current collection import, retarget, or Blend Space test | Test exact IK Rig/Skeleton and state graph |
| Godot unspecified | not-evaluated | No current collection import or AnimationTree test | Test exact Skeleton3D and state graph |
| Bevy unspecified | not-evaluated | No current collection loader or graph test | Test exact animation targets and state graph |

## Fit and limitations

Best fit is source intake for projects willing to choose one constituent/set at a time, own movement, and validate full-body state changes. Poor fits include drop-in cross-pack blending, shared root-motion policy, weapon layering, motion matching, or production use without target-engine and artistic review.

The nine constituent reports are authoritative for their fresh source findings and new bounded hypotheses. This rollup does not recreate historical vendor mappings or claim that common role labels/bone counts establish compatibility.

## Changes between AnimSmith versions

AnimSmith 0.14.0 — Fresh source-only matrix ran 996 commands over 249 FBXs, preserved identical before/after inventory, and added explicit new constituent/controller and cross-pack hypotheses without claiming old membership authority.

AnimSmith 0.10.0 — Retained historical finding totals agree but supply no current collection semantics or engine acceptance. AnimSmith 0.7.0 collection conclusions remain superseded.

## Evidence status

Current evidence covers nine constituents, 249 physical FBXs, and 119,754,377 bytes under evaluator revision `e8321ad40be5ef6f162b31f085819c039175c3c9`. Legacy metadata says 231 motions but does not provide an authoritative 231-to-249 mapping. Source inventory, ledger, summary, and report-data digests are in the appendix; licensed data remains external. See the [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder).

## Sources

- The nine linked current constituent reports and fresh external AnimSmith 0.14.0 evidence.
- [AnimSmith game-ready clip guidance](../game-ready-clips.md).
