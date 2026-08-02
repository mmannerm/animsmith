//! End-to-end contract for versioned multi-source character assembly.

#![cfg(feature = "fbx")]

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::Path;
use std::process::{Command, Output};

const RIGGED_TRIANGLE_FBX: &str = include_str!("../../animsmith-fbx/testdata/rigged_triangle.fbx");
const EVIDENCE_SCHEMA: &str =
    include_str!("../../../docs/schemas/character-assembly-evidence-v1.schema.json");

fn write_inputs(dir: &Path) {
    std::fs::create_dir(dir.join("inputs")).expect("creates input root");
    std::fs::write(dir.join("inputs/base.fbx"), RIGGED_TRIANGLE_FBX).expect("writes base");
    std::fs::write(dir.join("inputs/motion.fbx"), RIGGED_TRIANGLE_FBX).expect("writes clip");
}

fn success_recipe() -> &'static str {
    concat!(
        "schema_version = 1\n",
        "schema = \"urn:animsmith:schema:character-assembly-recipe:1\"\n",
        "input_root = \"inputs\"\n",
        "base_input = \"base.fbx\"\n",
        "mesh_instances = [\"tri\"]\n",
        "complete_tracks = true\n",
        "canonicalize_skin = true\n",
        "ground_and_center = true\n",
        "fps = 30.0\n",
        "\n",
        "[[clips]]\n",
        "name = \"motion\"\n",
        "input = \"motion.fbx\"\n",
        "take = \"take\"\n",
        "frame_window = [1, 31]\n",
        "drop_closing_endpoint = true\n",
        "hold_frames = 3\n",
        "strip_bones = [\"<fbx-root>\"]\n",
    )
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
            "character.assembly.json",
        ])
        .output()
        .expect("runs animsmith assemble")
}

#[test]
fn assembles_schema_valid_byte_stable_character_and_evidence() {
    let dir = tempfile::tempdir().expect("creates temp directory");
    write_inputs(dir.path());
    std::fs::write(dir.path().join("recipe.toml"), success_recipe()).expect("writes recipe");
    std::fs::write(dir.path().join("animsmith.toml"), b"").expect("writes config");

    let first = run(dir.path());
    assert!(
        first.status.success(),
        "assemble failed: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert_eq!(
        String::from_utf8(first.stdout).unwrap(),
        "wrote character.glb and character.assembly.json (1 clip(s), 1 mesh(es), 0 material(s))\n"
    );
    let first_glb = std::fs::read(dir.path().join("character.glb")).expect("reads GLB");
    let first_evidence =
        std::fs::read(dir.path().join("character.assembly.json")).expect("reads evidence");
    let evidence: Value = serde_json::from_slice(&first_evidence).expect("evidence JSON");
    let schema: Value = serde_json::from_str(EVIDENCE_SCHEMA).expect("schema JSON");
    let validator = jsonschema::validator_for(&schema).expect("schema compiles");
    let errors = validator
        .iter_errors(&evidence)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(errors.is_empty(), "schema errors: {errors:#?}");
    assert_eq!(evidence["schema_version"], 1);
    assert_eq!(
        evidence["schema"],
        "urn:animsmith:schema:character-assembly-evidence:1"
    );
    assert_eq!(evidence["recipe"]["effective"]["input_root"], "inputs");
    assert_eq!(evidence["config"]["source"], "file");
    assert_eq!(evidence["config"]["path"], "animsmith.toml");
    assert_eq!(
        evidence["config"]["sha256"],
        format!("{:x}", Sha256::digest(b""))
    );
    assert_eq!(evidence["config"]["bytes"], 0);
    assert_eq!(evidence["clips"][0]["source_tracks"], 3);
    assert_eq!(evidence["clips"][0]["remapped_tracks"], 3);
    assert_eq!(evidence["clips"][0]["dropped_closing_endpoint"], true);
    assert_eq!(evidence["clips"][0]["hold_frames"], 3);
    assert_eq!(evidence["transforms"]["removed_mesh_instances"], 0);
    assert_eq!(evidence["transforms"]["canonicalized_skin"], true);
    assert_eq!(evidence["transforms"]["ground_and_center"], true);
    assert_eq!(
        evidence["artifact"]["sha256"],
        format!("{:x}", Sha256::digest(&first_glb))
    );

    let document =
        animsmith_gltf::load(&dir.path().join("character.glb")).expect("assembled GLB reloads");
    assert_eq!(document.clips.len(), 1);
    assert_eq!(document.clips[0].name, "motion");
    assert_eq!(document.assets.meshes.len(), 1);
    assert_eq!(document.assets.instances.len(), 1);
    assert_eq!(document.assets.instances[0].skin_joints.len(), 1);
    assert!(
        document
            .skeleton
            .bones
            .iter()
            .any(|bone| bone.name == "animsmith-canonical-root")
    );
    let root = document
        .skeleton
        .bones
        .iter()
        .position(|bone| bone.name == "<fbx-root>")
        .unwrap();
    assert!(
        document.clips[0]
            .tracks
            .iter()
            .all(|track| track.bone != root),
        "named root tracks were stripped after completion"
    );

    let second = run(dir.path());
    assert!(
        second.status.success(),
        "second assemble failed: {}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert_eq!(
        std::fs::read(dir.path().join("character.glb")).unwrap(),
        first_glb
    );
    assert_eq!(
        std::fs::read(dir.path().join("character.assembly.json")).unwrap(),
        first_evidence
    );
}

#[test]
fn invalid_recipe_leaves_no_partial_outputs() {
    let dir = tempfile::tempdir().expect("creates temp directory");
    write_inputs(dir.path());
    let recipe = concat!(
        "schema_version = 1\n",
        "schema = \"urn:animsmith:schema:character-assembly-recipe:1\"\n",
        "input_root = \"inputs\"\n",
        "base_input = \"base.fbx\"\n",
        "\n",
        "[[clips]]\n",
        "name = \"same\"\n",
        "input = \"motion.fbx\"\n",
        "take = \"take\"\n",
        "\n",
        "[[clips]]\n",
        "name = \"same\"\n",
        "input = \"motion.fbx\"\n",
        "take = \"take\"\n",
    );
    std::fs::write(dir.path().join("recipe.toml"), recipe).expect("writes recipe");

    let output = run(dir.path());
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("duplicate output clip name \"same\"")
    );
    assert!(!dir.path().join("character.glb").exists());
    assert!(!dir.path().join("character.assembly.json").exists());
}

#[test]
#[cfg(unix)]
fn linked_input_is_rejected_before_outputs_change() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().expect("creates temp directory");
    write_inputs(dir.path());
    std::fs::remove_file(dir.path().join("inputs/motion.fbx")).unwrap();
    symlink("base.fbx", dir.path().join("inputs/motion.fbx")).expect("creates linked input");
    std::fs::write(dir.path().join("recipe.toml"), success_recipe()).expect("writes recipe");
    std::fs::write(dir.path().join("character.glb"), b"old artifact").unwrap();
    std::fs::write(dir.path().join("character.assembly.json"), b"old evidence").unwrap();

    let output = run(dir.path());
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("traverses a symbolic link"));
    assert_eq!(
        std::fs::read(dir.path().join("character.glb")).unwrap(),
        b"old artifact"
    );
    assert_eq!(
        std::fs::read(dir.path().join("character.assembly.json")).unwrap(),
        b"old evidence"
    );
}
