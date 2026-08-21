//! Bounded glTF animation inventory and exact Bevy addressability envelope.
//!
//! This contract projects only animation declarations already retained by the
//! immutable raw-source facts sidecar. It deliberately has no scene, skin,
//! extension-policy, named-map, or animation-target-path vocabulary.

use animsmith_core::evaluation::{
    Applicability, CheckEvaluation, ConfigurationState, EvaluationScope, EvaluationState,
    SelectionState,
};
use animsmith_core::{
    DependencyClosureV1, EngineAnimationAddressabilityV1, EngineFactIdV1, EngineFactStateV1,
    EngineFactValueV1, EnginePredictionFacetStateV1, EnginePredictionV1, InputIdentity,
    LoadedSource, PredictionBasisReferenceV1, PredictionContractError, PredictionProvenanceV1,
    PredictionScalarV1, PredictionUnavailableReasonV1, RawSourceDomainV1, RawSourceKeyV1,
    RawSourceSetCoverageStateV1, RawSourceUnavailableReasonV1, SourceChannelPropertyV1,
    SourceFormatV1, SourceObservationStateV1, SourceSetCoverageStateV1, SourceTargetKindV1,
    SourceUnavailableReasonV1, ToolInfo,
};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::value::RawValue;
use std::io::Read;

/// Version of the standalone glTF animation-addressability envelope.
pub const GLTF_ANIMATION_ADDRESSABILITY_SCHEMA_VERSION: u32 = 1;
/// Immutable identity of the standalone glTF animation-addressability contract.
pub const GLTF_ANIMATION_ADDRESSABILITY_V1_ID: &str =
    "urn:animsmith:schema:gltf-animation-addressability:1";
/// Immutable command discriminator carried by the standalone envelope.
pub const GLTF_ANIMATION_ADDRESSABILITY_COMMAND: &str = "generate-addressability";
/// Maximum serialized bytes accepted by the standalone report reader.
pub const GLTF_ANIMATION_ADDRESSABILITY_V1_MAX_REPORT_BYTES: u64 = 256 * 1024 * 1024;

const INVENTORY_IDENTITY_DOMAIN: &str = "animsmith-gltf-animation-addressability-inventory-v1";
const ENGINE_ADDRESSABILITY_CHECK_ID: &str = "engine-addressability";
const BEVY_FAMILY: &str = "bevy";
const BEVY_PROFILE_REVISION: u32 = 1;
const BEVY_ENGINE_VERSION: &str = "0.19.0";
const BEVY_IMPORTER: &str = "gltf-asset-loader";
const BEVY_ANIMATION_LABEL_SOURCE: &str = "bevy-gltf-asset-label-0.19.0";
const BEVY_PROFILE_FACTS_SHA256: &str =
    "873b98e896f05de73d5ea30560a4555c1f93650beeec9ed929e30dbcf7ce8c1e";
const BEVY_PROFILE_FACTS_CANONICAL_BYTES: u64 = 1_642;

/// Domain-separated identity of one engine-neutral animation inventory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GltfAnimationAddressabilityIdentityV1(InputIdentity);

impl GltfAnimationAddressabilityIdentityV1 {
    /// SHA-256 and byte count of the canonical inventory preimage.
    pub const fn input_identity(&self) -> &InputIdentity {
        &self.0
    }
}

/// Stable reason why a raw animation observation is incomplete.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GltfAnimationUnavailableReasonV1 {
    /// The source declaration is malformed.
    Malformed,
    /// The loader discarded the source value.
    Discarded,
    /// Normalization removed the original form.
    NormalizedAway,
    /// Animation baking removed the original form.
    BakedAway,
    /// The loader does not model the source domain.
    LoaderUnsupported,
    /// The deterministic raw-source budget was exhausted.
    ProjectionBudgetExceeded,
    /// The parser did not make the evidence available.
    ParserUnavailable,
}

impl From<SourceUnavailableReasonV1> for GltfAnimationUnavailableReasonV1 {
    fn from(value: SourceUnavailableReasonV1) -> Self {
        match value {
            SourceUnavailableReasonV1::Malformed => Self::Malformed,
            SourceUnavailableReasonV1::Discarded => Self::Discarded,
            SourceUnavailableReasonV1::NormalizedAway => Self::NormalizedAway,
            SourceUnavailableReasonV1::BakedAway => Self::BakedAway,
            SourceUnavailableReasonV1::LoaderUnsupported => Self::LoaderUnsupported,
            SourceUnavailableReasonV1::ProjectionBudgetExceeded => Self::ProjectionBudgetExceeded,
            SourceUnavailableReasonV1::ParserUnavailable => Self::ParserUnavailable,
        }
    }
}

/// Exhaustiveness state of one raw animation row domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GltfAnimationCoverageStateV1 {
    /// Every source row is retained.
    Complete,
    /// Retained rows are an authoritative source-order prefix.
    Partial,
    /// No source rows are available.
    Unavailable,
}

/// Coverage and typed reason for one raw animation row domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GltfAnimationCoverageV1 {
    state: GltfAnimationCoverageStateV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<GltfAnimationUnavailableReasonV1>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GltfAnimationCoverageWireV1 {
    state: GltfAnimationCoverageStateV1,
    #[serde(default, deserialize_with = "deserialize_required_nullable")]
    reason: RequiredNullable<GltfAnimationUnavailableReasonV1>,
}

impl<'de> Deserialize<'de> for GltfAnimationCoverageV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = GltfAnimationCoverageWireV1::deserialize(deserializer)?;
        let reason = match (wire.state, wire.reason) {
            (GltfAnimationCoverageStateV1::Complete, RequiredNullable::Missing) => None,
            (
                GltfAnimationCoverageStateV1::Partial | GltfAnimationCoverageStateV1::Unavailable,
                RequiredNullable::Present(Some(reason)),
            ) => Some(reason),
            _ => {
                return Err(D::Error::custom(
                    GltfAnimationAddressabilityError::InvalidCoverage,
                ));
            }
        };
        let value = Self {
            state: wire.state,
            reason,
        };
        value.validate().map_err(D::Error::custom)?;
        Ok(value)
    }
}

impl GltfAnimationCoverageV1 {
    fn from_source(value: animsmith_core::SourceSetCoverageV1) -> Self {
        Self {
            state: match value.state() {
                SourceSetCoverageStateV1::Complete => GltfAnimationCoverageStateV1::Complete,
                SourceSetCoverageStateV1::Partial => GltfAnimationCoverageStateV1::Partial,
                SourceSetCoverageStateV1::Unavailable => GltfAnimationCoverageStateV1::Unavailable,
            },
            reason: value.reason().map(Into::into),
        }
    }

    /// Exhaustiveness state.
    pub const fn state(self) -> GltfAnimationCoverageStateV1 {
        self.state
    }

    /// Typed incompleteness reason, absent exactly for complete coverage.
    pub const fn reason(self) -> Option<GltfAnimationUnavailableReasonV1> {
        self.reason
    }

    fn validate(self) -> Result<(), GltfAnimationAddressabilityError> {
        if matches!(
            (self.state, self.reason),
            (GltfAnimationCoverageStateV1::Complete, None)
                | (
                    GltfAnimationCoverageStateV1::Partial
                        | GltfAnimationCoverageStateV1::Unavailable,
                    Some(_)
                )
        ) {
            Ok(())
        } else {
            Err(GltfAnimationAddressabilityError::InvalidCoverage)
        }
    }
}

/// Exact state of one scalar raw animation observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum GltfAnimationObservationV1<T> {
    /// The raw value was observed.
    Observed {
        /// Exact retained value.
        value: T,
    },
    /// Complete evidence proves that no source value was authored.
    ProvenAbsent,
    /// The raw value could not be established.
    Unavailable {
        /// Stable reason.
        reason: GltfAnimationUnavailableReasonV1,
    },
}

#[derive(Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
enum GltfAnimationObservationWireV1<T> {
    Observed {
        value: T,
    },
    ProvenAbsent,
    Unavailable {
        reason: GltfAnimationUnavailableReasonV1,
    },
}

impl<T> From<GltfAnimationObservationWireV1<T>> for GltfAnimationObservationV1<T> {
    fn from(value: GltfAnimationObservationWireV1<T>) -> Self {
        match value {
            GltfAnimationObservationWireV1::Observed { value } => Self::Observed { value },
            GltfAnimationObservationWireV1::ProvenAbsent => Self::ProvenAbsent,
            GltfAnimationObservationWireV1::Unavailable { reason } => Self::Unavailable { reason },
        }
    }
}

impl<T> GltfAnimationObservationV1<T> {
    fn from_source<U>(
        value: &animsmith_core::SourceObservationV1<U>,
        map: impl FnOnce(&U) -> T,
    ) -> Self {
        match value.state() {
            SourceObservationStateV1::Observed(value) => Self::Observed { value: map(value) },
            SourceObservationStateV1::ProvenAbsent => Self::ProvenAbsent,
            SourceObservationStateV1::Unavailable(reason) => Self::Unavailable {
                reason: (*reason).into(),
            },
        }
    }
}

/// Source target identity kind for one raw animation channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GltfAnimationTargetKindV1 {
    /// glTF source node-array identity.
    Node,
    /// Parser-stable non-node identity.
    Element,
    /// Another bounded source target domain.
    Other,
}

impl From<SourceTargetKindV1> for GltfAnimationTargetKindV1 {
    fn from(value: SourceTargetKindV1) -> Self {
        match value {
            SourceTargetKindV1::Node => Self::Node,
            SourceTargetKindV1::Element => Self::Element,
            SourceTargetKindV1::Other => Self::Other,
        }
    }
}

/// Source target identity for one raw animation channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GltfAnimationTargetV1 {
    kind: GltfAnimationTargetKindV1,
    index: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GltfAnimationTargetWireV1 {
    kind: GltfAnimationTargetKindV1,
    index: u64,
}

impl<'de> Deserialize<'de> for GltfAnimationTargetV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = GltfAnimationTargetWireV1::deserialize(deserializer)?;
        if wire.kind != GltfAnimationTargetKindV1::Node {
            return Err(D::Error::custom(
                GltfAnimationAddressabilityError::InvalidGltfTargetKind,
            ));
        }
        Ok(Self {
            kind: wire.kind,
            index: wire.index,
        })
    }
}

impl GltfAnimationTargetV1 {
    /// Source target identity kind.
    pub const fn kind(self) -> GltfAnimationTargetKindV1 {
        self.kind
    }

    /// Stable index in the source target domain.
    pub const fn index(self) -> u64 {
        self.index
    }
}

/// Format-neutral property targeted by one raw animation channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GltfAnimationChannelPropertyV1 {
    /// Local translation.
    Translation,
    /// Local rotation.
    Rotation,
    /// Local scale.
    Scale,
    /// Morph-target weights.
    Weights,
    /// Another bounded source property.
    Other,
}

impl From<SourceChannelPropertyV1> for GltfAnimationChannelPropertyV1 {
    fn from(value: SourceChannelPropertyV1) -> Self {
        match value {
            SourceChannelPropertyV1::Translation => Self::Translation,
            SourceChannelPropertyV1::Rotation => Self::Rotation,
            SourceChannelPropertyV1::Scale => Self::Scale,
            SourceChannelPropertyV1::Weights => Self::Weights,
            SourceChannelPropertyV1::Other => Self::Other,
        }
    }
}

/// One exact source-order raw animation channel row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GltfAnimationAddressabilityChannelV1 {
    source_channel_index: u64,
    target: GltfAnimationTargetV1,
    property: GltfAnimationChannelPropertyV1,
    input_accessor_index: u64,
    output_accessor_index: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GltfAnimationAddressabilityChannelWireV1 {
    source_channel_index: u64,
    target: GltfAnimationTargetV1,
    property: GltfAnimationChannelPropertyV1,
    input_accessor_index: u64,
    output_accessor_index: u64,
}

impl<'de> Deserialize<'de> for GltfAnimationAddressabilityChannelV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = GltfAnimationAddressabilityChannelWireV1::deserialize(deserializer)?;
        let limit = animsmith_core::RAW_SOURCE_V1_MAX_OBSERVATIONS - 1;
        if wire.source_channel_index >= limit as u64 {
            return Err(D::Error::custom(
                GltfAnimationAddressabilityError::SourceIndexOutOfRange {
                    field: "source channel index",
                    actual: wire.source_channel_index,
                    limit,
                },
            ));
        }
        Ok(Self {
            source_channel_index: wire.source_channel_index,
            target: wire.target,
            property: wire.property,
            input_accessor_index: wire.input_accessor_index,
            output_accessor_index: wire.output_accessor_index,
        })
    }
}

impl GltfAnimationAddressabilityChannelV1 {
    /// Exact source-order channel index within its animation.
    pub const fn source_channel_index(&self) -> u64 {
        self.source_channel_index
    }

    /// Exact source target identity.
    pub const fn target(&self) -> GltfAnimationTargetV1 {
        self.target
    }

    /// Raw channel property.
    pub const fn property(&self) -> GltfAnimationChannelPropertyV1 {
        self.property
    }

