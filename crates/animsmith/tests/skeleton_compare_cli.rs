//! Black-box coverage for the identity-pinned structural skeleton comparison.

use animsmith_core::InputIdentity;
use animsmith_core::glam::{Mat4, Quat, Vec3};
use animsmith_core::model::{Bone, MeshAsset, MeshInstance, Transform};
use animsmith_testkit::{quats_from_angles, two_bone_rotation_doc};
use serde_json::{Value, json};
use std::path::Path;
use std::process::{Command, Output};

const SCHEMA: &str = include_str!("../../../docs/schemas/skeleton-compatibility-v1.schema.json");

fn write_doc(path: &Path, root: &str, child: &str, child_height: f32) {
    let mut document = two_bone_rotation_doc("walk", quats_from_angles(&[0.0; 5]), false);
    document.skeleton.bones[0].name = root.into();
    document.skeleton.bones[1].name = child.into();
    document.skeleton.bones[1].rest.translation = Vec3::new(0.0, child_height, 0.0);
    animsmith_gltf::write::write(&document, path).expect("writes self-authored skeleton fixture");
}

fn write_doc_with_skin(path: &Path, selected_skin: bool) {
    let mut document = two_bone_rotation_doc("walk", quats_from_angles(&[0.0; 5]), false);
    let joint = if selected_skin {
        1
    } else {
        document.skeleton.bones.push(Bone {
            name: "unrelated".into(),
            parent: None,
            rest: Transform::IDENTITY,
            inverse_bind: None,
        });
        2
    };
    document.assets.meshes.push(MeshAsset::default());
    document.assets.instances.push(MeshInstance {
        node: joint,
        mesh: 0,
        skin_joints: vec![joint],
        skin_ibms: vec![Mat4::IDENTITY],
        ..MeshInstance::default()
    });
    animsmith_gltf::write::write(&document, path).expect("writes skinned skeleton fixture");
}

fn write_duplicate_name_doc(path: &Path) {
    let mut document = two_bone_rotation_doc("walk", quats_from_angles(&[0.0; 5]), false);
    document.skeleton.bones[0].name = "root".into();
    document.skeleton.bones[1].name = "root".into();
    document.skeleton.bones.push(Bone {
        name: "spine".into(),
        parent: Some(0),
        rest: Transform::IDENTITY,
        inverse_bind: None,
    });
    animsmith_gltf::write::write(&document, path).expect("writes duplicate-name fixture");
}

fn write_three_bone_doc(path: &Path, head_parent: usize, spine_rotation: f32, spine_scale: f32) {
    let mut document = two_bone_rotation_doc("walk", quats_from_angles(&[0.0; 5]), false);
    document.skeleton.bones[1].rest.rotation = Quat::from_rotation_y(spine_rotation);
    document.skeleton.bones[1].rest.scale = Vec3::splat(spine_scale);
    document.skeleton.bones.push(Bone {
        name: "head".into(),
        parent: Some(head_parent),
        rest: Transform {
            translation: Vec3::Y,
            ..Transform::IDENTITY
        },
        inverse_bind: None,
    });
    animsmith_gltf::write::write(&document, path).expect("writes self-authored three-bone fixture");
}

fn correspondence(source: &Path, target: &Path, explicit: bool, length_tolerance: f64) -> String {
    correspondence_with_selectors(
        source,
        target,
        explicit,
        length_tolerance,
        &["root", "spine"],
        if explicit {
            &["target_root", "target_spine"]
        } else {
            &["root", "spine"]
        },
    )
}

