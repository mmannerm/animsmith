//! Exact, bounded FBX/KTime timing evidence retained beside normalized source facts.
//!
//! This contract is intentionally in-memory only. It does not widen the V1 raw-source
//! facts or prediction wire contracts, and it never treats floating-point seconds or
//! decimal frame rates as exact frame coordinates.

use crate::{
    RAW_SOURCE_V1_MAX_CLIPS, SourceLoaderDispositionV1, SourceProvenanceV1, SourceSetCoverageV1,
};

/// Semantic identity of the exact in-memory FBX timing vocabulary.
pub const EXACT_FBX_TIMING_V1_ID: &str = "urn:animsmith:exact-fbx-timing:1";

/// Legacy FBX/KTime units in one second.
pub const FBX_KTIME_LEGACY_TICKS_PER_SECOND: i64 = 46_186_158_000;

/// Standard FBX/KTime units in one second.
pub const FBX_KTIME_STANDARD_TICKS_PER_SECOND: i64 = 141_120_000;

/// Maximum retained animation-stack timing rows.
pub const EXACT_FBX_TIMING_V1_MAX_STACKS: usize = RAW_SOURCE_V1_MAX_CLIPS;

/// Why one exact FBX timing observation cannot be established.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExactFbxTimingUnavailableReasonV1 {
    /// A source property or resolved span is structurally invalid.
    Malformed,
    /// ufbx exposes a custom frame rate only through floating-point data.
    CustomFrameRateNotExact,
    /// The time mode has no frozen exact period rule in this contract.
    UnsupportedTimeMode,
    /// The KTime basis cannot exactly represent the requested standard frame mode.
    UnsupportedKTimeBasis,
    /// The parser did not make the required exact evidence available.
    ParserUnavailable,
}

/// Availability of one exact FBX timing observation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExactFbxTimingObservationStateV1<T> {
    /// The exact value was retained.
    Observed(T),
    /// Complete parser evidence proves that no declaration exists.
    ProvenAbsent,
    /// Exact evidence cannot be established.
    Unavailable(ExactFbxTimingUnavailableReasonV1),
}

/// One exact FBX timing value with orthogonal provenance and loader treatment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactFbxTimingObservationV1<T> {
    state: ExactFbxTimingObservationStateV1<T>,
    disposition: SourceLoaderDispositionV1,
    provenance: Option<SourceProvenanceV1>,
}

impl<T> ExactFbxTimingObservationV1<T> {
    /// Retain an exact observed value.
    pub fn observed(
        value: T,
        provenance: SourceProvenanceV1,
        disposition: SourceLoaderDispositionV1,
    ) -> Self {
        Self {
            state: ExactFbxTimingObservationStateV1::Observed(value),
            disposition,
            provenance: Some(provenance),
        }
    }

    /// Record proven source absence.
    pub fn proven_absent(provenance: SourceProvenanceV1) -> Self {
        Self {
            state: ExactFbxTimingObservationStateV1::ProvenAbsent,
            disposition: SourceLoaderDispositionV1::NotApplicable,
            provenance: Some(provenance),
        }
    }

    /// Record a typed exact-evidence failure.
    pub fn unavailable(
        reason: ExactFbxTimingUnavailableReasonV1,
        provenance: Option<SourceProvenanceV1>,
        disposition: SourceLoaderDispositionV1,
    ) -> Self {
        Self {
            state: ExactFbxTimingObservationStateV1::Unavailable(reason),
            disposition,
            provenance,
        }
    }

