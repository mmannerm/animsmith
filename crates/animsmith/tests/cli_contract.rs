use animsmith_core::glam::{Quat, Vec3};
use animsmith_core::model::*;
use animsmith_gltf::fix::{FixSession, Repair as GltfRepair};
use animsmith_testkit::{quats_from_angles, scaled_quat, two_bone_rotation_doc};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

const OUTPUT_SCHEMA_ID: &str = "urn:animsmith:schema:output:7";
const MEASUREMENTS_SCHEMA_ID: &str = "urn:animsmith:schema:measurements:13";
const HOSTILE_PRESENTATION_TEXT: &str = "forged\nline\u{1b}[31m\u{2028}\u{2029}\u{202e}";
const OUTPUT_SCHEMA: &str = include_str!("../../../docs/schemas/output-v7.schema.json");
const MEASUREMENTS_SCHEMA: &str =
    include_str!("../../../docs/schemas/measurements-v13.schema.json");
const EXPECTED_CHECK_IDS: [&str; 26] = [
    "nan",
    "time-monotonic",
    "quat-norm",
    "quat-flip",
    "duration-sanity",
    "scale-keys",
    "non-uniform-scale",
    "constant-nonunit-scale",
    "constant-track",
    "missing-bones",
    "required-bones",
    "rest-world-scale",
    "frozen-bone",
    "duplicate-loop-endpoint",
    "loop-closure",
    "loop-seam",
    "loop-seam-vel",
    "loop-seam-rot",
    "root-motion-speed",
    "gait-group",
    "sync-group",
    "time-complement",
    "in-place",
    "fps",
    "bind-pose",
    "foot-slide",
];

fn output_validator() -> jsonschema::Validator {
    let output: Value = serde_json::from_str(OUTPUT_SCHEMA).expect("valid output schema JSON");
    let measurements: Value =
        serde_json::from_str(MEASUREMENTS_SCHEMA).expect("valid measurement schema JSON");
    let registry = jsonschema::Registry::new()
        .add(MEASUREMENTS_SCHEMA_ID, measurements)
        .expect("valid measurement schema identity")
        .prepare()
        .expect("measurement schema registry prepares");
    jsonschema::options()
        .with_registry(&registry)
        .build(&output)
        .expect("output schema compiles with nested measurement contract")
}

fn assert_output_schema_valid(instance: &Value) {
    let validator = output_validator();
    let errors: Vec<_> = validator
        .iter_errors(instance)
        .map(|error| error.to_string())
        .collect();
    assert!(
        errors.is_empty(),
        "output must satisfy the published v7 schemas:\n{}\ninstance: {instance:#}",
        errors.join("\n")
    );
}

fn assert_evaluation_summary_matches_checks(instance: &Value) {
    let checks: Vec<_> = instance["files"]
        .as_array()
        .expect("output files")
        .iter()
        .flat_map(|file| file["checks"].as_array().expect("output checks"))
        .collect();
    let summary = &instance["summary"]["checks"];
    for (field, dimension, value) in [
        ("complete", "evaluation", "complete"),
        ("partial", "evaluation", "partial"),
        ("not_evaluated", "evaluation", "not_evaluated"),
    ] {
        let expected = checks
            .iter()
            .filter(|check| check[dimension] == value)
            .count();
        assert_eq!(summary["evaluation"][field], expected, "summary.{field}");
    }
    for (field, dimension, value) in [
        ("not_applicable", "applicability", "not_applicable"),
        ("disabled", "configuration", "disabled"),
        ("unselected", "selection", "unselected"),
    ] {
        let expected = checks
            .iter()
            .filter(|check| check[dimension] == value)
            .count();
        assert_eq!(summary[dimension][field], expected, "summary.{field}");
    }
    let expected_gaps: usize = checks
        .iter()
        .map(|check| check["gaps"].as_array().map_or(0, Vec::len))
        .sum();
    assert_eq!(summary["gaps"], expected_gaps, "summary.gaps");
    let total = checks.len();
    assert_eq!(summary["total"], total);
    for fields in [
        &["selected", "unselected"][..],
        &["enabled", "disabled"][..],
        &["applicable", "not_applicable"][..],
        &["complete", "partial", "not_evaluated"][..],
    ] {
        let axis = if fields[0] == "selected" {
            "selection"
        } else if fields[0] == "enabled" {
            "configuration"
        } else if fields[0] == "applicable" {
            "applicability"
        } else {
            "evaluation"
        };
        let sum: u64 = fields
            .iter()
            .map(|field| summary[axis][field].as_u64().unwrap())
            .sum();
        assert_eq!(sum, total as u64, "{axis} partition");
    }
}

fn animsmith() -> Command {
    Command::new(env!("CARGO_BIN_EXE_animsmith"))
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("testdata")
        .join(name)
}

fn example_asset(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/assets")
        .join(name)
}

fn unique_temp_dir(name: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(&format!("animsmith-cli-{name}-"))
        .tempdir()
        .expect("creates temp dir")
}

fn embedded_input_identity() -> animsmith_core::InputIdentity {
    animsmith_core::InputIdentity::from_bytes(b"embedded primary-file bytes")
}

fn input_identity_json(path: &std::path::Path) -> Value {
    serde_json::to_value(animsmith_core::InputIdentity::from_bytes(
        &std::fs::read(path).expect("reads fixture bytes"),
    ))
    .expect("input identity serializes")
}

/// Analytic rotation sequence: consecutive y-rotations 0.4 rad apart,
/// so every adjacent pair has a positive dot product — the clean form
/// is exactly the un-negated sequence.
fn sway_quats(flipped: bool) -> Vec<Quat> {
    let mut quats = quats_from_angles(&[0.0, 0.4, 0.8, 1.2, 1.6]);
    if flipped {
        quats[1] = -quats[1];
        quats[3] = -quats[3];
    }
    quats
}

fn sway_doc_with_quats(quats: Vec<Quat>) -> Document {
    two_bone_rotation_doc("sway", quats, false)
}

fn sway_doc(flipped: bool) -> Document {
    sway_doc_with_quats(sway_quats(flipped))
}

fn sway_doc_with_distinct_repairs() -> Document {
    let mut quats = sway_quats(true);
    quats[1] = scaled_quat(quats[1], 1.2);
    sway_doc_with_quats(quats)
}

fn write_flipped_glb(path: &std::path::Path) {
    animsmith_gltf::write::write(&sway_doc(true), path).expect("writes flipped fixture");
}

fn write_distinct_repair_glb(path: &std::path::Path) {
    animsmith_gltf::write::write(&sway_doc_with_distinct_repairs(), path)
        .expect("writes distinct repair fixture");
}

fn write_clean_glb(path: &std::path::Path) {
    animsmith_gltf::write::write(&sway_doc(false), path).expect("writes clean fixture");
}

/// A static skeleton with one deliberately ambiguous display name. It has no
/// clips, which proves the required-bones contract is structural rather than
/// a disguised per-clip keyframe requirement.
fn write_required_bones_glb(path: &std::path::Path) {
    let bone = |name: &str| Bone {
        name: name.into(),
        parent: None,
        rest: Transform::IDENTITY,
        inverse_bind: None,
    };
    let doc = Document {
        skeleton: Skeleton {
            bones: vec![
                bone("root"),
                bone("weapon_socket"),
                bone("duplicate"),
                bone("duplicate"),
            ],
        },
        ..Document::default()
    };
    animsmith_gltf::write::write(&doc, path).expect("writes required-bones fixture");
}

fn write_empty_skeleton_glb(path: &std::path::Path) {
    animsmith_gltf::write::write(&Document::default(), path)
        .expect("writes empty-skeleton fixture");
}

/// A closed rotational loop with a C1 cusp at the wrap: it leaves at 2 rad/s
/// and enters at -1 rad/s. There are no translation tracks, so the linear
/// seam metric must remain zero while the angular check reports the cusp.
fn write_angular_cusp_glb(path: &std::path::Path) {
    let doc = two_bone_rotation_doc(
        "angular_cusp",
        quats_from_angles(&[0.0, 0.5, 1.0, 0.25, 0.0]),
        false,
    );
    animsmith_gltf::write::write(&doc, path).expect("writes angular-cusp fixture");
}

fn write_two_clip_clean_glb(path: &std::path::Path) {
    let mut doc = sway_doc(false);
    let mut second = doc.clips[0].clone();
    second.name = "sway_b".into();
    doc.clips.push(second);
    animsmith_gltf::write::write(&doc, path).expect("writes two-clip fixture");
}

fn write_time_complement_glb(path: &std::path::Path) {
    let mut doc =
        animsmith_gltf::load(&example_asset("walk.glb")).expect("loads synthetic walk example");
    doc.clips[0].name = "forward".into();
    let mut reflected = doc.clips[0].clone();
    reflected.name = "reflected".into();
    for track in &mut reflected.tracks {
        match &mut track.values {
            TrackValues::Vec3s(values) => values.reverse(),
            TrackValues::Quats(_) => unreachable!("walk fixture uses translation tracks"),
        }
    }
    doc.clips.push(reflected);
    animsmith_gltf::write::write(&doc, path).expect("writes time-complement fixture");
}

fn write_duplicate_loop_endpoint_glb(path: &std::path::Path) {
    let mut doc = sway_doc(false);
    doc.clips[0].name = "guard".into();
    doc.clips[0].duration_s = 1.0;
    doc.clips[0].tracks = vec![Track {
        bone: 0,
        property: Property::Translation,
        interpolation: Interpolation::Linear,
        times: vec![0.0, 0.25, 0.5, 0.75, 1.0],
        values: TrackValues::Vec3s(vec![
            Vec3::ZERO,
            Vec3::X,
            2.0 * Vec3::X,
            Vec3::X,
            Vec3::ZERO,
        ]),
    }];
    animsmith_gltf::write::write(&doc, path).expect("writes duplicate endpoint fixture");
}

fn write_hostile_glb(path: &std::path::Path, hostile: &str, flipped: bool) {
    let mut doc = sway_doc(flipped);
    doc.clips[0].name = hostile.into();
    for bone in &mut doc.skeleton.bones {
        bone.name = hostile.into();
    }
    doc.assets.meshes.push(MeshAsset {
        name: hostile.into(),
        source_mesh_index: doc.assets.meshes.len(),
        primitives: vec![Primitive {
            positions: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
            ],
            ..Primitive::default()
        }],
    });
    doc.assets.instances.push(MeshInstance {
        source_node_index: doc.assets.instances.len(),
        node: 0,
        mesh: doc.assets.meshes.len() - 1,
        ..MeshInstance::default()
    });
    animsmith_gltf::write::write(&doc, path).expect("writes hostile-name fixture");
}

fn write_multi_mesh_glb(path: &std::path::Path) {
    let bone = |name: &str| Bone {
        name: name.into(),
        parent: None,
        rest: Transform::IDENTITY,
        inverse_bind: None,
    };
    let material = |name: &str| MaterialAsset {
        name: name.into(),
        base_color: [1.0; 4],
        metallic: 0.0,
        roughness: 1.0,
        base_color_texture: None,
        normal_texture: None,
        metallic_roughness_texture: None,
        occlusion_texture: None,
    };
    let primitive = |material| Primitive {
        material,
        positions: vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        ],
        ..Primitive::default()
    };
    let doc = Document {
        skeleton: Skeleton {
            bones: vec![
                bone("body-node"),
                bone("prop-node"),
                bone(" duplicate-node "),
                bone(" duplicate-node "),
                bone(" duplicate-node "),
            ],
        },
        assets: SceneAssets {
            meshes: vec![
                MeshAsset {
                    name: "body-mesh".into(),
                    source_mesh_index: 0,
                    primitives: vec![primitive(Some(0))],
                },
                MeshAsset {
                    name: "prop-mesh".into(),
                    source_mesh_index: 1,
                    primitives: vec![primitive(Some(1))],
                },
            ],
            instances: vec![
                MeshInstance {
                    source_node_index: 0,
                    node: 0,
                    mesh: 0,
                    ..MeshInstance::default()
                },
                MeshInstance {
                    source_node_index: 1,
                    node: 1,
                    mesh: 1,
                    ..MeshInstance::default()
                },
                MeshInstance {
                    source_node_index: 2,
                    node: 2,
                    mesh: 0,
                    ..MeshInstance::default()
                },
                MeshInstance {
                    source_node_index: 3,
                    node: 3,
                    mesh: 0,
                    ..MeshInstance::default()
                },
                MeshInstance {
                    source_node_index: 4,
                    node: 4,
                    mesh: 0,
                    ..MeshInstance::default()
                },
            ],
            materials: vec![
                material("body-material"),
                material("prop-material"),
                material(" duplicate-material "),
                material(" duplicate-material "),
                material(" duplicate-material "),
            ],
            ..SceneAssets::default()
        },
        ..Document::default()
    };
    animsmith_gltf::write::write(&doc, path).expect("writes multi-mesh fixture");
}

fn assert_hostile_text_is_escaped(text: &str) {
    assert!(
        !text.contains(HOSTILE_PRESENTATION_TEXT),
        "raw hostile text leaked:\n{text}"
    );
    assert!(
        !text.contains('\u{1b}'),
        "raw terminal escape leaked:\n{text}"
    );
    assert!(
        !text.contains('\u{2028}'),
        "raw line separator leaked:\n{text}"
    );
    assert!(
        !text.contains('\u{2029}'),
        "raw paragraph separator leaked:\n{text}"
    );
    assert!(
        !text.contains('\u{202e}'),
        "raw bidi override leaked:\n{text}"
    );
    assert!(
        text.contains("forged\\nline\\u{1b}[31m\\u{2028}\\u{2029}\\u{202e}"),
        "escaped form missing:\n{text}"
    );
}

fn write_json(path: &std::path::Path, value: &Value) {
    std::fs::write(
        path,
        serde_json::to_vec_pretty(value).expect("serializes JSON fixture"),
    )
    .expect("writes JSON fixture");
}

fn measurement_report(duration_s: f64) -> Value {
    json!({
        "schema_version": 7,
        "schema": OUTPUT_SCHEMA_ID,
        "command": "measure",
        "files": [{
            "path": "fixture.gltf",
            "input": {
                "sha256": "2c26b46b68ffc68ff99b453c1d30413413422d706483bfa0f98a5e886266e7ae",
                "bytes": 3
            },
            "rig": { "profile": "unknown" },
            "measurements": {
                "schema_version": 13,
                "schema": MEASUREMENTS_SCHEMA_ID,
                "clips": {
                    "walk": {
                        "duration_s": duration_s,
                        "frame_count": 31,
                        "animated_bones": [],
                        "bone_rotation_range_deg": {}
                    }
                },
                "mesh_definitions": [],
                "node_instances": [],
                "scenes": [],
                "material_resource_coverage": "unavailable",
                "material_definitions": [],
                "textures": [],
                "images": [],
                "skeleton_source_coverage": "unavailable",
                "skeleton_nodes": [],
                "skins": []
            }
        }]
    })
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

const EMPTY_ANIMATION_GLTF: &str = r#"{
  "asset": { "version": "2.0" },
  "nodes": [{ "name": "root" }],
  "animations": [{ "name": "empty", "samplers": [], "channels": [] }],
  "scenes": [{ "nodes": [0] }],
  "scene": 0
}"#;

#[test]
fn transform_summary_reports_a_loaded_clip_omitted_from_the_artifact() {
    let dir = unique_temp_dir("transform-empty-animation");
    let input = dir.path().join("empty-animation.gltf");
    let output_path = dir.path().join("transformed.glb");
    std::fs::write(&input, EMPTY_ANIMATION_GLTF).expect("writes empty animation fixture");

    let output = animsmith()
        .arg("transform")
        .arg(&input)
        .arg("-o")
        .arg(&output_path)
        .output()
        .expect("runs transform");

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&output)
    );
    let written = animsmith_gltf::load(&output_path).expect("loads transformed output");
    assert!(written.clips.is_empty(), "empty animation is not emitted");
    assert_eq!(
        stdout(&output),
        format!(
            "wrote {} (1 node(s), 0 clip(s), 0 mesh(es) / 0 position(s), 0 material(s)); dropped 1 clip(s) with no writable tracks\n",
            output_path.display()
        )
    );
}

