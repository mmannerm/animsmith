use jsonschema::Validator;
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const COLLECTION_SCHEMA_ID: &str = "urn:animsmith:schema:collection-output:9";
const OUTPUT_SCHEMA_ID: &str = "urn:animsmith:schema:output:17";
const OUTPUT_V14_SCHEMA_ID: &str = "urn:animsmith:schema:output:14";
const OUTPUT_V13_SCHEMA_ID: &str = "urn:animsmith:schema:output:13";
const OUTPUT_V10_SCHEMA_ID: &str = "urn:animsmith:schema:output:10";
const MEASUREMENTS_V15_SCHEMA_ID: &str = "urn:animsmith:schema:measurements:15";
const MEASUREMENTS_SCHEMA_ID: &str = "urn:animsmith:schema:measurements:16";
const COLLECTION_SCHEMA: &str =
    include_str!("../../../docs/schemas/collection-output-v9.schema.json");
const OUTPUT_SCHEMA: &str = include_str!("../../../docs/schemas/output-v17.schema.json");
const OUTPUT_V14_SCHEMA: &str = include_str!("../../../docs/schemas/output-v14.schema.json");
const OUTPUT_V13_SCHEMA: &str = include_str!("../../../docs/schemas/output-v13.schema.json");
const OUTPUT_V10_SCHEMA: &str = include_str!("../../../docs/schemas/output-v10.schema.json");
const MEASUREMENTS_V15_SCHEMA: &str =
    include_str!("../../../docs/schemas/measurements-v15.schema.json");
const MEASUREMENTS_SCHEMA: &str =
    include_str!("../../../docs/schemas/measurements-v16.schema.json");

fn spike_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("testdata/collection-spike")
        .join(relative)
}

