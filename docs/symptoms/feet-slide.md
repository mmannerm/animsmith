# Feet slide within a clip

A planted foot drifts along the ground during stance, so the character looks
like it is skating.

<img src="../visuals/icons/feet-slide.svg" alt="A foot marker that drifts while it should be planted" width="160" align="right">

Checks: [`foot-slide`](../built-in-checks.md#foot-slide)

## Why it happens

During stance the foot must move consistently with the clip's declared travel:
at `speed_mps` relative to the character for an in-place clip, or planted in
the world for a root-motion clip. Retargeting, resampling, a wrong speed pin,
or hand-keyed contacts break that relationship, and runtime IK and blend
band-aids exist to hide the result.

## What AnimSmith measures

The synthetic gait below has a left foot that skates during stance. The report
samples stance intervals with the run's contact threshold and shades them, so
the slide is visible as foot motion inside a shaded interval.

<iframe src="../visuals/foot-slide-before.report.html#embed=1&finding=3" title="AnimSmith report for report-comparison-before.glb, scrubbed to the first foot-slide finding" width="100%" height="520" loading="lazy"></iframe>

[Open the interactive report](../visuals/foot-slide-before.report.html) to
scrub the exact frames the checks judged; it opens on the first of the two
`foot-slide` findings. This clip declares a loop and a `speed_mps` pin, so the
gait panel's caption says its curves should end where they began and that the
shaded bands are the stance intervals `foot-slide` judged: a foot that moves
horizontally inside its band is the slide.

## What the finding looks like

```console
$ animsmith --config examples/report-comparison.animsmith.toml lint examples/assets/report-comparison-before.glb
examples/assets/report-comparison-before.glb:
  error[loop-closure] clip 'acceptance-matrix' bone 'left_foot' @1.000s: loop does not close in position: bone 'left_foot' is 0.4000 m from its first-frame model-space position (cap 0.0100 m) (measured 0.4000, expected 0.0100)
  error[loop-seam-vel] clip 'acceptance-matrix' bone 'left_foot' @1.000s: loop velocity changes at the seam: bone 'left_foot' differs by 1.3416 m/s between the incoming and outgoing model-space velocities (cap 0.1000 m/s) (measured 1.3416, expected 0.1000)
  warning[foot-slide] clip 'acceptance-matrix' bone 'left_foot' @0.250s: left foot skates during stance: speed deviates 0.72 m/s from the expected 1.00 m/s (cap 0.30) — foot plants will slip at runtime (measured 0.7200, expected 0.3000)
  warning[foot-slide] clip 'acceptance-matrix' bone 'right_foot' @1.000s: right foot skates during stance: speed deviates 0.88 m/s from the expected 1.00 m/s (cap 0.30) — foot plants will slip at runtime (measured 0.8800, expected 0.3000)
  note[constant-track] clip 'acceptance-matrix' bone 'hand': rotation track has 5 keys but never moves — export bloat
  coverage[bind-pose] insufficient_rotation_evidence ×1 (scopes: first_frame_rest_delta; subjects: acceptance-matrix): only 1 usable first-frame rotation track(s); at least three are required
2 error(s), 2 warning(s), 1 note(s), 1 coverage gap(s), 0 available prediction facet(s), 0 required-unavailable prediction facet(s)   # exits 1
```

## What the repair looks like

The after clip below is an authored repair: both sides come from this
repository's fixture generator, and the after side is the same gait with those
defects repaired. It is not the output of an AnimSmith transform, because
AnimSmith does not re-author contacts.

<iframe src="../visuals/foot-slide.comparison.html#embed=1" title="AnimSmith comparison of the synthetic foot-slide pair and its authored repair" width="100%" height="640" loading="lazy"></iframe>

[Open the interactive comparison](../visuals/foot-slide.comparison.html) to see
the two side by side; press play beside the shared phase to run them together,
or tick **Overlay after on before** to put the two skeletons in one pane. The
skeleton and the root path are the same on both sides, so what the panels show
is the stance repair.

What AnimSmith itself does here is the measuring and one narrow transform: it
reports the slide with the stance intervals it judged, and
`transform --gait-anchor` can rotate an eligible in-place cycle's phase to
choose a better stride cut. Anchoring changes which frame the cycle starts on,
not how fast a planted foot travels, so it does not repair this finding.
Contact cleanup stays DCC work.

## What to do

1. **Check the speed pin first.** Run `animsmith measure` on the clip and
   compare the measured horizontal root speed with the declared `speed_mps`;
   a stale pin produces a slide finding on a clean clip.
2. **Re-author the contacts in the DCC.** Plant the stance foot (or move it at
   the declared speed for an in-place cycle), then re-export and re-lint.
3. **Do not paper over it with runtime IK** before the source is clean; the
   check is there so the band-aid stays optional.

Who fixes it: the artist. AnimSmith can report the slide and, for an eligible
in-place gait, re-anchor the cycle, but contact cleanup and blend timing are
DCC and runtime work. The gate closes when the clip re-lints clean under the
declared contract and stance behaves in the actual blend.

## Config

```toml
[rig]
roles = { root = "root", hips = "hips", left_foot = "left_foot", right_foot = "right_foot" }

[clips.acceptance-matrix]
loop = true
speed_mps = { value = 1.0, tolerance = 0.1 }
movement_owner_xz = "gameplay"

[checks.foot-slide]
contact_height_m = 0.03
max_slide_mps = 0.3
```

<details>
<summary>Precise contract: stance detection, the declared speed, and why this check is a warning</summary>

During stance — the part of the stride where a foot is planted — the
foot must move consistently with the clip's declared travel: at
`speed_mps` relative to the character for an in-place clip, or planted
in the world for a root-motion clip. Deviation is the skate that
runtime IK and blend band-aids exist to hide.

The `foot-slide` check measures stance-phase foot velocity against the
declared `speed_mps` and the clip's measured horizontal root speed. It is the
research-grade check of the catalog: contact
detection is heuristic, so it ships as a warning with generous
defaults, and is judged only on clips that declare `speed_mps`.

The check's prerequisites, configuration keys and gap semantics are listed
with every other check in the
[built-in check reference](../built-in-checks.md#foot-slide).

</details>
