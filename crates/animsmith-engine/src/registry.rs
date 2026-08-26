use crate::canonical::facts_identity;
use crate::{
    AnimationAddressability, ConversionControl, CoordinateBasis, DefaultStatus, EngineProfile,
    EngineProfileV2, FactId, FactState, FactValue, ForwardAxis, Handedness, LinearUnit,
    PrimarySource, PrimarySourceV2, ProfileFact, ProfileSelection, RegistryValidationError,
    RootMotionAddressability, SettingApplicability, SettingDefaultV2, SettingDescriptor,
    SettingDescriptorV2, SettingDomain, SettingDomainV2, SettingId, SettingIdV2, SettingScope,
    SettingValueV2, TargetAddressability, UpAxis,
};
use animsmith_core::{InputIdentity, SourceFormatV1};
use std::collections::BTreeSet;
use std::sync::OnceLock;

const VERIFIED_ON: &str = "2026-08-20";
const UNITY_GENERIC_V2_VERIFIED_ON: &str = "2026-08-26";
const IMPORT_ADVICE_V2_VERIFIED_ON: &str = "2026-08-26";
const BEVY_V2_VERIFIED_ON: &str = "2026-08-25";
const BEVY_0_19_COMMIT: &str = "c6f634ca9f406d68ba5109d921247b654cb42c10";
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

/// Enumerate immutable V2-contract profile records in exact tuple order.
///
/// This registry is separate from [`profiles_v1`], whose five records and
/// canonical identities remain unchanged.
pub fn profiles_v2() -> &'static [EngineProfileV2] {
    static PROFILES: OnceLock<Vec<EngineProfileV2>> = OnceLock::new();
    PROFILES.get_or_init(|| {
        let mut profiles = vec![
            unity_generic_v2(),
            unreal_v2(),
            godot_v2(),
            bevy_v2(),
            bevy_v3(),
        ];
        profiles.sort_by(|left, right| left.selection().cmp(right.selection()));
        profiles
    })
}

