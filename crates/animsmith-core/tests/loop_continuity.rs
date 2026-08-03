use animsmith_core::model::{
    Bone, Clip, Document, Interpolation, Property, Skeleton, Track, TrackValues, Transform,
};
use animsmith_core::{
    Applicability, CheckCtx, CheckEvaluation, CheckSelection, Config, CoverageGapCode,
    EvaluationScopeCode, EvaluationState, MetricGrids, ResolvedRoles, Value, all_checks,
    evaluate_checks,
};
use glam::{Quat, Vec3};

const TIMES: [f32; 5] = [0.0, 0.25, 0.5, 0.75, 1.0];

fn translation_doc(values: [f32; 5]) -> Document {
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
            tracks: vec![Track {
                bone: 0,
                property: Property::Translation,
                interpolation: Interpolation::Linear,
                times: TIMES.into(),
                values: TrackValues::Vec3s(
                    values.into_iter().map(|x| Vec3::new(x, 0.0, 0.0)).collect(),
                ),
            }],
        }],
        ..Document::default()
    }
}

fn rotation_doc(end_degrees: f32) -> Document {
    let mut doc = translation_doc([0.0; 5]);
    doc.clips[0].tracks.push(rotation_track(
        [0.0, 0.25, 0.5, 0.75, 1.0].map(|fraction| end_degrees * fraction),
    ));
    doc
}

fn rotation_track(degrees: [f32; 5]) -> Track {
    Track {
        bone: 0,
        property: Property::Rotation,
        interpolation: Interpolation::Linear,
        times: TIMES.into(),
        values: TrackValues::Quats(
            degrees
                .into_iter()
                .map(|degrees| Quat::from_rotation_y(degrees.to_radians()))
                .collect(),
        ),
    }
}

fn rotation_steps_doc(degrees: [f32; 5]) -> Document {
    let mut doc = translation_doc([0.0; 5]);
    doc.clips[0].tracks.push(rotation_track(degrees));
    doc
}

fn loop_config() -> Config {
    serde_json::from_value(serde_json::json!({
        "clips": { "guard": { "loop": true } }
    }))
    .expect("loop config")
}

fn evaluate(doc: &Document, config: &Config) -> Vec<CheckEvaluation> {
    let roles = ResolvedRoles::default();
    let grids = MetricGrids::new(doc);
    let ctx = CheckCtx::new(&grids, &roles, config);
    evaluate_checks(&ctx, &all_checks(), CheckSelection::All).expect("valid built-in catalog")
}

fn check<'a>(records: &'a [CheckEvaluation], check_id: &str) -> &'a CheckEvaluation {
    records
        .iter()
        .find(|record| record.check_id() == check_id)
        .unwrap_or_else(|| panic!("missing check {check_id}"))
}

fn number(value: &Option<Value>) -> f64 {
    match value {
        Some(Value::Number(value)) => *value,
        other => panic!("expected numeric finding value, got {other:?}"),
    }
}

