//! Exact, bounded source-timing evidence retained beside normalized source facts.
//!
//! This contract is intentionally in-memory only. It does not widen the V1 raw-source
//! facts or prediction wire contracts, and it never treats floating-point seconds or
//! decimal frame rates as exact frame coordinates.

use crate::{
    RAW_SOURCE_V1_MAX_CLIPS, SourceLoaderDispositionV1, SourceProvenanceV1, SourceSetCoverageV1,
};

/// Semantic identity of the exact in-memory source-timing vocabulary.
pub const EXACT_SOURCE_TIMING_V1_ID: &str = "urn:animsmith:exact-source-timing:1";

/// Maximum retained source-clip timing rows.
pub const EXACT_SOURCE_TIMING_V1_MAX_CLIPS: usize = RAW_SOURCE_V1_MAX_CLIPS;

/// Why one exact source timing observation cannot be established.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExactSourceTimingUnavailableReasonV1 {
    /// A source property or resolved span is structurally invalid.
    Malformed,
    /// The loader exposes a custom frame rate only through floating-point data.
    CustomFrameRateNotExact,
    /// The source timeline mode has no exact period rule in the loader projection.
    UnsupportedTimeMode,
    /// The source time basis cannot exactly represent the requested frame mode.
    UnsupportedTimeBasis,
    /// The parser did not make the required exact evidence available.
    ParserUnavailable,
}

/// Availability of one exact source timing observation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExactSourceTimingObservationStateV1<T> {
    /// The exact value was retained.
    Observed(T),
    /// Complete parser evidence proves that no declaration exists.
    ProvenAbsent,
    /// Exact evidence cannot be established.
    Unavailable(ExactSourceTimingUnavailableReasonV1),
}

/// One exact source timing value with orthogonal provenance and loader treatment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactSourceTimingObservationV1<T> {
    state: ExactSourceTimingObservationStateV1<T>,
    disposition: SourceLoaderDispositionV1,
    provenance: Option<SourceProvenanceV1>,
}

impl<T> ExactSourceTimingObservationV1<T> {
    /// Retain an exact observed value.
    pub fn observed(
        value: T,
        provenance: SourceProvenanceV1,
        disposition: SourceLoaderDispositionV1,
    ) -> Self {
        Self {
            state: ExactSourceTimingObservationStateV1::Observed(value),
            disposition,
            provenance: Some(provenance),
        }
    }

    /// Record proven source absence.
    pub fn proven_absent(provenance: SourceProvenanceV1) -> Self {
        Self {
            state: ExactSourceTimingObservationStateV1::ProvenAbsent,
            disposition: SourceLoaderDispositionV1::NotApplicable,
            provenance: Some(provenance),
        }
    }

    /// Record a typed exact-evidence failure.
    pub fn unavailable(
        reason: ExactSourceTimingUnavailableReasonV1,
        provenance: Option<SourceProvenanceV1>,
        disposition: SourceLoaderDispositionV1,
    ) -> Self {
        Self {
            state: ExactSourceTimingObservationStateV1::Unavailable(reason),
            disposition,
            provenance,
        }
    }

    /// Availability and exact value state.
    pub const fn state(&self) -> &ExactSourceTimingObservationStateV1<T> {
        &self.state
    }

    /// Loader treatment of this source domain.
    pub const fn disposition(&self) -> SourceLoaderDispositionV1 {
        self.disposition
    }

    /// Source or parser provenance, when retained.
    pub const fn provenance(&self) -> Option<&SourceProvenanceV1> {
        self.provenance.as_ref()
    }
}

/// Positive exact source-time units in one second.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExactSourceTimeBasisV1(i64);

impl ExactSourceTimeBasisV1 {
    /// Validate an exact positive source-time basis.
    pub fn new(units_per_second: i64) -> Result<Self, ExactSourceTimingContractError> {
        if units_per_second <= 0 {
            return Err(ExactSourceTimingContractError::InvalidTimeBasis);
        }
        Ok(Self(units_per_second))
    }

    /// Exact source-time units in one second.
    pub const fn units_per_second(self) -> i64 {
        self.0
    }
}

