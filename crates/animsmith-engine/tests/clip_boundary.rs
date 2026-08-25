use animsmith_core::{
    Check, CheckCtx, CheckSelection, DependencyClosureBuilderV1, Document,
    EnginePredictionFacetStateV1, ExactFbxStackTimingV1, ExactFbxTimingObservationV1,
    ExactFbxTimingUnavailableReasonV1, ExactFbxTimingV1, FBX_KTIME_LEGACY_TICKS_PER_SECOND,
    FbxFramePeriodV1, FbxKTimeBasisV1, FbxStackTickRangeV1, FbxTimeModeV1, FbxTimeProtocolV1,
    FbxTimeSpanSelectionV1, InputIdentity, MetricGrids, RawSourceFactsBuilderV1, ResolvedRoles,
    SourceClipFactV1, SourceFactDomainV1, SourceFactSetV1, SourceFormatV1,
    SourceLoaderDispositionV1, SourceObservationV1, SourceProvenanceV1, SourceSetCoverageV1,
    SourceTextV1, SourceUnavailableReasonV1,
};
use animsmith_engine::{
    ENGINE_CLIP_BOUNDARY_CHECK_ID, EngineClipBoundaryCheck, EngineDeclaration, ProfileSelection,
    project_prediction_provenance_v3, resolve_static,
};

#[derive(Clone, Copy)]
enum Coverage {
    Complete,
    Partial,
}

#[derive(Clone, Copy)]
enum StackRange {
    Observed { begin: i64, end: i64 },
    Unavailable(ExactFbxTimingUnavailableReasonV1),
}

fn exact_observed<T>(value: T) -> ExactFbxTimingObservationV1<T> {
    ExactFbxTimingObservationV1::observed(
        value,
        SourceProvenanceV1::format_defined(),
        SourceLoaderDispositionV1::Preserved,
    )
}

fn exact_unavailable<T>(
    reason: ExactFbxTimingUnavailableReasonV1,
) -> ExactFbxTimingObservationV1<T> {
    ExactFbxTimingObservationV1::unavailable(
        reason,
        Some(SourceProvenanceV1::format_defined()),
        SourceLoaderDispositionV1::Unknown,
    )
}

fn timing(coverage: Coverage, mode: FbxTimeModeV1, ranges: &[StackRange]) -> ExactFbxTimingV1 {
    let basis = FbxKTimeBasisV1::new(FBX_KTIME_LEGACY_TICKS_PER_SECOND).unwrap();
    let period = FbxFramePeriodV1::for_mode(basis, mode).unwrap();
    ExactFbxTimingV1::new(
        exact_observed(basis),
        exact_observed(mode),
        exact_observed(mode),
        ExactFbxTimingObservationV1::proven_absent(SourceProvenanceV1::format_defined()),
        exact_observed(period),
        ExactFbxTimingObservationV1::proven_absent(SourceProvenanceV1::format_defined()),
        exact_observed(FbxTimeProtocolV1::Default),
        match coverage {
            Coverage::Complete => SourceSetCoverageV1::complete(),
            Coverage::Partial => {
                SourceSetCoverageV1::partial(SourceUnavailableReasonV1::ProjectionBudgetExceeded)
            }
        },
        ranges
            .iter()
            .enumerate()
            .map(|(index, range)| {
                let range = match *range {
                    StackRange::Observed { begin, end } => exact_observed(
                        FbxStackTickRangeV1::new(FbxTimeSpanSelectionV1::Local, begin, end)
                            .unwrap(),
                    ),
                    StackRange::Unavailable(reason) => exact_unavailable(reason),
                };
                ExactFbxStackTimingV1::new(index, range)
            })
            .collect(),
    )
    .unwrap()
}

fn timing_without_declared_mode(end_ticks: i64) -> ExactFbxTimingV1 {
    let basis = FbxKTimeBasisV1::new(FBX_KTIME_LEGACY_TICKS_PER_SECOND).unwrap();
    let mode = FbxTimeModeV1::Fps24;
    let period = FbxFramePeriodV1::for_mode(basis, mode).unwrap();
    ExactFbxTimingV1::new(
        exact_observed(basis),
        ExactFbxTimingObservationV1::proven_absent(SourceProvenanceV1::format_defined()),
        exact_observed(mode),
        ExactFbxTimingObservationV1::proven_absent(SourceProvenanceV1::format_defined()),
        exact_observed(period),
        ExactFbxTimingObservationV1::proven_absent(SourceProvenanceV1::format_defined()),
        exact_observed(FbxTimeProtocolV1::Default),
        SourceSetCoverageV1::complete(),
        vec![ExactFbxStackTimingV1::new(
            0,
            exact_observed(
                FbxStackTickRangeV1::new(FbxTimeSpanSelectionV1::Local, 0, end_ticks).unwrap(),
            ),
        )],
    )
    .unwrap()
}