#[test]
fn default_loop_caps_are_applied_at_the_boundary() {
    let position_under = translation_doc([0.0, 0.00225, 0.0045, 0.00675, 0.009]);
    assert!(
        check(&evaluate(&position_under, &loop_config()), "loop-closure")
            .findings()
            .is_empty()
    );
    let position_at = translation_doc([0.0, 0.0025, 0.005, 0.0075, 0.01]);
    assert!(
        check(&evaluate(&position_at, &loop_config()), "loop-closure")
            .findings()
            .is_empty()
    );
    let position_over = translation_doc([0.0, 0.00275, 0.0055, 0.00825, 0.011]);
    assert_eq!(
        check(&evaluate(&position_over, &loop_config()), "loop-closure")
            .findings()
            .len(),
        1
    );

    assert!(
        check(
            &evaluate(&rotation_doc(0.9), &loop_config()),
            "loop-closure"
        )
        .findings()
        .is_empty()
    );
    let rotation_at = evaluate(&rotation_doc(1.0), &loop_config());
    let rotation_at = check(&rotation_at, "loop-closure");
    assert!(rotation_at.findings().is_empty(), "{rotation_at:#?}");
    assert_eq!(
        check(
            &evaluate(&rotation_doc(1.1), &loop_config()),
            "loop-closure"
        )
        .findings()
        .len(),
        1
    );

    let velocity_under = translation_doc([0.0, 0.01125, 0.02, 0.01125, 0.0]);
    assert!(
        check(&evaluate(&velocity_under, &loop_config()), "loop-seam-vel")
            .findings()
            .is_empty()
    );
    let velocity_at = translation_doc([0.0, 0.0125, 0.02, 0.0125, 0.0]);
    assert!(
        check(&evaluate(&velocity_at, &loop_config()), "loop-seam-vel")
            .findings()
            .is_empty()
    );
    let velocity_over = translation_doc([0.0, 0.01375, 0.02, 0.01375, 0.0]);
    assert_eq!(
        check(&evaluate(&velocity_over, &loop_config()), "loop-seam-vel")
            .findings()
            .len(),
        1
    );

    // With the 0.25-second seam-adjacent steps, 1.25 degrees is exactly
    // 5 deg/s. The linear metric remains zero for every rotational fixture.
    let angular_under = rotation_steps_doc([0.0, 1.24975, 5.0, 0.0, 0.0]);
    assert!(
        check(&evaluate(&angular_under, &loop_config()), "loop-seam-rot")
            .findings()
            .is_empty()
    );
    let angular_at = rotation_steps_doc([0.0, 1.25, 5.0, 0.0, 0.0]);
    assert!(
        check(&evaluate(&angular_at, &loop_config()), "loop-seam-rot")
            .findings()
            .is_empty()
    );
    let angular_over = rotation_steps_doc([0.0, 1.25025, 5.0, 0.0, 0.0]);
    let angular_over_records = evaluate(&angular_over, &loop_config());
    let angular_over = check(&angular_over_records, "loop-seam-rot");
    assert_eq!(angular_over.findings().len(), 1);
    assert!((number(&angular_over.findings()[0].measured) - 5.001).abs() < 1e-3);
    assert_eq!(number(&angular_over.findings()[0].expected), 5.0);
}

#[test]
fn clean_stationary_loop_is_measured_without_roles_or_stride() {
    let doc = translation_doc([0.0; 5]);
    let mut roles = ResolvedRoles::default();
    roles.profile = "intentionally-unresolved".into();
    let config = loop_config();
    let grids = MetricGrids::new(&doc);
    let measurements = animsmith_core::measure::measure_document(&grids, &roles, &config);
    let continuity = measurements["guard"]
        .loop_continuity
        .as_ref()
        .expect("role-independent loop measurement");
    assert_eq!(continuity.bones.len(), 1);
    let root = &continuity.bones[0];
    assert_eq!(root.bone_index, 0);
    assert_eq!(root.bone_name, "root");
    assert_eq!(root.position_delta_m, 0.0);
    assert_eq!(root.rotation_delta_deg, 0.0);
    assert_eq!(root.seam_velocity_delta_mps, 0.0);
    assert_eq!(root.seam_angular_velocity_delta_degps, 0.0);

    let ctx = CheckCtx::new(&grids, &roles, &config);
    let records =
        evaluate_checks(&ctx, &all_checks(), CheckSelection::All).expect("valid built-in catalog");
    for (id, scope_code) in [
        ("loop-closure", EvaluationScopeCode::LOOP_CLOSURE),
        ("loop-seam-vel", EvaluationScopeCode::LOOP_SEAM_VELOCITY),
        ("loop-seam-rot", EvaluationScopeCode::LOOP_SEAM_ROTATION),
    ] {
        let result = check(&records, id);
        assert_eq!(result.evaluation(), EvaluationState::Complete, "{id}");
        assert!(result.findings().is_empty(), "{id}: {result:#?}");
        assert_eq!(result.evaluated_scopes()[0].code, scope_code);
        assert_eq!(
            result.evaluated_scopes()[0].subject.as_deref(),
            Some("guard")
        );
    }
}

