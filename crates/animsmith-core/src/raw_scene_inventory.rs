//! Bounded format-neutral raw scene, attachment, and primitive evidence.
//!
//! Normalized mesh assets are not source-presence authority: a loader can
//! discard source primitive modes it does not normalize. This contract keeps
//! definition-side primitive rows separate from node attachments, so a later
//! consumer must make any attachment-by-primitive join under its own budget.

use crate::InputIdentity;
use crate::bounded_deserialize::{
    BudgetedCappedSequenceSeed, CappedSequence, RowBudget, consume_ignored_tail,
};
use serde::de::{DeserializeSeed, Error as _, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;
use std::marker::PhantomData;

/// Semantic identity of the raw scene/attachment inventory V1 contract.
pub const RAW_SCENE_ATTACHMENT_INVENTORY_V1_ID: &str =
    "urn:animsmith:raw-scene-attachment-inventory:1";
/// Maximum aggregate rows retained by one V1 inventory.
pub const RAW_SCENE_ATTACHMENT_INVENTORY_V1_MAX_ROWS: usize = 4_096;
/// Maximum aggregate UTF-8 bytes retained by one V1 inventory.
///
/// V1 is index-only and therefore retains no text. The explicit zero bound
/// prevents a later producer from slipping unbounded names into this contract.
pub const RAW_SCENE_ATTACHMENT_INVENTORY_V1_MAX_TEXT_BYTES: usize = 0;

fn deserialize_inventory_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct BoundedVecVisitor<T>(PhantomData<T>);

    impl<'de, T> Visitor<'de> for BoundedVecVisitor<T>
    where
        T: Deserialize<'de>,
    {
        type Value = Vec<T>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                formatter,
                "at most {RAW_SCENE_ATTACHMENT_INVENTORY_V1_MAX_ROWS} raw inventory rows"
            )
        }

        fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut values = Vec::with_capacity(
                sequence
                    .size_hint()
                    .unwrap_or(0)
                    .min(RAW_SCENE_ATTACHMENT_INVENTORY_V1_MAX_ROWS),
            );
            while values.len() < RAW_SCENE_ATTACHMENT_INVENTORY_V1_MAX_ROWS {
                let Some(value) = sequence.next_element()? else {
                    return Ok(values);
                };
                values.push(value);
            }
            if sequence.next_element::<serde::de::IgnoredAny>()?.is_some() {
                return Err(A::Error::custom(
                    "raw scene/attachment inventory exceeded its row bound",
                ));
            }
            Ok(values)
        }
    }

    deserializer.deserialize_seq(BoundedVecVisitor(PhantomData))
}

/// Whether a source-order row set is exhaustive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RawSceneAttachmentCoverageV1 {
    /// Every source row is retained; empty proves absence.
    Complete,
    /// Rows are the canonical source-order prefix and later rows overflowed the V1 budget.
    PrefixOverflow,
    /// The loader could not make this source domain available.
    Unavailable,
}

impl RawSceneAttachmentCoverageV1 {
    /// Whether an empty row list proves the source domain absent.
    pub const fn proves_absence(self) -> bool {
        matches!(self, Self::Complete)
    }
}

/// Same-load source-skeleton evidence carried beside raw rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawSourceSkeletonEvidenceV1 {
    coverage: RawSceneAttachmentCoverageV1,
    source_node_count: u64,
    source_skin_count: u64,
}

impl RawSourceSkeletonEvidenceV1 {
    /// Construct coverage-qualified source-skeleton cardinality evidence.
    pub const fn new(
        coverage: RawSceneAttachmentCoverageV1,
        source_node_count: u64,
        source_skin_count: u64,
    ) -> Self {
        Self {
            coverage,
            source_node_count,
            source_skin_count,
        }
    }
    /// Source-skeleton evidence coverage.
    pub const fn coverage(&self) -> RawSceneAttachmentCoverageV1 {
        self.coverage
    }
    /// Source-node rows observed by this load.
    pub const fn source_node_count(&self) -> u64 {
        self.source_node_count
    }
    /// Source-skin rows observed by this load.
    pub const fn source_skin_count(&self) -> u64 {
        self.source_skin_count
    }
}

/// One source scene and its declared roots in authored order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawSceneRootRowV1 {
    source_scene_index: u64,
    #[serde(deserialize_with = "deserialize_inventory_vec")]
    root_node_indices: Vec<u64>,
}
impl RawSceneRootRowV1 {
    /// Construct one source-scene row.
    pub fn new(source_scene_index: u64, root_node_indices: Vec<u64>) -> Self {
        Self {
            source_scene_index,
            root_node_indices,
        }
    }
    /// Source scene-array index.
    pub const fn source_scene_index(&self) -> u64 {
        self.source_scene_index
    }
    /// Root-node identities in authored order.
    pub fn root_node_indices(&self) -> &[u64] {
        &self.root_node_indices
    }
}

/// One raw node-to-mesh declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawNodeMeshAttachmentRowV1 {
    source_node_index: u64,
    source_mesh_index: u64,
}
impl RawNodeMeshAttachmentRowV1 {
    /// Construct one raw attachment row.
    pub const fn new(source_node_index: u64, source_mesh_index: u64) -> Self {
        Self {
            source_node_index,
            source_mesh_index,
        }
    }
    /// Source node-array index.
    pub const fn source_node_index(&self) -> u64 {
        self.source_node_index
    }
    /// Source mesh-array index.
    pub const fn source_mesh_index(&self) -> u64 {
        self.source_mesh_index
    }
}

