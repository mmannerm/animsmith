# Raw asset to game-ready pipeline

Where animsmith fits in an animation asset pipeline.

This guide is the process-level companion to the
[game-ready clips guide](game-ready-clips.md) and the
[examples cookbook](../examples/README.md). The game-ready guide explains
why a check fires; the cookbook gives runnable command transcripts. This
page shows how teams place those commands in the larger path from raw
source animation to engine-facing, reviewable, CI-gated assets.

The common shape:

1. Acquire the source asset.
2. Preserve the raw source and provenance.
3. Parse and inspect the file.
4. Segment long takes into named clips.
5. Decide root-motion policy per clip.
6. Retarget, conform, or clean up in DCC tools.
7. Validate motion semantics.
8. Optimize the exported data.
9. Export engine-facing artifacts.
10. Gate and report changes in CI.

animsmith does not replace DCC work, engine importers, or a team's
contract format. It gives the pipeline a repeatable measurement and
linting step around them: `inspect` for inventory, `measure` for raw
numbers, `lint` for the declared contract, `fix` for safe repairs,
`transform` for mechanical edits, `convert` for FBX/glTF handoff,
`diff` for regression checks, and `report` for review artifacts.

## Pipeline map

| Stage | Pipeline decision | animsmith workflow |
|---|---|---|
| Acquire | Is this source usable and redistributable? | Keep the vendor/source asset outside generated outputs; record license and intended rig/profile in project metadata. |
| Preserve raw | What must stay immutable? | Store raw FBX/glTF/GLB separately from any `fix`, `transform`, or `convert` outputs. |
| Inspect | What clips, bones, tracks, durations, and rig roles exist? | Use `inspect` for inventory and `measure` for metrics without judgment. |
| Segment | Which gameplay clips come from the take? | Use DCC slicing or mechanical `transform` slicing; declare clip names and frame policy in config. |
| Root motion | Is each clip in-place or root-motion? | Pin `[clips.<name>] in_place` and `speed_mps`; verify with `lint` and measure drift with `measure`. |
| Conform | What needs artistic cleanup or retargeting? | Use findings as DCC work orders; reserve `fix` and `transform` for safe mechanical operations only. |
| Validate | Does the clip meet the game contract? | Run config-backed `lint`; use `--deny-warnings` when warnings must block CI. |
| Optimize | Did cleanup change what matters? | Run `lint` for `constant-track` / `scale-keys` hygiene, then use `diff` for metric deltas that catch motion drift. |
| Export | Which artifact does the engine consume? | Use `convert` for FBX/glTF handoff; keep transformed GLB and any sidecars separate from raw source. |
| Gate/report | Can reviewers and automation trust the change? | Attach JSON or Markdown lint output to CI, and generate `report` artifacts for visual review. |

## Shared contract config

Every scenario below becomes much more useful once the team commits an
`animsmith.toml` next to the asset set. The exact tolerances belong to
your project, and the config is where the team records facts such as
which clips loop, which are in-place, what speed a locomotion clip
promises, which bones must move, and which clips form a directional
blend set. animsmith checks those facts against the measured asset. It
skips role-dependent semantic checks with a note when rig roles cannot
be resolved, rather than guessing.

