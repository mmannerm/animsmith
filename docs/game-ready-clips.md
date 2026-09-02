# What game-ready means

What animsmith checks for, and why exported clips fail at runtime.

A skeletal animation clip can pass every format validator and still
break the game it ships in: the loop pops, the character glides, feet
skate through blends, a limb stays T-posed. Those are not file-format
errors — the file is spec-conformant — they are *content* problems that
only surface after the slowest step of the pipeline: engine import, a
bake, a playtest.

This page defines what "game-ready" means here. The
[readiness ladder](#the-readiness-ladder) below stages the evidence
from file-ready data to shipped acceptance and says who owns each
level. The checks, repairs and config surfaces themselves live in the
[symptom index](symptoms/README.md), which routes every runtime symptom
— and the narrower presentations of one — to the page that walks it,
and carries every registered check id in one table.

Each symptom has its own page under [symptoms](symptoms/README.md):
what you see in the engine, what AnimSmith measured, the finding, the
repair, and the precise contract one click down. If you want runnable
commands, use the [examples cookbook](../examples/README.md); if you
want the current per-check reference for IDs, default findings,
prerequisites, config keys, coverage gaps, and remediation boundaries,
see the [built-in check reference](built-in-checks.md); if you want the
reasoning behind the tool itself — why it exists and what it is worth
to your team — see [why animsmith](why-animsmith.md).

## A valid file is not a usable clip

Format validators — Khronos glTF-Validator being the canonical one —
check *spec conformance*: accessor validity, buffer bounds, quaternion
norms at the container level. They have no concept of a loop, a gait,
or root motion. A clip whose walk cycle was cut a quarter-stride short
is perfectly valid glTF; it will also visibly pop every second of
gameplay.

The characteristics below fall into two groups, and animsmith treats
them differently:

- **Mechanical characteristics** hold for every clip, with no knowledge
  of your project: finite values, monotonic key times, unit
  quaternions, consistent durations.
- **Semantic characteristics** are contracts only you can declare:
  *this* clip loops, *this* one is authored in place, *these four*
  form a blend ring — declared in a
  [project config](../examples/README.md#4-a-project-contract-config)
  and resolved through a rig profile.

These two groups are the first two levels of the
[readiness ladder](#the-readiness-ladder), which states what animsmith
does about each.

Two loops benefit. The **artist inner loop** — `animsmith lint
export.fbx` seconds after a DCC export catches "the loop pops" or
"wrong rig" while the DCC session is still open, instead of after
import and bake. And the **CI gate** — the same checks with
machine-readable output and stable exit codes hold every committed
asset to the contract, so a re-export can't silently drift.

## The readiness ladder

"Game-ready" is not one property a tool can certify, because most of
it is relative to a consumer: *your* engine, *your* controllers,
*your* bar for quality. It is a ladder of evidence, and each level
has a different owner. animsmith's job is to make the early levels
checked and repeatable, make the declared middle measurable, and say
plainly what it did not evaluate — not to stamp the whole ladder.

1. **File-ready** — the data is parseable, finite, and mechanically
   valid: no NaN/Inf, monotonic key times, unit quaternions, sane
   durations, clean track hygiene. This is animsmith's primary
   generic coverage: the mechanical checks (`nan`, `time-monotonic`,
   `quat-norm`, `quat-flip`, `duration-sanity`, `scale-keys`,
   `non-uniform-scale`, `constant-track`) run on every file with no
   configuration. The narrower `constant-nonunit-scale` policy signal is
   registered but opt-in. `fix` repairs the two losslessly repairable defect classes
   (`quat-norm`, `quat-flip`).

2. **Clip-ready** — the clip honors what you declared about it: loop
   closure, duration and frame grid, in-place vs root-motion policy,
   required bone motion, structural rig presence, bind-pose consistency. Strong, config-backed
   coverage where a check exists: `fps`, `loop-seam`, `in-place`,
   `root-motion-speed`, `foot-slide`, `missing-bones`, `required-bones`, `frozen-bone`,
   and `bind-pose` judge exactly the expectations you declare — and
   the checks that need rig roles report a typed coverage gap instead
   of guessing when a role cannot be resolved. One member is heuristic:
   `foot-slide` ships as a warning (see
   [feet slide within one clip](symptoms/feet-slide.md)).

3. **Set-ready** — clips that blend or sync together are compatible
   as a set. Generic measurement and checking where implemented:
   `gait-group` holds a declared directional blend ring to a shared
   stride phase, `sync-group` checks same-time timing surfaces, and
   `time-complement` explains pairs whose stride phase aligns materially
   better under reflected time. `measure` supplies the per-clip numbers, and
   `animsmith diff` catches drift between revisions. Set
   compatibility beyond the implemented checks is yours to review.

4. **Rig and use prerequisites** — which bones play which roles on
   the target rig, which bones must exist, which bones a clip must animate, and what each
   clip is for. A shared boundary: you supply the meaning (a rig
   profile or `[rig.roles]`, `[rig] required_bones`, `animates_bones`, per-clip
   expectations), and animsmith resolves roles against the skeleton,
   checks the declarations, and reports the resolved roles it used.
   Nothing at this level can be inferred from the file alone.

5. **Runtime integration** — importer behavior, blend-graph
   topology, animation target IDs, masks, sync and reset behavior,
   and the poses your engine actually evaluates. Consumer-owned:
   animsmith's exact Bevy 0.19.0 profiles can predict the canonical
   `Animation{i}` source-animation selector and, in the current revision 3
   slice, negative animation/channel gate outcomes. It does not run
   the engine, prove runtime asset existence, or validate graph wiring, target
   survival, or positive playback. A dropped source row is prediction evidence,
   not a content finding; when both gates allow loading, runtime survival is
   required-unavailable.
   Its measurements come from its own documented sampling model — a model of
   engine samplers, not a reproduction of yours.

6. **Gameplay, artistic, and production acceptance** — controller
   feel and timing, readability, visual quality, provenance,
   reproducibility, shipping sign-off. Consumer-owned: reports and
   measurements inform the review; people make the call.

A clean run is evidence, and evidence has scope: it covers the checks
that ran, on the file that ran, against the contract you declared.
Only an actual animsmith run on the actual file establishes that
evidence — nothing transfers from vendor previews, other files in the
pack, or another export's report. And where generic validation touches
a later level, it supplies prerequisites or evidence for that level,
never blanket certification of it: a mechanically pristine,
contract-clean clip can still be rejected by your importer, your blend
graph, or your art director.

### Reading a lint run

One `animsmith lint` run answers five independent questions. Keep
them separate when you automate on the output:

- **Was the check active?** The full catalog is selected by default;
  `--select` narrows the selected set, and `[checks.<id>] severity = "off"`
  disables a check. A built-in opt-in check stays disabled until its severity
  is set to "note", "warn", or "error". Final JSON still records inactive
  checks without executing them.
- **Did it apply here?** Contract-aware checks judge only declared
  expectations. With no `loop = true` clip in the config, `loop-seam`
  has nothing to judge and is recorded as `not_applicable`.
- **Was the work evaluated?** When declared work exists but a prerequisite
  or measurement is missing, the check reports a typed coverage gap. A check
  can also complete part of its work: `gait-group` still validates declared
  ring members when unresolved roles keep it from measuring phase, then
  reports member existence as completed and phase coherence as a gap. Engine
  prediction work uses a separate `required_prediction_unavailable` facet;
  unlike an ordinary coverage gap, it makes `lint` exit `1` and cannot be
  suppressed with severity or `--allow`.
- **What did the evaluated work find?** Content findings at note,
  warning, or error severity, carrying clip, bone, time, and
  measured-vs-expected context.
- **What blocks?** Gate policy is yours, not animsmith's verdict:
  exit `1` on error findings, `--deny-warnings` to promote warnings,
  per-check severity overrides, and presentation-only `--allow` in text or
  Markdown. Coverage gaps never fail a run, while required-unavailable engine
  prediction work does. Exit `0` means no failing findings and no required
  prediction work remains unavailable; it still does not imply ordinary gap
  completeness. A gate that requires full coverage must inspect gaps too.

There is deliberately no single "pass" state: a run can complete with
warnings, and it can evaluate some declared work while skipping the
rest. See [machine-readable output](output.md) for the current v19
representation. It models selection, configuration, applicability, and
evaluation independently, keeps content findings separate from typed gaps,
and records completed work scopes. This is evidence about animsmith's checks,
not runtime certification; stricter completeness policy belongs to the
consuming pipeline.

Each clip symptom the levels above describe has a page of its own under
[symptoms](symptoms/README.md), whose one table routes it to the checks that
measure it, the repair, the config surface, and who owns the fix; the three
runtime problems that are not about a clip are answered on that index itself.

## Why animsmith exists

The positioning case — what animsmith is, why nothing else fills this
role, and what it is worth to each role on a team — lives in
[why animsmith](why-animsmith.md).

Everything else — the symptom pages themselves, runnable workflows,
pipeline scenarios, the CLI reference, embedding, and the dated engine
survey behind this page's contract — is routed from
[all pages](README.md).
