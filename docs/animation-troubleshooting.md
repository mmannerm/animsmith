# Animation troubleshooting

Start with the symptom, retain the source evidence, and route the work to the
owner who can change it. This page deliberately links to the
[configuration reference](configuration-reference.md) and
[built-in check reference](built-in-checks.md) rather than restating their
contracts. The [game-ready guide](game-ready-clips.md#from-symptom-to-command)
has the broader symptom-to-command matrix.

For every row, capture the current `lint --format json` result and, when motion
is contested, the offline `report` or before/after `diff`. A current,
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

| Symptom | Inspect and current diagnostic example | Safe remediation vs owner | Gate-closing evidence |
|---|---|---|---|
| Loader error or an AnimSmith refusal | Read stderr and the command's exit code; inspect `engine-track-support`, `engine-addressability`, input identity, and coverage facets. For Bevy, run the revision-3 `lint` command in the [intake workflow](game-developer-intake-workflow.md#worked-path-bevy-0190-profile-revision-3). | Correct a bad path/config or supported source fact; do not reinterpret a refusal. Engine feature/import settings belong to the engine project. | A successful rerun with complete applicable source evidence, plus engine-observed load evidence where required. |
| Unexpected scale or rest/bind behavior | Inspect `rest-world-scale`, `scale-keys`, `non-uniform-scale`, selected runtime-node ancestry, and the report's skeleton/trajectory context. | Supported `scale` operations require their proof boundary; unintended hierarchy or animated scale is DCC/export work. Imported hierarchy, physics, and attachments are engine work. | Source recheck plus target-engine scale, attachment, and visual observation. |
| A loop pops | Inspect `loop-closure`, `loop-seam`, `loop-seam-vel`, `loop-seam-rot`, and the compared endpoint frame in `comparison.html`. | Only a strict redundant endpoint may be mechanically removed; pose/tangent/contact repair is DCC work. | Matching declared closure/seam evidence and observed loop playback in the target graph. |
| Feet slide | Inspect `foot-slide`, `gait-group`, resolved foot roles, stance intervals, and the report's root/foot trails. | AnimSmith can report and narrowly re-anchor an eligible in-place gait; contact cleanup and blend timing are DCC/runtime work. | Re-lint under the declared contract and observe stance/contact behavior in the actual blend. |
| Double or missing root motion | Inspect declared `movement_owner_*`, `in-place`, `root-motion-speed`, and applicable engine prediction. | No automatic movement-policy repair exists. Decide whether animation or gameplay owns each component; configure importer/controller code accordingly. | Source declaration and measurements agree, and an engine trial proves one—not zero or two—movement producers per component. |
| Missing or frozen bones | Inspect `missing-bones`, `required-bones`, `frozen-bone`, `bind-pose`, clip `animates_bones`, and resolved rig roles. | Re-export or repair the source rig/animation in the DCC; target binding and masks are engine work. | A re-export meets the structural contract and plays with the required moving bones in-engine. |
| Skeleton or retarget mismatch | Inspect `inspect` skeleton inventory, rig-role resolution, required-bone declarations, bind-pose evidence, and exact source/clip identity. | Retarget maps, Avatar/Skeleton setup, and target-character deformation are DCC/engine responsibilities; AnimSmith does not retarget automatically. | Recorded source-to-target mapping and target-character playback/visual evidence. |
| Mask or contact breaks | Inspect runtime-set membership, `gait-group`/`sync-group`/`foot-slide` evidence, relevant report trails, and the project mask/graph. | Mask topology, contact events, graph timing, and interaction logic are project work; re-author source contacts in the DCC when needed. | A project playback capture covers the masked/transition state and contacts with the intended target character. |
| A clip exists but cannot be addressed in-engine | Inspect `generate addressability` source rows, selector/index evidence, dependency closure, and the engine profile's coverage state. | Regenerate an application manifest after a source-order change; application name policy, loaded handles, targets, and graph wiring are engine code. | Source inventory is complete and the target project records a resolved runtime asset, selector, target, and playback. |

The exact fields, severities, prerequisites, and configuration keys remain
authoritative in the linked references. If evidence identifies a vendor/source
delivery defect, return the source and its evidence; if it identifies runtime
or artistic uncertainty, keep the project-owned gate open.
