//! Exact Unity Generic 6000.3 FBX root-motion routing prediction.

use crate::error::PredictionRuleError;
use crate::profiles_v2;
use animsmith_core::engine_contract::{
    EngineBakeOrExtractV1, EngineSettingIdV2 as SettingIdV2, EngineSettingValueOriginV3,
    EngineSettingValueV2,
};
use animsmith_core::measure::{
    ClipMeasurements, MeasurementAvailability, RootTrajectorySourceRole,
};
use animsmith_core::profile::Role;
use animsmith_core::{
    Applicability, Check, CheckCtx, CheckOutput, EngineMachineResultV1, EnginePredictionBasisV4,
    EnginePredictionFacetV4, EnginePredictionV4, EnginePredictionV6,
    EngineRootMotionClipMappingStateV1, EngineRootMotionProjectIntentCountV1,
    EngineRootMotionProjectIntentCoverageV1, EvaluationScope, EvaluationScopeCode, Finding,
    LoadedSource, MeasurementPointerV1, PREDICTION_V2_MAX_CANDIDATE_FACETS_PER_RULE,
    PredictionBasisReferenceV1, PredictionBasisReferenceV2, PredictionBasisReferenceV4,
    PredictionFacetDemandV2, PredictionProvenanceV6, PredictionRuleAllocationV2,
    PredictionScalarV1, PredictionUnavailableReasonV2, RawAnimationChannelInventoryV1,
    RawSourceBasisReferenceV1, RawSourceBindingV2, RawSourceDomainV1, RawSourceFieldIdV1,
    RawSourceKeyV1, RawSourceSetCoverageStateV1, RawTransformPathCoverageReasonV1,
    RawTransformPathCoverageV1, RawTransformPathResolutionV1, RawTransformPathV1,
    ResolvedEngineSettingsCoverageStateV2, ResolvedRoles, ResolvedSettingLocationV1,
    RootMotionAxisV1, RootMotionCompatibilityV1, RootMotionImporterDispositionV1,
    RootMotionProjectOwnerV1, RootMotionRoutingResultV1, Severity, SourceFormatV1,
    SourceObservationStateV1,
};
use std::collections::BTreeMap;

/// Stable id for exact Unity Generic root-motion routing prediction.
pub const ENGINE_ROOT_MOTION_CHECK_ID: &str = "engine-root-motion";

const UNITY_FAMILY: &str = "unity-generic";
const UNITY_PROFILE_REVISION: u32 = 2;
const UNITY_ENGINE_VERSION: &str = "6000.3";
const UNITY_IMPORTER: &str = "fbx-model-importer";

const MODEL_IMPORT_SOURCE: &str = "unity-fbx-model-importer-6000.3";
const CLIP_IMPORT_SOURCE: &str = "unity-fbx-animation-clip-6000.3";
const MOTION_NODE_SOURCE: &str = "unity-fbx-motion-node-6000.3";

const CLIP_AXIS_SCOPE: &str = "engine-root-motion:clip-axis";
const INVENTORY_SCOPE: &str = "engine-root-motion:inventory";
const BUDGET_SCOPE: &str = "engine-root-motion:facet-budget";

const ROOT_SOURCE_MISMATCH_REASON: &str = "animsmith:root_motion_source_not_explicit_root";
const INTENT_WORK_BUDGET_REASON: &str = "animsmith:root_motion_intent_work_budget_exceeded";

/// Engine-owned check for the exact Unity Generic revision-2 FBX profile.
///
/// The indexed measurements are borrowed from the caller so the check consumes
/// the same measurement rows later published by the output boundary. They must
/// be in normalized [`animsmith_core::Document::clips`] order.
pub struct EngineRootMotionCheck<'a> {
    source: &'a LoadedSource,
    provenance: Option<&'a PredictionProvenanceV6>,
    measurements: &'a [ClipMeasurements],
}

impl<'a> EngineRootMotionCheck<'a> {
    /// Bind one optional V6 sidecar, explicit role resolution, and the exact
    /// indexed measurement rows produced for this load.
    ///
    /// # Errors
    ///
    /// Returns [`PredictionRuleError`] when the provenance does not reproduce
    /// the supplied source and raw-path inventory, the measurement cardinality
    /// differs from the normalized document, or the exact Unity tuple carries
    /// a profile other than the frozen registry record.
    pub fn new(
        source: &'a LoadedSource,
        provenance: Option<&'a PredictionProvenanceV6>,
        roles: &'a ResolvedRoles,
        measurements: &'a [ClipMeasurements],
    ) -> Result<Self, PredictionRuleError> {
        if measurements.len() != source.document().clips.len() {
            return Err(PredictionRuleError::SourceProvenanceMismatch);
        }
        if let Some(provenance) = provenance {
            provenance
                .validate()
                .map_err(|_| PredictionRuleError::SourceProvenanceMismatch)?;
            let raw_source = RawSourceBindingV2::from_source(
                source.source_facts(),
                source.exact_source_timing(),
            )
            .map_err(|_| PredictionRuleError::SourceProvenanceMismatch)?;
            let base = provenance.base();
            let resolved_root_bone_index = roles
                .get(Role::Root)
                .and_then(|index| u64::try_from(index).ok());
            if base.base().raw_source() != &raw_source
                || base.raw_animation_channels()
                    != &RawAnimationChannelInventoryV1::from_source(source.source_facts())
                || base.base().dependency_closure() != source.dependency_closure()
                || !raw_transform_paths_match(source, provenance)
                || !intent_matches_source(source, provenance)
                || provenance
                    .root_motion_project_intent()
                    .resolved_root_bone_index()
                    != resolved_root_bone_index
            {
                return Err(PredictionRuleError::SourceProvenanceMismatch);
            }
            if is_unity_tuple(provenance) && !has_frozen_unity_profile(provenance) {
                return Err(PredictionRuleError::FrozenProfileMismatch);
            }
        }
        Ok(Self {
            source,
            provenance,
            measurements,
        })
    }

