# Animation pack evaluation: Protofactor One-Handed Melee Animset

> Technical verdict: **Usable with conditions**
>
> Evaluation completeness: **partial** — source files, declared checks and transform trials were reviewed, with limited Unity source playback; target-character visuals, contacts, retargeting and the game controller remain untested.
>
> Confidence: **medium**
>
> Evaluation date: **2026-09-16**
>
> Current evaluator: **AnimSmith 0.14.0**
>
> Report format: **3**
>
> Detailed evidence: [Protofactor One-Handed Melee Animset evidence appendix](protofactor-one-handed-melee-evidence.md)

## Technical decision

**Prototype decision:** Try one in-place or root-motion gait variant at a time after loop-policy review. Continuous gait defects require artist or source correction if confirmed. The 73-bone block/guard animation differs from the common 56-bone rig and needs an isolated retarget/import check. In-place clips leave travel to the controller; root-motion clips carry authored travel.

**Camera scope:** Full-body humanoid animations; appearance on your target character and camera is untested. Dedicated first-person arms/viewmodel use is unverified.

This animset is one of eight locally evaluated parts of [Ultimate Animation Collection](protofactor-ultimate-animation-collection.md). The local asset revision is unknown; the collection report separates evaluated files from the dated vendor listing.

**Content in evaluated inventory:** Armed walk, run, crouch and equipment/actions. Evaluated filenames include airborne, attacks and reactions; these have no accepted controller topology here. These are identified motions; gameplay behavior has not been approved.

AnimSmith 0.14.0 reads all 113 delivered FBX candidates. The untouched baseline has no errors; it reports 13629 constant-track notes. Under the intended per-file project settings, 23/110 motion files pass and 87 fail, mainly because loop settings need review. Set confirmed one-time actions to play once before treating their wrap seams as source defects. For intended repeating gaits, review the loop policy and correct actual source seams.

All 25 transform trials passed file inspection and measurement. All transformed outputs still fail their declared checks. For a target-engine pilot, first correct the one-shot loop settings and secure one gait set that passes its intended checks. Production use still requires blending, foot-contact, movement and visual approval.

## Capability coverage

### Content present

Filename-classified walk, run and crouch combat gaits have separate in-place and root-motion candidates. The evaluated role inventory also includes airborne, actions and reactions.

### Content gaps and unknowns

Additive aim, paired interactions and dedicated viewmodel content have no established classification. The 73-bone block/guard animation differs from the common 56-bone family and needs its own import/retarget check.

### Evaluation still needed

Redeclare one-shots, resolve continuous gait contracts, and test blend coordinates, root ownership, grip/contact and target-character playback. Named Unity source executions do not accept the full controller.

## Runtime sets and authored motion

Use each row as a separate blend or sequence proposal. The linked appendix names its exact clips, timings and measurements; controller playback and contact quality still need testing.

