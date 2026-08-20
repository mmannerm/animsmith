use crate::canonical::facts_identity;
use crate::{
    AnimationAddressability, ConversionControl, CoordinateBasis, DefaultStatus, EngineProfile,
    FactId, FactState, FactValue, ForwardAxis, Handedness, LinearUnit, PrimarySource, ProfileFact,
    ProfileSelection, RegistryValidationError, RootMotionAddressability, SettingApplicability,
    SettingDescriptor, SettingDomain, SettingId, SettingScope, TargetAddressability, UpAxis,
};
use animsmith_core::{InputIdentity, SourceFormatV1};
use std::collections::BTreeSet;
use std::sync::OnceLock;

const VERIFIED_ON: &str = "2026-08-20";
const ALL_FACT_IDS: [FactId; 14] = [
    FactId::AcceptedInputs,
    FactId::AnimationAddressability,
    FactId::TargetCoordinateBasis,
    FactId::TargetLinearUnit,
    FactId::UnitConversionControl,
    FactId::AxisConversionControl,
    FactId::ExactAxisConversion,
    FactId::ResultingHierarchyScale,
    FactId::WholeEndFrameRequired,
    FactId::AnimationChannelHandling,
    FactId::ExtensionHandling,
    FactId::ConstructHandling,
    FactId::AnimationTargetAddressability,
    FactId::RootMotionAddressability,
];

/// Enumerate the five immutable V1 engine profiles in tuple order.
pub fn profiles_v1() -> &'static [EngineProfile] {
    static PROFILES: OnceLock<Vec<EngineProfile>> = OnceLock::new();
    PROFILES.get_or_init(|| {
        let mut profiles = vec![unity_generic(), unity_humanoid(), unreal(), godot(), bevy()];
        profiles.sort_by(|left, right| left.selection.cmp(&right.selection));
        profiles
    })
}

/// Validate all author-owned registry declarations and cross references.
///
/// This checks tuple and URN uniqueness, complete fact inventories, the
/// canonical accepted-input fact, descriptor/default consistency, and every
/// source's stable fact/setting references.
///
/// # Errors
///
/// Returns a typed invariant error naming the affected profile and field.
pub fn validate_registry_v1() -> Result<(), RegistryValidationError> {
    validate_profiles(profiles_v1())
}

