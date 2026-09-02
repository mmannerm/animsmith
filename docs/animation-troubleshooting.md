# Animation troubleshooting

Start with the symptom, retain the source evidence, and route the work to the
owner who can change it. This page is the router: every row below opens a page
that shows what AnimSmith measured, names the owner, and states the evidence
that closes the gate. It deliberately links to the
[configuration reference](configuration-reference.md) and
[built-in check reference](built-in-checks.md) rather than restating their
contracts, and the [symptom index](symptoms/README.md) carries the complete
check-to-symptom table.

For every route, capture the current `lint --format json` result and, when
motion is contested, the offline `report` or before/after `diff`. A current,
repository-safe diagnostic pair is:

```console
# workflow-exit: 0
$ANIMSMITH --config "$CONFIG_DIR/report-comparison.animsmith.toml" report \
    "$ASSET_DIR/report-comparison-before.glb" \
    --compare-after "$ASSET_DIR/report-comparison-after.glb" \
    --before-clip acceptance-matrix --after-clip acceptance-matrix \
    -o "$WORK_DIR/troubleshooting-comparison.html"
```

It shows a deliberately defective loop and stance trail; it is diagnostic
evidence, not an engine or artistic acceptance result.

| What you see | Where to go |
|---|---|
| The pose flickers, spins, or explodes | [The pose flickers, spins, or explodes](symptoms/pose-flickers.md) |
| A limb freezes before the clip ends, or the length disagrees with the manifest | [The clip is the wrong length or freezes at the end](symptoms/wrong-length.md) |
| A loop pops or pulses at the wrap | [The loop pops](symptoms/loop-pops.md) |
| Double or missing root motion; the character glides or runs in place | [The character glides or runs in place](symptoms/character-glides.md) |
| Feet skate across a blend, or a mask, transition, or contact state breaks | [Feet skate when clips blend](symptoms/blend-skate.md) |
| A planted foot slides during stance inside one clip | [Feet slide within a clip](symptoms/feet-slide.md) |
| Missing or frozen bones, a T-posed limb, or a suspect bind | [A limb is T-posed, or a bone never moves](symptoms/limb-frozen.md) |
| A skeleton or retarget mismatch, or a pack whose clips all share one name | [Files disagree about skeleton or clip identity](symptoms/identity-mismatch.md) |
| Unexpected scale or rest/bind behavior, an attachment at the wrong size, or export bloat | [The file is bloated, or the retargeter chokes](symptoms/file-bloat.md) |
| Animations vanish in Bevy with no lint error | Answered here: no content finding exists, because the loader's `bevy_animation` feature or its `load_animations` setting dropped them — the [Bevy profile's revision-3 gate](engine-profile-bevy.md#revision-3-animationchannel-gate-support) predicts exactly that negative outcome. |

## Two symptoms that are not about a clip

Neither of these produces a content finding, so neither has a symptom page.
Both are answered by reading the run itself and then by the project that
consumes the asset.

**A loader error or an AnimSmith refusal.** Read stderr and the command's exit
code, then inspect `engine-track-support`, `engine-addressability`, input
identity, and coverage facets. Correct a bad path, config, or supported source
fact; do not reinterpret a refusal. Engine feature and import settings belong
to the engine project. The gate closes on a successful rerun with complete
applicable source evidence, plus engine-observed load evidence where the route
requires it — for Bevy, the revision-3 `lint` command in the
[intake workflow](game-developer-intake-workflow.md#worked-path-bevy-0190-profile-revision-3).

**A clip exists but cannot be addressed in-engine.** Inspect
`generate addressability` source rows, selector and index evidence, the
dependency closure, and the engine profile's coverage state. Regenerate an
application manifest after a source-order change; application name policy,
loaded handles, targets, and graph wiring are engine code. The gate closes when
the source inventory is complete and the target project records a resolved
runtime asset, selector, target, and playback.

The exact fields, severities, prerequisites, and configuration keys remain
authoritative in the linked references. If evidence identifies a vendor or
source delivery defect, return the source and its evidence; if it identifies
runtime or artistic uncertainty, keep the project-owned gate open.
