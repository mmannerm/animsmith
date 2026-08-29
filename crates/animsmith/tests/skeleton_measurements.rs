//! Black-box source-skeleton measurement coverage using deliberately raw,
//! synthetic glTF fixtures. The normal writer creates generated skins, so it
//! cannot prove source skin identity, shared joints, or inverse-bind absence.

use serde_json::{Value, json};

const MEASUREMENTS_SCHEMA: &str =
    include_str!("../../../docs/schemas/measurements-v18.schema.json");
const DEEP_HIERARCHY_DEPTH: usize = 4_096;

#[derive(Clone, Copy)]
enum InverseBindCase {
    Available,
    Absent,
    Singular,
    NonAffine,
    IllConditioned,
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

fn matrix_uniform_scale(scale: f32) -> [f32; 16] {
    [
        scale, 0.0, 0.0, 0.0, 0.0, scale, 0.0, 0.0, 0.0, 0.0, scale, 0.0, 0.0, 0.0, 0.0, 1.0,
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
        InverseBindCase::Singular => matrix_uniform_scale(0.0),
        InverseBindCase::NonAffine => {
            let mut matrix = matrix_translation(-3.0, -4.0, -5.0);
            matrix[3] = 0.1;
            matrix
        }
        InverseBindCase::IllConditioned => [
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0e-7, 0.0, 0.0, 0.0, 0.0, 1.0,
        ],
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
    skins[0]["skeleton"] = json!(1);
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

fn write_uniform_bind_summary_gltf(path: &std::path::Path, reverse_joint_order: bool) {
    let mut bytes = Vec::new();
    let (positions_offset, positions_length) = append_f32s(
        &mut bytes,
        [0.0_f32, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0],
    );
    let inverse_bind_scales = [1.0_f32, f32::from_bits(0x3f7f_ff80)];
    let order = if reverse_joint_order {
        [1usize, 0usize]
    } else {
        [0, 1]
    };
    let (inverse_binds_offset, inverse_binds_length) = append_f32s(
        &mut bytes,
        order
            .into_iter()
            .flat_map(|index| matrix_uniform_scale(inverse_bind_scales[index])),
    );
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
            { "buffer": 0, "byteOffset": inverse_binds_offset, "byteLength": inverse_binds_length }
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
            { "bufferView": 1, "componentType": 5126, "count": 2, "type": "MAT4" }
        ],
        "meshes": [{ "primitives": [{ "attributes": { "POSITION": 0 } }] }],
        "nodes": [
            { "name": "root", "children": [1, 2, 3] },
            { "name": "joint-a" },
            { "name": "joint-b" },
            { "name": "mesh", "mesh": 0, "skin": 0 }
        ],
        "skins": [{
            "name": "uniform-summary",
            "joints": order.map(|index| index + 1),
            "inverseBindMatrices": 1
        }],
        "scenes": [{ "nodes": [0] }],
        "scene": 0
    });
    std::fs::write(
        path,
        serde_json::to_vec_pretty(&document).expect("serializes summary glTF"),
    )
    .expect("writes summary glTF");
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
    let mut non_finite_bottom_row = matrix_uniform_scale(1.0);
    non_finite_bottom_row[3] = f32::NAN;
    let (non_finite_offset, non_finite_length) = append_f32s(&mut bytes, non_finite_bottom_row);
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
    let schema: Value =
        serde_json::from_str(MEASUREMENTS_SCHEMA).expect("valid measurement schema JSON");
    let validator = jsonschema::validator_for(&schema).expect("measurement schema compiles");
    let errors = validator
        .iter_errors(value)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(
        errors.is_empty(),
        "measurement schema errors: {errors:#?}\n{value:#}"
    );
}

