//! CI-run coverage that `convert` now carries glTF-input geometry (#16)
//! and that `--animation-only` still strips it — the uniform behaviour
//! #33 promises, exercised on an in-repo glTF (no licensed FBX needed,
//! unlike `convert_mesh.rs`).
#![cfg(feature = "fbx")] // the `convert` subcommand is gated on the fbx feature

use animsmith_core::glam::Vec3;
use animsmith_core::model::*;

fn mesh_count(glb: &std::path::Path) -> usize {
    let bytes = std::fs::read(glb).unwrap();
    gltf::Gltf::from_slice(&bytes)
        .expect("valid glTF")
        .meshes()
        .count()
}

/// Positions of the first mesh's first primitive in a GLB — so a
/// carry-through test can assert the actual geometry survived, not just
/// that *a* mesh exists.
fn first_primitive_positions(glb: &std::path::Path) -> Vec<[f32; 3]> {
    let bytes = std::fs::read(glb).unwrap();
    let gltf = gltf::Gltf::from_slice(&bytes).expect("valid glTF");
    let blob = gltf.blob.clone().expect("BIN chunk");
    let prim = gltf
        .meshes()
        .next()
        .expect("mesh")
        .primitives()
        .next()
        .expect("primitive");
    prim.reader(|b| Some(&blob[..b.length()]))
        .read_positions()
        .expect("POSITION")
        .collect()
}

/// Author a minimal geometry-carrying GLB (one unindexed triangle) to
/// feed `convert` as input.
fn write_geometry_glb(path: &std::path::Path) {
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
                name: "tri".into(),
                node: 0,
                primitives: vec![Primitive {
                    positions: vec![
                        Vec3::new(0.0, 0.0, 0.0),
                        Vec3::new(1.0, 0.0, 0.0),
                        Vec3::new(0.0, 1.0, 0.0),
                    ],
                    ..Primitive::default()
                }],
                skin_joints: vec![],
                skin_ibms: vec![],
            }],
            materials: vec![],
        },
        source: SourceInfo::default(),
    };
    animsmith_gltf::write::write(&doc, path).expect("writes input glb");
}

#[test]
fn cli_convert_gltf_input_carries_and_strips_geometry() {
    let dir =
        std::env::temp_dir().join(format!("animsmith-cli-convert-gltf-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    let input = dir.join("in.glb");
    write_geometry_glb(&input);
    assert_eq!(mesh_count(&input), 1, "input GLB carries a mesh");

    let convert = |out: &std::path::Path, animation_only: bool| {
        let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_animsmith"));
        cmd.arg("convert").arg(&input).arg("-o").arg(out);
        if animation_only {
            cmd.arg("--animation-only");
        }
        let status = cmd.status().expect("runs animsmith");
        assert!(status.success(), "convert exited {status}");
    };

    // Default: glTF geometry now flows through the loader into the output.
    let carried = dir.join("carried.glb");
    convert(&carried, false);
    assert_eq!(
        mesh_count(&carried),
        1,
        "convert carries glTF-input geometry through (#16)"
    );
    // Not just *a* mesh — the actual fixture triangle survived.
    assert_eq!(
        first_primitive_positions(&carried),
        vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        "carried geometry matches the input triangle"
    );

    // `--animation-only` still drops it, uniformly across formats.
    let stripped = dir.join("stripped.glb");
    convert(&stripped, true);
    assert_eq!(
        mesh_count(&stripped),
        0,
        "convert --animation-only strips geometry"
    );
}
