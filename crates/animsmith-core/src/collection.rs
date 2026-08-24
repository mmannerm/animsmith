//! Format-neutral validated values for collection-manifest V1.
//!
//! The CLI owns manifest-byte limits, TOML parsing, rooted filesystem access,
//! and collection execution. This module deliberately owns only the immutable
//! declaration vocabulary that those layers exchange.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{
    DEPENDENCY_CLOSURE_V1_MAX_KEY_BYTES, DEPENDENCY_CLOSURE_V1_MAX_PATH_COMPONENTS,
    DependencyResourceKeyV1,
};

/// Immutable schema identity for collection manifest V1.
pub const COLLECTION_MANIFEST_V1_ID: &str = "urn:animsmith:schema:collection-manifest:1";
/// Immutable schema version for collection manifest V1.
pub const COLLECTION_MANIFEST_V1_SCHEMA_VERSION: u32 = 1;
/// Immutable identity of the output-independent collection manifest V1 budgets.
pub const COLLECTION_MANIFEST_V1_BUDGET_ID: &str = "urn:animsmith:collection-manifest-budget:1";
/// Maximum manifest bytes accepted by the frontend before TOML decoding.
pub const COLLECTION_MANIFEST_V1_MAX_MANIFEST_BYTES: u64 = 8 * 1024 * 1024;
/// Maximum source rows in one manifest.
pub const COLLECTION_MANIFEST_V1_MAX_SOURCES: usize = 4_096;
/// Maximum clip rows in one manifest.
pub const COLLECTION_MANIFEST_V1_MAX_CLIPS: usize = 4_096;
/// Maximum runtime-set rows in one manifest.
pub const COLLECTION_MANIFEST_V1_MAX_RUNTIME_SETS: usize = 4_096;
/// Maximum members across all runtime sets in one manifest.
pub const COLLECTION_MANIFEST_V1_MAX_AGGREGATE_MEMBERS: usize = 16_384;
/// Maximum aggregate declaration work retained by V1 validation.
pub const COLLECTION_MANIFEST_V1_MAX_AGGREGATE_WORK: usize = 24_576;
/// Maximum UTF-8 bytes in a collection id, source key, clip id, or set id.
pub const COLLECTION_MANIFEST_V1_MAX_IDENTIFIER_BYTES: usize = 255;
/// Maximum UTF-8 bytes in an exact embedded take-name witness.
pub const COLLECTION_MANIFEST_V1_MAX_TAKE_NAME_BYTES: usize = 4_096;

/// Serializable immutable budget record shared by collection-manifest V1 consumers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CollectionManifestBudgetV1 {
    id: &'static str,
    max_manifest_bytes: u64,
    max_sources: usize,
    max_clips: usize,
    max_runtime_sets: usize,
    max_aggregate_members: usize,
    max_aggregate_work: usize,
    max_identifier_bytes: usize,
    max_take_name_bytes: usize,
    max_path_bytes: usize,
    max_path_components: usize,
}

impl CollectionManifestBudgetV1 {
    /// Return the immutable V1 budget values.
    pub const fn v1() -> Self {
        Self {
            id: COLLECTION_MANIFEST_V1_BUDGET_ID,
            max_manifest_bytes: COLLECTION_MANIFEST_V1_MAX_MANIFEST_BYTES,
            max_sources: COLLECTION_MANIFEST_V1_MAX_SOURCES,
            max_clips: COLLECTION_MANIFEST_V1_MAX_CLIPS,
            max_runtime_sets: COLLECTION_MANIFEST_V1_MAX_RUNTIME_SETS,
            max_aggregate_members: COLLECTION_MANIFEST_V1_MAX_AGGREGATE_MEMBERS,
            max_aggregate_work: COLLECTION_MANIFEST_V1_MAX_AGGREGATE_WORK,
            max_identifier_bytes: COLLECTION_MANIFEST_V1_MAX_IDENTIFIER_BYTES,
            max_take_name_bytes: COLLECTION_MANIFEST_V1_MAX_TAKE_NAME_BYTES,
            max_path_bytes: DEPENDENCY_CLOSURE_V1_MAX_KEY_BYTES,
            max_path_components: DEPENDENCY_CLOSURE_V1_MAX_PATH_COMPONENTS,
        }
    }

    /// Immutable budget identity.
    pub const fn id(self) -> &'static str {
        self.id
    }
    /// Maximum manifest input bytes.
    pub const fn max_manifest_bytes(self) -> u64 {
        self.max_manifest_bytes
    }
    /// Maximum source rows.
    pub const fn max_sources(self) -> usize {
        self.max_sources
    }
    /// Maximum clip rows.
    pub const fn max_clips(self) -> usize {
        self.max_clips
    }
    /// Maximum runtime-set rows.
    pub const fn max_runtime_sets(self) -> usize {
        self.max_runtime_sets
    }
    /// Maximum aggregate runtime-set memberships.
    pub const fn max_aggregate_members(self) -> usize {
        self.max_aggregate_members
    }
    /// Maximum aggregate declaration work.
    pub const fn max_aggregate_work(self) -> usize {
        self.max_aggregate_work
    }
    /// Maximum collection/source/logical identifier bytes.
    pub const fn max_identifier_bytes(self) -> usize {
        self.max_identifier_bytes
    }
    /// Maximum expected embedded take-name bytes.
    pub const fn max_take_name_bytes(self) -> usize {
        self.max_take_name_bytes
    }
    /// Maximum safe declared-path bytes.
    pub const fn max_path_bytes(self) -> usize {
        self.max_path_bytes
    }
    /// Maximum safe declared-path components.
    pub const fn max_path_components(self) -> usize {
        self.max_path_components
    }
}