/// Source timeline mode retained without converting it to decimal FPS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SourceTimelineModeV1 {
    /// Explicit source default mode, physically 30 fps.
    Default,
    /// 120 fps.
    Fps120,
    /// 100 fps.
    Fps100,
    /// 60 fps.
    Fps60,
    /// 50 fps.
    Fps50,
    /// 48 fps.
    Fps48,
    /// 30 fps.
    Fps30,
    /// 30 fps with drop-style display semantics.
    Fps30Drop,
    /// NTSC approximately 29.97 fps with drop-frame numbering.
    NtscDropFrame,
    /// NTSC approximately 29.97 fps with full-frame numbering.
    NtscFullFrame,
    /// PAL 25 fps.
    Pal,
    /// 24 fps.
    Fps24,
    /// 1000 fps.
    Fps1000,
    /// Film approximately 23.976 fps.
    FilmFullFrame,
    /// Source-defined custom frame rate.
    Custom,
    /// 96 fps.
    Fps96,
    /// 72 fps.
    Fps72,
    /// NTSC approximately 59.94 fps.
    Fps59Dot94,
}

/// Source timecode display protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SourceTimeDisplayProtocolV1 {
    /// SMPTE timecode display.
    Smpte,
    /// Frame-count display.
    FrameCount,
    /// Parser-resolved default protocol marker.
    Default,
}

/// Exact binary64 payload exposed by a loader for a finite positive custom frame rate.
///
/// This preserves the parser projection for evidence and diagnostics. It is not an
/// exact rational rate and cannot authorize an integer frame lattice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ParserFrameRateProjectionV1(u64);

impl ParserFrameRateProjectionV1 {
    /// Retain the exact finite positive binary64 payload.
    pub fn new(value: f64) -> Result<Self, ExactSourceTimingContractError> {
        if !value.is_finite() || value <= 0.0 {
            return Err(ExactSourceTimingContractError::InvalidFrameRateProjection);
        }
        Ok(Self(value.to_bits()))
    }

    /// Exact IEEE-754 binary64 bit pattern exposed by the parser.
    pub const fn binary64_bits(self) -> u64 {
        self.0
    }

    /// Parser-projected floating-point value for display or diagnostics only.
    pub const fn parser_value(self) -> f64 {
        f64::from_bits(self.0)
    }
}

/// Positive exact source-time units in one physical source frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExactSourceFramePeriodV1(i64);

impl ExactSourceFramePeriodV1 {
    /// Validate a positive exact frame period.
    pub fn new(units_per_frame: i64) -> Result<Self, ExactSourceTimingContractError> {
        if units_per_frame <= 0 {
            return Err(ExactSourceTimingContractError::InvalidFramePeriod);
        }
        Ok(Self(units_per_frame))
    }

    /// Exact source-time units in one physical source frame.
    pub const fn units_per_frame(self) -> i64 {
        self.0
    }

    /// Whether an absolute signed source-time coordinate lies on the frame lattice.
    pub fn is_whole_frame(self, coordinate_units: i64) -> bool {
        coordinate_units.rem_euclid(self.0) == 0
    }

    /// Exact signed frame index when the coordinate lies on the frame lattice.
    pub fn frame_index(self, coordinate_units: i64) -> Option<i64> {
        self.is_whole_frame(coordinate_units)
            .then_some(coordinate_units / self.0)
    }
}

/// Parser-selected source time-span property pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExactSourceRangeSelectionV1 {
    /// The loader's preferred complete pair.
    Primary,
    /// The loader's complete fallback pair.
    Fallback,
}

/// Exact signed begin/end source-time coordinates for one animation clip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExactSourceClipTimeRangeV1 {
    selection: ExactSourceRangeSelectionV1,
    begin_units: i64,
    end_units: i64,
}

impl ExactSourceClipTimeRangeV1 {
    /// Validate a parser-selected, ordered exact clip range.
    pub fn new(
        selection: ExactSourceRangeSelectionV1,
        begin_units: i64,
        end_units: i64,
    ) -> Result<Self, ExactSourceTimingContractError> {
        if begin_units > end_units {
            return Err(ExactSourceTimingContractError::ReversedClipRange);
        }
        Ok(Self {
            selection,
            begin_units,
            end_units,
        })
    }

    /// Property pair selected by parser semantics.
    pub const fn selection(self) -> ExactSourceRangeSelectionV1 {
        self.selection
    }

    /// Exact signed source begin coordinate.
    pub const fn begin_units(self) -> i64 {
        self.begin_units
    }

