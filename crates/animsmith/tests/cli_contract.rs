use animsmith_core::glam::{Quat, Vec3};
use animsmith_core::model::*;
use serde_json::Value;
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

fn flipped_doc() -> Document {
    let angles = [0.0f32, 0.4, 0.8, 1.2, 1.6];
    let mut quats: Vec<Quat> = angles.iter().map(|&a| Quat::from_rotation_y(a)).collect();
    quats[1] = -quats[1];
    quats[3] = -quats[3];
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
    animsmith_gltf::write::write(&flipped_doc(), path).expect("writes flipped fixture");
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn fix_lists_stable_repairs_and_groups() {
    let output = animsmith()
        .args(["fix", "--list-repairs"])
        .output()
        .expect("runs animsmith");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("quat-flip"));
    assert!(out.contains("default"));
    assert!(out.contains("lossless"));
    assert!(out.contains("mechanical"));
    assert!(out.contains("all"));
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

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    assert!(
        stdout(&output).contains("would be fixed"),
        "stdout:\n{}",
        stdout(&output)
    );
    assert_eq!(before, std::fs::read(&input).expect("reads input"));
}

#[test]
fn fix_group_writes_output() {
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
            "--group",
            "default",
        ])
        .output()
        .expect("runs animsmith");

    assert!(output.status.success(), "stderr:\n{}", stderr(&output));
    assert!(output_path.exists());
    let report =
        animsmith_gltf::fix::inspect_quat_hemisphere(&output_path).expect("inspects fixed output");
    assert_eq!(report.total_flipped(), 0);
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
    assert_eq!(json["command"], "diff");
    assert_eq!(json["summary"]["deltas"], 0);
    assert_eq!(json["deltas"].as_array().expect("deltas array").len(), 0);
    assert!(json["inputs"]["before"].is_string());
    assert!(json["inputs"]["after"].is_string());
}