fn correspondence_with_selectors(
    source: &Path,
    target: &Path,
    explicit: bool,
    length_tolerance: f64,
    source_nodes: &[&str],
    target_nodes: &[&str],
) -> String {
    let source = InputIdentity::from_bytes(&std::fs::read(source).unwrap());
    let target = InputIdentity::from_bytes(&std::fs::read(target).unwrap());
    let toml_names = |names: &[&str]| {
        names
            .iter()
            .map(|name| format!("\"{name}\""))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let matching = if explicit {
        "[correspondence]\nmode = \"explicit\"\nmap = { root = \"target_root\", spine = \"target_spine\" }"
    } else {
        "[correspondence]\nmode = \"exact_name\""
    };
    format!(
        r#"schema = "urn:animsmith:skeleton-correspondence:1"
schema_version = 1

[source]
input = {{ sha256 = "{}", bytes = {} }}
selector = {{ root_name = "root", node_names = [{}] }}

[target]
input = {{ sha256 = "{}", bytes = {} }}
selector = {{ root_name = "{}", node_names = [{}] }}

{}

[tolerances]
translation_m = 0.0
rotation_deg = 0.0
scale_delta = 0.0
normalized_bone_length_ratio_delta = {}
"#,
        source.sha256(),
        source.bytes(),
        toml_names(source_nodes),
        target.sha256(),
        target.bytes(),
        target_nodes[0],
        toml_names(target_nodes),
        matching,
        length_tolerance,
    )
}

fn run(source: &Path, target: &Path, control: &Path) -> Output {
    run_format(source, target, control, "json")
}

fn run_format(source: &Path, target: &Path, control: &Path, format: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .args(["skeleton", "compare"])
        .arg(source)
        .arg(target)
        .args(["--correspondence"])
        .arg(control)
        .args(["--format", format])
        .output()
        .expect("runs skeleton compare")
}

fn json(output: &Output) -> Value {
    assert!(
        output.stderr.is_empty(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("result JSON")
}

fn validate(value: &Value) {
    let schema: Value = serde_json::from_str(SCHEMA).expect("schema parses");
    let validator = jsonschema::validator_for(&schema).expect("schema compiles");
    assert!(
        validator.is_valid(value),
        "schema errors: {:?}",
        validator.iter_errors(value).collect::<Vec<_>>()
    );
}

#[test]
fn compares_exact_and_explicit_correspondence_with_provenance_and_stable_outcomes() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.glb");
    let target = temp.path().join("target.glb");
    let renamed = temp.path().join("renamed.glb");
    let control = temp.path().join("correspondence.toml");
    write_doc(&source, "root", "spine", 0.5);
    write_doc(&target, "root", "spine", 0.5);
    std::fs::write(&control, correspondence(&source, &target, false, 0.0)).unwrap();

    let output = run(&source, &target, &control);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value = json(&output);
    validate(&value);
    assert_eq!(
        value["schema"],
        "urn:animsmith:schema:skeleton-compatibility:1"
    );
    assert_eq!(value["outcome"], "compatible");
    assert_eq!(value["correspondence"]["matching_mode"], "exact_name");
    assert_eq!(value["correspondence"]["mapping"], json!({}));
    let mut invalid_exact_mapping = value.clone();
    invalid_exact_mapping["correspondence"]["mapping"] = json!({"root":"root"});
    let schema: Value = serde_json::from_str(SCHEMA).unwrap();
    assert!(
        !jsonschema::validator_for(&schema)
            .unwrap()
            .is_valid(&invalid_exact_mapping)
    );
    assert_eq!(
        value["source"]["selected_skeleton_identity"],
        value["target"]["selected_skeleton_identity"]
    );
    assert!(
        value["source"]["dependency_closure_complete"]
            .as_bool()
            .unwrap()
    );
    assert!(
        value["rows"]
            .as_array()
            .unwrap()
            .iter()
            .all(|row| row["kind"] == "matched")
    );
    assert_eq!(value["facets"]["deformation_model"]["state"], "unavailable");
    assert_eq!(value["facets"]["skin_membership"]["state"], "unavailable");
    assert_eq!(
        value["facets"]["skin_membership"]["source"]["detail"],
        "no source skin declarations include selected skeleton joints"
    );
    assert_eq!(
        value["facets"]["skin_membership"]["target"]["detail"],
        "no target skin declarations include selected skeleton joints"
    );
    assert_eq!(
        value["facets"]["skin_membership"]["source"]["owner_surface"],
        "selected_skin_bind_evidence"
    );
    assert_eq!(
        value["facets"]["skin_membership"]["source"]["remedy_class"],
        "supply_skin_bind_evidence"
    );
    assert_eq!(value["facets"]["inverse_bind"]["state"], "unavailable");
    let repeated = run(&source, &target, &control);
    assert_eq!(output.stdout, repeated.stdout, "serialization is stable");

    write_doc(&renamed, "target_root", "target_spine", 1.0);
    std::fs::write(&control, correspondence(&source, &renamed, true, 0.1)).unwrap();
    let output = run(&source, &renamed, &control);
    assert_eq!(output.status.code(), Some(1));
    let value = json(&output);
    validate(&value);
    assert_eq!(value["outcome"], "incompatible");
    assert_eq!(value["correspondence"]["matching_mode"], "explicit");
    assert_eq!(
        value["correspondence"]["mapping"],
        json!({"root":"target_root", "spine":"target_spine"})
    );
    let mut invalid_explicit_mapping = value.clone();
    invalid_explicit_mapping["correspondence"]["mapping"] = json!({});
    assert!(
        !jsonschema::validator_for(&schema)
            .unwrap()
            .is_valid(&invalid_explicit_mapping)
    );
    let spine = value["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["source_name"] == "spine")
        .unwrap();
    assert_eq!(
        spine["normalized_child_bone_length_ratio"]["state"],
        "mismatch"
    );
    assert_eq!(spine["owner_surface"], "selected_skeleton_authority");
    assert_eq!(spine["remedy_class"], "align_rest_pose");
}

#[test]
fn skin_and_bind_facets_only_use_skins_that_include_the_selected_skeleton() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.glb");
    let target = temp.path().join("target.glb");
    let control = temp.path().join("correspondence.toml");

    write_doc_with_skin(&source, true);
    write_doc_with_skin(&target, true);
    std::fs::write(&control, correspondence(&source, &target, false, 0.0)).unwrap();
    let selected = json(&run(&source, &target, &control));
    assert_eq!(selected["facets"]["skin_membership"]["state"], "pass");
    assert_eq!(selected["facets"]["inverse_bind"]["state"], "pass");
    assert_eq!(
        selected["facets"]["skin_membership"]["target"]["detail"],
        "1 target skin declarations include selected skeleton joints"
    );

    write_doc_with_skin(&source, false);
    write_doc_with_skin(&target, false);
    std::fs::write(&control, correspondence(&source, &target, false, 0.0)).unwrap();
    let unrelated = json(&run(&source, &target, &control));
    assert_eq!(
        unrelated["facets"]["skin_membership"]["state"],
        "unavailable"
    );
    assert_eq!(unrelated["facets"]["inverse_bind"]["state"], "unavailable");
}

#[test]
fn text_report_preserves_authority_deltas_remediation_and_all_outcome_names() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.glb");
    let target = temp.path().join("target.glb");
    let control = temp.path().join("correspondence.toml");

    write_doc(&source, "root", "spine", 0.5);
    write_doc(&target, "root", "spine", 0.5);
    std::fs::write(&control, correspondence(&source, &target, false, 0.0)).unwrap();
    let machine = json(&run(&source, &target, &control));
    let output = run_format(&source, &target, &control, "text");
    assert!(output.status.success());
    let report = String::from_utf8(output.stdout).unwrap();
    let source_identity = InputIdentity::from_bytes(&std::fs::read(&source).unwrap());
    let target_identity = InputIdentity::from_bytes(&std::fs::read(&target).unwrap());
    let control_identity = InputIdentity::from_bytes(&std::fs::read(&control).unwrap());
    assert!(report.starts_with("skeleton compatibility: compatible\n"));
    assert!(report.contains(&format!(
        "source input: sha256={} bytes={}",
        source_identity.sha256(),
        source_identity.bytes()
    )));
    assert!(report.contains(&format!(
        "target input: sha256={} bytes={}",
        target_identity.sha256(),
        target_identity.bytes()
    )));
    assert!(report.contains(&format!(
        "correspondence input: sha256={} bytes={}",
        control_identity.sha256(),
        control_identity.bytes()
    )));
    assert!(report.contains(&format!(
        "source selected skeleton: sha256={} bytes={}",
        machine["source"]["selected_skeleton_identity"]["sha256"]
            .as_str()
            .unwrap(),
        machine["source"]["selected_skeleton_identity"]["bytes"]
            .as_u64()
            .unwrap()
    )));
    assert!(report.contains(&format!(
        "target selected skeleton: sha256={} bytes={}",
        machine["target"]["selected_skeleton_identity"]["sha256"]
            .as_str()
            .unwrap(),
        machine["target"]["selected_skeleton_identity"]["bytes"]
            .as_u64()
            .unwrap()
    )));
    assert!(report.contains("source selector node: root"));
    assert!(report.contains("target selector node: spine"));
    assert!(report.contains("matching mode: exact_name"));
    assert!(report.contains("tolerances: translation_m=0 rotation_deg=0 scale_delta=0 normalized_bone_length_ratio_delta=0"));
    assert!(report.contains("local_rest.translation_m: state=pass"));
    assert!(report.contains("facet skin_membership: unavailable required=false"));
    assert!(report.contains("remedy_class=supply_skin_bind_evidence"));

    write_doc(&target, "target_root", "target_spine", 1.0);
    std::fs::write(&control, correspondence(&source, &target, true, 0.0)).unwrap();
    let output = run_format(&source, &target, &control, "text");
    assert_eq!(output.status.code(), Some(1));
    let report = String::from_utf8(output.stdout).unwrap();
    assert!(report.starts_with("skeleton compatibility: incompatible\n"));
    assert!(report.contains("matching mode: explicit"));
    assert!(report.contains("mapping: root -> target_root"));
    assert!(report.contains("mapping: spine -> target_spine"));
    assert!(report.contains("remedy_class=align_rest_pose"));

    write_three_bone_doc(&source, 1, 0.0, 1.0);
    write_three_bone_doc(&target, 1, 0.0, 2.0);
    std::fs::write(
        &control,
        correspondence_with_selectors(
            &source,
            &target,
            false,
            0.0,
            &["root", "spine", "head"],
            &["root", "spine", "head"],
        ),
    )
    .unwrap();
    let output = run_format(&source, &target, &control, "text");
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .starts_with("skeleton compatibility: partial\n")
    );

    write_doc(&source, "root", "spine", 0.5);
    write_doc(&target, "target_root", "target_spine", 0.5);
    std::fs::write(
        &control,
        correspondence_with_selectors(
            &source,
            &target,
            false,
            0.0,
            &["root", "spine"],
            &["target_root", "target_spine"],
        ),
    )
    .unwrap();
    let machine = run(&source, &target, &control);
    assert_eq!(machine.status.code(), Some(1));
    let machine = json(&machine);
    validate(&machine);
    assert_eq!(machine["outcome"], "not_evaluated");
    let output = run_format(&source, &target, &control, "text");
    assert_eq!(output.status.code(), Some(1));
    let report = String::from_utf8(output.stdout).unwrap();
    assert!(report.starts_with("skeleton compatibility: not_evaluated\n"));
    assert!(report.contains("row missing_target: root -> -"));
    assert!(report.contains("remedy_class=rename_or_remap"));
}

