# Bevy 0.19.0 animation profile

Use this page for the exact `bevy` / `0.19.0` / `gltf-asset-loader`
profiles. Revision 1 owns animation-label addressability. Revision 2 owns the
unit/effective-scale prediction substrate and its version-pinned load settings.
Both accept glTF JSON and GLB only; neither applies to a later Bevy release.

## What Bevy expects

Bevy loads glTF subassets through typed labels. In 0.19.0,
[`GltfAssetLabel::Animation(index)`](https://docs.rs/bevy/0.19.0/bevy/gltf/enum.GltfAssetLabel.html)
formats as `Animation{index}`. An animation name is separate optional metadata,
not part of that typed selector. Runtime playback still requires the relevant
animation feature/settings, loaded assets, an `AnimationPlayer`, targets, and
an animation graph or equivalent application setup.

The frozen profile also records that animation target ids are derived from
name paths. AnimSmith does not yet emit those ids, named-animation map winners,
scene/skin labels, graph templates, extension support, or proof that a runtime
asset exists.

## AnimSmith checks and thresholds

| Check id | Exact boundary | Bevy-facing use |
|---|---|---|
| `engine-addressability` | no numeric tolerance; exactly one `Animation{i}` facet per completely inventoried source animation | Predicts the canonical typed label spelling from source order. Partial inventory produces one unsuppressible required-unavailable inventory facet. |
| `engine-unit-scale` | exact revision-2 profile facts and settings; no tolerance and no content finding | Emits exact 1:1 glTF-metre to Bevy world-length-unit mapping plus distinct loader-scene, primitive-child, and selected-source-node affine classifications. Missing required evidence is unsuppressible required-unavailable. |
| `rest-world-scale` | selected node factor `1.0` ± `0.0001` inclusive | Finds authored inherited scale at attachment/IK nodes; it does not predict Bevy's imported result. |
| `scale-keys` | component range greater than `1e-4` | Finds animated scale before runtime transform propagation. |
| `non-uniform-scale` | relative spread greater than `1e-4` | Flags possible shear/attachment/physics consequences in composed transforms. |
| `loop-closure` | `0.01 m` and `1.0°` | Checks declared loops before graph playback. |
| `in-place` | XZ speed at least `0.5 m/s` counts as travelling | Makes gameplay versus animation movement ownership explicit. |

The rest of the [check catalog](../README.md#checks) is unchanged by the
profile.

For scale, [#267](https://github.com/mmannerm/animsmith/issues/267) owns the
measurement domains and [#268](https://github.com/mmannerm/animsmith/issues/268)
owns the selected-node finding. The [#155](https://github.com/mmannerm/animsmith/issues/155)
import-advice work has no Bevy payload. Static placement baking
[#224](https://github.com/mmannerm/animsmith/issues/224) and skinned rest/bind
reparameterization [#269](https://github.com/mmannerm/animsmith/issues/269)
remain separate from selector generation.

## Configure and generate selectors

```toml
[engine]
profile = "bevy"
profile_revision = 1
engine_version = "0.19.0"
importer = "gltf-asset-loader"

[runtime_nodes]
selectors = ["weapon_socket", "ik_*_target"]

[checks.rest-world-scale]
expected_uniform_scale = 1.0
uniform_scale_tolerance = 0.0001
```

```console
animsmith --config bevy.animsmith.toml lint --select engine-addressability character.glb
animsmith --config bevy.animsmith.toml generate addressability character.glb
```

The standalone addressability inventory retains source animation/channel
indices, optional names, raw targets/accessors, and dependency closure. The
optional Bevy section uses the same existing check. Named, unnamed, and
duplicate-named animations remain distinct because source index is the
authority. The selector can change when animation array order changes.

Bevy revision 1 has no settings vocabulary, so `[engine.settings]` and clip
engine settings are invalid. `generate import-advice` is not a Bevy settings
generator.

For unit/effective-scale prediction, select revision 2 and declare the two
required environment facts. Defaults are retained with their origin; list
them explicitly only when the load differs:

```toml
[engine]
profile = "bevy"
profile_revision = 2
engine_version = "0.19.0"
importer = "gltf-asset-loader"

[engine.settings]
extension_handler_environment = "bare_empty" # or "bevy_pbr_stock_0_19"
bevy_animation_feature = true
rotate_scene_entity = false # default
rotate_meshes = false       # default
load_meshes = "nonempty"    # default; or "empty"
load_animations = true      # default

[runtime_nodes]
selectors = ["weapon_socket", "ik_*_target"]
```

```console
animsmith --config examples/bevy-v2.animsmith.toml lint \
  --select engine-unit-scale character.glb --format json
```

The handler environment is closed to an empty registry or exactly Bevy 0.19's
stock PBR handler. Arbitrary application handlers are not approximated. With
`load_meshes = "empty"`, the rule reports each otherwise-created primitive
child as suppressed while retaining the source-node entity semantics.

## Common failures and fixes

| Symptom | Evidence to inspect | Correct owner |
|---|---|---|
| Code loads the wrong animation after a source edit | generated source index and `Animation{i}` selector | Regenerate the manifest and update the code/asset contract; a numeric typed label is not stable across reordering. |
| Selector exists but nothing plays | dependency closure, Bevy loader settings/features, `AnimationPlayer`, graph and targets | Fix runtime loading/graph wiring; the selector prediction does not prove existence. |
| Duplicate names collide in application lookup | source names and distinct indices | Use typed index labels or define an application-owned name policy. |
| Attachment or IK target is scaled/sheared | `rest-world-scale` path, ancestry, affine class | Repair source hierarchy or supported rest/bind scale; validate the composed Bevy transform at runtime. |
| Clip loops or movement ownership disagree with gameplay | loop checks, `in-place`, `root-motion-speed` | Fix source/contract and graph/controller behavior. |

## Scale and unit workflow

glTF defines all linear distances in metres. Revision 2 proves that one glTF
metre-valued length reaches one Bevy world-space length unit with no loader
scale conversion; it deliberately does not claim a universal application,
physics, or gameplay metre convention. Treat the format mapping and the
application's world convention as two separate authorities.
Inherited scale still multiplies descendant attachment, IK, and collision
offsets, authored animation translations, and root travel in the composed
transform hierarchy.

Use `scale whole-document` only when every represented glTF length is wrong.
Use `scale rest-bind` when geometry is already correct and one skinned
hierarchy carries a declared compensating uniform scale. Use static transform
baking only for unskinned placement. None of these operations proves Bevy
scene spawning, target-id paths, attachment behavior, physics/collision scale,
root motion, animation-graph wiring, or visual playback.

Before: a selected weapon node has a world affine classification that includes
uniform factor `0.01`. After rest/bind: AnimSmith proves preserved geometry and
joint trajectories under the reparameterized hierarchy. Then load the emitted
asset in the exact Bevy application, validate the caller-owned `WorldAssetRoot`
and ancestors that are outside importer output, resolve the typed label, attach
the prop, and exercise the graph and physics setup. See
[Scaling safely](scale.md).
