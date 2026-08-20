//! Bounded identities for a source file and its declared resource closure.
//!
//! Core owns the immutable value contract and canonical digest. Format crates
//! own rooted filesystem access and provide only safe logical keys and byte
//! identities to [`DependencyClosureBuilderV1`].

use crate::{
    InputIdentity, SourceFactSetV1, SourceFormatV1, SourceRelativeLocatorV1, SourceResourceKindV1,
    SourceResourceLocatorV1, SourceResourceReferenceV1, SourceSetCoverageStateV1,
    SourceSetCoverageV1,
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

/// Semantic identity of the dependency-closure V1 value contract.
pub const DEPENDENCY_CLOSURE_V1_ID: &str = "urn:animsmith:dependency-closure:1";
/// Semantic identity of the immutable dependency-closure V1 budget.
pub const DEPENDENCY_CLOSURE_BUDGET_V1_ID: &str = "urn:animsmith:dependency-closure-budget:1";
/// Maximum source-resource declarations inspected by one closure capture.
pub const DEPENDENCY_CLOSURE_V1_MAX_REFERENCES: usize = 4_096;
/// Maximum distinct external logical keys captured by one closure.
pub const DEPENDENCY_CLOSURE_V1_MAX_EXTERNAL_RESOURCES: usize = 1_024;
/// Maximum UTF-8 bytes in a source locator or normalized logical key.
pub const DEPENDENCY_CLOSURE_V1_MAX_KEY_BYTES: usize = 4_096;
/// Maximum path components in one logical resource key.
pub const DEPENDENCY_CLOSURE_V1_MAX_PATH_COMPONENTS: usize = 128;
/// Maximum aggregate source-locator bytes inspected during normalization.
pub const DEPENDENCY_CLOSURE_V1_MAX_NORMALIZATION_BYTES: usize = 8 * 1024 * 1024;
/// Maximum bytes read and hashed for one external resource.
pub const DEPENDENCY_CLOSURE_V1_MAX_RESOURCE_BYTES: u64 = 64 * 1024 * 1024;
/// Maximum aggregate distinct external bytes read and hashed.
pub const DEPENDENCY_CLOSURE_V1_MAX_TOTAL_RESOURCE_BYTES: u64 = 256 * 1024 * 1024;
/// Maximum alias/deduplication probes, one per inspected declaration.
pub const DEPENDENCY_CLOSURE_V1_MAX_DEDUP_PROBES: usize = 4_096;

/// Immutable numeric limits that define dependency-closure V1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ResourceClosureBudgetV1 {
    schema: &'static str,
    max_references: usize,
    max_external_resources: usize,
    max_key_bytes: usize,
    max_path_components: usize,
    max_normalization_bytes: usize,
    max_resource_bytes: u64,
    max_total_resource_bytes: u64,
    max_dedup_probes: usize,
}

impl ResourceClosureBudgetV1 {
    /// The only budget used by dependency-closure V1.
    pub const VALUE: Self = Self {
        schema: DEPENDENCY_CLOSURE_BUDGET_V1_ID,
        max_references: DEPENDENCY_CLOSURE_V1_MAX_REFERENCES,
        max_external_resources: DEPENDENCY_CLOSURE_V1_MAX_EXTERNAL_RESOURCES,
        max_key_bytes: DEPENDENCY_CLOSURE_V1_MAX_KEY_BYTES,
        max_path_components: DEPENDENCY_CLOSURE_V1_MAX_PATH_COMPONENTS,
        max_normalization_bytes: DEPENDENCY_CLOSURE_V1_MAX_NORMALIZATION_BYTES,
        max_resource_bytes: DEPENDENCY_CLOSURE_V1_MAX_RESOURCE_BYTES,
        max_total_resource_bytes: DEPENDENCY_CLOSURE_V1_MAX_TOTAL_RESOURCE_BYTES,
        max_dedup_probes: DEPENDENCY_CLOSURE_V1_MAX_DEDUP_PROBES,
    };

    /// Semantic identity of these fixed limits.
    pub const fn contract_id(self) -> &'static str {
        DEPENDENCY_CLOSURE_BUDGET_V1_ID
    }

    /// Maximum declaration rows.
    pub const fn max_references(self) -> usize {
        self.max_references
    }

    /// Maximum distinct external logical keys.
    pub const fn max_external_resources(self) -> usize {
        self.max_external_resources
    }

    /// Maximum bytes in one locator or normalized key.
    pub const fn max_key_bytes(self) -> usize {
        self.max_key_bytes
    }

    /// Maximum components in one locator.
    pub const fn max_path_components(self) -> usize {
        self.max_path_components
    }

    /// Maximum aggregate bytes inspected by normalization.
    pub const fn max_normalization_bytes(self) -> usize {
        self.max_normalization_bytes
    }

    /// Maximum bytes captured for one external resource.
    pub const fn max_resource_bytes(self) -> u64 {
        self.max_resource_bytes
    }

    /// Maximum aggregate distinct external bytes captured.
    pub const fn max_total_resource_bytes(self) -> u64 {
        self.max_total_resource_bytes
    }

    /// Maximum alias/deduplication probes.
    pub const fn max_dedup_probes(self) -> usize {
        self.max_dedup_probes
    }
}

/// Format-neutral consumer purpose derived from a source resource kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyResourcePurposeV1 {
    /// The loader requires these bytes to construct its normalized document.
    LoaderEssential,
    /// The declaration is retained, but absence need not fail document loading.
    Nonessential,
    /// The declaration is relevant only to a later target/importer workflow.
    TargetOnly,
}

impl DependencyResourcePurposeV1 {
    const fn from_kind(kind: SourceResourceKindV1) -> Self {
        match kind {
            SourceResourceKindV1::Buffer => Self::LoaderEssential,
            SourceResourceKindV1::Image | SourceResourceKindV1::Texture => Self::Nonessential,
            SourceResourceKindV1::Video | SourceResourceKindV1::Cache => Self::TargetOnly,
        }
    }
}

/// Format-specific lexical interpretation for a retained relative locator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceKeySyntaxV1 {
    /// A glTF URI: valid percent escapes are decoded before canonicalization.
    GltfUri,
    /// A parser-projected relative path: percent signs remain literal bytes.
    ParserRelativePath,
}

/// Safe, normalized, source-relative dependency key.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct DependencyResourceKeyV1(String);

impl DependencyResourceKeyV1 {
    /// Normalize one already-redacted relative locator without host I/O.
    ///
    /// # Errors
    ///
    /// Returns [`DependencyClosureError`] for invalid percent encoding,
    /// unsafe path syntax, too many components, or an oversized normalized key.
    pub fn from_relative(
        locator: &SourceRelativeLocatorV1,
        syntax: ResourceKeySyntaxV1,
    ) -> Result<Self, DependencyClosureError> {
        Self::from_source_str(locator.as_str(), syntax)
    }

    /// Normalize one raw source spelling while enforcing the same fail-closed
    /// contract as [`Self::from_relative`].
    ///
    /// Format loaders normally call [`Self::from_relative`] after raw-source
    /// classification. This constructor is also useful for bounded preflight
    /// tests and custom loaders that do not retain a raw-facts row first.
    ///
    /// # Errors
    ///
    /// Returns [`DependencyClosureError`] for an unsafe or oversized key.
    pub fn from_source_str(
        raw: &str,
        syntax: ResourceKeySyntaxV1,
    ) -> Result<Self, DependencyClosureError> {
        if raw.len() > DEPENDENCY_CLOSURE_V1_MAX_KEY_BYTES {
            return Err(DependencyClosureError::ResourceKeyTooLong {
                bytes: raw.len(),
                limit: DEPENDENCY_CLOSURE_V1_MAX_KEY_BYTES,
            });
        }
        if raw.contains('\\') || raw.chars().any(char::is_control) {
            return Err(DependencyClosureError::InvalidResourceKey);
        }
        let normalized = match syntax {
            ResourceKeySyntaxV1::GltfUri => decode_percent_utf8(raw)?,
            ResourceKeySyntaxV1::ParserRelativePath => raw.to_owned(),
        };
        validate_normalized_key(&normalized)?;
        Ok(Self(normalized))
    }

    /// Safe normalized spelling used in digest input and rooted lookup.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Number of source path components, without allocation.
    pub fn source_component_count(locator: &SourceRelativeLocatorV1) -> usize {
        locator.as_str().split('/').count()
    }
}

fn decode_percent_utf8(raw: &str) -> Result<String, DependencyClosureError> {
    let bytes = raw.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] != b'%' {
            decoded.push(bytes[index]);
            index += 1;
            continue;
        }
        let high = bytes
            .get(index + 1)
            .copied()
            .and_then(hex)
            .ok_or(DependencyClosureError::InvalidResourceKey)?;
        let low = bytes
            .get(index + 2)
            .copied()
            .and_then(hex)
            .ok_or(DependencyClosureError::InvalidResourceKey)?;
        let value = (high << 4) | low;
        if matches!(value, b'/' | b'\\' | 0) {
            return Err(DependencyClosureError::InvalidResourceKey);
        }
        decoded.push(value);
        index += 3;
    }
    String::from_utf8(decoded).map_err(|_| DependencyClosureError::InvalidResourceKey)
}

fn hex(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn validate_normalized_key(value: &str) -> Result<(), DependencyClosureError> {
    if value.is_empty()
        || value.len() > DEPENDENCY_CLOSURE_V1_MAX_KEY_BYTES
        || value.starts_with('/')
        || value.contains('\\')
        || value.contains([':', '?', '#'])
        || value.chars().any(char::is_control)
        || has_uri_scheme(value)
    {
        return Err(DependencyClosureError::InvalidResourceKey);
    }
    let mut components = 0usize;
    for component in value.split('/') {
        components = components.saturating_add(1);
        if component.is_empty() || matches!(component, "." | "..") {
            return Err(DependencyClosureError::InvalidResourceKey);
        }
    }
    if components > DEPENDENCY_CLOSURE_V1_MAX_PATH_COMPONENTS {
        return Err(DependencyClosureError::TooManyPathComponents {
            components,
            limit: DEPENDENCY_CLOSURE_V1_MAX_PATH_COMPONENTS,
        });
    }
    Ok(())
}

fn has_uri_scheme(value: &str) -> bool {
    let Some((scheme, _)) = value.split_once(':') else {
        return false;
    };
    !scheme.is_empty()
        && scheme.as_bytes()[0].is_ascii_alphabetic()
        && scheme
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
}

/// Why a source-controlled locator was refused without opening it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyResourceRefusalReasonV1 {
    /// Absolute or drive-qualified locator.
    Absolute,
    /// Lexical traversal or out-of-root locator.
    Escaping,
    /// Remote URI scheme.
    Remote,
    /// Malformed source spelling.
    Malformed,
    /// Source spelling exceeded the locator budget.
    Oversized,
    /// A host path component or final target was a symbolic link.
    Symlink,
}

/// Why a safe declaration could not be assigned a content identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyResourceUnavailableReasonV1 {
    /// The byte-loading caller supplied no trusted resource root.
    ResourceRootUnavailable,
    /// The accepted relative resource does not exist.
    Missing,
    /// The accepted relative resource could not be opened or read.
    Unreadable,
    /// A closure capture budget stopped this resource.
    ResourceBudgetExceeded,
}