#[test]
fn uses_only_the_explicitly_declared_node_set() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.glb");
    let target = temp.path().join("target-with-attachment.glb");
    let control = temp.path().join("correspondence.toml");
    write_doc(&source, "root", "spine", 0.5);
    write_three_bone_doc(&target, 1, 0.0, 1.0);
    std::fs::write(&control, correspondence(&source, &target, false, 0.0)).unwrap();

    let output = run(&source, &target, &control);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value = json(&output);
    validate(&value);
    assert_eq!(value["outcome"], "compatible");
    assert_eq!(value["rows"].as_array().unwrap().len(), 2);
    assert!(
        value["rows"]
            .as_array()
            .unwrap()
            .iter()
            .all(|row| row["source_name"] != "head" && row["target_name"] != "head")
    );
}

#[test]
fn selected_skeleton_identity_is_invariant_to_selector_name_order() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.glb");
    let target = temp.path().join("target.glb");
    let control = temp.path().join("correspondence.toml");
    write_doc(&source, "root", "spine", 0.5);
    write_doc(&target, "root", "spine", 0.5);
    std::fs::write(&control, correspondence(&source, &target, false, 0.0)).unwrap();
    let canonical = json(&run(&source, &target, &control));
    let reordered = correspondence(&source, &target, false, 0.0).replace(
        "node_names = [\"root\", \"spine\"]",
        "node_names = [\"spine\", \"root\"]",
    );
    std::fs::write(&control, reordered).unwrap();
    let reordered = json(&run(&source, &target, &control));
    assert_eq!(canonical["outcome"], reordered["outcome"]);
    assert_eq!(canonical["rows"], reordered["rows"]);
    assert_eq!(
        canonical["source"]["selected_skeleton_identity"],
        reordered["source"]["selected_skeleton_identity"]
    );
    assert_eq!(
        canonical["target"]["selected_skeleton_identity"],
        reordered["target"]["selected_skeleton_identity"]
    );
}

