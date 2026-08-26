use animsmith_core::{
    Check, CheckCtx, CheckSelection, DependencyClosureBuilderV1, Document,
    EnginePredictionFacetStateV1, ExactSourceClipTimeRangeV1, ExactSourceClipTimingV1,
    ExactSourceFramePeriodV1, ExactSourceRangeSelectionV1, ExactSourceTimeBasisV1,
    ExactSourceTimingObservationV1, ExactSourceTimingUnavailableReasonV1, ExactSourceTimingV1,
    InputIdentity, MetricGrids, RawSourceFactsBuilderV1, ResolvedRoles, SourceClipFactV1,
    SourceFactDomainV1, SourceFactSetV1, SourceFormatV1, SourceLoaderDispositionV1,
    SourceObservationV1, SourceProvenanceV1, SourceSetCoverageV1, SourceTextV1,
    SourceTimeDisplayProtocolV1, SourceTimelineModeV1, SourceUnavailableReasonV1,
};
use animsmith_engine::{
    ENGINE_CLIP_BOUNDARY_CHECK_ID, EngineClipBoundaryCheck, EngineDeclaration, PredictionRuleError,
    ProfileSelection, project_prediction_provenance_v3, resolve_static,
};

#[derive(Clone, Copy)]
enum Coverage {
    Complete,
    Partial,
}

#[derive(Clone, Copy)]
enum StackRange {
    Observed { begin: i64, end: i64 },
    Unavailable(ExactSourceTimingUnavailableReasonV1),
}

const TEST_SOURCE_UNITS_PER_SECOND: i64 = 46_186_158_000;

fn source_period(mode: SourceTimelineModeV1) -> ExactSourceFramePeriodV1 {
    let units = match mode {
        SourceTimelineModeV1::Fps24 => TEST_SOURCE_UNITS_PER_SECOND / 24,
        SourceTimelineModeV1::Fps30 => TEST_SOURCE_UNITS_PER_SECOND / 30,
        SourceTimelineModeV1::NtscDropFrame => (TEST_SOURCE_UNITS_PER_SECOND / 30 * 1001) / 1000,
        _ => TEST_SOURCE_UNITS_PER_SECOND / 30,
    };
    ExactSourceFramePeriodV1::new(units).unwrap()
}

fn exact_observed<T>(value: T) -> ExactSourceTimingObservationV1<T> {
    ExactSourceTimingObservationV1::observed(
        value,
        SourceProvenanceV1::format_defined(),
        SourceLoaderDispositionV1::Preserved,
    )
}

fn exact_unavailable<T>(
    reason: ExactSourceTimingUnavailableReasonV1,
) -> ExactSourceTimingObservationV1<T> {
    ExactSourceTimingObservationV1::unavailable(
        reason,
        Some(SourceProvenanceV1::format_defined()),
        SourceLoaderDispositionV1::Unknown,
    )
}

fn timing(
    coverage: Coverage,
    mode: SourceTimelineModeV1,
    ranges: &[StackRange],
) -> ExactSourceTimingV1 {
    let basis = ExactSourceTimeBasisV1::new(TEST_SOURCE_UNITS_PER_SECOND).unwrap();
    let period = source_period(mode);
    ExactSourceTimingV1::new(
        exact_observed(basis),
        exact_observed(mode),
        exact_observed(mode),
        ExactSourceTimingObservationV1::proven_absent(SourceProvenanceV1::format_defined()),
        exact_observed(period),
        ExactSourceTimingObservationV1::proven_absent(SourceProvenanceV1::format_defined()),
        exact_observed(SourceTimeDisplayProtocolV1::Default),
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
                        ExactSourceClipTimeRangeV1::new(
                            ExactSourceRangeSelectionV1::Primary,
                            begin,
                            end,
                        )
                        .unwrap(),
                    ),
                    StackRange::Unavailable(reason) => exact_unavailable(reason),
                };
                ExactSourceClipTimingV1::new(index, range)
            })
            .collect(),
    )
    .unwrap()
}

fn timing_without_declared_mode(end_units: i64) -> ExactSourceTimingV1 {
    let basis = ExactSourceTimeBasisV1::new(TEST_SOURCE_UNITS_PER_SECOND).unwrap();
    let mode = SourceTimelineModeV1::Fps24;
    let period = source_period(mode);
    ExactSourceTimingV1::new(
        exact_observed(basis),
        ExactSourceTimingObservationV1::proven_absent(SourceProvenanceV1::format_defined()),
        exact_observed(mode),
        ExactSourceTimingObservationV1::proven_absent(SourceProvenanceV1::format_defined()),
        exact_observed(period),
        ExactSourceTimingObservationV1::proven_absent(SourceProvenanceV1::format_defined()),
        exact_observed(SourceTimeDisplayProtocolV1::Default),
        SourceSetCoverageV1::complete(),
        vec![ExactSourceClipTimingV1::new(
            0,
            exact_observed(
                ExactSourceClipTimeRangeV1::new(ExactSourceRangeSelectionV1::Primary, 0, end_units)
                    .unwrap(),
            ),
        )],
    )
    .unwrap()
}