/// Validate revision-2 registry declarations and source cross-references.
///
/// # Errors
///
/// Returns a typed invariant error naming the affected profile or setting.
pub fn validate_registry_v2() -> Result<(), RegistryValidationErrorV2> {
    use animsmith_core::engine_contract::{EngineFactIdV2, EngineFactStateV2};
    const FACT_IDS: [EngineFactIdV2; 10] = [
        EngineFactIdV2::AcceptedInputs,
        EngineFactIdV2::ApplicationWorldUnitPolicy,
        EngineFactIdV2::ImporterScaleConversion,
        EngineFactIdV2::ImportSettingProjection,
        EngineFactIdV2::PhysicalDimensionsPreserved,
        EngineFactIdV2::ResultingTransformScale,
        EngineFactIdV2::RootMotionAddressability,
        EngineFactIdV2::SourceImportDisposition,
        EngineFactIdV2::SourceToTargetUnitMapping,
        EngineFactIdV2::TargetLinearUnit,
    ];
    let profiles = profiles_v2();
    if profiles.len() != 5 {
        return Err(RegistryValidationErrorV2::ProfileCount {
            found: profiles.len(),
        });
    }
    let mut selections = BTreeSet::new();
    let mut urns = BTreeSet::new();
    for profile in profiles {
        if !selections.insert(profile.selection().clone()) {
            return Err(RegistryValidationErrorV2::DuplicateSelection {
                selection: profile.selection().clone(),
            });
        }
        if !urns.insert(profile.profile_urn()) {
            return Err(RegistryValidationErrorV2::DuplicateProfileUrn {
                urn: profile.profile_urn(),
            });
        }
        if profile.accepted_inputs().is_empty()
            || !profile.accepted_inputs().windows(2).all(|pair| {
                crate::canonical::format_name(pair[0]) < crate::canonical::format_name(pair[1])
            })
        {
            return Err(RegistryValidationErrorV2::InvalidAcceptedInputs {
                selection: profile.selection().clone(),
            });
        }
        let fact_ids: BTreeSet<_> = profile.facts().iter().map(|fact| fact.id()).collect();
        if fact_ids.len() != profile.facts().len()
            || fact_ids != FACT_IDS.into_iter().collect::<BTreeSet<_>>()
        {
            return Err(RegistryValidationErrorV2::InvalidFactInventory {
                selection: profile.selection().clone(),
            });
        }

        let setting_ids: BTreeSet<_> = profile
            .setting_descriptors()
            .iter()
            .map(SettingDescriptorV2::id)
            .collect();
        if setting_ids.len() != profile.setting_descriptors().len() {
            return Err(RegistryValidationErrorV2::DuplicateSettingDescriptor {
                selection: profile.selection().clone(),
            });
        }
        for descriptor in profile.setting_descriptors() {
            if let SettingDefaultV2::Verified(value) = descriptor.default()
                && !value.matches_domain(descriptor.domain())
            {
                return Err(RegistryValidationErrorV2::InvalidVerifiedDefault {
                    selection: profile.selection().clone(),
                    setting: descriptor.id(),
                });
            }
        }

        let mut source_ids = BTreeSet::new();
        let mut supported_settings = BTreeSet::new();
        let mut supported_facts = BTreeSet::new();
        let mut accepted_inputs_supported = false;
        for source in profile.sources() {
            if !source_ids.insert(source.id()) {
                return Err(RegistryValidationErrorV2::DuplicateSourceId {
                    selection: profile.selection().clone(),
                    source_id: source.id(),
                });
            }
            accepted_inputs_supported |= source.supports_accepted_inputs();
            for fact in source.supported_facts() {
                let state = profile
                    .facts()
                    .iter()
                    .find(|row| row.id() == *fact)
                    .map(|row| row.state());
                if !matches!(state, Some(EngineFactStateV2::Known(_))) {
                    return Err(RegistryValidationErrorV2::UnknownSourceFact {
                        selection: profile.selection().clone(),
                        source_id: source.id(),
                        fact: *fact,
                    });
                }
                supported_facts.insert(*fact);
            }
            for setting in source.supported_settings() {
                if !setting_ids.contains(setting) {
                    return Err(RegistryValidationErrorV2::UnknownSourceSetting {
                        selection: profile.selection().clone(),
                        source_id: source.id(),
                        setting: *setting,
                    });
                }
                supported_settings.insert(*setting);
            }
        }
        if !accepted_inputs_supported {
            return Err(RegistryValidationErrorV2::UnreferencedAcceptedInputs {
                selection: profile.selection().clone(),
            });
        }
        if let Some(fact) = profile
            .facts()
            .iter()
            .filter(|row| matches!(row.state(), EngineFactStateV2::Known(_)))
            .map(|row| row.id())
            .find(|fact| !supported_facts.contains(fact))
        {
            return Err(RegistryValidationErrorV2::UnreferencedFact {
                selection: profile.selection().clone(),
                fact,
            });
        }
        if let Some(setting) = setting_ids.difference(&supported_settings).next() {
            return Err(RegistryValidationErrorV2::UnreferencedSetting {
                selection: profile.selection().clone(),
                setting: *setting,
            });
        }
    }
    Ok(())
}