#[test]
fn schema_rejects_omitted_and_forbidden_fields_for_every_row_kind() {
    let schema: Value = serde_json::from_str(SCHEMA).unwrap();
    let row_schema = json!({"$defs": schema["$defs"], "$ref": "#/$defs/row"});
    let validator = jsonschema::validator_for(&row_schema).unwrap();
    let rest = json!({
        "translation_m": {"tolerance": 0.0, "state": "pass"},
        "rotation_deg": {"tolerance": 0.0, "state": "pass"},
        "scale_delta": {"tolerance": 0.0, "state": "pass"}
    });
    let delta = json!({"tolerance": 0.0, "state": "pass"});
    let rows = [
        json!({"kind":"matched", "source_name":"a", "target_name":"b", "parent_correspondence":"pass", "local_rest":rest, "rest_world":rest, "normalized_child_bone_length_ratio":delta}),
        json!({"kind":"parent_mismatch", "source_name":"a", "target_name":"b", "parent_correspondence":"mismatch", "local_rest":rest, "rest_world":rest, "normalized_child_bone_length_ratio":delta, "owner_surface":"selected_skeleton_authority", "remedy_class":"align_hierarchy"}),
        json!({"kind":"missing_target", "source_name":"a", "owner_surface":"correspondence", "remedy_class":"rename_or_remap"}),
        json!({"kind":"missing_source", "target_name":"b", "owner_surface":"correspondence", "remedy_class":"rename_or_remap"}),
    ];
    for (row, required_field) in
        rows.into_iter()
            .zip(["local_rest", "rest_world", "source_name", "target_name"])
    {
        assert!(
            validator.is_valid(&row),
            "valid row: {row}; errors: {:?}",
            validator.iter_errors(&row).collect::<Vec<_>>()
        );
        let mut omitted = row.clone();
        omitted.as_object_mut().unwrap().remove(required_field);
        assert!(!validator.is_valid(&omitted), "omitted field: {omitted}");
        let mut forbidden = row.clone();
        forbidden["forbidden"] = json!(true);
        assert!(
            !validator.is_valid(&forbidden),
            "forbidden field: {forbidden}"
        );
    }
    let matched_with_half_remediation = json!({"kind":"matched", "source_name":"a", "target_name":"b", "parent_correspondence":"pass", "local_rest":rest, "rest_world":rest, "owner_surface":"measurement_boundary"});
    assert!(!validator.is_valid(&matched_with_half_remediation));
    assert!(
        !validator.is_valid(&json!({"kind":"unavailable", "source_name":"a", "target_name":"b"}))
    );
}

