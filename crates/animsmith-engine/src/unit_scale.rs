//! Exact Bevy 0.19 glTF unit and static/default-rest scale prediction.

use crate::error::PredictionRuleError;
use crate::{SettingIdV2, profiles_v2};
use animsmith_core::engine_contract::{EngineSettingIdV2, EngineSettingValueV2};
use animsmith_core::measure::{
    AssetMeasurements, LinearTransformClassification, SkeletonNodeMeasurements, measure_assets,
};
use animsmith_core::prediction::{
    EnginePredictionBasisV4, PredictionBasisReferenceV4, RawSceneAttachmentBasisDomainV1,
    RawSceneAttachmentBasisReferenceV1,
};
use animsmith_core::{
    Applicability, Check, CheckCtx, CheckOutput, DependencyClosureCoverageV1,
    EngineMachineResultV1, EnginePredictionFacetV4, EnginePredictionV4, EvaluationScope,
    EvaluationScopeCode, ImporterSubjectCreationV1, InventoryCoverageResultV1, LoadedSource,
    MeasurementPointerV1, PREDICTION_V2_MAX_CANDIDATE_FACETS_PER_RULE, PredictionBasisReferenceV1,
    PredictionBasisReferenceV2, PredictionFacetDemandV2, PredictionInventoryCoverageStateV1,
    PredictionInventoryDomainV1, PredictionProvenanceV4, PredictionRuleAllocationV2,
    PredictionScalarV1, PredictionUnavailableReasonV2, RawSceneAttachmentCoverageV1,
    RawSceneAttachmentInventoryV1, RawSourceBasisReferenceV1, RawSourceDomainV1,
    RawSourceFieldIdV1, RawSourceKeyV1, ResolvedSettingLocationV1, SourceFormatV1,
    SourceNodeLocalRest, SourceSkeletonCoverage, SourceSkeletonRowKindV1, TransformScaleDomainV1,
    TransformScaleResultV1, TransformScaleSubjectKindV1, UnitMappingResultV1,
};
use std::collections::{BTreeMap, BTreeSet};

/// Stable id for Bevy's exact glTF unit/static-scale prediction.
pub const ENGINE_UNIT_SCALE_CHECK_ID: &str = "engine-unit-scale";

const BEVY_FAMILY: &str = "bevy";
const BEVY_PROFILE_REVISION: u32 = 2;
const BEVY_ENGINE_VERSION: &str = "0.19.0";
const BEVY_IMPORTER: &str = "gltf-asset-loader";

const BEVY_LOADER_SOURCE: &str = "bevy-gltf-loader-0.19.0-c6f634ca";
const BEVY_COORDINATE_SOURCE: &str = "bevy-gltf-coordinate-conversion-0.19.0-c6f634ca";
const BEVY_RENDER_ASSET_SOURCE: &str = "bevy-render-asset-usages-0.19.0-c6f634ca";
const GLTF_UNIT_SOURCE: &str = "khronos-gltf-2.0-coordinate-units";

const FILE_SCOPE: &str = "engine-unit-scale:file-unit";
const SCENE_SCOPE: &str = "engine-unit-scale:loader-scene-root";
const SCENE_INVENTORY_SCOPE: &str = "engine-unit-scale:scene-inventory";
const MESH_SCOPE: &str = "engine-unit-scale:loader-mesh-primitive";
const MESH_INVENTORY_SCOPE: &str = "engine-unit-scale:mesh-inventory";
const SELECTED_NODE_SCOPE: &str = "engine-unit-scale:selected-source-node";
const SELECTED_REACHABILITY_UNAVAILABLE_REASON: &str =
    "animsmith:selected_node_scene_reachability_unavailable";

// Bound malformed or adversarial parent walks independently of the source
// inventory size. Ordinary production rigs fit within 128 retained rows;
// deeper ancestry is a typed unavailable result instead of an unbounded walk.
const SELECTED_ANCESTRY_MAX_NODES: usize = 128;

/// Engine-owned check for the exact Bevy revision-2 glTF/GLB profile.
pub struct EngineUnitScaleCheck<'a> {
    source: &'a LoadedSource,
    provenance: Option<&'a PredictionProvenanceV4>,
}

impl<'a> EngineUnitScaleCheck<'a> {
    /// Bind one optional exact-profile V4 provenance record to its same load.
    ///
    /// # Errors
    ///
    /// Returns [`PredictionRuleError`] when provenance is invalid, does not
    /// reproduce the source's raw facts, inventory, or dependency closure, or
    /// claims the frozen Bevy tuple with a non-registry profile record.
    pub fn new(
        source: &'a LoadedSource,
        provenance: Option<&'a PredictionProvenanceV4>,
    ) -> Result<Self, PredictionRuleError> {
        if let Some(provenance) = provenance {
            provenance
                .validate()
                .map_err(|_| PredictionRuleError::SourceProvenanceMismatch)?;
            let raw_source = animsmith_core::RawSourceBindingV2::from_source(
                source.source_facts(),
                source.exact_source_timing(),
            )
            .map_err(|_| PredictionRuleError::SourceProvenanceMismatch)?;
            let raw_inventory_matches = match (
                provenance.raw_scene_attachment().inventory(),
                source.raw_scene_attachment_inventory(),
            ) {
                (Some(left), Some(right)) => left == right,
                (None, None) => true,
                _ => false,
            };
            if &raw_source != provenance.raw_source()
                || !raw_inventory_matches
                || source.dependency_closure() != provenance.dependency_closure()
            {
                return Err(PredictionRuleError::SourceProvenanceMismatch);
            }
            if is_bevy_tuple(provenance) && !has_frozen_bevy_profile(provenance) {
                return Err(PredictionRuleError::FrozenProfileMismatch);
            }
        }
        Ok(Self { source, provenance })
    }
}

