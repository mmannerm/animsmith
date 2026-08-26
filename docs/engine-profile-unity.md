# Unity 6000.3 animation profile

Use this page when FBX animation will be imported by Unity 6.3 LTS. AnimSmith
has a current Unity Generic root-motion profile plus the preserved Humanoid
profile:

| Rig path | Exact tuple |
|---|---|
| Generic | `unity-generic` / revision `2` / `6000.3` / `fbx-model-importer` |
| Humanoid | `unity-humanoid` / revision `1` / `6000.3` / `fbx-model-importer` |

These are importer contracts, not claims that every Unity 6000.3 project uses
the same rig, controller, physics setup, or root-motion policy. Both accept FBX
only. Revision 2 is the exact Generic contract used by the
`engine-root-motion` prediction; revision 1 remains readable for historical
advice artifacts. An engine profile never changes AnimSmith measurements.

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
and Transform scale. The preserved revision-1 advice profile models `Convert
Units` and `Bake Axis Conversion`; Generic revision 2 deliberately does not
claim either setting. It models only the closed animation/root-motion controls
listed below, not Scale Factor, Avatar construction, or the imported hierarchy
that Unity ultimately creates.

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

The current Unity-only check is `engine-root-motion`, and it is applicable only
for the exact Generic revision-2 tuple, an FBX source, and declared movement
ownership. `engine-addressability` remains a Bevy-only rule. The separate
`generate import-advice` contract still projects only settings declared under
the historical revision-1 Unity advice profile.

The ownership line is explicit: [#267](https://github.com/mmannerm/animsmith/issues/267)
provides the parent/rest-world/bind scale measurements,
[#268](https://github.com/mmannerm/animsmith/issues/268) provides the selected-node
`rest-world-scale` finding, and [#155](https://github.com/mmannerm/animsmith/issues/155)
provides the Unity importer suggestion document. Static placement baking
([#224](https://github.com/mmannerm/animsmith/issues/224)) and skinned rest/bind
reparameterization ([#269](https://github.com/mmannerm/animsmith/issues/269))
are different repair domains.

## Configure the exact importer contract

Every applicable revision-2 setting is required; the profile does not invent a
default. The exact Generic root-motion tuple is:

```toml
[engine]
profile = "unity-generic"
profile_revision = 2
engine_version = "6000.3"
importer = "fbx-model-importer"

[engine.settings]
animation_type = "generic"
avatar_setup = "create_from_this_model"
import_animation = true
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

For Humanoid, select the preserved `unity-humanoid` revision-1 profile and omit
`root_motion_source`; that setting is not applicable to the Humanoid profile.
Generic revision 2 freezes `animation_type = "generic"`,
`avatar_setup = "create_from_this_model"`, and `import_animation = true`; other
values are rejected. The three clip-scoped `bake` or `extract` choices are
required for every actual Generic clip. The example above is a prediction
configuration, not a Unity project file.

## Root-motion prediction

`engine-root-motion` compares one declared movement owner with one materialized
Unity clip setting for each declared axis: `horizontal_xz` uses
`root_position_xz`, `vertical_y` uses `root_position_y`, and `yaw` uses
`root_rotation`. `bake` means `baked_into_pose`; `extract` means
`stored_as_root_motion`. Gameplay ownership is compatible with baking, while
animation ownership is compatible with extraction. A conflict is an ordinary
error finding with a `prediction_scope`, not a rewritten measurement or an
automatic fix.

The check's lifecycle is explicit. A selected, enabled, applicable check emits
one available facet per clip/axis when all evidence is present. Its machine
result is `RootMotionRouting` with `project_owner`, `importer_disposition`, and
`compatibility`. A clip with no declared axis is not applicable for that axis.
Missing raw-path coverage, incomplete project intent, duplicate/overflowed
settings, a missing or ambiguous source path, a path that does not identify the
resolved Root role, or unavailable axis-specific trajectory evidence emits a
`required_prediction_unavailable` facet instead. Required-unavailable work is
not a content finding, cannot be suppressed with `--allow`, makes the check
`not_evaluated` when all work is unavailable (or `partial` when mixed), and
makes `lint` exit 1.

The source path must resolve to the explicitly resolved `Root` role. This rule
does not use the consumer-neutral Hips fallback: a resolved Hips role cannot
stand in for a missing Root. Root trajectory measurements may be unavailable
for an individual axis, but their numeric magnitude is never an applicability
or routing threshold. A zero-travel clip and a travelling clip use the same
ownership/setting comparison when their required measurement availability is
`measured`.

### Closed raw FBX path evidence

`root_motion_source` is a case-sensitive, byte-exact relative transform path.
The V1 grammar uses `/` as its only separator and has no escaping or Unicode
normalization. Paths must have nonempty segments; leading/trailing `/`, `//`,
`.` and `..` segments, backslashes, control or Unicode format characters are
rejected. A segment is at most 1,024 UTF-8 bytes, a complete path at most 4,096
bytes, and a path at most 256 segments.

The FBX loader projects this path inventory from the same input bytes through a
raw-preserving ufbx load. Original source node identities, parent chains, and
names are retained; the implicit ufbx root and generated geometry/scale helper
nodes remain evidence but cannot satisfy a configured path. Complete coverage
is required to prove `NoMatch`; incomplete or unavailable coverage produces
`CoverageIncomplete`, never a guessed absence. This is raw-source evidence,
not a claim that Unity executed the import.

Generate the bounded advice document with the historical advice profile using:

```console
animsmith --config unity.animsmith.toml generate import-advice character.fbx
```

The result maps the declared values to Unity importer properties. It does not
assert that Unity imported successfully or that the imported motion matches
the source visually. The root-motion prediction likewise performs no Unity
editor execution, imported-asset readback, runtime playback, or engine
certification. Repository and CI evidence for this slice uses only
self-authored synthetic FBX fixtures; commercial animation packs are not
fixtures or uploaded artifacts.

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
