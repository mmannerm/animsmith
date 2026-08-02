//! CI-run coverage that `measure` emits per-mesh geometry measurements
//! (#16) end-to-end through the real CLI on an in-repo glTF — vertex
//! count, AABB, joints-per-vertex, and weight-sum range.

use animsmith_core::glam::{Mat4, Vec3};
use animsmith_core::model::*;

fn write_nested_instanced_gltf(path: &std::path::Path) {
    let positions = [
        [0.0_f32, 0.0, 0.0],
        [1.0_f32, 0.0, 0.0],
        [0.0_f32, 1.0, 0.0],
    ];
    let morph_deltas = [[50.0_f32, 0.0, 0.0]; 3];
    let animation_times = [0.0_f32, 1.0];
    let animated_translations = [[1000.0_f32, 0.0, 0.0]; 2];
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
    let buffer = path.with_file_name("nested-scene.bin");
    std::fs::write(&buffer, &bytes).expect("writes position buffer");
    let document = serde_json::json!({
        "asset": { "version": "2.0" },
        "buffers": [{ "uri": "nested-scene.bin", "byteLength": bytes.len() }],
        "bufferViews": [
            { "buffer": 0, "byteLength": morph_offset },
            { "buffer": 0, "byteOffset": morph_offset, "byteLength": animation_times_offset - morph_offset },
            { "buffer": 0, "byteOffset": animation_times_offset, "byteLength": animation_values_offset - animation_times_offset },
            { "buffer": 0, "byteOffset": animation_values_offset, "byteLength": bytes.len() - animation_values_offset }
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
            }
        ],
        "meshes": [
            {
                "name": "shared",
                "weights": [1.0],
                "primitives": [{
                    "attributes": { "POSITION": 0 },
                    "targets": [{ "POSITION": 1 }]
                }]
            },
            { "name": "uninstanced", "primitives": [{ "attributes": { "POSITION": 0 } }] }
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
    assert_body_mesh(&measurements["mesh_definitions"][0]);
    assert_eq!(
        measurements["node_instances"][0]["static_node_world_aabb_unavailable_reason"],
        "skinned_deformation_excluded"
    );
    assert_eq!(measurements["scenes"][0]["instance_count"], 1);
    assert_eq!(measurements["scenes"][0]["excluded_instance_count"], 1);
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
    assert_eq!(
        definitions[0]["geometry_aabb"],
        serde_json::json!({ "min": [0.0, 0.0, 0.0], "max": [1.0, 1.0, 0.0] })
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
    assert_eq!(
        measurements["node_instances"][0]["static_node_world_aabb_unavailable_reason"],
        "no_finite_positions"
    );
}
