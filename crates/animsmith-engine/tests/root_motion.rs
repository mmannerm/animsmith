use animsmith_core::config::CheckSettings;
use animsmith_core::glam::{Quat, Vec3};
use animsmith_core::measure::{MeasurementAvailability, measure_document_indexed};
use animsmith_core::{
    Applicability, Bone, Check, CheckCtx, CheckEvaluation, CheckSelection, Clip, Config,
    EngineMachineResultV1, EnginePredictionBasisV4, EnginePredictionFacetStateV1,
    EnginePredictionFacetV4, EnginePredictionV4, EnginePredictionV6,
    EngineRootMotionClipIntentInputV1, EngineRootMotionClipMappingStateV1,
    EngineRootMotionProjectIntentV1, EvaluationScope, EvaluationScopeCode, InputIdentity,
    Interpolation, LintEnvelopeV17, LintFileReportV17, MeasurementContract, MeasurementReportInput,
    MetricGrids, PREDICTION_V2_MAX_CANDIDATE_FACETS_PER_RULE, PredictionFacetDemandV2,
    PredictionRuleDemandV2, PredictionUnavailableReasonV2, Property, RawSourceFactsBuilderV1,
    RawTransformPathInventoryV1, RawTransformPathNodeInputV1, RawTransformPathNodeKindV1,
    ResolvedRoles, RigInfo, Role, RootMotionCompatibilityV1, RootMotionImporterDispositionV1,
    RootMotionProjectOwnerV1, SourceClipFactV1, SourceFactDomainV1, SourceFactSetV1,
    SourceFormatV1, SourceLoaderDispositionV1, SourceObservationV1, SourceProvenanceV1,
    SourceTextV1, ToolInfo, ToolSource, Track, TrackValues, Transform,
    allocate_prediction_facets_v2,
};
use animsmith_engine::{
    BakeOrExtract, ENGINE_ROOT_MOTION_CHECK_ID, EngineDeclarationV2, EngineRootMotionCheck,
    PredictionRuleError, ProfileSelection, SettingValueV2, UnityAnimationTypeV2,
    UnityAvatarSetupV2, project_prediction_provenance_v6, resolve_static_v2,
};
use std::collections::BTreeMap;

#[derive(Clone, Copy)]
enum PathFixture {
    Exact,
    NoMatch,
    HelperOnly,
    MismatchedBone,
    Ambiguous,
    Incomplete,
    MissingSidecar,
}

fn document(names: &[&str]) -> animsmith_core::Document {
    let mut document = animsmith_core::Document::default();
    document.skeleton.bones = vec![
        Bone {
            name: "Root".into(),
            parent: None,
            rest: Transform::IDENTITY,
            inverse_bind: None,
        },
        Bone {
            name: "Other".into(),
            parent: None,
            rest: Transform::IDENTITY,
            inverse_bind: None,
        },
    ];
    document.clips = names
        .iter()
        .map(|name| Clip {
            name: (*name).into(),
            duration_s: 1.0,
            tracks: vec![
                Track {
                    bone: 0,
                    property: Property::Translation,
                    interpolation: Interpolation::Linear,
                    times: vec![0.0, 0.5, 1.0],
                    values: TrackValues::Vec3s(vec![Vec3::ZERO, Vec3::splat(0.5), Vec3::ONE]),
                },
                Track {
                    bone: 0,
                    property: Property::Rotation,
                    interpolation: Interpolation::Linear,
                    times: vec![0.0, 0.5, 1.0],
                    values: TrackValues::Quats(vec![
                        Quat::IDENTITY,
                        Quat::from_rotation_y(0.25),
                        Quat::from_rotation_y(0.5),
                    ]),
                },
            ],
        })
        .collect();
    document
}

fn source(
    witness: &str,
    names: &[&str],
    path_fixture: PathFixture,
) -> animsmith_core::LoadedSource {
    source_with_format(witness, names, path_fixture, SourceFormatV1::Fbx)
}

