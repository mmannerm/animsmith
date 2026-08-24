use animsmith_core::{
    COLLECTION_DIRECTIONAL_SPEED_EVIDENCE_V1_MAX_BYTES,
    COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_MAX_BYTES, COLLECTION_MANIFEST_V1_MAX_MANIFEST_BYTES,
    CollectionDirectionalSpeedDiagonalBehaviorV1, CollectionDirectionalSpeedEvidenceMemberV1,
    CollectionDirectionalSpeedEvidenceV1, CollectionDirectionalSpeedLifecycleV1,
    CollectionDirectionalSpeedManifestIdentityV1, CollectionDirectionalSpeedMemberV1,
    CollectionDirectionalSpeedModeV1, CollectionDirectionalSpeedNotEvaluatedReasonV1,
    CollectionDirectionalSpeedPolicyV1, CollectionDirectionalSpeedSourceBasisV1, CollectionIdV1,
    CollectionLogicalIdV1, CollectionRuntimeSetKindV1, InputIdentity,
    evaluate_collection_directional_speed_v1,
};

const SCHEMA: &str =
    include_str!("../../../docs/schemas/collection-directional-speed-evaluation-v1.schema.json");

fn id(value: &str) -> CollectionLogicalIdV1 {
    CollectionLogicalIdV1::new(value).unwrap()
}

fn policy() -> CollectionDirectionalSpeedPolicyV1 {
    CollectionDirectionalSpeedPolicyV1::new(
        CollectionDirectionalSpeedManifestIdentityV1::new(
            CollectionIdV1::new("com.example.collection").unwrap(),
            InputIdentity::from_bytes(b"manifest"),
        )
        .unwrap(),
        id("com.example/set"),
        CollectionDirectionalSpeedSourceBasisV1::new([1.0, 0.0], [0.0, 1.0]).unwrap(),
        CollectionDirectionalSpeedDiagonalBehaviorV1::Normalize,
        0.0,
        CollectionDirectionalSpeedModeV1::Uniform {
            speed_mps: 1.0,
            speed_tolerance_mps: 0.1,
        },
        vec![
            CollectionDirectionalSpeedMemberV1::new(id("com.example/x"), [1.0, 0.0], None, None),
            CollectionDirectionalSpeedMemberV1::new(id("com.example/z"), [0.0, 1.0], None, None),
        ],
    )
    .unwrap()
}

fn evidence(
    lifecycle: CollectionDirectionalSpeedLifecycleV1,
    x_speed: Option<f64>,
) -> CollectionDirectionalSpeedEvidenceV1 {
    let policy = policy();
    CollectionDirectionalSpeedEvidenceV1::new(
        policy.manifest().clone(),
        policy.runtime_set_id().clone(),
        CollectionRuntimeSetKindV1::DirectionalBlend,
        lifecycle,
        vec![],
        vec![
            CollectionDirectionalSpeedEvidenceMemberV1::new(
                id("com.example/x"),
                Some(1.0),
                Some(1.0),
                Some(0.0),
                Some(1.0),
                x_speed,
            ),
            CollectionDirectionalSpeedEvidenceMemberV1::new(
                id("com.example/z"),
                Some(1.0),
                Some(0.0),
                Some(1.0),
                Some(1.0),
                Some(1.0),
            ),
        ],
    )
    .unwrap()
}

fn ratios_policy() -> CollectionDirectionalSpeedPolicyV1 {
    let base = policy();
    CollectionDirectionalSpeedPolicyV1::new(
        base.manifest().clone(),
        base.runtime_set_id().clone(),
        CollectionDirectionalSpeedSourceBasisV1::new([1.0, 0.0], [0.0, 1.0]).unwrap(),
        CollectionDirectionalSpeedDiagonalBehaviorV1::Normalize,
        0.0,
        CollectionDirectionalSpeedModeV1::Ratios {
            reference_member: id("com.example/x"),
            ratio_tolerance: 0.0,
        },
        vec![
            CollectionDirectionalSpeedMemberV1::new(
                id("com.example/x"),
                [1.0, 0.0],
                None,
                Some(1.0),
            ),
            CollectionDirectionalSpeedMemberV1::new(
                id("com.example/z"),
                [0.0, 1.0],
                None,
                Some(2.0),
            ),
        ],
    )
    .unwrap()
}

