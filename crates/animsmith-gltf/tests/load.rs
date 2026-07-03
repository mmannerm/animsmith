//! End-to-end: load the checked-in fixture, verify the model shape,
//! and confirm the mechanical checks pass on clean data.

use animsmith_core::profile::ResolvedRoles;
use animsmith_core::{CheckCtx, Config, Severity, mechanical_checks, run_checks, sample_clip};
use std::path::PathBuf;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/rig.gltf")
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
    let ctx = CheckCtx::new(&doc, &roles, &config);
    let findings = run_checks(&ctx, &mechanical_checks());
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
    let measurements = animsmith_core::measure::measure_document(&doc, &ResolvedRoles::default());
    let walk = &measurements["walk"];
    assert_eq!(walk.frame_count, 3);
    assert_eq!(walk.animated_bones, vec!["hips", "root"]);
    let range = walk.bone_rotation_range_deg["hips"];
    assert!((range - 90.0).abs() < 0.1, "expected ~90°, got {range}");
}
