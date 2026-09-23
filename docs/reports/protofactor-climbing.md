# Animation pack evaluation: Protofactor Climbing Animset

> Technical verdict: **Usable with conditions**
>
> Evaluation completeness: **partial** — all source files and declared checks were reviewed; engine playback and visual quality remain untested.
>
> Confidence: **medium**
>
> Evaluation date: **2026-09-16**
>
> Current evaluator: **AnimSmith 0.14.0**
>
> Report format: **3**
>
> Detailed evidence: [Protofactor Climbing evidence](protofactor-climbing-evidence.md)

## Technical decision

**Prototype decision:** Try separate wall and ladder states with controller-owned movement. Configure cyclic policy, wall distance and contacts; source correction depends on reviewed motion.

**Camera scope:** Full-body humanoid animations; appearance on your target character and camera is untested. Dedicated first-person arms/viewmodel use is unverified.

This animset is one of eight locally evaluated parts of [Ultimate Animation Collection](protofactor-ultimate-animation-collection.md). The local asset revision is unknown; the collection report separates evaluated files from the dated vendor listing.

**Content in evaluated inventory:** Wall movement and ladder movement. Wall directions and ladder up/down are selected; ledge, jump and obstacle actions are outside the selected runtime sets. These are identified motions; gameplay behavior has not been approved.

**A conditional traversal source, not a drop-in climbing controller.** The selected wall set supplies eight named directions with equal durations; the ladder pair is a separate topology. Use controller-owned surface movement with the selected stationary-root files, or specify how root-motion travel and collision will work. Hand/foot contact, wall distance and ledge transitions remain acceptance gates.

`observed-animsmith`: the official 0.14.0 evaluator inspected and measured all 77 FBXs; 34/75 declared-contract files pass and 41 fail per output format. The findings below separate source problems, project settings and untested gameplay. Transformed outputs still need gameplay and visual checks.

## Capability coverage

### Content present

Filename-classified wall movement offers eight directions; ladder up/down forms a separate two-member candidate. The wider inventory also names obstacle and jump actions.

### Content gaps and unknowns

Entry, top-out and fall/interrupt state paths are not classified as accepted runtime sets. Wall geometry and hand/foot contact requirements are project dependent.

### Evaluation still needed

Test cyclic intent, reversals, surface alignment, collision and contact on actual wall and ladder geometry. Target-engine, IK and artistic acceptance remain open.

## Runtime sets and authored motion

Use each row as a separate blend or sequence proposal. The linked appendix names its exact clips, timings and measurements; controller playback and contact quality still need testing.

| Set | Controller use | Adoption decision | Exact members |
|---|---|---|---|
| Wall eight directions | Separate traversal state | Traversal prototype. All ten selected wall/ladder members fail declared-loop contracts; pruning did not clear the WallClimbUp loop findings. Review true cycles and scene contacts ([CL-LOOP](#technical-issue-register)). | [8 exact members](protofactor-climbing-evidence.md#exact-runtime-members) |
| Ladder up and down | Separate traversal state | Ladder prototype with controller travel. Both are among the ten selected motions that fail their declared loop checks; review cyclic intent, seams and rung contacts before repetition. | [2 exact members](protofactor-climbing-evidence.md#exact-runtime-members) |

## Integration recipe

1. **Members/topology:** Use the wall set in the wall plane, with horizontal/vertical input. Use ladder up/down as a separate state; select obstacle-height and wall-jump actions discretely.
2. **Timing/synchronization:** Establish intended cyclic playback and support-contact timing before choosing synchronization. Equal durations do not prove matching hand/foot support phases. Check neighboring wall directions at intermediate weights and ladder reversals on actual geometry.
3. **State ownership:** The selected root trajectories are stationary. The controller owns wall projection, vertical displacement and collisions. Do not use horizontal-only speed as a measure of a root-motion climb.
4. **Composition constraints:** Keep arm/leg coupling and solve scene contact constraints. Locomotion weapon masks are not validated for climbing.
5. **Acceptance gate:** Test entry, top-out, corner changes, interruption/fall and varying wall/ladder dimensions before treating this as a traversal system.

## Technical issue register

Severity describes impact as delivered for the stated use. Residual status is explicit; untested risks are not confirmed artist defects.

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| CL-LOOP | major | All ten selected wall/ladder members fail their declared-loop contracts; `Humanoid@WallClimbUp.fbx` still fails its declared checks after pruning. Cyclic traversal requires stable seams and contacts. [Readiness guidance](../game-ready-clips.md#the-readiness-ladder). | artist-author | Review genuine climb loops, correct endpoint pose/velocity and preserve contact timing. Pruning did not remove the measured loop findings. Repeating these motions still needs reviewed source loops and engine contact tests. | Current pruning reduces redundant tracks, not contact or loop defects. | `Take 001` and current output lint; `observed-animsmith`. |
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
