//! End-to-end CLI contract for canonical static mesh transform baking.

#![cfg(feature = "fbx")]

use animsmith_core::glam::{Mat3, Quat, Vec3};
use animsmith_core::model::{
    Bone, Clip, Document, Interpolation, MaterialAsset, MeshAsset, MeshInstance,
    NormalTextureAsset, Primitive, Property, SceneAsset, SceneAssets, Skeleton,
    SourceSkeletonAssets, TextureAsset, Track, TrackValues, Transform,
};
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ExtendedColorType, ImageEncoder};
use serde_json::Value;
use std::path::Path;
use std::process::{Command, Output, Stdio};

const CONVERSION_SCHEMA: &str =
    include_str!("../../../docs/schemas/conversion-evidence-v2.schema.json");
const EPSILON: f32 = 1.0e-5;
const TINY_JPEG: &[u8] = &[
    0xff, 0xd8, 0xff, 0xe0, 0x00, 0x10, b'J', b'F', b'I', b'F', 0xff, 0xd9,
];

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
                    material: Some(0),
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
            materials: vec![MaterialAsset {
                name: "painted".into(),
                base_color: [0.3, 0.4, 0.5, 0.6],
                metallic: 0.7,
                roughness: 0.2,
                base_color_texture: Some(TextureAsset {
                    bytes: TINY_JPEG.to_vec(),
                    mime: "image/jpeg".into(),
                }),
                normal_texture: Some(NormalTextureAsset {
                    texture: TextureAsset {
                        bytes: TINY_JPEG.to_vec(),
                        mime: "image/jpeg".into(),
                    },
                    scale: 0.65,
                }),
                metallic_roughness_texture: None,
                occlusion_texture: None,
            }],
            material_resources: Default::default(),
            scenes: vec![SceneAsset {
                source_scene_index: 0,
                name: Some("authored-scene".into()),
                roots: vec![0],
            }],
            default_scene: Some(0),
            source_skeleton: SourceSkeletonAssets::default(),
        },
        ..Document::default()
    }
}

