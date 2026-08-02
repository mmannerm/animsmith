//! End-to-end CLI contract for declarative material texture recipes.

#![cfg(feature = "fbx")]

use animsmith_core::glam::Vec3;
use animsmith_core::model::{
    Bone, Document, MaterialAsset, MeshAsset, MeshInstance, NormalTextureAsset, Primitive,
    SceneAsset, SceneAssets, Skeleton, TextureAsset, Transform,
};
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ExtendedColorType, ImageEncoder};
use serde_json::Value;
use std::path::Path;
use std::process::{Command, Output};

const CONVERSION_SCHEMA: &str =
    include_str!("../../../docs/schemas/conversion-evidence-v2.schema.json");

fn fixture() -> Document {
    Document {
        skeleton: Skeleton {
            bones: vec![Bone {
                name: "root".into(),
                parent: None,
                rest: Transform::IDENTITY,
                inverse_bind: None,
            }],
        },
        assets: SceneAssets {
            meshes: vec![MeshAsset {
                name: "triangle".into(),
                source_mesh_index: 0,
                primitives: vec![Primitive {
                    material: Some(0),
                    indices: vec![0, 1, 2],
                    positions: vec![
                        Vec3::ZERO,
                        Vec3::new(1.0, 0.0, 0.0),
                        Vec3::new(0.0, 1.0, 0.0),
                    ],
                    normals: vec![Vec3::Z; 3],
                    uvs: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
                    ..Primitive::default()
                }],
            }],
            instances: vec![MeshInstance {
                source_node_index: 0,
                node: 0,
                mesh: 0,
                ..MeshInstance::default()
            }],
            materials: vec![MaterialAsset {
                name: "painted".into(),
                base_color: [0.2, 0.3, 0.4, 0.5],
                metallic: 0.1,
                roughness: 0.8,
                base_color_texture: None,
                normal_texture: None,
            }],
            scenes: vec![SceneAsset {
                source_scene_index: 0,
                name: Some("scene".into()),
                roots: vec![0],
            }],
            default_scene: Some(0),
        },
        ..Document::default()
    }
}

fn png_rgba(width: u32, height: u32, pixels: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    PngEncoder::new_with_quality(&mut bytes, CompressionType::Best, FilterType::NoFilter)
        .write_image(pixels, width, height, ExtendedColorType::Rgba8)
        .expect("encodes PNG fixture");
    bytes
}

fn jpeg_rgb(width: u32, height: u32, pixels: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    JpegEncoder::new_with_quality(&mut bytes, 100)
        .write_image(pixels, width, height, ExtendedColorType::Rgb8)
        .expect("encodes JPEG fixture");
    bytes
}

fn run_convert(dir: &Path, recipe: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .current_dir(dir)
        .args([
            "convert",
            "input.glb",
            "-o",
            "output.glb",
            "--material-texture-recipe",
            recipe,
            "--format",
            "json",
        ])
        .output()
        .expect("runs animsmith convert")
}

fn write_success_fixture(dir: &Path) {
    animsmith_gltf::write::write(&fixture(), &dir.join("input.glb")).expect("writes input GLB");
    std::fs::create_dir_all(dir.join("recipes/textures")).expect("creates texture root");
    std::fs::write(
        dir.join("recipes/textures/base.png"),
        png_rgba(2, 1, &[0, 0, 0, 255, 255, 255, 255, 255]),
    )
    .expect("writes base-color fixture");
    std::fs::write(
        dir.join("recipes/textures/normal.jpg"),
        jpeg_rgb(1, 1, &[128, 128, 255]),
    )
    .expect("writes normal fixture");
    std::fs::write(
        dir.join("recipes/materials.toml"),
        concat!(
            "schema_version = 1\n",
            "schema = \"urn:animsmith:schema:material-texture-recipe:1\"\n",
            "texture_root = \"textures\"\n",
            "max_dimension = 1\n",
            "\n",
            "[[materials]]\n",
            "name = \"painted\"\n",
            "base_color = \"base.png\"\n",
            "normal = \"normal.jpg\"\n",
        ),
    )
    .expect("writes recipe");
}

