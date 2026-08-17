# Animation-pack evaluation reports

These reports apply the repository's versioned animation-pack evaluation skill
to delivered game-animation assets. Each report is a dated technical snapshot,
not a vendor guarantee, license opinion, or substitute for testing in the
target game.

## Report organization

Keep one evidence-backed report per constituent animation pack. A pack report
owns its source identity, canonical clip-role inventory, runtime sets,
AnimSmith results, remediation trials, engine evidence, and adoption decision.
This keeps reevaluation bounded when one pack or evaluator version changes.

Add a collection-level report after multiple constituent packs have been
evaluated. The collection report should link rather than duplicate the pack
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

Use a flat, stable filename for each report:

- `<vendor>-<pack>.md` for a constituent pack;
- `<vendor>-<collection>.md` for the future rollup.

## Current reports

| Report | Scope | Evaluation status |
|---|---|---|
| [Protofactor Basic Locomotion](protofactor-basic-locomotion.md) | One locally held Basic Locomotion pack from the Ultimate Animation Collection | Partial: exhaustive file/tool evaluation; engine runtime pass deferred |

Licensed source files and detailed derived evidence are not committed. Reports
retain portable artifact labels, digests, evaluator identity, coverage gaps,
and the procedures needed to audit or repeat the work with authorized inputs.
