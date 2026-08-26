use animsmith_core::SourceFormatV1;
use animsmith_core::engine_contract::{
    EngineSampleRateV2, EngineSettingDomainV2 as CoreSettingDomainV2,
    EngineSettingIdV2 as CoreSettingIdV2, EngineSettingValueOriginV3,
    EngineSettingValueV2 as CoreSettingValueV2,
};
use animsmith_engine::{
    EngineDeclarationV2, ProfileSelection, ResolutionError, ResolutionErrorV2,
    ResolvedSettingOriginV2, SettingDefaultV2, SettingDomainV2, SettingIdV2, SettingValueV2,
    lookup_profile, lookup_profile_v2, profiles_v1, profiles_v2, project_engine_profile_v2,
    project_resolved_engine_settings_v3, resolve_static_v2, validate_registry_v1,
    validate_registry_v2,
};
use std::collections::BTreeMap;

fn godot_selection() -> ProfileSelection {
    ProfileSelection::new("godot", 2, "4.7", "resource-importer-scene")
}

fn unreal_selection() -> ProfileSelection {
    ProfileSelection::new("unreal", 2, "5.8", "fbx-importer")
}

#[test]
fn exact_godot_and_unreal_v2_profiles_preserve_the_v1_registry() {
    validate_registry_v1().unwrap();
    validate_registry_v2().unwrap();
    assert_eq!(profiles_v1().len(), 5);
    assert_eq!(profiles_v2().len(), 5);

    for selection in [godot_selection(), unreal_selection()] {
        assert!(lookup_profile_v2(&selection).is_ok());
        assert_eq!(
            lookup_profile(&selection),
            Err(ResolutionError::UnknownProfile(selection))
        );
    }
}

#[test]
fn godot_v2_freezes_glb_and_gltf_defaults_with_origins() {
    let profile = lookup_profile_v2(&godot_selection()).unwrap();
    assert_eq!(
        profile.profile_urn(),
        "urn:animsmith:engine-profile:godot:2"
    );
    assert_eq!(
        profile.accepted_inputs(),
        &[SourceFormatV1::Glb, SourceFormatV1::GltfJson]
    );
    assert_eq!(
        profile
            .setting_descriptor(SettingIdV2::AnimationFps)
            .unwrap()
            .default(),
        &SettingDefaultV2::Verified(SettingValueV2::PositiveInteger(30))
    );
    assert_eq!(
        profile
            .setting_descriptor(SettingIdV2::AnimationTrimming)
            .unwrap()
            .default(),
        &SettingDefaultV2::Verified(SettingValueV2::Boolean(false))
    );
    assert!(profile.sources().iter().any(|source| {
        source.target_version() == "4.7"
            && source
                .supported_settings()
                .contains(&SettingIdV2::AnimationFps)
            && source
                .supported_settings()
                .contains(&SettingIdV2::AnimationTrimming)
    }));
    let core_profile = project_engine_profile_v2(profile).unwrap();
    assert_eq!(
        core_profile.facts_identity().sha256(),
        "3d2c21f0652c0d62e65db4044a6413b7d3ac6283c3b06328e2253b58f3e11cca"
    );
    assert_eq!(core_profile.facts_identity().bytes(), 1_518);

    let static_resolution = resolve_static_v2(EngineDeclarationV2 {
        selection: Some(godot_selection()),
        document_settings: Some(BTreeMap::new()),
        ..EngineDeclarationV2::default()
    })
    .unwrap()
    .unwrap();
    for format in [SourceFormatV1::Glb, SourceFormatV1::GltfJson] {
        let resolved = static_resolution.resolve_input(format).unwrap();
        assert_eq!(
            resolved.document_settings()[&SettingIdV2::AnimationFps].origin(),
            ResolvedSettingOriginV2::ProfileDefault
        );
        assert_eq!(
            resolved.document_settings()[&SettingIdV2::AnimationTrimming].origin(),
            ResolvedSettingOriginV2::ProfileDefault
        );
        let (core_profile, core_settings) = project_resolved_engine_settings_v3(&resolved).unwrap();
        core_settings.validate_against(&core_profile).unwrap();
        assert_eq!(
            core_settings
                .document_setting(CoreSettingIdV2::AnimationFps)
                .unwrap()
                .value(),
            &CoreSettingValueV2::PositiveInteger(30)
        );
        assert_eq!(
            core_settings
                .document_setting(CoreSettingIdV2::AnimationFps)
                .unwrap()
                .value_origin(),
            EngineSettingValueOriginV3::ProfileDefault
        );
    }
    assert!(matches!(
        static_resolution.resolve_input(SourceFormatV1::Fbx),
        Err(ResolutionErrorV2::UnacceptedInputFormat { .. })
    ));

    let explicit = resolve_static_v2(EngineDeclarationV2 {
        selection: Some(godot_selection()),
        document_settings: Some(BTreeMap::from([
            ("animation_fps".into(), SettingValueV2::PositiveInteger(60)),
            ("animation_trimming".into(), SettingValueV2::Boolean(true)),
        ])),
        ..EngineDeclarationV2::default()
    })
    .unwrap()
    .unwrap();
    assert!(
        explicit
            .document_settings()
            .values()
            .all(|row| { row.origin() == ResolvedSettingOriginV2::ExplicitConfig })
    );
    for invalid_fps in [0, 121] {
        let result = resolve_static_v2(EngineDeclarationV2 {
            selection: Some(godot_selection()),
            document_settings: Some(BTreeMap::from([(
                "animation_fps".into(),
                SettingValueV2::PositiveInteger(invalid_fps),
            )])),
            ..EngineDeclarationV2::default()
        });
        assert!(matches!(
            result,
            Err(ResolutionErrorV2::InvalidSettingValue {
                setting: SettingIdV2::AnimationFps,
                ..
            })
        ));
    }
}

