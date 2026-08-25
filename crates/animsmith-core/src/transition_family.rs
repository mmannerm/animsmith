//! Format-neutral transition-family declaration V1 values.
//!
//! The CLI owns strict, bounded TOML decoding. This module owns the closed
//! declaration vocabulary, its validation, deterministic family ordering, and
//! the distinct exact-source and normalized-JCS identities. It deliberately
//! does not evaluate poses, load documents, or integrate the declaration into
//! [`crate::Config`].

use serde::Serialize;
use std::collections::BTreeSet;
use std::io::{self, Write};

use crate::{CollectionIdV1, CollectionLogicalIdV1, CollectionSourceKeyV1, InputIdentity};

/// Schema identity for a transition-family declaration.
pub const TRANSITION_FAMILY_V1_ID: &str = "urn:animsmith:schema:transition-family:1";
/// Schema version for a transition-family declaration.
pub const TRANSITION_FAMILY_V1_SCHEMA_VERSION: u32 = 1;
/// Maximum exact TOML source bytes accepted by V1.
pub const TRANSITION_FAMILY_V1_MAX_SOURCE_BYTES: u64 = 8 * 1024 * 1024;
/// Maximum normalized RFC 8785 JCS bytes accepted by V1.
pub const TRANSITION_FAMILY_V1_MAX_NORMALIZED_BYTES: u64 = 8 * 1024 * 1024;
/// Maximum table/array or JSON object/array depth, including the root.
pub const TRANSITION_FAMILY_V1_MAX_DEPTH: usize = 16;
/// Maximum family declarations in one owner.
pub const TRANSITION_FAMILY_V1_MAX_FAMILIES: usize = 4_096;
/// Maximum ordered members in one family.
pub const TRANSITION_FAMILY_V1_MAX_MEMBERS_PER_FAMILY: usize = 4_096;
/// Maximum ordered members across one declaration.
pub const TRANSITION_FAMILY_V1_MAX_AGGREGATE_MEMBERS: usize = 16_384;
/// Maximum UTF-8 bytes in an authored non-identifier string.
pub const TRANSITION_FAMILY_V1_MAX_STRING_BYTES: usize = 4_096;
/// Maximum UTF-8 bytes in a document-local family identifier.
pub const TRANSITION_FAMILY_V1_MAX_DOCUMENT_FAMILY_ID_BYTES: usize = 255;

/// Selects the endpoint boundary a later evaluator will compare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TransitionFamilyBoundaryV1 {
    /// Compare entry poses at normalized time zero.
    Entry,
    /// Compare exit poses at normalized time one.
    Exit,
    /// Compare both entry and exit poses.
    Both,
}

/// The fixed V1 skeleton-local basis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct TransitionFamilyBasisV1 {
    translation: &'static str,
    rotation: &'static str,
    time: &'static str,
}

impl TransitionFamilyBasisV1 {
    /// Construct the only basis accepted by transition-family V1.
    #[must_use]
    pub const fn skeleton_local() -> Self {
        Self {
            translation: "skeleton-local-metres",
            rotation: "skeleton-local-degrees",
            time: "normalized-clip",
        }
    }
}

/// Unit-bearing tolerances for a transition family.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct TransitionFamilyTolerancesV1 {
    translation_m: f64,
    rotation_deg: f64,
    time_normalized: f64,
}

impl TransitionFamilyTolerancesV1 {
    /// Construct finite, non-negative V1 tolerances.
    pub fn new(
        translation_m: f64,
        rotation_deg: f64,
        time_normalized: f64,
    ) -> Result<Self, TransitionFamilyError> {
        for (field, value) in [
            ("translation_m", translation_m),
            ("rotation_deg", rotation_deg),
            ("time_normalized", time_normalized),
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(TransitionFamilyError::InvalidTolerance { field });
            }
        }
        Ok(Self {
            translation_m,
            rotation_deg,
            time_normalized,
        })
    }

    /// Translation tolerance in skeleton-local metres.
    pub const fn translation_m(self) -> f64 {
        self.translation_m
    }
    /// Rotation tolerance in skeleton-local degrees.
    pub const fn rotation_deg(self) -> f64 {
        self.rotation_deg
    }
    /// Time tolerance in normalized clip time.
    pub const fn time_normalized(self) -> f64 {
        self.time_normalized
    }
}

/// One exact embedded take witness in a document-local family.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DocumentTransitionFamilyMemberV1 {
    take_index: u64,
    take_name: String,
}

