# Animation-pack evaluation reports

[Latest evaluations](https://mmannerm.github.io/animsmith/evaluations/) always
opens the newest reports. Older release pages keep their dated findings.

Choose animations for your controller, see what needs fixing, and find out
whether that work belongs in your game, in AnimSmith, or with the artist.
Each report distinguishes measured source problems from behavior that still
needs testing on your character.

## Browse by collection

Start with a collection to compare packs, or open a pack directly for its
suggested blend sets, known problems, and setup instructions.

| Collection overview | Evaluated constituents |
|---|---|
| [Protofactor Ultimate Animation Collection](protofactor-ultimate-animation-collection.md) | [Basic Locomotion](protofactor-basic-locomotion.md), [Sword & Shield](protofactor-sword-and-shield.md), [Campfire](protofactor-campfire.md), [Climbing](protofactor-climbing.md), [Injured](protofactor-injured.md), [1-Handed Melee](protofactor-one-handed-melee.md), [2-Handed Melee](protofactor-two-handed-melee.md), [Dual Swords](protofactor-dual-swords.md) |
| [Mixamo Locomotion Collection](mixamo-locomotion-collection.md) | [Basic Locomotion](mixamo-basic-locomotion.md), [Female Basic Locomotion](mixamo-female-basic-locomotion.md), [Female Locomotion](mixamo-female-locomotion.md), [Locomotion](mixamo-locomotion.md), [Longbow Locomotion](mixamo-longbow-locomotion.md), [Magic Locomotion](mixamo-magic-locomotion.md), [Male Locomotion](mixamo-male-locomotion.md), [Pistol-Handgun Locomotion](mixamo-pistol-handgun-locomotion.md), [Rifle 8-Way Locomotion](mixamo-rifle-8-way-locomotion.md) |

**Coverage:** Protofactor Ultimate advertises 24 animsets as of the 2026-09-17
vendor listing; these reports evaluate eight locally held constituents and leave
16 unevaluated. The held revision's complete membership is unverified. The
Mixamo collection groups nine locally evaluated source pools; it is not a
verified current vendor bundle or complete catalog. Neither collection has
established visual or gameplay acceptance.

**Camera use:** These are full-body humanoid animations to consider for visible
characters, including third-person prototypes. Their appearance on your
character and through a gameplay camera has not been tested. Dedicated
first-person arms and weapon-viewmodel suitability are unknown.

## What each report tells you

- **Developers:** which sets can be prototyped, how to blend them, who moves the
  character, and what must be checked before use.
- **Artists:** measured source-file findings, their severity, the affected clips,
  and what to inspect or repair. Untested behavior is not an artist defect.
- **Detailed evidence:** exact clip lists, measurements, tested configurations,
  and evaluation dates. Open this when implementing or checking a finding.

An AnimSmith trial that improves a measurement is a possible fix, not proof
that the resulting animation looks right in your game.

## Current reports

Use this catalog for each report’s scope, evaluation status, and evidence
appendix. Its technical verdict, confidence, date, and evaluator are recorded
in the report itself.

| Technical report | Evidence appendix | Scope | Evaluation status |
|---|---|---|---|
| [Protofactor Basic Locomotion](protofactor-basic-locomotion.md) | [Detailed evidence](protofactor-basic-locomotion-evidence.md) | Eight-direction in-place walk, run, and crouch sets. | Limited Unity source tests passed; check loop seams, foot contacts, and transitions. AnimSmith trial outputs still need in-game checks. |
| [Protofactor Sword & Shield](protofactor-sword-and-shield.md) | [Detailed evidence](protofactor-sword-and-shield-evidence.md) | Shield combat with separate in-place and root-motion gaits. | Limited Unity source tests passed. Root-motion crouch has a two-bone clip that needs source repair; other trial outputs still need in-game checks. |
| [Protofactor Campfire](protofactor-campfire.md) | [Detailed evidence](protofactor-campfire-evidence.md) | Kneel, sit, lie, and grill interaction chains. | Not tested in an engine. Set prop and posture anchors; the kneeling hold has a loop seam that pruning did not fix. |
| [Protofactor Climbing](protofactor-climbing.md) | [Detailed evidence](protofactor-climbing-evidence.md) | Separate wall-climbing and ladder states. | Not tested in an engine. Selected clips fail the requested loop checks; pruning did not fix WallClimbUp. Check contacts against your geometry. |
| [Protofactor Injured](protofactor-injured.md) | [Detailed evidence](protofactor-injured-evidence.md) | Seven separate injury styles with idle, walk, and run. | Not tested in an engine. Review loops and healthy/injured transitions; trial outputs still need in-game checks. |
| [Protofactor 1-Handed Melee](protofactor-one-handed-melee.md) | [Detailed evidence](protofactor-one-handed-melee-evidence.md) | One-handed combat with separate in-place and root-motion gaits. | Limited Unity source tests passed. Review loop seams and the 73-bone block/guard animation separately when retargeting; trial outputs still need in-game checks. |
| [Protofactor 2-Handed Melee](protofactor-two-handed-melee.md) | [Detailed evidence](protofactor-two-handed-melee-evidence.md) | Two-handed combat with separate in-place and root-motion gaits. | Limited Unity source tests passed. Review loop seams and the two 56-bone block/guard animations separately when retargeting; trial outputs still need in-game checks. |
| [Protofactor Dual Swords](protofactor-dual-swords.md) | [Detailed evidence](protofactor-dual-swords-evidence.md) | Dual-weapon combat with separate in-place and root-motion gaits. | Limited Unity source tests passed. Check loop seams, both weapon grips, and state transitions; trial outputs still need in-game checks. |
| [Protofactor Ultimate Animation Collection](protofactor-ultimate-animation-collection.md) | [Detailed evidence](protofactor-ultimate-animation-collection-evidence.md) | Comparison of the eight Protofactor packs above. | Limited Unity source tests cover named Basic/Shield-to-melee combinations. A combined controller and AnimSmith trial outputs still need in-game checks. |
| [Mixamo Basic Locomotion](mixamo-basic-locomotion.md) | [Detailed evidence](mixamo-basic-locomotion-evidence.md) | Three-direction unarmed walk: forward, left, and right. | No engine tests. Check loops and foot contacts; choose movement ownership per state rather than by folder name. |
| [Mixamo Female Basic Locomotion](mixamo-female-basic-locomotion.md) | [Detailed evidence](mixamo-female-basic-locomotion-evidence.md) | Unarmed walk/run pair for a speed blend. | No engine tests. Check timing and foot contacts; extra gameplay clips have not been grouped into usable sets. |
| [Mixamo Female Locomotion](mixamo-female-locomotion.md) | [Detailed evidence](mixamo-female-locomotion-evidence.md) | Unarmed walk/run pair; reported names and timings match Female Basic. | No engine tests. Matching measurements do not establish identical files or interchangeable use; check timing and foot contacts. |
| [Mixamo Locomotion](mixamo-locomotion.md) | [Detailed evidence](mixamo-locomotion-evidence.md) | Unarmed walk/run pair for a speed blend. | No engine tests. Check walk/run foot timing; overlap with the other unarmed packs is unverified. |
| [Mixamo Longbow Locomotion](mixamo-longbow-locomotion.md) | [Detailed evidence](mixamo-longbow-locomotion-evidence.md) | Four-direction full-body bow run. | No engine tests. Check loop duration, foot timing, and bow grip before blending. |
| [Mixamo Magic Locomotion](mixamo-magic-locomotion.md) | [Detailed evidence](mixamo-magic-locomotion-evidence.md) | Four-direction full-body magic run. | No engine tests. Check loop duration and foot timing; upper-body layering is untested. |
| [Mixamo Male Locomotion](mixamo-male-locomotion.md) | [Detailed evidence](mixamo-male-locomotion-evidence.md) | Unarmed walk/standard-run pair for a speed blend. | No engine tests. Check foot timing and fit on your character; the pack name does not establish retarget compatibility. |
| [Mixamo Pistol-Handgun Locomotion](mixamo-pistol-handgun-locomotion.md) | [Detailed evidence](mixamo-pistol-handgun-locomotion-evidence.md) | Forward/backward full-body pistol run. | No engine tests. The selected clips differ in foot-cycle timing; check blending and weapon grip. |
| [Mixamo Rifle 8-Way Locomotion](mixamo-rifle-8-way-locomotion.md) | [Detailed evidence](mixamo-rifle-8-way-locomotion-evidence.md) | Eight-direction full-body rifle run. | No engine tests. Check loops, contacts, and per-state movement ownership; other named actions are not yet validated as controller states. |
| [Mixamo Locomotion Collection](mixamo-locomotion-collection.md) | [Detailed evidence](mixamo-locomotion-collection-evidence.md) | Comparison of nine local Mixamo animation pools. | No engine or cross-pack compatibility tests. Choose a pool first; test rig, style, and transitions before combining packs. |

Source animations and generated game projects are not redistributed here.
The detailed evidence explains how to repeat the checks with your own licensed
assets. See the [evaluation guide](../commercial-pack-evaluations.md) for how to
use these findings in your project.
