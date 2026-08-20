//! Public-boundary coverage for character-assembly rest/bind scale contracts.

#![cfg(feature = "fbx")]

use animsmith_core::glam::{Mat4, Quat, Vec3};
use animsmith_core::model::{Document, Interpolation, Property, TrackValues};
use animsmith_core::scale::{
    AssemblyScaleBasis, ScaleOperation, ScaleRequest, assembly_scale_basis, plan_scale,
};
use animsmith_core::sha256_hex;
use animsmith_testkit::{rest_bind_scale_rig_glb, rest_bind_scale_rig_gltf};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::Path;
use std::process::{Command, Output};

const RECIPE_SCHEMA: &str =
    include_str!("../../../docs/schemas/character-assembly-recipe-v4.schema.json");
const EVIDENCE_SCHEMA: &str =
    include_str!("../../../docs/schemas/character-assembly-evidence-v4.schema.json");
const RECIPE_SCHEMA_V5: &str =
    include_str!("../../../docs/schemas/character-assembly-recipe-v5.schema.json");
const EVIDENCE_SCHEMA_V5: &str =
    include_str!("../../../docs/schemas/character-assembly-evidence-v5.schema.json");
const RECIPE_SCHEMA_V6: &str =
    include_str!("../../../docs/schemas/character-assembly-recipe-v6.schema.json");
const EVIDENCE_SCHEMA_V6: &str =
    include_str!("../../../docs/schemas/character-assembly-evidence-v6.schema.json");
const RIGGED_TRIANGLE_FBX: &str = include_str!("../../animsmith-fbx/testdata/rigged_triangle.fbx");

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

fn recipe_v5(clip: &str) -> String {
    recipe(clip)
        .replacen("schema_version = 4", "schema_version = 5", 1)
        .replacen(
            "urn:animsmith:schema:character-assembly-recipe:4",
            "urn:animsmith:schema:character-assembly-recipe:5",
            1,
        )
        .replacen(
            "fps = 30.0",
            "fps = 30.0\ncanonicalize_skin = true\nground_and_center = true\nremove_nodes = [\"attach\"]",
            1,
        )
}

fn fbx_recipe_v6(clip: &str) -> String {
    format!(
        r#"schema_version = 6
schema = "urn:animsmith:schema:character-assembly-recipe:6"
input_root = "inputs"
base_input = "base.fbx"
fps = 30.0

[rest_bind_scale]
source_skin_index = 0
source_root_node_index = 1
expected_factor = 0.01

[[clips]]
name = "walk"
input = "{clip}"
take = "take"
"#
    )
}

fn write_cubic_asset_from(path: &Path, bytes: &[u8], offset: f32) {
    let mut document = animsmith_gltf::load_bytes(Path::new("source.glb"), bytes).unwrap();
    let track = document.clips[0]
        .tracks
        .iter_mut()
        .find(|track| track.property == Property::Translation)
        .expect("fixture translation track");
    track.interpolation = Interpolation::CubicSpline;
    track.times = vec![0.0, 1.0];
    track.values = TrackValues::Vec3s(vec![
        Vec3::new(offset + 2.0, 1.0, -1.0),
        Vec3::new(offset, 100.0, 2.0),
        Vec3::new(offset + 3.0, 2.0, -2.0),
        Vec3::new(offset + 4.0, 3.0, -3.0),
        Vec3::new(offset, 200.0, 4.0),
        Vec3::new(offset + 5.0, 4.0, -4.0),
    ]);
    animsmith_gltf::write::write(&document, path).expect("writes cubic fixture");
}

fn write_cubic_asset(path: &Path, offset: f32) {
    write_cubic_asset_from(path, &rest_bind_scale_rig_glb(), offset);
}

fn write_scale_sensitive_clip_asset(path: &Path, translation_end_y: f32) {
    let mut document =
        animsmith_gltf::load_bytes(Path::new("source.glb"), &rest_bind_scale_rig_glb()).unwrap();
    let translation = document.clips[0]
        .tracks
        .iter_mut()
        .find(|track| track.property == Property::Translation)
        .expect("fixture translation track");
    translation.values = TrackValues::Vec3s(vec![
        Vec3::new(0.0, 100.0, 0.0),
        Vec3::new(0.0, translation_end_y, 0.0),
    ]);
    let rotation = document.clips[0]
        .tracks
        .iter_mut()
        .find(|track| track.property == Property::Rotation)
        .expect("fixture rotation track");
    rotation.bone = 0;
    rotation.values = TrackValues::Quats(vec![Quat::IDENTITY, Quat::from_rotation_z(0.2)]);
    animsmith_gltf::write::write(&document, path).expect("writes scale-sensitive fixture");
}

fn factor_two_rig_glb() -> Vec<u8> {
    let mut bytes = rest_bind_scale_rig_glb();
    let json_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let json = &mut bytes[20..20 + json_len];
    let authored = b"\"scale\": [0.01, 0.01, 0.01]";
    let replacement = b"\"scale\": [2.00, 2.00, 2.00]";
    let at = json
        .windows(authored.len())
        .position(|window| window == authored)
        .unwrap();
    json[at..at + authored.len()].copy_from_slice(replacement);
    let bin_start = 20 + json_len + 8;
    let inverse_bind: [f32; 16] = [
        0.5, 0.0, 0.0, 0.0, //
        0.0, 0.5, 0.0, 0.0, //
        0.0, 0.0, 0.5, 0.0, //
        0.0, -100.0, 0.0, 1.0,
    ];
    for (index, value) in inverse_bind.into_iter().enumerate() {
        bytes[bin_start + 108 + index * 4..bin_start + 112 + index * 4]
            .copy_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn run(dir: &Path) -> Output {
    run_to(dir, "recipe.toml", "character.glb", "character.json")
}

fn run_to(dir: &Path, recipe: &str, output: &str, evidence: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .current_dir(dir)
        .args([
            "assemble",
            recipe,
            "-o",
            output,
            "--evidence",
            evidence,
            "--format",
            "json",
        ])
        .output()
        .expect("runs assemble")
}

fn rest_worlds(document: &Document) -> Vec<Mat4> {
    let mut worlds = Vec::with_capacity(document.skeleton.bones.len());
    for bone in &document.skeleton.bones {
        let local = bone.rest.to_mat4();
        worlds.push(bone.parent.map_or(local, |parent| worlds[parent] * local));
    }
    worlds
}

fn refusal_detail(output: &Output) -> String {
    assert!(output.stderr.is_empty(), "JSON refusals are stdout-only");
    let record: Value = serde_json::from_slice(&output.stdout).expect("typed refusal JSON");
    assert_eq!(record["schema"], "urn:animsmith:schema:producer-refusal:1");
    assert_eq!(record["command"], "assemble");
    assert_eq!(record["outcome"], "rejected");
    record["rejection"]["detail"]
        .as_str()
        .expect("refusal detail")
        .to_owned()
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

fn expected_basis_fingerprint(path: &Path, tool: &Value) -> String {
    let bytes = std::fs::read(path).unwrap();
    let source = animsmith_gltf::preflight_scale_source_bytes(path, &bytes).unwrap();
    let operation = ScaleOperation::RestBindUniformScale {
        source_skin_index: 0,
        source_root_node_index: 0,
        expected_factor: 0.01,
    };
    let facts = animsmith_gltf::operation_capability_facts(source.manifest(), operation).unwrap();
    let plan = plan_scale(&ScaleRequest {
        operation,
        document: source.document(),
        capability: &facts,
    })
    .unwrap();
    let basis = assembly_scale_basis(source.document(), &plan).unwrap();
    #[derive(Serialize)]
    struct ToolSource<'a> {
        revision: &'a Value,
        dirty: &'a Value,
    }
    #[derive(Serialize)]
    struct Tool<'a> {
        name: &'a Value,
        version: &'a Value,
        source: ToolSource<'a>,
    }
    #[derive(Serialize)]
    struct Fingerprint<'a> {
        schema: &'static str,
        tool: Tool<'a>,
        input_sha256: String,
        basis: &'a AssemblyScaleBasis,
    }
    let fingerprint = Fingerprint {
        schema: "urn:animsmith:character-assembly-scale-basis:1",
        tool: Tool {
            name: &tool["name"],
            version: &tool["version"],
            source: ToolSource {
                revision: &tool["source"]["revision"],
                dirty: &tool["source"]["dirty"],
            },
        },
        input_sha256: sha256_hex(&bytes),
        basis: &basis,
    };
    sha256_hex(&serde_json::to_vec(&fingerprint).unwrap())
}

