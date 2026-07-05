//! Mutation-style check tests, after the incubating pipeline's
//! discipline: start from a clean document (zero findings), corrupt
//! exactly one thing, and assert exactly the expected check fires.

use animsmith_core::model::*;
use animsmith_core::profile::ResolvedRoles;
use animsmith_core::{CheckCtx, Config, Severity, mechanical_checks, run_checks};
use glam::{Quat, Vec3};

/// A clean 2-bone document with a rotation and a translation track.
fn clean_doc() -> Document {
    let skeleton = Skeleton {
        bones: vec![
            Bone {
                name: "hips".into(),
                parent: None,
                rest: Transform::IDENTITY,
                inverse_bind: None,
            },
            Bone {
                name: "spine".into(),
                parent: Some(0),
                rest: Transform {
                    translation: Vec3::new(0.0, 0.3, 0.0),
                    ..Transform::IDENTITY
                },
                inverse_bind: None,
            },
        ],
    };
    let rotation = Track {
        bone: 1,
        property: Property::Rotation,
        interpolation: Interpolation::Linear,
        times: vec![0.0, 0.5, 1.0],
        values: TrackValues::Quats(vec![
            Quat::IDENTITY,
            Quat::from_rotation_y(0.8),
            Quat::from_rotation_y(1.6),
        ]),
    };
    let translation = Track {
        bone: 0,
        property: Property::Translation,
        interpolation: Interpolation::Linear,
        times: vec![0.0, 0.5, 1.0],
        values: TrackValues::Vec3s(vec![
            Vec3::ZERO,
            Vec3::new(0.0, 0.05, 0.5),
            Vec3::new(0.0, 0.0, 1.0),
        ]),
    };
    Document {
        skeleton,
        clips: vec![Clip {
            name: "walk".into(),
            duration_s: 1.0,
            tracks: vec![rotation, translation],
        }],
        assets: Default::default(),
        source: SourceInfo::default(),
    }
}

fn lint(doc: &Document) -> Vec<animsmith_core::Finding> {
    let config = Config::default();
    let roles = ResolvedRoles::default();
    let ctx = CheckCtx::new(doc, &roles, &config);
    run_checks(&ctx, &mechanical_checks())
}

fn assert_single(doc: &Document, check_id: &str, severity: Severity) {
    let findings = lint(doc);
    assert_eq!(
        findings.len(),
        1,
        "expected exactly one finding, got: {findings:#?}"
    );
    assert_eq!(findings[0].check_id, check_id);
    assert_eq!(findings[0].severity, severity);
}

#[test]
fn clean_document_has_no_findings() {
    let findings = lint(&clean_doc());
    assert!(findings.is_empty(), "clean doc flagged: {findings:#?}");
}

#[test]
fn nan_value_is_flagged() {
    let mut doc = clean_doc();
    if let TrackValues::Vec3s(v) = &mut doc.clips[0].tracks[1].values {
        v[1].y = f32::NAN;
    }
    assert_single(&doc, "nan", Severity::Error);
}

#[test]
fn nan_time_is_flagged() {
    let mut doc = clean_doc();
    doc.clips[0].tracks[1].times[1] = f32::INFINITY;
    // The corrupted time also breaks monotonicity; the nan finding must
    // be among the results and attributed to the right bone.
    let findings = lint(&doc);
    let nan = findings
        .iter()
        .find(|f| f.check_id == "nan")
        .expect("nan finding");
    assert_eq!(nan.bone.as_deref(), Some("hips"));
}

#[test]
fn non_monotonic_times_are_flagged() {
    let mut doc = clean_doc();
    // Corrupt ordering mid-track, keeping start (0.0) and end (1.0)
    // intact so no other check fires.
    doc.clips[0].tracks[0].times = vec![0.0, 0.6, 0.4, 1.0];
    if let TrackValues::Quats(v) = &mut doc.clips[0].tracks[0].values {
        v.push(Quat::from_rotation_y(2.0));
    }
    assert_single(&doc, "time-monotonic", Severity::Error);
}

#[test]
fn negative_time_beyond_tolerance_is_flagged() {
    let mut doc = clean_doc();
    doc.clips[0].tracks[0].times = vec![-0.01, 0.5, 1.0];
    assert_single(&doc, "time-monotonic", Severity::Error);
}

#[test]
fn f32_quantization_dust_at_zero_is_tolerated() {
    // Bake pipelines that slice frame ranges leave first keys like
    // -1e-6 s; engines clamp these harmlessly.
    let mut doc = clean_doc();
    doc.clips[0].tracks[0].times = vec![-1e-6, 0.5, 1.0];
    let findings = lint(&doc);
    assert!(findings.is_empty(), "dust flagged: {findings:#?}");
}

#[test]
fn late_first_key_is_noted() {
    let mut doc = clean_doc();
    doc.clips[0].tracks[0].times = vec![0.4, 0.7, 1.0];
    let findings = lint(&doc);
    assert!(
        findings
            .iter()
            .any(|f| f.check_id == "time-monotonic" && f.severity == Severity::Note),
        "got: {findings:#?}"
    );
}

