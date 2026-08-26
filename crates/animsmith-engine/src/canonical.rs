use crate::{
    AnimationAddressability, BakeOrExtract, ConversionControl, DefaultStatus, EngineProfile,
    EngineProfileV2, FactState, FactValue, ForwardAxis, Handedness, ImportHandling, LinearUnit,
    ResolvedClipCoverageV2, ResolvedProfile, ResolvedProfileSettingsV2, ResolvedProfileV2,
    ResolvedSettingOriginV2, RootMotionAddressability, SettingApplicability, SettingDefaultV2,
    SettingDomainV2, SettingId, SettingIdV2, SettingScope, SettingValue, SettingValueV2,
    TargetAddressability, UpAxis,
};
use animsmith_core::engine_contract::{
    EngineAnimationAddressabilityV1, EngineBakeOrExtractV1, EngineClipSettingsV1,
    EngineContractError, EngineConversionControlV1, EngineCoordinateBasisV1, EngineDefaultStatusV1,
    EngineFactIdV1, EngineFactStateV1, EngineFactValueV1, EngineForwardAxisV1, EngineHandednessV1,
    EngineImportHandlingV1, EngineLinearUnitV1, EnginePrimarySourceV1, EngineProfileFactV1,
    EngineProfileSelectionV1, EngineRootMotionAddressabilityV1, EngineSettingApplicabilityV1,
    EngineSettingDescriptorV1, EngineSettingDomainV1, EngineSettingIdV1, EngineSettingRowV1,
    EngineSettingScopeV1, EngineSettingValueV1, EngineTargetAddressabilityV1, EngineUpAxisV1,
    ResolvedEngineProfileV1, ResolvedEngineSettingsCoverageV2, ResolvedEngineSettingsV1,
    ResolvedEngineSettingsV2, ResolvedEngineSettingsWorkV2,
};
use animsmith_core::engine_contract::{
    EnginePrimarySourceV2 as CorePrimarySourceV2,
    EngineSettingDescriptorV2 as CoreSettingDescriptorV2,
    EngineSettingDomainV2 as CoreSettingDomainV2, EngineSettingIdV2 as CoreSettingIdV2,
    EngineSettingRowV3, EngineSettingValueOriginV3, EngineSettingValueV2 as CoreSettingValueV2,
    ResolvedEngineProfileV2 as CoreEngineProfileV2,
    ResolvedEngineSettingsCoverageV2 as CoreSettingsCoverageV2,
    ResolvedEngineSettingsV3 as CoreSettingsV3, ResolvedEngineSettingsWorkV2 as CoreSettingsWorkV2,
};
use animsmith_core::{InputIdentity, SourceFormatV1};
use std::collections::BTreeMap;

pub(crate) fn facts_identity(profile: &EngineProfile) -> InputIdentity {
    project_profile(profile)
        .expect("the frozen engine registry must project into its core-owned V1 contract")
        .facts_identity()
        .clone()
}

pub(crate) fn settings_identity<'a>(
    profile: &EngineProfile,
    document: &BTreeMap<SettingId, SettingValue>,
    clips: impl IntoIterator<Item = (&'a str, &'a BTreeMap<SettingId, SettingValue>)>,
) -> Result<InputIdentity, EngineContractError> {
    let profile = project_profile(profile)?;
    Ok(project_settings(&profile, document, clips)?
        .settings_identity()
        .clone())
}

pub(crate) fn project_resolved_profile(
    resolved: &ResolvedProfile,
) -> Result<(ResolvedEngineProfileV1, ResolvedEngineSettingsV1), EngineContractError> {
    let profile = project_profile(resolved.profile())?;
    let settings = project_settings(
        &profile,
        resolved.document_settings(),
        resolved
            .clip_settings()
            .iter()
            .map(|clip| (clip.clip_name(), clip.settings())),
    )?;
    Ok((profile, settings))
}

