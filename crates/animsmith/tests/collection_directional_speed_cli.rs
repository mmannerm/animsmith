//! Black-box contract tests for `collection evaluate-directional-speed`.

use animsmith_core::InputIdentity;
use animsmith_core::glam::Vec3;
use animsmith_core::model::{Property, TrackValues};
use animsmith_testkit::{quats_from_angles, two_bone_rotation_doc};
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

const EVALUATION_SCHEMA: &str =
    include_str!("../../../docs/schemas/collection-directional-speed-evaluation-v1.schema.json");
const EVALUATION_SCHEMA_ID: &str = "urn:animsmith:schema:collection-directional-speed-evaluation:1";

fn animsmith() -> Command {
    Command::new(env!("CARGO_BIN_EXE_animsmith"))
}

fn write_motion(path: &Path, endpoint: Option<Vec3>) {
    let mut document = two_bone_rotation_doc("move", quats_from_angles(&[0.0; 5]), true);
    if let Some(endpoint) = endpoint {
        let translation = document.clips[0]
            .tracks
            .iter_mut()
            .find(|track| track.property == Property::Translation)
            .unwrap();
        let TrackValues::Vec3s(values) = &mut translation.values else {
            unreachable!("translation track has vector values")
        };
        *values.last_mut().unwrap() = endpoint;
    } else {
        document.clips[0]
            .tracks
            .retain(|track| track.property != Property::Translation);
    }
    animsmith_gltf::write::write(&document, path).unwrap();
}

fn collection_output(
    temp: &tempfile::TempDir,
    kind: &str,
    endpoints: [Option<Vec3>; 2],
    missing_first: bool,
) -> Value {
    if !missing_first {
        write_motion(&temp.path().join("x.glb"), endpoints[0]);
    }
    write_motion(&temp.path().join("z.glb"), endpoints[1]);
    fs::write(
        temp.path().join("roles.toml"),
        "[rig.roles]\nroot = \"root\"\n",
    )
    .unwrap();
    let manifest = temp.path().join("collection.toml");
    fs::write(
        &manifest,
        format!(
            r#"schema = "urn:animsmith:schema:collection-manifest:1"
schema_version = 1
collection_id = "com.example.directional"

[[sources]]
key = "x"
path = "x.glb"
config = "roles.toml"

[[sources]]
key = "z"
path = "z.glb"
config = "roles.toml"

[[clips]]
id = "com.example.directional/x"
source = "x"
take_index = 0
take_name = "move"

[[clips]]
id = "com.example.directional/z"
source = "z"
take_index = 0
take_name = "move"

[[runtime_sets]]
id = "com.example.directional/set"
kind = "{kind}"
members = ["com.example.directional/x", "com.example.directional/z"]
"#,
        ),
    )
    .unwrap();
    let output = animsmith()
        .current_dir(temp.path())
        .args(["collection", "lint", "collection.toml", "--format", "json"])
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(if missing_first { 1 } else { 0 }),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    serde_json::from_slice(&output.stdout).unwrap()
}

fn policy(evidence: &Value, mode: &str, fields: &str, members: &str) -> String {
    format!(
        r#"schema = "urn:animsmith:schema:collection-directional-speed-policy:1"
schema_version = 1
runtime_set_id = "{}"
diagonal_behavior = "normalize"
direction_tolerance_deg = 0.0
mode = "{mode}"
{fields}

[manifest]
schema = "urn:animsmith:schema:collection-manifest:1"
schema_version = 1
collection_id = "{}"
[manifest.input]
sha256 = "{}"
bytes = {}

[source_basis]
x = [1.0, 0.0]
z = [0.0, 1.0]

{members}
"#,
        evidence["runtime_sets"][0]["id"].as_str().unwrap(),
        evidence["manifest"]["collection_id"].as_str().unwrap(),
        evidence["manifest"]["input"]["sha256"].as_str().unwrap(),
        evidence["manifest"]["input"]["bytes"].as_u64().unwrap(),
    )
}

fn uniform_policy(evidence: &Value, speed: f64) -> String {
    let members = evidence["runtime_sets"][0]["members"].as_array().unwrap();
    policy(
        evidence,
        "uniform",
        &format!("uniform_speed_mps = {speed}\nspeed_tolerance_mps = 0.0"),
        &format!(
            "[[members]]\nid = \"{}\"\ncoordinate = [1.0, 0.0]\n\n[[members]]\nid = \"{}\"\ncoordinate = [0.0, 1.0]",
            members[0]["id"].as_str().unwrap(),
            members[1]["id"].as_str().unwrap(),
        ),
    )
}

