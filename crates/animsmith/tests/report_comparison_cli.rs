//! CLI contracts for offline HTML reports: safe publication of ordinary and
//! explicit before/after reports, including the evidence-only form of both.

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

fn assert_publication_refused(result: &std::process::Output, boundary: &str) {
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert_eq!(result.status.code(), Some(2), "{stderr}");
    assert!(stderr.contains(boundary), "{stderr}");
    assert!(
        result.stdout.is_empty(),
        "a refusal must not announce a report"
    );
}

fn assert_file_unchanged(path: &std::path::Path, expected: &[u8]) {
    let actual = std::fs::read(path).unwrap();
    assert!(
        actual.as_slice() == expected,
        "{} bytes changed ({} bytes became {})",
        path.display(),
        expected.len(),
        actual.len()
    );
}

#[test]
fn ordinary_report_refuses_primary_path_aliases() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input.glb");
    std::fs::copy(fixture(), &input).unwrap();
    std::fs::create_dir(directory.path().join("nested")).unwrap();
    let original = std::fs::read(&input).unwrap();
    for (source, output, evidence_only) in [
        (input.clone(), input.clone(), false),
        (PathBuf::from("input.glb"), input.clone(), true),
        (input.clone(), PathBuf::from("nested/../input.glb"), true),
    ] {
        let mut command = animsmith();
        command
            .current_dir(directory.path())
            .arg("report")
            .arg(source)
            .arg("--output")
            .arg(output);
        if evidence_only {
            command.arg("--evidence-only");
        }
        assert_publication_refused(&command.output().unwrap(), "different files");
        assert_file_unchanged(&input, &original);
    }
}

#[test]
fn ordinary_report_hardlink_destination_preserves_source_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input.glb");
    let output = directory.path().join("alias.html");
    std::fs::copy(fixture(), &input).unwrap();
    std::fs::hard_link(&input, &output).unwrap();
    let original = std::fs::read(&input).unwrap();
    let result = animsmith()
        .current_dir(directory.path())
        .arg("report")
        .arg(&input)
        .arg("--output")
        .arg(&output)
        .output()
        .unwrap();
    assert_file_unchanged(&input, &original);
    if cfg!(unix) || result.status.code() == Some(2) {
        assert_publication_refused(&result, "different files");
        assert_file_unchanged(&output, &original);
    } else {
        // Without the Unix inode check, replacing a distinct hardlink entry
        // is safe: the source bytes must survive and the report must be valid.
        assert_eq!(result.status.code(), Some(0));
        let html = std::fs::read_to_string(&output).unwrap();
        assert_eq!(
            embedded_data(&html, "report-data")["clips"][0]["name"],
            "acceptance-matrix"
        );
    }
}

#[cfg(unix)]
#[test]
fn ordinary_report_refuses_symlink_aliases_without_replacing_entries() {
    use std::os::unix::fs::{MetadataExt, symlink};
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input.glb");
    let alias = directory.path().join("alias.glb");
    std::fs::copy(fixture(), &input).unwrap();
    symlink(&input, &alias).unwrap();
    let original = std::fs::read(&input).unwrap();
    let inode = std::fs::metadata(&input).unwrap().ino();
    for (source, output) in [(&input, &alias), (&alias, &input)] {
        let result = animsmith()
            .current_dir(directory.path())
            .arg("report")
            .arg(source)
            .args(["--evidence-only", "--output"])
            .arg(output)
            .output()
            .unwrap();
        assert_publication_refused(&result, "different files");
        assert_file_unchanged(&input, &original);
        assert_eq!(std::fs::metadata(&input).unwrap().ino(), inode);
        assert_eq!(std::fs::read_link(&alias).unwrap(), input);
    }
}

#[test]
fn ordinary_report_protects_explicit_and_discovered_configuration() {
    for explicit in [true, false] {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("input.glb");
        std::fs::copy(fixture(), &input).unwrap();
        let source_bytes = std::fs::read(&input).unwrap();
        let config = directory.path().join(if explicit {
            "policy.toml"
        } else {
            "animsmith.toml"
        });
        let config_bytes = b"[checks.nan]\nseverity = \"warn\"\n";
        std::fs::write(&config, config_bytes).unwrap();
        let mut command = animsmith();
        command.current_dir(directory.path());
        if explicit {
            command.arg("--config").arg(&config);
        }
        let result = command
            .arg("report")
            .arg(&input)
            .args(["--evidence-only", "--output"])
            .arg(&config)
            .output()
            .unwrap();
        assert_publication_refused(&result, "configuration input");
        assert_file_unchanged(&input, &source_bytes);
        assert_file_unchanged(&config, config_bytes);
    }
}

