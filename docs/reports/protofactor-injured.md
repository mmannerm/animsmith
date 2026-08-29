# Animation pack evaluation: Protofactor Injured

> Technical verdict: **Usable with conditions**
>
> Evaluation completeness: **partial** — AnimSmith 0.7.0 reevaluated all 72 FBXs, 70 contracts, remediation, addressability, and bounded advice; dated Unity evidence exists, but blends, loops, transitions, masks, target-character results, and candidates lack visual acceptance.
>
> Confidence: **high**
>
> Evaluation date: **2026-08-26**
>
> Current evaluator: **AnimSmith 0.7.0**
>
> Report format: **2**
>
> Detailed evidence: [Protofactor Injured evidence](protofactor-injured-evidence.md)

## Technical decision

Use the pack as seven coherent full-body injury styles, each with idle, walk, run, sit, kneel, and transition material. Do not pool all A–G clips into one interchangeable blend space: root-motion walk speed differs by 48%, run speed by 22%, and the walk/run gait-phase relationship varies materially by style. All standard files share the collection's 56-bone skeleton, all 70 individual clips import as Unity Humanoid, and each IP/RM gait pair has matching duration and near-identical phase.

The main out-of-box defect is looping: 42 of 70 declared contracts fail, including all 27 explicitly loop-declared locomotion files and 15 of 21 injury idles. AnimSmith 0.7.0 reports 48 loop-seam-applicable and 22 not-applicable clips, with 25 complete and 45 not evaluated under the declared contracts. It measures a vertical `positive_y` heading on 71/72 clips and anchors all 14 selected in-place gaits, reducing cross-style circular phase spread from 0.554 to 0.110 for run and 0.603 to 0.051 for walk. This is a mechanical candidate only: no Humanoid retarget or visual import ran, so the outputs remain unpromoted.

## Capability coverage

### Complete core

- Seven named injury styles each contain standing idle, paired IP/RM walk and run, sitting idle, kneeling idle, and entry/return motions.
- Fourteen IP/RM gait pairs have matched durations and gait phase; all individual files use the standard collection skeleton.

### Partial supporting gameplay

- Sitting has enter and return clips for all styles; kneeling has entry clips but no matching kneel-to-stand exits.
- A Basic-locomotion lower body plus Injured-idle upper-body mask graph executes headlessly, but the visual result and lost leg/pelvis injury cues were not accepted.

### Absent

- No turns, strafes, backwards locomotion, starts/stops, jumps, weapon-specific injuries, additive poses, hits, deaths, events, contacts, or network-movement contract.
- No native Unreal, Godot, or Bevy delivery and no tested target-character retarget.

## Runtime sets and authored motion

| Set/profile | Role or coordinate | Exact members | Variant/type | Timing or motion | Runtime contract |
|---|---|---|---|---|---|
| Style A locomotion | walk | IP `Humanoid@WalkInjuredA.fbx`; RM `Humanoid@WalkInjuredA_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.540 m/s | loop_ip=true; loop_rm=true; sync=phase; movement=animation |
| Style A locomotion | run | IP `Humanoid@RunInjuredA.fbx`; RM `Humanoid@RunInjuredA_RM.fbx` | variant=paired-ip-rm | duration=0.800 s; rm_speed=2.021 m/s | loop_ip=true; loop_rm=true; sync=phase; movement=animation |
| Style B locomotion | walk | IP `Humanoid@WalkInjuredB.fbx`; RM `Humanoid@WalkInjuredB_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.541 m/s | loop_ip=true; loop_rm=true; sync=phase; movement=animation |
| Style B locomotion | run | IP `Humanoid@RunInjuredB.fbx`; RM `Humanoid@RunInjuredB_RM.fbx` | variant=paired-ip-rm | duration=0.800 s; rm_speed=2.060 m/s | loop_ip=true; loop_rm=true; sync=phase; movement=animation |
| Style C locomotion | walk | IP `Humanoid@WalkInjuredC.fbx`; RM `Humanoid@WalkInjuredC_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.494 m/s | loop_ip=true; loop_rm=true; sync=phase; movement=animation |
| Style C locomotion | run | IP `Humanoid@RunInjuredC.fbx`; RM `Humanoid@RunInjuredC_RM.fbx` | variant=paired-ip-rm | duration=0.700 s; rm_speed=1.748 m/s | loop_ip=true; loop_rm=true; sync=phase; movement=animation |
| Style D locomotion | walk | IP `Humanoid@WalkInjuredD.fbx`; RM `Humanoid@WalkInjuredD_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.504 m/s | loop_ip=true; loop_rm=true; sync=phase; movement=animation |
| Style D locomotion | run | IP `Humanoid@RunInjuredD.fbx`; RM `Humanoid@RunInjuredD_RM.fbx` | variant=paired-ip-rm | duration=0.700 s; rm_speed=1.821 m/s | loop_ip=true; loop_rm=true; sync=phase; movement=animation |
| Style E locomotion | walk | IP `Humanoid@WalkInjuredE.fbx`; RM `Humanoid@WalkInjuredE_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.733 m/s | loop_ip=true; loop_rm=true; sync=phase; movement=animation |
| Style E locomotion | run | IP `Humanoid@RunInjuredE.fbx`; RM `Humanoid@RunInjuredE_RM.fbx` | variant=paired-ip-rm | duration=0.800 s; rm_speed=2.005 m/s | loop_ip=true; loop_rm=true; sync=phase; movement=animation |
| Style F locomotion | walk | IP `Humanoid@WalkInjuredF.fbx`; RM `Humanoid@WalkInjuredF_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.692 m/s | loop_ip=true; loop_rm=true; sync=phase; movement=animation |
| Style F locomotion | run | IP `Humanoid@RunInjuredF.fbx`; RM `Humanoid@RunInjuredF_RM.fbx` | variant=paired-ip-rm | duration=0.800 s; rm_speed=2.005 m/s | loop_ip=true; loop_rm=true; sync=phase; movement=animation |
| Style G locomotion | walk | IP `Humanoid@WalkInjuredG.fbx`; RM `Humanoid@WalkInjuredG_RM.fbx` | variant=paired-ip-rm | duration=1.333 s; rm_speed=0.519 m/s | loop_ip=true; loop_rm=true; sync=phase; movement=animation |
| Style G locomotion | run | IP `Humanoid@RunInjuredG.fbx`; RM `Humanoid@RunInjuredG_RM.fbx` | variant=paired-ip-rm | duration=0.800 s; rm_speed=2.127 m/s | loop_ip=true; loop_rm=true; sync=phase; movement=animation |

