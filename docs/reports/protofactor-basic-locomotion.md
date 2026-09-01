# Animation pack evaluation: Protofactor Basic Locomotion Animset

> Technical verdict: **Usable with conditions**
>
> Evaluation completeness: **partial** — current mechanical and declared-contract evidence is exhaustive, but no current engine or visual acceptance ran.
>
> Confidence: **medium**
>
> Evaluation date: **2026-09-01**
>
> Current evaluator: **AnimSmith 0.10.0**
>
> Report format: **2**
>
> Detailed evidence: [Protofactor Basic Locomotion evidence](protofactor-basic-locomotion-evidence.md)

## Technical decision

The verified AnimSmith 0.10.0 release loads all 179 delivered FBXs. Untouched lint finds 24,186 `constant-track` notes and 36 `time-monotonic` errors across 12 files. Current declared contracts cover 177 individual motions; 58 command invocations pass and 119 fail per format, so loop and continuity results are conditions rather than approval. No engine, visual, retarget, contact, or gameplay run occurred.

## Capability coverage

### Complete core

- Delivered filename families cover locomotion, cover, turns, airborne actions, and transitions; 70 labelled root-motion files have matching in-place partners.

### Partial supporting gameplay

- Mechanical input health is measured, but contract failures require loop and continuity decisions before locomotion blend adoption.

### Absent

- No current evidence establishes additive, first-person, paired interaction, engine acceptance, or artistic readiness.

## Runtime sets and authored motion

No important runtime sets were identified.

## Integration recipe

1. **Members/topology:** `topology=not-evaluated`; declare selected locomotion rings rather than infer them from names.
2. **Timing/synchronization:** `sync=not-evaluated`; resolve contract failures and measure selected loop members.
3. **State ownership:** `owner=not-evaluated`; declare IP/RM movement ownership per clip.
4. **Composition constraints:** `composition=full-body`; do not approve masks or additive use.
5. **Acceptance gate:** `gate=engine-and-visual-review`; import, blend, retarget, and playtest selected candidates.

## Technical issue register

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| BL-010 | major | [Time ordering and loop continuity](../game-ready-clips.md#the-readiness-ladder) require declared review before runtime use. | artist-author | Repair or re-export malformed source and decide intended loop policy. | Only declared mechanical checks are current; no automatic motion repair is established. | `observed-animsmith`; 12 baseline time errors and 119 failing contract invocations per format. |

## Engine status

| Runtime | Evidence level | Technical result | Remaining gate |
|---|---|---|---|
| Unity unspecified | not-evaluated | No current import or playback run. | Import, controller, visual, and build tests. |
| Unreal Engine unspecified | not-evaluated | No current import or playback run. | Import, retarget, graph, and build tests. |
| Godot unspecified | not-evaluated | No current conversion, import, or playback run. | Conversion/import, graph, and export tests. |
| Bevy unspecified | not-evaluated | No current glTF handoff or runtime run. | Conversion, addressability, runtime, and performance tests. |

## Fit and limitations

Best fit is an engine project willing to declare and validate its locomotion contract. Cross-pack compatibility, visual loop quality, contacts, and artistic fit remain untested.

## Changes between AnimSmith versions

AnimSmith 0.10.0 — official release revalidated the 179-FBX baseline and 177 declared contracts. Earlier 0.7.0 evidence is retained historical evidence only.

## Evidence status

Current evidence uses the official 0.10.0 binary and the [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder). Commercial sources and derivatives remain external.

## Sources

- Protofactor, [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/) — collection-level product context.
- AnimSmith, [game-ready clips](../game-ready-clips.md) and [CLI reference](../cli.md) — readiness and command boundaries.
