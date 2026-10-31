//! Real command refusal/publication boundaries for self-authored cubic motion.
use animsmith_core::glam::{Quat, Vec3};
use animsmith_core::model::*;
use serde_json::Value;
use std::path::Path;
use std::process::{Command, Output};

fn source(path: &Path, outgoing: f32, incoming: f32) {
    let zero = Quat::from_xyzw(0.0, 0.0, 0.0, 0.0);
    let doc = Document {
        skeleton: Skeleton {
            bones: vec![Bone {
                name: "joint".into(),
                parent: None,
                rest: Transform::IDENTITY,
                inverse_bind: None,
            }],
        },
        clips: vec![Clip {
            name: "curve".into(),
            duration_s: 1.0,
            tracks: vec![
                Track {
                    bone: 0,
                    property: Property::Rotation,
                    interpolation: Interpolation::CubicSpline,
                    times: vec![0.0, 1.0],
                    values: TrackValues::Quats(vec![
                        zero,
                        Quat::IDENTITY,
                        Quat::from_xyzw(0.0, 0.0, outgoing, 0.0),
                        Quat::from_xyzw(0.0, 0.0, incoming, 0.0),
                        Quat::IDENTITY,
                        zero,
                    ]),
                },
                Track {
                    // Three translation keys admit the existing report grid;
                    // rotation evidence remains the two-key cubic curve.
                    bone: 0,
                    property: Property::Translation,
                    interpolation: Interpolation::Linear,
                    times: vec![0.0, 0.5, 1.0],
                    values: TrackValues::Vec3s(vec![Vec3::ZERO; 3]),
                },
            ],
        }],
        ..Document::default()
    };
    animsmith_gltf::write::write(&doc, path).unwrap();
}

fn command(config: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_animsmith"));
    command.arg("--config").arg(config);
    command
}

fn assert_refused(output: &Output) {
    assert_eq!(
        output.status.code(),
        Some(2),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "refusal must not publish partial evidence"
    );
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    for subject in ["frozen-bone", "curve", "joint", "cubic"] {
        assert!(diagnostic.contains(subject), "{diagnostic}");
    }
}

#[test]
fn actual_cubic_motion_and_flat_hold_have_distinct_complete_results() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("animsmith.toml");
    std::fs::write(&config, "[clips.curve]\nanimates_bones = [\"joint\"]\n").unwrap();
    for (name, outgoing, incoming, exit, count) in
        [("moving", 4.0, -4.0, 0, 0), ("flat", 0.0, 0.0, 1, 1)]
    {
        let input = dir.path().join(format!("{name}.glb"));
        source(&input, outgoing, incoming);
        let original = std::fs::read(&input).unwrap();
        let output = command(&config)
            .arg("lint")
            .arg(&input)
            .args(["--select", "frozen-bone", "--format", "json"])
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(exit),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let json: Value = serde_json::from_slice(&output.stdout).unwrap();
        let check = json["files"][0]["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|check| check["check_id"] == "frozen-bone")
            .unwrap();
        assert_eq!(check["evaluation"], "complete");
        assert_eq!(check["findings"].as_array().unwrap().len(), count);
        if count == 1 {
            assert_eq!(check["findings"][0]["measured"], 0.0);
        }
        // Equal identity keys still have zero keyed range, independent of
        // the analytically known 90-degree cubic midpoint in the moving case.
        assert!(
            json["files"][0]["measurements"]["clips"]["curve"]["bone_rotation_range_deg"]
                .as_object()
                .unwrap()
                .is_empty()
        );
        assert_eq!(std::fs::read(input).unwrap(), original);
    }
}

#[test]
fn undecidable_cubic_aborts_lint_and_collection_without_partial_output() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("animsmith.toml");
    std::fs::write(&config, "[clips.curve]\nanimates_bones = [\"joint\"]\n").unwrap();
    let input = dir.path().join("unknown.glb");
    let good = dir.path().join("good.glb");
    // Identity at keys and midpoint, but about 41 degrees at u=1/4.
    source(&input, 4.0, 4.0);
    source(&good, 4.0, -4.0);
    let original = std::fs::read(&input).unwrap();
    for format in ["json", "text"] {
        let output = command(&config)
            .arg("lint")
            .arg(&good)
            .arg(&input)
            .args(["--select", "frozen-bone", "--format", format])
            .output()
            .unwrap();
        assert_refused(&output);
    }
    let manifest = dir.path().join("collection.toml");
    std::fs::write(
        &manifest,
        r#"schema = "urn:animsmith:schema:collection-manifest:1"
schema_version = 1
collection_id = "example.frozen"
[[sources]]
key = "good"
path = "good.glb"
config = "animsmith.toml"
[[sources]]
key = "unknown"
path = "unknown.glb"
config = "animsmith.toml"
[[clips]]
id = "example.frozen/good"
source = "good"
take_index = 0
take_name = "curve"
[[clips]]
id = "example.frozen/unknown"
source = "unknown"
take_index = 0
take_name = "curve"
"#,
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .args(["collection", "lint"])
        .arg(manifest)
        .args(["--format", "json"])
        .output()
        .unwrap();
    assert_refused(&output);
    assert_eq!(std::fs::read(input).unwrap(), original);
}

