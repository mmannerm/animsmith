use std::collections::{BTreeMap, BTreeSet};

use animsmith_core::{
    BUILTIN_COVERAGE_GAP_CODE_DOCS, BUILTIN_EVALUATION_SCOPE_CODE_DOCS, BuiltinCodeDocumentation,
    Document, LintFileReport, MEASUREMENTS_SCHEMA_ID, MEASUREMENTS_SCHEMA_VERSION,
    MeasureFileReport, MeasurementContract, MeasurementReportError, MeasurementReportInput,
    OUTPUT_SCHEMA_ID, OUTPUT_SCHEMA_VERSION, ReportEnvelope, ResolvedRoles, RigInfo, ToolInfo,
    ToolSource,
};

fn tool() -> ToolInfo {
    ToolInfo::animsmith("0.1.0", ToolSource::new(None, None))
}

fn rig() -> RigInfo {
    let doc = Document::default();
    RigInfo::from_resolved(&doc, &ResolvedRoles::default())
}

fn measurements() -> MeasurementContract {
    MeasurementContract::new(BTreeMap::new(), Vec::new())
}

#[test]
fn command_specific_file_types_serialize_only_their_valid_shape() {
    let measure = ReportEnvelope::measure(
        tool(),
        vec![MeasureFileReport::new("measure.glb", rig(), measurements())],
    );
    let lint = ReportEnvelope::lint(
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
        let json = serde_json::to_value(ToolInfo::animsmith("0.1.0", source))
            .expect("tool identity serializes");
        assert!(json["source"]["revision"].is_null());
        assert_eq!(json["source"]["dirty"], true);
    }

    let revision = "0123456789abcdef0123456789abcdef01234567";
    let source = ToolSource::new(Some(revision.into()), Some(false));
    let json = serde_json::to_value(ToolInfo::animsmith("0.1.0", source))
        .expect("tool identity serializes");
    assert_eq!(json["source"]["revision"], revision);
    assert_eq!(json["source"]["dirty"], false);
}

fn assert_reference_table(docs: &str, heading: &str, entries: &[BuiltinCodeDocumentation]) {
    let section = docs
        .split_once(heading)
        .unwrap_or_else(|| panic!("missing reference heading {heading:?}"))
        .1;
    let table = section
        .trim_start()
        .split_once("\n\n")
        .map_or(section.trim(), |(table, _)| table);
    let documented = table
        .lines()
        .filter_map(|line| line.strip_prefix("| `"))
        .filter_map(|line| line.split_once('`').map(|(code, _)| code))
        .collect::<BTreeSet<_>>();
    let registered = entries.iter().map(|entry| entry.code()).collect();
    assert_eq!(documented, registered, "code inventory for {heading}");

    for entry in entries {
        let emitters = entry
            .emitted_by()
            .iter()
            .map(|check_id| format!("`{check_id}`"))
            .collect::<Vec<_>>()
            .join(", ");
        let expected = format!(
            "| `{}` | {} | {} |",
            entry.code(),
            entry.meaning(),
            emitters
        );
        assert!(
            table.lines().any(|line| line == expected),
            "missing or stale reference row {expected:?}"
        );
    }
}

#[test]
fn output_docs_match_registered_builtin_evidence_codes_exactly() {
    let docs = include_str!("../../../docs/output.md");
    assert_reference_table(
        docs,
        "Built-in gap codes are:",
        BUILTIN_COVERAGE_GAP_CODE_DOCS,
    );
    assert_reference_table(
        docs,
        "Built-in completed/gap scope codes are:",
        BUILTIN_EVALUATION_SCOPE_CODE_DOCS,
    );
}
