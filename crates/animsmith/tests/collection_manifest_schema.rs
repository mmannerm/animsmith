use jsonschema::Validator;
use std::path::PathBuf;
use std::process::Command;

const SCHEMA: &str = include_str!("../../../docs/schemas/collection-manifest-v1.schema.json");
const VALID_MANIFEST: &str = include_str!("../testdata/collection-spike/collection.toml");

fn spike_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("testdata/collection-spike")
        .join(relative)
}

fn compiled_schema() -> Validator {
    let schema: serde_json::Value = serde_json::from_str(SCHEMA).expect("schema JSON");
    Validator::new(&schema).expect("collection manifest schema compiles")
}

#[test]
fn retained_toml_shape_satisfies_published_schema() {
    let decoded: toml::Value = toml::from_str(VALID_MANIFEST).expect("retained manifest TOML");
    let decoded_json = serde_json::to_value(decoded).expect("JSON-transcoded TOML");
    compiled_schema()
        .validate(&decoded_json)
        .expect("retained manifest satisfies decoded TOML schema");
}

#[test]
fn published_schema_rejects_unknown_fields_at_every_level() {
    let decoded: toml::Value = toml::from_str(VALID_MANIFEST).expect("retained manifest TOML");
    let mut decoded_json = serde_json::to_value(decoded).expect("JSON-transcoded TOML");
    let validator = compiled_schema();

    for pointer in ["", "/sources/0", "/clips/0", "/runtime_sets/0"] {
        let mut candidate = decoded_json.clone();
        candidate
            .pointer_mut(pointer)
            .expect("test pointer")
            .as_object_mut()
            .expect("test object")
            .insert("unknown".to_owned(), serde_json::json!(true));
        assert!(
            validator.validate(&candidate).is_err(),
            "unknown field at {pointer:?} must fail"
        );
    }

    decoded_json
        .as_object_mut()
        .expect("manifest object")
        .insert("schema_version".to_owned(), serde_json::json!(2));
    assert!(validator.validate(&decoded_json).is_err());
}

#[test]
fn ordinary_lint_does_not_implicitly_activate_collection_semantics() {
    let output = Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .args([
            "lint",
            spike_path("collection.toml").to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .expect("ordinary lint runs");
    assert_eq!(output.status.code(), Some(2));
    assert!(
        output.stdout.is_empty(),
        "ordinary operator errors retain no-output behavior"
    );
}

#[test]
fn ordinary_explicit_config_and_lint_output_contract_are_unchanged() {
    let output = Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .args([
            "--config",
            spike_path("fixture.animsmith.toml").to_str().unwrap(),
            "lint",
            spike_path("source/walk-a.gltf").to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .expect("ordinary configured lint runs");
    assert!(
        matches!(output.status.code(), Some(0 | 1)),
        "ordinary lint should complete with evidence: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("lint JSON");
    assert_eq!(value["schema"], "urn:animsmith:schema:output:14");
    assert_eq!(
        value["files"][0]["measurements"]["schema"],
        "urn:animsmith:schema:measurements:16"
    );
}
