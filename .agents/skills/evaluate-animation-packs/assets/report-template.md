# Animation pack evaluation: {{PACK_NAME}}

> Technical verdict: **{{USABLE_USABLE_WITH_CONDITIONS_RESTRICTED_USE_POOR_FIT_OR_INSUFFICIENT_TECHNICAL_EVIDENCE}}**
>
> Evaluation completeness: **{{COMPLETE_PARTIAL_OR_PREVIEW_ONLY}}** — {{LARGEST_EVIDENCE_BOUNDARY}}
>
> Confidence: **{{HIGH_MEDIUM_OR_LOW}}**
>
> Evaluation date: **{{YYYY-MM-DD}}**
>
> Report format: **1**
>
> Detailed evidence: `{{REPORT_STEM}}-evidence.md` (replace with a relative Markdown link in the completed report)

## Technical decision

{{ONE_SHORT_OUTCOME_FIRST_TECHNICAL_DECISION}}

State separately:

- what works unchanged;
- what current AnimSmith makes usable;
- what still needs engine, artist/vendor, or future-tool work;
- the largest confidence boundary.

Do not let evaluator setup, price, or transaction-record gaps change the
technical verdict. Put provenance and evaluation-completeness limitations in
their own places.

## Capability coverage

### Complete core

- {{COMPLETE_GAMEPLAY_CAPABILITY}}

### Partial supporting gameplay

- {{PARTIAL_GAMEPLAY_CAPABILITY_AND_MISSING_PREREQUISITE}}

### Absent

- {{MATERIAL_ABSENT_CAPABILITY}}

Use gameplay capabilities, not marketing families. Explicitly cover core
locomotion, transitions, airborne/traversal, combat/actions, reactions/deaths,
paired interactions, additive/aim use, and first-person content.

## Runtime sets and authored motion

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

Use this comparison table for locomotion, sync, transition, mask-composition,
paired-interaction, motion-database, or other important sets. `Exact members`
must name every file-scoped member with the delivered case and spelling; never
silently normalize them from a vendor list or display label. State separately
when another bundled manifest or animation list disagrees. `Runtime contract`
captures the applicable loop, sync, state transition, mask, additive, contact,
or interaction policy.
Treat this table as decision evidence. When it carries the detailed per-member
measurements, the evidence appendix should link to it and preserve only the
grouping basis, validation status, and evidence boundary rather than duplicating
the rows.
Use semicolon-separated `key=value` timing terms (`duration`, `rm_speed`,
`sample_rate`, `frames`, or `threshold`) with finite non-negative values and
units. Use semicolon-separated runtime terms keyed by `loop`, `sync`,
`transition`, `mask`, `additive`, `contact`, `interaction`, `movement`, `state`,
`database`, or `playback`; use a specific lowercase/hyphenated value such as
`one-shot`, `gait-phase`, or `unknown`. Fields that do not apply stay explicitly
`N/A`. If no important runtime sets exist, write exactly: `No important runtime
sets were identified.` Retain the grouping evidence in the appendix.

Write `Variant/type` as one `variant=<id>` or `set_type=<id>` token. Moving
root-motion and paired IP/RM rows require `duration` and `rm_speed`; paired rows
also require distinct `loop_ip`, `loop_rm`, and `sync` policies. Do not repeat a
key with conflicting values. Prefix paired exact members with `IP` and `RM`;
any movement-labeled member requires the matching `in-place`, `root-motion`,
`rotation-only-root`, or `paired-ip-rm` variant.

## Integration recipe

