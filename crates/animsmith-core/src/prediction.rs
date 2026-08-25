//! Registry-independent engine-prediction provenance and per-check evidence.
//!
//! The types in this module are immutable output-contract values. Engine
//! registries project their resolved facts and settings into the sibling
//! [`crate::engine_contract`] wire types; prediction records only consume those
//! projections and loader-owned evidence from the same [`crate::LoadedSource`].

use serde::de::{DeserializeSeed, Error as _, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::ser::{
    Error as _, SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant,
    SerializeTuple, SerializeTupleStruct, SerializeTupleVariant,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::value::RawValue;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;
use std::marker::PhantomData;

use crate::bounded_deserialize::{
    BudgetedCappedSequenceSeed, CappedSequence, CappedSequenceSeed, RowBudget,
    consume_ignored_tail, deserialize_capped_sequence,
};

use crate::dependency_closure::{
    DependencyClosureCoverageReasonV1, DependencyClosureCoverageV1, DependencyClosureDecodeError,
    DependencyReferenceTargetV1, DependencyResourcePurposeV1, DependencyResourceRefusalReasonV1,
    DependencyResourceUnavailableReasonV1, decode_dependency_closure_v1,
};
use crate::engine_contract::{
    CanonicalEncoder, ENGINE_PROFILE_FACTS_V1_ID, EngineContractDecodeError, EngineContractError,
    EngineProfileLimitedDecodeError, EngineSettingIdV1, EngineSettingScopeV1,
    EngineSettingsLimitedDecodeError, ResolvedEngineProfileV1, ResolvedEngineSettingsV1,
    ResolvedEngineSettingsV2, decode_resolved_engine_profile_v1_with_provenance_limit,
    decode_resolved_engine_settings_v1_with_provenance_limit,
    decode_resolved_engine_settings_v2_with_provenance_limit, encode_input_identity,
};
use crate::evaluation::{CoverageGap, EvaluationScope};
use crate::finding::Finding;
use crate::source_facts::{
    RAW_SOURCE_FACTS_V1_ID, RAW_SOURCE_V1_MAX_OBSERVATIONS, RAW_SOURCE_V1_MAX_TOTAL_TEXT_BYTES,
    RAW_SOURCE_V1_MAX_TRAVERSAL_DEPTH, SourceAxisV1, SourceChannelPropertyV1,
    SourceConstructKindV1, SourceCoordinateBasisV1, SourceFactsViewV1, SourceFormatV1,
    SourceFramesPerSecondV1, SourceInterpolationV1, SourceLinearUnitV1, SourceLoaderDispositionV1,
    SourceObservationStateV1, SourceObservationV1, SourceProvenanceKindV1, SourceProvenanceV1,
    SourceResourceKindV1, SourceResourceLocatorV1, SourceSetCoverageStateV1, SourceSetCoverageV1,
    SourceTargetKindV1, SourceUnavailableReasonV1,
};
use crate::{
    DEPENDENCY_CLOSURE_V1_ID, DependencyClosureV1, InputIdentity, MEASUREMENTS_SCHEMA_ID,
    MeasurementContract, OUTPUT_V10_SCHEMA_ID, SourceInverseBindAccessorStatus,
    SourceNodeLocalRest, SourceSkeletonCoverage,
};

/// Immutable prediction-provenance V1 schema identity.
pub const PREDICTION_PROVENANCE_V1_ID: &str = "urn:animsmith:prediction-provenance:1";
/// Immutable bounded-overflow prediction-provenance V2 schema identity.
pub const PREDICTION_PROVENANCE_V2_ID: &str = "urn:animsmith:prediction-provenance:2";
/// Immutable per-check engine-prediction V1 schema identity.
pub const ENGINE_PREDICTION_V1_ID: &str = "urn:animsmith:engine-prediction:1";
/// Immutable bounded-overflow engine-prediction V2 schema identity.
pub const ENGINE_PREDICTION_V2_ID: &str = "urn:animsmith:engine-prediction:2";
/// Maximum facets retained across one lint file.
pub const PREDICTION_V1_MAX_FACETS_PER_FILE: usize = 4_096;
/// Maximum basis references retained by one prediction facet.
pub const PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FACET: usize = 4_096;
/// Maximum basis references retained across one lint file.
pub const PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE: usize = 65_536;
/// Maximum UTF-8 bytes retained in one new prediction string.
pub const PREDICTION_V1_MAX_TEXT_BYTES: usize = 4_096;
/// Maximum aggregate new provenance/prediction text retained by one lint file.
pub const PREDICTION_V1_MAX_TOTAL_TEXT_BYTES_PER_FILE: usize = 8 * 1024 * 1024;
/// Maximum aggregate profile, settings, and raw-source rows retained in one file provenance.
pub const PREDICTION_V1_MAX_AGGREGATE_PROVENANCE_ROWS: usize = 65_536;
/// Maximum decoded components in one measurement JSON pointer.
pub const PREDICTION_V1_MAX_MEASUREMENT_POINTER_COMPONENTS: usize = 128;
/// Maximum unavailable-reason codes retained by one prediction facet.
pub const PREDICTION_V1_MAX_REASONS_PER_FACET: usize = 4_096;

/// Maximum candidate facets one V2 production rule may report before its
/// bounded N+1 sentinel replaces further counting.
pub const PREDICTION_V2_MAX_CANDIDATE_FACETS_PER_RULE: usize = 4_096;

/// Bounded candidate demand reported before V2 production-rule evaluation.
///
/// A producer must count only until the first excess candidate. `NPlusOne`
/// never carries or retains the omitted candidate payload; it says only that
/// demand is greater than the per-rule bound. The file allocator consumes this
/// value before any rule emits facets, preventing orphaned finding bindings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PredictionFacetDemandV2 {
    /// The exact demand was counted within the bound.
    Exact(usize),
    /// Counting reached the first candidate after the bound.
    NPlusOne,
}

impl PredictionFacetDemandV2 {
    /// Construct a bounded exact demand.
    pub fn exact(count: usize) -> Result<Self, PredictionContractError> {
        if count > PREDICTION_V2_MAX_CANDIDATE_FACETS_PER_RULE {
            return Err(PredictionContractError::TooManyFacets {
                found: count,
                limit: PREDICTION_V2_MAX_CANDIDATE_FACETS_PER_RULE,
            });
        }
        Ok(Self::Exact(count))
    }

    /// Return the retained candidate count, treating N+1 as the cap.
    pub const fn bounded_count(self) -> usize {
        match self {
            Self::Exact(count) => count,
            Self::NPlusOne => PREDICTION_V2_MAX_CANDIDATE_FACETS_PER_RULE,
        }
    }

    /// Whether the actual demand exceeded the counted bound.
    pub const fn overflowed(self) -> bool {
        matches!(self, Self::NPlusOne)
    }
}

/// Catalog-ordered demand for one V2 production rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PredictionRuleDemandV2<'a> {
    rule_id: &'a str,
    demand: PredictionFacetDemandV2,
}

impl<'a> PredictionRuleDemandV2<'a> {
    /// Construct one catalog entry. The caller supplies entries in immutable
    /// registration order; this type deliberately has no map-based form.
    pub fn new(
        rule_id: &'a str,
        demand: PredictionFacetDemandV2,
    ) -> Result<Self, PredictionContractError> {
        stable_token("production rule id", rule_id)?;
        // `Exact` is intentionally public for serde-free, allocation-only
        // callers.  Keep the contract at this boundary as well as in
        // `PredictionFacetDemandV2::exact`: a direct enum construction must
        // never smuggle an unbounded demand into the production allocator.
        if let PredictionFacetDemandV2::Exact(count) = demand
            && count > PREDICTION_V2_MAX_CANDIDATE_FACETS_PER_RULE
        {
            return Err(PredictionContractError::TooManyFacets {
                found: count,
                limit: PREDICTION_V2_MAX_CANDIDATE_FACETS_PER_RULE,
            });
        }
        Ok(Self { rule_id, demand })
    }

    /// Stable production-rule id.
    pub const fn rule_id(&self) -> &str {
        self.rule_id
    }

    /// Bounded pre-evaluation candidate demand.
    pub const fn demand(&self) -> PredictionFacetDemandV2 {
        self.demand
    }
}

/// Allocated V2 candidate capacity for one catalog production rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PredictionRuleAllocationV2<'a> {
    rule_id: &'a str,
    candidate_capacity: usize,
    summary_required: bool,
}

impl<'a> PredictionRuleAllocationV2<'a> {
    /// Stable production-rule id.
    pub const fn rule_id(&self) -> &str {
        self.rule_id
    }

    /// Candidate facets this rule may construct before its summary facet.
    pub const fn candidate_capacity(&self) -> usize {
        self.candidate_capacity
    }

    /// Whether omitted candidates must be replaced by one unavailable summary.
    pub const fn summary_required(&self) -> bool {
        self.summary_required
    }

    /// Total emitted slots for this rule.
    pub const fn emitted_slots(&self) -> usize {
        self.candidate_capacity + if self.summary_required { 1 } else { 0 }
    }
}

/// Allocate the shared V2 file facet budget in catalog registration order.
///
/// Later nonzero rules reserve one summary slot before earlier candidates are
/// admitted. Thus every production rule with demand remains represented, while
/// a lone rule requesting exactly 4,096 candidates receives all 4,096.
pub fn allocate_prediction_facets_v2<'a>(
    catalog_order: &'a [PredictionRuleDemandV2<'a>],
) -> Result<Vec<PredictionRuleAllocationV2<'a>>, PredictionContractError> {
    let mut registered = BTreeMap::new();
    for entry in catalog_order {
        if registered.insert(entry.rule_id, ()).is_some() {
            return Err(PredictionContractError::DuplicateProductionRule(
                entry.rule_id.to_owned(),
            ));
        }
    }
    let mut remaining = PREDICTION_V1_MAX_FACETS_PER_FILE;
    let mut allocations = Vec::with_capacity(catalog_order.len());
    for (index, entry) in catalog_order.iter().enumerate() {
        let later_nonzero = catalog_order[index + 1..]
            .iter()
            .filter(|later| later.demand.bounded_count() != 0)
            .count();
        let available = remaining.checked_sub(later_nonzero).ok_or(
            PredictionContractError::ArithmeticOverflow("facet reservation"),
        )?;
        let demand = entry.demand.bounded_count();
        let truncated = entry.demand.overflowed() || demand > available;
        let candidate_capacity = if truncated {
            available.saturating_sub(1).min(demand)
        } else {
            demand
        };
        let summary_required = truncated;
        remaining = remaining
            .checked_sub(candidate_capacity + usize::from(summary_required))
            .ok_or(PredictionContractError::ArithmeticOverflow(
                "facet allocation",
            ))?;
        allocations.push(PredictionRuleAllocationV2 {
            rule_id: entry.rule_id,
            candidate_capacity,
            summary_required,
        });
    }
    Ok(allocations)
}

fn deserialize_basis_references<'de, D>(
    deserializer: D,
) -> Result<CappedSequence<PredictionBasisReferenceWireV1>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_capped_sequence(deserializer, PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FACET)
}

fn deserialize_unavailable_reasons<'de, D>(
    deserializer: D,
) -> Result<CappedSequence<String>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_capped_sequence(deserializer, PREDICTION_V1_MAX_REASONS_PER_FACET)
}

fn deserialize_consumed_contracts<'de, D>(
    deserializer: D,
) -> Result<CappedSequence<String>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_capped_sequence(deserializer, CONSUMED_CONTRACTS_V1.len())
}

/// A prediction/provenance value violated the immutable V1 contract.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum PredictionContractError {
    /// An embedded profile or settings value was invalid.
    #[error("invalid embedded engine contract: {0}")]
    InvalidEngineContract(#[from] EngineContractError),
    /// Catalog registration repeated one V2 production-rule id.
    #[error("duplicate V2 prediction production rule {0:?}")]
    DuplicateProductionRule(String),
    /// An embedded dependency-closure wire was structurally valid but violated
    /// its immutable semantic contract.
    #[error("invalid embedded dependency closure: {0}")]
    InvalidDependencyClosure(String),
    /// A retained string exceeded its per-value bound.
    #[error("prediction {field} is {bytes} UTF-8 bytes, exceeding the V1 limit of {limit}")]
    TextTooLong {
        /// Semantic field being retained.
        field: &'static str,
        /// Actual UTF-8 byte count.
        bytes: usize,
        /// Contract limit.
        limit: usize,
    },
    /// A stable token was empty, contained controls, or used invalid syntax.
    #[error("invalid prediction {field} token {value:?}")]
    InvalidToken {
        /// Semantic token field.
        field: &'static str,
        /// Rejected spelling.
        value: String,
    },
    /// A finite-number scalar contained NaN or infinity.
    #[error("prediction finite_number must be finite")]
    NonFiniteNumber,
    /// A measurement pointer was not a canonical measurements-root RFC 6901 pointer.
    #[error("invalid measurements-v15 JSON pointer {0:?}")]
    InvalidMeasurementPointer(String),
    /// A measurement pointer did not resolve in the validated contract.
    #[error("measurement pointer {0:?} does not resolve")]
    MeasurementPointerMissing(String),
    /// A measurement pointer resolved to an object or array rather than a scalar.
    #[error("measurement pointer {0:?} does not resolve to a scalar")]
    MeasurementPointerNotScalar(String),
    /// A measurement pointer's retained scalar disagreed with the validated contract.
    #[error("measurement pointer {0:?} scalar disagrees with measurements-v15")]
    MeasurementValueMismatch(String),
    /// A measurement pointer exceeded its component bound.
    #[error("measurement pointer has {components} components, exceeding the V1 limit of {limit}")]
    TooManyMeasurementPointerComponents {
        /// Decoded component count.
        components: usize,
        /// Contract limit.
        limit: usize,
    },
    /// A raw-source domain and stable key identify different row kinds.
    #[error("raw-source domain and row key disagree")]
    RawSourceDomainKeyMismatch,
    /// The stable raw-source row key was absent from the same-load facts.
    #[error("raw-source basis row was not found")]
    RawSourceRowNotFound,
    /// The field id does not name a scalar on the selected raw-source row.
    #[error("raw-source basis field {0:?} is not available on the selected row")]
    RawSourceFieldUnavailable(String),
    /// The retained raw-source scalar disagrees with same-load facts.
    #[error("raw-source basis scalar disagrees with same-load facts")]
    RawSourceValueMismatch,
    /// One facet exceeded the per-facet basis-reference bound.
    #[error("prediction basis has {found} references, exceeding the V1 limit of {limit}")]
    TooManyBasisReferences {
        /// Supplied reference count.
        found: usize,
        /// Contract limit.
        limit: usize,
    },
    /// Canonical basis rows contained an exact duplicate.
    #[error("prediction basis contains a duplicate reference")]
    DuplicateBasisReference,
    /// An available facet had no evidence basis.
    #[error("available prediction facet must have a nonempty basis")]
    AvailableBasisEmpty,
    /// An available facet carried an unavailable reason.
    #[error("available prediction facet cannot carry unavailable reasons")]
    AvailableHasReasons,
    /// A required-unavailable facet carried no stable reason.
    #[error("required-unavailable prediction facet must carry at least one reason")]
    RequiredUnavailableWithoutReason,
    /// A reason list contained an exact duplicate.
    #[error("prediction facet contains duplicate unavailable reason {0:?}")]
    DuplicateUnavailableReason(String),
    /// A custom reason code was not a bounded namespaced ASCII code.
    #[error("invalid prediction-unavailable reason code {0:?}")]
    InvalidUnavailableReasonCode(String),
    /// One facet exceeded the unavailable-reason bound.
    #[error("prediction facet has {found} reasons, exceeding the V1 limit of {limit}")]
    TooManyUnavailableReasons {
        /// Supplied reason count.
        found: usize,
        /// Contract limit.
        limit: usize,
    },
    /// An engine prediction did not contain any facets.
    #[error("engine prediction must contain at least one facet")]
    EmptyFacetList,
    /// One engine prediction exceeded the facet bound.
    #[error("engine prediction has {found} facets, exceeding the V1 limit of {limit}")]
    TooManyFacets {
        /// Supplied facet count.
        found: usize,
        /// Contract limit.
        limit: usize,
    },
    /// Canonical facets reused a scope.
    #[error("engine prediction contains duplicate facet scope")]
    DuplicateFacetScope,
    /// The profile, raw-source binding, and header source formats disagreed.
    #[error("prediction provenance source formats disagree")]
    SourceFormatMismatch,
    /// The resolved profile does not accept the same-load source format.
    #[error("prediction provenance source format is not accepted by the resolved profile")]
    SourceFormatNotAccepted,
    /// Raw-source and dependency-closure primary identities disagreed.
    #[error("prediction provenance primary input identities disagree")]
    PrimaryInputMismatch,
    /// Raw resource coverage and the same-load dependency closure disagreed.
    #[error("prediction provenance raw-resource and dependency-closure coverage disagree")]
    DependencyClosureCoverageMismatch,
    /// A prediction refers to a different file provenance identity.
    #[error("engine prediction provenance identity does not match its lint file")]
    ProvenanceIdentityMismatch,
    /// A basis profile-fact id is absent from the embedded profile.
    #[error("prediction basis names unknown profile fact {0:?}")]
    UnknownProfileFact(String),
    /// A basis setting location/id is absent or contradicts the profile descriptor.
    #[error("prediction basis names unknown or mismatched resolved setting {0:?}")]
    UnknownResolvedSetting(String),
    /// A basis primary-source id is absent from the embedded profile.
    #[error("prediction basis names unknown primary source {0:?}")]
    UnknownPrimarySource(String),
    /// An available facet scope was absent or duplicated in completed scopes.
    #[error("available prediction facet scope must occur exactly once in evaluated_scopes")]
    AvailableScopeNotEvaluatedExactlyOnce,
    /// A V2 shared-file budget summary was not the canonical unavailable
    /// summary facet for its owning production rule.
    #[error("facet-budget summary is not the canonical rule-scoped unavailable facet")]
    InvalidFacetBudgetSummary,
    /// A V2 production rule emitted more than one shared-file budget summary.
    #[error("engine prediction contains multiple facet-budget summaries")]
    DuplicateFacetBudgetSummary,
    /// The current engine-addressability inventory did not carry the exact
    /// raw/settings incompleteness reasons implied by its V2 provenance.
    #[error("engine-addressability inventory reasons contradict V2 provenance coverage")]
    EngineAddressabilityInventoryReasonsMismatch,
    /// The current engine-addressability available facets did not retain the
    /// canonical source-index prefix.
    #[error("engine-addressability facets are not the canonical source-index prefix")]
    EngineAddressabilityFacetPrefixMismatch,
    /// A required-unavailable facet scope was also reported as completed.
    #[error("required-unavailable prediction facet scope cannot occur in evaluated_scopes")]
    UnavailableScopeEvaluated,
    /// A required-unavailable facet was duplicated as an ordinary coverage gap.
    #[error("required-unavailable prediction facet scope cannot occur in gaps")]
    UnavailableScopeDuplicatedAsGap,
    /// A prediction-bearing finding lacked its required facet binding.
    #[error("finding on a prediction-bearing check has no prediction_scope")]
    FindingMissingPredictionScope,
    /// A finding scope did not name exactly one available facet.
    #[error("finding prediction_scope does not identify an available facet")]
    FindingScopeNotAvailable,
    /// A canonical identity did not match its recomputed preimage.
    #[error("{contract} identity does not match its canonical V1 preimage")]
    IdentityMismatch {
        /// Contract whose identity was rejected.
        contract: &'static str,
    },
    /// A schema field did not carry its immutable V1 identity.
    #[error("{field} must be {expected:?}, found {found:?}")]
    InvalidSchema {
        /// Field carrying the schema id.
        field: &'static str,
        /// Required identity.
        expected: &'static str,
        /// Supplied identity.
        found: String,
    },
    /// A canonical ordered collection was supplied out of order.
    #[error("prediction {0} is not in canonical order")]
    NonCanonicalOrder(&'static str),
    /// The exact derived consumed-contract inventory was changed.
    #[error("prediction provenance consumed-contract inventory is invalid")]
    InvalidConsumedContracts,
    /// Aggregate retained text exceeded its file bound.
    #[error("prediction retains {found} UTF-8 bytes, exceeding the V1 limit of {limit}")]
    TooMuchRetainedText {
        /// Observed retained text.
        found: usize,
        /// Contract limit.
        limit: usize,
    },
    /// Aggregate embedded profile, settings, and raw-source rows exceeded the file bound.
    #[error("prediction provenance retains {found} rows, exceeding the V1 limit of {limit}")]
    TooManyAggregateProvenanceRows {
        /// Observed aggregate retained row count.
        found: usize,
        /// Immutable V1 row limit.
        limit: usize,
    },
    /// Checked accounting overflowed.
    #[error("prediction {0} accounting overflowed")]
    ArithmeticOverflow(&'static str),
}

fn bounded_string(
    field: &'static str,
    value: impl Into<String>,
) -> Result<String, PredictionContractError> {
    let value = value.into();
    if value.len() > PREDICTION_V1_MAX_TEXT_BYTES {
        return Err(PredictionContractError::TextTooLong {
            field,
            bytes: value.len(),
            limit: PREDICTION_V1_MAX_TEXT_BYTES,
        });
    }
    Ok(value)
}

fn stable_token(
    field: &'static str,
    value: impl Into<String>,
) -> Result<String, PredictionContractError> {
    let value = bounded_string(field, value)?;
    if value.is_empty() || value.chars().any(char::is_control) {
        return Err(PredictionContractError::InvalidToken { field, value });
    }
    Ok(value)
}

fn checked_sum(
    field: &'static str,
    values: impl IntoIterator<Item = usize>,
) -> Result<usize, PredictionContractError> {
    values.into_iter().try_fold(0usize, |total, value| {
        total
            .checked_add(value)
            .ok_or(PredictionContractError::ArithmeticOverflow(field))
    })
}

/// Canonical finite binary64 value retained by prediction basis records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FinitePredictionNumberV1(u64);

impl FinitePredictionNumberV1 {
    /// Normalize and retain one finite binary64 value.
    pub fn new(value: f64) -> Result<Self, PredictionContractError> {
        if !value.is_finite() {
            return Err(PredictionContractError::NonFiniteNumber);
        }
        let value = if value == 0.0 { 0.0 } else { value };
        Ok(Self(value.to_bits()))
    }

    /// Normalized finite value.
    pub fn get(self) -> f64 {
        f64::from_bits(self.0)
    }

    fn canonical_bits(self) -> String {
        format!("{:016x}", self.0)
    }
}

impl Serialize for FinitePredictionNumberV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_f64(self.get())
    }
}

impl<'de> Deserialize<'de> for FinitePredictionNumberV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(f64::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

/// Closed scalar vocabulary used by prediction bases.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum PredictionScalarV1 {
    /// Explicit null value.
    Null,
    /// Boolean value.
    Boolean {
        /// Exact value.
        value: bool,
    },
    /// Signed integer value.
    SignedInteger {
        /// Exact value.
        value: i64,
    },
    /// Unsigned integer value.
    UnsignedInteger {
        /// Exact value.
        value: u64,
    },
    /// Finite binary64 value.
    FiniteNumber {
        /// Exact normalized value.
        value: FinitePredictionNumberV1,
    },
    /// Stable machine token.
    Token {
        /// Bounded token value.
        value: String,
    },
    /// Bounded human/source text.
    Text {
        /// Bounded text value.
        value: String,
    },
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum PredictionScalarWireV1 {
    Null,
    Boolean { value: bool },
    SignedInteger { value: i64 },
    UnsignedInteger { value: u64 },
    FiniteNumber { value: FinitePredictionNumberV1 },
    Token { value: String },
    Text { value: String },
}

impl TryFrom<PredictionScalarWireV1> for PredictionScalarV1 {
    type Error = PredictionContractError;

    fn try_from(wire: PredictionScalarWireV1) -> Result<Self, Self::Error> {
        match wire {
            PredictionScalarWireV1::Null => Ok(Self::Null),
            PredictionScalarWireV1::Boolean { value } => Ok(Self::Boolean { value }),
            PredictionScalarWireV1::SignedInteger { value } => Ok(Self::SignedInteger { value }),
            PredictionScalarWireV1::UnsignedInteger { value } => {
                Ok(Self::UnsignedInteger { value })
            }
            PredictionScalarWireV1::FiniteNumber { value } => Ok(Self::FiniteNumber { value }),
            PredictionScalarWireV1::Token { value } => Self::token(value),
            PredictionScalarWireV1::Text { value } => Self::text(value),
        }
    }
}

impl<'de> Deserialize<'de> for PredictionScalarV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_from(PredictionScalarWireV1::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

impl PredictionScalarV1 {
    /// Construct a finite-number scalar.
    pub fn finite_number(value: f64) -> Result<Self, PredictionContractError> {
        Ok(Self::FiniteNumber {
            value: FinitePredictionNumberV1::new(value)?,
        })
    }

    /// Construct a bounded nonempty control-free token scalar.
    pub fn token(value: impl Into<String>) -> Result<Self, PredictionContractError> {
        Ok(Self::Token {
            value: stable_token("scalar token", value)?,
        })
    }

    /// Construct a bounded text scalar.
    pub fn text(value: impl Into<String>) -> Result<Self, PredictionContractError> {
        Ok(Self::Text {
            value: bounded_string("scalar text", value)?,
        })
    }

    fn retained_text_bytes(&self) -> usize {
        match self {
            Self::Token { value } | Self::Text { value } => value.len(),
            _ => 0,
        }
    }
}

/// Exact resolved-setting location retained in a basis reference.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(tag = "scope", rename_all = "snake_case")]
pub enum ResolvedSettingLocationV1 {
    /// Fully materialized document setting.
    Document,
    /// Fully materialized setting on one actual clip row.
    Clip {
        /// Zero-based ordinal in lexical actual-name order.
        clip_ordinal: u64,
        /// Exact actual clip name, including duplicate-name rows.
        clip_name: String,
    },
}

#[derive(Deserialize)]
#[serde(tag = "scope", rename_all = "snake_case", deny_unknown_fields)]
enum ResolvedSettingLocationWireV1 {
    Document,
    Clip {
        clip_ordinal: u64,
        clip_name: String,
    },
}

impl TryFrom<ResolvedSettingLocationWireV1> for ResolvedSettingLocationV1 {
    type Error = PredictionContractError;

    fn try_from(wire: ResolvedSettingLocationWireV1) -> Result<Self, Self::Error> {
        match wire {
            ResolvedSettingLocationWireV1::Document => Ok(Self::Document),
            ResolvedSettingLocationWireV1::Clip {
                clip_ordinal,
                clip_name,
            } => Self::clip(clip_ordinal, clip_name),
        }
    }
}

impl<'de> Deserialize<'de> for ResolvedSettingLocationV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_from(ResolvedSettingLocationWireV1::deserialize(deserializer)?)
            .map_err(D::Error::custom)
    }
}

impl ResolvedSettingLocationV1 {
    /// Construct one bounded clip-row location.
    pub fn clip(
        clip_ordinal: u64,
        clip_name: impl Into<String>,
    ) -> Result<Self, PredictionContractError> {
        Ok(Self::Clip {
            clip_ordinal,
            clip_name: bounded_string("clip name", clip_name)?,
        })
    }

    fn retained_text_bytes(&self) -> usize {
        match self {
            Self::Document => 0,
            Self::Clip { clip_name, .. } => clip_name.len(),
        }
    }
}

/// Canonical RFC 6901 path rooted at one file's `measurements` member.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct MeasurementPointerV1(String);

impl MeasurementPointerV1 {
    /// Validate one canonical measurements-root pointer.
    pub fn new(pointer: impl Into<String>) -> Result<Self, PredictionContractError> {
        let pointer = bounded_string("measurement pointer", pointer)?;
        let Some(rest) = pointer.strip_prefix("/measurements") else {
            return Err(PredictionContractError::InvalidMeasurementPointer(pointer));
        };
        if !rest.is_empty() && !rest.starts_with('/') {
            return Err(PredictionContractError::InvalidMeasurementPointer(pointer));
        }
        let components = 1usize.saturating_add(if rest.is_empty() {
            0
        } else {
            rest[1..].split('/').count()
        });
        if components > PREDICTION_V1_MAX_MEASUREMENT_POINTER_COMPONENTS {
            return Err(
                PredictionContractError::TooManyMeasurementPointerComponents {
                    components,
                    limit: PREDICTION_V1_MAX_MEASUREMENT_POINTER_COMPONENTS,
                },
            );
        }
        if rest
            .strip_prefix('/')
            .into_iter()
            .flat_map(|value| value.split('/'))
            .any(|component| !canonical_pointer_component(component))
        {
            return Err(PredictionContractError::InvalidMeasurementPointer(pointer));
        }
        Ok(Self(pointer))
    }

    /// Canonical pointer spelling.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for MeasurementPointerV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(String::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

fn canonical_pointer_component(component: &str) -> bool {
    let bytes = component.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] != b'~' {
            index += 1;
            continue;
        }
        if !matches!(bytes.get(index + 1), Some(b'0' | b'1')) {
            return false;
        }
        index += 2;
    }
    true
}

/// Closed raw-source evidence domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RawSourceDomainV1 {
    /// File-level source linear-unit observation.
    LinearUnit,
    /// File-level source coordinate-basis observation.
    CoordinateBasis,
    /// File-level source frame-rate observation.
    FramesPerSecond,
    /// One source clip/take row.
    Clip,
    /// One source channel row nested in a source clip.
    Channel,
    /// One source construct row.
    Construct,
    /// One source resource row.
    Resource,
    /// One source-skeleton node row.
    SourceNode,
    /// One source-skeleton skin row.
    SourceSkin,
}

/// Source-skeleton row kind used by the stable raw-source key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceSkeletonRowKindV1 {
    /// Source node row.
    SourceNode,
    /// Source skin row.
    SourceSkin,
}

/// Stable raw-source row identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RawSourceKeyV1 {
    /// File-level scalar observation; no row index exists.
    Scalar,
    /// Source clip/take row.
    Clip {
        /// Stable source clip index.
        source_clip_index: u64,
    },
    /// Source channel row nested in a clip.
    Channel {
        /// Stable source clip index.
        source_clip_index: u64,
        /// Stable channel index inside that source clip.
        source_channel_index: u64,
    },
    /// Source construct row.
    Construct {
        /// Stable source-order index.
        source_order_index: u64,
    },
    /// Source resource declaration row.
    Resource {
        /// Stable source-order index.
        source_order_index: u64,
        /// Stable parser/source declaration index.
        source_index: u64,
    },
    /// Source node or skin row.
    SourceSkeleton {
        /// Node-versus-skin row domain.
        row_kind: SourceSkeletonRowKindV1,
        /// Stable source node/skin index.
        source_index: u64,
    },
}

/// Bounded exact scalar field inside one raw-source row.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RawSourceFieldIdV1(String);

impl RawSourceFieldIdV1 {
    /// Construct a bounded stable dot-separated field id.
    pub fn new(field: impl Into<String>) -> Result<Self, PredictionContractError> {
        let field = stable_token("raw-source field", field)?;
        if !field.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'.' | b'-')
        }) || !field.as_bytes()[0].is_ascii_lowercase()
        {
            return Err(PredictionContractError::InvalidToken {
                field: "raw-source field",
                value: field,
            });
        }
        Ok(Self(field))
    }

    /// Stable field spelling.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for RawSourceFieldIdV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(String::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

/// One raw-source scalar retained directly in a prediction basis.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct RawSourceBasisReferenceV1 {
    domain: RawSourceDomainV1,
    key: RawSourceKeyV1,
    field: RawSourceFieldIdV1,
    value: PredictionScalarV1,
}

impl<'de> Deserialize<'de> for RawSourceBasisReferenceV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct WireReference {
            domain: RawSourceDomainV1,
            key: RawSourceKeyV1,
            field: RawSourceFieldIdV1,
            value: PredictionScalarV1,
        }
        let wire = WireReference::deserialize(deserializer)?;
        Self::from_wire(wire.domain, wire.key, wire.field, wire.value).map_err(D::Error::custom)
    }
}

impl RawSourceBasisReferenceV1 {
    /// Construct one raw-source reference from the authoritative same-load view.
    pub fn from_source(
        domain: RawSourceDomainV1,
        key: RawSourceKeyV1,
        field: RawSourceFieldIdV1,
        facts: SourceFactsViewV1<'_>,
    ) -> Result<Self, PredictionContractError> {
        let mut reference = Self::from_wire(domain, key, field, PredictionScalarV1::Null)?;
        reference.value = raw_source_scalar(&reference, facts)?;
        Ok(reference)
    }

    pub(crate) fn from_wire(
        domain: RawSourceDomainV1,
        key: RawSourceKeyV1,
        field: RawSourceFieldIdV1,
        value: PredictionScalarV1,
    ) -> Result<Self, PredictionContractError> {
        if !raw_domain_matches_key(domain, &key) {
            return Err(PredictionContractError::RawSourceDomainKeyMismatch);
        }
        Ok(Self {
            domain,
            key,
            field,
            value,
        })
    }

    /// Validate this exact scalar against the same-load source-facts view.
    pub fn validate_against(
        &self,
        facts: SourceFactsViewV1<'_>,
    ) -> Result<(), PredictionContractError> {
        validate_raw_source_reference(self, facts)
    }

