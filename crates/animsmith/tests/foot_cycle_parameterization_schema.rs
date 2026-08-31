use jsonschema::Validator;

const SCHEMA: &str =
    include_str!("../../../docs/schemas/foot-cycle-parameterization-v1.schema.json");
const VALID: &str = r#"schema = "urn:animsmith:schema:foot-cycle-parameterization:1"
schema_version = 1
runtime_set_id = "com.example/sets/walk"
reference_member = "com.example/walk-forward"
output_directory = "generated/walk-aligned"
minimum_segment_slope = 0.5
maximum_segment_slope = 2.0

[proof]
max_gait_phase_spread = 0.08
min_lr_amplitude_m = 0.05
max_contact_boundary_phase_error = 0.01

[manifest]
schema = "urn:animsmith:schema:collection-manifest:1"
schema_version = 1
collection_id = "com.example"

[manifest.input]
sha256 = "0000000000000000000000000000000000000000000000000000000000000000"
bytes = 1024

[[members]]
id = "com.example/walk-forward"
contact_fragment = "contacts/walk-forward.json"

[[members]]
id = "com.example/walk-right"
contact_fragment = "contacts/walk-right.json"
"#;

fn compiled_schema() -> Validator {
    let schema: serde_json::Value = serde_json::from_str(SCHEMA).expect("schema JSON");
    Validator::new(&schema).expect("foot-cycle parameterization schema compiles")
}

fn decoded() -> serde_json::Value {
    serde_json::to_value(toml::from_str::<toml::Value>(VALID).expect("valid TOML"))
        .expect("JSON-transcoded TOML")
}

#[test]
fn strict_toml_shape_satisfies_published_schema() {
    compiled_schema()
        .validate(&decoded())
        .expect("fixture satisfies decoded TOML schema");
}

#[test]
fn schema_rejects_unknown_fields_at_every_level() {
    let validator = compiled_schema();
    for pointer in ["", "/proof", "/manifest", "/manifest/input", "/members/0"] {
        let mut candidate = decoded();
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
}

#[test]
fn schema_requires_every_proof_field_and_enforces_ranges() {
    let validator = compiled_schema();
    let mut missing_table = decoded();
    missing_table.as_object_mut().unwrap().remove("proof");
    assert!(validator.validate(&missing_table).is_err());

    for field in [
        "max_gait_phase_spread",
        "min_lr_amplitude_m",
        "max_contact_boundary_phase_error",
    ] {
        let mut candidate = decoded();
        candidate["proof"].as_object_mut().unwrap().remove(field);
        assert!(validator.validate(&candidate).is_err(), "missing {field}");
    }

    for (field, value) in [
        ("max_gait_phase_spread", -0.000_000_1),
        ("max_gait_phase_spread", 0.500_000_1),
        ("min_lr_amplitude_m", -0.000_000_1),
        ("max_contact_boundary_phase_error", -0.000_000_1),
        ("max_contact_boundary_phase_error", 0.500_000_1),
    ] {
        let mut candidate = decoded();
        candidate["proof"][field] = serde_json::json!(value);
        assert!(validator.validate(&candidate).is_err(), "invalid {field}");
    }

    let mut exact = decoded();
    exact["proof"]["max_gait_phase_spread"] = serde_json::json!(0.5);
    exact["proof"]["min_lr_amplitude_m"] = serde_json::json!(0.0);
    exact["proof"]["max_contact_boundary_phase_error"] = serde_json::json!(0.5);
    validator.validate(&exact).expect("inclusive boundaries");
}
