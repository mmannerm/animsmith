//! Typed check-evaluation records.
//!
//! Selection, configuration, applicability, evaluation coverage, content
//! findings, and coverage gaps are independent dimensions. The types in this
//! module are the single execution/result boundary for both CLI and embedded
//! consumers.

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::fmt;

use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};

use crate::check::{Check, CheckCtx};
use crate::config::{ConfigValidationError, SeveritySetting};
use crate::finding::Finding;
use crate::prediction::{EnginePredictionV1, EnginePredictionV2, PredictionContractError};

/// One authoritative built-in evidence-code definition.
///
/// The `builtin_codes!` rows below own each built-in code's serialized
/// identity, meaning, and allowed emitters. Both runtime validation and the
/// output-documentation contract consume these definitions.
#[derive(Debug, Clone, Copy)]
struct BuiltinEvidenceCode {
    code: &'static str,
    #[cfg_attr(not(test), allow(dead_code))]
    meaning: &'static str,
    emitted_by: &'static [&'static str],
}

macro_rules! builtin_codes {
    (
        $kind:ident, $registry:ident, $definitions:ident, $error:ident, $registry_doc:literal;
        $($name:ident => $value:literal,
            meaning = $meaning:literal,
            emitted_by = [$($emitter:literal),+ $(,)?]),+ $(,)?
    ) => {
        impl $kind {
            $(#[doc = $meaning] pub const $name: Self = Self::from_static($value);)+
        }

        #[doc = $registry_doc]
        pub const $registry: &[$kind] = &[$($kind::$name),+];

        const $definitions: &[BuiltinEvidenceCode] = &[
            $(BuiltinEvidenceCode {
                code: $value,
                meaning: $meaning,
                emitted_by: &[$($emitter),+],
            }),+
        ];

        impl $kind {
            #[allow(dead_code)]
            fn builtin_definition(&self) -> Option<&'static BuiltinEvidenceCode> {
                $definitions
                    .iter()
                    .find(|definition| definition.code == self.as_str())
            }

            #[allow(dead_code)]
            fn validate_emitter(&self, check_id: &'static str) -> Result<(), EvaluationError> {
                if self
                    .builtin_definition()
                    .is_some_and(|definition| !definition.emitted_by.contains(&check_id))
                {
                    return Err(EvaluationError::$error {
                        check_id,
                        code: self.clone(),
                    });
                }
                Ok(())
            }
        }
    };
}

/// Whether a check was selected for this invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectionState {
    /// Selected explicitly or through the default full catalog.
    Selected,
    /// Omitted by an explicit selection.
    Unselected,
}

/// Whether configuration enabled the check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigurationState {
    /// The check is enabled.
    Enabled,
    /// The check is opt-in without an enabling severity, or
    /// `severity = "off"` disabled it.
    Disabled,
}

/// Whether a check applies to the supplied document and declarations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Applicability {
    /// The check has work for this document/configuration.
    Applicable,
    /// The check has no work for this document/configuration.
    NotApplicable,
}

/// How much applicable work was evaluated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EvaluationScopeCode(Cow<'static, str>);

builtin_codes!(
    EvaluationScopeCode,
    BUILTIN_EVALUATION_SCOPE_CODES,
    BUILTIN_EVALUATION_SCOPE_CODE_DEFINITIONS,
    BuiltinEvaluationScopeEmitterMismatch,
    "Complete built-in evaluation-scope vocabulary for consumers that need to inspect or allow-list animsmith's catalog codes. Custom checks may use additional namespaced codes.";
    FIRST_FRAME_REST_DELTA => "first_frame_rest_delta",
        meaning = "The named clip's first-frame/rest-pose rotation evidence was evaluated.",
        emitted_by = ["bind-pose"],
    LOOP_CLOSURE => "loop_closure",
        meaning = "One named clip's per-bone model-space pose closure was measured.",
        emitted_by = ["loop-closure"],
    DUPLICATE_LOOP_ENDPOINT => "duplicate_loop_endpoint",
        meaning = "One named clip's authored tracks were analyzed for redundant closing endpoint keys.",
        emitted_by = ["duplicate-loop-endpoint"],
    LOOP_SEAM => "loop_seam",
        meaning = "One named clip's positional loop seam was measured.",
        emitted_by = ["loop-seam"],
    LOOP_SEAM_VELOCITY => "loop_seam_velocity",
        meaning = "One named clip's per-bone model-space seam velocity continuity was measured.",
        emitted_by = ["loop-seam-vel"],
    LOOP_SEAM_ROTATION => "loop_seam_rotation",
        meaning = "One named clip's per-bone model-space angular seam velocity continuity was measured.",
        emitted_by = ["loop-seam-rot"],
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
        meaning = "Configured group members were checked for existence.",
        emitted_by = ["gait-group", "sync-group", "time-complement"],
    PHASE_MEASUREMENT => "phase_measurement",
        meaning = "One named clip's gait phase was measured or lacked usable evidence.",
        emitted_by = ["gait-group", "time-complement"],
    PHASE_COHERENCE => "phase_coherence",
        meaning = "One named group's measurable gait phases were compared.",
        emitted_by = ["gait-group", "time-complement"],
    SYNC_MEMBER_MEASUREMENT => "sync_member_measurement",
        meaning = "One named same-time sync-group member's timing evidence was measured.",
        emitted_by = ["sync-group"],
    SYNC_COMPATIBILITY => "sync_compatibility",
        meaning = "One named same-time sync group had compatible member timing evidence compared.",
        emitted_by = ["sync-group"],
    TRAVEL_MODE => "travel_mode",
        meaning = "One named clip's XZ movement-owner declaration was judged.",
        emitted_by = ["in-place"],
    FRAME_GRID => "frame_grid",
        meaning = "The named clip's declared frame grid was evaluated.",
        emitted_by = ["fps"],
    REQUIRED_BONE_PRESENCE => "required_bone_presence",
        meaning = "Configured structural skeleton-bone presence requirements were evaluated.",
        emitted_by = ["required-bones"],
    SELECTED_NODE_REST_SCALE => "selected_node_rest_scale",
        meaning = "One configured source-node selector resolved and its effective rest-world linear scale was evaluated.",
        emitted_by = ["rest-world-scale"],
    ANIMATION_ASSET_LABEL => "animation_asset_label",
        meaning = "One source animation index was projected to the selected engine profile's canonical asset-label selector.",
        emitted_by = ["engine-addressability"],
    ANIMATION_ASSET_LABEL_INVENTORY => "animation_asset_label_inventory",
        meaning = "Complete source-animation inventory required for asset-label prediction was unavailable.",
        emitted_by = ["engine-addressability"],
);