    /// Raw-source evidence domain.
    pub const fn domain(&self) -> RawSourceDomainV1 {
        self.domain
    }

    /// Stable raw-source row key.
    pub const fn key(&self) -> &RawSourceKeyV1 {
        &self.key
    }

    /// Exact scalar field.
    pub const fn field(&self) -> &RawSourceFieldIdV1 {
        &self.field
    }

    /// Exact scalar value.
    pub const fn value(&self) -> &PredictionScalarV1 {
        &self.value
    }

    #[allow(
        dead_code,
        reason = "V1 standalone prediction remains an explicit historical API"
    )]
    fn retained_text_bytes(&self) -> Result<usize, PredictionContractError> {
        checked_sum(
            "raw-source basis retained text",
            [self.field.0.len(), self.value.retained_text_bytes()],
        )
    }
}

fn raw_domain_matches_key(domain: RawSourceDomainV1, key: &RawSourceKeyV1) -> bool {
    matches!(
        (domain, key),
        (
            RawSourceDomainV1::LinearUnit
                | RawSourceDomainV1::CoordinateBasis
                | RawSourceDomainV1::FramesPerSecond,
            RawSourceKeyV1::Scalar
        ) | (RawSourceDomainV1::Clip, RawSourceKeyV1::Clip { .. })
            | (RawSourceDomainV1::Channel, RawSourceKeyV1::Channel { .. })
            | (
                RawSourceDomainV1::Construct,
                RawSourceKeyV1::Construct { .. }
            )
            | (RawSourceDomainV1::Resource, RawSourceKeyV1::Resource { .. })
            | (
                RawSourceDomainV1::SourceNode,
                RawSourceKeyV1::SourceSkeleton {
                    row_kind: SourceSkeletonRowKindV1::SourceNode,
                    ..
                }
            )
            | (
                RawSourceDomainV1::SourceSkin,
                RawSourceKeyV1::SourceSkeleton {
                    row_kind: SourceSkeletonRowKindV1::SourceSkin,
                    ..
                }
            )
    )
}

fn validate_raw_source_reference(
    reference: &RawSourceBasisReferenceV1,
    facts: SourceFactsViewV1<'_>,
) -> Result<(), PredictionContractError> {
    let actual = raw_source_scalar(reference, facts)?;
    if actual != reference.value {
        return Err(PredictionContractError::RawSourceValueMismatch);
    }
    Ok(())
}

fn raw_source_scalar(
    reference: &RawSourceBasisReferenceV1,
    facts: SourceFactsViewV1<'_>,
) -> Result<PredictionScalarV1, PredictionContractError> {
    let field = reference.field.as_str();
    match (&reference.key, reference.domain) {
        (RawSourceKeyV1::Scalar, RawSourceDomainV1::LinearUnit) => {
            scalar_observation_value(facts.linear_unit(), field, |value| {
                PredictionScalarV1::finite_number(value.meters_per_source_unit())
            })
        }
        (RawSourceKeyV1::Scalar, RawSourceDomainV1::FramesPerSecond) => {
            scalar_observation_value(facts.frames_per_second(), field, |value| {
                PredictionScalarV1::finite_number(value.get())
            })
        }
        (RawSourceKeyV1::Scalar, RawSourceDomainV1::CoordinateBasis) => {
            coordinate_observation_value(facts.coordinate_basis(), field)
        }
        (RawSourceKeyV1::Clip { source_clip_index }, RawSourceDomainV1::Clip) => {
            let row = facts
                .clips()
                .rows()
                .iter()
                .find(|row| u64::try_from(row.source_clip_index()).ok() == Some(*source_clip_index))
                .ok_or(PredictionContractError::RawSourceRowNotFound)?;
            if let Some(value) = observation_metadata(row.source_name(), field)? {
                return Ok(value);
            }
            match field {
                "source_name.value" => observation_value(row.source_name(), |value| {
                    Ok(PredictionScalarV1::Text {
                        value: value.as_str().to_owned(),
                    })
                }),
                "normalized_clip_index.value" => {
                    observation_value(row.normalized_clip_index(), |value| {
                        Ok(PredictionScalarV1::UnsignedInteger {
                            value: *value as u64,
                        })
                    })
                }
                "source_range.begin_s" => observation_value(row.source_range(), |value| {
                    PredictionScalarV1::finite_number(value.begin_s())
                }),
                "source_range.end_s" => observation_value(row.source_range(), |value| {
                    PredictionScalarV1::finite_number(value.end_s())
                }),
                "sampler_range.begin_s" => observation_value(row.sampler_range(), |value| {
                    PredictionScalarV1::finite_number(value.begin_s())
                }),
                "sampler_range.end_s" => observation_value(row.sampler_range(), |value| {
                    PredictionScalarV1::finite_number(value.end_s())
                }),
                "channels.coverage.state" => Ok(token_scalar(source_coverage_state_name(
                    row.channels().coverage().state(),
                ))),
                "channels.coverage.reason" => Ok(optional_token_scalar(
                    row.channels()
                        .coverage()
                        .reason()
                        .map(source_unavailable_reason_name),
                )),
                _ => Err(PredictionContractError::RawSourceFieldUnavailable(
                    field.to_owned(),
                )),
            }
        }
        (
            RawSourceKeyV1::Channel {
                source_clip_index,
                source_channel_index,
            },
            RawSourceDomainV1::Channel,
        ) => {
            let clip = facts
                .clips()
                .rows()
                .iter()
                .find(|row| u64::try_from(row.source_clip_index()).ok() == Some(*source_clip_index))
                .ok_or(PredictionContractError::RawSourceRowNotFound)?;
            let row = clip
                .channels()
                .rows()
                .iter()
                .find(|row| {
                    u64::try_from(row.source_channel_index()).ok() == Some(*source_channel_index)
                })
                .ok_or(PredictionContractError::RawSourceRowNotFound)?;
            match field {
                "source_layer_index" => Ok(optional_unsigned_scalar(
                    row.source_layer_index().map(|value| value as u64),
                )),
                "target.kind" => Ok(token_scalar(source_target_kind_name(row.target().kind()))),
                "target.index" => Ok(PredictionScalarV1::UnsignedInteger {
                    value: row.target().index(),
                }),
                "property" => Ok(token_scalar(source_channel_property_name(row.property()))),
                "property_name" => Ok(optional_text_scalar(
                    row.property_name().map(|value| value.as_str()),
                )),
                "components.x" => Ok(PredictionScalarV1::Boolean {
                    value: row.components().x(),
                }),
                "components.y" => Ok(PredictionScalarV1::Boolean {
                    value: row.components().y(),
                }),
                "components.z" => Ok(PredictionScalarV1::Boolean {
                    value: row.components().z(),
                }),
                "interpolation.state" => Ok(token_scalar(observation_state_name(
                    row.interpolation().state(),
                ))),
                "interpolation.value" => observation_value(row.interpolation(), |value| {
                    Ok(token_scalar(source_interpolation_name(*value)))
                }),
                "input_accessor_index" => Ok(optional_unsigned_scalar(
                    row.input_accessor_index().map(|value| value as u64),
                )),
                "output_accessor_index" => Ok(optional_unsigned_scalar(
                    row.output_accessor_index().map(|value| value as u64),
                )),
                "disposition" => Ok(token_scalar(source_disposition_name(row.disposition()))),
                "provenance.kind" => Ok(token_scalar(source_provenance_kind_name(
                    row.provenance().kind(),
                ))),
                "provenance.locator" => Ok(optional_text_scalar(
                    row.provenance().locator().map(|value| value.as_str()),
                )),
                _ => Err(PredictionContractError::RawSourceFieldUnavailable(
                    field.to_owned(),
                )),
            }
        }
        (RawSourceKeyV1::Construct { source_order_index }, RawSourceDomainV1::Construct) => {
            let row = facts
                .constructs()
                .rows()
                .iter()
                .find(|row| {
                    u64::try_from(row.source_order_index()).ok() == Some(*source_order_index)
                })
                .ok_or(PredictionContractError::RawSourceRowNotFound)?;
            match field {
                "kind" => Ok(token_scalar(source_construct_kind_name(row.kind()))),
                "name" => Ok(text_scalar(row.name().as_str())),
                "required" => Ok(PredictionScalarV1::Boolean {
                    value: row.required(),
                }),
                "count" => Ok(PredictionScalarV1::UnsignedInteger { value: row.count() }),
                "disposition" => Ok(token_scalar(source_disposition_name(row.disposition()))),
                "provenance.kind" => Ok(token_scalar(source_provenance_kind_name(
                    row.provenance().kind(),
                ))),
                "provenance.locator" => Ok(optional_text_scalar(
                    row.provenance().locator().map(|value| value.as_str()),
                )),
                _ => Err(PredictionContractError::RawSourceFieldUnavailable(
                    field.to_owned(),
                )),
            }
        }
        (
            RawSourceKeyV1::Resource {
                source_order_index,
                source_index,
            },
            RawSourceDomainV1::Resource,
        ) => {
            let row = facts
                .resources()
                .rows()
                .iter()
                .find(|row| {
                    u64::try_from(row.source_order_index()).ok() == Some(*source_order_index)
                        && row.source_index() == *source_index
                })
                .ok_or(PredictionContractError::RawSourceRowNotFound)?;
            match field {
                "kind" => Ok(token_scalar(source_resource_kind_name(row.kind()))),
                "source_index" => Ok(PredictionScalarV1::UnsignedInteger {
                    value: row.source_index(),
                }),
                "locator.kind" => Ok(token_scalar(source_locator_kind_name(row.locator()))),
                "locator.value" => Ok(match row.locator() {
                    SourceResourceLocatorV1::Relative(value) => text_scalar(value.as_str()),
                    _ => PredictionScalarV1::Null,
                }),
                "disposition" => Ok(token_scalar(source_disposition_name(row.disposition()))),
                "provenance.kind" => Ok(token_scalar(source_provenance_kind_name(
                    row.provenance().kind(),
                ))),
                "provenance.locator" => Ok(optional_text_scalar(
                    row.provenance().locator().map(|value| value.as_str()),
                )),
                _ => Err(PredictionContractError::RawSourceFieldUnavailable(
                    field.to_owned(),
                )),
            }
        }
        (
            RawSourceKeyV1::SourceSkeleton {
                row_kind: SourceSkeletonRowKindV1::SourceNode,
                source_index,
            },
            RawSourceDomainV1::SourceNode,
        ) => {
            let row = facts
                .source_skeleton()
                .nodes
                .iter()
                .find(|row| u64::try_from(row.source_node_index).ok() == Some(*source_index))
                .ok_or(PredictionContractError::RawSourceRowNotFound)?;
            match field {
                "name" => Ok(optional_text_scalar(row.name.as_deref())),
                "parent_source_node_index" => Ok(optional_unsigned_scalar(
                    row.parent_source_node_index.map(|value| value as u64),
                )),
                "bone" => Ok(optional_unsigned_scalar(row.bone.map(|value| value as u64))),
                "local_rest.kind" => Ok(token_scalar(match row.local_rest {
                    SourceNodeLocalRest::Trs { .. } => "trs",
                    SourceNodeLocalRest::Matrix(_) => "matrix",
                })),
                _ => source_node_local_rest_scalar(&row.local_rest, field),
            }
        }
        (
            RawSourceKeyV1::SourceSkeleton {
                row_kind: SourceSkeletonRowKindV1::SourceSkin,
                source_index,
            },
            RawSourceDomainV1::SourceSkin,
        ) => {
            let row = facts
                .source_skeleton()
                .skins
                .iter()
                .find(|row| u64::try_from(row.source_skin_index).ok() == Some(*source_index))
                .ok_or(PredictionContractError::RawSourceRowNotFound)?;
            match field {
                "name" => Ok(optional_text_scalar(row.name.as_deref())),
                "skeleton_root_source_node_index" => Ok(optional_unsigned_scalar(
                    row.skeleton_root_source_node_index
                        .map(|value| value as u64),
                )),
                "joint_count" => Ok(PredictionScalarV1::UnsignedInteger {
                    value: row.joint_source_node_indices.len() as u64,
                }),
                "inverse_bind.status" => Ok(token_scalar(inverse_bind_status_name(
                    row.inverse_bind_accessor.status,
                ))),
                "inverse_bind.declared_count" => Ok(optional_unsigned_scalar(
                    row.inverse_bind_accessor
                        .declared_count
                        .map(|value| value as u64),
                )),
                "attachment_count" => Ok(PredictionScalarV1::UnsignedInteger {
                    value: row.attachments.len() as u64,
                }),
                _ => Err(PredictionContractError::RawSourceFieldUnavailable(
                    field.to_owned(),
                )),
            }
        }
        _ => Err(PredictionContractError::RawSourceDomainKeyMismatch),
    }
}

fn scalar_observation_value<T>(
    observation: &SourceObservationV1<T>,
    field: &str,
    value: impl FnOnce(&T) -> Result<PredictionScalarV1, PredictionContractError>,
) -> Result<PredictionScalarV1, PredictionContractError> {
    if let Some(metadata) = observation_metadata(observation, field)? {
        return Ok(metadata);
    }
    if field == "value" {
        return observation_value(observation, value);
    }
    Err(PredictionContractError::RawSourceFieldUnavailable(
        field.to_owned(),
    ))
}

fn coordinate_observation_value(
    observation: &SourceObservationV1<SourceCoordinateBasisV1>,
    field: &str,
) -> Result<PredictionScalarV1, PredictionContractError> {
    if let Some(metadata) = observation_metadata(observation, field)? {
        return Ok(metadata);
    }
    observation_value(observation, |basis| {
        Ok(token_scalar(match field {
            "right" => source_axis_name(basis.right()),
            "up" => source_axis_name(basis.up()),
            "forward" => source_axis_name(basis.forward()),
            "handedness" => match basis.handedness() {
                crate::source_facts::SourceHandednessV1::Right => "right",
                crate::source_facts::SourceHandednessV1::Left => "left",
            },
            _ => {
                return Err(PredictionContractError::RawSourceFieldUnavailable(
                    field.to_owned(),
                ));
            }
        }))
    })
}

fn observation_metadata<T>(
    observation: &SourceObservationV1<T>,
    field: &str,
) -> Result<Option<PredictionScalarV1>, PredictionContractError> {
    let value = match field {
        "state" | "source_name.state" => {
            Some(token_scalar(observation_state_name(observation.state())))
        }
        "unavailable_reason" => Some(optional_token_scalar(match observation.state() {
            SourceObservationStateV1::Unavailable(reason) => {
                Some(source_unavailable_reason_name(*reason))
            }
            _ => None,
        })),
        "disposition" => Some(token_scalar(source_disposition_name(
            observation.disposition(),
        ))),
        "provenance.kind" => Some(optional_token_scalar(
            observation
                .provenance()
                .map(|value| source_provenance_kind_name(value.kind())),
        )),
        "provenance.locator" => Some(optional_text_scalar(
            observation
                .provenance()
                .and_then(SourceProvenanceV1::locator)
                .map(|value| value.as_str()),
        )),
        _ => None,
    };
    Ok(value)
}

fn observation_value<T>(
    observation: &SourceObservationV1<T>,
    value: impl FnOnce(&T) -> Result<PredictionScalarV1, PredictionContractError>,
) -> Result<PredictionScalarV1, PredictionContractError> {
    match observation.state() {
        SourceObservationStateV1::Observed(observed) => value(observed),
        SourceObservationStateV1::ProvenAbsent | SourceObservationStateV1::Unavailable(_) => {
            Ok(PredictionScalarV1::Null)
        }
    }
}

fn source_node_local_rest_scalar(
    rest: &SourceNodeLocalRest,
    field: &str,
) -> Result<PredictionScalarV1, PredictionContractError> {
    let value = match rest {
        SourceNodeLocalRest::Trs {
            translation,
            rotation,
            scale,
        } => match field {
            "local_rest.translation.x" => translation.x,
            "local_rest.translation.y" => translation.y,
            "local_rest.translation.z" => translation.z,
            "local_rest.rotation.x" => rotation.x,
            "local_rest.rotation.y" => rotation.y,
            "local_rest.rotation.z" => rotation.z,
            "local_rest.rotation.w" => rotation.w,
            "local_rest.scale.x" => scale.x,
            "local_rest.scale.y" => scale.y,
            "local_rest.scale.z" => scale.z,
            _ => {
                return Err(PredictionContractError::RawSourceFieldUnavailable(
                    field.to_owned(),
                ));
            }
        },
        SourceNodeLocalRest::Matrix(matrix) => {
            let Some(component) = field.strip_prefix("local_rest.matrix.") else {
                return Err(PredictionContractError::RawSourceFieldUnavailable(
                    field.to_owned(),
                ));
            };
            let index = component
                .parse::<usize>()
                .ok()
                .filter(|index| *index < 16)
                .ok_or_else(|| {
                    PredictionContractError::RawSourceFieldUnavailable(field.to_owned())
                })?;
            matrix.to_cols_array()[index]
        }
    };
    PredictionScalarV1::finite_number(f64::from(value))
}

fn token_scalar(value: &str) -> PredictionScalarV1 {
    PredictionScalarV1::Token {
        value: value.to_owned(),
    }
}

fn text_scalar(value: &str) -> PredictionScalarV1 {
    PredictionScalarV1::Text {
        value: value.to_owned(),
    }
}

fn optional_text_scalar(value: Option<&str>) -> PredictionScalarV1 {
    value.map_or(PredictionScalarV1::Null, text_scalar)
}

fn optional_token_scalar(value: Option<&str>) -> PredictionScalarV1 {
    value.map_or(PredictionScalarV1::Null, token_scalar)
}

fn optional_unsigned_scalar(value: Option<u64>) -> PredictionScalarV1 {
    value.map_or(PredictionScalarV1::Null, |value| {
        PredictionScalarV1::UnsignedInteger { value }
    })
}

fn source_format_name(value: SourceFormatV1) -> &'static str {
    match value {
        SourceFormatV1::GltfJson => "gltf_json",
        SourceFormatV1::Glb => "glb",
        SourceFormatV1::Fbx => "fbx",
    }
}

fn source_unavailable_reason_name(value: SourceUnavailableReasonV1) -> &'static str {
    match value {
        SourceUnavailableReasonV1::Malformed => "malformed",
        SourceUnavailableReasonV1::Discarded => "discarded",
        SourceUnavailableReasonV1::NormalizedAway => "normalized_away",
        SourceUnavailableReasonV1::BakedAway => "baked_away",
        SourceUnavailableReasonV1::LoaderUnsupported => "loader_unsupported",
        SourceUnavailableReasonV1::ProjectionBudgetExceeded => "projection_budget_exceeded",
        SourceUnavailableReasonV1::ParserUnavailable => "parser_unavailable",
    }
}

fn source_coverage_state_name(value: SourceSetCoverageStateV1) -> &'static str {
    match value {
        SourceSetCoverageStateV1::Complete => "complete",
        SourceSetCoverageStateV1::Partial => "partial",
        SourceSetCoverageStateV1::Unavailable => "unavailable",
    }
}

fn source_disposition_name(value: SourceLoaderDispositionV1) -> &'static str {
    match value {
        SourceLoaderDispositionV1::Preserved => "preserved",
        SourceLoaderDispositionV1::Normalized => "normalized",
        SourceLoaderDispositionV1::Baked => "baked",
        SourceLoaderDispositionV1::Discarded => "discarded",
        SourceLoaderDispositionV1::Unsupported => "unsupported",
        SourceLoaderDispositionV1::Unknown => "unknown",
        SourceLoaderDispositionV1::NotApplicable => "not_applicable",
    }
}

fn source_provenance_kind_name(value: SourceProvenanceKindV1) -> &'static str {
    match value {
        SourceProvenanceKindV1::FormatDefined => "format_defined",
        SourceProvenanceKindV1::SourceDeclared => "source_declared",
        SourceProvenanceKindV1::ParserProjected => "parser_projected",
        SourceProvenanceKindV1::DerivedFromSource => "derived_from_source",
    }
}

fn source_axis_name(value: SourceAxisV1) -> &'static str {
    match value {
        SourceAxisV1::PositiveX => "positive_x",
        SourceAxisV1::NegativeX => "negative_x",
        SourceAxisV1::PositiveY => "positive_y",
        SourceAxisV1::NegativeY => "negative_y",
        SourceAxisV1::PositiveZ => "positive_z",
        SourceAxisV1::NegativeZ => "negative_z",
    }
}

fn observation_state_name<T>(value: &SourceObservationStateV1<T>) -> &'static str {
    match value {
        SourceObservationStateV1::Observed(_) => "observed",
        SourceObservationStateV1::ProvenAbsent => "proven_absent",
        SourceObservationStateV1::Unavailable(_) => "unavailable",
    }
}

fn source_target_kind_name(value: SourceTargetKindV1) -> &'static str {
    match value {
        SourceTargetKindV1::Node => "node",
        SourceTargetKindV1::Element => "element",
        SourceTargetKindV1::Other => "other",
    }
}

fn source_channel_property_name(value: SourceChannelPropertyV1) -> &'static str {
    match value {
        SourceChannelPropertyV1::Translation => "translation",
        SourceChannelPropertyV1::Rotation => "rotation",
        SourceChannelPropertyV1::Scale => "scale",
        SourceChannelPropertyV1::Weights => "weights",
        SourceChannelPropertyV1::Other => "other",
    }
}

fn source_interpolation_name(value: SourceInterpolationV1) -> &'static str {
    match value {
        SourceInterpolationV1::Step => "step",
        SourceInterpolationV1::Linear => "linear",
        SourceInterpolationV1::CubicSpline => "cubic_spline",
        SourceInterpolationV1::Other => "other",
    }
}

fn source_construct_kind_name(value: SourceConstructKindV1) -> &'static str {
    match value {
        SourceConstructKindV1::Extension => "extension",
        SourceConstructKindV1::CustomProperty => "custom_property",
        SourceConstructKindV1::UnknownElement => "unknown_element",
    }
}

fn source_resource_kind_name(value: SourceResourceKindV1) -> &'static str {
    match value {
        SourceResourceKindV1::Buffer => "buffer",
        SourceResourceKindV1::Image => "image",
        SourceResourceKindV1::Texture => "texture",
        SourceResourceKindV1::Video => "video",
        SourceResourceKindV1::Cache => "cache",
    }
}

fn source_locator_kind_name(value: &SourceResourceLocatorV1) -> &'static str {
    match value {
        SourceResourceLocatorV1::Embedded => "embedded",
        SourceResourceLocatorV1::DataUri => "data_uri",
        SourceResourceLocatorV1::Relative(_) => "relative",
        SourceResourceLocatorV1::Absolute => "absolute",
        SourceResourceLocatorV1::Escaping => "escaping",
        SourceResourceLocatorV1::Remote => "remote",
        SourceResourceLocatorV1::Malformed => "malformed",
        SourceResourceLocatorV1::Oversized => "oversized",
        SourceResourceLocatorV1::Missing => "missing",
    }
}

fn inverse_bind_status_name(value: SourceInverseBindAccessorStatus) -> &'static str {
    match value {
        SourceInverseBindAccessorStatus::Absent => "absent",
        SourceInverseBindAccessorStatus::Available => "available",
        SourceInverseBindAccessorStatus::EmptyAccessor => "empty_accessor",
        SourceInverseBindAccessorStatus::CountMismatch => "count_mismatch",
        SourceInverseBindAccessorStatus::Unreadable => "unreadable",
    }
}

/// One typed reference in a prediction basis.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PredictionBasisReferenceV1 {
    /// Exact profile-fact id resolved against the embedded profile record.
    ProfileFact {
        /// Stable fact id.
        fact_id: String,
    },
    /// Fully materialized engine setting.
    ResolvedSetting {
        /// Exact document or duplicate-safe clip-row location.
        location: ResolvedSettingLocationV1,
        /// Stable setting id.
        setting_id: String,
    },
    /// Stable project/config field and exact scalar value.
    ProjectField {
        /// Stable project-field id.
        field_id: String,
        /// Exact resolved value.
        value: PredictionScalarV1,
    },
    /// Same-load raw-source scalar evidence.
    RawSource {
        /// Closed row/field/value reference.
        #[serde(flatten)]
        reference: RawSourceBasisReferenceV1,
    },
    /// Validated scalar in the same file's measurements-v15 contract.
    Measurement {
        /// Immutable measurements contract identity.
        schema: &'static str,
        /// Canonical measurements-root pointer.
        pointer: MeasurementPointerV1,
        /// Exact scalar found at the pointer.
        value: PredictionScalarV1,
    },
    /// Exact primary-source id in the embedded profile record.
    PrimarySource {
        /// Stable source id.
        source_id: String,
    },
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum PredictionBasisReferenceWireV1 {
    ProfileFact {
        fact_id: String,
    },
    ResolvedSetting {
        location: ResolvedSettingLocationWireV1,
        setting_id: String,
    },
    ProjectField {
        field_id: String,
        value: PredictionScalarWireV1,
    },
    RawSource {
        domain: RawSourceDomainV1,
        key: RawSourceKeyV1,
        field: String,
        value: PredictionScalarWireV1,
    },
    Measurement {
        schema: String,
        pointer: String,
        value: PredictionScalarWireV1,
    },
    PrimarySource {
        source_id: String,
    },
}

impl TryFrom<PredictionBasisReferenceWireV1> for PredictionBasisReferenceV1 {
    type Error = PredictionContractError;

    fn try_from(wire: PredictionBasisReferenceWireV1) -> Result<Self, Self::Error> {
        match wire {
            PredictionBasisReferenceWireV1::ProfileFact { fact_id } => Self::profile_fact(fact_id),
            PredictionBasisReferenceWireV1::ResolvedSetting {
                location,
                setting_id,
            } => Self::resolved_setting(location.try_into()?, setting_id),
            PredictionBasisReferenceWireV1::ProjectField { field_id, value } => {
                Self::project_field(field_id, value.try_into()?)
            }
            PredictionBasisReferenceWireV1::RawSource {
                domain,
                key,
                field,
                value,
            } => RawSourceBasisReferenceV1::from_wire(
                domain,
                key,
                RawSourceFieldIdV1::new(field)?,
                value.try_into()?,
            )
            .map(Self::raw_source),
            PredictionBasisReferenceWireV1::Measurement {
                schema,
                pointer,
                value,
            } => {
                if schema != MEASUREMENTS_SCHEMA_ID {
                    return Err(PredictionContractError::InvalidSchema {
                        field: "basis.measurement.schema",
                        expected: MEASUREMENTS_SCHEMA_ID,
                        found: schema,
                    });
                }
                Ok(Self::measurement(
                    MeasurementPointerV1::new(pointer)?,
                    value.try_into()?,
                ))
            }
            PredictionBasisReferenceWireV1::PrimarySource { source_id } => {
                Self::primary_source(source_id)
            }
        }
    }
}

impl<'de> Deserialize<'de> for PredictionBasisReferenceV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_from(PredictionBasisReferenceWireV1::deserialize(deserializer)?)
            .map_err(D::Error::custom)
    }
}

impl PredictionBasisReferenceV1 {
    /// Construct a profile-fact reference.
    pub fn profile_fact(fact_id: impl Into<String>) -> Result<Self, PredictionContractError> {
        Ok(Self::ProfileFact {
            fact_id: stable_token("profile fact id", fact_id)?,
        })
    }

    /// Construct a fully materialized setting reference.
    pub fn resolved_setting(
        location: ResolvedSettingLocationV1,
        setting_id: impl Into<String>,
    ) -> Result<Self, PredictionContractError> {
        Ok(Self::ResolvedSetting {
            location,
            setting_id: stable_token("setting id", setting_id)?,
        })
    }

    /// Construct a project/config field reference.
    pub fn project_field(
        field_id: impl Into<String>,
        value: PredictionScalarV1,
    ) -> Result<Self, PredictionContractError> {
        Ok(Self::ProjectField {
            field_id: stable_bounded_id("project field id", field_id)?,
            value,
        })
    }

    /// Construct a raw-source reference already validated against its same-load view.
    pub fn raw_source(reference: RawSourceBasisReferenceV1) -> Self {
        Self::RawSource { reference }
    }

    /// Construct a measurements-v15 scalar reference.
    pub fn measurement(pointer: MeasurementPointerV1, value: PredictionScalarV1) -> Self {
        Self::Measurement {
            schema: MEASUREMENTS_SCHEMA_ID,
            pointer,
            value,
        }
    }

    /// Construct an embedded primary-source reference.
    pub fn primary_source(source_id: impl Into<String>) -> Result<Self, PredictionContractError> {
        Ok(Self::PrimarySource {
            source_id: stable_token("primary source id", source_id)?,
        })
    }

    fn retained_text_bytes(&self) -> Result<usize, PredictionContractError> {
        match self {
            Self::ProfileFact { fact_id } => Ok(fact_id.len()),
            Self::ResolvedSetting {
                location,
                setting_id,
            } => checked_sum(
                "resolved-setting basis retained text",
                [location.retained_text_bytes(), setting_id.len()],
            ),
            Self::ProjectField { field_id, value } => checked_sum(
                "project-field basis retained text",
                [field_id.len(), value.retained_text_bytes()],
            ),
            Self::RawSource { reference } => reference.retained_text_bytes(),
            Self::Measurement { pointer, value, .. } => checked_sum(
                "measurement basis retained text",
                [pointer.0.len(), value.retained_text_bytes()],
            ),
            Self::PrimarySource { source_id } => Ok(source_id.len()),
        }
    }
}

/// Raw-source unavailability reason retained by the output wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RawSourceUnavailableReasonV1 {
    /// Malformed declaration.
    Malformed,
    /// Loader discarded the value.
    Discarded,
    /// Coordinate/transform normalization removed the original form.
    NormalizedAway,
    /// Baking removed the original form.
    BakedAway,
    /// Loader does not model the domain.
    LoaderUnsupported,
    /// V1 projection budget was exhausted.
    ProjectionBudgetExceeded,
    /// Parser did not expose the evidence.
    ParserUnavailable,
}

impl From<SourceUnavailableReasonV1> for RawSourceUnavailableReasonV1 {
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

/// Loader treatment retained with a raw-source observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RawSourceDispositionV1 {
    /// Preserved without reinterpretation.
    Preserved,
    /// Normalized into AnimSmith's model domain.
    Normalized,
    /// Evaluated into baked samples.
    Baked,
    /// Deliberately discarded.
    Discarded,
    /// Recognized but unsupported.
    Unsupported,
    /// Loader treatment is unknown.
    Unknown,
    /// Domain does not apply.
    NotApplicable,
}

impl From<SourceLoaderDispositionV1> for RawSourceDispositionV1 {
    fn from(value: SourceLoaderDispositionV1) -> Self {
        match value {
            SourceLoaderDispositionV1::Preserved => Self::Preserved,
            SourceLoaderDispositionV1::Normalized => Self::Normalized,
            SourceLoaderDispositionV1::Baked => Self::Baked,
            SourceLoaderDispositionV1::Discarded => Self::Discarded,
            SourceLoaderDispositionV1::Unsupported => Self::Unsupported,
            SourceLoaderDispositionV1::Unknown => Self::Unknown,
            SourceLoaderDispositionV1::NotApplicable => Self::NotApplicable,
        }
    }
}

/// How a raw-source observation was established.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RawSourceProvenanceKindV1 {
    /// Normative format semantics.
    FormatDefined,
    /// Exact source declaration.
    SourceDeclared,
    /// Parser-effective projection.
    ParserProjected,
    /// Derived from exact source declarations.
    DerivedFromSource,
}

impl From<SourceProvenanceKindV1> for RawSourceProvenanceKindV1 {
    fn from(value: SourceProvenanceKindV1) -> Self {
        match value {
            SourceProvenanceKindV1::FormatDefined => Self::FormatDefined,
            SourceProvenanceKindV1::SourceDeclared => Self::SourceDeclared,
            SourceProvenanceKindV1::ParserProjected => Self::ParserProjected,
            SourceProvenanceKindV1::DerivedFromSource => Self::DerivedFromSource,
        }
    }
}

/// Bounded logical provenance retained with a scalar raw-source observation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawSourceProvenanceV1 {
    kind: RawSourceProvenanceKindV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    locator: Option<String>,
}

impl RawSourceProvenanceV1 {
    fn from_source(value: &SourceProvenanceV1) -> Self {
        Self {
            kind: value.kind().into(),
            locator: value.locator().map(|locator| locator.as_str().to_owned()),
        }
    }

    fn retained_text_bytes(&self) -> usize {
        self.locator.as_ref().map_or(0, String::len)
    }
}

/// Availability and value of one raw-source scalar observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum RawSourceObservationStateWireV1<T> {
    /// Exact observed value.
    Observed {
        /// Typed observed value.
        value: T,
    },
    /// Complete evidence proves absence.
    ProvenAbsent,
    /// The value could not be established.
    Unavailable {
        /// Stable reason.
        reason: RawSourceUnavailableReasonV1,
    },
}

/// One raw-source observation with orthogonal state, disposition, and provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RawSourceObservationWireV1<T> {
    #[serde(flatten)]
    state: RawSourceObservationStateWireV1<T>,
    disposition: RawSourceDispositionV1,
    provenance: Option<RawSourceProvenanceV1>,
}