#[cfg(feature = "report")]
#[test]
fn inconclusive_report_checks_preserve_prior_single_and_comparison_output() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("animsmith.toml");
    std::fs::write(&config, "[clips.curve]\nanimates_bones = [\"joint\"]\n").unwrap();
    let input = dir.path().join("unknown.glb");
    let good = dir.path().join("good.glb");
    source(&input, 4.0, 4.0);
    source(&good, 4.0, -4.0);
    let originals = [
        std::fs::read(&input).unwrap(),
        std::fs::read(&good).unwrap(),
    ];
    let output = dir.path().join("report.html");
    let previous = b"previous report bytes";
    std::fs::write(&output, previous).unwrap();
    for evidence_only in [false, true] {
        let mut cmd = command(&config);
        cmd.arg("report").arg(&input).arg("--output").arg(&output);
        if evidence_only {
            cmd.arg("--evidence-only");
        }
        assert_refused(&cmd.output().unwrap());
        assert_eq!(std::fs::read(&output).unwrap(), previous);
    }
    // Put the undecidable asset second to prove a successful first analysis
    // cannot publish an incomplete comparison.
    let result = command(&config)
        .arg("report")
        .arg(&good)
        .arg("--compare-after")
        .arg(&input)
        .args(["--before-clip", "curve", "--after-clip", "curve"])
        .arg("--output")
        .arg(&output)
        .output()
        .unwrap();
    assert_refused(&result);
    assert_eq!(std::fs::read(&output).unwrap(), previous);
    assert_eq!(std::fs::read(input).unwrap(), originals[0]);
    assert_eq!(std::fs::read(good).unwrap(), originals[1]);
}

#[test]
fn keyed_cubic_motion_keeps_the_existing_nan_diagnostic() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("animsmith.toml");
    std::fs::write(&config, "[clips.curve]\nanimates_bones = [\"joint\"]\n").unwrap();
    let input = dir.path().join("moving-nan.glb");
    source(&input, 0.0, 0.0);
    // Mutate the actual GLB payload, bypassing the writer's finite-output
    // admission. The output accessor's second key is exactly 90 degrees;
    // its first (unused) incoming tangent independently contains NaN.
    let mut raw = std::fs::read(&input).unwrap();
    let json_len = u32::from_le_bytes(raw[12..16].try_into().unwrap()) as usize;
    let json: Value = serde_json::from_slice(&raw[20..20 + json_len]).unwrap();
    let output_accessor = json["animations"][0]["samplers"][0]["output"]
        .as_u64()
        .unwrap() as usize;
    let accessor = &json["accessors"][output_accessor];
    assert_eq!(accessor["count"], 6);
    let view = accessor["bufferView"].as_u64().unwrap() as usize;
    let values = 28
        + json_len
        + json["bufferViews"][view]["byteOffset"]
            .as_u64()
            .unwrap_or(0) as usize
        + accessor["byteOffset"].as_u64().unwrap_or(0) as usize;
    raw[values + 4 * 16 + 8..values + 4 * 16 + 12].copy_from_slice(&1.0f32.to_le_bytes());
    raw[values + 8..values + 12].copy_from_slice(&f32::NAN.to_le_bytes());
    std::fs::write(&input, &raw).unwrap();
    let output = command(&config)
        .arg("lint")
        .arg(&input)
        .args(["--select", "nan,frozen-bone", "--format", "json"])
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let checks = json["files"][0]["checks"].as_array().unwrap();
    let frozen = checks
        .iter()
        .find(|check| check["check_id"] == "frozen-bone")
        .unwrap();
    assert_eq!(frozen["evaluation"], "complete");
    assert!(frozen["findings"].as_array().unwrap().is_empty());
    let nan = checks
        .iter()
        .find(|check| check["check_id"] == "nan")
        .unwrap();
    assert_eq!(nan["findings"].as_array().unwrap().len(), 1);
    assert_eq!(nan["findings"][0]["clip"], "curve");
    assert_eq!(nan["findings"][0]["bone"], "joint");
    assert_eq!(std::fs::read(input).unwrap(), raw);
}
