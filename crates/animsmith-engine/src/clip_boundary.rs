//! Exact Unreal FBX animation-stack boundary prediction.

use crate::error::PredictionRuleError;
use crate::registry::profiles_v1;
use animsmith_core::{
    Applicability, Check, CheckCtx, CheckOutput, EngineFactIdV1, EngineFactStateV1,
    EngineFactValueV1, EnginePredictionBasisV2, EnginePredictionFacetV3, EnginePredictionV3,
    EvaluationScope, EvaluationScopeCode, ExactSourceTimingBasisReferenceV1,
    ExactSourceTimingDomainV1, ExactSourceTimingKeyV1, ExactSourceTimingObservationStateV1,
    Finding, LoadedSource, PredictionBasisReferenceV1, PredictionBasisReferenceV2,
    PredictionFacetDemandV2, PredictionProvenanceV3, PredictionRuleAllocationV2,
    PredictionUnavailableReasonV2, RawSourceBindingV2, RawSourceFieldIdV1, Severity,
    SourceFormatV1, SourceSetCoverageStateV1,
};

/// Stable id for the exact Unreal FBX animation-stack boundary check.
pub const ENGINE_CLIP_BOUNDARY_CHECK_ID: &str = "engine-clip-boundary";

const UNREAL_FAMILY: &str = "unreal";
const UNREAL_PROFILE_REVISION: u32 = 1;
const UNREAL_ENGINE_VERSION: &str = "5.8";
const UNREAL_IMPORTER: &str = "fbx-importer";
const UNREAL_BOUNDARY_SOURCE: &str = "unreal-animation-sequences-5.8";
const FACET_LIMIT: usize = animsmith_core::PREDICTION_V2_MAX_CANDIDATE_FACETS_PER_RULE;

/// Engine-owned Unreal 5.8 check over exact same-load FBX stack timing.
pub struct EngineClipBoundaryCheck<'a> {
    source: &'a LoadedSource,
    provenance: Option<&'a PredictionProvenanceV3>,
}

impl<'a> EngineClipBoundaryCheck<'a> {
    /// Bind an optional exact-profile provenance record to the same loaded source.
    pub fn new(
        source: &'a LoadedSource,
        provenance: Option<&'a PredictionProvenanceV3>,
    ) -> Result<Self, PredictionRuleError> {
        if let Some(provenance) = provenance {
            provenance
                .validate()
                .map_err(|_| PredictionRuleError::SourceProvenanceMismatch)?;
            let raw_source = RawSourceBindingV2::from_source(
                source.source_facts(),
                source.exact_source_timing(),
            )
            .map_err(|_| PredictionRuleError::SourceProvenanceMismatch)?;
            if &raw_source != provenance.raw_source()
                || source.dependency_closure() != provenance.dependency_closure()
            {
                return Err(PredictionRuleError::SourceProvenanceMismatch);
            }
            if is_unreal_tuple(provenance) && !has_frozen_unreal_profile(provenance) {
                return Err(PredictionRuleError::FrozenProfileMismatch);
            }
        }
        Ok(Self { source, provenance })
    }
}