    /// Availability and exact value state.
    pub const fn state(&self) -> &ExactFbxTimingObservationStateV1<T> {
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

/// Positive exact FBX/KTime units in one second.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FbxKTimeBasisV1(i64);

impl FbxKTimeBasisV1 {
    /// Validate an exact positive KTime basis.
    pub fn new(ticks_per_second: i64) -> Result<Self, ExactFbxTimingContractError> {
        if ticks_per_second <= 0 {
            return Err(ExactFbxTimingContractError::InvalidKTimeBasis);
        }
        Ok(Self(ticks_per_second))
    }

    /// Exact KTime ticks in one second.
    pub const fn ticks_per_second(self) -> i64 {
        self.0
    }
}

/// FBX time mode retained without converting it to decimal FPS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FbxTimeModeV1 {
    /// Explicit FBX default mode, physically 30 fps.
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

/// FBX timecode display protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FbxTimeProtocolV1 {
    /// SMPTE timecode display.
    Smpte,
    /// Frame-count display.
    FrameCount,
    /// Parser-resolved default protocol marker.
    Default,
}

/// Exact binary64 payload exposed by ufbx for a finite positive custom frame rate.
///
/// This preserves the parser projection for evidence and diagnostics. It is not an
/// exact rational rate and cannot authorize an integer frame lattice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FbxCustomFrameRateV1(u64);

impl FbxCustomFrameRateV1 {
    /// Retain the exact finite positive binary64 payload.
    pub fn new(value: f64) -> Result<Self, ExactFbxTimingContractError> {
        if !value.is_finite() || value <= 0.0 {
            return Err(ExactFbxTimingContractError::InvalidCustomFrameRate);
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

/// Positive exact KTime ticks in one physical source frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FbxFramePeriodV1(i64);

impl FbxFramePeriodV1 {
    /// Validate a positive exact frame period.
    pub fn new(ticks_per_frame: i64) -> Result<Self, ExactFbxTimingContractError> {
        if ticks_per_frame <= 0 {
            return Err(ExactFbxTimingContractError::InvalidFramePeriod);
        }
        Ok(Self(ticks_per_frame))
    }

    /// Derive the SDK-compatible integer KTime period for a standard FBX mode.
    pub fn for_mode(
        basis: FbxKTimeBasisV1,
        mode: FbxTimeModeV1,
    ) -> Result<Self, ExactFbxTimingUnavailableReasonV1> {
        let ticks = basis.ticks_per_second();
        if !matches!(
            ticks,
            FBX_KTIME_LEGACY_TICKS_PER_SECOND | FBX_KTIME_STANDARD_TICKS_PER_SECOND
        ) {
            return Err(ExactFbxTimingUnavailableReasonV1::UnsupportedKTimeBasis);
        }

        let integer_rate = match mode {
            FbxTimeModeV1::Default | FbxTimeModeV1::Fps30 | FbxTimeModeV1::Fps30Drop => Some(30),
            FbxTimeModeV1::Fps120 => Some(120),
            FbxTimeModeV1::Fps100 => Some(100),
            FbxTimeModeV1::Fps60 => Some(60),
            FbxTimeModeV1::Fps50 => Some(50),
            FbxTimeModeV1::Fps48 => Some(48),
            FbxTimeModeV1::Pal => Some(25),
            FbxTimeModeV1::Fps24 => Some(24),
            FbxTimeModeV1::Fps1000 => Some(1000),
            FbxTimeModeV1::Fps96 => Some(96),
            FbxTimeModeV1::Fps72 => Some(72),
            FbxTimeModeV1::NtscDropFrame
            | FbxTimeModeV1::NtscFullFrame
            | FbxTimeModeV1::FilmFullFrame
            | FbxTimeModeV1::Fps59Dot94
            | FbxTimeModeV1::Custom => None,
        };
        if let Some(rate) = integer_rate {
            if ticks % rate != 0 {
                return Err(ExactFbxTimingUnavailableReasonV1::UnsupportedKTimeBasis);
            }
            return Ok(Self(ticks / rate));
        }

        let ticks = i128::from(ticks);
        let period = match mode {
            FbxTimeModeV1::NtscDropFrame | FbxTimeModeV1::NtscFullFrame => {
                (ticks / 30 * 1001) / 1000
            }
            FbxTimeModeV1::FilmFullFrame => (ticks / 24 * 1001) / 1000,
            FbxTimeModeV1::Fps59Dot94 => ((ticks / 30 * 1001) / 1000) / 2,
            FbxTimeModeV1::Custom => {
                return Err(ExactFbxTimingUnavailableReasonV1::CustomFrameRateNotExact);
            }
            _ => return Err(ExactFbxTimingUnavailableReasonV1::UnsupportedTimeMode),
        };
        let period = i64::try_from(period)
            .map_err(|_| ExactFbxTimingUnavailableReasonV1::UnsupportedKTimeBasis)?;
        if period <= 0 {
            return Err(ExactFbxTimingUnavailableReasonV1::UnsupportedKTimeBasis);
        }
        Ok(Self(period))
    }

    /// Exact KTime ticks in one physical source frame.
    pub const fn ticks_per_frame(self) -> i64 {
        self.0
    }

    /// Whether an absolute signed KTime coordinate lies on the frame lattice.
    pub fn is_whole_frame(self, coordinate_ticks: i64) -> bool {
        coordinate_ticks.rem_euclid(self.0) == 0
    }

    /// Exact signed frame index when the coordinate lies on the frame lattice.
    pub fn frame_index(self, coordinate_ticks: i64) -> Option<i64> {
        self.is_whole_frame(coordinate_ticks)
            .then_some(coordinate_ticks / self.0)
    }
}

/// Parser-selected animation-stack time-span property pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FbxTimeSpanSelectionV1 {
    /// Complete `LocalStart` and `LocalStop` pair.
    Local,
    /// Complete `ReferenceStart` and `ReferenceStop` fallback pair.
    Reference,
}

/// Exact signed begin/end KTime coordinates for one animation stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FbxStackTickRangeV1 {
    selection: FbxTimeSpanSelectionV1,
    begin_ticks: i64,
    end_ticks: i64,
}

impl FbxStackTickRangeV1 {
    /// Validate a parser-selected, ordered exact stack range.
    pub fn new(
        selection: FbxTimeSpanSelectionV1,
        begin_ticks: i64,
        end_ticks: i64,
    ) -> Result<Self, ExactFbxTimingContractError> {
        if begin_ticks > end_ticks {
            return Err(ExactFbxTimingContractError::ReversedStackRange);
        }
        Ok(Self {
            selection,
            begin_ticks,
            end_ticks,
        })
    }