#[test]
fn v4_rebases_before_remap_then_proves_and_publishes_the_exact_final_artifact() {
    let dir = tempfile::tempdir().expect("temporary directory");
    std::fs::create_dir(dir.path().join("inputs")).unwrap();
    write_cubic_asset(&dir.path().join("inputs/base.glb"), 0.0);
    write_cubic_asset(&dir.path().join("inputs/clip.glb"), 10.0);
    write_cubic_asset(&dir.path().join("inputs/clip-two.glb"), 20.0);
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
    let evidence_schema: Value = serde_json::from_str(EVIDENCE_SCHEMA).unwrap();
    let evidence_validator = jsonschema::validator_for(&evidence_schema).unwrap();
    for pointer in ["/schema", "/rest_bind_scale/inputs/0/basis_schema"] {
        let mut wrong_identity = evidence.clone();
        *wrong_identity.pointer_mut(pointer).unwrap() = Value::String("urn:wrong:identity".into());
        assert!(
            !evidence_validator.is_valid(&wrong_identity),
            "schema admitted mismatched identity at {pointer}"
        );
    }
    assert_eq!(evidence["schema_version"], 4);
    let scale = &evidence["rest_bind_scale"];
    assert_eq!(scale["source_skin_index"], 0);
    assert_eq!(scale["source_root_node_index"], 0);
    assert_eq!(scale["expected_factor"], 0.01);
    let inputs = scale["inputs"].as_array().unwrap();
    assert_eq!(inputs.len(), 3);
    for (input, (role, declared, expected_sha256)) in inputs.iter().zip([
        (
            "base",
            "base.glb",
            "7e3b530146affb9e43c19305845c7df8132ccaada9b4d01effb96f37d71c5e90",
        ),
        (
            "clip:walk",
            "clip.glb",
            "f6836542a41ed13bb25b671132714eea16ff39810f57fbfba113fe84ce1dc44a",
        ),
        (
            "clip:run",
            "clip-two.glb",
            "2b917418748a51a263208a8cec32985427c0e6ec07469b362daf0e92ae4b219d",
        ),
    ]) {
        let bytes = std::fs::read(dir.path().join("inputs").join(declared)).unwrap();
        assert_eq!(input["role"], role);
        assert_eq!(input["declared_path"], declared);
        assert_eq!(input["bytes"], bytes.len());
        assert_eq!(bytes.len(), 2516);
        assert_eq!(input["sha256"], sha256_hex(&bytes));
        assert_eq!(input["sha256"], expected_sha256);
        assert_eq!(
            input["basis_schema"],
            "urn:animsmith:character-assembly-scale-basis:1"
        );
        assert_eq!(
            input["basis_fingerprint"],
            expected_basis_fingerprint(
                &dir.path().join("inputs").join(declared),
                &evidence["tool"]
            )
        );
        assert_eq!(input["compatible"], true);
        assert_eq!(input["compatibility"], "compatible");
    }
    assert_ne!(inputs[0]["sha256"], inputs[1]["sha256"]);
    assert_ne!(inputs[1]["sha256"], inputs[2]["sha256"]);
    let mut different_tool = evidence["tool"].clone();
    different_tool["version"] = Value::String("999.0.0".into());
    assert_ne!(
        inputs[0]["basis_fingerprint"],
        expected_basis_fingerprint(&dir.path().join("inputs/base.glb"), &different_tool),
        "tool identity is fingerprint material"
    );
    let digest_only_variant = dir.path().join("inputs/clip-digest-only.gltf");
    let mut semantically_equal = rest_bind_scale_rig_gltf();
    semantically_equal.extend_from_slice(b"\n");
    std::fs::write(&digest_only_variant, semantically_equal).unwrap();
    assert_ne!(
        inputs[1]["basis_fingerprint"],
        expected_basis_fingerprint(&digest_only_variant, &evidence["tool"]),
        "exact input digest is fingerprint material"
    );
    assert_eq!(evidence["artifact"]["sha256"], sha256_hex(&artifact));
    assert_eq!(
        evidence["artifact"]["sha256"],
        "0d60b71f18fc265ff61c7b0b7501d3fc6de23a5313776826ad79c411022373ec"
    );
    assert_eq!(evidence["artifact"]["bytes"], 3424);
    assert_eq!(
        scale["staged_source_sha256"],
        "38d3a0855cb8ad9cf56ad72614f6c96df8869fad6c5eb3d45a6746173946841a"
    );
    assert_eq!(scale["read_back_sha256"], evidence["artifact"]["sha256"]);
    assert_eq!(
        scale["proof"]["artifact"]["sha256"],
        evidence["artifact"]["sha256"]
    );
    assert_eq!(scale["proof"]["artifact"]["bytes"], artifact.len());
    assert_eq!(scale["proof"]["proof"]["read_back_digest_matches"], true);
    assert!(
        scale["proof"]["proof"]["residuals"]
            .as_object()
            .is_some_and(|residuals| residuals
                .values()
                .all(|value| value.get("evaluated").is_some()))
    );
    assert_eq!(
        scale["residual_comparison_counts"],
        serde_json::json!({
            "bounds": 42,
            "cubic_interior": 2,
            "key_translation": 4,
            "mesh_position": 3,
            "rest_rotation": 3,
            "rest_translation": 3,
            "skin_matrix": 7,
            "track_value": 16,
            "trajectory": 18,
            "transform_only_affine": 1,
            "unaffected_inverse_bind": 0,
            "unit_scale": 3
        })
    );
    let document = animsmith_gltf::load(&dir.path().join("character.glb")).unwrap();
    assert_eq!(document.clips.len(), 2);
    for (clip, offset) in document.clips.iter().zip([10.0, 20.0]) {
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
        let expected = [
            Vec3::new(offset + 2.0, 1.0, -1.0),
            Vec3::new(offset, 100.0, 2.0),
            Vec3::new(offset + 3.0, 2.0, -2.0),
            Vec3::new(offset + 4.0, 3.0, -3.0),
            Vec3::new(offset, 200.0, 4.0),
            Vec3::new(offset + 5.0, 4.0, -4.0),
        ];
        for (slot, expected) in values.iter().zip(expected) {
            assert!(slot.abs_diff_eq(expected * 0.01, 1.0e-6));
        }
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
fn v4_rebases_every_cubic_slot_for_a_factor_greater_than_one() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("inputs")).unwrap();
    let authored = factor_two_rig_glb();
    write_cubic_asset_from(&dir.path().join("inputs/base.glb"), &authored, 0.0);
    write_cubic_asset_from(&dir.path().join("inputs/clip.glb"), &authored, 10.0);
    std::fs::write(
        dir.path().join("recipe.toml"),
        recipe("clip.glb").replace("expected_factor = 0.01", "expected_factor = 2.0"),
    )
    .unwrap();

    let output = run(dir.path());
    assert!(
        output.status.success(),
        "stderr: {}\nstdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    let document = animsmith_gltf::load(&dir.path().join("character.glb")).unwrap();
    let track = document.clips[0]
        .tracks
        .iter()
        .find(|track| track.property == Property::Translation)
        .unwrap();
    assert_eq!(track.interpolation, Interpolation::CubicSpline);
    let TrackValues::Vec3s(values) = &track.values else {
        panic!("translation values")
    };
    let expected = [
        Vec3::new(12.0, 1.0, -1.0),
        Vec3::new(10.0, 100.0, 2.0),
        Vec3::new(13.0, 2.0, -2.0),
        Vec3::new(14.0, 3.0, -3.0),
        Vec3::new(10.0, 200.0, 4.0),
        Vec3::new(15.0, 4.0, -4.0),
    ];
    for (slot, expected) in values.iter().zip(expected) {
        assert!(slot.abs_diff_eq(expected * 2.0, 1.0e-5));
    }
}

#[test]
fn v4_strip_bone_motion_evidence_uses_the_rebased_clip_basis() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("inputs")).unwrap();
    write_scale_sensitive_clip_asset(&dir.path().join("inputs/base.glb"), 300.0);
    write_scale_sensitive_clip_asset(&dir.path().join("inputs/clip.glb"), 300.0);
    let recipe = recipe("clip.glb").replace(
        "take = \"clip\"\n",
        "take = \"clip\"\nstrip_bones = [\"joint\"]\n",
    );
    std::fs::write(dir.path().join("recipe.toml"), &recipe).unwrap();

    let output = run(dir.path());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let evidence: Value =
        serde_json::from_slice(&std::fs::read(dir.path().join("character.json")).unwrap()).unwrap();
    assert_eq!(evidence["clips"][0]["stripped_tracks"], 1);
    assert_eq!(
        evidence["clips"][0]["stripped_bone_motion"],
        serde_json::json!([{
            "bone": "joint",
            "translation_start": [0.0, 1.0, 0.0],
            "translation_end": [0.0, 3.0, 0.0],
            "translation_delta": [0.0, 2.0, 0.0],
            "duration_s": 1.0
        }])
    );
    let document = animsmith_gltf::load(&dir.path().join("character.glb")).unwrap();
    assert_eq!(document.clips.len(), 1);
    assert_eq!(document.clips[0].tracks.len(), 1);
    assert_eq!(document.clips[0].tracks[0].bone, 0);
    assert_eq!(document.clips[0].tracks[0].property, Property::Rotation);
}

