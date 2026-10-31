//! Current evidence stays immutable; this module projects display fields only.

use animsmith_core::{
    CheckEvaluation, DependencyClosureV1, EngineMachineResultV1, EnginePredictionFacetStateV1,
    EngineProfileSelectionV1, EvaluationScope, LintFileReportV19, LoadedSource, MetricGrids,
    PredictionProvenanceV3, PredictionProvenanceV5, PredictionProvenanceV6,
    PredictionUnavailableReasonV2,
};
use serde::Serialize;

use crate::ReportError;

#[derive(Clone, Copy, Serialize)]
#[serde(untagged)]
enum Provenance<'a> {
    V3(&'a PredictionProvenanceV3),
    V5(&'a PredictionProvenanceV5),
    V6(&'a PredictionProvenanceV6),
}

impl<'a> Provenance<'a> {
    fn of(evidence: &'a LintFileReportV19) -> Option<Self> {
        evidence
            .prediction_provenance_v3()
            .map(Self::V3)
            .or_else(|| evidence.prediction_provenance_v5().map(Self::V5))
            .or_else(|| evidence.prediction_provenance_v6().map(Self::V6))
    }

    fn closure(self) -> &'a DependencyClosureV1 {
        match self {
            Self::V3(p) => p.dependency_closure(),
            Self::V5(p) => p.base().dependency_closure(),
            Self::V6(p) => p.base().base().dependency_closure(),
        }
    }

    fn selection(self) -> &'a EngineProfileSelectionV1 {
        match self {
            Self::V3(p) => p.profile().selection(),
            Self::V5(p) => p.base().profile().selection(),
            Self::V6(p) => p.base().base().profile().selection(),
        }
    }
}

pub(super) fn validate(
    source: &LoadedSource,
    grids: &MetricGrids<'_>,
    evidence: &LintFileReportV19,
    side: &'static str,
) -> Result<(), ReportError> {
    if !std::ptr::eq(source.document(), grids.document()) {
        return Err(ReportError::AuthorityDocumentMismatch { side });
    }
    if evidence.input() != source.source_facts().primary_identity() {
        return Err(ReportError::PredictionAuthorityMismatch {
            side,
            detail: "evidence primary identity differs from the loaded source",
        });
    }
    if Provenance::of(evidence).is_some_and(|p| p.closure() != source.dependency_closure()) {
        return Err(ReportError::PredictionAuthorityMismatch {
            side,
            detail: "provenance dependency closure differs from the loaded source",
        });
    }
    // V19 already validates nested prediction/provenance/measurement links.
    // Profile-free evidence has no closure/config binding; the CLI's shared
    // analysis establishes that association, not this public type alone.
    Ok(())
}

#[derive(Serialize)]
struct DisplayFacet<'a> {
    check_id: &'static str,
    scope: &'a EvaluationScope,
    state: EnginePredictionFacetStateV1,
    reasons: &'a [PredictionUnavailableReasonV2],
    result: Option<&'a EngineMachineResultV1>,
}

fn facets(evidence: &LintFileReportV19) -> impl Iterator<Item = DisplayFacet<'_>> {
    evidence.checks().iter().flat_map(|check| {
        let v3 = check.engine_prediction_v3().map_or(&[][..], |p| p.facets());
        let v4 = check
            .engine_prediction_v5()
            .map(|p| p.facets())
            .or_else(|| check.engine_prediction_v6().map(|p| p.facets()))
            .unwrap_or(&[]);
        v3.iter()
            .map(move |facet| DisplayFacet {
                check_id: check.check_id(),
                scope: facet.scope(),
                state: facet.state(),
                reasons: facet.reasons(),
                result: None,
            })
            .chain(v4.iter().map(move |facet| DisplayFacet {
                check_id: check.check_id(),
                scope: facet.scope(),
                state: facet.state(),
                reasons: facet.reasons(),
                result: facet.result(),
            }))
    })
}

#[derive(Serialize)]
struct DisplayProjection<'a> {
    kind: &'static str,
    scope: &'static str,
    profile: Option<&'a EngineProfileSelectionV1>,
    facets: Vec<DisplayFacet<'a>>,
}

/// Originals and a separately named presentation projection. Serialization of
/// each check preserves its exact attachment schema, wrapper and lifecycle.
#[derive(Serialize)]
pub(super) struct EngineEvidence<'a> {
    provenance: Option<Provenance<'a>>,
    checks: Vec<&'a CheckEvaluation>,
    projection: DisplayProjection<'a>,
}

impl<'a> EngineEvidence<'a> {
    pub(super) fn new(evidence: &'a LintFileReportV19) -> Self {
        // V19 owns the 4096-facet aggregate bound and all nested lifecycle links.
        let facets = facets(evidence);
        let provenance = Provenance::of(evidence);
        Self {
            provenance,
            // Preserve inactive built-in engine lifecycle rows as well as
            // attached custom predictions; this is presentation, not a catalog.
            checks: evidence
                .checks()
                .iter()
                .filter(|check| {
                    check.check_id().starts_with("engine-")
                        || check.engine_prediction_v3().is_some()
                        || check.engine_prediction_v5().is_some()
                        || check.engine_prediction_v6().is_some()
                })
                .collect(),
            projection: DisplayProjection {
                kind: "animsmith-engine-display-v1",
                scope: "whole_input",
                profile: provenance.map(Provenance::selection),
                facets: facets.collect(),
            },
        }
    }
}
