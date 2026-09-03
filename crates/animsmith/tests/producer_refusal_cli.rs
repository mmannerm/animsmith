//! Public contract for typed `convert`/`assemble` asset refusals.

#![cfg(feature = "fbx")]

use animsmith_testkit::closed_stream::ClosedStream;
use serde_json::Value;
use std::path::Path;
use std::process::{Command, Output, Stdio};

const REFUSAL_SCHEMA: &str = include_str!("../../../docs/schemas/producer-refusal-v1.schema.json");
const RIGGED_TRIANGLE_FBX: &str = include_str!("../../animsmith-fbx/testdata/rigged_triangle.fbx");

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_animsmith"))
}

fn into_closed_stdout(dir: &Path, args: &[&str], close_stderr: bool) -> Output {
    let mut command = binary();
    command.current_dir(dir).args(args).closed_stdout();
    if close_stderr {
        command.stderr(Stdio::null());
    } else {
        command.stderr(Stdio::piped());
    }
    command.spawn().unwrap().wait_with_output().unwrap()
}

fn into_closed_stderr(dir: &Path, args: &[&str]) -> Output {
    binary()
        .current_dir(dir)
        .args(args)
        .stdout(Stdio::piped())
        .closed_stderr()
        .spawn()
        .unwrap()
        .wait_with_output()
        .unwrap()
}

fn assert_schema(value: &Value) {
    let schema: Value = serde_json::from_str(REFUSAL_SCHEMA).expect("schema JSON");
    let validator = jsonschema::validator_for(&schema).expect("schema compiles");
    let errors = validator
        .iter_errors(value)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(errors.is_empty(), "schema errors: {errors:#?}\n{value:#}");
}

fn assert_refusal(output: &Output, command: &str, kind: &str) -> Value {
    assert_eq!(
        output.status.code(),
        Some(1),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "JSON refusal is stdout-only");
    let record: Value = serde_json::from_slice(&output.stdout).expect("refusal JSON");
    assert_schema(&record);
    assert_eq!(record["schema_version"], 1);
    assert_eq!(record["schema"], "urn:animsmith:schema:producer-refusal:1");
    assert_eq!(record["command"], command);
    assert_eq!(record["outcome"], "rejected");
    assert_eq!(record["result"], Value::Null);
    assert_eq!(record["rejection"]["kind"], kind);
    record
}

fn write_assembly_inputs(dir: &Path) {
    std::fs::create_dir(dir.join("inputs")).unwrap();
    std::fs::write(dir.join("inputs/base.fbx"), RIGGED_TRIANGLE_FBX).unwrap();
    std::fs::write(dir.join("inputs/motion.fbx"), RIGGED_TRIANGLE_FBX).unwrap();
}

fn assembly_recipe_with_base(base_input: &str, take: &str) -> String {
    format!(
        "schema_version = 3\n\
         schema = \"urn:animsmith:schema:character-assembly-recipe:3\"\n\
         input_root = \"inputs\"\n\
         base_input = {base_input:?}\n\
         mesh_instances = [\"tri\"]\n\
         complete_tracks = true\n\
         canonicalize_skin = true\n\
         ground_and_center = true\n\
         fps = 30.0\n\
         [[clips]]\n\
         name = \"motion\"\n\
         input = \"motion.fbx\"\n\
         take = {take:?}\n\
         frame_window = [1, 31]\n\
         drop_closing_endpoint = true\n\
         hold_frames = 3\n\
         strip_bones = [\"<fbx-root>\"]\n"
    )
}

fn assembly_recipe(take: &str) -> String {
    assembly_recipe_with_base("base.fbx", take)
}

fn missing_external_buffer_gltf() -> &'static [u8] {
    br#"{
      "asset": {"version": "2.0"},
      "buffers": [{"uri": "missing.bin", "byteLength": 1}],
      "scenes": [{"nodes": []}],
      "scene": 0
    }"#
}

fn assemble(dir: &Path, format: &str) -> Output {
    binary()
        .current_dir(dir)
        .args([
            "assemble",
            "recipe.toml",
            "-o",
            "character.glb",
            "--evidence",
            "character.json",
            "--format",
            format,
        ])
        .output()
        .unwrap()
}

