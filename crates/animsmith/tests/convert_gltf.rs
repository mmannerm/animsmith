//! CI-run coverage that scene-writing commands preserve glTF-input assets.
//! The `transform` case runs in every feature set; the `convert` case is
//! feature-gated with its subcommand and also pins `--animation-only`.

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

/// Distinct JPEG-like payload; image decoding is deliberately outside the
/// scene round-trip contract, which preserves these bytes opaquely.
const TINY_JPEG: &[u8] = &[
    0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F', 0xFF, 0xD9,
];

#[cfg(feature = "fbx")]
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
#[cfg(feature = "fbx")]
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

/// Assert that the first primitive still points through its named material to
/// the original embedded base-color image, independent of how it is embedded.
fn assert_embedded_base_color_textures(glb: &std::path::Path) -> usize {
    let bytes = std::fs::read(glb).unwrap();
    let gltf = gltf::Gltf::from_slice(&bytes).expect("valid glTF");
    let primitive = gltf
        .meshes()
        .next()
        .expect("mesh")
        .primitives()
        .next()
        .expect("primitive");
    let material = primitive.material();
    assert_eq!(material.name(), Some("bound-jpeg"));
    let material_index = material.index().expect("primitive keeps a material");

    // Loading resolves either buffer-view or data-URI image storage into the
    // public scene model, keeping this oracle about semantics rather than the
    // writer's current representation.
    let doc = animsmith_gltf::load(glb).expect("loads output scene");
    let loaded_material_index = doc.assets.meshes[0].primitives[0]
        .material
        .expect("loaded primitive keeps a material");
    let texture = doc.assets.materials[loaded_material_index]
        .base_color_texture
        .as_ref()
        .expect("linked material keeps its base-color texture");
    assert_eq!(texture.mime, "image/jpeg");
    assert_eq!(texture.bytes, TINY_JPEG, "embedded image bytes survive");

    let unused_texture = doc
        .assets
        .materials
        .iter()
        .find(|material| material.name == "unused-png")
        .expect("unreferenced material survives")
        .base_color_texture
        .as_ref()
        .expect("unreferenced material keeps its base-color texture");
    assert_eq!(unused_texture.mime, "image/png");
    assert_eq!(
        unused_texture.bytes, TINY_PNG,
        "unreferenced embedded image bytes survive"
    );
    material_index
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
                    material: Some(1),
                    ..Primitive::default()
                }],
                skin_joints: vec![],
                skin_ibms: vec![],
            }],
            materials: vec![
                MaterialAsset {
                    name: "unused-png".into(),
                    base_color: [1.0; 4],
                    metallic: 0.0,
                    roughness: 1.0,
                    base_color_texture: Some(TextureAsset {
                        bytes: TINY_PNG.to_vec(),
                        mime: "image/png".into(),
                    }),
                },
                MaterialAsset {
                    name: "bound-jpeg".into(),
                    base_color: [1.0; 4],
                    metallic: 0.0,
                    roughness: 1.0,
                    base_color_texture: Some(TextureAsset {
                        bytes: TINY_JPEG.to_vec(),
                        mime: "image/jpeg".into(),
                    }),
                },
            ],
        },
        source: SourceInfo::default(),
    };
    animsmith_gltf::write::write(&doc, path).expect("writes input glb");
    assert_eq!(
        assert_embedded_base_color_textures(path),
        1,
        "fixture exercises nonzero material and image linkage"
    );
}

#[test]
fn cli_transform_preserves_embedded_base_color_textures() {
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

    assert_embedded_base_color_textures(&output);
}

#[test]
#[cfg(feature = "fbx")]
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
    assert_embedded_base_color_textures(&carried);
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
