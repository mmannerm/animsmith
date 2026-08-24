//! Internal producer and strict reader types for collection-output V1.
//!
//! This is deliberately a CLI-local contract.  Core owns the validated
//! collection declaration vocabulary; this module owns the command's evidence
//! envelope, including bounded read-back of an untrusted previously emitted
//! envelope.

use animsmith_core::measure::{ClipMeasurements, MeasurementAvailability};
use animsmith_core::metrics::circular_phase_spread;
use animsmith_core::{
    COLLECTION_MANIFEST_V1_MAX_AGGREGATE_MEMBERS, COLLECTION_MANIFEST_V1_MAX_AGGREGATE_WORK,
    COLLECTION_MANIFEST_V1_MAX_CLIPS, COLLECTION_MANIFEST_V1_MAX_RUNTIME_SETS,
    COLLECTION_MANIFEST_V1_MAX_SOURCES, COLLECTION_MANIFEST_V1_MAX_TAKE_NAME_BYTES, CollectionIdV1,
    CollectionLogicalIdV1, CollectionRuntimeSetKindV1, CollectionSourceKeyV1,
    DependencyResourceKeyV1, InputIdentity, LintEnvelope, MeasurementReportInput,
    ResourceKeySyntaxV1, ToolInfo,
};
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Read, Write};

