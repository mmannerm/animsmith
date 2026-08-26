use animsmith_core::ImportSettingProjectionKindV1;
use animsmith_core::{
    DependencyClosureBuilderV1, Document, InputIdentity, LoadedSource, RawSourceFactsBuilderV1,
    SourceFactDomainV1, SourceFormatV1, ToolInfo, ToolSource,
};
use animsmith_engine::{
    EngineDeclarationV2, EngineImportAdviceInputV2, EngineImportAdviceProjectionValueV2,
    EngineImportAdviceRefusalReasonV2, EngineImportAdviceStateV2, EngineImportAdviceV2,
    ProfileSelection, SettingValueV2, resolve_static_v2,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn tool() -> ToolInfo {
    ToolInfo::animsmith(ToolSource::new(
        Some("0123456789abcdef0123456789abcdef01234567".into()),
        Some(false),
    ))
}

#[test]
fn supports_profile_pins_only_the_two_v2_import_tuples() {
    assert!(EngineImportAdviceV2::supports_profile(
        animsmith_engine::lookup_profile_v2(&ProfileSelection::new(
            "godot",
            2,
            "4.7",
            "resource-importer-scene",
        ))
        .unwrap()
    ));
    assert!(EngineImportAdviceV2::supports_profile(
        animsmith_engine::lookup_profile_v2(&ProfileSelection::new(
            "unreal",
            2,
            "5.8",
            "fbx-importer",
        ))
        .unwrap()
    ));
    assert!(!EngineImportAdviceV2::supports_profile(
        animsmith_engine::lookup_profile_v2(&ProfileSelection::new(
            "bevy",
            2,
            "0.19.0",
            "gltf-asset-loader",
        ))
        .unwrap()
    ));
}

fn source(format: SourceFormatV1) -> LoadedSource {
    let primary = InputIdentity::from_bytes(b"v2 import-advice source");
    let mut facts = RawSourceFactsBuilderV1::new(format, primary.clone());
    facts.mark_complete(SourceFactDomainV1::Clips);
    facts.mark_complete(SourceFactDomainV1::Constructs);
    facts.mark_complete(SourceFactDomainV1::Resources);
    let closure = DependencyClosureBuilderV1::new(
        primary,
        facts.resource_coverage(),
        facts.resource_rows().len(),
    )
    .finish()
    .unwrap();
    facts
        .finish_with_dependency_closure(Document::default(), closure)
        .unwrap()
}

fn resolved(
    selection: ProfileSelection,
    format: SourceFormatV1,
    settings: BTreeMap<String, SettingValueV2>,
) -> (LoadedSource, animsmith_engine::ResolvedProfileSettingsV2) {
    let source = source(format);
    let declaration = EngineDeclarationV2 {
        selection: Some(selection),
        document_settings: Some(settings),
        ..EngineDeclarationV2::default()
    };
    let resolution = resolve_static_v2(declaration).unwrap().unwrap();
    let resolved = resolution.resolve_input(format).unwrap();
    (source, resolved)
}

#[test]
fn godot_defaults_project_exact_params_and_round_trip_strictly() {
    let (source, resolved) = resolved(
        ProfileSelection::new("godot", 2, "4.7", "resource-importer-scene"),
        SourceFormatV1::GltfJson,
        BTreeMap::new(),
    );
    let report = EngineImportAdviceV2::from_source(tool(), &source, &resolved).unwrap();
    assert_eq!(report.state(), EngineImportAdviceStateV2::Available);
    assert_eq!(report.identity().input_identity().bytes(), 1_066);
    assert_eq!(
        report.identity().input_identity().sha256(),
        "d80b40b7726eea07283148a6fef5f697008334f8b0fa8958ee29911529d5564f"
    );
    assert_eq!(
        serde_json::to_value(report.basis().references()).unwrap(),
        json!([
            { "contract": "v2", "reference": { "contract": "v1", "reference": { "kind": "primary_source", "source_id": "godot-resource-importer-scene-4.7" } } },
            { "contract": "v2", "reference": { "contract": "v1", "reference": { "kind": "profile_fact", "fact_id": "import_setting_projection" } } },
            { "contract": "v2", "reference": { "contract": "v1", "reference": { "kind": "resolved_setting", "location": { "scope": "document" }, "setting_id": "animation_fps" } } },
            { "contract": "v2", "reference": { "contract": "v1", "reference": { "kind": "resolved_setting", "location": { "scope": "document" }, "setting_id": "animation_trimming" } } }
        ])
    );
    let projection = report
        .projection()
        .expect("expected import-setting projection");
    assert_eq!(
        projection.projection_kind,
        ImportSettingProjectionKindV1::GodotParams
    );
    assert_eq!(
        projection
            .fields
            .iter()
            .map(|field| field.key.as_str())
            .collect::<Vec<_>>(),
        ["animation/fps", "animation/trimming"]
    );
    assert!(projection.fields.iter().all(|field| {
        field.value_origin
            == animsmith_core::engine_contract::EngineSettingValueOriginV3::ProfileDefault
    }));
    let wire = serde_json::to_vec(&report).unwrap();
    let readback = EngineImportAdviceInputV2::read_from(wire.as_slice())
        .unwrap()
        .into_report()
        .unwrap();
    assert_eq!(readback.identity(), report.identity());
    assert_eq!(
        readback.prediction_provenance(),
        report.prediction_provenance()
    );
}

#[test]
fn unreal_projection_has_no_hidden_custom_rate_for_default30() {
    let (source, resolved) = resolved(
        ProfileSelection::new("unreal", 2, "5.8", "fbx-importer"),
        SourceFormatV1::Fbx,
        BTreeMap::from([(
            "sample_rate".into(),
            SettingValueV2::SampleRate(animsmith_engine::UnrealSampleRateV2::Default30),
        )]),
    );
    let report = EngineImportAdviceV2::from_source(tool(), &source, &resolved).unwrap();
    let projection = report
        .projection()
        .expect("expected import-setting projection");
    assert_eq!(
        projection
            .fields
            .iter()
            .map(|field| field.key.as_str())
            .collect::<Vec<_>>(),
        ["bUseDefaultSampleRate"]
    );
}

#[test]
fn unreal_source_determined_and_custom_rates_are_native_numeric_fields() {
    for (policy, expected_custom) in [
        (
            animsmith_engine::UnrealSampleRateV2::SourceDetermined,
            Some(0),
        ),
        (
            animsmith_engine::UnrealSampleRateV2::CustomHz(48_000),
            Some(48_000),
        ),
    ] {
        let (source, resolved) = resolved(
            ProfileSelection::new("unreal", 2, "5.8", "fbx-importer"),
            SourceFormatV1::Fbx,
            BTreeMap::from([("sample_rate".into(), SettingValueV2::SampleRate(policy))]),
        );
        let report = EngineImportAdviceV2::from_source(tool(), &source, &resolved).unwrap();
        let projection = report
            .projection()
            .expect("expected import-setting projection");
        assert_eq!(projection.fields.len(), 2);
        assert_eq!(projection.fields[0].key, "CustomSampleRate");
        assert_eq!(projection.fields[1].key, "bUseDefaultSampleRate");
        let wire = serde_json::to_string(&report).unwrap();
        if expected_custom == Some(0) {
            assert!(wire.contains("\"unsigned_integer\":0"));
        }
        assert_eq!(
            projection.fields[0].value,
            EngineImportAdviceProjectionValueV2::UnsignedInteger(expected_custom.unwrap())
        );
    }
}

#[test]
fn incomplete_dependency_closure_is_typed_refusal_with_basis() {
    let primary = InputIdentity::from_bytes(b"unavailable closure");
    let mut facts = RawSourceFactsBuilderV1::new(SourceFormatV1::Fbx, primary.clone());
    facts.mark_complete(SourceFactDomainV1::Clips);
    facts.mark_complete(SourceFactDomainV1::Constructs);
    let closure = DependencyClosureBuilderV1::new(
        primary,
        facts.resource_coverage(),
        facts.resource_rows().len(),
    )
    .finish()
    .unwrap();
    let source = facts
        .finish_with_dependency_closure(Document::default(), closure)
        .unwrap();
    let resolution = resolve_static_v2(EngineDeclarationV2 {
        selection: Some(ProfileSelection::new("unreal", 2, "5.8", "fbx-importer")),
        document_settings: Some(BTreeMap::from([(
            "sample_rate".into(),
            SettingValueV2::SampleRate(animsmith_engine::UnrealSampleRateV2::SourceDetermined),
        )])),
        ..EngineDeclarationV2::default()
    })
    .unwrap()
    .unwrap();
    let resolved = resolution.resolve_input(SourceFormatV1::Fbx).unwrap();
    let report = EngineImportAdviceV2::from_source(tool(), &source, &resolved).unwrap();
    assert_eq!(report.state(), EngineImportAdviceStateV2::Refused);
    assert_eq!(
        report.refusal_reason(),
        Some(EngineImportAdviceRefusalReasonV2::DependencyClosureIncomplete)
    );
    assert!(!report.basis().references().is_empty());
}

#[test]
fn strict_readback_rejects_unknown_envelope_fields() {
    let (source, resolved) = resolved(
        ProfileSelection::new("godot", 2, "4.7", "resource-importer-scene"),
        SourceFormatV1::Glb,
        BTreeMap::new(),
    );
    let report = EngineImportAdviceV2::from_source(tool(), &source, &resolved).unwrap();
    let mut value: Value = serde_json::from_slice(&serde_json::to_vec(&report).unwrap()).unwrap();
    value["unexpected"] = Value::Bool(true);
    let bytes = serde_json::to_vec(&value).unwrap();
    assert!(EngineImportAdviceInputV2::read_from(bytes.as_slice()).is_err());
}

#[test]
fn strict_readback_rejects_projection_value_mutation() {
    let (source, resolved) = resolved(
        ProfileSelection::new("unreal", 2, "5.8", "fbx-importer"),
        SourceFormatV1::Fbx,
        BTreeMap::from([(
            "sample_rate".into(),
            SettingValueV2::SampleRate(animsmith_engine::UnrealSampleRateV2::SourceDetermined),
        )]),
    );
    let report = EngineImportAdviceV2::from_source(tool(), &source, &resolved).unwrap();
    let mut value: Value = serde_json::from_slice(&serde_json::to_vec(&report).unwrap()).unwrap();
    value["projection"]["fields"][0]["value"]["unsigned_integer"] = Value::from(1);
    let bytes = serde_json::to_vec(&value).unwrap();
    let input = EngineImportAdviceInputV2::read_from(bytes.as_slice()).unwrap();
    assert!(input.into_report().is_err());

    let mut lifecycle: Value =
        serde_json::from_slice(&serde_json::to_vec(&report).unwrap()).unwrap();
    lifecycle["refusal_reason"] = Value::from("dependency_closure_incomplete");
    let bytes = serde_json::to_vec(&lifecycle).unwrap();
    let input = EngineImportAdviceInputV2::read_from(bytes.as_slice()).unwrap();
    assert!(input.into_report().is_err());
}

#[test]
fn strict_readback_rejects_embedded_authority_and_basis_mutations() {
    let (source, resolved) = resolved(
        ProfileSelection::new("godot", 2, "4.7", "resource-importer-scene"),
        SourceFormatV1::Glb,
        BTreeMap::new(),
    );
    let report = EngineImportAdviceV2::from_source(tool(), &source, &resolved).unwrap();
    let value: Value = serde_json::from_slice(&serde_json::to_vec(&report).unwrap()).unwrap();

    let mut profile_identity = value.clone();
    profile_identity["prediction_provenance"]["profile"]["identity"]["bytes"] = Value::from(0);
    let input = EngineImportAdviceInputV2::read_from(
        serde_json::to_vec(&profile_identity).unwrap().as_slice(),
    )
    .unwrap();
    assert!(matches!(
        input.into_report(),
        Err(animsmith_engine::EngineImportAdviceError::InvalidV2Provenance(_))
    ));

    let mut fact_identity = value.clone();
    fact_identity["prediction_provenance"]["profile"]["facts"][0]["state"] = json!("unknown");
    let input = EngineImportAdviceInputV2::read_from(
        serde_json::to_vec(&fact_identity).unwrap().as_slice(),
    )
    .unwrap();
    assert!(matches!(
        input.into_report(),
        Err(animsmith_engine::EngineImportAdviceError::InvalidV2Provenance(_))
    ));

    let mut setting_origin = value.clone();
    setting_origin["prediction_provenance"]["settings"]["document_settings"][0]["value_origin"] =
        json!("explicit_config");
    let input = EngineImportAdviceInputV2::read_from(
        serde_json::to_vec(&setting_origin).unwrap().as_slice(),
    )
    .unwrap();
    assert!(matches!(
        input.into_report(),
        Err(animsmith_engine::EngineImportAdviceError::InvalidV2Provenance(_))
    ));

    let mut basis_reference = value;
    basis_reference["basis"]["references"][0]["reference"]["reference"]["source_id"] =
        json!("unknown-source");
    let input = EngineImportAdviceInputV2::read_from(
        serde_json::to_vec(&basis_reference).unwrap().as_slice(),
    )
    .unwrap();
    assert!(matches!(
        input.into_report(),
        Err(animsmith_engine::EngineImportAdviceError::InvalidV2Prediction(_))
    ));
}

#[test]
fn strict_readback_rejects_refusal_when_authority_is_complete() {
    let (source, resolved) = resolved(
        ProfileSelection::new("godot", 2, "4.7", "resource-importer-scene"),
        SourceFormatV1::Glb,
        BTreeMap::new(),
    );
    let report = EngineImportAdviceV2::from_source(tool(), &source, &resolved).unwrap();
    let mut value: Value = serde_json::from_slice(&serde_json::to_vec(&report).unwrap()).unwrap();
    value["state"] = Value::from("refused");
    value
        .as_object_mut()
        .expect("advice envelope must be an object")
        .remove("projection");
    value["refusal_reason"] = Value::from("dependency_closure_incomplete");

    let bytes = serde_json::to_vec(&value).unwrap();
    let input = EngineImportAdviceInputV2::read_from(bytes.as_slice()).unwrap();
    assert!(matches!(
        input.into_report(),
        Err(animsmith_engine::EngineImportAdviceError::InvalidV2Lifecycle)
    ));
}
