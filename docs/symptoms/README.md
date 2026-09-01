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
| [The loop pops](loop-pops.md) | Fix a looping clip that jumps or hitches at the wrap: `loop-closure`, `loop-seam-vel`, `loop-seam-rot`, `loop-seam`, `duplicate-loop-endpoint`. |
| [Feet slide within a clip](feet-slide.md) | Fix a planted foot that skates during stance: `foot-slide`. |

## From symptom to command

| Symptom | Check(s) | Repair / transform | Config surface | Who fixes it |
|---|---|---|---|---|
| Pose flickers, spins, or explodes | `nan`, `quat-norm`, `quat-flip`, `time-monotonic` | `fix` (quat repairs, lossless) | — | AnimSmith repairs the representation; the DCC owns non-finite data |
| Wrong length, freezes at the end | `duration-sanity`, `fps` | `transform --slice`, `--hold-extend` | `[clips.<name>] duration_s`, `fps` | Pipeline edit or re-export |
| [The loop pops or pulses at the wrap](loop-pops.md) | `duplicate-loop-endpoint`, `loop-closure`, `loop-seam-vel`, `loop-seam-rot`, `loop-seam` | `transform --drop-duplicate-loop-endpoint` for a strict duplicated endpoint; otherwise re-author the endpoint pose or tangents | `[clips.<name>] loop = true`, `[checks.loop-closure]`, `[checks.loop-seam-vel]`, `[checks.loop-seam-rot]` | Artist in the DCC; loop policy is a project decision |
| Glides or runs in place | `in-place`, `root-motion-speed` | re-export; `measure` for ground truth | `[clips.<name>] movement_owner_xz`, `speed_mps` | Gameplay decides ownership; artist re-exports |
| Feet skate across blends | `gait-group`, `sync-group`, `time-complement` | `transform --gait-anchor` for explicitly in-place cycles; runtime phase offsets for root motion | `[gait_groups.<name>]`, `[sync_groups.<name>]` | Technical animator declares the ring |
| [Feet slide within a clip](feet-slide.md) | `foot-slide` | re-author in the DCC | `[clips.<name>] speed_mps` | Artist; contact cleanup is DCC work |
| T-posed limb, static bone, wrong bind | `missing-bones`, `required-bones`, `frozen-bone`, `bind-pose` | re-export | `[clips.<name>] animates_bones`, `[rig] required_bones` | Artist repairs the source rig |
| Bloat, retargeter breakage | `constant-track`, `scale-keys`, `non-uniform-scale`, `rest-world-scale` | `transform --prune-constant-tracks` after reviewing transition coverage | `[checks.<id>] severity`, `[runtime_nodes] selectors` | Artist or exporter settings |

The remaining rows keep their full treatment in the
[game-ready clips guide](../game-ready-clips.md), whose
[symptom table](../game-ready-clips.md#from-symptom-to-command) stays the
complete one until the missing pages exist.
Where the repair column says *re-export*, that is deliberate: AnimSmith
rewrites a clip only in ways whose within-clip correctness its own checks can
verify.
