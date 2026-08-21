use animsmith_core::{ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS, EngineContractError, SourceFormatV1};
use animsmith_engine::{
    BakeOrExtract, EngineDeclaration, InvalidSettingReason, ProfileSelection, ResolutionError,
    SettingDomain, SettingId, SettingLocation, SettingScope, SettingValue, SettingValueKind,
    resolve_static,
};
use std::cell::Cell;
use std::collections::BTreeMap;

fn selection(family: &str, version: &str, importer: &str) -> ProfileSelection {
    ProfileSelection::new(family, 1, version, importer)
}

fn unity(generic: bool) -> EngineDeclaration {
    let mut document = BTreeMap::from([
        ("convert_units".into(), SettingValue::Boolean(true)),
        ("bake_axis_conversion".into(), SettingValue::Boolean(true)),
    ]);
    if generic {
        document.insert(
            "root_motion_source".into(),
            SettingValue::SourceTransformPath("Reference/Root".into()),
        );
    }
    EngineDeclaration {
        selection: Some(selection(
            if generic {
                "unity-generic"
            } else {
                "unity-humanoid"
            },
            "6000.3",
            "fbx-model-importer",
        )),
        document_settings: Some(document),
        clip_settings: BTreeMap::new(),
    }
}

fn all_clip_values(
    rotation: BakeOrExtract,
    y: BakeOrExtract,
    xz: BakeOrExtract,
) -> BTreeMap<String, SettingValue> {
    BTreeMap::from([
        (
            "root_rotation".into(),
            SettingValue::BakeOrExtract(rotation),
        ),
        ("root_position_y".into(), SettingValue::BakeOrExtract(y)),
        ("root_position_xz".into(), SettingValue::BakeOrExtract(xz)),
    ])
}

#[test]
fn no_selection_and_no_settings_preserves_engine_neutral_behavior() {
    assert!(
        resolve_static(EngineDeclaration::default())
            .unwrap()
            .is_none()
    );
}

#[test]
fn every_settings_declaration_without_selection_is_rejected() {
    let empty_document = EngineDeclaration {
        document_settings: Some(BTreeMap::new()),
        ..EngineDeclaration::default()
    };
    assert_eq!(
        resolve_static(empty_document),
        Err(ResolutionError::SettingsWithoutSelection)
    );
    let empty_clip = EngineDeclaration {
        clip_settings: BTreeMap::from([("never_matches".into(), BTreeMap::new())]),
        ..EngineDeclaration::default()
    };
    assert_eq!(
        resolve_static(empty_clip),
        Err(ResolutionError::SettingsWithoutSelection)
    );
}

#[test]
fn static_phase_validates_unmatched_selectors_before_input_resolution() {
    let mut input = unity(true);
    input.clip_settings.insert(
        "never_matches".into(),
        BTreeMap::from([("not_a_setting".into(), SettingValue::Boolean(true))]),
    );
    assert!(matches!(
        resolve_static(input),
        Err(ResolutionError::UnknownSetting { key, .. }) if key == "not_a_setting"
    ));

    let mut valid_unmatched = unity(true);
    valid_unmatched.clip_settings.insert(
        "never_matches".into(),
        all_clip_values(
            BakeOrExtract::Bake,
            BakeOrExtract::Bake,
            BakeOrExtract::Bake,
        ),
    );
    let static_profile = resolve_static(valid_unmatched).unwrap().unwrap();
    assert!(
        static_profile
            .resolve_input(SourceFormatV1::Fbx, &[])
            .is_ok()
    );
}

#[test]
fn unknown_and_not_applicable_settings_are_distinct() {
    let mut unknown = EngineDeclaration {
        selection: Some(selection("unreal", "5.8", "fbx-importer")),
        document_settings: Some(BTreeMap::from([(
            "convert_units".into(),
            SettingValue::Boolean(true),
        )])),
        ..EngineDeclaration::default()
    };
    assert!(matches!(
        resolve_static(unknown.clone()),
        Err(ResolutionError::UnknownSetting { key, .. }) if key == "convert_units"
    ));
    unknown.document_settings = None;
    unknown.clip_settings.insert(
        "*".into(),
        BTreeMap::from([("convert_units".into(), SettingValue::Boolean(true))]),
    );
    assert!(matches!(
        resolve_static(unknown),
        Err(ResolutionError::UnknownSetting { .. })
    ));

    let mut not_applicable = unity(false);
    not_applicable.document_settings.as_mut().unwrap().insert(
        "root_motion_source".into(),
        SettingValue::SourceTransformPath("Root".into()),
    );
    assert!(matches!(
        resolve_static(not_applicable),
        Err(ResolutionError::NotApplicable {
            setting: SettingId::RootMotionSource,
            ..
        })
    ));
}

