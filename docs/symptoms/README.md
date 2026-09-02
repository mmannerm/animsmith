# Symptoms

Start from what you see in the engine, not from a check id. One row per
symptom: what it looks like, the checks that measure it, the repair, the
config surface, who owns the fix, and the page that walks it. Each page opens
with the runtime symptom, shows what AnimSmith measured on a synthetic clip,
and keeps the precise contract one click down; the
[built-in check reference](../built-in-checks.md) and the
[configuration reference](../configuration-reference.md) remain the exact
authorities.

| Symptom | Check(s) | Repair / transform | Config surface | Who fixes it | Page |
|---|---|---|---|---|---|
| A joint snaps the long way round, or the pose turns to noise | `nan`, `quat-norm`, `quat-flip`, `time-monotonic` | `fix` (quat repairs, lossless) | `[checks.<id>] severity` | AnimSmith repairs the representation; the DCC owns non-finite data | [The pose flickers, spins, or explodes](pose-flickers.md) |
| A limb stops before the rest of the body, or the clip is a frame short | `duration-sanity`, `fps` | `transform --slice`, `--hold-extend` | `[clips.<name>] duration_s`, `fps` | Pipeline edit or re-export | [The clip is the wrong length or freezes at the end](wrong-length.md) |
| The cycle jumps or hitches every time it wraps | `duplicate-loop-endpoint`, `loop-closure`, `loop-seam-vel`, `loop-seam-rot`, `loop-seam` | `transform --drop-duplicate-loop-endpoint` for a strict duplicated endpoint; otherwise re-author the endpoint pose or tangents | `[clips.<name>] loop = true`, `[checks.loop-closure]`, `[checks.loop-seam-vel]`, `[checks.loop-seam-rot]` | Artist in the DCC; loop policy is a project decision | [The loop pops](loop-pops.md) |
| The character slides across the floor, or runs without moving | `in-place`, `root-motion-speed` | re-export; `measure` for ground truth | `[clips.<name>] movement_owner_xz`, `speed_mps` | Gameplay decides ownership; artist re-exports | [The character glides or runs in place](character-glides.md) |
| Feet skate, pop or desync when the runtime blends two clips | `gait-group`, `sync-group`, `time-complement` | `transform --gait-anchor` for explicitly in-place cycles; runtime phase offsets for root motion | `[gait_groups.<name>]`, `[sync_groups.<name>]` | Technical animator declares the ring | [Feet skate when clips blend](blend-skate.md) |
| A planted foot drifts during stance inside one clip | `foot-slide` | re-author in the DCC | `[clips.<name>] speed_mps` | Artist; contact cleanup is DCC work | [Feet slide within a clip](feet-slide.md) |
| An arm hangs in a T-pose, or a bone never moves | `missing-bones`, `required-bones`, `frozen-bone`, `bind-pose` | re-export | `[clips.<name>] animates_bones`, `[rig] required_bones` | Artist repairs the source rig | [A limb is T-posed, or a bone never moves](limb-frozen.md) |
| Two files disagree about the skeleton, or a pack reuses one clip name | no per-file check; `animsmith inspect` plus a collection manifest retain identity | none — retain the exact `(file, clip)` binding | collection manifest sources, clips, and runtime sets | Pack owner and the ingesting pipeline | [Files disagree about skeleton or clip identity](identity-mismatch.md) |
| An attachment, socket or helper imports at the wrong size | `rest-world-scale` | apply or rebake the unintended source hierarchy scale, then re-export | `[runtime_nodes] selectors`, `[checks.rest-world-scale] expected_uniform_scale` | Artist or exporter settings | [The file is bloated, or the retargeter chokes](file-bloat.md#attachment-nodes-and-inherited-rest-world-scale) |
| Clips cost far more than the motion in them, or a retargeter chokes | `constant-track`, `scale-keys`, `non-uniform-scale`, opt-in `constant-nonunit-scale` | `transform --prune-constant-tracks` after reviewing transition coverage | `[checks.<id>] severity`, `[clips.<name>] animates_bones` | Artist or exporter settings | [The file is bloated, or the retargeter chokes](file-bloat.md) |

Where the repair column says *re-export*, that is deliberate: AnimSmith
rewrites a clip only in ways whose within-clip correctness its own checks can
verify.

The table above carries every registered check id, including the narrower
presentations of a symptom that no page of its own owns. The
[readiness ladder](../game-ready-clips.md#the-readiness-ladder) stages what a
clean run on any of these pages is evidence *for*.

Whichever row you follow, capture the current `lint --format json` result, and
when the motion itself is contested, the offline `report` or a before/after
`diff`. A current, repository-safe diagnostic pair is:

```console
# workflow-exit: 0
$ANIMSMITH --config "$CONFIG_DIR/report-comparison.animsmith.toml" report \
    "$ASSET_DIR/report-comparison-before.glb" \
    --compare-after "$ASSET_DIR/report-comparison-after.glb" \
    --before-clip acceptance-matrix --after-clip acceptance-matrix \
    -o "$WORK_DIR/symptom-comparison.html"
```

It shows a deliberately defective loop and stance trail; it is diagnostic
evidence, not an engine or artistic acceptance result.

## Not about the clip

Three runtime problems produce no content finding, so none of them has a row
above. Each is answered by reading the run itself and then by the project that
consumes the asset.

**A loader error or an AnimSmith refusal.** Read stderr and the command's exit
code, then inspect `engine-track-support`, `engine-addressability`, input
identity, and coverage facets. Correct a bad path, config, or supported source
fact; do not reinterpret a refusal. Engine feature and import settings belong
to the engine project. The gate closes on a successful rerun with complete
applicable source evidence, plus engine-observed load evidence where the route
requires it — for Bevy, the revision-3 `lint` command in the
[intake workflow](../game-developer-intake-workflow.md#worked-path-bevy-0190-profile-revision-3).

**A clip exists but cannot be addressed in-engine.** Inspect
`generate addressability` source rows, selector and index evidence, the
dependency closure, and the engine profile's coverage state. Regenerate an
application manifest after a source-order change; application name policy,
loaded handles, targets, and graph wiring are engine code. The gate closes when
the source inventory is complete and the target project records a resolved
runtime asset, selector, target, and playback.

**Animations vanish in Bevy with no lint error.** No content finding exists,
because the loader's `bevy_animation` feature or its `load_animations` setting
dropped them; the
[Bevy profile's revision-3 gate](../engine-profile-bevy.md#revision-3-animationchannel-gate-support)
predicts exactly that negative outcome. The gate closes in the Bevy project
that loads the asset, not in a lint run.

The exact fields, severities, prerequisites, and configuration keys stay
authoritative in the [built-in check reference](../built-in-checks.md) and the
[configuration reference](../configuration-reference.md). If evidence
identifies a vendor or source delivery defect, return the source and its
evidence; if it identifies runtime or artistic uncertainty, keep the
project-owned gate open.
