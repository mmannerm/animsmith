//! Public contract tests for the rest/bind hierarchy reparameterization
//! (DESIGN.md Appendix D §D.2/§D.3, issue #283).
//!
//! Every expectation is a hand-computed literal from the Appendix D §D.3
//! worked cases, or — for the noisy tolerance boundary — derived from
//! [`ScaleTolerancePolicy::APPENDIX_D_V6`]'s own constants at test time. No
//! assertion is stated against a value the code under test produced, and no
//! rewritten value is cross-compared against core's analytic reference
//! candidate: the two routes narrow to
//! `f32` at different points (core narrows the factor *before* multiplying;
//! the raw route keeps `f64` to a single narrowing), so at `s = 0.01` core
//! computes `0.99999998` where this route computes exactly `1.0`. Both
//! satisfy `prove_scale`; neither is evidence about the other.
//!
//! # The §D.3 case 2 rig, and why its numbers are the ones they are
//!
//! A root contributing `0.01`, a joint child authored 100 times too large,
//! and an inverse bind carrying the compensating linear magnitude `100`:
//!
//! ```text
//! node 0 "root"    scale (0.01, 0.01, 0.01)      -> local scale becomes 1
//! node 1 "joint"   translation (0, 100, 0)       -> becomes (0, 1, 0)
//! node 2 "attach"  translation (1, 0, 0)         -> becomes (0.01, 0, 0)
//! node 3 "holder"  mesh 0, skin 0                -> outside the closure
//! skin 0           joints [1], IBM diag(100) with translation (0, -100, 0)
//!                                                -> rigid: diag(1), (0, -1, 0)
//! ```
//!
//! Composed child world position is `(0, 1, 0)` before and after; the mesh is
//! authored in that already-correct world space, so `W * B = I` on both
//! sides. `0.01` is deliberately *not* a power of two — #282's fixtures were,
//! which is why they never met the narrowing divergence above — and every
//! product asserted below is exact in `f64` and again in `f32`:
//! `100 * 0.01 == 1.0`, `300 * 0.01 == 3.0`, `0.01 * (1/0.01) == 1.0`, and
//! `1 * 0.01` is the `f32` whose shortest round-tripping spelling is `0.01`.

use animsmith_core::model::{
    AffineDomainViolation, Document, DocumentShapeError, MeshInstanceShapeViolation,
    TrackShapeViolation,
};
use animsmith_core::scale::{
    ScaleCandidate, ScaleError, ScaleOperation, ScalePlan, ScaleProofObligation, ScaleRequest,
    ScaleTolerancePolicy, plan_scale, prove_scale,
};
use animsmith_gltf::{
    GltfCapabilityViolationKind, GltfScaleArtifact, GltfScalePreflightError, GltfScaleRewriteError,
    GltfScaleSource, capability_facts, load_bytes, preflight_scale_source_bytes,
    prove_rewritten_rest_bind, rewrite_rest_bind, rewrite_scale_plan,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use std::path::Path;

// --- Fixture helpers --------------------------------------------------------

fn bytes(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).expect("literal JSON serializes")
}

fn data_uri(buffer: &[u8]) -> String {
    format!(
        "data:application/octet-stream;base64,{}",
        STANDARD.encode(buffer)
    )
}

fn f32_bytes(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

fn u16_bytes(values: &[u16]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

fn read_f32(slice: &[u8]) -> Vec<f32> {
    slice
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes(chunk.try_into().expect("four bytes")))
        .collect()
}

fn glb(value: &Value, bin: &[u8]) -> Vec<u8> {
    let mut json = bytes(value);
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    let mut binary = bin.to_vec();
    while !binary.len().is_multiple_of(4) {
        binary.push(0);
    }
    let total = 12 + 8 + json.len() + 8 + binary.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend_from_slice(&0x4e4f_534au32.to_le_bytes());
    out.extend_from_slice(&json);
    out.extend_from_slice(&(binary.len() as u32).to_le_bytes());
    out.extend_from_slice(&0x004e_4942u32.to_le_bytes());
    out.extend_from_slice(&binary);
    out
}

fn accepted(name: &str, value: &Value) -> GltfScaleSource {
    preflight_scale_source_bytes(Path::new(name), &bytes(value))
        .unwrap_or_else(|error| panic!("{name} should preflight cleanly: {error:?}"))
}

fn plan_for(source: &GltfScaleSource, factor: f64) -> ScalePlan {
    plan_scale(&ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: factor,
        },
        document: source.document(),
        capability: &capability_facts(source.manifest()),
    })
    .expect("rest/bind plan")
}