impl DocumentTransitionFamilyMemberV1 {
    /// Construct one bounded, non-empty document take witness.
    pub fn new(take_index: u64, take_name: String) -> Result<Self, TransitionFamilyError> {
        validate_text("take_name", &take_name, false)?;
        Ok(Self {
            take_index,
            take_name,
        })
    }
    /// Exact embedded take index witness.
    pub const fn take_index(&self) -> u64 {
        self.take_index
    }
    /// Exact embedded take-name witness.
    pub fn take_name(&self) -> &str {
        &self.take_name
    }
}

/// One manifest-bound logical clip and source/take witness in a collection family.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CollectionTransitionFamilyMemberV1 {
    logical_id: CollectionLogicalIdV1,
    source: CollectionSourceKeyV1,
    take_index: u64,
    take_name: String,
}

impl CollectionTransitionFamilyMemberV1 {
    /// Construct one bounded collection member witness.
    pub fn new(
        logical_id: CollectionLogicalIdV1,
        source: CollectionSourceKeyV1,
        take_index: u64,
        take_name: String,
    ) -> Result<Self, TransitionFamilyError> {
        validate_text("take_name", &take_name, false)?;
        Ok(Self {
            logical_id,
            source,
            take_index,
            take_name,
        })
    }
    /// Manifest logical clip identity.
    pub fn logical_id(&self) -> &CollectionLogicalIdV1 {
        &self.logical_id
    }
    /// Manifest-local source identity.
    pub fn source(&self) -> &CollectionSourceKeyV1 {
        &self.source
    }
    /// Exact embedded take index witness.
    pub const fn take_index(&self) -> u64 {
        self.take_index
    }
    /// Exact embedded take-name witness.
    pub fn take_name(&self) -> &str {
        &self.take_name
    }
}

/// One document-owned transition family.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DocumentTransitionFamilyV1 {
    family_id: String,
    boundary: TransitionFamilyBoundaryV1,
    basis: TransitionFamilyBasisV1,
    tolerances: TransitionFamilyTolerancesV1,
    members: Vec<DocumentTransitionFamilyMemberV1>,
}

impl DocumentTransitionFamilyV1 {
    /// Construct one document family while preserving declared member order.
    pub fn new(
        family_id: String,
        boundary: TransitionFamilyBoundaryV1,
        tolerances: TransitionFamilyTolerancesV1,
        members: Vec<DocumentTransitionFamilyMemberV1>,
    ) -> Result<Self, TransitionFamilyError> {
        validate_document_family_id(&family_id)?;
        validate_document_members(&members)?;
        Ok(Self {
            family_id,
            boundary,
            basis: TransitionFamilyBasisV1::skeleton_local(),
            tolerances,
            members,
        })
    }
    /// Stable document-local family identifier.
    pub fn family_id(&self) -> &str {
        &self.family_id
    }
    /// Selected future evaluation boundary.
    pub const fn boundary(&self) -> TransitionFamilyBoundaryV1 {
        self.boundary
    }
    /// Fixed V1 coordinate basis.
    pub const fn basis(&self) -> TransitionFamilyBasisV1 {
        self.basis
    }
    /// Declared unit-bearing tolerances.
    pub const fn tolerances(&self) -> TransitionFamilyTolerancesV1 {
        self.tolerances
    }
    /// Members in exact declared order.
    pub fn members(&self) -> &[DocumentTransitionFamilyMemberV1] {
        &self.members
    }
}

/// One collection-owned transition family.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CollectionTransitionFamilyV1 {
    family_id: CollectionLogicalIdV1,
    boundary: TransitionFamilyBoundaryV1,
    basis: TransitionFamilyBasisV1,
    tolerances: TransitionFamilyTolerancesV1,
    members: Vec<CollectionTransitionFamilyMemberV1>,
}

impl CollectionTransitionFamilyV1 {
    /// Construct one collection family while preserving declared member order.
    pub fn new(
        family_id: CollectionLogicalIdV1,
        boundary: TransitionFamilyBoundaryV1,
        tolerances: TransitionFamilyTolerancesV1,
        members: Vec<CollectionTransitionFamilyMemberV1>,
    ) -> Result<Self, TransitionFamilyError> {
        validate_collection_members(&members)?;
        Ok(Self {
            family_id,
            boundary,
            basis: TransitionFamilyBasisV1::skeleton_local(),
            tolerances,
            members,
        })
    }
    /// Stable collection logical family identifier.
    pub fn family_id(&self) -> &CollectionLogicalIdV1 {
        &self.family_id
    }
    /// Selected future evaluation boundary.
    pub const fn boundary(&self) -> TransitionFamilyBoundaryV1 {
        self.boundary
    }
    /// Fixed V1 coordinate basis.
    pub const fn basis(&self) -> TransitionFamilyBasisV1 {
        self.basis
    }
    /// Declared unit-bearing tolerances.
    pub const fn tolerances(&self) -> TransitionFamilyTolerancesV1 {
        self.tolerances
    }
    /// Members in exact declared order.
    pub fn members(&self) -> &[CollectionTransitionFamilyMemberV1] {
        &self.members
    }
}