fn run_json_convert(input: &Path, output: &Path, bake: bool) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_animsmith"));
    command
        .arg("convert")
        .arg(input)
        .arg("-o")
        .arg(output)
        .arg("--format")
        .arg("json");
    if bake {
        command.arg("--bake-static-mesh-transforms");
    }
    command.output().expect("runs animsmith convert")
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

    let first = run_json_convert(&input, &output_a, true);
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
    let mut missing_bake_payload = evidence.clone();
    missing_bake_payload
        .as_object_mut()
        .unwrap()
        .remove("static_mesh_bake");
    assert!(
        !validator.is_valid(&missing_bake_payload),
        "the schema requires bake evidence when the bake option is true"
    );
    let mut unexpected_bake_payload = evidence.clone();
    unexpected_bake_payload["options"]["bake_static_mesh_transforms"] = false.into();
    assert!(
        !validator.is_valid(&unexpected_bake_payload),
        "the schema rejects bake evidence when the bake option is false"
    );
    let mut conflicting_options = evidence.clone();
    conflicting_options["options"]["animation_only"] = true.into();
    assert!(
        !validator.is_valid(&conflicting_options),
        "the schema rejects mutually exclusive conversion options"
    );
    assert_eq!(evidence["schema_version"], 2);
    assert_eq!(
        evidence["schema"],
        "urn:animsmith:schema:conversion-evidence:2"
    );
    assert_eq!(evidence["artifact"]["meshes"], 1);
    assert_eq!(evidence["artifact"]["primitive_positions"], 3);
    assert_eq!(evidence["artifact"]["nodes"], 2);
    assert_eq!(evidence["options"]["animation_only"], false);
    assert_eq!(evidence["options"]["bake_static_mesh_transforms"], true);
    assert_eq!(evidence["options"]["material_texture_recipe"], Value::Null);
    let entries = evidence["static_mesh_bake"]["entries"]
        .as_array()
        .expect("bake entries array");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["source_node_index"], instance.source_node_index);
    assert_eq!(
        entries[0]["source_node_name"],
        loaded_input.skeleton.bones[instance.node].name
    );
    assert_eq!(entries[0]["source_mesh_ordinal"], instance.mesh);
    assert_eq!(
        entries[0]["source_mesh_index"],
        loaded_input.assets.meshes[instance.mesh].source_mesh_index
    );
    assert_eq!(
        entries[0]["source_mesh_name"],
        loaded_input.assets.meshes[instance.mesh].name
    );
    assert_eq!(entries[0]["output_node_index"], 1);
    assert_eq!(entries[0]["output_mesh_index"], 0);
    assert_eq!(entries[0]["primitive_count"], 1);
    assert_eq!(entries[0]["position_count"], 3);
    assert_eq!(entries[0]["normal_count"], 3);
    let evidence_world: [f32; 16] =
        serde_json::from_value(entries[0]["world_transform"].clone()).unwrap();
    assert_eq!(evidence_world, world.to_cols_array());
    let evidence_determinant: f32 =
        serde_json::from_value(entries[0]["linear_determinant"].clone()).unwrap();
    assert_eq!(evidence_determinant, Mat3::from_mat4(world).determinant());

    let second = run_json_convert(&input, &output_b, true);
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
    assert_eq!(primitive.material, source_primitive.material);
    let baked_normal_texture = baked.assets.materials[0]
        .normal_texture
        .as_ref()
        .expect("static bake keeps embedded normal texture");
    let source_normal_texture = loaded_input.assets.materials[0]
        .normal_texture
        .as_ref()
        .unwrap();
    assert_eq!(
        baked_normal_texture.texture.bytes,
        source_normal_texture.texture.bytes
    );
    assert_eq!(
        baked_normal_texture.texture.mime,
        source_normal_texture.texture.mime
    );
    assert_eq!(baked_normal_texture.scale, source_normal_texture.scale);
    assert_eq!(primitive.positions.len(), expected_positions.len());
    assert_eq!(primitive.normals.len(), expected_normals.len());
    for (actual, expected) in primitive.positions.iter().zip(expected_positions) {
        assert_vec3_close(*actual, expected);
    }
    for (actual, expected) in primitive.normals.iter().zip(expected_normals) {
        assert_vec3_close(*actual, expected);
    }

    let gltf = gltf::Gltf::from_slice(&std::fs::read(&output_a).unwrap()).expect("valid GLB");
    let scene = gltf.default_scene().expect("default baked scene");
    assert_eq!(scene.nodes().count(), 1, "one canonical scene root");
    let root = scene.nodes().next().expect("canonical scene root");
    assert!(root.mesh().is_none(), "the canonical root is only a root");
    let children = root.children().collect::<Vec<_>>();
    assert_eq!(children.len(), 1, "one identity child per baked mesh");
    assert!(
        children[0].mesh().is_some(),
        "the identity child owns the mesh"
    );
    assert_eq!(children[0].children().count(), 0);
    assert!(gltf.nodes().all(|node| {
        let (translation, rotation, scale) = node.transform().decomposed();
        translation == [0.0, 0.0, 0.0]
            && rotation == [0.0, 0.0, 0.0, 1.0]
            && scale == [1.0, 1.0, 1.0]
    }));
}