fn assert_measurements_schema_rejected(value: &Value, case: &str) {
    let schema: Value =
        serde_json::from_str(MEASUREMENTS_SCHEMA).expect("valid measurement schema JSON");
    let validator = jsonschema::validator_for(&schema).expect("measurement schema compiles");
    assert!(
        !validator.is_valid(value),
        "measurement schema must reject {case}:\n{value:#}"
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
            "rest_world_translation_m",
            "rest_world_linear",
        ],
    );
    let local_rest = json_container_after_key(node, "local_rest", b'{', b'}');
    assert_ordered_key_markers(
        local_rest,
        &[
            "kind",
            "translation_parent_space_m",
            "rotation_xyzw",
            "scale",
        ],
    );

    let skin = json_object_in_array_after_key(measurements, "skins", 0);
    assert_ordered_key_markers(
        skin,
        &[
            "skin_index",
            "name",
            "skeleton_root_node_index",
            "joints",
            "joint_bind_linear_summary",
            "inverse_bind_accessor",
            "attachments",
        ],
    );
    let inverse_bind_accessor = json_container_after_key(skin, "inverse_bind_accessor", b'{', b'}');
    assert_ordered_key_markers(
        inverse_bind_accessor,
        &["status", "declared_count", "matrices"],
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
    let joint_bind_to_mesh = json_container_after_key(joint, "joint_bind_to_mesh", b'{', b'}');
    assert_ordered_key_markers(
        joint_bind_to_mesh,
        &[
            "source_inverse_bind_matrix",
            "inversion_quality",
            "matrix",
            "linear",
        ],
    );
    let mesh_bind_world = json_container_after_key(joint, "mesh_bind_world", b'{', b'}');
    assert_ordered_key_markers(
        mesh_bind_world,
        &["source_inverse_bind_matrix", "matrix", "linear"],
    );
}

fn assert_inverse_bind_serializer_order(
    stdout: &[u8],
    skin_index: usize,
    accessor_keys: &[&str],
    unavailable_joint_indices: &[usize],
) {
    let measurements = json_container_after_key(stdout, "measurements", b'{', b'}');
    let skin = json_object_in_array_after_key(measurements, "skins", skin_index);
    let accessor = json_container_after_key(skin, "inverse_bind_accessor", b'{', b'}');
    assert_ordered_key_markers(accessor, accessor_keys);

    for &joint_index in unavailable_joint_indices {
        let joint = json_object_in_array_after_key(skin, "joints", joint_index);
        assert_ordered_key_markers(
            joint,
            &[
                "joint_index",
                "node_index",
                "joint_bind_to_mesh",
                "mesh_bind_world",
            ],
        );
        for field in ["joint_bind_to_mesh", "mesh_bind_world"] {
            let derived = json_container_after_key(joint, field, b'{', b'}');
            assert_ordered_key_markers(derived, &["unavailable_reason"]);
            assert!(
                !derived
                    .windows(b"\"matrix\":".len())
                    .any(|window| window == b"\"matrix\":"),
                "unavailable {field} omits matrix"
            );
        }
    }
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
    assert_eq!(measurements["schema_version"], 18);
    assert_eq!(
        measurements["schema"],
        "urn:animsmith:schema:measurements:18"
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
            "kind": "trs", "translation_parent_space_m": [10.0, 0.0, 0.0],
            "rotation_xyzw": [0.0, 0.0, 0.0, 1.0], "scale": [1.0, 1.0, 1.0]
        })
    );
    assert_eq!(
        measurements["skeleton_nodes"][2]["rest_world_matrix"],
        json!(matrix_translation(10.0, 3.0, 0.0))
    );
    assert_eq!(
        measurements["skeleton_nodes"][2]["rest_world_translation_m"],
        json!([10.0, 3.0, 0.0])
    );
    assert_eq!(
        measurements["skeleton_nodes"][2]["rest_world_linear"],
        json!({
            "classification": "unit_orthonormal",
            "axis_lengths": [1.0, 1.0, 1.0],
            "determinant": 1.0,
            "orientation": "positive",
            "rotation_xyzw": [0.0, 0.0, 0.0, 1.0],
            "uniform_scale": 1.0
        })
    );

    let skins = measurements["skins"].as_array().expect("skins");
    assert_eq!(skins.len(), 2, "shared joint keeps two source skins");
    assert_eq!(skins[0]["skin_index"], 0);
    assert_eq!(skins[1]["skin_index"], 1);
    assert_eq!(skins[0]["name"], "skin_a");
    assert_eq!(skins[1]["name"], "skin_b");
    assert_eq!(skins[0]["skeleton_root_node_index"], 1);
    assert!(skins[1].get("skeleton_root_node_index").is_none());
    for skin in skins {
        assert_eq!(skin["joints"][0]["joint_index"], 0);
        assert_eq!(skin["joints"][0]["node_index"], 2);
        assert_eq!(
            skin["joint_bind_linear_summary"],
            json!({
                "classification": "consistent_uniform",
                "joint_count": 1,
                "available_joint_count": 1,
                "unavailable_joint_count": 0,
                "consistent_uniform_scale": 1.0
            })
        );
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
        skins[0]["inverse_bind_accessor"]["matrices"][0],
        json!(matrix_translation(0.0, -3.0, 0.0))
    );
    assert_eq!(
        skins[1]["inverse_bind_accessor"]["matrices"][0],
        json!(matrix_translation(-1.0, -3.0, 0.0))
    );
    for skin in skins {
        let raw = skin["inverse_bind_accessor"]["matrices"][0].clone();
        assert_eq!(
            skin["joints"][0]["joint_bind_to_mesh"]["source_inverse_bind_matrix"],
            raw
        );
        assert_eq!(
            skin["joints"][0]["mesh_bind_world"]["source_inverse_bind_matrix"],
            raw
        );
        assert_eq!(
            skin["joints"][0]["joint_bind_to_mesh"]["inversion_quality"]["reciprocal_condition_number_inf"],
            1.0
        );
    }
    assert_eq!(
        skins[0]["joints"][0]["joint_bind_to_mesh"]["matrix"],
        json!(matrix_translation(0.0, 3.0, 0.0))
    );
    assert_eq!(
        skins[1]["joints"][0]["joint_bind_to_mesh"]["matrix"],
        json!(matrix_translation(1.0, 3.0, 0.0))
    );
    assert_eq!(
        skins[0]["joints"][0]["mesh_bind_world"]["matrix"],
        json!(matrix_translation(10.0, 0.0, 0.0))
    );
    assert_eq!(
        skins[1]["joints"][0]["mesh_bind_world"]["matrix"],
        json!(matrix_translation(9.0, 0.0, 0.0))
    );
}