    fn exact_applicable(&self) -> bool {
        self.provenance.is_some_and(|provenance| {
            is_exact_unity_fbx(self.source, provenance) && has_declared_work(provenance)
        })
    }
}

impl Check for EngineRootMotionCheck<'_> {
    fn id(&self) -> &'static str {
        ENGINE_ROOT_MOTION_CHECK_ID
    }

    fn allows_severity_off(&self) -> bool {
        false
    }

    fn applicability(&self, _ctx: &CheckCtx<'_>) -> Applicability {
        if self.exact_applicable() {
            Applicability::Applicable
        } else {
            Applicability::NotApplicable
        }
    }

    fn evaluate(&self, _ctx: &CheckCtx<'_>) -> CheckOutput {
        let Some(provenance) = self.provenance.filter(|_| self.exact_applicable()) else {
            return empty_output();
        };
        let demand = plan_demand(provenance);
        let (candidate_capacity, summary_required) = match demand {
            PredictionFacetDemandV2::Exact(count) => (count, false),
            PredictionFacetDemandV2::NPlusOne => (
                PREDICTION_V2_MAX_CANDIDATE_FACETS_PER_RULE.saturating_sub(1),
                true,
            ),
        };
        evaluate_allocated(
            self.source,
            provenance,
            self.measurements,
            candidate_capacity,
            summary_required,
        )
    }

    fn prediction_facet_demand_v2(&self, _ctx: &CheckCtx<'_>) -> PredictionFacetDemandV2 {
        self.provenance
            .filter(|_| self.exact_applicable())
            .map_or(PredictionFacetDemandV2::Exact(0), plan_demand)
    }

    fn evaluate_with_prediction_allocation_v2(
        &self,
        _ctx: &CheckCtx<'_>,
        allocation: PredictionRuleAllocationV2<'_>,
    ) -> CheckOutput {
        let Some(provenance) = self.provenance.filter(|_| self.exact_applicable()) else {
            return empty_output();
        };
        evaluate_allocated(
            self.source,
            provenance,
            self.measurements,
            allocation.candidate_capacity(),
            allocation.summary_required(),
        )
    }
}

fn has_declared_work(provenance: &PredictionProvenanceV6) -> bool {
    // Incomplete intent or settings evidence is applicable even when the
    // retained prefix has no owner. Otherwise an unvisited declaration tail
    // could be mistaken for a complete-empty project and incorrectly be N/A.
    let intent = provenance.root_motion_project_intent();
    if intent.clip_coverage() != EngineRootMotionProjectIntentCoverageV1::Complete
        || provenance.base().base().settings().clip_coverage().state()
            != ResolvedEngineSettingsCoverageStateV2::Complete
    {
        return true;
    }
    match intent.declared_axis_candidates() {
        EngineRootMotionProjectIntentCountV1::Exact { count } => count != 0,
        EngineRootMotionProjectIntentCountV1::NPlusOne => true,
    }
}

fn plan_demand(provenance: &PredictionProvenanceV6) -> PredictionFacetDemandV2 {
    if atomic_unavailable_reasons(provenance).is_some() {
        return PredictionFacetDemandV2::Exact(1);
    }
    match provenance
        .root_motion_project_intent()
        .declared_axis_candidates()
    {
        EngineRootMotionProjectIntentCountV1::Exact { count } => usize::try_from(count)
            .ok()
            .filter(|count| *count <= PREDICTION_V2_MAX_CANDIDATE_FACETS_PER_RULE)
            .map_or(
                PredictionFacetDemandV2::Exact(1),
                PredictionFacetDemandV2::Exact,
            ),
        EngineRootMotionProjectIntentCountV1::NPlusOne => PredictionFacetDemandV2::Exact(1),
    }
}

fn atomic_unavailable_reasons(
    provenance: &PredictionProvenanceV6,
) -> Option<Vec<PredictionUnavailableReasonV2>> {
    let mut reasons = Vec::new();
    if provenance
        .base()
        .base()
        .raw_source()
        .clips_coverage()
        .state()
        != RawSourceSetCoverageStateV1::Complete
        || provenance.raw_transform_paths().coverage() != RawTransformPathCoverageV1::Complete
    {
        reasons.push(PredictionUnavailableReasonV2::RawSourceIncomplete);
    }
    let intent = provenance.root_motion_project_intent();
    if intent.clip_coverage() != EngineRootMotionProjectIntentCoverageV1::Complete
        || has_unmapped_declared_work(intent)
    {
        reasons.push(PredictionUnavailableReasonV2::ProjectIntentUnavailable);
    }
    if intent.declared_axis_candidates().overflowed()
        || intent.unmapped_declared_axis_candidates().overflowed()
    {
        reasons.push(custom_reason(INTENT_WORK_BUDGET_REASON));
    }
    if provenance.base().base().settings().clip_coverage().state()
        != ResolvedEngineSettingsCoverageStateV2::Complete
    {
        reasons.push(PredictionUnavailableReasonV2::ResolvedSettingsOverflow);
    }
    (!reasons.is_empty()).then_some(reasons)
}

