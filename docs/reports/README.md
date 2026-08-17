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
evaluated. Its concise report should link rather than duplicate the constituent
reports and own only collection-wide conclusions:

- evaluated and missing constituent packs;
- combined gameplay/content coverage and meaningful duplicates;
- cross-pack skeleton, scale, root-motion, timing, style, and retarget paths;
- cross-pack blend, transition, mask, interaction, and motion-database sets;
- gaps that one constituent pack fills for another;
- collection-level value, adoption conditions, and confidence boundaries.

Reference cross-pack runtime-set members with stable namespaced motion ids such
as `protofactor-basic-locomotion:walk-forward-unarmed`. Never imply
compatibility merely because two pack reports use the same canonical primary
role. Cross-pack sets require their own grouping evidence and validation.

Use flat, stable filenames for each pair:

- `<vendor>-<pack>.md` and `<vendor>-<pack>-evidence.md` for a constituent pack;
- `<vendor>-<collection>.md` and `<vendor>-<collection>-evidence.md` for a
  future collection rollup.

## Current reports

| Technical report | Evidence appendix | Scope | Evaluation status |
|---|---|---|---|
| [Protofactor Basic Locomotion](protofactor-basic-locomotion.md) | [Detailed evidence](protofactor-basic-locomotion-evidence.md) | One locally held Basic Locomotion pack from the Ultimate Animation Collection | Partial: exhaustive file/tool evaluation plus Unity 6000.5.8f1 import and headless Playables probe; other engines and visual acceptance deferred |

Licensed source files and detailed derived evidence are not committed. Reports
retain portable artifact labels, digests, evaluator identity, coverage gaps,
and the procedures needed to audit or repeat the work with authorized inputs.
