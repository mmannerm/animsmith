use animsmith_core::model::{
    Bone, Clip, Document, Interpolation, Property, Skeleton, Track, TrackValues, Transform,
};
use animsmith_core::{
    Applicability, CheckCtx, CheckEvaluation, CheckSelection, Config, CoverageGapCode,
    EvaluationScopeCode, EvaluationState, MetricGrids, ResolvedRoles, all_checks, evaluate_checks,
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
    doc.clips[0].tracks.push(Track {
        bone: 0,
        property: Property::Rotation,
        interpolation: Interpolation::Linear,
        times: TIMES.into(),
        values: TrackValues::Quats(
            [0.0, 0.25, 0.5, 0.75, 1.0]
                .into_iter()
                .map(|fraction| Quat::from_rotation_y(end_degrees.to_radians() * fraction))
                .collect(),
        ),
    });
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

#[test]
fn default_loop_caps_are_applied_at_the_boundary() {
    let position_under = translation_doc([0.0, 0.00225, 0.0045, 0.00675, 0.009]);
    assert!(
        check(&evaluate(&position_under, &loop_config()), "loop-closure")
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
    let velocity_over = translation_doc([0.0, 0.01375, 0.02, 0.01375, 0.0]);
    assert_eq!(
        check(&evaluate(&velocity_over, &loop_config()), "loop-seam-vel")
            .findings()
            .len(),
        1
    );
}

#[test]
fn clean_stationary_loop_is_measured_without_roles_or_stride() {
    let doc = translation_doc([0.0; 5]);
    let roles = ResolvedRoles::default();
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

    let records = evaluate(&doc, &config);
    for (id, scope_code) in [
        ("loop-closure", EvaluationScopeCode::LOOP_CLOSURE),
        ("loop-seam-vel", EvaluationScopeCode::LOOP_SEAM_VELOCITY),
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

    let mut sign_equivalent = rotation_doc(0.0);
    let TrackValues::Quats(values) = &mut sign_equivalent.clips[0].tracks[1].values else {
        unreachable!()
    };
    values[4] = -Quat::IDENTITY;
    let roles = ResolvedRoles::default();
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
}

#[test]
fn undeclared_and_too_short_clips_keep_typed_coverage_semantics() {
    let doc = translation_doc([0.0; 5]);
    let records = evaluate(&doc, &Config::default());
    for id in ["loop-closure", "loop-seam-vel"] {
        let result = check(&records, id);
        assert_eq!(result.applicability(), Applicability::NotApplicable);
        assert_eq!(result.evaluation(), EvaluationState::NotEvaluated);
        assert!(result.gaps().is_empty());
    }

    let mut short = doc;
    short.clips[0].tracks[0].times = vec![0.0, 1.0];
    short.clips[0].tracks[0].values = TrackValues::Vec3s(vec![Vec3::ZERO, Vec3::ZERO]);
    let records = evaluate(&short, &loop_config());
    for id in ["loop-closure", "loop-seam-vel"] {
        let result = check(&records, id);
        assert_eq!(result.evaluation(), EvaluationState::NotEvaluated);
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
    let doc = translation_doc([0.0, f32::NAN, 0.0, 0.0, 0.0]);
    let records = evaluate(&doc, &loop_config());
    for id in ["loop-closure", "loop-seam-vel"] {
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
}
