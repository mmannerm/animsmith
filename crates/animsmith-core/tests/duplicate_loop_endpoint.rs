use animsmith_core::model::{
    Bone, Clip, Document, Interpolation, Property, Skeleton, Track, TrackValues, Transform,
};
use animsmith_core::transform::{
    DUPLICATE_ENDPOINT_QUATERNION_TOLERANCE_RAD, DUPLICATE_ENDPOINT_VEC3_TOLERANCE,
    DuplicateLoopEndpointError, analyze_duplicate_loop_endpoint,
};
use animsmith_core::{
    Applicability, CheckCtx, CheckEvaluation, CheckSelection, Config, CoverageGapCode,
    EvaluationScopeCode, EvaluationState, MetricGrids, ResolvedRoles, Severity, all_checks,
    evaluate_checks,
};
use glam::{Quat, Vec3};

fn track(bone: usize, times: &[f32], values: &[f32]) -> Track {
    Track {
        bone,
        property: Property::Translation,
        interpolation: Interpolation::Linear,
        times: times.to_vec(),
        values: TrackValues::Vec3s(values.iter().map(|&x| Vec3::new(x, 0.0, 0.0)).collect()),
    }
}

fn document(values: &[f32]) -> Document {
    let times = [0.0, 0.5, 1.0];
    Document {
        skeleton: Skeleton {
            bones: vec![Bone {
                name: "root".into(),
                parent: None,
                rest: Transform::IDENTITY,
                inverse_bind: None,
            }],
        },
        clips: vec![Clip {
            name: "guard".into(),
            duration_s: 1.0,
            tracks: vec![track(0, &times, values)],
        }],
        ..Document::default()
    }
}

fn config(looping: Option<bool>) -> Config {
    match looping {
        Some(looping) => serde_json::from_value(serde_json::json!({
            "clips": { "guard": { "loop": looping } }
        }))
        .expect("loop config"),
        None => Config::default(),
    }
}

fn evaluate(doc: &Document, config: &Config) -> Vec<CheckEvaluation> {
    let roles = ResolvedRoles::default();
    let grids = MetricGrids::new(doc);
    let ctx = CheckCtx::new(&grids, &roles, config);
    evaluate_checks(&ctx, &all_checks(), CheckSelection::All).expect("valid built-in catalog")
}

fn check(records: &[CheckEvaluation]) -> &CheckEvaluation {
    records
        .iter()
        .find(|record| record.check_id() == "duplicate-loop-endpoint")
        .expect("duplicate-loop-endpoint record")
}

#[test]
fn duplicate_endpoint_warns_with_completed_authored_scope() {
    let records = evaluate(&document(&[0.0, 1.0, 0.0]), &config(Some(true)));
    let duplicate = check(&records);

    assert_eq!(duplicate.applicability(), Applicability::Applicable);
    assert_eq!(duplicate.evaluation(), EvaluationState::Complete);
    assert_eq!(duplicate.findings().len(), 1);
    assert_eq!(duplicate.findings()[0].severity, Severity::Warning);
    assert_eq!(duplicate.findings()[0].clip.as_deref(), Some("guard"));
    assert!(
        duplicate.findings()[0]
            .message
            .contains("open-cycle representation")
    );
    assert_eq!(duplicate.evaluated_scopes().len(), 1);
    assert_eq!(
        duplicate.evaluated_scopes()[0].code,
        EvaluationScopeCode::DUPLICATE_LOOP_ENDPOINT
    );
    assert_eq!(
        duplicate.evaluated_scopes()[0].subject.as_deref(),
        Some("guard")
    );
    assert!(duplicate.gaps().is_empty());
}

#[test]
fn open_cycle_and_stationary_hold_are_clean_completed_analysis() {
    for values in [&[0.0, 1.0, 2.0][..], &[0.0, 0.0, 0.0][..]] {
        let records = evaluate(&document(values), &config(Some(true)));
        let duplicate = check(&records);
        assert_eq!(duplicate.evaluation(), EvaluationState::Complete);
        assert!(duplicate.findings().is_empty(), "{duplicate:#?}");
        assert_eq!(duplicate.evaluated_scopes().len(), 1);
        assert!(duplicate.gaps().is_empty());
    }
}

#[test]
fn undeclared_loop_is_not_applicable() {
    let records = evaluate(&document(&[0.0, 1.0, 0.0]), &config(None));
    let duplicate = check(&records);
    assert_eq!(duplicate.applicability(), Applicability::NotApplicable);
    assert!(duplicate.findings().is_empty());
    assert!(duplicate.evaluated_scopes().is_empty());
    assert!(duplicate.gaps().is_empty());
}

