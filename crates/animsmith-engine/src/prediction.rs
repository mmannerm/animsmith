//! Engine-owned checks derived from immutable profile and same-load source evidence.

use animsmith_core::{
    Applicability, Check, CheckCtx, CheckOutput, EngineAnimationAddressabilityV1, EngineFactIdV1,
    EngineFactStateV1, EngineFactValueV1, EnginePredictionBasisV1, EnginePredictionFacetV1,
    EnginePredictionV1, EvaluationScope, EvaluationScopeCode, LoadedSource,
    PredictionBasisReferenceV1, PredictionProvenanceV1, PredictionUnavailableReasonV1,
    RawSourceBasisReferenceV1, RawSourceDomainV1, RawSourceFieldIdV1, RawSourceKeyV1,
    SourceFormatV1, SourceSetCoverageStateV1,
};

/// Stable check id for engine scene, animation, target, and runtime-label addressability.
pub const ENGINE_ADDRESSABILITY_CHECK_ID: &str = "engine-addressability";

/// Versioned engine-owned check ids callers may use for pre-I/O selection validation.
pub const ENGINE_CHECK_IDS_V1: &[&str] = &[ENGINE_ADDRESSABILITY_CHECK_ID];

const BEVY_FAMILY: &str = "bevy";
const BEVY_PROFILE_REVISION: u32 = 1;
const BEVY_ENGINE_VERSION: &str = "0.19.0";
const BEVY_IMPORTER: &str = "gltf-asset-loader";
const BEVY_ANIMATION_LABEL_SOURCE: &str = "bevy-gltf-asset-label-0.19.0";

/// A borrowed source-animation label check with optional resolved engine provenance.
///
/// When provenance is absent or is not exactly the frozen Bevy 0.19.0 glTF profile, the check
/// records a stable not-applicable evaluation. The check records no loader traversal at
/// construction; source rows are traversed only from [`Check::evaluate`].
pub struct AnimationAssetLabelCheck<'a> {
    source: &'a LoadedSource,
    provenance: Option<&'a PredictionProvenanceV1>,
}

impl<'a> AnimationAssetLabelCheck<'a> {
    /// Borrow one same-load source and optional immutable prediction provenance.
    pub const fn new(
        source: &'a LoadedSource,
        provenance: Option<&'a PredictionProvenanceV1>,
    ) -> Self {
        Self { source, provenance }
    }
}

impl Check for AnimationAssetLabelCheck<'_> {
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
    let selection = provenance.profile().selection();
    facts.primary_identity() == provenance.raw_source().primary_input()
        && facts.format() == provenance.source_format()
        && selection.family() == BEVY_FAMILY
        && selection.profile_revision() == BEVY_PROFILE_REVISION
        && selection.engine_version() == BEVY_ENGINE_VERSION
        && selection.importer() == BEVY_IMPORTER
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
        let scope = EvaluationScope::new(EvaluationScopeCode::ANIMATION_ASSET_LABEL)
            .subject(format!("Animation{source_index}"));
        let source_name = RawSourceBasisReferenceV1::from_source(
            RawSourceDomainV1::Clip,
            RawSourceKeyV1::Clip {
                source_clip_index: source_index as u64,
            },
            RawSourceFieldIdV1::new("source_name.state").expect("static field is valid"),
            facts,
        );
        let facet = match source_name {
            Ok(source_name) => {
                let basis = EnginePredictionBasisV1::new(vec![
                    PredictionBasisReferenceV1::profile_fact("animation_addressability")
                        .expect("static fact id is valid"),
                    PredictionBasisReferenceV1::primary_source(BEVY_ANIMATION_LABEL_SOURCE)
                        .expect("static primary-source id is valid"),
                    PredictionBasisReferenceV1::raw_source(source_name),
                ]);
                match basis
                    .and_then(|basis| EnginePredictionFacetV1::available(scope.clone(), basis))
                {
                    Ok(facet) => facet,
                    Err(_) => return unavailable_inventory(provenance, inventory_scope),
                }
            }
            _ => return unavailable_inventory(provenance, inventory_scope),
        };
        scopes.push(scope);
        facets.push(facet);
    }

    if facets.is_empty() {
        return CheckOutput::from_coverage(Vec::new(), Vec::new(), Vec::new());
    }
    let prediction = match EnginePredictionV1::new(provenance.identity().clone(), facets) {
        Ok(prediction) => prediction,
        Err(_) => return unavailable_inventory(provenance, inventory_scope),
    };
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
