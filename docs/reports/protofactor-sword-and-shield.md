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
> Report format: **2**
>
> Detailed evidence: [Protofactor Sword and Shield Animset evidence appendix](protofactor-sword-and-shield-evidence.md)

## Technical decision

AnimSmith 0.14.0 reads all 136 delivered FBX candidates. The untouched baseline has no errors; it reports 17078 constant-track notes, one animated-scale warning. The retained per-file declarations are stricter: 17/132 motion files pass and 115 fail, chiefly because loop treatment is unresolved. Named one-shots must be re-declared before their seam failures are attributed to the source. Intended continuous gait files still need source or loop-policy correction. One combined-take file also carries animated scale, and the malformed 2-bone crouch RM file blocks that blend set.

All 28 fresh transform candidates were produced outside the repository and passed inspect/measure. Only the combined-take prune candidate lints clean; the other 27 still fail their retained configs. None is adopted, so residual severity remains unresolved. Developer decision: run a bounded full-body target-engine pilot only after correcting one-shot loop declarations and obtaining clean contracts for one named gait set; require blend, foot-contact, root-owner, and visual acceptance before production use.

## Capability coverage

### Complete core

- No complete gameplay core has been validated on a target game controller. Candidate content and integration scope are listed below; completed file checks do not establish gameplay completeness.

### Partial supporting gameplay

- Directional IP/RM gait families, equipment transitions, and attacks are present; current set membership is a fresh evaluator-defined hypothesis and every named gait set remains contract-non-clean.
- Airborne content is absent; additive aim, paired interactions, and first-person use are not established.

### Absent

- Unity 6000.5.8f1 has bounded source/controller feasibility for `IdleCombatS&S` and the named Playables transitions; visual/contact, authored-controller, deformation, player-build, retarget, mask/layer, performance, network movement, and artistic evaluation remain absent.

## Runtime sets and authored motion

These current hypotheses use exact source identities and fresh 0.14.0 measurements. They do not restore any unavailable historical membership authority.

| Set/profile | Role or coordinate | Exact members | Variant/type | Timing or motion | Runtime contract |
|---|---|---|---|---|---|
| `walk-combat-8-way-in-place` | eight-way directional gait | `Humanoid@WalkForwardS&S.fbx`; `Humanoid@WalkForwardLeftS&S.fbx`; `Humanoid@WalkLeftS&S.fbx`; `Humanoid@WalkBackwardsLeftS&S.fbx`; `Humanoid@WalkBackwardsS&S.fbx`; `Humanoid@WalkBackwardsRightS&S.fbx`; `Humanoid@WalkRightS&S.fbx`; `Humanoid@WalkForwardRightS&S.fbx` | variant=in-place | duration=1.333 s | loop=true; sync=not-evaluated |
| `walk-combat-8-way-root-motion` | eight-way directional gait | `Humanoid@WalkForwardS&S_RM.fbx`; `Humanoid@WalkForwardLeftS&S_RM.fbx`; `Humanoid@WalkLeftS&S_RM.fbx`; `Humanoid@WalkBackwardsLeftS&S_RM.fbx`; `Humanoid@WalkBackwardsS&S_RM.fbx`; `Humanoid@WalkBackwardsRightS&S_RM.fbx`; `Humanoid@WalkRightS&S_RM.fbx`; `Humanoid@WalkForwardRightS&S_RM.fbx` | variant=root-motion | duration=1.333 s; rm_speed=0.750 m/s | loop=true; sync=not-evaluated |
| `run-combat-8-way-in-place` | eight-way directional gait | `Humanoid@RunForwardS&S.fbx`; `Humanoid@RunForwardLeftS&S.fbx`; `Humanoid@RunLeftS&S.fbx`; `Humanoid@RunBackwardsLeftS&S.fbx`; `Humanoid@RunBackwardsS&S.fbx`; `Humanoid@RunBackwardsRightS&S.fbx`; `Humanoid@RunRightS&S.fbx`; `Humanoid@RunForwardRightS&S.fbx` | variant=in-place | duration=0.600 s | loop=true; sync=not-evaluated |
| `run-combat-8-way-root-motion` | eight-way directional gait | `Humanoid@RunForwardS&S_RM.fbx`; `Humanoid@RunForwardLeftS&S_RM.fbx`; `Humanoid@RunLeftS&S_RM.fbx`; `Humanoid@RunBackwardsLeftS&S_RM.fbx`; `Humanoid@RunBackwardsS&S_RM.fbx`; `Humanoid@RunBackwardsRightS&S_RM.fbx`; `Humanoid@RunRightS&S_RM.fbx`; `Humanoid@RunForwardRightS&S_RM.fbx` | variant=root-motion | duration=0.600 s; rm_speed=2.793 m/s | loop=true; sync=not-evaluated |
| `crouch-combat-8-way-in-place` | eight-way directional gait | `Humanoid@CrouchForwardS&S.fbx`; `Humanoid@CrouchForwardLeftS&S.fbx`; `Humanoid@CrouchLeftS&S.fbx`; `Humanoid@CrouchBackwardsLeftS&S.fbx`; `Humanoid@CrouchBackwardsS&S.fbx`; `Humanoid@CrouchBackwardsRightS&S.fbx`; `Humanoid@CrouchRightS&S.fbx`; `Humanoid@CrouchForwardRightS&S.fbx` | variant=in-place | duration=1.667 s | loop=true; sync=not-evaluated |
| `crouch-combat-8-way-root-motion` | eight-way directional gait | `Humanoid@CrouchForwardS&S_RM.fbx`; `Humanoid@CrouchForwardLeftS&S_RM.fbx`; `Humanoid@CrouchLeftS&S_RM.fbx`; `Humanoid@CrouchBackwardsLeftS&S_RM.fbx`; `Humanoid@CrouchBackwardsS&S_RM.fbx`; `Humanoid@CrouchBackwardsRightS&S_RM.fbx`; `Humanoid@CrouchRightS&S_RM.fbx`; `Humanoid@CrouchForwardRightS&S_RM.fbx` | variant=root-motion | duration=1.667 s; rm_speed=0.701 m/s | loop=true; sync=not-evaluated |