/// Project one bounded V2 resolution into core-owned V2 settings.
pub(crate) fn project_resolved_profile_v2(
    resolved: &ResolvedProfileV2,
) -> Result<(ResolvedEngineProfileV1, ResolvedEngineSettingsV2), EngineContractError> {
    let profile = project_profile(resolved.profile())?;
    let coverage = match resolved.clip_coverage() {
        ResolvedClipCoverageV2::Complete => ResolvedEngineSettingsCoverageV2::complete(),
        ResolvedClipCoverageV2::Partial { .. } => {
            ResolvedEngineSettingsCoverageV2::actual_clip_rows_exceeded()
        }
    };
    let work = resolved.work();
    let settings = ResolvedEngineSettingsV2::new(
        &profile,
        resolved
            .document_settings()
            .iter()
            .map(|(id, value)| EngineSettingRowV1::new(setting_id(*id), setting_value(value)))
            .collect(),
        resolved
            .clip_settings()
            .iter()
            .map(|clip| {
                EngineClipSettingsV1::new(
                    clip.clip_name(),
                    clip.settings()
                        .iter()
                        .map(|(id, value)| {
                            EngineSettingRowV1::new(setting_id(*id), setting_value(value))
                        })
                        .collect(),
                )
            })
            .collect::<Result<Vec<_>, _>>()?,
        coverage,
        ResolvedEngineSettingsWorkV2::new(
            work.actual_clip_rows_inspected(),
            work.materialized_clip_rows(),
            work.retained_clip_rows(),
        ),
    )?;
    Ok((profile, settings))
}

/// Project one engine-owned revision-2 profile into the core-owned V2 wire
/// contract without changing the historical V1 projection.
pub fn project_engine_profile_v2(
    profile: &EngineProfileV2,
) -> Result<CoreEngineProfileV2, EngineContractError> {
    let selection = profile.selection();
    CoreEngineProfileV2::new(
        EngineProfileSelectionV1::new(
            selection.family(),
            selection.profile_revision(),
            selection.engine_version(),
            selection.importer(),
        )?,
        profile.profile_urn(),
        profile.facts().to_vec(),
        profile
            .setting_descriptors()
            .iter()
            .map(|descriptor| {
                CoreSettingDescriptorV2::new(
                    setting_id_v2(descriptor.id()),
                    match descriptor.scope() {
                        SettingScope::Document => EngineSettingScopeV1::Document,
                        SettingScope::Clip => EngineSettingScopeV1::Clip,
                    },
                    match descriptor.domain() {
                        SettingDomainV2::Boolean => CoreSettingDomainV2::Boolean,
                        SettingDomainV2::LoadMeshesState | SettingDomainV2::HandlerEnvironment => {
                            CoreSettingDomainV2::Token
                        }
                    },
                    profile.accepted_inputs().to_vec(),
                    match descriptor.default() {
                        SettingDefaultV2::RequiredExplicit => None,
                        SettingDefaultV2::Verified(value) => Some(setting_value_v2(value)),
                    },
                )
            })
            .collect::<Result<Vec<_>, _>>()?,
        profile
            .sources()
            .iter()
            .map(|source| {
                CorePrimarySourceV2::new(
                    source.id(),
                    source.target_version(),
                    source.url(),
                    source.verified_on(),
                    source.supported_facts().to_vec(),
                    source
                        .supported_settings()
                        .iter()
                        .copied()
                        .map(setting_id_v2)
                        .collect(),
                )
            })
            .collect::<Result<Vec<_>, _>>()?,
    )
}

