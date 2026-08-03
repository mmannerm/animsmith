use std::collections::BTreeMap;

use animsmith_core::check::{Check, CheckCtx};
use animsmith_core::config::{CheckSettings, SeveritySetting};
use animsmith_core::measure::{AssetMeasurements, ClipMeasurements, MeshDefinitionMeasurements};
use animsmith_core::{
    Bone, CheckEvaluation, CheckOutput, CheckSelection, Config, CoverageGap, CoverageGapCode,
    Document, EvaluationScope, EvaluationScopeCode, Finding, LintEnvelope, LintFileReport,
    MEASUREMENTS_SCHEMA_ID, MEASUREMENTS_SCHEMA_VERSION, MeasureEnvelope, MeasureFileReport,
    MeasurementContract, MeasurementContractError, MeasurementFileError, MeasurementReportError,
    MeasurementReportFile, MeasurementReportInput, MetricGrids, OUTPUT_SCHEMA_ID,
    OUTPUT_SCHEMA_VERSION, ResolvedRoles, RigInfo, RigInfoError, Role, Severity, ToolInfo,
    ToolSource, Transform, evaluate_checks,
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
    MeasurementContract::new(BTreeMap::new(), AssetMeasurements::default())
        .expect("empty measurements are valid")
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
            .into_files()
            .expect("current measure envelope is accepted")
            .first()
            .expect("measure envelope has one file")
            .measurements()
            .clips()
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
            "path": "clip.glb",
            "measurements": {
                "schema_version": MEASUREMENTS_SCHEMA_VERSION,
                "schema": MEASUREMENTS_SCHEMA_ID,
                "clips": {},
                "mesh_definitions": [],
                "node_instances": [],
                "scenes": [],
            },
        }],
    })
}

fn measurement_report_error(value: serde_json::Value) -> MeasurementReportError {
    let input: MeasurementReportInput =
        serde_json::from_value(value).expect("test case remains structurally deserializable");
    input
        .into_files()
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
    let mut future_measurements = base.clone();
    future_measurements["files"][0]["measurements"]["schema_version"] =
        serde_json::json!(MEASUREMENTS_SCHEMA_VERSION + 1);
    let mut wrong_measurement_identity = base.clone();
    wrong_measurement_identity["files"][0]["measurements"]["schema"] =
        serde_json::json!("urn:other:measurements");
    let input_without_files: MeasurementReportInput = serde_json::from_value(without("/files"))
        .expect("report without files remains structurally deserializable");
    assert_eq!(input_without_files.file_count(), None);

    let cases = vec![
        (
            "missing output version",
            without("/schema_version"),
            MeasurementReportError::MissingOutputVersion,
            "report envelope has no `schema_version`".to_owned(),
        ),
        (
            "unsupported output version",
            future_output,
            MeasurementReportError::UnsupportedOutputVersion {
                found: OUTPUT_SCHEMA_VERSION + 1,
            },
            format!(
                "has schema_version {}; this build reads schema_version {OUTPUT_SCHEMA_VERSION}",
                OUTPUT_SCHEMA_VERSION + 1
            ),
        ),
        (
            "wrong output identity",
            wrong_output_identity,
            MeasurementReportError::WrongOutputIdentity,
            format!("report envelope does not identify output contract {OUTPUT_SCHEMA_ID}"),
        ),
        (
            "missing command",
            without("/command"),
            MeasurementReportError::MissingCommand,
            "report envelope has no `command`".to_owned(),
        ),
        (
            "unsupported command",
            unsupported_command,
            MeasurementReportError::UnsupportedCommand {
                command: "inspect".into(),
            },
            "report command \"inspect\" does not carry measurement file records".to_owned(),
        ),
        (
            "missing files",
            without("/files"),
            MeasurementReportError::MissingFiles,
            "report envelope has no `files` array".to_owned(),
        ),
        (
            "missing path",
            without("/files/0/path"),
            MeasurementReportError::File {
                file_index: 0,
                source: MeasurementFileError::MissingPath,
            },
            "files[0] has no `path`".to_owned(),
        ),
        (
            "missing measurements",
            without("/files/0/measurements"),
            MeasurementReportError::File {
                file_index: 0,
                source: MeasurementFileError::MissingMeasurements,
            },
            "files[0] has no measurements".to_owned(),
        ),
        (
            "missing measurement version",
            without("/files/0/measurements/schema_version"),
            MeasurementReportError::File {
                file_index: 0,
                source: MeasurementFileError::MissingMeasurementVersion,
            },
            "files[0] has no versioned measurement contract".to_owned(),
        ),
        (
            "unsupported measurement version",
            future_measurements,
            MeasurementReportError::File {
                file_index: 0,
                source: MeasurementFileError::UnsupportedMeasurementVersion {
                    found: MEASUREMENTS_SCHEMA_VERSION + 1,
                },
            },
            format!(
                "files[0] has measurement schema_version {}; this build reads measurement schema_version {MEASUREMENTS_SCHEMA_VERSION}",
                MEASUREMENTS_SCHEMA_VERSION + 1
            ),
        ),
        (
            "wrong measurement identity",
            wrong_measurement_identity,
            MeasurementReportError::File {
                file_index: 0,
                source: MeasurementFileError::WrongMeasurementIdentity,
            },
            format!("files[0] does not identify measurement contract {MEASUREMENTS_SCHEMA_ID}"),
        ),
        (
            "missing clips",
            without("/files/0/measurements/clips"),
            MeasurementReportError::File {
                file_index: 0,
                source: MeasurementFileError::MissingClips,
            },
            "files[0] measurement contract has no `clips` map".to_owned(),
        ),
        (
            "missing mesh definitions",
            without("/files/0/measurements/mesh_definitions"),
            MeasurementReportError::File {
                file_index: 0,
                source: MeasurementFileError::MissingMeshDefinitions,
            },
            "files[0] measurement contract has no `mesh_definitions` array".to_owned(),
        ),
        (
            "missing node instances",
            without("/files/0/measurements/node_instances"),
            MeasurementReportError::File {
                file_index: 0,
                source: MeasurementFileError::MissingNodeInstances,
            },
            "files[0] measurement contract has no `node_instances` array".to_owned(),
        ),
        (
            "missing scenes",
            without("/files/0/measurements/scenes"),
            MeasurementReportError::File {
                file_index: 0,
                source: MeasurementFileError::MissingScenes,
            },
            "files[0] measurement contract has no `scenes` array".to_owned(),
        ),
    ];

    for (name, value, expected, expected_display) in cases {
        let error = measurement_report_error(value);
        assert_eq!(error, expected, "{name}");
        let expected_file_index = match &expected {
            MeasurementReportError::File { .. } => Some(0),
            _ => None,
        };
        assert_eq!(error.file_index(), expected_file_index, "{name}");
        assert_eq!(error.to_string(), expected_display, "{name}");
    }
}

