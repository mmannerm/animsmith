# The clip is the wrong length or freezes at the end

A limb stops dead a fraction of a second before the rest of the body, or a
clip that should be one second long plays a frame short in the engine.

<img src="../visuals/icons/wrong-length.svg" alt="Three channel bars on one timeline, the middle one stopping early and held flat to the end" width="160" align="right">

Checks: [`duration-sanity`](../built-in-checks.md#duration-sanity) ·
[`fps`](../built-in-checks.md#fps)

## Why it happens

A clip's duration is not one number in the file: it is whatever its longest
channel reaches, and an engine clamp-holds every channel that ends earlier. A
partial export, a slice that dropped one bone's tail keys, or a retiming step
that drifted off the frame grid all leave a file that looks valid and plays
wrong. The length a clip is *supposed* to be is project knowledge, so
AnimSmith checks the mechanical part always and the declared part only once you
pin it.

## What AnimSmith measures

The synthetic walk below has one left-ankle rotation channel that stops at
0.75 s while both translation channels run to 1.0 s. The report's judged frame
is the moment the short channel starts being held.

<iframe src="../visuals/walk-short-channel.report.html#embed=1&finding=0" title="AnimSmith report for walk-short-channel.glb, scrubbed to the judged frame" width="100%" height="520" loading="lazy"></iframe>

[Open the interactive report](../visuals/walk-short-channel.report.html) to
scrub the frames after the ankle stops.

## What the finding looks like

The spread is mechanical, so no configuration is needed to see it:

```console
$ animsmith lint examples/assets/walk-short-channel.glb
examples/assets/walk-short-channel.glb:
  warning[duration-sanity] clip 'walk_short_channel': channels end at different times (0.750s..1.000s) — shorter channels will be clamp-held (measured 0.2500, expected 0.0000)
  coverage[bind-pose] insufficient_rotation_evidence ×1 (scopes: first_frame_rest_delta; subjects: walk_short_channel): only 1 usable first-frame rotation track(s); at least three are required
0 error(s), 1 warning(s), 0 note(s), 1 coverage gap(s), 0 available prediction facet(s), 0 required-unavailable prediction facet(s)   # exits 0
```

The other half of this symptom — a clip whose channels agree but whose length
no longer matches the manifest — has no mechanical signature at all. Only a
declared `duration_s` (and `fps` for the frame grid) turns it into a finding.

## What to do

1. **Channels ending at different times.** The export dropped part of a
   channel. Re-export the full range; `duration-sanity` reports the spread, it
   does not resample the clip.
2. **A declared length that is missed.** Pin `duration_s` for the clip and
   let the check report the measured and expected seconds. The tolerance
   absorbs harmless exporter rounding, not a wrong authored range.
3. **Keys off the frame grid.** Declare `fps` and fix the retiming step that
   drifted. Unreal, for example, documents that
   [animations with non-whole end frames do not import
   correctly](https://dev.epicgames.com/documentation/en-us/unreal-engine/animation-sequences-in-unreal-engine).
4. **When the input is deliberately being edited.** `animsmith transform
   --slice` cuts a window on the frame grid and retimes it to start at zero,
   and `--hold-extend` appends a linear hold of the final pose for charge and
   block poses. See [editing a clip](../../examples/README.md#3-editing-a-clip).

Who fixes it: the pipeline. A partial export or a drifted retime is repaired
where it was produced — a re-export, or an explicit `transform` when the cut
itself is the intended edit. The gate closes when the clip's channels agree,
its declared duration and frame grid are met, and the engine plays the whole
range.

## Config

```toml
[clips.walk_short_channel]
# The authored range. Linting reports a missed pin; it never resamples.
duration_s = { value = 1.0, tolerance = 0.02 }
# The grid the clip was authored on, so `fps` can check that its keys stay
# on that grid and that it spans a whole number of frames.
fps = 32.0
```

<details>
<summary>Precise contract: channel ends, declared durations, the frame grid, and the mechanical edits</summary>

- **Channels that end at different times mean a partial export.** When
  one bone's track is shorter than the clip, the engine clamp-holds the
  shorter channel — a limb freezes while the rest of the body keeps
  moving. The `duration-sanity` check flags degenerate durations and
  mismatched channel ends.
- **A valid-looking clip can still be one frame too short.** Export slicing,
  inclusive-vs-exclusive frame ranges, and endpoint removal can produce a
  positive duration whose channels agree but which no longer matches the
  gameplay or animation manifest. Declare
  `duration_s = { value = 1.033, tolerance = 0.02 }` for the clip and
  `duration-sanity` reports the measured and expected seconds when that pin
  is missed. The value must be finite and positive, and the tolerance finite
  and non-negative; invalid pins are errors instead of silently disabling the
  contract. The tolerance absorbs harmless exporter rounding. Linting only
  reports the mismatch; it does not repair or resample the clip. Re-export
  when the authored range is wrong, or use the explicit slice/hold transforms
  below when the source is intentionally being edited.
- **Keys off the frame grid mean a retiming step drifted.** A clip with
  a declared frame rate should keep its keys on that rate's time grid
  and span a whole number of frames. Off-grid keys mean a resample or
  retiming step drifted; a fractional frame count means a slice cut
  mid-frame — and engines care: Unreal, for example, documents that
  [animations with non-whole end frames do not import
  correctly](https://dev.epicgames.com/documentation/en-us/unreal-engine/animation-sequences-in-unreal-engine). The
  `fps` check verifies both once the config declares a rate.

When the wrong length is the *input* problem — a capture with garbage
at the head, a one-shot that should hold its final pose — the
`transform` command does the mechanical edit:
`--slice` cuts a window on the frame grid and retimes it to start at
zero, and `--hold-extend` appends a linear hold of the final pose
(charge and block poses). See [editing a
clip](../../examples/README.md#3-editing-a-clip).

The checks' prerequisites, configuration keys and gap semantics are listed
with every other check in the
[built-in check reference](../built-in-checks.md#duration-sanity).

</details>
