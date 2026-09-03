//! Bounded engine-neutral evidence for raw transform-path addressability.
//!
//! Format loaders project original node identities, parent chains, names, and
//! synthetic-helper classification into this contract before normalized
//! skeleton names can erase the distinction. The closed V1 grammar has no
//! escaping: a source segment which cannot be represented remains visible as
//! bounded row evidence and makes absence unprovable.

use crate::bounded_deserialize::{
    CappedSequence, deserialize_capped_option_string, deserialize_capped_sequence,
    deserialize_capped_string,
};
use crate::{InputIdentity, SourceFormatV1};
use serde::de::{Error as _, IgnoredAny, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Immutable raw transform-path inventory contract identity.
pub const RAW_TRANSFORM_PATH_INVENTORY_V1_ID: &str = "urn:animsmith:raw-transform-path-inventory:1";
/// Maximum source-node rows retained by one inventory.
pub const RAW_TRANSFORM_PATH_INVENTORY_V1_MAX_ROWS: usize = 4_096;
/// Maximum UTF-8 bytes in one addressable path segment.
pub const RAW_TRANSFORM_PATH_V1_MAX_SEGMENT_BYTES: usize = 1_024;
/// Maximum UTF-8 bytes in one complete addressable path.
pub const RAW_TRANSFORM_PATH_V1_MAX_PATH_BYTES: usize = 4_096;
/// Maximum segments in one addressable path and parents in one retained chain.
pub const RAW_TRANSFORM_PATH_V1_MAX_DEPTH: usize = 256;
/// Maximum parent-index references retained across one inventory.
pub const RAW_TRANSFORM_PATH_INVENTORY_V1_MAX_PARENT_REFERENCES: usize = 65_536;
/// Maximum aggregate UTF-8 bytes retained across names and materialized paths.
pub const RAW_TRANSFORM_PATH_INVENTORY_V1_MAX_TEXT_BYTES: usize = 1024 * 1024;

/// One validated path in the closed, unescaped V1 grammar.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct RawTransformPathV1(String);

impl RawTransformPathV1 {
    /// Parse and validate one configured transform path.
    ///
    /// Matching never trims, normalizes Unicode, strips namespaces, or changes
    /// case. `/` is the only separator and has no escape syntax.
    ///
    /// # Errors
    ///
    /// Returns a typed syntax error for an empty path, an empty or reserved
    /// segment, a forbidden character, or a V1 length/depth overflow.
    pub fn parse(value: &str) -> Result<Self, RawTransformPathSyntaxErrorV1> {
        validate_path(value)?;
        Ok(Self(value.to_owned()))
    }

    /// Exact UTF-8 path spelling.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Exact case-sensitive segment sequence.
    pub fn segments(&self) -> impl Iterator<Item = &str> {
        self.0.split('/')
    }
}

impl<'de> Deserialize<'de> for RawTransformPathV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = deserialize_capped_string(deserializer, RAW_TRANSFORM_PATH_V1_MAX_PATH_BYTES)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}

/// Why configured path syntax is invalid under the closed V1 grammar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum RawTransformPathSyntaxErrorV1 {
    /// The aggregate configured path is empty.
    #[error("transform path must contain at least one nonempty segment")]
    EmptyPath,
    /// The path starts or ends with `/`, or contains `//`.
    #[error("transform path contains an empty segment")]
    EmptySegment,
    /// A segment is the reserved single-dot spelling.
    #[error("transform path contains reserved '.' segment")]
    DotSegment,
    /// A segment is the reserved double-dot spelling.
    #[error("transform path contains reserved '..' segment")]
    DotDotSegment,
    /// Backslash has no escaping or separator meaning in V1.
    #[error("transform path contains forbidden backslash")]
    Backslash,
    /// A literal slash cannot occur inside one source segment.
    #[error("transform path source segment contains forbidden slash")]
    Slash,
    /// A Unicode control character is forbidden.
    #[error("transform path contains a control character")]
    ControlCharacter,
    /// A Unicode format character is forbidden.
    #[error("transform path contains a format character")]
    FormatCharacter,
    /// One segment exceeded its independent UTF-8 byte bound.
    #[error("transform path segment exceeds the V1 byte bound")]
    SegmentTooLong,
    /// The aggregate path exceeded its independent UTF-8 byte bound.
    #[error("transform path exceeds the V1 byte bound")]
    PathTooLong,
    /// The segment sequence exceeded the V1 depth bound.
    #[error("transform path exceeds the V1 segment bound")]
    TooManySegments,
}

fn validate_path(value: &str) -> Result<(), RawTransformPathSyntaxErrorV1> {
    if value.is_empty() {
        return Err(RawTransformPathSyntaxErrorV1::EmptyPath);
    }
    if value.len() > RAW_TRANSFORM_PATH_V1_MAX_PATH_BYTES {
        return Err(RawTransformPathSyntaxErrorV1::PathTooLong);
    }
    let mut count = 0usize;
    for segment in value.split('/') {
        count = count.saturating_add(1);
        if count > RAW_TRANSFORM_PATH_V1_MAX_DEPTH {
            return Err(RawTransformPathSyntaxErrorV1::TooManySegments);
        }
        validate_segment(segment)?;
    }
    Ok(())
}

fn validate_segment(segment: &str) -> Result<(), RawTransformPathSyntaxErrorV1> {
    if segment.is_empty() {
        return Err(RawTransformPathSyntaxErrorV1::EmptySegment);
    }
    if segment == "." {
        return Err(RawTransformPathSyntaxErrorV1::DotSegment);
    }
    if segment == ".." {
        return Err(RawTransformPathSyntaxErrorV1::DotDotSegment);
    }
    if segment.len() > RAW_TRANSFORM_PATH_V1_MAX_SEGMENT_BYTES {
        return Err(RawTransformPathSyntaxErrorV1::SegmentTooLong);
    }
    for character in segment.chars() {
        if character == '/' {
            return Err(RawTransformPathSyntaxErrorV1::Slash);
        }
        if character == '\\' {
            return Err(RawTransformPathSyntaxErrorV1::Backslash);
        }
        if character.is_control() {
            return Err(RawTransformPathSyntaxErrorV1::ControlCharacter);
        }
        if is_unicode_format_character(character) {
            return Err(RawTransformPathSyntaxErrorV1::FormatCharacter);
        }
    }
    Ok(())
}

// Unicode General_Category=Cf ranges from the Unicode scalar repertoire.
// `char::is_control()` separately covers General_Category=Cc.
fn is_unicode_format_character(character: char) -> bool {
    matches!(
        character as u32,
        0x00ad
            | 0x061c
            | 0x06dd
            | 0x070f
            | 0x0890..=0x0891
            | 0x08e2
            | 0x180e
            | 0x200b..=0x200f
            | 0x202a..=0x202e
            | 0x2060..=0x2064
            | 0x2066..=0x206f
            | 0xfeff
            | 0xfff9..=0xfffb
            | 0x0600..=0x0605
            | 0x110bd
            | 0x110cd
            | 0x13430..=0x1343f
            | 0x1bca0..=0x1bca3
            | 0x1d173..=0x1d17a
            | 0xe0001
            | 0xe0020..=0xe007f
    )
}

