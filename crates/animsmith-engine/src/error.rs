use crate::{
    ProfileSelection, SettingDomain, SettingId, SettingScope, SettingValue, SettingValueKind,
};
use animsmith_core::SourceFormatV1;
use animsmith_core::engine_contract::EngineContractError;
use std::fmt;

/// Location of one settings declaration.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum SettingLocation {
    /// Document-wide engine settings.
    Document,
    /// Settings declared under one clip selector.
    ClipSelector(String),
}

impl fmt::Display for SettingLocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Document => formatter.write_str("document settings"),
            Self::ClipSelector(selector) => write!(formatter, "clip selector {selector:?}"),
        }
    }
}

/// Why a closed setting value failed its declared domain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvalidSettingReason {
    /// The value variant does not match the descriptor domain.
    WrongDomain {
        /// Required descriptor domain.
        expected: SettingDomain,
        /// Supplied closed value kind.
        found: SettingValueKind,
    },
    /// The transform path is empty.
    EmptyPath,
    /// The transform path exceeds the V1 UTF-8 byte limit.
    PathTooLong {
        /// Actual UTF-8 byte count.
        bytes: usize,
        /// Maximum UTF-8 byte count.
        limit: usize,
    },
    /// The transform path begins with `/` and is not relative.
    AbsolutePath,
    /// The transform path contains an empty segment.
    EmptyPathSegment,
    /// The transform path contains `.` or `..`.
    DotPathSegment,
    /// The transform path contains an ASCII control character.
    ControlCharacter,
}

/// Typed failure from static or input-dependent engine resolution.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ResolutionError {
    /// Settings were declared without selecting an engine profile.
    #[error("engine settings were declared without an engine profile selection")]
    SettingsWithoutSelection,
    /// No exact registry tuple matches the selection.
    #[error("unknown engine profile selection {0:?}")]
    UnknownProfile(ProfileSelection),
    /// A setting key is not in the selected profile's descriptor inventory.
    #[error("unknown engine setting {key:?} in {location}")]
    UnknownSetting {
        /// Supplied setting key.
        key: String,
        /// Declaration location.
        location: SettingLocation,
    },
    /// A known setting was declared in the wrong scope.
    #[error(
        "engine setting {setting:?} has {expected:?} scope but was declared in {found:?} scope"
    )]
    WrongScope {
        /// Stable setting id.
        setting: SettingId,
        /// Descriptor scope.
        expected: SettingScope,
        /// Supplied scope.
        found: SettingScope,
        /// Declaration location.
        location: SettingLocation,
    },
    /// A known descriptor genuinely does not apply to the selected profile.
    #[error("engine setting {setting:?} is not applicable in {location}")]
    NotApplicable {
        /// Stable setting id.
        setting: SettingId,
        /// Declaration location.
        location: SettingLocation,
    },
    /// A setting value is outside its closed descriptor domain.
    #[error("invalid value for engine setting {setting:?} in {location}: {reason:?}")]
    InvalidSettingValue {
        /// Stable setting id.
        setting: SettingId,
        /// Declaration location.
        location: SettingLocation,
        /// Exact domain/path failure.
        reason: InvalidSettingReason,
    },
    /// One required-without-default setting was not materialized.
    #[error("missing required engine setting {setting:?} in {location}")]
    MissingRequiredSetting {
        /// Stable setting id.
        setting: SettingId,
        /// Document or exact real clip requiring the setting.
        location: SettingLocation,
    },
    /// The authoritative input container is outside the selected V1 boundary.
    #[error("input format {format:?} is not accepted by engine profile {selection:?}")]
    UnacceptedInputFormat {
        /// Selected exact profile tuple.
        selection: ProfileSelection,
        /// Authoritative loader-owned input format.
        format: SourceFormatV1,
    },
    /// Fully materialized settings exceeded or contradicted the bounded V1
    /// engine contract.
    #[error("resolved engine settings are outside the V1 contract: {0}")]
    ResolvedSettingsContract(#[from] EngineContractError),
}

