//! CLI contract for the explicit offline visual before/after report.

use std::path::PathBuf;
use std::process::Command;

fn fixture() -> PathBuf {
    comparison_fixture("before")
}

fn comparison_fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/assets")
        .join(format!("report-comparison-{name}.glb"))
}

fn rig_fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../animsmith-report/testdata/rig.gltf")
}

fn write_duplicate_clip_gltf(path: &std::path::Path) {
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(rig_fixture()).expect("rig fixture bytes"))
            .expect("rig fixture JSON");
    let animations = value["animations"]
        .as_array_mut()
        .expect("animations array");
    let mut duplicate = animations[0].clone();
    duplicate["name"] = serde_json::Value::String("walk".to_owned());
    animations.push(duplicate);
    std::fs::write(path, serde_json::to_vec(&value).unwrap()).expect("duplicate clip fixture");
}

fn write_external_buffer_gltf(directory: &std::path::Path, name: &str) -> (PathBuf, PathBuf) {
    const BUFFER: &[u8] = &[
        0, 0, 0, 0, 0, 0, 0, 63, 0, 0, 128, 63, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 128, 63,
        0, 0, 0, 0, 21, 239, 195, 62, 0, 0, 0, 0, 94, 131, 108, 63, 0, 0, 0, 0, 243, 4, 53, 63, 0,
        0, 0, 0, 243, 4, 53, 63, 0, 0, 0, 0, 0, 0, 128, 63, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 128, 63,
    ];
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(rig_fixture()).expect("rig fixture bytes"))
            .expect("rig fixture JSON");
    value["asset"]["generator"] = serde_json::Value::String(format!("comparison-{name}"));
    let sidecar_name = format!("{name}.bin");
    value["buffers"][0]["uri"] = serde_json::Value::String(sidecar_name.clone());
    let primary = directory.join(format!("{name}.gltf"));
    let sidecar = directory.join(sidecar_name);
    std::fs::write(&primary, serde_json::to_vec(&value).unwrap()).expect("external glTF");
    std::fs::write(&sidecar, BUFFER).expect("external buffer");
    (primary, sidecar)
}

#[cfg(unix)]
#[test]
fn report_comparison_refuses_symlink_and_hardlink_outputs_without_touching_inputs() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let before = fixture();
    let after = comparison_fixture("after");
    let original = std::fs::read(&before).expect("fixture bytes");
    for (name, symlink) in [("symlink.html", true), ("hardlink.html", false)] {
        let output = directory.path().join(name);
        if symlink {
            std::os::unix::fs::symlink(&before, &output).expect("creates symlink output");
        } else {
            std::fs::hard_link(&before, &output).expect("creates hardlink output");
        }
        let result = animsmith()
            .args([
                "report",
                after.to_str().expect("UTF-8 fixture path"),
                "--compare-after",
                before.to_str().expect("UTF-8 fixture path"),
                "--before-clip",
                "acceptance-matrix",
                "--after-clip",
                "acceptance-matrix",
                "--output",
                output.to_str().expect("UTF-8 output path"),
            ])
            .output()
            .expect("runs alias refusal");
        assert_eq!(
            result.status.code(),
            Some(2),
            "{name}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(std::fs::read(&before).expect("fixture survives"), original);
    }
}

fn animsmith() -> Command {
    Command::new(env!("CARGO_BIN_EXE_animsmith"))
}

