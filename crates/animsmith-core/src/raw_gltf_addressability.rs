//! Immutable, bounded raw glTF scene, node, skin, attachment, and path evidence.
//!
//! The normalized [`crate::Document`] is intentionally not authority for these
//! source-array identities. Format loaders construct this sidecar during the
//! same load and bind it to both the exact primary bytes and the complete
//! bounded [`crate::DependencyClosureV1`] record they observed.

use crate::bounded_deserialize::{
    CappedSequence, deserialize_capped_option_string, deserialize_capped_sequence,
};
use crate::{DependencyClosureV1, InputIdentity};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeSet;
use std::io::Read;

/// Immutable raw glTF addressability inventory contract identity.
pub const RAW_GLTF_ADDRESSABILITY_INVENTORY_V1_ID: &str =
    "urn:animsmith:raw-gltf-addressability-inventory:1";
/// Maximum retained rows in each independent addressability domain.
pub const RAW_GLTF_ADDRESSABILITY_V1_MAX_ROWS_PER_DOMAIN: usize = 4_096;
/// Maximum aggregate structural index references retained by one inventory.
pub const RAW_GLTF_ADDRESSABILITY_V1_MAX_STRUCTURAL_REFERENCES: usize = 65_536;
/// Maximum UTF-8 bytes in one retained source name.
pub const RAW_GLTF_ADDRESSABILITY_V1_MAX_NAME_BYTES: usize = 1_024;
/// Maximum node-index segments in one retained scene path candidate.
pub const RAW_GLTF_ADDRESSABILITY_V1_MAX_PATH_SEGMENTS: usize = 256;
/// Maximum UTF-8 bytes in one slash-delimited authored-or-fallback path.
pub const RAW_GLTF_ADDRESSABILITY_V1_MAX_PATH_BYTES: usize = 4_096;
/// Maximum aggregate UTF-8 bytes retained in source names.
pub const RAW_GLTF_ADDRESSABILITY_V1_MAX_TEXT_BYTES: usize = 1024 * 1024;
/// Maximum serialized inventory bytes accepted by [`RawGltfAddressabilityInventoryV1::read_from`].
pub const RAW_GLTF_ADDRESSABILITY_V1_MAX_READER_BYTES: u64 = 256 * 1024 * 1024;

fn deserialize_name<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_capped_option_string(deserializer, RAW_GLTF_ADDRESSABILITY_V1_MAX_NAME_BYTES)
}

fn deserialize_rows<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    let rows: CappedSequence<T> =
        deserialize_capped_sequence(deserializer, RAW_GLTF_ADDRESSABILITY_V1_MAX_ROWS_PER_DOMAIN)?;
    if rows.overflowed {
        return Err(D::Error::custom(
            "raw glTF addressability domain exceeded its row bound",
        ));
    }
    Ok(rows.values)
}

fn deserialize_scene_rows<'de, D>(deserializer: D) -> Result<Vec<RawGltfSceneRowV1>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_rows(deserializer)
}

fn deserialize_node_rows<'de, D>(deserializer: D) -> Result<Vec<RawGltfNodeRowV1>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_rows(deserializer)
}

fn deserialize_skin_rows<'de, D>(deserializer: D) -> Result<Vec<RawGltfSkinRowV1>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_rows(deserializer)
}

fn deserialize_attachment_rows<'de, D>(
    deserializer: D,
) -> Result<Vec<RawGltfSkinAttachmentRowV1>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_rows(deserializer)
}

fn deserialize_path_rows<'de, D>(
    deserializer: D,
) -> Result<Vec<RawGltfScenePathCandidateRowV1>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_rows(deserializer)
}

fn deserialize_structural_references<'de, D>(deserializer: D) -> Result<Vec<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    let references: CappedSequence<u64> = deserialize_capped_sequence(
        deserializer,
        RAW_GLTF_ADDRESSABILITY_V1_MAX_STRUCTURAL_REFERENCES,
    )?;
    if references.overflowed {
        return Err(D::Error::custom(
            "raw glTF addressability row exceeded its structural-reference bound",
        ));
    }
    Ok(references.values)
}

fn deserialize_path_segments<'de, D>(deserializer: D) -> Result<Vec<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    let segments: CappedSequence<u64> =
        deserialize_capped_sequence(deserializer, RAW_GLTF_ADDRESSABILITY_V1_MAX_PATH_SEGMENTS)?;
    if segments.overflowed {
        return Err(D::Error::custom(
            "raw glTF addressability path exceeded its segment bound",
        ));
    }
    Ok(segments.values)
}

/// Terminal reason for incomplete raw glTF projection coverage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RawGltfAddressabilityCoverageReasonV1 {
    /// A V1 row, reference, name, or aggregate text ceiling was exceeded.
    ProjectionBudgetExceeded,
    /// The loader could not observe this domain through its parser.
    ParserUnavailable,
}

/// Independent exhaustive/prefix/unavailable state for one row domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum RawGltfAddressabilityCoverageV1 {
    /// Every source row is retained; empty proves absence.
    Complete,
    /// The retained rows are a canonical source-order prefix only.
    Partial {
        /// Why projection stopped after the retained prefix.
        reason: RawGltfAddressabilityCoverageReasonV1,
    },
    /// No positive source rows are authoritative for this domain.
    Unavailable {
        /// Why the domain could not be projected.
        reason: RawGltfAddressabilityCoverageReasonV1,
    },
}

impl RawGltfAddressabilityCoverageV1 {
    /// Canonical projection-budget partial state.
    pub const fn budget_exceeded() -> Self {
        Self::Partial {
            reason: RawGltfAddressabilityCoverageReasonV1::ProjectionBudgetExceeded,
        }
    }

    /// Whether an empty row set proves source absence.
    pub const fn proves_absence(self) -> bool {
        matches!(self, Self::Complete)
    }

    /// Whether the domain is exhaustive.
    pub const fn is_complete(self) -> bool {
        matches!(self, Self::Complete)
    }
}

/// Exact observation of the optional top-level glTF `scene` member.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum RawGltfDefaultSceneObservationV1 {
    /// The member was absent. No scene is selected by default.
    Absent,
    /// The member selected one existing source scene index.
    Selected {
        /// Exact source scene-array index.
        source_scene_index: u64,
    },
    /// The parser could not observe the member.
    Unavailable {
        /// Why no exact observation is present.
        reason: RawGltfAddressabilityCoverageReasonV1,
    },
}

impl RawGltfDefaultSceneObservationV1 {
    /// Selected source scene index, if one was explicitly observed.
    pub const fn selected_scene_index(self) -> Option<u64> {
        match self {
            Self::Selected { source_scene_index } => Some(source_scene_index),
            Self::Absent | Self::Unavailable { .. } => None,
        }
    }
}

/// Exact source observation of a skin's optional inverse-bind accessor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum RawGltfInverseBindMatricesObservationV1 {
    /// No accessor was declared; glTF's identity fallback applies.
    Absent,
    /// An accessor was explicitly declared.
    Declared {
        /// Exact source accessor-array index.
        source_accessor_index: u64,
    },
}

/// One source scene and its declared roots in authored order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawGltfSceneRowV1 {
    source_scene_index: u64,
    #[serde(deserialize_with = "deserialize_name")]
    name: Option<String>,
    #[serde(deserialize_with = "deserialize_structural_references")]
    root_node_indices: Vec<u64>,
}

