# Animation pack evaluation: Protofactor Ultimate Animation Collection (partial: eight packs)

> Technical verdict: **Insufficient technical evidence**
>
> Evaluation completeness: **partial** — eight constituent source baselines and declared contracts were rerun with AnimSmith 0.14.0; new constituent scenarios guide integration; collection semantic authority and visual/gameplay acceptance remain open.
>
> Confidence: **medium**
>
> Evaluation date: **2026-09-16**
>
> Current evaluator: **AnimSmith 0.14.0**
>
> Report format: **2**
>
> Detailed evidence: [partial collection evidence appendix](protofactor-ultimate-animation-collection-evidence.md)

## Technical decision

The official AnimSmith 0.14.0 release reran [Basic Locomotion](protofactor-basic-locomotion.md), [Sword & Shield](protofactor-sword-and-shield.md), [Campfire](protofactor-campfire.md), [Climbing](protofactor-climbing.md), [Injured](protofactor-injured.md), [1-Handed Melee](protofactor-one-handed-melee.md), [2-Handed Melee](protofactor-two-handed-melee.md), and [Dual Swords](protofactor-dual-swords.md). They contain 918 source candidates, including 895 individual motion-labelled inputs. This is current mechanical intake evidence, not collection-level approval.

**Can these packs form a kinematic animation controller?** They provide useful candidate ingredients, but the evaluated combination is not an accepted controller. Use Basic Locomotion as the ground state, Injured as separate style states, melee packs as full-body armed states, and Climbing/Campfire as constrained traversal/interaction states. Shared clip naming and successful import cannot establish smooth handoffs, matching contacts, or compatible upper-body layers.

The constituent reports now name exact current scenario members and actions. They do not reconstruct a missing historical collection manifest. All 159 external transform candidates remain unpromoted; passing a transform is not a ready-to-use motion verdict.

## Capability coverage

### Complete core

- No complete gameplay core has been validated on a target game controller. Candidate content and integration scope are listed below; completed file checks do not establish gameplay completeness.

### Partial supporting gameplay

- Ground locomotion, injury states, armed actions, campfire interactions and traversal are candidate building blocks. Starts/stops, interruptions, contacts and cross-pack stance changes require project-specific acceptance.

### Absent

- No accepted full-controller, target-character, visual/contact, additive, first-person or artistic result. Collection-wide canonical semantic classification remains unavailable.
- Fifteen collection constituents are outside this partial evaluation: 2-Handed Gun, Assault Rifle, Bazooka, Bow & Arrow, Combat Bare Fists, Creature, Crowd, Double Guns, Fencing, Hostage, Minigun, Push & Pull Cube, Shotgun, Wizard, and Zombie.

## Runtime sets and authored motion

Representative current constituent scenarios are shown below; every other selected set remains in its linked report. These are independently selectable state ingredients, not a single combined blend tree. No historical runtime-set totals are reconstructed.

