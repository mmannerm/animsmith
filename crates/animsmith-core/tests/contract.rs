use std::collections::BTreeMap;

use animsmith_core::check::{Check, CheckCtx};
use animsmith_core::config::{CheckSettings, SeveritySetting};
use animsmith_core::measure::{ClipMeasurements, MeshMeasurements};
use animsmith_core::{
    Bone, CheckEvaluation, CheckOutput, CheckSelection, Config, CoverageGap, CoverageGapCode,
    Document, EvaluationScope, EvaluationScopeCode, Finding, LintEnvelope, LintFileReport,
    MEASUREMENTS_SCHEMA_ID, MEASUREMENTS_SCHEMA_VERSION, MeasureEnvelope, MeasureFileReport,
    MeasurementContract, MeasurementContractError, MeasurementReportError, MeasurementReportInput,
    MetricGrids, OUTPUT_SCHEMA_ID, OUTPUT_SCHEMA_VERSION, ResolvedRoles, RigInfo, RigInfoError,
    Role, Severity, ToolInfo, ToolSource, Transform, evaluate_checks,
};

fn tool() -> ToolInfo {
    ToolInfo::animsmith(ToolSource::new(None, None))
}

fn rig() -> RigInfo {
    let doc = Document::default();
    RigInfo::from_resolved(&doc, &ResolvedRoles::default())
        .expect("empty roles match an empty document")
}

fn measurements() -> MeasurementContract {
    MeasurementContract::new(BTreeMap::new(), Vec::new()).expect("empty measurements are valid")
}

#[test]
fn command_specific_file_types_serialize_only_their_valid_shape() {
    let measure = MeasureEnvelope::new(
        tool(),
        vec![MeasureFileReport::new("measure.glb", rig(), measurements())],
    );
    let lint = LintEnvelope::new(
        tool(),
        vec![LintFileReport::new(
            "lint.glb",
            rig(),
            Vec::new(),
            measurements(),
        )],
    );
    let measure = serde_json::to_value(measure).expect("measure envelope serializes");
    let lint = serde_json::to_value(lint).expect("lint envelope serializes");
    assert!(measure["files"][0].get("checks").is_none());
    assert_eq!(lint["files"][0]["checks"], serde_json::json!([]));

    let input: MeasurementReportInput =
        serde_json::from_value(measure).expect("current measure envelope deserializes");
    assert!(
        input
            .into_clip_measurements()
            .expect("current measure envelope is accepted")
            .is_empty()
    );
}

struct DisabledCheck;

impl Check for DisabledCheck {
    fn id(&self) -> &'static str {
        "disabled"
    }

    fn evaluate(&self, _ctx: &CheckCtx<'_>) -> CheckOutput {
        panic!("disabled checks must not evaluate")
    }
}

fn disabled_evaluation() -> CheckEvaluation {
    let doc = Document::default();
    let roles = ResolvedRoles::default();
    let config = Config {
        checks: BTreeMap::from([(
            "disabled".to_owned(),
            CheckSettings {
                severity: Some(SeveritySetting::Off),
                ..CheckSettings::default()
            },
        )]),
        ..Config::default()
    };
    let grids = MetricGrids::new(&doc);
    let ctx = CheckCtx::new(&grids, &roles, &config);
    evaluate_checks(&ctx, &[Box::new(DisabledCheck)], CheckSelection::All)
        .expect("disabled check record is valid")
        .remove(0)
}

