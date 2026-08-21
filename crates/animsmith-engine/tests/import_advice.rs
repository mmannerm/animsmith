use animsmith_core::{
    Clip, ClipExpectations, Config, Document, InputIdentity, LoadedSource, MovementOwner,
    RawSourceFactsBuilderV1, SourceClipFactV1, SourceFactDomainV1, SourceFactSetV1, SourceFormatV1,
    SourceLoaderDispositionV1, SourceObservationV1, SourceProvenanceV1, SourceTextV1,
    SourceUnavailableReasonV1, ToolInfo, ToolSource,
};
use animsmith_engine::{
    BakeOrExtract, EngineDeclaration, EngineImportAdviceError, EngineImportAdviceInput,
    EngineImportAdvicePayloadV1, EngineImportAdviceRefusalReasonV1, EngineImportAdviceStateV1,
    EngineImportAdviceV1, ProfileSelection, SettingMap, SettingValue, profiles_v1, resolve_static,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn tool() -> ToolInfo {
    ToolInfo::animsmith(ToolSource::new(
        Some("0123456789abcdef0123456789abcdef01234567".into()),
        Some(false),
    ))
}

fn source(coverage_complete: bool, names: &[(&str, &str)]) -> LoadedSource {
    let primary = InputIdentity::from_bytes(b"engine import advice analytic source");
    let mut facts = RawSourceFactsBuilderV1::new(SourceFormatV1::Fbx, primary);
    let mut document = Document::default();
    for (index, (source_name, normalized_name)) in names.iter().enumerate() {
        assert!(facts.push_clip(SourceClipFactV1::new(
            index,
            SourceObservationV1::observed(
                SourceTextV1::new(*source_name).unwrap(),
                SourceProvenanceV1::format_defined(),
                SourceLoaderDispositionV1::Preserved,
            ),
            SourceObservationV1::observed(
                index,
                SourceProvenanceV1::format_defined(),
                SourceLoaderDispositionV1::Preserved,
            ),
            SourceObservationV1::proven_absent(SourceProvenanceV1::format_defined()),
            SourceObservationV1::proven_absent(SourceProvenanceV1::format_defined()),
            SourceFactSetV1::complete(Vec::new()),
        )));
        document.clips.push(Clip {
            name: (*normalized_name).into(),
            duration_s: 1.0 + index as f64,
            tracks: Vec::new(),
        });
    }
    if coverage_complete {
        facts.mark_complete(SourceFactDomainV1::Clips);
    } else {
        facts.mark_partial(
            SourceFactDomainV1::Clips,
            animsmith_core::SourceUnavailableReasonV1::ProjectionBudgetExceeded,
        );
    }
    facts.mark_complete(SourceFactDomainV1::Constructs);
    facts.mark_complete(SourceFactDomainV1::Resources);
    facts.finish(document).unwrap()
}

fn source_with_unavailable_normalized_index() -> LoadedSource {
    let primary = InputIdentity::from_bytes(b"unavailable normalized clip identity");
    let mut facts = RawSourceFactsBuilderV1::new(SourceFormatV1::Fbx, primary);
    assert!(facts.push_clip(SourceClipFactV1::new(
        0,
        SourceObservationV1::observed(
            SourceTextV1::new("Take").unwrap(),
            SourceProvenanceV1::format_defined(),
            SourceLoaderDispositionV1::Preserved,
        ),
        SourceObservationV1::unavailable(
            SourceUnavailableReasonV1::ParserUnavailable,
            None,
            SourceLoaderDispositionV1::Unknown,
        ),
        SourceObservationV1::proven_absent(SourceProvenanceV1::format_defined()),
        SourceObservationV1::proven_absent(SourceProvenanceV1::format_defined()),
        SourceFactSetV1::complete(Vec::new()),
    )));
    facts.mark_complete(SourceFactDomainV1::Clips);
    facts.mark_complete(SourceFactDomainV1::Constructs);
    facts.mark_complete(SourceFactDomainV1::Resources);
    facts
        .finish(Document {
            clips: vec![Clip {
                name: "walk".into(),
                duration_s: 1.0,
                tracks: Vec::new(),
            }],
            ..Document::default()
        })
        .unwrap()
}

fn source_with_swapped_normalized_indices() -> LoadedSource {
    let primary = InputIdentity::from_bytes(b"swapped source to normalized clip mapping");
    let mut facts = RawSourceFactsBuilderV1::new(SourceFormatV1::Fbx, primary);
    for (source_index, normalized_index, source_name) in [(0, 1, "Take Run"), (1, 0, "Take Walk")] {
        assert!(facts.push_clip(SourceClipFactV1::new(
            source_index,
            SourceObservationV1::observed(
                SourceTextV1::new(source_name).unwrap(),
                SourceProvenanceV1::format_defined(),
                SourceLoaderDispositionV1::Preserved,
            ),
            SourceObservationV1::observed(
                normalized_index,
                SourceProvenanceV1::format_defined(),
                SourceLoaderDispositionV1::Preserved,
            ),
            SourceObservationV1::proven_absent(SourceProvenanceV1::format_defined()),
            SourceObservationV1::proven_absent(SourceProvenanceV1::format_defined()),
            SourceFactSetV1::complete(Vec::new()),
        )));
    }
    facts.mark_complete(SourceFactDomainV1::Clips);
    facts.mark_complete(SourceFactDomainV1::Constructs);
    facts.mark_complete(SourceFactDomainV1::Resources);
    facts
        .finish(Document {
            clips: vec![
                Clip {
                    name: "walk".into(),
                    duration_s: 1.0,
                    tracks: Vec::new(),
                },
                Clip {
                    name: "run".into(),
                    duration_s: 2.0,
                    tracks: Vec::new(),
                },
            ],
            ..Document::default()
        })
        .unwrap()
}

fn unity_declaration(generic: bool) -> EngineDeclaration {
    let mut document_settings = SettingMap::from([
        ("convert_units".into(), SettingValue::Boolean(true)),
        ("bake_axis_conversion".into(), SettingValue::Boolean(false)),
    ]);
    if generic {
        document_settings.insert(
            "root_motion_source".into(),
            SettingValue::SourceTransformPath("Reference/Root".into()),
        );
    }
    let clip_settings = SettingMap::from([
        (
            "root_rotation".into(),
            SettingValue::BakeOrExtract(BakeOrExtract::Extract),
        ),
        (
            "root_position_y".into(),
            SettingValue::BakeOrExtract(BakeOrExtract::Bake),
        ),
        (
            "root_position_xz".into(),
            SettingValue::BakeOrExtract(BakeOrExtract::Extract),
        ),
    ]);
    EngineDeclaration {
        selection: Some(ProfileSelection::new(
            if generic {
                "unity-generic"
            } else {
                "unity-humanoid"
            },
            1,
            "6000.3",
            "fbx-model-importer",
        )),
        document_settings: Some(document_settings),
        clip_settings: BTreeMap::from([("*".into(), clip_settings)]),
    }
}

fn resolved(
    declaration: EngineDeclaration,
    source: &LoadedSource,
) -> animsmith_engine::ResolvedProfile {
    resolve_static(declaration)
        .unwrap()
        .unwrap()
        .resolve_input_iter(
            SourceFormatV1::Fbx,
            source
                .document()
                .clips
                .iter()
                .map(|clip| clip.name.as_str()),
        )
        .unwrap()
}

fn config() -> Config {
    let mut config = Config::default();
    config.clips.insert(
        "walk".into(),
        ClipExpectations {
            looping: Some(true),
            movement_owner_xz: Some(MovementOwner::Animation),
            movement_owner_y: Some(MovementOwner::Gameplay),
            movement_owner_yaw: Some(MovementOwner::Animation),
            ..ClipExpectations::default()
        },
    );
    config
}

fn available_generic() -> EngineImportAdviceV1 {
    let source = source(true, &[("Take 001", "walk")]);
    let config = config();
    let profile = resolved(unity_declaration(true), &source);
    EngineImportAdviceV1::from_source(tool(), &source, &profile, &config).unwrap()
}

#[test]
fn static_support_check_accepts_only_the_four_exact_v1_profiles() {
    let supported = profiles_v1()
        .iter()
        .filter(|profile| EngineImportAdviceV1::supports_profile(profile))
        .map(|profile| profile.selection().family())
        .collect::<Vec<_>>();
    assert_eq!(
        supported,
        ["godot", "unity-generic", "unity-humanoid", "unreal"]
    );
}

#[test]
fn unity_generic_projects_exact_materialized_settings_and_clip_evidence() {
    let report = available_generic();
    assert_eq!(report.state(), EngineImportAdviceStateV1::Available);
    assert_eq!(report.refusal_reason(), None);
    assert_eq!(report.clips().len(), 1);
    let clip = &report.clips()[0];
    assert_eq!(clip.source_clip_index(), 0);
    assert_eq!(clip.normalized_clip_index(), 0);
    assert_eq!(clip.normalized_clip_name(), "walk");
    assert_eq!(clip.evidence().duration_s(), 1.0);
    assert_eq!(clip.evidence().looping(), Some(true));
    assert_eq!(
        clip.evidence().movement_owner_xz(),
        Some(animsmith_engine::EngineImportAdviceMovementOwnerV1::Animation)
    );
    let EngineImportAdvicePayloadV1::UnityGeneric { document, clips } = report.payload() else {
        panic!("expected Unity Generic payload");
    };
    assert!(document.convert_units());
    assert!(!document.bake_axis_conversion());
    assert_eq!(document.root_motion_source(), Some("Reference/Root"));
    assert_eq!(clips.len(), 1);
    assert!(!clips[0].lock_root_rotation());
    assert!(clips[0].lock_root_height_y());
    assert!(!clips[0].lock_root_position_xz());

    let bytes = serde_json::to_vec(&report).unwrap();
    let readback = EngineImportAdviceInput::read_from(bytes.as_slice())
        .unwrap()
        .into_report()
        .unwrap();
    assert_eq!(readback.identity(), report.identity());
    assert_eq!(readback.clips(), report.clips());
    assert_eq!(readback.payload(), report.payload());
}

#[test]
fn unity_root_controls_and_movement_axes_are_independently_projected() {
    let source = source(true, &[("Take 001", "walk")]);
    let mut config = Config::default();
    config.clips.insert(
        "walk".into(),
        ClipExpectations {
            movement_owner_xz: Some(MovementOwner::Animation),
            movement_owner_y: Some(MovementOwner::Gameplay),
            movement_owner_yaw: None,
            ..ClipExpectations::default()
        },
    );

    for (baked_setting, expected) in [
        ("root_rotation", (true, false, false)),
        ("root_position_y", (false, true, false)),
        ("root_position_xz", (false, false, true)),
    ] {
        let mut declaration = unity_declaration(true);
        let settings = declaration.clip_settings.get_mut("*").unwrap();
        for setting in ["root_rotation", "root_position_y", "root_position_xz"] {
            settings.insert(
                setting.into(),
                SettingValue::BakeOrExtract(BakeOrExtract::Extract),
            );
        }
        settings.insert(
            baked_setting.into(),
            SettingValue::BakeOrExtract(BakeOrExtract::Bake),
        );

        let profile = resolved(declaration, &source);
        let report = EngineImportAdviceV1::from_source(tool(), &source, &profile, &config).unwrap();
        let evidence = report.clips()[0].evidence();
        assert_eq!(
            (
                evidence.movement_owner_xz(),
                evidence.movement_owner_y(),
                evidence.movement_owner_yaw(),
            ),
            (
                Some(animsmith_engine::EngineImportAdviceMovementOwnerV1::Animation),
                Some(animsmith_engine::EngineImportAdviceMovementOwnerV1::Gameplay),
                None,
            )
        );
        assert_eq!(
            (
                report.payload().unity_clips()[0].lock_root_rotation(),
                report.payload().unity_clips()[0].lock_root_height_y(),
                report.payload().unity_clips()[0].lock_root_position_xz(),
            ),
            expected,
            "wrong Unity control projection when only {baked_setting} is baked"
        );
    }
}

#[test]
fn source_rows_retain_source_order_while_unity_rows_use_normalized_order() {
    let source = source_with_swapped_normalized_indices();
    let profile = resolved(unity_declaration(true), &source);
    let report =
        EngineImportAdviceV1::from_source(tool(), &source, &profile, &Config::default()).unwrap();
    assert_eq!(
        report
            .clips()
            .iter()
            .map(|clip| (
                clip.source_clip_index(),
                clip.normalized_clip_index(),
                clip.normalized_clip_name(),
            ))
            .collect::<Vec<_>>(),
        [(0, 1, "run"), (1, 0, "walk")]
    );
    assert_eq!(
        report
            .payload()
            .unity_clips()
            .iter()
            .map(|clip| clip.normalized_clip_index())
            .collect::<Vec<_>>(),
        [0, 1]
    );
    let bytes = serde_json::to_vec(&report).unwrap();
    EngineImportAdviceInput::read_from(bytes.as_slice())
        .unwrap()
        .into_report()
        .expect("swapped source mapping remains strict-reader valid");
}

#[test]
fn humanoid_omits_the_inapplicable_generic_root_motion_source() {
    let source = source(true, &[("Idle", "idle")]);
    let config = Config::default();
    let profile = resolved(unity_declaration(false), &source);
    let report = EngineImportAdviceV1::from_source(tool(), &source, &profile, &config).unwrap();
    let EngineImportAdvicePayloadV1::UnityHumanoid { document, .. } = report.payload() else {
        panic!("expected Unity Humanoid payload");
    };
    assert_eq!(document.root_motion_source(), None);
}

#[test]
fn unreal_and_godot_refuse_without_fabricating_unmodeled_settings() {
    for (family, version, importer, payload_engine) in [
        ("unreal", "5.8", "fbx-importer", "unreal"),
        ("godot", "4.7", "resource-importer-scene", "godot"),
    ] {
        let source = source(true, &[("Take", "walk")]);
        let profile = resolved(
            EngineDeclaration {
                selection: Some(ProfileSelection::new(family, 1, version, importer)),
                ..EngineDeclaration::default()
            },
            &source,
        );
        let report =
            EngineImportAdviceV1::from_source(tool(), &source, &profile, &Config::default())
                .unwrap();
        assert_eq!(report.state(), EngineImportAdviceStateV1::Refused);
        assert_eq!(
            report.refusal_reason(),
            Some(EngineImportAdviceRefusalReasonV1::ProfileSettingsUnmodeled)
        );
        assert!(report.clips().is_empty());
        assert_eq!(
            serde_json::to_value(report.payload()).unwrap()["engine"],
            payload_engine
        );
    }
}

#[test]
fn partial_raw_clip_inventory_refuses_before_exposing_a_prefix() {
    let source = source(false, &[("Take", "walk")]);
    let config = Config::default();
    let profile = resolved(unity_declaration(true), &source);
    let report = EngineImportAdviceV1::from_source(tool(), &source, &profile, &config).unwrap();
    assert_eq!(report.state(), EngineImportAdviceStateV1::Refused);
    assert_eq!(
        report.refusal_reason(),
        Some(EngineImportAdviceRefusalReasonV1::RawClipInventoryIncomplete)
    );
    assert!(report.clips().is_empty());
    assert!(report.payload().unity_clips().is_empty());

    let mut contradictory = serde_json::to_value(&report).unwrap();
    contradictory["refusal_reason"] = json!("measurement_unavailable");
    assert_eq!(
        read_semantic(contradictory),
        Err(EngineImportAdviceError::InvalidLifecycle)
    );
}

#[test]
fn unavailable_or_non_bijective_clip_identity_refuses_without_suggestions() {
    let unavailable = source_with_unavailable_normalized_index();
    let profile = resolved(unity_declaration(true), &unavailable);
    let report =
        EngineImportAdviceV1::from_source(tool(), &unavailable, &profile, &Config::default())
            .unwrap();
    assert_eq!(
        report.refusal_reason(),
        Some(EngineImportAdviceRefusalReasonV1::ClipIdentityUnavailable)
    );
    assert!(report.clips().is_empty());
    assert!(report.payload().unity_clips().is_empty());
    let mut contradictory = serde_json::to_value(&report).unwrap();
    contradictory["refusal_reason"] = json!("raw_clip_inventory_incomplete");
    assert_eq!(
        read_semantic(contradictory),
        Err(EngineImportAdviceError::InvalidLifecycle)
    );

    let duplicate_names = source(true, &[("Take A", "walk"), ("Take B", "walk")]);
    let profile = resolved(unity_declaration(true), &duplicate_names);
    let report =
        EngineImportAdviceV1::from_source(tool(), &duplicate_names, &profile, &Config::default())
            .unwrap();
    assert_eq!(
        report.refusal_reason(),
        Some(EngineImportAdviceRefusalReasonV1::ClipIdentityMismatch)
    );
    assert!(report.clips().is_empty());
    assert!(report.payload().unity_clips().is_empty());
}

#[test]
fn direct_rust_callers_cannot_bypass_core_intent_validation() {
    let loaded = source(true, &[("Take", "walk")]);
    let profile = resolved(unity_declaration(true), &loaded);
    let mut config = Config::default();
    config.clips.insert(
        "walk".into(),
        ClipExpectations {
            movement_owner_xz: Some(MovementOwner::Animation),
            in_place: Some(true),
            ..ClipExpectations::default()
        },
    );
    assert!(matches!(
        EngineImportAdviceV1::from_source(tool(), &loaded, &profile, &config),
        Err(EngineImportAdviceError::InvalidConfig(_))
    ));

    let other_source = source(true, &[("Other Take", "run")]);
    let other_profile = resolved(unity_declaration(true), &other_source);
    let report =
        EngineImportAdviceV1::from_source(tool(), &loaded, &other_profile, &Config::default())
            .unwrap();
    assert_eq!(report.state(), EngineImportAdviceStateV1::Refused);
    assert_eq!(
        report.refusal_reason(),
        Some(EngineImportAdviceRefusalReasonV1::ClipIdentityMismatch)
    );
    assert!(report.clips().is_empty());
    assert!(report.payload().unity_clips().is_empty());
}

#[test]
fn strict_reader_rejects_identity_settings_profile_and_tool_shape_mutations() {
    let report = available_generic();
    let original = serde_json::to_value(&report).unwrap();

    let mut identity = original.clone();
    identity["identity"]["bytes"] = json!(1);
    assert_eq!(
        read_semantic(identity),
        Err(EngineImportAdviceError::IdentityMismatch)
    );

    let mut setting = original.clone();
    setting["payload"]["document"]["convert_units"] = json!(false);
    assert_eq!(
        read_semantic(setting),
        Err(EngineImportAdviceError::UnitySettingsMismatch)
    );

    let mut profile = original.clone();
    profile["prediction_provenance"]["profile"]["identity"]["bytes"] = json!(1);
    assert!(matches!(
        read_semantic(profile),
        Err(EngineImportAdviceError::InvalidProvenance(_))
    ));

    for field in ["revision", "dirty"] {
        let mut missing = original.clone();
        missing["tool"]["source"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert_eq!(
            read_semantic(missing),
            Err(EngineImportAdviceError::InvalidTool)
        );
    }

    let mut other_version = original.clone();
    other_version["tool"]["version"] = json!("9.9.9-beta.1+pipeline");
    assert_eq!(read_semantic(other_version), Ok(()));

    let mut negative_speed = original.clone();
    negative_speed["clips"][0]["evidence"]["speed_mps"] = json!(-1.0);
    negative_speed["clips"][0]["evidence"]["speed_mps_availability"] = json!("measured");
    assert_eq!(
        read_semantic(negative_speed),
        Err(EngineImportAdviceError::InvalidMeasurement)
    );

    let mut contradictory_loop_evidence = original.clone();
    contradictory_loop_evidence["clips"][0]["evidence"]["loop"] = json!(false);
    assert_eq!(
        read_semantic(contradictory_loop_evidence),
        Err(EngineImportAdviceError::InvalidMeasurement)
    );

    let mut non_dense_normalized_index = original.clone();
    non_dense_normalized_index["clips"][0]["normalized_clip_index"] = json!(1);
    non_dense_normalized_index["payload"]["clips"][0]["normalized_clip_index"] = json!(1);
    assert_eq!(
        read_semantic(non_dense_normalized_index),
        Err(EngineImportAdviceError::InvalidClipIdentity)
    );

    let mut unknown = original;
    unknown["unexpected"] = json!(true);
    let bytes = serde_json::to_vec(&unknown).unwrap();
    assert!(EngineImportAdviceInput::read_from(bytes.as_slice()).is_err());

    for pointer in [
        "/refusal_reason",
        "/clips/0/evidence/speed_mps",
        "/clips/0/evidence/loop",
        "/payload/document/root_motion_source",
    ] {
        let mut explicit_null = serde_json::to_value(available_generic()).unwrap();
        let (parent, field) = pointer.rsplit_once('/').unwrap();
        explicit_null
            .pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert(field.into(), Value::Null);
        assert!(
            serde_json::from_value::<EngineImportAdviceInput>(explicit_null).is_err(),
            "explicit null at {pointer} must not be conflated with omission"
        );
    }
}

#[test]
fn clip_collections_accept_n_and_reject_n_plus_one_without_decoding_the_sentinel() {
    let original = serde_json::to_value(available_generic()).unwrap();
    let clip = original["clips"][0].clone();
    let unity_clip = original["payload"]["clips"][0].clone();

    let mut exact = original.clone();
    exact["clips"] = Value::Array(
        (0..animsmith_core::RAW_SOURCE_V1_MAX_CLIPS)
            .map(|index| {
                let mut row = clip.clone();
                row["source_clip_index"] = json!(index);
                row["normalized_clip_index"] = json!(index);
                row
            })
            .collect(),
    );
    assert!(serde_json::from_value::<EngineImportAdviceInput>(exact).is_ok());

    let mut over = original.clone();
    let mut rows = (0..animsmith_core::RAW_SOURCE_V1_MAX_CLIPS)
        .map(|index| {
            let mut row = clip.clone();
            row["source_clip_index"] = json!(index);
            row["normalized_clip_index"] = json!(index);
            row
        })
        .collect::<Vec<_>>();
    rows.push(Value::Null);
    over["clips"] = Value::Array(rows);
    assert!(serde_json::from_value::<EngineImportAdviceInput>(over).is_err());

    let mut payload_over = original;
    let mut rows = vec![unity_clip; animsmith_core::RAW_SOURCE_V1_MAX_CLIPS];
    rows.push(Value::Null);
    payload_over["payload"]["clips"] = Value::Array(rows);
    assert!(serde_json::from_value::<EngineImportAdviceInput>(payload_over).is_err());
}

#[test]
fn canonical_identity_is_pinned_for_a_full_available_record() {
    let report = available_generic();
    assert_eq!(
        report.identity().input_identity().sha256(),
        "0ca38da8c25b876b87229b2fbeb1642491e649ee25601f2af7974126128fac36"
    );
    assert_eq!(report.identity().input_identity().bytes(), 413);
}

fn read_semantic(value: Value) -> Result<(), EngineImportAdviceError> {
    let bytes = serde_json::to_vec(&value).unwrap();
    EngineImportAdviceInput::read_from(bytes.as_slice())
        .unwrap()
        .into_report()
        .map(|_| ())
}