#[test]
fn known_settings_are_rejected_in_both_wrong_scope_directions() {
    let mut document_in_clip = unity(true);
    document_in_clip.clip_settings.insert(
        "*".into(),
        BTreeMap::from([("convert_units".into(), SettingValue::Boolean(true))]),
    );
    assert_eq!(
        resolve_static(document_in_clip),
        Err(ResolutionError::WrongScope {
            setting: SettingId::ConvertUnits,
            expected: SettingScope::Document,
            found: SettingScope::Clip,
            location: SettingLocation::ClipSelector("*".into()),
        })
    );

    let mut clip_in_document = unity(true);
    clip_in_document.document_settings.as_mut().unwrap().insert(
        "root_rotation".into(),
        SettingValue::BakeOrExtract(BakeOrExtract::Bake),
    );
    assert_eq!(
        resolve_static(clip_in_document),
        Err(ResolutionError::WrongScope {
            setting: SettingId::RootRotation,
            expected: SettingScope::Clip,
            found: SettingScope::Document,
            location: SettingLocation::Document,
        })
    );
}

#[test]
fn every_setting_descriptor_rejects_a_value_from_another_domain() {
    let cases = [
        (
            "convert_units",
            SettingId::ConvertUnits,
            SettingScope::Document,
            SettingDomain::Boolean,
            SettingValue::BakeOrExtract(BakeOrExtract::Bake),
            SettingValueKind::BakeOrExtract,
        ),
        (
            "bake_axis_conversion",
            SettingId::BakeAxisConversion,
            SettingScope::Document,
            SettingDomain::Boolean,
            SettingValue::SourceTransformPath("Root".into()),
            SettingValueKind::SourceTransformPath,
        ),
        (
            "root_motion_source",
            SettingId::RootMotionSource,
            SettingScope::Document,
            SettingDomain::SourceTransformPath,
            SettingValue::Boolean(true),
            SettingValueKind::Boolean,
        ),
        (
            "root_rotation",
            SettingId::RootRotation,
            SettingScope::Clip,
            SettingDomain::BakeOrExtract,
            SettingValue::Boolean(true),
            SettingValueKind::Boolean,
        ),
        (
            "root_position_y",
            SettingId::RootPositionY,
            SettingScope::Clip,
            SettingDomain::BakeOrExtract,
            SettingValue::SourceTransformPath("Root".into()),
            SettingValueKind::SourceTransformPath,
        ),
        (
            "root_position_xz",
            SettingId::RootPositionXz,
            SettingScope::Clip,
            SettingDomain::BakeOrExtract,
            SettingValue::Boolean(false),
            SettingValueKind::Boolean,
        ),
    ];
    for (key, setting, scope, expected, value, found) in cases {
        let location = match scope {
            SettingScope::Document => SettingLocation::Document,
            SettingScope::Clip => SettingLocation::ClipSelector("*".into()),
        };
        let mut input = unity(true);
        match scope {
            SettingScope::Document => {
                input
                    .document_settings
                    .as_mut()
                    .unwrap()
                    .insert(key.into(), value);
            }
            SettingScope::Clip => {
                input
                    .clip_settings
                    .insert("*".into(), BTreeMap::from([(key.into(), value)]));
            }
        }
        assert_eq!(
            resolve_static(input),
            Err(ResolutionError::InvalidSettingValue {
                setting,
                location,
                reason: InvalidSettingReason::WrongDomain { expected, found },
            }),
            "{key}"
        );
    }
}

