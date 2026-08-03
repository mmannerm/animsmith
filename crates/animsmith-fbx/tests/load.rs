use animsmith_core::glam::{Mat4, Vec3};
use animsmith_core::measure::measure_assets;
use animsmith_core::model::Property;
use animsmith_core::{Document, TrackValues};
use std::path::PathBuf;

/// A valid 1x1 PNG used as an externally referenced FBX normal map.
const TINY_PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0xF8, 0xFF, 0xFF, 0x3F,
    0x00, 0x05, 0xFE, 0x02, 0xFE, 0xA7, 0x35, 0x81, 0x84, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E,
    0x44, 0xAE, 0x42, 0x60, 0x82,
];

const TINY_PNG_B64: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4//8/AAX+Av6nNYGEAAAAAElFTkSuQmCC";

/// A valid, self-authored 1x1 JPEG used to prove opaque byte pass-through.
const TINY_JPEG: &[u8] = &[
    0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01, 0x00, 0x00, 0x01,
    0x00, 0x01, 0x00, 0x00, 0xFF, 0xDB, 0x00, 0x43, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xDB, 0x00, 0x43, 0x01, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xC0,
    0x00, 0x11, 0x08, 0x00, 0x01, 0x00, 0x01, 0x03, 0x01, 0x22, 0x00, 0x02, 0x11, 0x01, 0x03, 0x11,
    0x01, 0xFF, 0xC4, 0x00, 0x15, 0x00, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, 0xFF, 0xC4, 0x00, 0x14, 0x10, 0x01, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xC4,
    0x00, 0x14, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0xFF, 0xC4, 0x00, 0x14, 0x11, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xDA, 0x00, 0x0C, 0x03, 0x01,
    0x00, 0x02, 0x11, 0x03, 0x11, 0x00, 0x3F, 0x00, 0xA0, 0x00, 0xFF, 0xD9,
];

#[derive(Clone, Copy)]
enum NormalImage {
    Linked(&'static [u8]),
    Embedded,
    MissingWithParentDecoy,
    Unreadable,
}

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/rigged_triangle.fbx")
}

fn assert_same_loaded_shape(left: &Document, right: &Document) {
    assert_eq!(left.source.path, right.source.path);
    assert_eq!(left.source.format, right.source.format);
    assert_eq!(left.skeleton.bones.len(), right.skeleton.bones.len());
    for (left, right) in left.skeleton.bones.iter().zip(&right.skeleton.bones) {
        assert_eq!(left.name, right.name);
        assert_eq!(left.parent, right.parent);
        assert_eq!(left.rest, right.rest);
        assert_eq!(left.inverse_bind, right.inverse_bind);
    }
    assert_eq!(left.clips.len(), right.clips.len());
    for (left, right) in left.clips.iter().zip(&right.clips) {
        assert_eq!(left.name, right.name);
        assert_eq!(left.duration_s, right.duration_s);
        assert_eq!(left.tracks.len(), right.tracks.len());
        for (left, right) in left.tracks.iter().zip(&right.tracks) {
            assert_eq!(left.bone, right.bone);
            assert_eq!(left.property, right.property);
            assert_eq!(left.interpolation, right.interpolation);
            assert_eq!(left.times, right.times);
            match (&left.values, &right.values) {
                (TrackValues::Vec3s(left), TrackValues::Vec3s(right)) => {
                    assert_eq!(left, right);
                }
                (TrackValues::Quats(left), TrackValues::Quats(right)) => {
                    assert_eq!(left, right);
                }
                _ => panic!("track value kinds differ"),
            }
        }
    }
    assert_eq!(left.assets.meshes.len(), right.assets.meshes.len());
    assert_eq!(left.assets.instances.len(), right.assets.instances.len());
    assert_eq!(left.assets.materials.len(), right.assets.materials.len());
    assert_eq!(
        left.assets.material_resources.textures.len(),
        right.assets.material_resources.textures.len()
    );
    assert_eq!(left.assets.scenes.len(), right.assets.scenes.len());
    assert_eq!(left.assets.default_scene, right.assets.default_scene);
}

fn assert_vec3_near(got: Vec3, want: Vec3) {
    assert!(
        (got - want).length() < 1e-5,
        "expected {want:?}, got {got:?}"
    );
}

