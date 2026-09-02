//! CLI contract for the offline HTML `report` command: the explicit visual
//! before/after form, and the evidence-only form of both.

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

/// The embedded report payload, which is the machine-readable half of a
/// generated document.
fn embedded_data(html: &str, id: &str) -> serde_json::Value {
    let marker = format!("<script type=\"application/json\" id=\"{id}\">");
    let (_, tail) = html.split_once(&marker).expect("report data marker");
    let (raw, _) = tail.split_once("</script>").expect("report data close");
    serde_json::from_str(raw).expect("report data is JSON")
}

#[test]
fn evidence_only_publishes_both_report_forms_without_their_sampled_motion() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let rig = rig_fixture();
    let before = fixture();
    let after = comparison_fixture("after");

    let render = |name: &str, evidence_only: bool, comparison: bool| {
        let output = directory.path().join(name);
        let mut command = animsmith();
        command.arg("report");
        if comparison {
            command
                .arg(&before)
                .args(["--compare-after"])
                .arg(&after)
                .args(["--before-clip", "acceptance-matrix"])
                .args(["--after-clip", "acceptance-matrix"]);
        } else {
            command.arg(&rig);
        }
        command.arg("--output").arg(&output);
        if evidence_only {
            command.arg("--evidence-only");
        }
        let result = command.output().expect("runs report");
        assert_eq!(
            result.status.code(),
            Some(0),
            "{name}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let html = std::fs::read_to_string(&output).expect("report is written");
        let bytes = std::fs::metadata(&output).expect("report metadata").len();
        (html, bytes)
    };

    for (comparison, id, prefix) in [
        (false, "report-data", None),
        (true, "comparison-report-data", Some(["before", "after"])),
    ] {
        let form = if comparison {
            "comparison"
        } else {
            "single-clip"
        };
        let (full_html, full_bytes) = render(&format!("{form}-full.html"), false, comparison);
        let (html, bytes) = render(&format!("{form}-evidence.html"), true, comparison);
        let full_data = embedded_data(&full_html, id);
        let data = embedded_data(&html, id);

        assert_eq!(full_data["evidence_only"], false, "{form}");
        assert_eq!(data["evidence_only"], true, "{form}");
        assert!(
            !html.contains("\"positions\""),
            "{form}: no sampled pose grid is embedded anywhere in the document"
        );
        assert!(
            full_html.contains("\"positions\""),
            "{form}: a full report still carries its pose grid"
        );
        match prefix {
            None => {
                assert_eq!(data["findings"], full_data["findings"], "{form}");
                assert!(data["clips"][0].get("positions").is_none(), "{form}");
            }
            Some(sides) => {
                for side in sides {
                    assert_eq!(
                        data[side]["findings"], full_data[side]["findings"],
                        "{form}"
                    );
                    assert_eq!(
                        data[side]["identity"], full_data[side]["identity"],
                        "{form}"
                    );
                    assert!(data[side]["clip"].get("positions").is_none(), "{form}");
                }
            }
        }
        assert!(
            html.contains("Pose playback omitted: evidence-only report"),
            "{form}: the document says where its pose view went"
        );
        assert!(
            bytes < full_bytes,
            "{form}: {bytes} bytes must be smaller than the full {full_bytes}"
        );
    }
}

#[test]
fn development_guide_names_the_evidence_only_flag_the_cli_offers() {
    let guide = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../DEVELOPMENT.md"),
    )
    .expect("DEVELOPMENT.md");
    assert!(
        guide.contains("`animsmith report --evidence-only`"),
        "DEVELOPMENT.md names the evidence-only report as the publishable form of a licensed clip"
    );

    let output = animsmith()
        .args(["report", "--help"])
        .output()
        .expect("runs report --help");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let help = String::from_utf8_lossy(&output.stdout);
    assert!(help.contains("--evidence-only"), "{help}");
}
