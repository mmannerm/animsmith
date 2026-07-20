use std::collections::BTreeMap;

use animsmith_core::{
    ContractError, Document, FileReport, MeasurementContract, ReportEnvelope, ResolvedRoles,
    RigInfo, ToolInfo, ToolSource,
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
fn command_envelopes_reject_the_opposite_file_record_shape() {
    let lint_file = FileReport::lint("lint.glb", rig(), Vec::new(), measurements());
    assert_eq!(
        ReportEnvelope::measure(tool(), vec![lint_file]).unwrap_err(),
        ContractError::MeasureFileCarriesChecks {
            path: "lint.glb".into()
        }
    );

    let measure_file = FileReport::measure("measure.glb", rig(), measurements());
    assert_eq!(
        ReportEnvelope::lint(tool(), vec![measure_file]).unwrap_err(),
        ContractError::LintFileMissingChecks {
            path: "measure.glb".into()
        }
    );
}

#[test]
fn tool_source_drops_revision_text_outside_the_v2_schema() {
    let source = ToolSource::new(Some("bad\nrevision".into()), Some(true));
    let json = serde_json::to_value(ToolInfo::animsmith("0.1.0", source))
        .expect("tool identity serializes");
    assert!(json["source"]["revision"].is_null());
    assert_eq!(json["source"]["dirty"], true);
}
