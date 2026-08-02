//! End-to-end CLI contract for canonical static mesh transform baking.

#![cfg(feature = "fbx")]

use animsmith_core::glam::{Mat3, Quat, Vec3};
use animsmith_core::model::{
    Bone, Document, MeshAsset, MeshInstance, Primitive, SceneAsset, SceneAssets, Skeleton,
    Transform,
};
use serde_json::Value;
use std::path::Path;
use std::process::{Command, Output};

const CONVERSION_SCHEMA: &str =
    include_str!("../../../docs/schemas/conversion-evidence-v1.schema.json");
const EPSILON: f32 = 1.0e-5;

fn fixture() -> Document {
    Document {
        skeleton: Skeleton {
            bones: vec![
                Bone {
                    name: "normalized-root".into(),
                    parent: None,
                    rest: Transform {
                        translation: Vec3::new(2.0, -3.0, 4.0),
                        rotation: Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
                        scale: Vec3::new(2.0, 3.0, 4.0),
                    },
                    inverse_bind: None,
                },
                Bone {
                    name: "static-mesh".into(),
                    parent: Some(0),
                    rest: Transform {
                        translation: Vec3::new(1.0, 0.5, -2.0),
                        rotation: Quat::from_rotation_x(0.3),
                        scale: Vec3::new(1.0, 0.5, 2.0),
                    },
                    inverse_bind: None,
                },
            ],
        },
        assets: SceneAssets {
            meshes: vec![MeshAsset {
                name: "asymmetric-triangle".into(),
                source_mesh_index: 17,
                primitives: vec![Primitive {
                    indices: vec![0, 1, 2],
                    positions: vec![
                        Vec3::new(-1.0, 0.25, 0.5),
                        Vec3::new(2.0, -0.75, 1.0),
                        Vec3::new(0.5, 1.5, -0.25),
                    ],
                    normals: vec![Vec3::new(1.0, 2.0, 3.0).normalize(); 3],
                    uvs: vec![[0.0, 0.0], [1.0, 0.0], [0.25, 1.0]],
                    ..Primitive::default()
                }],
            }],
            instances: vec![MeshInstance {
                source_node_index: 23,
                node: 1,
                mesh: 0,
                ..MeshInstance::default()
            }],
            scenes: vec![SceneAsset {
                source_scene_index: 0,
                name: Some("authored-scene".into()),
                roots: vec![0],
            }],
            default_scene: Some(0),
            ..SceneAssets::default()
        },
        ..Document::default()
    }
}

fn run_bake(input: &Path, output: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .arg("convert")
        .arg(input)
        .arg("-o")
        .arg(output)
        .arg("--bake-static-mesh-transforms")
        .arg("--format")
        .arg("json")
        .output()
        .expect("runs animsmith convert")
}

fn assert_vec3_close(actual: Vec3, expected: Vec3) {
    assert!(
        actual.abs_diff_eq(expected, EPSILON),
        "got {actual:?}, expected {expected:?} within {EPSILON}"
    );
}

