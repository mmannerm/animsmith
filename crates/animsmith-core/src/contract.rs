//! Versioned JSON result-contract types shared by CLI and embedded producers.
//!
//! The CLI is one producer of these envelopes. Embedded pipelines can use the
//! same constructors and immutable protocol identities without duplicating the
//! wire shape or hard-coding URNs.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::diff::MetricDelta;
use crate::evaluation::{
    Applicability, CheckEvaluation, ConfigurationState, EvaluationState, SelectionState,
};
use crate::measure::{ClipMeasurements, MeshMeasurements};
use crate::profile::ResolvedRoles;
use crate::{Document, Severity};

/// Current outer result-envelope version.
pub const OUTPUT_SCHEMA_VERSION: u32 = 2;
/// Immutable identity of the current outer result envelope.
pub const OUTPUT_SCHEMA_ID: &str = "urn:animsmith:schema:output:2";
/// Current nested measurement-contract version.
pub const MEASUREMENTS_SCHEMA_VERSION: u32 = 1;
/// Immutable identity of the current nested measurement contract.
pub const MEASUREMENTS_SCHEMA_ID: &str = "urn:animsmith:schema:measurements:1";

/// Source checkout identity for the producing animsmith build.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolSource {
    revision: Option<String>,
    dirty: Option<bool>,
}

impl ToolSource {
    /// Construct source identity from a full Git revision and dirty bit.
    ///
    /// Packaged or otherwise provenance-free builds use `None` for fields they
    /// cannot establish rather than claiming a clean checkout. Revisions that
    /// are not full 40-character hexadecimal Git object ids are dropped so an
    /// envelope constructed through this API remains within output v2.
    pub fn new(revision: Option<String>, dirty: Option<bool>) -> Self {
        let revision = revision.filter(|revision| {
            revision.len() == 40 && revision.bytes().all(|byte| byte.is_ascii_hexdigit())
        });
        Self { revision, dirty }
    }
}

/// Identity of the animsmith producer that emitted an envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ToolInfo {
    name: &'static str,
    version: String,
    source: ToolSource,
}

impl ToolInfo {
    /// Construct animsmith producer identity from a stable package version and
    /// optional source-checkout metadata.
    pub fn animsmith(version: impl Into<String>, source: ToolSource) -> Self {
        Self {
            name: "animsmith",
            version: version.into(),
            source,
        }
    }
}

/// Rig profile and resolved semantic-role bindings for one input file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RigInfo {
    profile: String,
    resolved_roles: BTreeMap<&'static str, String>,
}

impl RigInfo {
    /// Project resolved roles into their stable role names and source bone
    /// names for the result contract.
    pub fn from_resolved(doc: &Document, roles: &ResolvedRoles) -> Self {
        Self {
            profile: roles.profile.clone(),
            resolved_roles: roles
                .iter()
                .map(|(role, bone)| (role.as_str(), doc.skeleton.bones[bone].name.clone()))
                .collect(),
        }
    }
}

/// Independently versioned measurement payload nested in measure and lint
/// file records.
#[derive(Debug, Clone, Serialize)]
pub struct MeasurementContract {
    schema_version: u32,
    schema: &'static str,
    clips: BTreeMap<String, ClipMeasurements>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    meshes: Vec<MeshMeasurements>,
}

impl MeasurementContract {
    /// Construct the current measurement contract.
    pub fn new(clips: BTreeMap<String, ClipMeasurements>, meshes: Vec<MeshMeasurements>) -> Self {
        Self {
            schema_version: MEASUREMENTS_SCHEMA_VERSION,
            schema: MEASUREMENTS_SCHEMA_ID,
            clips,
            meshes,
        }
    }

    /// Per-clip measurements keyed by clip name.
    pub fn clips(&self) -> &BTreeMap<String, ClipMeasurements> {
        &self.clips
    }

    /// Per-mesh measurements in source order.
    pub fn meshes(&self) -> &[MeshMeasurements] {
        &self.meshes
    }
}

/// One source file and all result-contract evidence produced for it.
#[derive(Debug, Clone, Serialize)]
pub struct FileReport {
    path: String,
    rig: RigInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    checks: Option<Vec<CheckEvaluation>>,
    measurements: MeasurementContract,
}