#[test]
fn duplicate_loop_endpoint_cli_detects_trims_and_exposes_changed_contracts() {
    let dir = unique_temp_dir("duplicate-loop-endpoint");
    let input = dir.path().join("input.glb");
    let output_path = dir.path().join("open-cycle.glb");
    let second_output = dir.path().join("open-cycle-again.glb");
    let undeclared_output = dir.path().join("undeclared.glb");
    let config = dir.path().join("animsmith.toml");
    write_duplicate_loop_endpoint_glb(&input);
    std::fs::write(
        &config,
        "[clips.guard]\nloop = true\nduration_s = { value = 1.0, tolerance = 0.0 }\n",
    )
    .expect("writes loop contract");

    let undeclared = animsmith()
        .arg("transform")
        .arg(&input)
        .arg("-o")
        .arg(&undeclared_output)
        .arg("--drop-duplicate-loop-endpoint")
        .output()
        .expect("runs transform without a loop declaration");
    assert_eq!(undeclared.status.code(), Some(0));
    assert!(
        stdout(&undeclared).contains(
            "duplicate-loop-endpoint skipped 'guard': clip is not declared `loop = true` in config"
        ),
        "stdout:\n{}",
        stdout(&undeclared)
    );
    let undeclared_written =
        animsmith_gltf::load(&undeclared_output).expect("loads unchanged undeclared output");
    assert_eq!(undeclared_written.clips[0].duration_s, 1.0);
    assert_eq!(undeclared_written.clips[0].tracks[0].key_count(), 5);

    let lint = animsmith()
        .arg("--config")
        .arg(&config)
        .args([
            "lint",
            input.to_str().unwrap(),
            "--select",
            "duplicate-loop-endpoint",
        ])
        .output()
        .expect("runs duplicate endpoint check");
    assert_eq!(lint.status.code(), Some(0), "stderr:\n{}", stderr(&lint));
    assert!(
        stdout(&lint).contains("warning[duplicate-loop-endpoint]"),
        "stdout:\n{}",
        stdout(&lint)
    );

    let lint_json = animsmith()
        .arg("--config")
        .arg(&config)
        .args([
            "lint",
            input.to_str().unwrap(),
            "--select",
            "duplicate-loop-endpoint",
            "--format",
            "json",
        ])
        .output()
        .expect("serializes duplicate endpoint evidence");
    assert_eq!(lint_json.status.code(), Some(0));
    let lint_json: Value = serde_json::from_slice(&lint_json.stdout).expect("valid lint JSON");
    assert_output_schema_valid(&lint_json);
    assert_eq!(lint_json["schema_version"], 7);
    assert_eq!(lint_json["schema"], OUTPUT_SCHEMA_ID);
    assert_eq!(lint_json["files"][0]["measurements"]["schema_version"], 13);
    assert_eq!(
        lint_json["files"][0]["measurements"]["schema"],
        MEASUREMENTS_SCHEMA_ID
    );
    let duplicate_record = lint_json["files"][0]["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|record| record["check_id"] == "duplicate-loop-endpoint")
        .expect("duplicate endpoint check record");
    assert_eq!(duplicate_record["evaluation"], "complete");
    assert_eq!(
        duplicate_record["evaluated_scopes"][0]["code"],
        "duplicate_loop_endpoint"
    );
    assert_eq!(duplicate_record["findings"][0]["severity"], "warning");

    let contract_before = animsmith()
        .arg("--config")
        .arg(&config)
        .args([
            "lint",
            input.to_str().unwrap(),
            "--select",
            "loop-closure,duration-sanity",
        ])
        .output()
        .expect("checks contracts before trimming");
    assert_eq!(
        contract_before.status.code(),
        Some(0),
        "the candidate starts with clean #14/#15 evidence:\n{}",
        stdout(&contract_before)
    );

    let transformed = animsmith()
        .arg("--config")
        .arg(&config)
        .arg("transform")
        .arg(&input)
        .arg("-o")
        .arg(&output_path)
        .arg("--drop-duplicate-loop-endpoint")
        .output()
        .expect("runs duplicate endpoint transform");
    assert_eq!(
        transformed.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&transformed)
    );
    assert!(
        stdout(&transformed).contains(
            "dropped duplicate loop endpoint 'guard': 1 key(s) per track, duration 1.000000s -> 0.750000s (open cycle)"
        ),
        "stdout:\n{}",
        stdout(&transformed)
    );
    let written = animsmith_gltf::load(&output_path).expect("loads open-cycle output");
    assert_eq!(written.clips[0].duration_s, 0.75);
    assert_eq!(written.clips[0].tracks[0].times, vec![0.0, 0.25, 0.5, 0.75]);

    let duplicate_recheck = animsmith()
        .arg("--config")
        .arg(&config)
        .args([
            "lint",
            output_path.to_str().unwrap(),
            "--select",
            "duplicate-loop-endpoint",
        ])
        .output()
        .expect("reruns duplicate endpoint check");
    assert_eq!(
        duplicate_recheck.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&duplicate_recheck)
    );
    assert!(
        !stdout(&duplicate_recheck).contains("warning[duplicate-loop-endpoint]"),
        "stdout:\n{}",
        stdout(&duplicate_recheck)
    );

    // The transform intentionally changes the clip to #22's future
    // open/unique-cycle representation. The inclusive #14 closure contract
    // and the old #15 duration pin must therefore be rerun and reconciled.
    let contract_recheck = animsmith()
        .arg("--config")
        .arg(&config)
        .args([
            "lint",
            output_path.to_str().unwrap(),
            "--select",
            "loop-closure,duration-sanity",
        ])
        .output()
        .expect("reruns changed contracts");
    assert_eq!(contract_recheck.status.code(), Some(1));
    let contract_text = stdout(&contract_recheck);
    assert!(
        contract_text.contains(
            "error[loop-closure] clip 'guard' bone 'spine' @0.750s: loop does not close in position"
        ),
        "stdout:\n{contract_text}\nstderr:\n{}",
        stderr(&contract_recheck)
    );
    assert!(
        contract_text.contains(
            "error[duration-sanity] clip 'guard': measured duration 0.7500s disagrees with the declared 1.0000"
        ),
        "stdout:\n{contract_text}\nstderr:\n{}",
        stderr(&contract_recheck)
    );

    let idempotent = animsmith()
        .arg("--config")
        .arg(&config)
        .arg("transform")
        .arg(&output_path)
        .arg("-o")
        .arg(&second_output)
        .arg("--drop-duplicate-loop-endpoint")
        .output()
        .expect("reruns transform");
    assert_eq!(idempotent.status.code(), Some(0));
    assert!(
        stdout(&idempotent).contains(
            "duplicate-loop-endpoint skipped 'guard': no mechanically removable repeated endpoint"
        ),
        "stdout:\n{}",
        stdout(&idempotent)
    );
}

#[test]
fn fix_rejects_unknown_repair_ids() {
    // Nonexistent input on purpose: flag validation must produce exit 2
    // regardless of file state, so no fixture is needed.
    let output = animsmith()
        .args(["fix", "clip.glb", "--dry-run", "--repair", "no-such-repair"])
        .output()
        .expect("runs animsmith");

    assert_eq!(
        output.status.code(),
        Some(2),
        "stdout:\n{}",
        stdout(&output)
    );
    assert!(
        stderr(&output).contains("quat-flip"),
        "stderr should list valid repair ids:\n{}",
        stderr(&output)
    );
    assert!(
        stderr(&output).contains("quat-norm"),
        "stderr should list valid repair ids:\n{}",
        stderr(&output)
    );
}

#[test]
fn fix_rejects_removed_group_flags() {
    // `--group` and `--list-repairs` were removed in the pre-publish
    // contract trim; wrapper scripts still passing them must fail
    // loudly, not silently change meaning.
    for removed in [&["--group", "default"][..], &["--list-repairs"][..]] {
        let output = animsmith()
            .args(["fix", "clip.glb"])
            .args(removed)
            .output()
            .expect("runs animsmith");

        assert_eq!(
            output.status.code(),
            Some(2),
            "{removed:?} must be rejected; stdout:\n{}",
            stdout(&output)
        );
        assert!(
            stderr(&output).contains("unexpected argument"),
            "stderr:\n{}",
            stderr(&output)
        );
    }
}

#[test]
fn fix_requires_an_explicit_write_target() {
    let output = animsmith()
        .args(["fix", "clip.glb"])
        .output()
        .expect("runs animsmith");

    assert_eq!(
        output.status.code(),
        Some(2),
        "stdout:\n{}",
        stdout(&output)
    );
    assert!(
        stderr(&output).contains("fix requires --output <PATH> or --in-place"),
        "stderr:\n{}",
        stderr(&output)
    );
}

#[test]
fn fix_dry_run_reports_without_writing() {
    let dir = unique_temp_dir("fix-dry-run");
    let input = dir.path().join("dirty.glb");
    write_flipped_glb(&input);
    let before = std::fs::read(&input).expect("reads input");

    let output = animsmith()
        .args([
            "fix",
            input.to_str().expect("utf-8 input path"),
            "--dry-run",
            "--repair",
            "quat-flip",
        ])
        .output()
        .expect("runs animsmith");

    // Pending repairs are findings: dry run exits 1 (the check mode),
    // and the input is untouched.
    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    assert!(
        stdout(&output).contains("would be fixed"),
        "stdout:\n{}",
        stdout(&output)
    );
    assert_eq!(before, std::fs::read(&input).expect("reads input"));
}

#[test]
fn fix_dry_run_dedupes_duplicate_repairs() {
    let dir = unique_temp_dir("fix-dry-run-dedup");
    let input = dir.path().join("dirty.glb");
    write_flipped_glb(&input);

    let output = animsmith()
        .args([
            "fix",
            input.to_str().expect("utf-8 input path"),
            "--dry-run",
            "--repair",
            "quat-flip,quat-flip",
        ])
        .output()
        .expect("runs animsmith");

    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    let out = stdout(&output);
    assert!(
        out.contains("2 key(s) would be fixed across 1 track(s)"),
        "stdout:\n{out}"
    );
    assert_eq!(
        out.matches("key(s) would be fixed across").count(),
        1,
        "duplicate repairs should be reported once:\n{out}"
    );
}

#[test]
fn fix_dry_run_dedupes_non_adjacent_distinct_repairs_without_writing() {
    let dir = unique_temp_dir("fix-dry-run-compose");
    let input = dir.path().join("dirty.glb");
    write_distinct_repair_glb(&input);
    let before = std::fs::read(&input).expect("reads input");

    let output = animsmith()
        .args([
            "fix",
            input.to_str().expect("utf-8 input path"),
            "--dry-run",
            "--repair",
            "quat-norm,quat-flip,quat-norm",
        ])
        .output()
        .expect("runs animsmith");

    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    let out = stdout(&output);
    assert!(out.contains("would fix[quat-norm]"), "stdout:\n{out}");
    assert!(out.contains("would fix[quat-flip]"), "stdout:\n{out}");
    assert_eq!(
        out.matches("would fix[quat-norm]").count(),
        1,
        "non-adjacent duplicate repairs should be reported once:\n{out}"
    );
    assert_eq!(before, std::fs::read(&input).expect("reads input"));
    assert_eq!(
        FixSession::inspect(&input, GltfRepair::QuatNorm)
            .expect("inspects dirty input")
            .total_fixed(),
        1
    );
    assert_eq!(
        FixSession::inspect(&input, GltfRepair::QuatFlip)
            .expect("inspects dirty input")
            .total_fixed(),
        2
    );
}

#[test]
fn fix_dry_run_labels_each_repair_with_its_action() {
    // The distinct-repair fixture needs both a quat-norm (non-unit key)
    // and a quat-flip (hemisphere) repair on the same bone, so the report
    // prints one per-track line per repair. Each line must carry its own
    // action suffix; a swapped or stale Repair::action() would pair the
    // wrong verb with the id.
    let dir = unique_temp_dir("fix-action-labels");
    let input = dir.path().join("dirty.glb");
    write_distinct_repair_glb(&input);

    let output = animsmith()
        .args([
            "fix",
            input.to_str().expect("utf-8 input path"),
            "--dry-run",
            "--repair",
            "quat-norm,quat-flip",
        ])
        .output()
        .expect("runs animsmith");

    let out = stdout(&output);
    let norm_line = out
        .lines()
        .find(|l| l.contains("would fix[quat-norm]"))
        .unwrap_or_else(|| panic!("no quat-norm track line:\n{out}"));
    assert!(
        norm_line.contains("unit-normalized"),
        "quat-norm line must report unit-normalized: {norm_line}"
    );
    let flip_line = out
        .lines()
        .find(|l| l.contains("would fix[quat-flip]"))
        .unwrap_or_else(|| panic!("no quat-flip track line:\n{out}"));
    assert!(
        flip_line.contains("hemisphere-normalized"),
        "quat-flip line must report hemisphere-normalized: {flip_line}"
    );
}

#[test]
fn fix_dry_run_on_clean_input_exits_zero() {
    let dir = unique_temp_dir("fix-dry-run-clean");
    let input = dir.path().join("clean.glb");
    write_clean_glb(&input);

    let output = animsmith()
        .args([
            "fix",
            input.to_str().expect("utf-8 input path"),
            "--dry-run",
        ])
        .output()
        .expect("runs animsmith");

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    assert!(
        stdout(&output).contains("0 key(s) would be fixed"),
        "stdout:\n{}",
        stdout(&output)
    );
}

#[test]
fn fix_dry_run_skipped_tracks_do_not_fail_the_check() {
    // A .gltf written by the writer embeds its buffer as a data URI,
    // which fix cannot patch: the track is reported as skipped. The
    // dry-run exit code reflects repairs fix would PERFORM — skipped
    // tracks print loudly but exit 0; detection-only gating is lint's
    // job (the quat-flip check).
    let dir = unique_temp_dir("fix-dry-run-skip");
    let input = dir.path().join("dirty.gltf");
    animsmith_gltf::write::write(&sway_doc(true), &input).expect("writes gltf fixture");

    let output = animsmith()
        .args([
            "fix",
            input.to_str().expect("utf-8 input path"),
            "--dry-run",
        ])
        .output()
        .expect("runs animsmith");

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    assert!(
        stdout(&output).contains("skipped[quat-flip]"),
        "stdout:\n{}",
        stdout(&output)
    );
}

#[test]
fn fix_dry_run_conflicts_with_write_targets() {
    for write_flag in [&["-o", "out.glb"][..], &["--in-place"][..]] {
        let output = animsmith()
            .args(["fix", "clip.glb", "--dry-run"])
            .args(write_flag)
            .output()
            .expect("runs animsmith");

        assert_eq!(
            output.status.code(),
            Some(2),
            "--dry-run with {write_flag:?} must be rejected; stdout:\n{}",
            stdout(&output)
        );
        assert!(
            stderr(&output).contains("--dry-run"),
            "stderr:\n{}",
            stderr(&output)
        );
    }
}

#[test]
fn fix_default_repairs_write_output() {
    let dir = unique_temp_dir("fix-output");
    let input = dir.path().join("dirty.glb");
    let output_path = dir.path().join("fixed.glb");
    write_flipped_glb(&input);

    let output = animsmith()
        .args([
            "fix",
            input.to_str().expect("utf-8 input path"),
            "--output",
            output_path.to_str().expect("utf-8 output path"),
        ])
        .output()
        .expect("runs animsmith");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    assert!(output_path.exists());

    // Analytic oracle: hemisphere normalization must restore exactly
    // the un-flipped source sequence (negation is a lossless bit flip).
    let fixed = animsmith_gltf::load(&output_path).expect("loads fixed output");
    let TrackValues::Quats(quats) = &fixed.clips[0].tracks[0].values else {
        panic!("rotation track expected");
    };
    let expected = sway_quats(false);
    for (got, want) in quats.iter().zip(&expected) {
        assert_eq!(got.to_array(), want.to_array());
    }
}

#[test]
fn fix_write_composes_distinct_repairs() {
    let dir = unique_temp_dir("fix-output-compose");
    let input = dir.path().join("dirty.glb");
    let output_path = dir.path().join("fixed.glb");
    write_distinct_repair_glb(&input);

    assert_eq!(
        FixSession::inspect(&input, GltfRepair::QuatNorm)
            .expect("inspects dirty input")
            .total_fixed(),
        1
    );
    assert_eq!(
        FixSession::inspect(&input, GltfRepair::QuatFlip)
            .expect("inspects dirty input")
            .total_fixed(),
        2
    );

    let output = animsmith()
        .args([
            "fix",
            input.to_str().expect("utf-8 input path"),
            "--output",
            output_path.to_str().expect("utf-8 output path"),
            "--repair",
            "quat-norm,quat-flip",
        ])
        .output()
        .expect("runs animsmith");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    let out = stdout(&output);
    assert!(out.contains("fixed[quat-norm]"), "stdout:\n{out}");
    assert!(out.contains("fixed[quat-flip]"), "stdout:\n{out}");

    assert_eq!(
        FixSession::inspect(&output_path, GltfRepair::QuatNorm)
            .expect("inspects fixed output")
            .total_fixed(),
        0
    );
    assert_eq!(
        FixSession::inspect(&output_path, GltfRepair::QuatFlip)
            .expect("inspects fixed output")
            .total_fixed(),
        0
    );

    let fixed = animsmith_gltf::load(&output_path).expect("loads fixed output");
    let TrackValues::Quats(quats) = &fixed.clips[0].tracks[0].values else {
        panic!("rotation track expected");
    };
    for (got, want) in quats.iter().zip(sway_quats(false)) {
        assert!(
            got.dot(want).abs() > 1.0 - 1e-5,
            "composed repairs must preserve the represented rotation"
        );
    }
}

#[test]
fn fix_write_dedupes_duplicate_repairs() {
    let dir = unique_temp_dir("fix-output-dedup");
    let input = dir.path().join("dirty.glb");
    let output_path = dir.path().join("fixed.glb");
    write_flipped_glb(&input);

    let output = animsmith()
        .args([
            "fix",
            input.to_str().expect("utf-8 input path"),
            "--output",
            output_path.to_str().expect("utf-8 output path"),
            "--repair",
            "quat-flip,quat-flip",
        ])
        .output()
        .expect("runs animsmith");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    let out = stdout(&output);
    assert_eq!(
        out.matches("key(s) fixed across").count(),
        1,
        "duplicate repairs should be reported once:\n{out}"
    );
    assert_eq!(
        FixSession::inspect(&output_path, GltfRepair::QuatFlip)
            .expect("inspects fixed output")
            .total_fixed(),
        0
    );
}

#[test]
fn fix_in_place_writes_selected_repair() {
    let dir = unique_temp_dir("fix-in-place");
    let input = dir.path().join("dirty.glb");
    write_flipped_glb(&input);
    assert_eq!(
        FixSession::inspect(&input, GltfRepair::QuatFlip)
            .expect("inspects dirty input")
            .total_fixed(),
        2
    );

    let output = animsmith()
        .args([
            "fix",
            input.to_str().expect("utf-8 input path"),
            "--in-place",
            "--repair",
            "quat-flip",
        ])
        .output()
        .expect("runs animsmith");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    assert_eq!(
        FixSession::inspect(&input, GltfRepair::QuatFlip)
            .expect("inspects fixed input")
            .total_fixed(),
        0
    );
}

#[test]
fn help_matches_compiled_feature_set() {
    let output = animsmith().arg("--help").output().expect("runs animsmith");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("inspect"));
    assert!(out.contains("measure"));
    assert!(out.contains("lint"));
    assert!(out.contains("transform"));
    assert!(out.contains("fix"));
    assert!(out.contains("scale"));
    assert!(out.contains("diff"));

    // One-line summaries come from the doc comments (clap derives
    // `about` from the first line); pin them so description drift is
    // visible.
    assert!(out.contains("Repair safe mechanical glTF/GLB defects"));
    assert!(out.contains("Apply mechanical clip transforms"));
    assert!(out.contains("Compare animation measurements"));
    // `scale` is unconditional: it is the minimal binary's only
    // evidence-emitting producer, so a feature gate on it would silently
    // remove that surface.
    assert!(out.contains("Rewrite declared linear scale and publish versioned evidence"));

    assert_eq!(out.contains("\n  convert "), cfg!(feature = "fbx"), "{out}");
    assert_eq!(
        out.contains("\n  report "),
        cfg!(feature = "report"),
        "{out}"
    );

    let transform = animsmith()
        .args(["transform", "--help"])
        .output()
        .expect("runs transform help");
    assert!(
        transform.status.success(),
        "stderr:\n{}",
        stderr(&transform)
    );
    assert!(
        stdout(&transform).contains("--prune-constant-tracks"),
        "{}",
        stdout(&transform)
    );

    let diff = animsmith()
        .args(["diff", "--help"])
        .output()
        .expect("runs diff help");
    assert!(diff.status.success(), "stderr:\n{}", stderr(&diff));
    let out = stdout(&diff);
    assert!(out.contains("output-v7"), "{out}");
    assert!(out.contains("measurements-v13"), "{out}");
    assert!(!out.contains("v5"), "{out}");
}