/// Project input-validated revision-2 settings into the core-owned V3
/// origin-bearing contract.
pub fn project_resolved_engine_settings_v3(
    resolved: &ResolvedProfileSettingsV2,
) -> Result<(CoreEngineProfileV2, CoreSettingsV3), EngineContractError> {
    let profile = project_engine_profile_v2(resolved.profile())?;
    let document_settings = resolved
        .document_settings()
        .iter()
        .map(|(id, resolved)| {
            EngineSettingRowV3::new(
                setting_id_v2(*id),
                setting_value_v2(resolved.value()),
                match resolved.origin() {
                    ResolvedSettingOriginV2::ExplicitConfig => {
                        EngineSettingValueOriginV3::ExplicitConfig
                    }
                    ResolvedSettingOriginV2::ProfileDefault => {
                        EngineSettingValueOriginV3::ProfileDefault
                    }
                },
            )
        })
        .collect::<Vec<_>>();
    let settings = CoreSettingsV3::new(
        &profile,
        resolved.source_format(),
        document_settings,
        vec![],
        CoreSettingsCoverageV2::complete(),
        CoreSettingsWorkV2::new(0, 0, 0),
    )?;
    Ok((profile, settings))
}

fn project_profile(
    profile: &EngineProfile,
) -> Result<ResolvedEngineProfileV1, EngineContractError> {
    let selection = profile.selection();
    ResolvedEngineProfileV1::new(
        EngineProfileSelectionV1::new(
            selection.family(),
            selection.profile_revision(),
            selection.engine_version(),
            selection.importer(),
        )?,
        profile.fact_bundle_urn(),
        profile
            .facts()
            .iter()
            .map(|fact| EngineProfileFactV1::new(fact_id(fact.id()), fact_state(fact.state())))
            .collect(),
        profile
            .setting_descriptors()
            .iter()
            .map(|descriptor| {
                EngineSettingDescriptorV1::new(
                    setting_id(descriptor.id()),
                    match descriptor.scope() {
                        SettingScope::Document => EngineSettingScopeV1::Document,
                        SettingScope::Clip => EngineSettingScopeV1::Clip,
                    },
                    match descriptor.domain() {
                        crate::SettingDomain::Boolean => EngineSettingDomainV1::Boolean,
                        crate::SettingDomain::BakeOrExtract => EngineSettingDomainV1::BakeOrExtract,
                        crate::SettingDomain::SourceTransformPath => {
                            EngineSettingDomainV1::SourceTransformPath
                        }
                    },
                    match descriptor.applicability() {
                        SettingApplicability::Applicable => {
                            EngineSettingApplicabilityV1::Applicable
                        }
                        SettingApplicability::NotApplicable => {
                            EngineSettingApplicabilityV1::NotApplicable
                        }
                    },
                    match descriptor.default_status() {
                        DefaultStatus::RequiredWithoutDefault => {
                            EngineDefaultStatusV1::RequiredWithoutDefault
                        }
                        DefaultStatus::NotApplicable => EngineDefaultStatusV1::NotApplicable,
                    },
                )
            })
            .collect(),
        profile
            .sources()
            .iter()
            .map(|source| {
                EnginePrimarySourceV1::new(
                    source.id(),
                    source.target_version(),
                    source.url(),
                    source.verified_on(),
                    source
                        .supported_facts()
                        .iter()
                        .copied()
                        .map(fact_id)
                        .collect(),
                    source
                        .supported_settings()
                        .iter()
                        .copied()
                        .map(setting_id)
                        .collect(),
                )
            })
            .collect::<Result<Vec<_>, _>>()?,
    )
}

fn project_settings<'a>(
    profile: &ResolvedEngineProfileV1,
    document: &BTreeMap<SettingId, SettingValue>,
    clips: impl IntoIterator<Item = (&'a str, &'a BTreeMap<SettingId, SettingValue>)>,
) -> Result<ResolvedEngineSettingsV1, EngineContractError> {
    ResolvedEngineSettingsV1::new(
        profile,
        document
            .iter()
            .map(|(id, value)| EngineSettingRowV1::new(setting_id(*id), setting_value(value)))
            .collect(),
        clips
            .into_iter()
            .map(|(clip_name, values)| {
                EngineClipSettingsV1::new(
                    clip_name,
                    values
                        .iter()
                        .map(|(id, value)| {
                            EngineSettingRowV1::new(setting_id(*id), setting_value(value))
                        })
                        .collect(),
                )
            })
            .collect::<Result<Vec<_>, _>>()?,
    )
}

