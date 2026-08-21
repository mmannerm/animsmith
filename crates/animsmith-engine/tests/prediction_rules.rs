use animsmith_core::{
    Check, CheckCtx, CheckSelection, Document, EnginePredictionBasisV1,
    EnginePredictionFacetStateV1, MetricGrids, PredictionBasisReferenceV1,
    PredictionUnavailableReasonV1, RawSourceFactsBuilderV1, ResolvedRoles, SourceClipFactV1,
    SourceFactDomainV1, SourceFactSetV1, SourceFormatV1, SourceLoaderDispositionV1,
    SourceObservationV1, SourceProvenanceV1, SourceTextV1, SourceUnavailableReasonV1,
};
use animsmith_engine::{
    AnimationAssetLabelCheck, ENGINE_ADDRESSABILITY_CHECK_ID, ENGINE_CHECK_IDS_V1,
    EngineDeclaration, ProfileSelection, project_prediction_provenance_v1, resolve_static,
};
use std::collections::BTreeSet;

#[derive(Clone, Copy)]
enum ClipCoverage {
    Complete,
    Partial,
    Unavailable,
}

fn loaded_source(coverage: ClipCoverage, names: &[Option<&str>]) -> animsmith_core::LoadedSource {
    let identity_witness = format!("bevy-addressability:{names:?}");
    let primary = animsmith_core::InputIdentity::from_bytes(identity_witness.as_bytes());
    let mut facts = RawSourceFactsBuilderV1::new(SourceFormatV1::GltfJson, primary.clone());
    if !matches!(coverage, ClipCoverage::Unavailable) {
        for (index, name) in names.iter().enumerate() {
            if !facts.push_clip(SourceClipFactV1::new(
                index,
                match name {
                    Some(name) => SourceObservationV1::observed(
                        SourceTextV1::new(name).unwrap(),
                        SourceProvenanceV1::format_defined(),
                        SourceLoaderDispositionV1::Preserved,
                    ),
                    None => {
                        SourceObservationV1::proven_absent(SourceProvenanceV1::format_defined())
                    }
                },
                SourceObservationV1::observed(
                    index,
                    SourceProvenanceV1::format_defined(),
                    SourceLoaderDispositionV1::Preserved,
                ),
                SourceObservationV1::proven_absent(SourceProvenanceV1::format_defined()),
                SourceObservationV1::proven_absent(SourceProvenanceV1::format_defined()),
                SourceFactSetV1::complete(Vec::new()),
            )) {
                break;
            }
        }
    }
    match coverage {
        ClipCoverage::Complete => facts.mark_complete(SourceFactDomainV1::Clips),
        ClipCoverage::Partial => facts.mark_partial(
            SourceFactDomainV1::Clips,
            SourceUnavailableReasonV1::ProjectionBudgetExceeded,
        ),
        ClipCoverage::Unavailable => {}
    }
    let closure = animsmith_core::DependencyClosureBuilderV1::new(
        primary,
        facts.resource_coverage(),
        facts.resource_rows().len(),
    )
    .finish()
    .unwrap();
    let mut document = Document::default();
    for name in names {
        document.clips.push(animsmith_core::Clip {
            name: name.unwrap_or("unnamed").into(),
            duration_s: 0.0,
            tracks: Vec::new(),
        });
    }
    facts
        .finish_with_dependency_closure(document, closure)
        .unwrap()
}

fn bevy_profile(source: &animsmith_core::LoadedSource) -> animsmith_engine::ResolvedProfile {
    resolve_static(EngineDeclaration {
        selection: Some(ProfileSelection::new(
            "bevy",
            1,
            "0.19.0",
            "gltf-asset-loader",
        )),
        ..EngineDeclaration::default()
    })
    .unwrap()
    .unwrap()
    .resolve_input(
        SourceFormatV1::GltfJson,
        &source
            .document()
            .clips
            .iter()
            .map(|clip| clip.name.clone())
            .collect::<Vec<_>>(),
    )
    .unwrap()
}

fn evaluate(
    check: &dyn Check,
    source: &animsmith_core::LoadedSource,
) -> animsmith_core::CheckOutput {
    let grids = MetricGrids::new(source.document());
    let roles = ResolvedRoles::default();
    let config = animsmith_core::Config::default();
    check.evaluate(&CheckCtx::new(&grids, &roles, &config))
}

