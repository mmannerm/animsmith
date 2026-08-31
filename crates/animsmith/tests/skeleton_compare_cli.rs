//! Black-box coverage for the identity-pinned structural skeleton comparison.

use animsmith_core::InputIdentity;
use animsmith_core::glam::{Quat, Vec3};
use animsmith_core::model::{Bone, Transform};
use animsmith_testkit::{quats_from_angles, two_bone_rotation_doc};
use serde_json::Value;
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

fn write_duplicate_name_doc(path: &Path) {
    let mut document = two_bone_rotation_doc("walk", quats_from_angles(&[0.0; 5]), false);
    document.skeleton.bones[0].name = "root".into();
    document.skeleton.bones[1].name = "root".into();
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
        if explicit { "target_root" } else { "root" },
        toml_names(target_nodes),
        matching,
        length_tolerance,
    )
}

fn run(source: &Path, target: &Path, control: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .args(["skeleton", "compare"])
        .arg(source)
        .arg(target)
        .args(["--correspondence"])
        .arg(control)
        .args(["--format", "json"])
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
    assert_eq!(value["facets"]["skin_membership"]["state"], "pass");
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