#[test]
fn scale_help_requires_every_factor_and_selector_of_appendix_d7() {
    let root = animsmith()
        .args(["scale", "--help"])
        .output()
        .expect("runs scale help");
    assert!(root.status.success(), "stderr:\n{}", stderr(&root));
    let out = stdout(&root);
    assert!(out.contains("whole-document"), "{out}");
    assert!(out.contains("rest-bind"), "{out}");

    let whole = animsmith()
        .args(["scale", "whole-document", "--help"])
        .output()
        .expect("runs whole-document help");
    assert!(whole.status.success(), "stderr:\n{}", stderr(&whole));
    let out = stdout(&whole);
    // Required, not optional: clap renders an optional argument in square
    // brackets, so `<FACTOR>` bare is what pins "no inferred factor".
    assert!(out.contains("--factor <FACTOR>"), "{out}");
    assert!(out.contains("--evidence <EVIDENCE>"), "{out}");
    assert!(out.contains("--format <FORMAT>"), "{out}");
    assert!(out.contains("[default: text]"), "{out}");
    assert!(out.contains("[possible values: text, json]"), "{out}");
    assert!(out.contains("POSITION morph-target deltas"), "{out}");
    assert!(
        !out.contains("--in-place") && !out.contains("--tolerance"),
        "there is no in-place mode and no per-run tolerance flag: {out}"
    );

    let rest_bind = animsmith()
        .args(["scale", "rest-bind", "--help"])
        .output()
        .expect("runs rest-bind help");
    assert!(
        rest_bind.status.success(),
        "stderr:\n{}",
        stderr(&rest_bind)
    );
    let out = stdout(&rest_bind);
    assert!(
        out.contains("--source-skin-index <SOURCE_SKIN_INDEX>"),
        "{out}"
    );
    assert!(
        out.contains("--source-root-node-index <SOURCE_ROOT_NODE_INDEX>"),
        "{out}"
    );
    assert!(out.contains("--expected-factor <EXPECTED_FACTOR>"), "{out}");
    assert!(out.contains("--evidence <EVIDENCE>"), "{out}");

    // Omitting any required selector is a usage error, not a default.
    let missing = animsmith()
        .args(["scale", "rest-bind", "rig.glb", "-o", "out.glb"])
        .output()
        .expect("runs rest-bind without selectors");
    assert_eq!(missing.status.code(), Some(2), "{}", stderr(&missing));
    for required in [
        "--source-skin-index",
        "--source-root-node-index",
        "--expected-factor",
        "--evidence",
    ] {
        assert!(
            stderr(&missing).contains(required),
            "{required} is not reported as required:\n{}",
            stderr(&missing)
        );
    }
}

#[cfg(feature = "fbx")]
#[test]
fn convert_help_exposes_texture_recipe_static_bake_and_machine_evidence_contract() {
    let output = animsmith()
        .args(["convert", "--help"])
        .output()
        .expect("runs animsmith");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("--material-texture-recipe <PATH>"), "{out}");
    assert!(out.contains("--bake-static-mesh-transforms"), "{out}");
    assert!(out.contains("--format <FORMAT>"), "{out}");
    assert!(out.contains("[default: text]"), "{out}");
    assert!(out.contains("[possible values: text, json]"), "{out}");

    let conflict = animsmith()
        .args([
            "convert",
            "input.glb",
            "-o",
            "output.glb",
            "--animation-only",
            "--bake-static-mesh-transforms",
        ])
        .output()
        .expect("runs animsmith");
    assert_eq!(conflict.status.code(), Some(2));
    assert!(
        stderr(&conflict).contains("cannot be used with"),
        "{}",
        stderr(&conflict)
    );

    let recipe_conflict = animsmith()
        .args([
            "convert",
            "input.glb",
            "-o",
            "output.glb",
            "--animation-only",
            "--material-texture-recipe",
            "recipe.toml",
        ])
        .output()
        .expect("runs animsmith");
    assert_eq!(recipe_conflict.status.code(), Some(2));
    assert!(
        stderr(&recipe_conflict).contains("cannot be used with"),
        "{}",
        stderr(&recipe_conflict)
    );
}

#[test]
fn fix_help_lists_repair_possible_values() {
    let output = animsmith()
        .args(["fix", "--help"])
        .output()
        .expect("runs animsmith");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let out = stdout(&output);
    assert!(
        out.contains("[possible values: quat-norm, quat-flip]"),
        "stdout:\n{out}"
    );
}

#[test]
fn version_uses_the_composed_build_version_at_the_cli_boundary() {
    let output = animsmith()
        .arg("--version")
        .output()
        .expect("runs animsmith");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    assert!(stderr(&output).is_empty(), "stderr:\n{}", stderr(&output));
    let out = stdout(&output);
    assert!(
        out.starts_with(concat!("animsmith ", env!("CARGO_PKG_VERSION"))),
        "{out}"
    );
}

#[test]
fn measure_json_uses_versioned_envelope() {
    let output = animsmith()
        .args([
            "measure",
            fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
            "--format",
            "json",
        ])
        .output()
        .expect("runs animsmith");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    assert_output_schema_valid(&json);
    assert_eq!(json["schema_version"], 7);
    assert_eq!(json["schema"], OUTPUT_SCHEMA_ID);
    assert_eq!(json["tool"]["name"], "animsmith");
    assert_eq!(json["tool"]["version"], env!("CARGO_PKG_VERSION"));
    assert!(json["tool"]["source"].is_object());
    let expected_revision = option_env!("ANIMSMITH_GIT_REVISION");
    assert_eq!(
        json["tool"]["source"]["revision"].as_str(),
        expected_revision
    );
    let expected_dirty =
        option_env!("ANIMSMITH_GIT_DIRTY").and_then(|value| value.parse::<bool>().ok());
    assert_eq!(json["tool"]["source"]["dirty"].as_bool(), expected_dirty);
    if let Some(revision) = expected_revision {
        assert_eq!(revision.len(), 40, "full source revision: {revision}");
        assert!(revision.bytes().all(|byte| byte.is_ascii_hexdigit()));
    }
    assert_eq!(json["command"], "measure");
    assert_eq!(json["summary"]["files"], 1);

    let files = json["files"].as_array().expect("files array");
    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["rig"]["profile"], "unknown");
    assert!(files[0]["checks"].is_null());
    assert_eq!(files[0]["measurements"]["schema_version"], 13);
    assert_eq!(files[0]["measurements"]["schema"], MEASUREMENTS_SCHEMA_ID);
    assert!(files[0]["measurements"]["clips"]["walk"]["duration_s"].is_number());
    let loop_bones = files[0]["measurements"]["clips"]["walk"]["loop_continuity"]["bones"]
        .as_array()
        .expect("measurable clip exposes per-bone loop continuity");
    assert_eq!(loop_bones.len(), 3);
    for (bone, (index, name)) in loop_bones
        .iter()
        .zip([(0, "root"), (1, "hips"), (2, "foot")])
    {
        assert_eq!(bone["bone_index"], index);
        assert_eq!(bone["bone_name"], name);
        assert!(bone["position_delta_m"].is_number());
        assert!(bone["rotation_delta_deg"].is_number());
        assert!(bone["seam_velocity_delta_mps"].is_number());
        assert!(bone["seam_angular_velocity_delta_degps"].is_number());
    }
}

#[test]
fn angular_loop_seam_is_versioned_and_configurable_at_the_cli_boundary() {
    let dir = unique_temp_dir("angular-loop-seam");
    let input = dir.path().join("angular-cusp.glb");
    write_angular_cusp_glb(&input);
    let config = write_config(
        dir.path(),
        "angular-loop.toml",
        "[clips.angular_cusp]\nloop = true\n",
    );

    let baseline = animsmith()
        .arg("--config")
        .arg(&config)
        .arg("lint")
        .arg(&input)
        .args(["--select", "loop-seam-rot", "--format", "json"])
        .output()
        .expect("runs angular seam lint");
    assert_eq!(
        baseline.status.code(),
        Some(1),
        "stderr:\n{}",
        stderr(&baseline)
    );
    let baseline: Value = serde_json::from_slice(&baseline.stdout).expect("valid lint JSON");
    assert_output_schema_valid(&baseline);
    assert_eq!(baseline["files"][0]["measurements"]["schema_version"], 13);
    assert_eq!(
        baseline["files"][0]["measurements"]["schema"],
        MEASUREMENTS_SCHEMA_ID
    );
    let mut missing_angular_evidence = baseline.clone();
    missing_angular_evidence["files"][0]["measurements"]["clips"]["angular_cusp"]
        ["loop_continuity"]["bones"][0]
        .as_object_mut()
        .expect("bone measurement object")
        .remove("seam_angular_velocity_delta_degps");
    assert!(
        !output_validator().is_valid(&missing_angular_evidence),
        "measurements-v13 requires angular seam evidence in every loop-continuity row"
    );
    let bones =
        baseline["files"][0]["measurements"]["clips"]["angular_cusp"]["loop_continuity"]["bones"]
            .as_array()
            .expect("per-bone loop continuity");
    assert_eq!(bones.len(), 2);
    for (bone, (index, name)) in bones.iter().zip([(0, "root"), (1, "spine")]) {
        assert_eq!(bone["bone_index"], index);
        assert_eq!(bone["bone_name"], name);
        assert_eq!(bone["seam_velocity_delta_mps"], 0.0);
    }
    let spine = &bones[1];
    assert!(
        spine["seam_angular_velocity_delta_degps"]
            .as_f64()
            .expect("angular seam metric")
            > 100.0,
        "expected analytic angular cusp: {spine:#}"
    );
    let check = baseline["files"][0]["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|check| check["check_id"] == "loop-seam-rot")
        .expect("angular seam check");
    assert_eq!(check["applicability"], "applicable");
    assert_eq!(check["evaluation"], "complete");
    assert_eq!(check["evaluated_scopes"][0]["code"], "loop_seam_rotation");
    assert_eq!(check["findings"][0]["severity"], "error");
    assert_eq!(check["findings"][0]["bone"], "spine");
    assert_eq!(
        check["findings"][0]["measured"],
        spine["seam_angular_velocity_delta_degps"]
    );
    assert_eq!(check["findings"][0]["expected"], 5.0);

    let relaxed = write_config(
        dir.path(),
        "relaxed-angular-loop.toml",
        "[clips.angular_cusp]\nloop = true\n\n[checks.loop-seam-rot]\nmax_angular_velocity_delta_degps = 200.0\n",
    );
    let relaxed = animsmith()
        .arg("--config")
        .arg(&relaxed)
        .arg("lint")
        .arg(&input)
        .args(["--select", "loop-seam-rot", "--format", "json"])
        .output()
        .expect("runs configured angular seam lint");
    assert_eq!(
        relaxed.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&relaxed)
    );
    let relaxed: Value = serde_json::from_slice(&relaxed.stdout).expect("valid lint JSON");
    let check = relaxed["files"][0]["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|check| check["check_id"] == "loop-seam-rot")
        .expect("angular seam check");
    assert_eq!(check["findings"], json!([]));
    assert_eq!(check["evaluation"], "complete");
}

#[test]
fn measure_text_escapes_controls_in_clip_and_mesh_names() {
    let dir = unique_temp_dir("measure-text-controls");
    let input = dir.path().join("hostile.glb");
    write_hostile_glb(&input, HOSTILE_PRESENTATION_TEXT, false);

    let output = animsmith()
        .arg("measure")
        .arg(&input)
        .args(["--format", "text"])
        .output()
        .expect("runs animsmith");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&output)
    );
    let text = stdout(&output);
    assert_hostile_text_is_escaped(&text);
    assert!(text.contains("Δω=0.00°/s"), "angular aggregate: {text}");
    assert_eq!(
        text.matches("\\n").count(),
        5,
        "clip, mesh, node instance, and two source nodes: {text}"
    );
    assert_eq!(
        text.matches("\\u{1b}").count(),
        5,
        "clip, mesh, node instance, and two source nodes: {text}"
    );
    assert_eq!(
        text.matches("\\u{2028}").count(),
        5,
        "clip, mesh, node instance, and two source nodes: {text}"
    );
    assert_eq!(
        text.matches("\\u{202e}").count(),
        5,
        "clip, mesh, node instance, and two source nodes: {text}"
    );
}

#[test]
fn inspect_fix_and_transform_escape_asset_derived_text() {
    let dir = unique_temp_dir("command-text-controls");
    let clean = dir.path().join("hostile-clean.glb");
    let flipped = dir.path().join("hostile-flipped.glb");
    let transformed = dir.path().join("transformed.glb");
    write_hostile_glb(&clean, HOSTILE_PRESENTATION_TEXT, false);
    write_hostile_glb(&flipped, HOSTILE_PRESENTATION_TEXT, true);

    let inspect = animsmith()
        .arg("inspect")
        .arg(&clean)
        .output()
        .expect("runs inspect");
    assert_eq!(inspect.status.code(), Some(0), "{}", stderr(&inspect));
    assert_hostile_text_is_escaped(&stdout(&inspect));

    let transform = animsmith()
        .arg("transform")
        .arg(&clean)
        .args(["--hold-extend", "0.25", "--output"])
        .arg(&transformed)
        .output()
        .expect("runs transform");
    assert_eq!(transform.status.code(), Some(0), "{}", stderr(&transform));
    assert_hostile_text_is_escaped(&stdout(&transform));

    let fix = animsmith()
        .arg("fix")
        .arg(&flipped)
        .args(["--dry-run", "--repair", "quat-flip"])
        .output()
        .expect("runs fix");
    assert_eq!(fix.status.code(), Some(1), "{}", stderr(&fix));
    assert_hostile_text_is_escaped(&stdout(&fix));
}

#[cfg(unix)]
#[test]
fn measure_text_escapes_controls_in_the_input_path() {
    let dir = unique_temp_dir("measure-text-path-controls");
    let hostile_name = "asset\nforged\u{1b}[31m.gltf";
    let input = dir.path().join(hostile_name);
    std::fs::copy(fixture("rig.gltf"), &input).expect("copies self-contained glTF fixture");

    let output = animsmith()
        .arg("measure")
        .arg(&input)
        .args(["--format", "text"])
        .output()
        .expect("runs animsmith");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&output)
    );
    let text = stdout(&output);
    assert!(
        !text.contains(hostile_name),
        "raw path controls leaked:\n{text}"
    );
    assert!(text.contains("asset\\nforged\\u{1b}[31m.gltf"), "{text}");
}

#[cfg(unix)]
#[test]
fn measure_text_renderer_escapes_hostile_asset_text_and_input_path() {
    let dir = unique_temp_dir("measure-renderer-controls");
    let hostile_path_text = "path\nasset\u{1b}[32m\u{2028}\u{2029}\u{2066}";
    let input = dir.path().join(format!("asset-{hostile_path_text}.glb"));
    write_hostile_glb(&input, HOSTILE_PRESENTATION_TEXT, false);

    let output = animsmith()
        .arg("measure")
        .arg(&input)
        .args(["--format", "text"])
        .output()
        .expect("runs measure");
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));

    let text = stdout(&output);
    assert_hostile_text_is_escaped(&text);
    assert!(
        !text.contains(hostile_path_text),
        "raw path leaked:\n{text}"
    );
    let escaped_path = "path\\nasset\\u{1b}[32m\\u{2028}\\u{2029}\\u{2066}";
    let escaped_asset = "forged\\nline\\u{1b}[31m\\u{2028}\\u{2029}\\u{202e}";
    assert_eq!(
        text.matches(escaped_path).count(),
        1,
        "input path should be escaped once:\n{text}"
    );
    assert_eq!(
        text.matches(escaped_asset).count(),
        5,
        "clip, mesh-definition, node-instance, and source-node names should each be escaped:\n{text}"
    );
    assert!(
        text.lines()
            .next()
            .is_some_and(|line| line.contains(escaped_path)),
        "escaped path should remain the heading:\n{text}"
    );
}

#[cfg(unix)]
#[test]
fn command_text_escapes_output_and_operator_error_paths() {
    let dir = unique_temp_dir("command-path-controls");
    let input = dir.path().join("clean.glb");
    let flipped = dir.path().join("flipped.glb");
    write_clean_glb(&input);
    write_flipped_glb(&flipped);
    let hostile_output = dir.path().join(format!("{HOSTILE_PRESENTATION_TEXT}.glb"));

    let transform = animsmith()
        .arg("transform")
        .arg(&input)
        .args(["--hold-extend", "0.25", "--output"])
        .arg(&hostile_output)
        .output()
        .expect("runs transform");
    assert_eq!(transform.status.code(), Some(0), "{}", stderr(&transform));
    assert_hostile_text_is_escaped(&stdout(&transform));

    let hostile_fix_output = dir
        .path()
        .join(format!("fixed-{HOSTILE_PRESENTATION_TEXT}.glb"));
    let fix = animsmith()
        .arg("fix")
        .arg(&flipped)
        .args(["--repair", "quat-flip", "--output"])
        .arg(&hostile_fix_output)
        .output()
        .expect("runs fix");
    assert_eq!(fix.status.code(), Some(0), "{}", stderr(&fix));
    assert_hostile_text_is_escaped(&stdout(&fix));

    let missing = dir
        .path()
        .join(format!("missing-{HOSTILE_PRESENTATION_TEXT}.glb"));
    let inspect = animsmith()
        .arg("inspect")
        .arg(&missing)
        .output()
        .expect("runs inspect");
    assert_eq!(inspect.status.code(), Some(2));
    assert_hostile_text_is_escaped(&stderr(&inspect));
}

#[cfg(all(feature = "report", unix))]
#[test]
fn report_text_escapes_its_output_path() {
    let dir = unique_temp_dir("report-path-controls");
    let output_path = dir
        .path()
        .join(format!("report-{HOSTILE_PRESENTATION_TEXT}.html"));
    let output = animsmith()
        .arg("report")
        .arg(fixture("rig.gltf"))
        .arg("--output")
        .arg(&output_path)
        .output()
        .expect("runs report");

    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_hostile_text_is_escaped(&stdout(&output));
    assert!(output_path.is_file(), "report output was not written");
}

#[test]
fn embedded_contract_types_emit_the_published_v7_envelope() {
    let doc = Document::default();
    let config = animsmith_core::Config::default();
    let roles = animsmith_core::ResolvedRoles::default();
    let grids = animsmith_core::MetricGrids::new(&doc);
    let ctx = animsmith_core::CheckCtx::new(&grids, &roles, &config);
    let checks = animsmith_core::evaluate_checks(
        &ctx,
        &animsmith_core::all_checks(),
        animsmith_core::CheckSelection::All,
    )
    .expect("built-in catalog evaluates");
    let file = animsmith_core::LintFileReport::new(
        "embedded.glb",
        embedded_input_identity(),
        animsmith_core::RigInfo::from_resolved(&doc, &roles)
            .expect("roles were resolved from this document"),
        checks,
        animsmith_core::MeasurementContract::new(
            animsmith_core::measure::measure_document(&grids, &roles, &config),
            animsmith_core::measure::measure_assets(&doc),
        )
        .expect("measured evidence is finite"),
    );
    let envelope = animsmith_core::LintEnvelope::new(
        animsmith_core::ToolInfo::animsmith(animsmith_core::ToolSource::new(None, None)),
        vec![file],
    );

    let json = serde_json::to_value(envelope).expect("embedded envelope serializes");
    assert_output_schema_valid(&json);
    assert_eq!(json["schema"], animsmith_core::OUTPUT_SCHEMA_ID);
    assert_eq!(
        json["files"][0]["measurements"]["schema"],
        animsmith_core::MEASUREMENTS_SCHEMA_ID
    );
}

