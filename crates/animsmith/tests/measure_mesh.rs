//! CI-run coverage that `measure` emits per-mesh geometry measurements
//! end-to-end through the real CLI on synthetic glTF: vertex count, AABB,
//! centroid, skin influences, and weight-sum range.

use animsmith_core::glam::{Mat4, Vec3};
use animsmith_core::model::*;

const MEASUREMENTS_SCHEMA: &str =
    include_str!("../../../docs/schemas/measurements-v15.schema.json");

fn assert_measurements_schema_valid(measurements: &serde_json::Value) {
    let schema = serde_json::from_str(MEASUREMENTS_SCHEMA).expect("valid measurement schema JSON");
    let validator = jsonschema::validator_for(&schema).expect("measurement schema compiles");
    let errors = validator
        .iter_errors(measurements)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(
        errors.is_empty(),
        "measurement output must satisfy the published v15 schema:\n{}\ninstance: {measurements:#}",
        errors.join("\n")
    );
}

fn assert_measurements_schema_invalid(measurements: &serde_json::Value) {
    let schema = serde_json::from_str(MEASUREMENTS_SCHEMA).expect("valid measurement schema JSON");
    let validator = jsonschema::validator_for(&schema).expect("measurement schema compiles");
    assert!(
        !validator.is_valid(measurements),
        "measurement output must violate the published v15 schema:\n{measurements:#}"
    );
}

#[derive(Clone, Copy)]
enum SecondaryInfluenceSet {
    None,
    Paired,
    JointsOnly,
    WeightsOnly,
    HigherIndependentSets,
}

fn append_accessor(
    bytes: &mut Vec<u8>,
    buffer_views: &mut Vec<serde_json::Value>,
    accessors: &mut Vec<serde_json::Value>,
    data: &[u8],
    component_type: u32,
) -> usize {
    while !bytes.len().is_multiple_of(4) {
        bytes.push(0);
    }
    let offset = bytes.len();
    bytes.extend_from_slice(data);
    let view = buffer_views.len();
    buffer_views.push(serde_json::json!({
        "buffer": 0,
        "byteOffset": offset,
        "byteLength": data.len(),
    }));
    let accessor = accessors.len();
    accessors.push(serde_json::json!({
        "bufferView": view,
        "componentType": component_type,
        "count": 3,
        "type": "VEC4",
    }));
    accessor
}