#[test]
fn angular_continuity_catches_a_rotational_cusp_with_no_linear_metric() {
    // The endpoint rotation is C0-closed and all translations are fixed. The
    // +40 deg/s outgoing step and -40 deg/s incoming step still make the
    // rotational derivative discontinuous at playback wrap.
    let doc = rotation_steps_doc([0.0, 10.0, 20.0, 10.0, 0.0]);
    let records = evaluate(&doc, &loop_config());
    assert!(
        check(&records, "loop-closure").findings().is_empty(),
        "the endpoint pose closes"
    );
    assert!(
        check(&records, "loop-seam-vel").findings().is_empty(),
        "fixed translations have no linear seam change"
    );
    let angular = check(&records, "loop-seam-rot");
    assert_eq!(angular.findings().len(), 1, "{angular:#?}");
    assert_eq!(angular.findings()[0].bone.as_deref(), Some("root"));
    assert!(
        angular.findings()[0]
            .message
            .contains("angular velocity changes")
    );

    let grids = MetricGrids::new(&doc);
    let measurements = animsmith_core::measure::measure_document(
        &grids,
        &ResolvedRoles::default(),
        &Config::default(),
    );
    let metric = &measurements["guard"]
        .loop_continuity
        .as_ref()
        .expect("continuity")
        .bones[0];
    assert_eq!(metric.seam_velocity_delta_mps, 0.0);
    assert!((metric.seam_angular_velocity_delta_degps - 80.0).abs() < 1e-4);
}

#[test]
fn angular_continuity_uses_shortest_path_and_quaternion_sign_equivalence() {
    // The 350-to-360 degree incoming step is +10 degrees on the shortest
    // path, matching the 0-to-10 degree outgoing step. Negating an endpoint
    // is the same rotation and must preserve that result.
    let doc = rotation_steps_doc([0.0, 10.0, 180.0, 350.0, 360.0]);
    let grids = MetricGrids::new(&doc);
    let clean = animsmith_core::measure::measure_document(
        &grids,
        &ResolvedRoles::default(),
        &Config::default(),
    );
    let clean_metric = clean["guard"]
        .loop_continuity
        .as_ref()
        .expect("continuity")
        .bones[0]
        .seam_angular_velocity_delta_degps;
    assert!(clean_metric < 1e-3, "{clean_metric}");
    let clean_records = evaluate(&doc, &loop_config());
    let clean_check = check(&clean_records, "loop-seam-rot");
    assert!(clean_check.findings().is_empty(), "{clean_check:#?}");
    assert_eq!(clean_check.evaluation(), EvaluationState::Complete);

    let mut sign_equivalent = doc;
    let TrackValues::Quats(values) = &mut sign_equivalent.clips[0].tracks[1].values else {
        unreachable!()
    };
    values[4] = -values[4];
    let grids = MetricGrids::new(&sign_equivalent);
    let sign_equivalent = animsmith_core::measure::measure_document(
        &grids,
        &ResolvedRoles::default(),
        &Config::default(),
    );
    let sign_metric = sign_equivalent["guard"]
        .loop_continuity
        .as_ref()
        .expect("continuity")
        .bones[0]
        .seam_angular_velocity_delta_degps;
    assert!((sign_metric - clean_metric).abs() < 1e-4, "{sign_metric}");
}