fn collection(manifest: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .args([
            "collection",
            "lint",
            manifest.to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .expect("collection lint runs")
}

fn collection_validator() -> Validator {
    let collection: Value = serde_json::from_str(COLLECTION_SCHEMA).unwrap();
    let output: Value = serde_json::from_str(OUTPUT_SCHEMA).unwrap();
    let output_v14: Value = serde_json::from_str(OUTPUT_V14_SCHEMA).unwrap();
    let output_v13: Value = serde_json::from_str(OUTPUT_V13_SCHEMA).unwrap();
    let output_v10: Value = serde_json::from_str(OUTPUT_V10_SCHEMA).unwrap();
    let measurements_v15: Value = serde_json::from_str(MEASUREMENTS_V15_SCHEMA).unwrap();
    let measurements: Value = serde_json::from_str(MEASUREMENTS_SCHEMA).unwrap();
    let registry = jsonschema::Registry::new()
        .add(OUTPUT_V14_SCHEMA_ID, output_v14)
        .unwrap()
        .add(OUTPUT_V13_SCHEMA_ID, output_v13)
        .unwrap()
        .add(OUTPUT_V10_SCHEMA_ID, output_v10)
        .unwrap()
        .add(MEASUREMENTS_V15_SCHEMA_ID, measurements_v15)
        .unwrap()
        .add(OUTPUT_SCHEMA_ID, output)
        .unwrap()
        .add(MEASUREMENTS_SCHEMA_ID, measurements)
        .unwrap()
        .prepare()
        .unwrap();
    jsonschema::options()
        .with_registry(&registry)
        .build(&collection)
        .expect("collection schema compiles")
}

fn assert_schema(value: &Value) {
    let errors = collection_validator()
        .iter_errors(value)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(errors.is_empty(), "schema errors:\n{}", errors.join("\n"));
}

#[test]
fn retained_spike_emits_exact_deterministic_collection_evidence() {
    let first = collection(&spike_path("collection.toml"));
    assert_eq!(
        first.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let value: Value = serde_json::from_slice(&first.stdout).expect("collection JSON");
    assert_schema(&value);
    assert_eq!(value["schema"], COLLECTION_SCHEMA_ID);
    assert_eq!(value["schema_version"], 9);
    assert_eq!(
        value["sources"][0]["result"]["envelope"]["schema"],
        OUTPUT_SCHEMA_ID
    );
    assert_eq!(
        value["sources"][0]["result"]["envelope"]["schema_version"],
        17
    );
    assert_eq!(value["summary"]["sources"], 3);
    assert_eq!(value["summary"]["established_clips"], 4);
    assert_eq!(value["summary"]["complete_runtime_sets"], 2);
    assert_eq!(value["summary"]["incomplete"], false);
    assert_eq!(value["sources"][0]["key"], "multi");
    assert_eq!(
        value["sources"][0]["dependency_closure"]["state"],
        "complete"
    );
    assert!(
        value["sources"][0]["dependency_closure"]["identity"]["sha256"]
            .as_str()
            .is_some_and(|digest| digest.len() == 64)
    );
    assert_eq!(
        value["sources"][0]["config"]["input"]["sha256"],
        "385b7a67171994d8099fb7d4623721fc7b84fcdbe8cba1b7883f72fbba75182e"
    );
    assert_eq!(
        value["sources"][1]["input"]["input"]["sha256"],
        "277f55812602cc560dbb432dede43bb145b3caa6cb90493675442a8f5499f044"
    );
    assert_eq!(value["clips"][0]["binding"]["state"], "established");
    assert_eq!(value["runtime_sets"][0]["decision"], "not_evaluated");
    assert_eq!(
        value["runtime_sets"][0]["members"][0]["gait_phase"]["availability"],
        "not_applicable"
    );
    assert_eq!(
        value["runtime_sets"][0]["evidence"]["gait_phase"]["lifecycle"],
        "incomplete"
    );
    assert!(
        value["runtime_sets"][0]["evidence"]["gait_phase"]
            .get("phase_spread")
            .is_none()
    );
    assert_eq!(
        value["runtime_sets"][0]["members"][0]["root_travel"]["translation_availability"],
        "not_applicable"
    );
    assert_eq!(
        value["runtime_sets"][0]["evidence"]["root_travel"]["lifecycle"],
        "incomplete"
    );
    assert_eq!(
        value["runtime_sets"][1]["evidence"]["root_travel"]["members_measured"],
        0
    );
    assert!(
        value["runtime_sets"][1]["members"][0]
            .get("gait_phase")
            .is_none()
    );
    let mut missing_gait_member_evidence = value.clone();
    missing_gait_member_evidence["runtime_sets"][0]["members"][0]
        .as_object_mut()
        .unwrap()
        .remove("gait_phase");
    assert!(
        !collection_validator().is_valid(&missing_gait_member_evidence),
        "the schema requires a gait-phase row for every gait-group member"
    );
    let mut non_gait_member_evidence = value.clone();
    non_gait_member_evidence["runtime_sets"][1]["members"][0]["gait_phase"] =
        serde_json::json!({"availability": "unavailable"});
    assert!(
        !collection_validator().is_valid(&non_gait_member_evidence),
        "the schema rejects gait-phase rows on non-gait sets"
    );
    let mut missing_root_travel = value.clone();
    missing_root_travel["runtime_sets"][1]["members"][0]
        .as_object_mut()
        .unwrap()
        .remove("root_travel");
    assert!(
        !collection_validator().is_valid(&missing_root_travel),
        "the schema requires root-travel evidence for every declared member"
    );
    assert_eq!(value["work"]["serialized_bytes"], first.stdout.len());

    let second = collection(&spike_path("collection.toml"));
    assert_eq!(second.status.code(), Some(0));
    assert_eq!(
        first.stdout, second.stdout,
        "same inputs must be byte stable"
    );
}

#[test]
fn control_errors_and_global_config_emit_no_envelope() {
    for manifest in [
        "invalid-duplicate-member.toml",
        "invalid-missing-member.toml",
        "invalid-escaping-source.toml",
    ] {
        let output = collection(&spike_path(manifest));
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }

    let output = Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .args([
            "--config",
            spike_path("fixture.animsmith.toml").to_str().unwrap(),
            "collection",
            "lint",
            spike_path("collection.toml").to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());

    let output = Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .args([
            "collection",
            "lint",
            spike_path("collection.toml").to_str().unwrap(),
            "--format",
            "text",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty(), "collection V1 accepts JSON only");

    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("bad.toml"), "not = [valid").unwrap();
    let manifest = temp.path().join("collection.toml");
    fs::write(
        &manifest,
        r#"schema = "urn:animsmith:schema:collection-manifest:1"
schema_version = 1
collection_id = "com.example.controls"
[[sources]]
key = "missing"
path = "missing.gltf"
config = "bad.toml"
[[clips]]
id = "com.example.controls/missing"
source = "missing"
take_index = 0
take_name = "Take 001"
"#,
    )
    .unwrap();
    let output = collection(&manifest);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        output.stdout.is_empty(),
        "config control failure wins before missing-source data routing"
    );
}

#[test]
fn absent_manifest_config_ignores_ambient_discovery() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("source.gltf"),
        fs::read(spike_path("source/walk-a.gltf")).unwrap(),
    )
    .unwrap();
    fs::write(temp.path().join("animsmith.toml"), "not = [valid").unwrap();
    let manifest = temp.path().join("collection.toml");
    fs::write(
        &manifest,
        r#"schema = "urn:animsmith:schema:collection-manifest:1"
schema_version = 1
collection_id = "com.example.default-config"
[[sources]]
key = "source"
path = "source.gltf"
[[clips]]
id = "com.example.default-config/take"
source = "source"
take_index = 0
take_name = "Take 001"
"#,
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .current_dir(temp.path())
        .args(["collection", "lint", "collection.toml", "--format", "json"])
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["sources"][0]["config"]["state"], "default");
}

#[test]
fn missing_gltf_dependency_has_a_stable_typed_loader_state() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("source.gltf"),
        r#"{
  "asset": {"version": "2.0"},
  "buffers": [{"uri": "missing.bin", "byteLength": 4}],
  "nodes": [{"name": "root"}],
  "animations": [{"name": "Take 001", "samplers": [], "channels": []}],
  "scenes": [{"nodes": [0]}],
  "scene": 0
}"#,
    )
    .unwrap();
    let manifest = temp.path().join("collection.toml");
    fs::write(
        &manifest,
        r#"schema = "urn:animsmith:schema:collection-manifest:1"
schema_version = 1
collection_id = "com.example.dependency"
[[sources]]
key = "source"
path = "source.gltf"
[[clips]]
id = "com.example.dependency/take"
source = "source"
take_index = 0
take_name = "Take 001"
"#,
    )
    .unwrap();

    let output = collection(&manifest);
    assert_eq!(output.status.code(), Some(1));
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_schema(&value);
    assert_eq!(
        value["sources"][0]["loader"]["reason"],
        "dependency_unavailable"
    );
    assert_eq!(
        value["sources"][0]["dependency_closure"],
        json!({"state": "unavailable", "reasons": ["capture_unavailable"]})
    );
}