#[test]
fn v4_prunes_constant_tracks_after_rebasing_the_clip() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("inputs")).unwrap();
    write_scale_sensitive_clip_asset(&dir.path().join("inputs/base.glb"), 100.005);
    write_scale_sensitive_clip_asset(&dir.path().join("inputs/clip.glb"), 100.005);
    let recipe =
        recipe("clip.glb").replacen("fps = 30.0", "fps = 30.0\nprune_constant_tracks = true", 1);
    std::fs::write(dir.path().join("recipe.toml"), recipe).unwrap();

    let output = run(dir.path());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let evidence: Value =
        serde_json::from_slice(&std::fs::read(dir.path().join("character.json")).unwrap()).unwrap();
    assert_eq!(
        evidence["clips"][0]["pruned_constant_tracks"],
        serde_json::json!([{
            "original_track_index": 0,
            "bone": "joint",
            "bone_index": 1,
            "property": "translation",
            "interpolation": "linear",
            "key_count": 2
        }])
    );
    let document = animsmith_gltf::load(&dir.path().join("character.glb")).unwrap();
    assert_eq!(document.clips.len(), 1);
    assert_eq!(document.clips[0].tracks.len(), 1);
    assert_eq!(document.clips[0].tracks[0].property, Property::Rotation);
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
    assert_eq!(output.status.code(), Some(1));
    assert!(refusal_detail(&output).contains("named-orientation"));
    assert!(!dir.path().join("character.glb").exists());
    assert!(!dir.path().join("character.json").exists());
}

#[test]
fn v5_missing_source_selectors_refuse_before_replacing_a_prior_pair() {
    for (replacement, expected_detail) in [
        ("source_skin_index = 9", "skin"),
        ("source_root_node_index = 9", "root"),
    ] {
        let dir = tempfile::tempdir().expect("temporary directory");
        std::fs::create_dir(dir.path().join("inputs")).unwrap();
        write_cubic_asset(&dir.path().join("inputs/base.glb"), 0.0);
        write_cubic_asset(&dir.path().join("inputs/walk.glb"), 10.0);
        let recipe = if replacement.starts_with("source_skin") {
            recipe_v5("walk.glb").replace("source_skin_index = 0", replacement)
        } else {
            recipe_v5("walk.glb").replace("source_root_node_index = 0", replacement)
        };
        std::fs::write(dir.path().join("recipe.toml"), recipe).unwrap();
        let prior_artifact = b"prior artifact";
        let prior_evidence = b"prior evidence";
        std::fs::write(dir.path().join("character.glb"), prior_artifact).unwrap();
        std::fs::write(dir.path().join("character.json"), prior_evidence).unwrap();

        let output = run(dir.path());
        assert_eq!(output.status.code(), Some(1));
        assert!(refusal_detail(&output).contains(expected_detail));
        assert_eq!(
            std::fs::read(dir.path().join("character.glb")).unwrap(),
            prior_artifact
        );
        assert_eq!(
            std::fs::read(dir.path().join("character.json")).unwrap(),
            prior_evidence
        );
    }
}