    /// Exact signed source end coordinate.
    pub const fn end_units(self) -> i64 {
        self.end_units
    }
}

/// Exact timing evidence for one retained source animation clip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactSourceClipTimingV1 {
    source_clip_index: usize,
    source_time_range: ExactSourceTimingObservationV1<ExactSourceClipTimeRangeV1>,
}

impl ExactSourceClipTimingV1 {
    /// Construct one source-indexed exact clip row.
    pub fn new(
        source_clip_index: usize,
        source_time_range: ExactSourceTimingObservationV1<ExactSourceClipTimeRangeV1>,
    ) -> Self {
        Self {
            source_clip_index,
            source_time_range,
        }
    }

    /// Stable zero-based source clip index.
    pub const fn source_clip_index(&self) -> usize {
        self.source_clip_index
    }

    /// Exact parser-selected source time range.
    pub const fn source_time_range(
        &self,
    ) -> &ExactSourceTimingObservationV1<ExactSourceClipTimeRangeV1> {
        &self.source_time_range
    }
}

/// Bounded exact source timing evidence from one successful loader parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactSourceTimingV1 {
    time_basis: ExactSourceTimingObservationV1<ExactSourceTimeBasisV1>,
    declared_time_mode: ExactSourceTimingObservationV1<SourceTimelineModeV1>,
    effective_time_mode: ExactSourceTimingObservationV1<SourceTimelineModeV1>,
    declared_custom_frame_rate: ExactSourceTimingObservationV1<ParserFrameRateProjectionV1>,
    frame_period: ExactSourceTimingObservationV1<ExactSourceFramePeriodV1>,
    declared_time_protocol: ExactSourceTimingObservationV1<SourceTimeDisplayProtocolV1>,
    effective_time_protocol: ExactSourceTimingObservationV1<SourceTimeDisplayProtocolV1>,
    clip_coverage: SourceSetCoverageV1,
    clips: Vec<ExactSourceClipTimingV1>,
}

impl ExactSourceTimingV1 {
    /// Construct and validate one bounded exact timing projection.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        time_basis: ExactSourceTimingObservationV1<ExactSourceTimeBasisV1>,
        declared_time_mode: ExactSourceTimingObservationV1<SourceTimelineModeV1>,
        effective_time_mode: ExactSourceTimingObservationV1<SourceTimelineModeV1>,
        declared_custom_frame_rate: ExactSourceTimingObservationV1<ParserFrameRateProjectionV1>,
        frame_period: ExactSourceTimingObservationV1<ExactSourceFramePeriodV1>,
        declared_time_protocol: ExactSourceTimingObservationV1<SourceTimeDisplayProtocolV1>,
        effective_time_protocol: ExactSourceTimingObservationV1<SourceTimeDisplayProtocolV1>,
        clip_coverage: SourceSetCoverageV1,
        clips: Vec<ExactSourceClipTimingV1>,
    ) -> Result<Self, ExactSourceTimingContractError> {
        if clips.len() > EXACT_SOURCE_TIMING_V1_MAX_CLIPS {
            return Err(ExactSourceTimingContractError::TooManyClips {
                count: clips.len(),
                limit: EXACT_SOURCE_TIMING_V1_MAX_CLIPS,
            });
        }
        for (expected, clip) in clips.iter().enumerate() {
            if clip.source_clip_index != expected {
                return Err(ExactSourceTimingContractError::NonCanonicalClipIndex {
                    expected,
                    actual: clip.source_clip_index,
                });
            }
        }
        Ok(Self {
            time_basis,
            declared_time_mode,
            effective_time_mode,
            declared_custom_frame_rate,
            frame_period,
            declared_time_protocol,
            effective_time_protocol,
            clip_coverage,
            clips,
        })
    }

    /// Semantic identity of this in-memory contract.
    pub const fn contract_id(&self) -> &'static str {
        EXACT_SOURCE_TIMING_V1_ID
    }

    /// Exact parser-resolved source-time basis.
    pub const fn time_basis(&self) -> &ExactSourceTimingObservationV1<ExactSourceTimeBasisV1> {
        &self.time_basis
    }

    /// Raw source declaration state for the timeline mode.
    pub const fn declared_time_mode(
        &self,
    ) -> &ExactSourceTimingObservationV1<SourceTimelineModeV1> {
        &self.declared_time_mode
    }

    /// Loader-effective time mode, including parser fallback.
    pub const fn effective_time_mode(
        &self,
    ) -> &ExactSourceTimingObservationV1<SourceTimelineModeV1> {
        &self.effective_time_mode
    }

    /// Raw direct-property state for parser-projected `CustomFrameRate` binary64 evidence.
    pub const fn declared_custom_frame_rate(
        &self,
    ) -> &ExactSourceTimingObservationV1<ParserFrameRateProjectionV1> {
        &self.declared_custom_frame_rate
    }

    /// Exact integer physical frame period, when supported.
    pub const fn frame_period(&self) -> &ExactSourceTimingObservationV1<ExactSourceFramePeriodV1> {
        &self.frame_period
    }

    /// Raw source declaration state for the time-display protocol.
    pub const fn declared_time_protocol(
        &self,
    ) -> &ExactSourceTimingObservationV1<SourceTimeDisplayProtocolV1> {
        &self.declared_time_protocol
    }

    /// Loader-effective time protocol, including parser fallback.
    pub const fn effective_time_protocol(
        &self,
    ) -> &ExactSourceTimingObservationV1<SourceTimeDisplayProtocolV1> {
        &self.effective_time_protocol
    }

    /// Coverage of the independently enumerable source-clip domain.
    pub const fn clip_coverage(&self) -> SourceSetCoverageV1 {
        self.clip_coverage
    }

    /// Retained deterministic source-clip-prefix rows.
    pub fn clips(&self) -> &[ExactSourceClipTimingV1] {
        &self.clips
    }
}