impl RawGltfSceneRowV1 {
    /// Construct one source scene row.
    pub fn new(source_scene_index: u64, name: Option<String>, root_node_indices: Vec<u64>) -> Self {
        Self {
            source_scene_index,
            name,
            root_node_indices,
        }
    }
    /// Exact source scene-array index.
    pub const fn source_scene_index(&self) -> u64 {
        self.source_scene_index
    }
    /// Optional authored scene name.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
    /// Root node identities in authored order.
    pub fn root_node_indices(&self) -> &[u64] {
        &self.root_node_indices
    }
}

/// One source node with exact parent and authored child order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawGltfNodeRowV1 {
    source_node_index: u64,
    #[serde(deserialize_with = "deserialize_name")]
    name: Option<String>,
    parent_node_index: Option<u64>,
    #[serde(deserialize_with = "deserialize_structural_references")]
    child_node_indices: Vec<u64>,
}

impl RawGltfNodeRowV1 {
    /// Construct one source node row.
    pub fn new(
        source_node_index: u64,
        name: Option<String>,
        parent_node_index: Option<u64>,
        child_node_indices: Vec<u64>,
    ) -> Self {
        Self {
            source_node_index,
            name,
            parent_node_index,
            child_node_indices,
        }
    }
    /// Exact source node-array index.
    pub const fn source_node_index(&self) -> u64 {
        self.source_node_index
    }
    /// Optional authored node name.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
    /// Exact source parent, absent for a forest root.
    pub const fn parent_node_index(&self) -> Option<u64> {
        self.parent_node_index
    }
    /// Child node identities in authored order.
    pub fn child_node_indices(&self) -> &[u64] {
        &self.child_node_indices
    }
}

/// One source skin with exact joints and optional authored skeleton root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawGltfSkinRowV1 {
    source_skin_index: u64,
    #[serde(deserialize_with = "deserialize_name")]
    name: Option<String>,
    #[serde(deserialize_with = "deserialize_structural_references")]
    joint_node_indices: Vec<u64>,
    skeleton_root_node_index: Option<u64>,
    inverse_bind_matrices: RawGltfInverseBindMatricesObservationV1,
}

impl RawGltfSkinRowV1 {
    /// Construct one source skin row.
    pub fn new(
        source_skin_index: u64,
        name: Option<String>,
        joint_node_indices: Vec<u64>,
        skeleton_root_node_index: Option<u64>,
        inverse_bind_matrices: RawGltfInverseBindMatricesObservationV1,
    ) -> Self {
        Self {
            source_skin_index,
            name,
            joint_node_indices,
            skeleton_root_node_index,
            inverse_bind_matrices,
        }
    }
    /// Exact source skin-array index.
    pub const fn source_skin_index(&self) -> u64 {
        self.source_skin_index
    }
    /// Optional authored skin name.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
    /// Joint node identities in authored order.
    pub fn joint_node_indices(&self) -> &[u64] {
        &self.joint_node_indices
    }
    /// Explicit source `skin.skeleton`, without any inferred-root claim.
    pub const fn skeleton_root_node_index(&self) -> Option<u64> {
        self.skeleton_root_node_index
    }
    /// Exact optional inverse-bind accessor observation.
    pub const fn inverse_bind_matrices(&self) -> RawGltfInverseBindMatricesObservationV1 {
        self.inverse_bind_matrices
    }
}

/// One source node-to-skin reference, in source node order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawGltfSkinAttachmentRowV1 {
    source_node_index: u64,
    source_skin_index: u64,
}

impl RawGltfSkinAttachmentRowV1 {
    /// Construct one source node-to-skin reference.
    pub const fn new(source_node_index: u64, source_skin_index: u64) -> Self {
        Self {
            source_node_index,
            source_skin_index,
        }
    }
    /// Exact source node-array index.
    pub const fn source_node_index(&self) -> u64 {
        self.source_node_index
    }
    /// Exact referenced source skin-array index.
    pub const fn source_skin_index(&self) -> u64 {
        self.source_skin_index
    }
}

/// One scene-root-to-node candidate path in deterministic scene DFS order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawGltfScenePathCandidateRowV1 {
    source_path_candidate_index: u64,
    source_scene_index: u64,
    #[serde(deserialize_with = "deserialize_path_segments")]
    source_node_indices: Vec<u64>,
}

impl RawGltfScenePathCandidateRowV1 {
    /// Construct one exact source-index path candidate.
    pub fn new(
        source_path_candidate_index: u64,
        source_scene_index: u64,
        source_node_indices: Vec<u64>,
    ) -> Self {
        Self {
            source_path_candidate_index,
            source_scene_index,
            source_node_indices,
        }
    }
    /// Canonical candidate ordinal across all scenes.
    pub const fn source_path_candidate_index(&self) -> u64 {
        self.source_path_candidate_index
    }
    /// Exact source scene-array index.
    pub const fn source_scene_index(&self) -> u64 {
        self.source_scene_index
    }
    /// Root-to-node source identities, inclusive and in traversal order.
    pub fn source_node_indices(&self) -> &[u64] {
        &self.source_node_indices
    }
    /// Target node at the end of this candidate path.
    pub fn target_node_index(&self) -> Option<u64> {
        self.source_node_indices.last().copied()
    }
}

/// Public validated constructor input for one raw glTF inventory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawGltfAddressabilityInventoryInputV1 {
    /// Exact observation of the optional top-level default-scene selector.
    pub default_scene: RawGltfDefaultSceneObservationV1,
    /// Independent source scene coverage.
    pub scene_coverage: RawGltfAddressabilityCoverageV1,
    /// Canonical source scene prefix.
    pub scenes: Vec<RawGltfSceneRowV1>,
    /// Independent source node coverage.
    pub node_coverage: RawGltfAddressabilityCoverageV1,
    /// Canonical source node prefix.
    pub nodes: Vec<RawGltfNodeRowV1>,
    /// Independent source skin coverage.
    pub skin_coverage: RawGltfAddressabilityCoverageV1,
    /// Canonical source skin prefix.
    pub skins: Vec<RawGltfSkinRowV1>,
    /// Independent node-to-skin attachment coverage.
    pub attachment_coverage: RawGltfAddressabilityCoverageV1,
    /// Canonical source-node-order attachment prefix.
    pub attachments: Vec<RawGltfSkinAttachmentRowV1>,
    /// Independent all-scene path-candidate coverage.
    pub path_candidate_coverage: RawGltfAddressabilityCoverageV1,
    /// Canonical scene/root/child traversal prefix.
    pub path_candidates: Vec<RawGltfScenePathCandidateRowV1>,
}

