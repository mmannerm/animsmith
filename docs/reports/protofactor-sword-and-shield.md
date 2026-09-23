# Animation pack evaluation: Protofactor Sword and Shield Animset

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
> Detailed evidence: [Protofactor Sword and Shield Animset evidence appendix](protofactor-sword-and-shield-evidence.md)

## Technical decision

**Prototype decision:** Try one in-place or root-motion variant per gait. Correct loop declarations for one-shots; continuous gait seams need source or loop-policy correction. The two-bone root-motion crouch file is a source structure blocker. In-place clips leave travel to the controller; root-motion clips carry authored travel.

**Camera scope:** Full-body humanoid animations; appearance on your target character and camera is untested. Dedicated first-person arms/viewmodel use is unverified.

This animset is one of eight locally evaluated parts of [Ultimate Animation Collection](protofactor-ultimate-animation-collection.md). The local asset revision is unknown; the collection report separates evaluated files from the dated vendor listing.

**Content in evaluated inventory:** Armed walk, run, crouch and equipment/actions. Evaluated filenames include combat actions and equipment transitions; airborne and additive aim are not established. These are identified motions; gameplay behavior has not been approved.

AnimSmith 0.14.0 reads all 136 delivered FBX candidates. The untouched baseline has no errors; it reports 17078 constant-track notes, one animated-scale warning. Under the intended per-file project settings, 17/132 motion files pass and 115 fail, mainly because loop settings need review. Set confirmed one-time actions to play once before treating their wrap seams as source defects. For intended repeating gaits, review the loop policy and correct actual source seams. One combined-take file also carries animated scale, and the malformed 2-bone root-motion crouch file blocks that blend set.

All 28 transform trials passed file inspection and measurement. Only the combined-take pruning output passes its declared checks; the other 27 still fail. Before a target-engine pilot, correct one-shot loop settings, replace the two-bone crouch source if that set is needed, and secure one gait set that passes its intended checks. Production use still needs blending, foot-contact, movement and visual approval.

## Capability coverage

### Content present

Filename-classified walk, run and crouch combat gaits have separate in-place and root-motion candidates. Equipment transitions and attack/reaction content also appear in the evaluated inventory.

### Content gaps and unknowns

The evaluated role inventory has no airborne class. Additive aim, paired interaction and dedicated viewmodel content have no established classification. The two-bone root-motion crouch source cannot join its proposed blend.

### Evaluation still needed

Redeclare one-shots, obtain clean intended continuous gait contracts, then test full blend coordinates, root ownership, grip, foot contact and target-character playback. Bounded Unity execution is not visual acceptance.

## Runtime sets and authored motion

Use each row as a separate blend or sequence proposal. The linked appendix names its exact clips, timings and measurements; controller playback and contact quality still need testing.

