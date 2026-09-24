# Animation pack evaluation: Protofactor Ultimate Animation Collection (partial: eight packs)

> Technical verdict: **Insufficient technical evidence**
>
> Evaluation completeness: **partial** — source and declared checks were rerun for eight animsets with AnimSmith 0.14.0; proposed cross-pack states and visual/gameplay behavior remain untested.
>
> Confidence: **medium**
>
> Evaluation date: **2026-09-16**
>
> Current evaluator: **AnimSmith 0.14.0**
>
> Report format: **3**
>
> Detailed evidence: [partial collection evidence appendix](protofactor-ultimate-animation-collection-evidence.md)

## Technical decision

**Prototype decision:** For a third-person controller-driven action pilot, start with unchanged Basic in-place walk/run sources on the target character, then add one full-body melee state. The first test is direction and speed sweeps plus the armed idle/walk handoff, checking foot slide, grip, pose pop and collision. Resolve intended loops and contacts before controller admission. Measured AnimSmith outputs are optional trials that still need those checks; source correction is required where a confirmed seam or malformed hierarchy blocks the selected set. In-place clips leave travel to the controller; root-motion clips carry authored travel.

**Camera scope:** Full-body humanoid animations; appearance on your target character and camera is untested. Dedicated first-person arms/viewmodel use is unverified.

As of the 2026-09-17 vendor listing, Ultimate advertises 24 animsets. Eight constituents were evaluated here; 16 were not: Combat Bare Fists, Fencing, Wizard, Bow & Arrow, 2 Handed Gun, Assault Rifle, Bazooka, Dual Guns, Minigun, Shotgun, Hostage, Zombie, Creature, Crowd, Push & Pull Cube, and Female Basic Locomotion. The older local report named “Double Guns”; its relationship to the listed “Dual Guns” is unverified. Current listing membership does not establish that the unknown local revision contained all 24.

**Content in evaluated inventory:** Eight evaluated constituent state families. Ground locomotion, injured styles, three melee styles, shield combat, climbing and campfire interactions have selected candidates; sixteen currently advertised constituents are unevaluated. These are identified motions; gameplay behavior has not been approved.

The official AnimSmith 0.14.0 release reran [Basic Locomotion](protofactor-basic-locomotion.md), [Sword & Shield](protofactor-sword-and-shield.md), [Campfire](protofactor-campfire.md), [Climbing](protofactor-climbing.md), [Injured](protofactor-injured.md), [1-Handed Melee](protofactor-one-handed-melee.md), [2-Handed Melee](protofactor-two-handed-melee.md), and [Dual Swords](protofactor-dual-swords.md). They contain 918 source candidates, including 895 individual motion-labelled inputs. This is current mechanical intake evidence, not collection-level approval.

**Can these packs form a kinematic animation controller?** They provide useful candidate ingredients, but the evaluated combination is not an accepted controller. Use Basic Locomotion as the ground state, Injured as separate style states, melee packs as full-body armed states, and Climbing/Campfire as constrained traversal/interaction states. Shared clip naming and successful import cannot establish smooth handoffs, matching contacts, or compatible upper-body layers.

The constituent reports now name exact current scenario members and actions. They do not reconstruct a missing historical collection manifest. All 159 transform trials still need the relevant in-game checks before any output can replace its source.

## Capability coverage

### Content present

Eight locally evaluated constituents offer selected ground gait rings, seven injury styles, four combat families, wall/ladder traversal and campfire interaction chains. Their classifications are evaluator hypotheses tied to constituent source inventories.

### Content gaps and unknowns

Sixteen animsets on the 2026-09-17 vendor listing were not locally evaluated, including Female Basic Locomotion. Their content, overlap and compatibility are unknown for this local revision. Missing evidence is not missing product content.

### Evaluation still needed

Test one cross-pack state boundary at a time, then target-character blend, contact, root ownership, retarget, visual and build behavior. The partial source and Unity probes do not establish collection-wide controller acceptance.

### Action-game inspection leads