/// Strict, canonical raw glTF addressability sidecar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RawGltfAddressabilityInventoryV1 {
    schema: &'static str,
    identity: InputIdentity,
    primary_input: InputIdentity,
    dependency_closure: DependencyClosureV1,
    default_scene: RawGltfDefaultSceneObservationV1,
    scene_coverage: RawGltfAddressabilityCoverageV1,
    scenes: Vec<RawGltfSceneRowV1>,
    node_coverage: RawGltfAddressabilityCoverageV1,
    nodes: Vec<RawGltfNodeRowV1>,
    skin_coverage: RawGltfAddressabilityCoverageV1,
    skins: Vec<RawGltfSkinRowV1>,
    attachment_coverage: RawGltfAddressabilityCoverageV1,
    attachments: Vec<RawGltfSkinAttachmentRowV1>,
    path_candidate_coverage: RawGltfAddressabilityCoverageV1,
    path_candidates: Vec<RawGltfScenePathCandidateRowV1>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawGltfAddressabilityInventoryWireV1 {
    schema: String,
    identity: InputIdentity,
    primary_input: InputIdentity,
    dependency_closure: DependencyClosureV1,
    default_scene: RawGltfDefaultSceneObservationV1,
    scene_coverage: RawGltfAddressabilityCoverageV1,
    #[serde(deserialize_with = "deserialize_scene_rows")]
    scenes: Vec<RawGltfSceneRowV1>,
    node_coverage: RawGltfAddressabilityCoverageV1,
    #[serde(deserialize_with = "deserialize_node_rows")]
    nodes: Vec<RawGltfNodeRowV1>,
    skin_coverage: RawGltfAddressabilityCoverageV1,
    #[serde(deserialize_with = "deserialize_skin_rows")]
    skins: Vec<RawGltfSkinRowV1>,
    attachment_coverage: RawGltfAddressabilityCoverageV1,
    #[serde(deserialize_with = "deserialize_attachment_rows")]
    attachments: Vec<RawGltfSkinAttachmentRowV1>,
    path_candidate_coverage: RawGltfAddressabilityCoverageV1,
    #[serde(deserialize_with = "deserialize_path_rows")]
    path_candidates: Vec<RawGltfScenePathCandidateRowV1>,
}

impl<'de> Deserialize<'de> for RawGltfAddressabilityInventoryV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = RawGltfAddressabilityInventoryWireV1::deserialize(deserializer)?;
        if wire.schema != RAW_GLTF_ADDRESSABILITY_INVENTORY_V1_ID {
            return Err(D::Error::custom("invalid raw glTF addressability schema"));
        }
        let value = Self {
            schema: RAW_GLTF_ADDRESSABILITY_INVENTORY_V1_ID,
            identity: wire.identity,
            primary_input: wire.primary_input,
            dependency_closure: wire.dependency_closure,
            default_scene: wire.default_scene,
            scene_coverage: wire.scene_coverage,
            scenes: wire.scenes,
            node_coverage: wire.node_coverage,
            nodes: wire.nodes,
            skin_coverage: wire.skin_coverage,
            skins: wire.skins,
            attachment_coverage: wire.attachment_coverage,
            attachments: wire.attachments,
            path_candidate_coverage: wire.path_candidate_coverage,
            path_candidates: wire.path_candidates,
        };
        value.validate().map_err(D::Error::custom)?;
        if value.identity != value.canonical_identity().map_err(D::Error::custom)? {
            return Err(D::Error::custom(
                "raw glTF addressability identity does not match its contents",
            ));
        }
        Ok(value)
    }
}

impl RawGltfAddressabilityInventoryV1 {
    /// Construct and validate one exact same-load inventory.
    ///
    /// # Errors
    ///
    /// Returns [`RawGltfAddressabilityInventoryErrorV1`] for mismatched
    /// provenance, noncanonical prefixes, contradictory coverage, or a V1
    /// collection, structural-reference, or text bound violation.
    pub fn new(
        primary_input: InputIdentity,
        dependency_closure: DependencyClosureV1,
        input: RawGltfAddressabilityInventoryInputV1,
    ) -> Result<Self, RawGltfAddressabilityInventoryErrorV1> {
        let mut value = Self {
            schema: RAW_GLTF_ADDRESSABILITY_INVENTORY_V1_ID,
            identity: InputIdentity::from_bytes(&[]),
            primary_input,
            dependency_closure,
            default_scene: input.default_scene,
            scene_coverage: input.scene_coverage,
            scenes: input.scenes,
            node_coverage: input.node_coverage,
            nodes: input.nodes,
            skin_coverage: input.skin_coverage,
            skins: input.skins,
            attachment_coverage: input.attachment_coverage,
            attachments: input.attachments,
            path_candidate_coverage: input.path_candidate_coverage,
            path_candidates: input.path_candidates,
        };
        value.validate()?;
        value.identity = value.canonical_identity()?;
        Ok(value)
    }

    /// Read one strict inventory through the immutable 256 MiB byte cap.
    ///
    /// # Errors
    ///
    /// Returns a typed I/O, N+1 size, JSON-shape, or semantic contract error.
    pub fn read_from(reader: impl Read) -> Result<Self, RawGltfAddressabilityInventoryReadErrorV1> {
        Self::read_from_with_limit(reader, RAW_GLTF_ADDRESSABILITY_V1_MAX_READER_BYTES)
    }

    fn read_from_with_limit(
        reader: impl Read,
        limit: u64,
    ) -> Result<Self, RawGltfAddressabilityInventoryReadErrorV1> {
        let mut bounded = reader.take(limit + 1);
        let mut bytes = Vec::new();
        bounded
            .read_to_end(&mut bytes)
            .map_err(|source| RawGltfAddressabilityInventoryReadErrorV1::Io { source })?;
        if bytes.len() as u64 > limit {
            return Err(RawGltfAddressabilityInventoryReadErrorV1::InventoryTooLarge { limit });
        }
        serde_json::from_slice(&bytes)
            .map_err(|source| RawGltfAddressabilityInventoryReadErrorV1::InvalidJson { source })
    }

