use animsmith_core::{
    Bone, Clip, DependencyClosureBuilderV1, DependencyClosureV1, Document,
    DocumentTransitionFamilyMemberV1, DocumentTransitionFamilyV1, InputIdentity, Interpolation,
    Property, Skeleton, SourceSetCoverageV1, Track, TrackValues, Transform,
    TransitionFamilyBoundaryV1, TransitionFamilyDeclarationInputV1, TransitionFamilyDeclarationV1,
    TransitionFamilyTolerancesV1, TransitionPoseEvaluationControlError, TransitionPoseEvaluationV1,
    evaluate_document_transition_poses_v1 as evaluate_document_transition_poses_with_closure_v1,
    glam,
};
use serde_json::Value;
use std::path::Path;

const SCHEMA: &str = include_str!("../schemas/transition-pose-evaluation-v1.schema.json");

fn validator() -> jsonschema::Validator {
    jsonschema::validator_for(&serde_json::from_str::<Value>(SCHEMA).unwrap()).unwrap()
}

fn document(delta: Option<f32>) -> Document {
    let mut run = Clip {
        name: "Run".into(),
        duration_s: 1.0,
        tracks: Vec::new(),
    };
    if let Some(delta) = delta {
        run.tracks.push(Track {
            bone: 0,
            property: Property::Translation,
            interpolation: Interpolation::Linear,
            times: vec![0.0, 1.0],
            values: TrackValues::Vec3s(vec![glam::Vec3::splat(delta); 2]),
        });
    }
    Document {
        skeleton: Skeleton {
            bones: vec![Bone {
                name: "root".into(),
                parent: None,
                rest: Transform::IDENTITY,
                inverse_bind: None,
            }],
        },
        clips: vec![
            Clip {
                name: "Walk".into(),
                duration_s: 1.0,
                tracks: Vec::new(),
            },
            run,
        ],
        ..Document::default()
    }
}

fn declaration(tolerance: f64, time: f64) -> TransitionFamilyDeclarationInputV1 {
    let family = DocumentTransitionFamilyV1::new(
        "walk_to_run".into(),
        TransitionFamilyBoundaryV1::Entry,
        TransitionFamilyTolerancesV1::new(tolerance, 180.0, time).unwrap(),
        vec![
            DocumentTransitionFamilyMemberV1::new(0, "Walk".into()).unwrap(),
            DocumentTransitionFamilyMemberV1::new(1, "Run".into()).unwrap(),
        ],
    )
    .unwrap();
    TransitionFamilyDeclarationInputV1::new(
        TransitionFamilyDeclarationV1::document(vec![family]).unwrap(),
        b"declaration",
    )
    .unwrap()
}

fn wire(value: &impl serde::Serialize) -> Value {
    serde_json::to_value(value).unwrap()
}

fn evaluate_document_transition_poses_v1(
    declaration: &TransitionFamilyDeclarationInputV1,
    subject_input: InputIdentity,
    document: &Document,
) -> Result<TransitionPoseEvaluationV1, TransitionPoseEvaluationControlError> {
    let closure =
        DependencyClosureBuilderV1::new(subject_input, SourceSetCoverageV1::complete(), 0)
            .finish()
            .unwrap();
    evaluate_document_transition_poses_with_closure_v1(declaration, &closure, document)
}

#[test]
fn schema_accepts_all_producer_lifecycle_rows() {
    let source = InputIdentity::from_bytes(b"document");
    let pass = evaluate_document_transition_poses_v1(
        &declaration(2.0, 0.0),
        source.clone(),
        &document(Some(1.0)),
    )
    .unwrap();
    let finding = evaluate_document_transition_poses_v1(
        &declaration(0.0, 0.0),
        source.clone(),
        &document(Some(1.0)),
    )
    .unwrap();
    let incomplete = evaluate_document_transition_poses_v1(
        &declaration(0.0, 0.1),
        source.clone(),
        &document(None),
    )
    .unwrap();
    let no_config = evaluate_document_transition_poses_v1(
        &TransitionFamilyDeclarationInputV1::new(
            TransitionFamilyDeclarationV1::document(Vec::new()).unwrap(),
            b"empty",
        )
        .unwrap(),
        source,
        &document(None),
    )
    .unwrap();
    let unavailable = DependencyClosureV1::unavailable(InputIdentity::from_bytes(b"document"));
    let dependency_incomplete = evaluate_document_transition_poses_with_closure_v1(
        &declaration(0.0, 0.0),
        &unavailable,
        &document(Some(1.0)),
    )
    .unwrap();
    let validator = validator();
    for output in [
        &pass,
        &finding,
        &incomplete,
        &no_config,
        &dependency_incomplete,
    ] {
        assert!(validator.is_valid(&wire(output)));
    }
}