/// Original or synthetic role of one loader node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RawTransformPathNodeKindV1 {
    /// Original source transform node eligible for exact matching.
    Source,
    /// ufbx's implicit scene root, never a configured path segment.
    ImplicitUfbxRoot,
    /// Synthetic helper generated for an FBX geometric transform.
    GeometryTransformHelper,
    /// Synthetic helper generated for scale compensation.
    ScaleCompensationHelper,
    /// A conservatively classified helper carrying both ufbx flags.
    GeometryAndScaleHelper,
}

impl RawTransformPathNodeKindV1 {
    /// Whether this row can satisfy a configured motion path.
    pub const fn is_matchable(self) -> bool {
        matches!(self, Self::Source)
    }
}

/// Addressability state of one retained source row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RawTransformPathRowAddressabilityV1 {
    /// The exact path is retained and can match.
    Addressable,
    /// The implicit root is retained but excluded from path segments.
    ExcludedImplicitRoot,
    /// A synthetic helper is retained but excluded from path segments and matching.
    ExcludedSyntheticHelper,
    /// This source segment violates the closed grammar.
    UnrepresentableSourceSegment,
    /// An earlier source segment on this path violates the closed grammar.
    UnrepresentableAncestorSegment,
    /// The complete source path exceeded the independent aggregate byte bound.
    PathTooLong,
}

/// Coverage of the raw transform inventory and its exact-addressability proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "reason", rename_all = "snake_case")]
pub enum RawTransformPathCoverageV1 {
    /// Every source row and addressability fact is retained.
    Complete,
    /// A canonical source prefix or all rows with incomplete addressability are retained.
    Partial(RawTransformPathCoverageReasonV1),
    /// No source rows are available.
    Unavailable(RawTransformPathCoverageReasonV1),
}

impl RawTransformPathCoverageV1 {
    /// Whether a zero-match search proves absence.
    pub const fn proves_absence(self) -> bool {
        matches!(self, Self::Complete)
    }
}

/// Why raw transform-path coverage is incomplete.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RawTransformPathCoverageReasonV1 {
    /// At least one original source segment cannot be expressed in V1 grammar.
    UnrepresentableSourceSegment,
    /// Row, parent-chain, text, or path-depth projection work exceeded a V1 bound.
    ProjectionBudgetExceeded,
    /// The format loader could not expose raw transform nodes.
    LoaderEvidenceUnavailable,
}

/// Loader input for one source-order node.
#[derive(Debug, Clone, Copy)]
pub struct RawTransformPathNodeInputV1<'a> {
    /// Stable inventory node identity.
    ///
    /// Original and implicit-root rows use the raw-preserving loader index;
    /// synthetic helpers occupy a disjoint appended range.
    pub source_node_index: u64,
    /// Direct parent identity in the same inventory domain.
    pub parent_source_node_index: Option<u64>,
    /// Exact parser-projected UTF-8 source name; the implicit root uses `None`.
    pub source_name: Option<&'a str>,
    /// Same-load normalized document bone index for this node.
    ///
    /// An original node which could not be correlated by immutable parser
    /// element identity uses `None`.
    pub projected_bone_index: Option<u64>,
    /// Original or synthetic node role.
    pub kind: RawTransformPathNodeKindV1,
}

/// One bounded raw transform-path inventory row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RawTransformPathNodeRowV1 {
    source_node_index: u64,
    parent_source_node_index: Option<u64>,
    #[serde(deserialize_with = "deserialize_parent_chain")]
    parent_chain: Vec<u64>,
    source_name: Option<String>,
    source_name_utf8_bytes: u64,
    projected_bone_index: Option<u64>,
    kind: RawTransformPathNodeKindV1,
    addressability: RawTransformPathRowAddressabilityV1,
    addressable_path: Option<RawTransformPathV1>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTransformPathNodeRowWireV1 {
    source_node_index: u64,
    parent_source_node_index: Option<u64>,
    #[serde(deserialize_with = "deserialize_parent_chain")]
    parent_chain: Vec<u64>,
    #[serde(deserialize_with = "deserialize_source_name")]
    source_name: Option<String>,
    source_name_utf8_bytes: u64,
    projected_bone_index: Option<u64>,
    kind: RawTransformPathNodeKindV1,
    addressability: RawTransformPathRowAddressabilityV1,
    addressable_path: Option<RawTransformPathV1>,
}

impl<'de> Deserialize<'de> for RawTransformPathNodeRowV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = RawTransformPathNodeRowWireV1::deserialize(deserializer)?;
        Ok(Self {
            source_node_index: wire.source_node_index,
            parent_source_node_index: wire.parent_source_node_index,
            parent_chain: wire.parent_chain,
            source_name: wire.source_name,
            source_name_utf8_bytes: wire.source_name_utf8_bytes,
            projected_bone_index: wire.projected_bone_index,
            kind: wire.kind,
            addressability: wire.addressability,
            addressable_path: wire.addressable_path,
        })
    }
}

fn deserialize_source_name<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_capped_option_string(deserializer, RAW_TRANSFORM_PATH_V1_MAX_SEGMENT_BYTES)
}

impl RawTransformPathNodeRowV1 {
    /// Stable raw-original or appended synthetic inventory identity.
    pub const fn source_node_index(&self) -> u64 {
        self.source_node_index
    }
    /// Direct parent identity in the same inventory domain.
    pub const fn parent_source_node_index(&self) -> Option<u64> {
        self.parent_source_node_index
    }
    /// Root-to-direct-parent chain, including excluded implicit/helper rows.
    pub fn parent_chain(&self) -> &[u64] {
        &self.parent_chain
    }
    /// Exact retained source name, or `None` when absent/redacted by a bound.
    pub fn source_name(&self) -> Option<&str> {
        self.source_name.as_deref()
    }
    /// Original UTF-8 byte length even when the spelling could not be retained.
    pub const fn source_name_utf8_bytes(&self) -> u64 {
        self.source_name_utf8_bytes
    }
    /// Same-load normalized document bone identity, when correlated exactly.
    pub const fn projected_bone_index(&self) -> Option<u64> {
        self.projected_bone_index
    }
    /// Original or synthetic node role.
    pub const fn kind(&self) -> RawTransformPathNodeKindV1 {
        self.kind
    }
    /// Exact-addressability state.
    pub const fn addressability(&self) -> RawTransformPathRowAddressabilityV1 {
        self.addressability
    }
    /// Exact path for a matchable original source node.
    pub const fn addressable_path(&self) -> Option<&RawTransformPathV1> {
        self.addressable_path.as_ref()
    }
}

