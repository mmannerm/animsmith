//! Engine-owned checks derived from immutable profile and same-load source evidence.

use crate::addressability::{
    GltfAnimationAddressabilityBevyAdapterV1, GltfAnimationAddressabilityInventoryV1,
};
use crate::error::PredictionRuleError;
use crate::registry::profiles_v1;
use animsmith_core::{
    Applicability, Check, CheckCtx, CheckOutput, CheckSelection, EngineAnimationAddressabilityV1,
    EngineFactIdV1, EngineFactStateV1, EngineFactValueV1, EnginePredictionBasisV1,
    EnginePredictionFacetV1, EnginePredictionFacetV2, EnginePredictionV1, EnginePredictionV2,
    EvaluationScope, EvaluationScopeCode, LoadedSource, PredictionBasisReferenceV1,
    PredictionFacetDemandV2, PredictionProvenanceV1, PredictionProvenanceV2,
    PredictionRuleAllocationV2, PredictionUnavailableReasonV1, RAW_SOURCE_V1_MAX_CLIPS,
    RawSourceBasisReferenceV1, RawSourceBindingV1, RawSourceDomainV1, RawSourceFieldIdV1,
    RawSourceKeyV1, SourceFormatV1, SourceSetCoverageStateV1, evaluate_checks,
};

/// A Bevy animation asset-label index outside the bounded raw-source
/// animation inventory.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("source animation index {source_clip_index} is outside the V1 limit of {limit} animations")]
pub struct BevyAnimationAssetLabelError {
    /// Supplied source animation index.
    pub source_clip_index: usize,
    /// Exclusive upper bound for V1 source animation indices.
    pub limit: usize,
}

/// Typed failure while constructing the optional exact Bevy adapter for a
/// glTF animation-addressability document.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum GltfAnimationAddressabilityAdapterError {
    /// Same-load source binding or frozen-profile validation failed.
    #[error(transparent)]
    PredictionRule(#[from] PredictionRuleError),
    /// The existing engine-addressability check could not form a valid
    /// [`animsmith_core::CheckEvaluation`].
    #[error(transparent)]
    Evaluation(#[from] animsmith_core::EvaluationError),
    /// The standalone contract rejected inconsistent inventory, provenance,
    /// or check evidence.
    #[error(transparent)]
    Contract(#[from] crate::GltfAnimationAddressabilityError),
}

/// Stable check id for engine scene, animation, target, and runtime-label addressability.
pub const ENGINE_ADDRESSABILITY_CHECK_ID: &str = "engine-addressability";

/// Versioned engine-owned check ids callers may use for pre-I/O selection validation.
pub const ENGINE_CHECK_IDS_V1: &[&str] = &[ENGINE_ADDRESSABILITY_CHECK_ID];

const BEVY_FAMILY: &str = "bevy";
const BEVY_PROFILE_REVISION: u32 = 1;
const BEVY_ENGINE_VERSION: &str = "0.19.0";
const BEVY_IMPORTER: &str = "gltf-asset-loader";
const BEVY_ANIMATION_LABEL_SOURCE: &str = "bevy-gltf-asset-label-0.19.0";

/// One exact Bevy 0.19.0 `GltfAssetLabel::Animation(index)` display selector.
///
/// The source animation index is authoritative. Source names are optional,
/// non-unique metadata and never affect this value. Construction enforces the
/// same 4,096-row ceiling as raw-source facts, so the retained selector text is
/// bounded to `Animation0` through `Animation4095`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BevyAnimationAssetLabelV1 {
    source_clip_index: usize,
    display_selector: String,
}

impl BevyAnimationAssetLabelV1 {
    /// Construct the indexed selector for one retained source animation.
    ///
    /// # Errors
    ///
    /// Returns [`BevyAnimationAssetLabelError`] when the index cannot occur in
    /// the bounded V1 raw-source animation inventory.
    pub fn new(source_clip_index: usize) -> Result<Self, BevyAnimationAssetLabelError> {
        if source_clip_index >= RAW_SOURCE_V1_MAX_CLIPS {
            return Err(BevyAnimationAssetLabelError {
                source_clip_index,
                limit: RAW_SOURCE_V1_MAX_CLIPS,
            });
        }
        Ok(Self {
            source_clip_index,
            display_selector: format!("Animation{source_clip_index}"),
        })
    }

    /// Exact source animation index carried by this selector.
    pub const fn source_clip_index(&self) -> usize {
        self.source_clip_index
    }

    /// Exact Bevy display selector, such as `Animation0`.
    pub fn as_str(&self) -> &str {
        &self.display_selector
    }
}

/// The engine addressability check over borrowed same-load source evidence.
///
/// When provenance is absent or is not exactly the frozen Bevy 0.19.0 glTF profile, the check
/// records a stable not-applicable evaluation. Construction performs bounded in-memory
/// validation against the immutable source sidecars but no filesystem or parser work.
pub struct EngineAddressabilityCheck<'a> {
    source: &'a LoadedSource,
    provenance: Option<&'a PredictionProvenanceV1>,
}

