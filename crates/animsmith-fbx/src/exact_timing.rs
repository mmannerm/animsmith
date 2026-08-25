//! Exact FBX/KTime projection from the same successfully parsed ufbx scene.

use animsmith_core::{
    ExactFbxStackTimingV1, ExactFbxTimingContractError, ExactFbxTimingObservationStateV1,
    ExactFbxTimingObservationV1, ExactFbxTimingUnavailableReasonV1, ExactFbxTimingV1,
    FbxCustomFrameRateV1, FbxFramePeriodV1, FbxKTimeBasisV1, FbxStackTickRangeV1, FbxTimeModeV1,
    FbxTimeProtocolV1, FbxTimeSpanSelectionV1, SourceFactsError, SourceLoaderDispositionV1,
    SourceLogicalLocatorV1, SourceProvenanceV1, SourceSetCoverageV1,
};

pub(crate) fn project(
    scene: &ufbx::Scene,
    stack_coverage: SourceSetCoverageV1,
    retained_stack_count: usize,
) -> Result<ExactFbxTimingV1, ExactFbxTimingProjectionError> {
    let basis_provenance = parser_provenance("fbx:scene.metadata.ktime_second")?;
    let basis = FbxKTimeBasisV1::new(scene.metadata.ktime_second).ok();
    let ktime_basis = match basis {
        Some(value) => ExactFbxTimingObservationV1::observed(
            value,
            basis_provenance,
            SourceLoaderDispositionV1::Discarded,
        ),
        None => ExactFbxTimingObservationV1::unavailable(
            ExactFbxTimingUnavailableReasonV1::ParserUnavailable,
            Some(basis_provenance),
            SourceLoaderDispositionV1::Discarded,
        ),
    };

    let declared_time_mode = declared_time_mode(&scene.settings.props)?;
    let effective_mode = map_time_mode(scene.settings.time_mode);
    let effective_time_mode = ExactFbxTimingObservationV1::observed(
        effective_mode,
        parser_provenance("fbx:scene.settings.time_mode")?,
        SourceLoaderDispositionV1::Discarded,
    );
    let declared_custom_frame_rate = declared_custom_frame_rate(&scene.settings.props)?;
    let frame_period_provenance = match declared_time_mode.state() {
        ExactFbxTimingObservationStateV1::Observed(_) => {
            derived_provenance("fbx:scene.settings.time_mode/frame_period")?
        }
        ExactFbxTimingObservationStateV1::ProvenAbsent
        | ExactFbxTimingObservationStateV1::Unavailable(_) => {
            parser_provenance("fbx:scene.settings.time_mode/frame_period")?
        }
    };
    let frame_period = match basis {
        Some(basis) => match FbxFramePeriodV1::for_mode(basis, effective_mode) {
            Ok(period) => ExactFbxTimingObservationV1::observed(
                period,
                frame_period_provenance,
                SourceLoaderDispositionV1::Discarded,
            ),
            Err(reason) => ExactFbxTimingObservationV1::unavailable(
                reason,
                Some(frame_period_provenance),
                SourceLoaderDispositionV1::Unsupported,
            ),
        },
        None => ExactFbxTimingObservationV1::unavailable(
            ExactFbxTimingUnavailableReasonV1::ParserUnavailable,
            Some(frame_period_provenance),
            SourceLoaderDispositionV1::Unsupported,
        ),
    };

    let declared_time_protocol = declared_time_protocol(&scene.settings.props)?;
    let effective_time_protocol = ExactFbxTimingObservationV1::observed(
        map_time_protocol(scene.settings.time_protocol),
        parser_provenance("fbx:scene.settings.time_protocol")?,
        SourceLoaderDispositionV1::Discarded,
    );

    if retained_stack_count > scene.anim_stacks.len() {
        return Err(ExactFbxTimingProjectionError::RetainedStackCount {
            retained: retained_stack_count,
            parsed: scene.anim_stacks.len(),
        });
    }
    let mut stacks = Vec::with_capacity(retained_stack_count);
    for (source_stack_index, stack) in scene
        .anim_stacks
        .iter()
        .take(retained_stack_count)
        .enumerate()
    {
        stacks.push(project_stack(source_stack_index, stack)?);
    }

    Ok(ExactFbxTimingV1::new(
        ktime_basis,
        declared_time_mode,
        effective_time_mode,
        declared_custom_frame_rate,
        frame_period,
        declared_time_protocol,
        effective_time_protocol,
        stack_coverage,
        stacks,
    )?)
}