#[test]
fn report_comparison_requires_explicit_clips_and_publishes_one_offline_document() {
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let output = output_dir.path().join("comparison.html");
    let fixture = fixture();
    let after = comparison_fixture("after");
    let generated = animsmith()
        .args([
            "report",
            after.to_str().expect("UTF-8 fixture path"),
            "--compare-after",
            fixture.to_str().expect("UTF-8 fixture path"),
            "--before-clip",
            "acceptance-matrix",
            "--after-clip",
            "acceptance-matrix",
            "--output",
            output.to_str().expect("UTF-8 output path"),
        ])
        .output()
        .expect("runs visual comparison");
    assert!(
        generated.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&generated.stderr)
    );
    let html = std::fs::read_to_string(&output).expect("comparison is written");
    assert!(html.contains("comparison-report-data"));
    assert!(html.contains("normalized sampled-frame phase"));
    assert!(html.contains("item.id = `${kind.slice(0, -1)}-${name}-${index}`"));

    let refused_path = output_dir.path().join("must-not-exist.html");
    let refused = animsmith()
        .args([
            "report",
            after.to_str().expect("UTF-8 fixture path"),
            "--compare-after",
            fixture.to_str().expect("UTF-8 fixture path"),
            "--before-clip",
            "missing",
            "--after-clip",
            "acceptance-matrix",
            "--output",
            refused_path.to_str().expect("UTF-8 output path"),
        ])
        .output()
        .expect("runs refusal");
    assert_eq!(refused.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&refused.stderr).contains("before clip \"missing\" was not found")
    );
    assert!(
        !refused_path.exists(),
        "a correspondence refusal publishes nothing"
    );
}

#[test]
fn report_comparison_refuses_authored_duplicate_clip_names_on_either_side() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let duplicate = directory.path().join("duplicate.gltf");
    write_duplicate_clip_gltf(&duplicate);
    let rig = rig_fixture();
    for (label, before, after) in [
        ("before", duplicate.as_path(), rig.as_path()),
        ("after", rig.as_path(), duplicate.as_path()),
    ] {
        let output = directory.path().join(format!("{label}.html"));
        std::fs::write(&output, b"prior report").expect("prior output");
        let result = animsmith()
            .args([
                "report",
                before.to_str().unwrap(),
                "--compare-after",
                after.to_str().unwrap(),
                "--before-clip",
                "walk",
                "--after-clip",
                "walk",
                "--output",
                output.to_str().unwrap(),
            ])
            .output()
            .expect("runs duplicate refusal");
        assert_eq!(
            result.status.code(),
            Some(2),
            "{label}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(
            String::from_utf8_lossy(&result.stderr)
                .contains(&format!("{label} authored clip \"walk\" is ambiguous"))
        );
        assert_eq!(std::fs::read(&output).unwrap(), b"prior report");
    }
}

#[test]
fn report_comparison_guards_sidecar_and_configuration_inputs_atomically() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let (before, before_sidecar) = write_external_buffer_gltf(directory.path(), "before");
    let (after, after_sidecar) = write_external_buffer_gltf(directory.path(), "after");
    for sidecar in [&before_sidecar, &after_sidecar] {
        let sidecar_bytes = std::fs::read(sidecar).unwrap();
        let sidecar_alias = animsmith()
            .args([
                "report",
                before.to_str().unwrap(),
                "--compare-after",
                after.to_str().unwrap(),
                "--before-clip",
                "walk",
                "--after-clip",
                "walk",
                "--output",
                sidecar.to_str().unwrap(),
            ])
            .output()
            .expect("runs sidecar alias refusal");
        assert_eq!(sidecar_alias.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&sidecar_alias.stderr).contains("external dependency"));
        assert_eq!(std::fs::read(sidecar).unwrap(), sidecar_bytes);
    }

    let config = directory.path().join("policy.toml");
    std::fs::write(&config, b"[checks.nan]\nseverity = \"warn\"\n").unwrap();
    let config_bytes = std::fs::read(&config).unwrap();
    let config_alias = animsmith()
        .args([
            "--config",
            config.to_str().unwrap(),
            "report",
            before.to_str().unwrap(),
            "--compare-after",
            after.to_str().unwrap(),
            "--before-clip",
            "walk",
            "--after-clip",
            "walk",
            "--output",
            config.to_str().unwrap(),
        ])
        .output()
        .expect("runs configuration alias refusal");
    assert_eq!(config_alias.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&config_alias.stderr).contains("configuration input"));
    assert_eq!(std::fs::read(&config).unwrap(), config_bytes);
}
