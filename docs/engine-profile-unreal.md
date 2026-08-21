# Unreal Engine 5.8 animation profile

Use this page for the exact AnimSmith tuple `unreal` / revision `1` / `5.8` /
`fbx-importer`. V1 accepts FBX only.

## What Unreal expects

Unreal imports skeletal motion as Animation Sequence assets tied to a
Skeleton. The [FBX animation pipeline](https://dev.epicgames.com/documentation/en-us/unreal-engine/fbx-animation-pipeline-in-unreal-engine?application_version=5.8)
documents FBX 2020.2 and requires an existing compatible Skeleton for an
animation-only import. The [Animation Sequences guide](https://dev.epicgames.com/documentation/en-us/unreal-engine/animation-sequences-in-unreal-engine?application_version=5.8)
also documents frame ranges, sample rates, custom attributes, bone-name and
hierarchy matching, and the requirement that animation end frames be whole.

The frozen profile knows that Unreal uses a left-handed, Z-up basis with +X
forward and a centimetre distance unit. It also knows that the importer exposes
unit and axis conversion controls. Those facts do not prove the exact imported
hierarchy, conversion result, or runtime root-motion policy.

## AnimSmith checks and thresholds

The normal [check catalog](../README.md#checks) is engine-neutral. Prioritize:

| Check id | Exact default boundary | Unreal-facing use |
|---|---|---|
| `fps` | duration and keys must be within `0.1` frame of the explicitly declared frame grid | Detects source timing that disagrees with the project contract; it does not prove Unreal's imported frame range. |
| `loop-closure` | `0.01 m` and `1.0°` | Checks declared loop endpoints before import/resampling. |
| `loop-seam-vel` / `loop-seam-rot` | `0.1 m/s` / `5.0°/s` | Finds a wrap pulse after pose equality. |
| `scale-keys` | component range greater than `1e-4` | Makes imported scale tracks explicit; Unreal may store non-unit scale only when present. |
| `non-uniform-scale` | relative spread greater than `1e-4` | Flags a portability and retargeting risk even though Unreal can import scale animation. |
| `rest-world-scale` | selected node factor `1.0` ± `0.0001` inclusive | Finds inherited socket, attachment, IK, and collision-anchor scale. |
| `in-place` | XZ speed at least `0.5 m/s` counts as travelling | Cross-checks whether gameplay or animation owns horizontal motion. |

There is currently no `whole-end-frame` engine check. AnimSmith retains FBX
seconds and FPS, not an authoritative FBX tick/frame coordinate, so it does
not guess at the documented Unreal rule. `engine-addressability` is Bevy-only.

Scale evidence and policy stay separated: [#267](https://github.com/mmannerm/animsmith/issues/267)
owns the measured scale domains and [#268](https://github.com/mmannerm/animsmith/issues/268)
owns the selected-node finding. The [#155](https://github.com/mmannerm/animsmith/issues/155)
advice contract reports an honest refusal for this profile. Static placement
baking [#224](https://github.com/mmannerm/animsmith/issues/224) does not replace
the skinned rest/bind design [#269](https://github.com/mmannerm/animsmith/issues/269).

## Configure the profile

```toml
[engine]
profile = "unreal"
profile_revision = 1
engine_version = "5.8"
importer = "fbx-importer"

[clips."locomotion_*"]
loop = true
movement_owner_xz = "animation"
max_loop_position_delta_m = 0.01
max_loop_rotation_delta_deg = 1.0
```

Unreal revision 1 has no materialized setting vocabulary. Supplying
`[engine.settings]` or clip engine settings is an error. `generate
import-advice` returns the typed `profile_settings_unmodeled` refusal rather
than inventing Convert Scene Unit, frame ranges, sample rates, or root-motion
settings.

## Common failures and fixes

| Symptom | Evidence to inspect | Correct owner |
|---|---|---|
| Animation is the wrong physical size | source units, Unreal `Convert Scene Unit`, `rest-world-scale` | Author centimetre/metre intent in the DCC or choose the importer option explicitly. |
| Sequence truncates or imports the wrong range | source take range, declared FPS, Unreal Animation Length/Frame Import Range | Fix the exact frame cut in the DCC or importer; AnimSmith V1 does not infer frame numbers. |
| Animation-only import cannot bind | `required-bones`, `missing-bones`, hierarchy and names | Export against the target Skeleton or retarget in Unreal/DCC. |
| Root motion does not drive the actor | `in-place`, `root-motion-speed`, root role evidence | Ensure a root bone carries the intended motion and configure the Animation Sequence/graph. |
| Socket, IK, or collision attachment scales incorrectly | `rest-world-scale` source path and ancestry | Repair inherited scale or use supported rest/bind reparameterization, then retest in Unreal. |

## Scale and unit workflow

Unreal's [units guide](https://dev.epicgames.com/documentation/en-us/unreal-engine/units-of-measurement-in-unreal-engine?application_version=5.8)
uses centimetres for distance by default, while AnimSmith measurements are
normalized to metres. Compare physical quantities, not the raw number printed
in two different unit systems. The FBX import option `Convert Scene Unit`
converts source units to centimetres.
Inherited bone scale also multiplies descendant sockets, attachments, IK and
collision anchors, animation translations, and root-motion distance.

Use `scale whole-document` only for self-contained glTF/GLB whose entire
represented length domain is wrong. Use `scale rest-bind` when world geometry
is correct but a selected skinned hierarchy carries a compensating uniform
scale; narrow FBX rest/bind produces GLB. Use static transform baking only for
unskinned placement. None of these operations chooses Unreal Skeleton,
retarget, root-motion extraction, socket, IK, collision, or compression policy.

Before: a weapon socket inherits `100` while the mesh is visually compensated.
After rest/bind: the proved world geometry and trajectories are unchanged and
the selected hierarchy is canonicalized. Unreal import, socket attachment,
physics asset, root motion, retargeting, and runtime playback still require
engine validation. See [Scaling safely](scale.md).
