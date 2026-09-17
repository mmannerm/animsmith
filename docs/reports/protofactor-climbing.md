# Animation pack evaluation: Protofactor Climbing Animset

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
> Detailed evidence: [Protofactor Climbing evidence](protofactor-climbing-evidence.md)

## Technical decision

**A conditional traversal source, not a drop-in climbing controller.** The selected wall set supplies eight named directions with equal durations; the ladder pair is a separate topology. Use kinematic surface-relative movement with the selected stationary-root files, or an explicit RM policy. Hand/foot contact, wall distance and ledge transitions remain acceptance gates.

`observed-animsmith`: the official 0.14.0 evaluator inspected and measured all 77 FBXs; 34/75 declared-contract files pass and 41 fail per output format. Conditions below distinguish source findings, evaluator policy and untested gameplay. No remediation candidate is promoted.

## Capability coverage

### Complete core

- No complete gameplay core has been validated on a target game controller. Candidate content and integration scope are listed below; completed file checks do not establish gameplay completeness.

### Partial supporting gameplay

- Eight wall directions and a separate ladder up/down pair are concrete traversal candidates. Scene alignment, contact locking, entry/top-out and collision behavior remain open.

### Absent

- No current IK, retarget, engine, or artistic acceptance is established.

## Runtime sets and authored motion

New evaluator-selected generic scenarios: current source bytes establish the exact members, take names, durations, and measured roots below. Names suggest gameplay roles; topology and semantic intent are hypotheses requiring clip review. These are not reconstructed historical manifests or measured collection-output sets. Every listed member uses `Take 001`; full identities are in the external selected-set ledger.

| Set/profile | Role or coordinate | Exact members | Variant/type | Timing or motion | Runtime contract |
|---|---|---|---|---|---|
| Wall eight directions | Proposed up (0,1) | `Humanoid@WallClimbUp.fbx::Take 001` | set_type=directional-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Wall eight directions | Proposed up-left (-1,1) | `Humanoid@WallClimbUpLeft.fbx::Take 001` | set_type=directional-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Wall eight directions | Proposed left (-1,0) | `Humanoid@WallClimbLeft.fbx::Take 001` | set_type=directional-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Wall eight directions | Proposed down-left (-1,-1) | `Humanoid@WallClimbDownLeft.fbx::Take 001` | set_type=directional-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Wall eight directions | Proposed down (0,-1) | `Humanoid@WallClimbDown.fbx::Take 001` | set_type=directional-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Wall eight directions | Proposed down-right (1,-1) | `Humanoid@WallClimbDownRight.fbx::Take 001` | set_type=directional-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Wall eight directions | Proposed right (1,0) | `Humanoid@WallClimbRight.fbx::Take 001` | set_type=directional-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Wall eight directions | Proposed up-right (1,1) | `Humanoid@WallClimbUpRight.fbx::Take 001` | set_type=directional-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Ladder up and down | Proposed up (0,1) | `Humanoid@ClimbUpLadder.fbx::Take 001` | set_type=directional-blend | duration=1.200 s | loop=unknown; movement=controller; contact=not-evaluated |
| Ladder up and down | Proposed down | `Humanoid@ClimbDownLadder.fbx::Take 001` | set_type=directional-blend | duration=1.200 s | loop=unknown; movement=controller; contact=not-evaluated |

## Integration recipe

1. **Members/topology:** `topology=directional-blend`; use the wall set in the wall plane, with horizontal/vertical input. Use ladder up/down as a separate state; select obstacle-height and wall-jump actions discretely.
2. **Timing/synchronization:** `sync=not-evaluated`; establish intended cyclic playback and support-contact timing before choosing synchronization. equal durations do not prove matching hand/foot support phases. Check neighboring wall directions at intermediate weights and ladder reversals on actual geometry.
3. **State ownership:** `owner=controller`; the selected root trajectories are stationary. The controller owns wall projection, vertical displacement and collisions. Do not use horizontal-only speed as a measure of an RM climb.
4. **Composition constraints:** `composition=full-body`; keep arm/leg coupling and solve scene contact constraints. Locomotion weapon masks are not validated for climbing.
5. **Acceptance gate:** `gate=engine-and-visual-review`; test entry, top-out, corner changes, interruption/fall and varying wall/ladder dimensions before treating this as a traversal system.

## Technical issue register

Severity describes impact as delivered for the stated use. Residual status is explicit; untested risks are not confirmed artist defects.

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| CL-LOOP | major | All ten selected wall/ladder members fail their declared-loop contracts; `Humanoid@WallClimbUp.fbx` remains non-clean after pruning. Cyclic traversal requires stable seams and contacts. [Readiness guidance](../game-ready-clips.md#the-readiness-ladder). | artist-author | Review genuine climb loops, correct endpoint pose/velocity and preserve contact timing. Candidate adoption remains unresolved; pruning is unpromoted and the measured loop findings persist. | Current pruning reduces redundant tracks, not contact or loop defects. | `Take 001` and current output lint; `observed-animsmith`. |
| CL-POLICY | minor | Loop-policy failures on obstacle one-shots can be expected; they must not all be reported as broken loops. [Readiness guidance](../game-ready-clips.md#the-readiness-ladder). | engine-config | Use non-looping obstacle actions where intended; require entry/exit continuity instead of wrap continuity. Residual: unresolved policy. | Per-clip declarations exist; project semantics remain explicit. | For example `Humanoid@ClimbUp1MeterObstacleUnarmed.FBX`; `observed-animsmith`, one-shot role `inferred`. |
| CL-CONTACT | major | No current test establishes wall distance, rung spacing or hand/foot lock at blended weights. [Readiness guidance](../game-ready-clips.md#the-readiness-ladder). | engine-config | Validate selected sets against target geometry and collision movement. Escalate authored contact defects to the artist with clip/time/geometry reproduction. Residual: unknown, not a demonstrated vendor defect. | Future generic contact diagnostics may help; no automatic climbing controller is claimed. | Scene/contact acceptance `not-evaluated`. |

## Engine status

| Runtime | Evidence level | Technical result | Remaining gate |
|---|---|---|---|
| Unity unspecified | not-evaluated | No current import or traversal playback. | Controller, contact, visual, and build tests. |
| Unreal Engine unspecified | not-evaluated | No current import or traversal playback. | Retarget, IK, contact, and build tests. |
| Godot unspecified | not-evaluated | No current conversion or playback. | Conversion/import and graph tests. |
| Bevy unspecified | not-evaluated | No current handoff or runtime test. | glTF handoff and runtime test. |

## Fit and limitations

Basic Locomotion can provide the approach/ground state, with an explicit transition into constrained traversal. Shared naming or rig conventions do not prove top-out/ground pose continuity. A wall loop cannot supply environment detection, capsule placement or a missing transition by itself.

These are proposed integration decisions (`inferred`), with target-controller, contact and artistic acceptance still `not-evaluated`.

## Changes between AnimSmith versions

AnimSmith 0.14.0 — reran 77 input baselines, 75 per-file declared contracts and one remediation trial using the official release. Added current exact-member generic scenarios and developer/artist actions. AnimSmith 0.10.0 — superseded historical evidence; none of its generated outputs or engine results is relabelled as a fresh run.

## Evidence status

Current evidence uses the official release and the [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder); commercial artifacts remain external.

## Sources

- Protofactor, [Animset: Climbing](https://protofactor.biz/product/animset-climbing/) — product context.
- AnimSmith, [CLI reference](../cli.md) and [game-ready clips](../game-ready-clips.md) — evaluation boundaries.
