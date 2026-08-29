//! Exact negative-only Bevy animation/channel import-gate prediction.

use crate::SettingIdV2;
use crate::canonical::matches_frozen_registry_projection_v2;
use crate::error::PredictionRuleError;
use animsmith_core::engine_contract::{EngineSettingIdV2, EngineSettingValueV2};
use animsmith_core::prediction::{EnginePredictionBasisV4, PredictionBasisReferenceV4};
use animsmith_core::{
    Applicability, Check, CheckCtx, CheckOutput, EngineMachineResultV1, EnginePredictionFacetV4,
    EnginePredictionV4, EnginePredictionV5, EvaluationScope, EvaluationScopeCode, LoadedSource,
    PREDICTION_V2_MAX_CANDIDATE_FACETS_PER_RULE, PredictionBasisReferenceV1,
    PredictionBasisReferenceV2, PredictionFacetDemandV2, PredictionProvenanceV5,
    PredictionRuleAllocationV2, PredictionScalarV1, PredictionUnavailableReasonV2,
    RawAnimationChannelInventoryV1, ResolvedSettingLocationV1, SourceFormatV1,
    SourceImportDispositionResultV1, SourceImportDispositionV1, SourceImportSubjectKindV1,
};

/// Stable id for the exact Bevy raw animation/channel gate prediction.
pub const ENGINE_TRACK_SUPPORT_CHECK_ID: &str = "engine-track-support";

const BEVY_FAMILY: &str = "bevy";
const BEVY_PROFILE_REVISION: u32 = 3;
const BEVY_ENGINE_VERSION: &str = "0.19.0";
const BEVY_IMPORTER: &str = "gltf-asset-loader";
const BEVY_LOADER_SOURCE: &str = "bevy-gltf-loader-0.19.0-c6f634ca";
const BEVY_MANIFEST_SOURCE: &str = "bevy-feature-manifest-0.19.0-c6f634ca";

const ANIMATION_SCOPE: &str = "engine-track-support:animation";
const CHANNEL_SCOPE: &str = "engine-track-support:animation-channel";
const INVENTORY_SCOPE: &str = "engine-track-support:inventory";
const BUDGET_SCOPE: &str = "engine-track-support:facet-budget";

/// Engine-owned check for Bevy revision 3's two materialized loading gates.
pub struct EngineTrackSupportCheck<'a> {
    source: &'a LoadedSource,
    provenance: Option<&'a PredictionProvenanceV5>,
}

impl<'a> EngineTrackSupportCheck<'a> {
    /// Bind a V5 sidecar to the exact same source load.
    pub fn new(
        source: &'a LoadedSource,
        provenance: Option<&'a PredictionProvenanceV5>,
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
            if provenance.base().raw_source() != &raw_source
                || provenance.raw_animation_channels()
                    != &RawAnimationChannelInventoryV1::from_source(source.source_facts())
                || provenance.base().dependency_closure() != source.dependency_closure()
            {
                return Err(PredictionRuleError::SourceProvenanceMismatch);
            }
            if is_bevy_tuple(provenance)
                && !matches_frozen_registry_projection_v2(provenance.base().profile())
            {
                return Err(PredictionRuleError::FrozenProfileMismatch);
            }
        }
        Ok(Self { source, provenance })
    }
}

