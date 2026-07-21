//! Typed check-evaluation records.
//!
//! Selection, configuration, applicability, evaluation coverage, content
//! findings, and coverage gaps are independent dimensions. The types in this
//! module are the single execution/result boundary for both CLI and embedded
//! consumers.

use std::collections::BTreeSet;
use std::fmt;

use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

use crate::check::{Check, CheckCtx};
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
    /// All modelled work completed.
    Complete,
    /// Some modelled work completed and some has a typed coverage gap.
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

/// Stable machine code for an evaluation-coverage gap.
///
/// Built-in codes are constants. Custom checks may construct their own code;
/// embedders should namespace custom values so they cannot collide with future
/// built-ins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct CoverageGapCode(&'static str);

impl CoverageGapCode {
    /// Required rig roles could not be resolved.
    pub const ROLES_UNRESOLVED: Self = Self("roles_unresolved");
    /// A metric needed by declared work could not be produced.
    pub const MEASUREMENT_UNAVAILABLE: Self = Self("measurement_unavailable");
    /// Fewer than two gait-group members produced a phase measurement.
    pub const INSUFFICIENT_MEASURABLE_MEMBERS: Self = Self("insufficient_measurable_members");
    /// Some configured gait-group members did not produce a phase measurement.
    pub const MEMBERS_NOT_EVALUATED: Self = Self("members_not_evaluated");
    /// A declared frame rate was zero, negative, or non-finite.
    pub const INVALID_DECLARED_FPS: Self = Self("invalid_declared_fps");
    /// Too few usable rotation tracks existed for bind-pose comparison.
    pub const INSUFFICIENT_ROTATION_EVIDENCE: Self = Self("insufficient_rotation_evidence");

    /// Construct a stable custom code.
    ///
    /// Custom checks should use a namespaced value such as
    /// `acme:reference_unavailable`.
    pub const fn custom(code: &'static str) -> Self {
        Self(code)
    }

    /// Return the serialized snake-case or namespaced code.
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for CoverageGapCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

/// A typed reason applicable work could not be evaluated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CoverageGap {
    /// Stable machine code. Automation must not parse [`CoverageGap::message`].
    pub code: CoverageGapCode,
    /// Human-readable display text.
    pub message: String,
    /// Optional work scope affected by the gap.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<EvaluationScope>,
}