| Set | Controller use | Adoption decision | Exact members |
|---|---|---|---|
| `walk-combat-8-way-in-place` | Walk eight directions; controller travel | Walk prototype with controller travel. Its declared checks still fail. Confirm which motions should loop, correct any genuine seam, and test foot contacts before using the full blend. | [8 exact members](protofactor-sword-and-shield-evidence.md#exact-runtime-members) |
| `walk-combat-8-way-root-motion` | Walk eight directions; animation travel; measured root-motion 0.750–1.107 m/s | Walk prototype with animation travel. Its declared checks still fail. Confirm which motions should loop, correct any genuine seam, and test foot contacts before using the full blend. | [8 exact members](protofactor-sword-and-shield-evidence.md#exact-runtime-members) |
| `run-combat-8-way-in-place` | Run eight directions; controller travel | Run prototype with controller travel. Its declared checks still fail. Confirm which motions should loop, correct any genuine seam, and test foot contacts before using the full blend. | [8 exact members](protofactor-sword-and-shield-evidence.md#exact-runtime-members) |
| `run-combat-8-way-root-motion` | Run eight directions; animation travel; measured root-motion 2.793–3.189 m/s | Run prototype with animation travel. Its declared checks still fail. Confirm which motions should loop, correct any genuine seam, and test foot contacts before using the full blend. | [8 exact members](protofactor-sword-and-shield-evidence.md#exact-runtime-members) |
| `crouch-combat-8-way-in-place` | Crouch eight directions; controller travel | Crouch prototype with controller travel. Its declared checks still fail. Confirm which motions should loop, correct any genuine seam, and test foot contacts before using the full blend. | [8 exact members](protofactor-sword-and-shield-evidence.md#exact-runtime-members) |
| `crouch-combat-8-way-root-motion` | Crouch eight directions; animation travel; measured root-motion 0.701–0.789 m/s | Blocked: the two-bone root-motion crouch member needs source replacement before this eight-way set can be admitted; other gait loops also still fail their declared checks. | [8 exact members](protofactor-sword-and-shield-evidence.md#exact-runtime-members) |

The appendix gives timings, travel speeds and phase measurements for each motion. Use the in-place and root-motion rows as alternatives; neither variant has accepted contact synchronization.

## Integration recipe

1. **Members/topology:** Use only the exact members in one named gait row and map forward/cardinal/diagonal coordinates explicitly.
2. **Timing/synchronization:** Loop only confirmed continuous states, keep named one-shots at `loop=false`, and require phase/contact review before synchronized blending.
3. **State ownership:** Choose controller translation for in-place clips or animation translation for root-motion clips per state, with one owner for translation, yaw, and collision.
4. **Composition constraints:** Keep weapon combat states namespaced to this pack and require separate mask/socket/IK evidence before layering.
5. **Acceptance gate:** Require clean intended contracts plus target-engine import, full blend-space playback, foot/weapon contact, and artist review.

## Technical issue register

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| PFS-PHASE | note | The three selected in-place gait sets have measured phase differences (see appendix). Foot-contact timing and intended synchronization are unknown, so the measurements alone do not show a source defect. [Guidance](../game-ready-clips.md#the-readiness-ladder). | unknown | Obtain project or vendor loop/contact intent, then sweep the exact blend coordinates against a declared contact tolerance. Use engine phase markers or declared alignment only after intent is established; request artist cleanup only for a confirmed source-motion problem. Residual unresolved. | Current phase measurements inform a declared alignment trial; they do not prove contact or visual acceptance. | `observed-animsmith` phases; synchronization/contact acceptance `not-evaluated`. |
| PFS-01 | major | The six selected eight-direction gait sets, including `Humanoid@RunForwardS&S.fbx` and `Humanoid@RunForwardS&S_RM.fbx`, have no members passing their intended checks. None is ready for a production blend. [Guidance](../game-ready-clips.md#the-readiness-ladder) | artist-author | Re-export intended repeating gaits with matching endpoint pose and velocity, or document a play-once setting. All eight members must pass their intended checks, then pass visual and contact review across the blend. Unresolved. | AnimSmith can validate and compare candidates, but cannot infer intended contact phase or approve visible motion. | `observed-animsmith`; current exact-file evidence, high confidence mechanically |
| PFS-02 | moderate | Scope: `Humanoid@SwordAttack1S&S.fbx` and other attacks, reactions, parries, or emotes currently declared as loops; reproduce: run the retained config and observe loop seam errors; impact: a project declaration can reject valid one-shots or cause unintended replay seams. [Guidance](../game-ready-clips.md#the-readiness-ladder) | engine-config | Set confirmed one-time actions to `loop=false`; keep looping only where the project or vendor intends it. Recheck the settings and play each transition once. Unresolved. | AnimSmith may suggest a play-once setting, but the project or vendor must confirm intent. | `observed-animsmith`; declaration defect distinguished from source-animation quality |
| PFS-03 | blocker | Scope: `Humanoid@CrouchForwardRightS&S_RM.fbx`; reproduce: inspect the delivered file and compare its 2-bone skeleton with the other seven 56-bone root-motion crouch members; impact: it cannot join the intended humanoid eight-way set. [Guidance](../game-ready-clips.md#the-readiness-ladder) | artist-author | re-export the full humanoid hierarchy and animation; Acceptance: skeleton signature matches the approved 56-bone family, contract lint passes, and engine blend/contact review succeeds; Unresolved | AnimSmith can detect the structural mismatch but cannot reconstruct missing authored bones. | `observed-file` and `observed-animsmith`; high-confidence source defect |
| PFS-04 | moderate | Scope: 14 `_RM` attack/combo/reaction files, including `Humanoid@3HitCombo1S&S_RM.fbx`; reproduce: retained configs declare them in-place and trigger `in-place`; impact: root-motion variants are rejected under the wrong controller policy. [Guidance](../game-ready-clips.md#the-readiness-ladder) | engine-config | change those declarations to project-owned root motion and test collision/heading ownership; Acceptance: the intended root policy lints clean and engine displacement matches design; Unresolved | AnimSmith measures displacement but cannot choose controller ownership. | `observed-animsmith`; current config mismatch |

## Engine status

| Runtime | Evidence level | Technical result | Remaining gate |
|---|---|---|---|
| Unity 6000.5.8f1 | source-controller-feasibility | Fresh import and standalone sampling passed only for `IdleCombatS&S`; `sword-shield-idle-to-onehand-idle`, `sword-shield-idle-to-twohand-idle`, and `sword-shield-idle-to-dual-idle` produced finite mixer poses with fixed gameplay-owned root. | Inspect rendered motion, contacts, deformation, authored AnimatorController behavior, and a player build; do not generalize beyond named sources. |
| Unreal Engine unspecified | not-evaluated | No current import or retarget result. | Import, retarget, root-motion, blend, and visual tests. |
| Godot unspecified | not-evaluated | No current conversion or playback result. | Convert/import and test animation graph, roots, contacts, and visuals. |
| Bevy unspecified | not-evaluated | No current conversion or playback result. | Convert/load and test graph, root ownership, performance, and visuals. |

## Fit and limitations

Best fit is a full-body combat prototype whose controller can explicitly select one gait variant and keep pack-local state names. It is a poor fit for immediate production admission, motion matching, layered combat, or retargeted characters without the missing engine and human gates.

The fresh Unity probe establishes finite source playback and the named cross-pack transitions for `IdleCombatS&S`; it does not establish visual quality, contact, phase, retargeting, or broad pack compatibility. Keep states namespaced and retain explicit root ownership.

## Changes between AnimSmith versions

AnimSmith 0.14.0 — revalidated the unchanged retained source inventory, per-file declarations, measured runtime hypotheses, and every previously recommended bounded transform. Current transformed outputs still need gameplay and visual checks. AnimSmith 0.10.0 — retained command ledgers and classification material supplied replay controls; its conclusions are superseded for current behavior.

## Evidence status

Current evidence covers 136 physical FBXs and 132 logical individual-motion files with official AnimSmith 0.14.0 and the retained evaluation-manifest schema, now presented in editorial report format 3. A bounded Unity source/controller probe is current; visual/artistic, contact, retarget, authored-controller, and player-build gates remain open. See the [readiness ladder](../game-ready-clips.md#the-readiness-ladder) and the [evidence appendix](protofactor-sword-and-shield-evidence.md). Licensed source and generated candidates remain outside Git.

## Sources

- Protofactor product context and [End User License Agreement](https://protofactor.biz/end-user-license-agreement/) — vendor context only; it does not identify the local constituent revision.
