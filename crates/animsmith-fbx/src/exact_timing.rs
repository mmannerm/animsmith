//! Exact FBX/KTime projection from the same successfully parsed ufbx scene.

use animsmith_core::{
    ExactSourceClipTimeRangeV1, ExactSourceClipTimingV1, ExactSourceFramePeriodV1,
    ExactSourceRangeSelectionV1, ExactSourceTimeBasisV1, ExactSourceTimingContractError,
    ExactSourceTimingObservationStateV1, ExactSourceTimingObservationV1,
    ExactSourceTimingUnavailableReasonV1, ExactSourceTimingV1, ParserFrameRateProjectionV1,
    SourceFactsError, SourceLoaderDispositionV1, SourceLogicalLocatorV1, SourceProvenanceV1,
    SourceSetCoverageV1, SourceTimeDisplayProtocolV1, SourceTimelineModeV1,
};

/// Legacy FBX/KTime units in one second.
pub const FBX_KTIME_LEGACY_UNITS_PER_SECOND: i64 = 46_186_158_000;
/// Standard FBX/KTime units in one second.
pub const FBX_KTIME_STANDARD_UNITS_PER_SECOND: i64 = 141_120_000;

pub(crate) fn project(
    scene: &ufbx::Scene,
    clip_coverage: SourceSetCoverageV1,
    retained_stack_count: usize,
) -> Result<ExactSourceTimingV1, ExactSourceTimingProjectionError> {
    let basis_provenance = parser_provenance("fbx:scene.metadata.ktime_second")?;
    let basis = ExactSourceTimeBasisV1::new(scene.metadata.ktime_second).ok();
    let time_basis = match basis {
        Some(value) => ExactSourceTimingObservationV1::observed(
            value,
            basis_provenance,
            SourceLoaderDispositionV1::Discarded,
        ),
        None => ExactSourceTimingObservationV1::unavailable(
            ExactSourceTimingUnavailableReasonV1::ParserUnavailable,
            Some(basis_provenance),
            SourceLoaderDispositionV1::Discarded,
        ),
    };

    let declared_time_mode = declared_time_mode(&scene.settings.props)?;
    let effective_mode = map_time_mode(scene.settings.time_mode);
    let effective_time_mode = ExactSourceTimingObservationV1::observed(
        effective_mode,
        parser_provenance("fbx:scene.settings.time_mode")?,
        SourceLoaderDispositionV1::Discarded,
    );
    let declared_custom_frame_rate = declared_custom_frame_rate(&scene.settings.props)?;
    let frame_period_provenance = match declared_time_mode.state() {
        ExactSourceTimingObservationStateV1::Observed(_) => {
            derived_provenance("fbx:scene.settings.time_mode/frame_period")?
        }
        ExactSourceTimingObservationStateV1::ProvenAbsent
        | ExactSourceTimingObservationStateV1::Unavailable(_) => {
            parser_provenance("fbx:scene.settings.time_mode/frame_period")?
        }
    };
    let frame_period = match basis {
        Some(basis) => match fbx_frame_period_for_mode(basis, effective_mode) {
            Ok(period) => ExactSourceTimingObservationV1::observed(
                period,
                frame_period_provenance,
                SourceLoaderDispositionV1::Discarded,
            ),
            Err(reason) => ExactSourceTimingObservationV1::unavailable(
                reason,
                Some(frame_period_provenance),
                SourceLoaderDispositionV1::Unsupported,
            ),
        },
        None => ExactSourceTimingObservationV1::unavailable(
            ExactSourceTimingUnavailableReasonV1::ParserUnavailable,
            Some(frame_period_provenance),
            SourceLoaderDispositionV1::Unsupported,
        ),
    };

    let declared_time_protocol = declared_time_protocol(&scene.settings.props)?;
    let effective_time_protocol = ExactSourceTimingObservationV1::observed(
        map_time_protocol(scene.settings.time_protocol),
        parser_provenance("fbx:scene.settings.time_protocol")?,
        SourceLoaderDispositionV1::Discarded,
    );

    if retained_stack_count > scene.anim_stacks.len() {
        return Err(ExactSourceTimingProjectionError::RetainedStackCount {
            retained: retained_stack_count,
            parsed: scene.anim_stacks.len(),
        });
    }
    let mut clips = Vec::with_capacity(retained_stack_count);
    for (source_clip_index, stack) in scene
        .anim_stacks
        .iter()
        .take(retained_stack_count)
        .enumerate()
    {
        clips.push(project_stack(source_clip_index, stack)?);
    }

    Ok(ExactSourceTimingV1::new(
        time_basis,
        declared_time_mode,
        effective_time_mode,
        declared_custom_frame_rate,
        frame_period,
        declared_time_protocol,
        effective_time_protocol,
        clip_coverage,
        clips,
    )?)
}

