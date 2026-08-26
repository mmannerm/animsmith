//! Bounded, format-neutral observations retained from a source loader.
//!
//! This module binds immutable raw-source observations to the normalized
//! [`Document`] produced from the same primary bytes.

use crate::{
    DependencyClosureError, DependencyClosureV1, Document, ExactSourceTimingContractError,
    ExactSourceTimingV1, InputIdentity, RawGltfAddressabilityInventoryV1,
    RawSceneAttachmentCoverageV1, RawSceneAttachmentInventoryV1, RawSourceSkeletonEvidenceV1,
    RawTransformPathInventoryV1, SourceSkeletonAssets, SourceSkeletonCoverage,
};
use serde::Serialize;
use std::fmt;

/// Semantic identity of the in-memory raw-source vocabulary.
pub const RAW_SOURCE_FACTS_V1_ID: &str = "urn:animsmith:raw-source-facts:1";
/// Maximum enumerable clip/channel/construct/resource rows retained by one V1 projection.
pub const RAW_SOURCE_V1_MAX_OBSERVATIONS: usize = 65_536;
/// Maximum source clips/takes retained by one V1 projection.
pub const RAW_SOURCE_V1_MAX_CLIPS: usize = 4_096;
/// Maximum external-resource declarations retained by one V1 projection.
pub const RAW_SOURCE_V1_MAX_RESOURCE_REFERENCES: usize = 4_096;
/// Maximum UTF-8 bytes retained in one source text or locator.
pub const RAW_SOURCE_V1_MAX_TEXT_BYTES: usize = 4_096;
/// Maximum UTF-8 source-text bytes retained by one V1 projection.
pub const RAW_SOURCE_V1_MAX_TOTAL_TEXT_BYTES: usize = 8 * 1024 * 1024;
/// Maximum source traversal depth inspected by a V1 projection.
pub const RAW_SOURCE_V1_MAX_TRAVERSAL_DEPTH: usize = 128;

/// Exact input container recognized by a format loader.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SourceFormatV1 {
    /// JSON `.gltf` container.
    GltfJson,
    /// Binary `.glb` container.
    Glb,
    /// Autodesk FBX container parsed through `ufbx`.
    Fbx,
}

/// Stable reason why a source observation or set is incomplete.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SourceUnavailableReasonV1 {
    /// The source declaration is malformed.
    Malformed,
    /// The AnimSmith loader discarded the source value.
    Discarded,
    /// Coordinate or transform normalization removed the original form.
    NormalizedAway,
    /// Animation baking removed the original form.
    BakedAway,
    /// The AnimSmith loader does not model this source domain.
    LoaderUnsupported,
    /// The deterministic V1 projection budget was exhausted.
    ProjectionBudgetExceeded,
    /// The source parser did not make this evidence available.
    ParserUnavailable,
}

/// Whether a source-fact row set is exhaustive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceSetCoverageStateV1 {
    /// Every row in the source domain is represented.
    Complete,
    /// Retained rows are authoritative positive-presence evidence, but the set is truncated.
    Partial,
    /// No exhaustive row projection is available.
    Unavailable,
}

/// Coverage and reason for one independently enumerable source-fact domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceSetCoverageV1 {
    state: SourceSetCoverageStateV1,
    reason: Option<SourceUnavailableReasonV1>,
}

impl SourceSetCoverageV1 {
    /// Complete coverage; an empty set proves absence.
    pub const fn complete() -> Self {
        Self {
            state: SourceSetCoverageStateV1::Complete,
            reason: None,
        }
    }

    /// Partial coverage; retained rows prove presence but omissions prove nothing.
    pub const fn partial(reason: SourceUnavailableReasonV1) -> Self {
        Self {
            state: SourceSetCoverageStateV1::Partial,
            reason: Some(reason),
        }
    }

    /// Unavailable coverage for a source domain.
    pub const fn unavailable(reason: SourceUnavailableReasonV1) -> Self {
        Self {
            state: SourceSetCoverageStateV1::Unavailable,
            reason: Some(reason),
        }
    }

    /// Coverage state.
    pub const fn state(self) -> SourceSetCoverageStateV1 {
        self.state
    }

    /// Stable incompleteness reason, absent only for complete coverage.
    pub const fn reason(self) -> Option<SourceUnavailableReasonV1> {
        self.reason
    }

    /// Whether an empty row set proves source absence.
    pub const fn proves_absence(self) -> bool {
        matches!(self.state, SourceSetCoverageStateV1::Complete)
    }
}

/// What the AnimSmith loader did with an observed source declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SourceLoaderDispositionV1 {
    /// Retained without semantic reinterpretation.
    Preserved,
    /// Converted into AnimSmith's normalized coordinate/model domain.
    Normalized,
    /// Evaluated into baked samples.
    Baked,
    /// Deliberately omitted from the normalized document.
    Discarded,
    /// Recognized but unsupported by the AnimSmith loader.
    Unsupported,
    /// The loader cannot classify its treatment.
    Unknown,
    /// The source domain does not apply to this format.
    NotApplicable,
}

/// How a source fact was established.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SourceProvenanceKindV1 {
    /// Normative interchange-format semantics rather than a file member.
    FormatDefined,
    /// An exact source declaration retained across the loader boundary.
    SourceDeclared,
    /// A parser-effective projection of source declarations.
    ParserProjected,
    /// Derived from exact authored source declarations.
    DerivedFromSource,
}

/// Bounded source text safe for the raw-facts surface.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceTextV1(String);

impl SourceTextV1 {
    /// Retain bounded UTF-8 source text.
    ///
    /// # Errors
    ///
    /// Returns [`SourceFactsError::TextTooLong`] when the text exceeds the V1 limit.
    pub fn new(value: impl AsRef<str>) -> Result<Self, SourceFactsError> {
        let value = value.as_ref();
        if value.len() > RAW_SOURCE_V1_MAX_TEXT_BYTES {
            return Err(SourceFactsError::TextTooLong {
                bytes: value.len(),
                limit: RAW_SOURCE_V1_MAX_TEXT_BYTES,
            });
        }
        Ok(Self(value.to_owned()))
    }

    /// Retained source spelling.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn retained_bytes(&self) -> usize {
        self.0.len()
    }
}

impl fmt::Debug for SourceTextV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("SourceTextV1")
            .field(&self.0)
            .finish()
    }
}

/// Validated bounded source-internal locator used only for V1 provenance.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SourceLogicalLocatorV1 {
    text: SourceTextV1,
}

impl SourceLogicalLocatorV1 {
    /// Validate a generated glTF JSON Pointer used by this V1 fact model.
    ///
    /// # Errors
    ///
    /// Returns [`SourceFactsError::InvalidLogicalLocator`] outside the
    /// structural vocabulary and [`SourceFactsError::TextTooLong`] when oversized.
    pub fn gltf_json_pointer(value: impl AsRef<str>) -> Result<Self, SourceFactsError> {
        let value = value.as_ref();
        let mut segments = value
            .strip_prefix('/')
            .into_iter()
            .flat_map(|value| value.split('/'));
        let valid_root = matches!(
            segments.next(),
            Some("animations" | "buffers" | "images" | "extensionsUsed" | "extensionsRequired")
        );
        if !valid_root || !segments.all(valid_logical_segment) {
            return Err(SourceFactsError::InvalidLogicalLocator);
        }
        Ok(Self {
            text: SourceTextV1::new(value)?,
        })
    }

    /// Validate a generated parser-domain locator under the `fbx:` namespace.
    ///
    /// # Errors
    ///
    /// Returns [`SourceFactsError::InvalidLogicalLocator`] for host/resource
    /// paths, controls, or malformed segments, and
    /// [`SourceFactsError::TextTooLong`] when oversized.
    pub fn fbx_parser_path(value: impl AsRef<str>) -> Result<Self, SourceFactsError> {
        let value = value.as_ref();
        let Some(path) = value.strip_prefix("fbx:") else {
            return Err(SourceFactsError::InvalidLogicalLocator);
        };
        if path.is_empty()
            || path
                .split('/')
                .any(|segment| !valid_logical_segment(segment))
        {
            return Err(SourceFactsError::InvalidLogicalLocator);
        }
        Ok(Self {
            text: SourceTextV1::new(value)?,
        })
    }

    /// Retained generated logical spelling.
    pub fn as_str(&self) -> &str {
        self.text.as_str()
    }

    fn retained_bytes(&self) -> usize {
        self.text.retained_bytes()
    }
}

fn valid_logical_segment(segment: &str) -> bool {
    !segment.is_empty()
        && !matches!(segment, "." | "..")
        && segment.bytes().all(|value| {
            value.is_ascii_alphanumeric() || matches!(value, b'_' | b'-' | b'.' | b'*')
        })
}

/// Provenance for one source observation.
#[derive(Clone, PartialEq, Eq)]
pub struct SourceProvenanceV1 {
    kind: SourceProvenanceKindV1,
    locator: Option<SourceLogicalLocatorV1>,
}

impl fmt::Debug for SourceProvenanceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SourceProvenanceV1")
            .field("kind", &self.kind)
            .field("locator_retained", &self.locator.is_some())
            .finish()
    }
}

impl SourceProvenanceV1 {
    /// Format-defined evidence without a file-member locator.
    pub const fn format_defined() -> Self {
        Self {
            kind: SourceProvenanceKindV1::FormatDefined,
            locator: None,
        }
    }

    /// Exact source-declaration evidence at a validated logical locator.
    pub fn source_declared(locator: SourceLogicalLocatorV1) -> Self {
        Self {
            kind: SourceProvenanceKindV1::SourceDeclared,
            locator: Some(locator),
        }
    }

    /// Parser-effective evidence at a validated logical locator.
    pub fn parser_projected(locator: SourceLogicalLocatorV1) -> Self {
        Self {
            kind: SourceProvenanceKindV1::ParserProjected,
            locator: Some(locator),
        }
    }

    /// Evidence derived from an exact declaration at a validated logical locator.
    pub fn derived_from_source(locator: SourceLogicalLocatorV1) -> Self {
        Self {
            kind: SourceProvenanceKindV1::DerivedFromSource,
            locator: Some(locator),
        }
    }

    /// Provenance kind.
    pub const fn kind(&self) -> SourceProvenanceKindV1 {
        self.kind
    }

    /// Bounded source-internal logical locator, when one exists.
    pub fn locator(&self) -> Option<&SourceLogicalLocatorV1> {
        self.locator.as_ref()
    }

    fn retained_bytes(&self) -> usize {
        self.locator
            .as_ref()
            .map_or(0, SourceLogicalLocatorV1::retained_bytes)
    }
}

/// Availability of one scalar source observation.
#[derive(Debug, Clone, PartialEq)]
pub enum SourceObservationStateV1<T> {
    /// The value was observed at the documented provenance boundary.
    Observed(T),
    /// Complete evidence proves that this format/source has no value.
    ProvenAbsent,
    /// The value cannot be established.
    Unavailable(SourceUnavailableReasonV1),
}

/// One value with orthogonal availability, provenance, and loader treatment.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceObservationV1<T> {
    state: SourceObservationStateV1<T>,
    disposition: SourceLoaderDispositionV1,
    provenance: Option<SourceProvenanceV1>,
}

impl<T> SourceObservationV1<T> {
    /// Retain one observed value with explicit provenance and loader treatment.
    pub fn observed(
        value: T,
        provenance: SourceProvenanceV1,
        disposition: SourceLoaderDispositionV1,
    ) -> Self {
        Self {
            state: SourceObservationStateV1::Observed(value),
            disposition,
            provenance: Some(provenance),
        }
    }

    /// Prove that a complete source domain has no such value.
    pub fn proven_absent(provenance: SourceProvenanceV1) -> Self {
        Self {
            state: SourceObservationStateV1::ProvenAbsent,
            disposition: SourceLoaderDispositionV1::NotApplicable,
            provenance: Some(provenance),
        }
    }

    /// Record unavailable evidence without substituting a normalized value.
    pub fn unavailable(
        reason: SourceUnavailableReasonV1,
        provenance: Option<SourceProvenanceV1>,
        disposition: SourceLoaderDispositionV1,
    ) -> Self {
        Self {
            state: SourceObservationStateV1::Unavailable(reason),
            disposition,
            provenance,
        }
    }

    /// Availability/value state.
    pub const fn state(&self) -> &SourceObservationStateV1<T> {
        &self.state
    }

    /// AnimSmith-loader disposition, independent of target-engine policy.
    pub const fn disposition(&self) -> SourceLoaderDispositionV1 {
        self.disposition
    }

    /// Evidence provenance, when the loader retained it.
    pub fn provenance(&self) -> Option<&SourceProvenanceV1> {
        self.provenance.as_ref()
    }

    fn retained_bytes(&self) -> usize {
        self.provenance
            .as_ref()
            .map_or(0, SourceProvenanceV1::retained_bytes)
    }
}

impl SourceObservationV1<SourceTextV1> {
    fn retained_text_bytes(&self) -> usize {
        let value_bytes = match &self.state {
            SourceObservationStateV1::Observed(value) => value.retained_bytes(),
            SourceObservationStateV1::ProvenAbsent | SourceObservationStateV1::Unavailable(_) => 0,
        };
        value_bytes.saturating_add(self.retained_bytes())
    }
}

/// Coverage-qualified rows from one source domain.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceFactSetV1<T> {
    coverage: SourceSetCoverageV1,
    rows: Vec<T>,
}

impl<T> SourceFactSetV1<T> {
    /// Complete source-order rows; empty proves absence.
    pub fn complete(rows: Vec<T>) -> Self {
        Self {
            coverage: SourceSetCoverageV1::complete(),
            rows,
        }
    }