/// Validate one materialized clip row before a bounded resolver advances.
pub(crate) fn validate_clip_settings(
    clip_name: &str,
    values: &BTreeMap<SettingId, SettingValue>,
) -> Result<(), EngineContractError> {
    EngineClipSettingsV1::new(
        clip_name,
        values
            .iter()
            .map(|(id, value)| EngineSettingRowV1::new(setting_id(*id), setting_value(value)))
            .collect(),
    )?;
    Ok(())
}

fn fact_state(state: &FactState) -> EngineFactStateV1 {
    match state {
        FactState::Known(value) => EngineFactStateV1::Known(fact_value(value)),
        FactState::Unknown => EngineFactStateV1::Unknown,
        FactState::NotApplicable => EngineFactStateV1::NotApplicable,
    }
}

fn fact_value(value: &FactValue) -> EngineFactValueV1 {
    match value {
        FactValue::AcceptedFormats(formats) => EngineFactValueV1::AcceptedFormats(formats.clone()),
        FactValue::AnimationAddressability(AnimationAddressability::GltfAssetLabel) => {
            EngineFactValueV1::AnimationAddressability(
                EngineAnimationAddressabilityV1::GltfAssetLabel,
            )
        }
        FactValue::CoordinateBasis(value) => {
            EngineFactValueV1::CoordinateBasis(EngineCoordinateBasisV1 {
                handedness: match value.handedness {
                    Handedness::Left => EngineHandednessV1::Left,
                    Handedness::Right => EngineHandednessV1::Right,
                },
                up_axis: match value.up_axis {
                    UpAxis::X => EngineUpAxisV1::X,
                    UpAxis::Y => EngineUpAxisV1::Y,
                    UpAxis::Z => EngineUpAxisV1::Z,
                },
                forward_axis: match value.forward_axis {
                    ForwardAxis::PositiveX => EngineForwardAxisV1::PositiveX,
                    ForwardAxis::NegativeX => EngineForwardAxisV1::NegativeX,
                    ForwardAxis::PositiveY => EngineForwardAxisV1::PositiveY,
                    ForwardAxis::NegativeY => EngineForwardAxisV1::NegativeY,
                    ForwardAxis::PositiveZ => EngineForwardAxisV1::PositiveZ,
                    ForwardAxis::NegativeZ => EngineForwardAxisV1::NegativeZ,
                },
            })
        }
        FactValue::LinearUnit(value) => EngineFactValueV1::LinearUnit(match value {
            LinearUnit::Metre => EngineLinearUnitV1::Metre,
            LinearUnit::Centimetre => EngineLinearUnitV1::Centimetre,
        }),
        FactValue::ConversionControl(value) => EngineFactValueV1::ConversionControl(match value {
            ConversionControl::ProfileSetting(id) => {
                EngineConversionControlV1::ProfileSetting(setting_id(*id))
            }
            ConversionControl::ImporterOption => EngineConversionControlV1::ImporterOption,
        }),
        FactValue::Boolean(value) => EngineFactValueV1::Boolean(*value),
        FactValue::ImportHandling(value) => EngineFactValueV1::ImportHandling(match value {
            ImportHandling::Preserved => EngineImportHandlingV1::Preserved,
            ImportHandling::Converted => EngineImportHandlingV1::Converted,
            ImportHandling::Discarded => EngineImportHandlingV1::Discarded,
            ImportHandling::Unsupported => EngineImportHandlingV1::Unsupported,
        }),
        FactValue::TargetAddressability(TargetAddressability::NamePathDerivedId) => {
            EngineFactValueV1::TargetAddressability(EngineTargetAddressabilityV1::NamePathDerivedId)
        }
        FactValue::RootMotionAddressability(value) => {
            EngineFactValueV1::RootMotionAddressability(match value {
                RootMotionAddressability::ExactSourceTransformPath => {
                    EngineRootMotionAddressabilityV1::ExactSourceTransformPath
                }
                RootMotionAddressability::HumanoidAvatarBody => {
                    EngineRootMotionAddressabilityV1::HumanoidAvatarBody
                }
            })
        }
    }
}

