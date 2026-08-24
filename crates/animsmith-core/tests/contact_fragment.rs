use animsmith_core::{
    CONTACT_FRAGMENT_V1_MAX_EVENTS, CONTACT_FRAGMENT_V1_MAX_EXTENSION_PAYLOAD_BYTES,
    CONTACT_FRAGMENT_V1_MAX_EXTENSIONS, CONTACT_FRAGMENT_V1_MAX_IDENTIFIER_BYTES,
    CONTACT_FRAGMENT_V1_MAX_SAFE_INTEGER, CONTACT_FRAGMENT_V1_MAX_SOURCE_BYTES,
    CONTACT_FRAGMENT_V1_MAX_TEXT_BYTES, ContactClipReferenceV1, ContactExtensionV1,
    ContactFragmentError, ContactFragmentV1, ContactProducerV1, DependencyClosureIdentityV1,
    InputIdentity,
};
use serde_json::{Value, json};

type Mutation = Box<dyn Fn(&mut Value)>;

fn identity(ch: char) -> Value {
    json!({"sha256": ch.to_string().repeat(64), "bytes": 1})
}

fn valid() -> Value {
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

fn read(value: Value) -> Result<ContactFragmentV1, ContactFragmentError> {
    ContactFragmentV1::read_json(&serde_json::to_vec(&value).unwrap())
}

fn closure_identity(bytes: u64) -> DependencyClosureIdentityV1 {
    serde_json::from_value(json!({
        "sha256": "b".repeat(64),
        "bytes": bytes,
    }))
    .unwrap()
}

fn number(spelling: &str) -> Value {
    serde_json::from_str(spelling).unwrap()
}

fn raw_fragment(duration: &str, payload_number: &str) -> Vec<u8> {
    r#"{"schema":"urn:animsmith:schema:contact-fragment:1","schema_version":1,"producer":{"tool":"animsmith","version":"0.6.0"},"artifact":{"sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","bytes":1},"dependency_closure_identity":{"sha256":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","bytes":1},"clip":{"scope":"document","clip_name":"walk"},"duration_s":__DURATION__,"events":[],"extensions":[{"schema":"urn:example:float","schema_version":1,"payload":{"n":__PAYLOAD__}}]}"#
        .replace("__DURATION__", duration)
        .replace("__PAYLOAD__", payload_number)
        .into_bytes()
}

#[test]
fn accepts_exact_event_and_extension_limits_and_refuses_n_plus_one() {
    let mut value = valid();
    value["events"] = Value::Array(
        (0..CONTACT_FRAGMENT_V1_MAX_EVENTS)
            .map(|index| {
                json!({"event_id": format!("id-{index}"), "role":"left_foot", "phase":"marker", "time": 0.0})
            })
            .collect(),
    );
    value["extensions"] = Value::Array(
        (0..CONTACT_FRAGMENT_V1_MAX_EXTENSIONS)
            .map(|index| json!({"schema": format!("urn:example:{index}"), "schema_version":1, "payload": {}}))
            .collect(),
    );
    assert!(read(value.clone()).is_ok());
    value["events"]
        .as_array_mut()
        .unwrap()
        .push(json!({"event_id":"too-many", "role":"left_foot", "phase":"marker", "time":0.0}));
    assert!(matches!(
        read(value),
        Err(ContactFragmentError::LimitExceeded {
            field: "events",
            ..
        })
    ));

    let mut extensions = valid();
    extensions["extensions"] = Value::Array(
        (0..=CONTACT_FRAGMENT_V1_MAX_EXTENSIONS)
            .map(|index| json!({"schema": format!("urn:example:{index}"), "schema_version":1, "payload": {}}))
            .collect(),
    );
    assert!(matches!(
        read(extensions),
        Err(ContactFragmentError::LimitExceeded {
            field: "extensions",
            ..
        })
    ));
}

#[test]
fn source_text_and_identifier_limits_accept_exact_n_and_refuse_n_plus_one() {
    let mut exact_source = serde_json::to_vec(&valid()).unwrap();
    exact_source.extend(std::iter::repeat_n(
        b' ',
        CONTACT_FRAGMENT_V1_MAX_SOURCE_BYTES - exact_source.len(),
    ));
    assert_eq!(exact_source.len(), CONTACT_FRAGMENT_V1_MAX_SOURCE_BYTES);
    assert!(ContactFragmentV1::read_json(&exact_source).is_ok());
    exact_source.push(b' ');
    assert!(matches!(
        ContactFragmentV1::read_json(&exact_source),
        Err(ContactFragmentError::SourceTooLarge { .. })
    ));

    for (field, exact, over) in [
        (
            "clip_name",
            CONTACT_FRAGMENT_V1_MAX_TEXT_BYTES,
            CONTACT_FRAGMENT_V1_MAX_TEXT_BYTES + 1,
        ),
        (
            "event_id",
            CONTACT_FRAGMENT_V1_MAX_IDENTIFIER_BYTES,
            CONTACT_FRAGMENT_V1_MAX_IDENTIFIER_BYTES + 1,
        ),
    ] {
        let mut value = valid();
        if field == "clip_name" {
            value["clip"][field] = "x".repeat(exact).into();
        } else {
            value["events"] =
                json!([{"event_id":"x", "role":"body", "phase":"marker", "time":0.0}]);
            value["events"][0][field] = "x".repeat(exact).into();
        }
        assert!(
            read(value.clone()).is_ok(),
            "{field} accepts exact V1 bound"
        );
        if field == "clip_name" {
            value["clip"][field] = "x".repeat(over).into();
        } else {
            value["events"][0][field] = "x".repeat(over).into();
        }
        assert!(read(value).is_err(), "{field} refuses N+1");
    }
}

#[test]
fn semantic_refusal_matrix_covers_identity_events_and_collection_witnesses() {
    let cases: Vec<(&str, Mutation)> = vec![
        (
            "upper identity hex",
            Box::new(|v| v["artifact"]["sha256"] = "A".repeat(64).into()),
        ),
        (
            "duplicate event ids",
            Box::new(|v| {
                v["events"] = json!([
                    {"event_id":"same", "role":"body", "phase":"marker", "time":0.0},
                    {"event_id":"same", "role":"body", "phase":"marker", "time":0.1}
                ])
            }),
        ),
        (
            "unknown role",
            Box::new(|v| {
                v["events"] = json!([{"event_id":"x", "role":"wing", "phase":"marker", "time":0.0}])
            }),
        ),
        (
            "unknown phase",
            Box::new(|v| {
                v["events"] = json!([{"event_id":"x", "role":"body", "phase":"land", "time":0.0}])
            }),
        ),
        (
            "bad confidence",
            Box::new(
                |v| v["events"] = json!([{"event_id":"x", "role":"body", "phase":"marker", "time":0.0, "confidence":1.01}]),
            ),
        ),
        (
            "reversed window",
            Box::new(
                |v| v["events"] = json!([{"event_id":"x", "role":"body", "phase":"marker", "window":{"start":0.8,"end":0.2}}]),
            ),
        ),
        (
            "nonpositive duration",
            Box::new(|v| v["duration_s"] = 0.0.into()),
        ),
        (
            "bad collection witness",
            Box::new(
                |v| v["clip"] = json!({"scope":"collection","logical_id":"x", "source":"s", "take_index":0, "take_name":""}),
            ),
        ),
    ];
    for (name, mutate) in cases {
        let mut value = valid();
        mutate(&mut value);
        assert!(read(value).is_err(), "{name} must refuse");
    }
}

#[test]
fn extension_payload_and_depth_boundaries_are_exact() {
    let rows = vec!["x".repeat(CONTACT_FRAGMENT_V1_MAX_TEXT_BYTES); 63];
    let mut payload = json!({"rows": rows, "pad": ""});
    let size = serde_jcs::to_vec(&payload).unwrap().len();
    let pad = CONTACT_FRAGMENT_V1_MAX_EXTENSION_PAYLOAD_BYTES - size;
    assert!(pad <= CONTACT_FRAGMENT_V1_MAX_TEXT_BYTES);
    payload["pad"] = "x".repeat(pad).into();
    assert_eq!(
        serde_jcs::to_vec(&payload).unwrap().len(),
        CONTACT_FRAGMENT_V1_MAX_EXTENSION_PAYLOAD_BYTES
    );
    assert!(ContactExtensionV1::new("urn:example:payload", 1, payload.clone()).is_ok());
    let mut reader_exact = valid();
    reader_exact["extensions"] = json!([{
        "schema": "urn:example:payload", "schema_version": 1, "payload": payload.clone()
    }]);
    assert!(
        read(reader_exact).is_ok(),
        "reader accepts exact payload bytes"
    );
    payload["pad"] = "x".repeat(pad + 1).into();
    assert!(ContactExtensionV1::new("urn:example:payload", 1, payload.clone()).is_err());
    let mut reader_over = valid();
    reader_over["extensions"] = json!([{
        "schema": "urn:example:payload", "schema_version": 1, "payload": payload
    }]);
    assert!(
        read(reader_over).is_err(),
        "reader refuses N+1 payload bytes"
    );

    let mut exact = json!("leaf");
    for _ in 0..16 {
        exact = json!({"next": exact});
    }
    assert!(ContactExtensionV1::new("urn:example:depth", 1, exact.clone()).is_ok());
    assert!(
        ContactExtensionV1::new("urn:example:depth", 1, json!({"next": exact.clone()})).is_err()
    );
    let mut reader_exact = valid();
    reader_exact["extensions"] = json!([{
        "schema": "urn:example:depth", "schema_version": 1, "payload": exact
    }]);
    assert!(read(reader_exact).is_ok());
}

#[test]
fn jcs_safe_integer_contract_refuses_noncolliding_values_before_canonicalization() {
    let mut exact = valid();
    exact["artifact"]["bytes"] = CONTACT_FRAGMENT_V1_MAX_SAFE_INTEGER.into();
    assert!(read(exact).is_ok());

    for bytes in [9_007_199_254_740_992_u64, 9_007_199_254_740_993] {
        let mut value = valid();
        value["artifact"]["bytes"] = bytes.into();
        assert!(read(value).is_err(), "{bytes} must not enter the JCS seam");
    }
    let negative: Value = serde_json::from_str(r#"{"n":-9007199254740992}"#).unwrap();
    assert!(ContactExtensionV1::new("urn:example:unsafe", 1, negative).is_err());

    let producer = ContactProducerV1::new("animsmith", "0.6.0").unwrap();
    let clip = ContactClipReferenceV1::document("walk").unwrap();
    let unsafe_identity = InputIdentity::from_sha256_digest([0; 32], 9_007_199_254_740_992);
    assert!(
        ContactFragmentV1::new(
            producer.clone(),
            unsafe_identity,
            closure_identity(1),
            clip.clone(),
            1.0,
            vec![],
            vec![]
        )
        .is_err()
    );
    assert!(
        ContactFragmentV1::new(
            producer,
            InputIdentity::from_sha256_digest([0; 32], 1),
            closure_identity(9_007_199_254_740_993),
            clip,
            1.0,
            vec![],
            vec![],
        )
        .is_err()
    );
}

#[test]
fn integer_valued_decimal_and_exponent_aliases_normalize_at_contact_boundaries() {
    for spelling in ["1.0", "1e0"] {
        let mut value = valid();
        value["schema_version"] = number(spelling);
        value["artifact"]["bytes"] = number(spelling);
        value["dependency_closure_identity"]["bytes"] = number(spelling);
        value["clip"] = json!({
            "scope":"collection", "logical_id":"walk", "source":"source",
            "take_index": number(spelling), "take_name":"Walk"
        });
        value["extensions"] = json!([{
            "schema":"urn:example:integer", "schema_version": number(spelling), "payload":{}
        }]);
        let fragment = read(value).unwrap();
        let canonical = fragment.canonical_json().unwrap();
        assert_eq!(ContactFragmentV1::read_json(&canonical).unwrap(), fragment);
    }

    for spelling in ["1.5", "-1.0", "9007199254740992.0"] {
        let mut artifact = valid();
        artifact["artifact"]["bytes"] = number(spelling);
        assert!(read(artifact).is_err(), "artifact.bytes {spelling}");

        let mut closure = valid();
        closure["dependency_closure_identity"]["bytes"] = number(spelling);
        assert!(read(closure).is_err(), "closure bytes {spelling}");

        let mut schema_version = valid();
        schema_version["schema_version"] = number(spelling);
        assert!(read(schema_version).is_err(), "schema_version {spelling}");

        let mut take_index = valid();
        take_index["clip"] = json!({
            "scope":"collection", "logical_id":"walk", "source":"source",
            "take_index": number(spelling), "take_name":"Walk"
        });
        assert!(read(take_index).is_err(), "take_index {spelling}");

        let mut extension_version = valid();
        extension_version["extensions"] = json!([{
            "schema":"urn:example:integer", "schema_version": number(spelling), "payload":{}
        }]);
        assert!(
            read(extension_version).is_err(),
            "extension schema_version {spelling}"
        );
    }
}

#[test]
fn jcs_safe_number_contract_refuses_large_decimal_and_exponent_floats() {
    let spellings = [
        "9007199254740992.0",
        "9007199254740993e0",
        "9.007199254740993e15",
        "9007199254740992.5",
        "1.5e16",
    ];
    for spelling in spellings {
        for signed in [spelling.to_owned(), format!("-{spelling}")] {
            assert!(matches!(
                ContactFragmentV1::read_json(&raw_fragment(&signed, "1.25")),
                Err(ContactFragmentError::InvalidJson { .. })
            ));
            assert!(matches!(
                ContactFragmentV1::read_json(&raw_fragment("1.25", &signed)),
                Err(ContactFragmentError::InvalidJson { .. })
            ));
            let payload: Value = serde_json::from_str(&format!(r#"{{"n":{signed}}}"#)).unwrap();
            assert!(ContactExtensionV1::new("urn:example:unsafe-float", 1, payload).is_err());
        }
    }

    let producer = ContactProducerV1::new("animsmith", "0.6.0").unwrap();
    assert!(
        ContactFragmentV1::new(
            producer,
            InputIdentity::from_sha256_digest([0; 32], 1),
            closure_identity(1),
            ContactClipReferenceV1::document("walk").unwrap(),
            1.5e16,
            vec![],
            vec![],
        )
        .is_err()
    );

    let fragment = ContactFragmentV1::read_json(&raw_fragment("1.25", "1.25")).unwrap();
    assert_eq!(fragment.duration_s(), 1.25);
    assert_eq!(fragment.producer().tool(), "animsmith");
    assert_eq!(fragment.producer().version(), "0.6.0");
    assert!(ContactExtensionV1::new("urn:example:fraction", 1, json!({"n": 1.25})).is_ok());
}

#[test]
fn canonical_fragment_cap_and_rfc8785_vectors_are_behavioral() {
    let payload = json!({"rows": vec!["x".repeat(CONTACT_FRAGMENT_V1_MAX_TEXT_BYTES); 9]});
    let extensions = (0..CONTACT_FRAGMENT_V1_MAX_EXTENSIONS)
        .map(|index| {
            ContactExtensionV1::new(format!("urn:example:cap:{index}"), 1, payload.clone()).unwrap()
        })
        .collect();
    let result = ContactFragmentV1::new(
        ContactProducerV1::new("animsmith", "0.6.0").unwrap(),
        InputIdentity::from_sha256_digest([0; 32], 1),
        closure_identity(1),
        ContactClipReferenceV1::document("walk").unwrap(),
        1.0,
        vec![],
        extensions,
    );
    assert!(matches!(
        result,
        Err(ContactFragmentError::CanonicalTooLarge { .. })
    ));

    let mut value = valid();
    value["extensions"] = json!([{
        "schema":"urn:example:rfc8785", "schema_version":1,
        "payload":{"z":1e-7,"a":0.000001,"text":"<>&\u{2028}\u{2029}\\\""}
    }]);
    let canonical = String::from_utf8(read(value).unwrap().canonical_json().unwrap()).unwrap();
    assert!(canonical.contains(r#""a":0.000001,"text":"<>&  \\\"","z":1e-7"#));
}

#[test]
fn depth_limits_count_containers_not_scalar_leaves_and_empty_extensions_are_noncanonical() {
    let mut exact = valid();
    let mut nested = json!("leaf");
    for _ in 0..31 {
        nested = json!({"nested": nested});
    }
    exact["artifact"] = nested;
    assert!(matches!(
        read(exact),
        Err(ContactFragmentError::InvalidField {
            field: "artifact",
            ..
        })
    ));

    let mut over = valid();
    let mut nested = json!("leaf");
    for _ in 0..32 {
        nested = json!({"nested": nested});
    }
    over["artifact"] = nested;
    assert!(matches!(
        read(over),
        Err(ContactFragmentError::InvalidJson { .. })
    ));

    let mut empty = valid();
    empty["extensions"] = json!([]);
    assert!(matches!(
        read(empty),
        Err(ContactFragmentError::InvalidField {
            field: "extensions",
            ..
        })
    ));
}

#[test]
fn reader_byte_bounds_are_utf8_even_where_schema_counts_code_points() {
    let mut value = valid();
    value["events"] = json!([{
        "event_id": "\u{10000}".repeat(CONTACT_FRAGMENT_V1_MAX_IDENTIFIER_BYTES),
        "role": "body", "phase": "marker", "time": 0.0
    }]);
    assert!(
        read(value).is_err(),
        "255 Unicode code points exceed 255 UTF-8 bytes"
    );
}

#[test]
fn confidence_boundaries_and_jcs_number_key_order_are_pinned() {
    let mut value = valid();
    value["events"] = json!([
        {"event_id":"zero", "role":"body", "phase":"marker", "time":0.0, "confidence":0.0},
        {"event_id":"one", "role":"body", "phase":"marker", "time":1.0, "confidence":1.0}
    ]);
    value["extensions"] = json!([{
        "schema":"urn:example:jcs", "schema_version":1,
        "payload":{"\u{e000}":1, "\u{10000}":2}
    }]);
    let fragment = read(value).unwrap();
    let canonical = String::from_utf8(fragment.canonical_json().unwrap()).unwrap();
    assert!(
        canonical.contains("\"duration_s\":1,"),
        "JCS serializes integral binary64 without .0"
    );
    assert!(
        canonical.find("\u{10000}").unwrap() < canonical.find("\u{e000}").unwrap(),
        "JCS object keys use unsigned UTF-16 ordering"
    );
    assert!(ContactFragmentV1::read_json(br#"{"schema":"urn:animsmith:schema:contact-fragment:1","schema_version":1,"producer":{"tool":"a","version":"b"},"artifact":{"sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","bytes":1},"dependency_closure_identity":{"sha256":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","bytes":1},"clip":{"scope":"document","clip_name":"x"},"duration_s":1e999,"events":[]}"#).is_err());
}

#[test]
fn canonicalizes_mixed_event_order_with_utf16_ties() {
    let mut first = valid();
    first["events"] = json!([
        {"event_id":"z", "role":"right_foot", "phase":"marker", "time":0.25},
        {"event_id":"window", "role":"left_foot", "phase":"begin", "window":{"start":0.25,"end":0.25}},
        {"event_id":"a", "role":"body", "phase":"end", "time":0.25},
        {"event_id":"b", "role":"body", "phase":"begin", "time":0.25}
    ]);
    let mut second = first.clone();
    second["events"].as_array_mut().unwrap().reverse();
    let first = read(first).unwrap();
    let second = read(second).unwrap();
    assert_eq!(
        first.canonical_json().unwrap(),
        second.canonical_json().unwrap()
    );
    assert_eq!(
        first
            .events()
            .iter()
            .map(|event| event.event_id())
            .collect::<Vec<_>>(),
        vec!["b", "a", "z", "window"]
    );
    assert_eq!(
        first.canonical_identity().unwrap(),
        second.canonical_identity().unwrap()
    );
}

#[test]
fn canonical_fragment_round_trip_preserves_the_validated_value() {
    let mut value = valid();
    value["duration_s"] = 1.25.into();
    value["events"] = json!([{
        "event_id":"left", "role":"left_foot", "phase":"marker", "time":0.25,
        "confidence":0.75
    }]);
    value["extensions"] = json!([{
        "schema":"urn:example:fraction", "schema_version":1,
        "payload":{"fraction":1.25,"flags":[true, false]}
    }]);
    let fragment = read(value).unwrap();
    let canonical = fragment.canonical_json().unwrap();
    let decoded = ContactFragmentV1::read_json(&canonical).unwrap();
    assert_eq!(decoded, fragment);
    assert_eq!(decoded.canonical_json().unwrap(), canonical);
}

#[test]
fn extension_payload_is_stored_in_its_canonical_json_shape() {
    let extension = ContactExtensionV1::new(
        "urn:example:canonical-payload",
        1,
        json!({"negative_zero": -0.0, "one": 1.0}),
    )
    .unwrap();
    assert_eq!(extension.payload(), &json!({"negative_zero": 0, "one": 1}));

    let mut value = valid();
    value["extensions"] = json!([{
        "schema":"urn:example:canonical-payload", "schema_version":1,
        "payload":{"negative_zero": -0.0, "one": 1.0}
    }]);
    let fragment = read(value).unwrap();
    let canonical = fragment.canonical_json().unwrap();
    assert_eq!(ContactFragmentV1::read_json(&canonical).unwrap(), fragment);
}

#[test]
fn canonical_event_ties_use_unsigned_utf16_not_unicode_scalar_order() {
    let mut value = valid();
    value["events"] = json!([
        {"event_id":"\u{e000}", "role":"body", "phase":"marker", "time":0.5},
        {"event_id":"\u{10000}", "role":"body", "phase":"marker", "time":0.5}
    ]);
    let fragment = read(value).unwrap();
    assert_eq!(
        fragment
            .events()
            .iter()
            .map(|event| event.event_id())
            .collect::<Vec<_>>(),
        vec!["\u{10000}", "\u{e000}"]
    );
}

#[test]
fn signed_zero_has_one_canonical_event_order_and_identity() {
    let mut negative = valid();
    negative["events"] = json!([
        {"event_id":"b", "role":"body", "phase":"marker", "time":-0.0},
        {"event_id":"a", "role":"body", "phase":"marker", "time":0.0}
    ]);
    let mut positive = negative.clone();
    positive["events"][0]["time"] = 0.0.into();
    let negative = read(negative).unwrap();
    let positive = read(positive).unwrap();
    assert_eq!(
        negative.canonical_json().unwrap(),
        positive.canonical_json().unwrap()
    );
    assert_eq!(
        negative
            .events()
            .iter()
            .map(|event| event.event_id())
            .collect::<Vec<_>>(),
        vec!["a", "b"]
    );
}

#[test]
fn strict_reader_rejects_duplicate_unknown_or_malformed_fields() {
    let duplicate = br#"{
      "schema":"urn:animsmith:schema:contact-fragment:1","schema":"urn:animsmith:schema:contact-fragment:1","schema_version":1,
      "producer":{"tool":"animsmith","version":"0.6.0"},"artifact":{"sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","bytes":1},
      "dependency_closure_identity":{"sha256":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","bytes":1},
      "clip":{"scope":"document","clip_name":"walk"},"duration_s":1,"events":[]
    }"#;
    assert!(matches!(
        ContactFragmentV1::read_json(duplicate),
        Err(ContactFragmentError::InvalidJson { .. })
    ));

    let mut unknown = valid();
    unknown["surprise"] = true.into();
    assert!(
        read(unknown).is_err(),
        "unknown root field must fail before retention"
    );

    let mut malformed = valid();
    malformed["events"] = json!([{"event_id":"x", "role":"left_foot", "phase":"marker", "time":0.0, "window":{"start":0.0,"end":0.0}}]);
    assert!(matches!(
        read(malformed),
        Err(ContactFragmentError::InvalidField { field: "event", .. })
    ));

    let mut missing_events = valid();
    missing_events.as_object_mut().unwrap().remove("events");
    assert!(matches!(
        read(missing_events),
        Err(ContactFragmentError::InvalidField {
            field: "fragment",
            ..
        })
    ));
}

#[test]
fn rejects_nonfinite_range_and_depth_violations() {
    let mut invalid_time = valid();
    invalid_time["events"] =
        json!([{"event_id":"x", "role":"left_foot", "phase":"marker", "time":1.1}]);
    assert!(matches!(
        read(invalid_time),
        Err(ContactFragmentError::InvalidField {
            field: "event.time",
            ..
        })
    ));

    let mut depth = valid();
    let mut nested = json!({});
    for _ in 0..33 {
        nested = json!({"nested": nested});
    }
    depth["extensions"] =
        json!([{"schema":"urn:example:depth", "schema_version":1, "payload":nested}]);
    assert!(
        read(depth).is_err(),
        "nested payload beyond the frozen depth must fail"
    );
}