fn source(
    coverage: Coverage,
    stack_count: usize,
    exact: Option<ExactSourceTimingV1>,
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
        Some(exact) => source.with_exact_source_timing(exact).unwrap(),
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
fn absolute_end_coordinate_uses_exact_integer_source_time_lattice() {
    let period = source_period(SourceTimelineModeV1::NtscDropFrame).units_per_frame();
    let original = source(
        Coverage::Complete,
        4,
        Some(timing(
            Coverage::Complete,
            SourceTimelineModeV1::NtscDropFrame,
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
    let output = evaluate(&original);
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
            && facet.reasons()[0].as_str() == "animsmith:exact_source_timing_unavailable"
    }));

    let period = source_period(SourceTimelineModeV1::Fps30).units_per_frame();
    let partial_row = source(
        Coverage::Complete,
        2,
        Some(timing(
            Coverage::Complete,
            SourceTimelineModeV1::Fps30,
            &[
                StackRange::Observed {
                    begin: 0,
                    end: period,
                },
                StackRange::Unavailable(ExactSourceTimingUnavailableReasonV1::Malformed),
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
        "animsmith:source_clip_time_range_unavailable"
    );

    let fallback_period = source_period(SourceTimelineModeV1::Fps24).units_per_frame();
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
        "animsmith:source_declared_time_mode_unavailable"
    );
}

#[test]
fn retained_partial_rows_are_emitted_beside_inventory_summary() {
    let period = source_period(SourceTimelineModeV1::Fps30).units_per_frame();
    let source = source(
        Coverage::Partial,
        2,
        Some(timing(
            Coverage::Partial,
            SourceTimelineModeV1::Fps30,
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
    let period = source_period(SourceTimelineModeV1::Fps30).units_per_frame();
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
        Some(timing(
            Coverage::Partial,
            SourceTimelineModeV1::Fps30,
            &ranges,
        )),
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

#[test]
fn non_exact_unreal_tuples_do_not_resolve_the_boundary_profile() {
    for selection in [
        ProfileSelection::new("unreal", 2, "5.8", "fbx-importer"),
        ProfileSelection::new("unreal", 1, "5.7", "fbx-importer"),
        ProfileSelection::new("unreal", 1, "5.8", "interchange-importer"),
        ProfileSelection::new("unity", 1, "5.8", "fbx-importer"),
    ] {
        assert!(
            resolve_static(EngineDeclaration {
                selection: Some(selection),
                ..EngineDeclaration::default()
            })
            .is_err()
        );
    }
}

#[test]
fn construction_rejects_same_load_timing_and_frozen_profile_mismatches() {
    let period = source_period(SourceTimelineModeV1::Fps24).units_per_frame();
    let original = source(
        Coverage::Complete,
        1,
        Some(timing(
            Coverage::Complete,
            SourceTimelineModeV1::Fps24,
            &[StackRange::Observed {
                begin: 0,
                end: period,
            }],
        )),
    );
    let provenance = provenance(&original);
    let changed_timing = source(
        Coverage::Complete,
        1,
        Some(timing(
            Coverage::Complete,
            SourceTimelineModeV1::Fps24,
            &[StackRange::Observed {
                begin: 0,
                end: period + 1,
            }],
        )),
    );
    assert!(matches!(
        EngineClipBoundaryCheck::new(&changed_timing, Some(&provenance)),
        Err(PredictionRuleError::SourceProvenanceMismatch)
    ));

    let altered_sources = provenance
        .profile()
        .primary_sources()
        .iter()
        .map(|source| {
            let url = if source.id() == "unreal-animation-sequences-5.8" {
                format!("{}#altered", source.url())
            } else {
                source.url().to_owned()
            };
            animsmith_core::EnginePrimarySourceV1::new(
                source.id(),
                source.target_version(),
                url,
                source.verified_on(),
                source.supported_fact_ids().to_vec(),
                source.supported_setting_ids().to_vec(),
            )
            .unwrap()
        })
        .collect();
    let altered_profile = animsmith_core::ResolvedEngineProfileV1::new(
        provenance.profile().selection().clone(),
        provenance.profile().fact_bundle_urn(),
        provenance.profile().facts().to_vec(),
        provenance.profile().setting_descriptors().to_vec(),
        altered_sources,
    )
    .unwrap();
    let altered_settings = animsmith_core::ResolvedEngineSettingsV2::new(
        &altered_profile,
        provenance.settings().document_settings().to_vec(),
        provenance.settings().clips().to_vec(),
        provenance.settings().clip_coverage().clone(),
        *provenance.settings().work(),
    )
    .unwrap();
    let altered_provenance = animsmith_core::PredictionProvenanceV3::new(
        altered_profile,
        provenance.source_format(),
        altered_settings,
        provenance.raw_source().clone(),
        provenance.dependency_closure().clone(),
    )
    .unwrap();
    assert!(matches!(
        EngineClipBoundaryCheck::new(&original, Some(&altered_provenance)),
        Err(PredictionRuleError::FrozenProfileMismatch)
    ));
}