fn evaluate_allocated(
    source: &LoadedSource,
    provenance: &PredictionProvenanceV6,
    measurements: &[ClipMeasurements],
    candidate_capacity: usize,
    summary_required: bool,
) -> CheckOutput {
    let mut findings = Vec::new();
    let mut evaluated_scopes = Vec::new();
    let mut facets =
        Vec::with_capacity(candidate_capacity.saturating_add(usize::from(summary_required)));

    if let Some(reasons) = atomic_unavailable_reasons(provenance) {
        if candidate_capacity != 0 {
            facets.push(unavailable(
                inventory_scope(),
                inventory_basis(provenance),
                reasons,
            ));
        }
    } else {
        let duplicate_names = duplicate_clip_names(provenance);
        let path = configured_path(provenance);
        let path_resolution = path
            .as_ref()
            .map(|path| provenance.raw_transform_paths().resolve(path));
        let explicit_root = provenance
            .root_motion_project_intent()
            .resolved_root_bone_index();
        let explicit_root_name = explicit_root
            .and_then(|index| usize::try_from(index).ok())
            .and_then(|index| source.document().skeleton.bones.get(index))
            .map(|bone| bone.name.as_str());

        'clips: for clip in provenance.root_motion_project_intent().clips() {
            for (axis, owner) in declared_axes(clip) {
                if facets.len() == candidate_capacity {
                    break 'clips;
                }
                let (Some(normalized_clip_index), Some(clip_name)) = (
                    clip.normalized_clip_index()
                        .and_then(|index| usize::try_from(index).ok()),
                    clip.normalized_clip_name(),
                ) else {
                    continue;
                };
                let scope = axis_scope(clip.source_clip_index(), axis);
                let setting_id = axis_setting(axis);
                let clip_setting = provenance
                    .base()
                    .base()
                    .settings()
                    .clip_row(normalized_clip_index as u64, clip_name)
                    .and_then(|settings| settings.setting(setting_id));
                let measurement = measurements.get(normalized_clip_index);
                let duplicate_name_count = duplicate_names.get(clip_name).copied().unwrap_or(0);
                let basis = facet_basis(
                    source,
                    provenance,
                    clip.source_clip_index(),
                    clip_name,
                    Some(normalized_clip_index),
                    clip.normalized_clip_mapping_state(),
                    duplicate_name_count,
                    axis,
                    owner,
                    path.as_ref(),
                    path_resolution.as_ref(),
                    explicit_root,
                    clip_setting.map(|setting| setting.value_origin()),
                    measurement,
                );

                let reason = facet_unavailable_reason(
                    duplicate_name_count,
                    path_resolution.as_ref(),
                    explicit_root,
                    explicit_root_name,
                    measurement,
                    clip_setting.map(|setting| setting.value()),
                    axis,
                );
                if let Some(reason) = reason {
                    facets.push(unavailable(scope, basis, vec![reason]));
                    continue;
                }

                let disposition = importer_disposition(
                    clip_setting
                        .expect("available facet has the axis setting")
                        .value(),
                )
                .expect("available facet has bake/extract setting");
                let compatibility = compatibility(owner, disposition);
                facets.push(
                    EnginePredictionFacetV4::available(
                        scope.clone(),
                        basis,
                        EngineMachineResultV1::RootMotionRouting(RootMotionRoutingResultV1 {
                            axis,
                            project_owner: owner,
                            importer_disposition: disposition,
                            compatibility,
                        }),
                    )
                    .expect("complete Unity routing evidence forms an available facet"),
                );
                evaluated_scopes.push(scope.clone());
                if compatibility == RootMotionCompatibilityV1::Conflict {
                    findings.push(
                        Finding::new(
                            ENGINE_ROOT_MOTION_CHECK_ID,
                            Severity::Error,
                            format!(
                                "clip {:?} assigns {} movement to {}, but Unity imports that axis as {}",
                                clip_name,
                                axis_display_name(axis),
                                owner_name(owner),
                                disposition_name(disposition),
                            ),
                        )
                        .clip(clip_name)
                        .prediction_scope(scope),
                    );
                }
            }
        }
    }

    if summary_required {
        facets.push(unavailable(
            budget_scope(),
            static_basis(),
            vec![PredictionUnavailableReasonV2::FacetBudgetExceeded],
        ));
    }
    let prediction = EnginePredictionV4::new(provenance.base().base().identity().clone(), facets)
        .expect("allocated root-motion facets satisfy V4 bounds");
    CheckOutput::from_coverage(findings, evaluated_scopes, Vec::new()).with_engine_prediction_v6(
        EnginePredictionV6::new(provenance, prediction)
            .expect("root-motion V6 prediction binds its immutable provenance"),
    )
}

fn configured_path(provenance: &PredictionProvenanceV6) -> Option<RawTransformPathV1> {
    match provenance
        .base()
        .base()
        .settings()
        .document_setting(SettingIdV2::RootMotionSource)
        .map(|setting| setting.value())
    {
        Some(EngineSettingValueV2::SourceTransformPath(path)) => {
            RawTransformPathV1::parse(path).ok()
        }
        _ => None,
    }
}

