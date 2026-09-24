# Choose animations for your game

**For a third-person action prototype with controller-driven movement, inspect
Protofactor Basic first for directional walk/run/crouch. For a smaller walk/run
prototype or rifle running, inspect the corresponding Mixamo set.** These are
starting points based on the evaluated content, not rankings of animation
quality. Neither collection has an accepted complete gameplay controller.

Use the comparison below to shortlist content, then open its report for the
actual clips, known problems, and setup. The [collection catalog](reports/README.md#browse-by-collection)
is the complete list of evaluated packs. For custom combat timing or a distinct
style, compare a library plus authored changes against [authoring in Cascadeur](#use-a-library-customize-it-or-author-in-cascadeur)
before committing to either approach.

## Compare the features you need

This example brief is a humanoid action game with a kinematic controller
(game code owns movement and collision), directional locomotion, melee or
ranged combat, aiming, dodges, jumps, and reactions. Select the features your
game needs; a focused pack is not defective for omitting others.

**Candidate** means content worth testing, not accepted gameplay. **Named only**
means filenames or inventory groups suggest a use that has not been validated.
**Unevaluated** means the reports cannot answer, not that the product lacks it.

| Game feature | Evaluated Protofactor content | Evaluated Mixamo content |
|---|---|---|
| Directional ground movement | [Basic](reports/protofactor-basic-locomotion.md#runtime-sets-and-authored-motion): eight-direction in-place walk, run and crouch candidates. | [Basic](reports/mixamo-basic-locomotion.md): three-direction walk; [Male](reports/mixamo-male-locomotion.md) and other [unarmed pools](reports/mixamo-locomotion-collection.md#choosing-a-local-pool): walk/run speed pairs. |
| Sprint, starts/stops and turns | [Basic inventory](reports/protofactor-basic-locomotion-evidence.md#mechanical-baseline) names sprint, fast-run and U-turn candidates. A complete responsive transition set is untested. | [Longbow](reports/mixamo-longbow-locomotion.md#capability-coverage) names a forward stop; [Basic](reports/mixamo-basic-locomotion.md#capability-coverage) includes turn candidates. Complete coverage is unverified. |
| Melee actions, dodge and recovery | [Two-Handed](reports/protofactor-two-handed-melee-evidence.md#runtime-set-inventory) includes dodge/parry groups; [Dual Swords](reports/protofactor-dual-swords-evidence.md#runtime-set-inventory) includes attacks/combos; [Shield](reports/protofactor-sword-and-shield-evidence.md#runtime-set-inventory) includes downed/recovery chains. These are hypotheses, not accepted combat sequences. | These locomotion reports do not establish a complete melee kit or dodge/recovery sequences. This does not describe the whole Mixamo catalog. |
| Ranged locomotion | Ultimate's advertised gun and bow constituents are [unevaluated](reports/protofactor-ultimate-animation-collection.md#technical-decision). | [Rifle](reports/mixamo-rifle-8-way-locomotion.md): eight-direction run; [Pistol](reports/mixamo-pistol-handgun-locomotion.md): forward/back run; [Longbow](reports/mixamo-longbow-locomotion.md): four-direction run candidates. |
| Aim/fire over moving legs | Full-body combat motions are documented; a usable upper-body aim/fire layer is unproven. | Rifle aiming filenames and armed runs do not establish usable aim/fire layers. Grip and foot contacts remain untested. |
| Jumps, hit reactions and death | [Basic](reports/protofactor-basic-locomotion.md#capability-coverage) and combat inventories contain relevant candidates; [Injured](reports/protofactor-injured.md) offers injury-style gait states. Interruptions, landings and return to movement are untested. | [Rifle](reports/mixamo-rifle-8-way-locomotion.md#capability-coverage) names jump/death content. Complete action sequences are unclassified or untested. |
| Traversal and interactions | [Climbing](reports/protofactor-climbing.md) and [Campfire](reports/protofactor-campfire.md) have selected sequences with loop/contact work remaining. | These reports do not establish corresponding traversal or interaction sets. |
| Combining packs and characters | [Ultimate](reports/protofactor-ultimate-animation-collection.md#engine-status) has limited named Unity source tests; whole-controller, target-character retargeting and style acceptance are untested. | [Collection](reports/mixamo-locomotion-collection.md#engine-status): no engine or cross-pool compatibility tests. Protofactor-to-Mixamo compatibility is also untested. |

**Camera:** these evaluations concern full-body humanoids for visible characters.
Appearance through a third-person gameplay camera is untested. Dedicated
first-person arms/viewmodel content and suitability are unknown; full-body
motion may still be relevant to NPCs or a visible player body in a first-person
game.

**Scope:** Ultimate advertises 24 animsets in the dated listing, with eight
locally evaluated here. Mixamo's nine local pools are not a verified vendor
bundle or exhaustive catalog. Follow the collection reports for exact scope.

## Use a library, customize it, or author in Cascadeur

Choose the route using one representative gameplay loop on your own character:

- **Library first:** useful when its selected movements match the prototype.
  Inspect the original clips, configure movement/looping, and test one coherent
  set. Count retargeting, contact and transition work before expanding to more
  weapons or states.
- **Library plus authored changes:** worth trying when the base movement fits
  but a specific transition, grip, attack timing or style needs work. First
  distinguish unclassified content from a genuinely missing motion. Keep useful
  source clips and author only the demonstrated gaps.
- **Author the motion set:** worth comparing when required motion or style
  cannot be achieved economically from the tested library candidates, or when
  exact gameplay timing is central to the design. You must create and validate
  the directional variants, transitions, contacts and action recoveries as well
  as the headline walk or attack.

Cascadeur documents [character animation authoring](https://cascadeur.com/help),
[animation import](https://cascadeur.com/help/getting_started/import_fbxdae),
[retargeting between rigs](https://cascadeur.com/help/category/219), and
[export](https://cascadeur.com/help/interface/main_menu/file_menu). These official
pages were checked on 2026-09-23. They establish possible workflows, not a tested
round trip for these packs or your character. No Cascadeur authoring, editing,
export or effort trial was performed for these reports. Confirm the needed
features in your edition before choosing that workflow.

**Which costs less work? Unknown.** There is no common-character, common-engine
comparison of library integration, library customization and original authoring.
AnimSmith can check declared motion and produce specific mechanical candidates;
it does not choose artistic intent or author missing motion. A clean source
check does not establish good gameplay motion.

## What can I use now, and who must fix it?

Each report's **Adoption decision** describes one set and the remaining work:

- **Inspect unchanged source:** a diagnostic playback experiment is possible;
  this is not approval to ship or to include the set in the finished controller.
- **Configure the game:** choose movement ownership, looping, blend parameters
  and state transitions. This changes interpretation, not source animation.
- **Try an AnimSmith candidate:** a named operation improved a stated
  measurement. Check the resulting motion, contacts and transitions before
  replacing the source. An output file alone is not a successful fix.
- **Repair or replace source:** a confirmed defect requires the specified artist
  or vendor action. For example, Shield's root-motion crouch set contains a
  two-bone member; that blocks that set, not every Shield animation.
- **Resolve missing evidence:** inspect clip intent or run the named test.
  An untested layer, endpoint hold or phase difference is not automatically an
  artist defect.

The issue register records **severity before changes**. A demonstrated workaround
may reduce the remaining impact, but only after the relevant checks pass.
An unaccepted candidate leaves the outcome unresolved. Artists should read the
exact affected clips, current action and acceptance condition before editing.

Do not rank libraries by warning totals or headline verdicts. The reports use
different declared expectations and have different engine-test coverage.
“Restricted use” and “Usable with conditions” describe scoped decisions, not
comparable quality scores.

## Run a small comparison before committing

Use the same character, camera, engine, controller speeds and acceptance criteria
for each route. Start with **idle → move/turn → one action → return to movement**.
If aiming while moving is essential, include it in this first experiment rather
than deferring it until after buying or authoring the full set.

1. Record exact clips and which motion is absent, unclassified or unsuitable.
   Begin with the report's linked members. A missing report entry is not proof
   that a vendor lacks that motion.
2. Assign translation, yaw and collision to one owner per state. For a
   kinematic prototype, start with controller-owned travel and pose animation;
   test any animation-driven action as a separate mode.
3. Test rapid direction/speed changes, starts/stops and interrupted actions.
   Check foot sliding, pose pops, ground contacts, hand/weapon grip and
   deformation through the gameplay camera.
4. When testing a cross-pack handoff, hold gait and speed constant where the
   source supports it. A walk-to-armed-run example changes two variables;
   isolate each state's behavior first, then test the combined transition.
5. Record preparation, retargeting, source edits, controller setup, authoring
   and retesting effort separately. Keep failed attempts and unresolved issues
   in the comparison. Repeat the same gameplay loop for a custom-authored
   alternative before concluding it saves time.

A useful result is: **selected set and character; unchanged/configured/edited
source; checks passed; visible problems remaining; next owner; hands-on effort**.
Set tolerances for your game before comparing results. For a complete intake
procedure, continue to the [game-developer workflow](game-developer-intake-workflow.md).

## Read a report efficiently

Read **Technical decision** for the first experiment, **Capability coverage**
for other candidate motions, and **Runtime sets and authored motion** for each
set's adoption path. Use **Technical issue register** for blockers and repair
instructions. **Engine status** tells you which tests actually ran.

Open **Detailed evidence** for exact filenames, timings, grouping assumptions,
commands and provenance. Historical evaluator results remain under **Changes
between AnimSmith versions**. This guide and the report prose were refreshed on
2026-09-23 using retained evidence; the measurement dates in each report remain
the dates of those tests.

In the reports, a **hypothesis** is a proposed grouping to test; a **declared
contract** is the movement, timing or loop behavior a check was asked to enforce.
A passed contract is limited to those checks. The [readiness ladder](game-ready-clips.md#the-readiness-ladder)
explains the steps from readable files to accepted gameplay.
