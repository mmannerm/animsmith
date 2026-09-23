# Animation pack evaluation: {{PACK_NAME}}

> Technical verdict: **{{USABLE_USABLE_WITH_CONDITIONS_RESTRICTED_USE_POOR_FIT_OR_INSUFFICIENT_TECHNICAL_EVIDENCE}}**
>
> Evaluation completeness: **{{COMPLETE_PARTIAL_OR_PREVIEW_ONLY}}** — {{LARGEST_EVIDENCE_BOUNDARY}}
>
> Confidence: **{{HIGH_MEDIUM_OR_LOW}}**
>
> Evaluation date: **{{YYYY-MM-DD}}**
>
> Current evaluator: **AnimSmith {{SEMVER}}**
>
> Report format: **3**
>
> Detailed evidence: `{{REPORT_STEM}}-evidence.md` (replace with a relative Markdown link in the completed report)

## Technical decision

{{ONE_SHORT_OUTCOME_FIRST_TECHNICAL_DECISION}}

State camera and character scope: full-body or dedicated viewmodel content,
intended camera, and actually tested camera acceptance are separate facts.
An untested first-person view is not evidence that first-person assets are absent.

State separately:

- what works unchanged;
- what current AnimSmith makes usable;
- what still needs engine, artist/vendor, or future-tool work;
- the largest confidence boundary.

End with one developer decision for the stated use: use unchanged, adopt a
named configuration or verified output, run a bounded pilot, request the exact
artist/vendor change, or reject. Name the acceptance evidence that would move
the decision when it remains conditional.

If any explicit evaluator or project configuration materially increases
evaluation coverage, state the untouched result and the config-backed result
separately, including measured, linted, or otherwise validated results as
applicable, and summarize the outcome with a required link to the appendix's
configuration evidence. The appendix is authoritative for the portable/scrubbed
configuration locator, digest, exact covered assets, clips, skeleton variants,
post-configuration result, and newly runnable checks or contracts; never put an
absolute or private path in the primary report. Do not
describe configuration as source-asset repair or generalize beyond the
measured subset; it does not prove artistic correctness, engine acceptance,
or semantic applicability. For example, a rig-role map may close an evaluator
coverage gap for only the skeleton variants it names; automatic or
case-insensitive aliases remain acceptable only when ambiguity is detected and
handled fail-closed.

Lead with the usable set, its main blocker, and the next action. Write for the
developer choosing a controller or the artist repairing a clip. Explain
internal evaluation terms in ordinary words; link detailed evidence instead of
narrating the evaluation process.

Write this section and every ordinary report section as the current evaluation
against the declared current evaluator. Do not narrate earlier tool behavior,
ticket history, superseded measurements, or the steps by which the current
result was reached here.

Do not let evaluator setup, price, or transaction-record gaps change the
technical verdict. Put provenance and evaluation-completeness limitations in
their own places.

For a collection rollup, state whether it is partial, name the evaluated and
missing constituents, link each constituent report, and reserve the verdict
for the evaluated combination. Prefer a full-body state handoff as the
conservative unarmed/armed baseline. A headless mask or blend execution is not
visual/contact acceptance and must not by itself justify layered use.

## Capability coverage

### Content present

- {{OBSERVED_OR_FILENAME_INFERRED_CONTENT_AND_BASIS}}

### Content gaps and unknowns

- {{CONTENT_NOT_FOUND_OR_UNCLASSIFIED_WITH_SCOPE}}

### Evaluation still needed

- {{UNTESTED_RUNTIME_VISUAL_OR_CONTACT_BEHAVIOR}}

Use gameplay capabilities, not marketing families. Explicitly cover core
locomotion, transitions, airborne/traversal, combat/actions, reactions/deaths,
paired interactions, additive/aim use, and first-person content.

## Runtime sets and authored motion

Summarize each coherent set once. An eight-direction gait ring is one set;
in-place and root-motion rings are separate alternatives with separate owners.
Collection summaries compare all evaluated constituents and link their evidence.
Do not enumerate eight direction members as eight adoption decisions.

| Set | Controller use | Adoption decision | Exact members |
|---|---|---|---|
| {{SET}} | {{CONTROLLER_MOVEMENT_AND_TOPOLOGY}} | {{UNCHANGED_PROTOTYPE_CONFIGURATION_TOOL_CANDIDATE_SOURCE_FIX_OR_UNKNOWN_WITH_LIMIT}} | `{{REPORT_STEM}}-evidence.md#exact-runtime-members` (replace with a relative Markdown link) |