    /// Deterministic positive-presence prefix from a truncated domain.
    pub fn partial(rows: Vec<T>, reason: SourceUnavailableReasonV1) -> Self {
        Self {
            coverage: SourceSetCoverageV1::partial(reason),
            rows,
        }
    }

    /// No rows are available for the domain.
    pub fn unavailable(reason: SourceUnavailableReasonV1) -> Self {
        Self {
            coverage: SourceSetCoverageV1::unavailable(reason),
            rows: Vec::new(),
        }
    }

    /// Coverage for this independently enumerable domain.
    pub const fn coverage(&self) -> SourceSetCoverageV1 {
        self.coverage
    }

    /// Retained deterministic rows.
    pub fn rows(&self) -> &[T] {
        &self.rows
    }

    /// Whether this complete empty set proves source absence.
    pub fn proves_absence(&self) -> bool {
        self.rows.is_empty() && self.coverage.proves_absence()
    }

    fn mark_partial(&mut self, reason: SourceUnavailableReasonV1) {
        self.coverage = SourceSetCoverageV1::partial(reason);
    }
}

/// Signed coordinate axis used by a source basis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SourceAxisV1 {
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

impl SourceAxisV1 {
    const fn unsigned(self) -> u8 {
        match self {
            Self::PositiveX | Self::NegativeX => 0,
            Self::PositiveY | Self::NegativeY => 1,
            Self::PositiveZ | Self::NegativeZ => 2,
        }
    }

    const fn vector(self) -> [i8; 3] {
        match self {
            Self::PositiveX => [1, 0, 0],
            Self::NegativeX => [-1, 0, 0],
            Self::PositiveY => [0, 1, 0],
            Self::NegativeY => [0, -1, 0],
            Self::PositiveZ => [0, 0, 1],
            Self::NegativeZ => [0, 0, -1],
        }
    }
}

/// Handedness derived from a complete signed right/up/forward basis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceHandednessV1 {
    /// Right-handed signed basis.
    Right,
    /// Left-handed signed basis.
    Left,
}

/// Validated signed semantic right/up/forward source basis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceCoordinateBasisV1 {
    right: SourceAxisV1,
    up: SourceAxisV1,
    forward: SourceAxisV1,
}

impl SourceCoordinateBasisV1 {
    /// Construct one orthogonal signed semantic basis.
    ///
    /// # Errors
    ///
    /// Returns [`SourceFactsError::DuplicateBasisAxis`] when two semantic
    /// directions use the same unsigned axis.
    pub fn new(
        right: SourceAxisV1,
        up: SourceAxisV1,
        forward: SourceAxisV1,
    ) -> Result<Self, SourceFactsError> {
        if right.unsigned() == up.unsigned()
            || right.unsigned() == forward.unsigned()
            || up.unsigned() == forward.unsigned()
        {
            return Err(SourceFactsError::DuplicateBasisAxis);
        }
        Ok(Self { right, up, forward })
    }

    /// Semantic right direction.
    pub const fn right(self) -> SourceAxisV1 {
        self.right
    }

    /// Semantic up direction.
    pub const fn up(self) -> SourceAxisV1 {
        self.up
    }

    /// Semantic forward direction.
    pub const fn forward(self) -> SourceAxisV1 {
        self.forward
    }

    /// Handedness derived from the determinant of right/up/forward.
    pub fn handedness(self) -> SourceHandednessV1 {
        let [rx, ry, rz] = self.right.vector();
        let [ux, uy, uz] = self.up.vector();
        let [fx, fy, fz] = self.forward.vector();
        let determinant = i16::from(rx)
            * (i16::from(uy) * i16::from(fz) - i16::from(uz) * i16::from(fy))
            - i16::from(ry) * (i16::from(ux) * i16::from(fz) - i16::from(uz) * i16::from(fx))
            + i16::from(rz) * (i16::from(ux) * i16::from(fy) - i16::from(uy) * i16::from(fx));
        if determinant > 0 {
            SourceHandednessV1::Right
        } else {
            SourceHandednessV1::Left
        }
    }
}

/// Validated linear unit expressed as metres per source unit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SourceLinearUnitV1(f64);

impl SourceLinearUnitV1 {
    /// Construct a finite positive source linear unit.
    ///
    /// # Errors
    ///
    /// Returns [`SourceFactsError::InvalidLinearUnit`] for zero, negative, NaN, or infinity.
    pub fn new(meters_per_source_unit: f64) -> Result<Self, SourceFactsError> {
        if !meters_per_source_unit.is_finite() || meters_per_source_unit <= 0.0 {
            return Err(SourceFactsError::InvalidLinearUnit);
        }
        Ok(Self(meters_per_source_unit))
    }

    /// Metres represented by one source linear unit.
    pub const fn meters_per_source_unit(self) -> f64 {
        self.0
    }
}

/// Validated finite positive source frame rate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SourceFramesPerSecondV1(f64);

impl SourceFramesPerSecondV1 {
    /// Construct a finite positive frame rate.
    ///
    /// # Errors
    ///
    /// Returns [`SourceFactsError::InvalidFramesPerSecond`] for zero, negative, NaN, or infinity.
    pub fn new(value: f64) -> Result<Self, SourceFactsError> {
        if !value.is_finite() || value <= 0.0 {
            return Err(SourceFactsError::InvalidFramesPerSecond);
        }
        Ok(Self(value))
    }

    /// Frames per second.
    pub const fn get(self) -> f64 {
        self.0
    }
}

/// Finite inclusive source-time range in seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SourceTimeRangeV1 {
    begin_s: f64,
    end_s: f64,
}

impl SourceTimeRangeV1 {
    /// Construct a finite ordered source-time range. Zero duration is valid.
    ///
    /// # Errors
    ///
    /// Returns [`SourceFactsError::InvalidTimeRange`] for non-finite or reversed endpoints.
    pub fn new(begin_s: f64, end_s: f64) -> Result<Self, SourceFactsError> {
        if !begin_s.is_finite() || !end_s.is_finite() || begin_s > end_s {
            return Err(SourceFactsError::InvalidTimeRange);
        }
        Ok(Self { begin_s, end_s })
    }

    /// Beginning of the source range in seconds.
    pub const fn begin_s(self) -> f64 {
        self.begin_s
    }

    /// End of the source range in seconds.
    pub const fn end_s(self) -> f64 {
        self.end_s
    }
}

/// Format-neutral animation property present at the source boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SourceChannelPropertyV1 {
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

/// Source interpolation retained by a loader.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SourceInterpolationV1 {
    /// Step/constant interpolation.
    Step,
    /// Linear interpolation.
    Linear,
    /// Cubic-spline interpolation.
    CubicSpline,
    /// Another source interpolation spelling.
    Other,
}

/// Component curves present for a source property.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceComponentMaskV1 {
    x: bool,
    y: bool,
    z: bool,
}

impl SourceComponentMaskV1 {
    /// Construct an explicit source component mask.
    pub const fn new(x: bool, y: bool, z: bool) -> Self {
        Self { x, y, z }
    }

    /// Whether the X/component-0 curve is present.
    pub const fn x(self) -> bool {
        self.x
    }

    /// Whether the Y/component-1 curve is present.
    pub const fn y(self) -> bool {
        self.y
    }

    /// Whether the Z/component-2 curve is present.
    pub const fn z(self) -> bool {
        self.z
    }
}

/// Source target identity kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SourceTargetKindV1 {
    /// Source node-array identity.
    Node,
    /// Parser-stable non-node element identity.
    Element,
    /// Another source target domain.
    Other,
}

/// Stable target identity retained beside one channel/property declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SourceTargetV1 {
    kind: SourceTargetKindV1,
    index: u64,
}

impl SourceTargetV1 {
    /// Construct a stable source target identity.
    pub const fn new(kind: SourceTargetKindV1, index: u64) -> Self {
        Self { kind, index }
    }

    /// Target identity kind.
    pub const fn kind(self) -> SourceTargetKindV1 {
        self.kind
    }

    /// Stable index in the documented source/parser domain.
    pub const fn index(self) -> u64 {
        self.index
    }
}

/// One raw animation channel/property declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceChannelFactV1 {
    source_channel_index: usize,
    source_layer_index: Option<usize>,
    target: SourceTargetV1,
    property: SourceChannelPropertyV1,
    property_name: Option<SourceTextV1>,
    components: SourceComponentMaskV1,
    interpolation: SourceObservationV1<SourceInterpolationV1>,
    input_accessor_index: Option<usize>,
    output_accessor_index: Option<usize>,
    disposition: SourceLoaderDispositionV1,
    provenance: SourceProvenanceV1,
}

impl SourceChannelFactV1 {
    /// Construct one stable source channel/property row.
    pub fn new(
        source_channel_index: usize,
        target: SourceTargetV1,
        property: SourceChannelPropertyV1,
        components: SourceComponentMaskV1,
        interpolation: SourceObservationV1<SourceInterpolationV1>,
        disposition: SourceLoaderDispositionV1,
        provenance: SourceProvenanceV1,
    ) -> Self {
        Self {
            source_channel_index,
            source_layer_index: None,
            target,
            property,
            property_name: None,
            components,
            interpolation,
            input_accessor_index: None,
            output_accessor_index: None,
            disposition,
            provenance,
        }
    }

    /// Attach the source animation-layer index used by layered formats.
    pub fn with_source_layer_index(mut self, index: usize) -> Self {
        self.source_layer_index = Some(index);
        self
    }

    /// Attach a bounded source property spelling, primarily for [`SourceChannelPropertyV1::Other`].
    pub fn with_property_name(mut self, name: SourceTextV1) -> Self {
        self.property_name = Some(name);
        self
    }

    /// Attach exact glTF input/output accessor identities.
    pub fn with_accessors(mut self, input: usize, output: usize) -> Self {
        self.input_accessor_index = Some(input);
        self.output_accessor_index = Some(output);
        self
    }

    /// Stable channel/property index inside its source clip/layer projection.
    pub const fn source_channel_index(&self) -> usize {
        self.source_channel_index
    }

    /// Source animation-layer index, when the format has layers.
    pub const fn source_layer_index(&self) -> Option<usize> {
        self.source_layer_index
    }

    /// Stable source target identity.
    pub const fn target(&self) -> SourceTargetV1 {
        self.target
    }

    /// Format-neutral property kind.
    pub const fn property(&self) -> SourceChannelPropertyV1 {
        self.property
    }

    /// Source property spelling for an `Other` property, when retained.
    pub fn property_name(&self) -> Option<&SourceTextV1> {
        self.property_name.as_ref()
    }

    /// Present component curves.
    pub const fn components(&self) -> SourceComponentMaskV1 {
        self.components
    }

    /// Source interpolation evidence.
    pub const fn interpolation(&self) -> &SourceObservationV1<SourceInterpolationV1> {
        &self.interpolation
    }

    /// Exact glTF input accessor index, when applicable.
    pub const fn input_accessor_index(&self) -> Option<usize> {
        self.input_accessor_index
    }

    /// Exact glTF output accessor index, when applicable.
    pub const fn output_accessor_index(&self) -> Option<usize> {
        self.output_accessor_index
    }

    /// AnimSmith-loader treatment of this channel/property.
    pub const fn disposition(&self) -> SourceLoaderDispositionV1 {
        self.disposition
    }

    /// Source provenance for this declaration.
    pub const fn provenance(&self) -> &SourceProvenanceV1 {
        &self.provenance
    }

    fn retained_bytes(&self) -> usize {
        self.property_name
            .as_ref()
            .map_or(0, SourceTextV1::retained_bytes)
            .saturating_add(self.interpolation.retained_bytes())
            .saturating_add(self.provenance.retained_bytes())
    }
}

/// One source animation/stack and its independently covered channel rows.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceClipFactV1 {
    source_clip_index: usize,
    source_name: SourceObservationV1<SourceTextV1>,
    normalized_clip_index: SourceObservationV1<usize>,
    source_range: SourceObservationV1<SourceTimeRangeV1>,
    sampler_range: SourceObservationV1<SourceTimeRangeV1>,
    channels: SourceFactSetV1<SourceChannelFactV1>,
}

impl SourceClipFactV1 {
    /// Construct one source clip/take fact row.
    pub fn new(
        source_clip_index: usize,
        source_name: SourceObservationV1<SourceTextV1>,
        normalized_clip_index: SourceObservationV1<usize>,
        source_range: SourceObservationV1<SourceTimeRangeV1>,
        sampler_range: SourceObservationV1<SourceTimeRangeV1>,
        channels: SourceFactSetV1<SourceChannelFactV1>,
    ) -> Self {
        Self {
            source_clip_index,
            source_name,
            normalized_clip_index,
            source_range,
            sampler_range,
            channels,
        }
    }

    /// Stable source animation/stack index.
    pub const fn source_clip_index(&self) -> usize {
        self.source_clip_index
    }

    /// Authored/parser source name, distinct from a normalized synthetic name.
    pub const fn source_name(&self) -> &SourceObservationV1<SourceTextV1> {
        &self.source_name
    }

    /// Mapping into the normalized [`Document::clips`] array.
    pub const fn normalized_clip_index(&self) -> &SourceObservationV1<usize> {
        &self.normalized_clip_index
    }

    /// Parser-resolved source stack range, when this source format has one.
    pub const fn source_range(&self) -> &SourceObservationV1<SourceTimeRangeV1> {
        &self.source_range
    }

    /// Range derived from exact authored sampler inputs, when applicable.
    pub const fn sampler_range(&self) -> &SourceObservationV1<SourceTimeRangeV1> {
        &self.sampler_range
    }

    /// Raw channel/property rows and their independent coverage.
    pub const fn channels(&self) -> &SourceFactSetV1<SourceChannelFactV1> {
        &self.channels
    }

