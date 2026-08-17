# Animation pack evaluation: {{PACK_NAME}}

> Evaluation status: {{FULL_PARTIAL_OR_PREVIEW_ONLY}}
>
> Overall recommendation: {{ADOPT_ADOPT_WITH_CONDITIONS_PROTOTYPE_ONLY_DO_NOT_ADOPT_OR_INSUFFICIENT_EVIDENCE}}
>
> Confidence: {{HIGH_MEDIUM_OR_LOW}}
>
> Evaluation date: {{YYYY-MM-DD}}

## Executive decision

### Decision

{{ONE_SHORT_OUTCOME_FIRST_DECISION_AND_LARGEST_CONFIDENCE_BOUNDARY}}

### Canonical clip-role inventory

Use every versioned primary role, including zero-count rows. Count physical
files and logical motions from the validated evaluation manifest. Group
in-place/root-motion variants under one motion only when evidenced.

| Canonical primary role | Logical motions | Delivered files | Material tags or variants | Classification evidence |
|---|---:|---:|---|---|
| {{PRIMARY_ROLE}} | {{COUNT}} | {{COUNT}} | {{TAGS_AND_VARIANTS}} | {{EVIDENCE_AND_CAVEAT}} |
| **Total** | **{{COUNT}}** | **{{COUNT}}** |  | {{MANIFEST_AND_DIGEST}} |

### Runtime-set inventory

List only groups with a real runtime relationship. A clip may appear in
multiple sets; runtime-set totals therefore do not reconcile to clip totals.

| Runtime set | Type | Members/variants | Intended relationship | Grouping evidence | Validation status |
|---|---|---|---|---|---|
| {{SET_NAME}} | {{SET_TYPE}} | {{MEMBERS_AND_VARIANTS}} | {{RELATIONSHIP}} | {{EVIDENCE_AND_CONFIDENCE}} | {{CLEAN_FINDING_PARTIAL_NOT_EVALUATED_OR_NA}} |

### Pipeline-stage coverage

Keep process completion separate from readiness outcomes. Include all ten
stages even when a stage is not applicable or was not evaluated.

| Stage | Coverage state | Pack result or required decision | Evidence / next gate |
|---|---|---|---|
| Acquire | {{COVERAGE_STATE}} | {{RESULT}} | {{EVIDENCE_OR_GATE}} |
| Preserve raw | {{COVERAGE_STATE}} | {{RESULT}} | {{EVIDENCE_OR_GATE}} |
| Inspect | {{COVERAGE_STATE}} | {{RESULT}} | {{EVIDENCE_OR_GATE}} |
| Segment | {{COVERAGE_STATE}} | {{RESULT}} | {{EVIDENCE_OR_GATE}} |
| Root motion | {{COVERAGE_STATE}} | {{RESULT}} | {{EVIDENCE_OR_GATE}} |
| Conform | {{COVERAGE_STATE}} | {{RESULT}} | {{EVIDENCE_OR_GATE}} |
| Validate | {{COVERAGE_STATE}} | {{RESULT}} | {{EVIDENCE_OR_GATE}} |
| Optimize | {{COVERAGE_STATE}} | {{RESULT}} | {{EVIDENCE_OR_GATE}} |
| Export | {{COVERAGE_STATE}} | {{RESULT}} | {{EVIDENCE_OR_GATE}} |
| Gate/report | {{COVERAGE_STATE}} | {{RESULT}} | {{EVIDENCE_OR_GATE}} |

### Readiness ladder by clip set

Use `clean`, `finding`, `partial`, `not evaluated`, or `not applicable` with
the material counts. Keep mechanical errors separate from hygiene notes, and
state the likely runtime/user impact of every finding.

#### File-ready and clip-ready

| Primary role / runtime set | File-ready: mechanical | Clip-ready: declared semantics |
|---|---|---|
| {{ROLE_OR_SET}} | {{MECHANICAL_CHECKS_FINDINGS_NOTES_AND_CURRENT_RESULT}} | {{LOOP_ROOT_FPS_DURATION_BONES_AND_COVERAGE}} |

#### Set-ready and rig/use