These loop values are the intended runtime policy, not proof that the delivered seams are acceptable. `Humanoid@RunInjuredB.fbx` lacks an explicit Unity clip definition; its loop policy is inferred from its paired gait family and must be reviewed. Posture chains are inventoried in the appendix.

## Integration recipe

1. **Members/topology:** `topology=seven-style-speed-blends`; keep A–G as distinct injury states and choose either IP or RM authority for each gait pair.
2. **Timing/synchronization:** `sync=gait-phase`; use per-style measured speeds, add engine sync/contact markers, and visually accept every loop and walk/run blend; see [blend guidance](../game-ready-clips.md#feet-skate-when-clips-blend).
3. **State ownership:** `owner=movement-state-machine`; gameplay owns injury style, speed, posture, movement authority, and recovery/cancellation.
4. **Composition constraints:** `composition=full-body-default`; test torso-only masks deliberately because lower-body injury posture and locomotion character will otherwise be lost.
5. **Acceptance gate:** `gate=style-speed-posture-matrix`; test seven styles at idle/walk/run, IP/RM parity, loop wraps, sit/kneel transitions, masks, target rigs, and gameplay speed thresholds.

## Technical issue register

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| IN-001 | major | All 27 explicit locomotion loops fail a strict seam check, risking a once-per-cycle pose or velocity pulse; [loop guidance](../game-ready-clips.md#the-loop-pops). | artist-author | Inspect each wrap and re-author/crossfade only with visual acceptance. | Current diagnostics identify seams; safe repair needs authored acceptance. | High mechanical; visual severity open. |
| IN-002 | major | Walk speeds span 0.494–0.733 m/s and runs 1.748–2.127 m/s; uncalibrated shared thresholds cause speed jumps or foot sliding; [blend guidance](../game-ready-clips.md#feet-skate-when-clips-blend). | engine-config | Build one speed blend per style using measured thresholds. | Current declared-set policies can check a supplied controller contract; this evaluation did not supply one. | High measurements; blends visually open. |
| IN-003 | major | Walk-to-run phase offsets reach 0.181 cycles; direct blending can double-contact or scuff even though each IP/RM pair matches; [blend guidance](../game-ready-clips.md#feet-skate-when-clips-blend). | engine-config | Trial the 14 anchored IP candidates with per-style engine sync/contact markers, then test transition windows before adoption. | Vertical-heading anchoring is available; 0.7.0 anchors all 14 IP gaits, but the candidates remain unpromoted without Humanoid-retarget or visual proof. | High phase evidence; 14/14 outputs produced, with run spread 0.554 to 0.110 and walk spread 0.603 to 0.051. |
| IN-004 | moderate | Fifteen of 21 injury idles show loop-closure/seam lint findings, risking visible periodic pops during long holds. Current loop-seam coverage is 48 applicable/22 not-applicable and 25 complete/45 not-evaluated, so each idle still needs individual review; [loop guidance](../game-ready-clips.md#the-loop-pops). | artist-author | Preview all wraps and repair or crossfade accepted loops. | Current diagnostics exist; repair remains author-reviewed. | High mechanical; visual severity open. |
| IN-005 | moderate | All seven kneel styles have entry motions but no kneel-to-stand counterpart, so recovery requires a reversed or custom transition that may look implausible. Guidance: not applicable. | artist-author | Author/approve a recovery path; do not assume reverse playback is valid. | Transition-family reporting can expose the gap but cannot invent motion. | High inventory evidence. |
| IN-006 | moderate | The current listing says zero root-motion clips while the local archive has 14 `_RM` gait files; purchase expectations and controller design may diverge. Guidance: not applicable. | vendor-license | Scope decisions to the hashed local archive and ask the vendor to reconcile the listing. | Report tooling can flag the discrepancy, not establish revision provenance. | High current-page/local-file evidence. |
| IN-007 | minor | 9,644 constant-track notes across 70 individual files add export/retarget overhead; [optimization guidance](../game-ready-clips.md#the-file-is-bloated-or-the-retargeter-chokes). | animsmith-current-declared | Retain sources; prune only after equivalence tests. | Current pruning is available; still bounded by [issue #401](https://github.com/mmannerm/animsmith/issues/401). | One sample shrank 658,816 to 45,152 bytes; seam remained. |

## Engine status

| Runtime | Evidence level | Technical result | Remaining gate |
|---|---|---|---|
| Unity 6000.5.8f1; 6000.3 advice | Observed headless import/sampling plus 0.7.0 evaluator advice | All 70 clips import; 6/6 samples, both mixers, and the mask graph pass. Observed in-place root locks make the advice declare bake for all three root components. | Visual loops/blends/mask, controller, target rig, compression, and build; 6000.3 advice is not observed 6000.5.8f1 behavior. |
| Unreal Engine 5.8 | 0.7.0 evaluator import-advice attempt | **Not evaluated in-engine.** Current revision-2 settings projection is available; no engine process ran. | FBX import/retarget, markers, root authority, masks, build. |
| Godot 4.7 | 0.7.0 evaluator import-advice attempt | **Not evaluated in-engine.** Current revision-2 settings projection is available; no engine process ran. | Conversion/import, retarget, sync policy, masks, export. |
| Bevy 0.19.0 | 0.7.0 evaluator addressability (generated GLB) | Exit 0: 1 animation row, coverage complete, predicted selector `Animation0`, facet available, 0 findings — inventory/selector prediction only. | glTF conversion of the delivered source, mapping, phase policy, performance. |

## Fit and limitations

Best fit: third-person games that treat injury A–G as explicit full-body states and can tune their own speed thresholds, sync markers, and transition graph.

Poor fit: drop-in omnidirectional locomotion, first-person arms, one universal blend space, or games requiring complete kneel recovery and visually clean loops without authoring. Upper-body-only masking may add a torso injury to Basic or Sword movement, and its headless graph executes, but it discards authored injured legs/pelvis and remains visually unaccepted.

The pack combines technically with Basic Locomotion, Sword & Shield, Campfire, and Climbing: standard skeleton signatures align, pairwise shared paths are byte-identical, and the five-pack Unity project co-imports. Artistic style, gait transition, weapon posture, and state handoffs remain gates; see the [partial collection report](protofactor-ultimate-animation-collection.md).

## Changes between AnimSmith versions

AnimSmith 0.7.0 — Revalidated the unchanged 72-FBX corpus under output v17 / measurements v16. Its 70 contracts reproduce 28 pass / 42 fail and the prior findings; 15 remediation candidates passed verification and emitted addressability V1 plus Bevy rich V2. Unity, Unreal, and Godot advice was available, but these projections do not prove engine execution, blending, retargeting, or visuals.

AnimSmith 0.4.0 — Added vertical-heading gait anchoring; a direct Unity 6000.5.8f1 root-lock observation corrected the metadata inference.

AnimSmith 0.3.0 — Refused the same 14 gait trials.

## Evidence status

Current mechanical evidence is the exact 0.7.0 rerun. The evaluation covers 72 FBXs: 70 individual motions, one combined take, and one actor, plus the dated Unity observation. This report uses manifest schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1` and the [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder). The local archive was supplied as a Protofactor-site download; current store and license pages establish context, not its revision or transaction rights. Exact evidence and current limitations are in the [appendix](protofactor-injured-evidence.md).

## Sources

- Protofactor, [Animset: Injured](https://protofactor.biz/product/animset-injured/) and [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/) — current product scope and collection membership.
- Protofactor, [End User License Agreement](https://protofactor.biz/end-user-license-agreement/) — current vendor-site license terms; not legal advice.
- Unity, [Animation Blend Trees](https://docs.unity3d.com/6000.5/Documentation/Manual/class-BlendTree.html), [Root Motion](https://docs.unity3d.com/6000.5/Documentation/Manual/RootMotion.html), and [looping clips](https://docs.unity3d.com/6000.5/Documentation/Manual/LoopingAnimationClips.html); Epic Games, [animation system](https://dev.epicgames.com/documentation/unreal-engine/skeletal-mesh-animation-system-in-unreal-engine?application_version=5.7); Godot, [AnimationTree](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html); Bevy, [Animation Masks](https://bevy.org/examples/animation/animation-masks/) — runtime capability context only.