/// Author-owned invariant failure in the revision-2 registry.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum RegistryValidationErrorV2 {
    /// The registry does not contain the authorized V2 profile set.
    #[error("V2 registry contains {found} profiles rather than the authorized set")]
    ProfileCount {
        /// Actual profile count.
        found: usize,
    },
    /// Two records use the same exact tuple.
    #[error("duplicate V2 profile selection {selection:?}")]
    DuplicateSelection {
        /// Repeated selection.
        selection: ProfileSelection,
    },
    /// Two records use the same stable record URN.
    #[error("duplicate V2 profile URN {urn}")]
    DuplicateProfileUrn {
        /// Repeated URN.
        urn: &'static str,
    },
    /// Accepted inputs are empty, repeated, or not canonically sorted.
    #[error("invalid V2 accepted inputs for {selection:?}")]
    InvalidAcceptedInputs {
        /// Affected selection.
        selection: ProfileSelection,
    },
    /// The record does not contain every V2 fact exactly once.
    #[error("invalid V2 fact inventory for {selection:?}")]
    InvalidFactInventory {
        /// Affected selection.
        selection: ProfileSelection,
    },
    /// A setting descriptor id is repeated.
    #[error("duplicate V2 setting descriptor for {selection:?}")]
    DuplicateSettingDescriptor {
        /// Affected selection.
        selection: ProfileSelection,
    },
    /// A verified default does not match its descriptor domain.
    #[error("invalid V2 verified default for {setting:?} in {selection:?}")]
    InvalidVerifiedDefault {
        /// Affected selection.
        selection: ProfileSelection,
        /// Affected setting.
        setting: SettingIdV2,
    },
    /// A primary-source id is repeated within one profile.
    #[error("duplicate V2 source id {source_id} in {selection:?}")]
    DuplicateSourceId {
        /// Affected selection.
        selection: ProfileSelection,
        /// Repeated source id.
        source_id: &'static str,
    },
    /// A source references a setting absent from the record.
    #[error("V2 source {source_id} references unknown setting {setting:?} in {selection:?}")]
    UnknownSourceSetting {
        /// Affected selection.
        selection: ProfileSelection,
        /// Source id.
        source_id: &'static str,
        /// Missing setting id.
        setting: SettingIdV2,
    },
    /// A source references an absent or non-known fact.
    #[error("V2 source {source_id} references unavailable fact {fact:?} in {selection:?}")]
    UnknownSourceFact {
        /// Affected selection.
        selection: ProfileSelection,
        /// Source id.
        source_id: &'static str,
        /// Missing or non-known fact.
        fact: animsmith_core::engine_contract::EngineFactIdV2,
    },
    /// Accepted inputs have no primary authority.
    #[error("V2 accepted inputs have no source reference in {selection:?}")]
    UnreferencedAcceptedInputs {
        /// Affected selection.
        selection: ProfileSelection,
    },
    /// A known fact has no supporting primary authority.
    #[error("V2 fact {fact:?} has no source reference in {selection:?}")]
    UnreferencedFact {
        /// Affected selection.
        selection: ProfileSelection,
        /// Unsupported known fact.
        fact: animsmith_core::engine_contract::EngineFactIdV2,
    },
    /// A descriptor has no supporting primary authority.
    #[error("V2 setting {setting:?} has no source reference in {selection:?}")]
    UnreferencedSetting {
        /// Affected selection.
        selection: ProfileSelection,
        /// Unsupported descriptor.
        setting: SettingIdV2,
    },
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

