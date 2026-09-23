# Animation pack evaluation: Protofactor Two-Handed Melee Animset

> Technical verdict: **Usable with conditions**
>
> Evaluation completeness: **partial** — current mechanical, declared-contract, runtime-group, and remediation checks are complete, with bounded Unity source/mixer probes; visual, contact, retargeting, and target game-controller acceptance remain open.
>
> Confidence: **medium**
>
> Evaluation date: **2026-09-16**
>
> Current evaluator: **AnimSmith 0.14.0**
>
> Report format: **3**
>
> Detailed evidence: [Protofactor Two-Handed Melee Animset evidence appendix](protofactor-two-handed-melee-evidence.md)

## Technical decision

**Use and evidence boundary:** Full-body humanoid content for a third-person prototype context. Target-character and gameplay-camera appearance have not been accepted; dedicated first-person arms/viewmodel suitability is not established.

The 2026-09-17 vendor listing places this animset in Ultimate Animation Collection, which advertises 24 animsets. This report evaluates one of eight locally evaluated constituents; sixteen advertised constituents, including Female Basic Locomotion, were not evaluated. The local asset revision is unknown, so current listing membership is a scope reference, not proof of local contents.

**Content in evaluated inventory:** Armed walk, run, crouch and equipment/actions. Evaluated filenames include airborne, attacks and reactions; these have no accepted controller topology here. This describes candidate content, not accepted gameplay behavior.

**Adoption route:** Prototype one motion-owner variant per gait after loop-policy review. Continuous gait defects require artist or source correction if confirmed. Two 56-bone block states need isolated retarget/import checks.

AnimSmith 0.14.0 reads all 123 delivered FBX candidates. The untouched baseline has no errors; it reports 17010 constant-track notes, one duration warning. The retained per-file declarations are stricter: 13/120 motion files pass and 107 fail, chiefly because loop treatment is unresolved. Named one-shots must be re-declared before their seam failures are attributed to the source. Intended continuous gait files still need source or loop-policy correction. One heavy-hit RM file has unequal channel end times.

All 25 fresh transform candidates were produced outside the repository and passed inspect/measure. All candidates remain non-clean under their retained configs. None is adopted, so residual severity remains unresolved. Developer decision: run a bounded full-body target-engine pilot only after correcting one-shot loop declarations and obtaining clean contracts for one named gait set; require blend, foot-contact, root-owner, and visual acceptance before production use.

## Capability coverage

### Content present

Filename-classified walk, run and crouch combat gaits have separate in-place and root-motion candidates. The evaluated role inventory also includes airborne, actions and reactions.

### Content gaps and unknowns

Additive aim, paired interactions and dedicated viewmodel content have no established classification. The 56-bone `Blocked2HandMelee` and `IdleBlock2HandMelee` states differ from the common 58-bone family.

### Evaluation still needed

Redeclare one-shots, resolve continuous gait contracts, and test those block-state imports, blend coordinates, root ownership, weapon/foot contact and target-character playback.

## Runtime sets and authored motion

These are evaluator-selected source candidates, not accepted controller states. Follow a single row for each blend or chain; the appendix preserves file names, coordinates, timings and measurements.

