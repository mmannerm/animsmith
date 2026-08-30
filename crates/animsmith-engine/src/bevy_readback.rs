//! Closed, engine-neutral contract for the isolated exact-Bevy probe.
//!
//! The executable lives outside this workspace because Bevy 0.19 needs Rust
//! 1.95. This module owns its bounded wire format and imports no Bevy APIs.

use crate::{
    GltfAddressabilityNamedMapKindV2, GltfAddressabilityProjectionV2, GltfAddressabilityReadbackV2,
    GltfAnimationCoverageStateV1,
};
use animsmith_core::InputIdentity;
use serde::{Deserialize, Serialize};
use std::io::Read;

/// Immutable schema identifier for Bevy readback V1.
pub const BEVY_READBACK_V1_ID: &str = "urn:animsmith:schema:bevy-readback:1";
/// Immutable schema version for Bevy readback V1.
pub const BEVY_READBACK_V1_SCHEMA_VERSION: u32 = 1;
/// Maximum serialized readback bytes accepted by the strict reader.
pub const BEVY_READBACK_V1_MAX_REPORT_BYTES: u64 = 16 * 1024 * 1024;
/// Maximum rows in each observed collection.
pub const BEVY_READBACK_V1_MAX_ROWS: usize = 4_096;
/// Maximum aggregate observed rows.
pub const BEVY_READBACK_V1_MAX_WORK: usize = 65_536;
/// Maximum `App::update` calls before the harness stops.
pub const BEVY_READBACK_V1_MAX_UPDATES: u64 = 4_096;
/// Exact compiler identity required by the isolated harness build script.
pub const BEVY_READBACK_V1_RUSTC: &str = "rustc 1.95.0 (59807616e 2026-04-14)";
/// Frozen byte count of the committed excluded-tool lock graph.
pub const BEVY_READBACK_V1_LOCK_BYTES: u64 = 86_374;
/// Frozen SHA-256 of the committed excluded-tool lock graph.
pub const BEVY_READBACK_V1_LOCK_SHA256: &str =
    "b674e2f74daff0a0815bfbbce258cbb76181c18b39ee5b6499c12a064a809b18";
const MAX_TEXT_BYTES: usize = 1_024;

/// Exact V2 document identity plus its canonical V4 provenance header.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BevyPredictionReferenceV1 {
    prediction_input: InputIdentity,
    provenance_schema: String,
    provenance_identity: InputIdentity,
}
impl BevyPredictionReferenceV1 {
    /// Construct an identity-only reference; settings and source facts remain in V2.
    pub fn new(
        prediction_input: InputIdentity,
        provenance_schema: String,
        provenance_identity: InputIdentity,
    ) -> Self {
        Self {
            prediction_input,
            provenance_schema,
            provenance_identity,
        }
    }
    /// Exact V2 document bytes.
    pub const fn prediction_input(&self) -> &InputIdentity {
        &self.prediction_input
    }
    /// V4 provenance schema identifier.
    pub fn provenance_schema(&self) -> &str {
        &self.provenance_schema
    }
    /// Canonical V4 provenance identity.
    pub const fn provenance_identity(&self) -> &InputIdentity {
        &self.provenance_identity
    }
}

/// Frozen executable and resolved-lock identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BevyHarnessIdentityV1 {
    engine: String,
    engine_version: String,
    tool_version: String,
    rust_toolchain: String,
    bevy_animation_feature: bool,
    load_animations: bool,
    lock_identity: InputIdentity,
    updates: u64,
}
impl BevyHarnessIdentityV1 {
    /// Construct the V1 exact-Bevy harness tuple.
    pub fn new(
        tool_version: String,
        rust_toolchain: String,
        bevy_animation_feature: bool,
        load_animations: bool,
        lock_identity: InputIdentity,
        updates: u64,
    ) -> Self {
        Self {
            engine: "bevy".into(),
            engine_version: "0.19.0".into(),
            tool_version,
            rust_toolchain,
            bevy_animation_feature,
            load_animations,
            lock_identity,
            updates,
        }
    }
}

/// One source-indexed Bevy subasset label.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BevyIndexedLabelV1 {
    index: u32,
    label: String,
}
impl BevyIndexedLabelV1 {
    /// Construct an indexed label.
    pub fn new(index: u32, label: String) -> Self {
        Self { index, label }
    }
    /// Observed source-array index.
    pub const fn index(&self) -> u32 {
        self.index
    }
}

/// One name-to-source-index winner in Bevy's named map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BevyNamedWinnerV1 {
    name: String,
    index: u32,
}
impl BevyNamedWinnerV1 {
    /// Construct a named-map winner.
    pub fn new(name: String, index: u32) -> Self {
        Self { name, index }
    }
    /// Canonical ordering key.
    pub fn sort_key(&self) -> (&str, u32) {
        (&self.name, self.index)
    }
}