fn setting_value(value: &SettingValue) -> EngineSettingValueV1 {
    match value {
        SettingValue::Boolean(value) => EngineSettingValueV1::Boolean(*value),
        SettingValue::BakeOrExtract(value) => EngineSettingValueV1::BakeOrExtract(match value {
            BakeOrExtract::Bake => EngineBakeOrExtractV1::Bake,
            BakeOrExtract::Extract => EngineBakeOrExtractV1::Extract,
        }),
        SettingValue::SourceTransformPath(value) => {
            EngineSettingValueV1::SourceTransformPath(value.clone())
        }
    }
}

const fn fact_id(id: crate::FactId) -> EngineFactIdV1 {
    match id {
        crate::FactId::AcceptedInputs => EngineFactIdV1::AcceptedInputs,
        crate::FactId::AnimationAddressability => EngineFactIdV1::AnimationAddressability,
        crate::FactId::TargetCoordinateBasis => EngineFactIdV1::TargetCoordinateBasis,
        crate::FactId::TargetLinearUnit => EngineFactIdV1::TargetLinearUnit,
        crate::FactId::UnitConversionControl => EngineFactIdV1::UnitConversionControl,
        crate::FactId::AxisConversionControl => EngineFactIdV1::AxisConversionControl,
        crate::FactId::ExactAxisConversion => EngineFactIdV1::ExactAxisConversion,
        crate::FactId::ResultingHierarchyScale => EngineFactIdV1::ResultingHierarchyScale,
        crate::FactId::WholeEndFrameRequired => EngineFactIdV1::WholeEndFrameRequired,
        crate::FactId::AnimationChannelHandling => EngineFactIdV1::AnimationChannelHandling,
        crate::FactId::ExtensionHandling => EngineFactIdV1::ExtensionHandling,
        crate::FactId::ConstructHandling => EngineFactIdV1::ConstructHandling,
        crate::FactId::AnimationTargetAddressability => {
            EngineFactIdV1::AnimationTargetAddressability
        }
        crate::FactId::RootMotionAddressability => EngineFactIdV1::RootMotionAddressability,
    }
}

const fn setting_id(id: SettingId) -> EngineSettingIdV1 {
    match id {
        SettingId::ConvertUnits => EngineSettingIdV1::ConvertUnits,
        SettingId::BakeAxisConversion => EngineSettingIdV1::BakeAxisConversion,
        SettingId::RootMotionSource => EngineSettingIdV1::RootMotionSource,
        SettingId::RootRotation => EngineSettingIdV1::RootRotation,
        SettingId::RootPositionY => EngineSettingIdV1::RootPositionY,
        SettingId::RootPositionXz => EngineSettingIdV1::RootPositionXz,
    }
}

const fn setting_id_v2(id: SettingIdV2) -> CoreSettingIdV2 {
    match id {
        SettingIdV2::RotateSceneEntity => CoreSettingIdV2::RotateSceneEntity,
        SettingIdV2::RotateMeshes => CoreSettingIdV2::RotateMeshes,
        SettingIdV2::LoadMeshes => CoreSettingIdV2::LoadMeshes,
        SettingIdV2::ExtensionHandlerEnvironment => CoreSettingIdV2::ExtensionHandlerEnvironment,
        SettingIdV2::BevyAnimationFeature => CoreSettingIdV2::BevyAnimationFeature,
        SettingIdV2::LoadAnimations => CoreSettingIdV2::LoadAnimations,
    }
}

fn setting_value_v2(value: &SettingValueV2) -> CoreSettingValueV2 {
    match value {
        SettingValueV2::Boolean(value) => CoreSettingValueV2::Boolean(*value),
        SettingValueV2::LoadMeshesState(value) => CoreSettingValueV2::Token(
            match value {
                crate::BevyLoadMeshesStateV2::Empty => "empty",
                crate::BevyLoadMeshesStateV2::Nonempty => "nonempty",
            }
            .into(),
        ),
        SettingValueV2::HandlerEnvironment(value) => {
            CoreSettingValueV2::Token(value.as_str().into())
        }
    }
}