Use the same set name in the appendix's exact-member table and inventory.
A collection may instead link a constituent's exact-member appendix; identify
its scope without manufacturing a collection contract. Keep the adoption
answer short and state what changed after a tool trial and what remains.
If no important sets exist, omit the table and write exactly:
`No important runtime sets were identified.`

## Integration recipe

1. **Members/topology:** {{NAMED_SET_MEMBERS_BLEND_TOPOLOGY_AND_THRESHOLDS}}
2. **Timing/synchronization:** {{LOOP_PHASE_TRANSITION_OR_CONTACT_POLICY}}
3. **State ownership:** {{MOVEMENT_ACTION_INTERACTION_OR_STATE_OWNER}}
4. **Composition constraints:** {{TRANSITION_MASK_ADDITIVE_SOCKET_OR_IK_POLICY}}
5. **Acceptance gate:** {{TARGET_ENGINE_AND_VISUAL_ACCEPTANCE_GATE}}

A recipe must be implementable: name members, blend coordinates/thresholds,
loop flags, phase policy, movement owner, and what may not be mixed. Link the
relevant repository guidance for loop, gait, root-motion, rig, or scale issues.
Write these steps as instructions to the developer, without internal policy
identifiers or `key=value` tags. State exactly what has not been tested. The
appendix retains structured runtime contracts and detailed measurements.

Cover each material common pattern present in scope: a named blend tree;
controller ownership of translation, yaw, and collision; full-body transition
and any separately gated layer; and namespaced cross-pack state/layer
composition. A recipe may be a bounded project proposal when runtime evidence
is unavailable, but it must state what is untested and
name the engine, visual, or contact checks needed before use.

## Technical issue register

Keep one issue register in the primary report; the appendix supplies evidence
rather than a second ownership view. Link each issue to the closest applicable
section of `docs/game-ready-clips.md`. If no section applies, write `Guidance:
not applicable` in the problem cell. If no material issue was found at the
stated scope, omit the table and write exactly: `No material technical issues
were found at the stated scope.`

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| {{ISSUE_ID}} | {{AS_DELIVERED_SEVERITY}} | Scope: {{EXACT_FILES_OR_SET_MEMBERS}}; reproduce: {{BOUNDED_COMMAND_CONFIG_OR_ENGINE_STATE}}; impact: {{PLAYER_OR_DEVELOPER_VISIBLE_RESULT}}; link `../game-ready-clips.md#{{RELEVANT_SECTION}}` in the completed report | {{ONE_PRIMARY_OWNER}} | Action: {{CONCRETE_CHANGE_OR_PROJECT_DECISION}}; acceptance: {{OBSERVABLE_PASS_CONDITION}}; residual: {{SEVERITY_AFTER_ACCEPTED_FIX_OR_UNRESOLVED}} | {{FUTURE_FEASIBILITY_SAFETY_AND_PROOF}} | {{CONFIDENCE_AND_STATUS}} |

Apply the canonical [issue-severity](../references/assessment-taxonomy.md)
and [ownership](../references/assessment-taxonomy.md) policies.
Fill the separate severity/residual fields above, with available time/frame,
bone, contact, or transition boundaries for artist/vendor work.

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

For collection reports, include digest results for overlapping package paths,
namespaced cross-pack members, exact skeleton/reference-rig evidence, and the
gameplay gaps that one constituent fills for another. Do not infer compatibility
from shared role names, humanoid labels, or co-import alone.

## Changes between AnimSmith versions

List only evaluator-version changes, newest first. Keep this concise and useful
to a returning reader: changed conclusions or measurements, results explicitly
revalidated as unchanged, and operations that became available, unavailable,
or fail-closed. Put all prior evaluator behavior, superseded evidence, ticket
implementation history, and before/after version comparisons here—not in the
technical decision, issue register, engine status, recipe, or evidence status.
A current safety refusal is not a successful remediation; keep any older
generated-output measurements explicitly historical here and do not use them
as the current integration recipe.
For an initial evaluation, write: `AnimSmith {{SEMVER}} — Initial evaluation;
no earlier AnimSmith comparison.`

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