/// Invalid source-order data supplied by a format projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum RawTransformPathInventoryErrorV1 {
    /// The immutable semantic schema identity changed.
    #[error("raw transform inventory schema identity is invalid")]
    InvalidSchema,
    /// V1 is currently a raw FBX transform-path contract.
    #[error("raw transform inventory source format is not FBX")]
    UnsupportedSourceFormat,
    /// More rows than the V1 source-node ceiling were decoded.
    #[error("raw transform inventory exceeds its row bound")]
    TooManyRows,
    /// Aggregate parent-chain evidence exceeded the V1 ceiling.
    #[error("raw transform inventory exceeds its parent-reference bound")]
    TooManyParentReferences,
    /// Aggregate retained source/path text exceeded the V1 ceiling.
    #[error("raw transform inventory exceeds its text bound")]
    TooMuchText,
    /// Source node indices must be zero-based and contiguous.
    #[error("raw transform nodes are not in canonical source-index order")]
    NonCanonicalSourceNodeIndex,
    /// Parent identity must reference an earlier retained source node.
    #[error("raw transform node parent does not reference an earlier node")]
    InvalidParent,
    /// Exactly one first row must identify the implicit ufbx root.
    #[error("raw transform inventory has an invalid implicit-root classification")]
    InvalidImplicitRoot,
    /// Retained row spelling, path, or addressability disagrees with its ancestry.
    #[error("raw transform inventory row addressability is not canonical")]
    InvalidAddressability,
    /// A projected bone identity is outside the same-load normalized document.
    #[error("raw transform inventory projected bone index is out of range")]
    ProjectedBoneOutOfRange,
    /// Two original source nodes cannot claim the same normalized bone identity.
    #[error("raw transform inventory projected bone index is duplicated")]
    DuplicateProjectedBone,
    /// Coverage contradicts the retained source/addressability rows.
    #[error("raw transform inventory coverage contradicts its rows")]
    InvalidCoverage,
}

/// Bounded same-load raw transform-path inventory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RawTransformPathInventoryV1 {
    schema: String,
    primary_input: InputIdentity,
    source_format: SourceFormatV1,
    projected_bone_count: u64,
    coverage: RawTransformPathCoverageV1,
    rows: Vec<RawTransformPathNodeRowV1>,
}