/// The artifact's top-level JSON and its resolved buffers, decoded here in
/// the test rather than through any crate helper.
fn artifact_parts(artifact: &GltfScaleArtifact) -> (Value, Vec<Vec<u8>>) {
    let raw = artifact.bytes();
    let (json_bytes, bin): (&[u8], Option<Vec<u8>>) = if raw.starts_with(b"glTF") {
        let json_len = u32::from_le_bytes(raw[12..16].try_into().expect("four bytes")) as usize;
        let bin_start = 20 + json_len;
        let bin_len = u32::from_le_bytes(
            raw[bin_start..bin_start + 4]
                .try_into()
                .expect("four bytes"),
        ) as usize;
        (
            &raw[20..20 + json_len],
            Some(raw[bin_start + 8..bin_start + 8 + bin_len].to_vec()),
        )
    } else {
        (raw, None)
    };
    let value: Value = serde_json::from_slice(json_bytes).expect("artifact JSON parses");
    let mut buffers = Vec::new();
    for (index, buffer) in value["buffers"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .enumerate()
    {
        match buffer.get("uri").and_then(Value::as_str) {
            Some(uri) => {
                let payload = uri.split_once("base64,").expect("base64 data URI").1;
                buffers.push(STANDARD.decode(payload).expect("valid base64"));
            }
            None => {
                assert_eq!(index, 0, "only buffer 0 may come from the GLB BIN chunk");
                buffers.push(bin.clone().expect("GLB BIN chunk"));
            }
        }
    }
    (value, buffers)
}

// --- The §D.3 case 2 rig ----------------------------------------------------

/// Byte offsets inside the rig's single buffer, all four-byte aligned and
/// mutually disjoint. `TRANSLATION` is sized for a `CUBICSPLINE` output's six
/// `VEC3`s so the `LINEAR` and `CUBICSPLINE` rigs share one layout.
mod at {
    pub const POSITION: usize = 0; // 36
    pub const JOINTS: usize = 36; // 24
    pub const WEIGHTS: usize = 60; // 48
    pub const INVERSE_BIND: usize = 108; // 64
    pub const TIMES: usize = 172; // 8
    pub const TRANSLATION: usize = 180; // 72
    pub const ROTATION: usize = 252; // 32
    /// Reached by no `bufferView` in the base rig. Tests that need an extra
    /// sampler input or output declare a view over it; the base rig leaves it
    /// as payload no accessor touches, which is also what makes it a target
    /// for corrupting a byte outside every rewritten range.
    pub const SPARE: usize = 284; // 128
    pub const LENGTH: usize = 412;
}

/// Vertices authored in the already-correct world space of §D.1, so
/// `W(rest) * B = I` and the reparameterization must leave them alone.
const POSITIONS: [f32; 9] = [0.0, 1.0, 0.0, 0.5, 1.25, -0.25, -0.5, 0.75, 0.5];

/// `B = W(rest)^-1` for the compensated joint: column-major `diag(100)` with
/// translation `(0, -100, 0)`. This is §D.1's "inverse-bind matrices carry a
/// compensating linear magnitude of 100".
const INVERSE_BIND: [f32; 16] = [
    100.0, 0.0, 0.0, 0.0, //
    0.0, 100.0, 0.0, 0.0, //
    0.0, 0.0, 100.0, 0.0, //
    0.0, -100.0, 0.0, 1.0,
];

/// The rigid inverse bind §D.3 case 2 requires of the output.
const REBASED_INVERSE_BIND: [f32; 16] = [
    1.0, 0.0, 0.0, 0.0, //
    0.0, 1.0, 0.0, 0.0, //
    0.0, 0.0, 1.0, 0.0, //
    0.0, -1.0, 0.0, 1.0,
];

/// The column-major identity `MAT4`.
const IDENTITY_MAT4: [f32; 16] = [
    1.0, 0.0, 0.0, 0.0, //
    0.0, 1.0, 0.0, 0.0, //
    0.0, 0.0, 1.0, 0.0, //
    0.0, 0.0, 0.0, 1.0,
];

const TIMES: [f32; 2] = [0.0, 1.0];
const TRANSLATIONS: [f32; 6] = [0.0, 100.0, 0.0, 0.0, 300.0, 0.0];
/// `[in-tangent, value, out-tangent]` per key. All eighteen floats are
/// lengths, so all eighteen scale.
const CUBIC_TRANSLATIONS: [f32; 18] = [
    0.0, 0.0, 0.0, 0.0, 100.0, 0.0, 0.0, 25.0, 0.0, //
    0.0, 50.0, 0.0, 0.0, 300.0, 0.0, 0.0, 0.0, 0.0,
];
const ROTATIONS: [f32; 8] = [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];

fn rig_buffer(interpolation: &str) -> Vec<u8> {
    let mut buffer = vec![0u8; at::LENGTH];
    let mut put = |offset: usize, payload: Vec<u8>| {
        buffer[offset..offset + payload.len()].copy_from_slice(&payload);
    };
    put(at::POSITION, f32_bytes(&POSITIONS));
    put(at::JOINTS, u16_bytes(&[0; 12]));
    put(
        at::WEIGHTS,
        f32_bytes(&[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0]),
    );
    put(at::INVERSE_BIND, f32_bytes(&INVERSE_BIND));
    put(at::TIMES, f32_bytes(&TIMES));
    put(at::ROTATION, f32_bytes(&ROTATIONS));
    if interpolation == "CUBICSPLINE" {
        put(at::TRANSLATION, f32_bytes(&CUBIC_TRANSLATIONS));
    } else {
        put(at::TRANSLATION, f32_bytes(&TRANSLATIONS));
    }
    buffer
}

fn rig_json(interpolation: &str, buffer: &[u8]) -> Value {
    let (keys, view_length) = if interpolation == "CUBICSPLINE" {
        (6, 72)
    } else {
        (2, 24)
    };
    json!({
        "asset": { "version": "2.0", "generator": "animsmith test rig" },
        "buffers": [{ "uri": data_uri(buffer), "byteLength": at::LENGTH }],
        "bufferViews": [
            { "buffer": 0, "byteOffset": at::POSITION, "byteLength": 36 },
            { "buffer": 0, "byteOffset": at::JOINTS, "byteLength": 24 },
            { "buffer": 0, "byteOffset": at::WEIGHTS, "byteLength": 48 },
            { "buffer": 0, "byteOffset": at::INVERSE_BIND, "byteLength": 64 },
            { "buffer": 0, "byteOffset": at::TIMES, "byteLength": 8 },
            { "buffer": 0, "byteOffset": at::TRANSLATION, "byteLength": view_length },
            { "buffer": 0, "byteOffset": at::ROTATION, "byteLength": 32 }
        ],
        "accessors": [
            { "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3",
              "min": [-0.5, 0.75, -0.25], "max": [0.5, 1.25, 0.5] },
            { "bufferView": 1, "componentType": 5123, "count": 3, "type": "VEC4" },
            { "bufferView": 2, "componentType": 5126, "count": 3, "type": "VEC4" },
            { "bufferView": 3, "componentType": 5126, "count": 1, "type": "MAT4" },
            { "bufferView": 4, "componentType": 5126, "count": 2, "type": "SCALAR",
              "min": [0.0], "max": [1.0] },
            { "bufferView": 5, "componentType": 5126, "count": keys, "type": "VEC3" },
            { "bufferView": 6, "componentType": 5126, "count": 2, "type": "VEC4" }
        ],
        "materials": [{ "name": "surface", "alphaCutoff": 0.25 }],
        "meshes": [{ "primitives": [{
            "attributes": { "POSITION": 0, "JOINTS_0": 1, "WEIGHTS_0": 2 },
            "material": 0
        }] }],
        "nodes": [
            { "name": "root", "scale": [0.01, 0.01, 0.01], "children": [1] },
            { "name": "joint", "translation": [0.0, 100.0, 0.0], "children": [2] },
            { "name": "attach", "translation": [1.0, 0.0, 0.0] },
            { "name": "holder", "mesh": 0, "skin": 0 }
        ],
        "scenes": [{ "nodes": [0, 3] }],
        "scene": 0,
        "skins": [{ "joints": [1], "skeleton": 0, "inverseBindMatrices": 3 }],
        "animations": [{
            "name": "clip",
            "samplers": [
                { "input": 4, "interpolation": interpolation, "output": 5 },
                { "input": 4, "interpolation": "LINEAR", "output": 6 }
            ],
            "channels": [
                { "sampler": 0, "target": { "node": 1, "path": "translation" } },
                { "sampler": 1, "target": { "node": 1, "path": "rotation" } }
            ]
        }]
    })
}

fn rig(interpolation: &str) -> (Value, Vec<u8>) {
    let buffer = rig_buffer(interpolation);
    let value = rig_json(interpolation, &buffer);
    (value, buffer)
}

#[test]
fn rest_bind_refuses_position_morphs_that_whole_document_can_preserve() {
    let (mut value, _) = rig("LINEAR");
    value["meshes"][0]["primitives"][0]["targets"] = json!([{ "POSITION": 0 }]);
    value["meshes"][0]["weights"] = json!([0.5]);
    value["nodes"][3]["weights"] = json!([0.5]);
    let source = accepted("rest-bind-morph.gltf", &value);
    match rewrite_rest_bind(&source, 0, 0, 0.01) {
        Err(GltfScaleRewriteError::Capability { violations, count }) => {
            assert_eq!(count, violations.len());
            assert!(violations.iter().any(|violation| {
                violation.kind == GltfCapabilityViolationKind::MorphTarget
                    && violation.location == "/meshes/0/primitives/0/targets"
            }));
            assert!(violations.iter().any(|violation| {
                violation.kind == GltfCapabilityViolationKind::MorphWeights
                    && violation.location == "/meshes/0/weights"
            }));
        }
        other => panic!("rest/bind morph source should fail at capability: {other:?}"),
    }
}

#[test]
fn a_rest_bind_plan_from_a_different_primitive_inventory_is_refused_before_raw_binding() {
    let (mut planned_value, mut buffer) = rig("LINEAR");
    buffer[at::SPARE..at::SPARE + 36]
        .copy_from_slice(&f32_bytes(&[0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0]));
    planned_value["buffers"][0]["uri"] = json!(data_uri(&buffer));
    planned_value["bufferViews"]
        .as_array_mut()
        .expect("bufferViews")
        .push(json!({ "buffer": 0, "byteOffset": at::SPARE, "byteLength": 36 }));
    planned_value["accessors"]
        .as_array_mut()
        .expect("accessors")
        .push(json!({ "bufferView": 7, "componentType": 5126, "count": 3, "type": "VEC3" }));
    planned_value["meshes"][0]["primitives"][0]["attributes"]["NORMAL"] = json!(7);
    let planned_source = accepted("planned-rest-normal.gltf", &planned_value);
    let plan = plan_for(&planned_source, 0.01);

    let mut replay_value = planned_value;
    replay_value["meshes"][0]["primitives"][0]["attributes"]
        .as_object_mut()
        .expect("attributes")
        .remove("NORMAL");
    let replay_source = accepted("replay-rest-without-normal.gltf", &replay_value);

    assert!(matches!(
        rewrite_scale_plan(&replay_source, &plan),
        Err(GltfScaleRewriteError::Plan(
            ScaleError::PlanDocumentMismatch {
                reason: "payload_shape_inventory_mismatch"
            }
        ))
    ));
}

#[test]
fn a_replayed_plan_revalidates_numeric_source_requirements_before_raw_binding() {
    let (mut value, mut buffer) = rig("LINEAR");
    let planned_source = accepted("planned-finite.gltf", &value);
    let plan = plan_for(&planned_source, 0.01);

    buffer[at::POSITION..at::POSITION + 4].copy_from_slice(&f32::NAN.to_le_bytes());
    value["buffers"][0]["uri"] = json!(data_uri(&buffer));
    let replay_source = accepted("replay-non-finite-position.gltf", &value);

    assert!(matches!(
        rewrite_scale_plan(&replay_source, &plan),
        Err(GltfScaleRewriteError::Plan(
            ScaleError::InvalidMeshPrimitive {
                mesh_index: 0,
                primitive_index: 0,
                reason: "non_finite_position"
            }
        ))
    ));
}

/// Add one scale channel backed by a new dense `f32 VEC3` accessor.
fn add_scale_track(
    value: &mut Value,
    buffer: &mut [u8],
    offset: usize,
    input_accessor: usize,
    node: usize,
    interpolation: &str,
    values: &[f32],
) -> usize {
    let payload = f32_bytes(values);
    buffer[offset..offset + payload.len()].copy_from_slice(&payload);
    let view_index = value["bufferViews"].as_array().expect("bufferViews").len();
    value["bufferViews"]
        .as_array_mut()
        .expect("bufferViews")
        .push(json!({
            "buffer": 0,
            "byteOffset": offset,
            "byteLength": payload.len()
        }));
    let accessor_index = value["accessors"].as_array().expect("accessors").len();
    value["accessors"]
        .as_array_mut()
        .expect("accessors")
        .push(json!({
            "bufferView": view_index,
            "componentType": 5126,
            "count": values.len() / 3,
            "type": "VEC3"
        }));
    let sampler_index = value["animations"][0]["samplers"]
        .as_array()
        .expect("samplers")
        .len();
    value["animations"][0]["samplers"]
        .as_array_mut()
        .expect("samplers")
        .push(json!({
            "input": input_accessor,
            "interpolation": interpolation,
            "output": accessor_index
        }));
    value["animations"][0]["channels"]
        .as_array_mut()
        .expect("channels")
        .push(json!({
            "sampler": sampler_index,
            "target": { "node": node, "path": "scale" }
        }));
    value["buffers"][0]["uri"] = json!(data_uri(buffer));
    accessor_index
}

/// A rig source, its rewritten artifact at `factor`, and the plan the
/// artifact must prove against.
fn rebased_at(
    name: &str,
    value: &Value,
    factor: f64,
) -> (GltfScaleSource, GltfScaleArtifact, ScalePlan) {
    let source = accepted(name, value);
    let plan = plan_for(&source, factor);
    let artifact = rewrite_rest_bind(&source, 0, 0, factor)
        .unwrap_or_else(|error| panic!("{name} should rebase: {error:?}"));
    (source, artifact, plan)
}

/// A rig source, its rewritten artifact at `s = 0.01`, and the plan the
/// artifact must prove against.
fn rebased(name: &str, value: &Value) -> (GltfScaleSource, GltfScaleArtifact, ScalePlan) {
    rebased_at(name, value, 0.01)
}

/// Every mesh instance's identity, in document order: where it hangs, which
/// source node it came from, which mesh it draws, and which joints it binds.
fn instance_identity(document: &Document) -> Vec<(usize, usize, usize, Vec<usize>)> {
    document
        .assets
        .instances
        .iter()
        .map(|instance| {
            (
                instance.node,
                instance.source_node_index,
                instance.mesh,
                instance.skin_joints.clone(),
            )
        })
        .collect()
}

/// Assert that planning or rewriting `value` fails, and hand back the error.
fn refused(name: &str, value: &Value, factor: f64) -> GltfScaleRewriteError {
    let source = accepted(name, value);
    rewrite_rest_bind(&source, 0, 0, factor)
        .map(|_| ())
        .expect_err(&format!("{name} must be refused"))
}

fn plan_error(error: GltfScaleRewriteError) -> ScaleError {
    match error {
        GltfScaleRewriteError::Plan(error) => error,
        other => panic!("expected a shared-plan rejection, got {other:?}"),
    }
}

#[test]
fn common_preflight_ranges_orphan_sampler_outputs_without_self_conflicting_referenced_ones() {
    let (value, _) = rig("LINEAR");
    accepted("ordinary-referenced-translation.gltf", &value);

    let mut same_index = value.clone();
    same_index["animations"][0]["samplers"]
        .as_array_mut()
        .expect("samplers")
        .push(json!({ "input": 4, "interpolation": "LINEAR", "output": 5 }));
    match preflight_scale_source_bytes(
        Path::new("translation-orphan-same-index.gltf"),
        &bytes(&same_index),
    ) {
        Err(GltfScalePreflightError::Unsupported {
            violations, count, ..
        }) => {
            assert_eq!(count, 1);
            assert_eq!(
                violations,
                vec![animsmith_gltf::GltfCapabilityViolation {
                    kind: GltfCapabilityViolationKind::ConflictingAccessorUse,
                    location: "/accessors/5".to_owned(),
                }]
            );
        }
        other => panic!("expected same-index orphan refusal, got {other:?}"),
    }

    let mut distinct = value;
    distinct["accessors"]
        .as_array_mut()
        .expect("accessors")
        .push(json!({
            "bufferView": 5,
            "componentType": 5126,
            "count": 2,
            "type": "VEC3"
        }));
    distinct["animations"][0]["samplers"]
        .as_array_mut()
        .expect("samplers")
        .push(json!({ "input": 4, "interpolation": "LINEAR", "output": 7 }));
    match preflight_scale_source_bytes(
        Path::new("translation-orphan-overlap.gltf"),
        &bytes(&distinct),
    ) {
        Err(GltfScalePreflightError::Unsupported {
            violations, count, ..
        }) => {
            assert_eq!(count, 2);
            assert_eq!(
                violations
                    .into_iter()
                    .map(|violation| (violation.kind, violation.location))
                    .collect::<Vec<_>>(),
                vec![
                    (
                        GltfCapabilityViolationKind::OverlappingAccessorRanges,
                        "/accessors/5".to_owned()
                    ),
                    (
                        GltfCapabilityViolationKind::OverlappingAccessorRanges,
                        "/accessors/7".to_owned()
                    )
                ]
            );
        }
        other => panic!("expected distinct-range orphan refusal, got {other:?}"),
    }
}

// --- 1: the §D.3 case 2 literals -------------------------------------------

#[test]
fn the_compensated_hierarchy_rebases_to_the_appendix_d3_case_2_literals() {
    let (value, source_buffer) = rig("LINEAR");
    let (_, artifact, _) = rebased("case2.gltf", &value);
    let (json, buffers) = artifact_parts(&artifact);

    // The root's local scale absorbs `1/s`: `0.01 * 100 == 1.0` exactly.
    assert_eq!(json["nodes"][0]["scale"], json!([1.0, 1.0, 1.0]));
    // Every other affected node's local translation takes `s`.
    assert_eq!(json["nodes"][1]["translation"], json!([0.0, 1.0, 0.0]));
    assert_eq!(
        json["nodes"][2]["translation"],
        json!([0.01, 0.0, 0.0]),
        "§D.3 case 2: the transform-only child at (1, 0, 0) rebases to (0.01, 0, 0)"
    );
    // The root's translation is *not* multiplied: its parent is outside the
    // closure, so `s_parent` is one there and only there.
    assert_eq!(
        json["nodes"][0].get("translation"),
        None,
        "the root has no authored translation and none is materialized"
    );
    assert_eq!(
        json["nodes"][1].get("scale"),
        None,
        "an affected non-root node's local scale is unchanged, so no `scale` appears"
    );
    assert_eq!(
        json["nodes"][3], value["nodes"][3],
        "the mesh holder is outside the closure and is untouched"
    );

    // The inverse bind's rows take `s`, leaving it rigid.
    assert_eq!(
        read_f32(&buffers[0][at::INVERSE_BIND..at::INVERSE_BIND + 64]),
        REBASED_INVERSE_BIND.to_vec()
    );
    // Translation animation values take `s`; key times do not.
    assert_eq!(
        read_f32(&buffers[0][at::TRANSLATION..at::TRANSLATION + 24]),
        vec![0.0, 1.0, 0.0, 0.0, 3.0, 0.0]
    );
    assert_eq!(
        &buffers[0][at::TIMES..at::TIMES + 8],
        &source_buffer[at::TIMES..at::TIMES + 8],
        "key times are seconds and stay authored"
    );
    // Mesh geometry and rotation tracks are byte-identical: the world
    // geometry is already correct and rotation is dimensionless.
    assert_eq!(
        &buffers[0][at::POSITION..at::POSITION + 36],
        &source_buffer[at::POSITION..at::POSITION + 36]
    );
    assert_eq!(
        &buffers[0][at::ROTATION..at::ROTATION + 32],
        &source_buffer[at::ROTATION..at::ROTATION + 32]
    );
    assert_eq!(
        json["accessors"][0]["min"],
        json!([-0.5, 0.75, -0.25]),
        "mesh POSITION bounds are unchanged because POSITION is unchanged"
    );

    assert_eq!(
        artifact.rewritten_accessors(),
        [3, 5],
        "exactly the inverse bind and the affected translation output"
    );
    assert_eq!(
        artifact.rewritten_json_pointers(),
        [
            "/nodes/0/scale",
            "/nodes/1/translation",
            "/nodes/2/translation"
        ]
    );
    assert_eq!(artifact.declared_factor(), 0.01);
    assert_eq!(
        artifact.operation(),
        ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 0.01,
        }
    );

    // Affected identity is reported in the raw source-node space the
    // selectors use, not as normalized bone ids: the scaled root, the skin's
    // joint, and the transform-only attachment. Node 3, the mesh holder, is
    // outside the closure and is not named.
    assert_eq!(artifact.affected_source_nodes(), [0, 1, 2]);
    assert_eq!(artifact.affected_source_skins(), [0]);
}