/// One typed `AnimationClip` target ID observed from Bevy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BevyAnimationTargetV1 {
    animation_index: u32,
    target_id: String,
}
impl BevyAnimationTargetV1 {
    /// Construct one target observation.
    pub fn new(animation_index: u32, target_id: String) -> Self {
        Self {
            animation_index,
            target_id,
        }
    }
    /// Canonical ordering key.
    pub fn sort_key(&self) -> (u32, &str) {
        (self.animation_index, &self.target_id)
    }
}

/// Closed, prose-free classification of a public Bevy load error variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BevyLoadErrorCodeV1 {
    /// Empty asset path.
    EmptyPath,
    /// Requested handle had a wrong type.
    RequestedHandleTypeMismatch,
    /// No matching loader exists.
    MissingAssetLoader,
    /// Extension selected no loader.
    MissingAssetLoaderForExtension,
    /// Loader type name is missing.
    MissingAssetLoaderForTypeName,
    /// Loader type ID is missing.
    MissingAssetLoaderForTypeId,
    /// Asset bytes cannot be read.
    AssetReader,
    /// Asset source is unavailable.
    MissingAssetSource,
    /// Processed reader is unavailable.
    MissingProcessedAssetReader,
    /// Metadata bytes cannot be read.
    AssetMetadata,
    /// Metadata cannot be decoded.
    DeserializeMetadata,
    /// Asset is processed-only.
    CannotLoadProcessedAsset,
    /// Asset is ignored.
    CannotLoadIgnoredAsset,
    /// Loader panicked.
    LoaderPanic,
    /// Loader returned an opaque error.
    Loader,
    /// Async dependency resolution failed.
    AddAsync,
    /// A labeled subasset was absent.
    MissingLabel,
}

/// Terminal state observed from stock Bevy asset loading.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum BevyTerminalStateV1 {
    /// Root and recursive dependencies reached `Loaded`.
    Loaded,
    /// Root asset failure.
    RootFailure {
        /// Closed error family only.
        error: BevyLoadErrorCodeV1,
    },
    /// Recursive dependency failure.
    DependencyFailure {
        /// Closed error family only.
        error: BevyLoadErrorCodeV1,
    },
    /// The fixed update budget was exhausted.
    WorkLimit,
}

/// Bounded redacted tracing metadata, never formatted warning prose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BevyWarningV1 {
    target: String,
    level: String,
}
impl BevyWarningV1 {
    /// Construct redacted tracing metadata.
    pub fn new(target: String, level: String) -> Self {
        Self { target, level }
    }
}

/// Typed observations retained from one Bevy load lifecycle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BevyObservationV1 {
    terminal: BevyTerminalStateV1,
    animations: Vec<BevyIndexedLabelV1>,
    named_animation_winners: Vec<BevyNamedWinnerV1>,
    named_scene_winners: Vec<BevyNamedWinnerV1>,
    named_skin_winners: Vec<BevyNamedWinnerV1>,
    default_scene: Option<u32>,
    scenes: Vec<BevyIndexedLabelV1>,
    nodes: Vec<BevyIndexedLabelV1>,
    skins: Vec<BevyIndexedLabelV1>,
    inverse_bind_matrices: Vec<BevyIndexedLabelV1>,
    targets: Vec<BevyAnimationTargetV1>,
    warnings: Vec<BevyWarningV1>,
    warnings_truncated: bool,
    primary_verified: bool,
    dependencies_verified: bool,
}
impl BevyObservationV1 {
    /// Construct one observation; V1 validation enforces its bounds and ordering.
    ///
    /// The direct arguments intentionally mirror the closed, flat V1 wire
    /// fields. Grouping them would add a one-use public payload solely to
    /// silence this lint, without a separate contract meaning.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        terminal: BevyTerminalStateV1,
        animations: Vec<BevyIndexedLabelV1>,
        named_animation_winners: Vec<BevyNamedWinnerV1>,
        named_scene_winners: Vec<BevyNamedWinnerV1>,
        named_skin_winners: Vec<BevyNamedWinnerV1>,
        default_scene: Option<u32>,
        scenes: Vec<BevyIndexedLabelV1>,
        nodes: Vec<BevyIndexedLabelV1>,
        skins: Vec<BevyIndexedLabelV1>,
        inverse_bind_matrices: Vec<BevyIndexedLabelV1>,
        targets: Vec<BevyAnimationTargetV1>,
        warnings: Vec<BevyWarningV1>,
        warnings_truncated: bool,
        primary_verified: bool,
        dependencies_verified: bool,
    ) -> Self {
        Self {
            terminal,
            animations,
            named_animation_winners,
            named_scene_winners,
            named_skin_winners,
            default_scene,
            scenes,
            nodes,
            skins,
            inverse_bind_matrices,
            targets,
            warnings,
            warnings_truncated,
            primary_verified,
            dependencies_verified,
        }
    }
    /// Whether one or more warning events arrived after the bounded capture cap.
    pub const fn warnings_truncated(&self) -> bool {
        self.warnings_truncated
    }
}