#[test]
fn refuses_missing_or_ambiguous_declared_selectors_without_a_result() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.glb");
    let target = temp.path().join("target.glb");
    let control = temp.path().join("correspondence.toml");
    write_doc(&source, "root", "spine", 0.5);
    write_doc(&target, "root", "spine", 0.5);
    let missing = correspondence(&source, &target, false, 0.0).replace(
        "node_names = [\"root\", \"spine\"]",
        "node_names = [\"root\", \"missing\"]",
    );
    std::fs::write(&control, missing).unwrap();
    let output = run(&source, &target, &control);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("exactly one declared node"));

    write_duplicate_name_doc(&source);
    std::fs::write(&control, correspondence(&source, &target, false, 0.0)).unwrap();
    let output = run(&source, &target, &control);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("exactly one declared node"));
}

#[test]
fn refuses_explicit_mapping_entries_outside_the_declared_selectors() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.glb");
    let target = temp.path().join("target.glb");
    let control = temp.path().join("correspondence.toml");
    write_doc(&source, "root", "spine", 0.5);
    write_doc(&target, "target_root", "target_spine", 0.5);
    let source_injection = correspondence(&source, &target, true, 0.0).replace(
        "map = { root = \"target_root\", spine = \"target_spine\" }",
        "map = { head = \"target_root\", spine = \"target_spine\" }",
    );
    std::fs::write(&control, source_injection).unwrap();
    let output = run(&source, &target, &control);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("source-mapping-outside-selector"));

    let target_injection = correspondence(&source, &target, true, 0.0).replace(
        "map = { root = \"target_root\", spine = \"target_spine\" }",
        "map = { root = \"head\", spine = \"target_spine\" }",
    );
    std::fs::write(&control, target_injection).unwrap();
    let output = run(&source, &target, &control);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("target-mapping-outside-selector"));
}