impl Check for EngineUnitScaleCheck<'_> {
    fn id(&self) -> &'static str {
        ENGINE_UNIT_SCALE_CHECK_ID
    }

    fn applicability(&self, _ctx: &CheckCtx<'_>) -> Applicability {
        match self.provenance {
            Some(provenance) if is_exact_bevy_gltf(self.source, provenance) => {
                Applicability::Applicable
            }
            _ => Applicability::NotApplicable,
        }
    }

    fn evaluate(&self, _ctx: &CheckCtx<'_>) -> CheckOutput {
        let Some(provenance) = self.provenance else {
            return empty_output();
        };
        if !is_exact_bevy_gltf(self.source, provenance) {
            return empty_output();
        }
        let plan = plan(self.source, provenance);
        let (capacity, summary_required) = match plan.demand {
            PredictionFacetDemandV2::Exact(count) => (count, false),
            PredictionFacetDemandV2::NPlusOne => (
                PREDICTION_V2_MAX_CANDIDATE_FACETS_PER_RULE.saturating_sub(1),
                true,
            ),
        };
        evaluate_allocated(self.source, provenance, &plan, capacity, summary_required)
    }

    fn prediction_facet_demand_v2(&self, _ctx: &CheckCtx<'_>) -> PredictionFacetDemandV2 {
        match self.provenance {
            Some(provenance) if is_exact_bevy_gltf(self.source, provenance) => {
                plan(self.source, provenance).demand
            }
            _ => PredictionFacetDemandV2::Exact(0),
        }
    }

    fn evaluate_with_prediction_allocation_v2(
        &self,
        _ctx: &CheckCtx<'_>,
        allocation: PredictionRuleAllocationV2<'_>,
    ) -> CheckOutput {
        let Some(provenance) = self.provenance else {
            return empty_output();
        };
        if !is_exact_bevy_gltf(self.source, provenance) {
            return empty_output();
        }
        let plan = plan(self.source, provenance);
        evaluate_allocated(
            self.source,
            provenance,
            &plan,
            allocation.candidate_capacity(),
            allocation.summary_required(),
        )
    }
}

#[derive(Debug, Clone, Copy)]
enum ScenePlan {
    Detailed(usize),
    Incomplete,
}

#[derive(Debug, Clone)]
enum MeshPlan {
    Detailed(Vec<MeshJoinRow>),
    CompleteEmpty,
    Incomplete,
    JoinOverflow,
}

/// One deterministic atomic row of the bounded scene/attachment/primitive join.
#[derive(Debug, Clone, Copy)]
struct MeshJoinRow {
    source_scene_index: u64,
    source_root_ordinal: u64,
    root_node_index: u64,
    source_node_index: u64,
    source_mesh_index: u64,
    source_primitive_index: u64,
}

#[derive(Debug, Clone)]
struct RulePlan {
    demand: PredictionFacetDemandV2,
    scenes: ScenePlan,
    meshes: MeshPlan,
    selectors: usize,
}

fn plan(source: &LoadedSource, provenance: &PredictionProvenanceV4) -> RulePlan {
    let inventory = provenance.raw_scene_attachment().inventory();
    let scenes = match inventory {
        Some(inventory)
            if inventory.scenes().coverage() == RawSceneAttachmentCoverageV1::Complete =>
        {
            ScenePlan::Detailed(inventory.scenes().rows().len())
        }
        _ => ScenePlan::Incomplete,
    };
    let meshes = plan_meshes(source, inventory);
    let selectors = selected_facet_count(source, inventory, provenance);

    let scene_facets = match scenes {
        ScenePlan::Detailed(count) => count,
        ScenePlan::Incomplete => 1,
    };
    let mesh_facets = match &meshes {
        MeshPlan::Detailed(rows) => rows.len(),
        MeshPlan::CompleteEmpty | MeshPlan::Incomplete | MeshPlan::JoinOverflow => 1,
    };
    let exact = 1usize
        .checked_add(scene_facets)
        .and_then(|count| count.checked_add(mesh_facets))
        .and_then(|count| count.checked_add(selectors));
    let demand = match exact {
        Some(count) if count <= PREDICTION_V2_MAX_CANDIDATE_FACETS_PER_RULE => {
            PredictionFacetDemandV2::Exact(count)
        }
        _ => PredictionFacetDemandV2::NPlusOne,
    };
    RulePlan {
        demand,
        scenes,
        meshes,
        selectors,
    }
}

fn plan_meshes(
    source: &LoadedSource,
    inventory: Option<&RawSceneAttachmentInventoryV1>,
) -> MeshPlan {
    let Some(inventory) = inventory else {
        return MeshPlan::Incomplete;
    };
    if inventory.scenes().coverage() != RawSceneAttachmentCoverageV1::Complete
        || inventory.node_mesh_attachments().coverage() != RawSceneAttachmentCoverageV1::Complete
        || inventory.mesh_primitives().coverage() != RawSceneAttachmentCoverageV1::Complete
        || inventory.source_skeleton().coverage() != RawSceneAttachmentCoverageV1::Complete
        || source.document().assets.source_skeleton.coverage != SourceSkeletonCoverage::Complete
    {
        return MeshPlan::Incomplete;
    }
    checked_mesh_join(source, inventory)
}

fn selected_facet_count(
    source: &LoadedSource,
    inventory: Option<&RawSceneAttachmentInventoryV1>,
    provenance: &PredictionProvenanceV4,
) -> usize {
    let selectors = provenance.rule_inputs().runtime_node_selectors();
    if selectors.is_empty() {
        return 0;
    }
    if source.document().assets.source_skeleton.coverage != SourceSkeletonCoverage::Complete {
        return selectors.len();
    }
    let nodes = &source.document().assets.source_skeleton.nodes;
    let parents = selected_parent_index(source);
    let complete_scenes = inventory.filter(|inventory| {
        inventory.scenes().coverage() == RawSceneAttachmentCoverageV1::Complete
            && inventory.source_skeleton().coverage() == RawSceneAttachmentCoverageV1::Complete
    });
    let mut count = 0usize;
    for selector in selectors {
        let mut matches = nodes.iter().filter(|node| {
            node.name
                .as_deref()
                .is_some_and(|name| animsmith_core::config::glob_match(selector, name))
        });
        let first = matches.next();
        let ambiguous = matches.next().is_some();
        let facets = match (first, ambiguous, complete_scenes) {
            (Some(node), false, Some(inventory)) => {
                let mut work = 0usize;
                let mut reachable = 0usize;
                for scene in inventory.scenes().rows() {
                    match selected_scene_reachability(
                        node.source_node_index as u64,
                        scene.root_node_indices(),
                        &parents,
                        &mut work,
                    ) {
                        SelectedSceneReachability::Reachable(_, _) => {
                            reachable = reachable.saturating_add(1);
                        }
                        SelectedSceneReachability::Unreachable => {}
                        SelectedSceneReachability::Unavailable
                        | SelectedSceneReachability::WorkBudgetExceeded => {
                            reachable = 0;
                            break;
                        }
                    }
                }
                reachable.max(1)
            }
            _ => 1,
        };
        count = count.saturating_add(facets);
        if count > PREDICTION_V2_MAX_CANDIDATE_FACETS_PER_RULE {
            return count;
        }
    }
    count
}