/// Frozen Unity Generic revision-2 profile for root-motion routing.
///
/// The record deliberately contains only the importer controls required by
/// the first bounded root-motion slice. Revision 1 remains the historical V1
/// profile and is not amended by this sibling record.
fn unity_generic_v2() -> EngineProfileV2 {
    use SettingDefaultV2::RequiredExplicit;
    use SettingDomainV2::{
        AnimationType, AvatarSetup, BakeOrExtract, Boolean, SourceTransformPath,
    };
    use SettingIdV2::{
        AnimationType as AnimationTypeId, AvatarSetup as AvatarSetupId, ImportAnimation,
        RootMotionSource, RootPositionXz, RootPositionY, RootRotation,
    };
    use animsmith_core::engine_contract::{
        EngineFactIdV2, EngineFactStateV2, EngineFactValueV2, EngineProfileFactV2,
        EngineRootMotionAddressabilityV1,
    };

    let mut settings = vec![
        SettingDescriptorV2::new(
            AnimationTypeId,
            SettingScope::Document,
            AnimationType,
            RequiredExplicit,
        ),
        SettingDescriptorV2::new(
            AvatarSetupId,
            SettingScope::Document,
            AvatarSetup,
            RequiredExplicit,
        ),
        SettingDescriptorV2::new(
            ImportAnimation,
            SettingScope::Document,
            Boolean,
            RequiredExplicit,
        ),
        SettingDescriptorV2::new(
            RootMotionSource,
            SettingScope::Document,
            SourceTransformPath,
            RequiredExplicit,
        ),
        SettingDescriptorV2::new(
            RootRotation,
            SettingScope::Clip,
            BakeOrExtract,
            RequiredExplicit,
        ),
        SettingDescriptorV2::new(
            RootPositionY,
            SettingScope::Clip,
            BakeOrExtract,
            RequiredExplicit,
        ),
        SettingDescriptorV2::new(
            RootPositionXz,
            SettingScope::Clip,
            BakeOrExtract,
            RequiredExplicit,
        ),
    ];
    settings.sort_by_key(|descriptor| descriptor.id().as_str());

    let facts = vec![
        EngineProfileFactV2::new(
            EngineFactIdV2::AcceptedInputs,
            EngineFactStateV2::Known(EngineFactValueV2::AcceptedFormats(vec![
                SourceFormatV1::Fbx,
            ])),
        ),
        EngineProfileFactV2::new(
            EngineFactIdV2::ApplicationWorldUnitPolicy,
            EngineFactStateV2::Unknown,
        ),
        EngineProfileFactV2::new(
            EngineFactIdV2::ImportSettingProjection,
            EngineFactStateV2::Unknown,
        ),
        EngineProfileFactV2::new(
            EngineFactIdV2::ImporterScaleConversion,
            EngineFactStateV2::Unknown,
        ),
        EngineProfileFactV2::new(
            EngineFactIdV2::PhysicalDimensionsPreserved,
            EngineFactStateV2::Unknown,
        ),
        EngineProfileFactV2::new(
            EngineFactIdV2::ResultingTransformScale,
            EngineFactStateV2::Unknown,
        ),
        EngineProfileFactV2::new(
            EngineFactIdV2::RootMotionAddressability,
            EngineFactStateV2::Known(EngineFactValueV2::RootMotionAddressability(
                EngineRootMotionAddressabilityV1::ExactSourceTransformPath,
            )),
        ),
        EngineProfileFactV2::new(
            EngineFactIdV2::SourceImportDisposition,
            EngineFactStateV2::Unknown,
        ),
        EngineProfileFactV2::new(
            EngineFactIdV2::SourceToTargetUnitMapping,
            EngineFactStateV2::Unknown,
        ),
        EngineProfileFactV2::new(EngineFactIdV2::TargetLinearUnit, EngineFactStateV2::Unknown),
    ];

    let mut sources = vec![
        PrimarySourceV2 {
            id: "unity-fbx-model-importer-6000.3",
            target_version: "6000.3",
            url: "https://docs.unity3d.com/6000.3/Documentation/Manual/FBXImporter-Model.html",
            verified_on: UNITY_GENERIC_V2_VERIFIED_ON,
            supported_facts: vec![EngineFactIdV2::AcceptedInputs],
            supported_settings: vec![AnimationTypeId, AvatarSetupId, ImportAnimation],
        },
        PrimarySourceV2 {
            id: "unity-fbx-animation-clip-6000.3",
            target_version: "6000.3",
            url: "https://docs.unity3d.com/6000.3/Documentation/Manual/class-AnimationClip.html",
            verified_on: UNITY_GENERIC_V2_VERIFIED_ON,
            supported_facts: vec![],
            supported_settings: vec![RootRotation, RootPositionY, RootPositionXz],
        },
        PrimarySourceV2 {
            id: "unity-fbx-motion-node-6000.3",
            target_version: "6000.3",
            url: "https://docs.unity3d.com/6000.3/Documentation/ScriptReference/ModelImporter-motionNodeName.html",
            verified_on: UNITY_GENERIC_V2_VERIFIED_ON,
            supported_facts: vec![EngineFactIdV2::RootMotionAddressability],
            supported_settings: vec![RootMotionSource],
        },
    ];
    for source in &mut sources {
        source.supported_facts.sort_by_key(|fact| fact.as_str());
        source
            .supported_settings
            .sort_by_key(|setting| setting.as_str());
    }
    sources.sort_by_key(|source| source.id);

    EngineProfileV2 {
        selection: ProfileSelection::new("unity-generic", 2, "6000.3", "fbx-model-importer"),
        profile_urn: "urn:animsmith:engine-profile:unity-generic:2",
        accepted_inputs: vec![SourceFormatV1::Fbx],
        facts,
        settings,
        sources,
    }
}

