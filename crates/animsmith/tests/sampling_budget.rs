//! Real CLI allocation regressions run in memory- and time-bounded children.
//! Portable sampler/cache/coverage tests live in animsmith-core.
#![cfg(all(target_os = "linux", target_pointer_width = "64"))]

use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant};

fn fixture(dir: &Path, name: &str, bones: usize, frames: usize, clips: usize) -> PathBuf {
    let mut bytes = Vec::with_capacity(frames * 16);
    for i in 0..frames {
        bytes.extend((i as f32 / (frames - 1) as f32).to_le_bytes());
    }
    for i in 0..frames {
        for value in [i as f32 / (frames - 1) as f32, 0.0, 0.0] {
            bytes.extend(value.to_le_bytes());
        }
    }
    let bin = format!("{name}.bin");
    std::fs::write(dir.join(&bin), &bytes).unwrap();
    let source = json!({
        "asset":{"version":"2.0"}, "buffers":[{"uri":bin,"byteLength":bytes.len()}],
        "bufferViews":[{"buffer":0,"byteOffset":0,"byteLength":frames*4},{"buffer":0,"byteOffset":frames*4,"byteLength":frames*12}],
        "accessors":[{"bufferView":0,"componentType":5126,"count":frames,"type":"SCALAR","min":[0],"max":[1]},
            {"bufferView":1,"componentType":5126,"count":frames,"type":"VEC3"}],
        "nodes":(0..bones).map(|i| json!({"name":format!("bone_{i}")})).collect::<Vec<_>>(),
        "scenes":[{"nodes":(0..bones).collect::<Vec<_>>()}], "scene":0,
        "animations":(0..clips).map(|i|json!({"name":format!("clip_{i}"),
            "samplers":[{"input":0,"output":1,"interpolation":"LINEAR"}],
            "channels":[{"sampler":0,"target":{"node":0,"path":"translation"}}]})).collect::<Vec<_>>()
    });
    let path = dir.join(format!("{name}.gltf"));
    std::fs::write(&path, serde_json::to_vec(&source).unwrap()).unwrap();
    path
}

fn run(args: &[&str]) -> Output {
    run_with_memory(args, "524288")
}

fn run_with_memory(args: &[&str], memory_kib: &str) -> Output {
    let output = tempfile::tempdir().unwrap();
    let stdout = output.path().join("stdout");
    let stderr = output.path().join("stderr");
    let mut child = Command::new("bash")
        .args([
            "-c",
            "set -e; ulimit -c 0; ulimit -v \"$1\"; shift; exec \"$@\"",
            "bounded-sampling",
        ])
        .arg(memory_kib)
        .arg(env!("CARGO_BIN_EXE_animsmith"))
        .args(args)
        .stdout(std::fs::File::create(&stdout).unwrap())
        .stderr(std::fs::File::create(&stderr).unwrap())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("sampling command exceeded 20 seconds: {args:?}");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    Output {
        status,
        stdout: std::fs::read(stdout).unwrap(),
        stderr: std::fs::read(stderr).unwrap(),
    }
}

fn refused(args: &[&str], resource: &str) {
    let result = run(args);
    let diagnostic = String::from_utf8_lossy(&result.stderr);
    assert_eq!(result.status.code(), Some(2), "{args:?}: {diagnostic}");
    assert!(result.stdout.is_empty(), "no partial result: {args:?}");
    assert!(result.stderr.len() < 4096);
    assert!(
        diagnostic.contains("sampling")
            && diagnostic.contains(resource)
            && diagnostic.contains("budget exceeded"),
        "{diagnostic}"
    );
}