impl Check for EngineTrackSupportCheck<'_> {
    fn id(&self) -> &'static str {
        ENGINE_TRACK_SUPPORT_CHECK_ID
    }

    fn applicability(&self, _ctx: &CheckCtx<'_>) -> Applicability {
        self.provenance
            .filter(|provenance| {
                is_exact_bevy_gltf(self.source, provenance)
                    && !provenance.raw_animation_channels().is_complete_empty()
            })
            .map_or(Applicability::NotApplicable, |_| Applicability::Applicable)
    }

    fn evaluate(&self, _ctx: &CheckCtx<'_>) -> CheckOutput {
        let Some(provenance) = self.provenance else {
            return empty_output();
        };
        if !is_exact_bevy_gltf(self.source, provenance)
            || provenance.raw_animation_channels().is_complete_empty()
        {
            return empty_output();
        }
        let demand = demand(provenance.raw_animation_channels());
        let (capacity, summary_required) = match demand {
            PredictionFacetDemandV2::Exact(count) => (count, false),
            PredictionFacetDemandV2::NPlusOne => (
                PREDICTION_V2_MAX_CANDIDATE_FACETS_PER_RULE.saturating_sub(1),
                true,
            ),
        };
        evaluate_allocated(provenance, capacity, summary_required)
    }

    fn prediction_facet_demand_v2(&self, _ctx: &CheckCtx<'_>) -> PredictionFacetDemandV2 {
        self.provenance
            .filter(|provenance| {
                is_exact_bevy_gltf(self.source, provenance)
                    && !provenance.raw_animation_channels().is_complete_empty()
            })
            .map_or(PredictionFacetDemandV2::Exact(0), |provenance| {
                demand(provenance.raw_animation_channels())
            })
    }

    fn evaluate_with_prediction_allocation_v2(
        &self,
        _ctx: &CheckCtx<'_>,
        allocation: PredictionRuleAllocationV2<'_>,
    ) -> CheckOutput {
        let Some(provenance) = self.provenance else {
            return empty_output();
        };
        if !is_exact_bevy_gltf(self.source, provenance)
            || provenance.raw_animation_channels().is_complete_empty()
        {
            return empty_output();
        }
        evaluate_allocated(
            provenance,
            allocation.candidate_capacity(),
            allocation.summary_required(),
        )
    }
}

fn demand(inventory: &RawAnimationChannelInventoryV1) -> PredictionFacetDemandV2 {
    if inventory.is_complete_empty() {
        return PredictionFacetDemandV2::Exact(0);
    }
    if !inventory.source_coverage_complete() {
        return PredictionFacetDemandV2::Exact(1);
    }
    if inventory.candidate_overflow() {
        return PredictionFacetDemandV2::NPlusOne;
    }
    usize::try_from(inventory.candidate_count())
        .ok()
        .filter(|count| *count <= PREDICTION_V2_MAX_CANDIDATE_FACETS_PER_RULE)
        .map_or(
            PredictionFacetDemandV2::NPlusOne,
            PredictionFacetDemandV2::Exact,
        )
}

fn evaluate_allocated(
    provenance: &PredictionProvenanceV5,
    candidate_capacity: usize,
    summary_required: bool,
) -> CheckOutput {
    let inventory = provenance.raw_animation_channels();
    let mut facets =
        Vec::with_capacity(candidate_capacity.saturating_add(usize::from(summary_required)));
    if !inventory.is_complete_empty() && candidate_capacity != 0 {
        if !inventory.source_coverage_complete() {
            facets.push(unavailable(
                EvaluationScope::new(EvaluationScopeCode::custom(INVENTORY_SCOPE)),
                inventory_basis(inventory),
                PredictionUnavailableReasonV2::RawSourceIncomplete,
            ));
        } else {
            for row in inventory.rows() {
                if facets.len() == candidate_capacity {
                    break;
                }
                if let Some(channel) = row.source_channel_index() {
                    facets.push(subject_facet(
                        channel_scope(row.source_animation_index(), channel),
                        row_basis(inventory, row.source_animation_index(), Some(channel)),
                        SourceImportSubjectKindV1::AnimationChannel,
                        gate(provenance),
                    ));
                } else {
                    facets.push(subject_facet(
                        animation_scope(row.source_animation_index()),
                        row_basis(inventory, row.source_animation_index(), None),
                        SourceImportSubjectKindV1::Animation,
                        gate(provenance),
                    ));
                }
            }
        }
    }
    if summary_required {
        facets.push(unavailable(
            EvaluationScope::new(EvaluationScopeCode::custom(BUDGET_SCOPE)),
            static_basis(),
            PredictionUnavailableReasonV2::FacetBudgetExceeded,
        ));
    }
    let evaluated_scopes = facets
        .iter()
        .filter(|facet| facet.result().is_some())
        .map(|facet| facet.scope().clone())
        .collect();
    let inner = EnginePredictionV4::new(provenance.base().identity().clone(), facets)
        .expect("bounded track-support facets satisfy V4");
    let prediction = EnginePredictionV5::new(provenance, inner)
        .expect("track-support V5 prediction binds its V5 provenance");
    CheckOutput::from_coverage(Vec::new(), evaluated_scopes, Vec::new())
        .with_engine_prediction_v5(prediction)
}