/// A closed reason prediction and observation differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BevyConformanceCodeV1 {
    /// Observed primary bytes do not match V2.
    InputIdentityMismatch,
    /// Strict V2 document differs from its readback reference.
    PredictionDocumentMismatch,
    /// Exact Bevy revision-3 adapter is absent.
    BevyAdapterUnavailable,
    /// V4 provenance header differs.
    ProvenanceMismatch,
    /// Closure verification did not succeed.
    DependencyIdentityMismatch,
    /// A required prediction facet is unavailable.
    RequiredPredictionUnavailable,
    /// Loading did not reach `Loaded`.
    LoadDidNotSucceed,
    /// The compiled feature or loader setting differs.
    SettingsMismatch,
    /// Typed inventory differs.
    InventoryMismatch,
    /// Typed scene inventory differs.
    SceneMismatch,
    /// Default-scene route differs.
    DefaultSceneMismatch,
    /// Typed skin or inverse-bind inventory differs.
    SkinMismatch,
    /// Named winner differs.
    NamedWinnerMismatch,
    /// Animation target differs.
    TargetMismatch,
}

/// Result of exact comparison with the independently strict-read V2 report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum BevyConformanceV1 {
    /// Every applicable available fact agreed.
    Exact,
    /// One or more facts differed or could not be predicted.
    NotExact {
        /// Canonically ordered concrete disagreement reasons.
        mismatch_codes: Vec<BevyConformanceCodeV1>,
        /// Canonically ordered unavailable prediction reasons.
        unavailable_codes: Vec<BevyConformanceCodeV1>,
    },
}

/// Closed standalone root with a canonical self-identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BevyReadbackV1 {
    schema_version: u32,
    schema: String,
    identity: InputIdentity,
    harness: BevyHarnessIdentityV1,
    input: InputIdentity,
    prediction: BevyPredictionReferenceV1,
    observation: BevyObservationV1,
    conformance: BevyConformanceV1,
}
impl BevyReadbackV1 {
    /// Construct V1 and calculate its canonical self-identity.
    pub fn new(
        harness: BevyHarnessIdentityV1,
        input: InputIdentity,
        prediction: BevyPredictionReferenceV1,
        observation: BevyObservationV1,
        conformance: BevyConformanceV1,
    ) -> Result<Self, BevyReadbackV1Error> {
        let mut value = Self {
            schema_version: 1,
            schema: BEVY_READBACK_V1_ID.into(),
            identity: InputIdentity::from_bytes(&[]),
            harness,
            input,
            prediction,
            observation,
            conformance,
        };
        value.identity = value.computed_identity()?;
        value.validate()?;
        Ok(value)
    }
    /// Exact asset identity actually read before loading.
    pub const fn input(&self) -> &InputIdentity {
        &self.input
    }
    /// Typed engine observations.
    pub const fn observation(&self) -> &BevyObservationV1 {
        &self.observation
    }
    /// Comparison result.
    pub const fn conformance(&self) -> &BevyConformanceV1 {
        &self.conformance
    }
    fn computed_identity(&self) -> Result<InputIdentity, BevyReadbackV1Error> {
        serde_jcs::to_vec(&Seed {
            schema_version: self.schema_version,
            schema: &self.schema,
            harness: &self.harness,
            input: &self.input,
            prediction: &self.prediction,
            observation: &self.observation,
            conformance: &self.conformance,
        })
        .map(|bytes| InputIdentity::from_bytes(&bytes))
        .map_err(BevyReadbackV1Error::Canonical)
    }
    /// Validate fixed tuple, canonical identity, bounds, and closed state.
    pub fn validate(&self) -> Result<(), BevyReadbackV1Error> {
        if self.schema_version != 1 || self.schema != BEVY_READBACK_V1_ID {
            return Err(BevyReadbackV1Error::Contract("invalid schema header"));
        }
        let h = &self.harness;
        if h.engine != "bevy"
            || h.engine_version != "0.19.0"
            || !safe(&h.tool_version)
            || h.rust_toolchain != BEVY_READBACK_V1_RUSTC
            || !h.bevy_animation_feature
            || !h.load_animations
            || h.lock_identity.bytes() != BEVY_READBACK_V1_LOCK_BYTES
            || h.lock_identity.sha256() != BEVY_READBACK_V1_LOCK_SHA256
            || h.updates > BEVY_READBACK_V1_MAX_UPDATES
        {
            return Err(BevyReadbackV1Error::Contract(
                "invalid frozen harness tuple",
            ));
        }
        if self.prediction.provenance_schema != animsmith_core::PREDICTION_PROVENANCE_V4_ID {
            return Err(BevyReadbackV1Error::Contract(
                "invalid V4 provenance header",
            ));
        }
        validate_observation(&self.observation)?;
        validate_conformance(&self.conformance)?;
        if self.identity != self.computed_identity()? {
            return Err(BevyReadbackV1Error::Contract("self identity mismatch"));
        }
        Ok(())
    }
}
#[derive(Serialize)]
struct Seed<'a> {
    schema_version: u32,
    schema: &'a str,
    harness: &'a BevyHarnessIdentityV1,
    input: &'a InputIdentity,
    prediction: &'a BevyPredictionReferenceV1,
    observation: &'a BevyObservationV1,
    conformance: &'a BevyConformanceV1,
}

