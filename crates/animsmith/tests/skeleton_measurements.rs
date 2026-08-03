//! Black-box source-skeleton measurement coverage using deliberately raw,
//! synthetic glTF fixtures. The normal writer creates generated skins, so it
//! cannot prove source skin identity, shared joints, or inverse-bind absence.

use serde_json::{Value, json};

const MEASUREMENTS_SCHEMA: &str = include_str!("../../../docs/schemas/measurements-v5.schema.json");
const DEEP_HIERARCHY_DEPTH: usize = 4_096;

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

fn write_matrix_node_gltf(path: &std::path::Path) {
    let matrix = [
        2.0, 0.0, 0.0, 0.0, 0.0, 3.0, 0.0, 0.0, 0.0, 0.0, 4.0, 0.0, 10.0, 20.0, 30.0, 1.0,
    ];
    let document = json!({
        "asset": { "version": "2.0" },
        "nodes": [{ "name": "matrix-node", "matrix": matrix }],
        "scenes": [{ "nodes": [0] }],
        "scene": 0
    });
    std::fs::write(
        path,
        serde_json::to_vec_pretty(&document).expect("serializes matrix-node glTF"),
    )
    .expect("writes matrix-node glTF");
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
        json!({ "name": "shared_display_name", "translation": [10.0, 0.0, 0.0], "children": [1, 3] }),
        json!({ "name": "shared_display_name", "translation": [0.0, 2.0, 0.0], "children": [2] }),
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

fn write_inverse_bind_state_gltf(path: &std::path::Path) {
    let mut bytes = Vec::new();
    let (positions_offset, positions_length) = append_f32s(
        &mut bytes,
        [0.0_f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
    );
    let (extra_offset, extra_length) = append_f32s(
        &mut bytes,
        matrix_translation(4.0, 0.0, 0.0)
            .into_iter()
            .chain(matrix_translation(5.0, 0.0, 0.0))
            .chain(matrix_translation(6.0, 0.0, 0.0)),
    );
    let (short_offset, short_length) = append_f32s(&mut bytes, matrix_translation(0.0, -3.0, 0.0));
    let (non_finite_offset, non_finite_length) = append_f32s(&mut bytes, [f32::NAN; 16]);
    let wrong_type_indices_offset = bytes.len();
    bytes.push(0);
    let (wrong_type_values_offset, wrong_type_values_length) =
        append_f32s(&mut bytes, [0.0_f32, -3.0, 0.0, 1.0]);
    let buffer = path.with_extension("bin");
    std::fs::write(&buffer, &bytes).expect("writes synthetic buffer");

    let document = json!({
        "asset": { "version": "2.0" },
        "buffers": [{
            "uri": buffer.file_name().and_then(|name| name.to_str()).expect("UTF-8 name"),
            "byteLength": bytes.len()
        }],
        "bufferViews": [
            { "buffer": 0, "byteOffset": positions_offset, "byteLength": positions_length },
            { "buffer": 0, "byteOffset": extra_offset, "byteLength": extra_length },
            { "buffer": 0, "byteOffset": short_offset, "byteLength": short_length },
            { "buffer": 0, "byteOffset": non_finite_offset, "byteLength": non_finite_length },
            { "buffer": 0, "byteOffset": wrong_type_indices_offset, "byteLength": 1 },
            { "buffer": 0, "byteOffset": wrong_type_values_offset, "byteLength": wrong_type_values_length }
        ],
        "accessors": [
            {
                "bufferView": 0,
                "componentType": 5126,
                "count": 3,
                "type": "VEC3",
                "min": [0.0, 0.0, 0.0],
                "max": [1.0, 1.0, 0.0]
            },
            { "bufferView": 1, "componentType": 5126, "count": 3, "type": "MAT4" },
            { "bufferView": 0, "componentType": 5126, "count": 0, "type": "MAT4" },
            { "bufferView": 2, "componentType": 5126, "count": 1, "type": "MAT4" },
            { "bufferView": 3, "componentType": 5126, "count": 1, "type": "MAT4" },
            {
                "componentType": 5126,
                "count": 1,
                "type": "VEC4",
                "sparse": {
                    "count": 1,
                    "indices": { "bufferView": 4, "componentType": 5121 },
                    "values": { "bufferView": 5 }
                }
            }
        ],
        "meshes": [{ "primitives": [{ "attributes": { "POSITION": 0 } }] }],
        "nodes": [
            { "name": "scene-root", "children": [1, 2, 3, 4, 5, 6, 7] },
            { "name": "joint-a" },
            { "name": "joint-b" },
            { "name": "extra", "mesh": 0, "skin": 0 },
            { "name": "empty", "mesh": 0, "skin": 1 },
            { "name": "short", "mesh": 0, "skin": 2 },
            { "name": "non-finite", "mesh": 0, "skin": 3 },
            { "name": "wrong-type", "mesh": 0, "skin": 4 }
        ],
        "skins": [
            { "name": "extra", "joints": [1, 2], "inverseBindMatrices": 1 },
            { "name": "empty", "joints": [1], "inverseBindMatrices": 2 },
            { "name": "short", "joints": [1, 2], "inverseBindMatrices": 3 },
            { "name": "non-finite", "joints": [1], "inverseBindMatrices": 4 },
            { "name": "wrong-type", "joints": [1], "inverseBindMatrices": 5 }
        ],
        "scenes": [{ "nodes": [0] }],
        "scene": 0
    });
    std::fs::write(
        path,
        serde_json::to_vec_pretty(&document).expect("serializes glTF"),
    )
    .expect("writes synthetic glTF");
}

fn write_deep_leaf_first_hierarchy_gltf(path: &std::path::Path) {
    let nodes = (0..DEEP_HIERARCHY_DEPTH)
        .map(|node_index| {
            let mut node = json!({ "translation": [1.0, 0.0, 0.0] });
            if node_index != 0 {
                node["name"] = json!(if node_index + 1 == DEEP_HIERARCHY_DEPTH {
                    "root"
                } else {
                    "link"
                });
            }
            if node_index > 0 {
                node["children"] = json!([node_index - 1]);
            }
            node
        })
        .collect::<Vec<_>>();
    let document = json!({
        "asset": { "version": "2.0" },
        "nodes": nodes,
        "skins": [{ "name": "deep-skin", "joints": [0] }],
        "scenes": [{ "nodes": [DEEP_HIERARCHY_DEPTH - 1] }],
        "scene": 0
    });
    std::fs::write(
        path,
        serde_json::to_vec(&document).expect("serializes deep synthetic glTF"),
    )
    .expect("writes deep synthetic glTF");
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

fn json_container_end(bytes: &[u8], start: usize, opening: u8, closing: u8) -> usize {
    assert_eq!(
        bytes[start], opening,
        "container starts at the expected delimiter"
    );
    let mut depth = 0;
    let mut in_string = false;
    let mut escaped = false;
    for (index, byte) in bytes.iter().copied().enumerate().skip(start) {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
        } else if byte == b'"' {
            in_string = true;
        } else if byte == opening {
            depth += 1;
        } else if byte == closing {
            depth -= 1;
            if depth == 0 {
                return index;
            }
        }
    }
    panic!("unterminated JSON container");
}

fn json_container_after_key<'a>(bytes: &'a [u8], key: &str, opening: u8, closing: u8) -> &'a [u8] {
    let marker = format!("\"{key}\":").into_bytes();
    let key_start = bytes
        .windows(marker.len())
        .position(|window| window == marker)
        .unwrap_or_else(|| panic!("JSON key {key:?} is present"));
    let start = bytes[key_start + marker.len()..]
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .map(|offset| key_start + marker.len() + offset)
        .expect("JSON value follows key");
    let end = json_container_end(bytes, start, opening, closing);
    &bytes[start..=end]
}

