# Animation-pack evaluation reports

These reports apply the repository's versioned animation-pack evaluation skill
to delivered game-animation assets. Each report is a dated technical snapshot,
not a vendor guarantee, license opinion, or substitute for testing in the
target game.

## Scorecard

Every cell after the pack name is copied from that report's header block, so
the scorecard reaches no conclusion of its own and carries no numeric score. It
is the fastest way to pick a pair; read the pair itself for the reasoning. The
completeness column carries only the word — each report states the evidence
boundary behind it — and [Current reports](#current-reports) carries the scope
and evaluation status of every pair.

| Pack | Technical verdict | Evaluation completeness | Confidence | Evaluation date | Current evaluator |
|---|---|---|---|---|---|
| [Protofactor Basic Locomotion](protofactor-basic-locomotion.md) | Usable with conditions | partial | medium | 2026-09-01 | AnimSmith 0.10.0 |
| [Protofactor Sword & Shield](protofactor-sword-and-shield.md) | Usable with conditions | partial | medium | 2026-09-01 | AnimSmith 0.10.0 |
| [Protofactor Campfire](protofactor-campfire.md) | Usable with conditions | partial | medium | 2026-09-01 | AnimSmith 0.10.0 |
| [Protofactor Climbing](protofactor-climbing.md) | Usable with conditions | partial | medium | 2026-09-01 | AnimSmith 0.10.0 |
| [Protofactor Injured](protofactor-injured.md) | Usable with conditions | partial | medium | 2026-09-01 | AnimSmith 0.10.0 |
| [Protofactor 1-Handed Melee](protofactor-one-handed-melee.md) | Usable with conditions | partial | medium | 2026-09-01 | AnimSmith 0.10.0 |
| [Protofactor 2-Handed Melee](protofactor-two-handed-melee.md) | Usable with conditions | partial | medium | 2026-09-01 | AnimSmith 0.10.0 |
| [Protofactor Dual Swords](protofactor-dual-swords.md) | Usable with conditions | partial | medium | 2026-09-01 | AnimSmith 0.10.0 |
| [Protofactor Ultimate Animation Collection](protofactor-ultimate-animation-collection.md) | Insufficient technical evidence | partial | medium | 2026-09-01 | AnimSmith 0.10.0 |
| [Mixamo Basic Locomotion](mixamo-basic-locomotion.md) | Insufficient technical evidence | partial | medium | 2026-09-01 | AnimSmith 0.10.0 |
| [Mixamo Female Basic Locomotion](mixamo-female-basic-locomotion.md) | Insufficient technical evidence | partial | medium | 2026-09-01 | AnimSmith 0.10.0 |
| [Mixamo Female Locomotion](mixamo-female-locomotion.md) | Insufficient technical evidence | partial | medium | 2026-09-01 | AnimSmith 0.10.0 |
| [Mixamo Locomotion](mixamo-locomotion.md) | Insufficient technical evidence | partial | medium | 2026-09-01 | AnimSmith 0.10.0 |
| [Mixamo Longbow Locomotion](mixamo-longbow-locomotion.md) | Insufficient technical evidence | partial | medium | 2026-09-01 | AnimSmith 0.10.0 |
| [Mixamo Magic Locomotion](mixamo-magic-locomotion.md) | Insufficient technical evidence | partial | medium | 2026-09-01 | AnimSmith 0.10.0 |
| [Mixamo Male Locomotion](mixamo-male-locomotion.md) | Insufficient technical evidence | partial | medium | 2026-09-01 | AnimSmith 0.10.0 |
| [Mixamo Pistol-Handgun Locomotion](mixamo-pistol-handgun-locomotion.md) | Insufficient technical evidence | partial | medium | 2026-09-01 | AnimSmith 0.10.0 |
| [Mixamo Rifle 8-Way Locomotion](mixamo-rifle-8-way-locomotion.md) | Insufficient technical evidence | partial | medium | 2026-09-01 | AnimSmith 0.10.0 |
| [Mixamo Locomotion Collection](mixamo-locomotion-collection.md) | Insufficient technical evidence | partial | medium | 2026-09-01 | AnimSmith 0.10.0 |

## Report organization

Keep one linked report pair per constituent animation pack:

- a concise technical report for the decision, capability tiers, important
  runtime sets, integration recipe, issue ownership, engine status, and fit;
- an evidence appendix for source identity, canonical roles, pipeline stages,
  readiness evidence, validation profiles, commands, digests, and detailed
  engine/compatibility procedures.

This keeps the reader-facing decision short and reevaluation bounded when one
pack, engine, or evaluator version changes.

The current linked-pair layout is report format version 2. Both documents name
one current AnimSmith evaluator and present ordinary sections as current state
for a game developer. Prior evaluator behavior, superseded evidence, and
version comparisons belong only in `Changes between AnimSmith versions`.
Report-format version 1 remains an immutable historical/generated contract.

Add a collection-level report pair after multiple constituent packs have been
evaluated, even while the rollup is partial. Name evaluated and missing
constituents. Its concise report should link rather than duplicate the
constituent reports and own only collection-wide conclusions:

- evaluated and missing constituent packs;
- combined gameplay/content coverage and meaningful duplicates;
- cross-pack skeleton, scale, root-motion, timing, style, and retarget paths;
- cross-pack blend, transition, mask, interaction, and motion-database sets;
- gaps that one constituent pack fills for another;
- collection-level value, adoption conditions, and confidence boundaries.

Build the rollup manifest by namespacing validated constituent manifests.
Digest-compare every overlapping logical package path before claiming safe
co-installation. Treat skeleton identity, engine graph execution, visual blend
quality, masking/contact behavior, and target-character acceptance as separate
claims. For unarmed/armed combinations, use a full-body state handoff as the
default; promote masks only with member-specific pelvis/root/contact evidence.

Reference cross-pack runtime-set members with stable namespaced motion ids such
as `protofactor-basic-locomotion:walk-forward-unarmed`. Never imply
compatibility merely because two pack reports use the same canonical primary
role. Cross-pack sets require their own grouping evidence and validation.

Use flat, stable filenames for each pair:

- `<vendor>-<pack>.md` and `<vendor>-<pack>-evidence.md` for a constituent pack;
- `<vendor>-<collection>.md` and `<vendor>-<collection>-evidence.md` for a
  collection rollup.

## Current reports

| Technical report | Evidence appendix | Scope | Evaluation status |
|---|---|---|---|
| [Protofactor Basic Locomotion](protofactor-basic-locomotion.md) | [Detailed evidence](protofactor-basic-locomotion-evidence.md) | One locally held Basic Locomotion pack from the Ultimate Animation Collection | Partial: current AnimSmith 0.10.0 mechanical evaluation and 39 unpromoted candidates; no current engine or visual acceptance |
| [Protofactor Sword & Shield](protofactor-sword-and-shield.md) | [Detailed evidence](protofactor-sword-and-shield-evidence.md) | One locally held Sword & Shield pack from the Ultimate Animation Collection | Partial: current AnimSmith 0.10.0 mechanical evaluation and 28 unpromoted candidates; no current engine or visual acceptance |
| [Protofactor Campfire](protofactor-campfire.md) | [Detailed evidence](protofactor-campfire-evidence.md) | One locally held Campfire pack from the Ultimate Animation Collection | Partial: current AnimSmith 0.10.0 mechanical evaluation and one unpromoted candidate; no current engine, contact, or visual acceptance |
| [Protofactor Climbing](protofactor-climbing.md) | [Detailed evidence](protofactor-climbing-evidence.md) | One locally held Climbing pack from the Ultimate Animation Collection | Partial: current AnimSmith 0.10.0 mechanical evaluation and one unpromoted candidate; no current engine, environment, or visual acceptance |
| [Protofactor Injured](protofactor-injured.md) | [Detailed evidence](protofactor-injured-evidence.md) | One locally held Injured pack from the Ultimate Animation Collection | Partial: current AnimSmith 0.10.0 mechanical evaluation and 15 unpromoted candidates; no current engine, blend, or visual acceptance |
| [Protofactor 1-Handed Melee](protofactor-one-handed-melee.md) | [Detailed evidence](protofactor-one-handed-melee-evidence.md) | One locally held 1-Handed Melee pack from the Ultimate Animation Collection | Partial: current AnimSmith 0.10.0 mechanical evaluation and 25 unpromoted candidates; no current engine, attachment, or visual acceptance |
| [Protofactor 2-Handed Melee](protofactor-two-handed-melee.md) | [Detailed evidence](protofactor-two-handed-melee-evidence.md) | One locally held 2-Handed Melee pack from the Ultimate Animation Collection | Partial: current AnimSmith 0.10.0 mechanical evaluation and 25 unpromoted candidates; no current engine, attachment, or visual acceptance |
| [Protofactor Dual Swords](protofactor-dual-swords.md) | [Detailed evidence](protofactor-dual-swords-evidence.md) | One locally held Dual Swords pack from the Ultimate Animation Collection | Partial: current AnimSmith 0.10.0 mechanical evaluation and 25 unpromoted candidates; no current engine, contact, or visual acceptance |
| [Protofactor Ultimate Animation Collection](protofactor-ultimate-animation-collection.md) | [Detailed evidence](protofactor-ultimate-animation-collection-evidence.md) | Partial rollup of Basic Locomotion, Sword & Shield, Campfire, Climbing, Injured, 1-Handed Melee, 2-Handed Melee, and Dual Swords | Partial: current AnimSmith 0.10.0 mechanical rollup of 159 unpromoted candidates; no current cross-pack compatibility, engine, or visual acceptance |
| [Mixamo Basic Locomotion](mixamo-basic-locomotion.md) | [Detailed evidence](mixamo-basic-locomotion-evidence.md) | One archive-paired Basic Locomotion constituent | Partial: exhaustive file/tool baseline and variant ownership contracts; licensing and runtime gates deferred |
| [Mixamo Female Basic Locomotion](mixamo-female-basic-locomotion.md) | [Detailed evidence](mixamo-female-basic-locomotion-evidence.md) | One archive-paired Female Basic Locomotion constituent | Partial: exhaustive file/tool baseline and variant ownership contracts; licensing and runtime gates deferred |
| [Mixamo Female Locomotion](mixamo-female-locomotion.md) | [Detailed evidence](mixamo-female-locomotion-evidence.md) | One archive-paired Female Locomotion constituent | Partial: exhaustive file/tool baseline and variant ownership contracts; licensing and runtime gates deferred |
| [Mixamo Locomotion](mixamo-locomotion.md) | [Detailed evidence](mixamo-locomotion-evidence.md) | One archive-paired Locomotion constituent | Partial: exhaustive file/tool baseline and variant ownership contracts; licensing and runtime gates deferred |
| [Mixamo Longbow Locomotion](mixamo-longbow-locomotion.md) | [Detailed evidence](mixamo-longbow-locomotion-evidence.md) | One archive-paired Longbow Locomotion constituent | Partial: exhaustive file/tool baseline and variant ownership contracts; licensing and runtime gates deferred |
| [Mixamo Magic Locomotion](mixamo-magic-locomotion.md) | [Detailed evidence](mixamo-magic-locomotion-evidence.md) | One archive-paired Magic Locomotion constituent | Partial: exhaustive file/tool baseline and variant ownership contracts; licensing and runtime gates deferred |
| [Mixamo Male Locomotion](mixamo-male-locomotion.md) | [Detailed evidence](mixamo-male-locomotion-evidence.md) | One archive-paired Male Locomotion constituent | Partial: exhaustive file/tool baseline and variant ownership contracts; licensing and runtime gates deferred |
| [Mixamo Pistol-Handgun Locomotion](mixamo-pistol-handgun-locomotion.md) | [Detailed evidence](mixamo-pistol-handgun-locomotion-evidence.md) | One archive-paired Pistol-Handgun Locomotion constituent | Partial: exhaustive file/tool baseline and variant ownership contracts; licensing and runtime gates deferred |
| [Mixamo Rifle 8-Way Locomotion](mixamo-rifle-8-way-locomotion.md) | [Detailed evidence](mixamo-rifle-8-way-locomotion-evidence.md) | One archive-paired Rifle 8-Way Locomotion constituent | Partial: exhaustive file/tool baseline and variant ownership contracts; licensing and runtime gates deferred |
| [Mixamo Locomotion Collection](mixamo-locomotion-collection.md) | [Detailed evidence](mixamo-locomotion-collection-evidence.md) | Partial rollup of nine archive-paired locomotion constituents | Partial: mechanical rollup only; no cross-pack compatibility, engine, or visual acceptance claim |

Commercial and other redistribution-restricted source files, excerpts,
motion-bearing derivatives, and generated engine projects are not committed.
They may be used only in an authorized external workspace for one-time local
validation. Reports retain scrubbed artifact labels, non-recoverable digests,
evaluator identity, coverage gaps, and procedures needed to repeat the work
with authorized inputs.

Repository and CI validation uses only synthetic/self-authored fixtures or
assets whose licenses explicitly permit repository inclusion and CI
use/redistribution. CI must not fetch, cache, or publish a credential-gated
marketplace pack.
