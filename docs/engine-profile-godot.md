# Godot 4.7 animation profile

Use this page for `godot` / revision `1` / `4.7` /
`resource-importer-scene`. The frozen profile accepts glTF JSON, GLB, and FBX.
That is AnimSmith's evidence boundary, not a complete list of formats Godot can
import.

## What Godot expects

Godot's [scene importer](https://docs.godotengine.org/en/4.7/classes/class_resourceimporterscene.html)
turns a source scene into Godot nodes and resources. Its advanced settings can
slice animation timelines, optimize tracks, save animations externally, and
configure subresources. [Retargeting 3D skeletons](https://docs.godotengine.org/en/4.7/tutorials/assets_pipeline/retargeting_3d_skeletons.html)
depends on a BoneMap, compatible hierarchy, and aligned rest poses; matching
bone names alone is not enough.

Godot documents a right-handed, Y-up, metre-oriented 3D convention. Its
`nodes/root_scale` defaults to `1.0`; `nodes/apply_root_scale` decides whether
that value is applied into descendants, meshes, animations, and bones or left
on the root node. AnimSmith revision 1 does not model either setting or predict
the resulting imported hierarchy.

## AnimSmith checks and thresholds

All ordinary checks remain engine-neutral. The most relevant are:

| Check id | Exact default boundary | Godot-facing use |
|---|---|---|
| `rest-world-scale` | selected node factor `1.0` ± `0.0001` inclusive | Finds inherited scale before choosing `root_scale`/`apply_root_scale`. |
| `scale-keys` | component range greater than `1e-4` | Finds scale animation that can complicate retargeting. |
| `non-uniform-scale` | relative spread greater than `1e-4` | Flags a hierarchy/physics/attachment risk. |
| `constant-nonunit-scale` | unit deviation greater than `1e-4`; off by default | Enforce a project policy against retained scale channels. |
| `bind-pose` | mean first-frame/rest deviation greater than `45°` | Surfaces a likely rest-pose mismatch before BoneMap retargeting. |
| `frozen-bone` | required bone rotation never exceeds `1°` | Finds wrongly mapped or static required roles. |
| `loop-closure` | `0.01 m` and `1.0°` | Checks a declared source loop before Godot slicing/optimization. |

`engine-addressability` is Bevy-only. Godot has no engine-specific prediction
check in V1.

The measured parent/rest-world/bind domains come from
[#267](https://github.com/mmannerm/animsmith/issues/267), while
[#268](https://github.com/mmannerm/animsmith/issues/268) owns the selected-node
finding. The [#155](https://github.com/mmannerm/animsmith/issues/155) advice
contract refuses rather than inventing Godot settings. Static placement baking
[#224](https://github.com/mmannerm/animsmith/issues/224) and the skinned
rest/bind design [#269](https://github.com/mmannerm/animsmith/issues/269) solve
different transform problems.

## Configure the profile

```toml
[engine]
profile = "godot"
profile_revision = 1
engine_version = "4.7"
importer = "resource-importer-scene"

[runtime_nodes]
selectors = ["weapon_socket", "ik_*_target"]

[checks.rest-world-scale]
expected_uniform_scale = 1.0
uniform_scale_tolerance = 0.0001
```

Godot revision 1 has no setting descriptors. Any engine setting is rejected,
and `generate import-advice` emits `profile_settings_unmodeled`. Configure
timeline slices, root scale, animation optimization, external animation files,
BoneMap, and root-motion behavior in Godot or the source DCC.

## Common failures and fixes

| Symptom | Evidence to inspect | Correct owner |
|---|---|---|
| Imported character has the wrong size | source units, `rest-world-scale`, Godot `root_scale` and `apply_root_scale` | Fix DCC units or set the importer deliberately; avoid stacked compensation. |
| Socket, IK target, or collision anchor is offset | selected-node ancestry and affine class | Repair the hierarchy or canonicalize a supported rest/bind scale; verify after import. |
| Retarget pose twists or limbs drift | `required-bones`, `bind-pose`, first-frame and hierarchy evidence | Align rest pose/BoneMap in Godot or DCC. |
| One long source timeline is not usable as clips | source take inventory and intended cuts | Define slices in Advanced Import Settings or author separate clips in the DCC. |
| Loop pops after optimization | loop closure and velocity checks | Repair the cut/source curves, then verify the imported optimized animation. |

## Scale and unit workflow

Godot's [3D introduction](https://docs.godotengine.org/en/4.7/tutorials/3d/introduction_to_3d.html#coordinate-system)
states that 3D uses metres and recommends authoring at the correct scale because
post-import scaling can create precision problems. Keep three decisions
separate:

- Whole-file unit correction changes every represented length and is supported
  by `scale whole-document` only for self-contained glTF/GLB.
- Skinned rest/bind reparameterization preserves world geometry while removing
  one declared compensating hierarchy scale; glTF/GLB and the narrow FBX-to-GLB
  path are supported.
- `nodes/root_scale` is a Godot importer choice. AnimSmith does not emit it.

An inherited bone scale multiplies descendant sockets, attachments, IK and
collision anchors, animation translations, and root-motion distance no matter
which layer introduced the compensation.

Static transform baking fixes only supported unskinned placement. It cannot
repair a Skeleton3D/BoneMap contract. After any source rewrite, retest
attachments, IK, collision shapes, retarget pose, root motion, animation
slices, and runtime playback in Godot. See [Scaling safely](scale.md).