/// Exact collection-manifest binding carried by a collection declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TransitionFamilyManifestIdentityV1 {
    collection_id: CollectionIdV1,
    #[serde(rename = "manifest_input_identity")]
    input: InputIdentity,
}

impl TransitionFamilyManifestIdentityV1 {
    /// Construct a collection id and exact manifest-byte identity binding.
    pub fn new(
        collection_id: CollectionIdV1,
        input: InputIdentity,
    ) -> Result<Self, TransitionFamilyError> {
        if input.bytes() > crate::COLLECTION_MANIFEST_V1_MAX_MANIFEST_BYTES {
            return Err(TransitionFamilyError::ManifestTooLarge);
        }
        Ok(Self {
            collection_id,
            input,
        })
    }
    /// Bound collection identifier.
    pub fn collection_id(&self) -> &CollectionIdV1 {
        &self.collection_id
    }
    /// Bound exact manifest-byte identity.
    pub const fn input(&self) -> &InputIdentity {
        &self.input
    }
}

/// A fully validated document or collection declaration, without source bytes.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "scope", rename_all = "kebab-case")]
#[non_exhaustive]
pub enum TransitionFamilyDeclarationV1 {
    /// Families owned by one document config.
    Document {
        /// Fixed V1 schema identifier retained by the typed contract.
        #[serde(skip)]
        schema: &'static str,
        /// Fixed V1 schema version retained by the typed contract.
        #[serde(skip)]
        schema_version: u32,
        /// Families sorted by stable family id, with member order preserved.
        families: Vec<DocumentTransitionFamilyV1>,
    },
    /// Families bound to one collection manifest.
    Collection {
        /// Fixed V1 schema identifier retained by the typed contract.
        #[serde(skip)]
        schema: &'static str,
        /// Fixed V1 schema version retained by the typed contract.
        #[serde(skip)]
        schema_version: u32,
        /// Exact collection-manifest binding.
        #[serde(flatten)]
        manifest: TransitionFamilyManifestIdentityV1,
        /// Families sorted by stable family id, with member order preserved.
        families: Vec<CollectionTransitionFamilyV1>,
    },
}

impl TransitionFamilyDeclarationV1 {
    /// Construct a document declaration. An empty list is a valid no-family configuration.
    pub fn document(
        mut families: Vec<DocumentTransitionFamilyV1>,
    ) -> Result<Self, TransitionFamilyError> {
        validate_family_count(families.len())?;
        families.sort_by(|left, right| left.family_id.cmp(&right.family_id));
        if families
            .windows(2)
            .any(|pair| pair[0].family_id == pair[1].family_id)
        {
            return Err(TransitionFamilyError::DuplicateFamily);
        }
        validate_aggregate_document(&families)?;
        Ok(Self::Document {
            schema: TRANSITION_FAMILY_V1_ID,
            schema_version: TRANSITION_FAMILY_V1_SCHEMA_VERSION,
            families,
        })
    }

    /// Construct a non-empty collection declaration.
    pub fn collection(
        manifest: TransitionFamilyManifestIdentityV1,
        mut families: Vec<CollectionTransitionFamilyV1>,
    ) -> Result<Self, TransitionFamilyError> {
        if families.is_empty() {
            return Err(TransitionFamilyError::EmptyCollection);
        }
        validate_family_count(families.len())?;
        let owner_prefix = format!("{}/", manifest.collection_id.as_str());
        if families
            .iter()
            .any(|family| !family.family_id.as_str().starts_with(&owner_prefix))
        {
            return Err(TransitionFamilyError::CollectionFamilyOutsideOwner);
        }
        if families
            .iter()
            .flat_map(|family| family.members.iter())
            .any(|member| !member.logical_id.as_str().starts_with(&owner_prefix))
        {
            return Err(TransitionFamilyError::CollectionMemberOutsideOwner);
        }
        families.sort_by(|left, right| left.family_id.cmp(&right.family_id));
        if families
            .windows(2)
            .any(|pair| pair[0].family_id == pair[1].family_id)
        {
            return Err(TransitionFamilyError::DuplicateFamily);
        }
        validate_aggregate_collection(&families)?;
        Ok(Self::Collection {
            schema: TRANSITION_FAMILY_V1_ID,
            schema_version: TRANSITION_FAMILY_V1_SCHEMA_VERSION,
            manifest,
            families,
        })
    }

