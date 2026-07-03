//! Hemisphere-normalization fix: flipped keys are repaired losslessly
//! (same rotations, hemisphere-consistent), the fix is idempotent, and
//! untouched bytes stay untouched.

use animsmith_core::model::*;
use animsmith_core::profile::ResolvedRoles;
use animsmith_core::{CheckCtx, Config, mechanical_checks, run_checks};
use glam::{Quat, Vec3};

/// A document whose rotation track has two sign-flipped keys (same
/// rotations as the clean sequence, opposite hemisphere).
fn flipped_doc() -> Document {
    let angles = [0.0f32, 0.4, 0.8, 1.2, 1.6];
    let mut quats: Vec<Quat> = angles.iter().map(|&a| Quat::from_rotation_y(a)).collect();
    quats[1] = -quats[1];
    quats[3] = -quats[3];
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
        source: SourceInfo::default(),
    }
}

fn lint_flip_count(doc: &Document) -> usize {
    let config = Config::default();
    let roles = ResolvedRoles::default();
    let ctx = CheckCtx::new(doc, &roles, &config);
    run_checks(&ctx, &mechanical_checks())
        .iter()
        .filter(|f| f.check_id == "quat-flip")
        .count()
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
    assert_eq!(report.total_flipped(), 2, "both flipped keys repaired");
    assert!(report.skipped.is_empty(), "skipped: {:?}", report.skipped);

    let repaired = animsmith_gltf::load(&fixed).expect("reloads");
    assert_eq!(lint_flip_count(&repaired), 0, "no flips remain");

    // Lossless: every key still represents the same rotation.
    let clean: Vec<Quat> = [0.0f32, 0.4, 0.8, 1.2, 1.6]
        .iter()
        .map(|&a| Quat::from_rotation_y(a))
        .collect();
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
    assert_eq!(report2.total_flipped(), 0);
    assert_eq!(
        std::fs::read(&fixed).unwrap(),
        std::fs::read(&again).unwrap()
    );
}

#[test]
fn in_place_fix_works() {
    let dir = std::env::temp_dir().join("animsmith-fix-test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("inplace.glb");
    animsmith_gltf::write::write(&flipped_doc(), &path).expect("writes");
    let report = animsmith_gltf::fix::fix_quat_hemisphere(&path, &path).expect("fixes");
    assert_eq!(report.total_flipped(), 2);
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
    assert_eq!(report.total_flipped(), 1, "one triplet repaired");
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