#[test]
fn angular_continuity_compares_rotation_vectors_and_uses_the_actual_timestep() {
    // Over a 0.5-second seam step, the outgoing velocity is +20 deg/s about
    // model X and the incoming velocity is +20 deg/s about model Y. Their
    // magnitudes match, but the vector delta is 20*sqrt(2) deg/s. A scalar
    // angle-only comparison would incorrectly report zero; a fixed 0.25 s
    // divisor would incorrectly report twice the expected value.
    let mut doc = translation_doc([0.0; 5]);
    doc.clips[0].duration_s = 2.0;
    let scaled_times = vec![0.0, 0.5, 1.0, 1.5, 2.0];
    doc.clips[0].tracks[0].times = scaled_times.clone();
    doc.clips[0].tracks.push(Track {
        bone: 0,
        property: Property::Rotation,
        interpolation: Interpolation::Linear,
        times: scaled_times,
        values: TrackValues::Quats(vec![
            Quat::IDENTITY,
            Quat::from_rotation_x(10.0f32.to_radians()),
            Quat::from_rotation_x(20.0f32.to_radians()),
            Quat::from_rotation_y(-10.0f32.to_radians()),
            Quat::IDENTITY,
        ]),
    });

    let grids = MetricGrids::new(&doc);
    let measurements = animsmith_core::measure::measure_document(
        &grids,
        &ResolvedRoles::default(),
        &Config::default(),
    );
    let metric = &measurements["guard"]
        .loop_continuity
        .as_ref()
        .expect("continuity")
        .bones[0];
    assert_eq!(metric.rotation_delta_deg, 0.0);
    assert_eq!(metric.seam_velocity_delta_mps, 0.0);
    let expected = 20.0 * std::f64::consts::SQRT_2;
    assert!(
        (metric.seam_angular_velocity_delta_degps - expected).abs() < 1e-4,
        "{metric:#?}"
    );

    let records = evaluate(&doc, &loop_config());
    let result = check(&records, "loop-seam-rot");
    assert_eq!(result.findings().len(), 1, "{result:#?}");
    assert!(
        (number(&result.findings()[0].measured) - expected).abs() < 1e-4,
        "{result:#?}"
    );
}

#[test]
fn angular_check_reports_the_maximum_bone_and_preserves_per_bone_evidence() {
    let mut doc = translation_doc([0.0; 5]);
    doc.skeleton.bones.push(Bone {
        name: "second-root".into(),
        parent: None,
        rest: Transform::IDENTITY,
        inverse_bind: None,
    });
    doc.clips[0]
        .tracks
        .push(rotation_track([0.0, 1.0, 2.0, 1.0, 0.0]));
    let mut second_rotation = rotation_track([0.0, 10.0, 20.0, 10.0, 0.0]);
    second_rotation.bone = 1;
    doc.clips[0].tracks.push(second_rotation);

    let grids = MetricGrids::new(&doc);
    let measurements = animsmith_core::measure::measure_document(
        &grids,
        &ResolvedRoles::default(),
        &Config::default(),
    );
    let bones = &measurements["guard"]
        .loop_continuity
        .as_ref()
        .expect("continuity")
        .bones;
    assert_eq!(
        (bones[0].bone_index, bones[0].bone_name.as_str()),
        (0, "root")
    );
    assert_eq!(
        (bones[1].bone_index, bones[1].bone_name.as_str()),
        (1, "second-root")
    );
    assert!((bones[0].seam_angular_velocity_delta_degps - 8.0).abs() < 1e-4);
    assert!((bones[1].seam_angular_velocity_delta_degps - 80.0).abs() < 1e-4);

    let records = evaluate(&doc, &loop_config());
    let result = check(&records, "loop-seam-rot");
    assert_eq!(result.findings().len(), 1, "{result:#?}");
    assert_eq!(result.findings()[0].bone.as_deref(), Some("second-root"));
    assert_eq!(
        number(&result.findings()[0].measured),
        bones[1].seam_angular_velocity_delta_degps
    );
    assert_eq!(number(&result.findings()[0].expected), 5.0);
}

