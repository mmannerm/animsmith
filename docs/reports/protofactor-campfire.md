# Animation pack evaluation: Protofactor Campfire

> Technical verdict: **Usable with conditions**
>
> Evaluation completeness: **partial** — every delivered FBX and declared clip contract was evaluated and Unity 6000.5.8f1 was probed, but transition, prop-contact, and loop quality were not visually accepted.
>
> Confidence: **high**
>
> Evaluation date: **2026-08-17**
>
> Report format: **1**
>
> Detailed evidence: [Protofactor Campfire evidence](protofactor-campfire-evidence.md)

## Technical decision

Use the 25 individual motion FBXs as a full-body campfire state pack after replacing seven implausible delivered loop flags and visually accepting every posture/contact transition. All 25 share the collection's standard 56-bone skeleton, pass the default mechanical checks apart from optimization notes, import as Unity Humanoid clips, and sample on the shared actor. The delivered campfire and skewer props instantiate at plausible scale.

AnimSmith 0.3.0 identifies loop seam risk and can prune constant tracks, but pruning does not repair motion. The pack does not supply the flint, lighter, matches, stick, or log props implied by five actions, and neither AnimSmith nor headless Unity proves hand/object/fire contacts. The largest boundary is visual gameplay acceptance.

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

The loop values above are the integration policy, not a copy of the package metadata. The package marks all four lighting actions and the grill transition to loop; use them as one-shots unless a visual review proves a deliberate cyclic subrange. Other posture/log candidates are retained in the appendix.

## Integration recipe

1. **Members/topology:** `topology=full-body-state-chain`; build explicit stand, kneel, sit, lie, sleep, and grill states from the exact files above and appendix candidates; choose lighting/log actions discretely.
2. **Timing/synchronization:** `sync=state-endpoints`; keep idles cyclic, make every transition and interaction one-shot, and crossfade only after first/last poses are visually matched; see [loop guidance](../game-ready-clips.md#the-loop-pops).
3. **State ownership:** `owner=game-state-machine`; gameplay owns posture, interaction completion, cancellation, and any approach/departure movement.
4. **Composition constraints:** `composition=full-body`; do not mask posture/contact motions; attach `SM_Skewer.fbx` to the right hand with reviewed offsets and keep `SM_Campfire.fbx` world-owned.
5. **Acceptance gate:** `gate=unity-visual-contact`; test source and target characters at transition boundaries, prop/fire contact frames, cancellation points, and every true idle wrap.

## Technical issue register

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| CF-001 | major | Seven one-shot-like actions/transitions are delivered as loops, so a controller may restart or visibly snap them; [loop guidance](../game-ready-clips.md#the-loop-pops). | engine-config | Override their Unity loop flags and play once. | Report generation can suggest semantic loop review, but intent cannot be changed automatically. | High; filenames, metadata, and contracts agree on the mismatch. |
| CF-002 | moderate | Two true idle candidates have strict rotation/velocity seam errors, risking a once-per-cycle pulse; [loop guidance](../game-ready-clips.md#the-loop-pops). | artist-author | Preview each wrap and use a crossfade only if acceptable. | Deterministic diagnostics exist; pose/tangent repair needs authored acceptance. | High mechanical, visual impact unaccepted. |
| CF-003 | moderate | Lighting, eating, and log actions lack most implied props plus event/contact/IK data, so hands and objects may miss or slide. Guidance: not applicable. | artist-author | Supply props, offsets, events, and contact review in the game. | Contact-sidecar diagnostics may help; missing objects and artistic contacts cannot be invented. | High inventory, no visual contact pass. |
| CF-004 | minor | 3,394 constant-track notes across 25 files add export/retarget overhead; [optimization guidance](../game-ready-clips.md#the-file-is-bloated-or-the-retargeter-chokes). | animsmith-current-declared | Keep sources; consider declared pruning only after equivalence tests. | Current pruning is available and independently inspectable. | One sample shrank from 815,008 to 53,628 bytes; loop finding remained. |
| CF-005 | minor | The bundled animation list renames several delivered files, so automation based on list labels can select the wrong clip; [identity guidance](../game-ready-clips.md#files-disagree-about-skeleton-or-clip-identity). | vendor-license | Treat case-sensitive filenames and manifest IDs as authoritative. | Report tooling can reconcile inventories but must preserve source names. | High; 25-file count reconciles, labels do not. |

## Engine status

| Runtime | Evidence level | Technical result | Remaining gate |
|---|---|---|---|
| Unity 6000.5.8f1 | Observed headless import/sampling | All 25 individual Humanoid clips import; 4/4 samples, one Basic-to-Campfire mixer, skewer attachment, and campfire instantiation pass. | Visual transitions, contacts, loop wraps, target rig, compression, player build. |
| Unreal Engine 5.7 | Documentation only | **Not evaluated.** State machines, montages, and layered animation can express the policy; vendor supplies no native Unreal package. | FBX import/retarget, notifies, contacts, transitions, build. |
| Godot stable | Documentation only | **Not evaluated.** AnimationTree can express transitions, one-shots, filters, and state playback. | Conversion/import, retarget, contacts, graph, export. |
| Bevy unspecified | Documentation only | **Not evaluated.** AnimationGraph supports blending/masks, but the FBX-to-glTF and retarget route is project work. | Conversion, target mapping, state graph, contacts, performance. |

## Fit and limitations

Best fit: third-person RPG, survival, social, or cinematic camp scenes using a full-body state machine and the supplied actor or a visually reviewed retarget.

Poor fit: first-person hands, procedural interaction, seamless locomotion-layered camp actions, or games expecting complete props/events/IK out of the box. Upper-body masking is not recommended for kneel/sit/lie/contact clips because the pelvis and support posture are integral.

Campfire combines technically with Basic Locomotion, Sword & Shield, Climbing, and Injured: the standard files share the same 56-bone signature, all 25 overlapping package paths are byte-identical across every evaluated pair, and the five-pack Unity project co-imports. Style, entry timing, and contacts remain visual gates; see the [partial collection report](protofactor-ultimate-animation-collection.md).

## Evidence status

The evaluation covers 29 FBXs: 25 individual motions, one combined take, one actor, and two props. It uses AnimSmith 0.3.0 at revision `aabac28edf2719db236068339f1208bbf156d0bb`, manifest schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1`, and the [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder). The local archive was supplied as a Protofactor-site download from the Ultimate Animation Collection; current store/license pages establish context, not the local revision or transaction rights. Exact evidence and limitations are in the [appendix](protofactor-campfire-evidence.md).

## Sources

- Protofactor, [Animset: Campfire](https://protofactor.biz/product/animset-campfire/) and [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/) — current product scope and collection membership.
- Protofactor, [End User License Agreement](https://protofactor.biz/end-user-license-agreement/) — current vendor-site license terms; not legal advice.
- Unity, [Animation Blend Trees](https://docs.unity3d.com/6000.5/Documentation/Manual/class-BlendTree.html) and [looping clips](https://docs.unity3d.com/6000.5/Documentation/Manual/LoopingAnimationClips.html); Epic Games, [animation system](https://dev.epicgames.com/documentation/unreal-engine/skeletal-mesh-animation-system-in-unreal-engine?application_version=5.7); Godot, [AnimationTree](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html); Bevy, [Animation Masks](https://bevy.org/examples/animation/animation-masks/) — runtime capability context only.
