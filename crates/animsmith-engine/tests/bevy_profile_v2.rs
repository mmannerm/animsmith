use animsmith_core::SourceFormatV1;
use animsmith_core::engine_contract::{
    EngineFactIdV2, EngineFactStateV2, EngineFactValueV2, EngineLinearUnitV2,
    EngineSettingIdV2 as CoreSettingIdV2, EngineSettingValueOriginV3,
    EngineSettingValueV2 as CoreSettingValueV2, ReducedRatioV1,
};
use animsmith_engine::{
    BevyGltfHandlerEnvironmentV2, BevyLoadMeshesStateV2, EngineDeclarationV2, ProfileSelection,
    ResolutionError, ResolutionErrorV2, ResolvedSettingOriginV2, SettingDefaultV2, SettingIdV2,
    SettingLocation, SettingValueV2, lookup_profile, lookup_profile_v2, profiles_v1, profiles_v2,
    project_engine_profile_v2, project_resolved_engine_settings_v3, resolve_static_v2,
    validate_registry_v1, validate_registry_v2,
};
use std::collections::{BTreeMap, BTreeSet};

const BEVY_COMMIT: &str = "c6f634ca9f406d68ba5109d921247b654cb42c10";

fn selection(revision: u32) -> ProfileSelection {
    ProfileSelection::new("bevy", revision, "0.19.0", "gltf-asset-loader")
}

fn required_settings() -> BTreeMap<String, SettingValueV2> {
    BTreeMap::from([
        (
            "bevy_animation_feature".into(),
            SettingValueV2::Boolean(true),
        ),
        (
            "extension_handler_environment".into(),
            SettingValueV2::HandlerEnvironment(BevyGltfHandlerEnvironmentV2::BareEmpty),
        ),
    ])
}

#[test]
fn v2_contract_registry_retains_revision_2_and_adds_exact_revision_3_sibling() {
    validate_registry_v1().unwrap();
    validate_registry_v2().unwrap();
    assert_eq!(profiles_v1().len(), 5);
    assert_eq!(profiles_v2().len(), 2);

    let profile = lookup_profile_v2(&selection(2)).unwrap();
    assert_eq!(profile.profile_urn(), "urn:animsmith:engine-profile:bevy:2");
    assert_eq!(
        profile.accepted_inputs(),
        &[SourceFormatV1::Glb, SourceFormatV1::GltfJson]
    );
    assert_eq!(
        lookup_profile(&selection(2)),
        Err(ResolutionError::UnknownProfile(selection(2)))
    );
    assert_eq!(
        lookup_profile_v2(&selection(1)),
        Err(ResolutionErrorV2::UnknownProfile(selection(1)))
    );
    assert!(lookup_profile(&selection(1)).is_ok());

    let successor = lookup_profile_v2(&selection(3)).unwrap();
    assert_eq!(
        successor.profile_urn(),
        "urn:animsmith:engine-profile:bevy:3"
    );
    let projected = project_engine_profile_v2(successor).unwrap();
    assert_eq!(
        projected.facts_identity().sha256(),
        "d532b00621bf06a2db2dedf896c19aae2c07b3b1873a1b05beade2252d7a89c5"
    );
    assert_eq!(projected.facts_identity().bytes(), 4_849);
}