/// One declaration's closure mapping.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum DependencyReferenceTargetV1 {
    /// Bytes are carried by the exact primary input.
    Primary,
    /// Bytes came from one captured normalized external key.
    External {
        /// Key of the distinct external-resource row.
        key: DependencyResourceKeyV1,
    },
    /// Locator was rejected before any open attempt.
    Refused {
        /// Safe normalized key for a relative locator refused as a symlink.
        #[serde(skip_serializing_if = "Option::is_none")]
        key: Option<DependencyResourceKeyV1>,
        /// Stable refusal class; unsafe spelling is never retained.
        reason: DependencyResourceRefusalReasonV1,
    },
    /// A safe declaration had no available captured identity.
    Unavailable {
        /// Safe normalized key when the declaration supplied one.
        #[serde(skip_serializing_if = "Option::is_none")]
        key: Option<DependencyResourceKeyV1>,
        /// Stable unavailable class; host error text is never retained.
        reason: DependencyResourceUnavailableReasonV1,
    },
}

/// One source-order declaration-to-content mapping.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DependencyClosureReferenceV1 {
    source_order_index: usize,
    kind: SourceResourceKindV1,
    purpose: DependencyResourcePurposeV1,
    source_index: u64,
    target: DependencyReferenceTargetV1,
}

impl DependencyClosureReferenceV1 {
    fn new(
        source_order_index: usize,
        kind: SourceResourceKindV1,
        source_index: u64,
        target: DependencyReferenceTargetV1,
    ) -> Self {
        Self {
            source_order_index,
            kind,
            purpose: DependencyResourcePurposeV1::from_kind(kind),
            source_index,
            target,
        }
    }

    /// Deterministic source declaration order.
    pub const fn source_order_index(&self) -> usize {
        self.source_order_index
    }

    /// Source declaration kind.
    pub const fn kind(&self) -> SourceResourceKindV1 {
        self.kind
    }

    /// Format-neutral consumer purpose authoritatively derived from [`Self::kind`].
    pub const fn purpose(&self) -> DependencyResourcePurposeV1 {
        self.purpose
    }

    /// Stable source/parser declaration index.
    pub const fn source_index(&self) -> u64 {
        self.source_index
    }

    /// Captured/refused/unavailable mapping outcome.
    pub const fn target(&self) -> &DependencyReferenceTargetV1 {
        &self.target
    }
}

/// One distinct external logical key and the exact bytes captured once.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExternalResourceIdentityV1 {
    key: DependencyResourceKeyV1,
    identity: InputIdentity,
}

impl ExternalResourceIdentityV1 {
    /// Safe normalized source-relative key.
    pub const fn key(&self) -> &DependencyResourceKeyV1 {
        &self.key
    }

    /// SHA-256 and byte count of the exact captured bytes.
    pub const fn identity(&self) -> &InputIdentity {
        &self.identity
    }
}

/// Why the closure as a whole is not complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyClosureCoverageReasonV1 {
    /// Raw declaration projection retained only a positive prefix.
    SourceDeclarationsPartial,
    /// Raw declaration projection was unavailable.
    SourceDeclarationsUnavailable,
    /// The legacy/custom completion path did not capture a dependency closure.
    CaptureUnavailable,
    /// At least one declaration was refused.
    RefusedResource,
    /// At least one safe declaration lacked an identity.
    UnavailableResource,
    /// A closure budget stopped capture at N+1.
    ResourceBudgetExceeded,
    /// A known format/parser domain can carry unmodelled resource declarations.
    UnmodeledResourceDomain,
}

/// Complete, partial, or unavailable closure coverage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum DependencyClosureCoverageV1 {
    /// Every retained source declaration maps to exact content and the source domain is complete.
    Complete,
    /// Retained mappings are positive evidence but do not establish a full closure.
    Partial {
        /// Sorted unique reasons completeness could not be established.
        reasons: Vec<DependencyClosureCoverageReasonV1>,
    },
    /// The source declaration domain itself could not be projected.
    Unavailable {
        /// Sorted unique reasons no closure can be established.
        reasons: Vec<DependencyClosureCoverageReasonV1>,
    },
}

impl DependencyClosureCoverageV1 {
    /// Stable incompleteness reasons, empty only for complete coverage.
    pub fn reasons(&self) -> &[DependencyClosureCoverageReasonV1] {
        match self {
            Self::Complete => &[],
            Self::Partial { reasons } | Self::Unavailable { reasons } => reasons,
        }
    }

    /// Whether this coverage proves the exact V1 dependency closure.
    pub const fn is_complete(&self) -> bool {
        matches!(self, Self::Complete)
    }
}

/// Domain-separated identity of one complete canonical closure record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct DependencyClosureIdentityV1(InputIdentity);

impl DependencyClosureIdentityV1 {
    /// SHA-256 and canonical-preimage byte count.
    pub const fn input_identity(&self) -> &InputIdentity {
        &self.0
    }
}

/// Bounded capture work, including the N+1 stop witness.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct DependencyClosureWorkV1 {
    inspected_references: usize,
    retained_references: usize,
    normalization_bytes_inspected: usize,
    path_components_inspected: usize,
    dedup_probes: usize,
    external_open_attempts: usize,
    distinct_external_keys: usize,
    captured_external_resources: usize,
    external_bytes_read_hashed: u64,
}

impl DependencyClosureWorkV1 {
    /// Declaration rows inspected, including a terminal N+1.
    pub const fn inspected_references(self) -> usize {
        self.inspected_references
    }

    /// Source-order declaration mappings retained.
    pub const fn retained_references(self) -> usize {
        self.retained_references
    }

    /// Aggregate raw locator bytes inspected, including a terminal N+1 witness.
    pub const fn normalization_bytes_inspected(self) -> usize {
        self.normalization_bytes_inspected
    }

    /// Aggregate path components inspected, including a terminal N+1 witness.
    pub const fn path_components_inspected(self) -> usize {
        self.path_components_inspected
    }

    /// Alias/deduplication probes, including a terminal N+1 witness.
    pub const fn dedup_probes(self) -> usize {
        self.dedup_probes
    }

    /// Rooted external open attempts; aliases do not add another attempt.
    pub const fn external_open_attempts(self) -> usize {
        self.external_open_attempts
    }

    /// Distinct normalized external keys admitted before rooted I/O.
    pub const fn distinct_external_keys(self) -> usize {
        self.distinct_external_keys
    }

    /// Distinct external keys with captured identities.
    pub const fn captured_external_resources(self) -> usize {
        self.captured_external_resources
    }

    /// Aggregate distinct external bytes read and hashed.
    pub const fn external_bytes_read_hashed(self) -> u64 {
        self.external_bytes_read_hashed
    }
}

/// Immutable V1 closure bound to one primary input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DependencyClosureV1 {
    schema: &'static str,
    budget: ResourceClosureBudgetV1,
    primary_input: InputIdentity,
    coverage: DependencyClosureCoverageV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    identity: Option<DependencyClosureIdentityV1>,
    references: Vec<DependencyClosureReferenceV1>,
    external_resources: Vec<ExternalResourceIdentityV1>,
    work: DependencyClosureWorkV1,
}

impl DependencyClosureV1 {
    /// Safe fail-closed value for a loader that has no resource projection.
    pub fn unavailable(primary_input: InputIdentity) -> Self {
        Self {
            schema: DEPENDENCY_CLOSURE_V1_ID,
            budget: ResourceClosureBudgetV1::VALUE,
            primary_input,
            coverage: DependencyClosureCoverageV1::Unavailable {
                reasons: vec![DependencyClosureCoverageReasonV1::SourceDeclarationsUnavailable],
            },
            identity: None,
            references: Vec::new(),
            external_resources: Vec::new(),
            work: DependencyClosureWorkV1::default(),
        }
    }

    pub(crate) fn capture_unavailable(
        primary_input: InputIdentity,
        source_coverage: SourceSetCoverageV1,
    ) -> Self {
        let mut reasons = Vec::with_capacity(2);
        match source_coverage.state() {
            SourceSetCoverageStateV1::Complete => {}
            SourceSetCoverageStateV1::Partial => {
                reasons.push(DependencyClosureCoverageReasonV1::SourceDeclarationsPartial);
            }
            SourceSetCoverageStateV1::Unavailable => {
                reasons.push(DependencyClosureCoverageReasonV1::SourceDeclarationsUnavailable);
            }
        }
        reasons.push(DependencyClosureCoverageReasonV1::CaptureUnavailable);
        reasons.sort_unstable();
        Self {
            schema: DEPENDENCY_CLOSURE_V1_ID,
            budget: ResourceClosureBudgetV1::VALUE,
            primary_input,
            coverage: DependencyClosureCoverageV1::Unavailable { reasons },
            identity: None,
            references: Vec::new(),
            external_resources: Vec::new(),
            work: DependencyClosureWorkV1::default(),
        }
    }

    /// Semantic identity of this value contract.
    pub const fn contract_id(&self) -> &'static str {
        DEPENDENCY_CLOSURE_V1_ID
    }

    /// Immutable V1 budget recorded with the closure.
    pub const fn budget(&self) -> ResourceClosureBudgetV1 {
        self.budget
    }

    /// Exact primary input identity.
    pub const fn primary_input(&self) -> &InputIdentity {
        &self.primary_input
    }

    /// Closure coverage.
    pub const fn coverage(&self) -> &DependencyClosureCoverageV1 {
        &self.coverage
    }

    /// Exact canonical closure identity, present only for complete coverage.
    pub const fn identity(&self) -> Option<&DependencyClosureIdentityV1> {
        self.identity.as_ref()
    }

    /// Source-order declaration mappings retained before any terminal stop.
    pub fn references(&self) -> &[DependencyClosureReferenceV1] {
        &self.references
    }

    /// Distinct external identities in normalized-key order.
    pub fn external_resources(&self) -> &[ExternalResourceIdentityV1] {
        &self.external_resources
    }

    /// Explicit bounded capture work counters.
    pub const fn work(&self) -> DependencyClosureWorkV1 {
        self.work
    }

    pub(crate) fn validate_against(
        &self,
        format: SourceFormatV1,
        primary: &InputIdentity,
        resources: &SourceFactSetV1<SourceResourceReferenceV1>,
    ) -> Result<(), DependencyClosureError> {
        if &self.primary_input != primary {
            return Err(DependencyClosureError::PrimaryIdentityMismatch);
        }
        if self.references.len() > resources.rows().len() {
            return Err(DependencyClosureError::ResourceReferenceCountMismatch {
                facts: resources.rows().len(),
                closure: self.references.len(),
            });
        }
        for (closure, source) in self.references.iter().zip(resources.rows()) {
            if closure.source_order_index != source.source_order_index()
                || closure.kind != source.kind()
                || closure.purpose != DependencyResourcePurposeV1::from_kind(source.kind())
                || closure.source_index != source.source_index()
            {
                return Err(DependencyClosureError::ResourceReferenceMismatch {
                    source_order_index: closure.source_order_index,
                });
            }
            validate_target_against_locator(
                format,
                closure.source_order_index,
                &closure.target,
                source.locator(),
            )?;
        }
        if self.coverage.is_complete()
            && (!matches!(
                resources.coverage().state(),
                SourceSetCoverageStateV1::Complete
            ) || self.references.len() != resources.rows().len())
        {
            return Err(DependencyClosureError::CompleteCoverageMismatch);
        }
        if matches!(
            resources.coverage().state(),
            SourceSetCoverageStateV1::Unavailable
        ) && (!self.references.is_empty()
            || !matches!(
                self.coverage,
                DependencyClosureCoverageV1::Unavailable { .. }
            ))
        {
            return Err(DependencyClosureError::UnavailableCoverageMismatch);
        }
        let reasons = self.coverage.reasons();
        let source_reason_matches = match resources.coverage().state() {
            SourceSetCoverageStateV1::Complete => {
                !reasons.contains(&DependencyClosureCoverageReasonV1::SourceDeclarationsPartial)
                    && !reasons
                        .contains(&DependencyClosureCoverageReasonV1::SourceDeclarationsUnavailable)
            }
            SourceSetCoverageStateV1::Partial => {
                reasons.contains(&DependencyClosureCoverageReasonV1::SourceDeclarationsPartial)
                    && !reasons
                        .contains(&DependencyClosureCoverageReasonV1::SourceDeclarationsUnavailable)
            }
            SourceSetCoverageStateV1::Unavailable => {
                reasons.contains(&DependencyClosureCoverageReasonV1::SourceDeclarationsUnavailable)
                    && !reasons
                        .contains(&DependencyClosureCoverageReasonV1::SourceDeclarationsPartial)
            }
        };
        let capture_reason_matches = !reasons
            .contains(&DependencyClosureCoverageReasonV1::CaptureUnavailable)
            || (matches!(
                self.coverage,
                DependencyClosureCoverageV1::Unavailable { .. }
            ) && self.references.is_empty()
                && self.external_resources.is_empty()
                && self.identity.is_none());
        if !source_reason_matches || !capture_reason_matches {
            return Err(DependencyClosureError::CoverageReasonMismatch);
        }
        if self.coverage.is_complete() != self.identity.is_some() {
            return Err(DependencyClosureError::ClosureIdentityCoverageMismatch);
        }
        Ok(())
    }
}