/// A V1 collection manifest was malformed or exceeded a frozen bound.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CollectionManifestError {
    /// The schema identity was not the immutable V1 identity.
    #[error("unsupported collection manifest schema {found:?}")]
    UnsupportedSchema {
        /// Received schema identity.
        found: String,
    },
    /// The schema version did not match V1.
    #[error("unsupported collection manifest schema version {found}")]
    UnsupportedSchemaVersion {
        /// Received schema version.
        found: u32,
    },
    /// A required text field was empty or exceeded its byte bound.
    #[error("invalid {field}: expected nonempty UTF-8 text no longer than {max} bytes")]
    InvalidText {
        /// Stable field path.
        field: &'static str,
        /// Maximum accepted byte length.
        max: usize,
    },
    /// An identifier did not satisfy the V1 lowercase ASCII grammar.
    #[error("invalid {field} {value:?}: expected V1 lowercase ASCII identifier")]
    InvalidIdentifier {
        /// Stable field path.
        field: &'static str,
        /// Rejected value.
        value: String,
    },
    /// A bounded row collection exceeded its limit.
    #[error("{field} has {found} rows, exceeding V1 limit {max}")]
    TooManyRows {
        /// Stable collection field.
        field: &'static str,
        /// Observed row count.
        found: usize,
        /// Maximum accepted count.
        max: usize,
    },
    /// A required source or clip array was empty.
    #[error("{field} must contain at least one row")]
    EmptyRows {
        /// Stable collection field.
        field: &'static str,
    },
    /// Aggregate member work exceeded its V1 limit.
    #[error("runtime_sets members total {found} exceeds V1 limit {max}")]
    TooManyMembers {
        /// Observed aggregate member count.
        found: usize,
        /// Maximum accepted count.
        max: usize,
    },
    /// Aggregate declaration work exceeded its V1 limit.
    #[error("collection manifest aggregate work {found} exceeds V1 limit {max}")]
    TooMuchWork {
        /// Observed aggregate work count.
        found: usize,
        /// Maximum accepted work count.
        max: usize,
    },
    /// A source key, clip id, set id, binding, or member was repeated.
    #[error("duplicate {field} {value:?}")]
    Duplicate {
        /// Stable duplicate category.
        field: &'static str,
        /// Stable rejected value.
        value: String,
    },
    /// A clip named an undeclared source key.
    #[error("clip {clip_id:?} references undeclared source {source_key:?}")]
    DanglingSource {
        /// Logical clip identifier.
        clip_id: String,
        /// Missing source key.
        source_key: String,
    },
    /// A runtime set named an undeclared logical clip id.
    #[error("runtime set {set_id:?} references undeclared member {member:?}")]
    DanglingMember {
        /// Runtime-set identifier.
        set_id: String,
        /// Missing logical clip id.
        member: String,
    },
    /// A runtime set did not contain at least two distinct members.
    #[error("runtime set {set_id:?} needs at least two members, found {found}")]
    TooFewMembers {
        /// Runtime-set identifier.
        set_id: String,
        /// Received member count.
        found: usize,
    },
    /// A clip or runtime-set id escaped the declared collection namespace.
    #[error("{field} {value:?} must start with collection id {collection_id:?} followed by '/'")]
    OutsideCollectionNamespace {
        /// Stable field path.
        field: &'static str,
        /// Rejected id.
        value: String,
        /// Declared collection id.
        collection_id: String,
    },
    /// A digest pin was not exactly 64 lowercase hexadecimal characters.
    #[error("expected_sha256 must be exactly 64 lowercase hexadecimal digits")]
    InvalidDigest,
}

/// One lowercase ASCII namespace token used by V1 collection ids and source keys.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct CollectionIdV1(String);

impl CollectionIdV1 {
    /// Construct one V1 collection id or source key token.
    ///
    /// # Errors
    ///
    /// Returns [`CollectionManifestError::InvalidIdentifier`] when `value` is
    /// not one valid V1 token.
    pub fn new(value: impl Into<String>) -> Result<Self, CollectionManifestError> {
        let value = value.into();
        validate_token("collection_id", &value)?;
        Ok(Self(value))
    }

    /// Exact declared spelling.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Manifest-local source-key token.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct CollectionSourceKeyV1(String);

impl CollectionSourceKeyV1 {
    /// Construct one V1 manifest-local source key.
    ///
    /// # Errors
    ///
    /// Returns [`CollectionManifestError::InvalidIdentifier`] when `value` is
    /// not one valid V1 token.
    pub fn new(value: impl Into<String>) -> Result<Self, CollectionManifestError> {
        let value = value.into();
        validate_token("source.key", &value)?;
        Ok(Self(value))
    }