    /// Exact input accessor index retained for this glTF channel.
    pub const fn input_accessor_index(&self) -> u64 {
        self.input_accessor_index
    }

    /// Exact output accessor index retained for this glTF channel.
    pub const fn output_accessor_index(&self) -> u64 {
        self.output_accessor_index
    }
}

/// Coverage-qualified raw channel rows for one source animation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GltfAnimationAddressabilityChannelSetV1 {
    coverage: GltfAnimationCoverageV1,
    rows: Vec<GltfAnimationAddressabilityChannelV1>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GltfAnimationAddressabilityChannelSetWireV1 {
    coverage: GltfAnimationCoverageV1,
    rows: Vec<GltfAnimationAddressabilityChannelV1>,
}

impl<'de> Deserialize<'de> for GltfAnimationAddressabilityChannelSetV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = GltfAnimationAddressabilityChannelSetWireV1::deserialize(deserializer)?;
        if wire.coverage.state == GltfAnimationCoverageStateV1::Unavailable && !wire.rows.is_empty()
        {
            return Err(D::Error::custom(
                GltfAnimationAddressabilityError::RowsWithUnavailableCoverage,
            ));
        }
        if wire.rows.len() >= animsmith_core::RAW_SOURCE_V1_MAX_OBSERVATIONS {
            return Err(D::Error::custom(
                GltfAnimationAddressabilityError::TooManyRows {
                    found: wire.rows.len(),
                    limit: animsmith_core::RAW_SOURCE_V1_MAX_OBSERVATIONS - 1,
                },
            ));
        }
        for (offset, row) in wire.rows.iter().enumerate() {
            let expected = u64::try_from(offset)
                .map_err(|_| D::Error::custom("channel index cannot be represented as u64"))?;
            if row.source_channel_index != expected {
                return Err(D::Error::custom(
                    GltfAnimationAddressabilityError::NonCanonicalChannelIndex {
                        source_clip_index: 0,
                        expected,
                        actual: row.source_channel_index,
                    },
                ));
            }
        }
        Ok(Self {
            coverage: wire.coverage,
            rows: wire.rows,
        })
    }
}

impl GltfAnimationAddressabilityChannelSetV1 {
    /// Exhaustiveness and typed reason for this channel domain.
    pub const fn coverage(&self) -> GltfAnimationCoverageV1 {
        self.coverage
    }

    /// Retained source-order channel rows.
    pub fn rows(&self) -> &[GltfAnimationAddressabilityChannelV1] {
        &self.rows
    }
}

/// One exact source-order raw animation row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GltfAnimationAddressabilityAnimationV1 {
    source_clip_index: u64,
    source_name: GltfAnimationObservationV1<String>,
    normalized_clip_index: GltfAnimationObservationV1<u64>,
    channels: GltfAnimationAddressabilityChannelSetV1,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GltfAnimationAddressabilityAnimationWireV1 {
    source_clip_index: u64,
    source_name: GltfAnimationObservationWireV1<String>,
    normalized_clip_index: GltfAnimationObservationWireV1<u64>,
    channels: GltfAnimationAddressabilityChannelSetV1,
}

impl<'de> Deserialize<'de> for GltfAnimationAddressabilityAnimationV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = GltfAnimationAddressabilityAnimationWireV1::deserialize(deserializer)?;
        if wire.source_clip_index >= animsmith_core::RAW_SOURCE_V1_MAX_CLIPS as u64 {
            return Err(D::Error::custom(
                GltfAnimationAddressabilityError::SourceIndexOutOfRange {
                    field: "source clip index",
                    actual: wire.source_clip_index,
                    limit: animsmith_core::RAW_SOURCE_V1_MAX_CLIPS,
                },
            ));
        }
        if let GltfAnimationObservationWireV1::Observed { value } = &wire.source_name
            && value.len() > animsmith_core::RAW_SOURCE_V1_MAX_TEXT_BYTES
        {
            return Err(D::Error::custom(
                GltfAnimationAddressabilityError::TextTooLong {
                    found: value.len(),
                    limit: animsmith_core::RAW_SOURCE_V1_MAX_TEXT_BYTES,
                },
            ));
        }
        Ok(Self {
            source_clip_index: wire.source_clip_index,
            source_name: wire.source_name.into(),
            normalized_clip_index: wire.normalized_clip_index.into(),
            channels: wire.channels,
        })
    }
}

impl GltfAnimationAddressabilityAnimationV1 {
    /// Exact source animation-array index.
    pub const fn source_clip_index(&self) -> u64 {
        self.source_clip_index
    }

    /// Raw authored-name observation; names remain optional and non-unique.
    pub const fn source_name(&self) -> &GltfAnimationObservationV1<String> {
        &self.source_name
    }

    /// Mapping to the normalized clip table, separate from source identity.
    pub const fn normalized_clip_index(&self) -> &GltfAnimationObservationV1<u64> {
        &self.normalized_clip_index
    }

    /// Coverage-qualified raw channel rows.
    pub const fn channels(&self) -> &GltfAnimationAddressabilityChannelSetV1 {
        &self.channels
    }
}

/// Coverage-qualified source animation rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GltfAnimationAddressabilityAnimationSetV1 {
    coverage: GltfAnimationCoverageV1,
    rows: Vec<GltfAnimationAddressabilityAnimationV1>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GltfAnimationAddressabilityAnimationSetWireV1 {
    coverage: GltfAnimationCoverageV1,
    rows: Vec<GltfAnimationAddressabilityAnimationV1>,
}

impl<'de> Deserialize<'de> for GltfAnimationAddressabilityAnimationSetV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = GltfAnimationAddressabilityAnimationSetWireV1::deserialize(deserializer)?;
        let value = Self {
            coverage: wire.coverage,
            rows: wire.rows,
        };
        validate_animation_set(&value).map_err(D::Error::custom)?;
        Ok(value)
    }
}

impl GltfAnimationAddressabilityAnimationSetV1 {
    /// Exhaustiveness and typed reason for the source animation domain.
    pub const fn coverage(&self) -> GltfAnimationCoverageV1 {
        self.coverage
    }

    /// Retained source-order animation rows.
    pub fn rows(&self) -> &[GltfAnimationAddressabilityAnimationV1] {
        &self.rows
    }
}

/// Engine-neutral glTF animation inventory projected from one immutable load.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GltfAnimationAddressabilityInventoryV1 {
    schema: &'static str,
    identity: GltfAnimationAddressabilityIdentityV1,
    source_format: SourceFormatV1,
    primary_input: InputIdentity,
    dependency_closure: DependencyClosureV1,
    animations: GltfAnimationAddressabilityAnimationSetV1,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GltfAnimationAddressabilityInventoryWireV1 {
    schema: String,
    identity: GltfAnimationAddressabilityIdentityV1,
    source_format: SourceFormatV1,
    primary_input: InputIdentity,
    dependency_closure: DependencyClosureV1,
    animations: GltfAnimationAddressabilityAnimationSetV1,
}

impl<'de> Deserialize<'de> for GltfAnimationAddressabilityInventoryV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = GltfAnimationAddressabilityInventoryWireV1::deserialize(deserializer)?;
        let value = Self {
            schema: if wire.schema == GLTF_ANIMATION_ADDRESSABILITY_V1_ID {
                GLTF_ANIMATION_ADDRESSABILITY_V1_ID
            } else {
                return Err(D::Error::custom(format!(
                    "inventory schema must be {GLTF_ANIMATION_ADDRESSABILITY_V1_ID:?}"
                )));
            },
            identity: wire.identity,
            source_format: wire.source_format,
            primary_input: wire.primary_input,
            dependency_closure: wire.dependency_closure,
            animations: wire.animations,
        };
        value.validate().map_err(D::Error::custom)?;
        Ok(value)
    }
}

impl GltfAnimationAddressabilityInventoryV1 {
    /// Project one bounded engine-neutral animation inventory from a same-load source.
    ///
    /// # Errors
    ///
    /// Returns [`GltfAnimationAddressabilityError::UnsupportedSourceFormat`] for
    /// a non-glTF source. Loader-valid glTF/GLB sidecars already satisfy all
    /// other collection, text, ordering, and closure invariants.
    pub fn from_source(source: &LoadedSource) -> Result<Self, GltfAnimationAddressabilityError> {
        let facts = source.source_facts();
        require_gltf(facts.format())?;
        preflight_source(facts)?;

        let mut animations = Vec::with_capacity(facts.clips().rows().len());
        for clip in facts.clips().rows() {
            let mut channels = Vec::with_capacity(clip.channels().rows().len());
            for channel in clip.channels().rows() {
                let (input_accessor_index, output_accessor_index) = match (
                    channel.input_accessor_index(),
                    channel.output_accessor_index(),
                ) {
                    (Some(input), Some(output)) => (
                        to_u64("input accessor index", input)?,
                        to_u64("output accessor index", output)?,
                    ),
                    (None, None) => {
                        return Err(GltfAnimationAddressabilityError::MissingAccessorPair);
                    }
                    _ => {
                        return Err(GltfAnimationAddressabilityError::IncompleteAccessorPair);
                    }
                };
                channels.push(GltfAnimationAddressabilityChannelV1 {
                    source_channel_index: to_u64(
                        "source channel index",
                        channel.source_channel_index(),
                    )?,
                    target: GltfAnimationTargetV1 {
                        kind: channel.target().kind().into(),
                        index: channel.target().index(),
                    },
                    property: channel.property().into(),
                    input_accessor_index,
                    output_accessor_index,
                });
            }
            animations.push(GltfAnimationAddressabilityAnimationV1 {
                source_clip_index: to_u64("source clip index", clip.source_clip_index())?,
                source_name: GltfAnimationObservationV1::from_source(clip.source_name(), |value| {
                    value.as_str().to_owned()
                }),
                normalized_clip_index: map_index_observation(clip.normalized_clip_index())?,
                channels: GltfAnimationAddressabilityChannelSetV1 {
                    coverage: GltfAnimationCoverageV1::from_source(clip.channels().coverage()),
                    rows: channels,
                },
            });
        }

        let mut value = Self {
            schema: GLTF_ANIMATION_ADDRESSABILITY_V1_ID,
            identity: GltfAnimationAddressabilityIdentityV1(InputIdentity::from_bytes(&[])),
            source_format: facts.format(),
            primary_input: facts.primary_identity().clone(),
            dependency_closure: source.dependency_closure().clone(),
            animations: GltfAnimationAddressabilityAnimationSetV1 {
                coverage: GltfAnimationCoverageV1::from_source(facts.clips().coverage()),
                rows: animations,
            },
        };
        value.validate_without_identity()?;
        value.identity = GltfAnimationAddressabilityIdentityV1(value.computed_identity());
        Ok(value)
    }

    /// Immutable inventory contract identity.
    pub const fn contract_id(&self) -> &'static str {
        self.schema
    }

    /// Canonical identity over engine-neutral fields only.
    pub const fn identity(&self) -> &GltfAnimationAddressabilityIdentityV1 {
        &self.identity
    }

    /// Exact glTF or GLB source format.
    pub const fn source_format(&self) -> SourceFormatV1 {
        self.source_format
    }

    /// Exact primary input parsed by the loader.
    pub const fn primary_input(&self) -> &InputIdentity {
        &self.primary_input
    }

    /// Full same-load dependency-closure evidence.
    pub const fn dependency_closure(&self) -> &DependencyClosureV1 {
        &self.dependency_closure
    }

    /// Coverage-qualified raw animation inventory.
    pub const fn animations(&self) -> &GltfAnimationAddressabilityAnimationSetV1 {
        &self.animations
    }

    fn validate(&self) -> Result<(), GltfAnimationAddressabilityError> {
        self.validate_without_identity()?;
        if self.identity.0 != self.computed_identity() {
            return Err(GltfAnimationAddressabilityError::InventoryIdentityMismatch);
        }
        Ok(())
    }

    fn validate_without_identity(&self) -> Result<(), GltfAnimationAddressabilityError> {
        require_gltf(self.source_format)?;
        if self.dependency_closure.primary_input() != &self.primary_input {
            return Err(GltfAnimationAddressabilityError::DependencyClosurePrimaryMismatch);
        }
        validate_animation_set(&self.animations)
    }

    fn computed_identity(&self) -> InputIdentity {
        let closure_fingerprint = self.dependency_closure.record_identity();
        let mut encoder = CanonicalEncoder::new(INVENTORY_IDENTITY_DOMAIN);
        encoder.field("schema");
        encoder.token(self.schema);
        encoder.field("source_format");
        encoder.token(source_format_name(self.source_format));
        encoder.field("primary_input");
        encode_input_identity(&mut encoder, &self.primary_input);
        encoder.field("dependency_closure_fingerprint");
        encode_input_identity(&mut encoder, &closure_fingerprint);
        encode_animation_set(&mut encoder, &self.animations);
        encoder.identity()
    }
}