Keep the process guide focused on where the contract is used. For the
canonical config shape, use the
[project contract config cookbook](../examples/README.md#4-a-project-contract-config),
the committed
[`examples/character.animsmith.toml`](../examples/character.animsmith.toml),
and the [configuration reference](../README.md#configuration).

## Cookbook routing

This guide keeps the pipeline shape here and the runnable transcripts in
the command-owned references:

| Pipeline need | Recipe or reference |
|---|---|
| Inventory, measurement, lint, and exit-code gating | [A first CLI gate](../examples/README.md#1-a-first-cli-gate) |
| Mechanical slicing, hold extension, or gait anchoring | [Editing a clip](../examples/README.md#3-editing-a-clip) |
| Shared clip contract and severity policy | [A project contract config](../examples/README.md#4-a-project-contract-config) |
| FBX handoff and HTML review reports | [Migrating an FBX export](../examples/README.md#5-migrating-an-fbx-export-default-features-only) |
| JSON, Markdown, and schema details | [Machine-readable output](output.md) |

## Scenario: marketplace-pack intake

Marketplace packs usually arrive as many clips with inconsistent naming,
export settings, skeletons, and quality. The intake goal is to sort the
pack into "usable now", "needs cleanup", and "reject or replace" before
artists build gameplay on top of it.

Store the downloaded source in a raw, immutable location with its
license and vendor metadata. Convert FBX assets into the format your
pipeline uses, inspect representative files, measure the pack, then lint
the batch before adding stricter project contract expectations. Use the
FBX handoff recipe for conversion and reports, the first CLI gate recipe
for inventory and batch linting, and the project contract recipe once
clip names and policies settle.

Use the first pass to catch obvious importer hazards: non-finite values,
quaternion problems, inconsistent durations, scale keys, constant tracks,
missing required bones, loop pops, wrong in-place/root-motion policy, and
gait-phase drift in blend sets. When a finding requires artistic
judgment, treat it as a DCC cleanup ticket instead of trying to auto-fix
motion.

## Scenario: mocap cleanup gate

Mocap data often moves through capture, solve, cleanup, retarget, slice,
and export steps. The risk is subtle drift: the cleaned clip looks
better, but a loop seam, stride phase, speed pin, or required-bone motion
changed without anyone noticing.

Measure the raw solve before cleanup. Use `transform` only for
mechanical edits such as slicing, hold extension, or gait anchoring, then
measure and lint before and after DCC cleanup with the same config.
Compare the cleaned output against the pre-cleanup or last accepted
revision with `diff`. Use the first CLI gate recipe for measurement and
linting, the editing recipe for mechanical `transform` work, and the
project contract recipe for semantic checks.

`diff` is the key review step: it catches meaningful changes in the
measurements animators and gameplay programmers care about, rather than
asking reviewers to compare binary assets or trust a re-export.

## Scenario: outsourced-asset acceptance

Outsourced animation acceptance works best when the vendor receives a
machine-readable contract instead of a vague "make it game-ready"
request. animsmith can be the acceptance gate both sides run locally.

Recommended contract package:

- The target skeleton or rig profile expectations.
- The committed `animsmith.toml`.
- A naming table for required clips.
- Required source and engine-facing artifact paths.
- The named cookbook recipe or wrapper script the vendor and the
  receiving team will run.

Use exit codes as the automation boundary:

| Exit code | Acceptance meaning |
|---:|---|
| 0 | Accepted by the lint contract. |
| 1 | Rejected until findings are fixed, or the contract is intentionally updated. |
| 2 | Delivery or command error: missing file, unreadable asset, bad config, or unsupported format. |

Use the project contract recipe for the shared `animsmith.toml`, then
pair the first CLI gate recipe with the machine-readable output
reference for JSON `lint` output and exit-code gating.

For review, attach the JSON output to the ticket and generate an HTML
report for clips with contested findings. The structured output makes it
clear whether rejection came from a semantic contract failure such as a
popped loop or from a mechanical problem such as bad quaternion keys.

If your acceptance system is already written in Rust, embed the check
catalog instead of shelling out. Load with `animsmith-gltf` or
`animsmith-fbx`, build `Config` from your own contract format, and map
`Finding` severities to your vendor workflow. The
[embedding guide](embedding.md) shows the API boundary.

## Scenario: CI gating on animation changes

CI should answer two questions on every asset change: does the new asset
still meet the declared contract, and did any important measurement move?

Run `lint` on changed clips with the project config, compare changed
clips against the previous accepted revision with `diff`, publish
human-readable review artifacts, and gate on process exit rather than by
scraping text. Use JSON for automation, Markdown and HTML for humans.
Use the first CLI gate recipe for CI `lint`, the project contract recipe
for config-backed checks, the FBX handoff recipe for `report`, and the
machine-readable output reference for JSON and Markdown outputs.

`--deny-warnings` is a project policy choice. Many teams start by gating
only errors, then promote warnings once the back catalog is clean.
Severity overrides in `[checks.<id>]` let a team make that transition one
check at a time without losing visibility.

## Scenario: raw vs transformed artifact storage

A reliable pipeline treats raw source and engine-facing outputs as
different artifacts with different ownership.

Use this split:

| Artifact | Ownership | Typical path | Notes |
|---|---|---|---|
| Raw source | Immutable vendor/capture/DCC input | `assets/raw/vendor_pack/run_forward.fbx` | Keep license, author, DCC version, export settings, and original units here. |
| Work-in-progress cleanup | Animator-owned intermediate | `assets/work/run_forward.blend` or `assets/work/run_forward.ma` | Artistic changes happen here, outside animsmith. |
| Mechanical transform output | Pipeline-generated | `assets/generated/run_forward.glb` | Produced by `convert`, `fix`, or `transform`; regenerate rather than hand-edit. |
| Contract config | Team-owned policy | `assets/animsmith.toml` | Version with the asset set; review changes like code. |
| Reports and measurements | CI/review artifacts | `reports/run_forward.html`, `animsmith-lint.json` | Usually generated per run, not committed unless the project wants baselines. |

This separation keeps `fix` and `transform` honest. Quaternion repairs,
frame slicing, hold extension, gait anchoring, and conversion are
mechanical steps whose results can be rechecked. Retargeting, pose edits,
contact cleanup, and motion redesign remain DCC work, then re-enter the
pipeline through `measure`, `lint`, and `diff`.

## Where to go next

- [Game-ready clips guide](game-ready-clips.md) explains the runtime
  symptoms behind each check.
- [Examples cookbook](../examples/README.md) provides runnable commands
  and expected output.
- [Embedding guide](embedding.md) shows how to drive the same checks from
  a Rust pipeline.
- [Machine-readable output](output.md) documents the JSON envelope and
  schema.
- [Game-ready animation research](research/game-ready-animation-clips.md)
  records the dated source research behind this process model.
