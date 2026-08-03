//! End-to-end CLI contract for declarative material texture recipes.

#![cfg(feature = "fbx")]

use animsmith_core::glam::Vec3;
use animsmith_core::model::{
    Bone, Document, MaterialAsset, MeshAsset, MeshInstance, NormalTextureAsset, Primitive,
    SceneAsset, SceneAssets, Skeleton, SourceSkeletonAssets, TextureAsset, Transform,
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
                metallic_roughness_texture: None,
                occlusion_texture: None,
            }],
            material_resources: Default::default(),
            scenes: vec![SceneAsset {
                source_scene_index: 0,
                name: Some("scene".into()),
                roots: vec![0],
            }],
            default_scene: Some(0),
            source_skeleton: SourceSkeletonAssets::default(),
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
        dir.join("recipes/textures/normal.png"),
        png_rgba(2, 1, &[255, 128, 128, 255, 128, 255, 128, 255]),
    )
    .expect("writes normal fixture");
    std::fs::write(
        dir.join("recipes/textures/metallic-roughness.png"),
        png_rgba(2, 1, &[0, 128, 64, 255, 0, 64, 192, 255]),
    )
    .expect("writes metallic-roughness fixture");
    std::fs::write(
        dir.join("recipes/textures/occlusion.png"),
        png_rgba(2, 1, &[32, 0, 0, 255, 224, 0, 0, 255]),
    )
    .expect("writes occlusion fixture");
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
            "normal = \"normal.png\"\n",
            "metallic_roughness = \"metallic-roughness.png\"\n",
            "occlusion = \"occlusion.png\"\n",
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
    assert_eq!(consumed.len(), 4);
    assert_eq!(emitted.len(), 4);
    assert_eq!(consumed[0]["slot"], "base_color");
    assert_eq!(consumed[1]["slot"], "normal");
    assert_eq!(consumed[2]["slot"], "metallic_roughness");
    assert_eq!(consumed[3]["slot"], "occlusion");
    assert_eq!(consumed[0]["dimensions"], serde_json::json!([2, 1]));
    assert_eq!(consumed[1]["dimensions"], serde_json::json!([2, 1]));
    assert_eq!(emitted[0]["mime"], "image/png");
    assert_eq!(emitted[0]["dimensions"], serde_json::json!([1, 1]));
    assert_eq!(emitted[0]["resized"], true);
    assert_eq!(emitted[1]["mime"], "image/png");
    assert_eq!(emitted[1]["dimensions"], serde_json::json!([1, 1]));
    assert_eq!(emitted[1]["resized"], true);
    assert_eq!(emitted[2]["slot"], "metallic_roughness");
    assert_eq!(emitted[3]["slot"], "occlusion");

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
    assert_eq!(normal.texture.mime, "image/png");
    assert_eq!(normal.scale, 1.0);
    let metallic_roughness = material
        .metallic_roughness_texture
        .as_ref()
        .expect("metallic-roughness texture");
    let occlusion = material
        .occlusion_texture
        .as_ref()
        .expect("occlusion texture");
    assert_eq!(occlusion.strength, 1.0);
    let base_pixel = image::load_from_memory(&material.base_color_texture.as_ref().unwrap().bytes)
        .expect("decodes emitted base color")
        .into_rgba8()
        .get_pixel(0, 0)
        .0;
    assert!(
        (187..=189).contains(&base_pixel[0]),
        "linear-light base-color midpoint: {base_pixel:?}"
    );
    assert_eq!(base_pixel[0], base_pixel[1]);
    assert_eq!(base_pixel[1], base_pixel[2]);
    assert_eq!(base_pixel[3], 255);
    let normal_pixel = image::load_from_memory(&normal.texture.bytes)
        .expect("decodes emitted normal")
        .into_rgba8()
        .get_pixel(0, 0)
        .0;
    let vector = [normal_pixel[0], normal_pixel[1], normal_pixel[2]]
        .map(|component| f32::from(component) / 255.0 * 2.0 - 1.0);
    let length = vector
        .iter()
        .map(|component| component * component)
        .sum::<f32>()
        .sqrt();
    assert!(
        (length - 1.0).abs() < 0.01,
        "normal was not renormalized: {normal_pixel:?}"
    );
    assert!(
        vector[0] > 0.69 && vector[1] > 0.69 && vector[2].abs() < 0.01,
        "normal semantics or filtering changed: {normal_pixel:?}"
    );
    let metallic_roughness_pixel = image::load_from_memory(&metallic_roughness.bytes)
        .expect("decodes emitted metallic-roughness")
        .into_rgba8()
        .get_pixel(0, 0)
        .0;
    assert_eq!(
        metallic_roughness_pixel,
        [0, 96, 128, 255],
        "linear data slots retain their channel meanings"
    );
    let occlusion_pixel = image::load_from_memory(&occlusion.texture.bytes)
        .expect("decodes emitted occlusion")
        .into_rgba8()
        .get_pixel(0, 0)
        .0;
    assert_eq!(
        occlusion_pixel,
        [128, 0, 0, 255],
        "occlusion remains in the red channel"
    );

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
fn inspected_control_bearing_material_name_copies_directly_into_recipe() {
    let dir = tempfile::tempdir().expect("creates temp directory");
    write_success_fixture(dir.path());
    let material_name = "paint\u{1b}\u{202e}ed";
    let mut source = fixture();
    source.assets.materials[0].name = material_name.into();
    animsmith_gltf::write::write(&source, &dir.path().join("input.glb"))
        .expect("writes control-bearing material name");

    let inspected = Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .current_dir(dir.path())
        .args(["inspect", "input.glb"])
        .output()
        .expect("inspects material name");
    assert!(
        inspected.status.success(),
        "inspect failed: {}",
        String::from_utf8_lossy(&inspected.stderr)
    );
    let inspect_text = String::from_utf8(inspected.stdout).expect("inspect output is UTF-8");
    let discovered_selector = inspect_text
        .lines()
        .find_map(|line| line.strip_prefix("  #0 "))
        .expect("inspect exposes first material selector");
    assert_eq!(discovered_selector, "\"paint\\u001B\\u202Eed\"");

    let recipe_path = dir.path().join("recipes/materials.toml");
    let recipe = std::fs::read_to_string(&recipe_path)
        .expect("reads recipe")
        .replacen(
            "name = \"painted\"",
            &format!("name = {discovered_selector}"),
            1,
        );
    std::fs::write(&recipe_path, recipe).expect("writes copied material selector");

    let output = run_convert(dir.path(), "recipes/materials.toml");
    assert!(
        output.status.success(),
        "convert failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let converted =
        animsmith_gltf::load(&dir.path().join("output.glb")).expect("loads converted output");
    assert_eq!(converted.assets.materials[0].name, material_name);
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