#[test]
fn broken_position_closure_can_still_have_continuous_velocity() {
    // Constant 1 m/s translation: the last pose is 1 m from the first, while
    // the derivative entering and leaving the endpoint is identical.
    let doc = translation_doc([0.0, 0.25, 0.5, 0.75, 1.0]);
    let records = evaluate(&doc, &loop_config());

    let closure = check(&records, "loop-closure");
    assert_eq!(closure.findings().len(), 1, "{closure:#?}");
    assert_eq!(closure.findings()[0].bone.as_deref(), Some("root"));
    assert!(
        closure.findings()[0]
            .message
            .contains("does not close in position")
    );

    let velocity = check(&records, "loop-seam-vel");
    assert!(velocity.findings().is_empty(), "{velocity:#?}");
    assert_eq!(velocity.evaluation(), EvaluationState::Complete);
}

#[test]
fn closed_pose_with_a_velocity_cusp_fails_only_c1() {
    // First/last position is exactly zero, but the outgoing velocity is
    // +1 m/s and the incoming velocity is -1 m/s.
    let doc = translation_doc([0.0, 0.25, 0.5, 0.25, 0.0]);
    let records = evaluate(&doc, &loop_config());

    let closure = check(&records, "loop-closure");
    assert!(closure.findings().is_empty(), "{closure:#?}");

    let velocity = check(&records, "loop-seam-vel");
    assert_eq!(velocity.findings().len(), 1, "{velocity:#?}");
    assert_eq!(velocity.findings()[0].bone.as_deref(), Some("root"));
    assert!(velocity.findings()[0].message.contains("velocity changes"));

    let roles = ResolvedRoles::default();
    let grids = MetricGrids::new(&doc);
    let measurements =
        animsmith_core::measure::measure_document(&grids, &roles, &Config::default());
    let metric = &measurements["guard"]
        .loop_continuity
        .as_ref()
        .expect("continuity")
        .bones[0];
    assert_eq!(metric.position_delta_m, 0.0);
    assert!((metric.seam_velocity_delta_mps - 2.0).abs() < 1e-6);
}

#[test]
fn seam_velocity_uses_independent_incoming_and_outgoing_steps() {
    // Outgoing is +0.1 m/s and incoming is -0.2 m/s. Keeping the slopes
    // asymmetric distinguishes the C1 definition from a symmetric cusp
    // shortcut while the endpoint pose remains exactly closed.
    let doc = translation_doc([0.0, 0.025, 0.08, 0.05, 0.0]);
    let roles = ResolvedRoles::default();
    let grids = MetricGrids::new(&doc);
    let measurements =
        animsmith_core::measure::measure_document(&grids, &roles, &Config::default());
    let metric = &measurements["guard"]
        .loop_continuity
        .as_ref()
        .expect("continuity")
        .bones[0];
    assert_eq!(metric.position_delta_m, 0.0);
    assert!((metric.seam_velocity_delta_mps - 0.3).abs() < 1e-6);
}

#[test]
fn rotation_closure_uses_shortest_path_model_space_delta() {
    let doc = rotation_doc(10.0);
    let records = evaluate(&doc, &loop_config());
    let closure = check(&records, "loop-closure");
    assert_eq!(closure.findings().len(), 1, "{closure:#?}");
    assert!(
        closure.findings()[0]
            .message
            .contains("does not close in rotation")
    );
    let roles = ResolvedRoles::default();
    let grids = MetricGrids::new(&doc);
    let measurements =
        animsmith_core::measure::measure_document(&grids, &roles, &Config::default());
    assert!(
        (measurements["guard"]
            .loop_continuity
            .as_ref()
            .unwrap()
            .bones[0]
            .rotation_delta_deg
            - 10.0)
            .abs()
            < 1e-5
    );

    let long_path = rotation_doc(350.0);
    let grids = MetricGrids::new(&long_path);
    let measurements =
        animsmith_core::measure::measure_document(&grids, &roles, &Config::default());
    assert!(
        (measurements["guard"]
            .loop_continuity
            .as_ref()
            .unwrap()
            .bones[0]
            .rotation_delta_deg
            - 10.0)
            .abs()
            < 1e-4,
        "350 degrees must measure as the 10-degree shortest path"
    );

    let mut sign_equivalent = rotation_doc(0.0);
    let TrackValues::Quats(values) = &mut sign_equivalent.clips[0].tracks[1].values else {
        unreachable!()
    };
    values[4] = -Quat::IDENTITY;
    let grids = MetricGrids::new(&sign_equivalent);
    let measurements =
        animsmith_core::measure::measure_document(&grids, &roles, &Config::default());
    assert_eq!(
        measurements["guard"]
            .loop_continuity
            .as_ref()
            .unwrap()
            .bones[0]
            .rotation_delta_deg,
        0.0
    );
}