#[test]
fn v2_registry_freezes_setting_defaults_and_pinned_primary_authority() {
    let profile = lookup_profile_v2(&selection(2)).unwrap();
    let actual_ids = profile
        .setting_descriptors()
        .iter()
        .map(|descriptor| descriptor.id())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        actual_ids,
        BTreeSet::from([
            SettingIdV2::RotateSceneEntity,
            SettingIdV2::RotateMeshes,
            SettingIdV2::LoadMeshes,
            SettingIdV2::ExtensionHandlerEnvironment,
            SettingIdV2::BevyAnimationFeature,
            SettingIdV2::LoadAnimations,
        ])
    );
    assert_eq!(
        profile
            .setting_descriptor(SettingIdV2::RotateSceneEntity)
            .unwrap()
            .default(),
        &SettingDefaultV2::Verified(SettingValueV2::Boolean(false))
    );
    assert_eq!(
        profile
            .setting_descriptor(SettingIdV2::RotateMeshes)
            .unwrap()
            .default(),
        &SettingDefaultV2::Verified(SettingValueV2::Boolean(false))
    );
    assert_eq!(
        profile
            .setting_descriptor(SettingIdV2::LoadMeshes)
            .unwrap()
            .default(),
        &SettingDefaultV2::Verified(SettingValueV2::LoadMeshesState(
            BevyLoadMeshesStateV2::Nonempty
        ))
    );
    assert_eq!(
        profile
            .setting_descriptor(SettingIdV2::ExtensionHandlerEnvironment)
            .unwrap()
            .default(),
        &SettingDefaultV2::RequiredExplicit
    );
    assert_eq!(
        profile
            .setting_descriptor(SettingIdV2::BevyAnimationFeature)
            .unwrap()
            .default(),
        &SettingDefaultV2::RequiredExplicit
    );
    assert_eq!(
        profile
            .setting_descriptor(SettingIdV2::LoadAnimations)
            .unwrap()
            .default(),
        &SettingDefaultV2::Verified(SettingValueV2::Boolean(true))
    );

    assert!(profile.sources().iter().any(|source| {
        source.url().contains(BEVY_COMMIT)
            && source.target_version() == BEVY_COMMIT
            && source
                .supported_settings()
                .contains(&SettingIdV2::RotateSceneEntity)
    }));
    assert!(profile.sources().iter().any(|source| {
        source
            .url()
            .starts_with("https://registry.khronos.org/glTF/")
            && source.supports_accepted_inputs()
    }));
    assert!(profile.sources().iter().all(|source| {
        source.verified_on() == "2026-08-25"
            && (source.url().contains(BEVY_COMMIT)
                || source
                    .url()
                    .starts_with("https://registry.khronos.org/glTF/"))
    }));
}

#[test]
fn v2_profile_projects_exact_unit_mapping_without_a_metre_world_policy_claim() {
    let core = project_engine_profile_v2(lookup_profile_v2(&selection(2)).unwrap()).unwrap();
    assert_eq!(
        core.fact(EngineFactIdV2::TargetLinearUnit).unwrap().state(),
        &EngineFactStateV2::Known(EngineFactValueV2::LinearUnit(
            EngineLinearUnitV2::EngineWorldLengthUnit
        ))
    );
    assert_eq!(
        core.fact(EngineFactIdV2::SourceToTargetUnitMapping)
            .unwrap()
            .state(),
        &EngineFactStateV2::Known(EngineFactValueV2::UnitRatio(
            ReducedRatioV1::new(1, 1).unwrap()
        ))
    );
    assert_eq!(
        core.fact(EngineFactIdV2::ImporterScaleConversion)
            .unwrap()
            .state(),
        &EngineFactStateV2::Known(EngineFactValueV2::Token("none".into()))
    );
    assert_eq!(
        core.fact(EngineFactIdV2::ApplicationWorldUnitPolicy)
            .unwrap()
            .state(),
        &EngineFactStateV2::Known(EngineFactValueV2::Boolean(false))
    );
}

#[test]
fn verified_defaults_materialize_and_explicit_values_retain_origin() {
    let resolved = resolve_static_v2(EngineDeclarationV2 {
        selection: Some(selection(2)),
        document_settings: Some(required_settings()),
        ..EngineDeclarationV2::default()
    })
    .unwrap()
    .unwrap();
    let values = resolved.document_settings();
    assert_eq!(values.len(), 6);
    assert_eq!(
        values[&SettingIdV2::RotateSceneEntity].value(),
        &SettingValueV2::Boolean(false)
    );
    assert_eq!(
        values[&SettingIdV2::RotateSceneEntity].origin(),
        ResolvedSettingOriginV2::ProfileDefault
    );
    assert_eq!(
        values[&SettingIdV2::LoadMeshes].value(),
        &SettingValueV2::LoadMeshesState(BevyLoadMeshesStateV2::Nonempty)
    );
    assert_eq!(
        values[&SettingIdV2::ExtensionHandlerEnvironment].origin(),
        ResolvedSettingOriginV2::ExplicitConfig
    );
    assert_eq!(
        values[&SettingIdV2::BevyAnimationFeature].origin(),
        ResolvedSettingOriginV2::ExplicitConfig
    );

    let input_resolved = resolved.resolve_input(SourceFormatV1::Glb).unwrap();
    let (core_profile, core_settings) =
        project_resolved_engine_settings_v3(&input_resolved).unwrap();
    core_settings.validate_against(&core_profile).unwrap();
    let rotate_scene = core_settings
        .document_setting(CoreSettingIdV2::RotateSceneEntity)
        .unwrap();
    assert_eq!(rotate_scene.value(), &CoreSettingValueV2::Boolean(false));
    assert_eq!(
        rotate_scene.value_origin(),
        EngineSettingValueOriginV3::ProfileDefault
    );
    let handler = core_settings
        .document_setting(CoreSettingIdV2::ExtensionHandlerEnvironment)
        .unwrap();
    assert_eq!(
        handler.value(),
        &CoreSettingValueV2::Token("bare_empty".into())
    );
    assert_eq!(
        handler.value_origin(),
        EngineSettingValueOriginV3::ExplicitConfig
    );

    let mut explicit = required_settings();
    explicit.insert("rotate_scene_entity".into(), SettingValueV2::Boolean(true));
    explicit.insert(
        "load_meshes".into(),
        SettingValueV2::LoadMeshesState(BevyLoadMeshesStateV2::Empty),
    );
    let overridden = resolve_static_v2(EngineDeclarationV2 {
        selection: Some(selection(2)),
        document_settings: Some(explicit),
        ..EngineDeclarationV2::default()
    })
    .unwrap()
    .unwrap();
    assert_eq!(
        overridden.document_settings()[&SettingIdV2::RotateSceneEntity].origin(),
        ResolvedSettingOriginV2::ExplicitConfig
    );
    assert_eq!(
        overridden.document_settings()[&SettingIdV2::LoadMeshes].value(),
        &SettingValueV2::LoadMeshesState(BevyLoadMeshesStateV2::Empty)
    );
}

