use animsmith_core::glam::{Quat, Vec3};
use animsmith_core::measure::LinearTransformClassification;
use animsmith_core::{
    Applicability, Check, CheckCtx, CheckSelection, Config, DependencyClosureBuilderV1, Document,
    EngineMachineResultV1, EnginePredictionFacetStateV1, ImporterSubjectCreationV1, InputIdentity,
    MetricGrids, PredictionBasisReferenceV1, PredictionBasisReferenceV2,
    PredictionBasisReferenceV4, PredictionFacetDemandV2, PredictionInventoryCoverageStateV1,
    PredictionInventoryDomainV1, PredictionUnavailableReasonV2, RawMeshPrimitiveRowV1,
    RawMeshPrimitiveRowsV1, RawNodeMeshAttachmentRowV1, RawNodeMeshAttachmentRowsV1,
    RawPrimitiveTopologyV1, RawSceneAttachmentCoverageV1, RawSceneAttachmentInventoryV1,
    RawSceneRootRowV1, RawSceneRootRowsV1, RawSourceFactsBuilderV1, RawSourceSkeletonEvidenceV1,
    ResolvedRoles, Severity, SourceFactDomainV1, SourceFormatV1, SourceNodeAsset,
    SourceNodeLocalRest, SourceSkeletonCoverage, TransformScaleSubjectKindV1,
};
use animsmith_engine::{
    BevyGltfHandlerEnvironmentV2, BevyLoadMeshesStateV2, ENGINE_UNIT_SCALE_CHECK_ID,
    EngineDeclarationV2, EngineUnitScaleCheck, ProfileSelection, SettingValueV2,
    project_prediction_provenance_v4, resolve_static_v2,
};
use std::collections::{BTreeMap, BTreeSet};

fn source_node(index: usize, parent: Option<usize>, name: &str, scale: Vec3) -> SourceNodeAsset {
    let mut node = SourceNodeAsset::new(
        index,
        SourceNodeLocalRest::Trs {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale,
        },
    );
    node.parent_source_node_index = parent;
    node.name = Some(name.into());
    node
}

fn loaded_source(
    witness: &str,
    nodes: Vec<SourceNodeAsset>,
    scenes: Vec<RawSceneRootRowV1>,
    attachments: Vec<RawNodeMeshAttachmentRowV1>,
    primitives: Vec<RawMeshPrimitiveRowV1>,
) -> animsmith_core::LoadedSource {
    loaded_source_with_coverages(
        witness,
        nodes,
        scenes,
        attachments,
        primitives,
        InventoryCoverages::COMPLETE,
    )
}

#[derive(Clone, Copy)]
struct InventoryCoverages {
    source_skeleton: RawSceneAttachmentCoverageV1,
    scenes: RawSceneAttachmentCoverageV1,
    attachments: RawSceneAttachmentCoverageV1,
    primitives: RawSceneAttachmentCoverageV1,
}

impl InventoryCoverages {
    const COMPLETE: Self = Self {
        source_skeleton: RawSceneAttachmentCoverageV1::Complete,
        scenes: RawSceneAttachmentCoverageV1::Complete,
        attachments: RawSceneAttachmentCoverageV1::Complete,
        primitives: RawSceneAttachmentCoverageV1::Complete,
    };
}