fn facet_unavailable_reason(
    duplicate_name_count: usize,
    path_resolution: Option<&RawTransformPathResolutionV1>,
    explicit_root: Option<u64>,
    explicit_root_name: Option<&str>,
    measurement: Option<&ClipMeasurements>,
    setting: Option<&EngineSettingValueV2>,
    axis: RootMotionAxisV1,
) -> Option<PredictionUnavailableReasonV2> {
    if duplicate_name_count > 1 {
        return Some(PredictionUnavailableReasonV2::MeasurementUnavailable);
    }
    match path_resolution {
        Some(RawTransformPathResolutionV1::NoMatch) | None => {
            return Some(PredictionUnavailableReasonV2::SourceSelectorNoMatch);
        }
        Some(RawTransformPathResolutionV1::Ambiguous { .. }) => {
            return Some(PredictionUnavailableReasonV2::SourceSelectorAmbiguous);
        }
        Some(RawTransformPathResolutionV1::CoverageIncomplete { .. }) => {
            return Some(PredictionUnavailableReasonV2::RawSourceIncomplete);
        }
        Some(RawTransformPathResolutionV1::Exact(path_match))
            if explicit_root.is_none() || path_match.projected_bone_index() != explicit_root =>
        {
            return Some(custom_reason(ROOT_SOURCE_MISMATCH_REASON));
        }
        Some(RawTransformPathResolutionV1::Exact(_)) => {}
    }
    let trajectory = measurement
        .filter(|measurement| {
            measurement.root_trajectory_availability == MeasurementAvailability::Measured
        })
        .and_then(|measurement| measurement.root_trajectory.as_ref());
    let Some(trajectory) = trajectory else {
        return Some(PredictionUnavailableReasonV2::MeasurementUnavailable);
    };
    if trajectory.source_role != RootTrajectorySourceRole::Root
        || Some(u64::from(trajectory.bone_index)) != explicit_root
        || Some(trajectory.bone_name.as_str()) != explicit_root_name
    {
        return Some(custom_reason(ROOT_SOURCE_MISMATCH_REASON));
    }
    let measured = match axis {
        RootMotionAxisV1::HorizontalXz | RootMotionAxisV1::VerticalY => {
            trajectory.translation_availability == MeasurementAvailability::Measured
                && trajectory.translation.is_some()
        }
        RootMotionAxisV1::Yaw => {
            trajectory.yaw_availability == MeasurementAvailability::Measured
                && trajectory.yaw.is_some()
        }
    };
    if !measured {
        return Some(PredictionUnavailableReasonV2::MeasurementUnavailable);
    }
    let Some(setting) = setting else {
        return Some(PredictionUnavailableReasonV2::ResolvedSettingsOverflow);
    };
    if importer_disposition(setting).is_none() {
        return Some(PredictionUnavailableReasonV2::ResolvedSettingsOverflow);
    }
    None
}

fn declared_axes(
    clip: &animsmith_core::EngineRootMotionClipIntentV1,
) -> impl Iterator<Item = (RootMotionAxisV1, RootMotionProjectOwnerV1)> {
    [
        (RootMotionAxisV1::HorizontalXz, clip.movement_owner_xz()),
        (RootMotionAxisV1::VerticalY, clip.movement_owner_y()),
        (RootMotionAxisV1::Yaw, clip.movement_owner_yaw()),
    ]
    .into_iter()
    .filter_map(|(axis, owner)| owner.map(|owner| (axis, owner)))
}

fn duplicate_clip_names(provenance: &PredictionProvenanceV6) -> BTreeMap<&str, usize> {
    let mut counts = BTreeMap::new();
    for clip in provenance.root_motion_project_intent().clips() {
        if let Some(name) = clip.normalized_clip_name() {
            *counts.entry(name).or_insert(0) += 1;
        }
    }
    counts
}

fn has_unmapped_declared_work(intent: &animsmith_core::EngineRootMotionProjectIntentV1) -> bool {
    match intent.unmapped_declared_axis_candidates() {
        EngineRootMotionProjectIntentCountV1::Exact { count } => count != 0,
        EngineRootMotionProjectIntentCountV1::NPlusOne => true,
    }
}

fn importer_disposition(setting: &EngineSettingValueV2) -> Option<RootMotionImporterDispositionV1> {
    match setting {
        EngineSettingValueV2::BakeOrExtract(EngineBakeOrExtractV1::Bake) => {
            Some(RootMotionImporterDispositionV1::BakedIntoPose)
        }
        EngineSettingValueV2::BakeOrExtract(EngineBakeOrExtractV1::Extract) => {
            Some(RootMotionImporterDispositionV1::StoredAsRootMotion)
        }
        _ => None,
    }
}

fn compatibility(
    owner: RootMotionProjectOwnerV1,
    disposition: RootMotionImporterDispositionV1,
) -> RootMotionCompatibilityV1 {
    if matches!(
        (owner, disposition),
        (
            RootMotionProjectOwnerV1::Gameplay,
            RootMotionImporterDispositionV1::BakedIntoPose
        ) | (
            RootMotionProjectOwnerV1::Animation,
            RootMotionImporterDispositionV1::StoredAsRootMotion
        )
    ) {
        RootMotionCompatibilityV1::Compatible
    } else {
        RootMotionCompatibilityV1::Conflict
    }
}