fn declared_custom_frame_rate(
    props: &ufbx::Props,
) -> Result<ExactFbxTimingObservationV1<FbxCustomFrameRateV1>, SourceFactsError> {
    let locator = "fbx:scene.settings.props.CustomFrameRate";
    let absent_provenance = parser_provenance(locator)?;
    let Some(found) = find_direct_prop(props, "CustomFrameRate") else {
        return Ok(ExactFbxTimingObservationV1::proven_absent(
            absent_provenance,
        ));
    };
    let provenance = found.provenance(locator)?;
    if found.prop.type_ != ufbx::PropType::Number
        || !found.prop.flags.has_any(ufbx::PropFlags::VALUE_REAL)
    {
        return Ok(ExactFbxTimingObservationV1::unavailable(
            ExactFbxTimingUnavailableReasonV1::Malformed,
            Some(provenance),
            SourceLoaderDispositionV1::Discarded,
        ));
    }
    match FbxCustomFrameRateV1::new(found.prop.value_vec4.x) {
        Ok(value) => Ok(ExactFbxTimingObservationV1::observed(
            value,
            provenance,
            SourceLoaderDispositionV1::Discarded,
        )),
        Err(_) => Ok(ExactFbxTimingObservationV1::unavailable(
            ExactFbxTimingUnavailableReasonV1::Malformed,
            Some(provenance),
            SourceLoaderDispositionV1::Discarded,
        )),
    }
}

fn declared_time_mode(
    props: &ufbx::Props,
) -> Result<ExactFbxTimingObservationV1<FbxTimeModeV1>, SourceFactsError> {
    declared_enum(
        props,
        "TimeMode",
        "fbx:scene.settings.props.TimeMode",
        ExactFbxTimingUnavailableReasonV1::UnsupportedTimeMode,
        |value| match value {
            0 => Some(FbxTimeModeV1::Default),
            1 => Some(FbxTimeModeV1::Fps120),
            2 => Some(FbxTimeModeV1::Fps100),
            3 => Some(FbxTimeModeV1::Fps60),
            4 => Some(FbxTimeModeV1::Fps50),
            5 => Some(FbxTimeModeV1::Fps48),
            6 => Some(FbxTimeModeV1::Fps30),
            7 => Some(FbxTimeModeV1::Fps30Drop),
            8 => Some(FbxTimeModeV1::NtscDropFrame),
            9 => Some(FbxTimeModeV1::NtscFullFrame),
            10 => Some(FbxTimeModeV1::Pal),
            11 => Some(FbxTimeModeV1::Fps24),
            12 => Some(FbxTimeModeV1::Fps1000),
            13 => Some(FbxTimeModeV1::FilmFullFrame),
            14 => Some(FbxTimeModeV1::Custom),
            15 => Some(FbxTimeModeV1::Fps96),
            16 => Some(FbxTimeModeV1::Fps72),
            17 => Some(FbxTimeModeV1::Fps59Dot94),
            _ => None,
        },
    )
}

fn declared_time_protocol(
    props: &ufbx::Props,
) -> Result<ExactFbxTimingObservationV1<FbxTimeProtocolV1>, SourceFactsError> {
    declared_enum(
        props,
        "TimeProtocol",
        "fbx:scene.settings.props.TimeProtocol",
        // Protocol controls timecode presentation rather than the physical frame lattice.
        // A structurally valid unknown integer is therefore closed as malformed evidence.
        ExactFbxTimingUnavailableReasonV1::Malformed,
        |value| match value {
            0 => Some(FbxTimeProtocolV1::Smpte),
            1 => Some(FbxTimeProtocolV1::FrameCount),
            2 => Some(FbxTimeProtocolV1::Default),
            _ => None,
        },
    )
}

fn declared_enum<T>(
    props: &ufbx::Props,
    name: &str,
    locator: &str,
    unsupported_reason: ExactFbxTimingUnavailableReasonV1,
    map: impl FnOnce(i64) -> Option<T>,
) -> Result<ExactFbxTimingObservationV1<T>, SourceFactsError> {
    let absent_provenance = parser_provenance(locator)?;
    let Some(found) = find_direct_prop(props, name) else {
        return Ok(ExactFbxTimingObservationV1::proven_absent(
            absent_provenance,
        ));
    };
    let provenance = found.provenance(locator)?;
    if found.prop.type_ != ufbx::PropType::Integer || !has_exact_enum_value(found.prop) {
        return Ok(ExactFbxTimingObservationV1::unavailable(
            ExactFbxTimingUnavailableReasonV1::Malformed,
            Some(provenance),
            SourceLoaderDispositionV1::Discarded,
        ));
    }
    match map(found.prop.value_int) {
        Some(value) => Ok(ExactFbxTimingObservationV1::observed(
            value,
            provenance,
            SourceLoaderDispositionV1::Discarded,
        )),
        None => Ok(ExactFbxTimingObservationV1::unavailable(
            unsupported_reason,
            Some(provenance),
            SourceLoaderDispositionV1::Discarded,
        )),
    }
}

