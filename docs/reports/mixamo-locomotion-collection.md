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
> Report format: **3**
>
> Detailed evidence: [Evidence appendix](mixamo-locomotion-collection-evidence.md)

## Technical decision

Choose one of the nine evaluated pools for a bounded controller prototype. For a combined unarmed-to-pistol prototype, try a full-body state handoff with one controller owning movement. The proposed pair (`hypothesis/full-body-unarmed-to-pistol`) has no compatibility result yet: compare its exact skeletons, rest poses, and scale, then review the transition, feet, weapon hands, deformation, and style in the target engine before wider use.

All 249 extracted FBX files loaded and completed current inspect, measure, and empty-baseline lint: 0 errors, 50 `duration-sanity` warnings, and 35405 `constant-track` notes. Current archive-level contracts produced 52 stationary-root errors. All sources resolve the Mixamo profile; 231 motion-named files have 66 bones and 18 `X Bot.fbx` reference files have 68.

## Capability coverage

This is a full-body humanoid controller hypothesis for visible characters. Third-person gameplay-camera appearance and target-character deformation remain visually untested. Dedicated first-person arms/viewmodel content and close-camera suitability were not established. These nine locally evaluated pools are not a verified current vendor bundle or complete Mixamo catalog.

### Content present

| Candidate ingredients in the evaluated files | Current use boundary |
|---|---|
| Nine local pools offer different filename-based controller ingredients; one cross-pool unarmed-to-pistol handoff is proposed. Shared names do not establish duplicate files. | Filename-based candidates and current measurements; these do not establish a complete accepted gameplay controller. |

### Content gaps and unknowns

The legacy metadata has not been matched reliably to individual clips, and current vendor grouping is unverified. Shared names and measurements have not been checked for duplicate bytes or safe combined use.

### Evaluation still needed

- Each pool has a selected controller prototype; the proposed full-body handoff between pools still needs pairwise validation.
- Across those nine constituent scenarios, AnimSmith measured gait phase for all 29 selected members. The constituent reports retain every member value and each set's normalized circular-range spread; synchronization and contact acceptance remain `not-evaluated`.
- Cross-pool skeleton identity, retargeting, transition, visual/contact, performance and style acceptance remain open.

### Choosing a local pool

These nine evaluated local pools have no verified current vendor-bundle membership. Choose a pool for its proposed controller shape, then inspect the [constituent report](mixamo-basic-locomotion.md) and the individual links below before adopting other named motions. All entries have filename-based content signals, unresolved loop/contact/visual acceptance, and per-file ownership work outside the selected set.

| Pool | Candidate use from current files | Practical difference and open decision |
|---|---|---|
| [Basic](mixamo-basic-locomotion.md) | Unarmed forward/left/right walk | Directional walking pilot; no selected run or complete gameplay core. |
| [Female Basic](mixamo-female-basic-locomotion.md) | Walk/run speed pair | Includes more delivered files than Female Locomotion; extra gameplay roles and value are unclassified. |
| [Female Locomotion](mixamo-female-locomotion.md) | Walk/run speed pair | Selected walk/run names, timings and phases match Female Basic in these reports; byte identity and interchangeable use were not checked. |
| [Locomotion](mixamo-locomotion.md) | Walk/run speed pair | Another unarmed speed pilot with a different reported run phase; practical overlap with the female and male pools is unresolved. |
| [Longbow](mixamo-longbow-locomotion.md) | Four-direction full-body run | Bow-themed run pose; duration and contact alignment need review. |
| [Magic](mixamo-magic-locomotion.md) | Four-direction full-body run | Magic-themed run pose; duration and contact alignment need review. |
| [Male](mixamo-male-locomotion.md) | Walk/standard-run speed pair | Another unarmed speed pilot; the label does not prove target body proportions or retarget suitability. |
| [Pistol-Handgun](mixamo-pistol-handgun-locomotion.md) | Forward/back full-body pistol run | Two-direction armed pilot, with a larger measured phase spread. |
| [Rifle 8-Way](mixamo-rifle-8-way-locomotion.md) | Eight-direction full-body rifle run | Widest selected run topology; aiming, crouch, jump and death filenames are candidate content, not accepted states. |