impl<'de, T> Deserialize<'de> for RawSourceObservationWireV1<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
        enum WireObservation<T> {
            Observed {
                value: T,
                disposition: RawSourceDispositionV1,
                provenance: Option<RawSourceProvenanceV1>,
            },
            ProvenAbsent {
                disposition: RawSourceDispositionV1,
                provenance: Option<RawSourceProvenanceV1>,
            },
            Unavailable {
                reason: RawSourceUnavailableReasonV1,
                disposition: RawSourceDispositionV1,
                provenance: Option<RawSourceProvenanceV1>,
            },
        }
        let (state, disposition, provenance) = match WireObservation::deserialize(deserializer)? {
            WireObservation::Observed {
                value,
                disposition,
                provenance,
            } => (
                RawSourceObservationStateWireV1::Observed { value },
                disposition,
                provenance,
            ),
            WireObservation::ProvenAbsent {
                disposition,
                provenance,
            } => (
                RawSourceObservationStateWireV1::ProvenAbsent,
                disposition,
                provenance,
            ),
            WireObservation::Unavailable {
                reason,
                disposition,
                provenance,
            } => (
                RawSourceObservationStateWireV1::Unavailable { reason },
                disposition,
                provenance,
            ),
        };
        Ok(Self {
            state,
            disposition,
            provenance,
        })
    }
}

impl<T> RawSourceObservationWireV1<T> {
    fn from_source<U>(value: &SourceObservationV1<U>, map: impl FnOnce(&U) -> T) -> Self {
        let state = match value.state() {
            SourceObservationStateV1::Observed(observed) => {
                RawSourceObservationStateWireV1::Observed {
                    value: map(observed),
                }
            }
            SourceObservationStateV1::ProvenAbsent => RawSourceObservationStateWireV1::ProvenAbsent,
            SourceObservationStateV1::Unavailable(reason) => {
                RawSourceObservationStateWireV1::Unavailable {
                    reason: (*reason).into(),
                }
            }
        };
        Self {
            state,
            disposition: value.disposition().into(),
            provenance: value.provenance().map(RawSourceProvenanceV1::from_source),
        }
    }

    fn retained_text_bytes(&self) -> usize {
        self.provenance
            .as_ref()
            .map_or(0, RawSourceProvenanceV1::retained_text_bytes)
    }
}

/// Signed axis retained in a raw-source coordinate-basis observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RawSourceAxisV1 {
    /// Positive X.
    PositiveX,
    /// Negative X.
    NegativeX,
    /// Positive Y.
    PositiveY,
    /// Negative Y.
    NegativeY,
    /// Positive Z.
    PositiveZ,
    /// Negative Z.
    NegativeZ,
}

impl From<SourceAxisV1> for RawSourceAxisV1 {
    fn from(value: SourceAxisV1) -> Self {
        match value {
            SourceAxisV1::PositiveX => Self::PositiveX,
            SourceAxisV1::NegativeX => Self::NegativeX,
            SourceAxisV1::PositiveY => Self::PositiveY,
            SourceAxisV1::NegativeY => Self::NegativeY,
            SourceAxisV1::PositiveZ => Self::PositiveZ,
            SourceAxisV1::NegativeZ => Self::NegativeZ,
        }
    }
}

/// Exact signed semantic source coordinate basis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawSourceCoordinateBasisV1 {
    right: RawSourceAxisV1,
    up: RawSourceAxisV1,
    forward: RawSourceAxisV1,
}

impl From<SourceCoordinateBasisV1> for RawSourceCoordinateBasisV1 {
    fn from(value: SourceCoordinateBasisV1) -> Self {
        Self {
            right: value.right().into(),
            up: value.up().into(),
            forward: value.forward().into(),
        }
    }
}

/// Coverage retained for one independently bounded raw-source row domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawSourceSetCoverageV1 {
    state: RawSourceSetCoverageStateV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<RawSourceUnavailableReasonV1>,
}

/// Exhaustiveness state of a raw-source row domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RawSourceSetCoverageStateV1 {
    /// Complete domain.
    Complete,
    /// Authoritative retained prefix.
    Partial,
    /// Domain unavailable.
    Unavailable,
}

impl From<SourceSetCoverageV1> for RawSourceSetCoverageV1 {
    fn from(value: SourceSetCoverageV1) -> Self {
        Self {
            state: match value.state() {
                SourceSetCoverageStateV1::Complete => RawSourceSetCoverageStateV1::Complete,
                SourceSetCoverageStateV1::Partial => RawSourceSetCoverageStateV1::Partial,
                SourceSetCoverageStateV1::Unavailable => RawSourceSetCoverageStateV1::Unavailable,
            },
            reason: value.reason().map(Into::into),
        }
    }
}

impl RawSourceSetCoverageV1 {
    /// Exhaustiveness state retained from the raw-source row domain.
    pub const fn state(self) -> RawSourceSetCoverageStateV1 {
        self.state
    }

    /// Typed incompleteness reason, absent exactly for complete coverage.
    pub const fn reason(self) -> Option<RawSourceUnavailableReasonV1> {
        self.reason
    }
}

/// Bounded raw-source projection work counters in their V1 declaration order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawSourceProjectionWorkWireV1 {
    inspected_rows: u64,
    retained_rows: u64,
    retained_text_bytes: u64,
    max_traversal_depth: u64,
}

/// Same-load raw-source evidence embedded in prediction provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RawSourceBindingV1 {
    schema: &'static str,
    primary_input: InputIdentity,
    #[serde(serialize_with = "serialize_source_format")]
    source_format: SourceFormatV1,
    linear_unit: RawSourceObservationWireV1<FinitePredictionNumberV1>,
    coordinate_basis: RawSourceObservationWireV1<RawSourceCoordinateBasisV1>,
    frames_per_second: RawSourceObservationWireV1<FinitePredictionNumberV1>,
    clips_coverage: RawSourceSetCoverageV1,
    constructs_coverage: RawSourceSetCoverageV1,
    resources_coverage: RawSourceSetCoverageV1,
    source_skeleton_coverage: SourceSkeletonCoverage,
    work: RawSourceProjectionWorkWireV1,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSourceBindingWireV1 {
    schema: String,
    primary_input: InputIdentity,
    source_format: SourceFormatV1,
    linear_unit: RawSourceObservationWireV1<FinitePredictionNumberV1>,
    coordinate_basis: RawSourceObservationWireV1<RawSourceCoordinateBasisV1>,
    frames_per_second: RawSourceObservationWireV1<FinitePredictionNumberV1>,
    clips_coverage: RawSourceSetCoverageV1,
    constructs_coverage: RawSourceSetCoverageV1,
    resources_coverage: RawSourceSetCoverageV1,
    source_skeleton_coverage: SourceSkeletonCoverage,
    work: RawSourceProjectionWorkWireV1,
}

impl<'de> Deserialize<'de> for RawSourceBindingV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::from_wire(RawSourceBindingWireV1::deserialize(deserializer)?)
            .map_err(D::Error::custom)
    }
}

impl RawSourceBindingV1 {
    fn from_wire(wire: RawSourceBindingWireV1) -> Result<Self, PredictionContractError> {
        if wire.schema != RAW_SOURCE_FACTS_V1_ID {
            return Err(PredictionContractError::InvalidSchema {
                field: "raw_source.schema",
                expected: RAW_SOURCE_FACTS_V1_ID,
                found: wire.schema,
            });
        }
        let binding = Self {
            schema: RAW_SOURCE_FACTS_V1_ID,
            primary_input: wire.primary_input,
            source_format: wire.source_format,
            linear_unit: wire.linear_unit,
            coordinate_basis: wire.coordinate_basis,
            frames_per_second: wire.frames_per_second,
            clips_coverage: wire.clips_coverage,
            constructs_coverage: wire.constructs_coverage,
            resources_coverage: wire.resources_coverage,
            source_skeleton_coverage: wire.source_skeleton_coverage,
            work: wire.work,
        };
        binding.validate_wire()?;
        Ok(binding)
    }

    /// Project the bounded scalar/coverage authority from one same-load facts view.
    pub fn from_source(facts: SourceFactsViewV1<'_>) -> Self {
        let work = facts.work();
        Self {
            schema: RAW_SOURCE_FACTS_V1_ID,
            primary_input: facts.primary_identity().clone(),
            source_format: facts.format(),
            linear_unit: RawSourceObservationWireV1::from_source(
                facts.linear_unit(),
                |value: &SourceLinearUnitV1| {
                    FinitePredictionNumberV1::new(value.meters_per_source_unit())
                        .expect("source linear units are finite")
                },
            ),
            coordinate_basis: RawSourceObservationWireV1::from_source(
                facts.coordinate_basis(),
                |value: &SourceCoordinateBasisV1| (*value).into(),
            ),
            frames_per_second: RawSourceObservationWireV1::from_source(
                facts.frames_per_second(),
                |value: &SourceFramesPerSecondV1| {
                    FinitePredictionNumberV1::new(value.get())
                        .expect("source frame rates are finite")
                },
            ),
            clips_coverage: facts.clips().coverage().into(),
            constructs_coverage: facts.constructs().coverage().into(),
            resources_coverage: facts.resources().coverage().into(),
            source_skeleton_coverage: facts.source_skeleton().coverage,
            work: RawSourceProjectionWorkWireV1 {
                inspected_rows: work.inspected_rows() as u64,
                retained_rows: work.retained_rows() as u64,
                retained_text_bytes: work.retained_text_bytes() as u64,
                max_traversal_depth: work.max_traversal_depth() as u64,
            },
        }
    }

    /// Raw-source facts contract identity.
    pub const fn contract_id(&self) -> &'static str {
        self.schema
    }

    /// Exact primary input parsed by the loader.
    pub const fn primary_input(&self) -> &InputIdentity {
        &self.primary_input
    }

    /// Exact source container format.
    pub const fn source_format(&self) -> SourceFormatV1 {
        self.source_format
    }

    /// Coverage of the raw source animation row domain.
    pub const fn clips_coverage(&self) -> RawSourceSetCoverageV1 {
        self.clips_coverage
    }

    fn retained_text_bytes(&self) -> Result<usize, PredictionContractError> {
        checked_sum(
            "raw-source binding retained text",
            [
                self.linear_unit.retained_text_bytes(),
                self.coordinate_basis.retained_text_bytes(),
                self.frames_per_second.retained_text_bytes(),
            ],
        )
    }

    fn validate_wire(&self) -> Result<(), PredictionContractError> {
        validate_raw_observation(&self.linear_unit, |value| value.get() > 0.0)?;
        validate_raw_observation(&self.frames_per_second, |value| value.get() > 0.0)?;
        validate_raw_observation(&self.coordinate_basis, valid_raw_basis)?;
        for coverage in [
            self.clips_coverage,
            self.constructs_coverage,
            self.resources_coverage,
        ] {
            let valid = matches!(
                (coverage.state, coverage.reason),
                (RawSourceSetCoverageStateV1::Complete, None)
                    | (RawSourceSetCoverageStateV1::Partial, Some(_))
                    | (RawSourceSetCoverageStateV1::Unavailable, Some(_))
            );
            if !valid {
                return Err(PredictionContractError::RawSourceFieldUnavailable(
                    "coverage state/reason".to_owned(),
                ));
            }
        }
        if self.work.retained_text_bytes
            > u64::try_from(RAW_SOURCE_V1_MAX_TOTAL_TEXT_BYTES).unwrap_or(u64::MAX)
        {
            return Err(PredictionContractError::TooMuchRetainedText {
                found: usize::try_from(self.work.retained_text_bytes).unwrap_or(usize::MAX),
                limit: RAW_SOURCE_V1_MAX_TOTAL_TEXT_BYTES,
            });
        }
        let max_inspected = RAW_SOURCE_V1_MAX_OBSERVATIONS.saturating_add(3);
        if self.work.inspected_rows > u64::try_from(max_inspected).unwrap_or(u64::MAX)
            || self.work.retained_rows
                > u64::try_from(RAW_SOURCE_V1_MAX_OBSERVATIONS).unwrap_or(u64::MAX)
            || self.work.retained_rows > self.work.inspected_rows
            || self.work.max_traversal_depth
                > u64::try_from(RAW_SOURCE_V1_MAX_TRAVERSAL_DEPTH.saturating_add(1))
                    .unwrap_or(u64::MAX)
        {
            return Err(PredictionContractError::RawSourceFieldUnavailable(
                "raw-source work counters".to_owned(),
            ));
        }
        for observation in [
            self.linear_unit.provenance.as_ref(),
            self.coordinate_basis.provenance.as_ref(),
            self.frames_per_second.provenance.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            if let Some(locator) = &observation.locator {
                bounded_string("raw-source provenance locator", locator)?;
            }
        }
        Ok(())
    }
}

fn validate_raw_observation<T>(
    observation: &RawSourceObservationWireV1<T>,
    valid_value: impl FnOnce(&T) -> bool,
) -> Result<(), PredictionContractError> {
    match &observation.state {
        RawSourceObservationStateWireV1::Observed { value } => {
            if observation.provenance.is_none() || !valid_value(value) {
                return Err(PredictionContractError::RawSourceValueMismatch);
            }
        }
        RawSourceObservationStateWireV1::ProvenAbsent => {
            if observation.provenance.is_none() {
                return Err(PredictionContractError::RawSourceValueMismatch);
            }
        }
        RawSourceObservationStateWireV1::Unavailable { .. } => {}
    }
    Ok(())
}

fn valid_raw_basis(value: &RawSourceCoordinateBasisV1) -> bool {
    fn unsigned(axis: RawSourceAxisV1) -> u8 {
        match axis {
            RawSourceAxisV1::PositiveX | RawSourceAxisV1::NegativeX => 0,
            RawSourceAxisV1::PositiveY | RawSourceAxisV1::NegativeY => 1,
            RawSourceAxisV1::PositiveZ | RawSourceAxisV1::NegativeZ => 2,
        }
    }
    unsigned(value.right) != unsigned(value.up)
        && unsigned(value.right) != unsigned(value.forward)
        && unsigned(value.up) != unsigned(value.forward)
}

fn serialize_source_format<S>(value: &SourceFormatV1, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(source_format_name(*value))
}

const CONSUMED_CONTRACTS_V1: [&str; 5] = [
    OUTPUT_V10_SCHEMA_ID,
    MEASUREMENTS_SCHEMA_ID,
    RAW_SOURCE_FACTS_V1_ID,
    DEPENDENCY_CLOSURE_V1_ID,
    ENGINE_PROFILE_FACTS_V1_ID,
];

fn encode_option<T>(
    encoder: &mut CanonicalEncoder,
    value: Option<T>,
    encode: impl FnOnce(&mut CanonicalEncoder, T),
) {
    match value {
        Some(value) => {
            encoder.token("some");
            encode(encoder, value);
        }
        None => encoder.token("none"),
    }
}

fn encode_scalar(encoder: &mut CanonicalEncoder, value: &PredictionScalarV1) {
    match value {
        PredictionScalarV1::Null => encoder.token("null"),
        PredictionScalarV1::Boolean { value } => {
            encoder.token("boolean");
            encoder.token(if *value { "true" } else { "false" });
        }
        PredictionScalarV1::SignedInteger { value } => {
            encoder.token("signed_integer");
            encoder.token(value.to_string());
        }
        PredictionScalarV1::UnsignedInteger { value } => {
            encoder.token("unsigned_integer");
            encoder.token(value.to_string());
        }
        PredictionScalarV1::FiniteNumber { value } => {
            encoder.token("finite_number");
            encoder.token(value.canonical_bits());
        }
        PredictionScalarV1::Token { value } => {
            encoder.token("token");
            encoder.token(value);
        }
        PredictionScalarV1::Text { value } => {
            encoder.token("text");
            encoder.token(value);
        }
    }
}

fn encode_setting_location(encoder: &mut CanonicalEncoder, location: &ResolvedSettingLocationV1) {
    match location {
        ResolvedSettingLocationV1::Document => encoder.token("document"),
        ResolvedSettingLocationV1::Clip {
            clip_ordinal,
            clip_name,
        } => {
            encoder.token("clip");
            encoder.token(clip_ordinal.to_string());
            encoder.token(clip_name);
        }
    }
}

fn raw_domain_name(value: RawSourceDomainV1) -> &'static str {
    match value {
        RawSourceDomainV1::LinearUnit => "linear_unit",
        RawSourceDomainV1::CoordinateBasis => "coordinate_basis",
        RawSourceDomainV1::FramesPerSecond => "frames_per_second",
        RawSourceDomainV1::Clip => "clip",
        RawSourceDomainV1::Channel => "channel",
        RawSourceDomainV1::Construct => "construct",
        RawSourceDomainV1::Resource => "resource",
        RawSourceDomainV1::SourceNode => "source_node",
        RawSourceDomainV1::SourceSkin => "source_skin",
    }
}

fn encode_raw_key(encoder: &mut CanonicalEncoder, key: &RawSourceKeyV1) {
    match key {
        RawSourceKeyV1::Scalar => encoder.token("scalar"),
        RawSourceKeyV1::Clip { source_clip_index } => {
            encoder.token("clip");
            encoder.token(source_clip_index.to_string());
        }
        RawSourceKeyV1::Channel {
            source_clip_index,
            source_channel_index,
        } => {
            encoder.token("channel");
            encoder.token(source_clip_index.to_string());
            encoder.token(source_channel_index.to_string());
        }
        RawSourceKeyV1::Construct { source_order_index } => {
            encoder.token("construct");
            encoder.token(source_order_index.to_string());
        }
        RawSourceKeyV1::Resource {
            source_order_index,
            source_index,
        } => {
            encoder.token("resource");
            encoder.token(source_order_index.to_string());
            encoder.token(source_index.to_string());
        }
        RawSourceKeyV1::SourceSkeleton {
            row_kind,
            source_index,
        } => {
            encoder.token("source_skeleton");
            encoder.token(match row_kind {
                SourceSkeletonRowKindV1::SourceNode => "source_node",
                SourceSkeletonRowKindV1::SourceSkin => "source_skin",
            });
            encoder.token(source_index.to_string());
        }
    }
}

fn encode_basis_reference(encoder: &mut CanonicalEncoder, reference: &PredictionBasisReferenceV1) {
    match reference {
        PredictionBasisReferenceV1::ProfileFact { fact_id } => {
            encoder.token("profile_fact");
            encoder.field("fact_id");
            encoder.token(fact_id);
        }
        PredictionBasisReferenceV1::ResolvedSetting {
            location,
            setting_id,
        } => {
            encoder.token("resolved_setting");
            encoder.field("location");
            encode_setting_location(encoder, location);
            encoder.field("setting_id");
            encoder.token(setting_id);
        }
        PredictionBasisReferenceV1::ProjectField { field_id, value } => {
            encoder.token("project_field");
            encoder.field("field_id");
            encoder.token(field_id);
            encoder.field("value");
            encode_scalar(encoder, value);
        }
        PredictionBasisReferenceV1::RawSource { reference } => {
            encoder.token("raw_source");
            encoder.field("domain");
            encoder.token(raw_domain_name(reference.domain));
            encoder.field("key");
            encode_raw_key(encoder, &reference.key);
            encoder.field("field");
            encoder.token(reference.field.as_str());
            encoder.field("value");
            encode_scalar(encoder, &reference.value);
        }
        PredictionBasisReferenceV1::Measurement {
            schema,
            pointer,
            value,
        } => {
            encoder.token("measurement");
            encoder.field("schema");
            encoder.token(schema);
            encoder.field("pointer");
            encoder.token(pointer.as_str());
            encoder.field("value");
            encode_scalar(encoder, value);
        }
        PredictionBasisReferenceV1::PrimarySource { source_id } => {
            encoder.token("primary_source");
            encoder.field("source_id");
            encoder.token(source_id);
        }
    }
}

fn basis_reference_key(reference: &PredictionBasisReferenceV1) -> (u8, Vec<u8>) {
    let mut encoder = CanonicalEncoder::default();
    encode_basis_reference(&mut encoder, reference);
    let variant = match reference {
        PredictionBasisReferenceV1::ProfileFact { .. } => 0,
        PredictionBasisReferenceV1::ResolvedSetting { .. } => 1,
        PredictionBasisReferenceV1::ProjectField { .. } => 2,
        PredictionBasisReferenceV1::RawSource { .. } => 3,
        PredictionBasisReferenceV1::Measurement { .. } => 4,
        PredictionBasisReferenceV1::PrimarySource { .. } => 5,
    };
    (variant, encoder.into_bytes())
}

/// Domain-separated identity of one canonical prediction basis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PredictionBasisIdentityV1(InputIdentity);

impl PredictionBasisIdentityV1 {
    /// SHA-256 and canonical-preimage byte count.
    pub const fn input_identity(&self) -> &InputIdentity {
        &self.0
    }
}

/// Canonical typed evidence used by one prediction facet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EnginePredictionBasisV1 {
    identity: PredictionBasisIdentityV1,
    references: Vec<PredictionBasisReferenceV1>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EnginePredictionBasisWireV1 {
    identity: PredictionBasisIdentityV1,
    #[serde(deserialize_with = "deserialize_basis_references")]
    references: CappedSequence<PredictionBasisReferenceWireV1>,
}

struct EnginePredictionBasisSeed<'a> {
    references: &'a mut RowBudget,
}

impl<'de> DeserializeSeed<'de> for EnginePredictionBasisSeed<'_> {
    type Value = EnginePredictionBasisWireV1;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Field {
            Identity,
            References,
        }

        struct BasisVisitor<'a> {
            references: &'a mut RowBudget,
        }

        impl<'de> Visitor<'de> for BasisVisitor<'_> {
            type Value = EnginePredictionBasisWireV1;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an engine prediction basis")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut identity = None;
                let mut references = None;
                while let Some(field) = map.next_key()? {
                    match field {
                        Field::Identity => {
                            set_prediction_field(&mut identity, map.next_value()?, "identity")?
                        }
                        Field::References => {
                            if references.is_some() {
                                return Err(A::Error::duplicate_field("references"));
                            }
                            references = Some(map.next_value_seed(BudgetedCappedSequenceSeed {
                                budget: self.references,
                                local_limit: PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FACET,
                                element: PhantomData,
                            })?);
                        }
                    }
                }
                Ok(EnginePredictionBasisWireV1 {
                    identity: required_prediction_field(identity, "identity")?,
                    references: required_prediction_field(references, "references")?,
                })
            }
        }

        deserializer.deserialize_struct(
            "EnginePredictionBasisV1",
            &["identity", "references"],
            BasisVisitor {
                references: self.references,
            },
        )
    }
}

fn set_prediction_field<E, T>(slot: &mut Option<T>, value: T, field: &'static str) -> Result<(), E>
where
    E: serde::de::Error,
{
    if slot.replace(value).is_some() {
        return Err(E::duplicate_field(field));
    }
    Ok(())
}

fn required_prediction_field<E, T>(value: Option<T>, field: &'static str) -> Result<T, E>
where
    E: serde::de::Error,
{
    value.ok_or_else(|| E::missing_field(field))
}

impl TryFrom<EnginePredictionBasisWireV1> for EnginePredictionBasisV1 {
    type Error = PredictionContractError;

    fn try_from(wire: EnginePredictionBasisWireV1) -> Result<Self, Self::Error> {
        if wire.references.overflowed {
            return Err(PredictionContractError::TooManyBasisReferences {
                found: PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FACET + 1,
                limit: PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FACET,
            });
        }
        let basis = Self {
            identity: wire.identity,
            references: wire
                .references
                .values
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_, _>>()?,
        };
        basis.validate()?;
        Ok(basis)
    }
}

impl<'de> Deserialize<'de> for EnginePredictionBasisV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_from(EnginePredictionBasisWireV1::deserialize(deserializer)?)
            .map_err(D::Error::custom)
    }
}

impl EnginePredictionBasisV1 {
    /// Construct and canonically order one bounded evidence basis.
    pub fn new(
        mut references: Vec<PredictionBasisReferenceV1>,
    ) -> Result<Self, PredictionContractError> {
        if references.len() > PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FACET {
            return Err(PredictionContractError::TooManyBasisReferences {
                found: references.len(),
                limit: PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FACET,
            });
        }
        for reference in &references {
            validate_basis_reference_structure(reference)?;
        }
        references.sort_by_cached_key(basis_reference_key);
        if references
            .windows(2)
            .any(|rows| basis_reference_key(&rows[0]) == basis_reference_key(&rows[1]))
        {
            return Err(PredictionContractError::DuplicateBasisReference);
        }
        let identity = PredictionBasisIdentityV1(compute_basis_identity(&references));
        Ok(Self {
            identity,
            references,
        })
    }

    /// Canonical basis identity.
    pub const fn identity(&self) -> &PredictionBasisIdentityV1 {
        &self.identity
    }

    /// Canonically ordered typed references.
    pub fn references(&self) -> &[PredictionBasisReferenceV1] {
        &self.references
    }

    fn validate(&self) -> Result<(), PredictionContractError> {
        if self.references.len() > PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FACET {
            return Err(PredictionContractError::TooManyBasisReferences {
                found: self.references.len(),
                limit: PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FACET,
            });
        }
        for reference in &self.references {
            validate_basis_reference_structure(reference)?;
        }
        let keys: Vec<_> = self.references.iter().map(basis_reference_key).collect();
        if keys.windows(2).any(|rows| rows[0] >= rows[1]) {
            return Err(if keys.windows(2).any(|rows| rows[0] == rows[1]) {
                PredictionContractError::DuplicateBasisReference
            } else {
                PredictionContractError::NonCanonicalOrder("basis references")
            });
        }
        if self.identity.0 != compute_basis_identity(&self.references) {
            return Err(PredictionContractError::IdentityMismatch {
                contract: "engine prediction basis v1",
            });
        }
        Ok(())
    }

    fn retained_text_bytes(&self) -> Result<usize, PredictionContractError> {
        self.references.iter().try_fold(0usize, |total, reference| {
            total.checked_add(reference.retained_text_bytes()?).ok_or(
                PredictionContractError::ArithmeticOverflow("basis retained text"),
            )
        })
    }
}

fn validate_basis_reference_structure(
    reference: &PredictionBasisReferenceV1,
) -> Result<(), PredictionContractError> {
    match reference {
        PredictionBasisReferenceV1::ProfileFact { fact_id } => {
            stable_token("profile fact id", fact_id)?;
        }
        PredictionBasisReferenceV1::ResolvedSetting {
            location,
            setting_id,
        } => {
            if let ResolvedSettingLocationV1::Clip { clip_name, .. } = location {
                bounded_string("clip name", clip_name)?;
            }
            stable_token("setting id", setting_id)?;
        }
        PredictionBasisReferenceV1::ProjectField { field_id, value } => {
            stable_bounded_id("project field id", field_id)?;
            validate_scalar(value)?;
        }
        PredictionBasisReferenceV1::RawSource { reference } => {
            if !raw_domain_matches_key(reference.domain, &reference.key) {
                return Err(PredictionContractError::RawSourceDomainKeyMismatch);
            }
            RawSourceFieldIdV1::new(reference.field.as_str())?;
            validate_scalar(&reference.value)?;
        }
        PredictionBasisReferenceV1::Measurement {
            schema,
            pointer,
            value,
        } => {
            if *schema != MEASUREMENTS_SCHEMA_ID {
                return Err(PredictionContractError::InvalidSchema {
                    field: "basis.measurement.schema",
                    expected: MEASUREMENTS_SCHEMA_ID,
                    found: (*schema).to_owned(),
                });
            }
            MeasurementPointerV1::new(pointer.as_str())?;
            validate_scalar(value)?;
        }
        PredictionBasisReferenceV1::PrimarySource { source_id } => {
            stable_token("primary source id", source_id)?;
        }
    }
    Ok(())
}

fn validate_scalar(value: &PredictionScalarV1) -> Result<(), PredictionContractError> {
    match value {
        PredictionScalarV1::FiniteNumber { value } => {
            FinitePredictionNumberV1::new(value.get())?;
        }
        PredictionScalarV1::Token { value } => {
            stable_token("scalar token", value)?;
        }
        PredictionScalarV1::Text { value } => {
            bounded_string("scalar text", value)?;
        }
        PredictionScalarV1::Null
        | PredictionScalarV1::Boolean { .. }
        | PredictionScalarV1::SignedInteger { .. }
        | PredictionScalarV1::UnsignedInteger { .. } => {}
    }
    Ok(())
}

fn stable_bounded_id(
    field: &'static str,
    value: impl Into<String>,
) -> Result<String, PredictionContractError> {
    let value = stable_token(field, value)?;
    if !value.bytes().enumerate().all(|(index, byte)| {
        if index == 0 {
            byte.is_ascii_alphanumeric()
        } else {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'/' | b'+' | b'-')
        }
    }) {
        return Err(PredictionContractError::InvalidToken { field, value });
    }
    Ok(value)
}

fn compute_basis_identity(references: &[PredictionBasisReferenceV1]) -> InputIdentity {
    let mut encoder = CanonicalEncoder::new("animsmith-engine-prediction-basis-v1");
    encoder.field("references");
    encoder.count(references.len());
    for reference in references {
        encode_basis_reference(&mut encoder, reference);
    }
    encoder.identity()
}

/// Stable reason prediction work was required but could not be completed.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PredictionUnavailableReasonV1 {
    /// Required raw-source evidence was incomplete.
    RawSourceIncomplete,
    /// Required dependency closure was incomplete.
    DependencyClosureIncomplete,
    /// A required immutable profile fact was unknown.
    ProfileFactUnknown,
    /// Required project intent was unavailable.
    ProjectIntentUnavailable,
    /// Required validated measurement was unavailable.
    MeasurementUnavailable,
    /// A source selector matched no row.
    SourceSelectorNoMatch,
    /// A source selector matched multiple rows.
    SourceSelectorAmbiguous,
    /// Required primary-source evidence was unavailable.
    PrimarySourceUnavailable,
    /// Namespaced custom-check reason.
    Custom(String),
}

impl PredictionUnavailableReasonV1 {
    /// Construct a bounded namespaced custom reason code.
    pub fn custom(value: impl Into<String>) -> Result<Self, PredictionContractError> {
        let value = bounded_string("unavailable reason", value)?;
        if !valid_custom_reason(&value) {
            return Err(PredictionContractError::InvalidUnavailableReasonCode(value));
        }
        Ok(Self::Custom(value))
    }

    /// Exact snake-case or namespaced wire code.
    pub fn as_str(&self) -> &str {
        match self {
            Self::RawSourceIncomplete => "raw_source_incomplete",
            Self::DependencyClosureIncomplete => "dependency_closure_incomplete",
            Self::ProfileFactUnknown => "profile_fact_unknown",
            Self::ProjectIntentUnavailable => "project_intent_unavailable",
            Self::MeasurementUnavailable => "measurement_unavailable",
            Self::SourceSelectorNoMatch => "source_selector_no_match",
            Self::SourceSelectorAmbiguous => "source_selector_ambiguous",
            Self::PrimarySourceUnavailable => "primary_source_unavailable",
            Self::Custom(value) => value,
        }
    }

    fn from_wire(value: String) -> Result<Self, PredictionContractError> {
        let builtin = match value.as_str() {
            "raw_source_incomplete" => Some(Self::RawSourceIncomplete),
            "dependency_closure_incomplete" => Some(Self::DependencyClosureIncomplete),
            "profile_fact_unknown" => Some(Self::ProfileFactUnknown),
            "project_intent_unavailable" => Some(Self::ProjectIntentUnavailable),
            "measurement_unavailable" => Some(Self::MeasurementUnavailable),
            "source_selector_no_match" => Some(Self::SourceSelectorNoMatch),
            "source_selector_ambiguous" => Some(Self::SourceSelectorAmbiguous),
            "primary_source_unavailable" => Some(Self::PrimarySourceUnavailable),
            _ => None,
        };
        builtin.map_or_else(|| Self::custom(value), Ok)
    }
}

fn valid_custom_reason(value: &str) -> bool {
    let mut segments = value.split(':');
    let valid_segment = |segment: &str| {
        !segment.is_empty()
            && segment.as_bytes()[0].is_ascii_lowercase()
            && segment.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
            })
    };
    let first = segments.next().is_some_and(valid_segment);
    let rest: Vec<_> = segments.collect();
    first && !rest.is_empty() && rest.iter().all(|segment| valid_segment(segment))
}

impl Serialize for PredictionUnavailableReasonV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for PredictionUnavailableReasonV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::from_wire(value).map_err(D::Error::custom)
    }
}

/// Availability of one independently scoped prediction work unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnginePredictionFacetStateV1 {
    /// The prediction completed from a nonempty basis.
    Available,
    /// The prediction was required but prerequisites were unavailable.
    RequiredPredictionUnavailable,
}

/// One independently scoped prediction work unit on an existing check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EnginePredictionFacetV1 {
    scope: EvaluationScope,
    state: EnginePredictionFacetStateV1,
    basis: EnginePredictionBasisV1,
    reasons: Vec<PredictionUnavailableReasonV1>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EnginePredictionFacetWireV1 {
    scope: EvaluationScope,
    state: EnginePredictionFacetStateV1,
    basis: EnginePredictionBasisWireV1,
    #[serde(deserialize_with = "deserialize_unavailable_reasons")]
    reasons: CappedSequence<String>,
}

struct EnginePredictionFacetSeed<'a> {
    references: &'a mut RowBudget,
}