#[test]
fn option_bearing_text_conversion_diagnoses_closed_stdout_once_after_publication() {
    let dir = tempfile::tempdir().expect("creates temp directory");
    let input = dir.path().join("input.glb");
    let output = dir.path().join("baked.glb");
    let recipe = dir.path().join("materials.toml");
    animsmith_gltf::write::write(&fixture(), &input).expect("writes input fixture");
    let source = animsmith_gltf::load(&input).expect("reloads source fixture");
    let source_instance = &source.assets.instances[0];
    let source_primitive = &source.assets.meshes[source_instance.mesh].primitives[0];
    let source_world = source.skeleton.bones[0].rest.to_mat4()
        * source.skeleton.bones[source_instance.node].rest.to_mat4();
    let expected_positions = source_primitive
        .positions
        .iter()
        .map(|position| source_world.transform_point3(*position))
        .collect::<Vec<_>>();
    for (name, pixel) in [
        ("base.png", [32, 64, 96, 255]),
        ("normal.png", [128, 128, 255, 255]),
    ] {
        let mut bytes = Vec::new();
        PngEncoder::new_with_quality(&mut bytes, CompressionType::Best, FilterType::NoFilter)
            .write_image(&pixel, 1, 1, ExtendedColorType::Rgba8)
            .expect("encodes recipe texture");
        std::fs::write(dir.path().join(name), bytes).expect("writes recipe texture");
    }
    std::fs::write(
        &recipe,
        concat!(
            "schema_version = 1\n",
            "schema = \"urn:animsmith:schema:material-texture-recipe:1\"\n",
            "max_dimension = 1\n",
            "\n",
            "[[materials]]\n",
            "name = \"painted\"\n",
            "base_color = \"base.png\"\n",
            "normal = \"normal.png\"\n",
        ),
    )
    .expect("writes material recipe");

    let (reader, writer) = std::io::pipe().expect("creates a pipe");
    drop(reader);
    let result = Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .arg("convert")
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .arg("--bake-static-mesh-transforms")
        .arg("--material-texture-recipe")
        .arg(&recipe)
        .stdout(Stdio::from(writer))
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawns option-bearing conversion")
        .wait_with_output()
        .expect("waits for option-bearing conversion");

    let stderr = String::from_utf8_lossy(&result.stderr);
    assert_eq!(
        result.status.code(),
        Some(0),
        "published conversion remains successful; stderr:\n{stderr}"
    );
    assert_eq!(
        stderr
            .matches("animsmith: cannot write text output to stdout")
            .count(),
        1,
        "one conversion transcript must produce one diagnostic:\n{stderr}"
    );
    assert!(!stderr.contains("panicked at"), "stderr:\n{stderr}");

    let published = animsmith_gltf::load(&output).expect("published artifact loads");
    assert_eq!(published.assets.instances.len(), 1);
    assert!(
        published
            .skeleton
            .bones
            .iter()
            .all(|bone| bone.rest == Transform::IDENTITY),
        "the option-bearing conversion was published before reporting"
    );
    let material = &published.assets.materials[0];
    let base_color = material
        .base_color_texture
        .as_ref()
        .expect("recipe emits a base-color texture");
    assert_eq!(base_color.mime, "image/png");
    assert_ne!(base_color.bytes, TINY_JPEG);
    assert_eq!(
        image::load_from_memory(&base_color.bytes)
            .expect("decodes emitted base-color texture")
            .to_rgba8()
            .into_raw(),
        vec![32, 64, 96, 255],
        "published bytes contain the recipe's base-color pixel"
    );
    assert_eq!(material.base_color, [1.0, 1.0, 1.0, 1.0]);

    let normal = material
        .normal_texture
        .as_ref()
        .expect("recipe emits a normal texture");
    assert_eq!(normal.texture.mime, "image/png");
    assert_ne!(normal.texture.bytes, TINY_JPEG);
    assert_eq!(
        image::load_from_memory(&normal.texture.bytes)
            .expect("decodes emitted normal texture")
            .to_rgba8()
            .into_raw(),
        vec![128, 128, 255, 255],
        "published bytes contain the recipe's normal pixel"
    );
    assert_eq!(normal.scale, 1.0);

    let baked_primitive =
        &published.assets.meshes[published.assets.instances[0].mesh].primitives[0];
    assert_ne!(
        baked_primitive.positions, source_primitive.positions,
        "the fixture must distinguish a skipped static bake"
    );
    assert_eq!(baked_primitive.positions.len(), expected_positions.len());
    for (actual, expected) in baked_primitive.positions.iter().zip(expected_positions) {
        assert_vec3_close(*actual, expected);
    }

    let writable_output = dir.path().join("writable.glb");
    let writable = Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .arg("convert")
        .arg(&input)
        .arg("-o")
        .arg(&writable_output)
        .arg("--bake-static-mesh-transforms")
        .arg("--material-texture-recipe")
        .arg(&recipe)
        .output()
        .expect("runs writable option-bearing conversion");
    assert!(
        writable.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&writable.stderr)
    );
    assert!(writable.stderr.is_empty());
    assert_eq!(
        String::from_utf8(writable.stdout).expect("conversion summary is UTF-8"),
        format!(
            "wrote {} (2 node(s), 0 clip(s), 1 mesh(es) / 3 position(s), 1 material(s))\n\
             baked 1 static mesh instance(s) into identity-root geometry\n\
             applied material texture recipe; emitted 2 texture(s)\n",
            writable_output.display()
        ),
        "the real conversion dispatch must retain and order both optional summaries"
    );
    animsmith_gltf::load(&writable_output).expect("writable conversion artifact loads");
}