| Set/profile | Role or coordinate | Exact members | Variant/type | Timing or motion | Runtime contract |
|---|---|---|---|---|---|
| Walk eight directions | Candidate 1; [basic-locomotion](protofactor-basic-locomotion.md) | `Humanoid@WalkForwardUnarmed2.fbx::Take 001` | set_type=directional-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Walk eight directions | Candidate 2; [basic-locomotion](protofactor-basic-locomotion.md) | `Humanoid@WalkForwardLeftUnarmed.fbx::Take 001` | set_type=directional-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Walk eight directions | Candidate 3; [basic-locomotion](protofactor-basic-locomotion.md) | `Humanoid@WalkLeftUnarmed.fbx::Take 001` | set_type=directional-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Walk eight directions | Candidate 4; [basic-locomotion](protofactor-basic-locomotion.md) | `Humanoid@WalkBackwardsLeftUnarmed.fbx::Take 001` | set_type=directional-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Walk eight directions | Candidate 5; [basic-locomotion](protofactor-basic-locomotion.md) | `Humanoid@WalkBackwardsUnarmed.fbx::Take 001` | set_type=directional-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Walk eight directions | Candidate 6; [basic-locomotion](protofactor-basic-locomotion.md) | `Humanoid@WalkBackwardsRightUnarmed.fbx::Take 001` | set_type=directional-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Walk eight directions | Candidate 7; [basic-locomotion](protofactor-basic-locomotion.md) | `Humanoid@WalkRightUnarmed.fbx::Take 001` | set_type=directional-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Walk eight directions | Candidate 8; [basic-locomotion](protofactor-basic-locomotion.md) | `Humanoid@WalkForwardRightUnarmed.fbx::Take 001` | set_type=directional-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Injury style A speed | Candidate 1; [injured](protofactor-injured.md) | `Humanoid@IdleInjuredA.fbx::Take 001` | set_type=speed-blend | duration=2.000 s | loop=unknown; movement=controller; contact=not-evaluated |
| Injury style A speed | Candidate 2; [injured](protofactor-injured.md) | `Humanoid@WalkInjuredA.fbx::Take 001` | set_type=speed-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Injury style A speed | Candidate 3; [injured](protofactor-injured.md) | `Humanoid@RunInjuredA.fbx::Take 001` | set_type=speed-blend | duration=0.800 s | loop=unknown; movement=controller; contact=not-evaluated |
| Kneel and sit sequence | Candidate 1; [campfire](protofactor-campfire.md) | `Humanoid@StandToKneelCampfire.fbx::Take 001` | set_type=transition-chain | duration=4.500 s | loop=unknown; movement=controller; contact=not-evaluated |
| Kneel and sit sequence | Candidate 2; [campfire](protofactor-campfire.md) | `Humanoid@IdleKneelCampfire.fbx::Take 001` | set_type=transition-chain | duration=2.167 s | loop=unknown; movement=controller; contact=not-evaluated |
| Kneel and sit sequence | Candidate 3; [campfire](protofactor-campfire.md) | `Humanoid@KneelToSitCampfire.fbx::Take 001` | set_type=transition-chain | duration=4.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Kneel and sit sequence | Candidate 4; [campfire](protofactor-campfire.md) | `Humanoid@IdleSitCampfire.fbx::Take 001` | set_type=transition-chain | duration=2.667 s | loop=unknown; movement=controller; contact=not-evaluated |

## Integration recipe

1. **Members/topology:** `topology=state-machine`; keep ground locomotion, injury styles, armed stances, traversal and interactions as explicit states. Use direction/speed trees only within a reviewed compatible set.
2. **Timing/synchronization:** `sync=not-evaluated`; confirm intended loop/contact correspondence before choosing normalized-phase synchronization, then test outgoing/incoming poses for state handoffs. Do not equate identical durations with matching support feet.
3. **State ownership:** `owner=controller`; select in-place ground motions for a kinematic capsule. Declare any RM action as a separate movement mode, including collision and return of authority.
4. **Composition constraints:** `composition=full-body`; armed/unarmed full-body handoffs are the starting hypothesis. A weapon mask needs explicit spine/root ownership, grip, stance and foot-contact checks before adoption.
5. **Acceptance gate:** `gate=current-cross-pack-engine-and-visual-review`; run speed/direction sweeps, starts/stops, interruptions, attachment and geometry tests on the intended character and engine build.

## Technical issue register

