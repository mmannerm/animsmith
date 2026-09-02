# Symptoms

Start from what you see in the engine, not from a check id. Each page below
opens with the runtime symptom, shows what AnimSmith measures on a synthetic
clip before and after the fix, and ends with the command and the config that
address it. The precise contract behind each check stays on the page, one
click down, and the [built-in check reference](../built-in-checks.md) and the
[configuration reference](../configuration-reference.md) remain the exact
authorities.

| Document | Use it to… |
|---|---|
| [The pose flickers, spins, or explodes](pose-flickers.md) | Repair the rotation representation itself: `nan`, `quat-norm`, `quat-flip`, `time-monotonic`. |
| [The clip is the wrong length or freezes at the end](wrong-length.md) | Find the channel that stopped early or the range that drifted: `duration-sanity`, `fps`. |
| [The loop pops](loop-pops.md) | Fix a looping clip that jumps or hitches at the wrap: `loop-closure`, `loop-seam-vel`, `loop-seam-rot`, `loop-seam`, `duplicate-loop-endpoint`. |
| [The character glides or runs in place](character-glides.md) | Settle who owns horizontal movement and hold the clip to it: `in-place`, `root-motion-speed`. |
| [Feet skate when clips blend](blend-skate.md) | Hold a directional set to one stride phase and one timing surface: `gait-group`, `sync-group`, `time-complement`. |
| [Feet slide within a clip](feet-slide.md) | Fix a planted foot that skates during stance: `foot-slide`. |
| [A limb is T-posed, or a bone never moves](limb-frozen.md) | Separate an absent bone from a frozen one and from a wrong bind: `missing-bones`, `required-bones`, `frozen-bone`, `bind-pose`. |
| [Files disagree about skeleton or clip identity](identity-mismatch.md) | Keep `(file, clip)` identity across a pack instead of trusting a display name. |
| [The file is bloated, or the retargeter chokes](file-bloat.md) | Decide what is redundant data and what is authored scale: `constant-track`, `scale-keys`, `non-uniform-scale`, `rest-world-scale`. |

## From symptom to command

| Symptom | Check(s) | Repair / transform | Config surface | Who fixes it |
|---|---|---|---|---|
| [The pose flickers, spins, or explodes](pose-flickers.md) | `nan`, `quat-norm`, `quat-flip`, `time-monotonic` | `fix` (quat repairs, lossless) | `[checks.<id>] severity` | AnimSmith repairs the representation; the DCC owns non-finite data |
| [The clip is the wrong length or freezes at the end](wrong-length.md) | `duration-sanity`, `fps` | `transform --slice`, `--hold-extend` | `[clips.<name>] duration_s`, `fps` | Pipeline edit or re-export |
| [The loop pops or pulses at the wrap](loop-pops.md) | `duplicate-loop-endpoint`, `loop-closure`, `loop-seam-vel`, `loop-seam-rot`, `loop-seam` | `transform --drop-duplicate-loop-endpoint` for a strict duplicated endpoint; otherwise re-author the endpoint pose or tangents | `[clips.<name>] loop = true`, `[checks.loop-closure]`, `[checks.loop-seam-vel]`, `[checks.loop-seam-rot]` | Artist in the DCC; loop policy is a project decision |
| [The character glides or runs in place](character-glides.md) | `in-place`, `root-motion-speed` | re-export; `measure` for ground truth | `[clips.<name>] movement_owner_xz`, `speed_mps` | Gameplay decides ownership; artist re-exports |
| [Feet skate when clips blend](blend-skate.md) | `gait-group`, `sync-group`, `time-complement` | `transform --gait-anchor` for explicitly in-place cycles; runtime phase offsets for root motion | `[gait_groups.<name>]`, `[sync_groups.<name>]` | Technical animator declares the ring |
| [Feet slide within a clip](feet-slide.md) | `foot-slide` | re-author in the DCC | `[clips.<name>] speed_mps` | Artist; contact cleanup is DCC work |
| [A limb is T-posed, or a bone never moves](limb-frozen.md) | `missing-bones`, `required-bones`, `frozen-bone`, `bind-pose` | re-export | `[clips.<name>] animates_bones`, `[rig] required_bones` | Artist repairs the source rig |
| [Files disagree about skeleton or clip identity](identity-mismatch.md) | no per-file check; `inspect` plus a collection manifest retain identity | none — retain the exact `(file, clip)` binding | collection manifest sources, clips, and runtime sets | Pack owner and the ingesting pipeline |
| [The file is bloated, or the retargeter chokes](file-bloat.md) | `constant-track`, `scale-keys`, `non-uniform-scale`, `rest-world-scale`, opt-in `constant-nonunit-scale` | `transform --prune-constant-tracks` after reviewing transition coverage | `[checks.<id>] severity`, `[runtime_nodes] selectors`, `[clips.<name>] animates_bones` | Artist or exporter settings |

Where the repair column says *re-export*, that is deliberate: AnimSmith
rewrites a clip only in ways whose within-clip correctness its own checks can
verify.

Two runtime problems have no symptom page, because their evidence is not a
finding about a clip: a loader error or an AnimSmith refusal, and a clip that
exists but cannot be addressed in the engine. Both are routed from
[animation troubleshooting](../animation-troubleshooting.md).

The [readiness ladder](../game-ready-clips.md#the-readiness-ladder) stages what
a clean run on any of these pages is evidence *for*, and its
[complete symptom table](../game-ready-clips.md#from-symptom-to-command) carries
every registered check id, including the ones no single symptom owns.