#[test]
fn convert_json_without_static_bake_omits_bake_evidence_and_keeps_source_transforms() {
    let dir = tempfile::tempdir().expect("creates temp directory");
    let input = dir.path().join("input.glb");
    let output = dir.path().join("ordinary.glb");
    animsmith_gltf::write::write(&fixture(), &input).expect("writes input fixture");
    let source = animsmith_gltf::load(&input).expect("loads input fixture");

    let result = run_json_convert(&input, &output, false);
    assert!(
        result.status.success(),
        "convert failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let evidence: Value = serde_json::from_slice(&result.stdout).expect("stdout is JSON evidence");
    let schema: Value = serde_json::from_str(CONVERSION_SCHEMA).expect("schema is JSON");
    assert!(
        jsonschema::validator_for(&schema)
            .expect("schema compiles")
            .is_valid(&evidence)
    );
    assert_eq!(evidence["options"]["animation_only"], false);
    assert_eq!(evidence["options"]["bake_static_mesh_transforms"], false);
    assert!(
        evidence.get("static_mesh_bake").is_none(),
        "ordinary conversion must not claim static-bake evidence"
    );

    let ordinary = animsmith_gltf::load(&output).expect("loads ordinary conversion");
    assert_eq!(ordinary.skeleton.bones.len(), source.skeleton.bones.len());
    for (actual, expected) in ordinary.skeleton.bones.iter().zip(&source.skeleton.bones) {
        assert_eq!(actual.parent, expected.parent);
        assert_eq!(actual.rest, expected.rest);
    }
    assert_eq!(
        ordinary.assets.meshes[0].primitives[0].positions,
        source.assets.meshes[0].primitives[0].positions,
        "default conversion keeps mesh-local geometry unchanged"
    );
    let ordinary_primitive = &ordinary.assets.meshes[0].primitives[0];
    let source_primitive = &source.assets.meshes[0].primitives[0];
    assert_eq!(ordinary_primitive.indices, source_primitive.indices);
    assert_eq!(ordinary_primitive.normals, source_primitive.normals);
    assert_eq!(ordinary_primitive.uvs, source_primitive.uvs);
    assert_eq!(ordinary_primitive.material, source_primitive.material);
    assert_eq!(
        ordinary.assets.materials.len(),
        source.assets.materials.len()
    );
    let ordinary_material = &ordinary.assets.materials[0];
    let source_material = &source.assets.materials[0];
    assert_eq!(ordinary_material.name, source_material.name);
    assert_eq!(ordinary_material.base_color, source_material.base_color);
    assert_eq!(ordinary_material.metallic, source_material.metallic);
    assert_eq!(ordinary_material.roughness, source_material.roughness);
    let ordinary_texture = ordinary_material
        .base_color_texture
        .as_ref()
        .expect("ordinary conversion keeps embedded texture");
    let source_texture = source_material.base_color_texture.as_ref().unwrap();
    assert_eq!(ordinary_texture.bytes, source_texture.bytes);
    assert_eq!(ordinary_texture.mime, source_texture.mime);
    let ordinary_normal_texture = ordinary_material
        .normal_texture
        .as_ref()
        .expect("ordinary conversion keeps embedded normal texture");
    let source_normal_texture = source_material.normal_texture.as_ref().unwrap();
    assert_eq!(
        ordinary_normal_texture.texture.bytes,
        source_normal_texture.texture.bytes
    );
    assert_eq!(
        ordinary_normal_texture.texture.mime,
        source_normal_texture.texture.mime
    );
    assert_eq!(ordinary_normal_texture.scale, source_normal_texture.scale);
}

#[test]
fn convert_static_bake_rejects_invalid_inputs_before_creating_output() {
    let dir = tempfile::tempdir().expect("creates temp directory");
    let mut shared = fixture();
    shared.assets.instances.push(MeshInstance {
        source_node_index: 24,
        node: 0,
        mesh: 0,
        ..MeshInstance::default()
    });

    let mut singular = fixture();
    singular.skeleton.bones[1].rest.scale.x = 0.0;

    let mut animated = fixture();
    animated.clips.push(Clip {
        name: "moving".into(),
        duration_s: 1.0,
        tracks: vec![Track {
            bone: 1,
            property: Property::Rotation,
            interpolation: Interpolation::Linear,
            times: vec![0.0, 1.0],
            values: TrackValues::Quats(vec![Quat::IDENTITY, Quat::from_rotation_z(0.5)]),
        }],
    });

    let mut overflowing = fixture();
    overflowing.assets.meshes[0].primitives[0].positions[0] = Vec3::splat(f32::MAX / 2.0);

    for (case, input_document) in [
        ("shared", shared),
        ("singular", singular),
        ("animated", animated),
        ("overflowing", overflowing),
    ] {
        let input = dir.path().join(format!("{case}.glb"));
        let output = dir.path().join(format!("{case}-must-not-exist.glb"));
        animsmith_gltf::write::write(&input_document, &input)
            .unwrap_or_else(|error| panic!("writes {case} input fixture: {error}"));

        let result = run_json_convert(&input, &output, true);
        assert_eq!(
            result.status.code(),
            Some(2),
            "{case} unexpectedly succeeded: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(
            result.stdout.is_empty(),
            "{case} operator failure emits no JSON evidence"
        );
        assert!(
            !output.exists(),
            "{case} validation fails before output creation"
        );
    }
}