/// Exact optional Bevy adapter embedded in the standalone envelope.
#[derive(Debug, Clone, Serialize)]
pub struct GltfAnimationAddressabilityBevyAdapterV1 {
    prediction_provenance: PredictionProvenanceV1,
    check: CheckEvaluation,
}

impl GltfAnimationAddressabilityBevyAdapterV1 {
    /// Bind the unchanged `engine-addressability` evaluation to its exact provenance.
    ///
    /// # Errors
    ///
    /// Returns a typed contract error unless the provenance is the exact Bevy
    /// 0.19.0 glTF tuple and the check is the sole #154 lifecycle for this
    /// inventory.
    pub fn new(
        prediction_provenance: PredictionProvenanceV1,
        check: CheckEvaluation,
        inventory: &GltfAnimationAddressabilityInventoryV1,
    ) -> Result<Self, GltfAnimationAddressabilityError> {
        validate_bevy_adapter(
            &prediction_provenance,
            check.check_id(),
            check.selection(),
            check.configuration(),
            check.applicability(),
            check.evaluation(),
            check.findings().len(),
            check.evaluated_scopes(),
            check.gaps().len(),
            check.engine_prediction(),
            inventory,
        )?;
        Ok(Self {
            prediction_provenance,
            check,
        })
    }

    /// Same-load prediction provenance used by the embedded check.
    pub const fn prediction_provenance(&self) -> &PredictionProvenanceV1 {
        &self.prediction_provenance
    }

    /// Unchanged existing `engine-addressability` check evaluation.
    pub const fn check(&self) -> &CheckEvaluation {
        &self.check
    }
}

/// Standalone producer envelope for `generate addressability`.
#[derive(Debug, Clone, Serialize)]
pub struct GltfAnimationAddressabilityV1 {
    schema_version: u32,
    schema: &'static str,
    tool: ToolInfo,
    command: &'static str,
    input: InputIdentity,
    inventory: GltfAnimationAddressabilityInventoryV1,
    bevy: Option<GltfAnimationAddressabilityBevyAdapterV1>,
}

impl GltfAnimationAddressabilityV1 {
    /// Construct one validated standalone result document.
    ///
    /// # Errors
    ///
    /// Returns a typed contract error when an optional adapter is not bound to
    /// this exact inventory.
    pub fn new(
        tool: ToolInfo,
        inventory: GltfAnimationAddressabilityInventoryV1,
        bevy: Option<GltfAnimationAddressabilityBevyAdapterV1>,
    ) -> Result<Self, GltfAnimationAddressabilityError> {
        inventory.validate()?;
        if let Some(adapter) = &bevy {
            validate_bevy_adapter(
                adapter.prediction_provenance(),
                adapter.check().check_id(),
                adapter.check().selection(),
                adapter.check().configuration(),
                adapter.check().applicability(),
                adapter.check().evaluation(),
                adapter.check().findings().len(),
                adapter.check().evaluated_scopes(),
                adapter.check().gaps().len(),
                adapter.check().engine_prediction(),
                &inventory,
            )?;
        }
        Ok(Self {
            schema_version: GLTF_ANIMATION_ADDRESSABILITY_SCHEMA_VERSION,
            schema: GLTF_ANIMATION_ADDRESSABILITY_V1_ID,
            tool,
            command: GLTF_ANIMATION_ADDRESSABILITY_COMMAND,
            input: inventory.primary_input.clone(),
            inventory,
            bevy,
        })
    }

    /// Exact primary input identity.
    pub const fn input(&self) -> &InputIdentity {
        &self.input
    }

    /// Engine-neutral animation inventory.
    pub const fn inventory(&self) -> &GltfAnimationAddressabilityInventoryV1 {
        &self.inventory
    }

    /// Exact Bevy adapter, absent without the selected compatible profile.
    pub const fn bevy(&self) -> Option<&GltfAnimationAddressabilityBevyAdapterV1> {
        self.bevy.as_ref()
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolSourceInputV1 {
    #[serde(default, deserialize_with = "deserialize_required_nullable")]
    revision: RequiredNullable<String>,
    #[serde(default, deserialize_with = "deserialize_required_nullable")]
    dirty: RequiredNullable<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolInputV1 {
    name: String,
    version: String,
    source: ToolSourceInputV1,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BevyAdapterInputV1 {
    prediction_provenance: Box<RawValue>,
    check: Box<RawValue>,
}

#[derive(Debug, Default)]
enum RequiredNullable<T> {
    #[default]
    Missing,
    Present(Option<T>),
}

fn deserialize_required_nullable<'de, D, T>(
    deserializer: D,
) -> Result<RequiredNullable<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(RequiredNullable::Present)
}

fn deserialize_present_non_null<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

fn required_nullable_value<T>(value: RequiredNullable<T>) -> Option<T> {
    match value {
        RequiredNullable::Present(value) => value,
        RequiredNullable::Missing => {
            unreachable!("validated required-nullable tool field cannot be missing")
        }
    }
}

/// Staged bounded reader input for a standalone addressability document.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GltfAnimationAddressabilityInput {
    schema_version: u32,
    schema: String,
    tool: ToolInputV1,
    command: String,
    input: InputIdentity,
    inventory: Box<RawValue>,
    #[serde(default, deserialize_with = "deserialize_required_nullable")]
    bevy: RequiredNullable<BevyAdapterInputV1>,
}

impl GltfAnimationAddressabilityInput {
    /// Read one document through the immutable byte bound before JSON parsing.
    ///
    /// # Errors
    ///
    /// Returns a typed I/O, N+1 size, or JSON-shape error. Semantic validation
    /// remains in [`Self::into_report`].
    pub fn read_from(reader: impl Read) -> Result<Self, GltfAnimationAddressabilityReadError> {
        Self::read_from_with_limit(reader, GLTF_ANIMATION_ADDRESSABILITY_V1_MAX_REPORT_BYTES)
    }

    fn read_from_with_limit(
        reader: impl Read,
        limit: u64,
    ) -> Result<Self, GltfAnimationAddressabilityReadError> {
        let mut bounded = reader.take(limit.saturating_add(1));
        let mut bytes = Vec::new();
        bounded
            .read_to_end(&mut bytes)
            .map_err(|source| GltfAnimationAddressabilityReadError::Io { source })?;
        if bytes.len() as u64 > limit {
            return Err(GltfAnimationAddressabilityReadError::ReportTooLarge { limit });
        }
        serde_json::from_slice(&bytes)
            .map_err(|source| GltfAnimationAddressabilityReadError::InvalidJson { source })
    }

    /// Validate every contract identity and recover the strict read-only report.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a wrong header, malformed tool identity,
    /// invalid inventory, input mismatch, or invalid Bevy adapter lifecycle.
    pub fn into_report(
        self,
    ) -> Result<GltfAnimationAddressabilityReadbackV1, GltfAnimationAddressabilityError> {
        if self.schema_version != GLTF_ANIMATION_ADDRESSABILITY_SCHEMA_VERSION {
            return Err(GltfAnimationAddressabilityError::UnsupportedSchemaVersion {
                found: self.schema_version,
            });
        }
        if self.schema != GLTF_ANIMATION_ADDRESSABILITY_V1_ID {
            return Err(GltfAnimationAddressabilityError::WrongSchema);
        }
        if self.command != GLTF_ANIMATION_ADDRESSABILITY_COMMAND {
            return Err(GltfAnimationAddressabilityError::WrongCommand);
        }
        validate_tool(&self.tool)?;
        let inventory: GltfAnimationAddressabilityInventoryV1 =
            serde_json::from_str(self.inventory.get()).map_err(|source| {
                GltfAnimationAddressabilityError::InvalidInventoryShape {
                    reason: source.to_string(),
                }
            })?;
        if self.input != *inventory.primary_input() {
            return Err(GltfAnimationAddressabilityError::RootInputMismatch);
        }
        let bevy = match self.bevy {
            RequiredNullable::Missing => {
                return Err(GltfAnimationAddressabilityError::MissingBevyField);
            }
            RequiredNullable::Present(None) => None,
            RequiredNullable::Present(Some(adapter)) => {
                let prediction_provenance: PredictionProvenanceV1 =
                    serde_json::from_str(adapter.prediction_provenance.get()).map_err(
                        |source| GltfAnimationAddressabilityError::InvalidBevyProvenance {
                            reason: source.to_string(),
                        },
                    )?;
                let check_wire: GltfAnimationAddressabilityCheckWireV1 =
                    serde_json::from_str(adapter.check.get()).map_err(|source| {
                        GltfAnimationAddressabilityError::InvalidBevyCheckShape {
                            reason: source.to_string(),
                        }
                    })?;
                let check = GltfAnimationAddressabilityCheckReadbackV1::from_wire(check_wire);
                check.validate(&prediction_provenance, &inventory)?;
                Some(GltfAnimationAddressabilityBevyReadbackV1 {
                    prediction_provenance,
                    check,
                })
            }
        };
        Ok(GltfAnimationAddressabilityReadbackV1 {
            tool: GltfAnimationAddressabilityToolReadbackV1 {
                name: self.tool.name,
                version: self.tool.version,
                revision: required_nullable_value(self.tool.source.revision),
                dirty: required_nullable_value(self.tool.source.dirty),
            },
            input: self.input,
            inventory,
            bevy,
        })
    }
}

/// Validated read-side producer identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GltfAnimationAddressabilityToolReadbackV1 {
    name: String,
    version: String,
    revision: Option<String>,
    dirty: Option<bool>,
}

impl GltfAnimationAddressabilityToolReadbackV1 {
    /// Producer name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Producer version.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Full source revision, when established.
    pub fn revision(&self) -> Option<&str> {
        self.revision.as_deref()
    }

    /// Dirty-worktree observation, when established.
    pub const fn dirty(&self) -> Option<bool> {
        self.dirty
    }
}

/// Validated read-side representation of the exact embedded check subset.
#[derive(Debug, Clone)]
pub struct GltfAnimationAddressabilityCheckReadbackV1 {
    check_id: String,
    selection: SelectionState,
    configuration: ConfigurationState,
    applicability: Applicability,
    evaluation: EvaluationState,
    findings: Vec<Box<RawValue>>,
    evaluated_scopes: Vec<EvaluationScope>,
    gaps: Vec<Box<RawValue>>,
    prediction: Option<EnginePredictionV1>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GltfAnimationAddressabilityCheckWireV1 {
    check_id: String,
    selection: SelectionState,
    configuration: ConfigurationState,
    applicability: Applicability,
    evaluation: EvaluationState,
    findings: Vec<Box<RawValue>>,
    #[serde(default)]
    evaluated_scopes: Vec<EvaluationScope>,
    #[serde(default)]
    gaps: Vec<Box<RawValue>>,
    #[serde(default, deserialize_with = "deserialize_present_non_null")]
    prediction: Option<EnginePredictionV1>,
}

impl GltfAnimationAddressabilityCheckReadbackV1 {
    fn from_wire(wire: GltfAnimationAddressabilityCheckWireV1) -> Self {
        Self {
            check_id: wire.check_id,
            selection: wire.selection,
            configuration: wire.configuration,
            applicability: wire.applicability,
            evaluation: wire.evaluation,
            findings: wire.findings,
            evaluated_scopes: wire.evaluated_scopes,
            gaps: wire.gaps,
            prediction: wire.prediction,
        }
    }

    /// Stable check id, always `engine-addressability` after validation.
    pub fn check_id(&self) -> &str {
        &self.check_id
    }

    /// Selection state carried by the unchanged embedded check.
    pub const fn selection(&self) -> SelectionState {
        self.selection
    }

    /// Configuration state carried by the unchanged embedded check.
    pub const fn configuration(&self) -> ConfigurationState {
        self.configuration
    }

    /// Applicability state carried by the unchanged embedded check.
    pub const fn applicability(&self) -> Applicability {
        self.applicability
    }

    /// Derived evaluation state carried by the serialized check.
    pub const fn evaluation(&self) -> EvaluationState {
        self.evaluation
    }

    /// Exact evaluated scopes carried by the unchanged embedded check.
    pub fn evaluated_scopes(&self) -> &[EvaluationScope] {
        &self.evaluated_scopes
    }

    /// Embedded prediction attachment, when applicable.
    pub const fn prediction(&self) -> Option<&EnginePredictionV1> {
        self.prediction.as_ref()
    }

    fn validate(
        &self,
        provenance: &PredictionProvenanceV1,
        inventory: &GltfAnimationAddressabilityInventoryV1,
    ) -> Result<(), GltfAnimationAddressabilityError> {
        if !self.findings.is_empty() || !self.gaps.is_empty() {
            return Err(GltfAnimationAddressabilityError::InvalidBevyCheckSubset);
        }
        validate_bevy_adapter(
            provenance,
            &self.check_id,
            self.selection,
            self.configuration,
            self.applicability,
            self.evaluation,
            self.findings.len(),
            &self.evaluated_scopes,
            self.gaps.len(),
            self.prediction.as_ref(),
            inventory,
        )
    }
}

/// Validated read-side Bevy adapter.
#[derive(Debug, Clone)]
pub struct GltfAnimationAddressabilityBevyReadbackV1 {
    prediction_provenance: PredictionProvenanceV1,
    check: GltfAnimationAddressabilityCheckReadbackV1,
}

impl GltfAnimationAddressabilityBevyReadbackV1 {
    /// Exact prediction provenance.
    pub const fn prediction_provenance(&self) -> &PredictionProvenanceV1 {
        &self.prediction_provenance
    }