fn vec3_accessor(
    bytes: &mut Vec<u8>,
    buffer_views: &mut Vec<serde_json::Value>,
    accessors: &mut Vec<serde_json::Value>,
) -> usize {
    let mut data = Vec::new();
    for value in [[0.0_f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]
        .into_iter()
        .flatten()
    {
        data.extend_from_slice(&value.to_le_bytes());
    }
    let accessor = append_accessor(bytes, buffer_views, accessors, &data, 5126);
    accessors[accessor]["type"] = serde_json::json!("VEC3");
    accessors[accessor]["min"] = serde_json::json!([0.0, 0.0, 0.0]);
    accessors[accessor]["max"] = serde_json::json!([1.0, 1.0, 0.0]);
    accessor
}

fn joints_accessor(
    bytes: &mut Vec<u8>,
    buffer_views: &mut Vec<serde_json::Value>,
    accessors: &mut Vec<serde_json::Value>,
) -> usize {
    let mut data = Vec::new();
    for value in [[0_u16, 1, 0, 0]; 3].into_iter().flatten() {
        data.extend_from_slice(&value.to_le_bytes());
    }
    append_accessor(bytes, buffer_views, accessors, &data, 5123)
}

fn weights_accessor(
    bytes: &mut Vec<u8>,
    buffer_views: &mut Vec<serde_json::Value>,
    accessors: &mut Vec<serde_json::Value>,
    weights: [f32; 4],
) -> usize {
    let mut data = Vec::new();
    for value in [weights; 3].into_iter().flatten() {
        data.extend_from_slice(&value.to_le_bytes());
    }
    append_accessor(bytes, buffer_views, accessors, &data, 5126)
}

/// Writes a deliberately raw glTF because the core writer carries only
/// `JOINTS_0`/`WEIGHTS_0`; secondary influence attributes must reach the
/// loader directly from source.
fn write_secondary_influence_gltf(path: &std::path::Path, sets: &[SecondaryInfluenceSet]) {
    let mut bytes = Vec::new();
    let mut buffer_views = Vec::new();
    let mut accessors = Vec::new();
    let mut primitives = Vec::new();
    for set in sets {
        let position = vec3_accessor(&mut bytes, &mut buffer_views, &mut accessors);
        let joints = joints_accessor(&mut bytes, &mut buffer_views, &mut accessors);
        let weights = weights_accessor(
            &mut bytes,
            &mut buffer_views,
            &mut accessors,
            [0.5, 0.5, 0.0, 0.0],
        );
        let mut attributes = serde_json::Map::from_iter([
            ("POSITION".into(), serde_json::json!(position)),
            ("JOINTS_0".into(), serde_json::json!(joints)),
            ("WEIGHTS_0".into(), serde_json::json!(weights)),
        ]);
        let joints_set_index = match set {
            SecondaryInfluenceSet::Paired | SecondaryInfluenceSet::JointsOnly => Some(1),
            SecondaryInfluenceSet::HigherIndependentSets => Some(3),
            _ => None,
        };
        if let Some(set_index) = joints_set_index {
            attributes.insert(
                format!("JOINTS_{set_index}"),
                serde_json::json!(joints_accessor(
                    &mut bytes,
                    &mut buffer_views,
                    &mut accessors
                )),
            );
        }
        let weights_set_index = match set {
            SecondaryInfluenceSet::Paired | SecondaryInfluenceSet::WeightsOnly => Some(1),
            SecondaryInfluenceSet::HigherIndependentSets => Some(2),
            _ => None,
        };
        if let Some(set_index) = weights_set_index {
            attributes.insert(
                format!("WEIGHTS_{set_index}"),
                serde_json::json!(weights_accessor(
                    &mut bytes,
                    &mut buffer_views,
                    &mut accessors,
                    [0.8, 0.8, 0.8, 0.8],
                )),
            );
        }
        primitives.push(serde_json::json!({ "attributes": attributes }));
    }
    let buffer = path.with_file_name("secondary-influences.bin");
    std::fs::write(&buffer, &bytes).expect("writes influence buffer");
    let document = serde_json::json!({
        "asset": { "version": "2.0" },
        "buffers": [{ "uri": "secondary-influences.bin", "byteLength": bytes.len() }],
        "bufferViews": buffer_views,
        "accessors": accessors,
        "meshes": [{ "name": "influences", "primitives": primitives }],
    });
    std::fs::write(
        path,
        serde_json::to_vec_pretty(&document).expect("serializes glTF"),
    )
    .expect("writes influence glTF");
}

fn write_nested_instanced_gltf(path: &std::path::Path) {
    let positions = [
        [0.0_f32, 0.0, 0.0],
        [1.0_f32, 0.0, 0.0],
        [0.0_f32, 1.0, 0.0],
    ];
    let morph_deltas = [[50.0_f32, 0.0, 0.0]; 3];
    let animation_times = [0.0_f32, 1.0];
    let animated_translations = [[1000.0_f32, 0.0, 0.0]; 2];
    let indices = [0_u16, 1, 1, 0, 1, 1];
    let mut bytes = Vec::new();
    for value in positions.into_iter().flatten() {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    let morph_offset = bytes.len();
    for value in morph_deltas.into_iter().flatten() {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    let animation_times_offset = bytes.len();
    for value in animation_times {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    let animation_values_offset = bytes.len();
    for value in animated_translations.into_iter().flatten() {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    let indices_offset = bytes.len();
    for value in indices {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    let buffer = path.with_file_name("nested-scene.bin");
    std::fs::write(&buffer, &bytes).expect("writes position buffer");
    let document = serde_json::json!({
        "asset": { "version": "2.0" },
        "buffers": [{ "uri": "nested-scene.bin", "byteLength": bytes.len() }],
        "bufferViews": [
            { "buffer": 0, "byteLength": morph_offset },
            { "buffer": 0, "byteOffset": morph_offset, "byteLength": animation_times_offset - morph_offset },
            { "buffer": 0, "byteOffset": animation_times_offset, "byteLength": animation_values_offset - animation_times_offset },
            { "buffer": 0, "byteOffset": animation_values_offset, "byteLength": indices_offset - animation_values_offset },
            { "buffer": 0, "byteOffset": indices_offset, "byteLength": bytes.len() - indices_offset }
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
            {
                "bufferView": 1,
                "componentType": 5126,
                "count": 3,
                "type": "VEC3",
                "min": [50.0, 0.0, 0.0],
                "max": [50.0, 0.0, 0.0]
            },
            {
                "bufferView": 2,
                "componentType": 5126,
                "count": 2,
                "type": "SCALAR",
                "min": [0.0],
                "max": [1.0]
            },
            {
                "bufferView": 3,
                "componentType": 5126,
                "count": 2,
                "type": "VEC3"
            },
            {
                "bufferView": 4,
                "componentType": 5123,
                "count": 6,
                "type": "SCALAR",
                "min": [0],
                "max": [1]
            }
        ],
        "meshes": [
            {
                "name": "duplicate-definition",
                "weights": [1.0],
                "primitives": [{
                    "attributes": { "POSITION": 0 },
                    "indices": 4,
                    "targets": [{ "POSITION": 1 }]
                }]
            },
            { "name": "duplicate-definition", "primitives": [{ "attributes": { "POSITION": 0 }, "indices": 4 }] }
        ],
        "nodes": [
            { "name": "root", "translation": [10.0, 0.0, 0.0], "children": [1] },
            { "name": "branch", "rotation": [0.0, 0.0, 0.70710677, 0.70710677], "children": [2] },
            { "name": "duplicate", "scale": [-2.0, 3.0, 1.0], "mesh": 0 },
            { "name": "duplicate", "translation": [-4.0, 1.0, 0.0], "mesh": 0 },
            { "name": "orphan", "translation": [100.0, 0.0, 0.0], "mesh": 0 }
        ],
        "scenes": [
            { "name": "wide", "nodes": [0, 3] },
            { "name": "solo", "nodes": [3] }
        ],
        "scene": 1,
        "animations": [{
            "name": "move",
            "samplers": [{ "input": 2, "output": 3, "interpolation": "LINEAR" }],
            "channels": [{ "sampler": 0, "target": { "node": 0, "path": "translation" } }]
        }]
    });
    std::fs::write(
        path,
        serde_json::to_vec_pretty(&document).expect("serializes glTF"),
    )
    .expect("writes glTF");
}

fn assert_aabb_close(value: &serde_json::Value, min: [f64; 3], max: [f64; 3]) {
    for (corner, expected) in [("min", min), ("max", max)] {
        let actual = value[corner].as_array().expect("AABB corner");
        for axis in 0..3 {
            let actual = actual[axis].as_f64().expect("finite AABB component");
            assert!(
                (actual - expected[axis]).abs() < 1e-5,
                "{corner}[{axis}] = {actual}, expected {}",
                expected[axis]
            );
        }
    }
}

fn assert_vec3_close(value: &serde_json::Value, expected: [f64; 3]) {
    let actual = value.as_array().expect("vec3");
    for axis in 0..3 {
        let actual = actual[axis].as_f64().expect("finite vec3 component");
        assert!(
            (actual - expected[axis]).abs() < 1e-5,
            "component {axis} = {actual}, expected {}",
            expected[axis]
        );
    }
}

/// A two-bone skinned triangle with an analytic AABB of (0,0,0)..(2,4,0)
/// and per-vertex influence counts 1/2/2 (all weight-sums 1.0).
fn write_skinned_glb(path: &std::path::Path) {
    let doc = Document {
        skeleton: Skeleton {
            bones: vec![
                Bone {
                    name: "root".into(),
                    parent: None,
                    rest: Transform::IDENTITY,
                    inverse_bind: Some(Mat4::IDENTITY),
                },
                Bone {
                    name: "tip".into(),
                    parent: Some(0),
                    rest: Transform::IDENTITY,
                    inverse_bind: Some(Mat4::IDENTITY),
                },
            ],
        },
        clips: vec![],
        assets: SceneAssets {
            meshes: vec![MeshAsset {
                name: "body".into(),
                source_mesh_index: 0,
                primitives: vec![Primitive {
                    positions: vec![
                        Vec3::new(0.0, 0.0, 0.0),
                        Vec3::new(2.0, 0.0, 0.0),
                        Vec3::new(0.0, 4.0, 0.0),
                    ],
                    joints: vec![[0, 1, 0, 0]; 3],
                    weights: vec![
                        [1.0, 0.0, 0.0, 0.0],
                        [0.5, 0.5, 0.0, 0.0],
                        [0.5, 0.5, 0.0, 0.0],
                    ],
                    ..Primitive::default()
                }],
            }],
            instances: vec![MeshInstance {
                source_node_index: 0,
                node: 0,
                mesh: 0,
                skin_joints: vec![0, 1],
                skin_ibms: vec![],
            }],
            materials: vec![],
            ..SceneAssets::default()
        },
        source: SourceInfo::default(),
    };
    animsmith_gltf::write::write(&doc, path).expect("writes skinned glb");
}

fn assert_body_mesh(mesh: &serde_json::Value) {
    assert_eq!(mesh["mesh_index"], 0);
    assert_eq!(mesh["name"], "body");
    assert_eq!(mesh["vertex_count"], 3);
    assert_eq!(
        mesh["geometry_aabb"]["min"],
        serde_json::json!([0.0, 0.0, 0.0])
    );
    assert_eq!(
        mesh["geometry_aabb"]["max"],
        serde_json::json!([2.0, 4.0, 0.0])
    );
    assert_vec3_close(&mesh["geometry_centroid"], [2.0 / 3.0, 4.0 / 3.0, 0.0]);
    assert_eq!(mesh["max_joints_per_vertex"], 2);
    let lo = mesh["weight_sum_min"].as_f64().expect("weight_sum_min");
    let hi = mesh["weight_sum_max"].as_f64().expect("weight_sum_max");
    assert!(
        (lo - 1.0).abs() < 1e-6 && (hi - 1.0).abs() < 1e-6,
        "weights normalized"
    );
}

#[test]
fn cli_measure_emits_mesh_measurements() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("skinned.glb");
    write_skinned_glb(&input);

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .arg("measure")
        .arg(&input)
        .arg("--format")
        .arg("json")
        .output()
        .expect("runs animsmith");
    assert!(out.status.success(), "measure exited {}", out.status);

    let report: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid JSON");
    let measurements = &report["files"][0]["measurements"];
    assert_measurements_schema_valid(measurements);
    assert_body_mesh(&measurements["mesh_definitions"][0]);
    let mut short_centroid = measurements.clone();
    short_centroid["mesh_definitions"][0]["geometry_centroid"] = serde_json::json!([0.0, 0.0]);
    assert_measurements_schema_invalid(&short_centroid);
    assert_eq!(
        measurements["node_instances"][0]["static_node_world_aabb_unavailable_reason"],
        "skinned_deformation_excluded"
    );
    assert_eq!(measurements["scenes"][0]["instance_count"], 1);
    assert_eq!(measurements["scenes"][0]["excluded_instance_count"], 1);

    let text_output = std::process::Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .arg("measure")
        .arg(&input)
        .args(["--format", "text"])
        .output()
        .expect("runs text measure");
    assert!(text_output.status.success());
    let text = String::from_utf8(text_output.stdout).expect("UTF-8 text output");
    assert!(
        text.contains("geometry centroid (0.667, 1.333, 0.000)"),
        "text output exposes the same centroid shape:\n{text}"
    );
}

#[test]
fn cli_measure_reports_secondary_influence_set_presence_without_changing_primary_metrics() {
    let cases = [
        (
            "primary-only",
            vec![SecondaryInfluenceSet::None],
            serde_json::json!([]),
            vec![(false, false)],
        ),
        (
            "paired",
            vec![SecondaryInfluenceSet::Paired],
            serde_json::json!([{
                "set_index": 1,
                "joints_present": true,
                "weights_present": true,
                "joints_without_weights_present": false,
                "weights_without_joints_present": false,
            }]),
            vec![(true, true)],
        ),
        (
            "joints-only",
            vec![SecondaryInfluenceSet::JointsOnly],
            serde_json::json!([{
                "set_index": 1,
                "joints_present": true,
                "weights_present": false,
                "joints_without_weights_present": true,
                "weights_without_joints_present": false,
            }]),
            vec![(true, false)],
        ),
        (
            "weights-only",
            vec![SecondaryInfluenceSet::WeightsOnly],
            serde_json::json!([{
                "set_index": 1,
                "joints_present": false,
                "weights_present": true,
                "joints_without_weights_present": false,
                "weights_without_joints_present": true,
            }]),
            vec![(false, true)],
        ),
        (
            "second-primitive-only",
            vec![SecondaryInfluenceSet::None, SecondaryInfluenceSet::Paired],
            serde_json::json!([{
                "set_index": 1,
                "joints_present": true,
                "weights_present": true,
                "joints_without_weights_present": false,
                "weights_without_joints_present": false,
            }]),
            vec![(false, false), (true, true)],
        ),
        (
            "cross-primitive-complementary-mismatch",
            vec![
                SecondaryInfluenceSet::JointsOnly,
                SecondaryInfluenceSet::WeightsOnly,
            ],
            serde_json::json!([{
                "set_index": 1,
                "joints_present": true,
                "weights_present": true,
                "joints_without_weights_present": true,
                "weights_without_joints_present": true,
            }]),
            vec![(true, false), (false, true)],
        ),
        (
            "paired-with-joints-only-mismatch",
            vec![
                SecondaryInfluenceSet::Paired,
                SecondaryInfluenceSet::JointsOnly,
            ],
            serde_json::json!([{
                "set_index": 1,
                "joints_present": true,
                "weights_present": true,
                "joints_without_weights_present": true,
                "weights_without_joints_present": false,
            }]),
            vec![(true, true), (true, false)],
        ),
        (
            "paired-with-weights-only-mismatch",
            vec![
                SecondaryInfluenceSet::Paired,
                SecondaryInfluenceSet::WeightsOnly,
            ],
            serde_json::json!([{
                "set_index": 1,
                "joints_present": true,
                "weights_present": true,
                "joints_without_weights_present": false,
                "weights_without_joints_present": true,
            }]),
            vec![(true, true), (false, true)],
        ),
    ];

    for (name, sets, expected_sets, expected_source_presence) in cases {
        let dir = tempfile::tempdir().expect("creates temp directory");
        let input = dir.path().join(format!("{name}.gltf"));
        write_secondary_influence_gltf(&input, &sets);

        let source = gltf::Gltf::open(&input).expect("fixture is valid glTF");
        let source_primitives = source
            .meshes()
            .next()
            .expect("fixture mesh")
            .primitives()
            .collect::<Vec<_>>();
        assert_eq!(
            source_primitives.len(),
            expected_source_presence.len(),
            "{name}"
        );
        for (primitive, (has_joints, has_weights)) in
            source_primitives.iter().zip(expected_source_presence)
        {
            assert_eq!(
                primitive.get(&gltf::Semantic::Joints(1)).is_some(),
                has_joints,
                "{name}: JOINTS_1 fixture preflight"
            );
            assert_eq!(
                primitive.get(&gltf::Semantic::Weights(1)).is_some(),
                has_weights,
                "{name}: WEIGHTS_1 fixture preflight"
            );
        }

        let out = std::process::Command::new(env!("CARGO_BIN_EXE_animsmith"))
            .args([
                "measure",
                input.to_str().expect("utf-8 fixture path"),
                "--format",
                "json",
            ])
            .output()
            .expect("runs animsmith");
        assert!(
            out.status.success(),
            "{name}: measure exited {}; stderr:\n{}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid JSON");
        let measurements = &report["files"][0]["measurements"];
        assert_measurements_schema_valid(measurements);
        let mesh = &measurements["mesh_definitions"][0];
        assert_eq!(mesh["additional_influence_sets"], expected_sets, "{name}");
        if name == "cross-primitive-complementary-mismatch" {
            let mut missing_mismatch_field = measurements.clone();
            missing_mismatch_field["mesh_definitions"][0]["additional_influence_sets"][0]
                .as_object_mut()
                .expect("influence set object")
                .remove("weights_without_joints_present");
            assert_measurements_schema_invalid(&missing_mismatch_field);

            let mut inconsistent_mismatch = measurements.clone();
            inconsistent_mismatch["mesh_definitions"][0]["additional_influence_sets"][0]["weights_present"] =
                serde_json::json!(false);
            inconsistent_mismatch["mesh_definitions"][0]["additional_influence_sets"][0]["joints_without_weights_present"] =
                serde_json::json!(false);
            inconsistent_mismatch["mesh_definitions"][0]["additional_influence_sets"][0]["weights_without_joints_present"] =
                serde_json::json!(false);
            assert_measurements_schema_invalid(&inconsistent_mismatch);
        }
        assert_eq!(mesh["max_joints_per_vertex"], 2, "{name}: primary only");
        assert_eq!(mesh["weight_sum_min"], 1.0, "{name}: primary only");
        assert_eq!(mesh["weight_sum_max"], 1.0, "{name}: primary only");
    }
}

#[test]
fn cli_measure_reports_higher_independent_influence_sets_in_ascending_order() {
    let dir = tempfile::tempdir().expect("creates temp directory");
    let input = dir.path().join("higher-independent-influences.gltf");
    write_secondary_influence_gltf(&input, &[SecondaryInfluenceSet::HigherIndependentSets]);

    let source = gltf::Gltf::open(&input).expect("fixture is valid glTF");
    let primitive = source
        .meshes()
        .next()
        .expect("fixture mesh")
        .primitives()
        .next()
        .expect("fixture primitive");
    assert!(primitive.get(&gltf::Semantic::Joints(3)).is_some());
    assert!(primitive.get(&gltf::Semantic::Weights(2)).is_some());
    assert!(primitive.get(&gltf::Semantic::Joints(2)).is_none());
    assert!(primitive.get(&gltf::Semantic::Weights(3)).is_none());

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .args([
            "measure",
            input.to_str().expect("utf-8 fixture path"),
            "--format",
            "json",
        ])
        .output()
        .expect("runs animsmith");
    assert!(
        out.status.success(),
        "measure stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid JSON");
    let mesh = &report["files"][0]["measurements"]["mesh_definitions"][0];
    assert_eq!(
        mesh["additional_influence_sets"],
        serde_json::json!([
            { "set_index": 2, "joints_present": false, "weights_present": true, "joints_without_weights_present": false, "weights_without_joints_present": true },
            { "set_index": 3, "joints_present": true, "weights_present": false, "joints_without_weights_present": true, "weights_without_joints_present": false },
        ])
    );
}

/// Final lint JSON carries the same nested measurement evidence as measure.
#[test]
fn cli_lint_json_emits_mesh_measurements() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("skinned.glb");
    write_skinned_glb(&input);

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .arg("lint")
        .arg(&input)
        .arg("--format")
        .arg("json")
        .arg("--select")
        .arg("nan")
        .output()
        .expect("runs animsmith");
    assert!(out.status.success(), "lint exited {}", out.status);

    let report: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid JSON");
    let measurements = &report["files"][0]["measurements"];
    assert_measurements_schema_valid(measurements);
    assert_body_mesh(&measurements["mesh_definitions"][0]);
    assert_eq!(
        measurements["node_instances"][0]["static_node_world_aabb_unavailable_reason"],
        "skinned_deformation_excluded"
    );
    let findings: usize = report["files"][0]["checks"]
        .as_array()
        .expect("lint check records")
        .iter()
        .map(|check| check["findings"].as_array().expect("findings").len())
        .sum();
    assert_eq!(
        findings, 0,
        "measurement-only mesh evidence must not create findings"
    );
}

#[test]
fn cli_measure_distinguishes_definition_instance_and_scene_domains() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("nested-scene.gltf");
    write_nested_instanced_gltf(&input);

    let source = gltf::Gltf::open(&input).expect("fixture is valid glTF");
    assert_eq!(source.animations().count(), 1, "fixture carries animation");
    assert_eq!(
        source
            .meshes()
            .next()
            .expect("shared mesh")
            .primitives()
            .next()
            .expect("shared primitive")
            .morph_targets()
            .count(),
        1,
        "fixture carries a morph target"
    );

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .arg("measure")
        .arg(&input)
        .arg("--format")
        .arg("json")
        .output()
        .expect("runs animsmith");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid JSON");
    let measurements = &report["files"][0]["measurements"];
    assert_measurements_schema_valid(measurements);
    assert!(
        measurements["clips"]["move"].is_object(),
        "the animated transform was loaded but must not affect static bounds"
    );

    let definitions = measurements["mesh_definitions"]
        .as_array()
        .expect("definitions");
    assert_eq!(definitions.len(), 2, "uninstanced definition is retained");
    assert_eq!(definitions[0]["mesh_index"], 0);
    assert_eq!(definitions[1]["mesh_index"], 1);
    assert!(
        definitions
            .iter()
            .all(|definition| definition["name"] == "duplicate-definition"),
        "definition identity comes from source indices, not names"
    );
    assert_eq!(
        definitions[0]["geometry_aabb"],
        serde_json::json!({ "min": [0.0, 0.0, 0.0], "max": [1.0, 1.0, 0.0] })
    );
    assert_eq!(
        definitions[0]["vertex_count"], 3,
        "indices do not recount vertices"
    );
    assert_vec3_close(
        &definitions[0]["geometry_centroid"],
        [1.0 / 3.0, 1.0 / 3.0, 0.0],
    );
    assert_eq!(
        definitions[0]["geometry_centroid"], definitions[1]["geometry_centroid"],
        "node placement, animation, morphs, and instancing do not alter definition-local centroid"
    );

    let instances = measurements["node_instances"]
        .as_array()
        .expect("instances");
    assert_eq!(instances.len(), 3);
    assert_eq!(
        instances
            .iter()
            .map(|instance| instance["node_index"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        [2, 3, 4]
    );
    assert!(
        instances.iter().all(|instance| instance["mesh_index"] == 0),
        "three nodes share one definition"
    );
    assert_eq!(instances[0]["node_name"], "duplicate");
    assert_eq!(instances[1]["node_name"], "duplicate");
    assert_aabb_close(
        &instances[0]["static_node_world_aabb"],
        [7.0, -2.0, 0.0],
        [10.0, 0.0, 0.0],
    );
    assert_eq!(
        instances[1]["static_node_world_aabb"],
        serde_json::json!({ "min": [-4.0, 1.0, 0.0], "max": [-3.0, 2.0, 0.0] })
    );
    assert_eq!(
        instances[2]["static_node_world_aabb"],
        serde_json::json!({ "min": [100.0, 0.0, 0.0], "max": [101.0, 1.0, 0.0] })
    );

    let scenes = measurements["scenes"].as_array().expect("scenes");
    assert_eq!(scenes.len(), 2);
    assert_eq!(measurements["default_scene_index"], 1);
    assert_eq!(scenes[0]["instance_count"], 2);
    assert_aabb_close(
        &scenes[0]["static_scene_world_aabb"],
        [-4.0, -2.0, 0.0],
        [10.0, 2.0, 0.0],
    );
    assert_eq!(scenes[1]["instance_count"], 1);
    assert_eq!(
        scenes[1]["static_scene_world_aabb"],
        instances[1]["static_node_world_aabb"]
    );
    assert!(
        scenes
            .iter()
            .all(|scene| scene["excluded_instance_count"] == 0),
        "all unskinned static bounds contribute"
    );
}

/// A skeleton-only glTF emits explicit empty definition/instance arrays.
#[test]
fn cli_measure_emits_empty_asset_domains_when_no_geometry() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("skeleton.glb");
    let doc = Document {
        skeleton: Skeleton {
            bones: vec![Bone {
                name: "root".into(),
                parent: None,
                rest: Transform::IDENTITY,
                inverse_bind: None,
            }],
        },
        clips: vec![],
        assets: SceneAssets::default(),
        source: SourceInfo::default(),
    };
    animsmith_gltf::write::write(&doc, &input).expect("writes");

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .arg("measure")
        .arg(&input)
        .arg("--format")
        .arg("json")
        .output()
        .expect("runs animsmith");
    assert!(out.status.success());
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid JSON");
    let measurements = &report["files"][0]["measurements"];
    assert_eq!(measurements["mesh_definitions"], serde_json::json!([]));
    assert_eq!(measurements["node_instances"], serde_json::json!([]));
    assert!(measurements["scenes"].is_array());
}

/// A mesh whose positions are all non-finite must not emit a bounding
/// box — an inf/-inf box serializes to JSON `null`, which violates the
/// numeric schema. The `aabb` key is omitted end-to-end; the vertex
/// count still reports.
#[test]
fn cli_measure_omits_aabb_for_non_finite_geometry() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("nan.glb");
    let doc = Document {
        skeleton: Skeleton {
            bones: vec![Bone {
                name: "root".into(),
                parent: None,
                rest: Transform::IDENTITY,
                inverse_bind: None,
            }],
        },
        clips: vec![],
        assets: SceneAssets {
            meshes: vec![MeshAsset {
                name: "nan".into(),
                source_mesh_index: 0,
                primitives: vec![Primitive {
                    positions: vec![Vec3::splat(f32::NAN); 3],
                    ..Primitive::default()
                }],
            }],
            instances: vec![MeshInstance {
                source_node_index: 0,
                node: 0,
                mesh: 0,
                ..MeshInstance::default()
            }],
            materials: vec![],
            ..SceneAssets::default()
        },
        source: SourceInfo::default(),
    };
    animsmith_gltf::write::write(&doc, &input).expect("writes");

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .arg("measure")
        .arg(&input)
        .arg("--format")
        .arg("json")
        .output()
        .expect("runs animsmith");
    assert!(out.status.success());
    let raw = String::from_utf8(out.stdout).expect("utf8");
    assert!(
        !raw.contains("null"),
        "no null must appear in measure JSON:\n{raw}"
    );
    let report: serde_json::Value = serde_json::from_str(&raw).expect("valid JSON");
    let measurements = &report["files"][0]["measurements"];
    let mesh = &measurements["mesh_definitions"][0];
    assert_eq!(mesh["vertex_count"], 3, "count still reported");
    assert!(
        mesh.get("geometry_aabb").is_none(),
        "no bounding box from non-finite geometry"
    );
    assert!(
        mesh.get("geometry_centroid").is_none(),
        "no centroid from non-finite geometry"
    );
    assert_eq!(
        measurements["node_instances"][0]["static_node_world_aabb_unavailable_reason"],
        "no_finite_positions"
    );
}

#[test]
fn published_schema_rejects_availability_status_value_mismatches() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("nested-scene.gltf");
    write_nested_instanced_gltf(&input);

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .arg("measure")
        .arg(&input)
        .arg("--format")
        .arg("json")
        .output()
        .expect("runs animsmith");
    assert!(out.status.success());
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid JSON");
    let measurements = &report["files"][0]["measurements"];
    assert_measurements_schema_valid(measurements);
    let clip = &measurements["clips"]["move"];
    assert!(clip.is_object(), "fixture must carry the animated clip");

    let mut missing_bone_channels = clip.clone();
    missing_bone_channels
        .as_object_mut()
        .unwrap()
        .remove("bone_channels");
    let mut invalid = measurements.clone();
    invalid["clips"]["move"] = missing_bone_channels;
    assert_measurements_schema_invalid(&invalid);

    for properties in [
        serde_json::json!([]),
        serde_json::json!(["translation", "translation"]),
        serde_json::json!(["weights"]),
        serde_json::json!(["rotation", "translation"]),
    ] {
        let mut invalid_channels = clip.clone();
        invalid_channels["bone_channels"] = serde_json::json!([{
            "bone_index": 0,
            "bone_name": "root",
            "properties": properties
        }]);
        let mut invalid = measurements.clone();
        invalid["clips"]["move"] = invalid_channels;
        assert_measurements_schema_invalid(&invalid);
    }

    let duplicate_channel = serde_json::json!({
        "bone_index": 0,
        "bone_name": "root",
        "properties": ["translation"]
    });
    let mut duplicate_channels = clip.clone();
    duplicate_channels["bone_channels"] =
        serde_json::json!([duplicate_channel.clone(), duplicate_channel]);
    let mut invalid = measurements.clone();
    invalid["clips"]["move"] = duplicate_channels;
    assert_measurements_schema_invalid(&invalid);

    let mut duplicate_animated_bones = clip.clone();
    duplicate_animated_bones["animated_bones"] = serde_json::json!(["root", "root"]);
    let mut invalid = measurements.clone();
    invalid["clips"]["move"] = duplicate_animated_bones;
    assert_measurements_schema_invalid(&invalid);

    // A `measured` availability status without its sibling value is invalid.
    let mut declared_measured_without_value = clip.clone();
    declared_measured_without_value["loop_continuity_availability"] = serde_json::json!("measured");
    let mut missing_value = measurements.clone();
    missing_value["clips"]["move"] = declared_measured_without_value;
    assert_measurements_schema_invalid(&missing_value);

    // A present value under a `not_applicable`/`unavailable` status is invalid.
    let mut spurious_value = clip.clone();
    spurious_value["loop_seam_ratio"] = serde_json::json!(0.2);
    spurious_value["loop_seam_ratio_availability"] = serde_json::json!("not_applicable");
    let mut invalid = measurements.clone();
    invalid["clips"]["move"] = spurious_value;
    assert_measurements_schema_invalid(&invalid);

    // An unrecognized availability status is invalid.
    let mut unknown_status = clip.clone();
    unknown_status["speed_mps_availability"] = serde_json::json!("maybe");
    let mut invalid_status = measurements.clone();
    invalid_status["clips"]["move"] = unknown_status;
    assert_measurements_schema_invalid(&invalid_status);

    // A `gait` object whose `phase` value disagrees with its own
    // `phase_availability` status is invalid even when the parent `gait`
    // availability is consistent.
    let mut gait_mismatch = clip.clone();
    gait_mismatch["gait_availability"] = serde_json::json!("measured");
    gait_mismatch["gait"] = serde_json::json!({
        "phase": 0.25,
        "phase_availability": "not_applicable",
        "lr_amplitude_m": 0.1
    });
    let mut invalid_gait = measurements.clone();
    invalid_gait["clips"]["move"] = gait_mismatch;
    assert_measurements_schema_invalid(&invalid_gait);

    let mut root_clip = clip.clone();
    root_clip["root_trajectory_availability"] = serde_json::json!("measured");
    root_clip["root_trajectory"] = serde_json::json!({
        "bone_index": 0,
        "bone_name": "root",
        "source_role": "root",
        "translation": {
            "horizontal_displacement_x_m": 1.0,
            "horizontal_displacement_z_m": -2.0,
            "horizontal_travel_m": 3.0,
            "vertical_displacement_m": 0.5,
            "vertical_min_displacement_m": -0.25,
            "vertical_max_displacement_m": 1.0
        },
        "translation_availability": "measured",
        "yaw": {
            "heading_axis": "positive_z",
            "net_yaw_deg": 90.0,
            "unwrapped_yaw_deg": 450.0,
            "yaw_travel_deg": 540.0
        },
        "yaw_availability": "measured"
    });
    let mut valid_root = measurements.clone();
    valid_root["clips"]["move"] = root_clip.clone();
    assert_measurements_schema_valid(&valid_root);

    for path in ["translation", "yaw"] {
        let mut missing_nested_value = root_clip.clone();
        missing_nested_value["root_trajectory"]
            .as_object_mut()
            .unwrap()
            .remove(path);
        let mut invalid = measurements.clone();
        invalid["clips"]["move"] = missing_nested_value;
        assert_measurements_schema_invalid(&invalid);

        let mut spurious_nested_value = root_clip.clone();
        spurious_nested_value["root_trajectory"][format!("{path}_availability")] =
            serde_json::json!("unavailable");
        let mut invalid = measurements.clone();
        invalid["clips"]["move"] = spurious_nested_value;
        assert_measurements_schema_invalid(&invalid);

        let mut nested_not_applicable = root_clip.clone();
        nested_not_applicable["root_trajectory"]
            .as_object_mut()
            .unwrap()
            .remove(path);
        nested_not_applicable["root_trajectory"][format!("{path}_availability")] =
            serde_json::json!("not_applicable");
        let mut invalid = measurements.clone();
        invalid["clips"]["move"] = nested_not_applicable;
        assert_measurements_schema_invalid(&invalid);
    }

    let mut missing_root_value = root_clip.clone();
    missing_root_value
        .as_object_mut()
        .unwrap()
        .remove("root_trajectory");
    let mut invalid = measurements.clone();
    invalid["clips"]["move"] = missing_root_value;
    assert_measurements_schema_invalid(&invalid);

    let mut spurious_root_value = root_clip.clone();
    spurious_root_value["root_trajectory_availability"] = serde_json::json!("unavailable");
    let mut invalid = measurements.clone();
    invalid["clips"]["move"] = spurious_root_value;
    assert_measurements_schema_invalid(&invalid);

    for (field, invalid_value) in [
        ("heading_axis", serde_json::json!("negative_z")),
        ("net_yaw_deg", serde_json::json!(180.1)),
        ("yaw_travel_deg", serde_json::json!(-0.1)),
    ] {
        let mut invalid_yaw = root_clip.clone();
        invalid_yaw["root_trajectory"]["yaw"][field] = invalid_value;
        let mut invalid = measurements.clone();
        invalid["clips"]["move"] = invalid_yaw;
        assert_measurements_schema_invalid(&invalid);
    }

    let mut invalid_translation = root_clip;
    invalid_translation["root_trajectory"]["translation"]["horizontal_travel_m"] =
        serde_json::json!(-0.1);
    let mut invalid = measurements.clone();
    invalid["clips"]["move"] = invalid_translation;
    assert_measurements_schema_invalid(&invalid);
}
