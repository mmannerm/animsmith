//! CI-run coverage that `measure` emits per-mesh geometry measurements
//! (#16) end-to-end through the real CLI on an in-repo glTF — vertex
//! count, AABB, joints-per-vertex, and weight-sum range.
#![cfg(feature = "fbx")] // convert/measure share the assets-aware loader path

use animsmith_core::glam::{Mat4, Vec3};
use animsmith_core::model::*;

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
                node: 0,
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
                skin_joints: vec![0, 1],
                skin_ibms: vec![],
            }],
            materials: vec![],
        },
        source: SourceInfo::default(),
    };
    animsmith_gltf::write::write(&doc, path).expect("writes skinned glb");
}

#[test]
fn cli_measure_emits_mesh_measurements() {
    let dir = std::env::temp_dir().join(format!("animsmith-measure-mesh-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let input = dir.join("skinned.glb");
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
    let mesh = &report["files"][0]["meshes"][0];
    assert_eq!(mesh["name"], "body");
    assert_eq!(mesh["vertex_count"], 3);
    assert_eq!(mesh["aabb"]["min"], serde_json::json!([0.0, 0.0, 0.0]));
    assert_eq!(mesh["aabb"]["max"], serde_json::json!([2.0, 4.0, 0.0]));
    assert_eq!(mesh["max_joints_per_vertex"], 2);
    let lo = mesh["weight_sum_min"].as_f64().expect("weight_sum_min");
    let hi = mesh["weight_sum_max"].as_f64().expect("weight_sum_max");
    assert!(
        (lo - 1.0).abs() < 1e-6 && (hi - 1.0).abs() < 1e-6,
        "weights normalized"
    );
}

/// A skeleton-only glTF (no geometry) emits no `meshes` key — the field
/// is omitted when empty, so asset-less inputs keep their v1 output.
#[test]
fn cli_measure_omits_meshes_when_no_geometry() {
    let dir = std::env::temp_dir().join(format!("animsmith-measure-nomesh-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let input = dir.join("skeleton.glb");
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
    assert!(
        report["files"][0].get("meshes").is_none(),
        "no meshes key without geometry"
    );
}

/// A mesh whose positions are all non-finite must not emit a bounding
/// box — an inf/-inf box serializes to JSON `null`, which violates the
/// numeric schema. The `aabb` key is omitted end-to-end; the vertex
/// count still reports.
#[test]
fn cli_measure_omits_aabb_for_non_finite_geometry() {
    let dir = std::env::temp_dir().join(format!("animsmith-measure-nan-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let input = dir.join("nan.glb");
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
                node: 0,
                primitives: vec![Primitive {
                    positions: vec![Vec3::splat(f32::NAN); 3],
                    ..Primitive::default()
                }],
                skin_joints: vec![],
                skin_ibms: vec![],
            }],
            materials: vec![],
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
    let mesh = &report["files"][0]["meshes"][0];
    assert_eq!(mesh["vertex_count"], 3, "count still reported");
    assert!(
        mesh.get("aabb").is_none(),
        "no bounding box from non-finite geometry"
    );
}