fn validate_profiles(profiles: &[EngineProfile]) -> Result<(), RegistryValidationError> {
    if profiles.len() != 5 {
        return Err(RegistryValidationError::ProfileCount {
            found: profiles.len(),
        });
    }
    let mut selections = BTreeSet::new();
    let mut urns = BTreeSet::new();
    for profile in profiles {
        if !selections.insert(profile.selection.clone()) {
            return Err(RegistryValidationError::DuplicateSelection {
                selection: profile.selection.clone(),
            });
        }
        if !urns.insert(profile.fact_bundle_urn) {
            return Err(RegistryValidationError::DuplicateFactBundleUrn {
                urn: profile.fact_bundle_urn,
            });
        }
        let fact_ids: BTreeSet<_> = profile.facts.iter().map(ProfileFact::id).collect();
        if fact_ids.len() != profile.facts.len()
            || fact_ids != ALL_FACT_IDS.into_iter().collect::<BTreeSet<_>>()
        {
            return Err(RegistryValidationError::InvalidFactInventory {
                selection: profile.selection.clone(),
            });
        }
        let accepted_inputs = profile.accepted_inputs();
        if accepted_inputs.is_empty()
            || !accepted_inputs.windows(2).all(|pair| {
                crate::canonical::format_name(pair[0]) < crate::canonical::format_name(pair[1])
            })
        {
            return Err(RegistryValidationError::InvalidAcceptedInputFact {
                selection: profile.selection.clone(),
            });
        }
        let setting_ids: BTreeSet<_> = profile.settings.iter().map(SettingDescriptor::id).collect();
        if setting_ids.len() != profile.settings.len() {
            return Err(RegistryValidationError::DuplicateSettingDescriptor {
                selection: profile.selection.clone(),
            });
        }
        for descriptor in &profile.settings {
            let valid_default = matches!(
                (descriptor.applicability(), descriptor.default_status()),
                (
                    SettingApplicability::Applicable,
                    DefaultStatus::RequiredWithoutDefault
                ) | (
                    SettingApplicability::NotApplicable,
                    DefaultStatus::NotApplicable
                )
            );
            if !valid_default {
                return Err(RegistryValidationError::InvalidDescriptorDefault {
                    selection: profile.selection.clone(),
                    setting: descriptor.id(),
                });
            }
        }
        let mut source_ids = BTreeSet::new();
        for source in &profile.sources {
            if !source_ids.insert(source.id()) {
                return Err(RegistryValidationError::DuplicateSourceId {
                    selection: profile.selection.clone(),
                    source_id: source.id(),
                });
            }
            for fact in source.supported_facts() {
                if !fact_ids.contains(fact) {
                    return Err(RegistryValidationError::UnknownSourceFact {
                        selection: profile.selection.clone(),
                        source_id: source.id(),
                        fact: *fact,
                    });
                }
                if !matches!(
                    profile.fact(*fact).map(ProfileFact::state),
                    Some(FactState::Known(_))
                ) {
                    return Err(RegistryValidationError::SourceReferencesNonKnownFact {
                        selection: profile.selection.clone(),
                        source_id: source.id(),
                        fact: *fact,
                    });
                }
            }
            for setting in source.supported_settings() {
                if !setting_ids.contains(setting) {
                    return Err(RegistryValidationError::UnknownSourceSetting {
                        selection: profile.selection.clone(),
                        source_id: source.id(),
                        setting: *setting,
                    });
                }
            }
        }
        for fact in &profile.facts {
            if matches!(fact.state(), FactState::Known(_))
                && !profile
                    .sources
                    .iter()
                    .any(|source| source.supported_facts().contains(&fact.id()))
            {
                return Err(RegistryValidationError::UnreferencedKnownFact {
                    selection: profile.selection.clone(),
                    fact: fact.id(),
                });
            }
        }
        for descriptor in &profile.settings {
            if !profile
                .sources
                .iter()
                .any(|source| source.supported_settings().contains(&descriptor.id()))
            {
                return Err(RegistryValidationError::UnreferencedSetting {
                    selection: profile.selection.clone(),
                    setting: descriptor.id(),
                });
            }
        }
        if facts_identity(profile) != profile.facts_identity {
            return Err(RegistryValidationError::FactsIdentityMismatch {
                selection: profile.selection.clone(),
            });
        }
    }
    Ok(())
}

fn unity_generic() -> EngineProfile {
    let accepted = vec![SourceFormatV1::Fbx];
    let facts = base_facts(
        &accepted,
        vec![
            (
                FactId::UnitConversionControl,
                FactValue::ConversionControl(ConversionControl::ProfileSetting(
                    SettingId::ConvertUnits,
                )),
            ),
            (
                FactId::AxisConversionControl,
                FactValue::ConversionControl(ConversionControl::ProfileSetting(
                    SettingId::BakeAxisConversion,
                )),
            ),
            (
                FactId::RootMotionAddressability,
                FactValue::RootMotionAddressability(
                    RootMotionAddressability::ExactSourceTransformPath,
                ),
            ),
        ],
    );
    finish_profile(
        ProfileSelection::new("unity-generic", 1, "6000.3", "fbx-model-importer"),
        "urn:animsmith:engine-profile:unity-generic:1",
        facts,
        unity_settings(true),
        unity_sources(true),
    )
}

fn unity_humanoid() -> EngineProfile {
    let accepted = vec![SourceFormatV1::Fbx];
    let facts = base_facts(
        &accepted,
        vec![
            (
                FactId::UnitConversionControl,
                FactValue::ConversionControl(ConversionControl::ProfileSetting(
                    SettingId::ConvertUnits,
                )),
            ),
            (
                FactId::AxisConversionControl,
                FactValue::ConversionControl(ConversionControl::ProfileSetting(
                    SettingId::BakeAxisConversion,
                )),
            ),
            (
                FactId::RootMotionAddressability,
                FactValue::RootMotionAddressability(RootMotionAddressability::HumanoidAvatarBody),
            ),
        ],
    );
    finish_profile(
        ProfileSelection::new("unity-humanoid", 1, "6000.3", "fbx-model-importer"),
        "urn:animsmith:engine-profile:unity-humanoid:1",
        facts,
        unity_settings(false),
        unity_sources(false),
    )
}