| Primary role / runtime set | Set-ready: sync/blend prerequisites | Rig/use prerequisites | Practical result |
|---|---|---|---|
| {{ROLE_OR_SET}} | {{GAIT_SYNC_TIMING_CONTACT_AND_COVERAGE}} | {{SKELETON_REST_BIND_SCALE_MASK_SOCKET_IK_AND_COVERAGE}} | {{UNCHANGED_CURRENT_ANIMSMITH_ARTIST_ENGINE_OR_UNKNOWN}} |

### Tooling frontier

Do not call a future idea non-destructive merely because it is automatable.
State whether it is measurement-only, lossless, declared mechanical editing,
or motion-altering, and whether its postcondition can be proved independently.

| Problem and likely impact | Untouched | After captured AnimSmith | Plausible future generic tooling | Still left for engine / artist / vendor |
|---|---|---|---|---|
| {{PROBLEM_AND_VISIBLE_OR_PIPELINE_IMPACT}} | {{STATE}} | {{VERIFIED_RESULT_OR_GAP}} | {{FEASIBILITY_SAFETY_PROOF_AND_ISSUE_OR_NOT_SUITABLE}} | {{IRREDUCIBLE_OR_UNTESTED_WORK}} |

### Validation-profile status

Include every profile in the captured profile-set version. An evaluator-selected
generic scenario may establish a caveat or unknown, not an unrelated-content
failure.

| Validation profile | Selection and activation basis | Result | Evidence boundary / next test |
|---|---|---|---|
| Marketplace intake | {{SELECTED_AND_BASIS}} | {{RESULT}} | {{EVIDENCE_AND_GAP}} |
| Blended locomotion | {{STATUS_AND_BASIS}} | {{RESULT}} | {{EVIDENCE_AND_GAP}} |
| Root-motion controller | {{STATUS_AND_BASIS}} | {{RESULT}} | {{EVIDENCE_AND_GAP}} |
| State-machine transitions | {{STATUS_AND_BASIS}} | {{RESULT}} | {{EVIDENCE_AND_GAP}} |
| Layered upper body/weapons | {{STATUS_AND_BASIS}} | {{RESULT}} | {{EVIDENCE_AND_GAP}} |
| Traversal/environment | {{STATUS_AND_BASIS}} | {{RESULT}} | {{EVIDENCE_AND_GAP}} |
| Contact actions/interactions | {{STATUS_AND_BASIS}} | {{RESULT}} | {{EVIDENCE_AND_GAP}} |
| Retargeted/customizable characters | {{STATUS_AND_BASIS}} | {{RESULT}} | {{EVIDENCE_AND_GAP}} |
| Motion matching/search | {{STATUS_AND_BASIS}} | {{RESULT}} | {{EVIDENCE_AND_GAP}} |
| Networked movement | {{STATUS_AND_BASIS}} | {{RESULT}} | {{EVIDENCE_AND_GAP}} |
| Runtime performance | {{STATUS_AND_BASIS}} | {{RESULT}} | {{EVIDENCE_AND_GAP}} |

### Common-engine status

Keep documentation research and actual prototype evidence separate. For a
broad engine evaluation include Unity, Unreal Engine, Godot, and Bevy even when
the engine pass is deferred.

| Runtime | Evidence level | Pack result | Documented context, runtime compensation, and next evidence |
|---|---|---|---|
| Unity | {{DOCUMENTATION_ATTEMPTED_OR_PROTOTYPE}} | {{PACK_RESULT}} | {{DOCUMENTED_CAPABILITY_NOT_A_PACK_RESULT_AND_REMAINING_TESTS}} |
| Unreal Engine | {{DOCUMENTATION_ATTEMPTED_OR_PROTOTYPE}} | {{PACK_RESULT}} | {{DOCUMENTED_CAPABILITY_NOT_A_PACK_RESULT_AND_REMAINING_TESTS}} |
| Godot | {{DOCUMENTATION_ATTEMPTED_OR_PROTOTYPE}} | {{PACK_RESULT}} | {{DOCUMENTED_CAPABILITY_NOT_A_PACK_RESULT_AND_REMAINING_TESTS}} |
| Bevy | {{DOCUMENTATION_ATTEMPTED_OR_PROTOTYPE}} | {{PACK_RESULT}} | {{DOCUMENTED_CAPABILITY_NOT_A_PACK_RESULT_AND_REMAINING_TESTS}} |

