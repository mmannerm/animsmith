//! CI-run coverage that `convert` now carries glTF-input geometry (#16)
//! and that `--animation-only` still strips it — the uniform behaviour
//! #33 promises, exercised on an in-repo glTF (no licensed FBX needed,
//! unlike `convert_mesh.rs`).
#![cfg(feature = "fbx")] // the `convert` subcommand is gated on the fbx feature

use animsmith_core::glam::{Quat, Vec3};
use animsmith_core::model::*;

/// A valid 1×1 white PNG used to pin embedded texture bytes through the CLI.
const TINY_PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0xF8, 0xFF, 0xFF, 0x3F,
    0x00, 0x05, 0xFE, 0x02, 0xFE, 0xA7, 0x35, 0x81, 0x84, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E,
    0x44, 0xAE, 0x42, 0x60, 0x82,
];

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

/// Assert that the first primitive still points through its material to the
/// original embedded base-color image.
fn assert_embedded_base_color_texture(glb: &std::path::Path) {
    let bytes = std::fs::read(glb).unwrap();
    let gltf = gltf::Gltf::from_slice(&bytes).expect("valid glTF");
    let blob = gltf.blob.as_ref().expect("BIN chunk");
    let primitive = gltf
        .meshes()
        .next()
        .expect("mesh")
        .primitives()
        .next()
        .expect("primitive");
    let material = primitive.material();
    assert_eq!(material.index(), Some(0), "primitive keeps its material");

    let image = material
        .pbr_metallic_roughness()
        .base_color_texture()
        .expect("material keeps its base-color texture")
        .texture()
        .source()
        .source();
    let gltf::image::Source::View { view, mime_type } = image else {
        panic!("base-color texture must remain embedded");
    };
    assert_eq!(mime_type, "image/png");
    assert_eq!(
        &blob[view.offset()..view.offset() + view.length()],
        TINY_PNG,
        "embedded image bytes survive"
    );
}

/// Author a minimal animated and textured GLB (one unindexed triangle) to
/// feed the public scene-writing commands as input.
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
        clips: vec![Clip {
            name: "spin".into(),
            duration_s: 1.0,
            tracks: vec![Track {
                bone: 0,
                property: Property::Rotation,
                interpolation: Interpolation::Linear,
                times: vec![0.0, 1.0],
                values: TrackValues::Quats(vec![Quat::IDENTITY, Quat::from_rotation_y(1.0)]),
            }],
        }],
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
                    uvs: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
                    material: Some(0),
                    ..Primitive::default()
                }],
                skin_joints: vec![],
                skin_ibms: vec![],
            }],
            materials: vec![MaterialAsset {
                name: "white".into(),
                base_color: [1.0; 4],
                metallic: 0.0,
                roughness: 1.0,
                base_color_texture: Some(TextureAsset {
                    bytes: TINY_PNG.to_vec(),
                    mime: "image/png".into(),
                }),
            }],
        },
        source: SourceInfo::default(),
    };
    animsmith_gltf::write::write(&doc, path).expect("writes input glb");
}

#[test]
fn cli_convert_preserves_embedded_base_color_texture() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("textured.glb");
    let output = dir.path().join("converted.glb");
    write_geometry_glb(&input);

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .arg("convert")
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .status()
        .expect("runs animsmith convert");
    assert!(status.success(), "convert exited {status}");

    assert_embedded_base_color_texture(&output);
}

#[test]
fn cli_transform_preserves_embedded_base_color_texture() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("textured.glb");
    let output = dir.path().join("transformed.glb");
    write_geometry_glb(&input);

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .arg("transform")
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .arg("--hold-extend")
        .arg("0.25")
        .status()
        .expect("runs animsmith transform");
    assert!(status.success(), "transform exited {status}");

    assert_embedded_base_color_texture(&output);
}

#[test]
fn cli_convert_gltf_input_carries_and_strips_geometry() {
    let dir = tempfile::tempdir().unwrap();

    let input = dir.path().join("in.glb");
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
    let carried = dir.path().join("carried.glb");
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
    let stripped = dir.path().join("stripped.glb");
    convert(&stripped, true);
    assert_eq!(
        mesh_count(&stripped),
        0,
        "convert --animation-only strips geometry"
    );
}