#[allow(clippy::too_many_arguments)]
fn facet_basis(
    source: &LoadedSource,
    provenance: &PredictionProvenanceV6,
    source_clip_index: u64,
    clip_name: &str,
    normalized_clip_index: Option<usize>,
    mapping_state: EngineRootMotionClipMappingStateV1,
    duplicate_name_count: usize,
    axis: RootMotionAxisV1,
    owner: RootMotionProjectOwnerV1,
    configured_path: Option<&RawTransformPathV1>,
    path_resolution: Option<&RawTransformPathResolutionV1>,
    explicit_root: Option<u64>,
    setting_origin: Option<EngineSettingValueOriginV3>,
    measurement: Option<&ClipMeasurements>,
) -> EnginePredictionBasisV4 {
    let mut references = static_references();
    for setting in [
        SettingIdV2::AnimationType,
        SettingIdV2::AvatarSetup,
        SettingIdV2::ImportAnimation,
        SettingIdV2::RootMotionSource,
    ] {
        let origin = provenance
            .base()
            .base()
            .settings()
            .document_setting(setting)
            .map(|row| row.value_origin());
        references.push(project_reference(
            match setting {
                SettingIdV2::AnimationType => {
                    "resolved_setting.document.animation_type.value_origin"
                }
                SettingIdV2::AvatarSetup => "resolved_setting.document.avatar_setup.value_origin",
                SettingIdV2::ImportAnimation => {
                    "resolved_setting.document.import_animation.value_origin"
                }
                SettingIdV2::RootMotionSource => {
                    "resolved_setting.document.root_motion_source.value_origin"
                }
                _ => unreachable!("closed document root-motion setting inventory"),
            },
            origin.map_or(PredictionScalarV1::Null, |origin| {
                token(origin_name(origin))
            }),
        ));
    }
    if let Some(normalized_clip_index) = normalized_clip_index {
        references.push(setting_reference(
            ResolvedSettingLocationV1::Clip {
                clip_ordinal: normalized_clip_index as u64,
                clip_name: clip_name.to_owned(),
            },
            axis_setting(axis),
        ));
    }
    references.push(project_reference(
        "root_motion_project_intent.source_clip_index",
        PredictionScalarV1::UnsignedInteger {
            value: source_clip_index,
        },
    ));
    references.push(raw_clip_reference(
        source,
        source_clip_index,
        "normalized_clip_index.state",
    ));
    if mapping_state == EngineRootMotionClipMappingStateV1::Observed {
        references.push(raw_clip_reference(
            source,
            source_clip_index,
            "normalized_clip_index.value",
        ));
    }
    references.push(project_reference(
        "root_motion_project_intent.clip_mapping_state",
        token(mapping_state_name(mapping_state)),
    ));
    references.push(project_reference(
        "root_motion_project_intent.normalized_clip_index",
        normalized_clip_index.map_or(PredictionScalarV1::Null, |value| {
            PredictionScalarV1::UnsignedInteger {
                value: value as u64,
            }
        }),
    ));
    references.push(project_reference(
        "root_motion_project_intent.normalized_clip_name",
        PredictionScalarV1::text(clip_name).expect("validated intent name is bounded"),
    ));
    references.push(project_reference(
        "measurement.clip_name_match_count",
        PredictionScalarV1::UnsignedInteger {
            value: duplicate_name_count as u64,
        },
    ));
    references.push(project_reference(
        "measurement.clip_identity_state",
        token(if duplicate_name_count > 1 {
            "duplicate"
        } else {
            "unique"
        }),
    ));
    references.push(project_reference(
        "root_motion_project_intent.axis",
        token(axis_name(axis)),
    ));
    references.push(project_reference(
        "root_motion_project_intent.owner",
        token(owner_name(owner)),
    ));
    references.push(project_reference(
        "resolved_setting.clip.value_origin",
        setting_origin.map_or(PredictionScalarV1::Null, |origin| {
            token(origin_name(origin))
        }),
    ));
    references.push(project_reference(
        "raw_source.clips.coverage",
        token(
            match provenance
                .base()
                .base()
                .raw_source()
                .clips_coverage()
                .state()
            {
                RawSourceSetCoverageStateV1::Complete => "complete",
                RawSourceSetCoverageStateV1::Partial => "partial",
                RawSourceSetCoverageStateV1::Unavailable => "unavailable",
            },
        ),
    ));
    references.push(project_reference(
        "raw_transform_path_inventory.coverage",
        token(path_coverage_name(
            provenance.raw_transform_paths().coverage(),
        )),
    ));
    references.push(project_reference(
        "resolved_role.root.bone_index",
        explicit_root.map_or(PredictionScalarV1::Null, |value| {
            PredictionScalarV1::UnsignedInteger { value }
        }),
    ));
    references.push(project_reference(
        "root_motion_source.configured_path",
        configured_path.map_or(PredictionScalarV1::Null, |path| {
            PredictionScalarV1::text(path.as_str()).expect("validated path is bounded")
        }),
    ));
    references.push(project_reference(
        "root_motion_source.resolution_state",
        token(match path_resolution {
            Some(RawTransformPathResolutionV1::Exact(_)) => "exact",
            Some(RawTransformPathResolutionV1::NoMatch) => "no_match",
            Some(RawTransformPathResolutionV1::Ambiguous { .. }) => "ambiguous",
            Some(RawTransformPathResolutionV1::CoverageIncomplete { .. }) => "coverage_incomplete",
            None => "invalid_configured_path",
        }),
    ));
    if let Some(RawTransformPathResolutionV1::Ambiguous { matches }) = path_resolution {
        references.push(project_reference(
            "root_motion_source.match_count",
            PredictionScalarV1::UnsignedInteger {
                value: matches.len() as u64,
            },
        ));
    }
    if let Some(RawTransformPathResolutionV1::Exact(path_match)) = path_resolution {
        references.push(project_reference(
            "root_motion_source.source_node_index",
            PredictionScalarV1::UnsignedInteger {
                value: path_match.source_node_index(),
            },
        ));
        references.push(project_reference(
            "root_motion_source.projected_bone_index",
            path_match
                .projected_bone_index()
                .map_or(PredictionScalarV1::Null, |value| {
                    PredictionScalarV1::UnsignedInteger { value }
                }),
        ));
        references.push(project_reference(
            "root_motion_source.path",
            PredictionScalarV1::text(path_match.path().as_str())
                .expect("validated matched path is bounded"),
        ));
        references.push(project_reference(
            "root_motion_source.node_kind",
            token("source"),
        ));
        let parent_chain_bytes = serde_json::to_vec(path_match.parent_chain())
            .expect("raw transform parent chain serializes");
        references.push(project_reference(
            "root_motion_source.parent_chain_count",
            PredictionScalarV1::UnsignedInteger {
                value: path_match.parent_chain().len() as u64,
            },
        ));
        references.push(project_reference(
            "root_motion_source.parent_chain_sha256",
            PredictionScalarV1::text(animsmith_core::sha256_hex(&parent_chain_bytes))
                .expect("SHA-256 spelling is bounded"),
        ));
    }
    if duplicate_name_count == 1
        && let Some(measurement) = measurement
    {
        append_measurement_references(&mut references, clip_name, axis, measurement);
    }
    EnginePredictionBasisV4::new(references).expect("root-motion facet basis is bounded and valid")
}