// Private evidence-vocabulary authority for built-in emitters implemented
// outside core. Owning crates publish their runnable check catalogs.
#[cfg(test)]
const EXTERNAL_BUILTIN_CHECK_IDS: &[&str] = &["engine-addressability"];

impl EvaluationScopeCode {
    const fn from_static(code: &'static str) -> Self {
        Self(Cow::Borrowed(code))
    }

    /// Construct a stable custom scope code.
    ///
    /// Custom checks should use a namespaced value such as `acme:reference`.
    pub const fn custom(code: &'static str) -> Self {
        Self(Cow::Borrowed(code))
    }

    /// Return the serialized snake-case or namespaced code.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for EvaluationScopeCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A stable identifier for work that completed or could not be evaluated.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
    BUILTIN_COVERAGE_GAP_CODE_DEFINITIONS,
    BuiltinCoverageGapEmitterMismatch,
    "Complete built-in coverage-gap vocabulary for consumers that need to inspect or allow-list animsmith's catalog codes. Custom checks may use additional namespaced codes.";
    ROLES_UNRESOLVED => "roles_unresolved",
        meaning = "Required semantic rig roles were not resolved.",
        emitted_by = ["loop-seam", "root-motion-speed", "in-place", "foot-slide", "gait-group", "time-complement"],
    MEASUREMENT_UNAVAILABLE => "measurement_unavailable",
        meaning = "A required numeric measurement could not be produced or did not meet its evidence floor.",
        emitted_by = ["loop-closure", "duplicate-loop-endpoint", "loop-seam", "loop-seam-vel", "loop-seam-rot", "root-motion-speed", "in-place", "foot-slide", "gait-group", "sync-group", "time-complement", "rest-world-scale"],
    SKELETON_UNAVAILABLE => "skeleton_unavailable",
        meaning = "Required skeleton presence work could not run because the file has no usable skeleton.",
        emitted_by = ["required-bones"],
    NODE_SELECTOR_NO_MATCH => "node_selector_no_match",
        meaning = "A configured source-node selector matched no named source node.",
        emitted_by = ["rest-world-scale"],
    NODE_SELECTOR_AMBIGUOUS => "node_selector_ambiguous",
        meaning = "A configured source-node selector matched more than one named source node.",
        emitted_by = ["rest-world-scale"],
    INSUFFICIENT_MEASURABLE_MEMBERS => "insufficient_measurable_members",
        meaning = "Fewer than two configured group members produced usable comparison evidence.",
        emitted_by = ["gait-group", "sync-group", "time-complement"],
    MEMBERS_NOT_EVALUATED => "members_not_evaluated",
        meaning = "Some configured group members did not produce usable comparison evidence.",
        emitted_by = ["gait-group", "sync-group", "time-complement"],
    INVALID_DECLARED_FPS => "invalid_declared_fps",
        meaning = "A declared frame rate was zero, negative, or non-finite.",
        emitted_by = ["fps"],
    SYNC_FRAME_GRID_UNAVAILABLE => "sync_frame_grid_unavailable",
        meaning = "A same-time sync-group member lacks usable declared frame-grid evidence.",
        emitted_by = ["sync-group"],
    INSUFFICIENT_ROTATION_EVIDENCE => "insufficient_rotation_evidence",
        meaning = "Too few usable rotation tracks existed for a bind-pose comparison.",
        emitted_by = ["bind-pose"],
);

impl CoverageGapCode {
    const fn from_static(code: &'static str) -> Self {
        Self(code)
    }

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
    engine_prediction: Option<EnginePredictionEvidence>,
}

/// Versioned prediction attachment kept private so standalone V1 contracts
/// retain their existing public API while current lint can carry V2 evidence.
#[derive(Debug, Clone)]
enum EnginePredictionEvidence {
    V1(EnginePredictionV1),
    V2(EnginePredictionV2),
}

impl EnginePredictionEvidence {
    fn scopes(&self) -> Vec<&EvaluationScope> {
        match self {
            Self::V1(prediction) => prediction
                .facets()
                .iter()
                .map(|facet| facet.scope())
                .collect(),
            Self::V2(prediction) => prediction
                .facets()
                .iter()
                .map(|facet| facet.scope())
                .collect(),
        }
    }

    fn scope(&self, index: usize) -> &EvaluationScope {
        match self {
            Self::V1(prediction) => prediction.facets()[index].scope(),
            Self::V2(prediction) => prediction.facets()[index].scope(),
        }
    }
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
            engine_prediction: None,
        }
    }

    /// Attach one engine-prediction record to this check output.
    ///
    /// The shared evaluation boundary validates facet/scope/finding
    /// relationships after it knows the authoritative parent check id.
    pub fn with_engine_prediction(mut self, prediction: EnginePredictionV1) -> Self {
        self.engine_prediction = Some(EnginePredictionEvidence::V1(prediction));
        self
    }

    /// Attach a bounded-overflow V2 engine-prediction record.
    pub fn with_engine_prediction_v2(mut self, prediction: EnginePredictionV2) -> Self {
        self.engine_prediction = Some(EnginePredictionEvidence::V2(prediction));
        self
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

    /// Engine-prediction facets emitted by this check, when any.
    pub const fn engine_prediction(&self) -> Option<&EnginePredictionV1> {
        match self.engine_prediction.as_ref() {
            Some(EnginePredictionEvidence::V1(prediction)) => Some(prediction),
            Some(EnginePredictionEvidence::V2(_)) | None => None,
        }
    }

    /// V2 prediction attachment used by the current output-v13 lint path.
    pub const fn engine_prediction_v2(&self) -> Option<&EnginePredictionV2> {
        match self.engine_prediction.as_ref() {
            Some(EnginePredictionEvidence::V2(prediction)) => Some(prediction),
            Some(EnginePredictionEvidence::V1(_)) | None => None,
        }
    }

    fn has_required_prediction_unavailable(&self) -> bool {
        self.engine_prediction
            .as_ref()
            .is_some_and(|prediction| match prediction {
                EnginePredictionEvidence::V1(prediction) => prediction.has_required_unavailable(),
                EnginePredictionEvidence::V2(prediction) => prediction.has_required_unavailable(),
            })
    }

    fn has_missing_work(&self) -> bool {
        !self.gaps.is_empty() || self.has_required_prediction_unavailable()
    }
}