fn loaded_source_with_coverages(
    witness: &str,
    nodes: Vec<SourceNodeAsset>,
    scenes: Vec<RawSceneRootRowV1>,
    attachments: Vec<RawNodeMeshAttachmentRowV1>,
    primitives: Vec<RawMeshPrimitiveRowV1>,
    coverages: InventoryCoverages,
) -> animsmith_core::LoadedSource {
    let primary = InputIdentity::from_bytes(witness.as_bytes());
    let mut facts = RawSourceFactsBuilderV1::new(SourceFormatV1::Glb, primary.clone());
    facts.mark_complete(SourceFactDomainV1::Clips);
    facts.mark_complete(SourceFactDomainV1::Constructs);
    facts.mark_complete(SourceFactDomainV1::Resources);
    let closure = DependencyClosureBuilderV1::new(
        primary.clone(),
        facts.resource_coverage(),
        facts.resource_rows().len(),
    )
    .finish()
    .unwrap();
    let node_count = nodes.len() as u64;
    let mut document = Document::default();
    document.assets.source_skeleton.coverage = SourceSkeletonCoverage::Complete;
    document.assets.source_skeleton.nodes = nodes;
    let source = facts
        .finish_with_dependency_closure(document, closure)
        .unwrap();
    let inventory = RawSceneAttachmentInventoryV1::new(
        primary,
        RawSourceSkeletonEvidenceV1::new(coverages.source_skeleton, node_count, 0),
        RawSceneRootRowsV1::new(coverages.scenes, scenes),
        RawNodeMeshAttachmentRowsV1::new(coverages.attachments, attachments),
        RawMeshPrimitiveRowsV1::new(coverages.primitives, primitives),
    )
    .unwrap();
    source
        .with_raw_scene_attachment_inventory(inventory)
        .unwrap()
}

fn settings(
    load_meshes: BevyLoadMeshesStateV2,
    rotate_scene: bool,
    handler: BevyGltfHandlerEnvironmentV2,
) -> BTreeMap<String, SettingValueV2> {
    BTreeMap::from([
        (
            "bevy_animation_feature".into(),
            SettingValueV2::Boolean(true),
        ),
        (
            "extension_handler_environment".into(),
            SettingValueV2::HandlerEnvironment(handler),
        ),
        (
            "load_meshes".into(),
            SettingValueV2::LoadMeshesState(load_meshes),
        ),
        (
            "rotate_scene_entity".into(),
            SettingValueV2::Boolean(rotate_scene),
        ),
    ])
}

fn provenance(
    source: &animsmith_core::LoadedSource,
    load_meshes: BevyLoadMeshesStateV2,
    rotate_scene: bool,
) -> animsmith_core::PredictionProvenanceV4 {
    provenance_with_handler_and_selectors(
        source,
        load_meshes,
        rotate_scene,
        BevyGltfHandlerEnvironmentV2::BareEmpty,
        vec![],
    )
}

fn provenance_with_handler_and_selectors(
    source: &animsmith_core::LoadedSource,
    load_meshes: BevyLoadMeshesStateV2,
    rotate_scene: bool,
    handler: BevyGltfHandlerEnvironmentV2,
    runtime_node_selectors: Vec<String>,
) -> animsmith_core::PredictionProvenanceV4 {
    let resolved = resolve_static_v2(EngineDeclarationV2 {
        selection: Some(ProfileSelection::new(
            "bevy",
            2,
            "0.19.0",
            "gltf-asset-loader",
        )),
        document_settings: Some(settings(load_meshes, rotate_scene, handler)),
        ..EngineDeclarationV2::default()
    })
    .unwrap()
    .unwrap()
    .resolve_input(SourceFormatV1::Glb)
    .unwrap();
    project_prediction_provenance_v4(&resolved, source, runtime_node_selectors).unwrap()
}

fn evaluate<'a>(
    source: &'a animsmith_core::LoadedSource,
    provenance: &'a animsmith_core::PredictionProvenanceV4,
    config: &'a Config,
) -> animsmith_core::CheckEvaluation {
    let grids = MetricGrids::new(source.document());
    let roles = ResolvedRoles::default();
    let check: Box<dyn Check + '_> =
        Box::new(EngineUnitScaleCheck::new(source, Some(provenance)).unwrap());
    animsmith_core::evaluate_checks_v2(
        &CheckCtx::new(&grids, &roles, config),
        &[check],
        CheckSelection::All,
    )
    .unwrap()
    .pop()
    .unwrap()
}