#[test]
fn optional_missing_dependency_makes_source_and_clip_incomplete() {
    let temp = tempfile::tempdir().unwrap();
    let mut source: Value =
        serde_json::from_slice(&fs::read(spike_path("source/walk-a.gltf")).unwrap()).unwrap();
    source["images"] = json!([{"uri": "missing.png"}]);
    source["animations"].as_array_mut().unwrap().push(json!({
        "name": "Take 002",
        "samplers": [],
        "channels": []
    }));
    fs::write(
        temp.path().join("source.gltf"),
        serde_json::to_vec(&source).unwrap(),
    )
    .unwrap();
    let manifest = temp.path().join("collection.toml");
    fs::write(
        &manifest,
        r#"schema = "urn:animsmith:schema:collection-manifest:1"
schema_version = 1
collection_id = "com.example.partial-dependency"
[[sources]]
key = "source"
path = "source.gltf"
[[clips]]
id = "com.example.partial-dependency/take"
source = "source"
take_index = 0
take_name = "Take 001"
[[clips]]
id = "com.example.partial-dependency/take-2"
source = "source"
take_index = 1
take_name = "Take 002"
[[runtime_sets]]
id = "com.example.partial-dependency/set"
kind = "sync-group"
members = [
  "com.example.partial-dependency/take",
  "com.example.partial-dependency/take-2",
]
"#,
    )
    .unwrap();

    let output = collection(&manifest);
    assert_eq!(output.status.code(), Some(1));
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_schema(&value);
    assert_eq!(value["sources"][0]["loader"]["state"], "ready");
    assert_eq!(value["sources"][0]["result"]["state"], "available");
    assert_eq!(
        value["sources"][0]["dependency_closure"],
        json!({"state": "partial", "reasons": ["unavailable_resource"]})
    );
    assert_eq!(value["summary"]["sources"], 1);
    assert_eq!(value["summary"]["established_sources"], 0);
    assert_eq!(value["summary"]["established_clips"], 0);
    assert_eq!(value["summary"]["complete_runtime_sets"], 0);
    assert_eq!(value["summary"]["incomplete"], true);
    let clips = value["clips"].as_array().unwrap();
    assert_eq!(clips.len(), 2);
    for clip in clips {
        assert_eq!(
            clip["binding"],
            json!({"state": "unavailable", "reason": "dependency_closure_incomplete"})
        );
    }
    let set = &value["runtime_sets"][0];
    assert_eq!(set["kind"], "sync-group");
    assert_eq!(set["members"].as_array().unwrap().len(), 2);
    assert_eq!(set["lifecycle"], "incomplete");
    assert_eq!(
        set["gaps"],
        json!([
            "com.example.partial-dependency/take",
            "com.example.partial-dependency/take-2"
        ])
    );
    assert_eq!(set["evidence"]["root_travel"]["members_measured"], 0);
    for member in set["members"].as_array().unwrap() {
        assert_eq!(
            member["resolution"],
            json!({"state": "unavailable", "reason": "dependency_closure_incomplete"})
        );
    }
}

