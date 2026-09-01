# Animation pack evaluation: Protofactor Ultimate Animation Collection (partial: eight packs)

> Technical verdict: **Insufficient technical evidence**
>
> Evaluation completeness: **partial** — eight constituent source baselines and declared contracts were rerun with AnimSmith 0.10.0; no current collection binding, semantic classification, runtime-set, cross-pack, engine, or visual acceptance run exists.
>
> Confidence: **medium**
>
> Evaluation date: **2026-09-01**
>
> Current evaluator: **AnimSmith 0.10.0**
>
> Report format: **2**
>
> Detailed evidence: [partial collection evidence appendix](protofactor-ultimate-animation-collection-evidence.md)

## Technical decision

The official AnimSmith 0.10.0 release reran [Basic Locomotion](protofactor-basic-locomotion.md), [Sword & Shield](protofactor-sword-and-shield.md), [Campfire](protofactor-campfire.md), [Climbing](protofactor-climbing.md), [Injured](protofactor-injured.md), [1-Handed Melee](protofactor-one-handed-melee.md), [2-Handed Melee](protofactor-two-handed-melee.md), and [Dual Swords](protofactor-dual-swords.md). They contain 918 source candidates, including 895 individual motion-labelled inputs. This is current mechanical intake evidence, not collection-level approval.

No current collection binding was rendered, so this rollup has no accepted canonical logical-motion inventory, semantic roles, runtime sets, cross-pack handoffs, or package-wide remediation result. The eight constituents produced 159 external transform candidates, but none was promoted or accepted for an engine. A project must make its own clip-selection, root-authority, contact, controller, and target-character decisions.

## Capability coverage

### Complete core

- Current source-inventory, baseline, and declared-contract evidence exists for each of the eight named constituents.
- The current report establishes the mechanical scope boundary: 918 source candidates and 895 individual motion-labelled inputs.

### Partial supporting gameplay

- All eight constituents have bounded external AnimSmith transform candidates; their runtime selection, visual result, and gameplay suitability remain untested.

### Absent

- No current canonical role classification, runtime-set inventory, cross-pack blend/mask/transition evidence, target rig, engine run, or visual acceptance.
- Fifteen collection constituents are outside this partial evaluation: 2-Handed Gun, Assault Rifle, Bazooka, Bow & Arrow, Combat Bare Fists, Creature, Crowd, Double Guns, Fencing, Hostage, Minigun, Push & Pull Cube, Shotgun, Wizard, and Zombie.

## Runtime sets and authored motion

No important runtime sets were identified.

## Integration recipe

1. **Members/topology:** `topology=not-evaluated`; preserve constituent boundaries and declare selected clips before creating collection graphs.
2. **Timing/synchronization:** `sync=not-evaluated`; resolve declared-contract findings and measure only project-selected transition or blend candidates.
3. **State ownership:** `owner=not-evaluated`; assign one root/displacement authority per selected clip and state.
4. **Composition constraints:** `composition=full-body`; do not approve masks, additive use, or cross-pack handoffs from this rollup.
5. **Acceptance gate:** `gate=current-cross-pack-engine-and-visual-review`; rerun selected compatibility, controller, contact, retarget, build, and visual tests.

## Technical issue register

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| UC-010 | blocker | No current collection binding, role taxonomy, or runtime-set inventory exists, so the corpus cannot safely define collection graphs. [Readiness guidance](../game-ready-clips.md#the-readiness-ladder) applies. | unknown | Select clips and declare collection roles, sets, and ownership in the target project. | A current collection manifest could preserve declared grouping evidence. | `not-evaluated`; 895 motion-labelled inputs remain unclassified at collection level. |
| UC-011 | major | Current declared-contract findings require clip-by-clip loop and continuity decisions before blend or transition use. Guidance: not applicable. | artist-author | Repair or re-export malformed source where required; otherwise document intended policy. | Declared mechanical diagnostics can support review, not infer intent. | `observed-animsmith`; constituent-specific results are linked above. |
| UC-012 | major | Cross-pack co-installation and engine behavior have no current validation. Guidance: not applicable. | engine-config | Run only project-relevant compatibility and engine tests using selected clips. | Cross-pack diagnostics could make the comparison reproducible. | `not-evaluated`; no current engine run. |
| UC-013 | major | Fifteen constituents are excluded, so this partial rollup cannot support a collection-wide coverage, compatibility, or value conclusion. Guidance: not applicable. | unknown | Evaluate additional constituents in bounded waves with a defined game decision. | Tooling cannot establish evidence for excluded content. | `not-evaluated`; explicit scope boundary. |

## Engine status

| Runtime | Evidence level | Technical result | Remaining gate |
|---|---|---|---|
| Unity | not-evaluated | No current import or playback run. | Co-import, controller, visual, root-motion, contacts, retarget, compression, and build tests. |
| Unreal Engine | not-evaluated | No current import or playback run. | Import, retarget, graph, contact, and build tests. |
| Godot | not-evaluated | No current conversion, import, or playback run. | Conversion/import, graph, contact, and export tests. |
| Bevy | not-evaluated | No current conversion, addressability, or runtime run. | Conversion, target mapping, runtime, and performance tests. |

## Fit and limitations

This report is suitable only as a mechanically scoped intake record for a project prepared to select clips and run its own current compatibility and engine tests. It does not support a blanket game-ready, engine-ready, retarget-ready, or artistic-ready conclusion.

It is a poor basis for approving a universal locomotion graph, seamless cross-pack transitions, masks, contact actions, first-person, motion matching, networking, or the fifteen excluded constituents without new scoped evidence.

## Changes between AnimSmith versions

AnimSmith 0.10.0 — official release evidence revalidated all eight constituent source baselines, declared contracts, and 159 bounded constituent remediation candidates. The current rollup intentionally records only constituent-derived counts and findings; collection-level taxonomy, runtime sets, and compatibility were not regenerated.

AnimSmith 0.7.0 — historical collection output recorded 582 logical motions, 90 runtime-set records, 14 cross-pack candidates, and 159 external candidates. Those superseded collection-derived claims are retained as historical context only and are not current evidence.

AnimSmith 0.4.0 — retained dated Unity 6000.5.8f1 graph probes recorded 22/22 contextual and 33/33 melee required checks with four expected Generic-rig failures; a shared-path comparison recorded 700 byte-identical comparisons across 28 constituent pairs. Neither result was rerun and neither is current engine or compatibility evidence.

## Evidence status

Current evidence is the official 0.10.0 constituent rerun: 918 source candidates, 895 individual motion-labelled inputs, 3,672 baseline commands, and 159 external transform candidates across all eight constituents. Current canonical logical-motion and runtime-set totals are both zero because no collection classification or binding was accepted. The [readiness ladder](../game-ready-clips.md#the-readiness-ladder) and [appendix](protofactor-ultimate-animation-collection-evidence.md) define the exact boundary.

## Sources

- Constituent reports: [Basic Locomotion](protofactor-basic-locomotion.md), [Sword & Shield](protofactor-sword-and-shield.md), [Campfire](protofactor-campfire.md), [Climbing](protofactor-climbing.md), [Injured](protofactor-injured.md), [1-Handed Melee](protofactor-one-handed-melee.md), [2-Handed Melee](protofactor-two-handed-melee.md), and [Dual Swords](protofactor-dual-swords.md).
- Protofactor, [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/) and [End User License Agreement](https://protofactor.biz/end-user-license-agreement/).
- AnimSmith, [game-ready clips](../game-ready-clips.md) and [CLI reference](../cli.md).
