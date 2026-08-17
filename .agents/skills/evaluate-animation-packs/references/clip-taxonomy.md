# Clip taxonomy and evaluation manifest

Use this taxonomy to make animation-pack inventories reproducible and
comparable without erasing vendor terminology or pack-specific relationships.

## Contents

- [Three separate axes](#three-separate-axes)
- [Primary clip roles](#primary-clip-roles)
- [Orthogonal tags](#orthogonal-tags)
- [Delivered variants](#delivered-variants)
- [Runtime sets](#runtime-sets)
- [Evaluation manifest](#evaluation-manifest)
- [Classification rules](#classification-rules)

## Three separate axes

Do not combine these concepts in one category:

1. **Primary clip role** answers what one logical motion principally does.
2. **Runtime set** identifies clips or delivered variants that must work
   together in a real animation system.
3. **Validation profile** describes a potential game-system use and the
   evidence needed to evaluate it.

One logical motion has exactly one primary role. It may carry many tags, have
multiple delivered files, belong to multiple runtime sets, and participate in
multiple validation profiles.

## Primary clip roles

Use exactly one of these stable identifiers for every logical motion:

| Identifier | Principal purpose | Examples |
|---|---|---|
| `idle-pose` | Maintain or establish a pose without meaningful traversal. | Idle variation, cover hold, aim hold, reference pose |
| `continuous-locomotion` | Sustained cyclic or continuously playable movement. | Walk, run, strafe, crouch locomotion, swim, fly |
| `locomotion-transition` | Enter, leave, redirect, or change a locomotion state. | Start, stop, pivot, turn, cover entry/exit |
| `airborne` | Represent an unsupported airborne state or its boundary. | Takeoff, fall, apex, landing |
| `traversal` | Align motion to a discrete environment obstacle or traversal target. | Vault, climb, mantle, ledge, obstacle pass |
| `action-interaction` | Perform an intentional action or interact with a prop, target, or system. | Throw, reload, attack, cast, use, carry |
| `reaction-death` | Respond to an external event or terminate character control. | Hit reaction, stagger, knockdown, death |
| `emote-cinematic` | Communicate, perform, or serve non-gameplay-directed presentation. | Emote, dialogue gesture, dance, cinematic beat |
| `other-unknown` | No defensible canonical role is established. | Ambiguous vendor name, utility motion outside the catalog |

Keep every role in the manifest totals, including zero-count roles. Do not add
a pack-specific primary role. Preserve such distinctions with tags and the
vendor label.

When one file contains several motions, classify the unsplit take as
`other-unknown` and record segmentation as required. Classify the resulting
logical motions only after an authoritative segmentation decision exists.

## Orthogonal tags

Use lowercase `dimension:value` tags. Tags are multi-valued descriptors, not
proof of compatibility. Prefer these dimensions:

| Dimension | Example values |
|---|---|
| `context` | `cover`, `combat`, `civilian`, `underwater` |
| `posture` | `standing`, `crouched`, `prone`, `kneeling` |
| `direction` | `forward`, `backward`, `left`, `right`, `forward-left` |
| `gait` | `walk`, `run`, `sprint` |
| `temporal` | `loop`, `one-shot`, `hold`, `unknown` |
| `body` | `full`, `upper`, `lower`, `additive`, `unknown` |
| `motion` | `in-place`, `root-motion`, `rotation-only`, `mixed`, `unknown` |
| `prop` | `grenade`, `firearm`, `melee`, `tool` |
| `contact` | `foot`, `hand`, `weapon`, `environment`, `paired-character` |
| `source` | `vendor-declared`, `filename-inferred`, `content-observed` |

Extend a dimension only when the report defines it. Do not infer a semantic
tag solely to make a validation profile applicable. Retain the original name
and classification basis so another evaluator can audit the mapping.

## Delivered variants

Group physical files under one logical motion only with direct evidence that
they are intended counterparts. Use one variant identifier per file:

- `in-place`;
- `root-motion`;
- `rotation-only-root`;
- `single`;
- `unknown`.

Record whether the label is vendor-stated, observed, or inferred. A filename
suffix is naming evidence, not proof that only the root track differs. Preserve
timing, skeleton, phase, sample, or track-level comparison evidence separately.

## Runtime sets

Use runtime sets only for a real relationship among motions or variants:

| Set type | Intended relationship |
|---|---|
| `directional-blend` | Directions intended to interpolate in a blend space |
| `speed-blend` | Speeds or gaits intended to interpolate |
| `sync-group` | Cyclic clips intended to share phase or markers |
| `transition-chain` | Ordered entry, state, exit, or recovery motions |
| `mask-composition` | Lower/full-body base plus an upper-body or additive layer |
| `retarget-group` | Motions expected to share one retarget path |
| `paired-interaction` | Motions expected to maintain contact with a prop or actor |
| `motion-database` | Clips intended for motion/pose-search ingestion |
| `other` | A named relationship not represented above |

A runtime set member identifies a logical motion and may select one physical
file. Omitting a file selects every delivered variant of the motion. Clips may
belong to multiple sets. Do not manufacture a set merely to populate a report.

## Evaluation manifest

Retain a UTF-8 JSON manifest with schema identifier
`urn:animsmith:skill:animation-pack-evaluation-manifest:1`. The shape below is
abridged, not a validator-ready example: a completed manifest must expand every
profile, pipeline stage, and primary-role total, and each runtime set that is
present must contain at least two real members.

```json
{
  "schema": "urn:animsmith:skill:animation-pack-evaluation-manifest:1",
  "taxonomy_version": "1",
  "validation_profile_set_version": "1",
  "evaluator": {"version": "...", "revision": "..."},
  "motions": [
    {
      "id": "walk-forward",
      "vendor_label": "WalkForward",
      "primary_role": "continuous-locomotion",
      "tags": ["gait:walk", "direction:forward"],
      "classification_basis": ["observed-file", "inferred"],
      "files": [
        {"path": "WalkForward.fbx", "variant": "in-place"},
        {"path": "WalkForward_RM.fbx", "variant": "root-motion"}
      ]
    }
  ],
  "runtime_sets": [],
  "profiles": [],
  "pipeline_stages": [],
  "role_totals": {},
  "totals": {"logical_motions": 1, "delivered_files": 2}
}
```

Run `scripts/validate_evaluation_manifest.py MANIFEST.json`. The validator
checks identifiers, references, enumerations, unique physical files, complete
role totals, profile selection, pipeline-stage coverage, and reconciled counts.
Extra evidence fields are allowed so a report may retain pack-specific metrics.
The production authority for version-1 machine identifiers, enumerations, and
report labels is `scripts/evaluation_contract_v1.py`; this reference explains
their human meaning.

## Classification rules

1. Classify from delivered evidence first, vendor documentation second, and
   filename or evaluator inference last.
2. Preserve `classification_basis` on every motion and runtime set.
3. Use `other-unknown` when two primary roles remain equally plausible.
4. Keep vendor-facing group names as tags or additional evidence; never mutate
   source filenames to fit the taxonomy.
5. Report both logical-motion and physical-file counts. Derive both from the
   retained manifest rather than hand-maintaining totals.
6. Keep canonical role summaries separate from runtime-set test results.
7. Version taxonomy changes. Do not silently reinterpret old manifests when a
   role definition or controlled identifier changes.