#[test]
fn external_animation_bytes_change_closure_identity_and_measurement_not_primary_identity() {
    let temp = tempfile::tempdir().unwrap();
    let source_path = temp.path().join("source.gltf");
    let buffer_path = temp.path().join("animation.bin");
    let source = json!({
        "asset": {"version": "2.0"},
        "buffers": [{"uri": "animation.bin", "byteLength": 48}],
        "bufferViews": [
            {"buffer": 0, "byteOffset": 0, "byteLength": 12},
            {"buffer": 0, "byteOffset": 12, "byteLength": 36}
        ],
        "accessors": [
            {"bufferView": 0, "componentType": 5126, "count": 3, "type": "SCALAR", "min": [0.0], "max": [1.0]},
            {"bufferView": 1, "componentType": 5126, "count": 3, "type": "VEC3"}
        ],
        "nodes": [{"name": "root", "skin": 0}],
        "skins": [{"joints": [0]}],
        "animations": [{
            "name": "Take 001",
            "samplers": [{"input": 0, "output": 1, "interpolation": "LINEAR"}],
            "channels": [{"sampler": 0, "target": {"node": 0, "path": "translation"}}]
        }],
        "scenes": [{"nodes": [0]}],
        "scene": 0
    });
    let source_bytes = serde_json::to_vec(&source).unwrap();
    let source_identity = animsmith_core::InputIdentity::from_bytes(&source_bytes);
    fs::write(&source_path, &source_bytes).unwrap();
    let manifest = temp.path().join("collection.toml");
    fs::write(
        &manifest,
        format!(
            r#"schema = "urn:animsmith:schema:collection-manifest:1"
schema_version = 1
collection_id = "com.example.external-animation"
[[sources]]
key = "source"
path = "source.gltf"
expected_sha256 = "{}"
[[clips]]
id = "com.example.external-animation/take"
source = "source"
take_index = 0
take_name = "Take 001"
"#,
            source_identity.sha256()
        ),
    )
    .unwrap();

    let write_buffer = |endpoint_x: f32| {
        let values = [
            0.0_f32,
            0.5,
            1.0,
            0.0,
            0.0,
            0.0,
            endpoint_x / 2.0,
            0.0,
            0.0,
            endpoint_x,
            0.0,
            0.0,
        ];
        let bytes = values
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect::<Vec<_>>();
        fs::write(&buffer_path, bytes).unwrap();
    };
    write_buffer(1.0);
    let first_output = collection(&manifest);
    assert_eq!(first_output.status.code(), Some(0));
    let first: Value = serde_json::from_slice(&first_output.stdout).unwrap();
    assert_schema(&first);

    write_buffer(2.0);
    let second_output = collection(&manifest);
    assert_eq!(second_output.status.code(), Some(0));
    let second: Value = serde_json::from_slice(&second_output.stdout).unwrap();
    assert_schema(&second);

    assert_eq!(
        first["sources"][0]["input"], second["sources"][0]["input"],
        "only the external buffer changed"
    );
    assert_eq!(first["sources"][0]["digest"]["state"], "matched");
    assert_eq!(second["sources"][0]["digest"]["state"], "matched");
    assert_ne!(
        first["sources"][0]["dependency_closure"]["identity"],
        second["sources"][0]["dependency_closure"]["identity"],
        "the complete dependency closure binds external animation bytes"
    );
    let position_delta = |value: &Value| {
        value["clips"][0]["binding"]["measurements"]["loop_continuity"]["bones"][0]
            ["position_delta_m"]
            .as_f64()
            .unwrap()
    };
    assert_eq!(position_delta(&first), 1.0);
    assert_eq!(position_delta(&second), 2.0);
}