fn source_with_format(
    witness: &str,
    names: &[&str],
    path_fixture: PathFixture,
    format: SourceFormatV1,
) -> animsmith_core::LoadedSource {
    let primary = InputIdentity::from_bytes(witness.as_bytes());
    let mut facts = RawSourceFactsBuilderV1::new(format, primary.clone());
    for (index, name) in names.iter().enumerate() {
        assert!(facts.push_clip(SourceClipFactV1::new(
            index,
            SourceObservationV1::observed(
                SourceTextV1::new(*name).unwrap(),
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
    }
    facts.mark_complete(SourceFactDomainV1::Clips);
    facts.mark_complete(SourceFactDomainV1::Constructs);
    facts.mark_complete(SourceFactDomainV1::Resources);
    let source = facts.finish(document(names)).unwrap();
    if matches!(path_fixture, PathFixture::MissingSidecar) {
        return source;
    }
    let mut nodes = vec![
        RawTransformPathNodeInputV1 {
            source_node_index: 0,
            parent_source_node_index: None,
            source_name: None,
            projected_bone_index: None,
            kind: RawTransformPathNodeKindV1::ImplicitUfbxRoot,
        },
        RawTransformPathNodeInputV1 {
            source_node_index: 1,
            parent_source_node_index: Some(0),
            source_name: Some("Reference"),
            projected_bone_index: None,
            kind: RawTransformPathNodeKindV1::Source,
        },
        RawTransformPathNodeInputV1 {
            source_node_index: 2,
            parent_source_node_index: Some(1),
            source_name: Some(if matches!(path_fixture, PathFixture::NoMatch) {
                "NotRoot"
            } else {
                "Root"
            }),
            projected_bone_index: if matches!(path_fixture, PathFixture::HelperOnly) {
                None
            } else {
                Some(if matches!(path_fixture, PathFixture::MismatchedBone) {
                    1
                } else {
                    0
                })
            },
            kind: if matches!(path_fixture, PathFixture::HelperOnly) {
                RawTransformPathNodeKindV1::GeometryTransformHelper
            } else {
                RawTransformPathNodeKindV1::Source
            },
        },
    ];
    if matches!(path_fixture, PathFixture::Ambiguous) {
        nodes.extend([
            RawTransformPathNodeInputV1 {
                source_node_index: 3,
                parent_source_node_index: Some(0),
                source_name: Some("Reference"),
                projected_bone_index: None,
                kind: RawTransformPathNodeKindV1::Source,
            },
            RawTransformPathNodeInputV1 {
                source_node_index: 4,
                parent_source_node_index: Some(3),
                source_name: Some("Root"),
                projected_bone_index: Some(1),
                kind: RawTransformPathNodeKindV1::Source,
            },
        ]);
    }
    if matches!(path_fixture, PathFixture::Incomplete) {
        nodes.push(RawTransformPathNodeInputV1 {
            source_node_index: 3,
            parent_source_node_index: Some(0),
            source_name: Some("bad\\segment"),
            projected_bone_index: None,
            kind: RawTransformPathNodeKindV1::Source,
        });
    }
    let inventory = RawTransformPathInventoryV1::from_nodes(primary, format, 2, nodes).unwrap();
    source.with_raw_transform_path_inventory(inventory).unwrap()
}

fn declaration(
    names: &[&str],
    xz: BakeOrExtract,
    y: BakeOrExtract,
    yaw: BakeOrExtract,
) -> animsmith_engine::ResolvedProfileSettingsV2 {
    resolve_static_v2(EngineDeclarationV2 {
        selection: Some(ProfileSelection::new(
            "unity-generic",
            2,
            "6000.3",
            "fbx-model-importer",
        )),
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
            "*".into(),
            BTreeMap::from([
                ("root_position_xz".into(), SettingValueV2::BakeOrExtract(xz)),
                ("root_position_y".into(), SettingValueV2::BakeOrExtract(y)),
                ("root_rotation".into(), SettingValueV2::BakeOrExtract(yaw)),
            ]),
        )]),
    })
    .unwrap()
    .unwrap()
    .resolve_input_with_clips(
        SourceFormatV1::Fbx,
        &names
            .iter()
            .map(|name| (*name).to_owned())
            .collect::<Vec<_>>(),
    )
    .unwrap()
}

fn intent(
    names: &[&str],
    owners: (
        Option<RootMotionProjectOwnerV1>,
        Option<RootMotionProjectOwnerV1>,
        Option<RootMotionProjectOwnerV1>,
    ),
) -> EngineRootMotionProjectIntentV1 {
    EngineRootMotionProjectIntentV1::from_clips_with_root_and_unmapped(
        Some(0),
        names.iter().enumerate().map(|(index, name)| {
            EngineRootMotionClipIntentInputV1::new(
                EngineRootMotionClipMappingStateV1::Observed,
                Some(index as u64),
                Some((*name).to_owned()),
                owners.0,
                owners.1,
                owners.2,
            )
        }),
        std::iter::empty(),
    )
    .unwrap()
}

struct Fixture {
    source: animsmith_core::LoadedSource,
    provenance: animsmith_core::PredictionProvenanceV6,
    roles: ResolvedRoles,
    measurements: Vec<animsmith_core::measure::ClipMeasurements>,
    config: Config,
}

fn fixture(
    witness: &str,
    names: &[&str],
    path: PathFixture,
    owners: (
        Option<RootMotionProjectOwnerV1>,
        Option<RootMotionProjectOwnerV1>,
        Option<RootMotionProjectOwnerV1>,
    ),
    settings: (BakeOrExtract, BakeOrExtract, BakeOrExtract),
) -> Fixture {
    let source = source(witness, names, path);
    let roles =
        ResolvedRoles::from_names(&source.document().skeleton, [(Role::Root, "Root".into())]);
    let config = Config::default();
    let grids = MetricGrids::new(source.document());
    let mut measurements = measure_document_indexed(&grids, &roles, &config);
    for measurement in &mut measurements {
        let trajectory = measurement
            .root_trajectory
            .as_mut()
            .expect("synthetic Root trajectory is measured");
        let translation = trajectory
            .translation
            .as_mut()
            .expect("synthetic Root translation is measured");
        translation.horizontal_displacement_x_m = 0.0;
        translation.horizontal_displacement_z_m = 0.0;
        translation.horizontal_travel_m = 0.0;
        translation.vertical_displacement_m = 0.0;
        translation.vertical_min_displacement_m = 0.0;
        translation.vertical_max_displacement_m = 0.0;
        let yaw = trajectory
            .yaw
            .as_mut()
            .expect("synthetic Root yaw is measured");
        yaw.net_yaw_deg = 0.0;
        yaw.unwrapped_yaw_deg = 0.0;
        yaw.yaw_travel_deg = 0.0;
    }
    let resolved = declaration(names, settings.0, settings.1, settings.2);
    let provenance =
        project_prediction_provenance_v6(&resolved, &source, Vec::new(), intent(names, owners))
            .unwrap();
    Fixture {
        source,
        provenance,
        roles,
        measurements,
        config,
    }
}

fn evaluate(fixture: &Fixture) -> animsmith_core::CheckEvaluation {
    let grids = MetricGrids::new(fixture.source.document());
    let check: Box<dyn Check + '_> = Box::new(
        EngineRootMotionCheck::new(
            &fixture.source,
            Some(&fixture.provenance),
            &fixture.roles,
            &fixture.measurements,
        )
        .unwrap(),
    );
    animsmith_core::evaluate_checks_v2(
        &CheckCtx::new(&grids, &fixture.roles, &fixture.config),
        &[check],
        CheckSelection::All,
    )
    .unwrap()
    .pop()
    .unwrap()
}

fn replace_intent_root(fixture: &mut Fixture, resolved_root_bone_index: Option<u64>) {
    let old = fixture.provenance.root_motion_project_intent();
    let intent = EngineRootMotionProjectIntentV1::new_with_root(
        resolved_root_bone_index,
        old.clips().to_vec(),
        old.clip_coverage(),
        old.observed_source_clips(),
        old.declared_axis_candidates(),
        old.unmapped_declared_axis_candidates(),
    )
    .unwrap();
    fixture.provenance = animsmith_core::PredictionProvenanceV6::new(
        fixture.provenance.base().clone(),
        fixture.provenance.raw_transform_paths().clone(),
        intent,
    )
    .unwrap();
}