#[test]
fn refuses_oversized_control_before_input_loading_and_oversized_primary_before_parsing() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.glb");
    let target = temp.path().join("target.glb");
    let control = temp.path().join("correspondence.toml");
    write_doc(&source, "root", "spine", 0.5);
    write_doc(&target, "root", "spine", 0.5);
    std::fs::write(&control, vec![b'x'; 64 * 1024 + 1]).unwrap();
    let output = run(&source, &target, &control);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "animsmith: skeleton correspondence exceeds its bounded reader limit\n"
    );

    let oversized = temp.path().join("oversized.glb");
    std::fs::write(&oversized, vec![0; 64 * 1024 * 1024 + 1]).unwrap();
    std::fs::write(&control, correspondence(&oversized, &target, false, 0.0)).unwrap();
    let output = run(&oversized, &target, &control);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("skeleton comparison input exceeds its 67108864 byte limit")
    );
}

#[test]
fn reports_an_isolated_translation_delta_with_its_numeric_value() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.glb");
    let target = temp.path().join("target.glb");
    let control = temp.path().join("correspondence.toml");
    write_doc(&source, "root", "spine", 0.5);
    write_doc(&target, "root", "spine", 0.75);
    std::fs::write(&control, correspondence(&source, &target, false, 1.0)).unwrap();

    let output = run(&source, &target, &control);
    assert_eq!(output.status.code(), Some(1));
    let value = json(&output);
    let spine = value["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["source_name"] == "spine")
        .unwrap();
    for rest in ["local_rest", "rest_world"] {
        assert_eq!(spine[rest]["translation_m"]["state"], "mismatch");
        assert!((spine[rest]["translation_m"]["value"].as_f64().unwrap() - 0.25).abs() < 1e-6);
        assert_eq!(spine[rest]["rotation_deg"]["state"], "pass");
        assert_eq!(spine[rest]["scale_delta"]["state"], "pass");
    }
    assert_eq!(spine["normalized_child_bone_length_ratio"]["state"], "pass");
}