fn unreal() -> EngineProfile {
    let accepted = vec![SourceFormatV1::Fbx];
    let facts = base_facts(
        &accepted,
        vec![
            (
                FactId::TargetCoordinateBasis,
                FactValue::CoordinateBasis(CoordinateBasis {
                    handedness: Handedness::Left,
                    up_axis: UpAxis::Z,
                    forward_axis: ForwardAxis::PositiveX,
                }),
            ),
            (
                FactId::TargetLinearUnit,
                FactValue::LinearUnit(LinearUnit::Centimetre),
            ),
            (
                FactId::UnitConversionControl,
                FactValue::ConversionControl(ConversionControl::ImporterOption),
            ),
            (
                FactId::AxisConversionControl,
                FactValue::ConversionControl(ConversionControl::ImporterOption),
            ),
            (FactId::WholeEndFrameRequired, FactValue::Boolean(true)),
        ],
    );
    finish_profile(
        ProfileSelection::new("unreal", 1, "5.8", "fbx-importer"),
        "urn:animsmith:engine-profile:unreal:1",
        facts,
        vec![],
        vec![
            source(
                "unreal-animation-sequences-5.8",
                "5.8",
                "https://dev.epicgames.com/documentation/en-us/unreal-engine/animation-sequences-in-unreal-engine?application_version=5.8",
                &[FactId::WholeEndFrameRequired],
                &[],
            ),
            source(
                "unreal-coordinate-system-5.8",
                "5.8",
                "https://dev.epicgames.com/documentation/en-us/unreal-engine/coordinate-system-and-spaces-in-unreal-engine?application_version=5.8",
                &[FactId::TargetCoordinateBasis],
                &[],
            ),
            source(
                "unreal-fbx-import-options-5.8",
                "5.8",
                "https://dev.epicgames.com/documentation/en-us/unreal-engine/fbx-import-options-reference-in-unreal-engine?application_version=5.8",
                &[
                    FactId::AcceptedInputs,
                    FactId::UnitConversionControl,
                    FactId::AxisConversionControl,
                ],
                &[],
            ),
            source(
                "unreal-units-5.8",
                "5.8",
                "https://dev.epicgames.com/documentation/en-us/unreal-engine/units-of-measurement-in-unreal-engine?application_version=5.8",
                &[FactId::TargetLinearUnit],
                &[],
            ),
        ],
    )
}

fn godot() -> EngineProfile {
    let accepted = vec![
        SourceFormatV1::GltfJson,
        SourceFormatV1::Glb,
        SourceFormatV1::Fbx,
    ];
    let facts = base_facts(&accepted, vec![]);
    finish_profile(
        ProfileSelection::new("godot", 1, "4.7", "resource-importer-scene"),
        "urn:animsmith:engine-profile:godot:1",
        facts,
        vec![],
        vec![source(
            "godot-resource-importer-scene-4.7",
            "4.7",
            "https://docs.godotengine.org/en/4.7/classes/class_resourceimporterscene.html",
            &[FactId::AcceptedInputs],
            &[],
        )],
    )
}

fn bevy() -> EngineProfile {
    let accepted = vec![SourceFormatV1::GltfJson, SourceFormatV1::Glb];
    let facts = base_facts(
        &accepted,
        vec![
            (
                FactId::AnimationAddressability,
                FactValue::AnimationAddressability(AnimationAddressability::GltfAssetLabel),
            ),
            (
                FactId::AnimationTargetAddressability,
                FactValue::TargetAddressability(TargetAddressability::NamePathDerivedId),
            ),
        ],
    );
    finish_profile(
        ProfileSelection::new("bevy", 1, "0.19.0", "gltf-asset-loader"),
        "urn:animsmith:engine-profile:bevy:1",
        facts,
        vec![],
        vec![
            source(
                "bevy-animation-target-id-0.19.0",
                "0.19.0",
                "https://docs.rs/bevy/0.19.0/bevy/animation/struct.AnimationTargetId.html",
                &[FactId::AnimationTargetAddressability],
                &[],
            ),
            source(
                "bevy-gltf-asset-label-0.19.0",
                "0.19.0",
                "https://docs.rs/bevy/0.19.0/bevy/gltf/enum.GltfAssetLabel.html",
                &[FactId::AcceptedInputs, FactId::AnimationAddressability],
                &[],
            ),
            source(
                "khronos-gltf-2.0",
                "2.0",
                "https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html",
                &[FactId::AcceptedInputs],
                &[],
            ),
        ],
    )
}