### Best fit

{{BEST_FIT_GAME_TYPES_AND_WORKFLOWS}}

### Poor fit or material caveats

{{POOR_FIT_GAME_TYPES_AND_WORKFLOWS}}

### Adoption conditions

1. {{CONDITION_OR_NONE}}

## Evaluation scope and evidence

| Field | Value |
|---|---|
| Pack | {{PACK_EDITION_AND_VERSION}} |
| Vendor/source | {{VENDOR_AND_URL}} |
| Access | {{COMMERCIAL_FREE_SAMPLE_OR_PREVIEW}} |
| Price observed | {{PRICE_CURRENCY_DATE_OR_NOT_RECORDED}} |
| Delivered scope | {{FULL_PARTIAL_PREVIEW_AND_CONTENT_DESCRIPTION}} |
| Target game/use | {{GAME_TYPE_CAMERA_CHARACTER_AND_SYSTEMS}} |
| Target engines | {{ENGINE_NAMES_AND_EXACT_VERSIONS_OR_NOT_EVALUATED}} |
| Target rigs/packs | {{RIGS_AND_OTHER_PACKS_OR_NONE}} |
| License evidence | {{LICENSE_DOCUMENT_URL_DATE_AND_UNCERTAINTIES}} |
| Source manifest | {{PATH_OR_NOT_AVAILABLE}} |
| Evaluation manifest | {{PATH_DIGEST_SCHEMA_TAXONOMY_AND_PROFILE_SET_VERSION}} |

### Evidence coverage

| Surface | Offered/delivered | Evaluated | Findings | Not evaluated and why |
|---|---:|---:|---:|---|
| Animation files | {{COUNT}} | {{COUNT}} | {{COUNT}} | {{GAP}} |
| Distinct rigs/export variants | {{COUNT}} | {{COUNT}} | {{COUNT}} | {{GAP}} |
| AnimSmith default lint | {{COUNT}} | {{COUNT}} | {{COUNT}} | {{GAP}} |
| AnimSmith contract lint | {{COUNT}} | {{COUNT}} | {{COUNT}} | {{GAP}} |
| Offline visual reports | {{COUNT}} | {{COUNT}} | {{COUNT}} | {{GAP}} |
| Engine imports | {{COUNT}} | {{COUNT}} | {{COUNT}} | {{GAP}} |
| Blend/mask/retarget tests | {{COUNT}} | {{COUNT}} | {{COUNT}} | {{GAP}} |

### Claim legend

Use: `observed-file`, `observed-animsmith`, `observed-report`,
`observed-engine`, `vendor-stated`, `documentation-stated`, `inferred`, and
`not-evaluated`.

## Pack inventory and content coverage

### Delivery and organization

{{ARCHIVE_FORMATS_DIRECTORIES_DOCUMENTATION_SOURCE_FILES_DEMO_CONTENT}}

### Animation/gameplay coverage

| Family | Delivered clips/variants | Intended use | Material gaps for this game | Evidence |
|---|---|---|---|---|
| Idle/locomotion | {{DETAIL}} | {{DETAIL}} | {{DETAIL}} | {{LABEL}} |
| Starts/stops/pivots/transitions | {{DETAIL}} | {{DETAIL}} | {{DETAIL}} | {{LABEL}} |
| Jump/traversal | {{DETAIL}} | {{DETAIL}} | {{DETAIL}} | {{LABEL}} |
| Combat/actions/interactions | {{DETAIL}} | {{DETAIL}} | {{DETAIL}} | {{LABEL}} |
| Additive/aim/masked layers | {{DETAIL}} | {{DETAIL}} | {{DETAIL}} | {{LABEL}} |
| Reactions/death/other | {{DETAIL}} | {{DETAIL}} | {{DETAIL}} | {{LABEL}} |

## Out-of-the-box results

### Summary scorecard