#[test]
fn complete_reachable_attachment_primitive_cross_product_is_emitted_exactly() {
    let source = loaded_source(
        "cross-product",
        vec![
            source_node(0, None, "Root", Vec3::ONE),
            source_node(1, Some(0), "MeshNode", Vec3::ONE),
        ],
        vec![
            RawSceneRootRowV1::new(0, vec![0]),
            RawSceneRootRowV1::new(1, vec![0]),
        ],
        vec![
            RawNodeMeshAttachmentRowV1::new(0, 0),
            RawNodeMeshAttachmentRowV1::new(1, 0),
        ],
        vec![
            RawMeshPrimitiveRowV1::new(0, 0, RawPrimitiveTopologyV1::Triangles, None),
            RawMeshPrimitiveRowV1::new(0, 1, RawPrimitiveTopologyV1::Lines, None),
        ],
    );
    let created_provenance = provenance(&source, BevyLoadMeshesStateV2::Nonempty, false);
    let check = EngineUnitScaleCheck::new(&source, Some(&created_provenance)).unwrap();
    let grids = MetricGrids::new(source.document());
    let roles = ResolvedRoles::default();
    let config = Config::default();
    assert_eq!(
        check.applicability(&CheckCtx::new(&grids, &roles, &config)),
        Applicability::Applicable
    );
    let record = evaluate(&source, &created_provenance, &Config::default());
    let prediction = record.engine_prediction_v4().unwrap();

    assert!(record.findings().is_empty());
    assert_eq!(prediction.facets().len(), 11); // file + 2 scenes + 2*2*2 children
    let mesh = prediction
        .facets()
        .iter()
        .filter(|facet| facet.scope().code.as_str() == "engine-unit-scale:loader-mesh-primitive")
        .collect::<Vec<_>>();
    assert_eq!(mesh.len(), 8);
    assert!(
        !prediction.facets().iter().any(|facet| {
            facet.scope().code.as_str() == "engine-unit-scale:selected-source-node"
        })
    );
    assert!(mesh.iter().all(|facet| {
        matches!(
            facet.result(),
            Some(EngineMachineResultV1::TransformScale(result))
                if result.subject_kind == TransformScaleSubjectKindV1::LoaderMeshPrimitiveEntity
                    && result.creation == ImporterSubjectCreationV1::Created
                    && result.classification == Some(LinearTransformClassification::UnitOrthonormal)
        )
    }));

    let suppressed_provenance = provenance(&source, BevyLoadMeshesStateV2::Empty, false);
    let suppressed_record = evaluate(&source, &suppressed_provenance, &Config::default());
    let suppressed_meshes = suppressed_record
        .engine_prediction_v4()
        .unwrap()
        .facets()
        .iter()
        .filter(|facet| facet.scope().code.as_str() == "engine-unit-scale:loader-mesh-primitive")
        .collect::<Vec<_>>();
    assert_eq!(suppressed_meshes.len(), 8);
    assert!(suppressed_meshes.iter().all(|facet| {
        matches!(
            facet.result(),
            Some(EngineMachineResultV1::TransformScale(result))
                if result.creation == ImporterSubjectCreationV1::SuppressedBySetting
                    && result.classification.is_none()
        )
    }));
}

fn basis_cites_handler_environment(facet: &animsmith_core::EnginePredictionFacetV4) -> bool {
    facet.basis().references().iter().any(|reference| {
        matches!(
            reference,
            PredictionBasisReferenceV4::V2(PredictionBasisReferenceV2::V1(
                PredictionBasisReferenceV1::ResolvedSetting { setting_id, .. }
            )) if setting_id == "extension_handler_environment"
        )
    })
}