/// Invalid exact source timing value or attachment invariant.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum ExactSourceTimingContractError {
    /// Source-time units per second must be positive.
    #[error("source-time units per second must be positive")]
    InvalidTimeBasis,
    /// Source-time units per frame must be positive.
    #[error("source-time units per frame must be positive")]
    InvalidFramePeriod,
    /// A retained frame-rate projection must be finite and positive.
    #[error("parser frame-rate projection must be finite and positive")]
    InvalidFrameRateProjection,
    /// An observed clip range is reversed.
    #[error("exact source clip range must satisfy begin_units <= end_units")]
    ReversedClipRange,
    /// The bounded clip limit was exceeded.
    #[error("exact source timing has {count} clips, exceeding the limit of {limit}")]
    TooManyClips {
        /// Retained row count.
        count: usize,
        /// Public contract limit.
        limit: usize,
    },
    /// Clip rows do not form a canonical zero-based prefix.
    #[error("exact source clip index {actual} is not expected prefix index {expected}")]
    NonCanonicalClipIndex {
        /// Expected zero-based index.
        expected: usize,
        /// Actual retained index.
        actual: usize,
    },
    /// Exact and raw clip prefixes have different lengths.
    #[error(
        "exact source clip count {exact} does not match retained raw clip count {source_count}"
    )]
    ClipCountMismatch {
        /// Exact timing clip rows.
        exact: usize,
        /// Existing V1 source clip rows.
        source_count: usize,
    },
    /// Exact and raw clip domains have different coverage.
    #[error("exact source clip coverage does not match retained raw clip coverage")]
    ClipCoverageMismatch,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_frame_predicate_is_signed_and_unit_exact() {
        let period = ExactSourceFramePeriodV1::new(100).unwrap();
        for coordinate in [-300, 0, 700] {
            assert!(period.is_whole_frame(coordinate));
        }
        for coordinate in [-301, -299, 699, 701] {
            assert!(!period.is_whole_frame(coordinate));
        }
        assert_eq!(period.frame_index(-300), Some(-3));
        assert_eq!(period.frame_index(701), None);
    }

    #[test]
    fn long_timeline_remains_unit_exact_beyond_binary64_integer_precision() {
        let period = ExactSourceFramePeriodV1::new(4_708_704).unwrap();
        let whole = period.units_per_frame() * 3_000_000_000;
        assert!(whole > (1i64 << 53));
        assert!(period.is_whole_frame(whole));
        assert!(!period.is_whole_frame(whole - 1));
        assert!(!period.is_whole_frame(whole + 1));
        assert_eq!(period.frame_index(whole), Some(3_000_000_000));

        assert_eq!(whole as f64, (whole - 1) as f64);
        assert_eq!(whole as f64, (whole + 1) as f64);
    }
}