    /// Property pair selected by parser semantics.
    pub const fn selection(self) -> FbxTimeSpanSelectionV1 {
        self.selection
    }

    /// Exact signed source begin coordinate.
    pub const fn begin_ticks(self) -> i64 {
        self.begin_ticks
    }

    /// Exact signed source end coordinate.
    pub const fn end_ticks(self) -> i64 {
        self.end_ticks
    }
}

/// Exact timing evidence for one retained source animation stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactFbxStackTimingV1 {
    source_stack_index: usize,
    source_tick_range: ExactFbxTimingObservationV1<FbxStackTickRangeV1>,
}

impl ExactFbxStackTimingV1 {
    /// Construct one source-indexed exact stack row.
    pub fn new(
        source_stack_index: usize,
        source_tick_range: ExactFbxTimingObservationV1<FbxStackTickRangeV1>,
    ) -> Self {
        Self {
            source_stack_index,
            source_tick_range,
        }
    }

    /// Stable zero-based source stack index.
    pub const fn source_stack_index(&self) -> usize {
        self.source_stack_index
    }

    /// Exact parser-selected source tick range.
    pub const fn source_tick_range(&self) -> &ExactFbxTimingObservationV1<FbxStackTickRangeV1> {
        &self.source_tick_range
    }
}

/// Bounded exact FBX timing evidence from one successful ufbx parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactFbxTimingV1 {
    ktime_basis: ExactFbxTimingObservationV1<FbxKTimeBasisV1>,
    declared_time_mode: ExactFbxTimingObservationV1<FbxTimeModeV1>,
    effective_time_mode: ExactFbxTimingObservationV1<FbxTimeModeV1>,
    declared_custom_frame_rate: ExactFbxTimingObservationV1<FbxCustomFrameRateV1>,
    frame_period: ExactFbxTimingObservationV1<FbxFramePeriodV1>,
    declared_time_protocol: ExactFbxTimingObservationV1<FbxTimeProtocolV1>,
    effective_time_protocol: ExactFbxTimingObservationV1<FbxTimeProtocolV1>,
    stack_coverage: SourceSetCoverageV1,
    stacks: Vec<ExactFbxStackTimingV1>,
}