impl RawTransformPathInventoryV1 {
    /// Project canonical source-order nodes into bounded path evidence.
    ///
    /// Synthetic helpers and the implicit ufbx root remain rows and parent-chain
    /// identities, but do not contribute path segments and cannot match.
    ///
    /// # Errors
    ///
    /// Returns an error when source indices, parents, or implicit-root
    /// classification violate the loader-order contract.
    pub fn from_nodes<'a>(
        primary_input: InputIdentity,
        source_format: SourceFormatV1,
        projected_bone_count: u64,
        nodes: impl IntoIterator<Item = RawTransformPathNodeInputV1<'a>>,
    ) -> Result<Self, RawTransformPathInventoryErrorV1> {
        let mut rows: Vec<RawTransformPathNodeRowV1> = Vec::new();
        let mut effective_paths: Vec<Option<String>> = Vec::new();
        let mut coverage = RawTransformPathCoverageV1::Complete;
        let mut parent_references = 0usize;
        let mut text_bytes = 0usize;

        for input in nodes {
            let expected = rows.len() as u64;
            if input.source_node_index != expected {
                return Err(RawTransformPathInventoryErrorV1::NonCanonicalSourceNodeIndex);
            }
            if rows.len() == RAW_TRANSFORM_PATH_INVENTORY_V1_MAX_ROWS {
                coverage = RawTransformPathCoverageV1::Partial(
                    RawTransformPathCoverageReasonV1::ProjectionBudgetExceeded,
                );
                break;
            }
            if expected == 0 {
                if input.parent_source_node_index.is_some()
                    || input.kind != RawTransformPathNodeKindV1::ImplicitUfbxRoot
                {
                    return Err(RawTransformPathInventoryErrorV1::InvalidImplicitRoot);
                }
            } else if input.kind == RawTransformPathNodeKindV1::ImplicitUfbxRoot {
                return Err(RawTransformPathInventoryErrorV1::InvalidImplicitRoot);
            }
            let parent = match input.parent_source_node_index {
                Some(parent) if parent < expected => Some(parent as usize),
                Some(_) => return Err(RawTransformPathInventoryErrorV1::InvalidParent),
                None => None,
            };
            let mut parent_chain = parent.map_or_else(Vec::new, |parent| {
                let mut chain = rows[parent].parent_chain.clone();
                chain.push(parent as u64);
                chain
            });
            if parent_chain.len() > RAW_TRANSFORM_PATH_V1_MAX_DEPTH
                || parent_references.saturating_add(parent_chain.len())
                    > RAW_TRANSFORM_PATH_INVENTORY_V1_MAX_PARENT_REFERENCES
            {
                coverage = RawTransformPathCoverageV1::Partial(
                    RawTransformPathCoverageReasonV1::ProjectionBudgetExceeded,
                );
                break;
            }
            parent_references += parent_chain.len();

            let source_name_bytes = input.source_name.map_or(0, str::len);
            let retained_name = input
                .source_name
                .filter(|name| name.len() <= RAW_TRANSFORM_PATH_V1_MAX_SEGMENT_BYTES)
                .map(str::to_owned);
            let inherited = parent
                .and_then(|parent| effective_paths[parent].as_deref())
                .unwrap_or("");
            let (addressability, effective_path, addressable_path) = match input.kind {
                RawTransformPathNodeKindV1::ImplicitUfbxRoot => (
                    RawTransformPathRowAddressabilityV1::ExcludedImplicitRoot,
                    Some(String::new()),
                    None,
                ),
                RawTransformPathNodeKindV1::GeometryTransformHelper
                | RawTransformPathNodeKindV1::ScaleCompensationHelper
                | RawTransformPathNodeKindV1::GeometryAndScaleHelper => (
                    RawTransformPathRowAddressabilityV1::ExcludedSyntheticHelper,
                    parent.and_then(|parent| effective_paths[parent].clone()),
                    None,
                ),
                RawTransformPathNodeKindV1::Source => match input.source_name {
                    Some(name) if validate_segment(name).is_ok() => {
                        if parent.is_some() && effective_paths[parent.unwrap()].is_none() {
                            coverage = incomplete_addressability(coverage);
                            (
                                RawTransformPathRowAddressabilityV1::UnrepresentableAncestorSegment,
                                None,
                                None,
                            )
                        } else {
                            let path = if inherited.is_empty() {
                                name.to_owned()
                            } else {
                                format!("{inherited}/{name}")
                            };
                            if path.len() > RAW_TRANSFORM_PATH_V1_MAX_PATH_BYTES
                                || path.split('/').count() > RAW_TRANSFORM_PATH_V1_MAX_DEPTH
                            {
                                coverage = incomplete_addressability(coverage);
                                (RawTransformPathRowAddressabilityV1::PathTooLong, None, None)
                            } else {
                                let parsed = RawTransformPathV1(path.clone());
                                (
                                    RawTransformPathRowAddressabilityV1::Addressable,
                                    Some(path),
                                    Some(parsed),
                                )
                            }
                        }
                    }
                    _ => {
                        coverage = incomplete_addressability(coverage);
                        (
                            RawTransformPathRowAddressabilityV1::UnrepresentableSourceSegment,
                            None,
                            None,
                        )
                    }
                },
            };

            let row_text = retained_name.as_ref().map_or(0, String::len)
                + addressable_path
                    .as_ref()
                    .map_or(0, |path| path.as_str().len());
            if text_bytes.saturating_add(row_text) > RAW_TRANSFORM_PATH_INVENTORY_V1_MAX_TEXT_BYTES
            {
                coverage = RawTransformPathCoverageV1::Partial(
                    RawTransformPathCoverageReasonV1::ProjectionBudgetExceeded,
                );
                break;
            }
            text_bytes += row_text;
            rows.push(RawTransformPathNodeRowV1 {
                source_node_index: input.source_node_index,
                parent_source_node_index: input.parent_source_node_index,
                parent_chain: std::mem::take(&mut parent_chain),
                source_name: retained_name,
                source_name_utf8_bytes: source_name_bytes as u64,
                projected_bone_index: input.projected_bone_index,
                kind: input.kind,
                addressability,
                addressable_path,
            });
            effective_paths.push(effective_path);
        }
        let value = Self {
            schema: RAW_TRANSFORM_PATH_INVENTORY_V1_ID.to_owned(),
            primary_input,
            source_format,
            projected_bone_count,
            coverage,
            rows,
        };
        value.validate()?;
        Ok(value)
    }

    /// Construct a typed empty unavailable inventory.
    pub fn unavailable(
        primary_input: InputIdentity,
        source_format: SourceFormatV1,
        projected_bone_count: u64,
    ) -> Self {
        Self {
            schema: RAW_TRANSFORM_PATH_INVENTORY_V1_ID.to_owned(),
            primary_input,
            source_format,
            projected_bone_count,
            coverage: RawTransformPathCoverageV1::Unavailable(
                RawTransformPathCoverageReasonV1::LoaderEvidenceUnavailable,
            ),
            rows: Vec::new(),
        }
    }

    /// Semantic inventory identifier.
    pub fn contract_id(&self) -> &str {
        &self.schema
    }
    /// Exact primary input identity for this loader pass.
    pub const fn primary_input(&self) -> &InputIdentity {
        &self.primary_input
    }
    /// Exact source format.
    pub const fn source_format(&self) -> SourceFormatV1 {
        self.source_format
    }
    /// Same-load normalized document bone count used to bound correlations.
    pub const fn projected_bone_count(&self) -> u64 {
        self.projected_bone_count
    }
    /// Coverage governing whether a zero match proves absence.
    pub const fn coverage(&self) -> RawTransformPathCoverageV1 {
        self.coverage
    }
    /// Canonical retained source-order rows.
    pub fn rows(&self) -> &[RawTransformPathNodeRowV1] {
        &self.rows
    }

    /// Aggregate UTF-8 bytes retained across source names and exact paths.
    pub fn retained_text_bytes(&self) -> Result<usize, RawTransformPathInventoryErrorV1> {
        self.rows
            .iter()
            .try_fold(0usize, |total, row| {
                total.checked_add(
                    row.source_name.as_ref().map_or(0, String::len)
                        + row
                            .addressable_path
                            .as_ref()
                            .map_or(0, |path| path.as_str().len()),
                )
            })
            .ok_or(RawTransformPathInventoryErrorV1::TooMuchText)
    }

    /// Re-check schema, format, bounds, source order, ancestry, path grammar,
    /// addressability, coverage, and same-load bone correlations.
    ///
    /// # Errors
    ///
    /// Returns a typed contract error for any forged or noncanonical state.
    pub fn validate(&self) -> Result<(), RawTransformPathInventoryErrorV1> {
        if self.schema != RAW_TRANSFORM_PATH_INVENTORY_V1_ID {
            return Err(RawTransformPathInventoryErrorV1::InvalidSchema);
        }
        if self.source_format != SourceFormatV1::Fbx {
            return Err(RawTransformPathInventoryErrorV1::UnsupportedSourceFormat);
        }
        if self.rows.len() > RAW_TRANSFORM_PATH_INVENTORY_V1_MAX_ROWS {
            return Err(RawTransformPathInventoryErrorV1::TooManyRows);
        }
        let parent_references = self
            .rows
            .iter()
            .try_fold(0usize, |total, row| {
                total.checked_add(row.parent_chain.len())
            })
            .ok_or(RawTransformPathInventoryErrorV1::TooManyParentReferences)?;
        if parent_references > RAW_TRANSFORM_PATH_INVENTORY_V1_MAX_PARENT_REFERENCES {
            return Err(RawTransformPathInventoryErrorV1::TooManyParentReferences);
        }
        if self.retained_text_bytes()? > RAW_TRANSFORM_PATH_INVENTORY_V1_MAX_TEXT_BYTES {
            return Err(RawTransformPathInventoryErrorV1::TooMuchText);
        }
        if matches!(
            self.coverage,
            RawTransformPathCoverageV1::Unavailable(
                RawTransformPathCoverageReasonV1::LoaderEvidenceUnavailable
            )
        ) {
            return if self.rows.is_empty() {
                Ok(())
            } else {
                Err(RawTransformPathInventoryErrorV1::InvalidCoverage)
            };
        }
        if matches!(self.coverage, RawTransformPathCoverageV1::Unavailable(_)) {
            return Err(RawTransformPathInventoryErrorV1::InvalidCoverage);
        }
        if self.rows.is_empty() {
            return Err(RawTransformPathInventoryErrorV1::InvalidImplicitRoot);
        }

        let mut effective_paths: Vec<Option<String>> = Vec::with_capacity(self.rows.len());
        let mut saw_unrepresentable = false;
        let mut projected = std::collections::BTreeSet::new();
        for (expected, row) in self.rows.iter().enumerate() {
            if row.source_node_index != expected as u64 {
                return Err(RawTransformPathInventoryErrorV1::NonCanonicalSourceNodeIndex);
            }
            let parent = match row.parent_source_node_index {
                Some(parent) if parent < expected as u64 => Some(parent as usize),
                Some(_) => return Err(RawTransformPathInventoryErrorV1::InvalidParent),
                None => None,
            };
            let expected_chain = parent.map_or_else(Vec::new, |parent| {
                let mut chain = self.rows[parent].parent_chain.clone();
                chain.push(parent as u64);
                chain
            });
            if row.parent_chain != expected_chain
                || row.parent_chain.len() > RAW_TRANSFORM_PATH_V1_MAX_DEPTH
            {
                return Err(RawTransformPathInventoryErrorV1::InvalidParent);
            }
            if expected == 0 {
                if parent.is_some()
                    || row.kind != RawTransformPathNodeKindV1::ImplicitUfbxRoot
                    || row.source_name.is_some()
                    || row.source_name_utf8_bytes != 0
                    || row.projected_bone_index.is_some()
                {
                    return Err(RawTransformPathInventoryErrorV1::InvalidImplicitRoot);
                }
            } else if row.kind == RawTransformPathNodeKindV1::ImplicitUfbxRoot {
                return Err(RawTransformPathInventoryErrorV1::InvalidImplicitRoot);
            }
            if let Some(name) = &row.source_name {
                if name.len() as u64 != row.source_name_utf8_bytes
                    || name.len() > RAW_TRANSFORM_PATH_V1_MAX_SEGMENT_BYTES
                {
                    return Err(RawTransformPathInventoryErrorV1::InvalidAddressability);
                }
            } else if row.kind.is_matchable()
                && row.source_name_utf8_bytes <= RAW_TRANSFORM_PATH_V1_MAX_SEGMENT_BYTES as u64
            {
                // Matchable empty names are retained as Some(""); None means a
                // spelling was redacted only because it exceeded the segment cap.
                return Err(RawTransformPathInventoryErrorV1::InvalidAddressability);
            }
            match row.projected_bone_index {
                Some(index) if index >= self.projected_bone_count => {
                    return Err(RawTransformPathInventoryErrorV1::ProjectedBoneOutOfRange);
                }
                Some(index) if !projected.insert(index) => {
                    return Err(RawTransformPathInventoryErrorV1::DuplicateProjectedBone);
                }
                _ => {}
            }

            let inherited = parent
                .and_then(|parent| effective_paths[parent].as_deref())
                .unwrap_or("");
            let (expected_addressability, effective_path, expected_path) = match row.kind {
                RawTransformPathNodeKindV1::ImplicitUfbxRoot => (
                    RawTransformPathRowAddressabilityV1::ExcludedImplicitRoot,
                    Some(String::new()),
                    None,
                ),
                RawTransformPathNodeKindV1::GeometryTransformHelper
                | RawTransformPathNodeKindV1::ScaleCompensationHelper
                | RawTransformPathNodeKindV1::GeometryAndScaleHelper => (
                    RawTransformPathRowAddressabilityV1::ExcludedSyntheticHelper,
                    parent.and_then(|parent| effective_paths[parent].clone()),
                    None,
                ),
                RawTransformPathNodeKindV1::Source => match row.source_name.as_deref() {
                    Some(name) if validate_segment(name).is_ok() => {
                        if parent.is_some() && effective_paths[parent.unwrap()].is_none() {
                            saw_unrepresentable = true;
                            (
                                RawTransformPathRowAddressabilityV1::UnrepresentableAncestorSegment,
                                None,
                                None,
                            )
                        } else {
                            let path = if inherited.is_empty() {
                                name.to_owned()
                            } else {
                                format!("{inherited}/{name}")
                            };
                            if path.len() > RAW_TRANSFORM_PATH_V1_MAX_PATH_BYTES
                                || path.split('/').count() > RAW_TRANSFORM_PATH_V1_MAX_DEPTH
                            {
                                saw_unrepresentable = true;
                                (RawTransformPathRowAddressabilityV1::PathTooLong, None, None)
                            } else {
                                (
                                    RawTransformPathRowAddressabilityV1::Addressable,
                                    Some(path.clone()),
                                    Some(path),
                                )
                            }
                        }
                    }
                    _ => {
                        saw_unrepresentable = true;
                        (
                            RawTransformPathRowAddressabilityV1::UnrepresentableSourceSegment,
                            None,
                            None,
                        )
                    }
                },
            };
            if row.addressability != expected_addressability
                || row
                    .addressable_path
                    .as_ref()
                    .map(RawTransformPathV1::as_str)
                    != expected_path.as_deref()
            {
                return Err(RawTransformPathInventoryErrorV1::InvalidAddressability);
            }
            effective_paths.push(effective_path);
        }
        match self.coverage {
            RawTransformPathCoverageV1::Complete if saw_unrepresentable => {
                Err(RawTransformPathInventoryErrorV1::InvalidCoverage)
            }
            RawTransformPathCoverageV1::Partial(
                RawTransformPathCoverageReasonV1::UnrepresentableSourceSegment,
            ) if !saw_unrepresentable => Err(RawTransformPathInventoryErrorV1::InvalidCoverage),
            RawTransformPathCoverageV1::Partial(
                RawTransformPathCoverageReasonV1::LoaderEvidenceUnavailable,
            ) => Err(RawTransformPathInventoryErrorV1::InvalidCoverage),
            RawTransformPathCoverageV1::Unavailable(_) => unreachable!(),
            _ => Ok(()),
        }
    }

    /// Resolve one already-validated configured path byte-exactly.
    pub fn resolve(&self, path: &RawTransformPathV1) -> RawTransformPathResolutionV1 {
        let mut matches = self
            .rows
            .iter()
            .filter(|row| {
                row.kind.is_matchable()
                    && row
                        .addressable_path
                        .as_ref()
                        .map(RawTransformPathV1::as_str)
                        == Some(path.as_str())
            })
            .map(|row| RawTransformPathMatchV1 {
                source_node_index: row.source_node_index,
                projected_bone_index: row.projected_bone_index,
                parent_chain: row.parent_chain.clone(),
                path: path.clone(),
            });
        let Some(first) = matches.next() else {
            return if self.coverage.proves_absence() {
                RawTransformPathResolutionV1::NoMatch
            } else {
                RawTransformPathResolutionV1::CoverageIncomplete {
                    coverage: self.coverage,
                }
            };
        };
        let Some(second) = matches.next() else {
            return RawTransformPathResolutionV1::Exact(first);
        };
        let mut all = vec![first, second];
        all.extend(matches);
        RawTransformPathResolutionV1::Ambiguous { matches: all }
    }
}