#[test]
fn unreal_v2_requires_explicit_sample_rate_and_projects_its_closed_domain() {
    let profile = lookup_profile_v2(&unreal_selection()).unwrap();
    assert_eq!(
        profile.profile_urn(),
        "urn:animsmith:engine-profile:unreal:2"
    );
    assert_eq!(profile.accepted_inputs(), &[SourceFormatV1::Fbx]);
    let descriptor = profile.setting_descriptor(SettingIdV2::SampleRate).unwrap();
    assert_eq!(descriptor.domain(), SettingDomainV2::SampleRate);
    assert_eq!(descriptor.default(), &SettingDefaultV2::RequiredExplicit);
    assert!(profile.sources().iter().any(|source| {
        source.target_version() == "5.8"
            && source
                .supported_settings()
                .contains(&SettingIdV2::SampleRate)
    }));
    let core = project_engine_profile_v2(profile).unwrap();
    assert_eq!(
        core.facts_identity().sha256(),
        "213c267c62be511fe4ca589433f0d2facb8630fc57ffe7885869c403ebe26af4"
    );
    assert_eq!(core.facts_identity().bytes(), 1_330);
    assert_eq!(
        core.setting_descriptor(CoreSettingIdV2::SampleRate)
            .unwrap()
            .domain(),
        CoreSettingDomainV2::SampleRate
    );
    assert!(matches!(
        resolve_static_v2(EngineDeclarationV2 {
            selection: Some(unreal_selection()),
            document_settings: Some(BTreeMap::new()),
            ..EngineDeclarationV2::default()
        }),
        Err(ResolutionErrorV2::MissingRequiredSetting {
            setting: SettingIdV2::SampleRate,
            ..
        })
    ));
    let _core_vocabulary = [
        EngineSampleRateV2::Default30,
        EngineSampleRateV2::SourceDetermined,
        EngineSampleRateV2::CustomHz(1),
        EngineSampleRateV2::CustomHz(48_000),
    ];
}
