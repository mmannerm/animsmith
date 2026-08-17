//! Public-boundary coverage for character-assembly recipe/evidence v4.

#![cfg(feature = "fbx")]

use animsmith_core::glam::Vec3;
use animsmith_core::model::{Interpolation, Property, TrackValues};
use animsmith_testkit::{rest_bind_scale_rig_glb, rest_bind_scale_rig_gltf};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::Path;
use std::process::{Command, Output};

const RECIPE_SCHEMA: &str =
    include_str!("../../../docs/schemas/character-assembly-recipe-v4.schema.json");
const EVIDENCE_SCHEMA: &str =
    include_str!("../../../docs/schemas/character-assembly-evidence-v4.schema.json");

fn recipe(clip: &str) -> String {
    format!(
        r#"schema_version = 4
schema = "urn:animsmith:schema:character-assembly-recipe:4"
input_root = "inputs"
base_input = "base.glb"
fps = 30.0

[rest_bind_scale]
source_skin_index = 0
source_root_node_index = 0
expected_factor = 0.01

[[clips]]
name = "walk"
input = "{clip}"
take = "clip"
"#
    )
}

fn write_cubic_asset(path: &Path) {
    let bytes = rest_bind_scale_rig_glb();
    let mut document = animsmith_gltf::load_bytes(Path::new("source.glb"), &bytes).unwrap();
    let track = document.clips[0]
        .tracks
        .iter_mut()
        .find(|track| track.property == Property::Translation)
        .expect("fixture translation track");
    track.interpolation = Interpolation::CubicSpline;
    track.times = vec![0.0, 1.0];
    track.values = TrackValues::Vec3s(vec![
        Vec3::new(2.0, 0.0, 0.0),
        Vec3::new(0.0, 100.0, 0.0),
        Vec3::new(3.0, 0.0, 0.0),
        Vec3::new(4.0, 0.0, 0.0),
        Vec3::new(0.0, 200.0, 0.0),
        Vec3::new(5.0, 0.0, 0.0),
    ]);
    animsmith_gltf::write::write(&document, path).expect("writes cubic fixture");
}

fn run(dir: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .current_dir(dir)
        .args([
            "assemble",
            "recipe.toml",
            "-o",
            "character.glb",
            "--evidence",
            "character.json",
            "--format",
            "json",
        ])
        .output()
        .expect("runs assemble")
}

fn assert_schema(instance: &Value, schema: &str) {
    let schema: Value = serde_json::from_str(schema).expect("schema JSON");
    let validator = jsonschema::validator_for(&schema).expect("schema compiles");
    let errors = validator
        .iter_errors(instance)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(errors.is_empty(), "schema errors: {errors:#?}");
}