    /// Exact declared spelling.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Opaque namespaced logical clip or runtime-set identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct CollectionLogicalIdV1(String);

impl CollectionLogicalIdV1 {
    /// Construct one V1 logical identifier.
    ///
    /// # Errors
    ///
    /// Returns [`CollectionManifestError::InvalidIdentifier`] when `value` is
    /// not a slash-separated V1 identifier with at least two tokens.
    pub fn new(value: impl Into<String>) -> Result<Self, CollectionManifestError> {
        let value = value.into();
        if value.len() > COLLECTION_MANIFEST_V1_MAX_IDENTIFIER_BYTES
            || value.split('/').count() < 2
            || value.split('/').any(|token| !is_valid_token(token))
        {
            return Err(CollectionManifestError::InvalidIdentifier {
                field: "logical_id",
                value,
            });
        }
        Ok(Self(value))
    }

    /// Exact declared spelling.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// An optional asserted lowercase SHA-256 digest for one source.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct CollectionDigestPinV1(String);

impl CollectionDigestPinV1 {
    /// Construct one exact SHA-256 digest pin.
    ///
    /// # Errors
    ///
    /// Returns [`CollectionManifestError::InvalidDigest`] when the value is
    /// not 64 lowercase hexadecimal digits.
    pub fn new(value: impl Into<String>) -> Result<Self, CollectionManifestError> {
        let value = value.into();
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        {
            return Err(CollectionManifestError::InvalidDigest);
        }
        Ok(Self(value))
    }

    /// Lowercase hexadecimal SHA-256 text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One source declaration, with safe relative locators but no host I/O state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CollectionSourceV1 {
    key: CollectionSourceKeyV1,
    path: DependencyResourceKeyV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    config: Option<DependencyResourceKeyV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expected_sha256: Option<CollectionDigestPinV1>,
}

impl CollectionSourceV1 {
    /// Construct one source declaration.
    pub fn new(
        key: CollectionSourceKeyV1,
        path: DependencyResourceKeyV1,
        config: Option<DependencyResourceKeyV1>,
        expected_sha256: Option<CollectionDigestPinV1>,
    ) -> Self {
        Self {
            key,
            path,
            config,
            expected_sha256,
        }
    }

    /// Manifest-local source key.
    pub fn key(&self) -> &CollectionSourceKeyV1 {
        &self.key
    }

    /// Safe source locator resolved under the optional input root.
    pub fn path(&self) -> &DependencyResourceKeyV1 {
        &self.path
    }

    /// Optional safe config locator resolved under the manifest directory.
    pub fn config(&self) -> Option<&DependencyResourceKeyV1> {
        self.config.as_ref()
    }

    /// Optional asserted source digest.
    pub fn expected_sha256(&self) -> Option<&CollectionDigestPinV1> {
        self.expected_sha256.as_ref()
    }
}

/// One declared logical-to-physical take binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CollectionClipV1 {
    id: CollectionLogicalIdV1,
    source: CollectionSourceKeyV1,
    take_index: u32,
    take_name: String,
}

impl CollectionClipV1 {
    /// Construct one exact source-local take witness.
    ///
    /// # Errors
    ///
    /// Returns [`CollectionManifestError::InvalidText`] when the exact
    /// expected embedded take name is empty or exceeds its V1 byte limit.
    pub fn new(
        id: CollectionLogicalIdV1,
        source: CollectionSourceKeyV1,
        take_index: u32,
        take_name: impl Into<String>,
    ) -> Result<Self, CollectionManifestError> {
        let take_name = take_name.into();
        validate_text(
            "clips.take_name",
            &take_name,
            COLLECTION_MANIFEST_V1_MAX_TAKE_NAME_BYTES,
        )?;
        Ok(Self {
            id,
            source,
            take_index,
            take_name,
        })
    }

    /// Durable logical id.
    pub fn id(&self) -> &CollectionLogicalIdV1 {
        &self.id
    }

    /// Declared source key.
    pub fn source(&self) -> &CollectionSourceKeyV1 {
        &self.source
    }

    /// Zero-based source-local take index.
    pub const fn take_index(&self) -> u32 {
        self.take_index
    }

    /// Exact expected embedded take name at [`Self::take_index`].
    pub fn take_name(&self) -> &str {
        &self.take_name
    }
}

/// Closed V1 runtime-set membership vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CollectionRuntimeSetKindV1 {
    /// A gait-related collection.
    GaitGroup,
    /// A synchronized-action collection.
    SyncGroup,
    /// A directional blend collection.
    DirectionalBlend,
    /// A speed blend collection.
    SpeedBlend,
    /// A transition-chain collection.
    TransitionChain,
    /// A mask-composition collection.
    MaskComposition,
    /// A retargeting collection.
    RetargetGroup,
    /// A paired interaction collection.
    PairedInteraction,
    /// A motion-database collection.
    MotionDatabase,
}

/// One ordered collection runtime-set declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CollectionRuntimeSetV1 {
    id: CollectionLogicalIdV1,
    kind: CollectionRuntimeSetKindV1,
    members: Vec<CollectionLogicalIdV1>,
}