/// Raw primitive topology, including modes normalized mesh assets can omit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RawPrimitiveTopologyV1 {
    /// glTF mode 0.
    Points,
    /// glTF mode 1.
    Lines,
    /// glTF mode 2.
    LineLoop,
    /// glTF mode 3.
    LineStrip,
    /// glTF mode 4.
    Triangles,
    /// glTF mode 5.
    TriangleStrip,
    /// glTF mode 6.
    TriangleFan,
    /// A raw mode outside glTF's enumerated domain.
    Other {
        /// Raw `mode` value.
        mode: u64,
    },
}
impl RawPrimitiveTopologyV1 {
    /// Classify a raw glTF primitive mode.
    pub const fn from_gltf_mode(mode: u64) -> Self {
        match mode {
            0 => Self::Points,
            1 => Self::Lines,
            2 => Self::LineLoop,
            3 => Self::LineStrip,
            4 => Self::Triangles,
            5 => Self::TriangleStrip,
            6 => Self::TriangleFan,
            _ => Self::Other { mode },
        }
    }
    /// Raw glTF mode number.
    pub const fn gltf_mode(self) -> u64 {
        match self {
            Self::Points => 0,
            Self::Lines => 1,
            Self::LineLoop => 2,
            Self::LineStrip => 3,
            Self::Triangles => 4,
            Self::TriangleStrip => 5,
            Self::TriangleFan => 6,
            Self::Other { mode } => mode,
        }
    }
}

/// One raw mesh primitive, independent of node attachment rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawMeshPrimitiveRowV1 {
    source_mesh_index: u64,
    source_primitive_index: u64,
    topology: RawPrimitiveTopologyV1,
    indices_accessor_index: Option<u64>,
}
impl RawMeshPrimitiveRowV1 {
    /// Construct one raw primitive row.
    pub const fn new(
        source_mesh_index: u64,
        source_primitive_index: u64,
        topology: RawPrimitiveTopologyV1,
        indices_accessor_index: Option<u64>,
    ) -> Self {
        Self {
            source_mesh_index,
            source_primitive_index,
            topology,
            indices_accessor_index,
        }
    }
    /// Source mesh-array index.
    pub const fn source_mesh_index(&self) -> u64 {
        self.source_mesh_index
    }
    /// Primitive-array index inside the source mesh.
    pub const fn source_primitive_index(&self) -> u64 {
        self.source_primitive_index
    }
    /// Raw source topology.
    pub const fn topology(&self) -> RawPrimitiveTopologyV1 {
        self.topology
    }
    /// Declared index accessor, absent for unindexed primitives.
    pub const fn indices_accessor_index(&self) -> Option<u64> {
        self.indices_accessor_index
    }
}

/// Coverage-qualified source scenes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawSceneRootRowsV1 {
    coverage: RawSceneAttachmentCoverageV1,
    #[serde(deserialize_with = "deserialize_inventory_vec")]
    rows: Vec<RawSceneRootRowV1>,
}
impl RawSceneRootRowsV1 {
    /// Construct a source-scene row set.
    pub fn new(coverage: RawSceneAttachmentCoverageV1, rows: Vec<RawSceneRootRowV1>) -> Self {
        Self { coverage, rows }
    }
    /// Exhaustiveness state.
    pub const fn coverage(&self) -> RawSceneAttachmentCoverageV1 {
        self.coverage
    }
    /// Canonical source-order rows.
    pub fn rows(&self) -> &[RawSceneRootRowV1] {
        &self.rows
    }
}

/// Coverage-qualified raw node-to-mesh declarations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawNodeMeshAttachmentRowsV1 {
    coverage: RawSceneAttachmentCoverageV1,
    #[serde(deserialize_with = "deserialize_inventory_vec")]
    rows: Vec<RawNodeMeshAttachmentRowV1>,
}
impl RawNodeMeshAttachmentRowsV1 {
    /// Construct an attachment row set.
    pub fn new(
        coverage: RawSceneAttachmentCoverageV1,
        rows: Vec<RawNodeMeshAttachmentRowV1>,
    ) -> Self {
        Self { coverage, rows }
    }
    /// Exhaustiveness state.
    pub const fn coverage(&self) -> RawSceneAttachmentCoverageV1 {
        self.coverage
    }
    /// Canonical source-node-order rows.
    pub fn rows(&self) -> &[RawNodeMeshAttachmentRowV1] {
        &self.rows
    }
}

/// Coverage-qualified raw mesh primitive declarations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawMeshPrimitiveRowsV1 {
    coverage: RawSceneAttachmentCoverageV1,
    #[serde(deserialize_with = "deserialize_inventory_vec")]
    rows: Vec<RawMeshPrimitiveRowV1>,
}
impl RawMeshPrimitiveRowsV1 {
    /// Construct a primitive row set.
    pub fn new(coverage: RawSceneAttachmentCoverageV1, rows: Vec<RawMeshPrimitiveRowV1>) -> Self {
        Self { coverage, rows }
    }
    /// Exhaustiveness state.
    pub const fn coverage(&self) -> RawSceneAttachmentCoverageV1 {
        self.coverage
    }
    /// Canonical mesh/primitive-order rows.
    pub fn rows(&self) -> &[RawMeshPrimitiveRowV1] {
        &self.rows
    }
}

/// Bounded same-load raw scene/node-mesh/primitive inventory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RawSceneAttachmentInventoryV1 {
    schema: &'static str,
    identity: InputIdentity,
    primary_input: InputIdentity,
    source_skeleton: RawSourceSkeletonEvidenceV1,
    scenes: RawSceneRootRowsV1,
    node_mesh_attachments: RawNodeMeshAttachmentRowsV1,
    mesh_primitives: RawMeshPrimitiveRowsV1,
}

