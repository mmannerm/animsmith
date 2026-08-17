---
name: evaluate-animation-packs
description: Evaluate commercial, free, sample, or preview-only skeletal animation packs for game-engine use and produce a concise technical Markdown report plus a reproducible evidence appendix. Use for marketplace pipeline intake, canonical clip-role and runtime-set classification, capability-oriented game validation profiles, engine-readiness assessment, AnimSmith remediation trials, blending or masking analysis, rig and retargeting compatibility, cross-pack compatibility, and triage between current AnimSmith fixes, plausible future generic tooling, engine/project configuration, and artist or vendor changes. Do not use for film/VFX-only suitability or as a substitute for license counsel, engine import testing, or artistic sign-off.
---

# Evaluate Animation Packs

Assess the delivered evidence in three distinct states: the untouched pack,
the pack after current AnimSmith operations, and the remaining engine or human
work. Produce two linked Markdown documents—a concise technical report and a
detailed evidence appendix—without treating a clean lint run as game-readiness
certification.

Resolve every bundled path relative to this canonical `SKILL.md`. From the
repository root, the canonical skill directory is
`.agents/skills/evaluate-animation-packs`; do not resolve resources through the
Claude adapter directory.

## Protect the source and the claim

- Work on authorized local files or public preview material only. Never buy an
  asset, accept new terms, bypass access controls, or redistribute licensed
  files without explicit authorization.
- Keep the acquired source immutable. Put inventories, configs, converted
  files, repaired files, reports, and engine imports in a separate evaluation
  workspace.
- Hash the source before running tools. Use
  `.agents/skills/evaluate-animation-packs/scripts/inventory_pack.py` when a
  directory is available; retain the manifest with the report evidence.
- Use only the evidence labels defined by the assessment taxonomy. Never
  promote user recollection, preview media, or a partial sample into a claim
  about files or transaction records that were not supplied.
- Report license facts and ambiguities, but do not give legal advice. Quote or
  link the controlling license/marketplace terms and recommend review when the
  intended use or redistribution rights remain uncertain.
- Do not upload licensed assets, generated reports containing proprietary
  motion data, or engine project files to public services.

Read [assessment taxonomy](references/assessment-taxonomy.md) before assigning
verdicts or remediation ownership. Read [engine and compatibility checks](references/engine-and-compatibility.md)
before evaluating any engine, blending, masking, retargeting, or cross-pack
claim. Read [clip taxonomy and evaluation manifest](references/clip-taxonomy.md)
before classifying motions or runtime sets. Read [validation profiles](references/validation-profiles.md)
before selecting hypothetical or target-game use cases. Use both [the technical
report template](assets/report-template.md) and [the evidence appendix
template](assets/evidence-appendix-template.md) for every final evaluation and
preserve their top-level sections.

## Establish scope

Determine and record:

- pack name, edition/version, vendor/source URL, acquisition date, optional
  price or free/sample status, license evidence, and whether the input is full,
  partial, or preview-only;
- target game types, camera distance, character type, gameplay needs, target
  engines and exact engine versions, platforms, skeletons, and any other packs
  that must interoperate;
- delivered formats, source/DCC files, engine-native packages, documentation,
  example controllers, meshes, rigs, and animation files;
- evaluation constraints such as unavailable engines, unavailable source
  files, encrypted archives, missing licenses, or time-limited samples.

Ask only for missing scope that would materially change the decision. If a
target engine or project contract is unknown, continue with engine-neutral
analysis and label runtime conclusions `not evaluated`; do not silently choose
an engine or invent a game contract.

Select the mandatory `marketplace-intake` validation profile plus only the
capability profiles justified by user requirements, vendor intent, observed
pack content, or a labeled generic evaluation hypothesis. Record every profile
and its activation basis in the evaluation manifest. An evaluator-selected
generic profile may establish caveats or unknowns, but must not penalize a
focused pack for lacking unrelated content.

For online research, use current vendor pages, controlling marketplace terms,
official engine documentation for the exact tested version, and the checked-out
AnimSmith documentation. Record URLs and access dates. Treat blogs, videos,
reviews, and forum posts as secondary context, not proof about the delivered
files.

