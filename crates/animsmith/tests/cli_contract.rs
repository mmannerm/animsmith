use animsmith_core::glam::{Quat, Vec3};
use animsmith_core::model::*;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::process::{Command, Output};

fn animsmith() -> Command {
    Command::new(env!("CARGO_BIN_EXE_animsmith"))
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("testdata")
        .join(name)
}

fn unique_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("animsmith-cli-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("creates temp dir");
    dir
}

/// Analytic rotation sequence: consecutive y-rotations 0.4 rad apart,
/// so every adjacent pair has a positive dot product — the clean form
/// is exactly the un-negated sequence.
fn sway_quats(flipped: bool) -> Vec<Quat> {
    let angles = [0.0f32, 0.4, 0.8, 1.2, 1.6];
    let mut quats: Vec<Quat> = angles.iter().map(|&a| Quat::from_rotation_y(a)).collect();
    if flipped {
        quats[1] = -quats[1];
        quats[3] = -quats[3];
    }
    quats
}

fn sway_doc(flipped: bool) -> Document {
    let quats = sway_quats(flipped);
    Document {
        skeleton: Skeleton {
            bones: vec![
                Bone {
                    name: "root".into(),
                    parent: None,
                    rest: Transform::IDENTITY,
                    inverse_bind: None,
                },
                Bone {
                    name: "spine".into(),
                    parent: Some(0),
                    rest: Transform {
                        translation: Vec3::new(0.0, 0.5, 0.0),
                        ..Transform::IDENTITY
                    },
                    inverse_bind: None,
                },
            ],
        },
        clips: vec![Clip {
            name: "sway".into(),
            duration_s: 1.0,
            tracks: vec![Track {
                bone: 1,
                property: Property::Rotation,
                interpolation: Interpolation::Linear,
                times: vec![0.0, 0.25, 0.5, 0.75, 1.0],
                values: TrackValues::Quats(quats),
            }],
        }],
        source: SourceInfo::default(),
    }
}

fn write_flipped_glb(path: &std::path::Path) {
    animsmith_gltf::write::write(&sway_doc(true), path).expect("writes flipped fixture");
}