    /// Canonically serialize this closed declaration as bounded RFC 8785 JCS bytes.
    pub fn normalized_jcs(&self) -> Result<Vec<u8>, TransitionFamilyError> {
        // Revalidate here too: enum construction remains possible in this
        // crate, and a public serialization boundary must never bless a
        // forged, non-canonical declaration with a stable digest.
        self.clone()
            .canonicalize()?
            .normalized_jcs_bounded(TRANSITION_FAMILY_V1_MAX_NORMALIZED_BYTES as usize)
    }

    /// Document families, when this is a document declaration.
    pub fn document_families(&self) -> Option<&[DocumentTransitionFamilyV1]> {
        match self {
            Self::Document { families, .. } => Some(families),
            Self::Collection { .. } => None,
        }
    }

    /// Collection families, when this is a collection declaration.
    pub fn collection_families(&self) -> Option<&[CollectionTransitionFamilyV1]> {
        match self {
            Self::Document { .. } => None,
            Self::Collection { families, .. } => Some(families),
        }
    }

    fn normalized_jcs_bounded(&self, maximum: usize) -> Result<Vec<u8>, TransitionFamilyError> {
        let mut output = BoundedWriter::new(maximum);
        let wire = match self {
            Self::Document { families, .. } => TransitionFamilyNormalizedWire::Document {
                schema: TRANSITION_FAMILY_V1_ID,
                schema_version: TRANSITION_FAMILY_V1_SCHEMA_VERSION,
                scope: "document",
                families,
            },
            Self::Collection {
                manifest, families, ..
            } => TransitionFamilyNormalizedWire::Collection {
                schema: TRANSITION_FAMILY_V1_ID,
                schema_version: TRANSITION_FAMILY_V1_SCHEMA_VERSION,
                scope: "collection",
                collection_id: manifest.collection_id(),
                manifest_input_identity: manifest.input(),
                families,
            },
        };
        serde_jcs::to_writer(&mut output, &wire)
            .map_err(|_| TransitionFamilyError::NormalizedTooLarge)?;
        Ok(output.into_inner())
    }

    fn canonicalize(self) -> Result<Self, TransitionFamilyError> {
        match self {
            Self::Document { families, .. } => Self::document(families),
            Self::Collection {
                manifest, families, ..
            } => Self::collection(manifest, families),
        }
    }
}

#[derive(Serialize)]
#[serde(untagged)]
enum TransitionFamilyNormalizedWire<'a> {
    Document {
        schema: &'static str,
        schema_version: u32,
        scope: &'static str,
        families: &'a [DocumentTransitionFamilyV1],
    },
    Collection {
        schema: &'static str,
        schema_version: u32,
        scope: &'static str,
        collection_id: &'a CollectionIdV1,
        manifest_input_identity: &'a InputIdentity,
        families: &'a [CollectionTransitionFamilyV1],
    },
}

/// Declaration plus the exact source identity and independently normalized identity.
#[derive(Debug, Clone, PartialEq)]
pub struct TransitionFamilyDeclarationInputV1 {
    declaration: TransitionFamilyDeclarationV1,
    source_identity: InputIdentity,
    normalized_identity: InputIdentity,
}

impl TransitionFamilyDeclarationInputV1 {
    /// Bind an already validated declaration to exact source bytes and bounded JCS identity.
    pub fn new(
        declaration: TransitionFamilyDeclarationV1,
        source: &[u8],
    ) -> Result<Self, TransitionFamilyError> {
        if source.len() as u64 > TRANSITION_FAMILY_V1_MAX_SOURCE_BYTES {
            return Err(TransitionFamilyError::SourceTooLarge);
        }
        // `TransitionFamilyDeclarationV1` is non-exhaustive to external
        // callers, but this identity boundary also replays the constructors
        // so any internal/deserialization route cannot preserve an invalid or
        // non-canonical declaration under a new source identity.
        let declaration = declaration.canonicalize()?;
        let normalized_identity = InputIdentity::from_bytes(&declaration.normalized_jcs()?);
        Ok(Self {
            declaration,
            source_identity: InputIdentity::from_bytes(source),
            normalized_identity,
        })
    }
    /// Validated typed declaration.
    pub fn declaration(&self) -> &TransitionFamilyDeclarationV1 {
        &self.declaration
    }
    /// Exact TOML source-byte identity.
    pub fn source_identity(&self) -> &InputIdentity {
        &self.source_identity
    }
    /// Independently normalized RFC 8785 JCS identity.
    pub fn normalized_identity(&self) -> &InputIdentity {
        &self.normalized_identity
    }
}