#[test]
fn data_failures_are_distinct_and_later_sources_continue() {
    let temp = tempfile::tempdir().unwrap();
    let valid = fs::read(spike_path("source/walk-a.gltf")).unwrap();
    fs::write(temp.path().join("valid.gltf"), &valid).unwrap();
    fs::write(temp.path().join("drift.gltf"), &valid).unwrap();
    fs::write(temp.path().join("name.gltf"), &valid).unwrap();
    fs::write(temp.path().join("bad.gltf"), b"not glTF").unwrap();
    fs::write(temp.path().join("unknown.xyz"), b"readable unknown bytes").unwrap();
    let manifest = temp.path().join("collection.toml");
    fs::write(
        &manifest,
        r#"schema = "urn:animsmith:schema:collection-manifest:1"
schema_version = 1
collection_id = "com.example.failures"

[[sources]]
key = "bad"
path = "bad.gltf"

[[sources]]
key = "drift"
path = "drift.gltf"
expected_sha256 = "0000000000000000000000000000000000000000000000000000000000000000"

[[sources]]
key = "missing"
path = "missing.gltf"

[[sources]]
key = "name"
path = "name.gltf"

[[sources]]
key = "unknown"
path = "unknown.xyz"

[[sources]]
key = "valid"
path = "valid.gltf"

[[clips]]
id = "com.example.failures/bad"
source = "bad"
take_index = 0
take_name = "Take 001"

[[clips]]
id = "com.example.failures/digest"
source = "drift"
take_index = 0
take_name = "Take 001"

[[clips]]
id = "com.example.failures/missing"
source = "missing"
take_index = 0
take_name = "Take 001"

[[clips]]
id = "com.example.failures/name"
source = "name"
take_index = 0
take_name = "Wrong Name"

[[clips]]
id = "com.example.failures/unknown"
source = "unknown"
take_index = 0
take_name = "Take 001"

[[clips]]
id = "com.example.failures/valid"
source = "valid"
take_index = 0
take_name = "Take 001"

[[runtime_sets]]
id = "com.example.failures/sets/all"
kind = "sync-group"
members = ["com.example.failures/bad", "com.example.failures/digest", "com.example.failures/missing", "com.example.failures/name", "com.example.failures/unknown", "com.example.failures/valid"]
"#,
    )
    .unwrap();

    let output = collection(&manifest);
    assert_eq!(
        output.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_schema(&value);
    assert!(output.stderr.is_empty(), "typed data failures stay in JSON");
    let json = String::from_utf8_lossy(&output.stdout);
    assert!(!json.contains(temp.path().to_string_lossy().as_ref()));
    assert!(!json.contains("No such file"));
    assert!(!json.contains("os error"));
    let sources = value["sources"].as_array().unwrap();
    assert_eq!(sources[0]["loader"]["reason"], "malformed_input");
    assert_eq!(sources[1]["digest"]["state"], "mismatched");
    assert_eq!(sources[2]["input"]["reason"], "missing");
    assert_eq!(sources[4]["loader"]["reason"], "unsupported_format");
    assert_eq!(sources[5]["result"]["state"], "available");
    assert_eq!(sources[5]["result"]["envelope"]["schema"], OUTPUT_SCHEMA_ID);
    assert_eq!(sources[5]["result"]["envelope"]["command"], "lint");
    assert_eq!(
        value["clips"][1]["binding"]["reason"], "digest_mismatched",
        "a matching exact take remains quarantined by digest drift"
    );
    assert_eq!(
        value["clips"][3]["binding"]["reason"], "take_name_mismatched",
        "take-name drift remains a distinct binding failure"
    );
    assert_eq!(value["clips"][5]["binding"]["state"], "established");
    assert_eq!(value["runtime_sets"][0]["lifecycle"], "incomplete");
    assert_eq!(
        value["runtime_sets"][0]["members"][0]["root_travel"]["translation_availability"],
        "unavailable"
    );
    assert!(
        value["runtime_sets"][0]["members"][0]["root_travel"]
            .get("duration_s")
            .is_none(),
        "binding-unavailable rows remain explicit but do not invent duration"
    );
    assert_eq!(
        value["runtime_sets"][0]["evidence"]["root_travel"]["lifecycle"],
        "incomplete"
    );
    assert_eq!(
        value["runtime_sets"][0]["members"]
            .as_array()
            .unwrap()
            .len(),
        6
    );
    assert_eq!(
        value["runtime_sets"][0]["members"][5]["id"],
        "com.example.failures/valid"
    );
    assert_eq!(
        value["runtime_sets"][0]["members"][5]["resolution"]["state"],
        "established"
    );
    assert_eq!(
        value["runtime_sets"][0]["gaps"].as_array().unwrap().len(),
        5
    );
}

#[test]
fn duplicate_take_names_keep_indexed_measurements_without_guessing_check_refs() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("duplicate.gltf"),
        br#"{"asset":{"version":"2.0"},"nodes":[{"name":"Root"}],"scenes":[{"nodes":[0]}],"scene":0,"animations":[{"name":"Take 001","channels":[],"samplers":[]},{"name":"Take 001","channels":[],"samplers":[]}]}"#,
    )
    .unwrap();
    let manifest = temp.path().join("collection.toml");
    fs::write(
        &manifest,
        r#"schema = "urn:animsmith:schema:collection-manifest:1"
schema_version = 1
collection_id = "com.example.duplicates"

[[sources]]
key = "takes"
path = "duplicate.gltf"

[[clips]]
id = "com.example.duplicates/first"
source = "takes"
take_index = 0
take_name = "Take 001"

[[clips]]
id = "com.example.duplicates/second"
source = "takes"
take_index = 1
take_name = "Take 001"

[[runtime_sets]]
id = "com.example.duplicates/sets/pair"
kind = "sync-group"
members = ["com.example.duplicates/second", "com.example.duplicates/first"]
"#,
    )
    .unwrap();
    let output = collection(&manifest);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_schema(&value);
    assert_eq!(value["clips"][0]["binding"]["normalized_clip_index"], 0);
    assert_eq!(value["clips"][1]["binding"]["normalized_clip_index"], 1);
    assert_eq!(
        value["clips"][0]["binding"]["observed_take_name"],
        "Take 001"
    );
    assert_eq!(
        value["clips"][1]["binding"]["observed_take_name"],
        "Take 001"
    );
    assert_eq!(
        value["clips"][0]["binding"]["check_reference"]["state"],
        "available"
    );
    assert_eq!(
        value["clips"][1]["binding"]["check_reference"]["state"],
        "available"
    );
    assert_ne!(
        value["clips"][0]["binding"]["check_reference"]["reference"]["measurement_key"],
        value["clips"][1]["binding"]["check_reference"]["reference"]["measurement_key"],
        "loader-normalized duplicate names must remain distinct in name-keyed output-v10"
    );
    assert_eq!(value["runtime_sets"][0]["lifecycle"], "complete");
    assert_eq!(
        value["runtime_sets"][0]["members"][0]["id"],
        "com.example.duplicates/second"
    );
}

