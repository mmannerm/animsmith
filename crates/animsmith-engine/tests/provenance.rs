use animsmith_core::engine_contract::{
    ENGINE_PROFILE_FACTS_V1_ID, EngineSettingIdV1, EngineSettingValueV1,
    RESOLVED_ENGINE_SETTINGS_V1_ID,
};
use animsmith_core::prediction::{PREDICTION_PROVENANCE_V1_ID, RawSourceBindingV1};
use animsmith_core::{
    DependencyClosureBuilderV1, DependencyClosureCoverageV1, DependencyResourceKeyV1, Document,
    InputIdentity, LoadedSource, RawSourceFactsBuilderV1, ResourceKeySyntaxV1,
    SourceConstructFactV1, SourceConstructKindV1, SourceFactDomainV1, SourceFormatV1,
    SourceFramesPerSecondV1, SourceLinearUnitV1, SourceLoaderDispositionV1, SourceObservationV1,
    SourceProvenanceV1, SourceResourceKindV1, SourceResourceLocatorV1, SourceResourceReferenceV1,
    SourceSetCoverageStateV1, SourceTextV1, SourceUnavailableReasonV1,
};
use animsmith_engine::{
    BakeOrExtract, EngineDeclaration, ProfileSelection, SettingMap, SettingValue,
    project_prediction_provenance_v1, resolve_static,
};

#[derive(Clone, Copy)]
enum CoverageCase {
    Complete,
    Partial,
    Unavailable,
}

fn resolved_godot(format: SourceFormatV1) -> animsmith_engine::ResolvedProfile {
    resolve_static(EngineDeclaration {
        selection: Some(ProfileSelection::new(
            "godot",
            1,
            "4.7",
            "resource-importer-scene",
        )),
        ..EngineDeclaration::default()
    })
    .expect("static profile")
    .expect("selected profile")
    .resolve_input(format, &[])
    .expect("accepted format")
}

fn loaded_source(format: SourceFormatV1, case: CoverageCase) -> LoadedSource {
    let primary = InputIdentity::from_bytes(match case {
        CoverageCase::Complete => b"complete source" as &[u8],
        CoverageCase::Partial => b"partial source" as &[u8],
        CoverageCase::Unavailable => b"unavailable source" as &[u8],
    });
    let mut facts = RawSourceFactsBuilderV1::new(format, primary.clone());
    assert!(facts.set_linear_unit(SourceObservationV1::observed(
        SourceLinearUnitV1::new(0.01).expect("centimetres"),
        SourceProvenanceV1::format_defined(),
        SourceLoaderDispositionV1::Preserved,
    )));
    assert!(facts.set_frames_per_second(SourceObservationV1::observed(
        SourceFramesPerSecondV1::new(30.0).expect("finite frame rate"),
        SourceProvenanceV1::format_defined(),
        SourceLoaderDispositionV1::Preserved,
    )));
    assert!(
        facts.push_construct(
            SourceConstructFactV1::new(
                0,
                SourceConstructKindV1::CustomProperty,
                SourceTextV1::new("analytic_property").expect("bounded source text"),
                false,
                1,
                SourceLoaderDispositionV1::Preserved,
                SourceProvenanceV1::format_defined(),
            )
            .expect("positive construct"),
        )
    );
    facts.mark_complete(SourceFactDomainV1::Clips);
    facts.mark_complete(SourceFactDomainV1::Constructs);
    match case {
        CoverageCase::Complete => facts.mark_complete(SourceFactDomainV1::Resources),
        CoverageCase::Partial => facts.mark_partial(
            SourceFactDomainV1::Resources,
            SourceUnavailableReasonV1::ParserUnavailable,
        ),
        CoverageCase::Unavailable => facts.mark_unavailable(
            SourceFactDomainV1::Resources,
            SourceUnavailableReasonV1::ParserUnavailable,
        ),
    }
    let closure = DependencyClosureBuilderV1::new(
        primary,
        facts.resource_coverage(),
        facts.resource_rows().len(),
    )
    .finish()
    .expect("empty analytic closure");
    facts
        .finish_with_dependency_closure(Document::default(), closure)
        .expect("same-load analytic facts")
}