#[test]
fn output_schema_rejects_every_empty_custom_check_identifier() {
    let check = animsmith_core::CheckEvaluation::evaluated(
        "custom",
        animsmith_core::CheckOutput::from_coverage(
            Vec::new(),
            vec![animsmith_core::EvaluationScope::new(
                animsmith_core::EvaluationScopeCode::custom("test:complete"),
            )],
            vec![
                animsmith_core::CoverageGap::new(
                    animsmith_core::CoverageGapCode::custom("test:gap"),
                    "missing evidence",
                )
                .scope(animsmith_core::EvaluationScope::new(
                    animsmith_core::EvaluationScopeCode::custom("test:missing"),
                )),
            ],
        ),
    )
    .expect("nonempty custom identifiers are valid");
    let doc = Document::default();
    let roles = animsmith_core::ResolvedRoles::default();
    let envelope = animsmith_core::LintEnvelope::new(
        animsmith_core::ToolInfo::animsmith(animsmith_core::ToolSource::new(None, None)),
        vec![animsmith_core::LintFileReport::new(
            "embedded.glb",
            embedded_input_identity(),
            animsmith_core::RigInfo::from_resolved(&doc, &roles)
                .expect("empty roles match an empty document"),
            vec![check],
            animsmith_core::MeasurementContract::new(
                BTreeMap::new(),
                animsmith_core::measure::AssetMeasurements::default(),
            )
            .expect("empty measurements are valid"),
        )],
    );
    let valid = serde_json::to_value(envelope).expect("embedded envelope serializes");
    assert_output_schema_valid(&valid);

    for pointer in [
        "/files/0/checks/0/check_id",
        "/files/0/checks/0/evaluated_scopes/0/code",
        "/files/0/checks/0/gaps/0/code",
        "/files/0/checks/0/gaps/0/scope/code",
    ] {
        let mut invalid = valid.clone();
        *invalid.pointer_mut(pointer).expect("fixture path exists") = json!("");
        assert!(
            !output_validator().is_valid(&invalid),
            "schema accepted an empty identifier at {pointer}"
        );
    }
}

