# Animation pack evaluation: Protofactor Climbing

> Technical verdict: **Usable with conditions**
>
> Evaluation completeness: **partial** — AnimSmith 0.4.0 re-evaluated all 77 delivered FBXs and 75 contracts, adding measured vertical/yaw root trajectory to the retained Unity probe and new Unreal/Godot import-advice attempts, but environment contacts, engine root-motion extraction, retarget, and motion quality were not visually accepted, and Bevy had no candidate to probe.
>
> Confidence: **high**
>
> Evaluation date: **2026-08-21**
>
> Report format: **1**
>
> Detailed evidence: [Protofactor Climbing evidence](protofactor-climbing-evidence.md)

## Technical decision

Use this as a traversal candidate library, not an out-of-the-box controller. The pack has useful paired in-place/root-motion wall, ladder, obstacle, and jump families on the collection's standard skeleton, and 74 of 75 individual motions import as Unity Humanoid clips. However, 41 declared contracts fail: every one of 36 loop-declared traversal files has a strict seam finding, 16 of those are one-shot-like obstacle/jump files that should not loop, and the `FallingUnarmed` outlier exposes no Unity AnimationClip.

This is a 0.4.0 refresh of the published 0.3.0 evaluation, not a new source: re-inventory reproduces the manifest exactly (0 added, 0 removed, 0 changed across all 77 FBXs) and the archive digest re-verifies. The pack's biggest change is that AnimSmith now measures vertical and yaw root trajectory alongside horizontal speed (`root_trajectory`, 76/77 clips): 24 clips move more than 1 cm horizontally, 51 are stationary, and 4 carry more than 1° of net yaw. Named exemplars land on their authored metre names — `Humanoid@ClimbUp2MetersObstacleUnarmed_RM.FBX` at +2.000 m, `Humanoid@ExitWallTop_RM.fbx` at +2.000 m, `Humanoid@EnterWallTop_RM.fbx` at -1.950 m with a -180° net yaw turn-to-face, `Humanoid@WallJumpUp_RM.fbx` at +1.903 m, and the ladder-top pair `Humanoid@EnterLadderTopUnarmed_RM.FBX`/`Humanoid@ExitLadderTopUnarmed_RM.FBX` at -1.500 m/+1.500 m — retiring the prior horizontal-only blind spot and closing [issue #408](https://github.com/mmannerm/animsmith/issues/408). These are sampled regression facts on the shared uniform metric grid, not continuous-curve extrema and not proof of what any engine's own root-motion extraction will do with the same file. Environment alignment, hand/foot contact quality, ledge height, root authority, and transition windows remain gameplay-specific gates unresolved by this measurement.

## Capability coverage

### Complete core

- Seventy-five individual files cover wall and ladder movement, obstacle climbs, wall jumps, enter/exit actions, airborne/landing clips, holds, and preparation transitions.
- Twenty-eight logical motions have paired in-place/root-motion files; the standard 74-file family shares the collection's 56-bone skeleton.

### Partial supporting gameplay

- Eight-way wall movement and ladder up/down cycles are structurally present, but every delivered traversal loop needs seam review; vertical and yaw root trajectory are now measured per clip, but engine root-motion extraction and environment contact remain unverified.
- Airborne and landing alternatives exist; one duplicate-looking falling variant is a rig/import outlier.

### Absent

- No locomotion approach, combat, injured, contact/event/IK, ledge probe, motion warping, root-authority, or network-correction contract.
- No native Unreal, Godot, or Bevy delivery and no tested target-character retarget.

## Runtime sets and authored motion

