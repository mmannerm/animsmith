//! Small evidence fixtures for presentation tests; no check evaluation or grid warming.
use animsmith_core::{CheckEvaluation, Document, LintFileReportV19, LoadedSource, ResolvedRoles};

pub(crate) fn source(document: Document) -> LoadedSource {
    animsmith_core::RawSourceFactsBuilderV1::new(
        animsmith_core::SourceFormatV1::GltfJson,
        animsmith_core::InputIdentity::from_bytes(b"explicit synthetic report fixture"),
    )
    .finish(document)
    .unwrap()
}

pub(crate) fn evidence(
    source: &LoadedSource,
    roles: &ResolvedRoles,
    checks: &[CheckEvaluation],
) -> LintFileReportV19 {
    LintFileReportV19::new(
        "fixture",
        source.source_facts().primary_identity().clone(),
        animsmith_core::RigInfo::from_resolved(source.document(), roles).unwrap(),
        None,
        checks.to_vec(),
        animsmith_core::MeasurementContract::new(
            Default::default(),
            animsmith_core::measure::measure_assets(source.document()),
        )
        .unwrap(),
    )
    .unwrap()
}