/// Bounded strict-reader failure for Bevy readback V1.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum BevyReadbackV1Error {
    /// Input stream cannot be read.
    #[error("cannot read Bevy readback: {0}")]
    Io(#[from] std::io::Error),
    /// Input exceeds the immutable limit.
    #[error("Bevy readback exceeds byte limit {limit}")]
    ReportTooLarge {
        /// Immutable byte limit.
        limit: u64,
    },
    /// JSON shape is not closed V1.
    #[error("invalid Bevy readback JSON: {0}")]
    Json(#[from] serde_json::Error),
    /// Canonical serializer failed.
    #[error("cannot canonicalize Bevy readback: {0}")]
    Canonical(serde_json::Error),
    /// A semantic invariant failed.
    #[error("invalid Bevy readback V1 contract: {0}")]
    Contract(&'static str),
}

/// Strictly read one complete bounded V1 document.
pub fn validate_bevy_readback_v1(reader: impl Read) -> Result<BevyReadbackV1, BevyReadbackV1Error> {
    let mut bytes = Vec::new();
    reader
        .take(BEVY_READBACK_V1_MAX_REPORT_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > BEVY_READBACK_V1_MAX_REPORT_BYTES {
        return Err(BevyReadbackV1Error::ReportTooLarge {
            limit: BEVY_READBACK_V1_MAX_REPORT_BYTES,
        });
    }
    let value: BevyReadbackV1 = serde_json::from_slice(&bytes)?;
    value.validate()?;
    Ok(value)
}

/// Verify a stored V1 result against its independently strict-read V2 input.
pub fn validate_bevy_readback_prediction_v1(
    readback: &BevyReadbackV1,
    prediction: &GltfAddressabilityReadbackV2,
    prediction_input: &InputIdentity,
) -> Result<(), BevyReadbackV1Error> {
    readback.validate()?;
    if compare_bevy_readback_v1(readback, prediction, prediction_input) != readback.conformance {
        return Err(BevyReadbackV1Error::Contract(
            "stored conformance differs from recomputation",
        ));
    }
    Ok(())
}

/// Compare a V1 readback with an independently strict-read V2 prediction.
pub fn compare_bevy_readback_v1(
    readback: &BevyReadbackV1,
    prediction: &GltfAddressabilityReadbackV2,
    prediction_input: &InputIdentity,
) -> BevyConformanceV1 {
    let mut mismatch = Vec::new();
    let mut unavailable = Vec::new();
    if readback.input != *prediction.input() || !readback.observation.primary_verified {
        mismatch.push(BevyConformanceCodeV1::InputIdentityMismatch);
    }
    if readback.prediction.prediction_input != *prediction_input {
        mismatch.push(BevyConformanceCodeV1::PredictionDocumentMismatch);
    }
    let Some(adapter) = prediction.bevy() else {
        return BevyConformanceV1::NotExact {
            mismatch_codes: vec![BevyConformanceCodeV1::BevyAdapterUnavailable],
            unavailable_codes: Vec::new(),
        };
    };
    let provenance = adapter.prediction_provenance();
    if readback.prediction.provenance_schema != provenance.contract_id()
        || readback.prediction.provenance_identity != *provenance.identity().input_identity()
    {
        mismatch.push(BevyConformanceCodeV1::ProvenanceMismatch);
    }
    if !readback.observation.dependencies_verified {
        mismatch.push(BevyConformanceCodeV1::DependencyIdentityMismatch);
    }
    if !matches!(readback.observation.terminal, BevyTerminalStateV1::Loaded) {
        mismatch.push(BevyConformanceCodeV1::LoadDidNotSucceed);
    }
    let projection = adapter.projection();
    if readback.harness.bevy_animation_feature != adapter.settings().bevy_animation_feature()
        || readback.harness.load_animations != adapter.settings().load_animations()
    {
        mismatch.push(BevyConformanceCodeV1::SettingsMismatch);
    }
    if prediction
        .inventory()
        .animations()
        .animations()
        .coverage()
        .state()
        != GltfAnimationCoverageStateV1::Complete
    {
        unavailable.push(BevyConformanceCodeV1::RequiredPredictionUnavailable);
    }
    if matches!(
        projection.target_coverage(),
        GltfAddressabilityProjectionV2::RequiredUnavailable { .. }
    ) {
        unavailable.push(BevyConformanceCodeV1::RequiredPredictionUnavailable);
    }
    let expected_animations = prediction
        .inventory()
        .animations()
        .animations()
        .rows()
        .iter()
        .map(|row| {
            BevyIndexedLabelV1::new(
                row.source_clip_index() as u32,
                format!("Animation{}", row.source_clip_index()),
            )
        })
        .collect::<Vec<_>>();
    if expected_animations != readback.observation.animations {
        mismatch.push(BevyConformanceCodeV1::InventoryMismatch);
    }
    let raw = prediction.inventory().raw();
    if raw.scene_coverage().is_complete() {
        let expected = raw
            .scenes()
            .iter()
            .map(|row| {
                BevyIndexedLabelV1::new(
                    row.source_scene_index() as u32,
                    format!("Scene{}", row.source_scene_index()),
                )
            })
            .collect::<Vec<_>>();
        if expected != readback.observation.scenes {
            mismatch.push(BevyConformanceCodeV1::SceneMismatch);
        }
    } else {
        unavailable.push(BevyConformanceCodeV1::RequiredPredictionUnavailable);
    }
    if raw.node_coverage().is_complete() {
        let expected = raw
            .nodes()
            .iter()
            .map(|row| {
                BevyIndexedLabelV1::new(
                    row.source_node_index() as u32,
                    format!("Node{}", row.source_node_index()),
                )
            })
            .collect::<Vec<_>>();
        if expected != readback.observation.nodes {
            mismatch.push(BevyConformanceCodeV1::InventoryMismatch);
        }
    } else {
        unavailable.push(BevyConformanceCodeV1::RequiredPredictionUnavailable);
    }
    let mut expected_skins = Vec::new();
    for skin in projection.skins() {
        match skin.skin_label() {
            GltfAddressabilityProjectionV2::Available { value } => {
                expected_skins.push(BevyIndexedLabelV1::new(
                    skin.source_skin_index() as u32,
                    value.clone(),
                ));
            }
            GltfAddressabilityProjectionV2::ProvenAbsent => {}
            GltfAddressabilityProjectionV2::RequiredUnavailable { .. } => {
                unavailable.push(BevyConformanceCodeV1::RequiredPredictionUnavailable);
            }
        }
    }
    if expected_skins != readback.observation.skins {
        mismatch.push(BevyConformanceCodeV1::SkinMismatch);
    }
    match projection.default_scene_route() {
        GltfAddressabilityProjectionV2::Available { value } => {
            if parse_label_index(value, "Scene") != readback.observation.default_scene {
                mismatch.push(BevyConformanceCodeV1::DefaultSceneMismatch);
            }
        }
        GltfAddressabilityProjectionV2::RequiredUnavailable { .. } => {
            unavailable.push(BevyConformanceCodeV1::RequiredPredictionUnavailable)
        }
        GltfAddressabilityProjectionV2::ProvenAbsent => {
            if readback.observation.default_scene.is_some() {
                mismatch.push(BevyConformanceCodeV1::DefaultSceneMismatch);
            }
        }
    }
    let expected_inverse = projection
        .skins()
        .iter()
        // V3 makes inverse-bind labels eager for every source skin, even
        // when its conditional `Skin{i}` asset label is absent because no
        // node attaches that skin.
        .map(|skin| {
            BevyIndexedLabelV1::new(
                skin.source_skin_index() as u32,
                skin.inverse_bind_matrices_label().into(),
            )
        })
        .collect::<Vec<_>>();
    if expected_inverse != readback.observation.inverse_bind_matrices {
        mismatch.push(BevyConformanceCodeV1::SkinMismatch);
    }
    let expected_named = projection
        .named_maps()
        .iter()
        .find(|map| map.kind() == GltfAddressabilityNamedMapKindV2::Animation)
        .and_then(|map| match map.winners() {
            GltfAddressabilityProjectionV2::Available { value } => Some(
                value
                    .iter()
                    .map(|row| BevyNamedWinnerV1::new(row.name().into(), row.source_index() as u32))
                    .collect::<Vec<_>>(),
            ),
            GltfAddressabilityProjectionV2::ProvenAbsent => Some(Vec::new()),
            GltfAddressabilityProjectionV2::RequiredUnavailable { .. } => None,
        });
    match expected_named {
        Some(expected) if expected != readback.observation.named_animation_winners => {
            mismatch.push(BevyConformanceCodeV1::NamedWinnerMismatch)
        }
        None => unavailable.push(BevyConformanceCodeV1::RequiredPredictionUnavailable),
        _ => {}
    }
    for (kind, observed) in [
        (
            GltfAddressabilityNamedMapKindV2::Scene,
            &readback.observation.named_scene_winners,
        ),
        (
            GltfAddressabilityNamedMapKindV2::Skin,
            &readback.observation.named_skin_winners,
        ),
    ] {
        let expected = projection
            .named_maps()
            .iter()
            .find(|map| map.kind() == kind)
            .and_then(|map| match map.winners() {
                GltfAddressabilityProjectionV2::Available { value } => Some(
                    value
                        .iter()
                        .map(|row| {
                            BevyNamedWinnerV1::new(row.name().into(), row.source_index() as u32)
                        })
                        .collect::<Vec<_>>(),
                ),
                GltfAddressabilityProjectionV2::ProvenAbsent => Some(Vec::new()),
                GltfAddressabilityProjectionV2::RequiredUnavailable { .. } => None,
            });
        match expected {
            Some(expected) if expected != *observed => {
                mismatch.push(BevyConformanceCodeV1::NamedWinnerMismatch)
            }
            None => unavailable.push(BevyConformanceCodeV1::RequiredPredictionUnavailable),
            _ => {}
        }
    }
    let mut expected_targets = Vec::new();
    for target in projection.targets() {
        match target.projection() {
            GltfAddressabilityProjectionV2::Available { value } => {
                for channel in target.contributing_channels() {
                    expected_targets.push(BevyAnimationTargetV1::new(
                        channel.source_animation_index() as u32,
                        value.uuid().into(),
                    ));
                }
            }
            GltfAddressabilityProjectionV2::RequiredUnavailable { .. } => {
                unavailable.push(BevyConformanceCodeV1::RequiredPredictionUnavailable)
            }
            GltfAddressabilityProjectionV2::ProvenAbsent => {}
        }
    }
    expected_targets
        .sort_by(|a, b| (a.animation_index, &a.target_id).cmp(&(b.animation_index, &b.target_id)));
    expected_targets.dedup();
    if expected_targets != readback.observation.targets {
        mismatch.push(BevyConformanceCodeV1::TargetMismatch);
    }
    mismatch.sort();
    mismatch.dedup();
    unavailable.sort();
    unavailable.dedup();
    if mismatch.is_empty() && unavailable.is_empty() {
        BevyConformanceV1::Exact
    } else {
        BevyConformanceV1::NotExact {
            mismatch_codes: mismatch,
            unavailable_codes: unavailable,
        }
    }
}

fn parse_label_index(value: &str, prefix: &str) -> Option<u32> {
    value.strip_prefix(prefix)?.parse().ok()
}

fn validate_observation(value: &BevyObservationV1) -> Result<(), BevyReadbackV1Error> {
    if [
        value.animations.len(),
        value.named_animation_winners.len(),
        value.named_scene_winners.len(),
        value.named_skin_winners.len(),
        value.scenes.len(),
        value.nodes.len(),
        value.skins.len(),
        value.inverse_bind_matrices.len(),
        value.targets.len(),
        value.warnings.len(),
    ]
    .iter()
    .any(|n| *n > BEVY_READBACK_V1_MAX_ROWS)
    {
        return Err(BevyReadbackV1Error::Contract("domain row limit"));
    }
    if value.animations.len()
        + value.named_animation_winners.len()
        + value.named_scene_winners.len()
        + value.named_skin_winners.len()
        + value.scenes.len()
        + value.nodes.len()
        + value.skins.len()
        + value.inverse_bind_matrices.len()
        + value.targets.len()
        + value.warnings.len()
        > BEVY_READBACK_V1_MAX_WORK
    {
        return Err(BevyReadbackV1Error::Contract("aggregate work limit"));
    }
    for rows in [
        &value.animations,
        &value.scenes,
        &value.nodes,
        &value.skins,
        &value.inverse_bind_matrices,
    ] {
        if !rows.windows(2).all(|p| p[0].index < p[1].index)
            || rows.iter().any(|row| !safe(&row.label))
        {
            return Err(BevyReadbackV1Error::Contract("noncanonical indexed rows"));
        }
    }
    for winners in [
        &value.named_animation_winners,
        &value.named_scene_winners,
        &value.named_skin_winners,
    ] {
        if !winners
            .windows(2)
            .all(|p| (&p[0].name, p[0].index) < (&p[1].name, p[1].index))
            || winners.iter().any(|row| !safe(&row.name))
        {
            return Err(BevyReadbackV1Error::Contract("noncanonical named winners"));
        }
    }
    if !value
        .targets
        .windows(2)
        .all(|p| (p[0].animation_index, &p[0].target_id) < (p[1].animation_index, &p[1].target_id))
        || value.targets.iter().any(|row| !safe(&row.target_id))
    {
        return Err(BevyReadbackV1Error::Contract("noncanonical targets"));
    }
    if !value
        .warnings
        .windows(2)
        .all(|p| (&p[0].target, &p[0].level) < (&p[1].target, &p[1].level))
        || value
            .warnings
            .iter()
            .any(|row| !safe(&row.target) || !safe(&row.level))
    {
        return Err(BevyReadbackV1Error::Contract("noncanonical warnings"));
    }
    if !matches!(value.terminal, BevyTerminalStateV1::Loaded)
        && (!value.animations.is_empty()
            || !value.named_animation_winners.is_empty()
            || !value.named_scene_winners.is_empty()
            || !value.named_skin_winners.is_empty()
            || value.default_scene.is_some()
            || !value.scenes.is_empty()
            || !value.nodes.is_empty()
            || !value.skins.is_empty()
            || !value.inverse_bind_matrices.is_empty()
            || !value.targets.is_empty())
    {
        return Err(BevyReadbackV1Error::Contract("failed load has inventory"));
    }
    Ok(())
}
fn validate_conformance(value: &BevyConformanceV1) -> Result<(), BevyReadbackV1Error> {
    match value {
        BevyConformanceV1::Exact => Ok(()),
        BevyConformanceV1::NotExact {
            mismatch_codes,
            unavailable_codes,
        } if !(mismatch_codes.is_empty() && unavailable_codes.is_empty())
            && mismatch_codes.windows(2).all(|p| p[0] < p[1])
            && unavailable_codes.windows(2).all(|p| p[0] < p[1]) =>
        {
            Ok(())
        }
        _ => Err(BevyReadbackV1Error::Contract("noncanonical conformance")),
    }
}
fn safe(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_TEXT_BYTES && !value.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GltfAddressabilityV2;

    fn frozen_lock_identity() -> InputIdentity {
        InputIdentity::from_sha256_digest(
            [
                0xb6, 0x74, 0xe2, 0xf7, 0x4d, 0xaf, 0xf0, 0xa0, 0x81, 0x5b, 0xfb, 0xbc, 0xe2, 0x58,
                0xcb, 0xb7, 0x61, 0x81, 0xc1, 0x8b, 0x39, 0xee, 0x5b, 0x64, 0x99, 0xc1, 0x2a, 0x06,
                0x4a, 0x80, 0x9b, 0x18,
            ],
            BEVY_READBACK_V1_LOCK_BYTES,
        )
    }

    fn valid_readback(warnings_truncated: bool) -> BevyReadbackV1 {
        BevyReadbackV1::new(
            BevyHarnessIdentityV1::new(
                "0.1.0".into(),
                BEVY_READBACK_V1_RUSTC.into(),
                true,
                true,
                frozen_lock_identity(),
                1,
            ),
            InputIdentity::from_bytes(b"asset"),
            BevyPredictionReferenceV1::new(
                InputIdentity::from_bytes(b"p"),
                animsmith_core::PREDICTION_PROVENANCE_V4_ID.into(),
                InputIdentity::from_bytes(b"q"),
            ),
            BevyObservationV1::new(
                BevyTerminalStateV1::Loaded,
                vec![],
                vec![],
                vec![],
                vec![],
                None,
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
                warnings_truncated,
                true,
                true,
            ),
            BevyConformanceV1::Exact,
        )
        .unwrap()
    }

    #[test]
    fn invalid_harness_cannot_form_a_readback() {
        let v = BevyReadbackV1::new(
            BevyHarnessIdentityV1::new(
                "0.1.0".into(),
                BEVY_READBACK_V1_RUSTC.into(),
                true,
                true,
                InputIdentity::from_bytes(b"not-the-tool-lock"),
                1,
            ),
            InputIdentity::from_bytes(b"asset"),
            BevyPredictionReferenceV1::new(
                InputIdentity::from_bytes(b"p"),
                animsmith_core::PREDICTION_PROVENANCE_V4_ID.into(),
                InputIdentity::from_bytes(b"q"),
            ),
            BevyObservationV1::new(
                BevyTerminalStateV1::Loaded,
                vec![],
                vec![],
                vec![],
                vec![],
                None,
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
                false,
                true,
                true,
            ),
            BevyConformanceV1::Exact,
        );
        assert!(v.is_err());
    }

    #[test]
    fn warning_truncation_is_identity_bound_and_strictly_read() {
        let readback = valid_readback(true);
        assert!(readback.observation().warnings_truncated());
        let mut value = serde_json::to_value(readback).unwrap();
        value["observation"]["warnings_truncated"] = serde_json::Value::Bool(false);
        assert!(validate_bevy_readback_v1(serde_json::to_vec(&value).unwrap().as_slice()).is_err());
    }

    #[test]
    fn strict_reader_rejects_mutated_public_contract_fields() {
        let source = serde_json::to_value(valid_readback(false)).unwrap();
        for (pointer, replacement) in [
            ("/schema_version", serde_json::json!(2)),
            ("/schema", serde_json::json!("urn:animsmith:schema:other:1")),
            ("/harness/engine", serde_json::json!("other")),
            ("/harness/tool_version", serde_json::json!("0.1.1")),
            ("/harness/rust_toolchain", serde_json::json!("rustc 1.95.0")),
            ("/harness/load_animations", serde_json::json!(false)),
            ("/harness/lock_identity/sha256", serde_json::json!("00")),
            ("/input/sha256", serde_json::json!("00")),
            ("/prediction/provenance_schema", serde_json::json!("other")),
            (
                "/observation/terminal",
                serde_json::json!({"state":"work_limit"}),
            ),
            (
                "/observation/animations",
                serde_json::json!([{"index": 0, "label": "Animation0"}]),
            ),
        ] {
            let mut changed = source.clone();
            *changed.pointer_mut(pointer).unwrap() = replacement;
            let bytes = serde_json::to_vec(&changed).unwrap();
            assert!(
                validate_bevy_readback_v1(bytes.as_slice()).is_err(),
                "{pointer}"
            );
        }
        let bytes = serde_json::to_vec(&source).unwrap();
        assert!(GltfAddressabilityV2::read_from(bytes.as_slice()).is_err());
    }

    #[test]
    fn strict_reader_then_prediction_recomputation_rejects_wrong_conformance() {
        let mut prediction_json = crate::addressability_v2::tests::adapter_report_json(Some(
            crate::TargetPointerWidth::Bits64,
        ));
        prediction_json["bevy"] = serde_json::Value::Null;
        let prediction_bytes = serde_json::to_vec(&prediction_json).unwrap();
        let prediction_input = InputIdentity::from_bytes(&prediction_bytes);
        let prediction = GltfAddressabilityV2::read_from(prediction_bytes.as_slice()).unwrap();
        let deliberately_wrong = BevyReadbackV1::new(
            BevyHarnessIdentityV1::new(
                "0.1.0".into(),
                BEVY_READBACK_V1_RUSTC.into(),
                true,
                true,
                frozen_lock_identity(),
                1,
            ),
            prediction.input().clone(),
            BevyPredictionReferenceV1::new(
                prediction_input.clone(),
                animsmith_core::PREDICTION_PROVENANCE_V4_ID.into(),
                InputIdentity::from_bytes(b"adapter unavailable has no V4 reference"),
            ),
            BevyObservationV1::new(
                BevyTerminalStateV1::Loaded,
                vec![],
                vec![],
                vec![],
                vec![],
                None,
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
                false,
                true,
                true,
            ),
            BevyConformanceV1::Exact,
        )
        .unwrap();
        let bytes = serde_json::to_vec(&deliberately_wrong).unwrap();
        let strict = validate_bevy_readback_v1(bytes.as_slice()).unwrap();
        assert_eq!(
            compare_bevy_readback_v1(&strict, &prediction, &prediction_input),
            BevyConformanceV1::NotExact {
                mismatch_codes: vec![BevyConformanceCodeV1::BevyAdapterUnavailable],
                unavailable_codes: vec![],
            }
        );
        assert!(
            validate_bevy_readback_prediction_v1(&strict, &prediction, &prediction_input).is_err()
        );
    }

    #[test]
    fn terminal_failure_readbacks_are_strict_and_non_exact() {
        for terminal in [
            BevyTerminalStateV1::RootFailure {
                error: BevyLoadErrorCodeV1::AssetReader,
            },
            BevyTerminalStateV1::DependencyFailure {
                error: BevyLoadErrorCodeV1::AssetReader,
            },
        ] {
            let readback = BevyReadbackV1::new(
                BevyHarnessIdentityV1::new(
                    "0.1.0".into(),
                    BEVY_READBACK_V1_RUSTC.into(),
                    true,
                    true,
                    frozen_lock_identity(),
                    1,
                ),
                InputIdentity::from_bytes(b"asset"),
                BevyPredictionReferenceV1::new(
                    InputIdentity::from_bytes(b"p"),
                    animsmith_core::PREDICTION_PROVENANCE_V4_ID.into(),
                    InputIdentity::from_bytes(b"q"),
                ),
                BevyObservationV1::new(
                    terminal,
                    vec![],
                    vec![],
                    vec![],
                    vec![],
                    None,
                    vec![],
                    vec![],
                    vec![],
                    vec![],
                    vec![],
                    vec![],
                    false,
                    true,
                    true,
                ),
                BevyConformanceV1::NotExact {
                    mismatch_codes: vec![BevyConformanceCodeV1::LoadDidNotSucceed],
                    unavailable_codes: vec![],
                },
            )
            .unwrap();
            let bytes = serde_json::to_vec(&readback).unwrap();
            let parsed = validate_bevy_readback_v1(bytes.as_slice()).unwrap();
            assert!(matches!(
                parsed.conformance(),
                BevyConformanceV1::NotExact { .. }
            ));
            assert!(matches!(
                parsed.observation.terminal,
                BevyTerminalStateV1::RootFailure { .. }
                    | BevyTerminalStateV1::DependencyFailure { .. }
            ));
        }
    }
}