    fn retained_row_count(&self) -> usize {
        1usize.saturating_add(self.channels.rows.len())
    }

    fn retained_bytes(&self) -> usize {
        self.retained_non_channel_bytes().saturating_add(
            self.channels
                .rows
                .iter()
                .map(SourceChannelFactV1::retained_bytes)
                .fold(0usize, usize::saturating_add),
        )
    }

    fn retained_non_channel_bytes(&self) -> usize {
        self.source_name
            .retained_text_bytes()
            .saturating_add(self.normalized_clip_index.retained_bytes())
            .saturating_add(self.source_range.retained_bytes())
            .saturating_add(self.sampler_range.retained_bytes())
    }

    fn truncate_channels(&mut self, retained: usize) {
        if self.channels.rows.len() > retained {
            self.channels.rows.truncate(retained);
            self.channels
                .mark_partial(SourceUnavailableReasonV1::ProjectionBudgetExceeded);
        }
    }

    fn truncate_channels_to_text(&mut self, available_bytes: usize) -> bool {
        let fixed_bytes = self.retained_non_channel_bytes();
        if fixed_bytes > available_bytes {
            return false;
        }
        let mut retained_bytes = fixed_bytes;
        let retained_channels = self
            .channels
            .rows
            .iter()
            .take_while(|channel| {
                let next = retained_bytes.saturating_add(channel.retained_bytes());
                if next > available_bytes {
                    false
                } else {
                    retained_bytes = next;
                    true
                }
            })
            .count();
        self.truncate_channels(retained_channels);
        true
    }
}

/// General source construct kind retained outside normalized animation tracks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SourceConstructKindV1 {
    /// glTF extension declaration.
    Extension,
    /// Parser-projected user/custom property domain.
    CustomProperty,
    /// Parser-projected unmodeled source element/domain.
    UnknownElement,
}

/// One extension/custom/unmodeled source construct declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceConstructFactV1 {
    source_order_index: usize,
    kind: SourceConstructKindV1,
    name: SourceTextV1,
    required: bool,
    count: u64,
    disposition: SourceLoaderDispositionV1,
    provenance: SourceProvenanceV1,
}

impl SourceConstructFactV1 {
    /// Construct one source construct fact.
    ///
    /// # Errors
    ///
    /// Returns [`SourceFactsError::ZeroConstructCount`] because every retained
    /// row is positive-presence evidence.
    pub fn new(
        source_order_index: usize,
        kind: SourceConstructKindV1,
        name: SourceTextV1,
        required: bool,
        count: u64,
        disposition: SourceLoaderDispositionV1,
        provenance: SourceProvenanceV1,
    ) -> Result<Self, SourceFactsError> {
        if count == 0 {
            return Err(SourceFactsError::ZeroConstructCount);
        }
        Ok(Self {
            source_order_index,
            kind,
            name,
            required,
            count,
            disposition,
            provenance,
        })
    }

    /// Zero-based deterministic row order in this projection domain.
    pub const fn source_order_index(&self) -> usize {
        self.source_order_index
    }

    /// Construct kind.
    pub const fn kind(&self) -> SourceConstructKindV1 {
        self.kind
    }

    /// Bounded source construct spelling.
    pub const fn name(&self) -> &SourceTextV1 {
        &self.name
    }

    /// Whether the source declaration makes the construct required.
    pub const fn required(&self) -> bool {
        self.required
    }

    /// Number of source occurrences represented by this aggregate row.
    pub const fn count(&self) -> u64 {
        self.count
    }

    /// AnimSmith-loader treatment.
    pub const fn disposition(&self) -> SourceLoaderDispositionV1 {
        self.disposition
    }

    /// Source provenance.
    pub const fn provenance(&self) -> &SourceProvenanceV1 {
        &self.provenance
    }

    fn retained_bytes(&self) -> usize {
        self.name
            .retained_bytes()
            .saturating_add(self.provenance.retained_bytes())
    }
}

/// Source declaration kind that may refer to external content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceResourceKindV1 {
    /// glTF buffer declaration.
    Buffer,
    /// glTF image declaration.
    Image,
    /// FBX texture declaration.
    Texture,
    /// FBX video declaration.
    Video,
    /// FBX geometry-cache declaration.
    Cache,
}

/// Redacted classification of one resource locator declaration.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum SourceResourceLocatorV1 {
    /// Payload is embedded in the primary container/buffer.
    Embedded,
    /// Payload is carried by a data URI; payload/spelling is not retained.
    DataUri,
    /// Bounded exact relative spelling, not normalized or deduplicated.
    Relative(SourceRelativeLocatorV1),
    /// Absolute locator; spelling is redacted.
    Absolute,
    /// Escaping/traversal locator; spelling is redacted.
    Escaping,
    /// Remote locator; spelling is redacted.
    Remote,
    /// Malformed locator; spelling is redacted.
    Malformed,
    /// Oversized locator; spelling is redacted.
    Oversized,
    /// Declaration has no locator.
    Missing,
}

impl SourceResourceLocatorV1 {
    /// Classify a resource spelling while redacting unsafe or oversized input.
    ///
    /// This is lexical classification only. It deliberately does not
    /// normalize, deduplicate, open, or assign resource identity; #475 owns
    /// those dependency-closure operations.
    pub fn classify(value: &str) -> Self {
        if let Some(classification) = redacted_resource_locator(value) {
            return classification;
        }
        SourceTextV1::new(value).map_or(Self::Oversized, |value| {
            Self::Relative(SourceRelativeLocatorV1(value))
        })
    }

    /// Exact UTF-8 bytes [`Self::classify`] would retain, without allocating.
    ///
    /// Loaders use this to reserve aggregate text capacity before constructing
    /// a safe-relative locator. Redacted classifications return zero.
    pub fn retained_relative_bytes(value: &str) -> usize {
        if redacted_resource_locator(value).is_none() {
            value.len()
        } else {
            0
        }
    }

    fn retained_bytes(&self) -> usize {
        match self {
            Self::Relative(value) => value.0.retained_bytes(),
            _ => 0,
        }
    }
}

fn redacted_resource_locator(value: &str) -> Option<SourceResourceLocatorV1> {
    if value
        .get(..5)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("data:"))
    {
        return Some(SourceResourceLocatorV1::DataUri);
    }
    if value.len() > RAW_SOURCE_V1_MAX_TEXT_BYTES {
        return Some(SourceResourceLocatorV1::Oversized);
    }
    if value.is_empty() || value.chars().any(char::is_control) || malformed_percent_escape(value) {
        return Some(SourceResourceLocatorV1::Malformed);
    }
    if value.starts_with(['/', '\\'])
        || value.as_bytes().get(1).is_some_and(|value| *value == b':')
        || value
            .get(..5)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("file:"))
    {
        return Some(SourceResourceLocatorV1::Absolute);
    }
    if has_uri_scheme(value) {
        return Some(SourceResourceLocatorV1::Remote);
    }
    let mut escaped = false;
    let mut malformed = false;
    for component in value.split(['/', '\\']) {
        escaped |= component == ".." || is_encoded_dot_segment(component);
        malformed |= component.is_empty() || component == ".";
    }
    if escaped || contains_encoded_path_escape(value) {
        return Some(SourceResourceLocatorV1::Escaping);
    }
    if malformed {
        return Some(SourceResourceLocatorV1::Malformed);
    }
    None
}

/// Validated bounded exact relative locator spelling.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SourceRelativeLocatorV1(SourceTextV1);

impl SourceRelativeLocatorV1 {
    /// Exact relative spelling; no normalization has been applied.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

fn malformed_percent_escape(value: &str) -> bool {
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'%' {
            index += 1;
            continue;
        }
        if bytes.get(index + 1).and_then(|value| hex(*value)).is_none()
            || bytes.get(index + 2).and_then(|value| hex(*value)).is_none()
        {
            return true;
        }
        index += 3;
    }
    false
}

fn contains_encoded_path_escape(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("%2f") || lower.contains("%5c") || lower.contains("%00")
}

fn is_encoded_dot_segment(value: &str) -> bool {
    let bytes = value.as_bytes();
    let mut index = 0;
    let mut dots = 0;
    let mut encoded = false;
    while index < bytes.len() {
        if bytes[index] == b'.' {
            dots += 1;
            index += 1;
        } else if bytes.get(index..index + 3).is_some_and(|escape| {
            escape[0] == b'%' && escape[1] == b'2' && matches!(escape[2], b'e' | b'E')
        }) {
            dots += 1;
            encoded = true;
            index += 3;
        } else {
            return false;
        }
        if dots > 2 {
            return false;
        }
    }
    encoded && matches!(dots, 1 | 2)
}

fn hex(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn has_uri_scheme(value: &str) -> bool {
    let Some((scheme, _)) = value.split_once(':') else {
        return false;
    };
    !scheme.is_empty()
        && scheme.as_bytes()[0].is_ascii_alphabetic()
        && scheme
            .bytes()
            .all(|value| value.is_ascii_alphanumeric() || matches!(value, b'+' | b'-' | b'.'))
}

/// One source resource declaration, not a resolved dependency identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceResourceReferenceV1 {
    source_order_index: usize,
    kind: SourceResourceKindV1,
    source_index: u64,
    locator: SourceResourceLocatorV1,
    disposition: SourceLoaderDispositionV1,
    provenance: SourceProvenanceV1,
}

impl SourceResourceReferenceV1 {
    /// Construct one stable declaration row.
    pub fn new(
        source_order_index: usize,
        kind: SourceResourceKindV1,
        source_index: u64,
        locator: SourceResourceLocatorV1,
        disposition: SourceLoaderDispositionV1,
        provenance: SourceProvenanceV1,
    ) -> Self {
        Self {
            source_order_index,
            kind,
            source_index,
            locator,
            disposition,
            provenance,
        }
    }

    /// Zero-based deterministic declaration order in this projection domain.
    pub const fn source_order_index(&self) -> usize {
        self.source_order_index
    }

    /// Resource declaration kind.
    pub const fn kind(&self) -> SourceResourceKindV1 {
        self.kind
    }

    /// Stable source/parser declaration index.
    pub const fn source_index(&self) -> u64 {
        self.source_index
    }

    /// Redacted declaration classification.
    pub const fn locator(&self) -> &SourceResourceLocatorV1 {
        &self.locator
    }

    /// AnimSmith-loader treatment of the declaration/content domain.
    pub const fn disposition(&self) -> SourceLoaderDispositionV1 {
        self.disposition
    }

    /// Source provenance.
    pub const fn provenance(&self) -> &SourceProvenanceV1 {
        &self.provenance
    }

    fn retained_bytes(&self) -> usize {
        self.locator
            .retained_bytes()
            .saturating_add(self.provenance.retained_bytes())
    }
}

/// Deterministic work retained for one V1 projection.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SourceProjectionWorkV1 {
    inspected_rows: usize,
    retained_rows: usize,
    retained_text_bytes: usize,
    max_traversal_depth: usize,
}

impl SourceProjectionWorkV1 {
    /// Rows inspected before bounded projection stopped affected sets.
    pub const fn inspected_rows(self) -> usize {
        self.inspected_rows
    }

    /// Rows retained across clips/channels, constructs, and resource declarations.
    pub const fn retained_rows(self) -> usize {
        self.retained_rows
    }

    /// Total UTF-8 bytes retained in source names and logical locators.
    pub const fn retained_text_bytes(self) -> usize {
        self.retained_text_bytes
    }

    /// Greatest traversal depth inspected, capped at V1 limit plus one.
    pub const fn max_traversal_depth(self) -> usize {
        self.max_traversal_depth
    }
}

/// Independently bounded enumerable projection domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceFactDomainV1 {
    /// Clips/takes and nested channel/property rows.
    Clips,
    /// Extension/custom/unmodeled construct rows.
    Constructs,
    /// External-resource declaration rows.
    Resources,
}

impl SourceFactDomainV1 {
    const fn index(self) -> usize {
        match self {
            Self::Clips => 0,
            Self::Constructs => 1,
            Self::Resources => 2,
        }
    }
}

/// Immutable owned V1 facts bound into a [`LoadedSource`].
#[derive(Debug, Clone, PartialEq)]
pub struct RawSourceFactsV1 {
    format: SourceFormatV1,
    primary_identity: InputIdentity,
    linear_unit: SourceObservationV1<SourceLinearUnitV1>,
    coordinate_basis: SourceObservationV1<SourceCoordinateBasisV1>,
    frames_per_second: SourceObservationV1<SourceFramesPerSecondV1>,
    clips: SourceFactSetV1<SourceClipFactV1>,
    constructs: SourceFactSetV1<SourceConstructFactV1>,
    resources: SourceFactSetV1<SourceResourceReferenceV1>,
    work: SourceProjectionWorkV1,
}

/// Builder used by format loaders to create a bounded immutable V1 projection.
pub struct RawSourceFactsBuilderV1 {
    facts: RawSourceFactsV1,
    stopped: [bool; 3],
}

fn unavailable_observation<T>() -> SourceObservationV1<T> {
    SourceObservationV1::unavailable(
        SourceUnavailableReasonV1::ParserUnavailable,
        None,
        SourceLoaderDispositionV1::Unknown,
    )
}

fn replace_scalar_observation<T>(
    work: &mut SourceProjectionWorkV1,
    slot: &mut SourceObservationV1<T>,
    value: SourceObservationV1<T>,
) -> bool {
    let previous_bytes = slot.retained_bytes();
    let value_bytes = value.retained_bytes();
    let baseline = work.retained_text_bytes.saturating_sub(previous_bytes);
    if baseline
        .checked_add(value_bytes)
        .is_some_and(|total| total <= RAW_SOURCE_V1_MAX_TOTAL_TEXT_BYTES)
    {
        work.retained_text_bytes = baseline.saturating_add(value_bytes);
        *slot = value;
        true
    } else {
        work.retained_text_bytes = baseline;
        *slot = SourceObservationV1::unavailable(
            SourceUnavailableReasonV1::ProjectionBudgetExceeded,
            None,
            SourceLoaderDispositionV1::Unknown,
        );
        false
    }
}

