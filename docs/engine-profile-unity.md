# Unity 6000.3 animation profile

Use this page when FBX animation will be imported by Unity 6.3 LTS. AnimSmith
has two exact V1 profiles:

| Rig path | Exact tuple |
|---|---|
| Generic | `unity-generic` / revision `1` / `6000.3` / `fbx-model-importer` |
| Humanoid | `unity-humanoid` / revision `1` / `6000.3` / `fbx-model-importer` |

These are importer contracts, not claims that every Unity 6000.3 project uses
the same rig, controller, physics setup, or root-motion policy. Both accept FBX
only. An engine profile never changes AnimSmith measurements.

## What Unity expects

Unity's [Rig tab](https://docs.unity3d.com/6000.3/Documentation/Manual/FBXImporter-Rig.html)
maps a Generic rig through a chosen root node and a Humanoid rig through an
Avatar. Its [Animation tab](https://docs.unity3d.com/6000.3/Documentation/Manual/class-AnimationClip.html)
defines clip ranges, loops, and whether rotation, vertical position, and XZ
position are baked into the pose or extracted as root motion. A clean source
therefore still needs an intentional rig type, Avatar/root selection, clip
cuts, movement ownership, and controller setup.

The [Model tab](https://docs.unity3d.com/6000.3/Documentation/Manual/FBXImporter-Model.html)
documents three separate scale influences: source-file scale, Scale Factor,
and Transform scale. `Convert Units` converts the file's declared scale;
`Bake Axis Conversion` either bakes conversion into asset data or leaves a
compensating root transform. AnimSmith V1 models those two booleans, not Scale
Factor, Avatar construction, or the imported hierarchy that Unity ultimately
creates.

## AnimSmith checks and thresholds

The ordinary [mechanical and contract-aware catalog](../README.md#checks) runs
unchanged under either Unity profile. The most importer-sensitive rows are:

| Check id | Exact default boundary | Why it matters in Unity |
|---|---|---|
| `rest-world-scale` | selected node factor `1.0`, inclusive tolerance `0.0001` | Finds inherited scale at sockets, IK targets, and attachment nodes before import. |
| `scale-keys` | component range greater than `1e-4` | Flags animated scale that may complicate retargeting and attachments. |
| `non-uniform-scale` | relative component spread greater than `1e-4` | Identifies non-uniform scale before Avatar/Transform composition. |
| `constant-nonunit-scale` | unit deviation greater than `1e-4`; off by default | Opt in when the project forbids retained constant scale channels. |
| `in-place` | measured XZ root speed at least `0.5 m/s` counts as travelling | Cross-checks declared gameplay-owned versus animation-owned XZ motion. |
| `loop-closure` | `0.01 m` position and `1.0°` rotation | Checks declared loops before Unity's Loop Pose processing. |
| `loop-seam-vel` / `loop-seam-rot` | `0.1 m/s` / `5.0°/s` | Finds velocity discontinuities that matching endpoint poses can hide. |

There is no Unity-only finding in V1. `engine-addressability` is not applicable;
it is an exact Bevy rule. Unity-specific output comes from `generate
import-advice`, which projects only settings you declared.

The ownership line is explicit: [#267](https://github.com/mmannerm/animsmith/issues/267)
provides the parent/rest-world/bind scale measurements,
[#268](https://github.com/mmannerm/animsmith/issues/268) provides the selected-node
`rest-world-scale` finding, and [#155](https://github.com/mmannerm/animsmith/issues/155)
provides the Unity importer suggestion document. Static placement baking
([#224](https://github.com/mmannerm/animsmith/issues/224)) and skinned rest/bind
reparameterization ([#269](https://github.com/mmannerm/animsmith/issues/269))
are different repair domains.

## Configure the exact importer contract

Every applicable Unity setting is required. The profile does not invent a
default. For Generic:

```toml
[engine]
profile = "unity-generic"
profile_revision = 1
engine_version = "6000.3"
importer = "fbx-model-importer"

[engine.settings]
convert_units = true
bake_axis_conversion = false
root_motion_source = "Reference/Root"

[clips."*".engine_settings]
root_rotation = "extract"
root_position_y = "bake"
root_position_xz = "extract"

[runtime_nodes]
selectors = ["weapon_socket", "ik_*_target"]

[checks.rest-world-scale]
expected_uniform_scale = 1.0
uniform_scale_tolerance = 0.0001
```

For Humanoid, select `unity-humanoid` and omit `root_motion_source`; that
setting is not applicable because the profile records the Avatar body as the
root-motion authority. Both profiles require the three clip-scoped `bake` or
`extract` choices for every actual clip. Generate the bounded advice document
with:

```console
animsmith --config unity.animsmith.toml generate import-advice character.fbx
```

The result maps the declared values to Unity importer properties. It does not
assert that Unity imported successfully or that the imported motion matches
the source visually.

## Common failures and fixes

| Symptom | Evidence to inspect | Correct owner |
|---|---|---|
| Character is 100 times too large or small | source units, `rest-world-scale`, Unity Scale Factor and `Convert Units` | Fix source units in the DCC or set the importer intentionally; do not infer a factor from height. |
| Prop or IK target is offset after import | `rest-world-scale` ancestry plus Unity root compensation | Remove a supported compensating skinned scale with `scale rest-bind`, or fix the hierarchy/import policy. |
| Character glides or runs in place | `in-place`, `root-motion-speed`, declared movement owners, three root extraction settings | Align clip intent, Unity bake/extract choices, and controller ownership. |
| Loop hitches once per cycle | `duplicate-loop-endpoint`, `loop-closure`, seam velocity checks | Correct the cut in the DCC or use the exact eligible endpoint transform; verify again in Unity. |
| Humanoid retarget differs from source | `required-bones`, `bind-pose`, scale checks, Unity import messages | Repair mapping/rest pose in the DCC or Avatar configuration; AnimSmith does not certify retarget quality. |

## Scale and unit workflow

Unity documents one imported world unit as one metre for its physics-oriented
scale convention. That does **not** mean every file should be multiplied until
its root Transform reads one: source units, importer conversion, and inherited
bone scale are separate domains.
An inherited bone scale also multiplies descendant socket, attachment, IK, and
collision-anchor offsets, authored animation translations, and measured root
travel.

- If every represented length is wrong, use DCC unit correction or the
  supported glTF/GLB `scale whole-document` operation before producing the
  Unity FBX. FBX whole-document scaling is not supported.
- If world geometry is already correct but one skinned hierarchy carries a
  compensating uniform scale, use the explicit `scale rest-bind` design. FBX
  input is re-encoded to a proved GLB, so a downstream FBX requirement still
  needs a deliberate DCC/export step.
- `convert --bake-static-mesh-transforms` is for static, unskinned placement.
  It is not a substitute for skinned rest/bind reparameterization.

Before: a hand socket inherits scale `0.01`, so a unit-sized prop or IK offset
is scaled unexpectedly. After a successful rest/bind operation: the measured
world geometry and joint trajectories are preserved while the selected
hierarchy is reparameterized to the declared factor. Still verify Avatar
mapping, attachments, collision anchors, root motion, compression, blending,
and a player build in Unity. See [Scaling safely](scale.md) for the exact write
set and proof boundary.
