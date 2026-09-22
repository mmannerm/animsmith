# Animation pack evidence appendix: {{PACK_NAME}}

> Companion report: `{{REPORT_STEM}}.md` (replace with a relative Markdown link in the completed appendix)
>
> Evidence status: **{{COMPLETE_PARTIAL_OR_PREVIEW_ONLY}}** — {{LARGEST_EVIDENCE_BOUNDARY}}
>
> Evaluation date: **{{YYYY-MM-DD}}**
>
> Current evaluator: **AnimSmith {{SEMVER}}**
>
> Report format: **3**

This appendix preserves the detailed evidence behind the concise technical
report. Link the canonical readiness ladder at
`../game-ready-clips.md#the-readiness-ladder` in the completed appendix; it
remains authoritative.

## Evaluation scope and provenance

| Field | Value |
|---|---|
| Pack/edition | {{PACK_EDITION_OR_UNKNOWN}} |
| Vendor/source | {{VENDOR_AND_DIRECT_URL}} |
| Delivered scope | {{FULL_PARTIAL_PREVIEW_AND_CONTENT_DESCRIPTION}} |
| Target use | {{GAME_ENGINE_USE_AND_SUPPLIED_GAME_REQUIREMENTS}} |
| Target engines | {{NAMES_AND_EXACT_VERSIONS_OR_NOT_EVALUATED}} |
| Target rigs/packs | {{RIGS_OTHER_PACKS_OR_NONE}} |
| Source manifest | {{PORTABLE_PATH_AND_DIGEST}} |
| Evaluation manifest | {{PATH_DIGEST_SCHEMA_TAXONOMY_AND_PROFILE_SET_VERSION}} |
| Acquisition/license provenance | {{SHORT_FACTUAL_RECORD_AND_UNCERTAINTIES_OR_NOT_EVALUATED}} |

Do not provide legal advice or let missing transaction records masquerade as a
pack technical failure.
When the artifact came from a collection, label collection-level and
constituent-level product facts separately. Never present a current collection
version, price, release date, or license listing as the local constituent pack's
revision or transaction record.
For a partial collection rollup, name every included/excluded constituent,
build a namespaced manifest from validated constituent manifests, and preserve
the overlapping-path digest comparison. Keep constituent evidence linked rather
than duplicated; collection-owned rows contain only new cross-pack conclusions.

### Evidence coverage

| Surface | Offered/delivered | Evaluated | Findings | Not evaluated and why |
|---|---:|---:|---:|---|
| Animation files | {{COUNT}} | {{COUNT}} | {{COUNT}} | {{GAP}} |
| Rigs/export variants | {{COUNT}} | {{COUNT}} | {{COUNT}} | {{GAP}} |
| AnimSmith baseline | {{COUNT}} | {{COUNT}} | {{COUNT}} | {{GAP}} |
| Declared contracts | {{COUNT}} | {{COUNT}} | {{COUNT}} | {{GAP}} |
| Offline visual reports | {{COUNT}} | {{COUNT}} | {{COUNT}} | {{GAP}} |
| Engine import/playback | {{COUNT}} | {{COUNT}} | {{COUNT}} | {{GAP}} |
| Blend/mask/retarget | {{COUNT}} | {{COUNT}} | {{COUNT}} | {{GAP}} |

### Claim legend

Use the versioned evidence labels from `references/assessment-taxonomy.md`.

## Evaluation manifest and taxonomy

### Canonical clip-role inventory

Include every versioned primary role, including zero-count rows.

| Canonical primary role | Logical motions | Delivered files | Evidence boundary |
|---|---:|---:|---|
| `{{PRIMARY_ROLE}}` | {{COUNT}} | {{COUNT}} | {{EVIDENCE_AND_CAVEAT}} |
| **Total** | **{{COUNT}}** | **{{COUNT}}** | {{MANIFEST_AND_DIGEST}} |

### Runtime-set inventory

| Runtime set | Type | Members/variants | Grouping evidence | Validation status |
|---|---|---|---|---|
| {{SET_NAME}} | {{SET_TYPE}} | {{MEMBERS_AND_VARIANTS}} | {{EVIDENCE_AND_CONFIDENCE}} | {{STATUS}} |

Use the exact same set name as the primary report for every promoted set; the
appendix may also retain additional candidate sets.
The primary report contains the adoption summary; preserve detailed members
and measurements below so they remain reachable without widening that summary.

If no runtime sets exist, omit the table and write exactly: `No runtime sets
were identified.` This is valid only when current structured evidence contains
no measured gait group or other runtime relationship. Preserve measured groups
under their exact current structured IDs and memberships. When vendor/project
semantics are missing, mark those boundaries `not-evaluated`; do not
reconstruct older memberships from prose or filenames.

### Exact runtime members