/// Author-owned invariant failure in the built-in V1 registry.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum RegistryValidationError {
    /// V1 did not contain exactly five singleton profiles.
    #[error("V1 registry contains {found} profiles rather than five")]
    ProfileCount {
        /// Actual profile count.
        found: usize,
    },
    /// Two records use the same exact selection.
    #[error("duplicate profile selection {selection:?}")]
    DuplicateSelection {
        /// Repeated selection.
        selection: ProfileSelection,
    },
    /// Two records use the same revisioned fact-bundle URN.
    #[error("duplicate fact-bundle URN {urn}")]
    DuplicateFactBundleUrn {
        /// Repeated URN.
        urn: &'static str,
    },
    /// The record does not enumerate every stable V1 fact id exactly once.
    #[error("invalid fact inventory for {selection:?}")]
    InvalidFactInventory {
        /// Affected selection.
        selection: ProfileSelection,
    },
    /// The accepted-input fact is not a nonempty canonical known-format set.
    #[error("invalid accepted-input fact for {selection:?}")]
    InvalidAcceptedInputFact {
        /// Affected selection.
        selection: ProfileSelection,
    },
    /// A setting descriptor id is repeated.
    #[error("duplicate setting descriptor for {selection:?}")]
    DuplicateSettingDescriptor {
        /// Affected selection.
        selection: ProfileSelection,
    },
    /// Applicability and default status contradict each other.
    #[error("invalid default status for {setting:?} in {selection:?}")]
    InvalidDescriptorDefault {
        /// Affected selection.
        selection: ProfileSelection,
        /// Affected setting.
        setting: SettingId,
    },
    /// A primary-source id is repeated within one profile.
    #[error("duplicate source id {source_id} in {selection:?}")]
    DuplicateSourceId {
        /// Affected selection.
        selection: ProfileSelection,
        /// Repeated source id.
        source_id: &'static str,
    },
    /// A source references a fact outside the record vocabulary.
    #[error("source {source_id} references unknown fact {fact:?} in {selection:?}")]
    UnknownSourceFact {
        /// Affected selection.
        selection: ProfileSelection,
        /// Source id.
        source_id: &'static str,
        /// Unknown fact id.
        fact: crate::FactId,
    },
    /// A source claims support for a fact whose record state is not known.
    #[error("source {source_id} references non-known fact {fact:?} in {selection:?}")]
    SourceReferencesNonKnownFact {
        /// Affected selection.
        selection: ProfileSelection,
        /// Source id.
        source_id: &'static str,
        /// Fact without a known value.
        fact: crate::FactId,
    },
    /// A source references a setting absent from the record.
    #[error("source {source_id} references unknown setting {setting:?} in {selection:?}")]
    UnknownSourceSetting {
        /// Affected selection.
        selection: ProfileSelection,
        /// Source id.
        source_id: &'static str,
        /// Missing setting id.
        setting: SettingId,
    },
    /// A known fact has no supporting primary-source reference.
    #[error("known fact {fact:?} has no source reference in {selection:?}")]
    UnreferencedKnownFact {
        /// Affected selection.
        selection: ProfileSelection,
        /// Unsupported known fact.
        fact: crate::FactId,
    },
    /// A descriptor has no supporting primary-source reference.
    #[error("setting descriptor {setting:?} has no source reference in {selection:?}")]
    UnreferencedSetting {
        /// Affected selection.
        selection: ProfileSelection,
        /// Unsupported descriptor.
        setting: SettingId,
    },
    /// The stored record identity differs from a fresh canonical encoding.
    #[error("facts identity mismatch for {selection:?}")]
    FactsIdentityMismatch {
        /// Affected selection.
        selection: ProfileSelection,
    },
}

impl SettingValue {
    pub(crate) const fn kind(&self) -> SettingValueKind {
        match self {
            SettingValue::Boolean(_) => SettingValueKind::Boolean,
            SettingValue::BakeOrExtract(_) => SettingValueKind::BakeOrExtract,
            SettingValue::SourceTransformPath(_) => SettingValueKind::SourceTransformPath,
        }
    }
}