When a constituent pack was acquired through a larger collection, keep the two
product identities separate. Label every observed price, version, release date,
listing, and license fact as collection-level or constituent-level. A current
collection version does not identify the revision of a constituent pack or a
local artifact.

## Snapshot the AnimSmith evaluator

Use the AnimSmith version requested by the user. Otherwise use the binary from
the checked-out repository version containing this skill. Do not substitute an
installed binary merely because it is easier to invoke.

Use documentation from the exact checkout, tag, or release that produced the
selected binary. If the user requests another version, obtain that version's
matching sources or release documentation in a separate location; never apply
the current checkout's command or schema claims to a mismatched executable.

Before assessing assets:

1. Record the repository commit when using a checkout.
2. Build with the checkout's documented process when needed.
3. Capture `animsmith --version`, top-level `--help`, and the help for every
   command used.
4. Record the invocation form, enabled command surface, config path and digest,
   and output/evidence paths.
5. Discover capabilities from that binary and its matching docs. Do not assume
   a command, repair, transform, check id, schema version, or format feature
   exists because another AnimSmith version had it.

When refreshing an earlier evaluation for a newer AnimSmith version, rerun the
baseline, every selected contract, every adopted or recommended remediation,
and any check that was previously unavailable because loader evidence was
missing. Treat earlier generated outputs as historical evidence. A new
fail-closed refusal is a current result, but it is not a successful remediation
and must not inherit the earlier version's post-transform claims.

Use the version-matched project sources as authorities:

- [README](../../../README.md) for the current check and command overview;
- [game-ready clips](../../../docs/game-ready-clips.md) for the readiness ladder and check
  boundaries;
- [pipeline scenarios](../../../docs/pipeline-scenarios.md) for raw-source and marketplace intake;
- [CLI](../../../docs/cli.md) and [output](../../../docs/output.md) for invocation and
  evidence contracts;
- command-specific docs such as [character assembly](../../../docs/character-assembly.md) and
  [scale](../../../docs/scale.md) only when those commands are actually candidates.

If the requested binary cannot be obtained or run, still complete the research
report, mark the tool-backed stages `not evaluated`, preserve the failure
evidence, and state what would unblock them.

## Create the inventory and coverage plan

Inventory the complete delivered tree, including licenses, documentation,
archives, source files, engine-native files, preview media, and AnimSmith input
candidates. Record unsupported and unreadable files rather than dropping them.

Build a coverage matrix before testing:

- include every delivered animation file in inventory counts;
- classify every logical motion into exactly one canonical primary role from
  `references/clip-taxonomy.md`; keep pack-specific context such as cover,
  weapon, posture, or vendor grouping in orthogonal tags rather than inventing
  new roles;
- count both physical files and logical motions. Group in-place/root-motion
  variants under one logical motion only when direct evidence establishes that they
  are intended counterparts; report timing, skeleton, phase, and other
  measured differences, and do not claim they differ only by root motion
  without track- or sample-level proof;
- define runtime sets separately from primary roles. Use them only for real
  blend, speed, sync, transition, mask, retarget, paired-interaction, or search
  relationships; a clip may belong to multiple sets;
- run inexpensive batch-safe checks on every AnimSmith-readable file when
  practical;
- select representative clips for visual and engine tests by declared use,
  not convenience: idle, locomotion directions and speeds, start/stop/pivot,
  jump or traversal, additive/aim, upper-body actions, contact-heavy actions,
  and known outliers;
- include each distinct skeleton/export variant and each pack in cross-pack
  comparisons;
- explain any sampling and never generalize sample results to untested files.

Use pack listings only to seed a content manifest. Verify names, counts, and
variants against delivered files wherever possible. Classify missing
transitions or gameplay coverage relative to the stated game, not to an
imaginary universal pack.

Retain a versioned evaluation manifest using
`urn:animsmith:skill:animation-pack-evaluation-manifest:1`. Record all canonical
roles including zero-count roles, runtime sets, validation-profile selection,
and the ten pipeline stages from the captured `docs/pipeline-scenarios.md`.
Run `scripts/validate_evaluation_manifest.py MANIFEST.json` before deriving
summary counts. Treat pipeline stages as process coverage and the readiness
ladder as outcome coverage; for example, a completed inspect stage may still
produce file-ready findings.

## Run the untouched baseline

