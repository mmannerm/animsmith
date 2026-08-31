use jsonschema::Validator;
use serde_json::{Value, json};

const MEMBER_SCHEMA: &str =
    include_str!("../../../docs/schemas/foot-cycle-member-evidence-v1.schema.json");
const AGGREGATE_SCHEMA: &str =
    include_str!("../../../docs/schemas/foot-cycle-aggregate-evidence-v1.schema.json");

fn identity(character: char, bytes: u64) -> Value {
    json!({"sha256": character.to_string().repeat(64), "bytes": bytes})
}

fn tool() -> Value {
    json!({"name": "animsmith", "version": "0.9.0", "source": {"revision": null, "dirty": null}})
}

fn policy() -> Value {
    json!({"max_gait_phase_spread": 0.08, "min_lr_amplitude_m": 0.05, "max_contact_boundary_phase_error": 0.01})
}

fn member(index: u64) -> Value {
    json!({
        "schema": "urn:animsmith:schema:foot-cycle-member-evidence:1",
        "schema_version": 1,
        "tool": tool(),
        "command": "collection-transform-foot-cycle",
        "member_index": index,
        "member_id": format!("com.example/{index}"),
        "paths": {
            "artifact": format!("members/{index:06}/artifact.glb"),
            "contact_fragment": format!("members/{index:06}/contact-fragment.json"),
            "evidence": format!("members/{index:06}/evidence.json")
        },
        "manifest_input": identity('a', 10),
        "parameterization_input": identity('b', 20),
        "source": {"source_key": format!("source-{index}"), "artifact": identity('c', 30), "dependency_closure_identity": identity('d', 40)},
        "output": {"artifact": identity('e', 100), "dependency_closure_identity": identity('f', 50), "contact_fragment": identity('1', 60), "independently_detected_contact_fragment": identity('2', 61)},
        "operation": identity('3', 70),
        "proof_policy": policy(),
        "proof": {"duration_s": 1.0, "gait_phase": 0.25, "lr_amplitude_m": 0.1, "max_contact_boundary_phase_error": 0.005, "root_endpoint_displacement_x_m": 0.0, "root_endpoint_displacement_z_m": 0.0, "root_accumulated_yaw_deg": 0.0, "max_loop_position_delta_m": 0.0, "max_loop_rotation_delta_deg": 0.0, "max_loop_velocity_delta_mps": 0.0, "max_loop_angular_velocity_delta_degps": 0.0},
        "resources": {"artifact_bytes": 100, "contact_fragment_bytes": 200}
    })
}

fn aggregate_member(index: u64) -> Value {
    json!({
        "member_index": index,
        "member_id": format!("com.example/{index}"),
        "artifact_path": format!("members/{index:06}/artifact.glb"),
        "contact_fragment_path": format!("members/{index:06}/contact-fragment.json"),
        "evidence_path": format!("members/{index:06}/evidence.json"),
        "source_artifact": identity('a', 10),
        "source_dependency_closure_identity": identity('b', 20),
        "output_artifact": identity('c', 100),
        "output_dependency_closure_identity": identity('d', 30),
        "output_contact_fragment": identity('e', 40),
        "independently_detected_contact_fragment": identity('f', 41),
        "evidence": identity('1', 200)
    })
}

fn validators() -> (Validator, Validator) {
    let member: Value = serde_json::from_str(MEMBER_SCHEMA).unwrap();
    let aggregate: Value = serde_json::from_str(AGGREGATE_SCHEMA).unwrap();
    let registry = jsonschema::Registry::new()
        .add(
            "urn:animsmith:schema:foot-cycle-member-evidence:1",
            member.clone(),
        )
        .unwrap()
        .prepare()
        .unwrap();
    let member_validator = Validator::new(&member).unwrap();
    let aggregate_validator = jsonschema::options()
        .with_registry(&registry)
        .build(&aggregate)
        .unwrap();
    (member_validator, aggregate_validator)
}

#[test]
fn member_and_aggregate_v1_schemas_close_every_record_level() {
    let (member_validator, aggregate_validator) = validators();
    assert!(member_validator.is_valid(&member(0)));
    let aggregate = json!({
        "schema": "urn:animsmith:schema:foot-cycle-aggregate-evidence:1",
        "schema_version": 1,
        "tool": tool(),
        "command": "collection-transform-foot-cycle",
        "outcome": "published",
        "manifest_input": identity('a', 10),
        "parameterization_input": identity('b', 20),
        "runtime_set_id": "com.example/sets/walk",
        "reference_member": "com.example/0",
        "proof_policy": policy(),
        "gait_phase_spread": 0.01,
        "members": [aggregate_member(0), aggregate_member(1)],
        "resources": {"members": 2, "files": 7, "artifact_bytes": 200, "contact_fragment_bytes": 400, "member_evidence_bytes": 500, "aggregate_evidence_bytes": 1000, "total_bytes": 2100, "retained_candidate_bytes": 200, "source_metric_pose_cells": 10, "source_metric_sample_evaluations": 20, "output_metric_pose_cells": 30, "output_metric_sample_evaluations": 40, "metric_pose_cells": 40, "metric_sample_evaluations": 60}
    });
    assert!(aggregate_validator.is_valid(&aggregate));

    for pointer in ["", "/tool", "/members/0", "/resources"] {
        let mut candidate = aggregate.clone();
        candidate
            .pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unknown".into(), true.into());
        assert!(
            !aggregate_validator.is_valid(&candidate),
            "unknown at {pointer}"
        );
    }
    let mut wrong_path = member(0);
    wrong_path["paths"]["artifact"] = "members/name/artifact.glb".into();
    assert!(!member_validator.is_valid(&wrong_path));
}