/// A file record does not match the command envelope being constructed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ContractError {
    /// A measurement envelope was given a lint file record.
    #[error("measure envelope file {path:?} unexpectedly carries check records")]
    MeasureFileCarriesChecks {
        /// Producer-supplied path of the invalid file record.
        path: String,
    },
    /// A lint envelope was given a measurement-only file record.
    #[error("lint envelope file {path:?} has no check records")]
    LintFileMissingChecks {
        /// Producer-supplied path of the invalid file record.
        path: String,
    },
    /// A lint check record carries evidence forbidden by its derived state.
    #[error("lint envelope file {path:?} check {check_id:?} has invalid evidence: {reason}")]
    InvalidCheckEvidence {
        /// Producer-supplied path of the invalid file record.
        path: String,
        /// Stable id of the invalid check record.
        check_id: &'static str,
        /// Contract rule violated by the record.
        reason: &'static str,
    },
}

impl FileReport {
    /// Construct a measurement-only file report.
    pub fn measure(
        path: impl Into<String>,
        rig: RigInfo,
        measurements: MeasurementContract,
    ) -> Self {
        Self {
            path: path.into(),
            rig,
            checks: None,
            measurements,
        }
    }

    /// Construct a lint file report with one record per catalog check.
    pub fn lint(
        path: impl Into<String>,
        rig: RigInfo,
        checks: Vec<CheckEvaluation>,
        measurements: MeasurementContract,
    ) -> Self {
        Self {
            path: path.into(),
            rig,
            checks: Some(checks),
            measurements,
        }
    }

    /// Display path supplied by the producer.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Check records, when this file belongs to a lint envelope.
    pub fn checks(&self) -> Option<&[CheckEvaluation]> {
        self.checks.as_deref()
    }

    /// Nested measurement evidence.
    pub fn measurements(&self) -> &MeasurementContract {
        &self.measurements
    }
}

#[derive(Debug, Clone, Serialize)]
struct EnvelopeHeader {
    schema_version: u32,
    schema: &'static str,
    tool: ToolInfo,
    command: &'static str,
}

impl EnvelopeHeader {
    fn new(tool: ToolInfo, command: &'static str) -> Self {
        Self {
            schema_version: OUTPUT_SCHEMA_VERSION,
            schema: OUTPUT_SCHEMA_ID,
            tool,
            command,
        }
    }
}

