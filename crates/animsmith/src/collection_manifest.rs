//! CLI-owned parsing and rooted path resolution for collection-manifest V1.
//!
//! The validated declaration vocabulary lives in animsmith-core. This module
//! owns strict TOML input and host filesystem access; canonical host paths and
//! OS diagnostics never cross its internal boundary.

#![allow(dead_code)]

use animsmith_core::{
    COLLECTION_MANIFEST_V1_ID, COLLECTION_MANIFEST_V1_MAX_CLIPS,
    COLLECTION_MANIFEST_V1_MAX_MANIFEST_BYTES, COLLECTION_MANIFEST_V1_MAX_RUNTIME_SETS,
    COLLECTION_MANIFEST_V1_MAX_SOURCES, CollectionClipV1, CollectionDigestPinV1, CollectionIdV1,
    CollectionLogicalIdV1, CollectionManifestError, CollectionManifestV1,
    CollectionRuntimeSetKindV1, CollectionRuntimeSetV1, CollectionSourceKeyV1, CollectionSourceV1,
    DependencyResourceKeyV1, ResourceKeySyntaxV1,
};
use serde::Deserialize;
use serde::de::{Deserializer, IgnoredAny, SeqAccess, Visitor};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

/// Stable category for a collection control-plane failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CollectionControlKind {
    ManifestRead,
    ManifestTooLarge,
    ManifestEncoding,
    ManifestMalformed,
    UnsupportedSchema,
    UnsupportedSchemaVersion,
    InvalidDeclaration,
    InvalidInputRoot,
    UnsafePath,
    SourceNonRegular,
    ConfigMissing,
    ConfigUnreadable,
    ConfigNonRegular,
    CanonicalSourceAlias,
    DuplicateSourcePath,
}

impl CollectionControlKind {
    fn label(self) -> &'static str {
        match self {
            Self::ManifestRead => "manifest-read",
            Self::ManifestTooLarge => "manifest-too-large",
            Self::ManifestEncoding => "manifest-encoding",
            Self::ManifestMalformed => "manifest-malformed",
            Self::UnsupportedSchema => "unsupported-schema",
            Self::UnsupportedSchemaVersion => "unsupported-schema-version",
            Self::InvalidDeclaration => "invalid-declaration",
            Self::InvalidInputRoot => "invalid-input-root",
            Self::UnsafePath => "unsafe-path",
            Self::SourceNonRegular => "source-non-regular",
            Self::ConfigMissing => "config-missing",
            Self::ConfigUnreadable => "config-unreadable",
            Self::ConfigNonRegular => "config-non-regular",
            Self::CanonicalSourceAlias => "canonical-source-alias",
            Self::DuplicateSourcePath => "duplicate-source-path",
        }
    }
}

/// A collection control-plane failure with no host path or raw OS diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CollectionControlError {
    kind: CollectionControlKind,
}

impl CollectionControlError {
    fn new(kind: CollectionControlKind) -> Self {
        Self { kind }
    }
    pub(crate) fn kind(&self) -> CollectionControlKind {
        self.kind
    }
}

impl fmt::Display for CollectionControlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "collection control error ({})",
            self.kind.label()
        )
    }
}

impl std::error::Error for CollectionControlError {}

/// Why a safe source declaration did not produce a readable source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CollectionSourceUnavailable {
    Missing,
    Unreadable,
}

/// One source path state returned by the rooted resolver.
#[derive(Clone, PartialEq, Eq)]
pub(crate) enum CollectionSourceResolution {
    Ready(CollectionResolvedPath),
    Unavailable {
        declared: String,
        reason: CollectionSourceUnavailable,
    },
}

impl fmt::Debug for CollectionSourceResolution {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ready(path) => formatter
                .debug_struct("Ready")
                .field("declared", &path.declared)
                .finish(),
            Self::Unavailable { declared, reason } => formatter
                .debug_struct("Unavailable")
                .field("declared", declared)
                .field("reason", reason)
                .finish(),
        }
    }
}

/// A safe declared path paired with an internal canonical host path.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct CollectionResolvedPath {
    declared: String,
    canonical: PathBuf,
}

impl fmt::Debug for CollectionResolvedPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CollectionResolvedPath")
            .field("declared", &self.declared)
            .finish_non_exhaustive()
    }
}

impl CollectionResolvedPath {
    pub(crate) fn declared(&self) -> &str {
        &self.declared
    }
    pub(crate) fn path(&self) -> &Path {
        &self.canonical
    }
}

/// A selected config path. Default means no config path was declared.
#[derive(Clone, PartialEq, Eq)]
pub(crate) enum CollectionConfigResolution {
    Default,
    Explicit(CollectionResolvedPath),
}