/// Derive the SDK-compatible integer KTime period for a standard FBX time mode.
///
/// This intentionally remains in the FBX crate: `animsmith-core` only models a
/// format-neutral exact source-time lattice and never knows KTime constants or
/// FBX's mode-to-period operation order.
fn fbx_frame_period_for_mode(
    basis: ExactSourceTimeBasisV1,
    mode: SourceTimelineModeV1,
) -> Result<ExactSourceFramePeriodV1, ExactSourceTimingUnavailableReasonV1> {
    let units = basis.units_per_second();
    if !matches!(
        units,
        FBX_KTIME_LEGACY_UNITS_PER_SECOND | FBX_KTIME_STANDARD_UNITS_PER_SECOND
    ) {
        return Err(ExactSourceTimingUnavailableReasonV1::UnsupportedTimeBasis);
    }
    let integer_rate = match mode {
        SourceTimelineModeV1::Default
        | SourceTimelineModeV1::Fps30
        | SourceTimelineModeV1::Fps30Drop => Some(30),
        SourceTimelineModeV1::Fps120 => Some(120),
        SourceTimelineModeV1::Fps100 => Some(100),
        SourceTimelineModeV1::Fps60 => Some(60),
        SourceTimelineModeV1::Fps50 => Some(50),
        SourceTimelineModeV1::Fps48 => Some(48),
        SourceTimelineModeV1::Pal => Some(25),
        SourceTimelineModeV1::Fps24 => Some(24),
        SourceTimelineModeV1::Fps1000 => Some(1000),
        SourceTimelineModeV1::Fps96 => Some(96),
        SourceTimelineModeV1::Fps72 => Some(72),
        SourceTimelineModeV1::NtscDropFrame
        | SourceTimelineModeV1::NtscFullFrame
        | SourceTimelineModeV1::FilmFullFrame
        | SourceTimelineModeV1::Fps59Dot94
        | SourceTimelineModeV1::Custom => None,
    };
    if let Some(rate) = integer_rate {
        if units % rate != 0 {
            return Err(ExactSourceTimingUnavailableReasonV1::UnsupportedTimeBasis);
        }
        return ExactSourceFramePeriodV1::new(units / rate)
            .map_err(|_| ExactSourceTimingUnavailableReasonV1::UnsupportedTimeBasis);
    }
    let units = i128::from(units);
    let period = match mode {
        SourceTimelineModeV1::NtscDropFrame | SourceTimelineModeV1::NtscFullFrame => {
            (units / 30 * 1001) / 1000
        }
        SourceTimelineModeV1::FilmFullFrame => (units / 24 * 1001) / 1000,
        SourceTimelineModeV1::Fps59Dot94 => ((units / 30 * 1001) / 1000) / 2,
        SourceTimelineModeV1::Custom => {
            return Err(ExactSourceTimingUnavailableReasonV1::CustomFrameRateNotExact);
        }
        _ => return Err(ExactSourceTimingUnavailableReasonV1::UnsupportedTimeMode),
    };
    let period = i64::try_from(period)
        .map_err(|_| ExactSourceTimingUnavailableReasonV1::UnsupportedTimeBasis)?;
    ExactSourceFramePeriodV1::new(period)
        .map_err(|_| ExactSourceTimingUnavailableReasonV1::UnsupportedTimeBasis)
}