#[test]
fn measurement_report_input_rejects_finite_value_that_overflows_mesh_f32() {
    let mut report = current_measure_report();
    report["files"][0]["measurements"]["mesh_definitions"] = serde_json::json!([{
        "mesh_index": 0,
        "name": "overflow",
        "vertex_count": 1,
        "geometry_aabb": {
            "min": [1e39, 0.0, 0.0],
            "max": [1e39, 0.0, 0.0],
        },
        "max_joints_per_vertex": 0,
        "additional_influence_sets": [],
    }]);

    let error = measurement_report_error(report);
    assert_eq!(
        error,
        MeasurementReportError::File {
            file_index: 0,
            source: MeasurementFileError::InvalidMeasurements {
                source: MeasurementContractError::NonFiniteValue {
                    path: "mesh_definitions[0].geometry_aabb.min[0]".into(),
                },
            },
        }
    );
    assert_eq!(error.file_index(), Some(0));
    assert_eq!(
        error.to_string(),
        "files[0] has invalid measurements: measurement value mesh_definitions[0].geometry_aabb.min[0] must be finite"
    );
}

#[test]
fn measurement_report_input_rejects_inconsistent_static_node_and_scene_evidence() {
    let mut base = current_measure_report();
    base["files"][0]["measurements"] = serde_json::json!({
        "schema_version": MEASUREMENTS_SCHEMA_VERSION,
        "schema": MEASUREMENTS_SCHEMA_ID,
        "clips": {},
        "mesh_definitions": [{
            "mesh_index": 0,
            "name": "mesh",
            "vertex_count": 3,
            "geometry_aabb": {
                "min": [0.0, 0.0, 0.0],
                "max": [1.0, 1.0, 1.0]
            },
            "max_joints_per_vertex": 0,
            "additional_influence_sets": []
        }],
        "node_instances": [{
            "node_index": 2,
            "node_name": "node",
            "mesh_index": 0,
            "static_node_world_aabb": {
                "min": [0.0, 0.0, 0.0],
                "max": [1.0, 1.0, 1.0]
            }
        }],
        "scenes": [{
            "scene_index": 4,
            "instance_count": 1,
            "static_scene_world_aabb": {
                "min": [0.0, 0.0, 0.0],
                "max": [1.0, 1.0, 1.0]
            },
            "excluded_instance_count": 0
        }],
        "default_scene_index": 4
    });

    let mut duplicate_node = base.clone();
    duplicate_node["files"][0]["measurements"]["node_instances"] = serde_json::json!([
        base["files"][0]["measurements"]["node_instances"][0].clone(),
        base["files"][0]["measurements"]["node_instances"][0].clone(),
    ]);
    let mut dangling_mesh = base.clone();
    dangling_mesh["files"][0]["measurements"]["node_instances"][0]["mesh_index"] =
        serde_json::json!(99);
    let mut unavailable_node_with_aabb = base.clone();
    unavailable_node_with_aabb["files"][0]["measurements"]["node_instances"][0]["static_node_world_aabb_unavailable_reason"] =
        serde_json::json!("non_finite_transform");
    let mut inconsistent_scene_count = base.clone();
    inconsistent_scene_count["files"][0]["measurements"]["scenes"][0]["excluded_instance_count"] =
        serde_json::json!(2);
    let mut inverted_node_aabb = base.clone();
    inverted_node_aabb["files"][0]["measurements"]["node_instances"][0]["static_node_world_aabb"]
        ["min"][0] = serde_json::json!(2.0);
    let mut dangling_default_scene = base;
    dangling_default_scene["files"][0]["measurements"]["default_scene_index"] =
        serde_json::json!(99);

    let cases = [
        (
            "duplicate node identity",
            duplicate_node,
            "node_instances[1].node_index",
            "node_index must be unique",
        ),
        (
            "dangling node mesh reference",
            dangling_mesh,
            "node_instances[0].mesh_index",
            "mesh_index must reference a mesh definition",
        ),
        (
            "unavailable node with an AABB",
            unavailable_node_with_aabb,
            "node_instances[0]",
            "an available static node AABB cannot have an unavailable reason",
        ),
        (
            "scene excludes more instances than it contains",
            inconsistent_scene_count,
            "scenes[0].excluded_instance_count",
            "excluded_instance_count cannot exceed instance_count",
        ),
        (
            "node AABB has inverted corners",
            inverted_node_aabb,
            "node_instances[0].static_node_world_aabb.min[0]",
            "AABB minimum cannot exceed maximum",
        ),
        (
            "dangling default scene reference",
            dangling_default_scene,
            "default_scene_index",
            "default_scene_index must reference a declared scene",
        ),
    ];

    for (name, report, path, reason) in cases {
        let error = measurement_report_error(report);
        assert_eq!(error.file_index(), Some(0), "{name}");
        assert_eq!(
            error,
            MeasurementReportError::File {
                file_index: 0,
                source: MeasurementFileError::InvalidMeasurements {
                    source: MeasurementContractError::InvalidStructure {
                        path: path.into(),
                        reason: reason.into(),
                    },
                },
            },
            "{name}",
        );
    }
}