Shared filenames, measurements, Mixamo role labels or bone counts do not prove duplicate assets, safe co-installation, one retarget profile, or compatible style. Compare exact source bytes and the target character before replacing one pool with another or combining them.

## Runtime sets and authored motion

| Set | Controller use | Adoption decision | Exact members |
|---|---|---|---|
| `hypothesis/full-body-unarmed-to-pistol` | Unarmed walk to pistol run full-body handoff; controller owns XZ translation, yaw and collision | Each constituent can be piloted separately with controller-owned movement. The cross-pool handoff remains an unaccepted proposal: check hierarchy, rest pose, scale, contact and style per exact pair. The 52 ownership findings concern other files across the pools. No AnimSmith remediation trial or accepted combined controller was reported. | [Member timings, coordinates and contracts](mixamo-locomotion-collection-evidence.md#exact-runtime-members) |

The selected handoff is a proposal. Test the exact pair before treating its clips as compatible.

The constituent gait-phase spreads range from `0.0312` to `0.2477` cycles, using normalized circular range (`1 - largest cyclic gap` across member phases on the unit cycle). Those measurements can guide target-engine tests but do not establish a shared phase policy, compatible support-foot events, or a defect in any source pack.

## Integration recipe

1. **Members/topology:** Place the exact unarmed and pistol members from the linked appendix in separate full-body states.
2. **Timing/synchronization:** Keep source timing. Set and review the crossfade in the target engine; loop intent and gait alignment across pools remain unverified.
3. **State ownership:** Use one kinematic controller for XZ translation, yaw, and collision across the state change.
4. **Composition constraints:** Start with a full-body handoff. Test any upper-body mask, additive motion, shared IK, socket, or retarget profile separately.
5. **Acceptance gate:** For each proposed pair, compare hierarchy, rest pose, and scale; then inspect interrupted transitions, feet, weapon hands, target deformation, and style from the gameplay camera.

## Technical issue register

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| MIX-OWN-001 | moderate | Scope: 52 exact paths listed in the nine linked constituent issue rows; reproduce: archive-level animation-owned XZ lint; impact: a package-wide policy can leave intended movement stationary. Guidance: not applicable. | engine-config | Choose ownership per file and state. Accept when declared lint and one target controller apply movement/yaw/collision exactly once for every admitted state. | Generic rewrite is not justified without intent. | `observed-animsmith`; 52 current findings; unresolved. |
| MIX-XPACK-001 | major | Scope: every proposed cross-pack state/layer boundary; reproduce: no current pairwise hierarchy/rest/scale or runtime transition test exists; impact: a shared controller may pop, deform, slide, misalign weapons, or change style. Guidance: not applicable. | unknown | Select exact pairs, compare skeleton/rest/scale, then run full-body target-engine transitions before considering layers. Accept when technical, contact, deformation, and artistic gates pass per pair. | Deterministic comparison can support evidence but cannot decide artistic compatibility. | `not-evaluated`; current source mechanics only; unresolved. |

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

AnimSmith 0.14.0 — The current evaluation reran 996 commands across 249 FBXs and revalidated that the source inventory stayed unchanged. It proposed constituent controller sets and one cross-pool handoff; clip mapping remains unverified.

AnimSmith 0.10.0 — Retained historical finding totals agree but supply no current collection semantics or engine acceptance. AnimSmith 0.7.0 collection conclusions remain superseded.

## Evidence status

The current tests covered nine constituents, 249 FBX files, and 119,754,377 source bytes. Legacy metadata lists 231 motions, but its clip-to-file mapping is unverified. The [evidence appendix](mixamo-locomotion-collection-evidence.md) records source integrity, evaluator revision, command results, and digests; licensed data remains external. See the [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder).

## Sources

- The nine linked current constituent reports and fresh external AnimSmith 0.14.0 evidence.
- [AnimSmith game-ready clip guidance](../game-ready-clips.md).