impl ExactFbxTimingV1 {
    /// Construct and validate one bounded exact timing projection.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        ktime_basis: ExactFbxTimingObservationV1<FbxKTimeBasisV1>,
        declared_time_mode: ExactFbxTimingObservationV1<FbxTimeModeV1>,
        effective_time_mode: ExactFbxTimingObservationV1<FbxTimeModeV1>,
        declared_custom_frame_rate: ExactFbxTimingObservationV1<FbxCustomFrameRateV1>,
        frame_period: ExactFbxTimingObservationV1<FbxFramePeriodV1>,
        declared_time_protocol: ExactFbxTimingObservationV1<FbxTimeProtocolV1>,
        effective_time_protocol: ExactFbxTimingObservationV1<FbxTimeProtocolV1>,
        stack_coverage: SourceSetCoverageV1,
        stacks: Vec<ExactFbxStackTimingV1>,
    ) -> Result<Self, ExactFbxTimingContractError> {
        if stacks.len() > EXACT_FBX_TIMING_V1_MAX_STACKS {
            return Err(ExactFbxTimingContractError::TooManyStacks {
                count: stacks.len(),
                limit: EXACT_FBX_TIMING_V1_MAX_STACKS,
            });
        }
        for (expected, stack) in stacks.iter().enumerate() {
            if stack.source_stack_index != expected {
                return Err(ExactFbxTimingContractError::NonCanonicalStackIndex {
                    expected,
                    actual: stack.source_stack_index,
                });
            }
        }
        Ok(Self {
            ktime_basis,
            declared_time_mode,
            effective_time_mode,
            declared_custom_frame_rate,
            frame_period,
            declared_time_protocol,
            effective_time_protocol,
            stack_coverage,
            stacks,
        })
    }

    /// Semantic identity of this in-memory contract.
    pub const fn contract_id(&self) -> &'static str {
        EXACT_FBX_TIMING_V1_ID
    }

    /// Exact parser-resolved KTime basis.
    pub const fn ktime_basis(&self) -> &ExactFbxTimingObservationV1<FbxKTimeBasisV1> {
        &self.ktime_basis
    }

    /// Raw source declaration state for `TimeMode`.
    pub const fn declared_time_mode(&self) -> &ExactFbxTimingObservationV1<FbxTimeModeV1> {
        &self.declared_time_mode
    }

    /// ufbx-effective time mode, including parser fallback.
    pub const fn effective_time_mode(&self) -> &ExactFbxTimingObservationV1<FbxTimeModeV1> {
        &self.effective_time_mode
    }

    /// Raw direct-property state for parser-projected `CustomFrameRate` binary64 evidence.
    pub const fn declared_custom_frame_rate(
        &self,
    ) -> &ExactFbxTimingObservationV1<FbxCustomFrameRateV1> {
        &self.declared_custom_frame_rate
    }

    /// Exact integer physical frame period, when supported.
    pub const fn frame_period(&self) -> &ExactFbxTimingObservationV1<FbxFramePeriodV1> {
        &self.frame_period
    }

    /// Raw source declaration state for `TimeProtocol`.
    pub const fn declared_time_protocol(&self) -> &ExactFbxTimingObservationV1<FbxTimeProtocolV1> {
        &self.declared_time_protocol
    }

    /// ufbx-effective time protocol, including parser fallback.
    pub const fn effective_time_protocol(&self) -> &ExactFbxTimingObservationV1<FbxTimeProtocolV1> {
        &self.effective_time_protocol
    }

    /// Coverage of the independently enumerable animation-stack domain.
    pub const fn stack_coverage(&self) -> SourceSetCoverageV1 {
        self.stack_coverage
    }

    /// Retained deterministic stack-prefix rows.
    pub fn stacks(&self) -> &[ExactFbxStackTimingV1] {
        &self.stacks
    }
}