fn set_once<T, E>(slot: &mut Option<T>, value: T, field: &'static str) -> Result<(), E>
where
    E: serde::de::Error,
{
    if slot.replace(value).is_some() {
        return Err(E::duplicate_field(field));
    }
    Ok(())
}

fn required<T, E>(value: Option<T>, field: &'static str) -> Result<T, E>
where
    E: serde::de::Error,
{
    value.ok_or_else(|| E::missing_field(field))
}

enum BudgetedSceneRow {
    Value(RawSceneRootRowV1),
    Skipped,
}

struct BudgetedSceneRowSeed<'a> {
    budget: &'a mut RowBudget,
}

impl<'de> DeserializeSeed<'de> for BudgetedSceneRowSeed<'_> {
    type Value = BudgetedSceneRow;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        if !self.budget.admit() {
            return IgnoredAny::deserialize(deserializer).map(|_| BudgetedSceneRow::Skipped);
        }

        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Field {
            SourceSceneIndex,
            RootNodeIndices,
        }

        struct SceneRowVisitor<'a> {
            budget: &'a mut RowBudget,
        }

        impl<'de> Visitor<'de> for SceneRowVisitor<'_> {
            type Value = RawSceneRootRowV1;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("one raw scene-root row")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut source_scene_index = None;
                let mut root_node_indices = None;
                while let Some(field) = map.next_key()? {
                    match field {
                        Field::SourceSceneIndex => set_once(
                            &mut source_scene_index,
                            map.next_value()?,
                            "source_scene_index",
                        )?,
                        Field::RootNodeIndices => {
                            if root_node_indices.is_some() {
                                return Err(A::Error::duplicate_field("root_node_indices"));
                            }
                            root_node_indices =
                                Some(map.next_value_seed(BudgetedCappedSequenceSeed {
                                    budget: self.budget,
                                    local_limit: RAW_SCENE_ATTACHMENT_INVENTORY_V1_MAX_ROWS,
                                    element: PhantomData,
                                })?);
                        }
                    }
                }
                let roots: CappedSequence<u64> = required(root_node_indices, "root_node_indices")?;
                if roots.overflowed {
                    return Err(A::Error::custom(
                        "raw scene/attachment inventory exceeded its row bound",
                    ));
                }
                Ok(RawSceneRootRowV1 {
                    source_scene_index: required(source_scene_index, "source_scene_index")?,
                    root_node_indices: roots.values,
                })
            }
        }

        deserializer
            .deserialize_struct(
                "RawSceneRootRowV1",
                &["source_scene_index", "root_node_indices"],
                SceneRowVisitor {
                    budget: self.budget,
                },
            )
            .map(BudgetedSceneRow::Value)
    }
}

struct BudgetedSceneRowsSeed<'a> {
    budget: &'a mut RowBudget,
}

impl<'de> DeserializeSeed<'de> for BudgetedSceneRowsSeed<'_> {
    type Value = CappedSequence<RawSceneRootRowV1>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct RowsVisitor<'a> {
            budget: &'a mut RowBudget,
        }

        impl<'de> Visitor<'de> for RowsVisitor<'_> {
            type Value = CappedSequence<RawSceneRootRowV1>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a bounded sequence of raw scene-root rows")
            }

            fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut values = Vec::with_capacity(
                    sequence
                        .size_hint()
                        .unwrap_or(0)
                        .min(RAW_SCENE_ATTACHMENT_INVENTORY_V1_MAX_ROWS),
                );
                let mut seen = 0usize;
                while seen < RAW_SCENE_ATTACHMENT_INVENTORY_V1_MAX_ROWS {
                    let Some(row) = sequence.next_element_seed(BudgetedSceneRowSeed {
                        budget: self.budget,
                    })?
                    else {
                        return Ok(CappedSequence {
                            values,
                            overflowed: false,
                        });
                    };
                    seen += 1;
                    match row {
                        BudgetedSceneRow::Value(row) => values.push(row),
                        BudgetedSceneRow::Skipped => {
                            let overflowed = consume_ignored_tail(
                                &mut sequence,
                                seen,
                                RAW_SCENE_ATTACHMENT_INVENTORY_V1_MAX_ROWS,
                            )?;
                            return Ok(CappedSequence { values, overflowed });
                        }
                    }
                }
                let overflowed = consume_ignored_tail(
                    &mut sequence,
                    seen,
                    RAW_SCENE_ATTACHMENT_INVENTORY_V1_MAX_ROWS,
                )?;
                Ok(CappedSequence { values, overflowed })
            }
        }

        deserializer.deserialize_seq(RowsVisitor {
            budget: self.budget,
        })
    }
}

struct RawSceneRowsSetSeed<'a> {
    budget: &'a mut RowBudget,
}

