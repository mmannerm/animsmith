use animsmith_core::{
    Bone, Clip, Document, DocumentTransitionFamilyMemberV1, DocumentTransitionFamilyV1,
    InputIdentity, Interpolation, Property, Skeleton, Track, TrackValues, Transform,
    TransitionFamilyBoundaryV1, TransitionFamilyDeclarationInputV1, TransitionFamilyDeclarationV1,
    TransitionFamilyTolerancesV1, evaluate_document_transition_poses_v1, glam,
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
    let validator = validator();
    for output in [&pass, &finding, &incomplete, &no_config] {
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
fn packaged_schema_snapshot_matches_canonical_docs_when_present() {
    let docs = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/schemas/transition-pose-evaluation-v1.schema.json");
    if docs.exists() {
        assert_eq!(std::fs::read_to_string(docs).unwrap(), SCHEMA);
    }
    let _ = validator();
}
