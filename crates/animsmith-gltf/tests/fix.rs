//! Quaternion fixes are byte-surgical, lossless, idempotent, and
//! composable through one session before writing.

use animsmith_core::model::*;
use animsmith_core::profile::ResolvedRoles;
use animsmith_core::{
    CheckCtx, CheckSelection, Config, MetricGrids, evaluate_checks, mechanical_checks,
};
use animsmith_gltf::fix::{FixSession, Repair};
use animsmith_testkit::{quats_from_angles, scaled_quat, two_bone_rotation_doc};
use glam::{Quat, Vec3};

fn clean_quats() -> Vec<Quat> {
    quats_from_angles(&[0.0, 0.4, 0.8, 1.2, 1.6])
}

fn doc_with_quats(quats: Vec<Quat>) -> Document {
    // The `sway` clip carries a translation track alongside the rotation
    // track; some tests assert it stays byte-identical across a fix.
    two_bone_rotation_doc("sway", quats, true)
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

/// Rotation keys bracketing the fix's `QUAT_NORM_TOLERANCE` (1e-3 on
/// `|len - 1|`, private to `fix.rs`) tightly: key 1 is `0.99e-3` off
/// unit (just inside) and key 3 is `1.01e-3` off (just outside). The
/// `1e-5` margin on each side pins the threshold to `1e-3 ± 1e-5`, so a
/// materially wrong tolerance (e.g. `1.05e-3`) or a flipped comparison
/// direction normalizes the wrong key and the test fails. (The exact
/// `>` vs `>=` behaviour *at* `|len-1| == tol` is immaterial — a key
/// sitting exactly on the tolerance is harmless either way — and not
/// robustly representable in f32, so it is deliberately not pinned.)
fn boundary_doc() -> Document {
    let mut quats = clean_quats();
    quats[1] = scaled_quat(quats[1], 1.0 + 0.99e-3); // |len-1| < tol: keep
    quats[3] = scaled_quat(quats[3], 1.0 + 1.01e-3); // |len-1| > tol: fix
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
    let grids = MetricGrids::new(doc);
    let ctx = CheckCtx::new(&grids, &roles, &config);
    evaluate_checks(&ctx, &mechanical_checks(), CheckSelection::All)
        .expect("valid built-in catalog")
        .iter()
        .flat_map(|check| check.findings())
        .filter(|f| f.check_id == check_id)
        .count()
}

fn lint_flip_count(doc: &Document) -> usize {
    lint_count(doc, "quat-flip")
}

fn lint_norm_count(doc: &Document) -> usize {
    lint_count(doc, "quat-norm")
}

fn unique_temp_dir(name: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(&format!("animsmith-{name}-"))
        .tempdir()
        .unwrap()
}

#[test]
fn fix_repairs_flips_losslessly_and_idempotently() {
    let dir = unique_temp_dir("fix-test");
    let dirty = dir.path().join("dirty.glb");
    let fixed = dir.path().join("fixed.glb");

    let doc = flipped_doc();
    animsmith_gltf::write::write(&doc, &dirty).expect("writes");
    assert_eq!(lint_flip_count(&animsmith_gltf::load(&dirty).unwrap()), 1);

    let report = FixSession::apply_to_path(&dirty, &fixed, Repair::QuatFlip).expect("fixes");
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
    // ...and those bytes are the rotation keys, not stray edits elsewhere:
    // the sibling translation track's decoded values survive unchanged.
    // Bounding the count alone leaves *where* the changes landed
    // unconstrained; re-asserting the untouched channel's values (issue
    // #35's sanctioned alternative to pinning raw byte offsets) means a
    // diff that corrupted the translation output would fail even while
    // staying under the 32-byte budget.
    let trans = |doc: &Document| -> Vec<Vec3> {
        let track = doc.clips[0]
            .tracks
            .iter()
            .find(|t| t.property == Property::Translation)
            .expect("translation track");
        match &track.values {
            TrackValues::Vec3s(v) => v.clone(),
            _ => panic!("translation track must be Vec3s"),
        }
    };
    assert_eq!(
        trans(&repaired),
        trans(&flipped_doc()),
        "translation track must be untouched by a rotation-only fix"
    );

    // Idempotent: a second pass changes nothing.
    let again = dir.path().join("again.glb");
    let report2 = FixSession::apply_to_path(&fixed, &again, Repair::QuatFlip).expect("re-fixes");
    assert_eq!(report2.total_fixed(), 0);
    assert_eq!(
        std::fs::read(&fixed).unwrap(),
        std::fs::read(&again).unwrap()
    );
}

#[test]
fn fix_quat_norm_repairs_keys_losslessly_and_idempotently() {
    let dir = unique_temp_dir("quat-norm");
    let dirty = dir.path().join("dirty.glb");
    let fixed = dir.path().join("fixed.glb");

    animsmith_gltf::write::write(&non_unit_doc(), &dirty).expect("writes");
    assert_eq!(lint_norm_count(&animsmith_gltf::load(&dirty).unwrap()), 1);

    let report = FixSession::apply_to_path(&dirty, &fixed, Repair::QuatNorm).expect("normalizes");
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

    let again = dir.path().join("again.glb");
    let report2 =
        FixSession::apply_to_path(&fixed, &again, Repair::QuatNorm).expect("re-normalizes");
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
    let dirty = dir.path().join("dirty.glb");
    let fixed = dir.path().join("fixed.glb");
    animsmith_gltf::write::write(&doc, &dirty).expect("writes");
    let before = std::fs::read(&dirty).unwrap();

    let report = FixSession::apply_to_path(&dirty, &fixed, Repair::QuatNorm).expect("checks cubic");
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
fn fix_quat_norm_repairs_only_keys_past_the_tolerance_boundary() {
    let dir = unique_temp_dir("quat-norm-boundary");
    let dirty = dir.path().join("dirty.glb");
    let fixed = dir.path().join("fixed.glb");
    animsmith_gltf::write::write(&boundary_doc(), &dirty).expect("writes");

    let report = FixSession::apply_to_path(&dirty, &fixed, Repair::QuatNorm).expect("normalizes");
    assert_eq!(
        report.total_fixed(),
        1,
        "only the key past the tolerance is repaired"
    );

    // The loader stores raw components, so lengths survive the round-trip.
    let repaired = animsmith_gltf::load(&fixed).expect("reloads");
    let TrackValues::Quats(quats) = &repaired.clips[0].tracks[0].values else {
        panic!("rotation track expected");
    };
    // Just-inside key keeps its authored (non-unit) length; a
    // wrongly-widened tolerance or flipped comparison would normalize it
    // (dropping the length to 1.0, ~1e-3 away — far outside this 1e-5 band).
    assert!(
        (quats[1].length() - (1.0 + 0.99e-3)).abs() < 1e-5,
        "just-inside key must be left untouched: len {}",
        quats[1].length()
    );
    // Just-outside key is scaled back to unit length.
    assert!(
        (quats[3].length() - 1.0).abs() < 1e-5,
        "just-outside key must be normalized: len {}",
        quats[3].length()
    );
}

#[test]
fn fix_quat_norm_pins_non_finite_skip_reason() {
    let mut quats = clean_quats();
    quats[2] = Quat::from_xyzw(f32::NAN, 0.0, 0.0, 0.0);
    let dir = unique_temp_dir("quat-norm-nonfinite");
    let dirty = dir.path().join("dirty.glb");
    let fixed = dir.path().join("fixed.glb");
    animsmith_gltf::write::write(&doc_with_quats(quats), &dirty).expect("writes");
    let before = std::fs::read(&dirty).unwrap();

    let report = FixSession::apply_to_path(&dirty, &fixed, Repair::QuatNorm).expect("normalizes");
    assert_eq!(
        report.total_fixed(),
        0,
        "a non-finite key is safely skipped, never scaled"
    );
    assert!(
        report
            .skipped
            .iter()
            .any(|reason| reason.contains("non-finite rotation key")),
        "skipped: {:?}",
        report.skipped
    );
    // The skip must be byte-surgical: a skipped key is never rewritten,
    // so the output is identical to the input (never divided by its
    // non-finite length, which would corrupt the key).
    assert_eq!(
        before,
        std::fs::read(&fixed).unwrap(),
        "skipping a non-finite key must not rewrite any bytes"
    );
}

#[test]
fn fix_quat_norm_pins_zero_length_skip_reason() {
    let mut quats = clean_quats();
    quats[2] = Quat::from_xyzw(0.0, 0.0, 0.0, 0.0);
    let dir = unique_temp_dir("quat-norm-zerolen");
    let dirty = dir.path().join("dirty.glb");
    let fixed = dir.path().join("fixed.glb");
    animsmith_gltf::write::write(&doc_with_quats(quats), &dirty).expect("writes");
    let before = std::fs::read(&dirty).unwrap();

    let report = FixSession::apply_to_path(&dirty, &fixed, Repair::QuatNorm).expect("normalizes");
    assert_eq!(
        report.total_fixed(),
        0,
        "a zero-length key cannot be normalized, so it is skipped"
    );
    assert!(
        report
            .skipped
            .iter()
            .any(|reason| reason.contains("zero-length rotation key")),
        "skipped: {:?}",
        report.skipped
    );
    // The skip must be byte-surgical: a zero-length key is never
    // rewritten (dividing by its zero length would produce NaN), so the
    // output stays identical to the input.
    assert_eq!(
        before,
        std::fs::read(&fixed).unwrap(),
        "skipping a zero-length key must not rewrite any bytes"
    );
}

#[test]
fn fix_session_composes_distinct_repairs_in_memory_before_writing() {
    let dir = unique_temp_dir("session-compose");
    let dirty = dir.path().join("dirty.glb");
    let fixed = dir.path().join("fixed.glb");
    animsmith_gltf::write::write(&flipped_non_unit_doc(), &dirty).expect("writes");

    let mut session = FixSession::read(&dirty).expect("opens session");
    let norm = session.apply(Repair::QuatNorm);
    assert_eq!(norm.total_fixed(), 1);

    let flip = session.apply(Repair::QuatFlip);
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
    let gltf = dir.path().join("short.gltf");
    let bin = dir.path().join("short.bin");
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

    let report = FixSession::inspect(&gltf, Repair::QuatFlip).expect("inspects");
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
    let gltf = dir.path().join("unsafe.gltf");
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

    let err = FixSession::inspect(&gltf, Repair::QuatFlip).expect_err("rejects URI");
    assert!(
        err.to_string().contains("unsafe external buffer URI"),
        "{err}"
    );
}

#[test]
fn in_place_fix_works() {
    let dir = unique_temp_dir("fix-test");
    let path = dir.path().join("inplace.glb");
    animsmith_gltf::write::write(&flipped_doc(), &path).expect("writes");
    let report = FixSession::apply_to_path(&path, &path, Repair::QuatFlip).expect("fixes");
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
    let dir = unique_temp_dir("fix-test");
    let path = dir.path().join("cubic.glb");
    animsmith_gltf::write::write(&doc, &path).expect("writes");
    assert_eq!(lint_flip_count(&animsmith_gltf::load(&path).unwrap()), 1);

    let report = FixSession::apply_to_path(&path, &path, Repair::QuatFlip).expect("fixes");
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