#[test]
fn lint_summary_aggregates_every_axis_and_finding_bucket_across_files() {
    let partial = CheckEvaluation::evaluated(
        "partial",
        CheckOutput::from_coverage(
            vec![
                Finding::new("partial", Severity::Error, "error"),
                Finding::new("partial", Severity::Note, "note one"),
            ],
            vec![EvaluationScope::new(EvaluationScopeCode::custom(
                "test:completed",
            ))],
            vec![CoverageGap::new(
                CoverageGapCode::custom("test:missing"),
                "missing evidence",
            )],
        ),
    )
    .expect("partial record is valid");
    let complete = CheckEvaluation::evaluated(
        "complete",
        CheckOutput::from_coverage(
            vec![
                Finding::new("complete", Severity::Warning, "warning"),
                Finding::new("complete", Severity::Note, "note two"),
            ],
            Vec::new(),
            Vec::new(),
        ),
    )
    .expect("complete record is valid");

    let report = LintEnvelope::new(
        tool(),
        vec![
            LintFileReport::new(
                "first.glb",
                rig(),
                vec![partial, disabled_evaluation()],
                measurements(),
            ),
            LintFileReport::new("second.glb", rig(), vec![complete], measurements()),
        ],
    );
    let report = serde_json::to_value(report).expect("lint envelope serializes");

    assert_eq!(
        report["summary"],
        serde_json::json!({
            "files": 2,
            "findings": { "error": 1, "warning": 1, "note": 2 },
            "checks": {
                "total": 3,
                "selection": { "selected": 3, "unselected": 0 },
                "configuration": { "enabled": 2, "disabled": 1 },
                "applicability": { "applicable": 3, "not_applicable": 0 },
                "evaluation": { "complete": 1, "partial": 1, "not_evaluated": 1 },
                "gaps": 1,
            },
        })
    );
}

fn current_measure_report() -> serde_json::Value {
    serde_json::json!({
        "schema_version": OUTPUT_SCHEMA_VERSION,
        "schema": OUTPUT_SCHEMA_ID,
        "command": "measure",
        "files": [{
            "measurements": {
                "schema_version": MEASUREMENTS_SCHEMA_VERSION,
                "schema": MEASUREMENTS_SCHEMA_ID,
                "clips": {},
            },
        }],
    })
}

fn measurement_report_error(value: serde_json::Value) -> MeasurementReportError {
    let input: MeasurementReportInput =
        serde_json::from_value(value).expect("test case remains structurally deserializable");
    input
        .into_clip_measurements()
        .expect_err("malformed report must be rejected")
}

#[test]
fn measurement_report_input_rejects_every_invalid_contract_branch() {
    let base = current_measure_report();

    let without = |pointer: &str| {
        let mut value = base.clone();
        let (parent, key) = pointer.rsplit_once('/').expect("JSON pointer has a key");
        let object = if parent.is_empty() {
            &mut value
        } else {
            value.pointer_mut(parent).expect("fixture path exists")
        };
        object
            .as_object_mut()
            .expect("path ends at an object")
            .remove(key);
        value
    };

    let mut future_output = base.clone();
    future_output["schema_version"] = serde_json::json!(OUTPUT_SCHEMA_VERSION + 1);
    let mut wrong_output_identity = base.clone();
    wrong_output_identity["schema"] = serde_json::json!("urn:other:output");
    let mut unsupported_command = base.clone();
    unsupported_command["command"] = serde_json::json!("inspect");
    let mut no_files = base.clone();
    no_files["files"] = serde_json::json!([]);
    let mut two_files = base.clone();
    two_files["files"] = serde_json::json!([base["files"][0].clone(), base["files"][0].clone(),]);
    let mut future_measurements = base.clone();
    future_measurements["files"][0]["measurements"]["schema_version"] =
        serde_json::json!(MEASUREMENTS_SCHEMA_VERSION + 1);
    let mut wrong_measurement_identity = base.clone();
    wrong_measurement_identity["files"][0]["measurements"]["schema"] =
        serde_json::json!("urn:other:measurements");

    let cases = [
        (
            "missing output version",
            without("/schema_version"),
            MeasurementReportError::MissingOutputVersion,
        ),
        (
            "unsupported output version",
            future_output,
            MeasurementReportError::UnsupportedOutputVersion {
                found: OUTPUT_SCHEMA_VERSION + 1,
            },
        ),
        (
            "wrong output identity",
            wrong_output_identity,
            MeasurementReportError::WrongOutputIdentity,
        ),
        (
            "missing command",
            without("/command"),
            MeasurementReportError::MissingCommand,
        ),
        (
            "unsupported command",
            unsupported_command,
            MeasurementReportError::UnsupportedCommand {
                command: "inspect".into(),
            },
        ),
        (
            "missing files",
            without("/files"),
            MeasurementReportError::MissingFiles,
        ),
        (
            "no file records",
            no_files,
            MeasurementReportError::FileCount { found: 0 },
        ),
        (
            "multiple file records",
            two_files,
            MeasurementReportError::FileCount { found: 2 },
        ),
        (
            "missing measurements",
            without("/files/0/measurements"),
            MeasurementReportError::MissingMeasurements,
        ),
        (
            "missing measurement version",
            without("/files/0/measurements/schema_version"),
            MeasurementReportError::MissingMeasurementVersion,
        ),
        (
            "unsupported measurement version",
            future_measurements,
            MeasurementReportError::UnsupportedMeasurementVersion {
                found: MEASUREMENTS_SCHEMA_VERSION + 1,
            },
        ),
        (
            "wrong measurement identity",
            wrong_measurement_identity,
            MeasurementReportError::WrongMeasurementIdentity,
        ),
        (
            "missing clips",
            without("/files/0/measurements/clips"),
            MeasurementReportError::MissingClips,
        ),
    ];

    for (name, value, expected) in cases {
        assert_eq!(measurement_report_error(value), expected, "{name}");
    }
}