impl CollectionRuntimeSetV1 {
    /// Construct one ordered runtime-set declaration.
    pub fn new(
        id: CollectionLogicalIdV1,
        kind: CollectionRuntimeSetKindV1,
        members: Vec<CollectionLogicalIdV1>,
    ) -> Self {
        Self { id, kind, members }
    }

    /// Durable runtime-set id.
    pub fn id(&self) -> &CollectionLogicalIdV1 {
        &self.id
    }

    /// Closed V1 membership kind.
    pub const fn kind(&self) -> CollectionRuntimeSetKindV1 {
        self.kind
    }

    /// Declared member order, retained without sorting.
    pub fn members(&self) -> &[CollectionLogicalIdV1] {
        &self.members
    }
}

/// Fully validated V1 collection declaration, canonically ordered by stable ids.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CollectionManifestV1 {
    schema: &'static str,
    schema_version: u32,
    collection_id: CollectionIdV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    input_root: Option<DependencyResourceKeyV1>,
    sources: Vec<CollectionSourceV1>,
    clips: Vec<CollectionClipV1>,
    runtime_sets: Vec<CollectionRuntimeSetV1>,
}

impl CollectionManifestV1 {
    /// Construct and validate one V1 manifest.
    ///
    /// Sources, clips, and runtime sets are sorted by their stable ids. A set's
    /// member order remains exactly as declared.
    ///
    /// # Errors
    ///
    /// Returns [`CollectionManifestError`] when a declaration is incomplete,
    /// ambiguous, dangling, outside the collection namespace, or exceeds a
    /// frozen V1 bound.
    pub fn new(
        collection_id: CollectionIdV1,
        input_root: Option<DependencyResourceKeyV1>,
        mut sources: Vec<CollectionSourceV1>,
        mut clips: Vec<CollectionClipV1>,
        mut runtime_sets: Vec<CollectionRuntimeSetV1>,
    ) -> Result<Self, CollectionManifestError> {
        validate_rows("sources", sources.len(), COLLECTION_MANIFEST_V1_MAX_SOURCES)?;
        validate_rows("clips", clips.len(), COLLECTION_MANIFEST_V1_MAX_CLIPS)?;
        validate_rows(
            "runtime_sets",
            runtime_sets.len(),
            COLLECTION_MANIFEST_V1_MAX_RUNTIME_SETS,
        )?;
        if sources.is_empty() {
            return Err(CollectionManifestError::EmptyRows { field: "sources" });
        }
        if clips.is_empty() {
            return Err(CollectionManifestError::EmptyRows { field: "clips" });
        }

        sources.sort_by(|left, right| left.key.cmp(&right.key));
        clips.sort_by(|left, right| left.id.cmp(&right.id));
        runtime_sets.sort_by(|left, right| left.id.cmp(&right.id));

        let mut source_keys = BTreeSet::new();
        for source in &sources {
            if !source_keys.insert(source.key.clone()) {
                return Err(CollectionManifestError::Duplicate {
                    field: "source key",
                    value: source.key.0.clone(),
                });
            }
        }

        let namespace = format!("{}/", collection_id.as_str());
        let mut clip_ids = BTreeSet::new();
        let mut bindings = BTreeSet::new();
        for clip in &clips {
            validate_namespace("clips.id", &clip.id, &collection_id, &namespace)?;
            if !source_keys.contains(&clip.source) {
                return Err(CollectionManifestError::DanglingSource {
                    clip_id: clip.id.0.clone(),
                    source_key: clip.source.0.clone(),
                });
            }
            if !clip_ids.insert(clip.id.clone()) {
                return Err(CollectionManifestError::Duplicate {
                    field: "clip id",
                    value: clip.id.0.clone(),
                });
            }
            if !bindings.insert((clip.source.clone(), clip.take_index)) {
                return Err(CollectionManifestError::Duplicate {
                    field: "source/take binding",
                    value: format!("{}:{}", clip.source.as_str(), clip.take_index),
                });
            }
        }

        let mut total_members = 0usize;
        let mut aggregate_work = sources
            .len()
            .checked_add(clips.len())
            .and_then(|value| value.checked_add(runtime_sets.len()))
            .ok_or(CollectionManifestError::TooMuchWork {
                found: usize::MAX,
                max: COLLECTION_MANIFEST_V1_MAX_AGGREGATE_WORK,
            })?;
        let mut set_ids = BTreeSet::new();
        for runtime_set in &runtime_sets {
            validate_namespace(
                "runtime_sets.id",
                &runtime_set.id,
                &collection_id,
                &namespace,
            )?;
            if !set_ids.insert(runtime_set.id.clone()) {
                return Err(CollectionManifestError::Duplicate {
                    field: "runtime set id",
                    value: runtime_set.id.0.clone(),
                });
            }
            if runtime_set.members.len() < 2 {
                return Err(CollectionManifestError::TooFewMembers {
                    set_id: runtime_set.id.0.clone(),
                    found: runtime_set.members.len(),
                });
            }
            total_members = total_members.checked_add(runtime_set.members.len()).ok_or(
                CollectionManifestError::TooManyMembers {
                    found: usize::MAX,
                    max: COLLECTION_MANIFEST_V1_MAX_AGGREGATE_MEMBERS,
                },
            )?;
            aggregate_work = aggregate_work
                .checked_add(runtime_set.members.len())
                .ok_or(CollectionManifestError::TooMuchWork {
                    found: usize::MAX,
                    max: COLLECTION_MANIFEST_V1_MAX_AGGREGATE_WORK,
                })?;
            if aggregate_work > COLLECTION_MANIFEST_V1_MAX_AGGREGATE_WORK {
                return Err(CollectionManifestError::TooMuchWork {
                    found: aggregate_work,
                    max: COLLECTION_MANIFEST_V1_MAX_AGGREGATE_WORK,
                });
            }
            if total_members > COLLECTION_MANIFEST_V1_MAX_AGGREGATE_MEMBERS {
                return Err(CollectionManifestError::TooManyMembers {
                    found: total_members,
                    max: COLLECTION_MANIFEST_V1_MAX_AGGREGATE_MEMBERS,
                });
            }
            let mut members = BTreeSet::new();
            for member in &runtime_set.members {
                if !members.insert(member.clone()) {
                    return Err(CollectionManifestError::Duplicate {
                        field: "runtime set member",
                        value: format!("{}:{}", runtime_set.id.as_str(), member.as_str()),
                    });
                }
                if !clip_ids.contains(member) {
                    return Err(CollectionManifestError::DanglingMember {
                        set_id: runtime_set.id.0.clone(),
                        member: member.0.clone(),
                    });
                }
            }
        }

        Ok(Self {
            schema: COLLECTION_MANIFEST_V1_ID,
            schema_version: COLLECTION_MANIFEST_V1_SCHEMA_VERSION,
            collection_id,
            input_root,
            sources,
            clips,
            runtime_sets,
        })
    }

