//! Quaternion fixes are byte-surgical, lossless, idempotent, and
//! composable through one session before writing.

use animsmith_core::model::*;
use animsmith_core::profile::ResolvedRoles;
use animsmith_core::{CheckCtx, Config, mechanical_checks, run_checks};
use glam::{Quat, Vec3};
use std::path::PathBuf;

fn clean_quats() -> Vec<Quat> {
    [0.0f32, 0.4, 0.8, 1.2, 1.6]
        .iter()
        .map(|&a| Quat::from_rotation_y(a))
        .collect()
}

fn scaled_quat(q: Quat, scale: f32) -> Quat {
    let [x, y, z, w] = q.to_array();
    Quat::from_xyzw(x * scale, y * scale, z * scale, w * scale)
}

fn doc_with_quats(quats: Vec<Quat>) -> Document {
    Document {
        skeleton: Skeleton {
            bones: vec![
                Bone {
                    name: "root".into(),
                    parent: None,
                    rest: Transform::IDENTITY,
                    inverse_bind: None,
                },
                Bone {
                    name: "spine".into(),
                    parent: Some(0),
                    rest: Transform {
                        translation: Vec3::new(0.0, 0.5, 0.0),
                        ..Transform::IDENTITY
                    },
                    inverse_bind: None,
                },
            ],
        },
        clips: vec![Clip {
            name: "sway".into(),
            duration_s: 1.0,
            tracks: vec![
                Track {
                    bone: 1,
                    property: Property::Rotation,
                    interpolation: Interpolation::Linear,
                    times: vec![0.0, 0.25, 0.5, 0.75, 1.0],
                    values: TrackValues::Quats(quats),
                },
                // A translation track that must remain byte-identical.
                Track {
                    bone: 0,
                    property: Property::Translation,
                    interpolation: Interpolation::Linear,
                    times: vec![0.0, 1.0],
                    values: TrackValues::Vec3s(vec![Vec3::ZERO, Vec3::new(0.0, 0.0, 2.0)]),
                },
            ],
        }],
        assets: Default::default(),
        source: SourceInfo::default(),
    }
}

/// A document whose rotation track has two sign-flipped keys (same
/// rotations as the clean sequence, opposite hemisphere).
fn flipped_doc() -> Document {
    let mut quats = clean_quats();
    quats[1] = -quats[1];
    quats[3] = -quats[3];
    doc_with_quats(quats)
}

fn non_unit_doc() -> Document {
    let mut quats = clean_quats();
    quats[2] = scaled_quat(quats[2], 1.2);
    doc_with_quats(quats)
}

fn flipped_non_unit_doc() -> Document {
    let mut quats = clean_quats();
    quats[1] = scaled_quat(-quats[1], 1.2);
    quats[3] = -quats[3];
    doc_with_quats(quats)
}

fn lint_count(doc: &Document, check_id: &str) -> usize {
    let config = Config::default();
    let roles = ResolvedRoles::default();
    let ctx = CheckCtx::new(doc, &roles, &config);
    run_checks(&ctx, &mechanical_checks())
        .iter()
        .filter(|f| f.check_id == check_id)
        .count()
}

fn lint_flip_count(doc: &Document) -> usize {
    lint_count(doc, "quat-flip")
}

fn lint_norm_count(doc: &Document) -> usize {
    lint_count(doc, "quat-norm")
}

fn unique_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("animsmith-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn fix_repairs_flips_losslessly_and_idempotently() {
    let dir = std::env::temp_dir().join("animsmith-fix-test");
    std::fs::create_dir_all(&dir).unwrap();
    let dirty = dir.join("dirty.glb");
    let fixed = dir.join("fixed.glb");

    let doc = flipped_doc();
    animsmith_gltf::write::write(&doc, &dirty).expect("writes");
    assert_eq!(lint_flip_count(&animsmith_gltf::load(&dirty).unwrap()), 1);

    let report = animsmith_gltf::fix::fix_quat_hemisphere(&dirty, &fixed).expect("fixes");
    assert_eq!(report.total_fixed(), 2, "both flipped keys repaired");
    assert!(report.skipped.is_empty(), "skipped: {:?}", report.skipped);

    let repaired = animsmith_gltf::load(&fixed).expect("reloads");
    assert_eq!(lint_flip_count(&repaired), 0, "no flips remain");

    // Lossless: every key still represents the same rotation.
    let clean = clean_quats();
    let track = &repaired.clips[0].tracks[0];
    for (k, want) in clean.iter().enumerate() {
        let got = track.key_quat(k).unwrap();
        assert!(
            got.angle_between(*want) < 1e-5,
            "key {k} rotation changed: {got:?} vs {want:?}"
        );
    }

    // Untouched bytes stay untouched: files differ only inside the
    // rotation accessor (2 keys × 16 bytes).
    let a = std::fs::read(&dirty).unwrap();
    let b = std::fs::read(&fixed).unwrap();
    assert_eq!(a.len(), b.len(), "file length must not change");
    let differing = a.iter().zip(&b).filter(|(x, y)| x != y).count();
    assert!(
        differing <= 32,
        "expected ≤32 differing bytes, got {differing}"
    );

    // Idempotent: a second pass changes nothing.
    let again = dir.join("again.glb");
    let report2 = animsmith_gltf::fix::fix_quat_hemisphere(&fixed, &again).expect("re-fixes");
    assert_eq!(report2.total_fixed(), 0);
    assert_eq!(
        std::fs::read(&fixed).unwrap(),
        std::fs::read(&again).unwrap()
    );
}

