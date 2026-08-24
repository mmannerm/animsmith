//! Shared typed outcome boundary for artifact-producing commands.
//!
//! `convert` and `assemble` classify failures where their provenance is
//! still typed.  Nothing at the CLI boundary inspects error prose to decide
//! an exit code: invocation/filesystem failures are [`Failure::Operator`],
//! while facts established from bytes already read are
//! [`Failure::Refusal`].  `scale` has a richer command-specific refusal
//! record, but uses the same outcome vocabulary.

use crate::publish::{emit, emit_error_text, serialize_record};
use crate::{EXIT_FINDINGS, Format};
use animsmith_core::ToolInfo;
use serde::Serialize;
use std::process::ExitCode;

pub(crate) const REFUSAL_SCHEMA_VERSION: u32 = 1;
pub(crate) const REFUSAL_SCHEMA_ID: &str = "urn:animsmith:schema:producer-refusal:1";

/// A command whose asset-producing boundary can refuse source facts.
#[cfg_attr(not(feature = "fbx"), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Command {
    Convert,
    Assemble,
    ContactFragment,
}

impl Command {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Convert => "convert",
            Self::Assemble => "assemble",
            Self::ContactFragment => "generate contact-fragment",
        }
    }
}

/// Pipeline stage at which an asset fact became a refusal.
#[cfg_attr(not(feature = "fbx"), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Stage {
    Load,
    Selection,
    Analysis,
    Transform,
    Proof,
    Encode,
}

/// Stable refusal identity.  Prose remains diagnostic only.
#[cfg_attr(not(feature = "fbx"), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Kind {
    UnreadableSource,
    UnsupportedSourceDomain,
    InvalidAssetStructure,
    AssetRecipeMismatch,
    TransformRefused,
    ProofFailed,
    UnrepresentableArtifact,
    IncompleteEvidence,
    SelectionMismatch,
}

impl Kind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::UnreadableSource => "unreadable-source",
            Self::UnsupportedSourceDomain => "unsupported-source-domain",
            Self::InvalidAssetStructure => "invalid-asset-structure",
            Self::AssetRecipeMismatch => "asset-recipe-mismatch",
            Self::TransformRefused => "transform-refused",
            Self::ProofFailed => "proof-failed",
            Self::UnrepresentableArtifact => "unrepresentable-artifact",
            Self::IncompleteEvidence => "incomplete-evidence",
            Self::SelectionMismatch => "selection-mismatch",
        }
    }
}

/// One asset-property refusal, still separate from operator failures.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Rejection {
    pub(crate) stage: Stage,
    pub(crate) kind: Kind,
    pub(crate) detail: String,
}

impl Rejection {
    pub(crate) fn new(stage: Stage, kind: Kind, detail: impl Into<String>) -> Self {
        Self {
            stage,
            kind,
            detail: detail.into(),
        }
    }
}

/// Typed failure authority used before rendering or exit-code selection.
pub(crate) enum Failure {
    /// Invocation, path, filesystem, recipe syntax, or publication failure.
    Operator(String),
    /// A fact established about source bytes or their fit to a valid recipe.
    Refusal(Rejection),
}

impl Failure {
    pub(crate) fn operator(error: impl ToString) -> Self {
        Self::Operator(error.to_string())
    }

    pub(crate) fn refusal(stage: Stage, kind: Kind, error: impl ToString) -> Self {
        Self::Refusal(Rejection::new(stage, kind, error.to_string()))
    }
}

/// The common producer result before presentation.
#[cfg_attr(not(feature = "fbx"), allow(dead_code))]
pub(crate) enum Outcome<T> {
    Published(T),
    Rejected(Rejection),
}

/// Explicit provenance adapters for fallible calls inside producer pipelines.
///
/// There is deliberately no `From<String>` for [`Failure`]: every `?` at a
/// producer boundary must state which side of the public exit contract owns
/// it while the call site still knows what was attempted.
#[cfg_attr(not(feature = "fbx"), allow(dead_code))]
pub(crate) trait Classify<T> {
    fn operator(self) -> Result<T, Failure>;
    fn refusal(self, stage: Stage, kind: Kind) -> Result<T, Failure>;
}

impl<T, E: ToString> Classify<T> for Result<T, E> {
    fn operator(self) -> Result<T, Failure> {
        self.map_err(Failure::operator)
    }

    fn refusal(self, stage: Stage, kind: Kind) -> Result<T, Failure> {
        self.map_err(|error| Failure::refusal(stage, kind, error))
    }
}

#[derive(Serialize)]
pub(crate) struct RefusalRecord {
    schema_version: u32,
    schema: &'static str,
    tool: ToolInfo,
    command: Command,
    outcome: &'static str,
    result: Option<()>,
    rejection: Rejection,
}

fn serialize_refusal(record: &impl Serialize) -> Result<Vec<u8>, String> {
    serialize_record(record)
}

/// Injectable refusal presentation used by the real command dispatch.
///
/// Keeping serialization and delivery behind this seam lets tests force the
/// otherwise-unreachable serde failure while exercising the exact
/// [`emit_rejection`] function production calls. A serializer failure returns
/// before either output method can run.
pub(crate) trait RefusalDelivery {
    fn serialize(&mut self, record: &RefusalRecord) -> Result<Vec<u8>, String>;
    fn emit_json(&mut self, bytes: &[u8]);
    fn emit_text(&mut self, text: &str);
}

pub(crate) struct ProcessRefusalDelivery;

impl RefusalDelivery for ProcessRefusalDelivery {
    fn serialize(&mut self, record: &RefusalRecord) -> Result<Vec<u8>, String> {
        serialize_refusal(record)
    }

    fn emit_json(&mut self, bytes: &[u8]) {
        emit(bytes);
    }

    fn emit_text(&mut self, text: &str) {
        emit_error_text(text);
    }
}

/// Present one already-established refusal without changing its exit code.
///
/// Serialization is intentionally fallible and remains an operator error.
/// Once bytes exist, a failed stdout delivery is only a reporting failure and
/// the refusal remains exit 1, matching the checked boundary used by `scale`.
pub(crate) fn emit_rejection(
    command: Command,
    format: Format,
    tool: ToolInfo,
    rejection: Rejection,
    delivery: &mut impl RefusalDelivery,
) -> Result<ExitCode, String> {
    match format {
        Format::Json => {
            let bytes = delivery.serialize(&RefusalRecord {
                schema_version: REFUSAL_SCHEMA_VERSION,
                schema: REFUSAL_SCHEMA_ID,
                tool,
                command,
                outcome: "rejected",
                result: None,
                rejection,
            })?;
            delivery.emit_json(&bytes);
        }
        Format::Text => delivery.emit_text(&crate::render::render_producer_rejected(
            command.label(),
            rejection.kind.label(),
            &rejection.detail,
        )),
    }
    Ok(ExitCode::from(EXIT_FINDINGS))
}