    /// Immutable V1 schema identity.
    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    /// Immutable V1 schema version.
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Collection namespace token.
    pub fn collection_id(&self) -> &CollectionIdV1 {
        &self.collection_id
    }

    /// Optional safe source-root locator below the manifest directory.
    pub fn input_root(&self) -> Option<&DependencyResourceKeyV1> {
        self.input_root.as_ref()
    }

    /// Sources in canonical source-key order.
    pub fn sources(&self) -> &[CollectionSourceV1] {
        &self.sources
    }

    /// Clips in canonical logical-id order.
    pub fn clips(&self) -> &[CollectionClipV1] {
        &self.clips
    }

    /// Runtime sets in canonical logical-id order; each set retains member order.
    pub fn runtime_sets(&self) -> &[CollectionRuntimeSetV1] {
        &self.runtime_sets
    }
}

fn validate_token(field: &'static str, value: &str) -> Result<(), CollectionManifestError> {
    if value.len() > COLLECTION_MANIFEST_V1_MAX_IDENTIFIER_BYTES || !is_valid_token(value) {
        return Err(CollectionManifestError::InvalidIdentifier {
            field,
            value: value.to_owned(),
        });
    }
    Ok(())
}

fn is_valid_token(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes
        .first()
        .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && bytes.iter().skip(1).all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
}

fn validate_text(
    field: &'static str,
    value: &str,
    max: usize,
) -> Result<(), CollectionManifestError> {
    if value.is_empty() || value.len() > max {
        return Err(CollectionManifestError::InvalidText { field, max });
    }
    Ok(())
}

fn validate_rows(
    field: &'static str,
    found: usize,
    max: usize,
) -> Result<(), CollectionManifestError> {
    if found > max {
        return Err(CollectionManifestError::TooManyRows { field, found, max });
    }
    Ok(())
}