    /// Strict validated check subset.
    pub const fn check(&self) -> &GltfAnimationAddressabilityCheckReadbackV1 {
        &self.check
    }
}

/// Fully validated read-side standalone document.
#[derive(Debug, Clone)]
pub struct GltfAnimationAddressabilityReadbackV1 {
    tool: GltfAnimationAddressabilityToolReadbackV1,
    input: InputIdentity,
    inventory: GltfAnimationAddressabilityInventoryV1,
    bevy: Option<GltfAnimationAddressabilityBevyReadbackV1>,
}

impl GltfAnimationAddressabilityReadbackV1 {
    /// Validated producer identity.
    pub const fn tool(&self) -> &GltfAnimationAddressabilityToolReadbackV1 {
        &self.tool
    }

    /// Exact primary input identity.
    pub const fn input(&self) -> &InputIdentity {
        &self.input
    }

    /// Validated engine-neutral inventory.
    pub const fn inventory(&self) -> &GltfAnimationAddressabilityInventoryV1 {
        &self.inventory
    }

    /// Validated exact Bevy adapter, when present.
    pub const fn bevy(&self) -> Option<&GltfAnimationAddressabilityBevyReadbackV1> {
        self.bevy.as_ref()
    }
}

/// A serialized standalone addressability document could not be read safely.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum GltfAnimationAddressabilityReadError {
    /// Reading the bounded input failed.
    #[error("cannot read addressability report: {source}")]
    Io {
        /// Underlying bounded-reader failure.
        #[source]
        source: std::io::Error,
    },
    /// The serialized report exceeded the immutable byte limit.
    #[error("addressability report exceeds the V1 limit of {limit} bytes")]
    ReportTooLarge {
        /// Immutable maximum accepted byte count.
        limit: u64,
    },
    /// The bounded bytes were not valid JSON for the strict root shape.
    #[error("invalid addressability report JSON: {source}")]
    InvalidJson {
        /// JSON syntax or typed-shape failure.
        #[source]
        source: serde_json::Error,
    },
}

/// A producer or reader encountered an invalid addressability V1 contract.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum GltfAnimationAddressabilityError {
    /// The raw source is not glTF JSON or GLB.
    #[error("source format {format:?} is not supported by glTF animation addressability V1")]
    UnsupportedSourceFormat {
        /// Actual source format.
        format: SourceFormatV1,
    },
    /// A platform-sized source index could not be represented on the wire.
    #[error("{field} cannot be represented as an unsigned 64-bit integer")]
    IndexOverflow {
        /// Field that overflowed.
        field: &'static str,
    },
    /// A source-order row index exceeded its standalone V1 wire bound.
    #[error("{field} {actual} is outside the V1 limit of {limit} rows")]
    SourceIndexOutOfRange {
        /// Bounded source index field.
        field: &'static str,
        /// Supplied source index.
        actual: u64,
        /// Exclusive V1 row-count limit.
        limit: usize,
    },
    /// Checked aggregate accounting overflowed.
    #[error("checked arithmetic overflow while validating {field}")]
    ArithmeticOverflow {
        /// Aggregate that overflowed.
        field: &'static str,
    },
    /// An animation collection exceeded the inherited raw-source bound.
    #[error("inventory contains {found} animations, exceeding the V1 limit of {limit}")]
    TooManyAnimations {
        /// Supplied row count.
        found: usize,
        /// Immutable limit.
        limit: usize,
    },
    /// Aggregate animation and channel rows exceeded the inherited bound.
    #[error("inventory contains {found} animation/channel rows, exceeding the V1 limit of {limit}")]
    TooManyRows {
        /// Supplied aggregate row count.
        found: usize,
        /// Immutable limit.
        limit: usize,
    },
    /// One retained name exceeded the inherited text bound.
    #[error("source name is {found} bytes, exceeding the V1 limit of {limit}")]
    TextTooLong {
        /// Supplied UTF-8 byte count.
        found: usize,
        /// Immutable limit.
        limit: usize,
    },
    /// Aggregate retained source-name text exceeded the inherited bound.
    #[error("inventory retains {found} source-name bytes, exceeding the V1 limit of {limit}")]
    TooMuchText {
        /// Supplied aggregate UTF-8 byte count.
        found: usize,
        /// Immutable limit.
        limit: usize,
    },
    /// Coverage state and reason do not form a valid pair.
    #[error("coverage reason must be absent exactly for complete coverage")]
    InvalidCoverage,
    /// Unavailable coverage incorrectly retained positive rows.
    #[error("unavailable coverage must retain no rows")]
    RowsWithUnavailableCoverage,
    /// A source animation row did not preserve prefix order.
    #[error("animation row index {actual} does not match expected source index {expected}")]
    NonCanonicalAnimationIndex {
        /// Expected source-order index.
        expected: u64,
        /// Actual source index.
        actual: u64,
    },
    /// A source channel row did not preserve prefix order.
    #[error(
        "animation {source_clip_index} channel index {actual} does not match expected {expected}"
    )]
    NonCanonicalChannelIndex {
        /// Parent source animation index.
        source_clip_index: u64,
        /// Expected source-order channel index.
        expected: u64,
        /// Actual source channel index.
        actual: u64,
    },
    /// Only one accessor of a required input/output pair was retained.
    #[error("channel accessor identities must be both present or both absent")]
    IncompleteAccessorPair,
    /// A retained glTF channel omitted its exact input/output accessor identities.
    #[error("retained glTF channels must carry exact input and output accessor identities")]
    MissingAccessorPair,
    /// A glTF channel targeted a non-node source domain.
    #[error("retained glTF channels must target source nodes")]
    InvalidGltfTargetKind,
    /// The embedded closure names a different primary input.
    #[error("dependency closure primary input does not match the inventory")]
    DependencyClosurePrimaryMismatch,
    /// The canonical inventory identity was mutated or forged.
    #[error("inventory identity does not match its canonical engine-neutral preimage")]
    InventoryIdentityMismatch,
    /// Envelope schema version is unsupported.
    #[error("addressability report has schema_version {found}; this build reads version 1")]
    UnsupportedSchemaVersion {
        /// Supplied version.
        found: u32,
    },
    /// Envelope schema identity is wrong.
    #[error("addressability report has the wrong schema identity")]
    WrongSchema,
    /// Envelope command discriminator is wrong.
    #[error("addressability report has the wrong command discriminator")]
    WrongCommand,
    /// Producer tool fields are malformed or exceed V1 text bounds.
    #[error("addressability report has an invalid tool identity")]
    InvalidTool,
    /// Nested inventory shape or semantic decoding failed.
    #[error("addressability report has an invalid inventory: {reason}")]
    InvalidInventoryShape {
        /// Stable nested decoding diagnostic.
        reason: String,
    },
    /// Root and inventory primary input identities differ.
    #[error("root input does not match the inventory primary input")]
    RootInputMismatch,
    /// Required nullable Bevy field is missing.
    #[error("addressability report has no required `bevy` field")]
    MissingBevyField,
    /// Bevy provenance failed strict decoding.
    #[error("addressability report has invalid Bevy provenance: {reason}")]
    InvalidBevyProvenance {
        /// Stable nested decoding diagnostic.
        reason: String,
    },
    /// Bevy check shape failed strict decoding.
    #[error("addressability report has an invalid Bevy check: {reason}")]
    InvalidBevyCheckShape {
        /// Stable nested decoding diagnostic.
        reason: String,
    },
    /// The adapter is not the exact frozen Bevy tuple and known label fact.
    #[error("addressability adapter is not the exact Bevy 0.19.0 glTF profile subset")]
    InvalidBevyProfile,
    /// Adapter provenance and inventory identify different source evidence.
    #[error("Bevy provenance does not bind to the engine-neutral inventory")]
    BevyInventoryMismatch,
    /// The embedded check is not the exact selected/enabled #154 subset.
    #[error("Bevy adapter check is not the exact engine-addressability subset")]
    InvalidBevyCheckSubset,
    /// Nested prediction evidence is invalid.
    #[error("invalid Bevy prediction: {0}")]
    InvalidPrediction(PredictionContractError),
}

fn require_gltf(format: SourceFormatV1) -> Result<(), GltfAnimationAddressabilityError> {
    if matches!(format, SourceFormatV1::GltfJson | SourceFormatV1::Glb) {
        Ok(())
    } else {
        Err(GltfAnimationAddressabilityError::UnsupportedSourceFormat { format })
    }
}

fn to_u64(field: &'static str, value: usize) -> Result<u64, GltfAnimationAddressabilityError> {
    u64::try_from(value).map_err(|_| GltfAnimationAddressabilityError::IndexOverflow { field })
}

fn map_index_observation(
    value: &animsmith_core::SourceObservationV1<usize>,
) -> Result<GltfAnimationObservationV1<u64>, GltfAnimationAddressabilityError> {
    Ok(match value.state() {
        SourceObservationStateV1::Observed(value) => GltfAnimationObservationV1::Observed {
            value: to_u64("normalized clip index", *value)?,
        },
        SourceObservationStateV1::ProvenAbsent => GltfAnimationObservationV1::ProvenAbsent,
        SourceObservationStateV1::Unavailable(reason) => GltfAnimationObservationV1::Unavailable {
            reason: (*reason).into(),
        },
    })
}

fn preflight_source(
    facts: animsmith_core::SourceFactsViewV1<'_>,
) -> Result<(), GltfAnimationAddressabilityError> {
    let mut rows = facts.clips().rows().len();
    let mut text = 0usize;
    for clip in facts.clips().rows() {
        rows = rows.checked_add(clip.channels().rows().len()).ok_or(
            GltfAnimationAddressabilityError::ArithmeticOverflow {
                field: "animation/channel rows",
            },
        )?;
        if let SourceObservationStateV1::Observed(name) = clip.source_name().state() {
            text = text.checked_add(name.as_str().len()).ok_or(
                GltfAnimationAddressabilityError::ArithmeticOverflow {
                    field: "source-name text",
                },
            )?;
        }
    }
    if facts.clips().rows().len() > animsmith_core::RAW_SOURCE_V1_MAX_CLIPS {
        return Err(GltfAnimationAddressabilityError::TooManyAnimations {
            found: facts.clips().rows().len(),
            limit: animsmith_core::RAW_SOURCE_V1_MAX_CLIPS,
        });
    }
    if rows > animsmith_core::RAW_SOURCE_V1_MAX_OBSERVATIONS {
        return Err(GltfAnimationAddressabilityError::TooManyRows {
            found: rows,
            limit: animsmith_core::RAW_SOURCE_V1_MAX_OBSERVATIONS,
        });
    }
    if text > animsmith_core::RAW_SOURCE_V1_MAX_TOTAL_TEXT_BYTES {
        return Err(GltfAnimationAddressabilityError::TooMuchText {
            found: text,
            limit: animsmith_core::RAW_SOURCE_V1_MAX_TOTAL_TEXT_BYTES,
        });
    }
    Ok(())
}