fn base_facts(accepted: &[SourceFormatV1], known: Vec<(FactId, FactValue)>) -> Vec<ProfileFact> {
    let mut accepted = accepted.to_vec();
    accepted.sort_by_key(|format| crate::canonical::format_name(*format));
    let known: std::collections::BTreeMap<_, _> = known.into_iter().collect();
    ALL_FACT_IDS
        .into_iter()
        .map(|id| {
            let state = if id == FactId::AcceptedInputs {
                FactState::Known(FactValue::AcceptedFormats(accepted.clone()))
            } else if let Some(value) = known.get(&id) {
                FactState::Known(value.clone())
            } else {
                FactState::Unknown
            };
            ProfileFact::new(id, state)
        })
        .collect()
}

fn unity_settings(generic: bool) -> Vec<SettingDescriptor> {
    use DefaultStatus::{NotApplicable, RequiredWithoutDefault};
    use SettingApplicability::{Applicable, NotApplicable as DoesNotApply};
    vec![
        SettingDescriptor::new(
            SettingId::ConvertUnits,
            SettingScope::Document,
            SettingDomain::Boolean,
            Applicable,
            RequiredWithoutDefault,
        ),
        SettingDescriptor::new(
            SettingId::BakeAxisConversion,
            SettingScope::Document,
            SettingDomain::Boolean,
            Applicable,
            RequiredWithoutDefault,
        ),
        SettingDescriptor::new(
            SettingId::RootMotionSource,
            SettingScope::Document,
            SettingDomain::SourceTransformPath,
            if generic { Applicable } else { DoesNotApply },
            if generic {
                RequiredWithoutDefault
            } else {
                NotApplicable
            },
        ),
        SettingDescriptor::new(
            SettingId::RootRotation,
            SettingScope::Clip,
            SettingDomain::BakeOrExtract,
            Applicable,
            RequiredWithoutDefault,
        ),
        SettingDescriptor::new(
            SettingId::RootPositionY,
            SettingScope::Clip,
            SettingDomain::BakeOrExtract,
            Applicable,
            RequiredWithoutDefault,
        ),
        SettingDescriptor::new(
            SettingId::RootPositionXz,
            SettingScope::Clip,
            SettingDomain::BakeOrExtract,
            Applicable,
            RequiredWithoutDefault,
        ),
    ]
}

fn unity_sources(generic: bool) -> Vec<PrimarySource> {
    let mut sources = vec![
        source(
            "unity-animation-clip-import-6000.3",
            "6000.3",
            "https://docs.unity3d.com/6000.3/Documentation/Manual/class-AnimationClip.html",
            &[],
            &[
                SettingId::RootRotation,
                SettingId::RootPositionY,
                SettingId::RootPositionXz,
            ],
        ),
        source(
            "unity-model-import-6000.3",
            "6000.3",
            "https://docs.unity3d.com/6000.3/Documentation/Manual/FBXImporter-Model.html",
            &[
                FactId::AcceptedInputs,
                FactId::UnitConversionControl,
                FactId::AxisConversionControl,
            ],
            &[SettingId::ConvertUnits, SettingId::BakeAxisConversion],
        ),
        source(
            "unity-model-importer-clip-animation-6000.3",
            "6000.3",
            "https://docs.unity3d.com/6000.3/Documentation/ScriptReference/ModelImporterClipAnimation.html",
            &[],
            &[
                SettingId::RootRotation,
                SettingId::RootPositionY,
                SettingId::RootPositionXz,
            ],
        ),
        source(
            "unity-rig-import-6000.3",
            "6000.3",
            "https://docs.unity3d.com/6000.3/Documentation/Manual/FBXImporter-Rig.html",
            &[FactId::RootMotionAddressability],
            if generic {
                &[]
            } else {
                &[SettingId::RootMotionSource]
            },
        ),
    ];
    if generic {
        sources.push(source(
            "unity-model-importer-motion-node-name-6000.3",
            "6000.3",
            "https://docs.unity3d.com/6000.3/Documentation/ScriptReference/ModelImporter-motionNodeName.html",
            &[FactId::RootMotionAddressability],
            &[SettingId::RootMotionSource],
        ));
    }
    sources
}

