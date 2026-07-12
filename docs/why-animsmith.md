# Why animsmith

What animsmith is, why it exists, and what it is worth to your team.

This page is for someone deciding whether to adopt animsmith — a lead
evaluating tools, a producer weighing pipeline changes, an engineer
asked "should we use this?". If you already use animsmith and want
reference material, start at the [documentation index](README.md)
instead.

## What animsmith is

animsmith validates, measures, reports, and prepares skeletal
animation clips for game-engine import and runtime use.

Concretely: point it at a glTF/GLB or FBX export and it answers —
before engine import, in seconds, with frame- and bone-level detail —
questions that otherwise wait for a playtest: Does the loop pop? Do
the feet slide? Does the measured root-motion speed match what the
character controller expects? Did this re-export silently change the
motion? It works from the command line, from CI via stable exit codes
and versioned JSON, or from Rust as a library — and it repairs the
mechanical problems that are provably safe to fix.

## Why it exists

Every game team answers "is this clip game-ready?" by hand today: a
tech artist scrubbing a timeline, a reviewer squinting at a playblast,
a bug report from playtest. Nothing open-source does game-semantics
clip validation. Format validators stop at spec conformance — a run
cycle whose feet skate across the ground is perfectly valid glTF; the
validator has no concept of a foot, a loop, or root motion. Engine
importers do see content problems, but they see them late, inside
editor workflows, after import-time conversion has already reshaped
the data — and every studio re-derives its custom import checks from
scratch. Academic motion-metric code lives in ML-evaluation repos, not
artist tools.

animsmith is the missing layer in between: a content quality gate and
repair assistant for animation clips — the linter and test suite
counterpart to engine import inspectors, packaged as a standalone Rust
library and CLI: glTF/GLB native, FBX ingested for DCC exports,
engine-agnostic, with machine-readable output for CI. It gives
animation teams the same repeatable, reviewable, CI-friendly
confidence that code teams expect from tests and linters.

## What "game-ready" means

"Game-ready" is staged evidence, not a stamp — a clip is ready *for a
consumer*, and the last stages of readiness belong to that consumer.
The [readiness ladder](game-ready-clips.md#the-readiness-ladder) is
the canonical definition; it stages the evidence from file-ready data
to shipped acceptance. What animsmith contributes splits three ways:

- **Validated generically, on every file:** the data is mechanically
  sound — finite values, unit rotations, monotonic key times, sane
  durations, no accidental scale animation or export bloat — and the
  losslessly repairable defects are repaired.
- **Validated against your declarations:** the semantic contracts
  only you can state — loops, in-place policy, speed pins, required
  bones, blend sets — declared in a project config, resolved through
  a rig profile, and skipped with a note rather than guessed when a
  prerequisite is missing.
- **Consumer-owned:** importer and blend-graph behavior, controller
  feel, artistic quality, and shipping sign-off. animsmith's reports
  and measurements are evidence for that review, not a substitute
  for it — no standalone tool can certify behavior inside a runtime
  it has never seen.

The [game-ready clips guide](game-ready-clips.md) covers the
measurable ground symptom by symptom: the runtime failure you see
when a declared contract is violated, the mechanics behind it, and
the check, repair, and config that address it.

## What it is worth, by role

**Artists and animators** get actionable frame- and bone-level reports
instead of "the import looked wrong": which clip, which bone, which
frame, and a fix hint that maps to a DCC action — trim frames, freeze
scale, rename bones, fix the rest pose. `animsmith lint export.fbx`
runs seconds after export, while the DCC session is still open.

**Technical animators** codify engine and rig assumptions once —
loop flags, speed pins, blend rings, rig profiles — in a
[project config](../examples/README.md#4-a-project-contract-config),
then batch-validate whole libraries against it instead of re-checking
every delivery by hand.

**Gameplay engineers** stop debugging capsule, root-motion, and
foot-slide problems after import. Root travel, stride, and gait phase
become measured numbers checked against declared tolerances, and
[CI gates](../examples/README.md#1-a-first-cli-gate) catch a drifted
re-export before it reaches a playtest.

**Producers and pipeline owners** turn outsourced-asset acceptance
into measurable requirements: a machine-readable acceptance contract
that both sides can run, fewer review round-trips, and asset state
that is visible in a report rather than in someone's head.

**Tool engineers** build on a Rust
[library](embedding.md) and a
[versioned JSON envelope](output.md) with published schemas, instead
of scraping engine import logs — the same checks the CLI runs, inside
your own pipeline.

## What animsmith is not

animsmith is not a converter, an animation editor, or a DCC
replacement. It converts FBX exports to glTF as pipeline plumbing and
applies byte-surgical repairs whose correctness its own checks can
verify, but artistic transformation — retargeting, motion editing,
re-authoring — is DCC work and stays out of scope. It also does not
replace glTF-Validator or engine importers: it sits between them,
checking the content semantics the format validator cannot see,
earlier and more repeatably than the engine can.

## Where to go next

Convinced, or curious? The
[game-ready clips guide](game-ready-clips.md) covers each failure mode
in depth — mechanics, check, repair, config — and everything else,
from install to runnable workflows, is routed from the
[documentation index](README.md).