#[test]
fn the_rebased_artifact_proves_and_reports_its_evidence() {
    // Without this the negatives below could all be passing for the wrong
    // reason: a fixture that never proves cannot show which claim caught
    // which corruption.
    let (value, _) = rig("LINEAR");
    let (source, artifact, plan) = rebased("case2-proof.gltf", &value);
    let planned_artifact = rewrite_scale_plan(&source, &plan).expect("planned rewrite");
    assert_eq!(planned_artifact.bytes(), artifact.bytes());
    let proof = prove_rewritten_rest_bind(&source, &artifact, &plan).expect("artifact proof");

    assert_eq!(proof.rewritten_accessor_count, 2);
    // Every §D.3 case 2 product is exact in `f64` and again in `f32`, so the
    // rebase residual is not merely small.
    assert_eq!(proof.length_factor_residual, 0.0);
    assert_eq!(proof.dimensionless_residual, 0.0);
    // 108..172 and 180..204 are rewritten, so the preserved complement is
    // 0..108, 172..180 and 204..284.
    assert_eq!(proof.preserved_byte_ranges, 3);

    let core = proof.core;
    assert_eq!(core.tolerance_policy.id, "appendix-d-v6");
    assert_eq!(core.unit_scale.max(), 0.0);
    assert_eq!(core.transform_only_affine.max(), 0.0);
    assert_eq!(core.skin_matrix.max(), 0.0);
    assert_eq!(core.rest_translation.max(), 0.0);
    assert_eq!(core.trajectory.max(), 0.0);
    assert_eq!(core.mesh_position.max(), 0.0);
    assert_eq!(core.bounds.max(), 0.0);
    // The observed factor is the `f32` the loader read for the root's
    // authored `0.01`; the declared factor is the exact `f64` decimal. §D.6
    // requires evidence to carry both, and they are not the same number.
    assert_eq!(core.observed_factor, f64::from(0.01f32));
    assert_eq!(plan.common_factor(), 0.01);
    assert_ne!(plan.observed_factor(), plan.common_factor());
    // The transform-only obligation is genuinely evidenced here: the closure
    // carries node 2, so the residual above is a checked zero rather than the
    // zero an empty attachment list reports.
    assert_eq!(plan.transform_only_attachments().len(), 1);
    assert!(
        plan.ledger()
            .obligations()
            .any(|obligation| matches!(obligation, ScaleProofObligation::TransformOnlyAffine))
    );
    assert!(
        plan.ledger()
            .obligations()
            .any(|obligation| matches!(obligation, ScaleProofObligation::SkinAndBounds))
    );

    // Mesh-instance *placement* identity, read off the reloaded artifact
    // rather than assumed from the rewriter's shape. The rebase clones the
    // source JSON and patches numbers in place, so `nodes`, `children`,
    // `scenes` and every mesh/skin attachment come through untouched and the
    // loader re-derives the same bone ids from the same DFS. The holder is
    // node 3 on both sides.
    let reloaded = load_bytes(Path::new("case2-proof.gltf"), artifact.bytes()).expect("reload");
    assert_eq!(
        instance_identity(&reloaded),
        instance_identity(source.document())
    );
    assert_eq!(instance_identity(&reloaded), vec![(3, 3, 0, vec![1])]);
}

#[test]
fn rest_bind_preserves_near_zero_joint_rotation_components_exactly() {
    // These are authored f32 values from the downstream rig that exposed
    // issue #399. Without serde_json's correctly rounded parser, serializing
    // and reparsing the first component moves it by one f64 ULP even though
    // the rest/bind writer never selected rotation for rewriting.
    const AUTHORED_ROTATION: &str =
        "[1.7028635168614414e-9,7.312918715030037e-9,-0.10423537343740463,0.9945526719093323]";
    let (mut value, _) = rig("LINEAR");
    value["nodes"][1]["rotation"] = json!("__AUTHORED_ROTATION__");
    let source_bytes = String::from_utf8(bytes(&value))
        .expect("fixture JSON is UTF-8")
        .replace("\"__AUTHORED_ROTATION__\"", AUTHORED_ROTATION)
        .into_bytes();
    let source =
        preflight_scale_source_bytes(Path::new("near-zero-joint-rotation.gltf"), &source_bytes)
            .expect("near-zero joint rotation should preflight");
    let plan = plan_for(&source, 0.01);
    let artifact =
        rewrite_rest_bind(&source, 0, 0, 0.01).expect("near-zero joint rotation should rebase");
    let authored_rotation: Value =
        serde_json::from_str(AUTHORED_ROTATION).expect("authored rotation is valid JSON");

    let (artifact_json, _) = artifact_parts(&artifact);

    assert_eq!(source.raw_json()["nodes"][1]["rotation"], authored_rotation);
    assert_eq!(artifact_json["nodes"][1]["rotation"], authored_rotation);
    prove_rewritten_rest_bind(&source, &artifact, &plan)
        .expect("an unselected authored rotation remains exactly preserved");
}

#[test]
fn a_wide_raw_hierarchy_proves_unaffected_world_rest_by_bone_identity() {
    let mut buffer = rig_buffer("LINEAR");
    let mut value = rig_json("LINEAR", &buffer);
    // Raw node order is deliberately unrelated to the loader's
    // parent-before-child DFS BoneId order:
    //
    // raw 1 -> bone 0 (scaled root), raw 2 -> bone 1 (joint),
    // raw 4 -> bone 2 (attachment), raw 3 -> bone 3 (unrelated root),
    // raw 0 -> bone 4 (independent prop), raw 5 -> bone 5 (holder).
    value["nodes"] = json!([
        { "name": "prop", "translation": [5.0, 0.0, 0.0], "mesh": 0 },
        { "name": "root", "scale": [0.01, 0.01, 0.01], "children": [2] },
        { "name": "joint", "translation": [0.0, 100.0, 0.0], "children": [4] },
        { "name": "unrelated", "children": [0] },
        { "name": "attach", "translation": [1.0, 0.0, 0.0] },
        { "name": "holder", "mesh": 0, "skin": 0 }
    ]);
    value["scenes"][0]["nodes"] = json!([1, 3, 5]);
    value["skins"][0]["joints"] = json!([2]);
    value["skins"][0]["skeleton"] = json!(1);
    value["animations"][0]["channels"][0]["target"]["node"] = json!(2);
    value["animations"][0]["channels"][1]["target"]["node"] = json!(2);
    add_scale_track(
        &mut value,
        &mut buffer,
        at::SPARE,
        4,
        1,
        "LINEAR",
        &[1.0, 1.0, 1.0, 1.0, 1.0, 1.0],
    );

    let source = accepted("wide-bone-order.gltf", &value);
    let prop_bone = source
        .document()
        .assets
        .source_skeleton
        .nodes
        .iter()
        .find(|node| node.source_node_index == 0)
        .and_then(|node| node.bone)
        .expect("the prop normalizes to a bone");
    // Make the raw/BoneId confusion discriminating without pinning the
    // loader's complete DFS order: the prop's BoneId must collide with one of
    // the raw ids in the selected closure, although the prop is not in that
    // closure semantically.
    assert!([1, 2, 4].contains(&prop_bone));

    let plan = plan_scale(&ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 1,
            expected_factor: 0.01,
        },
        document: source.document(),
        capability: &capability_facts(source.manifest()),
    })
    .expect("wide hierarchy plans");
    assert!(!plan.affected_nodes().contains(&prop_bone));
    let artifact = rewrite_rest_bind(&source, 0, 1, 0.01).expect("wide hierarchy rewrites");
    prove_rewritten_rest_bind(&source, &artifact, &plan).expect("the exact emitted reload proves");

    // An implementation that subtracts raw affected node ids `{1,2,4}` from
    // BoneIds would call the independent prop affected and silently skip this
    // 495-unit displacement.
    let mut changed =
        load_bytes(Path::new("wide-bone-order.gltf"), artifact.bytes()).expect("artifact reloads");
    changed.skeleton.bones[prop_bone].rest.translation.x = 500.0;
    assert_eq!(
        prove_scale(
            source.document(),
            &ScaleCandidate::from_document(changed),
            &plan
        )
        .unwrap_err(),
        ScaleError::CandidateStructureMismatch {
            reason: "unaffected_world_rest_mismatch"
        }
    );
}

#[test]
fn the_same_rig_rebases_identically_inside_a_glb() {
    let (mut value, buffer) = rig("LINEAR");
    value["buffers"][0] = json!({ "byteLength": at::LENGTH });
    let source = preflight_scale_source_bytes(Path::new("case2.glb"), &glb(&value, &buffer))
        .expect("the GLB rig preflights cleanly");
    let plan = plan_for(&source, 0.01);
    let artifact = rewrite_rest_bind(&source, 0, 0, 0.01).expect("glb rebase");
    let (json, buffers) = artifact_parts(&artifact);

    assert_eq!(json["nodes"][0]["scale"], json!([1.0, 1.0, 1.0]));
    assert_eq!(json["nodes"][1]["translation"], json!([0.0, 1.0, 0.0]));
    assert_eq!(
        read_f32(&buffers[0][at::INVERSE_BIND..at::INVERSE_BIND + 64]),
        REBASED_INVERSE_BIND.to_vec()
    );
    assert!(
        artifact.reencoded_buffers().is_empty(),
        "a GLB's BIN-chunk buffer is not a data URI and is never re-encoded"
    );
    prove_rewritten_rest_bind(&source, &artifact, &plan).expect("glb artifact proof");
}

/// The inverse bind of the closure root itself, `B = W(rest)^-1 = diag(100)`,
/// and its rebased form `diag(1)`.
const ROOT_INVERSE_BIND: [f32; 16] = [
    100.0, 0.0, 0.0, 0.0, //
    0.0, 100.0, 0.0, 0.0, //
    0.0, 0.0, 100.0, 0.0, //
    0.0, 0.0, 0.0, 1.0,
];

#[test]
fn the_closure_roots_own_inverse_bind_slot_still_takes_the_factor() {
    // The one domain whose multiplier is `s_i` and not `s_parent`. Every
    // other fixture here has a root that is not a joint, so a rewrite that
    // used `s_parent` for inverse binds would pass all of them and silently
    // leave the root's bind unrebased. `B_i' = C_i^-1 * B_i = scale(s_i) * B_i`
    // holds for the root exactly as it does for every other affected joint —
    // and `s_parent` at the root is one, so the two disagree here and only
    // here.
    let (mut value, mut buffer) = rig("LINEAR");
    value["skins"][0]["joints"] = json!([0, 1]);
    // Two `MAT4`s need 128 bytes; the authored 64-byte view cannot hold them,
    // so the accessor moves to the spare region and the old view goes unused.
    buffer[at::SPARE..at::SPARE + 64].copy_from_slice(&f32_bytes(&ROOT_INVERSE_BIND));
    buffer[at::SPARE + 64..at::SPARE + 128].copy_from_slice(&f32_bytes(&INVERSE_BIND));
    value["buffers"][0]["uri"] = json!(data_uri(&buffer));
    value["bufferViews"][3] = json!({ "buffer": 0, "byteOffset": at::SPARE, "byteLength": 128 });
    value["accessors"][3]["count"] = json!(2);
    // Every vertex is weighted onto slot 1, the joint the mesh was authored
    // against, so adding slot 0 changes no skinned position.

    let (source, artifact, plan) = rebased("root-joint.gltf", &value);
    let (_, buffers) = artifact_parts(&artifact);
    // `diag(100) * s` is the identity, and `s_parent` at the root is one — so
    // a rewrite that used `s_parent` here would leave `diag(100)` standing.
    const REBASED_ROOT_INVERSE_BIND: [f32; 16] = [
        1.0, 0.0, 0.0, 0.0, //
        0.0, 1.0, 0.0, 0.0, //
        0.0, 0.0, 1.0, 0.0, //
        0.0, 0.0, 0.0, 1.0,
    ];
    assert_eq!(
        read_f32(&buffers[0][at::SPARE..at::SPARE + 64]),
        REBASED_ROOT_INVERSE_BIND.to_vec(),
        "slot 0 is the root's own bind and takes s just like slot 1"
    );
    assert_eq!(
        read_f32(&buffers[0][at::SPARE + 64..at::SPARE + 128]),
        REBASED_INVERSE_BIND.to_vec()
    );
    prove_rewritten_rest_bind(&source, &artifact, &plan).expect("artifact proof");
}

