use animsmith_core::ContactFragmentV1;
use jsonschema::Validator;
use serde_json::json;

const SCHEMA: &str = include_str!("../../../docs/schemas/contact-fragment-v1.schema.json");

fn validator() -> Validator {
    let schema = serde_json::from_str(SCHEMA).expect("contact-fragment schema JSON");
    Validator::new(&schema).expect("contact-fragment schema compiles")
}

fn identity(character: char) -> serde_json::Value {
    json!({"sha256": character.to_string().repeat(64), "bytes": 1})
}

fn valid() -> serde_json::Value {
    json!({
        "schema": "urn:animsmith:schema:contact-fragment:1",
        "schema_version": 1,
        "producer": {"tool": "animsmith", "version": "0.6.0"},
        "artifact": identity('a'),
        "dependency_closure_identity": identity('b'),
        "clip": {"scope": "document", "clip_name": "walk"},
        "duration_s": 1.0,
        "events": [],
    })
}

fn number(spelling: &str) -> serde_json::Value {
    serde_json::from_str(spelling).unwrap()
}

#[test]
fn published_schema_matches_the_strict_core_shape_and_numeric_limits() {
    let validator = validator();
    validator
        .validate(&valid())
        .expect("valid core fixture satisfies schema");

    let mut event_shape = valid();
    event_shape["events"] = json!([{
        "event_id":"x", "role":"left_foot", "phase":"marker", "time":0.0,
        "window":{"start":0.0,"end":0.0}
    }]);
    assert!(!validator.is_valid(&event_shape));

    let mut unsafe_bytes = valid();
    unsafe_bytes["artifact"]["bytes"] = 9_007_199_254_740_992_u64.into();
    assert!(!validator.is_valid(&unsafe_bytes));

    let mut unsafe_duration = valid();
    unsafe_duration["duration_s"] = 1.5e16.into();
    assert!(!validator.is_valid(&unsafe_duration));

    let mut wide_identifier = valid();
    wide_identifier["events"] = json!([{
        "event_id": "\u{10000}".repeat(255),
        "role":"body", "phase":"marker", "time":0.0
    }]);
    assert!(
        validator.is_valid(&wide_identifier),
        "JSON Schema maxLength counts code points; the strict reader separately caps UTF-8 bytes"
    );
}

#[test]
fn schema_and_reader_accept_the_same_integer_valued_decimal_aliases() {
    let validator = validator();
    for spelling in ["1.0", "1e0"] {
        let mut value = valid();
        value["schema_version"] = number(spelling);
        value["artifact"]["bytes"] = number(spelling);
        value["dependency_closure_identity"]["bytes"] = number(spelling);
        value["clip"] = json!({
            "scope":"collection", "logical_id":"walk", "source":"source",
            "take_index":number(spelling), "take_name":"Walk"
        });
        value["extensions"] = json!([{
            "schema":"urn:example:integer", "schema_version":number(spelling), "payload":{}
        }]);
        assert!(validator.is_valid(&value), "schema accepts {spelling}");
        assert!(
            ContactFragmentV1::read_json(&serde_json::to_vec(&value).unwrap()).is_ok(),
            "reader accepts {spelling}"
        );
    }
}

#[test]
fn published_schema_closes_nested_objects_and_forbids_explicit_empty_extensions() {
    let validator = validator();
    let mut unknown = valid();
    unknown["clip"]["unknown"] = true.into();
    assert!(!validator.is_valid(&unknown));

    let mut empty_extensions = valid();
    empty_extensions["extensions"] = json!([]);
    assert!(!validator.is_valid(&empty_extensions));

    let mut overflowing_u32 = valid();
    overflowing_u32["clip"] = json!({
        "scope":"collection", "logical_id":"clip", "source":"source",
        "take_index": 4_294_967_296_u64, "take_name":"take"
    });
    assert!(!validator.is_valid(&overflowing_u32));

    let mut unsafe_payload_number = valid();
    unsafe_payload_number["extensions"] = json!([{
        "schema":"urn:example:payload", "schema_version":1,
        "payload":{"nested":{"number":1.5e16}}
    }]);
    assert!(!validator.is_valid(&unsafe_payload_number));

    let mut oversized_payload_text = valid();
    oversized_payload_text["extensions"] = json!([{
        "schema":"urn:example:payload", "schema_version":1,
        "payload":{"text":"x".repeat(4097)}
    }]);
    assert!(!validator.is_valid(&oversized_payload_text));
}