#[test]
fn lint_json_uses_versioned_envelope() {
    let output = animsmith()
        .args([
            "lint",
            fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
            "--format",
            "json",
        ])
        .output()
        .expect("runs animsmith");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    assert_eq!(json["schema_version"], 7);
    assert_eq!(json["schema"], OUTPUT_SCHEMA_ID);
    assert_eq!(json["tool"]["name"], "animsmith");
    assert_eq!(json["tool"]["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(json["command"], "lint");
    assert_eq!(json["summary"]["files"], 1);
    assert!(json["files"][0]["checks"].is_array());
    assert_eq!(json["files"][0]["measurements"]["schema_version"], 13);
    assert_eq!(
        json["files"][0]["measurements"]["schema"],
        MEASUREMENTS_SCHEMA_ID
    );
    assert!(json["files"][0]["measurements"]["clips"]["walk"]["duration_s"].is_number());
    let actual_ids: BTreeSet<_> = json["files"][0]["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .map(|check| check["check_id"].as_str().expect("check id"))
        .collect();
    assert_eq!(actual_ids, EXPECTED_CHECK_IDS.into_iter().collect());
    let constant_nonunit = json["files"][0]["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|check| check["check_id"] == "constant-nonunit-scale")
        .expect("opt-in scale-channel record");
    assert_eq!(constant_nonunit["selection"], "selected");
    assert_eq!(constant_nonunit["configuration"], "disabled");
    assert_eq!(constant_nonunit["evaluation"], "not_evaluated");
    assert_eq!(
        constant_nonunit
            .as_object()
            .expect("check object")
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        [
            "check_id",
            "selection",
            "configuration",
            "applicability",
            "evaluation",
            "findings",
        ]
        .into_iter()
        .collect()
    );
    assert_evaluation_summary_matches_checks(&json);
    assert_output_schema_valid(&json);
}

#[test]
fn measure_and_lint_json_retain_each_primary_file_identity_in_argument_order() {
    let dir = unique_temp_dir("input-identities");
    let first = dir.path().join("first.glb");
    let second = dir.path().join("second.glb");
    write_clean_glb(&first);
    write_flipped_glb(&second);
    let expected = [input_identity_json(&first), input_identity_json(&second)];

    for command in ["measure", "lint"] {
        let output = animsmith()
            .args([
                command,
                first.to_str().expect("utf-8 fixture path"),
                second.to_str().expect("utf-8 fixture path"),
                "--format",
                "json",
            ])
            .output()
            .expect("runs JSON command");
        assert!(
            matches!(output.status.code(), Some(0 | 1)),
            "{command}: {}",
            stderr(&output)
        );
        let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
        let files = json["files"].as_array().expect("file records");
        assert_eq!(files.len(), 2);
        for (index, expected_input) in expected.iter().enumerate() {
            assert_eq!(
                files[index]["path"],
                [first.as_path(), second.as_path()][index]
                    .display()
                    .to_string()
            );
            assert_eq!(
                files[index]["input"], *expected_input,
                "{command} row {index}"
            );
            assert_eq!(
                files[index]["input"]["sha256"].as_str().map(str::len),
                Some(64)
            );
            assert_eq!(files[index]["input"]["bytes"], expected_input["bytes"]);
        }
        assert_ne!(files[0]["input"], files[1]["input"]);
    }
}

#[test]
fn cli_and_embedded_role_resolution_are_identical() {
    let dir = unique_temp_dir("resolver-parity");
    let input = dir.path().join("sway.glb");
    write_clean_glb(&input);
    let config_path = write_config(
        dir.path(),
        "roles.toml",
        "[rig]\nprofile = \"ue-mannequin\"\n[rig.roles]\nhips = \"spine\"\n",
    );
    let config: animsmith_core::Config = serde_json::from_value(json!({
        "rig": {
            "profile": "ue-mannequin",
            "roles": { "hips": "spine" }
        }
    }))
    .expect("embedded config");
    let doc = animsmith_gltf::load(&input).expect("loads fixture for embedding");
    let embedded = animsmith_core::resolve_configured_roles(&doc.skeleton, &config.rig);
    let embedded_roles: BTreeMap<_, _> = embedded
        .iter()
        .map(|(role, bone)| (role.as_str(), doc.skeleton.bones[bone].name.as_str()))
        .collect();

    let output = animsmith()
        .arg("--config")
        .arg(&config_path)
        .args(["lint", input.to_str().unwrap(), "--format", "json"])
        .output()
        .expect("runs animsmith");
    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    assert_eq!(json["files"][0]["rig"]["profile"], embedded.profile);
    assert_eq!(
        json["files"][0]["rig"]["resolved_roles"],
        json!(embedded_roles)
    );
}

#[test]
fn removed_preview_format_is_rejected_as_an_operator_error() {
    let output = animsmith()
        .args([
            "lint",
            fixture("rig.gltf").to_str().unwrap(),
            "--format",
            &format!("json-v2-{}", "preview"),
        ])
        .output()
        .expect("runs animsmith");
    assert_eq!(output.status.code(), Some(2));
    assert!(stdout(&output).is_empty());
    assert!(stderr(&output).contains(&format!("invalid value 'json-v2-{}'", "preview")));
}

#[test]
fn lint_json_exposes_complete_clean_and_unselected_checks() {
    let output = animsmith()
        .args([
            "lint",
            fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
            "--format",
            "json",
            "--select",
            "nan",
        ])
        .output()
        .expect("runs animsmith");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    assert_eq!(json["schema_version"], 7);
    assert_eq!(json["schema"], OUTPUT_SCHEMA_ID);
    let checks = json["files"][0]["checks"].as_array().expect("checks");
    let nan = checks
        .iter()
        .find(|check| check["check_id"] == "nan")
        .expect("nan record");
    assert_eq!(nan["selection"], "selected");
    assert_eq!(nan["configuration"], "enabled");
    assert_eq!(nan["applicability"], "applicable");
    assert_eq!(nan["evaluation"], "complete");
    assert_eq!(nan["findings"], json!([]));
    let duration = checks
        .iter()
        .find(|check| check["check_id"] == "duration-sanity")
        .expect("duration record");
    assert_eq!(duration["selection"], "unselected");
    assert_eq!(duration["evaluation"], "not_evaluated");
    let gait_group = checks
        .iter()
        .find(|check| check["check_id"] == "gait-group")
        .expect("gait-group record");
    assert_eq!(gait_group["selection"], "unselected");
    assert_eq!(gait_group["applicability"], "not_applicable");
    assert_eq!(gait_group["evaluation"], "not_evaluated");
    assert_evaluation_summary_matches_checks(&json);
    assert_output_schema_valid(&json);
}

#[test]
fn lint_json_keeps_disabled_distinct_from_unselected() {
    let dir = unique_temp_dir("v2-disabled");
    let config = write_config(
        dir.path(),
        "disabled.toml",
        "[checks.nan]\nseverity = \"off\"\n",
    );
    let output = animsmith()
        .arg("--config")
        .arg(&config)
        .args([
            "lint",
            fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
            "--format",
            "json",
            "--select",
            "nan",
        ])
        .output()
        .expect("runs animsmith");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    let checks = json["files"][0]["checks"].as_array().expect("checks");
    let nan = checks
        .iter()
        .find(|check| check["check_id"] == "nan")
        .expect("nan record");
    assert_eq!(nan["selection"], "selected");
    assert_eq!(nan["configuration"], "disabled");
    assert_eq!(nan["evaluation"], "not_evaluated");
    let duration = checks
        .iter()
        .find(|check| check["check_id"] == "duration-sanity")
        .expect("duration record");
    assert_eq!(duration["selection"], "unselected");
    assert_eq!(duration["configuration"], "enabled");
    let constant_nonunit = checks
        .iter()
        .find(|check| check["check_id"] == "constant-nonunit-scale")
        .expect("opt-in scale-channel record");
    assert_eq!(constant_nonunit["selection"], "unselected");
    assert_eq!(constant_nonunit["configuration"], "disabled");
    assert_eq!(constant_nonunit["evaluation"], "not_evaluated");
}

#[test]
fn lint_json_explicit_severity_enables_opt_in_check() {
    let dir = unique_temp_dir("v2-opt-in-scale-channel");
    let config = write_config(
        dir.path(),
        "opt-in.toml",
        "[checks.constant-nonunit-scale]\nseverity = \"note\"\n",
    );
    let output = animsmith()
        .arg("--config")
        .arg(&config)
        .args([
            "lint",
            fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
            "--format",
            "json",
            "--select",
            "constant-nonunit-scale",
        ])
        .output()
        .expect("runs animsmith");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    let check = json["files"][0]["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|check| check["check_id"] == "constant-nonunit-scale")
        .expect("opt-in scale-channel record");
    assert_eq!(check["selection"], "selected");
    assert_eq!(check["configuration"], "enabled");
    assert_eq!(check["evaluation"], "complete");
    assert_eq!(check["findings"], json!([]));
    assert_evaluation_summary_matches_checks(&json);
    assert_output_schema_valid(&json);
}

#[test]
fn lint_json_selects_non_uniform_scale_independently() {
    let output = animsmith()
        .args([
            "lint",
            fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
            "--format",
            "json",
            "--select",
            "non-uniform-scale",
        ])
        .output()
        .expect("runs animsmith");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    let checks = json["files"][0]["checks"].as_array().expect("checks");
    let non_uniform = checks
        .iter()
        .find(|check| check["check_id"] == "non-uniform-scale")
        .expect("non-uniform scale record");
    assert_eq!(non_uniform["selection"], "selected");
    assert_eq!(non_uniform["configuration"], "enabled");
    assert_eq!(non_uniform["evaluation"], "complete");
    assert_eq!(non_uniform["findings"], json!([]));
    let scale_keys = checks
        .iter()
        .find(|check| check["check_id"] == "scale-keys")
        .expect("scale keys record");
    assert_eq!(scale_keys["selection"], "unselected");
    assert_eq!(scale_keys["evaluation"], "not_evaluated");
    assert_evaluation_summary_matches_checks(&json);
    assert_output_schema_valid(&json);
}

#[test]
fn lint_json_gait_group_can_carry_finding_and_coverage_gap() {
    let dir = unique_temp_dir("v2-partial-gait");
    let config = write_config(
        dir.path(),
        "partial.toml",
        "[gait_groups.ring]\nclips = [\"walk\", \"missing\"]\nmax_gait_phase_spread = 0.1\n",
    );
    let output = animsmith()
        .arg("--config")
        .arg(&config)
        .args([
            "lint",
            fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
            "--format",
            "json",
            "--select",
            "gait-group",
        ])
        .output()
        .expect("runs animsmith");

    assert_eq!(
        output.status.code(),
        Some(1),
        "stderr:\n{}",
        stderr(&output)
    );
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    let gait = json["files"][0]["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|check| check["check_id"] == "gait-group")
        .expect("gait-group record");
    assert_eq!(gait["applicability"], "applicable");
    assert_eq!(gait["evaluation"], "partial");
    assert_eq!(gait["findings"][0]["severity"], "error");
    assert_eq!(gait["findings"][0]["clip"], "missing");
    assert_eq!(gait["gaps"][0]["code"], "roles_unresolved");
    assert_eq!(gait["gaps"][0]["scope"]["code"], "phase_coherence");
    assert_eq!(gait["evaluated_scopes"][0]["code"], "member_existence");
    assert_eq!(json["summary"]["checks"]["evaluation"]["partial"], 1);
    assert_eq!(json["summary"]["checks"]["gaps"], 1);
    assert_eq!(json["summary"]["findings"]["error"], 1);
    assert_evaluation_summary_matches_checks(&json);
    assert_output_schema_valid(&json);
}

#[test]
fn lint_json_sync_group_emits_schema_valid_member_table() {
    let dir = unique_temp_dir("v7-partial-sync-group");
    let config = write_config(
        dir.path(),
        "partial.toml",
        "[clips.walk]\nloop = true\nfps = 30.0\n\n[sync_groups.ring]\nclips = [\"walk\", \"missing\"]\nmax_duration_delta_s = 0.001\nmax_frame_count_delta = 0\nmax_fps_delta = 0.01\n",
    );
    let output = animsmith()
        .arg("--config")
        .arg(&config)
        .args([
            "lint",
            fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
            "--format",
            "json",
            "--select",
            "sync-group",
        ])
        .output()
        .expect("runs animsmith");

    assert_eq!(
        output.status.code(),
        Some(1),
        "stderr:\n{}",
        stderr(&output)
    );
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    assert_output_schema_valid(&json);
    let sync = json["files"][0]["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|check| check["check_id"] == "sync-group")
        .expect("sync-group record");
    assert_eq!(sync["evaluation"], "partial");
    assert_eq!(sync["findings"][0]["clip"], "missing");
    assert_eq!(sync["findings"][0]["members"][0]["member"], "walk");
    assert_eq!(
        sync["findings"][0]["members"][0]["measurements"]["availability"],
        "present"
    );
    assert_eq!(
        sync["findings"][0]["members"][1],
        serde_json::json!({
            "member": "missing",
            "measurements": { "availability": "missing" }
        })
    );
    assert!(
        sync["gaps"]
            .as_array()
            .expect("gaps")
            .iter()
            .any(|gap| gap["code"] == "insufficient_measurable_members")
    );
    assert_evaluation_summary_matches_checks(&json);
}

#[test]
fn lint_json_time_complement_emits_stable_pair_scores() {
    let dir = unique_temp_dir("v7-time-complement");
    let input = dir.path().join("time-complement.glb");
    write_time_complement_glb(&input);
    let config = write_config(
        dir.path(),
        "time-complement.toml",
        "[sync_groups.ring]\nclips = [\"forward\", \"reflected\"]\nmax_duration_delta_s = 0.001\nmax_frame_count_delta = 0\nmax_fps_delta = 0.01\n\n[sync_groups.ring.time_complement]\nmin_reflected_time_advantage = 0.25\nmin_lr_amplitude_m = 0.03\n",
    );
    let output = animsmith()
        .arg("--config")
        .arg(&config)
        .arg("lint")
        .arg(&input)
        .args([
            "--format",
            "json",
            "--select",
            "time-complement",
            "--deny-warnings",
        ])
        .output()
        .expect("runs animsmith");

    assert_eq!(
        output.status.code(),
        Some(1),
        "stderr:\n{}",
        stderr(&output)
    );
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    assert_output_schema_valid(&json);
    let check = json["files"][0]["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|check| check["check_id"] == "time-complement")
        .expect("time-complement record");
    assert_eq!(check["applicability"], "applicable");
    assert_eq!(check["evaluation"], "complete");
    assert!(check.get("gaps").is_none(), "empty gaps are omitted");

    let finding = &check["findings"][0];
    assert_eq!(finding["severity"], "warning");
    assert_eq!(finding["expected"], 0.25);
    assert!(
        finding["message"]
            .as_str()
            .unwrap()
            .contains("'forward' and 'reflected'")
    );
    let members = finding["members"].as_array().expect("pair member rows");
    assert_eq!(members.len(), 2);
    assert_eq!(members[0]["member"], "forward");
    assert_eq!(members[1]["member"], "reflected");
    for member in members {
        let measurements = member["measurements"].as_object().expect("measurements");
        assert_eq!(
            measurements.keys().map(String::as_str).collect::<Vec<_>>(),
            [
                "gait_phase",
                "lr_amplitude_m",
                "reflected_time_advantage",
                "reflected_time_similarity",
                "same_time_similarity",
            ]
        );
    }
    let scores = members[0]["measurements"].as_object().unwrap();
    for score in [
        "same_time_similarity",
        "reflected_time_similarity",
        "reflected_time_advantage",
    ] {
        assert_eq!(
            members[0]["measurements"][score],
            members[1]["measurements"][score]
        );
    }
    let same = scores["same_time_similarity"].as_f64().unwrap();
    let reflected = scores["reflected_time_similarity"].as_f64().unwrap();
    let advantage = scores["reflected_time_advantage"].as_f64().unwrap();
    assert!(reflected > same);
    assert!((advantage - (reflected - same)).abs() < 1e-12);
    assert_eq!(finding["measured"].as_f64().unwrap(), advantage);
    assert_evaluation_summary_matches_checks(&json);
}

#[test]
fn lint_json_exit_policy_uses_findings_not_coverage_gaps() {
    let warning_dir = unique_temp_dir("v2-warning-exit");
    let warning_input = warning_dir.path().join("flipped.glb");
    write_flipped_glb(&warning_input);

    for (deny, expected) in [(false, 0), (true, 1)] {
        let mut args = vec![
            "lint",
            warning_input.to_str().expect("utf-8 input path"),
            "--format",
            "json",
            "--select",
            "quat-flip",
        ];
        if deny {
            args.push("--deny-warnings");
        }
        let output = animsmith().args(&args).output().expect("runs animsmith");
        assert_eq!(
            output.status.code(),
            Some(expected),
            "warning exit (deny-warnings: {deny}):\n{}",
            stderr(&output)
        );
        let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
        let quat_flip = json["files"][0]["checks"]
            .as_array()
            .expect("checks")
            .iter()
            .find(|check| check["check_id"] == "quat-flip")
            .expect("quat-flip record");
        assert_eq!(quat_flip["findings"][0]["severity"], "warning");
        assert!(quat_flip["gaps"].is_null());
    }

    let gap_dir = unique_temp_dir("v2-gap-exit");
    let gap_input = gap_dir.path().join("sway.glb");
    write_clean_glb(&gap_input);
    let config = write_config(gap_dir.path(), "gap.toml", "[clips.sway]\nloop = true\n");
    for deny in [false, true] {
        let mut args = vec![
            "--config",
            config.to_str().expect("utf-8 config path"),
            "lint",
            gap_input.to_str().expect("utf-8 input path"),
            "--format",
            "json",
            "--select",
            "loop-seam",
        ];
        if deny {
            args.push("--deny-warnings");
        }
        let output = animsmith().args(&args).output().expect("runs animsmith");
        assert_eq!(
            output.status.code(),
            Some(0),
            "coverage gap must not gate (deny-warnings: {deny}):\n{}",
            stderr(&output)
        );
        let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
        let loop_seam = json["files"][0]["checks"]
            .as_array()
            .expect("checks")
            .iter()
            .find(|check| check["check_id"] == "loop-seam")
            .expect("loop-seam record");
        assert_eq!(loop_seam["findings"], json!([]));
        assert_eq!(loop_seam["gaps"][0]["code"], "roles_unresolved");
    }
}

#[test]
fn lint_json_rejects_allow_instead_of_deleting_evidence() {
    let output = animsmith()
        .args([
            "lint",
            fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
            "--format",
            "json",
            "--allow",
            "nan",
        ])
        .output()
        .expect("runs animsmith");

    assert_eq!(output.status.code(), Some(2));
    assert!(stdout(&output).is_empty());
    assert!(stderr(&output).contains("machine-readable results retain every content finding"));
}

#[test]
fn diff_json_uses_versioned_envelope() {
    let path = fixture("rig.gltf");
    let output = animsmith()
        .args([
            "diff",
            path.to_str().expect("utf-8 fixture path"),
            path.to_str().expect("utf-8 fixture path"),
            "--format",
            "json",
        ])
        .output()
        .expect("runs animsmith");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    assert_output_schema_valid(&json);
    assert_eq!(json["schema_version"], 7);
    assert_eq!(json["schema"], OUTPUT_SCHEMA_ID);
    assert_eq!(json["tool"]["name"], "animsmith");
    assert_eq!(json["tool"]["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(json["command"], "diff");
    assert_eq!(json["summary"]["deltas"], 0);
    assert_eq!(json["deltas"].as_array().expect("deltas array").len(), 0);
    assert!(json["inputs"]["before"].is_string());
    assert!(json["inputs"]["after"].is_string());
}

#[test]
fn output_schema_rejects_cross_command_and_nested_contract_drift() {
    let output = animsmith()
        .args([
            "measure",
            fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
            "--format",
            "json",
        ])
        .output()
        .expect("runs animsmith");
    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let measure: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    assert_output_schema_valid(&measure);
    let validator = output_validator();

    let mut foreign_field = measure.clone();
    foreign_field["deltas"] = json!([]);
    assert!(!validator.is_valid(&foreign_field));

    let mut nested_version = measure.clone();
    nested_version["files"][0]["measurements"]["schema_version"] = json!(7);
    assert!(!validator.is_valid(&nested_version));

    let mut missing_input = measure.clone();
    missing_input["files"][0]
        .as_object_mut()
        .expect("file record")
        .remove("input");
    assert!(!validator.is_valid(&missing_input));

    let mut uppercase_digest = measure.clone();
    uppercase_digest["files"][0]["input"]["sha256"] = json!("A".repeat(64));
    assert!(!validator.is_valid(&uppercase_digest));

    let mut negative_byte_count = measure.clone();
    negative_byte_count["files"][0]["input"]["bytes"] = json!(-1);
    assert!(!validator.is_valid(&negative_byte_count));

    let mut lint_without_checks = measure;
    lint_without_checks["command"] = json!("lint");
    assert!(!validator.is_valid(&lint_without_checks));
}

#[test]
fn diff_accepts_single_file_measure_report_round_trip() {
    let dir = unique_temp_dir("diff-round-trip");
    let asset = fixture("rig.gltf");
    let report_path = dir.path().join("measure.json");

    let measured = animsmith()
        .args([
            "measure",
            asset.to_str().expect("utf-8 fixture path"),
            "--format",
            "json",
        ])
        .output()
        .expect("runs animsmith");
    assert!(measured.status.success(), "stderr:\n{}", stderr(&measured));
    std::fs::write(&report_path, &measured.stdout).expect("writes report");

    // A report diffed against the asset it was measured from is clean.
    let output = animsmith()
        .args([
            "diff",
            report_path.to_str().expect("utf-8 report path"),
            asset.to_str().expect("utf-8 fixture path"),
        ])
        .output()
        .expect("runs animsmith");
    // Clean == exit 0; the "no significant movement" prose is not the
    // contract (that's the exit code) and is left unpinned.
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
}

#[test]
fn diff_keeps_regeneration_guidance_for_finite_json_outside_f32() {
    let dir = unique_temp_dir("diff-f32-overflow-guidance");
    let asset = fixture("rig.gltf");
    let report_path = dir.path().join("measure.json");

    let measured = animsmith()
        .args([
            "measure",
            asset.to_str().expect("utf-8 fixture path"),
            "--format",
            "json",
        ])
        .output()
        .expect("runs animsmith");
    assert!(measured.status.success(), "stderr:\n{}", stderr(&measured));
    let mut report: Value =
        serde_json::from_slice(&measured.stdout).expect("measurement output is JSON");
    report["files"][0]["measurements"]["skeleton_nodes"][0]["local_rest"]["translation_parent_space_m"]
        [0] = json!(1e39_f64);
    write_json(&report_path, &report);

    let output = animsmith()
        .args([
            "diff",
            report_path.to_str().expect("utf-8 report path"),
            asset.to_str().expect("utf-8 fixture path"),
        ])
        .output()
        .expect("runs animsmith");

    assert_eq!(output.status.code(), Some(2));
    assert!(stdout(&output).is_empty());
    let error = stderr(&output);
    assert!(
        error.contains("must be finite; regenerate it from the original asset"),
        "stderr:\n{error}"
    );
    assert!(!error.contains("bad JSON"), "stderr:\n{error}");
    assert!(!error.contains("number out of range"), "stderr:\n{error}");
}

#[test]
fn diff_accepts_single_file_lint_report_round_trip() {
    let dir = unique_temp_dir("diff-lint-round-trip");
    let asset = fixture("rig.gltf");
    let report_path = dir.path().join("lint.json");
    let linted = animsmith()
        .args([
            "lint",
            asset.to_str().expect("utf-8 fixture path"),
            "--format",
            "json",
        ])
        .output()
        .expect("runs animsmith");
    assert!(linted.status.success(), "stderr:\n{}", stderr(&linted));
    std::fs::write(&report_path, &linted.stdout).expect("writes report");

    let output = animsmith()
        .args([
            "diff",
            report_path.to_str().expect("utf-8 report path"),
            asset.to_str().expect("utf-8 fixture path"),
        ])
        .output()
        .expect("runs animsmith");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&output)
    );
}

#[test]
fn diff_accepts_measurement_json_and_exits_one_for_deltas() {
    let dir = unique_temp_dir("diff-json-deltas");
    let before = dir.path().join("before.json");
    let after = dir.path().join("after.json");
    write_json(&before, &measurement_report(1.0));
    write_json(&after, &measurement_report(1.1));

    let output = animsmith()
        .args([
            "diff",
            before.to_str().expect("utf-8 before path"),
            after.to_str().expect("utf-8 after path"),
            "--format",
            "json",
        ])
        .output()
        .expect("runs animsmith");

    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    // The CLI contract is the envelope shape + exit code: one delta,
    // routed to its clip. The metric/note strings are the unit suite's
    // job (diff.rs), so they are not re-pinned here.
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    assert_eq!(json["summary"]["deltas"].as_u64(), Some(1));
    assert_eq!(json["deltas"][0]["clip"], "walk");
}

#[test]
fn diff_accepts_measurement_json_and_exits_zero_without_deltas() {
    let dir = unique_temp_dir("diff-json-clean");
    let before = dir.path().join("before.json");
    let after = dir.path().join("after.json");
    let report = measurement_report(1.0);
    write_json(&before, &report);
    write_json(&after, &report);

    let output = animsmith()
        .args([
            "diff",
            before.to_str().expect("utf-8 before path"),
            after.to_str().expect("utf-8 after path"),
        ])
        .output()
        .expect("runs animsmith");

    // Identical reports in, exit 0 out — the exit code is the contract,
    // not the human-format prose.
    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
}

#[test]
fn diff_rejects_zero_or_multiple_measurement_file_records() {
    let dir = unique_temp_dir("diff-report-file-count");
    let report_path = dir.path().join("report.json");

    for file_count in [0, 2, 3, 10] {
        let mut report = measurement_report(1.0);
        let file = report["files"][0].clone();
        report["files"] = Value::Array(vec![file; file_count]);
        write_json(&report_path, &report);

        let output = animsmith()
            .args([
                "diff",
                report_path.to_str().expect("utf-8 report path"),
                fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
            ])
            .output()
            .expect("runs animsmith");

        assert_eq!(
            output.status.code(),
            Some(2),
            "stdout:\n{}\nstderr:\n{}",
            stdout(&output),
            stderr(&output)
        );
        assert!(stdout(&output).is_empty());
        assert_eq!(
            stderr(&output),
            format!(
                "animsmith: {} contains {file_count} file records; diff expects a single-file measurement report\n",
                report_path.display()
            )
        );
    }
}

#[test]
fn diff_preserves_error_precedence_for_malformed_multi_file_reports() {
    let dir = unique_temp_dir("diff-malformed-multi-report");
    let report_path = dir.path().join("report.json");
    let base = measurement_report(1.0);
    let file = base["files"][0].clone();

    let mut invalid_first = base.clone();
    invalid_first["files"] = json!([file.clone(), file.clone()]);
    invalid_first["files"][0]
        .as_object_mut()
        .expect("file record")
        .remove("measurements");

    let mut invalid_second = base.clone();
    invalid_second["files"] = json!([file.clone(), file.clone()]);
    invalid_second["files"][1]
        .as_object_mut()
        .expect("file record")
        .remove("measurements");

    let mut missing_command = base.clone();
    missing_command["files"] = json!([file.clone(), file.clone()]);
    missing_command
        .as_object_mut()
        .expect("report envelope")
        .remove("command");

    let mut wrong_output_identity = base.clone();
    wrong_output_identity["files"] = json!([file.clone(), file.clone()]);
    wrong_output_identity["schema"] = json!("urn:other:output");

    let mut invalid_third = base;
    invalid_third["files"] = json!([file.clone(), file.clone(), file]);
    invalid_third["files"][2]["measurements"]["schema"] = json!("urn:other:measurements");

    let remediation =
        "regenerate it from the original asset with `animsmith measure --format json <asset>`";
    for (name, report, expected) in [
        (
            "invalid first record",
            invalid_first,
            "contains 2 file records; diff expects a single-file measurement report".to_owned(),
        ),
        (
            "invalid second record",
            invalid_second,
            "contains 2 file records; diff expects a single-file measurement report".to_owned(),
        ),
        (
            "invalid third record",
            invalid_third,
            "contains 3 file records; diff expects a single-file measurement report".to_owned(),
        ),
        (
            "missing envelope command",
            missing_command,
            format!("is not an animsmith measurement report (no `command`); {remediation}"),
        ),
        (
            "wrong output identity",
            wrong_output_identity,
            format!("does not identify output contract {OUTPUT_SCHEMA_ID}; {remediation}"),
        ),
    ] {
        write_json(&report_path, &report);
        let output = animsmith()
            .args([
                "diff",
                report_path.to_str().expect("utf-8 report path"),
                fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
            ])
            .output()
            .expect("runs animsmith");

        assert_eq!(output.status.code(), Some(2), "{name}");
        assert!(stdout(&output).is_empty(), "{name}");
        assert_eq!(
            stderr(&output),
            format!("animsmith: {} {expected}\n", report_path.display()),
            "{name}"
        );
    }
}

#[test]
fn diff_preserves_tailored_report_errors_and_remediation() {
    let dir = unique_temp_dir("diff-report-errors");
    let report_path = dir.path().join("report.json");
    let base = measurement_report(1.0);
    let without = |pointer: &str| {
        let mut report = base.clone();
        let (parent, key) = pointer.rsplit_once('/').expect("JSON pointer has a key");
        let object = if parent.is_empty() {
            &mut report
        } else {
            report.pointer_mut(parent).expect("fixture path exists")
        };
        object
            .as_object_mut()
            .expect("path ends at an object")
            .remove(key);
        report
    };
    let mut unsupported_output_version = base.clone();
    unsupported_output_version["schema_version"] = json!(2);
    let mut wrong_output_identity = base.clone();
    wrong_output_identity["schema"] = json!("urn:other:output");
    let mut unsupported_command = base.clone();
    unsupported_command["command"] = json!("diff");
    let mut unsupported_measurement_version = base.clone();
    unsupported_measurement_version["files"][0]["measurements"]["schema_version"] = json!(7);
    let mut wrong_measurement_identity = base.clone();
    wrong_measurement_identity["files"][0]["measurements"]["schema"] =
        json!("urn:other:measurements");

    let remediation =
        "regenerate it from the original asset with `animsmith measure --format json <asset>`";
    let cases = vec![
        (
            "missing output version",
            without("/schema_version"),
            format!("is not an animsmith report envelope (no `schema_version`); {remediation}"),
        ),
        (
            "wrong output identity",
            wrong_output_identity,
            format!("does not identify output contract {OUTPUT_SCHEMA_ID}; {remediation}"),
        ),
        (
            "unsupported output version",
            unsupported_output_version,
            format!("has schema_version 2; this build reads schema_version 7; {remediation}"),
        ),
        (
            "missing command",
            without("/command"),
            format!("is not an animsmith measurement report (no `command`); {remediation}"),
        ),
        (
            "unsupported command",
            unsupported_command,
            "is a \"diff\" report; diff reads only measure or lint reports".to_owned(),
        ),
        (
            "missing files",
            without("/files"),
            format!("is not an animsmith report envelope (no `files` array); {remediation}"),
        ),
        (
            "missing path",
            without("/files/0/path"),
            format!("report file record has no `path`; {remediation}"),
        ),
        (
            "missing measurements",
            without("/files/0/measurements"),
            "report has no measurements".to_owned(),
        ),
        (
            "missing measurement version",
            without("/files/0/measurements/schema_version"),
            format!("has no versioned measurement contract; {remediation}"),
        ),
        (
            "unsupported measurement version",
            unsupported_measurement_version,
            format!(
                "has measurement schema_version 7; this build reads measurement schema_version 13; {remediation}"
            ),
        ),
        (
            "wrong measurement identity",
            wrong_measurement_identity,
            format!(
                "does not identify measurement contract {MEASUREMENTS_SCHEMA_ID}; {remediation}"
            ),
        ),
        (
            "missing clips",
            without("/files/0/measurements/clips"),
            "measurement contract has no `clips` map".to_owned(),
        ),
    ];

    for (name, report, expected) in cases {
        write_json(&report_path, &report);
        let output = animsmith()
            .args([
                "diff",
                report_path.to_str().expect("utf-8 report path"),
                fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
            ])
            .output()
            .expect("runs animsmith");

        assert_eq!(output.status.code(), Some(2), "{name}");
        assert!(stdout(&output).is_empty(), "{name}");
        assert_eq!(
            stderr(&output),
            format!("animsmith: {} {expected}\n", report_path.display()),
            "{name}"
        );
    }
}

#[test]
fn diff_compares_decoded_numbers_not_json_lexical_spelling() {
    let dir = unique_temp_dir("diff-json-number-spelling");
    let before = dir.path().join("before.json");
    let after = dir.path().join("after.json");
    let decimal = serde_json::to_string(&measurement_report(1.0)).unwrap();
    assert!(decimal.contains("\"duration_s\":1.0,"));
    let integer = decimal.replace("\"duration_s\":1.0,", "\"duration_s\":1,");
    let exponent = decimal.replace("\"duration_s\":1.0,", "\"duration_s\":1e0,");

    for (left, right) in [
        (&integer, &decimal),
        (&decimal, &integer),
        (&exponent, &decimal),
    ] {
        std::fs::write(&before, left).unwrap();
        std::fs::write(&after, right).unwrap();
        let output = animsmith()
            .args([
                "diff",
                before.to_str().unwrap(),
                after.to_str().unwrap(),
                "--format",
                "json",
            ])
            .output()
            .expect("runs animsmith");
        assert_eq!(
            output.status.code(),
            Some(0),
            "stderr:\n{}",
            stderr(&output)
        );
        let json: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(json["summary"]["deltas"], 0);
    }
}

#[test]
fn diff_rejects_alpha_v1_reports() {
    let dir = unique_temp_dir("diff-v1-report");
    let old = dir.path().join("v1.json");
    let mut report = measurement_report(1.0);
    report["schema_version"] = json!(1);
    report["schema"] = json!("urn:animsmith:schema:output:1");
    write_json(&old, &report);

    let output = animsmith()
        .args([
            "diff",
            old.to_str().unwrap(),
            fixture("rig.gltf").to_str().unwrap(),
        ])
        .output()
        .expect("runs animsmith");

    assert_eq!(output.status.code(), Some(2));
    assert!(stdout(&output).is_empty());
    assert!(stderr(&output).contains("schema_version 1"));
}

#[test]
fn diff_rejects_outer_and_nested_contract_identity_drift() {
    let dir = unique_temp_dir("diff-contract-identity");
    let report_path = dir.path().join("report.json");
    let cases = [
        (
            {
                let mut report = measurement_report(1.0);
                report["schema"] = json!("urn:animsmith:schema:output:other");
                report
            },
            "does not identify output contract",
        ),
        (
            {
                let mut report = measurement_report(1.0);
                report["files"][0]["measurements"]["schema_version"] = json!(7);
                report
            },
            "has measurement schema_version 7; this build reads measurement schema_version 13; regenerate it from the original asset with `animsmith measure --format json <asset>`",
        ),
        (
            {
                let mut report = measurement_report(1.0);
                report["files"][0]["measurements"]["schema"] =
                    json!("urn:animsmith:schema:measurements:other");
                report
            },
            "does not identify measurement contract",
        ),
    ];
    for (report, expected) in cases {
        write_json(&report_path, &report);
        let output = animsmith()
            .args([
                "diff",
                report_path.to_str().unwrap(),
                fixture("rig.gltf").to_str().unwrap(),
            ])
            .output()
            .expect("runs animsmith");
        assert_eq!(output.status.code(), Some(2));
        assert!(stdout(&output).is_empty());
        assert!(
            stderr(&output).contains(expected),
            "stderr:\n{}",
            stderr(&output)
        );
    }
}

#[test]
fn diff_rejects_non_measurement_report_commands() {
    let dir = unique_temp_dir("diff-wrong-command");
    let report_path = dir.path().join("diff.json");
    let mut report = measurement_report(1.0);
    report["command"] = json!("diff");
    write_json(&report_path, &report);

    let output = animsmith()
        .args([
            "diff",
            report_path.to_str().unwrap(),
            fixture("rig.gltf").to_str().unwrap(),
        ])
        .output()
        .expect("runs animsmith");

    assert_eq!(output.status.code(), Some(2));
    assert!(stdout(&output).is_empty());
    assert!(stderr(&output).contains("reads only measure or lint reports"));
}

#[test]
fn diff_text_format_renders_deltas_and_clean_summary() {
    // The default (human) format has its own render branch that the JSON
    // contract tests never exercise. This is the one test that owns that
    // branch: a dirty diff must name the moved clip and print a change
    // summary; a clean diff must print its clean line. (The envelope /
    // exit-code contract tests deliberately do NOT string-match this
    // prose — pinning the renderer is this test's job, not theirs.)
    let dir = unique_temp_dir("diff-text-format");
    let before = dir.path().join("before.json");
    let after = dir.path().join("after.json");
    write_json(&before, &measurement_report(1.0));
    write_json(&after, &measurement_report(1.1));

    let dirty = animsmith()
        .args([
            "diff",
            before.to_str().expect("utf-8 before path"),
            after.to_str().expect("utf-8 after path"),
        ])
        .output()
        .expect("runs animsmith");
    assert_eq!(dirty.status.code(), Some(1), "stderr:\n{}", stderr(&dirty));
    let out = stdout(&dirty);
    assert!(
        out.contains("walk"),
        "dirty Text output names the clip:\n{out}"
    );
    assert!(
        out.contains("significant change"),
        "dirty Text output summarizes the change count:\n{out}"
    );

    let clean = animsmith()
        .args([
            "diff",
            before.to_str().expect("utf-8 before path"),
            before.to_str().expect("utf-8 before path"),
        ])
        .output()
        .expect("runs animsmith");
    assert_eq!(clean.status.code(), Some(0), "stderr:\n{}", stderr(&clean));
    assert!(
        stdout(&clean).contains("no significant movement"),
        "clean Text output states no movement:\n{}",
        stdout(&clean)
    );
}

#[test]
fn diff_text_escapes_controls_from_report_clip_metric_and_note_fields() {
    let dir = unique_temp_dir("diff-text-controls");
    let before_path = dir.path().join("before.json");
    let after_path = dir.path().join("after.json");
    let hostile = "forged\nline\u{1b}[31m";
    let mut before = measurement_report(1.0);
    let mut after = measurement_report(1.1);
    for report in [&mut before, &mut after] {
        let clip = report["files"][0]["measurements"]["clips"]
            .as_object_mut()
            .expect("clip map")
            .remove("walk")
            .expect("walk fixture");
        report["files"][0]["measurements"]["clips"]
            .as_object_mut()
            .expect("clip map")
            .insert(hostile.into(), clip);
    }
    before["files"][0]["measurements"]["clips"][hostile]["bone_rotation_range_deg"] = json!({});
    before["files"][0]["measurements"]["clips"][hostile]["bone_rotation_range_deg"]
        .as_object_mut()
        .expect("rotation map")
        .insert(hostile.into(), json!(0.0));
    after["files"][0]["measurements"]["clips"][hostile]["bone_rotation_range_deg"] = json!({});
    after["files"][0]["measurements"]["clips"][hostile]["bone_rotation_range_deg"]
        .as_object_mut()
        .expect("rotation map")
        .insert(hostile.into(), json!(10.0));
    after["files"][0]["measurements"]["clips"][hostile]["animated_bones"] = json!([hostile]);
    write_json(&before_path, &before);
    write_json(&after_path, &after);

    let output = animsmith()
        .args(["diff"])
        .arg(&before_path)
        .arg(&after_path)
        .output()
        .expect("runs animsmith");
    assert_eq!(
        output.status.code(),
        Some(1),
        "stderr:\n{}",
        stderr(&output)
    );
    let text = stdout(&output);
    assert!(!text.contains(hostile), "raw controls leaked:\n{text}");
    assert!(text.contains("\\n"), "newline not escaped:\n{text}");
    assert!(text.contains("\\u{1b}"), "escape not escaped:\n{text}");
}

#[test]
fn diff_rejects_json_without_schema_version_with_measure_remediation() {
    let dir = unique_temp_dir("diff-bare-map");
    let bare = dir.path().join("bare.json");
    // A bare measurement map (a pre-publish development shape) has no
    // schema_version and must be rejected with regenerate guidance.
    std::fs::write(&bare, r#"{"walk": {"duration_s": 1.0}}"#).expect("writes bare map");

    let output = animsmith()
        .args([
            "diff",
            bare.to_str().expect("utf-8 path"),
            fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
        ])
        .output()
        .expect("runs animsmith");

    assert_eq!(
        output.status.code(),
        Some(2),
        "stdout:\n{}",
        stdout(&output)
    );
    assert!(
        stderr(&output).contains("not an animsmith report envelope"),
        "stderr:\n{}",
        stderr(&output)
    );
    assert!(
        stderr(&output).contains(
            "regenerate it from the original asset with `animsmith measure --format json <asset>`"
        ),
        "stderr:\n{}",
        stderr(&output)
    );
}

#[test]
fn diff_rejects_unsupported_schema_versions() {
    let dir = unique_temp_dir("diff-future-schema");
    let future = dir.path().join("future.json");
    for version in [2, 3, 5, 99] {
        let mut report = measurement_report(1.0);
        report["schema_version"] = json!(version);
        write_json(&future, &report);
        let output = animsmith()
            .args([
                "diff",
                future.to_str().expect("utf-8 path"),
                fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
            ])
            .output()
            .expect("runs animsmith");
        assert_eq!(
            output.status.code(),
            Some(2),
            "stdout:\n{}",
            stdout(&output)
        );
        assert!(
            stderr(&output).contains(&format!("schema_version {version}")),
            "stderr:\n{}",
            stderr(&output)
        );
        assert!(
            stderr(&output).contains(
                "regenerate it from the original asset with `animsmith measure --format json <asset>`"
            ),
            "version {version}: stderr:\n{}",
            stderr(&output)
        );
    }
}

#[test]
fn diff_rejects_historical_output_v5_with_v11_measurements() {
    let dir = unique_temp_dir("diff-output-v5-measurements-v11");
    let report_path = dir.path().join("historical.json");
    let mut report = measurement_report(1.0);
    report["schema_version"] = json!(5);
    report["schema"] = json!("urn:animsmith:schema:output:5");
    report["files"][0]["measurements"]["schema_version"] = json!(11);
    report["files"][0]["measurements"]["schema"] = json!("urn:animsmith:schema:measurements:11");
    write_json(&report_path, &report);

    let output = animsmith()
        .args([
            "diff",
            report_path.to_str().expect("utf-8 report path"),
            fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
        ])
        .output()
        .expect("runs animsmith");

    assert_eq!(output.status.code(), Some(2));
    assert!(stdout(&output).is_empty());
    assert!(
        stderr(&output).contains(
            "has schema_version 5; this build reads schema_version 7; regenerate it from the original asset with `animsmith measure --format json <asset>`"
        ),
        "stderr:\n{}",
        stderr(&output)
    );
}

#[test]
fn diff_rejects_all_unsupported_nested_measurement_schema_versions() {
    let dir = unique_temp_dir("diff-unsupported-nested-schema");
    let report_path = dir.path().join("report.json");
    for version in [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 99] {
        let mut report = measurement_report(1.0);
        report["files"][0]["measurements"]["schema_version"] = json!(version);
        write_json(&report_path, &report);
        let output = animsmith()
            .args([
                "diff",
                report_path.to_str().expect("utf-8 path"),
                fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
            ])
            .output()
            .expect("runs animsmith");
        assert_eq!(
            output.status.code(),
            Some(2),
            "version {version}: stdout:\n{}",
            stdout(&output)
        );
        assert!(
            stderr(&output).contains(&format!(
                "has measurement schema_version {version}; this build reads measurement schema_version 13"
            )),
            "version {version}: stderr:\n{}",
            stderr(&output)
        );
    }
}

#[test]
fn diff_rejects_v11_skeleton_shape_before_decoding_v13_fields() {
    let dir = unique_temp_dir("diff-v11-skeleton-shape");
    let report_path = dir.path().join("report.json");
    let mut report = measurement_report(1.0);
    report["files"][0]["measurements"]["schema_version"] = json!(11);
    report["files"][0]["measurements"]["schema"] = json!("urn:animsmith:schema:measurements:11");
    report["files"][0]["measurements"]["skeleton_source_coverage"] = json!("complete");
    report["files"][0]["measurements"]["skeleton_nodes"] = json!([{
        "node_index": 0,
        "scene_root_indices": [],
        "local_rest": {
            "kind": "trs",
            "translation_m": [0.0, 0.0, 0.0],
            "rotation_xyzw": [0.0, 0.0, 0.0, 1.0],
            "scale": [1.0, 1.0, 1.0]
        },
        "rest_world_matrix": [
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 1.0
        ]
    }]);
    write_json(&report_path, &report);

    let output = animsmith()
        .args([
            "diff",
            report_path.to_str().expect("utf-8 report path"),
            fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
        ])
        .output()
        .expect("runs animsmith");

    assert_eq!(output.status.code(), Some(2));
    assert!(stdout(&output).is_empty());
    assert!(
        stderr(&output).contains(
            "has measurement schema_version 11; this build reads measurement schema_version 13; regenerate it from the original asset with `animsmith measure --format json <asset>`"
        ),
        "stderr:\n{}",
        stderr(&output)
    );
    assert!(!stderr(&output).contains("bad JSON"));
}

#[test]
fn diff_does_not_accept_v10_skeleton_or_skin_shapes_under_the_v13_identity() {
    let dir = unique_temp_dir("diff-v10-shape-v13-identity");
    let report_path = dir.path().join("report.json");
    let mut report = measurement_report(1.0);
    report["files"][0]["measurements"]["skeleton_source_coverage"] = json!("complete");
    report["files"][0]["measurements"]["skeleton_nodes"] = json!([{
        "node_index": 0,
        "scene_root_indices": [],
        "local_rest": {
            "kind": "trs",
            "translation_m": [0.0, 0.0, 0.0],
            "rotation_xyzw": [0.0, 0.0, 0.0, 1.0],
            "scale": [1.0, 1.0, 1.0]
        },
        "rest_world_matrix": [
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 1.0
        ]
    }]);
    write_json(&report_path, &report);

    let output = animsmith()
        .args([
            "diff",
            report_path.to_str().expect("utf-8 report path"),
            fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
        ])
        .output()
        .expect("runs animsmith");

    assert_eq!(output.status.code(), Some(2));
    assert!(stdout(&output).is_empty());
    assert!(
        stderr(&output).contains(
            "measurement structure skeleton_nodes[0] is invalid: uses a shape from an earlier measurement contract"
        ),
        "stderr:\n{}",
        stderr(&output)
    );

    report["files"][0]["measurements"]["skeleton_source_coverage"] = json!("unavailable");
    report["files"][0]["measurements"]["skeleton_nodes"] = json!([]);
    report["files"][0]["measurements"]["skins"] = json!([{ "skin_index": 0 }]);
    write_json(&report_path, &report);
    let output = animsmith()
        .args([
            "diff",
            report_path.to_str().expect("utf-8 report path"),
            fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
        ])
        .output()
        .expect("runs animsmith");
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains(
            "measurement structure skins[0] is invalid: uses a shape from an earlier measurement contract"
        ),
        "stderr:\n{}",
        stderr(&output)
    );
}

#[test]
fn diff_rejects_envelope_without_files() {
    let dir = unique_temp_dir("diff-no-files");
    let report = dir.path().join("no-files.json");
    std::fs::write(
        &report,
        r#"{"schema_version":7,"schema":"urn:animsmith:schema:output:7","command":"measure"}"#,
    )
    .expect("writes report");

    let output = animsmith()
        .args([
            "diff",
            report.to_str().expect("utf-8 path"),
            fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
        ])
        .output()
        .expect("runs animsmith");

    assert_eq!(
        output.status.code(),
        Some(2),
        "stdout:\n{}",
        stdout(&output)
    );
    assert!(
        stderr(&output).contains("no `files` array"),
        "stderr:\n{}",
        stderr(&output)
    );
}

#[test]
fn lint_counts_severities_in_summary_and_text() {
    let dir = unique_temp_dir("lint-severity-counts");
    let input = dir.path().join("dirty.glb");
    write_flipped_glb(&input);

    // JSON: the flipped fixture produces exactly one quat-flip warning;
    // the summary must bucket it as a warning, not a note or error.
    let output = animsmith()
        .args([
            "lint",
            input.to_str().expect("utf-8 input path"),
            "--format",
            "json",
            "--select",
            "quat-flip",
        ])
        .output()
        .expect("runs animsmith");
    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    assert_eq!(json["summary"]["findings"]["warning"], 1, "{json:#}");
    assert_eq!(json["summary"]["findings"]["error"], 0, "{json:#}");
    assert_eq!(json["summary"]["findings"]["note"], 0, "{json:#}");
    let quat_flip = json["files"][0]["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["check_id"] == "quat-flip")
        .unwrap();
    assert_eq!(quat_flip["findings"][0]["severity"], "warning");

    // Text mode counts through the same severity match.
    let output = animsmith()
        .args([
            "lint",
            input.to_str().expect("utf-8 input path"),
            "--select",
            "quat-flip",
        ])
        .output()
        .expect("runs animsmith");
    assert!(
        stdout(&output).contains("1 warning(s)"),
        "stdout:\n{}",
        stdout(&output)
    );
}

#[test]
fn fix_reports_unreadable_input_as_operator_error() {
    let output = animsmith()
        .args(["fix", "missing.glb", "--dry-run"])
        .output()
        .expect("runs animsmith");

    assert_eq!(
        output.status.code(),
        Some(2),
        "stdout:\n{}",
        stdout(&output)
    );
    assert!(
        stderr(&output).contains("failed to read"),
        "stderr:\n{}",
        stderr(&output)
    );
}

/// 3 keyframe times but 2 output values — structurally malformed.
const COUNT_MISMATCH_GLTF: &str = r#"{
  "asset": { "version": "2.0" },
  "buffers": [{ "uri": "data:application/octet-stream;base64,AAAAAAAAAD8AAIA/AAAAAAAAAAAAAAAAAACAPwAAAAAAAAAAAAAAAAAAgD8=", "byteLength": 44 }],
  "bufferViews": [
    { "buffer": 0, "byteOffset": 0, "byteLength": 12 },
    { "buffer": 0, "byteOffset": 12, "byteLength": 32 }
  ],
  "accessors": [
    { "bufferView": 0, "componentType": 5126, "count": 3, "type": "SCALAR", "min": [0], "max": [1] },
    { "bufferView": 1, "componentType": 5126, "count": 2, "type": "VEC4" }
  ],
  "nodes": [{ "name": "root" }],
  "animations": [{
    "name": "bad",
    "samplers": [{ "input": 0, "output": 1, "interpolation": "LINEAR" }],
    "channels": [{ "sampler": 0, "target": { "node": 0, "path": "rotation" } }]
  }],
  "scenes": [{ "nodes": [0] }],
  "scene": 0
}"#;

/// First keyframe time is NaN; values are valid identity quats.
const NAN_TIME_GLTF: &str = r#"{
  "asset": { "version": "2.0" },
  "buffers": [{ "uri": "data:application/octet-stream;base64,AADAfwAAAD8AAIA/AAAAAAAAAAAAAAAAAACAPwAAAAAAAAAAAAAAAAAAgD8AAAAAAAAAAAAAAAAAAIA/", "byteLength": 60 }],
  "bufferViews": [
    { "buffer": 0, "byteOffset": 0, "byteLength": 12 },
    { "buffer": 0, "byteOffset": 12, "byteLength": 48 }
  ],
  "accessors": [
    { "bufferView": 0, "componentType": 5126, "count": 3, "type": "SCALAR", "min": [0], "max": [1] },
    { "bufferView": 1, "componentType": 5126, "count": 3, "type": "VEC4" }
  ],
  "nodes": [{ "name": "root" }],
  "animations": [{
    "name": "poisoned",
    "samplers": [{ "input": 0, "output": 1, "interpolation": "LINEAR" }],
    "channels": [{ "sampler": 0, "target": { "node": 0, "path": "rotation" } }]
  }],
  "scenes": [{ "nodes": [0] }],
  "scene": 0
}"#;

/// First and last keyframe times are NaN and +Inf; values remain valid.
const NONFINITE_TIME_GLTF: &str = r#"{
  "asset": { "version": "2.0" },
  "buffers": [{ "uri": "data:application/octet-stream;base64,AADAfwAAAD8AAIB/AAAAAAAAAAAAAAAAAACAPwAAAAAAAAAAAAAAAAAAgD8AAAAAAAAAAAAAAAAAAIA/", "byteLength": 60 }],
  "bufferViews": [
    { "buffer": 0, "byteOffset": 0, "byteLength": 12 },
    { "buffer": 0, "byteOffset": 12, "byteLength": 48 }
  ],
  "accessors": [
    { "bufferView": 0, "componentType": 5126, "count": 3, "type": "SCALAR", "min": [0], "max": [1] },
    { "bufferView": 1, "componentType": 5126, "count": 3, "type": "VEC4" }
  ],
  "nodes": [{ "name": "root" }],
  "animations": [{
    "name": "poisoned",
    "samplers": [{ "input": 0, "output": 1, "interpolation": "LINEAR" }],
    "channels": [{ "sampler": 0, "target": { "node": 0, "path": "rotation" } }]
  }],
  "scenes": [{ "nodes": [0] }],
  "scene": 0
}"#;

#[test]
fn malformed_track_counts_are_operator_errors_everywhere() {
    let dir = unique_temp_dir("count-mismatch-cli");
    let input = dir.path().join("bad.gltf");
    std::fs::write(&input, COUNT_MISMATCH_GLTF).expect("writes fixture");
    let out = dir.path().join("out.glb");

    let commands: [&[&str]; 3] = [
        &["measure", input.to_str().expect("utf-8 path")],
        &["lint", input.to_str().expect("utf-8 path")],
        &[
            "transform",
            input.to_str().expect("utf-8 path"),
            "-o",
            out.to_str().expect("utf-8 path"),
        ],
    ];
    for args in commands {
        let output = animsmith().args(args).output().expect("runs animsmith");
        assert_eq!(
            output.status.code(),
            Some(2),
            "{args:?}: stdout:\n{}\nstderr:\n{}",
            stdout(&output),
            stderr(&output)
        );
        assert!(
            stderr(&output).contains("malformed animation data"),
            "{args:?}: stderr:\n{}",
            stderr(&output)
        );
    }
}

#[test]
fn nan_key_times_lint_as_errors_and_never_crash() {
    let dir = unique_temp_dir("nan-time-cli");
    let input = dir.path().join("nan.gltf");
    std::fs::write(&input, NAN_TIME_GLTF).expect("writes fixture");

    // measure survives (exit 0): NaN is a semantic defect for lint to
    // judge, not a crash.
    let output = animsmith()
        .args(["measure", input.to_str().expect("utf-8 path")])
        .output()
        .expect("runs animsmith");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );

    // lint reports the nan error finding and exits 1.
    let output = animsmith()
        .args(["lint", input.to_str().expect("utf-8 path")])
        .output()
        .expect("runs animsmith");
    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    assert!(
        stdout(&output).contains("error[nan]") && stdout(&output).contains("non-finite key time"),
        "stdout:\n{}",
        stdout(&output)
    );
}

#[test]
fn non_finite_key_times_never_escape_as_schema_invalid_nulls() {
    let dir = unique_temp_dir("nonfinite-time-json");
    let input = dir.path().join("nonfinite.gltf");
    std::fs::write(&input, NONFINITE_TIME_GLTF).expect("writes fixture");

    for (command, expected_exit) in [("measure", 0), ("lint", 1)] {
        let output = animsmith()
            .args([command, input.to_str().unwrap(), "--format", "json"])
            .output()
            .expect("runs animsmith");
        assert_eq!(
            output.status.code(),
            Some(expected_exit),
            "{command} stderr:\n{}",
            stderr(&output)
        );
        let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
        assert_output_schema_valid(&json);
        assert_eq!(
            json["files"][0]["measurements"]["clips"]["poisoned"]["duration_s"],
            0.5
        );
    }
}

/// Every `--format json` path in this CLI treats a stdout it cannot write to
/// the same way: the write failure is diagnosed on stderr and the command's
/// own outcome code stands.
///
/// `lint` is the load-bearing case. Findings it really found must not be
/// relabelled as an operator error because the consumer hung up — `lint …
/// --format json | head` still exits `1`. `diff`'s exit `1` for movement it
/// really measured is the same claim.
///
/// The pipe's read end is dropped **before** the child is spawned, so its
/// stdout has no reader from the moment it exists: the write failure is a
/// property of the setup rather than a race against how quickly the child
/// reaches its write.
#[test]
fn a_closed_stdout_is_diagnosed_without_rewriting_any_json_command_outcome() {
    #[cfg(feature = "fbx")]
    let dir = unique_temp_dir("closed-stdout-json");

    #[cfg_attr(not(feature = "fbx"), expect(unused_mut))]
    let mut cases: Vec<(&str, Vec<String>, i32)> = vec![
        (
            "measure",
            vec![example_asset("clip.glb").display().to_string()],
            0,
        ),
        (
            "lint",
            vec![example_asset("clip-dirty.glb").display().to_string()],
            1,
        ),
        (
            "diff",
            vec![
                example_asset("clip.glb").display().to_string(),
                example_asset("walk.glb").display().to_string(),
            ],
            1,
        ),
    ];
    #[cfg(feature = "fbx")]
    cases.push((
        "convert",
        vec![
            example_asset("clip.glb").display().to_string(),
            "-o".to_owned(),
            dir.path().join("converted.glb").display().to_string(),
        ],
        0,
    ));

    for (command, args, expected_exit) in cases {
        let (reader, writer) = std::io::pipe().expect("creates a pipe");
        drop(reader);
        let output = animsmith()
            .arg(command)
            .args(&args)
            .args(["--format", "json"])
            .stdout(Stdio::from(writer))
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|error| panic!("spawns animsmith {command}: {error}"))
            .wait_with_output()
            .unwrap_or_else(|error| panic!("waits for animsmith {command}: {error}"));

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(expected_exit),
            "{command} must keep its own outcome when stdout is closed; stderr:\n{stderr}"
        );
        // Ours, not the OS's: the platform's wording for a reader-less pipe
        // is not this contract.
        assert!(
            stderr.starts_with("animsmith: cannot write JSON output to stdout"),
            "{command} stderr:\n{stderr}"
        );
    }
}

/// Human-readable output follows the same reporting boundary as JSON: losing
/// stdout is diagnosed, never panicked over, and never substitutes exit `2`
/// for the command's own success or finding/refusal status.
///
/// This matrix includes iterator-shaped renderers (`inspect`, `measure`, and
/// `diff`), whole-result text and Markdown (`lint`), and write-summary paths
/// (`transform` and, when enabled, `report`). The non-feature-gated cases are
/// also run by the required `--no-default-features` CLI gate.
#[test]
fn a_closed_stdout_preserves_text_and_markdown_command_outcomes() {
    let dir = unique_temp_dir("closed-stdout-text");
    let clean = example_asset("clip.glb").display().to_string();
    let dirty = example_asset("clip-dirty.glb").display().to_string();
    let other = example_asset("walk.glb").display().to_string();

    #[cfg_attr(not(feature = "report"), expect(unused_mut))]
    let mut cases: Vec<(&str, Vec<String>, i32)> = vec![
        ("inspect", vec!["inspect".to_owned(), clean.clone()], 0),
        (
            "measure text",
            vec![
                "measure".to_owned(),
                clean.clone(),
                "--format".to_owned(),
                "text".to_owned(),
            ],
            0,
        ),
        (
            "lint text refusal",
            vec![
                "lint".to_owned(),
                dirty.clone(),
                "--format".to_owned(),
                "text".to_owned(),
            ],
            1,
        ),
        (
            "lint markdown refusal",
            vec![
                "lint".to_owned(),
                dirty,
                "--format".to_owned(),
                "markdown".to_owned(),
            ],
            1,
        ),
        (
            "diff refusal",
            vec![
                "diff".to_owned(),
                clean.clone(),
                other,
                "--format".to_owned(),
                "text".to_owned(),
            ],
            1,
        ),
        (
            "transform summary",
            vec![
                "transform".to_owned(),
                clean.clone(),
                "-o".to_owned(),
                dir.path().join("transformed.glb").display().to_string(),
                "--hold-extend".to_owned(),
                "0.1".to_owned(),
            ],
            0,
        ),
    ];
    #[cfg(feature = "report")]
    cases.push((
        "report summary",
        vec![
            "report".to_owned(),
            clean,
            "-o".to_owned(),
            dir.path().join("report.html").display().to_string(),
        ],
        0,
    ));

    for (case, args, expected_exit) in cases {
        let (reader, writer) = std::io::pipe().expect("creates a pipe");
        drop(reader);
        let output = animsmith()
            .args(&args)
            .stdout(Stdio::from(writer))
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|error| panic!("spawns {case}: {error}"))
            .wait_with_output()
            .unwrap_or_else(|error| panic!("waits for {case}: {error}"));

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(expected_exit),
            "{case} must keep its own outcome when stdout is closed; stderr:\n{stderr}"
        );
        assert!(
            stderr.starts_with("animsmith: cannot write text output to stdout"),
            "{case} stderr:\n{stderr}"
        );
        assert!(!stderr.contains("panicked at"), "{case} stderr:\n{stderr}");
    }

    let (reader, writer) = std::io::pipe().expect("creates a pipe");
    drop(reader);
    let closed_both_input = example_asset("clip.glb");
    let status = animsmith()
        .args(["inspect", closed_both_input.to_str().unwrap()])
        .stdout(Stdio::from(writer))
        .stderr(Stdio::null())
        .status()
        .expect("runs inspect with both reporting streams unavailable");
    assert_eq!(
        status.code(),
        Some(0),
        "a closed diagnostic stream must not turn reporting into a panic"
    );
}

#[test]
fn closed_stdout_help_and_version_are_checked_successful_deliveries() {
    for (case, args) in [
        ("root help", vec!["--help"]),
        ("subcommand help", vec!["fix", "--help"]),
        ("version", vec!["--version"]),
    ] {
        let (reader, writer) = std::io::pipe().expect("creates a pipe");
        drop(reader);
        let output = animsmith()
            .args(args)
            .stdout(Stdio::from(writer))
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|error| panic!("spawns {case}: {error}"))
            .wait_with_output()
            .unwrap_or_else(|error| panic!("waits for {case}: {error}"));

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{case} remains a successful parser outcome; stderr:\n{stderr}"
        );
        assert!(
            stderr.starts_with("animsmith: cannot write text output to stdout"),
            "{case} stderr:\n{stderr}"
        );
        assert!(!stderr.contains("panicked at"), "{case} stderr:\n{stderr}");
    }
}