#[test]
fn a_skin_straddling_the_closure_rebases_per_slot_and_re_derives_its_bounds() {
    // A second skin whose slot 0 is outside the closure and slot 1 inside.
    // Its inverse-bind accessor therefore has no single conversion factor,
    // and an authored `min`/`max` on it has no single conversion either — so
    // the emitted bounds are re-derived from the rebased payload rather than
    // scaled by a factor that applies to only half of it.
    let (mut value, mut buffer) = rig("LINEAR");
    buffer[at::SPARE..at::SPARE + 64].copy_from_slice(&f32_bytes(&IDENTITY_MAT4));
    buffer[at::SPARE + 64..at::SPARE + 128].copy_from_slice(&f32_bytes(&INVERSE_BIND));
    value["buffers"][0]["uri"] = json!(data_uri(&buffer));
    value["bufferViews"]
        .as_array_mut()
        .expect("bufferViews")
        .push(json!({ "buffer": 0, "byteOffset": at::SPARE, "byteLength": 128 }));
    value["accessors"]
        .as_array_mut()
        .expect("accessors")
        .push(json!({
            "bufferView": 7, "componentType": 5126, "count": 2, "type": "MAT4",
            "min": vec![-1000.0; 16], "max": vec![1000.0; 16]
        }));
    value["skins"]
        .as_array_mut()
        .expect("skins")
        .push(json!({ "joints": [3, 1], "inverseBindMatrices": 7 }));

    let (source, artifact, plan) = rebased("straddling-skin.gltf", &value);
    let (json, buffers) = artifact_parts(&artifact);
    let mut expected = Vec::from(IDENTITY_MAT4);
    expected.extend_from_slice(&REBASED_INVERSE_BIND);
    assert_eq!(
        read_f32(&buffers[0][at::SPARE..at::SPARE + 128]),
        expected,
        "slot 0, outside the closure, is byte-identical and slot 1 takes s"
    );
    // Per-component extrema of those two matrices, with the homogeneous row
    // (components 3, 7, 11 and 15) left at its authored bound because this
    // rule does not touch it.
    assert_eq!(
        json["accessors"][7]["min"],
        json!([
            1.0, 0.0, 0.0, -1000.0, //
            0.0, 1.0, 0.0, -1000.0, //
            0.0, 0.0, 1.0, -1000.0, //
            0.0, -1.0, 0.0, -1000.0
        ])
    );
    assert_eq!(
        json["accessors"][7]["max"],
        json!([
            1.0, 0.0, 0.0, 1000.0, //
            0.0, 1.0, 0.0, 1000.0, //
            0.0, 0.0, 1.0, 1000.0, //
            0.0, 0.0, 0.0, 1000.0
        ])
    );
    assert_eq!(artifact.rewritten_accessors(), [3, 5, 7]);
    assert!(
        artifact
            .rewritten_json_pointers()
            .contains(&"/accessors/7/min".to_owned())
    );
    assert_eq!(
        artifact.affected_source_skins(),
        [0, 1],
        "a skin straddling the boundary is affected even when its first joint is outside"
    );
    prove_rewritten_rest_bind(&source, &artifact, &plan).expect("artifact proof");
}

#[test]
fn a_skin_slot_outside_the_closure_keeps_its_bind_byte_identical() {
    // The mirror: a second skin whose joint is the mesh holder, outside the
    // closure, must come through untouched. Its multiplier is one, so its
    // accessor is not rewritten at all.
    let (mut value, mut buffer) = rig("LINEAR");
    buffer[at::SPARE..at::SPARE + 64].copy_from_slice(&f32_bytes(&INVERSE_BIND));
    value["buffers"][0]["uri"] = json!(data_uri(&buffer));
    value["bufferViews"]
        .as_array_mut()
        .expect("bufferViews")
        .push(json!({ "buffer": 0, "byteOffset": at::SPARE, "byteLength": 64 }));
    value["accessors"]
        .as_array_mut()
        .expect("accessors")
        .push(json!({ "bufferView": 7, "componentType": 5126, "count": 1, "type": "MAT4" }));
    value["skins"]
        .as_array_mut()
        .expect("skins")
        .push(json!({ "joints": [3], "inverseBindMatrices": 7 }));

    let (source, artifact, plan) = rebased("outside-skin.gltf", &value);
    let (_, buffers) = artifact_parts(&artifact);
    assert_eq!(
        read_f32(&buffers[0][at::SPARE..at::SPARE + 64]),
        INVERSE_BIND.to_vec()
    );
    assert_eq!(
        artifact.rewritten_accessors(),
        [3, 5],
        "accessor 7 is not in the rewrite set"
    );
    assert_eq!(
        artifact.affected_source_skins(),
        [0],
        "skin 1's only joint is the mesh holder, outside the closure"
    );
    let proof = prove_rewritten_rest_bind(&source, &artifact, &plan).expect("artifact proof");
    assert_eq!(proof.core.unaffected_inverse_bind.max(), 0.0);
}

#[test]
fn the_closure_roots_own_translation_track_is_left_byte_identical() {
    // `s_parent` is one at the root, so its translation *animation* is
    // untouched even though every other affected node's takes `s`. This is
    // the domain split that distinguishes `s_parent` from `s_i`: the root's
    // inverse-bind slot would still take `s_i` if it were a joint, and a
    // rewrite that used one where it needs the other passes every fixture
    // whose root is neither animated nor a joint.
    let (mut value, mut buffer) = rig("LINEAR");
    buffer[at::SPARE..at::SPARE + 24].copy_from_slice(&f32_bytes(&[7.0, 0.0, 0.0, 9.0, 0.0, 0.0]));
    value["buffers"][0]["uri"] = json!(data_uri(&buffer));
    value["bufferViews"]
        .as_array_mut()
        .expect("bufferViews")
        .push(json!({ "buffer": 0, "byteOffset": at::SPARE, "byteLength": 24 }));
    value["accessors"]
        .as_array_mut()
        .expect("accessors")
        .push(json!({ "bufferView": 7, "componentType": 5126, "count": 2, "type": "VEC3" }));
    value["animations"][0]["samplers"]
        .as_array_mut()
        .expect("samplers")
        .push(json!({ "input": 4, "interpolation": "LINEAR", "output": 7 }));
    value["animations"][0]["channels"]
        .as_array_mut()
        .expect("channels")
        .push(json!({ "sampler": 2, "target": { "node": 0, "path": "translation" } }));

    let (source, artifact, plan) = rebased("root-track.gltf", &value);
    let (_, buffers) = artifact_parts(&artifact);
    assert_eq!(
        read_f32(&buffers[0][at::SPARE..at::SPARE + 24]),
        vec![7.0, 0.0, 0.0, 9.0, 0.0, 0.0],
        "the closure root's translation values are in its parent's basis already"
    );
    assert_eq!(
        read_f32(&buffers[0][at::TRANSLATION..at::TRANSLATION + 24]),
        vec![0.0, 1.0, 0.0, 0.0, 3.0, 0.0],
        "while the joint's take s"
    );
    assert_eq!(
        artifact.rewritten_accessors(),
        [3, 5],
        "accessor 7 is left out of the rewrite set entirely"
    );
    prove_rewritten_rest_bind(&source, &artifact, &plan).expect("artifact proof");
}

#[test]
fn every_cubicspline_translation_element_rebases_including_both_tangents() {
    let (value, _) = rig("CUBICSPLINE");
    let (source, artifact, plan) = rebased("cubic.gltf", &value);
    let (_, buffers) = artifact_parts(&artifact);
    assert_eq!(
        read_f32(&buffers[0][at::TRANSLATION..at::TRANSLATION + 72]),
        vec![
            0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.25, 0.0, //
            0.0, 0.5, 0.0, 0.0, 3.0, 0.0, 0.0, 0.0, 0.0
        ],
        "a CUBICSPLINE output stores [in, value, out] per key and all three are lengths"
    );
    prove_rewritten_rest_bind(&source, &artifact, &plan).expect("cubic artifact proof");
}

#[test]
fn a_matrix_authored_hierarchy_rebases_its_linear_part_at_the_root_and_its_translation_below() {
    // The same rig with every affected node authored as a column-major
    // `matrix`. §D.2's `L' = C_parent^-1 * L * C_i` splits differently for
    // the two: the root's `s_parent` is one, so only its linear part moves;
    // every other node's `s_parent / s_i` is one, so only its translation
    // column moves.
    let (mut value, _) = rig("LINEAR");
    value["nodes"][0] = json!({
        "name": "root",
        "matrix": [0.01, 0.0, 0.0, 0.0, 0.0, 0.01, 0.0, 0.0, 0.0, 0.0, 0.01, 0.0, 0.0, 0.0, 0.0, 1.0],
        "children": [1]
    });
    value["nodes"][1] = json!({
        "name": "joint",
        // This first component narrows to exactly 1.0 in the frontend but its
        // parsed JSON value must survive without that extra f32 narrowing
        // because the joint's linear factor group is identity.
        "matrix": [1.0000000000000002, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 100.0, 0.0, 1.0],
        "children": [2]
    });
    // Animated nodes must author TRS. Keep this case about matrix-authored
    // *rest* transforms by targeting the TRS descendant instead.
    value["animations"][0]["channels"][0]["target"]["node"] = json!(2);
    value["animations"][0]["channels"][1]["target"]["node"] = json!(2);
    let (source, artifact, plan) = rebased("matrix.gltf", &value);
    let (json, _) = artifact_parts(&artifact);

    assert_eq!(
        json["nodes"][0]["matrix"],
        json!([
            1.0, 0.0, 0.0, 0.0, //
            0.0, 1.0, 0.0, 0.0, //
            0.0, 0.0, 1.0, 0.0, //
            0.0, 0.0, 0.0, 1.0
        ]),
        "the root's linear part takes 1/s; its translation column and homogeneous row do not"
    );
    assert_eq!(
        json["nodes"][1]["matrix"],
        json!([
            1.0000000000000002,
            0.0,
            0.0,
            0.0, //
            0.0,
            1.0,
            0.0,
            0.0, //
            0.0,
            0.0,
            1.0,
            0.0, //
            0.0,
            1.0,
            0.0,
            1.0
        ]),
        "a non-root node's translation column takes s; its linear part is unchanged"
    );
    assert_eq!(
        artifact.rewritten_json_pointers(),
        ["/nodes/0/matrix", "/nodes/1/matrix", "/nodes/2/translation"]
    );
    prove_rewritten_rest_bind(&source, &artifact, &plan).expect("matrix artifact proof");
}

#[test]
fn an_affected_non_root_node_declaring_no_translation_gains_none() {
    // Every other fixture here declares a `translation` on every affected
    // non-root node, so `collect_node_rebases`'s `declared(node,
    // "translation")` test is never the reason a rebase is skipped. Dropping
    // it emits a `Translation` rebase for this node; `rewrite_node_member`
    // then writes nothing — glTF's default `(0, 0, 0)` is fixed under
    // multiplication and only `scale` is materialized — while the artifact
    // still *reports* `/nodes/4/translation` as rewritten.
    let (mut value, _) = rig("LINEAR");
    value["nodes"][2]["children"] = json!([4]);
    value["nodes"]
        .as_array_mut()
        .expect("nodes")
        .push(json!({ "name": "bare" }));

    let (source, artifact, plan) = rebased("bare-node.gltf", &value);
    let (json, _) = artifact_parts(&artifact);
    assert_eq!(
        json["nodes"][4],
        json!({ "name": "bare" }),
        "an affected node with no authored transform members gains none"
    );
    assert_eq!(
        artifact.rewritten_json_pointers(),
        [
            "/nodes/0/scale",
            "/nodes/1/translation",
            "/nodes/2/translation"
        ],
        "and no pointer is reported for a member the node does not declare"
    );
    prove_rewritten_rest_bind(&source, &artifact, &plan).expect("artifact proof");
}

