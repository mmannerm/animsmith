# Animation pack evaluation: Protofactor Sword and Shield Animset

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
> Detailed evidence: [Protofactor Sword and Shield Animset evidence appendix](protofactor-sword-and-shield-evidence.md)

## Technical decision

**Use and evidence boundary:** Full-body humanoid content for a third-person prototype context. Target-character and gameplay-camera appearance have not been accepted; dedicated first-person arms/viewmodel suitability is not established.

The 2026-09-17 vendor listing places this animset in Ultimate Animation Collection, which advertises 24 animsets. This report evaluates one of eight locally evaluated constituents; sixteen advertised constituents, including Female Basic Locomotion, were not evaluated. The local asset revision is unknown, so current listing membership is a scope reference, not proof of local contents.

**Content in evaluated inventory:** Armed walk, run, crouch and equipment/actions. Evaluated filenames include combat actions and equipment transitions; airborne and additive aim are not established. This describes candidate content, not accepted gameplay behavior.

**Adoption route:** Prototype one in-place or root-motion variant per gait. Correct loop declarations for one-shots; continuous gait seams need source or loop-policy correction. The two-bone crouch RM file is a source structure blocker.

AnimSmith 0.14.0 reads all 136 delivered FBX candidates. The untouched baseline has no errors; it reports 17078 constant-track notes, one animated-scale warning. The retained per-file declarations are stricter: 17/132 motion files pass and 115 fail, chiefly because loop treatment is unresolved. Named one-shots must be re-declared before their seam failures are attributed to the source. Intended continuous gait files still need source or loop-policy correction. One combined-take file also carries animated scale, and the malformed 2-bone crouch RM file blocks that blend set.

All 28 fresh transform candidates were produced outside the repository and passed inspect/measure. Only the combined-take prune candidate lints clean; the other 27 still fail their retained configs. None is adopted, so residual severity remains unresolved. Developer decision: run a bounded full-body target-engine pilot only after correcting one-shot loop declarations and obtaining clean contracts for one named gait set; require blend, foot-contact, root-owner, and visual acceptance before production use.

## Capability coverage

### Content present

Filename-classified walk, run and crouch combat gaits have separate in-place and root-motion candidates. Equipment transitions and attack/reaction content also appear in the evaluated inventory.

### Content gaps and unknowns

The evaluated role inventory has no airborne class. Additive aim, paired interaction and dedicated viewmodel content have no established classification. The two-bone crouch RM source cannot join its proposed blend.

### Evaluation still needed

Redeclare one-shots, obtain clean intended continuous gait contracts, then test full blend coordinates, root ownership, grip, foot contact and target-character playback. Bounded Unity execution is not visual acceptance.

## Runtime sets and authored motion

These are evaluator-selected source candidates, not accepted controller states. Follow a single row for each blend or chain; the appendix preserves file names, coordinates, timings and measurements.