| Set/profile | Role or coordinate | Exact members | Variant/type | Timing or motion | Runtime contract |
|---|---|---|---|---|---|
| Wall-climb 8-way | up | IP `Humanoid@WallClimbUp.fbx`; RM `Humanoid@WallClimbUp_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.000 m/s | loop_ip=true; loop_rm=true; sync=phase; movement=animation |
| Wall-climb 8-way | down | IP `Humanoid@WallClimbDown.fbx`; RM `Humanoid@WallClimbDown_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.000 m/s | loop_ip=true; loop_rm=true; sync=phase; movement=animation |
| Wall-climb 8-way | left | IP `Humanoid@WallClimbLeft.fbx`; RM `Humanoid@WallClimbLeft_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.637 m/s | loop_ip=true; loop_rm=true; sync=phase; movement=animation |
| Wall-climb 8-way | right | IP `Humanoid@WallClimbRight.fbx`; RM `Humanoid@WallClimbRight_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.637 m/s | loop_ip=true; loop_rm=true; sync=phase; movement=animation |
| Ladder up/down | up | IP `Humanoid@ClimbUpLadder.fbx`; RM `Humanoid@ClimbUpLadder_RM.fbx` | variant=paired-ip-rm | duration=1.200 s; rm_speed=0.000 m/s | loop_ip=true; loop_rm=true; sync=phase; movement=animation |
| Ladder up/down | down | IP `Humanoid@ClimbDownLadder.fbx`; RM `Humanoid@ClimbDownLadder_RM.fbx` | variant=paired-ip-rm | duration=1.200 s; rm_speed=0.000 m/s | loop_ip=true; loop_rm=true; sync=phase; movement=animation |
| Obstacle alternatives | half-meter left | IP `Humanoid@ClimbUpHalfMeterObstacleLeftUnarmed.fbx`; RM `Humanoid@ClimbUpHalfMeterObstacleLeftUnarmed_RM.fbx` | variant=paired-ip-rm | duration=1.100 s; rm_speed=0.407 m/s | loop_ip=false; loop_rm=false; sync=start; movement=animation |
| Obstacle alternatives | one meter | IP `Humanoid@ClimbUp1MeterObstacleUnarmed.FBX`; RM `Humanoid@ClimbUp1MeterObstacleUnarmed_RM.FBX` | variant=paired-ip-rm | duration=1.300 s; rm_speed=0.360 m/s | loop_ip=false; loop_rm=false; sync=start; movement=animation |
| Obstacle alternatives | two meters | IP `Humanoid@ClimbUp2MetersObstacleUnarmed.FBX`; RM `Humanoid@ClimbUp2MetersObstacleUnarmed_RM.FBX` | variant=paired-ip-rm | duration=3.433 s; rm_speed=0.158 m/s | loop_ip=false; loop_rm=false; sync=start; movement=animation |

Diagonal wall directions and mirrored half-meter/right, wall-jump, entry/exit, hold, and fall/land candidates remain inventoried in the appendix. `rm_speed` above is horizontal-only; AnimSmith 0.4.0 separately measures vertical and yaw root trajectory for 76/77 clips (issue register and appendix), so a zero horizontal `rm_speed` no longer implies zero net displacement.

## Integration recipe

