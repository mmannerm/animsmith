# glTF and generic runtime profile

Use this page when glTF/GLB is consumed by a custom runtime or when no frozen
engine tuple matches the actual consumer. This is intentionally **not** a sixth
V1 engine profile. Leave `[engine]` absent and use AnimSmith's engine-neutral
measurements, checks, transforms, and evidence.

## What the format guarantees

The [glTF 2.0 specification](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html)
defines a right-handed, Y-up coordinate system and metres for linear distances.
Animation sampler input is time in seconds and must be finite, non-negative,
and strictly increasing; channels target node translation, rotation, scale, or
morph weights. Names are optional and not guaranteed unique. The runtime still
chooses scene selection, asset addressability, graph/controller behavior,
root-motion extraction, blending, events, masks, physics, and error handling.

## AnimSmith checks and thresholds

No engine-specific check applies without a profile. The engine-neutral catalog
still provides exact source contracts:

| Check id | Exact default boundary | Generic-runtime use |
|---|---|---|
| `time-monotonic` | strict increase; negative tolerance `0.0001 s`; late-first-key note after `0.017 s` | Enforces the glTF timing requirement and identifies an unauthored clamp-held prefix. |
| `quat-norm` | absolute length deviation greater than `0.001` | Prevents unstable runtime interpolation. |
| `scale-keys` | component range greater than `1e-4` | Makes animated scale explicit. |
| `non-uniform-scale` | relative spread greater than `1e-4` | Flags a transform/skin/physics portability risk. |
| `rest-world-scale` | selected node factor `1.0` ± `0.0001` inclusive | Checks project-declared attachment/IK nodes without guessing units from bounds. |
| `loop-closure` | `0.01 m` and `1.0°` | Checks a declared cyclic endpoint. |
| `loop-seam-vel` / `loop-seam-rot` | `0.1 m/s` / `5.0°/s` | Finds wrap velocity discontinuity. |
| `in-place` | XZ speed at least `0.5 m/s` counts as travelling | Enforces declared movement ownership. |

Use the complete [check catalog](../README.md#checks) for structural rig,
duration, FPS, gait, sync, foot-slide, and bind-pose contracts. Do not select
`engine-addressability`; it is defined only for the exact Bevy tuple.

The scale-domain measurements are the engine-neutral output of
[#267](https://github.com/mmannerm/animsmith/issues/267), and the selected-node
policy is [#268](https://github.com/mmannerm/animsmith/issues/268). The
profile-specific advice work in [#155](https://github.com/mmannerm/animsmith/issues/155)
does not invent a generic importer. Static placement baking
[#224](https://github.com/mmannerm/animsmith/issues/224) and skinned rest/bind
reparameterization [#269](https://github.com/mmannerm/animsmith/issues/269)
remain distinct repair contracts.

## Configure the runtime contract

Declare only what the project actually knows:

```toml
[rig]
profile = "humanoid"

[runtime_nodes]
selectors = ["weapon_socket", "ik_*_target"]

[checks.rest-world-scale]
expected_uniform_scale = 1.0
uniform_scale_tolerance = 0.0001

[clips."locomotion_*"]
loop = true
movement_owner_xz = "gameplay"
speed_mps = { value = 3.0, tolerance = 0.2 }
```

There is no fallback or nearest engine profile. Adding a made-up `[engine]`
tuple is an operator error, not a way to label a custom runtime.

## Common failures and fixes

| Symptom | Evidence to inspect | Correct owner |
|---|---|---|
| Asset is the wrong physical size | glTF metre contract, source exporter, `rest-world-scale` | Correct all lengths in DCC/whole-document conversion, or repair one compensating hierarchy scale. |
| Animation name lookup is ambiguous | source animation indices and optional names | Define a runtime-owned unique id policy; glTF names are not identities. |
| Attachment, IK, or collision anchor drifts | source node path, ancestry, affine class | Repair transforms or canonicalize supported rest/bind scale, then test runtime composition. |
| Loop or movement slides | loop checks, `in-place`, `root-motion-speed`, `foot-slide` | Align source motion, declared intent, and controller behavior. |
| Valid glTF still fails to load | dependency closure, required extensions, runtime logs | Fix resource packaging/extension support in the exporter or runtime. |

## Scale and unit workflow

If an exporter wrote centimetre-valued numbers into a format that defines
metres, the whole asset is physically wrong: a factor `0.01` whole-document
conversion is the relevant operation when the supported-source boundary is
met. If the mesh is already the correct world size but a skeleton root carries
`0.01` with compensating descendants, whole-document conversion is wrong;
use the explicit rest/bind operation instead.
That inherited scale also multiplies descendant socket, attachment, IK, and
collision-anchor offsets, animation translations, and root-motion distance.

Static transform baking moves supported unskinned placement into geometry. It
does not repair a skinned hierarchy. Rest/bind reparameterization covers the
accepted skinned design and preserves its stated world-space invariants. The
runtime still owns sockets, attachments, IK, collision anchors, root motion,
graph behavior, extensions, rendering, physics, and visual acceptance. See
[Scaling safely](scale.md) and the [static asset workflow](static-asset-workflows.md).
