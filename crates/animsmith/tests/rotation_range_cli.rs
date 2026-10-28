//! Keyed rotation-range facts and their frozen-bone consumer share the same
//! orientation meaning for self-authored finite, non-unit source quaternions.

use animsmith_core::glam::Quat;
use animsmith_core::model::*;
use serde_json::Value;
use std::collections::BTreeSet;
use std::process::Command;

#[test]
fn finite_rotation_magnitudes_keep_measurements_and_frozen_findings_consistent() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("ranges.glb");
    let config = dir.path().join("ranges.toml");
    let mut clips = Vec::new();
    let mut expectations = String::new();
    let mut frozen_clips = BTreeSet::new();
    for (name, scale) in [("unit", 1.0), ("large", 1e20), ("tiny", 1e-30)] {
        let identity = Quat::from_xyzw(0.0, 0.0, 0.0, scale);
        for (kind, middle) in [
            ("hold", -identity),
            ("turn", Quat::from_xyzw(0.0, scale, 0.0, scale)),
        ] {
            let name = format!("{name}-{kind}");
            expectations.push_str(&format!(
                "[clips.\"{name}\"]\nanimates_bones = [\"joint\"]\n"
            ));
            if kind == "hold" {
                frozen_clips.insert(name.clone());
            }
            clips.push(Clip {
                name,
                duration_s: 1.0,
                tracks: vec![Track {
                    bone: 0,
                    property: Property::Rotation,
                    interpolation: Interpolation::Linear,
                    times: vec![0.0, 0.5, 1.0],
                    values: TrackValues::Quats(vec![identity, middle, -identity]),
                }],
            });
        }
    }
    let document = Document {
        skeleton: Skeleton {
            bones: vec![Bone {
                name: "joint".into(),
                parent: None,
                rest: Transform::IDENTITY,
                inverse_bind: None,
            }],
        },
        clips,
        ..Document::default()
    };
    animsmith_gltf::write::write(&document, &input).unwrap();
    std::fs::write(&config, expectations).unwrap();
    let source_bytes = std::fs::read(&input).unwrap();

    let measured = Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .arg("measure")
        .arg(&input)
        .args(["--format", "json"])
        .output()
        .unwrap();
    assert_eq!(
        measured.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&measured.stderr)
    );
    let measured: Value = serde_json::from_slice(&measured.stdout).unwrap();
    let measured_clips = measured["files"][0]["measurements"]["clips"]
        .as_object()
        .unwrap();
    assert_eq!(measured_clips.len(), 6);
    for (clip, measurement) in measured_clips {
        let ranges = measurement["bone_rotation_range_deg"].as_object().unwrap();
        if frozen_clips.contains(clip) {
            // The wire format omits ranges below its existing recording floor.
            assert!(ranges.is_empty(), "{clip}: {ranges:?}");
        } else {
            assert_eq!(ranges.len(), 1);
            let degrees = ranges["joint"].as_f64().unwrap();
            assert!((degrees - 90.0).abs() < 1e-4, "{clip}: {degrees}");
        }
    }

    let linted = Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .arg("--config")
        .arg(&config)
        .arg("lint")
        .arg(&input)
        .args(["--select", "frozen-bone", "--format", "json"])
        .output()
        .unwrap();
    assert_eq!(
        linted.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&linted.stderr)
    );
    let linted: Value = serde_json::from_slice(&linted.stdout).unwrap();
    let check = linted["files"][0]["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["check_id"] == "frozen-bone")
        .unwrap();
    assert_eq!(check["evaluation"], "complete");
    let findings = check["findings"].as_array().unwrap();
    assert_eq!(findings.len(), 3);
    let found_clips: BTreeSet<_> = findings
        .iter()
        .map(|finding| {
            assert_eq!(finding["bone"], "joint");
            assert_eq!(finding["severity"], "error");
            assert_eq!(finding["measured"], 0.0);
            assert_eq!(finding["expected"], 1.0);
            finding["clip"].as_str().unwrap().to_owned()
        })
        .collect();
    assert_eq!(found_clips, frozen_clips);
    assert_eq!(std::fs::read(&input).unwrap(), source_bytes);
}
