use animsmith_core::SourceFormatV1;
use animsmith_core::engine_contract::{
    EngineSettingIdV2 as CoreSettingIdV2, EngineSettingValueV2 as CoreSettingValueV2,
};
use animsmith_engine::{
    BakeOrExtract, EngineDeclarationV2, ProfileSelection, ResolutionError, ResolutionErrorV2,
    ResolvedSettingOriginV2, SettingDefaultV2, SettingIdV2, SettingLocation, SettingValueV2,
    UnityAnimationTypeV2, UnityAvatarSetupV2, lookup_profile, lookup_profile_v2, profiles_v1,
    profiles_v2, project_engine_profile_v2, project_resolved_engine_settings_v3, resolve_static_v2,
    validate_registry_v1, validate_registry_v2,
};
use std::collections::{BTreeMap, BTreeSet};

fn selection() -> ProfileSelection {
    ProfileSelection::new("unity-generic", 2, "6000.3", "fbx-model-importer")
}

fn declaration() -> EngineDeclarationV2 {
    EngineDeclarationV2 {
        selection: Some(selection()),
        document_settings: Some(BTreeMap::from([
            (
                "animation_type".into(),
                SettingValueV2::AnimationType(UnityAnimationTypeV2::Generic),
            ),
            (
                "avatar_setup".into(),
                SettingValueV2::AvatarSetup(UnityAvatarSetupV2::CreateFromThisModel),
            ),
            ("import_animation".into(), SettingValueV2::Boolean(true)),
            (
                "root_motion_source".into(),
                SettingValueV2::SourceTransformPath("Reference/Root".into()),
            ),
        ])),
        clip_settings: BTreeMap::from([(
            "locomotion_*".into(),
            BTreeMap::from([
                (
                    "root_rotation".into(),
                    SettingValueV2::BakeOrExtract(BakeOrExtract::Extract),
                ),
                (
                    "root_position_y".into(),
                    SettingValueV2::BakeOrExtract(BakeOrExtract::Bake),
                ),
                (
                    "root_position_xz".into(),
                    SettingValueV2::BakeOrExtract(BakeOrExtract::Extract),
                ),
            ]),
        )]),
    }
}

#[test]
fn exact_unity_generic_v2_sibling_preserves_v1_registry() {
    validate_registry_v1().unwrap();
    validate_registry_v2().unwrap();
    assert_eq!(profiles_v1().len(), 5);
    assert_eq!(profiles_v2().len(), 3);
    assert!(
        lookup_profile(&ProfileSelection::new(
            "unity-generic",
            1,
            "6000.3",
            "fbx-model-importer",
        ))
        .is_ok()
    );
    assert!(lookup_profile_v2(&selection()).is_ok());
    assert!(matches!(
        lookup_profile(&selection()),
        Err(ResolutionError::UnknownProfile(_))
    ));
}

#[test]
fn unity_generic_v2_freezes_controls_and_primary_sources() {
    let profile = lookup_profile_v2(&selection()).unwrap();
    assert_eq!(
        profile.profile_urn(),
        "urn:animsmith:engine-profile:unity-generic:2"
    );
    assert_eq!(profile.accepted_inputs(), &[SourceFormatV1::Fbx]);
    assert_eq!(
        profile
            .setting_descriptors()
            .iter()
            .map(|descriptor| descriptor.id())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            SettingIdV2::AnimationType,
            SettingIdV2::AvatarSetup,
            SettingIdV2::ImportAnimation,
            SettingIdV2::RootMotionSource,
            SettingIdV2::RootRotation,
            SettingIdV2::RootPositionY,
            SettingIdV2::RootPositionXz,
        ])
    );
    assert!(
        profile.setting_descriptors().iter().all(|descriptor| {
            matches!(descriptor.default(), SettingDefaultV2::RequiredExplicit)
        })
    );
    assert!(profile.sources().iter().any(|source| {
        source.url().contains("6000.3")
            && source
                .supported_settings()
                .contains(&SettingIdV2::RootMotionSource)
    }));
}

#[test]
fn unity_generic_v2_resolves_document_and_clip_settings() {
    let static_resolution = resolve_static_v2(declaration()).unwrap().unwrap();
    let document = static_resolution.document_settings();
    assert_eq!(document.len(), 4);
    assert_eq!(
        document[&SettingIdV2::AnimationType].value(),
        &SettingValueV2::AnimationType(UnityAnimationTypeV2::Generic)
    );
    assert_eq!(
        document[&SettingIdV2::AnimationType].origin(),
        ResolvedSettingOriginV2::ExplicitConfig
    );
    assert_eq!(static_resolution.clip_overlays().len(), 1);

    let resolved = static_resolution
        .resolve_input_with_clips(SourceFormatV1::Fbx, &["locomotion_run".into()])
        .unwrap();
    assert_eq!(resolved.clip_settings().len(), 1);
    assert_eq!(resolved.clip_settings()[0].clip_name(), "locomotion_run");
    assert_eq!(
        resolved.clip_settings()[0].settings()[&SettingIdV2::RootPositionXz].value(),
        &SettingValueV2::BakeOrExtract(BakeOrExtract::Extract)
    );
    assert_eq!(
        resolved.clip_settings()[0].settings()[&SettingIdV2::RootPositionXz].origin(),
        ResolvedSettingOriginV2::ExplicitConfig
    );
}

