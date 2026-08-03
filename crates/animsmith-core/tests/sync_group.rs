use animsmith_core::finding::MemberMeasurement;
use animsmith_core::model::{
    Bone, Clip, Document, Interpolation, Property, Skeleton, Track, TrackValues, Transform,
};
use animsmith_core::{
    CheckCtx, CheckSelection, Config, ConfigValidationError, EvaluationState, MetricGrids,
    ResolvedRoles, all_checks, evaluate_checks,
};
use animsmith_core::{Finding, Severity};
use glam::Vec3;

fn track(values: [f32; 3]) -> Track {
    Track {
        bone: 0,
        property: Property::Translation,
        interpolation: Interpolation::Linear,
        times: vec![0.0, 0.5, 1.0],
        values: TrackValues::Vec3s(values.into_iter().map(|x| Vec3::new(x, 0.0, 0.0)).collect()),
    }
}

fn document() -> Document {
    Document {
        skeleton: Skeleton {
            bones: vec![Bone {
                name: "root".into(),
                parent: None,
                rest: Transform::IDENTITY,
                inverse_bind: None,
            }],
        },
        clips: vec![
            Clip {
                name: "a".into(),
                duration_s: 1.0,
                tracks: vec![track([0.0, 1.0, 0.0])],
            },
            Clip {
                name: "b".into(),
                duration_s: 1.1,
                tracks: vec![track([0.0, 1.0, 0.0])],
            },
        ],
        ..Document::default()
    }
}

fn compatible_document() -> Document {
    let mut doc = document();
    doc.clips[1].duration_s = 1.0;
    doc
}

fn config(members: serde_json::Value) -> Config {
    serde_json::from_value(serde_json::json!({
        "clips": {
            "a": { "loop": true, "fps": 30.0 },
            "b": { "loop": true, "fps": 30.0 }
        },
        "sync_groups": { "ring": {
            "clips": members,
            "max_duration_delta_s": 0.01,
            "max_frame_count_delta": 0,
            "max_fps_delta": 0.0
        }}
    }))
    .expect("valid sync config")
}

fn sync_record(doc: &Document, config: &Config) -> animsmith_core::CheckEvaluation {
    let grids = MetricGrids::new(doc);
    let roles = ResolvedRoles::default();
    evaluate_checks(
        &CheckCtx::new(&grids, &roles, config),
        &all_checks(),
        CheckSelection::All,
    )
    .expect("evaluates")
    .into_iter()
    .find(|record| record.check_id() == "sync-group")
    .expect("sync group record")
}

#[test]
fn duration_mismatch_carries_configured_order_member_table() {
    let doc = document();
    let record = sync_record(&doc, &config(serde_json::json!(["a", "b"])));
    let finding = record
        .findings()
        .iter()
        .find(|finding| finding.message.contains("duration spread"))
        .expect("duration mismatch");
    assert_eq!(finding.measured.as_ref().unwrap().to_string(), "0.1000");
    let members = finding.members.as_ref().expect("structured members");
    assert_eq!(
        members
            .iter()
            .map(|member| member.member.as_str())
            .collect::<Vec<_>>(),
        ["a", "b"]
    );
    assert_eq!(
        members[0].measurements["frame_intervals"].to_string(),
        "30.0000"
    );
}

#[test]
fn clean_same_length_group_has_complete_coverage() {
    let doc = compatible_document();
    let record = sync_record(&doc, &config(serde_json::json!(["a", "b"])));
    assert_eq!(record.evaluation(), EvaluationState::Complete);
    assert!(record.findings().is_empty());
    assert!(record.gaps().is_empty());
}