1. **Members/topology:** `topology={{LOWERCASE_POLICY_ID}}`; {{NAMED_SET_MEMBERS_BLEND_TOPOLOGY_AND_THRESHOLDS}}
2. **Timing/synchronization:** `sync={{LOWERCASE_POLICY_ID}}`; {{LOOP_PHASE_TRANSITION_OR_CONTACT_POLICY}}
3. **State ownership:** `owner={{LOWERCASE_POLICY_ID_OR_NOT_EVALUATED}}`; {{MOVEMENT_ACTION_INTERACTION_OR_STATE_OWNER}}
4. **Composition constraints:** `composition={{LOWERCASE_POLICY_ID}}`; {{TRANSITION_MASK_ADDITIVE_SOCKET_OR_IK_POLICY}}
5. **Acceptance gate:** `gate={{LOWERCASE_POLICY_ID}}`; {{TARGET_ENGINE_AND_VISUAL_ACCEPTANCE_GATE}}

A recipe must be implementable: name members, blend coordinates/thresholds,
loop flags, phase policy, movement owner, and what may not be mixed. Link the
relevant repository guidance for loop, gait, root-motion, rig, or scale issues.
The inline `key=value` tokens are the comparable contract; keep the explanation
reader-facing and use `not-evaluated` rather than vague prose when evidence is
missing.

## Technical issue register

Keep one issue register in the primary report; the appendix supplies evidence
rather than a second ownership view. Link each issue to the closest applicable
section of `docs/game-ready-clips.md`. If no section applies, write `Guidance:
not applicable` in the problem cell. If no material issue was found at the
stated scope, omit the table and write exactly: `No material technical issues
were found at the stated scope.`

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| {{ISSUE_ID}} | {{SEVERITY}} | {{PROBLEM_AND_RUNTIME_IMPACT}}; link `../game-ready-clips.md#{{RELEVANT_SECTION}}` in the completed report | {{ONE_PRIMARY_OWNER}} | {{CURRENT_ACTION}} | {{FUTURE_FEASIBILITY_SAFETY_AND_PROOF}} | {{CONFIDENCE_AND_STATUS}} |

## Engine status

Keep documentation capability separate from observed pack evidence. For broad
game-engine evaluation include Unity, Unreal Engine, Godot, and Bevy even when
a runtime is not evaluated.

| Runtime | Evidence level | Technical result | Remaining gate |
|---|---|---|---|
| Unity {{VERSION_OR_UNSPECIFIED}} | {{LEVEL}} | {{OBSERVED_PACK_RESULT_OR_NOT_EVALUATED}} | {{NEXT_TEST}} |
| Unreal Engine {{VERSION_OR_UNSPECIFIED}} | {{LEVEL}} | {{OBSERVED_PACK_RESULT_OR_NOT_EVALUATED}} | {{NEXT_TEST}} |
| Godot {{VERSION_OR_UNSPECIFIED}} | {{LEVEL}} | {{OBSERVED_PACK_RESULT_OR_NOT_EVALUATED}} | {{NEXT_TEST}} |
| Bevy {{VERSION_OR_UNSPECIFIED}} | {{LEVEL}} | {{OBSERVED_PACK_RESULT_OR_NOT_EVALUATED}} | {{NEXT_TEST}} |

Say whether an engine test proves import, sampling, actual playback, visual
quality, full blend-space behavior, retargeting, build behavior, or only a
smaller subset.

## Fit and limitations

{{BEST_FIT_GAME_TYPES_AND_WORKFLOWS}}

{{POOR_FIT_GAME_TYPES_MISSING_CONTENT_AND_MATERIAL_CAVEATS}}

{{CROSS_PACK_COMPATIBILITY_RESULT_OR_REQUIRED_PAIRWISE_TEST}}

## Evidence status

State evaluated physical files, logical motions, evaluator version/revision,
manifest schema, and the largest unevaluated surfaces. Link the
canonical readiness ladder at `../game-ready-clips.md#the-readiness-ladder` and
the companion appendix. Acquisition/license evidence belongs here only as a
short provenance boundary; do not turn it into the technical decision.
If acquisition was through a collection, distinguish collection listing,
version, price, and license facts from constituent-pack identity and revision.

## Sources

- {{SOURCE_WITH_DIRECT_LINK_AND_SCOPE}}