/// Current-lint V2 engine addressability check. Standalone addressability
/// artifacts keep using [`EngineAddressabilityCheck`] and immutable V1 wire
/// evidence.
pub struct EngineAddressabilityCheckV2<'a> {
    source: &'a LoadedSource,
    provenance: Option<&'a PredictionProvenanceV2>,
}

impl<'a> EngineAddressabilityCheckV2<'a> {
    /// Bind current-lint V2 provenance to same-load source evidence.
    pub fn new(
        source: &'a LoadedSource,
        provenance: Option<&'a PredictionProvenanceV2>,
    ) -> Result<Self, PredictionRuleError> {
        if let Some(provenance) = provenance {
            provenance
                .validate()
                .map_err(|_| PredictionRuleError::SourceProvenanceMismatch)?;
            let raw_source = RawSourceBindingV1::from_source(source.source_facts());
            if &raw_source != provenance.raw_source()
                || source.dependency_closure() != provenance.dependency_closure()
            {
                return Err(PredictionRuleError::SourceProvenanceMismatch);
            }
            let selection = provenance.profile().selection();
            if selection.family() == BEVY_FAMILY
                && selection.profile_revision() == BEVY_PROFILE_REVISION
                && selection.engine_version() == BEVY_ENGINE_VERSION
                && selection.importer() == BEVY_IMPORTER
                && !profiles_v1().iter().any(|profile| {
                    let selection = profile.selection();
                    selection.family() == BEVY_FAMILY
                        && selection.profile_revision() == BEVY_PROFILE_REVISION
                        && selection.engine_version() == BEVY_ENGINE_VERSION
                        && selection.importer() == BEVY_IMPORTER
                        && profile.facts_identity() == provenance.profile().facts_identity()
                })
            {
                return Err(PredictionRuleError::FrozenProfileMismatch);
            }
        }
        Ok(Self { source, provenance })
    }
}