/// Borrowed coverage-gap fields used by the shared producer/readback
/// evaluation validator.
pub(crate) struct CheckEvaluationGapRef<'a> {
    pub(crate) code: &'a str,
    pub(crate) scope: Option<&'a EvaluationScope>,
}

pub(crate) struct CheckEvaluationValidationInput<'a> {
    pub(crate) check_id: &'a str,
    pub(crate) selection: SelectionState,
    pub(crate) configuration: ConfigurationState,
    pub(crate) applicability: Applicability,
    pub(crate) finding_check_ids: &'a [&'a str],
    pub(crate) evaluated_scopes: &'a [EvaluationScope],
    pub(crate) gaps: &'a [CheckEvaluationGapRef<'a>],
    pub(crate) prediction_scopes: &'a [&'a EvaluationScope],
    pub(crate) has_prediction: bool,
    pub(crate) prediction_has_required_unavailable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CheckEvaluationValidationError {
    InvalidCheckId,
    InvalidOutput(&'static str),
    FindingCheckIdMismatch { finding_index: usize },
    EvaluatedScopeEmitterMismatch { scope_index: usize },
    GapScopeEmitterMismatch { gap_index: usize },
    GapEmitterMismatch { gap_index: usize },
    PredictionScopeEmitterMismatch { facet_index: usize },
}

impl CheckEvaluationValidationError {
    pub(crate) const fn reason(self) -> &'static str {
        match self {
            Self::InvalidCheckId => "check id cannot be empty",
            Self::InvalidOutput(reason) => reason,
            Self::FindingCheckIdMismatch { .. } => "finding check_id must match its parent check",
            Self::EvaluatedScopeEmitterMismatch { .. } => {
                "evaluated scope code is invalid for its parent check"
            }
            Self::GapScopeEmitterMismatch { .. } => {
                "coverage gap scope code is invalid for its parent check"
            }
            Self::GapEmitterMismatch { .. } => "coverage gap code is invalid for its parent check",
            Self::PredictionScopeEmitterMismatch { .. } => {
                "prediction facet scope code is invalid for its parent check"
            }
        }
    }
}

/// Validate the sole check-evaluation lifecycle and derive its serialized
/// coverage state. Both producer construction and output-v11 readback use this
/// authority.
pub(crate) fn validate_and_derive_check_evaluation(
    input: CheckEvaluationValidationInput<'_>,
) -> Result<EvaluationState, CheckEvaluationValidationError> {
    let CheckEvaluationValidationInput {
        check_id,
        selection,
        configuration,
        applicability,
        finding_check_ids,
        evaluated_scopes,
        gaps,
        prediction_scopes,
        has_prediction,
        prediction_has_required_unavailable,
    } = input;
    if check_id.is_empty() {
        return Err(CheckEvaluationValidationError::InvalidCheckId);
    }
    if let Some(finding_index) = finding_check_ids
        .iter()
        .position(|finding_check_id| *finding_check_id != check_id)
    {
        return Err(CheckEvaluationValidationError::FindingCheckIdMismatch { finding_index });
    }
    for (scope_index, scope) in evaluated_scopes.iter().enumerate() {
        if scope.code.as_str().is_empty()
            || scope
                .code
                .builtin_definition()
                .is_some_and(|definition| !definition.emitted_by.contains(&check_id))
        {
            return Err(
                CheckEvaluationValidationError::EvaluatedScopeEmitterMismatch { scope_index },
            );
        }
    }
    for (gap_index, gap) in gaps.iter().enumerate() {
        if gap.code.is_empty()
            || BUILTIN_COVERAGE_GAP_CODE_DEFINITIONS
                .iter()
                .find(|definition| definition.code == gap.code)
                .is_some_and(|definition| !definition.emitted_by.contains(&check_id))
        {
            return Err(CheckEvaluationValidationError::GapEmitterMismatch { gap_index });
        }
        if gap.scope.is_some_and(|scope| {
            scope.code.as_str().is_empty()
                || scope
                    .code
                    .builtin_definition()
                    .is_some_and(|definition| !definition.emitted_by.contains(&check_id))
        }) {
            return Err(CheckEvaluationValidationError::GapScopeEmitterMismatch { gap_index });
        }
    }
    for (facet_index, scope) in prediction_scopes.iter().enumerate() {
        if scope.code.as_str().is_empty()
            || scope
                .code
                .builtin_definition()
                .is_some_and(|definition| !definition.emitted_by.contains(&check_id))
        {
            return Err(
                CheckEvaluationValidationError::PredictionScopeEmitterMismatch { facet_index },
            );
        }
    }

    let inactive = selection == SelectionState::Unselected
        || configuration == ConfigurationState::Disabled
        || applicability == Applicability::NotApplicable;
    if inactive {
        if !finding_check_ids.is_empty()
            || !evaluated_scopes.is_empty()
            || !gaps.is_empty()
            || has_prediction
        {
            return Err(CheckEvaluationValidationError::InvalidOutput(
                "inactive check must have empty output",
            ));
        }
        return Ok(EvaluationState::NotEvaluated);
    }

    let missing = !gaps.is_empty() || prediction_has_required_unavailable;
    let derived = if !missing {
        EvaluationState::Complete
    } else if evaluated_scopes.is_empty() {
        EvaluationState::NotEvaluated
    } else {
        EvaluationState::Partial
    };
    if derived == EvaluationState::NotEvaluated && !finding_check_ids.is_empty() {
        return Err(CheckEvaluationValidationError::InvalidOutput(
            "not-evaluated output cannot carry content findings",
        ));
    }
    Ok(derived)
}

