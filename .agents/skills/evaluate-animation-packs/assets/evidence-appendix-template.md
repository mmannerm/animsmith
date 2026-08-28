# Animation pack evidence appendix: {{PACK_NAME}}

> Companion report: `{{REPORT_STEM}}.md` (replace with a relative Markdown link in the completed appendix)
>
> Evidence status: **{{COMPLETE_PARTIAL_OR_PREVIEW_ONLY}}** — {{LARGEST_EVIDENCE_BOUNDARY}}
>
> Evaluation date: **{{YYYY-MM-DD}}**
>
> Current evaluator: **AnimSmith {{SEMVER}}**
>
> Report format: **2**

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
If the primary runtime-set table contains detailed exact members, durations,
speeds, or ratios, link to that table here. Retain the grouping evidence and
validation boundary without duplicating the decision table or suggesting that
its measurements were not captured.

If no runtime sets exist, omit the table and write exactly: `No runtime sets
were identified.`

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

For an explicit rig-role map, retain the default unresolved-role result beside
the configured result, identify the covered skeleton variant, and record the
configuration digest. Configuration can close evaluator coverage without
changing or repairing the delivered animation bytes.

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

Record source identity/digest, evaluator identity, config/manifest digests,
commands, exit codes, retained evidence labels, and all engine procedures.

## Sources

- {{SOURCE_WITH_DIRECT_LINK_AND_SCOPE}}