fn declared_custom_frame_rate(
    props: &ufbx::Props,
) -> Result<ExactSourceTimingObservationV1<ParserFrameRateProjectionV1>, SourceFactsError> {
    let locator = "fbx:scene.settings.props.CustomFrameRate";
    let absent_provenance = parser_provenance(locator)?;
    let Some(found) = find_direct_prop(props, "CustomFrameRate") else {
        return Ok(ExactSourceTimingObservationV1::proven_absent(
            absent_provenance,
        ));
    };
    let provenance = found.provenance(locator)?;
    if found.prop.type_ != ufbx::PropType::Number
        || !found.prop.flags.has_any(ufbx::PropFlags::VALUE_REAL)
    {
        return Ok(ExactSourceTimingObservationV1::unavailable(
            ExactSourceTimingUnavailableReasonV1::Malformed,
            Some(provenance),
            SourceLoaderDispositionV1::Discarded,
        ));
    }
    match ParserFrameRateProjectionV1::new(found.prop.value_vec4.x) {
        Ok(value) => Ok(ExactSourceTimingObservationV1::observed(
            value,
            provenance,
            SourceLoaderDispositionV1::Discarded,
        )),
        Err(_) => Ok(ExactSourceTimingObservationV1::unavailable(
            ExactSourceTimingUnavailableReasonV1::Malformed,
            Some(provenance),
            SourceLoaderDispositionV1::Discarded,
        )),
    }
}

fn declared_time_mode(
    props: &ufbx::Props,
) -> Result<ExactSourceTimingObservationV1<SourceTimelineModeV1>, SourceFactsError> {
    declared_enum(
        props,
        "TimeMode",
        "fbx:scene.settings.props.TimeMode",
        ExactSourceTimingUnavailableReasonV1::UnsupportedTimeMode,
        |value| match value {
            0 => Some(SourceTimelineModeV1::Default),
            1 => Some(SourceTimelineModeV1::Fps120),
            2 => Some(SourceTimelineModeV1::Fps100),
            3 => Some(SourceTimelineModeV1::Fps60),
            4 => Some(SourceTimelineModeV1::Fps50),
            5 => Some(SourceTimelineModeV1::Fps48),
            6 => Some(SourceTimelineModeV1::Fps30),
            7 => Some(SourceTimelineModeV1::Fps30Drop),
            8 => Some(SourceTimelineModeV1::NtscDropFrame),
            9 => Some(SourceTimelineModeV1::NtscFullFrame),
            10 => Some(SourceTimelineModeV1::Pal),
            11 => Some(SourceTimelineModeV1::Fps24),
            12 => Some(SourceTimelineModeV1::Fps1000),
            13 => Some(SourceTimelineModeV1::FilmFullFrame),
            14 => Some(SourceTimelineModeV1::Custom),
            15 => Some(SourceTimelineModeV1::Fps96),
            16 => Some(SourceTimelineModeV1::Fps72),
            17 => Some(SourceTimelineModeV1::Fps59Dot94),
            _ => None,
        },
    )
}

fn declared_time_protocol(
    props: &ufbx::Props,
) -> Result<ExactSourceTimingObservationV1<SourceTimeDisplayProtocolV1>, SourceFactsError> {
    declared_enum(
        props,
        "TimeProtocol",
        "fbx:scene.settings.props.TimeProtocol",
        // Protocol controls timecode presentation rather than the physical frame lattice.
        // A structurally valid unknown integer is therefore closed as malformed evidence.
        ExactSourceTimingUnavailableReasonV1::Malformed,
        |value| match value {
            0 => Some(SourceTimeDisplayProtocolV1::Smpte),
            1 => Some(SourceTimeDisplayProtocolV1::FrameCount),
            2 => Some(SourceTimeDisplayProtocolV1::Default),
            _ => None,
        },
    )
}

fn declared_enum<T>(
    props: &ufbx::Props,
    name: &str,
    locator: &str,
    unsupported_reason: ExactSourceTimingUnavailableReasonV1,
    map: impl FnOnce(i64) -> Option<T>,
) -> Result<ExactSourceTimingObservationV1<T>, SourceFactsError> {
    let absent_provenance = parser_provenance(locator)?;
    let Some(found) = find_direct_prop(props, name) else {
        return Ok(ExactSourceTimingObservationV1::proven_absent(
            absent_provenance,
        ));
    };
    let provenance = found.provenance(locator)?;
    if found.prop.type_ != ufbx::PropType::Integer || !has_exact_enum_value(found.prop) {
        return Ok(ExactSourceTimingObservationV1::unavailable(
            ExactSourceTimingUnavailableReasonV1::Malformed,
            Some(provenance),
            SourceLoaderDispositionV1::Discarded,
        ));
    }
    match map(found.prop.value_int) {
        Some(value) => Ok(ExactSourceTimingObservationV1::observed(
            value,
            provenance,
            SourceLoaderDispositionV1::Discarded,
        )),
        None => Ok(ExactSourceTimingObservationV1::unavailable(
            unsupported_reason,
            Some(provenance),
            SourceLoaderDispositionV1::Discarded,
        )),
    }
}

