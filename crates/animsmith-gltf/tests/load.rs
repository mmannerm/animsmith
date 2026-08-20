//! End-to-end: load the checked-in fixture, verify the model shape,
//! and confirm the mechanical checks pass on clean data.

use animsmith_core::profile::ResolvedRoles;
use animsmith_core::{
    CheckCtx, CheckSelection, Config, MetricGrids, Severity, evaluate_checks, mechanical_checks,
    sample_clip,
};
use animsmith_core::{Document, TrackValues};
use std::path::PathBuf;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/rig.gltf")
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

#[test]
fn load_path_and_captured_bytes_are_equivalent() {
    let path = fixture();
    let bytes = std::fs::read(&path).expect("capture fixture");

    let from_path = animsmith_gltf::load(&path).expect("fixture loads by path");
    let from_bytes = animsmith_gltf::load_bytes(&path, &bytes).expect("fixture loads by bytes");

    assert_same_loaded_shape(&from_path, &from_bytes);
}

#[test]
fn glb_path_and_captured_bytes_are_equivalent() {
    let source = animsmith_gltf::load(&fixture()).expect("source fixture loads");
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("captured.glb");
    animsmith_gltf::write::write(&source, &path).expect("write synthetic GLB");
    let bytes = std::fs::read(&path).expect("capture GLB");

    let from_path = animsmith_gltf::load(&path).expect("GLB loads by path");
    let from_bytes = animsmith_gltf::load_bytes(&path, &bytes).expect("GLB loads by bytes");

    assert_same_loaded_shape(&from_path, &from_bytes);
}

#[test]
fn captured_self_contained_bytes_survive_primary_path_removal() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("captured.gltf");
    let bytes = std::fs::read(fixture()).expect("capture self-contained fixture");
    std::fs::write(&path, &bytes).expect("write primary input");
    std::fs::remove_file(&path).expect("remove primary input after capture");

    let doc = animsmith_gltf::load_bytes(&path, &bytes).expect("captured bytes still load");

    assert_eq!(
        doc.source.path.as_deref(),
        Some(path.to_str().expect("UTF-8 path"))
    );
    assert_eq!(doc.clips[0].name, "walk");
}

#[test]
fn byte_loader_requires_explicit_root_for_external_buffers() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("scene.gltf");
    let positions: [u8; 36] = [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 128, 63, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 128, 63, 0, 0, 0, 0,
    ];
    std::fs::write(dir.path().join("positions.bin"), positions).expect("write external buffer");
    let bytes = br#"{
        "asset": { "version": "2.0" },
        "buffers": [{ "uri": "positions.bin", "byteLength": 36 }],
        "bufferViews": [{ "buffer": 0, "byteOffset": 0, "byteLength": 36 }],
        "accessors": [{ "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3", "min": [0, 0, 0], "max": [1, 1, 0] }],
        "meshes": [{ "primitives": [{ "attributes": { "POSITION": 0 } }] }]
    }"#;

    let error = animsmith_gltf::load_bytes(&path, bytes).expect_err("ambient path is refused");
    assert!(
        error.to_string().contains("explicit trusted root"),
        "{error}"
    );

    let doc = animsmith_gltf::load_bytes_with_resource_root(&path, bytes, dir.path())
        .expect("resolves resource beneath supplied root");

    assert_eq!(doc.assets.meshes[0].primitives[0].positions.len(), 3);
}

#[test]
fn loads_fixture_into_core_model() {
    let doc = animsmith_gltf::load(&fixture()).expect("fixture loads");

    let names: Vec<&str> = doc.skeleton.bones.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(names, vec!["root", "hips", "foot"]);
    assert_eq!(doc.skeleton.bones[1].parent, Some(0));
    assert_eq!(doc.skeleton.bones[2].parent, Some(1));

    assert_eq!(doc.clips.len(), 1);
    let clip = &doc.clips[0];
    assert_eq!(clip.name, "walk");
    assert!((clip.duration_s - 1.0).abs() < 1e-6);
    assert_eq!(clip.tracks.len(), 2);
}

#[test]
fn fixture_is_lint_clean() {
    let doc = animsmith_gltf::load(&fixture()).expect("fixture loads");
    let config = Config::default();
    let roles = ResolvedRoles::default();
    let grids = MetricGrids::new(&doc);
    let ctx = CheckCtx::new(&grids, &roles, &config);
    let findings: Vec<_> = evaluate_checks(&ctx, &mechanical_checks(), CheckSelection::All)
        .expect("valid built-in catalog")
        .into_iter()
        .flat_map(|check| check.findings().to_vec())
        .collect();
    let serious: Vec<_> = findings
        .iter()
        .filter(|f| f.severity >= Severity::Warning)
        .collect();
    assert!(serious.is_empty(), "clean fixture flagged: {serious:#?}");
}

#[test]
fn fixture_pose_grid_fk_is_sane() {
    let doc = animsmith_gltf::load(&fixture()).expect("fixture loads");
    let clip = &doc.clips[0];
    let grid = sample_clip(&doc.skeleton, clip, 5);

    // At t=0 the foot sits at hips(0,1,0) + foot offset(0,-1,0) = origin.
    let foot0 = grid.model_position(0, 2);
    assert!(foot0.length() < 1e-5, "got {foot0:?}");

    // At t=1 the root has translated (0,0,1); hips rotated 90° about Y
    // moves the foot offset within the horizontal plane, height stays 0.
    let foot1 = grid.model_position(4, 2);
    assert!((foot1.z - 1.0).abs() < 1e-4, "got {foot1:?}");
    assert!(foot1.y.abs() < 1e-4, "got {foot1:?}");
}

#[test]
fn measurements_match_fixture() {
    let doc = animsmith_gltf::load(&fixture()).expect("fixture loads");
    let config = Config::default();
    let grids = MetricGrids::new(&doc);
    let measurements =
        animsmith_core::measure::measure_document(&grids, &ResolvedRoles::default(), &config);
    let walk = &measurements["walk"];
    assert_eq!(walk.frame_count, 3);
    assert_eq!(walk.animated_bones, vec!["hips", "root"]);
    let range = walk.bone_rotation_range_deg["hips"];
    assert!((range - 90.0).abs() < 0.1, "expected ~90°, got {range}");
}