#[test]
fn ordinary_report_protects_rooted_sidecars_and_unavailable_retained_keys() {
    let directory = tempfile::tempdir().unwrap();
    let assets = directory.path().join("assets");
    std::fs::create_dir(&assets).unwrap();
    let (input, buffer) = write_external_buffer_gltf(&assets, "source");
    let texture = assets.join("texture.png");
    let missing = assets.join("missing.png");
    let blocked = assets.join("blocked");
    // These images are retained source references, unused by the animation.
    let mut source: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&input).unwrap()).unwrap();
    source["images"] = serde_json::json!([
        {"uri":"texture.png"}, {"uri":"missing.png"},
        {"uri":"blocked/direct.png"}, {"uri":"blocked/nested/deep.png"}
    ]);
    std::fs::write(&input, serde_json::to_vec(&source).unwrap()).unwrap();
    std::fs::write(&texture, b"retained unconsumed texture bytes").unwrap();
    std::fs::write(&blocked, b"a regular file, not a directory").unwrap();
    let originals: Vec<_> = [&input, &buffer, &texture, &blocked]
        .into_iter()
        .map(|path| (path, std::fs::read(path).unwrap()))
        .collect();
    for output in [&buffer, &texture, &missing] {
        let result = animsmith()
            .current_dir(directory.path())
            .args([
                "report",
                "assets/source.gltf",
                "--evidence-only",
                "--output",
            ])
            .arg(output)
            .output()
            .unwrap();
        assert_publication_refused(&result, "external dependency");
        for (path, bytes) in &originals {
            assert_file_unchanged(path, bytes);
        }
        assert!(!missing.exists());
    }

    // Ordinary diagnostics can describe an incomplete closure; comparison's
    // complete-closure evidence requirement must not leak into publication.
    let output = directory.path().join("report.html");
    let result = animsmith()
        .current_dir(directory.path())
        .args(["report", "assets/source.gltf", "--output"])
        .arg(&output)
        .output()
        .unwrap();
    assert_eq!(
        result.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let html = std::fs::read_to_string(&output).unwrap();
    assert_eq!(
        embedded_data(&html, "report-data")["clips"][0]["name"],
        "walk"
    );
    for (path, bytes) in &originals {
        assert_file_unchanged(path, bytes);
    }
    assert!(!missing.exists());

    let comparison = animsmith()
        .current_dir(directory.path())
        .args(["report", "assets/source.gltf", "--compare-after"])
        .arg(rig_fixture())
        .args(["--before-clip", "walk", "--after-clip", "walk", "--output"])
        .arg(&output)
        .output()
        .unwrap();
    assert_publication_refused(&comparison, "no complete dependency-closure identity");
    assert_file_unchanged(&output, html.as_bytes());
}

#[test]
fn ordinary_report_load_errors_preserve_previous_output_and_inputs() {
    for malformed_source in [true, false] {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("input.glb");
        let config = directory.path().join("policy.toml");
        let output = directory.path().join("report.html");
        std::fs::copy(fixture(), &input).unwrap();
        std::fs::write(&config, b"[checks.nan]\nseverity = \"warn\"\n").unwrap();
        if malformed_source {
            std::fs::write(&input, b"malformed GLB").unwrap();
        } else {
            std::fs::write(&config, b"[malformed TOML").unwrap();
        }
        std::fs::write(&output, b"previous report").unwrap();
        let originals: Vec<_> = [&input, &config, &output]
            .into_iter()
            .map(|path| (path, std::fs::read(path).unwrap()))
            .collect();
        let result = animsmith()
            .current_dir(directory.path())
            .arg("--config")
            .arg(&config)
            .arg("report")
            .arg(&input)
            .arg("--output")
            .arg(&output)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(2));
        assert!(!result.stderr.is_empty());
        assert!(result.stdout.is_empty());
        for (path, bytes) in originals {
            assert_file_unchanged(path, &bytes);
        }
    }
}

#[cfg(unix)]
#[test]
fn ordinary_report_refuses_a_retained_symlink_even_for_an_unrelated_output() {
    let directory = tempfile::tempdir().unwrap();
    let (input, buffer) = write_external_buffer_gltf(directory.path(), "source");
    let target = directory.path().join("target.png");
    let link = directory.path().join("texture.png");
    let output = directory.path().join("report.html");
    let mut source: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&input).unwrap()).unwrap();
    source["images"] = serde_json::json!([{"uri":"texture.png"}]);
    std::fs::write(&input, serde_json::to_vec(&source).unwrap()).unwrap();
    std::fs::write(&target, b"unconsumed texture target").unwrap();
    std::os::unix::fs::symlink(&target, &link).unwrap();
    std::fs::write(&output, b"previous report").unwrap();
    let originals: Vec<_> = [&input, &buffer, &target, &output]
        .into_iter()
        .map(|path| (path, std::fs::read(path).unwrap()))
        .collect();

    let result = animsmith()
        .current_dir(directory.path())
        .arg("report")
        .arg(&input)
        .arg("--output")
        .arg(&output)
        .output()
        .unwrap();
    assert_publication_refused(&result, "external dependency \"texture.png\" is a symlink");
    for (path, bytes) in originals {
        assert_file_unchanged(path, &bytes);
    }
    assert_eq!(std::fs::read_link(&link).unwrap(), target);
}