Run the baseline before any conversion, repair, transform, or engine-side
reimport setting that changes interpretation.

1. Create an explicit empty baseline AnimSmith config outside the source tree
   so an ambient `animsmith.toml` cannot add undeclared expectations.
2. Run `inspect` on every readable input or a declared exhaustive subset when
   scale makes that impractical.
3. Run `measure --format json` and `lint --format json` over all readable
   inputs, split into deterministic batches if needed. Preserve stdout,
   stderr, and exit codes.
4. Generate `lint --format markdown` for human review; do not use it as the
   machine evidence authority.
5. Generate available offline HTML reports for representative and problematic
   clips. Visually inspect them before citing visual conclusions.
6. Run each requested engine's unmodified import and playback checks when the
   engine is available. Record exact importer settings and warnings.

Treat parse/load failures as out-of-the-box results. Keep format readiness,
mechanical findings, semantic coverage gaps, and engine behavior separate.
Exit code `0` means only what the version-matched CLI says it means; inspect
structured coverage rather than calling it a full pass.

Summarize the file-ready result explicitly using the mechanical check family
defined by the captured `docs/game-ready-clips.md`: `nan`, `time-monotonic`,
`quat-norm`, `quat-flip`, `duration-sanity`, `scale-keys`,
`non-uniform-scale`, and `constant-track`, plus any version-specific opt-in
mechanical policy that was actually enabled. Report error findings, non-gating
hygiene notes, coverage gaps, and inactive checks separately for every primary
role and affected runtime set.

## Add a declared-contract pass

Create an evaluation config only from user requirements, authoritative pack
documentation, and facts verified in the files. Label every interpretation of
clip names or intended behavior as an assumption until confirmed.

Declare applicable loop, duration/frame-grid, in-place/root-motion, speed,
required-bone, rig-role, gait-group, and sync-group expectations supported by
the current version. Derive declarations from the selected validation profiles,
but never turn a profile's likely needs into invented pack facts or tolerances.
Run lint again and preserve the config and digest.

Organize conclusions using the version-matched readiness ladder, not one flat
pass/fail score:

1. file-ready mechanical health;
2. clip-ready declared semantics;
3. set-ready timing, gait, sync, and blend prerequisites;
4. rig and use prerequisites such as required bones, retarget path, rest/bind
   state, scale, masks, sockets, IK, and attachments;
5. runtime integration;
6. gameplay, artistic, and production acceptance.

For each primary role and selected runtime set, state which applicable checks
actually ran and which readiness claims remain unavailable. Do not force
unrelated one-shots into a sync group merely to fill the table.

Do not guess skeleton roles, units, rest/bind corrections, clip intent, or
expected speeds merely to make checks run. A coverage gap is useful evidence;
do not conceal it with fabricated configuration.

## Trial current AnimSmith remediation

Apply only operations exposed and documented by the captured AnimSmith
version. Always write new outputs; never use in-place modification during an
evaluation.

- Run any available repair in dry-run mode first.
- Use repair for defects the current version describes as safely repairable.
- Use transforms only when the edit has an explicit source window, frame rate,
  loop declaration, gait contract, or other required intent.
- Before gait anchoring, inspect whether any resampled channel accumulates root
  translation or yaw. If the captured version cyclically resamples those
  channels, treat root-motion anchoring as unsafe unless a trajectory-preserving
  method and independent proof exist. Do not extrapolate an in-place success to
  a root-motion recommendation.
- Record whether every current-version gait-anchor trial produced an output or
  refused, including the exact safety policy and evidence behind the result.
  Verify that the selected root heading basis is measurable for the source rig.
  Describe a safe refusal as protection from a destructive rewrite, not as a
  repair of gait phase or set readiness.
- Use conversion only when it represents a real engine-facing handoff, and
  verify retained scene, skin, material, texture, and animation facts.
- Use assembly only with an authoritative base and an explicit versioned
  recipe. Exact-name remapping is not general retargeting.
- Use scale only with the declarations and selectors required by the current
  scale contract. Never infer a factor from character height, bounds, naming,
  or apparent engine size.

After every generated output:

