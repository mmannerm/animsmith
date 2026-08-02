//! End-to-end contract for versioned multi-source character assembly.

#![cfg(feature = "fbx")]

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::Path;
use std::process::{Command, Output};

const RIGGED_TRIANGLE_FBX: &str = include_str!("../../animsmith-fbx/testdata/rigged_triangle.fbx");
const RECIPE_SCHEMA: &str =
    include_str!("../../../docs/schemas/character-assembly-recipe-v1.schema.json");
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

fn assert_schema_valid(instance: &Value, schema_text: &str) {
    let schema: Value = serde_json::from_str(schema_text).expect("schema JSON");
    let validator = jsonschema::validator_for(&schema).expect("schema compiles");
    let errors = validator
        .iter_errors(instance)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(errors.is_empty(), "schema errors: {errors:#?}");
}

#[test]
fn assembles_schema_valid_byte_stable_character_and_evidence() {
    let dir = tempfile::tempdir().expect("creates temp directory");
    write_inputs(dir.path());
    std::fs::write(dir.path().join("recipe.toml"), success_recipe()).expect("writes recipe");
    std::fs::write(dir.path().join("animsmith.toml"), b"").expect("writes config");
    let parsed_recipe: toml::Value = toml::from_str(success_recipe()).expect("recipe TOML");
    assert_schema_valid(
        &serde_json::to_value(parsed_recipe).expect("recipe JSON value"),
        RECIPE_SCHEMA,
    );

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
    assert_schema_valid(&evidence, EVIDENCE_SCHEMA);
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
    assert!(
        evidence["clips"][0]["bone_remaps"]
            .as_array()
            .is_some_and(|remaps| !remaps.is_empty())
    );
    assert!(
        evidence["clips"][0]["bone_remaps"]
            .as_array()
            .unwrap()
            .iter()
            .all(|remap| remap["source_bone"] == remap["base_bone"])
    );
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
    assert!(
        (document.clips[0].duration_s - 32.0 / 30.0).abs() < 1.0e-5,
        "frame window, endpoint removal, and three-frame hold determine duration: {}",
        document.clips[0].duration_s
    );
    let held_tracks = document.clips[0]
        .tracks
        .iter()
        .filter(|track| track.key_count() > 1)
        .collect::<Vec<_>>();
    assert!(
        held_tracks
            .iter()
            .any(
                |track| (f64::from(track.end_time()) - document.clips[0].duration_s).abs() < 1.0e-5
            ),
        "the longest authored channel determines the held clip duration"
    );
    for track in held_tracks {
        let last = track.key_count() - 1;
        match track.property {
            animsmith_core::model::Property::Rotation => {
                assert_eq!(track.key_quat(last), track.key_quat(last - 1));
            }
            _ => assert_eq!(track.key_vec3(last), track.key_vec3(last - 1)),
        }
    }
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
fn rejects_unsupported_malformed_escaping_and_missing_input_selections() {
    let cases = [
        (
            success_recipe().replacen("schema_version = 1", "schema_version = 2", 1),
            "unsupported assembly recipe identity",
        ),
        (
            success_recipe().replacen(
                "base_input = \"base.fbx\"",
                "base_input = \"../outside.fbx\"",
                1,
            ),
            "must not contain a parent or root component",
        ),
        (
            success_recipe().replacen("take = \"take\"", "take = \"missing\"", 1),
            "has no take \"missing\"",
        ),
        (
            success_recipe().replacen("frame_window = [1, 31]", "frame_window = [31, 1]", 1),
            "frame_window must be one-based and increasing",
        ),
        (
            success_recipe().replacen("fps = 30.0", "fps = 30.0\nunknown_field = true", 1),
            "unknown field `unknown_field`",
        ),
    ];
    for (ordinal, (recipe, expected)) in cases.into_iter().enumerate() {
        let dir = tempfile::tempdir().expect("creates temp directory");
        write_inputs(dir.path());
        std::fs::write(dir.path().join("recipe.toml"), recipe).expect("writes invalid recipe");
        let output = run(dir.path());
        assert_eq!(output.status.code(), Some(2), "case {ordinal}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected),
            "case {ordinal}: expected {expected:?}, got {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!dir.path().join("character.glb").exists());
        assert!(!dir.path().join("character.assembly.json").exists());
    }

    let schema: Value = serde_json::from_str(RECIPE_SCHEMA).expect("recipe schema JSON");
    let validator = jsonschema::validator_for(&schema).expect("recipe schema compiles");
    for invalid in [
        success_recipe().replacen("schema_version = 1", "schema_version = 2", 1),
        success_recipe().replacen("fps = 30.0", "fps = 30.0\nunknown_field = true", 1),
        success_recipe().replacen(
            "frame_window = [1, 31]",
            "frame_window = [1, 31]\ntime_window = [0.0, 1.0]",
            1,
        ),
    ] {
        let parsed: toml::Value = toml::from_str(&invalid).expect("invalid-contract recipe TOML");
        let json = serde_json::to_value(parsed).expect("recipe JSON value");
        assert!(!validator.is_valid(&json), "schema accepted {invalid}");
    }
}

#[test]
fn accepts_time_window_and_gltf_clip_input_while_pruning_unselected_assets() {
    use animsmith_core::model::{Bone, MaterialAsset, Transform};

    let dir = tempfile::tempdir().expect("creates temp directory");
    write_inputs(dir.path());

    let mut base =
        animsmith_fbx::load(&dir.path().join("inputs/base.fbx")).expect("loads base FBX");
    base.clips.clear();
    let base_root = base
        .skeleton
        .bones
        .iter()
        .position(|bone| bone.name == "root")
        .expect("base root");
    let expected_base_root_rest = base.skeleton.bones[base_root].rest;
    base.assets.materials = vec![
        MaterialAsset {
            name: "body-material".into(),
            base_color: [1.0; 4],
            metallic: 0.0,
            roughness: 1.0,
            base_color_texture: None,
            normal_texture: None,
            metallic_roughness_texture: None,
            occlusion_texture: None,
        },
        MaterialAsset {
            name: "prop-material".into(),
            base_color: [1.0; 4],
            metallic: 0.0,
            roughness: 1.0,
            base_color_texture: None,
            normal_texture: None,
            metallic_roughness_texture: None,
            occlusion_texture: None,
        },
    ];
    for primitive in &mut base.assets.meshes[0].primitives {
        primitive.material = Some(0);
    }
    let mut prop_mesh = base.assets.meshes[0].clone();
    prop_mesh.name = "prop-mesh".into();
    for primitive in &mut prop_mesh.primitives {
        primitive.material = Some(1);
    }
    base.assets.meshes.push(prop_mesh);
    let prop_node = base.skeleton.bones.len();
    base.skeleton.bones.push(Bone {
        name: "prop-node".into(),
        parent: None,
        rest: Transform::IDENTITY,
        inverse_bind: None,
    });
    let mut prop_instance = base.assets.instances[0].clone();
    prop_instance.source_node_index += 1;
    prop_instance.node = prop_node;
    prop_instance.mesh = 1;
    base.assets.instances.push(prop_instance);
    animsmith_gltf::write::write(&base, &dir.path().join("inputs/base.glb"))
        .expect("writes multi-instance base GLB");

    let mut motion =
        animsmith_fbx::load(&dir.path().join("inputs/motion.fbx")).expect("loads motion FBX");
    let motion_root = motion
        .skeleton
        .bones
        .iter()
        .position(|bone| bone.name == "root")
        .expect("motion root");
    motion.skeleton.bones[motion_root].rest.translation += animsmith_core::glam::Vec3::splat(10.0);
    animsmith_gltf::write::write(&motion, &dir.path().join("inputs/motion.glb"))
        .expect("writes motion GLB");
    let recipe = success_recipe()
        .replacen("base_input = \"base.fbx\"", "base_input = \"base.glb\"", 1)
        .replacen(
            "mesh_instances = [\"tri\"]",
            "mesh_instances = [\"tri_skinned\"]",
            1,
        )
        .replacen("input = \"motion.fbx\"", "input = \"motion.glb\"", 1)
        .replacen("frame_window = [1, 31]", "time_window = [0.2, 0.8]", 1)
        .replacen("drop_closing_endpoint = true\n", "", 1)
        .replacen("hold_frames = 3\n", "", 1)
        .replacen("canonicalize_skin = true", "canonicalize_skin = false", 1)
        .replacen("ground_and_center = true\n", "", 1);
    std::fs::write(dir.path().join("recipe.toml"), recipe).expect("writes recipe");

    let output = run(dir.path());
    assert!(
        output.status.success(),
        "assemble failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let assembled =
        animsmith_gltf::load(&dir.path().join("character.glb")).expect("reloads assembled GLB");
    assert_eq!(assembled.assets.meshes.len(), 1);
    assert_eq!(assembled.assets.instances.len(), 1);
    assert_eq!(assembled.assets.materials.len(), 1);
    assert_eq!(assembled.assets.materials[0].name, "body-material");
    let assembled_root = assembled
        .skeleton
        .bones
        .iter()
        .position(|bone| bone.name == "root")
        .expect("assembled root");
    assert_eq!(
        assembled.skeleton.bones[assembled_root].rest, expected_base_root_rest,
        "base rest pose remains authoritative over the clip input"
    );
    assert!((assembled.clips[0].duration_s - 0.6).abs() < 1.0e-5);
    let evidence: Value = serde_json::from_slice(
        &std::fs::read(dir.path().join("character.assembly.json")).expect("reads evidence"),
    )
    .expect("evidence JSON");
    assert_eq!(
        evidence["transforms"]["retained_mesh_instances"],
        serde_json::json!(["tri_skinned"])
    );
    assert_eq!(evidence["transforms"]["removed_mesh_instances"], 1);
    assert_eq!(evidence["clips"][0]["declared_input"], "motion.glb");
    assert_eq!(
        evidence["clips"][0]["time_window"],
        serde_json::json!([0.2, 0.8])
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