impl fmt::Debug for CollectionConfigResolution {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Default => formatter.write_str("Default"),
            Self::Explicit(path) => formatter
                .debug_struct("Explicit")
                .field("declared", &path.declared)
                .finish(),
        }
    }
}

/// Canonical manifest and source roots used for collection declarations.
#[derive(Clone)]
pub(crate) struct CollectionPathResolver {
    control_root: PathBuf,
    source_root: PathBuf,
}

impl fmt::Debug for CollectionPathResolver {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CollectionPathResolver { roots: redacted }")
    }
}

/// Read and strictly validate one collection manifest from bounded bytes.
pub(crate) fn load_collection_manifest(
    manifest_path: &Path,
) -> Result<CollectionManifestV1, CollectionControlError> {
    let bytes = read_bounded(manifest_path, COLLECTION_MANIFEST_V1_MAX_MANIFEST_BYTES)
        .map_err(CollectionControlError::new)?;
    parse_collection_manifest_bytes(&bytes)
}

/// Parse one already bounded collection-manifest TOML byte sequence.
pub(crate) fn parse_collection_manifest_bytes(
    bytes: &[u8],
) -> Result<CollectionManifestV1, CollectionControlError> {
    if bytes.len() as u64 > COLLECTION_MANIFEST_V1_MAX_MANIFEST_BYTES {
        return Err(CollectionControlError::new(
            CollectionControlKind::ManifestTooLarge,
        ));
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| CollectionControlError::new(CollectionControlKind::ManifestEncoding))?;
    let header = toml::from_str::<CollectionManifestHeaderWire>(text)
        .map_err(|_| CollectionControlError::new(CollectionControlKind::ManifestMalformed))?;
    if header.schema != COLLECTION_MANIFEST_V1_ID {
        return Err(CollectionControlError::new(
            CollectionControlKind::UnsupportedSchema,
        ));
    }
    if header.schema_version != animsmith_core::COLLECTION_MANIFEST_V1_SCHEMA_VERSION {
        return Err(CollectionControlError::new(
            CollectionControlKind::UnsupportedSchemaVersion,
        ));
    }
    let wire = toml::from_str::<CollectionManifestWire>(text)
        .map_err(|_| CollectionControlError::new(CollectionControlKind::ManifestMalformed))?;

    let collection_id = CollectionIdV1::new(wire.collection_id)
        .map_err(|_| CollectionControlError::new(CollectionControlKind::InvalidDeclaration))?;
    let input_root = wire.input_root.as_deref().map(safe_key).transpose()?;

    let mut sources = Vec::with_capacity(wire.sources.len());
    for source in wire.sources {
        let key = CollectionSourceKeyV1::new(source.key)
            .map_err(|_| CollectionControlError::new(CollectionControlKind::InvalidDeclaration))?;
        let path = safe_key(&source.path)?;
        let config = source.config.as_deref().map(safe_key).transpose()?;
        let expected_sha256 = source
            .expected_sha256
            .map(CollectionDigestPinV1::new)
            .transpose()
            .map_err(|_| CollectionControlError::new(CollectionControlKind::InvalidDeclaration))?;
        sources.push(CollectionSourceV1::new(key, path, config, expected_sha256));
    }

    let mut clips = Vec::with_capacity(wire.clips.len());
    for clip in wire.clips {
        let id = CollectionLogicalIdV1::new(clip.id)
            .map_err(|_| CollectionControlError::new(CollectionControlKind::InvalidDeclaration))?;
        let source = CollectionSourceKeyV1::new(clip.source)
            .map_err(|_| CollectionControlError::new(CollectionControlKind::InvalidDeclaration))?;
        clips.push(
            CollectionClipV1::new(id, source, clip.take_index, clip.take_name).map_err(|_| {
                CollectionControlError::new(CollectionControlKind::InvalidDeclaration)
            })?,
        );
    }

    let mut runtime_sets = Vec::with_capacity(wire.runtime_sets.len());
    for runtime_set in wire.runtime_sets {
        let id = CollectionLogicalIdV1::new(runtime_set.id)
            .map_err(|_| CollectionControlError::new(CollectionControlKind::InvalidDeclaration))?;
        let members = runtime_set
            .members
            .into_iter()
            .map(|member| {
                CollectionLogicalIdV1::new(member).map_err(|_| {
                    CollectionControlError::new(CollectionControlKind::InvalidDeclaration)
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        runtime_sets.push(CollectionRuntimeSetV1::new(id, runtime_set.kind, members));
    }

    CollectionManifestV1::new(collection_id, input_root, sources, clips, runtime_sets)
        .map_err(|error| CollectionControlError::new(classify_manifest_error(&error)))
}

fn safe_key(value: &str) -> Result<DependencyResourceKeyV1, CollectionControlError> {
    DependencyResourceKeyV1::from_source_str(value, ResourceKeySyntaxV1::ParserRelativePath)
        .map_err(|_| CollectionControlError::new(CollectionControlKind::InvalidDeclaration))
}

impl CollectionPathResolver {
    /// Establish the manifest directory and optional source input root.
    pub(crate) fn new(
        manifest_path: &Path,
        input_root: Option<&DependencyResourceKeyV1>,
    ) -> Result<Self, CollectionControlError> {
        let manifest_parent = manifest_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let control_root = fs::canonicalize(manifest_parent)
            .map_err(|_| CollectionControlError::new(CollectionControlKind::InvalidInputRoot))?;
        if !control_root.is_dir() {
            return Err(CollectionControlError::new(
                CollectionControlKind::InvalidInputRoot,
            ));
        }

        let source_root = match input_root {
            Some(declared) => match inspect_path(&control_root, declared.as_str()) {
                Ok(PathState::RegularOrDirectory {
                    canonical,
                    metadata,
                }) if metadata.is_dir() && canonical.starts_with(&control_root) => canonical,
                Ok(PathState::RegularOrDirectory { .. })
                | Ok(PathState::NonRegular)
                | Ok(PathState::Missing)
                | Ok(PathState::Unreadable) => {
                    return Err(CollectionControlError::new(
                        CollectionControlKind::InvalidInputRoot,
                    ));
                }
                Err(PathFailure::Unsafe) => {
                    return Err(CollectionControlError::new(
                        CollectionControlKind::UnsafePath,
                    ));
                }
            },
            None => control_root.clone(),
        };
        Ok(Self {
            control_root,
            source_root,
        })
    }

    /// Resolve every source and reject lexical or canonical aliases.
    pub(crate) fn resolve_sources(
        &self,
        sources: &[CollectionSourceV1],
    ) -> Result<BTreeMap<String, CollectionSourceResolution>, CollectionControlError> {
        let mut declared_paths = BTreeSet::new();
        for source in sources {
            if !declared_paths.insert(source.path().as_str().to_owned()) {
                return Err(CollectionControlError::new(
                    CollectionControlKind::DuplicateSourcePath,
                ));
            }
        }

        let mut resolved = BTreeMap::new();
        let mut canonical_sources = BTreeSet::new();
        for source in sources {
            let declared = source.path().as_str().to_owned();
            let resolution = match inspect_path(&self.source_root, &declared) {
                Ok(PathState::RegularOrDirectory {
                    canonical,
                    metadata,
                }) => {
                    if !metadata.is_file() {
                        return Err(CollectionControlError::new(
                            CollectionControlKind::SourceNonRegular,
                        ));
                    }
                    if !canonical.starts_with(&self.source_root) {
                        return Err(CollectionControlError::new(
                            CollectionControlKind::UnsafePath,
                        ));
                    }
                    if !canonical_sources.insert(canonical.clone()) {
                        return Err(CollectionControlError::new(
                            CollectionControlKind::CanonicalSourceAlias,
                        ));
                    }
                    CollectionSourceResolution::Ready(CollectionResolvedPath {
                        declared,
                        canonical,
                    })
                }
                Ok(PathState::Missing) => CollectionSourceResolution::Unavailable {
                    declared,
                    reason: CollectionSourceUnavailable::Missing,
                },
                Ok(PathState::Unreadable) => CollectionSourceResolution::Unavailable {
                    declared,
                    reason: CollectionSourceUnavailable::Unreadable,
                },
                Ok(PathState::NonRegular) => {
                    return Err(CollectionControlError::new(
                        CollectionControlKind::SourceNonRegular,
                    ));
                }
                Err(PathFailure::Unsafe) => {
                    return Err(CollectionControlError::new(
                        CollectionControlKind::UnsafePath,
                    ));
                }
            };
            resolved.insert(source.key().as_str().to_owned(), resolution);
        }
        Ok(resolved)
    }

    /// Resolve one explicit config path below the manifest directory.
    pub(crate) fn resolve_config(
        &self,
        declared: Option<&DependencyResourceKeyV1>,
    ) -> Result<CollectionConfigResolution, CollectionControlError> {
        let Some(declared) = declared else {
            return Ok(CollectionConfigResolution::Default);
        };
        match inspect_path(&self.control_root, declared.as_str()) {
            Ok(PathState::RegularOrDirectory {
                canonical,
                metadata,
            }) if metadata.is_file() => {
                if !canonical.starts_with(&self.control_root) {
                    return Err(CollectionControlError::new(
                        CollectionControlKind::UnsafePath,
                    ));
                }
                Ok(CollectionConfigResolution::Explicit(
                    CollectionResolvedPath {
                        declared: declared.as_str().to_owned(),
                        canonical,
                    },
                ))
            }
            Ok(PathState::RegularOrDirectory { .. }) | Ok(PathState::NonRegular) => Err(
                CollectionControlError::new(CollectionControlKind::ConfigNonRegular),
            ),
            Ok(PathState::Missing) => Err(CollectionControlError::new(
                CollectionControlKind::ConfigMissing,
            )),
            Ok(PathState::Unreadable) => Err(CollectionControlError::new(
                CollectionControlKind::ConfigUnreadable,
            )),
            Err(PathFailure::Unsafe) => Err(CollectionControlError::new(
                CollectionControlKind::UnsafePath,
            )),
        }
    }
}

fn classify_manifest_error(error: &CollectionManifestError) -> CollectionControlKind {
    match error {
        CollectionManifestError::UnsupportedSchema { .. } => {
            CollectionControlKind::UnsupportedSchema
        }
        CollectionManifestError::UnsupportedSchemaVersion { .. } => {
            CollectionControlKind::UnsupportedSchemaVersion
        }
        _ => CollectionControlKind::InvalidDeclaration,
    }
}

fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>, CollectionControlKind> {
    let file = fs::File::open(path).map_err(|_| CollectionControlKind::ManifestRead)?;
    let mut bytes = Vec::new();
    let mut limited = file.take(limit.saturating_add(1));
    limited
        .read_to_end(&mut bytes)
        .map_err(|_| CollectionControlKind::ManifestRead)?;
    if bytes.len() as u64 > limit {
        return Err(CollectionControlKind::ManifestTooLarge);
    }
    Ok(bytes)
}

#[derive(Debug, Deserialize)]
struct CollectionManifestHeaderWire {
    schema: String,
    schema_version: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CollectionManifestWire {
    schema: String,
    schema_version: u32,
    collection_id: String,
    #[serde(default)]
    input_root: Option<String>,
    #[serde(deserialize_with = "deserialize_sources")]
    sources: Vec<CollectionSourceWire>,
    #[serde(deserialize_with = "deserialize_clips")]
    clips: Vec<CollectionClipWire>,
    #[serde(default, deserialize_with = "deserialize_runtime_sets")]
    runtime_sets: Vec<CollectionRuntimeSetWire>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CollectionSourceWire {
    key: String,
    path: String,
    #[serde(default)]
    config: Option<String>,
    #[serde(default)]
    expected_sha256: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CollectionClipWire {
    id: String,
    source: String,
    take_index: u32,
    take_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CollectionRuntimeSetWire {
    id: String,
    kind: CollectionRuntimeSetKindV1,
    #[serde(deserialize_with = "deserialize_members")]
    members: Vec<String>,
}

fn deserialize_sources<'de, D>(deserializer: D) -> Result<Vec<CollectionSourceWire>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_capped(deserializer, COLLECTION_MANIFEST_V1_MAX_SOURCES, "sources")
}

fn deserialize_clips<'de, D>(deserializer: D) -> Result<Vec<CollectionClipWire>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_capped(deserializer, COLLECTION_MANIFEST_V1_MAX_CLIPS, "clips")
}

fn deserialize_runtime_sets<'de, D>(
    deserializer: D,
) -> Result<Vec<CollectionRuntimeSetWire>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_capped(
        deserializer,
        COLLECTION_MANIFEST_V1_MAX_RUNTIME_SETS,
        "runtime_sets",
    )
}

fn deserialize_members<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_capped(
        deserializer,
        COLLECTION_MANIFEST_V1_MAX_CLIPS,
        "runtime_sets.members",
    )
}

fn deserialize_capped<'de, D, T>(
    deserializer: D,
    limit: usize,
    field: &'static str,
) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    struct CappedVisitor<T> {
        limit: usize,
        field: &'static str,
        marker: std::marker::PhantomData<fn() -> T>,
    }
    impl<'de, T> Visitor<'de> for CappedVisitor<T>
    where
        T: serde::Deserialize<'de>,
    {
        type Value = Vec<T>;
        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(formatter, "at most {} {} rows", self.limit, self.field)
        }
        fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut values = Vec::with_capacity(sequence.size_hint().unwrap_or(0).min(self.limit));
            while values.len() < self.limit {
                let Some(value) = sequence.next_element()? else {
                    return Ok(values);
                };
                values.push(value);
            }
            if sequence.next_element::<IgnoredAny>()?.is_some() {
                while sequence.next_element::<IgnoredAny>()?.is_some() {}
                return Err(serde::de::Error::custom(format!(
                    "{} exceeds V1 limit {}",
                    self.field, self.limit
                )));
            }
            Ok(values)
        }
    }
    deserializer.deserialize_seq(CappedVisitor {
        limit,
        field,
        marker: std::marker::PhantomData,
    })
}