#[test]
fn tool_source_drops_revision_text_outside_the_v2_schema() {
    for invalid in ["f".repeat(39), "z".repeat(40), "f".repeat(41)] {
        let source = ToolSource::new(Some(invalid), Some(true));
        let json =
            serde_json::to_value(ToolInfo::animsmith(source)).expect("tool identity serializes");
        assert!(json["source"]["revision"].is_null());
        assert_eq!(json["source"]["dirty"], true);
    }

    let revision = "0123456789abcdef0123456789abcdef01234567";
    let source = ToolSource::new(Some(revision.into()), Some(false));
    let json = serde_json::to_value(ToolInfo::animsmith(source)).expect("tool identity serializes");
    assert_eq!(json["source"]["revision"], revision);
    assert_eq!(json["source"]["dirty"], false);
}

#[test]
fn tool_info_uses_the_animsmith_core_package_version() {
    let json = serde_json::to_value(ToolInfo::animsmith(ToolSource::new(None, None)))
        .expect("tool identity serializes");
    assert_eq!(json["name"], "animsmith");
    assert_eq!(json["version"], env!("CARGO_PKG_VERSION"));
}

fn valid_clip_measurements() -> ClipMeasurements {
    serde_json::from_value(serde_json::json!({
        "duration_s": 1.0,
        "frame_count": 2,
        "animated_bones": ["hips"],
        "bone_rotation_range_deg": { "hips": 10.0 },
        "loop_seam_ratio": 0.1,
        "gait": { "phase": 0.25, "lr_amplitude_m": 0.2 },
        "speed_mps": 1.0,
    }))
    .expect("valid clip measurement fixture")
}

fn valid_mesh_measurements() -> MeshMeasurements {
    serde_json::from_value(serde_json::json!({
        "name": "mesh",
        "vertex_count": 3,
        "aabb": { "min": [0.0, 0.0, 0.0], "max": [1.0, 1.0, 1.0] },
        "max_joints_per_vertex": 4,
        "weight_sum_min": 0.9,
        "weight_sum_max": 1.1,
    }))
    .expect("valid mesh measurement fixture")
}

fn assert_invalid_clip(mutate: impl FnOnce(&mut ClipMeasurements), expected_path: &str) {
    let mut clip = valid_clip_measurements();
    mutate(&mut clip);
    assert_eq!(
        MeasurementContract::new(BTreeMap::from([("walk".into(), clip)]), Vec::new())
            .expect_err("non-finite clip evidence must be rejected"),
        MeasurementContractError::NonFiniteValue {
            path: expected_path.into(),
        }
    );
}

