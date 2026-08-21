# Animation pack evaluation: Protofactor Campfire

> Technical verdict: **Usable with conditions**
>
> Evaluation completeness: **partial** — every delivered FBX and declared clip contract was evaluated, Unity 6000.5.8f1 was probed directly, and 0.4.0 engine import-advice was probed for Unity, Unreal, and Godot, but transition, prop-contact, and loop quality were not visually accepted.
>
> Confidence: **high**
>
> Evaluation date: **2026-08-21**
>
> Report format: **1**
>
> Detailed evidence: [Protofactor Campfire evidence](protofactor-campfire-evidence.md)

## Technical decision

Use the 25 individual motion FBXs as a full-body campfire state pack after replacing seven implausible delivered loop flags and visually accepting every posture/contact transition. All 25 share the collection's standard 56-bone skeleton, pass the default mechanical checks apart from optimization notes, import as Unity Humanoid clips, and sample on the shared actor. The delivered campfire and skewer props instantiate at plausible scale.

This is a 0.4.0 refresh of the published 0.3.0 evaluation, not a new source: re-inventory reproduces the manifest exactly (0 added, 0 removed, 0 changed across 29 FBXs) and the archive digest re-verifies. The refresh's real change is an honest availability recount, not a new capability. Because every Campfire clip is stationary, `loop_seam_ratio` now reports 0 measured / 26 not_applicable / 1 unavailable — each clip correctly has no real stride to normalize a seam against, instead of risking a false pass or fail under the older accounting. `gait` phase is measured on 26 clips, but `gait-group` is `not_applicable` on all 25 individual contracts because no in-place cyclic ring exists, so no gait anchoring ran; that is a correct not-applicable, not a refusal or a failure, and it does not certify prop or contact quality, which remains a visual gate. Measured `root_trajectory` — delivered this release by closed issue #408 ("expose root displacement and accumulated yaw per clip", closed 2026-08-20) — on 27/27 clips newly confirms 0 clips move more than 1 cm horizontally and 0 exceed 1° of yaw travel, replacing the prior absence-of-evidence argument for "no root motion" with a measured one; these are sampled regression facts from the shared metric grid, not continuous-curve or engine-extraction proof. AnimSmith 0.4.0 still identifies loop seam risk and can prune constant tracks, but pruning does not repair motion. The pack does not supply the flint, lighter, matches, stick, or log props implied by five actions, and neither AnimSmith nor headless Unity proves hand/object/fire contacts. The largest boundary is visual gameplay acceptance.

## Capability coverage

### Complete core

- Seven stationary campfire idles, eleven posture transitions, and seven fire/food/log interactions are present as separate readable motion files.
- The campfire and skewer props, shared Humanoid actor, and Unity metadata are delivered.

### Partial supporting gameplay

- Sitting, lying, kneeling, sleeping, grilling, eating, lighting, and log-toss sequences are mechanically available, but endpoints, contacts, events, and prop offsets need a scene pass.
- Basic Locomotion can supply approach/departure movement; only a headless full-body mixer was tested.

### Absent

- No locomotion, airborne, combat, reaction/death, first-person, additive, aim, paired-character, or root-motion content.
- No authored props for flint, lighter, matches, stick, food, or log, and no contact/event/IK contract.

## Runtime sets and authored motion

| Set/profile | Role or coordinate | Exact members | Variant/type | Timing or motion | Runtime contract |
|---|---|---|---|---|---|
| Sit-lie chain | sit hold | `Humanoid@IdleSitCampfire.fbx` | variant=single | duration=2.667 s | loop=true; state=sit |
| Sit-lie chain | sit to lie | `Humanoid@IdleSitToIdleLayDownCampfire.fbx` | variant=single | duration=4.0 s | loop=false; transition=sit-to-lie |
| Sit-lie chain | lie hold | `Humanoid@IdleLayDownCampfire.fbx` | variant=single | duration=1.967 s | loop=true; state=lie |
| Sit-lie chain | lie to sit | `Humanoid@IdleLayDownToIdleSitCampfire.fbx` | variant=single | duration=2.667 s | loop=false; transition=lie-to-sit |
| Kneel-grill chain | kneel hold | `Humanoid@IdleKneelCampfire.fbx` | variant=single | duration=2.167 s | loop=true; state=kneel |
| Kneel-grill chain | enter grill | `Humanoid@IdleKneelToIdleGrillSkewerCampfire.fbx` | variant=single | duration=3.667 s | loop=false; transition=kneel-to-grill |
| Kneel-grill chain | grill hold | `Humanoid@IdleGrillSkewerCampfire.fbx` | variant=single | duration=3.333 s | loop=true; state=grill |
| Kneel-grill chain | eat | `Humanoid@KneelEatSkewerCampfire.fbx` | variant=single | duration=11.667 s | loop=false; interaction=skewer-eat |
| Fire-lighting alternatives | flint | `Humanoid@FlintstonesLightCampfire.fbx` | variant=single | duration=15.833 s | loop=false; interaction=light-fire |
| Fire-lighting alternatives | lighter | `Humanoid@LighterLightCampfire.fbx` | variant=single | duration=13.0 s | loop=false; interaction=light-fire |
| Fire-lighting alternatives | matches | `Humanoid@MatchesLightCampfire.fbx` | variant=single | duration=10.333 s | loop=false; interaction=light-fire |
| Fire-lighting alternatives | stick | `Humanoid@StickLightCampfire.fbx` | variant=single | duration=16.167 s | loop=false; interaction=light-fire |