#[test]
fn v5_ambiguous_post_assembly_skin_selector_refuses_atomically() {
    // Two source skins with the same selected, named joint topology are
    // individually valid glTF. The composed producer must not silently pick
    // the first staged skin after canonicalization shifts raw source indices.
    let dir = tempfile::tempdir().expect("temporary directory");
    std::fs::create_dir(dir.path().join("inputs")).unwrap();
    let mut ambiguous: Value = serde_json::from_slice(&rest_bind_scale_rig_gltf()).unwrap();
    let second_skin = ambiguous["skins"][0].clone();
    ambiguous["skins"].as_array_mut().unwrap().push(second_skin);
    ambiguous["nodes"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "name": "holder2",
            "mesh": 0,
            "skin": 1
        }));
    ambiguous["scenes"][0]["nodes"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!(4));
    let bytes = serde_json::to_vec(&ambiguous).unwrap();
    std::fs::write(dir.path().join("inputs/base.gltf"), &bytes).unwrap();
    std::fs::write(dir.path().join("inputs/walk.gltf"), bytes).unwrap();
    let recipe = recipe_v5("walk.gltf").replace("base.glb", "base.gltf");
    std::fs::write(dir.path().join("recipe.toml"), recipe).unwrap();
    let prior_artifact = b"prior artifact";
    let prior_evidence = b"prior evidence";
    std::fs::write(dir.path().join("character.glb"), prior_artifact).unwrap();
    std::fs::write(dir.path().join("character.json"), prior_evidence).unwrap();

    let output = run(dir.path());
    assert_eq!(output.status.code(), Some(1));
    assert!(
        refusal_detail(&output).contains("exactly one skin with the selected named joint topology"),
        "the refusal identifies the ambiguous selected skin topology: {}",
        refusal_detail(&output)
    );
    assert_eq!(
        std::fs::read(dir.path().join("character.glb")).unwrap(),
        prior_artifact
    );
    assert_eq!(
        std::fs::read(dir.path().join("character.json")).unwrap(),
        prior_evidence
    );
}

#[test]
fn v4_accepts_quaternion_sign_and_in_band_rest_spelling_differences() {
    let dir = tempfile::tempdir().expect("temporary directory");
    std::fs::create_dir(dir.path().join("inputs")).unwrap();
    std::fs::write(
        dir.path().join("inputs/base.glb"),
        rest_bind_scale_rig_glb(),
    )
    .unwrap();
    let mut clip: Value = serde_json::from_slice(&rest_bind_scale_rig_gltf()).unwrap();
    clip["nodes"][1]["rotation"] = serde_json::json!([-0.0, -0.0, -0.0, -1.0]);
    clip["nodes"][1]["translation"] = serde_json::json!([0.0, 100.00001, 0.0]);
    std::fs::write(
        dir.path().join("inputs/clip.gltf"),
        serde_json::to_vec(&clip).unwrap(),
    )
    .unwrap();
    std::fs::write(dir.path().join("recipe.toml"), recipe("clip.gltf")).unwrap();

    let output = run(dir.path());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(dir.path().join("character.glb").exists());
    assert!(dir.path().join("character.json").exists());
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
    std::fs::write(
        dir.path().join("inputs/clip.gltf"),
        rest_bind_scale_rig_gltf(),
    )
    .unwrap();
    std::fs::write(
        dir.path().join("inputs/later.gltf"),
        rest_bind_scale_rig_gltf(),
    )
    .unwrap();
    let recipe = format!(
        "{}\n[[clips]]\nname = \"later\"\ninput = \"later.gltf\"\ntake = \"clip\"\n",
        recipe("clip.gltf")
    );
    std::fs::write(dir.path().join("recipe.toml"), &recipe).unwrap();
    let published = run(dir.path());
    assert!(published.status.success());
    let prior_artifact = std::fs::read(dir.path().join("character.glb")).unwrap();
    let prior_evidence = std::fs::read(dir.path().join("character.json")).unwrap();

    let mut clip: Value = serde_json::from_slice(&rest_bind_scale_rig_gltf()).unwrap();
    clip["nodes"][2]["extras"] = serde_json::json!({ "private": true });
    std::fs::write(
        dir.path().join("inputs/later.gltf"),
        serde_json::to_vec(&clip).unwrap(),
    )
    .unwrap();

    let output = run(dir.path());
    assert_eq!(output.status.code(), Some(1));
    assert!(refusal_detail(&output).contains("preflight rejected input later.gltf"));
    assert_eq!(
        std::fs::read(dir.path().join("character.glb")).unwrap(),
        prior_artifact
    );
    assert_eq!(
        std::fs::read(dir.path().join("character.json")).unwrap(),
        prior_evidence
    );
}

#[test]
fn v4_rejects_an_unsupported_base_before_publication() {
    let dir = tempfile::tempdir().expect("temporary directory");
    std::fs::create_dir(dir.path().join("inputs")).unwrap();
    let mut base: Value = serde_json::from_slice(&rest_bind_scale_rig_gltf()).unwrap();
    base["nodes"][2]["extras"] = serde_json::json!({ "private": true });
    std::fs::write(
        dir.path().join("inputs/base.glb"),
        serde_json::to_vec(&base).unwrap(),
    )
    .unwrap();
    std::fs::write(
        dir.path().join("inputs/clip.glb"),
        rest_bind_scale_rig_glb(),
    )
    .unwrap();
    std::fs::write(dir.path().join("recipe.toml"), recipe("clip.glb")).unwrap();

    let output = run(dir.path());
    assert_eq!(output.status.code(), Some(1));
    assert!(refusal_detail(&output).contains("preflight rejected input base.glb"));
    assert!(!dir.path().join("character.glb").exists());
    assert!(!dir.path().join("character.json").exists());
}

#[test]
fn v4_active_block_rejects_fbx_instead_of_claiming_complete_coverage() {
    let dir = tempfile::tempdir().expect("temporary directory");
    std::fs::create_dir(dir.path().join("inputs")).unwrap();
    std::fs::write(dir.path().join("inputs/base.fbx"), RIGGED_TRIANGLE_FBX).unwrap();
    std::fs::write(
        dir.path().join("inputs/clip.glb"),
        rest_bind_scale_rig_glb(),
    )
    .unwrap();
    let recipe = recipe("clip.glb").replace("base.glb", "base.fbx");
    std::fs::write(dir.path().join("recipe.toml"), recipe).unwrap();

    let output = run(dir.path());
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("rest_bind_scale input base.fbx is not glTF/GLB"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!dir.path().join("character.glb").exists());
    assert!(!dir.path().join("character.json").exists());
}