fn source(
    id: &'static str,
    target_version: &'static str,
    url: &'static str,
    facts: &[FactId],
    settings: &[SettingId],
) -> PrimarySource {
    PrimarySource::new(
        id,
        target_version,
        url,
        VERIFIED_ON,
        facts.to_vec(),
        settings.to_vec(),
    )
}

fn finish_profile(
    selection: ProfileSelection,
    fact_bundle_urn: &'static str,
    mut facts: Vec<ProfileFact>,
    mut settings: Vec<SettingDescriptor>,
    mut sources: Vec<PrimarySource>,
) -> EngineProfile {
    facts.sort_by_key(|fact| fact.id().as_str());
    settings.sort_by_key(|descriptor| descriptor.id().as_str());
    sources.sort_by_key(PrimarySource::id);
    let mut profile = EngineProfile {
        selection,
        fact_bundle_urn,
        facts,
        settings,
        sources,
        facts_identity: InputIdentity::from_bytes(&[]),
    };
    profile.facts_identity = facts_identity(&profile);
    profile
}

#[cfg(test)]
mod tests {
    use super::*;

    fn without_fact(source: &PrimarySource, removed: FactId) -> PrimarySource {
        PrimarySource::new(
            source.id(),
            source.target_version(),
            source.url(),
            source.verified_on(),
            source
                .supported_facts()
                .iter()
                .copied()
                .filter(|id| *id != removed)
                .collect(),
            source.supported_settings().to_vec(),
        )
    }

    fn without_setting(source: &PrimarySource, removed: SettingId) -> PrimarySource {
        PrimarySource::new(
            source.id(),
            source.target_version(),
            source.url(),
            source.verified_on(),
            source.supported_facts().to_vec(),
            source
                .supported_settings()
                .iter()
                .copied()
                .filter(|id| *id != removed)
                .collect(),
        )
    }

    #[test]
    fn validation_refuses_known_fact_without_source_coverage() {
        let mut profiles = profiles_v1().to_vec();
        let bevy = profiles
            .iter_mut()
            .find(|profile| profile.selection.family() == "bevy")
            .unwrap();
        bevy.sources = bevy
            .sources
            .iter()
            .map(|source| without_fact(source, FactId::AnimationTargetAddressability))
            .collect();
        assert!(matches!(
            validate_profiles(&profiles),
            Err(RegistryValidationError::UnreferencedKnownFact {
                fact: FactId::AnimationTargetAddressability,
                ..
            })
        ));
    }

    #[test]
    fn validation_refuses_not_applicable_descriptor_without_source_coverage() {
        let mut profiles = profiles_v1().to_vec();
        let humanoid = profiles
            .iter_mut()
            .find(|profile| profile.selection.family() == "unity-humanoid")
            .unwrap();
        humanoid.sources = humanoid
            .sources
            .iter()
            .map(|source| without_setting(source, SettingId::RootMotionSource))
            .collect();
        assert!(matches!(
            validate_profiles(&profiles),
            Err(RegistryValidationError::UnreferencedSetting {
                setting: SettingId::RootMotionSource,
                ..
            })
        ));
    }

    #[test]
    fn validation_refuses_every_noncanonical_accepted_input_fact_shape() {
        for state in [
            FactState::Unknown,
            FactState::Known(FactValue::AcceptedFormats(vec![])),
            FactState::Known(FactValue::AcceptedFormats(vec![
                SourceFormatV1::GltfJson,
                SourceFormatV1::Fbx,
            ])),
        ] {
            let mut profiles = profiles_v1().to_vec();
            let godot = profiles
                .iter_mut()
                .find(|profile| profile.selection.family() == "godot")
                .unwrap();
            let accepted = godot
                .facts
                .iter_mut()
                .find(|fact| fact.id() == FactId::AcceptedInputs)
                .unwrap();
            *accepted = ProfileFact::new(FactId::AcceptedInputs, state);
            assert!(matches!(
                validate_profiles(&profiles),
                Err(RegistryValidationError::InvalidAcceptedInputFact { .. })
            ));
        }
    }
}