/// Frozen Unreal revision-2 profile for document-level FBX sample-rate advice.
fn unreal_v2() -> EngineProfileV2 {
    use SettingDefaultV2::RequiredExplicit;
    use SettingDomainV2::SampleRate;
    use SettingIdV2::SampleRate as SampleRateId;
    use animsmith_core::engine_contract::EngineFactIdV2;

    let settings = vec![SettingDescriptorV2::new(
        SampleRateId,
        SettingScope::Document,
        SampleRate,
        RequiredExplicit,
    )];
    let sources = vec![PrimarySourceV2 {
        id: "unreal-fbx-import-options-5.8",
        target_version: "5.8",
        url: "https://dev.epicgames.com/documentation/en-us/unreal-engine/fbx-import-options-reference-in-unreal-engine?application_version=5.8",
        verified_on: IMPORT_ADVICE_V2_VERIFIED_ON,
        supported_facts: vec![
            EngineFactIdV2::AcceptedInputs,
            EngineFactIdV2::ImportSettingProjection,
        ],
        supported_settings: vec![SampleRateId],
    }];

    EngineProfileV2 {
        selection: ProfileSelection::new("unreal", 2, "5.8", "fbx-importer"),
        profile_urn: "urn:animsmith:engine-profile:unreal:2",
        accepted_inputs: vec![SourceFormatV1::Fbx],
        facts: document_advice_facts_v2(vec![SourceFormatV1::Fbx], "unreal_fbx_import_data"),
        settings,
        sources,
    }
}

/// Frozen Godot revision-2 profile for document-level scene-import advice.
fn godot_v2() -> EngineProfileV2 {
    use SettingDefaultV2::Verified;
    use SettingDomainV2::{Boolean, PositiveInteger};
    use SettingIdV2::{AnimationFps, AnimationTrimming};
    use animsmith_core::engine_contract::EngineFactIdV2;

    let mut settings = vec![
        SettingDescriptorV2::new(
            AnimationFps,
            SettingScope::Document,
            PositiveInteger,
            Verified(SettingValueV2::PositiveInteger(30)),
        ),
        SettingDescriptorV2::new(
            AnimationTrimming,
            SettingScope::Document,
            Boolean,
            Verified(SettingValueV2::Boolean(false)),
        ),
    ];
    settings.sort_by_key(|descriptor| descriptor.id().as_str());

    let sources = vec![PrimarySourceV2 {
        id: "godot-resource-importer-scene-4.7",
        target_version: "4.7",
        url: "https://docs.godotengine.org/en/4.7/classes/class_resourceimporterscene.html",
        verified_on: IMPORT_ADVICE_V2_VERIFIED_ON,
        supported_facts: vec![
            EngineFactIdV2::AcceptedInputs,
            EngineFactIdV2::ImportSettingProjection,
        ],
        supported_settings: vec![AnimationFps, AnimationTrimming],
    }];

    let accepted_inputs = vec![SourceFormatV1::Glb, SourceFormatV1::GltfJson];
    EngineProfileV2 {
        selection: ProfileSelection::new("godot", 2, "4.7", "resource-importer-scene"),
        profile_urn: "urn:animsmith:engine-profile:godot:2",
        facts: document_advice_facts_v2(accepted_inputs.clone(), "godot_params"),
        accepted_inputs,
        settings,
        sources,
    }
}

