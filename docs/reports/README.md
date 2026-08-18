# Animation-pack evaluation reports

These reports apply the repository's versioned animation-pack evaluation skill
to delivered game-animation assets. Each report is a dated technical snapshot,
not a vendor guarantee, license opinion, or substitute for testing in the
target game.

## Report organization

Keep one linked report pair per constituent animation pack:

- a concise technical report for the decision, capability tiers, important
  runtime sets, integration recipe, issue ownership, engine status, and fit;
- an evidence appendix for source identity, canonical roles, pipeline stages,
  readiness evidence, validation profiles, commands, digests, and detailed
  engine/compatibility procedures.

This keeps the reader-facing decision short and reevaluation bounded when one
pack, engine, or evaluator version changes.

The linked-pair layout is report format version 1. Both documents record that
version independently of the captured AnimSmith evaluator version.

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
| [Protofactor Basic Locomotion](protofactor-basic-locomotion.md) | [Detailed evidence](protofactor-basic-locomotion-evidence.md) | One locally held Basic Locomotion pack from the Ultimate Animation Collection | Partial: exhaustive file/tool evaluation plus Unity 6000.5.8f1 import and headless Playables probe; other engines and visual acceptance deferred |
| [Protofactor Sword & Shield](protofactor-sword-and-shield.md) | [Detailed evidence](protofactor-sword-and-shield-evidence.md) | One locally held Sword & Shield pack from the Ultimate Animation Collection | Partial: exhaustive file/tool evaluation plus Unity 6000.5.8f1 combined-project Playables, mask, and prop probe; other engines and visual acceptance deferred |
| [Protofactor Campfire](protofactor-campfire.md) | [Detailed evidence](protofactor-campfire-evidence.md) | One locally held Campfire pack from the Ultimate Animation Collection | Partial: exhaustive file/tool evaluation plus Unity 6000.5.8f1 sampling, Basic mixer, and prop probes; contact and visual acceptance deferred |
| [Protofactor Climbing](protofactor-climbing.md) | [Detailed evidence](protofactor-climbing-evidence.md) | One locally held Climbing pack from the Ultimate Animation Collection | Partial: exhaustive file/tool evaluation plus Unity 6000.5.8f1 sampling/mixer and expected outlier evidence; environment and visual acceptance deferred |
| [Protofactor Injured](protofactor-injured.md) | [Detailed evidence](protofactor-injured-evidence.md) | One locally held Injured pack from the Ultimate Animation Collection | Partial: exhaustive file/tool evaluation plus Unity 6000.5.8f1 sampling, mixers, and mask execution; loop/blend and visual acceptance deferred |
| [Protofactor 1-Handed Melee](protofactor-one-handed-melee.md) | [Detailed evidence](protofactor-one-handed-melee-evidence.md) | One locally held 1-Handed Melee pack from the Ultimate Animation Collection | Partial: exhaustive file/tool evaluation plus Unity 6000.5.8f1 eight-pack co-import, sampling, blend, mask, and prop probes; two Generic block clips and visual acceptance remain open |
| [Protofactor 2-Handed Melee](protofactor-two-handed-melee.md) | [Detailed evidence](protofactor-two-handed-melee-evidence.md) | One locally held 2-Handed Melee pack from the Ultimate Animation Collection | Partial: exhaustive file/tool evaluation with an explicit rig-role map plus Unity 6000.5.8f1 eight-pack co-import, sampling, blend, mask, and prop probes; two Generic block clips and visual acceptance remain open |
| [Protofactor Dual Swords](protofactor-dual-swords.md) | [Detailed evidence](protofactor-dual-swords-evidence.md) | One locally held Dual Swords pack from the Ultimate Animation Collection | Partial: exhaustive file/tool evaluation plus Unity 6000.5.8f1 eight-pack co-import, sampling, blend, mask, and two-prop probes; contact and visual acceptance remain open |
| [Protofactor Ultimate Animation Collection](protofactor-ultimate-animation-collection.md) | [Detailed evidence](protofactor-ultimate-animation-collection-evidence.md) | Partial rollup of Basic Locomotion, Sword & Shield, Campfire, Climbing, Injured, 1-Handed Melee, 2-Handed Melee, and Dual Swords | Partial: all 28 pairwise shared-asset comparisons and eight-pack Unity co-import/composition evidence; 15 constituents and visual acceptance deferred |

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
