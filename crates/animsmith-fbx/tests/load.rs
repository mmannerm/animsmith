use animsmith_core::glam::{Mat4, Vec3};
use animsmith_core::model::Property;
use std::path::PathBuf;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/rigged_triangle.fbx")
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
    assert_eq!(mesh.name, "tri");
    assert_eq!(mesh.node, 2);
    assert_eq!(mesh.skin_joints, vec![1]);
    assert_eq!(mesh.skin_ibms.len(), 1);

    let prim = mesh.primitives.first().expect("primitive loaded");
    assert_eq!(prim.positions.len(), 3);
    assert_eq!(prim.indices, vec![0, 1, 2]);
    assert_eq!(prim.joints, vec![[0, 0, 0, 0]; 3]);
    assert_eq!(prim.weights, vec![[1.0, 0.0, 0.0, 0.0]; 3]);

    assert_eq!(doc.assets.materials.len(), 0);
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
    assert!(source.contains("\"UpAxis\", \"int\", \"Integer\", \"\",2"));
    assert!(source.contains("\"FrontAxis\", \"int\", \"Integer\", \"\",1"));

    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("centimetre-z-up.fbx");
    std::fs::write(&path, source).expect("write transformed fixture");
    let doc = animsmith_fbx::load(&path).expect("Z-up fixture loads");

    let mesh = doc.assets.meshes.first().expect("mesh loaded");
    let primitive = mesh.primitives.first().expect("primitive loaded");
    let model = rest_models(&doc)[mesh.node];
    let source_x = model.transform_point3(primitive.positions[1]);
    let source_y = model.transform_point3(primitive.positions[2]);

    assert_vec3_near(source_x, Vec3::X);
    assert_vec3_near(source_y, -Vec3::Z);
    assert!(source_x.y.abs() < 1e-5 && source_y.y.abs() < 1e-5);
}