fn validate_target_against_locator(
    format: SourceFormatV1,
    source_order_index: usize,
    target: &DependencyReferenceTargetV1,
    locator: &SourceResourceLocatorV1,
) -> Result<(), DependencyClosureError> {
    let matches = match locator {
        SourceResourceLocatorV1::Relative(locator) => {
            let syntax = match format {
                SourceFormatV1::GltfJson | SourceFormatV1::Glb => ResourceKeySyntaxV1::GltfUri,
                SourceFormatV1::Fbx => ResourceKeySyntaxV1::ParserRelativePath,
            };
            match DependencyResourceKeyV1::from_relative(locator, syntax) {
                Ok(expected) => match target {
                    DependencyReferenceTargetV1::External { key }
                    | DependencyReferenceTargetV1::Unavailable { key: Some(key), .. } => {
                        if key != &expected {
                            return Err(DependencyClosureError::ResourceKeyMismatch {
                                source_order_index,
                            });
                        }
                        true
                    }
                    DependencyReferenceTargetV1::Refused {
                        key: Some(key),
                        reason: DependencyResourceRefusalReasonV1::Symlink,
                    } => {
                        if key != &expected {
                            return Err(DependencyClosureError::ResourceKeyMismatch {
                                source_order_index,
                            });
                        }
                        true
                    }
                    _ => false,
                },
                Err(
                    DependencyClosureError::ResourceKeyTooLong { .. }
                    | DependencyClosureError::TooManyPathComponents { .. },
                ) => matches!(
                    target,
                    DependencyReferenceTargetV1::Refused {
                        key: None,
                        reason: DependencyResourceRefusalReasonV1::Oversized,
                    }
                ),
                Err(DependencyClosureError::InvalidResourceKey) => matches!(
                    target,
                    DependencyReferenceTargetV1::Refused {
                        key: None,
                        reason: DependencyResourceRefusalReasonV1::Malformed,
                    }
                ),
                Err(_) => false,
            }
        }
        _ => matches!(
            (target, locator),
            (
                DependencyReferenceTargetV1::Primary,
                SourceResourceLocatorV1::Embedded | SourceResourceLocatorV1::DataUri
            ) | (
                DependencyReferenceTargetV1::Unavailable {
                    key: None,
                    reason: DependencyResourceUnavailableReasonV1::Missing,
                },
                SourceResourceLocatorV1::Missing
            ) | (
                DependencyReferenceTargetV1::Refused {
                    key: None,
                    reason: DependencyResourceRefusalReasonV1::Absolute
                },
                SourceResourceLocatorV1::Absolute
            ) | (
                DependencyReferenceTargetV1::Refused {
                    key: None,
                    reason: DependencyResourceRefusalReasonV1::Escaping
                },
                SourceResourceLocatorV1::Escaping
            ) | (
                DependencyReferenceTargetV1::Refused {
                    key: None,
                    reason: DependencyResourceRefusalReasonV1::Remote
                },
                SourceResourceLocatorV1::Remote
            ) | (
                DependencyReferenceTargetV1::Refused {
                    key: None,
                    reason: DependencyResourceRefusalReasonV1::Malformed
                },
                SourceResourceLocatorV1::Malformed
            ) | (
                DependencyReferenceTargetV1::Refused {
                    key: None,
                    reason: DependencyResourceRefusalReasonV1::Oversized
                },
                SourceResourceLocatorV1::Oversized
            )
        ),
    };
    if matches {
        Ok(())
    } else {
        Err(DependencyClosureError::ResourceReferenceMismatch { source_order_index })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingReferenceV1 {
    external_key: Option<DependencyResourceKeyV1>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CachedExternalOutcomeV1 {
    Captured(InputIdentity),
    Refused(DependencyResourceRefusalReasonV1),
    Unavailable(DependencyResourceUnavailableReasonV1),
}

/// Incremental constructor used by format loaders after raw declarations are projected.
pub struct DependencyClosureBuilderV1 {
    primary_input: InputIdentity,
    source_coverage: SourceSetCoverageV1,
    expected_references: usize,
    references: Vec<DependencyClosureReferenceV1>,
    external_resources: BTreeMap<DependencyResourceKeyV1, InputIdentity>,
    external_keys: BTreeSet<DependencyResourceKeyV1>,
    external_outcomes: BTreeMap<DependencyResourceKeyV1, CachedExternalOutcomeV1>,
    opened_external_keys: BTreeSet<DependencyResourceKeyV1>,
    reasons: BTreeSet<DependencyClosureCoverageReasonV1>,
    work: DependencyClosureWorkV1,
    pending_reference: Option<PendingReferenceV1>,
    stopped: bool,
    unmodeled_domain: bool,
}

impl DependencyClosureBuilderV1 {
    /// Begin a closure bound to the raw resource set and exact primary bytes.
    pub fn new(
        primary_input: InputIdentity,
        source_coverage: SourceSetCoverageV1,
        expected_references: usize,
    ) -> Self {
        let mut reasons = BTreeSet::new();
        match source_coverage.state() {
            SourceSetCoverageStateV1::Complete => {}
            SourceSetCoverageStateV1::Partial => {
                reasons.insert(DependencyClosureCoverageReasonV1::SourceDeclarationsPartial);
            }
            SourceSetCoverageStateV1::Unavailable => {
                reasons.insert(DependencyClosureCoverageReasonV1::SourceDeclarationsUnavailable);
            }
        }
        Self {
            primary_input,
            source_coverage,
            expected_references,
            references: Vec::with_capacity(
                expected_references.min(DEPENDENCY_CLOSURE_V1_MAX_REFERENCES),
            ),
            external_resources: BTreeMap::new(),
            external_keys: BTreeSet::new(),
            external_outcomes: BTreeMap::new(),
            opened_external_keys: BTreeSet::new(),
            reasons,
            work: DependencyClosureWorkV1::default(),
            pending_reference: None,
            stopped: false,
            unmodeled_domain: false,
        }
    }

    /// Exact primary identity this builder is bound to.
    pub const fn primary_input(&self) -> &InputIdentity {
        &self.primary_input
    }

    /// Capacity remaining for a single external resource read.
    pub const fn max_resource_bytes(&self) -> u64 {
        DEPENDENCY_CLOSURE_V1_MAX_RESOURCE_BYTES
    }

    /// Aggregate distinct external byte capacity remaining.
    pub const fn remaining_external_bytes(&self) -> u64 {
        DEPENDENCY_CLOSURE_V1_MAX_TOTAL_RESOURCE_BYTES
            .saturating_sub(self.work.external_bytes_read_hashed)
    }

    /// Whether a normalized key already has a captured identity.
    pub fn external_identity(&self, key: &DependencyResourceKeyV1) -> Option<&InputIdentity> {
        self.external_resources.get(key)
    }

    /// Admit one source declaration before normalization/allocation/open work.
    ///
    /// Returns `false` at the first N+1 limit and permanently stops capture.
    pub fn begin_reference(&mut self, locator_bytes: usize, path_components: usize) -> bool {
        if self.stopped || self.pending_reference.is_some() {
            return false;
        }
        self.work.inspected_references = bounded_add(
            self.work.inspected_references,
            1,
            DEPENDENCY_CLOSURE_V1_MAX_REFERENCES,
        );
        self.work.normalization_bytes_inspected = bounded_add(
            self.work.normalization_bytes_inspected,
            locator_bytes,
            DEPENDENCY_CLOSURE_V1_MAX_NORMALIZATION_BYTES,
        );
        self.work.path_components_inspected = self
            .work
            .path_components_inspected
            .saturating_add(path_components.min(DEPENDENCY_CLOSURE_V1_MAX_PATH_COMPONENTS + 1));
        self.work.dedup_probes = bounded_add(
            self.work.dedup_probes,
            1,
            DEPENDENCY_CLOSURE_V1_MAX_DEDUP_PROBES,
        );
        if self.references.len() >= DEPENDENCY_CLOSURE_V1_MAX_REFERENCES
            || locator_bytes > DEPENDENCY_CLOSURE_V1_MAX_KEY_BYTES
            || path_components > DEPENDENCY_CLOSURE_V1_MAX_PATH_COMPONENTS
            || self.work.normalization_bytes_inspected
                > DEPENDENCY_CLOSURE_V1_MAX_NORMALIZATION_BYTES
            || self.work.dedup_probes > DEPENDENCY_CLOSURE_V1_MAX_DEDUP_PROBES
        {
            self.stop_for_budget();
            return false;
        }
        self.pending_reference = Some(PendingReferenceV1 { external_key: None });
        true
    }

    /// Admit one distinct normalized external key before any rooted open.
    ///
    /// `Ok(None)` is the terminal distinct-key N+1 stop. `Ok(Some(false))`
    /// means the key was already admitted and its cached outcome must be reused.
    pub fn prepare_external_key(
        &mut self,
        key: &DependencyResourceKeyV1,
    ) -> Result<Option<bool>, DependencyClosureError> {
        let pending = self
            .pending_reference
            .as_mut()
            .ok_or(DependencyClosureError::ReferenceNotStarted)?;
        if pending.external_key.is_some() {
            return Err(DependencyClosureError::ExternalKeyAlreadyPrepared);
        }
        if self.external_keys.contains(key) {
            if !self.external_outcomes.contains_key(key) {
                return Err(DependencyClosureError::ExternalOutcomeMissing);
            }
            pending.external_key = Some(key.clone());
            return Ok(Some(false));
        }
        if self.external_keys.len() >= DEPENDENCY_CLOSURE_V1_MAX_EXTERNAL_RESOURCES {
            self.pending_reference = None;
            self.stop_for_budget();
            return Ok(None);
        }
        self.external_keys.insert(key.clone());
        pending.external_key = Some(key.clone());
        self.work.distinct_external_keys = self.work.distinct_external_keys.saturating_add(1);
        Ok(Some(true))
    }

    /// Record one rooted open attempt for a newly admitted external key.
    ///
    /// # Errors
    ///
    /// Returns [`DependencyClosureError`] when the key is not prepared or was
    /// already opened. Alias declarations must reuse their cached outcome.
    pub fn record_external_open_attempt(
        &mut self,
        key: &DependencyResourceKeyV1,
    ) -> Result<(), DependencyClosureError> {
        self.require_pending_key(key)?;
        if self.external_outcomes.contains_key(key) {
            return Err(DependencyClosureError::ExternalOutcomeMismatch);
        }
        if !self.opened_external_keys.insert(key.clone()) {
            return Err(DependencyClosureError::DuplicateExternalOpen);
        }
        self.work.external_open_attempts = self.work.external_open_attempts.saturating_add(1);
        Ok(())
    }

    /// Retain an embedded/data/BIN/view-backed declaration mapping.
    pub fn push_primary(
        &mut self,
        source_order_index: usize,
        kind: SourceResourceKindV1,
        source_index: u64,
    ) -> Result<(), DependencyClosureError> {
        self.require_reference_order(source_order_index)?;
        self.require_no_pending_key()?;
        self.push_reference(DependencyClosureReferenceV1::new(
            source_order_index,
            kind,
            source_index,
            DependencyReferenceTargetV1::Primary,
        ))
    }

    /// Retain a declaration refused before opening.
    pub fn push_refused(
        &mut self,
        source_order_index: usize,
        kind: SourceResourceKindV1,
        source_index: u64,
        reason: DependencyResourceRefusalReasonV1,
    ) -> Result<(), DependencyClosureError> {
        self.require_reference_order(source_order_index)?;
        let prepared_key = self.pending_external_key()?.cloned();
        let key = match prepared_key {
            Some(key) if reason == DependencyResourceRefusalReasonV1::Symlink => {
                match self.external_outcomes.get(&key) {
                    Some(CachedExternalOutcomeV1::Refused(cached)) if *cached == reason => {}
                    Some(_) => return Err(DependencyClosureError::ExternalOutcomeMismatch),
                    None => {
                        self.external_outcomes
                            .insert(key.clone(), CachedExternalOutcomeV1::Refused(reason));
                    }
                }
                Some(key)
            }
            Some(_) => return Err(DependencyClosureError::ExternalOutcomeMismatch),
            None if reason == DependencyResourceRefusalReasonV1::Symlink => {
                return Err(DependencyClosureError::ExternalKeyNotPrepared);
            }
            None => None,
        };
        self.reasons
            .insert(DependencyClosureCoverageReasonV1::RefusedResource);
        self.push_reference(DependencyClosureReferenceV1::new(
            source_order_index,
            kind,
            source_index,
            DependencyReferenceTargetV1::Refused { key, reason },
        ))
    }

    /// Retain a safe declaration whose content identity was unavailable.
    pub fn push_unavailable(
        &mut self,
        source_order_index: usize,
        kind: SourceResourceKindV1,
        source_index: u64,
        key: Option<DependencyResourceKeyV1>,
        reason: DependencyResourceUnavailableReasonV1,
    ) -> Result<(), DependencyClosureError> {
        self.require_reference_order(source_order_index)?;
        match &key {
            Some(key) => {
                self.require_pending_key(key)?;
                match self.external_outcomes.get(key) {
                    Some(CachedExternalOutcomeV1::Unavailable(cached)) if *cached == reason => {}
                    Some(_) => return Err(DependencyClosureError::ExternalOutcomeMismatch),
                    None => {
                        self.external_outcomes
                            .insert(key.clone(), CachedExternalOutcomeV1::Unavailable(reason));
                    }
                }
            }
            None => self.require_no_pending_key()?,
        }
        self.reasons
            .insert(DependencyClosureCoverageReasonV1::UnavailableResource);
        self.push_reference(DependencyClosureReferenceV1::new(
            source_order_index,
            kind,
            source_index,
            DependencyReferenceTargetV1::Unavailable { key, reason },
        ))
    }

    /// Retain an alias mapping to an identity already captured under `key`.
    pub fn push_external_alias(
        &mut self,
        source_order_index: usize,
        kind: SourceResourceKindV1,
        source_index: u64,
        key: DependencyResourceKeyV1,
    ) -> Result<(), DependencyClosureError> {
        self.require_reference_order(source_order_index)?;
        self.require_pending_key(&key)?;
        match self.external_outcomes.get(&key) {
            Some(CachedExternalOutcomeV1::Captured(identity))
                if self.external_resources.get(&key) == Some(identity) => {}
            Some(_) => return Err(DependencyClosureError::ExternalOutcomeMismatch),
            None => return Err(DependencyClosureError::ExternalIdentityMissing),
        }
        self.push_external_reference(source_order_index, kind, source_index, key)
    }

    /// Retain a newly captured key and its first declaration mapping.
    ///
    /// Returns `Ok(false)` when the first external-byte N+1 records a typed
    /// unavailable mapping and permanently stops capture.
    pub fn push_captured_external(
        &mut self,
        source_order_index: usize,
        kind: SourceResourceKindV1,
        source_index: u64,
        key: DependencyResourceKeyV1,
        identity: InputIdentity,
    ) -> Result<bool, DependencyClosureError> {
        self.require_reference_order(source_order_index)?;
        self.require_pending_key(&key)?;
        if self.external_outcomes.contains_key(&key) || self.external_resources.contains_key(&key) {
            return Err(DependencyClosureError::ExternalOutcomeMismatch);
        }
        if !self.opened_external_keys.contains(&key) {
            return Err(DependencyClosureError::ExternalKeyNotOpened);
        }
        let bytes = identity.bytes();
        let next_total = self.work.external_bytes_read_hashed.checked_add(bytes);
        if bytes > DEPENDENCY_CLOSURE_V1_MAX_RESOURCE_BYTES
            || next_total.is_none_or(|total| total > DEPENDENCY_CLOSURE_V1_MAX_TOTAL_RESOURCE_BYTES)
        {
            let bounded_observed = bytes.min(DEPENDENCY_CLOSURE_V1_MAX_RESOURCE_BYTES + 1);
            self.work.external_bytes_read_hashed = self
                .work
                .external_bytes_read_hashed
                .saturating_add(bounded_observed)
                .min(DEPENDENCY_CLOSURE_V1_MAX_TOTAL_RESOURCE_BYTES + 1);
            self.push_unavailable(
                source_order_index,
                kind,
                source_index,
                Some(key),
                DependencyResourceUnavailableReasonV1::ResourceBudgetExceeded,
            )?;
            self.stop_for_budget();
            return Ok(false);
        }
        self.work.external_bytes_read_hashed = next_total.unwrap_or(u64::MAX);
        self.work.captured_external_resources =
            self.work.captured_external_resources.saturating_add(1);
        self.external_resources
            .insert(key.clone(), identity.clone());
        self.external_outcomes
            .insert(key.clone(), CachedExternalOutcomeV1::Captured(identity));
        self.push_external_reference(source_order_index, kind, source_index, key)?;
        Ok(true)
    }

    /// Conservatively prevent completeness for a resource-bearing domain not represented by rows.
    pub fn mark_unmodeled_resource_domain(&mut self) {
        self.unmodeled_domain = true;
        self.reasons
            .insert(DependencyClosureCoverageReasonV1::UnmodeledResourceDomain);
    }

    /// Finish the immutable closure and derive its identity only when complete.
    ///
    /// # Errors
    ///
    /// Returns [`DependencyClosureError`] if a declaration was begun but no
    /// typed outcome was supplied.
    pub fn finish(self) -> Result<DependencyClosureV1, DependencyClosureError> {
        if self.pending_reference.is_some() {
            return Err(DependencyClosureError::UnfinishedReference);
        }
        if self.references.len() != self.expected_references && !self.stopped {
            return Err(DependencyClosureError::ReferenceCountMismatch {
                expected: self.expected_references,
                actual: self.references.len(),
            });
        }
        if self.references.len() > self.expected_references {
            return Err(DependencyClosureError::ReferenceCountMismatch {
                expected: self.expected_references,
                actual: self.references.len(),
            });
        }
        let coverage = match self.source_coverage.state() {
            SourceSetCoverageStateV1::Unavailable => DependencyClosureCoverageV1::Unavailable {
                reasons: self.reasons.into_iter().collect(),
            },
            SourceSetCoverageStateV1::Complete
                if self.reasons.is_empty()
                    && !self.unmodeled_domain
                    && self.references.len() == self.expected_references
                    && self.references.iter().all(|reference| {
                        matches!(
                            reference.target,
                            DependencyReferenceTargetV1::Primary
                                | DependencyReferenceTargetV1::External { .. }
                        )
                    }) =>
            {
                DependencyClosureCoverageV1::Complete
            }
            _ => DependencyClosureCoverageV1::Partial {
                reasons: self.reasons.into_iter().collect(),
            },
        };
        let external_resources = self
            .external_resources
            .into_iter()
            .map(|(key, identity)| ExternalResourceIdentityV1 { key, identity })
            .collect::<Vec<_>>();
        let identity = coverage.is_complete().then(|| {
            canonical_identity(&self.primary_input, &self.references, &external_resources)
        });
        Ok(DependencyClosureV1 {
            schema: DEPENDENCY_CLOSURE_V1_ID,
            budget: ResourceClosureBudgetV1::VALUE,
            primary_input: self.primary_input,
            coverage,
            identity,
            references: self.references,
            external_resources,
            work: self.work,
        })
    }

    fn push_external_reference(
        &mut self,
        source_order_index: usize,
        kind: SourceResourceKindV1,
        source_index: u64,
        key: DependencyResourceKeyV1,
    ) -> Result<(), DependencyClosureError> {
        self.push_reference(DependencyClosureReferenceV1::new(
            source_order_index,
            kind,
            source_index,
            DependencyReferenceTargetV1::External { key },
        ))
    }

    fn push_reference(
        &mut self,
        reference: DependencyClosureReferenceV1,
    ) -> Result<(), DependencyClosureError> {
        if self.pending_reference.is_none() {
            return Err(DependencyClosureError::ReferenceNotStarted);
        }
        let expected = self.references.len();
        if reference.source_order_index != expected {
            return Err(DependencyClosureError::NonCanonicalReferenceOrder {
                expected,
                actual: reference.source_order_index,
            });
        }
        self.pending_reference = None;
        self.work.retained_references = self.work.retained_references.saturating_add(1);
        self.references.push(reference);
        Ok(())
    }

    fn stop_for_budget(&mut self) {
        self.reasons
            .insert(DependencyClosureCoverageReasonV1::ResourceBudgetExceeded);
        self.stopped = true;
    }

    fn pending_external_key(
        &self,
    ) -> Result<Option<&DependencyResourceKeyV1>, DependencyClosureError> {
        self.pending_reference
            .as_ref()
            .map(|pending| pending.external_key.as_ref())
            .ok_or(DependencyClosureError::ReferenceNotStarted)
    }

    fn require_no_pending_key(&self) -> Result<(), DependencyClosureError> {
        match self.pending_external_key()? {
            None => Ok(()),
            Some(_) => Err(DependencyClosureError::ExternalOutcomeMismatch),
        }
    }

    fn require_pending_key(
        &self,
        key: &DependencyResourceKeyV1,
    ) -> Result<(), DependencyClosureError> {
        match self.pending_external_key()? {
            Some(pending) if pending == key => Ok(()),
            Some(_) => Err(DependencyClosureError::ExternalKeyMismatch),
            None => Err(DependencyClosureError::ExternalKeyNotPrepared),
        }
    }

    fn require_reference_order(
        &self,
        source_order_index: usize,
    ) -> Result<(), DependencyClosureError> {
        let expected = self.references.len();
        if source_order_index == expected {
            Ok(())
        } else {
            Err(DependencyClosureError::NonCanonicalReferenceOrder {
                expected,
                actual: source_order_index,
            })
        }
    }
}

fn bounded_add(current: usize, observed: usize, limit: usize) -> usize {
    current
        .saturating_add(observed)
        .min(limit.saturating_add(1))
}

fn canonical_identity(
    primary: &InputIdentity,
    references: &[DependencyClosureReferenceV1],
    resources: &[ExternalResourceIdentityV1],
) -> DependencyClosureIdentityV1 {
    let mut bytes = Vec::new();
    encode_text(&mut bytes, DEPENDENCY_CLOSURE_V1_ID);
    encode_text(&mut bytes, DEPENDENCY_CLOSURE_BUDGET_V1_ID);
    encode_u64(&mut bytes, DEPENDENCY_CLOSURE_V1_MAX_REFERENCES as u64);
    encode_u64(
        &mut bytes,
        DEPENDENCY_CLOSURE_V1_MAX_EXTERNAL_RESOURCES as u64,
    );
    encode_u64(&mut bytes, DEPENDENCY_CLOSURE_V1_MAX_KEY_BYTES as u64);
    encode_u64(&mut bytes, DEPENDENCY_CLOSURE_V1_MAX_PATH_COMPONENTS as u64);
    encode_u64(
        &mut bytes,
        DEPENDENCY_CLOSURE_V1_MAX_NORMALIZATION_BYTES as u64,
    );
    encode_u64(&mut bytes, DEPENDENCY_CLOSURE_V1_MAX_RESOURCE_BYTES);
    encode_u64(&mut bytes, DEPENDENCY_CLOSURE_V1_MAX_TOTAL_RESOURCE_BYTES);
    encode_u64(&mut bytes, DEPENDENCY_CLOSURE_V1_MAX_DEDUP_PROBES as u64);
    bytes.push(0); // DependencyClosureCoverageV1::Complete
    encode_identity(&mut bytes, primary);
    encode_u64(&mut bytes, references.len() as u64);
    for reference in references {
        encode_u64(&mut bytes, reference.source_order_index as u64);
        bytes.push(resource_kind_tag(reference.kind));
        bytes.push(resource_purpose_tag(reference.purpose));
        encode_u64(&mut bytes, reference.source_index);
        match &reference.target {
            DependencyReferenceTargetV1::Primary => bytes.push(0),
            DependencyReferenceTargetV1::External { key } => {
                bytes.push(1);
                encode_text(&mut bytes, key.as_str());
            }
            DependencyReferenceTargetV1::Refused { .. }
            | DependencyReferenceTargetV1::Unavailable { .. } => {
                unreachable!("only complete reference targets enter closure identity")
            }
        }
    }
    encode_u64(&mut bytes, resources.len() as u64);
    for resource in resources {
        encode_text(&mut bytes, resource.key.as_str());
        encode_identity(&mut bytes, &resource.identity);
    }
    DependencyClosureIdentityV1(InputIdentity::from_bytes(&bytes))
}

fn encode_identity(bytes: &mut Vec<u8>, identity: &InputIdentity) {
    encode_text(bytes, identity.sha256());
    encode_u64(bytes, identity.bytes());
}

fn encode_text(bytes: &mut Vec<u8>, value: &str) {
    encode_u64(bytes, value.len() as u64);
    bytes.extend_from_slice(value.as_bytes());
}

fn encode_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn resource_kind_tag(kind: SourceResourceKindV1) -> u8 {
    match kind {
        SourceResourceKindV1::Buffer => 0,
        SourceResourceKindV1::Image => 1,
        SourceResourceKindV1::Texture => 2,
        SourceResourceKindV1::Video => 3,
        SourceResourceKindV1::Cache => 4,
    }
}

fn resource_purpose_tag(purpose: DependencyResourcePurposeV1) -> u8 {
    match purpose {
        DependencyResourcePurposeV1::LoaderEssential => 0,
        DependencyResourcePurposeV1::Nonessential => 1,
        DependencyResourcePurposeV1::TargetOnly => 2,
    }
}

/// Invalid closure construction or source-fact binding invariant.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum DependencyClosureError {
    /// A logical key exceeded the V1 byte limit.
    #[error("dependency resource key is {bytes} bytes, exceeding the V1 limit of {limit}")]
    ResourceKeyTooLong {
        /// Observed UTF-8 bytes.
        bytes: usize,
        /// Public V1 limit.
        limit: usize,
    },
    /// A source-relative key was malformed, absolute, remote, or escaping.
    #[error("dependency resource key is invalid or unsafe")]
    InvalidResourceKey,
    /// A key exceeded the component limit.
    #[error(
        "dependency resource key has {components} components, exceeding the V1 limit of {limit}"
    )]
    TooManyPathComponents {
        /// Observed components.
        components: usize,
        /// Public V1 limit.
        limit: usize,
    },
    /// A declaration outcome was supplied without successful preflight.
    #[error("dependency reference outcome was supplied without begin_reference")]
    ReferenceNotStarted,
    /// One declaration attempted to prepare more than one external key.
    #[error("dependency reference already has a prepared external key")]
    ExternalKeyAlreadyPrepared,
    /// A begun declaration did not receive a typed outcome.
    #[error("dependency reference was begun but no outcome was supplied")]
    UnfinishedReference,
    /// Reference rows were not a zero-based source-order prefix.
    #[error("dependency reference order {actual} is not expected prefix index {expected}")]
    NonCanonicalReferenceOrder {
        /// Expected source-order index.
        expected: usize,
        /// Actual source-order index.
        actual: usize,
    },
    /// An alias referenced a key not yet captured.
    #[error("dependency alias references an external key without an identity")]
    ExternalIdentityMissing,
    /// A loader used an external key without admitting it through the bounded key set.
    #[error("dependency external key was not prepared for rooted capture")]
    ExternalKeyNotPrepared,
    /// An outcome key differed from the exact key prepared for this declaration.
    #[error("dependency external key does not match the prepared reference key")]
    ExternalKeyMismatch,
    /// A cached key had no terminal captured/refused/unavailable outcome.
    #[error("dependency external key has no cached outcome")]
    ExternalOutcomeMissing,
    /// An alias or retry contradicted the cached outcome for its normalized key.
    #[error("dependency external outcome contradicts the cached key outcome")]
    ExternalOutcomeMismatch,
    /// A loader supplied an external identity without recording the same capture's open.
    #[error("dependency external key was not opened before capture")]
    ExternalKeyNotOpened,
    /// A normalized key was opened more than once instead of reusing its cached outcome.
    #[error("dependency external key was opened more than once")]
    DuplicateExternalOpen,
    /// The retained reference count differed from the declared source-row count.
    #[error("dependency closure retained {actual} references but expected {expected}")]
    ReferenceCountMismatch {
        /// Source rows the builder was required to map.
        expected: usize,
        /// Typed reference outcomes actually retained.
        actual: usize,
    },
    /// Closure and raw facts used different primary identities.
    #[error("dependency closure primary identity does not match raw source facts")]
    PrimaryIdentityMismatch,
    /// Closure retained more reference rows than raw facts.
    #[error("dependency closure has {closure} references but raw facts retain {facts}")]
    ResourceReferenceCountMismatch {
        /// Raw-fact rows.
        facts: usize,
        /// Closure rows.
        closure: usize,
    },
    /// A closure reference did not match the raw source row at the same position.
    #[error("dependency reference {source_order_index} does not match raw source facts")]
    ResourceReferenceMismatch {
        /// Mismatched source-order index.
        source_order_index: usize,
    },
    /// A closure target key was not the format-specific normalization of its raw locator.
    #[error("dependency reference {source_order_index} key does not match raw source facts")]
    ResourceKeyMismatch {
        /// Mismatched source-order index.
        source_order_index: usize,
    },
    /// Complete closure was claimed without complete, fully mapped raw declarations.
    #[error("complete dependency closure does not match complete raw resource coverage")]
    CompleteCoverageMismatch,
    /// Raw declaration coverage was unavailable but closure claimed retained mappings.
    #[error("unavailable raw resource coverage requires unavailable empty dependency closure")]
    UnavailableCoverageMismatch,
    /// Closure coverage reasons contradicted raw declaration coverage or retained rows.
    #[error("dependency closure coverage reasons do not match raw resource coverage")]
    CoverageReasonMismatch,
    /// Complete-only identity presence disagreed with closure coverage.
    #[error("dependency closure identity must be present exactly for complete coverage")]
    ClosureIdentityCoverageMismatch,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        SourceFactSetV1, SourceLoaderDispositionV1, SourceProvenanceV1, SourceResourceLocatorV1,
    };

    fn source_resource(
        order: usize,
        kind: SourceResourceKindV1,
        source_index: u64,
        locator: SourceResourceLocatorV1,
    ) -> SourceResourceReferenceV1 {
        SourceResourceReferenceV1::new(
            order,
            kind,
            source_index,
            locator,
            SourceLoaderDispositionV1::Preserved,
            SourceProvenanceV1::format_defined(),
        )
    }

    fn relative(value: &str) -> SourceRelativeLocatorV1 {
        let SourceResourceLocatorV1::Relative(value) = SourceResourceLocatorV1::classify(value)
        else {
            panic!("fixture must be safe relative")
        };
        value
    }

    #[test]
    fn gltf_percent_aliases_normalize_but_fbx_percent_is_literal() {
        let escaped = relative("textures/a%20b.png");
        let plain = relative("textures/a b.png");
        assert_eq!(
            DependencyResourceKeyV1::from_relative(&escaped, ResourceKeySyntaxV1::GltfUri).unwrap(),
            DependencyResourceKeyV1::from_relative(&plain, ResourceKeySyntaxV1::GltfUri).unwrap()
        );
        assert_ne!(
            DependencyResourceKeyV1::from_relative(
                &escaped,
                ResourceKeySyntaxV1::ParserRelativePath
            )
            .unwrap(),
            DependencyResourceKeyV1::from_relative(&plain, ResourceKeySyntaxV1::ParserRelativePath)
                .unwrap()
        );
    }

    #[test]
    fn precomputed_digest_constructor_preserves_the_canonical_identity_shape() {
        let identity = InputIdentity::from_sha256_digest([0xab; 32], 17);
        assert_eq!(identity.sha256(), "ab".repeat(32));
        assert_eq!(identity.bytes(), 17);
    }

    #[test]
    fn normalized_keys_reject_encoded_escapes_controls_and_component_n_plus_one() {
        for value in [
            "",
            "/absolute.bin",
            "C:/drive.bin",
            "file:secret.bin",
            "https://example.invalid/a.bin",
            "a\\b.bin",
            "a/./b.bin",
            "a/../b.bin",
            "a?query.bin",
            "a#fragment.bin",
            "a\ncontrol.bin",
            "a/%2f/b",
            "a/%5c/b",
            "a/%00/b",
            "a/%ff/b",
            "a/%zz/b",
        ] {
            assert!(
                matches!(
                    DependencyResourceKeyV1::from_source_str(value, ResourceKeySyntaxV1::GltfUri),
                    Err(DependencyClosureError::InvalidResourceKey)
                ),
                "unexpected safe key: {value:?}"
            );
        }
        let at_limit = (0..DEPENDENCY_CLOSURE_V1_MAX_PATH_COMPONENTS)
            .map(|_| "a")
            .collect::<Vec<_>>()
            .join("/");
        assert!(
            DependencyResourceKeyV1::from_relative(
                &relative(&at_limit),
                ResourceKeySyntaxV1::ParserRelativePath
            )
            .is_ok()
        );
        let over = format!("{at_limit}/a");
        assert!(matches!(
            DependencyResourceKeyV1::from_relative(
                &relative(&over),
                ResourceKeySyntaxV1::ParserRelativePath
            ),
            Err(DependencyClosureError::TooManyPathComponents { .. })
        ));
    }

    #[test]
    fn complete_closure_deduplicates_aliases_and_changes_with_external_identity() {
        let primary = InputIdentity::from_bytes(b"primary");
        let key = DependencyResourceKeyV1::from_relative(
            &relative("a.bin"),
            ResourceKeySyntaxV1::GltfUri,
        )
        .unwrap();
        let mut builder =
            DependencyClosureBuilderV1::new(primary.clone(), SourceSetCoverageV1::complete(), 2);
        assert!(builder.begin_reference(5, 1));
        assert_eq!(builder.prepare_external_key(&key).unwrap(), Some(true));
        builder.record_external_open_attempt(&key).unwrap();
        assert!(
            builder
                .push_captured_external(
                    0,
                    SourceResourceKindV1::Buffer,
                    0,
                    key.clone(),
                    InputIdentity::from_bytes(b"one"),
                )
                .unwrap()
        );
        assert!(builder.begin_reference(7, 1));
        assert_eq!(builder.prepare_external_key(&key).unwrap(), Some(false));
        builder
            .push_external_alias(1, SourceResourceKindV1::Image, 0, key.clone())
            .unwrap();
        let first = builder.finish().unwrap();
        assert!(first.coverage().is_complete());
        let identity = first.identity().expect("complete closure identity");
        assert_eq!(
            identity.input_identity().sha256(),
            "43fccaa09b2616c57863a1186b88d8d674cb404b4e99c35b18a239a4d4b782ad"
        );
        assert_eq!(identity.input_identity().bytes(), 409);
        assert_eq!(first.external_resources().len(), 1);
        assert_eq!(first.work().external_open_attempts(), 1);
        let wire = serde_json::to_value(&first).unwrap();
        assert_eq!(wire["schema"], DEPENDENCY_CLOSURE_V1_ID);
        assert_eq!(wire["budget"]["schema"], DEPENDENCY_CLOSURE_BUDGET_V1_ID);
        assert_eq!(
            wire["budget"]["max_external_resources"],
            DEPENDENCY_CLOSURE_V1_MAX_EXTERNAL_RESOURCES
        );
        assert_eq!(wire["coverage"]["state"], "complete");
        assert_eq!(
            wire["identity"]["sha256"],
            "43fccaa09b2616c57863a1186b88d8d674cb404b4e99c35b18a239a4d4b782ad"
        );

        let mut changed =
            DependencyClosureBuilderV1::new(primary, SourceSetCoverageV1::complete(), 2);
        assert!(changed.begin_reference(5, 1));
        let changed_key = DependencyResourceKeyV1::from_relative(
            &relative("a.bin"),
            ResourceKeySyntaxV1::GltfUri,
        )
        .unwrap();
        assert_eq!(
            changed.prepare_external_key(&changed_key).unwrap(),
            Some(true)
        );
        changed.record_external_open_attempt(&changed_key).unwrap();
        assert!(
            changed
                .push_captured_external(
                    0,
                    SourceResourceKindV1::Buffer,
                    0,
                    changed_key,
                    InputIdentity::from_bytes(b"two"),
                )
                .unwrap()
        );
        assert!(changed.begin_reference(7, 1));
        assert_eq!(changed.prepare_external_key(&key).unwrap(), Some(false));
        changed
            .push_external_alias(1, SourceResourceKindV1::Image, 0, key)
            .unwrap();
        let changed = changed.finish().unwrap();
        assert_eq!(first.references().len(), changed.references().len());
        assert_eq!(first.references()[0].kind(), changed.references()[0].kind());
        assert_eq!(first.references()[1].kind(), changed.references()[1].kind());
        assert_ne!(first.identity(), changed.identity());
    }

    #[test]
    fn partial_and_unavailable_closures_never_claim_identity() {
        let primary = InputIdentity::from_bytes(b"primary");
        let mut partial =
            DependencyClosureBuilderV1::new(primary.clone(), SourceSetCoverageV1::complete(), 1);
        assert!(partial.begin_reference(0, 0));
        partial
            .push_refused(
                0,
                SourceResourceKindV1::Image,
                0,
                DependencyResourceRefusalReasonV1::Remote,
            )
            .unwrap();
        let partial = partial.finish().unwrap();
        assert!(matches!(
            partial.coverage(),
            DependencyClosureCoverageV1::Partial { .. }
        ));
        assert!(partial.identity().is_none());

        let unavailable = DependencyClosureV1::unavailable(primary);
        assert!(matches!(
            unavailable.coverage(),
            DependencyClosureCoverageV1::Unavailable { .. }
        ));
        assert!(unavailable.identity().is_none());
    }

    #[test]
    fn reference_limit_stops_at_n_plus_one_and_never_resumes() {
        let primary = InputIdentity::from_bytes(b"primary");
        let mut builder = DependencyClosureBuilderV1::new(
            primary,
            SourceSetCoverageV1::complete(),
            DEPENDENCY_CLOSURE_V1_MAX_REFERENCES + 2,
        );
        for index in 0..DEPENDENCY_CLOSURE_V1_MAX_REFERENCES {
            assert!(builder.begin_reference(0, 0));
            builder
                .push_primary(index, SourceResourceKindV1::Image, index as u64)
                .unwrap();
        }
        assert!(!builder.begin_reference(0, 0));
        assert!(!builder.begin_reference(0, 0));
        let closure = builder.finish().unwrap();
        assert_eq!(
            closure.references().len(),
            DEPENDENCY_CLOSURE_V1_MAX_REFERENCES
        );
        assert_eq!(
            closure.work().inspected_references(),
            DEPENDENCY_CLOSURE_V1_MAX_REFERENCES + 1
        );
        assert_eq!(
            closure.work().dedup_probes(),
            DEPENDENCY_CLOSURE_V1_MAX_DEDUP_PROBES + 1
        );
        assert!(matches!(
            closure.coverage(),
            DependencyClosureCoverageV1::Partial { reasons }
                if reasons.contains(&DependencyClosureCoverageReasonV1::ResourceBudgetExceeded)
        ));
    }

    #[test]
    fn closure_binding_checks_primary_and_exact_resource_prefix() {
        let primary = InputIdentity::from_bytes(b"primary");
        let source_rows = SourceFactSetV1::complete(vec![source_resource(
            0,
            SourceResourceKindV1::Image,
            7,
            SourceResourceLocatorV1::Embedded,
        )]);
        let mut builder = DependencyClosureBuilderV1::new(
            primary.clone(),
            source_rows.coverage(),
            source_rows.rows().len(),
        );
        assert!(builder.begin_reference(0, 0));
        builder
            .push_primary(0, SourceResourceKindV1::Image, 7)
            .unwrap();
        let closure = builder.finish().unwrap();
        closure
            .validate_against(SourceFormatV1::Glb, &primary, &source_rows)
            .unwrap();
        assert_eq!(
            closure.validate_against(
                SourceFormatV1::Glb,
                &InputIdentity::from_bytes(b"other"),
                &source_rows,
            ),
            Err(DependencyClosureError::PrimaryIdentityMismatch)
        );

        let mut wrong_target = closure.clone();
        wrong_target.references[0].target = DependencyReferenceTargetV1::Refused {
            key: None,
            reason: DependencyResourceRefusalReasonV1::Remote,
        };
        assert_eq!(
            wrong_target.validate_against(SourceFormatV1::Glb, &primary, &source_rows),
            Err(DependencyClosureError::ResourceReferenceMismatch {
                source_order_index: 0
            })
        );
    }

    #[test]
    fn key_and_normalization_byte_limits_are_exact_and_terminal() {
        let at_key_limit = "a".repeat(DEPENDENCY_CLOSURE_V1_MAX_KEY_BYTES);
        assert!(
            DependencyResourceKeyV1::from_source_str(
                &at_key_limit,
                ResourceKeySyntaxV1::ParserRelativePath
            )
            .is_ok()
        );
        let over_key_limit = "a".repeat(DEPENDENCY_CLOSURE_V1_MAX_KEY_BYTES + 1);
        let error = DependencyResourceKeyV1::from_source_str(
            &over_key_limit,
            ResourceKeySyntaxV1::ParserRelativePath,
        )
        .unwrap_err();
        assert_eq!(
            error,
            DependencyClosureError::ResourceKeyTooLong {
                bytes: DEPENDENCY_CLOSURE_V1_MAX_KEY_BYTES + 1,
                limit: DEPENDENCY_CLOSURE_V1_MAX_KEY_BYTES,
            }
        );

        let rows =
            DEPENDENCY_CLOSURE_V1_MAX_NORMALIZATION_BYTES / DEPENDENCY_CLOSURE_V1_MAX_KEY_BYTES;
        let mut builder = DependencyClosureBuilderV1::new(
            InputIdentity::from_bytes(b"primary"),
            SourceSetCoverageV1::complete(),
            rows + 1,
        );
        for index in 0..rows {
            assert!(builder.begin_reference(DEPENDENCY_CLOSURE_V1_MAX_KEY_BYTES, 1));
            builder
                .push_primary(index, SourceResourceKindV1::Image, index as u64)
                .unwrap();
        }
        assert!(!builder.begin_reference(1, 1));
        assert!(!builder.begin_reference(0, 0));
        let closure = builder.finish().unwrap();
        assert_eq!(
            closure.work().normalization_bytes_inspected(),
            DEPENDENCY_CLOSURE_V1_MAX_NORMALIZATION_BYTES + 1
        );
        assert_eq!(closure.work().inspected_references(), rows + 1);
        assert_eq!(closure.work().path_components_inspected(), rows + 1);
        assert_eq!(closure.work().dedup_probes(), rows + 1);
    }

    #[test]
    fn distinct_external_key_limit_stops_before_n_plus_one_open() {
        let primary = InputIdentity::from_bytes(b"primary");
        let mut builder = DependencyClosureBuilderV1::new(
            primary,
            SourceSetCoverageV1::complete(),
            DEPENDENCY_CLOSURE_V1_MAX_EXTERNAL_RESOURCES + 1,
        );
        for index in 0..DEPENDENCY_CLOSURE_V1_MAX_EXTERNAL_RESOURCES {
            assert!(builder.begin_reference(8, 1));
            let key = DependencyResourceKeyV1::from_source_str(
                &format!("r{index}.bin"),
                ResourceKeySyntaxV1::ParserRelativePath,
            )
            .unwrap();
            assert_eq!(builder.prepare_external_key(&key).unwrap(), Some(true));
            builder.record_external_open_attempt(&key).unwrap();
            assert!(
                builder
                    .push_captured_external(
                        index,
                        SourceResourceKindV1::Buffer,
                        index as u64,
                        key,
                        InputIdentity::from_bytes(&[]),
                    )
                    .unwrap()
            );
        }
        assert!(builder.begin_reference(8, 1));
        let overflow = DependencyResourceKeyV1::from_source_str(
            "overflow.bin",
            ResourceKeySyntaxV1::ParserRelativePath,
        )
        .unwrap();
        assert_eq!(builder.prepare_external_key(&overflow).unwrap(), None);
        assert_eq!(
            builder.record_external_open_attempt(&overflow),
            Err(DependencyClosureError::ReferenceNotStarted)
        );
        let closure = builder.finish().unwrap();
        assert_eq!(
            closure.work().distinct_external_keys(),
            DEPENDENCY_CLOSURE_V1_MAX_EXTERNAL_RESOURCES
        );
        assert_eq!(
            closure.work().inspected_references(),
            DEPENDENCY_CLOSURE_V1_MAX_EXTERNAL_RESOURCES + 1
        );
        assert_eq!(
            closure.work().dedup_probes(),
            DEPENDENCY_CLOSURE_V1_MAX_EXTERNAL_RESOURCES + 1
        );
        assert_eq!(
            closure.work().external_open_attempts(),
            DEPENDENCY_CLOSURE_V1_MAX_EXTERNAL_RESOURCES
        );
        assert_eq!(
            closure.external_resources().len(),
            DEPENDENCY_CLOSURE_V1_MAX_EXTERNAL_RESOURCES
        );
    }

    #[test]
    fn captured_external_identity_requires_the_recorded_same_capture_open() {
        let mut builder = DependencyClosureBuilderV1::new(
            InputIdentity::from_bytes(b"primary"),
            SourceSetCoverageV1::complete(),
            1,
        );
        assert!(builder.begin_reference(5, 1));
        let key = DependencyResourceKeyV1::from_source_str(
            "a.bin",
            ResourceKeySyntaxV1::ParserRelativePath,
        )
        .unwrap();
        assert_eq!(builder.prepare_external_key(&key).unwrap(), Some(true));
        assert_eq!(
            builder.push_captured_external(
                0,
                SourceResourceKindV1::Buffer,
                0,
                key.clone(),
                InputIdentity::from_bytes(b"bytes"),
            ),
            Err(DependencyClosureError::ExternalKeyNotOpened)
        );
        builder.record_external_open_attempt(&key).unwrap();
        assert!(
            builder
                .push_captured_external(
                    0,
                    SourceResourceKindV1::Buffer,
                    0,
                    key,
                    InputIdentity::from_bytes(b"bytes"),
                )
                .unwrap()
        );
    }

    #[test]
    fn external_byte_limits_are_exact_without_allocating_fixture_payloads() {
        let identity = |tag: u8, bytes| InputIdentity::from_sha256_digest([tag; 32], bytes);
        let key = |index| {
            DependencyResourceKeyV1::from_source_str(
                &format!("r{index}.bin"),
                ResourceKeySyntaxV1::ParserRelativePath,
            )
            .unwrap()
        };

        let mut exact = DependencyClosureBuilderV1::new(
            InputIdentity::from_bytes(b"primary"),
            SourceSetCoverageV1::complete(),
            4,
        );
        for index in 0..4 {
            assert!(exact.begin_reference(8, 1));
            let key = key(index);
            assert_eq!(exact.prepare_external_key(&key).unwrap(), Some(true));
            exact.record_external_open_attempt(&key).unwrap();
            assert!(
                exact
                    .push_captured_external(
                        index,
                        SourceResourceKindV1::Buffer,
                        index as u64,
                        key,
                        identity(index as u8, DEPENDENCY_CLOSURE_V1_MAX_RESOURCE_BYTES),
                    )
                    .unwrap()
            );
        }
        let exact = exact.finish().unwrap();
        assert!(exact.coverage().is_complete());
        assert_eq!(
            exact.work().external_bytes_read_hashed(),
            DEPENDENCY_CLOSURE_V1_MAX_TOTAL_RESOURCE_BYTES
        );

        let mut per_resource_over = DependencyClosureBuilderV1::new(
            InputIdentity::from_bytes(b"primary"),
            SourceSetCoverageV1::complete(),
            1,
        );
        assert!(per_resource_over.begin_reference(8, 1));
        let over_key = key(9);
        assert_eq!(
            per_resource_over.prepare_external_key(&over_key).unwrap(),
            Some(true)
        );
        per_resource_over
            .record_external_open_attempt(&over_key)
            .unwrap();
        assert!(
            !per_resource_over
                .push_captured_external(
                    0,
                    SourceResourceKindV1::Buffer,
                    0,
                    over_key,
                    identity(9, DEPENDENCY_CLOSURE_V1_MAX_RESOURCE_BYTES + 1),
                )
                .unwrap()
        );
        let per_resource_over = per_resource_over.finish().unwrap();
        assert_eq!(per_resource_over.references().len(), 1);
        assert!(matches!(
            per_resource_over.references()[0].target(),
            DependencyReferenceTargetV1::Unavailable {
                reason: DependencyResourceUnavailableReasonV1::ResourceBudgetExceeded,
                ..
            }
        ));
        assert_eq!(
            per_resource_over.work().external_bytes_read_hashed(),
            DEPENDENCY_CLOSURE_V1_MAX_RESOURCE_BYTES + 1
        );
        assert!(matches!(
            per_resource_over.coverage(),
            DependencyClosureCoverageV1::Partial { reasons }
                if reasons.contains(&DependencyClosureCoverageReasonV1::ResourceBudgetExceeded)
                    && reasons.contains(&DependencyClosureCoverageReasonV1::UnavailableResource)
        ));

        let mut aggregate_over = DependencyClosureBuilderV1::new(
            InputIdentity::from_bytes(b"primary"),
            SourceSetCoverageV1::complete(),
            5,
        );
        for index in 0..4 {
            assert!(aggregate_over.begin_reference(8, 1));
            let key = key(index);
            assert_eq!(
                aggregate_over.prepare_external_key(&key).unwrap(),
                Some(true)
            );
            aggregate_over.record_external_open_attempt(&key).unwrap();
            assert!(
                aggregate_over
                    .push_captured_external(
                        index,
                        SourceResourceKindV1::Buffer,
                        index as u64,
                        key,
                        identity(index as u8, DEPENDENCY_CLOSURE_V1_MAX_RESOURCE_BYTES),
                    )
                    .unwrap()
            );
        }
        assert!(aggregate_over.begin_reference(8, 1));
        let fifth = key(5);
        assert_eq!(
            aggregate_over.prepare_external_key(&fifth).unwrap(),
            Some(true)
        );
        aggregate_over.record_external_open_attempt(&fifth).unwrap();
        assert!(
            !aggregate_over
                .push_captured_external(4, SourceResourceKindV1::Buffer, 4, fifth, identity(5, 1),)
                .unwrap()
        );
        let aggregate_over = aggregate_over.finish().unwrap();
        assert_eq!(aggregate_over.references().len(), 5);
        assert_eq!(aggregate_over.external_resources().len(), 4);
        assert_eq!(aggregate_over.work().inspected_references(), 5);
        assert_eq!(
            aggregate_over.work().external_bytes_read_hashed(),
            DEPENDENCY_CLOSURE_V1_MAX_TOTAL_RESOURCE_BYTES + 1
        );
        assert!(matches!(
            aggregate_over.coverage(),
            DependencyClosureCoverageV1::Partial { reasons }
                if reasons.contains(&DependencyClosureCoverageReasonV1::ResourceBudgetExceeded)
        ));
    }

    #[test]
    fn source_order_is_identity_bearing_while_external_rows_remain_key_sorted() {
        fn closure(order: [(&str, u64); 2]) -> DependencyClosureV1 {
            let mut builder = DependencyClosureBuilderV1::new(
                InputIdentity::from_bytes(b"primary"),
                SourceSetCoverageV1::complete(),
                2,
            );
            for (source_order, (name, source_index)) in order.into_iter().enumerate() {
                assert!(builder.begin_reference(name.len(), 1));
                let key = DependencyResourceKeyV1::from_source_str(
                    name,
                    ResourceKeySyntaxV1::ParserRelativePath,
                )
                .unwrap();
                assert_eq!(builder.prepare_external_key(&key).unwrap(), Some(true));
                builder.record_external_open_attempt(&key).unwrap();
                assert!(
                    builder
                        .push_captured_external(
                            source_order,
                            SourceResourceKindV1::Image,
                            source_index,
                            key,
                            InputIdentity::from_bytes(name.as_bytes()),
                        )
                        .unwrap()
                );
            }
            builder.finish().unwrap()
        }

        let first = closure([("z.png", 0), ("a.png", 1)]);
        let second = closure([("a.png", 1), ("z.png", 0)]);
        assert_eq!(
            first
                .external_resources()
                .iter()
                .map(|row| row.key().as_str())
                .collect::<Vec<_>>(),
            vec!["a.png", "z.png"]
        );
        assert_ne!(first.identity(), second.identity());
    }

    #[test]
    fn refused_and_unavailable_serialization_never_has_an_unsafe_spelling() {
        let mut builder = DependencyClosureBuilderV1::new(
            InputIdentity::from_bytes(b"primary"),
            SourceSetCoverageV1::complete(),
            2,
        );
        assert!(builder.begin_reference(0, 0));
        builder
            .push_refused(
                0,
                SourceResourceKindV1::Image,
                0,
                DependencyResourceRefusalReasonV1::Absolute,
            )
            .unwrap();
        assert!(builder.begin_reference(8, 1));
        let key = DependencyResourceKeyV1::from_source_str(
            "safe.png",
            ResourceKeySyntaxV1::ParserRelativePath,
        )
        .unwrap();
        assert_eq!(builder.prepare_external_key(&key).unwrap(), Some(true));
        builder
            .push_unavailable(
                1,
                SourceResourceKindV1::Image,
                1,
                Some(key),
                DependencyResourceUnavailableReasonV1::ResourceRootUnavailable,
            )
            .unwrap();
        let closure = builder.finish().unwrap();
        let json = serde_json::to_string(&closure).unwrap();
        let debug = format!("{closure:?}");
        for rendered in [json, debug] {
            assert!(!rendered.contains("/home/private/secret.png"));
            assert!(rendered.contains("safe.png"));
        }
    }

    #[test]
    fn resource_purpose_is_authoritatively_derived_and_serialized() {
        let cases = [
            (
                SourceResourceKindV1::Buffer,
                DependencyResourcePurposeV1::LoaderEssential,
                "loader_essential",
            ),
            (
                SourceResourceKindV1::Image,
                DependencyResourcePurposeV1::Nonessential,
                "nonessential",
            ),
            (
                SourceResourceKindV1::Texture,
                DependencyResourcePurposeV1::Nonessential,
                "nonessential",
            ),
            (
                SourceResourceKindV1::Video,
                DependencyResourcePurposeV1::TargetOnly,
                "target_only",
            ),
            (
                SourceResourceKindV1::Cache,
                DependencyResourcePurposeV1::TargetOnly,
                "target_only",
            ),
        ];
        let mut builder = DependencyClosureBuilderV1::new(
            InputIdentity::from_bytes(b"primary"),
            SourceSetCoverageV1::complete(),
            cases.len(),
        );
        for (index, (kind, _, _)) in cases.iter().copied().enumerate() {
            assert!(builder.begin_reference(0, 0));
            builder.push_primary(index, kind, index as u64).unwrap();
        }
        let closure = builder.finish().unwrap();
        let wire = serde_json::to_value(&closure).unwrap();
        for (index, (_, purpose, spelling)) in cases.iter().copied().enumerate() {
            assert_eq!(closure.references()[index].purpose(), purpose);
            assert_eq!(wire["references"][index]["purpose"], spelling);
        }

        let mut mutated = closure.references.clone();
        mutated[0].purpose = DependencyResourcePurposeV1::TargetOnly;
        assert_ne!(
            canonical_identity(
                closure.primary_input(),
                &mutated,
                closure.external_resources(),
            ),
            *closure.identity().unwrap()
        );
    }

    #[test]
    fn raw_relative_key_binding_uses_the_source_format_normalization() {
        let primary = InputIdentity::from_bytes(b"primary");
        let raw_locator = SourceResourceLocatorV1::classify("textures/a%20b.png");
        let source_rows = SourceFactSetV1::complete(vec![source_resource(
            0,
            SourceResourceKindV1::Image,
            0,
            raw_locator,
        )]);
        let literal_key = DependencyResourceKeyV1::from_source_str(
            "textures/a%20b.png",
            ResourceKeySyntaxV1::ParserRelativePath,
        )
        .unwrap();
        let mut builder = DependencyClosureBuilderV1::new(
            primary.clone(),
            source_rows.coverage(),
            source_rows.rows().len(),
        );
        assert!(builder.begin_reference(20, 2));
        assert_eq!(
            builder.prepare_external_key(&literal_key).unwrap(),
            Some(true)
        );
        builder.record_external_open_attempt(&literal_key).unwrap();
        assert!(
            builder
                .push_captured_external(
                    0,
                    SourceResourceKindV1::Image,
                    0,
                    literal_key,
                    InputIdentity::from_bytes(b"image"),
                )
                .unwrap()
        );
        let closure = builder.finish().unwrap();
        closure
            .validate_against(SourceFormatV1::Fbx, &primary, &source_rows)
            .unwrap();
        assert_eq!(
            closure.validate_against(SourceFormatV1::GltfJson, &primary, &source_rows),
            Err(DependencyClosureError::ResourceKeyMismatch {
                source_order_index: 0,
            })
        );

        let wrong_rows = SourceFactSetV1::complete(vec![source_resource(
            0,
            SourceResourceKindV1::Image,
            0,
            SourceResourceLocatorV1::classify("textures/b%20b.png"),
        )]);
        assert_eq!(
            closure.validate_against(SourceFormatV1::Fbx, &primary, &wrong_rows),
            Err(DependencyClosureError::ResourceKeyMismatch {
                source_order_index: 0,
            })
        );
    }

    #[test]
    fn binding_rejects_a_wrong_missing_reason_and_wrong_source_coverage_reason() {
        let primary = InputIdentity::from_bytes(b"primary");
        let missing_rows = SourceFactSetV1::complete(vec![source_resource(
            0,
            SourceResourceKindV1::Image,
            0,
            SourceResourceLocatorV1::Missing,
        )]);
        let mut builder = DependencyClosureBuilderV1::new(
            primary.clone(),
            missing_rows.coverage(),
            missing_rows.rows().len(),
        );
        assert!(builder.begin_reference(0, 0));
        builder
            .push_unavailable(
                0,
                SourceResourceKindV1::Image,
                0,
                None,
                DependencyResourceUnavailableReasonV1::Missing,
            )
            .unwrap();
        let closure = builder.finish().unwrap();
        closure
            .validate_against(SourceFormatV1::GltfJson, &primary, &missing_rows)
            .unwrap();
        let mut wrong_reason = closure.clone();
        wrong_reason.references[0].target = DependencyReferenceTargetV1::Unavailable {
            key: None,
            reason: DependencyResourceUnavailableReasonV1::Unreadable,
        };
        assert_eq!(
            wrong_reason.validate_against(SourceFormatV1::GltfJson, &primary, &missing_rows),
            Err(DependencyClosureError::ResourceReferenceMismatch {
                source_order_index: 0,
            })
        );

        let complete_rows = SourceFactSetV1::<SourceResourceReferenceV1>::complete(Vec::new());
        let mut wrong_coverage = DependencyClosureV1::capture_unavailable(
            primary.clone(),
            SourceSetCoverageV1::complete(),
        );
        wrong_coverage.coverage = DependencyClosureCoverageV1::Unavailable {
            reasons: vec![
                DependencyClosureCoverageReasonV1::SourceDeclarationsUnavailable,
                DependencyClosureCoverageReasonV1::CaptureUnavailable,
            ],
        };
        assert_eq!(
            wrong_coverage.validate_against(SourceFormatV1::GltfJson, &primary, &complete_rows,),
            Err(DependencyClosureError::CoverageReasonMismatch)
        );
    }

    #[test]
    fn safe_symlink_refusal_retains_and_validates_only_the_normalized_key() {
        let primary = InputIdentity::from_bytes(b"primary");
        let source_rows = SourceFactSetV1::complete(vec![
            source_resource(
                0,
                SourceResourceKindV1::Image,
                0,
                SourceResourceLocatorV1::classify("textures/a%20b.png"),
            ),
            source_resource(
                1,
                SourceResourceKindV1::Image,
                1,
                SourceResourceLocatorV1::classify("textures/a b.png"),
            ),
        ]);
        let key = DependencyResourceKeyV1::from_source_str(
            "textures/a b.png",
            ResourceKeySyntaxV1::GltfUri,
        )
        .unwrap();
        let mut builder = DependencyClosureBuilderV1::new(
            primary.clone(),
            source_rows.coverage(),
            source_rows.rows().len(),
        );
        for index in 0..2 {
            assert!(builder.begin_reference(20, 2));
            assert_eq!(
                builder.prepare_external_key(&key).unwrap(),
                Some(index == 0)
            );
            builder
                .push_refused(
                    index,
                    SourceResourceKindV1::Image,
                    index as u64,
                    DependencyResourceRefusalReasonV1::Symlink,
                )
                .unwrap();
        }
        let closure = builder.finish().unwrap();
        closure
            .validate_against(SourceFormatV1::GltfJson, &primary, &source_rows)
            .unwrap();
        assert_eq!(closure.work().external_open_attempts(), 0);
        let wire = serde_json::to_value(&closure).unwrap();
        assert_eq!(wire["references"][0]["target"]["key"], "textures/a b.png");
        assert_eq!(wire["references"][1]["target"]["key"], "textures/a b.png");
    }

    #[test]
    fn builder_rejects_multiple_keys_and_cached_outcome_contradictions() {
        let primary = InputIdentity::from_bytes(b"primary");
        let first = DependencyResourceKeyV1::from_source_str(
            "a.bin",
            ResourceKeySyntaxV1::ParserRelativePath,
        )
        .unwrap();
        let second = DependencyResourceKeyV1::from_source_str(
            "b.bin",
            ResourceKeySyntaxV1::ParserRelativePath,
        )
        .unwrap();
        let mut unavailable =
            DependencyClosureBuilderV1::new(primary.clone(), SourceSetCoverageV1::complete(), 2);
        assert!(unavailable.begin_reference(5, 1));
        assert_eq!(
            unavailable.prepare_external_key(&first).unwrap(),
            Some(true)
        );
        assert_eq!(
            unavailable.prepare_external_key(&second),
            Err(DependencyClosureError::ExternalKeyAlreadyPrepared)
        );
        assert_eq!(
            unavailable.record_external_open_attempt(&second),
            Err(DependencyClosureError::ExternalKeyMismatch)
        );
        assert_eq!(
            unavailable.push_unavailable(
                0,
                SourceResourceKindV1::Image,
                0,
                Some(second),
                DependencyResourceUnavailableReasonV1::Missing,
            ),
            Err(DependencyClosureError::ExternalKeyMismatch)
        );
        unavailable
            .push_unavailable(
                0,
                SourceResourceKindV1::Image,
                0,
                Some(first.clone()),
                DependencyResourceUnavailableReasonV1::Missing,
            )
            .unwrap();
        assert!(unavailable.begin_reference(5, 1));
        assert_eq!(
            unavailable.prepare_external_key(&first).unwrap(),
            Some(false)
        );
        assert_eq!(
            unavailable.push_unavailable(
                1,
                SourceResourceKindV1::Image,
                1,
                Some(first.clone()),
                DependencyResourceUnavailableReasonV1::Unreadable,
            ),
            Err(DependencyClosureError::ExternalOutcomeMismatch)
        );
        unavailable
            .push_unavailable(
                1,
                SourceResourceKindV1::Image,
                1,
                Some(first.clone()),
                DependencyResourceUnavailableReasonV1::Missing,
            )
            .unwrap();
        unavailable.finish().unwrap();

        let mut captured =
            DependencyClosureBuilderV1::new(primary, SourceSetCoverageV1::complete(), 2);
        assert!(captured.begin_reference(5, 1));
        assert_eq!(captured.prepare_external_key(&first).unwrap(), Some(true));
        captured.record_external_open_attempt(&first).unwrap();
        assert!(
            captured
                .push_captured_external(
                    0,
                    SourceResourceKindV1::Buffer,
                    0,
                    first.clone(),
                    InputIdentity::from_bytes(b"one"),
                )
                .unwrap()
        );
        assert!(captured.begin_reference(5, 1));
        assert_eq!(captured.prepare_external_key(&first).unwrap(), Some(false));
        assert_eq!(
            captured.push_captured_external(
                1,
                SourceResourceKindV1::Buffer,
                1,
                first.clone(),
                InputIdentity::from_bytes(b"two"),
            ),
            Err(DependencyClosureError::ExternalOutcomeMismatch)
        );
        assert_eq!(
            captured.push_unavailable(
                1,
                SourceResourceKindV1::Buffer,
                1,
                Some(first.clone()),
                DependencyResourceUnavailableReasonV1::Missing,
            ),
            Err(DependencyClosureError::ExternalOutcomeMismatch)
        );
        captured
            .push_external_alias(1, SourceResourceKindV1::Buffer, 1, first)
            .unwrap();
        captured.finish().unwrap();
    }

    #[test]
    fn finish_requires_all_expected_rows_without_a_real_terminal_stop() {
        let builder = DependencyClosureBuilderV1::new(
            InputIdentity::from_bytes(b"primary"),
            SourceSetCoverageV1::complete(),
            1,
        );
        assert_eq!(
            builder.finish(),
            Err(DependencyClosureError::ReferenceCountMismatch {
                expected: 1,
                actual: 0,
            })
        );
    }

    #[test]
    fn terminal_work_counters_retain_each_n_plus_one_witness() {
        let mut path = DependencyClosureBuilderV1::new(
            InputIdentity::from_bytes(b"primary"),
            SourceSetCoverageV1::complete(),
            1,
        );
        assert!(!path.begin_reference(
            DEPENDENCY_CLOSURE_V1_MAX_KEY_BYTES,
            DEPENDENCY_CLOSURE_V1_MAX_PATH_COMPONENTS + 1,
        ));
        assert!(!path.begin_reference(0, 0));
        let path = path.finish().unwrap();
        assert_eq!(path.work().inspected_references(), 1);
        assert_eq!(
            path.work().normalization_bytes_inspected(),
            DEPENDENCY_CLOSURE_V1_MAX_KEY_BYTES
        );
        assert_eq!(
            path.work().path_components_inspected(),
            DEPENDENCY_CLOSURE_V1_MAX_PATH_COMPONENTS + 1
        );
        assert_eq!(path.work().dedup_probes(), 1);

        let mut locator = DependencyClosureBuilderV1::new(
            InputIdentity::from_bytes(b"primary"),
            SourceSetCoverageV1::complete(),
            1,
        );
        assert!(!locator.begin_reference(DEPENDENCY_CLOSURE_V1_MAX_KEY_BYTES + 1, 1));
        let locator = locator.finish().unwrap();
        assert_eq!(
            locator.work().normalization_bytes_inspected(),
            DEPENDENCY_CLOSURE_V1_MAX_KEY_BYTES + 1
        );
        assert_eq!(locator.work().path_components_inspected(), 1);
        assert_eq!(locator.work().dedup_probes(), 1);
    }

    #[test]
    fn unmodeled_domain_prevents_complete_identity() {
        let mut builder = DependencyClosureBuilderV1::new(
            InputIdentity::from_bytes(b"primary"),
            SourceSetCoverageV1::complete(),
            0,
        );
        builder.mark_unmodeled_resource_domain();
        let closure = builder.finish().unwrap();
        assert!(matches!(
            closure.coverage(),
            DependencyClosureCoverageV1::Partial { reasons }
                if reasons == &[DependencyClosureCoverageReasonV1::UnmodeledResourceDomain]
        ));
        assert!(closure.identity().is_none());
    }

    #[test]
    fn equal_content_at_distinct_keys_remains_two_resources() {
        let mut builder = DependencyClosureBuilderV1::new(
            InputIdentity::from_bytes(b"primary"),
            SourceSetCoverageV1::complete(),
            2,
        );
        for (index, name) in ["a.bin", "b.bin"].into_iter().enumerate() {
            let key = DependencyResourceKeyV1::from_source_str(
                name,
                ResourceKeySyntaxV1::ParserRelativePath,
            )
            .unwrap();
            assert!(builder.begin_reference(name.len(), 1));
            assert_eq!(builder.prepare_external_key(&key).unwrap(), Some(true));
            builder.record_external_open_attempt(&key).unwrap();
            assert!(
                builder
                    .push_captured_external(
                        index,
                        SourceResourceKindV1::Buffer,
                        index as u64,
                        key,
                        InputIdentity::from_bytes(b"same"),
                    )
                    .unwrap()
            );
        }
        let closure = builder.finish().unwrap();
        assert!(closure.coverage().is_complete());
        assert_eq!(closure.external_resources().len(), 2);
        assert_eq!(closure.work().external_open_attempts(), 2);
        assert_eq!(
            closure
                .external_resources()
                .iter()
                .map(|resource| resource.identity())
                .collect::<Vec<_>>(),
            vec![
                &InputIdentity::from_bytes(b"same"),
                &InputIdentity::from_bytes(b"same"),
            ]
        );
    }
}