These [retained groupings](protofactor-ultimate-animation-collection-evidence.md#runtime-set-inventory) guide source inspection, not controller acceptance. The selected gait recipes below have exact members; the action-group rows generally do not publish member-level bindings or interruption contracts.

| Need | Source lead and current status | Next check |
|---|---|---|
| Sprint and turns | [Basic filenames](protofactor-basic-locomotion.md#capability-coverage) include sprint, fast-run, U-turn and 90/180-degree turns outside the selected rings; candidate, untested | Bind takes, direction and root ownership; test stops/turns under player input |
| Dodge and parry | [Two-Handed](protofactor-two-handed-melee-evidence.md#runtime-set-inventory) `dodge-forward-back` and `parry-3-way`; candidate groups, untested | Bind exact members, windows and contacts; check interruption and collision |
| Attacks and combos | [Dual Swords](protofactor-dual-swords-evidence.md#runtime-set-inventory) `combo-alternatives` and `single-attack-alternatives`; candidate groups, untested | Bind attacks, timing, grip, hit events and cancel rules |
| Equip and recovery | [Two-Handed](protofactor-two-handed-melee-evidence.md#runtime-set-inventory), [Dual Swords](protofactor-dual-swords-evidence.md#runtime-set-inventory), and [Sword & Shield](protofactor-sword-and-shield-evidence.md#runtime-set-inventory) hold draw/put-away groups; Sword & Shield also groups four death/downed/recovery chains. Candidate groups, untested | Bind source takes and test whole entry, interrupt and exit paths |

Selected gaits and candidate actions answer different questions. A declaration pass or Unity source sample permits a diagnostic trial; controller admission needs intended contracts, target-character visuals, contacts and the actual state graph. For library, hybrid and custom-authoring choices across Protofactor and Mixamo, use the [shared evaluation guide](../commercial-pack-evaluations.md). Comparable effort has not been measured.

## Runtime sets and authored motion

Use each row as a separate blend or sequence proposal. The linked appendix names its exact clips, timings and measurements; controller playback and contact quality still need testing.

| Set | Controller use | Adoption decision | Exact members |
|---|---|---|---|
| Basic locomotion | Ground walk/run/crouch; controller travel | Start a controller-owned source prototype; phase-alignment output still needs in-game checks and 22/24 candidate outputs retain lint across its three rings. Set loop/contact and root policy. | [Constituent exact members](protofactor-basic-locomotion-evidence.md#exact-runtime-members) |
| Injured | Seven style-local idle/walk/run states | Add one style-local source state at a time; 20/21 selected files fail declared contracts. Review true loops and healthy-state handoff. | [Constituent exact members](protofactor-injured-evidence.md#exact-runtime-members) |
| Sword & Shield | Full-body shield combat; in-place or root-motion gait | Choose in-place or root-motion source gait; no named set passes its declared checks. The root-motion crouch set is blocked by a two-bone source member; review one-shot policy separately. | [Constituent exact members](protofactor-sword-and-shield-evidence.md#exact-runtime-members) |
| 1-Handed Melee | Full-body one-hand combat; in-place or root-motion gait | Choose one source gait variant; no named set passes its declared checks. Review continuous seams and isolate the 73-bone blocked state for import/retarget. | [Constituent exact members](protofactor-one-handed-melee-evidence.md#exact-runtime-members) |
| 2-Handed Melee | Full-body two-hand combat; in-place or root-motion gait | Choose one source gait variant; no named set passes its declared checks. Review continuous seams and isolate the two 56-bone block states. | [Constituent exact members](protofactor-two-handed-melee-evidence.md#exact-runtime-members) |
| Dual Swords | Full-body dual-weapon combat; in-place or root-motion gait | Choose one source gait variant; no named set passes its declared checks. Review continuous seams, dual-grip contacts and full-body handoff. | [Constituent exact members](protofactor-dual-swords-evidence.md#exact-runtime-members) |
| Climbing | Separate wall and ladder traversal | Prototype wall and ladder separately. All ten selected members fail declared-loop contracts; pruning did not clear the WallClimbUp loop findings. Geometry/contact remains open. | [Constituent exact members](protofactor-climbing-evidence.md#exact-runtime-members) |
| Campfire | Posture and prop interaction chains | Prototype discrete interaction chains. Configure anchors/props/exits; kneel hold has a rotational seam that pruning did not clear. | [Constituent exact members](protofactor-campfire-evidence.md#exact-runtime-members) |

## Integration recipe

1. **Members/topology:** Keep ground locomotion, injury styles, armed stances, traversal and interactions as explicit states. Use direction/speed trees only within a reviewed compatible set.
2. **Timing/synchronization:** Confirm intended loop/contact correspondence before choosing normalized-phase synchronization, then test outgoing/incoming poses for state handoffs. Do not equate identical durations with matching support feet.
3. **State ownership:** Select in-place ground motions for a kinematic capsule. Declare any root-motion action as a separate movement mode, including collision and return of authority.
4. **Composition constraints:** Armed/unarmed full-body handoffs are the starting hypothesis. A weapon mask needs explicit spine/root ownership, grip, stance and foot-contact checks before adoption.
5. **Acceptance gate:** Run speed/direction sweeps, starts/stops, interruptions, attachment and geometry tests on the intended character and engine build.

## Technical issue register

Severity is scoped to the intended combined controller. A missing test is an adoption risk, not evidence that the vendor authored bad motion.

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| UC-PHASE | note | Evaluator-selected locomotion groups have measured phase differences; intended shared contact phase is not authoritative, so no artist defect follows from the spread. [Readiness guidance](../game-ready-clips.md#the-readiness-ladder). | unknown | Confirm project or vendor synchronization intent, then test the exact constituent sets against declared phase/contact tolerances. Treat per-pack loop findings separately and request artist changes only where intended playback establishes a source defect. Slicing and phase-alignment outputs still need gameplay checks. | Declared anchoring can reduce measured spread; it cannot choose or approve intended contacts. | `observed-animsmith` measurements; synchronization/contact acceptance `not-evaluated`. |
| UC-CONTROLLER | major | Arbitrary cross-pack blending has no accepted movement, transition or contact contract. Guidance: not applicable. | engine-config | Build the explicit state topology above and test exact handoffs. Residual unknown until project acceptance; no pack defect inferred. | Explicit collection contracts can record selected relationships and diagnostics. | `inferred` integration guidance; controller acceptance `not-evaluated`. |
| UC-MASK | major | Full-body weapon/interaction motions may conflict with locomotion when masked at the torso. Guidance: not applicable. | engine-config | Start with full-body state changes. Check any upper-body mask against grip, stance and planted feet before using it; compatibility remains unknown. | Mask diagnostics could expose affected channels; authored style/contact judgment remains. | Masked artistic/contact fit `not-evaluated`. |
| UC-SCOPE | minor | Sixteen currently advertised constituents are outside this evaluation; whole-collection coverage and value cannot be concluded. Guidance: not applicable. | unknown | Consult the explicit exclusion list and evaluate only needed missing gameplay capabilities. Residual scope limitation. | No tool can infer evidence for missing content. | `not-evaluated`; boundary, not vendor defect. |

## Engine status

| Runtime | Evidence level | Technical result | Remaining gate |
|---|---|---|---|
| Unity 6000.5.8f1 | observed-engine | 26/26 bounded source checks: 13 samples, nine Basic/Sword & Shield-to-melee mixer schedules, four root-motion samples. Injured, Climbing and Campfire handoffs were not tested. Finite poses/fixed owner under explicit policy. | Visual/contact, actual controller, target character, compression, candidate output and build acceptance. |
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

Fresh Unity 6000.5.8f1 source tests passed 26/26 bounded checks: 13 individual Humanoid samples, nine named cross-pack mixer schedules and four root-motion samples. The nine mixers cover Basic idle/walk and Sword & Shield idle paired with 1-Handed Melee, 2-Handed Melee and Dual Swords; they do not cover Injured, Climbing or Campfire handoffs. For the nine mixers, five weights (0, 0.25, 0.5, 0.75, 1) produced finite sampled bone transforms with `applyRootMotion=false` and a fixed Animator owner. This supports technical feasibility of the named source combinations, not visually smooth handoffs or a finished kinematic controller.

The proposed state architecture remains `inferred`; this bounded feasibility evidence is not an accepted full-controller verdict. The reports are useful for selecting a prototype and assigning author/project work; they do not establish a universal game-ready controller.

## Changes between AnimSmith versions

AnimSmith 0.14.0 — revalidated the eight constituent baselines/contracts and 159 bounded remediation candidates. Added exact current generic scenarios and developer/artist decisions.

AnimSmith 0.10.0 and earlier — superseded mechanical and remediation evidence. Historical collection logical-motion/set totals were not reconstructed or promoted into this run.

## Evidence status

Current conclusions are constituent-derived and explicitly scoped. Canonical collection taxonomy is unavailable; selected current scenarios are linked without claiming a recovered collection binding. Transformed outputs still need gameplay and visual checks. The [readiness ladder](../game-ready-clips.md#the-readiness-ladder) and [appendix](protofactor-ultimate-animation-collection-evidence.md) distinguish static, engine-execution and acceptance evidence.

## Sources

- Constituent reports: [Basic Locomotion](protofactor-basic-locomotion.md), [Sword & Shield](protofactor-sword-and-shield.md), [Campfire](protofactor-campfire.md), [Climbing](protofactor-climbing.md), [Injured](protofactor-injured.md), [1-Handed Melee](protofactor-one-handed-melee.md), [2-Handed Melee](protofactor-two-handed-melee.md), and [Dual Swords](protofactor-dual-swords.md).
- Protofactor, [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/) and [End User License Agreement](https://protofactor.biz/end-user-license-agreement/).
- AnimSmith, [game-ready clips](../game-ready-clips.md) and [CLI reference](../cli.md).
- Unity 6 documentation: [Blend Trees](https://docs.unity3d.com/6000.0/Documentation/Manual/class-BlendTree.html), [Root Motion](https://docs.unity3d.com/6000.0/Documentation/Manual/RootMotion.html), and [Animation Layers](https://docs.unity3d.com/6000.0/Documentation/Manual/AnimationLayers.html) — engine concepts only, not evidence of pack quality.