fn evaluate_paths(policy_path: &Path, evidence_path: &Path) -> Output {
    animsmith()
        .args([
            "collection",
            "evaluate-directional-speed",
            "--policy",
            policy_path.to_str().unwrap(),
            "--evidence",
            evidence_path.to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .unwrap()
}

fn evaluate(temp: &tempfile::TempDir, policy: &str, evidence: &Value) -> Output {
    let policy_path = temp.path().join("policy.toml");
    let evidence_path = temp.path().join("evidence.json");
    fs::write(&policy_path, policy).unwrap();
    fs::write(&evidence_path, serde_json::to_vec(evidence).unwrap()).unwrap();
    evaluate_paths(&policy_path, &evidence_path)
}

fn stabilize_serialized_bytes(evidence: &mut Value) {
    for _ in 0..8 {
        let bytes = serde_json::to_vec(evidence).unwrap().len() as u64;
        if evidence["work"]["serialized_bytes"].as_u64() == Some(bytes) {
            return;
        }
        evidence["work"]["serialized_bytes"] = bytes.into();
    }
    panic!("serialized byte count did not converge");
}

fn assert_result(output: &Output, code: i32, reason: Option<&str>, findings: usize) -> Value {
    assert_eq!(
        output.status.code(),
        Some(code),
        "stderr: {}\nstdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(output.stderr.is_empty());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    let validator =
        jsonschema::validator_for(&serde_json::from_str::<Value>(EVALUATION_SCHEMA).unwrap())
            .unwrap();
    validator.validate(&value).unwrap();
    assert_eq!(value["schema"], EVALUATION_SCHEMA_ID);
    assert_eq!(value["not_evaluated_reason"].as_str(), reason);
    assert_eq!(value["findings"].as_array().unwrap().len(), findings);
    value
}

#[test]
fn subprocess_pass_and_finding_emit_only_the_immutable_result() {
    let temp = tempfile::tempdir().unwrap();
    let evidence = collection_output(
        &temp,
        "directional-blend",
        [Some(Vec3::X), Some(Vec3::Z)],
        false,
    );
    let pass = evaluate(&temp, &uniform_policy(&evidence, 1.0), &evidence);
    let result = assert_result(&pass, 0, None, 0);
    assert_eq!(result["lifecycle"], "complete");

    let finding = evaluate(&temp, &uniform_policy(&evidence, 0.5), &evidence);
    let result = assert_result(&finding, 1, None, 2);
    assert_eq!(result["findings"][0]["kind"], "speed");
    assert_eq!(
        result["findings"][1]["member_id"],
        result["members"][1]["evidence"]["id"]
    );
}

#[test]
fn subprocess_result_identities_bind_exact_noncanonical_raw_inputs() {
    let temp = tempfile::tempdir().unwrap();
    let evidence = collection_output(
        &temp,
        "directional-blend",
        [Some(Vec3::X), Some(Vec3::Z)],
        false,
    );
    let canonical_policy = uniform_policy(&evidence, 1.0);
    let policy_bytes = format!(
        "# Deliberate presentation-only spelling; identity remains raw bytes.\n{}\n",
        canonical_policy,
    )
    .into_bytes();
    assert_ne!(policy_bytes, canonical_policy.into_bytes());
    let mut noncanonical_evidence = evidence.clone();
    let prefix = b"\n ";
    let suffix = b" \t\n";
    for _ in 0..3 {
        let rendered = serde_json::to_vec(&noncanonical_evidence).unwrap();
        noncanonical_evidence["work"]["serialized_bytes"] =
            (prefix.len() + rendered.len() + suffix.len()).into();
    }
    let canonical_evidence = serde_json::to_vec(&noncanonical_evidence).unwrap();
    let mut evidence_bytes = prefix.to_vec();
    evidence_bytes.extend_from_slice(&canonical_evidence);
    evidence_bytes.extend_from_slice(suffix);
    assert_ne!(evidence_bytes, canonical_evidence);
    assert_eq!(
        noncanonical_evidence["work"]["serialized_bytes"].as_u64(),
        Some(evidence_bytes.len() as u64)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&evidence_bytes).unwrap(),
        noncanonical_evidence
    );

    let policy_path = temp.path().join("noncanonical-policy.toml");
    let evidence_path = temp.path().join("noncanonical-evidence.json");
    fs::write(&policy_path, &policy_bytes).unwrap();
    fs::write(&evidence_path, &evidence_bytes).unwrap();

    let output = evaluate_paths(&policy_path, &evidence_path);
    let result = assert_result(&output, 0, None, 0);
    assert_eq!(
        result["policy_input"],
        serde_json::to_value(InputIdentity::from_bytes(&policy_bytes)).unwrap()
    );
    assert_eq!(
        result["evidence_input"],
        serde_json::to_value(InputIdentity::from_bytes(&evidence_bytes)).unwrap()
    );
}

#[test]
fn subprocess_incomplete_and_zero_net_not_evaluated_emit_result_and_exit_one() {
    let temp = tempfile::tempdir().unwrap();
    let incomplete = collection_output(
        &temp,
        "directional-blend",
        [Some(Vec3::X), Some(Vec3::Z)],
        true,
    );
    let output = evaluate(&temp, &uniform_policy(&incomplete, 1.0), &incomplete);
    assert_result(&output, 1, Some("incomplete_root_travel"), 0);

    let complete = collection_output(
        &temp,
        "directional-blend",
        [Some(Vec3::X), Some(Vec3::Z)],
        false,
    );
    let mut partial_closure = incomplete.clone();
    partial_closure["sources"][0] = complete["sources"][0].clone();
    partial_closure["sources"][0]["dependency_closure"] = serde_json::json!({
        "state": "partial",
        "reasons": ["unavailable_resource"]
    });
    partial_closure["clips"][0]["binding"] = serde_json::json!({
        "state": "unavailable",
        "reason": "dependency_closure_incomplete"
    });
    partial_closure["runtime_sets"][0]["members"][0]["resolution"] = serde_json::json!({
        "state": "unavailable",
        "reason": "dependency_closure_incomplete"
    });
    partial_closure["summary"]["readable_sources"] = 2.into();
    partial_closure["work"] = complete["work"].clone();
    stabilize_serialized_bytes(&mut partial_closure);
    let output = evaluate(
        &temp,
        &uniform_policy(&partial_closure, 1.0),
        &partial_closure,
    );
    let result = assert_result(&output, 1, Some("incomplete_root_travel"), 0);
    assert_eq!(
        result["gaps"],
        serde_json::json!(["com.example.directional/x"])
    );
    let members = result["members"].as_array().unwrap();
    assert_eq!(members.len(), 2);
    assert_eq!(members[0]["evidence"]["id"], "com.example.directional/x");
    assert_eq!(members[1]["evidence"]["id"], "com.example.directional/z");

    let zero_net = collection_output(
        &temp,
        "directional-blend",
        [Some(Vec3::ZERO), Some(Vec3::Z)],
        false,
    );
    let output = evaluate(&temp, &uniform_policy(&zero_net, 1.0), &zero_net);
    assert_result(&output, 1, Some("zero_net_displacement"), 0);
}

#[test]
fn subprocess_control_inputs_emit_no_stdout_and_stable_stderr() {
    let temp = tempfile::tempdir().unwrap();
    let evidence = collection_output(
        &temp,
        "directional-blend",
        [Some(Vec3::X), Some(Vec3::Z)],
        false,
    );
    let policy = uniform_policy(&evidence, 1.0);
    let valid_policy_path = temp.path().join("valid-policy.toml");
    let valid_evidence_path = temp.path().join("valid-evidence.json");
    fs::write(&valid_policy_path, &policy).unwrap();
    fs::write(&valid_evidence_path, serde_json::to_vec(&evidence).unwrap()).unwrap();

    let stale = policy.replacen(
        evidence["manifest"]["input"]["sha256"].as_str().unwrap(),
        "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
        1,
    );
    let output = evaluate(&temp, &stale, &evidence);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "animsmith: policy does not bind to directional-speed evidence\n"
    );

    let wrong_kind = collection_output(&temp, "gait-group", [Some(Vec3::X), Some(Vec3::Z)], false);
    let output = evaluate(&temp, &uniform_policy(&wrong_kind, 1.0), &wrong_kind);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "animsmith: policy does not bind to directional-speed evidence\n"
    );

    let invalid_path = temp.path().join("invalid.toml");
    fs::write(
        &invalid_path,
        "schema = \"not-a-policy\"\nschema_version = 1\n",
    )
    .unwrap();
    let invalid = animsmith()
        .args([
            "collection",
            "evaluate-directional-speed",
            "--policy",
            invalid_path.to_str().unwrap(),
            "--evidence",
            temp.path().join("evidence.json").to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert_eq!(invalid.status.code(), Some(2));
    assert!(invalid.stdout.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&invalid.stderr),
        "animsmith: directional-speed policy control error (unsupported-schema)\n"
    );

    let policy_path = temp.path().join("policy.toml");
    let evidence_path = temp.path().join("bad.json");
    fs::write(&policy_path, &policy).unwrap();
    fs::write(&evidence_path, b"{").unwrap();
    let malformed = animsmith()
        .args([
            "collection",
            "evaluate-directional-speed",
            "--policy",
            policy_path.to_str().unwrap(),
            "--evidence",
            evidence_path.to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert_eq!(malformed.status.code(), Some(2));
    assert!(malformed.stdout.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&malformed.stderr),
        "animsmith: collection output JSON is invalid\n"
    );

    let oversized = temp.path().join("oversized.toml");
    fs::write(&oversized, vec![b' '; 8 * 1024 * 1024 + 1]).unwrap();
    let overbudget = animsmith()
        .args([
            "collection",
            "evaluate-directional-speed",
            "--policy",
            oversized.to_str().unwrap(),
            "--evidence",
            evidence_path.to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert_eq!(overbudget.status.code(), Some(2));
    assert!(overbudget.stdout.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&overbudget.stderr),
        "animsmith: directional-speed policy exceeds its bounded reader limit\n"
    );

    let oversized_evidence = temp.path().join("oversized-evidence.json");
    // A sparse N+1 file keeps fixture setup cheap while the evaluator still
    // traverses its actual bounded read path before rejecting it.
    fs::File::create(&oversized_evidence)
        .unwrap()
        .set_len(256 * 1024 * 1024 + 1)
        .unwrap();
    let overbudget_evidence = evaluate_paths(&policy_path, &oversized_evidence);
    assert_eq!(overbudget_evidence.status.code(), Some(2));
    assert!(overbudget_evidence.stdout.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&overbudget_evidence.stderr),
        "animsmith: collection-output evidence exceeds its bounded reader limit\n"
    );

    let unreadable = animsmith()
        .args([
            "collection",
            "evaluate-directional-speed",
            "--policy",
            temp.path().join("missing.toml").to_str().unwrap(),
            "--evidence",
            evidence_path.to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert_eq!(unreadable.status.code(), Some(2));
    assert!(unreadable.stdout.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&unreadable.stderr),
        "animsmith: cannot read directional-speed policy\n"
    );

    let unreadable_evidence = animsmith()
        .args([
            "collection",
            "evaluate-directional-speed",
            "--policy",
            policy_path.to_str().unwrap(),
            "--evidence",
            temp.path().join("missing.json").to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert_eq!(unreadable_evidence.status.code(), Some(2));
    assert!(unreadable_evidence.stdout.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&unreadable_evidence.stderr),
        "animsmith: cannot read collection-output evidence\n"
    );

    let destination = temp.path().join("must-not-exist.json");
    let output_option = animsmith()
        .args([
            "collection",
            "evaluate-directional-speed",
            "--policy",
            valid_policy_path.to_str().unwrap(),
            "--evidence",
            valid_evidence_path.to_str().unwrap(),
            "--output",
            destination.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(output_option.status.code(), Some(2));
    assert!(output_option.stdout.is_empty());
    assert!(!destination.exists());

    let config = temp.path().join("ignored.toml");
    fs::write(&config, "[rig.roles]\nroot = \"root\"\n").unwrap();
    let global_config = animsmith()
        .args([
            "--config",
            config.to_str().unwrap(),
            "collection",
            "evaluate-directional-speed",
            "--policy",
            valid_policy_path.to_str().unwrap(),
            "--evidence",
            valid_evidence_path.to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert_eq!(global_config.status.code(), Some(2));
    assert!(global_config.stdout.is_empty());
    assert!(!destination.exists());
    assert_eq!(
        String::from_utf8_lossy(&global_config.stderr),
        "animsmith: --config is not accepted by collection commands; collection lint declares each source config in the collection manifest\n"
    );
}
