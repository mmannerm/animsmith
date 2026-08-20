use animsmith_core::SourceFormatV1;
use animsmith_engine::{
    CoordinateBasis, DefaultStatus, FactId, FactState, FactValue, ForwardAxis, Handedness,
    ProfileSelection, SettingApplicability, SettingDomain, SettingId, SettingScope, UpAxis,
    lookup_profile, profiles_v1, validate_registry_v1,
};
use std::collections::BTreeSet;

fn selection(family: &str, revision: u32, version: &str, importer: &str) -> ProfileSelection {
    ProfileSelection::new(family, revision, version, importer)
}

#[test]
fn registry_is_exactly_the_five_frozen_singletons() {
    validate_registry_v1().unwrap();
    let actual: BTreeSet<_> = profiles_v1()
        .iter()
        .map(|profile| {
            (
                profile.selection().clone(),
                profile.fact_bundle_urn(),
                profile.accepted_inputs().to_vec(),
            )
        })
        .collect();
    let expected = BTreeSet::from([
        (
            selection("unity-generic", 1, "6000.3", "fbx-model-importer"),
            "urn:animsmith:engine-profile:unity-generic:1",
            vec![SourceFormatV1::Fbx],
        ),
        (
            selection("unity-humanoid", 1, "6000.3", "fbx-model-importer"),
            "urn:animsmith:engine-profile:unity-humanoid:1",
            vec![SourceFormatV1::Fbx],
        ),
        (
            selection("unreal", 1, "5.8", "fbx-importer"),
            "urn:animsmith:engine-profile:unreal:1",
            vec![SourceFormatV1::Fbx],
        ),
        (
            selection("godot", 1, "4.7", "resource-importer-scene"),
            "urn:animsmith:engine-profile:godot:1",
            vec![
                SourceFormatV1::Fbx,
                SourceFormatV1::Glb,
                SourceFormatV1::GltfJson,
            ],
        ),
        (
            selection("bevy", 1, "0.19.0", "gltf-asset-loader"),
            "urn:animsmith:engine-profile:bevy:1",
            vec![SourceFormatV1::Glb, SourceFormatV1::GltfJson],
        ),
    ]);
    assert_eq!(actual, expected);
}

#[test]
fn every_tuple_field_is_exact_and_cross_combinations_do_not_resolve() {
    let exact = BTreeSet::from([
        selection("unity-generic", 1, "6000.3", "fbx-model-importer"),
        selection("unity-humanoid", 1, "6000.3", "fbx-model-importer"),
        selection("unreal", 1, "5.8", "fbx-importer"),
        selection("godot", 1, "4.7", "resource-importer-scene"),
        selection("bevy", 1, "0.19.0", "gltf-asset-loader"),
    ]);

    let families = ["unity-generic", "unity-humanoid", "unreal", "godot", "bevy"];
    let revisions = [1];
    let versions = ["6000.3", "5.8", "4.7", "0.19.0"];
    let importers = [
        "fbx-model-importer",
        "fbx-importer",
        "resource-importer-scene",
        "gltf-asset-loader",
    ];
    for family in families {
        for revision in revisions {
            for version in versions {
                for importer in importers {
                    let candidate = selection(family, revision, version, importer);
                    if exact.contains(&candidate) {
                        assert_eq!(
                            lookup_profile(&candidate).unwrap().selection(),
                            &candidate,
                            "known-vocabulary exact candidate {candidate:?}"
                        );
                    } else {
                        assert_eq!(
                            lookup_profile(&candidate).unwrap_err(),
                            animsmith_engine::ResolutionError::UnknownProfile(candidate.clone()),
                            "known-vocabulary cross-product candidate {candidate:?}"
                        );
                    }
                }
            }
        }
    }

    for valid in exact {
        let unknown_mutations = [
            selection(
                "unknown-family",
                valid.profile_revision(),
                valid.engine_version(),
                valid.importer(),
            ),
            selection(valid.family(), 2, valid.engine_version(), valid.importer()),
            selection(
                valid.family(),
                valid.profile_revision(),
                "unknown-version",
                valid.importer(),
            ),
            selection(
                valid.family(),
                valid.profile_revision(),
                valid.engine_version(),
                "unknown-importer",
            ),
        ];
        for candidate in unknown_mutations {
            assert_eq!(
                lookup_profile(&candidate).unwrap_err(),
                animsmith_engine::ResolutionError::UnknownProfile(candidate.clone()),
                "unknown-vocabulary mutation {candidate:?}"
            );
        }
    }
}