#[test]
fn configurable_caps_control_each_check() {
    let position = translation_doc([0.0, 0.25, 0.5, 0.75, 1.0]);
    let position_config: Config = serde_json::from_value(serde_json::json!({
        "clips": { "guard": { "loop": true } },
        "checks": { "loop-closure": {
            "max_position_delta_m": 1.1,
            "max_rotation_delta_deg": 1.0
        } }
    }))
    .expect("closure config");
    assert!(
        check(&evaluate(&position, &position_config), "loop-closure")
            .findings()
            .is_empty()
    );

    let cusp = translation_doc([0.0, 0.25, 0.5, 0.25, 0.0]);
    let velocity_config: Config = serde_json::from_value(serde_json::json!({
        "clips": { "guard": { "loop": true } },
        "checks": { "loop-seam-vel": { "max_velocity_delta_mps": 2.1 } }
    }))
    .expect("velocity config");
    assert!(
        check(&evaluate(&cusp, &velocity_config), "loop-seam-vel")
            .findings()
            .is_empty()
    );

    let angular_cusp = rotation_steps_doc([0.0, 10.0, 20.0, 10.0, 0.0]);
    let angular_config: Config = serde_json::from_value(serde_json::json!({
        "clips": { "guard": { "loop": true } },
        "checks": { "loop-seam-rot": { "max_angular_velocity_delta_degps": 80.1 } }
    }))
    .expect("angular velocity config");
    assert!(
        check(&evaluate(&angular_cusp, &angular_config), "loop-seam-rot")
            .findings()
            .is_empty()
    );
}

#[test]
fn undeclared_and_too_short_clips_keep_typed_coverage_semantics() {
    let doc = translation_doc([0.0; 5]);
    let records = evaluate(&doc, &Config::default());
    for id in ["loop-closure", "loop-seam-vel", "loop-seam-rot"] {
        let result = check(&records, id);
        assert_eq!(result.applicability(), Applicability::NotApplicable);
        assert_eq!(result.evaluation(), EvaluationState::NotEvaluated);
        assert!(result.findings().is_empty());
        assert!(result.gaps().is_empty());
    }

    let mut short = doc;
    short.clips[0].tracks[0].times = vec![0.0, 1.0];
    short.clips[0].tracks[0].values = TrackValues::Vec3s(vec![Vec3::ZERO, Vec3::ZERO]);
    let records = evaluate(&short, &loop_config());
    for id in ["loop-closure", "loop-seam-vel", "loop-seam-rot"] {
        let result = check(&records, id);
        assert_eq!(result.evaluation(), EvaluationState::NotEvaluated);
        assert!(result.findings().is_empty(), "{id}: {result:#?}");
        assert_eq!(result.gaps().len(), 1);
        assert_eq!(
            result.gaps()[0].code,
            CoverageGapCode::MEASUREMENT_UNAVAILABLE
        );
        assert_eq!(
            result.gaps()[0].scope.as_ref().unwrap().subject.as_deref(),
            Some("guard")
        );
    }
}