fn project_stack(
    source_stack_index: usize,
    stack: &ufbx::AnimStack,
) -> Result<ExactFbxStackTimingV1, SourceFactsError> {
    let local_begin = find_resolved_prop(&stack.element.props, "LocalStart");
    let local_end = find_resolved_prop(&stack.element.props, "LocalStop");
    let selected = if local_begin.is_some() && local_end.is_some() {
        Some((FbxTimeSpanSelectionV1::Local, local_begin, local_end))
    } else {
        let reference_begin = find_resolved_prop(&stack.element.props, "ReferenceStart");
        let reference_end = find_resolved_prop(&stack.element.props, "ReferenceStop");
        (reference_begin.is_some() && reference_end.is_some()).then_some((
            FbxTimeSpanSelectionV1::Reference,
            reference_begin,
            reference_end,
        ))
    };
    let locator = format!("fbx:anim_stacks/{source_stack_index}/exact_time_range");
    let source_tick_range = match selected {
        None => ExactFbxTimingObservationV1::proven_absent(parser_provenance(&locator)?),
        Some((selection, Some(begin), Some(end))) => {
            let provenance = pair_provenance(begin, end, &locator)?;
            if !has_exact_ktime_value(begin.prop) || !has_exact_ktime_value(end.prop) {
                ExactFbxTimingObservationV1::unavailable(
                    ExactFbxTimingUnavailableReasonV1::Malformed,
                    Some(provenance),
                    SourceLoaderDispositionV1::Baked,
                )
            } else {
                match FbxStackTickRangeV1::new(selection, begin.prop.value_int, end.prop.value_int)
                {
                    Ok(range) => ExactFbxTimingObservationV1::observed(
                        range,
                        provenance,
                        SourceLoaderDispositionV1::Baked,
                    ),
                    Err(ExactFbxTimingContractError::ReversedStackRange) => {
                        ExactFbxTimingObservationV1::unavailable(
                            ExactFbxTimingUnavailableReasonV1::Malformed,
                            Some(provenance),
                            SourceLoaderDispositionV1::Baked,
                        )
                    }
                    Err(_) => ExactFbxTimingObservationV1::unavailable(
                        ExactFbxTimingUnavailableReasonV1::ParserUnavailable,
                        Some(provenance),
                        SourceLoaderDispositionV1::Baked,
                    ),
                }
            }
        }
        Some((_, _, _)) => unreachable!("selected pair is complete"),
    };
    Ok(ExactFbxStackTimingV1::new(
        source_stack_index,
        source_tick_range,
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

fn map_time_mode(mode: ufbx::TimeMode) -> FbxTimeModeV1 {
    match mode {
        ufbx::TimeMode::Default => FbxTimeModeV1::Default,
        ufbx::TimeMode::E120Fps => FbxTimeModeV1::Fps120,
        ufbx::TimeMode::E100Fps => FbxTimeModeV1::Fps100,
        ufbx::TimeMode::E60Fps => FbxTimeModeV1::Fps60,
        ufbx::TimeMode::E50Fps => FbxTimeModeV1::Fps50,
        ufbx::TimeMode::E48Fps => FbxTimeModeV1::Fps48,
        ufbx::TimeMode::E30Fps => FbxTimeModeV1::Fps30,
        ufbx::TimeMode::E30FpsDrop => FbxTimeModeV1::Fps30Drop,
        ufbx::TimeMode::NtscDropFrame => FbxTimeModeV1::NtscDropFrame,
        ufbx::TimeMode::NtscFullFrame => FbxTimeModeV1::NtscFullFrame,
        ufbx::TimeMode::Pal => FbxTimeModeV1::Pal,
        ufbx::TimeMode::E24Fps => FbxTimeModeV1::Fps24,
        ufbx::TimeMode::E1000Fps => FbxTimeModeV1::Fps1000,
        ufbx::TimeMode::FilmFullFrame => FbxTimeModeV1::FilmFullFrame,
        ufbx::TimeMode::Custom => FbxTimeModeV1::Custom,
        ufbx::TimeMode::E96Fps => FbxTimeModeV1::Fps96,
        ufbx::TimeMode::E72Fps => FbxTimeModeV1::Fps72,
        ufbx::TimeMode::E5994Fps => FbxTimeModeV1::Fps59Dot94,
    }
}

fn map_time_protocol(protocol: ufbx::TimeProtocol) -> FbxTimeProtocolV1 {
    match protocol {
        ufbx::TimeProtocol::Smpte => FbxTimeProtocolV1::Smpte,
        ufbx::TimeProtocol::FrameCount => FbxTimeProtocolV1::FrameCount,
        ufbx::TimeProtocol::Default => FbxTimeProtocolV1::Default,
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
pub(crate) enum ExactFbxTimingProjectionError {
    #[error(transparent)]
    SourceFacts(#[from] SourceFactsError),
    #[error(transparent)]
    Contract(#[from] ExactFbxTimingContractError),
    #[error("retained source stack count {retained} exceeds parsed stack count {parsed}")]
    RetainedStackCount { retained: usize, parsed: usize },
}