#[test]
fn source_transform_path_is_relative_segmented_control_free_and_byte_bounded() {
    let invalid = [
        ("", InvalidSettingReason::EmptyPath),
        ("/Root", InvalidSettingReason::AbsolutePath),
        ("Root//Child", InvalidSettingReason::EmptyPathSegment),
        ("Root/./Child", InvalidSettingReason::DotPathSegment),
        ("Root/../Child", InvalidSettingReason::DotPathSegment),
    ];
    for (path, expected) in invalid {
        let mut input = unity(true);
        input.document_settings.as_mut().unwrap().insert(
            "root_motion_source".into(),
            SettingValue::SourceTransformPath(path.into()),
        );
        assert!(matches!(
            resolve_static(input),
            Err(ResolutionError::InvalidSettingValue { reason, .. }) if reason == expected
        ));
    }

    for control in (0..=0x1f).chain(0x7f..=0x9f) {
        let path = format!("Root{}Child", char::from_u32(control).unwrap());
        let mut input = unity(true);
        input.document_settings.as_mut().unwrap().insert(
            "root_motion_source".into(),
            SettingValue::SourceTransformPath(path),
        );
        assert!(matches!(
            resolve_static(input),
            Err(ResolutionError::InvalidSettingValue {
                reason: InvalidSettingReason::ControlCharacter,
                ..
            })
        ));
    }

    let mut at_limit = unity(true);
    at_limit.document_settings.as_mut().unwrap().insert(
        "root_motion_source".into(),
        SettingValue::SourceTransformPath("é".repeat(2048)),
    );
    assert!(resolve_static(at_limit).is_ok());
    let mut over_limit = unity(true);
    over_limit.document_settings.as_mut().unwrap().insert(
        "root_motion_source".into(),
        SettingValue::SourceTransformPath("é".repeat(2049)),
    );
    assert!(matches!(
        resolve_static(over_limit),
        Err(ResolutionError::InvalidSettingValue {
            reason: InvalidSettingReason::PathTooLong {
                bytes: 4098,
                limit: 4096
            },
            ..
        })
    ));
}

#[test]
fn every_required_document_and_clip_setting_is_independently_required() {
    let document_settings = [
        ("convert_units", SettingId::ConvertUnits),
        ("bake_axis_conversion", SettingId::BakeAxisConversion),
        ("root_motion_source", SettingId::RootMotionSource),
    ];
    let clip_settings = [
        ("root_rotation", SettingId::RootRotation),
        ("root_position_y", SettingId::RootPositionY),
        ("root_position_xz", SettingId::RootPositionXz),
    ];

    for generic in [true, false] {
        for (key, setting) in document_settings {
            if !generic && setting == SettingId::RootMotionSource {
                continue;
            }
            let mut input = unity(generic);
            input.document_settings.as_mut().unwrap().remove(key);
            assert_eq!(
                resolve_static(input),
                Err(ResolutionError::MissingRequiredSetting {
                    setting,
                    location: SettingLocation::Document,
                }),
                "generic={generic} {key}"
            );
        }

        for (key, setting) in clip_settings {
            let mut input = unity(generic);
            let mut settings = all_clip_values(
                BakeOrExtract::Bake,
                BakeOrExtract::Bake,
                BakeOrExtract::Bake,
            );
            settings.remove(key);
            input.clip_settings.insert("*".into(), settings);
            let result = resolve_static(input)
                .unwrap()
                .unwrap()
                .resolve_input(SourceFormatV1::Fbx, &["walk".into()]);
            assert_eq!(
                result,
                Err(ResolutionError::MissingRequiredSetting {
                    setting,
                    location: SettingLocation::ClipSelector("walk".into()),
                }),
                "generic={generic} {key}"
            );
        }
    }
}

#[test]
fn fully_materialized_document_and_all_clip_rows_reflect_overlay_precedence() {
    let mut input = unity(true);
    input.clip_settings.insert(
        "*".into(),
        all_clip_values(
            BakeOrExtract::Bake,
            BakeOrExtract::Bake,
            BakeOrExtract::Bake,
        ),
    );
    input.clip_settings.insert(
        "walk*".into(),
        BTreeMap::from([(
            "root_rotation".into(),
            SettingValue::BakeOrExtract(BakeOrExtract::Extract),
        )]),
    );
    input.clip_settings.insert(
        "walk_*".into(),
        BTreeMap::from([(
            "root_position_xz".into(),
            SettingValue::BakeOrExtract(BakeOrExtract::Extract),
        )]),
    );
    input.clip_settings.insert(
        "walk_forward".into(),
        BTreeMap::from([(
            "root_position_y".into(),
            SettingValue::BakeOrExtract(BakeOrExtract::Extract),
        )]),
    );
    let resolved = resolve_static(input)
        .unwrap()
        .unwrap()
        .resolve_input(SourceFormatV1::Fbx, &["walk_forward".into(), "idle".into()])
        .unwrap();
    assert_eq!(
        resolved.document_settings(),
        &BTreeMap::from([
            (SettingId::ConvertUnits, SettingValue::Boolean(true)),
            (SettingId::BakeAxisConversion, SettingValue::Boolean(true)),
            (
                SettingId::RootMotionSource,
                SettingValue::SourceTransformPath("Reference/Root".into())
            ),
        ])
    );
    let expected_idle = BTreeMap::from([
        (
            SettingId::RootRotation,
            SettingValue::BakeOrExtract(BakeOrExtract::Bake),
        ),
        (
            SettingId::RootPositionY,
            SettingValue::BakeOrExtract(BakeOrExtract::Bake),
        ),
        (
            SettingId::RootPositionXz,
            SettingValue::BakeOrExtract(BakeOrExtract::Bake),
        ),
    ]);
    let expected_walk = BTreeMap::from([
        (
            SettingId::RootRotation,
            SettingValue::BakeOrExtract(BakeOrExtract::Extract),
        ),
        (
            SettingId::RootPositionY,
            SettingValue::BakeOrExtract(BakeOrExtract::Extract),
        ),
        (
            SettingId::RootPositionXz,
            SettingValue::BakeOrExtract(BakeOrExtract::Extract),
        ),
    ]);
    assert_eq!(resolved.clip_settings().len(), 2);
    assert_eq!(resolved.clip_settings()[0].clip_name(), "idle");
    assert_eq!(resolved.clip_settings()[0].settings(), &expected_idle);
    assert_eq!(resolved.clip_settings()[1].clip_name(), "walk_forward");
    assert_eq!(resolved.clip_settings()[1].settings(), &expected_walk);
}