fn assert_invalid_mesh(mutate: impl FnOnce(&mut MeshMeasurements), expected_path: &str) {
    let mut mesh = valid_mesh_measurements();
    mutate(&mut mesh);
    assert_eq!(
        MeasurementContract::new(BTreeMap::new(), vec![mesh])
            .expect_err("non-finite mesh evidence must be rejected"),
        MeasurementContractError::NonFiniteValue {
            path: expected_path.into(),
        }
    );
}

#[test]
fn measurement_contract_rejects_every_non_finite_numeric_branch() {
    assert_invalid_clip(
        |clip| clip.duration_s = f64::NAN,
        "clips[\"walk\"].duration_s",
    );
    assert_invalid_clip(
        |clip| {
            clip.bone_rotation_range_deg
                .insert("hips".into(), f64::INFINITY);
        },
        "clips[\"walk\"].bone_rotation_range_deg[\"hips\"]",
    );
    assert_invalid_clip(
        |clip| clip.loop_seam_ratio = Some(f64::NEG_INFINITY),
        "clips[\"walk\"].loop_seam_ratio",
    );
    assert_invalid_clip(
        |clip| clip.gait.as_mut().expect("fixture gait").phase = Some(f64::NAN),
        "clips[\"walk\"].gait.phase",
    );
    assert_invalid_clip(
        |clip| clip.gait.as_mut().expect("fixture gait").lr_amplitude_m = f64::INFINITY,
        "clips[\"walk\"].gait.lr_amplitude_m",
    );
    assert_invalid_clip(
        |clip| clip.speed_mps = Some(f64::NAN),
        "clips[\"walk\"].speed_mps",
    );
    assert_invalid_mesh(
        |mesh| mesh.aabb.as_mut().expect("fixture aabb").min[1] = f32::NAN,
        "meshes[0].aabb.min[1]",
    );
    assert_invalid_mesh(
        |mesh| mesh.aabb.as_mut().expect("fixture aabb").max[2] = f32::INFINITY,
        "meshes[0].aabb.max[2]",
    );
    assert_invalid_mesh(
        |mesh| mesh.weight_sum_min = Some(f64::NEG_INFINITY),
        "meshes[0].weight_sum_min",
    );
    assert_invalid_mesh(
        |mesh| mesh.weight_sum_max = Some(f64::NAN),
        "meshes[0].weight_sum_max",
    );
}

#[test]
fn rig_info_rejects_roles_resolved_from_another_skeleton() {
    let mut source = Document::default();
    source.skeleton.bones = vec![
        Bone {
            name: "root".into(),
            parent: None,
            rest: Transform::IDENTITY,
            inverse_bind: None,
        },
        Bone {
            name: "foot".into(),
            parent: Some(0),
            rest: Transform::IDENTITY,
            inverse_bind: None,
        },
    ];
    let roles = ResolvedRoles::from_names(&source.skeleton, [(Role::LeftFoot, "foot".to_owned())]);
    let other = Document::default();

    assert_eq!(
        RigInfo::from_resolved(&other, &roles),
        Err(RigInfoError::InvalidBoneId {
            role: "left_foot",
            bone: 1,
            bone_count: 0,
        })
    );

    let mut same_size = Document::default();
    same_size.skeleton.bones = vec![
        Bone {
            name: "root".into(),
            parent: None,
            rest: Transform::IDENTITY,
            inverse_bind: None,
        },
        Bone {
            name: "hand".into(),
            parent: Some(0),
            rest: Transform::IDENTITY,
            inverse_bind: None,
        },
    ];
    assert_eq!(
        RigInfo::from_resolved(&same_size, &roles),
        Err(RigInfoError::BoneNameMismatch {
            role: "left_foot",
            bone: 1,
            expected: "foot".into(),
            found: "hand".into(),
        })
    );
}