impl RawSourceFactsBuilderV1 {
    /// Begin one projection bound to the exact primary bytes already captured by the loader.
    pub fn new(format: SourceFormatV1, primary_identity: InputIdentity) -> Self {
        Self {
            facts: RawSourceFactsV1 {
                format,
                primary_identity,
                linear_unit: unavailable_observation(),
                coordinate_basis: unavailable_observation(),
                frames_per_second: unavailable_observation(),
                clips: SourceFactSetV1::unavailable(SourceUnavailableReasonV1::ParserUnavailable),
                constructs: SourceFactSetV1::unavailable(
                    SourceUnavailableReasonV1::ParserUnavailable,
                ),
                resources: SourceFactSetV1::unavailable(
                    SourceUnavailableReasonV1::ParserUnavailable,
                ),
                work: SourceProjectionWorkV1::default(),
            },
            stopped: [false; 3],
        }
    }

    /// Set the source linear-unit observation.
    ///
    /// Returns `false` when its provenance would exceed the aggregate text
    /// budget; the stored observation is then unavailable with a budget reason.
    pub fn set_linear_unit(&mut self, value: SourceObservationV1<SourceLinearUnitV1>) -> bool {
        replace_scalar_observation(&mut self.facts.work, &mut self.facts.linear_unit, value)
    }

    /// Set the signed coordinate-basis observation.
    pub fn set_coordinate_basis(
        &mut self,
        value: SourceObservationV1<SourceCoordinateBasisV1>,
    ) -> bool {
        replace_scalar_observation(
            &mut self.facts.work,
            &mut self.facts.coordinate_basis,
            value,
        )
    }

    /// Set the source frame-rate observation.
    pub fn set_frames_per_second(
        &mut self,
        value: SourceObservationV1<SourceFramesPerSecondV1>,
    ) -> bool {
        replace_scalar_observation(
            &mut self.facts.work,
            &mut self.facts.frames_per_second,
            value,
        )
    }

    /// Observation-row capacity remaining across all enumerable domains.
    ///
    /// Loaders use this before allocating the next nested channel prefix.
    pub const fn remaining_observation_rows(&self) -> usize {
        RAW_SOURCE_V1_MAX_OBSERVATIONS.saturating_sub(self.facts.work.retained_rows)
    }

    /// Clip/take identity-row capacity remaining.
    pub fn remaining_clip_rows(&self) -> usize {
        RAW_SOURCE_V1_MAX_CLIPS.saturating_sub(self.facts.clips.rows.len())
    }

    /// Resource-reference row capacity remaining.
    pub fn remaining_resource_rows(&self) -> usize {
        RAW_SOURCE_V1_MAX_RESOURCE_REFERENCES.saturating_sub(self.facts.resources.rows.len())
    }

    /// Current raw resource-declaration coverage for closure capture.
    pub const fn resource_coverage(&self) -> SourceSetCoverageV1 {
        self.facts.resources.coverage
    }

    /// Retained raw resource-declaration prefix for closure capture.
    pub fn resource_rows(&self) -> &[SourceResourceReferenceV1] {
        &self.facts.resources.rows
    }

    /// Exact primary identity this projection is bound to.
    pub const fn primary_identity(&self) -> &InputIdentity {
        &self.facts.primary_identity
    }

    /// Retained UTF-8 byte capacity remaining.
    ///
    /// Loaders use this before cloning the next source name or locator.
    pub const fn remaining_text_bytes(&self) -> usize {
        RAW_SOURCE_V1_MAX_TOTAL_TEXT_BYTES.saturating_sub(self.facts.work.retained_text_bytes)
    }

    /// Mark a row domain unavailable and discard any retained prefix.
    pub fn mark_unavailable(
        &mut self,
        domain: SourceFactDomainV1,
        reason: SourceUnavailableReasonV1,
    ) {
        let (retained_rows, retained_bytes) = match domain {
            SourceFactDomainV1::Clips => self
                .facts
                .clips
                .rows
                .iter()
                .map(|clip| (clip.retained_row_count(), clip.retained_bytes()))
                .fold(
                    (0usize, 0usize),
                    |(rows, bytes), (clip_rows, clip_bytes)| {
                        (
                            rows.saturating_add(clip_rows),
                            bytes.saturating_add(clip_bytes),
                        )
                    },
                ),
            SourceFactDomainV1::Constructs => (
                self.facts.constructs.rows.len(),
                self.facts
                    .constructs
                    .rows
                    .iter()
                    .map(SourceConstructFactV1::retained_bytes)
                    .fold(0usize, usize::saturating_add),
            ),
            SourceFactDomainV1::Resources => (
                self.facts.resources.rows.len(),
                self.facts
                    .resources
                    .rows
                    .iter()
                    .map(SourceResourceReferenceV1::retained_bytes)
                    .fold(0usize, usize::saturating_add),
            ),
        };
        self.facts.work.retained_rows = self.facts.work.retained_rows.saturating_sub(retained_rows);
        self.facts.work.retained_text_bytes = self
            .facts
            .work
            .retained_text_bytes
            .saturating_sub(retained_bytes);
        self.stopped[domain.index()] = true;
        match domain {
            SourceFactDomainV1::Clips => self.facts.clips = SourceFactSetV1::unavailable(reason),
            SourceFactDomainV1::Constructs => {
                self.facts.constructs = SourceFactSetV1::unavailable(reason)
            }
            SourceFactDomainV1::Resources => {
                self.facts.resources = SourceFactSetV1::unavailable(reason)
            }
        }
    }

    /// Mark a retained row domain partial without removing positive-presence rows.
    pub fn mark_partial(&mut self, domain: SourceFactDomainV1, reason: SourceUnavailableReasonV1) {
        if !self.stopped[domain.index()] {
            *self.set_for_domain_mut(domain) = SourceSetCoverageV1::partial(reason);
        }
    }

    /// Confirm exhaustive traversal for one row domain.
    ///
    /// This proves absence for an empty set only when no earlier partial or
    /// terminal-unavailable condition was recorded.
    pub fn mark_complete(&mut self, domain: SourceFactDomainV1) {
        if !self.stopped[domain.index()]
            && matches!(
                self.set_for_domain_mut(domain).state(),
                SourceSetCoverageStateV1::Unavailable
            )
        {
            *self.set_for_domain_mut(domain) = SourceSetCoverageV1::complete();
        }
    }

    /// Record the terminal N+1 row without constructing or cloning it.
    ///
    /// Loaders call this once when a public remaining-capacity accessor reaches
    /// zero, then stop projection work for that set.
    pub fn mark_budget_exceeded(&mut self, domain: SourceFactDomainV1) {
        if self.stopped[domain.index()] {
            return;
        }
        self.facts.work.inspected_rows = self.facts.work.inspected_rows.saturating_add(1);
        self.stop_for_budget(domain);
    }

    /// Record source traversal work and stop the affected domain at depth N+1.
    ///
    /// Returns `true` while projection may continue for that domain.
    pub fn observe_traversal_depth(&mut self, domain: SourceFactDomainV1, depth: usize) -> bool {
        if self.stopped[domain.index()] {
            return false;
        }
        self.facts.work.max_traversal_depth = self
            .facts
            .work
            .max_traversal_depth
            .max(depth.min(RAW_SOURCE_V1_MAX_TRAVERSAL_DEPTH + 1));
        if depth > RAW_SOURCE_V1_MAX_TRAVERSAL_DEPTH {
            self.stop_for_budget(domain);
            return false;
        }
        true
    }

    /// Retain one source-order clip/take row and its channel prefix.
    ///
    /// Returns `true` when the clip identity row was retained. Budget excess
    /// never becomes a loader failure.
    pub fn push_clip(&mut self, mut clip: SourceClipFactV1) -> bool {
        if self.stopped[SourceFactDomainV1::Clips.index()] {
            return false;
        }
        if self.facts.clips.rows.len() >= RAW_SOURCE_V1_MAX_CLIPS {
            self.mark_budget_exceeded(SourceFactDomainV1::Clips);
            return false;
        }
        let remaining_rows =
            RAW_SOURCE_V1_MAX_OBSERVATIONS.saturating_sub(self.facts.work.retained_rows);
        if remaining_rows == 0 {
            self.mark_budget_exceeded(SourceFactDomainV1::Clips);
            return false;
        }
        let original_rows = clip.retained_row_count();
        let supplied_budget_prefix = matches!(
            clip.channels.coverage(),
            SourceSetCoverageV1 {
                state: SourceSetCoverageStateV1::Partial,
                reason: Some(SourceUnavailableReasonV1::ProjectionBudgetExceeded),
            }
        );
        let mut builder_truncated = false;
        if clip.retained_row_count() > remaining_rows {
            clip.truncate_channels(remaining_rows - 1);
            builder_truncated = true;
        }
        if !clip.truncate_channels_to_text(self.remaining_text_bytes()) {
            self.mark_budget_exceeded(SourceFactDomainV1::Clips);
            return false;
        }
        builder_truncated |= clip.retained_row_count() < original_rows;
        let retained_rows = clip.retained_row_count();
        let inspected_rows = if builder_truncated || supplied_budget_prefix {
            retained_rows.saturating_add(1)
        } else {
            retained_rows
        };
        self.facts.work.inspected_rows = self
            .facts
            .work
            .inspected_rows
            .saturating_add(inspected_rows);
        if builder_truncated || supplied_budget_prefix {
            self.stop_for_budget(SourceFactDomainV1::Clips);
        }
        self.retain_work(clip.retained_row_count(), clip.retained_bytes());
        self.facts.clips.rows.push(clip);
        true
    }

    /// Retain one deterministic extension/custom construct row.
    pub fn push_construct(&mut self, row: SourceConstructFactV1) -> bool {
        if self.stopped[SourceFactDomainV1::Constructs.index()] {
            return false;
        }
        if !self.can_retain_row(row.retained_bytes()) {
            self.mark_budget_exceeded(SourceFactDomainV1::Constructs);
            return false;
        }
        self.facts.work.inspected_rows = self.facts.work.inspected_rows.saturating_add(1);
        self.retain_work(1, row.retained_bytes());
        self.facts.constructs.rows.push(row);
        true
    }

    /// Retain one deterministic resource-declaration row.
    pub fn push_resource(&mut self, row: SourceResourceReferenceV1) -> bool {
        if self.stopped[SourceFactDomainV1::Resources.index()] {
            return false;
        }
        if self.facts.resources.rows.len() >= RAW_SOURCE_V1_MAX_RESOURCE_REFERENCES
            || !self.can_retain_row(row.retained_bytes())
        {
            self.mark_budget_exceeded(SourceFactDomainV1::Resources);
            return false;
        }
        self.facts.work.inspected_rows = self.facts.work.inspected_rows.saturating_add(1);
        self.retain_work(1, row.retained_bytes());
        self.facts.resources.rows.push(row);
        true
    }

    /// Bind the retained facts to the normalized document from the same parse.
    ///
    /// # Errors
    ///
    /// Returns [`SourceFactsError`] when source row ordering or normalized clip
    /// mappings contradict the document being bound.
    pub fn finish(mut self, document: Document) -> Result<LoadedSource, SourceFactsError> {
        self.qualify_unfinished_positive_rows();
        let closure = DependencyClosureV1::capture_unavailable(
            self.facts.primary_identity.clone(),
            self.facts.resources.coverage,
        );
        self.finish_with_dependency_closure(document, closure)
    }

    /// Bind retained facts and dependency closure to one normalized document.
    ///
    /// This is the format-loader completion path once bounded resource capture
    /// has run. The closure primary identity and retained reference prefix must
    /// match this raw projection exactly.
    ///
    /// # Errors
    ///
    /// Returns [`SourceFactsError`] for source row/document contradictions or
    /// a mismatched dependency closure.
    pub fn finish_with_dependency_closure(
        mut self,
        document: Document,
        dependency_closure: DependencyClosureV1,
    ) -> Result<LoadedSource, SourceFactsError> {
        self.qualify_unfinished_positive_rows();
        validate_clip_rows(&self.facts.clips, document.clips.len())?;
        validate_ordered_rows(&self.facts.constructs, &self.facts.resources)?;
        dependency_closure.validate_against(
            self.facts.format,
            &self.facts.primary_identity,
            &self.facts.resources,
        )?;
        Ok(LoadedSource {
            document,
            facts: self.facts,
            dependency_closure,
            exact_source_timing: None,
            raw_gltf_addressability_inventory: None,
            raw_scene_attachment_inventory: None,
            raw_transform_path_inventory: None,
        })
    }

    fn can_retain_text(&self, bytes: usize) -> bool {
        self.facts
            .work
            .retained_text_bytes
            .checked_add(bytes)
            .is_some_and(|total| total <= RAW_SOURCE_V1_MAX_TOTAL_TEXT_BYTES)
    }

    fn can_retain_row(&self, bytes: usize) -> bool {
        self.facts.work.retained_rows < RAW_SOURCE_V1_MAX_OBSERVATIONS
            && self.can_retain_text(bytes)
    }

    fn retain_work(&mut self, rows: usize, bytes: usize) {
        self.facts.work.retained_rows = self.facts.work.retained_rows.saturating_add(rows);
        self.facts.work.retained_text_bytes =
            self.facts.work.retained_text_bytes.saturating_add(bytes);
    }

