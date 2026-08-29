use animsmith_core::measure::MeasurementAvailability;
use animsmith_core::model::{
    Bone, Clip, Document, Interpolation, Property, Skeleton, Track, TrackValues, Transform,
};
use animsmith_core::{
    Applicability, CheckCtx, CheckEvaluation, CheckSelection, ClipExpectations, Config,
    ConfigValidationError, CoverageGapCode, EvaluationError, EvaluationScopeCode, EvaluationState,
    MetricGrids, ResolvedRoles, Value, all_checks, evaluate_checks,
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
    assert_eq!(root.availability, MeasurementAvailability::Measured);
    assert_eq!(root.position_delta_m, Some(0.0));
    assert_eq!(root.rotation_delta_deg, Some(0.0));
    assert_eq!(root.seam_velocity_delta_mps, Some(0.0));
    assert_eq!(root.seam_angular_velocity_delta_degps, Some(0.0));

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
    assert_eq!(metric.seam_velocity_delta_mps, Some(0.0));
    assert!((metric.seam_angular_velocity_delta_degps.unwrap() - 80.0).abs() < 1e-4);
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
        .seam_angular_velocity_delta_degps
        .unwrap();
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
        .seam_angular_velocity_delta_degps
        .unwrap();
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
    assert_eq!(metric.rotation_delta_deg, Some(0.0));
    assert_eq!(metric.seam_velocity_delta_mps, Some(0.0));
    let expected = 20.0 * std::f64::consts::SQRT_2;
    assert!(
        (metric.seam_angular_velocity_delta_degps.unwrap() - expected).abs() < 1e-4,
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
    assert!((bones[0].seam_angular_velocity_delta_degps.unwrap() - 8.0).abs() < 1e-4);
    assert!((bones[1].seam_angular_velocity_delta_degps.unwrap() - 80.0).abs() < 1e-4);

    let records = evaluate(&doc, &loop_config());
    let result = check(&records, "loop-seam-rot");
    assert_eq!(result.findings().len(), 1, "{result:#?}");
    assert_eq!(result.findings()[0].bone.as_deref(), Some("second-root"));
    assert_eq!(
        number(&result.findings()[0].measured),
        bones[1].seam_angular_velocity_delta_degps.unwrap()
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
    assert_eq!(metric.position_delta_m, Some(0.0));
    assert!((metric.seam_velocity_delta_mps.unwrap() - 2.0).abs() < 1e-6);
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
    assert_eq!(metric.position_delta_m, Some(0.0));
    assert!((metric.seam_velocity_delta_mps.unwrap() - 0.3).abs() < 1e-6);
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
            .unwrap()
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
            .unwrap()
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
        Some(0.0)
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
fn exact_clip_loop_caps_report_effective_values_and_global_velocity_fallback() {
    let mut doc = translation_doc([0.0, 0.005, 0.01, 0.015, 0.02]);
    doc.clips[0].name = "position_exact".into();

    let mut rotation_exact = rotation_doc(5.0).clips.remove(0);
    rotation_exact.name = "rotation_exact".into();
    doc.clips.push(rotation_exact);

    let mut velocity_exact = translation_doc([0.0, 0.25, 0.5, 0.25, 0.0]).clips.remove(0);
    velocity_exact.name = "velocity_exact".into();
    doc.clips.push(velocity_exact);

    let mut velocity_global = translation_doc([0.0, 0.25, 0.5, 0.25, 0.0]).clips.remove(0);
    velocity_global.name = "velocity_global".into();
    doc.clips.push(velocity_global);

    let config: Config = serde_json::from_value(serde_json::json!({
        "checks": {
            "loop-closure": {
                "max_position_delta_m": 0.1,
                "max_rotation_delta_deg": 10.0
            },
            "loop-seam-vel": { "max_velocity_delta_mps": 1.8 }
        },
        "clips": {
            "position_exact": { "loop": true, "max_loop_position_delta_m": 0.015 },
            "rotation_exact": { "loop": true, "max_loop_rotation_delta_deg": 4.0 },
            "velocity_exact": { "loop": true, "max_loop_velocity_delta_mps": 1.9 },
            "velocity_global": { "loop": true }
        }
    }))
    .expect("clip-scoped loop caps config");

    let records = evaluate(&doc, &config);
    let closure = check(&records, "loop-closure");
    assert_eq!(closure.findings().len(), 2, "{closure:#?}");
    assert!(closure.findings().iter().any(|finding| {
        finding.clip.as_deref() == Some("position_exact") && number(&finding.expected) == 0.015
    }));
    assert!(closure.findings().iter().any(|finding| {
        finding.clip.as_deref() == Some("rotation_exact") && number(&finding.expected) == 4.0
    }));

    let velocity = check(&records, "loop-seam-vel");
    assert_eq!(velocity.findings().len(), 2, "{velocity:#?}");
    assert!(velocity.findings().iter().any(|finding| {
        finding.clip.as_deref() == Some("velocity_exact") && number(&finding.expected) == 1.9
    }));
    assert!(velocity.findings().iter().any(|finding| {
        finding.clip.as_deref() == Some("velocity_global") && number(&finding.expected) == 1.8
    }));
}

#[test]
fn angular_clip_caps_layer_global_glob_and_exact_contracts() {
    let mut doc = rotation_steps_doc([0.0, 10.0, 20.0, 10.0, 0.0]);
    doc.clips[0].name = "turn_family".into();
    let mut exact = doc.clips[0].clone();
    exact.name = "turn_exact".into();
    doc.clips.push(exact);
    let mut global = doc.clips[0].clone();
    global.name = "plain_turn".into();
    doc.clips.push(global);

    let config: Config = serde_json::from_value(serde_json::json!({
        "checks": {
            "loop-seam-rot": { "max_angular_velocity_delta_degps": 70.0 }
        },
        "clips": {
            "turn_*": {
                "loop": true,
                "max_loop_angular_velocity_delta_degps": 79.0
            },
            "turn_exact": { "max_loop_angular_velocity_delta_degps": 78.0 },
            "plain_turn": { "loop": true }
        }
    }))
    .expect("angular clip caps config");

    let records = evaluate(&doc, &config);
    let angular = check(&records, "loop-seam-rot");
    assert_eq!(angular.findings().len(), 3, "{angular:#?}");
    assert!(angular.findings().iter().any(|finding| {
        finding.clip.as_deref() == Some("plain_turn") && number(&finding.expected) == 70.0
    }));
    assert!(angular.findings().iter().any(|finding| {
        finding.clip.as_deref() == Some("turn_family") && number(&finding.expected) == 79.0
    }));
    assert!(angular.findings().iter().any(|finding| {
        finding.clip.as_deref() == Some("turn_exact") && number(&finding.expected) == 78.0
    }));
}

#[test]
fn every_clip_loop_cap_accepts_zero_and_rejects_negative_or_non_finite_values() {
    for field in [
        "max_loop_position_delta_m",
        "max_loop_rotation_delta_deg",
        "max_loop_velocity_delta_mps",
        "max_loop_angular_velocity_delta_degps",
    ] {
        for value in ["-0.01", "nan", "inf", "-inf"] {
            let config = format!("[clips.guard]\n{field} = {value}\n");
            assert!(
                toml::from_str::<Config>(&config).is_err(),
                "accepted invalid {field}={value}"
            );
        }

        let config: Config = toml::from_str(&format!("[clips.guard]\n{field} = 0.0\n"))
            .unwrap_or_else(|error| panic!("rejected zero {field}: {error}"));
        let expectations = config.expectations_for("guard");
        let value = match field {
            "max_loop_position_delta_m" => expectations.max_loop_position_delta_m,
            "max_loop_rotation_delta_deg" => expectations.max_loop_rotation_delta_deg,
            "max_loop_velocity_delta_mps" => expectations.max_loop_velocity_delta_mps,
            "max_loop_angular_velocity_delta_degps" => {
                expectations.max_loop_angular_velocity_delta_degps
            }
            _ => unreachable!(),
        };
        assert_eq!(value, Some(0.0), "did not preserve zero {field}");
    }
}

#[test]
fn programmatic_clip_loop_caps_fail_closed_in_validation_and_evaluation() {
    let doc = translation_doc([0.0; 5]);
    let roles = ResolvedRoles::default();
    let grids = MetricGrids::new(&doc);

    for field in [
        "max_loop_position_delta_m",
        "max_loop_rotation_delta_deg",
        "max_loop_velocity_delta_mps",
        "max_loop_angular_velocity_delta_degps",
    ] {
        for value in [-0.01, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut expectations = ClipExpectations::default();
            match field {
                "max_loop_position_delta_m" => expectations.max_loop_position_delta_m = Some(value),
                "max_loop_rotation_delta_deg" => {
                    expectations.max_loop_rotation_delta_deg = Some(value)
                }
                "max_loop_velocity_delta_mps" => {
                    expectations.max_loop_velocity_delta_mps = Some(value)
                }
                "max_loop_angular_velocity_delta_degps" => {
                    expectations.max_loop_angular_velocity_delta_degps = Some(value)
                }
                _ => unreachable!(),
            }
            let mut config = Config::default();
            config.clips.insert("guard_*".into(), expectations);
            let expected = ConfigValidationError::InvalidClipLoopCap {
                selector: "guard_*".into(),
                field,
            };
            assert_eq!(config.validate().unwrap_err(), expected, "{field}={value}");

            let ctx = CheckCtx::new(&grids, &roles, &config);
            assert_eq!(
                evaluate_checks(&ctx, &all_checks(), CheckSelection::All).unwrap_err(),
                EvaluationError::InvalidConfiguration(expected),
                "{field}={value}"
            );
        }
    }

    let mut config = Config::default();
    config.clips.insert(
        "guard".into(),
        ClipExpectations {
            looping: Some(true),
            max_loop_position_delta_m: Some(0.0),
            max_loop_rotation_delta_deg: Some(0.0),
            max_loop_velocity_delta_mps: Some(0.0),
            max_loop_angular_velocity_delta_degps: Some(0.0),
            ..ClipExpectations::default()
        },
    );
    config.validate().expect("zero caps are valid");
    let ctx = CheckCtx::new(&grids, &roles, &config);
    evaluate_checks(&ctx, &all_checks(), CheckSelection::All)
        .expect("zero caps pass evaluation validation");
}

#[test]
fn loop_caps_merge_later_globs_then_exact_entries_fieldwise() {
    let config: Config = serde_json::from_value(serde_json::json!({
        "clips": {
            "walk*": {
                "max_loop_position_delta_m": 0.01,
                "max_loop_rotation_delta_deg": 1.0
            },
            "walk_*": {
                "max_loop_position_delta_m": 0.02,
                "max_loop_velocity_delta_mps": 0.2,
                "max_loop_angular_velocity_delta_degps": 20.0
            },
            "walk_position": { "max_loop_position_delta_m": 0.03 },
            "walk_rotation": { "max_loop_rotation_delta_deg": 2.0 },
            "walk_velocity": { "max_loop_velocity_delta_mps": 0.3 },
            "walk_angular": { "max_loop_angular_velocity_delta_degps": 30.0 }
        }
    }))
    .expect("glob and exact cap config");

    let position = config.expectations_for("walk_position");
    assert_eq!(position.max_loop_position_delta_m, Some(0.03));
    assert_eq!(position.max_loop_rotation_delta_deg, Some(1.0));
    assert_eq!(position.max_loop_velocity_delta_mps, Some(0.2));
    assert_eq!(position.max_loop_angular_velocity_delta_degps, Some(20.0));

    let rotation = config.expectations_for("walk_rotation");
    assert_eq!(rotation.max_loop_position_delta_m, Some(0.02));
    assert_eq!(rotation.max_loop_rotation_delta_deg, Some(2.0));
    assert_eq!(rotation.max_loop_velocity_delta_mps, Some(0.2));
    assert_eq!(rotation.max_loop_angular_velocity_delta_degps, Some(20.0));

    let velocity = config.expectations_for("walk_velocity");
    assert_eq!(velocity.max_loop_position_delta_m, Some(0.02));
    assert_eq!(velocity.max_loop_rotation_delta_deg, Some(1.0));
    assert_eq!(velocity.max_loop_velocity_delta_mps, Some(0.3));
    assert_eq!(velocity.max_loop_angular_velocity_delta_degps, Some(20.0));

    let angular = config.expectations_for("walk_angular");
    assert_eq!(angular.max_loop_position_delta_m, Some(0.02));
    assert_eq!(angular.max_loop_rotation_delta_deg, Some(1.0));
    assert_eq!(angular.max_loop_velocity_delta_mps, Some(0.2));
    assert_eq!(angular.max_loop_angular_velocity_delta_degps, Some(30.0));
}

#[test]
fn cap_only_clip_expectations_do_not_declare_a_loop() {
    let config: Config = serde_json::from_value(serde_json::json!({
        "clips": { "guard": {
            "max_loop_position_delta_m": 0.0,
            "max_loop_rotation_delta_deg": 0.0,
            "max_loop_velocity_delta_mps": 0.0,
            "max_loop_angular_velocity_delta_degps": 0.0
        } }
    }))
    .expect("cap-only clip config");
    let records = evaluate(&translation_doc([0.0; 5]), &config);
    for id in ["loop-closure", "loop-seam-vel", "loop-seam-rot"] {
        let result = check(&records, id);
        assert_eq!(result.applicability(), Applicability::NotApplicable, "{id}");
        assert_eq!(result.evaluation(), EvaluationState::NotEvaluated, "{id}");
        assert!(result.findings().is_empty(), "{id}: {result:#?}");
        assert!(result.gaps().is_empty(), "{id}: {result:#?}");
    }
}

#[test]
fn exact_clip_position_cap_is_inclusive_at_the_f32_evidence_boundary() {
    let cap = 0.02f32;
    let mut config = Config::default();
    config.clips.insert(
        "guard".into(),
        ClipExpectations {
            looping: Some(true),
            max_loop_position_delta_m: Some(f64::from(cap)),
            ..ClipExpectations::default()
        },
    );

    let at = translation_doc([0.0, 0.005, 0.01, 0.015, cap]);
    let at_records = evaluate(&at, &config);
    assert!(
        check(&at_records, "loop-closure").findings().is_empty(),
        "equal evidence must satisfy the inclusive clip cap"
    );

    let just_over = f32::from_bits(cap.to_bits() + 1);
    let over = translation_doc([0.0, 0.005, 0.01, 0.015, just_over]);
    let over_records = evaluate(&over, &config);
    let closure = check(&over_records, "loop-closure");
    assert_eq!(closure.findings().len(), 1, "{closure:#?}");
    assert_eq!(number(&closure.findings()[0].expected), f64::from(cap));
}

#[test]
fn linear_and_pose_clip_caps_do_not_change_angular_seam_expected_values() {
    let doc = rotation_steps_doc([0.0, 10.0, 20.0, 10.0, 0.0]);
    let clip_caps = serde_json::json!({
        "loop": true,
        "max_loop_position_delta_m": 0.0,
        "max_loop_rotation_delta_deg": 0.0,
        "max_loop_velocity_delta_mps": 0.0
    });

    let default_config: Config = serde_json::from_value(serde_json::json!({
        "clips": { "guard": clip_caps.clone() }
    }))
    .expect("default angular cap config");
    let default_records = evaluate(&doc, &default_config);
    let default_angular = check(&default_records, "loop-seam-rot");
    assert_eq!(default_angular.findings().len(), 1, "{default_angular:#?}");
    assert_eq!(number(&default_angular.findings()[0].expected), 5.0);

    let configured: Config = serde_json::from_value(serde_json::json!({
        "checks": {
            "loop-seam-rot": { "max_angular_velocity_delta_degps": 70.0 }
        },
        "clips": { "guard": clip_caps }
    }))
    .expect("configured angular cap config");
    let configured_records = evaluate(&doc, &configured);
    let configured_angular = check(&configured_records, "loop-seam-rot");
    assert_eq!(
        configured_angular.findings().len(),
        1,
        "{configured_angular:#?}"
    );
    assert_eq!(number(&configured_angular.findings()[0].expected), 70.0);
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
fn one_unusable_bone_retains_other_bones_measurements_and_findings() {
    let mut doc = translation_doc([0.0, 0.03, 0.01, 0.02, 0.02]);
    doc.clips[0]
        .tracks
        .push(rotation_track([0.0, 10.0, 20.0, 10.0, 0.0]));
    doc.skeleton.bones.push(Bone {
        name: "bad_helper".into(),
        parent: None,
        rest: Transform::IDENTITY,
        inverse_bind: None,
    });
    doc.clips[0].tracks.push(Track {
        bone: 1,
        property: Property::Translation,
        interpolation: Interpolation::Linear,
        times: TIMES.into(),
        values: TrackValues::Vec3s(vec![Vec3::splat(f32::NAN); TIMES.len()]),
    });

    let grids = MetricGrids::new(&doc);
    let measurements = animsmith_core::measure::measure_document(
        &grids,
        &ResolvedRoles::default(),
        &loop_config(),
    );
    let continuity = measurements["guard"]
        .loop_continuity
        .as_ref()
        .expect("shared loop grid remains measurable");
    assert_eq!(
        measurements["guard"].loop_continuity_availability,
        MeasurementAvailability::Measured
    );
    assert_eq!(continuity.bones.len(), 2);
    assert_eq!(
        continuity.bones[0].availability,
        MeasurementAvailability::Measured
    );
    assert_eq!(
        continuity.bones[0].position_delta_m,
        Some(f64::from(0.02f32))
    );
    assert_eq!(
        continuity.bones[1].availability,
        MeasurementAvailability::Unavailable
    );
    assert_eq!(continuity.bones[1].bone_name, "bad_helper");
    assert_eq!(continuity.bones[1].position_delta_m, None);
    assert_eq!(continuity.bones[1].rotation_delta_deg, None);
    assert_eq!(continuity.bones[1].seam_velocity_delta_mps, None);
    assert_eq!(continuity.bones[1].seam_angular_velocity_delta_degps, None);
    assert_eq!(
        measurements["guard"].loop_endpoint_mode_availability,
        MeasurementAvailability::Unavailable,
        "endpoint classification must fail closed over the complete skeleton"
    );
    assert_eq!(measurements["guard"].loop_endpoint_mode, None);

    let records = evaluate(&doc, &loop_config());
    for id in ["loop-closure", "loop-seam-vel", "loop-seam-rot"] {
        let result = check(&records, id);
        assert_eq!(result.evaluation(), EvaluationState::Partial, "{id}");
        assert_eq!(result.evaluated_scopes().len(), 1, "{id}");
        assert_eq!(result.gaps().len(), 1, "{id}");
        assert_eq!(
            result.gaps()[0].code,
            CoverageGapCode::MEASUREMENT_UNAVAILABLE,
            "{id}"
        );
        assert!(result.gaps()[0].message.contains("1 of 2 bones"), "{id}");
        assert_eq!(result.findings().len(), 1, "{id}: {result:#?}");
        assert_eq!(result.findings()[0].bone.as_deref(), Some("root"), "{id}");
    }
    let closure = check(&records, "loop-closure");
    assert_eq!(number(&closure.findings()[0].measured), f64::from(0.02f32));
}

#[test]
fn all_unusable_bones_are_explicit_and_checks_remain_not_evaluated() {
    let doc = translation_doc([f32::NAN; 5]);
    let grids = MetricGrids::new(&doc);
    let measurements = animsmith_core::measure::measure_document(
        &grids,
        &ResolvedRoles::default(),
        &loop_config(),
    );
    let clip = &measurements["guard"];
    let continuity = clip
        .loop_continuity
        .as_ref()
        .expect("the usable shared grid retains the skeleton row");
    assert_eq!(
        clip.loop_continuity_availability,
        MeasurementAvailability::Measured
    );
    assert_eq!(continuity.bones.len(), 1);
    assert_eq!(
        continuity.bones[0].availability,
        MeasurementAvailability::Unavailable
    );
    assert_eq!(continuity.bones[0].position_delta_m, None);

    let records = evaluate(&doc, &loop_config());
    for id in ["loop-closure", "loop-seam-vel", "loop-seam-rot"] {
        let result = check(&records, id);
        assert_eq!(result.evaluation(), EvaluationState::NotEvaluated, "{id}");
        assert!(result.evaluated_scopes().is_empty(), "{id}");
        assert_eq!(result.gaps().len(), 1, "{id}");
        assert!(result.gaps()[0].message.contains("1 of 1 bones"), "{id}");
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
        (bones[1].rotation_delta_deg.unwrap() - 10.0).abs() < 1e-3,
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
    assert_eq!(bones[0].seam_velocity_delta_mps, Some(0.0));
    assert!(bones[1].seam_velocity_delta_mps.unwrap() > 0.1);

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
    assert_eq!(
        angular_bones[0].seam_angular_velocity_delta_degps,
        Some(0.0)
    );
    assert!(angular_bones[1].seam_angular_velocity_delta_degps.unwrap() > 79.9);
    assert!(angular_bones[2].seam_angular_velocity_delta_degps.unwrap() > 79.9);
}