#[test]
fn measurement_report_input_recovers_every_file_without_cardinality_policy() {
    let mut report = current_measure_report();
    report["files"] = serde_json::json!([
        {
            "path": ".\\Walk Assets\\..\\WALK.GLB",
            "measurements": {
                "schema_version": MEASUREMENTS_SCHEMA_VERSION,
                "schema": MEASUREMENTS_SCHEMA_ID,
                "clips": { "walk": valid_clip_measurements() },
                "mesh_definitions": [],
                "node_instances": [],
                "scenes": [],
            },
        },
        {
            "path": "run.glb",
            "measurements": {
                "schema_version": MEASUREMENTS_SCHEMA_VERSION,
                "schema": MEASUREMENTS_SCHEMA_ID,
                "clips": { "run": valid_clip_measurements() },
                "mesh_definitions": [valid_mesh_measurements()],
                "node_instances": [],
                "scenes": [],
            },
        },
        {
            "path": "middle.glb",
            "measurements": {
                "schema_version": MEASUREMENTS_SCHEMA_VERSION,
                "schema": MEASUREMENTS_SCHEMA_ID,
                "clips": { "idle": valid_clip_measurements() },
                "mesh_definitions": [],
                "node_instances": [],
                "scenes": [],
            },
        },
    ]);
    let expected_measurements = report["files"]
        .as_array()
        .expect("file records")
        .iter()
        .map(|file| file["measurements"].clone())
        .collect::<Vec<_>>();

    let input: MeasurementReportInput =
        serde_json::from_value(report).expect("multi-file report deserializes");
    assert_eq!(input.file_count(), Some(3));
    let files: Vec<MeasurementReportFile> = input
        .into_files()
        .expect("multi-file report is consumer-neutral");

    assert_eq!(files.len(), 3);
    assert_eq!(
        files
            .iter()
            .map(MeasurementReportFile::path)
            .collect::<Vec<_>>(),
        [".\\Walk Assets\\..\\WALK.GLB", "run.glb", "middle.glb"]
    );
    assert_eq!(
        files
            .iter()
            .map(|file| {
                serde_json::to_value(file.measurements())
                    .expect("recovered measurement contract serializes")
            })
            .collect::<Vec<_>>(),
        expected_measurements
    );
    type MeasurementParts = (BTreeMap<String, ClipMeasurements>, AssetMeasurements);
    let into_parts: fn(MeasurementContract) -> MeasurementParts = MeasurementContract::into_parts;
    let (run_clips, run_assets) = into_parts(
        files
            .into_iter()
            .nth(1)
            .expect("second recovered record")
            .into_measurements(),
    );
    assert_eq!(
        serde_json::to_value(run_clips).expect("recovered clips serialize"),
        expected_measurements[1]["clips"]
    );
    assert_eq!(
        serde_json::to_value(run_assets.mesh_definitions)
            .expect("recovered mesh definitions serialize"),
        expected_measurements[1]["mesh_definitions"]
    );

    let mut empty_report = current_measure_report();
    empty_report["files"] = serde_json::json!([]);
    let input: MeasurementReportInput =
        serde_json::from_value(empty_report).expect("empty report deserializes");
    assert_eq!(input.file_count(), Some(0));
    assert!(
        input
            .into_files()
            .expect("empty report has no core cardinality error")
            .is_empty()
    );
}

