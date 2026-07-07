//! Structured lint findings. The structured fields (not just a message
//! string) are what make `diff`, the JSON schema, and the HTML report
//! cheap downstream.

use serde::Serialize;
use std::fmt;

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
    /// Effective severity after any non-diagnostic override.
    pub severity: Severity,
    /// Clip associated with the finding, when the finding is clip-local.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clip: Option<String>,
    /// Bone associated with the finding, when applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bone: Option<String>,
    /// Time in seconds associated with the finding, when applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_s: Option<f32>,
    /// Measured value that triggered the finding.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub measured: Option<Value>,
    /// Expected value or threshold for the finding.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<Value>,
    /// Human-readable explanation.
    pub message: String,
    /// A diagnostic (a "skipped: …" note about an unmet prerequisite),
    /// not a judgement of the content. Diagnostics are exempt from
    /// per-check severity overrides — a check declared `severity =
    /// "error"` must never turn a "roles unresolved" note into a false
    /// failure. Not serialized: the JSON output shape is unchanged.
    #[serde(skip)]
    pub diagnostic: bool,
}

impl Finding {
    /// Construct a finding with no optional context fields set.
    pub fn new(check_id: &'static str, severity: Severity, message: impl Into<String>) -> Self {
        Self {
            check_id,
            severity,
            clip: None,
            bone: None,
            time_s: None,
            measured: None,
            expected: None,
            message: message.into(),
            diagnostic: false,
        }
    }

    /// Mark this finding a diagnostic (see [`Finding::diagnostic`]):
    /// emitted at `Note`, exempt from severity overrides.
    pub fn as_diagnostic(mut self) -> Self {
        self.diagnostic = true;
        self
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

    /// Attach a clip time in seconds.
    pub fn time(mut self, t: f32) -> Self {
        self.time_s = Some(t);
        self
    }

    /// Attach a measured value.
    pub fn measured(mut self, v: impl Into<Value>) -> Self {
        self.measured = Some(v.into());
        self
    }

    /// Attach an expected value or threshold.
    pub fn expected(mut self, v: impl Into<Value>) -> Self {
        self.expected = Some(v.into());
        self
    }
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