/// Final output-v13 record for one catalog check.
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
    /// Returns an error for an empty check id, malformed coverage codes,
    /// built-in evidence emitted by an undeclared check, or when a nested
    /// finding names a different check.
    pub fn evaluated(check_id: &'static str, output: CheckOutput) -> Result<Self, EvaluationError> {
        let gap_refs = output
            .gaps
            .iter()
            .map(|gap| CheckEvaluationGapRef {
                code: gap.code.as_str(),
                scope: gap.scope.as_ref(),
            })
            .collect::<Vec<_>>();
        let finding_check_ids = output
            .findings
            .iter()
            .map(|finding| finding.check_id)
            .collect::<Vec<_>>();
        let prediction_scopes = output
            .engine_prediction
            .as_ref()
            .map_or_else(Vec::new, EnginePredictionEvidence::scopes);
        validate_and_derive_check_evaluation(CheckEvaluationValidationInput {
            check_id,
            selection: SelectionState::Selected,
            configuration: ConfigurationState::Enabled,
            applicability: Applicability::Applicable,
            finding_check_ids: &finding_check_ids,
            evaluated_scopes: &output.evaluated_scopes,
            gaps: &gap_refs,
            prediction_scopes: &prediction_scopes,
            has_prediction: output.engine_prediction.is_some(),
            prediction_has_required_unavailable: output.has_missing_work()
                && output.gaps.is_empty(),
        })
        .map_err(|error| match error {
            CheckEvaluationValidationError::InvalidCheckId => {
                EvaluationError::InvalidCheckId(check_id)
            }
            CheckEvaluationValidationError::InvalidOutput(reason) => {
                EvaluationError::InvalidCheckOutput { check_id, reason }
            }
            CheckEvaluationValidationError::FindingCheckIdMismatch { finding_index } => {
                EvaluationError::FindingCheckIdMismatch {
                    check_id,
                    finding_check_id: output.findings[finding_index].check_id,
                }
            }
            CheckEvaluationValidationError::EvaluatedScopeEmitterMismatch { scope_index } => {
                let code = output.evaluated_scopes[scope_index].code.clone();
                if code.as_str().is_empty() {
                    EvaluationError::InvalidCheckOutput {
                        check_id,
                        reason: "evaluated scope code cannot be empty",
                    }
                } else {
                    EvaluationError::BuiltinEvaluationScopeEmitterMismatch { check_id, code }
                }
            }
            CheckEvaluationValidationError::GapScopeEmitterMismatch { gap_index } => {
                let code = output.gaps[gap_index]
                    .scope
                    .as_ref()
                    .expect("the validator reports only present gap scopes")
                    .code
                    .clone();
                if code.as_str().is_empty() {
                    EvaluationError::InvalidCheckOutput {
                        check_id,
                        reason: "coverage gap scope code cannot be empty",
                    }
                } else {
                    EvaluationError::BuiltinEvaluationScopeEmitterMismatch { check_id, code }
                }
            }
            CheckEvaluationValidationError::GapEmitterMismatch { gap_index } => {
                let code = output.gaps[gap_index].code;
                if code.as_str().is_empty() {
                    EvaluationError::InvalidCheckOutput {
                        check_id,
                        reason: "coverage gap code cannot be empty",
                    }
                } else {
                    EvaluationError::BuiltinCoverageGapEmitterMismatch { check_id, code }
                }
            }
            CheckEvaluationValidationError::PredictionScopeEmitterMismatch { facet_index } => {
                let code = output
                    .engine_prediction
                    .as_ref()
                    .expect("the validator reports only present prediction scopes")
                    .scope(facet_index)
                    .code
                    .clone();
                if code.as_str().is_empty() {
                    EvaluationError::InvalidCheckOutput {
                        check_id,
                        reason: "prediction facet scope code cannot be empty",
                    }
                } else {
                    EvaluationError::BuiltinEvaluationScopeEmitterMismatch { check_id, code }
                }
            }
        })?;
        if let Some(prediction) = &output.engine_prediction {
            match prediction {
                EnginePredictionEvidence::V1(prediction) => prediction.validate_for_check(
                    check_id,
                    &output.evaluated_scopes,
                    &output.gaps,
                    &output.findings,
                )?,
                EnginePredictionEvidence::V2(prediction) => prediction.validate_for_check(
                    check_id,
                    &output.evaluated_scopes,
                    &output.gaps,
                    &output.findings,
                )?,
            }
        } else if output
            .findings
            .iter()
            .any(|finding| finding.prediction_scope.is_some())
        {
            return Err(EvaluationError::InvalidCheckOutput {
                check_id,
                reason: "finding has prediction_scope without an engine prediction",
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

    /// Derive evaluation coverage from activation, completed scopes, ordinary
    /// gaps, and required-unavailable prediction facets.
    pub fn evaluation(&self) -> EvaluationState {
        if self.selection == SelectionState::Unselected
            || self.configuration == ConfigurationState::Disabled
            || self.applicability == Applicability::NotApplicable
        {
            EvaluationState::NotEvaluated
        } else if !self.output.has_missing_work() {
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

    /// Engine-prediction facets attached to this check, when any.
    pub const fn engine_prediction(&self) -> Option<&EnginePredictionV1> {
        self.output.engine_prediction()
    }

    /// V2 engine-prediction attachment used by output-v13 lint reports.
    pub const fn engine_prediction_v2(&self) -> Option<&EnginePredictionV2> {
        self.output.engine_prediction_v2()
    }

    /// Whether this check has unavailable prediction work under either
    /// versioned prediction contract.
    pub fn has_required_prediction_unavailable(&self) -> bool {
        self.output.has_required_prediction_unavailable()
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
        fields += usize::from(self.output.engine_prediction.is_some());
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
        if let Some(prediction) = &self.output.engine_prediction {
            match prediction {
                EnginePredictionEvidence::V1(prediction) => {
                    state.serialize_field("prediction", prediction)?;
                }
                EnginePredictionEvidence::V2(prediction) => {
                    state.serialize_field("prediction", prediction)?;
                }
            }
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
    /// The supplied programmatic configuration contains an invalid value.
    #[error("invalid configuration: {0}")]
    InvalidConfiguration(#[from] ConfigValidationError),
    /// Engine-prediction evidence violates its wire or lifecycle contract.
    #[error("invalid engine prediction: {0}")]
    InvalidPrediction(#[from] PredictionContractError),
    /// A catalog or directly constructed check record used an empty id.
    #[error("check id cannot be empty")]
    InvalidCheckId(&'static str),
    /// Two catalog entries used the same stable check id.
    #[error("duplicate check id {0:?}")]
    DuplicateCheckId(&'static str),
    /// Explicit selection named an id absent from the supplied catalog.
    #[error("unknown selected check id {0:?}")]
    UnknownSelection(String),
    /// A V2 production rule did not emit exactly its preallocated slots.
    #[error(
        "check {check_id:?} emitted {emitted} V2 prediction facets after receiving {allocated} allocated slots"
    )]
    PredictionAllocationMismatch {
        /// Stable id of the production rule.
        check_id: &'static str,
        /// Candidate plus summary slots assigned before evaluation.
        allocated: usize,
        /// Facets actually returned by the rule.
        emitted: usize,
    },
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
    /// A built-in completed or missing-work scope came from an undeclared check.
    #[error("check {check_id:?} cannot emit built-in evaluation scope {code}")]
    BuiltinEvaluationScopeEmitterMismatch {
        /// Stable id of the check that emitted the scope.
        check_id: &'static str,
        /// Built-in scope code that does not declare this emitter.
        code: EvaluationScopeCode,
    },
    /// A built-in coverage-gap code came from an undeclared check.
    #[error("check {check_id:?} cannot emit built-in coverage gap {code}")]
    BuiltinCoverageGapEmitterMismatch {
        /// Stable id of the check that emitted the gap.
        check_id: &'static str,
        /// Built-in gap code that does not declare this emitter.
        code: CoverageGapCode,
    },
}

/// Apply the shared lint gate policy to validated check records.
///
/// Allowed check ids suppress content findings only. Required-unavailable
/// prediction facets always block, independent of severity or allow policy.
pub fn lint_requires_failure(
    checks: &[CheckEvaluation],
    fail_at: crate::finding::Severity,
    allowed_content_check_ids: &BTreeSet<String>,
) -> bool {
    checks.iter().any(|check| {
        check.has_required_prediction_unavailable()
            || check.findings().iter().any(|finding| {
                finding.severity >= fail_at && !allowed_content_check_ids.contains(finding.check_id)
            })
    })
}

/// Evaluate a full catalog into one record per check.
///
/// Selection and configuration are recorded independently. A check can be
/// disabled by `severity = "off"` or by its opt-in default; an explicit
/// `note`, `warn`, or `error` severity enables an opt-in check. Inactive checks
/// never evaluate, but their cheap applicability predicate still establishes
/// whether declared work exists. Coverage gaps are nonblocking evidence;
/// callers own any stricter policy.
///
/// # Errors
///
/// Returns an error for invalid directly constructed configuration, empty or
/// duplicate catalog ids, unknown explicitly selected ids, malformed coverage
/// evidence, built-in evidence emitted by an undeclared check, or a nested
/// finding whose id disagrees with its parent check. Configuration validation
/// runs before catalog inspection or check evaluation.
pub fn evaluate_checks(
    ctx: &CheckCtx<'_>,
    checks: &[Box<dyn Check + '_>],
    selection: CheckSelection<'_>,
) -> Result<Vec<CheckEvaluation>, EvaluationError> {
    ctx.config.validate()?;

    let mut catalog_ids = BTreeSet::new();
    for check in checks {
        if check.id().is_empty() {
            return Err(EvaluationError::InvalidCheckId(check.id()));
        }
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
        let configuration = match setting {
            Some(SeveritySetting::Off) => ConfigurationState::Disabled,
            Some(_) => ConfigurationState::Enabled,
            None if check.enabled_by_default() => ConfigurationState::Enabled,
            None => ConfigurationState::Disabled,
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

/// Current-output V2 evaluation: reserve every prediction facet in catalog
/// order before any selected check evaluates. Historical V1 callers retain
/// [`evaluate_checks`].
pub fn evaluate_checks_v2(
    ctx: &CheckCtx<'_>,
    checks: &[Box<dyn Check + '_>],
    selection: CheckSelection<'_>,
) -> Result<Vec<CheckEvaluation>, EvaluationError> {
    ctx.config.validate()?;
    let mut ids = BTreeSet::new();
    for check in checks {
        if check.id().is_empty() {
            return Err(EvaluationError::InvalidCheckId(check.id()));
        }
        if !ids.insert(check.id()) {
            return Err(EvaluationError::DuplicateCheckId(check.id()));
        }
    }
    if let CheckSelection::Only(selected) = selection
        && let Some(unknown) = selected.iter().find(|id| !ids.contains(id.as_str()))
    {
        return Err(EvaluationError::UnknownSelection(unknown.clone()));
    }
    let states = checks
        .iter()
        .map(|check| {
            let selected = selection.contains(check.id());
            let setting = ctx.config.check_settings(check.id()).severity;
            let configuration = match setting {
                Some(SeveritySetting::Off) => ConfigurationState::Disabled,
                Some(_) => ConfigurationState::Enabled,
                None if check.enabled_by_default() => ConfigurationState::Enabled,
                None => ConfigurationState::Disabled,
            };
            (selected, setting, configuration, check.applicability(ctx))
        })
        .collect::<Vec<_>>();
    let demands = checks
        .iter()
        .zip(&states)
        .filter(|(_, (selected, _, configuration, applicability))| {
            *selected
                && *configuration == ConfigurationState::Enabled
                && *applicability == Applicability::Applicable
        })
        .map(|(check, _)| {
            crate::PredictionRuleDemandV2::new(check.id(), check.prediction_facet_demand_v2(ctx))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let allocations = crate::allocate_prediction_facets_v2(&demands)?;
    let mut records = Vec::with_capacity(checks.len());
    for (check, (selected, setting, configuration, applicability)) in checks.iter().zip(states) {
        if !selected
            || configuration == ConfigurationState::Disabled
            || applicability == Applicability::NotApplicable
        {
            records.push(CheckEvaluation::inactive(
                check.id(),
                if selected {
                    SelectionState::Selected
                } else {
                    SelectionState::Unselected
                },
                configuration,
                applicability,
            ));
            continue;
        }
        let allocation = allocations
            .iter()
            .find(|allocation| allocation.rule_id() == check.id())
            .copied()
            .expect("active check was allocated");
        let output = check.evaluate_with_prediction_allocation_v2(ctx, allocation);
        let emitted = output
            .engine_prediction_v2()
            .map_or(0, |prediction| prediction.facets().len());
        if emitted != allocation.emitted_slots() {
            return Err(EvaluationError::PredictionAllocationMismatch {
                check_id: check.id(),
                allocated: allocation.emitted_slots(),
                emitted,
            });
        }
        let mut evaluation = CheckEvaluation::evaluated(check.id(), output)?;
        if let Some(setting) = setting {
            evaluation.override_severity(setting);
        }
        records.push(evaluation);
    }
    Ok(records)
}

#[cfg(test)]
mod authority_contract {
    use std::cell::RefCell;
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};
    use std::rc::Rc;

    use super::{
        BUILTIN_COVERAGE_GAP_CODE_DEFINITIONS, BUILTIN_EVALUATION_SCOPE_CODE_DEFINITIONS,
        BuiltinEvidenceCode, CheckEvaluation, CheckOutput, CoverageGap, CoverageGapCode,
        EXTERNAL_BUILTIN_CHECK_IDS, EvaluationError, EvaluationScope, EvaluationScopeCode,
        EvaluationState, lint_requires_failure,
    };
    use crate::InputIdentity;
    use crate::finding::{Finding, Severity};
    use crate::prediction::{
        EnginePredictionBasisV1, EnginePredictionFacetV1, EnginePredictionFacetV2,
        EnginePredictionV1, EnginePredictionV2, PredictionBasisReferenceV1,
        PredictionProvenanceIdentityV1, PredictionProvenanceIdentityV2, PredictionScalarV1,
        PredictionUnavailableReasonV1, PredictionUnavailableReasonV2,
    };
    use crate::{
        Applicability, Check, CheckCtx, CheckSelection, Config, Document, MetricGrids,
        PredictionFacetDemandV2, PredictionRuleAllocationV2, ResolvedRoles, evaluate_checks_v2,
    };

    struct AllocatedSyntheticCheck {
        id: &'static str,
        demand: usize,
        applicable: bool,
        allocations: Rc<RefCell<Vec<(String, usize, bool)>>>,
        constructed_candidates: Rc<RefCell<usize>>,
    }

    struct EmptyAllocatedCheck;

    impl Check for EmptyAllocatedCheck {
        fn id(&self) -> &'static str {
            "test:empty-allocated"
        }

        fn evaluate(&self, _ctx: &CheckCtx<'_>) -> CheckOutput {
            panic!("V2 orchestration must call the allocation-aware hook")
        }

        fn prediction_facet_demand_v2(&self, _ctx: &CheckCtx<'_>) -> PredictionFacetDemandV2 {
            PredictionFacetDemandV2::Exact(1)
        }

        fn evaluate_with_prediction_allocation_v2(
            &self,
            _ctx: &CheckCtx<'_>,
            _allocation: PredictionRuleAllocationV2<'_>,
        ) -> CheckOutput {
            CheckOutput::from_coverage(Vec::new(), Vec::new(), Vec::new())
        }
    }

    impl Check for AllocatedSyntheticCheck {
        fn id(&self) -> &'static str {
            self.id
        }

        fn applicability(&self, _ctx: &CheckCtx<'_>) -> Applicability {
            if self.applicable {
                Applicability::Applicable
            } else {
                Applicability::NotApplicable
            }
        }

        fn evaluate(&self, _ctx: &CheckCtx<'_>) -> CheckOutput {
            panic!("V2 orchestration must call the allocation-aware hook")
        }

        fn prediction_facet_demand_v2(&self, _ctx: &CheckCtx<'_>) -> PredictionFacetDemandV2 {
            PredictionFacetDemandV2::Exact(self.demand)
        }

        fn evaluate_with_prediction_allocation_v2(
            &self,
            _ctx: &CheckCtx<'_>,
            allocation: PredictionRuleAllocationV2<'_>,
        ) -> CheckOutput {
            self.allocations.borrow_mut().push((
                self.id.to_owned(),
                allocation.candidate_capacity(),
                allocation.summary_required(),
            ));
            // This represents actual candidate construction in a production
            // rule: omitted capacity must not be evaluated post hoc.
            *self.constructed_candidates.borrow_mut() += allocation.candidate_capacity();
            let basis = || {
                EnginePredictionBasisV1::new(vec![
                    PredictionBasisReferenceV1::profile_fact("animation_addressability").unwrap(),
                ])
                .unwrap()
            };
            let mut scopes = Vec::with_capacity(allocation.candidate_capacity());
            let mut facets = Vec::with_capacity(allocation.emitted_slots());
            for index in 0..allocation.candidate_capacity() {
                let scope =
                    EvaluationScope::new(EvaluationScopeCode::custom("test:allocated-candidate"))
                        .subject(format!("{}:{index}", self.id));
                scopes.push(scope.clone());
                facets.push(EnginePredictionFacetV2::available(scope, basis()).unwrap());
            }
            if allocation.summary_required() {
                facets.push(
                    EnginePredictionFacetV2::required_unavailable(
                        EvaluationScope::new(EvaluationScopeCode::custom(match self.id {
                            "test:first" => "test:first:facet-budget",
                            "test:second" => "test:second:facet-budget",
                            "test:active" => "test:active:facet-budget",
                            _ => "test:synthetic:facet-budget",
                        })),
                        basis(),
                        vec![PredictionUnavailableReasonV2::FacetBudgetExceeded],
                    )
                    .unwrap(),
                );
            }
            if facets.is_empty() {
                return CheckOutput::from_coverage(Vec::new(), Vec::new(), Vec::new());
            }
            let identity: PredictionProvenanceIdentityV2 = serde_json::from_value(
                serde_json::to_value(InputIdentity::from_bytes(b"synthetic-v2-provenance"))
                    .unwrap(),
            )
            .unwrap();
            CheckOutput::from_coverage(Vec::new(), scopes, Vec::new())
                .with_engine_prediction_v2(EnginePredictionV2::new(identity, facets).unwrap())
        }
    }

    fn assert_reference_table(docs: &str, heading: &str, entries: &[BuiltinEvidenceCode]) {
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
            .map(|definition| {
                let BuiltinEvidenceCode {
                    code,
                    meaning,
                    emitted_by,
                } = definition;
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
    fn v2_orchestration_reserves_catalog_order_before_constructing_candidates() {
        let doc = Document::default();
        let grids = MetricGrids::new(&doc);
        let roles = ResolvedRoles::default();
        let config = Config::default();
        let ctx = CheckCtx::new(&grids, &roles, &config);
        let allocations = Rc::new(RefCell::new(Vec::new()));
        let constructed = Rc::new(RefCell::new(0));
        let checks: Vec<Box<dyn Check>> = vec![
            Box::new(AllocatedSyntheticCheck {
                id: "test:first",
                demand: 4_096,
                applicable: true,
                allocations: allocations.clone(),
                constructed_candidates: constructed.clone(),
            }),
            Box::new(AllocatedSyntheticCheck {
                id: "test:second",
                demand: 1,
                applicable: true,
                allocations: allocations.clone(),
                constructed_candidates: constructed.clone(),
            }),
        ];

        let first = evaluate_checks_v2(&ctx, &checks, CheckSelection::All).unwrap();
        assert_eq!(
            allocations.borrow().as_slice(),
            [
                ("test:first".to_owned(), 4_094, true),
                ("test:second".to_owned(), 1, false),
            ]
        );
        // The first rule demanded 4,096, but its two omitted candidates were
        // never constructed; its reserved summary is constructed separately.
        assert_eq!(*constructed.borrow(), 4_095);
        let first_bytes = serde_json::to_vec(&first).unwrap();

        allocations.borrow_mut().clear();
        *constructed.borrow_mut() = 0;
        let second = evaluate_checks_v2(&ctx, &checks, CheckSelection::All).unwrap();
        assert_eq!(serde_json::to_vec(&second).unwrap(), first_bytes);
        assert_eq!(*constructed.borrow(), 4_095);
    }

    #[test]
    fn v2_orchestration_does_not_reserve_or_evaluate_inactive_rule_demand() {
        let doc = Document::default();
        let grids = MetricGrids::new(&doc);
        let roles = ResolvedRoles::default();
        let config = Config::default();
        let ctx = CheckCtx::new(&grids, &roles, &config);
        let allocations = Rc::new(RefCell::new(Vec::new()));
        let constructed = Rc::new(RefCell::new(0));
        let checks: Vec<Box<dyn Check>> = vec![
            Box::new(AllocatedSyntheticCheck {
                id: "test:active",
                demand: 4_096,
                applicable: true,
                allocations: allocations.clone(),
                constructed_candidates: constructed.clone(),
            }),
            Box::new(AllocatedSyntheticCheck {
                id: "test:inactive",
                demand: 4_096,
                applicable: false,
                allocations: allocations.clone(),
                constructed_candidates: constructed.clone(),
            }),
        ];

        let records = evaluate_checks_v2(&ctx, &checks, CheckSelection::All).unwrap();
        assert_eq!(
            allocations.borrow().as_slice(),
            [("test:active".to_owned(), 4_096, false)]
        );
        assert_eq!(*constructed.borrow(), 4_096);
        assert_eq!(records[1].applicability(), Applicability::NotApplicable);
    }

    #[test]
    fn v2_orchestration_rejects_output_that_does_not_fill_its_allocation() {
        let doc = Document::default();
        let grids = MetricGrids::new(&doc);
        let roles = ResolvedRoles::default();
        let config = Config::default();
        let ctx = CheckCtx::new(&grids, &roles, &config);
        let checks: Vec<Box<dyn Check>> = vec![Box::new(EmptyAllocatedCheck)];

        assert!(matches!(
            evaluate_checks_v2(&ctx, &checks, CheckSelection::All),
            Err(EvaluationError::PredictionAllocationMismatch {
                check_id: "test:empty-allocated",
                allocated: 1,
                emitted: 0,
            })
        ));
    }

    #[test]
    fn output_docs_match_registered_builtin_evidence_codes_exactly() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let Some(workspace_root) = source_workspace_root(manifest_dir) else {
            // Published crates intentionally exclude repository-level docs.
            return;
        };
        let docs_path = workspace_root.join("docs/output.md");
        let docs = std::fs::read_to_string(&docs_path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", docs_path.display()));
        let crlf = docs.lines().collect::<Vec<_>>().join("\r\n");
        for line_endings in [docs.as_str(), crlf.as_str()] {
            assert_reference_table(
                line_endings,
                "Built-in gap codes are:",
                BUILTIN_COVERAGE_GAP_CODE_DEFINITIONS,
            );
            assert_reference_table(
                line_endings,
                "Built-in completed/gap scope codes are:",
                BUILTIN_EVALUATION_SCOPE_CODE_DEFINITIONS,
            );
        }
    }

    #[test]
    fn builtin_evidence_authority_has_unique_codes_and_known_emitters() {
        let catalog_ids = crate::all_checks()
            .into_iter()
            .map(|check| check.id())
            .chain(EXTERNAL_BUILTIN_CHECK_IDS.iter().copied())
            .collect::<BTreeSet<_>>();
        let authorities = [
            ("coverage-gap", BUILTIN_COVERAGE_GAP_CODE_DEFINITIONS),
            (
                "evaluation-scope",
                BUILTIN_EVALUATION_SCOPE_CODE_DEFINITIONS,
            ),
        ];

        for (kind, definitions) in authorities {
            assert!(
                definitions
                    .iter()
                    .all(|definition| !definition.code.is_empty()),
                "{kind} authority codes must be nonempty"
            );
            let defined_codes = definitions
                .iter()
                .map(|definition| definition.code)
                .collect::<BTreeSet<_>>();
            assert_eq!(
                defined_codes.len(),
                definitions.len(),
                "duplicate {kind} authority code"
            );
            for definition in definitions {
                let emitters = definition
                    .emitted_by
                    .iter()
                    .copied()
                    .collect::<BTreeSet<_>>();
                assert_eq!(
                    emitters.len(),
                    definition.emitted_by.len(),
                    "{} has duplicate emitters",
                    definition.code
                );
                for emitter in emitters {
                    assert!(
                        catalog_ids.contains(emitter),
                        "{} declares unknown emitter {emitter:?}",
                        definition.code
                    );
                }
            }
        }
    }

    #[test]
    fn every_builtin_code_enforces_its_emitter_matrix() {
        let catalog_ids = crate::all_checks()
            .into_iter()
            .map(|check| check.id())
            .chain(EXTERNAL_BUILTIN_CHECK_IDS.iter().copied())
            .collect::<Vec<_>>();

        for definition in BUILTIN_EVALUATION_SCOPE_CODE_DEFINITIONS {
            for &check_id in &catalog_ids {
                let code = EvaluationScopeCode::custom(definition.code);
                let completed = CheckEvaluation::evaluated(
                    check_id,
                    CheckOutput::from_coverage(
                        Vec::new(),
                        vec![EvaluationScope::new(code.clone())],
                        Vec::new(),
                    ),
                );
                let gap_scope = CheckEvaluation::evaluated(
                    check_id,
                    CheckOutput::from_coverage(
                        Vec::new(),
                        Vec::new(),
                        vec![
                            CoverageGap::new(CoverageGapCode::custom("test:gap"), "gap")
                                .scope(EvaluationScope::new(code.clone())),
                        ],
                    ),
                );
                if definition.emitted_by.contains(&check_id) {
                    assert!(
                        completed.is_ok(),
                        "{} must allow {check_id:?}",
                        definition.code
                    );
                    assert!(
                        gap_scope.is_ok(),
                        "{} must allow {check_id:?}",
                        definition.code
                    );
                } else {
                    let expected =
                        EvaluationError::BuiltinEvaluationScopeEmitterMismatch { check_id, code };
                    assert_eq!(completed.unwrap_err(), expected);
                    assert_eq!(gap_scope.unwrap_err(), expected);
                }
            }
        }

        for definition in BUILTIN_COVERAGE_GAP_CODE_DEFINITIONS {
            for &check_id in &catalog_ids {
                let code = CoverageGapCode::custom(definition.code);
                let gap = CheckEvaluation::evaluated(
                    check_id,
                    CheckOutput::from_coverage(
                        Vec::new(),
                        Vec::new(),
                        vec![CoverageGap::new(code, "test gap")],
                    ),
                );
                if definition.emitted_by.contains(&check_id) {
                    assert!(gap.is_ok(), "{} must allow {check_id:?}", definition.code);
                } else {
                    assert_eq!(
                        gap.unwrap_err(),
                        EvaluationError::BuiltinCoverageGapEmitterMismatch { check_id, code }
                    );
                }
            }
        }
    }

    #[test]
    fn mixed_prediction_facets_use_the_existing_evaluation_and_exit_lifecycle() {
        const CHECK_ID: &str = "test:engine-prediction";
        let code = EvaluationScopeCode::custom("test:prediction-work");
        let available_scope = EvaluationScope::new(code.clone()).subject("available-clip");
        let unavailable_scope = EvaluationScope::new(code).subject("unavailable-clip");
        let available_basis = EnginePredictionBasisV1::new(vec![
            PredictionBasisReferenceV1::project_field(
                "project.mode",
                PredictionScalarV1::token("generic").unwrap(),
            )
            .unwrap(),
        ])
        .unwrap();
        let unavailable_basis = EnginePredictionBasisV1::new(Vec::new()).unwrap();
        let prediction = EnginePredictionV1::new(
            PredictionProvenanceIdentityV1::from_input_identity(InputIdentity::from_bytes(
                b"profile",
            )),
            vec![
                EnginePredictionFacetV1::available(available_scope.clone(), available_basis)
                    .unwrap(),
                EnginePredictionFacetV1::required_unavailable(
                    unavailable_scope,
                    unavailable_basis,
                    vec![PredictionUnavailableReasonV1::MeasurementUnavailable],
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let finding = Finding::new(CHECK_ID, Severity::Note, "available subject finding")
            .prediction_scope(available_scope.clone());
        let check = CheckEvaluation::evaluated(
            CHECK_ID,
            CheckOutput::from_coverage(vec![finding], vec![available_scope], Vec::new())
                .with_engine_prediction(prediction),
        )
        .unwrap();

        assert_eq!(check.evaluation(), EvaluationState::Partial);
        assert!(lint_requires_failure(
            &[check],
            Severity::Error,
            &BTreeSet::from([CHECK_ID.to_owned()]),
        ));
    }

    #[test]
    fn available_only_prediction_does_not_fail_without_a_threshold_finding() {
        const CHECK_ID: &str = "test:available-prediction";
        let scope = EvaluationScope::new(EvaluationScopeCode::custom("test:prediction-work"));
        let basis = EnginePredictionBasisV1::new(vec![
            PredictionBasisReferenceV1::project_field(
                "project.mode",
                PredictionScalarV1::token("generic").unwrap(),
            )
            .unwrap(),
        ])
        .unwrap();
        let prediction = EnginePredictionV1::new(
            PredictionProvenanceIdentityV1::from_input_identity(InputIdentity::from_bytes(
                b"profile",
            )),
            vec![EnginePredictionFacetV1::available(scope.clone(), basis).unwrap()],
        )
        .unwrap();
        let finding = Finding::new(CHECK_ID, Severity::Note, "nonblocking available finding")
            .prediction_scope(scope.clone());
        let check = CheckEvaluation::evaluated(
            CHECK_ID,
            CheckOutput::from_coverage(vec![finding], vec![scope], Vec::new())
                .with_engine_prediction(prediction),
        )
        .unwrap();

        assert_eq!(check.evaluation(), EvaluationState::Complete);
        assert!(!lint_requires_failure(
            &[check],
            Severity::Error,
            &BTreeSet::new(),
        ));
    }

    #[test]
    fn required_unavailable_prediction_fails_despite_allow_and_severity() {
        const CHECK_ID: &str = "test:unavailable-prediction";
        let scope = EvaluationScope::new(EvaluationScopeCode::custom("test:prediction-work"));
        let prediction = EnginePredictionV1::new(
            PredictionProvenanceIdentityV1::from_input_identity(InputIdentity::from_bytes(
                b"profile",
            )),
            vec![
                EnginePredictionFacetV1::required_unavailable(
                    scope,
                    EnginePredictionBasisV1::new(Vec::new()).unwrap(),
                    vec![PredictionUnavailableReasonV1::MeasurementUnavailable],
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let check = CheckEvaluation::evaluated(
            CHECK_ID,
            CheckOutput::from_coverage(Vec::new(), Vec::new(), Vec::new())
                .with_engine_prediction(prediction),
        )
        .unwrap();

        assert_eq!(check.evaluation(), EvaluationState::NotEvaluated);
        assert!(lint_requires_failure(
            &[check],
            Severity::Error,
            &BTreeSet::from([CHECK_ID.to_owned()]),
        ));
    }

    #[test]
    fn source_workspace_detection_has_a_positive_checkout_control() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let detected = source_workspace_root(manifest_dir);
        if manifest_dir.join(".cargo_vcs_info.json").is_file() {
            assert!(detected.is_none(), "published packages must skip repo docs");
            return;
        }

        let expected = manifest_dir.join("../..");
        if expected.join("docs/output.md").is_file() {
            assert_eq!(
                detected.as_deref(),
                Some(expected.as_path()),
                "the exact source checkout must enforce its output docs"
            );
        }
    }

    fn source_workspace_root(manifest_dir: &Path) -> Option<PathBuf> {
        if manifest_dir.join(".cargo_vcs_info.json").is_file() {
            return None;
        }
        let workspace_root = manifest_dir.join("../..");
        let current_manifest = manifest_dir.join("Cargo.toml").canonicalize().ok()?;
        let workspace_manifest = workspace_root
            .join("crates/animsmith-core/Cargo.toml")
            .canonicalize()
            .ok()?;
        (current_manifest == workspace_manifest).then_some(workspace_root)
    }
}