#[test]
fn a_loose_authored_bound_on_a_rebased_output_is_converted_not_re_derived() {
    // Every other fixture's authored bound *equals* the observed extremum of
    // its payload, which makes "a rewritten bound still bounds the rewritten
    // payload" vacuous: both directions of the comparison hold. Here the
    // authored bounds are deliberately looser than the data — the translation
    // values span `(0, 100, 0)..(0, 300, 0)` — so the emitted bound is the
    // authored one times `s` and is *strictly* outside the payload's extrema
    // in both directions.
    let (mut value, _) = rig("LINEAR");
    value["accessors"][5]["min"] = json!([-10.0, 50.0, -10.0]);
    value["accessors"][5]["max"] = json!([10.0, 400.0, 10.0]);

    let (source, artifact, plan) = rebased("loose-bounds.gltf", &value);
    let (json, _) = artifact_parts(&artifact);
    // The authored bound times `s`, exact in `f64` and again in `f32` — not
    // the rebased payload's own extrema, which are `(0, 1, 0)` and
    // `(0, 3, 0)`. A single shared factor converts an authored bound; only a
    // claim with mixed per-slot factors re-derives one.
    assert_eq!(json["accessors"][5]["min"], json!([-0.1, 0.5, -0.1]));
    assert_eq!(json["accessors"][5]["max"], json!([0.1, 4.0, 0.1]));
    prove_rewritten_rest_bind(&source, &artifact, &plan).expect("artifact proof");
}

#[test]
fn the_skin_selector_decides_which_skins_joints_the_closure_is_built_from() {
    // Every other fixture here has one skin, at index 0, so resolving
    // `skins[0]` regardless of the declared selector is indistinguishable
    // from honouring it. Here skin 0's only joint is the mesh holder — a
    // sibling scene root with no ancestor path to the declared root — so a
    // closure built from skin 0 cannot be derived at all, while skin 1 is the
    // §D.3 case 2 chain. This is also what makes "both selectors are required
    // raw source identity" evidenced rather than merely stated.
    let (mut value, mut buffer) = rig("LINEAR");
    buffer[at::SPARE..at::SPARE + 64].copy_from_slice(&f32_bytes(&IDENTITY_MAT4));
    value["buffers"][0]["uri"] = json!(data_uri(&buffer));
    value["bufferViews"]
        .as_array_mut()
        .expect("bufferViews")
        .push(json!({ "buffer": 0, "byteOffset": at::SPARE, "byteLength": 64 }));
    value["accessors"]
        .as_array_mut()
        .expect("accessors")
        .push(json!({ "bufferView": 7, "componentType": 5126, "count": 1, "type": "MAT4" }));
    value["skins"] = json!([
        { "joints": [3], "inverseBindMatrices": 7 },
        { "joints": [1], "skeleton": 0, "inverseBindMatrices": 3 }
    ]);
    value["nodes"][3]["skin"] = json!(1);

    let source = accepted("second-skin.gltf", &value);
    let plan = plan_scale(&ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 1,
            source_root_node_index: 0,
            expected_factor: 0.01,
        },
        document: source.document(),
        capability: &capability_facts(source.manifest()),
    })
    .expect("plan");
    let artifact = rewrite_rest_bind(&source, 1, 0, 0.01).expect("skin 1 rebases");
    let (json, buffers) = artifact_parts(&artifact);

    assert_eq!(json["nodes"][0]["scale"], json!([1.0, 1.0, 1.0]));
    assert_eq!(json["nodes"][1]["translation"], json!([0.0, 1.0, 0.0]));
    assert_eq!(
        read_f32(&buffers[0][at::INVERSE_BIND..at::INVERSE_BIND + 64]),
        REBASED_INVERSE_BIND.to_vec(),
        "skin 1's inverse bind takes s"
    );
    assert_eq!(
        read_f32(&buffers[0][at::SPARE..at::SPARE + 64]),
        IDENTITY_MAT4.to_vec(),
        "skin 0's joint is outside the closure, so its bind is byte-identical"
    );
    assert_eq!(
        artifact.rewritten_accessors(),
        [3, 5],
        "accessor 7 is skin 0's, whose only slot is unaffected"
    );
    prove_rewritten_rest_bind(&source, &artifact, &plan).expect("artifact proof");
}

#[test]
fn an_absent_root_scale_is_materialized_rather_than_left_at_the_gltf_default() {
    // A closure root that inherits the compensating factor from an ancestor
    // *outside* the closure declares no `scale` of its own. glTF's default is
    // `[1, 1, 1]`, so the output must declare `[1/s, 1/s, 1/s]` — the one
    // member this operation adds, and the one case where leaving the source's
    // JSON alone silently produces a document with the factor still in it.
    let (mut value, _) = rig("LINEAR");
    value["nodes"] = json!([
        { "name": "grandparent", "scale": [0.01, 0.01, 0.01], "children": [1] },
        { "name": "root", "children": [2] },
        { "name": "joint", "translation": [0.0, 100.0, 0.0], "children": [3] },
        { "name": "attach", "translation": [1.0, 0.0, 0.0] },
        { "name": "holder", "mesh": 0, "skin": 0 }
    ]);
    value["scenes"] = json!([{ "nodes": [0, 4] }]);
    value["skins"][0] = json!({ "joints": [2], "skeleton": 1, "inverseBindMatrices": 3 });
    value["animations"][0]["channels"] = json!([
        { "sampler": 0, "target": { "node": 2, "path": "translation" } },
        { "sampler": 1, "target": { "node": 2, "path": "rotation" } }
    ]);

    let source = accepted("absent-scale.gltf", &value);
    // The scaled root is node 1: the grandparent carries the factor but stays
    // outside the closure, since no joint's ancestor path reaches it.
    let plan = plan_scale(&ScaleRequest {
        operation: ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 1,
            expected_factor: 0.01,
        },
        document: source.document(),
        capability: &capability_facts(source.manifest()),
    })
    .expect("plan");
    let artifact = rewrite_rest_bind(&source, 0, 1, 0.01).expect("rebase");
    let (json, _) = artifact_parts(&artifact);
    assert_eq!(
        json["nodes"][1]["scale"],
        json!([100.0, 100.0, 100.0]),
        "the glTF default [1, 1, 1] times 1/s"
    );
    assert_eq!(json["nodes"][2]["translation"], json!([0.0, 1.0, 0.0]));
    assert_eq!(json["nodes"][3]["translation"], json!([0.01, 0.0, 0.0]));
    assert_eq!(
        json["nodes"][0], value["nodes"][0],
        "the ancestor carrying the factor is outside the closure and untouched"
    );
    assert_eq!(
        artifact.rewritten_json_pointers(),
        [
            "/nodes/1/scale",
            "/nodes/2/translation",
            "/nodes/3/translation"
        ]
    );
    prove_rewritten_rest_bind(&source, &artifact, &plan).expect("artifact proof");
}

// --- 2: §D.3 case 1, the unit skeleton -------------------------------------

/// §D.3 case 1: a root and child both at unit scale, the child rest
/// translation `(0, 1, 0)`, and a rigid inverse bind.
fn unit_rig() -> (Value, Vec<u8>) {
    let (mut value, mut buffer) = rig("LINEAR");
    value["nodes"][0] = json!({ "name": "root", "children": [1] });
    value["nodes"][1] = json!({ "name": "joint", "translation": [0.0, 1.0, 0.0], "children": [2] });
    buffer[at::INVERSE_BIND..at::INVERSE_BIND + 64]
        .copy_from_slice(&f32_bytes(&REBASED_INVERSE_BIND));
    buffer[at::TRANSLATION..at::TRANSLATION + 24]
        .copy_from_slice(&f32_bytes(&[0.0, 1.0, 0.0, 0.0, 3.0, 0.0]));
    value["buffers"][0]["uri"] = json!(data_uri(&buffer));
    (value, buffer)
}

#[test]
fn a_declared_factor_of_one_is_a_deterministic_no_op() {
    let (value, buffer) = unit_rig();
    let source = accepted("unit.gltf", &value);
    let plan = plan_for(&source, 1.0);
    let artifact = rewrite_rest_bind(&source, 0, 0, 1.0).expect("a unit skeleton rebases by one");
    let (json, buffers) = artifact_parts(&artifact);

    assert!(
        artifact.rewritten_accessors().is_empty(),
        "every multiplier is exactly one, so no accessor is selected"
    );
    assert!(
        artifact.rewritten_json_pointers().is_empty(),
        "no node member is selected either — not even a materialized `scale`"
    );
    assert_eq!(
        json, value,
        "the artifact's JSON tree is value-identical to the source's"
    );
    assert_eq!(buffers[0], buffer, "and every buffer byte is unchanged");
    prove_rewritten_rest_bind(&source, &artifact, &plan).expect("the no-op still proves");
}

#[test]
fn requesting_any_other_factor_on_a_unit_skeleton_is_a_source_fact_mismatch() {
    let (value, _) = unit_rig();
    match plan_error(refused("unit-mismatch.gltf", &value, 0.01)) {
        ScaleError::FactorMismatch { expected, observed } => {
            assert_eq!(expected, 0.01);
            assert_eq!(observed, 1.0, "the unit skeleton's measured factor");
        }
        other => panic!("expected FactorMismatch, got {other:?}"),
    }
    // And the mirror: the compensated rig refuses a declared factor of one.
    let (value, _) = rig("LINEAR");
    match plan_error(refused("case2-mismatch.gltf", &value, 1.0)) {
        ScaleError::FactorMismatch { expected, observed } => {
            assert_eq!(expected, 1.0);
            assert_eq!(observed, f64::from(0.01f32));
        }
        other => panic!("expected FactorMismatch, got {other:?}"),
    }
}

// --- 3: the aliasing refusal ------------------------------------------------
//
// The four shapes of DESIGN.md Appendix D that #280 accepts and this
// operation cannot satisfy. Each is paired with the mirror case that must
// still convert, because every fail-closed check in this lane has an
// over-rejection risk of exactly the same shape.

/// Add a second joint chain under the root so two affected nodes can share
/// one translation output.
fn two_child_rig() -> (Value, Vec<u8>) {
    let (mut value, buffer) = rig("LINEAR");
    value["nodes"][0]["children"] = json!([1, 4]);
    value["nodes"]
        .as_array_mut()
        .expect("nodes")
        .push(json!({ "name": "sibling", "translation": [0.0, 100.0, 0.0] }));
    (value, buffer)
}

/// Declare a three-key sampler input over the rig's spare buffer region, as
/// `bufferView 7` / `accessor 7`.
///
/// A sampler reading the three-vertex `POSITION` accessor as its output needs
/// a three-key input, and reusing the rig's two-key `TIMES` accessor by
/// widening it in place would overlap the translation output — which #280
/// refuses as `OverlappingAccessorRanges` long before the sharing under test
/// is reached.
fn with_three_key_times(value: &mut Value, mut buffer: Vec<u8>) -> (Value, Vec<u8>) {
    buffer[at::SPARE..at::SPARE + 12].copy_from_slice(&f32_bytes(&[0.0, 0.5, 1.0]));
    value["bufferViews"]
        .as_array_mut()
        .expect("bufferViews")
        .push(json!({ "buffer": 0, "byteOffset": at::SPARE, "byteLength": 12 }));
    value["accessors"]
        .as_array_mut()
        .expect("accessors")
        .push(json!({
            "bufferView": 7, "componentType": 5126, "count": 3, "type": "SCALAR",
            "min": [0.0], "max": [1.0]
        }));
    (value.clone(), buffer)
}