    /// Semantic inventory identifier.
    pub const fn contract_id(&self) -> &'static str {
        self.schema
    }
    /// Canonical identity over exact primary, closure, coverage, and rows.
    pub const fn identity(&self) -> &InputIdentity {
        &self.identity
    }
    /// Exact primary input identity.
    pub const fn primary_input(&self) -> &InputIdentity {
        &self.primary_input
    }
    /// Exact dependency-closure V1 record from the same loader invocation.
    pub const fn dependency_closure(&self) -> &DependencyClosureV1 {
        &self.dependency_closure
    }
    /// Optional top-level default-scene observation.
    pub const fn default_scene(&self) -> RawGltfDefaultSceneObservationV1 {
        self.default_scene
    }
    /// Source scene coverage.
    pub const fn scene_coverage(&self) -> RawGltfAddressabilityCoverageV1 {
        self.scene_coverage
    }
    /// Canonical source scene prefix.
    pub fn scenes(&self) -> &[RawGltfSceneRowV1] {
        &self.scenes
    }
    /// Source node coverage.
    pub const fn node_coverage(&self) -> RawGltfAddressabilityCoverageV1 {
        self.node_coverage
    }
    /// Canonical source node prefix.
    pub fn nodes(&self) -> &[RawGltfNodeRowV1] {
        &self.nodes
    }
    /// Source skin coverage.
    pub const fn skin_coverage(&self) -> RawGltfAddressabilityCoverageV1 {
        self.skin_coverage
    }
    /// Canonical source skin prefix.
    pub fn skins(&self) -> &[RawGltfSkinRowV1] {
        &self.skins
    }
    /// Source node-to-skin attachment coverage.
    pub const fn attachment_coverage(&self) -> RawGltfAddressabilityCoverageV1 {
        self.attachment_coverage
    }
    /// Canonical source-node-order attachment prefix.
    pub fn attachments(&self) -> &[RawGltfSkinAttachmentRowV1] {
        &self.attachments
    }
    /// All-scene path-candidate coverage.
    pub const fn path_candidate_coverage(&self) -> RawGltfAddressabilityCoverageV1 {
        self.path_candidate_coverage
    }
    /// Canonical scene/root/child traversal prefix.
    pub fn path_candidates(&self) -> &[RawGltfScenePathCandidateRowV1] {
        &self.path_candidates
    }

    /// Validate this inventory without changing its identity.
    pub fn validate(&self) -> Result<(), RawGltfAddressabilityInventoryErrorV1> {
        if self.schema != RAW_GLTF_ADDRESSABILITY_INVENTORY_V1_ID {
            return Err(RawGltfAddressabilityInventoryErrorV1::InvalidSchema);
        }
        if self.dependency_closure.primary_input() != &self.primary_input {
            return Err(RawGltfAddressabilityInventoryErrorV1::DependencyClosureMismatch);
        }
        validate_domain("scenes", self.scene_coverage, self.scenes.len())?;
        validate_domain("nodes", self.node_coverage, self.nodes.len())?;
        validate_domain("skins", self.skin_coverage, self.skins.len())?;
        validate_domain(
            "attachments",
            self.attachment_coverage,
            self.attachments.len(),
        )?;
        validate_domain(
            "path_candidates",
            self.path_candidate_coverage,
            self.path_candidates.len(),
        )?;

        for (expected, row) in self.scenes.iter().enumerate() {
            if row.source_scene_index != expected as u64 {
                return Err(RawGltfAddressabilityInventoryErrorV1::NonCanonicalRows {
                    domain: "scenes",
                });
            }
        }
        for (expected, row) in self.nodes.iter().enumerate() {
            if row.source_node_index != expected as u64 {
                return Err(RawGltfAddressabilityInventoryErrorV1::NonCanonicalRows {
                    domain: "nodes",
                });
            }
        }
        for (expected, row) in self.skins.iter().enumerate() {
            if row.source_skin_index != expected as u64 {
                return Err(RawGltfAddressabilityInventoryErrorV1::NonCanonicalRows {
                    domain: "skins",
                });
            }
        }
        if self
            .attachments
            .windows(2)
            .any(|rows| rows[0].source_node_index >= rows[1].source_node_index)
        {
            return Err(RawGltfAddressabilityInventoryErrorV1::NonCanonicalRows {
                domain: "attachments",
            });
        }
        for (expected, row) in self.path_candidates.iter().enumerate() {
            if row.source_path_candidate_index != expected as u64
                || row.source_node_indices.is_empty()
            {
                return Err(RawGltfAddressabilityInventoryErrorV1::NonCanonicalRows {
                    domain: "path_candidates",
                });
            }
            if row.source_node_indices.len() > RAW_GLTF_ADDRESSABILITY_V1_MAX_PATH_SEGMENTS {
                return Err(RawGltfAddressabilityInventoryErrorV1::TooManyPathSegments);
            }
        }

        let mut text_bytes = 0usize;
        for name in self
            .scenes
            .iter()
            .filter_map(|row| row.name.as_deref())
            .chain(self.nodes.iter().filter_map(|row| row.name.as_deref()))
            .chain(self.skins.iter().filter_map(|row| row.name.as_deref()))
        {
            if name.len() > RAW_GLTF_ADDRESSABILITY_V1_MAX_NAME_BYTES {
                return Err(RawGltfAddressabilityInventoryErrorV1::NameTooLong);
            }
            text_bytes = text_bytes
                .checked_add(name.len())
                .ok_or(RawGltfAddressabilityInventoryErrorV1::TooMuchText)?;
        }
        if text_bytes > RAW_GLTF_ADDRESSABILITY_V1_MAX_TEXT_BYTES {
            return Err(RawGltfAddressabilityInventoryErrorV1::TooMuchText);
        }
        for path in &self.path_candidates {
            let mut projected_bytes = 0usize;
            let mut observable = true;
            for (position, &node_index) in path.source_node_indices.iter().enumerate() {
                let Some(node) = usize::try_from(node_index)
                    .ok()
                    .and_then(|index| self.nodes.get(index))
                else {
                    observable = false;
                    break;
                };
                if node.source_node_index != node_index {
                    observable = false;
                    break;
                }
                let segment_bytes = node
                    .name
                    .as_ref()
                    .map_or_else(|| format!("GltfNode{node_index}").len(), String::len);
                projected_bytes = projected_bytes
                    .saturating_add(usize::from(position > 0))
                    .saturating_add(segment_bytes);
            }
            if observable && projected_bytes > RAW_GLTF_ADDRESSABILITY_V1_MAX_PATH_BYTES {
                return Err(RawGltfAddressabilityInventoryErrorV1::ProjectedPathTooLong);
            }
        }

        let mut references = usize::from(matches!(
            self.default_scene,
            RawGltfDefaultSceneObservationV1::Selected { .. }
        ));
        for row in &self.scenes {
            references = add_references(references, row.root_node_indices.len())?;
        }
        for row in &self.nodes {
            references = add_references(
                references,
                row.child_node_indices.len() + usize::from(row.parent_node_index.is_some()),
            )?;
        }
        for row in &self.skins {
            references = add_references(
                references,
                row.joint_node_indices.len()
                    + usize::from(row.skeleton_root_node_index.is_some())
                    + usize::from(matches!(
                        row.inverse_bind_matrices,
                        RawGltfInverseBindMatricesObservationV1::Declared { .. }
                    )),
            )?;
        }
        references = add_references(references, self.attachments.len().saturating_mul(2))?;
        for row in &self.path_candidates {
            references = add_references(references, 1 + row.source_node_indices.len())?;
        }
        if references > RAW_GLTF_ADDRESSABILITY_V1_MAX_STRUCTURAL_REFERENCES {
            return Err(RawGltfAddressabilityInventoryErrorV1::TooManyStructuralReferences);
        }

        self.validate_references()?;
        Ok(())
    }

    fn validate_references(&self) -> Result<(), RawGltfAddressabilityInventoryErrorV1> {
        if self.scene_coverage.is_complete()
            && let RawGltfDefaultSceneObservationV1::Selected { source_scene_index } =
                self.default_scene
            && source_scene_index >= self.scenes.len() as u64
        {
            return Err(RawGltfAddressabilityInventoryErrorV1::ReferenceOutOfRange);
        }
        if self.node_coverage.is_complete() {
            let node_count = self.nodes.len() as u64;
            for scene in &self.scenes {
                if scene
                    .root_node_indices
                    .iter()
                    .any(|&node| node >= node_count)
                {
                    return Err(RawGltfAddressabilityInventoryErrorV1::ReferenceOutOfRange);
                }
            }
            for node in &self.nodes {
                let mut unique_children = BTreeSet::new();
                if node
                    .parent_node_index
                    .is_some_and(|parent| parent >= node_count)
                    || node
                        .child_node_indices
                        .iter()
                        .any(|&child| child >= node_count || !unique_children.insert(child))
                {
                    return Err(RawGltfAddressabilityInventoryErrorV1::ReferenceOutOfRange);
                }
                for &child in &node.child_node_indices {
                    if self.nodes[child as usize].parent_node_index != Some(node.source_node_index)
                    {
                        return Err(RawGltfAddressabilityInventoryErrorV1::InvalidHierarchy);
                    }
                }
                if let Some(parent) = node.parent_node_index
                    && !self.nodes[parent as usize]
                        .child_node_indices
                        .contains(&node.source_node_index)
                {
                    return Err(RawGltfAddressabilityInventoryErrorV1::InvalidHierarchy);
                }
            }
            for node in &self.nodes {
                let mut current = Some(node.source_node_index);
                for _ in 0..self.nodes.len() {
                    let Some(index) = current else {
                        break;
                    };
                    current = self.nodes[index as usize].parent_node_index;
                }
                if current.is_some() {
                    return Err(RawGltfAddressabilityInventoryErrorV1::InvalidHierarchy);
                }
            }
            for skin in &self.skins {
                if skin
                    .joint_node_indices
                    .iter()
                    .any(|&node| node >= node_count)
                    || skin
                        .skeleton_root_node_index
                        .is_some_and(|node| node >= node_count)
                {
                    return Err(RawGltfAddressabilityInventoryErrorV1::ReferenceOutOfRange);
                }
            }
        }
        if self.node_coverage.is_complete() && self.skin_coverage.is_complete() {
            for attachment in &self.attachments {
                if attachment.source_node_index >= self.nodes.len() as u64
                    || attachment.source_skin_index >= self.skins.len() as u64
                {
                    return Err(RawGltfAddressabilityInventoryErrorV1::ReferenceOutOfRange);
                }
            }
        }
        for path in &self.path_candidates {
            if self.scene_coverage.is_complete()
                && path.source_scene_index >= self.scenes.len() as u64
            {
                return Err(RawGltfAddressabilityInventoryErrorV1::ReferenceOutOfRange);
            }
            if self.node_coverage.is_complete()
                && path
                    .source_node_indices
                    .iter()
                    .any(|&node| node >= self.nodes.len() as u64)
            {
                return Err(RawGltfAddressabilityInventoryErrorV1::ReferenceOutOfRange);
            }
            if self.scene_coverage.is_complete() && self.node_coverage.is_complete() {
                let scene = &self.scenes[path.source_scene_index as usize];
                if !scene
                    .root_node_indices
                    .contains(&path.source_node_indices[0])
                    || path.source_node_indices.windows(2).any(|pair| {
                        !self.nodes[pair[0] as usize]
                            .child_node_indices
                            .contains(&pair[1])
                    })
                {
                    return Err(RawGltfAddressabilityInventoryErrorV1::InvalidPathCandidate);
                }
            }
        }
        if self.scene_coverage.is_complete() && self.node_coverage.is_complete() {
            self.validate_canonical_path_prefix()?;
        }
        Ok(())
    }

    fn validate_canonical_path_prefix(&self) -> Result<(), RawGltfAddressabilityInventoryErrorV1> {
        let mut expected_index = 0usize;
        'scenes: for scene in &self.scenes {
            let mut stack = scene
                .root_node_indices
                .iter()
                .rev()
                .map(|&root| vec![root])
                .collect::<Vec<_>>();
            while let Some(path) = stack.pop() {
                if expected_index == self.path_candidates.len() {
                    if self.path_candidate_coverage.is_complete() {
                        return Err(RawGltfAddressabilityInventoryErrorV1::NonCanonicalRows {
                            domain: "path_candidates",
                        });
                    }
                    break 'scenes;
                }
                let expected = &self.path_candidates[expected_index];
                if expected.source_scene_index != scene.source_scene_index
                    || expected.source_node_indices != path
                {
                    return Err(RawGltfAddressabilityInventoryErrorV1::NonCanonicalRows {
                        domain: "path_candidates",
                    });
                }
                expected_index += 1;
                let target = *path.last().expect("canonical paths are nonempty");
                for &child in self.nodes[target as usize].child_node_indices.iter().rev() {
                    let mut child_path = path.clone();
                    child_path.push(child);
                    stack.push(child_path);
                }
            }
        }
        if expected_index != self.path_candidates.len() {
            return Err(RawGltfAddressabilityInventoryErrorV1::NonCanonicalRows {
                domain: "path_candidates",
            });
        }
        Ok(())
    }

    fn canonical_identity(&self) -> Result<InputIdentity, RawGltfAddressabilityInventoryErrorV1> {
        #[derive(Serialize)]
        struct IdentityFields<'a> {
            schema: &'static str,
            primary_input: &'a InputIdentity,
            dependency_closure: &'a DependencyClosureV1,
            default_scene: RawGltfDefaultSceneObservationV1,
            scene_coverage: RawGltfAddressabilityCoverageV1,
            scenes: &'a [RawGltfSceneRowV1],
            node_coverage: RawGltfAddressabilityCoverageV1,
            nodes: &'a [RawGltfNodeRowV1],
            skin_coverage: RawGltfAddressabilityCoverageV1,
            skins: &'a [RawGltfSkinRowV1],
            attachment_coverage: RawGltfAddressabilityCoverageV1,
            attachments: &'a [RawGltfSkinAttachmentRowV1],
            path_candidate_coverage: RawGltfAddressabilityCoverageV1,
            path_candidates: &'a [RawGltfScenePathCandidateRowV1],
        }
        let bytes = serde_json::to_vec(&IdentityFields {
            schema: RAW_GLTF_ADDRESSABILITY_INVENTORY_V1_ID,
            primary_input: &self.primary_input,
            dependency_closure: &self.dependency_closure,
            default_scene: self.default_scene,
            scene_coverage: self.scene_coverage,
            scenes: &self.scenes,
            node_coverage: self.node_coverage,
            nodes: &self.nodes,
            skin_coverage: self.skin_coverage,
            skins: &self.skins,
            attachment_coverage: self.attachment_coverage,
            attachments: &self.attachments,
            path_candidate_coverage: self.path_candidate_coverage,
            path_candidates: &self.path_candidates,
        })
        .map_err(|_| RawGltfAddressabilityInventoryErrorV1::IdentityEncoding)?;
        Ok(InputIdentity::from_bytes(&bytes))
    }
}