#[test]
fn incompatible_authored_timelines_report_typed_coverage_gap() {
    let mut doc = document(&[0.0, 1.0, 0.0]);
    doc.clips[0]
        .tracks
        .push(track(0, &[0.0, 0.4, 1.0], &[0.0, 2.0, 0.0]));
    let records = evaluate(&doc, &config(Some(true)));
    let duplicate = check(&records);

    assert_eq!(duplicate.evaluation(), EvaluationState::NotEvaluated);
    assert!(duplicate.findings().is_empty());
    assert!(duplicate.evaluated_scopes().is_empty());
    assert_eq!(duplicate.gaps().len(), 1);
    assert_eq!(
        duplicate.gaps()[0].code,
        CoverageGapCode::MEASUREMENT_UNAVAILABLE
    );
    let scope = duplicate.gaps()[0].scope.as_ref().expect("gap scope");
    assert_eq!(scope.code, EvaluationScopeCode::DUPLICATE_LOOP_ENDPOINT);
    assert_eq!(scope.subject.as_deref(), Some("guard"));
}

#[test]
fn fixed_endpoint_tolerances_and_quaternion_sign_equivalence_are_enforced() {
    let mut clip = document(&[0.0, 1.0, 0.9 * DUPLICATE_ENDPOINT_VEC3_TOLERANCE])
        .clips
        .remove(0);
    clip.tracks[0].values = TrackValues::Vec3s(vec![
        Vec3::ZERO,
        Vec3::X,
        Vec3::splat(0.9 * DUPLICATE_ENDPOINT_VEC3_TOLERANCE),
    ]);
    let vector_outcome = analyze_duplicate_loop_endpoint(&clip)
        .unwrap()
        .expect("vector just inside the documented tolerance is removable");
    assert_eq!(
        vector_outcome.max_translation_endpoint_delta_m,
        Some(0.9 * DUPLICATE_ENDPOINT_VEC3_TOLERANCE)
    );
    assert_eq!(vector_outcome.max_rotation_endpoint_delta_rad, None);
    let TrackValues::Vec3s(values) = &mut clip.tracks[0].values else {
        unreachable!()
    };
    values[2] = Vec3::splat(1.1 * DUPLICATE_ENDPOINT_VEC3_TOLERANCE);
    assert_eq!(analyze_duplicate_loop_endpoint(&clip).unwrap(), None);

    clip.tracks[0] = Track {
        bone: 0,
        property: Property::Rotation,
        interpolation: Interpolation::Linear,
        times: vec![0.0, 0.5, 1.0],
        values: TrackValues::Quats(vec![
            Quat::IDENTITY,
            Quat::from_rotation_y(0.5),
            -Quat::from_rotation_y(0.9 * DUPLICATE_ENDPOINT_QUATERNION_TOLERANCE_RAD),
        ]),
    };
    let rotation_outcome = analyze_duplicate_loop_endpoint(&clip)
        .unwrap()
        .expect("antipodal quaternion inside the angular tolerance is removable");
    let measured_rotation = rotation_outcome
        .max_rotation_endpoint_delta_rad
        .expect("rotation delta");
    assert!((measured_rotation - 0.9 * DUPLICATE_ENDPOINT_QUATERNION_TOLERANCE_RAD).abs() < 1.0e-7);
    let TrackValues::Quats(values) = &mut clip.tracks[0].values else {
        unreachable!()
    };
    values[2] = Quat::from_rotation_y(1.1 * DUPLICATE_ENDPOINT_QUATERNION_TOLERANCE_RAD);
    assert_eq!(analyze_duplicate_loop_endpoint(&clip).unwrap(), None);
}

#[test]
fn different_duplicate_counts_and_stale_duration_refuse_the_atomic_edit() {
    let mut doc = document(&[0.0, 1.0, 0.0]);
    doc.clips[0].tracks[0].times = vec![0.0, 0.5, 0.75, 1.0];
    doc.clips[0].tracks[0].values =
        TrackValues::Vec3s(vec![Vec3::ZERO, Vec3::X, Vec3::ZERO, Vec3::ZERO]);
    doc.clips[0]
        .tracks
        .push(track(0, &[0.0, 0.5, 0.75, 1.0], &[0.0, 2.0, 1.0, 0.0]));
    assert_eq!(
        analyze_duplicate_loop_endpoint(&doc.clips[0]).unwrap(),
        None,
        "all tracks must prove one common atomic removal count"
    );

    doc.clips[0].tracks.truncate(1);
    doc.clips[0].duration_s = 1.25;
    assert!(matches!(
        analyze_duplicate_loop_endpoint(&doc.clips[0]),
        Err(DuplicateLoopEndpointError::DurationMismatch { .. })
    ));
}

#[test]
fn constant_companion_tracks_do_not_block_a_moving_loop_candidate() {
    let mut doc = document(&[0.0, 1.0, 0.0]);
    doc.clips[0].tracks[0].times = vec![0.0, 0.5, 0.75, 1.0];
    doc.clips[0].tracks[0].values =
        TrackValues::Vec3s(vec![Vec3::ZERO, Vec3::X, Vec3::X, Vec3::ZERO]);
    doc.clips[0]
        .tracks
        .push(track(0, &[0.0, 0.5, 0.75, 1.0], &[2.0, 2.0, 2.0, 2.0]));

    let outcome = analyze_duplicate_loop_endpoint(&doc.clips[0])
        .unwrap()
        .expect("a held channel is invariant under the moving channel's trim");
    assert_eq!(outcome.removed_keys_per_track, 1);
}
