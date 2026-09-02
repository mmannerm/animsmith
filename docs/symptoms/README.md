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

Two runtime problems have no symptom page, because their evidence is not a
finding about a clip: a loader error or an AnimSmith refusal, and a clip that
exists but cannot be addressed in the engine. Both are routed from
[animation troubleshooting](../animation-troubleshooting.md).

The table above carries every registered check id. The
[readiness ladder](../game-ready-clips.md#the-readiness-ladder) stages what a
clean run on any of these pages is evidence *for*, and the guide's
[symptom table](../game-ready-clips.md#from-symptom-to-command) also routes the
narrower presentations of a symptom that no page of its own owns.