fn validate_domain(
    domain: &'static str,
    coverage: RawGltfAddressabilityCoverageV1,
    rows: usize,
) -> Result<(), RawGltfAddressabilityInventoryErrorV1> {
    if rows > RAW_GLTF_ADDRESSABILITY_V1_MAX_ROWS_PER_DOMAIN {
        return Err(RawGltfAddressabilityInventoryErrorV1::TooManyRows { domain });
    }
    if matches!(
        coverage,
        RawGltfAddressabilityCoverageV1::Unavailable { .. }
    ) && rows != 0
    {
        return Err(RawGltfAddressabilityInventoryErrorV1::UnavailableHasRows { domain });
    }
    Ok(())
}

fn add_references(
    current: usize,
    additional: usize,
) -> Result<usize, RawGltfAddressabilityInventoryErrorV1> {
    current
        .checked_add(additional)
        .ok_or(RawGltfAddressabilityInventoryErrorV1::TooManyStructuralReferences)
}

/// Invalid raw glTF addressability inventory.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum RawGltfAddressabilityInventoryErrorV1 {
    /// The semantic schema is not immutable V1.
    #[error("invalid raw glTF addressability inventory schema")]
    InvalidSchema,
    /// The embedded dependency closure identifies different primary bytes.
    #[error("raw glTF addressability dependency closure does not match primary input")]
    DependencyClosureMismatch,
    /// One independent domain exceeded its row ceiling.
    #[error("raw glTF addressability {domain} exceeded its row bound")]
    TooManyRows {
        /// Affected domain.
        domain: &'static str,
    },
    /// An unavailable domain retained positive-presence rows.
    #[error("raw glTF addressability unavailable {domain} retained rows")]
    UnavailableHasRows {
        /// Affected domain.
        domain: &'static str,
    },
    /// A row set is not a canonical source-order prefix.
    #[error("raw glTF addressability {domain} rows are not canonical")]
    NonCanonicalRows {
        /// Affected domain.
        domain: &'static str,
    },
    /// A retained name exceeded the per-name ceiling.
    #[error("raw glTF addressability name exceeded its UTF-8 byte bound")]
    NameTooLong,
    /// Aggregate retained text exceeded the V1 ceiling.
    #[error("raw glTF addressability retained too much text")]
    TooMuchText,
    /// One retained path exceeded the per-candidate segment ceiling.
    #[error("raw glTF addressability scene path exceeded its segment bound")]
    TooManyPathSegments,
    /// One observable authored-or-fallback path exceeded the UTF-8 byte ceiling.
    #[error("raw glTF addressability projected scene path exceeded its UTF-8 byte bound")]
    ProjectedPathTooLong,
    /// Aggregate structural references exceeded the V1 ceiling.
    #[error("raw glTF addressability retained too many structural references")]
    TooManyStructuralReferences,
    /// A reference is outside a completely observed source domain.
    #[error("raw glTF addressability reference is outside a complete source domain")]
    ReferenceOutOfRange,
    /// Complete parent/child observations disagree.
    #[error("raw glTF addressability node hierarchy is contradictory")]
    InvalidHierarchy,
    /// A retained candidate is not a scene-root-to-node path.
    #[error("raw glTF addressability scene path candidate is invalid")]
    InvalidPathCandidate,
    /// The deterministic identity encoding failed.
    #[error("raw glTF addressability identity encoding failed")]
    IdentityEncoding,
}