fn validate_namespace(
    field: &'static str,
    id: &CollectionLogicalIdV1,
    collection_id: &CollectionIdV1,
    namespace: &str,
) -> Result<(), CollectionManifestError> {
    if !id.as_str().starts_with(namespace) {
        return Err(CollectionManifestError::OutsideCollectionNamespace {
            field,
            value: id.0.clone(),
            collection_id: collection_id.0.clone(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ResourceKeySyntaxV1;

    fn key(value: &str) -> DependencyResourceKeyV1 {
        DependencyResourceKeyV1::from_source_str(value, ResourceKeySyntaxV1::ParserRelativePath)
            .unwrap()
    }

    fn source(value: &str) -> CollectionSourceV1 {
        CollectionSourceV1::new(
            CollectionSourceKeyV1::new(value).unwrap(),
            key("motion.fbx"),
            None,
            None,
        )
    }

    fn clip(id: &str, source: &str, take_index: u32) -> CollectionClipV1 {
        CollectionClipV1::new(
            CollectionLogicalIdV1::new(id).unwrap(),
            CollectionSourceKeyV1::new(source).unwrap(),
            take_index,
            "Take 001",
        )
        .unwrap()
    }

    fn set(id: &str, members: &[&str]) -> CollectionRuntimeSetV1 {
        CollectionRuntimeSetV1::new(
            CollectionLogicalIdV1::new(id).unwrap(),
            CollectionRuntimeSetKindV1::GaitGroup,
            members
                .iter()
                .map(|member| CollectionLogicalIdV1::new(*member).unwrap())
                .collect(),
        )
    }

    fn manifest(
        sources: Vec<CollectionSourceV1>,
        clips: Vec<CollectionClipV1>,
        sets: Vec<CollectionRuntimeSetV1>,
    ) -> Result<CollectionManifestV1, CollectionManifestError> {
        CollectionManifestV1::new(
            CollectionIdV1::new("com.example.pack").unwrap(),
            None,
            sources,
            clips,
            sets,
        )
    }

    #[test]
    fn valid_manifest_canonicalizes_rows_but_preserves_member_order() {
        let forward = "com.example.pack/locomotion/forward";
        let left = "com.example.pack/locomotion/left";
        let value = manifest(
            vec![source("zebra"), source("alpha")],
            vec![clip(left, "alpha", 0), clip(forward, "zebra", 1)],
            vec![set("com.example.pack/sets/ring", &[left, forward])],
        )
        .unwrap();
        assert_eq!(value.sources()[0].key().as_str(), "alpha");
        assert_eq!(value.clips()[0].id().as_str(), forward);
        assert_eq!(value.runtime_sets()[0].members()[0].as_str(), left);
        assert_eq!(value.schema(), COLLECTION_MANIFEST_V1_ID);
        assert_eq!(
            value.schema_version(),
            COLLECTION_MANIFEST_V1_SCHEMA_VERSION
        );
    }

    #[test]
    fn source_rename_does_not_change_logical_identity() {
        let id = "com.example.pack/locomotion/walk";
        let before = manifest(
            vec![source("old-file")],
            vec![clip(id, "old-file", 0)],
            vec![],
        )
        .unwrap();
        let after = manifest(
            vec![source("renamed-file")],
            vec![clip(id, "renamed-file", 0)],
            vec![],
        )
        .unwrap();
        assert_eq!(before.clips()[0].id(), after.clips()[0].id());
        assert_ne!(before.sources()[0].key(), after.sources()[0].key());
    }

    #[test]
    fn exact_take_names_and_distinct_indices_are_retained() {
        let id = "com.example.pack/locomotion/walk";
        let other = "com.example.pack/locomotion/run";
        let value = manifest(
            vec![source("loco")],
            vec![clip(id, "loco", 0), clip(other, "loco", 1)],
            vec![],
        )
        .unwrap();
        assert_eq!(value.clips()[0].take_name(), "Take 001");
        assert_eq!(value.clips()[1].take_name(), "Take 001");
    }

    #[test]
    fn rejects_duplicate_and_dangling_declarations() {
        let id = "com.example.pack/locomotion/walk";
        let other = "com.example.pack/locomotion/run";
        assert!(matches!(
            manifest(
                vec![source("a"), source("a")],
                vec![clip(id, "a", 0)],
                vec![]
            ),
            Err(CollectionManifestError::Duplicate {
                field: "source key",
                ..
            })
        ));
        assert!(matches!(
            manifest(
                vec![source("a")],
                vec![clip(id, "a", 0), clip(id, "a", 1)],
                vec![]
            ),
            Err(CollectionManifestError::Duplicate {
                field: "clip id",
                ..
            })
        ));
        assert!(matches!(
            manifest(
                vec![source("a")],
                vec![clip(id, "a", 0), clip(other, "a", 0)],
                vec![]
            ),
            Err(CollectionManifestError::Duplicate {
                field: "source/take binding",
                ..
            })
        ));
        assert!(matches!(
            manifest(vec![source("a")], vec![clip(id, "missing", 0)], vec![]),
            Err(CollectionManifestError::DanglingSource { .. })
        ));
        assert!(matches!(
            manifest(
                vec![source("a")],
                vec![clip(id, "a", 0), clip(other, "a", 1)],
                vec![set("com.example.pack/sets/a", &[id, id])]
            ),
            Err(CollectionManifestError::Duplicate {
                field: "runtime set member",
                ..
            })
        ));
        assert!(matches!(
            manifest(
                vec![source("a")],
                vec![clip(id, "a", 0)],
                vec![set(
                    "com.example.pack/sets/a",
                    &[id, "com.example.pack/locomotion/missing"]
                )]
            ),
            Err(CollectionManifestError::DanglingMember { .. })
        ));
        assert!(matches!(
            manifest(
                vec![source("a")],
                vec![clip(id, "a", 0)],
                vec![set("com.example.pack/sets/a", &[id])]
            ),
            Err(CollectionManifestError::TooFewMembers { .. })
        ));
    }

    #[test]
    fn rejects_namespace_and_digest_violations() {
        assert!(matches!(
            manifest(
                vec![source("a")],
                vec![clip("other.pack/locomotion/walk", "a", 0)],
                vec![]
            ),
            Err(CollectionManifestError::OutsideCollectionNamespace { .. })
        ));
        assert!(CollectionDigestPinV1::new("A".repeat(64)).is_err());
        assert!(CollectionDigestPinV1::new("0".repeat(63)).is_err());
    }

    #[test]
    fn preserves_unicode_take_names_and_reuses_safe_path_policy() {
        let value = CollectionClipV1::new(
            CollectionLogicalIdV1::new("com.example.pack/locomotion/walk").unwrap(),
            CollectionSourceKeyV1::new("walk").unwrap(),
            0,
            "Pas \u{00e9} 001",
        )
        .unwrap();
        assert_eq!(value.take_name(), "Pas \u{00e9} 001");
        for unsafe_path in [
            "/absolute.fbx",
            "a\\b.fbx",
            "../escape.fbx",
            "https://x/y.fbx",
        ] {
            assert!(
                DependencyResourceKeyV1::from_source_str(
                    unsafe_path,
                    ResourceKeySyntaxV1::ParserRelativePath,
                )
                .is_err()
            );
        }
    }

    #[test]
    fn identifiers_and_take_name_have_exact_bounds() {
        let token = format!(
            "a{}",
            "x".repeat(COLLECTION_MANIFEST_V1_MAX_IDENTIFIER_BYTES - 1)
        );
        assert!(CollectionIdV1::new(token).is_ok());
        assert!(
            CollectionIdV1::new(format!(
                "a{}",
                "x".repeat(COLLECTION_MANIFEST_V1_MAX_IDENTIFIER_BYTES)
            ))
            .is_err()
        );
        assert!(
            CollectionClipV1::new(
                CollectionLogicalIdV1::new("com.example/a").unwrap(),
                CollectionSourceKeyV1::new("a").unwrap(),
                0,
                "x".repeat(COLLECTION_MANIFEST_V1_MAX_TAKE_NAME_BYTES)
            )
            .is_ok()
        );
        assert!(
            CollectionClipV1::new(
                CollectionLogicalIdV1::new("com.example/a").unwrap(),
                CollectionSourceKeyV1::new("a").unwrap(),
                0,
                "x".repeat(COLLECTION_MANIFEST_V1_MAX_TAKE_NAME_BYTES + 1)
            )
            .is_err()
        );
        assert_eq!(
            CollectionClipV1::new(
                CollectionLogicalIdV1::new("com.example/a").unwrap(),
                CollectionSourceKeyV1::new("a").unwrap(),
                u32::MAX,
                "Take 001",
            )
            .unwrap()
            .take_index(),
            u32::MAX
        );
    }

    #[test]
    fn budget_record_exposes_every_frozen_core_limit() {
        let budget = CollectionManifestBudgetV1::v1();
        assert_eq!(budget.id(), COLLECTION_MANIFEST_V1_BUDGET_ID);
        assert_eq!(
            budget.max_manifest_bytes(),
            COLLECTION_MANIFEST_V1_MAX_MANIFEST_BYTES
        );
        assert_eq!(budget.max_sources(), COLLECTION_MANIFEST_V1_MAX_SOURCES);
        assert_eq!(budget.max_clips(), COLLECTION_MANIFEST_V1_MAX_CLIPS);
        assert_eq!(
            budget.max_runtime_sets(),
            COLLECTION_MANIFEST_V1_MAX_RUNTIME_SETS
        );
        assert_eq!(
            budget.max_aggregate_members(),
            COLLECTION_MANIFEST_V1_MAX_AGGREGATE_MEMBERS
        );
        assert_eq!(
            budget.max_aggregate_work(),
            COLLECTION_MANIFEST_V1_MAX_AGGREGATE_WORK
        );
        assert_eq!(
            budget.max_identifier_bytes(),
            COLLECTION_MANIFEST_V1_MAX_IDENTIFIER_BYTES
        );
        assert_eq!(
            budget.max_take_name_bytes(),
            COLLECTION_MANIFEST_V1_MAX_TAKE_NAME_BYTES
        );
        assert_eq!(budget.max_path_bytes(), DEPENDENCY_CLOSURE_V1_MAX_KEY_BYTES);
        assert_eq!(
            budget.max_path_components(),
            DEPENDENCY_CLOSURE_V1_MAX_PATH_COMPONENTS
        );
    }

    #[test]
    fn rows_and_aggregate_members_have_exact_bounds() {
        let id = "com.example.pack/locomotion/walk";
        let sources = (0..COLLECTION_MANIFEST_V1_MAX_SOURCES)
            .map(|index| source(&format!("s{index}")))
            .collect();
        assert!(manifest(sources, vec![clip(id, "s0", 0)], vec![]).is_ok());
        let too_many_sources = (0..=COLLECTION_MANIFEST_V1_MAX_SOURCES)
            .map(|index| source(&format!("s{index}")))
            .collect();
        assert!(matches!(
            manifest(too_many_sources, vec![clip(id, "s0", 0)], vec![]),
            Err(CollectionManifestError::TooManyRows {
                field: "sources",
                ..
            })
        ));

        let clips: Vec<_> = (0..COLLECTION_MANIFEST_V1_MAX_CLIPS)
            .map(|index| clip(&format!("com.example.pack/c/{index}"), "a", index as u32))
            .collect();
        assert!(manifest(vec![source("a")], clips, vec![]).is_ok());
        let too_many_clips: Vec<_> = (0..=COLLECTION_MANIFEST_V1_MAX_CLIPS)
            .map(|index| clip(&format!("com.example.pack/c/{index}"), "a", index as u32))
            .collect();
        assert!(matches!(
            manifest(vec![source("a")], too_many_clips, vec![]),
            Err(CollectionManifestError::TooManyRows { field: "clips", .. })
        ));

        let members: Vec<_> = (0..4)
            .map(|index| format!("com.example.pack/m/{index}"))
            .collect();
        let member_refs: Vec<_> = members.iter().map(String::as_str).collect();
        let member_clips = members
            .iter()
            .enumerate()
            .map(|(index, value)| clip(value, "a", index as u32))
            .collect();
        let sets = (0..COLLECTION_MANIFEST_V1_MAX_RUNTIME_SETS)
            .map(|index| set(&format!("com.example.pack/sets/{index}"), &member_refs))
            .collect();
        assert!(manifest(vec![source("a")], member_clips, sets).is_ok());
        let too_many_sets = (0..=COLLECTION_MANIFEST_V1_MAX_RUNTIME_SETS)
            .map(|index| set(&format!("com.example.pack/sets/{index}"), &member_refs))
            .collect();
        let member_clips = members
            .iter()
            .enumerate()
            .map(|(index, value)| clip(value, "a", index as u32))
            .collect();
        assert!(matches!(
            manifest(vec![source("a")], member_clips, too_many_sets),
            Err(CollectionManifestError::TooManyRows {
                field: "runtime_sets",
                ..
            })
        ));

        let overflow_members = (0..5)
            .map(|index| format!("com.example.pack/x/{index}"))
            .collect::<Vec<_>>();
        let overflow_refs = overflow_members
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        let overflow_clips = overflow_members
            .iter()
            .enumerate()
            .map(|(index, value)| clip(value, "a", index as u32))
            .collect();
        let overflow_sets = (0..COLLECTION_MANIFEST_V1_MAX_RUNTIME_SETS)
            .map(|index| set(&format!("com.example.pack/sets/{index}"), &overflow_refs))
            .collect();
        assert!(matches!(
            manifest(vec![source("a")], overflow_clips, overflow_sets),
            Err(CollectionManifestError::TooManyMembers { .. })
        ));
    }

    #[test]
    fn aggregate_work_has_an_exact_boundary_independent_of_membership() {
        let ids: Vec<_> = (0..4)
            .map(|index| format!("com.example.pack/c/{index}"))
            .collect();
        let members: Vec<_> = ids.iter().map(String::as_str).collect();
        let sources = (0..COLLECTION_MANIFEST_V1_MAX_SOURCES)
            .map(|index| source(&format!("s{index}")))
            .collect();
        let clips = (0..COLLECTION_MANIFEST_V1_MAX_CLIPS)
            .map(|index| clip(&format!("com.example.pack/c/{index}"), "s0", index as u32))
            .collect::<Vec<_>>();
        let sets = (0..COLLECTION_MANIFEST_V1_MAX_RUNTIME_SETS)
            .map(|index| set(&format!("com.example.pack/sets/{index}"), &members[..3]))
            .collect();
        assert!(manifest(sources, clips, sets).is_ok());

        let clips = (0..COLLECTION_MANIFEST_V1_MAX_CLIPS)
            .map(|index| clip(&format!("com.example.pack/c/{index}"), "s0", index as u32))
            .collect::<Vec<_>>();
        let sets = (0..COLLECTION_MANIFEST_V1_MAX_RUNTIME_SETS)
            .map(|index| set(&format!("com.example.pack/sets/{index}"), &members))
            .collect();
        assert!(matches!(
            manifest(vec![source("s0")], clips, sets),
            Err(CollectionManifestError::TooMuchWork { .. })
        ));
    }

    #[test]
    fn aggregate_members_have_an_exact_boundary_independent_of_work() {
        let clips = (0..COLLECTION_MANIFEST_V1_MAX_CLIPS)
            .map(|index| clip(&format!("com.example.pack/c/{index}"), "a", index as u32))
            .collect::<Vec<_>>();
        let members = clips
            .iter()
            .map(|clip| clip.id().as_str())
            .collect::<Vec<_>>();
        let sets = (0..4)
            .map(|index| set(&format!("com.example.pack/sets/{index}"), &members))
            .collect();
        assert!(manifest(vec![source("a")], clips, sets).is_ok());

        let clips = (0..COLLECTION_MANIFEST_V1_MAX_CLIPS)
            .map(|index| clip(&format!("com.example.pack/c/{index}"), "a", index as u32))
            .collect::<Vec<_>>();
        let members = clips
            .iter()
            .map(|clip| clip.id().as_str())
            .collect::<Vec<_>>();
        let sets = vec![
            set("com.example.pack/sets/0", &members),
            set("com.example.pack/sets/1", &members),
            set("com.example.pack/sets/2", &members),
            set("com.example.pack/sets/3", &members[..members.len() - 1]),
            set("com.example.pack/sets/4", &members[..2]),
        ];
        assert!(matches!(
            manifest(vec![source("a")], clips, sets),
            Err(CollectionManifestError::TooManyMembers { .. })
        ));
    }
}