fn rest_models(doc: &animsmith_core::Document) -> Vec<Mat4> {
    let mut model = vec![Mat4::IDENTITY; doc.skeleton.bones.len()];
    for (index, bone) in doc.skeleton.bones.iter().enumerate() {
        let local = bone.rest.to_mat4();
        model[index] = bone.parent.map_or(local, |parent| model[parent] * local);
    }
    model
}

#[test]
fn loads_self_authored_rigged_triangle_fixture() {
    let doc = animsmith_fbx::load(&fixture()).expect("FBX fixture loads");

    assert_eq!(doc.source.format.as_deref(), Some("fbx"));
    let bones: Vec<&str> = doc.skeleton.bones.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(bones, vec!["<fbx-root>", "root", "tri"]);
    assert_eq!(doc.skeleton.bones[1].parent, Some(0));
    assert_eq!(doc.skeleton.bones[2].parent, Some(1));

    assert_eq!(doc.clips.len(), 1);
    let clip = &doc.clips[0];
    assert_eq!(clip.name, "take");
    assert!((clip.duration_s - 1.0).abs() < 1e-6);

    let translation = clip
        .tracks
        .iter()
        .find(|t| t.bone == 1 && t.property == Property::Translation)
        .expect("root translation track");
    assert_eq!(translation.key_count(), 31);
    assert_vec3_near(translation.key_vec3(0).unwrap(), Vec3::ZERO);
    assert_vec3_near(
        translation.key_vec3(translation.key_count() - 1).unwrap(),
        Vec3::new(1.0, 0.0, 0.0),
    );

    let mesh = doc.assets.meshes.first().expect("mesh loaded");
    let instance = doc.assets.instances.first().expect("mesh instance loaded");
    assert_eq!(mesh.name, "tri");
    assert_eq!(instance.node, 2);
    assert_eq!(mesh.source_mesh_index, 0);
    assert_eq!(instance.mesh, 0);
    assert_eq!(instance.skin_joints, vec![1]);
    assert_eq!(instance.skin_ibms.len(), 1);
    assert_eq!(doc.assets.default_scene, Some(0));
    assert_eq!(doc.assets.scenes.len(), 1);
    assert_eq!(doc.assets.scenes[0].source_scene_index, 0);
    assert!(
        !doc.assets.scenes[0].roots.is_empty(),
        "FBX scene has roots"
    );

    let prim = mesh.primitives.first().expect("primitive loaded");
    assert_eq!(prim.positions.len(), 3);
    assert_eq!(prim.indices, vec![0, 1, 2]);
    assert_eq!(prim.joints, vec![[0, 0, 0, 0]; 3]);
    assert_eq!(prim.weights, vec![[1.0, 0.0, 0.0, 0.0]; 3]);

    assert_eq!(doc.assets.materials.len(), 0);
}

#[test]
fn load_path_and_captured_bytes_are_equivalent() {
    let path = fixture();
    let bytes = std::fs::read(&path).expect("capture fixture");

    let from_path = animsmith_fbx::load(&path).expect("fixture loads by path");
    let from_bytes = animsmith_fbx::load_bytes(&path, &bytes).expect("fixture loads by bytes");

    assert_same_loaded_shape(&from_path, &from_bytes);
}

#[test]
fn captured_bytes_survive_primary_path_removal() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("captured.fbx");
    let bytes = std::fs::read(fixture()).expect("capture fixture");
    std::fs::write(&path, &bytes).expect("write primary input");
    std::fs::remove_file(&path).expect("remove primary input after capture");

    let doc = animsmith_fbx::load_bytes(&path, &bytes).expect("captured bytes still load");

    assert_eq!(
        doc.source.path.as_deref(),
        Some(path.to_str().expect("UTF-8 path"))
    );
    assert_eq!(doc.clips[0].name, "take");
}