| Set | Controller use | Adoption decision | Exact members |
|---|---|---|---|
| `crouch-combat-8-way-in-place` | Crouch eight directions; controller travel | Source-only crouch prototype with controller travel. No named gait set is contract-clean; confirm loop intent, then correct genuine continuous seams and test contacts. | [8 exact members](protofactor-two-handed-melee-evidence.md#exact-runtime-members) |
| `crouch-combat-8-way-root-motion` | Crouch eight directions; animation travel; measured RM 0.642–0.835 m/s | Source-only crouch prototype with animation travel. No named gait set is contract-clean; confirm loop intent, then correct genuine continuous seams and test contacts. | [8 exact members](protofactor-two-handed-melee-evidence.md#exact-runtime-members) |
| `run-combat-8-way-in-place` | Run eight directions; controller travel | Source-only run prototype with controller travel. No named gait set is contract-clean; confirm loop intent, then correct genuine continuous seams and test contacts. | [8 exact members](protofactor-two-handed-melee-evidence.md#exact-runtime-members) |
| `run-combat-8-way-root-motion` | Run eight directions; animation travel; measured RM 2.202–2.690 m/s | Source-only run prototype with animation travel. No named gait set is contract-clean; confirm loop intent, then correct genuine continuous seams and test contacts. | [8 exact members](protofactor-two-handed-melee-evidence.md#exact-runtime-members) |
| `walk-combat-8-way-in-place` | Walk eight directions; controller travel | Source-only walk prototype with controller travel. No named gait set is contract-clean; confirm loop intent, then correct genuine continuous seams and test contacts. | [8 exact members](protofactor-two-handed-melee-evidence.md#exact-runtime-members) |
| `walk-combat-8-way-root-motion` | Walk eight directions; animation travel; measured RM 0.810–1.092 m/s | Source-only walk prototype with animation travel. No named gait set is contract-clean; confirm loop intent, then correct genuine continuous seams and test contacts. | [8 exact members](protofactor-two-handed-melee-evidence.md#exact-runtime-members) |

The appendix retains measured duration, RM speed and in-place phase ranges. Both eight-file `run-combat-8-way` alternatives span 0.533–0.567 s; the detailed table reports each variant’s minimum. Use IP and RM as separate motion-owner choices; neither variant has accepted contact synchronization.

## Integration recipe

1. **Members/topology:** `topology=eight-way-cartesian`; select one eight-member in-place or root-motion row from the appendix and map forward/cardinal/diagonal coordinates explicitly.
2. **Timing/synchronization:** `sync=not-evaluated`; loop only confirmed continuous states, keep named one-shots at `loop=false`, and require phase/contact review before synchronized blending.
3. **State ownership:** `owner=project-controller`; choose IP controller translation or RM animation translation per state, with one owner for translation, yaw, and collision.
4. **Composition constraints:** `composition=full-body-handoff`; keep weapon combat states namespaced to this pack and require separate mask/socket/IK evidence before layering.
5. **Acceptance gate:** `gate=engine-visual-contact`; require clean intended contracts plus target-engine import, full blend-space playback, foot/weapon contact, and artist review.

## Technical issue register

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| PF2-PHASE | note | Scope: the three named in-place directional gait hypotheses above; reproduce their measured minimum-covering-arc spreads from the appendix member phases. Contact correspondence and intended synchronization are unconfirmed, so spread alone is not an artist defect. [Guidance](../game-ready-clips.md#the-readiness-ladder). | unknown | Obtain project or vendor loop/contact intent, then sweep the exact blend coordinates against a declared contact tolerance. Use engine phase markers or declared alignment only after intent is established; request artist cleanup only for a confirmed source-motion problem. Residual unresolved. | Current phase measurements inform a declared alignment trial; they do not prove contact or visual acceptance. | `observed-animsmith` phases; synchronization/contact acceptance `not-evaluated`. |
| PF2-01 | major | Scope: `Humanoid@RunForwardCombat2HandMelee.fbx`, `Humanoid@RunForwardCombat2HandMelee_RM.fbx`, and all members of `run-combat-8-way`; reproduce: lint each exact file with its retained per-file config; impact: every named gait hypothesis has 0 contract-clean members and cannot be admitted as a production blend set. [Guidance](../game-ready-clips.md#the-readiness-ladder) | artist-author | Action: re-export intended continuous cycles with matching pose and velocity boundaries, or document a non-loop policy; acceptance: all exact members pass the intended loop contract and a full blend-space visual/contact review; residual: unresolved | AnimSmith can validate and compare candidates, but cannot infer intended contact phase or approve visible motion. | `observed-animsmith`; current exact-file evidence, high confidence mechanically |
| PF2-02 | moderate | Scope: `Humanoid@AttackA2HandMelee.fbx` and other attacks, reactions, parries, or emotes currently declared as loops; reproduce: run the retained config and observe loop seam errors; impact: a project declaration can reject valid one-shots or cause unintended replay seams. [Guidance](../game-ready-clips.md#the-readiness-ladder) | engine-config | Action: set `loop=false` for confirmed one-shots and retain looping only for vendor/project-authorized continuous states; acceptance: revised declarations lint clean and state transitions play once; residual: unresolved | A future classifier may suggest intent, but project or vendor authority remains required. | `observed-animsmith`; declaration defect distinguished from source-animation quality |
| PF2-03 | moderate | Scope: `Humanoid@GetHitLeftHeavy2HandMelee_RM.fbx`, channels ending at 1.700–1.733 s; reproduce: baseline lint with the retained baseline config; impact: shorter channels clamp-hold for the final frame interval. [Guidance](../game-ready-clips.md#the-readiness-ladder) | artist-author | Action: align channel endpoints or document the hold; acceptance: duration-sanity passes and the heavy-hit contact/recovery is visually approved; residual: unresolved | Current lint detects the mismatch; safe semantic repair still needs author intent. | `observed-animsmith`; one warning plus six time-monotonic notes |

## Engine status

| Runtime | Evidence level | Technical result | Remaining gate |
|---|---|---|---|
| Unity 6000.5.8f1 | source-controller-feasibility | Fresh import and standalone sampling passed for `IdleCombatA2HandMelee`, `WalkForwardCombat2HandMelee`, and `WalkForwardCombat2HandMelee_RM`; `basic-idle-to-twohand-idle`, `sword-shield-idle-to-twohand-idle`, and `basic-walk-ip-to-twohand-walk-ip` produced finite mixer poses with fixed gameplay-owned root; the RM walk produced finite 1.159724 owner translation with animation-owned root. | Inspect rendered motion, contacts, deformation, authored AnimatorController behavior, and a player build; do not generalize beyond named sources. |
| Unreal Engine unspecified | not-evaluated | No current import or retarget result. | Import, retarget, root-motion, blend, and visual tests. |
| Godot unspecified | not-evaluated | No current conversion or playback result. | Convert/import and test animation graph, roots, contacts, and visuals. |
| Bevy unspecified | not-evaluated | No current conversion or playback result. | Convert/load and test graph, root ownership, performance, and visuals. |

## Fit and limitations

Best fit is a full-body combat prototype whose controller can explicitly select one gait variant and keep pack-local state names. It is a poor fit for immediate production admission, motion matching, layered combat, or retargeted characters without the missing engine and human gates.

The fresh Unity probe establishes finite source playback and the named cross-pack transitions for `IdleCombatA2HandMelee`, `WalkForwardCombat2HandMelee`, and `WalkForwardCombat2HandMelee_RM`; it does not establish visual quality, contact, phase, retargeting, or broad pack compatibility. Keep states namespaced and retain explicit root ownership.

## Changes between AnimSmith versions

AnimSmith 0.14.0 — revalidated the unchanged retained source inventory, per-file declarations, measured runtime hypotheses, and every previously recommended bounded transform. Current candidates remain unadopted. AnimSmith 0.10.0 — retained command ledgers and classification material supplied replay controls; its conclusions are superseded for current behavior.

## Evidence status

Current evidence covers 123 physical FBXs and 120 logical individual-motion files with official AnimSmith 0.14.0 and the retained evaluation-manifest schema, now presented in editorial report format 3. A bounded Unity source/controller probe is current; visual/artistic, contact, retarget, authored-controller, and player-build gates remain open. See the [readiness ladder](../game-ready-clips.md#the-readiness-ladder) and the [evidence appendix](protofactor-two-handed-melee-evidence.md). Licensed source and generated candidates remain outside Git.

## Sources

- Protofactor product context and [End User License Agreement](https://protofactor.biz/end-user-license-agreement/) — vendor context only; it does not identify the local constituent revision.