1. rerun inspect, measurement, and config-backed lint;
2. compare before and after with the current `diff` command when available;
3. preserve command output and generated evidence;
4. import the generated candidate into the target engine when possible;
5. record which original problem changed, which facts remained stable, and
   which new findings or unknowns appeared.

Report effort as concrete steps plus `none`, `low`, `moderate`, `high`, or
`unknown`; explain assumptions instead of inventing precise labor hours.

## Test engine use, blending, and compatibility

Follow [engine and compatibility checks](references/engine-and-compatibility.md).
Distinguish static inspection, AnimSmith sampling, offline report inspection,
actual engine import, and gameplay playback.

For a broad game-engine evaluation with no narrower engine scope, maintain a
common-engine matrix covering at least Unity, Unreal Engine, Godot, and Bevy.
Research current official documentation for each runtime and prototype the
exact import/playback behavior where the engine and required licenses are
available. Record documentation research, prototype evidence, and unavailable
runtime evidence as different states. If this engine pass is explicitly
deferred, keep all four rows in the summary as `deferred` and do not infer what
their importers or runtimes will repair.

Evaluate within-pack compatibility before cross-pack compatibility. For every
important pair or group, decide separately whether it supports:

- direct use on one skeleton;
- engine retargeting with configuration only;
- current AnimSmith assembly or mechanical preprocessing;
- artist-authored retargeting or cleanup;
- no defensible conclusion from the available evidence.

When at least two constituents of a collection have been evaluated, create or
refresh a collection report pair even if the collection is incomplete. Mark it
as a partial rollup and name both evaluated and missing constituents; never
extend the two-pack verdict to the collection as sold. Build a namespaced
rollup manifest from the validated constituent manifests, and keep constituent
files, motions, and runtime sets traceable to their source pack.

Before concluding that two installed packages coexist safely, compare every
overlapping logical path by digest. A same-path match is positive packaging
evidence; a same-path byte conflict is a material integration finding. Compare
exact skeleton hierarchy/signature, shared reference-rig identity, units/axes,
root policy, timing, and delivered metadata independently. A matching
humanoid label or successful co-import is not enough.

For an unarmed/armed or otherwise mode-specific pairing, evaluate a full-body
state-machine handoff as the conservative baseline. Promote an upper-body mask
only after its exact base/action members, pelvis/root ownership, support-foot
behavior, prop/contact/IK requirements, and target-engine visual result are
tested. A headless mask graph that evaluates without exceptions proves graph
execution only. Keep kicks, lunges, displacement-bearing actions, and other
pelvis-driven motions full-body unless stronger evidence supports layering.

Explicitly analyze locomotion blends and sync, transitions, root-motion policy,
upper/lower-body masks, additive and aim use, attachment and IK expectations,
contact quality, and style mismatch when relevant to the target game. Do not
claim that shared bone names, a humanoid label, or successful import proves
good blending.

Use capability profiles rather than creating genre-specific checklists. A game
context may compose several profiles; record project-specific additions beside
the standard profile identifiers. Keep unselected profiles visible so absence
of evaluation cannot be mistaken for a pass.

For every material finding, explain the likely player- or developer-visible
impact in plain language: for example a pose pop, once-per-cycle pulse, foot
skate during a blend, frozen limb, controller double motion, wrong-sized
weapon, lost socket, or increased asset/evaluation cost. Cite the matching
repository-relative AnimSmith explanation and, where it materially helps the
reader, official documentation for the engine feature that consumes the data.
Documentation establishes why the prerequisite matters; it is not evidence
that the delivered pack succeeds in that engine.

## Classify every material problem

Assign exactly one primary owner and any secondary workaround using the
taxonomy:

- engine/project configuration;
- current AnimSmith safe repair;
- current AnimSmith declared transform, conversion, assembly, or scale;
- plausible future generic AnimSmith tooling;
- artist/author or vendor change;
- license/acquisition resolution;
- unknown pending evidence.

Call something a future AnimSmith candidate only when the desired change is
declarative, deterministic, format-neutral, and independently verifiable
without inventing animation or artistic intent. Search the current public issue
tracker for related work before citing an issue. Phrase the result as
potential, not a commitment or roadmap promise. Keep engine- or game-specific
policy in the consuming project.

Require artist/author work when the solution needs new motion, pose or timing
judgment, contact cleanup, deformation-aware retargeting, style matching,
missing transitions, additive-base creation, or recovery of information that
the file does not contain.