impl CoverageGap {
    /// Construct a whole-check coverage gap.
    pub fn new(code: CoverageGapCode, message: impl Into<String>) -> Self {
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

/// Validated output from one selected, enabled, applicable check.
///
/// Coverage state is derived from completed scopes and gaps rather than cached,
/// so the Rust value and serialized classification cannot drift apart.
#[derive(Debug, Clone)]
pub struct CheckOutput {
    findings: Vec<Finding>,
    evaluated_scopes: Vec<EvaluationScope>,
    gaps: Vec<CoverageGap>,
}

impl CheckOutput {
    /// Classify coverage from completed scopes and gaps.
    ///
    /// No gaps means complete work. Gaps plus completed scopes mean partial
    /// work. Gaps without completed scopes mean no applicable work completed.
    ///
    /// # Panics
    ///
    /// Panics if findings are supplied without a completed scope while gaps
    /// are present. Unevaluated work cannot produce content judgments.
    pub fn from_coverage(
        findings: Vec<Finding>,
        evaluated_scopes: Vec<EvaluationScope>,
        gaps: Vec<CoverageGap>,
    ) -> Self {
        if !gaps.is_empty() && evaluated_scopes.is_empty() {
            assert!(
                findings.is_empty(),
                "not-evaluated output cannot carry content findings"
            );
        }
        Self {
            findings,
            evaluated_scopes,
            gaps,
        }
    }

    /// Evaluation coverage represented by this output.
    pub fn evaluation(&self) -> EvaluationState {
        if self.gaps.is_empty() {
            EvaluationState::Complete
        } else if self.evaluated_scopes.is_empty() {
            EvaluationState::NotEvaluated
        } else {
            EvaluationState::Partial
        }
    }

    /// Content findings emitted by evaluated work.
    pub fn findings(&self) -> &[Finding] {
        &self.findings
    }

    /// Stable identifiers for work that completed.
    pub fn evaluated_scopes(&self) -> &[EvaluationScope] {
        &self.evaluated_scopes
    }

    /// Typed reasons work did not complete.
    pub fn gaps(&self) -> &[CoverageGap] {
        &self.gaps
    }

    fn into_parts(self) -> (Vec<Finding>, Vec<EvaluationScope>, Vec<CoverageGap>) {
        (self.findings, self.evaluated_scopes, self.gaps)
    }
}

/// Final v2 record for one catalog check.
#[derive(Debug, Clone)]
pub struct CheckEvaluation {
    /// Stable check id.
    pub check_id: &'static str,
    /// Invocation selection state.
    pub selection: SelectionState,
    /// Configuration activation state.
    pub configuration: ConfigurationState,
    /// Applicability to this document/configuration.
    pub applicability: Applicability,
    /// Content findings emitted by evaluated work.
    pub findings: Vec<Finding>,
    /// Stable identifiers for work that completed.
    pub evaluated_scopes: Vec<EvaluationScope>,
    /// Typed reasons work was not evaluated.
    pub gaps: Vec<CoverageGap>,
}

impl CheckEvaluation {
    /// Derive evaluation coverage from activation, completed scopes, and gaps.
    pub fn evaluation(&self) -> EvaluationState {
        if self.selection == SelectionState::Unselected
            || self.configuration == ConfigurationState::Disabled
            || self.applicability == Applicability::NotApplicable
        {
            EvaluationState::NotEvaluated
        } else if self.gaps.is_empty() {
            EvaluationState::Complete
        } else if self.evaluated_scopes.is_empty() {
            EvaluationState::NotEvaluated
        } else {
            EvaluationState::Partial
        }
    }
}

impl Serialize for CheckEvaluation {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut fields = 6;
        fields += usize::from(!self.evaluated_scopes.is_empty());
        fields += usize::from(!self.gaps.is_empty());
        let mut state = serializer.serialize_struct("CheckEvaluation", fields)?;
        state.serialize_field("check_id", &self.check_id)?;
        state.serialize_field("selection", &self.selection)?;
        state.serialize_field("configuration", &self.configuration)?;
        state.serialize_field("applicability", &self.applicability)?;
        state.serialize_field("evaluation", &self.evaluation())?;
        state.serialize_field("findings", &self.findings)?;
        if !self.evaluated_scopes.is_empty() {
            state.serialize_field("evaluated_scopes", &self.evaluated_scopes)?;
        }
        if !self.gaps.is_empty() {
            state.serialize_field("gaps", &self.gaps)?;
        }
        state.end()
    }
}

/// Catalog-selection policy for [`evaluate_checks`].
#[derive(Debug, Clone, Copy)]
pub enum CheckSelection<'a> {
    /// Select the whole supplied catalog.
    All,
    /// Select only the named ids.
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

/// Invalid catalog or check output supplied to [`evaluate_checks`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum EvaluationError {
    /// Two catalog entries used the same stable check id.
    #[error("duplicate check id {0:?}")]
    DuplicateCheckId(&'static str),
    /// Explicit selection named an id absent from the supplied catalog.
    #[error("unknown selected check id {0:?}")]
    UnknownSelection(String),
    /// A nested finding claimed a different check id than its parent record.
    #[error("check {check_id:?} emitted a finding for {finding_check_id:?}")]
    FindingCheckIdMismatch {
        /// Parent check id.
        check_id: &'static str,
        /// Mismatched nested finding id.
        finding_check_id: &'static str,
    },
}

/// Evaluate a full catalog into one record per check.
///
/// Selection and `severity = "off"` are recorded independently. Inactive
/// checks never evaluate, but their cheap applicability predicate still
/// establishes whether declared work exists. Coverage gaps are nonblocking
/// evidence; callers own any stricter policy.
///
/// # Errors
///
/// Returns an error for duplicate catalog ids, unknown explicitly selected
/// ids, or a nested finding whose id disagrees with its parent check.
pub fn evaluate_checks(
    ctx: &CheckCtx<'_>,
    checks: &[Box<dyn Check>],
    selection: CheckSelection<'_>,
) -> Result<Vec<CheckEvaluation>, EvaluationError> {
    let mut catalog_ids = BTreeSet::new();
    for check in checks {
        if !catalog_ids.insert(check.id()) {
            return Err(EvaluationError::DuplicateCheckId(check.id()));
        }
    }
    if let CheckSelection::Only(selected) = selection
        && let Some(unknown) = selected
            .iter()
            .find(|id| !catalog_ids.contains(id.as_str()))
    {
        return Err(EvaluationError::UnknownSelection(unknown.clone()));
    }

    let mut records = Vec::with_capacity(checks.len());
    for check in checks {
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
        let applicability = check.applicability(ctx);

        if selection_state == SelectionState::Unselected
            || configuration == ConfigurationState::Disabled
            || applicability == Applicability::NotApplicable
        {
            records.push(CheckEvaluation {
                check_id: check.id(),
                selection: selection_state,
                configuration,
                applicability,
                findings: Vec::new(),
                evaluated_scopes: Vec::new(),
                gaps: Vec::new(),
            });
            continue;
        }

        let (mut findings, evaluated_scopes, gaps) = check.evaluate(ctx).into_parts();
        for finding in &findings {
            if finding.check_id != check.id() {
                return Err(EvaluationError::FindingCheckIdMismatch {
                    check_id: check.id(),
                    finding_check_id: finding.check_id,
                });
            }
        }
        if let Some(severity) = setting.and_then(SeveritySetting::as_severity) {
            for finding in &mut findings {
                finding.severity = severity;
            }
        }
        records.push(CheckEvaluation {
            check_id: check.id(),
            selection: selection_state,
            configuration,
            applicability,
            findings,
            evaluated_scopes,
            gaps,
        });
    }
    Ok(records)
}
