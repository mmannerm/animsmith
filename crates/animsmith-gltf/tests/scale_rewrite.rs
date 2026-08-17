//! Public contract tests for the whole-document linear-unit rewrite.
//!
//! Every expectation below is a hand-computed literal. The conversion factors
//! are powers of two wherever an exact `f32` expectation is asserted, so the
//! literals are the arithmetic truth rather than a rounding of it, and no
//! assertion is derived from a value the code under test produced.

use animsmith_core::model::Document;
use animsmith_core::scale::{
    ScaleCandidate, ScaleError, ScaleOperation, ScalePlan, ScaleRequest, plan_scale, prove_scale,
};
use animsmith_gltf::{
    GltfCapabilityViolationKind, GltfScaleArtifact, GltfScalePreflightError, GltfScaleRewriteError,
    GltfScaleSource, capability_facts, load_bytes, preflight_scale_source_bytes,
    prove_rewritten_artifact, rewrite_linear_units, rewrite_scale_plan,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use std::path::Path;

// --- Fixture helpers -------------------------------------------------------

fn bytes(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).expect("literal JSON serializes")
}

fn data_uri(buffer: &[u8]) -> String {
    format!(
        "data:application/octet-stream;base64,{}",
        STANDARD.encode(buffer)
    )
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

fn accepted(name: &str, value: &Value) -> GltfScaleSource {
    preflight_scale_source_bytes(Path::new(name), &bytes(value))
        .unwrap_or_else(|error| panic!("{name} should preflight cleanly: {error:?}"))
}

fn accepted_glb(name: &str, value: &Value, bin: &[u8]) -> GltfScaleSource {
    preflight_scale_source_bytes(Path::new(name), &glb(value, bin))
        .unwrap_or_else(|error| panic!("{name} should preflight cleanly: {error:?}"))
}

fn rejected(name: &str, value: &Value) -> Vec<GltfCapabilityViolation> {
    match preflight_scale_source_bytes(Path::new(name), &bytes(value)) {
        Err(GltfScalePreflightError::Unsupported {
            violations,
            manifest,
            count,
        }) => {
            assert_eq!(count, violations.len());
            // The same manifest, projected for planning, must also refuse:
            // the capability boundary and the format-neutral projection
            // cannot disagree about whether a source is convertible.
            assert!(
                !capability_facts(&manifest).is_supported(),
                "{name}: preflight rejected but capability_facts reported the source supported"
            );
            violations
        }
        other => panic!("{name}: expected a typed capability rejection, got {other:?}"),
    }
}

use animsmith_gltf::GltfCapabilityViolation;

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

fn kinds(violations: &[GltfCapabilityViolation]) -> Vec<GltfCapabilityViolationKind> {
    let mut kinds: Vec<_> = violations.iter().map(|violation| violation.kind).collect();
    kinds.sort();
    kinds.dedup();
    kinds
}

/// The artifact's top-level JSON and its resolved buffers, decoded here in
/// the test rather than through any crate helper.
fn artifact_parts(artifact: &GltfScaleArtifact) -> (Value, Vec<Vec<u8>>) {
    let raw = artifact.bytes();
    let (json_bytes, bin): (&[u8], Option<Vec<u8>>) = if raw.starts_with(b"glTF") {
        let json_len = u32::from_le_bytes(raw[12..16].try_into().expect("four bytes")) as usize;
        let json = &raw[20..20 + json_len];
        let bin_start = 20 + json_len;
        let bin_len = u32::from_le_bytes(
            raw[bin_start..bin_start + 4]
                .try_into()
                .expect("four bytes"),
        ) as usize;
        (
            json,
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

fn plan_for(source: &GltfScaleSource, factor: f64) -> ScalePlan {
    plan_scale(&ScaleRequest {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor },
        document: source.document(),
        capability: &capability_facts(source.manifest()),
    })
    .expect("whole-document plan")
}

// --- Fixtures ---------------------------------------------------------------

/// A minimal accepted document: one `POSITION` accessor with the `min`/`max`
/// glTF requires on a `POSITION` attribute.
fn minimal_json(positions: &[f32]) -> (Value, Vec<u8>) {
    let buffer = f32_bytes(positions);
    let length = buffer.len();
    let component = |offset: usize, fold: fn(f32, f32) -> f32, seed: f32| {
        positions
            .iter()
            .skip(offset)
            .step_by(3)
            .copied()
            .fold(seed, fold)
    };
    let min: Vec<f32> = (0..3)
        .map(|offset| component(offset, f32::min, f32::INFINITY))
        .collect();
    let max: Vec<f32> = (0..3)
        .map(|offset| component(offset, f32::max, f32::NEG_INFINITY))
        .collect();
    (
        json!({
            "asset": { "version": "2.0" },
            "buffers": [{ "uri": data_uri(&buffer), "byteLength": length }],
            "bufferViews": [{ "buffer": 0, "byteLength": length }],
            "accessors": [{
                "bufferView": 0,
                "componentType": 5126,
                "count": positions.len() / 3,
                "type": "VEC3",
                "min": min,
                "max": max
            }],
            "meshes": [{ "primitives": [{ "attributes": { "POSITION": 0 } }] }]
        }),
        buffer,
    )
}

const MORPH_BASE: [f32; 9] = [1.0, 2.0, 3.0, -1.0, 0.5, 4.0, 2.0, -2.0, 0.0];
const MORPH_A: [f32; 9] = [0.25, 0.0, -0.5, 1.0, -1.0, 2.0, 0.0, 0.5, 0.25];
const MORPH_B: [f32; 9] = [-0.5, 1.0, 0.0, 0.25, 0.25, -0.25, 2.0, 0.0, 1.0];
const MORPH_TIMES: [f32; 2] = [0.0, 1.0];
const MORPH_WEIGHTS: [f32; 4] = [0.25, 0.75, 0.5, 0.5];

fn morph_json() -> (Value, Vec<u8>) {
    let mut buffer = Vec::new();
    for values in [
        &MORPH_BASE[..],
        &MORPH_A,
        &MORPH_B,
        &MORPH_TIMES,
        &MORPH_WEIGHTS,
    ] {
        buffer.extend(f32_bytes(values));
    }
    let length = buffer.len();
    (
        json!({
            "asset": { "version": "2.0" },
            "buffers": [{ "uri": data_uri(&buffer), "byteLength": length }],
            "bufferViews": [
                { "buffer": 0, "byteOffset": 0, "byteLength": 36 },
                { "buffer": 0, "byteOffset": 36, "byteLength": 36 },
                { "buffer": 0, "byteOffset": 72, "byteLength": 36 },
                { "buffer": 0, "byteOffset": 108, "byteLength": 8 },
                { "buffer": 0, "byteOffset": 116, "byteLength": 16 }
            ],
            "accessors": [
                { "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3",
                  "min": [-1.0, -2.0, 0.0], "max": [2.0, 2.0, 4.0] },
                { "bufferView": 1, "componentType": 5126, "count": 3, "type": "VEC3",
                  "min": [0.0, -1.0, -0.5], "max": [1.0, 0.5, 2.0] },
                { "bufferView": 2, "componentType": 5126, "count": 3, "type": "VEC3" },
                { "bufferView": 3, "componentType": 5126, "count": 2, "type": "SCALAR",
                  "min": [0.0], "max": [1.0] },
                { "bufferView": 4, "componentType": 5126, "count": 4, "type": "SCALAR" }
            ],
            "meshes": [{
                "weights": [0.25, 0.75],
                "primitives": [{
                    "attributes": { "POSITION": 0 },
                    "targets": [{ "POSITION": 1 }, { "POSITION": 2 }]
                }]
            }],
            "nodes": [{ "mesh": 0, "weights": [0.5, 0.5] }],
            "scenes": [{ "nodes": [0] }],
            "scene": 0,
            "animations": [{
                "samplers": [{ "input": 3, "output": 4, "interpolation": "LINEAR" }],
                "channels": [{ "sampler": 0, "target": { "node": 0, "path": "weights" } }]
            }]
        }),
        buffer,
    )
}

const RIG_POSITIONS: [f32; 9] = [1.0, 2.0, -3.0, 0.5, -0.25, 4.0, 2.0, 0.0, 1.5];
const RIG_NORMALS: [f32; 9] = [0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0];
const RIG_TEXCOORDS: [f32; 6] = [0.0, 0.0, 1.0, 0.0, 0.0, 1.0];
const RIG_WEIGHTS: [f32; 12] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0];
const RIG_INVERSE_BIND: [f32; 16] = [
    1.0, 0.0, 0.0, 0.0, //
    0.0, 1.0, 0.0, 0.0, //
    0.0, 0.0, 1.0, 0.0, //
    -1.0, 2.0, -0.5, 1.0,
];
const RIG_TIMES: [f32; 2] = [0.0, 1.0];
const RIG_TRANSLATIONS: [f32; 6] = [1.0, 0.0, 0.0, 0.0, 2.0, -4.0];
const RIG_ROTATIONS: [f32; 8] = [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
const RIG_SCALES: [f32; 6] = [1.0, 1.0, 1.0, 2.0, 2.0, 2.0];

/// Byte offsets of the rig fixture's buffer views, all four-byte aligned and
/// mutually disjoint.
mod rig {
    pub const POSITION: usize = 0; // 36 bytes
    pub const NORMAL: usize = 36; // 36
    pub const TEXCOORD: usize = 72; // 24
    pub const JOINTS: usize = 96; // 24
    pub const WEIGHTS: usize = 120; // 48
    pub const INVERSE_BIND: usize = 168; // 64
    pub const TIMES: usize = 232; // 8
    // Sized for a CUBICSPLINE output's six VEC3s so the LINEAR/STEP and
    // CUBICSPLINE fixtures share one layout.
    pub const TRANSLATION: usize = 240; // 72
    pub const ROTATION: usize = 312; // 32
    pub const SCALE: usize = 344; // 24
    pub const INDICES: usize = 368; // 6
    pub const LENGTH: usize = 374;
}

/// A complete skinned, animated fixture: a mesh with every accepted vertex
/// attribute, one skin with a non-identity inverse bind, and translation,
/// rotation and scale samplers sharing one time accessor.
fn rig_buffer(interpolation: &str) -> Vec<u8> {
    let mut buffer = vec![0u8; rig::LENGTH];
    let mut put = |offset: usize, payload: Vec<u8>| {
        buffer[offset..offset + payload.len()].copy_from_slice(&payload);
    };
    put(rig::POSITION, f32_bytes(&RIG_POSITIONS));
    put(rig::NORMAL, f32_bytes(&RIG_NORMALS));
    put(rig::TEXCOORD, f32_bytes(&RIG_TEXCOORDS));
    put(rig::JOINTS, u16_bytes(&[0; 12]));
    put(rig::WEIGHTS, f32_bytes(&RIG_WEIGHTS));
    put(rig::INVERSE_BIND, f32_bytes(&RIG_INVERSE_BIND));
    put(rig::TIMES, f32_bytes(&RIG_TIMES));
    put(rig::ROTATION, f32_bytes(&RIG_ROTATIONS));
    put(rig::SCALE, f32_bytes(&RIG_SCALES));
    put(rig::INDICES, u16_bytes(&[0, 1, 2]));
    if interpolation == "CUBICSPLINE" {
        // [in-tangent, value, out-tangent] per key, all six VEC3s lengths.
        put(
            rig::TRANSLATION,
            f32_bytes(&[
                0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.5, 0.0, 0.0, //
                0.25, 0.0, 0.0, 0.0, 2.0, -4.0, 0.0, 0.0, 0.0,
            ]),
        );
    } else {
        put(rig::TRANSLATION, f32_bytes(&RIG_TRANSLATIONS));
    }
    buffer
}

fn rig_json(interpolation: &str, buffer: &[u8]) -> Value {
    let translation_count = if interpolation == "CUBICSPLINE" { 6 } else { 2 };
    json!({
        "asset": { "version": "2.0" },
        "buffers": [{ "uri": data_uri(buffer), "byteLength": rig::LENGTH }],
        "bufferViews": [
            { "buffer": 0, "byteOffset": rig::POSITION, "byteLength": 36 },
            { "buffer": 0, "byteOffset": rig::NORMAL, "byteLength": 36 },
            { "buffer": 0, "byteOffset": rig::TEXCOORD, "byteLength": 24 },
            { "buffer": 0, "byteOffset": rig::JOINTS, "byteLength": 24 },
            { "buffer": 0, "byteOffset": rig::WEIGHTS, "byteLength": 48 },
            { "buffer": 0, "byteOffset": rig::INVERSE_BIND, "byteLength": 64 },
            { "buffer": 0, "byteOffset": rig::TIMES, "byteLength": 8 },
            { "buffer": 0, "byteOffset": rig::TRANSLATION, "byteLength": 72 },
            { "buffer": 0, "byteOffset": rig::ROTATION, "byteLength": 32 },
            { "buffer": 0, "byteOffset": rig::SCALE, "byteLength": 24 },
            { "buffer": 0, "byteOffset": rig::INDICES, "byteLength": 6 }
        ],
        "accessors": [
            { "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3",
              "min": [0.5, -0.25, -3.0], "max": [2.0, 2.0, 4.0] },
            { "bufferView": 1, "componentType": 5126, "count": 3, "type": "VEC3" },
            { "bufferView": 2, "componentType": 5126, "count": 3, "type": "VEC2" },
            { "bufferView": 3, "componentType": 5123, "count": 3, "type": "VEC4" },
            { "bufferView": 4, "componentType": 5126, "count": 3, "type": "VEC4" },
            { "bufferView": 5, "componentType": 5126, "count": 1, "type": "MAT4",
              "min": [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, -1.0, 2.0, -0.5, 1.0],
              "max": [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, -1.0, 2.0, -0.5, 1.0] },
            { "bufferView": 6, "componentType": 5126, "count": 2, "type": "SCALAR",
              "min": [0.0], "max": [1.0] },
            { "bufferView": 7, "componentType": 5126, "count": translation_count, "type": "VEC3" },
            { "bufferView": 8, "componentType": 5126, "count": 2, "type": "VEC4" },
            { "bufferView": 9, "componentType": 5126, "count": 2, "type": "VEC3" },
            { "bufferView": 10, "componentType": 5123, "count": 3, "type": "SCALAR" }
        ],
        "materials": [{
            "name": "surface",
            "alphaCutoff": 0.25,
            "emissiveFactor": [0.5, 0.25, 0.125],
            "pbrMetallicRoughness": { "baseColorFactor": [1.0, 0.5, 0.25, 1.0], "metallicFactor": 0.75 }
        }],
        "meshes": [{ "primitives": [{
            "attributes": {
                "POSITION": 0, "NORMAL": 1, "TEXCOORD_0": 2, "JOINTS_0": 3, "WEIGHTS_0": 4
            },
            "indices": 10,
            "material": 0
        }] }],
        "nodes": [
            { "name": "joint" },
            { "name": "mesh-holder", "mesh": 0, "skin": 0 }
        ],
        "scenes": [{ "nodes": [0, 1] }],
        "scene": 0,
        "skins": [{ "joints": [0], "skeleton": 0, "inverseBindMatrices": 5 }],
        "animations": [{
            "name": "clip",
            "samplers": [
                { "input": 6, "interpolation": interpolation, "output": 7 },
                { "input": 6, "interpolation": "LINEAR", "output": 8 },
                { "input": 6, "interpolation": "LINEAR", "output": 9 }
            ],
            "channels": [
                { "sampler": 0, "target": { "node": 0, "path": "translation" } },
                { "sampler": 1, "target": { "node": 0, "path": "rotation" } },
                { "sampler": 2, "target": { "node": 0, "path": "scale" } }
            ]
        }]
    })
}

// --- Raw POSITION morph targets --------------------------------------------

#[test]
fn whole_document_scales_raw_position_morphs_and_preserves_every_weight_byte() {
    let (json, original) = morph_json();
    let mut glb_json = json.clone();
    glb_json["buffers"][0]
        .as_object_mut()
        .expect("buffer object")
        .remove("uri");
    for (name, source) in [
        ("morph.gltf", accepted("morph.gltf", &json)),
        ("morph.glb", accepted_glb("morph.glb", &glb_json, &original)),
    ] {
        let plan = plan_for(&source, 2.0);
        let artifact = rewrite_scale_plan(&source, &plan).expect("raw morph conversion");
        let (artifact_json, buffers) = artifact_parts(&artifact);
        assert_eq!(artifact.rewritten_accessors(), [0, 1, 2], "{name}");
        assert_eq!(
            read_f32(&buffers[0][0..108]),
            MORPH_BASE
                .iter()
                .chain(MORPH_A.iter())
                .chain(MORPH_B.iter())
                .map(|value| value * 2.0)
                .collect::<Vec<_>>(),
            "{name}"
        );
        assert_eq!(&buffers[0][108..132], &original[108..132], "{name}");
        assert_eq!(artifact_json["meshes"][0]["weights"], json!([0.25, 0.75]));
        assert_eq!(artifact_json["nodes"][0]["weights"], json!([0.5, 0.5]));
        assert_eq!(
            artifact_json["accessors"][1]["min"],
            json!([0.0, -2.0, -1.0])
        );
        assert_eq!(artifact_json["accessors"][1]["max"], json!([2.0, 1.0, 4.0]));
        prove_rewritten_artifact(&source, &artifact, &plan).expect("raw-byte artifact proof");
    }
}

#[test]
fn morph_position_aliases_are_scaled_once_per_unique_accessor() {
    let (mut json, _) = morph_json();
    json["meshes"][0]["primitives"][0]["targets"] =
        json!([{ "POSITION": 0 }, { "POSITION": 1 }, { "POSITION": 1 }]);
    json["meshes"][0]["weights"] = json!([0.25, 0.5, 0.25]);
    json["nodes"][0]["weights"] = json!([0.25, 0.5, 0.25]);
    json["animations"] = json!([]);
    let source = accepted("morph-alias.gltf", &json);
    let plan = plan_for(&source, 2.0);
    let artifact = rewrite_scale_plan(&source, &plan).expect("compatible aliases collapse");
    let (_, buffers) = artifact_parts(&artifact);
    assert_eq!(artifact.rewritten_accessors(), [0, 1]);
    assert_eq!(
        read_f32(&buffers[0][0..36]),
        MORPH_BASE.map(|value| value * 2.0)
    );
    assert_eq!(
        read_f32(&buffers[0][36..72]),
        MORPH_A.map(|value| value * 2.0)
    );
    prove_rewritten_artifact(&source, &artifact, &plan).expect("alias proof");
}

#[test]
fn an_interleaved_morph_position_accessor_is_refused_at_its_own_location() {
    let (mut value, mut buffer) = morph_json();
    let interleaved_offset = buffer.len();
    for vector in MORPH_A.chunks_exact(3) {
        buffer.extend(f32_bytes(vector));
        buffer.extend(f32::to_le_bytes(1234.0));
    }
    value["buffers"][0]["uri"] = json!(data_uri(&buffer));
    value["buffers"][0]["byteLength"] = json!(buffer.len());
    value["bufferViews"][1] = json!({
        "buffer": 0,
        "byteOffset": interleaved_offset,
        "byteLength": 48,
        "byteStride": 16
    });

    let violations = rejected("morph-interleaved.gltf", &value);
    assert_eq!(
        violations
            .iter()
            .filter(|violation| {
                violation.kind == GltfCapabilityViolationKind::UnsafeAccessorLayout
            })
            .map(|violation| violation.location.as_str())
            .collect::<Vec<_>>(),
        ["/accessors/1"]
    );
}

#[test]
fn dense_non_f32_and_normalized_morph_positions_are_exactly_located_refusals() {
    for (name, normalized) in [("non-f32", false), ("normalized", true)] {
        let (mut value, _) = morph_json();
        value["accessors"][1]["componentType"] = json!(5123);
        if normalized {
            value["accessors"][1]["normalized"] = json!(true);
        }

        let violations = rejected(&format!("morph-{name}.gltf"), &value);
        assert_eq!(
            violations,
            [GltfCapabilityViolation {
                kind: GltfCapabilityViolationKind::UnsafeAccessorLayout,
                location: "/accessors/1".into(),
            }],
            "{name} must identify the morph accessor itself"
        );
    }
}

#[test]
fn a_morph_position_accessor_aliasing_an_image_payload_is_refused() {
    let (mut value, _) = morph_json();
    value["images"] = json!([{ "bufferView": 1, "mimeType": "image/png" }]);

    let violations = rejected("morph-image-alias.gltf", &value);
    assert_eq!(
        violations
            .iter()
            .filter(|violation| {
                violation.kind == GltfCapabilityViolationKind::ImagePayloadOverlap
            })
            .map(|violation| violation.location.as_str())
            .collect::<Vec<_>>(),
        ["/images/0/bufferView"]
    );
    assert_eq!(
        violations
            .iter()
            .filter(|violation| {
                violation.kind == GltfCapabilityViolationKind::OverlappingAccessorRanges
            })
            .map(|violation| violation.location.as_str())
            .collect::<Vec<_>>(),
        ["/accessors/1"],
        "the morph accessor, not the disjoint base POSITION, owns the unsafe alias"
    );
}

#[test]
fn unsupported_and_unsafe_morph_payloads_refuse_before_any_rewrite() {
    let (base, original_buffer) = morph_json();
    let original_json = bytes(&base);
    for semantic in ["NORMAL", "TANGENT"] {
        let mut unsupported = base.clone();
        unsupported["meshes"][0]["primitives"][0]["targets"][0] = json!({ (semantic): 1 });
        let violations = rejected(&format!("morph-{semantic}.gltf"), &unsupported);
        assert_eq!(
            kinds(&violations),
            [GltfCapabilityViolationKind::MorphTarget]
        );
        assert_eq!(
            violations[0].location,
            format!("/meshes/0/primitives/0/targets/0/{semantic}")
        );
    }

    let mut sparse = base.clone();
    sparse["accessors"][1]["sparse"] = json!({
        "count": 1,
        "indices": { "bufferView": 3, "componentType": 5123 },
        "values": { "bufferView": 1 }
    });
    let violations = rejected("morph-sparse.gltf", &sparse);
    assert!(kinds(&violations).contains(&GltfCapabilityViolationKind::UnsafeAccessorLayout));

    let mut overlapping = base.clone();
    overlapping["bufferViews"][2]["byteOffset"] = json!(60);
    let violations = rejected("morph-overlap.gltf", &overlapping);
    assert!(kinds(&violations).contains(&GltfCapabilityViolationKind::OverlappingAccessorRanges));

    let mut conflicting = base.clone();
    conflicting["meshes"][0]["primitives"][0]["attributes"]["NORMAL"] = json!(1);
    let violations = rejected("morph-normal-alias.gltf", &conflicting);
    assert!(kinds(&violations).contains(&GltfCapabilityViolationKind::ConflictingAccessorUse));

    assert_eq!(
        bytes(&base),
        original_json,
        "refusal leaves source JSON untouched"
    );
    assert_eq!(data_uri(&original_buffer), base["buffers"][0]["uri"]);
}

// --- 1: node rest translation ----------------------------------------------

#[test]
fn a_trs_node_rest_translation_scales_while_rotation_and_scale_are_untouched() {
    let (mut value, buffer) = minimal_json(&[0.0; 9]);
    value["nodes"] = json!([{
        "translation": [1.5, -2.0, 0.25],
        "rotation": [0.0, 0.0, 0.5, 0.8660254],
        "scale": [2.0, 3.0, 4.0]
    }]);

    let source = accepted("trs.gltf", &value);
    let artifact = rewrite_linear_units(&source, 4.0).expect("rewrite");
    let (json, _) = artifact_parts(&artifact);

    assert_eq!(json["nodes"][0]["translation"], json!([6.0, -8.0, 1.0]));
    assert_eq!(
        json["nodes"][0]["rotation"],
        json!([0.0, 0.0, 0.5, 0.8660254])
    );
    assert_eq!(json["nodes"][0]["scale"], json!([2.0, 3.0, 4.0]));
    assert_eq!(
        artifact.rewritten_json_pointers(),
        [
            "/accessors/0/max",
            "/accessors/0/min",
            "/nodes/0/translation"
        ]
    );

    let mut glb_value = value.clone();
    glb_value["buffers"][0] = json!({ "byteLength": buffer.len() });
    let glb_source = accepted_glb("trs.glb", &glb_value, &buffer);
    let glb_artifact = rewrite_linear_units(&glb_source, 4.0).expect("glb rewrite");
    let (glb_json, _) = artifact_parts(&glb_artifact);
    assert_eq!(glb_json["nodes"][0]["translation"], json!([6.0, -8.0, 1.0]));
    assert_eq!(glb_json["nodes"][0]["scale"], json!([2.0, 3.0, 4.0]));
}

// --- 2: matrix node ---------------------------------------------------------

#[test]
fn a_matrix_node_scales_only_its_translation_column() {
    let (mut value, _) = minimal_json(&[0.0; 9]);
    value["nodes"] = json!([{ "matrix": [
        2.0, 0.0, 0.0, 0.0,
        0.0, 0.5, 0.0, 0.0,
        0.0, 0.0, 4.0, 0.0,
        1.5, -2.0, 0.25, 1.0
    ] }]);

    let source = accepted("matrix.gltf", &value);
    let artifact = rewrite_linear_units(&source, 4.0).expect("rewrite");
    let (json, _) = artifact_parts(&artifact);

    assert_eq!(
        json["nodes"][0]["matrix"],
        json!([
            2.0, 0.0, 0.0, 0.0, //
            0.0, 0.5, 0.0, 0.0, //
            0.0, 0.0, 4.0, 0.0, //
            6.0, -8.0, 1.0, 1.0
        ])
    );
    assert_eq!(
        artifact.rewritten_json_pointers(),
        ["/accessors/0/max", "/accessors/0/min", "/nodes/0/matrix"]
    );

    // The artifact proof re-derives the same split independently: the 3x3
    // and the homogeneous component are dimensionless, so a rewrite that
    // scaled them would raise `dimensionless_residual` above zero rather
    // than pass.
    let plan = plan_for(&source, 4.0);
    let proof = prove_rewritten_artifact(&source, &artifact, &plan).expect("artifact proof");
    assert_eq!(proof.dimensionless_residual, 0.0);
    assert_eq!(proof.length_factor_residual, 0.0);
}

// --- 2b: out-of-contract node transforms ------------------------------------

/// The identity node `matrix`, column-major.
const IDENTITY_MATRIX: [f64; 16] = [
    1.0, 0.0, 0.0, 0.0, //
    0.0, 1.0, 0.0, 0.0, //
    0.0, 0.0, 1.0, 0.0, //
    0.0, 0.0, 0.0, 1.0,
];

fn node_matrix_document(matrix: [f64; 16], extra: Option<(&str, Value)>) -> Value {
    let (mut value, _) = minimal_json(&[0.0; 9]);
    let mut node = json!({ "matrix": Vec::from(matrix) });
    if let Some((member, member_value)) = extra {
        node[member] = member_value;
    }
    value["nodes"] = json!([node]);
    value
}

// A node combining `matrix` with a TRS member, and a node `matrix` whose
// last row is not `(0, 0, 0, 1)`, are refused by #280's preflight (#301), so
// no such source can be built here at all. The located preflight rejections
// live in `capability_preflight.rs`, and the rewriter's own defence-in-depth
// guard is exercised directly in `scale.rs`'s unit tests. What remains here
// is the end-to-end must-not-over-reject direction.

#[test]
fn an_affine_node_matrix_with_a_translation_column_still_converts() {
    // The guard above must not reject the ordinary case: entries 3, 7 and 11
    // are zero and entry 15 is one, and the translation column converts.
    let mut matrix = IDENTITY_MATRIX;
    matrix[12] = 1.5;
    matrix[13] = -2.0;
    matrix[14] = 0.25;
    let value = node_matrix_document(matrix, None);
    let source = accepted("affine-matrix.gltf", &value);
    let artifact = rewrite_linear_units(&source, 4.0).expect("an affine matrix converts");
    let (json, _) = artifact_parts(&artifact);
    assert_eq!(
        json["nodes"][0]["matrix"],
        json!([
            1.0, 0.0, 0.0, 0.0, //
            0.0, 1.0, 0.0, 0.0, //
            0.0, 0.0, 1.0, 0.0, //
            6.0, -8.0, 1.0, 1.0
        ])
    );
}

#[test]
fn a_transform_member_authored_as_json_null_declares_no_transform_and_converts() {
    // `"matrix": null` is `Some(Value::Null)` to `serde_json` and `None` to
    // the typed glTF parse. The raw rewrite selection must agree with the
    // typed parse: selecting the `null` would hand `rewrite_json_array` a
    // value that is not an array of numbers, which it can only report as a
    // malformed source — so the node would preflight clean at the gate and
    // then kill the rewriter.
    let (mut value, _) = minimal_json(&[0.0; 9]);
    value["nodes"] = json!([{ "matrix": null, "translation": [1.5, -2.0, 0.25] }]);
    let source = accepted("null-matrix.gltf", &value);
    let artifact = rewrite_linear_units(&source, 4.0).expect("a null `matrix` declares no matrix");
    let (json, _) = artifact_parts(&artifact);
    assert_eq!(json["nodes"][0]["translation"], json!([6.0, -8.0, 1.0]));
    assert_eq!(json["nodes"][0]["matrix"], Value::Null);
    assert!(
        !artifact
            .rewritten_json_pointers()
            .contains(&"/nodes/0/matrix".to_owned()),
        "a null `matrix` is not a rewritten location"
    );

    // The mirror: an affine `matrix` beside a null `translation` converts its
    // translation column, and the null is not selected either.
    let mut matrix = IDENTITY_MATRIX;
    matrix[12] = 1.5;
    matrix[13] = -2.0;
    matrix[14] = 0.25;
    let value = node_matrix_document(matrix, Some(("translation", Value::Null)));
    let source = accepted("null-translation.gltf", &value);
    let artifact =
        rewrite_linear_units(&source, 4.0).expect("a null `translation` declares no translation");
    let (json, _) = artifact_parts(&artifact);
    assert_eq!(
        json["nodes"][0]["matrix"],
        json!([
            1.0, 0.0, 0.0, 0.0, //
            0.0, 1.0, 0.0, 0.0, //
            0.0, 0.0, 1.0, 0.0, //
            6.0, -8.0, 1.0, 1.0
        ])
    );
    assert_eq!(json["nodes"][0]["translation"], Value::Null);
    assert!(
        !artifact
            .rewritten_json_pointers()
            .contains(&"/nodes/0/translation".to_owned()),
        "a null `translation` is not a rewritten location"
    );

    // A null `matrix` alone round-trips: nothing about the node converts, and
    // the member survives the rewrite unchanged.
    let (mut value, _) = minimal_json(&[0.0; 9]);
    value["nodes"] = json!([{ "matrix": null }]);
    let source = accepted("only-null-matrix.gltf", &value);
    let artifact = rewrite_linear_units(&source, 4.0).expect("a null `matrix` alone converts");
    let (json, _) = artifact_parts(&artifact);
    assert_eq!(json["nodes"][0], json!({ "matrix": null }));
}

// --- 3, 4, 5: translation samplers -----------------------------------------

#[test]
fn linear_and_step_translation_outputs_scale_while_times_are_untouched() {
    for interpolation in ["LINEAR", "STEP"] {
        let buffer = rig_buffer(interpolation);
        let value = rig_json(interpolation, &buffer);
        let source = accepted("sampler.gltf", &value);
        let artifact = rewrite_linear_units(&source, 4.0).expect("rewrite");
        let (_, buffers) = artifact_parts(&artifact);

        assert_eq!(
            read_f32(&buffers[0][rig::TRANSLATION..rig::TRANSLATION + 24]),
            vec![4.0, 0.0, 0.0, 0.0, 8.0, -16.0],
            "{interpolation} translation output"
        );
        assert_eq!(
            &buffers[0][rig::TIMES..rig::TIMES + 8],
            &buffer[rig::TIMES..rig::TIMES + 8],
            "{interpolation} sampler input times are byte-identical"
        );
    }
}

#[test]
fn every_cubicspline_translation_element_scales_including_both_tangents() {
    let buffer = rig_buffer("CUBICSPLINE");
    let value = rig_json("CUBICSPLINE", &buffer);
    let source = accepted("cubic.gltf", &value);
    let artifact = rewrite_linear_units(&source, 4.0).expect("rewrite");
    let (_, buffers) = artifact_parts(&artifact);

    // Six VEC3s: [in, value, out] for key 0 then key 1. All eighteen floats
    // are lengths, so all eighteen scale.
    assert_eq!(
        read_f32(&buffers[0][rig::TRANSLATION..rig::TRANSLATION + 72]),
        vec![
            0.0, 0.0, 0.0, 4.0, 0.0, 0.0, 2.0, 0.0, 0.0, //
            1.0, 0.0, 0.0, 0.0, 8.0, -16.0, 0.0, 0.0, 0.0
        ]
    );
    assert_eq!(
        &buffers[0][rig::TIMES..rig::TIMES + 8],
        &buffer[rig::TIMES..rig::TIMES + 8]
    );
}

// --- 6: rotation and scale samplers ----------------------------------------

#[test]
fn rotation_and_scale_sampler_outputs_are_byte_identical() {
    let buffer = rig_buffer("LINEAR");
    let value = rig_json("LINEAR", &buffer);
    let source = accepted("dimensionless.gltf", &value);
    let artifact = rewrite_linear_units(&source, 4.0).expect("rewrite");
    let (_, buffers) = artifact_parts(&artifact);

    assert_eq!(
        &buffers[0][rig::ROTATION..rig::ROTATION + 32],
        &buffer[rig::ROTATION..rig::ROTATION + 32]
    );
    assert_eq!(
        &buffers[0][rig::SCALE..rig::SCALE + 24],
        &buffer[rig::SCALE..rig::SCALE + 24]
    );
    assert_eq!(
        read_f32(&buffers[0][rig::SCALE..rig::SCALE + 24]),
        vec![1.0, 1.0, 1.0, 2.0, 2.0, 2.0],
        "an animated scale channel is dimensionless and must not be converted"
    );
}

// --- 7: mesh POSITION and accessor bounds ----------------------------------

#[test]
fn mesh_positions_and_their_bounds_scale_and_still_bound_the_payload() {
    let buffer = rig_buffer("LINEAR");
    let value = rig_json("LINEAR", &buffer);
    let source = accepted("positions.gltf", &value);
    let artifact = rewrite_linear_units(&source, 4.0).expect("rewrite");
    let (json, buffers) = artifact_parts(&artifact);

    assert_eq!(
        read_f32(&buffers[0][rig::POSITION..rig::POSITION + 36]),
        vec![4.0, 8.0, -12.0, 2.0, -1.0, 16.0, 8.0, 0.0, 6.0]
    );
    assert_eq!(json["accessors"][0]["min"], json!([2.0, -1.0, -12.0]));
    assert_eq!(json["accessors"][0]["max"], json!([8.0, 8.0, 16.0]));
    assert_eq!(json["accessors"][6]["min"], json!([0.0]));
    assert_eq!(
        json["accessors"][6]["max"],
        json!([1.0]),
        "sampler input bounds are seconds and stay authored"
    );

    let mut glb_value = value.clone();
    glb_value["buffers"][0] = json!({ "byteLength": rig::LENGTH });
    let glb_source = accepted_glb("positions.glb", &glb_value, &buffer);
    let glb_artifact = rewrite_linear_units(&glb_source, 4.0).expect("glb rewrite");
    let (glb_json, glb_buffers) = artifact_parts(&glb_artifact);
    assert_eq!(
        read_f32(&glb_buffers[0][rig::POSITION..rig::POSITION + 36]),
        vec![4.0, 8.0, -12.0, 2.0, -1.0, 16.0, 8.0, 0.0, 6.0]
    );
    assert_eq!(glb_json["accessors"][0]["min"], json!([2.0, -1.0, -12.0]));
}

#[test]
fn a_plan_from_a_different_primitive_inventory_is_refused_before_raw_rewrite() {
    let payload = f32_bytes(&[
        0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, // positions
        0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, // normals
    ]);
    let with_normals = json!({
        "asset": { "version": "2.0" },
        "buffers": [{ "uri": data_uri(&payload), "byteLength": payload.len() }],
        "bufferViews": [
            { "buffer": 0, "byteOffset": 0, "byteLength": 36 },
            { "buffer": 0, "byteOffset": 36, "byteLength": 36 }
        ],
        "accessors": [
            { "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3",
              "min": [0.0, 0.0, 0.0], "max": [1.0, 1.0, 0.0] },
            { "bufferView": 1, "componentType": 5126, "count": 3, "type": "VEC3" }
        ],
        "meshes": [{ "primitives": [{
            "attributes": { "POSITION": 0, "NORMAL": 1 }
        }] }]
    });
    let planned_source = accepted("planned-normal.gltf", &with_normals);
    let plan = plan_for(&planned_source, 2.0);

    let mut without_normals = with_normals;
    without_normals["meshes"][0]["primitives"][0]["attributes"]
        .as_object_mut()
        .expect("attributes")
        .remove("NORMAL");
    let replay_source = accepted("replay-without-normal.gltf", &without_normals);

    assert!(matches!(
        rewrite_scale_plan(&replay_source, &plan),
        Err(GltfScaleRewriteError::Plan(
            ScaleError::PlanDocumentMismatch {
                reason: "payload_shape_inventory_mismatch"
            }
        ))
    ));
}

/// The `f32` whose shortest round-tripping decimal spelling is
/// `29460752000`, which is *not* the decimal its `f64` widening prints
/// (`29460752384`). A real exporter writes the shortest spelling, so the
/// authored JSON bound and the stored payload disagree in `f64` even though
/// they are the same `f32`.
const BOUND_PROBE: f32 = 2.9460752e10;

/// The factor that makes the two `f64` narrowings straddle one `f32` ULP.
const BOUND_PROBE_FACTOR: f64 = 5.72563720703125e-11;

#[test]
fn a_bound_that_narrowing_would_leave_violated_is_widened_to_the_payload() {
    // The payload narrows `f64(29460752384) * q` to `1.6868159`; the authored
    // bound narrows `f64(29460752000) * q` to `1.6868157`, one ULP nearer
    // zero. So the converted `max` is *below* the largest converted sample
    // and the converted `min` is *above* the smallest — both invalid glTF —
    // unless each bound is reconciled against the observed extrema. A
    // one-ULP nudge is not enough to know which way to move; only the
    // observed extrema are.
    //
    // Both literals below are the arithmetic truth of that narrowing, not a
    // value this crate produced: see the constants above.
    let positions = [-BOUND_PROBE, 0.0, 0.0, BOUND_PROBE, 0.0, 0.0, 0.0, 0.0, 0.0];
    let (mut value, _) = minimal_json(&positions);
    // Authored as the shortest round-tripping decimal, the way an exporter
    // emits it. `minimal_json` would otherwise serialize the `f32`'s `f64`
    // widening, which agrees with the payload and hides the whole effect.
    value["accessors"][0]["min"] = json!([-2.9460752e10, 0.0, 0.0]);
    value["accessors"][0]["max"] = json!([2.9460752e10, 0.0, 0.0]);

    let source = accepted("bounds.gltf", &value);
    let artifact = rewrite_linear_units(&source, BOUND_PROBE_FACTOR).expect("rewrite");
    let (json, buffers) = artifact_parts(&artifact);

    assert_eq!(
        read_f32(&buffers[0]),
        vec![-1.6868159, 0.0, 0.0, 1.6868159, 0.0, 0.0, 0.0, 0.0, 0.0],
        "the payload narrows the f32's own widening"
    );
    assert_eq!(
        json["accessors"][0]["min"],
        json!([-1.6868159, 0.0, 0.0]),
        "widening the authored bound to the payload; narrowing alone gives -1.6868157"
    );
    assert_eq!(
        json["accessors"][0]["max"],
        json!([1.6868159, 0.0, 0.0]),
        "widening the authored bound to the payload; narrowing alone gives 1.6868157"
    );
}

#[test]
fn every_converted_bound_still_bounds_its_converted_payload() {
    // A factor sweep over payloads whose authored bounds are the shortest
    // round-tripping decimals of the stored `f32`s. Each case restates the
    // binding obligation directly: whatever the rounding, `min` is at or
    // below every converted sample and `max` at or above.
    type Case = (&'static str, [f32; 3], [f64; 3], f64);
    let cases: [Case; 6] = [
        // (name, x samples, authored x bounds [min, mid, max], factor)
        (
            "one-ULP straddle, both bounds",
            [-BOUND_PROBE, BOUND_PROBE, 0.0],
            [-2.9460752e10, 0.0, 2.9460752e10],
            BOUND_PROBE_FACTOR,
        ),
        (
            "min straddles upward under a metric factor",
            [1.8783379, 5.0, 9.0],
            [1.8783379, 5.0, 9.0],
            0.001,
        ),
        (
            "max straddles downward under a metric factor",
            [-9.0, -5.0, -1.8783379],
            [-9.0, -5.0, -1.8783379],
            0.001,
        ),
        (
            "repeating-binary factor",
            [1.0e-8, 3.0e-8, 5.0e-8],
            [1.0e-8, 3.0e-8, 5.0e-8],
            1.0 / 3.0,
        ),
        (
            "small samples under a 1e-5 factor",
            [0.0013283765, 0.002, 0.05],
            [0.0013283765, 0.002, 0.05],
            1.0e-5,
        ),
        (
            "exact power of two",
            [-3.0, 0.5, 2.0],
            [-3.0, 0.5, 2.0],
            0.25,
        ),
    ];

    for (name, samples, authored, factor) in cases {
        let positions = [
            samples[0], 0.0, 0.0, //
            samples[1], 0.0, 0.0, //
            samples[2], 0.0, 0.0,
        ];
        let (mut value, _) = minimal_json(&positions);
        let authored_min = authored.iter().copied().fold(f64::INFINITY, f64::min);
        let authored_max = authored.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        value["accessors"][0]["min"] = json!([authored_min, 0.0, 0.0]);
        value["accessors"][0]["max"] = json!([authored_max, 0.0, 0.0]);

        let source = accepted("sweep.gltf", &value);
        let artifact = rewrite_linear_units(&source, factor)
            .unwrap_or_else(|error| panic!("{name}: rewrite failed: {error:?}"));
        let (json, buffers) = artifact_parts(&artifact);

        let payload = read_f32(&buffers[0]);
        for component in 0..3 {
            let observed_min = payload
                .iter()
                .skip(component)
                .step_by(3)
                .copied()
                .fold(f32::INFINITY, f32::min);
            let observed_max = payload
                .iter()
                .skip(component)
                .step_by(3)
                .copied()
                .fold(f32::NEG_INFINITY, f32::max);
            // Compared in `f32`, the model glTF declares for `min`/`max`.
            let declared_min = json["accessors"][0]["min"][component]
                .as_f64()
                .expect("number") as f32;
            let declared_max = json["accessors"][0]["max"][component]
                .as_f64()
                .expect("number") as f32;
            assert!(
                declared_min <= observed_min,
                "{name}: min[{component}] {declared_min} does not bound {observed_min}"
            );
            assert!(
                declared_max >= observed_max,
                "{name}: max[{component}] {declared_max} does not bound {observed_max}"
            );
        }
    }
}

// --- 8: the accessor-aliasing guard ----------------------------------------

#[test]
fn an_accessor_shared_by_two_primitives_is_scaled_exactly_once() {
    let (mut value, _) = minimal_json(&[1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0, 128.0, 256.0]);
    // One POSITION accessor reached through three logical uses across two
    // meshes. Scaling per logical use would emit `q^3`.
    value["meshes"] = json!([
        { "primitives": [
            { "attributes": { "POSITION": 0 } },
            { "attributes": { "POSITION": 0 } }
        ] },
        { "primitives": [{ "attributes": { "POSITION": 0 } }] }
    ]);

    let source = accepted("aliased.gltf", &value);
    let artifact = rewrite_linear_units(&source, 2.0).expect("rewrite");
    let (_, buffers) = artifact_parts(&artifact);

    assert_eq!(
        artifact.rewritten_accessors(),
        [0],
        "one unique accessor, however many logical uses reach it"
    );
    assert_eq!(
        read_f32(&buffers[0]),
        vec![2.0, 4.0, 8.0, 16.0, 32.0, 64.0, 128.0, 256.0, 512.0],
        "q^2 would be 4.0 and q^3 would be 8.0 at the first component"
    );
}

// --- 9: multiple skins and repeated mesh instances --------------------------

#[test]
fn every_skin_inverse_bind_scales_only_its_translation_column() {
    // Two skins and two inverse-bind accessors. Skin 0 is attached to the
    // mesh instance; skin 1 is deliberately unattached, so its raw slots
    // must be authorized through the bone inverse-bind ledger rows instead.
    let mut buffer = vec![0u8; 36 + 64 + 64];
    buffer[0..36].copy_from_slice(&f32_bytes(&[0.0; 9]));
    buffer[36..100].copy_from_slice(&f32_bytes(&[
        2.0, 0.0, 0.0, 0.0, //
        0.0, 2.0, 0.0, 0.0, //
        0.0, 0.0, 2.0, 0.0, //
        1.0, -2.0, 3.0, 1.0,
    ]));
    buffer[100..164].copy_from_slice(&f32_bytes(&[
        1.0, 0.0, 0.0, 0.0, //
        0.0, 1.0, 0.0, 0.0, //
        0.0, 0.0, 1.0, 0.0, //
        -4.0, 0.5, 8.0, 1.0,
    ]));
    let value = json!({
        "asset": { "version": "2.0" },
        "buffers": [{ "uri": data_uri(&buffer), "byteLength": 164 }],
        "bufferViews": [
            { "buffer": 0, "byteOffset": 0, "byteLength": 36 },
            { "buffer": 0, "byteOffset": 36, "byteLength": 64 },
            { "buffer": 0, "byteOffset": 100, "byteLength": 64 }
        ],
        "accessors": [
            { "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3",
              "min": [0.0, 0.0, 0.0], "max": [0.0, 0.0, 0.0] },
            { "bufferView": 1, "componentType": 5126, "count": 1, "type": "MAT4" },
            { "bufferView": 2, "componentType": 5126, "count": 1, "type": "MAT4" }
        ],
        "meshes": [{ "primitives": [{ "attributes": { "POSITION": 0 } }] }],
        "nodes": [
            {}, {},
            // Skin 0 is retained through two modeled instances; skin 1 is
            // deliberately unattached so its raw IBM accessor exercises the
            // source-skin fallback rather than an instance-derived binding.
            { "mesh": 0, "skin": 0 },
            {},
            { "mesh": 0, "skin": 0 }
        ],
        "skins": [
            { "joints": [0], "skeleton": 0, "inverseBindMatrices": 1 },
            { "joints": [1], "skeleton": 1, "inverseBindMatrices": 2 }
        ]
    });

    let source = accepted("skins.gltf", &value);
    let artifact = rewrite_linear_units(&source, 0.5).expect("rewrite");
    let (_, buffers) = artifact_parts(&artifact);

    assert_eq!(artifact.rewritten_accessors(), [0, 1, 2]);
    // This operation's closure is the whole document, so every declared node
    // and skin is affected — including node 1, which authors no transform at
    // all, and both skins. Reporting only the nodes whose JSON changed would
    // understate what a whole-document conversion claims about the artifact.
    assert_eq!(artifact.affected_source_nodes(), [0, 1, 2, 3, 4]);
    assert_eq!(artifact.affected_source_skins(), [0, 1]);
    assert_eq!(
        read_f32(&buffers[0][36..100]),
        vec![
            2.0, 0.0, 0.0, 0.0, //
            0.0, 2.0, 0.0, 0.0, //
            0.0, 0.0, 2.0, 0.0, //
            0.5, -1.0, 1.5, 1.0
        ]
    );
    assert_eq!(
        read_f32(&buffers[0][100..164]),
        vec![
            1.0, 0.0, 0.0, 0.0, //
            0.0, 1.0, 0.0, 0.0, //
            0.0, 0.0, 1.0, 0.0, //
            -2.0, 0.25, 4.0, 1.0
        ]
    );
}

// --- 10: non-length domains -------------------------------------------------

#[test]
fn non_length_attributes_indices_and_materials_are_untouched() {
    let buffer = rig_buffer("LINEAR");
    let value = rig_json("LINEAR", &buffer);
    let source = accepted("untouched.gltf", &value);
    let artifact = rewrite_linear_units(&source, 4.0).expect("rewrite");
    let (json, buffers) = artifact_parts(&artifact);

    for (name, start, length) in [
        ("NORMAL", rig::NORMAL, 36),
        ("TEXCOORD_0", rig::TEXCOORD, 24),
        ("JOINTS_0", rig::JOINTS, 24),
        ("WEIGHTS_0", rig::WEIGHTS, 48),
        ("indices", rig::INDICES, 6),
    ] {
        assert_eq!(
            &buffers[0][start..start + length],
            &buffer[start..start + length],
            "{name} must be byte-identical"
        );
    }
    assert_eq!(json["materials"], value["materials"]);
    assert_eq!(json["asset"], value["asset"]);
    assert_eq!(json["nodes"], value["nodes"]);
    assert_eq!(json["scenes"], value["scenes"]);
    assert_eq!(json["skins"], value["skins"]);
}

#[test]
fn unreferenced_sparse_accessor_payloads_are_preserved_byte_identical() {
    let (mut value, mut buffer) = minimal_json(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0]);
    let sparse_index = 2u8;
    let sparse_value = [
        0x00, 0x00, 0x80, 0x3f, // 1.0
        0x00, 0x00, 0x00, 0x40, // 2.0
        0x00, 0x00, 0x40, 0x40, // 3.0
    ];
    buffer.push(sparse_index);
    buffer.extend_from_slice(&[0, 0, 0]);
    buffer.extend_from_slice(&sparse_value);
    value["buffers"][0] = json!({ "uri": data_uri(&buffer), "byteLength": buffer.len() });
    value["bufferViews"]
        .as_array_mut()
        .expect("buffer views")
        .extend([
            json!({ "buffer": 0, "byteOffset": 36, "byteLength": 1 }),
            json!({ "buffer": 0, "byteOffset": 40, "byteLength": 12 }),
        ]);
    value["accessors"]
        .as_array_mut()
        .expect("accessors")
        .push(json!({
            "componentType": 5126,
            "count": 3,
            "type": "VEC3",
            "sparse": {
                "count": 1,
                "indices": { "bufferView": 1, "componentType": 5121 },
                "values": { "bufferView": 2 }
            }
        }));

    let source = accepted("unreferenced-sparse.gltf", &value);
    let artifact = rewrite_linear_units(&source, 2.0).expect("rewrite");
    let (_, buffers) = artifact_parts(&artifact);
    assert_eq!(buffers[0][36], sparse_index);
    assert_eq!(&buffers[0][40..52], &sparse_value);

    let plan = plan_for(&source, 2.0);
    prove_rewritten_artifact(&source, &artifact, &plan)
        .expect("artifact proof covers the preserved unreferenced payload");
}

#[test]
fn compact_unreferenced_integer_matrix_payloads_are_preserved_byte_identical() {
    let (mut value, mut buffer) = minimal_json(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0]);
    let dense_matrices = [
        1, 2, 0xa1, 0xa2, 3, 4, 0xa3, 0xa4, // first MAT2 and inter-element padding
        5, 6, 0xb1, 0xb2, 7, 8, // compact final MAT2
    ];
    let sparse_index = 1u8;
    let sparse_matrix = [9, 10, 0xc1, 0xc2, 11, 12];
    buffer.extend_from_slice(&dense_matrices);
    buffer.push(sparse_index);
    buffer.push(0);
    buffer.extend_from_slice(&sparse_matrix);
    value["buffers"][0] = json!({ "uri": data_uri(&buffer), "byteLength": buffer.len() });
    value["bufferViews"]
        .as_array_mut()
        .expect("buffer views")
        .extend([
            json!({ "buffer": 0, "byteOffset": 36, "byteLength": 14 }),
            json!({ "buffer": 0, "byteOffset": 50, "byteLength": 1 }),
            json!({ "buffer": 0, "byteOffset": 52, "byteLength": 6 }),
        ]);
    value["accessors"]
        .as_array_mut()
        .expect("accessors")
        .extend([
            json!({
                "bufferView": 1,
                "componentType": 5121,
                "count": 2,
                "type": "MAT2"
            }),
            json!({
                "componentType": 5121,
                "count": 2,
                "type": "MAT2",
                "sparse": {
                    "count": 1,
                    "indices": { "bufferView": 2, "componentType": 5121 },
                    "values": { "bufferView": 3 }
                }
            }),
        ]);

    let source = accepted("compact-unreferenced-matrices.gltf", &value);
    let artifact = rewrite_linear_units(&source, 2.0).expect("rewrite");
    let (_, buffers) = artifact_parts(&artifact);
    assert_eq!(&buffers[0][36..50], &dense_matrices);
    assert_eq!(buffers[0][50], sparse_index);
    assert_eq!(&buffers[0][52..58], &sparse_matrix);

    let plan = plan_for(&source, 2.0);
    prove_rewritten_artifact(&source, &artifact, &plan)
        .expect("artifact proof covers compact unreferenced integer matrices");
}

// --- 11: array identities ---------------------------------------------------

#[test]
fn every_array_length_and_index_valued_field_survives() {
    let buffer = rig_buffer("LINEAR");
    let value = rig_json("LINEAR", &buffer);
    let source = accepted("identities.gltf", &value);
    let artifact = rewrite_linear_units(&source, 4.0).expect("rewrite");
    let (json, _) = artifact_parts(&artifact);

    for key in [
        "accessors",
        "animations",
        "bufferViews",
        "buffers",
        "materials",
        "meshes",
        "nodes",
        "scenes",
        "skins",
    ] {
        assert_eq!(
            json[key].as_array().map(Vec::len),
            value[key].as_array().map(Vec::len),
            "{key} array length"
        );
    }
    assert_eq!(json["meshes"][0]["primitives"][0]["indices"], json!(10));
    assert_eq!(json["meshes"][0]["primitives"][0]["material"], json!(0));
    assert_eq!(
        json["meshes"][0]["primitives"][0]["attributes"],
        value["meshes"][0]["primitives"][0]["attributes"]
    );
    assert_eq!(json["skins"][0]["inverseBindMatrices"], json!(5));
    assert_eq!(json["skins"][0]["joints"], json!([0]));
    assert_eq!(json["skins"][0]["skeleton"], json!(0));
    assert_eq!(json["scene"], json!(0));
    for accessor_index in 0..11 {
        for field in ["bufferView", "componentType", "count", "type"] {
            assert_eq!(
                json["accessors"][accessor_index][field], value["accessors"][accessor_index][field],
                "/accessors/{accessor_index}/{field}"
            );
        }
    }
    assert_eq!(json["bufferViews"], value["bufferViews"]);
    assert_eq!(json["buffers"][0]["byteLength"], json!(rig::LENGTH));
    assert_eq!(
        artifact.rewritten_json_pointers(),
        [
            "/accessors/0/max",
            "/accessors/0/min",
            "/accessors/5/max",
            "/accessors/5/min",
        ]
    );
    assert_eq!(artifact.reencoded_buffers(), [0]);
}

// --- 12: determinism and GLB framing ---------------------------------------

#[test]
fn rewriting_the_same_glb_twice_yields_identical_bytes_and_valid_framing() {
    let buffer = rig_buffer("LINEAR");
    let mut value = rig_json("LINEAR", &buffer);
    value["buffers"][0] = json!({ "byteLength": rig::LENGTH });
    let source = accepted_glb("determinism.glb", &value, &buffer);

    let first = rewrite_linear_units(&source, 4.0).expect("first rewrite");
    let second = rewrite_linear_units(&source, 4.0).expect("second rewrite");
    assert_eq!(first.bytes(), second.bytes());

    let raw = first.bytes();
    assert_eq!(&raw[0..4], b"glTF");
    assert_eq!(u32::from_le_bytes(raw[4..8].try_into().unwrap()), 2);
    assert_eq!(
        u32::from_le_bytes(raw[8..12].try_into().unwrap()) as usize,
        raw.len()
    );
    let json_len = u32::from_le_bytes(raw[12..16].try_into().unwrap()) as usize;
    assert!(json_len.is_multiple_of(4), "JSON chunk is 4-byte padded");
    assert_eq!(
        u32::from_le_bytes(raw[16..20].try_into().unwrap()),
        0x4e4f_534a
    );
    let bin_start = 20 + json_len;
    let bin_len = u32::from_le_bytes(raw[bin_start..bin_start + 4].try_into().unwrap()) as usize;
    assert!(bin_len.is_multiple_of(4), "BIN chunk is 4-byte padded");
    assert_eq!(bin_len, 376, "374 payload bytes pad to 376");
    assert_eq!(
        u32::from_le_bytes(raw[bin_start + 4..bin_start + 8].try_into().unwrap()),
        0x004e_4942
    );
    assert_eq!(raw.len(), 12 + 8 + json_len + 8 + bin_len);
}

// --- 13: q = 1 identity -----------------------------------------------------

#[test]
fn a_unit_factor_leaves_every_buffer_byte_and_reloaded_value_unchanged() {
    let buffer = rig_buffer("LINEAR");
    let mut value = rig_json("LINEAR", &buffer);
    let matrix_translation = f64::from_bits(1.0f64.to_bits() + 1);
    value["nodes"][1]["matrix"] = json!([
        1.0,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
        matrix_translation,
        0.0,
        0.0,
        1.0
    ]);
    let position_min = f64::from_bits(0.5f64.to_bits() - 1);
    value["accessors"][0]["min"][0] = json!(position_min);

    let source = accepted("identity.gltf", &value);
    let artifact = rewrite_linear_units(&source, 1.0).expect("rewrite");
    let (json, buffers) = artifact_parts(&artifact);
    assert_eq!(buffers[0], buffer, "q = 1 is a byte identity on payloads");
    assert_eq!(
        json["accessors"][0]["min"][0]
            .as_f64()
            .expect("numeric minimum")
            .to_bits(),
        position_min.to_bits(),
        "q = 1 preserves a non-f32-exact authored bound"
    );
    assert_eq!(json["accessors"][0]["max"], json!([2.0, 2.0, 4.0]));
    assert_eq!(
        json["nodes"][1]["matrix"][12]
            .as_f64()
            .expect("numeric matrix translation")
            .to_bits(),
        matrix_translation.to_bits(),
        "q = 1 preserves a non-f32-exact authored matrix translation"
    );
    assert!(artifact.rewritten_accessors().is_empty());
    assert!(artifact.rewritten_json_pointers().is_empty());
    assert!(artifact.reencoded_buffers().is_empty());

    let reloaded = load_bytes(Path::new("identity.gltf"), artifact.bytes()).expect("reload");
    let plan = plan_for(&source, 1.0);
    prove_scale(
        source.document(),
        &ScaleCandidate::from_document(reloaded),
        &plan,
    )
    .expect("a unit conversion proves against its own source");
    prove_rewritten_artifact(&source, &artifact, &plan)
        .expect("the raw factor-one artifact preserves exact authored values");

    let mut glb_value = value.clone();
    glb_value["buffers"][0] = json!({ "byteLength": rig::LENGTH });
    let glb_source = accepted_glb("identity.glb", &glb_value, &buffer);
    let glb_artifact = rewrite_linear_units(&glb_source, 1.0).expect("glb rewrite");
    let (_, glb_buffers) = artifact_parts(&glb_artifact);
    assert_eq!(&glb_buffers[0][..rig::LENGTH], &buffer[..]);
}

// --- 14: invalid factors ----------------------------------------------------

#[test]
fn invalid_and_unrepresentable_factors_are_typed_rejections_with_no_artifact() {
    let (value, _) = minimal_json(&[0.0; 9]);
    let source = accepted("factors.gltf", &value);

    for factor in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        match rewrite_linear_units(&source, factor) {
            Err(GltfScaleRewriteError::Plan(ScaleError::InvalidFactor { factor: reported })) => {
                assert_eq!(reported.is_nan(), factor.is_nan());
                if !factor.is_nan() {
                    assert_eq!(reported, factor);
                }
            }
            other => panic!("factor {factor} should be an InvalidFactor, got {other:?}"),
        }
    }
    match rewrite_linear_units(&source, 1.0e-50) {
        Err(GltfScaleRewriteError::Plan(ScaleError::FactorNotRepresentable {
            declared,
            factor,
            narrowed,
        })) => {
            assert_eq!(declared, 1.0e-50);
            assert_eq!(factor, 1.0e-50);
            assert_eq!(narrowed, 0.0);
        }
        other => panic!("1e-50 should be FactorNotRepresentable, got {other:?}"),
    }
}

#[test]
fn an_element_the_factor_annihilates_is_a_located_rejection() {
    // The factor itself narrows fine; the product does not.
    let (value, _) = minimal_json(&[1.0, 1.0e-30, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
    let source = accepted("annihilated.gltf", &value);
    match rewrite_linear_units(&source, 1.0e-20) {
        Err(GltfScaleRewriteError::ValueNotRepresentable { location, value }) => {
            assert_eq!(location, "/accessors/0[1]");
            assert_eq!(value, 1.0e-30f32 as f64 * 1.0e-20);
        }
        other => panic!("expected a located ValueNotRepresentable, got {other:?}"),
    }
}

// --- 15: unsupported payload refusal ----------------------------------------

#[test]
fn every_unpreservable_payload_is_refused_before_an_artifact_exists() {
    let base = || minimal_json(&[0.0; 9]).0;

    let mut external = base();
    external["buffers"][0]["uri"] = json!("payload.bin");
    assert!(
        kinds(&rejected("external.gltf", &external))
            .contains(&GltfCapabilityViolationKind::ExternalResource)
    );

    let mut extras = base();
    extras["extras"] = json!({ "vendor": "opaque" });
    assert!(
        kinds(&rejected("extras.gltf", &extras)).contains(&GltfCapabilityViolationKind::Extras)
    );

    let mut unknown = base();
    unknown["unmodeledTopLevel"] = json!(true);
    assert!(
        kinds(&rejected("unknown.gltf", &unknown))
            .contains(&GltfCapabilityViolationKind::UnknownJsonMember)
    );

    let mut mode = base();
    mode["meshes"][0]["primitives"][0]["mode"] = json!(1);
    assert!(
        kinds(&rejected("mode.gltf", &mode))
            .contains(&GltfCapabilityViolationKind::NonTrianglePrimitive)
    );

    let mut tangent = base();
    tangent["meshes"][0]["primitives"][0]["attributes"]["TANGENT"] = json!(0);
    assert!(
        kinds(&rejected("tangent.gltf", &tangent))
            .contains(&GltfCapabilityViolationKind::UnsupportedVertexAttribute)
    );

    let mut secondary = base();
    secondary["meshes"][0]["primitives"][0]["attributes"]["JOINTS_1"] = json!(0);
    assert!(
        kinds(&rejected("secondary.gltf", &secondary))
            .contains(&GltfCapabilityViolationKind::SecondarySkinInfluences)
    );

    let mut morph = base();
    morph["meshes"][0]["primitives"][0]["targets"] = json!([{ "TANGENT": 0 }]);
    assert!(
        kinds(&rejected("morph.gltf", &morph)).contains(&GltfCapabilityViolationKind::MorphTarget)
    );

    let mut interleaved = minimal_json(&[0.0; 12]).0;
    interleaved["bufferViews"][0]["byteStride"] = json!(16);
    interleaved["accessors"][0]["count"] = json!(3);
    assert!(
        kinds(&rejected("interleaved.gltf", &interleaved))
            .contains(&GltfCapabilityViolationKind::UnsafeAccessorLayout)
    );

    let mut sparse = base();
    sparse["buffers"][0] = json!({ "uri": data_uri(&[0u8; 48]), "byteLength": 48 });
    sparse["bufferViews"] = json!([
        { "buffer": 0, "byteOffset": 0, "byteLength": 36 },
        { "buffer": 0, "byteOffset": 36, "byteLength": 2 },
        { "buffer": 0, "byteOffset": 40, "byteLength": 8 }
    ]);
    sparse["accessors"][0]["sparse"] = json!({
        "count": 1,
        "indices": { "bufferView": 1, "byteOffset": 0, "componentType": 5123 },
        "values": { "bufferView": 2, "byteOffset": 0 }
    });
    assert!(
        kinds(&rejected("sparse.gltf", &sparse))
            .contains(&GltfCapabilityViolationKind::UnsafeAccessorLayout)
    );

    let mut skin = base();
    skin["nodes"] = json!([{}]);
    skin["skins"] = json!([{ "joints": [0] }]);
    assert!(
        kinds(&rejected("skin.gltf", &skin))
            .contains(&GltfCapabilityViolationKind::MissingInverseBinds)
    );
}

// --- 16: cameras, lights, extensions ---------------------------------------

#[test]
fn cameras_lights_and_extensions_are_refused_by_the_empty_handler_registry() {
    let base = || minimal_json(&[0.0; 9]).0;

    let mut camera = base();
    camera["cameras"] = json!([{ "type": "perspective", "perspective": { "yfov": 1.0, "znear": 0.1, "zfar": 100.0 } }]);
    camera["nodes"] = json!([{ "camera": 0 }]);
    assert!(
        kinds(&rejected("camera.gltf", &camera)).contains(&GltfCapabilityViolationKind::Camera)
    );

    let mut light = base();
    light["extensionsUsed"] = json!(["KHR_lights_punctual"]);
    light["extensions"] = json!({
        "KHR_lights_punctual": { "lights": [{ "type": "point", "range": 4.0, "intensity": 100.0 }] }
    });
    assert!(kinds(&rejected("light.gltf", &light)).contains(&GltfCapabilityViolationKind::Light));

    let mut extension = base();
    extension["extensionsUsed"] = json!(["ACME_units"]);
    assert!(
        kinds(&rejected("extension.gltf", &extension))
            .contains(&GltfCapabilityViolationKind::ExtensionDeclaration)
    );
}

// --- Image payload aliasing -------------------------------------------------
//
// An image payload sharing bytes with a converted accessor is refused by
// #280's preflight (#300), so no such source can be built here. The located
// preflight rejection lives in `capability_preflight.rs` and the rewriter's
// defence-in-depth guard in `scale.rs`'s unit tests. What remains here is the
// end-to-end must-not-over-reject direction, which also proves the image
// bytes really do survive the conversion untouched.

/// A 48-byte buffer holding one image payload and one `POSITION` accessor,
/// at caller-chosen offsets, so both sides of the half-open overlap test can
/// be exercised.
///
/// The `POSITION` accessor always occupies `position_offset ..
/// position_offset + 36`; the image view occupies `image_offset ..
/// image_offset + image_length`.
fn image_and_positions(image_offset: usize, image_length: usize, position_offset: usize) -> Value {
    let mut buffer = vec![0u8; 48];
    let positions = f32_bytes(&[
        1.0, 2.0, 3.0, //
        0.0, 0.0, 0.0, //
        0.0, 0.0, 0.0,
    ]);
    buffer[position_offset..position_offset + 36].copy_from_slice(&positions);
    json!({
        "asset": { "version": "2.0" },
        "buffers": [{ "uri": data_uri(&buffer), "byteLength": 48 }],
        "bufferViews": [
            { "buffer": 0, "byteOffset": image_offset, "byteLength": image_length },
            { "buffer": 0, "byteOffset": position_offset, "byteLength": 36 }
        ],
        "accessors": [{
            "bufferView": 1, "componentType": 5126, "count": 3, "type": "VEC3",
            "min": [0.0, 0.0, 0.0], "max": [1.0, 2.0, 3.0]
        }],
        "images": [{ "bufferView": 0, "mimeType": "image/png" }],
        "meshes": [{ "primitives": [{ "attributes": { "POSITION": 0 } }] }]
    })
}

#[test]
fn an_image_view_exactly_adjacent_to_a_converted_accessor_is_converted() {
    // Both ranges are half-open, so touching endpoints share no byte.
    // Refusing either case would reject the ordinary tightly-packed layout
    // every exporter emits. Both orders are pinned because the two
    // comparisons that decide it are independent.
    for (name, image_offset, image_length, position_offset) in [
        (
            "image ends where the accessor begins",
            0usize,
            12usize,
            12usize,
        ),
        ("image begins where the accessor ends", 36, 12, 0),
    ] {
        let value = image_and_positions(image_offset, image_length, position_offset);
        let source = accepted("image-adjacent.gltf", &value);
        let artifact = rewrite_linear_units(&source, 2.0)
            .unwrap_or_else(|error| panic!("{name}: adjacency is not an overlap: {error:?}"));
        let (_, buffers) = artifact_parts(&artifact);

        assert_eq!(
            &buffers[0][image_offset..image_offset + image_length],
            &[0u8; 12],
            "{name}: image bytes are untouched"
        );
        assert_eq!(
            read_f32(&buffers[0][position_offset..position_offset + 12]),
            vec![2.0, 4.0, 6.0],
            "{name}: the accessor still converts"
        );
    }
}

// --- 17: the artifact proof -------------------------------------------------

#[test]
fn the_artifact_proof_passes_for_gltf_and_glb_and_reports_its_evidence() {
    let buffer = rig_buffer("CUBICSPLINE");
    let value = rig_json("CUBICSPLINE", &buffer);

    let source = accepted("proof.gltf", &value);
    let plan = plan_for(&source, 4.0);
    let artifact = rewrite_linear_units(&source, 4.0).expect("rewrite");
    let proof = prove_rewritten_artifact(&source, &artifact, &plan).expect("artifact proof");

    assert_eq!(
        proof.rewritten_accessor_count, 3,
        "POSITION, IBM, translation"
    );
    assert_eq!(proof.dimensionless_residual, 0.0);
    assert_eq!(
        proof.length_factor_residual, 0.0,
        "every fixture value is exact under a factor of four"
    );
    // POSITION 0..36, IBM 168..232 and translation 240..312 are converted, so
    // the preserved complement is 36..168, 232..240 and 312..374.
    assert_eq!(proof.preserved_byte_ranges, 3);
    assert_eq!(proof.core.tolerance_policy, plan.tolerance_policy());

    let mut glb_value = value.clone();
    glb_value["buffers"][0] = json!({ "byteLength": rig::LENGTH });
    let glb_source = accepted_glb("proof.glb", &glb_value, &buffer);
    let glb_plan = plan_for(&glb_source, 4.0);
    let glb_artifact = rewrite_linear_units(&glb_source, 4.0).expect("glb rewrite");
    let glb_proof = prove_rewritten_artifact(&glb_source, &glb_artifact, &glb_plan)
        .expect("glb artifact proof");
    assert_eq!(glb_proof.rewritten_accessor_count, 3);
    // The GLB BIN chunk pads 374 bytes to 376, so the trailing pad joins the
    // final preserved range rather than adding one.
    assert_eq!(glb_proof.preserved_byte_ranges, 3);
}

#[test]
fn the_artifact_proof_refuses_a_plan_whose_factor_is_not_the_artifacts() {
    let (value, _) = minimal_json(&[1.0, 2.0, 3.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
    let source = accepted("mismatch.gltf", &value);
    let artifact = rewrite_linear_units(&source, 2.0).expect("rewrite");
    let plan = plan_for(&source, 4.0);
    match prove_rewritten_artifact(&source, &artifact, &plan) {
        Err(GltfScaleRewriteError::ArtifactProofFailed {
            claim,
            observed,
            tolerance,
            raw_json_differences,
        }) => {
            assert_eq!(claim, "plan factor equals the artifact's declared factor");
            assert_eq!(observed, 2.0);
            assert_eq!(tolerance, 0.0);
            assert_eq!(raw_json_differences, None);
        }
        other => panic!("expected ArtifactProofFailed, got {other:?}"),
    }
}

#[test]
fn the_full_composition_proves_both_layers_for_a_skinned_animated_source() {
    let buffer = rig_buffer("LINEAR");
    let value = rig_json("LINEAR", &buffer);
    let source = accepted("composition.gltf", &value);
    let factor = 0.01;

    let facts = capability_facts(source.manifest());
    assert!(facts.is_supported());
    let plan = plan_scale(&ScaleRequest {
        operation: ScaleOperation::WholeDocumentLinearUnits { factor },
        document: source.document(),
        capability: &facts,
    })
    .expect("plan");
    let artifact = rewrite_linear_units(&source, factor).expect("rewrite");
    let planned_artifact = rewrite_scale_plan(&source, &plan).expect("planned rewrite");
    assert_eq!(planned_artifact.bytes(), artifact.bytes());
    let reloaded = load_bytes(Path::new("composition.gltf"), artifact.bytes()).expect("reload");
    // Mesh-instance *placement* identity, read off the reloaded artifact
    // rather than assumed from the rewriter's shape. The conversion clones
    // the source JSON and patches numbers in place, so `nodes`, `children`,
    // `scenes` and every mesh/skin attachment come through untouched and the
    // loader re-derives the same bone ids from the same DFS. The mesh holder
    // is node 1 on both sides.
    assert_eq!(
        instance_identity(&reloaded),
        instance_identity(source.document())
    );
    assert_eq!(instance_identity(&reloaded), vec![(1, 1, 0, vec![0])]);
    let core = prove_scale(
        source.document(),
        &ScaleCandidate::from_document(reloaded),
        &plan,
    )
    .expect("in-memory proof");
    let artifact_proof =
        prove_rewritten_artifact(&source, &artifact, &plan).expect("artifact proof");
    assert_eq!(artifact_proof.core, core);

    // A hand-computed spot check of the non-power-of-two factor: the mesh's
    // first vertex is (1.0, 2.0, -3.0) metres-as-centimetres.
    let (_, buffers) = artifact_parts(&artifact);
    assert_eq!(
        read_f32(&buffers[0][rig::POSITION..rig::POSITION + 12]),
        vec![0.01, 0.02, -0.03]
    );
    assert_eq!(
        read_f32(&buffers[0][rig::INVERSE_BIND + 48..rig::INVERSE_BIND + 60]),
        vec![-0.01, 0.02, -0.005]
    );
}