fn document_advice_facts_v2(
    accepted_inputs: Vec<SourceFormatV1>,
    import_setting_projection: &'static str,
) -> Vec<animsmith_core::engine_contract::EngineProfileFactV2> {
    use animsmith_core::engine_contract::{
        EngineFactIdV2, EngineFactStateV2, EngineFactValueV2, EngineProfileFactV2,
    };

    vec![
        EngineProfileFactV2::new(
            EngineFactIdV2::AcceptedInputs,
            EngineFactStateV2::Known(EngineFactValueV2::AcceptedFormats(accepted_inputs)),
        ),
        EngineProfileFactV2::new(
            EngineFactIdV2::ApplicationWorldUnitPolicy,
            EngineFactStateV2::Unknown,
        ),
        EngineProfileFactV2::new(
            EngineFactIdV2::ImportSettingProjection,
            EngineFactStateV2::Known(EngineFactValueV2::Token(import_setting_projection.into())),
        ),
        EngineProfileFactV2::new(
            EngineFactIdV2::ImporterScaleConversion,
            EngineFactStateV2::Unknown,
        ),
        EngineProfileFactV2::new(
            EngineFactIdV2::PhysicalDimensionsPreserved,
            EngineFactStateV2::Unknown,
        ),
        EngineProfileFactV2::new(
            EngineFactIdV2::ResultingTransformScale,
            EngineFactStateV2::Unknown,
        ),
        EngineProfileFactV2::new(
            EngineFactIdV2::RootMotionAddressability,
            EngineFactStateV2::Unknown,
        ),
        EngineProfileFactV2::new(
            EngineFactIdV2::SourceImportDisposition,
            EngineFactStateV2::Unknown,
        ),
        EngineProfileFactV2::new(
            EngineFactIdV2::SourceToTargetUnitMapping,
            EngineFactStateV2::Unknown,
        ),
        EngineProfileFactV2::new(EngineFactIdV2::TargetLinearUnit, EngineFactStateV2::Unknown),
    ]
}

