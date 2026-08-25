use animsmith_core::InputIdentity;
use animsmith_core::glam::Quat;
use animsmith_testkit::{quats_from_angles, two_bone_rotation_doc};
use serde_json::Value;
use std::path::Path;
#[cfg(target_os = "linux")]
use std::process::Stdio;
use std::process::{Command, Output};

const SCHEMA: &str =
    include_str!("../../../docs/schemas/transition-pose-evaluation-v1.schema.json");
const SCHEMA_ID: &str = "urn:animsmith:schema:transition-pose-evaluation:1";

fn animsmith() -> Command {
    Command::new(env!("CARGO_BIN_EXE_animsmith"))
}

fn tempdir(name: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(&format!("animsmith-transition-pose-{name}-"))
        .tempdir()
        .expect("creates temp directory")
}

fn write_document(path: &Path, finding: bool) {
    let mut document = two_bone_rotation_doc("walk", quats_from_angles(&[0.0; 5]), false);
    let mut run = document.clips[0].clone();
    run.name = "run".into();
    if finding {
        run.tracks[0].values =
            animsmith_core::model::TrackValues::Quats(vec![Quat::from_rotation_y(0.5); 5]);
    }
    document.clips.push(run);
    animsmith_gltf::write::write(&document, path).expect("writes synthetic transition fixture");
}

fn config(time_normalized: f64) -> String {
    format!(
        r#"[transition_families."walk_to_run"]
schema = "urn:animsmith:schema:transition-family:1"
schema_version = 1
scope = "document"
boundary = "entry"

[transition_families."walk_to_run".basis]
translation = "skeleton-local-metres"
rotation = "skeleton-local-degrees"
time = "normalized-clip"

[transition_families."walk_to_run".tolerances]
translation_m = 0.0
rotation_deg = 0.0
time_normalized = {time_normalized}

[[transition_families."walk_to_run".members]]
take_index = 0
take_name = "walk"

[[transition_families."walk_to_run".members]]
take_index = 1
take_name = "run"
"#
    )
}

fn run(dir: &Path, input: &Path, config: Option<&Path>) -> Output {
    let mut command = animsmith();
    command
        .current_dir(dir)
        .arg("evaluate-transition-poses")
        .arg(input)
        .arg("--format")
        .arg("json");
    if let Some(config) = config {
        command.arg("--config").arg(config);
    }
    command.output().expect("runs transition-pose command")
}