fn source(
    coverage: Coverage,
    stack_count: usize,
    exact: Option<ExactFbxTimingV1>,
) -> animsmith_core::LoadedSource {
    let primary = InputIdentity::from_bytes(format!("fbx:{stack_count}").as_bytes());
    let mut facts = RawSourceFactsBuilderV1::new(SourceFormatV1::Fbx, primary.clone());
    for index in 0..stack_count {
        assert!(facts.push_clip(SourceClipFactV1::new(
            index,
            SourceObservationV1::observed(
                SourceTextV1::new(format!("stack-{index}")).unwrap(),
                SourceProvenanceV1::format_defined(),
                SourceLoaderDispositionV1::Preserved,
            ),
            SourceObservationV1::observed(
                index,
                SourceProvenanceV1::format_defined(),
                SourceLoaderDispositionV1::Preserved,
            ),
            SourceObservationV1::proven_absent(SourceProvenanceV1::format_defined()),
            SourceObservationV1::proven_absent(SourceProvenanceV1::format_defined()),
            SourceFactSetV1::complete(Vec::new()),
        )));
    }
    match coverage {
        Coverage::Complete => facts.mark_complete(SourceFactDomainV1::Clips),
        Coverage::Partial => facts.mark_partial(
            SourceFactDomainV1::Clips,
            SourceUnavailableReasonV1::ProjectionBudgetExceeded,
        ),
    }
    let closure = DependencyClosureBuilderV1::new(
        primary,
        facts.resource_coverage(),
        facts.resource_rows().len(),
    )
    .finish()
    .unwrap();
    let mut document = Document::default();
    for index in 0..stack_count {
        document.clips.push(animsmith_core::Clip {
            name: format!("stack-{index}"),
            duration_s: 0.0,
            tracks: Vec::new(),
        });
    }
    let source = facts
        .finish_with_dependency_closure(document, closure)
        .unwrap();
    match exact {
        Some(exact) => source.with_exact_fbx_timing(exact).unwrap(),
        None => source,
    }
}

fn provenance(source: &animsmith_core::LoadedSource) -> animsmith_core::PredictionProvenanceV3 {
    let profile = resolve_static(EngineDeclaration {
        selection: Some(ProfileSelection::new("unreal", 1, "5.8", "fbx-importer")),
        ..EngineDeclaration::default()
    })
    .unwrap()
    .unwrap()
    .resolve_input_v2_iter(
        SourceFormatV1::Fbx,
        source
            .document()
            .clips
            .iter()
            .map(|clip| clip.name.as_str()),
    )
    .unwrap();
    project_prediction_provenance_v3(&profile, source).unwrap()
}

fn evaluate(source: &animsmith_core::LoadedSource) -> animsmith_core::CheckOutput {
    let provenance = provenance(source);
    let check = EngineClipBoundaryCheck::new(source, Some(&provenance)).unwrap();
    let grids = MetricGrids::new(source.document());
    let roles = ResolvedRoles::default();
    let config = animsmith_core::Config::default();
    check.evaluate(&CheckCtx::new(&grids, &roles, &config))
}