impl Check for EngineAddressabilityCheckV2<'_> {
    fn id(&self) -> &'static str {
        ENGINE_ADDRESSABILITY_CHECK_ID
    }

    fn applicability(&self, _ctx: &CheckCtx<'_>) -> Applicability {
        let Some(provenance) = self.provenance else {
            return Applicability::NotApplicable;
        };
        if !is_bevy_gltf_v2(self.source, provenance) || facts_are_complete_and_empty(self.source) {
            Applicability::NotApplicable
        } else {
            Applicability::Applicable
        }
    }

    fn evaluate(&self, _ctx: &CheckCtx<'_>) -> CheckOutput {
        let Some(provenance) = self.provenance else {
            return CheckOutput::from_coverage(Vec::new(), Vec::new(), Vec::new());
        };
        if !is_bevy_gltf_v2(self.source, provenance) || facts_are_complete_and_empty(self.source) {
            return CheckOutput::from_coverage(Vec::new(), Vec::new(), Vec::new());
        }
        evaluate_animation_asset_labels_v2(self.source, provenance)
    }

    fn prediction_facet_demand_v2(&self, _ctx: &CheckCtx<'_>) -> PredictionFacetDemandV2 {
        match self.provenance {
            Some(provenance)
                if is_bevy_gltf_v2(self.source, provenance)
                    && !facts_are_complete_and_empty(self.source) =>
            {
                // An incomplete raw/settings inventory produces one
                // unavailable inventory facet, not one facet per retained
                // row.  Its pre-evaluation reservation must match that
                // actual candidate work.
                if self.source.source_facts().clips().coverage().state()
                    != SourceSetCoverageStateV1::Complete
                    || matches!(
                        provenance.settings().clip_coverage().state(),
                        animsmith_core::ResolvedEngineSettingsCoverageStateV2::Partial
                    )
                {
                    PredictionFacetDemandV2::Exact(1)
                } else {
                    PredictionFacetDemandV2::Exact(self.source.source_facts().clips().rows().len())
                }
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
            return CheckOutput::from_coverage(Vec::new(), Vec::new(), Vec::new());
        };
        if !is_bevy_gltf_v2(self.source, provenance) || facts_are_complete_and_empty(self.source) {
            return CheckOutput::from_coverage(Vec::new(), Vec::new(), Vec::new());
        }
        evaluate_animation_asset_labels_v2_allocated(self.source, provenance, allocation)
    }
}

impl<'a> EngineAddressabilityCheck<'a> {
    /// Bind one source and optional immutable prediction provenance.
    ///
    /// # Errors
    ///
    /// Returns [`PredictionRuleError::SourceProvenanceMismatch`] when the
    /// provenance was not projected from the supplied same-load source facts
    /// and dependency closure, or [`PredictionRuleError::FrozenProfileMismatch`]
    /// when an exact Bevy tuple does not carry the frozen registry facts.
    pub fn new(
        source: &'a LoadedSource,
        provenance: Option<&'a PredictionProvenanceV1>,
    ) -> Result<Self, PredictionRuleError> {
        if let Some(provenance) = provenance {
            let raw_source = RawSourceBindingV1::from_source(source.source_facts());
            if &raw_source != provenance.raw_source()
                || source.dependency_closure() != provenance.dependency_closure()
            {
                return Err(PredictionRuleError::SourceProvenanceMismatch);
            }
            if is_bevy_tuple(provenance)
                && !profiles_v1().iter().any(|profile| {
                    let selection = profile.selection();
                    selection.family() == BEVY_FAMILY
                        && selection.profile_revision() == BEVY_PROFILE_REVISION
                        && selection.engine_version() == BEVY_ENGINE_VERSION
                        && selection.importer() == BEVY_IMPORTER
                        && profile.facts_identity() == provenance.profile().facts_identity()
                })
            {
                return Err(PredictionRuleError::FrozenProfileMismatch);
            }
        }
        Ok(Self { source, provenance })
    }
}

impl Check for EngineAddressabilityCheck<'_> {
    fn id(&self) -> &'static str {
        ENGINE_ADDRESSABILITY_CHECK_ID
    }

    fn applicability(&self, _ctx: &CheckCtx<'_>) -> Applicability {
        borrowed_applicability(self.source, self.provenance)
    }

    fn evaluate(&self, _ctx: &CheckCtx<'_>) -> CheckOutput {
        let Some(provenance) = self.provenance else {
            return CheckOutput::from_coverage(Vec::new(), Vec::new(), Vec::new());
        };
        if !is_bevy_gltf(self.source, provenance) || facts_are_complete_and_empty(self.source) {
            return CheckOutput::from_coverage(Vec::new(), Vec::new(), Vec::new());
        }
        evaluate_animation_asset_labels(self.source, provenance)
    }
}

