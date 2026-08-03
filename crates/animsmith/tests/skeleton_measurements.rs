//! Black-box source-skeleton measurement coverage using deliberately raw,
//! synthetic glTF fixtures. The normal writer creates generated skins, so it
//! cannot prove source skin identity, shared joints, or inverse-bind absence.

use serde_json::{Value, json};

const MEASUREMENTS_SCHEMA: &str = include_str!("../../../docs/schemas/measurements-v5.schema.json");

#[derive(Clone, Copy)]
enum InverseBindCase {
    Available,
    Absent,
    Singular,
}

fn append_f32s(bytes: &mut Vec<u8>, values: impl IntoIterator<Item = f32>) -> (usize, usize) {
    while !bytes.len().is_multiple_of(4) {
        bytes.push(0);
    }
    let offset = bytes.len();
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    (offset, bytes.len() - offset)
}

fn matrix_translation(x: f32, y: f32, z: f32) -> [f32; 16] {
    [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, x, y, z, 1.0,
    ]
}

fn write_source_skin_gltf(path: &std::path::Path, case: InverseBindCase, multiple_skins: bool) {
    let mut bytes = Vec::new();
    let (positions_offset, positions_length) = append_f32s(
        &mut bytes,
        [0.0_f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
    );
    let first_ibm = match case {
        InverseBindCase::Singular => [0.0; 16],
        _ => matrix_translation(0.0, -3.0, 0.0),
    };
    let (first_ibm_offset, first_ibm_length) = append_f32s(&mut bytes, first_ibm);
    let (second_ibm_offset, second_ibm_length) =
        append_f32s(&mut bytes, matrix_translation(-1.0, -3.0, 0.0));
    let buffer = path.with_extension("bin");
    std::fs::write(&buffer, &bytes).expect("writes synthetic buffer");

    let mut accessors = vec![json!({
        "bufferView": 0,
        "componentType": 5126,
        "count": 3,
        "type": "VEC3",
        "min": [0.0, 0.0, 0.0],
        "max": [1.0, 1.0, 0.0]
    })];
    let mut buffer_views = vec![json!({
        "buffer": 0,
        "byteOffset": positions_offset,
        "byteLength": positions_length
    })];
    let first_ibm_accessor = if matches!(case, InverseBindCase::Absent) {
        None
    } else {
        buffer_views.push(json!({
            "buffer": 0,
            "byteOffset": first_ibm_offset,
            "byteLength": first_ibm_length
        }));
        accessors.push(json!({
            "bufferView": buffer_views.len() - 1,
            "componentType": 5126,
            "count": 1,
            "type": "MAT4"
        }));
        Some(accessors.len() - 1)
    };
    let second_ibm_accessor = if multiple_skins {
        buffer_views.push(json!({
            "buffer": 0,
            "byteOffset": second_ibm_offset,
            "byteLength": second_ibm_length
        }));
        accessors.push(json!({
            "bufferView": buffer_views.len() - 1,
            "componentType": 5126,
            "count": 1,
            "type": "MAT4"
        }));
        Some(accessors.len() - 1)
    } else {
        None
    };

    let mut skins = vec![json!({ "name": "skin_a", "joints": [2] })];
    if let Some(accessor) = first_ibm_accessor {
        skins[0]["inverseBindMatrices"] = json!(accessor);
    }
    if multiple_skins {
        skins.push(json!({
            "name": "skin_b",
            "joints": [2],
            "inverseBindMatrices": second_ibm_accessor.expect("second accessor")
        }));
    }
    let mut nodes = vec![
        json!({ "name": "scene_root", "translation": [10.0, 0.0, 0.0], "children": [1, 3] }),
        json!({ "name": "non_joint_parent", "translation": [0.0, 2.0, 0.0], "children": [2] }),
        json!({ "name": "joint", "translation": [0.0, 1.0, 0.0] }),
        json!({ "name": "mesh_a", "mesh": 0, "skin": 0 }),
    ];
    if multiple_skins {
        nodes[0]["children"] = json!([1, 3, 4]);
        nodes.push(json!({ "name": "mesh_b", "mesh": 0, "skin": 1 }));
    }
    let document = json!({
        "asset": { "version": "2.0" },
        "buffers": [{ "uri": buffer.file_name().and_then(|name| name.to_str()).expect("UTF-8 name"), "byteLength": bytes.len() }],
        "bufferViews": buffer_views,
        "accessors": accessors,
        "meshes": [{ "name": "body", "primitives": [{ "attributes": { "POSITION": 0 } }] }],
        "nodes": nodes,
        "skins": skins,
        "scenes": [{ "nodes": [0] }],
        "scene": 0
    });
    std::fs::write(
        path,
        serde_json::to_vec_pretty(&document).expect("serializes glTF"),
    )
    .expect("writes synthetic glTF");
}

fn measure(path: &std::path::Path) -> (Value, Vec<u8>) {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .arg("measure")
        .arg(path)
        .args(["--format", "json"])
        .output()
        .expect("runs animsmith");
    assert!(
        output.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json = serde_json::from_slice(&output.stdout).expect("valid output JSON");
    (json, output.stdout)
}

fn assert_measurements_schema(value: &Value) {
    let schema: Value = serde_json::from_str(MEASUREMENTS_SCHEMA).expect("valid v5 schema JSON");
    let validator = jsonschema::validator_for(&schema).expect("v5 schema compiles");
    let errors = validator
        .iter_errors(value)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(
        errors.is_empty(),
        "v5 schema errors: {errors:#?}\n{value:#}"
    );
}

#[test]
fn cli_measure_preserves_source_skin_identity_and_coordinate_domains() {
    let dir = tempfile::tempdir().expect("temp dir");
    let input = dir.path().join("multiple-skins.gltf");
    write_source_skin_gltf(&input, InverseBindCase::Available, true);

    let (first, first_bytes) = measure(&input);
    let (second, second_bytes) = measure(&input);
    let measurements = &first["files"][0]["measurements"];
    assert_measurements_schema(measurements);
    assert_eq!(
        first_bytes, second_bytes,
        "deterministic JSON field and array order"
    );
    assert_eq!(
        measurements, &second["files"][0]["measurements"],
        "deterministic measurements"
    );
    assert_eq!(measurements["schema_version"], 5);
    assert_eq!(
        measurements["schema"],
        "urn:animsmith:schema:measurements:5"
    );
    assert_eq!(measurements["skeleton_source_coverage"], "complete");
    assert_eq!(
        measurements["skeleton_nodes"]
            .as_array()
            .expect("nodes")
            .iter()
            .map(|node| node["node_index"].as_u64().expect("node index"))
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3, 4]
    );
    assert_eq!(measurements["skeleton_nodes"][2]["parent_node_index"], 1);
    assert_eq!(
        measurements["skeleton_nodes"][0]["scene_root_indices"],
        json!([0])
    );
    assert_eq!(
        measurements["skeleton_nodes"][2]["scene_root_indices"],
        json!([])
    );
    assert_eq!(
        measurements["skeleton_nodes"][0]["local_rest"],
        json!({
            "kind": "trs", "translation_m": [10.0, 0.0, 0.0],
            "rotation_xyzw": [0.0, 0.0, 0.0, 1.0], "scale": [1.0, 1.0, 1.0]
        })
    );
    assert_eq!(
        measurements["skeleton_nodes"][2]["rest_world_matrix"][12],
        10.0
    );
    assert_eq!(
        measurements["skeleton_nodes"][2]["rest_world_matrix"][13],
        3.0
    );

    let skins = measurements["skins"].as_array().expect("skins");
    assert_eq!(skins.len(), 2, "shared joint keeps two source skins");
    assert_eq!(skins[0]["skin_index"], 0);
    assert_eq!(skins[1]["skin_index"], 1);
    assert_eq!(
        skins[0]["joints"],
        json!([{ "joint_index": 0, "node_index": 2 }])
    );
    assert_eq!(
        skins[1]["joints"],
        json!([{ "joint_index": 0, "node_index": 2 }])
    );
    assert_eq!(
        skins[0]["attachments"],
        json!([{ "node_index": 3, "mesh_index": 0 }])
    );
    assert_eq!(
        skins[1]["attachments"],
        json!([{ "node_index": 4, "mesh_index": 0 }])
    );
    assert_eq!(skins[0]["inverse_bind_accessor"]["status"], "available");
    assert_eq!(skins[1]["inverse_bind_accessor"]["status"], "available");
    assert_eq!(
        skins[0]["joint_bind_to_mesh_matrices"][0]["matrix"][13],
        3.0
    );
    assert_eq!(
        skins[1]["joint_bind_to_mesh_matrices"][0]["matrix"][12],
        1.0
    );
    assert_eq!(skins[0]["mesh_bind_world_matrices"][0]["matrix"][12], 10.0);
    assert_eq!(skins[0]["mesh_bind_world_matrices"][0]["matrix"][13], 0.0);
}

#[test]
fn cli_measure_marks_absent_and_singular_inverse_binds_without_substitution() {
    let dir = tempfile::tempdir().expect("temp dir");
    let absent = dir.path().join("absent.gltf");
    write_source_skin_gltf(&absent, InverseBindCase::Absent, false);
    let (absent, _) = measure(&absent);
    let absent = &absent["files"][0]["measurements"];
    assert_measurements_schema(absent);
    let absent_skin = &absent["skins"][0];
    assert_eq!(
        absent_skin["inverse_bind_accessor"],
        json!({ "status": "absent", "matrices": [] })
    );
    for field in ["joint_bind_to_mesh_matrices", "mesh_bind_world_matrices"] {
        assert_eq!(
            absent_skin[field][0]["unavailable_reason"], "inverse_bind_accessor_absent",
            "{field} must not fall back to node-rest data"
        );
        assert!(absent_skin[field][0].get("matrix").is_none());
    }

    let singular = dir.path().join("singular.gltf");
    write_source_skin_gltf(&singular, InverseBindCase::Singular, false);
    let (singular, _) = measure(&singular);
    let singular = &singular["files"][0]["measurements"];
    assert_measurements_schema(singular);
    let singular_skin = &singular["skins"][0];
    assert_eq!(
        singular_skin["inverse_bind_accessor"]["status"],
        "available"
    );
    assert_eq!(
        singular_skin["joint_bind_to_mesh_matrices"][0]["unavailable_reason"],
        "inverse_bind_matrix_non_invertible"
    );
    assert!(
        singular_skin["joint_bind_to_mesh_matrices"][0]
            .get("matrix")
            .is_none()
    );
    assert_eq!(
        singular_skin["mesh_bind_world_matrices"][0]["matrix"],
        json!(vec![0.0; 16])
    );
}