#[test]
fn convert_static_bake_emits_schema_valid_evidence_and_byte_stable_identity_output() {
    let dir = tempfile::tempdir().expect("creates temp directory");
    let input = dir.path().join("input.glb");
    let output_a = dir.path().join("output-a.glb");
    let output_b = dir.path().join("output-b.glb");
    animsmith_gltf::write::write(&fixture(), &input).expect("writes input fixture");

    let loaded_input = animsmith_gltf::load(&input).expect("reloads input fixture");
    let instance = &loaded_input.assets.instances[0];
    let root_world = loaded_input.skeleton.bones[0].rest.to_mat4();
    let world = root_world * loaded_input.skeleton.bones[instance.node].rest.to_mat4();
    let normal_matrix = Mat3::from_mat4(world).inverse().transpose();
    let source_primitive = &loaded_input.assets.meshes[instance.mesh].primitives[0];
    let expected_positions = source_primitive
        .positions
        .iter()
        .map(|position| world.transform_point3(*position))
        .collect::<Vec<_>>();
    let expected_normals = source_primitive
        .normals
        .iter()
        .map(|normal| (normal_matrix * *normal).normalize())
        .collect::<Vec<_>>();

    let first = run_bake(&input, &output_a);
    assert!(
        first.status.success(),
        "convert failed: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    let evidence: Value = serde_json::from_slice(&first.stdout).expect("stdout is JSON evidence");
    let schema: Value = serde_json::from_str(CONVERSION_SCHEMA).expect("schema is JSON");
    let validator = jsonschema::validator_for(&schema).expect("schema compiles");
    assert!(
        validator.is_valid(&evidence),
        "evidence does not satisfy conversion schema: {evidence:#}"
    );
    assert_eq!(evidence["schema_version"], 1);
    assert_eq!(
        evidence["schema"],
        "urn:animsmith:schema:conversion-evidence:1"
    );
    assert_eq!(evidence["artifact"]["meshes"], 1);
    assert_eq!(evidence["artifact"]["primitive_positions"], 3);
    assert_eq!(
        evidence["static_mesh_bake"]["output_root_is_identity"],
        true
    );
    let entries = evidence["static_mesh_bake"]["entries"]
        .as_array()
        .expect("bake entries array");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["source_node_index"], instance.source_node_index);
    assert_eq!(entries[0]["world_transform"].as_array().unwrap().len(), 16);

    let second = run_bake(&input, &output_b);
    assert!(
        second.status.success(),
        "second convert failed: {}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert_eq!(
        std::fs::read(&output_a).unwrap(),
        std::fs::read(&output_b).unwrap(),
        "same-platform conversion writes byte-identical GLB artifacts"
    );

    let baked = animsmith_gltf::load(&output_a).expect("reloads baked output");
    assert!(
        baked
            .skeleton
            .bones
            .iter()
            .all(|bone| bone.rest == Transform::IDENTITY),
        "the baked scene has no hidden transform"
    );
    assert_eq!(baked.assets.instances.len(), 1);
    let primitive = &baked.assets.meshes[baked.assets.instances[0].mesh].primitives[0];
    assert_eq!(primitive.indices, source_primitive.indices);
    assert_eq!(primitive.uvs, source_primitive.uvs);
    for (actual, expected) in primitive.positions.iter().zip(expected_positions) {
        assert_vec3_close(*actual, expected);
    }
    for (actual, expected) in primitive.normals.iter().zip(expected_normals) {
        assert_vec3_close(*actual, expected);
    }

    let gltf = gltf::Gltf::from_slice(&std::fs::read(&output_a).unwrap()).expect("valid GLB");
    let scene = gltf.default_scene().expect("default baked scene");
    assert_eq!(scene.nodes().count(), 1, "one canonical scene root");
    assert!(gltf.nodes().all(|node| {
        let (translation, rotation, scale) = node.transform().decomposed();
        translation == [0.0, 0.0, 0.0]
            && rotation == [0.0, 0.0, 0.0, 1.0]
            && scale == [1.0, 1.0, 1.0]
    }));
}

#[test]
fn convert_static_bake_rejects_shared_mesh_before_creating_output() {
    let dir = tempfile::tempdir().expect("creates temp directory");
    let input = dir.path().join("shared.glb");
    let output = dir.path().join("must-not-exist.glb");
    let mut shared = fixture();
    shared.assets.instances.push(MeshInstance {
        source_node_index: 24,
        node: 0,
        mesh: 0,
        ..MeshInstance::default()
    });
    animsmith_gltf::write::write(&shared, &input).expect("writes shared input fixture");

    let result = run_bake(&input, &output);
    assert_eq!(result.status.code(), Some(2));
    assert!(
        result.stdout.is_empty(),
        "operator failures emit no JSON evidence"
    );
    assert!(!output.exists(), "validation fails before output creation");
}