fn write_normal_material(dir: &tempfile::TempDir, image: NormalImage) -> PathBuf {
    let source_dir = dir.path().join("scene");
    std::fs::create_dir(&source_dir).expect("create source directory");
    let source = std::fs::read_to_string(fixture())
        .expect("read fixture")
        .replace("\r\n", "\n");
    let source = source.replacen(
        "\tObjectType: \"Deformer\" { Count: 2 }\n}",
        "\tObjectType: \"Deformer\" { Count: 2 }\n\tObjectType: \"Material\" { Count: 1 }\n\tObjectType: \"Texture\" { Count: 1 }\n\tObjectType: \"Video\" { Count: 1 }\n}",
        1,
    );
    let content = if matches!(image, NormalImage::Embedded) {
        format!("\n\t\tContent: ,\"{TINY_PNG_B64}\"")
    } else {
        String::new()
    };
    let material_objects = format!(
        r#"	Material: 5001, "Material::normal_mat", "" {{
		Version: 102
		ShadingModel: "phong"
		MultiLayer: 0
	}}
	Texture: 5002, "Texture::normal", "" {{
		Type: "TextureVideoClip"
		Version: 202
		TextureName: "Texture::normal"
		Media: "Video::normal"
		FileName: "normal.png"
		RelativeFilename: "normal.png"
		ModelUVTranslation: 0,0
		ModelUVScaling: 1,1
		Texture_Alpha_Source: "None"
		Cropping: 0,0,0,0
	}}
	Video: 5003, "Video::normal", "Clip" {{
		Type: "Clip"
		Properties70: {{
			P: "Path", "KString", "XRefUrl", "", "normal.png"
		}}
		FileName: "normal.png"
		RelativeFilename: "normal.png"{content}
	}}
}}
Connections: {{"#
    );
    let source = source.replacen("}\nConnections: {", &material_objects, 1);
    let source = source.replacen(
        "Connections: {",
        "Connections: {\n\tC: \"OO\",5001,1002\n\tC: \"OP\",5002,5001,\"NormalMap\"\n\tC: \"OO\",5003,5002",
        1,
    );
    assert!(source.contains("Material::normal_mat"));
    assert!(source.contains("5002,5001,\"NormalMap\""));

    let path = source_dir.join("normal-material.fbx");
    std::fs::write(&path, source).expect("write FBX");
    match image {
        NormalImage::Linked(bytes) => {
            std::fs::write(source_dir.join("normal.png"), bytes).expect("write normal image");
        }
        NormalImage::Embedded => {}
        NormalImage::MissingWithParentDecoy => {
            std::fs::write(dir.path().join("normal.png"), TINY_PNG)
                .expect("write parent-directory decoy");
        }
        NormalImage::Unreadable => {
            std::fs::create_dir(source_dir.join("normal.png"))
                .expect("create unreadable image path");
        }
    }

    path
}

fn load_normal_material(image: NormalImage) -> animsmith_core::Document {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = write_normal_material(&dir, image);
    animsmith_fbx::load(&path).expect("normal-map FBX loads")
}

fn assert_normal_texture(doc: &animsmith_core::Document) {
    assert_eq!(doc.assets.materials.len(), 1);
    let normal = doc.assets.materials[0]
        .normal_texture
        .as_ref()
        .expect("normal map carried");
    assert_eq!(normal.texture.mime, "image/png");
    assert_eq!(normal.texture.bytes, TINY_PNG);
    assert_eq!(normal.scale, 1.0, "FBX uses glTF default normal scale");
    assert!(doc.assets.materials[0].base_color_texture.is_none());
}

#[test]
fn loads_linked_fbx_normal_texture() {
    assert_normal_texture(&load_normal_material(NormalImage::Linked(TINY_PNG)));
}

#[test]
fn byte_loader_resolves_external_texture_relative_to_supplied_path() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = write_normal_material(&dir, NormalImage::Linked(TINY_PNG));
    let bytes = std::fs::read(&path).expect("capture FBX");
    std::fs::remove_file(&path).expect("remove primary input after capture");

    let doc = animsmith_fbx::load_bytes(&path, &bytes).expect("captured FBX loads");

    assert_normal_texture(&doc);
}

#[test]
fn loads_embedded_fbx_normal_texture() {
    assert_normal_texture(&load_normal_material(NormalImage::Embedded));
}

#[test]
fn loads_linked_fbx_jpeg_normal_texture_without_rewriting_bytes() {
    let doc = load_normal_material(NormalImage::Linked(TINY_JPEG));
    let normal = doc.assets.materials[0]
        .normal_texture
        .as_ref()
        .expect("JPEG normal map carried");
    assert_eq!(normal.texture.mime, "image/jpeg");
    assert_eq!(normal.texture.bytes, TINY_JPEG);
}