/// Invalid exact FBX timing value or attachment invariant.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum ExactFbxTimingContractError {
    /// KTime ticks per second must be positive.
    #[error("FBX KTime ticks per second must be positive")]
    InvalidKTimeBasis,
    /// KTime ticks per frame must be positive.
    #[error("FBX KTime ticks per frame must be positive")]
    InvalidFramePeriod,
    /// A retained custom frame-rate payload must be finite and positive.
    #[error("FBX custom frame rate must be finite and positive")]
    InvalidCustomFrameRate,
    /// An observed stack range is reversed.
    #[error("exact FBX stack range must satisfy begin_ticks <= end_ticks")]
    ReversedStackRange,
    /// The bounded stack limit was exceeded.
    #[error("exact FBX timing has {count} stacks, exceeding the limit of {limit}")]
    TooManyStacks {
        /// Retained row count.
        count: usize,
        /// Public contract limit.
        limit: usize,
    },
    /// Stack rows do not form a canonical zero-based prefix.
    #[error("exact FBX stack index {actual} is not expected prefix index {expected}")]
    NonCanonicalStackIndex {
        /// Expected zero-based index.
        expected: usize,
        /// Actual retained index.
        actual: usize,
    },
    /// Exact FBX evidence was attached to a non-FBX source.
    #[error("exact FBX timing can only be attached to an FBX source")]
    NonFbxSource,
    /// Exact and raw stack prefixes have different lengths.
    #[error(
        "exact FBX stack count {exact} does not match retained source clip count {source_count}"
    )]
    StackCountMismatch {
        /// Exact timing rows.
        exact: usize,
        /// Existing V1 source clip rows.
        source_count: usize,
    },
    /// Exact and raw stack domains have different coverage.
    #[error("exact FBX stack coverage does not match existing source clip coverage")]
    StackCoverageMismatch,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_frame_periods_use_integer_ktime_rules() {
        let legacy = FbxKTimeBasisV1::new(FBX_KTIME_LEGACY_TICKS_PER_SECOND).unwrap();
        let standard = FbxKTimeBasisV1::new(FBX_KTIME_STANDARD_TICKS_PER_SECOND).unwrap();

        assert_eq!(
            FbxFramePeriodV1::for_mode(legacy, FbxTimeModeV1::NtscDropFrame)
                .unwrap()
                .ticks_per_frame(),
            1_541_078_138
        );
        assert_eq!(
            FbxFramePeriodV1::for_mode(legacy, FbxTimeModeV1::Fps59Dot94)
                .unwrap()
                .ticks_per_frame(),
            770_539_069
        );
        assert_eq!(
            FbxFramePeriodV1::for_mode(standard, FbxTimeModeV1::FilmFullFrame)
                .unwrap()
                .ticks_per_frame(),
            5_885_880
        );
    }

    #[test]
    fn custom_and_legacy_72_96_fail_closed() {
        let legacy = FbxKTimeBasisV1::new(FBX_KTIME_LEGACY_TICKS_PER_SECOND).unwrap();
        assert_eq!(
            FbxFramePeriodV1::for_mode(legacy, FbxTimeModeV1::Custom),
            Err(ExactFbxTimingUnavailableReasonV1::CustomFrameRateNotExact)
        );
        for mode in [FbxTimeModeV1::Fps72, FbxTimeModeV1::Fps96] {
            assert_eq!(
                FbxFramePeriodV1::for_mode(legacy, mode),
                Err(ExactFbxTimingUnavailableReasonV1::UnsupportedKTimeBasis)
            );
        }
    }

    #[test]
    fn whole_frame_predicate_is_signed_and_tick_exact() {
        let period = FbxFramePeriodV1::new(100).unwrap();
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
    fn long_timeline_remains_tick_exact_beyond_binary64_integer_precision() {
        let period = FbxFramePeriodV1::new(4_708_704).unwrap();
        let whole = period.ticks_per_frame() * 3_000_000_000;
        assert!(whole > (1i64 << 53));
        assert!(period.is_whole_frame(whole));
        assert!(!period.is_whole_frame(whole - 1));
        assert!(!period.is_whole_frame(whole + 1));
        assert_eq!(period.frame_index(whole), Some(3_000_000_000));

        assert_eq!(whole as f64, (whole - 1) as f64);
        assert_eq!(whole as f64, (whole + 1) as f64);
    }
}