fn subject_facet(
    scope: EvaluationScope,
    basis: EnginePredictionBasisV4,
    subject_kind: SourceImportSubjectKindV1,
    controlling_gate: Option<EngineSettingIdV2>,
) -> EnginePredictionFacetV4 {
    match controlling_gate {
        Some(gate) => EnginePredictionFacetV4::available(
            scope,
            basis,
            EngineMachineResultV1::SourceImportDisposition(SourceImportDispositionResultV1 {
                subject_kind,
                disposition: SourceImportDispositionV1::Dropped,
                controlling_gate: Some(gate),
            }),
        )
        .expect("negative Bevy gate result is valid"),
        None => unavailable(
            scope,
            basis,
            PredictionUnavailableReasonV2::RuntimeAnimationSurvivalUnavailable,
        ),
    }
}

fn unavailable(
    scope: EvaluationScope,
    basis: EnginePredictionBasisV4,
    reason: PredictionUnavailableReasonV2,
) -> EnginePredictionFacetV4 {
    EnginePredictionFacetV4::required_unavailable(scope, basis, vec![reason])
        .expect("track-support unavailable result is valid")
}

fn gate(provenance: &PredictionProvenanceV5) -> Option<EngineSettingIdV2> {
    let settings = provenance.base().settings();
    if matches!(
        settings
            .document_setting(EngineSettingIdV2::BevyAnimationFeature)
            .map(|row| row.value()),
        Some(EngineSettingValueV2::Boolean(false))
    ) {
        Some(EngineSettingIdV2::BevyAnimationFeature)
    } else if matches!(
        settings
            .document_setting(EngineSettingIdV2::LoadAnimations)
            .map(|row| row.value()),
        Some(EngineSettingValueV2::Boolean(false))
    ) {
        Some(EngineSettingIdV2::LoadAnimations)
    } else {
        None
    }
}

fn static_basis() -> EnginePredictionBasisV4 {
    let mut references = vec![
        profile_fact_reference(),
        primary_source_reference(BEVY_LOADER_SOURCE),
        primary_source_reference(BEVY_MANIFEST_SOURCE),
    ];
    references.push(setting_reference(SettingIdV2::BevyAnimationFeature));
    references.push(setting_reference(SettingIdV2::LoadAnimations));
    EnginePredictionBasisV4::new(references).expect("static track-support basis is valid")
}

fn inventory_basis(inventory: &RawAnimationChannelInventoryV1) -> EnginePredictionBasisV4 {
    let mut references = static_basis().references().to_vec();
    references.push(project_reference(
        "raw_animation_channel_inventory.animation_coverage",
        PredictionScalarV1::text(inventory_coverage_name(inventory))
            .expect("static coverage token"),
    ));
    references.push(project_reference(
        "raw_animation_channel_inventory.source_coverage_complete",
        PredictionScalarV1::Boolean {
            value: inventory.source_coverage_complete(),
        },
    ));
    if let Some(row) = inventory.rows().iter().find(|row| {
        row.channel_coverage().is_some_and(|coverage| {
            coverage.state() != animsmith_core::RawSourceSetCoverageStateV1::Complete
        })
    }) {
        let coverage = row.channel_coverage().expect("matched animation row");
        references.push(project_reference(
            "raw_animation_channel_inventory.incomplete_channel_animation_row",
            PredictionScalarV1::UnsignedInteger {
                value: row.source_animation_index(),
            },
        ));
        references.push(project_reference(
            "raw_animation_channel_inventory.incomplete_channel_coverage",
            PredictionScalarV1::text(coverage_state_name(coverage.state()))
                .expect("static coverage token"),
        ));
        if let Some(reason) = coverage.reason() {
            let reason = serde_json::to_value(reason).expect("coverage reason serializes");
            references.push(project_reference(
                "raw_animation_channel_inventory.incomplete_channel_reason",
                PredictionScalarV1::text(reason.as_str().expect("coverage reason is a token"))
                    .expect("static reason token"),
            ));
        }
    }
    EnginePredictionBasisV4::new(references).expect("inventory basis is valid")
}

