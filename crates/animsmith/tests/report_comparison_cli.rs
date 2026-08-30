//! CLI contract for the explicit offline visual before/after report.

use std::path::PathBuf;
use std::process::Command;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../animsmith-report/testdata/rig.gltf")
}

#[cfg(unix)]
#[test]
fn report_comparison_refuses_symlink_and_hardlink_outputs_without_touching_inputs() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let before = fixture();
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
                before.to_str().expect("UTF-8 fixture path"),
                "--compare-after",
                before.to_str().expect("UTF-8 fixture path"),
                "--before-clip",
                "walk",
                "--after-clip",
                "idle",
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
    let generated = animsmith()
        .args([
            "report",
            fixture.to_str().expect("UTF-8 fixture path"),
            "--compare-after",
            fixture.to_str().expect("UTF-8 fixture path"),
            "--before-clip",
            "walk",
            "--after-clip",
            "idle",
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
            fixture.to_str().expect("UTF-8 fixture path"),
            "--compare-after",
            fixture.to_str().expect("UTF-8 fixture path"),
            "--before-clip",
            "missing",
            "--after-clip",
            "idle",
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