#[test]
fn required_measurement_commands_refuse_the_small_on_disk_pose_grid_bomb() {
    let dir = tempfile::tempdir().unwrap();
    let bomb = fixture(dir.path(), "bomb", 2048, 4096, 1);
    let small = fixture(dir.path(), "small", 1, 4096, 1);
    assert!(std::fs::metadata(&bomb).unwrap().len() < 100_000);
    assert_eq!(
        std::fs::metadata(dir.path().join("bomb.bin"))
            .unwrap()
            .len(),
        65_536
    );
    let bomb = bomb.to_str().unwrap();
    let small = small.to_str().unwrap();
    for args in [
        vec!["measure", bomb],
        vec![
            "lint",
            bomb,
            "--select",
            "duration-sanity",
            "--allow",
            "duration-sanity",
        ],
        vec!["diff", bomb, small],
        vec!["diff", small, bomb],
    ] {
        refused(&args, "retained bytes");
    }
    let control = run(&["measure", small]);
    assert!(
        control.status.success(),
        "{}",
        String::from_utf8_lossy(&control.stderr)
    );
    let json: Value = serde_json::from_slice(&control.stdout).unwrap();
    assert_eq!(
        json["files"][0]["measurements"]["clips"]["clip_0"]["frame_count"],
        4096
    );
    assert_eq!(
        json["files"][0]["measurements"]["clips"]["clip_0"]["duration_s"],
        1.0
    );
    let report = dir.path().join("measurements.json");
    std::fs::write(&report, control.stdout).unwrap();
    assert!(
        run(&["diff", report.to_str().unwrap(), report.to_str().unwrap()])
            .status
            .success()
    );
    // Both operations intentionally inspect declarations without sampling.
    assert!(run(&["inspect", bomb]).status.success());
    assert!(run(&["generate", "addressability", bomb]).status.success());

    #[cfg(feature = "report")]
    {
        // This grid exceeds the 256 MiB three-array budget while its
        // 12-byte/cell presentation fits the existing 32 MiB comparison cap.
        let comparison_before = fixture(dir.path(), "comparison-before", 2048, 1100, 1);
        let comparison_control = fixture(dir.path(), "comparison-control", 2048, 3, 1);
        let output = dir.path().join("report.html");
        let output_str = output.to_str().unwrap();
        refused(&["report", bomb, "--output", output_str], "retained bytes");
        assert!(!output.exists());
        std::fs::write(&output, b"previous report").unwrap();
        for args in [
            vec!["report", bomb, "--evidence-only", "--output", output_str],
            vec![
                "report",
                comparison_before.to_str().unwrap(),
                "--compare-after",
                comparison_control.to_str().unwrap(),
                "--before-clip",
                "clip_0",
                "--after-clip",
                "clip_0",
                "--output",
                output_str,
            ],
        ] {
            refused(&args, "retained bytes");
            assert_eq!(std::fs::read(&output).unwrap(), b"previous report");
        }
    }
}

#[test]
fn individually_affordable_clips_cannot_accumulate_unbounded_cached_work() {
    let dir = tempfile::tempdir().unwrap();
    // Each grid needs ~59 MiB and 500001 frames. The second grid exceeds
    // the cumulative frame ceiling, before allocating its own pose arrays.
    let input = fixture(dir.path(), "aggregate", 1, 500_001, 2);
    refused(&["measure", input.to_str().unwrap()], "cache frames");
}

#[test]
fn an_admitted_grid_with_unaffordable_buffers_is_an_operator_error() {
    let dir = tempfile::tempdir().unwrap();
    // About 240 MiB fits the fixed grid budget, but cannot fit in this
    // child's 200 MiB address space. Reservation must fail without aborting.
    let input = fixture(dir.path(), "allocation", 512, 4096, 1);
    let result = run_with_memory(&["measure", input.to_str().unwrap()], "204800");
    assert_eq!(
        result.status.code(),
        Some(2),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stdout.is_empty());
    assert!(String::from_utf8_lossy(&result.stderr).contains("sampling could not allocate"));
}

#[test]
fn contact_producer_allocation_failure_preserves_prior_output_and_exits_two() {
    let dir = tempfile::tempdir().unwrap();
    // 999936 pose cells fit contact's stricter million-cell cap, but the
    // three arrays cannot fit this child's 80 MiB address space.
    let input = fixture(dir.path(), "contact-allocation", 512, 1953, 1);
    let output = dir.path().join("contact.json");
    std::fs::write(&output, b"previous contact").unwrap();
    let result = run_with_memory(
        &[
            "generate",
            "contact-fragment",
            input.to_str().unwrap(),
            "--clip",
            "clip_0",
            "--output",
            output.to_str().unwrap(),
            "--format",
            "json",
        ],
        "81920",
    );
    assert_eq!(
        result.status.code(),
        Some(2),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stdout.is_empty());
    assert!(String::from_utf8_lossy(&result.stderr).contains("sampling could not allocate"));
    assert_eq!(std::fs::read(output).unwrap(), b"previous contact");
}
