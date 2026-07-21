use std::collections::BTreeMap;

use animsmith_core::{
    BUILTIN_COVERAGE_GAP_CODES, BUILTIN_EVALUATION_SCOPE_CODES, Document, LintFileReport,
    MeasureFileReport, MeasurementContract, ReportEnvelope, ResolvedRoles, RigInfo, ToolInfo,
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

#[test]
fn output_docs_cover_every_registered_builtin_evidence_code() {
    let docs = include_str!("../../../docs/output.md");
    for code in BUILTIN_COVERAGE_GAP_CODES {
        assert!(
            docs.contains(&format!("`{}`", code.as_str())),
            "missing built-in gap code {}",
            code.as_str()
        );
    }
    for code in BUILTIN_EVALUATION_SCOPE_CODES {
        assert!(
            docs.contains(&format!("`{}`", code.as_str())),
            "missing built-in scope code {}",
            code.as_str()
        );
    }
}