#[test]
fn schema_rejects_impossible_top_level_and_family_lifecycle_mutations() {
    let validator = validator();
    let source = InputIdentity::from_bytes(b"document");
    let pass = wire(
        &evaluate_document_transition_poses_v1(
            &declaration(2.0, 0.0),
            source.clone(),
            &document(Some(1.0)),
        )
        .unwrap(),
    );
    let incomplete = wire(
        &evaluate_document_transition_poses_v1(&declaration(0.0, 0.1), source, &document(None))
            .unwrap(),
    );
    let finding = wire(
        &evaluate_document_transition_poses_v1(
            &declaration(0.0, 0.0),
            InputIdentity::from_bytes(b"document"),
            &document(Some(1.0)),
        )
        .unwrap(),
    );

    let mut normal_pass_with_reason = pass.clone();
    normal_pass_with_reason["reason"] = Value::String("no_configured_families".into());
    assert!(!validator.is_valid(&normal_pass_with_reason));

    let mut pass_without_member_closure = pass.clone();
    pass_without_member_closure["families"][0]["members"][0]
        .as_object_mut()
        .unwrap()
        .remove("source_dependency_closure_identity");
    assert!(!validator.is_valid(&pass_without_member_closure));

    let mut pass_with_null_source = pass.clone();
    pass_with_null_source["families"][0]["members"][0]["source_input"] = Value::Null;
    assert!(!validator.is_valid(&pass_with_null_source));

    let mut incomplete_top_reason = incomplete.clone();
    incomplete_top_reason["reason"] = Value::String("unsupported_sampling".into());
    assert!(!validator.is_valid(&incomplete_top_reason));

    let mut incomplete_empty = incomplete.clone();
    incomplete_empty["families"] = Value::Array(Vec::new());
    assert!(!validator.is_valid(&incomplete_empty));

    let mut family_complete_reason = pass.clone();
    family_complete_reason["families"][0]["reason"] = Value::String("unsupported_sampling".into());
    assert!(!validator.is_valid(&family_complete_reason));

    let mut family_no_config_reason = incomplete.clone();
    family_no_config_reason["families"][0]["reason"] =
        Value::String("no_configured_families".into());
    assert!(!validator.is_valid(&family_no_config_reason));

    let mut top_pass_with_incomplete = pass.clone();
    top_pass_with_incomplete["families"][0]["status"] = Value::String("incomplete".into());
    top_pass_with_incomplete["families"][0]["decision"] = Value::String("not_evaluated".into());
    top_pass_with_incomplete["families"][0]["reason"] =
        Value::String("unsupported_sampling".into());
    top_pass_with_incomplete["families"][0]["pairs"] = Value::Array(Vec::new());
    assert!(!validator.is_valid(&top_pass_with_incomplete));

    let mut top_finding_without_finding = pass.clone();
    top_finding_without_finding["decision"] = Value::String("finding".into());
    assert!(!validator.is_valid(&top_finding_without_finding));

    let mut top_incomplete_without_incomplete = incomplete.clone();
    top_incomplete_without_incomplete["families"][0]["status"] = Value::String("complete".into());
    top_incomplete_without_incomplete["families"][0]["decision"] = Value::String("pass".into());
    top_incomplete_without_incomplete["families"][0]
        .as_object_mut()
        .unwrap()
        .remove("reason");
    top_incomplete_without_incomplete["families"][0]["pairs"] =
        pass["families"][0]["pairs"].clone();
    top_incomplete_without_incomplete["families"][0]["skeleton_basis_input"] =
        pass["families"][0]["skeleton_basis_input"].clone();
    assert!(!validator.is_valid(&top_incomplete_without_incomplete));

    let mut incomplete_with_pairs = incomplete.clone();
    incomplete_with_pairs["families"][0]["pairs"] = pass["families"][0]["pairs"].clone();
    assert!(!validator.is_valid(&incomplete_with_pairs));

    let mut incomplete_without_member_closure = incomplete.clone();
    incomplete_without_member_closure["families"][0]["members"][0]
        .as_object_mut()
        .unwrap()
        .remove("source_dependency_closure_identity");
    assert!(!validator.is_valid(&incomplete_without_member_closure));

    let mut pass_with_offender = pass.clone();
    pass_with_offender["families"][0]["pairs"][0]["translation_offenders"] =
        finding["families"][0]["pairs"][0]["translation_offenders"].clone();
    assert!(!validator.is_valid(&pass_with_offender));

    let mut finding_without_offender = finding;
    for pair in finding_without_offender["families"][0]["pairs"]
        .as_array_mut()
        .unwrap()
    {
        pair["translation_offenders"] = Value::Array(Vec::new());
        pair["rotation_offenders"] = Value::Array(Vec::new());
    }
    assert!(!validator.is_valid(&finding_without_offender));
}