| Set | Controller use | Adoption decision | Exact members |
|---|---|---|---|
| `crouch-combat-8-way-in-place` | Crouch eight directions; controller travel | Crouch prototype with controller travel. Its declared checks still fail. Confirm which motions should loop, correct any genuine seam, and test foot contacts before using the full blend. | [8 exact members](protofactor-one-handed-melee-evidence.md#exact-runtime-members) |
| `crouch-combat-8-way-root-motion` | Crouch eight directions; animation travel; measured root-motion 0.480–0.730 m/s | Crouch prototype with animation travel. Its declared checks still fail. Confirm which motions should loop, correct any genuine seam, and test foot contacts before using the full blend. | [8 exact members](protofactor-one-handed-melee-evidence.md#exact-runtime-members) |
| `run-8-way-in-place` | Run eight directions; controller travel | Run prototype with controller travel. Its declared checks still fail. Confirm which motions should loop, correct any genuine seam, and test foot contacts before using the full blend. | [8 exact members](protofactor-one-handed-melee-evidence.md#exact-runtime-members) |
| `run-8-way-root-motion` | Run eight directions; animation travel; measured root-motion 1.905–2.117 m/s | Run prototype with animation travel. Its declared checks still fail. Confirm which motions should loop, correct any genuine seam, and test foot contacts before using the full blend. | [8 exact members](protofactor-one-handed-melee-evidence.md#exact-runtime-members) |
| `walk-combat-8-way-in-place` | Walk eight directions; controller travel | Walk prototype with controller travel. Its declared checks still fail. Confirm which motions should loop, correct any genuine seam, and test foot contacts before using the full blend. | [8 exact members](protofactor-one-handed-melee-evidence.md#exact-runtime-members) |
| `walk-combat-8-way-root-motion` | Walk eight directions; animation travel; measured root-motion 0.491–0.951 m/s | Walk prototype with animation travel. Its declared checks still fail. Confirm which motions should loop, correct any genuine seam, and test foot contacts before using the full blend. | [8 exact members](protofactor-one-handed-melee-evidence.md#exact-runtime-members) |

The appendix gives timings, travel speeds and phase measurements for each motion. Both eight-file `run-8-way` alternatives span 0.600–0.667 s; the detailed table reports each variant’s minimum. Use in-place and root-motion as separate movement ownership choices; neither variant has accepted contact synchronization.

## Integration recipe

1. **Members/topology:** Select one eight-member in-place or root-motion row from the appendix and map forward/cardinal/diagonal coordinates explicitly.
2. **Timing/synchronization:** Loop only confirmed continuous states, keep named one-shots at `loop=false`, and require phase/contact review before synchronized blending.
3. **State ownership:** Choose controller translation for in-place clips or animation translation for root-motion clips per state, with one owner for translation, yaw, and collision.
4. **Composition constraints:** Keep weapon combat states namespaced to this pack and require separate mask/socket/IK evidence before layering.
5. **Acceptance gate:** Require clean intended contracts plus target-engine import, full blend-space playback, foot/weapon contact, and artist review.

## Technical issue register

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| PF1-PHASE | note | The three selected in-place gait sets have measured phase differences (see appendix). Foot-contact timing and intended synchronization are unknown, so the measurements alone do not show a source defect. [Guidance](../game-ready-clips.md#the-readiness-ladder). | unknown | Obtain project or vendor loop/contact intent, then sweep the exact blend coordinates against a declared contact tolerance. Use engine phase markers or declared alignment only after intent is established; request artist cleanup only for a confirmed source-motion problem. Residual unresolved. | Current phase measurements inform a declared alignment trial; they do not prove contact or visual acceptance. | `observed-animsmith` phases; synchronization/contact acceptance `not-evaluated`. |
| PF1-01 | major | `Humanoid@RunForward1hMelee.fbx`, `Humanoid@RunForward1hMelee_RM.fbx` and their eight-direction sets fail the intended per-file checks; none of the selected gait sets is ready for a production blend. [Guidance](../game-ready-clips.md#the-readiness-ladder) | artist-author | Re-export intended repeating gaits with matching endpoint pose and velocity, or document a play-once setting. All eight members must pass their intended checks, then pass visual and contact review across the blend. Unresolved. | AnimSmith can validate and compare candidates, but cannot infer intended contact phase or approve visible motion. | `observed-animsmith`; current exact-file evidence, high confidence mechanically |
| PF1-02 | moderate | `Humanoid@AttackA1hMelee.fbx` and other attacks, reactions, parries or emotes are declared as loops. Their wrap-seam findings may reflect the project setting for one-time actions rather than bad source motion. [Guidance](../game-ready-clips.md#the-readiness-ladder) | engine-config | Set confirmed one-time actions to `loop=false`; keep looping only where the project or vendor intends it. Recheck the settings and play each transition once. Unresolved. | AnimSmith may suggest a play-once setting, but the project or vendor must confirm intent. | `observed-animsmith`; declaration defect distinguished from source-animation quality |

## Engine status

| Runtime | Evidence level | Technical result | Remaining gate |
|---|---|---|---|
| Unity 6000.5.8f1 | source-controller-feasibility | Fresh import and standalone sampling passed for `IdleCombat1hMelee`, `WalkForwardCombat1hMelee`, and `WalkForwardCombat1hMelee_RM`; `basic-idle-to-onehand-idle`, `sword-shield-idle-to-onehand-idle`, and `basic-walk-ip-to-onehand-walk-ip` produced finite mixer poses with fixed gameplay-owned root; the root-motion walk produced finite 1.215074 owner translation with animation-owned root. | Inspect rendered motion, contacts, deformation, authored AnimatorController behavior, and a player build; do not generalize beyond named sources. |
| Unreal Engine unspecified | not-evaluated | No current import or retarget result. | Import, retarget, root-motion, blend, and visual tests. |
| Godot unspecified | not-evaluated | No current conversion or playback result. | Convert/import and test animation graph, roots, contacts, and visuals. |
| Bevy unspecified | not-evaluated | No current conversion or playback result. | Convert/load and test graph, root ownership, performance, and visuals. |

## Fit and limitations

Best fit is a full-body combat prototype whose controller can explicitly select one gait variant and keep pack-local state names. It is a poor fit for immediate production admission, motion matching, layered combat, or retargeted characters without the missing engine and human gates.

The fresh Unity probe establishes finite source playback and the named cross-pack transitions for `IdleCombat1hMelee`, `WalkForwardCombat1hMelee`, and `WalkForwardCombat1hMelee_RM`; it does not establish visual quality, contact, phase, retargeting, or broad pack compatibility. Keep states namespaced and retain explicit root ownership.

## Changes between AnimSmith versions

AnimSmith 0.14.0 — revalidated the unchanged retained source inventory, per-file declarations, measured runtime hypotheses, and every previously recommended bounded transform. Current transformed outputs still need gameplay and visual checks. AnimSmith 0.10.0 — retained command ledgers and classification material supplied replay controls; its conclusions are superseded for current behavior.

## Evidence status

Current evidence covers 113 physical FBXs and 110 logical individual-motion files with official AnimSmith 0.14.0 and the retained evaluation-manifest schema, now presented in editorial report format 3. A bounded Unity source/controller probe is current; visual/artistic, contact, retarget, authored-controller, and player-build gates remain open. See the [readiness ladder](../game-ready-clips.md#the-readiness-ladder) and the [evidence appendix](protofactor-one-handed-melee-evidence.md). Licensed source and generated candidates remain outside Git.

## Sources

- Protofactor product context and [End User License Agreement](https://protofactor.biz/end-user-license-agreement/) — vendor context only; it does not identify the local constituent revision.