#[test]
fn maximum_authored_duplicate_name_keeps_schema_valid_normalized_keys() {
    let temp = tempfile::tempdir().unwrap();
    let take_name = "x".repeat(4_096);
    let source = json!({
        "asset": {"version": "2.0"},
        "nodes": [{"name": "Root"}],
        "scenes": [{"nodes": [0]}],
        "scene": 0,
        "animations": [
            {"name": take_name.clone(), "channels": [], "samplers": []},
            {"name": take_name.clone(), "channels": [], "samplers": []}
        ]
    });
    fs::write(
        temp.path().join("duplicate.gltf"),
        serde_json::to_vec(&source).unwrap(),
    )
    .unwrap();
    let manifest = temp.path().join("collection.toml");
    fs::write(
        &manifest,
        format!(
            r#"schema = "urn:animsmith:schema:collection-manifest:1"
schema_version = 1
collection_id = "com.example.max-duplicate"

[[sources]]
key = "takes"
path = "duplicate.gltf"

[[clips]]
id = "com.example.max-duplicate/first"
source = "takes"
take_index = 0
take_name = "{take_name}"

[[clips]]
id = "com.example.max-duplicate/second"
source = "takes"
take_index = 1
take_name = "{take_name}"
"#
        ),
    )
    .unwrap();

    let output = collection(&manifest);
    assert_eq!(
        output.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_schema(&value);
    assert_eq!(
        value["sources"][0]["observed_takes"][1]["normalized"]["name"]
            .as_str()
            .unwrap()
            .len(),
        4_098
    );
    assert_eq!(
        value["sources"][0]["result"]["reason"],
        "nested_output_unavailable"
    );
    assert_eq!(value["clips"][0]["binding"]["state"], "established");
    assert_eq!(
        value["clips"][1]["binding"]["check_reference"]["reason"],
        "nested_output_unavailable"
    );
}

#[test]
fn shuffled_declarations_change_only_the_raw_manifest_identity() {
    let source = fs::read(spike_path("source/walk-a.gltf")).unwrap();
    let manifests = [
        r#"schema = "urn:animsmith:schema:collection-manifest:1"
schema_version = 1
collection_id = "com.example.canonical"
[[sources]]
key = "a"
path = "a.gltf"
[[sources]]
key = "b"
path = "b.gltf"
[[clips]]
id = "com.example.canonical/a"
source = "a"
take_index = 0
take_name = "Take 001"
[[clips]]
id = "com.example.canonical/b"
source = "b"
take_index = 0
take_name = "Take 001"
[[runtime_sets]]
id = "com.example.canonical/sets/pair"
kind = "sync-group"
members = ["com.example.canonical/b", "com.example.canonical/a"]
"#,
        r#"schema_version = 1
schema = "urn:animsmith:schema:collection-manifest:1"
collection_id = "com.example.canonical"
[[sources]]
key = "b"
path = "b.gltf"
[[sources]]
key = "a"
path = "a.gltf"
[[clips]]
id = "com.example.canonical/b"
source = "b"
take_index = 0
take_name = "Take 001"
[[clips]]
id = "com.example.canonical/a"
source = "a"
take_index = 0
take_name = "Take 001"
[[runtime_sets]]
id = "com.example.canonical/sets/pair"
kind = "sync-group"
members = ["com.example.canonical/b", "com.example.canonical/a"]
"#,
    ];
    let mut outputs = Vec::new();
    for manifest_text in manifests {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("a.gltf"), &source).unwrap();
        fs::write(temp.path().join("b.gltf"), &source).unwrap();
        let path = temp.path().join("collection.toml");
        fs::write(&path, manifest_text).unwrap();
        let output = collection(&path);
        assert_eq!(output.status.code(), Some(0));
        outputs.push(serde_json::from_slice::<Value>(&output.stdout).unwrap());
    }
    assert_ne!(
        outputs[0]["manifest"]["input"],
        outputs[1]["manifest"]["input"]
    );
    for output in &mut outputs {
        output["manifest"]["input"] = json!({"sha256":"manifest-specific","bytes":0});
    }
    assert_eq!(outputs[0], outputs[1]);
}