#[test]
fn bevy_animation_index_rule_emits_one_available_facet_per_complete_source_row() {
    let source = loaded_source(ClipCoverage::Complete, &[Some("walk")]);
    let profile = bevy_profile(&source);
    let provenance = project_prediction_provenance_v1(&profile, &source).unwrap();
    let check = AnimationAssetLabelCheck::new(&source, Some(&provenance));
    let output = evaluate(&check, &source);

    assert_eq!(ENGINE_CHECK_IDS_V1, &[ENGINE_ADDRESSABILITY_CHECK_ID]);
    assert_eq!(
        ENGINE_CHECK_IDS_V1,
        animsmith_core::evaluation::EXTERNAL_BUILTIN_CHECK_IDS,
        "engine catalog and core-owned evidence-emitter vocabulary must agree"
    );
    assert!(output.findings().is_empty());
    assert_eq!(output.evaluated_scopes().len(), 1);
    assert_eq!(
        output.evaluated_scopes()[0].code.as_str(),
        "animation_asset_label"
    );
    assert_eq!(
        output.evaluated_scopes()[0].subject.as_deref(),
        Some("Animation0")
    );
    let prediction = output.engine_prediction().unwrap();
    prediction.validate_against_provenance(&provenance).unwrap();
    assert_eq!(prediction.facets().len(), 1);
    assert_eq!(
        prediction.facets()[0].state(),
        EnginePredictionFacetStateV1::Available
    );
}

#[test]
fn incomplete_animation_inventory_emits_one_required_unavailable_inventory_facet() {
    let source = loaded_source(ClipCoverage::Partial, &[Some("walk")]);
    let profile = bevy_profile(&source);
    let provenance = project_prediction_provenance_v1(&profile, &source).unwrap();
    let check = AnimationAssetLabelCheck::new(&source, Some(&provenance));
    let output = evaluate(&check, &source);

    assert!(output.findings().is_empty());
    assert!(output.evaluated_scopes().is_empty());
    let prediction = output.engine_prediction().unwrap();
    assert_eq!(prediction.facets().len(), 1);
    assert_eq!(
        prediction.facets()[0].scope().code.as_str(),
        "animation_asset_label_inventory"
    );
    assert_eq!(
        prediction.facets()[0].state(),
        EnginePredictionFacetStateV1::RequiredPredictionUnavailable
    );
    assert_eq!(prediction.facets()[0].scope().subject.as_deref(), None);
    assert_eq!(
        prediction.facets()[0].reasons(),
        &[PredictionUnavailableReasonV1::RawSourceIncomplete]
    );
    let expected_basis = EnginePredictionBasisV1::new(vec![
        PredictionBasisReferenceV1::profile_fact("animation_addressability").unwrap(),
        PredictionBasisReferenceV1::primary_source("bevy-gltf-asset-label-0.19.0").unwrap(),
    ])
    .unwrap();
    assert_eq!(
        prediction.facets()[0].basis().references(),
        expected_basis.references()
    );

    let roles = ResolvedRoles::default();
    let config = animsmith_core::Config::default();
    let grids = MetricGrids::new(source.document());
    let check: Box<dyn Check + '_> =
        Box::new(AnimationAssetLabelCheck::new(&source, Some(&provenance)));
    let records = animsmith_core::evaluate_checks(
        &CheckCtx::new(&grids, &roles, &config),
        &[check],
        CheckSelection::All,
    )
    .unwrap();
    assert_eq!(
        records[0].evaluation(),
        animsmith_core::EvaluationState::NotEvaluated
    );
    let allowed = BTreeSet::from([ENGINE_ADDRESSABILITY_CHECK_ID.to_owned()]);
    assert!(animsmith_core::evaluation::lint_requires_failure(
        &records,
        animsmith_core::Severity::Error,
        &allowed,
    ));

    let unavailable = loaded_source(ClipCoverage::Unavailable, &[Some("walk")]);
    let profile = bevy_profile(&unavailable);
    let provenance = project_prediction_provenance_v1(&profile, &unavailable).unwrap();
    let check = AnimationAssetLabelCheck::new(&unavailable, Some(&provenance));
    let output = evaluate(&check, &unavailable);
    assert_eq!(
        output.engine_prediction().unwrap().facets()[0].state(),
        EnginePredictionFacetStateV1::RequiredPredictionUnavailable
    );
}

#[test]
fn complete_empty_bevy_animation_inventory_is_not_applicable() {
    let source = loaded_source(ClipCoverage::Complete, &[]);
    let profile = bevy_profile(&source);
    let provenance = project_prediction_provenance_v1(&profile, &source).unwrap();
    let check = AnimationAssetLabelCheck::new(&source, Some(&provenance));
    let grids = MetricGrids::new(source.document());
    let roles = ResolvedRoles::default();
    let config = animsmith_core::Config::default();

    assert_eq!(
        check.applicability(&CheckCtx::new(&grids, &roles, &config)),
        animsmith_core::Applicability::NotApplicable
    );
}