fn validate_animation_set(
    animations: &GltfAnimationAddressabilityAnimationSetV1,
) -> Result<(), GltfAnimationAddressabilityError> {
    animations.coverage.validate()?;
    if animations.coverage.state == GltfAnimationCoverageStateV1::Unavailable
        && !animations.rows.is_empty()
    {
        return Err(GltfAnimationAddressabilityError::RowsWithUnavailableCoverage);
    }
    if animations.rows.len() > animsmith_core::RAW_SOURCE_V1_MAX_CLIPS {
        return Err(GltfAnimationAddressabilityError::TooManyAnimations {
            found: animations.rows.len(),
            limit: animsmith_core::RAW_SOURCE_V1_MAX_CLIPS,
        });
    }
    let mut total_rows = animations.rows.len();
    let mut total_text = 0usize;
    for (animation_offset, animation) in animations.rows.iter().enumerate() {
        let expected = to_u64("expected animation index", animation_offset)?;
        if animation.source_clip_index != expected {
            return Err(
                GltfAnimationAddressabilityError::NonCanonicalAnimationIndex {
                    expected,
                    actual: animation.source_clip_index,
                },
            );
        }
        animation.channels.coverage.validate()?;
        if animation.channels.coverage.state == GltfAnimationCoverageStateV1::Unavailable
            && !animation.channels.rows.is_empty()
        {
            return Err(GltfAnimationAddressabilityError::RowsWithUnavailableCoverage);
        }
        total_rows = total_rows
            .checked_add(animation.channels.rows.len())
            .ok_or(GltfAnimationAddressabilityError::ArithmeticOverflow {
                field: "animation/channel rows",
            })?;
        for (channel_offset, channel) in animation.channels.rows.iter().enumerate() {
            let expected = to_u64("expected channel index", channel_offset)?;
            if channel.source_channel_index != expected {
                return Err(GltfAnimationAddressabilityError::NonCanonicalChannelIndex {
                    source_clip_index: animation.source_clip_index,
                    expected,
                    actual: channel.source_channel_index,
                });
            }
            if channel.target.kind != GltfAnimationTargetKindV1::Node {
                return Err(GltfAnimationAddressabilityError::InvalidGltfTargetKind);
            }
        }
        if let GltfAnimationObservationV1::Observed { value } = &animation.source_name {
            if value.len() > animsmith_core::RAW_SOURCE_V1_MAX_TEXT_BYTES {
                return Err(GltfAnimationAddressabilityError::TextTooLong {
                    found: value.len(),
                    limit: animsmith_core::RAW_SOURCE_V1_MAX_TEXT_BYTES,
                });
            }
            total_text = total_text.checked_add(value.len()).ok_or(
                GltfAnimationAddressabilityError::ArithmeticOverflow {
                    field: "source-name text",
                },
            )?;
        }
    }
    if total_rows > animsmith_core::RAW_SOURCE_V1_MAX_OBSERVATIONS {
        return Err(GltfAnimationAddressabilityError::TooManyRows {
            found: total_rows,
            limit: animsmith_core::RAW_SOURCE_V1_MAX_OBSERVATIONS,
        });
    }
    if total_text > animsmith_core::RAW_SOURCE_V1_MAX_TOTAL_TEXT_BYTES {
        return Err(GltfAnimationAddressabilityError::TooMuchText {
            found: total_text,
            limit: animsmith_core::RAW_SOURCE_V1_MAX_TOTAL_TEXT_BYTES,
        });
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_bevy_adapter(
    provenance: &PredictionProvenanceV1,
    check_id: &str,
    selection: SelectionState,
    configuration: ConfigurationState,
    applicability: Applicability,
    evaluation: EvaluationState,
    finding_count: usize,
    evaluated_scopes: &[EvaluationScope],
    gap_count: usize,
    prediction: Option<&EnginePredictionV1>,
    inventory: &GltfAnimationAddressabilityInventoryV1,
) -> Result<(), GltfAnimationAddressabilityError> {
    provenance
        .validate()
        .map_err(GltfAnimationAddressabilityError::InvalidPrediction)?;
    let profile = provenance.profile();
    let profile_selection = profile.selection();
    if profile_selection.family() != BEVY_FAMILY
        || profile_selection.profile_revision() != BEVY_PROFILE_REVISION
        || profile_selection.engine_version() != BEVY_ENGINE_VERSION
        || profile_selection.importer() != BEVY_IMPORTER
        || profile.facts_identity().sha256() != BEVY_PROFILE_FACTS_SHA256
        || profile.facts_identity().bytes() != BEVY_PROFILE_FACTS_CANONICAL_BYTES
        || !matches!(
            profile
                .fact(EngineFactIdV1::AnimationAddressability)
                .map(|fact| fact.state()),
            Some(EngineFactStateV1::Known(
                EngineFactValueV1::AnimationAddressability(
                    EngineAnimationAddressabilityV1::GltfAssetLabel
                )
            ))
        )
        || profile.source(BEVY_ANIMATION_LABEL_SOURCE).is_none()
    {
        return Err(GltfAnimationAddressabilityError::InvalidBevyProfile);
    }
    validate_inventory_provenance_binding(provenance, inventory)?;
    if check_id != ENGINE_ADDRESSABILITY_CHECK_ID
        || selection != SelectionState::Selected
        || finding_count != 0
        || gap_count != 0
    {
        return Err(GltfAnimationAddressabilityError::InvalidBevyCheckSubset);
    }
    let expected_applicability = if inventory.animations.coverage.state
        == GltfAnimationCoverageStateV1::Complete
        && inventory.animations.rows.is_empty()
    {
        Applicability::NotApplicable
    } else {
        Applicability::Applicable
    };
    if applicability != expected_applicability {
        return Err(GltfAnimationAddressabilityError::InvalidBevyCheckSubset);
    }
    if configuration == ConfigurationState::Disabled {
        if evaluation != EvaluationState::NotEvaluated
            || !evaluated_scopes.is_empty()
            || prediction.is_some()
        {
            return Err(GltfAnimationAddressabilityError::InvalidBevyCheckSubset);
        }
        return Ok(());
    }
    if let Some(prediction) = prediction {
        prediction
            .validate_against_provenance(provenance)
            .map_err(GltfAnimationAddressabilityError::InvalidPrediction)?;
    }

    let rows = inventory.animations.rows();
    match inventory.animations.coverage.state {
        GltfAnimationCoverageStateV1::Complete if rows.is_empty() => {
            if applicability != Applicability::NotApplicable
                || evaluation != EvaluationState::NotEvaluated
                || !evaluated_scopes.is_empty()
                || prediction.is_some()
            {
                return Err(GltfAnimationAddressabilityError::InvalidBevyCheckSubset);
            }
        }
        GltfAnimationCoverageStateV1::Complete => {
            let prediction =
                prediction.ok_or(GltfAnimationAddressabilityError::InvalidBevyCheckSubset)?;
            if applicability != Applicability::Applicable
                || evaluation != EvaluationState::Complete
                || prediction.facets().len() != rows.len()
                || evaluated_scopes.len() != rows.len()
            {
                return Err(GltfAnimationAddressabilityError::InvalidBevyCheckSubset);
            }
            for (row, evaluated) in rows.iter().zip(evaluated_scopes) {
                let source_clip_index = usize::try_from(row.source_clip_index).map_err(|_| {
                    GltfAnimationAddressabilityError::IndexOverflow {
                        field: "source clip index",
                    }
                })?;
                let label = crate::BevyAnimationAssetLabelV1::new(source_clip_index)
                    .map_err(|_| GltfAnimationAddressabilityError::InvalidBevyCheckSubset)?;
                let expected = EvaluationScope::new(
                    animsmith_core::EvaluationScopeCode::ANIMATION_ASSET_LABEL,
                )
                .subject(label.as_str().to_owned());
                let Some(facet) = prediction.facets().iter().find(|facet| {
                    facet.state() == EnginePredictionFacetStateV1::Available
                        && facet.scope() == &expected
                }) else {
                    return Err(GltfAnimationAddressabilityError::InvalidBevyCheckSubset);
                };
                if evaluated != &expected
                    || prediction
                        .facets()
                        .iter()
                        .filter(|facet| {
                            facet.state() == EnginePredictionFacetStateV1::Available
                                && facet.scope() == &expected
                        })
                        .count()
                        != 1
                    || !is_exact_available_basis(facet.basis().references(), row)
                {
                    return Err(GltfAnimationAddressabilityError::InvalidBevyCheckSubset);
                }
            }
        }
        GltfAnimationCoverageStateV1::Partial | GltfAnimationCoverageStateV1::Unavailable => {
            let prediction =
                prediction.ok_or(GltfAnimationAddressabilityError::InvalidBevyCheckSubset)?;
            let facets = prediction.facets();
            let expected = EvaluationScope::new(
                animsmith_core::EvaluationScopeCode::ANIMATION_ASSET_LABEL_INVENTORY,
            );
            if applicability != Applicability::Applicable
                || evaluation != EvaluationState::NotEvaluated
                || !evaluated_scopes.is_empty()
                || facets.len() != 1
                || facets[0].state() != EnginePredictionFacetStateV1::RequiredPredictionUnavailable
                || facets[0].scope() != &expected
                || facets[0].reasons() != [PredictionUnavailableReasonV1::RawSourceIncomplete]
                || !is_exact_unavailable_basis(facets[0].basis().references())
            {
                return Err(GltfAnimationAddressabilityError::InvalidBevyCheckSubset);
            }
        }
    }
    Ok(())
}

fn is_exact_available_basis(
    references: &[PredictionBasisReferenceV1],
    row: &GltfAnimationAddressabilityAnimationV1,
) -> bool {
    if references.len() != 3 {
        return false;
    }
    let expected_state = match row.source_name() {
        GltfAnimationObservationV1::Observed { .. } => "observed",
        GltfAnimationObservationV1::ProvenAbsent => "proven_absent",
        GltfAnimationObservationV1::Unavailable { .. } => "unavailable",
    };
    let mut profile_fact = false;
    let mut primary_source = false;
    let mut raw_source = false;
    for reference in references {
        match reference {
            PredictionBasisReferenceV1::ProfileFact { fact_id }
                if fact_id == "animation_addressability" =>
            {
                profile_fact = true;
            }
            PredictionBasisReferenceV1::PrimarySource { source_id }
                if source_id == BEVY_ANIMATION_LABEL_SOURCE =>
            {
                primary_source = true;
            }
            PredictionBasisReferenceV1::RawSource { reference }
                if reference.domain() == RawSourceDomainV1::Clip
                    && reference.key()
                        == &RawSourceKeyV1::Clip {
                            source_clip_index: row.source_clip_index(),
                        }
                    && reference.field().as_str() == "source_name.state"
                    && matches!(
                        reference.value(),
                        PredictionScalarV1::Token { value } if value == expected_state
                    ) =>
            {
                raw_source = true;
            }
            _ => return false,
        }
    }
    profile_fact && primary_source && raw_source
}

fn is_exact_unavailable_basis(references: &[PredictionBasisReferenceV1]) -> bool {
    if references.len() != 2 {
        return false;
    }
    let mut profile_fact = false;
    let mut primary_source = false;
    for reference in references {
        match reference {
            PredictionBasisReferenceV1::ProfileFact { fact_id }
                if fact_id == "animation_addressability" =>
            {
                profile_fact = true;
            }
            PredictionBasisReferenceV1::PrimarySource { source_id }
                if source_id == BEVY_ANIMATION_LABEL_SOURCE =>
            {
                primary_source = true;
            }
            _ => return false,
        }
    }
    profile_fact && primary_source
}

fn validate_inventory_provenance_binding(
    provenance: &PredictionProvenanceV1,
    inventory: &GltfAnimationAddressabilityInventoryV1,
) -> Result<(), GltfAnimationAddressabilityError> {
    if provenance.source_format() != inventory.source_format
        || provenance.raw_source().primary_input() != inventory.primary_input()
        || provenance.dependency_closure() != inventory.dependency_closure()
        || raw_coverage_state(provenance.raw_source().clips_coverage().state())
            != inventory.animations.coverage.state
        || provenance
            .raw_source()
            .clips_coverage()
            .reason()
            .map(raw_unavailable_reason)
            != inventory.animations.coverage.reason
    {
        return Err(GltfAnimationAddressabilityError::BevyInventoryMismatch);
    }
    Ok(())
}

fn validate_tool(tool: &ToolInputV1) -> Result<(), GltfAnimationAddressabilityError> {
    if tool.name != "animsmith"
        || !is_schema_semver(&tool.version)
        || tool.name.len() > animsmith_core::RAW_SOURCE_V1_MAX_TEXT_BYTES
        || tool.version.len() > animsmith_core::RAW_SOURCE_V1_MAX_TEXT_BYTES
        || matches!(tool.source.revision, RequiredNullable::Missing)
        || matches!(tool.source.dirty, RequiredNullable::Missing)
        || matches!(
            &tool.source.revision,
            RequiredNullable::Present(Some(revision))
                if revision.len() != 40
                    || !revision.bytes().all(|value| value.is_ascii_hexdigit())
        )
    {
        return Err(GltfAnimationAddressabilityError::InvalidTool);
    }
    Ok(())
}

fn is_schema_semver(version: &str) -> bool {
    let (without_build, build) = match version.split_once('+') {
        Some((value, build))
            if !build.is_empty()
                && !build.contains('+')
                && build
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-')) =>
        {
            (value, Some(build))
        }
        Some(_) => return false,
        None => (version, None),
    };
    let _ = build;
    let (core, prerelease) = match without_build.split_once('-') {
        Some((core, prerelease))
            if !prerelease.is_empty()
                && prerelease
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-')) =>
        {
            (core, Some(prerelease))
        }
        Some(_) => return false,
        None => (without_build, None),
    };
    let _ = prerelease;
    let mut components = core.split('.');
    (0..3).all(|_| {
        components.next().is_some_and(|value| {
            !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
        })
    }) && components.next().is_none()
}

fn raw_coverage_state(value: RawSourceSetCoverageStateV1) -> GltfAnimationCoverageStateV1 {
    match value {
        RawSourceSetCoverageStateV1::Complete => GltfAnimationCoverageStateV1::Complete,
        RawSourceSetCoverageStateV1::Partial => GltfAnimationCoverageStateV1::Partial,
        RawSourceSetCoverageStateV1::Unavailable => GltfAnimationCoverageStateV1::Unavailable,
    }
}

fn raw_unavailable_reason(value: RawSourceUnavailableReasonV1) -> GltfAnimationUnavailableReasonV1 {
    match value {
        RawSourceUnavailableReasonV1::Malformed => GltfAnimationUnavailableReasonV1::Malformed,
        RawSourceUnavailableReasonV1::Discarded => GltfAnimationUnavailableReasonV1::Discarded,
        RawSourceUnavailableReasonV1::NormalizedAway => {
            GltfAnimationUnavailableReasonV1::NormalizedAway
        }
        RawSourceUnavailableReasonV1::BakedAway => GltfAnimationUnavailableReasonV1::BakedAway,
        RawSourceUnavailableReasonV1::LoaderUnsupported => {
            GltfAnimationUnavailableReasonV1::LoaderUnsupported
        }
        RawSourceUnavailableReasonV1::ProjectionBudgetExceeded => {
            GltfAnimationUnavailableReasonV1::ProjectionBudgetExceeded
        }
        RawSourceUnavailableReasonV1::ParserUnavailable => {
            GltfAnimationUnavailableReasonV1::ParserUnavailable
        }
    }
}

fn encode_animation_set(
    encoder: &mut CanonicalEncoder,
    animations: &GltfAnimationAddressabilityAnimationSetV1,
) {
    encoder.field("animations");
    encode_coverage(encoder, animations.coverage);
    encoder.count(animations.rows.len());
    for animation in &animations.rows {
        encoder.token(animation.source_clip_index.to_string());
        encode_observation(encoder, &animation.source_name, |encoder, value| {
            encoder.token(value)
        });
        encode_observation(
            encoder,
            &animation.normalized_clip_index,
            |encoder, value| encoder.token(value.to_string()),
        );
        encode_coverage(encoder, animation.channels.coverage);
        encoder.count(animation.channels.rows.len());
        for channel in &animation.channels.rows {
            encoder.token(channel.source_channel_index.to_string());
            encoder.token(target_kind_name(channel.target.kind));
            encoder.token(channel.target.index.to_string());
            encoder.token(property_name(channel.property));
            encoder.token(channel.input_accessor_index.to_string());
            encoder.token(channel.output_accessor_index.to_string());
        }
    }
}

fn encode_coverage(encoder: &mut CanonicalEncoder, coverage: GltfAnimationCoverageV1) {
    encoder.token(match coverage.state {
        GltfAnimationCoverageStateV1::Complete => "complete",
        GltfAnimationCoverageStateV1::Partial => "partial",
        GltfAnimationCoverageStateV1::Unavailable => "unavailable",
    });
    encoder.token(
        coverage
            .reason
            .map(unavailable_reason_name)
            .unwrap_or("null"),
    );
}

fn encode_observation<T>(
    encoder: &mut CanonicalEncoder,
    value: &GltfAnimationObservationV1<T>,
    encode_value: impl FnOnce(&mut CanonicalEncoder, &T),
) {
    match value {
        GltfAnimationObservationV1::Observed { value } => {
            encoder.token("observed");
            encode_value(encoder, value);
        }
        GltfAnimationObservationV1::ProvenAbsent => encoder.token("proven_absent"),
        GltfAnimationObservationV1::Unavailable { reason } => {
            encoder.token("unavailable");
            encoder.token(unavailable_reason_name(*reason));
        }
    }
}

#[derive(Debug, Default)]
struct CanonicalEncoder(Vec<u8>);

impl CanonicalEncoder {
    fn new(domain: &str) -> Self {
        let mut encoder = Self::default();
        encoder.token(domain);
        encoder
    }

    fn token(&mut self, token: impl AsRef<str>) {
        let bytes = token.as_ref().as_bytes();
        self.0
            .extend_from_slice(&(bytes.len() as u64).to_be_bytes());
        self.0.extend_from_slice(bytes);
    }

    fn field(&mut self, field: &'static str) {
        self.token(field);
    }

    fn count(&mut self, count: usize) {
        self.token(count.to_string());
    }

    fn identity(self) -> InputIdentity {
        InputIdentity::from_bytes(&self.0)
    }
}

fn encode_input_identity(encoder: &mut CanonicalEncoder, identity: &InputIdentity) {
    encoder.token("sha256");
    encoder.token(identity.sha256());
    encoder.token("bytes");
    encoder.token(identity.bytes().to_string());
}

fn source_format_name(value: SourceFormatV1) -> &'static str {
    match value {
        SourceFormatV1::GltfJson => "gltf_json",
        SourceFormatV1::Glb => "glb",
        SourceFormatV1::Fbx => "fbx",
    }
}

fn unavailable_reason_name(value: GltfAnimationUnavailableReasonV1) -> &'static str {
    match value {
        GltfAnimationUnavailableReasonV1::Malformed => "malformed",
        GltfAnimationUnavailableReasonV1::Discarded => "discarded",
        GltfAnimationUnavailableReasonV1::NormalizedAway => "normalized_away",
        GltfAnimationUnavailableReasonV1::BakedAway => "baked_away",
        GltfAnimationUnavailableReasonV1::LoaderUnsupported => "loader_unsupported",
        GltfAnimationUnavailableReasonV1::ProjectionBudgetExceeded => "projection_budget_exceeded",
        GltfAnimationUnavailableReasonV1::ParserUnavailable => "parser_unavailable",
    }
}