Measured RM speed spans are walk 0.750–1.107 m/s (ratio 0.677); run 2.793–3.189 m/s (ratio 0.876); crouch 0.701–0.789 m/s (ratio 0.889). Preserve authored variation unless the project declares a normalization policy; diagonals and cardinals require the full blend test. Measured source in-place phase spreads (cycles, eight measured members each) are `walk-combat-8-way-in-place` 0.7231; `run-combat-8-way-in-place` 0.6605; `crouch-combat-8-way-in-place` 0.6974. Phase spread here is the minimum covering arc in cycles: sort phases in `[0,1)`, include the wraparound gap, then subtract the largest gap from 1. It is not `max_circular_deviation_from_mean`; neither measure alone proves support-foot or visual compatibility. Keep `sync=not-evaluated` until the selected synchronization and contact policy is tested. The current catalog also measures paired normal/fast speed hypotheses, four death/downed/recovery chains, and two draw/combat/put-away chains. The transition chains are mechanically stronger than the gait sets, but still lack engine and visual acceptance.

## Integration recipe

1. **Members/topology:** `topology=eight-way-cartesian`; use only the exact members in one named gait row and map forward/cardinal/diagonal coordinates explicitly.
2. **Timing/synchronization:** `sync=not-evaluated`; loop only confirmed continuous states, keep named one-shots at `loop=false`, and require phase/contact review before synchronized blending.
3. **State ownership:** `owner=project-controller`; choose IP controller translation or RM animation translation per state, with one owner for translation, yaw, and collision.
4. **Composition constraints:** `composition=full-body-handoff`; keep weapon combat states namespaced to this pack and require separate mask/socket/IK evidence before layering.
5. **Acceptance gate:** `gate=engine-visual-contact`; require clean intended contracts plus target-engine import, full blend-space playback, foot/weapon contact, and artist review.

## Technical issue register

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
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

Current evidence covers 136 physical FBXs and 132 logical individual-motion files with official AnimSmith 0.14.0, report format 2, and the retained evaluation-manifest schema. A bounded Unity source/controller probe is current; visual/artistic, contact, retarget, authored-controller, and player-build gates remain open. See the [readiness ladder](../game-ready-clips.md#the-readiness-ladder) and the [evidence appendix](protofactor-sword-and-shield-evidence.md). Licensed source and generated candidates remain outside Git.

## Sources

- Protofactor product context and [End User License Agreement](https://protofactor.biz/end-user-license-agreement/) — vendor context only; it does not identify the local constituent revision.