## Write and validate the report

Create a linked pair:

- `<vendor>-<pack>.md`: a concise technical decision report, normally
  1,500–2,000 rendered words and never more than 2,000 words;
- `<vendor>-<pack>-evidence.md`: the exhaustive evidence appendix.

This two-document contract is report format version 1. Record the format
version in both files so future AnimSmith revisions can migrate or compare
reports explicitly.

Copy `assets/report-template.md` and
`assets/evidence-appendix-template.md`. Fill every required section and use
explicit `Not evaluated`, `Not applicable`, or `Unknown` states rather than
deleting required coverage.

Write the primary report in this reader order:

1. technical verdict and largest evidence boundary;
2. complete, partial, and absent gameplay capabilities;
3. named important runtime-set members with exact delivered identifiers,
   explicit movement variants, loop policy, measured cycle durations,
   root-motion speeds, and an interpretation of within-set speed/stride ranges
   where applicable;
4. an implementable blend/integration recipe with coordinates or thresholds,
   loop/phase policy, and movement ownership;
5. one issue/remediation register, with player/developer impact, one primary
   owner, and applicable repository-relative `docs/game-ready-clips.md` links;
6. Unity, Unreal Engine, Godot, and Bevy status;
7. best fit, poor fit, cross-pack status, and limitations;
8. a short evidence/provenance boundary and sources.

The primary report is technical, not a commercial scorecard. Keep evaluator
license/setup failures, acquisition records, prices, and legal uncertainty out
of the technical verdict. Record them briefly as provenance or
evaluation-completeness facts in the appendix. Never use a numeric score.

Put the following only in the evidence appendix unless a material result needs
one sentence in the primary report:

- exhaustive canonical role inventory and reconciled physical/logical totals;
- runtime-set grouping evidence;
- all pipeline stages and validation profiles;
- readiness evidence tied to the canonical six-level ladder;
- raw mechanical/contract counts, commands, configs, digests, and artifacts;
- detailed engine procedures;
- full rig, mask, retarget, and cross-pack matrices;
- acquisition/license provenance and remaining unknowns.

Do not duplicate the issue register or redefine the readiness ladder in the
appendix. Link to the canonical ladder instead.

Keep detailed decision evidence where the reader needs it. In particular, an
implementable primary runtime-set table may retain exact members, timing, and
motion measurements. The appendix must link directly to that evidence and
preserve its grouping basis, validation status, and evidence boundary without
copying the full table or implying that the measurements are absent.

Preserve misspelled source identifiers exactly when they are evidence. If the
repository's spelling tooling needs an exception, scope it to the complete
identifier and the narrowest applicable document/file type; never add a
misspelled component to a global vocabulary or silently correct the source.

The pair must make these answers obvious:

- What works unchanged?
- What current AnimSmith makes usable, and under which declarations?
- What still needs engine configuration, artist/vendor changes, or future
  generic tooling?
- Which game types and animation systems fit or conflict with the pack?
- How should the important sets be built and blended?
- How complete and trustworthy is the evidence?

For a partial collection rollup, the pair must additionally make obvious:

- exactly which constituents are included and excluded;
- which conclusions come from constituent reports versus new cross-pack tests;
- whether shared package paths are byte-identical or conflicting;
- whether the recommended integration uses a full-body state handoff, a mask,
  or both, and the acceptance level actually reached;
- which gameplay gaps one constituent fills and which remain when combined.

Run:

```text
.agents/skills/evaluate-animation-packs/scripts/validate_report.py \
  REPORT.md --appendix REPORT-evidence.md
```

Resolve placeholders, pair-link errors, structural errors, and primary-report
length errors before delivery. Link retained evidence, but do not publish
licensed source assets.

Markdown structure is parsed with the repository's pinned `pulldown-cmark`
CommonMark/GFM parser. Never replace that parser with regular expressions or
line-oriented recognition of headings, tables, links, code, or HTML blocks.
Completed reports must not contain raw HTML, including HTML comments.

When changing the skill or publishing repository reports, run
`just animation-pack-skill`. It exercises the helper executables and validates
every report/appendix pair under `docs/reports/`. The same check is part of
`just gates`.