fn project_stack(
    source_clip_index: usize,
    stack: &ufbx::AnimStack,
) -> Result<ExactSourceClipTimingV1, SourceFactsError> {
    let local_begin = find_resolved_prop(&stack.element.props, "LocalStart");
    let local_end = find_resolved_prop(&stack.element.props, "LocalStop");
    let selected = if local_begin.is_some() && local_end.is_some() {
        Some((ExactSourceRangeSelectionV1::Primary, local_begin, local_end))
    } else {
        let reference_begin = find_resolved_prop(&stack.element.props, "ReferenceStart");
        let reference_end = find_resolved_prop(&stack.element.props, "ReferenceStop");
        (reference_begin.is_some() && reference_end.is_some()).then_some((
            ExactSourceRangeSelectionV1::Fallback,
            reference_begin,
            reference_end,
        ))
    };
    let locator = format!("fbx:anim_stacks/{source_clip_index}/exact_time_range");
    let source_time_range = match selected {
        None => ExactSourceTimingObservationV1::proven_absent(parser_provenance(&locator)?),
        Some((selection, Some(begin), Some(end))) => {
            let provenance = pair_provenance(begin, end, &locator)?;
            if !has_exact_ktime_value(begin.prop) || !has_exact_ktime_value(end.prop) {
                ExactSourceTimingObservationV1::unavailable(
                    ExactSourceTimingUnavailableReasonV1::Malformed,
                    Some(provenance),
                    SourceLoaderDispositionV1::Baked,
                )
            } else {
                match ExactSourceClipTimeRangeV1::new(
                    selection,
                    begin.prop.value_int,
                    end.prop.value_int,
                ) {
                    Ok(range) => ExactSourceTimingObservationV1::observed(
                        range,
                        provenance,
                        SourceLoaderDispositionV1::Baked,
                    ),
                    Err(ExactSourceTimingContractError::ReversedClipRange) => {
                        ExactSourceTimingObservationV1::unavailable(
                            ExactSourceTimingUnavailableReasonV1::Malformed,
                            Some(provenance),
                            SourceLoaderDispositionV1::Baked,
                        )
                    }
                    Err(_) => ExactSourceTimingObservationV1::unavailable(
                        ExactSourceTimingUnavailableReasonV1::ParserUnavailable,
                        Some(provenance),
                        SourceLoaderDispositionV1::Baked,
                    ),
                }
            }
        }
        Some((_, _, _)) => unreachable!("selected pair is complete"),
    };
    Ok(ExactSourceClipTimingV1::new(
        source_clip_index,
        source_time_range,
    ))
}

#[derive(Clone, Copy)]
struct FoundProp<'a> {
    prop: &'a ufbx::Prop,
    projected: bool,
}

impl FoundProp<'_> {
    fn provenance(&self, locator: &str) -> Result<SourceProvenanceV1, SourceFactsError> {
        let locator = SourceLogicalLocatorV1::fbx_parser_path(locator)?;
        if self.projected || self.prop.flags.has_any(ufbx::PropFlags::SYNTHETIC) {
            Ok(SourceProvenanceV1::parser_projected(locator))
        } else {
            Ok(SourceProvenanceV1::source_declared(locator))
        }
    }
}

fn find_direct_prop<'a>(props: &'a ufbx::Props, name: &str) -> Option<FoundProp<'a>> {
    props
        .props
        .iter()
        .find(|prop| prop.name.as_ref() == name && !prop.flags.has_any(ufbx::PropFlags::NO_VALUE))
        .map(|prop| FoundProp {
            prop,
            projected: false,
        })
}

fn find_resolved_prop<'a>(props: &'a ufbx::Props, name: &str) -> Option<FoundProp<'a>> {
    let mut current = Some(props);
    let mut projected = false;
    while let Some(props) = current {
        if let Some(prop) = props.props.iter().find(|prop| {
            prop.name.as_ref() == name && !prop.flags.has_any(ufbx::PropFlags::NO_VALUE)
        }) {
            return Some(FoundProp { prop, projected });
        }
        current = props.defaults.as_ref().map(|defaults| defaults.as_ref());
        projected = true;
    }
    None
}