fn bevy_v2() -> EngineProfileV2 {
    use SettingDefaultV2::{RequiredExplicit, Verified};
    use SettingDomainV2::{Boolean, HandlerEnvironment, LoadMeshesState};
    use SettingIdV2::{
        BevyAnimationFeature, ExtensionHandlerEnvironment, LoadAnimations, LoadMeshes,
        RotateMeshes, RotateSceneEntity,
    };
    use animsmith_core::engine_contract::{
        EngineFactIdV2, EngineFactStateV2, EngineFactValueV2, EngineLinearUnitV2,
        EngineProfileFactV2, ReducedRatioV1,
    };

    let mut settings = vec![
        SettingDescriptorV2::new(
            RotateSceneEntity,
            SettingScope::Document,
            Boolean,
            Verified(SettingValueV2::Boolean(false)),
        ),
        SettingDescriptorV2::new(
            RotateMeshes,
            SettingScope::Document,
            Boolean,
            Verified(SettingValueV2::Boolean(false)),
        ),
        SettingDescriptorV2::new(
            LoadMeshes,
            SettingScope::Document,
            LoadMeshesState,
            Verified(SettingValueV2::LoadMeshesState(
                crate::BevyLoadMeshesStateV2::Nonempty,
            )),
        ),
        SettingDescriptorV2::new(
            ExtensionHandlerEnvironment,
            SettingScope::Document,
            HandlerEnvironment,
            RequiredExplicit,
        ),
        // These two settings are frozen now so #483 can consume the same
        // profile/settings contract without another incompatible profile
        // revision. This ticket attaches no prediction behavior to them.
        SettingDescriptorV2::new(
            BevyAnimationFeature,
            SettingScope::Document,
            Boolean,
            RequiredExplicit,
        ),
        SettingDescriptorV2::new(
            LoadAnimations,
            SettingScope::Document,
            Boolean,
            Verified(SettingValueV2::Boolean(true)),
        ),
    ];
    settings.sort_by_key(|descriptor| descriptor.id().as_str());

    let facts = vec![
        EngineProfileFactV2::new(
            EngineFactIdV2::AcceptedInputs,
            EngineFactStateV2::Known(EngineFactValueV2::AcceptedFormats(vec![
                SourceFormatV1::Glb,
                SourceFormatV1::GltfJson,
            ])),
        ),
        EngineProfileFactV2::new(
            EngineFactIdV2::ApplicationWorldUnitPolicy,
            EngineFactStateV2::Known(EngineFactValueV2::Boolean(false)),
        ),
        EngineProfileFactV2::new(
            EngineFactIdV2::ImporterScaleConversion,
            EngineFactStateV2::Known(EngineFactValueV2::Token("none".into())),
        ),
        EngineProfileFactV2::new(
            EngineFactIdV2::ImportSettingProjection,
            EngineFactStateV2::Unknown,
        ),
        EngineProfileFactV2::new(
            EngineFactIdV2::PhysicalDimensionsPreserved,
            EngineFactStateV2::Known(EngineFactValueV2::Boolean(true)),
        ),
        EngineProfileFactV2::new(
            EngineFactIdV2::ResultingTransformScale,
            EngineFactStateV2::Known(EngineFactValueV2::Token(
                "loader_entities_unit_orthonormal_trs_nodes_passthrough_matrix_nodes_decomposed"
                    .into(),
            )),
        ),
        EngineProfileFactV2::new(
            EngineFactIdV2::RootMotionAddressability,
            EngineFactStateV2::Unknown,
        ),
        EngineProfileFactV2::new(
            EngineFactIdV2::SourceImportDisposition,
            EngineFactStateV2::Unknown,
        ),
        EngineProfileFactV2::new(
            EngineFactIdV2::SourceToTargetUnitMapping,
            EngineFactStateV2::Known(EngineFactValueV2::UnitRatio(
                ReducedRatioV1::new(1, 1).expect("one-to-one is a reduced positive ratio"),
            )),
        ),
        EngineProfileFactV2::new(
            EngineFactIdV2::TargetLinearUnit,
            EngineFactStateV2::Known(EngineFactValueV2::LinearUnit(
                EngineLinearUnitV2::EngineWorldLengthUnit,
            )),
        ),
    ];

    let pinned = |path: &'static str| -> &'static str {
        match path {
            "loader" => {
                "https://github.com/bevyengine/bevy/blob/c6f634ca9f406d68ba5109d921247b654cb42c10/crates/bevy_gltf/src/loader/mod.rs"
            }
            "coordinates" => {
                "https://github.com/bevyengine/bevy/blob/c6f634ca9f406d68ba5109d921247b654cb42c10/crates/bevy_gltf/src/convert_coordinates.rs"
            }
            "handlers" => {
                "https://github.com/bevyengine/bevy/blob/c6f634ca9f406d68ba5109d921247b654cb42c10/crates/bevy_gltf/src/loader/extensions/mod.rs"
            }
            "pbr" => {
                "https://github.com/bevyengine/bevy/blob/c6f634ca9f406d68ba5109d921247b654cb42c10/crates/bevy_pbr/src/gltf.rs"
            }
            "render_asset" => {
                "https://github.com/bevyengine/bevy/blob/c6f634ca9f406d68ba5109d921247b654cb42c10/crates/bevy_asset/src/render_asset.rs"
            }
            "cargo" => {
                "https://github.com/bevyengine/bevy/blob/c6f634ca9f406d68ba5109d921247b654cb42c10/Cargo.toml"
            }
            _ => unreachable!("closed Bevy source path"),
        }
    };
    let mut sources = vec![
        PrimarySourceV2 {
            id: "bevy-gltf-loader-0.19.0-c6f634ca",
            target_version: BEVY_0_19_COMMIT,
            url: pinned("loader"),
            verified_on: BEVY_V2_VERIFIED_ON,
            supported_facts: vec![
                EngineFactIdV2::AcceptedInputs,
                EngineFactIdV2::ApplicationWorldUnitPolicy,
                EngineFactIdV2::ImporterScaleConversion,
                EngineFactIdV2::PhysicalDimensionsPreserved,
                EngineFactIdV2::ResultingTransformScale,
                EngineFactIdV2::SourceToTargetUnitMapping,
                EngineFactIdV2::TargetLinearUnit,
            ],
            supported_settings: vec![LoadAnimations, LoadMeshes, RotateMeshes, RotateSceneEntity],
        },
        PrimarySourceV2 {
            id: "bevy-gltf-coordinate-conversion-0.19.0-c6f634ca",
            target_version: BEVY_0_19_COMMIT,
            url: pinned("coordinates"),
            verified_on: BEVY_V2_VERIFIED_ON,
            supported_facts: vec![
                EngineFactIdV2::ImporterScaleConversion,
                EngineFactIdV2::PhysicalDimensionsPreserved,
                EngineFactIdV2::ResultingTransformScale,
                EngineFactIdV2::SourceToTargetUnitMapping,
            ],
            supported_settings: vec![RotateMeshes, RotateSceneEntity],
        },
        PrimarySourceV2 {
            id: "bevy-gltf-extension-registry-0.19.0-c6f634ca",
            target_version: BEVY_0_19_COMMIT,
            url: pinned("handlers"),
            verified_on: BEVY_V2_VERIFIED_ON,
            supported_facts: vec![],
            supported_settings: vec![ExtensionHandlerEnvironment],
        },
        PrimarySourceV2 {
            id: "bevy-pbr-gltf-handler-0.19.0-c6f634ca",
            target_version: BEVY_0_19_COMMIT,
            url: pinned("pbr"),
            verified_on: BEVY_V2_VERIFIED_ON,
            supported_facts: vec![],
            supported_settings: vec![ExtensionHandlerEnvironment],
        },
        PrimarySourceV2 {
            id: "bevy-render-asset-usages-0.19.0-c6f634ca",
            target_version: BEVY_0_19_COMMIT,
            url: pinned("render_asset"),
            verified_on: BEVY_V2_VERIFIED_ON,
            supported_facts: vec![],
            supported_settings: vec![LoadMeshes],
        },
        PrimarySourceV2 {
            id: "bevy-feature-manifest-0.19.0-c6f634ca",
            target_version: BEVY_0_19_COMMIT,
            url: pinned("cargo"),
            verified_on: BEVY_V2_VERIFIED_ON,
            supported_facts: vec![],
            supported_settings: vec![BevyAnimationFeature],
        },
        PrimarySourceV2 {
            id: "khronos-gltf-2.0-coordinate-units",
            target_version: "2.0",
            url: "https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html#coordinate-system-and-units",
            verified_on: BEVY_V2_VERIFIED_ON,
            supported_facts: vec![
                EngineFactIdV2::AcceptedInputs,
                EngineFactIdV2::PhysicalDimensionsPreserved,
                EngineFactIdV2::SourceToTargetUnitMapping,
            ],
            supported_settings: vec![],
        },
    ];
    for source in &mut sources {
        source.supported_facts.sort_by_key(|fact| fact.as_str());
        source
            .supported_settings
            .sort_by_key(|setting| setting.as_str());
    }
    sources.sort_by_key(|source| source.id());

    EngineProfileV2 {
        selection: ProfileSelection::new("bevy", 2, "0.19.0", "gltf-asset-loader"),
        profile_urn: "urn:animsmith:engine-profile:bevy:2",
        accepted_inputs: vec![SourceFormatV1::Glb, SourceFormatV1::GltfJson],
        facts,
        settings,
        sources,
    }
}