#[test]
fn duplicate_actual_clip_names_remain_distinct_sorted_rows_and_change_identity() {
    let make_static = || {
        let mut input = unity(true);
        input.clip_settings.insert(
            "*".into(),
            all_clip_values(
                BakeOrExtract::Extract,
                BakeOrExtract::Bake,
                BakeOrExtract::Extract,
            ),
        );
        resolve_static(input).unwrap().unwrap()
    };
    let duplicated = make_static()
        .resolve_input(
            SourceFormatV1::Fbx,
            &["walk".into(), "idle".into(), "walk".into()],
        )
        .unwrap();
    let deduplicated = make_static()
        .resolve_input(SourceFormatV1::Fbx, &["idle".into(), "walk".into()])
        .unwrap();

    assert_eq!(
        duplicated
            .clip_settings()
            .iter()
            .map(|clip| clip.clip_name())
            .collect::<Vec<_>>(),
        vec!["idle", "walk", "walk"]
    );
    assert_eq!(
        duplicated.clip_settings()[1].settings(),
        duplicated.clip_settings()[2].settings()
    );
    assert_ne!(
        duplicated.settings_identity(),
        deduplicated.settings_identity()
    );
}

#[test]
fn every_profile_rejects_each_unaccepted_input_without_fallback() {
    let declarations = [
        unity(true),
        unity(false),
        EngineDeclaration {
            selection: Some(selection("unreal", "5.8", "fbx-importer")),
            ..EngineDeclaration::default()
        },
        EngineDeclaration {
            selection: Some(selection("godot", "4.7", "resource-importer-scene")),
            ..EngineDeclaration::default()
        },
        EngineDeclaration {
            selection: Some(selection("bevy", "0.19.0", "gltf-asset-loader")),
            ..EngineDeclaration::default()
        },
    ];
    for declaration in declarations {
        let static_profile = resolve_static(declaration).unwrap().unwrap();
        for format in [
            SourceFormatV1::GltfJson,
            SourceFormatV1::Glb,
            SourceFormatV1::Fbx,
        ] {
            let result = static_profile.resolve_input(format, &[]);
            assert_eq!(
                result.is_ok(),
                static_profile.profile().accepted_inputs().contains(&format),
                "{:?} {format:?}",
                static_profile.profile().selection()
            );
        }
    }
}

