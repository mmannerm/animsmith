# For artists: from export to handoff

You have exported a clip, or cleaned one up, and somebody downstream has to
trust it. This page is the order to do that in: keep the file you exported
from, check what you actually shipped, make only the changes a tool can prove,
and hand over the evidence with the asset instead of a verbal "it's fine".

It keeps the original immutable, makes mechanical changes reviewable, and
leaves artistic approval where it belongs. For the meaning of the stages,
start with [why AnimSmith](why-animsmith.md) and the
[readiness ladder](game-ready-clips.md#the-readiness-ladder). When something
already looks wrong in the engine, start from the
[symptoms](symptoms/README.md) instead and come back here to hand it off.

## Outcome and boundary

AnimSmith can inspect source facts, measure and compare motion, evaluate a
declared contract, safely repair two quaternion representations, and perform
bounded mechanical transforms. It can also produce an offline HTML report. It
does not decide whether a performance, retarget, contact, controller, or
visual result is acceptable in a game. A clean lint result is scoped evidence,
not a blanket “game-ready” verdict.

Keep these artifacts separate:

| Artifact | Owner | Rule |
|---|---|---|
| Raw delivery or DCC export | author, vendor, or capture team | Preserve immutably with provenance and export settings. |
| Candidate GLB/FBX | animation author | Regenerate or replace only after reviewing evidence. |
| `animsmith.toml` | project | Declare only facts the project authoritatively owns. |
| Measurements, lint JSON, diff, and HTML report | review/build pipeline | Attach them to the candidate they describe. |

The broader storage split and its ownership rationale are in the
[pipeline guide](pipeline-scenarios.md#scenario-raw-vs-transformed-artifact-storage).

## Work the candidate, not the source

The commands below are executable examples. `$ANIMSMITH`, `$ASSET_DIR`,
`$CONFIG_DIR`, and `$WORK_DIR` are placeholders supplied by your checkout or
automation: the repository test binds them to the current binary, checked-in
self-authored fixtures, committed configs, and a disposable output directory.
The first comment gives the command's expected exit status; an exit of `1` can
be expected evidence that a candidate materially differs.

1. **Inventory the untouched baseline.** Run `inspect` and `measure` before
   changing the file. Keep the JSON measurement output with the baseline.

   ```console
   # workflow-exit: 0
   $ANIMSMITH inspect "$ASSET_DIR/report-comparison-before.glb"
   $ANIMSMITH measure --format json "$ASSET_DIR/report-comparison-before.glb"
   ```

2. **Declare only project facts.** Add loop, movement-owner, speed, rig-role,
   required-bone, and set membership declarations only after the team agrees
   they are true. The [configuration reference](configuration-reference.md)
   owns every key and precedence rule; the [contract-config cookbook](../examples/README.md#4-a-project-contract-config)
   is the runnable starting shape.

3. **Lint and report the baseline.** Treat a finding as a precise work order:
   its check id, measured value, subject, and coverage state say what was and
   was not evaluated. The [built-in check reference](built-in-checks.md)
   owns the check contracts and the [machine-output reference](output.md#measure-and-lint)
   owns the JSON fields. Use the existing self-authored comparison pair when a
   reviewer needs a judged frame, skeleton, root/foot trails, or charts.

   ```console
   # workflow-exit: 0
   $ANIMSMITH --config "$CONFIG_DIR/report-comparison.animsmith.toml" report \
       "$ASSET_DIR/report-comparison-before.glb" \
       --compare-after "$ASSET_DIR/report-comparison-after.glb" \
       --before-clip acceptance-matrix --after-clip acceptance-matrix \
       -o "$WORK_DIR/author-comparison.html"
   ```

4. **Choose the right remedy.** `fix` can losslessly normalize finite
   quaternions and correct quaternion hemisphere continuity. `transform` can
   make its explicitly bounded slice, hold, gait-anchor, duplicate-endpoint,
   or constant-track operation. Use the [editing cookbook](../examples/README.md#3-editing-a-clip)
   for those operations. Retargeting, pose/contact cleanup, motion redesign,
   and an intentional scale change remain DCC work; then export a new
   candidate. Importer settings and controller behavior remain engine/project
   work. A source delivery defect or missing contractual clip goes back to the
   vendor/source owner.

5. **Diff the candidate against the accepted baseline.** A binary rewrite is
   not review evidence. Compare the measured effects and hand off the output
   with the candidate.

   ```console
   # workflow-exit: 1
   $ANIMSMITH diff "$ASSET_DIR/report-comparison-before.glb" \
       "$ASSET_DIR/report-comparison-after.glb"
   ```

6. **Hand off exact evidence.** Include input identity, committed config,
   AnimSmith version, command, lint JSON (including coverage gaps), diff, and
   any generated report. State the remaining engine, gameplay, and visual
   gates instead of converting unknowns into a pass.

## When to stop and route

| Evidence says | Next owner |
|---|---|
| A lossless quaternion repair or documented mechanical transform resolves the finding and the diff is understood. | AnimSmith workflow; re-lint and submit the candidate. |
| Pose, contacts, timing, retargeting, rig mapping, or authored scale must change. | DCC/animation author; export a new candidate. |
| The asset is mechanically sound but importer settings, masks, graph wiring, root-motion application, or target lookup fails. | Engine/project integrator; follow the [game-developer workflow](game-developer-intake-workflow.md). |
| The delivered source cannot satisfy the agreed contract or is incomplete. | Vendor/source owner. |
| Runtime feel, readability, or style remains disputed after the technical evidence is clean. | Gameplay/art direction; perform project-owned visual acceptance. |

## Next step

Use [animation troubleshooting](animation-troubleshooting.md) for a symptom
first route, or hand the candidate to the
[game-developer intake workflow](game-developer-intake-workflow.md) for the
engine-observed gates.