#[test]
fn closed_stdout_fix_with_multiple_reports_is_diagnosed_once() {
    let dir = unique_temp_dir("closed-stdout-fix-multiple");
    let input = dir.path().join("distinct-repairs.glb");
    write_distinct_repair_glb(&input);
    let (reader, writer) = std::io::pipe().expect("creates a pipe");
    drop(reader);
    let output = animsmith()
        .args([
            "fix",
            input.to_str().expect("utf-8 fixture path"),
            "--dry-run",
            "--repair",
            "quat-norm,quat-flip",
        ])
        .stdout(Stdio::from(writer))
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawns fix")
        .wait_with_output()
        .expect("waits for fix");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(1),
        "pending repairs remain findings; stderr:\n{stderr}"
    );
    assert_eq!(
        stderr
            .matches("animsmith: cannot write text output to stdout")
            .count(),
        1,
        "one attempted fix stream must produce one diagnostic:\n{stderr}"
    );
    assert!(!stderr.contains("panicked at"), "stderr:\n{stderr}");
}

// --- #30: exit-code, config-path, and inspect contract ---

fn write_config(dir: &std::path::Path, name: &str, toml: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, toml).expect("writes config");
    path
}

fn example_config() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/character.animsmith.toml")
}

#[test]
fn required_bones_cli_distinguishes_static_presence_missing_ambiguity_and_empty_skeletons() {
    let dir = unique_temp_dir("required-bones");
    let input = dir.path().join("static-rig.glb");
    let empty = dir.path().join("empty.glb");
    write_required_bones_glb(&input);
    write_empty_skeleton_glb(&empty);

    let clean = write_config(
        dir.path(),
        "clean.toml",
        "[rig]\nrequired_bones = [\"root\", \"weapon_socket\"]\n",
    );
    let clean_output = animsmith()
        .arg("--config")
        .arg(&clean)
        .args([
            "lint",
            input.to_str().expect("utf-8 input"),
            "--select",
            "required-bones",
            "--format",
            "json",
        ])
        .output()
        .expect("runs required-bones against static glTF");
    assert!(
        clean_output.status.success(),
        "stderr:\n{}",
        stderr(&clean_output)
    );
    let clean_json: Value = serde_json::from_slice(&clean_output.stdout).expect("valid JSON");
    let clean_check = clean_json["files"][0]["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|check| check["check_id"] == "required-bones")
        .expect("required-bones record");
    assert_eq!(clean_check["applicability"], "applicable");
    assert_eq!(clean_check["evaluation"], "complete");
    assert_eq!(clean_check["findings"], json!([]));
    assert_eq!(
        clean_check["evaluated_scopes"],
        json!([{ "code": "required_bone_presence" }])
    );
    assert_output_schema_valid(&clean_json);

    let failing = write_config(
        dir.path(),
        "failing.toml",
        "[rig]\nrequired_bones = [\"weapon_socket\", \"weapon_socket\", \"missing_socket\", \"duplicate\"]\n",
    );
    let failing_output = animsmith()
        .arg("--config")
        .arg(&failing)
        .args([
            "lint",
            input.to_str().expect("utf-8 input"),
            "--select",
            "required-bones",
            "--format",
            "json",
        ])
        .output()
        .expect("runs required-bones failure cases");
    assert_eq!(
        failing_output.status.code(),
        Some(1),
        "stderr:\n{}",
        stderr(&failing_output)
    );
    let failing_json: Value = serde_json::from_slice(&failing_output.stdout).expect("valid JSON");
    let findings = failing_json["files"][0]["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|check| check["check_id"] == "required-bones")
        .expect("required-bones record")["findings"]
        .as_array()
        .expect("findings");
    assert_eq!(findings.len(), 2, "duplicate declarations must deduplicate");
    assert!(findings.iter().any(|finding| {
        finding["bone"] == "missing_socket"
            && finding["message"]
                .as_str()
                .is_some_and(|message| message.contains("does not exist"))
    }));
    assert!(findings.iter().any(|finding| {
        finding["bone"] == "duplicate"
            && finding["message"]
                .as_str()
                .is_some_and(|message| message.contains("ambiguous"))
    }));
    assert_output_schema_valid(&failing_json);

    let empty_output = animsmith()
        .arg("--config")
        .arg(&clean)
        .args([
            "lint",
            empty.to_str().expect("utf-8 empty input"),
            "--select",
            "required-bones",
            "--format",
            "json",
        ])
        .output()
        .expect("runs required-bones against empty skeleton");
    assert!(
        empty_output.status.success(),
        "stderr:\n{}",
        stderr(&empty_output)
    );
    let empty_json: Value = serde_json::from_slice(&empty_output.stdout).expect("valid JSON");
    let empty_check = empty_json["files"][0]["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|check| check["check_id"] == "required-bones")
        .expect("required-bones record");
    assert_eq!(
        empty_check["evaluation"], "not_evaluated",
        "a skeleton-unavailable gap completes no structural work: {empty_json:#}"
    );
    assert_eq!(empty_check["findings"], json!([]));
    let gaps = empty_check["gaps"].as_array().expect("coverage gaps");
    assert_eq!(gaps.len(), 1);
    assert_eq!(gaps[0]["code"], "skeleton_unavailable");
    assert_eq!(
        gaps[0]["scope"],
        json!({ "code": "required_bone_presence" })
    );
    assert_output_schema_valid(&empty_json);
}

#[cfg(feature = "fbx")]
#[test]
fn required_bones_cli_lints_direct_fbx_input() {
    let dir = unique_temp_dir("required-bones-fbx");
    let config = write_config(
        dir.path(),
        "fbx.toml",
        "[rig]\nrequired_bones = [\"tri\", \"missing_socket\"]\n",
    );
    let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../animsmith-fbx/testdata/rigged_triangle.fbx");
    let output = animsmith()
        .arg("--config")
        .arg(&config)
        .args([
            "lint",
            input.to_str().expect("utf-8 FBX fixture"),
            "--select",
            "required-bones",
            "--format",
            "json",
        ])
        .output()
        .expect("lints direct FBX input");
    assert_eq!(
        output.status.code(),
        Some(1),
        "stderr:\n{}",
        stderr(&output)
    );
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    let findings = json["files"][0]["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|check| check["check_id"] == "required-bones")
        .expect("required-bones record")["findings"]
        .as_array()
        .expect("findings");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0]["bone"], "missing_socket");
    assert_output_schema_valid(&json);
}

#[test]
fn lint_file_with_only_coverage_gaps_exits_zero() {
    let output = animsmith()
        .args(["lint", fixture("rig.gltf").to_str().expect("utf-8 path")])
        .output()
        .expect("runs animsmith");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&output)
    );
    assert!(
        stdout(&output).contains("coverage[bind-pose]"),
        "stdout:\n{}",
        stdout(&output)
    );
    assert!(
        stdout(&output).contains("0 error(s)"),
        "stdout:\n{}",
        stdout(&output)
    );
}