#[test]
fn fix_quat_norm_repairs_keys_losslessly_and_idempotently() {
    let dir = unique_temp_dir("quat-norm");
    let dirty = dir.join("dirty.glb");
    let fixed = dir.join("fixed.glb");

    animsmith_gltf::write::write(&non_unit_doc(), &dirty).expect("writes");
    assert_eq!(lint_norm_count(&animsmith_gltf::load(&dirty).unwrap()), 1);

    let report = animsmith_gltf::fix::fix_quat_norm(&dirty, &fixed).expect("normalizes");
    assert_eq!(report.total_fixed(), 1);
    assert!(report.skipped.is_empty(), "skipped: {:?}", report.skipped);

    let repaired = animsmith_gltf::load(&fixed).expect("reloads");
    assert_eq!(lint_norm_count(&repaired), 0);
    assert_eq!(lint_flip_count(&repaired), 0);
    let TrackValues::Quats(quats) = &repaired.clips[0].tracks[0].values else {
        panic!("rotation track expected");
    };
    for (got, want) in quats.iter().zip(clean_quats()) {
        assert!(
            got.angle_between(want) < 1e-5,
            "normalization must preserve the represented rotation"
        );
    }

    let again = dir.join("again.glb");
    let report2 = animsmith_gltf::fix::fix_quat_norm(&fixed, &again).expect("re-normalizes");
    assert_eq!(report2.total_fixed(), 0);
    assert_eq!(
        std::fs::read(&fixed).unwrap(),
        std::fs::read(&again).unwrap()
    );
}

#[test]
fn fix_quat_norm_skips_cubic_tracks_to_preserve_tangents() {
    let mut values = Vec::new();
    for (k, q) in clean_quats().into_iter().take(3).enumerate() {
        let value = if k == 1 { scaled_quat(q, 1.2) } else { q };
        values.push(Quat::from_xyzw(0.1, 0.0, 0.0, 0.0)); // in-tangent
        values.push(value);
        values.push(Quat::from_xyzw(0.0, 0.1, 0.0, 0.0)); // out-tangent
    }
    let mut doc = flipped_doc();
    doc.clips[0].tracks[0] = Track {
        bone: 1,
        property: Property::Rotation,
        interpolation: Interpolation::CubicSpline,
        times: vec![0.0, 0.5, 1.0],
        values: TrackValues::Quats(values),
    };

    let dir = unique_temp_dir("quat-norm-cubic");
    let dirty = dir.join("dirty.glb");
    let fixed = dir.join("fixed.glb");
    animsmith_gltf::write::write(&doc, &dirty).expect("writes");
    let before = std::fs::read(&dirty).unwrap();

    let report = animsmith_gltf::fix::fix_quat_norm(&dirty, &fixed).expect("checks cubic");
    assert_eq!(report.total_fixed(), 0);
    assert!(
        report
            .skipped
            .iter()
            .any(|reason| reason.contains("quat-norm skipped to preserve tangents")),
        "skipped: {:?}",
        report.skipped
    );
    assert_eq!(
        before,
        std::fs::read(&fixed).unwrap(),
        "cubic quat-norm skip must not rewrite animation bytes"
    );
}

#[test]
fn fix_session_composes_distinct_repairs_in_memory_before_writing() {
    let dir = unique_temp_dir("session-compose");
    let dirty = dir.join("dirty.glb");
    let fixed = dir.join("fixed.glb");
    animsmith_gltf::write::write(&flipped_non_unit_doc(), &dirty).expect("writes");

    let mut session = animsmith_gltf::fix::FixSession::read(&dirty).expect("opens session");
    let norm = session.fix_quat_norm();
    assert_eq!(norm.total_fixed(), 1);

    let flip = session.fix_quat_hemisphere();
    assert_eq!(flip.total_fixed(), 2);
    assert!(!fixed.exists(), "session should not write until requested");

    session
        .write(&dirty, &fixed)
        .expect("writes patched buffers");
    let repaired = animsmith_gltf::load(&fixed).unwrap();
    assert_eq!(lint_norm_count(&repaired), 0);
    assert_eq!(lint_flip_count(&repaired), 0);
}