#[test]
fn both_safe_handler_environments_preserve_results_and_are_cited() {
    let source = loaded_source(
        "handler-environments",
        vec![
            source_node(0, None, "Root", Vec3::ONE),
            source_node(1, Some(0), "Socket", Vec3::splat(2.0)),
        ],
        vec![RawSceneRootRowV1::new(0, vec![0])],
        vec![RawNodeMeshAttachmentRowV1::new(1, 0)],
        vec![RawMeshPrimitiveRowV1::new(
            0,
            0,
            RawPrimitiveTopologyV1::Triangles,
            None,
        )],
    );
    let mut config = Config::default();
    config.runtime_nodes.selectors = Some(vec!["Socket".into()]);
    let bare = provenance_with_handler_and_selectors(
        &source,
        BevyLoadMeshesStateV2::Nonempty,
        false,
        BevyGltfHandlerEnvironmentV2::BareEmpty,
        vec!["Socket".into()],
    );
    let pbr = provenance_with_handler_and_selectors(
        &source,
        BevyLoadMeshesStateV2::Nonempty,
        false,
        BevyGltfHandlerEnvironmentV2::BevyPbrStock019,
        vec!["Socket".into()],
    );
    let bare_record = evaluate(&source, &bare, &config);
    let pbr_record = evaluate(&source, &pbr, &config);
    let transform_results = |record: &animsmith_core::CheckEvaluation| {
        record
            .engine_prediction_v4()
            .unwrap()
            .facets()
            .iter()
            .filter_map(|facet| match facet.result() {
                Some(EngineMachineResultV1::TransformScale(result)) => {
                    Some((facet.scope().clone(), result.clone()))
                }
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        transform_results(&bare_record),
        transform_results(&pbr_record)
    );
    for record in [&bare_record, &pbr_record] {
        assert!(
            record
                .engine_prediction_v4()
                .unwrap()
                .facets()
                .iter()
                .all(|facet| !matches!(
                    facet.result(),
                    Some(EngineMachineResultV1::TransformScale(_))
                ) || basis_cites_handler_environment(facet))
        );
    }
}

#[test]
fn incomplete_raw_domains_only_suppress_the_domains_that_consume_them() {
    for (case, coverages, scene_available) in [
        (
            "scenes",
            InventoryCoverages {
                scenes: RawSceneAttachmentCoverageV1::PrefixOverflow,
                ..InventoryCoverages::COMPLETE
            },
            false,
        ),
        (
            "attachments",
            InventoryCoverages {
                attachments: RawSceneAttachmentCoverageV1::PrefixOverflow,
                ..InventoryCoverages::COMPLETE
            },
            true,
        ),
        (
            "primitives",
            InventoryCoverages {
                primitives: RawSceneAttachmentCoverageV1::PrefixOverflow,
                ..InventoryCoverages::COMPLETE
            },
            true,
        ),
    ] {
        let source = loaded_source_with_coverages(
            case,
            vec![
                source_node(0, None, "Root", Vec3::ONE),
                source_node(1, Some(0), "Mesh", Vec3::ONE),
            ],
            vec![RawSceneRootRowV1::new(0, vec![0])],
            vec![RawNodeMeshAttachmentRowV1::new(1, 0)],
            vec![RawMeshPrimitiveRowV1::new(
                0,
                0,
                RawPrimitiveTopologyV1::Triangles,
                None,
            )],
            coverages,
        );
        let provenance = provenance(&source, BevyLoadMeshesStateV2::Empty, false);
        let record = evaluate(&source, &provenance, &Config::default());
        let prediction = record.engine_prediction_v4().unwrap();
        assert_eq!(
            prediction
                .facets()
                .iter()
                .find(|facet| facet.scope().code.as_str() == "engine-unit-scale:file-unit")
                .unwrap()
                .state(),
            EnginePredictionFacetStateV1::Available
        );
        assert_eq!(
            prediction.facets().iter().any(|facet| {
                facet.scope().code.as_str() == "engine-unit-scale:loader-scene-root"
                    && facet.state() == EnginePredictionFacetStateV1::Available
            }),
            scene_available
        );
        let mesh = prediction
            .facets()
            .iter()
            .find(|facet| facet.scope().code.as_str() == "engine-unit-scale:mesh-inventory")
            .unwrap();
        assert_eq!(
            mesh.state(),
            EnginePredictionFacetStateV1::RequiredPredictionUnavailable
        );
        assert_eq!(
            mesh.reasons(),
            &[PredictionUnavailableReasonV2::RawSourceIncomplete]
        );
    }
}

#[test]
fn complete_empty_join_emits_the_joined_subject_inventory_coverage() {
    let source = loaded_source(
        "complete-empty",
        vec![source_node(0, None, "Root", Vec3::ONE)],
        vec![RawSceneRootRowV1::new(0, vec![0])],
        vec![],
        vec![],
    );
    let provenance = provenance(&source, BevyLoadMeshesStateV2::Empty, false);
    let record = evaluate(&source, &provenance, &Config::default());
    let result = record
        .engine_prediction_v4()
        .unwrap()
        .facets()
        .iter()
        .find(|facet| facet.scope().code.as_str() == "engine-unit-scale:mesh-inventory")
        .and_then(|facet| facet.result())
        .unwrap();
    assert!(matches!(
        result,
        EngineMachineResultV1::InventoryCoverage(coverage)
            if coverage.domain == PredictionInventoryDomainV1::LoaderMeshPrimitiveSubjects
                && coverage.coverage == PredictionInventoryCoverageStateV1::Complete
                && coverage.retained_rows == 0
    ));
}

#[test]
fn join_n_plus_one_replaces_the_atomic_mesh_domain_without_a_prefix() {
    let nodes = (0..65)
        .map(|index| {
            source_node(
                index,
                (index != 0).then_some(0),
                &format!("Node{index}"),
                Vec3::ONE,
            )
        })
        .collect::<Vec<_>>();
    let scenes = (0..65)
        .map(|index| RawSceneRootRowV1::new(index, vec![0]))
        .collect();
    let attachments = (0..65)
        .map(|index| RawNodeMeshAttachmentRowV1::new(index, 0))
        .collect();
    let source = loaded_source(
        "join-overflow",
        nodes,
        scenes,
        attachments,
        vec![RawMeshPrimitiveRowV1::new(
            0,
            0,
            RawPrimitiveTopologyV1::Triangles,
            None,
        )],
    );
    let provenance = provenance(&source, BevyLoadMeshesStateV2::Nonempty, false);
    let check = EngineUnitScaleCheck::new(&source, Some(&provenance)).unwrap();
    let grids = MetricGrids::new(source.document());
    let roles = ResolvedRoles::default();
    let config = Config::default();
    let ctx = CheckCtx::new(&grids, &roles, &config);

    // Pair probes and matching primitive expansions stop at 4,096 + 1 work
    // units, so the atomic domain is replaced instead of emitting a prefix.
    assert_eq!(
        check.prediction_facet_demand_v2(&ctx),
        PredictionFacetDemandV2::Exact(67)
    );
    let record = evaluate(&source, &provenance, &config);
    let prediction = record.engine_prediction_v4().unwrap();
    assert_eq!(prediction.facets().len(), 67);
    assert!(
        !prediction.facets().iter().any(|facet| {
            facet.scope().code.as_str() == "engine-unit-scale:loader-mesh-primitive"
        })
    );
    let overflow = prediction
        .facets()
        .iter()
        .find(|facet| facet.scope().code.as_str() == "engine-unit-scale:mesh-inventory")
        .unwrap();
    assert_eq!(
        overflow.reasons()[0].as_str(),
        "animsmith:mesh_join_work_budget_exceeded"
    );
}

#[test]
fn unmatched_deep_join_probes_overflow_without_a_complete_empty_claim() {
    let mut nodes = (0..65)
        .map(|index| {
            source_node(
                index,
                if index == 0 { None } else { Some(index - 1) },
                &format!("Node{index}"),
                Vec3::ONE,
            )
        })
        .collect::<Vec<_>>();
    nodes.extend(
        (65..130).map(|index| source_node(index, None, &format!("Detached{index}"), Vec3::ONE)),
    );
    let source = loaded_source(
        "unmatched-deep-join-overflow",
        nodes,
        (0..65)
            .map(|index| RawSceneRootRowV1::new(index, vec![65 + index]))
            .collect(),
        (0..65)
            .map(|index| RawNodeMeshAttachmentRowV1::new(index, 0))
            .collect(),
        vec![],
    );
    let provenance = provenance(&source, BevyLoadMeshesStateV2::Empty, false);
    let record = evaluate(&source, &provenance, &Config::default());
    let prediction = record.engine_prediction_v4().unwrap();

    // All 4,225 probes are unreachable and each follows the retained parent
    // chain. The planner still stops at N+1 and reports the join-work failure,
    // never a complete-empty claim after an incomplete join.
    assert_eq!(prediction.facets().len(), 67);
    assert!(
        !prediction.facets().iter().any(|facet| {
            facet.scope().code.as_str() == "engine-unit-scale:loader-mesh-primitive"
        })
    );
    let overflow = prediction
        .facets()
        .iter()
        .find(|facet| facet.scope().code.as_str() == "engine-unit-scale:mesh-inventory")
        .unwrap();
    assert_eq!(
        overflow.reasons()[0].as_str(),
        "animsmith:mesh_join_work_budget_exceeded"
    );
}

#[test]
fn selected_nodes_preserve_affine_classification_across_loader_y_rotation() {
    let source = loaded_source(
        "selected-nodes",
        vec![
            source_node(0, None, "Root", Vec3::ONE),
            source_node(1, Some(0), "Socket", Vec3::splat(2.0)),
            source_node(2, Some(0), "Other", Vec3::ONE),
        ],
        vec![
            RawSceneRootRowV1::new(0, vec![0]),
            RawSceneRootRowV1::new(1, vec![0]),
        ],
        vec![],
        vec![],
    );
    let provenance = provenance_with_handler_and_selectors(
        &source,
        BevyLoadMeshesStateV2::Empty,
        true,
        BevyGltfHandlerEnvironmentV2::BareEmpty,
        vec!["Socket".into(), "Missing".into(), "*".into()],
    );
    let mut config = Config::default();
    config.runtime_nodes.selectors = Some(vec!["Socket".into(), "Missing".into(), "*".into()]);
    let record = evaluate(&source, &provenance, &config);
    let prediction = record.engine_prediction_v4().unwrap();

    let selected = prediction
        .facets()
        .iter()
        .filter(|facet| facet.scope().code.as_str() == "engine-unit-scale:selected-source-node")
        .collect::<Vec<_>>();
    assert_eq!(selected.len(), 4);
    let socket = selected
        .iter()
        .filter(|facet| {
            facet
                .scope()
                .subject
                .as_deref()
                .is_some_and(|subject| subject.starts_with("selector:Socket:source_scene:"))
        })
        .collect::<Vec<_>>();
    assert_eq!(socket.len(), 2);
    assert!(socket.iter().all(|facet| {
        facet.state() == EnginePredictionFacetStateV1::Available
            && matches!(
                facet.result(),
                Some(EngineMachineResultV1::TransformScale(result))
                    if result.classification
                        == Some(LinearTransformClassification::UniformScaled)
                        && result.subject_kind == TransformScaleSubjectKindV1::SelectedSourceNode
            )
    }));
    assert_eq!(
        selected
            .iter()
            .find(|facet| facet.scope().subject.as_deref() == Some("selector:Missing"))
            .unwrap()
            .reasons(),
        &[PredictionUnavailableReasonV2::SourceSelectorNoMatch]
    );
    assert_eq!(
        selected
            .iter()
            .find(|facet| facet.scope().subject.as_deref() == Some("selector:*"))
            .unwrap()
            .reasons(),
        &[PredictionUnavailableReasonV2::SourceSelectorAmbiguous]
    );
}

#[test]
fn matrix_authored_selected_ancestry_is_unsuppressibly_unavailable() {
    let mut matrix_child = SourceNodeAsset::new(
        1,
        SourceNodeLocalRest::Matrix(animsmith_core::glam::Mat4::IDENTITY),
    );
    matrix_child.parent_source_node_index = Some(0);
    matrix_child.name = Some("Socket".into());
    let source = loaded_source(
        "matrix-selected",
        vec![source_node(0, None, "Root", Vec3::ONE), matrix_child],
        vec![RawSceneRootRowV1::new(0, vec![0])],
        vec![],
        vec![],
    );
    let provenance = provenance_with_handler_and_selectors(
        &source,
        BevyLoadMeshesStateV2::Empty,
        false,
        BevyGltfHandlerEnvironmentV2::BareEmpty,
        vec!["Socket".into()],
    );
    let mut config = Config::default();
    config.runtime_nodes.selectors = Some(vec!["Socket".into()]);
    let record = evaluate(&source, &provenance, &config);
    let facet = record
        .engine_prediction_v4()
        .unwrap()
        .facets()
        .iter()
        .find(|facet| facet.scope().code.as_str() == "engine-unit-scale:selected-source-node")
        .unwrap();
    assert_eq!(
        facet.state(),
        EnginePredictionFacetStateV1::RequiredPredictionUnavailable
    );
    assert_eq!(
        facet.reasons()[0].as_str(),
        "animsmith:matrix_authored_selected_node_or_ancestry"
    );
    assert!(record.findings().is_empty());
    assert_eq!(record.check_id(), ENGINE_UNIT_SCALE_CHECK_ID);
}

#[test]
fn non_finite_selected_trs_reports_measurement_unavailable() {
    let source = loaded_source(
        "non-finite-selected-trs",
        vec![
            source_node(0, None, "Root", Vec3::ONE),
            source_node(1, Some(0), "Socket", Vec3::new(f32::NAN, 1.0, 1.0)),
        ],
        vec![RawSceneRootRowV1::new(0, vec![0])],
        vec![],
        vec![],
    );
    let provenance = provenance_with_handler_and_selectors(
        &source,
        BevyLoadMeshesStateV2::Empty,
        false,
        BevyGltfHandlerEnvironmentV2::BareEmpty,
        vec!["Socket".into()],
    );
    let mut config = Config::default();
    config.runtime_nodes.selectors = Some(vec!["Socket".into()]);
    let record = evaluate(&source, &provenance, &config);
    let facet = record
        .engine_prediction_v4()
        .unwrap()
        .facets()
        .iter()
        .find(|facet| {
            facet.scope().code.as_str() == "engine-unit-scale:selected-source-node"
                && facet.scope().subject.as_deref()
                    == Some("selector:Socket:source_scene:0:source_node:1")
        })
        .unwrap();
    assert_eq!(
        facet.state(),
        EnginePredictionFacetStateV1::RequiredPredictionUnavailable
    );
    assert_eq!(
        facet.reasons(),
        &[PredictionUnavailableReasonV2::MeasurementUnavailable]
    );
}

#[test]
fn selected_node_reachability_at_128_nodes_remains_available() {
    let nodes = (0..128)
        .map(|index| {
            source_node(
                index,
                if index == 0 { None } else { Some(index - 1) },
                if index == 127 { "Deep" } else { "Ancestor" },
                Vec3::ONE,
            )
        })
        .collect::<Vec<_>>();
    let source = loaded_source(
        "selected-reachability-limit",
        nodes,
        vec![RawSceneRootRowV1::new(0, vec![0])],
        vec![],
        vec![],
    );
    let provenance = provenance_with_handler_and_selectors(
        &source,
        BevyLoadMeshesStateV2::Empty,
        false,
        BevyGltfHandlerEnvironmentV2::BareEmpty,
        vec!["Deep".into()],
    );
    let mut config = Config::default();
    config.runtime_nodes.selectors = Some(vec!["Deep".into()]);
    let record = evaluate(&source, &provenance, &config);
    let facet = record
        .engine_prediction_v4()
        .unwrap()
        .facets()
        .iter()
        .find(|facet| {
            facet.scope().subject.as_deref() == Some("selector:Deep:source_scene:0:source_node:127")
        })
        .expect("the selected node is reachable within the exact bound");
    assert_eq!(facet.state(), EnginePredictionFacetStateV1::Available);
}

#[test]
fn selected_node_reachability_over_128_nodes_is_unavailable_without_prefix() {
    let nodes = (0..=128)
        .map(|index| {
            source_node(
                index,
                if index == 0 { None } else { Some(index - 1) },
                if index == 128 { "Deep" } else { "Ancestor" },
                Vec3::ONE,
            )
        })
        .collect::<Vec<_>>();
    let source = loaded_source(
        "selected-unavailable",
        nodes,
        vec![RawSceneRootRowV1::new(0, vec![0])],
        vec![],
        vec![],
    );
    let provenance = provenance_with_handler_and_selectors(
        &source,
        BevyLoadMeshesStateV2::Empty,
        false,
        BevyGltfHandlerEnvironmentV2::BareEmpty,
        vec!["Deep".into()],
    );
    let mut config = Config::default();
    config.runtime_nodes.selectors = Some(vec!["Deep".into()]);
    let record = evaluate(&source, &provenance, &config);
    let selected = record
        .engine_prediction_v4()
        .unwrap()
        .facets()
        .iter()
        .filter(|facet| facet.scope().code.as_str() == "engine-unit-scale:selected-source-node")
        .collect::<Vec<_>>();
    assert_eq!(selected.len(), 1);
    let facet = selected[0];
    assert_eq!(facet.scope().subject.as_deref(), Some("selector:Deep"));
    assert_eq!(
        facet.state(),
        EnginePredictionFacetStateV1::RequiredPredictionUnavailable
    );
    assert_eq!(
        facet.reasons()[0].as_str(),
        "animsmith:selected_node_scene_reachability_unavailable"
    );
    assert!(animsmith_core::lint_requires_failure(
        &[record],
        Severity::Error,
        &BTreeSet::from([ENGINE_UNIT_SCALE_CHECK_ID.to_owned()]),
    ));
}

#[test]
fn selected_node_reachability_work_budget_accepts_n_and_refuses_n_plus_one_without_prefix() {
    let nodes = (0..128)
        .map(|index| {
            source_node(
                index,
                if index == 0 { None } else { Some(index - 1) },
                if index == 127 { "Deep" } else { "Ancestor" },
                Vec3::ONE,
            )
        })
        .collect::<Vec<_>>();
    let evaluate_scenes = |scene_count| {
        let source = loaded_source(
            "selected-work",
            nodes.clone(),
            (0..scene_count)
                .map(|index| RawSceneRootRowV1::new(index, vec![0]))
                .collect(),
            vec![],
            vec![],
        );
        let provenance = provenance_with_handler_and_selectors(
            &source,
            BevyLoadMeshesStateV2::Empty,
            false,
            BevyGltfHandlerEnvironmentV2::BareEmpty,
            vec!["Deep".into()],
        );
        let mut config = Config::default();
        config.runtime_nodes.selectors = Some(vec!["Deep".into()]);
        evaluate(&source, &provenance, &config)
    };
    let at_limit = evaluate_scenes(32);
    let at_limit_selected = at_limit
        .engine_prediction_v4()
        .unwrap()
        .facets()
        .iter()
        .filter(|facet| facet.scope().code.as_str() == "engine-unit-scale:selected-source-node")
        .collect::<Vec<_>>();
    assert_eq!(at_limit_selected.len(), 32);
    assert!(at_limit_selected.iter().all(|facet| {
        facet.state() == EnginePredictionFacetStateV1::Available
            && facet
                .scope()
                .subject
                .as_deref()
                .is_some_and(|subject| subject.contains("source_scene:"))
    }));

    let over_limit = evaluate_scenes(33);
    let over_limit_selected = over_limit
        .engine_prediction_v4()
        .unwrap()
        .facets()
        .iter()
        .filter(|facet| facet.scope().code.as_str() == "engine-unit-scale:selected-source-node")
        .collect::<Vec<_>>();
    assert_eq!(over_limit_selected.len(), 1);
    assert_eq!(
        over_limit_selected[0].scope().subject.as_deref(),
        Some("selector:Deep")
    );
    assert_eq!(
        over_limit_selected[0].reasons()[0].as_str(),
        "animsmith:selected_node_scene_reachability_unavailable"
    );
}