#[test]
fn non_finite_model_evidence_is_a_typed_gap_not_a_loop_finding() {
    for values in [
        [0.0, f32::NAN, 0.0, 0.0, 0.0],
        [0.0, 0.0, 0.0, 0.0, f32::INFINITY],
    ] {
        let records = evaluate(&translation_doc(values), &loop_config());
        for id in ["loop-closure", "loop-seam-vel", "loop-seam-rot"] {
            let result = check(&records, id);
            assert_eq!(result.evaluation(), EvaluationState::NotEvaluated);
            assert!(result.findings().is_empty(), "{result:#?}");
            assert_eq!(result.gaps().len(), 1);
            assert_eq!(
                result.gaps()[0].code,
                CoverageGapCode::MEASUREMENT_UNAVAILABLE
            );
        }
    }

    for (case, invalid) in [
        ("nan", Quat::from_xyzw(f32::NAN, 0.0, 0.0, 1.0)),
        ("infinite", Quat::from_xyzw(f32::INFINITY, 0.0, 0.0, 1.0)),
        ("zero-length", Quat::from_xyzw(0.0, 0.0, 0.0, 0.0)),
    ] {
        for frame in [0, 1, 3, 4] {
            let mut invalid_rotation = rotation_steps_doc([0.0, 10.0, 20.0, 10.0, 0.0]);
            let TrackValues::Quats(values) = &mut invalid_rotation.clips[0].tracks[1].values else {
                unreachable!()
            };
            values[frame] = invalid;
            let records = evaluate(&invalid_rotation, &loop_config());
            let angular = check(&records, "loop-seam-rot");
            assert_eq!(
                angular.evaluation(),
                EvaluationState::NotEvaluated,
                "{case} frame {frame}"
            );
            assert!(
                angular.findings().is_empty(),
                "{case} frame {frame}: {angular:#?}"
            );
            assert_eq!(angular.gaps().len(), 1, "{case} frame {frame}");
            assert_eq!(
                angular.gaps()[0].code,
                CoverageGapCode::MEASUREMENT_UNAVAILABLE
            );
        }
    }
}

#[test]
fn measurements_keep_indices_when_bone_names_repeat() {
    let mut doc = translation_doc([0.0; 5]);
    doc.skeleton.bones.push(Bone {
        name: "root".into(),
        parent: Some(0),
        rest: Transform {
            translation: Vec3::Y,
            ..Transform::IDENTITY
        },
        inverse_bind: None,
    });
    doc.skeleton.bones.push(Bone {
        name: "root".into(),
        parent: Some(1),
        rest: Transform {
            translation: Vec3::Z,
            ..Transform::IDENTITY
        },
        inverse_bind: None,
    });
    let roles = ResolvedRoles::default();
    let grids = MetricGrids::new(&doc);
    let measurements =
        animsmith_core::measure::measure_document(&grids, &roles, &Config::default());
    let bones = &measurements["guard"]
        .loop_continuity
        .as_ref()
        .expect("continuity")
        .bones;
    assert_eq!(bones.len(), 3);
    assert_eq!(
        (bones[0].bone_index, bones[0].bone_name.as_str()),
        (0, "root")
    );
    assert_eq!(
        (bones[1].bone_index, bones[1].bone_name.as_str()),
        (1, "root")
    );
    assert_eq!(
        (bones[2].bone_index, bones[2].bone_name.as_str()),
        (2, "root")
    );
}

#[test]
fn model_rotation_is_composed_independently_of_non_uniform_scale() {
    let mut doc = rotation_doc(10.0);
    doc.skeleton.bones[0].rest.scale = Vec3::new(2.0, 1.0, 0.5);
    doc.skeleton.bones.push(Bone {
        name: "child".into(),
        parent: Some(0),
        rest: Transform::IDENTITY,
        inverse_bind: None,
    });

    let roles = ResolvedRoles::default();
    let grids = MetricGrids::new(&doc);
    let measurements =
        animsmith_core::measure::measure_document(&grids, &roles, &Config::default());
    let bones = &measurements["guard"]
        .loop_continuity
        .as_ref()
        .expect("continuity")
        .bones;
    assert!(
        (bones[1].rotation_delta_deg - 10.0).abs() < 1e-3,
        "{bones:#?}"
    );
}