#[test]
fn facts_are_complete_explicit_and_source_bound() {
    let expected_ids = BTreeSet::from([
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
    ]);
    let mut identities = BTreeSet::new();
    for profile in profiles_v1() {
        assert_eq!(
            profile
                .facts()
                .iter()
                .map(|fact| fact.id())
                .collect::<BTreeSet<_>>(),
            expected_ids
        );
        assert_eq!(
            profile.fact(FactId::AcceptedInputs).unwrap().state(),
            &FactState::Known(FactValue::AcceptedFormats(
                profile.accepted_inputs().to_vec()
            ))
        );
        for id in [
            FactId::ExactAxisConversion,
            FactId::ResultingHierarchyScale,
            FactId::AnimationChannelHandling,
            FactId::ExtensionHandling,
            FactId::ConstructHandling,
        ] {
            assert_eq!(profile.fact(id).unwrap().state(), &FactState::Unknown);
        }
        assert!(profile.sources().iter().all(|source| {
            source.url().starts_with("https://")
                && source.verified_on() == "2026-08-20"
                && !source.target_version().is_empty()
        }));
        assert!(identities.insert((
            profile.facts_identity().sha256().to_owned(),
            profile.facts_identity().bytes()
        )));
    }
    let unreal = profiles_v1()
        .iter()
        .find(|profile| profile.selection().family() == "unreal")
        .unwrap();
    assert_eq!(
        unreal.fact(FactId::TargetCoordinateBasis).unwrap().state(),
        &FactState::Known(FactValue::CoordinateBasis(CoordinateBasis {
            handedness: Handedness::Left,
            up_axis: UpAxis::Z,
            forward_axis: ForwardAxis::PositiveX,
        }))
    );
}

#[test]
fn setting_descriptors_pin_domains_scopes_defaults_and_applicability() {
    let generic = lookup_profile(&selection(
        "unity-generic",
        1,
        "6000.3",
        "fbx-model-importer",
    ))
    .unwrap();
    let humanoid = lookup_profile(&selection(
        "unity-humanoid",
        1,
        "6000.3",
        "fbx-model-importer",
    ))
    .unwrap();
    assert_eq!(generic.setting_descriptors().len(), 6);
    assert_eq!(humanoid.setting_descriptors().len(), 6);
    for profile in [generic, humanoid] {
        for descriptor in profile.setting_descriptors() {
            let (scope, domain) = match descriptor.id() {
                SettingId::ConvertUnits | SettingId::BakeAxisConversion => {
                    (SettingScope::Document, SettingDomain::Boolean)
                }
                SettingId::RootMotionSource => {
                    (SettingScope::Document, SettingDomain::SourceTransformPath)
                }
                SettingId::RootRotation | SettingId::RootPositionY | SettingId::RootPositionXz => {
                    (SettingScope::Clip, SettingDomain::BakeOrExtract)
                }
            };
            assert_eq!(descriptor.scope(), scope);
            assert_eq!(descriptor.domain(), domain);
        }
    }
    let generic_root = generic
        .setting_descriptor(SettingId::RootMotionSource)
        .unwrap();
    assert_eq!(
        generic_root.applicability(),
        SettingApplicability::Applicable
    );
    assert_eq!(
        generic_root.default_status(),
        DefaultStatus::RequiredWithoutDefault
    );
    let humanoid_root = humanoid
        .setting_descriptor(SettingId::RootMotionSource)
        .unwrap();
    assert_eq!(
        humanoid_root.applicability(),
        SettingApplicability::NotApplicable
    );
    assert_eq!(humanoid_root.default_status(), DefaultStatus::NotApplicable);
    for descriptor in humanoid
        .setting_descriptors()
        .iter()
        .filter(|descriptor| descriptor.id() != SettingId::RootMotionSource)
        .chain(generic.setting_descriptors())
    {
        assert_eq!(
            descriptor.default_status(),
            DefaultStatus::RequiredWithoutDefault
        );
    }
    for family in ["unreal", "godot", "bevy"] {
        let profile = profiles_v1()
            .iter()
            .find(|profile| profile.selection().family() == family)
            .unwrap();
        assert!(profile.setting_descriptors().is_empty());
    }
}

#[test]
fn facts_identity_golden_values() {
    let actual: Vec<_> = profiles_v1()
        .iter()
        .map(|profile| {
            (
                profile.selection().family(),
                profile.facts_identity().sha256(),
                profile.facts_identity().bytes(),
            )
        })
        .collect();
    assert_eq!(
        actual,
        vec![
            (
                "bevy",
                "873b98e896f05de73d5ea30560a4555c1f93650beeec9ed929e30dbcf7ce8c1e",
                1642,
            ),
            (
                "godot",
                "e9c8316d1655c487b60dd35bbfc70289952c5fa12f4718f0be09c7e9a00fbe87",
                1166,
            ),
            (
                "unity-generic",
                "97afc05a02f7f9a946c66945cb84669a8a67d4dae7bf642486b94f1de3a17dd4",
                3097,
            ),
            (
                "unity-humanoid",
                "43f53df9f26ca3a1248972566029609bcd6b63194cbca399789444622680a12a",
                2847,
            ),
            (
                "unreal",
                "e44ca461aee46312b8265446f08338b988b96abeab0f8f502f560da5f1cdf759",
                2169,
            ),
        ]
    );
}