fn target_kind_name(value: GltfAnimationTargetKindV1) -> &'static str {
    match value {
        GltfAnimationTargetKindV1::Node => "node",
        GltfAnimationTargetKindV1::Element => "element",
        GltfAnimationTargetKindV1::Other => "other",
    }
}

fn property_name(value: GltfAnimationChannelPropertyV1) -> &'static str {
    match value {
        GltfAnimationChannelPropertyV1::Translation => "translation",
        GltfAnimationChannelPropertyV1::Rotation => "rotation",
        GltfAnimationChannelPropertyV1::Scale => "scale",
        GltfAnimationChannelPropertyV1::Weights => "weights",
        GltfAnimationChannelPropertyV1::Other => "other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use animsmith_core::{
        Check, CheckCtx, CheckOutput, CheckSelection, Clip, Config, Document, EngineFactValueV1,
        EnginePrimarySourceV1, EngineProfileFactV1, EngineProfileSelectionV1, MetricGrids,
        RawSourceBindingV1, RawSourceFactsBuilderV1, ResolvedEngineProfileV1,
        ResolvedEngineSettingsV1, SeveritySetting, SourceComponentMaskV1, SourceFactDomainV1,
        SourceFactSetV1, SourceLoaderDispositionV1, SourceObservationV1, SourceProvenanceV1,
        SourceTargetV1, SourceTextV1, ToolSource, evaluate_checks, resolve_configured_roles,
    };
    use std::collections::BTreeMap;
    use std::io::Cursor;

    fn dummy_clip(index: usize) -> Clip {
        Clip {
            name: format!("clip-{index}"),
            duration_s: 0.0,
            tracks: Vec::new(),
        }
    }

    fn raw_channel(
        index: usize,
        property: SourceChannelPropertyV1,
    ) -> animsmith_core::SourceChannelFactV1 {
        animsmith_core::SourceChannelFactV1::new(
            index,
            SourceTargetV1::new(SourceTargetKindV1::Node, index as u64),
            property,
            SourceComponentMaskV1::new(true, true, true),
            SourceObservationV1::proven_absent(SourceProvenanceV1::format_defined()),
            SourceLoaderDispositionV1::Preserved,
            SourceProvenanceV1::format_defined(),
        )
        .with_accessors(index * 2, index * 2 + 1)
    }

    fn raw_clip(
        index: usize,
        name: GltfAnimationObservationV1<String>,
        channels: Vec<animsmith_core::SourceChannelFactV1>,
    ) -> animsmith_core::SourceClipFactV1 {
        raw_clip_with_channel_set(index, name, SourceFactSetV1::complete(channels))
    }

    fn raw_clip_with_channel_set(
        index: usize,
        name: GltfAnimationObservationV1<String>,
        channels: SourceFactSetV1<animsmith_core::SourceChannelFactV1>,
    ) -> animsmith_core::SourceClipFactV1 {
        let source_name = match name {
            GltfAnimationObservationV1::Observed { value } => SourceObservationV1::observed(
                SourceTextV1::new(value).expect("bounded test name"),
                SourceProvenanceV1::format_defined(),
                SourceLoaderDispositionV1::Preserved,
            ),
            GltfAnimationObservationV1::ProvenAbsent => {
                SourceObservationV1::proven_absent(SourceProvenanceV1::format_defined())
            }
            GltfAnimationObservationV1::Unavailable { reason: _ } => {
                SourceObservationV1::unavailable(
                    SourceUnavailableReasonV1::ParserUnavailable,
                    None,
                    SourceLoaderDispositionV1::Unknown,
                )
            }
        };
        animsmith_core::SourceClipFactV1::new(
            index,
            source_name,
            SourceObservationV1::observed(
                index,
                SourceProvenanceV1::format_defined(),
                SourceLoaderDispositionV1::Normalized,
            ),
            SourceObservationV1::proven_absent(SourceProvenanceV1::format_defined()),
            SourceObservationV1::proven_absent(SourceProvenanceV1::format_defined()),
            channels,
        )
    }

    fn loaded_source(format: SourceFormatV1) -> LoadedSource {
        let primary = InputIdentity::from_bytes(b"analytic-gltf");
        let mut builder = RawSourceFactsBuilderV1::new(format, primary);
        assert!(builder.push_clip(raw_clip(
            0,
            GltfAnimationObservationV1::Observed {
                value: "walk".to_owned(),
            },
            vec![
                raw_channel(0, SourceChannelPropertyV1::Translation),
                raw_channel(1, SourceChannelPropertyV1::Rotation),
                raw_channel(2, SourceChannelPropertyV1::Scale),
                raw_channel(3, SourceChannelPropertyV1::Weights),
            ],
        )));
        assert!(builder.push_clip(raw_clip(
            1,
            GltfAnimationObservationV1::ProvenAbsent,
            Vec::new(),
        )));
        assert!(builder.push_clip(raw_clip(
            2,
            GltfAnimationObservationV1::Observed {
                value: "walk".to_owned(),
            },
            Vec::new(),
        )));
        builder.mark_complete(SourceFactDomainV1::Clips);
        let document = Document {
            clips: (0..3).map(dummy_clip).collect(),
            ..Document::default()
        };
        builder.finish(document).expect("valid same-load source")
    }

    fn loaded_source_with_channel_set(
        channels: SourceFactSetV1<animsmith_core::SourceChannelFactV1>,
    ) -> LoadedSource {
        let primary = InputIdentity::from_bytes(b"analytic-channel-coverage");
        let mut builder = RawSourceFactsBuilderV1::new(SourceFormatV1::GltfJson, primary);
        assert!(builder.push_clip(raw_clip_with_channel_set(
            0,
            GltfAnimationObservationV1::Observed {
                value: "walk".to_owned(),
            },
            channels,
        )));
        builder.mark_complete(SourceFactDomainV1::Clips);
        let document = Document {
            clips: vec![dummy_clip(0)],
            ..Document::default()
        };
        builder.finish(document).expect("valid same-load source")
    }

    fn frozen_bevy_provenance(source: &LoadedSource) -> PredictionProvenanceV1 {
        let profile = crate::resolve_static(crate::EngineDeclaration {
            selection: Some(crate::ProfileSelection::new(
                BEVY_FAMILY,
                BEVY_PROFILE_REVISION,
                BEVY_ENGINE_VERSION,
                BEVY_IMPORTER,
            )),
            ..crate::EngineDeclaration::default()
        })
        .unwrap()
        .unwrap()
        .resolve_input(SourceFormatV1::GltfJson, &["walk".to_owned()])
        .unwrap();
        crate::project_prediction_provenance_v1(&profile, source).unwrap()
    }

    fn direct_animation(index: usize, name: String) -> GltfAnimationAddressabilityAnimationV1 {
        GltfAnimationAddressabilityAnimationV1 {
            source_clip_index: index as u64,
            source_name: GltfAnimationObservationV1::Observed { value: name },
            normalized_clip_index: GltfAnimationObservationV1::Observed {
                value: index as u64,
            },
            channels: GltfAnimationAddressabilityChannelSetV1 {
                coverage: GltfAnimationCoverageV1 {
                    state: GltfAnimationCoverageStateV1::Complete,
                    reason: None,
                },
                rows: Vec::new(),
            },
        }
    }

    fn direct_inventory(
        animations: Vec<GltfAnimationAddressabilityAnimationV1>,
    ) -> GltfAnimationAddressabilityInventoryV1 {
        let primary_input = InputIdentity::from_bytes(b"direct");
        direct_inventory_with_closure(
            primary_input.clone(),
            DependencyClosureV1::unavailable(primary_input),
            animations,
        )
    }

    fn direct_inventory_with_closure(
        primary_input: InputIdentity,
        dependency_closure: DependencyClosureV1,
        animations: Vec<GltfAnimationAddressabilityAnimationV1>,
    ) -> GltfAnimationAddressabilityInventoryV1 {
        let mut value = GltfAnimationAddressabilityInventoryV1 {
            schema: GLTF_ANIMATION_ADDRESSABILITY_V1_ID,
            identity: GltfAnimationAddressabilityIdentityV1(InputIdentity::from_bytes(&[])),
            source_format: SourceFormatV1::GltfJson,
            primary_input: primary_input.clone(),
            dependency_closure,
            animations: GltfAnimationAddressabilityAnimationSetV1 {
                coverage: GltfAnimationCoverageV1 {
                    state: GltfAnimationCoverageStateV1::Complete,
                    reason: None,
                },
                rows: animations,
            },
        };
        value
            .validate_without_identity()
            .expect("valid direct inventory");
        value.identity = GltfAnimationAddressabilityIdentityV1(value.computed_identity());
        value
    }

    fn bevy_provenance(source: &LoadedSource) -> PredictionProvenanceV1 {
        let all_fact_ids = [
            EngineFactIdV1::AcceptedInputs,
            EngineFactIdV1::AnimationAddressability,
            EngineFactIdV1::AnimationChannelHandling,
            EngineFactIdV1::AnimationTargetAddressability,
            EngineFactIdV1::AxisConversionControl,
            EngineFactIdV1::ConstructHandling,
            EngineFactIdV1::ExactAxisConversion,
            EngineFactIdV1::ExtensionHandling,
            EngineFactIdV1::ResultingHierarchyScale,
            EngineFactIdV1::RootMotionAddressability,
            EngineFactIdV1::TargetCoordinateBasis,
            EngineFactIdV1::TargetLinearUnit,
            EngineFactIdV1::UnitConversionControl,
            EngineFactIdV1::WholeEndFrameRequired,
        ];
        let facts = all_fact_ids
            .into_iter()
            .map(|id| {
                let state = match id {
                    EngineFactIdV1::AcceptedInputs => {
                        EngineFactStateV1::Known(EngineFactValueV1::AcceptedFormats(vec![
                            SourceFormatV1::GltfJson,
                            SourceFormatV1::Glb,
                        ]))
                    }
                    EngineFactIdV1::AnimationAddressability => {
                        EngineFactStateV1::Known(EngineFactValueV1::AnimationAddressability(
                            EngineAnimationAddressabilityV1::GltfAssetLabel,
                        ))
                    }
                    _ => EngineFactStateV1::Unknown,
                };
                EngineProfileFactV1::new(id, state)
            })
            .collect();
        let profile = ResolvedEngineProfileV1::new(
            EngineProfileSelectionV1::new(
                BEVY_FAMILY,
                BEVY_PROFILE_REVISION,
                BEVY_ENGINE_VERSION,
                BEVY_IMPORTER,
            )
            .unwrap(),
            "urn:animsmith:engine-profile:bevy:test",
            facts,
            Vec::new(),
            vec![
                EnginePrimarySourceV1::new(
                    BEVY_ANIMATION_LABEL_SOURCE,
                    "test",
                    "https://example.invalid/bevy",
                    "2026-08-20",
                    vec![
                        EngineFactIdV1::AcceptedInputs,
                        EngineFactIdV1::AnimationAddressability,
                    ],
                    Vec::new(),
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let settings = ResolvedEngineSettingsV1::new(&profile, Vec::new(), Vec::new()).unwrap();
        PredictionProvenanceV1::new(
            profile,
            source.source_facts().format(),
            settings,
            RawSourceBindingV1::from_source(source.source_facts()),
            source.dependency_closure().clone(),
        )
        .unwrap()
    }

    fn disabled_addressability_check(source: &LoadedSource) -> CheckEvaluation {
        struct Stub;
        impl Check for Stub {
            fn id(&self) -> &'static str {
                ENGINE_ADDRESSABILITY_CHECK_ID
            }

            fn applicability(&self, _ctx: &CheckCtx<'_>) -> Applicability {
                Applicability::Applicable
            }

            fn evaluate(&self, _ctx: &CheckCtx<'_>) -> CheckOutput {
                panic!("disabled check must not evaluate")
            }
        }

        let mut config = Config::default();
        config.checks = BTreeMap::from([(
            ENGINE_ADDRESSABILITY_CHECK_ID.to_owned(),
            animsmith_core::config::CheckSettings {
                severity: Some(SeveritySetting::Off),
                ..animsmith_core::config::CheckSettings::default()
            },
        )]);
        let roles = resolve_configured_roles(&source.document().skeleton, &config.rig);
        let grids = MetricGrids::new(source.document());
        let ctx = CheckCtx::new(&grids, &roles, &config);
        evaluate_checks(&ctx, &[Box::new(Stub)], CheckSelection::All)
            .unwrap()
            .pop()
            .unwrap()
    }

    #[test]
    fn projection_preserves_source_identity_names_channels_and_accessors() {
        let source = loaded_source(SourceFormatV1::GltfJson);
        assert!(
            !source.dependency_closure().coverage().is_complete(),
            "the analytic source deliberately lacks complete dependency capture"
        );
        let inventory =
            GltfAnimationAddressabilityInventoryV1::from_source(&source).expect("glTF projection");
        assert_eq!(inventory.contract_id(), GLTF_ANIMATION_ADDRESSABILITY_V1_ID);
        assert_eq!(
            inventory.identity().input_identity().sha256(),
            "e6fa6c7e7cc8dbe5e61a8365f5c9c8a5e1c6343d3a5c0416eb1c2ea60cfba1b5"
        );
        assert_eq!(inventory.identity().input_identity().bytes(), 1_050);
        assert_eq!(
            inventory.primary_input(),
            source.source_facts().primary_identity()
        );
        assert_eq!(inventory.dependency_closure(), source.dependency_closure());
        assert_eq!(
            inventory.animations().coverage().state(),
            GltfAnimationCoverageStateV1::Complete
        );
        let rows = inventory.animations().rows();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].source_clip_index(), 0);
        assert_eq!(
            rows[0].source_name(),
            &GltfAnimationObservationV1::Observed {
                value: "walk".to_owned()
            }
        );
        assert_eq!(
            rows[1].source_name(),
            &GltfAnimationObservationV1::ProvenAbsent
        );
        assert_eq!(
            rows[2].source_name(),
            &GltfAnimationObservationV1::Observed {
                value: "walk".to_owned()
            }
        );
        let channels = rows[0].channels().rows();
        assert_eq!(channels.len(), 4);
        assert_eq!(
            channels[3].property(),
            GltfAnimationChannelPropertyV1::Weights
        );
        assert_eq!(channels[3].target().kind(), GltfAnimationTargetKindV1::Node);
        assert_eq!(channels[3].target().index(), 3);
        assert_eq!(channels[3].input_accessor_index(), 6);
        assert_eq!(channels[3].output_accessor_index(), 7);
    }

    #[test]
    fn public_nested_readback_rejects_impossible_values() {
        assert!(
            serde_json::from_value::<GltfAnimationCoverageV1>(serde_json::json!({
                "state": "complete",
                "reason": "malformed"
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<GltfAnimationCoverageV1>(serde_json::json!({
                "state": "complete",
                "reason": null
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<GltfAnimationTargetV1>(serde_json::json!({
                "kind": "element",
                "index": 0
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<GltfAnimationAddressabilityChannelV1>(serde_json::json!({
                "source_channel_index": 0,
                "target": {"kind": "node", "index": 0},
                "property": "translation",
                "input_accessor_index": 1,
                "output_accessor_index": null
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<GltfAnimationAddressabilityChannelSetV1>(serde_json::json!({
                "coverage": {"state": "unavailable", "reason": "parser_unavailable"},
                "rows": [{
                    "source_channel_index": 0,
                    "target": {"kind": "node", "index": 0},
                    "property": "translation",
                    "input_accessor_index": 1,
                    "output_accessor_index": 2
                }]
            }))
            .is_err()
        );

        let channel_at_limit = serde_json::json!({
            "source_channel_index": animsmith_core::RAW_SOURCE_V1_MAX_OBSERVATIONS - 2,
            "target": {"kind": "node", "index": 0},
            "property": "translation",
            "input_accessor_index": 1,
            "output_accessor_index": 2
        });
        assert!(
            serde_json::from_value::<GltfAnimationAddressabilityChannelV1>(
                channel_at_limit.clone()
            )
            .is_ok()
        );
        let mut channel_over_limit = channel_at_limit;
        channel_over_limit["source_channel_index"] =
            serde_json::json!(animsmith_core::RAW_SOURCE_V1_MAX_OBSERVATIONS - 1);
        assert!(
            serde_json::from_value::<GltfAnimationAddressabilityChannelV1>(channel_over_limit)
                .unwrap_err()
                .to_string()
                .contains("source channel index")
        );

        let animation_at_limit = serde_json::json!({
            "source_clip_index": animsmith_core::RAW_SOURCE_V1_MAX_CLIPS - 1,
            "source_name": {"state": "proven_absent"},
            "normalized_clip_index": {"state": "proven_absent"},
            "channels": {
                "coverage": {"state": "complete"},
                "rows": []
            }
        });
        assert!(
            serde_json::from_value::<GltfAnimationAddressabilityAnimationV1>(
                animation_at_limit.clone()
            )
            .is_ok()
        );
        let mut animation_over_limit = animation_at_limit;
        animation_over_limit["source_clip_index"] =
            serde_json::json!(animsmith_core::RAW_SOURCE_V1_MAX_CLIPS);
        assert!(
            serde_json::from_value::<GltfAnimationAddressabilityAnimationV1>(animation_over_limit)
                .unwrap_err()
                .to_string()
                .contains("source clip index")
        );
    }

    #[test]
    fn partial_and_unavailable_channel_coverage_round_trip_without_blocking_selectors() {
        let cases = [
            (
                SourceFactSetV1::partial(
                    vec![raw_channel(0, SourceChannelPropertyV1::Translation)],
                    SourceUnavailableReasonV1::ParserUnavailable,
                ),
                GltfAnimationCoverageStateV1::Partial,
                1,
            ),
            (
                SourceFactSetV1::unavailable(SourceUnavailableReasonV1::ParserUnavailable),
                GltfAnimationCoverageStateV1::Unavailable,
                0,
            ),
        ];
        let mut identities = Vec::new();

        for (channels, expected_state, expected_rows) in cases {
            let source = loaded_source_with_channel_set(channels);
            let inventory = GltfAnimationAddressabilityInventoryV1::from_source(&source).unwrap();
            assert_eq!(
                inventory.animations().coverage().state(),
                GltfAnimationCoverageStateV1::Complete
            );
            let projected_channels = inventory.animations().rows()[0].channels();
            assert_eq!(projected_channels.coverage().state(), expected_state);
            assert!(projected_channels.coverage().reason().is_some());
            assert_eq!(projected_channels.rows().len(), expected_rows);
            identities.push(inventory.identity().clone());

            let provenance = frozen_bevy_provenance(&source);
            let grids = MetricGrids::new(source.document());
            let roles = resolve_configured_roles(
                &source.document().skeleton,
                &animsmith_core::Config::default().rig,
            );
            let config = animsmith_core::Config::default();
            let adapter = crate::build_bevy_animation_addressability_adapter_v1(
                &source,
                &inventory,
                Some(provenance),
                &CheckCtx::new(&grids, &roles, &config),
            )
            .unwrap()
            .unwrap();
            let report = GltfAnimationAddressabilityV1::new(
                ToolInfo::animsmith(ToolSource::new(None, None)),
                inventory.clone(),
                Some(adapter),
            )
            .unwrap();
            let json = serde_json::to_vec(&report).unwrap();
            let readback = GltfAnimationAddressabilityInput::read_from(Cursor::new(json))
                .unwrap()
                .into_report()
                .unwrap();
            assert_eq!(readback.inventory(), &inventory);
            let check = readback.bevy().unwrap().check();
            assert_eq!(check.evaluation(), EvaluationState::Complete);
            let prediction = check.prediction().unwrap();
            assert_eq!(prediction.facets().len(), 1);
            assert_eq!(
                prediction.facets()[0].state(),
                EnginePredictionFacetStateV1::Available
            );
            assert_eq!(
                prediction.facets()[0].scope().subject.as_deref(),
                Some("Animation0")
            );
        }

        assert_ne!(identities[0], identities[1]);
    }

    #[test]
    fn strict_reader_rejects_explicit_null_prediction_for_a_disabled_check() {
        let source = loaded_source(SourceFormatV1::GltfJson);
        let inventory = GltfAnimationAddressabilityInventoryV1::from_source(&source).unwrap();
        let provenance = frozen_bevy_provenance(&source);
        let adapter = GltfAnimationAddressabilityBevyAdapterV1::new(
            provenance,
            disabled_addressability_check(&source),
            &inventory,
        )
        .unwrap();
        let report = GltfAnimationAddressabilityV1::new(
            ToolInfo::animsmith(ToolSource::new(None, None)),
            inventory,
            Some(adapter),
        )
        .unwrap();
        let mut json = serde_json::to_value(report).unwrap();
        assert!(json["bevy"]["check"].get("prediction").is_none());
        json["bevy"]["check"]["prediction"] = serde_json::Value::Null;
        let bytes = serde_json::to_vec(&json).unwrap();
        let input = GltfAnimationAddressabilityInput::read_from(Cursor::new(bytes)).unwrap();
        assert!(matches!(
            input.into_report(),
            Err(GltfAnimationAddressabilityError::InvalidBevyCheckShape { .. })
        ));
    }

    #[test]
    fn projection_rejects_non_gltf_without_mutating_raw_facts() {
        let source = loaded_source(SourceFormatV1::Fbx);
        assert_eq!(
            GltfAnimationAddressabilityInventoryV1::from_source(&source),
            Err(GltfAnimationAddressabilityError::UnsupportedSourceFormat {
                format: SourceFormatV1::Fbx
            })
        );
        assert_eq!(source.source_facts().clips().rows().len(), 3);
    }

    #[test]
    fn inventory_round_trip_is_strict_and_identity_bound() {
        let inventory = GltfAnimationAddressabilityInventoryV1::from_source(&loaded_source(
            SourceFormatV1::Glb,
        ))
        .unwrap();
        let json = serde_json::to_value(&inventory).unwrap();
        let round_trip: GltfAnimationAddressabilityInventoryV1 =
            serde_json::from_value(json.clone()).unwrap();
        assert_eq!(round_trip, inventory);

        let mut unknown = json.clone();
        unknown["unknown"] = serde_json::json!(true);
        assert!(serde_json::from_value::<GltfAnimationAddressabilityInventoryV1>(unknown).is_err());
        let mut changed = json;
        changed["animations"]["rows"][0]["source_name"]["value"] = serde_json::json!("run");
        assert!(
            serde_json::from_value::<GltfAnimationAddressabilityInventoryV1>(changed)
                .unwrap_err()
                .to_string()
                .contains("canonical engine-neutral preimage")
        );
    }

    #[test]
    fn gltf_rows_reject_non_node_targets_and_missing_accessors() {
        let inventory = GltfAnimationAddressabilityInventoryV1::from_source(&loaded_source(
            SourceFormatV1::GltfJson,
        ))
        .unwrap();
        let json = serde_json::to_value(&inventory).unwrap();

        let mut non_node = json.clone();
        non_node["animations"]["rows"][0]["channels"]["rows"][0]["target"]["kind"] =
            serde_json::json!("element");
        let error = serde_json::from_value::<GltfAnimationAddressabilityInventoryV1>(non_node)
            .unwrap_err()
            .to_string();
        assert!(error.contains("target source nodes"), "{error}");

        let mut missing_accessors = json;
        missing_accessors["animations"]["rows"][0]["channels"]["rows"][0]["input_accessor_index"] =
            serde_json::Value::Null;
        missing_accessors["animations"]["rows"][0]["channels"]["rows"][0]["output_accessor_index"] =
            serde_json::Value::Null;
        let error =
            serde_json::from_value::<GltfAnimationAddressabilityInventoryV1>(missing_accessors)
                .unwrap_err()
                .to_string();
        assert!(error.contains("expected u64"), "{error}");
    }

    #[test]
    fn inventory_identity_changes_with_external_content_identity() {
        fn inventory(external: &[u8]) -> GltfAnimationAddressabilityInventoryV1 {
            let primary = InputIdentity::from_bytes(b"primary");
            let key = animsmith_core::DependencyResourceKeyV1::from_source_str(
                "buffer.bin",
                animsmith_core::ResourceKeySyntaxV1::GltfUri,
            )
            .unwrap();
            let mut builder = animsmith_core::DependencyClosureBuilderV1::new(
                primary.clone(),
                animsmith_core::SourceSetCoverageV1::complete(),
                1,
            );
            assert!(builder.begin_reference("buffer.bin".len(), 1));
            assert_eq!(builder.prepare_external_key(&key).unwrap(), Some(true));
            builder.record_external_open_attempt(&key).unwrap();
            assert!(
                builder
                    .push_captured_external(
                        0,
                        animsmith_core::SourceResourceKindV1::Buffer,
                        0,
                        key,
                        InputIdentity::from_bytes(external),
                    )
                    .unwrap()
            );
            direct_inventory_with_closure(primary, builder.finish().unwrap(), Vec::new())
        }

        let before = inventory(b"before");
        let after = inventory(b"after");
        assert_ne!(before.dependency_closure(), after.dependency_closure());
        assert_ne!(before.identity(), after.identity());
    }

    #[test]
    fn adapter_rejects_fabricated_profile_identity_and_coverage_binding_detects_mutation() {
        let source = loaded_source(SourceFormatV1::GltfJson);
        let inventory = GltfAnimationAddressabilityInventoryV1::from_source(&source).unwrap();
        let provenance = bevy_provenance(&source);
        let check = disabled_addressability_check(&source);
        assert_eq!(check.configuration(), ConfigurationState::Disabled);
        assert_ne!(
            provenance.profile().facts_identity().sha256(),
            BEVY_PROFILE_FACTS_SHA256
        );
        assert!(matches!(
            GltfAnimationAddressabilityBevyAdapterV1::new(provenance.clone(), check, &inventory,),
            Err(GltfAnimationAddressabilityError::InvalidBevyProfile)
        ));

        let mut changed = inventory.clone();
        changed.animations.coverage = GltfAnimationCoverageV1 {
            state: GltfAnimationCoverageStateV1::Partial,
            reason: Some(GltfAnimationUnavailableReasonV1::ParserUnavailable),
        };
        changed.identity = GltfAnimationAddressabilityIdentityV1(changed.computed_identity());
        assert_eq!(
            validate_inventory_provenance_binding(&provenance, &changed),
            Err(GltfAnimationAddressabilityError::BevyInventoryMismatch)
        );
    }

    #[test]
    fn animation_collection_accepts_n_and_rejects_n_plus_one() {
        let rows = (0..animsmith_core::RAW_SOURCE_V1_MAX_CLIPS)
            .map(|index| direct_animation(index, String::new()))
            .collect();
        let inventory = direct_inventory(rows);
        let mut json = serde_json::to_value(&inventory).unwrap();
        let rows = json["animations"]["rows"].as_array_mut().unwrap();
        rows.push(rows.last().unwrap().clone());
        let error = serde_json::from_value::<GltfAnimationAddressabilityInventoryV1>(json)
            .unwrap_err()
            .to_string();
        assert!(error.contains("exceeding the V1 limit"), "{error}");
    }

    #[test]
    fn aggregate_row_collection_accepts_n_and_rejects_n_plus_one() {
        let channels = (0..animsmith_core::RAW_SOURCE_V1_MAX_OBSERVATIONS - 1)
            .map(|index| GltfAnimationAddressabilityChannelV1 {
                source_channel_index: index as u64,
                target: GltfAnimationTargetV1 {
                    kind: GltfAnimationTargetKindV1::Node,
                    index: 0,
                },
                property: GltfAnimationChannelPropertyV1::Translation,
                input_accessor_index: (index * 2) as u64,
                output_accessor_index: (index * 2 + 1) as u64,
            })
            .collect();
        let mut animation = direct_animation(0, String::new());
        animation.channels.rows = channels;
        let inventory = direct_inventory(vec![animation]);
        let mut json = serde_json::to_value(&inventory).unwrap();
        let rows = json["animations"]["rows"][0]["channels"]["rows"]
            .as_array_mut()
            .unwrap();
        rows.push(rows.last().unwrap().clone());
        let error = serde_json::from_value::<GltfAnimationAddressabilityInventoryV1>(json)
            .unwrap_err()
            .to_string();
        assert!(error.contains("animation/channel rows"), "{error}");
    }

    #[test]
    fn per_text_and_aggregate_text_bounds_accept_n_and_reject_n_plus_one() {
        let at_per_text_limit = direct_inventory(vec![direct_animation(
            0,
            "x".repeat(animsmith_core::RAW_SOURCE_V1_MAX_TEXT_BYTES),
        )]);
        let mut too_long = serde_json::to_value(&at_per_text_limit).unwrap();
        too_long["animations"]["rows"][0]["source_name"]["value"] =
            serde_json::json!("x".repeat(animsmith_core::RAW_SOURCE_V1_MAX_TEXT_BYTES + 1));
        let error = serde_json::from_value::<GltfAnimationAddressabilityInventoryV1>(too_long)
            .unwrap_err()
            .to_string();
        assert!(error.contains("source name"), "{error}");

        let full_names = animsmith_core::RAW_SOURCE_V1_MAX_TOTAL_TEXT_BYTES
            / animsmith_core::RAW_SOURCE_V1_MAX_TEXT_BYTES;
        let at_aggregate_limit = direct_inventory(
            (0..full_names)
                .map(|index| {
                    direct_animation(
                        index,
                        "x".repeat(animsmith_core::RAW_SOURCE_V1_MAX_TEXT_BYTES),
                    )
                })
                .collect(),
        );
        let mut over = at_aggregate_limit.animations.rows.clone();
        over.push(direct_animation(over.len(), "x".to_owned()));
        let mut invalid = GltfAnimationAddressabilityInventoryV1 {
            animations: GltfAnimationAddressabilityAnimationSetV1 {
                coverage: at_aggregate_limit.animations.coverage,
                rows: over,
            },
            ..at_aggregate_limit
        };
        assert_eq!(
            invalid.validate_without_identity(),
            Err(GltfAnimationAddressabilityError::TooMuchText {
                found: animsmith_core::RAW_SOURCE_V1_MAX_TOTAL_TEXT_BYTES + 1,
                limit: animsmith_core::RAW_SOURCE_V1_MAX_TOTAL_TEXT_BYTES,
            })
        );
        invalid.animations.rows.pop();
        assert!(invalid.validate_without_identity().is_ok());
    }

    #[test]
    fn standalone_null_adapter_round_trips_and_cross_contracts_are_rejected() {
        let inventory = GltfAnimationAddressabilityInventoryV1::from_source(&loaded_source(
            SourceFormatV1::GltfJson,
        ))
        .unwrap();
        let report = GltfAnimationAddressabilityV1::new(
            ToolInfo::animsmith(ToolSource::new(None, None)),
            inventory.clone(),
            None,
        )
        .unwrap();
        let json = serde_json::to_vec(&report).unwrap();
        let read = GltfAnimationAddressabilityInput::read_from(Cursor::new(&json))
            .unwrap()
            .into_report()
            .unwrap();
        assert_eq!(read.inventory(), &inventory);
        assert!(read.bevy().is_none());

        let output_v10 = serde_json::json!({
            "schema_version": 10,
            "schema": animsmith_core::OUTPUT_SCHEMA_ID,
            "tool": {"name":"animsmith","version":"0.3.1","source":{"revision":null,"dirty":null}},
            "command": "measure",
            "summary": {"files":0},
            "files": []
        });
        assert!(serde_json::from_value::<GltfAnimationAddressabilityInput>(output_v10).is_err());
        assert!(
            serde_json::from_slice::<animsmith_core::MeasurementReportInput>(&json)
                .unwrap()
                .into_files()
                .is_err()
        );

        let mut invalid_tool = serde_json::to_value(&report).unwrap();
        invalid_tool["tool"]["name"] = serde_json::json!("other");
        let input: GltfAnimationAddressabilityInput = serde_json::from_value(invalid_tool).unwrap();
        assert!(matches!(
            input.into_report(),
            Err(GltfAnimationAddressabilityError::InvalidTool)
        ));

        let mut other_version = serde_json::to_value(&report).unwrap();
        other_version["tool"]["version"] = serde_json::json!("9.9.9-beta.1+pipeline");
        let input: GltfAnimationAddressabilityInput =
            serde_json::from_value(other_version).unwrap();
        assert!(
            input.into_report().is_ok(),
            "immutable V1 readback must not be coupled to the reader package version"
        );

        for invalid_version in ["", "1", "1.2", "1.2.3.4", "1.2.3+", "1.2.3 bad"] {
            let mut invalid = serde_json::to_value(&report).unwrap();
            invalid["tool"]["version"] = serde_json::json!(invalid_version);
            let input: GltfAnimationAddressabilityInput = serde_json::from_value(invalid).unwrap();
            assert!(matches!(
                input.into_report(),
                Err(GltfAnimationAddressabilityError::InvalidTool)
            ));
        }

        let mut missing_tool_field = serde_json::to_value(&report).unwrap();
        missing_tool_field["tool"]["source"]
            .as_object_mut()
            .unwrap()
            .remove("revision");
        let input: GltfAnimationAddressabilityInput =
            serde_json::from_value(missing_tool_field).unwrap();
        assert!(matches!(
            input.into_report(),
            Err(GltfAnimationAddressabilityError::InvalidTool)
        ));
    }

    #[test]
    fn staged_reader_accepts_byte_n_and_rejects_n_plus_one() {
        let exact = GltfAnimationAddressabilityInput::read_from_with_limit(Cursor::new(b"null"), 4);
        assert!(matches!(
            exact,
            Err(GltfAnimationAddressabilityReadError::InvalidJson { .. })
        ));
        let over = GltfAnimationAddressabilityInput::read_from_with_limit(Cursor::new(b"null "), 4);
        assert!(matches!(
            over,
            Err(GltfAnimationAddressabilityReadError::ReportTooLarge { limit: 4 })
        ));
    }
}