fn append_measurement_references(
    references: &mut Vec<PredictionBasisReferenceV4>,
    clip_name: &str,
    axis: RootMotionAxisV1,
    measurement: &ClipMeasurements,
) {
    let escaped = escape_json_pointer_component(clip_name);
    let prefix = format!("/measurements/clips/{escaped}/root_trajectory");
    references.push(measurement_reference(
        format!("{prefix}_availability"),
        token(measurement_availability_name(
            measurement.root_trajectory_availability,
        )),
    ));
    let Some(trajectory) = measurement.root_trajectory.as_ref() else {
        return;
    };
    references.push(measurement_reference(
        format!("{prefix}/bone_index"),
        PredictionScalarV1::UnsignedInteger {
            value: u64::from(trajectory.bone_index),
        },
    ));
    references.push(measurement_reference(
        format!("{prefix}/source_role"),
        token(trajectory.source_role.as_str()),
    ));
    match axis {
        RootMotionAxisV1::HorizontalXz | RootMotionAxisV1::VerticalY => {
            references.push(measurement_reference(
                format!("{prefix}/translation_availability"),
                token(measurement_availability_name(
                    trajectory.translation_availability,
                )),
            ));
            references.push(project_reference(
                "measurement.root_translation_present",
                PredictionScalarV1::Boolean {
                    value: trajectory.translation.is_some(),
                },
            ));
        }
        RootMotionAxisV1::Yaw => {
            references.push(measurement_reference(
                format!("{prefix}/yaw_availability"),
                token(measurement_availability_name(trajectory.yaw_availability)),
            ));
            references.push(project_reference(
                "measurement.root_yaw_present",
                PredictionScalarV1::Boolean {
                    value: trajectory.yaw.is_some(),
                },
            ));
        }
    }
}

fn inventory_basis(provenance: &PredictionProvenanceV6) -> EnginePredictionBasisV4 {
    let mut references = static_references();
    references.push(project_reference(
        "raw_source.clips.coverage",
        token(
            match provenance
                .base()
                .base()
                .raw_source()
                .clips_coverage()
                .state()
            {
                RawSourceSetCoverageStateV1::Complete => "complete",
                RawSourceSetCoverageStateV1::Partial => "partial",
                RawSourceSetCoverageStateV1::Unavailable => "unavailable",
            },
        ),
    ));
    references.push(project_reference(
        "raw_transform_path_inventory.coverage",
        token(path_coverage_name(
            provenance.raw_transform_paths().coverage(),
        )),
    ));
    references.push(project_reference(
        "root_motion_project_intent.clip_coverage",
        token(
            match provenance.root_motion_project_intent().clip_coverage() {
                EngineRootMotionProjectIntentCoverageV1::Complete => "complete",
                EngineRootMotionProjectIntentCoverageV1::PartialProjectionBudgetExceeded => {
                    "partial_projection_budget_exceeded"
                }
            },
        ),
    ));
    references.push(project_reference(
        "root_motion_project_intent.declared_axis_candidates",
        count_scalar(
            provenance
                .root_motion_project_intent()
                .declared_axis_candidates(),
        ),
    ));
    references.push(project_reference(
        "root_motion_project_intent.unmapped_declared_axis_candidates",
        count_scalar(
            provenance
                .root_motion_project_intent()
                .unmapped_declared_axis_candidates(),
        ),
    ));
    references.push(project_reference(
        "resolved_settings.clips.coverage",
        token(
            match provenance.base().base().settings().clip_coverage().state() {
                ResolvedEngineSettingsCoverageStateV2::Complete => "complete",
                ResolvedEngineSettingsCoverageStateV2::Partial => "partial",
            },
        ),
    ));
    EnginePredictionBasisV4::new(references).expect("inventory summary basis is valid")
}