#[test]
fn reports_missing_parent_rotation_scale_and_unavailable_rotation_as_distinct_evidence() {
    let temp = tempfile::tempdir().unwrap();
    let two = temp.path().join("two.glb");
    let source = temp.path().join("source.glb");
    let target = temp.path().join("target.glb");
    let control = temp.path().join("correspondence.toml");
    write_doc(&two, "root", "spine", 0.5);
    write_three_bone_doc(&target, 1, 0.0, 1.0);
    std::fs::write(
        &control,
        correspondence_with_selectors(
            &two,
            &target,
            false,
            0.0,
            &["root", "spine"],
            &["root", "spine", "head"],
        ),
    )
    .unwrap();
    let missing = run(&two, &target, &control);
    assert_eq!(missing.status.code(), Some(1));
    let missing_json = json(&missing);
    validate(&missing_json);
    assert!(
        missing_json["rows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["kind"] == "missing_source" && row["target_name"] == "head")
    );
    let missing = missing_json["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["kind"] == "missing_source")
        .unwrap();
    assert_eq!(missing["owner_surface"], "correspondence");
    assert_eq!(missing["remedy_class"], "rename_or_remap");

    write_three_bone_doc(&source, 1, 0.0, 1.0);
    write_three_bone_doc(&target, 0, 0.0, 1.0);
    std::fs::write(
        &control,
        correspondence_with_selectors(
            &source,
            &target,
            false,
            0.0,
            &["root", "spine", "head"],
            &["root", "spine", "head"],
        ),
    )
    .unwrap();
    let parent = json(&run(&source, &target, &control));
    let head = parent["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["source_name"] == "head")
        .unwrap();
    assert_eq!(head["kind"], "parent_mismatch");
    assert_eq!(head["parent_correspondence"], "mismatch");
    assert_eq!(head["owner_surface"], "selected_skeleton_authority");
    assert_eq!(head["remedy_class"], "align_hierarchy");

    write_three_bone_doc(&target, 1, 0.5, 1.0);
    std::fs::write(
        &control,
        correspondence_with_selectors(
            &source,
            &target,
            false,
            0.0,
            &["root", "spine", "head"],
            &["root", "spine", "head"],
        ),
    )
    .unwrap();
    let rotation = json(&run(&source, &target, &control));
    let spine = rotation["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["source_name"] == "spine")
        .unwrap();
    assert_eq!(spine["local_rest"]["rotation_deg"]["state"], "mismatch");
    assert_eq!(spine["rest_world"]["rotation_deg"]["state"], "mismatch");

    write_three_bone_doc(&target, 1, 0.0, 2.0);
    std::fs::write(
        &control,
        correspondence_with_selectors(
            &source,
            &target,
            false,
            0.0,
            &["root", "spine", "head"],
            &["root", "spine", "head"],
        ),
    )
    .unwrap();
    let scale = json(&run(&source, &target, &control));
    let spine = scale["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["source_name"] == "spine")
        .unwrap();
    assert_eq!(scale["outcome"], "partial");
    assert_eq!(spine["local_rest"]["scale_delta"]["state"], "mismatch");
    assert_eq!(spine["rest_world"]["rotation_deg"]["state"], "unavailable");
}

#[test]
fn stale_identity_and_unknown_control_field_are_operator_errors_without_result() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.glb");
    let target = temp.path().join("target.glb");
    let control = temp.path().join("correspondence.toml");
    write_doc(&source, "root", "spine", 0.5);
    write_doc(&target, "root", "spine", 0.5);
    let mut current = correspondence(&source, &target, false, 0.0);
    current.push_str("unknown = true\n");
    std::fs::write(&control, current).unwrap();
    let invalid = run(&source, &target, &control);
    assert_eq!(invalid.status.code(), Some(2));
    assert!(invalid.stdout.is_empty());

    std::fs::write(&control, correspondence(&source, &target, false, 0.0)).unwrap();
    write_doc(&target, "root", "spine", 0.6);
    let stale = run(&source, &target, &control);
    assert_eq!(stale.status.code(), Some(2));
    assert!(stale.stdout.is_empty());
    assert!(String::from_utf8_lossy(&stale.stderr).contains("stale-target-identity"));
}