#[test]
fn json_consumers_distinguish_asset_refusals_from_operator_errors_without_prose() {
    let convert_dir = tempfile::tempdir().unwrap();
    std::fs::copy(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/assets/clip.glb"
        ),
        convert_dir.path().join("input.glb"),
    )
    .unwrap();
    let convert_refusal = binary()
        .current_dir(convert_dir.path())
        .args([
            "convert",
            "input.glb",
            "-o",
            "output.glb",
            "--bake-static-mesh-transforms",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    let record = assert_refusal(&convert_refusal, "convert", "transform-refused");
    assert_eq!(record["rejection"]["stage"], "transform");
    assert!(!convert_dir.path().join("output.glb").exists());

    let convert_operator = binary()
        .current_dir(convert_dir.path())
        .args([
            "convert",
            "missing.glb",
            "-o",
            "output.glb",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert_eq!(convert_operator.status.code(), Some(2));
    assert!(convert_operator.stdout.is_empty());
    assert!(!convert_operator.stderr.is_empty());

    let missing_recipe = binary()
        .current_dir(convert_dir.path())
        .args([
            "convert",
            "input.glb",
            "-o",
            "output.glb",
            "--material-texture-recipe",
            "missing.toml",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert_eq!(missing_recipe.status.code(), Some(2));
    assert!(missing_recipe.stdout.is_empty());
    assert!(!missing_recipe.stderr.is_empty());

    let assemble_dir = tempfile::tempdir().unwrap();
    write_assembly_inputs(assemble_dir.path());
    std::fs::write(
        assemble_dir.path().join("recipe.toml"),
        assembly_recipe("missing"),
    )
    .unwrap();
    let assembly_refusal = assemble(assemble_dir.path(), "json");
    let record = assert_refusal(&assembly_refusal, "assemble", "asset-recipe-mismatch");
    assert_eq!(record["rejection"]["stage"], "transform");
    assert!(!assemble_dir.path().join("character.glb").exists());
    assert!(!assemble_dir.path().join("character.json").exists());

    std::fs::write(assemble_dir.path().join("recipe.toml"), "not = [valid").unwrap();
    let assembly_operator = assemble(assemble_dir.path(), "json");
    assert_eq!(assembly_operator.status.code(), Some(2));
    assert!(assembly_operator.stdout.is_empty());
    assert!(!assembly_operator.stderr.is_empty());
}

#[test]
fn typed_load_errors_distinguish_external_io_from_malformed_asset_bytes() {
    let dir = tempfile::tempdir().unwrap();
    write_assembly_inputs(dir.path());
    std::fs::write(
        dir.path().join("external.gltf"),
        missing_external_buffer_gltf(),
    )
    .unwrap();
    std::fs::write(
        dir.path().join("inputs/external.gltf"),
        missing_external_buffer_gltf(),
    )
    .unwrap();
    std::fs::write(dir.path().join("malformed.gltf"), b"not glTF").unwrap();
    std::fs::write(dir.path().join("inputs/malformed.gltf"), b"not glTF").unwrap();

    for format in ["json", "text"] {
        let convert_external = binary()
            .current_dir(dir.path())
            .args([
                "convert",
                "external.gltf",
                "-o",
                "converted.glb",
                "--format",
                format,
            ])
            .output()
            .unwrap();
        assert_eq!(convert_external.status.code(), Some(2));
        assert!(convert_external.stdout.is_empty());
        assert!(!convert_external.stderr.is_empty());

        std::fs::write(
            dir.path().join("recipe.toml"),
            assembly_recipe_with_base("external.gltf", "missing"),
        )
        .unwrap();
        let assembly_external = assemble(dir.path(), format);
        assert_eq!(assembly_external.status.code(), Some(2));
        assert!(assembly_external.stdout.is_empty());
        assert!(!assembly_external.stderr.is_empty());

        let convert_malformed = binary()
            .current_dir(dir.path())
            .args([
                "convert",
                "malformed.gltf",
                "-o",
                "converted.glb",
                "--format",
                format,
            ])
            .output()
            .unwrap();
        if format == "json" {
            let record = assert_refusal(&convert_malformed, "convert", "unreadable-source");
            assert_eq!(record["rejection"]["stage"], "load");
        } else {
            assert_eq!(convert_malformed.status.code(), Some(1));
            assert!(convert_malformed.stdout.is_empty());
            let stderr = String::from_utf8(convert_malformed.stderr).unwrap();
            assert!(stderr.contains("convert refused"), "{stderr}");
            assert!(stderr.contains("[unreadable-source]"), "{stderr}");
        }

        std::fs::write(
            dir.path().join("recipe.toml"),
            assembly_recipe_with_base("malformed.gltf", "missing"),
        )
        .unwrap();
        let assembly_malformed = assemble(dir.path(), format);
        if format == "json" {
            let record = assert_refusal(&assembly_malformed, "assemble", "unreadable-source");
            assert_eq!(record["rejection"]["stage"], "load");
        } else {
            assert_eq!(assembly_malformed.status.code(), Some(1));
            assert!(assembly_malformed.stdout.is_empty());
            let stderr = String::from_utf8(assembly_malformed.stderr).unwrap();
            assert!(stderr.contains("assemble refused"), "{stderr}");
            assert!(stderr.contains("[unreadable-source]"), "{stderr}");
        }
    }

    assert!(!dir.path().join("converted.glb").exists());
    assert!(!dir.path().join("character.glb").exists());
    assert!(!dir.path().join("character.json").exists());
}

#[test]
fn malformed_fbx_is_a_refusal_while_a_missing_fbx_path_is_operator_owned() {
    let dir = tempfile::tempdir().unwrap();
    write_assembly_inputs(dir.path());
    std::fs::write(dir.path().join("malformed.fbx"), b"not an FBX container").unwrap();
    std::fs::write(
        dir.path().join("inputs/malformed.fbx"),
        b"not an FBX container",
    )
    .unwrap();
    std::fs::write(dir.path().join("converted.glb"), b"prior convert").unwrap();
    std::fs::write(dir.path().join("character.glb"), b"prior assembly").unwrap();
    std::fs::write(dir.path().join("character.json"), b"prior evidence").unwrap();

    for format in ["json", "text"] {
        let convert_refusal = binary()
            .current_dir(dir.path())
            .args([
                "convert",
                "malformed.fbx",
                "-o",
                "converted.glb",
                "--format",
                format,
            ])
            .output()
            .unwrap();
        if format == "json" {
            let record = assert_refusal(&convert_refusal, "convert", "unreadable-source");
            assert_eq!(record["rejection"]["stage"], "load");
        } else {
            assert_eq!(convert_refusal.status.code(), Some(1));
            assert!(convert_refusal.stdout.is_empty());
            let stderr = String::from_utf8(convert_refusal.stderr).unwrap();
            assert!(stderr.contains("convert refused"), "{stderr}");
            assert!(stderr.contains("[unreadable-source]"), "{stderr}");
        }

        let convert_operator = binary()
            .current_dir(dir.path())
            .args([
                "convert",
                "missing.fbx",
                "-o",
                "converted.glb",
                "--format",
                format,
            ])
            .output()
            .unwrap();
        assert_eq!(convert_operator.status.code(), Some(2));
        assert!(convert_operator.stdout.is_empty());
        assert!(!convert_operator.stderr.is_empty());

        std::fs::write(
            dir.path().join("recipe.toml"),
            assembly_recipe_with_base("malformed.fbx", "missing"),
        )
        .unwrap();
        let assembly_refusal = assemble(dir.path(), format);
        if format == "json" {
            let record = assert_refusal(&assembly_refusal, "assemble", "unreadable-source");
            assert_eq!(record["rejection"]["stage"], "load");
        } else {
            assert_eq!(assembly_refusal.status.code(), Some(1));
            assert!(assembly_refusal.stdout.is_empty());
            let stderr = String::from_utf8(assembly_refusal.stderr).unwrap();
            assert!(stderr.contains("assemble refused"), "{stderr}");
            assert!(stderr.contains("[unreadable-source]"), "{stderr}");
        }

        std::fs::write(
            dir.path().join("recipe.toml"),
            assembly_recipe_with_base("missing.fbx", "missing"),
        )
        .unwrap();
        let assembly_operator = assemble(dir.path(), format);
        assert_eq!(assembly_operator.status.code(), Some(2));
        assert!(assembly_operator.stdout.is_empty());
        assert!(!assembly_operator.stderr.is_empty());
    }

    assert_eq!(
        std::fs::read(dir.path().join("converted.glb")).unwrap(),
        b"prior convert"
    );
    assert_eq!(
        std::fs::read(dir.path().join("character.glb")).unwrap(),
        b"prior assembly"
    );
    assert_eq!(
        std::fs::read(dir.path().join("character.json")).unwrap(),
        b"prior evidence"
    );
}

#[test]
fn post_parse_operator_errors_survive_a_truly_broken_stderr() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("recipe.toml"), "not = [valid").unwrap();

    for format in ["json", "text"] {
        let convert = into_closed_stderr(
            dir.path(),
            &[
                "convert",
                "missing.glb",
                "-o",
                "output.glb",
                "--format",
                format,
            ],
        );
        assert_eq!(convert.status.code(), Some(2));
        assert!(convert.stdout.is_empty());

        let assembly = into_closed_stderr(
            dir.path(),
            &[
                "assemble",
                "recipe.toml",
                "-o",
                "character.glb",
                "--evidence",
                "character.json",
                "--format",
                format,
            ],
        );
        assert_eq!(assembly.status.code(), Some(2));
        assert!(assembly.stdout.is_empty());
    }
}

#[test]
fn text_refusals_are_typed_stderr_only_and_operator_errors_stay_exit_2() {
    let dir = tempfile::tempdir().unwrap();
    write_assembly_inputs(dir.path());
    std::fs::write(dir.path().join("recipe.toml"), assembly_recipe("missing")).unwrap();
    let refusal = assemble(dir.path(), "text");
    assert_eq!(refusal.status.code(), Some(1));
    assert!(refusal.stdout.is_empty());
    let stderr = String::from_utf8(refusal.stderr).unwrap();
    assert!(stderr.contains("assemble refused"), "{stderr}");
    assert!(stderr.contains("[asset-recipe-mismatch]"), "{stderr}");

    std::fs::write(dir.path().join("recipe.toml"), "not = [valid").unwrap();
    let operator = assemble(dir.path(), "text");
    assert_eq!(operator.status.code(), Some(2));
    assert!(operator.stdout.is_empty());
    assert!(!operator.stderr.is_empty());

    std::fs::copy(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/assets/clip.glb"
        ),
        dir.path().join("input.glb"),
    )
    .unwrap();
    let convert_refusal = binary()
        .current_dir(dir.path())
        .args([
            "convert",
            "input.glb",
            "-o",
            "output.glb",
            "--bake-static-mesh-transforms",
            "--format",
            "text",
        ])
        .output()
        .unwrap();
    assert_eq!(convert_refusal.status.code(), Some(1));
    assert!(convert_refusal.stdout.is_empty());
    let stderr = String::from_utf8(convert_refusal.stderr).unwrap();
    assert!(stderr.contains("convert refused"), "{stderr}");
    assert!(stderr.contains("[transform-refused]"), "{stderr}");

    let convert_operator = binary()
        .current_dir(dir.path())
        .args([
            "convert",
            "missing.glb",
            "-o",
            "output.glb",
            "--format",
            "text",
        ])
        .output()
        .unwrap();
    assert_eq!(convert_operator.status.code(), Some(2));
    assert!(convert_operator.stdout.is_empty());
    assert!(!convert_operator.stderr.is_empty());
}

#[test]
fn refusals_preserve_seeded_convert_artifact_and_assembly_pair() {
    let convert_dir = tempfile::tempdir().unwrap();
    std::fs::copy(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/assets/clip.glb"
        ),
        convert_dir.path().join("input.glb"),
    )
    .unwrap();
    std::fs::write(convert_dir.path().join("output.glb"), b"prior convert").unwrap();
    let convert = binary()
        .current_dir(convert_dir.path())
        .args([
            "convert",
            "input.glb",
            "-o",
            "output.glb",
            "--bake-static-mesh-transforms",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert_refusal(&convert, "convert", "transform-refused");
    assert_eq!(
        std::fs::read(convert_dir.path().join("output.glb")).unwrap(),
        b"prior convert"
    );

    let assemble_dir = tempfile::tempdir().unwrap();
    write_assembly_inputs(assemble_dir.path());
    std::fs::write(
        assemble_dir.path().join("recipe.toml"),
        assembly_recipe("missing"),
    )
    .unwrap();
    std::fs::write(assemble_dir.path().join("character.glb"), b"prior artifact").unwrap();
    std::fs::write(
        assemble_dir.path().join("character.json"),
        b"prior evidence",
    )
    .unwrap();
    let assembly = assemble(assemble_dir.path(), "json");
    assert_refusal(&assembly, "assemble", "asset-recipe-mismatch");
    assert_eq!(
        std::fs::read(assemble_dir.path().join("character.glb")).unwrap(),
        b"prior artifact"
    );
    assert_eq!(
        std::fs::read(assemble_dir.path().join("character.json")).unwrap(),
        b"prior evidence"
    );
}

#[test]
fn closed_json_refusal_stdout_keeps_exit_1_and_diagnoses_once() {
    let dir = tempfile::tempdir().unwrap();
    write_assembly_inputs(dir.path());
    std::fs::write(dir.path().join("recipe.toml"), assembly_recipe("missing")).unwrap();
    let assembly_args = [
        "assemble",
        "recipe.toml",
        "-o",
        "character.glb",
        "--evidence",
        "character.json",
        "--format",
        "json",
    ];
    let output = into_closed_stdout(dir.path(), &assembly_args, false);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(
        stderr
            .matches("animsmith: cannot write JSON output to stdout")
            .count(),
        1,
        "{stderr}"
    );

    assert_eq!(
        into_closed_stdout(dir.path(), &assembly_args, true)
            .status
            .code(),
        Some(1)
    );

    std::fs::copy(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../examples/assets/clip.glb"
        ),
        dir.path().join("input.glb"),
    )
    .unwrap();
    let convert_args = [
        "convert",
        "input.glb",
        "-o",
        "output.glb",
        "--bake-static-mesh-transforms",
        "--format",
        "json",
    ];
    let convert = into_closed_stdout(dir.path(), &convert_args, false);
    assert_eq!(convert.status.code(), Some(1));
    let stderr = String::from_utf8(convert.stderr).unwrap();
    assert_eq!(
        stderr
            .matches("animsmith: cannot write JSON output to stdout")
            .count(),
        1,
        "{stderr}"
    );
    assert_eq!(
        into_closed_stdout(dir.path(), &convert_args, true)
            .status
            .code(),
        Some(1)
    );
}

#[test]
fn refusal_schema_rejects_identity_outcome_command_and_shape_mutations() {
    let dir = tempfile::tempdir().unwrap();
    write_assembly_inputs(dir.path());
    std::fs::write(dir.path().join("recipe.toml"), assembly_recipe("missing")).unwrap();
    let output = assemble(dir.path(), "json");
    let record = assert_refusal(&output, "assemble", "asset-recipe-mismatch");
    let schema: Value = serde_json::from_str(REFUSAL_SCHEMA).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    for mutation in [
        record.clone(),
        record.clone(),
        record.clone(),
        record.clone(),
        record.clone(),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, mut value)| {
        match index {
            0 => value["schema_version"] = 2.into(),
            1 => value["schema"] = "urn:animsmith:schema:producer-refusal:2".into(),
            2 => value["command"] = "scale".into(),
            3 => value["outcome"] = "published".into(),
            4 => {
                value["rejection"].as_object_mut().unwrap().remove("kind");
            }
            _ => unreachable!(),
        }
        value
    }) {
        assert!(
            !validator.is_valid(&mutation),
            "mutation admitted: {mutation:#}"
        );
    }
}
