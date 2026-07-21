use std::collections::BTreeMap;

use animsmith_core::{
    Applicability, CheckEvaluation, ConfigurationState, ContractError, CoverageGap,
    CoverageGapCode, Document, FileReport, Finding, MeasurementContract, ReportEnvelope,
    ResolvedRoles, RigInfo, SelectionState, Severity, ToolInfo, ToolSource,
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
    let measure_file = || FileReport::measure("measure.glb", rig(), measurements());
    let lint_file = || FileReport::lint("lint.glb", rig(), Vec::new(), measurements());
    assert_eq!(
        ReportEnvelope::measure(tool(), vec![measure_file(), lint_file()]).unwrap_err(),
        ContractError::MeasureFileCarriesChecks {
            path: "lint.glb".into()
        }
    );

    assert_eq!(
        ReportEnvelope::lint(tool(), vec![lint_file(), measure_file()]).unwrap_err(),
        ContractError::LintFileMissingChecks {
            path: "measure.glb".into()
        }
    );
}

#[test]
fn lint_envelope_rejects_evidence_that_derives_not_evaluated() {
    let invalid = CheckEvaluation {
        check_id: "example",
        selection: SelectionState::Selected,
        configuration: ConfigurationState::Enabled,
        applicability: Applicability::Applicable,
        findings: vec![Finding::new(
            "example",
            Severity::Error,
            "unsupported judgment",
        )],
        evaluated_scopes: Vec::new(),
        gaps: vec![CoverageGap::new(
            CoverageGapCode::MEASUREMENT_UNAVAILABLE,
            "missing evidence",
        )],
    };
    let file = FileReport::lint("invalid.glb", rig(), vec![invalid], measurements());
    assert_eq!(
        ReportEnvelope::lint(tool(), vec![file]).unwrap_err(),
        ContractError::InvalidCheckEvidence {
            path: "invalid.glb".into(),
            check_id: "example",
            reason: "not-evaluated checks cannot carry findings",
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