| Readiness lane | Verdict | Evidence | Adoption consequence |
|---|---|---|---|
| Acquisition and rights | {{READY_CONDITIONAL_POOR_FIT_NA_OR_UNKNOWN}} | {{LABEL_AND_SUMMARY}} | {{CONSEQUENCE}} |
| Delivery completeness/organization | {{VERDICT}} | {{LABEL_AND_SUMMARY}} | {{CONSEQUENCE}} |
| AnimSmith-readable formats | {{VERDICT}} | {{LABEL_AND_SUMMARY}} | {{CONSEQUENCE}} |
| Untouched mechanical clip health | {{VERDICT}} | {{LABEL_AND_SUMMARY}} | {{CONSEQUENCE}} |
| Declared clip semantics | {{VERDICT}} | {{LABEL_AND_SUMMARY}} | {{CONSEQUENCE}} |
| Set/sync/locomotion behavior | {{VERDICT}} | {{LABEL_AND_SUMMARY}} | {{CONSEQUENCE}} |
| Rig/rest/bind/retargeting | {{VERDICT}} | {{LABEL_AND_SUMMARY}} | {{CONSEQUENCE}} |
| Root motion/in-place behavior | {{VERDICT}} | {{LABEL_AND_SUMMARY}} | {{CONSEQUENCE}} |
| Target-engine import/playback | {{VERDICT}} | {{LABEL_AND_SUMMARY}} | {{CONSEQUENCE}} |
| Masks/additive/IK/attachments | {{VERDICT}} | {{LABEL_AND_SUMMARY}} | {{CONSEQUENCE}} |
| Performance/runtime footprint | {{VERDICT}} | {{LABEL_AND_SUMMARY}} | {{CONSEQUENCE}} |
| Game/content/artistic fit | {{VERDICT}} | {{LABEL_AND_SUMMARY}} | {{CONSEQUENCE}} |
| Cross-pack compatibility | {{VERDICT}} | {{LABEL_AND_SUMMARY}} | {{CONSEQUENCE}} |
| Maintainability/reproducibility | {{VERDICT}} | {{LABEL_AND_SUMMARY}} | {{CONSEQUENCE}} |

### Untouched import and playback

{{DEFAULT_IMPORT_SETTINGS_WARNINGS_FAILURES_VISUAL_BEHAVIOR_AND_LIMITATIONS}}

### Untouched AnimSmith findings

| Finding or coverage gap | Affected scope | User-visible effect | Evidence |
|---|---|---|---|
| {{FINDING}} | {{FILES_CLIPS_OR_PERCENTAGE}} | {{EFFECT}} | {{LABEL_AND_ARTIFACT}} |

## AnimSmith results

### Captured evaluator

| Field | Value |
|---|---|
| AnimSmith version | {{EXACT_VERSION_OUTPUT}} |
| Repository commit | {{COMMIT_OR_NOT_APPLICABLE}} |
| Invocation | {{BINARY_OR_CARGO_INVOCATION}} |
| Available commands/features | {{HELP_SURFACE_SUMMARY}} |
| Baseline config and digest | {{PATH_AND_SHA256}} |
| Contract config and digest | {{PATH_AND_SHA256_OR_NONE}} |
| Evidence directory | {{PATH}} |

### Current-tool remediation trial

| Source issue | Operation and declarations | Result | Verification | Effort | Remaining caveat |
|---|---|---|---|---|---|
| {{ISSUE}} | {{EXACT_CURRENT_COMMAND_CLASS}} | {{RESULT}} | {{LINT_DIFF_ENGINE_OR_OTHER}} | {{BAND_AND_TASKS}} | {{CAVEAT}} |

### Before/after conclusion

{{WHAT_BECAME_USABLE_WHAT_DID_NOT_AND_WHAT_CHANGED_UNEXPECTEDLY}}

## Engine integration

### Import configuration

{{EXACT_ENGINE_VERSION_IMPORTER_SETTINGS_AND_WARNINGS_OR_NOT_EVALUATED}}

### Runtime playback and root motion

{{LOOPS_CONTACTS_DISPLACEMENT_YAW_GROUND_CONTROLLER_AND_NETWORK_CAVEATS}}

### Performance and packaging

{{IMPORTED_SIZE_MEMORY_EVALUATION_COST_COMPRESSION_AND_PLATFORM_RESULTS_OR_NOT_EVALUATED}}

## Blending, masking, and gameplay caveats

### Locomotion, sync, and transitions