fn unity_generic_declaration(
    reverse_insertion_order: bool,
    selector: Option<&str>,
) -> EngineDeclaration {
    let mut document_rows = vec![
        ("convert_units", SettingValue::Boolean(true)),
        ("bake_axis_conversion", SettingValue::Boolean(false)),
        (
            "root_motion_source",
            SettingValue::SourceTransformPath("Reference/Root".into()),
        ),
    ];
    let mut clip_rows = vec![
        (
            "root_rotation",
            SettingValue::BakeOrExtract(BakeOrExtract::Extract),
        ),
        (
            "root_position_y",
            SettingValue::BakeOrExtract(BakeOrExtract::Bake),
        ),
        (
            "root_position_xz",
            SettingValue::BakeOrExtract(BakeOrExtract::Extract),
        ),
    ];
    if reverse_insertion_order {
        document_rows.reverse();
        clip_rows.reverse();
    }

    let mut document_settings = SettingMap::new();
    for (id, value) in document_rows {
        document_settings.insert(id.into(), value);
    }
    let mut declaration = EngineDeclaration {
        selection: Some(ProfileSelection::new(
            "unity-generic",
            1,
            "6000.3",
            "fbx-model-importer",
        )),
        document_settings: Some(document_settings),
        ..EngineDeclaration::default()
    };
    if let Some(selector) = selector {
        let mut settings = SettingMap::new();
        for (id, value) in clip_rows {
            settings.insert(id.into(), value);
        }
        declaration.clip_settings.insert(selector.into(), settings);
    }
    declaration
}

fn loaded_source_with_external_resource(external_bytes: &[u8]) -> LoadedSource {
    let primary = InputIdentity::from_bytes(b"dependency-bearing analytic source");
    let mut facts = RawSourceFactsBuilderV1::new(SourceFormatV1::GltfJson, primary.clone());
    assert!(facts.push_resource(SourceResourceReferenceV1::new(
        0,
        SourceResourceKindV1::Buffer,
        0,
        SourceResourceLocatorV1::classify("buffers/a%20b.bin"),
        SourceLoaderDispositionV1::Preserved,
        SourceProvenanceV1::format_defined(),
    )));
    facts.mark_complete(SourceFactDomainV1::Clips);
    facts.mark_complete(SourceFactDomainV1::Constructs);
    facts.mark_complete(SourceFactDomainV1::Resources);

    let key =
        DependencyResourceKeyV1::from_source_str("buffers/a b.bin", ResourceKeySyntaxV1::GltfUri)
            .expect("safe normalized dependency key");
    let mut closure = DependencyClosureBuilderV1::new(
        primary,
        facts.resource_coverage(),
        facts.resource_rows().len(),
    );
    assert!(closure.begin_reference(17, 2));
    assert_eq!(
        closure.prepare_external_key(&key).expect("prepare key"),
        Some(true)
    );
    closure
        .record_external_open_attempt(&key)
        .expect("record open");
    assert!(
        closure
            .push_captured_external(
                0,
                SourceResourceKindV1::Buffer,
                0,
                key,
                InputIdentity::from_bytes(external_bytes),
            )
            .expect("capture external resource")
    );
    facts
        .finish_with_dependency_closure(
            Document::default(),
            closure.finish().expect("complete dependency closure"),
        )
        .expect("closure matches retained raw facts")
}

