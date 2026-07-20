//! Provisional v2 evaluation records.
//!
//! This module separates check activation, applicability, evaluation
//! coverage, content findings, and coverage gaps. It is intentionally
//! additive while the v2 contract is incubated with real embedders; the v1
//! [`crate::run_checks`] API remains available during the experiment.

use std::collections::BTreeSet;

use serde::Serialize;

use crate::check::{Check, CheckCtx, Readiness};
use crate::config::SeveritySetting;
use crate::finding::Finding;

/// Whether a check was selected for this invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectionState {
    /// Selected explicitly or through the default full catalog.
    Selected,
    /// Omitted by an explicit selection.
    Unselected,
}

/// Whether configuration enabled the check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigurationState {
    /// The check is enabled.
    Enabled,
    /// `severity = "off"` disabled the check.
    Disabled,
}

/// Whether a check applies to the supplied document and declarations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Applicability {
    /// The check has work for this document/configuration.
    Applicable,
    /// The check has no work for this document/configuration.
    NotApplicable,
}

/// How much applicable work was evaluated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvaluationState {
    /// All currently modelled work completed.
    Complete,
    /// Some work completed and some has a typed coverage gap.
    Partial,
    /// No applicable work was evaluated.
    NotEvaluated,
}

/// A stable identifier for work that completed or could not be evaluated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EvaluationScope {
    /// Consumer-neutral work-unit code such as `member_existence`.
    pub code: &'static str,
    /// Optional subject within the check, such as a group or clip name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
}

impl EvaluationScope {
    /// Construct a whole-check scope.
    pub fn new(code: &'static str) -> Self {
        Self {
            code,
            subject: None,
        }
    }

    /// Attach a subject identifier.
    pub fn subject(mut self, subject: impl Into<String>) -> Self {
        self.subject = Some(subject.into());
        self
    }
}

/// A typed reason applicable work could not be evaluated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CoverageGap {
    /// Stable machine code such as `roles_unresolved`.
    pub code: &'static str,
    /// Human-readable display text. Automation must use [`CoverageGap::code`].
    pub message: String,
    /// Optional work scope affected by the gap.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<EvaluationScope>,
}

impl CoverageGap {
    /// Construct a whole-check coverage gap.
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            scope: None,
        }
    }

    /// Attach the affected work scope.
    pub fn scope(mut self, scope: EvaluationScope) -> Self {
        self.scope = Some(scope);
        self
    }
}

/// Output produced by one selected, enabled, applicable check.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CheckOutput {
    /// Content judgements only; coverage diagnostics belong in `gaps`.
    pub findings: Vec<Finding>,
    /// Stable identifiers for work that completed cleanly or with findings.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub evaluated_scopes: Vec<EvaluationScope>,
    /// Work that could not be evaluated.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub gaps: Vec<CoverageGap>,
}

impl CheckOutput {
    /// Construct a complete output from content findings.
    pub fn complete(findings: Vec<Finding>) -> Self {
        Self {
            findings,
            evaluated_scopes: Vec::new(),
            gaps: Vec::new(),
        }
    }
}

/// Provisional v2 record for one catalog check.
#[derive(Debug, Clone, Serialize)]
pub struct CheckEvaluation {
    /// Stable check id.
    pub check_id: &'static str,
    /// Invocation selection state.
    pub selection: SelectionState,
    /// Configuration activation state.
    pub configuration: ConfigurationState,
    /// Applicability to this document/configuration.
    pub applicability: Applicability,
    /// Evaluation coverage.
    pub evaluation: EvaluationState,
    /// Content findings emitted by evaluated work.
    pub findings: Vec<Finding>,
    /// Stable identifiers for work that completed.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub evaluated_scopes: Vec<EvaluationScope>,
    /// Typed reasons work was not evaluated.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub gaps: Vec<CoverageGap>,
}