Name every member of each important runtime set. Record its semantic variant,
measured timing or motion, and implementable runtime contract. For moving
root-motion clips, include cycle duration and horizontal speed. Calculate the
within-set minimum/maximum speed ratio and compare forward, cardinal, and
diagonal members when those roles exist. Explain the controller consequence;
speed variation is not automatically a defect without a declared movement
policy. State how in-place counterparts relate and which owner must preserve,
normalize, or re-author the variation.

| Set/profile | Role or coordinate | Exact members | Variant/type | Timing or motion | Runtime contract |
|---|---|---|---|---|---|
| {{SET}} | {{DIRECTION_ROLE_OR_THRESHOLD}} | `{{EXACT_FILE_SCOPED_MEMBER}}` | variant={{VARIANT_ID}} | duration={{SECONDS}} s; rm_speed={{METERS_PER_SECOND}} m/s | loop={{TRUE_FALSE_UNKNOWN_OR_NOT_APPLICABLE}}; sync={{POLICY}} |

Use this detailed table for locomotion, sync, transition, mask-composition,
paired-interaction, motion-database, or other important sets. `Exact members`
must name every file-scoped member with the delivered case and spelling; never
silently normalize them from a vendor list or display label. State separately
when another bundled manifest or animation list disagrees. `Runtime contract`
captures the applicable loop, sync, state transition, mask, additive, contact,
or interaction policy.
Treat this table as exact decision evidence. The primary summary links here;
retain grouping basis and validation status in the inventory above without
duplicating these member rows.
Use semicolon-separated `key=value` timing terms (`duration`, `rm_speed`,
`sample_rate`, `frames`, or `threshold`) with finite non-negative values and
units. Use semicolon-separated runtime terms keyed by `loop`, `sync`,
`transition`, `mask`, `additive`, `contact`, `interaction`, `movement`, `state`,
`database`, or `playback`; use a specific lowercase/hyphenated value such as
`one-shot`, `gait-phase`, or `unknown`. Fields that do not apply stay explicitly
`N/A`. If no important runtime sets exist, write exactly: `No important runtime
sets were identified.` Retain the grouping evidence in the appendix. This
sentence is allowed only after reconciling current structured output: do not
use it when that output contains a gait group or other runtime relationship.
Use its exact current ID and membership. When semantic authority is missing,
use `unknown`/`not-evaluated` contracts and state the vendor or project
decision needed. Do not reconstruct historical memberships from prose,
filenames, or superseded evidence.

Write `Variant/type` as one `variant=<id>` or `set_type=<id>` token. Moving
root-motion and paired IP/RM rows require `duration` and `rm_speed`; paired rows
also require distinct `loop_ip`, `loop_rm`, and `sync` policies. Do not repeat a
key with conflicting values. Prefix paired exact members with `IP` and `RM`;
any movement-labeled member requires the matching `in-place`, `root-motion`,
`rotation-only-root`, or `paired-ip-rm` variant.

If no important sets exist, replace this detailed table with exactly:
`No important runtime sets were identified.`

### Pipeline-stage coverage

Include all ten stages; completion is not a readiness verdict.

| Stage | Coverage state | Evidence / remaining gate |
|---|---|---|
| Acquire | {{COVERAGE_STATE}} | {{EVIDENCE_OR_GATE}} |
| Preserve raw | {{COVERAGE_STATE}} | {{EVIDENCE_OR_GATE}} |
| Inspect | {{COVERAGE_STATE}} | {{EVIDENCE_OR_GATE}} |
| Segment | {{COVERAGE_STATE}} | {{EVIDENCE_OR_GATE}} |
| Root motion | {{COVERAGE_STATE}} | {{EVIDENCE_OR_GATE}} |
| Conform | {{COVERAGE_STATE}} | {{EVIDENCE_OR_GATE}} |
| Validate | {{COVERAGE_STATE}} | {{EVIDENCE_OR_GATE}} |
| Optimize | {{COVERAGE_STATE}} | {{EVIDENCE_OR_GATE}} |
| Export | {{COVERAGE_STATE}} | {{EVIDENCE_OR_GATE}} |
| Gate/report | {{COVERAGE_STATE}} | {{EVIDENCE_OR_GATE}} |

### Readiness evidence by clip set

Reference, rather than redefine, the repository's six readiness levels.

| Role or runtime set | File-ready / clip-ready | Set-ready / rig-use | Runtime / acceptance boundary |
|---|---|---|---|
| {{ROLE_OR_SET}} | {{MECHANICAL_AND_DECLARED_SEMANTIC_EVIDENCE}} | {{SET_AND_RIG_EVIDENCE}} | {{ENGINE_AND_HUMAN_GATE}} |

### Validation-profile status

Include every profile in the captured profile-set version.