#[test]
fn absolute_end_coordinate_uses_exact_integer_ktime_lattice() {
    let basis = FbxKTimeBasisV1::new(FBX_KTIME_LEGACY_TICKS_PER_SECOND).unwrap();
    let period = FbxFramePeriodV1::for_mode(basis, FbxTimeModeV1::NtscDropFrame)
        .unwrap()
        .ticks_per_frame();
    let source = source(
        Coverage::Complete,
        4,
        Some(timing(
            Coverage::Complete,
            FbxTimeModeV1::NtscDropFrame,
            &[
                StackRange::Observed {
                    begin: 1,
                    end: period * 10,
                },
                StackRange::Observed {
                    begin: 0,
                    end: period * 10 + 1,
                },
                StackRange::Observed {
                    begin: -period * 8,
                    end: -period * 2,
                },
                StackRange::Observed {
                    begin: -period * 8,
                    end: -period * 2 - 1,
                },
            ],
        )),
    );
    let output = evaluate(&source);
    let prediction = output.engine_prediction_v3().unwrap();

    assert_eq!(prediction.facets().len(), 4);
    assert!(prediction.facets().iter().all(|facet| {
        facet.state() == EnginePredictionFacetStateV1::Available
            && facet.scope().code.as_str() == "engine_clip_boundary"
    }));
    let subjects = output
        .findings()
        .iter()
        .map(|finding| {
            finding
                .prediction_scope
                .as_ref()
                .unwrap()
                .subject
                .as_deref()
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(subjects, ["source_stack:1", "source_stack:3"]);
    assert!(
        output.findings()[0]
            .message
            .contains(&format!("{}-tick frame lattice", period))
    );
}

#[test]
fn missing_and_per_row_unavailable_evidence_fail_closed() {
    let without_timing = source(Coverage::Complete, 2, None);
    let output = evaluate(&without_timing);
    assert!(output.findings().is_empty());
    let prediction = output.engine_prediction_v3().unwrap();
    assert_eq!(prediction.facets().len(), 2);
    assert!(prediction.facets().iter().all(|facet| {
        facet.state() == EnginePredictionFacetStateV1::RequiredPredictionUnavailable
            && facet.reasons()[0].as_str() == "animsmith:exact_fbx_timing_unavailable"
    }));

    let basis = FbxKTimeBasisV1::new(FBX_KTIME_LEGACY_TICKS_PER_SECOND).unwrap();
    let period = FbxFramePeriodV1::for_mode(basis, FbxTimeModeV1::Fps30)
        .unwrap()
        .ticks_per_frame();
    let partial_row = source(
        Coverage::Complete,
        2,
        Some(timing(
            Coverage::Complete,
            FbxTimeModeV1::Fps30,
            &[
                StackRange::Observed {
                    begin: 0,
                    end: period,
                },
                StackRange::Unavailable(ExactFbxTimingUnavailableReasonV1::Malformed),
            ],
        )),
    );
    let output = evaluate(&partial_row);
    let facets = output.engine_prediction_v3().unwrap().facets();
    assert_eq!(facets[0].state(), EnginePredictionFacetStateV1::Available);
    assert_eq!(
        facets[1].state(),
        EnginePredictionFacetStateV1::RequiredPredictionUnavailable
    );
    assert_eq!(
        facets[1].reasons()[0].as_str(),
        "animsmith:fbx_stack_tick_range_unavailable"
    );

    let fallback_period = FbxFramePeriodV1::for_mode(
        FbxKTimeBasisV1::new(FBX_KTIME_LEGACY_TICKS_PER_SECOND).unwrap(),
        FbxTimeModeV1::Fps24,
    )
    .unwrap()
    .ticks_per_frame();
    let missing_declaration = source(
        Coverage::Complete,
        1,
        Some(timing_without_declared_mode(fallback_period)),
    );
    let output = evaluate(&missing_declaration);
    let prediction = output.engine_prediction_v3().unwrap();
    let facet = &prediction.facets()[0];
    assert_eq!(
        facet.state(),
        EnginePredictionFacetStateV1::RequiredPredictionUnavailable
    );
    assert_eq!(
        facet.reasons()[0].as_str(),
        "animsmith:fbx_declared_time_mode_unavailable"
    );
}

#[test]
fn retained_partial_rows_are_emitted_beside_inventory_summary() {
    let basis = FbxKTimeBasisV1::new(FBX_KTIME_LEGACY_TICKS_PER_SECOND).unwrap();
    let period = FbxFramePeriodV1::for_mode(basis, FbxTimeModeV1::Fps30)
        .unwrap()
        .ticks_per_frame();
    let source = source(
        Coverage::Partial,
        2,
        Some(timing(
            Coverage::Partial,
            FbxTimeModeV1::Fps30,
            &[
                StackRange::Observed {
                    begin: 0,
                    end: period,
                },
                StackRange::Observed {
                    begin: 0,
                    end: period + 1,
                },
            ],
        )),
    );
    let output = evaluate(&source);
    let facets = output.engine_prediction_v3().unwrap().facets();
    assert_eq!(facets.len(), 3);
    assert_eq!(facets[0].state(), EnginePredictionFacetStateV1::Available);
    assert_eq!(facets[1].state(), EnginePredictionFacetStateV1::Available);
    assert_eq!(
        facets[2].scope().code.as_str(),
        "engine_clip_boundary_inventory"
    );
    assert_eq!(
        facets[2].state(),
        EnginePredictionFacetStateV1::RequiredPredictionUnavailable
    );
    assert_eq!(
        facets[2].reasons(),
        &[animsmith_core::PredictionUnavailableReasonV2::RawSourceIncomplete]
    );
    assert_eq!(output.findings().len(), 1);

    let provenance = provenance(&source);
    let grids = MetricGrids::new(source.document());
    let roles = ResolvedRoles::default();
    let config = animsmith_core::Config::default();
    let checks: [Box<dyn Check + '_>; 1] = [Box::new(
        EngineClipBoundaryCheck::new(&source, Some(&provenance)).unwrap(),
    )];
    let records = animsmith_core::evaluate_checks(
        &CheckCtx::new(&grids, &roles, &config),
        &checks,
        CheckSelection::All,
    )
    .unwrap();
    let allowed = std::collections::BTreeSet::from([ENGINE_CLIP_BOUNDARY_CHECK_ID.to_owned()]);
    assert!(animsmith_core::evaluation::lint_requires_failure(
        &records,
        animsmith_core::Severity::Error,
        &allowed,
    ));
}

#[test]
fn partial_inventory_at_row_limit_uses_n_plus_one_and_keeps_both_summaries() {
    let basis = FbxKTimeBasisV1::new(FBX_KTIME_LEGACY_TICKS_PER_SECOND).unwrap();
    let period = FbxFramePeriodV1::for_mode(basis, FbxTimeModeV1::Fps30)
        .unwrap()
        .ticks_per_frame();
    let ranges = vec![
        StackRange::Observed {
            begin: 0,
            end: period,
        };
        animsmith_core::RAW_SOURCE_V1_MAX_CLIPS
    ];
    let source = source(
        Coverage::Partial,
        ranges.len(),
        Some(timing(Coverage::Partial, FbxTimeModeV1::Fps30, &ranges)),
    );
    let provenance = provenance(&source);
    let check = EngineClipBoundaryCheck::new(&source, Some(&provenance)).unwrap();
    let grids = MetricGrids::new(source.document());
    let roles = ResolvedRoles::default();
    let config = animsmith_core::Config::default();
    let ctx = CheckCtx::new(&grids, &roles, &config);
    assert_eq!(
        check.prediction_facet_demand_v2(&ctx),
        animsmith_core::PredictionFacetDemandV2::NPlusOne
    );
    let output = check.evaluate(&ctx);
    let prediction = output.engine_prediction_v3().unwrap();
    assert_eq!(
        prediction.facets().len(),
        animsmith_core::PREDICTION_V1_MAX_FACETS_PER_FILE
    );
    assert!(prediction.facets().iter().any(|facet| {
        facet.scope().code.as_str() == "engine_clip_boundary_inventory"
            && facet.reasons()
                == [animsmith_core::PredictionUnavailableReasonV2::RawSourceIncomplete]
    }));
    assert!(prediction.facets().iter().any(|facet| {
        facet.scope().code.as_str() == "engine-clip-boundary:facet-budget"
            && facet.reasons()
                == [animsmith_core::PredictionUnavailableReasonV2::FacetBudgetExceeded]
    }));
}

#[test]
fn only_the_frozen_unreal_fbx_profile_is_applicable() {
    let source = source(Coverage::Complete, 1, None);
    let grids = MetricGrids::new(source.document());
    let roles = ResolvedRoles::default();
    let config = animsmith_core::Config::default();
    let ctx = CheckCtx::new(&grids, &roles, &config);
    assert_eq!(
        EngineClipBoundaryCheck::new(&source, None)
            .unwrap()
            .applicability(&ctx),
        animsmith_core::Applicability::NotApplicable
    );

    let provenance = provenance(&source);
    assert_eq!(
        EngineClipBoundaryCheck::new(&source, Some(&provenance))
            .unwrap()
            .applicability(&ctx),
        animsmith_core::Applicability::Applicable
    );
    assert_eq!(ENGINE_CLIP_BOUNDARY_CHECK_ID, "engine-clip-boundary");
}
