use jsonschema::Validator;
use serde_json::{Value, json};
use std::path::Path;

fn basis_reference_validator() -> Option<Validator> {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output_path = workspace.join("docs/schemas/output-v15.schema.json");
    let measurements_path = workspace.join("docs/schemas/measurements-v16.schema.json");
    if !output_path.is_file() || !measurements_path.is_file() {
        // Published crates intentionally exclude repository-level schemas.
        return None;
    }
    let output = std::fs::read_to_string(output_path).expect("reads output-v15 schema");
    let measurements =
        std::fs::read_to_string(measurements_path).expect("reads measurements-v16 schema");
    let published: Value = serde_json::from_str(&output).expect("output-v15 schema is valid JSON");
    let schema = json!({
        "$schema": published["$schema"].clone(),
        "$ref": "#/$defs/basis_reference_v4",
        "$defs": published["$defs"].clone(),
    });
    let measurements: Value =
        serde_json::from_str(&measurements).expect("measurements-v16 schema is valid JSON");
    let registry = jsonschema::Registry::new()
        .add("urn:animsmith:schema:measurements:16", measurements)
        .expect("measurements-v16 schema identity is valid")
        .prepare()
        .expect("measurements-v16 schema registry prepares");
    Some(
        jsonschema::options()
            .with_registry(&registry)
            .build(&schema)
            .expect("V4 basis-reference schema compiles"),
    )
}

#[test]
fn v4_schema_accepts_canonical_nested_v2_and_raw_inventory_references() {
    let Some(validator) = basis_reference_validator() else {
        return;
    };
    for reference in [
        json!({
            "contract": "v2",
            "reference": {
                "contract": "v1",
                "reference": {
                    "kind": "profile_fact",
                    "fact_id": "resulting_transform_scale"
                }
            }
        }),
        json!({
            "contract": "v2",
            "reference": {
                "contract": "v1",
                "reference": {
                    "kind": "resolved_setting",
                    "location": { "scope": "document" },
                    "setting_id": "rotate_scene_entity"
                }
            }
        }),
        json!({
            "contract": "raw_scene_attachment",
            "reference": { "field": "scene_row", "source_scene_index": 0 }
        }),
    ] {
        assert!(
            validator.is_valid(&reference),
            "canonical V4 basis reference rejected: {reference:#}"
        );
    }
}

#[test]
fn v4_schema_rejects_flattened_wrong_revision_and_unknown_basis_references() {
    let Some(validator) = basis_reference_validator() else {
        return;
    };
    for mutation in [
        json!({
            "contract": "v2",
            "reference": {
                "kind": "profile_fact",
                "fact_id": "resulting_transform_scale"
            }
        }),
        json!({
            "contract": "v2",
            "reference": {
                "contract": "v1",
                "reference": {
                    "kind": "profile_fact",
                    "fact_id": "resulting_hierarchy_scale"
                }
            }
        }),
        json!({
            "contract": "v2",
            "reference": {
                "contract": "v2",
                "reference": {
                    "kind": "profile_fact",
                    "fact_id": "resulting_transform_scale"
                }
            }
        }),
        json!({
            "contract": "raw_scene_attachment",
            "reference": {
                "field": "scene_row",
                "source_scene_index": 0,
                "forged": true
            }
        }),
    ] {
        assert!(
            !validator.is_valid(&mutation),
            "mutated V4 basis reference was accepted: {mutation:#}"
        );
    }
}