/// Build the optional exact Bevy 0.19.0 adapter for one addressability document.
///
/// This is the single adapter assembly path: it binds the existing
/// [`EngineAddressabilityCheck`] to the supplied same-load source and
/// provenance, sends that one check through [`evaluate_checks`] exactly once,
/// then lets the standalone contract validate the unchanged resulting evaluation
/// against the neutral inventory. No-profile and valid non-Bevy provenance
/// produce `None` without adding another check or prediction lifecycle.
///
/// # Errors
///
/// Returns [`GltfAnimationAddressabilityAdapterError`] when same-load or
/// frozen-profile validation fails, the existing check cannot produce a valid
/// evaluation, or the standalone contract rejects inconsistent evidence.
pub fn build_bevy_animation_addressability_adapter_v1(
    source: &LoadedSource,
    inventory: &GltfAnimationAddressabilityInventoryV1,
    prediction_provenance: Option<PredictionProvenanceV1>,
    ctx: &CheckCtx<'_>,
) -> Result<Option<GltfAnimationAddressabilityBevyAdapterV1>, GltfAnimationAddressabilityAdapterError>
{
    let Some(prediction_provenance) = prediction_provenance else {
        return Ok(None);
    };
    let check = EngineAddressabilityCheck::new(source, Some(&prediction_provenance))?;
    if !is_bevy_gltf(source, &prediction_provenance) {
        return Ok(None);
    }

    let evaluation = {
        let checks: [Box<dyn Check + '_>; 1] = [Box::new(check)];
        let mut evaluations = evaluate_checks(ctx, &checks, CheckSelection::All)?;
        let evaluation = evaluations
            .pop()
            .expect("one valid static check catalog produces one evaluation");
        debug_assert!(evaluations.is_empty());
        evaluation
    };
    Ok(Some(GltfAnimationAddressabilityBevyAdapterV1::new(
        prediction_provenance,
        evaluation,
        inventory,
    )?))
}

fn borrowed_applicability(
    source: &LoadedSource,
    provenance: Option<&PredictionProvenanceV1>,
) -> Applicability {
    let Some(provenance) = provenance else {
        return Applicability::NotApplicable;
    };
    if !is_bevy_gltf(source, provenance) {
        return Applicability::NotApplicable;
    }
    if facts_are_complete_and_empty(source) {
        Applicability::NotApplicable
    } else {
        Applicability::Applicable
    }
}

fn facts_are_complete_and_empty(source: &LoadedSource) -> bool {
    let clips = source.source_facts().clips();
    clips.coverage().state() == SourceSetCoverageStateV1::Complete && clips.rows().is_empty()
}

fn is_bevy_gltf(source: &LoadedSource, provenance: &PredictionProvenanceV1) -> bool {
    let facts = source.source_facts();
    facts.format() == provenance.source_format()
        && is_bevy_tuple(provenance)
        && matches!(
            provenance.source_format(),
            SourceFormatV1::GltfJson | SourceFormatV1::Glb
        )
        && matches!(
            provenance
                .profile()
                .fact(EngineFactIdV1::AnimationAddressability)
                .map(|fact| fact.state()),
            Some(EngineFactStateV1::Known(
                EngineFactValueV1::AnimationAddressability(
                    EngineAnimationAddressabilityV1::GltfAssetLabel
                )
            ))
        )
        && provenance
            .profile()
            .source(BEVY_ANIMATION_LABEL_SOURCE)
            .is_some()
}

fn is_bevy_gltf_v2(source: &LoadedSource, provenance: &PredictionProvenanceV2) -> bool {
    let selection = provenance.profile().selection();
    source.source_facts().format() == provenance.source_format()
        && selection.family() == BEVY_FAMILY
        && selection.profile_revision() == BEVY_PROFILE_REVISION
        && selection.engine_version() == BEVY_ENGINE_VERSION
        && selection.importer() == BEVY_IMPORTER
        && matches!(
            provenance
                .profile()
                .fact(EngineFactIdV1::AnimationAddressability)
                .map(|fact| fact.state()),
            Some(EngineFactStateV1::Known(
                EngineFactValueV1::AnimationAddressability(
                    EngineAnimationAddressabilityV1::GltfAssetLabel
                )
            ))
        )
        && provenance
            .profile()
            .source(BEVY_ANIMATION_LABEL_SOURCE)
            .is_some()
}

fn is_bevy_tuple(provenance: &PredictionProvenanceV1) -> bool {
    let selection = provenance.profile().selection();
    selection.family() == BEVY_FAMILY
        && selection.profile_revision() == BEVY_PROFILE_REVISION
        && selection.engine_version() == BEVY_ENGINE_VERSION
        && selection.importer() == BEVY_IMPORTER
}

fn evaluate_animation_asset_labels(
    source: &LoadedSource,
    provenance: &PredictionProvenanceV1,
) -> CheckOutput {
    let facts = source.source_facts();
    let inventory_scope =
        EvaluationScope::new(EvaluationScopeCode::ANIMATION_ASSET_LABEL_INVENTORY);
    if facts.format() != provenance.source_format()
        || facts.clips().coverage().state() != SourceSetCoverageStateV1::Complete
    {
        return unavailable_inventory(provenance, inventory_scope);
    }

    let mut scopes = Vec::with_capacity(facts.clips().rows().len());
    let mut facets = Vec::with_capacity(facts.clips().rows().len());
    for clip in facts.clips().rows() {
        let source_index = clip.source_clip_index();
        let label = BevyAnimationAssetLabelV1::new(source_index)
            .expect("loader-valid retained clip indices satisfy the V1 raw-source bound");
        let scope = EvaluationScope::new(EvaluationScopeCode::ANIMATION_ASSET_LABEL)
            .subject(label.as_str().to_owned());
        let source_name = RawSourceBasisReferenceV1::from_source(
            RawSourceDomainV1::Clip,
            RawSourceKeyV1::Clip {
                source_clip_index: source_index as u64,
            },
            RawSourceFieldIdV1::new("source_name.state").expect("static field is valid"),
            facts,
        )
        .expect("loader-valid retained clip rows must resolve their own raw-source witness");
        let basis = EnginePredictionBasisV1::new(vec![
            PredictionBasisReferenceV1::profile_fact("animation_addressability")
                .expect("static fact id is valid"),
            PredictionBasisReferenceV1::primary_source(BEVY_ANIMATION_LABEL_SOURCE)
                .expect("static primary-source id is valid"),
            PredictionBasisReferenceV1::raw_source(source_name),
        ])
        .expect("three distinct static basis references are valid");
        let facet = EnginePredictionFacetV1::available(scope.clone(), basis)
            .expect("complete source rows have unique bounded scopes");
        scopes.push(scope);
        facets.push(facet);
    }

    if facets.is_empty() {
        return CheckOutput::from_coverage(Vec::new(), Vec::new(), Vec::new());
    }
    let prediction = EnginePredictionV1::new(provenance.identity().clone(), facets)
        .expect("complete bounded source rows form one valid canonical prediction");
    CheckOutput::from_coverage(Vec::new(), scopes, Vec::new()).with_engine_prediction(prediction)
}

fn unavailable_inventory(
    provenance: &PredictionProvenanceV1,
    scope: EvaluationScope,
) -> CheckOutput {
    let basis = EnginePredictionBasisV1::new(vec![
        PredictionBasisReferenceV1::profile_fact("animation_addressability")
            .expect("static fact id is valid"),
        PredictionBasisReferenceV1::primary_source(BEVY_ANIMATION_LABEL_SOURCE)
            .expect("static primary-source id is valid"),
    ])
    .expect("static basis is nonempty and canonical");
    let facet = EnginePredictionFacetV1::required_unavailable(
        scope,
        basis,
        vec![PredictionUnavailableReasonV1::RawSourceIncomplete],
    )
    .expect("static unavailable facet is valid");
    let prediction = EnginePredictionV1::new(provenance.identity().clone(), vec![facet])
        .expect("one static facet is valid");
    CheckOutput::from_coverage(Vec::new(), Vec::new(), Vec::new())
        .with_engine_prediction(prediction)
}

fn evaluate_animation_asset_labels_v2(
    source: &LoadedSource,
    provenance: &PredictionProvenanceV2,
) -> CheckOutput {
    let facts = source.source_facts();
    let inventory_scope =
        EvaluationScope::new(EvaluationScopeCode::ANIMATION_ASSET_LABEL_INVENTORY);
    let settings_overflow = matches!(
        provenance.settings().clip_coverage().state(),
        animsmith_core::ResolvedEngineSettingsCoverageStateV2::Partial
    );
    if facts.clips().coverage().state() != SourceSetCoverageStateV1::Complete || settings_overflow {
        let basis = EnginePredictionBasisV1::new(vec![
            PredictionBasisReferenceV1::profile_fact("animation_addressability")
                .expect("static fact id is valid"),
            PredictionBasisReferenceV1::primary_source(BEVY_ANIMATION_LABEL_SOURCE)
                .expect("static primary-source id is valid"),
        ])
        .expect("static basis is canonical");
        let mut reasons = Vec::new();
        if facts.clips().coverage().state() != SourceSetCoverageStateV1::Complete {
            reasons.push(animsmith_core::PredictionUnavailableReasonV2::RawSourceIncomplete);
        }
        if settings_overflow {
            reasons.push(animsmith_core::PredictionUnavailableReasonV2::ResolvedSettingsOverflow);
        }
        let facet = EnginePredictionFacetV2::required_unavailable(inventory_scope, basis, reasons)
            .expect("static unavailable V2 facet is valid");
        let prediction = EnginePredictionV2::new(provenance.identity().clone(), vec![facet])
            .expect("one static V2 facet is valid");
        return CheckOutput::from_coverage(Vec::new(), Vec::new(), Vec::new())
            .with_engine_prediction_v2(prediction);
    }

    let mut scopes = Vec::with_capacity(facts.clips().rows().len());
    let mut facets = Vec::with_capacity(facts.clips().rows().len());
    for clip in facts.clips().rows() {
        let source_index = clip.source_clip_index();
        let label = BevyAnimationAssetLabelV1::new(source_index)
            .expect("loader-valid retained clip indices satisfy the V1 raw-source bound");
        let scope = EvaluationScope::new(EvaluationScopeCode::ANIMATION_ASSET_LABEL)
            .subject(label.as_str().to_owned());
        let source_name = RawSourceBasisReferenceV1::from_source(
            RawSourceDomainV1::Clip,
            RawSourceKeyV1::Clip {
                source_clip_index: source_index as u64,
            },
            RawSourceFieldIdV1::new("source_name.state").expect("static field is valid"),
            facts,
        )
        .expect("retained clip rows resolve their raw-source witness");
        let basis = EnginePredictionBasisV1::new(vec![
            PredictionBasisReferenceV1::profile_fact("animation_addressability")
                .expect("static fact id is valid"),
            PredictionBasisReferenceV1::primary_source(BEVY_ANIMATION_LABEL_SOURCE)
                .expect("static primary-source id is valid"),
            PredictionBasisReferenceV1::raw_source(source_name),
        ])
        .expect("three static basis references are valid");
        facets.push(
            EnginePredictionFacetV2::available(scope.clone(), basis)
                .expect("complete rows form V2 facets"),
        );
        scopes.push(scope);
    }
    if facets.is_empty() {
        return CheckOutput::from_coverage(Vec::new(), Vec::new(), Vec::new());
    }
    let prediction = EnginePredictionV2::new(provenance.identity().clone(), facets)
        .expect("complete bounded source rows form V2 prediction");
    CheckOutput::from_coverage(Vec::new(), scopes, Vec::new()).with_engine_prediction_v2(prediction)
}

fn evaluate_animation_asset_labels_v2_allocated(
    source: &LoadedSource,
    provenance: &PredictionProvenanceV2,
    allocation: PredictionRuleAllocationV2<'_>,
) -> CheckOutput {
    let facts = source.source_facts();
    let settings_partial = matches!(
        provenance.settings().clip_coverage().state(),
        animsmith_core::ResolvedEngineSettingsCoverageStateV2::Partial
    );
    let raw_partial = facts.clips().coverage().state() != SourceSetCoverageStateV1::Complete;
    let mut scopes = Vec::with_capacity(allocation.candidate_capacity());
    let mut facets = Vec::with_capacity(
        allocation.candidate_capacity() + usize::from(allocation.summary_required()),
    );

    if raw_partial || settings_partial {
        // The incomplete-inventory representation has exactly one candidate.
        // When its capacity is zero, do not construct it only to discard it.
        if allocation.candidate_capacity() != 0 {
            let mut reasons = Vec::new();
            if raw_partial {
                reasons.push(animsmith_core::PredictionUnavailableReasonV2::RawSourceIncomplete);
            }
            if settings_partial {
                reasons
                    .push(animsmith_core::PredictionUnavailableReasonV2::ResolvedSettingsOverflow);
            }
            let basis = EnginePredictionBasisV1::new(vec![
                PredictionBasisReferenceV1::profile_fact("animation_addressability")
                    .expect("static fact"),
                PredictionBasisReferenceV1::primary_source(BEVY_ANIMATION_LABEL_SOURCE)
                    .expect("static source"),
            ])
            .expect("static basis");
            facets.push(
                EnginePredictionFacetV2::required_unavailable(
                    EvaluationScope::new(EvaluationScopeCode::ANIMATION_ASSET_LABEL_INVENTORY),
                    basis,
                    reasons,
                )
                .expect("static unavailable V2 facet"),
            );
        }
    } else {
        // Construct only the catalog-allocated prefix.  In particular, do not
        // form raw-source basis references/facets for omitted candidates.
        for clip in facts
            .clips()
            .rows()
            .iter()
            .take(allocation.candidate_capacity())
        {
            let source_index = clip.source_clip_index();
            let label = BevyAnimationAssetLabelV1::new(source_index)
                .expect("loader-valid retained clip indices satisfy the V1 raw-source bound");
            let scope = EvaluationScope::new(EvaluationScopeCode::ANIMATION_ASSET_LABEL)
                .subject(label.as_str().to_owned());
            let source_name = RawSourceBasisReferenceV1::from_source(
                RawSourceDomainV1::Clip,
                RawSourceKeyV1::Clip {
                    source_clip_index: source_index as u64,
                },
                RawSourceFieldIdV1::new("source_name.state").expect("static field is valid"),
                facts,
            )
            .expect("retained clip rows resolve their raw-source witness");
            let basis = EnginePredictionBasisV1::new(vec![
                PredictionBasisReferenceV1::profile_fact("animation_addressability")
                    .expect("static fact id is valid"),
                PredictionBasisReferenceV1::primary_source(BEVY_ANIMATION_LABEL_SOURCE)
                    .expect("static primary-source id is valid"),
                PredictionBasisReferenceV1::raw_source(source_name),
            ])
            .expect("three static basis references are valid");
            facets.push(
                EnginePredictionFacetV2::available(scope.clone(), basis)
                    .expect("complete rows form V2 facets"),
            );
            scopes.push(scope);
        }
    }

    if allocation.summary_required() {
        let basis = EnginePredictionBasisV1::new(vec![
            PredictionBasisReferenceV1::profile_fact("animation_addressability")
                .expect("static fact"),
            PredictionBasisReferenceV1::primary_source(BEVY_ANIMATION_LABEL_SOURCE)
                .expect("static source"),
        ])
        .expect("static basis");
        facets.push(
            EnginePredictionFacetV2::required_unavailable(
                EvaluationScope::new(EvaluationScopeCode::custom(
                    "engine-addressability:facet-budget",
                )),
                basis,
                vec![animsmith_core::PredictionUnavailableReasonV2::FacetBudgetExceeded],
            )
            .expect("budget summary facet"),
        );
    }
    if facets.is_empty() {
        return CheckOutput::from_coverage(Vec::new(), Vec::new(), Vec::new());
    }
    CheckOutput::from_coverage(Vec::new(), scopes, Vec::new()).with_engine_prediction_v2(
        EnginePredictionV2::new(provenance.identity().clone(), facets)
            .expect("allocated V2 facets"),
    )
}