fn conflict(name: &str, value: &Value) -> (usize, usize, String, f64, String, f64) {
    match refused(name, value, 0.01) {
        GltfScaleRewriteError::ConflictingRestBindFactor {
            accessor_index,
            element,
            first_location,
            first_factor,
            second_location,
            second_factor,
        } => (
            accessor_index,
            element,
            first_location,
            first_factor,
            second_location,
            second_factor,
        ),
        other => panic!("{name}: expected ConflictingRestBindFactor, got {other:?}"),
    }
}

#[test]
fn one_translation_output_shared_by_the_root_and_a_child_is_refused() {
    // The root's multiplier is one and the child's is `s`, so no single
    // rewrite of accessor 5 satisfies both. #280 accepts this: both uses are
    // scale-bearing, and its guard only fires on the scale-bearing /
    // dimensionless cross.
    let (mut value, _) = rig("LINEAR");
    value["animations"][0]["channels"]
        .as_array_mut()
        .expect("channels")
        .push(json!({ "sampler": 0, "target": { "node": 0, "path": "translation" } }));
    let (accessor, element, first, first_factor, second, second_factor) =
        conflict("root-and-child.gltf", &value);
    assert_eq!(accessor, 5);
    assert_eq!(element, 0);
    assert_eq!(first, "/animations/0/channels/0");
    assert_eq!(first_factor, 0.01);
    assert_eq!(second, "/animations/0/channels/2");
    assert_eq!(second_factor, 1.0);
}

#[test]
fn one_translation_output_shared_with_a_node_outside_the_closure_is_refused() {
    // Node 3 is the mesh holder, outside the closure, so its translation
    // values must come through byte-identical while the joint's take `s`.
    let (mut value, _) = rig("LINEAR");
    value["animations"][0]["channels"]
        .as_array_mut()
        .expect("channels")
        .push(json!({ "sampler": 0, "target": { "node": 3, "path": "translation" } }));
    let (accessor, _, first, first_factor, second, second_factor) =
        conflict("inside-and-outside.gltf", &value);
    assert_eq!(accessor, 5);
    assert_eq!(first, "/animations/0/channels/0");
    assert_eq!(first_factor, 0.01);
    assert_eq!(second, "/animations/0/channels/2");
    assert_eq!(second_factor, 1.0);
}

#[test]
fn one_accessor_used_as_mesh_position_and_as_a_translation_output_is_refused() {
    // Mesh `POSITION` must stay byte-identical and the affected translation
    // output must take `s`. #282's justification for keying by accessor index
    // rested on type-disjointness, and this is where that half collapses:
    // both are `VEC3`.
    let (mut value, buffer) = rig("LINEAR");
    // Point the translation sampler at the `POSITION` accessor. It holds
    // three `VEC3`s, so the sampler needs a three-key input of its own.
    let (mut value, buffer) = with_three_key_times(&mut value, buffer);
    value["animations"][0]["samplers"][0] =
        json!({ "input": 7, "interpolation": "STEP", "output": 0 });
    value["buffers"][0]["uri"] = json!(data_uri(&buffer));

    let (accessor, _, first, first_factor, second, second_factor) =
        conflict("position-and-output.gltf", &value);
    assert_eq!(accessor, 0);
    assert_eq!(first, "/meshes/0/primitives/0/attributes/POSITION");
    assert_eq!(first_factor, 1.0);
    assert_eq!(second, "/animations/0/channels/0");
    assert_eq!(second_factor, 0.01);
}

#[test]
fn one_inverse_bind_accessor_shared_by_two_skins_with_different_slots_is_refused() {
    // Skin 0's joint is inside the closure and skin 1's is not, so slot 0's
    // factor is `s` for one skin and one for the other.
    let (mut value, _) = rig("LINEAR");
    value["skins"]
        .as_array_mut()
        .expect("skins")
        .push(json!({ "joints": [3], "inverseBindMatrices": 3 }));
    let (accessor, element, first, first_factor, second, second_factor) =
        conflict("shared-ibm.gltf", &value);
    assert_eq!(accessor, 3);
    assert_eq!(element, 0, "the two skins disagree at slot 0");
    assert_eq!(first, "/skins/0/inverseBindMatrices");
    assert_eq!(first_factor, 0.01);
    assert_eq!(second, "/skins/1/inverseBindMatrices");
    assert_eq!(second_factor, 1.0);
}

#[test]
fn a_root_scale_output_shared_with_a_normal_is_refused() {
    let (mut value, mut buffer) = rig("LINEAR");
    let accessor = add_scale_track(
        &mut value,
        &mut buffer,
        at::SPARE,
        4,
        0,
        "LINEAR",
        &[1.0, 1.0, 1.0, 1.0, 1.0, 1.0],
    );
    value["meshes"][0]["primitives"][0]["attributes"]["NORMAL"] = json!(accessor);
    let (actual, element, first, first_factor, second, second_factor) =
        conflict("root-scale-normal.gltf", &value);
    assert_eq!(actual, accessor);
    assert_eq!(element, 0);
    assert_eq!(first, "/meshes/0/primitives/0/attributes/NORMAL");
    assert_eq!(first_factor, 1.0);
    assert_eq!(second, "/animations/0/channels/2");
    assert_eq!(second_factor, 100.0);
}

#[test]
fn a_descendant_scale_output_shared_with_a_normal_is_preserved_and_proves() {
    let (mut value, buffer) = rig("LINEAR");
    let (mut value, mut buffer) = with_three_key_times(&mut value, buffer);
    let accessor = add_scale_track(
        &mut value,
        &mut buffer,
        at::SPARE + 16,
        7,
        1,
        "STEP",
        &[1.0, 1.0, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0],
    );
    value["meshes"][0]["primitives"][0]["attributes"]["NORMAL"] = json!(accessor);
    let authored = buffer[at::SPARE + 16..at::SPARE + 52].to_vec();
    let (source, artifact, plan) = rebased("descendant-scale-normal.gltf", &value);
    let (_, buffers) = artifact_parts(&artifact);
    assert_eq!(&buffers[0][at::SPARE + 16..at::SPARE + 52], &authored);
    assert!(!artifact.rewritten_accessors().contains(&accessor));
    prove_rewritten_rest_bind(&source, &artifact, &plan).expect("compatible alias proves");
}

#[test]
fn one_scale_output_shared_by_the_root_and_a_descendant_is_refused() {
    // Both uses are scale channels, so the common scale-bearing/dimensionless
    // preflight cannot distinguish them. The rest/bind ledger must still see
    // the root's `1 / s` beside the strict descendant's factor one.
    let (mut value, mut buffer) = rig("LINEAR");
    let accessor = add_scale_track(
        &mut value,
        &mut buffer,
        at::SPARE,
        4,
        0,
        "LINEAR",
        &[1.0, 1.0, 1.0, 2.0, 2.0, 2.0],
    );
    value["animations"][0]["channels"]
        .as_array_mut()
        .expect("channels")
        .push(json!({ "sampler": 2, "target": { "node": 1, "path": "scale" } }));

    let (actual, element, first, first_factor, second, second_factor) =
        conflict("root-descendant-shared-scale.gltf", &value);
    assert_eq!(actual, accessor);
    assert_eq!(element, 0);
    assert_eq!(first, "/animations/0/channels/2");
    assert_eq!(first_factor, 100.0);
    assert_eq!(second, "/animations/0/channels/3");
    assert_eq!(second_factor, 1.0);
}

#[test]
fn two_descendants_may_share_one_scale_output_at_factor_one() {
    let (mut value, mut buffer) = rig("LINEAR");
    let authored = [1.0, 2.0, 3.0, 1.5, 2.5, 3.5];
    let accessor = add_scale_track(&mut value, &mut buffer, at::SPARE, 4, 1, "STEP", &authored);
    value["animations"][0]["channels"]
        .as_array_mut()
        .expect("channels")
        .push(json!({ "sampler": 2, "target": { "node": 2, "path": "scale" } }));

    let (source, artifact, plan) = rebased("descendants-shared-scale.gltf", &value);
    let (_, buffers) = artifact_parts(&artifact);
    assert_eq!(
        read_f32(&buffers[0][at::SPARE..at::SPARE + 24]),
        authored,
        "the shared factor-one output stays byte-identical"
    );
    assert!(!artifact.rewritten_accessors().contains(&accessor));
    prove_rewritten_rest_bind(&source, &artifact, &plan).expect("compatible scale sharing proves");
}

#[test]
fn a_root_scale_output_shared_with_an_orphan_sampler_output_is_refused() {
    let (mut value, mut buffer) = rig("LINEAR");
    let accessor = add_scale_track(
        &mut value,
        &mut buffer,
        at::SPARE,
        4,
        0,
        "LINEAR",
        &[1.0, 1.0, 1.0, 2.0, 2.0, 2.0],
    );
    value["animations"][0]["samplers"]
        .as_array_mut()
        .expect("samplers")
        .push(json!({ "input": 4, "interpolation": "LINEAR", "output": accessor }));
    let (actual, _, first, first_factor, second, second_factor) =
        conflict("root-scale-orphan-output.gltf", &value);
    assert_eq!(actual, accessor);
    assert_eq!(first, "/animations/0/samplers/3/output");
    assert_eq!(first_factor, 1.0);
    assert_eq!(second, "/animations/0/channels/2");
    assert_eq!(second_factor, 100.0);
}

#[test]
fn distinct_overlapping_root_scale_and_normal_accessors_are_refused_by_the_operation() {
    let (mut value, buffer) = rig("LINEAR");
    let (mut value, mut buffer) = with_three_key_times(&mut value, buffer);
    let scale_accessor = add_scale_track(
        &mut value,
        &mut buffer,
        at::SPARE + 16,
        7,
        0,
        "STEP",
        &[1.0, 1.0, 1.0, 1.5, 1.5, 1.5, 2.0, 2.0, 2.0],
    );
    let normal_accessor = value["accessors"].as_array().expect("accessors").len();
    let shared_view = value["accessors"][scale_accessor]["bufferView"].clone();
    value["accessors"]
        .as_array_mut()
        .expect("accessors")
        .push(json!({
            "bufferView": shared_view,
            "componentType": 5126,
            "count": 3,
            "type": "VEC3"
        }));
    value["meshes"][0]["primitives"][0]["attributes"]["NORMAL"] = json!(normal_accessor);

    let source = accepted("overlapping-root-scale-normal.gltf", &value);
    match rewrite_rest_bind(&source, 0, 0, 0.01) {
        Err(GltfScaleRewriteError::Capability { violations, count }) => {
            assert_eq!(count, 2);
            assert_eq!(
                violations
                    .into_iter()
                    .map(|violation| (violation.kind, violation.location))
                    .collect::<Vec<_>>(),
                vec![
                    (
                        GltfCapabilityViolationKind::OverlappingAccessorRanges,
                        format!("/accessors/{scale_accessor}")
                    ),
                    (
                        GltfCapabilityViolationKind::OverlappingAccessorRanges,
                        format!("/accessors/{normal_accessor}")
                    )
                ]
            );
        }
        other => panic!("expected operation-specific overlap refusal, got {other:?}"),
    }
}

#[test]
fn distinct_non_overlapping_root_scale_and_normal_accessors_still_rebase() {
    let (mut value, buffer) = rig("LINEAR");
    let (mut value, mut buffer) = with_three_key_times(&mut value, buffer);
    let normal_accessor = value["accessors"].as_array().expect("accessors").len();
    value["bufferViews"]
        .as_array_mut()
        .expect("bufferViews")
        .push(json!({ "buffer": 0, "byteOffset": at::SPARE + 16, "byteLength": 36 }));
    value["accessors"]
        .as_array_mut()
        .expect("accessors")
        .push(json!({
            "bufferView": 8,
            "componentType": 5126,
            "count": 3,
            "type": "VEC3"
        }));
    value["meshes"][0]["primitives"][0]["attributes"]["NORMAL"] = json!(normal_accessor);
    let scale_accessor = add_scale_track(
        &mut value,
        &mut buffer,
        at::SPARE + 64,
        7,
        0,
        "STEP",
        &[1.0, 1.0, 1.0, 1.5, 1.5, 1.5, 2.0, 2.0, 2.0],
    );
    let (source, artifact, plan) = rebased("disjoint-root-scale-normal.gltf", &value);
    assert!(artifact.rewritten_accessors().contains(&scale_accessor));
    prove_rewritten_rest_bind(&source, &artifact, &plan).expect("disjoint control proves");
}