    fn set_for_domain_mut(&mut self, domain: SourceFactDomainV1) -> &mut SourceSetCoverageV1 {
        match domain {
            SourceFactDomainV1::Clips => &mut self.facts.clips.coverage,
            SourceFactDomainV1::Constructs => &mut self.facts.constructs.coverage,
            SourceFactDomainV1::Resources => &mut self.facts.resources.coverage,
        }
    }

    fn stop_for_budget(&mut self, domain: SourceFactDomainV1) {
        *self.set_for_domain_mut(domain) =
            SourceSetCoverageV1::partial(SourceUnavailableReasonV1::ProjectionBudgetExceeded);
        self.stopped[domain.index()] = true;
    }

    fn qualify_unfinished_positive_rows(&mut self) {
        for domain in [
            SourceFactDomainV1::Clips,
            SourceFactDomainV1::Constructs,
            SourceFactDomainV1::Resources,
        ] {
            let has_rows = match domain {
                SourceFactDomainV1::Clips => !self.facts.clips.rows.is_empty(),
                SourceFactDomainV1::Constructs => !self.facts.constructs.rows.is_empty(),
                SourceFactDomainV1::Resources => !self.facts.resources.rows.is_empty(),
            };
            if has_rows
                && matches!(
                    self.set_for_domain_mut(domain).state(),
                    SourceSetCoverageStateV1::Unavailable
                )
            {
                *self.set_for_domain_mut(domain) =
                    SourceSetCoverageV1::partial(SourceUnavailableReasonV1::ParserUnavailable);
            }
        }
    }
}

fn validate_clip_rows(
    clips: &SourceFactSetV1<SourceClipFactV1>,
    normalized_clip_count: usize,
) -> Result<(), SourceFactsError> {
    for (expected_clip_index, clip) in clips.rows.iter().enumerate() {
        if clip.source_clip_index != expected_clip_index {
            return Err(SourceFactsError::NonCanonicalClipIndex {
                expected: expected_clip_index,
                actual: clip.source_clip_index,
            });
        }
        if let SourceObservationStateV1::Observed(index) = clip.normalized_clip_index.state()
            && *index >= normalized_clip_count
        {
            return Err(SourceFactsError::NormalizedClipIndexOutOfRange {
                index: *index,
                clip_count: normalized_clip_count,
            });
        }
        for (expected_channel_index, channel) in clip.channels.rows.iter().enumerate() {
            if channel.source_channel_index != expected_channel_index {
                return Err(SourceFactsError::NonCanonicalChannelIndex {
                    source_clip_index: clip.source_clip_index,
                    expected: expected_channel_index,
                    actual: channel.source_channel_index,
                });
            }
        }
    }
    Ok(())
}

fn validate_ordered_rows(
    constructs: &SourceFactSetV1<SourceConstructFactV1>,
    resources: &SourceFactSetV1<SourceResourceReferenceV1>,
) -> Result<(), SourceFactsError> {
    for (expected, row) in constructs.rows.iter().enumerate() {
        if row.source_order_index != expected {
            return Err(SourceFactsError::NonCanonicalConstructOrder {
                expected,
                actual: row.source_order_index,
            });
        }
    }
    for (expected, row) in resources.rows.iter().enumerate() {
        if row.source_order_index != expected {
            return Err(SourceFactsError::NonCanonicalResourceOrder {
                expected,
                actual: row.source_order_index,
            });
        }
    }
    Ok(())
}

/// Immutable source-plus-normalized-document owner returned by format loaders.
///
/// This type intentionally provides no mutable document reference and no
/// operation that separates live facts from a mutable `Document`.
pub struct LoadedSource {
    document: Document,
    facts: RawSourceFactsV1,
    dependency_closure: DependencyClosureV1,
    exact_source_timing: Option<ExactSourceTimingV1>,
    raw_gltf_addressability_inventory: Option<RawGltfAddressabilityInventoryV1>,
    raw_scene_attachment_inventory: Option<RawSceneAttachmentInventoryV1>,
    raw_transform_path_inventory: Option<RawTransformPathInventoryV1>,
}

impl fmt::Debug for LoadedSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LoadedSource")
            .field("format", &self.facts.format)
            .field("primary_identity", &self.facts.primary_identity)
            .field("dependency_closure", &self.dependency_closure)
            .field("exact_source_timing", &self.exact_source_timing)
            .field(
                "raw_gltf_addressability_inventory",
                &self.raw_gltf_addressability_inventory,
            )
            .field(
                "raw_scene_attachment_inventory",
                &self.raw_scene_attachment_inventory,
            )
            .field(
                "raw_transform_path_inventory",
                &self.raw_transform_path_inventory,
            )
            .field("work", &self.facts.work)
            .finish_non_exhaustive()
    }
}

impl LoadedSource {
    /// Borrow the normalized document without permitting mutation beside live facts.
    pub const fn document(&self) -> &Document {
        &self.document
    }

    /// Borrow the V1 facts together with the canonical existing source skeleton projection.
    pub fn source_facts(&self) -> SourceFactsViewV1<'_> {
        SourceFactsViewV1 {
            facts: &self.facts,
            source_skeleton: &self.document.assets.source_skeleton,
        }
    }

    /// Borrow the bounded dependency closure captured during this same load.
    pub const fn dependency_closure(&self) -> &DependencyClosureV1 {
        &self.dependency_closure
    }

    /// Attach bounded exact timing evidence produced by the same loader parse.
    ///
    /// # Errors
    ///
    /// Returns [`ExactSourceTimingContractError`] when the exact clip prefix or
    /// coverage does not match the existing V1 clip domain.
    pub fn with_exact_source_timing(
        mut self,
        timing: ExactSourceTimingV1,
    ) -> Result<Self, ExactSourceTimingContractError> {
        if timing.clips().len() != self.facts.clips.rows.len() {
            return Err(ExactSourceTimingContractError::ClipCountMismatch {
                exact: timing.clips().len(),
                source_count: self.facts.clips.rows.len(),
            });
        }
        if timing.clip_coverage() != self.facts.clips.coverage {
            return Err(ExactSourceTimingContractError::ClipCoverageMismatch);
        }
        self.exact_source_timing = Some(timing);
        Ok(self)
    }

    /// Borrow exact source timing evidence when this loader retained it.
    ///
    /// Sources produced by callers that retain only legacy V1 facts return `None`.
    pub const fn exact_source_timing(&self) -> Option<&ExactSourceTimingV1> {
        self.exact_source_timing.as_ref()
    }

    /// Attach bounded raw glTF addressability evidence from this exact load.
    ///
    /// # Errors
    ///
    /// Returns [`RawGltfAddressabilityBindingErrorV1`] when the inventory is
    /// invalid, belongs to a non-glTF source, or disagrees with the exact
    /// primary input or dependency closure already bound to this source.
    pub fn with_raw_gltf_addressability_inventory(
        mut self,
        inventory: RawGltfAddressabilityInventoryV1,
    ) -> Result<Self, RawGltfAddressabilityBindingErrorV1> {
        inventory
            .validate()
            .map_err(|_| RawGltfAddressabilityBindingErrorV1::InvalidInventory)?;
        if !matches!(
            self.facts.format,
            SourceFormatV1::GltfJson | SourceFormatV1::Glb
        ) {
            return Err(RawGltfAddressabilityBindingErrorV1::UnsupportedSourceFormat);
        }
        if inventory.primary_input() != &self.facts.primary_identity {
            return Err(RawGltfAddressabilityBindingErrorV1::PrimaryIdentityMismatch);
        }
        if inventory.dependency_closure() != &self.dependency_closure {
            return Err(RawGltfAddressabilityBindingErrorV1::DependencyClosureMismatch);
        }
        self.raw_gltf_addressability_inventory = Some(inventory);
        Ok(self)
    }

    /// Borrow same-load raw glTF scene/node/skin/path evidence when retained.
    pub const fn raw_gltf_addressability_inventory(
        &self,
    ) -> Option<&RawGltfAddressabilityInventoryV1> {
        self.raw_gltf_addressability_inventory.as_ref()
    }

    /// Attach bounded raw scene/attachment evidence from this exact glTF load.
    ///
    /// # Errors
    ///
    /// Returns [`RawSceneAttachmentBindingError`] when the inventory does not
    /// bind this source's exact primary identity, a glTF container, or the
    /// canonical source-skeleton evidence retained beside this document.
    pub fn with_raw_scene_attachment_inventory(
        mut self,
        inventory: RawSceneAttachmentInventoryV1,
    ) -> Result<Self, RawSceneAttachmentBindingError> {
        if !matches!(
            self.facts.format,
            SourceFormatV1::GltfJson | SourceFormatV1::Glb
        ) {
            return Err(RawSceneAttachmentBindingError::UnsupportedSourceFormat);
        }
        if inventory.primary_input() != &self.facts.primary_identity {
            return Err(RawSceneAttachmentBindingError::PrimaryIdentityMismatch);
        }
        if inventory.source_skeleton()
            != &source_skeleton_evidence(&self.document.assets.source_skeleton)
        {
            return Err(RawSceneAttachmentBindingError::SourceSkeletonMismatch);
        }
        self.raw_scene_attachment_inventory = Some(inventory);
        Ok(self)
    }

    /// Borrow same-load raw scene/attachment evidence when the loader retained it.
    ///
    /// Legacy and non-glTF producers return `None` rather than inferring
    /// source presence from normalized mesh assets.
    pub const fn raw_scene_attachment_inventory(&self) -> Option<&RawSceneAttachmentInventoryV1> {
        self.raw_scene_attachment_inventory.as_ref()
    }

    /// Attach bounded raw FBX transform-path evidence from this exact load.
    ///
    /// # Errors
    ///
    /// Returns [`RawTransformPathBindingError`] when the inventory is invalid,
    /// was not projected from FBX, identifies different primary bytes, or its
    /// same-load normalized bone count disagrees with the document.
    pub fn with_raw_transform_path_inventory(
        mut self,
        inventory: RawTransformPathInventoryV1,
    ) -> Result<Self, RawTransformPathBindingError> {
        inventory
            .validate()
            .map_err(|_| RawTransformPathBindingError::InvalidInventory)?;
        if self.facts.format != SourceFormatV1::Fbx
            || inventory.source_format() != SourceFormatV1::Fbx
        {
            return Err(RawTransformPathBindingError::UnsupportedSourceFormat);
        }
        if inventory.primary_input() != &self.facts.primary_identity {
            return Err(RawTransformPathBindingError::PrimaryIdentityMismatch);
        }
        if inventory.projected_bone_count() != self.document.skeleton.bones.len() as u64 {
            return Err(RawTransformPathBindingError::ProjectedBoneCountMismatch);
        }
        self.raw_transform_path_inventory = Some(inventory);
        Ok(self)
    }

    /// Borrow same-load raw FBX transform-path evidence when retained.
    pub const fn raw_transform_path_inventory(&self) -> Option<&RawTransformPathInventoryV1> {
        self.raw_transform_path_inventory.as_ref()
    }

    /// Consume the owner and deliberately discard importer-sensitive source facts.
    pub fn into_document(self) -> Document {
        self.document
    }
}

fn source_skeleton_evidence(source: &SourceSkeletonAssets) -> RawSourceSkeletonEvidenceV1 {
    RawSourceSkeletonEvidenceV1::new(
        match source.coverage {
            SourceSkeletonCoverage::Complete => RawSceneAttachmentCoverageV1::Complete,
            SourceSkeletonCoverage::Unavailable => RawSceneAttachmentCoverageV1::Unavailable,
        },
        source.nodes.len() as u64,
        source.skins.len() as u64,
    )
}

/// An attempted raw scene/attachment sidecar did not bind this loaded source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum RawSceneAttachmentBindingError {
    /// Raw scene/attachment inventories are currently defined only for glTF and GLB loads.
    #[error("raw scene/attachment inventory requires a glTF or GLB source")]
    UnsupportedSourceFormat,
    /// The inventory was produced from different primary bytes.
    #[error("raw scene/attachment inventory primary input does not match the loaded source")]
    PrimaryIdentityMismatch,
    /// The inventory's source-skeleton evidence does not match this loaded source.
    #[error(
        "raw scene/attachment inventory source-skeleton evidence does not match the loaded source"
    )]
    SourceSkeletonMismatch,
}

/// An attempted raw glTF addressability sidecar did not bind this loaded source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum RawGltfAddressabilityBindingErrorV1 {
    /// Raw glTF addressability inventories require glTF JSON or GLB input.
    #[error("raw glTF addressability inventory requires a glTF or GLB source")]
    UnsupportedSourceFormat,
    /// The standalone inventory failed semantic validation.
    #[error("raw glTF addressability inventory is invalid")]
    InvalidInventory,
    /// The inventory identifies different primary bytes.
    #[error("raw glTF addressability inventory primary input does not match loaded source")]
    PrimaryIdentityMismatch,
    /// The inventory embeds a different dependency-closure record.
    #[error("raw glTF addressability inventory dependency closure does not match loaded source")]
    DependencyClosureMismatch,
}

/// An attempted raw transform-path sidecar did not bind this loaded source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum RawTransformPathBindingError {
    /// Raw transform-path inventories V1 are currently defined only for FBX loads.
    #[error("raw transform-path inventory requires an FBX source")]
    UnsupportedSourceFormat,
    /// The inventory failed its standalone semantic validation.
    #[error("raw transform-path inventory is invalid")]
    InvalidInventory,
    /// The inventory was produced from different primary bytes.
    #[error("raw transform-path inventory primary input does not match the loaded source")]
    PrimaryIdentityMismatch,
    /// Same-load normalized bone cardinality does not match the document.
    #[error("raw transform-path inventory projected bone count does not match the document")]
    ProjectedBoneCountMismatch,
}