fn ratio_evidence(x_speed: f64, z_speed: f64) -> CollectionDirectionalSpeedEvidenceV1 {
    let policy = ratios_policy();
    CollectionDirectionalSpeedEvidenceV1::new(
        policy.manifest().clone(),
        policy.runtime_set_id().clone(),
        CollectionRuntimeSetKindV1::DirectionalBlend,
        CollectionDirectionalSpeedLifecycleV1::Complete,
        vec![],
        vec![
            CollectionDirectionalSpeedEvidenceMemberV1::new(
                id("com.example/x"),
                Some(1.0),
                Some(1.0),
                Some(0.0),
                Some(1.0),
                Some(x_speed),
            ),
            CollectionDirectionalSpeedEvidenceMemberV1::new(
                id("com.example/z"),
                Some(1.0),
                Some(0.0),
                Some(1.0),
                Some(1.0),
                Some(z_speed),
            ),
        ],
    )
    .unwrap()
}

#[test]
fn complete_finding_and_not_evaluated_results_satisfy_the_packaged_schema() {
    let validator =
        jsonschema::validator_for(&serde_json::from_str::<serde_json::Value>(SCHEMA).unwrap())
            .unwrap();
    let policy = policy();
    for evidence in [
        evidence(CollectionDirectionalSpeedLifecycleV1::Complete, Some(1.0)),
        evidence(CollectionDirectionalSpeedLifecycleV1::Complete, Some(1.2)),
        evidence(CollectionDirectionalSpeedLifecycleV1::Incomplete, None),
    ] {
        let result = evaluate_collection_directional_speed_v1(
            &policy,
            InputIdentity::from_bytes(b"policy"),
            InputIdentity::from_bytes(b"evidence"),
            &evidence,
        )
        .unwrap();
        validator
            .validate(&serde_json::to_value(result).unwrap())
            .unwrap();
    }
}

#[test]
fn packaged_schema_fixtures_cover_ratios_and_every_not_evaluated_reason() {
    let validator =
        jsonschema::validator_for(&serde_json::from_str::<serde_json::Value>(SCHEMA).unwrap())
            .unwrap();
    let uniform = policy();
    let ratios = ratios_policy();
    let zero_endpoint = CollectionDirectionalSpeedEvidenceV1::new(
        uniform.manifest().clone(),
        uniform.runtime_set_id().clone(),
        CollectionRuntimeSetKindV1::DirectionalBlend,
        CollectionDirectionalSpeedLifecycleV1::Complete,
        vec![],
        vec![
            CollectionDirectionalSpeedEvidenceMemberV1::new(
                id("com.example/x"),
                Some(1.0),
                Some(0.0),
                Some(0.0),
                Some(0.0),
                Some(0.0),
            ),
            CollectionDirectionalSpeedEvidenceMemberV1::new(
                id("com.example/z"),
                Some(1.0),
                Some(0.0),
                Some(1.0),
                Some(1.0),
                Some(1.0),
            ),
        ],
    )
    .unwrap();
    let fixtures = [
        (
            &uniform,
            evidence(CollectionDirectionalSpeedLifecycleV1::Incomplete, None),
            CollectionDirectionalSpeedNotEvaluatedReasonV1::IncompleteRootTravel,
        ),
        (
            &uniform,
            zero_endpoint,
            CollectionDirectionalSpeedNotEvaluatedReasonV1::ZeroNetDisplacement,
        ),
        (
            &ratios,
            ratio_evidence(0.0, 1.0),
            CollectionDirectionalSpeedNotEvaluatedReasonV1::ZeroReferenceSpeed,
        ),
        (
            &ratios,
            ratio_evidence(f64::MIN_POSITIVE, f64::MAX),
            CollectionDirectionalSpeedNotEvaluatedReasonV1::NumericRange,
        ),
    ];
    for (policy, evidence, reason) in fixtures {
        let result = evaluate_collection_directional_speed_v1(
            policy,
            InputIdentity::from_bytes(b"policy"),
            InputIdentity::from_bytes(b"evidence"),
            &evidence,
        )
        .unwrap();
        assert_eq!(result.not_evaluated_reason(), Some(reason));
        validator
            .validate(&serde_json::to_value(result).unwrap())
            .unwrap();
    }

    let result = evaluate_collection_directional_speed_v1(
        &ratios,
        InputIdentity::from_bytes(b"policy"),
        InputIdentity::from_bytes(b"evidence"),
        &ratio_evidence(1.0, 2.0),
    )
    .unwrap();
    assert!(result.not_evaluated_reason().is_none());
    assert!(result.findings().is_empty());
    validator
        .validate(&serde_json::to_value(result).unwrap())
        .unwrap();

    let ratio_finding = evaluate_collection_directional_speed_v1(
        &ratios,
        InputIdentity::from_bytes(b"policy"),
        InputIdentity::from_bytes(b"evidence"),
        &ratio_evidence(1.0, 1.0),
    )
    .unwrap();
    let ratio_finding = serde_json::to_value(ratio_finding).unwrap();
    validator.validate(&ratio_finding).unwrap();
    assert_eq!(
        ratio_finding["findings"],
        serde_json::json!([{
            "kind": "ratio",
            "member_id": "com.example/z",
            "measured_ratio": 1.0,
            "expected_ratio": 2.0,
            "tolerance": 0.0,
        }])
    );
}