#[test]
fn settings_digest_depends_only_on_fully_materialized_semantics() {
    let values = all_clip_values(
        BakeOrExtract::Extract,
        BakeOrExtract::Bake,
        BakeOrExtract::Extract,
    );
    let mut broad = unity(true);
    broad.clip_settings.insert("walk*".into(), values.clone());
    let mut exact = unity(true);
    exact
        .clip_settings
        .insert("walk_forward".into(), values.clone());
    let clip_names = vec!["walk_forward".into()];
    let broad = resolve_static(broad)
        .unwrap()
        .unwrap()
        .resolve_input(SourceFormatV1::Fbx, &clip_names)
        .unwrap();
    let exact = resolve_static(exact)
        .unwrap()
        .unwrap()
        .resolve_input(SourceFormatV1::Fbx, &clip_names)
        .unwrap();
    assert_eq!(broad.settings_identity(), exact.settings_identity());

    let mut shadowed = unity(true);
    shadowed.clip_settings.insert(
        "*".into(),
        all_clip_values(
            BakeOrExtract::Bake,
            BakeOrExtract::Extract,
            BakeOrExtract::Bake,
        ),
    );
    shadowed
        .clip_settings
        .insert("walk_forward".into(), values.clone());
    let shadowed = resolve_static(shadowed)
        .unwrap()
        .unwrap()
        .resolve_input(SourceFormatV1::Fbx, &clip_names)
        .unwrap();
    assert_eq!(exact.settings_identity(), shadowed.settings_identity());

    let document_mutations = [
        ("convert_units", SettingValue::Boolean(false)),
        ("bake_axis_conversion", SettingValue::Boolean(false)),
        (
            "root_motion_source",
            SettingValue::SourceTransformPath("Reference/ChangedRoot".into()),
        ),
    ];
    for (key, value) in document_mutations {
        let mut changed = unity(true);
        changed
            .document_settings
            .as_mut()
            .unwrap()
            .insert(key.into(), value);
        changed
            .clip_settings
            .insert("walk_forward".into(), values.clone());
        let changed = resolve_static(changed)
            .unwrap()
            .unwrap()
            .resolve_input(SourceFormatV1::Fbx, &clip_names)
            .unwrap();
        assert_ne!(
            exact.settings_identity(),
            changed.settings_identity(),
            "document setting {key}"
        );
    }

    let clip_mutations = [
        (
            "root_rotation",
            SettingValue::BakeOrExtract(BakeOrExtract::Bake),
        ),
        (
            "root_position_y",
            SettingValue::BakeOrExtract(BakeOrExtract::Extract),
        ),
        (
            "root_position_xz",
            SettingValue::BakeOrExtract(BakeOrExtract::Bake),
        ),
    ];
    for (key, value) in clip_mutations {
        let mut changed_values = values.clone();
        changed_values.insert(key.into(), value);
        let mut changed = unity(true);
        changed
            .clip_settings
            .insert("walk_forward".into(), changed_values);
        let changed = resolve_static(changed)
            .unwrap()
            .unwrap()
            .resolve_input(SourceFormatV1::Fbx, &clip_names)
            .unwrap();
        assert_ne!(
            exact.settings_identity(),
            changed.settings_identity(),
            "clip setting {key}"
        );
    }

    let renamed = resolve_static({
        let mut input = unity(true);
        input.clip_settings.insert(
            "walk*".into(),
            all_clip_values(
                BakeOrExtract::Extract,
                BakeOrExtract::Bake,
                BakeOrExtract::Extract,
            ),
        );
        input
    })
    .unwrap()
    .unwrap()
    .resolve_input(SourceFormatV1::Fbx, &["walk_left".into()])
    .unwrap();
    assert_ne!(exact.settings_identity(), renamed.settings_identity());

    let ordered_names = vec!["walk_left".into(), "walk_forward".into()];
    let reversed_names = vec!["walk_forward".into(), "walk_left".into()];
    let make_ordered = || {
        let mut input = unity(true);
        input.clip_settings.insert(
            "walk*".into(),
            all_clip_values(
                BakeOrExtract::Extract,
                BakeOrExtract::Bake,
                BakeOrExtract::Extract,
            ),
        );
        resolve_static(input).unwrap().unwrap()
    };
    let ordered = make_ordered()
        .resolve_input(SourceFormatV1::Fbx, &ordered_names)
        .unwrap();
    let reversed = make_ordered()
        .resolve_input(SourceFormatV1::Fbx, &reversed_names)
        .unwrap();
    assert_eq!(ordered.settings_identity(), reversed.settings_identity());

    let godot = resolve_static(EngineDeclaration {
        selection: Some(selection("godot", "4.7", "resource-importer-scene")),
        ..EngineDeclaration::default()
    })
    .unwrap()
    .unwrap();
    let gltf = godot.resolve_input(SourceFormatV1::GltfJson, &[]).unwrap();
    let glb = godot.resolve_input(SourceFormatV1::Glb, &[]).unwrap();
    assert_eq!(gltf.settings_identity(), glb.settings_identity());

    assert_eq!(
        exact.settings_identity().sha256(),
        "2784140a014437f9932f254f2088bca24e65abc6b0282c848be9086b7840a0a7"
    );
    assert_eq!(exact.settings_identity().bytes(), 642);
}

