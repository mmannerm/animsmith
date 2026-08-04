//! Public raw-capability preflight contract tests.

use animsmith_gltf::{
    GltfBufferSourceKind, GltfCapabilityViolation, GltfCapabilityViolationKind, GltfContainerKind,
    GltfNodeRestKind, GltfScalePreflightError, preflight_scale_source,
    preflight_scale_source_bytes,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use std::path::Path;

const ZERO_F32X9: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
const ZERO_F32X12: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
const ZERO_F32X16: &str =
    "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA==";

fn base_json() -> Value {
    json!({
        "asset": { "version": "2.0" },
        "buffers": [{
            "uri": format!("data:application/octet-stream;base64,{ZERO_F32X9}"),
            "byteLength": 36
        }],
        "bufferViews": [{ "buffer": 0, "byteLength": 36 }],
        "accessors": [{
            "bufferView": 0,
            "componentType": 5126,
            "count": 3,
            "type": "VEC3",
            "min": [0, 0, 0],
            "max": [0, 0, 0]
        }],
        "meshes": [{ "primitives": [{ "attributes": { "POSITION": 0 } }] }]
    })
}

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

fn unsupported(
    value: &Value,
) -> (
    Vec<GltfCapabilityViolation>,
    animsmith_gltf::GltfCapabilityManifest,
) {
    match preflight_scale_source_bytes(Path::new("synthetic.gltf"), &bytes(value)) {
        Err(GltfScalePreflightError::Unsupported {
            manifest,
            violations,
            count,
        }) => {
            assert_eq!(count, violations.len());
            (violations, *manifest)
        }
        other => panic!("expected typed capability rejection, got {other:?}"),
    }
}

fn assert_has(violations: &[GltfCapabilityViolation], kind: GltfCapabilityViolationKind) {
    assert!(
        violations.iter().any(|violation| violation.kind == kind),
        "missing {kind:?} in {violations:#?}"
    );
}

#[test]
fn accepts_self_contained_gltf_and_equivalent_glb_without_writing() {
    let mut gltf = base_json();
    gltf["nodes"] = json!([{ "translation": [2, 0, 0] }]);
    let gltf_bytes = bytes(&gltf);
    let directory = tempfile::tempdir().expect("temporary provenance directory");
    let input = directory.path().join("input.gltf");

    let source = preflight_scale_source_bytes(&input, &gltf_bytes).expect("data-URI glTF accepted");
    assert_eq!(source.source_bytes(), gltf_bytes);
    assert_eq!(source.raw_json(), &gltf);
    assert_eq!(source.document().skeleton.bones[0].rest.translation.x, 2.0);
    assert_eq!(source.manifest().container, GltfContainerKind::Gltf);
    assert_eq!(
        source.manifest().buffers[0].source_kind,
        GltfBufferSourceKind::DataUri
    );
    assert_eq!(source.resolved_buffers(), &[vec![0; 36]]);
    assert_eq!(source.manifest().buffers[0].buffer_index, 0);
    assert_eq!(source.manifest().buffers[0].declared_byte_length, 36);
    assert_eq!(source.manifest().buffer_views[0].buffer_index, 0);
    assert_eq!(source.manifest().buffer_views[0].byte_offset, 0);
    assert_eq!(source.manifest().buffer_views[0].byte_length, 36);
    assert_eq!(source.manifest().buffer_views[0].byte_stride, None);
    assert_eq!(source.manifest().accessors[0].buffer_view_index, Some(0));
    assert_eq!(source.manifest().accessors[0].byte_offset, 0);
    assert_eq!(source.manifest().accessors[0].component_type, 5126);
    assert_eq!(source.manifest().accessors[0].accessor_type, "VEC3");
    assert_eq!(source.manifest().accessors[0].count, 3);
    assert!(!source.manifest().accessors[0].normalized);
    assert!(!source.manifest().accessors[0].sparse);
    assert_eq!(
        source.manifest().primitives[0].attributes[0].semantic,
        "POSITION"
    );
    assert_eq!(
        source.manifest().primitives[0].attributes[0].accessor_index,
        0
    );
    assert!(
        std::fs::read_dir(directory.path())
            .unwrap()
            .next()
            .is_none(),
        "preflight creates no output"
    );

    let mut glb_json = base_json();
    glb_json["buffers"][0] = json!({ "byteLength": 36 });
    let glb_bytes = glb(&glb_json, &[0; 36]);
    let glb_source = preflight_scale_source_bytes(Path::new("synthetic.glb"), &glb_bytes)
        .expect("equivalent GLB accepted");
    assert_eq!(glb_source.manifest().container, GltfContainerKind::Glb);
    assert_eq!(glb_source.source_bytes(), glb_bytes);
    assert_eq!(
        glb_source.manifest().buffers[0].source_kind,
        GltfBufferSourceKind::BinaryChunk
    );
    assert_eq!(glb_source.resolved_buffers(), &[vec![0; 36]]);
}

#[test]
fn accepts_every_primary_attribute_with_distinct_valid_accessor_storage() {
    let mut value = base_json();
    let mut buffer = vec![0; 176];
    buffer[168..174].copy_from_slice(&[0, 0, 1, 0, 2, 0]);
    value["buffers"][0] = json!({ "uri": data_uri(&buffer), "byteLength": 176 });
    value["bufferViews"] = json!([
        { "buffer": 0, "byteOffset": 0, "byteLength": 36 },
        { "buffer": 0, "byteOffset": 36, "byteLength": 36 },
        { "buffer": 0, "byteOffset": 72, "byteLength": 24 },
        { "buffer": 0, "byteOffset": 96, "byteLength": 24 },
        { "buffer": 0, "byteOffset": 120, "byteLength": 48 },
        { "buffer": 0, "byteOffset": 168, "byteLength": 6 }
    ]);
    value["accessors"] = json!([
        { "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3", "min": [0, 0, 0], "max": [0, 0, 0] },
        { "bufferView": 1, "componentType": 5126, "count": 3, "type": "VEC3" },
        { "bufferView": 2, "componentType": 5126, "count": 3, "type": "VEC2" },
        { "bufferView": 3, "componentType": 5123, "count": 3, "type": "VEC4" },
        { "bufferView": 4, "componentType": 5126, "count": 3, "type": "VEC4" },
        { "bufferView": 5, "componentType": 5123, "count": 3, "type": "SCALAR" }
    ]);
    value["meshes"][0]["primitives"][0] = json!({
        "attributes": {
            "WEIGHTS_0": 4,
            "POSITION": 0,
            "TEXCOORD_0": 2,
            "NORMAL": 1,
            "JOINTS_0": 3
        },
        "indices": 5
    });

    let source = preflight_scale_source_bytes(Path::new("primary.gltf"), &bytes(&value))
        .expect("the complete primary attribute subset is accepted");

    assert_eq!(
        source.manifest().primitives[0]
            .attributes
            .iter()
            .map(|attribute| (attribute.semantic.as_str(), attribute.accessor_index))
            .collect::<Vec<_>>(),
        vec![
            ("JOINTS_0", 3),
            ("NORMAL", 1),
            ("POSITION", 0),
            ("TEXCOORD_0", 2),
            ("WEIGHTS_0", 4),
        ]
    );
    assert_eq!(source.resolved_buffers(), &[buffer]);
}

#[test]
fn accepts_dense_inverse_binds_and_ordinary_translation_animation() {
    let mut value = base_json();
    let mut buffer = vec![0; 132];
    for matrix_component in [0usize, 5, 10, 15] {
        let offset = 36 + matrix_component * 4;
        buffer[offset..offset + 4].copy_from_slice(&1.0f32.to_le_bytes());
    }
    buffer[104..108].copy_from_slice(&1.0f32.to_le_bytes());
    value["buffers"][0] = json!({ "uri": data_uri(&buffer), "byteLength": 132 });
    value["bufferViews"] = json!([
        { "buffer": 0, "byteOffset": 0, "byteLength": 36 },
        { "buffer": 0, "byteOffset": 36, "byteLength": 64 },
        { "buffer": 0, "byteOffset": 100, "byteLength": 8 },
        { "buffer": 0, "byteOffset": 108, "byteLength": 24 }
    ]);
    value["accessors"] = json!([
        { "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3", "min": [0, 0, 0], "max": [0, 0, 0] },
        { "bufferView": 1, "componentType": 5126, "count": 1, "type": "MAT4" },
        { "bufferView": 2, "componentType": 5126, "count": 2, "type": "SCALAR", "min": [0], "max": [1] },
        { "bufferView": 3, "componentType": 5126, "count": 2, "type": "VEC3" }
    ]);
    value["nodes"] = json!([{}, { "mesh": 0, "skin": 0 }]);
    value["skins"] = json!([{
        "joints": [0],
        "skeleton": 0,
        "inverseBindMatrices": 1
    }]);
    value["animations"] = json!([{
        "samplers": [{ "input": 2, "interpolation": "LINEAR", "output": 3 }],
        "channels": [{ "sampler": 0, "target": { "node": 0, "path": "translation" } }]
    }]);

    let source = preflight_scale_source_bytes(Path::new("skinned.gltf"), &bytes(&value))
        .expect("ordinary dense skin and translation animation are supported");

    assert_eq!(source.manifest().skins[0].skin_index, 0);
    assert_eq!(source.manifest().skins[0].joint_count, 1);
    assert_eq!(
        source.manifest().skins[0].inverse_bind_accessor_index,
        Some(1)
    );
    assert_eq!(source.manifest().skins[0].inverse_bind_count, Some(1));
    assert_eq!(source.manifest().nodes[1].skin_index, Some(0));
    assert_eq!(source.manifest().animation_channels[0].target_node_index, 0);
    assert_eq!(
        source.manifest().animation_channels[0].target_path,
        "translation"
    );
    assert_eq!(
        source.manifest().animation_channels[0].interpolation,
        "LINEAR"
    );
    assert_eq!(
        source.manifest().animation_channels[0].input_accessor_index,
        2
    );
    assert_eq!(
        source.manifest().animation_channels[0].output_accessor_index,
        3
    );
}

#[test]
fn filesystem_api_captures_nonzero_bytes_without_reopening_external_resources() {
    let mut value = base_json();
    let mut buffer = vec![0; 36];
    buffer[0..4].copy_from_slice(&1.0f32.to_le_bytes());
    value["buffers"][0]["uri"] = json!(data_uri(&buffer));
    let source_bytes = bytes(&value);
    let directory = tempfile::tempdir().expect("temporary source directory");
    let input = directory.path().join("source.gltf");
    std::fs::write(&input, &source_bytes).expect("write synthetic source");

    let source = preflight_scale_source(&input).expect("filesystem preflight accepts source");

    assert_eq!(source.source_bytes(), source_bytes);
    assert_eq!(source.resolved_buffers(), &[buffer]);
    assert_eq!(std::fs::read(&input).unwrap(), source_bytes);
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[test]
fn inventories_trs_and_matrix_nodes_from_raw_json() {
    let mut value = base_json();
    value["nodes"] = json!([
        { "translation": [1, 2, 3], "mesh": 0 },
        { "matrix": [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1] }
    ]);
    let source = preflight_scale_source_bytes(Path::new("nodes.gltf"), &bytes(&value))
        .expect("TRS and matrix are both inventoried");
    assert_eq!(
        source
            .manifest()
            .nodes
            .iter()
            .map(|node| node.rest_kind)
            .collect::<Vec<_>>(),
        vec![GltfNodeRestKind::Trs, GltfNodeRestKind::Matrix]
    );
    assert_eq!(source.manifest().nodes[0].node_index, 0);
    assert_eq!(source.manifest().nodes[0].mesh_index, Some(0));
    assert_eq!(source.manifest().nodes[0].skin_index, None);
    assert_eq!(source.manifest().nodes[1].node_index, 1);
}

#[test]
fn rejects_raw_extras_extensions_and_unknown_members() {
    let mut value = base_json();
    value["extras"] = json!({ "opaque": true });
    value["extensionsUsed"] = json!(["ACME_opaque"]);
    value["extensions"] = json!({ "ACME_opaque": { "preserve": "me" } });
    value["unmodeledTopLevel"] = json!(true);
    value["a/b~c"] = json!(true);
    value["nodes"] = json!([{ "extras": { "nested": true }, "unmodeledNode": 7 }]);
    value["materials"] = json!([{ "extras": { "nested": true } }]);
    let (violations, manifest) = unsupported(&value);
    assert_has(&violations, GltfCapabilityViolationKind::Extras);
    assert_has(
        &violations,
        GltfCapabilityViolationKind::ExtensionDeclaration,
    );
    assert_has(&violations, GltfCapabilityViolationKind::ExtensionPayload);
    assert_has(&violations, GltfCapabilityViolationKind::UnknownJsonMember);
    assert_eq!(manifest.extensions, vec!["ACME_opaque"]);
    assert_eq!(
        manifest.extras_locations,
        vec!["/extras", "/materials/0/extras", "/nodes/0/extras"]
    );
    assert_eq!(
        manifest.unknown_member_locations,
        vec!["/a~1b~0c", "/nodes/0/unmodeledNode", "/unmodeledTopLevel"]
    );
    assert_eq!(
        violations
            .iter()
            .filter(|violation| violation.kind == GltfCapabilityViolationKind::Extras)
            .map(|violation| violation.location.as_str())
            .collect::<Vec<_>>(),
        vec!["/extras", "/materials/0/extras", "/nodes/0/extras"]
    );

    let mut required_conflict = base_json();
    required_conflict["extensionsRequired"] = json!(["ACME_required"]);
    required_conflict["buffers"][0] = json!({
        "uri": format!("data:application/octet-stream;base64,{ZERO_F32X12}"),
        "byteLength": 48
    });
    required_conflict["bufferViews"][0] =
        json!({ "buffer": 0, "byteLength": 48, "byteStride": 16 });
    required_conflict["meshes"][0]["primitives"][0]["attributes"] =
        json!({ "POSITION": 0, "NORMAL": 0 });
    let (violations, _) = unsupported(&required_conflict);
    assert_has(
        &violations,
        GltfCapabilityViolationKind::ExtensionDeclaration,
    );
    assert_has(
        &violations,
        GltfCapabilityViolationKind::ConflictingAccessorUse,
    );
    assert_has(
        &violations,
        GltfCapabilityViolationKind::UnsafeAccessorLayout,
    );
}

#[test]
fn inventories_and_rejects_gpu_instancing_with_stable_accessor_identities() {
    let mut value = base_json();
    value["extensionsUsed"] = json!(["EXT_mesh_gpu_instancing"]);
    value["nodes"] = json!([{
        "mesh": 0,
        "extensions": {
            "EXT_mesh_gpu_instancing": {
                "attributes": { "TRANSLATION": 0, "SCALE": 0 }
            }
        }
    }]);

    let (violations, manifest) = unsupported(&value);

    assert_eq!(
        manifest.instancing[0]
            .attributes
            .iter()
            .map(|attribute| (attribute.semantic.as_str(), attribute.accessor_index))
            .collect::<Vec<_>>(),
        vec![("SCALE", 0), ("TRANSLATION", 0)]
    );
    assert_eq!(manifest.instancing[0].node_index, 0);
    assert_eq!(
        violations
            .iter()
            .filter(|violation| violation.kind == GltfCapabilityViolationKind::Instancing)
            .map(|violation| violation.location.as_str())
            .collect::<Vec<_>>(),
        vec![
            "/extensionsUsed/0",
            "/nodes/0/extensions/EXT_mesh_gpu_instancing"
        ]
    );
}

#[test]
fn rejects_external_resources_and_punctual_lights_before_resolution() {
    let mut value = base_json();
    value["buffers"][0]["uri"] = json!("missing.bin");
    value["images"] = json!([{ "uri": "missing.png" }]);
    value["extensionsUsed"] = json!(["KHR_lights_punctual"]);
    value["extensions"] = json!({
        "KHR_lights_punctual": { "lights": [{ "type": "point", "range": 4.0 }] }
    });
    let (violations, manifest) = unsupported(&value);
    assert_has(&violations, GltfCapabilityViolationKind::ExternalResource);
    assert_has(&violations, GltfCapabilityViolationKind::Light);
    assert_eq!(
        manifest.external_resource_locations,
        vec!["/buffers/0/uri", "/images/0/uri"]
    );
    assert_eq!(
        manifest.extension_locations,
        vec!["/extensions/KHR_lights_punctual"]
    );
    assert_eq!(
        violations
            .iter()
            .filter(|violation| violation.kind == GltfCapabilityViolationKind::Light)
            .map(|violation| violation.location.as_str())
            .collect::<Vec<_>>(),
        vec!["/extensions/KHR_lights_punctual", "/extensionsUsed/0"]
    );

    let mut declaration_only = base_json();
    declaration_only["extensionsRequired"] = json!(["KHR_lights_punctual"]);
    let (violations, _) = unsupported(&declaration_only);
    assert_eq!(
        violations
            .iter()
            .filter(|violation| violation.kind == GltfCapabilityViolationKind::Light)
            .map(|violation| violation.location.as_str())
            .collect::<Vec<_>>(),
        vec!["/extensionsRequired/0"]
    );

    let mut payload_only = base_json();
    payload_only["extensions"] = json!({ "KHR_lights_punctual": { "lights": [] } });
    let (violations, _) = unsupported(&payload_only);
    assert_eq!(
        violations
            .iter()
            .filter(|violation| violation.kind == GltfCapabilityViolationKind::Light)
            .map(|violation| violation.location.as_str())
            .collect::<Vec<_>>(),
        vec!["/extensions/KHR_lights_punctual"]
    );

    let mut external_image = base_json();
    external_image["images"] = json!([{ "uri": "missing.png" }]);
    external_image["buffers"][0] = json!({
        "uri": format!("data:application/octet-stream;base64,{ZERO_F32X12}"),
        "byteLength": 48
    });
    external_image["bufferViews"][0] = json!({ "buffer": 0, "byteLength": 48, "byteStride": 16 });
    let (violations, _) = unsupported(&external_image);
    assert_has(&violations, GltfCapabilityViolationKind::ExternalResource);
    assert_has(
        &violations,
        GltfCapabilityViolationKind::UnsafeAccessorLayout,
    );
}

#[test]
fn rejects_morphs_cameras_modes_attributes_and_secondary_influences() {
    let mut value = base_json();
    value["meshes"] = json!([
        {
            "weights": [0.5],
            "primitives": [{
                "mode": 1,
                "attributes": { "POSITION": 0, "COLOR_0": 0, "JOINTS_1": 0, "WEIGHTS_1": 0 },
                "targets": [{ "POSITION": 0 }, { "POSITION": 0 }]
            }]
        },
        {
            "primitives": [{
                "mode": 0,
                "attributes": { "POSITION": 0, "TANGENT": 0, "JOINTS_2": 0, "WEIGHTS_2": 0 }
            }]
        }
    ]);
    value["nodes"] = json!([{ "mesh": 0, "weights": [0.25], "camera": 0 }]);
    value["cameras"] =
        json!([{ "type": "perspective", "perspective": { "yfov": 1.0, "znear": 0.1 } }]);
    let (violations, manifest) = unsupported(&value);
    for kind in [
        GltfCapabilityViolationKind::MorphTarget,
        GltfCapabilityViolationKind::MorphWeights,
        GltfCapabilityViolationKind::Camera,
        GltfCapabilityViolationKind::NonTrianglePrimitive,
        GltfCapabilityViolationKind::UnsupportedVertexAttribute,
        GltfCapabilityViolationKind::SecondarySkinInfluences,
    ] {
        assert_has(&violations, kind);
    }
    assert_eq!(manifest.primitives[0].mode, 1);
    assert_eq!(manifest.primitives[0].morph_target_count, 2);
    assert_eq!(manifest.primitives[0].morph_position_accessors, vec![0, 0]);
    assert_eq!(manifest.primitives[1].mesh_index, 1);
    assert_eq!(manifest.primitives[1].primitive_index, 0);
    assert_eq!(manifest.primitives[1].mode, 0);
    assert_eq!(manifest.camera_count, 1);
    assert_eq!(
        violations
            .iter()
            .filter(|violation| violation.kind == GltfCapabilityViolationKind::Camera)
            .map(|violation| violation.location.as_str())
            .collect::<Vec<_>>(),
        vec!["/cameras", "/nodes/0/camera"]
    );
    assert_eq!(
        violations
            .iter()
            .filter(|violation| {
                violation.kind == GltfCapabilityViolationKind::NonTrianglePrimitive
            })
            .map(|violation| violation.location.as_str())
            .collect::<Vec<_>>(),
        vec!["/meshes/0/primitives/0/mode", "/meshes/1/primitives/0/mode"]
    );
    assert_eq!(
        violations
            .iter()
            .filter(|violation| {
                violation.kind == GltfCapabilityViolationKind::SecondarySkinInfluences
            })
            .map(|violation| violation.location.as_str())
            .collect::<Vec<_>>(),
        vec![
            "/meshes/0/primitives/0/attributes/JOINTS_1",
            "/meshes/0/primitives/0/attributes/WEIGHTS_1",
            "/meshes/1/primitives/0/attributes/JOINTS_2",
            "/meshes/1/primitives/0/attributes/WEIGHTS_2"
        ]
    );
}

#[test]
fn rejects_missing_empty_and_count_mismatched_inverse_binds() {
    let mut missing = base_json();
    missing["meshes"] = json!([]);
    missing["nodes"] = json!([{}, {}]);
    missing["skins"] = json!([{ "joints": [0, 1] }]);
    let (violations, manifest) = unsupported(&missing);
    assert_has(
        &violations,
        GltfCapabilityViolationKind::MissingInverseBinds,
    );
    assert_eq!(manifest.skins[0].skin_index, 0);
    assert_eq!(manifest.skins[0].joint_count, 2);
    assert_eq!(manifest.skins[0].inverse_bind_accessor_index, None);

    let mut empty = base_json();
    empty["meshes"] = json!([]);
    empty["nodes"] = json!([{}]);
    empty["bufferViews"] = json!([{ "buffer": 0, "byteLength": 0 }]);
    empty["accessors"] =
        json!([{ "bufferView": 0, "componentType": 5126, "count": 0, "type": "MAT4" }]);
    empty["skins"] = json!([{ "joints": [0], "inverseBindMatrices": 0 }]);
    let (violations, manifest) = unsupported(&empty);
    assert_has(
        &violations,
        GltfCapabilityViolationKind::EmptyInverseBindAccessor,
    );
    assert_eq!(manifest.skins[0].inverse_bind_count, Some(0));

    let mut mismatch = base_json();
    mismatch["meshes"] = json!([]);
    mismatch["buffers"][0]["uri"] = json!(format!(
        "data:application/octet-stream;base64,{ZERO_F32X16}"
    ));
    mismatch["buffers"][0]["byteLength"] = json!(64);
    mismatch["bufferViews"][0]["byteLength"] = json!(64);
    mismatch["nodes"] = json!([{}, {}]);
    mismatch["accessors"] =
        json!([{ "bufferView": 0, "componentType": 5126, "count": 1, "type": "MAT4" }]);
    mismatch["skins"] = json!([{ "joints": [0, 1], "inverseBindMatrices": 0 }]);
    let (violations, manifest) = unsupported(&mismatch);
    assert_has(
        &violations,
        GltfCapabilityViolationKind::InverseBindCountMismatch,
    );
    assert_eq!(manifest.skins[0].inverse_bind_accessor_index, Some(0));
    assert_eq!(manifest.skins[0].inverse_bind_count, Some(1));

    let mut oversized = base_json();
    oversized["meshes"] = json!([]);
    oversized["buffers"][0] = json!({ "uri": data_uri(&[0; 128]), "byteLength": 128 });
    oversized["bufferViews"][0]["byteLength"] = json!(128);
    oversized["nodes"] = json!([{}]);
    oversized["accessors"] = json!([
        { "bufferView": 0, "componentType": 5126, "count": 2, "type": "MAT4" }
    ]);
    oversized["skins"] = json!([{ "joints": [0], "inverseBindMatrices": 0 }]);
    let (violations, manifest) = unsupported(&oversized);
    assert_has(
        &violations,
        GltfCapabilityViolationKind::InverseBindCountMismatch,
    );
    assert_eq!(manifest.skins[0].inverse_bind_count, Some(2));
}

#[test]
fn rejects_unsafe_shared_and_overlapping_scale_accessor_layouts() {
    let mut unsafe_layout = base_json();
    unsafe_layout["buffers"][0]["uri"] = json!(format!(
        "data:application/octet-stream;base64,{ZERO_F32X12}"
    ));
    unsafe_layout["buffers"][0]["byteLength"] = json!(48);
    unsafe_layout["bufferViews"][0] = json!({ "buffer": 0, "byteLength": 48, "byteStride": 16 });
    let (violations, _) = unsupported(&unsafe_layout);
    assert_has(
        &violations,
        GltfCapabilityViolationKind::UnsafeAccessorLayout,
    );

    let mut shared = base_json();
    shared["meshes"][0]["primitives"][0]["attributes"] = json!({ "POSITION": 0, "NORMAL": 0 });
    let (violations, _) = unsupported(&shared);
    assert_has(
        &violations,
        GltfCapabilityViolationKind::ConflictingAccessorUse,
    );

    let mut overlapping = base_json();
    overlapping["accessors"] = json!([
        { "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3", "min": [0, 0, 0], "max": [0, 0, 0] },
        { "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3", "min": [0, 0, 0], "max": [0, 0, 0] }
    ]);
    overlapping["meshes"] = json!([
        { "primitives": [{ "attributes": { "POSITION": 0 } }] },
        { "primitives": [{ "attributes": { "POSITION": 1 } }] }
    ]);
    let (violations, _) = unsupported(&overlapping);
    assert_has(
        &violations,
        GltfCapabilityViolationKind::OverlappingAccessorRanges,
    );

    let mut cross_domain_overlap = base_json();
    cross_domain_overlap["accessors"] = json!([
        { "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3", "min": [0, 0, 0], "max": [0, 0, 0] },
        { "bufferView": 0, "componentType": 5123, "count": 3, "type": "SCALAR" }
    ]);
    cross_domain_overlap["meshes"][0]["primitives"][0]["indices"] = json!(1);
    let (violations, _) = unsupported(&cross_domain_overlap);
    assert_has(
        &violations,
        GltfCapabilityViolationKind::OverlappingAccessorRanges,
    );

    let mut masked_overlap = base_json();
    masked_overlap["buffers"][0] = json!({ "uri": data_uri(&[0; 1204]), "byteLength": 1204 });
    masked_overlap["bufferViews"] = json!([
        { "buffer": 0, "byteOffset": 0, "byteLength": 900 },
        { "buffer": 0, "byteOffset": 4, "byteLength": 1200 },
        { "buffer": 0, "byteOffset": 800, "byteLength": 24 }
    ]);
    masked_overlap["accessors"] = json!([
        { "bufferView": 0, "componentType": 5126, "count": 75, "type": "VEC3", "min": [0, 0, 0], "max": [0, 0, 0] },
        { "bufferView": 1, "componentType": 5126, "count": 100, "type": "VEC3" },
        { "bufferView": 2, "componentType": 5126, "count": 3, "type": "VEC2" }
    ]);
    masked_overlap["meshes"][0]["primitives"][0]["attributes"] =
        json!({ "POSITION": 0, "NORMAL": 1, "TEXCOORD_0": 2 });
    let (violations, _) = unsupported(&masked_overlap);
    assert_eq!(
        violations
            .iter()
            .filter(|violation| {
                violation.kind == GltfCapabilityViolationKind::OverlappingAccessorRanges
            })
            .map(|violation| violation.location.as_str())
            .collect::<Vec<_>>(),
        vec!["/accessors/0", "/accessors/1", "/accessors/2"]
    );
}

#[test]
fn rejects_unreadable_inverse_binds_and_animated_morph_weights() {
    let mut value = base_json();
    value["meshes"] = json!([]);
    value["nodes"] = json!([{}]);
    value["accessors"] = json!([
        { "bufferView": 0, "componentType": 5126, "count": 1, "type": "VEC4" },
        { "bufferView": 0, "componentType": 5126, "count": 1, "type": "SCALAR" }
    ]);
    value["skins"] = json!([{ "joints": [0], "inverseBindMatrices": 0 }]);
    value["animations"] = json!([{
        "samplers": [{ "input": 1, "interpolation": "CUBICSPLINE", "output": 1 }],
        "channels": [{ "sampler": 0, "target": { "node": 0, "path": "weights" } }]
    }]);
    let (violations, manifest) = unsupported(&value);
    assert_has(
        &violations,
        GltfCapabilityViolationKind::UnreadableInverseBinds,
    );
    assert_has(&violations, GltfCapabilityViolationKind::MorphWeights);
    assert_eq!(manifest.animation_channels[0].animation_index, 0);
    assert_eq!(manifest.animation_channels[0].channel_index, 0);
    assert_eq!(manifest.animation_channels[0].target_node_index, 0);
    assert_eq!(manifest.animation_channels[0].target_path, "weights");
    assert_eq!(manifest.animation_channels[0].interpolation, "CUBICSPLINE");
    assert_eq!(manifest.animation_channels[0].input_accessor_index, 1);
    assert_eq!(manifest.animation_channels[0].output_accessor_index, 1);
    assert_eq!(manifest.accessors[0].accessor_type, "VEC4");
}

#[test]
fn multiple_violations_are_typed_sorted_and_repeatable() {
    let mut value = base_json();
    value["extras"] = json!({ "unhandled": true });
    value["meshes"][0]["primitives"][0]["mode"] = json!(5);
    value["nodes"] = json!([{ "camera": 0 }]);
    value["cameras"] = json!([{ "type": "orthographic", "orthographic": { "xmag": 1, "ymag": 1, "zfar": 1, "znear": 0 } }]);
    let (first, first_manifest) = unsupported(&value);
    let (second, second_manifest) = unsupported(&value);
    assert_eq!(first, second);
    assert_eq!(first_manifest, second_manifest);
    assert!(
        first.windows(2).all(|pair| pair[0] <= pair[1]),
        "violations are stable: {first:#?}"
    );
    for kind in [
        GltfCapabilityViolationKind::Camera,
        GltfCapabilityViolationKind::Extras,
        GltfCapabilityViolationKind::NonTrianglePrimitive,
    ] {
        assert_has(&first, kind);
    }
}

#[test]
fn reordered_source_arrays_retain_deterministic_source_order_and_identity() {
    let mut value = base_json();
    value["nodes"] = json!([
        { "translation": [1, 0, 0], "extras": { "source": "first" } },
        { "matrix": [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1], "extras": { "source": "second" } }
    ]);
    value["nodes"].as_array_mut().unwrap().reverse();

    let (first_violations, first_manifest) = unsupported(&value);
    let (second_violations, second_manifest) = unsupported(&value);

    assert_eq!(first_violations, second_violations);
    assert_eq!(first_manifest, second_manifest);
    assert_eq!(
        first_manifest
            .nodes
            .iter()
            .map(|node| (node.node_index, node.rest_kind))
            .collect::<Vec<_>>(),
        vec![(0, GltfNodeRestKind::Matrix), (1, GltfNodeRestKind::Trs)]
    );
    assert_eq!(
        first_manifest.extras_locations,
        vec!["/nodes/0/extras", "/nodes/1/extras"]
    );
}

#[test]
fn malformed_topology_wins_over_unsupported_domains_before_inventory_returns() {
    let mut value = base_json();
    value["extras"] = json!({ "unsupported": true });
    value["extensionsRequired"] = json!(["ACME_required"]);
    value["nodes"] = json!([
        { "children": [2] },
        { "children": [2] },
        {}
    ]);

    let error = preflight_scale_source_bytes(Path::new("malformed.gltf"), &bytes(&value))
        .expect_err("duplicate-parent topology is malformed");

    assert!(matches!(error, GltfScalePreflightError::Load(_)));
}

#[test]
fn hostile_glb_length_is_a_load_error_without_allocating_declared_size() {
    let mut glb_bytes = Vec::new();
    glb_bytes.extend_from_slice(b"glTF");
    glb_bytes.extend_from_slice(&2u32.to_le_bytes());
    glb_bytes.extend_from_slice(&u32::MAX.to_le_bytes());
    let error = preflight_scale_source_bytes(Path::new("hostile.glb"), &glb_bytes)
        .expect_err("spoofed GLB length is rejected");
    assert!(matches!(error, GltfScalePreflightError::Load(_)));

    let mut hostile_accessor = base_json();
    hostile_accessor["accessors"][0]["count"] = json!(u64::MAX);
    let result = preflight_scale_source_bytes(
        Path::new("hostile-accessor.gltf"),
        &bytes(&hostile_accessor),
    );
    assert!(result.is_err(), "hostile accessor count must fail closed");
}