#[test]
fn packaged_schema_rejects_negative_retained_measurements_and_unknown_row_fields() {
    let validator =
        jsonschema::validator_for(&serde_json::from_str::<serde_json::Value>(SCHEMA).unwrap())
            .unwrap();
    let value = serde_json::to_value(
        evaluate_collection_directional_speed_v1(
            &policy(),
            InputIdentity::from_bytes(b"policy"),
            InputIdentity::from_bytes(b"evidence"),
            &evidence(CollectionDirectionalSpeedLifecycleV1::Complete, Some(1.0)),
        )
        .unwrap(),
    )
    .unwrap();
    let mut negative = value.clone();
    negative["members"][0]["evidence"]["speed_mps"] = serde_json::json!(-1.0);
    assert!(validator.validate(&negative).is_err());
    let mut unknown = value;
    unknown["members"][0]["bogus"] = serde_json::json!(true);
    assert!(validator.validate(&unknown).is_err());
    let mut invalid_member = serde_json::to_value(
        evaluate_collection_directional_speed_v1(
            &policy(),
            InputIdentity::from_bytes(b"policy"),
            InputIdentity::from_bytes(b"evidence"),
            &evidence(CollectionDirectionalSpeedLifecycleV1::Complete, Some(1.0)),
        )
        .unwrap(),
    )
    .unwrap();
    invalid_member["members"][0]["magnitude_deviation"] = serde_json::json!(-0.1);
    assert!(validator.validate(&invalid_member).is_err());
}

#[test]
fn packaged_schema_matches_provenance_byte_limits() {
    let validator =
        jsonschema::validator_for(&serde_json::from_str::<serde_json::Value>(SCHEMA).unwrap())
            .unwrap();
    let value = serde_json::to_value(
        evaluate_collection_directional_speed_v1(
            &policy(),
            InputIdentity::from_bytes(b"policy"),
            InputIdentity::from_bytes(b"evidence"),
            &evidence(CollectionDirectionalSpeedLifecycleV1::Complete, Some(1.0)),
        )
        .unwrap(),
    )
    .unwrap();
    let mut exact = value.clone();
    exact["manifest"]["input"]["bytes"] = COLLECTION_MANIFEST_V1_MAX_MANIFEST_BYTES.into();
    exact["policy_input"]["bytes"] = COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_MAX_BYTES.into();
    exact["evidence_input"]["bytes"] = COLLECTION_DIRECTIONAL_SPEED_EVIDENCE_V1_MAX_BYTES.into();
    validator.validate(&exact).unwrap();

    let mut manifest_over = exact.clone();
    manifest_over["manifest"]["input"]["bytes"] =
        (COLLECTION_MANIFEST_V1_MAX_MANIFEST_BYTES + 1).into();
    assert!(validator.validate(&manifest_over).is_err());
    let mut policy_over = exact.clone();
    policy_over["policy_input"]["bytes"] =
        (COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_MAX_BYTES + 1).into();
    assert!(validator.validate(&policy_over).is_err());
    let mut evidence_over = exact;
    evidence_over["evidence_input"]["bytes"] =
        (COLLECTION_DIRECTIONAL_SPEED_EVIDENCE_V1_MAX_BYTES + 1).into();
    assert!(validator.validate(&evidence_over).is_err());
}
