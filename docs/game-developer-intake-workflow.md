# For game developers: from pack to engine gate

Where you are: [Artist export](animation-author-workflow.md) → [Contract](declaring-the-contract.md) → **[Developer intake](game-developer-intake-workflow.md)** → [CI gate](pipeline-scenarios.md#scenario-ci-gating-on-animation-changes) → [Engine check](game-developer-intake-workflow.md#other-engine-routes)

A pack has landed, and somebody has to say whether it can ship in your game.
This page is the order to find out in: inventory what the delivery really
contains, pin the engine you are judging it against, settle who moves the
character, gather the source evidence, and then close the question in the
project itself.

It begins with source evidence and ends with an observed engine gate;
AnimSmith predictions are useful prerequisites, not engine execution. When one
clip already misbehaves, the [symptoms](symptoms/README.md) pages route it
faster than a full intake pass.

## Intake path

1. **Inventory the actual delivery.** Keep source files immutable. Inspect
   every physical file, record logical clip names separately from file names,
   and classify the real runtime sets: locomotion rings, transitions, attacks,
   masked overlays, contacts, and interaction chains. A collection manifest
   can retain that physical-to-logical binding; see
   [collection contracts](collection-contracts.md).

   **Handed over:** the untouched delivery, with its licence and vendor
   metadata, into an immutable raw location —
   [marketplace-pack intake](pipeline-scenarios.md#scenario-marketplace-pack-intake)
   is the sorting pass that follows, and
   [raw vs transformed artifact storage](pipeline-scenarios.md#scenario-raw-vs-transformed-artifact-storage)
   is where each artifact lives.

2. **Choose the engine tuple before interpreting an engine check.** An engine
   profile identifies one documented importer boundary. It does not select a
   skeleton, create a controller, retarget clips, or replace target-engine
   testing. Use the [configuration reference](configuration-reference.md#engine-profiles-cli)
   for the profile schema and the maintained engine page for the exact tuple.

3. **Make movement ownership explicit.** For each locomotion clip, decide
   whether gameplay/controller code or extracted animation owns XZ, Y, and
   yaw movement. This is one of the surfaces only your project can settle;
   [who writes what](declaring-the-contract.md) has the rest. Declare that
   contract, then inspect `in-place`,
   `root-motion-speed`, and any applicable engine prediction. Do not apply
   extracted root motion and controller movement twice, or omit both.

4. **Run source evidence before conversion.** `lint` and `report` establish
   file and declared-contract evidence; `measure` exposes the facts used in a
   disagreement. The [built-in check reference](built-in-checks.md) and
   [output reference](output.md#measure-and-lint) are canonical for those
   records. Convert or assemble only when the target boundary needs it, and
   retain the conversion/assembly evidence beside the derived artifact.

5. **Exercise the target project.** Load the exact emitted artifact, play each
   intended clip, test skeleton/retarget mapping, blend/transition timing,
   masks and contacts, root-motion application, target lookup, and a visual
   gameplay scene. This is the gate that establishes engine-observed evidence.

   **Handed over:** the exact emitted artifact and its evidence, to the
   project that will ship it. Once it is accepted there, every later
   re-export is judged by
   [CI gating on animation changes](pipeline-scenarios.md#scenario-ci-gating-on-animation-changes)
   against this same contract.

## Worked path: Bevy 0.19.0, profile revision 3

The executable examples use the placeholder convention from the
[animation-author workflow](animation-author-workflow.md#work-the-candidate-not-the-source).
`$FBX_FIXTURE` is an authorized self-authored FBX input and `$BEVY_CONFIG` is
the exact TOML fence below, materialized by automation before the command runs.

This path is exact only for `bevy` / revision `3` / `0.19.0` /
`gltf-asset-loader`. It applies to glTF JSON or GLB, not FBX. Preserve an FBX
source, then convert the *candidate* at the format boundary before applying a
Bevy profile:

```console
# workflow-exit: 0
$ANIMSMITH convert "$FBX_FIXTURE" -o "$WORK_DIR/candidate.glb"
```

The conversion is source-to-GLB evidence, not a Bevy import. Then use the
versioned configuration below; it is also committed as
[`examples/bevy-v3.animsmith.toml`](../examples/bevy-v3.animsmith.toml).

```toml
[engine]
profile = "bevy"
profile_revision = 3
engine_version = "0.19.0"
importer = "gltf-asset-loader"

[engine.settings]
extension_handler_environment = "bare_empty"
bevy_animation_feature = true
load_animations = true
```

Run the source-side gate and generate the neutral addressability inventory:

```console
# workflow-exit: 1
$ANIMSMITH --config "$BEVY_CONFIG" lint \
    --select engine-track-support "$ASSET_DIR/walk.glb" --format json
```

```console
# workflow-exit: 0
$ANIMSMITH generate addressability "$ASSET_DIR/walk.glb"
```

The profile's `engine-track-support` prediction can prove only negative loader
gate outcomes from the declared feature/settings and bounded source
animation/channel inventory. If both gates allow loading, the correct result
is required-unavailable runtime survival: AnimSmith does not run Bevy, read
back a Bevy import, or prove that an asset, target, graph, or playback survived.
Likewise, addressability records source animation indices and canonical
selector evidence; it does not prove a live handle resolves in an application.
The [Bevy profile guide](engine-profile-bevy.md#revision-3-animationchannel-gate-support)
owns the precise rule and coverage boundary.

Close the Bevy gate in the project that will ship the asset:

- load `candidate.glb` through the selected application path and record the
  loaded asset/animation evidence;
- resolve the expected typed selector and verify the target mapping after the
  project's own scene root, names, and spawned hierarchy are present;
- create and exercise the application-owned `AnimationPlayer`, graph/state
  transitions, masks, and reset policy;
- make one explicit decision about each root-motion component, then test it
  against controller/physics movement so it is neither double-applied nor
  missing; and
- capture a visual gameplay trial for clip playback, blending, contacts,
  attachments, and scale.

Those are project-owned runtime and visual gates. Their recorded observations,
not a prediction facet, are the evidence that closes them.

## Other engine routes

Use the maintained profile page for each narrower route; do not copy settings
from a different project or version.

| Target | Route |
|---|---|
| Unity 6000.3 | [Unity profile guide](engine-profile-unity.md): exact Generic revision-2 root-motion prediction or preserved Humanoid route, then Unity import/readback, Avatar, controller, and visual test. |
| Unreal Engine 5.8 | [Unreal profile guide](engine-profile-unreal.md): FBX tuple, Skeleton and sequence boundary, then import, retarget/graph, root-motion, and visual test. |
| Godot 4.7 | [Godot profile guide](engine-profile-godot.md): glTF/GLB scene-import tuple, then scene import, retarget/AnimationTree, root motion, and visual test. |
| Custom glTF runtime | [glTF and generic runtime guide](engine-profile-gltf-runtime.md): establish the consumer's own mapping and runtime gates. |

## Close the intake decision

Keep four results separate: source mechanics, declared contract, engine
observation, and gameplay/artistic acceptance. A pack can be usable for a
prototype while a blend, retarget, mask/contact, or visual gate remains open;
record the owner and the exact missing evidence. For commercial evidence that
already exists, use the [commercial-pack evaluation guide](commercial-pack-evaluations.md)
instead of treating a report as an approval for a different project.
