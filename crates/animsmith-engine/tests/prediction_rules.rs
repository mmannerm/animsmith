use animsmith_core::config::{CheckSettings, SeveritySetting};
use animsmith_core::{
    Check, CheckCtx, CheckEvaluation, CheckOutput, CheckSelection, DependencyClosureBuilderV1,
    DependencyResourceKeyV1, Document, EnginePredictionBasisV1, EnginePredictionFacetStateV1,
    EnginePredictionFacetV2, EnginePredictionV2, EvaluationScope, EvaluationScopeCode,
    InputIdentity, MetricGrids, PredictionBasisReferenceV1, PredictionFacetDemandV2,
    PredictionProvenanceIdentityV2, PredictionRuleAllocationV2, PredictionUnavailableReasonV1,
    PredictionUnavailableReasonV2, RawSourceBasisReferenceV1, RawSourceDomainV1,
    RawSourceFactsBuilderV1, RawSourceFieldIdV1, RawSourceKeyV1, ResolvedRoles,
    ResourceKeySyntaxV1, SourceClipFactV1, SourceFactDomainV1, SourceFactSetV1, SourceFormatV1,
    SourceLoaderDispositionV1, SourceObservationV1, SourceProvenanceV1, SourceResourceKindV1,
    SourceResourceLocatorV1, SourceResourceReferenceV1, SourceTextV1, SourceUnavailableReasonV1,
};
use animsmith_engine::{
    BevyAnimationAssetLabelError, BevyAnimationAssetLabelV1, ENGINE_ADDRESSABILITY_CHECK_ID,
    ENGINE_CHECK_IDS_V1, EngineAddressabilityCheck, EngineAddressabilityCheckV2,
    EngineAddressabilityCheckV3, EngineDeclaration, GltfAnimationAddressabilityInventoryV1,
    PredictionRuleError, ProfileSelection, build_bevy_animation_addressability_adapter_v1,
    project_prediction_provenance_v1, project_prediction_provenance_v2,
    project_prediction_provenance_v3, resolve_static,
};
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

#[derive(Clone, Copy)]
enum ClipCoverage {
    Complete,
    Partial,
    Unavailable,
}

struct LaterFacetRule {
    identity: PredictionProvenanceIdentityV2,
    allocations: Rc<RefCell<Vec<(usize, bool)>>>,
}

impl Check for LaterFacetRule {
    fn id(&self) -> &'static str {
        "test:later-facet"
    }

    fn evaluate(&self, _ctx: &CheckCtx<'_>) -> CheckOutput {
        panic!("current V2 evaluation must use the allocated hook")
    }

    fn prediction_facet_demand_v2(&self, _ctx: &CheckCtx<'_>) -> PredictionFacetDemandV2 {
        PredictionFacetDemandV2::Exact(1)
    }

    fn evaluate_with_prediction_allocation_v2(
        &self,
        _ctx: &CheckCtx<'_>,
        allocation: PredictionRuleAllocationV2<'_>,
    ) -> CheckOutput {
        self.allocations.borrow_mut().push((
            allocation.candidate_capacity(),
            allocation.summary_required(),
        ));
        let scope = EvaluationScope::new(EvaluationScopeCode::custom("test:later-facet"));
        let basis = EnginePredictionBasisV1::new(vec![
            PredictionBasisReferenceV1::profile_fact("animation_addressability").unwrap(),
        ])
        .unwrap();
        let facet = EnginePredictionFacetV2::available(scope.clone(), basis).unwrap();
        let prediction = EnginePredictionV2::new(self.identity.clone(), vec![facet]).unwrap();
        CheckOutput::from_coverage(Vec::new(), vec![scope], Vec::new())
            .with_engine_prediction_v2(prediction)
    }
}

fn loaded_source(coverage: ClipCoverage, names: &[Option<&str>]) -> animsmith_core::LoadedSource {
    let identity_witness = format!("bevy-addressability:{names:?}");
    let primary = animsmith_core::InputIdentity::from_bytes(identity_witness.as_bytes());
    loaded_source_with_primary(coverage, names, primary)
}