#[test]
fn child_metrics_include_parent_driven_model_space_c0_and_c1_motion() {
    let mut closure_doc = rotation_doc(10.0);
    closure_doc.skeleton.bones.push(Bone {
        name: "child".into(),
        parent: Some(0),
        rest: Transform {
            translation: Vec3::X,
            ..Transform::IDENTITY
        },
        inverse_bind: None,
    });
    let closure_records = evaluate(&closure_doc, &loop_config());
    let closure = check(&closure_records, "loop-closure");
    let inherited_position = closure
        .findings()
        .iter()
        .find(|finding| finding.message.contains("does not close in position"))
        .expect("parent rotation moves the locally static child in model space");
    assert_eq!(inherited_position.bone.as_deref(), Some("child"));

    let mut cusp_doc = translation_doc([0.0; 5]);
    cusp_doc.skeleton.bones.push(Bone {
        name: "child".into(),
        parent: Some(0),
        rest: Transform {
            translation: Vec3::X,
            ..Transform::IDENTITY
        },
        inverse_bind: None,
    });
    cusp_doc.clips[0].tracks.push(Track {
        bone: 0,
        property: Property::Rotation,
        interpolation: Interpolation::Linear,
        times: TIMES.into(),
        values: TrackValues::Quats(
            [0.0f32, 0.1, 0.2, 0.1, 0.0]
                .into_iter()
                .map(Quat::from_rotation_z)
                .collect(),
        ),
    });
    let cusp_records = evaluate(&cusp_doc, &loop_config());
    let velocity = check(&cusp_records, "loop-seam-vel");
    assert_eq!(velocity.findings().len(), 1, "{velocity:#?}");
    assert_eq!(velocity.findings()[0].bone.as_deref(), Some("child"));

    let roles = ResolvedRoles::default();
    let grids = MetricGrids::new(&cusp_doc);
    let measurements =
        animsmith_core::measure::measure_document(&grids, &roles, &Config::default());
    let bones = &measurements["guard"]
        .loop_continuity
        .as_ref()
        .expect("continuity")
        .bones;
    assert_eq!(bones[0].seam_velocity_delta_mps, 0.0);
    assert!(bones[1].seam_velocity_delta_mps > 0.1);

    let angular_cusp_doc = {
        let mut doc = translation_doc([0.0; 5]);
        doc.skeleton.bones.push(Bone {
            name: "angular-parent".into(),
            parent: Some(0),
            rest: Transform::IDENTITY,
            inverse_bind: None,
        });
        doc.skeleton.bones.push(Bone {
            name: "angular-child".into(),
            parent: Some(1),
            rest: Transform {
                rotation: Quat::from_rotation_x(0.5),
                ..Transform::IDENTITY
            },
            inverse_bind: None,
        });
        let mut parent_rotation = rotation_track([0.0, 10.0, 20.0, 10.0, 0.0]);
        parent_rotation.bone = 1;
        doc.clips[0].tracks.push(parent_rotation);
        doc
    };
    let grids = MetricGrids::new(&angular_cusp_doc);
    let angular_measurements = animsmith_core::measure::measure_document(
        &grids,
        &ResolvedRoles::default(),
        &Config::default(),
    );
    let angular_bones = &angular_measurements["guard"]
        .loop_continuity
        .as_ref()
        .expect("continuity")
        .bones;
    assert_eq!(angular_bones[0].seam_angular_velocity_delta_degps, 0.0);
    assert!(angular_bones[1].seam_angular_velocity_delta_degps > 79.9);
    assert!(angular_bones[2].seam_angular_velocity_delta_degps > 79.9);
}