#[test]
fn ordinary_report_replaces_the_destination_without_truncating_its_previous_hardlink() {
    let directory = tempfile::tempdir().unwrap();
    let (input, buffer) = write_external_buffer_gltf(directory.path(), "source");
    let config = directory.path().join("policy.toml");
    std::fs::write(&config, b"[checks.nan]\nseverity = \"warn\"\n").unwrap();
    let originals: Vec<_> = [&input, &buffer, &config]
        .into_iter()
        .map(|path| (path, std::fs::read(path).unwrap()))
        .collect();
    let mut full_findings = None;
    for evidence_only in [false, true] {
        let output = directory
            .path()
            .join(format!("report-{evidence_only}.html"));
        let backup = directory
            .path()
            .join(format!("backup-{evidence_only}.html"));
        std::fs::write(&output, b"previous report").unwrap();
        std::fs::hard_link(&output, &backup).unwrap();
        let mut command = animsmith();
        command
            .current_dir(directory.path())
            .arg("--config")
            .arg(&config)
            .arg("report")
            .arg(&input)
            .arg("--output")
            .arg(&output);
        if evidence_only {
            command.arg("--evidence-only");
        }
        let result = command.output().unwrap();
        assert_eq!(
            result.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let html = std::fs::read_to_string(&output).unwrap();
        let data = embedded_data(&html, "report-data");
        assert_eq!(data["clips"][0]["name"], "walk");
        assert_eq!(data["evidence_only"], evidence_only);
        if evidence_only {
            assert_eq!(Some(&data["findings"]), full_findings.as_ref());
            assert!(data["clips"][0].get("positions").is_none());
            assert!(data["clips"][0].get("locals").is_none());
        } else {
            assert!(data["clips"][0]["positions"].is_string());
            full_findings = Some(data["findings"].clone());
        }
        assert_file_unchanged(&backup, b"previous report");
        for (path, bytes) in &originals {
            assert_file_unchanged(path, bytes);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&output).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn report_comparison_refuses_symlink_and_hardlink_outputs_without_touching_inputs() {
    let directory = tempfile::tempdir().expect("temporary directory");
    // The input and its hardlink must share a filesystem, including on CI.
    let before = directory.path().join("before.glb");
    std::fs::copy(fixture(), &before).expect("copies input fixture");
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
                assert!(data["clips"][0].get("locals").is_none(), "{form}");
                assert!(data.get("blend").is_none(), "{form}");
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
        let notice = if prefix.is_none() {
            "Pose playback and illustrative blending are omitted in this evidence-only report. No sampled positions or local transforms are embedded."
        } else {
            "Pose playback omitted: evidence-only report"
        };
        assert!(
            html.contains(notice),
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
    // The policy sentence itself names the command, so a guide that kept the
    // flag in unrelated prose while pointing the policy at something else
    // would fail here: the command is read out of that sentence.
    const POLICY: &str = "The artifact form that satisfies this rule is `";
    let prose = guide.split_whitespace().collect::<Vec<_>>().join(" ");
    let start = prose.find(POLICY).expect(
        "DEVELOPMENT.md states which artifact form satisfies the published-derivative rule",
    ) + POLICY.len();
    let end = prose[start..]
        .find('`')
        .expect("the policy sentence closes its command span")
        + start;
    let command: Vec<&str> = prose[start..end].split_whitespace().collect();
    assert_eq!(command.first(), Some(&"animsmith"), "{command:?}");
    let subcommand = command.get(1).expect("the policy names a subcommand");
    let flags: Vec<&str> = command[2..]
        .iter()
        .copied()
        .filter(|word| word.starts_with("--"))
        .collect();
    assert_eq!(flags, ["--evidence-only"], "{command:?}");

    let output = animsmith()
        .args([subcommand, "--help"])
        .output()
        .expect("runs the policy's subcommand with --help");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let help = String::from_utf8_lossy(&output.stdout);
    for flag in flags {
        assert!(
            help.lines().any(|line| line.trim_start().starts_with(flag)),
            "`animsmith {subcommand} --help` offers {flag}:\n{help}"
        );
    }
}
