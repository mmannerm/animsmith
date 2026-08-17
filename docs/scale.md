# Scaling glTF safely

`animsmith scale` is an evidence-emitting producer for two different
glTF/GLB operations. Both require an explicit factor and prove the emitted
artifact before publishing it. Neither operation guesses units or scale policy
from geometry, names, or asset categories.

Use the [CLI reference](cli.md#commands) for the exact command grammar and
[machine-readable output](output.md#scale) for the scale-evidence v4 schema.
This guide owns the operator workflow and the boundary between the two
operations.

## Choose the operation

| Operation | Use it when… | Physical result |
|---|---|---|
| `whole-document` | The entire source uses the wrong linear unit. | Every represented length changes by the declared factor. |
| `rest-bind` | World geometry is already correct, but one skinned hierarchy carries a compensating inherited uniform scale. | World joint translations and orientations, sampled trajectories, and skinned geometry are preserved while the hierarchy is reparameterized to remove the composed scale. |

These operations are not interchangeable. A rest/bind cleanup must not be
selected merely because a character appears too large or small, and a unit
conversion must not be used to remove an inherited scale from one hierarchy.

Whole-document conversion takes one finite positive `--factor`. Rest/bind
requires all three declarations: a raw source-skin index, a raw source-root
node index, and the finite positive factor expected at that root. Use
`animsmith measure source.glb` to list authored source-node and source-skin
indices; neither selector is a normalized bone id or mesh-instance ordinal.

```console
animsmith scale whole-document source.glb -o metres.glb \
  --factor 0.01 --evidence metres.scale.json

animsmith scale rest-bind source.glb -o canonical.glb \
  --source-skin-index 0 --source-root-node-index 3 \
  --expected-factor 0.01 --evidence canonical.scale.json
```

There is no in-place mode, plan file, `animsmith.toml` key, implicit first
skin/root, or per-run tolerance override. Input, artifact, and evidence paths
must be three distinct files, and the artifact keeps the input container
extension.

## What one run proves

One invocation performs one ordered transaction:

1. Parse the source and build a complete raw capability inventory.
2. Compile one format-neutral `ScalePlan` containing the exact topology,
   payload shape, field ownership, and proof claims for that source.
3. Apply that plan directly to the source JSON and buffer bytes. The normalized
   model writer is not used for scale output.
4. Reload the exact emitted bytes and run the independent normalized core
   proof plus the raw artifact proof. The artifact proof also reruns the writer
   and requires deterministic bytes.
5. Stage the artifact/evidence pair, read the staged artifact back, and require
   its digest to match the bytes that were proved before promoting both files.

A reported refusal publishes neither file and restores any prior pair. The two
destinations require two renames, so a process killed between them has the
crash window documented in [machine-readable output](output.md#contract-identities).
Proof is not run again after staging: digest equality binds the already-proved
bytes to the published file.

The current fixed tolerance identity is `appendix-d-v6`. It is recorded in
scale evidence together with each evaluated residual and both observed-factor
witness fields. Rest/bind derives those witnesses independently from the raw
source projection and normalized skeleton; whole-document conversion records
the declared factor in both because that operation has no source factor to
measure. There is no runtime alias for an older policy and no per-run tolerance
knob.

## Write sets and preservation

Whole-document conversion rewrites represented lengths: node TRS translations
and matrix translation columns, mesh `POSITION`, translation animation values
and cubic tangents, raw glTF morph-target `POSITION` deltas, inverse-bind
translation columns, and corresponding accessor bounds. Rotation, scale,
normals, UVs, static and animated morph weights, key times, and other
dimensionless payloads remain outside the write set. Static JSON morph weights
retain their numeric values; animated weight accessor payloads remain
byte-exact.

Rest/bind reparameterization derives every multiplier from the selected raw
topology. It rewrites the necessary node-local translation/scale or matrix
components, translation and scale animation payload, and affected inverse
binds. Mesh positions and the already-correct world geometry remain unchanged.

At factor one, whole-document conversion plans no semantic writes. See the
[`scale` output contract](output.md#scale) for the exact artifact-preservation
and evidence-field semantics at that boundary.

## Supported source boundary

Scale currently accepts self-contained glTF/GLB only. A `.gltf` source with an
external buffer or image is refused rather than partially converted. Raw
preflight also refuses any source domain the current model and artifact proof
cannot preserve completely. Whole-document conversion admits only raw glTF
`POSITION` morph-target deltas with dimensionless static/animated weights;
`NORMAL`, `TANGENT`, sparse/interleaved scale-bearing accessors, unsafe aliases,
and every morph payload under rest/bind reparameterization remain refused.
It also refuses cameras and lights, along with GPU instancing, unregistered
extensions, `extras`, non-triangle primitives, secondary skin-influence sets,
unsafe accessor layouts, and animation targeting a matrix-authored node. The
rejection record contains the complete typed violation inventory.

Do not read the core model's static unprojected-connector rows as a current glTF
capability. The glTF loader projects every accepted node today, and the raw
writer has no connector-bridge implementation; that path is outside the
supported source boundary.

FBX scaling is not enabled. The FBX loader now publishes a complete
conservative ufbx-side inventory and a normalized source-skeleton projection
when every declared slot is representable, but that inventory records consumed
coordinate/unit and inheritance semantics, baked (not authored) curves,
rebuilt geometry, and unavailable raw payload-span proof. It therefore keeps
both operations refused rather than discharging an artifact-preservation
boundary that no FBX writer implements. Character assembly likewise does not
silently apply either scale operation. Its existing bind-pose canonicalization
remains a distinct, explicit recipe operation. Morph support intentionally
remains a raw glTF whole-document capability; it does not add morphs to the
shared normalized model or enable rest/bind morph reparameterization.

## Outcomes and evidence

The [CLI reference](cli.md#exit-codes) owns exit-status semantics, and the
[output reference](output.md#scale) owns the scale-evidence v4 wire contract
and publication outcomes. In brief, `--format json` prints the same record that
a successful run writes to `--evidence`; a refused run prints its rejection
record but never writes the evidence destination because there is no
artifact/evidence pair to publish.
The record carries no timestamp; declared paths remain as entered, and
identical inputs and arguments produce byte-identical evidence.

## Embedding the same contract

Rust pipelines use `animsmith_core::scale` for planning and normalized proof,
and `animsmith-gltf` for raw preflight, exact-source rewrite, reload, and
artifact proof. Pass one compiled plan through `rewrite_scale_plan`; do not
substitute the fixture-only analytic reference builder for a format writer.
See the [embedding guide](embedding.md#scale-plan-and-proof-contracts) and the
crate rustdoc for exact type and error contracts. Appendix D in
[DESIGN.md](../DESIGN.md#appendix-d--decision-record-skinned-restbind-scale-canonicalization)
is the normative algebra and preservation decision; the
[calibration notes](scale-calibration.md) record how the fixed proof policy is
measured.