impl<'de> DeserializeSeed<'de> for RawSceneRowsSetSeed<'_> {
    type Value = RawSceneRootRowsV1;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Field {
            Coverage,
            Rows,
        }
        struct SetVisitor<'a> {
            budget: &'a mut RowBudget,
        }
        impl<'de> Visitor<'de> for SetVisitor<'_> {
            type Value = RawSceneRootRowsV1;
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("coverage-qualified raw scene-root rows")
            }
            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut coverage = None;
                let mut rows = None;
                while let Some(field) = map.next_key()? {
                    match field {
                        Field::Coverage => set_once(&mut coverage, map.next_value()?, "coverage")?,
                        Field::Rows => {
                            if rows.is_some() {
                                return Err(A::Error::duplicate_field("rows"));
                            }
                            rows = Some(map.next_value_seed(BudgetedSceneRowsSeed {
                                budget: self.budget,
                            })?);
                        }
                    }
                }
                let rows: CappedSequence<_> = required(rows, "rows")?;
                if rows.overflowed {
                    return Err(A::Error::custom(
                        "raw scene/attachment inventory exceeded its row bound",
                    ));
                }
                Ok(RawSceneRootRowsV1 {
                    coverage: required(coverage, "coverage")?,
                    rows: rows.values,
                })
            }
        }
        deserializer.deserialize_struct(
            "RawSceneRootRowsV1",
            &["coverage", "rows"],
            SetVisitor {
                budget: self.budget,
            },
        )
    }
}

trait RawRowsSet: Sized {
    type Row;
    const NAME: &'static str;
    fn from_parts(coverage: RawSceneAttachmentCoverageV1, rows: Vec<Self::Row>) -> Self;
}

impl RawRowsSet for RawNodeMeshAttachmentRowsV1 {
    type Row = RawNodeMeshAttachmentRowV1;
    const NAME: &'static str = "RawNodeMeshAttachmentRowsV1";
    fn from_parts(coverage: RawSceneAttachmentCoverageV1, rows: Vec<Self::Row>) -> Self {
        Self { coverage, rows }
    }
}

impl RawRowsSet for RawMeshPrimitiveRowsV1 {
    type Row = RawMeshPrimitiveRowV1;
    const NAME: &'static str = "RawMeshPrimitiveRowsV1";
    fn from_parts(coverage: RawSceneAttachmentCoverageV1, rows: Vec<Self::Row>) -> Self {
        Self { coverage, rows }
    }
}

struct RawRowsSetSeed<'a, S> {
    budget: &'a mut RowBudget,
    set: PhantomData<fn() -> S>,
}

impl<'de, S> DeserializeSeed<'de> for RawRowsSetSeed<'_, S>
where
    S: RawRowsSet,
    S::Row: Deserialize<'de>,
{
    type Value = S;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Field {
            Coverage,
            Rows,
        }
        struct SetVisitor<'a, S> {
            budget: &'a mut RowBudget,
            set: PhantomData<fn() -> S>,
        }
        impl<'de, S> Visitor<'de> for SetVisitor<'_, S>
        where
            S: RawRowsSet,
            S::Row: Deserialize<'de>,
        {
            type Value = S;
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, "coverage-qualified {} rows", S::NAME)
            }
            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut coverage = None;
                let mut rows = None;
                while let Some(field) = map.next_key()? {
                    match field {
                        Field::Coverage => set_once(&mut coverage, map.next_value()?, "coverage")?,
                        Field::Rows => {
                            if rows.is_some() {
                                return Err(A::Error::duplicate_field("rows"));
                            }
                            rows = Some(map.next_value_seed(BudgetedCappedSequenceSeed {
                                budget: self.budget,
                                local_limit: RAW_SCENE_ATTACHMENT_INVENTORY_V1_MAX_ROWS,
                                element: PhantomData,
                            })?);
                        }
                    }
                }
                let rows: CappedSequence<S::Row> = required(rows, "rows")?;
                if rows.overflowed {
                    return Err(A::Error::custom(
                        "raw scene/attachment inventory exceeded its row bound",
                    ));
                }
                Ok(S::from_parts(required(coverage, "coverage")?, rows.values))
            }
        }
        deserializer.deserialize_struct(
            S::NAME,
            &["coverage", "rows"],
            SetVisitor {
                budget: self.budget,
                set: PhantomData,
            },
        )
    }
}