/// Typed validation failure for transition-family V1 declarations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum TransitionFamilyError {
    /// The exact source exceeded the V1 byte cap.
    #[error("transition-family source exceeds the V1 byte cap")]
    SourceTooLarge,
    /// The normalized JCS representation exceeded the V1 byte cap.
    #[error("transition-family normalized declaration exceeds the V1 byte cap")]
    NormalizedTooLarge,
    /// An authored field was malformed, empty, or exceeded its byte cap.
    #[error("invalid transition-family {field}")]
    InvalidText {
        /// Closed field label whose text was invalid.
        field: &'static str,
    },
    /// A document family id did not meet the V1 grammar.
    #[error("invalid document transition-family identifier")]
    InvalidDocumentFamilyId,
    /// A tolerance was non-finite or negative.
    #[error("invalid transition-family tolerance {field}")]
    InvalidTolerance {
        /// Closed tolerance label whose number was invalid.
        field: &'static str,
    },
    /// One family did not have at least two members.
    #[error("transition family requires at least two members")]
    TooFewMembers,
    /// One family exceeded its member cap.
    #[error("transition family exceeds the member cap")]
    TooManyMembers,
    /// The declaration exceeded its family cap.
    #[error("transition declaration exceeds the family cap")]
    TooManyFamilies,
    /// The declaration exceeded its aggregate member cap.
    #[error("transition declaration exceeds the aggregate member cap")]
    TooManyAggregateMembers,
    /// A member witness appeared more than once in a family.
    #[error("duplicate transition-family member")]
    DuplicateMember,
    /// A family id appeared more than once in an owner.
    #[error("duplicate transition-family family id")]
    DuplicateFamily,
    /// A collection envelope must contain at least one family.
    #[error("collection transition declaration requires at least one family")]
    EmptyCollection,
    /// A collection family id was outside its bound collection namespace.
    #[error("collection transition-family id is outside its collection owner")]
    CollectionFamilyOutsideOwner,
    /// A collection member logical id was outside its bound collection namespace.
    #[error("collection transition-family member is outside its collection owner")]
    CollectionMemberOutsideOwner,
    /// The embedded manifest byte identity exceeds the manifest cap.
    #[error("bound collection manifest is too large")]
    ManifestTooLarge,
}

fn validate_text(
    field: &'static str,
    value: &str,
    allow_empty: bool,
) -> Result<(), TransitionFamilyError> {
    if (!allow_empty && value.is_empty()) || value.len() > TRANSITION_FAMILY_V1_MAX_STRING_BYTES {
        return Err(TransitionFamilyError::InvalidText { field });
    }
    Ok(())
}

fn validate_document_family_id(value: &str) -> Result<(), TransitionFamilyError> {
    if value.is_empty() || value.len() > TRANSITION_FAMILY_V1_MAX_DOCUMENT_FAMILY_ID_BYTES {
        return Err(TransitionFamilyError::InvalidDocumentFamilyId);
    }
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return Err(TransitionFamilyError::InvalidDocumentFamilyId);
    };
    if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
        return Err(TransitionFamilyError::InvalidDocumentFamilyId);
    }
    if bytes.any(|byte| {
        !(byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-'))
    }) {
        return Err(TransitionFamilyError::InvalidDocumentFamilyId);
    }
    Ok(())
}

fn validate_document_members(
    members: &[DocumentTransitionFamilyMemberV1],
) -> Result<(), TransitionFamilyError> {
    validate_member_count(members.len())?;
    let mut seen = BTreeSet::new();
    for member in members {
        if !seen.insert((member.take_index, member.take_name.as_str())) {
            return Err(TransitionFamilyError::DuplicateMember);
        }
    }
    Ok(())
}

fn validate_collection_members(
    members: &[CollectionTransitionFamilyMemberV1],
) -> Result<(), TransitionFamilyError> {
    validate_member_count(members.len())?;
    let mut seen = BTreeSet::new();
    for member in members {
        if !seen.insert(member.logical_id.clone()) {
            return Err(TransitionFamilyError::DuplicateMember);
        }
    }
    Ok(())
}

fn validate_member_count(count: usize) -> Result<(), TransitionFamilyError> {
    if count < 2 {
        return Err(TransitionFamilyError::TooFewMembers);
    }
    if count > TRANSITION_FAMILY_V1_MAX_MEMBERS_PER_FAMILY {
        return Err(TransitionFamilyError::TooManyMembers);
    }
    Ok(())
}