#[test]
fn v6_assembles_eligible_fbx_base_and_clip_through_one_proved_glb() {
    let dir = tempfile::tempdir().expect("temporary directory");
    std::fs::create_dir(dir.path().join("inputs")).unwrap();
    std::fs::write(dir.path().join("inputs/base.fbx"), RIGGED_TRIANGLE_FBX).unwrap();
    std::fs::write(dir.path().join("inputs/walk.fbx"), RIGGED_TRIANGLE_FBX).unwrap();
    let recipe = fbx_recipe_v6("walk.fbx");
    let recipe_value: toml::Value = toml::from_str(&recipe).unwrap();
    assert_schema(
        &serde_json::to_value(recipe_value).unwrap(),
        RECIPE_SCHEMA_V6,
    );
    std::fs::write(dir.path().join("recipe.toml"), recipe).unwrap();

    let first = run(dir.path());
    assert!(
        first.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(first.stderr.is_empty());
    let artifact = std::fs::read(dir.path().join("character.glb")).unwrap();
    let evidence_bytes = std::fs::read(dir.path().join("character.json")).unwrap();
    assert_eq!(first.stdout, evidence_bytes);
    let evidence: Value = serde_json::from_slice(&evidence_bytes).unwrap();
    assert_schema(&evidence, EVIDENCE_SCHEMA_V6);
    assert_eq!(evidence["schema_version"], 6);
    assert_eq!(
        evidence["schema"],
        "urn:animsmith:schema:character-assembly-evidence:6"
    );
    assert_eq!(evidence["artifact"]["sha256"], sha256_hex(&artifact));
    let scale = &evidence["rest_bind_scale"];
    assert_eq!(scale["read_back_sha256"], evidence["artifact"]["sha256"]);
    assert_eq!(scale["proof"]["proof"]["read_back_digest_matches"], true);
    assert_eq!(scale["inputs"].as_array().unwrap().len(), 2);
    for (input, role, declared) in [
        (&scale["inputs"][0], "base", "base.fbx"),
        (&scale["inputs"][1], "clip:walk", "walk.fbx"),
    ] {
        assert_eq!(input["role"], role);
        assert_eq!(input["declared_path"], declared);
        assert_eq!(input["input_format"], "fbx");
        let projection = &input["source_projection"];
        assert_eq!(projection["kind"], "normalized-baked-fbx");
        assert_eq!(projection["authored_curve_keys_preserved"], false);
        assert_eq!(projection["raw_source_spans_preserved"], false);
        assert_eq!(projection["capability"]["animation_takes_baked"], true);
        assert_eq!(
            projection["capability"]["authored_curve_keys_preserved"],
            false
        );
        assert_eq!(
            projection["capability"]["domains"]["translation_animation"],
            "baked"
        );
        assert!(projection["staged_source"]["bytes"].as_u64().unwrap() > 0);
    }
    let assembled = animsmith_gltf::load(&dir.path().join("character.glb")).unwrap();
    assert_eq!(assembled.clips.len(), 1);
    assert_eq!(assembled.clips[0].name, "walk");
    assert!(
        assembled
            .skeleton
            .bones
            .iter()
            .all(|bone| bone.rest.scale == Vec3::ONE),
        "the published rest hierarchy is fully reparameterized"
    );

    let second = run(dir.path());
    assert!(second.status.success());
    assert_eq!(
        std::fs::read(dir.path().join("character.glb")).unwrap(),
        artifact
    );
    assert_eq!(second.stdout, evidence_bytes);
    assert_eq!(
        std::fs::read(dir.path().join("character.json")).unwrap(),
        evidence_bytes
    );
}

#[test]
fn v6_refuses_an_incomplete_fbx_clip_inventory_atomically() {
    let dir = tempfile::tempdir().expect("temporary directory");
    std::fs::create_dir(dir.path().join("inputs")).unwrap();
    std::fs::write(dir.path().join("inputs/base.fbx"), RIGGED_TRIANGLE_FBX).unwrap();
    let unsupported = RIGGED_TRIANGLE_FBX.replacen(
        "\t\t\tP: \"Lcl Scaling\", \"Lcl Scaling\", \"\", \"A\",1,1,1",
        "\t\t\tP: \"Lcl Scaling\", \"Lcl Scaling\", \"\", \"A\",1,1,1\n\t\t\tP: \"PipelineTag\", \"KString\", \"\", \"U\",\"unsupported\"",
        1,
    );
    std::fs::write(dir.path().join("inputs/walk.fbx"), unsupported).unwrap();
    std::fs::write(dir.path().join("recipe.toml"), fbx_recipe_v6("walk.fbx")).unwrap();
    let prior_artifact = b"prior artifact";
    let prior_evidence = b"prior evidence";
    std::fs::write(dir.path().join("character.glb"), prior_artifact).unwrap();
    std::fs::write(dir.path().join("character.json"), prior_evidence).unwrap();

    let output = run(dir.path());
    assert_eq!(output.status.code(), Some(1));
    assert!(
        refusal_detail(&output).contains("FBX capability rejected input walk.fbx"),
        "{}",
        refusal_detail(&output)
    );
    assert_eq!(
        std::fs::read(dir.path().join("character.glb")).unwrap(),
        prior_artifact
    );
    assert_eq!(
        std::fs::read(dir.path().join("character.json")).unwrap(),
        prior_evidence
    );
}

#[test]
fn v6_gltf_projection_adds_only_the_new_evidence_boundary() {
    let dir = tempfile::tempdir().expect("temporary directory");
    std::fs::create_dir(dir.path().join("inputs")).unwrap();
    let bytes = rest_bind_scale_rig_glb();
    std::fs::write(dir.path().join("inputs/base.glb"), &bytes).unwrap();
    std::fs::write(dir.path().join("inputs/walk.glb"), &bytes).unwrap();
    let v6 = recipe("walk.glb")
        .replacen("schema_version = 4", "schema_version = 6", 1)
        .replacen(
            "urn:animsmith:schema:character-assembly-recipe:4",
            "urn:animsmith:schema:character-assembly-recipe:6",
            1,
        );
    std::fs::write(dir.path().join("recipe.toml"), &v6).unwrap();
    let output = run(dir.path());
    assert!(output.status.success());
    let v6_artifact = std::fs::read(dir.path().join("character.glb")).unwrap();
    let evidence: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_schema(&evidence, EVIDENCE_SCHEMA_V6);
    for input in evidence["rest_bind_scale"]["inputs"].as_array().unwrap() {
        assert_eq!(input["input_format"], "glb");
        assert_eq!(input["source_projection"]["kind"], "raw-gltf");
        assert_eq!(
            input["source_projection"]["authored_curve_keys_preserved"],
            true
        );
        assert_eq!(
            input["source_projection"]["raw_source_spans_preserved"],
            true
        );
    }

    let v5 = v6
        .replacen("schema_version = 6", "schema_version = 5", 1)
        .replacen(
            "urn:animsmith:schema:character-assembly-recipe:6",
            "urn:animsmith:schema:character-assembly-recipe:5",
            1,
        );
    std::fs::write(dir.path().join("legacy.toml"), v5).unwrap();
    let legacy = run_to(dir.path(), "legacy.toml", "legacy.glb", "legacy.json");
    assert!(legacy.status.success());
    assert_eq!(
        std::fs::read(dir.path().join("legacy.glb")).unwrap(),
        v6_artifact,
        "v6 changes evidence/admission, not established glTF artifact semantics"
    );
    let legacy_evidence: Value = serde_json::from_slice(&legacy.stdout).unwrap();
    assert_schema(&legacy_evidence, EVIDENCE_SCHEMA_V5);
    assert!(
        legacy_evidence["rest_bind_scale"]["inputs"]
            .as_array()
            .unwrap()
            .iter()
            .all(|input| input.get("input_format").is_none()
                && input.get("source_projection").is_none())
    );
}

#[test]
fn v4_has_no_default_rest_bind_operation() {
    let dir = tempfile::tempdir().expect("temporary directory");
    std::fs::create_dir(dir.path().join("inputs")).unwrap();
    let input = rest_bind_scale_rig_glb();
    std::fs::write(dir.path().join("inputs/base.glb"), &input).unwrap();
    std::fs::write(
        dir.path().join("inputs/clip.gltf"),
        rest_bind_scale_rig_gltf(),
    )
    .unwrap();
    std::fs::write(dir.path().join("inputs/clip-two.glb"), &input).unwrap();
    let recipe = format!(
        "{}\n[[clips]]\nname = \"run\"\ninput = \"clip-two.glb\"\ntake = \"clip\"\n",
        recipe("clip.gltf")
    )
    .replace(
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
    assert_eq!(document.clips.len(), 2);
    assert!(document.clips.iter().all(|clip| clip.tracks.len() == 2));
}

#[test]
fn v5_composes_rest_bind_scale_with_canonical_grounding_and_node_removal() {
    let dir = tempfile::tempdir().expect("temporary directory");
    std::fs::create_dir(dir.path().join("inputs")).unwrap();
    write_cubic_asset(&dir.path().join("inputs/base.glb"), 0.0);
    write_cubic_asset(&dir.path().join("inputs/walk.glb"), 10.0);
    write_cubic_asset(&dir.path().join("inputs/run.glb"), 20.0);
    write_cubic_asset(&dir.path().join("inputs/stripped.glb"), 30.0);
    let recipe = format!(
        "{}\n[[clips]]\nname = \"run\"\ninput = \"run.glb\"\ntake = \"clip\"\n\n[[clips]]\nname = \"stripped\"\ninput = \"stripped.glb\"\ntake = \"clip\"\nstrip_bones = [\"joint\"]\n",
        recipe_v5("walk.glb")
    );
    let recipe_value: toml::Value = toml::from_str(&recipe).unwrap();
    assert_schema(
        &serde_json::to_value(recipe_value).unwrap(),
        RECIPE_SCHEMA_V5,
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
    assert_schema(&evidence, EVIDENCE_SCHEMA_V5);
    assert_eq!(evidence["schema_version"], 5);
    assert!(
        evidence["transforms"]["canonicalized_skin"]
            .as_bool()
            .unwrap()
    );
    assert!(
        evidence["transforms"]["ground_and_center"]
            .as_bool()
            .unwrap()
    );
    assert_eq!(evidence["transforms"]["removed_nodes"][0]["name"], "attach");
    assert!(evidence["transforms"]["converted_bounds_min"][1].is_number());
    let scale = &evidence["rest_bind_scale"];
    assert_eq!(scale["source_skin_index"], 0);
    assert_eq!(scale["source_root_node_index"], 0);
    assert_eq!(scale["effective_source_skin_index"], 0);
    assert_eq!(scale["effective_source_root_node_index"], 1);
    assert_eq!(scale["expected_factor"], 0.01);
    assert_eq!(scale["inputs"].as_array().unwrap().len(), 4);
    assert_eq!(scale["inputs"][0]["role"], "base");
    assert_eq!(scale["inputs"][1]["role"], "clip:walk");
    assert_eq!(scale["inputs"][2]["role"], "clip:run");
    assert_eq!(scale["inputs"][3]["role"], "clip:stripped");
    assert!(
        scale["inputs"]
            .as_array()
            .unwrap()
            .iter()
            .all(|input| input["compatible"] == true)
    );
    assert_eq!(scale["read_back_sha256"], evidence["artifact"]["sha256"]);
    assert_eq!(
        scale["proof"]["artifact"]["sha256"],
        evidence["artifact"]["sha256"]
    );
    assert_eq!(scale["proof"]["proof"]["read_back_digest_matches"], true);
    assert_eq!(
        scale["residual_comparison_counts"],
        serde_json::json!({
            "bounds": 42,
            "cubic_interior": 2,
            "key_translation": 4,
            "mesh_position": 3,
            "rest_rotation": 2,
            "rest_translation": 2,
            "skin_matrix": 7,
            "track_value": 16,
            "trajectory": 12,
            "transform_only_affine": 0,
            "unaffected_inverse_bind": 0,
            "unit_scale": 2,
        })
    );
    assert_eq!(evidence["clips"].as_array().unwrap().len(), 3);
    let document = animsmith_gltf::load(&dir.path().join("character.glb")).unwrap();
    assert!(
        document
            .skeleton
            .bones
            .iter()
            .all(|bone| bone.name != "attach")
    );
    assert!(
        document
            .skeleton
            .bones
            .iter()
            .all(|bone| bone.rest.scale == Vec3::ONE),
        "the composed rest hierarchy has unit local scale"
    );
    let worlds = rest_worlds(&document);
    let skinned_joints = document
        .assets
        .instances
        .iter()
        .flat_map(|instance| instance.skin_joints.iter().copied())
        .collect::<BTreeSet<_>>();
    for (index, (bone, world)) in document.skeleton.bones.iter().zip(&worlds).enumerate() {
        for axis in [world.x_axis, world.y_axis, world.z_axis] {
            assert!(
                (axis.truncate().length() - 1.0).abs() <= 1.0e-5,
                "{} has a unit rest-world axis",
                bone.name
            );
        }
        if skinned_joints.contains(&index) {
            let inverse_bind = bone
                .inverse_bind
                .expect("emitted skin joint has an inverse bind");
            assert!(
                (*world * inverse_bind).abs_diff_eq(Mat4::IDENTITY, 1.0e-4),
                "{} inverse bind matches the emitted rest world",
                bone.name
            );
        }
    }
    assert_eq!(document.clips.len(), 2);
    let walk = document
        .clips
        .iter()
        .find(|clip| clip.name == "walk")
        .unwrap();
    let track = walk
        .tracks
        .iter()
        .find(|track| track.property == Property::Translation)
        .expect("rebased walk translation");
    assert_eq!(track.interpolation, Interpolation::CubicSpline);
    let TrackValues::Vec3s(values) = &track.values else {
        panic!("translation must remain VEC3");
    };
    let expected = [
        Vec3::new(12.0, 1.0, -1.0),
        Vec3::new(10.0, 100.0, 2.0),
        Vec3::new(13.0, 2.0, -2.0),
        Vec3::new(14.0, 3.0, -3.0),
        Vec3::new(10.0, 200.0, 4.0),
        Vec3::new(15.0, 4.0, -4.0),
    ];
    assert_eq!(values.len(), expected.len());
    for (actual, expected) in values.iter().zip(expected) {
        assert!(actual.abs_diff_eq(expected * 0.01, 1.0e-6));
    }
    let run = document
        .clips
        .iter()
        .find(|clip| clip.name == "run")
        .expect("independently scaled second clip");
    let run_track = run
        .tracks
        .iter()
        .find(|track| track.property == Property::Translation)
        .expect("rebased run translation");
    let TrackValues::Vec3s(run_values) = &run_track.values else {
        panic!("run translation must remain VEC3");
    };
    for (actual, expected) in run_values.iter().zip([
        Vec3::new(22.0, 1.0, -1.0),
        Vec3::new(20.0, 100.0, 2.0),
        Vec3::new(23.0, 2.0, -2.0),
        Vec3::new(24.0, 3.0, -3.0),
        Vec3::new(20.0, 200.0, 4.0),
        Vec3::new(25.0, 4.0, -4.0),
    ]) {
        assert!(actual.abs_diff_eq(expected * 0.01, 1.0e-6));
    }
    assert_eq!(evidence["clips"][2]["stripped_tracks"], 2);
    assert!(
        document.clips.iter().all(|clip| clip.name != "stripped"),
        "a fully stripped clip is omitted only at publication, not from evidence"
    );
}

#[test]
fn v5_composed_selector_removal_refuses_before_replacing_a_prior_pair() {
    let dir = tempfile::tempdir().expect("temporary directory");
    std::fs::create_dir(dir.path().join("inputs")).unwrap();
    write_cubic_asset(&dir.path().join("inputs/base.glb"), 0.0);
    write_cubic_asset(&dir.path().join("inputs/walk.glb"), 10.0);
    let recipe =
        recipe_v5("walk.glb").replace("remove_nodes = [\"attach\"]", "remove_nodes = [\"joint\"]");
    std::fs::write(dir.path().join("recipe.toml"), recipe).unwrap();
    let prior_artifact = b"prior artifact";
    let prior_evidence = b"prior evidence";
    std::fs::write(dir.path().join("character.glb"), prior_artifact).unwrap();
    std::fs::write(dir.path().join("character.json"), prior_evidence).unwrap();

    let output = run(dir.path());
    assert_eq!(output.status.code(), Some(1));
    assert!(
        refusal_detail(&output).contains("selected node"),
        "the removed selected joint is rejected at a located node: {}",
        refusal_detail(&output)
    );
    assert_eq!(
        std::fs::read(dir.path().join("character.glb")).unwrap(),
        prior_artifact
    );
    assert_eq!(
        std::fs::read(dir.path().join("character.json")).unwrap(),
        prior_evidence
    );
}

#[test]
fn v5_composed_artifact_matches_the_canonical_assembly_then_scale_pipeline() {
    let dir = tempfile::tempdir().expect("temporary directory");
    std::fs::create_dir(dir.path().join("inputs")).unwrap();
    write_cubic_asset(&dir.path().join("inputs/base.glb"), 0.0);
    write_cubic_asset(&dir.path().join("inputs/walk.glb"), 10.0);
    let composed_recipe = recipe_v5("walk.glb");
    std::fs::write(dir.path().join("recipe.toml"), &composed_recipe).unwrap();
    let composed = run(dir.path());
    assert!(
        composed.status.success(),
        "{}",
        String::from_utf8_lossy(&composed.stderr)
    );
    let composed_bytes = std::fs::read(dir.path().join("character.glb")).unwrap();

    let stage_recipe = composed_recipe.replace(
        "[rest_bind_scale]\nsource_skin_index = 0\nsource_root_node_index = 0\nexpected_factor = 0.01\n\n",
        "",
    );
    std::fs::write(dir.path().join("stage.toml"), stage_recipe).unwrap();
    let stage = run_to(dir.path(), "stage.toml", "stage.glb", "stage.json");
    assert!(
        stage.status.success(),
        "{}",
        String::from_utf8_lossy(&stage.stderr)
    );
    let scaled = Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .current_dir(dir.path())
        .args([
            "scale",
            "rest-bind",
            "stage.glb",
            "-o",
            "two-stage.glb",
            "--source-skin-index",
            "0",
            "--source-root-node-index",
            "1",
            "--expected-factor=0.01",
            "--evidence",
            "two-stage.json",
            "--format",
            "json",
        ])
        .output()
        .expect("runs standalone rest-bind scale");
    assert!(
        scaled.status.success(),
        "{}",
        String::from_utf8_lossy(&scaled.stderr)
    );
    assert_eq!(
        std::fs::read(dir.path().join("two-stage.glb")).unwrap(),
        composed_bytes,
        "v5 preserves the exact geometry placement and grounding of the established two-stage path"
    );
}

#[test]
fn v5_without_rest_bind_scale_retains_ordinary_assembly() {
    let dir = tempfile::tempdir().expect("temporary directory");
    std::fs::create_dir(dir.path().join("inputs")).unwrap();
    let bytes = rest_bind_scale_rig_glb();
    std::fs::write(dir.path().join("inputs/base.glb"), &bytes).unwrap();
    std::fs::write(dir.path().join("inputs/walk.glb"), &bytes).unwrap();
    let recipe = recipe_v5("walk.glb").replace(
        "[rest_bind_scale]\nsource_skin_index = 0\nsource_root_node_index = 0\nexpected_factor = 0.01\n\n",
        "",
    );
    std::fs::write(dir.path().join("recipe.toml"), &recipe).unwrap();

    let output = run(dir.path());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let evidence: Value =
        serde_json::from_slice(&std::fs::read(dir.path().join("character.json")).unwrap()).unwrap();
    assert_schema(&evidence, EVIDENCE_SCHEMA_V5);
    assert_eq!(evidence["schema_version"], 5);
    assert!(evidence.get("rest_bind_scale").is_none());
    let document = animsmith_gltf::load(&dir.path().join("character.glb")).unwrap();
    assert_eq!(document.skeleton.bones[0].rest.scale, Vec3::ONE);
    assert_eq!(document.clips.len(), 1);
    assert_eq!(document.clips[0].tracks.len(), 2);

    let legacy_recipe = recipe
        .replace("schema_version = 5", "schema_version = 4")
        .replace(
            "urn:animsmith:schema:character-assembly-recipe:5",
            "urn:animsmith:schema:character-assembly-recipe:4",
        );
    std::fs::write(dir.path().join("legacy.toml"), legacy_recipe).unwrap();
    let legacy = run_to(dir.path(), "legacy.toml", "legacy.glb", "legacy.json");
    assert!(
        legacy.status.success(),
        "{}",
        String::from_utf8_lossy(&legacy.stderr)
    );
    assert_eq!(
        std::fs::read(dir.path().join("character.glb")).unwrap(),
        std::fs::read(dir.path().join("legacy.glb")).unwrap(),
        "v5 without rest_bind_scale preserves the v4 ordinary-assembly artifact"
    );
}

#[test]
fn v5_rest_bind_scale_keeps_a_surviving_attachment_at_unit_world_scale() {
    let dir = tempfile::tempdir().expect("temporary directory");
    std::fs::create_dir(dir.path().join("inputs")).unwrap();
    write_cubic_asset(&dir.path().join("inputs/base.glb"), 0.0);
    write_cubic_asset(&dir.path().join("inputs/walk.glb"), 10.0);
    let recipe = recipe_v5("walk.glb").replace("remove_nodes = [\"attach\"]\n", "");
    std::fs::write(dir.path().join("recipe.toml"), recipe).unwrap();

    let output = run(dir.path());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document = animsmith_gltf::load(&dir.path().join("character.glb")).unwrap();
    let attachment = document
        .skeleton
        .bones
        .iter()
        .position(|bone| bone.name == "attach")
        .expect("unselected attachment survives");
    let world = rest_worlds(&document)[attachment];
    for axis in [world.x_axis, world.y_axis, world.z_axis] {
        assert!(
            (axis.truncate().length() - 1.0).abs() <= 1.0e-5,
            "surviving attachment has unit world scale"
        );
    }
}

#[test]
fn v4_rest_bind_recipe_fields_factors_and_conflicts_fail_closed() {
    let base = recipe("clip.glb");
    let cases = [
        (
            base.replace("source_skin_index = 0\n", ""),
            "missing field `source_skin_index`",
        ),
        (
            base.replace("source_root_node_index = 0\n", ""),
            "missing field `source_root_node_index`",
        ),
        (
            base.replace("expected_factor = 0.01\n", ""),
            "missing field `expected_factor`",
        ),
        (
            base.replace("expected_factor = 0.01", "expected_factor = 0.0"),
            "must be finite and greater than zero",
        ),
        (
            base.replace("expected_factor = 0.01", "expected_factor = nan"),
            "must be finite and greater than zero",
        ),
        (
            base.replacen("fps = 30.0", "fps = 30.0\ncanonicalize_skin = true", 1),
            "cannot be combined with canonicalize_skin, ground_and_center, or remove_nodes",
        ),
        (
            base.replacen(
                "fps = 30.0",
                "fps = 30.0\ncanonicalize_skin = true\nground_and_center = true",
                1,
            ),
            "cannot be combined with canonicalize_skin, ground_and_center, or remove_nodes",
        ),
        (
            base.replacen("fps = 30.0", "fps = 30.0\nremove_nodes = [\"joint\"]", 1),
            "cannot be combined with canonicalize_skin, ground_and_center, or remove_nodes",
        ),
        (
            base.replace(
                "urn:animsmith:schema:character-assembly-recipe:4",
                "urn:animsmith:schema:character-assembly-recipe:3",
            ),
            "unsupported assembly recipe identity",
        ),
    ];
    for (ordinal, (invalid, expected)) in cases.into_iter().enumerate() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("inputs")).unwrap();
        let bytes = rest_bind_scale_rig_glb();
        std::fs::write(dir.path().join("inputs/base.glb"), &bytes).unwrap();
        std::fs::write(dir.path().join("inputs/clip.glb"), &bytes).unwrap();
        std::fs::write(dir.path().join("recipe.toml"), invalid).unwrap();
        let output = run(dir.path());
        assert_eq!(output.status.code(), Some(2), "case {ordinal}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected),
            "case {ordinal}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!dir.path().join("character.glb").exists());
        assert!(!dir.path().join("character.json").exists());
    }
}

fn assert_scale_refusal(recipe_text: &str, base: &[u8], clip: &[u8], expected: &str) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("inputs")).unwrap();
    std::fs::write(dir.path().join("inputs/base.glb"), base).unwrap();
    std::fs::write(dir.path().join("inputs/clip.gltf"), clip).unwrap();
    std::fs::write(dir.path().join("recipe.toml"), recipe_text).unwrap();
    let output = run(dir.path());
    assert_eq!(output.status.code(), Some(1));
    let detail = refusal_detail(&output);
    assert!(detail.contains(expected), "expected {expected:?}: {detail}");
    assert!(!dir.path().join("character.glb").exists());
    assert!(!dir.path().join("character.json").exists());
}