#[test]
fn adapter_preserves_complete_partial_and_unavailable_same_load_evidence() {
    for (case, expected_raw, expected_closure) in [
        (
            CoverageCase::Complete,
            SourceSetCoverageStateV1::Complete,
            "complete",
        ),
        (
            CoverageCase::Partial,
            SourceSetCoverageStateV1::Partial,
            "partial",
        ),
        (
            CoverageCase::Unavailable,
            SourceSetCoverageStateV1::Unavailable,
            "unavailable",
        ),
    ] {
        let resolved = resolved_godot(SourceFormatV1::Fbx);
        let source = loaded_source(SourceFormatV1::Fbx, case);
        assert_eq!(
            source.source_facts().resources().coverage().state(),
            expected_raw
        );
        let expected_raw_binding = RawSourceBindingV1::from_source(source.source_facts());
        let expected_closure_value = source.dependency_closure().clone();

        let provenance = project_prediction_provenance_v1(&resolved, &source)
            .expect("same-load evidence projects");

        assert_eq!(provenance.raw_source(), &expected_raw_binding);
        assert_eq!(provenance.dependency_closure(), &expected_closure_value);
        assert_eq!(
            provenance.raw_source().primary_input(),
            source.source_facts().primary_identity()
        );
        assert_eq!(provenance.raw_source().source_format(), SourceFormatV1::Fbx);
        match (expected_closure, provenance.dependency_closure().coverage()) {
            ("complete", DependencyClosureCoverageV1::Complete)
            | ("partial", DependencyClosureCoverageV1::Partial { .. })
            | ("unavailable", DependencyClosureCoverageV1::Unavailable { .. }) => {}
            _ => panic!("closure coverage was not preserved"),
        }
    }
}

#[test]
fn adapter_projects_the_full_profile_tuple_settings_and_canonical_identities() {
    let resolved = resolve_static(EngineDeclaration {
        selection: Some(ProfileSelection::new(
            "unity-generic",
            1,
            "6000.3",
            "fbx-model-importer",
        )),
        document_settings: Some(
            [
                ("convert_units".into(), SettingValue::Boolean(true)),
                ("bake_axis_conversion".into(), SettingValue::Boolean(false)),
                (
                    "root_motion_source".into(),
                    SettingValue::SourceTransformPath("Reference/Root".into()),
                ),
            ]
            .into(),
        ),
        ..EngineDeclaration::default()
    })
    .expect("static profile")
    .expect("selected profile")
    .resolve_input(SourceFormatV1::Fbx, &[])
    .expect("accepted input");
    let source = loaded_source(SourceFormatV1::Fbx, CoverageCase::Complete);

    let provenance =
        project_prediction_provenance_v1(&resolved, &source).expect("complete projection");
    let original = resolved.profile();
    let projected = provenance.profile();
    let selection = projected.selection();

    assert_eq!(provenance.contract_id(), PREDICTION_PROVENANCE_V1_ID);
    assert_eq!(provenance.source_format(), resolved.source_format());
    assert_eq!(projected.contract_id(), ENGINE_PROFILE_FACTS_V1_ID);
    assert_eq!(
        provenance.settings().contract_id(),
        RESOLVED_ENGINE_SETTINGS_V1_ID
    );
    assert_eq!(selection.family(), original.selection().family());
    assert_eq!(
        selection.profile_revision(),
        original.selection().profile_revision()
    );
    assert_eq!(
        selection.engine_version(),
        original.selection().engine_version()
    );
    assert_eq!(selection.importer(), original.selection().importer());
    assert_eq!(projected.fact_bundle_urn(), original.fact_bundle_urn());
    assert_eq!(projected.facts().len(), original.facts().len());
    assert_eq!(
        projected.setting_descriptors().len(),
        original.setting_descriptors().len()
    );
    assert_eq!(projected.primary_sources().len(), original.sources().len());
    assert_eq!(projected.facts_identity(), original.facts_identity());
    assert_eq!(
        provenance.settings().settings_identity(),
        resolved.settings_identity()
    );
    assert_eq!(provenance.settings().document_settings().len(), 3);
    assert_eq!(
        provenance
            .settings()
            .document_setting(EngineSettingIdV1::ConvertUnits),
        Some(&EngineSettingValueV1::Boolean(true))
    );
    assert_eq!(
        provenance
            .settings()
            .document_setting(EngineSettingIdV1::BakeAxisConversion),
        Some(&EngineSettingValueV1::Boolean(false))
    );
    assert_eq!(provenance.settings().clips(), &[]);

    for (projected, original) in projected.primary_sources().iter().zip(original.sources()) {
        assert_eq!(projected.id(), original.id());
        assert_eq!(projected.target_version(), original.target_version());
        assert_eq!(projected.url(), original.url());
        assert_eq!(projected.verified_on(), original.verified_on());
        assert_eq!(
            projected.supported_fact_ids().len(),
            original.supported_facts().len()
        );
        assert_eq!(
            projected.supported_setting_ids().len(),
            original.supported_settings().len()
        );
    }
}