/// Catalog-selection policy for [`evaluate_checks`].
#[derive(Debug, Clone, Copy)]
pub enum CheckSelection<'a> {
    /// Select the whole supplied catalog.
    All,
    /// Select only the named ids. Unknown ids are a frontend error.
    Only(&'a BTreeSet<String>),
}

impl CheckSelection<'_> {
    fn contains(self, id: &str) -> bool {
        match self {
            Self::All => true,
            Self::Only(ids) => ids.contains(id),
        }
    }
}

/// Evaluate a full catalog into one record per check.
///
/// Selection and `severity = "off"` are recorded independently. Inactive
/// checks never execute, but their cheap readiness predicate still establishes
/// generic applicability. Coverage gaps are nonblocking evidence; callers own
/// any stricter policy.
pub fn evaluate_checks(
    ctx: &CheckCtx<'_>,
    checks: &[Box<dyn Check>],
    selection: CheckSelection<'_>,
) -> Vec<CheckEvaluation> {
    checks
        .iter()
        .map(|check| {
            let selection_state = if selection.contains(check.id()) {
                SelectionState::Selected
            } else {
                SelectionState::Unselected
            };
            let setting = ctx.config.check_settings(check.id()).severity;
            let configuration = if setting == Some(SeveritySetting::Off) {
                ConfigurationState::Disabled
            } else {
                ConfigurationState::Enabled
            };
            let readiness = check.readiness(ctx);
            let applicability = if matches!(readiness, Readiness::Idle) {
                Applicability::NotApplicable
            } else {
                Applicability::Applicable
            };

            if selection_state == SelectionState::Unselected
                || configuration == ConfigurationState::Disabled
                || applicability == Applicability::NotApplicable
            {
                return CheckEvaluation {
                    check_id: check.id(),
                    selection: selection_state,
                    configuration,
                    applicability,
                    evaluation: EvaluationState::NotEvaluated,
                    findings: Vec::new(),
                    evaluated_scopes: Vec::new(),
                    gaps: Vec::new(),
                };
            }

            match readiness {
                Readiness::Idle => unreachable!("not-applicable returned above"),
                Readiness::Skipped(reason) => CheckEvaluation {
                    check_id: check.id(),
                    selection: selection_state,
                    configuration,
                    applicability,
                    evaluation: EvaluationState::NotEvaluated,
                    findings: Vec::new(),
                    evaluated_scopes: Vec::new(),
                    // The retained v1 readiness API carries display text.
                    // v2's adapter owns the typed representation rather than
                    // changing that public enum out from under embedders.
                    gaps: vec![CoverageGap::new("roles_unresolved", reason)],
                },
                Readiness::Ready => {
                    let mut output = check.evaluate(ctx);
                    // Legacy `run` implementations may encode unavailable
                    // work as diagnostic findings. Preserve that evidence as
                    // a typed gap rather than either promoting it to content
                    // or misreporting the check as completed-clean. Apply the
                    // boundary here so custom `evaluate` implementations get
                    // the same protection.
                    let mut content_findings = Vec::with_capacity(output.findings.len());
                    for finding in std::mem::take(&mut output.findings) {
                        if finding.diagnostic {
                            output
                                .gaps
                                .push(CoverageGap::new("legacy_diagnostic", finding.message));
                        } else {
                            content_findings.push(finding);
                        }
                    }
                    output.findings = content_findings;
                    if let Some(severity) = setting.and_then(SeveritySetting::as_severity) {
                        for finding in &mut output.findings {
                            finding.severity = severity;
                        }
                    }
                    let evaluation = if output.gaps.is_empty() {
                        EvaluationState::Complete
                    } else if output.evaluated_scopes.is_empty() && output.findings.is_empty() {
                        EvaluationState::NotEvaluated
                    } else {
                        EvaluationState::Partial
                    };
                    CheckEvaluation {
                        check_id: check.id(),
                        selection: selection_state,
                        configuration,
                        applicability,
                        evaluation,
                        findings: output.findings,
                        evaluated_scopes: output.evaluated_scopes,
                        gaps: output.gaps,
                    }
                }
            }
        })
        .collect()
}