pub(crate) const fn format_name(format: SourceFormatV1) -> &'static str {
    match format {
        SourceFormatV1::GltfJson => "gltf_json",
        SourceFormatV1::Glb => "glb",
        SourceFormatV1::Fbx => "fbx",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FactId, ProfileFact, SettingDescriptor};

    #[test]
    fn facts_encoding_sorts_every_repeated_stable_id() {
        for original in crate::profiles_v1() {
            let expected = facts_identity(original);
            let mut reordered = original.clone();
            let accepted = reordered
                .facts
                .iter_mut()
                .find(|fact| fact.id() == FactId::AcceptedInputs)
                .unwrap();
            if let FactState::Known(FactValue::AcceptedFormats(formats)) = accepted.state() {
                let mut formats = formats.clone();
                formats.reverse();
                *accepted = ProfileFact::new(
                    FactId::AcceptedInputs,
                    FactState::Known(FactValue::AcceptedFormats(formats)),
                );
            }
            reordered.facts.reverse();
            reordered.settings.reverse();
            reordered.sources.reverse();
            for source in &mut reordered.sources {
                source.reverse_supported_ids_for_test();
            }
            assert_eq!(
                facts_identity(&reordered),
                expected,
                "{:?}",
                original.selection()
            );
        }
    }

    #[test]
    fn facts_encoding_is_sensitive_to_each_record_domain() {
        let original = crate::profiles_v1()
            .iter()
            .find(|profile| profile.selection.family() == "unity-generic")
            .unwrap()
            .clone();
        let expected = facts_identity(&original);
        let changed = |profile: &EngineProfile| {
            if let Ok(projected) = project_profile(profile) {
                assert_ne!(projected.facts_identity(), &expected);
            }
        };

        let mut profile = original.clone();
        profile.selection = crate::ProfileSelection::new(
            "unity-generic-changed",
            1,
            "6000.3",
            "fbx-model-importer",
        );
        changed(&profile);

        let mut profile = original.clone();
        profile.selection =
            crate::ProfileSelection::new("unity-generic", 2, "6000.3", "fbx-model-importer");
        changed(&profile);

        let mut profile = original.clone();
        profile.selection =
            crate::ProfileSelection::new("unity-generic", 1, "6000.4", "fbx-model-importer");
        changed(&profile);

        let mut profile = original.clone();
        profile.selection =
            crate::ProfileSelection::new("unity-generic", 1, "6000.3", "other-importer");
        changed(&profile);

        let mut profile = original.clone();
        profile.fact_bundle_urn = "urn:animsmith:engine-profile:unity-generic:changed";
        changed(&profile);

        let mut profile = original.clone();
        let fact = profile
            .facts
            .iter_mut()
            .find(|fact| fact.id() == FactId::ExactAxisConversion)
            .unwrap();
        *fact = ProfileFact::new(FactId::ExactAxisConversion, FactState::NotApplicable);
        changed(&profile);

        let mut profile = original.clone();
        let fact = profile
            .facts
            .iter_mut()
            .find(|fact| fact.id() == FactId::AcceptedInputs)
            .unwrap();
        *fact = ProfileFact::new(
            FactId::AcceptedInputs,
            FactState::Known(FactValue::AcceptedFormats(vec![
                SourceFormatV1::Fbx,
                SourceFormatV1::Glb,
            ])),
        );
        changed(&profile);

        let mut profile = original.clone();
        let descriptor = profile.settings[0].clone();
        profile.settings[0] = SettingDescriptor::new(
            descriptor.id(),
            descriptor.scope(),
            crate::SettingDomain::BakeOrExtract,
            descriptor.applicability(),
            descriptor.default_status(),
        );
        changed(&profile);

        let mut profile = original.clone();
        let descriptor = profile.settings[0].clone();
        profile.settings[0] = SettingDescriptor::new(
            descriptor.id(),
            crate::SettingScope::Clip,
            descriptor.domain(),
            descriptor.applicability(),
            descriptor.default_status(),
        );
        changed(&profile);

        let mut profile = original.clone();
        let descriptor = profile.settings[0].clone();
        profile.settings[0] = SettingDescriptor::new(
            descriptor.id(),
            descriptor.scope(),
            descriptor.domain(),
            crate::SettingApplicability::NotApplicable,
            descriptor.default_status(),
        );
        changed(&profile);

        let mut profile = original.clone();
        let descriptor = profile.settings[0].clone();
        profile.settings[0] = SettingDescriptor::new(
            descriptor.id(),
            descriptor.scope(),
            descriptor.domain(),
            descriptor.applicability(),
            crate::DefaultStatus::NotApplicable,
        );
        changed(&profile);

        let mut profile = original.clone();
        let source = profile.sources[0].clone();
        profile.sources[0] = crate::PrimarySource::new(
            "changed-source-id",
            source.target_version(),
            source.url(),
            source.verified_on(),
            source.supported_facts().to_vec(),
            source.supported_settings().to_vec(),
        );
        changed(&profile);

        let replace_source = |profile: &mut EngineProfile,
                              target_version,
                              url,
                              verified_on,
                              facts: Vec<_>,
                              settings: Vec<_>| {
            let source = profile.sources[0].clone();
            profile.sources[0] = crate::PrimarySource::new(
                source.id(),
                target_version,
                url,
                verified_on,
                facts,
                settings,
            );
        };

        let mut profile = original.clone();
        let source = profile.sources[0].clone();
        replace_source(
            &mut profile,
            "changed-version",
            source.url(),
            source.verified_on(),
            source.supported_facts().to_vec(),
            source.supported_settings().to_vec(),
        );
        changed(&profile);

        let mut profile = original.clone();
        let source = profile.sources[0].clone();
        replace_source(
            &mut profile,
            source.target_version(),
            "https://example.invalid/changed",
            source.verified_on(),
            source.supported_facts().to_vec(),
            source.supported_settings().to_vec(),
        );
        changed(&profile);

        let mut profile = original.clone();
        let source = profile.sources[0].clone();
        replace_source(
            &mut profile,
            source.target_version(),
            source.url(),
            "2099-01-01",
            source.supported_facts().to_vec(),
            source.supported_settings().to_vec(),
        );
        changed(&profile);

        let mut profile = original.clone();
        let source = profile.sources[0].clone();
        let mut facts = source.supported_facts().to_vec();
        facts.push(FactId::AcceptedInputs);
        replace_source(
            &mut profile,
            source.target_version(),
            source.url(),
            source.verified_on(),
            facts,
            source.supported_settings().to_vec(),
        );
        changed(&profile);

        let mut profile = original.clone();
        let source = profile.sources[0].clone();
        let mut settings = source.supported_settings().to_vec();
        settings.pop();
        replace_source(
            &mut profile,
            source.target_version(),
            source.url(),
            source.verified_on(),
            source.supported_facts().to_vec(),
            settings,
        );
        changed(&profile);

        let unreal = crate::profiles_v1()
            .iter()
            .find(|profile| profile.selection.family() == "unreal")
            .unwrap();
        let expected = facts_identity(unreal);
        let mut profile = unreal.clone();
        let fact = profile
            .facts
            .iter_mut()
            .find(|fact| fact.id() == FactId::TargetCoordinateBasis)
            .unwrap();
        *fact = ProfileFact::new(
            FactId::TargetCoordinateBasis,
            FactState::Known(FactValue::CoordinateBasis(crate::CoordinateBasis {
                handedness: crate::Handedness::Left,
                up_axis: crate::UpAxis::Z,
                forward_axis: crate::ForwardAxis::NegativeX,
            })),
        );
        assert_ne!(facts_identity(&profile), expected);
    }
}