fn incomplete_addressability(coverage: RawTransformPathCoverageV1) -> RawTransformPathCoverageV1 {
    match coverage {
        RawTransformPathCoverageV1::Complete => RawTransformPathCoverageV1::Partial(
            RawTransformPathCoverageReasonV1::UnrepresentableSourceSegment,
        ),
        other => other,
    }
}

/// One exact original-node match, suitable for rig-role identity comparison.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawTransformPathMatchV1 {
    source_node_index: u64,
    projected_bone_index: Option<u64>,
    #[serde(deserialize_with = "deserialize_parent_chain")]
    parent_chain: Vec<u64>,
    path: RawTransformPathV1,
}

fn deserialize_parent_chain<'de, D>(deserializer: D) -> Result<Vec<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let values: CappedSequence<u64> =
        deserialize_capped_sequence(deserializer, RAW_TRANSFORM_PATH_V1_MAX_DEPTH)?;
    if values.overflowed {
        return Err(serde::de::Error::custom(
            "raw transform parent chain exceeded the V1 depth bound",
        ));
    }
    Ok(values.values)
}

fn deserialize_rows<'de, D>(deserializer: D) -> Result<Vec<RawTransformPathNodeRowV1>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct RowsVisitor;

    impl<'de> Visitor<'de> for RowsVisitor {
        type Value = Vec<RawTransformPathNodeRowV1>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a bounded raw transform-path inventory row sequence")
        }

        fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut rows = Vec::with_capacity(
                sequence
                    .size_hint()
                    .unwrap_or(0)
                    .min(RAW_TRANSFORM_PATH_INVENTORY_V1_MAX_ROWS),
            );
            let mut parent_references = 0usize;
            let mut text_bytes = 0usize;
            while rows.len() < RAW_TRANSFORM_PATH_INVENTORY_V1_MAX_ROWS {
                let Some(row) = sequence.next_element::<RawTransformPathNodeRowV1>()? else {
                    return Ok(rows);
                };
                parent_references = parent_references
                    .checked_add(row.parent_chain.len())
                    .ok_or_else(|| {
                        A::Error::custom(
                            "raw transform inventory parent-reference count overflowed",
                        )
                    })?;
                if parent_references > RAW_TRANSFORM_PATH_INVENTORY_V1_MAX_PARENT_REFERENCES {
                    return Err(A::Error::custom(
                        "raw transform inventory exceeded its parent-reference bound",
                    ));
                }
                let retained_text = row.source_name.as_ref().map_or(0, String::len)
                    + row
                        .addressable_path
                        .as_ref()
                        .map_or(0, |path| path.as_str().len());
                text_bytes = text_bytes.checked_add(retained_text).ok_or_else(|| {
                    A::Error::custom("raw transform inventory retained-text count overflowed")
                })?;
                if text_bytes > RAW_TRANSFORM_PATH_INVENTORY_V1_MAX_TEXT_BYTES {
                    return Err(A::Error::custom(
                        "raw transform inventory exceeded its text bound",
                    ));
                }
                rows.push(row);
            }
            if sequence.next_element::<IgnoredAny>()?.is_some() {
                return Err(A::Error::custom(
                    "raw transform inventory exceeded the V1 row bound",
                ));
            }
            Ok(rows)
        }
    }

    deserializer.deserialize_seq(RowsVisitor)
}

