# Bevy 0.19.0 animation profile

Use this page for the exact `bevy` / `0.19.0` / `gltf-asset-loader`
profiles. Revision 1 owns animation-label addressability. Revision 2 owns the
unit/effective-scale prediction substrate and its version-pinned load settings.
Revision 3 is the current slice for animation/channel gate support and the
separately versioned rich addressability-rule bundle. All three
accept glTF JSON and GLB only; none applies to a later Bevy
release.

## What Bevy expects

Bevy loads glTF subassets through typed labels. In 0.19.0,
[`GltfAssetLabel::Animation(index)`](https://docs.rs/bevy/0.19.0/bevy/gltf/enum.GltfAssetLabel.html)
formats as `Animation{index}`. An animation name is separate optional metadata,
not part of that typed selector. Runtime playback still requires the relevant
animation feature/settings, loaded assets, an `AnimationPlayer`, targets, and
an animation graph or equivalent application setup.

The frozen profile also records that animation target ids are derived from
name paths. Revision 3's separately versioned rich addressability bundle emits
bounded predictions for those paths, named-map winners, scene/skin labels, and
the optional default-scene route. It does not emit graph templates, extension
support, or proof that a runtime asset exists.

## AnimSmith checks and thresholds

| Check id | Exact boundary | Bevy-facing use |
|---|---|---|
| `engine-addressability` | no numeric tolerance; exactly one `Animation{i}` facet per completely inventoried source animation | Predicts the canonical typed label spelling from source order. Partial inventory produces one unsuppressible required-unavailable inventory facet. |
| `engine-unit-scale` | exact revision-2 profile facts and settings; no tolerance and no content finding | Emits exact 1:1 glTF-metre to Bevy world-length-unit mapping plus distinct loader-scene, primitive-child, and selected-source-node affine classifications. Missing required evidence is unsuppressible required-unavailable. |
| `engine-track-support` | exact revision-3 gate settings and bounded same-load animation/channel inventory; no content finding | Emits only negative gate outcomes for source animation/channel rows, or required-unavailable evidence when inventory is incomplete or both gates allow loading. Extensions, other constructs, and positive runtime survival are outside this slice. |
| `engine-addressability` (rich V2) | exact revision-3 Bevy addressability-rule bundle; no numeric tolerance or content finding | Emits typed scene, default-scene, skin/IBM, named-map, and target path/UUID projections beside the unchanged `Animation{i}` facets. Incomplete, unreachable, multiply reachable, feature-disabled, or colliding work is required-unavailable. |
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

### Revision 3: animation/channel gate support

Revision 3 preserves the revision-2 Bevy tuple and adds the narrow
`engine-track-support` check. The only new settings are the document-scoped
`bevy_animation_feature` and `load_animations`, each recorded with its
explicit/default origin:

```toml
[engine]
profile = "bevy"
profile_revision = 3
engine_version = "0.19.0"
importer = "gltf-asset-loader"

[engine.settings]
extension_handler_environment = "bare_empty" # or "bevy_pbr_stock_0_19"
bevy_animation_feature = true
load_animations = true
```

The compiled `bevy_animation` feature gate has precedence over
`load_animations`. A disabled feature gate therefore predicts a negative drop
even when `load_animations = true`; with the feature enabled, a false
`load_animations` setting predicts the corresponding loader drop. These are
negative importer outcomes, not content findings. If both gates allow loading,
the result is a stable required-unavailable runtime-survival state because
AnimSmith does not execute Bevy or inspect its runtime asset.

The rule inventories raw source animation rows and independent per-animation
channel rows from the same load. Complete-empty source animation inventory is
not applicable. Partial or unavailable animation/channel coverage emits
exactly one subjectless, unsuppressible inventory
`required_prediction_unavailable` facet and no retained-prefix prediction;
bounded N+1 evidence is therefore distinguishable from a complete result.
Extensions, unsupported constructs, target survival, graph wiring, and other
positive runtime claims remain outside revision 3. Current output-v18 carries
its immutable V5 provenance unchanged; the output-v17 and output-v16 readers and the
revision-2/V4 output-v15 reader remain preserved.

### Revision 3 rich addressability

The rich standalone contract is the immutable
`urn:animsmith:schema:gltf-addressability:2`; see
[`gltf-addressability-v2.schema.json`](schemas/gltf-addressability-v2.schema.json).
It is selected only for the exact revision-3 tuple and preserves the V1
`gltf-animation-addressability:1` inventory as a nested animation domain.
The adapter runs one `engine-addressability` evaluation and does not create a
second lifecycle.

The separately versioned authority pins Bevy tag `v0.19.0`, commit
`c6f634ca9f406d68ba5109d921247b654cb42c10`, `bevy_gltf 0.19.0`, locked
`gltf 1.4.1`, and the label, loader, node-path, `AnimationTargetId`, feature,
and root `Cargo.lock` sources. It requires explicit target pointer width
(`bits32` or `bits64`) because Bevy hashes path segment lengths using the target
pointer width; AnimSmith never uses the host width. Missing `bevy_animation` or disabled
`load_animations` is typed unavailable, not runtime success.

`Scene{i}` is emitted for every declared source scene. `Gltf.default_scene`
is only a route to the selected existing `Scene{i}`: there is no
`DefaultScene` label and no fabricated `Scene0`. Every source skin eagerly
gets `Skin{i}/InverseBindMatrices`, including unreferenced skins, with Bevy's
identity fallback when inverse-bind data is absent. `Skin{i}` is created when
any source node references it during the all-source-node construction pass;
source skin indices, never collected-vector position, are authoritative.
Explicit `skin.skeleton` remains source evidence only because Bevy ignores it;
scene-instantiated `SkinnedMesh` attachment is outside this static slice.

Named scene/animation/skin maps are separate from typed labels and are
source-order last-write-wins; skin winners follow lazily created skins in
first-reference order. Target rows are per unique source animation target node
with contributing animation/channel identities. Authored names or
`GltfNode{source_index}` fallbacks form the path, excluding the scene
world-root. A target path/UUID is published only when complete hierarchy,
reachability, closure, feature/settings, and collision evidence exists.

The report carries an independent `target_coverage` projection alongside the
retained target rows. It is complete when the unique-target domain is
exhaustively represented by complete raw node/scene/path and animation/channel
evidence, including an empty domain; incomplete evidence makes it
`required_unavailable`. It also becomes
`required_unavailable` with `target_domain_truncated` when more than 4,096
targets exist, or with `projection_bounds_exceeded` when a new rich projection
exceeds its aggregate structural/text budget.

Each new rich projection scene/node/skin/attachment/path/target/map domain is
capped at 4,096 rows, with aggregate projection structural references at
65,536 and dynamic projection text at 1 MiB. The sealed V1 animation
inventory and `CheckEvaluation` scopes retain their own bounds. One name/path
segment is capped at 1,024 UTF-8 bytes, a path at 4,096 bytes and 256
segments, and a report at 256 MiB. The explicit target-coverage projection
reports `target_domain_truncated` at the target row ceiling and
`projection_bounds_exceeded` for other rich-projection budget exhaustion;
readback rejects N+1 collections and contradictory states. This is prediction
evidence only and does not certify Bevy loading, spawning, target survival,
graph wiring, or playback.

If the shared check's core per-file facet budget would be exceeded, V2 exposes
one subjectless `engine-addressability:facet-budget` scope with
`facet_budget_exceeded` rather than a misleading retained-facet prefix. This
is compaction within the same check lifecycle, not an additional check.

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