fn row_basis(
    inventory: &RawAnimationChannelInventoryV1,
    animation: u64,
    channel: Option<u64>,
) -> EnginePredictionBasisV4 {
    let mut references = inventory_basis(inventory).references().to_vec();
    references.push(project_reference(
        "raw_animation_channel_inventory.animation_row",
        PredictionScalarV1::UnsignedInteger { value: animation },
    ));
    if let Some(channel) = channel {
        references.push(project_reference(
            "raw_animation_channel_inventory.channel_row",
            PredictionScalarV1::UnsignedInteger { value: channel },
        ));
    }
    EnginePredictionBasisV4::new(references).expect("row basis is valid")
}

fn project_reference(field: &'static str, value: PredictionScalarV1) -> PredictionBasisReferenceV4 {
    PredictionBasisReferenceV4::v2(PredictionBasisReferenceV2::v1(
        PredictionBasisReferenceV1::project_field(field, value).expect("static inventory field id"),
    ))
}

fn profile_fact_reference() -> PredictionBasisReferenceV4 {
    PredictionBasisReferenceV4::v2(PredictionBasisReferenceV2::v1(
        PredictionBasisReferenceV1::profile_fact("source_import_disposition")
            .expect("static fact id"),
    ))
}

fn primary_source_reference(source: &'static str) -> PredictionBasisReferenceV4 {
    PredictionBasisReferenceV4::v2(PredictionBasisReferenceV2::v1(
        PredictionBasisReferenceV1::primary_source(source).expect("static source id"),
    ))
}

fn setting_reference(setting: SettingIdV2) -> PredictionBasisReferenceV4 {
    PredictionBasisReferenceV4::v2(PredictionBasisReferenceV2::v1(
        PredictionBasisReferenceV1::resolved_setting(
            ResolvedSettingLocationV1::Document,
            setting.as_str(),
        )
        .expect("static setting id"),
    ))
}

fn animation_scope(animation: u64) -> EvaluationScope {
    EvaluationScope::new(EvaluationScopeCode::custom(ANIMATION_SCOPE))
        .subject(format!("source_animation:{animation}"))
}

fn channel_scope(animation: u64, channel: u64) -> EvaluationScope {
    EvaluationScope::new(EvaluationScopeCode::custom(CHANNEL_SCOPE)).subject(format!(
        "source_animation:{animation}:source_channel:{channel}"
    ))
}

fn inventory_coverage_name(inventory: &RawAnimationChannelInventoryV1) -> &'static str {
    coverage_state_name(inventory.animation_coverage().state())
}

fn coverage_state_name(state: animsmith_core::RawSourceSetCoverageStateV1) -> &'static str {
    match state {
        animsmith_core::RawSourceSetCoverageStateV1::Complete => "complete",
        animsmith_core::RawSourceSetCoverageStateV1::Partial => "partial",
        animsmith_core::RawSourceSetCoverageStateV1::Unavailable => "unavailable",
    }
}

fn is_bevy_tuple(provenance: &PredictionProvenanceV5) -> bool {
    let selection = provenance.base().profile().selection();
    selection.family() == BEVY_FAMILY
        && selection.profile_revision() == BEVY_PROFILE_REVISION
        && selection.engine_version() == BEVY_ENGINE_VERSION
        && selection.importer() == BEVY_IMPORTER
}

fn is_exact_bevy_gltf(source: &LoadedSource, provenance: &PredictionProvenanceV5) -> bool {
    matches!(
        source.source_facts().format(),
        SourceFormatV1::GltfJson | SourceFormatV1::Glb
    ) && source.source_facts().format() == provenance.base().source_format()
        && is_bevy_tuple(provenance)
        && matches_frozen_registry_projection_v2(provenance.base().profile())
}

fn empty_output() -> CheckOutput {
    CheckOutput::from_coverage(Vec::new(), Vec::new(), Vec::new())
}