#[test]
fn lint_markdown_renders_findings_for_failing_asset() {
    let dir = unique_temp_dir("markdown-findings");
    let input = dir.path().join("dirty.glb");
    write_distinct_repair_glb(&input); // quat-norm error + quat-flip warning
    let path = input.to_str().expect("utf-8 path");

    let output = animsmith()
        .args(["lint", path, "--format", "markdown"])
        .output()
        .expect("runs animsmith");
    // A failing asset exits 1 in markdown mode just like text/json — the
    // renderer must not swallow the content-failure status.
    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    let out = stdout(&output);

    // Presentation surface: a heading, the per-clip table header, the
    // collapsible section, and both findings' check ids and severities.
    assert!(out.contains("## animsmith lint"), "stdout:\n{out}");
    assert!(
        out.contains("| Severity | Check | Location | Measured | Expected | Message |"),
        "stdout:\n{out}"
    );
    assert!(out.contains("<details"), "stdout:\n{out}");
    assert!(out.contains("#### clip `sway`"), "stdout:\n{out}");
    assert!(out.contains("`quat-norm`"), "stdout:\n{out}");
    assert!(out.contains("`quat-flip`"), "stdout:\n{out}");
    // End-to-end smoke check that the summary footer reaches stdout;
    // per-branch tallies/grouping/escaping are pinned by the render unit
    // tests in the binary crate. Anchor on the footer's `**N file**`
    // prefix so this matches the aggregate line, not the per-file header.
    assert!(
        out.contains("**1 file** — ❌ 1 error(s) · ⚠️ 1 warning(s)"),
        "stdout:\n{out}"
    );
}

#[test]
fn lint_markdown_escapes_unicode_paragraph_separator_at_the_cli_boundary() {
    let dir = unique_temp_dir("markdown-presentation-controls");
    let input = dir.path().join("hostile.glb");
    let hostile = "forged\nline\u{1b}[31m\u{2028}left\u{2029}right\u{202e}";
    write_hostile_glb(&input, hostile, true);

    let output = animsmith()
        .arg("lint")
        .arg(&input)
        .args(["--format", "markdown"])
        .output()
        .expect("runs animsmith");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    let markdown = stdout(&output);
    assert!(
        !markdown.contains('\u{2028}') && !markdown.contains('\u{2029}'),
        "raw Unicode separators leaked into Markdown:\n{markdown}"
    );
    assert!(
        !markdown.contains('\u{202e}') && markdown.contains("\\u{202e}"),
        "bidi override was not rendered visibly:\n{markdown}"
    );
    assert!(
        markdown.contains("left right"),
        "the paragraph separator was deleted instead of flattened:\n{markdown}"
    );
}

#[test]
fn lint_markdown_surfaces_nonblocking_coverage_gaps() {
    let dir = unique_temp_dir("markdown-clean");
    let input = dir.path().join("clean.glb");
    write_clean_glb(&input);
    let path = input.to_str().expect("utf-8 path");

    let output = animsmith()
        .args(["lint", path, "--format", "markdown"])
        .output()
        .expect("runs animsmith");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&output)
    );
    let out = stdout(&output);
    assert!(out.contains("0 error(s)"), "stdout:\n{out}");
    assert!(out.contains("coverage gap(s)"), "stdout:\n{out}");
    assert!(
        out.contains("`insufficient_rotation_evidence`"),
        "stdout:\n{out}"
    );
}

#[test]
fn lint_warnings_pass_but_deny_warnings_fails() {
    let dir = unique_temp_dir("deny-warnings");
    let input = dir.path().join("flipped.glb");
    write_flipped_glb(&input); // quat-flip → warning
    let path = input.to_str().expect("utf-8 path");

    // Warnings alone are exit 0.
    let output = animsmith().args(["lint", path]).output().expect("runs");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&output)
    );
    assert!(
        stdout(&output).contains("quat-flip"),
        "stdout:\n{}",
        stdout(&output)
    );

    // --deny-warnings promotes the exit to 1.
    let output = animsmith()
        .args(["lint", path, "--deny-warnings"])
        .output()
        .expect("runs");
    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
}