/// Bounded raw glTF inventory reader failure.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RawGltfAddressabilityInventoryReadErrorV1 {
    /// Reading the bounded input failed.
    #[error("failed to read raw glTF addressability inventory: {source}")]
    Io {
        /// Underlying reader error.
        source: std::io::Error,
    },
    /// The serialized input exceeded the immutable reader cap.
    #[error("raw glTF addressability inventory exceeds byte limit {limit}")]
    InventoryTooLarge {
        /// Immutable byte ceiling.
        limit: u64,
    },
    /// JSON shape or semantic validation failed.
    #[error("invalid raw glTF addressability inventory: {source}")]
    InvalidJson {
        /// Strict JSON decoder diagnostic.
        source: serde_json::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty() -> RawGltfAddressabilityInventoryV1 {
        let primary = InputIdentity::from_bytes(b"gltf");
        RawGltfAddressabilityInventoryV1::new(
            primary.clone(),
            DependencyClosureV1::unavailable(primary),
            RawGltfAddressabilityInventoryInputV1 {
                default_scene: RawGltfDefaultSceneObservationV1::Absent,
                scene_coverage: RawGltfAddressabilityCoverageV1::Complete,
                scenes: Vec::new(),
                node_coverage: RawGltfAddressabilityCoverageV1::Complete,
                nodes: Vec::new(),
                skin_coverage: RawGltfAddressabilityCoverageV1::Complete,
                skins: Vec::new(),
                attachment_coverage: RawGltfAddressabilityCoverageV1::Complete,
                attachments: Vec::new(),
                path_candidate_coverage: RawGltfAddressabilityCoverageV1::Complete,
                path_candidates: Vec::new(),
            },
        )
        .unwrap()
    }

    #[test]
    fn strict_round_trip_rejects_schema_identity_and_unknown_field_mutations() {
        let inventory = empty();
        let encoded = serde_json::to_vec(&inventory).unwrap();
        assert_eq!(
            RawGltfAddressabilityInventoryV1::read_from(encoded.as_slice()).unwrap(),
            inventory
        );
        let mut value = serde_json::to_value(&inventory).unwrap();
        value["schema"] = serde_json::json!("urn:animsmith:raw-gltf-addressability-inventory:2");
        assert!(serde_json::from_value::<RawGltfAddressabilityInventoryV1>(value).is_err());
        let mut value = serde_json::to_value(&inventory).unwrap();
        value["identity"]["sha256"] = serde_json::json!("0".repeat(64));
        assert!(serde_json::from_value::<RawGltfAddressabilityInventoryV1>(value).is_err());
        let mut value = serde_json::to_value(&inventory).unwrap();
        value["extra"] = serde_json::json!(true);
        assert!(serde_json::from_value::<RawGltfAddressabilityInventoryV1>(value).is_err());
    }

    #[test]
    fn row_name_and_structural_reference_bounds_accept_n_and_reject_n_plus_one() {
        let primary = InputIdentity::from_bytes(b"bounded");
        let closure = DependencyClosureV1::unavailable(primary.clone());
        let rows = (0..RAW_GLTF_ADDRESSABILITY_V1_MAX_ROWS_PER_DOMAIN)
            .map(|index| RawGltfSceneRowV1::new(index as u64, None, Vec::new()))
            .collect::<Vec<_>>();
        let inventory = RawGltfAddressabilityInventoryV1::new(
            primary.clone(),
            closure.clone(),
            RawGltfAddressabilityInventoryInputV1 {
                default_scene: RawGltfDefaultSceneObservationV1::Absent,
                scene_coverage: RawGltfAddressabilityCoverageV1::Complete,
                scenes: rows,
                node_coverage: RawGltfAddressabilityCoverageV1::Complete,
                nodes: Vec::new(),
                skin_coverage: RawGltfAddressabilityCoverageV1::Complete,
                skins: Vec::new(),
                attachment_coverage: RawGltfAddressabilityCoverageV1::Complete,
                attachments: Vec::new(),
                path_candidate_coverage: RawGltfAddressabilityCoverageV1::Complete,
                path_candidates: Vec::new(),
            },
        )
        .unwrap();
        let mut value = serde_json::to_value(&inventory).unwrap();
        value["scenes"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "source_scene_index": RAW_GLTF_ADDRESSABILITY_V1_MAX_ROWS_PER_DOMAIN,
                "name": null,
                "root_node_indices": []
            }));
        assert!(serde_json::from_value::<RawGltfAddressabilityInventoryV1>(value).is_err());

        let exact_name = "x".repeat(RAW_GLTF_ADDRESSABILITY_V1_MAX_NAME_BYTES);
        let row = RawGltfSceneRowV1::new(0, Some(exact_name), Vec::new());
        assert!(
            RawGltfAddressabilityInventoryV1::new(
                primary.clone(),
                closure.clone(),
                RawGltfAddressabilityInventoryInputV1 {
                    default_scene: RawGltfDefaultSceneObservationV1::Absent,
                    scene_coverage: RawGltfAddressabilityCoverageV1::Complete,
                    scenes: vec![row],
                    node_coverage: RawGltfAddressabilityCoverageV1::Complete,
                    nodes: Vec::new(),
                    skin_coverage: RawGltfAddressabilityCoverageV1::Complete,
                    skins: Vec::new(),
                    attachment_coverage: RawGltfAddressabilityCoverageV1::Complete,
                    attachments: Vec::new(),
                    path_candidate_coverage: RawGltfAddressabilityCoverageV1::Complete,
                    path_candidates: Vec::new(),
                },
            )
            .is_ok()
        );
        let mut value = serde_json::to_value(empty()).unwrap();
        value["scenes"] = serde_json::json!([{
            "source_scene_index": 0,
            "name": "x".repeat(RAW_GLTF_ADDRESSABILITY_V1_MAX_NAME_BYTES + 1),
            "root_node_indices": []
        }]);
        value["scene_coverage"] = serde_json::json!({"state":"complete"});
        assert!(serde_json::from_value::<RawGltfAddressabilityInventoryV1>(value).is_err());

        let bounded_input = |roots| RawGltfAddressabilityInventoryInputV1 {
            default_scene: RawGltfDefaultSceneObservationV1::Absent,
            scene_coverage: RawGltfAddressabilityCoverageV1::budget_exceeded(),
            scenes: vec![RawGltfSceneRowV1::new(0, None, roots)],
            node_coverage: RawGltfAddressabilityCoverageV1::Unavailable {
                reason: RawGltfAddressabilityCoverageReasonV1::ParserUnavailable,
            },
            nodes: Vec::new(),
            skin_coverage: RawGltfAddressabilityCoverageV1::Complete,
            skins: Vec::new(),
            attachment_coverage: RawGltfAddressabilityCoverageV1::Complete,
            attachments: Vec::new(),
            path_candidate_coverage: RawGltfAddressabilityCoverageV1::Unavailable {
                reason: RawGltfAddressabilityCoverageReasonV1::ParserUnavailable,
            },
            path_candidates: Vec::new(),
        };
        assert!(
            RawGltfAddressabilityInventoryV1::new(
                primary.clone(),
                closure.clone(),
                bounded_input(vec![
                    0;
                    RAW_GLTF_ADDRESSABILITY_V1_MAX_STRUCTURAL_REFERENCES
                ]),
            )
            .is_ok()
        );
        assert_eq!(
            RawGltfAddressabilityInventoryV1::new(
                primary.clone(),
                closure.clone(),
                bounded_input(vec![
                    0;
                    RAW_GLTF_ADDRESSABILITY_V1_MAX_STRUCTURAL_REFERENCES + 1
                ]),
            )
            .unwrap_err(),
            RawGltfAddressabilityInventoryErrorV1::TooManyStructuralReferences
        );

        let exact_text_rows = (0..1024)
            .map(|index| {
                RawGltfSceneRowV1::new(
                    index,
                    Some("x".repeat(RAW_GLTF_ADDRESSABILITY_V1_MAX_NAME_BYTES)),
                    Vec::new(),
                )
            })
            .collect::<Vec<_>>();
        let mut over_text_rows = exact_text_rows.clone();
        over_text_rows.push(RawGltfSceneRowV1::new(1024, Some("x".into()), Vec::new()));
        let text_input = |scenes| RawGltfAddressabilityInventoryInputV1 {
            default_scene: RawGltfDefaultSceneObservationV1::Absent,
            scene_coverage: RawGltfAddressabilityCoverageV1::budget_exceeded(),
            scenes,
            node_coverage: RawGltfAddressabilityCoverageV1::Complete,
            nodes: Vec::new(),
            skin_coverage: RawGltfAddressabilityCoverageV1::Complete,
            skins: Vec::new(),
            attachment_coverage: RawGltfAddressabilityCoverageV1::Complete,
            attachments: Vec::new(),
            path_candidate_coverage: RawGltfAddressabilityCoverageV1::Complete,
            path_candidates: Vec::new(),
        };
        assert!(
            RawGltfAddressabilityInventoryV1::new(
                primary.clone(),
                closure.clone(),
                text_input(exact_text_rows),
            )
            .is_ok()
        );
        assert_eq!(
            RawGltfAddressabilityInventoryV1::new(primary, closure, text_input(over_text_rows),)
                .unwrap_err(),
            RawGltfAddressabilityInventoryErrorV1::TooMuchText
        );
    }

    #[test]
    fn path_segment_and_projected_byte_bounds_accept_n_and_reject_n_plus_one() {
        let primary = InputIdentity::from_bytes(b"path-bounds");
        let closure = DependencyClosureV1::unavailable(primary.clone());
        let partial = RawGltfAddressabilityCoverageV1::budget_exceeded();
        let path_input = |segments| RawGltfAddressabilityInventoryInputV1 {
            default_scene: RawGltfDefaultSceneObservationV1::Absent,
            scene_coverage: RawGltfAddressabilityCoverageV1::Unavailable {
                reason: RawGltfAddressabilityCoverageReasonV1::ParserUnavailable,
            },
            scenes: Vec::new(),
            node_coverage: RawGltfAddressabilityCoverageV1::Unavailable {
                reason: RawGltfAddressabilityCoverageReasonV1::ParserUnavailable,
            },
            nodes: Vec::new(),
            skin_coverage: RawGltfAddressabilityCoverageV1::Complete,
            skins: Vec::new(),
            attachment_coverage: RawGltfAddressabilityCoverageV1::Complete,
            attachments: Vec::new(),
            path_candidate_coverage: partial,
            path_candidates: vec![RawGltfScenePathCandidateRowV1::new(0, 0, segments)],
        };
        let exact_segments = (0..RAW_GLTF_ADDRESSABILITY_V1_MAX_PATH_SEGMENTS as u64).collect();
        let exact = RawGltfAddressabilityInventoryV1::new(
            primary.clone(),
            closure.clone(),
            path_input(exact_segments),
        )
        .expect("the exact path-segment ceiling is valid");
        let mut serialized = serde_json::to_value(&exact).unwrap();
        serialized["path_candidates"][0]["source_node_indices"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!(
                RAW_GLTF_ADDRESSABILITY_V1_MAX_PATH_SEGMENTS
            ));
        assert!(
            serde_json::from_value::<RawGltfAddressabilityInventoryV1>(serialized).is_err(),
            "strict readback must reject the 257th segment"
        );
        assert_eq!(
            RawGltfAddressabilityInventoryV1::new(
                primary.clone(),
                closure.clone(),
                path_input((0..=RAW_GLTF_ADDRESSABILITY_V1_MAX_PATH_SEGMENTS as u64).collect()),
            )
            .unwrap_err(),
            RawGltfAddressabilityInventoryErrorV1::TooManyPathSegments
        );

        let chain_input = |names: Vec<String>| {
            let count = names.len();
            RawGltfAddressabilityInventoryInputV1 {
                default_scene: RawGltfDefaultSceneObservationV1::Selected {
                    source_scene_index: 0,
                },
                scene_coverage: RawGltfAddressabilityCoverageV1::Complete,
                scenes: vec![RawGltfSceneRowV1::new(0, None, vec![0])],
                node_coverage: RawGltfAddressabilityCoverageV1::Complete,
                nodes: names
                    .into_iter()
                    .enumerate()
                    .map(|(index, name)| {
                        RawGltfNodeRowV1::new(
                            index as u64,
                            Some(name),
                            index.checked_sub(1).map(|parent| parent as u64),
                            (index + 1 < count)
                                .then_some(vec![(index + 1) as u64])
                                .unwrap_or_default(),
                        )
                    })
                    .collect(),
                skin_coverage: RawGltfAddressabilityCoverageV1::Complete,
                skins: Vec::new(),
                attachment_coverage: RawGltfAddressabilityCoverageV1::Complete,
                attachments: Vec::new(),
                path_candidate_coverage: RawGltfAddressabilityCoverageV1::Complete,
                path_candidates: (0..count)
                    .map(|index| {
                        RawGltfScenePathCandidateRowV1::new(
                            index as u64,
                            0,
                            (0..=index as u64).collect(),
                        )
                    })
                    .collect(),
            }
        };
        let exact_names = vec![
            "a".repeat(1_023),
            "b".repeat(1_023),
            "c".repeat(1_023),
            "d".repeat(1_024),
        ];
        assert!(
            RawGltfAddressabilityInventoryV1::new(
                primary.clone(),
                closure.clone(),
                chain_input(exact_names.clone()),
            )
            .is_ok(),
            "4,096 projected bytes are valid"
        );
        let mut over_names = exact_names;
        over_names.push(String::new());
        assert_eq!(
            RawGltfAddressabilityInventoryV1::new(primary, closure, chain_input(over_names),)
                .unwrap_err(),
            RawGltfAddressabilityInventoryErrorV1::ProjectedPathTooLong
        );
    }

    #[test]
    fn every_row_domain_accepts_n_and_strict_readback_rejects_n_plus_one() {
        fn build(input: RawGltfAddressabilityInventoryInputV1) -> RawGltfAddressabilityInventoryV1 {
            let primary = InputIdentity::from_bytes(b"all-row-domains");
            RawGltfAddressabilityInventoryV1::new(
                primary.clone(),
                DependencyClosureV1::unavailable(primary),
                input,
            )
            .expect("the exact row ceiling is valid")
        }

        fn assert_appended_row_is_rejected(
            inventory: &RawGltfAddressabilityInventoryV1,
            field: &str,
            row: serde_json::Value,
        ) {
            let mut value = serde_json::to_value(inventory).unwrap();
            value[field].as_array_mut().unwrap().push(row);
            assert!(
                serde_json::from_value::<RawGltfAddressabilityInventoryV1>(value).is_err(),
                "{field} must reject N+1 before accepting an over-limit contract"
            );
        }

        let unavailable = RawGltfAddressabilityCoverageV1::Unavailable {
            reason: RawGltfAddressabilityCoverageReasonV1::ParserUnavailable,
        };
        let partial = RawGltfAddressabilityCoverageV1::budget_exceeded();
        let limit = RAW_GLTF_ADDRESSABILITY_V1_MAX_ROWS_PER_DOMAIN;

        let scenes = build(RawGltfAddressabilityInventoryInputV1 {
            default_scene: RawGltfDefaultSceneObservationV1::Absent,
            scene_coverage: RawGltfAddressabilityCoverageV1::Complete,
            scenes: (0..limit)
                .map(|index| RawGltfSceneRowV1::new(index as u64, None, Vec::new()))
                .collect(),
            node_coverage: unavailable,
            nodes: Vec::new(),
            skin_coverage: unavailable,
            skins: Vec::new(),
            attachment_coverage: unavailable,
            attachments: Vec::new(),
            path_candidate_coverage: unavailable,
            path_candidates: Vec::new(),
        });
        assert_appended_row_is_rejected(
            &scenes,
            "scenes",
            serde_json::to_value(RawGltfSceneRowV1::new(limit as u64, None, Vec::new())).unwrap(),
        );

        let nodes = build(RawGltfAddressabilityInventoryInputV1 {
            default_scene: RawGltfDefaultSceneObservationV1::Absent,
            scene_coverage: unavailable,
            scenes: Vec::new(),
            node_coverage: RawGltfAddressabilityCoverageV1::Complete,
            nodes: (0..limit)
                .map(|index| RawGltfNodeRowV1::new(index as u64, None, None, Vec::new()))
                .collect(),
            skin_coverage: unavailable,
            skins: Vec::new(),
            attachment_coverage: unavailable,
            attachments: Vec::new(),
            path_candidate_coverage: unavailable,
            path_candidates: Vec::new(),
        });
        assert_appended_row_is_rejected(
            &nodes,
            "nodes",
            serde_json::to_value(RawGltfNodeRowV1::new(limit as u64, None, None, Vec::new()))
                .unwrap(),
        );

        let skins = build(RawGltfAddressabilityInventoryInputV1 {
            default_scene: RawGltfDefaultSceneObservationV1::Absent,
            scene_coverage: unavailable,
            scenes: Vec::new(),
            node_coverage: unavailable,
            nodes: Vec::new(),
            skin_coverage: RawGltfAddressabilityCoverageV1::Complete,
            skins: (0..limit)
                .map(|index| {
                    RawGltfSkinRowV1::new(
                        index as u64,
                        None,
                        Vec::new(),
                        None,
                        RawGltfInverseBindMatricesObservationV1::Absent,
                    )
                })
                .collect(),
            attachment_coverage: unavailable,
            attachments: Vec::new(),
            path_candidate_coverage: unavailable,
            path_candidates: Vec::new(),
        });
        assert_appended_row_is_rejected(
            &skins,
            "skins",
            serde_json::to_value(RawGltfSkinRowV1::new(
                limit as u64,
                None,
                Vec::new(),
                None,
                RawGltfInverseBindMatricesObservationV1::Absent,
            ))
            .unwrap(),
        );

        let attachments = build(RawGltfAddressabilityInventoryInputV1 {
            default_scene: RawGltfDefaultSceneObservationV1::Absent,
            scene_coverage: unavailable,
            scenes: Vec::new(),
            node_coverage: unavailable,
            nodes: Vec::new(),
            skin_coverage: unavailable,
            skins: Vec::new(),
            attachment_coverage: partial,
            attachments: (0..limit)
                .map(|index| RawGltfSkinAttachmentRowV1::new(index as u64, index as u64))
                .collect(),
            path_candidate_coverage: unavailable,
            path_candidates: Vec::new(),
        });
        assert_appended_row_is_rejected(
            &attachments,
            "attachments",
            serde_json::to_value(RawGltfSkinAttachmentRowV1::new(limit as u64, limit as u64))
                .unwrap(),
        );

        let paths = build(RawGltfAddressabilityInventoryInputV1 {
            default_scene: RawGltfDefaultSceneObservationV1::Absent,
            scene_coverage: unavailable,
            scenes: Vec::new(),
            node_coverage: unavailable,
            nodes: Vec::new(),
            skin_coverage: unavailable,
            skins: Vec::new(),
            attachment_coverage: unavailable,
            attachments: Vec::new(),
            path_candidate_coverage: partial,
            path_candidates: (0..limit)
                .map(|index| {
                    RawGltfScenePathCandidateRowV1::new(index as u64, 0, vec![index as u64])
                })
                .collect(),
        });
        assert_appended_row_is_rejected(
            &paths,
            "path_candidates",
            serde_json::to_value(RawGltfScenePathCandidateRowV1::new(
                limit as u64,
                0,
                vec![limit as u64],
            ))
            .unwrap(),
        );
    }

    #[test]
    fn source_and_closure_mutations_are_rejected() {
        let inventory = empty();
        let mut value = serde_json::to_value(&inventory).unwrap();
        value["primary_input"]["bytes"] = serde_json::json!(999);
        assert!(serde_json::from_value::<RawGltfAddressabilityInventoryV1>(value).is_err());
        let mut value = serde_json::to_value(&inventory).unwrap();
        value["dependency_closure"]["primary_input"]["bytes"] = serde_json::json!(999);
        assert!(serde_json::from_value::<RawGltfAddressabilityInventoryV1>(value).is_err());
    }

    #[test]
    fn bounded_reader_accepts_n_and_rejects_n_plus_one_before_json_decode() {
        let encoded = serde_json::to_vec(&empty()).unwrap();
        assert!(
            RawGltfAddressabilityInventoryV1::read_from_with_limit(
                encoded.as_slice(),
                encoded.len() as u64,
            )
            .is_ok()
        );
        assert!(matches!(
            RawGltfAddressabilityInventoryV1::read_from_with_limit(
                encoded.as_slice(),
                encoded.len() as u64 - 1,
            ),
            Err(RawGltfAddressabilityInventoryReadErrorV1::InventoryTooLarge { .. })
        ));
    }
}
