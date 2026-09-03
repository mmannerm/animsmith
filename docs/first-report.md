# Your first report

A finding tells you which clip, bone and frame are wrong. The HTML report
shows you: it plays back the exact pose-grid frames the checks judged, with
foot and root trails, metric charts, and a findings list that scrubs the
viewer to the judged frame when you click it. One file, no server, no
dependencies; attach it to a pull request or send it to whoever owns the fix.

The commands below run from a checkout of the repository; the clips are the
same synthetic samples used in [first lint](first-lint.md).

## Render the report

```console
$ animsmith --config examples/walk.animsmith.toml report examples/assets/walk-dirty.glb -o report.html
wrote report.html (1 clip(s), 3 finding(s), 0.0 MB)   # exits 0
```

Open `report.html` in a browser. This is the same report, embedded here and
scrubbed to the first finding:

<iframe src="visuals/walk-dirty.report.html#embed=1&finding=0" title="AnimSmith report for walk-dirty.glb" width="100%" height="520" loading="lazy"></iframe>

[Open it full size](visuals/walk-dirty.report.html). Drag to orbit, use the
wheel to zoom, and click a finding to jump to its frame. The charts share a
playhead with the 3D view, and each chart caption says what to look for in
it — derived from what your configuration declares for that clip and what the
checks judged, not from an assumption about what the clip is. A clip declared
`loop = true` is told its curves should end where they began; one that pins
`speed_mps` with animation-owned travel is told the root should keep
travelling at that speed, with no claim about the shape it travels; a clip
that declares neither is told so, and named the checks that judged it. A root
path is also marked where the track starts (a hollow circle) and where it ends
(a filled square), and the caption states how far apart the two are.

## Compare before and after

When a clip has been repaired or re-exported, render both versions into one
comparison report. The pair below is a synthetic gait with a seam and a
sliding foot on the left, and the corrected clip on the right:

```console
$ animsmith --config examples/report-comparison.animsmith.toml report \
    examples/assets/report-comparison-before.glb \
    --compare-after examples/assets/report-comparison-after.glb \
    --before-clip acceptance-matrix --after-clip acceptance-matrix \
    -o comparison.html
wrote comparison.html (1 clip(s), 5 finding(s), 0.1 MB)   # exits 0
```

[Open the comparison](visuals/foot-slide.comparison.html). Press play beside
the shared phase to run both sides together, or scrub it by hand. The judged
poses come first, then the shared root trajectory — fitted to its own panel,
with a scale bar in the corner and the magnification against the
role-trajectory panels in its caption, so a two-centimetre sway is legible
and still reads as two centimetres — and then each side's trails and gait.
The gait panel
shades the sampled stance intervals, and every panel's caption says what to
look for in it.

## Share evidence without sharing the asset

The report embeds the sampled pose grid, which is the motion itself. When the
clip is licensed or confidential, render an evidence-only report: findings,
coverage gaps, engine predictions and charts stay, the poses are omitted, and
the file is safe to attach to a vendor ticket.

```console
$ animsmith --config examples/walk.animsmith.toml report examples/assets/walk-dirty.glb -o evidence.html --evidence-only
wrote evidence.html (1 clip(s), 3 finding(s), 0.0 MB)   # exits 0
```

## Where next

- Something specific looks wrong in the engine: start from the
  [symptoms](symptoms/README.md).
- You want this in your export loop or CI:
  [for artists](animation-author-workflow.md) or
  [for game developers](game-developer-intake-workflow.md).
- You want the full flag list: the [CLI reference](cli.md#commands).