| Validation profile | Selection | Result / next evidence |
|---|---|---|
| Marketplace intake | {{SELECTION}} | {{RESULT_AND_GAP}} |
| Blended locomotion | {{SELECTION}} | {{RESULT_AND_GAP}} |
| Root-motion controller | {{SELECTION}} | {{RESULT_AND_GAP}} |
| State-machine transitions | {{SELECTION}} | {{RESULT_AND_GAP}} |
| Layered upper body/weapons | {{SELECTION}} | {{RESULT_AND_GAP}} |
| Traversal/environment | {{SELECTION}} | {{RESULT_AND_GAP}} |
| Contact actions/interactions | {{SELECTION}} | {{RESULT_AND_GAP}} |
| Retargeted/customizable characters | {{SELECTION}} | {{RESULT_AND_GAP}} |
| Motion matching/search | {{SELECTION}} | {{RESULT_AND_GAP}} |
| Networked movement | {{SELECTION}} | {{RESULT_AND_GAP}} |
| Runtime performance | {{SELECTION}} | {{RESULT_AND_GAP}} |

## Pack inventory and content evidence

{{DELIVERY_ORGANIZATION_CONTENT_COUNTS_AND_CAPABILITY_DETAILS}}

## Mechanical baseline

| Finding/check | Affected scope | Potential impact | Evidence |
|---|---|---|---|
| {{FINDING_OR_COVERAGE_GAP}} | {{FILES_CLIPS_OR_PERCENTAGE}} | {{RUNTIME_OR_PIPELINE_IMPACT}} | {{LABEL_AND_ARTIFACT}} |

## AnimSmith remediation evidence

| Source issue | Operation/declarations | Result | Independent verification | Remaining caveat |
|---|---|---|---|---|
| {{ISSUE}} | {{CAPTURED_COMMAND}} | {{RESULT}} | {{POSTCONDITION_EVIDENCE}} | {{GAP}} |

Link each row to the primary issue ID and identify the exact generated output
or configuration. Record candidate production, postcondition checks,
promotion/adoption, and acceptance as separate facts under the canonical
[issue-severity policy](../references/assessment-taxonomy.md).

For any explicit evaluator or project configuration that materially increases
coverage, retain the untouched result beside the configured result. Record a
portable/scrubbed configuration locator (never an absolute/private path) and
digest, exact covered assets/clips and skeleton subset, the post-configuration
result, and any newly runnable checks or contracts. Configuration can close evaluator coverage
without changing or repairing delivered animation bytes, and does not prove
artistic correctness, engine acceptance, or applicability beyond that subset.
An explicit rig-role map is the concrete example; automatic or case-insensitive
aliases must detect ambiguity and refuse rather than select among multiple
bones.

For gait anchoring, explicitly state whether any root translation or yaw
accumulates. AnimSmith versions that cyclically resample every channel must not
be recommended on root-motion clips without independently re-derived
displacement and yaw proof for a trajectory-preserving operation.
Also record whether the current version accepted or refused each representative
set, whether the selected root heading basis was measurable, and whether an
output was actually produced. This section describes only the current result.

## Engine procedures and evidence

| Runtime | Version | Procedure | Observed result | Remaining gate |
|---|---|---|---|---|
| {{ENGINE}} | {{VERSION}} | {{REPRODUCIBLE_STEPS}} | {{RESULT}} | {{GAP}} |

## Rig, masking, and compatibility evidence

| Pack/rig/set pair | Skeleton/retarget | Scale/axes | Root policy | Timing/blend | Overall evidence |
|---|---|---|---|---|---|
| {{PAIR}} | {{RESULT}} | {{RESULT}} | {{RESULT}} | {{RESULT}} | {{RESULT_AND_LABEL}} |

For mode-specific combinations, include the conservative full-body state
handoff and each proposed mask composition separately. Distinguish co-import,
graph execution, visual blending, contact/IK acceptance, and target-character
retargeting as different evidence levels.

## Limitations and unknowns

1. {{MATERIAL_LIMITATION_OR_UNKNOWN}}

## Changes between AnimSmith versions

Record older evaluator identities, superseded results, and version-to-version
changes here only, newest first. Preserve exact provenance needed to interpret
the change, but omit internal implementation reasoning and ticket chronology
unless a public issue is the developer-facing current limitation. For an
initial evaluation, write: `AnimSmith {{SEMVER}} — Initial evaluation; no
earlier AnimSmith comparison.`

## Reproduction

Record source identity/digest, config/manifest digests, retained evidence
labels, and all engine procedures. For every evaluator batch, also record:

- artifact locator, archive SHA-256, and exact binary member path;
- binary version and SHA-256, source tag or commit, and working-tree state
  (`N/A` for an official artifact);
- required feature surface, including source formats and commands;
- top-level and required-command help checks, representative-format admission,
  commands, inputs, exit codes, and whether each check succeeded;
- a scrubbed logical preflight-evidence locator and its SHA-256 digest.

Keep machine-local paths and licensed inputs outside the report. Do not start
the exhaustive batch until the preflight proves the required feature and
format admission.

## Sources

- {{SOURCE_WITH_DIRECT_LINK_AND_SCOPE}}