#[test]
fn denormalized_quat_is_flagged() {
    let mut doc = clean_doc();
    if let TrackValues::Quats(v) = &mut doc.clips[0].tracks[0].values {
        v[1] = Quat::from_xyzw(0.0, 0.65, 0.0, 0.65); // |q| ≈ 0.92
    }
    let findings = lint(&doc);
    let f = findings
        .iter()
        .find(|f| f.check_id == "quat-norm")
        .expect("quat-norm finding");
    assert_eq!(f.severity, Severity::Error);
    assert_eq!(f.bone.as_deref(), Some("spine"));
}

#[test]
fn hemisphere_flip_is_flagged() {
    let mut doc = clean_doc();
    if let TrackValues::Quats(v) = &mut doc.clips[0].tracks[0].values {
        v[1] = -v[1]; // same rotation, opposite hemisphere
    }
    assert_single(&doc, "quat-flip", Severity::Warning);
}

#[test]
fn zero_duration_is_flagged() {
    let mut doc = clean_doc();
    doc.clips[0].duration_s = 0.0;
    // Track end times no longer matter for a degenerate clip.
    let findings = lint(&doc);
    assert!(
        findings
            .iter()
            .any(|f| f.check_id == "duration-sanity" && f.severity == Severity::Error),
        "got: {findings:#?}"
    );
}

#[test]
fn mismatched_channel_ends_are_flagged() {
    let mut doc = clean_doc();
    doc.clips[0].tracks[1].times = vec![0.0, 0.3, 0.6]; // rotation still ends at 1.0
    assert_single(&doc, "duration-sanity", Severity::Warning);
}

#[test]
fn single_key_pin_does_not_count_toward_end_spread() {
    // A one-key track at t=0 is a pinned value (a common bake idiom),
    // not a truncated channel.
    let mut doc = clean_doc();
    doc.clips[0].tracks.push(Track {
        bone: 1,
        property: Property::Translation,
        interpolation: Interpolation::Linear,
        times: vec![0.0],
        values: TrackValues::Vec3s(vec![Vec3::new(0.0, 0.3, 0.0)]),
    });
    let findings = lint(&doc);
    assert!(findings.is_empty(), "pin flagged: {findings:#?}");
}

#[test]
fn empty_clip_is_flagged() {
    let mut doc = clean_doc();
    doc.clips[0].tracks.clear();
    assert_single(&doc, "duration-sanity", Severity::Warning);
}

#[test]
fn scale_keys_are_flagged() {
    let mut doc = clean_doc();
    doc.clips[0].tracks.push(Track {
        bone: 1,
        property: Property::Scale,
        interpolation: Interpolation::Linear,
        times: vec![0.0, 1.0],
        values: TrackValues::Vec3s(vec![Vec3::ONE, Vec3::splat(1.2)]),
    });
    assert_single(&doc, "scale-keys", Severity::Warning);
}

#[test]
fn non_uniform_scale_gets_second_finding() {
    let mut doc = clean_doc();
    doc.clips[0].tracks.push(Track {
        bone: 1,
        property: Property::Scale,
        interpolation: Interpolation::Linear,
        times: vec![0.0, 1.0],
        values: TrackValues::Vec3s(vec![Vec3::ONE, Vec3::new(1.2, 1.0, 1.0)]),
    });
    let findings = lint(&doc);
    assert_eq!(
        findings
            .iter()
            .filter(|f| f.check_id == "scale-keys")
            .count(),
        2,
        "got: {findings:#?}"
    );
}

#[test]
fn all_ones_scale_track_is_constant_not_scaling() {
    let mut doc = clean_doc();
    doc.clips[0].tracks.push(Track {
        bone: 1,
        property: Property::Scale,
        interpolation: Interpolation::Linear,
        times: vec![0.0, 1.0],
        values: TrackValues::Vec3s(vec![Vec3::ONE, Vec3::ONE]),
    });
    assert_single(&doc, "constant-track", Severity::Note);
}

#[test]
fn constant_rotation_track_is_noted() {
    let mut doc = clean_doc();
    let q = Quat::from_rotation_y(0.8);
    if let TrackValues::Quats(v) = &mut doc.clips[0].tracks[0].values {
        *v = vec![q, q, q];
    }
    assert_single(&doc, "constant-track", Severity::Note);
}

#[test]
fn measurements_report_rotation_range() {
    let doc = clean_doc();
    let config = Config::default();
    let measurements =
        animsmith_core::measure::measure_document(&doc, &ResolvedRoles::default(), &config);
    let walk = &measurements["walk"];
    assert_eq!(walk.frame_count, 3);
    assert_eq!(walk.animated_bones, vec!["hips", "spine"]);
    let range = walk.bone_rotation_range_deg["spine"];
    let expected = 1.6f64.to_degrees();
    assert!(
        (range - expected).abs() < 0.1,
        "expected ~{expected}, got {range}"
    );
}