| Set | Controller use | Adoption decision | Exact members |
|---|---|---|---|
| `walk-combat-8-way-in-place` | Walk eight directions; controller travel | Source-only walk prototype with controller travel. No named gait set is contract-clean; confirm loop intent, then correct genuine continuous seams and test contacts. | [8 exact members](protofactor-sword-and-shield-evidence.md#exact-runtime-members) |
| `walk-combat-8-way-root-motion` | Walk eight directions; animation travel; measured RM 0.750–1.107 m/s | Source-only walk prototype with animation travel. No named gait set is contract-clean; confirm loop intent, then correct genuine continuous seams and test contacts. | [8 exact members](protofactor-sword-and-shield-evidence.md#exact-runtime-members) |
| `run-combat-8-way-in-place` | Run eight directions; controller travel | Source-only run prototype with controller travel. No named gait set is contract-clean; confirm loop intent, then correct genuine continuous seams and test contacts. | [8 exact members](protofactor-sword-and-shield-evidence.md#exact-runtime-members) |
| `run-combat-8-way-root-motion` | Run eight directions; animation travel; measured RM 2.793–3.189 m/s | Source-only run prototype with animation travel. No named gait set is contract-clean; confirm loop intent, then correct genuine continuous seams and test contacts. | [8 exact members](protofactor-sword-and-shield-evidence.md#exact-runtime-members) |
| `crouch-combat-8-way-in-place` | Crouch eight directions; controller travel | Source-only crouch prototype with controller travel. No named gait set is contract-clean; confirm loop intent, then correct genuine continuous seams and test contacts. | [8 exact members](protofactor-sword-and-shield-evidence.md#exact-runtime-members) |
| `crouch-combat-8-way-root-motion` | Crouch eight directions; animation travel; measured RM 0.701–0.789 m/s | Blocked: the two-bone RM crouch member needs source replacement before this eight-way set can be admitted; other gait loops also remain non-clean. | [8 exact members](protofactor-sword-and-shield-evidence.md#exact-runtime-members) |

The appendix retains the measured duration, RM speed and in-place phase ranges. Use the in-place and RM rows as alternatives; neither variant has accepted contact synchronization.

## Integration recipe

1. **Members/topology:** `topology=eight-way-cartesian`; use only the exact members in one named gait row and map forward/cardinal/diagonal coordinates explicitly.
2. **Timing/synchronization:** `sync=not-evaluated`; loop only confirmed continuous states, keep named one-shots at `loop=false`, and require phase/contact review before synchronized blending.
3. **State ownership:** `owner=project-controller`; choose IP controller translation or RM animation translation per state, with one owner for translation, yaw, and collision.
4. **Composition constraints:** `composition=full-body-handoff`; keep weapon combat states namespaced to this pack and require separate mask/socket/IK evidence before layering.
5. **Acceptance gate:** `gate=engine-visual-contact`; require clean intended contracts plus target-engine import, full blend-space playback, foot/weapon contact, and artist review.

## Technical issue register

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| PFS-PHASE | note | Scope: the three named in-place directional gait hypotheses above; reproduce their measured minimum-covering-arc spreads from the appendix member phases. Contact correspondence and intended synchronization are unconfirmed, so spread alone is not an artist defect. [Guidance](../game-ready-clips.md#the-readiness-ladder). | unknown | Obtain project or vendor loop/contact intent, then sweep the exact blend coordinates against a declared contact tolerance. Use engine phase markers or declared alignment only after intent is established; request artist cleanup only for a confirmed source-motion problem. Residual unresolved. | Current phase measurements inform a declared alignment trial; they do not prove contact or visual acceptance. | `observed-animsmith` phases; synchronization/contact acceptance `not-evaluated`. |
| PFS-01 | major | Scope: `Humanoid@RunForwardS&S.fbx`, `Humanoid@RunForwardS&S_RM.fbx`, and the six current eight-way gait hypotheses; reproduce: lint each exact file with its retained per-file config; impact: every named gait hypothesis has 0 contract-clean members and cannot be admitted as a production blend set. [Guidance](../game-ready-clips.md#the-readiness-ladder) | artist-author | Action: re-export intended continuous cycles with matching pose and velocity boundaries, or document a non-loop policy; acceptance: all exact members pass the intended loop contract and a full blend-space visual/contact review; residual: unresolved | AnimSmith can validate and compare candidates, but cannot infer intended contact phase or approve visible motion. | `observed-animsmith`; current exact-file evidence, high confidence mechanically |
| PFS-02 | moderate | Scope: `Humanoid@SwordAttack1S&S.fbx` and other attacks, reactions, parries, or emotes currently declared as loops; reproduce: run the retained config and observe loop seam errors; impact: a project declaration can reject valid one-shots or cause unintended replay seams. [Guidance](../game-ready-clips.md#the-readiness-ladder) | engine-config | Action: set `loop=false` for confirmed one-shots and retain looping only for vendor/project-authorized continuous states; acceptance: revised declarations lint clean and state transitions play once; residual: unresolved | A future classifier may suggest intent, but project or vendor authority remains required. | `observed-animsmith`; declaration defect distinguished from source-animation quality |
| PFS-03 | blocker | Scope: `Humanoid@CrouchForwardRightS&S_RM.fbx`; reproduce: inspect the delivered file and compare its 2-bone skeleton with the other seven 56-bone RM crouch members; impact: it cannot join the intended humanoid eight-way set. [Guidance](../game-ready-clips.md#the-readiness-ladder) | artist-author | Action: re-export the full humanoid hierarchy and animation; acceptance: skeleton signature matches the approved 56-bone family, contract lint passes, and engine blend/contact review succeeds; residual: unresolved | AnimSmith can detect the structural mismatch but cannot reconstruct missing authored bones. | `observed-file` and `observed-animsmith`; high-confidence source defect |
| PFS-04 | moderate | Scope: 14 `_RM` attack/combo/reaction files, including `Humanoid@3HitCombo1S&S_RM.fbx`; reproduce: retained configs declare them in-place and trigger `in-place`; impact: root-motion variants are rejected under the wrong controller policy. [Guidance](../game-ready-clips.md#the-readiness-ladder) | engine-config | Action: change those declarations to project-owned root motion and test collision/heading ownership; acceptance: the intended root policy lints clean and engine displacement matches design; residual: unresolved | AnimSmith measures displacement but cannot choose controller ownership. | `observed-animsmith`; current config mismatch |

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

AnimSmith 0.14.0 — revalidated the unchanged retained source inventory, per-file declarations, measured runtime hypotheses, and every previously recommended bounded transform. Current candidates remain unadopted. AnimSmith 0.10.0 — retained command ledgers and classification material supplied replay controls; its conclusions are superseded for current behavior.

## Evidence status

Current evidence covers 136 physical FBXs and 132 logical individual-motion files with official AnimSmith 0.14.0 and the retained evaluation-manifest schema, now presented in editorial report format 3. A bounded Unity source/controller probe is current; visual/artistic, contact, retarget, authored-controller, and player-build gates remain open. See the [readiness ladder](../game-ready-clips.md#the-readiness-ladder) and the [evidence appendix](protofactor-sword-and-shield-evidence.md). Licensed source and generated candidates remain outside Git.

## Sources

- Protofactor product context and [End User License Agreement](https://protofactor.biz/end-user-license-agreement/) — vendor context only; it does not identify the local constituent revision.