#[test]
fn recipe_conversion_is_schema_valid_byte_stable_and_semantically_ordered() {
    let dir = tempfile::tempdir().expect("creates temp directory");
    write_success_fixture(dir.path());

    let first = run_convert(dir.path(), "recipes/materials.toml");
    assert!(
        first.status.success(),
        "convert failed: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    let first_artifact = std::fs::read(dir.path().join("output.glb")).expect("reads output");
    let evidence: Value = serde_json::from_slice(&first.stdout).expect("stdout is JSON");
    let schema: Value = serde_json::from_str(CONVERSION_SCHEMA).expect("schema is JSON");
    let validator = jsonschema::validator_for(&schema).expect("schema compiles");
    assert!(
        validator.is_valid(&evidence),
        "evidence does not satisfy conversion schema: {evidence:#}"
    );
    assert_eq!(evidence["schema_version"], 2);
    assert_eq!(
        evidence["schema"],
        "urn:animsmith:schema:conversion-evidence:2"
    );
    assert_eq!(
        evidence["options"]["material_texture_recipe"],
        "recipes/materials.toml"
    );
    assert_eq!(
        evidence["material_texture_recipe"]["path"],
        "recipes/materials.toml"
    );
    assert_eq!(
        evidence["material_texture_recipe"]["texture_root"],
        "textures"
    );
    assert_eq!(
        evidence["material_texture_recipe"]["processor"]["image_crate"],
        "image@0.25.10"
    );
    assert_eq!(
        evidence["material_texture_recipe"]["processor"]["png_crate"],
        "png@0.18.1"
    );
    assert_eq!(
        evidence["material_texture_recipe"]["processor"]["jpeg_crate"],
        "zune-jpeg@0.5.15"
    );
    let consumed = evidence["material_texture_recipe"]["consumed_inputs"]
        .as_array()
        .expect("consumed input array");
    let emitted = evidence["material_texture_recipe"]["emitted_textures"]
        .as_array()
        .expect("emitted texture array");
    assert_eq!(consumed.len(), 2);
    assert_eq!(emitted.len(), 2);
    assert_eq!(consumed[0]["slot"], "base_color");
    assert_eq!(consumed[1]["slot"], "normal");
    assert_eq!(consumed[0]["dimensions"], serde_json::json!([2, 1]));
    assert_eq!(consumed[1]["dimensions"], serde_json::json!([1, 1]));
    assert_eq!(emitted[0]["mime"], "image/png");
    assert_eq!(emitted[0]["dimensions"], serde_json::json!([1, 1]));
    assert_eq!(emitted[0]["resized"], true);
    assert_eq!(emitted[1]["mime"], "image/jpeg");
    assert_eq!(emitted[1]["resized"], false);

    let loaded = animsmith_gltf::load(&dir.path().join("output.glb")).expect("loads output");
    let material = &loaded.assets.materials[0];
    assert_eq!(material.base_color, [1.0; 4]);
    assert_eq!(
        material
            .base_color_texture
            .as_ref()
            .expect("base color")
            .mime,
        "image/png"
    );
    let normal = material.normal_texture.as_ref().expect("normal texture");
    assert_eq!(normal.texture.mime, "image/jpeg");
    assert_eq!(normal.scale, 1.0);

    let second = run_convert(dir.path(), "recipes/materials.toml");
    assert!(second.status.success());
    assert_eq!(second.stdout, first.stdout, "evidence is deterministic");
    assert_eq!(
        std::fs::read(dir.path().join("output.glb")).expect("reads repeated output"),
        first_artifact,
        "artifact bytes are deterministic"
    );
}

#[test]
fn recipe_mapping_failure_is_stderr_only_and_leaves_no_output() {
    let dir = tempfile::tempdir().expect("creates temp directory");
    write_success_fixture(dir.path());
    std::fs::write(
        dir.path().join("recipes/materials.toml"),
        concat!(
            "schema_version = 1\n",
            "schema = \"urn:animsmith:schema:material-texture-recipe:1\"\n",
            "max_dimension = 1\n",
            "[[materials]]\n",
            "name = \"unknown\"\n",
            "base_color = \"textures/base.png\"\n",
            "normal = \"textures/normal.jpg\"\n",
        ),
    )
    .expect("rewrites invalid recipe");

    let output = run_convert(dir.path(), "recipes/materials.toml");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("matches no source material"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!dir.path().join("output.glb").exists());
}

#[test]
fn conversion_without_recipe_preserves_ordinary_embedded_textures() {
    let dir = tempfile::tempdir().expect("creates temp directory");
    let base_color = png_rgba(1, 1, &[12, 34, 56, 78]);
    let normal = jpeg_rgb(1, 1, &[128, 128, 255]);
    let mut source = fixture();
    source.assets.materials[0].base_color_texture = Some(TextureAsset {
        bytes: base_color.clone(),
        mime: "image/png".into(),
    });
    source.assets.materials[0].normal_texture = Some(NormalTextureAsset {
        texture: TextureAsset {
            bytes: normal.clone(),
            mime: "image/jpeg".into(),
        },
        scale: 0.65,
    });
    animsmith_gltf::write::write(&source, &dir.path().join("input.glb"))
        .expect("writes linked-texture input");

    let output = Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .current_dir(dir.path())
        .args(["convert", "input.glb", "-o", "ordinary.glb"])
        .output()
        .expect("runs ordinary conversion");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let loaded = animsmith_gltf::load(&dir.path().join("ordinary.glb")).expect("loads output");
    let material = &loaded.assets.materials[0];
    let loaded_base = material.base_color_texture.as_ref().expect("base color");
    assert_eq!(loaded_base.bytes, base_color);
    assert_eq!(loaded_base.mime, "image/png");
    let loaded_normal = material.normal_texture.as_ref().expect("normal");
    assert_eq!(loaded_normal.texture.bytes, normal);
    assert_eq!(loaded_normal.texture.mime, "image/jpeg");
    assert_eq!(loaded_normal.scale, 0.65);
}