/// Measure-command summary.
#[derive(Debug, Clone, Serialize)]
pub struct MeasureSummary {
    files: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
struct FindingSummary {
    error: usize,
    warning: usize,
    note: usize,
}

impl FindingSummary {
    fn add(&mut self, severity: Severity) {
        match severity {
            Severity::Error => self.error += 1,
            Severity::Warning => self.warning += 1,
            Severity::Note => self.note += 1,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
struct SelectionSummary {
    selected: usize,
    unselected: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
struct ConfigurationSummary {
    enabled: usize,
    disabled: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
struct ApplicabilitySummary {
    applicable: usize,
    not_applicable: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
struct EvaluationStateSummary {
    complete: usize,
    partial: usize,
    not_evaluated: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
struct CheckSummary {
    total: usize,
    selection: SelectionSummary,
    configuration: ConfigurationSummary,
    applicability: ApplicabilitySummary,
    evaluation: EvaluationStateSummary,
    gaps: usize,
}

/// Lint-command summary over all file and check records.
#[derive(Debug, Clone, Serialize)]
pub struct LintSummary {
    files: usize,
    findings: FindingSummary,
    checks: CheckSummary,
}

/// Current measure or lint result envelope.
#[derive(Debug, Clone, Serialize)]
pub struct ReportEnvelope<S> {
    #[serde(flatten)]
    header: EnvelopeHeader,
    summary: S,
    files: Vec<FileReport>,
}

impl ReportEnvelope<MeasureSummary> {
    /// Construct a schema-valid measurement envelope.
    ///
    /// # Errors
    ///
    /// Returns an error when any supplied file was constructed as a lint
    /// record and therefore carries a forbidden `checks` field.
    pub fn measure(tool: ToolInfo, files: Vec<FileReport>) -> Result<Self, ContractError> {
        if let Some(file) = files.iter().find(|file| file.checks.is_some()) {
            return Err(ContractError::MeasureFileCarriesChecks {
                path: file.path.clone(),
            });
        }
        Ok(Self {
            header: EnvelopeHeader::new(tool, "measure"),
            summary: MeasureSummary { files: files.len() },
            files,
        })
    }
}

impl ReportEnvelope<LintSummary> {
    /// Construct a schema-valid lint envelope and derive its summary from the
    /// supplied check records.
    ///
    /// # Errors
    ///
    /// Returns an error when any supplied file was constructed as a
    /// measurement-only record and therefore lacks `checks`, or when a check
    /// carries evidence forbidden by its derived activation/evaluation state.
    pub fn lint(tool: ToolInfo, files: Vec<FileReport>) -> Result<Self, ContractError> {
        if let Some(file) = files.iter().find(|file| file.checks.is_none()) {
            return Err(ContractError::LintFileMissingChecks {
                path: file.path.clone(),
            });
        }
        let mut findings = FindingSummary::default();
        let mut checks = CheckSummary::default();
        for file in &files {
            for check in file.checks().unwrap_or_default() {
                let inactive = check.selection == SelectionState::Unselected
                    || check.configuration == ConfigurationState::Disabled
                    || check.applicability == Applicability::NotApplicable;
                if inactive
                    && (!check.findings.is_empty()
                        || !check.evaluated_scopes.is_empty()
                        || !check.gaps.is_empty())
                {
                    return Err(ContractError::InvalidCheckEvidence {
                        path: file.path.clone(),
                        check_id: check.check_id,
                        reason: "inactive checks cannot carry findings, scopes, or gaps",
                    });
                }
                if check.evaluation() == EvaluationState::NotEvaluated && !check.findings.is_empty()
                {
                    return Err(ContractError::InvalidCheckEvidence {
                        path: file.path.clone(),
                        check_id: check.check_id,
                        reason: "not-evaluated checks cannot carry findings",
                    });
                }
                checks.total += 1;
                for finding in &check.findings {
                    findings.add(finding.severity);
                }
                match check.selection {
                    SelectionState::Selected => checks.selection.selected += 1,
                    SelectionState::Unselected => checks.selection.unselected += 1,
                }
                match check.configuration {
                    ConfigurationState::Enabled => checks.configuration.enabled += 1,
                    ConfigurationState::Disabled => checks.configuration.disabled += 1,
                }
                match check.applicability {
                    Applicability::Applicable => checks.applicability.applicable += 1,
                    Applicability::NotApplicable => checks.applicability.not_applicable += 1,
                }
                match check.evaluation() {
                    EvaluationState::Complete => checks.evaluation.complete += 1,
                    EvaluationState::Partial => checks.evaluation.partial += 1,
                    EvaluationState::NotEvaluated => checks.evaluation.not_evaluated += 1,
                }
                checks.gaps += check.gaps.len();
            }
        }
        Ok(Self {
            header: EnvelopeHeader::new(tool, "lint"),
            summary: LintSummary {
                files: files.len(),
                findings,
                checks,
            },
            files,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
struct DiffInputs {
    before: String,
    after: String,
}

#[derive(Debug, Clone, Serialize)]
struct DiffSummary {
    deltas: usize,
}

/// Current diff-command result envelope.
#[derive(Debug, Serialize)]
pub struct DiffEnvelope {
    #[serde(flatten)]
    header: EnvelopeHeader,
    inputs: DiffInputs,
    summary: DiffSummary,
    deltas: Vec<MetricDelta>,
}

impl DiffEnvelope {
    /// Construct a schema-valid diff envelope.
    pub fn new(
        tool: ToolInfo,
        before: impl Into<String>,
        after: impl Into<String>,
        deltas: Vec<MetricDelta>,
    ) -> Self {
        Self {
            header: EnvelopeHeader::new(tool, "diff"),
            inputs: DiffInputs {
                before: before.into(),
                after: after.into(),
            },
            summary: DiffSummary {
                deltas: deltas.len(),
            },
            deltas,
        }
    }
}