#[test]
fn v4_rebases_before_remap_then_proves_and_publishes_the_exact_final_artifact() {
    let dir = tempfile::tempdir().expect("temporary directory");
    std::fs::create_dir(dir.path().join("inputs")).unwrap();
    write_cubic_asset(&dir.path().join("inputs/base.glb"));
    write_cubic_asset(&dir.path().join("inputs/clip.glb"));
    write_cubic_asset(&dir.path().join("inputs/clip-two.glb"));
    let recipe = format!(
        "{}\n[[clips]]\nname = \"run\"\ninput = \"clip-two.glb\"\ntake = \"clip\"\n",
        recipe("clip.glb")
    );
    std::fs::write(dir.path().join("recipe.toml"), &recipe).unwrap();
    let recipe_value: toml::Value = toml::from_str(&recipe).unwrap();
    assert_schema(&serde_json::to_value(recipe_value).unwrap(), RECIPE_SCHEMA);

    let first = run(dir.path());
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let artifact = std::fs::read(dir.path().join("character.glb")).unwrap();
    let evidence_bytes = std::fs::read(dir.path().join("character.json")).unwrap();
    assert_eq!(first.stdout, evidence_bytes);
    let evidence: Value = serde_json::from_slice(&evidence_bytes).unwrap();
    assert_schema(&evidence, EVIDENCE_SCHEMA);
    assert_eq!(evidence["schema_version"], 4);
    assert_eq!(
        evidence["rest_bind_scale"]["inputs"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert!(
        evidence["rest_bind_scale"]["inputs"]
            .as_array()
            .unwrap()
            .iter()
            .all(|input| input["compatible"] == true)
    );
    assert_eq!(
        evidence["artifact"]["sha256"],
        format!("{:x}", Sha256::digest(&artifact))
    );
    assert!(
        evidence["rest_bind_scale"]["proof"]["proof"]["residuals"]
            .as_object()
            .is_some_and(|residuals| residuals
                .values()
                .all(|value| value.get("evaluated").is_some()))
    );
    assert!(
        evidence["rest_bind_scale"]["residual_comparison_counts"]
            .as_object()
            .is_some_and(|counts| counts.values().all(Value::is_u64))
    );
    let document = animsmith_gltf::load(&dir.path().join("character.glb")).unwrap();
    assert_eq!(document.clips.len(), 2);
    for clip in &document.clips {
        let track = clip
            .tracks
            .iter()
            .find(|track| track.property == Property::Translation)
            .expect("emitted translation track");
        assert_eq!(track.interpolation, Interpolation::CubicSpline);
        let TrackValues::Vec3s(values) = &track.values else {
            panic!("translation values")
        };
        assert_eq!(values.len(), 6);
        assert!((values[0].x - 0.02).abs() < 1.0e-6, "incoming tangent");
        assert!((values[2].x - 0.03).abs() < 1.0e-6, "outgoing tangent");
    }

    let first_artifact = artifact;
    let first_evidence = evidence_bytes;
    let second = run(dir.path());
    assert!(second.status.success());
    assert_eq!(
        std::fs::read(dir.path().join("character.glb")).unwrap(),
        first_artifact
    );
    assert_eq!(
        std::fs::read(dir.path().join("character.json")).unwrap(),
        first_evidence
    );
}

#[test]
fn v4_rejects_an_orientation_basis_mismatch_atomically() {
    let dir = tempfile::tempdir().expect("temporary directory");
    std::fs::create_dir(dir.path().join("inputs")).unwrap();
    std::fs::write(
        dir.path().join("inputs/base.glb"),
        rest_bind_scale_rig_glb(),
    )
    .unwrap();
    let mut clip: Value = serde_json::from_slice(&rest_bind_scale_rig_gltf()).unwrap();
    clip["nodes"][1]["rotation"] = serde_json::json!([0.0, 0.0, 0.001, 0.9999995]);
    std::fs::write(
        dir.path().join("inputs/clip.gltf"),
        serde_json::to_vec(&clip).unwrap(),
    )
    .unwrap();
    std::fs::write(dir.path().join("recipe.toml"), recipe("clip.gltf")).unwrap();

    let output = run(dir.path());
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("named-topology-rest-orientation"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!dir.path().join("character.glb").exists());
    assert!(!dir.path().join("character.json").exists());
}

#[test]
fn v4_rejects_an_unsupported_clip_before_any_remap_or_publication() {
    let dir = tempfile::tempdir().expect("temporary directory");
    std::fs::create_dir(dir.path().join("inputs")).unwrap();
    std::fs::write(
        dir.path().join("inputs/base.glb"),
        rest_bind_scale_rig_glb(),
    )
    .unwrap();
    let mut clip: Value = serde_json::from_slice(&rest_bind_scale_rig_gltf()).unwrap();
    clip["nodes"][2]["extras"] = serde_json::json!({ "private": true });
    std::fs::write(
        dir.path().join("inputs/clip.gltf"),
        serde_json::to_vec(&clip).unwrap(),
    )
    .unwrap();
    std::fs::write(dir.path().join("recipe.toml"), recipe("clip.gltf")).unwrap();

    let output = run(dir.path());
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("preflight rejected input clip.gltf"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!dir.path().join("character.glb").exists());
    assert!(!dir.path().join("character.json").exists());
}

#[test]
fn v4_has_no_default_rest_bind_operation() {
    let dir = tempfile::tempdir().expect("temporary directory");
    std::fs::create_dir(dir.path().join("inputs")).unwrap();
    let input = rest_bind_scale_rig_glb();
    std::fs::write(dir.path().join("inputs/base.glb"), &input).unwrap();
    std::fs::write(dir.path().join("inputs/clip.glb"), &input).unwrap();
    let recipe = recipe("clip.glb").replace(
        "[rest_bind_scale]\nsource_skin_index = 0\nsource_root_node_index = 0\nexpected_factor = 0.01\n\n",
        "",
    );
    std::fs::write(dir.path().join("recipe.toml"), recipe).unwrap();

    let output = run(dir.path());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let evidence: Value =
        serde_json::from_slice(&std::fs::read(dir.path().join("character.json")).unwrap()).unwrap();
    assert_schema(&evidence, EVIDENCE_SCHEMA);
    assert_eq!(evidence["schema_version"], 4);
    assert!(evidence.get("rest_bind_scale").is_none());
    let document = animsmith_gltf::load(&dir.path().join("character.glb")).unwrap();
    assert_eq!(document.skeleton.bones[0].rest.scale, Vec3::splat(0.01));
}