/// Declared work on an unresolved rig is a typed, nonblocking coverage gap,
/// never a content finding. `--deny-warnings` does not change that policy.
#[test]
fn lint_unresolved_roles_serialize_as_a_gap_and_exit_zero() {
    let dir = unique_temp_dir("coverage-gap");
    let input = dir.path().join("sway.glb");
    write_clean_glb(&input); // root->spine rig: no hips/foot roles resolve
    let config = dir.path().join("animsmith.toml");
    std::fs::write(
        &config,
        "[clips.sway]\nloop = true\n\n[checks.loop-closure]\nseverity = \"off\"\n\n[checks.loop-seam-vel]\nseverity = \"off\"\n",
    )
    .expect("writes config");

    for deny in [false, true] {
        let mut args = vec![
            "--config",
            config.to_str().expect("utf-8 config path"),
            "lint",
            input.to_str().expect("utf-8 input path"),
            "--format",
            "json",
        ];
        if deny {
            args.push("--deny-warnings");
        }
        let output = animsmith().args(&args).output().expect("runs animsmith");
        assert_eq!(
            output.status.code(),
            Some(0),
            "coverage gaps must not fail the run (deny-warnings: {deny}):\n{}",
            stderr(&output)
        );
        let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
        assert_eq!(json["summary"]["findings"]["note"], 0, "{json:#}");
        let loop_seam = json["files"][0]["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|check| check["check_id"] == "loop-seam")
            .unwrap();
        assert_eq!(loop_seam["evaluation"], "not_evaluated", "{json:#}");
        assert_eq!(loop_seam["findings"], json!([]), "{json:#}");
        assert_eq!(loop_seam["gaps"][0]["code"], "roles_unresolved", "{json:#}");
    }
}

#[test]
fn lint_text_groups_repeated_per_clip_coverage_gaps() {
    let dir = unique_temp_dir("grouped-coverage-gap");
    let input = dir.path().join("sways.glb");
    write_two_clip_clean_glb(&input);
    let config = dir.path().join("animsmith.toml");
    std::fs::write(&config, "[clips.\"sway*\"]\nloop = true\n").expect("writes config");

    let output = animsmith()
        .args(["--config"])
        .arg(&config)
        .arg("lint")
        .arg(&input)
        .args(["--select", "loop-seam"])
        .output()
        .expect("runs animsmith");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&output)
    );
    let text = stdout(&output);
    assert_eq!(text.matches("coverage[loop-seam]").count(), 1, "{text}");
    assert!(text.contains("roles_unresolved ×2"), "{text}");
    assert!(text.contains("sway, sway_b"), "{text}");
    assert!(text.contains("2 coverage gap(s)"), "{text}");

    let json_output = animsmith()
        .args(["--config"])
        .arg(&config)
        .arg("lint")
        .arg(&input)
        .args(["--select", "loop-seam", "--format", "json"])
        .output()
        .expect("runs JSON lint");
    assert_eq!(
        json_output.status.code(),
        Some(0),
        "{}",
        stderr(&json_output)
    );
    let json: Value = serde_json::from_slice(&json_output.stdout).expect("valid JSON");
    let loop_seam = json["files"][0]["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["check_id"] == "loop-seam")
        .unwrap();
    let subjects: Vec<_> = loop_seam["gaps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|gap| gap["scope"]["subject"].as_str().unwrap())
        .collect();
    assert_eq!(subjects, ["sway", "sway_b"]);
}

#[test]
fn lint_allow_suppresses_a_check() {
    let dir = unique_temp_dir("allow");
    let input = dir.path().join("flipped.glb");
    write_flipped_glb(&input);
    let path = input.to_str().expect("utf-8 path");

    // Positive control: quat-flip fires on this fixture without --allow.
    let baseline = animsmith()
        .args(["lint", path, "--deny-warnings"])
        .output()
        .expect("runs");
    assert_eq!(baseline.status.code(), Some(1), "warning gate baseline");
    assert!(
        stdout(&baseline).contains("quat-flip"),
        "fixture no longer produces quat-flip; suppression test would be vacuous:\n{}",
        stdout(&baseline)
    );

    // With --allow, the same finding is gone.
    let output = animsmith()
        .args(["lint", path, "--allow", "quat-flip", "--deny-warnings"])
        .output()
        .expect("runs");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&output)
    );
    assert!(
        !stdout(&output).contains("quat-flip"),
        "allowed check still reported:\n{}",
        stdout(&output)
    );

    let markdown = animsmith()
        .args([
            "lint",
            path,
            "--format",
            "markdown",
            "--allow",
            "quat-flip",
            "--deny-warnings",
        ])
        .output()
        .expect("runs Markdown renderer");
    assert_eq!(
        markdown.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&markdown)
    );
    assert!(
        !stdout(&markdown).contains("quat-flip"),
        "allowed check still present in Markdown:\n{}",
        stdout(&markdown)
    );
}

#[test]
fn lint_unknown_select_is_operator_error() {
    let output = animsmith()
        .args([
            "lint",
            fixture("rig.gltf").to_str().expect("utf-8 path"),
            "--select",
            "no-such-check",
        ])
        .output()
        .expect("runs animsmith");
    assert_eq!(
        output.status.code(),
        Some(2),
        "stdout:\n{}",
        stdout(&output)
    );
    let err = stderr(&output);
    assert!(
        err.contains("unknown check 'no-such-check'"),
        "stderr:\n{err}"
    );
    // The error also lists the known check ids so the user can correct
    // the typo without reading the docs.
    assert!(
        err.contains("known:") && err.contains("quat-flip"),
        "error should list known check ids:\n{err}"
    );
}

#[test]
fn lint_missing_file_is_operator_error() {
    let output = animsmith()
        .args(["lint", "/no/such/file.glb"])
        .output()
        .expect("runs animsmith");
    assert_eq!(
        output.status.code(),
        Some(2),
        "stdout:\n{}",
        stdout(&output)
    );
    // Exit 2 is the catch-all; pin that it failed at load (the right
    // cause) rather than arg parsing or config. The loader reads the file
    // itself now, so a missing file is an I/O error, not a parse error.
    // The OS "file not found" text differs across platforms, so anchor on
    // the stable prefix.
    assert!(
        stderr(&output).contains("failed to read"),
        "stderr:\n{}",
        stderr(&output)
    );
}

#[test]
fn lint_bad_config_is_operator_error() {
    let dir = unique_temp_dir("bad-config");
    let config = write_config(dir.path(), "bad.toml", "not valid = = toml [[[\n");
    let output = animsmith()
        .args([
            "--config",
            config.to_str().expect("utf-8 path"),
            "lint",
            fixture("rig.gltf").to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("runs animsmith");
    assert_eq!(
        output.status.code(),
        Some(2),
        "stdout:\n{}",
        stdout(&output)
    );
    assert!(
        stderr(&output).contains("bad config"),
        "stderr:\n{}",
        stderr(&output)
    );
}

#[test]
fn invalid_sync_group_tolerances_are_operator_errors() {
    let dir = unique_temp_dir("invalid-sync-group-tolerance");
    for (name, field, value) in [
        ("negative-duration", "max_duration_delta_s", "-0.001"),
        ("nonfinite-fps", "max_fps_delta", "nan"),
        ("negative-frame-count", "max_frame_count_delta", "-1"),
    ] {
        let config = write_config(
            dir.path(),
            &format!("{name}.toml"),
            &format!(
                "[sync_groups.ring]\nclips = [\"walk\", \"run\"]\nmax_duration_delta_s = {}\nmax_frame_count_delta = {}\nmax_fps_delta = {}\n",
                if field == "max_duration_delta_s" {
                    value
                } else {
                    "0.001"
                },
                if field == "max_frame_count_delta" {
                    value
                } else {
                    "0"
                },
                if field == "max_fps_delta" {
                    value
                } else {
                    "0.01"
                },
            ),
        );
        let output = animsmith()
            .arg("--config")
            .arg(&config)
            .args([
                "lint",
                fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
                "--select",
                "sync-group",
            ])
            .output()
            .expect("runs animsmith");
        assert_eq!(
            output.status.code(),
            Some(2),
            "{name}: stdout:\n{}\nstderr:\n{}",
            stdout(&output),
            stderr(&output)
        );
        let error = stderr(&output);
        assert!(error.starts_with("animsmith: "), "{name}: {error}");
        assert!(error.contains(field), "{name}: {error}");
    }
}

#[test]
fn invalid_time_complement_settings_are_operator_errors() {
    let dir = unique_temp_dir("invalid-time-complement-setting");
    for (name, field, advantage, amplitude) in [
        (
            "negative-advantage",
            "min_reflected_time_advantage",
            "-0.01",
            "0.03",
        ),
        (
            "advantage-over-one",
            "min_reflected_time_advantage",
            "1.01",
            "0.03",
        ),
        (
            "nonfinite-advantage",
            "min_reflected_time_advantage",
            "nan",
            "0.03",
        ),
        ("negative-amplitude", "min_lr_amplitude_m", "0.25", "-0.01"),
        ("nonfinite-amplitude", "min_lr_amplitude_m", "0.25", "nan"),
    ] {
        let config = write_config(
            dir.path(),
            &format!("{name}.toml"),
            &format!(
                "[sync_groups.ring]\nclips = [\"walk\", \"run\"]\nmax_duration_delta_s = 0.001\nmax_frame_count_delta = 0\nmax_fps_delta = 0.01\n\n[sync_groups.ring.time_complement]\nmin_reflected_time_advantage = {advantage}\nmin_lr_amplitude_m = {amplitude}\n"
            ),
        );
        let output = animsmith()
            .arg("--config")
            .arg(&config)
            .args([
                "lint",
                fixture("rig.gltf").to_str().expect("utf-8 fixture path"),
                "--select",
                "time-complement",
            ])
            .output()
            .expect("runs animsmith");
        assert_eq!(
            output.status.code(),
            Some(2),
            "{name}: stdout:\n{}\nstderr:\n{}",
            stdout(&output),
            stderr(&output)
        );
        let error = stderr(&output);
        assert!(error.starts_with("animsmith: "), "{name}: {error}");
        assert!(error.contains(field), "{name}: {error}");
    }
}

/// The `--config` TOML path is otherwise only reached through the CLI:
/// a config that disables `quat-flip` must suppress it on a flipped
/// clip, proving `toml::from_str` → `Config` → severity handling works
/// end to end.
#[test]
fn config_toml_path_drives_check_behaviour() {
    let dir = unique_temp_dir("config-toml");
    let input = dir.path().join("flipped.glb");
    write_flipped_glb(&input);
    let path = input.to_str().expect("utf-8 path");
    let config = write_config(
        dir.path(),
        "animsmith.toml",
        "[checks.quat-flip]\nseverity = \"off\"\n",
    );

    // Positive control: without the config, quat-flip fires.
    let baseline = animsmith().args(["lint", path]).output().expect("runs");
    assert!(
        stdout(&baseline).contains("quat-flip"),
        "fixture no longer produces quat-flip; the config test would be vacuous:\n{}",
        stdout(&baseline)
    );

    // The TOML config turns it off end to end.
    let output = animsmith()
        .args([
            "--config",
            config.to_str().expect("utf-8 path"),
            "lint",
            path,
        ])
        .output()
        .expect("runs animsmith");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&output)
    );
    assert!(
        !stdout(&output).contains("quat-flip"),
        "off check still reported via TOML config:\n{}",
        stdout(&output)
    );
}

#[test]
fn duration_toml_pins_report_structured_evidence() {
    let dir = unique_temp_dir("duration-pin");
    let input = dir.path().join("sway.glb");
    write_clean_glb(&input);
    for (name, text) in [
        (
            "glob.toml",
            "[clips.\"sway*\"]\nduration_s = { value = 1.25, tolerance = 0.125 }\n",
        ),
        (
            "exact.toml",
            "[clips.\"sway*\"]\nduration_s = { value = 1.0, tolerance = 0.0 }\n\
             [clips.sway]\nduration_s = { value = 1.25, tolerance = 0.125 }\n",
        ),
    ] {
        let config = write_config(dir.path(), name, text);
        let output = animsmith()
            .args(["--config"])
            .arg(&config)
            .args(["lint"])
            .arg(&input)
            .args(["--select", "duration-sanity", "--format", "json"])
            .output()
            .expect("runs animsmith");
        assert_eq!(
            output.status.code(),
            Some(1),
            "stdout:\n{}\nstderr:\n{}",
            stdout(&output),
            stderr(&output)
        );
        let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
        let duration = json["files"][0]["checks"]
            .as_array()
            .expect("checks array")
            .iter()
            .find(|check| check["check_id"] == "duration-sanity")
            .expect("duration-sanity record");
        let findings = duration["findings"].as_array().expect("findings array");
        assert_eq!(findings.len(), 1, "{json:#}");
        assert_eq!(findings[0]["check_id"], "duration-sanity");
        assert_eq!(findings[0]["severity"], "error");
        assert_eq!(findings[0]["clip"], "sway");
        assert_eq!(findings[0]["measured"], 1.0);
        assert_eq!(findings[0]["expected"], 1.25);
    }
}

#[test]
fn invalid_duration_pin_from_toml_is_an_explicit_error() {
    let dir = unique_temp_dir("invalid-duration-pin");
    let input = dir.path().join("sway.glb");
    write_clean_glb(&input);
    let config = write_config(
        dir.path(),
        "animsmith.toml",
        "[clips.sway]\nduration_s = { value = nan, tolerance = 0.125 }\n",
    );
    let output = animsmith()
        .args(["--config"])
        .arg(&config)
        .args(["lint"])
        .arg(&input)
        .args(["--select", "duration-sanity", "--format", "json"])
        .output()
        .expect("runs animsmith");
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    let finding = json["files"][0]["checks"]
        .as_array()
        .expect("checks array")
        .iter()
        .find(|check| check["check_id"] == "duration-sanity")
        .expect("duration-sanity record")["findings"][0]
        .clone();
    assert_eq!(finding["severity"], "error");
    assert!(
        finding["message"]
            .as_str()
            .expect("message")
            .contains("invalid declared duration pin")
    );
}

#[test]
fn loop_continuity_clip_caps_layer_global_glob_and_exact_contracts() {
    let dir = unique_temp_dir("clip-loop-caps");
    let input = fixture("rig.gltf");
    for (name, text, expected_position, expected_rotation) in [
        (
            "global.toml",
            "[checks.loop-closure]\nmax_position_delta_m = 0.5\nmax_rotation_delta_deg = 80.0\n\n[clips.walk]\nloop = true\n",
            0.5,
            80.0,
        ),
        (
            "glob.toml",
            "[checks.loop-closure]\nmax_position_delta_m = 0.5\nmax_rotation_delta_deg = 80.0\n\n[clips.\"wa*\"]\nloop = true\nmax_loop_position_delta_m = 0.75\nmax_loop_rotation_delta_deg = 85.0\n",
            0.75,
            85.0,
        ),
        (
            "exact.toml",
            "[checks.loop-closure]\nmax_position_delta_m = 0.5\nmax_rotation_delta_deg = 80.0\n\n[clips.\"wa*\"]\nloop = true\nmax_loop_position_delta_m = 0.75\nmax_loop_rotation_delta_deg = 85.0\n\n[clips.walk]\nmax_loop_rotation_delta_deg = 87.0\n",
            0.75,
            87.0,
        ),
    ] {
        let config = write_config(dir.path(), name, text);
        let output = animsmith()
            .args(["--config"])
            .arg(&config)
            .args(["lint"])
            .arg(&input)
            .args(["--select", "loop-closure", "--format", "json"])
            .output()
            .expect("runs animsmith");
        assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
        let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
        let closure = json["files"][0]["checks"]
            .as_array()
            .expect("checks array")
            .iter()
            .find(|check| check["check_id"] == "loop-closure")
            .expect("loop-closure record");
        let findings = closure["findings"].as_array().expect("findings array");
        assert_eq!(findings.len(), 2, "{json:#}");
        assert_eq!(findings[0]["expected"], expected_position, "{json:#}");
        assert_eq!(findings[1]["expected"], expected_rotation, "{json:#}");
    }
}

#[test]
fn invalid_clip_loop_caps_are_operator_errors() {
    let dir = unique_temp_dir("invalid-clip-loop-cap");
    let input = fixture("rig.gltf");
    for (name, key, value) in [
        ("negative", "max_loop_position_delta_m", "-0.01"),
        ("nan", "max_loop_rotation_delta_deg", "nan"),
        ("infinite", "max_loop_velocity_delta_mps", "inf"),
        (
            "angular-infinite",
            "max_loop_angular_velocity_delta_degps",
            "inf",
        ),
    ] {
        let config = write_config(
            dir.path(),
            &format!("{name}.toml"),
            &format!("[clips.walk]\nloop = true\n{key} = {value}\n"),
        );
        let output = animsmith()
            .args(["--config"])
            .arg(&config)
            .args(["lint"])
            .arg(&input)
            .output()
            .expect("runs animsmith");
        assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
        assert!(
            stderr(&output).contains("must be a finite non-negative number"),
            "{name}: {}",
            stderr(&output)
        );
    }
}

/// The shipped example config must parse verbatim — otherwise it drifts
/// from the schema and fails users at runtime while CI stays green.
#[test]
fn example_config_parses_verbatim() {
    let config = example_config();
    assert!(config.exists(), "example config missing at {config:?}");
    let output = animsmith()
        .args([
            "--config",
            config.to_str().expect("utf-8 path"),
            "inspect",
            fixture("rig.gltf").to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("runs animsmith");
    assert_eq!(
        output.status.code(),
        Some(0),
        "example config did not parse:\nstderr:\n{}",
        stderr(&output)
    );
}

#[test]
fn inspect_reports_clip_and_profile() {
    let output = animsmith()
        .args(["inspect", fixture("rig.gltf").to_str().expect("utf-8 path")])
        .output()
        .expect("runs animsmith");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&output)
    );
    let out = stdout(&output);
    // Distinctive clip detail: the fixture's one clip, its duration and
    // track/key counts — pins that inspect actually read the file, not
    // just that it printed a static template.
    assert!(
        out.contains("walk: 1.000s, 2 tracks, 3 keys max"),
        "clip summary missing/changed:\n{out}"
    );
    assert!(out.contains("rig profile:"), "no profile line:\n{out}");
    assert!(
        out.contains("skeleton: 3 bones"),
        "no skeleton line:\n{out}"
    );
}

#[test]
fn inspect_reports_every_selectable_mesh_instance_and_material() {
    let dir = unique_temp_dir("inspect-mesh-instances");
    let input = dir.path().join("multi-mesh.glb");
    write_multi_mesh_glb(&input);

    let output = animsmith()
        .args(["inspect", input.to_str().expect("utf-8 path")])
        .output()
        .expect("runs animsmith");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&output)
    );
    let out = stdout(&output);
    let inventory = &out[out.find("materials:").expect("material inventory")..];
    assert_eq!(
        inventory,
        concat!(
            "materials: 5\n",
            "  #0 \"body-material\"\n",
            "  #1 \"prop-material\"\n",
            "  #2 \" duplicate-material \" [ambiguous: 3 materials share this name]\n",
            "  #3 \" duplicate-material \" [ambiguous: 3 materials share this name]\n",
            "  #4 \" duplicate-material \" [ambiguous: 3 materials share this name]\n",
            "mesh instances: 5\n",
            "  node \"body-node\"\n",
            "    source node: #0\n",
            "    mesh: #0 \"body-mesh\" (source mesh #0)\n",
            "    skin: unskinned\n",
            "    primitive #0: material #0 \"body-material\"\n",
            "  node \"prop-node\"\n",
            "    source node: #1\n",
            "    mesh: #1 \"prop-mesh\" (source mesh #1)\n",
            "    skin: unskinned\n",
            "    primitive #0: material #1 \"prop-material\"\n",
            "  node \" duplicate-node \" [ambiguous: 3 skeleton nodes share this name]\n",
            "    source node: #2\n",
            "    mesh: #0 \"body-mesh\" (source mesh #0)\n",
            "    skin: unskinned\n",
            "    primitive #0: material #0 \"body-material\"\n",
            "  node \" duplicate-node \" [ambiguous: 3 skeleton nodes share this name]\n",
            "    source node: #3\n",
            "    mesh: #0 \"body-mesh\" (source mesh #0)\n",
            "    skin: unskinned\n",
            "    primitive #0: material #0 \"body-material\"\n",
            "  node \" duplicate-node \" [ambiguous: 3 skeleton nodes share this name]\n",
            "    source node: #4\n",
            "    mesh: #0 \"body-mesh\" (source mesh #0)\n",
            "    skin: unskinned\n",
            "    primitive #0: material #0 \"body-material\"\n",
            "clips: 0\n",
        )
    );
}