fn strict_v17_bytes_with_evaluations(
    fixture: &Fixture,
    evaluations: Vec<CheckEvaluation>,
) -> Vec<u8> {
    let clip_measurements = fixture
        .source
        .document()
        .clips
        .iter()
        .zip(&fixture.measurements)
        .map(|(clip, measurements)| (clip.name.clone(), measurements.clone()))
        .collect::<BTreeMap<_, _>>();
    let measurements = MeasurementContract::new(
        clip_measurements,
        animsmith_core::measure::measure_assets(fixture.source.document()),
    )
    .unwrap();
    let report = LintFileReportV17::new_v6(
        "strict-v17.fbx",
        fixture.source.source_facts().primary_identity().clone(),
        RigInfo::from_resolved(fixture.source.document(), &fixture.roles).unwrap(),
        Some(fixture.provenance.clone()),
        evaluations,
        measurements,
    )
    .unwrap();
    let envelope = LintEnvelopeV17::new(
        ToolInfo::animsmith(ToolSource::new(None, None)),
        vec![report],
    )
    .unwrap();
    serde_json::to_vec(&envelope).unwrap()
}

fn strict_v17_bytes(fixture: &Fixture, evaluation: CheckEvaluation) -> Vec<u8> {
    strict_v17_bytes_with_evaluations(fixture, vec![evaluation])
}

fn budget_filler_evaluation(
    provenance: &animsmith_core::PredictionProvenanceV6,
    count: usize,
) -> CheckEvaluation {
    let facets = (0..count)
        .map(|index| {
            EnginePredictionFacetV4::required_unavailable(
                EvaluationScope::new(EvaluationScopeCode::custom("test:budget-filler"))
                    .subject(format!("filler:{index:04}")),
                EnginePredictionBasisV4::new(Vec::new()).unwrap(),
                vec![PredictionUnavailableReasonV2::RawSourceIncomplete],
            )
            .unwrap()
        })
        .collect();
    let prediction =
        EnginePredictionV4::new(provenance.base().base().identity().clone(), facets).unwrap();
    let prediction = EnginePredictionV6::new(provenance, prediction).unwrap();
    CheckEvaluation::evaluated(
        "test:budget-filler",
        animsmith_core::CheckOutput::from_coverage(Vec::new(), Vec::new(), Vec::new())
            .with_engine_prediction_v6(prediction),
    )
    .unwrap()
}

fn assert_strict_v17_mutation_rejected(
    bytes: &[u8],
    label: &str,
    mutate: impl FnOnce(&mut serde_json::Value),
) {
    let mut wire = serde_json::from_slice(bytes).unwrap();
    mutate(&mut wire);
    let wire = serde_json::to_vec(&wire).unwrap();
    let rejected = match MeasurementReportInput::read_from(&wire[..]) {
        Ok(report) => report.into_files().is_err(),
        Err(_) => true,
    };
    assert!(rejected, "strict V17 readback accepted {label} mutation");
}

fn first_candidate_references(wire: &mut serde_json::Value) -> &mut Vec<serde_json::Value> {
    wire["files"][0]["checks"][0]["prediction"]["prediction"]["facets"][0]["basis"]["references"]
        .as_array_mut()
        .unwrap()
}