#[test]
fn malformed_external_accessor_range_is_skipped_without_panic() {
    let dir = unique_temp_dir("short-buffer");
    let gltf = dir.join("short.gltf");
    let bin = dir.join("short.bin");
    std::fs::write(&bin, [0u8; 8]).unwrap();
    std::fs::write(
        &gltf,
        r#"{
  "asset": { "version": "2.0" },
  "buffers": [{ "uri": "short.bin", "byteLength": 64 }],
  "bufferViews": [{ "buffer": 0, "byteOffset": 0, "byteLength": 64 }],
  "accessors": [
    { "bufferView": 0, "componentType": 5126, "count": 2, "type": "SCALAR", "min": [0], "max": [1] },
    { "bufferView": 0, "componentType": 5126, "count": 2, "type": "VEC4" }
  ],
  "nodes": [{ "name": "root" }],
  "animations": [{
    "samplers": [{ "input": 0, "output": 1, "interpolation": "LINEAR" }],
    "channels": [{ "sampler": 0, "target": { "node": 0, "path": "rotation" } }]
  }],
  "scenes": [{ "nodes": [0] }],
  "scene": 0
}"#,
    )
    .unwrap();

    let report = animsmith_gltf::fix::inspect_quat_hemisphere(&gltf).expect("inspects");
    assert_eq!(report.total_fixed(), 0);
    assert!(
        report
            .skipped
            .iter()
            .any(|reason| reason.contains("outside buffer length")),
        "skipped: {:?}",
        report.skipped
    );
}

#[test]
fn unsafe_external_buffer_uri_is_rejected() {
    let dir = unique_temp_dir("unsafe-buffer-uri");
    let gltf = dir.join("unsafe.gltf");
    std::fs::write(
        &gltf,
        r#"{
  "asset": { "version": "2.0" },
  "buffers": [{ "uri": "../escape.bin", "byteLength": 1 }],
  "nodes": [{ "name": "root" }],
  "scenes": [{ "nodes": [0] }],
  "scene": 0
}"#,
    )
    .unwrap();

    let err = animsmith_gltf::fix::inspect_quat_hemisphere(&gltf).expect_err("rejects URI");
    assert!(
        err.to_string().contains("unsafe external buffer URI"),
        "{err}"
    );
}

#[test]
fn in_place_fix_works() {
    let dir = std::env::temp_dir().join("animsmith-fix-test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("inplace.glb");
    animsmith_gltf::write::write(&flipped_doc(), &path).expect("writes");
    let report = animsmith_gltf::fix::fix_quat_hemisphere(&path, &path).expect("fixes");
    assert_eq!(report.total_fixed(), 2);
    assert_eq!(lint_flip_count(&animsmith_gltf::load(&path).unwrap()), 0);
}

#[test]
fn cubic_tracks_are_fixed_by_triplet() {
    // A CUBICSPLINE rotation track whose middle key (value + tangents)
    // is sign-flipped.
    let angles = [0.0f32, 0.5, 1.0];
    let mut values: Vec<Quat> = Vec::new();
    for (k, &a) in angles.iter().enumerate() {
        let q = Quat::from_rotation_y(a);
        let sign = if k == 1 { -1.0 } else { 1.0 };
        values.push(Quat::from_xyzw(0.0, 0.0, 0.0, 0.0)); // in-tangent
        values.push(q * sign);
        values.push(Quat::from_xyzw(0.0, 0.0, 0.0, 0.0)); // out-tangent
    }
    let mut doc = flipped_doc();
    doc.clips[0].tracks[0] = Track {
        bone: 1,
        property: Property::Rotation,
        interpolation: Interpolation::CubicSpline,
        times: vec![0.0, 0.5, 1.0],
        values: TrackValues::Quats(values),
    };
    let dir = std::env::temp_dir().join("animsmith-fix-test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("cubic.glb");
    animsmith_gltf::write::write(&doc, &path).expect("writes");
    assert_eq!(lint_flip_count(&animsmith_gltf::load(&path).unwrap()), 1);

    let report = animsmith_gltf::fix::fix_quat_hemisphere(&path, &path).expect("fixes");
    assert_eq!(report.total_fixed(), 1, "one triplet repaired");
    assert!(report.skipped.is_empty(), "cubic no longer skipped");

    let repaired = animsmith_gltf::load(&path).expect("reloads");
    assert_eq!(lint_flip_count(&repaired), 0);
    let track = &repaired.clips[0].tracks[0];
    assert!(
        track
            .key_quat(1)
            .unwrap()
            .angle_between(Quat::from_rotation_y(0.5))
            < 1e-5,
        "rotation preserved"
    );
}
