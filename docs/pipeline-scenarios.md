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
| Inspect | What clips, bones, tracks, durations, and rig roles exist? | `animsmith inspect source.fbx`; `animsmith measure --format json source.fbx` for metrics without judgment. |
| Segment | Which gameplay clips come from the take? | Use DCC slicing or `animsmith transform --slice START:END --fps N`; declare clip names and frame policy in config. |
| Root motion | Is each clip in-place or root-motion? | Pin `[clips.<name>] in_place` and `speed_mps`; verify with `lint` and measure drift with `measure`. |
| Conform | What needs artistic cleanup or retargeting? | Use findings as DCC work orders; reserve `fix` and `transform` for safe mechanical operations only. |
| Validate | Does the clip meet the game contract? | `animsmith lint --config animsmith.toml --format json clip.glb`; use `--deny-warnings` when warnings must block CI. |
| Optimize | Did cleanup change what matters? | `animsmith diff before.glb after.glb`; use `constant-track`, `scale-keys`, and metric deltas to catch bloat or drift. |
| Export | Which artifact does the engine consume? | `animsmith convert source.fbx -o clip.glb`; keep transformed GLB and any sidecars separate from raw source. |
| Gate/report | Can reviewers and automation trust the change? | Attach JSON or Markdown lint output to CI, and generate `animsmith report clip.glb -o report.html` for visual review. |

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

## Scenario: marketplace-pack intake

Marketplace packs usually arrive as many clips with inconsistent naming,
export settings, skeletons, and quality. The intake goal is to sort the
pack into "usable now", "needs cleanup", and "reject or replace" before
artists build gameplay on top of it.

Recommended flow:

1. Store the downloaded source in a raw, immutable location with its
   license and vendor metadata.
2. Convert FBX assets into the format your pipeline uses, keeping the
   source files unchanged:

   ```console
   animsmith convert vendor/run_forward.fbx -o generated/run_forward.glb
   ```

3. Inspect representative files, then measure the pack:

   ```console
   animsmith inspect generated/run_forward.glb
   animsmith measure --format json generated/*.glb > generated/measurements.json
   ```

4. Start with mechanical linting across the whole batch, then add the
   project contract as clip names and policies settle:

   ```console
   animsmith lint --format json generated/*.glb > generated/lint.json
   animsmith lint --config animsmith.toml --deny-warnings generated/*.glb
   ```

5. Generate reports for borderline clips that need human review:

   ```console
   animsmith report generated/run_forward.glb -o reports/run_forward.html
   ```

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

Recommended flow:

1. Measure the raw solve before cleanup:

   ```console
   animsmith measure --format json raw/session_42.glb > raw/session_42.measure.json
   ```

2. Slice or hold-extend only when the edit is mechanical and measurable:

   ```console
   animsmith transform raw/session_42.glb -o work/run_forward.glb --slice 12:13 --fps 30
   animsmith transform work/attack.glb -o work/attack_hold.glb --hold-extend 0.2
   ```

3. Measure and lint before and after DCC cleanup with the same config:

   ```console
   animsmith measure --format json work/run_forward.glb > work/run_forward.measure.json
   animsmith lint --config animsmith.toml work/run_forward.glb
   animsmith measure --format json clean/run_forward.glb > clean/run_forward.measure.json
   animsmith lint --config animsmith.toml clean/run_forward.glb
   ```

4. Compare the cleaned output against the pre-cleanup or last accepted
   revision:

   ```console
   animsmith diff work/run_forward.glb clean/run_forward.glb
   ```

5. For locomotion sets, align cycles only when the gait anchor is the
   mechanical problem:

   ```console
   animsmith transform clean/run_left.glb -o clean/run_left_anchored.glb --gait-anchor
   ```

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
- The exact command the vendor and the receiving team will run.

Example acceptance command:

```console
animsmith lint --config animsmith.toml --deny-warnings --format json deliveries/*.glb > acceptance-lint.json
```

Use exit codes as the automation boundary:

| Exit code | Acceptance meaning |
|---:|---|
| 0 | Accepted by the lint contract. |
| 1 | Rejected until findings are fixed, or the contract is intentionally updated. |
| 2 | Delivery or command error: missing file, unreadable asset, bad config, or unsupported format. |

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

Recommended flow:

1. Run lint on changed clips with the project config:

   ```console
   animsmith lint --config animsmith.toml --deny-warnings --format json clips/*.glb > animsmith-lint.json
   ```

2. Compare changed clips against the previous accepted revision:

   ```console
   git show origin/main:clips/run_forward.glb > /tmp/run_forward.previous.glb
   animsmith diff /tmp/run_forward.previous.glb clips/run_forward.glb
   ```

   If your CI needs more than one file, check out the previous revision
   into a temporary directory and diff those files.

3. Publish human-readable review artifacts:

   ```console
   animsmith lint --config animsmith.toml --format markdown clips/*.glb >> "$GITHUB_STEP_SUMMARY"
   animsmith report clips/run_forward.glb -o animsmith-run-forward.html
   ```

4. Gate on process exit, not by scraping text. Use JSON for automation,
   Markdown and HTML for humans.

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