{{BLEND_SPACE_PHASE_DURATION_ENDPOINT_CONTACT_AND_TRANSITION_RESULTS}}

### Upper/lower-body masking and additive use

{{MASK_BOUNDARY_KEY_COVERAGE_REFERENCE_POSE_WEAPON_AND_IK_RESULTS}}

### Game-type caveats

| Game/system context | Suitability | Caveat or required work | Evidence |
|---|---|---|---|
| {{CONTEXT}} | {{SUITABILITY}} | {{CAVEAT}} | {{LABEL}} |

## Compatibility

### Within-pack sets

| Clip set/pair | Skeleton | Root motion | Timing/sync | Runtime blend/mask | Result | Evidence |
|---|---|---|---|---|---|---|
| {{SET_OR_PAIR}} | {{RESULT}} | {{RESULT}} | {{RESULT}} | {{RESULT}} | {{DIRECT_ENGINE_CONFIG_ANIMSMITH_CURRENT_ARTIST_REQUIRED_INCOMPATIBLE_OR_UNKNOWN}} | {{LABEL}} |

### Cross-pack or target-rig compatibility

| Pack/rig pair | Skeleton/retarget | Scale/axes | Root policy | Timing/blend | Style/semantics | Overall | Evidence |
|---|---|---|---|---|---|---|---|
| {{PAIR}} | {{RESULT}} | {{RESULT}} | {{RESULT}} | {{RESULT}} | {{RESULT}} | {{CATEGORY}} | {{LABEL}} |

## Issue and remediation register

| ID | Severity | Problem and impact | Primary owner | Current workaround | Future AnimSmith potential | Confidence/status |
|---|---|---|---|---|---|---|
| AP-001 | {{BLOCKER_MAJOR_MODERATE_MINOR_OR_NOTE}} | {{PROBLEM}} | {{ENGINE_CONFIG_ANIMSMITH_CURRENT_SAFE_ANIMSMITH_CURRENT_DECLARED_ANIMSMITH_FUTURE_CANDIDATE_ARTIST_AUTHOR_VENDOR_LICENSE_OR_UNKNOWN}} | {{WORKAROUND_OR_NONE}} | {{NOT_NEEDED_NOT_SUITABLE_POTENTIAL_WITH_RATIONALE_AND_OPTIONAL_ISSUE_LINK}} | {{CONFIDENCE_AND_EVIDENCE}} |

## Acquisition and adoption guidance

### Value and expected work

| State | Usable scope | Required tasks | Effort | Owner |
|---|---|---|---|---|
| Untouched | {{SCOPE}} | {{TASKS}} | {{BAND}} | {{OWNER}} |
| After current AnimSmith | {{SCOPE}} | {{TASKS}} | {{BAND}} | {{OWNER}} |
| Target production state | {{SCOPE}} | {{TASKS}} | {{BAND}} | {{OWNER}} |

### Recommendation rationale

{{PURCHASE_OR_ADOPTION_RATIONALE_PRICE_VALUE_ALTERNATIVES_AND_CONDITIONS}}

## Limitations and unknowns

1. {{LIMITATION_AND_DECISION_IMPACT}}

## Reproduction appendix

### Source identity

{{MANIFEST_PATH_PACK_HASHES_EXCLUSIONS_AND_LICENSE_ARTIFACTS}}

### Evaluation manifest

{{VALIDATED_MANIFEST_PATH_SCHEMA_TAXONOMY_PROFILE_SET_AND_DIGEST}}

### AnimSmith commands and outcomes

```text
{{EXACT_COMMANDS_EXIT_CODES_AND_RELEVANT_OUTPUT_PATHS}}
```

### Engine procedure

{{PROJECT_VERSION_SETTINGS_TEST_SCENES_AND_STEPS_OR_NOT_EVALUATED}}

### Evidence artifacts

| Artifact | Purpose | Digest or identity |
|---|---|---|
| {{PATH}} | {{PURPOSE}} | {{SHA256_OR_SCHEMA_ID}} |

## Sources

- {{TITLE_OR_LICENSE}} — {{DIRECT_URL_OR_LOCAL_ARTIFACT}}, accessed {{YYYY-MM-DD}}.
