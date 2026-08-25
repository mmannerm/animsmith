use crate::canonical::{project_resolved_profile, project_resolved_profile_v2};
use crate::{ResolvedProfile, ResolvedProfileV2};
use animsmith_core::LoadedSource;
use animsmith_core::engine_contract::EngineContractError;
use animsmith_core::prediction::{
    PredictionContractError, PredictionProvenanceV1, PredictionProvenanceV2,
    PredictionProvenanceV3, RawSourceBindingV1, RawSourceBindingV2,
};

/// Failure to project already-resolved engine and same-load source evidence.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PredictionProvenanceProjectionError {
    /// The frozen engine profile or its materialized settings contradicted the
    /// core-owned V1 engine contract.
    #[error("resolved engine evidence is invalid: {0}")]
    EngineContract(#[from] EngineContractError),
    /// The same-load engine, raw-source, or dependency-closure evidence could
    /// not form one valid prediction-provenance record.
    #[error("prediction provenance is invalid: {0}")]
    PredictionContract(#[from] PredictionContractError),
}

/// Project one resolved engine profile and its same-load source evidence into
/// the self-contained prediction-provenance V1 contract.
///
/// This is a one-way, allocation-only adapter. It does not read source bytes,
/// resolve a profile or selector again, consult the registry, apply defaults,
/// or recompute the dependency-closure identity. The caller must pass the
/// [`LoadedSource`] retained from the same load used to resolve `profile`.
///
/// # Errors
///
/// Returns [`PredictionProvenanceProjectionError`] if the already-resolved
/// engine evidence no longer satisfies its core contract or if the source
/// format and primary identity do not agree across the supplied same-load
/// evidence.
pub fn project_prediction_provenance_v1(
    profile: &ResolvedProfile,
    source: &LoadedSource,
) -> Result<PredictionProvenanceV1, PredictionProvenanceProjectionError> {
    let (profile_contract, settings_contract) = project_resolved_profile(profile)?;
    let raw_source = RawSourceBindingV1::from_source(source.source_facts());
    Ok(PredictionProvenanceV1::new(
        profile_contract,
        profile.source_format(),
        settings_contract,
        raw_source,
        source.dependency_closure().clone(),
    )?)
}

/// Project one bounded V2 engine resolution into V2 prediction provenance.
///
/// This adapter consumes only the canonical retained prefix plus its explicit
/// N+1 coverage/work evidence. It does not revisit the tail inventory.
pub fn project_prediction_provenance_v2(
    profile: &ResolvedProfileV2,
    source: &LoadedSource,
) -> Result<PredictionProvenanceV2, PredictionProvenanceProjectionError> {
    let (profile_contract, settings_contract) = project_resolved_profile_v2(profile)?;
    let raw_source = RawSourceBindingV1::from_source(source.source_facts());
    Ok(PredictionProvenanceV2::new(
        profile_contract,
        profile.source_format(),
        settings_contract,
        raw_source,
        source.dependency_closure().clone(),
    )?)
}

/// Project one bounded V2 engine resolution and same-load exact source
/// evidence into V3 prediction provenance.
///
/// The engine profile and settings remain the immutable V2 contracts. V3
/// widens only the raw-source binding so engine-owned rules can cite exact FBX
/// timing observations without copying or reconstructing them.
pub fn project_prediction_provenance_v3(
    profile: &ResolvedProfileV2,
    source: &LoadedSource,
) -> Result<PredictionProvenanceV3, PredictionProvenanceProjectionError> {
    let (profile_contract, settings_contract) = project_resolved_profile_v2(profile)?;
    let raw_source =
        RawSourceBindingV2::from_source(source.source_facts(), source.exact_fbx_timing())?;
    Ok(PredictionProvenanceV3::new(
        profile_contract,
        profile.source_format(),
        settings_contract,
        raw_source,
        source.dependency_closure().clone(),
    )?)
}