The loop values above are the integration policy, not a copy of the package metadata. The package marks all four lighting actions and the grill transition to loop; use them as one-shots unless a visual review proves a deliberate cyclic subrange. Other posture/log candidates are retained in the appendix. AnimSmith 0.4.0's `loop_seam_ratio` recount reports every stationary clip above as `not_applicable` — a stride-availability label, not a semantic verdict — so this one-shot-versus-loop integration policy is unchanged.

## Integration recipe

1. **Members/topology:** `topology=full-body-state-chain`; build explicit stand, kneel, sit, lie, sleep, and grill states from the exact files above and appendix candidates; choose lighting/log actions discretely.
2. **Timing/synchronization:** `sync=state-endpoints`; keep idles cyclic, make every transition and interaction one-shot, and crossfade only after first/last poses are visually matched; see [loop guidance](../game-ready-clips.md#the-loop-pops).
3. **State ownership:** `owner=game-state-machine`; gameplay owns posture, interaction completion, cancellation, and any approach/departure movement.
4. **Composition constraints:** `composition=full-body`; do not mask posture/contact motions; attach `SM_Skewer.fbx` to the right hand with reviewed offsets and keep `SM_Campfire.fbx` world-owned.
5. **Acceptance gate:** `gate=unity-visual-contact`; test source and target characters at transition boundaries, prop/fire contact frames, cancellation points, and every true idle wrap.

## Technical issue register

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| CF-001 | major | Seven one-shot-like actions/transitions are delivered as loops, so a controller may restart or visibly snap them; [loop guidance](../game-ready-clips.md#the-loop-pops). | engine-config | Override their Unity loop flags and play once. | Report generation can suggest semantic loop review, but intent cannot be changed automatically. | High; filenames, metadata, and contracts agree on the mismatch. The 2026-08-21 `loop_seam_ratio` availability recount is a declaration/semantic issue check, not a fix, and does not resolve it. |
| CF-002 | moderate | Two true idle candidates have strict rotation/velocity seam errors, risking a once-per-cycle pulse; [loop guidance](../game-ready-clips.md#the-loop-pops). | artist-author | Preview each wrap and use a crossfade only if acceptable. | Deterministic diagnostics exist; pose/tangent repair needs authored acceptance. | High mechanical, visual impact unaccepted. |
| CF-003 | moderate | Lighting, eating, and log actions lack most implied props plus event/contact/IK data, so hands and objects may miss or slide. Guidance: not applicable. | artist-author | Supply props, offsets, events, and contact review in the game. | Contact-sidecar diagnostics may help; missing objects and artistic contacts cannot be invented. | High inventory, no visual contact pass. |
| CF-004 | minor | 3,394 constant-track notes across 25 files add export/retarget overhead; [optimization guidance](../game-ready-clips.md#the-file-is-bloated-or-the-retargeter-chokes). | animsmith-current-declared | Keep sources; consider declared pruning only after equivalence tests. | Current pruning is available and independently inspectable; tracked by open issue #401 (verified open 2026-08-21). | One sample shrank from 815,008 to 53,628 bytes; loop finding remained. |
| CF-005 | minor | The bundled animation list renames several delivered files, so automation based on list labels can select the wrong clip; [identity guidance](../game-ready-clips.md#files-disagree-about-skeleton-or-clip-identity). | vendor-license | Treat case-sensitive filenames and manifest IDs as authoritative. | Report tooling can reconcile inventories but must preserve source names. | High; 25-file count reconciles, labels do not. |

## Engine status

| Runtime | Evidence level | Technical result | Remaining gate |
|---|---|---|---|
| Unity 6000.3 / 6000.5.8f1 | Retained observed headless import/sampling (6000.5.8f1, 2026-08-17) plus AnimSmith `generate import-advice` under the frozen `unity-humanoid` revision 1 / Unity 6000.3 / `fbx-model-importer` profile (2026-08-21). | 25/25 individual Humanoid clips import; 4/4 samples, one Basic-to-Campfire mixer, skewer attachment, and campfire instantiation pass (retained). Import-advice is `available` (exit 0): every clip's `lockRootRotation`/`lockRootHeightY`/`lockRootPositionXZ` is derived from each delivered `.fbx.meta` (`useFileUnits:1`; `lockRoot*` absent on every meta, so Unity's serialization default of `false` applies, mapping to `extract`). | Visual transitions, contacts, loop wraps, target rig, compression, player build; import-advice is settings guidance, not observed 6000.5.8f1 behavior. |
| Unreal Engine 5.8 | AnimSmith `generate import-advice` probe under `unreal` revision 1 (2026-08-21) plus documentation review. | Typed refusal `profile_settings_unmodeled` (exit 1): profile revision 1 has no modeled Unreal setting vocabulary. State machines, montages, and layered animation can express the policy; vendor supplies no native Unreal package. | FBX import/retarget, notifies, contacts, transitions, build. |
| Godot 4.7 | AnimSmith `generate import-advice` probe under `godot` revision 1 (2026-08-21) plus documentation review. | Typed refusal `profile_settings_unmodeled` (exit 1): profile revision 1 has no modeled Godot setting vocabulary. AnimationTree can express transitions, one-shots, filters, and state playback. | Conversion/import, retarget, contacts, graph, export. |
| Bevy 0.19.0 | Not evaluated (2026-08-21); documentation review only. | No generated glTF/GLB candidate exists for this stationary pack (no gait-anchored in-place ring), so `generate addressability` had nothing to inventory. This is a coverage gap, not an observed Bevy failure. AnimationGraph supports blending/masks, but the FBX-to-glTF and retarget route is project work. | Conversion, target mapping, state graph, contacts, performance. |

## Fit and limitations

Best fit: third-person RPG, survival, social, or cinematic camp scenes using a full-body state machine and the supplied actor or a visually reviewed retarget.

Poor fit: first-person hands, procedural interaction, seamless locomotion-layered camp actions, or games expecting complete props/events/IK out of the box. Upper-body masking is not recommended for kneel/sit/lie/contact clips because the pelvis and support posture are integral. AnimSmith 0.4.0's per-bone `bone_channels` coverage — delivered this release by closed issue #402 ("expose per-clip channel coverage at (bone, property) granularity", closed 2026-08-20) — confirms which bones carry authored translation/rotation/scale tracks on these clips, which can narrow a composition/prop-mask risk discussion, but channel presence alone does not prove a visually acceptable engine mask.

Campfire combines technically with Basic Locomotion, Sword & Shield, Climbing, and Injured: the standard files share the same 56-bone signature, all 25 overlapping package paths are byte-identical across every evaluated pair, and the five-pack Unity project co-imports. Style, entry timing, and contacts remain visual gates; see the [partial collection report](protofactor-ultimate-animation-collection.md).

## Evidence status

The evaluation covers 29 FBXs: 25 individual motions, one combined take, one actor, and two props. This 2026-08-21 refresh uses AnimSmith 0.4.0 (tag `v0.4.0`, revision `6b37ad636b198ef8ff47fadbf6a3a51eb1a27c8e`, binary SHA-256 `fd1eee57407aa02db88763d144389a7f5104204c40ddfbb28eb5885ca8cd54c6`, output schema v10, measurements schema v15) against the same source: re-inventory reproduces the published manifest exactly (0 added, 0 removed, 0 changed) and the archive digest re-verifies, so this is a pure evaluator-version refresh. Historical results from AnimSmith 0.3.0 (revision `aabac28edf2719db236068339f1208bbf156d0bb`) and the 2026-08-17 Unity 6000.5.8f1 headless probe are retained as dated evidence and keep their original date because the source is byte-identical. This report uses manifest schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1` and the [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder). The local archive was supplied as a Protofactor-site download from the Ultimate Animation Collection; current store/license pages establish context, not the local revision or transaction rights. Exact evidence and limitations are in the [appendix](protofactor-campfire-evidence.md).

## Sources

- Protofactor, [Animset: Campfire](https://protofactor.biz/product/animset-campfire/) and [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/) — current product scope and collection membership.
- Protofactor, [End User License Agreement](https://protofactor.biz/end-user-license-agreement/) — current vendor-site license terms; not legal advice.
- Unity, [Animation Blend Trees](https://docs.unity3d.com/6000.5/Documentation/Manual/class-BlendTree.html) and [looping clips](https://docs.unity3d.com/6000.5/Documentation/Manual/LoopingAnimationClips.html); Epic Games, [animation system](https://dev.epicgames.com/documentation/unreal-engine/skeletal-mesh-animation-system-in-unreal-engine?application_version=5.8); Godot, [AnimationTree](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html); Bevy, [Animation Masks](https://bevy.org/examples/animation/animation-masks/) — runtime capability context only.
- AnimSmith, [Unity 6000.3 animation profile](../engine-profile-unity.md), [Unreal Engine 5.8 animation profile](../engine-profile-unreal.md), [Godot 4.7 animation profile](../engine-profile-godot.md), and [Bevy 0.19.0 animation profile](../engine-profile-bevy.md) — modeled 0.4.0 import-advice profile facts, accessed 2026-08-21.