#[test]
fn settings_identity_golden_values_cover_every_frozen_profile() {
    let clip_values = all_clip_values(
        BakeOrExtract::Extract,
        BakeOrExtract::Bake,
        BakeOrExtract::Extract,
    );
    let mut generic = unity(true);
    generic
        .clip_settings
        .insert("walk_forward".into(), clip_values.clone());
    let mut humanoid = unity(false);
    humanoid
        .clip_settings
        .insert("walk_forward".into(), clip_values);
    let declarations = [
        generic,
        humanoid,
        EngineDeclaration {
            selection: Some(selection("unreal", "5.8", "fbx-importer")),
            ..EngineDeclaration::default()
        },
        EngineDeclaration {
            selection: Some(selection("godot", "4.7", "resource-importer-scene")),
            ..EngineDeclaration::default()
        },
        EngineDeclaration {
            selection: Some(selection("bevy", "0.19.0", "gltf-asset-loader")),
            ..EngineDeclaration::default()
        },
    ];
    let actual = declarations
        .into_iter()
        .map(|declaration| {
            let static_profile = resolve_static(declaration).unwrap().unwrap();
            let family = static_profile.profile().selection().family();
            let clip_names = if family.starts_with("unity-") {
                vec!["walk_forward".into()]
            } else {
                vec![]
            };
            let resolved = static_profile
                .resolve_input(static_profile.profile().accepted_inputs()[0], &clip_names)
                .unwrap();
            (
                family.to_owned(),
                resolved.settings_identity().sha256().to_owned(),
                resolved.settings_identity().bytes(),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        actual,
        vec![
            (
                "unity-generic".into(),
                "2784140a014437f9932f254f2088bca24e65abc6b0282c848be9086b7840a0a7".into(),
                642,
            ),
            (
                "unity-humanoid".into(),
                "e3302a2b5b34858963224f27cfa0b5b702f84b64b1b91581ef78aaf2aabdf9e9".into(),
                567,
            ),
            (
                "unreal".into(),
                "0329761fc6bdbc9b1f16ba5ca51bb882c8fc2d779686cabd570ffe080b3c4826".into(),
                231,
            ),
            (
                "godot".into(),
                "02032c315fa41ad65249efe1b6914456b3b98caf9b5374b168854cd357f85515".into(),
                240,
            ),
            (
                "bevy".into(),
                "c8d075b3abbbac652c6b432b3dee68e49b2453f908dc55fbd3392df97930a7fc".into(),
                235,
            ),
        ]
    );
}

#[test]
fn actual_clip_inventory_over_v1_settings_bound_returns_a_typed_error_without_panicking() {
    let bevy = resolve_static(EngineDeclaration {
        selection: Some(selection("bevy", "0.19.0", "gltf-asset-loader")),
        ..EngineDeclaration::default()
    })
    .unwrap()
    .unwrap();
    let clip_names = vec!["clip".into(); ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS + 1];

    assert_eq!(
        bevy.resolve_input(SourceFormatV1::GltfJson, &clip_names),
        Err(ResolutionError::ResolvedSettingsContract(
            EngineContractError::TooManyRows {
                field: "settings.clips",
                found: ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS + 1,
                max: ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
            }
        ))
    );
}

#[test]
fn oversized_exact_iterator_is_rejected_before_consuming_or_cloning_a_name() {
    struct Oversized<'a> {
        next_calls: &'a Cell<usize>,
    }

    impl Iterator for Oversized<'_> {
        type Item = &'static str;

        fn next(&mut self) -> Option<Self::Item> {
            self.next_calls.set(self.next_calls.get() + 1);
            Some("must-not-be-consumed")
        }

        fn size_hint(&self) -> (usize, Option<usize>) {
            let length = ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS + 1;
            (length, Some(length))
        }
    }

    impl ExactSizeIterator for Oversized<'_> {
        fn len(&self) -> usize {
            ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS + 1
        }
    }

    let bevy = resolve_static(EngineDeclaration {
        selection: Some(selection("bevy", "0.19.0", "gltf-asset-loader")),
        ..EngineDeclaration::default()
    })
    .unwrap()
    .unwrap();
    let next_calls = Cell::new(0);

    assert_eq!(
        bevy.resolve_input_iter(
            SourceFormatV1::GltfJson,
            Oversized {
                next_calls: &next_calls,
            },
        ),
        Err(ResolutionError::ResolvedSettingsContract(
            EngineContractError::TooManyRows {
                field: "settings.clips",
                found: ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS + 1,
                max: ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
            }
        ))
    );
    assert_eq!(next_calls.get(), 0);
}