/// Borrowing view over V1 facts and canonical source skeleton evidence.
#[derive(Debug, Clone, Copy)]
pub struct SourceFactsViewV1<'a> {
    facts: &'a RawSourceFactsV1,
    source_skeleton: &'a SourceSkeletonAssets,
}

impl<'a> SourceFactsViewV1<'a> {
    /// Semantic identity of this in-memory vocabulary.
    pub const fn contract_id(self) -> &'static str {
        RAW_SOURCE_FACTS_V1_ID
    }

    /// Exact source container kind captured by the loader.
    pub const fn format(self) -> SourceFormatV1 {
        self.facts.format
    }

    /// SHA-256 and byte count of the exact primary bytes parsed by the loader.
    pub const fn primary_identity(self) -> &'a InputIdentity {
        &self.facts.primary_identity
    }

    /// Source linear-unit observation.
    pub const fn linear_unit(self) -> &'a SourceObservationV1<SourceLinearUnitV1> {
        &self.facts.linear_unit
    }

    /// Signed source coordinate-basis observation.
    pub const fn coordinate_basis(self) -> &'a SourceObservationV1<SourceCoordinateBasisV1> {
        &self.facts.coordinate_basis
    }

    /// Source frame-rate observation.
    pub const fn frames_per_second(self) -> &'a SourceObservationV1<SourceFramesPerSecondV1> {
        &self.facts.frames_per_second
    }

    /// Source clip/take rows and nested channel coverage.
    pub const fn clips(self) -> &'a SourceFactSetV1<SourceClipFactV1> {
        &self.facts.clips
    }

    /// Source extension/custom construct rows.
    pub const fn constructs(self) -> &'a SourceFactSetV1<SourceConstructFactV1> {
        &self.facts.constructs
    }

    /// Source resource declarations; these are not resolved dependency identities.
    pub const fn resources(self) -> &'a SourceFactSetV1<SourceResourceReferenceV1> {
        &self.facts.resources
    }

    /// Canonical pre-existing source node/skin evidence, never copied into the sidecar.
    pub const fn source_skeleton(self) -> &'a SourceSkeletonAssets {
        self.source_skeleton
    }

    /// Explicit projection work counters.
    pub const fn work(self) -> SourceProjectionWorkV1 {
        self.facts.work
    }
}