fn json(output: &Output) -> Value {
    assert!(
        output.stderr.is_empty(),
        "result-bearing command must not write stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("stdout is JSON")
}

fn assert_schema(value: &Value) {
    let schema: Value = serde_json::from_str(SCHEMA).expect("schema parses");
    let validator = jsonschema::validator_for(&schema).expect("schema compiles");
    let errors = validator
        .iter_errors(value)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(
        errors.is_empty(),
        "transition-pose output violates schema:\n{}\n{value:#}",
        errors.join("\n")
    );
}

#[test]
fn omitted_and_explicit_empty_config_are_the_same_zero_byte_declaration() {
    let dir = tempdir("empty-config");
    let input = dir.path().join("input.glb");
    let empty = dir.path().join("empty.toml");
    write_document(&input, false);
    std::fs::write(&empty, []).expect("writes explicit empty TOML");

    let omitted = run(dir.path(), &input, None);
    assert!(omitted.status.success());
    let explicit = run(dir.path(), &input, Some(&empty));
    assert!(explicit.status.success());
    assert_eq!(omitted.stdout, explicit.stdout, "same defined declaration");

    let value = json(&omitted);
    assert_schema(&value);
    assert_eq!(value["schema"], SCHEMA_ID);
    assert_eq!(value["status"], "complete");
    assert_eq!(value["decision"], "pass");
    assert_eq!(value["reason"], "no_configured_families");
    assert_eq!(value["families"], Value::Array(vec![]));
    assert_eq!(
        value["declaration_input"],
        serde_json::to_value(InputIdentity::from_bytes(b"")).unwrap(),
        "the omitted declaration is the exact zero-byte source, not a placeholder"
    );
    assert_eq!(
        value["subject_input"],
        serde_json::to_value(InputIdentity::from_bytes(&std::fs::read(&input).unwrap())).unwrap()
    );
}

#[test]
fn command_reports_pass_finding_and_incomplete_with_the_contract_exit_codes() {
    let dir = tempdir("outcomes");
    let pass_input = dir.path().join("pass.glb");
    let finding_input = dir.path().join("finding.glb");
    let pass_config = dir.path().join("pass.toml");
    let equivalent_config = dir.path().join("equivalent.toml");
    let incomplete_config = dir.path().join("incomplete.toml");
    write_document(&pass_input, false);
    write_document(&finding_input, true);
    let pass_bytes = config(0.0);
    std::fs::write(&pass_config, &pass_bytes).unwrap();
    let equivalent_bytes = format!("# independently authored control bytes\n{pass_bytes}");
    std::fs::write(&equivalent_config, &equivalent_bytes).unwrap();
    std::fs::write(&incomplete_config, config(0.1)).unwrap();

    let pass = run(dir.path(), &pass_input, Some(&pass_config));
    assert!(pass.status.success());
    let pass = json(&pass);
    assert_schema(&pass);
    assert_eq!(pass["status"], "complete");
    assert_eq!(pass["decision"], "pass");
    assert_eq!(
        pass["declaration_input"],
        serde_json::to_value(InputIdentity::from_bytes(pass_bytes.as_bytes())).unwrap()
    );

    let equivalent = run(dir.path(), &pass_input, Some(&equivalent_config));
    assert!(equivalent.status.success());
    let equivalent = json(&equivalent);
    assert_schema(&equivalent);
    assert_ne!(
        pass["declaration_input"], equivalent["declaration_input"],
        "exact config identities retain distinct source bytes"
    );
    assert_eq!(
        pass["declaration_normalized"], equivalent["declaration_normalized"],
        "the normalized declaration excludes irrelevant source presentation"
    );

    let finding = run(dir.path(), &finding_input, Some(&pass_config));
    assert_eq!(finding.status.code(), Some(1));
    let finding = json(&finding);
    assert_schema(&finding);
    assert_eq!(finding["status"], "complete");
    assert_eq!(finding["decision"], "finding");

    let incomplete = run(dir.path(), &pass_input, Some(&incomplete_config));
    assert_eq!(incomplete.status.code(), Some(1));
    let incomplete = json(&incomplete);
    assert_schema(&incomplete);
    assert_eq!(incomplete["status"], "incomplete");
    assert_eq!(incomplete["decision"], "not_evaluated");
    assert_eq!(
        incomplete["families"][0]["reason"],
        "time_tolerance_unsupported"
    );
}

#[test]
fn command_rejects_control_loader_and_non_json_format_errors_without_a_result() {
    let dir = tempdir("errors");
    let input = dir.path().join("input.glb");
    let invalid = dir.path().join("invalid.toml");
    let duplicate = dir.path().join("duplicate.toml");
    write_document(&input, false);
    std::fs::write(&invalid, format!("{}\nunexpected = true\n", config(0.0))).unwrap();

    let invalid_config = run(dir.path(), &input, Some(&invalid));
    assert_eq!(invalid_config.status.code(), Some(2));
    assert!(invalid_config.stdout.is_empty());

    std::fs::write(
        &duplicate,
        format!(
            "{}\n[transition_families.\"walk_to_run\"]\nschema = \"urn:animsmith:schema:transition-family:1\"\n",
            config(0.0)
        ),
    )
    .unwrap();
    let duplicate_config = run(dir.path(), &input, Some(&duplicate));
    assert_eq!(duplicate_config.status.code(), Some(2));
    assert!(duplicate_config.stdout.is_empty());

    let missing = run(dir.path(), &dir.path().join("missing.glb"), None);
    assert_eq!(missing.status.code(), Some(2));
    assert!(missing.stdout.is_empty());

    let format = animsmith()
        .current_dir(dir.path())
        .args([
            "evaluate-transition-poses",
            input.to_str().unwrap(),
            "--format",
            "text",
        ])
        .output()
        .unwrap();
    assert_eq!(format.status.code(), Some(2));
    assert!(format.stdout.is_empty());
}

#[test]
fn command_is_deterministic_and_help_advertises_json_only_output() {
    let dir = tempdir("deterministic");
    let input = dir.path().join("input.glb");
    let config_path = dir.path().join("config.toml");
    write_document(&input, false);
    std::fs::write(&config_path, config(0.0)).unwrap();

    let first = run(dir.path(), &input, Some(&config_path));
    let second = run(dir.path(), &input, Some(&config_path));
    assert!(first.status.success());
    assert_eq!(first.stdout, second.stdout);

    let help = animsmith()
        .args(["evaluate-transition-poses", "--help"])
        .output()
        .unwrap();
    assert!(help.status.success());
    let help = String::from_utf8(help.stdout).unwrap();
    assert!(help.contains("--format <FORMAT>"));
    assert!(help.contains("[possible values: json]"));
}

#[cfg(target_os = "linux")]
#[test]
fn failed_standalone_result_delivery_is_an_operator_error() {
    let dir = tempdir("stdout-full");
    let input = dir.path().join("input.glb");
    write_document(&input, false);

    let full = std::fs::OpenOptions::new()
        .write(true)
        .open("/dev/full")
        .expect("Unix /dev/full device");
    let output = animsmith()
        .current_dir(dir.path())
        .args(["evaluate-transition-poses", "input.glb", "--format", "json"])
        .stdout(Stdio::from(full))
        .output()
        .expect("runs transition-pose command");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("cannot write JSON output to stdout"));
}