impl<'de> Deserialize<'de> for RawSceneAttachmentInventoryV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Field {
            Schema,
            Identity,
            PrimaryInput,
            SourceSkeleton,
            Scenes,
            NodeMeshAttachments,
            MeshPrimitives,
        }
        struct InventoryVisitor;
        impl<'de> Visitor<'de> for InventoryVisitor {
            type Value = (
                String,
                InputIdentity,
                InputIdentity,
                RawSourceSkeletonEvidenceV1,
                RawSceneRootRowsV1,
                RawNodeMeshAttachmentRowsV1,
                RawMeshPrimitiveRowsV1,
                usize,
                bool,
            );
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a bounded raw scene/attachment inventory")
            }
            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut budget = RowBudget::new(RAW_SCENE_ATTACHMENT_INVENTORY_V1_MAX_ROWS);
                let mut schema = None;
                let mut identity = None;
                let mut primary_input = None;
                let mut source_skeleton = None;
                let mut scenes = None;
                let mut node_mesh_attachments = None;
                let mut mesh_primitives = None;
                while let Some(field) = map.next_key()? {
                    match field {
                        Field::Schema => set_once(&mut schema, map.next_value()?, "schema")?,
                        Field::Identity => set_once(&mut identity, map.next_value()?, "identity")?,
                        Field::PrimaryInput => {
                            set_once(&mut primary_input, map.next_value()?, "primary_input")?
                        }
                        Field::SourceSkeleton => {
                            set_once(&mut source_skeleton, map.next_value()?, "source_skeleton")?
                        }
                        Field::Scenes => {
                            if scenes.is_some() {
                                return Err(A::Error::duplicate_field("scenes"));
                            }
                            scenes = Some(map.next_value_seed(RawSceneRowsSetSeed {
                                budget: &mut budget,
                            })?);
                        }
                        Field::NodeMeshAttachments => {
                            if node_mesh_attachments.is_some() {
                                return Err(A::Error::duplicate_field("node_mesh_attachments"));
                            }
                            node_mesh_attachments = Some(map.next_value_seed(RawRowsSetSeed {
                                budget: &mut budget,
                                set: PhantomData,
                            })?);
                        }
                        Field::MeshPrimitives => {
                            if mesh_primitives.is_some() {
                                return Err(A::Error::duplicate_field("mesh_primitives"));
                            }
                            mesh_primitives = Some(map.next_value_seed(RawRowsSetSeed {
                                budget: &mut budget,
                                set: PhantomData,
                            })?);
                        }
                    }
                }
                Ok((
                    required(schema, "schema")?,
                    required(identity, "identity")?,
                    required(primary_input, "primary_input")?,
                    required(source_skeleton, "source_skeleton")?,
                    required(scenes, "scenes")?,
                    required(node_mesh_attachments, "node_mesh_attachments")?,
                    required(mesh_primitives, "mesh_primitives")?,
                    budget.found(),
                    budget.overflowed(),
                ))
            }
        }
        let (
            schema,
            identity,
            primary_input,
            source_skeleton,
            scenes,
            node_mesh_attachments,
            mesh_primitives,
            found,
            overflowed,
        ) = deserializer.deserialize_struct(
            "RawSceneAttachmentInventoryV1",
            &[
                "schema",
                "identity",
                "primary_input",
                "source_skeleton",
                "scenes",
                "node_mesh_attachments",
                "mesh_primitives",
            ],
            InventoryVisitor,
        )?;
        if overflowed {
            return Err(D::Error::custom(
                RawSceneAttachmentInventoryError::TooManyRows {
                    found,
                    limit: RAW_SCENE_ATTACHMENT_INVENTORY_V1_MAX_ROWS,
                },
            ));
        }
        if schema != RAW_SCENE_ATTACHMENT_INVENTORY_V1_ID {
            return Err(serde::de::Error::custom(format!(
                "raw scene/attachment inventory schema must be {RAW_SCENE_ATTACHMENT_INVENTORY_V1_ID:?}"
            )));
        }
        let value = Self {
            schema: RAW_SCENE_ATTACHMENT_INVENTORY_V1_ID,
            identity,
            primary_input,
            source_skeleton,
            scenes,
            node_mesh_attachments,
            mesh_primitives,
        };
        value.validate().map_err(serde::de::Error::custom)?;
        let expected = inventory_identity(
            &value.primary_input,
            value.source_skeleton,
            &value.scenes,
            &value.node_mesh_attachments,
            &value.mesh_primitives,
        )
        .map_err(serde::de::Error::custom)?;
        if value.identity != expected {
            return Err(serde::de::Error::custom(
                "raw scene/attachment inventory identity does not match its contents",
            ));
        }
        Ok(value)
    }
}
impl RawSceneAttachmentInventoryV1 {
    /// Construct and validate a bounded same-load inventory.
    pub fn new(
        primary_input: InputIdentity,
        source_skeleton: RawSourceSkeletonEvidenceV1,
        scenes: RawSceneRootRowsV1,
        node_mesh_attachments: RawNodeMeshAttachmentRowsV1,
        mesh_primitives: RawMeshPrimitiveRowsV1,
    ) -> Result<Self, RawSceneAttachmentInventoryError> {
        let identity = inventory_identity(
            &primary_input,
            source_skeleton,
            &scenes,
            &node_mesh_attachments,
            &mesh_primitives,
        )?;
        let value = Self {
            schema: RAW_SCENE_ATTACHMENT_INVENTORY_V1_ID,
            identity,
            primary_input,
            source_skeleton,
            scenes,
            node_mesh_attachments,
            mesh_primitives,
        };
        value.validate()?;
        Ok(value)
    }
    /// Semantic inventory identifier.
    pub const fn contract_id(&self) -> &'static str {
        self.schema
    }
    /// Canonical identity over the complete bounded inventory contract.
    pub const fn identity(&self) -> &InputIdentity {
        &self.identity
    }
    /// Exact primary input identity for this loader pass.
    pub const fn primary_input(&self) -> &InputIdentity {
        &self.primary_input
    }
    /// Same-load source-skeleton evidence.
    pub const fn source_skeleton(&self) -> &RawSourceSkeletonEvidenceV1 {
        &self.source_skeleton
    }
    /// Coverage-qualified source-scene rows.
    pub const fn scenes(&self) -> &RawSceneRootRowsV1 {
        &self.scenes
    }
    /// Coverage-qualified node-to-mesh rows.
    pub const fn node_mesh_attachments(&self) -> &RawNodeMeshAttachmentRowsV1 {
        &self.node_mesh_attachments
    }
    /// Coverage-qualified primitive definition rows.
    pub const fn mesh_primitives(&self) -> &RawMeshPrimitiveRowsV1 {
        &self.mesh_primitives
    }
    fn validate(&self) -> Result<(), RawSceneAttachmentInventoryError> {
        validate_unavailable_rows("scenes", self.scenes.coverage, self.scenes.rows.len())?;
        validate_unavailable_rows(
            "node_mesh_attachments",
            self.node_mesh_attachments.coverage,
            self.node_mesh_attachments.rows.len(),
        )?;
        validate_unavailable_rows(
            "mesh_primitives",
            self.mesh_primitives.coverage,
            self.mesh_primitives.rows.len(),
        )?;
        if self.source_skeleton.coverage == RawSceneAttachmentCoverageV1::Unavailable
            && (self.source_skeleton.source_node_count != 0
                || self.source_skeleton.source_skin_count != 0)
        {
            return Err(RawSceneAttachmentInventoryError::UnavailableSkeletonHasCounts);
        }
        let rows = self
            .scenes
            .rows
            .iter()
            .try_fold(0usize, |total, row| {
                total
                    .checked_add(1)?
                    .checked_add(row.root_node_indices.len())
            })
            .and_then(|rows| rows.checked_add(self.node_mesh_attachments.rows.len()))
            .and_then(|rows| rows.checked_add(self.mesh_primitives.rows.len()))
            .ok_or(RawSceneAttachmentInventoryError::AggregateRowsOverflow)?;
        if rows > RAW_SCENE_ATTACHMENT_INVENTORY_V1_MAX_ROWS {
            return Err(RawSceneAttachmentInventoryError::TooManyRows {
                found: rows,
                limit: RAW_SCENE_ATTACHMENT_INVENTORY_V1_MAX_ROWS,
            });
        }
        for (expected, row) in self.scenes.rows.iter().enumerate() {
            if row.source_scene_index != expected as u64 {
                return Err(RawSceneAttachmentInventoryError::NonCanonicalSceneOrder {
                    expected: expected as u64,
                    found: row.source_scene_index,
                });
            }
        }
        if self.source_skeleton.coverage == RawSceneAttachmentCoverageV1::Complete {
            for scene in &self.scenes.rows {
                for &node in &scene.root_node_indices {
                    if node >= self.source_skeleton.source_node_count {
                        return Err(RawSceneAttachmentInventoryError::SceneRootNodeOutOfRange {
                            node,
                            node_count: self.source_skeleton.source_node_count,
                        });
                    }
                }
            }
            for attachment in &self.node_mesh_attachments.rows {
                if attachment.source_node_index >= self.source_skeleton.source_node_count {
                    return Err(RawSceneAttachmentInventoryError::AttachmentNodeOutOfRange {
                        node: attachment.source_node_index,
                        node_count: self.source_skeleton.source_node_count,
                    });
                }
            }
        }
        if self
            .node_mesh_attachments
            .rows
            .windows(2)
            .any(|rows| rows[0].source_node_index >= rows[1].source_node_index)
        {
            return Err(RawSceneAttachmentInventoryError::NonCanonicalAttachmentOrder);
        }
        if self.mesh_primitives.rows.windows(2).any(|rows| {
            (rows[0].source_mesh_index, rows[0].source_primitive_index)
                >= (rows[1].source_mesh_index, rows[1].source_primitive_index)
        }) {
            return Err(RawSceneAttachmentInventoryError::NonCanonicalPrimitiveOrder);
        }
        if self.mesh_primitives.coverage == RawSceneAttachmentCoverageV1::Complete {
            let mut current_mesh = None;
            let mut expected_primitive = 0;
            for primitive in &self.mesh_primitives.rows {
                if current_mesh != Some(primitive.source_mesh_index) {
                    current_mesh = Some(primitive.source_mesh_index);
                    expected_primitive = 0;
                }
                if primitive.source_primitive_index != expected_primitive {
                    return Err(
                        RawSceneAttachmentInventoryError::NonContiguousPrimitiveOrdinal {
                            mesh: primitive.source_mesh_index,
                            expected: expected_primitive,
                            found: primitive.source_primitive_index,
                        },
                    );
                }
                expected_primitive = expected_primitive.saturating_add(1);
            }
        }
        Ok(())
    }
}