impl<'de> DeserializeSeed<'de> for EnginePredictionFacetSeed<'_> {
    type Value = EnginePredictionFacetWireV1;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Field {
            Scope,
            State,
            Basis,
            Reasons,
        }

        struct FacetVisitor<'a> {
            references: &'a mut RowBudget,
        }

        impl<'de> Visitor<'de> for FacetVisitor<'_> {
            type Value = EnginePredictionFacetWireV1;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an engine prediction facet")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut scope = None;
                let mut state = None;
                let mut basis = None;
                let mut reasons = None;
                while let Some(field) = map.next_key()? {
                    match field {
                        Field::Scope => {
                            set_prediction_field(&mut scope, map.next_value()?, "scope")?
                        }
                        Field::State => {
                            set_prediction_field(&mut state, map.next_value()?, "state")?
                        }
                        Field::Basis => {
                            if basis.is_some() {
                                return Err(A::Error::duplicate_field("basis"));
                            }
                            basis = Some(map.next_value_seed(EnginePredictionBasisSeed {
                                references: self.references,
                            })?);
                        }
                        Field::Reasons => {
                            if reasons.is_some() {
                                return Err(A::Error::duplicate_field("reasons"));
                            }
                            reasons = Some(map.next_value_seed(CappedSequenceSeed {
                                limit: PREDICTION_V1_MAX_REASONS_PER_FACET,
                                element: PhantomData,
                            })?);
                        }
                    }
                }
                Ok(EnginePredictionFacetWireV1 {
                    scope: required_prediction_field(scope, "scope")?,
                    state: required_prediction_field(state, "state")?,
                    basis: required_prediction_field(basis, "basis")?,
                    reasons: required_prediction_field(reasons, "reasons")?,
                })
            }
        }

        deserializer.deserialize_struct(
            "EnginePredictionFacetV1",
            &["scope", "state", "basis", "reasons"],
            FacetVisitor {
                references: self.references,
            },
        )
    }
}

impl TryFrom<EnginePredictionFacetWireV1> for EnginePredictionFacetV1 {
    type Error = PredictionContractError;

    fn try_from(wire: EnginePredictionFacetWireV1) -> Result<Self, Self::Error> {
        if wire.reasons.overflowed {
            return Err(PredictionContractError::TooManyUnavailableReasons {
                found: PREDICTION_V1_MAX_REASONS_PER_FACET + 1,
                limit: PREDICTION_V1_MAX_REASONS_PER_FACET,
            });
        }
        let facet = Self {
            scope: wire.scope,
            state: wire.state,
            basis: wire.basis.try_into()?,
            reasons: wire
                .reasons
                .values
                .into_iter()
                .map(PredictionUnavailableReasonV1::from_wire)
                .collect::<Result<_, _>>()?,
        };
        facet.validate()?;
        Ok(facet)
    }
}

impl<'de> Deserialize<'de> for EnginePredictionFacetV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_from(EnginePredictionFacetWireV1::deserialize(deserializer)?)
            .map_err(D::Error::custom)
    }
}

impl EnginePredictionFacetV1 {
    /// Construct an available facet with nonempty evidence.
    pub fn available(
        scope: EvaluationScope,
        basis: EnginePredictionBasisV1,
    ) -> Result<Self, PredictionContractError> {
        Self::from_parts(
            scope,
            EnginePredictionFacetStateV1::Available,
            basis,
            Vec::new(),
        )
    }

    /// Construct a required-unavailable facet and canonicalize its reasons.
    pub fn required_unavailable(
        scope: EvaluationScope,
        basis: EnginePredictionBasisV1,
        reasons: Vec<PredictionUnavailableReasonV1>,
    ) -> Result<Self, PredictionContractError> {
        Self::from_parts(
            scope,
            EnginePredictionFacetStateV1::RequiredPredictionUnavailable,
            basis,
            reasons,
        )
    }

    fn from_parts(
        scope: EvaluationScope,
        state: EnginePredictionFacetStateV1,
        basis: EnginePredictionBasisV1,
        mut reasons: Vec<PredictionUnavailableReasonV1>,
    ) -> Result<Self, PredictionContractError> {
        validate_scope(&scope)?;
        basis.validate()?;
        if reasons.len() > PREDICTION_V1_MAX_REASONS_PER_FACET {
            return Err(PredictionContractError::TooManyUnavailableReasons {
                found: reasons.len(),
                limit: PREDICTION_V1_MAX_REASONS_PER_FACET,
            });
        }
        reasons.sort_by(|left, right| left.as_str().as_bytes().cmp(right.as_str().as_bytes()));
        if let Some(reason) = reasons
            .windows(2)
            .find(|rows| rows[0].as_str() == rows[1].as_str())
            .map(|rows| rows[0].as_str().to_owned())
        {
            return Err(PredictionContractError::DuplicateUnavailableReason(reason));
        }
        match state {
            EnginePredictionFacetStateV1::Available => {
                if basis.references.is_empty() {
                    return Err(PredictionContractError::AvailableBasisEmpty);
                }
                if !reasons.is_empty() {
                    return Err(PredictionContractError::AvailableHasReasons);
                }
            }
            EnginePredictionFacetStateV1::RequiredPredictionUnavailable => {
                if reasons.is_empty() {
                    return Err(PredictionContractError::RequiredUnavailableWithoutReason);
                }
            }
        }
        Ok(Self {
            scope,
            state,
            basis,
            reasons,
        })
    }

    /// Existing check-evaluation scope identifying this work unit.
    pub const fn scope(&self) -> &EvaluationScope {
        &self.scope
    }

    /// Availability state.
    pub const fn state(&self) -> EnginePredictionFacetStateV1 {
        self.state
    }

    /// Canonical evidence basis, including any unavailable prefix.
    pub const fn basis(&self) -> &EnginePredictionBasisV1 {
        &self.basis
    }

    /// Canonically ordered unavailable reasons.
    pub fn reasons(&self) -> &[PredictionUnavailableReasonV1] {
        &self.reasons
    }

    fn retained_text_bytes(&self) -> Result<usize, PredictionContractError> {
        let reason_text = checked_sum(
            "V1 facet reason retained text",
            self.reasons.iter().map(|reason| reason.as_str().len()),
        )?;
        checked_sum(
            "V1 facet retained text",
            [
                self.scope.code.as_str().len(),
                self.scope.subject.as_ref().map_or(0, String::len),
                reason_text,
                self.basis.retained_text_bytes()?,
            ],
        )
    }

    fn validate(&self) -> Result<(), PredictionContractError> {
        validate_scope(&self.scope)?;
        self.basis.validate()?;
        if self.reasons.len() > PREDICTION_V1_MAX_REASONS_PER_FACET {
            return Err(PredictionContractError::TooManyUnavailableReasons {
                found: self.reasons.len(),
                limit: PREDICTION_V1_MAX_REASONS_PER_FACET,
            });
        }
        if self
            .reasons
            .windows(2)
            .any(|rows| rows[0].as_str().as_bytes() >= rows[1].as_str().as_bytes())
        {
            return Err(
                if self
                    .reasons
                    .windows(2)
                    .any(|rows| rows[0].as_str() == rows[1].as_str())
                {
                    PredictionContractError::DuplicateUnavailableReason(
                        self.reasons
                            .windows(2)
                            .find(|rows| rows[0].as_str() == rows[1].as_str())
                            .map_or_else(String::new, |rows| rows[0].as_str().to_owned()),
                    )
                } else {
                    PredictionContractError::NonCanonicalOrder("facet reasons")
                },
            );
        }
        match self.state {
            EnginePredictionFacetStateV1::Available => {
                if self.basis.references.is_empty() {
                    return Err(PredictionContractError::AvailableBasisEmpty);
                }
                if !self.reasons.is_empty() {
                    return Err(PredictionContractError::AvailableHasReasons);
                }
            }
            EnginePredictionFacetStateV1::RequiredPredictionUnavailable
                if self.reasons.is_empty() =>
            {
                return Err(PredictionContractError::RequiredUnavailableWithoutReason);
            }
            EnginePredictionFacetStateV1::RequiredPredictionUnavailable => {}
        }
        Ok(())
    }
}

fn validate_scope(scope: &EvaluationScope) -> Result<(), PredictionContractError> {
    stable_token("facet scope code", scope.code.as_str())?;
    if let Some(subject) = &scope.subject {
        bounded_string("facet scope subject", subject)?;
    }
    Ok(())
}

fn compare_scopes(left: &EvaluationScope, right: &EvaluationScope) -> Ordering {
    left.code
        .as_str()
        .as_bytes()
        .cmp(right.code.as_str().as_bytes())
        .then_with(|| match (&left.subject, &right.subject) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Less,
            (Some(_), None) => Ordering::Greater,
            (Some(left), Some(right)) => left.as_bytes().cmp(right.as_bytes()),
        })
}

/// Per-check engine-prediction attachment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EnginePredictionV1 {
    schema: &'static str,
    provenance_identity: PredictionProvenanceIdentityV1,
    facets: Vec<EnginePredictionFacetV1>,
}

struct EnginePredictionWireV1 {
    schema: String,
    provenance_identity: PredictionProvenanceIdentityV1,
    facets: CappedSequence<EnginePredictionFacetWireV1>,
    facet_budget: RowBudget,
    reference_budget: RowBudget,
}

enum FacetElement {
    Value(EnginePredictionFacetWireV1),
    Skipped,
}

struct FacetElementSeed<'a> {
    facets: &'a mut RowBudget,
    references: &'a mut RowBudget,
}

impl<'de> DeserializeSeed<'de> for FacetElementSeed<'_> {
    type Value = FacetElement;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        if self.facets.admit() {
            EnginePredictionFacetSeed {
                references: self.references,
            }
            .deserialize(deserializer)
            .map(FacetElement::Value)
        } else {
            IgnoredAny::deserialize(deserializer).map(|_| FacetElement::Skipped)
        }
    }
}

struct FacetsSeed<'a> {
    facets: &'a mut RowBudget,
    references: &'a mut RowBudget,
}

impl<'de> DeserializeSeed<'de> for FacetsSeed<'_> {
    type Value = CappedSequence<EnginePredictionFacetWireV1>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct FacetsVisitor<'a> {
            facets: &'a mut RowBudget,
            references: &'a mut RowBudget,
        }

        impl<'de> Visitor<'de> for FacetsVisitor<'_> {
            type Value = CappedSequence<EnginePredictionFacetWireV1>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a bounded sequence of engine prediction facets")
            }

            fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut values = Vec::with_capacity(
                    sequence
                        .size_hint()
                        .unwrap_or(0)
                        .min(PREDICTION_V1_MAX_FACETS_PER_FILE),
                );
                let mut seen = 0usize;
                while seen < PREDICTION_V1_MAX_FACETS_PER_FILE {
                    let Some(element) = sequence.next_element_seed(FacetElementSeed {
                        facets: self.facets,
                        references: self.references,
                    })?
                    else {
                        return Ok(CappedSequence {
                            values,
                            overflowed: false,
                        });
                    };
                    seen += 1;
                    match element {
                        FacetElement::Value(value) => values.push(value),
                        FacetElement::Skipped => {
                            let overflowed = consume_ignored_tail(
                                &mut sequence,
                                seen,
                                PREDICTION_V1_MAX_FACETS_PER_FILE,
                            )?;
                            return Ok(CappedSequence { values, overflowed });
                        }
                    }
                }
                let overflowed =
                    consume_ignored_tail(&mut sequence, seen, PREDICTION_V1_MAX_FACETS_PER_FILE)?;
                Ok(CappedSequence { values, overflowed })
            }
        }

        deserializer.deserialize_seq(FacetsVisitor {
            facets: self.facets,
            references: self.references,
        })
    }
}

struct EnginePredictionWireSeed {
    facet_limit: usize,
    reference_limit: usize,
}

impl<'de> DeserializeSeed<'de> for EnginePredictionWireSeed {
    type Value = EnginePredictionWireV1;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Field {
            Schema,
            ProvenanceIdentity,
            Facets,
        }

        struct PredictionVisitor {
            facet_limit: usize,
            reference_limit: usize,
        }

        impl<'de> Visitor<'de> for PredictionVisitor {
            type Value = EnginePredictionWireV1;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an engine prediction")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut facet_budget = RowBudget::new(self.facet_limit);
                let mut reference_budget = RowBudget::new(self.reference_limit);
                let mut schema = None;
                let mut provenance_identity = None;
                let mut facets = None;
                while let Some(field) = map.next_key()? {
                    match field {
                        Field::Schema => {
                            set_prediction_field(&mut schema, map.next_value()?, "schema")?
                        }
                        Field::ProvenanceIdentity => set_prediction_field(
                            &mut provenance_identity,
                            map.next_value()?,
                            "provenance_identity",
                        )?,
                        Field::Facets => {
                            if facets.is_some() {
                                return Err(A::Error::duplicate_field("facets"));
                            }
                            facets = Some(map.next_value_seed(FacetsSeed {
                                facets: &mut facet_budget,
                                references: &mut reference_budget,
                            })?);
                        }
                    }
                }
                Ok(EnginePredictionWireV1 {
                    schema: required_prediction_field(schema, "schema")?,
                    provenance_identity: required_prediction_field(
                        provenance_identity,
                        "provenance_identity",
                    )?,
                    facets: required_prediction_field(facets, "facets")?,
                    facet_budget,
                    reference_budget,
                })
            }
        }

        deserializer.deserialize_struct(
            "EnginePredictionV1",
            &["schema", "provenance_identity", "facets"],
            PredictionVisitor {
                facet_limit: self.facet_limit,
                reference_limit: self.reference_limit,
            },
        )
    }
}

impl<'de> Deserialize<'de> for EnginePredictionWireV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        EnginePredictionWireSeed {
            facet_limit: PREDICTION_V1_MAX_FACETS_PER_FILE,
            reference_limit: usize::MAX,
        }
        .deserialize(deserializer)
    }
}

#[allow(
    dead_code,
    reason = "V1 standalone deserialization remains an explicit historical API"
)]
#[derive(Debug)]
pub(crate) enum PredictionDecodeError {
    Shape(serde_json::Error),
    Semantic(PredictionContractError),
    TooManyFileFacets,
    TooManyFileBasisReferences,
}

impl EnginePredictionV1 {
    fn validate_wire_schema(schema: &str) -> Result<(), PredictionContractError> {
        if schema != ENGINE_PREDICTION_V1_ID {
            return Err(PredictionContractError::InvalidSchema {
                field: "prediction.schema",
                expected: ENGINE_PREDICTION_V1_ID,
                found: schema.to_owned(),
            });
        }
        Ok(())
    }

    fn from_wire(wire: EnginePredictionWireV1) -> Result<Self, PredictionContractError> {
        Self::validate_wire_schema(&wire.schema)?;
        if wire.facets.overflowed {
            return Err(PredictionContractError::TooManyFacets {
                found: PREDICTION_V1_MAX_FACETS_PER_FILE + 1,
                limit: PREDICTION_V1_MAX_FACETS_PER_FILE,
            });
        }
        let prediction = Self {
            schema: ENGINE_PREDICTION_V1_ID,
            provenance_identity: wire.provenance_identity,
            facets: wire
                .facets
                .values
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_, _>>()?,
        };
        prediction.validate_structure()?;
        Ok(prediction)
    }

    fn first_nested_limit_error(wire: &EnginePredictionWireV1) -> Option<PredictionContractError> {
        for facet in &wire.facets.values {
            if facet.reasons.overflowed {
                return Some(PredictionContractError::TooManyUnavailableReasons {
                    found: PREDICTION_V1_MAX_REASONS_PER_FACET + 1,
                    limit: PREDICTION_V1_MAX_REASONS_PER_FACET,
                });
            }
            if facet.basis.references.overflowed {
                return Some(PredictionContractError::TooManyBasisReferences {
                    found: PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FACET + 1,
                    limit: PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FACET,
                });
            }
        }
        None
    }
}

#[allow(
    dead_code,
    reason = "V1 standalone deserialization remains an explicit historical API"
)]
pub(crate) fn decode_engine_prediction_v1(
    raw: &str,
    facet_limit: usize,
    reference_limit: usize,
) -> Result<EnginePredictionV1, PredictionDecodeError> {
    let mut deserializer = serde_json::Deserializer::from_str(raw);
    let wire = EnginePredictionWireSeed {
        facet_limit,
        reference_limit,
    }
    .deserialize(&mut deserializer)
    .map_err(PredictionDecodeError::Shape)?;
    deserializer.end().map_err(PredictionDecodeError::Shape)?;
    EnginePredictionV1::validate_wire_schema(&wire.schema)
        .map_err(PredictionDecodeError::Semantic)?;
    if wire.facets.overflowed {
        return Err(PredictionDecodeError::Semantic(
            PredictionContractError::TooManyFacets {
                found: PREDICTION_V1_MAX_FACETS_PER_FILE + 1,
                limit: PREDICTION_V1_MAX_FACETS_PER_FILE,
            },
        ));
    }
    if let Some(error) = EnginePredictionV1::first_nested_limit_error(&wire) {
        return Err(PredictionDecodeError::Semantic(error));
    }
    if wire.facet_budget.overflowed() {
        return Err(PredictionDecodeError::TooManyFileFacets);
    }
    if wire.reference_budget.overflowed() {
        return Err(PredictionDecodeError::TooManyFileBasisReferences);
    }
    EnginePredictionV1::from_wire(wire).map_err(PredictionDecodeError::Semantic)
}

impl<'de> Deserialize<'de> for EnginePredictionV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = EnginePredictionWireV1::deserialize(deserializer)?;
        if let Some(error) = Self::first_nested_limit_error(&wire) {
            return Err(D::Error::custom(error));
        }
        Self::from_wire(wire).map_err(D::Error::custom)
    }
}

impl EnginePredictionV1 {
    /// Deserialize one prediction while enforcing caller-owned file budgets.
    ///
    /// Standalone [`Deserialize`] enforces only prediction-local V1 caps.
    /// Staged file and envelope readers use this entry point to stop before a
    /// row exceeds their remaining aggregate facet or basis-reference budget.
    pub fn deserialize_with_file_limits<'de, D>(
        deserializer: D,
        facet_limit: usize,
        reference_limit: usize,
    ) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = EnginePredictionWireSeed {
            facet_limit,
            reference_limit,
        }
        .deserialize(deserializer)?;
        Self::validate_wire_schema(&wire.schema).map_err(D::Error::custom)?;
        if wire.facets.overflowed {
            return Err(D::Error::custom(PredictionContractError::TooManyFacets {
                found: PREDICTION_V1_MAX_FACETS_PER_FILE + 1,
                limit: PREDICTION_V1_MAX_FACETS_PER_FILE,
            }));
        }
        if let Some(error) = Self::first_nested_limit_error(&wire) {
            return Err(D::Error::custom(error));
        }
        if wire.facet_budget.overflowed() {
            return Err(D::Error::custom(
                "engine prediction exceeds the V1 file facet limit",
            ));
        }
        if wire.reference_budget.overflowed() {
            return Err(D::Error::custom(
                "engine prediction exceeds the V1 file basis-reference limit",
            ));
        }
        Self::from_wire(wire).map_err(D::Error::custom)
    }

    /// Construct a nonempty canonical prediction attachment.
    pub fn new(
        provenance_identity: PredictionProvenanceIdentityV1,
        mut facets: Vec<EnginePredictionFacetV1>,
    ) -> Result<Self, PredictionContractError> {
        if facets.is_empty() {
            return Err(PredictionContractError::EmptyFacetList);
        }
        if facets.len() > PREDICTION_V1_MAX_FACETS_PER_FILE {
            return Err(PredictionContractError::TooManyFacets {
                found: facets.len(),
                limit: PREDICTION_V1_MAX_FACETS_PER_FILE,
            });
        }
        facets.sort_by(|left, right| compare_scopes(&left.scope, &right.scope));
        if facets
            .windows(2)
            .any(|rows| compare_scopes(&rows[0].scope, &rows[1].scope).is_eq())
        {
            return Err(PredictionContractError::DuplicateFacetScope);
        }
        Ok(Self {
            schema: ENGINE_PREDICTION_V1_ID,
            provenance_identity,
            facets,
        })
    }

    /// Immutable schema identity.
    pub const fn contract_id(&self) -> &'static str {
        self.schema
    }

    /// Exact enclosing file provenance identity.
    pub const fn provenance_identity(&self) -> &PredictionProvenanceIdentityV1 {
        &self.provenance_identity
    }

    /// Canonically ordered scoped work units.
    pub fn facets(&self) -> &[EnginePredictionFacetV1] {
        &self.facets
    }

    /// Whether any required prediction work was unavailable.
    pub fn has_required_unavailable(&self) -> bool {
        self.facets
            .iter()
            .any(|facet| facet.state == EnginePredictionFacetStateV1::RequiredPredictionUnavailable)
    }

    /// Number of typed basis rows retained by this attachment.
    pub fn basis_reference_count(&self) -> usize {
        self.facets
            .iter()
            .map(|facet| facet.basis.references.len())
            .sum()
    }

    /// Total retained V1 attachment text for enclosing reader accounting.
    pub(crate) fn retained_text_bytes(&self) -> Result<usize, PredictionContractError> {
        self.facets.iter().try_fold(0usize, |total, facet| {
            total.checked_add(facet.retained_text_bytes()?).ok_or(
                PredictionContractError::ArithmeticOverflow("V1 prediction retained text"),
            )
        })
    }

    /// Cross-validate basis references against the exact embedded provenance.
    pub fn validate_against_provenance(
        &self,
        provenance: &PredictionProvenanceV1,
    ) -> Result<(), PredictionContractError> {
        if self.provenance_identity != provenance.identity {
            return Err(PredictionContractError::ProvenanceIdentityMismatch);
        }
        self.validate_structure()?;
        for reference in self
            .facets
            .iter()
            .flat_map(|facet| facet.basis.references.iter())
        {
            validate_basis_reference(reference, provenance)?;
        }
        Ok(())
    }

    /// Resolve and compare every measurement basis row after measurements-v15 validation.
    pub fn validate_measurement_references(
        &self,
        measurements: &MeasurementContract,
    ) -> Result<(), PredictionContractError> {
        validate_measurement_references_batch(measurements, [(0, self)])
            .map_err(|error| error.source)
    }

    pub(crate) fn validate_for_check(
        &self,
        _check_id: &'static str,
        evaluated_scopes: &[EvaluationScope],
        gaps: &[CoverageGap],
        findings: &[Finding],
    ) -> Result<(), PredictionContractError> {
        self.validate_structure()?;
        for facet in &self.facets {
            let evaluated = evaluated_scopes
                .iter()
                .filter(|scope| *scope == &facet.scope)
                .count();
            let is_gap = gaps
                .iter()
                .any(|gap| gap.scope.as_ref() == Some(&facet.scope));
            match facet.state {
                EnginePredictionFacetStateV1::Available if evaluated != 1 => {
                    return Err(PredictionContractError::AvailableScopeNotEvaluatedExactlyOnce);
                }
                EnginePredictionFacetStateV1::RequiredPredictionUnavailable => {
                    if evaluated != 0 {
                        return Err(PredictionContractError::UnavailableScopeEvaluated);
                    }
                    if is_gap {
                        return Err(PredictionContractError::UnavailableScopeDuplicatedAsGap);
                    }
                }
                EnginePredictionFacetStateV1::Available => {}
            }
        }
        for finding in findings {
            let Some(scope) = finding.prediction_scope.as_ref() else {
                return Err(PredictionContractError::FindingMissingPredictionScope);
            };
            let matches = self
                .facets
                .iter()
                .filter(|facet| {
                    &facet.scope == scope && facet.state == EnginePredictionFacetStateV1::Available
                })
                .count();
            if matches != 1 {
                return Err(PredictionContractError::FindingScopeNotAvailable);
            }
        }
        Ok(())
    }

    fn validate_structure(&self) -> Result<(), PredictionContractError> {
        if self.schema != ENGINE_PREDICTION_V1_ID {
            return Err(PredictionContractError::InvalidSchema {
                field: "prediction.schema",
                expected: ENGINE_PREDICTION_V1_ID,
                found: self.schema.to_owned(),
            });
        }
        if self.facets.is_empty() {
            return Err(PredictionContractError::EmptyFacetList);
        }
        if self.facets.len() > PREDICTION_V1_MAX_FACETS_PER_FILE {
            return Err(PredictionContractError::TooManyFacets {
                found: self.facets.len(),
                limit: PREDICTION_V1_MAX_FACETS_PER_FILE,
            });
        }
        for facet in &self.facets {
            validate_scope(&facet.scope)?;
            facet.validate()?;
        }
        if self
            .facets
            .windows(2)
            .any(|rows| !compare_scopes(&rows[0].scope, &rows[1].scope).is_lt())
        {
            return Err(PredictionContractError::NonCanonicalOrder("facets"));
        }
        Ok(())
    }
}

/// Stable V2 reason why a required prediction facet is unavailable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PredictionUnavailableReasonV2 {
    /// Loader-owned raw source inventory was partial or unavailable.
    RawSourceIncomplete,
    /// Resolved actual-clip settings retained only their bounded prefix.
    ResolvedSettingsOverflow,
    /// The shared file facet budget replaced omitted candidates with a summary.
    FacetBudgetExceeded,
    /// Same-load dependency closure was incomplete.
    DependencyClosureIncomplete,
    /// Profile data did not establish a required fact.
    ProfileFactUnknown,
    /// Project configuration did not establish a required intent.
    ProjectIntentUnavailable,
    /// Measurement evidence was unavailable.
    MeasurementUnavailable,
    /// A source selector matched no row.
    SourceSelectorNoMatch,
    /// A source selector matched multiple rows.
    SourceSelectorAmbiguous,
    /// Required primary-source evidence was unavailable.
    PrimarySourceUnavailable,
    /// Bounded namespaced extension reason retained from V1.
    Custom(String),
}

impl PredictionUnavailableReasonV2 {
    /// Construct a bounded namespaced extension reason.
    pub fn custom(value: impl Into<String>) -> Result<Self, PredictionContractError> {
        let value = bounded_string("unavailable reason", value)?;
        if !valid_custom_reason(&value) {
            return Err(PredictionContractError::InvalidUnavailableReasonCode(value));
        }
        Ok(Self::Custom(value))
    }

    /// Exact wire spelling.
    pub fn as_str(&self) -> &str {
        match self {
            Self::RawSourceIncomplete => "raw_source_incomplete",
            Self::ResolvedSettingsOverflow => "resolved_settings_overflow",
            Self::FacetBudgetExceeded => "facet_budget_exceeded",
            Self::DependencyClosureIncomplete => "dependency_closure_incomplete",
            Self::ProfileFactUnknown => "profile_fact_unknown",
            Self::ProjectIntentUnavailable => "project_intent_unavailable",
            Self::MeasurementUnavailable => "measurement_unavailable",
            Self::SourceSelectorNoMatch => "source_selector_no_match",
            Self::SourceSelectorAmbiguous => "source_selector_ambiguous",
            Self::PrimarySourceUnavailable => "primary_source_unavailable",
            Self::Custom(value) => value,
        }
    }

    fn from_wire(value: String) -> Result<Self, PredictionContractError> {
        let builtin = match value.as_str() {
            "raw_source_incomplete" => Some(Self::RawSourceIncomplete),
            "resolved_settings_overflow" => Some(Self::ResolvedSettingsOverflow),
            "facet_budget_exceeded" => Some(Self::FacetBudgetExceeded),
            "dependency_closure_incomplete" => Some(Self::DependencyClosureIncomplete),
            "profile_fact_unknown" => Some(Self::ProfileFactUnknown),
            "project_intent_unavailable" => Some(Self::ProjectIntentUnavailable),
            "measurement_unavailable" => Some(Self::MeasurementUnavailable),
            "source_selector_no_match" => Some(Self::SourceSelectorNoMatch),
            "source_selector_ambiguous" => Some(Self::SourceSelectorAmbiguous),
            "primary_source_unavailable" => Some(Self::PrimarySourceUnavailable),
            _ => None,
        };
        builtin.map_or_else(|| Self::custom(value), Ok)
    }
}

impl Serialize for PredictionUnavailableReasonV2 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for PredictionUnavailableReasonV2 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::from_wire(String::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

/// One independently scoped V2 engine-prediction work unit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EnginePredictionFacetV2 {
    scope: EvaluationScope,
    state: EnginePredictionFacetStateV1,
    basis: EnginePredictionBasisV1,
    reasons: Vec<PredictionUnavailableReasonV2>,
}

struct EnginePredictionFacetWireV2 {
    scope: EvaluationScope,
    state: EnginePredictionFacetStateV1,
    basis: EnginePredictionBasisWireV1,
    reasons: CappedSequence<String>,
}

struct EnginePredictionFacetSeedV2<'a> {
    references: &'a mut RowBudget,
}

impl<'de> DeserializeSeed<'de> for EnginePredictionFacetSeedV2<'_> {
    type Value = EnginePredictionFacetWireV2;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Field {
            Scope,
            State,
            Basis,
            Reasons,
        }
        struct VisitorV2<'a> {
            references: &'a mut RowBudget,
        }
        impl<'de> Visitor<'de> for VisitorV2<'_> {
            type Value = EnginePredictionFacetWireV2;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an engine prediction V2 facet")
            }
            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut scope = None;
                let mut state = None;
                let mut basis = None;
                let mut reasons = None;
                while let Some(field) = map.next_key()? {
                    match field {
                        Field::Scope => {
                            set_prediction_field(&mut scope, map.next_value()?, "scope")?
                        }
                        Field::State => {
                            set_prediction_field(&mut state, map.next_value()?, "state")?
                        }
                        Field::Basis => {
                            if basis.is_some() {
                                return Err(A::Error::duplicate_field("basis"));
                            }
                            basis = Some(map.next_value_seed(EnginePredictionBasisSeed {
                                references: self.references,
                            })?);
                        }
                        Field::Reasons => {
                            if reasons.is_some() {
                                return Err(A::Error::duplicate_field("reasons"));
                            }
                            reasons = Some(map.next_value_seed(CappedSequenceSeed {
                                limit: PREDICTION_V1_MAX_REASONS_PER_FACET,
                                element: PhantomData,
                            })?);
                        }
                    }
                }
                Ok(EnginePredictionFacetWireV2 {
                    scope: required_prediction_field(scope, "scope")?,
                    state: required_prediction_field(state, "state")?,
                    basis: required_prediction_field(basis, "basis")?,
                    reasons: required_prediction_field(reasons, "reasons")?,
                })
            }
        }
        deserializer.deserialize_struct(
            "EnginePredictionFacetV2",
            &["scope", "state", "basis", "reasons"],
            VisitorV2 {
                references: self.references,
            },
        )
    }
}

impl EnginePredictionFacetV2 {
    /// Construct one available facet with nonempty evidence.
    pub fn available(
        scope: EvaluationScope,
        basis: EnginePredictionBasisV1,
    ) -> Result<Self, PredictionContractError> {
        if basis.references().is_empty() {
            return Err(PredictionContractError::AvailableBasisEmpty);
        }
        Ok(Self {
            scope,
            state: EnginePredictionFacetStateV1::Available,
            basis,
            reasons: Vec::new(),
        })
    }

    /// Construct a required-unavailable facet with sorted distinct reasons.
    pub fn required_unavailable(
        scope: EvaluationScope,
        basis: EnginePredictionBasisV1,
        mut reasons: Vec<PredictionUnavailableReasonV2>,
    ) -> Result<Self, PredictionContractError> {
        reasons.sort_by(|left, right| left.as_str().cmp(right.as_str()));
        reasons.dedup();
        if reasons.is_empty() {
            return Err(PredictionContractError::RequiredUnavailableWithoutReason);
        }
        Ok(Self {
            scope,
            state: EnginePredictionFacetStateV1::RequiredPredictionUnavailable,
            basis,
            reasons,
        })
    }

    /// Existing check-evaluation work scope.
    pub const fn scope(&self) -> &EvaluationScope {
        &self.scope
    }

    /// Facet availability state.
    pub const fn state(&self) -> EnginePredictionFacetStateV1 {
        self.state
    }

    /// Canonical basis evidence.
    pub const fn basis(&self) -> &EnginePredictionBasisV1 {
        &self.basis
    }

    /// Sorted stable unavailable reasons.
    pub fn reasons(&self) -> &[PredictionUnavailableReasonV2] {
        &self.reasons
    }

    fn retained_text_bytes(&self) -> Result<usize, PredictionContractError> {
        let reason_text = checked_sum(
            "V2 facet reason retained text",
            self.reasons.iter().map(|reason| reason.as_str().len()),
        )?;
        checked_sum(
            "V2 facet retained text",
            [
                self.scope.code.as_str().len(),
                self.scope.subject.as_ref().map_or(0, String::len),
                reason_text,
                self.basis.retained_text_bytes()?,
            ],
        )
    }