#[test]
fn cli_measure_publishes_a_canonical_multi_joint_uniform_summary() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut summaries = Vec::new();

    for (name, reverse_joint_order, expected_nodes) in [
        ("forward", false, [1_u64, 2_u64]),
        ("reverse", true, [2_u64, 1_u64]),
    ] {
        let input = dir.path().join(format!("{name}.gltf"));
        write_uniform_bind_summary_gltf(&input, reverse_joint_order);
        let (document, _) = measure(&input);
        let measurements = &document["files"][0]["measurements"];
        assert_measurements_schema(measurements);
        let skin = &measurements["skins"][0];
        let joints = skin["joints"].as_array().expect("joint rows");
        assert_eq!(
            joints
                .iter()
                .map(|joint| joint["node_index"].as_u64().expect("node index"))
                .collect::<Vec<_>>(),
            expected_nodes,
            "the fixture really changes authored joint order"
        );
        let mut factors = joints
            .iter()
            .map(|joint| {
                joint["joint_bind_to_mesh"]["linear"]["uniform_scale"]
                    .as_f64()
                    .expect("uniform factor")
            })
            .collect::<Vec<_>>();
        assert_ne!(factors[0], factors[1], "joint factors must be distinct");
        let raw_matrices = skin["inverse_bind_accessor"]["matrices"]
            .as_array()
            .expect("raw inverse-bind matrices");
        assert_eq!(raw_matrices.len(), joints.len());
        for (slot, joint) in joints.iter().enumerate() {
            for field in ["joint_bind_to_mesh", "mesh_bind_world"] {
                assert_eq!(
                    joint[field]["source_inverse_bind_matrix"], raw_matrices[slot],
                    "joint slot {slot} {field} retains its exact raw accessor matrix"
                );
            }
        }
        factors.sort_by(f64::total_cmp);
        assert_eq!(
            factors
                .iter()
                .map(|factor| factor.to_bits())
                .collect::<Vec<_>>(),
            [0x3ff0_0000_0000_0000, 0x3ff0_0008_0000_0000],
            "the fixture must publish the independently pinned joint factors"
        );
        let expected_mean = f64::from_bits(0x3ff0_0004_0000_0000);
        let summary = &skin["joint_bind_linear_summary"];
        assert_eq!(summary["classification"], "consistent_uniform");
        assert_eq!(summary["joint_count"], 2);
        assert_eq!(summary["available_joint_count"], 2);
        assert_eq!(summary["unavailable_joint_count"], 0);
        assert_eq!(
            summary["consistent_uniform_scale"]
                .as_f64()
                .expect("summary factor")
                .to_bits(),
            expected_mean.to_bits(),
            "the JSON boundary publishes the canonical mean rather than joint 0"
        );
        summaries.push(summary.clone());
    }

    assert_eq!(
        summaries[0], summaries[1],
        "authored joint order cannot change the serialized summary"
    );
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
    let (absent, absent_bytes) = measure(&absent);
    let absent = &absent["files"][0]["measurements"];
    assert_measurements_schema(absent);
    assert_inverse_bind_serializer_order(&absent_bytes, 0, &["status", "matrices"], &[0]);
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
        assert!(absent_skin["joints"][0][field].get("linear").is_none());
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
    let singular_raw = singular_skin["inverse_bind_accessor"]["matrices"][0].clone();
    for field in ["joint_bind_to_mesh", "mesh_bind_world"] {
        assert_eq!(
            singular_skin["joints"][0][field]["source_inverse_bind_matrix"], singular_raw,
            "singular derivation still retains raw evidence in {field}"
        );
    }
    assert!(
        singular_skin["joints"][0]["joint_bind_to_mesh"]
            .get("matrix")
            .is_none()
    );
    assert_eq!(
        singular_skin["joints"][0]["mesh_bind_world"]["matrix"],
        json!([
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 10.0, 3.0, 0.0, 1.0
        ])
    );
    assert_eq!(
        singular_skin["joints"][0]["mesh_bind_world"]["linear"]["classification"],
        "singular"
    );
}