Severity is scoped to the intended combined controller. A missing test is an adoption risk, not evidence that the vendor authored bad motion.

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| UC-PHASE | note | Evaluator-selected locomotion groups have measured phase differences; intended shared contact phase is not authoritative, so no artist defect follows from the spread. [Readiness guidance](../game-ready-clips.md#the-readiness-ladder). | unknown | Confirm project or vendor synchronization intent, then test the exact constituent sets against declared phase/contact tolerances. Treat per-pack loop findings separately and request artist changes only where intended playback establishes a source defect. Slicing/anchoring candidates remain unpromoted; residual adoption unresolved. | Declared anchoring can reduce measured spread; it cannot choose or approve intended contacts. | `observed-animsmith` measurements; synchronization/contact acceptance `not-evaluated`. |
| UC-CONTROLLER | major | Arbitrary cross-pack blending has no accepted movement, transition or contact contract. Guidance: not applicable. | engine-config | Build the explicit state topology above and test exact handoffs. Residual unknown until project acceptance; no pack defect inferred. | Explicit collection contracts can record selected relationships and diagnostics. | `inferred` integration guidance; controller acceptance `not-evaluated`. |
| UC-MASK | major | Full-body weapon/interaction motions may conflict with locomotion when masked at the torso. Guidance: not applicable. | engine-config | Start with full-body state changes. Validate any selected upper-body mask, grip and planted feet before promotion; residual unknown. | Mask diagnostics could expose affected channels; authored style/contact judgment remains. | Masked artistic/contact fit `not-evaluated`. |
| UC-SCOPE | minor | Fifteen constituents are outside this evaluation; whole-collection coverage and value cannot be concluded. Guidance: not applicable. | unknown | Consult the explicit exclusion list and evaluate only needed missing gameplay capabilities. Residual scope limitation. | No tool can infer evidence for missing content. | `not-evaluated`; boundary, not vendor defect. |

## Engine status

| Runtime | Evidence level | Technical result | Remaining gate |
|---|---|---|---|
| Unity 6000.5.8f1 | observed-engine | 26/26 bounded source checks: 13 samples, nine cross-pack mixer schedules, four RM samples. Finite poses/fixed owner under explicit policy. | Visual/contact, actual controller, target character, compression, candidate output and build acceptance. |
| Unreal Engine | not-evaluated | No current import or playback run. | Import, retarget, graph, contact, and build tests. |
| Godot | not-evaluated | No current conversion, import, or playback run. | Conversion/import, graph, contact, and export tests. |
| Bevy | not-evaluated | No current conversion, addressability, or runtime run. | Conversion, target mapping, runtime, and performance tests. |

## Fit and limitations

| Combination | Practical decision | Remaining acceptance |
|---|---|---|
| Basic + Injured | Candidate healthy/injured state handoff; keep injury letters separate | Speed/phase calibration, stance continuity and interruptions |
| Basic + Sword & Shield / 1-Handed / 2-Handed / Dual Swords | Candidate armed/unarmed full-body state change | Weapon grip, stance, turn/stop continuity; masking is a separate decision |
| Basic + Climbing | Ground-to-traversal state handoff with environment anchor | Entry/top-out, wall distance, contacts and collision ownership |
| Basic + Campfire | Stop at interaction anchor, then enter explicit posture sequence | Seat/fire placement, props, interrupt and return-to-ground behavior |
| Protofactor + Mixamo | No approved cross-library handoff | Target rig, rest pose, scale, root convention, phase, style and attachment tests |

Fresh Unity 6000.5.8f1 source tests passed 26/26 bounded checks: 13 individual Humanoid samples, nine named cross-pack mixer schedules and four RM samples. For the nine mixers, five weights (0, 0.25, 0.5, 0.75, 1) produced finite sampled bone transforms with `applyRootMotion=false` and a fixed Animator owner. This supports technical feasibility of the named source combinations, not visually smooth handoffs or a finished kinematic controller.

The proposed state architecture remains `inferred`; this bounded feasibility evidence is not an accepted full-controller verdict. The reports are useful for selecting a prototype and assigning author/project work; they do not establish a universal game-ready controller.

## Changes between AnimSmith versions

AnimSmith 0.14.0 — revalidated the eight constituent baselines/contracts and 159 bounded remediation candidates. Added exact current generic scenarios and developer/artist decisions.

AnimSmith 0.10.0 and earlier — superseded mechanical and remediation evidence. Historical collection logical-motion/set totals were not reconstructed or promoted into this run.

## Evidence status

Current conclusions are constituent-derived and explicitly scoped. Canonical collection taxonomy is unavailable; selected current scenarios are linked without claiming a recovered collection binding. No candidate is promoted. The [readiness ladder](../game-ready-clips.md#the-readiness-ladder) and [appendix](protofactor-ultimate-animation-collection-evidence.md) distinguish static, engine-execution and acceptance evidence.

## Sources

- Constituent reports: [Basic Locomotion](protofactor-basic-locomotion.md), [Sword & Shield](protofactor-sword-and-shield.md), [Campfire](protofactor-campfire.md), [Climbing](protofactor-climbing.md), [Injured](protofactor-injured.md), [1-Handed Melee](protofactor-one-handed-melee.md), [2-Handed Melee](protofactor-two-handed-melee.md), and [Dual Swords](protofactor-dual-swords.md).
- Protofactor, [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/) and [End User License Agreement](https://protofactor.biz/end-user-license-agreement/).
- AnimSmith, [game-ready clips](../game-ready-clips.md) and [CLI reference](../cli.md).
- Unity 6 documentation: [Blend Trees](https://docs.unity3d.com/6000.0/Documentation/Manual/class-BlendTree.html), [Root Motion](https://docs.unity3d.com/6000.0/Documentation/Manual/RootMotion.html), and [Animation Layers](https://docs.unity3d.com/6000.0/Documentation/Manual/AnimationLayers.html) — engine concepts only, not evidence of pack quality.