    fn validate(&self) -> Result<(), PredictionContractError> {
        validate_scope(&self.scope)?;
        self.basis.validate()?;
        if self.reasons.len() > PREDICTION_V1_MAX_REASONS_PER_FACET {
            return Err(PredictionContractError::TooManyUnavailableReasons {
                found: self.reasons.len(),
                limit: PREDICTION_V1_MAX_REASONS_PER_FACET,
            });
        }
        if self
            .reasons
            .windows(2)
            .any(|pair| pair[0].as_str().as_bytes() >= pair[1].as_str().as_bytes())
        {
            return Err(PredictionContractError::NonCanonicalOrder(
                "V2 facet reasons",
            ));
        }
        match self.state {
            EnginePredictionFacetStateV1::Available if self.basis.references().is_empty() => {
                Err(PredictionContractError::AvailableBasisEmpty)
            }
            EnginePredictionFacetStateV1::Available if !self.reasons.is_empty() => {
                Err(PredictionContractError::AvailableHasReasons)
            }
            EnginePredictionFacetStateV1::RequiredPredictionUnavailable
                if self.reasons.is_empty() =>
            {
                Err(PredictionContractError::RequiredUnavailableWithoutReason)
            }
            _ => Ok(()),
        }
    }
}

impl TryFrom<EnginePredictionFacetWireV2> for EnginePredictionFacetV2 {
    type Error = PredictionContractError;

    fn try_from(wire: EnginePredictionFacetWireV2) -> Result<Self, Self::Error> {
        if wire.reasons.overflowed {
            return Err(PredictionContractError::TooManyUnavailableReasons {
                found: PREDICTION_V1_MAX_REASONS_PER_FACET + 1,
                limit: PREDICTION_V1_MAX_REASONS_PER_FACET,
            });
        }
        let reasons = wire
            .reasons
            .values
            .into_iter()
            .map(PredictionUnavailableReasonV2::from_wire)
            .collect::<Result<Vec<_>, _>>()?;
        if reasons
            .windows(2)
            .any(|pair| pair[0].as_str() >= pair[1].as_str())
        {
            return Err(PredictionContractError::NonCanonicalOrder(
                "V2 facet reasons",
            ));
        }
        let basis: EnginePredictionBasisV1 = wire.basis.try_into()?;
        validate_scope(&wire.scope)?;
        basis.validate()?;
        match wire.state {
            EnginePredictionFacetStateV1::Available if basis.references().is_empty() => {
                Err(PredictionContractError::AvailableBasisEmpty)
            }
            EnginePredictionFacetStateV1::Available if !reasons.is_empty() => {
                Err(PredictionContractError::AvailableHasReasons)
            }
            EnginePredictionFacetStateV1::RequiredPredictionUnavailable if reasons.is_empty() => {
                Err(PredictionContractError::RequiredUnavailableWithoutReason)
            }
            _ => Ok(Self {
                scope: wire.scope,
                state: wire.state,
                basis,
                reasons,
            }),
        }
    }
}

impl<'de> Deserialize<'de> for EnginePredictionFacetV2 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut references = RowBudget::new(usize::MAX);
        Self::try_from(
            EnginePredictionFacetSeedV2 {
                references: &mut references,
            }
            .deserialize(deserializer)?,
        )
        .map_err(D::Error::custom)
    }
}

/// Per-check bounded-overflow engine prediction attachment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EnginePredictionV2 {
    schema: &'static str,
    provenance_identity: PredictionProvenanceIdentityV2,
    facets: Vec<EnginePredictionFacetV2>,
}

struct EnginePredictionWireV2 {
    schema: String,
    provenance_identity: PredictionProvenanceIdentityV2,
    facets: CappedSequence<EnginePredictionFacetWireV2>,
    facet_budget: RowBudget,
    reference_budget: RowBudget,
}

enum FacetElementV2 {
    Value(EnginePredictionFacetWireV2),
    Skipped,
}
struct FacetElementSeedV2<'a> {
    facets: &'a mut RowBudget,
    references: &'a mut RowBudget,
}
impl<'de> DeserializeSeed<'de> for FacetElementSeedV2<'_> {
    type Value = FacetElementV2;
    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        if self.facets.admit() {
            EnginePredictionFacetSeedV2 {
                references: self.references,
            }
            .deserialize(deserializer)
            .map(FacetElementV2::Value)
        } else {
            IgnoredAny::deserialize(deserializer).map(|_| FacetElementV2::Skipped)
        }
    }
}
struct FacetsSeedV2<'a> {
    facets: &'a mut RowBudget,
    references: &'a mut RowBudget,
}
impl<'de> DeserializeSeed<'de> for FacetsSeedV2<'_> {
    type Value = CappedSequence<EnginePredictionFacetWireV2>;
    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct VisitorV2<'a> {
            facets: &'a mut RowBudget,
            references: &'a mut RowBudget,
        }
        impl<'de> Visitor<'de> for VisitorV2<'_> {
            type Value = CappedSequence<EnginePredictionFacetWireV2>;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a bounded sequence of engine prediction V2 facets")
            }
            fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut values = Vec::with_capacity(
                    sequence
                        .size_hint()
                        .unwrap_or(0)
                        .min(PREDICTION_V1_MAX_FACETS_PER_FILE),
                );
                let mut seen = 0usize;
                while seen < PREDICTION_V1_MAX_FACETS_PER_FILE {
                    let Some(element) = sequence.next_element_seed(FacetElementSeedV2 {
                        facets: self.facets,
                        references: self.references,
                    })?
                    else {
                        return Ok(CappedSequence {
                            values,
                            overflowed: false,
                        });
                    };
                    seen += 1;
                    match element {
                        FacetElementV2::Value(value) => values.push(value),
                        FacetElementV2::Skipped => {
                            return Ok(CappedSequence {
                                values,
                                overflowed: consume_ignored_tail(
                                    &mut sequence,
                                    seen,
                                    PREDICTION_V1_MAX_FACETS_PER_FILE,
                                )?,
                            });
                        }
                    }
                }
                Ok(CappedSequence {
                    values,
                    overflowed: consume_ignored_tail(
                        &mut sequence,
                        seen,
                        PREDICTION_V1_MAX_FACETS_PER_FILE,
                    )?,
                })
            }
        }
        deserializer.deserialize_seq(VisitorV2 {
            facets: self.facets,
            references: self.references,
        })
    }
}
struct EnginePredictionWireSeedV2 {
    facet_limit: usize,
    reference_limit: usize,
}
impl<'de> DeserializeSeed<'de> for EnginePredictionWireSeedV2 {
    type Value = EnginePredictionWireV2;
    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Field {
            Schema,
            ProvenanceIdentity,
            Facets,
        }
        struct VisitorV2 {
            facet_limit: usize,
            reference_limit: usize,
        }
        impl<'de> Visitor<'de> for VisitorV2 {
            type Value = EnginePredictionWireV2;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an engine prediction V2")
            }
            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut facet_budget = RowBudget::new(self.facet_limit);
                let mut reference_budget = RowBudget::new(self.reference_limit);
                let mut schema = None;
                let mut provenance_identity = None;
                let mut facets = None;
                while let Some(field) = map.next_key()? {
                    match field {
                        Field::Schema => {
                            set_prediction_field(&mut schema, map.next_value()?, "schema")?
                        }
                        Field::ProvenanceIdentity => set_prediction_field(
                            &mut provenance_identity,
                            map.next_value()?,
                            "provenance_identity",
                        )?,
                        Field::Facets => {
                            if facets.is_some() {
                                return Err(A::Error::duplicate_field("facets"));
                            }
                            facets = Some(map.next_value_seed(FacetsSeedV2 {
                                facets: &mut facet_budget,
                                references: &mut reference_budget,
                            })?);
                        }
                    }
                }
                Ok(EnginePredictionWireV2 {
                    schema: required_prediction_field(schema, "schema")?,
                    provenance_identity: required_prediction_field(
                        provenance_identity,
                        "provenance_identity",
                    )?,
                    facets: required_prediction_field(facets, "facets")?,
                    facet_budget,
                    reference_budget,
                })
            }
        }
        deserializer.deserialize_struct(
            "EnginePredictionV2",
            &["schema", "provenance_identity", "facets"],
            VisitorV2 {
                facet_limit: self.facet_limit,
                reference_limit: self.reference_limit,
            },
        )
    }
}
impl<'de> Deserialize<'de> for EnginePredictionWireV2 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        EnginePredictionWireSeedV2 {
            facet_limit: PREDICTION_V1_MAX_FACETS_PER_FILE,
            reference_limit: usize::MAX,
        }
        .deserialize(deserializer)
    }
}

impl EnginePredictionV2 {
    fn from_wire(wire: EnginePredictionWireV2) -> Result<Self, PredictionContractError> {
        if wire.schema != ENGINE_PREDICTION_V2_ID {
            return Err(PredictionContractError::InvalidSchema {
                field: "prediction.schema",
                expected: ENGINE_PREDICTION_V2_ID,
                found: wire.schema,
            });
        }
        if wire.facets.overflowed {
            return Err(PredictionContractError::TooManyFacets {
                found: PREDICTION_V1_MAX_FACETS_PER_FILE + 1,
                limit: PREDICTION_V1_MAX_FACETS_PER_FILE,
            });
        }
        if let Some(error) = Self::first_nested_limit_error(&wire) {
            return Err(error);
        }
        Self::new(
            wire.provenance_identity,
            wire.facets
                .values
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_, _>>()?,
        )
    }

    fn first_nested_limit_error(wire: &EnginePredictionWireV2) -> Option<PredictionContractError> {
        for facet in &wire.facets.values {
            if facet.reasons.overflowed {
                return Some(PredictionContractError::TooManyUnavailableReasons {
                    found: PREDICTION_V1_MAX_REASONS_PER_FACET + 1,
                    limit: PREDICTION_V1_MAX_REASONS_PER_FACET,
                });
            }
            if facet.basis.references.overflowed {
                return Some(PredictionContractError::TooManyBasisReferences {
                    found: PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FACET + 1,
                    limit: PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FACET,
                });
            }
        }
        None
    }

    /// Construct one V2 prediction with canonical unique facet scopes.
    pub fn new(
        provenance_identity: PredictionProvenanceIdentityV2,
        mut facets: Vec<EnginePredictionFacetV2>,
    ) -> Result<Self, PredictionContractError> {
        if facets.is_empty() {
            return Err(PredictionContractError::EmptyFacetList);
        }
        if facets.len() > PREDICTION_V1_MAX_FACETS_PER_FILE {
            return Err(PredictionContractError::TooManyFacets {
                found: facets.len(),
                limit: PREDICTION_V1_MAX_FACETS_PER_FILE,
            });
        }
        facets.sort_by(|left, right| compare_scopes(left.scope(), right.scope()));
        if facets
            .windows(2)
            .any(|pair| compare_scopes(pair[0].scope(), pair[1].scope()) == Ordering::Equal)
        {
            return Err(PredictionContractError::DuplicateFacetScope);
        }
        Ok(Self {
            schema: ENGINE_PREDICTION_V2_ID,
            provenance_identity,
            facets,
        })
    }

    /// Immutable schema identity.
    pub const fn contract_id(&self) -> &'static str {
        self.schema
    }

    /// V2 file provenance identity.
    pub const fn provenance_identity(&self) -> &PredictionProvenanceIdentityV2 {
        &self.provenance_identity
    }

    /// Canonically ordered facets.
    pub fn facets(&self) -> &[EnginePredictionFacetV2] {
        &self.facets
    }

    /// Whether any required work is unavailable.
    pub fn has_required_unavailable(&self) -> bool {
        self.facets
            .iter()
            .any(|facet| facet.state == EnginePredictionFacetStateV1::RequiredPredictionUnavailable)
    }

    /// Number of typed basis rows retained by this attachment.
    pub fn basis_reference_count(&self) -> usize {
        self.facets
            .iter()
            .map(|facet| facet.basis.references().len())
            .sum()
    }

    pub(crate) fn retained_text_bytes(&self) -> Result<usize, PredictionContractError> {
        self.facets.iter().try_fold(0usize, |total, facet| {
            total.checked_add(facet.retained_text_bytes()?).ok_or(
                PredictionContractError::ArithmeticOverflow("V2 prediction retained text"),
            )
        })
    }

    /// Cross-validate every basis reference against this V2 provenance.
    pub fn validate_against_provenance(
        &self,
        provenance: &PredictionProvenanceV2,
    ) -> Result<(), PredictionContractError> {
        if self.provenance_identity != provenance.identity {
            return Err(PredictionContractError::ProvenanceIdentityMismatch);
        }
        self.validate_structure()?;
        for reference in self
            .facets
            .iter()
            .flat_map(|facet| facet.basis.references())
        {
            validate_basis_reference_v2(reference, provenance)?;
        }
        Ok(())
    }

    pub(crate) fn validate_for_check(
        &self,
        check_id: &str,
        evaluated_scopes: &[EvaluationScope],
        gaps: &[CoverageGap],
        findings: &[Finding],
    ) -> Result<(), PredictionContractError> {
        self.validate_structure()?;
        self.validate_facet_budget_summary_for_check(check_id)?;
        for facet in &self.facets {
            let evaluated = evaluated_scopes
                .iter()
                .filter(|scope| *scope == &facet.scope)
                .count();
            let is_gap = gaps
                .iter()
                .any(|gap| gap.scope.as_ref() == Some(&facet.scope));
            match facet.state {
                EnginePredictionFacetStateV1::Available if evaluated != 1 => {
                    return Err(PredictionContractError::AvailableScopeNotEvaluatedExactlyOnce);
                }
                EnginePredictionFacetStateV1::RequiredPredictionUnavailable => {
                    if evaluated != 0 {
                        return Err(PredictionContractError::UnavailableScopeEvaluated);
                    }
                    if is_gap {
                        return Err(PredictionContractError::UnavailableScopeDuplicatedAsGap);
                    }
                }
                EnginePredictionFacetStateV1::Available => {}
            }
        }
        for finding in findings {
            let Some(scope) = finding.prediction_scope.as_ref() else {
                return Err(PredictionContractError::FindingMissingPredictionScope);
            };
            if self
                .facets
                .iter()
                .filter(|facet| {
                    &facet.scope == scope && facet.state == EnginePredictionFacetStateV1::Available
                })
                .count()
                != 1
            {
                return Err(PredictionContractError::FindingScopeNotAvailable);
            }
        }
        Ok(())
    }

    /// Whether this attachment contains its one canonical file-budget summary.
    pub(crate) fn has_facet_budget_summary(&self) -> bool {
        self.facets
            .iter()
            .any(|facet| facet.reasons == [PredictionUnavailableReasonV2::FacetBudgetExceeded])
    }

    /// Enforce the check-scoped shape of a shared-file budget summary without
    /// requiring producer-only finding values.  The staged output reader uses
    /// this before it validates its separate lifecycle representation.
    pub(crate) fn validate_facet_budget_summary_for_check(
        &self,
        check_id: &str,
    ) -> Result<(), PredictionContractError> {
        let expected_budget_scope = format!("{check_id}:facet-budget");
        let mut budget_summaries = 0usize;
        for facet in &self.facets {
            if facet
                .reasons
                .contains(&PredictionUnavailableReasonV2::FacetBudgetExceeded)
            {
                if facet.state != EnginePredictionFacetStateV1::RequiredPredictionUnavailable
                    || facet.scope.subject.is_some()
                    || facet.scope.code.as_str() != expected_budget_scope
                    || facet.reasons != [PredictionUnavailableReasonV2::FacetBudgetExceeded]
                {
                    return Err(PredictionContractError::InvalidFacetBudgetSummary);
                }
                budget_summaries += 1;
                if budget_summaries > 1 {
                    return Err(PredictionContractError::DuplicateFacetBudgetSummary);
                }
            }
        }
        Ok(())
    }

    fn validate_structure(&self) -> Result<(), PredictionContractError> {
        if self.schema != ENGINE_PREDICTION_V2_ID {
            return Err(PredictionContractError::InvalidSchema {
                field: "prediction.schema",
                expected: ENGINE_PREDICTION_V2_ID,
                found: self.schema.to_owned(),
            });
        }
        if self.facets.is_empty() {
            return Err(PredictionContractError::EmptyFacetList);
        }
        if self.facets.len() > PREDICTION_V1_MAX_FACETS_PER_FILE {
            return Err(PredictionContractError::TooManyFacets {
                found: self.facets.len(),
                limit: PREDICTION_V1_MAX_FACETS_PER_FILE,
            });
        }
        for facet in &self.facets {
            facet.validate()?;
        }
        if self
            .facets
            .windows(2)
            .any(|pair| !compare_scopes(pair[0].scope(), pair[1].scope()).is_lt())
        {
            return Err(PredictionContractError::NonCanonicalOrder("V2 facets"));
        }
        Ok(())
    }
}

impl<'de> Deserialize<'de> for EnginePredictionV2 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = EnginePredictionWireV2::deserialize(deserializer)?;
        Self::from_wire(wire).map_err(D::Error::custom)
    }
}

/// Decode a V2 prediction with aggregate file budgets without retaining a
/// prefix after either budget is exhausted.
pub(crate) fn decode_engine_prediction_v2(
    raw: &str,
    facet_limit: usize,
    reference_limit: usize,
) -> Result<EnginePredictionV2, PredictionDecodeError> {
    let mut deserializer = serde_json::Deserializer::from_str(raw);
    let wire = EnginePredictionWireSeedV2 {
        facet_limit,
        reference_limit,
    }
    .deserialize(&mut deserializer)
    .map_err(PredictionDecodeError::Shape)?;
    deserializer.end().map_err(PredictionDecodeError::Shape)?;
    if wire.facet_budget.overflowed() {
        return Err(PredictionDecodeError::TooManyFileFacets);
    }
    if wire.reference_budget.overflowed() {
        return Err(PredictionDecodeError::TooManyFileBasisReferences);
    }
    EnginePredictionV2::from_wire(wire).map_err(PredictionDecodeError::Semantic)
}

/// Domain-separated identity of one complete prediction-provenance header.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PredictionProvenanceIdentityV1(InputIdentity);

impl PredictionProvenanceIdentityV1 {
    /// SHA-256 and canonical-preimage byte count.
    pub const fn input_identity(&self) -> &InputIdentity {
        &self.0
    }

    #[cfg(test)]
    pub(crate) fn from_input_identity(identity: InputIdentity) -> Self {
        Self(identity)
    }
}

/// File-scoped immutable evidence shared by every engine prediction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PredictionProvenanceV1 {
    schema: &'static str,
    identity: PredictionProvenanceIdentityV1,
    profile: ResolvedEngineProfileV1,
    #[serde(serialize_with = "serialize_source_format")]
    source_format: SourceFormatV1,
    settings: ResolvedEngineSettingsV1,
    raw_source: RawSourceBindingV1,
    dependency_closure: DependencyClosureV1,
    consumed_contracts: [&'static str; 5],
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StagedPredictionProvenanceWireV1 {
    schema: String,
    identity: PredictionProvenanceIdentityV1,
    profile: Box<RawValue>,
    source_format: SourceFormatV1,
    settings: Box<RawValue>,
    raw_source: Box<RawValue>,
    dependency_closure: Box<RawValue>,
    #[serde(deserialize_with = "deserialize_consumed_contracts")]
    consumed_contracts: CappedSequence<String>,
}

impl PredictionProvenanceV1 {
    fn validate_capped_wire_header(
        schema: &str,
        consumed_contracts: &CappedSequence<String>,
    ) -> Result<(), PredictionContractError> {
        if consumed_contracts.overflowed {
            return Err(PredictionContractError::InvalidConsumedContracts);
        }
        Self::validate_wire_header(schema, &consumed_contracts.values)
    }

    fn validate_wire_header(
        schema: &str,
        consumed_contracts: &[String],
    ) -> Result<(), PredictionContractError> {
        if schema != PREDICTION_PROVENANCE_V1_ID {
            return Err(PredictionContractError::InvalidSchema {
                field: "provenance.schema",
                expected: PREDICTION_PROVENANCE_V1_ID,
                found: schema.to_owned(),
            });
        }
        if consumed_contracts.len() != CONSUMED_CONTRACTS_V1.len()
            || !consumed_contracts
                .iter()
                .zip(CONSUMED_CONTRACTS_V1)
                .all(|(found, expected)| found == expected)
        {
            return Err(PredictionContractError::InvalidConsumedContracts);
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn from_wire_parts(
        schema: String,
        identity: PredictionProvenanceIdentityV1,
        profile: ResolvedEngineProfileV1,
        source_format: SourceFormatV1,
        settings: ResolvedEngineSettingsV1,
        raw_source: RawSourceBindingV1,
        dependency_closure: DependencyClosureV1,
        consumed_contracts: Vec<String>,
    ) -> Result<Self, PredictionContractError> {
        Self::validate_wire_header(&schema, &consumed_contracts)?;
        let provenance = Self {
            schema: PREDICTION_PROVENANCE_V1_ID,
            identity,
            profile,
            source_format,
            settings,
            raw_source,
            dependency_closure,
            consumed_contracts: CONSUMED_CONTRACTS_V1,
        };
        provenance.validate()?;
        Ok(provenance)
    }
}

#[allow(
    dead_code,
    reason = "V1 standalone deserialization remains an explicit historical API"
)]
pub(crate) fn decode_prediction_provenance_v1(
    raw: &str,
) -> Result<PredictionProvenanceV1, PredictionDecodeError> {
    let wire: StagedPredictionProvenanceWireV1 =
        serde_json::from_str(raw).map_err(PredictionDecodeError::Shape)?;
    decode_prediction_provenance_wire(wire)
}

fn decode_prediction_provenance_wire(
    wire: StagedPredictionProvenanceWireV1,
) -> Result<PredictionProvenanceV1, PredictionDecodeError> {
    PredictionProvenanceV1::validate_capped_wire_header(&wire.schema, &wire.consumed_contracts)
        .map_err(PredictionDecodeError::Semantic)?;
    let raw_source_result = serde_json::from_str::<RawSourceBindingWireV1>(wire.raw_source.get())
        .map_err(PredictionDecodeError::Shape)
        .and_then(|raw| {
            RawSourceBindingV1::from_wire(raw).map_err(PredictionDecodeError::Semantic)
        });
    let reserved_raw_rows = match raw_source_result.as_ref() {
        Ok(raw) => usize::try_from(raw.work.retained_rows).map_err(|_| {
            PredictionDecodeError::Semantic(PredictionContractError::ArithmeticOverflow(
                "raw-source rows",
            ))
        })?,
        Err(_) => 0,
    };
    let remaining_after_raw =
        PREDICTION_V1_MAX_AGGREGATE_PROVENANCE_ROWS.saturating_sub(reserved_raw_rows);
    let profile = decode_resolved_engine_profile_v1_with_provenance_limit(
        wire.profile.get(),
        remaining_after_raw,
    )
    .map_err(|error| match error {
        EngineProfileLimitedDecodeError::Contract(EngineContractDecodeError::Shape(source)) => {
            PredictionDecodeError::Shape(source)
        }
        EngineProfileLimitedDecodeError::Contract(EngineContractDecodeError::Semantic(source)) => {
            PredictionDecodeError::Semantic(source.into())
        }
        EngineProfileLimitedDecodeError::ProvenanceRowsOverflow => PredictionDecodeError::Semantic(
            PredictionContractError::TooManyAggregateProvenanceRows {
                found: PREDICTION_V1_MAX_AGGREGATE_PROVENANCE_ROWS + 1,
                limit: PREDICTION_V1_MAX_AGGREGATE_PROVENANCE_ROWS,
            },
        ),
    })?;
    let remaining_provenance_rows = remaining_after_raw.saturating_sub(profile.provenance_rows());
    let settings = decode_resolved_engine_settings_v1_with_provenance_limit(
        wire.settings.get(),
        remaining_provenance_rows,
    )
    .map_err(|error| match error {
        EngineSettingsLimitedDecodeError::Contract(EngineContractDecodeError::Shape(source)) => {
            PredictionDecodeError::Shape(source)
        }
        EngineSettingsLimitedDecodeError::Contract(EngineContractDecodeError::Semantic(source)) => {
            PredictionDecodeError::Semantic(source.into())
        }
        EngineSettingsLimitedDecodeError::ProvenanceRowsOverflow => {
            PredictionDecodeError::Semantic(
                PredictionContractError::TooManyAggregateProvenanceRows {
                    found: PREDICTION_V1_MAX_AGGREGATE_PROVENANCE_ROWS + 1,
                    limit: PREDICTION_V1_MAX_AGGREGATE_PROVENANCE_ROWS,
                },
            )
        }
    })?;
    let raw_source = raw_source_result?;
    let dependency_closure = decode_dependency_closure_v1(wire.dependency_closure.get()).map_err(
        |error| match error {
            DependencyClosureDecodeError::Shape(source) => PredictionDecodeError::Shape(source),
            DependencyClosureDecodeError::Semantic(reason) => PredictionDecodeError::Semantic(
                PredictionContractError::InvalidDependencyClosure(reason),
            ),
        },
    )?;
    PredictionProvenanceV1::from_wire_parts(
        wire.schema,
        wire.identity,
        profile,
        wire.source_format,
        settings,
        raw_source,
        dependency_closure,
        wire.consumed_contracts.values,
    )
    .map_err(PredictionDecodeError::Semantic)
}

impl<'de> Deserialize<'de> for PredictionProvenanceV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        decode_prediction_provenance_wire(StagedPredictionProvenanceWireV1::deserialize(
            deserializer,
        )?)
        .map_err(|error| match error {
            PredictionDecodeError::Shape(source) => D::Error::custom(source),
            PredictionDecodeError::Semantic(source) => D::Error::custom(source),
            PredictionDecodeError::TooManyFileFacets
            | PredictionDecodeError::TooManyFileBasisReferences => {
                unreachable!("provenance decoding cannot consume prediction budgets")
            }
        })
    }
}

impl PredictionProvenanceV1 {
    /// Bind one exact resolved profile to same-load source and closure evidence.
    pub fn new(
        profile: ResolvedEngineProfileV1,
        source_format: SourceFormatV1,
        settings: ResolvedEngineSettingsV1,
        raw_source: RawSourceBindingV1,
        dependency_closure: DependencyClosureV1,
    ) -> Result<Self, PredictionContractError> {
        profile.validate()?;
        settings.validate_against(&profile)?;
        if source_format != raw_source.source_format {
            return Err(PredictionContractError::SourceFormatMismatch);
        }
        if !profile.accepts_format(source_format) {
            return Err(PredictionContractError::SourceFormatNotAccepted);
        }
        if raw_source.primary_input != *dependency_closure.primary_input() {
            return Err(PredictionContractError::PrimaryInputMismatch);
        }
        let mut provenance = Self {
            schema: PREDICTION_PROVENANCE_V1_ID,
            identity: PredictionProvenanceIdentityV1(InputIdentity::from_bytes(&[])),
            profile,
            source_format,
            settings,
            raw_source,
            dependency_closure,
            consumed_contracts: CONSUMED_CONTRACTS_V1,
        };
        provenance.validate_without_identity()?;
        provenance.identity = PredictionProvenanceIdentityV1(provenance.computed_identity());
        Ok(provenance)
    }

    /// Immutable schema identity.
    pub const fn contract_id(&self) -> &'static str {
        self.schema
    }

    /// Canonical identity over every semantic field.
    pub const fn identity(&self) -> &PredictionProvenanceIdentityV1 {
        &self.identity
    }

    /// Exact embedded resolved profile.
    pub const fn profile(&self) -> &ResolvedEngineProfileV1 {
        &self.profile
    }

    /// Authoritative source format used during resolution.
    pub const fn source_format(&self) -> SourceFormatV1 {
        self.source_format
    }

    /// Fully materialized settings.
    pub const fn settings(&self) -> &ResolvedEngineSettingsV1 {
        &self.settings
    }

    /// Same-load raw-source header evidence.
    pub const fn raw_source(&self) -> &RawSourceBindingV1 {
        &self.raw_source
    }

    /// Complete serialized dependency-closure evidence.
    pub const fn dependency_closure(&self) -> &DependencyClosureV1 {
        &self.dependency_closure
    }

    /// Exact derived consumed-contract inventory.
    pub const fn consumed_contracts(&self) -> &[&'static str; 5] {
        &self.consumed_contracts
    }

    /// Validate schema, cross-links, bounds, and canonical identity.
    pub fn validate(&self) -> Result<(), PredictionContractError> {
        self.validate_without_identity()?;
        if self.identity.0 != self.computed_identity() {
            return Err(PredictionContractError::IdentityMismatch {
                contract: PREDICTION_PROVENANCE_V1_ID,
            });
        }
        Ok(())
    }

    pub(crate) fn retained_text_bytes(&self) -> Result<usize, PredictionContractError> {
        let closure_text = checked_sum(
            "closure retained text",
            self.dependency_closure
                .references()
                .iter()
                .filter_map(|reference| closure_target_key(reference.target()).map(str::len))
                .chain(
                    self.dependency_closure
                        .external_resources()
                        .iter()
                        .map(|resource| resource.key().as_str().len()),
                ),
        )?;
        checked_sum(
            "provenance retained text",
            [
                self.profile.retained_text_bytes()?,
                self.settings.retained_text_bytes()?,
                self.raw_source.retained_text_bytes()?,
                closure_text,
            ],
        )
    }

    fn retained_provenance_rows(&self) -> Result<usize, PredictionContractError> {
        let clip_settings = checked_sum(
            "clip setting rows",
            self.settings
                .clips()
                .iter()
                .map(|clip| clip.settings().len()),
        )?;
        let raw_rows = usize::try_from(self.raw_source.work.retained_rows)
            .map_err(|_| PredictionContractError::ArithmeticOverflow("raw-source rows"))?;
        checked_sum(
            "aggregate provenance rows",
            [
                self.profile.facts().len(),
                self.profile.setting_descriptors().len(),
                self.profile.primary_sources().len(),
                self.settings.document_settings().len(),
                clip_settings,
                raw_rows,
            ],
        )
    }

    fn validate_without_identity(&self) -> Result<(), PredictionContractError> {
        if self.schema != PREDICTION_PROVENANCE_V1_ID {
            return Err(PredictionContractError::InvalidSchema {
                field: "provenance.schema",
                expected: PREDICTION_PROVENANCE_V1_ID,
                found: self.schema.to_owned(),
            });
        }
        self.profile.validate()?;
        self.settings.validate_against(&self.profile)?;
        if self.source_format != self.raw_source.source_format {
            return Err(PredictionContractError::SourceFormatMismatch);
        }
        if !self.profile.accepts_format(self.source_format) {
            return Err(PredictionContractError::SourceFormatNotAccepted);
        }
        if self.raw_source.schema != RAW_SOURCE_FACTS_V1_ID {
            return Err(PredictionContractError::InvalidSchema {
                field: "provenance.raw_source.schema",
                expected: RAW_SOURCE_FACTS_V1_ID,
                found: self.raw_source.schema.to_owned(),
            });
        }
        if self.raw_source.primary_input != *self.dependency_closure.primary_input() {
            return Err(PredictionContractError::PrimaryInputMismatch);
        }
        let closure_reasons = self.dependency_closure.coverage().reasons();
        let source_reason_matches = match self.raw_source.resources_coverage.state {
            RawSourceSetCoverageStateV1::Complete => {
                !closure_reasons
                    .contains(&DependencyClosureCoverageReasonV1::SourceDeclarationsPartial)
                    && !closure_reasons
                        .contains(&DependencyClosureCoverageReasonV1::SourceDeclarationsUnavailable)
            }
            RawSourceSetCoverageStateV1::Partial => {
                closure_reasons
                    .contains(&DependencyClosureCoverageReasonV1::SourceDeclarationsPartial)
                    && !closure_reasons
                        .contains(&DependencyClosureCoverageReasonV1::SourceDeclarationsUnavailable)
            }
            RawSourceSetCoverageStateV1::Unavailable => {
                closure_reasons
                    .contains(&DependencyClosureCoverageReasonV1::SourceDeclarationsUnavailable)
                    && !closure_reasons
                        .contains(&DependencyClosureCoverageReasonV1::SourceDeclarationsPartial)
                    && self.dependency_closure.references().is_empty()
                    && matches!(
                        self.dependency_closure.coverage(),
                        DependencyClosureCoverageV1::Unavailable { .. }
                    )
            }
        };
        if !source_reason_matches {
            return Err(PredictionContractError::DependencyClosureCoverageMismatch);
        }
        if self.consumed_contracts != CONSUMED_CONTRACTS_V1 {
            return Err(PredictionContractError::InvalidConsumedContracts);
        }
        let rows = self.retained_provenance_rows()?;
        if rows > PREDICTION_V1_MAX_AGGREGATE_PROVENANCE_ROWS {
            return Err(PredictionContractError::TooManyAggregateProvenanceRows {
                found: rows,
                limit: PREDICTION_V1_MAX_AGGREGATE_PROVENANCE_ROWS,
            });
        }
        let text = self.retained_text_bytes()?;
        if text > PREDICTION_V1_MAX_TOTAL_TEXT_BYTES_PER_FILE {
            return Err(PredictionContractError::TooMuchRetainedText {
                found: text,
                limit: PREDICTION_V1_MAX_TOTAL_TEXT_BYTES_PER_FILE,
            });
        }
        Ok(())
    }

    fn computed_identity(&self) -> InputIdentity {
        let mut encoder = CanonicalEncoder::new("animsmith-prediction-provenance-v1");
        encoder.field("schema");
        encoder.token(self.schema);
        encoder.field("profile");
        self.profile.encode_preimage(&mut encoder);
        encoder.field("source_format");
        encoder.token(source_format_name(self.source_format));
        encoder.field("settings");
        self.settings.encode_preimage(&self.profile, &mut encoder);
        encoder.field("raw_source");
        encode_raw_binding(&mut encoder, &self.raw_source);
        encoder.field("dependency_closure");
        encode_dependency_closure(&mut encoder, &self.dependency_closure);
        encoder.field("consumed_contracts");
        encoder.count(self.consumed_contracts.len());
        for contract in self.consumed_contracts {
            encoder.token(contract);
        }
        encoder.identity()
    }
}