#[test]
fn two_affected_nodes_sharing_one_translation_output_still_rebase_once() {
    // The must-not-over-reject mirror of the first two refusals: two channels
    // targeting two *different* affected non-root nodes agree on `s`, so the
    // accessor is rewritten exactly once. A rewrite driven by logical uses
    // rather than by unique accessor index would apply `s` twice.
    let (mut value, _) = two_child_rig();
    value["animations"][0]["channels"]
        .as_array_mut()
        .expect("channels")
        .push(json!({ "sampler": 0, "target": { "node": 4, "path": "translation" } }));
    let (source, artifact, plan) = rebased("shared-output.gltf", &value);
    let (_, buffers) = artifact_parts(&artifact);
    assert_eq!(
        read_f32(&buffers[0][at::TRANSLATION..at::TRANSLATION + 24]),
        vec![0.0, 1.0, 0.0, 0.0, 3.0, 0.0],
        "s applied once, not s^2"
    );
    assert_eq!(artifact.rewritten_accessors(), [3, 5]);
    prove_rewritten_rest_bind(&source, &artifact, &plan).expect("artifact proof");
}

#[test]
fn a_position_shared_with_an_unaffected_nodes_translation_output_still_rebases() {
    // Both uses claim a factor of one, so they agree and the accessor is left
    // byte-identical. A guard that refused every shared scale-bearing
    // accessor outright would reject this valid source.
    let (mut value, buffer) = rig("LINEAR");
    let (mut value, buffer) = with_three_key_times(&mut value, buffer);
    value["buffers"][0]["uri"] = json!(data_uri(&buffer));
    // A second animation whose translation channel targets the holder — a
    // node outside the closure — reading the mesh `POSITION` accessor.
    value["animations"]
        .as_array_mut()
        .expect("animations")
        .push(json!({
            "samplers": [{ "input": 7, "interpolation": "STEP", "output": 0 }],
            "channels": [{ "sampler": 0, "target": { "node": 3, "path": "translation" } }]
        }));

    let source = accepted("agreeing-share.gltf", &value);
    let plan = plan_for(&source, 0.01);
    let artifact = rewrite_rest_bind(&source, 0, 0, 0.01).expect("agreeing uses convert");
    let (_, buffers) = artifact_parts(&artifact);
    assert_eq!(
        &buffers[0][at::POSITION..at::POSITION + 36],
        &f32_bytes(&POSITIONS)[..],
        "an accessor every claimant leaves at factor one is byte-identical"
    );
    assert_eq!(artifact.rewritten_accessors(), [3, 5]);
    prove_rewritten_rest_bind(&source, &artifact, &plan).expect("artifact proof");
}

#[test]
fn two_skins_with_the_same_joints_may_share_one_inverse_bind_accessor() {
    let (mut value, _) = rig("LINEAR");
    value["skins"]
        .as_array_mut()
        .expect("skins")
        .push(json!({ "joints": [1], "inverseBindMatrices": 3 }));
    let (source, artifact, plan) = rebased("agreeing-ibm.gltf", &value);
    let (_, buffers) = artifact_parts(&artifact);
    assert_eq!(
        read_f32(&buffers[0][at::INVERSE_BIND..at::INVERSE_BIND + 64]),
        REBASED_INVERSE_BIND.to_vec(),
        "s applied once, not s^2"
    );
    prove_rewritten_rest_bind(&source, &artifact, &plan).expect("artifact proof");
}

// --- 4: §D.3 case 4, rejected affines and the rest of the refusal list ------

fn affine_violation(name: &str, value: &Value) -> AffineDomainViolation {
    match plan_error(refused(name, value, 0.01)) {
        ScaleError::InvalidAffineDomain { node, reason } => {
            assert_eq!(node, 0, "{name}: the scaled root is bone 0");
            reason
        }
        other => panic!("{name}: expected InvalidAffineDomain, got {other:?}"),
    }
}

#[test]
fn a_non_uniform_root_scale_is_refused() {
    let (mut value, _) = rig("LINEAR");
    value["nodes"][0]["scale"] = json!([0.01, 0.02, 0.01]);
    assert_eq!(
        affine_violation("non-uniform.gltf", &value),
        AffineDomainViolation::NonUniformScale
    );
}

#[test]
fn a_sheared_root_matrix_is_refused() {
    let (mut value, _) = rig("LINEAR");
    value["nodes"][0] = json!({
        "name": "root",
        // Column 1 is column 0 rotated by a 3-4-5 triangle into the x/y
        // plane: `|(0.006, 0.008, 0)| == 0.01` exactly, so the axes stay
        // equal-length and only the orthogonality check can catch it. Their
        // dot product is `6e-5`, four orders of magnitude above the
        // `relative_orthogonality * average^2 == 1e-9` threshold.
        "matrix": [0.01, 0.0, 0.0, 0.0, 0.006, 0.008, 0.0, 0.0, 0.0, 0.0, 0.01, 0.0, 0.0, 0.0, 0.0, 1.0],
        "children": [1]
    });
    assert_eq!(
        affine_violation("shear.gltf", &value),
        AffineDomainViolation::Sheared
    );
}

#[test]
fn an_animation_targeting_a_matrix_node_is_rejected_with_raw_identity() {
    let (mut value, mut buffer) = rig("LINEAR");
    value["nodes"][3] = json!({
        "name": "holder",
        "mesh": 0,
        "skin": 0,
        "matrix": [
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 1.0
        ]
    });
    // Put raw node 3 first in scene traversal. It would normalize as BoneId
    // 0, so the diagnostic can only pass by retaining the authored raw node
    // identity rather than accidentally reporting a normalized bone index.
    value["scenes"][0]["nodes"] = json!([3, 0]);
    add_scale_track(
        &mut value,
        &mut buffer,
        at::SPARE,
        4,
        3,
        "LINEAR",
        &[1.0, 1.0, 1.0, 2.0, 2.0, 2.0],
    );
    match preflight_scale_source_bytes(Path::new("animated-matrix.gltf"), &bytes(&value)) {
        Err(GltfScalePreflightError::Unsupported {
            manifest,
            violations,
            count,
        }) => {
            assert_eq!(count, 1);
            assert_eq!(violations.len(), 1);
            assert_eq!(
                violations[0].kind,
                GltfCapabilityViolationKind::AnimatedMatrixNode
            );
            assert_eq!(violations[0].location, "/animations/0/channels/2/target");
            let channel = &manifest.animation_channels[2];
            assert_eq!(channel.animation_index, 0);
            assert_eq!(channel.channel_index, 2);
            assert_eq!(channel.target_node_index, 3);
            assert_eq!(channel.target_path, "scale");
            assert_eq!(channel.input_accessor_index, 4);
            assert_eq!(channel.output_accessor_index, 7);
        }
        other => panic!("expected animated-matrix preflight refusal, got {other:?}"),
    }
}

#[test]
fn a_reflected_root_scale_is_refused() {
    let (mut value, _) = rig("LINEAR");
    value["nodes"][0]["scale"] = json!([-0.01, 0.01, 0.01]);
    assert_eq!(
        affine_violation("reflected.gltf", &value),
        AffineDomainViolation::Reflected
    );
}

#[test]
fn a_singular_root_scale_is_refused() {
    let (mut value, _) = rig("LINEAR");
    value["nodes"][0]["scale"] = json!([0.01, 0.01, 0.0]);
    assert_eq!(
        affine_violation("singular.gltf", &value),
        AffineDomainViolation::Singular
    );
}

#[test]
fn a_joint_with_its_own_extra_scale_is_a_mixed_factor() {
    let (mut value, _) = rig("LINEAR");
    value["nodes"][2]["scale"] = json!([2.0, 2.0, 2.0]);
    match plan_error(refused("mixed.gltf", &value, 0.01)) {
        ScaleError::MixedFactor {
            expected,
            observed,
            node,
        } => {
            assert_eq!(node, 2);
            assert_eq!(expected, f64::from(0.01f32));
            assert_eq!(observed, f64::from(0.02f32));
        }
        other => panic!("expected MixedFactor, got {other:?}"),
    }
}

#[test]
fn a_root_linear_scale_track_absorbs_the_inverse_factor() {
    let (mut value, mut buffer) = rig("LINEAR");
    let accessor = add_scale_track(
        &mut value,
        &mut buffer,
        at::SPARE,
        4,
        0,
        "LINEAR",
        &[1.0, 1.0, 1.0, 1.0, 1.0, 1.0],
    );
    let (source, artifact, plan) = rebased("root-scale-linear.gltf", &value);
    let (_, buffers) = artifact_parts(&artifact);
    assert_eq!(
        read_f32(&buffers[0][at::SPARE..at::SPARE + 24]),
        vec![100.0, 100.0, 100.0, 100.0, 100.0, 100.0]
    );
    assert!(artifact.rewritten_accessors().contains(&accessor));
    prove_rewritten_rest_bind(&source, &artifact, &plan).expect("root scale proves");
}

#[test]
fn every_root_scale_interpolation_uses_the_f64_reciprocal_then_one_f32_narrowing() {
    let authored = [
        0.7, -1.3, 2.9, 1.1, -2.3, 3.7, 0.2, -0.4, 0.8, -1.7, 2.1, -3.3, 4.1, -5.3, 6.7, -0.9, 1.9,
        -2.7,
    ];
    let multiplier = 1.0f64 / 0.03f64;
    assert_ne!(multiplier, f64::from(1.0f32 / 0.03f32));
    for (interpolation, value_count) in [("LINEAR", 6), ("STEP", 6), ("CUBICSPLINE", 18)] {
        let (mut value, mut buffer) = rig("LINEAR");
        value["nodes"][0]["scale"] = json!([0.03, 0.03, 0.03]);
        let authored = &authored[..value_count];
        add_scale_track(
            &mut value,
            &mut buffer,
            at::SPARE,
            4,
            0,
            interpolation,
            authored,
        );
        let name = format!("root-scale-f64-{interpolation}.gltf");
        let (source, artifact, plan) = rebased_at(&name, &value, 0.03);
        let (_, buffers) = artifact_parts(&artifact);
        let expected = authored
            .iter()
            .map(|&value| (f64::from(value) * multiplier) as f32)
            .collect::<Vec<_>>();
        assert_eq!(
            read_f32(&buffers[0][at::SPARE..at::SPARE + value_count * 4]),
            expected,
            "{interpolation} must use the same single-narrowing arithmetic"
        );
        prove_rewritten_rest_bind(&source, &artifact, &plan).unwrap_or_else(|error| {
            panic!("non-dyadic {interpolation} root scale proves: {error}")
        });
    }
}

#[test]
fn a_root_step_scale_track_absorbs_the_inverse_factor() {
    let (mut value, mut buffer) = rig("LINEAR");
    add_scale_track(
        &mut value,
        &mut buffer,
        at::SPARE,
        4,
        0,
        "STEP",
        &[1.0, 1.0, 1.0, 3.0, 3.0, 3.0],
    );
    let (source, artifact, plan) = rebased("root-scale-step.gltf", &value);
    let (_, buffers) = artifact_parts(&artifact);
    assert_eq!(
        read_f32(&buffers[0][at::SPARE..at::SPARE + 24]),
        vec![100.0, 100.0, 100.0, 300.0, 300.0, 300.0]
    );
    prove_rewritten_rest_bind(&source, &artifact, &plan).expect("root STEP scale proves");
}

#[test]
fn a_root_cubic_scale_track_rebases_values_and_both_tangents() {
    let (mut value, mut buffer) = rig("LINEAR");
    let authored: Vec<f32> = (1..=18).map(|value| value as f32).collect();
    add_scale_track(
        &mut value,
        &mut buffer,
        at::SPARE,
        4,
        0,
        "CUBICSPLINE",
        &authored,
    );
    let (source, artifact, plan) = rebased("root-scale-cubic.gltf", &value);
    let (_, buffers) = artifact_parts(&artifact);
    let expected: Vec<f32> = (1..=18).map(|value| value as f32 * 100.0).collect();
    assert_eq!(
        read_f32(&buffers[0][at::SPARE..at::SPARE + 72]),
        expected,
        "all six VEC3 elements, including both tangents per key, take 1/s"
    );
    prove_rewritten_rest_bind(&source, &artifact, &plan).expect("root cubic scale proves");
}