fn write_clean_glb(path: &std::path::Path) {
    animsmith_gltf::write::write(&sway_doc(false), path).expect("writes clean fixture");
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
        "schema_version": 1,
        "files": [{
            "path": "fixture.gltf",
            "rig": { "profile": "unknown" },
            "measurements": {
                "walk": {
                    "duration_s": duration_s,
                    "frame_count": 31,
                    "animated_bones": [],
                    "bone_rotation_range_deg": {}
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
    let input = dir.join("dirty.glb");
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
fn fix_dry_run_on_clean_input_exits_zero() {
    let dir = unique_temp_dir("fix-dry-run-clean");
    let input = dir.join("clean.glb");
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
    let input = dir.join("dirty.gltf");
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
    let input = dir.join("dirty.glb");
    let output_path = dir.join("fixed.glb");
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
fn fix_in_place_writes_selected_repair() {
    let dir = unique_temp_dir("fix-in-place");
    let input = dir.join("dirty.glb");
    write_flipped_glb(&input);
    assert_eq!(
        animsmith_gltf::fix::inspect_quat_hemisphere(&input)
            .expect("inspects dirty input")
            .total_flipped(),
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
        animsmith_gltf::fix::inspect_quat_hemisphere(&input)
            .expect("inspects fixed input")
            .total_flipped(),
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
fn version_starts_with_manifest_version() {
    let output = animsmith()
        .arg("--version")
        .output()
        .expect("runs animsmith");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
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
    assert_eq!(json["schema_version"], 1);
    assert_eq!(json["tool"]["name"], "animsmith");
    assert!(
        json["tool"]["version"]
            .as_str()
            .is_some_and(|s| s.starts_with(env!("CARGO_PKG_VERSION"))),
        "{json:#}"
    );
    assert_eq!(json["command"], "measure");
    assert_eq!(json["summary"]["files"], 1);
    assert_eq!(json["summary"]["findings"]["error"], 0);
    assert!(
        json["schema"]
            .as_str()
            .is_some_and(|s| s.ends_with("output-v1.schema.json"))
    );

    let files = json["files"].as_array().expect("files array");
    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["rig"]["profile"], "unknown");
    assert!(files[0]["findings"].is_null());
    assert!(files[0]["measurements"]["walk"]["duration_s"].is_number());
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
    assert_eq!(json["schema_version"], 1);
    assert!(
        json["schema"]
            .as_str()
            .is_some_and(|s| s.ends_with("output-v1.schema.json"))
    );
    assert_eq!(json["tool"]["name"], "animsmith");
    assert!(
        json["tool"]["version"]
            .as_str()
            .is_some_and(|s| s.starts_with(env!("CARGO_PKG_VERSION")))
    );
    assert_eq!(json["command"], "lint");
    assert_eq!(json["summary"]["files"], 1);
    assert!(json["files"][0]["findings"].is_array());
    assert!(json["files"][0]["measurements"]["walk"]["duration_s"].is_number());
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
    assert_eq!(json["schema_version"], 1);
    assert!(
        json["schema"]
            .as_str()
            .is_some_and(|s| s.ends_with("output-v1.schema.json"))
    );
    assert_eq!(json["tool"]["name"], "animsmith");
    assert!(
        json["tool"]["version"]
            .as_str()
            .is_some_and(|s| s.starts_with(env!("CARGO_PKG_VERSION")))
    );
    assert_eq!(json["command"], "diff");
    assert_eq!(json["summary"]["deltas"], 0);
    assert_eq!(json["deltas"].as_array().expect("deltas array").len(), 0);
    assert!(json["inputs"]["before"].is_string());
    assert!(json["inputs"]["after"].is_string());
}

#[test]
fn diff_accepts_single_file_measure_report_round_trip() {
    let dir = unique_temp_dir("diff-round-trip");
    let asset = fixture("rig.gltf");
    let report_path = dir.join("measure.json");

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
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    assert!(
        stdout(&output).contains("no significant movement"),
        "stdout:\n{}",
        stdout(&output)
    );
}

#[test]
fn diff_accepts_measurement_json_and_exits_one_for_deltas() {
    let dir = unique_temp_dir("diff-json-deltas");
    let before = dir.join("before.json");
    let after = dir.join("after.json");
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
    let json: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    assert_eq!(json["summary"]["deltas"].as_u64(), Some(1));
    assert_eq!(json["deltas"][0]["clip"], "walk");
    assert_eq!(json["deltas"][0]["metric"], "duration_s");
    assert_eq!(json["deltas"][0]["note"], "moved");
}

#[test]
fn diff_accepts_measurement_json_and_exits_zero_without_deltas() {
    let dir = unique_temp_dir("diff-json-clean");
    let before = dir.join("before.json");
    let after = dir.join("after.json");
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

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    assert!(stdout(&output).contains("no significant movement"));
}

#[test]
fn diff_rejects_json_without_schema_version() {
    let dir = unique_temp_dir("diff-bare-map");
    let bare = dir.join("bare.json");
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
    let future = dir.join("future.json");
    std::fs::write(
        &future,
        r#"{"schema_version": 99, "files": [{"measurements": {}}]}"#,
    )
    .expect("writes future report");

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
        stderr(&output).contains("schema_version 99"),
        "stderr:\n{}",
        stderr(&output)
    );
}

#[test]
fn diff_rejects_envelope_without_files() {
    let dir = unique_temp_dir("diff-no-files");
    let report = dir.join("no-files.json");
    std::fs::write(&report, r#"{"schema_version": 1}"#).expect("writes report");

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
    let input = dir.join("dirty.glb");
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
    assert_eq!(json["files"][0]["findings"][0]["severity"], "warning");

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

#[test]
fn malformed_track_counts_are_operator_errors_everywhere() {
    let dir = unique_temp_dir("count-mismatch-cli");
    let input = dir.join("bad.gltf");
    std::fs::write(&input, COUNT_MISMATCH_GLTF).expect("writes fixture");
    let out = dir.join("out.glb");

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
    let input = dir.join("nan.gltf");
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
fn lint_clean_file_exits_zero() {
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
        stdout(&output).contains("clean"),
        "stdout:\n{}",
        stdout(&output)
    );
}

#[test]
fn lint_warnings_pass_but_deny_warnings_fails() {
    let dir = unique_temp_dir("deny-warnings");
    let input = dir.join("flipped.glb");
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

#[test]
fn lint_allow_suppresses_a_check() {
    let dir = unique_temp_dir("allow");
    let input = dir.join("flipped.glb");
    write_flipped_glb(&input);
    let path = input.to_str().expect("utf-8 path");

    // Positive control: quat-flip fires on this fixture without --allow.
    let baseline = animsmith().args(["lint", path]).output().expect("runs");
    assert!(
        stdout(&baseline).contains("quat-flip"),
        "fixture no longer produces quat-flip; suppression test would be vacuous:\n{}",
        stdout(&baseline)
    );

    // With --allow, the same finding is gone.
    let output = animsmith()
        .args(["lint", path, "--allow", "quat-flip"])
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
    assert!(
        stderr(&output).contains("unknown check 'no-such-check'"),
        "stderr:\n{}",
        stderr(&output)
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
    // Exit 2 is the catch-all; pin that it failed for the right reason.
    assert!(
        stderr(&output).contains("No such file"),
        "stderr:\n{}",
        stderr(&output)
    );
}

#[test]
fn lint_bad_config_is_operator_error() {
    let dir = unique_temp_dir("bad-config");
    let config = write_config(&dir, "bad.toml", "not valid = = toml [[[\n");
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
    let input = dir.join("flipped.glb");
    write_flipped_glb(&input);
    let path = input.to_str().expect("utf-8 path");
    let config = write_config(
        &dir,
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