/// Domain-separated identity of a complete V2 prediction-provenance record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PredictionProvenanceIdentityV2(InputIdentity);

impl PredictionProvenanceIdentityV2 {
    /// SHA-256 and canonical-preimage byte count.
    pub const fn input_identity(&self) -> &InputIdentity {
        &self.0
    }
}

const CONSUMED_CONTRACTS_V2: [&str; 6] = [
    "urn:animsmith:schema:output:12",
    MEASUREMENTS_SCHEMA_ID,
    RAW_SOURCE_FACTS_V1_ID,
    DEPENDENCY_CLOSURE_V1_ID,
    ENGINE_PROFILE_FACTS_V1_ID,
    "urn:animsmith:resolved-engine-settings:2",
];

/// File-scoped immutable V2 evidence shared by bounded-overflow predictions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PredictionProvenanceV2 {
    schema: &'static str,
    identity: PredictionProvenanceIdentityV2,
    profile: ResolvedEngineProfileV1,
    source_format: SourceFormatV1,
    settings: ResolvedEngineSettingsV2,
    raw_source: RawSourceBindingV1,
    dependency_closure: DependencyClosureV1,
    consumed_contracts: [&'static str; 6],
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PredictionProvenanceWireV2 {
    schema: String,
    identity: PredictionProvenanceIdentityV2,
    profile: Box<RawValue>,
    source_format: SourceFormatV1,
    settings: Box<RawValue>,
    raw_source: Box<RawValue>,
    dependency_closure: Box<RawValue>,
    #[serde(deserialize_with = "deserialize_consumed_contracts_v2")]
    consumed_contracts: CappedSequence<String>,
}

fn deserialize_consumed_contracts_v2<'de, D>(
    deserializer: D,
) -> Result<CappedSequence<String>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_capped_sequence(deserializer, CONSUMED_CONTRACTS_V2.len())
}

impl PredictionProvenanceV2 {
    /// Bind V2 settings to same-load source and closure evidence.
    pub fn new(
        profile: ResolvedEngineProfileV1,
        source_format: SourceFormatV1,
        settings: ResolvedEngineSettingsV2,
        raw_source: RawSourceBindingV1,
        dependency_closure: DependencyClosureV1,
    ) -> Result<Self, PredictionContractError> {
        // The immutable V1 construction supplies the established profile/raw/
        // closure cross-link validation without changing its artifact shape.
        let prefix = settings.validation_only_prefix(&profile)?;
        PredictionProvenanceV1::new(
            profile.clone(),
            source_format,
            prefix,
            raw_source.clone(),
            dependency_closure.clone(),
        )?;
        settings.validate_against(&profile)?;
        let mut provenance = Self {
            schema: PREDICTION_PROVENANCE_V2_ID,
            identity: PredictionProvenanceIdentityV2(InputIdentity::from_bytes(&[])),
            profile,
            source_format,
            settings,
            raw_source,
            dependency_closure,
            consumed_contracts: CONSUMED_CONTRACTS_V2,
        };
        provenance.identity = PredictionProvenanceIdentityV2(provenance.computed_identity());
        Ok(provenance)
    }

    /// Immutable V2 schema identity.
    pub const fn contract_id(&self) -> &'static str {
        self.schema
    }

    /// Canonical V2 identity.
    pub const fn identity(&self) -> &PredictionProvenanceIdentityV2 {
        &self.identity
    }

    /// Exact embedded profile.
    pub const fn profile(&self) -> &ResolvedEngineProfileV1 {
        &self.profile
    }

    /// Authoritative resolved source format.
    pub const fn source_format(&self) -> SourceFormatV1 {
        self.source_format
    }

    /// Explicitly complete or partial resolved settings.
    pub const fn settings(&self) -> &ResolvedEngineSettingsV2 {
        &self.settings
    }

    /// Same-load raw source evidence.
    pub const fn raw_source(&self) -> &RawSourceBindingV1 {
        &self.raw_source
    }

    /// Same-load dependency closure evidence.
    pub const fn dependency_closure(&self) -> &DependencyClosureV1 {
        &self.dependency_closure
    }

    /// Validate the V2 identity and all inherited evidence cross-links.
    pub fn validate(&self) -> Result<(), PredictionContractError> {
        if self.schema != PREDICTION_PROVENANCE_V2_ID
            || self.consumed_contracts != CONSUMED_CONTRACTS_V2
        {
            return Err(PredictionContractError::InvalidConsumedContracts);
        }
        let prefix = self.settings.validation_only_prefix(&self.profile)?;
        PredictionProvenanceV1::new(
            self.profile.clone(),
            self.source_format,
            prefix,
            self.raw_source.clone(),
            self.dependency_closure.clone(),
        )?;
        self.settings.validate_against(&self.profile)?;
        let rows = self.retained_provenance_rows()?;
        if rows > PREDICTION_V1_MAX_AGGREGATE_PROVENANCE_ROWS {
            return Err(PredictionContractError::TooManyAggregateProvenanceRows {
                found: rows,
                limit: PREDICTION_V1_MAX_AGGREGATE_PROVENANCE_ROWS,
            });
        }
        let text = self.retained_text_bytes()?;
        if text > PREDICTION_V1_MAX_TOTAL_TEXT_BYTES_PER_FILE {
            return Err(PredictionContractError::TooMuchRetainedText {
                found: text,
                limit: PREDICTION_V1_MAX_TOTAL_TEXT_BYTES_PER_FILE,
            });
        }
        if self.identity.0 != self.computed_identity() {
            return Err(PredictionContractError::IdentityMismatch {
                contract: PREDICTION_PROVENANCE_V2_ID,
            });
        }
        Ok(())
    }

    pub(crate) fn retained_text_bytes(&self) -> Result<usize, PredictionContractError> {
        let closure_text = checked_sum(
            "V2 closure retained text",
            self.dependency_closure
                .references()
                .iter()
                .filter_map(|reference| closure_target_key(reference.target()).map(str::len))
                .chain(
                    self.dependency_closure
                        .external_resources()
                        .iter()
                        .map(|resource| resource.key().as_str().len()),
                ),
        )?;
        checked_sum(
            "V2 provenance retained text",
            [
                self.profile.retained_text_bytes()?,
                self.settings.retained_text_bytes()?,
                self.raw_source.retained_text_bytes()?,
                closure_text,
            ],
        )
    }

    fn retained_provenance_rows(&self) -> Result<usize, PredictionContractError> {
        let clip_settings = checked_sum(
            "V2 clip setting rows",
            self.settings
                .clips()
                .iter()
                .map(|clip| clip.settings().len()),
        )?;
        let raw_rows = usize::try_from(self.raw_source.work.retained_rows)
            .map_err(|_| PredictionContractError::ArithmeticOverflow("V2 raw-source rows"))?;
        checked_sum(
            "V2 aggregate provenance rows",
            [
                self.profile.facts().len(),
                self.profile.setting_descriptors().len(),
                self.profile.primary_sources().len(),
                self.settings.document_settings().len(),
                clip_settings,
                raw_rows,
            ],
        )
    }

    fn computed_identity(&self) -> InputIdentity {
        let mut encoder = CanonicalEncoder::new("animsmith-prediction-provenance-v2");
        encoder.field("schema");
        encoder.token(self.schema);
        encoder.field("profile");
        self.profile.encode_preimage(&mut encoder);
        encoder.field("source_format");
        encoder.token(source_format_name(self.source_format));
        encoder.field("settings_identity");
        encode_input_identity(&mut encoder, self.settings.settings_identity());
        encoder.field("raw_source");
        encode_raw_binding(&mut encoder, &self.raw_source);
        encoder.field("dependency_closure");
        encode_dependency_closure(&mut encoder, &self.dependency_closure);
        encoder.field("consumed_contracts");
        encoder.count(self.consumed_contracts.len());
        for contract in self.consumed_contracts {
            encoder.token(contract);
        }
        encoder.identity()
    }
}

impl<'de> Deserialize<'de> for PredictionProvenanceV2 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        decode_prediction_provenance_v2_wire(PredictionProvenanceWireV2::deserialize(deserializer)?)
            .map_err(|error| match error {
                PredictionDecodeError::Shape(source) => D::Error::custom(source),
                PredictionDecodeError::Semantic(source) => D::Error::custom(source),
                PredictionDecodeError::TooManyFileFacets
                | PredictionDecodeError::TooManyFileBasisReferences => {
                    unreachable!("provenance decoding cannot consume prediction budgets")
                }
            })
    }
}

/// Decode V2 provenance in dependency order so malformed/header evidence wins
/// before nested payloads and profile/raw/closure rows are admitted under the
/// shared provenance-row budget.
pub(crate) fn decode_prediction_provenance_v2(
    raw: &str,
) -> Result<PredictionProvenanceV2, PredictionDecodeError> {
    let wire: PredictionProvenanceWireV2 =
        serde_json::from_str(raw).map_err(PredictionDecodeError::Shape)?;
    decode_prediction_provenance_v2_wire(wire)
}

fn decode_prediction_provenance_v2_wire(
    wire: PredictionProvenanceWireV2,
) -> Result<PredictionProvenanceV2, PredictionDecodeError> {
    if wire.schema != PREDICTION_PROVENANCE_V2_ID
        || wire.consumed_contracts.overflowed
        || wire
            .consumed_contracts
            .values
            .iter()
            .map(String::as_str)
            .ne(CONSUMED_CONTRACTS_V2)
    {
        return Err(PredictionDecodeError::Semantic(
            PredictionContractError::InvalidConsumedContracts,
        ));
    }
    let raw_source = serde_json::from_str::<RawSourceBindingWireV1>(wire.raw_source.get())
        .map_err(PredictionDecodeError::Shape)
        .and_then(|raw| {
            RawSourceBindingV1::from_wire(raw).map_err(PredictionDecodeError::Semantic)
        })?;
    let raw_rows = usize::try_from(raw_source.work.retained_rows).map_err(|_| {
        PredictionDecodeError::Semantic(PredictionContractError::ArithmeticOverflow(
            "V2 raw-source rows",
        ))
    })?;
    let remaining = PREDICTION_V1_MAX_AGGREGATE_PROVENANCE_ROWS.saturating_sub(raw_rows);
    let profile =
        decode_resolved_engine_profile_v1_with_provenance_limit(wire.profile.get(), remaining)
            .map_err(|error| match error {
                EngineProfileLimitedDecodeError::Contract(EngineContractDecodeError::Shape(
                    source,
                )) => PredictionDecodeError::Shape(source),
                EngineProfileLimitedDecodeError::Contract(EngineContractDecodeError::Semantic(
                    source,
                )) => PredictionDecodeError::Semantic(source.into()),
                EngineProfileLimitedDecodeError::ProvenanceRowsOverflow => {
                    PredictionDecodeError::Semantic(
                        PredictionContractError::TooManyAggregateProvenanceRows {
                            found: PREDICTION_V1_MAX_AGGREGATE_PROVENANCE_ROWS + 1,
                            limit: PREDICTION_V1_MAX_AGGREGATE_PROVENANCE_ROWS,
                        },
                    )
                }
            })?;
    let remaining_after_profile = remaining.saturating_sub(profile.provenance_rows());
    let settings = decode_resolved_engine_settings_v2_with_provenance_limit(
        wire.settings.get(),
        remaining_after_profile,
    )
    .map_err(|error| match error {
        EngineSettingsLimitedDecodeError::Contract(EngineContractDecodeError::Shape(source)) => {
            PredictionDecodeError::Shape(source)
        }
        EngineSettingsLimitedDecodeError::Contract(EngineContractDecodeError::Semantic(source)) => {
            PredictionDecodeError::Semantic(source.into())
        }
        EngineSettingsLimitedDecodeError::ProvenanceRowsOverflow => {
            PredictionDecodeError::Semantic(
                PredictionContractError::TooManyAggregateProvenanceRows {
                    found: PREDICTION_V1_MAX_AGGREGATE_PROVENANCE_ROWS + 1,
                    limit: PREDICTION_V1_MAX_AGGREGATE_PROVENANCE_ROWS,
                },
            )
        }
    })?;
    let dependency_closure = decode_dependency_closure_v1(wire.dependency_closure.get()).map_err(
        |error| match error {
            DependencyClosureDecodeError::Shape(source) => PredictionDecodeError::Shape(source),
            DependencyClosureDecodeError::Semantic(reason) => PredictionDecodeError::Semantic(
                PredictionContractError::InvalidDependencyClosure(reason),
            ),
        },
    )?;
    let provenance = PredictionProvenanceV2::new(
        profile,
        wire.source_format,
        settings,
        raw_source,
        dependency_closure,
    )
    .map_err(PredictionDecodeError::Semantic)?;
    if provenance.identity != wire.identity {
        return Err(PredictionDecodeError::Semantic(
            PredictionContractError::IdentityMismatch {
                contract: PREDICTION_PROVENANCE_V2_ID,
            },
        ));
    }
    Ok(provenance)
}

fn validate_basis_reference(
    reference: &PredictionBasisReferenceV1,
    provenance: &PredictionProvenanceV1,
) -> Result<(), PredictionContractError> {
    match reference {
        PredictionBasisReferenceV1::ProfileFact { fact_id } => {
            if !provenance
                .profile
                .facts()
                .iter()
                .any(|fact| fact.id().as_str() == fact_id)
            {
                return Err(PredictionContractError::UnknownProfileFact(fact_id.clone()));
            }
        }
        PredictionBasisReferenceV1::ResolvedSetting {
            location,
            setting_id,
        } => {
            let Some(id) = parse_setting_id(setting_id) else {
                return Err(PredictionContractError::UnknownResolvedSetting(
                    setting_id.clone(),
                ));
            };
            let Some(descriptor) = provenance.profile.setting_descriptor(id) else {
                return Err(PredictionContractError::UnknownResolvedSetting(
                    setting_id.clone(),
                ));
            };
            let present = match location {
                ResolvedSettingLocationV1::Document => {
                    descriptor.scope() == EngineSettingScopeV1::Document
                        && provenance.settings.document_setting(id).is_some()
                }
                ResolvedSettingLocationV1::Clip {
                    clip_ordinal,
                    clip_name,
                } => usize::try_from(*clip_ordinal)
                    .ok()
                    .and_then(|ordinal| provenance.settings.clip_row(ordinal, clip_name))
                    .is_some_and(|row| {
                        descriptor.scope() == EngineSettingScopeV1::Clip
                            && row.setting(id).is_some()
                    }),
            };
            if !present {
                return Err(PredictionContractError::UnknownResolvedSetting(
                    setting_id.clone(),
                ));
            }
        }
        PredictionBasisReferenceV1::PrimarySource { source_id } => {
            if provenance.profile.source(source_id).is_none() {
                return Err(PredictionContractError::UnknownPrimarySource(
                    source_id.clone(),
                ));
            }
        }
        PredictionBasisReferenceV1::Measurement { schema, .. }
            if *schema != MEASUREMENTS_SCHEMA_ID =>
        {
            return Err(PredictionContractError::InvalidSchema {
                field: "basis.measurement.schema",
                expected: MEASUREMENTS_SCHEMA_ID,
                found: (*schema).to_owned(),
            });
        }
        PredictionBasisReferenceV1::RawSource { reference } => {
            if !raw_domain_matches_key(reference.domain, &reference.key) {
                return Err(PredictionContractError::RawSourceDomainKeyMismatch);
            }
        }
        PredictionBasisReferenceV1::ProjectField { .. }
        | PredictionBasisReferenceV1::Measurement { .. } => {}
    }
    Ok(())
}

fn validate_basis_reference_v2(
    reference: &PredictionBasisReferenceV1,
    provenance: &PredictionProvenanceV2,
) -> Result<(), PredictionContractError> {
    match reference {
        PredictionBasisReferenceV1::ProfileFact { fact_id } => {
            if !provenance
                .profile()
                .facts()
                .iter()
                .any(|fact| fact.id().as_str() == fact_id)
            {
                return Err(PredictionContractError::UnknownProfileFact(fact_id.clone()));
            }
        }
        PredictionBasisReferenceV1::ResolvedSetting {
            location,
            setting_id,
        } => {
            let Some(id) = parse_setting_id(setting_id) else {
                return Err(PredictionContractError::UnknownResolvedSetting(
                    setting_id.clone(),
                ));
            };
            let Some(descriptor) = provenance.profile().setting_descriptor(id) else {
                return Err(PredictionContractError::UnknownResolvedSetting(
                    setting_id.clone(),
                ));
            };
            let present = match location {
                ResolvedSettingLocationV1::Document => {
                    descriptor.scope() == EngineSettingScopeV1::Document
                        && provenance.settings().document_setting(id).is_some()
                }
                ResolvedSettingLocationV1::Clip {
                    clip_ordinal,
                    clip_name,
                } => usize::try_from(*clip_ordinal)
                    .ok()
                    .and_then(|ordinal| provenance.settings().clip_row(ordinal, clip_name))
                    .is_some_and(|row| {
                        descriptor.scope() == EngineSettingScopeV1::Clip
                            && row.setting(id).is_some()
                    }),
            };
            if !present {
                return Err(PredictionContractError::UnknownResolvedSetting(
                    setting_id.clone(),
                ));
            }
        }
        PredictionBasisReferenceV1::PrimarySource { source_id } => {
            if provenance.profile().source(source_id).is_none() {
                return Err(PredictionContractError::UnknownPrimarySource(
                    source_id.clone(),
                ));
            }
        }
        PredictionBasisReferenceV1::Measurement { schema, .. }
            if *schema != MEASUREMENTS_SCHEMA_ID =>
        {
            return Err(PredictionContractError::InvalidSchema {
                field: "basis.measurement.schema",
                expected: MEASUREMENTS_SCHEMA_ID,
                found: (*schema).to_owned(),
            });
        }
        PredictionBasisReferenceV1::RawSource { reference } => {
            if !raw_domain_matches_key(reference.domain, &reference.key) {
                return Err(PredictionContractError::RawSourceDomainKeyMismatch);
            }
        }
        PredictionBasisReferenceV1::ProjectField { .. }
        | PredictionBasisReferenceV1::Measurement { .. } => {}
    }
    Ok(())
}

fn parse_setting_id(value: &str) -> Option<EngineSettingIdV1> {
    [
        EngineSettingIdV1::ConvertUnits,
        EngineSettingIdV1::BakeAxisConversion,
        EngineSettingIdV1::RootMotionSource,
        EngineSettingIdV1::RootRotation,
        EngineSettingIdV1::RootPositionY,
        EngineSettingIdV1::RootPositionXz,
    ]
    .into_iter()
    .find(|id| id.as_str() == value)
}

fn closure_target_key(target: &DependencyReferenceTargetV1) -> Option<&str> {
    match target {
        DependencyReferenceTargetV1::External { key }
        | DependencyReferenceTargetV1::Refused { key: Some(key), .. }
        | DependencyReferenceTargetV1::Unavailable { key: Some(key), .. } => Some(key.as_str()),
        _ => None,
    }
}

fn encode_raw_binding(encoder: &mut CanonicalEncoder, raw: &RawSourceBindingV1) {
    encoder.token("animsmith-raw-source-binding-v1");
    encoder.field("schema");
    encoder.token(raw.schema);
    encoder.field("primary_input");
    encode_input_identity(encoder, &raw.primary_input);
    encoder.field("source_format");
    encoder.token(source_format_name(raw.source_format));
    encoder.field("linear_unit");
    encode_raw_observation(encoder, &raw.linear_unit, |encoder, value| {
        encoder.token(value.canonical_bits());
    });
    encoder.field("coordinate_basis");
    encode_raw_observation(encoder, &raw.coordinate_basis, |encoder, value| {
        encoder.token(raw_axis_name(value.right));
        encoder.token(raw_axis_name(value.up));
        encoder.token(raw_axis_name(value.forward));
    });
    encoder.field("frames_per_second");
    encode_raw_observation(encoder, &raw.frames_per_second, |encoder, value| {
        encoder.token(value.canonical_bits());
    });
    encoder.field("clips_coverage");
    encode_raw_coverage(encoder, raw.clips_coverage);
    encoder.field("constructs_coverage");
    encode_raw_coverage(encoder, raw.constructs_coverage);
    encoder.field("resources_coverage");
    encode_raw_coverage(encoder, raw.resources_coverage);
    encoder.field("source_skeleton_coverage");
    encoder.token(match raw.source_skeleton_coverage {
        SourceSkeletonCoverage::Unavailable => "unavailable",
        SourceSkeletonCoverage::Complete => "complete",
    });
    encoder.field("work");
    encoder.token(raw.work.inspected_rows.to_string());
    encoder.token(raw.work.retained_rows.to_string());
    encoder.token(raw.work.retained_text_bytes.to_string());
    encoder.token(raw.work.max_traversal_depth.to_string());
}

fn encode_raw_observation<T>(
    encoder: &mut CanonicalEncoder,
    observation: &RawSourceObservationWireV1<T>,
    encode_value: impl FnOnce(&mut CanonicalEncoder, &T),
) {
    match &observation.state {
        RawSourceObservationStateWireV1::Observed { value } => {
            encoder.token("observed");
            encode_value(encoder, value);
        }
        RawSourceObservationStateWireV1::ProvenAbsent => encoder.token("proven_absent"),
        RawSourceObservationStateWireV1::Unavailable { reason } => {
            encoder.token("unavailable");
            encoder.token(raw_unavailable_reason_name(*reason));
        }
    }
    encoder.token(raw_disposition_name(observation.disposition));
    encode_option(
        encoder,
        observation.provenance.as_ref(),
        |encoder, provenance| {
            encoder.token(raw_provenance_kind_name(provenance.kind));
            encode_option(
                encoder,
                provenance.locator.as_deref(),
                |encoder, locator| {
                    encoder.token(locator);
                },
            );
        },
    );
}

fn encode_raw_coverage(encoder: &mut CanonicalEncoder, coverage: RawSourceSetCoverageV1) {
    encoder.token(match coverage.state {
        RawSourceSetCoverageStateV1::Complete => "complete",
        RawSourceSetCoverageStateV1::Partial => "partial",
        RawSourceSetCoverageStateV1::Unavailable => "unavailable",
    });
    encode_option(encoder, coverage.reason, |encoder, reason| {
        encoder.token(raw_unavailable_reason_name(reason));
    });
}

fn raw_axis_name(value: RawSourceAxisV1) -> &'static str {
    match value {
        RawSourceAxisV1::PositiveX => "positive_x",
        RawSourceAxisV1::NegativeX => "negative_x",
        RawSourceAxisV1::PositiveY => "positive_y",
        RawSourceAxisV1::NegativeY => "negative_y",
        RawSourceAxisV1::PositiveZ => "positive_z",
        RawSourceAxisV1::NegativeZ => "negative_z",
    }
}

fn raw_unavailable_reason_name(value: RawSourceUnavailableReasonV1) -> &'static str {
    match value {
        RawSourceUnavailableReasonV1::Malformed => "malformed",
        RawSourceUnavailableReasonV1::Discarded => "discarded",
        RawSourceUnavailableReasonV1::NormalizedAway => "normalized_away",
        RawSourceUnavailableReasonV1::BakedAway => "baked_away",
        RawSourceUnavailableReasonV1::LoaderUnsupported => "loader_unsupported",
        RawSourceUnavailableReasonV1::ProjectionBudgetExceeded => "projection_budget_exceeded",
        RawSourceUnavailableReasonV1::ParserUnavailable => "parser_unavailable",
    }
}

fn raw_disposition_name(value: RawSourceDispositionV1) -> &'static str {
    match value {
        RawSourceDispositionV1::Preserved => "preserved",
        RawSourceDispositionV1::Normalized => "normalized",
        RawSourceDispositionV1::Baked => "baked",
        RawSourceDispositionV1::Discarded => "discarded",
        RawSourceDispositionV1::Unsupported => "unsupported",
        RawSourceDispositionV1::Unknown => "unknown",
        RawSourceDispositionV1::NotApplicable => "not_applicable",
    }
}

fn raw_provenance_kind_name(value: RawSourceProvenanceKindV1) -> &'static str {
    match value {
        RawSourceProvenanceKindV1::FormatDefined => "format_defined",
        RawSourceProvenanceKindV1::SourceDeclared => "source_declared",
        RawSourceProvenanceKindV1::ParserProjected => "parser_projected",
        RawSourceProvenanceKindV1::DerivedFromSource => "derived_from_source",
    }
}

fn encode_dependency_closure(encoder: &mut CanonicalEncoder, closure: &DependencyClosureV1) {
    encoder.token("animsmith-dependency-closure-wire-v1");
    encoder.field("schema");
    encoder.token(closure.contract_id());
    encoder.field("budget");
    let budget = closure.budget();
    encoder.token(budget.contract_id());
    encoder.token(budget.max_references().to_string());
    encoder.token(budget.max_external_resources().to_string());
    encoder.token(budget.max_key_bytes().to_string());
    encoder.token(budget.max_path_components().to_string());
    encoder.token(budget.max_normalization_bytes().to_string());
    encoder.token(budget.max_resource_bytes().to_string());
    encoder.token(budget.max_total_resource_bytes().to_string());
    encoder.token(budget.max_dedup_probes().to_string());
    encoder.field("primary_input");
    encode_input_identity(encoder, closure.primary_input());
    encoder.field("coverage");
    match closure.coverage() {
        DependencyClosureCoverageV1::Complete => {
            encoder.token("complete");
            encoder.count(0);
        }
        DependencyClosureCoverageV1::Partial { .. } => {
            encoder.token("partial");
            encode_closure_reasons(encoder, closure.coverage().reasons());
        }
        DependencyClosureCoverageV1::Unavailable { .. } => {
            encoder.token("unavailable");
            encode_closure_reasons(encoder, closure.coverage().reasons());
        }
    }
    encoder.field("identity");
    encode_option(encoder, closure.identity(), |encoder, identity| {
        encode_input_identity(encoder, identity.input_identity());
    });
    encoder.field("references");
    encoder.count(closure.references().len());
    for reference in closure.references() {
        encoder.token(reference.source_order_index().to_string());
        encoder.token(source_resource_kind_name(reference.kind()));
        encoder.token(dependency_purpose_name(reference.purpose()));
        encoder.token(reference.source_index().to_string());
        match reference.target() {
            DependencyReferenceTargetV1::Primary => {
                encoder.token("primary");
                encoder.token("none");
                encoder.token("none");
            }
            DependencyReferenceTargetV1::External { key } => {
                encoder.token("external");
                encoder.token("some");
                encoder.token(key.as_str());
                encoder.token("none");
            }
            DependencyReferenceTargetV1::Refused { key, reason } => {
                encoder.token("refused");
                encode_option(encoder, key.as_ref(), |encoder, key| {
                    encoder.token(key.as_str());
                });
                encoder.token("some");
                encoder.token(dependency_refusal_reason_name(*reason));
            }
            DependencyReferenceTargetV1::Unavailable { key, reason } => {
                encoder.token("unavailable");
                encode_option(encoder, key.as_ref(), |encoder, key| {
                    encoder.token(key.as_str());
                });
                encoder.token("some");
                encoder.token(dependency_unavailable_reason_name(*reason));
            }
        }
    }
    encoder.field("external_resources");
    encoder.count(closure.external_resources().len());
    for resource in closure.external_resources() {
        encoder.token(resource.key().as_str());
        encode_input_identity(encoder, resource.identity());
    }
    encoder.field("work");
    let work = closure.work();
    encoder.token(work.inspected_references().to_string());
    encoder.token(work.retained_references().to_string());
    encoder.token(work.normalization_bytes_inspected().to_string());
    encoder.token(work.path_components_inspected().to_string());
    encoder.token(work.dedup_probes().to_string());
    encoder.token(work.external_open_attempts().to_string());
    encoder.token(work.distinct_external_keys().to_string());
    encoder.token(work.captured_external_resources().to_string());
    encoder.token(work.external_bytes_read_hashed().to_string());
}

fn encode_closure_reasons(
    encoder: &mut CanonicalEncoder,
    reasons: &[DependencyClosureCoverageReasonV1],
) {
    encoder.count(reasons.len());
    for reason in reasons {
        encoder.token(dependency_coverage_reason_name(*reason));
    }
}

fn dependency_purpose_name(value: DependencyResourcePurposeV1) -> &'static str {
    match value {
        DependencyResourcePurposeV1::LoaderEssential => "loader_essential",
        DependencyResourcePurposeV1::Nonessential => "nonessential",
        DependencyResourcePurposeV1::TargetOnly => "target_only",
    }
}

fn dependency_refusal_reason_name(value: DependencyResourceRefusalReasonV1) -> &'static str {
    match value {
        DependencyResourceRefusalReasonV1::Absolute => "absolute",
        DependencyResourceRefusalReasonV1::Escaping => "escaping",
        DependencyResourceRefusalReasonV1::Remote => "remote",
        DependencyResourceRefusalReasonV1::Malformed => "malformed",
        DependencyResourceRefusalReasonV1::Oversized => "oversized",
        DependencyResourceRefusalReasonV1::Symlink => "symlink",
    }
}

fn dependency_unavailable_reason_name(
    value: DependencyResourceUnavailableReasonV1,
) -> &'static str {
    match value {
        DependencyResourceUnavailableReasonV1::ResourceRootUnavailable => {
            "resource_root_unavailable"
        }
        DependencyResourceUnavailableReasonV1::Missing => "missing",
        DependencyResourceUnavailableReasonV1::Unreadable => "unreadable",
        DependencyResourceUnavailableReasonV1::ResourceBudgetExceeded => "resource_budget_exceeded",
    }
}

fn dependency_coverage_reason_name(value: DependencyClosureCoverageReasonV1) -> &'static str {
    match value {
        DependencyClosureCoverageReasonV1::SourceDeclarationsPartial => {
            "source_declarations_partial"
        }
        DependencyClosureCoverageReasonV1::SourceDeclarationsUnavailable => {
            "source_declarations_unavailable"
        }
        DependencyClosureCoverageReasonV1::CaptureUnavailable => "capture_unavailable",
        DependencyClosureCoverageReasonV1::RefusedResource => "refused_resource",
        DependencyClosureCoverageReasonV1::UnavailableResource => "unavailable_resource",
        DependencyClosureCoverageReasonV1::ResourceBudgetExceeded => "resource_budget_exceeded",
        DependencyClosureCoverageReasonV1::UnmodeledResourceDomain => "unmodeled_resource_domain",
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ResolvedMeasurementNode {
    Scalar(PredictionScalarV1),
    NonScalar,
}

#[derive(Debug)]
pub(crate) struct MeasurementReferenceBatchError {
    pub(crate) prediction_index: usize,
    pub(crate) source: PredictionContractError,
}

struct MeasurementExpectation<'prediction> {
    prediction_index: usize,
    pointer: &'prediction MeasurementPointerV1,
    expected: &'prediction PredictionScalarV1,
    target_index: usize,
}

pub(crate) fn validate_measurement_references_batch<'prediction>(
    measurements: &MeasurementContract,
    predictions: impl IntoIterator<Item = (usize, &'prediction EnginePredictionV1)>,
) -> Result<(), MeasurementReferenceBatchError> {
    validate_measurement_references_batch_impl(measurements, predictions).map(|_| ())
}

/// V2 counterpart of the V1 batch resolver. The referenced measurement wire
/// vocabulary is deliberately shared, but V2 must not be routed through a V1
/// prediction wrapper because its provenance identity and overflow semantics
/// are distinct.
pub(crate) fn validate_measurement_references_batch_v2<'prediction>(
    measurements: &MeasurementContract,
    predictions: impl IntoIterator<Item = (usize, &'prediction EnginePredictionV2)>,
) -> Result<(), MeasurementReferenceBatchError> {
    let mut targets = BTreeMap::<Vec<String>, usize>::new();
    let mut expectations = Vec::new();
    for (prediction_index, prediction) in predictions {
        for reference in prediction
            .facets
            .iter()
            .flat_map(|facet| facet.basis.references.iter())
        {
            let PredictionBasisReferenceV1::Measurement { pointer, value, .. } = reference else {
                continue;
            };
            let target = pointer
                .as_str()
                .split('/')
                .skip(2)
                .map(decode_pointer_component)
                .collect::<Vec<_>>();
            let next_index = targets.len();
            let target_index = *targets.entry(target).or_insert(next_index);
            expectations.push(MeasurementExpectation {
                prediction_index,
                pointer,
                expected: value,
                target_index,
            });
        }
    }
    if expectations.is_empty() {
        return Ok(());
    }
    let mut found = vec![None; targets.len()];
    let mut resolver = MeasurementScalarResolver {
        targets: &targets,
        path: Vec::new(),
        found: &mut found,
    };
    if measurements.serialize(&mut resolver).is_err() {
        let first = &expectations[0];
        return Err(MeasurementReferenceBatchError {
            prediction_index: first.prediction_index,
            source: PredictionContractError::MeasurementPointerMissing(first.pointer.0.clone()),
        });
    }
    for expectation in expectations {
        let source = match found[expectation.target_index].as_ref() {
            Some(ResolvedMeasurementNode::Scalar(actual)) if actual == expectation.expected => {
                continue;
            }
            Some(ResolvedMeasurementNode::Scalar(_)) => {
                PredictionContractError::MeasurementValueMismatch(expectation.pointer.0.clone())
            }
            Some(ResolvedMeasurementNode::NonScalar) => {
                PredictionContractError::MeasurementPointerNotScalar(expectation.pointer.0.clone())
            }
            None => {
                PredictionContractError::MeasurementPointerMissing(expectation.pointer.0.clone())
            }
        };
        return Err(MeasurementReferenceBatchError {
            prediction_index: expectation.prediction_index,
            source,
        });
    }
    Ok(())
}