#[test]
fn schema_requires_exactly_two_pair_member_indices() {
    let validator = validator();
    let exact = wire(
        &evaluate_document_transition_poses_v1(
            &declaration(2.0, 0.0),
            InputIdentity::from_bytes(b"document"),
            &document(Some(1.0)),
        )
        .unwrap(),
    );
    assert!(validator.is_valid(&exact));

    for indices in [
        Value::Array(Vec::new()),
        Value::Array(vec![Value::from(0)]),
        Value::Array(vec![Value::from(0), Value::from(1), Value::from(2)]),
    ] {
        let mut mutated = exact.clone();
        mutated["families"][0]["pairs"][0]["member_indices"] = indices;
        assert!(!validator.is_valid(&mutated));
    }
}

#[test]
fn schema_conditions_closure_authority_for_document_and_collection_rows() {
    let validator = validator();
    let unavailable = DependencyClosureV1::unavailable(InputIdentity::from_bytes(b"document"));
    let dependency_incomplete = wire(
        &evaluate_document_transition_poses_with_closure_v1(
            &declaration(0.0, 0.0),
            &unavailable,
            &document(Some(1.0)),
        )
        .unwrap(),
    );
    assert!(validator.is_valid(&dependency_incomplete));

    // A collection family may retain closure identities for members that did
    // load while another member makes the whole family incomplete.
    let complete = wire(
        &evaluate_document_transition_poses_v1(
            &declaration(2.0, 0.0),
            InputIdentity::from_bytes(b"document"),
            &document(Some(1.0)),
        )
        .unwrap(),
    );
    assert!(
        complete
            .get("subject_dependency_closure_identity")
            .is_some()
    );
    let mut collection_shaped_complete = complete.clone();
    collection_shaped_complete
        .as_object_mut()
        .unwrap()
        .remove("subject_dependency_closure_identity");
    assert!(validator.is_valid(&collection_shaped_complete));

    let closure_identity =
        complete["families"][0]["members"][0]["source_dependency_closure_identity"].clone();
    let mut mixed = dependency_incomplete.clone();
    mixed["families"][0]["members"][0]["source_dependency_closure_identity"] =
        closure_identity.clone();
    mixed["families"][0]["members"][1]["source_input"] = Value::Null;
    assert!(validator.is_valid(&mixed));

    let mut no_missing_closure = mixed;
    no_missing_closure["families"][0]["members"][1]["source_dependency_closure_identity"] =
        closure_identity;
    assert!(!validator.is_valid(&no_missing_closure));

    let mut unavailable_member = complete.clone();
    unavailable_member["status"] = Value::String("incomplete".into());
    unavailable_member["decision"] = Value::String("not_evaluated".into());
    unavailable_member["families"][0]["status"] = Value::String("incomplete".into());
    unavailable_member["families"][0]["decision"] = Value::String("not_evaluated".into());
    unavailable_member["families"][0]["reason"] = Value::String("member_unavailable".into());
    unavailable_member["families"][0]["pairs"] = Value::Array(Vec::new());
    unavailable_member["families"][0]["members"][1]["source_input"] = Value::Null;
    unavailable_member["families"][0]["members"][1]
        .as_object_mut()
        .unwrap()
        .remove("source_dependency_closure_identity");
    assert!(validator.is_valid(&unavailable_member));

    let empty = TransitionFamilyDeclarationInputV1::new(
        TransitionFamilyDeclarationV1::document(Vec::new()).unwrap(),
        b"empty",
    )
    .unwrap();
    let no_config = wire(
        &evaluate_document_transition_poses_with_closure_v1(&empty, &unavailable, &document(None))
            .unwrap(),
    );
    assert!(
        no_config
            .get("subject_dependency_closure_identity")
            .is_none()
    );
    assert!(validator.is_valid(&no_config));
}

#[test]
fn packaged_schema_snapshot_matches_canonical_docs_when_present() {
    let docs = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/schemas/transition-pose-evaluation-v1.schema.json");
    if docs.exists() {
        assert_eq!(std::fs::read_to_string(docs).unwrap(), SCHEMA);
    }
    let _ = validator();
}