/// Invalid raw-source fact or document-binding invariant.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum SourceFactsError {
    /// One retained source string exceeds the public V1 limit.
    #[error("source text is {bytes} bytes, exceeding the V1 limit of {limit}")]
    TextTooLong {
        /// Observed UTF-8 byte count.
        bytes: usize,
        /// Public V1 limit.
        limit: usize,
    },
    /// A provenance locator is outside the generated source-logical vocabulary.
    #[error("source logical locator is invalid or unsafe")]
    InvalidLogicalLocator,
    /// Coordinate basis reuses an unsigned axis.
    #[error("source coordinate basis must use each unsigned axis exactly once")]
    DuplicateBasisAxis,
    /// Linear unit is not finite and positive.
    #[error("metres per source unit must be finite and positive")]
    InvalidLinearUnit,
    /// Source frame rate is not finite and positive.
    #[error("source frames per second must be finite and positive")]
    InvalidFramesPerSecond,
    /// Source range is non-finite or reversed.
    #[error("source time range endpoints must be finite with begin <= end")]
    InvalidTimeRange,
    /// A positive-presence construct row has no occurrences.
    #[error("source construct occurrence count must be positive")]
    ZeroConstructCount,
    /// Source clip rows do not form the canonical zero-based source prefix.
    #[error("source clip index {actual} is not the expected prefix index {expected}")]
    NonCanonicalClipIndex {
        /// Expected zero-based source index.
        expected: usize,
        /// Actual source index.
        actual: usize,
    },
    /// Nested channel rows do not form the canonical zero-based source prefix.
    #[error(
        "source channel index {actual} is not expected prefix index {expected} in clip {source_clip_index}"
    )]
    NonCanonicalChannelIndex {
        /// Source clip containing the invalid order.
        source_clip_index: usize,
        /// Expected zero-based channel index.
        expected: usize,
        /// Actual channel index.
        actual: usize,
    },
    /// Construct rows do not form the canonical deterministic prefix.
    #[error("source construct order {actual} is not expected prefix index {expected}")]
    NonCanonicalConstructOrder {
        /// Expected zero-based row order.
        expected: usize,
        /// Actual source order.
        actual: usize,
    },
    /// Resource rows do not form the canonical deterministic prefix.
    #[error("source resource order {actual} is not expected prefix index {expected}")]
    NonCanonicalResourceOrder {
        /// Expected zero-based row order.
        expected: usize,
        /// Actual source order.
        actual: usize,
    },
    /// A source clip maps outside the normalized document.
    #[error("normalized clip index {index} is outside document clip count {clip_count}")]
    NormalizedClipIndexOutOfRange {
        /// Invalid normalized clip index.
        index: usize,
        /// Number of normalized clips.
        clip_count: usize,
    },
    /// The dependency closure did not bind to these exact raw facts.
    #[error(transparent)]
    DependencyClosure(#[from] DependencyClosureError),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn format_provenance() -> SourceProvenanceV1 {
        SourceProvenanceV1::format_defined()
    }

    fn unavailable<T>() -> SourceObservationV1<T> {
        SourceObservationV1::unavailable(
            SourceUnavailableReasonV1::ParserUnavailable,
            None,
            SourceLoaderDispositionV1::Unknown,
        )
    }

    fn exact_observed<T>(value: T) -> crate::ExactSourceTimingObservationV1<T> {
        crate::ExactSourceTimingObservationV1::observed(
            value,
            format_provenance(),
            SourceLoaderDispositionV1::Preserved,
        )
    }

    fn clip(index: usize) -> SourceClipFactV1 {
        SourceClipFactV1::new(
            index,
            SourceObservationV1::proven_absent(format_provenance()),
            unavailable(),
            SourceObservationV1::proven_absent(format_provenance()),
            SourceObservationV1::proven_absent(format_provenance()),
            SourceFactSetV1::complete(Vec::new()),
        )
    }

    fn construct(index: usize, name: String) -> SourceConstructFactV1 {
        SourceConstructFactV1::new(
            index,
            SourceConstructKindV1::Extension,
            SourceTextV1::new(name).expect("bounded name"),
            false,
            1,
            SourceLoaderDispositionV1::Unsupported,
            format_provenance(),
        )
        .expect("positive construct count")
    }

    fn resource(index: usize) -> SourceResourceReferenceV1 {
        SourceResourceReferenceV1::new(
            index,
            SourceResourceKindV1::Image,
            index as u64,
            SourceResourceLocatorV1::Embedded,
            SourceLoaderDispositionV1::Preserved,
            format_provenance(),
        )
    }

    fn named_channel(index: usize, name: &str) -> SourceChannelFactV1 {
        SourceChannelFactV1::new(
            index,
            SourceTargetV1::new(SourceTargetKindV1::Node, 0),
            SourceChannelPropertyV1::Other,
            SourceComponentMaskV1::new(true, false, false),
            SourceObservationV1::proven_absent(format_provenance()),
            SourceLoaderDispositionV1::Unsupported,
            format_provenance(),
        )
        .with_property_name(SourceTextV1::new(name).expect("bounded property name"))
    }

    #[test]
    fn scalar_value_types_reject_non_finite_and_contradictory_values() {
        for value in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(
                SourceLinearUnitV1::new(value),
                Err(SourceFactsError::InvalidLinearUnit)
            );
            assert_eq!(
                SourceFramesPerSecondV1::new(value),
                Err(SourceFactsError::InvalidFramesPerSecond)
            );
        }
        assert_eq!(
            SourceTimeRangeV1::new(2.0, 1.0),
            Err(SourceFactsError::InvalidTimeRange)
        );
        assert_eq!(
            SourceTimeRangeV1::new(f64::NAN, 1.0),
            Err(SourceFactsError::InvalidTimeRange)
        );
        assert_eq!(
            SourceCoordinateBasisV1::new(
                SourceAxisV1::PositiveX,
                SourceAxisV1::NegativeX,
                SourceAxisV1::PositiveZ,
            ),
            Err(SourceFactsError::DuplicateBasisAxis)
        );

        let range = SourceTimeRangeV1::new(1.0, 1.0).expect("zero duration is evidence");
        assert_eq!(range.begin_s(), range.end_s());
        let basis = SourceCoordinateBasisV1::new(
            SourceAxisV1::PositiveX,
            SourceAxisV1::PositiveY,
            SourceAxisV1::PositiveZ,
        )
        .expect("orthogonal basis");
        assert_eq!(basis.handedness(), SourceHandednessV1::Right);
        assert_eq!(
            SourceConstructFactV1::new(
                0,
                SourceConstructKindV1::Extension,
                SourceTextV1::new("EXT_zero").expect("bounded name"),
                false,
                0,
                SourceLoaderDispositionV1::Unsupported,
                format_provenance(),
            ),
            Err(SourceFactsError::ZeroConstructCount)
        );
    }

    #[test]
    fn partial_empty_sets_never_prove_absence() {
        let complete = SourceFactSetV1::<SourceConstructFactV1>::complete(Vec::new());
        let partial = SourceFactSetV1::<SourceConstructFactV1>::partial(
            Vec::new(),
            SourceUnavailableReasonV1::ProjectionBudgetExceeded,
        );
        let unavailable = SourceFactSetV1::<SourceConstructFactV1>::unavailable(
            SourceUnavailableReasonV1::LoaderUnsupported,
        );
        assert!(complete.proves_absence());
        assert!(!partial.proves_absence());
        assert!(!unavailable.proves_absence());
    }

    #[test]
    fn builder_requires_explicit_complete_coverage_to_prove_absence() {
        let builder = RawSourceFactsBuilderV1::new(
            SourceFormatV1::GltfJson,
            InputIdentity::from_bytes(b"untouched"),
        );
        let loaded = builder
            .finish(Document::default())
            .expect("unavailable defaults");
        let facts = loaded.source_facts();
        assert!(!facts.clips().proves_absence());
        assert!(!facts.constructs().proves_absence());
        assert!(!facts.resources().proves_absence());
        assert_eq!(
            loaded.dependency_closure().coverage().reasons(),
            &[
                crate::DependencyClosureCoverageReasonV1::SourceDeclarationsUnavailable,
                crate::DependencyClosureCoverageReasonV1::CaptureUnavailable,
            ]
        );

        let mut builder = RawSourceFactsBuilderV1::new(
            SourceFormatV1::GltfJson,
            InputIdentity::from_bytes(b"exhaustive"),
        );
        builder.mark_complete(SourceFactDomainV1::Clips);
        builder.mark_complete(SourceFactDomainV1::Constructs);
        builder.mark_complete(SourceFactDomainV1::Resources);
        let loaded = builder
            .finish(Document::default())
            .expect("complete empty domains");
        let facts = loaded.source_facts();
        assert!(facts.clips().proves_absence());
        assert!(facts.constructs().proves_absence());
        assert!(facts.resources().proves_absence());
        assert_eq!(
            loaded.dependency_closure().coverage().reasons(),
            &[crate::DependencyClosureCoverageReasonV1::CaptureUnavailable]
        );
    }

    #[test]
    fn loaded_source_binds_raw_gltf_addressability_to_exact_primary_and_closure() {
        fn inventory(
            primary: InputIdentity,
            closure: DependencyClosureV1,
        ) -> RawGltfAddressabilityInventoryV1 {
            RawGltfAddressabilityInventoryV1::new(
                primary,
                closure,
                crate::RawGltfAddressabilityInventoryInputV1 {
                    default_scene: crate::RawGltfDefaultSceneObservationV1::Absent,
                    scene_coverage: crate::RawGltfAddressabilityCoverageV1::Complete,
                    scenes: Vec::new(),
                    node_coverage: crate::RawGltfAddressabilityCoverageV1::Complete,
                    nodes: Vec::new(),
                    skin_coverage: crate::RawGltfAddressabilityCoverageV1::Complete,
                    skins: Vec::new(),
                    attachment_coverage: crate::RawGltfAddressabilityCoverageV1::Complete,
                    attachments: Vec::new(),
                    path_candidate_coverage: crate::RawGltfAddressabilityCoverageV1::Complete,
                    path_candidates: Vec::new(),
                },
            )
            .unwrap()
        }

        let primary = InputIdentity::from_bytes(b"gltf-addressability");
        let source = RawSourceFactsBuilderV1::new(SourceFormatV1::GltfJson, primary.clone())
            .finish(Document::default())
            .unwrap();
        let exact = inventory(primary.clone(), source.dependency_closure().clone());
        let source = source
            .with_raw_gltf_addressability_inventory(exact)
            .expect("exact sidecar binds");
        assert!(source.raw_gltf_addressability_inventory().is_some());

        let wrong_primary = InputIdentity::from_bytes(b"other");
        let wrong = inventory(
            wrong_primary.clone(),
            DependencyClosureV1::unavailable(wrong_primary),
        );
        assert_eq!(
            RawSourceFactsBuilderV1::new(SourceFormatV1::GltfJson, primary.clone())
                .finish(Document::default())
                .unwrap()
                .with_raw_gltf_addressability_inventory(wrong)
                .unwrap_err(),
            RawGltfAddressabilityBindingErrorV1::PrimaryIdentityMismatch
        );

        let wrong_closure = DependencyClosureV1::unavailable(primary.clone());
        let wrong = inventory(primary.clone(), wrong_closure);
        assert_eq!(
            RawSourceFactsBuilderV1::new(SourceFormatV1::GltfJson, primary)
                .finish(Document::default())
                .unwrap()
                .with_raw_gltf_addressability_inventory(wrong)
                .unwrap_err(),
            RawGltfAddressabilityBindingErrorV1::DependencyClosureMismatch
        );
    }

    #[test]
    fn loaded_source_accepts_a_complete_format_bound_dependency_closure() {
        let primary = InputIdentity::from_bytes(b"gltf");
        let mut facts = RawSourceFactsBuilderV1::new(SourceFormatV1::GltfJson, primary.clone());
        assert!(facts.push_resource(SourceResourceReferenceV1::new(
            0,
            SourceResourceKindV1::Buffer,
            0,
            SourceResourceLocatorV1::classify("buffers/a%20b.bin"),
            SourceLoaderDispositionV1::Preserved,
            format_provenance(),
        )));
        facts.mark_complete(SourceFactDomainV1::Resources);

        let key = crate::DependencyResourceKeyV1::from_source_str(
            "buffers/a b.bin",
            crate::ResourceKeySyntaxV1::GltfUri,
        )
        .unwrap();
        let mut closure = crate::DependencyClosureBuilderV1::new(
            primary.clone(),
            facts.resource_coverage(),
            facts.resource_rows().len(),
        );
        assert!(closure.begin_reference(17, 2));
        assert_eq!(closure.prepare_external_key(&key).unwrap(), Some(true));
        closure.record_external_open_attempt(&key).unwrap();
        assert!(
            closure
                .push_captured_external(
                    0,
                    SourceResourceKindV1::Buffer,
                    0,
                    key,
                    InputIdentity::from_bytes(b"buffer"),
                )
                .unwrap()
        );
        let loaded = facts
            .finish_with_dependency_closure(Document::default(), closure.finish().unwrap())
            .unwrap();
        assert!(loaded.dependency_closure().coverage().is_complete());
        assert!(loaded.dependency_closure().identity().is_some());
        let document = loaded.into_document();
        assert!(document.clips.is_empty());
    }

    #[test]
    fn nested_partial_channels_do_not_weaken_complete_clip_identity_coverage() {
        let mut builder = RawSourceFactsBuilderV1::new(
            SourceFormatV1::Fbx,
            InputIdentity::from_bytes(b"nested-partial"),
        );
        assert!(builder.push_clip(SourceClipFactV1::new(
            0,
            SourceObservationV1::proven_absent(format_provenance()),
            unavailable(),
            SourceObservationV1::proven_absent(format_provenance()),
            SourceObservationV1::proven_absent(format_provenance()),
            SourceFactSetV1::partial(
                vec![named_channel(0, "property")],
                SourceUnavailableReasonV1::ParserUnavailable,
            ),
        )));
        builder.mark_complete(SourceFactDomainV1::Clips);
        let loaded = builder
            .finish(Document::default())
            .expect("independent coverage");
        let facts = loaded.source_facts();
        assert_eq!(facts.clips().coverage(), SourceSetCoverageV1::complete());
        assert_eq!(
            facts.clips().rows()[0].channels().coverage(),
            SourceSetCoverageV1::partial(SourceUnavailableReasonV1::ParserUnavailable)
        );
    }

    #[test]
    fn resource_locator_classification_redacts_unsafe_spelling() {
        let secret = "/home/example/private.bin";
        for classified in [
            SourceResourceLocatorV1::classify(secret),
            SourceResourceLocatorV1::classify("https://example.invalid/a.bin"),
            SourceResourceLocatorV1::classify("../escape.bin"),
            SourceResourceLocatorV1::classify("a/%2e%2e/escape.bin"),
            SourceResourceLocatorV1::classify("a/%2f/escape.bin"),
            SourceResourceLocatorV1::classify("bad%q0.bin"),
        ] {
            assert!(!format!("{classified:?}").contains(secret));
            assert!(!matches!(classified, SourceResourceLocatorV1::Relative(_)));
        }
        let control_bearing = "textures/TOP_SECRET\nname.png";
        let classified = SourceResourceLocatorV1::classify(control_bearing);
        assert_eq!(classified, SourceResourceLocatorV1::Malformed);
        assert!(!format!("{classified:?}").contains("TOP_SECRET"));
        assert_eq!(
            SourceResourceLocatorV1::retained_relative_bytes(control_bearing),
            0
        );
        let SourceResourceLocatorV1::Relative(relative) =
            SourceResourceLocatorV1::classify("textures/normal.png")
        else {
            panic!("safe relative declaration retained");
        };
        assert_eq!(relative.as_str(), "textures/normal.png");
        assert_eq!(
            SourceResourceLocatorV1::retained_relative_bytes("textures/normal.png"),
            "textures/normal.png".len()
        );
        assert_eq!(
            SourceResourceLocatorV1::retained_relative_bytes("../private.bin"),
            0
        );
        assert_eq!(
            SourceResourceLocatorV1::classify("data:image/png;base64,private"),
            SourceResourceLocatorV1::DataUri
        );
        assert_eq!(
            SourceResourceLocatorV1::classify("DATA:image/png;base64,private"),
            SourceResourceLocatorV1::DataUri
        );
        let oversized_data_uri = format!(
            "data:application/octet-stream;base64,{}",
            "A".repeat(RAW_SOURCE_V1_MAX_TEXT_BYTES)
        );
        assert_eq!(
            SourceResourceLocatorV1::classify(&oversized_data_uri),
            SourceResourceLocatorV1::DataUri
        );
        assert_eq!(
            SourceResourceLocatorV1::retained_relative_bytes(&oversized_data_uri),
            0
        );
        assert_eq!(
            SourceResourceLocatorV1::classify(r"C:\private\texture.png"),
            SourceResourceLocatorV1::Absolute
        );
        assert_eq!(
            SourceResourceLocatorV1::classify("file:///private/texture.png"),
            SourceResourceLocatorV1::Absolute
        );
    }

    #[test]
    fn provenance_uses_only_validated_source_logical_locators() {
        for value in [
            "/home/private/input.glb",
            "/../animations",
            "https://host/path",
        ] {
            assert_eq!(
                SourceLogicalLocatorV1::gltf_json_pointer(value),
                Err(SourceFactsError::InvalidLogicalLocator)
            );
        }
        for value in [
            "/home/private/input.fbx",
            "fbx:/home/private",
            "fbx:../private",
        ] {
            assert_eq!(
                SourceLogicalLocatorV1::fbx_parser_path(value),
                Err(SourceFactsError::InvalidLogicalLocator)
            );
        }

        let pointer = SourceLogicalLocatorV1::gltf_json_pointer("/animations/0/channels/1")
            .expect("generated glTF pointer");
        let provenance = SourceProvenanceV1::source_declared(pointer);
        assert_eq!(provenance.kind(), SourceProvenanceKindV1::SourceDeclared);
        assert_eq!(
            provenance.locator().map(SourceLogicalLocatorV1::as_str),
            Some("/animations/0/channels/1")
        );
        assert!(!format!("{provenance:?}").contains("animations"));

        let path = SourceLogicalLocatorV1::fbx_parser_path("fbx:scene.settings.axes")
            .expect("generated FBX parser path");
        assert_eq!(
            SourceProvenanceV1::parser_projected(path).kind(),
            SourceProvenanceKindV1::ParserProjected
        );
        assert!(SourceProvenanceV1::format_defined().locator().is_none());
    }

    #[test]
    fn loaded_source_binds_exact_identity_and_canonical_source_skeleton() {
        let bytes = b"same bytes parsed by the loader";
        let identity = InputIdentity::from_bytes(bytes);
        let mut document = Document::default();
        document.source.path = Some("/home/example/private/input.glb".into());
        let mut builder = RawSourceFactsBuilderV1::new(SourceFormatV1::Glb, identity.clone());
        builder.set_linear_unit(SourceObservationV1::observed(
            SourceLinearUnitV1::new(1.0).expect("metres"),
            format_provenance(),
            SourceLoaderDispositionV1::Preserved,
        ));
        let loaded = builder.finish(document).expect("facts bind");
        let source_skeleton_ptr = &loaded.document().assets.source_skeleton as *const _;
        let facts = loaded.source_facts();
        assert_eq!(facts.contract_id(), RAW_SOURCE_FACTS_V1_ID);
        assert_eq!(facts.format(), SourceFormatV1::Glb);
        assert_eq!(facts.primary_identity(), &identity);
        assert_eq!(facts.primary_identity().bytes(), bytes.len() as u64);
        assert!(loaded.exact_source_timing().is_none());
        assert!(std::ptr::eq(
            facts.source_skeleton() as *const _,
            source_skeleton_ptr
        ));
        assert_eq!(loaded.dependency_closure().primary_input(), &identity);
        assert!(matches!(
            loaded.dependency_closure().coverage(),
            crate::DependencyClosureCoverageV1::Unavailable { .. }
        ));
        assert!(loaded.dependency_closure().identity().is_none());
        assert!(!format!("{loaded:?}").contains("/home/example/private"));
        let document = loaded.into_document();
        assert!(document.clips.is_empty());
    }

    #[test]
    fn exact_source_timing_attachment_is_format_neutral_and_rejects_mismatched_clip_domains() {
        let loaded = || {
            let mut builder = RawSourceFactsBuilderV1::new(
                SourceFormatV1::Glb,
                InputIdentity::from_bytes(b"generic-exact-source-timing"),
            );
            assert!(builder.push_clip(clip(0)));
            builder.mark_complete(SourceFactDomainV1::Clips);
            builder.finish(Document::default()).expect("facts bind")
        };

        let exact = |coverage, clips: Vec<crate::ExactSourceClipTimingV1>| {
            ExactSourceTimingV1::new(
                exact_observed(crate::ExactSourceTimeBasisV1::new(1_000).unwrap()),
                exact_observed(crate::SourceTimelineModeV1::Fps24),
                exact_observed(crate::SourceTimelineModeV1::Fps24),
                crate::ExactSourceTimingObservationV1::proven_absent(format_provenance()),
                exact_observed(crate::ExactSourceFramePeriodV1::new(1).unwrap()),
                crate::ExactSourceTimingObservationV1::proven_absent(format_provenance()),
                exact_observed(crate::SourceTimeDisplayProtocolV1::Default),
                coverage,
                clips,
            )
            .unwrap()
        };
        let one_clip = || {
            vec![crate::ExactSourceClipTimingV1::new(
                0,
                exact_observed(
                    crate::ExactSourceClipTimeRangeV1::new(
                        crate::ExactSourceRangeSelectionV1::Primary,
                        0,
                        1,
                    )
                    .unwrap(),
                ),
            )]
        };

        let attached = loaded()
            .with_exact_source_timing(exact(SourceSetCoverageV1::complete(), one_clip()))
            .expect("generic exact timing attaches to a GLB source");
        assert!(attached.exact_source_timing().is_some());

        assert!(matches!(
            loaded().with_exact_source_timing(exact(SourceSetCoverageV1::complete(), Vec::new())),
            Err(ExactSourceTimingContractError::ClipCountMismatch {
                exact: 0,
                source_count: 1,
            })
        ));
        assert!(matches!(
            loaded().with_exact_source_timing(exact(
                SourceSetCoverageV1::partial(SourceUnavailableReasonV1::ParserUnavailable),
                one_clip(),
            )),
            Err(ExactSourceTimingContractError::ClipCoverageMismatch)
        ));
    }

    #[test]
    fn clip_limit_retains_n_then_marks_n_plus_one_partial() {
        let mut builder =
            RawSourceFactsBuilderV1::new(SourceFormatV1::Fbx, InputIdentity::from_bytes(b"fbx"));
        for index in 0..RAW_SOURCE_V1_MAX_CLIPS {
            assert!(builder.push_clip(clip(index)));
        }
        assert!(!builder.push_clip(clip(RAW_SOURCE_V1_MAX_CLIPS)));
        assert!(!builder.push_clip(clip(RAW_SOURCE_V1_MAX_CLIPS + 1)));
        let loaded = builder.finish(Document::default()).expect("bounded facts");
        let facts = loaded.source_facts();
        assert_eq!(facts.clips().rows().len(), RAW_SOURCE_V1_MAX_CLIPS);
        assert_eq!(
            facts.clips().coverage(),
            SourceSetCoverageV1::partial(SourceUnavailableReasonV1::ProjectionBudgetExceeded)
        );
        assert_eq!(facts.work().inspected_rows(), RAW_SOURCE_V1_MAX_CLIPS + 1);
    }

    #[test]
    fn resource_limit_retains_n_then_marks_n_plus_one_partial() {
        let mut builder = RawSourceFactsBuilderV1::new(
            SourceFormatV1::GltfJson,
            InputIdentity::from_bytes(b"gltf"),
        );
        for index in 0..RAW_SOURCE_V1_MAX_RESOURCE_REFERENCES {
            assert!(builder.push_resource(resource(index)));
        }
        assert!(!builder.push_resource(resource(RAW_SOURCE_V1_MAX_RESOURCE_REFERENCES)));
        assert!(!builder.push_resource(resource(RAW_SOURCE_V1_MAX_RESOURCE_REFERENCES + 1)));
        let loaded = builder.finish(Document::default()).expect("bounded facts");
        let facts = loaded.source_facts();
        assert_eq!(
            facts.resources().rows().len(),
            RAW_SOURCE_V1_MAX_RESOURCE_REFERENCES
        );
        assert_eq!(
            facts.resources().coverage().state(),
            SourceSetCoverageStateV1::Partial
        );
        assert_eq!(
            facts.work().inspected_rows(),
            RAW_SOURCE_V1_MAX_RESOURCE_REFERENCES + 1
        );
    }

    #[test]
    fn observed_clip_names_count_toward_the_aggregate_text_limit() {
        let rows_at_limit = RAW_SOURCE_V1_MAX_TOTAL_TEXT_BYTES / RAW_SOURCE_V1_MAX_TEXT_BYTES;
        let mut named_clips = RawSourceFactsBuilderV1::new(
            SourceFormatV1::GltfJson,
            InputIdentity::from_bytes(b"named-clips"),
        );
        for index in 0..rows_at_limit {
            let source_name = SourceObservationV1::observed(
                SourceTextV1::new("n".repeat(RAW_SOURCE_V1_MAX_TEXT_BYTES)).expect("bounded name"),
                format_provenance(),
                SourceLoaderDispositionV1::Preserved,
            );
            assert!(named_clips.push_clip(SourceClipFactV1::new(
                index,
                source_name,
                SourceObservationV1::proven_absent(format_provenance()),
                SourceObservationV1::proven_absent(format_provenance()),
                SourceObservationV1::proven_absent(format_provenance()),
                SourceFactSetV1::complete(Vec::new()),
            )));
        }
        let overflow_name = SourceObservationV1::observed(
            SourceTextV1::new("n".repeat(RAW_SOURCE_V1_MAX_TEXT_BYTES)).expect("bounded name"),
            format_provenance(),
            SourceLoaderDispositionV1::Preserved,
        );
        assert!(!named_clips.push_clip(SourceClipFactV1::new(
            rows_at_limit,
            overflow_name,
            SourceObservationV1::proven_absent(format_provenance()),
            SourceObservationV1::proven_absent(format_provenance()),
            SourceObservationV1::proven_absent(format_provenance()),
            SourceFactSetV1::complete(Vec::new()),
        )));
        let loaded = named_clips
            .finish(Document::default())
            .expect("bounded named clips");
        assert_eq!(
            loaded.source_facts().work().retained_text_bytes(),
            RAW_SOURCE_V1_MAX_TOTAL_TEXT_BYTES
        );
        assert_eq!(
            loaded.source_facts().clips().coverage().state(),
            SourceSetCoverageStateV1::Partial
        );
    }

    #[test]
    fn aggregate_text_limit_retains_nested_channel_prefix() {
        let mut builder = RawSourceFactsBuilderV1::new(
            SourceFormatV1::GltfJson,
            InputIdentity::from_bytes(b"channel-prefix"),
        );
        let full_rows = RAW_SOURCE_V1_MAX_TOTAL_TEXT_BYTES / RAW_SOURCE_V1_MAX_TEXT_BYTES;
        for _ in 0..full_rows - 1 {
            assert!(builder.push_construct(construct(
                builder.facts.constructs.rows.len(),
                "x".repeat(RAW_SOURCE_V1_MAX_TEXT_BYTES)
            )));
        }
        assert!(builder.push_construct(construct(
            builder.facts.constructs.rows.len(),
            "x".repeat(RAW_SOURCE_V1_MAX_TEXT_BYTES - 12)
        )));
        assert_eq!(builder.remaining_text_bytes(), 12);

        let channels = SourceFactSetV1::complete(vec![
            named_channel(0, "aaaa"),
            named_channel(1, "bbbb"),
            named_channel(2, "cccc"),
        ]);
        assert!(builder.push_clip(SourceClipFactV1::new(
            0,
            SourceObservationV1::observed(
                SourceTextV1::new("name").expect("bounded name"),
                format_provenance(),
                SourceLoaderDispositionV1::Preserved,
            ),
            SourceObservationV1::proven_absent(format_provenance()),
            SourceObservationV1::proven_absent(format_provenance()),
            SourceObservationV1::proven_absent(format_provenance()),
            channels,
        )));

        let loaded = builder.finish(Document::default()).expect("bounded prefix");
        let facts = loaded.source_facts();
        assert_eq!(
            facts.work().retained_text_bytes(),
            RAW_SOURCE_V1_MAX_TOTAL_TEXT_BYTES
        );
        assert_eq!(facts.clips().rows().len(), 1);
        assert_eq!(facts.clips().rows()[0].channels().rows().len(), 2);
        assert_eq!(
            facts.clips().rows()[0].channels().coverage().state(),
            SourceSetCoverageStateV1::Partial
        );
        assert_eq!(
            facts.clips().coverage().state(),
            SourceSetCoverageStateV1::Partial
        );
    }

    #[test]
    fn total_observation_limit_retains_exact_prefix_and_work_count() {
        let mut builder = RawSourceFactsBuilderV1::new(
            SourceFormatV1::GltfJson,
            InputIdentity::from_bytes(b"gltf"),
        );
        for index in 0..=RAW_SOURCE_V1_MAX_OBSERVATIONS {
            let retained = builder.push_construct(construct(index, format!("e{index}")));
            assert_eq!(retained, index < RAW_SOURCE_V1_MAX_OBSERVATIONS);
        }
        assert!(!builder.push_construct(construct(
            RAW_SOURCE_V1_MAX_OBSERVATIONS + 1,
            "must-not-resume".to_string()
        )));
        let loaded = builder.finish(Document::default()).expect("bounded facts");
        let facts = loaded.source_facts();
        assert_eq!(
            facts.constructs().rows().len(),
            RAW_SOURCE_V1_MAX_OBSERVATIONS
        );
        assert_eq!(
            facts.constructs().coverage(),
            SourceSetCoverageV1::partial(SourceUnavailableReasonV1::ProjectionBudgetExceeded)
        );
        assert_eq!(
            facts.work().inspected_rows(),
            RAW_SOURCE_V1_MAX_OBSERVATIONS + 1
        );
        assert_eq!(facts.work().retained_rows(), RAW_SOURCE_V1_MAX_OBSERVATIONS);
    }

    #[test]
    fn unavailable_domain_discards_prefix_and_updates_retained_work() {
        let mut builder = RawSourceFactsBuilderV1::new(
            SourceFormatV1::GltfJson,
            InputIdentity::from_bytes(b"discarded-prefix"),
        );
        assert!(builder.push_construct(construct(0, "retained-name".to_string())));
        assert_eq!(
            builder.remaining_observation_rows(),
            RAW_SOURCE_V1_MAX_OBSERVATIONS - 1
        );
        assert_eq!(
            builder.remaining_text_bytes(),
            RAW_SOURCE_V1_MAX_TOTAL_TEXT_BYTES - 13
        );

        builder.mark_unavailable(
            SourceFactDomainV1::Constructs,
            SourceUnavailableReasonV1::ParserUnavailable,
        );
        builder.mark_partial(
            SourceFactDomainV1::Constructs,
            SourceUnavailableReasonV1::ProjectionBudgetExceeded,
        );
        let loaded = builder
            .finish(Document::default())
            .expect("unavailable set remains valid");
        let facts = loaded.source_facts();
        assert!(facts.constructs().rows().is_empty());
        assert_eq!(
            facts.constructs().coverage(),
            SourceSetCoverageV1::unavailable(SourceUnavailableReasonV1::ParserUnavailable)
        );
        assert_eq!(facts.work().retained_rows(), 0);
        assert_eq!(facts.work().retained_text_bytes(), 0);
        assert_eq!(facts.work().inspected_rows(), 1);
    }

    #[test]
    fn preallocation_budget_stop_counts_terminal_row() {
        let mut builder = RawSourceFactsBuilderV1::new(
            SourceFormatV1::GltfJson,
            InputIdentity::from_bytes(b"preallocation-stop"),
        );
        assert!(builder.push_construct(construct(0, "first".to_string())));
        builder.mark_budget_exceeded(SourceFactDomainV1::Constructs);
        assert!(!builder.push_construct(construct(1, "must-not-resume".to_string())));
        let loaded = builder.finish(Document::default()).expect("partial prefix");
        let facts = loaded.source_facts();
        assert_eq!(facts.work().inspected_rows(), 2);
        assert_eq!(facts.work().retained_rows(), 1);
        assert_eq!(facts.constructs().rows()[0].name().as_str(), "first");
        assert_eq!(
            facts.constructs().coverage(),
            SourceSetCoverageV1::partial(SourceUnavailableReasonV1::ProjectionBudgetExceeded)
        );
    }

    #[test]
    fn text_and_traversal_limits_are_exact_and_coverage_qualified() {
        assert!(SourceTextV1::new("x".repeat(RAW_SOURCE_V1_MAX_TEXT_BYTES)).is_ok());
        assert_eq!(
            SourceTextV1::new("x".repeat(RAW_SOURCE_V1_MAX_TEXT_BYTES + 1)),
            Err(SourceFactsError::TextTooLong {
                bytes: RAW_SOURCE_V1_MAX_TEXT_BYTES + 1,
                limit: RAW_SOURCE_V1_MAX_TEXT_BYTES,
            })
        );

        let mut builder = RawSourceFactsBuilderV1::new(
            SourceFormatV1::GltfJson,
            InputIdentity::from_bytes(b"gltf"),
        );
        let rows_at_limit = RAW_SOURCE_V1_MAX_TOTAL_TEXT_BYTES / RAW_SOURCE_V1_MAX_TEXT_BYTES;
        for index in 0..rows_at_limit {
            assert!(
                builder.push_construct(construct(index, "x".repeat(RAW_SOURCE_V1_MAX_TEXT_BYTES)))
            );
        }
        assert!(!builder.push_construct(construct(
            rows_at_limit,
            "x".repeat(RAW_SOURCE_V1_MAX_TEXT_BYTES)
        )));
        assert!(!builder.push_construct(construct(rows_at_limit + 1, "short".to_string())));
        assert!(
            !builder.set_linear_unit(SourceObservationV1::observed(
                SourceLinearUnitV1::new(1.0).expect("metres"),
                SourceProvenanceV1::source_declared(
                    SourceLogicalLocatorV1::gltf_json_pointer("/animations")
                        .expect("generated logical locator"),
                ),
                SourceLoaderDispositionV1::Preserved,
            ))
        );
        assert!(builder.observe_traversal_depth(
            SourceFactDomainV1::Resources,
            RAW_SOURCE_V1_MAX_TRAVERSAL_DEPTH
        ));
        assert!(!builder.observe_traversal_depth(
            SourceFactDomainV1::Resources,
            RAW_SOURCE_V1_MAX_TRAVERSAL_DEPTH + 1
        ));
        assert!(!builder.push_resource(resource(0)));
        let loaded = builder.finish(Document::default()).expect("bounded facts");
        let facts = loaded.source_facts();
        assert_eq!(
            facts.work().retained_text_bytes(),
            RAW_SOURCE_V1_MAX_TOTAL_TEXT_BYTES
        );
        assert!(matches!(
            facts.linear_unit().state(),
            SourceObservationStateV1::Unavailable(
                SourceUnavailableReasonV1::ProjectionBudgetExceeded
            )
        ));
        assert_eq!(
            facts.work().max_traversal_depth(),
            RAW_SOURCE_V1_MAX_TRAVERSAL_DEPTH + 1
        );
        assert_eq!(
            facts.resources().coverage().state(),
            SourceSetCoverageStateV1::Partial
        );
    }

    #[test]
    fn finish_rejects_stale_normalized_clip_mappings_and_noncanonical_order() {
        let observed_index = |index| {
            SourceObservationV1::observed(
                index,
                format_provenance(),
                SourceLoaderDispositionV1::Preserved,
            )
        };
        let make = |source_index, normalized_index| {
            SourceClipFactV1::new(
                source_index,
                SourceObservationV1::proven_absent(format_provenance()),
                observed_index(normalized_index),
                SourceObservationV1::proven_absent(format_provenance()),
                SourceObservationV1::proven_absent(format_provenance()),
                SourceFactSetV1::complete(Vec::new()),
            )
        };

        let mut stale =
            RawSourceFactsBuilderV1::new(SourceFormatV1::Fbx, InputIdentity::from_bytes(b"fbx"));
        assert!(stale.push_clip(make(0, 0)));
        assert!(matches!(
            stale.finish(Document::default()),
            Err(SourceFactsError::NormalizedClipIndexOutOfRange { .. })
        ));

        let mut unordered =
            RawSourceFactsBuilderV1::new(SourceFormatV1::Fbx, InputIdentity::from_bytes(b"fbx"));
        assert!(unordered.push_clip(clip(1)));
        assert!(unordered.push_clip(clip(0)));
        assert!(matches!(
            unordered.finish(Document::default()),
            Err(SourceFactsError::NonCanonicalClipIndex { .. })
        ));

        let mut channel_gap =
            RawSourceFactsBuilderV1::new(SourceFormatV1::Fbx, InputIdentity::from_bytes(b"fbx"));
        assert!(channel_gap.push_clip(SourceClipFactV1::new(
            0,
            SourceObservationV1::proven_absent(format_provenance()),
            unavailable(),
            SourceObservationV1::proven_absent(format_provenance()),
            SourceObservationV1::proven_absent(format_provenance()),
            SourceFactSetV1::partial(
                vec![named_channel(1, "gap")],
                SourceUnavailableReasonV1::ParserUnavailable,
            ),
        )));
        assert!(matches!(
            channel_gap.finish(Document::default()),
            Err(SourceFactsError::NonCanonicalChannelIndex { .. })
        ));

        let mut construct_gap = RawSourceFactsBuilderV1::new(
            SourceFormatV1::GltfJson,
            InputIdentity::from_bytes(b"gltf"),
        );
        assert!(construct_gap.push_construct(construct(1, "gap".to_string())));
        assert!(matches!(
            construct_gap.finish(Document::default()),
            Err(SourceFactsError::NonCanonicalConstructOrder { .. })
        ));

        let mut resource_gap = RawSourceFactsBuilderV1::new(
            SourceFormatV1::GltfJson,
            InputIdentity::from_bytes(b"gltf"),
        );
        assert!(resource_gap.push_resource(resource(1)));
        assert!(matches!(
            resource_gap.finish(Document::default()),
            Err(SourceFactsError::NonCanonicalResourceOrder { .. })
        ));
    }
}