#[test]
fn bake_extract_truth_table_ignores_zero_measured_magnitudes() {
    use RootMotionProjectOwnerV1::{Animation, Gameplay};
    let fixture = fixture(
        "truth-table",
        &["idle"],
        PathFixture::Exact,
        (Some(Animation), Some(Gameplay), Some(Gameplay)),
        (
            BakeOrExtract::Extract,
            BakeOrExtract::Bake,
            BakeOrExtract::Extract,
        ),
    );
    let trajectory = fixture.measurements[0].root_trajectory.as_ref().unwrap();
    let translation = trajectory.translation.unwrap();
    let yaw = trajectory.yaw.unwrap();
    assert_eq!(translation.horizontal_travel_m, 0.0);
    assert_eq!(translation.vertical_displacement_m, 0.0);
    assert_eq!(yaw.yaw_travel_deg, 0.0);

    let record = evaluate(&fixture);
    let facets = record.engine_prediction_v6().unwrap().facets();
    assert_eq!(facets.len(), 3);
    let results = facets
        .iter()
        .map(|facet| {
            match facet
                .result()
                .unwrap_or_else(|| panic!("unexpected unavailable facet: {:?}", facet.reasons()))
            {
                EngineMachineResultV1::RootMotionRouting(result) => (
                    result.project_owner,
                    result.importer_disposition,
                    result.compatibility,
                ),
                result => panic!("unexpected result: {result:?}"),
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(
        results,
        vec![
            (
                Animation,
                RootMotionImporterDispositionV1::StoredAsRootMotion,
                RootMotionCompatibilityV1::Compatible,
            ),
            (
                Gameplay,
                RootMotionImporterDispositionV1::BakedIntoPose,
                RootMotionCompatibilityV1::Compatible,
            ),
            (
                Gameplay,
                RootMotionImporterDispositionV1::StoredAsRootMotion,
                RootMotionCompatibilityV1::Conflict,
            ),
        ]
    );
    assert_eq!(record.findings().len(), 1);
    assert_eq!(
        record.findings()[0].severity,
        animsmith_core::Severity::Error
    );
    assert!(record.findings()[0].prediction_scope.is_some());
}

#[test]
fn routing_truth_table_covers_every_owner_disposition_and_axis() {
    assert_eq!(ENGINE_ROOT_MOTION_CHECK_ID, "engine-root-motion");
    for axis in [
        animsmith_core::RootMotionAxisV1::HorizontalXz,
        animsmith_core::RootMotionAxisV1::VerticalY,
        animsmith_core::RootMotionAxisV1::Yaw,
    ] {
        for owner in [
            RootMotionProjectOwnerV1::Gameplay,
            RootMotionProjectOwnerV1::Animation,
        ] {
            for setting in [BakeOrExtract::Bake, BakeOrExtract::Extract] {
                let owners = match axis {
                    animsmith_core::RootMotionAxisV1::HorizontalXz => (Some(owner), None, None),
                    animsmith_core::RootMotionAxisV1::VerticalY => (None, Some(owner), None),
                    animsmith_core::RootMotionAxisV1::Yaw => (None, None, Some(owner)),
                };
                let settings = match axis {
                    animsmith_core::RootMotionAxisV1::HorizontalXz => {
                        (setting, BakeOrExtract::Bake, BakeOrExtract::Bake)
                    }
                    animsmith_core::RootMotionAxisV1::VerticalY => {
                        (BakeOrExtract::Bake, setting, BakeOrExtract::Bake)
                    }
                    animsmith_core::RootMotionAxisV1::Yaw => {
                        (BakeOrExtract::Bake, BakeOrExtract::Bake, setting)
                    }
                };
                let fixture = fixture(
                    &format!("matrix-{axis:?}-{owner:?}-{setting:?}"),
                    &["move"],
                    PathFixture::Exact,
                    owners,
                    settings,
                );
                let record = evaluate(&fixture);
                let facets = record.engine_prediction_v6().unwrap().facets();
                assert_eq!(facets.len(), 1);
                let facet = &facets[0];
                let EngineMachineResultV1::RootMotionRouting(result) = facet.result().unwrap()
                else {
                    panic!("unexpected machine result")
                };
                let disposition = match setting {
                    BakeOrExtract::Bake => RootMotionImporterDispositionV1::BakedIntoPose,
                    BakeOrExtract::Extract => RootMotionImporterDispositionV1::StoredAsRootMotion,
                };
                let compatibility = if matches!(
                    (owner, setting),
                    (RootMotionProjectOwnerV1::Gameplay, BakeOrExtract::Bake)
                        | (RootMotionProjectOwnerV1::Animation, BakeOrExtract::Extract)
                ) {
                    RootMotionCompatibilityV1::Compatible
                } else {
                    RootMotionCompatibilityV1::Conflict
                };
                assert_eq!(result.axis, axis);
                assert_eq!(result.project_owner, owner);
                assert_eq!(result.importer_disposition, disposition);
                assert_eq!(result.compatibility, compatibility);
                if compatibility == RootMotionCompatibilityV1::Conflict {
                    assert_eq!(record.findings().len(), 1);
                    assert_eq!(
                        record.findings()[0].severity,
                        animsmith_core::Severity::Error
                    );
                    assert_eq!(
                        record.findings()[0].prediction_scope.as_ref(),
                        Some(facet.scope())
                    );
                } else {
                    assert!(record.findings().is_empty());
                }
            }
        }
    }
}

#[test]
fn no_provenance_or_declared_owner_is_not_applicable() {
    let fixture = fixture(
        "no-owner",
        &["idle"],
        PathFixture::Exact,
        (None, None, None),
        (
            BakeOrExtract::Bake,
            BakeOrExtract::Bake,
            BakeOrExtract::Bake,
        ),
    );
    let grids = MetricGrids::new(fixture.source.document());
    let ctx = CheckCtx::new(&grids, &fixture.roles, &fixture.config);
    let no_owner = EngineRootMotionCheck::new(
        &fixture.source,
        Some(&fixture.provenance),
        &fixture.roles,
        &fixture.measurements,
    )
    .unwrap();
    assert_eq!(no_owner.applicability(&ctx), Applicability::NotApplicable);
    let no_provenance =
        EngineRootMotionCheck::new(&fixture.source, None, &fixture.roles, &fixture.measurements)
            .unwrap();
    assert_eq!(
        no_provenance.applicability(&ctx),
        Applicability::NotApplicable
    );
}

#[test]
fn non_fbx_non_unity_tuple_is_not_applicable() {
    let source = source_with_format(
        "bevy-glb",
        &["idle"],
        PathFixture::MissingSidecar,
        SourceFormatV1::Glb,
    );
    let roles =
        ResolvedRoles::from_names(&source.document().skeleton, [(Role::Root, "Root".into())]);
    let config = Config::default();
    let grids = MetricGrids::new(source.document());
    let measurements = measure_document_indexed(&grids, &roles, &config);
    let check = EngineRootMotionCheck::new(&source, None, &roles, &measurements).unwrap();
    assert_eq!(
        check.applicability(&CheckCtx::new(&grids, &roles, &config)),
        Applicability::NotApplicable
    );
}

#[test]
fn applicable_root_motion_check_rejects_severity_off() {
    use RootMotionProjectOwnerV1::Gameplay;
    let mut fixture = fixture(
        "off-policy",
        &["walk"],
        PathFixture::Exact,
        (Some(Gameplay), None, None),
        (
            BakeOrExtract::Bake,
            BakeOrExtract::Bake,
            BakeOrExtract::Bake,
        ),
    );
    fixture.config.checks.insert(
        ENGINE_ROOT_MOTION_CHECK_ID.to_owned(),
        CheckSettings {
            severity: Some(animsmith_core::SeveritySetting::Off),
            ..CheckSettings::default()
        },
    );
    let grids = MetricGrids::new(fixture.source.document());
    let ctx = CheckCtx::new(&grids, &fixture.roles, &fixture.config);
    let check: Box<dyn Check + '_> = Box::new(
        EngineRootMotionCheck::new(
            &fixture.source,
            Some(&fixture.provenance),
            &fixture.roles,
            &fixture.measurements,
        )
        .unwrap(),
    );
    assert!(matches!(
        animsmith_core::evaluate_checks_v2(&ctx, &[check], CheckSelection::All),
        Err(animsmith_core::EvaluationError::SeverityOffNotAllowed {
            check_id: ENGINE_ROOT_MOTION_CHECK_ID
        })
    ));
}

#[test]
fn output_v17_candidate_basis_round_trips_through_the_strict_reader() {
    let fixture = fixture(
        "strict-v17",
        &["walk"],
        PathFixture::Exact,
        (Some(RootMotionProjectOwnerV1::Animation), None, None),
        (
            BakeOrExtract::Extract,
            BakeOrExtract::Bake,
            BakeOrExtract::Bake,
        ),
    );
    let bytes = strict_v17_bytes(&fixture, evaluate(&fixture));
    let decoded = MeasurementReportInput::read_from(&bytes[..])
        .unwrap()
        .into_files()
        .unwrap();
    assert_eq!(decoded.len(), 1);

    assert_strict_v17_mutation_rejected(&bytes, "candidate basis value", |wire| {
        first_candidate_references(wire)[0]["reference"]["reference"]["fact_id"] =
            serde_json::json!("accepted_inputs");
    });
    assert_strict_v17_mutation_rejected(&bytes, "missing candidate basis reference", |wire| {
        first_candidate_references(wire).remove(0);
    });
    assert_strict_v17_mutation_rejected(&bytes, "extra candidate basis reference", |wire| {
        let extra = first_candidate_references(wire)[0].clone();
        first_candidate_references(wire).push(extra);
    });
    assert_strict_v17_mutation_rejected(&bytes, "candidate result", |wire| {
        wire["files"][0]["checks"][0]["prediction"]["prediction"]["facets"][0]["result"]["result"]
            ["compatibility"] = serde_json::json!("conflict");
    });
    assert_strict_v17_mutation_rejected(&bytes, "candidate state and reason", |wire| {
        let facet = &mut wire["files"][0]["checks"][0]["prediction"]["prediction"]["facets"][0];
        facet["state"] = serde_json::json!("required_prediction_unavailable");
        facet["result"] = serde_json::Value::Null;
        facet["reasons"] = serde_json::json!(["measurement_unavailable"]);
    });
    assert_strict_v17_mutation_rejected(&bytes, "candidate scope", |wire| {
        wire["files"][0]["checks"][0]["prediction"]["prediction"]["facets"][0]["scope"]["subject"] =
            serde_json::json!("source_clip:00000000000000000001:walk:horizontal_xz");
    });
    assert_strict_v17_mutation_rejected(&bytes, "frozen profile identity", |wire| {
        wire["files"][0]["prediction_provenance"]["base"]["base"]["profile"]["identity"]["sha256"] =
            serde_json::json!("0000000000000000000000000000000000000000000000000000000000000000");
    });
    assert_strict_v17_mutation_rejected(&bytes, "frozen profile source metadata", |wire| {
        wire["files"][0]["prediction_provenance"]["base"]["base"]["profile"]["primary_sources"]
            [0]["verified_on"] = serde_json::json!("2026-08-25");
    });
    assert_strict_v17_mutation_rejected(&bytes, "resolved Root index", |wire| {
        wire["files"][0]["prediction_provenance"]["root_motion_project_intent"]["resolved_root_bone_index"] =
            serde_json::json!(1);
    });
    assert_strict_v17_mutation_rejected(&bytes, "clip mapping", |wire| {
        wire["files"][0]["prediction_provenance"]["root_motion_project_intent"]["clips"][0]["normalized_clip_index"] =
            serde_json::json!(1);
    });
    assert_strict_v17_mutation_rejected(&bytes, "resolved clip setting", |wire| {
        wire["files"][0]["prediction_provenance"]["base"]["base"]["settings"]["clips"][0]["settings"]
            [0]["value"]["bake_or_extract"] = serde_json::json!("bake");
    });
    assert_strict_v17_mutation_rejected(&bytes, "resolved transform path", |wire| {
        wire["files"][0]["prediction_provenance"]["raw_transform_paths"]["rows"][2]["addressable_path"] =
            serde_json::json!("Reference/Other");
    });
    assert_strict_v17_mutation_rejected(&bytes, "measurement availability", |wire| {
        wire["files"][0]["measurements"]["clips"]["walk"]["root_trajectory"]["translation_availability"] =
            serde_json::json!("unavailable");
    });
}

#[test]
fn output_v17_conflict_atomic_and_budget_facets_reject_strict_mutations() {
    let conflict = fixture(
        "strict-conflict",
        &["walk"],
        PathFixture::Exact,
        (Some(RootMotionProjectOwnerV1::Gameplay), None, None),
        (
            BakeOrExtract::Extract,
            BakeOrExtract::Bake,
            BakeOrExtract::Bake,
        ),
    );
    let conflict_bytes = strict_v17_bytes(&conflict, evaluate(&conflict));
    assert_strict_v17_mutation_rejected(&conflict_bytes, "conflict finding message", |wire| {
        wire["files"][0]["checks"][0]["findings"][0]["message"] =
            serde_json::json!("mutated conflict");
    });
    assert_strict_v17_mutation_rejected(&conflict_bytes, "conflict finding scope", |wire| {
        wire["files"][0]["checks"][0]["findings"][0]["prediction_scope"]["subject"] =
            serde_json::json!("source_clip:00000000000000000001:walk:horizontal_xz");
    });

    let atomic = fixture(
        "strict-atomic",
        &["walk"],
        PathFixture::Incomplete,
        (Some(RootMotionProjectOwnerV1::Gameplay), None, None),
        (
            BakeOrExtract::Bake,
            BakeOrExtract::Bake,
            BakeOrExtract::Bake,
        ),
    );
    let atomic_bytes = strict_v17_bytes(&atomic, evaluate(&atomic));
    assert_strict_v17_mutation_rejected(&atomic_bytes, "atomic basis", |wire| {
        wire["files"][0]["checks"][0]["prediction"]["prediction"]["facets"][0]["basis"]
            ["references"]
            .as_array_mut()
            .unwrap()
            .remove(0);
    });

    let names = (0..12)
        .map(|index| format!("clip-{index}"))
        .collect::<Vec<_>>();
    let borrowed = names.iter().map(String::as_str).collect::<Vec<_>>();
    let budget = fixture(
        "strict-budget",
        &borrowed,
        PathFixture::Exact,
        (Some(RootMotionProjectOwnerV1::Gameplay), None, None),
        (
            BakeOrExtract::Bake,
            BakeOrExtract::Bake,
            BakeOrExtract::Bake,
        ),
    );
    let grids = MetricGrids::new(budget.source.document());
    let ctx = CheckCtx::new(&grids, &budget.roles, &budget.config);
    let check = EngineRootMotionCheck::new(
        &budget.source,
        Some(&budget.provenance),
        &budget.roles,
        &budget.measurements,
    )
    .unwrap();
    let demands = vec![
        PredictionRuleDemandV2::new(
            "prior",
            PredictionFacetDemandV2::Exact(PREDICTION_V2_MAX_CANDIDATE_FACETS_PER_RULE - 2),
        )
        .unwrap(),
        PredictionRuleDemandV2::new(
            ENGINE_ROOT_MOTION_CHECK_ID,
            check.prediction_facet_demand_v2(&ctx),
        )
        .unwrap(),
    ];
    let allocation = allocate_prediction_facets_v2(&demands)
        .unwrap()
        .into_iter()
        .find(|allocation| allocation.rule_id() == ENGINE_ROOT_MOTION_CHECK_ID)
        .unwrap();
    let budget_evaluation = CheckEvaluation::evaluated(
        ENGINE_ROOT_MOTION_CHECK_ID,
        check.evaluate_with_prediction_allocation_v2(&ctx, allocation),
    )
    .unwrap();
    let budget_bytes = strict_v17_bytes_with_evaluations(
        &budget,
        vec![
            budget_filler_evaluation(
                &budget.provenance,
                PREDICTION_V2_MAX_CANDIDATE_FACETS_PER_RULE - 2,
            ),
            budget_evaluation,
        ],
    );
    assert_strict_v17_mutation_rejected(&budget_bytes, "budget basis", |wire| {
        let check = wire["files"][0]["checks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|check| check["check_id"] == ENGINE_ROOT_MOTION_CHECK_ID)
            .unwrap();
        let facets = check["prediction"]["prediction"]["facets"]
            .as_array_mut()
            .unwrap();
        facets.last_mut().unwrap()["basis"]["references"]
            .as_array_mut()
            .unwrap()
            .remove(0);
    });
}

#[test]
fn path_failures_and_explicit_root_identity_fail_closed() {
    use RootMotionProjectOwnerV1::Gameplay;
    for (case, expected_reason) in [
        (
            PathFixture::NoMatch,
            PredictionUnavailableReasonV2::SourceSelectorNoMatch,
        ),
        (
            PathFixture::HelperOnly,
            PredictionUnavailableReasonV2::SourceSelectorNoMatch,
        ),
        (
            PathFixture::Ambiguous,
            PredictionUnavailableReasonV2::SourceSelectorAmbiguous,
        ),
        (
            PathFixture::MismatchedBone,
            PredictionUnavailableReasonV2::custom("animsmith:root_motion_source_not_explicit_root")
                .unwrap(),
        ),
    ] {
        let fixture = fixture(
            "path-failure",
            &["walk"],
            case,
            (Some(Gameplay), None, None),
            (
                BakeOrExtract::Bake,
                BakeOrExtract::Bake,
                BakeOrExtract::Bake,
            ),
        );
        let record = evaluate(&fixture);
        let facet = &record.engine_prediction_v6().unwrap().facets()[0];
        assert_eq!(
            facet.state(),
            EnginePredictionFacetStateV1::RequiredPredictionUnavailable
        );
        assert_eq!(facet.reasons(), &[expected_reason]);
        assert!(record.findings().is_empty());
    }

    let mut hips_only = fixture(
        "hips-fallback",
        &["walk"],
        PathFixture::Exact,
        (Some(Gameplay), None, None),
        (
            BakeOrExtract::Bake,
            BakeOrExtract::Bake,
            BakeOrExtract::Bake,
        ),
    );
    hips_only.roles = ResolvedRoles::from_names(
        &hips_only.source.document().skeleton,
        [(Role::Hips, "Root".into())],
    );
    let grids = MetricGrids::new(hips_only.source.document());
    hips_only.measurements = measure_document_indexed(&grids, &hips_only.roles, &hips_only.config);
    replace_intent_root(&mut hips_only, None);
    let hips_record = evaluate(&hips_only);
    let facet = &hips_record.engine_prediction_v6().unwrap().facets()[0];
    assert_eq!(
        facet.reasons()[0].as_str(),
        "animsmith:root_motion_source_not_explicit_root"
    );

    let mut missing_root = fixture(
        "missing-root",
        &["walk"],
        PathFixture::Exact,
        (Some(Gameplay), None, None),
        (
            BakeOrExtract::Bake,
            BakeOrExtract::Bake,
            BakeOrExtract::Bake,
        ),
    );
    missing_root.roles = ResolvedRoles::default();
    let grids = MetricGrids::new(missing_root.source.document());
    missing_root.measurements =
        measure_document_indexed(&grids, &missing_root.roles, &missing_root.config);
    replace_intent_root(&mut missing_root, None);
    let missing_record = evaluate(&missing_root);
    assert_eq!(
        missing_record.engine_prediction_v6().unwrap().facets()[0].reasons()[0].as_str(),
        "animsmith:root_motion_source_not_explicit_root"
    );
}

#[test]
fn incomplete_path_inventory_is_one_atomic_summary_without_prefix() {
    let fixture = fixture(
        "incomplete-path",
        &["walk", "run"],
        PathFixture::Incomplete,
        (
            Some(RootMotionProjectOwnerV1::Gameplay),
            Some(RootMotionProjectOwnerV1::Gameplay),
            None,
        ),
        (
            BakeOrExtract::Bake,
            BakeOrExtract::Bake,
            BakeOrExtract::Bake,
        ),
    );
    let record = evaluate(&fixture);
    let facets = record.engine_prediction_v6().unwrap().facets();
    assert_eq!(facets.len(), 1);
    assert_eq!(
        facets[0].scope().code.as_str(),
        "engine-root-motion:inventory"
    );
    assert_eq!(
        facets[0].reasons(),
        &[PredictionUnavailableReasonV2::RawSourceIncomplete]
    );
}

#[test]
fn partial_settings_are_one_atomic_summary_without_prefix() {
    let mut fixture = fixture(
        "partial-settings",
        &["walk"],
        PathFixture::Exact,
        (Some(RootMotionProjectOwnerV1::Gameplay), None, None),
        (
            BakeOrExtract::Bake,
            BakeOrExtract::Bake,
            BakeOrExtract::Bake,
        ),
    );
    let mut names = (0..=animsmith_engine::RESOLVED_ENGINE_SETTINGS_V2_MAX_CLIPS)
        .map(|index| format!("clip-{index:04}"))
        .collect::<Vec<_>>();
    names[0] = "walk".to_owned();
    let borrowed = names.iter().map(String::as_str).collect::<Vec<_>>();
    let resolved = declaration(
        &borrowed,
        BakeOrExtract::Bake,
        BakeOrExtract::Bake,
        BakeOrExtract::Bake,
    );
    fixture.provenance = project_prediction_provenance_v6(
        &resolved,
        &fixture.source,
        Vec::new(),
        intent(
            &["walk"],
            (Some(RootMotionProjectOwnerV1::Gameplay), None, None),
        ),
    )
    .unwrap();

    let record = evaluate(&fixture);
    let facets = record.engine_prediction_v6().unwrap().facets();
    assert_eq!(facets.len(), 1);
    assert_eq!(
        facets[0].scope().code.as_str(),
        "engine-root-motion:inventory"
    );
    assert_eq!(
        facets[0].reasons(),
        &[PredictionUnavailableReasonV2::ResolvedSettingsOverflow]
    );
}

#[test]
fn unmapped_declared_work_is_one_atomic_summary_without_prefix() {
    let mut fixture = fixture(
        "unmapped-intent",
        &["idle"],
        PathFixture::Exact,
        (None, None, None),
        (
            BakeOrExtract::Extract,
            BakeOrExtract::Extract,
            BakeOrExtract::Extract,
        ),
    );
    let intent = EngineRootMotionProjectIntentV1::from_clips_with_root_and_unmapped(
        Some(0),
        [EngineRootMotionClipIntentInputV1::new(
            EngineRootMotionClipMappingStateV1::Observed,
            Some(0),
            Some("idle".to_owned()),
            None,
            None,
            None,
        )],
        [[Some(RootMotionProjectOwnerV1::Gameplay), None, None]],
    )
    .unwrap();
    fixture.provenance = animsmith_core::PredictionProvenanceV6::new(
        fixture.provenance.base().clone(),
        fixture.provenance.raw_transform_paths().clone(),
        intent,
    )
    .unwrap();

    let record = evaluate(&fixture);
    let facets = record.engine_prediction_v6().unwrap().facets();
    assert_eq!(facets.len(), 1);
    assert_eq!(
        facets[0].state(),
        EnginePredictionFacetStateV1::RequiredPredictionUnavailable
    );
    assert_eq!(
        facets[0].reasons(),
        &[PredictionUnavailableReasonV2::ProjectIntentUnavailable]
    );
    assert_eq!(
        facets[0].scope().code.as_str(),
        "engine-root-motion:inventory"
    );
}

#[test]
fn intent_work_overflow_is_one_atomic_summary_without_prefix() {
    let mut fixture = fixture(
        "intent-overflow",
        &["idle"],
        PathFixture::Exact,
        (None, None, None),
        (
            BakeOrExtract::Extract,
            BakeOrExtract::Extract,
            BakeOrExtract::Extract,
        ),
    );
    let intent = EngineRootMotionProjectIntentV1::from_clips_with_root_and_unmapped(
        Some(0),
        [EngineRootMotionClipIntentInputV1::new(
            EngineRootMotionClipMappingStateV1::Observed,
            Some(0),
            Some("idle".to_owned()),
            None,
            None,
            None,
        )],
        std::iter::repeat_n(
            [Some(RootMotionProjectOwnerV1::Gameplay), None, None],
            PREDICTION_V2_MAX_CANDIDATE_FACETS_PER_RULE + 1,
        ),
    )
    .unwrap();
    fixture.provenance = animsmith_core::PredictionProvenanceV6::new(
        fixture.provenance.base().clone(),
        fixture.provenance.raw_transform_paths().clone(),
        intent,
    )
    .unwrap();

    let record = evaluate(&fixture);
    let facets = record.engine_prediction_v6().unwrap().facets();
    assert_eq!(facets.len(), 1);
    assert_eq!(
        facets[0].reasons(),
        &[
            PredictionUnavailableReasonV2::custom(
                "animsmith:root_motion_intent_work_budget_exceeded"
            )
            .unwrap(),
            PredictionUnavailableReasonV2::ProjectIntentUnavailable,
        ]
    );
    assert_eq!(
        facets[0].scope().code.as_str(),
        "engine-root-motion:inventory"
    );
}

#[test]
fn missing_loader_sidecar_is_typed_unavailable_not_constructor_failure() {
    let fixture = fixture(
        "missing-sidecar",
        &["walk"],
        PathFixture::MissingSidecar,
        (Some(RootMotionProjectOwnerV1::Gameplay), None, None),
        (
            BakeOrExtract::Bake,
            BakeOrExtract::Bake,
            BakeOrExtract::Bake,
        ),
    );
    let record = evaluate(&fixture);
    assert_eq!(
        record.engine_prediction_v6().unwrap().facets()[0].reasons(),
        &[PredictionUnavailableReasonV2::RawSourceIncomplete]
    );
}

#[test]
fn duplicate_names_make_only_their_declared_facets_unavailable() {
    let fixture = fixture(
        "duplicates",
        &["walk", "walk", "idle"],
        PathFixture::Exact,
        (Some(RootMotionProjectOwnerV1::Gameplay), None, None),
        (
            BakeOrExtract::Bake,
            BakeOrExtract::Bake,
            BakeOrExtract::Bake,
        ),
    );
    let record = evaluate(&fixture);
    let facets = record.engine_prediction_v6().unwrap().facets();
    assert_eq!(facets.len(), 3);
    assert_eq!(
        facets[0].reasons(),
        &[PredictionUnavailableReasonV2::MeasurementUnavailable]
    );
    assert_eq!(
        facets[1].reasons(),
        &[PredictionUnavailableReasonV2::MeasurementUnavailable]
    );
    assert_eq!(facets[2].state(), EnginePredictionFacetStateV1::Available);
}

#[test]
fn measurement_axis_availability_is_independent() {
    let mut yaw_unavailable = fixture(
        "measurements",
        &["walk"],
        PathFixture::Exact,
        (
            Some(RootMotionProjectOwnerV1::Gameplay),
            None,
            Some(RootMotionProjectOwnerV1::Gameplay),
        ),
        (
            BakeOrExtract::Bake,
            BakeOrExtract::Bake,
            BakeOrExtract::Bake,
        ),
    );
    let trajectory = yaw_unavailable.measurements[0]
        .root_trajectory
        .as_mut()
        .unwrap();
    trajectory.yaw = None;
    trajectory.yaw_availability = MeasurementAvailability::Unavailable;
    let record = evaluate(&yaw_unavailable);
    let facets = record.engine_prediction_v6().unwrap().facets();
    assert_eq!(facets[0].state(), EnginePredictionFacetStateV1::Available);
    assert_eq!(
        facets[1].reasons(),
        &[PredictionUnavailableReasonV2::MeasurementUnavailable]
    );

    let mut translation_unavailable = fixture(
        "measurements-reverse",
        &["walk"],
        PathFixture::Exact,
        (
            Some(RootMotionProjectOwnerV1::Gameplay),
            None,
            Some(RootMotionProjectOwnerV1::Gameplay),
        ),
        (
            BakeOrExtract::Bake,
            BakeOrExtract::Bake,
            BakeOrExtract::Bake,
        ),
    );
    let trajectory = translation_unavailable.measurements[0]
        .root_trajectory
        .as_mut()
        .unwrap();
    trajectory.translation = None;
    trajectory.translation_availability = MeasurementAvailability::Unavailable;
    let record = evaluate(&translation_unavailable);
    let facets = record.engine_prediction_v6().unwrap().facets();
    assert_eq!(
        facets[0].reasons(),
        &[PredictionUnavailableReasonV2::MeasurementUnavailable]
    );
    assert_eq!(facets[1].state(), EnginePredictionFacetStateV1::Available);

    let mut trajectory_unavailable = fixture(
        "trajectory-availability",
        &["walk"],
        PathFixture::Exact,
        (Some(RootMotionProjectOwnerV1::Gameplay), None, None),
        (
            BakeOrExtract::Bake,
            BakeOrExtract::Bake,
            BakeOrExtract::Bake,
        ),
    );
    trajectory_unavailable.measurements[0].root_trajectory_availability =
        MeasurementAvailability::Unavailable;
    let record = evaluate(&trajectory_unavailable);
    assert_eq!(
        record.engine_prediction_v6().unwrap().facets()[0].reasons(),
        &[PredictionUnavailableReasonV2::MeasurementUnavailable]
    );
}

#[test]
fn shared_allocator_emits_canonical_prefix_and_one_budget_summary() {
    let names = (0..12)
        .map(|index| format!("clip-{index}"))
        .collect::<Vec<_>>();
    let borrowed = names.iter().map(String::as_str).collect::<Vec<_>>();
    let fixture = fixture(
        "allocator",
        &borrowed,
        PathFixture::Exact,
        (Some(RootMotionProjectOwnerV1::Gameplay), None, None),
        (
            BakeOrExtract::Bake,
            BakeOrExtract::Bake,
            BakeOrExtract::Bake,
        ),
    );
    let grids = MetricGrids::new(fixture.source.document());
    let ctx = CheckCtx::new(&grids, &fixture.roles, &fixture.config);
    let check = EngineRootMotionCheck::new(
        &fixture.source,
        Some(&fixture.provenance),
        &fixture.roles,
        &fixture.measurements,
    )
    .unwrap();
    assert_eq!(
        check.prediction_facet_demand_v2(&ctx),
        PredictionFacetDemandV2::Exact(12)
    );
    let demands = vec![
        PredictionRuleDemandV2::new(
            "prior",
            PredictionFacetDemandV2::Exact(PREDICTION_V2_MAX_CANDIDATE_FACETS_PER_RULE - 2),
        )
        .unwrap(),
        PredictionRuleDemandV2::new(
            ENGINE_ROOT_MOTION_CHECK_ID,
            check.prediction_facet_demand_v2(&ctx),
        )
        .unwrap(),
    ];
    let allocations = allocate_prediction_facets_v2(&demands).unwrap();
    let allocation = allocations
        .iter()
        .find(|allocation| allocation.rule_id() == ENGINE_ROOT_MOTION_CHECK_ID)
        .copied()
        .unwrap();
    assert_eq!(allocation.candidate_capacity(), 1);
    assert!(allocation.summary_required());
    let output = check.evaluate_with_prediction_allocation_v2(&ctx, allocation);
    let facets = output.engine_prediction_v6().unwrap().facets();
    assert_eq!(facets.len(), 2);
    assert!(
        facets[0]
            .scope()
            .subject
            .as_deref()
            .unwrap()
            .starts_with("source_clip:00000000000000000000:")
    );
    assert_eq!(
        facets[1].reasons(),
        &[PredictionUnavailableReasonV2::FacetBudgetExceeded]
    );
}

#[test]
fn constructor_rejects_measurement_and_same_load_mismatches() {
    let fixture = fixture(
        "lifecycle-a",
        &["walk"],
        PathFixture::Exact,
        (Some(RootMotionProjectOwnerV1::Gameplay), None, None),
        (
            BakeOrExtract::Bake,
            BakeOrExtract::Bake,
            BakeOrExtract::Bake,
        ),
    );
    assert!(matches!(
        EngineRootMotionCheck::new(
            &fixture.source,
            Some(&fixture.provenance),
            &fixture.roles,
            &[],
        ),
        Err(PredictionRuleError::SourceProvenanceMismatch)
    ));
    let other = source("lifecycle-b", &["walk"], PathFixture::Exact);
    assert!(matches!(
        EngineRootMotionCheck::new(
            &other,
            Some(&fixture.provenance),
            &fixture.roles,
            &fixture.measurements,
        ),
        Err(PredictionRuleError::SourceProvenanceMismatch)
    ));
    assert!(matches!(
        EngineRootMotionCheck::new(
            &fixture.source,
            Some(&fixture.provenance),
            &ResolvedRoles::default(),
            &fixture.measurements,
        ),
        Err(PredictionRuleError::SourceProvenanceMismatch)
    ));
}

#[test]
fn exact_profile_identity_is_pinned_for_strict_root_motion_readback() {
    let profile = animsmith_engine::lookup_profile_v2(&ProfileSelection::new(
        "unity-generic",
        2,
        "6000.3",
        "fbx-model-importer",
    ))
    .unwrap();
    let projected = animsmith_engine::project_engine_profile_v2(profile).unwrap();
    assert_eq!(
        projected.facts_identity().sha256(),
        "740e1c324a7a5b13efa2d9980fe255a6245d858adec55fb3387614a3ff45274c"
    );
    assert_eq!(projected.facts_identity().bytes(), 2_776);
}