fn validate_unavailable_rows(
    domain: &'static str,
    coverage: RawSceneAttachmentCoverageV1,
    rows: usize,
) -> Result<(), RawSceneAttachmentInventoryError> {
    if coverage == RawSceneAttachmentCoverageV1::Unavailable && rows != 0 {
        return Err(RawSceneAttachmentInventoryError::UnavailableHasRows { domain, rows });
    }
    Ok(())
}

#[derive(Serialize)]
struct InventoryIdentityFields<'a> {
    schema: &'static str,
    primary_input: &'a InputIdentity,
    source_skeleton: RawSourceSkeletonEvidenceV1,
    scenes: &'a RawSceneRootRowsV1,
    node_mesh_attachments: &'a RawNodeMeshAttachmentRowsV1,
    mesh_primitives: &'a RawMeshPrimitiveRowsV1,
}

fn inventory_identity(
    primary_input: &InputIdentity,
    source_skeleton: RawSourceSkeletonEvidenceV1,
    scenes: &RawSceneRootRowsV1,
    node_mesh_attachments: &RawNodeMeshAttachmentRowsV1,
    mesh_primitives: &RawMeshPrimitiveRowsV1,
) -> Result<InputIdentity, RawSceneAttachmentInventoryError> {
    let bytes = serde_json::to_vec(&InventoryIdentityFields {
        schema: RAW_SCENE_ATTACHMENT_INVENTORY_V1_ID,
        primary_input,
        source_skeleton,
        scenes,
        node_mesh_attachments,
        mesh_primitives,
    })
    .map_err(|error| RawSceneAttachmentInventoryError::IdentityEncoding {
        message: error.to_string(),
    })?;
    Ok(InputIdentity::from_bytes(&bytes))
}