#[test]
fn cli_measure_refuses_non_affine_and_ill_conditioned_inverse_binds_with_raw_evidence() {
    let dir = tempfile::tempdir().expect("temp dir");
    for (case, expected_reason, expected_quality) in [
        (
            InverseBindCase::NonAffine,
            "inverse_bind_matrix_non_affine",
            None,
        ),
        (
            InverseBindCase::IllConditioned,
            "inverse_bind_matrix_ill_conditioned",
            Some(1.0e-7_f32 as f64),
        ),
    ] {
        let input = dir.path().join(format!("{expected_reason}.gltf"));
        write_source_skin_gltf(&input, case, false);
        let (output, _) = measure(&input);
        let skin = &output["files"][0]["measurements"]["skins"][0];
        let source = skin["inverse_bind_accessor"]["matrices"][0].clone();
        let derived = &skin["joints"][0]["joint_bind_to_mesh"];
        assert_eq!(derived["source_inverse_bind_matrix"], source);
        assert_eq!(derived["unavailable_reason"], expected_reason);
        assert!(derived.get("matrix").is_none());
        match expected_quality {
            Some(expected) => assert_eq!(
                derived["inversion_quality"]["reciprocal_condition_number_inf"],
                expected
            ),
            None => assert!(derived.get("inversion_quality").is_none()),
        }
        assert_eq!(
            skin["joints"][0]["mesh_bind_world"]["source_inverse_bind_matrix"],
            source
        );
    }
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

    assert_inverse_bind_serializer_order(
        &first_bytes,
        0,
        &["status", "declared_count", "matrices"],
        &[],
    );
    assert_inverse_bind_serializer_order(
        &first_bytes,
        1,
        &["status", "declared_count", "matrices"],
        &[0],
    );
    assert_inverse_bind_serializer_order(
        &first_bytes,
        2,
        &["status", "declared_count", "matrices"],
        &[1],
    );
    for skin_index in [3, 4] {
        assert_inverse_bind_serializer_order(
            &first_bytes,
            skin_index,
            &["status", "declared_count", "matrices"],
            &[0],
        );
    }

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

#[test]
fn published_schema_rejects_impossible_skeleton_transform_states() {
    let dir = tempfile::tempdir().expect("temp dir");
    let input = dir.path().join("schema-state-base.gltf");
    write_source_skin_gltf(&input, InverseBindCase::Available, false);
    let (document, _) = measure(&input);
    let base = &document["files"][0]["measurements"];
    assert_measurements_schema(base);
    let matrix = base["skins"][0]["inverse_bind_accessor"]["matrices"][0].clone();

    for (case, accessor) in [
        (
            "absent accessor with matrices",
            json!({ "status": "absent", "matrices": [matrix.clone()] }),
        ),
        (
            "empty accessor with nonzero declared count",
            json!({ "status": "empty_accessor", "declared_count": 1, "matrices": [] }),
        ),
        (
            "empty accessor with matrices",
            json!({ "status": "empty_accessor", "declared_count": 0, "matrices": [matrix.clone()] }),
        ),
        (
            "unreadable accessor with matrices",
            json!({ "status": "unreadable", "declared_count": 1, "matrices": [matrix.clone()] }),
        ),
    ] {
        let mut invalid = base.clone();
        invalid["skins"][0]["inverse_bind_accessor"] = accessor;
        assert_measurements_schema_rejected(&invalid, case);
    }

    let mut missing_world_translation = base.clone();
    missing_world_translation["skeleton_nodes"][0]
        .as_object_mut()
        .expect("node object")
        .remove("rest_world_translation_m");
    assert_measurements_schema_rejected(
        &missing_world_translation,
        "an available rest-world matrix requires its direct translation",
    );

    let mut non_finite_with_numbers = base.clone();
    non_finite_with_numbers["skeleton_nodes"][0]["rest_world_linear"]["classification"] =
        json!("non_finite");
    assert_measurements_schema_rejected(
        &non_finite_with_numbers,
        "non-finite linear facts cannot carry finite numeric observations",
    );

    let mut unit_without_rotation = base.clone();
    unit_without_rotation["skeleton_nodes"][0]["rest_world_linear"]
        .as_object_mut()
        .expect("linear facts object")
        .remove("rotation_xyzw");
    assert_measurements_schema_rejected(
        &unit_without_rotation,
        "unit-orthonormal linear facts require their canonical rotation",
    );

    let mut non_unit_with_rotation = base.clone();
    non_unit_with_rotation["skeleton_nodes"][0]["rest_world_linear"]["classification"] =
        json!("uniform_scaled");
    assert_measurements_schema_rejected(
        &non_unit_with_rotation,
        "only unit-orthonormal linear facts may carry a canonical rotation",
    );

    let mut available_world_with_non_finite_linear = base.clone();
    available_world_with_non_finite_linear["skeleton_nodes"][0]["rest_world_linear"] =
        json!({ "classification": "non_finite" });
    assert_measurements_schema_rejected(
        &available_world_with_non_finite_linear,
        "an available rest-world matrix requires finite linear facts",
    );

    let mut missing_bind_linear = base.clone();
    missing_bind_linear["skins"][0]["joints"][0]["joint_bind_to_mesh"]
        .as_object_mut()
        .expect("derived matrix object")
        .remove("linear");
    assert_measurements_schema_rejected(
        &missing_bind_linear,
        "an available derived matrix requires linear facts",
    );

    let mut available_bind_with_non_finite_linear = base.clone();
    available_bind_with_non_finite_linear["skins"][0]["joints"][0]["joint_bind_to_mesh"]["linear"] =
        json!({ "classification": "non_finite" });
    assert_measurements_schema_rejected(
        &available_bind_with_non_finite_linear,
        "an available derived matrix requires finite linear facts",
    );

    let mut missing_consistent_factor = base.clone();
    missing_consistent_factor["skins"][0]["joint_bind_linear_summary"]
        .as_object_mut()
        .expect("skin summary object")
        .remove("consistent_uniform_scale");
    assert_measurements_schema_rejected(
        &missing_consistent_factor,
        "a consistent uniform skin summary requires its factor",
    );
}

fn assert_derived_unavailable(skin: &Value, reason: &str) {
    for field in ["joint_bind_to_mesh", "mesh_bind_world"] {
        assert_eq!(
            skin["joints"][0][field]["unavailable_reason"], reason,
            "{field}"
        );
        assert!(skin["joints"][0][field].get("matrix").is_none(), "{field}");
        assert!(skin["joints"][0][field].get("linear").is_none(), "{field}");
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