#[derive(Debug)]
enum PathState {
    RegularOrDirectory {
        canonical: PathBuf,
        metadata: fs::Metadata,
    },
    NonRegular,
    Missing,
    Unreadable,
}

#[derive(Debug)]
enum PathFailure {
    Unsafe,
}

fn inspect_path(root: &Path, declared: &str) -> Result<PathState, PathFailure> {
    let mut current = root.to_path_buf();
    let components: Vec<_> = Path::new(declared).components().collect();
    if components.is_empty() {
        return Err(PathFailure::Unsafe);
    }
    for (index, component) in components.iter().enumerate() {
        let Component::Normal(component) = component else {
            return Err(PathFailure::Unsafe);
        };
        current.push(component);
        let metadata = match fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(PathState::Missing),
            Err(_) => return Ok(PathState::Unreadable),
        };
        if metadata.file_type().is_symlink() {
            return Err(PathFailure::Unsafe);
        }
        if index + 1 < components.len() && !metadata.is_dir() {
            return Ok(PathState::Missing);
        }
        if index + 1 == components.len() {
            if !metadata.is_file() && !metadata.is_dir() {
                return Ok(PathState::NonRegular);
            }
            let canonical = match fs::canonicalize(&current) {
                Ok(canonical) => canonical,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    return Ok(PathState::Missing);
                }
                Err(_) => return Ok(PathState::Unreadable),
            };
            return Ok(PathState::RegularOrDirectory {
                canonical,
                metadata,
            });
        }
    }
    Err(PathFailure::Unsafe)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    const VALID: &str = r#"