/// Invalid raw scene/attachment inventory.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum RawSceneAttachmentInventoryError {
    /// The canonical identity encoding could not be produced.
    #[error("raw scene/attachment inventory identity encoding failed: {message}")]
    IdentityEncoding {
        /// Serializer diagnostic.
        message: String,
    },
    /// An unavailable row domain retained rows.
    #[error("raw scene/attachment unavailable {domain} retained {rows} row(s)")]
    UnavailableHasRows {
        /// Affected row domain.
        domain: &'static str,
        /// Retained row count.
        rows: usize,
    },
    /// Unavailable source-skeleton evidence retained cardinality facts.
    #[error("raw scene/attachment unavailable source skeleton retained nonzero counts")]
    UnavailableSkeletonHasCounts,
    /// Aggregate retained-row addition overflowed.
    #[error("raw scene/attachment aggregate row count overflowed")]
    AggregateRowsOverflow,
    /// Aggregate retained rows exceed the V1 limit.
    #[error("raw scene/attachment inventory retained {found} rows, exceeding V1 limit {limit}")]
    TooManyRows {
        /// Retained rows.
        found: usize,
        /// V1 limit.
        limit: usize,
    },
    /// Scene rows are not a source-order prefix.
    #[error("raw scene rows are not in canonical source order: expected {expected}, found {found}")]
    NonCanonicalSceneOrder {
        /// Required index.
        expected: u64,
        /// Retained index.
        found: u64,
    },
    /// Attachment rows are not strictly source-node ordered.
    #[error("raw node-to-mesh attachment rows are not in canonical source-node order")]
    NonCanonicalAttachmentOrder,
    /// A scene root references a node outside complete source-skeleton evidence.
    #[error("raw scene root node {node} is outside source node count {node_count}")]
    SceneRootNodeOutOfRange {
        /// Referenced source node.
        node: u64,
        /// Complete source-node count.
        node_count: u64,
    },
    /// An attachment references a node outside complete source-skeleton evidence.
    #[error("raw attachment node {node} is outside source node count {node_count}")]
    AttachmentNodeOutOfRange {
        /// Referenced source node.
        node: u64,
        /// Complete source-node count.
        node_count: u64,
    },
    /// Primitive rows are not strictly mesh/primitive ordered.
    #[error("raw mesh primitive rows are not in canonical mesh/primitive order")]
    NonCanonicalPrimitiveOrder,
    /// A complete primitive row set skipped an ordinal inside one mesh.
    #[error(
        "raw mesh {mesh} primitive order is not contiguous: expected {expected}, found {found}"
    )]
    NonContiguousPrimitiveOrdinal {
        /// Source mesh index.
        mesh: u64,
        /// Required primitive ordinal.
        expected: u64,
        /// Retained primitive ordinal.
        found: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_complete_sets_prove_absence() {
        let inventory = RawSceneAttachmentInventoryV1::new(
            InputIdentity::from_bytes(b"raw"),
            RawSourceSkeletonEvidenceV1::new(RawSceneAttachmentCoverageV1::Complete, 0, 0),
            RawSceneRootRowsV1::new(RawSceneAttachmentCoverageV1::Complete, vec![]),
            RawNodeMeshAttachmentRowsV1::new(RawSceneAttachmentCoverageV1::Complete, vec![]),
            RawMeshPrimitiveRowsV1::new(RawSceneAttachmentCoverageV1::Complete, vec![]),
        )
        .unwrap();
        assert!(inventory.mesh_primitives().coverage().proves_absence());
    }
    #[test]
    fn rejects_noncanonical_primitives() {
        let error = RawSceneAttachmentInventoryV1::new(
            InputIdentity::from_bytes(b"raw"),
            RawSourceSkeletonEvidenceV1::new(RawSceneAttachmentCoverageV1::Complete, 0, 0),
            RawSceneRootRowsV1::new(RawSceneAttachmentCoverageV1::Complete, vec![]),
            RawNodeMeshAttachmentRowsV1::new(RawSceneAttachmentCoverageV1::Complete, vec![]),
            RawMeshPrimitiveRowsV1::new(
                RawSceneAttachmentCoverageV1::Complete,
                vec![
                    RawMeshPrimitiveRowV1::new(1, 0, RawPrimitiveTopologyV1::Triangles, None),
                    RawMeshPrimitiveRowV1::new(0, 0, RawPrimitiveTopologyV1::Points, Some(2)),
                ],
            ),
        )
        .unwrap_err();
        assert_eq!(
            error,
            RawSceneAttachmentInventoryError::NonCanonicalPrimitiveOrder
        );
    }

    #[test]
    fn rejects_unavailable_rows_and_skeleton_counts() {
        let identity = InputIdentity::from_bytes(b"raw");
        let error = RawSceneAttachmentInventoryV1::new(
            identity.clone(),
            RawSourceSkeletonEvidenceV1::new(RawSceneAttachmentCoverageV1::Unavailable, 0, 0),
            RawSceneRootRowsV1::new(
                RawSceneAttachmentCoverageV1::Unavailable,
                vec![RawSceneRootRowV1::new(0, vec![])],
            ),
            RawNodeMeshAttachmentRowsV1::new(RawSceneAttachmentCoverageV1::Unavailable, vec![]),
            RawMeshPrimitiveRowsV1::new(RawSceneAttachmentCoverageV1::Unavailable, vec![]),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            RawSceneAttachmentInventoryError::UnavailableHasRows {
                domain: "scenes",
                rows: 1
            }
        ));

        let error = RawSceneAttachmentInventoryV1::new(
            identity,
            RawSourceSkeletonEvidenceV1::new(RawSceneAttachmentCoverageV1::Unavailable, 1, 0),
            RawSceneRootRowsV1::new(RawSceneAttachmentCoverageV1::Unavailable, vec![]),
            RawNodeMeshAttachmentRowsV1::new(RawSceneAttachmentCoverageV1::Unavailable, vec![]),
            RawMeshPrimitiveRowsV1::new(RawSceneAttachmentCoverageV1::Unavailable, vec![]),
        )
        .unwrap_err();
        assert_eq!(
            error,
            RawSceneAttachmentInventoryError::UnavailableSkeletonHasCounts
        );
    }

    #[test]
    fn deserialization_stops_at_each_raw_inventory_n_plus_one_boundary() {
        let roots = serde_json::json!({
            "source_scene_index": 0,
            "root_node_indices": vec![0_u64; RAW_SCENE_ATTACHMENT_INVENTORY_V1_MAX_ROWS + 1]
        });
        assert!(serde_json::from_value::<RawSceneRootRowV1>(roots).is_err());

        let attachments = serde_json::json!({
            "coverage": "prefix_overflow",
            "rows": (0..=RAW_SCENE_ATTACHMENT_INVENTORY_V1_MAX_ROWS)
                .map(|index| serde_json::json!({
                    "source_node_index": index,
                    "source_mesh_index": 0
                }))
                .collect::<Vec<_>>()
        });
        assert!(serde_json::from_value::<RawNodeMeshAttachmentRowsV1>(attachments).is_err());
    }

    #[test]
    fn inventory_deserialization_enforces_nested_scene_roots_against_aggregate_budget() {
        let inventory = RawSceneAttachmentInventoryV1::new(
            InputIdentity::from_bytes(b"raw"),
            RawSourceSkeletonEvidenceV1::new(RawSceneAttachmentCoverageV1::Complete, 1, 0),
            RawSceneRootRowsV1::new(
                RawSceneAttachmentCoverageV1::Complete,
                vec![RawSceneRootRowV1::new(
                    0,
                    vec![0; RAW_SCENE_ATTACHMENT_INVENTORY_V1_MAX_ROWS - 1],
                )],
            ),
            RawNodeMeshAttachmentRowsV1::new(RawSceneAttachmentCoverageV1::Complete, vec![]),
            RawMeshPrimitiveRowsV1::new(RawSceneAttachmentCoverageV1::Complete, vec![]),
        )
        .expect("one scene row plus N-1 roots consumes the exact aggregate budget");
        let exact = serde_json::to_string(&inventory).unwrap();
        let decoded: RawSceneAttachmentInventoryV1 = serde_json::from_str(&exact).unwrap();
        assert_eq!(decoded, inventory);

        let mut overflow: serde_json::Value = serde_json::from_str(&exact).unwrap();
        overflow["scenes"]["rows"][0]["root_node_indices"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::Value::Null);
        let error = serde_json::from_str::<RawSceneAttachmentInventoryV1>(
            &serde_json::to_string(&overflow).unwrap(),
        )
        .unwrap_err()
        .to_string();
        assert!(
            error.contains("retained 4097 rows"),
            "aggregate N+1 must win before the hostile tail value is decoded: {error}"
        );
    }

    #[test]
    fn rejects_complete_skeleton_node_references_outside_its_evidence() {
        let identity = InputIdentity::from_bytes(b"raw");
        let error = RawSceneAttachmentInventoryV1::new(
            identity.clone(),
            RawSourceSkeletonEvidenceV1::new(RawSceneAttachmentCoverageV1::Complete, 1, 0),
            RawSceneRootRowsV1::new(
                RawSceneAttachmentCoverageV1::Complete,
                vec![RawSceneRootRowV1::new(0, vec![1])],
            ),
            RawNodeMeshAttachmentRowsV1::new(RawSceneAttachmentCoverageV1::Complete, vec![]),
            RawMeshPrimitiveRowsV1::new(RawSceneAttachmentCoverageV1::Complete, vec![]),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            RawSceneAttachmentInventoryError::SceneRootNodeOutOfRange {
                node: 1,
                node_count: 1
            }
        ));

        let error = RawSceneAttachmentInventoryV1::new(
            identity,
            RawSourceSkeletonEvidenceV1::new(RawSceneAttachmentCoverageV1::Complete, 1, 0),
            RawSceneRootRowsV1::new(RawSceneAttachmentCoverageV1::Complete, vec![]),
            RawNodeMeshAttachmentRowsV1::new(
                RawSceneAttachmentCoverageV1::Complete,
                vec![RawNodeMeshAttachmentRowV1::new(1, 0)],
            ),
            RawMeshPrimitiveRowsV1::new(RawSceneAttachmentCoverageV1::Complete, vec![]),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            RawSceneAttachmentInventoryError::AttachmentNodeOutOfRange {
                node: 1,
                node_count: 1
            }
        ));
    }

    #[test]
    fn complete_primitive_rows_must_be_contiguous_per_mesh() {
        let error = RawSceneAttachmentInventoryV1::new(
            InputIdentity::from_bytes(b"raw"),
            RawSourceSkeletonEvidenceV1::new(RawSceneAttachmentCoverageV1::Complete, 0, 0),
            RawSceneRootRowsV1::new(RawSceneAttachmentCoverageV1::Complete, vec![]),
            RawNodeMeshAttachmentRowsV1::new(RawSceneAttachmentCoverageV1::Complete, vec![]),
            RawMeshPrimitiveRowsV1::new(
                RawSceneAttachmentCoverageV1::Complete,
                vec![RawMeshPrimitiveRowV1::new(
                    0,
                    1,
                    RawPrimitiveTopologyV1::Triangles,
                    None,
                )],
            ),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            RawSceneAttachmentInventoryError::NonContiguousPrimitiveOrdinal {
                mesh: 0,
                expected: 0,
                found: 1,
            }
        ));
    }
}