fn static_basis() -> EnginePredictionBasisV4 {
    EnginePredictionBasisV4::new(static_references()).expect("static root-motion basis is valid")
}

fn static_references() -> Vec<PredictionBasisReferenceV4> {
    let mut references = vec![
        profile_fact_reference(),
        primary_source_reference(MODEL_IMPORT_SOURCE),
        primary_source_reference(CLIP_IMPORT_SOURCE),
        primary_source_reference(MOTION_NODE_SOURCE),
    ];
    for setting in [
        SettingIdV2::AnimationType,
        SettingIdV2::AvatarSetup,
        SettingIdV2::ImportAnimation,
        SettingIdV2::RootMotionSource,
    ] {
        references.push(setting_reference(
            ResolvedSettingLocationV1::Document,
            setting,
        ));
    }
    references
}

fn profile_fact_reference() -> PredictionBasisReferenceV4 {
    lift(
        PredictionBasisReferenceV1::profile_fact("root_motion_addressability")
            .expect("static root-motion fact id"),
    )
}

fn primary_source_reference(source: &'static str) -> PredictionBasisReferenceV4 {
    lift(PredictionBasisReferenceV1::primary_source(source).expect("static Unity source id"))
}

fn setting_reference(
    location: ResolvedSettingLocationV1,
    setting: SettingIdV2,
) -> PredictionBasisReferenceV4 {
    lift(
        PredictionBasisReferenceV1::resolved_setting(location, setting.as_str())
            .expect("static Unity setting id"),
    )
}

fn project_reference(field: &'static str, value: PredictionScalarV1) -> PredictionBasisReferenceV4 {
    lift(PredictionBasisReferenceV1::project_field(field, value).expect("static project field id"))
}

fn measurement_reference(pointer: String, value: PredictionScalarV1) -> PredictionBasisReferenceV4 {
    lift(PredictionBasisReferenceV1::measurement_v16(
        MeasurementPointerV1::new(pointer).expect("measurement pointer is canonical"),
        value,
    ))
}

fn lift(reference: PredictionBasisReferenceV1) -> PredictionBasisReferenceV4 {
    PredictionBasisReferenceV4::v2(PredictionBasisReferenceV2::v1(reference))
}

fn raw_clip_reference(
    source: &LoadedSource,
    source_clip_index: u64,
    field: &'static str,
) -> PredictionBasisReferenceV4 {
    lift(PredictionBasisReferenceV1::raw_source(
        RawSourceBasisReferenceV1::from_source(
            RawSourceDomainV1::Clip,
            RawSourceKeyV1::Clip { source_clip_index },
            RawSourceFieldIdV1::new(field).expect("static raw clip field"),
            source.source_facts(),
        )
        .expect("same-load intent row has the cited raw clip field"),
    ))
}

fn token(value: &'static str) -> PredictionScalarV1 {
    PredictionScalarV1::token(value).expect("static token is valid")
}

fn count_scalar(count: EngineRootMotionProjectIntentCountV1) -> PredictionScalarV1 {
    match count {
        EngineRootMotionProjectIntentCountV1::Exact { count } => {
            PredictionScalarV1::UnsignedInteger { value: count }
        }
        EngineRootMotionProjectIntentCountV1::NPlusOne => token("n_plus_one"),
    }
}

fn unavailable(
    scope: EvaluationScope,
    basis: EnginePredictionBasisV4,
    reasons: Vec<PredictionUnavailableReasonV2>,
) -> EnginePredictionFacetV4 {
    EnginePredictionFacetV4::required_unavailable(scope, basis, reasons)
        .expect("typed root-motion unavailability is valid")
}

fn custom_reason(code: &'static str) -> PredictionUnavailableReasonV2 {
    PredictionUnavailableReasonV2::custom(code).expect("static unavailable reason is valid")
}

fn axis_setting(axis: RootMotionAxisV1) -> SettingIdV2 {
    match axis {
        RootMotionAxisV1::HorizontalXz => SettingIdV2::RootPositionXz,
        RootMotionAxisV1::VerticalY => SettingIdV2::RootPositionY,
        RootMotionAxisV1::Yaw => SettingIdV2::RootRotation,
    }
}

fn axis_scope(source_clip_index: u64, axis: RootMotionAxisV1) -> EvaluationScope {
    EvaluationScope::new(EvaluationScopeCode::custom(CLIP_AXIS_SCOPE)).subject(format!(
        "source_clip:{source_clip_index:020}:axis:{}",
        axis_name(axis)
    ))
}

fn inventory_scope() -> EvaluationScope {
    EvaluationScope::new(EvaluationScopeCode::custom(INVENTORY_SCOPE))
}

fn budget_scope() -> EvaluationScope {
    EvaluationScope::new(EvaluationScopeCode::custom(BUDGET_SCOPE))
}

fn axis_name(axis: RootMotionAxisV1) -> &'static str {
    match axis {
        RootMotionAxisV1::HorizontalXz => "horizontal_xz",
        RootMotionAxisV1::VerticalY => "vertical_y",
        RootMotionAxisV1::Yaw => "yaw",
    }
}

fn axis_display_name(axis: RootMotionAxisV1) -> &'static str {
    match axis {
        RootMotionAxisV1::HorizontalXz => "horizontal XZ",
        RootMotionAxisV1::VerticalY => "vertical Y",
        RootMotionAxisV1::Yaw => "yaw",
    }
}