fn deserialize_schema<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_capped_string(deserializer, RAW_TRANSFORM_PATH_INVENTORY_V1_ID.len())
}

fn deserialize_source_format<'de, D>(deserializer: D) -> Result<SourceFormatV1, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = deserialize_capped_string(deserializer, "gltf_json".len())?;
    match value.as_str() {
        "fbx" => Ok(SourceFormatV1::Fbx),
        other => Err(serde::de::Error::custom(format!(
            "unknown V1 source format {other:?}"
        ))),
    }
}

fn deserialize_primary_input<'de, D>(deserializer: D) -> Result<InputIdentity, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct WireIdentity {
        #[serde(deserialize_with = "deserialize_sha256")]
        sha256: String,
        bytes: u64,
    }

    let wire = WireIdentity::deserialize(deserializer)?;
    InputIdentity::from_sha256_hex(&wire.sha256, wire.bytes).ok_or_else(|| {
        serde::de::Error::custom(
            "input identity sha256 must be exactly 64 lowercase hexadecimal digits",
        )
    })
}

fn deserialize_sha256<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_capped_string(deserializer, 64)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTransformPathInventoryWireV1 {
    #[serde(deserialize_with = "deserialize_schema")]
    schema: String,
    #[serde(deserialize_with = "deserialize_primary_input")]
    primary_input: InputIdentity,
    #[serde(deserialize_with = "deserialize_source_format")]
    source_format: SourceFormatV1,
    projected_bone_count: u64,
    coverage: RawTransformPathCoverageV1,
    #[serde(deserialize_with = "deserialize_rows")]
    rows: Vec<RawTransformPathNodeRowV1>,
}

impl<'de> Deserialize<'de> for RawTransformPathInventoryV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = RawTransformPathInventoryWireV1::deserialize(deserializer)?;
        let value = Self {
            schema: wire.schema,
            primary_input: wire.primary_input,
            source_format: wire.source_format,
            projected_bone_count: wire.projected_bone_count,
            coverage: wire.coverage,
            rows: wire.rows,
        };
        value.validate().map_err(serde::de::Error::custom)?;
        Ok(value)
    }
}

impl RawTransformPathMatchV1 {
    /// Exact original source node identity.
    pub const fn source_node_index(&self) -> u64 {
        self.source_node_index
    }
    /// Same-load normalized bone identity used for exact rig-role comparison.
    pub const fn projected_bone_index(&self) -> Option<u64> {
        self.projected_bone_index
    }
    /// Root-to-direct-parent original identity chain.
    pub fn parent_chain(&self) -> &[u64] {
        &self.parent_chain
    }
    /// Exact configured/source path which matched.
    pub const fn path(&self) -> &RawTransformPathV1 {
        &self.path
    }
}

/// Byte-exact transform-path resolution result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RawTransformPathResolutionV1 {
    /// Exactly one original non-helper source node matched.
    Exact(RawTransformPathMatchV1),
    /// Complete coverage proves no original source node matched.
    NoMatch,
    /// Two or more original non-helper source nodes have the same path.
    Ambiguous {
        /// All bounded matching original identities in source order.
        matches: Vec<RawTransformPathMatchV1>,
    },
    /// Zero retained matches cannot prove absence under incomplete coverage.
    CoverageIncomplete {
        /// The exact inventory/addressability coverage state.
        coverage: RawTransformPathCoverageV1,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum RawTransformPathResolutionWireV1 {
    Exact(RawTransformPathMatchV1),
    NoMatch,
    Ambiguous {
        #[serde(deserialize_with = "deserialize_matches")]
        matches: Vec<RawTransformPathMatchV1>,
    },
    CoverageIncomplete {
        coverage: RawTransformPathCoverageV1,
    },
}

fn deserialize_matches<'de, D>(deserializer: D) -> Result<Vec<RawTransformPathMatchV1>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let values =
        deserialize_capped_sequence(deserializer, RAW_TRANSFORM_PATH_INVENTORY_V1_MAX_ROWS)?;
    if values.overflowed {
        return Err(serde::de::Error::custom(
            "raw transform ambiguity exceeds the V1 row bound",
        ));
    }
    Ok(values.values)
}