fn loaded_source_with_primary(
    coverage: ClipCoverage,
    names: &[Option<&str>],
    primary: animsmith_core::InputIdentity,
) -> animsmith_core::LoadedSource {
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

fn loaded_source_with_external_resource(external_bytes: &[u8]) -> animsmith_core::LoadedSource {
    let primary = InputIdentity::from_bytes(b"same-primary-and-raw-resource-facts");
    let mut facts = RawSourceFactsBuilderV1::new(SourceFormatV1::GltfJson, primary.clone());
    assert!(facts.push_clip(SourceClipFactV1::new(
        0,
        SourceObservationV1::observed(
            SourceTextV1::new("walk").unwrap(),
            SourceProvenanceV1::format_defined(),
            SourceLoaderDispositionV1::Preserved,
        ),
        SourceObservationV1::observed(
            0,
            SourceProvenanceV1::format_defined(),
            SourceLoaderDispositionV1::Preserved,
        ),
        SourceObservationV1::proven_absent(SourceProvenanceV1::format_defined()),
        SourceObservationV1::proven_absent(SourceProvenanceV1::format_defined()),
        SourceFactSetV1::complete(Vec::new()),
    )));
    assert!(facts.push_resource(SourceResourceReferenceV1::new(
        0,
        SourceResourceKindV1::Buffer,
        0,
        SourceResourceLocatorV1::classify("buffers/animation.bin"),
        SourceLoaderDispositionV1::Preserved,
        SourceProvenanceV1::format_defined(),
    )));
    facts.mark_complete(SourceFactDomainV1::Clips);
    facts.mark_complete(SourceFactDomainV1::Constructs);
    facts.mark_complete(SourceFactDomainV1::Resources);

    let key = DependencyResourceKeyV1::from_source_str(
        "buffers/animation.bin",
        ResourceKeySyntaxV1::GltfUri,
    )
    .unwrap();
    let mut closure = DependencyClosureBuilderV1::new(
        primary,
        facts.resource_coverage(),
        facts.resource_rows().len(),
    );
    assert!(closure.begin_reference(22, 2));
    assert_eq!(closure.prepare_external_key(&key).unwrap(), Some(true));
    closure.record_external_open_attempt(&key).unwrap();
    assert!(
        closure
            .push_captured_external(
                0,
                SourceResourceKindV1::Buffer,
                0,
                key,
                InputIdentity::from_bytes(external_bytes),
            )
            .unwrap()
    );
    let mut document = Document::default();
    document.clips.push(animsmith_core::Clip {
        name: "walk".into(),
        duration_s: 0.0,
        tracks: Vec::new(),
    });
    facts
        .finish_with_dependency_closure(document, closure.finish().unwrap())
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

fn evaluate_record(
    source: &animsmith_core::LoadedSource,
    provenance: &animsmith_core::PredictionProvenanceV1,
) -> CheckEvaluation {
    let grids = MetricGrids::new(source.document());
    let roles = ResolvedRoles::default();
    let config = animsmith_core::Config::default();
    let check: Box<dyn Check + '_> =
        Box::new(EngineAddressabilityCheck::new(source, Some(provenance)).unwrap());
    animsmith_core::evaluate_checks(
        &CheckCtx::new(&grids, &roles, &config),
        &[check],
        CheckSelection::All,
    )
    .unwrap()
    .pop()
    .unwrap()
}

fn assert_same_evaluation(left: &CheckEvaluation, right: &CheckEvaluation) {
    assert_eq!(left.check_id(), right.check_id());
    assert_eq!(left.selection(), right.selection());
    assert_eq!(left.configuration(), right.configuration());
    assert_eq!(left.applicability(), right.applicability());
    assert_eq!(left.evaluation(), right.evaluation());
    assert!(left.findings().is_empty());
    assert!(right.findings().is_empty());
    assert_eq!(left.evaluated_scopes(), right.evaluated_scopes());
    assert_eq!(left.gaps(), right.gaps());
    assert_eq!(left.engine_prediction(), right.engine_prediction());
}

fn assert_adapter_reuses_exact_existing_evaluation(source: &animsmith_core::LoadedSource) {
    let profile = bevy_profile(source);
    let provenance = project_prediction_provenance_v1(&profile, source).unwrap();
    let independently_evaluated = evaluate_record(source, &provenance);
    let inventory = GltfAnimationAddressabilityInventoryV1::from_source(source).unwrap();
    let grids = MetricGrids::new(source.document());
    let roles = ResolvedRoles::default();
    let config = animsmith_core::Config::default();
    let adapter = build_bevy_animation_addressability_adapter_v1(
        source,
        &inventory,
        Some(provenance.clone()),
        &CheckCtx::new(&grids, &roles, &config),
    )
    .unwrap()
    .unwrap();

    assert_eq!(adapter.prediction_provenance(), &provenance);
    assert_same_evaluation(adapter.check(), &independently_evaluated);
}

#[test]
fn bevy_animation_index_rule_emits_one_available_facet_per_complete_source_row() {
    let source = loaded_source(ClipCoverage::Complete, &[Some("walk")]);
    assert!(
        !source.dependency_closure().coverage().is_complete(),
        "the selector prediction is independent of closure completeness"
    );
    let profile = bevy_profile(&source);
    let provenance = project_prediction_provenance_v1(&profile, &source).unwrap();
    let check = EngineAddressabilityCheck::new(&source, Some(&provenance)).unwrap();
    let output = evaluate(&check, &source);

    assert_eq!(ENGINE_CHECK_IDS_V1, &[ENGINE_ADDRESSABILITY_CHECK_ID]);
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
    assert!(!animsmith_core::evaluation::lint_requires_failure(
        &[evaluate_record(&source, &provenance)],
        animsmith_core::Severity::Error,
        &BTreeSet::new(),
    ));
}

#[test]
fn incomplete_animation_inventory_emits_one_required_unavailable_inventory_facet() {
    let source = loaded_source(ClipCoverage::Partial, &[Some("walk")]);
    let profile = bevy_profile(&source);
    let provenance = project_prediction_provenance_v1(&profile, &source).unwrap();
    let check = EngineAddressabilityCheck::new(&source, Some(&provenance)).unwrap();
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
        Box::new(EngineAddressabilityCheck::new(&source, Some(&provenance)).unwrap());
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
    let check = EngineAddressabilityCheck::new(&unavailable, Some(&provenance)).unwrap();
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
    let check = EngineAddressabilityCheck::new(&source, Some(&provenance)).unwrap();
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
        EngineAddressabilityCheck::new(&source, None)
            .unwrap()
            .applicability(&ctx),
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
        EngineAddressabilityCheck::new(&source, Some(&provenance))
            .unwrap()
            .applicability(&ctx),
        animsmith_core::Applicability::NotApplicable
    );

    let other_source = loaded_source(ClipCoverage::Complete, &[Some("other")]);
    let source_profile = bevy_profile(&source);
    let source_provenance = project_prediction_provenance_v1(&source_profile, &source).unwrap();
    assert!(matches!(
        EngineAddressabilityCheck::new(&other_source, Some(&source_provenance)),
        Err(PredictionRuleError::SourceProvenanceMismatch)
    ));
}

#[test]
fn construction_rejects_same_primary_with_different_source_coverage() {
    let primary = animsmith_core::InputIdentity::from_bytes(b"same-primary");
    let complete =
        loaded_source_with_primary(ClipCoverage::Complete, &[Some("walk")], primary.clone());
    let partial =
        loaded_source_with_primary(ClipCoverage::Partial, &[Some("walk")], primary.clone());
    let unavailable =
        loaded_source_with_primary(ClipCoverage::Unavailable, &[Some("walk")], primary);
    let profile = bevy_profile(&partial);
    let partial_provenance = project_prediction_provenance_v1(&profile, &partial).unwrap();

    assert!(matches!(
        EngineAddressabilityCheck::new(&complete, Some(&partial_provenance)),
        Err(PredictionRuleError::SourceProvenanceMismatch)
    ));
    assert!(matches!(
        EngineAddressabilityCheck::new(&unavailable, Some(&partial_provenance)),
        Err(PredictionRuleError::SourceProvenanceMismatch)
    ));
}

#[test]
fn construction_rejects_same_primary_and_raw_facts_with_different_dependency_content() {
    let first = loaded_source_with_external_resource(b"external content one");
    let second = loaded_source_with_external_resource(b"external content two");
    let profile = bevy_profile(&first);
    let provenance = project_prediction_provenance_v1(&profile, &first).unwrap();

    assert_eq!(
        animsmith_core::RawSourceBindingV1::from_source(first.source_facts()),
        animsmith_core::RawSourceBindingV1::from_source(second.source_facts())
    );
    assert_ne!(first.dependency_closure(), second.dependency_closure());
    assert!(matches!(
        EngineAddressabilityCheck::new(&second, Some(&provenance)),
        Err(PredictionRuleError::SourceProvenanceMismatch)
    ));
}

#[test]
fn construction_rejects_an_exact_tuple_with_altered_profile_facts_identity() {
    let source = loaded_source(ClipCoverage::Complete, &[Some("walk")]);
    let profile = bevy_profile(&source);
    let provenance = project_prediction_provenance_v1(&profile, &source).unwrap();
    let altered_sources = provenance
        .profile()
        .primary_sources()
        .iter()
        .map(|source| {
            let url = if source.id() == "bevy-gltf-asset-label-0.19.0" {
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
    let altered_settings = animsmith_core::ResolvedEngineSettingsV1::new(
        &altered_profile,
        provenance.settings().document_settings().to_vec(),
        provenance.settings().clips().to_vec(),
    )
    .unwrap();
    let altered_provenance = animsmith_core::PredictionProvenanceV1::new(
        altered_profile,
        provenance.source_format(),
        altered_settings,
        provenance.raw_source().clone(),
        provenance.dependency_closure().clone(),
    )
    .unwrap();

    assert!(matches!(
        EngineAddressabilityCheck::new(&source, Some(&altered_provenance)),
        Err(PredictionRuleError::FrozenProfileMismatch)
    ));
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
        Box::new(EngineAddressabilityCheck::new(&source, Some(&provenance)).unwrap());

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
    let check = EngineAddressabilityCheck::new(&source, Some(&provenance)).unwrap();
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
        [0, 1, 2]
            .into_iter()
            .map(|index| BevyAnimationAssetLabelV1::new(index).unwrap())
            .map(|label| Some(label.as_str().to_owned()))
            .collect::<Vec<_>>()
            .iter()
            .map(|subject| subject.as_deref())
            .collect::<Vec<_>>()
    );
}

#[test]
fn bevy_animation_asset_label_helper_is_exact_and_bounded() {
    for source_clip_index in 0..animsmith_core::RAW_SOURCE_V1_MAX_CLIPS {
        let label = BevyAnimationAssetLabelV1::new(source_clip_index).unwrap();
        assert_eq!(label.source_clip_index(), source_clip_index);
        assert_eq!(
            label.as_str().as_bytes(),
            format!("Animation{source_clip_index}").as_bytes()
        );
        assert!(label.as_str().len() <= "Animation4095".len());
    }

    assert!(matches!(
        BevyAnimationAssetLabelV1::new(animsmith_core::RAW_SOURCE_V1_MAX_CLIPS),
        Err(BevyAnimationAssetLabelError {
            source_clip_index: 4_096,
            limit: 4_096,
        })
    ));
}

#[test]
fn adapter_reuses_the_exact_existing_evaluation_for_all_inventory_states() {
    assert_adapter_reuses_exact_existing_evaluation(&loaded_source(ClipCoverage::Complete, &[]));
    assert_adapter_reuses_exact_existing_evaluation(&loaded_source(
        ClipCoverage::Complete,
        &[Some("same"), None, Some("same")],
    ));
    assert_adapter_reuses_exact_existing_evaluation(&loaded_source(
        ClipCoverage::Partial,
        &[Some("walk")],
    ));
    assert_adapter_reuses_exact_existing_evaluation(&loaded_source(
        ClipCoverage::Unavailable,
        &[Some("walk")],
    ));
    assert_adapter_reuses_exact_existing_evaluation(&loaded_source(
        ClipCoverage::Complete,
        &vec![Some("clip"); animsmith_core::RAW_SOURCE_V1_MAX_CLIPS],
    ));
}

#[test]
fn adapter_is_absent_without_the_exact_bevy_profile() {
    let source = loaded_source(ClipCoverage::Complete, &[Some("walk")]);
    let inventory = GltfAnimationAddressabilityInventoryV1::from_source(&source).unwrap();
    let grids = MetricGrids::new(source.document());
    let roles = ResolvedRoles::default();
    let config = animsmith_core::Config::default();
    let ctx = CheckCtx::new(&grids, &roles, &config);

    assert!(
        build_bevy_animation_addressability_adapter_v1(&source, &inventory, None, &ctx)
            .unwrap()
            .is_none()
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
    assert!(
        build_bevy_animation_addressability_adapter_v1(
            &source,
            &inventory,
            Some(provenance),
            &ctx,
        )
        .unwrap()
        .is_none()
    );
}

#[test]
fn adapter_preserves_the_existing_disabled_check_lifecycle() {
    let source = loaded_source(ClipCoverage::Complete, &[Some("walk")]);
    let inventory = GltfAnimationAddressabilityInventoryV1::from_source(&source).unwrap();
    let profile = bevy_profile(&source);
    let provenance = project_prediction_provenance_v1(&profile, &source).unwrap();
    let grids = MetricGrids::new(source.document());
    let roles = ResolvedRoles::default();
    let mut config = animsmith_core::Config::default();
    config.checks.insert(
        ENGINE_ADDRESSABILITY_CHECK_ID.to_owned(),
        CheckSettings {
            severity: Some(SeveritySetting::Off),
            ..CheckSettings::default()
        },
    );

    let adapter = build_bevy_animation_addressability_adapter_v1(
        &source,
        &inventory,
        Some(provenance),
        &CheckCtx::new(&grids, &roles, &config),
    )
    .unwrap()
    .expect("an exact Bevy profile retains an adapter even when its check is disabled");
    let check = adapter.check();
    assert_eq!(check.selection(), animsmith_core::SelectionState::Selected);
    assert_eq!(
        check.configuration(),
        animsmith_core::ConfigurationState::Disabled
    );
    assert_eq!(
        check.applicability(),
        animsmith_core::Applicability::Applicable
    );
    assert_eq!(
        check.evaluation(),
        animsmith_core::EvaluationState::NotEvaluated
    );
    assert!(check.findings().is_empty());
    assert!(check.evaluated_scopes().is_empty());
    assert!(check.gaps().is_empty());
    assert!(check.engine_prediction().is_none());
}

#[test]
fn raw_clip_bound_uses_available_facets_at_4096_and_one_partial_inventory_facet() {
    let at_limit = vec![Some("clip"); animsmith_core::RAW_SOURCE_V1_MAX_CLIPS];
    let source = loaded_source(ClipCoverage::Complete, &at_limit);
    let profile = bevy_profile(&source);
    let provenance = project_prediction_provenance_v1(&profile, &source).unwrap();
    let check = EngineAddressabilityCheck::new(&source, Some(&provenance)).unwrap();
    let output = evaluate(&check, &source);
    let facets = output.engine_prediction().unwrap().facets();
    assert_eq!(facets.len(), 4096);
    let mut seen = vec![false; 4096];
    for facet in facets {
        assert_eq!(facet.scope().code.as_str(), "animation_asset_label");
        let subject = facet.scope().subject.as_deref().unwrap();
        let index = subject
            .strip_prefix("Animation")
            .unwrap()
            .parse::<usize>()
            .unwrap();
        let helper = BevyAnimationAssetLabelV1::new(index).unwrap();
        assert_eq!(subject.as_bytes(), helper.as_str().as_bytes());
        assert!(index < seen.len());
        assert!(!std::mem::replace(&mut seen[index], true));
        let expected_raw = RawSourceBasisReferenceV1::from_source(
            RawSourceDomainV1::Clip,
            RawSourceKeyV1::Clip {
                source_clip_index: index as u64,
            },
            RawSourceFieldIdV1::new("source_name.state").unwrap(),
            source.source_facts(),
        )
        .unwrap();
        let expected_basis = EnginePredictionBasisV1::new(vec![
            PredictionBasisReferenceV1::profile_fact("animation_addressability").unwrap(),
            PredictionBasisReferenceV1::primary_source("bevy-gltf-asset-label-0.19.0").unwrap(),
            PredictionBasisReferenceV1::raw_source(expected_raw),
        ])
        .unwrap();
        assert_eq!(facet.basis(), &expected_basis);
    }
    assert!(seen.into_iter().all(|was_seen| was_seen));

    let source = loaded_source(ClipCoverage::Partial, &at_limit);
    let profile = bevy_profile(&source);
    let provenance = project_prediction_provenance_v1(&profile, &source).unwrap();
    let check = EngineAddressabilityCheck::new(&source, Some(&provenance)).unwrap();
    let output = evaluate(&check, &source);
    let prediction = output.engine_prediction().unwrap();
    assert_eq!(prediction.facets().len(), 1);
    assert_eq!(
        prediction.facets()[0].state(),
        EnginePredictionFacetStateV1::RequiredPredictionUnavailable
    );
}

#[test]
fn v2_raw_clip_bound_has_complete_4096_facets_and_n_plus_one_reasons() {
    let clips = vec![Some("clip"); animsmith_core::RAW_SOURCE_V1_MAX_CLIPS];
    let source = loaded_source(ClipCoverage::Complete, &clips);
    let profile = resolve_static(EngineDeclaration {
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
    .resolve_input_v2_iter(SourceFormatV1::GltfJson, std::iter::repeat_n("walk", 4096))
    .unwrap();
    let provenance = project_prediction_provenance_v2(&profile, &source).unwrap();
    let output = evaluate(
        &EngineAddressabilityCheckV2::new(&source, Some(&provenance)).unwrap(),
        &source,
    );
    let prediction = output.engine_prediction_v2().unwrap();
    assert_eq!(prediction.facets().len(), 4096);
    assert!(
        prediction
            .facets()
            .iter()
            .all(|facet| facet.state() == EnginePredictionFacetStateV1::Available)
    );
    CheckEvaluation::evaluated(ENGINE_ADDRESSABILITY_CHECK_ID, output).unwrap();

    let source = loaded_source(ClipCoverage::Partial, &clips);
    let overflow_profile = resolve_static(EngineDeclaration {
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
    .resolve_input_v2_iter(SourceFormatV1::GltfJson, std::iter::repeat_n("walk", 4097))
    .unwrap();
    let provenance = project_prediction_provenance_v2(&overflow_profile, &source).unwrap();
    let output = evaluate(
        &EngineAddressabilityCheckV2::new(&source, Some(&provenance)).unwrap(),
        &source,
    );
    let prediction = output.engine_prediction_v2().unwrap();
    assert_eq!(prediction.facets().len(), 1);
    let facet = &prediction.facets()[0];
    assert_eq!(
        facet.scope().code.as_str(),
        "animation_asset_label_inventory"
    );
    assert!(facet.scope().subject.is_none());
    assert_eq!(
        facet.state(),
        EnginePredictionFacetStateV1::RequiredPredictionUnavailable
    );
    assert_eq!(
        facet
            .reasons()
            .iter()
            .map(|reason| reason.as_str())
            .collect::<Vec<_>>(),
        vec!["raw_source_incomplete", "resolved_settings_overflow"]
    );
}

#[test]
fn v3_addressability_preserves_v2_rule_and_binds_current_raw_source() {
    let source = loaded_source(ClipCoverage::Complete, &[Some("idle"), Some("walk")]);
    let profile = resolve_static(EngineDeclaration {
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
    .resolve_input_v2_iter(SourceFormatV1::GltfJson, ["idle", "walk"].into_iter())
    .unwrap();
    let provenance_v2 = project_prediction_provenance_v2(&profile, &source).unwrap();
    let provenance_v3 = project_prediction_provenance_v3(&profile, &source).unwrap();

    let output_v2 = evaluate(
        &EngineAddressabilityCheckV2::new(&source, Some(&provenance_v2)).unwrap(),
        &source,
    );
    let output_v3 = evaluate(
        &EngineAddressabilityCheckV3::new(&source, Some(&provenance_v3)).unwrap(),
        &source,
    );
    let grids = MetricGrids::new(source.document());
    let roles = ResolvedRoles::default();
    let config = animsmith_core::Config::default();
    let checks: Vec<Box<dyn Check + '_>> = vec![Box::new(
        EngineAddressabilityCheckV3::new(&source, Some(&provenance_v3)).unwrap(),
    )];
    let allocated_v3 = animsmith_core::evaluate_checks_v2(
        &CheckCtx::new(&grids, &roles, &config),
        &checks,
        CheckSelection::All,
    )
    .unwrap();
    let facets_v2 = output_v2.engine_prediction_v2().unwrap().facets();
    let facets_v3 = output_v3.engine_prediction_v3().unwrap().facets();
    assert_eq!(
        allocated_v3[0]
            .engine_prediction_v3()
            .expect("current allocator retains V3 prediction")
            .facets(),
        facets_v3
    );
    assert_eq!(facets_v2.len(), facets_v3.len());
    for (v2, v3) in facets_v2.iter().zip(facets_v3) {
        assert_eq!(v2.scope(), v3.scope());
        assert_eq!(v2.state(), v3.state());
        assert_eq!(v2.reasons(), v3.reasons());
    }
    assert_eq!(
        provenance_v3.raw_source().contract_id(),
        animsmith_core::RAW_SOURCE_FACTS_V2_ID
    );
    assert!(provenance_v3.raw_source().exact_source_timing().is_none());
    assert_eq!(
        provenance_v3.raw_source().primary_input(),
        provenance_v2.raw_source().primary_input()
    );

    let other_source = loaded_source_with_primary(
        ClipCoverage::Complete,
        &[Some("idle"), Some("walk")],
        InputIdentity::from_bytes(b"different-primary"),
    );
    assert!(matches!(
        EngineAddressabilityCheckV3::new(&other_source, Some(&provenance_v3)),
        Err(PredictionRuleError::SourceProvenanceMismatch)
    ));
}

#[test]
fn v2_current_evaluation_allocates_addressability_before_constructing_facets() {
    let clips = vec![Some("clip"); animsmith_core::RAW_SOURCE_V1_MAX_CLIPS];
    let source = loaded_source(ClipCoverage::Complete, &clips);
    let profile = resolve_static(EngineDeclaration {
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
    .resolve_input_v2_iter(SourceFormatV1::GltfJson, std::iter::repeat_n("walk", 4096))
    .unwrap();
    let provenance = project_prediction_provenance_v2(&profile, &source).unwrap();
    let grids = MetricGrids::new(source.document());
    let roles = ResolvedRoles::default();
    let config = animsmith_core::Config::default();
    let allocations = Rc::new(RefCell::new(Vec::new()));
    let checks: Vec<Box<dyn Check + '_>> = vec![
        Box::new(EngineAddressabilityCheckV2::new(&source, Some(&provenance)).unwrap()),
        Box::new(LaterFacetRule {
            identity: provenance.identity().clone(),
            allocations: allocations.clone(),
        }),
    ];

    let ctx = CheckCtx::new(&grids, &roles, &config);
    let first = animsmith_core::evaluate_checks_v2(&ctx, &checks, CheckSelection::All).unwrap();
    let engine = first[0].engine_prediction_v2().unwrap();
    assert_eq!(engine.facets().len(), 4_095);
    assert!(
        engine.facets()[..4_094]
            .iter()
            .all(|facet| facet.state() == EnginePredictionFacetStateV1::Available)
    );
    let summary = &engine.facets()[4_094];
    assert_eq!(
        summary.scope().code.as_str(),
        "engine-addressability:facet-budget"
    );
    assert!(summary.scope().subject.is_none());
    assert_eq!(
        summary.state(),
        EnginePredictionFacetStateV1::RequiredPredictionUnavailable
    );
    assert_eq!(
        summary.reasons(),
        &[PredictionUnavailableReasonV2::FacetBudgetExceeded]
    );
    assert_eq!(allocations.borrow().as_slice(), &[(1, false)]);
    assert_eq!(first[1].engine_prediction_v2().unwrap().facets().len(), 1);
    assert_eq!(
        first
            .iter()
            .filter_map(CheckEvaluation::engine_prediction_v2)
            .map(|prediction| prediction.facets().len())
            .sum::<usize>(),
        4_096
    );
    let first_bytes = serde_json::to_vec(&first).unwrap();

    allocations.borrow_mut().clear();
    let second = animsmith_core::evaluate_checks_v2(&ctx, &checks, CheckSelection::All).unwrap();
    assert_eq!(serde_json::to_vec(&second).unwrap(), first_bytes);
    assert_eq!(allocations.borrow().as_slice(), &[(1, false)]);
}