fn json_object_in_array_after_key<'a>(bytes: &'a [u8], key: &str, index: usize) -> &'a [u8] {
    let array = json_container_after_key(bytes, key, b'[', b']');
    let mut search_from = 0;
    for current_index in 0..=index {
        let start = array[search_from..]
            .iter()
            .position(|byte| *byte == b'{')
            .map(|offset| search_from + offset)
            .unwrap_or_else(|| panic!("array contains object {current_index}"));
        let end = json_container_end(array, start, b'{', b'}');
        if current_index == index {
            return &array[start..=end];
        }
        search_from = end + 1;
    }
    unreachable!("the loop returns when it reaches the requested index")
}

fn assert_ordered_key_markers(object: &[u8], keys: &[&str]) {
    let mut search_from = 0;
    for key in keys {
        let marker = format!("\"{key}\":").into_bytes();
        let offset = object[search_from..]
            .windows(marker.len())
            .position(|window| window == marker)
            .unwrap_or_else(|| panic!("JSON object contains key {key:?}"));
        search_from += offset + marker.len();
    }
}

fn assert_skeleton_serializer_order(stdout: &[u8]) {
    let measurements = json_container_after_key(stdout, "measurements", b'{', b'}');
    assert_ordered_key_markers(
        measurements,
        &["skeleton_source_coverage", "skeleton_nodes", "skins"],
    );

    let node = json_object_in_array_after_key(measurements, "skeleton_nodes", 1);
    assert_ordered_key_markers(
        node,
        &[
            "node_index",
            "name",
            "parent_node_index",
            "scene_root_indices",
            "local_rest",
            "rest_world_matrix",
        ],
    );

    let skin = json_object_in_array_after_key(measurements, "skins", 0);
    assert_ordered_key_markers(
        skin,
        &[
            "skin_index",
            "name",
            "joints",
            "inverse_bind_accessor",
            "attachments",
        ],
    );

    let joint = json_object_in_array_after_key(skin, "joints", 0);
    assert_ordered_key_markers(
        joint,
        &[
            "joint_index",
            "node_index",
            "joint_bind_to_mesh",
            "mesh_bind_world",
        ],
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
    assert_skeleton_serializer_order(&first_bytes);
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
    assert_eq!(
        measurements["skeleton_nodes"]
            .as_array()
            .expect("nodes")
            .iter()
            .map(|node| node["name"].as_str().expect("node name"))
            .collect::<Vec<_>>(),
        vec![
            "shared_display_name",
            "shared_display_name",
            "joint",
            "mesh_a",
            "mesh_b"
        ],
        "authored names are display data and do not merge source identities"
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
    assert_eq!(skins[0]["name"], "skin_a");
    assert_eq!(skins[1]["name"], "skin_b");
    for skin in skins {
        assert_eq!(skin["joints"][0]["joint_index"], 0);
        assert_eq!(skin["joints"][0]["node_index"], 2);
    }
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
        skins[0]["joints"][0]["joint_bind_to_mesh"]["matrix"][13],
        3.0
    );
    assert_eq!(
        skins[1]["joints"][0]["joint_bind_to_mesh"]["matrix"][12],
        1.0
    );
    assert_eq!(skins[0]["joints"][0]["mesh_bind_world"]["matrix"][12], 10.0);
    assert_eq!(skins[0]["joints"][0]["mesh_bind_world"]["matrix"][13], 0.0);
}

#[test]
fn cli_measure_preserves_authored_matrix_local_rest() {
    let dir = tempfile::tempdir().expect("temp dir");
    let input = dir.path().join("matrix-node.gltf");
    write_matrix_node_gltf(&input);

    let (document, _) = measure(&input);
    let measurements = &document["files"][0]["measurements"];
    assert_measurements_schema(measurements);
    let expected = json!([
        2.0, 0.0, 0.0, 0.0, 0.0, 3.0, 0.0, 0.0, 0.0, 0.0, 4.0, 0.0, 10.0, 20.0, 30.0, 1.0
    ]);
    assert_eq!(
        measurements["skeleton_nodes"][0]["local_rest"],
        json!({ "kind": "matrix", "matrix": expected }),
        "an authored matrix remains a matrix instead of being decomposed to TRS"
    );
    assert_eq!(
        measurements["skeleton_nodes"][0]["rest_world_matrix"], expected,
        "the scene-root world domain retains every column-major component"
    );
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
    for field in ["joint_bind_to_mesh", "mesh_bind_world"] {
        assert_eq!(
            absent_skin["joints"][0][field]["unavailable_reason"], "inverse_bind_accessor_absent",
            "{field} must not fall back to node-rest data"
        );
        assert!(absent_skin["joints"][0][field].get("matrix").is_none());
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
        singular_skin["joints"][0]["joint_bind_to_mesh"]["unavailable_reason"],
        "inverse_bind_matrix_non_invertible"
    );
    assert!(
        singular_skin["joints"][0]["joint_bind_to_mesh"]
            .get("matrix")
            .is_none()
    );
    assert_eq!(
        singular_skin["joints"][0]["mesh_bind_world"]["matrix"],
        json!(vec![0.0; 16])
    );
}

#[test]
fn cli_measure_preserves_each_inverse_bind_accessor_state() {
    let dir = tempfile::tempdir().expect("temp dir");
    let input = dir.path().join("inverse-bind-states.gltf");
    write_inverse_bind_state_gltf(&input);

    let (first, first_bytes) = measure(&input);
    let (second, second_bytes) = measure(&input);
    let measurements = &first["files"][0]["measurements"];
    assert_measurements_schema(measurements);
    assert_eq!(
        first_bytes, second_bytes,
        "every inverse-bind state has deterministic JSON output"
    );
    assert_eq!(measurements, &second["files"][0]["measurements"]);

    let skins = measurements["skins"].as_array().expect("skins");
    assert_eq!(skins.len(), 5);

    let extra = &skins[0];
    assert_eq!(extra["inverse_bind_accessor"]["status"], "available");
    assert_eq!(extra["inverse_bind_accessor"]["declared_count"], 3);
    assert_eq!(
        extra["inverse_bind_accessor"]["matrices"]
            .as_array()
            .map(Vec::len),
        Some(3)
    );
    assert_eq!(extra["inverse_bind_accessor"]["matrices"][0][12], 4.0);
    assert_eq!(extra["inverse_bind_accessor"]["matrices"][1][12], 5.0);
    assert_eq!(extra["inverse_bind_accessor"]["matrices"][2][12], 6.0);
    assert_eq!(extra["joints"][0]["joint_index"], 0);
    assert_eq!(extra["joints"][0]["node_index"], 1);
    assert_eq!(extra["joints"][0]["joint_bind_to_mesh"]["matrix"][12], -4.0);
    assert_eq!(extra["joints"][0]["mesh_bind_world"]["matrix"][12], 4.0);
    assert_eq!(extra["joints"][1]["joint_index"], 1);
    assert_eq!(extra["joints"][1]["node_index"], 2);
    assert_eq!(extra["joints"][1]["joint_bind_to_mesh"]["matrix"][12], -5.0);
    assert_eq!(extra["joints"][1]["mesh_bind_world"]["matrix"][12], 5.0);

    let empty = &skins[1];
    assert_eq!(
        empty["inverse_bind_accessor"],
        json!({
            "status": "empty_accessor", "declared_count": 0, "matrices": []
        })
    );
    assert_derived_unavailable(empty, "inverse_bind_accessor_empty");

    let short = &skins[2];
    assert_eq!(short["inverse_bind_accessor"]["status"], "count_mismatch");
    assert_eq!(short["inverse_bind_accessor"]["declared_count"], 1);
    assert_eq!(
        short["inverse_bind_accessor"]["matrices"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(short["inverse_bind_accessor"]["matrices"][0][13], -3.0);
    for field in ["joint_bind_to_mesh", "mesh_bind_world"] {
        assert!(
            short["joints"][0][field].get("matrix").is_some(),
            "readable prefix in {field}"
        );
        assert_eq!(
            short["joints"][1][field]["unavailable_reason"], "inverse_bind_accessor_count_mismatch",
            "missing joint row in {field}"
        );
        assert!(short["joints"][1][field].get("matrix").is_none());
    }

    let non_finite = &skins[3];
    assert_eq!(
        non_finite["inverse_bind_accessor"],
        json!({
            "status": "unreadable", "declared_count": 1, "matrices": []
        })
    );
    assert_derived_unavailable(non_finite, "inverse_bind_accessor_unreadable");

    let wrong_type = &skins[4];
    assert_eq!(
        wrong_type["inverse_bind_accessor"],
        json!({
            "status": "unreadable", "declared_count": 1, "matrices": []
        }),
        "a finite VEC4 inverse-bind accessor is parser-valid but unreadable as matrices"
    );
    assert_derived_unavailable(wrong_type, "inverse_bind_accessor_unreadable");
}

fn assert_derived_unavailable(skin: &Value, reason: &str) {
    for field in ["joint_bind_to_mesh", "mesh_bind_world"] {
        assert_eq!(
            skin["joints"][0][field]["unavailable_reason"], reason,
            "{field}"
        );
        assert!(skin["joints"][0][field].get("matrix").is_none(), "{field}");
    }
}

#[test]
fn cli_measure_handles_a_deep_leaf_first_source_hierarchy() {
    let dir = tempfile::tempdir().expect("temp dir");
    let input = dir.path().join("deep-leaf-first-hierarchy.gltf");
    write_deep_leaf_first_hierarchy_gltf(&input);

    let (document, _) = measure(&input);
    let measurements = &document["files"][0]["measurements"];
    assert_measurements_schema(measurements);
    let nodes = measurements["skeleton_nodes"]
        .as_array()
        .expect("source nodes");
    assert_eq!(nodes.len(), DEEP_HIERARCHY_DEPTH);

    let leaf = &nodes[0];
    assert_eq!(leaf["node_index"], 0, "leaf keeps source node identity");
    assert!(
        leaf.get("name").is_none(),
        "unnamed leaf omits display data"
    );
    assert_eq!(leaf["parent_node_index"], 1);

    let root = nodes.last().expect("root node");
    assert_eq!(
        root["node_index"],
        DEEP_HIERARCHY_DEPTH - 1,
        "source nodes remain in source-array order"
    );
    assert_eq!(root["name"], "root");
    assert!(root.get("parent_node_index").is_none());
    assert_eq!(root["scene_root_indices"], json!([0]));

    let leaf_world = leaf["rest_world_matrix"].as_array().expect("leaf world");
    assert!(
        leaf_world
            .iter()
            .all(|entry| entry.as_f64().is_some_and(f64::is_finite)),
        "leaf rest-world matrix is finite"
    );
    assert_eq!(
        leaf_world[12].as_f64(),
        Some(DEEP_HIERARCHY_DEPTH as f64),
        "leaf rest-world translation accumulates every ancestor"
    );

    let skins = measurements["skins"].as_array().expect("source skins");
    assert_eq!(skins.len(), 1, "unattached source skin is retained");
    assert_eq!(skins[0]["skin_index"], 0);
    assert_eq!(skins[0]["name"], "deep-skin");
    assert_eq!(skins[0]["joints"][0]["joint_index"], 0);
    assert_eq!(skins[0]["joints"][0]["node_index"], 0);
    assert_eq!(skins[0]["attachments"], json!([]));
}