#[test]
fn absent_or_non_bevy_profile_has_a_stable_not_applicable_record() {
    let source = loaded_source(ClipCoverage::Complete, &[Some("walk")]);
    let grids = MetricGrids::new(source.document());
    let roles = ResolvedRoles::default();
    let config = animsmith_core::Config::default();
    let ctx = CheckCtx::new(&grids, &roles, &config);
    assert_eq!(
        AnimationAssetLabelCheck::new(&source, None).applicability(&ctx),
        animsmith_core::Applicability::NotApplicable
    );

    let profile = resolve_static(EngineDeclaration {
        selection: Some(ProfileSelection::new(
            "godot",
            1,
            "4.7",
            "resource-importer-scene",
        )),
        ..EngineDeclaration::default()
    })
    .unwrap()
    .unwrap()
    .resolve_input(SourceFormatV1::GltfJson, &["walk".into()])
    .unwrap();
    let provenance = project_prediction_provenance_v1(&profile, &source).unwrap();
    assert_eq!(
        AnimationAssetLabelCheck::new(&source, Some(&provenance)).applicability(&ctx),
        animsmith_core::Applicability::NotApplicable
    );

    let other_source = loaded_source(ClipCoverage::Complete, &[Some("other")]);
    let grids = MetricGrids::new(other_source.document());
    let other_ctx = CheckCtx::new(&grids, &roles, &config);
    let source_profile = bevy_profile(&source);
    let source_provenance = project_prediction_provenance_v1(&source_profile, &source).unwrap();
    assert_eq!(
        AnimationAssetLabelCheck::new(&other_source, Some(&source_provenance))
            .applicability(&other_ctx),
        animsmith_core::Applicability::NotApplicable,
        "a prediction check cannot combine evidence from two primary inputs"
    );
}

#[test]
fn default_catalog_lifecycle_selects_and_enables_the_borrowed_bevy_check() {
    let source = loaded_source(ClipCoverage::Complete, &[Some("walk")]);
    let profile = bevy_profile(&source);
    let provenance = project_prediction_provenance_v1(&profile, &source).unwrap();
    let grids = MetricGrids::new(source.document());
    let roles = ResolvedRoles::default();
    let config = animsmith_core::Config::default();
    let check: Box<dyn Check + '_> =
        Box::new(AnimationAssetLabelCheck::new(&source, Some(&provenance)));

    let records = animsmith_core::evaluate_checks(
        &CheckCtx::new(&grids, &roles, &config),
        &[check],
        CheckSelection::All,
    )
    .unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].selection(),
        animsmith_core::SelectionState::Selected
    );
    assert_eq!(
        records[0].configuration(),
        animsmith_core::ConfigurationState::Enabled
    );
    assert_eq!(
        records[0].applicability(),
        animsmith_core::Applicability::Applicable
    );
    assert_eq!(
        records[0].evaluation(),
        animsmith_core::EvaluationState::Complete
    );
}

#[test]
fn names_do_not_affect_source_index_label_subjects() {
    let source = loaded_source(ClipCoverage::Complete, &[Some("same"), None, Some("same")]);
    let profile = bevy_profile(&source);
    let provenance = project_prediction_provenance_v1(&profile, &source).unwrap();
    let check = AnimationAssetLabelCheck::new(&source, Some(&provenance));
    let output = evaluate(&check, &source);
    let subjects = output
        .engine_prediction()
        .unwrap()
        .facets()
        .iter()
        .map(|facet| facet.scope().subject.as_deref())
        .collect::<Vec<_>>();
    assert_eq!(
        subjects,
        vec![Some("Animation0"), Some("Animation1"), Some("Animation2")]
    );
}

#[test]
fn raw_clip_bound_uses_available_facets_at_4096_and_one_partial_inventory_facet() {
    let at_limit = vec![Some("clip"); animsmith_core::RAW_SOURCE_V1_MAX_CLIPS];
    let source = loaded_source(ClipCoverage::Complete, &at_limit);
    let profile = bevy_profile(&source);
    let provenance = project_prediction_provenance_v1(&profile, &source).unwrap();
    let check = AnimationAssetLabelCheck::new(&source, Some(&provenance));
    assert_eq!(
        evaluate(&check, &source)
            .engine_prediction()
            .unwrap()
            .facets()
            .len(),
        4096
    );

    let source = loaded_source(ClipCoverage::Partial, &at_limit);
    let profile = bevy_profile(&source);
    let provenance = project_prediction_provenance_v1(&profile, &source).unwrap();
    let check = AnimationAssetLabelCheck::new(&source, Some(&provenance));
    let output = evaluate(&check, &source);
    let prediction = output.engine_prediction().unwrap();
    assert_eq!(prediction.facets().len(), 1);
    assert_eq!(
        prediction.facets()[0].state(),
        EnginePredictionFacetStateV1::RequiredPredictionUnavailable
    );
}