impl<'de> Deserialize<'de> for RawTransformPathResolutionV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        match RawTransformPathResolutionWireV1::deserialize(deserializer)? {
            RawTransformPathResolutionWireV1::Exact(value) => Ok(Self::Exact(value)),
            RawTransformPathResolutionWireV1::NoMatch => Ok(Self::NoMatch),
            RawTransformPathResolutionWireV1::Ambiguous { matches } => {
                Ok(Self::Ambiguous { matches })
            }
            RawTransformPathResolutionWireV1::CoverageIncomplete { coverage } => {
                Ok(Self::CoverageIncomplete { coverage })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> InputIdentity {
        InputIdentity::from_bytes(b"raw-transform-path-test")
    }

    fn input<'a>(
        index: u64,
        parent: Option<u64>,
        name: Option<&'a str>,
        kind: RawTransformPathNodeKindV1,
    ) -> RawTransformPathNodeInputV1<'a> {
        RawTransformPathNodeInputV1 {
            source_node_index: index,
            parent_source_node_index: parent,
            source_name: name,
            projected_bone_index: kind.is_matchable().then_some(index),
            kind,
        }
    }

    fn path_with_bytes(bytes: usize) -> String {
        let mut path = String::new();
        while path.len() < bytes {
            if !path.is_empty() {
                path.push('/');
            }
            let remaining = bytes - path.len();
            let segment_bytes = remaining.min(RAW_TRANSFORM_PATH_V1_MAX_SEGMENT_BYTES);
            path.push_str(&"a".repeat(segment_bytes));
        }
        path
    }

    fn inventory_with_maximum_source_name() -> RawTransformPathInventoryV1 {
        let name = "a".repeat(RAW_TRANSFORM_PATH_V1_MAX_SEGMENT_BYTES);
        RawTransformPathInventoryV1::from_nodes(
            identity(),
            SourceFormatV1::Fbx,
            2,
            [
                input(0, None, None, RawTransformPathNodeKindV1::ImplicitUfbxRoot),
                input(1, Some(0), Some(&name), RawTransformPathNodeKindV1::Source),
            ],
        )
        .unwrap()
    }

    #[test]
    fn grammar_is_closed_unescaped_and_byte_exact() {
        for invalid in [
            "",
            "/Root",
            "Root/",
            "Root//Bone",
            ".",
            "..",
            "A/./B",
            "A\\B",
            "A\nB",
            "A\u{200d}B",
        ] {
            assert!(RawTransformPathV1::parse(invalid).is_err(), "{invalid:?}");
        }
        let path = RawTransformPathV1::parse("Róot/Bone.01").unwrap();
        assert_eq!(path.segments().collect::<Vec<_>>(), ["Róot", "Bone.01"]);
        assert_ne!(path, RawTransformPathV1::parse("róot/Bone.01").unwrap());
    }

    #[test]
    fn resolution_is_byte_exact_without_transforming_source_or_configured_segments() {
        let cases = [
            ("trimming", &[" Root "][..], "Root"),
            ("NFC normalization", &["\u{0065}\u{0301}"][..], "\u{00e9}"),
            ("namespace stripping", &["Armature:Root"][..], "Root"),
            ("prefix matching", &["Rig", "Rooted"][..], "Rig/Root"),
        ];
        for (label, source_segments, configured_path) in cases {
            let mut nodes = vec![input(
                0,
                None,
                None,
                RawTransformPathNodeKindV1::ImplicitUfbxRoot,
            )];
            for (index, segment) in source_segments.iter().enumerate() {
                nodes.push(input(
                    index as u64 + 1,
                    Some(index as u64),
                    Some(segment),
                    RawTransformPathNodeKindV1::Source,
                ));
            }
            let inventory = RawTransformPathInventoryV1::from_nodes(
                identity(),
                SourceFormatV1::Fbx,
                source_segments.len() as u64 + 1,
                nodes,
            )
            .unwrap();
            assert_eq!(
                inventory.resolve(&RawTransformPathV1::parse(configured_path).unwrap()),
                RawTransformPathResolutionV1::NoMatch,
                "{label} must not turn a non-identical path into a match",
            );
        }
    }

    #[test]
    fn path_deserialization_enforces_byte_bounds_before_retention_for_direct_and_escaped_json() {
        let maximum = path_with_bytes(RAW_TRANSFORM_PATH_V1_MAX_PATH_BYTES);
        let direct = serde_json::to_string(&maximum).unwrap();
        assert_eq!(
            serde_json::from_str::<RawTransformPathV1>(&direct)
                .unwrap()
                .as_str(),
            maximum
        );
        let escaped = format!("\"{}\"", maximum.replace('a', "\\u0061"));
        assert_eq!(
            serde_json::from_str::<RawTransformPathV1>(&escaped)
                .unwrap()
                .as_str(),
            maximum
        );

        let oversized = path_with_bytes(RAW_TRANSFORM_PATH_V1_MAX_PATH_BYTES + 1);
        assert!(
            serde_json::from_str::<RawTransformPathV1>(&serde_json::to_string(&oversized).unwrap())
                .is_err()
        );
        let escaped_oversized = format!("\"{}\"", oversized.replace('a', "\\u0061"));
        assert!(serde_json::from_str::<RawTransformPathV1>(&escaped_oversized).is_err());
    }

    #[test]
    fn helper_and_implicit_root_are_retained_but_skipped_for_matching() {
        let inventory = RawTransformPathInventoryV1::from_nodes(
            identity(),
            SourceFormatV1::Fbx,
            4,
            [
                input(0, None, None, RawTransformPathNodeKindV1::ImplicitUfbxRoot),
                input(1, Some(0), Some("Rig"), RawTransformPathNodeKindV1::Source),
                input(
                    2,
                    Some(1),
                    Some("helper"),
                    RawTransformPathNodeKindV1::GeometryTransformHelper,
                ),
                input(3, Some(2), Some("Root"), RawTransformPathNodeKindV1::Source),
            ],
        )
        .unwrap();
        assert_eq!(inventory.coverage(), RawTransformPathCoverageV1::Complete);
        assert_eq!(inventory.rows()[2].parent_chain(), &[0, 1]);
        assert_eq!(inventory.rows()[3].parent_chain(), &[0, 1, 2]);
        let exact = inventory.resolve(&RawTransformPathV1::parse("Rig/Root").unwrap());
        let RawTransformPathResolutionV1::Exact(exact) = exact else {
            panic!("expected exact match");
        };
        assert_eq!(exact.source_node_index(), 3);
        assert_eq!(exact.parent_chain(), &[0, 1, 2]);
        assert_eq!(
            inventory.resolve(&RawTransformPathV1::parse("Rig/helper").unwrap()),
            RawTransformPathResolutionV1::NoMatch
        );
    }

    #[test]
    fn unrepresentable_source_segment_prevents_proven_no_match_without_guessing() {
        let inventory = RawTransformPathInventoryV1::from_nodes(
            identity(),
            SourceFormatV1::Fbx,
            4,
            [
                input(0, None, None, RawTransformPathNodeKindV1::ImplicitUfbxRoot),
                input(
                    1,
                    Some(0),
                    Some("bad/name"),
                    RawTransformPathNodeKindV1::Source,
                ),
                input(2, Some(1), Some("Root"), RawTransformPathNodeKindV1::Source),
                input(3, Some(0), Some("Good"), RawTransformPathNodeKindV1::Source),
            ],
        )
        .unwrap();
        assert!(matches!(
            inventory.coverage(),
            RawTransformPathCoverageV1::Partial(_)
        ));
        assert_eq!(inventory.rows()[1].source_name(), Some("bad/name"));
        assert!(matches!(
            inventory.resolve(&RawTransformPathV1::parse("missing").unwrap()),
            RawTransformPathResolutionV1::CoverageIncomplete { .. }
        ));
        assert!(matches!(
            inventory.resolve(&RawTransformPathV1::parse("Good").unwrap()),
            RawTransformPathResolutionV1::Exact(_)
        ));
    }

    #[test]
    fn every_forbidden_source_segment_class_makes_coverage_incomplete() {
        let oversized = "a".repeat(RAW_TRANSFORM_PATH_V1_MAX_SEGMENT_BYTES + 1);
        let cases = vec![
            ("empty", Some(String::new())),
            ("dot", Some(".".to_owned())),
            ("dot-dot", Some("..".to_owned())),
            ("slash", Some("bad/name".to_owned())),
            ("backslash", Some("bad\\name".to_owned())),
            ("control", Some("bad\nname".to_owned())),
            ("format", Some("bad\u{200d}name".to_owned())),
            ("too long", Some(oversized)),
        ];
        for (label, source_name) in cases {
            let inventory = RawTransformPathInventoryV1::from_nodes(
                identity(),
                SourceFormatV1::Fbx,
                2,
                [
                    input(0, None, None, RawTransformPathNodeKindV1::ImplicitUfbxRoot),
                    input(
                        1,
                        Some(0),
                        source_name.as_deref(),
                        RawTransformPathNodeKindV1::Source,
                    ),
                ],
            )
            .unwrap();
            assert_eq!(
                inventory.coverage(),
                RawTransformPathCoverageV1::Partial(
                    RawTransformPathCoverageReasonV1::UnrepresentableSourceSegment
                ),
                "{label} source segment must make absence unprovable",
            );
            assert!(matches!(
                inventory.resolve(&RawTransformPathV1::parse("Missing").unwrap()),
                RawTransformPathResolutionV1::CoverageIncomplete { .. }
            ));
        }
    }

    #[test]
    fn duplicate_non_helper_paths_are_ambiguous() {
        let inventory = RawTransformPathInventoryV1::from_nodes(
            identity(),
            SourceFormatV1::Fbx,
            3,
            [
                input(0, None, None, RawTransformPathNodeKindV1::ImplicitUfbxRoot),
                input(1, Some(0), Some("Root"), RawTransformPathNodeKindV1::Source),
                input(2, Some(0), Some("Root"), RawTransformPathNodeKindV1::Source),
            ],
        )
        .unwrap();
        let RawTransformPathResolutionV1::Ambiguous { matches } =
            inventory.resolve(&RawTransformPathV1::parse("Root").unwrap())
        else {
            panic!("expected ambiguity");
        };
        assert_eq!(
            matches
                .iter()
                .map(RawTransformPathMatchV1::source_node_index)
                .collect::<Vec<_>>(),
            [1, 2]
        );
    }

    #[test]
    fn row_overflow_and_unavailable_inventory_never_prove_no_match() {
        let mut nodes = Vec::with_capacity(RAW_TRANSFORM_PATH_INVENTORY_V1_MAX_ROWS + 1);
        nodes.push(input(
            0,
            None,
            None,
            RawTransformPathNodeKindV1::ImplicitUfbxRoot,
        ));
        for index in 1..=RAW_TRANSFORM_PATH_INVENTORY_V1_MAX_ROWS as u64 {
            nodes.push(input(
                index,
                Some(0),
                Some("Node"),
                RawTransformPathNodeKindV1::Source,
            ));
        }
        let partial = RawTransformPathInventoryV1::from_nodes(
            identity(),
            SourceFormatV1::Fbx,
            nodes.len() as u64,
            nodes,
        )
        .unwrap();
        assert_eq!(
            partial.rows().len(),
            RAW_TRANSFORM_PATH_INVENTORY_V1_MAX_ROWS
        );
        assert_eq!(
            partial.coverage(),
            RawTransformPathCoverageV1::Partial(
                RawTransformPathCoverageReasonV1::ProjectionBudgetExceeded
            )
        );
        assert!(matches!(
            partial.resolve(&RawTransformPathV1::parse("Missing").unwrap()),
            RawTransformPathResolutionV1::CoverageIncomplete { .. }
        ));

        let unavailable =
            RawTransformPathInventoryV1::unavailable(identity(), SourceFormatV1::Fbx, 0);
        unavailable.validate().unwrap();
        assert!(matches!(
            unavailable.resolve(&RawTransformPathV1::parse("Missing").unwrap()),
            RawTransformPathResolutionV1::CoverageIncomplete { .. }
        ));
    }

    #[test]
    fn readback_rejects_forged_schema_parent_path_coverage_and_bone_mapping() {
        let inventory = RawTransformPathInventoryV1::from_nodes(
            identity(),
            SourceFormatV1::Fbx,
            3,
            [
                input(0, None, None, RawTransformPathNodeKindV1::ImplicitUfbxRoot),
                input(1, Some(0), Some("Rig"), RawTransformPathNodeKindV1::Source),
                input(2, Some(1), Some("Root"), RawTransformPathNodeKindV1::Source),
            ],
        )
        .unwrap();
        let canonical = serde_json::to_value(&inventory).unwrap();
        let round_trip: RawTransformPathInventoryV1 =
            serde_json::from_value(canonical.clone()).unwrap();
        assert_eq!(round_trip, inventory);

        let mutations: [fn(&mut serde_json::Value); 5] = [
            |value| value["schema"] = "forged".into(),
            |value| value["rows"][2]["parent_chain"] = serde_json::json!([0]),
            |value| value["rows"][2]["addressable_path"] = "Rig/Other".into(),
            |value| {
                value["coverage"] = serde_json::json!({
                    "state": "partial",
                    "reason": "unrepresentable_source_segment"
                });
            },
            |value| value["rows"][2]["projected_bone_index"] = 1.into(),
        ];
        for mutation in mutations {
            let mut forged = canonical.clone();
            mutation(&mut forged);
            assert!(serde_json::from_value::<RawTransformPathInventoryV1>(forged).is_err());
        }
    }

    #[test]
    fn inventory_deserialization_rejects_oversized_nested_strings() {
        let inventory = inventory_with_maximum_source_name();
        let canonical = serde_json::to_value(&inventory).unwrap();
        let canonical_json = serde_json::to_string(&canonical).unwrap();
        assert_eq!(
            serde_json::from_str::<RawTransformPathInventoryV1>(&canonical_json).unwrap(),
            inventory
        );

        let oversized_name = "a".repeat(RAW_TRANSFORM_PATH_V1_MAX_SEGMENT_BYTES + 1);
        let mut name = canonical.clone();
        name["rows"][1]["source_name"] = oversized_name.clone().into();
        let direct_name = serde_json::to_string(&name).unwrap();
        assert!(serde_json::from_str::<RawTransformPathInventoryV1>(&direct_name).is_err());
        let escaped_name = direct_name.replacen(
            &serde_json::to_string(&oversized_name).unwrap(),
            &format!("\"{}\"", "\\u0061".repeat(oversized_name.len())),
            1,
        );
        assert!(serde_json::from_str::<RawTransformPathInventoryV1>(&escaped_name).is_err());

        let mut schema = canonical.clone();
        schema["schema"] = format!("{RAW_TRANSFORM_PATH_INVENTORY_V1_ID}x").into();
        assert!(
            serde_json::from_str::<RawTransformPathInventoryV1>(
                &serde_json::to_string(&schema).unwrap()
            )
            .is_err()
        );

        let mut primary_sha256 = canonical;
        primary_sha256["primary_input"]["sha256"] = "a".repeat(65).into();
        assert!(
            serde_json::from_str::<RawTransformPathInventoryV1>(
                &serde_json::to_string(&primary_sha256).unwrap()
            )
            .is_err()
        );
    }

    #[test]
    fn ambiguity_deserialization_is_capped_at_the_inventory_row_limit() {
        let inventory = RawTransformPathInventoryV1::from_nodes(
            identity(),
            SourceFormatV1::Fbx,
            3,
            [
                input(0, None, None, RawTransformPathNodeKindV1::ImplicitUfbxRoot),
                input(1, Some(0), Some("Root"), RawTransformPathNodeKindV1::Source),
                input(2, Some(0), Some("Root"), RawTransformPathNodeKindV1::Source),
            ],
        )
        .unwrap();
        let resolution = inventory.resolve(&RawTransformPathV1::parse("Root").unwrap());
        let mut wire = serde_json::to_value(resolution).unwrap();
        let matches = wire["ambiguous"]["matches"].as_array_mut().unwrap();
        let sample = matches[0].clone();
        matches.clear();
        matches.extend(std::iter::repeat_n(
            sample,
            RAW_TRANSFORM_PATH_INVENTORY_V1_MAX_ROWS,
        ));
        let exact_limit = serde_json::to_string(&wire).unwrap();
        assert!(serde_json::from_str::<RawTransformPathResolutionV1>(&exact_limit).is_ok());

        wire["ambiguous"]["matches"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "source_node_index": 1,
                "projected_bone_index": 1,
                "parent_chain": [0],
                "path": "Root"
            }));
        assert!(
            serde_json::from_str::<RawTransformPathResolutionV1>(
                &serde_json::to_string(&wire).unwrap()
            )
            .is_err()
        );
    }
}