/// The successor Bevy record proves only the two materialized animation-loading
/// gates.  Revision 2 remains byte-for-byte immutable: its aggregate source
/// disposition stays unknown, so it cannot accidentally gain this later rule.
fn bevy_v3() -> EngineProfileV2 {
    use animsmith_core::engine_contract::{EngineFactIdV2, EngineFactStateV2, EngineFactValueV2};

    let mut profile = bevy_v2();
    profile.selection = ProfileSelection::new("bevy", 3, "0.19.0", "gltf-asset-loader");
    profile.profile_urn = "urn:animsmith:engine-profile:bevy:3";
    let disposition = profile
        .facts
        .iter_mut()
        .find(|fact| fact.id() == EngineFactIdV2::SourceImportDisposition)
        .expect("every V2 profile has source import disposition");
    *disposition = animsmith_core::engine_contract::EngineProfileFactV2::new(
        EngineFactIdV2::SourceImportDisposition,
        EngineFactStateV2::Known(EngineFactValueV2::Token("materialized_import_gates".into())),
    );
    let loader = profile
        .sources
        .iter_mut()
        .find(|source| source.id == "bevy-gltf-loader-0.19.0-c6f634ca")
        .expect("Bevy V2 loader source exists");
    loader
        .supported_facts
        .push(EngineFactIdV2::SourceImportDisposition);
    loader.supported_facts.sort_by_key(|fact| fact.as_str());
    profile
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