fn has_exact_ktime_value(prop: &ufbx::Prop) -> bool {
    prop.type_ == ufbx::PropType::Integer
        && prop.flags.has_all(ufbx::PropFlags::VALUE_INT)
        && !prop.flags.has_any(ufbx::PropFlags::NO_VALUE)
}

fn has_exact_enum_value(prop: &ufbx::Prop) -> bool {
    prop.flags.has_all(ufbx::PropFlags::VALUE_INT) || prop.flags.has_any(ufbx::PropFlags::SYNTHETIC)
}

fn pair_provenance(
    begin: FoundProp<'_>,
    end: FoundProp<'_>,
    locator: &str,
) -> Result<SourceProvenanceV1, SourceFactsError> {
    let locator = SourceLogicalLocatorV1::fbx_parser_path(locator)?;
    if begin.projected
        || end.projected
        || begin.prop.flags.has_any(ufbx::PropFlags::SYNTHETIC)
        || end.prop.flags.has_any(ufbx::PropFlags::SYNTHETIC)
    {
        Ok(SourceProvenanceV1::parser_projected(locator))
    } else {
        Ok(SourceProvenanceV1::source_declared(locator))
    }
}

fn map_time_mode(mode: ufbx::TimeMode) -> SourceTimelineModeV1 {
    match mode {
        ufbx::TimeMode::Default => SourceTimelineModeV1::Default,
        ufbx::TimeMode::E120Fps => SourceTimelineModeV1::Fps120,
        ufbx::TimeMode::E100Fps => SourceTimelineModeV1::Fps100,
        ufbx::TimeMode::E60Fps => SourceTimelineModeV1::Fps60,
        ufbx::TimeMode::E50Fps => SourceTimelineModeV1::Fps50,
        ufbx::TimeMode::E48Fps => SourceTimelineModeV1::Fps48,
        ufbx::TimeMode::E30Fps => SourceTimelineModeV1::Fps30,
        ufbx::TimeMode::E30FpsDrop => SourceTimelineModeV1::Fps30Drop,
        ufbx::TimeMode::NtscDropFrame => SourceTimelineModeV1::NtscDropFrame,
        ufbx::TimeMode::NtscFullFrame => SourceTimelineModeV1::NtscFullFrame,
        ufbx::TimeMode::Pal => SourceTimelineModeV1::Pal,
        ufbx::TimeMode::E24Fps => SourceTimelineModeV1::Fps24,
        ufbx::TimeMode::E1000Fps => SourceTimelineModeV1::Fps1000,
        ufbx::TimeMode::FilmFullFrame => SourceTimelineModeV1::FilmFullFrame,
        ufbx::TimeMode::Custom => SourceTimelineModeV1::Custom,
        ufbx::TimeMode::E96Fps => SourceTimelineModeV1::Fps96,
        ufbx::TimeMode::E72Fps => SourceTimelineModeV1::Fps72,
        ufbx::TimeMode::E5994Fps => SourceTimelineModeV1::Fps59Dot94,
    }
}

fn map_time_protocol(protocol: ufbx::TimeProtocol) -> SourceTimeDisplayProtocolV1 {
    match protocol {
        ufbx::TimeProtocol::Smpte => SourceTimeDisplayProtocolV1::Smpte,
        ufbx::TimeProtocol::FrameCount => SourceTimeDisplayProtocolV1::FrameCount,
        ufbx::TimeProtocol::Default => SourceTimeDisplayProtocolV1::Default,
    }
}

fn parser_provenance(value: &str) -> Result<SourceProvenanceV1, SourceFactsError> {
    Ok(SourceProvenanceV1::parser_projected(
        SourceLogicalLocatorV1::fbx_parser_path(value)?,
    ))
}

fn derived_provenance(value: &str) -> Result<SourceProvenanceV1, SourceFactsError> {
    Ok(SourceProvenanceV1::derived_from_source(
        SourceLogicalLocatorV1::fbx_parser_path(value)?,
    ))
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum ExactSourceTimingProjectionError {
    #[error(transparent)]
    SourceFacts(#[from] SourceFactsError),
    #[error(transparent)]
    Contract(#[from] ExactSourceTimingContractError),
    #[error("retained source stack count {retained} exceeds parsed stack count {parsed}")]
    RetainedStackCount { retained: usize, parsed: usize },
}
