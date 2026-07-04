//! Structured lint findings. The structured fields (not just a message
//! string) are what make `diff`, the JSON schema, and the HTML report
//! cheap downstream.

use serde::Serialize;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Note,
    Warning,
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
    Number(f64),
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

#[derive(Debug, Clone, Serialize)]
#[non_exhaustive]
pub struct Finding {
    pub check_id: &'static str,
    pub severity: Severity,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_s: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub measured: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<Value>,
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

    pub fn clip(mut self, clip: impl Into<String>) -> Self {
        self.clip = Some(clip.into());
        self
    }

    pub fn bone(mut self, bone: impl Into<String>) -> Self {
        self.bone = Some(bone.into());
        self
    }

    pub fn time(mut self, t: f32) -> Self {
        self.time_s = Some(t);
        self
    }

    pub fn measured(mut self, v: impl Into<Value>) -> Self {
        self.measured = Some(v.into());
        self
    }

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