#[test]
fn missing_unreadable_and_unsupported_fbx_normal_images_are_not_guessed_or_injected() {
    for (label, image) in [
        (
            "missing-with-parent-decoy",
            NormalImage::MissingWithParentDecoy,
        ),
        ("unreadable", NormalImage::Unreadable),
        ("unsupported", NormalImage::Linked(b"not an image")),
    ] {
        let doc = load_normal_material(image);
        assert!(
            doc.assets.materials[0].normal_texture.is_none(),
            "{label}: no normal texture should be invented"
        );
    }
}

#[test]
fn garbage_file_is_reported_as_fbx_parse_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("garbage.fbx");
    std::fs::write(&path, b"not an fbx file").expect("write garbage input");

    let err = animsmith_fbx::load(&path).expect_err("garbage input should not load");
    assert!(
        matches!(err, animsmith_fbx::LoadError::Fbx(_)),
        "expected LoadError::Fbx, got {err:?}"
    );
}

#[test]
fn normalizes_centimetre_z_up_scene_to_metre_y_up() {
    let source = std::fs::read_to_string(fixture()).expect("read self-authored fixture");
    let source = source.replacen(
        "P: \"UpAxis\", \"int\", \"Integer\", \"\",1",
        "P: \"UpAxis\", \"int\", \"Integer\", \"\",2",
        1,
    );
    let source = source.replacen(
        "P: \"FrontAxis\", \"int\", \"Integer\", \"\",2",
        "P: \"FrontAxis\", \"int\", \"Integer\", \"\",1",
        1,
    );
    let source = source.replacen(
        "P: \"FrontAxisSign\", \"int\", \"Integer\", \"\",1",
        "P: \"FrontAxisSign\", \"int\", \"Integer\", \"\",-1",
        1,
    );
    let source = source.replacen(
        "Vertices: *9 { a: 0,0,0,100,0,0,0,100,0 }",
        "Vertices: *9 { a: 0,0,0,100,0,0,0,100,100 }",
        1,
    );
    let source = source.replacen(
        "C: \"OP\",3004,3003,\"d|X\"",
        "C: \"OP\",3004,3003,\"d|Z\"",
        1,
    );
    assert!(source.contains("\"UpAxis\", \"int\", \"Integer\", \"\",2"));
    assert!(source.contains("\"FrontAxis\", \"int\", \"Integer\", \"\",1"));
    assert!(source.contains("\"FrontAxisSign\", \"int\", \"Integer\", \"\",-1"));
    assert!(source.contains("0,100,100"));
    assert!(source.contains("3004,3003,\"d|Z\""));

    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("centimetre-z-up.fbx");
    std::fs::write(&path, source).expect("write transformed fixture");
    let doc = animsmith_fbx::load(&path).expect("Z-up fixture loads");

    let mesh = doc.assets.meshes.first().expect("mesh loaded");
    let primitive = mesh.primitives.first().expect("primitive loaded");
    let instance = doc.assets.instances.first().expect("mesh instance loaded");
    let model = rest_models(&doc)[instance.node];
    let source_x = model.transform_point3(primitive.positions[1]);
    let source_yz = model.transform_point3(primitive.positions[2]);

    assert_vec3_near(source_x, Vec3::X);
    assert_vec3_near(source_yz, Vec3::Y - Vec3::Z);

    let translation = doc.clips[0]
        .tracks
        .iter()
        .find(|track| track.bone == 1 && track.property == Property::Translation)
        .expect("root translation track");
    assert_vec3_near(
        translation
            .key_vec3(translation.key_count() - 1)
            .expect("final translation key"),
        Vec3::Y,
    );

    // Geometry measurements describe the mesh definition's finite POSITION
    // stream. They deliberately do not bake this instance's converted world
    // transform into the result: the transform carries the centimetre-to-metre
    // and Z-up-to-Y-up conversion for this fixture.
    let finite_positions = primitive
        .positions
        .iter()
        .copied()
        .filter(|position| position.is_finite())
        .collect::<Vec<_>>();
    assert!(!finite_positions.is_empty(), "fixture has finite positions");
    let local_centroid =
        finite_positions.iter().copied().sum::<Vec3>() / finite_positions.len() as f32;
    let measured_centroid = measure_assets(&doc).mesh_definitions[0]
        .geometry_centroid
        .expect("finite FBX geometry has a centroid");
    assert_vec3_near(Vec3::from_array(measured_centroid), local_centroid);
    assert!(
        (model.transform_point3(local_centroid) - local_centroid).length() > 1e-5,
        "fixture node transform must make the world centroid observably different"
    );
}