#[test]
fn descendant_and_outside_scale_tracks_are_byte_exact() {
    for (location, node) in [("descendant", 1usize), ("outside", 3)] {
        for (interpolation, authored) in [
            (
                "LINEAR",
                (1..=6).map(|value| value as f32 * 1.25).collect::<Vec<_>>(),
            ),
            (
                "STEP",
                (1..=6).map(|value| value as f32 * 1.5).collect::<Vec<_>>(),
            ),
            (
                "CUBICSPLINE",
                (1..=18)
                    .map(|value| value as f32 * 1.75)
                    .collect::<Vec<_>>(),
            ),
        ] {
            let (mut value, mut buffer) = rig("LINEAR");
            add_scale_track(
                &mut value,
                &mut buffer,
                at::SPARE,
                4,
                node,
                interpolation,
                &authored,
            );
            let name = format!("{location}-scale-{interpolation}.gltf");
            let (source, artifact, plan) = rebased(&name, &value);
            let (_, buffers) = artifact_parts(&artifact);
            let byte_len = authored.len() * 4;
            assert_eq!(
                &buffers[0][at::SPARE..at::SPARE + byte_len],
                &f32_bytes(&authored),
                "{location} {interpolation} scale has multiplier one"
            );
            prove_rewritten_rest_bind(&source, &artifact, &plan).expect("identity scale proves");
        }
    }
}

#[test]
fn a_unit_root_scale_track_is_byte_exact() {
    for (interpolation, authored) in [
        (
            "LINEAR",
            (1..=6).map(|value| value as f32 * 1.25).collect::<Vec<_>>(),
        ),
        (
            "STEP",
            (1..=6).map(|value| value as f32 * 1.5).collect::<Vec<_>>(),
        ),
        (
            "CUBICSPLINE",
            (1..=18)
                .map(|value| value as f32 * 1.75)
                .collect::<Vec<_>>(),
        ),
    ] {
        let (mut value, mut buffer) = unit_rig();
        add_scale_track(
            &mut value,
            &mut buffer,
            at::SPARE,
            4,
            0,
            interpolation,
            &authored,
        );
        let name = format!("unit-root-scale-{interpolation}.gltf");
        let source = accepted(&name, &value);
        let plan = plan_for(&source, 1.0);
        let artifact = rewrite_rest_bind(&source, 0, 0, 1.0).expect("unit scale rewrites");
        let (_, buffers) = artifact_parts(&artifact);
        let byte_len = authored.len() * 4;
        assert_eq!(
            &buffers[0][at::SPARE..at::SPARE + byte_len],
            &f32_bytes(&authored),
            "root {interpolation} scale is byte-identical at factor one"
        );
        prove_rewritten_rest_bind(&source, &artifact, &plan).expect("unit scale proves");
    }
}

#[test]
fn unskinned_geometry_inside_the_closure_is_refused() {
    let (mut value, _) = rig("LINEAR");
    value["nodes"][1]["children"] = json!([2, 4]);
    value["nodes"]
        .as_array_mut()
        .expect("nodes")
        .push(json!({ "name": "prop", "mesh": 0 }));
    match plan_error(refused("unskinned.gltf", &value, 0.01)) {
        // A `BoneId`, not the source node index: bone order follows scene
        // traversal (roots `[0, 3]`, depth first), so source node 4 — the
        // second child of node 1 — normalizes to bone 3 while source node 3
        // becomes bone 4. That divergence is exactly why both selectors this
        // operation takes are raw source indices.
        ScaleError::UnsupportedUnskinnedGeometry { node } => assert_eq!(node, 3),
        other => panic!("expected UnsupportedUnskinnedGeometry, got {other:?}"),
    }
}

#[test]
fn a_non_finite_inverse_bind_is_refused() {
    // `AffineDomainViolation::NonFinite` is unreachable from a glTF node
    // transform, because `classify_affine` reads only the authored node
    // transform and JSON has no `NaN` or `Infinity` literal — `serde_json`
    // refuses to parse one, which
    // `json_cannot_express_a_non_finite_node_transform` pins. The two
    // reachable non-finite paths both live in binary accessors, and both are
    // caught by the shared document-shape validation rather than by affine
    // classification.
    let (mut value, mut buffer) = rig("LINEAR");
    buffer[at::INVERSE_BIND..at::INVERSE_BIND + 4].copy_from_slice(&f32::NAN.to_le_bytes());
    value["buffers"][0]["uri"] = json!(data_uri(&buffer));
    match plan_error(refused("nan-ibm.gltf", &value, 0.01)) {
        ScaleError::InvalidDocumentShape(DocumentShapeError::MeshInstanceShape {
            instance_index,
            violation,
        }) => {
            assert_eq!(instance_index, 0);
            assert_eq!(
                violation,
                MeshInstanceShapeViolation::NonFiniteSkinInverseBind
            );
        }
        other => panic!("expected InvalidDocumentShape, got {other:?}"),
    }
}

#[test]
fn a_non_finite_translation_value_is_refused() {
    let (mut value, mut buffer) = rig("LINEAR");
    buffer[at::TRANSLATION + 4..at::TRANSLATION + 8].copy_from_slice(&f32::INFINITY.to_le_bytes());
    value["buffers"][0]["uri"] = json!(data_uri(&buffer));
    match plan_error(refused("inf-value.gltf", &value, 0.01)) {
        ScaleError::InvalidDocumentShape(DocumentShapeError::TrackShape {
            clip_index,
            node,
            violation,
        }) => {
            assert_eq!(clip_index, 0);
            assert_eq!(node, 1);
            assert_eq!(violation, TrackShapeViolation::NonFiniteValue);
        }
        other => panic!("expected InvalidDocumentShape, got {other:?}"),
    }
}

#[test]
fn json_cannot_express_a_non_finite_node_transform() {
    // The evidence for the unreachability argument above, stated as a fact
    // about the format rather than as an absence of test coverage. Both
    // spellings a producer might reach for are parse errors, so no glTF
    // source can carry a `NaN` node scale into `classify_affine`.
    for spelling in ["NaN", "Infinity", "-Infinity", "nan", "inf"] {
        let text = format!(r#"{{ "scale": [{spelling}, 1.0, 1.0] }}"#);
        serde_json::from_str::<Value>(&text)
            .expect_err(&format!("`{spelling}` must not parse as a JSON number"));
    }
}

#[test]
fn an_incomplete_capability_manifest_is_refused_before_any_rewrite() {
    let (mut value, _) = rig("LINEAR");
    value["cameras"] =
        json!([{ "type": "perspective", "perspective": { "yfov": 1.0, "znear": 0.1 } }]);
    match preflight_scale_source_bytes(Path::new("camera.gltf"), &bytes(&value)) {
        Err(GltfScalePreflightError::Unsupported {
            violations, count, ..
        }) => {
            assert_eq!(count, violations.len());
            assert!(
                violations
                    .iter()
                    .any(|violation| violation.kind == GltfCapabilityViolationKind::Camera)
            );
        }
        other => panic!("expected a typed capability rejection, got {other:?}"),
    }
}

#[test]
fn both_selectors_are_required_source_identity_and_a_bad_one_rejects() {
    let (value, _) = rig("LINEAR");
    let source = accepted("selectors.gltf", &value);
    match plan_error(
        rewrite_rest_bind(&source, 0, 9, 0.01)
            .map(|_| ())
            .expect_err("node 9 is not a source node"),
    ) {
        ScaleError::InvalidRootSelector {
            source_root_node_index,
        } => assert_eq!(source_root_node_index, 9),
        other => panic!("expected InvalidRootSelector, got {other:?}"),
    }
    match plan_error(
        rewrite_rest_bind(&source, 7, 0, 0.01)
            .map(|_| ())
            .expect_err("skin 7 is not a source skin"),
    ) {
        ScaleError::InvalidSkinSelector { source_skin_index } => {
            assert_eq!(source_skin_index, 7);
        }
        other => panic!("expected InvalidSkinSelector, got {other:?}"),
    }
    for factor in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        match plan_error(
            rewrite_rest_bind(&source, 0, 0, factor)
                .map(|_| ())
                .expect_err("a non-positive or non-finite factor is refused"),
        ) {
            ScaleError::InvalidExpectedFactor { .. } => {}
            other => panic!("expected InvalidExpectedFactor for {factor}, got {other:?}"),
        }
    }
}

// --- 5: the noisy-but-valid tolerance boundary ------------------------------

#[test]
fn a_noisy_but_in_band_hierarchy_is_accepted_and_its_residual_is_reported() {
    // Derived from the policy's own constants rather than hard-coded, so a
    // policy revision moves the fixture with it instead of silently turning
    // it into a boundary test of a band that no longer exists.
    let policy = ScaleTolerancePolicy::APPENDIX_D_V6;
    let declared = 0.01_f64;
    // For axis lengths `(a, a, a(1 + e))` the average is `a(1 + e/3)`, so the
    // largest per-axis deviation is the long axis's `2ae/3` measured against
    // `equal_axis * a(1 + e)`. The band therefore admits `e` up to about
    // `1.5 * equal_axis`; one band is inside it with room, and still four
    // orders of magnitude above any `f32` rounding artefact at this
    // magnitude.
    let skew = policy.equal_axis;
    let noisy_z = declared * (1.0 + skew);
    // The observed common factor is the average, `declared * (1 + skew/3)`,
    // whose relative distance from the declared factor is `skew/3` — inside
    // `common_factor` by a factor of three.
    assert!(
        2.0 * skew / 3.0 < policy.equal_axis && skew / 3.0 < policy.common_factor,
        "the fixture stays inside both input bands"
    );

    let (mut value, mut buffer) = rig("LINEAR");
    value["nodes"][0]["scale"] = json!([declared, declared, noisy_z]);
    // The inverse bind is the exact inverse of the noisy rest world, so the
    // skin equation holds on the source side to begin with.
    buffer[at::INVERSE_BIND..at::INVERSE_BIND + 64].copy_from_slice(&f32_bytes(&[
        (1.0 / declared) as f32,
        0.0,
        0.0,
        0.0, //
        0.0,
        (1.0 / declared) as f32,
        0.0,
        0.0, //
        0.0,
        0.0,
        (1.0 / noisy_z) as f32,
        0.0, //
        0.0,
        -100.0,
        0.0,
        1.0,
    ]));
    value["buffers"][0]["uri"] = json!(data_uri(&buffer));

    let source = accepted("noisy.gltf", &value);
    let plan = plan_for(&source, declared);
    let artifact = rewrite_rest_bind(&source, 0, 0, declared).expect("a noisy in-band rig rebases");
    let proof = prove_rewritten_rest_bind(&source, &artifact, &plan).expect("artifact proof");
    // Accepted for a reviewable reason: the residual is recorded and it is
    // inside the derived postcondition bound, not merely non-zero-and-small.
    assert!(
        proof.core.unit_scale.max() <= policy.postcondition_unit_scale_residual,
        "unit-scale residual {} exceeds the derived bound {}",
        proof.core.unit_scale.max(),
        policy.postcondition_unit_scale_residual
    );
    assert!(
        proof.core.unit_scale.max() > 0.0,
        "a noisy fixture whose residual is exactly zero is not testing the band"
    );
    assert!(plan.observed_factor() != plan.common_factor());
}

#[test]
fn a_hierarchy_just_outside_the_equal_axis_band_is_refused() {
    // The over-acceptance mirror of the test above, derived from the same
    // constant: four bands of skew is unambiguously outside the roughly
    // three-band admissible window.
    let policy = ScaleTolerancePolicy::APPENDIX_D_V6;
    let declared = 0.01_f64;
    let (mut value, _) = rig("LINEAR");
    value["nodes"][0]["scale"] = json!([
        declared,
        declared,
        declared * (1.0 + 4.0 * policy.equal_axis)
    ]);
    assert_eq!(
        affine_violation("noisy-out-of-band.gltf", &value),
        AffineDomainViolation::NonUniformScale
    );
}

#[test]
fn a_declared_factor_outside_the_common_factor_band_is_refused() {
    let policy = ScaleTolerancePolicy::APPENDIX_D_V6;
    let (value, _) = rig("LINEAR");
    // The source's measured factor is `f32(0.01)`; declare one two bands
    // away from it.
    let observed = f64::from(0.01f32);
    let declared = observed * (1.0 + 2.0 * policy.common_factor);
    match plan_error(refused("off-band-factor.gltf", &value, declared)) {
        ScaleError::FactorMismatch {
            expected,
            observed: measured,
        } => {
            assert_eq!(expected, declared);
            assert_eq!(measured, observed);
        }
        other => panic!("expected FactorMismatch, got {other:?}"),
    }
}