fn validate_measurement_references_batch_impl<'prediction>(
    measurements: &MeasurementContract,
    predictions: impl IntoIterator<Item = (usize, &'prediction EnginePredictionV1)>,
) -> Result<usize, MeasurementReferenceBatchError> {
    let mut targets = BTreeMap::<Vec<String>, usize>::new();
    let mut expectations = Vec::new();
    for (prediction_index, prediction) in predictions {
        for reference in prediction
            .facets
            .iter()
            .flat_map(|facet| facet.basis.references.iter())
        {
            let PredictionBasisReferenceV1::Measurement { pointer, value, .. } = reference else {
                continue;
            };
            let target = pointer
                .as_str()
                .split('/')
                .skip(2)
                .map(decode_pointer_component)
                .collect::<Vec<_>>();
            let next_index = targets.len();
            let target_index = *targets.entry(target).or_insert(next_index);
            expectations.push(MeasurementExpectation {
                prediction_index,
                pointer,
                expected: value,
                target_index,
            });
        }
    }
    if expectations.is_empty() {
        return Ok(0);
    }

    let mut found = vec![None; targets.len()];
    let mut resolver = MeasurementScalarResolver {
        targets: &targets,
        path: Vec::new(),
        found: &mut found,
    };
    if measurements.serialize(&mut resolver).is_err() {
        let first = &expectations[0];
        return Err(MeasurementReferenceBatchError {
            prediction_index: first.prediction_index,
            source: PredictionContractError::MeasurementPointerMissing(first.pointer.0.clone()),
        });
    }
    for expectation in expectations {
        let source = match found[expectation.target_index].as_ref() {
            Some(ResolvedMeasurementNode::Scalar(actual)) if actual == expectation.expected => {
                continue;
            }
            Some(ResolvedMeasurementNode::Scalar(_)) => {
                PredictionContractError::MeasurementValueMismatch(expectation.pointer.0.clone())
            }
            Some(ResolvedMeasurementNode::NonScalar) => {
                PredictionContractError::MeasurementPointerNotScalar(expectation.pointer.0.clone())
            }
            None => {
                PredictionContractError::MeasurementPointerMissing(expectation.pointer.0.clone())
            }
        };
        return Err(MeasurementReferenceBatchError {
            prediction_index: expectation.prediction_index,
            source,
        });
    }
    Ok(1)
}

fn decode_pointer_component(component: &str) -> String {
    let mut decoded = String::with_capacity(component.len());
    let mut chars = component.chars();
    while let Some(character) = chars.next() {
        if character == '~' {
            decoded.push(match chars.next().expect("pointer was validated") {
                '0' => '~',
                '1' => '/',
                _ => unreachable!("pointer was validated"),
            });
        } else {
            decoded.push(character);
        }
    }
    decoded
}

#[derive(Debug)]
struct MeasurementResolveError(String);

impl std::fmt::Display for MeasurementResolveError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for MeasurementResolveError {}

impl serde::ser::Error for MeasurementResolveError {
    fn custom<T: std::fmt::Display>(message: T) -> Self {
        Self(message.to_string())
    }
}

struct MeasurementScalarResolver<'target, 'found> {
    targets: &'target BTreeMap<Vec<String>, usize>,
    path: Vec<String>,
    found: &'found mut [Option<ResolvedMeasurementNode>],
}

impl MeasurementScalarResolver<'_, '_> {
    fn record(&mut self, node: ResolvedMeasurementNode) {
        if let Some(index) = self.targets.get(&self.path).copied()
            && self.found[index].is_none()
        {
            self.found[index] = Some(node);
        }
    }

    fn with_component(
        &mut self,
        component: String,
        value: &(impl Serialize + ?Sized),
    ) -> Result<(), MeasurementResolveError> {
        self.path.push(component);
        value.serialize(&mut *self)?;
        self.path.pop();
        Ok(())
    }
}

struct MeasurementCompound<'resolver, 'target, 'found> {
    resolver: &'resolver mut MeasurementScalarResolver<'target, 'found>,
    next_index: usize,
    pending_key: Option<String>,
    pop_on_end: bool,
}

impl MeasurementCompound<'_, '_, '_> {
    fn finish(self) {
        if self.pop_on_end {
            self.resolver.path.pop();
        }
    }
}

impl<'resolver, 'target, 'found> Serializer
    for &'resolver mut MeasurementScalarResolver<'target, 'found>
{
    type Ok = ();
    type Error = MeasurementResolveError;
    type SerializeSeq = MeasurementCompound<'resolver, 'target, 'found>;
    type SerializeTuple = MeasurementCompound<'resolver, 'target, 'found>;
    type SerializeTupleStruct = MeasurementCompound<'resolver, 'target, 'found>;
    type SerializeTupleVariant = MeasurementCompound<'resolver, 'target, 'found>;
    type SerializeMap = MeasurementCompound<'resolver, 'target, 'found>;
    type SerializeStruct = MeasurementCompound<'resolver, 'target, 'found>;
    type SerializeStructVariant = MeasurementCompound<'resolver, 'target, 'found>;

    fn serialize_bool(self, value: bool) -> Result<Self::Ok, Self::Error> {
        self.record(ResolvedMeasurementNode::Scalar(
            PredictionScalarV1::Boolean { value },
        ));
        Ok(())
    }

    fn serialize_i8(self, value: i8) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(i64::from(value))
    }
    fn serialize_i16(self, value: i16) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(i64::from(value))
    }
    fn serialize_i32(self, value: i32) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(i64::from(value))
    }
    fn serialize_i64(self, value: i64) -> Result<Self::Ok, Self::Error> {
        self.record(ResolvedMeasurementNode::Scalar(
            PredictionScalarV1::SignedInteger { value },
        ));
        Ok(())
    }
    fn serialize_i128(self, value: i128) -> Result<Self::Ok, Self::Error> {
        let value = i64::try_from(value)
            .map_err(|_| MeasurementResolveError("i128 is outside V1 scalar range".into()))?;
        self.serialize_i64(value)
    }
    fn serialize_u8(self, value: u8) -> Result<Self::Ok, Self::Error> {
        self.serialize_u64(u64::from(value))
    }
    fn serialize_u16(self, value: u16) -> Result<Self::Ok, Self::Error> {
        self.serialize_u64(u64::from(value))
    }
    fn serialize_u32(self, value: u32) -> Result<Self::Ok, Self::Error> {
        self.serialize_u64(u64::from(value))
    }
    fn serialize_u64(self, value: u64) -> Result<Self::Ok, Self::Error> {
        self.record(ResolvedMeasurementNode::Scalar(
            PredictionScalarV1::UnsignedInteger { value },
        ));
        Ok(())
    }
    fn serialize_u128(self, value: u128) -> Result<Self::Ok, Self::Error> {
        let value = u64::try_from(value)
            .map_err(|_| MeasurementResolveError("u128 is outside V1 scalar range".into()))?;
        self.serialize_u64(value)
    }
    fn serialize_f32(self, value: f32) -> Result<Self::Ok, Self::Error> {
        self.serialize_f64(f64::from(value))
    }
    fn serialize_f64(self, value: f64) -> Result<Self::Ok, Self::Error> {
        let scalar =
            PredictionScalarV1::finite_number(value).map_err(MeasurementResolveError::custom)?;
        self.record(ResolvedMeasurementNode::Scalar(scalar));
        Ok(())
    }
    fn serialize_char(self, value: char) -> Result<Self::Ok, Self::Error> {
        self.serialize_str(&value.to_string())
    }
    fn serialize_str(self, value: &str) -> Result<Self::Ok, Self::Error> {
        let scalar = PredictionScalarV1::text(value).map_err(MeasurementResolveError::custom)?;
        self.record(ResolvedMeasurementNode::Scalar(scalar));
        Ok(())
    }
    fn serialize_bytes(self, value: &[u8]) -> Result<Self::Ok, Self::Error> {
        let mut sequence = self.serialize_seq(Some(value.len()))?;
        for byte in value {
            SerializeSeq::serialize_element(&mut sequence, byte)?;
        }
        SerializeSeq::end(sequence)
    }
    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        self.record(ResolvedMeasurementNode::Scalar(PredictionScalarV1::Null));
        Ok(())
    }
    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<Self::Ok, Self::Error> {
        value.serialize(self)
    }
    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        self.serialize_none()
    }
    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        self.serialize_unit()
    }
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        let scalar = PredictionScalarV1::token(variant).map_err(MeasurementResolveError::custom)?;
        self.record(ResolvedMeasurementNode::Scalar(scalar));
        Ok(())
    }
    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        value.serialize(self)
    }
    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        self.record(ResolvedMeasurementNode::NonScalar);
        self.with_component(variant.to_owned(), value)
    }
    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        self.record(ResolvedMeasurementNode::NonScalar);
        Ok(MeasurementCompound {
            resolver: self,
            next_index: 0,
            pending_key: None,
            pop_on_end: false,
        })
    }
    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        self.serialize_seq(Some(len))
    }
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.serialize_seq(Some(len))
    }
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        self.record(ResolvedMeasurementNode::NonScalar);
        self.path.push(variant.to_owned());
        self.record(ResolvedMeasurementNode::NonScalar);
        Ok(MeasurementCompound {
            resolver: self,
            next_index: 0,
            pending_key: None,
            pop_on_end: true,
        })
    }
    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        self.record(ResolvedMeasurementNode::NonScalar);
        Ok(MeasurementCompound {
            resolver: self,
            next_index: 0,
            pending_key: None,
            pop_on_end: false,
        })
    }
    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        self.serialize_map(None)
    }
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        self.record(ResolvedMeasurementNode::NonScalar);
        self.path.push(variant.to_owned());
        self.record(ResolvedMeasurementNode::NonScalar);
        Ok(MeasurementCompound {
            resolver: self,
            next_index: 0,
            pending_key: None,
            pop_on_end: true,
        })
    }
    fn collect_str<T: ?Sized + std::fmt::Display>(
        self,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        self.serialize_str(&value.to_string())
    }
}

impl SerializeSeq for MeasurementCompound<'_, '_, '_> {
    type Ok = ();
    type Error = MeasurementResolveError;

    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        let index = self.next_index;
        self.next_index += 1;
        self.resolver.with_component(index.to_string(), value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.finish();
        Ok(())
    }
}

impl SerializeTuple for MeasurementCompound<'_, '_, '_> {
    type Ok = ();
    type Error = MeasurementResolveError;
    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        SerializeSeq::serialize_element(self, value)
    }
    fn end(self) -> Result<Self::Ok, Self::Error> {
        SerializeSeq::end(self)
    }
}

impl SerializeTupleStruct for MeasurementCompound<'_, '_, '_> {
    type Ok = ();
    type Error = MeasurementResolveError;
    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        SerializeSeq::serialize_element(self, value)
    }
    fn end(self) -> Result<Self::Ok, Self::Error> {
        SerializeSeq::end(self)
    }
}

impl SerializeTupleVariant for MeasurementCompound<'_, '_, '_> {
    type Ok = ();
    type Error = MeasurementResolveError;
    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        SerializeSeq::serialize_element(self, value)
    }
    fn end(self) -> Result<Self::Ok, Self::Error> {
        SerializeSeq::end(self)
    }
}

impl SerializeMap for MeasurementCompound<'_, '_, '_> {
    type Ok = ();
    type Error = MeasurementResolveError;

    fn serialize_key<T: ?Sized + Serialize>(&mut self, key: &T) -> Result<(), Self::Error> {
        self.pending_key = Some(key.serialize(MeasurementMapKeySerializer)?);
        Ok(())
    }

    fn serialize_value<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        let key = self
            .pending_key
            .take()
            .ok_or_else(|| MeasurementResolveError("map value had no key".into()))?;
        self.resolver.with_component(key, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.finish();
        Ok(())
    }
}

impl SerializeStruct for MeasurementCompound<'_, '_, '_> {
    type Ok = ();
    type Error = MeasurementResolveError;
    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error> {
        self.resolver.with_component(key.to_owned(), value)
    }
    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.finish();
        Ok(())
    }
}

impl SerializeStructVariant for MeasurementCompound<'_, '_, '_> {
    type Ok = ();
    type Error = MeasurementResolveError;
    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error> {
        self.resolver.with_component(key.to_owned(), value)
    }
    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.finish();
        Ok(())
    }
}

struct MeasurementMapKeySerializer;

impl Serializer for MeasurementMapKeySerializer {
    type Ok = String;
    type Error = MeasurementResolveError;
    type SerializeSeq = serde::ser::Impossible<String, MeasurementResolveError>;
    type SerializeTuple = serde::ser::Impossible<String, MeasurementResolveError>;
    type SerializeTupleStruct = serde::ser::Impossible<String, MeasurementResolveError>;
    type SerializeTupleVariant = serde::ser::Impossible<String, MeasurementResolveError>;
    type SerializeMap = serde::ser::Impossible<String, MeasurementResolveError>;
    type SerializeStruct = serde::ser::Impossible<String, MeasurementResolveError>;
    type SerializeStructVariant = serde::ser::Impossible<String, MeasurementResolveError>;

