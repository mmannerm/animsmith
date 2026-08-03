use animsmith_core::fixtures::{self, WALK_STRIDE, WalkBones};
use animsmith_core::model::{Document, TrackValues};
use animsmith_core::profile::{ResolvedRoles, Role};
use animsmith_core::{
    CheckCtx, CheckEvaluation, CheckSelection, Config, ConfigValidationError, EvaluationState,
    MetricGrids, all_checks, evaluate_checks,
};

const BONES: WalkBones = WalkBones {
    hips: "pelvis",
    left_foot: "l_foot",
    right_foot: "r_foot",
};

fn roles(doc: &Document) -> ResolvedRoles {
    ResolvedRoles::from_names(
        &doc.skeleton,
        [
            (Role::Hips, BONES.hips.to_owned()),
            (Role::LeftFoot, BONES.left_foot.to_owned()),
            (Role::RightFoot, BONES.right_foot.to_owned()),
        ],
    )
}

fn paired_document(reflected: bool) -> Document {
    let mut doc = fixtures::walk_doc(&BONES, "a", 1.0, WALK_STRIDE, f64::sin);
    let mut second = doc.clips[0].clone();
    second.name = "b".into();
    if reflected {
        for track in &mut second.tracks {
            match &mut track.values {
                TrackValues::Vec3s(values) => values.reverse(),
                TrackValues::Quats(_) => unreachable!("walk fixture uses translation tracks"),
            }
        }
    }
    doc.clips.push(second);
    doc
}

fn config(members: serde_json::Value, min_lr_amplitude_m: f64) -> Config {
    serde_json::from_value(serde_json::json!({
        "sync_groups": { "ring": {
            "clips": members,
            "max_duration_delta_s": 0.0,
            "max_frame_count_delta": 0,
            "max_fps_delta": 0.0,
            "time_complement": {
                "min_reflected_time_advantage": 0.25,
                "min_lr_amplitude_m": min_lr_amplitude_m
            }
        }}
    }))
    .expect("valid time-complement config")
}

fn record(doc: &Document, roles: &ResolvedRoles, config: &Config) -> CheckEvaluation {
    let grids = MetricGrids::new(doc);
    evaluate_checks(
        &CheckCtx::new(&grids, roles, config),
        &all_checks(),
        CheckSelection::All,
    )
    .expect("valid catalog")
    .into_iter()
    .find(|record| record.check_id() == "time-complement")
    .expect("time-complement record")
}

#[test]
fn aligned_pair_is_same_time_compatible() {
    let doc = paired_document(false);
    let roles = roles(&doc);
    let record = record(&doc, &roles, &config(serde_json::json!(["a", "b"]), 0.03));

    assert_eq!(record.evaluation(), EvaluationState::Complete);
    assert!(record.findings().is_empty(), "{record:#?}");
    assert!(record.gaps().is_empty(), "{record:#?}");
}

#[test]
fn reflected_pair_warns_with_stable_pair_evidence() {
    let doc = paired_document(true);
    let roles = roles(&doc);
    let record = record(&doc, &roles, &config(serde_json::json!(["a", "b"]), 0.03));

    let finding = record.findings().first().expect("time-complement finding");
    assert_eq!(finding.severity.to_string(), "warning");
    assert!(
        finding
            .message
            .contains("same-time / absolute-sync group 'ring'")
    );
    assert!(finding.message.contains("reflected-time gait similarity"));
    assert!(
        finding
            .measured
            .as_ref()
            .expect("advantage")
            .to_string()
            .parse::<f64>()
            .unwrap()
            > 0.9
    );
    let members = finding.members.as_ref().expect("pair evidence");
    assert_eq!(
        members
            .iter()
            .map(|member| member.member.as_str())
            .collect::<Vec<_>>(),
        ["a", "b"]
    );
    for member in members {
        assert!(member.measurements.contains_key("gait_phase"));
        assert!(member.measurements.contains_key("lr_amplitude_m"));
        assert!(member.measurements.contains_key("same_time_similarity"));
        assert!(
            member
                .measurements
                .contains_key("reflected_time_similarity")
        );
        assert!(member.measurements.contains_key("reflected_time_advantage"));
    }
}