#[test]
fn unity_generic_v2_projection_uses_core_closed_values() {
    let profile = lookup_profile_v2(&selection()).unwrap();
    let core_profile = project_engine_profile_v2(profile).unwrap();
    let animation_type = core_profile
        .setting_descriptor(CoreSettingIdV2::AnimationType)
        .unwrap();
    assert_eq!(
        animation_type.domain(),
        animsmith_core::engine_contract::EngineSettingDomainV2::Token
    );

    let static_resolution = resolve_static_v2(declaration()).unwrap().unwrap();
    let resolved = static_resolution
        .resolve_input_with_clips(SourceFormatV1::Fbx, &["locomotion_run".into()])
        .unwrap();
    let (core_profile, core_settings) = project_resolved_engine_settings_v3(&resolved).unwrap();
    core_settings.validate_against(&core_profile).unwrap();
    assert_eq!(
        core_settings
            .document_setting(CoreSettingIdV2::AnimationType)
            .unwrap()
            .value(),
        &CoreSettingValueV2::Token("generic".into())
    );
    assert_eq!(core_settings.clips().len(), 1);
    assert_eq!(
        core_settings.clips()[0]
            .setting(CoreSettingIdV2::RootPositionY)
            .unwrap()
            .value(),
        &CoreSettingValueV2::BakeOrExtract(
            animsmith_core::engine_contract::EngineBakeOrExtractV1::Bake
        )
    );
}

#[test]
fn unity_generic_v2_rejects_wrong_scope_and_non_fbx() {
    let mut wrong_scope = declaration();
    wrong_scope
        .clip_settings
        .get_mut("locomotion_*")
        .unwrap()
        .insert(
            "root_motion_source".into(),
            SettingValueV2::SourceTransformPath("Reference/Root".into()),
        );
    assert!(matches!(
        resolve_static_v2(wrong_scope),
        Err(ResolutionErrorV2::WrongScope {
            setting: SettingIdV2::RootMotionSource,
            location: SettingLocation::ClipSelector(_),
            ..
        })
    ));

    let static_resolution = resolve_static_v2(declaration()).unwrap().unwrap();
    assert!(matches!(
        static_resolution.resolve_input_with_clips(SourceFormatV1::Glb, &[]),
        Err(ResolutionErrorV2::UnacceptedInputFormat { .. })
    ));
}

#[test]
fn unity_generic_v2_rejects_non_frozen_import_modes() {
    for (key, value) in [
        (
            "animation_type",
            SettingValueV2::AnimationType(UnityAnimationTypeV2::Humanoid),
        ),
        (
            "avatar_setup",
            SettingValueV2::AvatarSetup(UnityAvatarSetupV2::CopyFromOtherAvatar),
        ),
        ("import_animation", SettingValueV2::Boolean(false)),
    ] {
        let mut input = declaration();
        input
            .document_settings
            .as_mut()
            .unwrap()
            .insert(key.into(), value);
        assert!(matches!(
            resolve_static_v2(input),
            Err(ResolutionErrorV2::InvalidSettingValue { .. })
        ));
    }
}

#[test]
fn unity_generic_v2_preserves_duplicate_clip_ordinals_through_sorting() {
    let static_resolution = resolve_static_v2(declaration()).unwrap().unwrap();
    let resolved = static_resolution
        .resolve_input_with_clips(
            SourceFormatV1::Fbx,
            &["locomotion_run".into(), "locomotion_run".into()],
        )
        .unwrap();
    assert_eq!(resolved.clip_settings().len(), 2);
    assert_eq!(resolved.clip_settings()[0].clip_name(), "locomotion_run");
    assert_eq!(resolved.clip_settings()[0].clip_ordinal(), 0);
    assert_eq!(resolved.clip_settings()[1].clip_name(), "locomotion_run");
    assert_eq!(resolved.clip_settings()[1].clip_ordinal(), 1);
    assert_eq!(
        resolved
            .clip_setting(0, "locomotion_run")
            .unwrap()
            .clip_ordinal(),
        0
    );
    assert_eq!(
        resolved
            .clip_setting(1, "locomotion_run")
            .unwrap()
            .clip_ordinal(),
        1
    );
}