impl Check for EngineClipBoundaryCheck<'_> {
    fn id(&self) -> &'static str {
        ENGINE_CLIP_BOUNDARY_CHECK_ID
    }

    fn applicability(&self, _ctx: &CheckCtx<'_>) -> Applicability {
        match self.provenance {
            Some(provenance)
                if is_exact_unreal_fbx(self.source, provenance)
                    && !complete_empty_stack_inventory(self.source) =>
            {
                Applicability::Applicable
            }
            _ => Applicability::NotApplicable,
        }
    }

    fn evaluate(&self, _ctx: &CheckCtx<'_>) -> CheckOutput {
        let Some(provenance) = self.provenance else {
            return empty_output();
        };
        if !is_exact_unreal_fbx(self.source, provenance)
            || complete_empty_stack_inventory(self.source)
        {
            return empty_output();
        }
        let demand = facet_demand(self.source);
        let (capacity, summary_required) = match demand {
            PredictionFacetDemandV2::Exact(count) => (count, false),
            PredictionFacetDemandV2::NPlusOne => (FACET_LIMIT.saturating_sub(1), true),
        };
        evaluate_allocated(self.source, provenance, capacity, summary_required)
    }

    fn prediction_facet_demand_v2(&self, _ctx: &CheckCtx<'_>) -> PredictionFacetDemandV2 {
        match self.provenance {
            Some(provenance)
                if is_exact_unreal_fbx(self.source, provenance)
                    && !complete_empty_stack_inventory(self.source) =>
            {
                facet_demand(self.source)
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
        if !is_exact_unreal_fbx(self.source, provenance)
            || complete_empty_stack_inventory(self.source)
        {
            return empty_output();
        }
        evaluate_allocated(
            self.source,
            provenance,
            allocation.candidate_capacity(),
            allocation.summary_required(),
        )
    }
}

fn facet_demand(source: &LoadedSource) -> PredictionFacetDemandV2 {
    let rows = source.source_facts().clips().rows().len();
    let inventory = usize::from(
        source.source_facts().clips().coverage().state() != SourceSetCoverageStateV1::Complete,
    );
    match rows.checked_add(inventory) {
        Some(count) if count <= FACET_LIMIT => PredictionFacetDemandV2::Exact(count),
        _ => PredictionFacetDemandV2::NPlusOne,
    }
}

fn evaluate_allocated(
    source: &LoadedSource,
    provenance: &PredictionProvenanceV3,
    candidate_capacity: usize,
    summary_required: bool,
) -> CheckOutput {
    let inventory_incomplete =
        source.source_facts().clips().coverage().state() != SourceSetCoverageStateV1::Complete;
    // An incomplete inventory must retain one unsuppressible summary whenever
    // a candidate slot exists. Row candidates consume only the remainder.
    let inventory_slots = usize::from(inventory_incomplete && candidate_capacity != 0);
    let row_capacity = candidate_capacity.saturating_sub(inventory_slots);
    let mut findings = Vec::new();
    let mut evaluated_scopes = Vec::with_capacity(row_capacity);
    let mut facets =
        Vec::with_capacity(candidate_capacity.saturating_add(usize::from(summary_required)));

    let timing = source.exact_source_timing();
    for source_clip_index in 0..source.source_facts().clips().rows().len().min(row_capacity) {
        let scope = stack_scope(source_clip_index);
        let row = timing.and_then(|timing| timing.clips().get(source_clip_index));
        let declared_time_mode = timing.map(|timing| timing.declared_time_mode().state());
        let frame_period = timing.map(|timing| timing.frame_period().state());
        let tick_range = row.map(|row| row.source_time_range().state());
        let basis = stack_basis(
            provenance,
            source_clip_index,
            declared_time_mode,
            frame_period,
            tick_range,
        );

        match (declared_time_mode, frame_period, tick_range) {
            (
                Some(ExactSourceTimingObservationStateV1::Observed(_)),
                Some(ExactSourceTimingObservationStateV1::Observed(period)),
                Some(ExactSourceTimingObservationStateV1::Observed(range)),
            ) => {
                facets.push(
                    EnginePredictionFacetV3::available(scope.clone(), basis)
                        .expect("exact stack evidence forms an available facet"),
                );
                evaluated_scopes.push(scope.clone());
                if range.end_units().rem_euclid(period.units_per_frame()) != 0 {
                    findings.push(
                        Finding::new(
                            ENGINE_CLIP_BOUNDARY_CHECK_ID,
                            Severity::Warning,
                            format!(
                                "FBX animation stack {source_clip_index} ends at KTime tick {}, which is not on the exact {}-tick frame lattice required by Unreal Engine 5.8",
                                range.end_units(),
                                period.units_per_frame()
                            ),
                        )
                        .prediction_scope(scope),
                    );
                }
            }
            _ => facets.push(
                EnginePredictionFacetV3::required_unavailable(
                    scope,
                    basis,
                    unavailable_reasons(declared_time_mode, frame_period, tick_range),
                )
                .expect("missing exact stack evidence forms required-unavailable"),
            ),
        }
    }

    if inventory_slots != 0 {
        facets.push(
            EnginePredictionFacetV3::required_unavailable(
                inventory_scope(),
                inventory_basis(provenance),
                vec![PredictionUnavailableReasonV2::RawSourceIncomplete],
            )
            .expect("partial stack inventory forms required-unavailable"),
        );
    }

    if summary_required {
        facets.push(
            EnginePredictionFacetV3::required_unavailable(
                EvaluationScope::new(EvaluationScopeCode::custom(
                    "engine-clip-boundary:facet-budget",
                )),
                inventory_basis(provenance),
                vec![PredictionUnavailableReasonV2::FacetBudgetExceeded],
            )
            .expect("facet-budget summary is valid"),
        );
    }

    if facets.is_empty() {
        return empty_output();
    }
    let prediction = EnginePredictionV3::new(provenance.identity().clone(), facets)
        .expect("allocated clip-boundary facets satisfy the shared bound");
    CheckOutput::from_coverage(findings, evaluated_scopes, Vec::new())
        .with_engine_prediction_v3(prediction)
}

fn stack_basis<T, U, V>(
    provenance: &PredictionProvenanceV3,
    source_clip_index: usize,
    declared_time_mode: Option<&ExactSourceTimingObservationStateV1<T>>,
    frame_period: Option<&ExactSourceTimingObservationStateV1<U>>,
    tick_range: Option<&ExactSourceTimingObservationStateV1<V>>,
) -> EnginePredictionBasisV2 {
    let mut references = common_basis();
    let Some(binding) = provenance.raw_source().exact_source_timing() else {
        return EnginePredictionBasisV2::new(references).expect("static basis is valid");
    };
    references.push(exact_reference(
        ExactSourceTimingDomainV1::Document,
        ExactSourceTimingKeyV1::Document,
        "declared_time_mode.state",
        binding,
    ));
    references.push(exact_reference(
        ExactSourceTimingDomainV1::Document,
        ExactSourceTimingKeyV1::Document,
        "frame_period.state",
        binding,
    ));
    references.push(exact_reference(
        ExactSourceTimingDomainV1::Clip,
        ExactSourceTimingKeyV1::Clip {
            source_clip_index: source_clip_index as u64,
        },
        "source_time_range.state",
        binding,
    ));
    if matches!(
        declared_time_mode,
        Some(ExactSourceTimingObservationStateV1::Observed(_))
    ) {
        references.push(exact_reference(
            ExactSourceTimingDomainV1::Document,
            ExactSourceTimingKeyV1::Document,
            "declared_time_mode.value.time_mode",
            binding,
        ));
    }
    if matches!(
        frame_period,
        Some(ExactSourceTimingObservationStateV1::Observed(_))
    ) {
        references.push(exact_reference(
            ExactSourceTimingDomainV1::Document,
            ExactSourceTimingKeyV1::Document,
            "frame_period.value.units_per_frame",
            binding,
        ));
    }
    if matches!(
        tick_range,
        Some(ExactSourceTimingObservationStateV1::Observed(_))
    ) {
        references.push(exact_reference(
            ExactSourceTimingDomainV1::Clip,
            ExactSourceTimingKeyV1::Clip {
                source_clip_index: source_clip_index as u64,
            },
            "source_time_range.value.end_units",
            binding,
        ));
    }
    EnginePredictionBasisV2::new(references).expect("exact timing basis is valid")
}

fn inventory_basis(provenance: &PredictionProvenanceV3) -> EnginePredictionBasisV2 {
    let mut references = common_basis();
    if let Some(binding) = provenance.raw_source().exact_source_timing() {
        references.push(exact_reference(
            ExactSourceTimingDomainV1::Document,
            ExactSourceTimingKeyV1::Document,
            "clip_coverage.state",
            binding,
        ));
        references.push(exact_reference(
            ExactSourceTimingDomainV1::Document,
            ExactSourceTimingKeyV1::Document,
            "clip_coverage.reason",
            binding,
        ));
    }
    EnginePredictionBasisV2::new(references).expect("inventory basis is valid")
}

fn common_basis() -> Vec<PredictionBasisReferenceV2> {
    vec![
        PredictionBasisReferenceV2::v1(
            PredictionBasisReferenceV1::profile_fact("whole_end_frame_required")
                .expect("static fact id is valid"),
        ),
        PredictionBasisReferenceV2::v1(
            PredictionBasisReferenceV1::primary_source(UNREAL_BOUNDARY_SOURCE)
                .expect("static source id is valid"),
        ),
    ]
}

fn exact_reference(
    domain: ExactSourceTimingDomainV1,
    key: ExactSourceTimingKeyV1,
    field: &'static str,
    binding: &animsmith_core::ExactSourceTimingBindingV1,
) -> PredictionBasisReferenceV2 {
    PredictionBasisReferenceV2::exact_source_timing(
        ExactSourceTimingBasisReferenceV1::from_binding(
            domain,
            key,
            RawSourceFieldIdV1::new(field).expect("static exact field id is valid"),
            binding,
        )
        .expect("same-load exact field resolves"),
    )
}

fn unavailable_reasons<T, U, V>(
    declared_time_mode: Option<&ExactSourceTimingObservationStateV1<T>>,
    frame_period: Option<&ExactSourceTimingObservationStateV1<U>>,
    tick_range: Option<&ExactSourceTimingObservationStateV1<V>>,
) -> Vec<PredictionUnavailableReasonV2> {
    if declared_time_mode.is_none() && frame_period.is_none() && tick_range.is_none() {
        return vec![custom_reason("animsmith:exact_source_timing_unavailable")];
    }
    let mut reasons = Vec::new();
    if !matches!(
        declared_time_mode,
        Some(ExactSourceTimingObservationStateV1::Observed(_))
    ) {
        reasons.push(custom_reason(
            "animsmith:source_declared_time_mode_unavailable",
        ));
    }
    if !matches!(
        frame_period,
        Some(ExactSourceTimingObservationStateV1::Observed(_))
    ) {
        reasons.push(custom_reason("animsmith:source_frame_period_unavailable"));
    }
    if !matches!(
        tick_range,
        Some(ExactSourceTimingObservationStateV1::Observed(_))
    ) {
        reasons.push(custom_reason(
            "animsmith:source_clip_time_range_unavailable",
        ));
    }
    reasons
}

fn custom_reason(code: &'static str) -> PredictionUnavailableReasonV2 {
    PredictionUnavailableReasonV2::custom(code).expect("static reason code is valid")
}

fn stack_scope(source_clip_index: usize) -> EvaluationScope {
    EvaluationScope::new(EvaluationScopeCode::ENGINE_CLIP_BOUNDARY)
        .subject(format!("source_stack:{source_clip_index}"))
}

fn inventory_scope() -> EvaluationScope {
    EvaluationScope::new(EvaluationScopeCode::ENGINE_CLIP_BOUNDARY_INVENTORY)
}

fn empty_output() -> CheckOutput {
    CheckOutput::from_coverage(Vec::new(), Vec::new(), Vec::new())
}

fn complete_empty_stack_inventory(source: &LoadedSource) -> bool {
    let clips = source.source_facts().clips();
    clips.coverage().state() == SourceSetCoverageStateV1::Complete && clips.rows().is_empty()
}

fn is_unreal_tuple(provenance: &PredictionProvenanceV3) -> bool {
    let selection = provenance.profile().selection();
    selection.family() == UNREAL_FAMILY
        && selection.profile_revision() == UNREAL_PROFILE_REVISION
        && selection.engine_version() == UNREAL_ENGINE_VERSION
        && selection.importer() == UNREAL_IMPORTER
}

fn has_frozen_unreal_profile(provenance: &PredictionProvenanceV3) -> bool {
    profiles_v1().iter().any(|profile| {
        let selection = profile.selection();
        selection.family() == UNREAL_FAMILY
            && selection.profile_revision() == UNREAL_PROFILE_REVISION
            && selection.engine_version() == UNREAL_ENGINE_VERSION
            && selection.importer() == UNREAL_IMPORTER
            && profile.facts_identity() == provenance.profile().facts_identity()
    })
}

fn is_exact_unreal_fbx(source: &LoadedSource, provenance: &PredictionProvenanceV3) -> bool {
    source.source_facts().format() == SourceFormatV1::Fbx
        && provenance.source_format() == SourceFormatV1::Fbx
        && is_unreal_tuple(provenance)
        && has_frozen_unreal_profile(provenance)
        && matches!(
            provenance
                .profile()
                .fact(EngineFactIdV1::WholeEndFrameRequired)
                .map(|fact| fact.state()),
            Some(EngineFactStateV1::Known(EngineFactValueV1::Boolean(true)))
        )
        && provenance
            .profile()
            .source(UNREAL_BOUNDARY_SOURCE)
            .is_some()
}
