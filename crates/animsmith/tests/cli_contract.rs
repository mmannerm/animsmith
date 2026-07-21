use animsmith_core::glam::{Quat, Vec3};
use animsmith_core::model::*;
use animsmith_gltf::fix::{FixSession, Repair as GltfRepair};
use animsmith_testkit::{quats_from_angles, scaled_quat, two_bone_rotation_doc};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::process::{Command, Output};

const OUTPUT_SCHEMA_ID: &str = "urn:animsmith:schema:output:2";
const MEASUREMENTS_SCHEMA_ID: &str = "urn:animsmith:schema:measurements:1";
const OUTPUT_SCHEMA: &str = include_str!("../../../docs/schemas/output-v2.schema.json");
const MEASUREMENTS_SCHEMA: &str = include_str!("../../../docs/schemas/measurements-v1.schema.json");
const EXPECTED_CHECK_IDS: [&str; 16] = [
    "nan",
    "time-monotonic",
    "quat-norm",
    "quat-flip",
    "duration-sanity",
    "scale-keys",
    "constant-track",
    "missing-bones",
    "frozen-bone",
    "loop-seam",
    "root-motion-speed",
    "gait-group",
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
        "output must satisfy the published v2 schemas:\n{}\ninstance: {instance:#}",
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

fn unique_temp_dir(name: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(&format!("animsmith-cli-{name}-"))
        .tempdir()
        .expect("creates temp dir")
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

fn write_two_clip_clean_glb(path: &std::path::Path) {
    let mut doc = sway_doc(false);
    let mut second = doc.clips[0].clone();
    second.name = "sway_b".into();
    doc.clips.push(second);
    animsmith_gltf::write::write(&doc, path).expect("writes two-clip fixture");
}

fn write_hostile_measure_glb(path: &std::path::Path, hostile: &str) {
    let mut doc = sway_doc(false);
    doc.clips[0].name = hostile.into();
    doc.assets.meshes.push(MeshAsset {
        name: hostile.into(),
        node: 0,
        primitives: vec![Primitive {
            positions: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
            ],
            ..Primitive::default()
        }],
        ..MeshAsset::default()
    });
    animsmith_gltf::write::write(&doc, path).expect("writes hostile-name fixture");
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
        "schema_version": 2,
        "schema": OUTPUT_SCHEMA_ID,
        "command": "measure",
        "files": [{
            "path": "fixture.gltf",
            "rig": { "profile": "unknown" },
            "measurements": {
                "schema_version": 1,
                "schema": MEASUREMENTS_SCHEMA_ID,
                "clips": {
                    "walk": {
                        "duration_s": duration_s,
                        "frame_count": 31,
                        "animated_bones": [],
                        "bone_rotation_range_deg": {}
                    }
                }
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
    assert!(out.contains("diff"));

    // One-line summaries come from the doc comments (clap derives
    // `about` from the first line); pin them so description drift is
    // visible.
    assert!(out.contains("Repair safe mechanical glTF/GLB defects"));
    assert!(out.contains("Apply mechanical clip transforms"));
    assert!(out.contains("Compare animation measurements"));

    assert_eq!(out.contains("\n  convert "), cfg!(feature = "fbx"), "{out}");
    assert_eq!(
        out.contains("\n  report "),
        cfg!(feature = "report"),
        "{out}"
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
    assert_eq!(json["schema_version"], 2);
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
    assert_eq!(files[0]["measurements"]["schema_version"], 1);
    assert_eq!(files[0]["measurements"]["schema"], MEASUREMENTS_SCHEMA_ID);
    assert!(files[0]["measurements"]["clips"]["walk"]["duration_s"].is_number());
}

#[test]
fn measure_text_escapes_controls_in_clip_and_mesh_names() {
    let dir = unique_temp_dir("measure-text-controls");
    let hostile = "forged\nline\u{1b}[31m";
    let input = dir.path().join("hostile.glb");
    write_hostile_measure_glb(&input, hostile);

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
    assert!(!text.contains(hostile), "raw controls leaked:\n{text}");
    assert_eq!(text.matches("\\n").count(), 2, "clip and mesh: {text}");
    assert_eq!(text.matches("\\u{1b}").count(), 2, "clip and mesh: {text}");
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

#[test]
fn embedded_contract_types_emit_the_published_v2_envelope() {
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
        animsmith_core::RigInfo::from_resolved(&doc, &roles)
            .expect("roles were resolved from this document"),
        checks,
        animsmith_core::MeasurementContract::new(
            animsmith_core::measure::measure_document(&grids, &roles, &config),
            animsmith_core::measure::measure_meshes(&doc.assets),
        ),
    );
    let envelope = animsmith_core::LintEnvelope::new(
        animsmith_core::ToolInfo::animsmith(
            env!("CARGO_PKG_VERSION"),
            animsmith_core::ToolSource::new(None, None),
        ),
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
    assert_eq!(json["schema_version"], 2);
    assert_eq!(json["schema"], OUTPUT_SCHEMA_ID);
    assert_eq!(json["tool"]["name"], "animsmith");
    assert_eq!(json["tool"]["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(json["command"], "lint");
    assert_eq!(json["summary"]["files"], 1);
    assert!(json["files"][0]["checks"].is_array());
    assert_eq!(json["files"][0]["measurements"]["schema_version"], 1);
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
    assert_evaluation_summary_matches_checks(&json);
    assert_output_schema_valid(&json);
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
    assert_eq!(json["schema_version"], 2);
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
    assert_eq!(json["schema_version"], 2);
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
    nested_version["files"][0]["measurements"]["schema_version"] = json!(2);
    assert!(!validator.is_valid(&nested_version));

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
                report["files"][0]["measurements"]["schema_version"] = json!(2);
                report
            },
            "measurement schema_version 2",
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
fn diff_rejects_json_without_schema_version() {
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
        stderr(&output).contains("regenerate it with"),
        "stderr:\n{}",
        stderr(&output)
    );
}

#[test]
fn diff_rejects_unsupported_schema_versions() {
    let dir = unique_temp_dir("diff-future-schema");
    let future = dir.path().join("future.json");
    for version in [3, 99] {
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
    }
}

#[test]
fn diff_rejects_envelope_without_files() {
    let dir = unique_temp_dir("diff-no-files");
    let report = dir.path().join("no-files.json");
    std::fs::write(
        &report,
        r#"{"schema_version":2,"schema":"urn:animsmith:schema:output:2","command":"measure"}"#,
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
    std::fs::write(&config, "[clips.sway]\nloop = true\n").expect("writes config");

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