#[test]
fn measurement_report_input_identifies_invalid_file_without_cli_remediation() {
    let base = current_measure_report();
    let mut two_files = base.clone();
    two_files["files"] = serde_json::json!([base["files"][0].clone(), base["files"][0].clone(),]);
    let without = |pointer: &str| {
        let mut report = two_files.clone();
        let (parent, key) = pointer.rsplit_once('/').expect("JSON pointer has a key");
        report
            .pointer_mut(parent)
            .expect("fixture path exists")
            .as_object_mut()
            .expect("path ends at an object")
            .remove(key);
        report
    };
    let mut future_measurements = two_files.clone();
    future_measurements["files"][1]["measurements"]["schema_version"] =
        serde_json::json!(MEASUREMENTS_SCHEMA_VERSION + 1);
    let mut wrong_measurement_identity = two_files.clone();
    wrong_measurement_identity["files"][1]["measurements"]["schema"] =
        serde_json::json!("urn:other:measurements");

    let cases = vec![
        (
            without("/files/1/path"),
            MeasurementReportError::File {
                file_index: 1,
                source: MeasurementFileError::MissingPath,
            },
            "files[1] has no `path`".to_owned(),
        ),
        (
            without("/files/1/measurements"),
            MeasurementReportError::File {
                file_index: 1,
                source: MeasurementFileError::MissingMeasurements,
            },
            "files[1] has no measurements".to_owned(),
        ),
        (
            without("/files/1/measurements/schema_version"),
            MeasurementReportError::File {
                file_index: 1,
                source: MeasurementFileError::MissingMeasurementVersion,
            },
            "files[1] has no versioned measurement contract".to_owned(),
        ),
        (
            future_measurements,
            MeasurementReportError::File {
                file_index: 1,
                source: MeasurementFileError::UnsupportedMeasurementVersion {
                    found: MEASUREMENTS_SCHEMA_VERSION + 1,
                },
            },
            format!(
                "files[1] has measurement schema_version {}; this build reads measurement schema_version {MEASUREMENTS_SCHEMA_VERSION}",
                MEASUREMENTS_SCHEMA_VERSION + 1
            ),
        ),
        (
            wrong_measurement_identity,
            MeasurementReportError::File {
                file_index: 1,
                source: MeasurementFileError::WrongMeasurementIdentity,
            },
            format!("files[1] does not identify measurement contract {MEASUREMENTS_SCHEMA_ID}"),
        ),
        (
            without("/files/1/measurements/clips"),
            MeasurementReportError::File {
                file_index: 1,
                source: MeasurementFileError::MissingClips,
            },
            "files[1] measurement contract has no `clips` map".to_owned(),
        ),
    ];

    for (report, expected, expected_display) in cases {
        let error = measurement_report_error(report);
        assert_eq!(error, expected);
        assert_eq!(error.file_index(), Some(1));
        assert_eq!(error.to_string(), expected_display);
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

fn valid_mesh_measurements() -> MeshDefinitionMeasurements {
    serde_json::from_value(serde_json::json!({
        "mesh_index": 0,
        "name": "mesh",
        "vertex_count": 3,
        "geometry_aabb": { "min": [0.0, 0.0, 0.0], "max": [1.0, 1.0, 1.0] },
        "max_joints_per_vertex": 4,
        "weight_sum_min": 0.9,
        "weight_sum_max": 1.1,
        "additional_influence_sets": [],
    }))
    .expect("valid mesh measurement fixture")
}

fn additional_influence_set(
    set_index: u32,
    joints_present: bool,
    weights_present: bool,
) -> animsmith_core::measure::AdditionalInfluenceSetMeasurements {
    serde_json::from_value(serde_json::json!({
        "set_index": set_index,
        "joints_present": joints_present,
        "weights_present": weights_present,
    }))
    .expect("valid additional influence-set fixture")
}

fn valid_asset_measurements() -> AssetMeasurements {
    serde_json::from_value(serde_json::json!({
        "mesh_definitions": [valid_mesh_measurements()],
        "node_instances": [{
            "node_index": 2,
            "node_name": "node",
            "mesh_index": 0,
            "static_node_world_aabb": {
                "min": [0.0, 0.0, 0.0],
                "max": [1.0, 1.0, 1.0]
            }
        }],
        "scenes": [{
            "scene_index": 4,
            "instance_count": 1,
            "static_scene_world_aabb": {
                "min": [0.0, 0.0, 0.0],
                "max": [1.0, 1.0, 1.0]
            },
            "excluded_instance_count": 0
        }],
        "default_scene_index": 4
    }))
    .expect("valid static asset measurement fixture")
}

fn assert_invalid_assets(
    mutate: impl FnOnce(&mut AssetMeasurements),
    expected_path: &str,
    expected_reason: &str,
) {
    let mut assets = valid_asset_measurements();
    mutate(&mut assets);
    assert_eq!(
        MeasurementContract::new(BTreeMap::new(), assets)
            .expect_err("structurally inconsistent evidence must be rejected"),
        MeasurementContractError::InvalidStructure {
            path: expected_path.into(),
            reason: expected_reason.into(),
        }
    );
}

fn assert_invalid_clip(mutate: impl FnOnce(&mut ClipMeasurements), expected_path: &str) {
    let mut clip = valid_clip_measurements();
    mutate(&mut clip);
    assert_eq!(
        MeasurementContract::new(
            BTreeMap::from([("walk".into(), clip)]),
            AssetMeasurements::default(),
        )
        .expect_err("non-finite clip evidence must be rejected"),
        MeasurementContractError::NonFiniteValue {
            path: expected_path.into(),
        }
    );
}

fn assert_invalid_mesh(mutate: impl FnOnce(&mut MeshDefinitionMeasurements), expected_path: &str) {
    let mut mesh = valid_mesh_measurements();
    mutate(&mut mesh);
    let mut assets = AssetMeasurements::default();
    assets.mesh_definitions.push(mesh);
    assert_eq!(
        MeasurementContract::new(BTreeMap::new(), assets)
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
        |mesh| {
            mesh.geometry_aabb.as_mut().expect("fixture aabb").min[1] = f32::NAN;
        },
        "mesh_definitions[0].geometry_aabb.min[1]",
    );
    assert_invalid_mesh(
        |mesh| {
            mesh.geometry_aabb.as_mut().expect("fixture aabb").max[2] = f32::INFINITY;
        },
        "mesh_definitions[0].geometry_aabb.max[2]",
    );
    assert_invalid_mesh(
        |mesh| mesh.weight_sum_min = Some(f64::NEG_INFINITY),
        "mesh_definitions[0].weight_sum_min",
    );
    assert_invalid_mesh(
        |mesh| mesh.weight_sum_max = Some(f64::NAN),
        "mesh_definitions[0].weight_sum_max",
    );
}

#[test]
fn measurement_contract_rejects_inconsistent_static_asset_relationships() {
    assert_invalid_assets(
        |assets| {
            assets.mesh_definitions[0]
                .geometry_aabb
                .as_mut()
                .expect("fixture geometry AABB")
                .min[0] = 2.0;
        },
        "mesh_definitions[0].geometry_aabb.min[0]",
        "AABB minimum cannot exceed maximum",
    );
    assert_invalid_assets(
        |assets| {
            assets.node_instances[0]
                .static_node_world_aabb
                .as_mut()
                .expect("fixture node AABB")
                .min[1] = 2.0;
        },
        "node_instances[0].static_node_world_aabb.min[1]",
        "AABB minimum cannot exceed maximum",
    );
    assert_invalid_assets(
        |assets| {
            assets.scenes[0]
                .static_scene_world_aabb
                .as_mut()
                .expect("fixture scene AABB")
                .min[2] = 2.0;
        },
        "scenes[0].static_scene_world_aabb.min[2]",
        "AABB minimum cannot exceed maximum",
    );
    assert_invalid_assets(
        |assets| {
            let duplicate = assets.mesh_definitions[0].clone();
            assets.mesh_definitions.push(duplicate);
        },
        "mesh_definitions[1].mesh_index",
        "mesh_index must be unique",
    );
    assert_invalid_assets(
        |assets| assets.node_instances[0].mesh_index = 99,
        "node_instances[0].mesh_index",
        "mesh_index must reference a mesh definition",
    );
    assert_invalid_assets(
        |assets| assets.node_instances[0].static_node_world_aabb = None,
        "node_instances[0]",
        "a missing static node AABB requires an unavailable reason",
    );
    assert_invalid_assets(
        |assets| {
            assets.node_instances[0].static_node_world_aabb_unavailable_reason =
                Some(animsmith_core::measure::StaticNodeAabbUnavailableReason::NonFiniteTransform);
        },
        "node_instances[0]",
        "an available static node AABB cannot have an unavailable reason",
    );
    assert_invalid_assets(
        |assets| assets.scenes[0].excluded_instance_count = 2,
        "scenes[0].excluded_instance_count",
        "excluded_instance_count cannot exceed instance_count",
    );
    assert_invalid_assets(
        |assets| assets.scenes[0].static_scene_world_aabb = None,
        "scenes[0].static_scene_world_aabb",
        "a scene with available instances requires an AABB",
    );
    assert_invalid_assets(
        |assets| assets.default_scene_index = Some(99),
        "default_scene_index",
        "default_scene_index must reference a declared scene",
    );
}

#[test]
fn measurement_contract_rejects_invalid_additional_influence_set_ordering() {
    assert_invalid_assets(
        |assets| {
            assets.mesh_definitions[0].additional_influence_sets =
                vec![additional_influence_set(0, true, false)];
        },
        "mesh_definitions[0].additional_influence_sets[0].set_index",
        "set_index must be at least 1",
    );
    assert_invalid_assets(
        |assets| {
            assets.mesh_definitions[0].additional_influence_sets = vec![
                additional_influence_set(1, true, false),
                additional_influence_set(0, false, true),
            ];
        },
        "mesh_definitions[0].additional_influence_sets[1].set_index",
        "set_index must be at least 1",
    );
    assert_invalid_assets(
        |assets| {
            assets.mesh_definitions[0].additional_influence_sets = vec![
                additional_influence_set(2, true, true),
                additional_influence_set(1, false, true),
            ];
        },
        "mesh_definitions[0].additional_influence_sets[1].set_index",
        "set_index values must be strictly increasing and unique",
    );
    assert_invalid_assets(
        |assets| {
            assets.mesh_definitions[0].additional_influence_sets = vec![
                additional_influence_set(1, true, true),
                additional_influence_set(3, true, false),
                additional_influence_set(2, false, true),
            ];
        },
        "mesh_definitions[0].additional_influence_sets[2].set_index",
        "set_index values must be strictly increasing and unique",
    );
    assert_invalid_assets(
        |assets| {
            assets.mesh_definitions[0].additional_influence_sets = vec![
                additional_influence_set(1, true, true),
                additional_influence_set(1, false, true),
            ];
        },
        "mesh_definitions[0].additional_influence_sets[1].set_index",
        "set_index values must be strictly increasing and unique",
    );
    assert_invalid_assets(
        |assets| {
            assets.mesh_definitions[0].additional_influence_sets =
                vec![additional_influence_set(1, false, false)];
        },
        "mesh_definitions[0].additional_influence_sets[0]",
        "an additional influence set must declare joints, weights, or both",
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
