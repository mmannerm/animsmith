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

fn run_args(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .current_dir(dir)
        .arg("assemble")
        .args(args)
        .output()
        .expect("runs animsmith assemble")
}

fn run(dir: &Path) -> Output {
    run_args(
        dir,
        &[
            "recipe.toml",
            "-o",
            "character.glb",
            "--evidence",
            "character.assembly.json",
        ],
    )
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

/// `--format json` puts the published evidence on stdout — the same bytes,
/// not a second rendering of the same record.
///
/// The byte equality guards **drift**; it does not prove the record is
/// serialized once. A second serializer producing identical bytes would pass
/// this unchanged — which is precisely why the property lives in the
/// construction instead: `publish::serialize_record` runs once and both the
/// evidence temp and `publish::emit` receive that one `Vec<u8>`. This test is
/// what notices if a later change lets the two destinations diverge.
#[test]
fn json_format_prints_the_published_evidence_bytes_verbatim() {
    let dir = tempfile::tempdir().expect("creates temp directory");
    write_inputs(dir.path());
    std::fs::write(dir.path().join("recipe.toml"), success_recipe()).expect("writes recipe");

    let output = run_args(
        dir.path(),
        &[
            "recipe.toml",
            "-o",
            "character.glb",
            "--evidence",
            "character.assembly.json",
            "--format",
            "json",
        ],
    );
    assert!(
        output.status.success(),
        "assemble failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let evidence =
        std::fs::read(dir.path().join("character.assembly.json")).expect("reads evidence");
    assert_eq!(
        output.stdout, evidence,
        "stdout must be the bytes the evidence file received"
    );

    let record: Value = serde_json::from_slice(&output.stdout).expect("stdout is one JSON record");
    assert_eq!(
        record["schema"], "urn:animsmith:schema:character-assembly-evidence:1",
        "no second schema and no envelope around the record"
    );
    assert_schema_valid(&record, EVIDENCE_SCHEMA);
}

/// The format *selects* the rendering rather than adding to it, as `convert`
/// and `scale` already do: stdout parses whole as one JSON document, so no
/// summary line survives beside it.
#[test]
fn json_format_emits_no_text_summary() {
    let dir = tempfile::tempdir().expect("creates temp directory");
    write_inputs(dir.path());
    std::fs::write(dir.path().join("recipe.toml"), success_recipe()).expect("writes recipe");

    let output = run_args(
        dir.path(),
        &[
            "recipe.toml",
            "-o",
            "character.glb",
            "--evidence",
            "character.assembly.json",
            "--format",
            "json",
        ],
    );
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout is UTF-8");
    assert!(
        !stdout.contains("wrote "),
        "the text summary must not survive --format json: {stdout}"
    );
    // A trailing or leading summary line would make this a parse failure
    // rather than one complete document.
    serde_json::from_str::<Value>(&stdout).expect("stdout is exactly one JSON document");
}

/// The publication summary escapes its declared paths, because it now goes
/// through `render.rs` beside every other command summary rather than being
/// an inline `println!` of a raw `Path::display`.
///
/// This is the one place the move is deliberately *not* byte-identical: a
/// path carrying an ESC used to reach the terminal able to run it.
#[test]
#[cfg(unix)]
fn the_text_summary_escapes_a_control_character_in_a_declared_path() {
    let dir = tempfile::tempdir().expect("creates temp directory");
    write_inputs(dir.path());
    std::fs::write(dir.path().join("recipe.toml"), success_recipe()).expect("writes recipe");

    let output = run_args(
        dir.path(),
        &[
            "recipe.toml",
            "-o",
            "char\u{1b}[31m.glb",
            "--evidence",
            "char\u{1b}[31m.json",
        ],
    );
    assert!(
        output.status.success(),
        "assemble failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout is UTF-8"),
        "wrote char\\u{1b}[31m.glb and char\\u{1b}[31m.json (1 clip(s), 1 mesh(es), 0 material(s))\n"
    );
    // The escaping is presentation only: the operator's real paths are
    // untouched, and the evidence keeps them verbatim.
    assert!(dir.path().join("char\u{1b}[31m.glb").is_file());
    let evidence: Value = serde_json::from_slice(
        &std::fs::read(dir.path().join("char\u{1b}[31m.json")).expect("reads evidence"),
    )
    .expect("evidence JSON");
    assert_eq!(evidence["artifact"]["path"], "char\u{1b}[31m.glb");
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
            success_recipe().replacen(
                "fps = 30.0",
                "fps = 30.0\nmaterial_texture_recipe = \"../outside.toml\"",
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
    let selected_name = "selected\u{1b}\u{202e}mesh_skinned";
    base.assets.meshes[0].name = "selected\u{1b}\u{202e}mesh".into();
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
    let inspected = Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .current_dir(dir.path())
        .args(["inspect", "inputs/base.glb"])
        .output()
        .expect("inspects assembly base");
    assert!(inspected.status.success());
    let inspect_text = String::from_utf8(inspected.stdout).expect("inspect output is UTF-8");
    let discovered_selector = inspect_text
        .lines()
        .find_map(|line| line.strip_prefix("  node "))
        .expect("inspect exposes a quoted mesh-instance name")
        .to_owned();
    assert_eq!(
        discovered_selector,
        "\"selected\\u001B\\u202Emesh_skinned\""
    );
    let parsed_selector: toml::Value =
        toml::from_str(&format!("mesh_instances = [{discovered_selector}]"))
            .expect("copied inspect selector is valid TOML");
    assert_eq!(
        parsed_selector["mesh_instances"][0].as_str(),
        Some(selected_name)
    );

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
            &format!("mesh_instances = [{discovered_selector}]"),
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
        serde_json::json!([selected_name]),
        "the exact name copied from inspect is accepted and retained"
    );
    assert_eq!(evidence["transforms"]["removed_mesh_instances"], 1);
    assert_eq!(evidence["clips"][0]["declared_input"], "motion.glb");
    assert_eq!(
        evidence["clips"][0]["time_window"],
        serde_json::json!([0.2, 0.8])
    );
}

#[test]
fn duplicate_inspected_mesh_instance_name_fails_closed_without_publication() {
    for duplicate_count in [2, 3, 4] {
        let dir = tempfile::tempdir().expect("creates temp directory");
        write_inputs(dir.path());

        let mut base =
            animsmith_fbx::load(&dir.path().join("inputs/base.fbx")).expect("loads base FBX");
        let original = base.assets.instances[0].clone();
        for ordinal in 1..duplicate_count {
            let mut duplicate = original.clone();
            duplicate.source_node_index += ordinal;
            base.assets.instances.push(duplicate);
        }
        animsmith_gltf::write::write(&base, &dir.path().join("inputs/base.glb"))
            .expect("writes ambiguous base GLB");

        let inspected = Command::new(env!("CARGO_BIN_EXE_animsmith"))
            .current_dir(dir.path())
            .args(["inspect", "inputs/base.glb"])
            .output()
            .expect("inspects ambiguous base");
        assert!(inspected.status.success());
        let inspect_text = String::from_utf8_lossy(&inspected.stdout);
        let marker = format!(
            "  node \"tri_skinned\" [ambiguous: {duplicate_count} skeleton nodes share this name]\n"
        );
        assert_eq!(
            inspect_text.matches(&marker).count(),
            duplicate_count,
            "inspect must show every ambiguous instance: {inspect_text}"
        );

        let recipe = success_recipe()
            .replacen("base_input = \"base.fbx\"", "base_input = \"base.glb\"", 1)
            .replacen(
                "mesh_instances = [\"tri\"]",
                "mesh_instances = [\"tri_skinned\"]",
                1,
            );
        std::fs::write(dir.path().join("recipe.toml"), recipe).expect("writes ambiguous recipe");

        let output = run(dir.path());
        assert_eq!(output.status.code(), Some(2));
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("base input contains ambiguous duplicate bone name \"tri_skinned\""),
            "unexpected error: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!dir.path().join("character.glb").exists());
        assert!(!dir.path().join("character.assembly.json").exists());
    }
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

    for (linked, target) in [("base.fbx", "motion.fbx"), ("motion.fbx", "base.fbx")] {
        let dir = tempfile::tempdir().expect("creates temp directory");
        write_inputs(dir.path());
        std::fs::remove_file(dir.path().join("inputs").join(linked)).unwrap();
        symlink(target, dir.path().join("inputs").join(linked)).expect("creates linked input");
        std::fs::write(dir.path().join("recipe.toml"), success_recipe()).expect("writes recipe");
        std::fs::write(dir.path().join("character.glb"), b"old artifact").unwrap();
        std::fs::write(dir.path().join("character.assembly.json"), b"old evidence").unwrap();

        let output = run(dir.path());
        assert_eq!(output.status.code(), Some(2), "linked {linked}");
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
}

#[test]
fn assembles_synthetic_skinned_recipe_with_complete_public_provenance() {
    use animsmith_core::glam::{Quat, Vec3};
    use animsmith_core::model::{
        Bone, Clip, Document, Interpolation, MaterialAsset, MeshAsset, MeshInstance, Primitive,
        Property, SceneAssets, Skeleton, Track, TrackValues, Transform,
    };
    use image::codecs::png::{CompressionType, FilterType, PngEncoder};
    use image::{ExtendedColorType, ImageEncoder};
    use std::f64::consts::TAU;

    const FPS: f32 = 32.0;
    const KEYS: usize = 32;

    fn track_vec3(bone: usize, property: Property, values: Vec<Vec3>) -> Track {
        Track {
            bone,
            property,
            interpolation: Interpolation::Linear,
            times: (0..KEYS).map(|key| key as f32 / FPS).collect(),
            values: TrackValues::Vec3s(values),
        }
    }

    fn foot_track(bone: usize, rest: Vec3, sign: f32) -> Track {
        track_vec3(
            bone,
            Property::Translation,
            (0..KEYS)
                .map(|key| {
                    let theta = (TAU * key as f64 / KEYS as f64) as f32;
                    rest + Vec3::new(0.0, sign * 0.06 * theta.sin(), 0.0)
                })
                .collect(),
        )
    }

    fn png(pixel: [u8; 4]) -> Vec<u8> {
        let mut bytes = Vec::new();
        PngEncoder::new_with_quality(&mut bytes, CompressionType::Best, FilterType::NoFilter)
            .write_image(&pixel, 1, 1, ExtendedColorType::Rgba8)
            .expect("encodes fixture PNG");
        bytes
    }

    let dir = tempfile::tempdir().expect("creates temp directory");
    let inputs = dir.path().join("inputs");
    std::fs::create_dir(&inputs).expect("creates input root");
    std::fs::create_dir(inputs.join("textures")).expect("creates texture root");

    // The base has the authoritative rest pose.  The source clip below uses
    // the same exact names in a different order and intentionally different
    // rest transforms, so any accidental source-rest import is observable.
    let base_skeleton = Skeleton {
        bones: vec![
            Bone {
                name: "hips".into(),
                parent: None,
                rest: Transform {
                    translation: Vec3::new(4.0, 3.0, -2.0),
                    rotation: Quat::from_rotation_y(0.2),
                    scale: Vec3::ONE,
                },
                inverse_bind: None,
            },
            Bone {
                name: "left_foot".into(),
                parent: Some(0),
                rest: Transform {
                    translation: Vec3::new(0.45, -1.1, 0.0),
                    ..Transform::IDENTITY
                },
                inverse_bind: None,
            },
            Bone {
                name: "right_foot".into(),
                parent: Some(0),
                rest: Transform {
                    translation: Vec3::new(-0.45, -1.1, 0.0),
                    ..Transform::IDENTITY
                },
                inverse_bind: None,
            },
            Bone {
                name: "motion_root".into(),
                parent: None,
                rest: Transform::IDENTITY,
                inverse_bind: None,
            },
            Bone {
                name: "body_node".into(),
                parent: None,
                rest: Transform::IDENTITY,
                inverse_bind: None,
            },
            Bone {
                name: "prop_node".into(),
                parent: None,
                rest: Transform::IDENTITY,
                inverse_bind: None,
            },
        ],
    };
    let base_worlds = {
        let mut worlds = Vec::new();
        for bone in &base_skeleton.bones {
            worlds.push(match bone.parent {
                Some(parent) => worlds[parent] * bone.rest.to_mat4(),
                None => bone.rest.to_mat4(),
            });
        }
        worlds
    };
    let geometry_world = base_worlds[0];
    let skin_ibms = [0, 1, 2]
        .into_iter()
        .map(|joint| base_worlds[joint].inverse() * geometry_world)
        .collect();
    let body_primitive = Primitive {
        material: Some(0),
        indices: vec![0, 1, 2],
        positions: vec![
            Vec3::new(-1.0, -1.0, -0.5),
            Vec3::new(1.0, -1.0, -0.5),
            Vec3::new(0.0, 1.0, 0.5),
        ],
        normals: vec![Vec3::Y; 3],
        uvs: vec![[0.0, 0.0], [1.0, 0.0], [0.5, 1.0]],
        joints: vec![[0, 1, 2, 0]; 3],
        weights: vec![[0.5, 0.25, 0.25, 0.0]; 3],
        additional_influence_sets: Vec::new(),
    };
    let base = Document {
        skeleton: base_skeleton,
        clips: vec![],
        assets: SceneAssets {
            meshes: vec![
                MeshAsset {
                    name: "body_mesh".into(),
                    source_mesh_index: 0,
                    primitives: vec![body_primitive],
                },
                MeshAsset {
                    name: "removable_prop".into(),
                    source_mesh_index: 1,
                    primitives: vec![Primitive {
                        material: Some(1),
                        indices: vec![0, 1, 2],
                        positions: vec![Vec3::ZERO, Vec3::X, Vec3::Y],
                        normals: vec![Vec3::Z; 3],
                        ..Primitive::default()
                    }],
                },
            ],
            instances: vec![
                MeshInstance {
                    source_node_index: 10,
                    node: 4,
                    mesh: 0,
                    skin_joints: vec![0, 1, 2],
                    skin_ibms,
                },
                MeshInstance {
                    source_node_index: 11,
                    node: 5,
                    mesh: 1,
                    ..MeshInstance::default()
                },
            ],
            materials: vec![
                MaterialAsset {
                    name: "body_finish".into(),
                    base_color: [0.9, 0.8, 0.7, 1.0],
                    metallic: 0.2,
                    roughness: 0.6,
                    base_color_texture: None,
                    normal_texture: None,
                    metallic_roughness_texture: None,
                    occlusion_texture: None,
                },
                MaterialAsset {
                    name: "prop_finish".into(),
                    base_color: [0.1, 0.2, 0.3, 1.0],
                    metallic: 0.0,
                    roughness: 1.0,
                    base_color_texture: None,
                    normal_texture: None,
                    metallic_roughness_texture: None,
                    occlusion_texture: None,
                },
            ],
            ..SceneAssets::default()
        },
        ..Document::default()
    };
    animsmith_gltf::write::write(&base, &inputs.join("base.glb")).expect("writes base GLB");

    let source_skeleton = Skeleton {
        bones: vec![
            Bone {
                name: "hips".into(),
                parent: None,
                rest: Transform {
                    translation: Vec3::new(-8.0, 9.0, 6.0),
                    rotation: Quat::from_rotation_x(-0.4),
                    scale: Vec3::splat(1.5),
                },
                inverse_bind: None,
            },
            Bone {
                name: "right_foot".into(),
                parent: Some(0),
                rest: Transform {
                    translation: Vec3::new(-3.0, -2.0, 1.0),
                    ..Transform::IDENTITY
                },
                inverse_bind: None,
            },
            Bone {
                name: "left_foot".into(),
                parent: Some(0),
                rest: Transform {
                    translation: Vec3::new(3.0, -2.0, 1.0),
                    ..Transform::IDENTITY
                },
                inverse_bind: None,
            },
            Bone {
                name: "motion_root".into(),
                parent: None,
                rest: Transform::IDENTITY,
                inverse_bind: None,
            },
        ],
    };
    let rotation_a = Quat::from_rotation_y(0.35);
    let rotation_b = -Quat::from_rotation_y(0.45);
    let selected = Clip {
        name: "selected_take".into(),
        duration_s: f64::from((KEYS - 1) as f32 / FPS),
        tracks: vec![
            track_vec3(
                3,
                Property::Translation,
                (0..KEYS)
                    .map(|key| Vec3::new(key as f32 * 0.1, 0.0, 0.0))
                    .collect(),
            ),
            Track {
                bone: 0,
                property: Property::Rotation,
                interpolation: Interpolation::Linear,
                times: vec![0.0, (KEYS - 1) as f32 / FPS],
                values: TrackValues::Quats(vec![
                    Quat::from_xyzw(
                        rotation_a.x * 2.0,
                        rotation_a.y * 2.0,
                        rotation_a.z * 2.0,
                        rotation_a.w * 2.0,
                    ),
                    Quat::from_xyzw(
                        rotation_b.x * 3.0,
                        rotation_b.y * 3.0,
                        rotation_b.z * 3.0,
                        rotation_b.w * 3.0,
                    ),
                ]),
            },
            foot_track(1, source_skeleton.bones[1].rest.translation, -1.0),
            foot_track(2, source_skeleton.bones[2].rest.translation, 1.0),
        ],
    };
    let source = Document {
        skeleton: source_skeleton,
        clips: vec![
            Clip {
                name: "unselected_take".into(),
                duration_s: 0.0,
                tracks: vec![],
            },
            selected,
        ],
        ..Document::default()
    };
    animsmith_gltf::write::write(&source, &inputs.join("clips.glb"))
        .expect("writes clip source GLB");

    let texture_inputs = [
        ("base.png", [230, 30, 20, 255]),
        ("normal.png", [128, 128, 255, 255]),
        ("metallic-roughness.png", [0, 190, 70, 255]),
        ("occlusion.png", [50, 0, 0, 255]),
    ];
    for (name, pixel) in texture_inputs {
        std::fs::write(inputs.join("textures").join(name), png(pixel))
            .expect("writes distinct texture");
    }
    std::fs::write(
        inputs.join("materials.toml"),
        concat!(
            "schema_version = 1\n",
            "schema = \"urn:animsmith:schema:material-texture-recipe:1\"\n",
            "texture_root = \"textures\"\n",
            "max_dimension = 1\n\n",
            "[[materials]]\n",
            "name = \"body_finish\"\n",
            "base_color = \"base.png\"\n",
            "normal = \"normal.png\"\n",
            "metallic_roughness = \"metallic-roughness.png\"\n",
            "occlusion = \"occlusion.png\"\n",
        ),
    )
    .expect("writes material recipe");
    std::fs::write(
        dir.path().join("animsmith.toml"),
        concat!(
            "[rig]\nprofile = \"auto\"\n\n",
            "[rig.roles]\n",
            "hips = \"hips\"\n",
            "left_foot = \"left_foot\"\n",
            "right_foot = \"right_foot\"\n",
        ),
    )
    .expect("writes role config");
    let recipe = concat!(
        "schema_version = 1\n",
        "schema = \"urn:animsmith:schema:character-assembly-recipe:1\"\n",
        "input_root = \"inputs\"\n",
        "base_input = \"base.glb\"\n",
        "mesh_instances = [\"body_mesh_skinned\"]\n",
        "material_texture_recipe = \"materials.toml\"\n",
        "complete_tracks = true\n",
        "canonicalize_skin = true\n",
        "ground_and_center = true\n",
        "fps = 32.0\n\n",
        "[[clips]]\n",
        "name = \"assembled_cycle\"\n",
        "input = \"clips.glb\"\n",
        "take = \"selected_take\"\n",
        "gait_anchor = true\n",
        "strip_bones = [\"motion_root\"]\n",
    );
    std::fs::write(dir.path().join("recipe.toml"), recipe).expect("writes assembly recipe");

    let first = run(dir.path());
    assert!(
        first.status.success(),
        "assemble failed: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    let first_glb = std::fs::read(dir.path().join("character.glb")).expect("reads artifact");
    let first_evidence =
        std::fs::read(dir.path().join("character.assembly.json")).expect("reads evidence");
    let evidence: Value = serde_json::from_slice(&first_evidence).expect("parses evidence");
    assert_schema_valid(&evidence, EVIDENCE_SCHEMA);
    assert_eq!(evidence["tool"]["name"], "animsmith");
    assert_eq!(evidence["command"], "assemble");
    assert_eq!(evidence["recipe"]["effective"]["base_input"], "base.glb");
    assert_eq!(evidence["config"]["source"], "file");
    assert_eq!(
        evidence["transforms"]["retained_mesh_instances"],
        serde_json::json!(["body_mesh_skinned"])
    );
    assert_eq!(evidence["transforms"]["removed_mesh_instances"], 1);
    assert_eq!(evidence["transforms"]["canonicalized_skin"], true);
    assert_eq!(evidence["transforms"]["ground_and_center"], true);
    assert!(evidence["transforms"]["source_world_to_canonical"].is_array());
    assert!(evidence["transforms"]["converted_bounds_min"].is_array());
    assert!(evidence["transforms"]["converted_bounds_max"].is_array());
    assert_eq!(evidence["clips"][0]["name"], "assembled_cycle");
    assert_eq!(evidence["clips"][0]["source_take"], "selected_take");
    assert_eq!(evidence["clips"][0]["declared_input"], "clips.glb");
    assert!(
        evidence["clips"][0]["gait_anchor_frame_offset"]
            .as_i64()
            .is_some()
    );
    assert_eq!(evidence["clips"][0]["stripped_tracks"], 1);
    assert_eq!(
        evidence["clips"][0]["stripped_bone_motion"][0]["bone"],
        "motion_root"
    );
    assert!(
        (evidence["clips"][0]["stripped_bone_motion"][0]["translation_delta"][0]
            .as_f64()
            .expect("root translation delta")
            - f64::from((KEYS - 1) as f32 * 0.1))
        .abs()
            < 1.0e-5
    );
    let remaps = evidence["clips"][0]["bone_remaps"]
        .as_array()
        .expect("remap evidence array");
    assert!(remaps.iter().any(|remap| {
        remap["source_bone"] == "right_foot"
            && remap["base_bone"] == "right_foot"
            && remap["source_index"] != remap["base_index"]
    }));
    let roles = evidence["inputs"]
        .as_array()
        .expect("input evidence array")
        .iter()
        .map(|input| input["role"].as_str().expect("input role"))
        .collect::<Vec<_>>();
    assert_eq!(
        roles,
        [
            "base",
            "material_texture_recipe",
            "texture",
            "texture",
            "texture",
            "texture",
            "clip"
        ]
    );
    for input in evidence["inputs"].as_array().unwrap() {
        assert_eq!(input["sha256"].as_str().map(str::len), Some(64));
        assert!(input["bytes"].as_u64().is_some());
    }
    let texture_evidence = evidence["material_texture_recipe"]["consumed_inputs"]
        .as_array()
        .expect("texture provenance");
    assert_eq!(texture_evidence.len(), 4);
    assert_eq!(
        texture_evidence
            .iter()
            .map(|entry| entry["declared_path"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "base.png",
            "normal.png",
            "metallic-roughness.png",
            "occlusion.png"
        ]
    );
    assert_eq!(
        evidence["artifact"]["sha256"],
        format!("{:x}", Sha256::digest(&first_glb))
    );
    assert_eq!(evidence["artifact"]["animations"], 1);
    assert_eq!(evidence["artifact"]["meshes"], 1);
    assert_eq!(evidence["artifact"]["materials"], 1);

    let assembled =
        animsmith_gltf::load(&dir.path().join("character.glb")).expect("reloads artifact");
    assert_eq!(assembled.clips.len(), 1);
    assert_eq!(assembled.clips[0].name, "assembled_cycle");
    assert!((assembled.clips[0].duration_s - f64::from((KEYS - 1) as f32 / FPS)).abs() < 1.0e-6);
    assert_eq!(assembled.assets.meshes.len(), 1);
    assert_eq!(assembled.assets.instances.len(), 1);
    assert_eq!(assembled.assets.materials.len(), 1);
    let material = &assembled.assets.materials[0];
    assert_eq!(material.name, "body_finish");
    assert!(material.base_color_texture.is_some());
    assert!(material.normal_texture.is_some());
    assert!(material.metallic_roughness_texture.is_some());
    assert!(material.occlusion_texture.is_some());
    assert_eq!(assembled.skeleton.bones[0].name, "animsmith-canonical-root");
    assert_eq!(assembled.skeleton.bones[0].rest, Transform::IDENTITY);
    let hips = assembled
        .skeleton
        .bones
        .iter()
        .position(|bone| bone.name == "hips")
        .expect("assembled hips");
    let left = assembled
        .skeleton
        .bones
        .iter()
        .position(|bone| bone.name == "left_foot")
        .expect("assembled left foot");
    let right = assembled
        .skeleton
        .bones
        .iter()
        .position(|bone| bone.name == "right_foot")
        .expect("assembled right foot");
    let stripped = assembled
        .skeleton
        .bones
        .iter()
        .position(|bone| bone.name == "motion_root")
        .expect("assembled motion root");
    assert!(
        assembled.clips[0]
            .tracks
            .iter()
            .all(|track| track.bone != stripped)
    );
    assert!(
        assembled.clips[0]
            .tracks
            .iter()
            .any(|track| track.bone == hips)
    );
    for bone in [hips, left, right] {
        for property in [Property::Translation, Property::Rotation, Property::Scale] {
            assert!(
                assembled.clips[0]
                    .tracks
                    .iter()
                    .any(|track| track.bone == bone && track.property == property)
            );
        }
    }
    for track in assembled.clips[0]
        .tracks
        .iter()
        .filter(|track| track.property == Property::Rotation)
    {
        let mut previous: Option<Quat> = None;
        for key in 0..track.key_count() {
            let quaternion = track.key_quat(key).expect("rotation key");
            assert!((quaternion.length() - 1.0).abs() < 1.0e-5);
            if let Some(previous) = previous {
                assert!(previous.dot(quaternion) >= -1.0e-6);
            }
            previous = Some(quaternion);
        }
    }
    let primitive = &assembled.assets.meshes[0].primitives[0];
    let min = primitive
        .positions
        .iter()
        .copied()
        .reduce(Vec3::min)
        .expect("body positions");
    let max = primitive
        .positions
        .iter()
        .copied()
        .reduce(Vec3::max)
        .expect("body positions");
    assert!(min.y.abs() < 1.0e-5, "grounded minimum: {min:?}");
    assert!(
        ((min.x + max.x) * 0.5).abs() < 1.0e-5,
        "centered x bounds: {min:?}..{max:?}"
    );
    assert!(
        ((min.z + max.z) * 0.5).abs() < 1.0e-5,
        "centered z bounds: {min:?}..{max:?}"
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
    let diff = Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .current_dir(dir.path())
        .args(["diff", "character.glb", "character.glb", "--format", "json"])
        .output()
        .expect("runs deterministic diff");
    assert!(
        diff.status.success(),
        "diff failed: {}",
        String::from_utf8_lossy(&diff.stderr)
    );
    let diff: Value = serde_json::from_slice(&diff.stdout).expect("diff JSON");
    assert_eq!(diff["summary"]["deltas"], 0);
    assert_eq!(diff["deltas"], serde_json::json!([]));

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        let linked_target = inputs.join("textures/base-target.png");
        std::fs::rename(inputs.join("textures/base.png"), &linked_target)
            .expect("moves texture behind a link");
        symlink("base-target.png", inputs.join("textures/base.png"))
            .expect("links texture inside its declared root");

        let failure = run(dir.path());
        assert_eq!(failure.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&failure.stderr).contains("traverses a symbolic link"));
        assert_eq!(
            std::fs::read(dir.path().join("character.glb")).unwrap(),
            first_glb
        );
        assert_eq!(
            std::fs::read(dir.path().join("character.assembly.json")).unwrap(),
            first_evidence
        );
    }
}