fn validate_family_count(count: usize) -> Result<(), TransitionFamilyError> {
    if count > TRANSITION_FAMILY_V1_MAX_FAMILIES {
        Err(TransitionFamilyError::TooManyFamilies)
    } else {
        Ok(())
    }
}

fn validate_aggregate_document(
    families: &[DocumentTransitionFamilyV1],
) -> Result<(), TransitionFamilyError> {
    if families
        .iter()
        .map(|family| family.members.len())
        .sum::<usize>()
        > TRANSITION_FAMILY_V1_MAX_AGGREGATE_MEMBERS
    {
        Err(TransitionFamilyError::TooManyAggregateMembers)
    } else {
        Ok(())
    }
}

fn validate_aggregate_collection(
    families: &[CollectionTransitionFamilyV1],
) -> Result<(), TransitionFamilyError> {
    if families
        .iter()
        .map(|family| family.members.len())
        .sum::<usize>()
        > TRANSITION_FAMILY_V1_MAX_AGGREGATE_MEMBERS
    {
        Err(TransitionFamilyError::TooManyAggregateMembers)
    } else {
        Ok(())
    }
}

struct BoundedWriter {
    bytes: Vec<u8>,
    maximum: usize,
}
impl BoundedWriter {
    fn new(maximum: usize) -> Self {
        Self {
            bytes: Vec::new(),
            maximum,
        }
    }
    fn into_inner(self) -> Vec<u8> {
        self.bytes
    }
}
impl Write for BoundedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let total = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .ok_or_else(|| io::Error::other("normalized transition declaration too large"))?;
        if total > self.maximum {
            return Err(io::Error::other(
                "normalized transition declaration too large",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tolerances() -> TransitionFamilyTolerancesV1 {
        TransitionFamilyTolerancesV1::new(0.05, 5.0, 0.0).unwrap()
    }
    fn family(id: &str, names: &[&str]) -> DocumentTransitionFamilyV1 {
        DocumentTransitionFamilyV1::new(
            id.into(),
            TransitionFamilyBoundaryV1::Both,
            tolerances(),
            names
                .iter()
                .enumerate()
                .map(|(index, name)| {
                    DocumentTransitionFamilyMemberV1::new(index as u64, (*name).into()).unwrap()
                })
                .collect(),
        )
        .unwrap()
    }

    #[test]
    fn document_sorting_preserves_declared_member_order_and_distinct_identities() {
        let declaration = TransitionFamilyDeclarationV1::document(vec![
            family("z", &["Z", "A"]),
            family("a", &["B", "C"]),
        ])
        .unwrap();
        let TransitionFamilyDeclarationV1::Document { families, .. } = &declaration else {
            panic!("document declaration")
        };
        assert_eq!(
            families
                .iter()
                .map(|family| family.family_id())
                .collect::<Vec<_>>(),
            ["a", "z"]
        );
        assert_eq!(
            families[1]
                .members()
                .iter()
                .map(|member| member.take_name())
                .collect::<Vec<_>>(),
            ["Z", "A"]
        );
        let input = TransitionFamilyDeclarationInputV1::new(declaration, b"[source]").unwrap();
        assert_ne!(input.source_identity(), input.normalized_identity());
    }

    #[test]
    fn rejects_invalid_tolerances_identifiers_and_duplicate_members() {
        assert!(TransitionFamilyTolerancesV1::new(f64::NAN, 0.0, 0.0).is_err());
        assert_eq!(
            TransitionFamilyTolerancesV1::new(-0.01, 0.0, 0.0),
            Err(TransitionFamilyError::InvalidTolerance {
                field: "translation_m"
            })
        );
        assert!(
            DocumentTransitionFamilyV1::new(
                "Upper".into(),
                TransitionFamilyBoundaryV1::Entry,
                tolerances(),
                vec![]
            )
            .is_err()
        );
        assert!(
            DocumentTransitionFamilyV1::new(
                "ok".into(),
                TransitionFamilyBoundaryV1::Entry,
                tolerances(),
                vec![
                    DocumentTransitionFamilyMemberV1::new(0, "same".into()).unwrap(),
                    DocumentTransitionFamilyMemberV1::new(0, "same".into()).unwrap()
                ]
            )
            .is_err()
        );
        assert_eq!(
            DocumentTransitionFamilyV1::new(
                "ok".into(),
                TransitionFamilyBoundaryV1::Entry,
                tolerances(),
                vec![DocumentTransitionFamilyMemberV1::new(0, "one".into()).unwrap()]
            ),
            Err(TransitionFamilyError::TooFewMembers)
        );
        assert!(
            DocumentTransitionFamilyV1::new(
                "bad/slash".into(),
                TransitionFamilyBoundaryV1::Entry,
                tolerances(),
                vec![
                    DocumentTransitionFamilyMemberV1::new(0, "one".into()).unwrap(),
                    DocumentTransitionFamilyMemberV1::new(1, "two".into()).unwrap(),
                ]
            )
            .is_err()
        );
        assert!(
            DocumentTransitionFamilyV1::new(
                "tuple".into(),
                TransitionFamilyBoundaryV1::Entry,
                tolerances(),
                vec![
                    DocumentTransitionFamilyMemberV1::new(0, "same-name".into()).unwrap(),
                    DocumentTransitionFamilyMemberV1::new(1, "same-name".into()).unwrap(),
                ]
            )
            .is_ok()
        );
    }

    #[test]
    fn collection_families_and_members_must_share_the_manifest_namespace() {
        let manifest = TransitionFamilyManifestIdentityV1::new(
            CollectionIdV1::new("com.example").unwrap(),
            InputIdentity::from_bytes(b"manifest"),
        )
        .unwrap();
        let members = |first: &str, second: &str| {
            vec![
                CollectionTransitionFamilyMemberV1::new(
                    CollectionLogicalIdV1::new(first).unwrap(),
                    CollectionSourceKeyV1::new("source").unwrap(),
                    0,
                    "one".into(),
                )
                .unwrap(),
                CollectionTransitionFamilyMemberV1::new(
                    CollectionLogicalIdV1::new(second).unwrap(),
                    CollectionSourceKeyV1::new("source").unwrap(),
                    1,
                    "two".into(),
                )
                .unwrap(),
            ]
        };
        let family = |members| {
            CollectionTransitionFamilyV1::new(
                CollectionLogicalIdV1::new("com.example/family").unwrap(),
                TransitionFamilyBoundaryV1::Entry,
                tolerances(),
                members,
            )
            .unwrap()
        };
        let outside_family = CollectionTransitionFamilyV1::new(
            CollectionLogicalIdV1::new("other.example/family").unwrap(),
            TransitionFamilyBoundaryV1::Entry,
            tolerances(),
            members("com.example/one", "com.example/two"),
        )
        .unwrap();
        assert_eq!(
            TransitionFamilyDeclarationV1::collection(manifest.clone(), vec![outside_family]),
            Err(TransitionFamilyError::CollectionFamilyOutsideOwner)
        );
        assert_eq!(
            TransitionFamilyDeclarationV1::collection(
                manifest.clone(),
                vec![family(members("com.example/one", "other.example/two"))]
            ),
            Err(TransitionFamilyError::CollectionMemberOutsideOwner)
        );
        assert!(
            TransitionFamilyDeclarationV1::collection(
                manifest,
                vec![family(members("com.example/one", "com.example/two"))]
            )
            .is_ok()
        );
    }

    #[test]
    fn direct_identity_and_manifest_bounds_refuse_first_excess() {
        let declaration =
            TransitionFamilyDeclarationV1::document(vec![family("ok", &["A", "B"])]).unwrap();
        assert_eq!(
            TransitionFamilyDeclarationInputV1::new(
                declaration,
                &vec![b'x'; TRANSITION_FAMILY_V1_MAX_SOURCE_BYTES as usize + 1]
            ),
            Err(TransitionFamilyError::SourceTooLarge)
        );
        assert_eq!(
            TransitionFamilyManifestIdentityV1::new(
                CollectionIdV1::new("com.example").unwrap(),
                InputIdentity::from_sha256_digest(
                    [0; 32],
                    crate::COLLECTION_MANIFEST_V1_MAX_MANIFEST_BYTES + 1
                )
            ),
            Err(TransitionFamilyError::ManifestTooLarge)
        );
    }

    #[test]
    fn limits_are_inclusive_and_the_first_aggregate_member_over_is_rejected() {
        let exact_member_count = (0..TRANSITION_FAMILY_V1_MAX_MEMBERS_PER_FAMILY)
            .map(|index| {
                DocumentTransitionFamilyMemberV1::new(index as u64, format!("take-{index}"))
                    .unwrap()
            })
            .collect();
        assert!(
            DocumentTransitionFamilyV1::new(
                "exact".into(),
                TransitionFamilyBoundaryV1::Entry,
                tolerances(),
                exact_member_count,
            )
            .is_ok()
        );

        let family = |id: String, members| {
            DocumentTransitionFamilyV1::new(
                id,
                TransitionFamilyBoundaryV1::Entry,
                tolerances(),
                (0..members)
                    .map(|index| {
                        DocumentTransitionFamilyMemberV1::new(index as u64, format!("{index}"))
                            .unwrap()
                    })
                    .collect(),
            )
            .unwrap()
        };
        let mut families = (0..4)
            .map(|index| family(format!("family-{index}"), 4_096))
            .collect::<Vec<_>>();
        families.push(family("family-over".into(), 2));
        assert_eq!(
            TransitionFamilyDeclarationV1::document(families),
            Err(TransitionFamilyError::TooManyAggregateMembers)
        );
    }

    #[test]
    fn normalized_jcs_is_exact_and_uses_the_bounded_writer_seam() {
        let declaration =
            TransitionFamilyDeclarationV1::document(vec![family("walk", &["Walk", "Run"])])
                .unwrap();
        assert_eq!(
            String::from_utf8(declaration.normalized_jcs().unwrap()).unwrap(),
            "{\"families\":[{\"basis\":{\"rotation\":\"skeleton-local-degrees\",\"time\":\"normalized-clip\",\"translation\":\"skeleton-local-metres\"},\"boundary\":\"both\",\"family_id\":\"walk\",\"members\":[{\"take_index\":0,\"take_name\":\"Walk\"},{\"take_index\":1,\"take_name\":\"Run\"}],\"tolerances\":{\"rotation_deg\":5,\"time_normalized\":0,\"translation_m\":0.05}}],\"schema\":\"urn:animsmith:schema:transition-family:1\",\"schema_version\":1,\"scope\":\"document\"}"
        );
        assert_eq!(
            declaration.normalized_jcs_bounded(1),
            Err(TransitionFamilyError::NormalizedTooLarge)
        );
        let input = TransitionFamilyDeclarationInputV1::new(declaration, b"exact source").unwrap();
        assert_eq!(input.normalized_identity().bytes(), 408);
        assert_eq!(
            input.normalized_identity().sha256(),
            "f9b6b85b4fa066324b6c9bbe2ea0fe1ff6c217b3175ef964054b08c03b327cd0"
        );
    }

    #[test]
    fn source_identity_boundary_revalidates_internal_declaration_construction() {
        let invalid = TransitionFamilyDeclarationV1::Document {
            schema: "forged",
            schema_version: 99,
            families: vec![family("same", &["A", "B"]), family("same", &["C", "D"])],
        };
        assert_eq!(
            invalid.normalized_jcs(),
            Err(TransitionFamilyError::DuplicateFamily)
        );
        assert_eq!(
            TransitionFamilyDeclarationInputV1::new(invalid, b"source"),
            Err(TransitionFamilyError::DuplicateFamily)
        );
        let noncanonical = TransitionFamilyDeclarationV1::Document {
            schema: "forged",
            schema_version: 99,
            families: vec![family("z", &["Z", "A"]), family("a", &["B", "C"])],
        };
        let bound = TransitionFamilyDeclarationInputV1::new(noncanonical, b"source").unwrap();
        assert_eq!(
            bound
                .declaration()
                .document_families()
                .unwrap()
                .iter()
                .map(|family| family.family_id())
                .collect::<Vec<_>>(),
            ["a", "z"]
        );
    }

    #[test]
    fn collection_sorting_preserves_declared_member_order() {
        let member = |id: &str, take_name: &str| {
            CollectionTransitionFamilyMemberV1::new(
                CollectionLogicalIdV1::new(id).unwrap(),
                CollectionSourceKeyV1::new("source").unwrap(),
                0,
                take_name.into(),
            )
            .unwrap()
        };
        let family = |id: &str, members| {
            CollectionTransitionFamilyV1::new(
                CollectionLogicalIdV1::new(id).unwrap(),
                TransitionFamilyBoundaryV1::Entry,
                tolerances(),
                members,
            )
            .unwrap()
        };
        let manifest = TransitionFamilyManifestIdentityV1::new(
            CollectionIdV1::new("com.example").unwrap(),
            InputIdentity::from_bytes(b"manifest"),
        )
        .unwrap();
        let declaration = TransitionFamilyDeclarationV1::collection(
            manifest,
            vec![
                family(
                    "com.example/z",
                    vec![
                        member("com.example/z-first", "Z"),
                        member("com.example/z-last", "A"),
                    ],
                ),
                family(
                    "com.example/a",
                    vec![
                        member("com.example/a-first", "B"),
                        member("com.example/a-last", "C"),
                    ],
                ),
            ],
        )
        .unwrap();
        let TransitionFamilyDeclarationV1::Collection { families, .. } = declaration else {
            panic!("collection")
        };
        assert_eq!(families[0].family_id().as_str(), "com.example/a");
        assert_eq!(families[1].members()[0].take_name(), "Z");
        assert_eq!(families[1].members()[1].take_name(), "A");
    }
}