schema = "urn:animsmith:schema:collection-manifest:1"
schema_version = 1
collection_id = "com.example.test"

[[sources]]
key = "walk"
path = "walk.gltf"

[[clips]]
id = "com.example.test/locomotion/walk"
source = "walk"
take_index = 0
take_name = "Take 001"
"#;

    fn key(value: &str) -> DependencyResourceKeyV1 {
        DependencyResourceKeyV1::from_source_str(value, ResourceKeySyntaxV1::ParserRelativePath)
            .unwrap()
    }

    fn source_key(value: &str) -> CollectionSourceKeyV1 {
        CollectionSourceKeyV1::new(value).unwrap()
    }

    #[test]
    fn parser_rejects_unknown_fields() {
        let unknown = VALID.replace("schema_version = 1", "schema_version = 1\nunknown = true");
        assert_eq!(
            parse_collection_manifest_bytes(unknown.as_bytes())
                .unwrap_err()
                .kind(),
            CollectionControlKind::ManifestMalformed
        );
    }

    #[test]
    fn schema_identity_and_version_are_checked_before_scalar_validation() {
        let unsupported_schema = VALID
            .replace(
                "urn:animsmith:schema:collection-manifest:1",
                "urn:example:future",
            )
            .replace(
                "collection_id = \"com.example.test\"",
                "collection_id = \"INVALID\"",
            );
        assert_eq!(
            parse_collection_manifest_bytes(unsupported_schema.as_bytes())
                .unwrap_err()
                .kind(),
            CollectionControlKind::UnsupportedSchema
        );

        let unsupported_version = VALID.replace("schema_version = 1", "schema_version = 2");
        assert_eq!(
            parse_collection_manifest_bytes(unsupported_version.as_bytes())
                .unwrap_err()
                .kind(),
            CollectionControlKind::UnsupportedSchemaVersion
        );
    }

    #[test]
    fn schema_header_wins_over_a_bounded_body_overflow() {
        let mut overflow = VALID.to_owned();
        for index in 1..=COLLECTION_MANIFEST_V1_MAX_CLIPS {
            overflow.push_str(&format!(
                "\n[[clips]]\nid = \"com.example.test/locomotion/walk-{index}\"\nsource = \"walk\"\ntake_index = {index}\ntake_name = \"Take {index:03}\"\n"
            ));
        }
        let unsupported_schema = overflow.replace(
            "urn:animsmith:schema:collection-manifest:1",
            "urn:example:future",
        );
        assert_eq!(
            parse_collection_manifest_bytes(unsupported_schema.as_bytes())
                .unwrap_err()
                .kind(),
            CollectionControlKind::UnsupportedSchema
        );

        let unsupported_version = overflow.replace("schema_version = 1", "schema_version = 2");
        assert_eq!(
            parse_collection_manifest_bytes(unsupported_version.as_bytes())
                .unwrap_err()
                .kind(),
            CollectionControlKind::UnsupportedSchemaVersion
        );
    }

    #[test]
    fn parser_rejects_n_plus_one_clips() {
        let mut exact = VALID.to_owned();
        for index in 1..COLLECTION_MANIFEST_V1_MAX_CLIPS {
            exact.push_str(&format!(
                "\n[[clips]]\nid = \"com.example.test/locomotion/walk-{index}\"\nsource = \"walk\"\ntake_index = {index}\ntake_name = \"Take {index:03}\"\n"
            ));
        }
        assert!(parse_collection_manifest_bytes(exact.as_bytes()).is_ok());

        let mut text = exact;
        let index = COLLECTION_MANIFEST_V1_MAX_CLIPS;
        text.push_str(&format!(
            "\n[[clips]]\nid = \"com.example.test/locomotion/walk-{index}\"\nsource = \"walk\"\ntake_index = {index}\ntake_name = \"Take {index:03}\"\n"
        ));
        assert_eq!(
            parse_collection_manifest_bytes(text.as_bytes())
                .unwrap_err()
                .kind(),
            CollectionControlKind::ManifestMalformed
        );
    }

    #[test]
    fn parser_rejects_n_plus_one_sources_runtime_sets_and_members() {
        let mut sources_exact = VALID.to_owned();
        for index in 1..COLLECTION_MANIFEST_V1_MAX_SOURCES {
            sources_exact.push_str(&format!(
                "\n[[sources]]\nkey = \"source-{index}\"\npath = \"source-{index}.gltf\"\n"
            ));
        }
        assert!(parse_collection_manifest_bytes(sources_exact.as_bytes()).is_ok());
        let mut sources = sources_exact;
        let index = COLLECTION_MANIFEST_V1_MAX_SOURCES;
        sources.push_str(&format!(
            "\n[[sources]]\nkey = \"source-{index}\"\npath = \"source-{index}.gltf\"\n"
        ));
        assert_eq!(
            parse_collection_manifest_bytes(sources.as_bytes())
                .unwrap_err()
                .kind(),
            CollectionControlKind::ManifestMalformed
        );

        let mut runtime_exact = VALID.to_owned();
        runtime_exact.push_str("\n[[clips]]\nid = \"com.example.test/locomotion/walk-2\"\nsource = \"walk\"\ntake_index = 1\ntake_name = \"Take 002\"\n");
        for index in 0..COLLECTION_MANIFEST_V1_MAX_RUNTIME_SETS {
            runtime_exact.push_str(&format!(
                "\n[[runtime_sets]]\nid = \"com.example.test/sets/set-{index}\"\nkind = \"gait-group\"\nmembers = [\"com.example.test/locomotion/walk\", \"com.example.test/locomotion/walk-2\"]\n"
            ));
        }
        assert!(parse_collection_manifest_bytes(runtime_exact.as_bytes()).is_ok());
        let mut runtime_sets = runtime_exact;
        let index = COLLECTION_MANIFEST_V1_MAX_RUNTIME_SETS;
        runtime_sets.push_str(&format!(
            "\n[[runtime_sets]]\nid = \"com.example.test/sets/set-{index}\"\nkind = \"gait-group\"\nmembers = [\"com.example.test/locomotion/walk\", \"com.example.test/locomotion/walk-2\"]\n"
        ));
        assert_eq!(
            parse_collection_manifest_bytes(runtime_sets.as_bytes())
                .unwrap_err()
                .kind(),
            CollectionControlKind::ManifestMalformed
        );

        let mut members_exact = VALID.to_owned();
        for index in 1..COLLECTION_MANIFEST_V1_MAX_CLIPS {
            members_exact.push_str(&format!(
                "\n[[clips]]\nid = \"com.example.test/locomotion/member-{index}\"\nsource = \"walk\"\ntake_index = {index}\ntake_name = \"Take {index:03}\"\n"
            ));
        }
        members_exact.push_str("\n[[runtime_sets]]\nid = \"com.example.test/sets/many\"\nkind = \"gait-group\"\nmembers = [");
        for index in 0..COLLECTION_MANIFEST_V1_MAX_CLIPS {
            if index != 0 {
                members_exact.push_str(", ");
            }
            let id = if index == 0 {
                "com.example.test/locomotion/walk".to_owned()
            } else {
                format!("com.example.test/locomotion/member-{index}")
            };
            members_exact.push_str(&format!("\"{id}\""));
        }
        members_exact.push_str("]\n");
        assert!(parse_collection_manifest_bytes(members_exact.as_bytes()).is_ok());

        let mut members = members_exact;
        members.push_str("\n[[runtime_sets]]\nid = \"com.example.test/sets/too-many\"\nkind = \"gait-group\"\nmembers = [");
        for index in 0..=COLLECTION_MANIFEST_V1_MAX_CLIPS {
            if index != 0 {
                members.push_str(", ");
            }
            let id = if index == 0 {
                "com.example.test/locomotion/walk".to_owned()
            } else {
                format!("com.example.test/locomotion/member-{index}")
            };
            members.push_str(&format!("\"{id}\""));
        }
        members.push_str("]\n");
        assert_eq!(
            parse_collection_manifest_bytes(members.as_bytes())
                .unwrap_err()
                .kind(),
            CollectionControlKind::ManifestMalformed
        );
    }

    #[test]
    fn lexical_windows_forms_are_refused_before_host_path_use() {
        for value in [
            "C:/assets/walk.gltf",
            "C:\\assets\\walk.gltf",
            "\\\\server\\share\\walk.gltf",
            "//server/share/walk.gltf",
            "file:walk.gltf",
            "https://example.test/walk.gltf",
            "walk/../walk.gltf",
            "walk\\walk.gltf",
        ] {
            assert!(
                DependencyResourceKeyV1::from_source_str(
                    value,
                    ResourceKeySyntaxV1::ParserRelativePath
                )
                .is_err(),
                "{value}"
            );
        }
    }

    #[test]
    fn resolver_distinguishes_missing_and_nonregular_sources_without_path_leakage() {
        let directory = tempfile::tempdir().unwrap();
        let manifest = directory.path().join("collection.toml");
        File::create(&manifest).unwrap();
        fs::write(directory.path().join("regular.gltf"), b"fixture").unwrap();
        fs::create_dir(directory.path().join("directory.gltf")).unwrap();
        let resolver = CollectionPathResolver::new(&manifest, None).unwrap();
        let sources = vec![
            CollectionSourceV1::new(source_key("missing"), key("missing.gltf"), None, None),
            CollectionSourceV1::new(source_key("regular"), key("regular.gltf"), None, None),
        ];
        let resolved = resolver.resolve_sources(&sources).unwrap();
        assert!(matches!(
            resolved["missing"],
            CollectionSourceResolution::Unavailable {
                reason: CollectionSourceUnavailable::Missing,
                ..
            }
        ));
        let CollectionSourceResolution::Ready(regular) = &resolved["regular"] else {
            panic!("regular source should resolve")
        };
        assert_eq!(regular.declared(), "regular.gltf");
        assert!(regular.path().is_file());

        let nonregular = vec![CollectionSourceV1::new(
            source_key("directory"),
            key("directory.gltf"),
            None,
            None,
        )];
        let error = resolver.resolve_sources(&nonregular).unwrap_err();
        assert_eq!(error.kind(), CollectionControlKind::SourceNonRegular);
        assert!(
            !error
                .to_string()
                .contains(&directory.path().display().to_string())
        );
    }

    #[test]
    fn resolver_uses_manifest_root_for_config_and_rejects_duplicate_source_paths() {
        let directory = tempfile::tempdir().unwrap();
        let manifest = directory.path().join("collection.toml");
        File::create(&manifest).unwrap();
        fs::create_dir(directory.path().join("assets")).unwrap();
        fs::write(directory.path().join("animsmith.toml"), b"# config\n").unwrap();
        fs::write(directory.path().join("assets/walk.gltf"), b"fixture").unwrap();
        let resolver = CollectionPathResolver::new(&manifest, Some(&key("assets"))).unwrap();
        assert!(matches!(
            resolver
                .resolve_config(Some(&key("animsmith.toml")))
                .unwrap(),
            CollectionConfigResolution::Explicit(_)
        ));

        let duplicate = vec![
            CollectionSourceV1::new(source_key("a"), key("walk.gltf"), None, None),
            CollectionSourceV1::new(source_key("b"), key("walk.gltf"), None, None),
        ];
        assert_eq!(
            resolver.resolve_sources(&duplicate).unwrap_err().kind(),
            CollectionControlKind::DuplicateSourcePath
        );
    }

    #[test]
    fn resolver_rejects_missing_and_nonregular_input_roots_and_configs() {
        let directory = tempfile::tempdir().unwrap();
        let manifest = directory.path().join("collection.toml");
        File::create(&manifest).unwrap();
        fs::write(directory.path().join("file"), b"fixture").unwrap();
        let missing_root = key("missing");
        assert_eq!(
            CollectionPathResolver::new(&manifest, Some(&missing_root))
                .unwrap_err()
                .kind(),
            CollectionControlKind::InvalidInputRoot
        );
        let file_root = key("file");
        assert_eq!(
            CollectionPathResolver::new(&manifest, Some(&file_root))
                .unwrap_err()
                .kind(),
            CollectionControlKind::InvalidInputRoot
        );

        let resolver = CollectionPathResolver::new(&manifest, None).unwrap();
        assert_eq!(
            resolver
                .resolve_config(Some(&key("missing.toml")))
                .unwrap_err()
                .kind(),
            CollectionControlKind::ConfigMissing
        );
        fs::create_dir(directory.path().join("config-dir")).unwrap();
        assert_eq!(
            resolver
                .resolve_config(Some(&key("config-dir")))
                .unwrap_err()
                .kind(),
            CollectionControlKind::ConfigNonRegular
        );
    }

    #[test]
    fn retained_spike_manifests_use_the_same_strict_parser() {
        let valid = include_bytes!("../testdata/collection-spike/collection.toml");
        assert!(parse_collection_manifest_bytes(valid).is_ok());
        for name in [
            "invalid-duplicate-member.toml",
            "invalid-missing-member.toml",
            "invalid-escaping-source.toml",
        ] {
            let path = format!("../testdata/collection-spike/{name}");
            let bytes = match name {
                "invalid-duplicate-member.toml" => {
                    include_bytes!("../testdata/collection-spike/invalid-duplicate-member.toml")
                        as &[u8]
                }
                "invalid-missing-member.toml" => {
                    include_bytes!("../testdata/collection-spike/invalid-missing-member.toml")
                        as &[u8]
                }
                _ => include_bytes!("../testdata/collection-spike/invalid-escaping-source.toml")
                    as &[u8],
            };
            assert!(
                parse_collection_manifest_bytes(bytes).is_err(),
                "retained fixture {path} should be rejected"
            );
        }
    }

    #[test]
    fn resolver_preserves_case_and_unicode_declared_locators() {
        let directory = tempfile::tempdir().unwrap();
        let manifest = directory.path().join("collection.toml");
        File::create(&manifest).unwrap();
        let declared = "Ässets/Walk Ü.gltf";
        let path = directory.path().join(declared);
        fs::create_dir(path.parent().unwrap()).unwrap();
        fs::write(&path, b"fixture").unwrap();
        let resolver = CollectionPathResolver::new(&manifest, None).unwrap();
        let sources = vec![CollectionSourceV1::new(
            source_key("unicode"),
            key(declared),
            None,
            None,
        )];
        let resolved = resolver.resolve_sources(&sources).unwrap();
        let CollectionSourceResolution::Ready(path) = &resolved["unicode"] else {
            panic!("unicode source should resolve")
        };
        assert_eq!(path.declared(), declared);
    }

    #[cfg(unix)]
    #[test]
    fn resolver_rejects_symlink_component_and_final_target() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let manifest = directory.path().join("collection.toml");
        File::create(&manifest).unwrap();
        fs::create_dir(directory.path().join("real")).unwrap();
        fs::write(directory.path().join("real/walk.gltf"), b"fixture").unwrap();
        symlink("real/walk.gltf", directory.path().join("final.gltf")).unwrap();
        symlink("real", directory.path().join("linked")).unwrap();
        let resolver = CollectionPathResolver::new(&manifest, None).unwrap();

        for (key_name, declared) in [("final", "final.gltf"), ("component", "linked/walk.gltf")] {
            let sources = vec![CollectionSourceV1::new(
                source_key(key_name),
                key(declared),
                None,
                None,
            )];
            assert_eq!(
                resolver.resolve_sources(&sources).unwrap_err().kind(),
                CollectionControlKind::UnsafePath
            );
        }
    }

    #[test]
    fn bounded_manifest_read_refuses_n_plus_one_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let exact_manifest = directory.path().join("exact.toml");
        let mut exact_file = File::create(&exact_manifest).unwrap();
        exact_file
            .write_all(&vec![
                b'x';
                COLLECTION_MANIFEST_V1_MAX_MANIFEST_BYTES as usize
            ])
            .unwrap();
        assert_eq!(
            read_bounded(&exact_manifest, COLLECTION_MANIFEST_V1_MAX_MANIFEST_BYTES)
                .unwrap()
                .len() as u64,
            COLLECTION_MANIFEST_V1_MAX_MANIFEST_BYTES
        );

        let oversized_manifest = directory.path().join("oversized.toml");
        let mut oversized_file = File::create(&oversized_manifest).unwrap();
        oversized_file
            .write_all(&vec![
                b'x';
                (COLLECTION_MANIFEST_V1_MAX_MANIFEST_BYTES + 1)
                    as usize
            ])
            .unwrap();
        assert_eq!(
            load_collection_manifest(&oversized_manifest)
                .unwrap_err()
                .kind(),
            CollectionControlKind::ManifestTooLarge
        );
    }
}