#[test]
fn v4_public_selector_factor_topology_and_real_helper_mismatches_fail_closed() {
    let base = rest_bind_scale_rig_glb();
    let valid_clip = rest_bind_scale_rig_gltf();
    assert_scale_refusal(
        &recipe("clip.gltf").replace("source_skin_index = 0", "source_skin_index = 9"),
        &base,
        &valid_clip,
        "source skin index 9 is not a skin",
    );

    let mut factor: Value = serde_json::from_slice(&valid_clip).unwrap();
    factor["nodes"][0]["scale"] = serde_json::json!([0.02, 0.02, 0.02]);
    assert_scale_refusal(
        &recipe("clip.gltf"),
        &base,
        &serde_json::to_vec(&factor).unwrap(),
        "expected factor",
    );

    let mut topology: Value = serde_json::from_slice(&valid_clip).unwrap();
    topology["nodes"][0]["children"] = serde_json::json!([]);
    topology["scenes"][0]["nodes"] = serde_json::json!([0, 1, 3]);
    assert_scale_refusal(
        &recipe("clip.gltf"),
        &base,
        &serde_json::to_vec(&topology).unwrap(),
        "joint_not_descendant_of_scaled_root",
    );

    let mut helper: Value = serde_json::from_slice(&valid_clip).unwrap();
    helper["nodes"][0]["children"] = serde_json::json!([4]);
    helper["nodes"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "matrix": [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0,
                       0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0],
            "children": [1]
        }));
    assert_scale_refusal(
        &recipe("clip.gltf"),
        &base,
        &serde_json::to_vec(&helper).unwrap(),
        "named-topology",
    );
}