#[test]
fn caller_setting_map_insertion_order_does_not_change_settings_identity() {
    let forward = resolve_static(unity_generic_declaration(false, None))
        .expect("static profile")
        .expect("selected profile")
        .resolve_input(SourceFormatV1::Fbx, &[])
        .expect("accepted input");
    let reverse = resolve_static(unity_generic_declaration(true, None))
        .expect("static profile")
        .expect("selected profile")
        .resolve_input(SourceFormatV1::Fbx, &[])
        .expect("accepted input");

    assert_eq!(forward.settings_identity(), reverse.settings_identity());
}

#[test]
fn equivalent_exact_and_glob_declarations_project_the_same_provenance_identity() {
    let clip_names = vec!["walk_forward".into()];
    let exact = resolve_static(unity_generic_declaration(false, Some("walk_forward")))
        .expect("static profile")
        .expect("selected profile")
        .resolve_input(SourceFormatV1::Fbx, &clip_names)
        .expect("accepted input");
    let glob = resolve_static(unity_generic_declaration(true, Some("walk*")))
        .expect("static profile")
        .expect("selected profile")
        .resolve_input(SourceFormatV1::Fbx, &clip_names)
        .expect("accepted input");
    assert_eq!(exact.settings_identity(), glob.settings_identity());

    let source = loaded_source(SourceFormatV1::Fbx, CoverageCase::Complete);
    let exact = project_prediction_provenance_v1(&exact, &source).expect("exact projection");
    let glob = project_prediction_provenance_v1(&glob, &source).expect("glob projection");

    assert_eq!(exact.identity(), glob.identity());
}

#[test]
fn changed_complete_dependency_content_changes_provenance_identity() {
    let first_source = loaded_source_with_external_resource(b"external content one");
    let second_source = loaded_source_with_external_resource(b"external content two");
    assert_eq!(
        first_source.source_facts().primary_identity(),
        second_source.source_facts().primary_identity()
    );
    assert_eq!(
        RawSourceBindingV1::from_source(first_source.source_facts()),
        RawSourceBindingV1::from_source(second_source.source_facts())
    );
    assert!(first_source.dependency_closure().coverage().is_complete());
    assert!(second_source.dependency_closure().coverage().is_complete());
    assert_ne!(
        first_source.dependency_closure().identity(),
        second_source.dependency_closure().identity()
    );

    let resolved = resolved_godot(SourceFormatV1::GltfJson);
    let first = project_prediction_provenance_v1(&resolved, &first_source)
        .expect("first complete projection");
    let second = project_prediction_provenance_v1(&resolved, &second_source)
        .expect("second complete projection");

    assert_eq!(first.profile(), second.profile());
    assert_eq!(first.settings(), second.settings());
    assert_eq!(first.raw_source(), second.raw_source());
    assert_ne!(first.dependency_closure(), second.dependency_closure());
    assert_ne!(first.identity(), second.identity());
}

#[test]
fn core_manifest_has_no_reverse_engine_dependency() {
    let core_manifest =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../animsmith-core/Cargo.toml");
    let contents = std::fs::read_to_string(core_manifest).expect("read core manifest");
    assert!(!contents.contains("animsmith-engine"));
}