/// Materialize the atomic scene×reachable-attachment×primitive join to N+1.
///
/// Each scene/attachment probe, parent step, and matching primitive expansion
/// spends one deterministic work unit. That bounds adversarial unmatched
/// cross-products and deep walks as well as emitted facets; an incomplete probe
/// is a budget result, never a complete-empty inventory claim.
fn checked_mesh_join(source: &LoadedSource, inventory: &RawSceneAttachmentInventoryV1) -> MeshPlan {
    let parents = source
        .document()
        .assets
        .source_skeleton
        .nodes
        .iter()
        .map(|node| {
            (
                node.source_node_index as u64,
                node.parent_source_node_index.map(|parent| parent as u64),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let scene_roots = inventory
        .scenes()
        .rows()
        .iter()
        .map(|scene| {
            (
                scene.source_scene_index(),
                scene
                    .root_node_indices()
                    .iter()
                    .copied()
                    .enumerate()
                    .map(|(ordinal, node)| (node, ordinal as u64))
                    .collect::<BTreeMap<_, _>>(),
            )
        })
        .collect::<Vec<_>>();
    let primitives_by_mesh = inventory.mesh_primitives().rows().iter().fold(
        BTreeMap::<u64, Vec<u64>>::new(),
        |mut grouped, primitive| {
            grouped
                .entry(primitive.source_mesh_index())
                .or_default()
                .push(primitive.source_primitive_index());
            grouped
        },
    );
    let mut work = 0usize;
    let mut rows = Vec::new();
    for (source_scene_index, roots) in scene_roots {
        for attachment in inventory.node_mesh_attachments().rows() {
            if join_budget_exceeded(&mut work) {
                return MeshPlan::JoinOverflow;
            }
            let Ok(reachable) = reachable_scene_root_indexed(
                attachment.source_node_index(),
                &roots,
                &parents,
                &mut work,
            ) else {
                return MeshPlan::JoinOverflow;
            };
            let Some((source_root_ordinal, root_node_index)) = reachable else {
                continue;
            };
            for &source_primitive_index in primitives_by_mesh
                .get(&attachment.source_mesh_index())
                .into_iter()
                .flatten()
            {
                if join_budget_exceeded(&mut work) {
                    return MeshPlan::JoinOverflow;
                }
                rows.push(MeshJoinRow {
                    source_scene_index,
                    source_root_ordinal,
                    root_node_index,
                    source_node_index: attachment.source_node_index(),
                    source_mesh_index: attachment.source_mesh_index(),
                    source_primitive_index,
                });
            }
        }
    }
    if rows.is_empty() {
        MeshPlan::CompleteEmpty
    } else {
        MeshPlan::Detailed(rows)
    }
}

fn join_budget_exceeded(work: &mut usize) -> bool {
    *work = work.saturating_add(1);
    *work > PREDICTION_V2_MAX_CANDIDATE_FACETS_PER_RULE
}

fn evaluate_allocated(
    source: &LoadedSource,
    provenance: &PredictionProvenanceV4,
    plan: &RulePlan,
    candidate_capacity: usize,
    summary_required: bool,
) -> CheckOutput {
    let measurements = measure_assets(source.document());
    let dependency_complete = matches!(
        provenance.dependency_closure().coverage(),
        DependencyClosureCoverageV1::Complete
    );
    let mut candidates = Vec::with_capacity(plan.demand.bounded_count());

    if candidate_capacity != 0 {
        candidates.push(file_facet(provenance, dependency_complete));
    }
    append_scene_facets(
        &mut candidates,
        provenance,
        plan.scenes,
        dependency_complete,
        candidate_capacity,
    );
    append_mesh_facets(
        &mut candidates,
        provenance,
        &plan.meshes,
        dependency_complete,
        candidate_capacity,
    );
    append_selected_node_facets(
        &mut candidates,
        plan.selectors,
        SelectedEvaluation {
            source,
            provenance,
            measurements: &measurements,
            dependency_complete,
            candidate_capacity,
        },
    );

    let mut facets = candidates;
    if summary_required {
        facets.push(unavailable_facet(
            budget_scope(),
            static_basis(&[BEVY_LOADER_SOURCE]),
            vec![PredictionUnavailableReasonV2::FacetBudgetExceeded],
        ));
    }
    let evaluated_scopes = facets
        .iter()
        .filter(|facet| facet.result().is_some())
        .map(|facet| facet.scope().clone())
        .collect();
    let prediction = EnginePredictionV4::new(provenance.identity().clone(), facets)
        .expect("allocated unit/scale facets satisfy V4 bounds");
    CheckOutput::from_coverage(Vec::new(), evaluated_scopes, Vec::new())
        .with_engine_prediction_v4(prediction)
}

fn file_facet(
    _provenance: &PredictionProvenanceV4,
    dependency_complete: bool,
) -> EnginePredictionFacetV4 {
    let scope = EvaluationScope::new(EvaluationScopeCode::custom(FILE_SCOPE));
    let basis = unit_basis();
    if !dependency_complete {
        return unavailable_facet(
            scope,
            basis,
            vec![PredictionUnavailableReasonV2::DependencyClosureIncomplete],
        );
    }
    EnginePredictionFacetV4::available(
        scope,
        basis,
        EngineMachineResultV1::UnitMapping(UnitMappingResultV1::gltf_to_engine_world_length_unit()),
    )
    .expect("the frozen Bevy file mapping is valid")
}

fn append_scene_facets(
    facets: &mut Vec<EnginePredictionFacetV4>,
    provenance: &PredictionProvenanceV4,
    plan: ScenePlan,
    dependency_complete: bool,
    candidate_capacity: usize,
) {
    if facets.len() >= candidate_capacity {
        return;
    }
    let Some(inventory) = provenance.raw_scene_attachment().inventory() else {
        facets.push(unavailable_facet(
            EvaluationScope::new(EvaluationScopeCode::custom(SCENE_INVENTORY_SCOPE)),
            scene_inventory_basis(None),
            unavailable_with_dependency(
                PredictionUnavailableReasonV2::RawSourceIncomplete,
                dependency_complete,
            ),
        ));
        return;
    };
    match plan {
        ScenePlan::Incomplete => facets.push(unavailable_facet(
            EvaluationScope::new(EvaluationScopeCode::custom(SCENE_INVENTORY_SCOPE)),
            scene_inventory_basis(Some(inventory)),
            unavailable_with_dependency(
                PredictionUnavailableReasonV2::RawSourceIncomplete,
                dependency_complete,
            ),
        )),
        ScenePlan::Detailed(expected) => {
            debug_assert_eq!(expected, inventory.scenes().rows().len());
            for scene in inventory.scenes().rows() {
                if facets.len() >= candidate_capacity {
                    break;
                }
                let scope = EvaluationScope::new(EvaluationScopeCode::custom(SCENE_SCOPE))
                    .subject(format!("source_scene:{}", scene.source_scene_index()));
                let basis = scene_row_basis(scene.source_scene_index());
                if dependency_complete {
                    facets.push(available_transform(
                        scope,
                        basis,
                        TransformScaleSubjectKindV1::LoaderSceneEntity,
                        ImporterSubjectCreationV1::Created,
                        TransformScaleDomainV1::Local,
                        LinearTransformClassification::UnitOrthonormal,
                    ));
                } else {
                    facets.push(unavailable_facet(
                        scope,
                        basis,
                        vec![PredictionUnavailableReasonV2::DependencyClosureIncomplete],
                    ));
                }
            }
        }
    }
}

fn append_mesh_facets(
    facets: &mut Vec<EnginePredictionFacetV4>,
    provenance: &PredictionProvenanceV4,
    plan: &MeshPlan,
    dependency_complete: bool,
    candidate_capacity: usize,
) {
    if facets.len() >= candidate_capacity {
        return;
    }
    match plan {
        MeshPlan::Incomplete => {
            facets.push(unavailable_facet(
                EvaluationScope::new(EvaluationScopeCode::custom(MESH_INVENTORY_SCOPE)),
                mesh_inventory_basis(provenance.raw_scene_attachment().inventory()),
                unavailable_with_dependency(
                    PredictionUnavailableReasonV2::RawSourceIncomplete,
                    dependency_complete,
                ),
            ));
        }
        MeshPlan::JoinOverflow => {
            facets.push(unavailable_facet(
                EvaluationScope::new(EvaluationScopeCode::custom(MESH_INVENTORY_SCOPE)),
                mesh_inventory_basis(provenance.raw_scene_attachment().inventory()),
                vec![custom_reason("animsmith:mesh_join_work_budget_exceeded")],
            ));
        }
        MeshPlan::CompleteEmpty => {
            let scope = EvaluationScope::new(EvaluationScopeCode::custom(MESH_INVENTORY_SCOPE));
            let basis = mesh_inventory_basis(provenance.raw_scene_attachment().inventory());
            if dependency_complete {
                facets.push(
                    EnginePredictionFacetV4::available(
                        scope,
                        basis,
                        EngineMachineResultV1::InventoryCoverage(InventoryCoverageResultV1 {
                            domain: PredictionInventoryDomainV1::LoaderMeshPrimitiveSubjects,
                            coverage: PredictionInventoryCoverageStateV1::Complete,
                            retained_rows: 0,
                        }),
                    )
                    .expect("complete-empty joined mesh subjects form an available result"),
                );
            } else {
                facets.push(unavailable_facet(
                    scope,
                    basis,
                    vec![PredictionUnavailableReasonV2::DependencyClosureIncomplete],
                ));
            }
        }
        MeshPlan::Detailed(rows) => {
            for row in rows {
                if facets.len() >= candidate_capacity {
                    break;
                }
                let scope =
                    EvaluationScope::new(EvaluationScopeCode::custom(MESH_SCOPE)).subject(format!(
                        "source_scene:{}:source_node:{}:source_mesh:{}:source_primitive:{}",
                        row.source_scene_index,
                        row.source_node_index,
                        row.source_mesh_index,
                        row.source_primitive_index,
                    ));
                let basis = mesh_row_basis(
                    row.source_scene_index,
                    row.source_root_ordinal,
                    row.root_node_index,
                    row.source_node_index,
                    row.source_mesh_index,
                    row.source_primitive_index,
                );
                if dependency_complete {
                    facets.push(available_transform(
                        scope,
                        basis,
                        TransformScaleSubjectKindV1::LoaderMeshPrimitiveEntity,
                        if load_meshes(provenance) {
                            ImporterSubjectCreationV1::Created
                        } else {
                            ImporterSubjectCreationV1::SuppressedBySetting
                        },
                        TransformScaleDomainV1::Local,
                        LinearTransformClassification::UnitOrthonormal,
                    ));
                } else {
                    facets.push(unavailable_facet(
                        scope,
                        basis,
                        vec![PredictionUnavailableReasonV2::DependencyClosureIncomplete],
                    ));
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
struct SelectedEvaluation<'a> {
    source: &'a LoadedSource,
    provenance: &'a PredictionProvenanceV4,
    measurements: &'a AssetMeasurements,
    dependency_complete: bool,
    candidate_capacity: usize,
}

fn append_selected_node_facets(
    facets: &mut Vec<EnginePredictionFacetV4>,
    expected_facets: usize,
    evaluation: SelectedEvaluation<'_>,
) {
    let SelectedEvaluation {
        source,
        provenance,
        measurements: _,
        dependency_complete,
        candidate_capacity,
    } = evaluation;
    let selectors = provenance.rule_inputs().runtime_node_selectors();
    if selectors.is_empty() {
        debug_assert_eq!(expected_facets, 0);
        return;
    }
    let inventory = provenance.raw_scene_attachment().inventory();
    let inventory_complete = inventory.is_some_and(|inventory| {
        inventory.scenes().coverage() == RawSceneAttachmentCoverageV1::Complete
            && inventory.source_skeleton().coverage() == RawSceneAttachmentCoverageV1::Complete
    });
    let start_len = facets.len();
    if source.document().assets.source_skeleton.coverage != SourceSkeletonCoverage::Complete
        || !inventory_complete
    {
        for selector in selectors {
            if facets.len() >= candidate_capacity {
                break;
            }
            facets.push(unavailable_facet(
                selected_scope(selector),
                selected_resolution_basis(source, inventory, selector, &[]),
                unavailable_with_dependency(
                    PredictionUnavailableReasonV2::RawSourceIncomplete,
                    dependency_complete,
                ),
            ));
        }
        return;
    }

    let inventory = inventory.expect("complete selected-node inventory exists");
    for selector in selectors {
        if facets.len() >= candidate_capacity {
            break;
        }
        let mut matches = source
            .document()
            .assets
            .source_skeleton
            .nodes
            .iter()
            .filter(|node| {
                node.name
                    .as_deref()
                    .is_some_and(|name| animsmith_core::config::glob_match(selector, name))
            });
        let first = matches.next();
        let second = matches.next();
        match (first, second) {
            (None, _) => facets.push(unavailable_facet(
                selected_scope(selector),
                selected_resolution_basis(source, Some(inventory), selector, &[]),
                unavailable_with_dependency(
                    PredictionUnavailableReasonV2::SourceSelectorNoMatch,
                    dependency_complete,
                ),
            )),
            (Some(first), Some(second)) => facets.push(unavailable_facet(
                selected_scope(selector),
                selected_resolution_basis(
                    source,
                    Some(inventory),
                    selector,
                    &[first.source_node_index, second.source_node_index],
                ),
                unavailable_with_dependency(
                    PredictionUnavailableReasonV2::SourceSelectorAmbiguous,
                    dependency_complete,
                ),
            )),
            (Some(node), None) => append_resolved_selected_node(
                facets,
                inventory,
                selector,
                node.source_node_index,
                evaluation,
            ),
        }
    }
    if candidate_capacity >= start_len.saturating_add(expected_facets) {
        debug_assert_eq!(facets.len() - start_len, expected_facets);
    }
}

fn append_resolved_selected_node(
    facets: &mut Vec<EnginePredictionFacetV4>,
    inventory: &RawSceneAttachmentInventoryV1,
    selector: &str,
    node_index: usize,
    evaluation: SelectedEvaluation<'_>,
) {
    let SelectedEvaluation {
        source,
        measurements,
        dependency_complete,
        candidate_capacity,
        ..
    } = evaluation;
    let parents = selected_parent_index(source);
    let mut reachable_scenes = Vec::new();
    let mut work = 0usize;
    for scene in inventory.scenes().rows() {
        match selected_scene_reachability(
            node_index as u64,
            scene.root_node_indices(),
            &parents,
            &mut work,
        ) {
            SelectedSceneReachability::Reachable(root_ordinal, root_node_index) => {
                reachable_scenes.push((scene, (root_ordinal, root_node_index)));
            }
            SelectedSceneReachability::Unreachable => {}
            SelectedSceneReachability::Unavailable
            | SelectedSceneReachability::WorkBudgetExceeded => {
                facets.push(unavailable_facet(
                    selected_scope(selector),
                    selected_node_basis(
                        source,
                        Some(inventory),
                        selector,
                        node_index,
                        &[],
                        None,
                        None,
                    ),
                    unavailable_with_dependency(
                        custom_reason(SELECTED_REACHABILITY_UNAVAILABLE_REASON),
                        dependency_complete,
                    ),
                ));
                return;
            }
        }
    }
    let ancestry = selected_ancestry(source, node_index);
    let measurement = measurements
        .skeleton_nodes
        .iter()
        .enumerate()
        .find(|(_, node)| node.node_index == node_index);
    if reachable_scenes.is_empty() {
        facets.push(unavailable_facet(
            selected_scope(selector),
            selected_node_basis(
                source,
                Some(inventory),
                selector,
                node_index,
                &ancestry.rows,
                None,
                None,
            ),
            unavailable_with_dependency(
                custom_reason("animsmith:selected_node_unreachable"),
                dependency_complete,
            ),
        ));
        return;
    }

    for (scene, (root_ordinal, root_node_index)) in reachable_scenes {
        if facets.len() >= candidate_capacity {
            break;
        }
        let scene_witness = Some(SelectedSceneWitness {
            source_scene_index: scene.source_scene_index(),
            source_root_ordinal: root_ordinal,
            root_node_index,
        });
        let basis = selected_node_basis(
            source,
            Some(inventory),
            selector,
            node_index,
            &ancestry.rows,
            scene_witness,
            measurement,
        );
        let scope = selected_scene_scope(selector, scene.source_scene_index(), node_index);
        let unavailable_reason = match ancestry.representation {
            SelectedRepresentation::AllTrs => measurement.and_then(|(_, node)| {
                (node.rest_world_linear.classification == LinearTransformClassification::NonFinite
                    || node.rest_world_matrix.is_none())
                .then_some(PredictionUnavailableReasonV2::MeasurementUnavailable)
            }),
            SelectedRepresentation::MatrixAuthored => Some(custom_reason(
                "animsmith:matrix_authored_selected_node_or_ancestry",
            )),
            SelectedRepresentation::Unavailable => Some(custom_reason(
                "animsmith:selected_node_ancestry_unavailable",
            )),
        }
        .or_else(|| {
            measurement
                .is_none()
                .then_some(PredictionUnavailableReasonV2::MeasurementUnavailable)
        });
        if let Some(reason) = unavailable_reason {
            facets.push(unavailable_facet(
                scope,
                basis,
                unavailable_with_dependency(reason, dependency_complete),
            ));
        } else if dependency_complete {
            let node = measurement
                .expect("available selected node has a measurement")
                .1;
            // Bevy's optional loader-root 180-degree Y rotation is proper
            // orthonormal pre-multiplication. Its two linear sign flips
            // preserve the same-load affine classification exactly.
            facets.push(available_transform(
                scope,
                basis,
                TransformScaleSubjectKindV1::SelectedSourceNode,
                ImporterSubjectCreationV1::Created,
                TransformScaleDomainV1::LoaderRootToSubject,
                node.rest_world_linear.classification,
            ));
        } else {
            facets.push(unavailable_facet(
                scope,
                basis,
                vec![PredictionUnavailableReasonV2::DependencyClosureIncomplete],
            ));
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectedRepresentation {
    AllTrs,
    MatrixAuthored,
    Unavailable,
}

struct SelectedAncestry {
    representation: SelectedRepresentation,
    rows: Vec<usize>,
}

fn selected_ancestry(source: &LoadedSource, start: usize) -> SelectedAncestry {
    let nodes = source
        .document()
        .assets
        .source_skeleton
        .nodes
        .iter()
        .map(|node| (node.source_node_index, node))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    let mut current = start;
    let mut rows = Vec::new();
    for _ in 0..SELECTED_ANCESTRY_MAX_NODES {
        let index = current;
        if !seen.insert(index) {
            return SelectedAncestry {
                representation: SelectedRepresentation::Unavailable,
                rows,
            };
        }
        let Some(node) = nodes.get(&index) else {
            return SelectedAncestry {
                representation: SelectedRepresentation::Unavailable,
                rows,
            };
        };
        rows.push(index);
        if matches!(node.local_rest, SourceNodeLocalRest::Matrix(_)) {
            return SelectedAncestry {
                representation: SelectedRepresentation::MatrixAuthored,
                rows,
            };
        }
        let Some(parent) = node.parent_source_node_index else {
            return SelectedAncestry {
                representation: SelectedRepresentation::AllTrs,
                rows,
            };
        };
        current = parent;
    }
    SelectedAncestry {
        representation: SelectedRepresentation::Unavailable,
        rows,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectedSceneReachability {
    Reachable(u64, u64),
    Unreachable,
    Unavailable,
    WorkBudgetExceeded,
}

fn selected_reachability_budget_exceeded(work: &mut usize) -> bool {
    *work = work.saturating_add(1);
    *work > PREDICTION_V2_MAX_CANDIDATE_FACETS_PER_RULE
}

fn selected_parent_index(source: &LoadedSource) -> BTreeMap<u64, Option<u64>> {
    source
        .document()
        .assets
        .source_skeleton
        .nodes
        .iter()
        .map(|node| {
            (
                node.source_node_index as u64,
                node.parent_source_node_index.map(|parent| parent as u64),
            )
        })
        .collect()
}

fn selected_scene_reachability(
    start: u64,
    roots: &[u64],
    parents: &BTreeMap<u64, Option<u64>>,
    work: &mut usize,
) -> SelectedSceneReachability {
    if *work >= PREDICTION_V2_MAX_CANDIDATE_FACETS_PER_RULE {
        return SelectedSceneReachability::WorkBudgetExceeded;
    }
    let roots = roots
        .iter()
        .copied()
        .enumerate()
        .map(|(ordinal, node)| (node, ordinal as u64))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    let mut current = Some(start);
    for _ in 0..SELECTED_ANCESTRY_MAX_NODES {
        let Some(index) = current else {
            return SelectedSceneReachability::Unreachable;
        };
        if selected_reachability_budget_exceeded(work) {
            return SelectedSceneReachability::WorkBudgetExceeded;
        }
        if let Some(ordinal) = roots.get(&index) {
            return SelectedSceneReachability::Reachable(*ordinal, index);
        }
        if !seen.insert(index) {
            return SelectedSceneReachability::Unavailable;
        }
        current = match parents.get(&index) {
            Some(parent) => *parent,
            None => return SelectedSceneReachability::Unavailable,
        };
    }
    match current {
        Some(_) => SelectedSceneReachability::Unavailable,
        None => SelectedSceneReachability::Unreachable,
    }
}

fn reachable_scene_root_indexed(
    start: u64,
    roots: &BTreeMap<u64, u64>,
    parents: &BTreeMap<u64, Option<u64>>,
    work: &mut usize,
) -> Result<Option<(u64, u64)>, ()> {
    let mut seen = BTreeSet::new();
    let mut current = Some(start);
    while let Some(index) = current {
        if join_budget_exceeded(work) {
            return Err(());
        }
        if let Some(ordinal) = roots.get(&index) {
            return Ok(Some((*ordinal, index)));
        }
        if !seen.insert(index) {
            return Ok(None);
        }
        current = match parents.get(&index) {
            Some(parent) => *parent,
            None => return Ok(None),
        };
    }
    Ok(None)
}

fn available_transform(
    scope: EvaluationScope,
    basis: EnginePredictionBasisV4,
    subject_kind: TransformScaleSubjectKindV1,
    creation: ImporterSubjectCreationV1,
    domain: TransformScaleDomainV1,
    classification: LinearTransformClassification,
) -> EnginePredictionFacetV4 {
    EnginePredictionFacetV4::available(
        scope,
        basis,
        EngineMachineResultV1::TransformScale(TransformScaleResultV1 {
            subject_kind,
            creation,
            domain,
            classification: (creation == ImporterSubjectCreationV1::Created)
                .then_some(classification),
        }),
    )
    .expect("the exact Bevy transform result is valid")
}

fn unavailable_facet(
    scope: EvaluationScope,
    basis: EnginePredictionBasisV4,
    reasons: Vec<PredictionUnavailableReasonV2>,
) -> EnginePredictionFacetV4 {
    EnginePredictionFacetV4::required_unavailable(scope, basis, reasons)
        .expect("typed unit/scale unavailability is valid")
}

fn unavailable_with_dependency(
    reason: PredictionUnavailableReasonV2,
    dependency_complete: bool,
) -> Vec<PredictionUnavailableReasonV2> {
    let mut reasons = vec![reason];
    if !dependency_complete {
        reasons.push(PredictionUnavailableReasonV2::DependencyClosureIncomplete);
    }
    reasons
}

fn unit_basis() -> EnginePredictionBasisV4 {
    let mut references = [
        "application_world_unit_policy",
        "importer_scale_conversion",
        "physical_dimensions_preserved",
        "source_to_target_unit_mapping",
        "target_linear_unit",
    ]
    .into_iter()
    .map(|fact| {
        PredictionBasisReferenceV4::v2(PredictionBasisReferenceV2::v1(
            PredictionBasisReferenceV1::profile_fact(fact).expect("static fact id is valid"),
        ))
    })
    .collect::<Vec<_>>();
    for source in [BEVY_LOADER_SOURCE, BEVY_COORDINATE_SOURCE, GLTF_UNIT_SOURCE] {
        references.push(primary_source_reference(source));
    }
    EnginePredictionBasisV4::new(references).expect("the frozen unit basis is valid")
}

fn scene_inventory_basis(
    inventory: Option<&RawSceneAttachmentInventoryV1>,
) -> EnginePredictionBasisV4 {
    let mut references = static_references(&[BEVY_LOADER_SOURCE, BEVY_COORDINATE_SOURCE]);
    references.push(transform_profile_fact_reference());
    references.push(setting_reference(SettingIdV2::ExtensionHandlerEnvironment));
    references.push(setting_reference(SettingIdV2::RotateSceneEntity));
    if inventory.is_some() {
        references.push(raw_inventory_reference(
            RawSceneAttachmentBasisReferenceV1::Coverage {
                domain: RawSceneAttachmentBasisDomainV1::Scenes,
            },
        ));
    }
    EnginePredictionBasisV4::new(references).expect("the frozen scene basis is valid")
}

fn scene_row_basis(source_scene_index: u64) -> EnginePredictionBasisV4 {
    let mut references = static_references(&[BEVY_LOADER_SOURCE, BEVY_COORDINATE_SOURCE]);
    references.push(transform_profile_fact_reference());
    references.push(setting_reference(SettingIdV2::ExtensionHandlerEnvironment));
    references.push(setting_reference(SettingIdV2::RotateSceneEntity));
    references.push(raw_inventory_reference(
        RawSceneAttachmentBasisReferenceV1::Coverage {
            domain: RawSceneAttachmentBasisDomainV1::Scenes,
        },
    ));
    references.push(raw_inventory_reference(
        RawSceneAttachmentBasisReferenceV1::SceneRow { source_scene_index },
    ));
    EnginePredictionBasisV4::new(references).expect("the scene-row basis is valid")
}

fn mesh_inventory_basis(
    inventory: Option<&RawSceneAttachmentInventoryV1>,
) -> EnginePredictionBasisV4 {
    let mut references = static_references(&[
        BEVY_LOADER_SOURCE,
        BEVY_COORDINATE_SOURCE,
        BEVY_RENDER_ASSET_SOURCE,
    ]);
    references.push(transform_profile_fact_reference());
    references.push(setting_reference(SettingIdV2::ExtensionHandlerEnvironment));
    references.push(setting_reference(SettingIdV2::LoadMeshes));
    references.push(setting_reference(SettingIdV2::RotateMeshes));
    if inventory.is_some() {
        for domain in [
            RawSceneAttachmentBasisDomainV1::SourceSkeleton,
            RawSceneAttachmentBasisDomainV1::Scenes,
            RawSceneAttachmentBasisDomainV1::NodeMeshAttachments,
            RawSceneAttachmentBasisDomainV1::MeshPrimitives,
        ] {
            references.push(raw_inventory_reference(
                RawSceneAttachmentBasisReferenceV1::Coverage { domain },
            ));
        }
    }
    EnginePredictionBasisV4::new(references).expect("the frozen mesh basis is valid")
}

fn mesh_row_basis(
    source_scene_index: u64,
    source_root_ordinal: u64,
    root_node_index: u64,
    source_node_index: u64,
    source_mesh_index: u64,
    source_primitive_index: u64,
) -> EnginePredictionBasisV4 {
    let mut references = static_references(&[
        BEVY_LOADER_SOURCE,
        BEVY_COORDINATE_SOURCE,
        BEVY_RENDER_ASSET_SOURCE,
    ]);
    references.push(transform_profile_fact_reference());
    references.push(setting_reference(SettingIdV2::ExtensionHandlerEnvironment));
    references.push(setting_reference(SettingIdV2::LoadMeshes));
    references.push(setting_reference(SettingIdV2::RotateMeshes));
    for domain in [
        RawSceneAttachmentBasisDomainV1::SourceSkeleton,
        RawSceneAttachmentBasisDomainV1::Scenes,
        RawSceneAttachmentBasisDomainV1::NodeMeshAttachments,
        RawSceneAttachmentBasisDomainV1::MeshPrimitives,
    ] {
        references.push(raw_inventory_reference(
            RawSceneAttachmentBasisReferenceV1::Coverage { domain },
        ));
    }
    for reference in [
        RawSceneAttachmentBasisReferenceV1::SceneRow { source_scene_index },
        RawSceneAttachmentBasisReferenceV1::SceneRoot {
            source_scene_index,
            source_root_ordinal,
            source_node_index: root_node_index,
        },
        RawSceneAttachmentBasisReferenceV1::NodeMeshAttachmentRow {
            source_node_index,
            source_mesh_index,
        },
        RawSceneAttachmentBasisReferenceV1::MeshPrimitiveRow {
            source_mesh_index,
            source_primitive_index,
        },
    ] {
        references.push(raw_inventory_reference(reference));
    }
    EnginePredictionBasisV4::new(references).expect("the mesh-row basis is valid")
}

fn raw_inventory_reference(
    reference: RawSceneAttachmentBasisReferenceV1,
) -> PredictionBasisReferenceV4 {
    PredictionBasisReferenceV4::raw_scene_attachment(reference)
}

fn selected_static_references(selector: &str) -> Vec<PredictionBasisReferenceV4> {
    let mut references = static_references(&[BEVY_LOADER_SOURCE, BEVY_COORDINATE_SOURCE]);
    references.push(transform_profile_fact_reference());
    references.push(setting_reference(SettingIdV2::ExtensionHandlerEnvironment));
    references.push(setting_reference(SettingIdV2::RotateSceneEntity));
    references.push(PredictionBasisReferenceV4::v2(
        PredictionBasisReferenceV2::v1(
            PredictionBasisReferenceV1::project_field(
                "runtime_nodes.selector",
                PredictionScalarV1::text(selector).expect("validated selector text is bounded"),
            )
            .expect("the runtime-node selector field id is valid"),
        ),
    ));
    references
}

fn selected_resolution_basis(
    source: &LoadedSource,
    inventory: Option<&RawSceneAttachmentInventoryV1>,
    selector: &str,
    matched_node_indices: &[usize],
) -> EnginePredictionBasisV4 {
    let mut references = selected_static_references(selector);
    if inventory.is_some() {
        for domain in [
            RawSceneAttachmentBasisDomainV1::SourceSkeleton,
            RawSceneAttachmentBasisDomainV1::Scenes,
        ] {
            references.push(raw_inventory_reference(
                RawSceneAttachmentBasisReferenceV1::Coverage { domain },
            ));
        }
    }
    for &node_index in matched_node_indices {
        references.push(raw_source_node_reference(source, node_index, "name"));
    }
    EnginePredictionBasisV4::new(references).expect("the selected-node resolution basis is valid")
}

#[derive(Debug, Clone, Copy)]
struct SelectedSceneWitness {
    source_scene_index: u64,
    source_root_ordinal: u64,
    root_node_index: u64,
}

fn selected_node_basis(
    source: &LoadedSource,
    inventory: Option<&RawSceneAttachmentInventoryV1>,
    selector: &str,
    node_index: usize,
    ancestry_rows: &[usize],
    scene: Option<SelectedSceneWitness>,
    measurement: Option<(usize, &SkeletonNodeMeasurements)>,
) -> EnginePredictionBasisV4 {
    let mut references = selected_resolution_basis(source, inventory, selector, &[node_index])
        .references()
        .to_vec();
    for &ancestry_node_index in ancestry_rows {
        references.push(raw_source_node_reference(
            source,
            ancestry_node_index,
            "local_rest.kind",
        ));
        references.push(raw_source_node_reference(
            source,
            ancestry_node_index,
            "parent_source_node_index",
        ));
    }
    if let Some(scene) = scene {
        references.push(raw_inventory_reference(
            RawSceneAttachmentBasisReferenceV1::SceneRow {
                source_scene_index: scene.source_scene_index,
            },
        ));
        references.push(raw_inventory_reference(
            RawSceneAttachmentBasisReferenceV1::SceneRoot {
                source_scene_index: scene.source_scene_index,
                source_root_ordinal: scene.source_root_ordinal,
                source_node_index: scene.root_node_index,
            },
        ));
    }
    if let Some((measurement_ordinal, node)) = measurement {
        references.push(PredictionBasisReferenceV4::v2(
            PredictionBasisReferenceV2::v1(PredictionBasisReferenceV1::measurement_v16(
                MeasurementPointerV1::new(format!(
                    "/measurements/skeleton_nodes/{measurement_ordinal}/rest_world_linear/classification"
                ))
                .expect("static measurement path shape is valid"),
                PredictionScalarV1::token(classification_name(
                    node.rest_world_linear.classification,
                ))
                .expect("classification spelling is a token"),
            )),
        ));
    }
    EnginePredictionBasisV4::new(references).expect("the selected-node evidence basis is valid")
}

fn raw_source_node_reference(
    source: &LoadedSource,
    node_index: usize,
    field: &'static str,
) -> PredictionBasisReferenceV4 {
    let raw = RawSourceBasisReferenceV1::from_source(
        RawSourceDomainV1::SourceNode,
        RawSourceKeyV1::SourceSkeleton {
            row_kind: SourceSkeletonRowKindV1::SourceNode,
            source_index: node_index as u64,
        },
        RawSourceFieldIdV1::new(field).expect("static raw field is valid"),
        source.source_facts(),
    )
    .expect("resolved selected source node has the cited raw field");
    PredictionBasisReferenceV4::v2(PredictionBasisReferenceV2::v1(
        PredictionBasisReferenceV1::raw_source(raw),
    ))
}

fn transform_profile_fact_reference() -> PredictionBasisReferenceV4 {
    PredictionBasisReferenceV4::v2(PredictionBasisReferenceV2::v1(
        PredictionBasisReferenceV1::profile_fact("resulting_transform_scale")
            .expect("static profile fact id is valid"),
    ))
}

fn static_basis(sources: &[&str]) -> EnginePredictionBasisV4 {
    EnginePredictionBasisV4::new(static_references(sources))
        .expect("the frozen static basis is valid")
}

fn static_references(sources: &[&str]) -> Vec<PredictionBasisReferenceV4> {
    sources
        .iter()
        .map(|source| primary_source_reference(source))
        .collect()
}

fn primary_source_reference(source: &str) -> PredictionBasisReferenceV4 {
    PredictionBasisReferenceV4::v2(PredictionBasisReferenceV2::v1(
        PredictionBasisReferenceV1::primary_source(source)
            .expect("static primary-source id is valid"),
    ))
}

fn setting_reference(setting: SettingIdV2) -> PredictionBasisReferenceV4 {
    PredictionBasisReferenceV4::v2(PredictionBasisReferenceV2::v1(
        PredictionBasisReferenceV1::resolved_setting(
            ResolvedSettingLocationV1::Document,
            setting.as_str(),
        )
        .expect("static setting id is valid"),
    ))
}

fn selected_scope(selector: &str) -> EvaluationScope {
    EvaluationScope::new(EvaluationScopeCode::custom(SELECTED_NODE_SCOPE))
        .subject(format!("selector:{selector}"))
}

fn selected_scene_scope(
    selector: &str,
    source_scene_index: u64,
    node_index: usize,
) -> EvaluationScope {
    EvaluationScope::new(EvaluationScopeCode::custom(SELECTED_NODE_SCOPE)).subject(format!(
        "selector:{selector}:source_scene:{source_scene_index}:source_node:{node_index}"
    ))
}

fn budget_scope() -> EvaluationScope {
    EvaluationScope::new(EvaluationScopeCode::custom(
        "engine-unit-scale:facet-budget",
    ))
}

fn custom_reason(code: &'static str) -> PredictionUnavailableReasonV2 {
    PredictionUnavailableReasonV2::custom(code).expect("static reason code is valid")
}

fn classification_name(classification: LinearTransformClassification) -> &'static str {
    match classification {
        LinearTransformClassification::UnitOrthonormal => "unit_orthonormal",
        LinearTransformClassification::UniformScaled => "uniform_scaled",
        LinearTransformClassification::NonUniform => "non_uniform",
        LinearTransformClassification::Sheared => "sheared",
        LinearTransformClassification::Reflected => "reflected",
        LinearTransformClassification::Singular => "singular",
        LinearTransformClassification::NonFinite => "non_finite",
        _ => "unknown",
    }
}

fn load_meshes(provenance: &PredictionProvenanceV4) -> bool {
    matches!(
        provenance
            .settings()
            .document_setting(EngineSettingIdV2::LoadMeshes)
            .map(|setting| setting.value()),
        Some(EngineSettingValueV2::Token(value)) if value == "nonempty"
    )
}

fn is_bevy_tuple(provenance: &PredictionProvenanceV4) -> bool {
    let selection = provenance.profile().selection();
    selection.family() == BEVY_FAMILY
        && selection.profile_revision() == BEVY_PROFILE_REVISION
        && selection.engine_version() == BEVY_ENGINE_VERSION
        && selection.importer() == BEVY_IMPORTER
}

fn has_frozen_bevy_profile(provenance: &PredictionProvenanceV4) -> bool {
    profiles_v2().iter().any(|profile| {
        let selection = profile.selection();
        selection.family() == BEVY_FAMILY
            && selection.profile_revision() == BEVY_PROFILE_REVISION
            && selection.engine_version() == BEVY_ENGINE_VERSION
            && selection.importer() == BEVY_IMPORTER
            && crate::project_engine_profile_v2(profile)
                .is_ok_and(|projected| &projected == provenance.profile())
    })
}

fn is_exact_bevy_gltf(source: &LoadedSource, provenance: &PredictionProvenanceV4) -> bool {
    matches!(
        source.source_facts().format(),
        SourceFormatV1::GltfJson | SourceFormatV1::Glb
    ) && source.source_facts().format() == provenance.source_format()
        && is_bevy_tuple(provenance)
        && has_frozen_bevy_profile(provenance)
}

fn empty_output() -> CheckOutput {
    CheckOutput::from_coverage(Vec::new(), Vec::new(), Vec::new())
}