#[test]
fn frame_count_fps_and_frame_interval_mismatches_are_independent() {
    let mut doc = compatible_document();
    doc.clips[1].tracks[0].times = vec![0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0];
    doc.clips[1].tracks[0].values =
        TrackValues::Vec3s(vec![Vec3::ZERO, Vec3::X, Vec3::X, Vec3::ZERO]);
    let mut config = config(serde_json::json!(["a", "b"]));
    config.clips.get_mut("b").expect("b config").fps = Some(24.0);

    let record = sync_record(&doc, &config);
    for dimension in ["frame count", "FPS", "frame intervals"] {
        assert!(
            record
                .findings()
                .iter()
                .any(|finding| finding.message.contains(dimension)),
            "missing {dimension} finding: {:#?}",
            record.findings(),
        );
    }
    assert!(
        !record
            .findings()
            .iter()
            .any(|finding| finding.message.contains("duration spread")),
    );
}

#[test]
fn duplicate_and_nonclosing_endpoint_modes_are_incompatible() {
    let mut doc = compatible_document();
    doc.clips[1].tracks[0].values =
        TrackValues::Vec3s(vec![Vec3::ZERO, Vec3::X, Vec3::new(0.02, 0.0, 0.0)]);

    let record = sync_record(&doc, &config(serde_json::json!(["a", "b"])));
    let finding = record
        .findings()
        .iter()
        .find(|finding| finding.message.contains("incompatible loop endpoint modes"))
        .expect("endpoint mismatch finding");
    let members = finding.members.as_ref().expect("member table");
    assert_eq!(
        members[0].measurements["loop_endpoint_mode"].to_string(),
        "duplicate_endpoint"
    );
    assert_eq!(
        members[1].measurements["loop_endpoint_mode"].to_string(),
        "non_closing"
    );
}

#[test]
fn missing_member_is_content_error_and_keeps_table_row() {
    let doc = document();
    let record = sync_record(&doc, &config(serde_json::json!(["a", "missing"])));
    assert_eq!(record.evaluation(), EvaluationState::Partial);
    assert!(
        record
            .findings()
            .iter()
            .any(|finding| finding.clip.as_deref() == Some("missing"))
    );
    let finding = record
        .findings()
        .iter()
        .find(|finding| finding.clip.as_deref() == Some("missing"))
        .expect("missing finding");
    assert_eq!(finding.members.as_ref().unwrap()[1].member, "missing");
    assert_eq!(
        finding.members.as_ref().unwrap()[1].measurements["availability"].to_string(),
        "missing"
    );
    assert!(
        record
            .gaps()
            .iter()
            .any(|gap| gap.code.as_str() == "insufficient_measurable_members")
    );
}

#[test]
fn direct_config_rejects_nonfinite_sync_tolerance() {
    let mut config = config(serde_json::json!(["a", "b"]));
    config.sync_groups.get_mut("ring").unwrap().max_fps_delta = f64::NAN;
    assert_eq!(
        config.validate(),
        Err(ConfigValidationError::InvalidSyncGroupTolerance {
            group: "ring".into(),
            field: "max_fps_delta",
        })
    );
}

#[test]
fn member_measurements_omit_nonfinite_numbers() {
    let finding = Finding::new("sync-group", Severity::Error, "test").members(vec![
        MemberMeasurement::new("a")
            .measurement("finite", 1.0)
            .measurement("nonfinite", f64::NAN),
    ]);
    let value = serde_json::to_value(finding).expect("serializes");
    assert_eq!(value["members"][0]["measurements"]["finite"], 1.0);
    assert!(
        value["members"][0]["measurements"]
            .get("nonfinite")
            .is_none()
    );
}

#[test]
fn nonfinite_duration_is_unavailable_group_evidence() {
    let mut doc = compatible_document();
    doc.clips[1].duration_s = f64::NAN;
    let record = sync_record(&doc, &config(serde_json::json!(["a", "b"])));
    assert_eq!(record.evaluation(), EvaluationState::Partial);
    assert!(record.gaps().iter().any(|gap| {
        gap.code.as_str() == "measurement_unavailable" && gap.message.contains("finite durations")
    }));
    assert!(
        !record
            .findings()
            .iter()
            .any(|finding| finding.message.contains("duration spread"))
    );
}
