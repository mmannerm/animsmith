# Validation profiles

Use these versioned capability profiles to test plausible game-system uses
without embedding an ever-growing catalog of game genres in the skill.

## Contents

- [Core rule](#core-rule)
- [Activation and conclusion rules](#activation-and-conclusion-rules)
- [Profile catalog version 1](#profile-catalog-version-1)
- [Composing game contexts](#composing-game-contexts)
- [Pipeline-stage coverage](#pipeline-stage-coverage)

## Core rule

Run the engine-neutral `marketplace-intake` profile for every pack. Select
other profiles from stated project needs, vendor-intended uses, observed pack
capabilities, or an explicitly labeled generic evaluation hypothesis.

Profiles describe evaluation surfaces. They do not declare that the pack is
suitable, that a check ran, or that every game of a named genre shares one
contract.

## Activation and conclusion rules

Record every catalog profile in the evaluation manifest with one status:

- `selected`;
- `not-selected`;
- `not-applicable`.

For a selected profile, record one activation basis:

- `user-required` — supplied target-project requirement;
- `vendor-intended` — explicit vendor claim for the delivered pack;
- `observed-pack-capability` — delivered roles or sets make the use material;
- `evaluator-selected-generic-scenario` — a reasonable exploratory use chosen
  without a target-project requirement.

The first three bases may support an adoption condition or failure when backed
by sufficient evidence. An evaluator-selected generic scenario may establish a
conditional caveat, unknown, or best/poor-fit statement; by itself it must not
turn a focused pack's unrelated missing content into an adoption failure.

Do not invent loop flags, speeds, bone roles, masks, additive bases, controller
ownership, or tolerances to make a profile pass or fail. Record missing
declarations as coverage gaps.

## Profile catalog version 1

### `marketplace-intake`

Always selected. Inventory immutable source/provenance, readable formats,
physical files, logical motions, primary roles, variants, skeleton signatures,
mechanical checks, declared semantic coverage, generated outputs, and evidence
digests. Separate vendor claims, file observations, AnimSmith results, engine
results, inference, and work not evaluated.

### `blended-locomotion`

Select for directional or speed runtime sets. Check skeleton/retarget path,
root policy, duration and sampling, endpoint convention, gait/contact phase,
speed interpretation, center/axis/diagonal interpolation, transitions, and
actual engine blend behavior. Static timing agreement is only a prerequisite.

### `root-motion-controller`

Select when authored root translation or yaw may drive gameplay. Check root
layout, displacement, direction, speed, yaw, ground reference, extraction,
controller ownership, in-place counterparts, turns, starts/stops, and
replication or reconciliation when networking is required.

### `state-machine-transitions`

Select for starts, stops, pivots, landings, recoveries, cover transitions, or
other state boundaries. Check chain completeness, entry/exit pose and velocity,
contacts, interruption behavior, crossfade duration, sync policy, and missing
authored transitions.

### `layered-upper-body-weapons`

Select when actions may overlay locomotion or require weapon/prop use. Check
body-track coverage, hierarchy split, mask boundary, additive/override mode and
reference pose, root/pelvis/leg interference, spine/shoulder continuity, hand
contacts, socket scale/orientation, two-hand alignment, recoil/look layers, and
retargeted mask behavior.

### `traversal-environment`

Select for jumps, landings, vaults, mantles, climbs, ledges, or obstacle clips.
Check transition chains, root trajectory and alignment, takeoff/apex/landing,
environment and collision coupling, hand/foot contacts, interruption, and
controller handoff.

### `contact-actions-interactions`

Select for melee, paired interactions, prop manipulation, or other contact-
critical actions. Check contact timing and targets, root displacement, weapon
arcs or grip alignment, hit/event metadata, paired alignment, cancels,
recovery, and whether missing intent requires artist work.

### `retargeted-customizable-characters`

Select when the target is not the authoritative delivered skeleton or must
span material proportion changes. Check hierarchy, rest/reference pose, axes,
scale, chain/profile completeness, translation-bearing tracks, twist/helper
bones, deformation, attachments, and representative extreme/contact poses in
the intended retargeter.

### `motion-matching-search`

Select when clips may feed motion matching, pose search, or distance matching.
Check database breadth, state/trajectory coverage, contacts and phase,
consistent roots and sampling, semantic annotations, transitions, mirroring,
and the metadata required by the named runtime. Locomotion-loop quality alone
does not establish suitability.

### `networked-movement`

Select when authoritative movement, prediction, rollback, or replication is
in scope. Check deterministic timing, interruption, root-motion ownership,
replication/reconciliation, compression stability, controller integration, and
representative latency or rollback behavior. Never infer this profile from
single-player playback alone.

### `runtime-performance`

Select when target hardware, memory, evaluation cost, compression, or crowd
scale matters. Measure source and imported size, tracks, keys, bones, runtime
memory/evaluation cost, resampling, compression, and LOD policy. Compare
optimized candidates with `lint`, `measure`, `diff`, engine playback, and
performance evidence. Smaller files alone are not a success result.

## Composing game contexts

Describe a game as a composition of capability profiles rather than a new
profile identifier. Examples:

| Context | Likely profile composition; confirm with the user |
|---|---|
| Third-person shooter | blended locomotion, transitions, layered weapons, retargeting, possibly root motion and networking |
| First-person weapon game | layered weapons plus a dedicated first-person rig/camera test recorded as project-specific evidence |
| Action RPG | blended locomotion, transitions, root motion, contact actions, traversal, retargeting |
| Platformer | transitions, traversal/environment, root-motion policy |
| Motion-matching controller | motion-matching/search plus transitions, root policy, retargeting, and performance |
| Networked action game | relevant motion profiles plus networked movement |
| Distant crowd/strategy | retargeting and runtime performance; close contact detail may be not applicable |

Do not activate every likely profile automatically. Treat this table as scope
discovery guidance.

## Pipeline-stage coverage

Record all stages from `docs/pipeline-scenarios.md` in the manifest and report:

1. `acquire`;
2. `preserve-raw`;
3. `inspect`;
4. `segment`;
5. `root-motion`;
6. `conform`;
7. `validate`;
8. `optimize`;
9. `export`;
10. `gate-report`.

Assign the assessment taxonomy's coverage state and evidence to each stage.
Stages describe work and decisions; readiness levels describe achieved
outcomes. For example, a completed inspect stage may still produce a file-ready
finding, and a segmented-per-file pack may correctly mark segmentation
`not-applicable`.

Do not optimize before correctness and intended use are established. Constant
track or compression opportunities are findings, not authorization to rewrite
source or change runtime transition behavior.