fn owner_name(owner: RootMotionProjectOwnerV1) -> &'static str {
    match owner {
        RootMotionProjectOwnerV1::Gameplay => "gameplay",
        RootMotionProjectOwnerV1::Animation => "animation",
    }
}

fn disposition_name(disposition: RootMotionImporterDispositionV1) -> &'static str {
    match disposition {
        RootMotionImporterDispositionV1::BakedIntoPose => "baked into the pose",
        RootMotionImporterDispositionV1::StoredAsRootMotion => "stored as root motion",
    }
}

fn origin_name(origin: EngineSettingValueOriginV3) -> &'static str {
    match origin {
        EngineSettingValueOriginV3::ExplicitConfig => "explicit_config",
        EngineSettingValueOriginV3::ProfileDefault => "profile_default",
    }
}

fn mapping_state_name(state: EngineRootMotionClipMappingStateV1) -> &'static str {
    match state {
        EngineRootMotionClipMappingStateV1::Observed => "observed",
        EngineRootMotionClipMappingStateV1::ProvenAbsent => "proven_absent",
        EngineRootMotionClipMappingStateV1::Unavailable => "unavailable",
    }
}

fn measurement_availability_name(availability: MeasurementAvailability) -> &'static str {
    match availability {
        MeasurementAvailability::Measured => "measured",
        MeasurementAvailability::NotApplicable => "not_applicable",
        MeasurementAvailability::Unavailable => "unavailable",
        _ => "unknown",
    }
}

fn path_coverage_name(coverage: RawTransformPathCoverageV1) -> &'static str {
    match coverage {
        RawTransformPathCoverageV1::Complete => "complete",
        RawTransformPathCoverageV1::Partial(_) => "partial",
        RawTransformPathCoverageV1::Unavailable(_) => "unavailable",
    }
}

fn escape_json_pointer_component(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}

fn intent_matches_source(source: &LoadedSource, provenance: &PredictionProvenanceV6) -> bool {
    let source_rows = source.source_facts().clips().rows();
    provenance
        .root_motion_project_intent()
        .clips()
        .iter()
        .all(|intent| {
            let Ok(source_index) = usize::try_from(intent.source_clip_index()) else {
                return false;
            };
            let Some(source_row) = source_rows.get(source_index) else {
                return false;
            };
            match source_row.normalized_clip_index().state() {
                SourceObservationStateV1::Observed(normalized_index) => {
                    intent.normalized_clip_mapping_state()
                        == EngineRootMotionClipMappingStateV1::Observed
                        && intent.normalized_clip_index() == u64::try_from(*normalized_index).ok()
                        && source
                            .document()
                            .clips
                            .get(*normalized_index)
                            .is_some_and(|clip| {
                                intent
                                    .normalized_clip_name()
                                    .is_some_and(|name| clip.name == name)
                            })
                }
                SourceObservationStateV1::ProvenAbsent
                    if intent.normalized_clip_mapping_state()
                        == EngineRootMotionClipMappingStateV1::ProvenAbsent =>
                {
                    intent.normalized_clip_index().is_none()
                        && intent.normalized_clip_name().is_none()
                }
                SourceObservationStateV1::Unavailable(_)
                    if intent.normalized_clip_mapping_state()
                        == EngineRootMotionClipMappingStateV1::Unavailable =>
                {
                    intent.normalized_clip_index().is_none()
                        && intent.normalized_clip_name().is_none()
                }
                SourceObservationStateV1::ProvenAbsent
                | SourceObservationStateV1::Unavailable(_) => false,
            }
        })
}

fn raw_transform_paths_match(source: &LoadedSource, provenance: &PredictionProvenanceV6) -> bool {
    match source.raw_transform_path_inventory() {
        Some(inventory) => inventory == provenance.raw_transform_paths(),
        None => matches!(
            provenance.raw_transform_paths().coverage(),
            RawTransformPathCoverageV1::Unavailable(
                RawTransformPathCoverageReasonV1::LoaderEvidenceUnavailable
            )
        ),
    }
}

fn is_unity_tuple(provenance: &PredictionProvenanceV6) -> bool {
    let selection = provenance.base().base().profile().selection();
    selection.family() == UNITY_FAMILY
        && selection.profile_revision() == UNITY_PROFILE_REVISION
        && selection.engine_version() == UNITY_ENGINE_VERSION
        && selection.importer() == UNITY_IMPORTER
}

fn has_frozen_unity_profile(provenance: &PredictionProvenanceV6) -> bool {
    profiles_v2().iter().any(|profile| {
        let selection = profile.selection();
        selection.family() == UNITY_FAMILY
            && selection.profile_revision() == UNITY_PROFILE_REVISION
            && selection.engine_version() == UNITY_ENGINE_VERSION
            && selection.importer() == UNITY_IMPORTER
            && crate::project_engine_profile_v2(profile)
                .is_ok_and(|projected| &projected == provenance.base().base().profile())
    })
}

fn is_exact_unity_fbx(source: &LoadedSource, provenance: &PredictionProvenanceV6) -> bool {
    source.source_facts().format() == SourceFormatV1::Fbx
        && provenance.base().base().source_format() == SourceFormatV1::Fbx
        && is_unity_tuple(provenance)
        && has_frozen_unity_profile(provenance)
}

fn empty_output() -> CheckOutput {
    CheckOutput::from_coverage(Vec::new(), Vec::new(), Vec::new())
}