#[test]
fn required_environment_and_feature_fail_closed() {
    assert_eq!(
        resolve_static_v2(EngineDeclarationV2 {
            selection: Some(selection(2)),
            document_settings: Some(BTreeMap::new()),
            ..EngineDeclarationV2::default()
        }),
        Err(ResolutionErrorV2::MissingRequiredSetting {
            setting: SettingIdV2::BevyAnimationFeature,
            location: SettingLocation::Document,
        })
    );
    assert_eq!(
        resolve_static_v2(EngineDeclarationV2 {
            selection: Some(selection(2)),
            document_settings: Some(BTreeMap::from([(
                "bevy_animation_feature".into(),
                SettingValueV2::Boolean(true),
            )])),
            ..EngineDeclarationV2::default()
        }),
        Err(ResolutionErrorV2::MissingRequiredSetting {
            setting: SettingIdV2::ExtensionHandlerEnvironment,
            location: SettingLocation::Document,
        })
    );
    assert_eq!(
        BevyGltfHandlerEnvironmentV2::BareEmpty.as_str(),
        "bare_empty"
    );
    assert_eq!(
        BevyGltfHandlerEnvironmentV2::BevyPbrStock019.as_str(),
        "bevy_pbr_stock_0_19"
    );
}

#[test]
fn v2_resolution_rejects_wrong_domain_scope_and_source_format() {
    let mut wrong_domain = required_settings();
    wrong_domain.insert(
        "rotate_meshes".into(),
        SettingValueV2::HandlerEnvironment(BevyGltfHandlerEnvironmentV2::BareEmpty),
    );
    assert!(matches!(
        resolve_static_v2(EngineDeclarationV2 {
            selection: Some(selection(2)),
            document_settings: Some(wrong_domain),
            ..EngineDeclarationV2::default()
        }),
        Err(ResolutionErrorV2::InvalidSettingValue {
            setting: SettingIdV2::RotateMeshes,
            ..
        })
    ));

    assert!(matches!(
        resolve_static_v2(EngineDeclarationV2 {
            selection: Some(selection(2)),
            document_settings: Some(required_settings()),
            clip_settings: BTreeMap::from([(
                "*".into(),
                BTreeMap::from([("rotate_meshes".into(), SettingValueV2::Boolean(false),)]),
            )]),
        }),
        Err(ResolutionErrorV2::WrongScope {
            setting: SettingIdV2::RotateMeshes,
            ..
        })
    ));

    let resolved = resolve_static_v2(EngineDeclarationV2 {
        selection: Some(selection(2)),
        document_settings: Some(required_settings()),
        ..EngineDeclarationV2::default()
    })
    .unwrap()
    .unwrap();
    assert!(resolved.resolve_input(SourceFormatV1::Glb).is_ok());
    assert!(resolved.resolve_input(SourceFormatV1::GltfJson).is_ok());
    assert_eq!(
        resolved.resolve_input(SourceFormatV1::Fbx),
        Err(ResolutionErrorV2::UnacceptedInputFormat {
            selection: selection(2),
            format: SourceFormatV1::Fbx,
        })
    );
}