#[test]
fn larger_groups_use_configured_pair_order_and_skip_duplicate_members() {
    let mut doc = paired_document(true);
    let mut third = doc.clips[0].clone();
    third.name = "c".into();
    doc.clips.push(third);
    let roles = roles(&doc);
    let record = record(
        &doc,
        &roles,
        &config(serde_json::json!(["b", "a", "a", "c"]), 0.03),
    );

    assert_eq!(record.evaluation(), EvaluationState::Complete);
    assert!(record.gaps().is_empty(), "{record:#?}");
    let pairs = record
        .findings()
        .iter()
        .map(|finding| {
            finding
                .members
                .as_ref()
                .expect("pair evidence")
                .iter()
                .map(|member| member.member.as_str())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert_eq!(pairs, [["b", "a"], ["b", "c"]]);
}

#[test]
fn advantage_threshold_is_exclusive() {
    let doc = paired_document(true);
    let roles = roles(&doc);
    let initial = config(serde_json::json!(["a", "b"]), 0.03);
    let measured = record(&doc, &roles, &initial).findings()[0]
        .measured
        .as_ref()
        .expect("advantage")
        .to_string()
        .parse::<f64>()
        .expect("finite score");
    let mut at_boundary = initial;
    at_boundary
        .sync_groups
        .get_mut("ring")
        .expect("ring")
        .time_complement
        .as_mut()
        .expect("settings")
        .min_reflected_time_advantage = measured;

    let record = record(&doc, &roles, &at_boundary);
    assert_eq!(record.evaluation(), EvaluationState::Complete);
    assert!(record.findings().is_empty(), "{record:#?}");
}

#[test]
fn low_amplitude_pair_is_coverage_not_a_flaky_finding() {
    let mut doc = paired_document(true);
    for clip in &mut doc.clips {
        for track in &mut clip.tracks {
            let TrackValues::Vec3s(values) = &mut track.values else {
                unreachable!("walk fixture uses translation tracks");
            };
            for value in values {
                value.y = -1.0;
            }
        }
    }
    let roles = roles(&doc);
    let record = record(&doc, &roles, &config(serde_json::json!(["a", "b"]), 0.03));

    assert_eq!(record.evaluation(), EvaluationState::Partial);
    assert!(record.findings().is_empty(), "{record:#?}");
    assert!(record.gaps().iter().any(|gap| {
        gap.code.as_str() == "measurement_unavailable" && gap.message.contains("evidence floor")
    }));
}

#[test]
fn unresolved_roles_are_a_typed_gap() {
    let doc = paired_document(true);
    let record = record(
        &doc,
        &ResolvedRoles::default(),
        &config(serde_json::json!(["a", "b"]), 0.03),
    );

    assert_eq!(record.evaluation(), EvaluationState::Partial);
    assert!(record.findings().is_empty());
    assert_eq!(record.gaps().len(), 1);
    assert_eq!(record.gaps()[0].code.as_str(), "roles_unresolved");
}

#[test]
fn missing_member_is_a_typed_gap() {
    let doc = paired_document(false);
    let roles = roles(&doc);
    let record = record(
        &doc,
        &roles,
        &config(serde_json::json!(["a", "missing"]), 0.03),
    );

    assert_eq!(record.evaluation(), EvaluationState::Partial);
    assert!(record.findings().is_empty());
    assert!(
        record
            .gaps()
            .iter()
            .any(|gap| gap.code.as_str() == "members_not_evaluated")
    );
    assert!(
        record
            .gaps()
            .iter()
            .any(|gap| gap.code.as_str() == "insufficient_measurable_members")
    );
}

#[test]
fn direct_config_rejects_invalid_time_complement_settings() {
    let mut config = config(serde_json::json!(["a", "b"]), 0.03);
    config
        .sync_groups
        .get_mut("ring")
        .expect("ring")
        .time_complement
        .as_mut()
        .expect("settings")
        .min_reflected_time_advantage = f64::NAN;
    assert_eq!(
        config.validate(),
        Err(ConfigValidationError::InvalidTimeComplementSetting {
            group: "ring".into(),
            field: "min_reflected_time_advantage",
        })
    );

    config
        .sync_groups
        .get_mut("ring")
        .expect("ring")
        .time_complement
        .as_mut()
        .expect("settings")
        .min_reflected_time_advantage = 0.25;
    config
        .sync_groups
        .get_mut("ring")
        .expect("ring")
        .time_complement
        .as_mut()
        .expect("settings")
        .min_lr_amplitude_m = -0.01;
    assert_eq!(
        config.validate(),
        Err(ConfigValidationError::InvalidTimeComplementSetting {
            group: "ring".into(),
            field: "min_lr_amplitude_m",
        })
    );
}