    fn serialize_str(self, value: &str) -> Result<Self::Ok, Self::Error> {
        Ok(value.to_owned())
    }
    fn serialize_char(self, value: char) -> Result<Self::Ok, Self::Error> {
        Ok(value.to_string())
    }
    fn serialize_bool(self, value: bool) -> Result<Self::Ok, Self::Error> {
        Ok(value.to_string())
    }
    fn serialize_i8(self, value: i8) -> Result<Self::Ok, Self::Error> {
        Ok(value.to_string())
    }
    fn serialize_i16(self, value: i16) -> Result<Self::Ok, Self::Error> {
        Ok(value.to_string())
    }
    fn serialize_i32(self, value: i32) -> Result<Self::Ok, Self::Error> {
        Ok(value.to_string())
    }
    fn serialize_i64(self, value: i64) -> Result<Self::Ok, Self::Error> {
        Ok(value.to_string())
    }
    fn serialize_i128(self, value: i128) -> Result<Self::Ok, Self::Error> {
        Ok(value.to_string())
    }
    fn serialize_u8(self, value: u8) -> Result<Self::Ok, Self::Error> {
        Ok(value.to_string())
    }
    fn serialize_u16(self, value: u16) -> Result<Self::Ok, Self::Error> {
        Ok(value.to_string())
    }
    fn serialize_u32(self, value: u32) -> Result<Self::Ok, Self::Error> {
        Ok(value.to_string())
    }
    fn serialize_u64(self, value: u64) -> Result<Self::Ok, Self::Error> {
        Ok(value.to_string())
    }
    fn serialize_u128(self, value: u128) -> Result<Self::Ok, Self::Error> {
        Ok(value.to_string())
    }
    fn serialize_f32(self, value: f32) -> Result<Self::Ok, Self::Error> {
        Ok(value.to_string())
    }
    fn serialize_f64(self, value: f64) -> Result<Self::Ok, Self::Error> {
        Ok(value.to_string())
    }
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Ok(variant.to_owned())
    }
    fn collect_str<T: ?Sized + std::fmt::Display>(
        self,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        Ok(value.to_string())
    }

    fn serialize_bytes(self, _value: &[u8]) -> Result<Self::Ok, Self::Error> {
        Err(MeasurementResolveError("invalid map key".into()))
    }
    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Err(MeasurementResolveError("invalid map key".into()))
    }
    fn serialize_some<T: ?Sized + Serialize>(self, _value: &T) -> Result<Self::Ok, Self::Error> {
        Err(MeasurementResolveError("invalid map key".into()))
    }
    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Err(MeasurementResolveError("invalid map key".into()))
    }
    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        Err(MeasurementResolveError("invalid map key".into()))
    }
    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        value.serialize(self)
    }
    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        Err(MeasurementResolveError("invalid map key".into()))
    }
    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Err(MeasurementResolveError("invalid map key".into()))
    }
    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Err(MeasurementResolveError("invalid map key".into()))
    }
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(MeasurementResolveError("invalid map key".into()))
    }
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(MeasurementResolveError("invalid map key".into()))
    }
    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(MeasurementResolveError("invalid map key".into()))
    }
    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Err(MeasurementResolveError("invalid map key".into()))
    }
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(MeasurementResolveError("invalid map key".into()))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::*;
    use crate::engine_contract::{
        EngineDefaultStatusV1, EngineFactIdV1, EngineFactStateV1, EngineFactValueV1,
        EnginePrimarySourceV1, EngineProfileFactV1, EngineProfileSelectionV1,
        EngineSettingApplicabilityV1, EngineSettingDescriptorV1, EngineSettingDomainV1,
    };
    use crate::evaluation::EvaluationScopeCode;
    use crate::measure::AssetMeasurements;
    use crate::{
        DependencyClosureBuilderV1, ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS, EngineClipSettingsV1,
    };

    fn test_identity() -> PredictionProvenanceIdentityV1 {
        PredictionProvenanceIdentityV1::from_input_identity(InputIdentity::from_bytes(b"profile"))
    }

    fn prediction_with_reference(reference: PredictionBasisReferenceV1) -> EnginePredictionV1 {
        let basis = EnginePredictionBasisV1::new(vec![reference]).expect("valid basis");
        let facet = EnginePredictionFacetV1::available(
            EvaluationScope::new(EvaluationScopeCode::custom("acme:prediction")),
            basis,
        )
        .expect("valid facet");
        EnginePredictionV1::new(test_identity(), vec![facet]).expect("valid prediction")
    }

    fn raw_binding_wire() -> serde_json::Value {
        json!({
            "schema": RAW_SOURCE_FACTS_V1_ID,
            "primary_input": {"sha256": "00".repeat(32), "bytes": 0},
            "source_format": "glb",
            "linear_unit": {
                "state": "observed", "value": 1.0, "disposition": "preserved",
                "provenance": {"kind": "format_defined"}
            },
            "coordinate_basis": {
                "state": "observed",
                "value": {"right": "positive_x", "up": "positive_y", "forward": "positive_z"},
                "disposition": "preserved", "provenance": {"kind": "format_defined"}
            },
            "frames_per_second": {
                "state": "observed", "value": 30.0, "disposition": "preserved",
                "provenance": {"kind": "format_defined"}
            },
            "clips_coverage": {"state": "complete"},
            "constructs_coverage": {"state": "complete"},
            "resources_coverage": {"state": "unavailable", "reason": "parser_unavailable"},
            "source_skeleton_coverage": "unavailable",
            "work": {
                "inspected_rows": 0, "retained_rows": 0,
                "retained_text_bytes": 0, "max_traversal_depth": 0
            }
        })
    }

    fn minimal_profile() -> ResolvedEngineProfileV1 {
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
                let state = if id == EngineFactIdV1::AcceptedInputs {
                    EngineFactStateV1::Known(EngineFactValueV1::AcceptedFormats(vec![
                        SourceFormatV1::Glb,
                    ]))
                } else {
                    EngineFactStateV1::Unknown
                };
                EngineProfileFactV1::new(id, state)
            })
            .collect();
        ResolvedEngineProfileV1::new(
            EngineProfileSelectionV1::new("test", 1, "1", "test-importer").unwrap(),
            "urn:animsmith:engine-profile:test:1",
            facts,
            vec![],
            vec![
                EnginePrimarySourceV1::new(
                    "test-source",
                    "1",
                    "https://example.invalid/test",
                    "2026-08-20",
                    vec![EngineFactIdV1::AcceptedInputs],
                    vec![],
                )
                .unwrap(),
            ],
        )
        .unwrap()
    }

    fn minimal_provenance() -> PredictionProvenanceV1 {
        let raw: RawSourceBindingV1 = serde_json::from_value(raw_binding_wire()).unwrap();
        let closure = DependencyClosureV1::unavailable(raw.primary_input().clone());
        let profile = minimal_profile();
        let settings = ResolvedEngineSettingsV1::new(&profile, vec![], vec![]).unwrap();
        PredictionProvenanceV1::new(profile, SourceFormatV1::Glb, settings, raw, closure).unwrap()
    }

    #[test]
    fn v2_provenance_identity_commits_to_bounded_settings_coverage_and_work() {
        let raw: RawSourceBindingV1 = serde_json::from_value(raw_binding_wire()).unwrap();
        let closure = DependencyClosureV1::unavailable(raw.primary_input().clone());
        let profile = minimal_profile();
        let clips: Vec<_> = (0..ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS)
            .map(|_| EngineClipSettingsV1::new("same", Vec::new()).unwrap())
            .collect();
        let complete_settings = ResolvedEngineSettingsV2::new(
            &profile,
            vec![],
            clips.clone(),
            crate::ResolvedEngineSettingsCoverageV2::complete(),
            crate::ResolvedEngineSettingsWorkV2::new(
                ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
                ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
                ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
            ),
        )
        .unwrap();
        let partial_settings = ResolvedEngineSettingsV2::new(
            &profile,
            vec![],
            clips,
            crate::ResolvedEngineSettingsCoverageV2::actual_clip_rows_exceeded(),
            crate::ResolvedEngineSettingsWorkV2::new(
                ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS + 1,
                ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
                ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
            ),
        )
        .unwrap();
        let complete = PredictionProvenanceV2::new(
            profile.clone(),
            SourceFormatV1::Glb,
            complete_settings,
            raw.clone(),
            closure.clone(),
        )
        .unwrap();
        let partial = PredictionProvenanceV2::new(
            profile,
            SourceFormatV1::Glb,
            partial_settings,
            raw,
            closure,
        )
        .unwrap();

        assert_ne!(complete.identity(), partial.identity());
        let mut forged = serde_json::to_value(&partial).unwrap();
        forged["settings"]["work"]["actual_clip_rows_inspected"] = json!(4_096);
        assert!(serde_json::from_value::<PredictionProvenanceV2>(forged).is_err());
    }

    #[test]
    fn v2_provenance_settings_rows_stop_at_the_reserved_aggregate_n_plus_one() {
        let raw: RawSourceBindingV1 = serde_json::from_value(raw_binding_wire()).unwrap();
        let closure = DependencyClosureV1::unavailable(raw.primary_input().clone());
        let base_profile = minimal_profile();
        let profile = ResolvedEngineProfileV1::new(
            base_profile.selection().clone(),
            base_profile.fact_bundle_urn(),
            base_profile.facts().to_vec(),
            vec![EngineSettingDescriptorV1::new(
                crate::EngineSettingIdV1::ConvertUnits,
                crate::EngineSettingScopeV1::Clip,
                EngineSettingDomainV1::Boolean,
                EngineSettingApplicabilityV1::Applicable,
                EngineDefaultStatusV1::RequiredWithoutDefault,
            )],
            vec![
                EnginePrimarySourceV1::new(
                    "test-source",
                    "1",
                    "https://example.invalid/test",
                    "2026-08-20",
                    vec![EngineFactIdV1::AcceptedInputs],
                    vec![crate::EngineSettingIdV1::ConvertUnits],
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let clips = (0..ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS)
            .map(|index| {
                EngineClipSettingsV1::new(
                    format!("clip-{index:04}"),
                    vec![crate::EngineSettingRowV1::new(
                        crate::EngineSettingIdV1::ConvertUnits,
                        crate::EngineSettingValueV1::Boolean(true),
                    )],
                )
                .unwrap()
            })
            .collect();
        let settings = ResolvedEngineSettingsV2::new(
            &profile,
            vec![],
            clips,
            crate::ResolvedEngineSettingsCoverageV2::actual_clip_rows_exceeded(),
            crate::ResolvedEngineSettingsWorkV2::new(4_097, 4_096, 4_096),
        )
        .unwrap();
        let provenance = PredictionProvenanceV2::new(
            profile.clone(),
            SourceFormatV1::Glb,
            settings,
            raw,
            closure,
        )
        .unwrap();
        let mut wire = serde_json::to_value(provenance).unwrap();
        // Reserve all but 4,095 settings rows for valid raw/profile evidence.
        let raw_rows =
            PREDICTION_V1_MAX_AGGREGATE_PROVENANCE_ROWS - profile.provenance_rows() - 4_095;
        wire["raw_source"]["work"]["inspected_rows"] = json!(raw_rows);
        wire["raw_source"]["work"]["retained_rows"] = json!(raw_rows);
        // A later invalid coverage witness must not be decoded after the first
        // unadmitted settings row; the aggregate sentinel owns precedence.
        wire["settings"]["work"]["actual_clip_rows_inspected"] = json!(0);
        let result = decode_prediction_provenance_v2(&serde_json::to_string(&wire).unwrap());
        assert!(
            matches!(result, Err(PredictionDecodeError::Semantic(
            PredictionContractError::TooManyAggregateProvenanceRows { found, limit }
        )) if found == PREDICTION_V1_MAX_AGGREGATE_PROVENANCE_ROWS + 1
            && limit == PREDICTION_V1_MAX_AGGREGATE_PROVENANCE_ROWS),
            "unexpected V2 aggregate result: {result:?}"
        );
    }

    #[test]
    fn v2_catalog_allocator_reserves_later_rule_summaries_before_evaluation() {
        assert!(matches!(
            PredictionRuleDemandV2::new(
                "oversized-direct",
                PredictionFacetDemandV2::Exact(PREDICTION_V1_MAX_FACETS_PER_FILE + 1),
            ),
            Err(PredictionContractError::TooManyFacets { found, limit })
                if found == PREDICTION_V1_MAX_FACETS_PER_FILE + 1
                    && limit == PREDICTION_V1_MAX_FACETS_PER_FILE
        ));

        let demands = [
            PredictionRuleDemandV2::new(
                "first",
                PredictionFacetDemandV2::exact(PREDICTION_V1_MAX_FACETS_PER_FILE).unwrap(),
            )
            .unwrap(),
            PredictionRuleDemandV2::new("second", PredictionFacetDemandV2::exact(1).unwrap())
                .unwrap(),
            PredictionRuleDemandV2::new("third", PredictionFacetDemandV2::NPlusOne).unwrap(),
        ];
        let allocations = allocate_prediction_facets_v2(&demands).unwrap();

        assert_eq!(allocations[0].candidate_capacity(), 4_093);
        assert!(allocations[0].summary_required());
        assert_eq!(allocations[1].candidate_capacity(), 1);
        assert!(!allocations[1].summary_required());
        assert_eq!(allocations[2].candidate_capacity(), 0);
        assert!(allocations[2].summary_required());
        assert_eq!(
            allocations
                .iter()
                .map(PredictionRuleAllocationV2::emitted_slots)
                .sum::<usize>(),
            PREDICTION_V1_MAX_FACETS_PER_FILE
        );

        let sole = [PredictionRuleDemandV2::new(
            "sole",
            PredictionFacetDemandV2::exact(PREDICTION_V1_MAX_FACETS_PER_FILE).unwrap(),
        )
        .unwrap()];
        let sole_allocation = allocate_prediction_facets_v2(&sole).unwrap();
        assert_eq!(sole_allocation[0].candidate_capacity(), 4_096);
        assert!(!sole_allocation[0].summary_required());

        let duplicate = [
            PredictionRuleDemandV2::new("duplicate", PredictionFacetDemandV2::exact(1).unwrap())
                .unwrap(),
            PredictionRuleDemandV2::new("duplicate", PredictionFacetDemandV2::exact(1).unwrap())
                .unwrap(),
        ];
        assert!(matches!(
            allocate_prediction_facets_v2(&duplicate),
            Err(PredictionContractError::DuplicateProductionRule(rule)) if rule == "duplicate"
        ));
    }

    #[test]
    fn v2_unavailable_reasons_preserve_v1_selector_and_custom_vocabulary() {
        let custom = PredictionUnavailableReasonV2::custom("acme:selector_pending").unwrap();
        for reason in [
            PredictionUnavailableReasonV2::SourceSelectorNoMatch,
            PredictionUnavailableReasonV2::SourceSelectorAmbiguous,
            PredictionUnavailableReasonV2::PrimarySourceUnavailable,
            custom,
        ] {
            let wire = serde_json::to_string(&reason).unwrap();
            assert_eq!(
                serde_json::from_str::<PredictionUnavailableReasonV2>(&wire).unwrap(),
                reason
            );
        }
        let facet = EnginePredictionFacetV2::required_unavailable(
            EvaluationScope::new(EvaluationScopeCode::custom("acme:v2")),
            EnginePredictionBasisV1::new(Vec::new()).unwrap(),
            vec![
                PredictionUnavailableReasonV2::SourceSelectorNoMatch,
                PredictionUnavailableReasonV2::FacetBudgetExceeded,
            ],
        )
        .unwrap();
        assert_eq!(
            facet
                .reasons()
                .iter()
                .map(PredictionUnavailableReasonV2::as_str)
                .collect::<Vec<_>>(),
            vec!["facet_budget_exceeded", "source_selector_no_match"]
        );
    }

    fn complete_closure_with_primary_reference() -> DependencyClosureV1 {
        let primary = InputIdentity::from_bytes(b"primary");
        let mut builder =
            DependencyClosureBuilderV1::new(primary, SourceSetCoverageV1::complete(), 1);
        assert!(builder.begin_reference(0, 0));
        builder
            .push_primary(0, SourceResourceKindV1::Buffer, 0)
            .unwrap();
        builder.finish().unwrap()
    }

    fn provenance_with_raw_rows(
        raw_rows: usize,
    ) -> Result<PredictionProvenanceV1, PredictionContractError> {
        let mut raw_wire = raw_binding_wire();
        raw_wire["work"]["inspected_rows"] = json!(raw_rows);
        raw_wire["work"]["retained_rows"] = json!(raw_rows);
        let raw: RawSourceBindingV1 = serde_json::from_value(raw_wire).unwrap();
        let closure = DependencyClosureV1::unavailable(raw.primary_input().clone());
        let profile = minimal_profile();
        let settings = ResolvedEngineSettingsV1::new(&profile, vec![], vec![]).unwrap();
        PredictionProvenanceV1::new(profile, SourceFormatV1::Glb, settings, raw, closure)
    }

    #[test]
    fn raw_binding_round_trips_and_rejects_unknown_fields() {
        let wire = raw_binding_wire();
        let binding: RawSourceBindingV1 =
            serde_json::from_value(wire.clone()).expect("valid binding");
        assert_eq!(serde_json::to_value(&binding).unwrap(), wire);

        let mut invalid = wire;
        invalid["extra"] = json!(true);
        assert!(serde_json::from_value::<RawSourceBindingV1>(invalid).is_err());
    }

    #[test]
    fn dependency_closure_round_trips_strictly() {
        let closure = DependencyClosureV1::unavailable(InputIdentity::from_bytes(b"source"));
        let wire = serde_json::to_value(&closure).unwrap();
        let round_trip: DependencyClosureV1 =
            serde_json::from_value(wire.clone()).expect("valid closure");
        assert_eq!(round_trip, closure);

        let mut invalid = wire;
        invalid["unknown"] = json!(0);
        assert!(serde_json::from_value::<DependencyClosureV1>(invalid).is_err());
    }

    #[test]
    fn raw_source_acceptance_mutation_matrix_pins_scalars_and_every_coverage_domain() {
        for (field, value) in [
            ("linear_unit", json!(0.0)),
            ("frames_per_second", json!(0.0)),
        ] {
            let mut wire = raw_binding_wire();
            wire[field]["value"] = value;
            let error = serde_json::from_value::<RawSourceBindingV1>(wire).unwrap_err();
            assert_eq!(
                error.to_string(),
                PredictionContractError::RawSourceValueMismatch.to_string(),
                "raw scalar {field}"
            );
        }

        let mut wire = raw_binding_wire();
        wire["coordinate_basis"]["value"]["right"] = json!("positive_y");
        let error = serde_json::from_value::<RawSourceBindingV1>(wire).unwrap_err();
        assert_eq!(
            error.to_string(),
            PredictionContractError::RawSourceValueMismatch.to_string(),
            "raw scalar coordinate_basis"
        );

        for field in [
            "clips_coverage",
            "constructs_coverage",
            "resources_coverage",
        ] {
            let mut wire = raw_binding_wire();
            wire[field] = json!({"state": "partial"});
            let error = serde_json::from_value::<RawSourceBindingV1>(wire).unwrap_err();
            assert_eq!(
                error.to_string(),
                PredictionContractError::RawSourceFieldUnavailable(
                    "coverage state/reason".to_owned()
                )
                .to_string(),
                "raw coverage {field}"
            );
        }

        let mut wire = raw_binding_wire();
        wire["work"]["retained_rows"] = json!(1);
        let error = serde_json::from_value::<RawSourceBindingV1>(wire).unwrap_err();
        assert_eq!(
            error.to_string(),
            PredictionContractError::RawSourceFieldUnavailable(
                "raw-source work counters".to_owned()
            )
            .to_string(),
            "retained rows cannot exceed inspected rows"
        );

        let mut provenance = minimal_provenance();
        provenance.raw_source.source_skeleton_coverage = SourceSkeletonCoverage::Complete;
        assert_eq!(
            provenance.validate(),
            Err(PredictionContractError::IdentityMismatch {
                contract: PREDICTION_PROVENANCE_V1_ID,
            })
        );
    }

    #[test]
    fn dependency_closure_acceptance_mutations_pin_content_and_identity() {
        let closure = complete_closure_with_primary_reference();
        let wire = serde_json::to_value(&closure).unwrap();

        let mut changed_schema = wire.clone();
        changed_schema["schema"] = json!("urn:changed");
        let error = serde_json::from_value::<DependencyClosureV1>(changed_schema).unwrap_err();
        assert_eq!(
            error.to_string(),
            format!("dependency closure schema must be {DEPENDENCY_CLOSURE_V1_ID:?}")
        );

        let mut changed_content = wire.clone();
        changed_content["references"][0]["source_index"] = json!(1);
        let error = serde_json::from_value::<DependencyClosureV1>(changed_content).unwrap_err();
        assert_eq!(
            error.to_string(),
            "dependency closure identity does not match its preimage"
        );

        let mut changed_identity = wire;
        changed_identity["identity"]["bytes"] = json!(0);
        let error = serde_json::from_value::<DependencyClosureV1>(changed_identity).unwrap_err();
        assert_eq!(
            error.to_string(),
            "dependency closure identity does not match its preimage"
        );
    }

    #[test]
    fn prediction_round_trip_preserves_owned_scope_and_rejects_unknown_fields() {
        let prediction = prediction_with_reference(
            PredictionBasisReferenceV1::project_field(
                "project.mode",
                PredictionScalarV1::token("generic").unwrap(),
            )
            .unwrap(),
        );
        let wire = serde_json::to_value(&prediction).unwrap();
        let round_trip: EnginePredictionV1 =
            serde_json::from_value(wire.clone()).expect("valid prediction");
        assert_eq!(round_trip, prediction);

        let mut invalid = wire;
        invalid["facets"][0]["basis"]["references"][0]["unknown"] = json!(true);
        assert!(serde_json::from_value::<EnginePredictionV1>(invalid).is_err());
    }

    #[test]
    fn provenance_acceptance_mutation_matrix_pins_source_binding_contracts_and_identity() {
        let provenance = minimal_provenance();

        let mut changed = provenance.clone();
        changed.source_format = SourceFormatV1::Fbx;
        assert_eq!(
            changed.validate(),
            Err(PredictionContractError::SourceFormatMismatch)
        );

        let mut changed = provenance.clone();
        changed.raw_source.primary_input = InputIdentity::from_bytes(b"changed-primary");
        assert_eq!(
            changed.validate(),
            Err(PredictionContractError::PrimaryInputMismatch)
        );

        let mut changed = provenance.clone();
        changed.raw_source.schema = "urn:changed";
        assert_eq!(
            changed.validate(),
            Err(PredictionContractError::InvalidSchema {
                field: "provenance.raw_source.schema",
                expected: RAW_SOURCE_FACTS_V1_ID,
                found: "urn:changed".to_owned(),
            })
        );

        for index in 0..CONSUMED_CONTRACTS_V1.len() {
            let mut changed = provenance.clone();
            changed.consumed_contracts[index] = "urn:changed";
            assert_eq!(
                changed.validate(),
                Err(PredictionContractError::InvalidConsumedContracts),
                "consumed contract row {index}"
            );
        }

        let mut changed = provenance.clone();
        changed.schema = "urn:changed";
        assert_eq!(
            changed.validate(),
            Err(PredictionContractError::InvalidSchema {
                field: "provenance.schema",
                expected: PREDICTION_PROVENANCE_V1_ID,
                found: "urn:changed".to_owned(),
            })
        );

        let mut changed = provenance;
        changed.identity = PredictionProvenanceIdentityV1(InputIdentity::from_bytes(b"changed"));
        assert_eq!(
            changed.validate(),
            Err(PredictionContractError::IdentityMismatch {
                contract: PREDICTION_PROVENANCE_V1_ID,
            })
        );
    }

    #[test]
    fn basis_and_prediction_acceptance_mutation_matrix_pins_reference_scalar_schema_order_and_identity()
     {
        let provenance = minimal_provenance();
        let scope = EvaluationScope::new(EvaluationScopeCode::custom("acme:prediction"));

        let basis = EnginePredictionBasisV1::new(vec![
            PredictionBasisReferenceV1::profile_fact("accepted_inputs").unwrap(),
        ])
        .unwrap();
        let facet = EnginePredictionFacetV1::available(scope.clone(), basis).unwrap();
        let prediction =
            EnginePredictionV1::new(provenance.identity().clone(), vec![facet]).unwrap();
        assert_eq!(prediction.validate_against_provenance(&provenance), Ok(()));

        let mut changed = prediction.clone();
        let PredictionBasisReferenceV1::ProfileFact { fact_id } =
            &mut changed.facets[0].basis.references[0]
        else {
            panic!("fixture must retain a profile-fact reference");
        };
        *fact_id = "missing_fact".to_owned();
        changed.facets[0].basis =
            EnginePredictionBasisV1::new(changed.facets[0].basis.references.clone()).unwrap();
        assert_eq!(
            changed.validate_against_provenance(&provenance),
            Err(PredictionContractError::UnknownProfileFact(
                "missing_fact".to_owned()
            ))
        );

        let mut basis = EnginePredictionBasisV1::new(vec![
            PredictionBasisReferenceV1::project_field(
                "project.mode",
                PredictionScalarV1::token("generic").unwrap(),
            )
            .unwrap(),
        ])
        .unwrap();
        let PredictionBasisReferenceV1::ProjectField { value, .. } = &mut basis.references[0]
        else {
            panic!("fixture must retain a project-field reference");
        };
        *value = PredictionScalarV1::Token {
            value: String::new(),
        };
        assert_eq!(
            basis.validate(),
            Err(PredictionContractError::InvalidToken {
                field: "scalar token",
                value: String::new(),
            })
        );

        let mut basis =
            EnginePredictionBasisV1::new(vec![PredictionBasisReferenceV1::measurement(
                MeasurementPointerV1::new("/measurements/schema_version").unwrap(),
                PredictionScalarV1::UnsignedInteger { value: 15 },
            )])
            .unwrap();
        let PredictionBasisReferenceV1::Measurement { schema, .. } = &mut basis.references[0]
        else {
            panic!("fixture must retain a measurement reference");
        };
        *schema = "urn:changed";
        assert_eq!(
            basis.validate(),
            Err(PredictionContractError::InvalidSchema {
                field: "basis.measurement.schema",
                expected: MEASUREMENTS_SCHEMA_ID,
                found: "urn:changed".to_owned(),
            })
        );

        let mut basis = EnginePredictionBasisV1::new(vec![
            PredictionBasisReferenceV1::profile_fact("accepted_inputs").unwrap(),
            PredictionBasisReferenceV1::project_field(
                "project.mode",
                PredictionScalarV1::token("generic").unwrap(),
            )
            .unwrap(),
        ])
        .unwrap();
        basis.references.swap(0, 1);
        assert_eq!(
            basis.validate(),
            Err(PredictionContractError::NonCanonicalOrder(
                "basis references"
            ))
        );

        let mut basis = EnginePredictionBasisV1::new(vec![
            PredictionBasisReferenceV1::profile_fact("accepted_inputs").unwrap(),
        ])
        .unwrap();
        basis.identity = PredictionBasisIdentityV1(InputIdentity::from_bytes(b"changed"));
        assert_eq!(
            basis.validate(),
            Err(PredictionContractError::IdentityMismatch {
                contract: "engine prediction basis v1",
            })
        );

        let mut changed = prediction.clone();
        changed.schema = "urn:changed";
        assert_eq!(
            changed.validate_structure(),
            Err(PredictionContractError::InvalidSchema {
                field: "prediction.schema",
                expected: ENGINE_PREDICTION_V1_ID,
                found: "urn:changed".to_owned(),
            })
        );

        let mut changed = prediction;
        changed.provenance_identity = test_identity();
        assert_eq!(
            changed.validate_against_provenance(&provenance),
            Err(PredictionContractError::ProvenanceIdentityMismatch)
        );
    }

    #[test]
    fn measurement_pointer_bound_counts_the_measurements_root_component() {
        let at_limit = format!(
            "/measurements{}",
            "/x".repeat(PREDICTION_V1_MAX_MEASUREMENT_POINTER_COMPONENTS - 1)
        );
        MeasurementPointerV1::new(at_limit).expect("exactly 128 components is valid");

        let above_limit = format!(
            "/measurements{}",
            "/x".repeat(PREDICTION_V1_MAX_MEASUREMENT_POINTER_COMPONENTS)
        );
        assert_eq!(
            MeasurementPointerV1::new(above_limit),
            Err(
                PredictionContractError::TooManyMeasurementPointerComponents {
                    components: PREDICTION_V1_MAX_MEASUREMENT_POINTER_COMPONENTS + 1,
                    limit: PREDICTION_V1_MAX_MEASUREMENT_POINTER_COMPONENTS,
                }
            )
        );
    }

    #[test]
    fn owned_prediction_constructor_bounds_accept_n_and_reject_n_plus_one() {
        PredictionScalarV1::text("x".repeat(PREDICTION_V1_MAX_TEXT_BYTES))
            .expect("exact text limit is valid");
        assert!(matches!(
            PredictionScalarV1::text("x".repeat(PREDICTION_V1_MAX_TEXT_BYTES + 1)),
            Err(PredictionContractError::TextTooLong { .. })
        ));

        let references = (0..PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FACET)
            .map(|index| {
                PredictionBasisReferenceV1::profile_fact(format!("fact-{index:04}"))
                    .expect("bounded unique fact id")
            })
            .collect::<Vec<_>>();
        let _at_limit_basis =
            EnginePredictionBasisV1::new(references.clone()).expect("exact basis limit is valid");
        let mut above_limit_references = references;
        above_limit_references.push(PredictionBasisReferenceV1::profile_fact("fact-over").unwrap());
        assert!(matches!(
            EnginePredictionBasisV1::new(above_limit_references),
            Err(PredictionContractError::TooManyBasisReferences { .. })
        ));

        let reasons = (0..PREDICTION_V1_MAX_REASONS_PER_FACET)
            .map(|index| {
                PredictionUnavailableReasonV1::custom(format!("acme:r{index:04}"))
                    .expect("bounded unique reason")
            })
            .collect::<Vec<_>>();
        let empty_basis = EnginePredictionBasisV1::new(vec![]).unwrap();
        EnginePredictionFacetV1::required_unavailable(
            EvaluationScope::new(EvaluationScopeCode::custom("acme:unavailable")),
            empty_basis.clone(),
            reasons.clone(),
        )
        .expect("exact reason limit is valid");
        let mut above_limit_reasons = reasons;
        above_limit_reasons.push(PredictionUnavailableReasonV1::custom("acme:overflow").unwrap());
        assert!(matches!(
            EnginePredictionFacetV1::required_unavailable(
                EvaluationScope::new(EvaluationScopeCode::custom("acme:unavailable")),
                empty_basis,
                above_limit_reasons,
            ),
            Err(PredictionContractError::TooManyUnavailableReasons { .. })
        ));

        let single_reference_basis = EnginePredictionBasisV1::new(vec![
            PredictionBasisReferenceV1::profile_fact("fact-one").unwrap(),
        ])
        .unwrap();
        let facets = (0..PREDICTION_V1_MAX_FACETS_PER_FILE)
            .map(|index| {
                EnginePredictionFacetV1::available(
                    EvaluationScope::new(EvaluationScopeCode::custom("acme:facet"))
                        .subject(format!("subject-{index:04}")),
                    single_reference_basis.clone(),
                )
                .expect("bounded unique facet")
            })
            .collect::<Vec<_>>();
        let at_limit_prediction =
            EnginePredictionV1::new(test_identity(), facets).expect("exact facet limit is valid");
        let mut above_limit_facets = at_limit_prediction.facets().to_vec();
        above_limit_facets.push(
            EnginePredictionFacetV1::available(
                EvaluationScope::new(EvaluationScopeCode::custom("acme:facet"))
                    .subject("subject-over"),
                single_reference_basis,
            )
            .unwrap(),
        );
        assert!(matches!(
            EnginePredictionV1::new(test_identity(), above_limit_facets),
            Err(PredictionContractError::TooManyFacets { .. })
        ));
    }

    #[test]
    fn basis_sort_is_variant_first_then_canonical_tuple() {
        let basis = EnginePredictionBasisV1::new(vec![
            PredictionBasisReferenceV1::primary_source("source-b").unwrap(),
            PredictionBasisReferenceV1::profile_fact("fact-z").unwrap(),
            PredictionBasisReferenceV1::primary_source("source-a").unwrap(),
            PredictionBasisReferenceV1::profile_fact("fact-a").unwrap(),
        ])
        .expect("distinct bounded references form a basis");

        assert!(matches!(
            &basis.references()[0],
            PredictionBasisReferenceV1::ProfileFact { fact_id } if fact_id == "fact-a"
        ));
        assert!(matches!(
            &basis.references()[1],
            PredictionBasisReferenceV1::ProfileFact { fact_id } if fact_id == "fact-z"
        ));
        assert!(matches!(
            &basis.references()[2],
            PredictionBasisReferenceV1::PrimarySource { source_id } if source_id == "source-a"
        ));
        assert!(matches!(
            &basis.references()[3],
            PredictionBasisReferenceV1::PrimarySource { source_id } if source_id == "source-b"
        ));
    }

    #[test]
    fn new_basis_and_provenance_preimages_are_frozen() {
        let basis = EnginePredictionBasisV1::new(vec![
            PredictionBasisReferenceV1::project_field(
                "project.mode",
                PredictionScalarV1::token("generic").unwrap(),
            )
            .unwrap(),
            PredictionBasisReferenceV1::measurement(
                MeasurementPointerV1::new("/measurements/schema_version").unwrap(),
                PredictionScalarV1::UnsignedInteger { value: 15 },
            ),
        ])
        .unwrap();

        let raw: RawSourceBindingV1 = serde_json::from_value(raw_binding_wire()).unwrap();
        let closure = DependencyClosureV1::unavailable(raw.primary_input().clone());
        let profile = minimal_profile();
        let settings = ResolvedEngineSettingsV1::new(&profile, vec![], vec![]).unwrap();
        let provenance =
            PredictionProvenanceV1::new(profile, SourceFormatV1::Glb, settings, raw, closure)
                .unwrap();

        assert_eq!(
            basis.identity().input_identity().sha256(),
            "41310b60d5a1a7bfa9bf3b1cf7e41e4b33d30755da986ab5011189f076854cf2"
        );
        assert_eq!(basis.identity().input_identity().bytes(), 344);
        assert_eq!(
            provenance.identity().input_identity().sha256(),
            "3e957ce9518a3f89c76f27b399c1ff594ec4adc5c10ac529de0f4df570bd693d"
        );
        assert_eq!(provenance.identity().input_identity().bytes(), 3_342);
    }

    #[test]
    fn provenance_rejects_raw_resource_and_closure_coverage_mismatch() {
        let mut raw_wire = raw_binding_wire();
        raw_wire["resources_coverage"] = json!({"state": "complete"});
        let raw: RawSourceBindingV1 = serde_json::from_value(raw_wire).unwrap();
        let closure = DependencyClosureV1::unavailable(raw.primary_input().clone());
        let profile = minimal_profile();
        let settings = ResolvedEngineSettingsV1::new(&profile, vec![], vec![]).unwrap();

        assert_eq!(
            PredictionProvenanceV1::new(profile, SourceFormatV1::Glb, settings, raw, closure,),
            Err(PredictionContractError::DependencyClosureCoverageMismatch)
        );
    }

    #[test]
    fn aggregate_provenance_row_bound_accepts_n_and_rejects_n_plus_one_on_write_and_read() {
        let fixed_profile_rows = {
            let profile = minimal_profile();
            profile.facts().len()
                + profile.setting_descriptors().len()
                + profile.primary_sources().len()
        };
        let raw_rows_at_limit = PREDICTION_V1_MAX_AGGREGATE_PROVENANCE_ROWS - fixed_profile_rows;
        let at_limit = provenance_with_raw_rows(raw_rows_at_limit)
            .expect("exact aggregate provenance-row limit is valid");
        let at_limit_wire = serde_json::to_value(&at_limit).unwrap();
        let round_trip: PredictionProvenanceV1 = serde_json::from_value(at_limit_wire.clone())
            .expect("exact aggregate provenance-row limit reads back");
        assert_eq!(round_trip, at_limit);

        assert_eq!(
            provenance_with_raw_rows(raw_rows_at_limit + 1),
            Err(PredictionContractError::TooManyAggregateProvenanceRows {
                found: PREDICTION_V1_MAX_AGGREGATE_PROVENANCE_ROWS + 1,
                limit: PREDICTION_V1_MAX_AGGREGATE_PROVENANCE_ROWS,
            })
        );

        let mut above_limit_wire = at_limit_wire;
        above_limit_wire["raw_source"]["work"]["inspected_rows"] = json!(raw_rows_at_limit + 1);
        above_limit_wire["raw_source"]["work"]["retained_rows"] = json!(raw_rows_at_limit + 1);
        let error = serde_json::from_value::<PredictionProvenanceV1>(above_limit_wire)
            .expect_err("N+1 aggregate provenance rows must fail before identity comparison");
        assert!(
            error
                .to_string()
                .contains("prediction provenance retains 65537 rows"),
            "unexpected read error: {error}"
        );
    }

    #[test]
    fn measurement_references_distinguish_missing_object_and_wrong_scalar() {
        let measurements = MeasurementContract::new(BTreeMap::new(), AssetMeasurements::default())
            .expect("empty measurement fixture is valid");
        let correct = prediction_with_reference(PredictionBasisReferenceV1::measurement(
            MeasurementPointerV1::new("/measurements/schema_version").unwrap(),
            PredictionScalarV1::UnsignedInteger { value: 15 },
        ));
        assert_eq!(
            correct.validate_measurement_references(&measurements),
            Ok(())
        );

        let wrong = prediction_with_reference(PredictionBasisReferenceV1::measurement(
            MeasurementPointerV1::new("/measurements/schema_version").unwrap(),
            PredictionScalarV1::UnsignedInteger { value: 14 },
        ));
        assert!(matches!(
            wrong.validate_measurement_references(&measurements),
            Err(PredictionContractError::MeasurementValueMismatch(_))
        ));

        let missing = prediction_with_reference(PredictionBasisReferenceV1::measurement(
            MeasurementPointerV1::new("/measurements/not_present").unwrap(),
            PredictionScalarV1::Null,
        ));
        assert!(matches!(
            missing.validate_measurement_references(&measurements),
            Err(PredictionContractError::MeasurementPointerMissing(_))
        ));

        let object = prediction_with_reference(PredictionBasisReferenceV1::measurement(
            MeasurementPointerV1::new("/measurements").unwrap(),
            PredictionScalarV1::Null,
        ));
        assert!(matches!(
            object.validate_measurement_references(&measurements),
            Err(PredictionContractError::MeasurementPointerNotScalar(_))
        ));
    }

    #[test]
    fn measurement_reference_batch_traverses_once_across_predictions() {
        let measurements = MeasurementContract::new(BTreeMap::new(), AssetMeasurements::default())
            .expect("empty measurement fixture is valid");
        let first = prediction_with_reference(PredictionBasisReferenceV1::measurement(
            MeasurementPointerV1::new("/measurements/schema_version").unwrap(),
            PredictionScalarV1::UnsignedInteger { value: 15 },
        ));
        let second = prediction_with_reference(PredictionBasisReferenceV1::measurement(
            MeasurementPointerV1::new("/measurements/schema_version").unwrap(),
            PredictionScalarV1::UnsignedInteger { value: 15 },
        ));

        assert_eq!(
            validate_measurement_references_batch_impl(&measurements, [(3, &first), (8, &second)],)
                .expect("both predictions reference the same exact scalar"),
            1,
        );

        let without_measurements = prediction_with_reference(
            PredictionBasisReferenceV1::project_field(
                "project.mode",
                PredictionScalarV1::token("generic").unwrap(),
            )
            .unwrap(),
        );
        assert_eq!(
            validate_measurement_references_batch_impl(
                &measurements,
                [(3, &without_measurements)],
            )
            .expect("no measurement references need no traversal"),
            0,
        );
    }

    #[test]
    fn consumed_contracts_reject_n_plus_one_before_decoding_null_or_large_tail() {
        let provenance = minimal_provenance();
        let mut wire = serde_json::to_value(provenance).unwrap();
        let contracts = wire["consumed_contracts"].as_array_mut().unwrap();
        assert_eq!(contracts.len(), CONSUMED_CONTRACTS_V1.len());
        contracts.push(serde_json::Value::Null);
        contracts.extend((0..10_000).map(|_| serde_json::json!("")));
        let result = decode_prediction_provenance_v1(&serde_json::to_string(&wire).unwrap());
        assert!(matches!(
            result,
            Err(PredictionDecodeError::Semantic(
                PredictionContractError::InvalidConsumedContracts
            ))
        ));
    }

    #[test]
    fn prediction_sequences_reject_n_plus_one_before_decoding_null_sentinels() {
        let prediction = prediction_with_reference(
            PredictionBasisReferenceV1::project_field(
                "project.mode",
                PredictionScalarV1::token("generic").unwrap(),
            )
            .unwrap(),
        );
        let base = serde_json::to_value(prediction).unwrap();

        let mut facets = vec![base["facets"][0].clone(); PREDICTION_V1_MAX_FACETS_PER_FILE];
        facets.push(serde_json::Value::Null);
        let mut over = base.clone();
        over["facets"] = facets.into();
        assert!(matches!(
            decode_engine_prediction_v1(
                &serde_json::to_string(&over).unwrap(),
                PREDICTION_V1_MAX_FACETS_PER_FILE,
                PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE,
            ),
            Err(PredictionDecodeError::Semantic(
                PredictionContractError::TooManyFacets {
                    found,
                    limit: PREDICTION_V1_MAX_FACETS_PER_FILE,
                }
            )) if found == PREDICTION_V1_MAX_FACETS_PER_FILE + 1
        ));

        let mut reasons = vec![
            serde_json::json!("project_intent_unavailable");
            PREDICTION_V1_MAX_REASONS_PER_FACET
        ];
        reasons.push(serde_json::Value::Null);
        let mut over = base.clone();
        over["facets"][0]["reasons"] = reasons.into();
        assert!(matches!(
            decode_engine_prediction_v1(
                &serde_json::to_string(&over).unwrap(),
                PREDICTION_V1_MAX_FACETS_PER_FILE,
                PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE,
            ),
            Err(PredictionDecodeError::Semantic(
                PredictionContractError::TooManyUnavailableReasons {
                    found,
                    limit: PREDICTION_V1_MAX_REASONS_PER_FACET,
                }
            )) if found == PREDICTION_V1_MAX_REASONS_PER_FACET + 1
        ));

        let reference = base["facets"][0]["basis"]["references"][0].clone();
        let mut references = vec![reference; PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FACET];
        references.push(serde_json::Value::Null);
        let mut over = base;
        over["facets"][0]["basis"]["references"] = references.into();
        assert!(matches!(
            decode_engine_prediction_v1(
                &serde_json::to_string(&over).unwrap(),
                PREDICTION_V1_MAX_FACETS_PER_FILE,
                PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE,
            ),
            Err(PredictionDecodeError::Semantic(
                PredictionContractError::TooManyBasisReferences {
                    found,
                    limit: PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FACET,
                }
            )) if found == PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FACET + 1
        ));
    }

    #[test]
    fn prediction_basis_aggregate_stops_at_cross_facet_n_plus_one() {
        let prediction = prediction_with_reference(
            PredictionBasisReferenceV1::project_field(
                "project.mode",
                PredictionScalarV1::token("generic").unwrap(),
            )
            .unwrap(),
        );
        let mut wire = serde_json::to_value(prediction).unwrap();
        let reference = wire["facets"][0]["basis"]["references"][0].clone();
        let mut full_facet = wire["facets"][0].clone();
        full_facet["basis"]["references"] =
            vec![reference; PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FACET].into();
        let facet_count = PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE
            / PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FACET;
        let exact_facets = vec![full_facet.clone(); facet_count];
        wire["facets"] = exact_facets.clone().into();
        assert!(!matches!(
            decode_engine_prediction_v1(
                &serde_json::to_string(&wire).unwrap(),
                PREDICTION_V1_MAX_FACETS_PER_FILE,
                PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE,
            ),
            Err(PredictionDecodeError::TooManyFileBasisReferences)
        ));

        let mut sentinel_facet = full_facet.clone();
        sentinel_facet["basis"]["references"] = serde_json::json!([null]);
        let mut over_facets = exact_facets.clone();
        over_facets.push(sentinel_facet);
        wire["facets"] = over_facets.into();
        assert!(matches!(
            decode_engine_prediction_v1(
                &serde_json::to_string(&wire).unwrap(),
                PREDICTION_V1_MAX_FACETS_PER_FILE,
                PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE,
            ),
            Err(PredictionDecodeError::TooManyFileBasisReferences)
        ));

        let reference = wire["facets"][0]["basis"]["references"][0].clone();
        let mut locally_oversized = vec![reference; PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FACET];
        locally_oversized.push(serde_json::Value::Null);
        full_facet["basis"]["references"] = locally_oversized.into();
        let mut over_facets = exact_facets;
        over_facets.push(full_facet);
        wire["facets"] = over_facets.into();
        assert!(matches!(
            decode_engine_prediction_v1(
                &serde_json::to_string(&wire).unwrap(),
                PREDICTION_V1_MAX_FACETS_PER_FILE,
                PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE,
            ),
            Err(PredictionDecodeError::Semantic(
                PredictionContractError::TooManyBasisReferences {
                    found,
                    limit: PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FACET,
                }
            )) if found == PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FACET + 1
        ));
    }

    #[test]
    fn standalone_prediction_round_trips_above_the_file_basis_budget() {
        let basis = EnginePredictionBasisV1::new(
            (0..PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FACET)
                .map(|index| {
                    PredictionBasisReferenceV1::project_field(
                        format!("project.standalone.{index:04}"),
                        PredictionScalarV1::Null,
                    )
                    .unwrap()
                })
                .collect(),
        )
        .unwrap();
        let facets = (0..17)
            .map(|index| {
                EnginePredictionFacetV1::available(
                    EvaluationScope::new(EvaluationScopeCode::custom("acme:standalone"))
                        .subject(format!("subject-{index:02}")),
                    basis.clone(),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let prediction = EnginePredictionV1::new(test_identity(), facets).unwrap();
        assert!(prediction.basis_reference_count() > PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FILE);
        let round_trip: EnginePredictionV1 =
            serde_json::from_slice(&serde_json::to_vec(&prediction).unwrap()).unwrap();
        assert_eq!(round_trip, prediction);
    }

    #[test]
    fn provenance_collection_aggregate_stops_before_settings_n_plus_one() {
        let base_profile = minimal_profile();
        let sources = (0..PREDICTION_V1_MAX_FACETS_PER_FILE)
            .map(|index| {
                EnginePrimarySourceV1::new(
                    format!("source-{index:04}"),
                    "1",
                    format!("https://example.invalid/{index:04}"),
                    "2026-08-20",
                    vec![EngineFactIdV1::AcceptedInputs],
                    vec![],
                )
                .unwrap()
            })
            .collect();
        let profile = ResolvedEngineProfileV1::new(
            base_profile.selection().clone(),
            base_profile.fact_bundle_urn(),
            base_profile.facts().to_vec(),
            base_profile.setting_descriptors().to_vec(),
            sources,
        )
        .unwrap();
        assert_eq!(profile.provenance_rows(), 4_110);
        let raw: RawSourceBindingV1 = serde_json::from_value(raw_binding_wire()).unwrap();
        let closure = DependencyClosureV1::unavailable(raw.primary_input().clone());
        let settings = ResolvedEngineSettingsV1::new(&profile, vec![], vec![]).unwrap();
        let provenance =
            PredictionProvenanceV1::new(profile, SourceFormatV1::Glb, settings, raw, closure)
                .unwrap();
        let mut wire = serde_json::to_value(provenance).unwrap();
        let setting = serde_json::json!({"id": "convert_units", "value": {"boolean": true}});
        let mut document_settings = vec![setting.clone(); PREDICTION_V1_MAX_FACETS_PER_FILE - 1];
        document_settings.push(serde_json::Value::Null);
        wire["settings"]["document_settings"] = document_settings.into();
        let full_clip = serde_json::json!({
            "clip_name": "clip",
            "settings": vec![
                setting.clone();
                PREDICTION_V1_MAX_BASIS_REFERENCES_PER_FACET
            ]
        });
        let mut clips = vec![full_clip; 13];
        let last = serde_json::json!({
            "clip_name": "clip",
            "settings": vec![setting; 4_083]
        });
        clips.push(last);
        wire["settings"]["clips"] = clips.into();
        assert_eq!(
            wire["settings"]["document_settings"]
                .as_array()
                .unwrap()
                .len()
                + wire["settings"]["clips"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|clip| clip["settings"].as_array().unwrap().len())
                    .sum::<usize>(),
            61_427,
        );
        let result = decode_prediction_provenance_v1(&serde_json::to_string(&wire).unwrap());
        assert!(
            matches!(
                result,
                Err(PredictionDecodeError::Semantic(
                    PredictionContractError::TooManyAggregateProvenanceRows {
                        found,
                        limit: PREDICTION_V1_MAX_AGGREGATE_PROVENANCE_ROWS,
                    }
                )) if found == PREDICTION_V1_MAX_AGGREGATE_PROVENANCE_ROWS + 1
            ),
            "unexpected provenance aggregate result: {result:?}"
        );
    }

    #[test]
    fn raw_rows_are_reserved_before_profile_and_settings_n_plus_one() {
        let provenance = minimal_provenance();
        let profile_rows = provenance.profile().provenance_rows();
        let mut wire = serde_json::to_value(provenance).unwrap();

        wire["raw_source"]["work"]["inspected_rows"] =
            serde_json::json!(PREDICTION_V1_MAX_AGGREGATE_PROVENANCE_ROWS);
        wire["raw_source"]["work"]["retained_rows"] =
            serde_json::json!(PREDICTION_V1_MAX_AGGREGATE_PROVENANCE_ROWS);
        wire["profile"]["facts"][0] = serde_json::Value::Null;
        let result = decode_prediction_provenance_v1(&serde_json::to_string(&wire).unwrap());
        assert!(matches!(
            result,
            Err(PredictionDecodeError::Semantic(
                PredictionContractError::TooManyAggregateProvenanceRows {
                    found,
                    limit: PREDICTION_V1_MAX_AGGREGATE_PROVENANCE_ROWS,
                }
            )) if found == PREDICTION_V1_MAX_AGGREGATE_PROVENANCE_ROWS + 1
        ));

        let provenance = minimal_provenance();
        let mut wire = serde_json::to_value(provenance).unwrap();
        let raw_rows = PREDICTION_V1_MAX_AGGREGATE_PROVENANCE_ROWS - profile_rows - 1;
        wire["raw_source"]["work"]["inspected_rows"] = serde_json::json!(raw_rows);
        wire["raw_source"]["work"]["retained_rows"] = serde_json::json!(raw_rows);
        wire["settings"]["document_settings"] = serde_json::json!([
            {"id": "convert_units", "value": {"boolean": true}},
            null
        ]);
        let result = decode_prediction_provenance_v1(&serde_json::to_string(&wire).unwrap());
        assert!(matches!(
            result,
            Err(PredictionDecodeError::Semantic(
                PredictionContractError::TooManyAggregateProvenanceRows {
                    found,
                    limit: PREDICTION_V1_MAX_AGGREGATE_PROVENANCE_ROWS,
                }
            )) if found == PREDICTION_V1_MAX_AGGREGATE_PROVENANCE_ROWS + 1
        ));
    }

    #[test]
    fn raw_row_reservation_preserves_profile_and_settings_error_precedence() {
        let provenance = minimal_provenance();
        let mut wire = serde_json::to_value(provenance).unwrap();
        wire["raw_source"]["work"]["retained_rows"] = serde_json::json!(1);
        wire["profile"]["schema"] = serde_json::json!("wrong-profile");
        let result = decode_prediction_provenance_v1(&serde_json::to_string(&wire).unwrap());
        assert!(matches!(
            result,
            Err(PredictionDecodeError::Semantic(
                PredictionContractError::InvalidEngineContract(
                    EngineContractError::InvalidSchema {
                        field: "profile.schema",
                        ..
                    }
                )
            ))
        ));

        let provenance = minimal_provenance();
        let mut wire = serde_json::to_value(provenance).unwrap();
        wire["raw_source"]["work"]["retained_rows"] = serde_json::json!(1);
        wire["settings"]["schema"] = serde_json::json!("wrong-settings");
        let result = decode_prediction_provenance_v1(&serde_json::to_string(&wire).unwrap());
        assert!(matches!(
            result,
            Err(PredictionDecodeError::Semantic(
                PredictionContractError::InvalidEngineContract(
                    EngineContractError::InvalidSchema {
                        field: "settings.schema",
                        ..
                    }
                )
            ))
        ));
    }
}