1. **Members/topology:** `topology=environment-traversal-state-machine`; select one movement authority per paired family and treat obstacles/jumps as discrete actions.
2. **Timing/synchronization:** `sync=contact-windows`; override loop=false for one-shot obstacles/jumps, visually inspect all retained cycle wraps, and gate transitions at authored hand/foot/ledge contacts; see [loop guidance](../game-ready-clips.md#the-loop-pops).
3. **State ownership:** `owner=traversal-controller`; gameplay owns probe results, surface frame, entry side, cancellation, gravity, and recovery; consult each clip's measured horizontal/vertical/yaw `root_trajectory` facts (issue register CL-002) before assigning RM vs. code authority per family instead of assuming zero horizontal speed means zero net displacement.
4. **Composition constraints:** `composition=full-body-root-exclusive`; never apply RM and code displacement together; do not upper-body-mask contact traversal.
5. **Acceptance gate:** `gate=environment-matrix`; test every direction/height against representative geometry, both motion authorities, camera/facing, target rigs, and failure/cancellation paths.

## Technical issue register

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| CL-001 | major | All 36 loop-declared traversal files fail at least one strict seam check; a retained cycle may pulse or pop, while 16 one-shot-like obstacle/jump files may restart incorrectly; [loop guidance](../game-ready-clips.md#the-loop-pops). | engine-config | Disable one-shot loops and visually accept or re-author every retained cycle. | Current diagnostics identify seams; semantic loop intent and repair require acceptance. | High mechanical; visual severity open. |
| CL-002 | major | [RM/IP movement-authority choice](../game-ready-clips.md#the-character-glides-or-runs-in-place) is not settled by measurement alone: 0.4.0 reports horizontal, vertical, and yaw `root_trajectory` per clip (76/77), but each is a sampled regression fact on the metric grid, not a curve extremum or proof of an engine's own root-motion extraction; choosing the wrong authority can still stall or double-move the character. | engine-config | Cross-check the measured per-clip facts against the target engine's extracted root motion before declaring movement ownership; keep RM and code displacement exclusive. | Displacement/yaw measurement shipped in 0.4.0, closing [issue #408](https://github.com/mmannerm/animsmith/issues/408); remaining environment-alignment and contact-quality gates are engine/gameplay work. | `root_trajectory` measured on 76/77 clips: 24 move >1 cm horizontally, 51 stationary, 4 with >1° net yaw; per-family ranges are in the appendix. |
| CL-003 | major | `Humanoid@FallingUnarmed.FBX` has a distinct 58-bone signature and exposes no Unity AnimationClip, so silent inclusion can break an airborne state. Guidance: not applicable. | vendor-license | Exclude it until the author supplies or identifies a usable clip. | Channel coverage can surface anomalies; missing source animation needs author clarification. | Reproduced in file inspection and Unity; reconfirmed unchanged under the 0.4.0 re-inventory. |
| CL-004 | major | No ledge/surface/contact/event contract exists, so hands/feet may miss and root motion may penetrate or drift from geometry. Guidance: not applicable. | artist-author | Author probes, offsets, warping, contacts, events, and cancellation policy per game. | Contact/transition diagnostics may assist but cannot infer environment intent. | Inventory-confirmed; visual scene pass absent. |
| CL-005 | minor | 8,753 constant-track notes across 75 individual files increase export/retarget overhead; [optimization guidance](../game-ready-clips.md#the-file-is-bloated-or-the-retargeter-chokes). | animsmith-current-declared | Retain sources; prune only after equivalence tests. | Current pruning is available; bounded by open [issue #401](https://github.com/mmannerm/animsmith/issues/401). | One sample shrank 563,984 to 71,288 bytes; seam remained; re-run under 0.4.0 reproduces the same trial. |
| CL-006 | moderate | The current listing says 69 animations and 18 RM, while the local archive has 75 individual files and 28 `_RM` files, so purchase expectations and automation counts can diverge. Guidance: not applicable. | vendor-license | Treat the hashed local inventory as evaluation scope and ask the vendor to reconcile the listing. | Report tooling can flag listing/delivery differences, not resolve provenance. | High for current pages/local files. |

## Engine status

| Runtime | Evidence level | Technical result | Remaining gate |
|---|---|---|---|
| Unity 6000.3 / 6000.5.8f1 | Retained headless import/sampling (6000.5.8f1, 2026-08-17) plus `generate import-advice` under the frozen `unity-humanoid` rev 1 / 6000.3 / `fbx-model-importer` profile (2026-08-21). | 74/75 individual clips import as Humanoid; 5/5 required samples and a Basic-to-Climbing mixer pass; `FallingUnarmed` exposes no clip as expected (retained). Import-advice: `available` (exit 0); each clip's lock-root settings derive from the delivered `.fbx.meta` (`useFileUnits:1`; `lockRoot*` absent on every meta, so Unity's default `false` applies, mapping to `extract`). | Visual contacts, in-Editor displacement/yaw acceptance, controller, target rig, compression, build; advice only, not observed behavior. |
| Unreal Engine 5.8 | `generate import-advice` under `unreal` revision 1 (2026-08-21) plus documentation review. | Typed refusal `profile_settings_unmodeled` (exit 1): revision 1 has no modeled Unreal setting vocabulary. Root motion, state machines, and montages can express the policy; no native package is supplied. | FBX import/retarget, root lock/authority, warping, contacts, build. |
| Godot 4.7 | `generate import-advice` under `godot` revision 1 (2026-08-21) plus documentation review. | Typed refusal `profile_settings_unmodeled` (exit 1): revision 1 has no modeled Godot setting vocabulary. AnimationTree can express transitions, one-shots, and state playback. | Conversion/import, root extraction, environment controller, export. |
| Bevy 0.19.0 | Not evaluated (2026-08-21); documentation review only. | No generated glTF/GLB candidate exists: no in-place `gait-group` ring (`not_applicable` on all 75 contracts), so `generate addressability` had nothing to inventory — a coverage gap, not a Bevy failure. AnimationGraph supports blending/masks; conversion and traversal control remain project work. | glTF conversion, mapping, root policy, contacts, performance. |

## Fit and limitations

Best fit: third-person games willing to build an environment-aware traversal controller around a broad animation library and visually tune contacts.

Poor fit: drop-in climbing, first-person arms, procedural geometry without motion warping, or networked root motion without an authority/correction design. Full-body contact motion should not be split by upper/lower masking.

The standard motions combine technically with Basic Locomotion, Sword & Shield, Campfire, and Injured: skeleton signatures align, pairwise shared paths are byte-identical, and the five-pack Unity project co-imports. Basic can own approach/departure and Sword can own combat, but weapon sheathing, hand release, surface entry, style, and timing remain untested; see the [partial collection report](protofactor-ultimate-animation-collection.md).

## Evidence status

The evaluation covers 77 FBXs: 75 individual motions, one combined take, and one actor. This 2026-08-21 refresh uses AnimSmith 0.4.0 (tag `v0.4.0`, revision `6b37ad636b198ef8ff47fadbf6a3a51eb1a27c8e`, binary SHA-256 `fd1eee57407aa02db88763d144389a7f5104204c40ddfbb28eb5885ca8cd54c6`, output schema v10, measurements schema v15) against the same source: re-inventory reproduces the published manifest exactly (0 added, 0 removed, 0 changed) and the archive digest re-verifies, so this is a pure evaluator-version refresh. Historical results from AnimSmith 0.3.0 (revision `aabac28edf2719db236068339f1208bbf156d0bb`) and the 2026-08-17 Unity 6000.5.8f1 headless probe are retained as dated evidence and keep their original date because the source is byte-identical. This report uses manifest schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1` and the [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder). The local archive was supplied as a Protofactor-site download; current store/license pages establish context, not its revision or transaction rights. Exact evidence and limitations are in the [appendix](protofactor-climbing-evidence.md).

## Sources

- Protofactor, [Animset: Climbing](https://protofactor.biz/product/animset-climbing/) and [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/) — current product scope and collection membership.
- Protofactor, [End User License Agreement](https://protofactor.biz/end-user-license-agreement/) — current vendor-site license terms; not legal advice.
- Unity, [Root Motion](https://docs.unity3d.com/6000.5/Documentation/Manual/RootMotion.html) and [looping clips](https://docs.unity3d.com/6000.5/Documentation/Manual/LoopingAnimationClips.html); Epic Games, [root motion](https://dev.epicgames.com/documentation/unreal-engine/root-motion-in-unreal-engine?application_version=5.8); Godot, [AnimationTree](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html); Bevy, [Animation Masks](https://bevy.org/examples/animation/animation-masks/) — runtime capability context only.
- AnimSmith, [Unity 6000.3 animation profile](../engine-profile-unity.md), [Unreal Engine 5.8 animation profile](../engine-profile-unreal.md), [Godot 4.7 animation profile](../engine-profile-godot.md), and [Bevy 0.19.0 animation profile](../engine-profile-bevy.md) — modeled 0.4.0 import-advice profile facts, accessed 2026-08-21.
