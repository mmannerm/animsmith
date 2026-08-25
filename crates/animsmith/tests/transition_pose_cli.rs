use animsmith_core::InputIdentity;
use animsmith_core::glam::Quat;
use animsmith_testkit::{quats_from_angles, two_bone_rotation_doc};
use serde_json::Value;
use std::path::{Path, PathBuf};
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

fn glb_u32(bytes: &[u8], offset: usize) -> usize {
    usize::try_from(u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("self-authored GLB has a complete length field"),
    ))
    .expect("u32 GLB length fits usize")
}

struct ExternalDocument {
    bin: PathBuf,
    run_rotation_entry_offset: usize,
}

fn run_rotation_entry_offset(document: &Value) -> usize {
    let animation = document["animations"]
        .as_array()
        .and_then(|animations| {
            animations
                .iter()
                .find(|animation| animation["name"] == "run")
        })
        .expect("self-authored fixture has the run animation");
    let channel = animation["channels"]
        .as_array()
        .and_then(|channels| {
            channels
                .iter()
                .find(|channel| channel["target"]["path"] == "rotation")
        })
        .expect("self-authored fixture has a run rotation channel");
    let sampler = animation["samplers"]
        .as_array()
        .and_then(|samplers| {
            channel["sampler"]
                .as_u64()
                .and_then(|index| samplers.get(index as usize))
        })
        .expect("rotation channel selects a sampler");
    let accessor = document["accessors"]
        .as_array()
        .and_then(|accessors| {
            sampler["output"]
                .as_u64()
                .and_then(|index| accessors.get(index as usize))
        })
        .expect("rotation sampler selects an output accessor");
    assert_eq!(accessor["type"], "VEC4");
    let view_offset = document["bufferViews"]
        .as_array()
        .and_then(|views| {
            accessor["bufferView"]
                .as_u64()
                .and_then(|index| views.get(index as usize))
        })
        .and_then(|view| view["byteOffset"].as_u64())
        .and_then(|offset| usize::try_from(offset).ok())
        .unwrap_or(0);
    let accessor_offset = accessor["byteOffset"]
        .as_u64()
        .and_then(|offset| usize::try_from(offset).ok())
        .unwrap_or(0);
    view_offset
        .checked_add(accessor_offset)
        .expect("self-authored rotation accessor offset fits usize")
}

