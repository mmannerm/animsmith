use jsonschema::Validator;
use serde_json::{Value, json};

const RESULT_SCHEMA: &str =
    include_str!("../../../docs/schemas/contact-transform-result-v1.schema.json");
const FRAGMENT_SCHEMA: &str = include_str!("../../../docs/schemas/contact-fragment-v1.schema.json");

fn validator() -> Validator {
    let result: Value = serde_json::from_str(RESULT_SCHEMA).expect("result schema JSON");
    let fragment: Value = serde_json::from_str(FRAGMENT_SCHEMA).expect("fragment schema JSON");
    let registry = jsonschema::Registry::new()
        .add("urn:animsmith:schema:contact-fragment:1", fragment)
        .expect("fragment schema identity")
        .prepare()
        .expect("fragment schema registry");
    jsonschema::options()
        .with_registry(&registry)
        .build(&result)
        .expect("contact transform schema compiles")
}

fn identity(character: char) -> Value {
    json!({"sha256": character.to_string().repeat(64), "bytes": 1})
}

fn fragment() -> Value {
    json!({
        "schema": "urn:animsmith:schema:contact-fragment:1",
        "schema_version": 1,
        "producer": {"tool": "fixture", "version": "1"},
        "artifact": identity('d'),
        "dependency_closure_identity": identity('e'),
        "clip": {"scope": "document", "clip_name": "walk"},
        "duration_s": 1.0,
        "events": []
    })
}

fn success() -> Value {
    json!({
        "schema": "urn:animsmith:schema:contact-transform-result:1",
        "schema_version": 1,
        "operation": {"kind": "time_warp", "version": 1, "output_duration_s": 1.0,
            "control_points": [{"input_time": 0.0, "output_time": 0.0}, {"input_time": 1.0, "output_time": 1.0}]},
        "input": {"artifact": identity('a'), "dependency_closure_identity": identity('b'), "fragment": identity('c')},
        "outcome": "transformed",
        "event_outcomes": [],
        "output": {"artifact": identity('d'), "dependency_closure_identity": identity('e'), "fragment": identity('f'), "contact_fragment": fragment()}
    })
}

#[test]
fn schema_closes_success_refusal_and_nested_fragment_shapes() {
    let validator = validator();
    assert!(validator.is_valid(&success()));

    let mut missing_output = success();
    missing_output.as_object_mut().unwrap().remove("output");
    assert!(!validator.is_valid(&missing_output));

    let mut success_with_refusal = success();
    success_with_refusal["refusal"] = json!({"code": "invalid_mapping", "message": "no"});
    assert!(!validator.is_valid(&success_with_refusal));

    let mut invalid_nested_fragment = success();
    invalid_nested_fragment["output"]["contact_fragment"]["unknown"] = true.into();
    assert!(!validator.is_valid(&invalid_nested_fragment));

    let refusal = json!({
        "schema": "urn:animsmith:schema:contact-transform-result:1",
        "schema_version": 1,
        "operation": {"kind": "resample", "version": 1, "mapping": "identity"},
        "input": {"artifact": identity('a'), "dependency_closure_identity": identity('b'), "fragment": identity('c')},
        "outcome": "refused",
        "event_outcomes": [],
        "refusal": {"code": "invalid_binding", "message": "stale"}
    });
    assert!(validator.is_valid(&refusal));
}

#[test]
fn schema_caps_rows_and_closes_operation_and_event_values() {
    let validator = validator();
    let mut overflow = success();
    overflow["operation"]["control_points"] = Value::Array(
        (0..=4096)
            .map(|index| json!({"input_time": index, "output_time": index}))
            .collect(),
    );
    assert!(!validator.is_valid(&overflow));

    let mut unknown_operation_field = success();
    unknown_operation_field["operation"]["smoothing"] = true.into();
    assert!(!validator.is_valid(&unknown_operation_field));

    let mut malformed_event_value = success();
    malformed_event_value["event_outcomes"] = json!([{
        "event_id": "left/0", "outcome": "transformed", "value": {"time": 0.5, "window": {"start": 0.0, "end": 1.0}}
    }]);
    assert!(!validator.is_valid(&malformed_event_value));
}