pub(crate) const COLLECTION_OUTPUT_V1_ID: &str = "urn:animsmith:schema:collection-output:1";
pub(crate) const COLLECTION_OUTPUT_V1_SCHEMA_VERSION: u32 = 1;
pub(crate) const COLLECTION_OUTPUT_V1_BUDGET_ID: &str = "urn:animsmith:collection-output-budget:1";
pub(crate) const COLLECTION_OUTPUT_V1_MAX_SOURCE_BYTES: u64 = 1024 * 1024 * 1024;
pub(crate) const COLLECTION_OUTPUT_V1_MAX_AGGREGATE_SOURCE_BYTES: u64 = 16 * 1024 * 1024 * 1024;
pub(crate) const COLLECTION_OUTPUT_V1_MAX_SERIALIZED_BYTES: u64 = 256 * 1024 * 1024;
const COLLECTION_OUTPUT_V1_MAX_NORMALIZED_CLIP_NAME_BYTES: usize =
    COLLECTION_MANIFEST_V1_MAX_TAKE_NAME_BYTES
        + 1
        + decimal_digits(COLLECTION_MANIFEST_V1_MAX_CLIPS.saturating_sub(1) as u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CollectionOutputBudgetV1 {
    id: &'static str,
    max_source_bytes: u64,
    max_aggregate_source_bytes: u64,
    max_serialized_bytes: u64,
    max_sources: usize,
    max_clips: usize,
    max_runtime_sets: usize,
    max_aggregate_members: usize,
    max_aggregate_work: usize,
}

impl CollectionOutputBudgetV1 {
    pub(crate) const fn v1() -> Self {
        Self {
            id: COLLECTION_OUTPUT_V1_BUDGET_ID,
            max_source_bytes: COLLECTION_OUTPUT_V1_MAX_SOURCE_BYTES,
            max_aggregate_source_bytes: COLLECTION_OUTPUT_V1_MAX_AGGREGATE_SOURCE_BYTES,
            max_serialized_bytes: COLLECTION_OUTPUT_V1_MAX_SERIALIZED_BYTES,
            max_sources: COLLECTION_MANIFEST_V1_MAX_SOURCES,
            max_clips: COLLECTION_MANIFEST_V1_MAX_CLIPS,
            max_runtime_sets: COLLECTION_MANIFEST_V1_MAX_RUNTIME_SETS,
            max_aggregate_members: COLLECTION_MANIFEST_V1_MAX_AGGREGATE_MEMBERS,
            max_aggregate_work: COLLECTION_MANIFEST_V1_MAX_AGGREGATE_WORK,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct CollectionManifestIdentity {
    schema: &'static str,
    schema_version: u32,
    collection_id: String,
    input: InputIdentity,
}

impl CollectionManifestIdentity {
    pub(crate) fn new(collection_id: impl Into<String>, input: InputIdentity) -> Self {
        Self {
            schema: animsmith_core::COLLECTION_MANIFEST_V1_ID,
            schema_version: animsmith_core::COLLECTION_MANIFEST_V1_SCHEMA_VERSION,
            collection_id: collection_id.into(),
            input,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub(crate) enum SourceInputState {
    Available {
        input: InputIdentity,
    },
    Unavailable {
        reason: SourceUnavailableReason,
        /// Bytes consumed before this source became unavailable (N+1 on a limit).
        inspected_bytes: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SourceUnavailableReason {
    Missing,
    Unreadable,
    TooLarge,
    AggregateExhausted,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub(crate) enum DigestPinState {
    Unpinned,
    Matched {
        expected_sha256: String,
    },
    Mismatched {
        expected_sha256: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        observed_sha256: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub(crate) enum ConfigState {
    Default,
    Explicit {
        locator: String,
        input: InputIdentity,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub(crate) enum LoaderState {
    Ready,
    Unavailable { reason: LoaderUnavailableReason },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LoaderUnavailableReason {
    SourceUnavailable,
    UnsupportedFormat,
    MalformedInput,
    DependencyUnavailable,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub(crate) enum DocumentResult {
    Available { envelope: Box<LintEnvelope> },
    Unavailable { reason: DocumentUnavailableReason },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DocumentUnavailableReason {
    #[serde(rename = "source_unavailable")]
    Source,
    #[serde(rename = "loader_unavailable")]
    Loader,
    #[serde(rename = "nested_output_unavailable")]
    NestedOutput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TakeInventoryState {
    Complete,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub(crate) enum TakeNameState {
    Available { value: String },
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub(crate) enum NormalizedClipState {
    Available { index: u32, name: String },
    Unavailable,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ObservedTake {
    source_take_index: u32,
    name: TakeNameState,
    normalized: NormalizedClipState,
}

impl ObservedTake {
    pub(crate) fn new(
        source_take_index: u32,
        name: impl Into<String>,
        normalized_clip_index: u32,
        normalized_name: impl Into<String>,
    ) -> Self {
        Self {
            source_take_index,
            name: TakeNameState::Available { value: name.into() },
            normalized: NormalizedClipState::Available {
                index: normalized_clip_index,
                name: normalized_name.into(),
            },
        }
    }

    pub(crate) fn with_unavailable(
        source_take_index: u32,
        name: TakeNameState,
        normalized: NormalizedClipState,
    ) -> Self {
        Self {
            source_take_index,
            name,
            normalized,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CollectionSourceRecord {
    key: String,
    locator: String,
    input: SourceInputState,
    digest: DigestPinState,
    config: ConfigState,
    loader: LoaderState,
    take_inventory: TakeInventoryState,
    observed_takes: Vec<ObservedTake>,
    result: DocumentResult,
}

impl CollectionSourceRecord {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        key: impl Into<String>,
        locator: impl Into<String>,
        input: SourceInputState,
        digest: DigestPinState,
        config: ConfigState,
        loader: LoaderState,
        take_inventory: TakeInventoryState,
        observed_takes: Vec<ObservedTake>,
        result: DocumentResult,
    ) -> Self {
        Self {
            key: key.into(),
            locator: locator.into(),
            input,
            digest,
            config,
            loader,
            take_inventory,
            observed_takes,
            result,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct MeasurementReference {
    source: String,
    normalized_clip_index: u32,
    measurement_key: String,
}

/// Whether the ordinary name-addressed lint result can be tied to this exact
/// physical take.  The measurement itself is always carried in the clip row;
/// duplicate authored names make only this name-keyed reference unavailable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub(crate) enum CheckReferenceState {
    Available {
        reference: MeasurementReference,
    },
    Unavailable {
        reason: CheckReferenceUnavailableReason,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CheckReferenceUnavailableReason {
    DuplicateEmbeddedTakeName,
    NestedOutputUnavailable,
}

impl MeasurementReference {
    pub(crate) fn new(
        source: impl Into<String>,
        normalized_clip_index: u32,
        measurement_key: impl Into<String>,
    ) -> Self {
        Self {
            source: source.into(),
            normalized_clip_index,
            measurement_key: measurement_key.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub(crate) enum ClipBindingState {
    Established {
        observed_source_take_index: u32,
        observed_take_name: String,
        normalized_clip_index: u32,
        /// Exact source-indexed value, not a v15 name-keyed lookup.
        measurements: Box<ClipMeasurements>,
        check_reference: CheckReferenceState,
    },
    Unavailable {
        reason: ClipUnavailableReason,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ClipUnavailableReason {
    SourceUnavailable,
    DigestMismatched,
    LoaderUnavailable,
    DocumentUnavailable,
    TakeInventoryUnavailable,
    TakeIndexMissing,
    TakeNameUnavailable,
    TakeNameMismatched,
    NormalizedClipUnavailable,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CollectionClipRecord {
    id: String,
    source: String,
    take_index: u32,
    take_name: String,
    binding: ClipBindingState,
}

impl CollectionClipRecord {
    pub(crate) fn new(
        id: impl Into<String>,
        source: impl Into<String>,
        take_index: u32,
        take_name: impl Into<String>,
        binding: ClipBindingState,
    ) -> Self {
        Self {
            id: id.into(),
            source: source.into(),
            take_index,
            take_name: take_name.into(),
            binding,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub(crate) enum RuntimeSetMemberState {
    Established,
    Unavailable { reason: ClipUnavailableReason },
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RuntimeSetMember {
    id: String,
    resolution: RuntimeSetMemberState,
    #[serde(skip_serializing_if = "Option::is_none")]
    gait_phase: Option<GaitPhaseMemberEvidence>,
}

impl RuntimeSetMember {
    pub(crate) fn new(id: impl Into<String>, resolution: RuntimeSetMemberState) -> Self {
        Self {
            id: id.into(),
            resolution,
            gait_phase: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RuntimeSetLifecycle {
    Complete,
    Incomplete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RuntimeSetDecision {
    NotEvaluated,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CollectionRuntimeSetRecord {
    id: String,
    kind: CollectionRuntimeSetKindV1,
    members: Vec<RuntimeSetMember>,
    lifecycle: RuntimeSetLifecycle,
    decision: RuntimeSetDecision,
    gaps: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    evidence: Option<RuntimeSetEvidence>,
}

/// Raw set evidence.  This is intentionally an object so later collection
/// evidence (for example root-travel facts) can be a sibling of gait phase
/// without inventing another result-state shape.
#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
struct RuntimeSetEvidence {
    gait_phase: GaitPhaseEvidence,
}

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
struct GaitPhaseEvidence {
    lifecycle: RuntimeSetLifecycle,
    members_measured: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    phase_spread: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    spread_basis: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
struct GaitPhaseMemberEvidence {
    availability: MeasurementAvailability,
    #[serde(skip_serializing_if = "Option::is_none")]
    phase: Option<f64>,
}

const GAIT_PHASE_SPREAD_BASIS: &str = "max_circular_deviation_from_mean";

impl CollectionRuntimeSetRecord {
    pub(crate) fn new(
        id: impl Into<String>,
        kind: CollectionRuntimeSetKindV1,
        members: Vec<RuntimeSetMember>,
    ) -> Self {
        let gaps = members
            .iter()
            .filter(|member| !matches!(member.resolution, RuntimeSetMemberState::Established))
            .map(|member| member.id.clone())
            .collect::<Vec<_>>();
        let lifecycle = if gaps.is_empty() {
            RuntimeSetLifecycle::Complete
        } else {
            RuntimeSetLifecycle::Incomplete
        };
        Self {
            id: id.into(),
            kind,
            members,
            lifecycle,
            decision: RuntimeSetDecision::NotEvaluated,
            gaps,
            evidence: None,
        }
    }

    fn populate_evidence(
        &mut self,
        clips: &BTreeMap<&str, &CollectionClipRecord>,
    ) -> Result<(), CollectionOutputError> {
        if self.kind != CollectionRuntimeSetKindV1::GaitGroup {
            return Ok(());
        }
        let member_count = self.members.len();
        let mut phases = Vec::with_capacity(member_count);
        for member in &mut self.members {
            let clip =
                clips
                    .get(member.id.as_str())
                    .ok_or(CollectionOutputError::Contradictory(
                        "runtime-set member has no clip",
                    ))?;
            let (availability, phase) = match (&member.resolution, &clip.binding) {
                (
                    RuntimeSetMemberState::Established,
                    ClipBindingState::Established { measurements, .. },
                ) => gait_phase_from_measurements(measurements),
                (
                    RuntimeSetMemberState::Unavailable { reason },
                    ClipBindingState::Unavailable {
                        reason: clip_reason,
                    },
                ) if reason == clip_reason => (MeasurementAvailability::Unavailable, None),
                _ => {
                    return Err(CollectionOutputError::Contradictory(
                        "runtime-set member resolution mismatch",
                    ));
                }
            };
            member.gait_phase = Some(GaitPhaseMemberEvidence {
                availability,
                phase,
            });
            if availability == MeasurementAvailability::Measured {
                phases.push((
                    member.id.clone(),
                    phase.ok_or(CollectionOutputError::Contradictory(
                        "measured gait phase is missing",
                    ))?,
                ));
            }
        }
        let phase_spread = if phases.len() == member_count {
            phases.sort_by(|left, right| left.0.cmp(&right.0));
            Some(circular_phase_spread(
                &phases.iter().map(|(_, phase)| *phase).collect::<Vec<_>>(),
            ))
        } else {
            None
        };
        self.evidence = Some(RuntimeSetEvidence {
            gait_phase: GaitPhaseEvidence {
                lifecycle: if phase_spread.is_some() {
                    RuntimeSetLifecycle::Complete
                } else {
                    RuntimeSetLifecycle::Incomplete
                },
                members_measured: phases.len(),
                spread_basis: phase_spread.map(|_| GAIT_PHASE_SPREAD_BASIS),
                phase_spread,
            },
        });
        Ok(())
    }
}

fn gait_phase_from_measurements(
    measurements: &ClipMeasurements,
) -> (MeasurementAvailability, Option<f64>) {
    match measurements.gait_availability {
        MeasurementAvailability::Measured => match measurements.gait.as_ref() {
            Some(gait) if gait.phase_availability == MeasurementAvailability::Measured => {
                (MeasurementAvailability::Measured, gait.phase)
            }
            Some(gait) => (gait.phase_availability, None),
            None => (MeasurementAvailability::Unavailable, None),
        },
        MeasurementAvailability::NotApplicable => (MeasurementAvailability::NotApplicable, None),
        MeasurementAvailability::Unavailable => (MeasurementAvailability::Unavailable, None),
        _ => (MeasurementAvailability::Unavailable, None),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CollectionSummary {
    sources: usize,
    readable_sources: usize,
    established_sources: usize,
    clips: usize,
    established_clips: usize,
    runtime_sets: usize,
    complete_runtime_sets: usize,
    incomplete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CollectionWork {
    manifest_rows: usize,
    runtime_set_members: usize,
    aggregate_work: usize,
    primary_source_bytes: u64,
    serialized_bytes: u64,
}

impl CollectionWork {
    pub(crate) fn new(
        sources: usize,
        clips: usize,
        runtime_sets: usize,
        runtime_set_members: usize,
        primary_source_bytes: u64,
        serialized_bytes: u64,
    ) -> Result<Self, CollectionOutputError> {
        let manifest_rows = sources
            .checked_add(clips)
            .and_then(|value| value.checked_add(runtime_sets))
            .ok_or(CollectionOutputError::Contradictory("work overflow"))?;
        let aggregate_work = manifest_rows
            .checked_add(runtime_set_members)
            .ok_or(CollectionOutputError::Contradictory("work overflow"))?;
        Ok(Self {
            manifest_rows,
            runtime_set_members,
            aggregate_work,
            primary_source_bytes,
            serialized_bytes,
        })
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct CollectionOutput {
    schema_version: u32,
    schema: &'static str,
    tool: ToolInfo,
    command: &'static str,
    manifest: CollectionManifestIdentity,
    budget: CollectionOutputBudgetV1,
    summary: CollectionSummary,
    work: CollectionWork,
    sources: Vec<CollectionSourceRecord>,
    clips: Vec<CollectionClipRecord>,
    runtime_sets: Vec<CollectionRuntimeSetRecord>,
}

impl CollectionOutput {
    pub(crate) fn new(
        tool: ToolInfo,
        manifest: CollectionManifestIdentity,
        sources: Vec<CollectionSourceRecord>,
        clips: Vec<CollectionClipRecord>,
        mut runtime_sets: Vec<CollectionRuntimeSetRecord>,
        primary_source_bytes: u64,
        serialized_bytes: u64,
    ) -> Result<Self, CollectionOutputError> {
        let clip_rows = clips
            .iter()
            .map(|clip| (clip.id.as_str(), clip))
            .collect::<BTreeMap<_, _>>();
        for set in &mut runtime_sets {
            set.populate_evidence(&clip_rows)?;
        }
        let summary = summarize(&sources, &clips, &runtime_sets)?;
        let work = CollectionWork::new(
            sources.len(),
            clips.len(),
            runtime_sets.len(),
            runtime_sets.iter().map(|set| set.members.len()).sum(),
            primary_source_bytes,
            serialized_bytes,
        )?;
        let output = Self {
            schema_version: COLLECTION_OUTPUT_V1_SCHEMA_VERSION,
            schema: COLLECTION_OUTPUT_V1_ID,
            tool,
            command: "collection lint",
            manifest,
            budget: CollectionOutputBudgetV1::v1(),
            summary,
            work,
            sources,
            clips,
            runtime_sets,
        };
        validate_producer(&output)?;
        Ok(output)
    }

    /// Serialize the envelope while converging its self-reported byte count.
    /// The count includes exactly these JSON bytes, not a buffered write size.
    pub(crate) fn render_json_vec(&mut self) -> Result<Vec<u8>, CollectionOutputError> {
        // Only the decimal width of `serialized_bytes` can change the next
        // length. Starting at zero, there are at most as many transitions as
        // the decimal digits in the immutable output cap, plus one stable pass.
        for _ in 0..=decimal_digits(COLLECTION_OUTPUT_V1_MAX_SERIALIZED_BYTES) {
            let bytes = serialize_json_bounded(self, COLLECTION_OUTPUT_V1_MAX_SERIALIZED_BYTES)?;
            if self.work.serialized_bytes == bytes.len() as u64 {
                return Ok(bytes);
            }
            self.work.serialized_bytes = bytes.len() as u64;
        }
        Err(CollectionOutputError::Contradictory(
            "serialized byte count did not converge",
        ))
    }
}

const fn decimal_digits(mut value: u64) -> usize {
    let mut digits = 1;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

struct BoundedJsonCounter {
    bytes: u64,
    terminal: u64,
    exceeded: bool,
}

impl BoundedJsonCounter {
    fn new(limit: u64) -> Self {
        Self {
            bytes: 0,
            terminal: limit.saturating_add(1),
            exceeded: false,
        }
    }
}

impl Write for BoundedJsonCounter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let remaining = self.terminal.saturating_sub(self.bytes);
        if remaining == 0 {
            self.exceeded = true;
            return Err(io::Error::other("collection output limit reached"));
        }
        let consumed = usize::try_from(remaining)
            .unwrap_or(usize::MAX)
            .min(buffer.len());
        self.bytes += consumed as u64;
        Ok(consumed)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn serialize_json_bounded<T: Serialize>(
    value: &T,
    limit: u64,
) -> Result<Vec<u8>, CollectionOutputError> {
    // Count through a streaming N+1 sink first. Only an accepted length is
    // allocated, so serialization cannot trigger Vec's geometric growth past
    // the contract bound.
    let mut counter = BoundedJsonCounter::new(limit);
    let counted = serde_json::to_writer(&mut counter, value);
    if counter.exceeded || counter.bytes > limit {
        return Err(CollectionOutputError::TooLarge);
    }
    counted.map_err(|_| CollectionOutputError::Json)?;
    let length = usize::try_from(counter.bytes).map_err(|_| CollectionOutputError::TooLarge)?;
    let mut bytes = vec![0u8; length];
    let mut cursor = io::Cursor::new(bytes.as_mut_slice());
    serde_json::to_writer(&mut cursor, value).map_err(|_| CollectionOutputError::Json)?;
    if cursor.position() != counter.bytes {
        return Err(CollectionOutputError::Json);
    }
    Ok(bytes)
}

#[derive(Debug)]
pub(crate) enum CollectionOutputError {
    Contradictory(&'static str),
    Malformed,
    TooLarge,
    Read,
    Json,
}

impl std::fmt::Display for CollectionOutputError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Contradictory(reason) => {
                write!(formatter, "collection output is contradictory: {reason}")
            }
            Self::Malformed => formatter.write_str("collection output is malformed"),
            Self::TooLarge => {
                formatter.write_str("collection output exceeds its bounded reader limit")
            }
            Self::Read => formatter.write_str("collection output cannot be read"),
            Self::Json => formatter.write_str("collection output JSON is invalid"),
        }
    }
}

impl std::error::Error for CollectionOutputError {}

pub(crate) fn read_collection_output(
    mut reader: impl Read,
) -> Result<CollectionOutputInput, CollectionOutputError> {
    let mut bytes = Vec::new();
    let mut limited = reader
        .by_ref()
        .take(COLLECTION_OUTPUT_V1_MAX_SERIALIZED_BYTES + 1);
    limited
        .read_to_end(&mut bytes)
        .map_err(|_| CollectionOutputError::Read)?;
    if bytes.len() as u64 > COLLECTION_OUTPUT_V1_MAX_SERIALIZED_BYTES {
        return Err(CollectionOutputError::TooLarge);
    }
    let wire = serde_json::from_slice::<CollectionOutputWire>(&bytes)
        .map_err(|_| CollectionOutputError::Json)?;
    let output = CollectionOutputInput { wire };
    output.validate(bytes.len() as u64)?;
    Ok(output)
}

pub(crate) struct CollectionOutputInput {
    wire: CollectionOutputWire,
}

impl CollectionOutputInput {
    #[cfg(test)]
    pub(crate) fn source_count(&self) -> usize {
        self.wire.sources.len()
    }

    #[cfg(test)]
    pub(crate) fn clip_count(&self) -> usize {
        self.wire.clips.len()
    }

    fn validate(&self, read_bytes: u64) -> Result<(), CollectionOutputError> {
        let wire = &self.wire;
        if wire.schema_version != COLLECTION_OUTPUT_V1_SCHEMA_VERSION
            || wire.schema != COLLECTION_OUTPUT_V1_ID
            || wire.command != "collection lint"
            || !valid_tool(&wire.tool)
            || wire.manifest.schema != animsmith_core::COLLECTION_MANIFEST_V1_ID
            || wire.manifest.schema_version != animsmith_core::COLLECTION_MANIFEST_V1_SCHEMA_VERSION
            || CollectionIdV1::new(wire.manifest.collection_id.clone()).is_err()
            || !valid_identity(&wire.manifest.input)
            || !valid_budget(&wire.budget)
        {
            return Err(CollectionOutputError::Malformed);
        }
        if wire.sources.len() > COLLECTION_MANIFEST_V1_MAX_SOURCES
            || wire.clips.len() > COLLECTION_MANIFEST_V1_MAX_CLIPS
            || wire.runtime_sets.len() > COLLECTION_MANIFEST_V1_MAX_RUNTIME_SETS
        {
            return Err(CollectionOutputError::Malformed);
        }
        let mut source_keys = BTreeSet::new();
        let mut sources = BTreeMap::new();
        for source in &wire.sources {
            if CollectionSourceKeyV1::new(source.key.clone()).is_err()
                || !valid_locator(&source.locator)
                || !source_keys.insert(source.key.clone())
            {
                return Err(CollectionOutputError::Malformed);
            }
            validate_source(source)?;
            sources.insert(source.key.as_str(), source);
        }
        if !strictly_sorted(wire.sources.iter().map(|source| source.key.as_str())) {
            return Err(CollectionOutputError::Malformed);
        }
        let mut clip_ids = BTreeSet::new();
        let mut clip_states = BTreeMap::new();
        for clip in &wire.clips {
            if CollectionLogicalIdV1::new(clip.id.clone()).is_err()
                || clip.take_name.is_empty()
                || clip.take_name.len() > COLLECTION_MANIFEST_V1_MAX_TAKE_NAME_BYTES
                || !clip_ids.insert(clip.id.clone())
                || !sources.contains_key(clip.source.as_str())
            {
                return Err(CollectionOutputError::Malformed);
            }
            validate_clip(clip, sources[clip.source.as_str()])?;
            clip_states.insert(clip.id.as_str(), &clip.binding);
        }
        if !strictly_sorted(wire.clips.iter().map(|clip| clip.id.as_str())) {
            return Err(CollectionOutputError::Malformed);
        }
        let mut set_ids = BTreeSet::new();
        let mut members = 0usize;
        for set in &wire.runtime_sets {
            if CollectionLogicalIdV1::new(set.id.clone()).is_err()
                || !set_ids.insert(set.id.clone())
            {
                return Err(CollectionOutputError::Malformed);
            }
            members = members
                .checked_add(set.members.len())
                .ok_or(CollectionOutputError::Malformed)?;
            validate_set(set, &clip_states)?;
        }
        if !strictly_sorted(wire.runtime_sets.iter().map(|set| set.id.as_str()))
            || members > COLLECTION_MANIFEST_V1_MAX_AGGREGATE_MEMBERS
        {
            return Err(CollectionOutputError::Malformed);
        }
        let expected_summary = summarize_wire(&wire.sources, &wire.clips, &wire.runtime_sets)?;
        if wire.summary != expected_summary {
            return Err(CollectionOutputError::Malformed);
        }
        let primary_bytes =
            validate_primary_source_sequence(wire.sources.iter().map(
                |source| match &source.input {
                    SourceInputStateWire::Available { input } => (false, input.bytes),
                    SourceInputStateWire::Unavailable {
                        reason,
                        inspected_bytes,
                    } => (
                        reason == &SourceUnavailableReason::AggregateExhausted,
                        *inspected_bytes,
                    ),
                },
            ))
            .ok_or(CollectionOutputError::Malformed)?;
        let expected_work = CollectionWork::new(
            wire.sources.len(),
            wire.clips.len(),
            wire.runtime_sets.len(),
            members,
            primary_bytes,
            read_bytes,
        )?;
        if wire.work != expected_work
            || primary_bytes > COLLECTION_OUTPUT_V1_MAX_AGGREGATE_SOURCE_BYTES + 1
            || wire.sources.iter().any(|source| {
                matches!(&source.input,
                SourceInputStateWire::Available { input }
                    if input.bytes > COLLECTION_OUTPUT_V1_MAX_SOURCE_BYTES)
                    || matches!(&source.input,
                    SourceInputStateWire::Unavailable { inspected_bytes, .. }
                    if *inspected_bytes > COLLECTION_OUTPUT_V1_MAX_SOURCE_BYTES + 1)
            })
        {
            return Err(CollectionOutputError::Malformed);
        }
        Ok(())
    }
}

fn summarize(
    sources: &[CollectionSourceRecord],
    clips: &[CollectionClipRecord],
    sets: &[CollectionRuntimeSetRecord],
) -> Result<CollectionSummary, CollectionOutputError> {
    let readable_sources = sources
        .iter()
        .filter(|source| matches!(source.input, SourceInputState::Available { .. }))
        .count();
    let established_sources = sources
        .iter()
        .filter(|source| {
            matches!(source.input, SourceInputState::Available { .. })
                && matches!(
                    source.digest,
                    DigestPinState::Unpinned | DigestPinState::Matched { .. }
                )
                && matches!(source.loader, LoaderState::Ready)
                && matches!(source.result, DocumentResult::Available { .. })
        })
        .count();
    let established_clips = clips
        .iter()
        .filter(|clip| matches!(clip.binding, ClipBindingState::Established { .. }))
        .count();
    let complete_runtime_sets = sets
        .iter()
        .filter(|set| set.lifecycle == RuntimeSetLifecycle::Complete)
        .count();
    Ok(CollectionSummary {
        sources: sources.len(),
        readable_sources,
        established_sources,
        clips: clips.len(),
        established_clips,
        runtime_sets: sets.len(),
        complete_runtime_sets,
        incomplete: established_sources != sources.len()
            || established_clips != clips.len()
            || complete_runtime_sets != sets.len(),
    })
}

fn validate_producer(output: &CollectionOutput) -> Result<(), CollectionOutputError> {
    let source_keys = output
        .sources
        .iter()
        .map(|source| source.key.as_str())
        .collect::<BTreeSet<_>>();
    if source_keys.len() != output.sources.len()
        || !strictly_sorted(output.sources.iter().map(|source| source.key.as_str()))
        || !strictly_sorted(output.clips.iter().map(|clip| clip.id.as_str()))
        || !strictly_sorted(output.runtime_sets.iter().map(|set| set.id.as_str()))
    {
        return Err(CollectionOutputError::Contradictory(
            "rows must be unique and canonical",
        ));
    }
    if output.work.primary_source_bytes > COLLECTION_OUTPUT_V1_MAX_AGGREGATE_SOURCE_BYTES + 1
        || output.work.serialized_bytes > COLLECTION_OUTPUT_V1_MAX_SERIALIZED_BYTES
        || output.work.aggregate_work > COLLECTION_MANIFEST_V1_MAX_AGGREGATE_WORK
    {
        return Err(CollectionOutputError::Contradictory("budget exceeded"));
    }
    for source in &output.sources {
        if matches!(&source.input, SourceInputState::Available { input } if input.bytes() > COLLECTION_OUTPUT_V1_MAX_SOURCE_BYTES)
            || matches!(&source.input, SourceInputState::Unavailable { inspected_bytes, .. } if *inspected_bytes > COLLECTION_OUTPUT_V1_MAX_SOURCE_BYTES + 1)
        {
            return Err(CollectionOutputError::Contradictory(
                "source budget exceeded",
            ));
        }
        let has_oversized_normalized_name = source.observed_takes.iter().any(|take| {
            matches!(
                &take.normalized,
                NormalizedClipState::Available { name, .. }
                    if name.len() > COLLECTION_MANIFEST_V1_MAX_TAKE_NAME_BYTES
            )
        });
        let nested_output_unavailable = matches!(
            source.result,
            DocumentResult::Unavailable {
                reason: DocumentUnavailableReason::NestedOutput
            }
        );
        if has_oversized_normalized_name != nested_output_unavailable {
            return Err(CollectionOutputError::Contradictory(
                "nested-output availability mismatch",
            ));
        }
    }
    let observed_primary_bytes =
        validate_primary_source_sequence(output.sources.iter().map(|source| match &source.input {
            SourceInputState::Available { input } => (false, input.bytes()),
            SourceInputState::Unavailable {
                reason,
                inspected_bytes,
            } => (
                reason == &SourceUnavailableReason::AggregateExhausted,
                *inspected_bytes,
            ),
        }))
        .ok_or(CollectionOutputError::Contradictory(
            "invalid primary-source sequence",
        ))?;
    if observed_primary_bytes != output.work.primary_source_bytes {
        return Err(CollectionOutputError::Contradictory(
            "primary-source byte counter mismatch",
        ));
    }
    let expected = summarize(&output.sources, &output.clips, &output.runtime_sets)?;
    if expected != output.summary {
        return Err(CollectionOutputError::Contradictory("summary mismatch"));
    }
    Ok(())
}

fn validate_primary_source_sequence(sources: impl Iterator<Item = (bool, u64)>) -> Option<u64> {
    let mut total = 0u64;
    let mut exhausted = false;
    for (aggregate_exhausted, inspected_bytes) in sources {
        if aggregate_exhausted {
            if inspected_bytes != 0 || total != COLLECTION_OUTPUT_V1_MAX_AGGREGATE_SOURCE_BYTES + 1
            {
                return None;
            }
            exhausted = true;
        } else {
            if exhausted {
                return None;
            }
            total = total.checked_add(inspected_bytes)?;
            if total > COLLECTION_OUTPUT_V1_MAX_AGGREGATE_SOURCE_BYTES + 1 {
                return None;
            }
        }
    }
    Some(total)
}

fn strictly_sorted<'a>(mut values: impl Iterator<Item = &'a str>) -> bool {
    let Some(mut previous) = values.next() else {
        return true;
    };
    for value in values {
        if previous >= value {
            return false;
        }
        previous = value;
    }
    true
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CollectionOutputWire {
    schema_version: u32,
    schema: String,
    tool: ToolWire,
    command: String,
    manifest: ManifestWire,
    budget: BudgetWire,
    summary: CollectionSummary,
    work: CollectionWork,
    sources: Vec<CollectionSourceWire>,
    clips: Vec<CollectionClipWire>,
    runtime_sets: Vec<RuntimeSetWire>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolWire {
    name: String,
    version: String,
    source: ToolSourceWire,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolSourceWire {
    revision: NullableRevision,
    dirty: NullableBool,
}
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum NullableRevision {
    Value(String),
    Null(()),
}
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum NullableBool {
    Value(bool),
    Null(()),
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestWire {
    schema: String,
    schema_version: u32,
    collection_id: String,
    input: IdentityWire,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct IdentityWire {
    sha256: String,
    bytes: u64,
}
#[derive(Debug, Deserialize)]
struct NestedEnvelopeCommandWire {
    command: String,
}
#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct BudgetWire {
    id: String,
    max_source_bytes: u64,
    max_aggregate_source_bytes: u64,
    max_serialized_bytes: u64,
    max_sources: usize,
    max_clips: usize,
    max_runtime_sets: usize,
    max_aggregate_members: usize,
    max_aggregate_work: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CollectionSourceWire {
    key: String,
    locator: String,
    input: SourceInputStateWire,
    digest: DigestPinStateWire,
    config: ConfigStateWire,
    loader: LoaderStateWire,
    take_inventory: TakeInventoryState,
    observed_takes: Vec<ObservedTakeWire>,
    result: DocumentResultWire,
}
#[derive(Debug, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
enum SourceInputStateWire {
    Available {
        input: IdentityWire,
    },
    Unavailable {
        reason: SourceUnavailableReason,
        inspected_bytes: u64,
    },
}
#[derive(Debug, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
enum DigestPinStateWire {
    Unpinned,
    Matched {
        expected_sha256: String,
    },
    Mismatched {
        expected_sha256: String,
        observed_sha256: Option<String>,
    },
}
#[derive(Debug, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
enum ConfigStateWire {
    Default,
    Explicit {
        locator: String,
        input: IdentityWire,
    },
}
#[derive(Debug, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
enum LoaderStateWire {
    Ready,
    Unavailable { reason: LoaderUnavailableReason },
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ObservedTakeWire {
    source_take_index: u32,
    name: TakeNameState,
    normalized: NormalizedClipState,
}
#[derive(Debug)]
enum DocumentResultWire {
    Available { envelope: Box<RawValue> },
    Unavailable { reason: DocumentUnavailableReason },
}

impl<'de> Deserialize<'de> for DocumentResultWire {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct ResultVisitor;

        impl<'de> Visitor<'de> for ResultVisitor {
            type Value = DocumentResultWire;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a strict collection document-result object")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut state = None::<String>;
                let mut envelope = None::<Box<RawValue>>;
                let mut reason = None::<DocumentUnavailableReason>;
                while let Some(field) = map.next_key::<String>()? {
                    match field.as_str() {
                        "state" if state.is_none() => state = Some(map.next_value()?),
                        "envelope" if envelope.is_none() => envelope = Some(map.next_value()?),
                        "reason" if reason.is_none() => reason = Some(map.next_value()?),
                        "state" => return Err(serde::de::Error::duplicate_field("state")),
                        "envelope" => {
                            return Err(serde::de::Error::duplicate_field("envelope"));
                        }
                        "reason" => return Err(serde::de::Error::duplicate_field("reason")),
                        _ => {
                            return Err(serde::de::Error::unknown_field(
                                &field,
                                &["state", "envelope", "reason"],
                            ));
                        }
                    }
                }
                match (state.as_deref(), envelope, reason) {
                    (Some("available"), Some(envelope), None) => {
                        Ok(DocumentResultWire::Available { envelope })
                    }
                    (Some("unavailable"), None, Some(reason)) => {
                        Ok(DocumentResultWire::Unavailable { reason })
                    }
                    _ => Err(serde::de::Error::custom(
                        "document result state contradicts its fields",
                    )),
                }
            }
        }

        deserializer.deserialize_map(ResultVisitor)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CollectionClipWire {
    id: String,
    source: String,
    take_index: u32,
    take_name: String,
    binding: ClipBindingStateWire,
}
#[derive(Debug, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
enum ClipBindingStateWire {
    Established {
        observed_source_take_index: u32,
        observed_take_name: String,
        normalized_clip_index: u32,
        measurements: Box<ClipMeasurements>,
        check_reference: CheckReferenceStateWire,
    },
    Unavailable {
        reason: ClipUnavailableReason,
    },
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MeasurementReferenceWire {
    source: String,
    normalized_clip_index: u32,
    measurement_key: String,
}
#[derive(Debug, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
enum CheckReferenceStateWire {
    Available {
        reference: MeasurementReferenceWire,
    },
    Unavailable {
        reason: CheckReferenceUnavailableReason,
    },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeSetWire {
    id: String,
    #[serde(rename = "kind")]
    _kind: CollectionRuntimeSetKindV1,
    members: Vec<RuntimeSetMemberWire>,
    lifecycle: RuntimeSetLifecycle,
    decision: RuntimeSetDecision,
    gaps: Vec<String>,
    evidence: Option<RuntimeSetEvidenceWire>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeSetEvidenceWire {
    gait_phase: GaitPhaseEvidenceWire,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GaitPhaseEvidenceWire {
    lifecycle: RuntimeSetLifecycle,
    members_measured: usize,
    phase_spread: Option<f64>,
    spread_basis: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GaitPhaseMemberEvidenceWire {
    availability: MeasurementAvailability,
    phase: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeSetMemberWire {
    id: String,
    resolution: RuntimeSetMemberStateWire,
    gait_phase: Option<GaitPhaseMemberEvidenceWire>,
}
#[derive(Debug, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
enum RuntimeSetMemberStateWire {
    Established,
    Unavailable { reason: ClipUnavailableReason },
}

fn valid_budget(budget: &BudgetWire) -> bool {
    budget.id == COLLECTION_OUTPUT_V1_BUDGET_ID
        && budget.max_source_bytes == COLLECTION_OUTPUT_V1_MAX_SOURCE_BYTES
        && budget.max_aggregate_source_bytes == COLLECTION_OUTPUT_V1_MAX_AGGREGATE_SOURCE_BYTES
        && budget.max_serialized_bytes == COLLECTION_OUTPUT_V1_MAX_SERIALIZED_BYTES
        && budget.max_sources == COLLECTION_MANIFEST_V1_MAX_SOURCES
        && budget.max_clips == COLLECTION_MANIFEST_V1_MAX_CLIPS
        && budget.max_runtime_sets == COLLECTION_MANIFEST_V1_MAX_RUNTIME_SETS
        && budget.max_aggregate_members == COLLECTION_MANIFEST_V1_MAX_AGGREGATE_MEMBERS
        && budget.max_aggregate_work == COLLECTION_MANIFEST_V1_MAX_AGGREGATE_WORK
}
fn valid_tool(tool: &ToolWire) -> bool {
    let revision_valid = match &tool.source.revision {
        NullableRevision::Value(revision) => {
            revision.len() == 40 && revision.bytes().all(|byte| byte.is_ascii_hexdigit())
        }
        NullableRevision::Null(()) => true,
    };
    let _dirty = match tool.source.dirty {
        NullableBool::Value(value) => Some(value),
        NullableBool::Null(()) => None,
    };
    tool.name == "animsmith" && valid_semver_shape(&tool.version) && revision_valid
}
fn valid_semver_shape(version: &str) -> bool {
    let (without_build, build) = version
        .split_once('+')
        .map_or((version, None), |(left, right)| (left, Some(right)));
    if build.is_some_and(|value| value.is_empty() || !valid_version_suffix(value))
        || without_build.matches('+').count() != 0
    {
        return false;
    }
    let (core, pre) = without_build
        .split_once('-')
        .map_or((without_build, None), |(left, right)| (left, Some(right)));
    pre.is_none_or(|value| !value.is_empty() && valid_version_suffix(value))
        && core.split('.').count() == 3
        && core
            .split('.')
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}
fn valid_version_suffix(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
}
fn valid_locator(value: &str) -> bool {
    DependencyResourceKeyV1::from_source_str(value, ResourceKeySyntaxV1::ParserRelativePath).is_ok()
}
fn valid_identity(identity: &IdentityWire) -> bool {
    identity.sha256.len() == 64
        && identity
            .sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (byte.is_ascii_lowercase() && byte <= b'f'))
}
fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (byte.is_ascii_lowercase() && byte <= b'f'))
}

fn validate_source(source: &CollectionSourceWire) -> Result<(), CollectionOutputError> {
    if let SourceInputStateWire::Unavailable {
        reason,
        inspected_bytes,
    } = &source.input
    {
        match reason {
            SourceUnavailableReason::Missing if *inspected_bytes == 0 => {}
            SourceUnavailableReason::Unreadable
                if *inspected_bytes <= COLLECTION_OUTPUT_V1_MAX_SOURCE_BYTES => {}
            SourceUnavailableReason::TooLarge if *inspected_bytes > 0 => {}
            SourceUnavailableReason::AggregateExhausted if *inspected_bytes == 0 => {}
            _ => return Err(CollectionOutputError::Malformed),
        }
    }
    match (
        &source.input,
        &source.digest,
        &source.loader,
        &source.result,
    ) {
        (
            SourceInputStateWire::Available { input },
            DigestPinStateWire::Matched { expected_sha256 },
            LoaderStateWire::Ready,
            DocumentResultWire::Available { envelope },
        ) if valid_identity(input)
            && valid_digest(expected_sha256)
            && expected_sha256 == &input.sha256 =>
        {
            validate_envelope(envelope, input)?;
        }
        (
            SourceInputStateWire::Available { input },
            DigestPinStateWire::Unpinned,
            LoaderStateWire::Ready,
            DocumentResultWire::Available { envelope },
        ) if valid_identity(input) => validate_envelope(envelope, input)?,
        (
            SourceInputStateWire::Available { input },
            DigestPinStateWire::Mismatched {
                expected_sha256,
                observed_sha256,
            },
            LoaderStateWire::Ready,
            DocumentResultWire::Available { envelope },
        ) if valid_identity(input)
            && valid_digest(expected_sha256)
            && observed_sha256.as_deref() == Some(input.sha256.as_str()) =>
        {
            validate_envelope(envelope, input)?
        }
        (
            SourceInputStateWire::Available { input },
            DigestPinStateWire::Matched { expected_sha256 },
            LoaderStateWire::Unavailable { reason },
            DocumentResultWire::Unavailable {
                reason: DocumentUnavailableReason::Loader,
            },
        ) if valid_identity(input)
            && expected_sha256 == &input.sha256
            && valid_digest(expected_sha256)
            && reason != &LoaderUnavailableReason::SourceUnavailable => {}
        (
            SourceInputStateWire::Available { input },
            DigestPinStateWire::Unpinned,
            LoaderStateWire::Unavailable { reason },
            DocumentResultWire::Unavailable {
                reason: DocumentUnavailableReason::Loader,
            },
        ) if valid_identity(input) && reason != &LoaderUnavailableReason::SourceUnavailable => {}
        (
            SourceInputStateWire::Available { input },
            DigestPinStateWire::Mismatched {
                expected_sha256,
                observed_sha256,
            },
            LoaderStateWire::Unavailable { reason },
            DocumentResultWire::Unavailable {
                reason: DocumentUnavailableReason::Loader,
            },
        ) if valid_identity(input)
            && valid_digest(expected_sha256)
            && observed_sha256.as_deref() == Some(input.sha256.as_str())
            && reason != &LoaderUnavailableReason::SourceUnavailable => {}
        (
            SourceInputStateWire::Available { input },
            DigestPinStateWire::Matched { expected_sha256 },
            LoaderStateWire::Ready,
            DocumentResultWire::Unavailable {
                reason: DocumentUnavailableReason::NestedOutput,
            },
        ) if valid_identity(input)
            && expected_sha256 == &input.sha256
            && valid_digest(expected_sha256) => {}
        (
            SourceInputStateWire::Available { input },
            DigestPinStateWire::Unpinned,
            LoaderStateWire::Ready,
            DocumentResultWire::Unavailable {
                reason: DocumentUnavailableReason::NestedOutput,
            },
        ) if valid_identity(input) => {}
        (
            SourceInputStateWire::Available { input },
            DigestPinStateWire::Mismatched {
                expected_sha256,
                observed_sha256,
            },
            LoaderStateWire::Ready,
            DocumentResultWire::Unavailable {
                reason: DocumentUnavailableReason::NestedOutput,
            },
        ) if valid_identity(input)
            && valid_digest(expected_sha256)
            && observed_sha256.as_deref() == Some(input.sha256.as_str()) => {}
        (
            SourceInputStateWire::Unavailable { .. },
            DigestPinStateWire::Unpinned,
            LoaderStateWire::Unavailable {
                reason: LoaderUnavailableReason::SourceUnavailable,
            },
            DocumentResultWire::Unavailable {
                reason: DocumentUnavailableReason::Source,
            },
        ) => {}
        (
            SourceInputStateWire::Unavailable { .. },
            DigestPinStateWire::Mismatched {
                expected_sha256,
                observed_sha256: None,
            },
            LoaderStateWire::Unavailable {
                reason: LoaderUnavailableReason::SourceUnavailable,
            },
            DocumentResultWire::Unavailable {
                reason: DocumentUnavailableReason::Source,
            },
        ) if valid_digest(expected_sha256) => {}
        _ => return Err(CollectionOutputError::Malformed),
    }
    if let ConfigStateWire::Explicit { locator, input } = &source.config
        && (!valid_locator(locator) || !valid_identity(input))
    {
        return Err(CollectionOutputError::Malformed);
    }
    if matches!(source.input, SourceInputStateWire::Unavailable { .. })
        && (source.take_inventory != TakeInventoryState::Unavailable
            || !source.observed_takes.is_empty())
    {
        return Err(CollectionOutputError::Malformed);
    }
    let mut source_indices = BTreeSet::new();
    let mut normalized_indices = BTreeSet::new();
    for take in &source.observed_takes {
        if !source_indices.insert(take.source_take_index) {
            return Err(CollectionOutputError::Malformed);
        }
        match &take.name {
            TakeNameState::Available { value }
                if value.is_empty() || value.len() > COLLECTION_MANIFEST_V1_MAX_TAKE_NAME_BYTES =>
            {
                return Err(CollectionOutputError::Malformed);
            }
            TakeNameState::Available { .. } | TakeNameState::Unavailable => {}
        }
        match &take.normalized {
            NormalizedClipState::Available { index, name }
                if name.is_empty()
                    || name.len() > COLLECTION_OUTPUT_V1_MAX_NORMALIZED_CLIP_NAME_BYTES
                    || !normalized_indices.insert(*index) =>
            {
                return Err(CollectionOutputError::Malformed);
            }
            NormalizedClipState::Available { .. } | NormalizedClipState::Unavailable => {}
        }
    }
    let has_oversized_normalized_name = source.observed_takes.iter().any(|take| {
        matches!(
            &take.normalized,
            NormalizedClipState::Available { name, .. }
                if name.len() > COLLECTION_MANIFEST_V1_MAX_TAKE_NAME_BYTES
        )
    });
    let nested_output_unavailable = matches!(
        source.result,
        DocumentResultWire::Unavailable {
            reason: DocumentUnavailableReason::NestedOutput
        }
    );
    if has_oversized_normalized_name != nested_output_unavailable {
        return Err(CollectionOutputError::Malformed);
    }
    Ok(())
}
fn validate_envelope(raw: &RawValue, source: &IdentityWire) -> Result<(), CollectionOutputError> {
    let command: NestedEnvelopeCommandWire =
        serde_json::from_str(raw.get()).map_err(|_| CollectionOutputError::Malformed)?;
    if command.command != "lint" {
        return Err(CollectionOutputError::Malformed);
    }
    let report: MeasurementReportInput =
        serde_json::from_str(raw.get()).map_err(|_| CollectionOutputError::Malformed)?;
    let files = report
        .into_files()
        .map_err(|_| CollectionOutputError::Malformed)?;
    if files.len() != 1
        || files[0].input().sha256() != source.sha256
        || files[0].input().bytes() != source.bytes
    {
        return Err(CollectionOutputError::Malformed);
    }
    Ok(())
}
fn validate_clip(
    clip: &CollectionClipWire,
    source: &CollectionSourceWire,
) -> Result<(), CollectionOutputError> {
    match &clip.binding {
        ClipBindingStateWire::Established {
            observed_source_take_index,
            observed_take_name,
            normalized_clip_index,
            measurements,
            check_reference,
        } => {
            let observed = source
                .observed_takes
                .iter()
                .find(|take| take.source_take_index == *observed_source_take_index)
                .ok_or(CollectionOutputError::Malformed)?;
            let (TakeNameState::Available { value: observed_name }, NormalizedClipState::Available { index: observed_index, name: observed_normalized_name }) = (&observed.name, &observed.normalized) else {
                return Err(CollectionOutputError::Malformed);
            };
            if observed_source_take_index != &clip.take_index
                || observed_take_name != &clip.take_name
                || observed_name != &clip.take_name
                || observed_index != normalized_clip_index
            {
                return Err(CollectionOutputError::Malformed);
            }
            if !matches!(
                source.digest,
                DigestPinStateWire::Unpinned | DigestPinStateWire::Matched { .. }
            ) || !matches!(source.loader, LoaderStateWire::Ready)
            {
                return Err(CollectionOutputError::Malformed);
            }
            match check_reference {
                CheckReferenceStateWire::Available { reference } => {
                    let DocumentResultWire::Available { envelope } = &source.result else {
                        return Err(CollectionOutputError::Malformed);
                    };
                    let report: MeasurementReportInput = serde_json::from_str(envelope.get())
                        .map_err(|_| CollectionOutputError::Malformed)?;
                    let files = report
                        .into_files()
                        .map_err(|_| CollectionOutputError::Malformed)?;
                    if files.len() != 1
                        || reference.source != clip.source
                        || reference.normalized_clip_index != *normalized_clip_index
                        || reference.measurement_key.len()
                            > COLLECTION_MANIFEST_V1_MAX_TAKE_NAME_BYTES
                        || reference.measurement_key.as_str() != observed_normalized_name.as_str()
                        || !files[0]
                            .measurements()
                            .clips()
                            .contains_key(reference.measurement_key.as_str())
                    {
                        return Err(CollectionOutputError::Malformed);
                    }
                    let nested = serde_json::to_value(
                        files[0]
                            .measurements()
                            .clips()
                            .get(reference.measurement_key.as_str())
                            .ok_or(CollectionOutputError::Malformed)?,
                    )
                    .map_err(|_| CollectionOutputError::Malformed)?;
                    let indexed = serde_json::to_value(measurements)
                        .map_err(|_| CollectionOutputError::Malformed)?;
                    if nested != indexed {
                        return Err(CollectionOutputError::Malformed);
                    }
                }
                CheckReferenceStateWire::Unavailable {
                    reason: CheckReferenceUnavailableReason::DuplicateEmbeddedTakeName,
                } => {
                    if !matches!(source.result, DocumentResultWire::Available { .. }) {
                        return Err(CollectionOutputError::Malformed);
                    }
                    let duplicate_count = source
                        .observed_takes
                        .iter()
                        .filter(|take| {
                            matches!(
                                &take.normalized,
                                NormalizedClipState::Available { name, .. }
                                    if name == observed_normalized_name
                            )
                        })
                        .count();
                    if duplicate_count < 2 {
                        return Err(CollectionOutputError::Malformed);
                    }
                }
                CheckReferenceStateWire::Unavailable {
                    reason: CheckReferenceUnavailableReason::NestedOutputUnavailable,
                } => {
                    if !matches!(
                        source.result,
                        DocumentResultWire::Unavailable {
                            reason: DocumentUnavailableReason::NestedOutput
                        }
                    ) {
                        return Err(CollectionOutputError::Malformed);
                    }
                }
            }
        }
        ClipBindingStateWire::Unavailable { reason } => match reason {
            ClipUnavailableReason::SourceUnavailable => {
                if !matches!(source.input, SourceInputStateWire::Unavailable { .. }) { return Err(CollectionOutputError::Malformed); }
            }
            ClipUnavailableReason::DigestMismatched => {
                if !matches!(source.digest, DigestPinStateWire::Mismatched { .. }) { return Err(CollectionOutputError::Malformed); }
            }
            ClipUnavailableReason::LoaderUnavailable | ClipUnavailableReason::DocumentUnavailable => {
                if !matches!(source.loader, LoaderStateWire::Unavailable { .. }) && !matches!(source.result, DocumentResultWire::Unavailable { .. }) { return Err(CollectionOutputError::Malformed); }
            }
            ClipUnavailableReason::TakeInventoryUnavailable => {
                if source.take_inventory != TakeInventoryState::Unavailable { return Err(CollectionOutputError::Malformed); }
            }
            ClipUnavailableReason::TakeIndexMissing => {
                if source.take_inventory != TakeInventoryState::Complete || source.observed_takes.iter().any(|take| take.source_take_index == clip.take_index) { return Err(CollectionOutputError::Malformed); }
            }
            ClipUnavailableReason::TakeNameUnavailable => {
                if source.take_inventory != TakeInventoryState::Complete || !source.observed_takes.iter().any(|take| take.source_take_index == clip.take_index && matches!(take.name, TakeNameState::Unavailable)) { return Err(CollectionOutputError::Malformed); }
            }
            ClipUnavailableReason::TakeNameMismatched => {
                if source.take_inventory != TakeInventoryState::Complete || !source.observed_takes.iter().any(|take| take.source_take_index == clip.take_index && matches!(&take.name, TakeNameState::Available { value } if value != &clip.take_name)) { return Err(CollectionOutputError::Malformed); }
            }
            ClipUnavailableReason::NormalizedClipUnavailable => {
                if source.take_inventory != TakeInventoryState::Complete || !source.observed_takes.iter().any(|take| take.source_take_index == clip.take_index && matches!(take.name, TakeNameState::Available { ref value } if value == &clip.take_name) && matches!(take.normalized, NormalizedClipState::Unavailable)) { return Err(CollectionOutputError::Malformed); }
            }
        },
    }
    Ok(())
}
fn validate_set(
    set: &RuntimeSetWire,
    clips: &BTreeMap<&str, &ClipBindingStateWire>,
) -> Result<(), CollectionOutputError> {
    if set.decision != RuntimeSetDecision::NotEvaluated || set.members.len() < 2 {
        return Err(CollectionOutputError::Malformed);
    }
    let mut ids = BTreeSet::new();
    let mut expected_gaps = Vec::new();
    for member in &set.members {
        if !ids.insert(member.id.as_str()) {
            return Err(CollectionOutputError::Malformed);
        }
        let Some(binding) = clips.get(member.id.as_str()) else {
            return Err(CollectionOutputError::Malformed);
        };
        match (binding, &member.resolution) {
            (ClipBindingStateWire::Established { .. }, RuntimeSetMemberStateWire::Established) => {}
            (
                ClipBindingStateWire::Unavailable {
                    reason: clip_reason,
                },
                RuntimeSetMemberStateWire::Unavailable { reason },
            ) if clip_reason == reason => expected_gaps.push(member.id.clone()),
            _ => return Err(CollectionOutputError::Malformed),
        }
    }
    let expected_lifecycle = if expected_gaps.is_empty() {
        RuntimeSetLifecycle::Complete
    } else {
        RuntimeSetLifecycle::Incomplete
    };
    if set.gaps != expected_gaps || set.lifecycle != expected_lifecycle {
        return Err(CollectionOutputError::Malformed);
    }
    validate_set_evidence(set, clips)?;
    Ok(())
}

fn validate_set_evidence(
    set: &RuntimeSetWire,
    clips: &BTreeMap<&str, &ClipBindingStateWire>,
) -> Result<(), CollectionOutputError> {
    if set._kind != CollectionRuntimeSetKindV1::GaitGroup {
        return (set.evidence.is_none()
            && set.members.iter().all(|member| member.gait_phase.is_none()))
        .then_some(())
        .ok_or(CollectionOutputError::Malformed);
    }
    let evidence = set
        .evidence
        .as_ref()
        .ok_or(CollectionOutputError::Malformed)?;
    let mut phases = Vec::with_capacity(set.members.len());
    for member in &set.members {
        let binding = clips
            .get(member.id.as_str())
            .ok_or(CollectionOutputError::Malformed)?;
        let (availability, phase) = match (binding, &member.resolution) {
            (
                ClipBindingStateWire::Established { measurements, .. },
                RuntimeSetMemberStateWire::Established,
            ) => gait_phase_from_measurements(measurements),
            (
                ClipBindingStateWire::Unavailable {
                    reason: clip_reason,
                },
                RuntimeSetMemberStateWire::Unavailable { reason },
            ) if reason == clip_reason => (MeasurementAvailability::Unavailable, None),
            _ => return Err(CollectionOutputError::Malformed),
        };
        let phase_evidence = member
            .gait_phase
            .as_ref()
            .ok_or(CollectionOutputError::Malformed)?;
        if phase_evidence.availability != availability
            || phase_evidence.phase != phase
            || phase_evidence
                .phase
                .is_some_and(|value| !value.is_finite() || !(0.0..1.0).contains(&value))
        {
            return Err(CollectionOutputError::Malformed);
        }
        if availability == MeasurementAvailability::Measured {
            phases.push((
                member.id.as_str(),
                phase.ok_or(CollectionOutputError::Malformed)?,
            ));
        }
    }
    let expected = if phases.len() == set.members.len() {
        phases.sort_by(|left, right| left.0.cmp(right.0));
        Some(circular_phase_spread(
            &phases.iter().map(|(_, phase)| *phase).collect::<Vec<_>>(),
        ))
    } else {
        None
    };
    let members_measured = phases.len();
    let expected_lifecycle = if expected.is_some() {
        RuntimeSetLifecycle::Complete
    } else {
        RuntimeSetLifecycle::Incomplete
    };
    if evidence.gait_phase.lifecycle != expected_lifecycle
        || evidence.gait_phase.members_measured != members_measured
    {
        return Err(CollectionOutputError::Malformed);
    }
    match (
        expected,
        evidence.gait_phase.phase_spread,
        evidence.gait_phase.spread_basis.as_deref(),
    ) {
        (None, None, None) => Ok(()),
        (Some(expected), Some(phase_spread), Some(spread_basis))
            if spread_basis == GAIT_PHASE_SPREAD_BASIS
                && phase_spread.is_finite()
                && (0.0..=0.5).contains(&phase_spread)
                && phase_spread == expected =>
        {
            Ok(())
        }
        _ => Err(CollectionOutputError::Malformed),
    }
}
fn summarize_wire(
    sources: &[CollectionSourceWire],
    clips: &[CollectionClipWire],
    sets: &[RuntimeSetWire],
) -> Result<CollectionSummary, CollectionOutputError> {
    let readable_sources = sources
        .iter()
        .filter(|source| matches!(source.input, SourceInputStateWire::Available { .. }))
        .count();
    let established_sources = sources
        .iter()
        .filter(|source| {
            matches!(source.input, SourceInputStateWire::Available { .. })
                && matches!(
                    source.digest,
                    DigestPinStateWire::Unpinned | DigestPinStateWire::Matched { .. }
                )
                && matches!(source.loader, LoaderStateWire::Ready)
                && matches!(source.result, DocumentResultWire::Available { .. })
        })
        .count();
    let established_clips = clips
        .iter()
        .filter(|clip| matches!(clip.binding, ClipBindingStateWire::Established { .. }))
        .count();
    let complete_runtime_sets = sets
        .iter()
        .filter(|set| set.lifecycle == RuntimeSetLifecycle::Complete)
        .count();
    Ok(CollectionSummary {
        sources: sources.len(),
        readable_sources,
        established_sources,
        clips: clips.len(),
        established_clips,
        runtime_sets: sets.len(),
        complete_runtime_sets,
        incomplete: established_sources != sources.len()
            || established_clips != clips.len()
            || complete_runtime_sets != sets.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use animsmith_core::{
        Clip, Config, Document, LintFileReport, MeasurementContract, MetricGrids, ResolvedRoles,
        RigInfo,
    };
    type JsonValue = serde_json::value::Value;

    fn source_fixture() -> (InputIdentity, LintEnvelope, ClipMeasurements) {
        let input = InputIdentity::from_bytes(b"collection-output-source");
        let mut document = Document::default();
        document.clips.push(Clip {
            name: "take".into(),
            duration_s: 0.0,
            tracks: Vec::new(),
        });
        let roles = ResolvedRoles::default();
        let config = Config::default();
        let grids = MetricGrids::new(&document);
        let measurements = MeasurementContract::new(
            animsmith_core::measure::measure_document(&grids, &roles, &config),
            animsmith_core::measure::measure_assets(&document),
        )
        .unwrap();
        let clip = measurements.clips()["take"].clone();
        let report = LintFileReport::new(
            "safe/source.glb",
            input.clone(),
            RigInfo::from_resolved(&document, &roles).unwrap(),
            None,
            Vec::new(),
            measurements,
        )
        .unwrap();
        (
            input,
            LintEnvelope::new(crate::current_tool(), vec![report]).unwrap(),
            clip,
        )
    }

    fn output_model_fixture(
        two_sources: bool,
        with_set: bool,
        locator_padding: usize,
    ) -> CollectionOutput {
        let (input, envelope, measurements) = source_fixture();
        let count = usize::from(two_sources) + 1;
        let mut sources = Vec::new();
        let mut clips = Vec::new();
        for index in 0..count {
            let key = if index == 0 { "a" } else { "b" };
            let id = if index == 0 {
                "com.example/clip-a"
            } else {
                "com.example/clip-b"
            };
            sources.push(CollectionSourceRecord::new(
                key,
                format!("safe/{}{key}.glb", "x".repeat(locator_padding)),
                SourceInputState::Available {
                    input: input.clone(),
                },
                DigestPinState::Matched {
                    expected_sha256: input.sha256().to_owned(),
                },
                ConfigState::Default,
                LoaderState::Ready,
                TakeInventoryState::Complete,
                vec![ObservedTake::new(0, "take", 0, "take")],
                DocumentResult::Available {
                    envelope: Box::new(envelope.clone()),
                },
            ));
            clips.push(CollectionClipRecord::new(
                id,
                key,
                0,
                "take",
                ClipBindingState::Established {
                    observed_source_take_index: 0,
                    observed_take_name: "take".into(),
                    normalized_clip_index: 0,
                    measurements: Box::new(measurements.clone()),
                    check_reference: CheckReferenceState::Available {
                        reference: MeasurementReference::new(key, 0, "take"),
                    },
                },
            ));
        }
        let runtime_sets = if with_set {
            vec![CollectionRuntimeSetRecord::new(
                "com.example/set",
                CollectionRuntimeSetKindV1::GaitGroup,
                vec![
                    RuntimeSetMember::new("com.example/clip-a", RuntimeSetMemberState::Established),
                    RuntimeSetMember::new("com.example/clip-b", RuntimeSetMemberState::Established),
                ],
            )]
        } else {
            Vec::new()
        };
        CollectionOutput::new(
            crate::current_tool(),
            CollectionManifestIdentity::new(
                "com.example",
                InputIdentity::from_bytes(b"[manifest]"),
            ),
            sources,
            clips,
            runtime_sets,
            input.bytes() * count as u64,
            0,
        )
        .unwrap()
    }

    fn output_fixture(two_sources: bool, with_set: bool) -> Vec<u8> {
        let mut output = output_model_fixture(two_sources, with_set, 0);
        output.render_json_vec().unwrap()
    }

    fn json_fixture(two_sources: bool, with_set: bool) -> JsonValue {
        serde_json::from_slice(&output_fixture(two_sources, with_set)).unwrap()
    }

    fn set_gait_phases(output: &mut CollectionOutput, phases: &[(&str, f64)]) {
        for (id, phase) in phases {
            let clip = output
                .clips
                .iter_mut()
                .find(|clip| clip.id == *id)
                .expect("fixture clip exists");
            let ClipBindingState::Established { measurements, .. } = &mut clip.binding else {
                panic!("fixture clip is established");
            };
            measurements.gait_availability = MeasurementAvailability::Measured;
            measurements.gait = Some(
                serde_json::from_value(serde_json::json!({
                    "phase": phase,
                    "phase_availability": "measured",
                    "lr_amplitude_m": 0.1
                }))
                .expect("synthetic gait measurement"),
            );
        }
        let clips = output
            .clips
            .iter()
            .map(|clip| (clip.id.as_str(), clip))
            .collect::<BTreeMap<_, _>>();
        for set in &mut output.runtime_sets {
            set.populate_evidence(&clips).unwrap();
        }
    }

    /// Source fixtures intentionally have no bilateral-foot signal, so their
    /// CLI evidence covers the incomplete path.  Build measured rows directly
    /// here to test the deterministic circular aggregate independently of
    /// manifest member order.
    fn three_member_gait_output(member_order: &[&str]) -> CollectionOutput {
        let (input, envelope, measurements) = source_fixture();
        let rows = [
            ("a", "com.example/clip-a"),
            ("b", "com.example/clip-b"),
            ("c", "com.example/clip-c"),
        ];
        let sources = rows
            .iter()
            .map(|(key, _)| {
                CollectionSourceRecord::new(
                    *key,
                    format!("safe/{key}.glb"),
                    SourceInputState::Available {
                        input: input.clone(),
                    },
                    DigestPinState::Matched {
                        expected_sha256: input.sha256().to_owned(),
                    },
                    ConfigState::Default,
                    LoaderState::Ready,
                    TakeInventoryState::Complete,
                    vec![ObservedTake::new(0, "take", 0, "take")],
                    DocumentResult::Available {
                        envelope: Box::new(envelope.clone()),
                    },
                )
            })
            .collect::<Vec<_>>();
        let clips = rows
            .iter()
            .map(|(key, id)| {
                CollectionClipRecord::new(
                    *id,
                    *key,
                    0,
                    "take",
                    ClipBindingState::Established {
                        observed_source_take_index: 0,
                        observed_take_name: "take".into(),
                        normalized_clip_index: 0,
                        measurements: Box::new(measurements.clone()),
                        check_reference: CheckReferenceState::Available {
                            reference: MeasurementReference::new(*key, 0, "take"),
                        },
                    },
                )
            })
            .collect::<Vec<_>>();
        let members = member_order
            .iter()
            .map(|id| RuntimeSetMember::new(*id, RuntimeSetMemberState::Established))
            .collect();
        CollectionOutput::new(
            crate::current_tool(),
            CollectionManifestIdentity::new(
                "com.example",
                InputIdentity::from_bytes(b"[manifest]"),
            ),
            sources,
            clips,
            vec![CollectionRuntimeSetRecord::new(
                "com.example/set",
                CollectionRuntimeSetKindV1::GaitGroup,
                members,
            )],
            input.bytes() * 3,
            0,
        )
        .unwrap()
    }

    fn rejects(mut value: JsonValue) {
        let bytes = (0..3)
            .find_map(|_| {
                let bytes = serde_json::to_vec(&value).unwrap();
                let prior = value["work"]["serialized_bytes"].as_u64();
                value["work"]["serialized_bytes"] = (bytes.len() as u64).into();
                (prior == Some(bytes.len() as u64)).then_some(bytes)
            })
            .expect("serialized byte count converges");
        assert!(read_collection_output(&bytes[..]).is_err());
    }

    #[test]
    fn budget_is_immutable() {
        let budget = serde_json::to_value(CollectionOutputBudgetV1::v1()).unwrap();
        assert_eq!(budget["max_source_bytes"], 1024_u64 * 1024 * 1024);
        assert_eq!(
            budget["max_aggregate_source_bytes"],
            16_u64 * 1024 * 1024 * 1024
        );
        assert_eq!(budget["max_serialized_bytes"], 256_u64 * 1024 * 1024);
    }

    #[test]
    fn producer_round_trips_through_strict_reader() {
        let bytes = output_fixture(true, true);
        let output = read_collection_output(&bytes[..]).unwrap();
        assert_eq!(output.source_count(), 2);
        assert_eq!(output.clip_count(), 2);
    }

    #[test]
    fn gait_phase_evidence_uses_member_order_but_logical_id_sorted_aggregate() {
        let mut output = output_model_fixture(true, true, 0);
        set_gait_phases(
            &mut output,
            &[("com.example/clip-a", 0.03), ("com.example/clip-b", 0.97)],
        );
        let value: JsonValue = serde_json::from_slice(&output.render_json_vec().unwrap()).unwrap();
        let set = &value["runtime_sets"][0];
        assert_eq!(set["members"][0]["id"], "com.example/clip-a");
        assert_eq!(set["members"][1]["id"], "com.example/clip-b");
        assert_eq!(set["members"][0]["gait_phase"]["phase"], 0.03);
        assert_eq!(set["members"][1]["gait_phase"]["phase"], 0.97);
        assert_eq!(set["evidence"]["gait_phase"]["lifecycle"], "complete");
        assert_eq!(set["evidence"]["gait_phase"]["members_measured"], 2);
        assert_eq!(
            set["evidence"]["gait_phase"]["spread_basis"],
            GAIT_PHASE_SPREAD_BASIS
        );
        assert_eq!(
            set["evidence"]["gait_phase"]["phase_spread"],
            circular_phase_spread(&[0.03, 0.97])
        );
    }

    #[test]
    fn gait_phase_aggregate_is_invariant_to_three_member_manifest_order() {
        let phases = [
            ("com.example/clip-a", 0.02),
            ("com.example/clip-b", 0.98),
            ("com.example/clip-c", 0.05),
        ];
        let mut first = three_member_gait_output(&[
            "com.example/clip-c",
            "com.example/clip-a",
            "com.example/clip-b",
        ]);
        let mut second = three_member_gait_output(&[
            "com.example/clip-b",
            "com.example/clip-c",
            "com.example/clip-a",
        ]);
        set_gait_phases(&mut first, &phases);
        set_gait_phases(&mut second, &phases);
        let first: JsonValue = serde_json::from_slice(&first.render_json_vec().unwrap()).unwrap();
        let second: JsonValue = serde_json::from_slice(&second.render_json_vec().unwrap()).unwrap();
        assert_eq!(
            first["runtime_sets"][0]["members"][0]["id"],
            "com.example/clip-c"
        );
        assert_eq!(
            second["runtime_sets"][0]["members"][0]["id"],
            "com.example/clip-b"
        );
        assert_eq!(
            first["runtime_sets"][0]["evidence"]["gait_phase"]["phase_spread"],
            second["runtime_sets"][0]["evidence"]["gait_phase"]["phase_spread"]
        );
    }

    #[test]
    fn partial_gait_phase_set_never_aggregates_a_measured_subset() {
        let mut output = three_member_gait_output(&[
            "com.example/clip-c",
            "com.example/clip-a",
            "com.example/clip-b",
        ]);
        set_gait_phases(
            &mut output,
            &[("com.example/clip-a", 0.02), ("com.example/clip-b", 0.98)],
        );
        let value: JsonValue = serde_json::from_slice(&output.render_json_vec().unwrap()).unwrap();
        let set = &value["runtime_sets"][0];
        assert_eq!(set["members"].as_array().unwrap().len(), 3);
        assert_eq!(set["members"][0]["id"], "com.example/clip-c");
        assert_eq!(
            set["members"][0]["gait_phase"]["availability"],
            "not_applicable"
        );
        assert_eq!(set["members"][1]["gait_phase"]["availability"], "measured");
        assert_eq!(set["members"][2]["gait_phase"]["availability"], "measured");
        assert_eq!(set["evidence"]["gait_phase"]["lifecycle"], "incomplete");
        assert_eq!(set["evidence"]["gait_phase"]["members_measured"], 2);
        assert!(set["evidence"]["gait_phase"].get("phase_spread").is_none());
        assert!(set["evidence"]["gait_phase"].get("spread_basis").is_none());

        let wire: CollectionOutputWire = serde_json::from_value(value.clone()).unwrap();
        let clips = wire
            .clips
            .iter()
            .map(|clip| (clip.id.as_str(), &clip.binding))
            .collect::<BTreeMap<_, _>>();
        assert!(validate_set_evidence(&wire.runtime_sets[0], &clips).is_ok());

        let mut injected = value;
        injected["runtime_sets"][0]["evidence"]["gait_phase"]["lifecycle"] = "complete".into();
        injected["runtime_sets"][0]["evidence"]["gait_phase"]["members_measured"] = 3.into();
        injected["runtime_sets"][0]["evidence"]["gait_phase"]["phase_spread"] = 0.02.into();
        injected["runtime_sets"][0]["evidence"]["gait_phase"]["spread_basis"] =
            GAIT_PHASE_SPREAD_BASIS.into();
        let injected: CollectionOutputWire = serde_json::from_value(injected).unwrap();
        let clips = injected
            .clips
            .iter()
            .map(|clip| (clip.id.as_str(), &clip.binding))
            .collect::<BTreeMap<_, _>>();
        assert!(validate_set_evidence(&injected.runtime_sets[0], &clips).is_err());
    }

    #[test]
    fn incomplete_gait_phase_evidence_keeps_all_members_and_omits_scalar() {
        let value = json_fixture(true, true);
        let set = &value["runtime_sets"][0];
        assert_eq!(set["evidence"]["gait_phase"]["lifecycle"], "incomplete");
        assert_eq!(set["evidence"]["gait_phase"]["members_measured"], 0);
        assert!(set["evidence"]["gait_phase"].get("phase_spread").is_none());
        assert!(set["evidence"]["gait_phase"].get("spread_basis").is_none());
        assert_eq!(set["members"].as_array().unwrap().len(), 2);
        assert_eq!(
            set["members"][0]["gait_phase"]["availability"],
            "not_applicable"
        );
    }

    #[test]
    fn serialized_byte_count_converges_across_a_decimal_boundary() {
        let baseline = output_fixture(true, false);
        assert!(baseline.len() < 10_000, "fixture must begin below boundary");
        let padding = (10_000 - baseline.len()).div_ceil(2);
        let mut output = output_model_fixture(true, false, padding);
        let bytes = output.render_json_vec().unwrap();
        assert!((10_000..10_010).contains(&bytes.len()));
        assert!(
            read_collection_output(&bytes[..]).is_ok(),
            "baseline={}, padding={}, final={}",
            baseline.len(),
            padding,
            bytes.len()
        );
    }

    #[test]
    fn serializer_stops_at_n_plus_one_before_unbounded_allocation() {
        let value = "0123456789";
        assert!(matches!(
            serialize_json_bounded(&value, 4),
            Err(CollectionOutputError::TooLarge)
        ));
        assert_eq!(serialize_json_bounded(&value, 12).unwrap().len(), 12);

        let mut counter = BoundedJsonCounter::new(4);
        counter.write_all(b"12345").unwrap();
        assert_eq!(counter.bytes, 5);
        assert_eq!(counter.terminal, 5);
    }

    #[test]
    fn aggregate_exhaustion_requires_a_prior_n_plus_one_witness() {
        let full_source = (false, COLLECTION_OUTPUT_V1_MAX_SOURCE_BYTES);
        let mut valid = vec![full_source; 16];
        valid.push((false, 1));
        valid.push((true, 0));
        valid.push((true, 0));
        assert_eq!(
            validate_primary_source_sequence(valid.into_iter()),
            Some(COLLECTION_OUTPUT_V1_MAX_AGGREGATE_SOURCE_BYTES + 1)
        );
        assert_eq!(
            validate_primary_source_sequence([(true, 0)].into_iter()),
            None
        );
        assert_eq!(
            validate_primary_source_sequence([(false, 1), (true, 0)].into_iter()),
            None
        );
        assert_eq!(
            validate_primary_source_sequence([(true, 0), (false, 1)].into_iter()),
            None
        );
    }

    #[test]
    fn strict_reader_rejects_unknown_and_invalid_contract_fields() {
        let mut unknown_nested = json_fixture(false, false);
        unknown_nested["sources"][0]["result"]["envelope"]["unknown"] = true.into();
        rejects(unknown_nested);

        let mut wrong_schema = json_fixture(false, false);
        wrong_schema["schema"] = "urn:example:wrong".into();
        rejects(wrong_schema);

        let mut wrong_budget = json_fixture(false, false);
        wrong_budget["budget"]["max_source_bytes"] = 1.into();
        rejects(wrong_budget);
    }

    #[test]
    fn strict_reader_rejects_row_identity_and_source_state_contradictions() {
        let mut noncanonical = json_fixture(true, false);
        let sources = noncanonical["sources"].as_array_mut().unwrap();
        sources.swap(0, 1);
        rejects(noncanonical);

        let mut duplicate = json_fixture(true, false);
        duplicate["sources"][1]["key"] = "a".into();
        rejects(duplicate);

        let mut digest = json_fixture(false, false);
        digest["sources"][0]["digest"] = serde_json::json!({
            "state": "mismatched",
            "expected_sha256": "0".repeat(64),
            "observed_sha256": digest["sources"][0]["input"]["input"]["sha256"],
        });
        rejects(digest);

        let mut loader = json_fixture(false, false);
        loader["sources"][0]["loader"] = serde_json::json!({
            "state": "unavailable", "reason": "malformed_input"
        });
        rejects(loader);

        let mut result = json_fixture(false, false);
        result["sources"][0]["result"] = serde_json::json!({
            "state": "unavailable", "reason": "loader_unavailable"
        });
        rejects(result);

        let mut unavailable_with_inventory = json_fixture(false, false);
        unavailable_with_inventory["sources"][0]["input"] = serde_json::json!({
            "state": "unavailable", "reason": "missing", "inspected_bytes": 0
        });
        unavailable_with_inventory["sources"][0]["digest"] =
            serde_json::json!({"state": "unpinned"});
        unavailable_with_inventory["sources"][0]["loader"] = serde_json::json!({
            "state": "unavailable", "reason": "source_unavailable"
        });
        unavailable_with_inventory["sources"][0]["result"] = serde_json::json!({
            "state": "unavailable", "reason": "source_unavailable"
        });
        let unavailable_with_inventory: CollectionOutputWire =
            serde_json::from_value(unavailable_with_inventory).unwrap();
        assert!(validate_source(&unavailable_with_inventory.sources[0]).is_err());

        let mut fabricated_nested_unavailability = json_fixture(false, false);
        fabricated_nested_unavailability["sources"][0]["result"] = serde_json::json!({
            "state": "unavailable", "reason": "nested_output_unavailable"
        });
        let fabricated_nested_unavailability: CollectionOutputWire =
            serde_json::from_value(fabricated_nested_unavailability).unwrap();
        assert!(
            validate_source(&fabricated_nested_unavailability.sources[0]).is_err(),
            "ordinary normalized names cannot justify nested-output refusal"
        );
    }

    #[test]
    fn duplicate_check_reference_unavailability_requires_a_real_key_collision() {
        let mut unique = json_fixture(false, false);
        unique["clips"][0]["binding"]["check_reference"] = serde_json::json!({
            "state": "unavailable",
            "reason": "duplicate_embedded_take_name"
        });
        let unique: CollectionOutputWire = serde_json::from_value(unique).unwrap();
        assert!(validate_clip(&unique.clips[0], &unique.sources[0]).is_err());

        let mut duplicate = json_fixture(false, false);
        let mut second = duplicate["sources"][0]["observed_takes"][0].clone();
        second["source_take_index"] = 1.into();
        second["normalized"]["index"] = 1.into();
        duplicate["sources"][0]["observed_takes"]
            .as_array_mut()
            .unwrap()
            .push(second);
        duplicate["clips"][0]["binding"]["check_reference"] = serde_json::json!({
            "state": "unavailable",
            "reason": "duplicate_embedded_take_name"
        });
        let duplicate: CollectionOutputWire = serde_json::from_value(duplicate).unwrap();
        assert!(validate_clip(&duplicate.clips[0], &duplicate.sources[0]).is_ok());
    }

    #[test]
    fn strict_reader_rejects_clip_set_summary_and_work_contradictions() {
        let mut reference = json_fixture(false, false);
        reference["clips"][0]["binding"]["check_reference"]["reference"]["source"] = "other".into();
        rejects(reference);

        let mut set = json_fixture(true, true);
        set["runtime_sets"][0]["members"][1]["resolution"] = serde_json::json!({
            "state": "unavailable", "reason": "take_index_missing"
        });
        rejects(set);

        let mut summary = json_fixture(false, false);
        summary["summary"]["established_clips"] = 0.into();
        rejects(summary);

        let mut work = json_fixture(false, false);
        work["work"]["primary_source_bytes"] = 0.into();
        rejects(work);

        let mut missing_gait_member_evidence = json_fixture(true, true);
        missing_gait_member_evidence["runtime_sets"][0]["members"][0]
            .as_object_mut()
            .unwrap()
            .remove("gait_phase");
        rejects(missing_gait_member_evidence);

        let mut invented_gait_scalar = json_fixture(true, true);
        invented_gait_scalar["runtime_sets"][0]["evidence"]["gait_phase"]["lifecycle"] =
            "complete".into();
        invented_gait_scalar["runtime_sets"][0]["evidence"]["gait_phase"]["members_measured"] =
            2.into();
        invented_gait_scalar["runtime_sets"][0]["evidence"]["gait_phase"]["phase_spread"] =
            0.1.into();
        invented_gait_scalar["runtime_sets"][0]["evidence"]["gait_phase"]["spread_basis"] =
            GAIT_PHASE_SPREAD_BASIS.into();
        rejects(invented_gait_scalar);

        let mut non_gait_evidence = json_fixture(true, true);
        non_gait_evidence["runtime_sets"][0]["kind"] = "sync-group".into();
        rejects(non_gait_evidence);
    }

    #[test]
    fn strict_reader_rejects_missing_linked_rows_and_duplicate_ids() {
        let mut missing_clip = json_fixture(true, true);
        missing_clip["clips"].as_array_mut().unwrap().pop();
        rejects(missing_clip);

        let mut duplicate_clip = json_fixture(true, false);
        duplicate_clip["clips"][1]["id"] = duplicate_clip["clips"][0]["id"].clone();
        rejects(duplicate_clip);

        let mut duplicate_set = json_fixture(true, true);
        let second = duplicate_set["runtime_sets"][0].clone();
        duplicate_set["runtime_sets"]
            .as_array_mut()
            .unwrap()
            .push(second);
        duplicate_set["summary"]["runtime_sets"] = 2.into();
        duplicate_set["summary"]["complete_runtime_sets"] = 2.into();
        duplicate_set["work"]["manifest_rows"] = 6.into();
        duplicate_set["work"]["runtime_set_members"] = 4.into();
        duplicate_set["work"]["aggregate_work"] = 10.into();
        rejects(duplicate_set);
    }

    #[test]
    fn strict_reader_rejects_non_finite_json_numbers() {
        let bytes = output_fixture(false, false);
        let json = String::from_utf8(bytes).unwrap();
        let mutated = json.replacen("\"duration_s\":0.0", "\"duration_s\":NaN", 1);
        assert_ne!(mutated, json, "fixture contains the analytic duration");
        assert!(read_collection_output(mutated.as_bytes()).is_err());
    }

    #[test]
    fn strict_reader_validates_raw_document_result_and_exact_byte_count() {
        let bytes = output_fixture(false, false);
        assert!(read_collection_output(&bytes[..]).is_ok());

        let mut raw_result = json_fixture(false, false);
        raw_result["sources"][0]["result"]["envelope"] = serde_json::json!([]);
        rejects(raw_result);

        let mut measure_result = json_fixture(false, false);
        let envelope = measure_result["sources"][0]["result"]["envelope"]
            .as_object_mut()
            .unwrap();
        envelope.insert("command".into(), "measure".into());
        envelope.insert("summary".into(), serde_json::json!({"files": 1}));
        let file = envelope["files"].as_array_mut().unwrap()[0]
            .as_object_mut()
            .unwrap();
        file.remove("checks");
        file.remove("prediction_provenance");
        rejects(measure_result);

        let mut n_plus_one = bytes.clone();
        n_plus_one.push(b'\n');
        assert!(matches!(
            read_collection_output(&n_plus_one[..]),
            Err(CollectionOutputError::Malformed)
        ));
    }

    #[test]
    fn work_is_checked() {
        assert!(CollectionWork::new(usize::MAX, 1, 0, 0, 0, 0).is_err());
    }
}