fn write_external_document(
    path: &Path,
    finding: bool,
    unmodeled_extension: bool,
) -> ExternalDocument {
    let glb = path.with_extension("glb");
    write_document(&glb, finding);
    let bytes = std::fs::read(&glb).expect("reads self-authored GLB");
    assert_eq!(&bytes[..4], b"glTF");
    assert_eq!(&bytes[16..20], b"JSON");
    let json_end = 20 + glb_u32(&bytes, 12);
    assert_eq!(&bytes[json_end + 4..json_end + 8], b"BIN\0");
    let bin_end = json_end + 8 + glb_u32(&bytes, json_end);
    assert_eq!(bin_end, bytes.len(), "writer emits one complete BIN chunk");

    let mut document: Value =
        serde_json::from_slice(&bytes[20..json_end]).expect("self-authored GLB JSON parses");
    document["buffers"][0]["uri"] = Value::String("animation.bin".into());
    if unmodeled_extension {
        document["extensionsUsed"] = serde_json::json!(["ANIMSMITH_test_unmodeled"]);
    }
    let bin = path.with_file_name("animation.bin");
    std::fs::write(path, serde_json::to_vec(&document).unwrap()).expect("writes primary glTF");
    std::fs::write(&bin, &bytes[json_end + 8..bin_end]).expect("writes external animation bytes");
    ExternalDocument {
        bin,
        run_rotation_entry_offset: run_rotation_entry_offset(&document),
    }
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
fn external_animation_bytes_bind_the_same_load_dependency_closure() {
    let dir = tempdir("external-closure");
    let input = dir.path().join("input.gltf");
    let config_path = dir.path().join("config.toml");
    let external = write_external_document(&input, false, false);
    std::fs::write(&config_path, config(0.0)).unwrap();
    let primary = std::fs::read(&input).unwrap();

    let first = run(dir.path(), &input, Some(&config_path));
    assert!(first.status.success());
    let first = json(&first);
    assert_schema(&first);
    assert_eq!(first["decision"], "pass");
    let first_closure = first["subject_dependency_closure_identity"].clone();
    assert!(
        first_closure.is_object(),
        "configured source retains its closure"
    );
    for member in first["families"][0]["members"].as_array().unwrap() {
        assert_eq!(member["source_dependency_closure_identity"], first_closure);
    }

    let mut changed = std::fs::read(&external.bin).unwrap();
    let offset = external.run_rotation_entry_offset;
    changed[offset..offset + 4].copy_from_slice(&0.24740396f32.to_le_bytes());
    changed[offset + 12..offset + 16].copy_from_slice(&0.9689124f32.to_le_bytes());
    std::fs::write(&external.bin, changed).unwrap();
    assert_eq!(
        std::fs::read(&input).unwrap(),
        primary,
        "primary glTF remains unchanged"
    );

    let second = run(dir.path(), &input, Some(&config_path));
    assert_eq!(second.status.code(), Some(1));
    let second = json(&second);
    assert_schema(&second);
    assert_eq!(second["status"], "complete");
    assert_eq!(second["decision"], "finding");
    assert_eq!(second["subject_input"], first["subject_input"]);
    assert_ne!(second["subject_dependency_closure_identity"], first_closure);
    assert_ne!(
        second["families"][0]["pairs"],
        first["families"][0]["pairs"]
    );
    assert!(
        !second["families"][0]["pairs"][0]["rotation_offenders"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    for member in second["families"][0]["members"].as_array().unwrap() {
        assert_eq!(
            member["source_dependency_closure_identity"],
            second["subject_dependency_closure_identity"]
        );
        assert_ne!(member["source_dependency_closure_identity"], first_closure);
    }
}

#[test]
fn incomplete_load_closure_blocks_configured_families_but_not_no_config() {
    let dir = tempdir("incomplete-closure");
    let input = dir.path().join("input.gltf");
    let config_path = dir.path().join("config.toml");
    write_external_document(&input, false, true);
    std::fs::write(&config_path, config(0.0)).unwrap();

    let configured = run(dir.path(), &input, Some(&config_path));
    assert_eq!(configured.status.code(), Some(1));
    let configured = json(&configured);
    assert_schema(&configured);
    assert_eq!(configured["status"], "incomplete");
    assert_eq!(configured["decision"], "not_evaluated");
    assert!(
        configured
            .get("subject_dependency_closure_identity")
            .is_none()
    );
    assert_eq!(
        configured["families"][0]["reason"],
        "dependency_closure_incomplete"
    );
    for member in configured["families"][0]["members"].as_array().unwrap() {
        assert!(member.get("source_dependency_closure_identity").is_none());
    }

    let no_config = run(dir.path(), &input, None);
    assert!(no_config.status.success());
    let no_config = json(&no_config);
    assert_schema(&no_config);
    assert_eq!(no_config["reason"], "no_configured_families");
    assert!(
        no_config
            .get("subject_dependency_closure_identity")
            .is_none()
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

#[test]
fn collection_command_binds_manifest_and_compares_cross_file_members() {
    let dir = tempdir("collection");
    let walk = dir.path().join("walk.glb");
    let run = dir.path().join("run.glb");
    let manifest = dir.path().join("collection.toml");
    let families = dir.path().join("families.toml");
    write_document(&walk, false);
    write_document(&run, true);
    let manifest_bytes = r#"schema = "urn:animsmith:schema:collection-manifest:1"
schema_version = 1
collection_id = "test"
sources = [
  { key = "walk", path = "walk.glb" },
  { key = "run", path = "run.glb" },
]
clips = [
  { id = "test/walk", source = "walk", take_index = 0, take_name = "walk" },
  { id = "test/run", source = "run", take_index = 1, take_name = "run" },
]
"#;
    std::fs::write(&manifest, manifest_bytes).unwrap();
    let identity = InputIdentity::from_bytes(manifest_bytes.as_bytes());
    std::fs::write(
        &families,
        format!(
            r#"schema = "urn:animsmith:schema:transition-family:1"
schema_version = 1
scope = "collection"
collection_id = "test"
manifest_input_identity = {{ sha256 = "{}", bytes = {} }}
[[families]]
family_id = "test/walk_to_run"
boundary = "entry"
[families.basis]
translation = "skeleton-local-metres"
rotation = "skeleton-local-degrees"
time = "normalized-clip"
[families.tolerances]
translation_m = 0.0
rotation_deg = 0.0
time_normalized = 0.0
[[families.members]]
logical_id = "test/walk"
source = "walk"
take_index = 0
take_name = "walk"
[[families.members]]
logical_id = "test/run"
source = "run"
take_index = 1
take_name = "run"
"#,
            identity.sha256(),
            identity.bytes()
        ),
    )
    .unwrap();
    let output = animsmith()
        .current_dir(dir.path())
        .args([
            "collection",
            "evaluate-transition-poses",
            "collection.toml",
            "--families",
            "families.toml",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let value = json(&output);
    assert_schema(&value);
    assert_eq!(
        value["subject_input"],
        serde_json::to_value(identity).unwrap()
    );
    assert_eq!(value["decision"], "finding");
    assert_ne!(
        value["families"][0]["members"][0]["source_input"],
        value["families"][0]["members"][1]["source_input"]
    );
    assert!(
        value["families"][0]["members"][0]
            .get("source_dependency_closure_identity")
            .is_some(),
        "available collection members bind same-load closure identity"
    );
}

#[test]
fn collection_stale_late_member_has_control_precedence_over_source_loading() {
    let dir = tempdir("collection-stale-late-member");
    let manifest = dir.path().join("collection.toml");
    let families = dir.path().join("families.toml");
    let manifest_bytes = r#"schema = "urn:animsmith:schema:collection-manifest:1"
schema_version = 1
collection_id = "test"
sources = [
  { key = "missing", path = "does-not-exist.glb" },
  { key = "run", path = "also-does-not-exist.glb" },
]
clips = [
  { id = "test/walk", source = "missing", take_index = 0, take_name = "walk" },
  { id = "test/run", source = "run", take_index = 1, take_name = "run" },
]
"#;
    std::fs::write(&manifest, manifest_bytes).unwrap();
    let identity = InputIdentity::from_bytes(manifest_bytes.as_bytes());
    std::fs::write(
        &families,
        format!(
            r#"schema = "urn:animsmith:schema:transition-family:1"
schema_version = 1
scope = "collection"
collection_id = "test"
manifest_input_identity = {{ sha256 = "{}", bytes = {} }}
[[families]]
family_id = "test/walk_to_run"
boundary = "entry"
[families.basis]
translation = "skeleton-local-metres"
rotation = "skeleton-local-degrees"
time = "normalized-clip"
[families.tolerances]
translation_m = 0.0
rotation_deg = 0.0
time_normalized = 0.0
[[families.members]]
logical_id = "test/walk"
source = "missing"
take_index = 0
take_name = "walk"
[[families.members]]
logical_id = "test/run"
source = "run"
take_index = 1
take_name = "stale"
"#,
            identity.sha256(),
            identity.bytes()
        ),
    )
    .unwrap();

    let output = animsmith()
        .current_dir(dir.path())
        .args([
            "collection",
            "evaluate-transition-poses",
            "collection.toml",
            "--families",
            "families.toml",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("stale-member-binding"));
}

#[test]
fn collection_external_animation_closure_changes_when_primary_gltf_does_not() {
    let dir = tempdir("collection-external-closure");
    let walk_dir = dir.path().join("walk");
    let run_dir = dir.path().join("run");
    std::fs::create_dir_all(&walk_dir).unwrap();
    std::fs::create_dir_all(&run_dir).unwrap();
    let walk = walk_dir.join("walk.gltf");
    let run = run_dir.join("run.gltf");
    let manifest = dir.path().join("collection.toml");
    let families = dir.path().join("families.toml");
    write_external_document(&walk, false, false);
    let run_external = write_external_document(&run, false, false);
    let run_primary = std::fs::read(&run).unwrap();
    let run_pin = InputIdentity::from_bytes(&run_primary);
    let manifest_bytes = format!(
        r#"schema = "urn:animsmith:schema:collection-manifest:1"
schema_version = 1
collection_id = "test"
sources = [
  {{ key = "walk", path = "walk/walk.gltf" }},
  {{ key = "run", path = "run/run.gltf", expected_sha256 = "{}" }},
]
clips = [
  {{ id = "test/walk", source = "walk", take_index = 0, take_name = "walk" }},
  {{ id = "test/run", source = "run", take_index = 1, take_name = "run" }},
]
"#,
        run_pin.sha256()
    );
    std::fs::write(&manifest, &manifest_bytes).unwrap();
    let identity = InputIdentity::from_bytes(manifest_bytes.as_bytes());
    std::fs::write(
        &families,
        format!(
            r#"schema = "urn:animsmith:schema:transition-family:1"
schema_version = 1
scope = "collection"
collection_id = "test"
manifest_input_identity = {{ sha256 = "{}", bytes = {} }}
[[families]]
family_id = "test/walk_to_run"
boundary = "entry"
[families.basis]
translation = "skeleton-local-metres"
rotation = "skeleton-local-degrees"
time = "normalized-clip"
[families.tolerances]
translation_m = 0.0
rotation_deg = 0.0
time_normalized = 0.0
[[families.members]]
logical_id = "test/walk"
source = "walk"
take_index = 0
take_name = "walk"
[[families.members]]
logical_id = "test/run"
source = "run"
take_index = 1
take_name = "run"
"#,
            identity.sha256(),
            identity.bytes()
        ),
    )
    .unwrap();
    let command = [
        "collection",
        "evaluate-transition-poses",
        "collection.toml",
        "--families",
        "families.toml",
        "--format",
        "json",
    ];

    let first = animsmith()
        .current_dir(dir.path())
        .args(command)
        .output()
        .unwrap();
    assert!(first.status.success());
    let first = json(&first);
    assert_schema(&first);
    assert_eq!(first["status"], "complete");
    assert_eq!(first["decision"], "pass");
    let first_closure =
        first["families"][0]["members"][1]["source_dependency_closure_identity"].clone();
    assert!(first_closure.is_object());

    let mut changed = std::fs::read(&run_external.bin).unwrap();
    let offset = run_external.run_rotation_entry_offset;
    changed[offset..offset + 4].copy_from_slice(&0.24740396f32.to_le_bytes());
    changed[offset + 12..offset + 16].copy_from_slice(&0.9689124f32.to_le_bytes());
    std::fs::write(&run_external.bin, changed).unwrap();
    assert_eq!(std::fs::read(&run).unwrap(), run_primary);
    assert_eq!(
        InputIdentity::from_bytes(&std::fs::read(&run).unwrap()).sha256(),
        run_pin.sha256(),
        "the manifest primary digest pin remains valid"
    );

    let second = animsmith()
        .current_dir(dir.path())
        .args(command)
        .output()
        .unwrap();
    assert_eq!(second.status.code(), Some(1));
    let second = json(&second);
    assert_schema(&second);
    assert_eq!(second["status"], "complete");
    assert_eq!(second["decision"], "finding");
    assert_eq!(second["subject_input"], first["subject_input"]);
    assert_eq!(
        second["families"][0]["members"][1]["source_input"],
        first["families"][0]["members"][1]["source_input"],
        "the source pin covers only the unchanged primary glTF"
    );
    assert_ne!(
        second["families"][0]["members"][1]["source_dependency_closure_identity"],
        first_closure
    );
    assert_ne!(
        second["families"][0]["pairs"],
        first["families"][0]["pairs"]
    );
    assert!(
        !second["families"][0]["pairs"][0]["rotation_offenders"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn collection_partial_closure_never_evaluates_the_available_member_subset() {
    let dir = tempdir("collection-partial-closure");
    let walk_dir = dir.path().join("walk");
    let run_dir = dir.path().join("run");
    std::fs::create_dir_all(&walk_dir).unwrap();
    std::fs::create_dir_all(&run_dir).unwrap();
    write_external_document(&walk_dir.join("walk.gltf"), false, false);
    write_external_document(&run_dir.join("run.gltf"), false, true);
    let manifest = dir.path().join("collection.toml");
    let families = dir.path().join("families.toml");
    let manifest_bytes = r#"schema = "urn:animsmith:schema:collection-manifest:1"
schema_version = 1
collection_id = "test"
sources = [
  { key = "walk", path = "walk/walk.gltf" },
  { key = "run", path = "run/run.gltf" },
]
clips = [
  { id = "test/walk", source = "walk", take_index = 0, take_name = "walk" },
  { id = "test/run", source = "run", take_index = 1, take_name = "run" },
]
"#;
    std::fs::write(&manifest, manifest_bytes).unwrap();
    let identity = InputIdentity::from_bytes(manifest_bytes.as_bytes());
    std::fs::write(
        &families,
        format!(
            r#"schema = "urn:animsmith:schema:transition-family:1"
schema_version = 1
scope = "collection"
collection_id = "test"
manifest_input_identity = {{ sha256 = "{}", bytes = {} }}
[[families]]
family_id = "test/walk_to_run"
boundary = "both"
[families.basis]
translation = "skeleton-local-metres"
rotation = "skeleton-local-degrees"
time = "normalized-clip"
[families.tolerances]
translation_m = 0.0
rotation_deg = 0.0
time_normalized = 0.0
[[families.members]]
logical_id = "test/walk"
source = "walk"
take_index = 0
take_name = "walk"
[[families.members]]
logical_id = "test/run"
source = "run"
take_index = 1
take_name = "run"
"#,
            identity.sha256(),
            identity.bytes()
        ),
    )
    .unwrap();

    let output = animsmith()
        .current_dir(dir.path())
        .args([
            "collection",
            "evaluate-transition-poses",
            "collection.toml",
            "--families",
            "families.toml",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let value = json(&output);
    assert_schema(&value);
    assert_eq!(value["status"], "incomplete");
    assert_eq!(value["decision"], "not_evaluated");
    assert_eq!(
        value["families"][0]["reason"],
        "dependency_closure_incomplete"
    );
    assert!(value["families"][0]["members"][0]["source_input"].is_object());
    assert!(value["families"][0]["members"][0]["source_dependency_closure_identity"].is_object());
    assert!(value["families"][0]["members"][1]["source_input"].is_object());
    assert!(
        value["families"][0]["members"][1]
            .get("source_dependency_closure_identity")
            .is_none()
    );
    assert!(value["families"][0].get("skeleton_basis_input").is_none());
    assert!(value["families"][0]["pairs"].as_array().unwrap().is_empty());
}

#[test]
fn collection_unrelated_malformed_source_is_never_loaded() {
    let dir = tempdir("collection-unrelated-poison");
    write_document(&dir.path().join("selected.glb"), false);
    std::fs::write(dir.path().join("poison.glb"), b"not an asset").unwrap();
    let manifest_bytes = r#"schema = "urn:animsmith:schema:collection-manifest:1"
schema_version = 1
collection_id = "test"
sources = [
  { key = "selected", path = "selected.glb" },
  { key = "poison", path = "poison.glb" },
]
clips = [
  { id = "test/walk", source = "selected", take_index = 0, take_name = "walk" },
  { id = "test/run", source = "selected", take_index = 1, take_name = "run" },
  { id = "test/poison", source = "poison", take_index = 0, take_name = "poison" },
]
"#;
    std::fs::write(dir.path().join("collection.toml"), manifest_bytes).unwrap();
    let identity = InputIdentity::from_bytes(manifest_bytes.as_bytes());
    std::fs::write(
        dir.path().join("families.toml"),
        format!(
            r#"schema = "urn:animsmith:schema:transition-family:1"
schema_version = 1
scope = "collection"
collection_id = "test"
manifest_input_identity = {{ sha256 = "{}", bytes = {} }}
[[families]]
family_id = "test/shared"
boundary = "entry"
[families.basis]
translation = "skeleton-local-metres"
rotation = "skeleton-local-degrees"
time = "normalized-clip"
[families.tolerances]
translation_m = 0.0
rotation_deg = 0.0
time_normalized = 0.0
[[families.members]]
logical_id = "test/walk"
source = "selected"
take_index = 0
take_name = "walk"
[[families.members]]
logical_id = "test/run"
source = "selected"
take_index = 1
take_name = "run"
"#,
            identity.sha256(),
            identity.bytes()
        ),
    )
    .unwrap();

    let output = animsmith()
        .current_dir(dir.path())
        .args([
            "collection",
            "evaluate-transition-poses",
            "collection.toml",
            "--families",
            "families.toml",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let value = json(&output);
    assert_schema(&value);
    assert_eq!(value["decision"], "pass");
    assert_eq!(
        value["families"][0]["members"][0]["source_input"],
        value["families"][0]["members"][1]["source_input"]
    );
}

#[test]
fn collection_invalid_explicit_source_config_fails_before_asset_runtime() {
    let dir = tempdir("collection-invalid-source-config");
    std::fs::write(dir.path().join("bad.toml"), b"not valid = = toml [[[").unwrap();
    let manifest_bytes = r#"schema = "urn:animsmith:schema:collection-manifest:1"
schema_version = 1
collection_id = "test"
sources = [
  { key = "selected", path = "missing.glb", config = "bad.toml" },
]
clips = [
  { id = "test/walk", source = "selected", take_index = 0, take_name = "walk" },
  { id = "test/run", source = "selected", take_index = 1, take_name = "run" },
]
"#;
    std::fs::write(dir.path().join("collection.toml"), manifest_bytes).unwrap();
    let identity = InputIdentity::from_bytes(manifest_bytes.as_bytes());
    std::fs::write(
        dir.path().join("families.toml"),
        format!(
            r#"schema = "urn:animsmith:schema:transition-family:1"
schema_version = 1
scope = "collection"
collection_id = "test"
manifest_input_identity = {{ sha256 = "{}", bytes = {} }}
[[families]]
family_id = "test/shared"
boundary = "entry"
[families.basis]
translation = "skeleton-local-metres"
rotation = "skeleton-local-degrees"
time = "normalized-clip"
[families.tolerances]
translation_m = 0.0
rotation_deg = 0.0
time_normalized = 0.0
[[families.members]]
logical_id = "test/walk"
source = "selected"
take_index = 0
take_name = "walk"
[[families.members]]
logical_id = "test/run"
source = "selected"
take_index = 1
take_name = "run"
"#,
            identity.sha256(),
            identity.bytes()
        ),
    )
    .unwrap();

    let output = animsmith()
        .current_dir(dir.path())
        .args([
            "collection",
            "evaluate-transition-poses",
            "collection.toml",
            "--families",
            "families.toml",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("config-malformed"), "{stderr}");
}
