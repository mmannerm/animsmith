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

Concretely: point it at a glTF/GLB or FBX export and it tells you —
before engine import, in seconds, with frame- and bone-level detail —
whether the clip will behave in a game runtime. Does the loop pop? Do
the feet slide? Does the measured root-motion speed match what the
character controller expects? Did this re-export silently change the
motion? It answers from the command line, from CI via stable exit
codes and versioned JSON, or from Rust as a library — and it repairs
the mechanical problems that are provably safe to fix.

## Why it exists

Every game team answers "is this clip game-ready?" by hand today: a
tech artist scrubbing a timeline, a reviewer squinting at a playblast,
a bug report from playtest. Format validators stop at spec
conformance — a walk cycle cut a quarter-stride short is perfectly
valid glTF and still pops every second of gameplay. Engine import
inspectors do see content problems, but they see them late, inside
editor workflows, after engine-specific conversion has already
reshaped the data — and every studio re-derives its custom import
checks from scratch.

animsmith is the missing layer in between: a content quality gate and
repair assistant for animation clips — the linter and test suite
counterpart to engine import inspectors. It gives animation teams the
same repeatable, reviewable, CI-friendly confidence that code teams
expect from tests and linters.

## What "game-ready" means

A clip is game-ready when it holds four contracts at once. animsmith
exists to check all four before the engine does:

- **Runtime contract** — the engine can import, sample, blend, and
  compress the clip predictably: finite values, unit rotations,
  monotonic key times, sane durations and key counts, no data the
  importer will silently discard.
- **Character controller contract** — the clip's spatial behavior
  matches the gameplay system that drives it: in-place vs root-motion
  classification is explicit, measured speed and stride match the
  locomotion design, feet stay planted when they should.
- **Retargeting contract** — the clip is usable on the intended rigs
  without silent degradation: expected hierarchy and bone names,
  known rest pose and units, no tracks the target engine drops
  without telling anyone.
- **Pipeline contract** — the state of an asset is understandable and
  reviewable: stable naming, actionable findings instead of vague
  pass/fail, machine-readable reports, and a clear path from "failed"
  to "fix in DCC" or "auto-repair safely".

The [game-ready clips guide](game-ready-clips.md) walks each contract
in depth — every runtime failure mode, and which check, repair, and
config covers it.

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
into measurable requirements: a machine-readable definition of "done"
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

- [Game-ready clips guide](game-ready-clips.md) — the four contracts
  in failure-mode depth: symptom, mechanics, check, repair, config.
- [Examples cookbook](../examples/README.md) — runnable workflows:
  CLI gates, repair, clip edits, contract configs, embedding.
- [Documentation index](README.md) — topic-by-topic reference pages.
- [README](../README.md) — install, quickstart, and the check
  catalog.
