//! Structured lint findings. The structured fields (not just a message
//! string) are what make `diff`, the JSON schema, and the HTML report
//! cheap downstream.

use serde::Serialize;
use std::collections::BTreeMap;
use std::fmt;

use crate::evaluation::EvaluationScope;

/// Severity of a lint finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Informational diagnostic that does not fail a gate.
    Note,
    /// Warning-level finding; the CLI treats warnings as a clean exit
    /// unless configured to deny warnings.
    Warning,
    /// Error-level finding; the CLI exits with a content-failure status
    /// when any error is present.
    Error,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Severity::Note => "note",
            Severity::Warning => "warning",
            Severity::Error => "error",
        })
    }
}

/// A measured or expected quantity attached to a finding.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
#[non_exhaustive]
pub enum Value {
    /// Numeric measured or expected value.
    Number(f64),
    /// Textual measured or expected value.
    Text(String),
}

/// One configured member's machine-readable evidence attached to a group
/// finding.
///
/// The member order is the configuration order, while the measurement map is
/// key-sorted for stable JSON. Values are deliberately scalar so consumers do
/// not need to parse presentation text to recover a group comparison table.
#[derive(Debug, Clone, Serialize)]
#[non_exhaustive]
pub struct MemberMeasurement {
    /// Configured member name, including a name absent from the input file.
    pub member: String,
    /// Named scalar measurements for this member. Non-finite numeric values
    /// are excluded by [`Self::measurement`] and [`Finding::members`].
    pub measurements: BTreeMap<String, Value>,
}

impl MemberMeasurement {
    /// Construct an empty evidence row for `member`.
    pub fn new(member: impl Into<String>) -> Self {
        Self {
            member: member.into(),
            measurements: BTreeMap::new(),
        }
    }

    /// Attach a finite numeric or textual measurement.
    pub fn measurement(mut self, name: impl Into<String>, value: impl Into<Value>) -> Self {
        let value = value.into();
        if value.is_finite() {
            self.measurements.insert(name.into(), value);
        }
        self
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Number(n) => write!(f, "{n:.4}"),
            Value::Text(s) => f.write_str(s),
        }
    }
}

/// A structured lint result emitted by a [`crate::Check`].
///
/// The JSON shape is part of animsmith's automation contract. The Rust
/// struct is marked `non_exhaustive` so new optional context fields can
/// be added before 1.0 without forcing downstream construction through
/// struct literals.
#[derive(Debug, Clone, Serialize)]
#[non_exhaustive]
pub struct Finding {
    /// Stable check id such as `"loop-seam"`.
    pub check_id: &'static str,
    /// Effective severity after any per-check override.
    pub severity: Severity,
    /// Clip associated with the finding, when the finding is clip-local.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clip: Option<String>,
    /// Bone associated with the finding, when applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bone: Option<String>,
    /// Stable source-node path associated with the finding, when applicable.
    /// Path components carry source indices so repeated display names remain
    /// distinguishable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node: Option<String>,
    /// Exact prediction facet that produced this finding, when the parent
    /// check carries an engine-prediction attachment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prediction_scope: Option<EvaluationScope>,
    /// Finite time in seconds associated with the finding, when applicable.
    /// Non-finite values are omitted from serialized output.
    #[serde(skip_serializing_if = "non_finite_time_or_none")]
    pub time_s: Option<f32>,
    /// Measured value that triggered the finding. Non-finite numeric values
    /// are omitted from serialized output.
    #[serde(skip_serializing_if = "non_finite_value_or_none")]
    pub measured: Option<Value>,
    /// Expected value or threshold for the finding. Non-finite numeric values
    /// are omitted from serialized output.
    #[serde(skip_serializing_if = "non_finite_value_or_none")]
    pub expected: Option<Value>,
    /// Per-member evidence for a group-level finding, in configured order.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub members: Option<Vec<MemberMeasurement>>,
    /// Human-readable explanation.
    pub message: String,
}

impl Finding {
    /// Construct a finding with no optional context fields set.
    pub fn new(check_id: &'static str, severity: Severity, message: impl Into<String>) -> Self {
        Self {
            check_id,
            severity,
            clip: None,
            bone: None,
            node: None,
            prediction_scope: None,
            time_s: None,
            measured: None,
            expected: None,
            members: None,
            message: message.into(),
        }
    }

    /// Attach a clip name.
    pub fn clip(mut self, clip: impl Into<String>) -> Self {
        self.clip = Some(clip.into());
        self
    }

    /// Attach a bone name.
    pub fn bone(mut self, bone: impl Into<String>) -> Self {
        self.bone = Some(bone.into());
        self
    }

    /// Attach a stable source-node path.
    pub fn node(mut self, node: impl Into<String>) -> Self {
        self.node = Some(node.into());
        self
    }

    /// Bind this finding to one available engine-prediction facet.
    ///
    /// The shared check-evaluation boundary verifies that the scope identifies
    /// exactly one available facet on the parent check. It rejects a scope that
    /// is missing or required-unavailable.
    pub fn prediction_scope(mut self, scope: EvaluationScope) -> Self {
        self.prediction_scope = Some(scope);
        self
    }

    /// Attach a clip time in seconds.
    pub fn time(mut self, t: f32) -> Self {
        self.time_s = t.is_finite().then_some(t);
        self
    }

    /// Attach a measured value.
    pub fn measured(mut self, v: impl Into<Value>) -> Self {
        let value = v.into();
        self.measured = value.is_finite().then_some(value);
        self
    }

    /// Attach an expected value or threshold.
    pub fn expected(mut self, v: impl Into<Value>) -> Self {
        let value = v.into();
        self.expected = value.is_finite().then_some(value);
        self
    }

    /// Attach a configured-order group member table.
    ///
    /// Any non-finite numeric values supplied through a directly constructed
    /// row are removed before serialization, matching scalar finding values.
    pub fn members(mut self, mut members: Vec<MemberMeasurement>) -> Self {
        for member in &mut members {
            member.measurements.retain(|_, value| value.is_finite());
        }
        self.members = Some(members);
        self
    }
}

impl Value {
    fn is_finite(&self) -> bool {
        match self {
            Self::Number(number) => number.is_finite(),
            Self::Text(_) => true,
        }
    }
}

fn non_finite_time_or_none(value: &Option<f32>) -> bool {
    value.is_none_or(|time| !time.is_finite())
}

fn non_finite_value_or_none(value: &Option<Value>) -> bool {
    value.as_ref().is_none_or(|value| !value.is_finite())
}

impl From<f64> for Value {
    fn from(n: f64) -> Self {
        Value::Number(n)
    }
}

impl From<f32> for Value {
    fn from(n: f32) -> Self {
        Value::Number(n as f64)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::Text(s.to_owned())
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::Text(s)
    }
}
