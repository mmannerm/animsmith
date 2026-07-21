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

macro_rules! builtin_codes {
    (
        $kind:ident, $registry:ident, $docs:ident, $registry_doc:literal;
        $($name:ident => $value:literal,
            meaning = $meaning:literal,
            emitted_by = [$($emitter:literal),+ $(,)?]),+ $(,)?
    ) => {
        impl $kind {
            $(#[doc = $meaning] pub const $name: Self = Self($value);)+
        }

        #[doc = $registry_doc]
        pub const $registry: &[$kind] = &[$($kind::$name),+];

        #[cfg(test)]
        const $docs: &[(&str, &str, &[&str])] = &[
            $(($value, $meaning, &[$($emitter),+])),+
        ];
    };
}

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

/// Stable machine code for a unit of evaluated or missing work.
///
/// Built-in codes are constants. Custom checks may construct namespaced codes
/// without forcing animsmith's built-in registry to become a closed enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct EvaluationScopeCode(&'static str);

builtin_codes!(
    EvaluationScopeCode,
    BUILTIN_EVALUATION_SCOPE_CODES,
    BUILTIN_EVALUATION_SCOPE_CODE_DOCS,
    "Built-in evaluation-scope codes used by animsmith's catalog.";
    FIRST_FRAME_REST_DELTA => "first_frame_rest_delta",
        meaning = "The named clip's first-frame/rest-pose rotation evidence was evaluated.",
        emitted_by = ["bind-pose"],
    LOOP_SEAM => "loop_seam",
        meaning = "One named clip's positional loop seam was measured.",
        emitted_by = ["loop-seam"],
    FOOT_STANCE => "foot_stance",
        meaning = "Whole-clip prerequisites for stance analysis were evaluated.",
        emitted_by = ["foot-slide"],
    LEFT_FOOT_STANCE => "left_foot_stance",
        meaning = "The named clip's left foot/toe stance was evaluated.",
        emitted_by = ["foot-slide"],
    RIGHT_FOOT_STANCE => "right_foot_stance",
        meaning = "The named clip's right foot/toe stance was evaluated.",
        emitted_by = ["foot-slide"],
    ROOT_MOTION_SPEED => "root_motion_speed",
        meaning = "One named clip's root-motion speed was measured.",
        emitted_by = ["root-motion-speed"],
    MEMBER_EXISTENCE => "member_existence",
        meaning = "Configured gait-group members were checked for existence.",
        emitted_by = ["gait-group"],
    PHASE_MEASUREMENT => "phase_measurement",
        meaning = "One named clip's gait phase was measured or lacked usable evidence.",
        emitted_by = ["gait-group"],
    PHASE_COHERENCE => "phase_coherence",
        meaning = "One named gait group's measurable phases were compared.",
        emitted_by = ["gait-group"],
    TRAVEL_MODE => "travel_mode",
        meaning = "One named clip's in-place/root-motion declaration was judged.",
        emitted_by = ["in-place"],
    FRAME_GRID => "frame_grid",
        meaning = "The named clip's declared frame grid was evaluated.",
        emitted_by = ["fps"],
);

impl EvaluationScopeCode {
    /// Construct a stable custom scope code.
    ///
    /// Custom checks should use a namespaced value such as `acme:reference`.
    pub const fn custom(code: &'static str) -> Self {
        Self(code)
    }

    /// Return the serialized snake-case or namespaced code.
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for EvaluationScopeCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

/// A stable identifier for work that completed or could not be evaluated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EvaluationScope {
    /// Consumer-neutral work-unit code such as `member_existence`.
    pub code: EvaluationScopeCode,
    /// Optional subject within the check, such as a group or clip name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
}

impl EvaluationScope {
    /// Construct a whole-check scope.
    pub fn new(code: EvaluationScopeCode) -> Self {
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

builtin_codes!(
    CoverageGapCode,
    BUILTIN_COVERAGE_GAP_CODES,
    BUILTIN_COVERAGE_GAP_CODE_DOCS,
    "Built-in coverage-gap codes used by animsmith's catalog.";
    ROLES_UNRESOLVED => "roles_unresolved",
        meaning = "Required semantic rig roles were not resolved.",
        emitted_by = ["loop-seam", "root-motion-speed", "in-place", "foot-slide", "gait-group"],
    MEASUREMENT_UNAVAILABLE => "measurement_unavailable",
        meaning = "A required numeric measurement could not be produced or did not meet its evidence floor.",
        emitted_by = ["loop-seam", "root-motion-speed", "in-place", "foot-slide", "gait-group"],
    INSUFFICIENT_MEASURABLE_MEMBERS => "insufficient_measurable_members",
        meaning = "Fewer than two gait-group members produced usable phases.",
        emitted_by = ["gait-group"],
    MEMBERS_NOT_EVALUATED => "members_not_evaluated",
        meaning = "Some configured gait-group members did not produce usable phases.",
        emitted_by = ["gait-group"],
    INVALID_DECLARED_FPS => "invalid_declared_fps",
        meaning = "A declared frame rate was zero, negative, or non-finite.",
        emitted_by = ["fps"],
    INSUFFICIENT_ROTATION_EVIDENCE => "insufficient_rotation_evidence",
        meaning = "Too few usable rotation tracks existed for a bind-pose comparison.",
        emitted_by = ["bind-pose"],
);

impl CoverageGapCode {
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

/// Output evidence from one selected, enabled, applicable check.
///
/// The shared evaluation boundary derives coverage from completed scopes and
/// gaps and reports malformed evidence through [`EvaluationError`].
#[derive(Debug, Clone)]
pub struct CheckOutput {
    findings: Vec<Finding>,
    evaluated_scopes: Vec<EvaluationScope>,
    gaps: Vec<CoverageGap>,
}

impl CheckOutput {
    /// Collect findings, completed scopes, and coverage gaps through the one
    /// check-output construction path.
    ///
    /// Classification and validation happen when [`evaluate_checks`] projects
    /// this evidence into a [`CheckEvaluation`].
    pub fn from_coverage(
        findings: Vec<Finding>,
        evaluated_scopes: Vec<EvaluationScope>,
        gaps: Vec<CoverageGap>,
    ) -> Self {
        Self {
            findings,
            evaluated_scopes,
            gaps,
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
}

/// Final v2 record for one catalog check.
#[derive(Debug, Clone)]
pub struct CheckEvaluation {
    check_id: &'static str,
    selection: SelectionState,
    configuration: ConfigurationState,
    applicability: Applicability,
    output: CheckOutput,
}

impl CheckEvaluation {
    /// Construct a selected, enabled, applicable evaluation from validated
    /// check output.
    ///
    /// # Errors
    ///
    /// Returns an error when a nested finding names a different check.
    pub fn evaluated(check_id: &'static str, output: CheckOutput) -> Result<Self, EvaluationError> {
        if !output.gaps.is_empty()
            && output.evaluated_scopes.is_empty()
            && !output.findings.is_empty()
        {
            return Err(EvaluationError::InvalidCheckOutput {
                check_id,
                reason: "not-evaluated output cannot carry content findings",
            });
        }
        if let Some(finding) = output
            .findings
            .iter()
            .find(|finding| finding.check_id != check_id)
        {
            return Err(EvaluationError::FindingCheckIdMismatch {
                check_id,
                finding_check_id: finding.check_id,
            });
        }
        Ok(Self {
            check_id,
            selection: SelectionState::Selected,
            configuration: ConfigurationState::Enabled,
            applicability: Applicability::Applicable,
            output,
        })
    }

    /// Stable check id.
    pub fn check_id(&self) -> &'static str {
        self.check_id
    }

    /// Invocation selection state.
    pub fn selection(&self) -> SelectionState {
        self.selection
    }

    /// Configuration activation state.
    pub fn configuration(&self) -> ConfigurationState {
        self.configuration
    }

    /// Applicability to this document/configuration.
    pub fn applicability(&self) -> Applicability {
        self.applicability
    }

    /// Derive evaluation coverage from activation, completed scopes, and gaps.
    pub fn evaluation(&self) -> EvaluationState {
        if self.selection == SelectionState::Unselected
            || self.configuration == ConfigurationState::Disabled
            || self.applicability == Applicability::NotApplicable
        {
            EvaluationState::NotEvaluated
        } else if self.output.gaps.is_empty() {
            EvaluationState::Complete
        } else if self.output.evaluated_scopes.is_empty() {
            EvaluationState::NotEvaluated
        } else {
            EvaluationState::Partial
        }
    }

    /// Content findings emitted by evaluated work.
    pub fn findings(&self) -> &[Finding] {
        self.output.findings()
    }

    /// Stable identifiers for work that completed.
    pub fn evaluated_scopes(&self) -> &[EvaluationScope] {
        self.output.evaluated_scopes()
    }

    /// Typed reasons work was not evaluated.
    pub fn gaps(&self) -> &[CoverageGap] {
        self.output.gaps()
    }

    fn inactive(
        check_id: &'static str,
        selection: SelectionState,
        configuration: ConfigurationState,
        applicability: Applicability,
    ) -> Self {
        debug_assert!(
            selection == SelectionState::Unselected
                || configuration == ConfigurationState::Disabled
                || applicability == Applicability::NotApplicable
        );
        Self {
            check_id,
            selection,
            configuration,
            applicability,
            output: CheckOutput::from_coverage(Vec::new(), Vec::new(), Vec::new()),
        }
    }

    fn override_severity(&mut self, severity: SeveritySetting) {
        if let Some(severity) = severity.as_severity() {
            for finding in &mut self.output.findings {
                finding.severity = severity;
            }
        }
    }
}

impl Serialize for CheckEvaluation {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut fields = 6;
        fields += usize::from(!self.output.evaluated_scopes.is_empty());
        fields += usize::from(!self.output.gaps.is_empty());
        let mut state = serializer.serialize_struct("CheckEvaluation", fields)?;
        state.serialize_field("check_id", &self.check_id)?;
        state.serialize_field("selection", &self.selection)?;
        state.serialize_field("configuration", &self.configuration)?;
        state.serialize_field("applicability", &self.applicability)?;
        state.serialize_field("evaluation", &self.evaluation())?;
        state.serialize_field("findings", &self.output.findings)?;
        if !self.output.evaluated_scopes.is_empty() {
            state.serialize_field("evaluated_scopes", &self.output.evaluated_scopes)?;
        }
        if !self.output.gaps.is_empty() {
            state.serialize_field("gaps", &self.output.gaps)?;
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
    /// Check evidence violates the derived coverage-state invariants.
    #[error("check {check_id:?} emitted invalid output: {reason}")]
    InvalidCheckOutput {
        /// Stable id of the check that emitted malformed evidence.
        check_id: &'static str,
        /// Contract rule violated by the output.
        reason: &'static str,
    },
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
/// ids, malformed coverage evidence, or a nested finding whose id disagrees
/// with its parent check.
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
            records.push(CheckEvaluation::inactive(
                check.id(),
                selection_state,
                configuration,
                applicability,
            ));
            continue;
        }

        let mut evaluation = CheckEvaluation::evaluated(check.id(), check.evaluate(ctx))?;
        if let Some(setting) = setting {
            evaluation.override_severity(setting);
        }
        records.push(evaluation);
    }
    Ok(records)
}

#[cfg(test)]
mod docs_contract {
    use std::collections::BTreeSet;

    use super::{BUILTIN_COVERAGE_GAP_CODE_DOCS, BUILTIN_EVALUATION_SCOPE_CODE_DOCS};

    fn assert_reference_table(docs: &str, heading: &str, entries: &[(&str, &str, &[&str])]) {
        let section = docs
            .split_once(heading)
            .unwrap_or_else(|| panic!("missing reference heading {heading:?}"))
            .1;
        let documented = section
            .lines()
            .skip_while(|line| line.trim().is_empty())
            .take_while(|line| !line.trim().is_empty())
            .filter(|line| line.starts_with("| `"))
            .collect::<Vec<_>>();
        let expected = entries
            .iter()
            .map(|(code, meaning, emitted_by)| {
                assert!(
                    !meaning.trim().is_empty() && !meaning.contains(['\r', '\n']),
                    "{code} must have a one-line meaning"
                );
                let emitters = emitted_by
                    .iter()
                    .map(|check_id| format!("`{check_id}`"))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("| `{code}` | {meaning} | {emitters} |")
            })
            .collect::<Vec<_>>();

        assert_eq!(documented.len(), expected.len(), "row count for {heading}");
        let documented = documented.into_iter().collect::<BTreeSet<_>>();
        assert_eq!(
            documented.len(),
            expected.len(),
            "duplicate rows for {heading}"
        );
        let expected = expected.iter().map(String::as_str).collect::<BTreeSet<_>>();
        assert_eq!(documented, expected, "exact rows for {heading}");
    }

    #[test]
    fn output_docs_match_registered_builtin_evidence_codes_exactly() {
        let docs = include_str!("../../../docs/output.md");
        let crlf = docs.lines().collect::<Vec<_>>().join("\r\n");
        for line_endings in [docs, crlf.as_str()] {
            assert_reference_table(
                line_endings,
                "Built-in gap codes are:",
                BUILTIN_COVERAGE_GAP_CODE_DOCS,
            );
            assert_reference_table(
                line_endings,
                "Built-in completed/gap scope codes are:",
                BUILTIN_EVALUATION_SCOPE_CODE_DOCS,
            );
        }
    }
}
