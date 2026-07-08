//! Drift guards for the examples cookbook (docs/examples.md) and its
//! committed assets (examples/assets/). Two kinds of coverage:
//!
//! 1. `example_assets_match_generator_output` rebuilds the committed
//!    `.glb` from the shared `animsmith-testkit` documents (the same
//!    ones `gen_example_assets` writes) and asserts the bytes match, so
//!    the checked-in assets can never silently drift from the generator.
//! 2. The `cookbook_*` tests run the commands the cookbook documents
//!    against the committed assets and assert each one's exit code plus
//!    one distinctive substring — enough to catch the CLI's contract
//!    drifting out from under the docs, without pinning brittle
//!    verbatim transcripts.

use serde_json::Value;
use std::path::PathBuf;
use std::process::{Command, Output};

fn animsmith() -> Command {
    Command::new(env!("CARGO_BIN_EXE_animsmith"))
}

/// A committed cookbook asset under `examples/assets/`.
fn asset(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/assets")
        .join(name)
}

fn unique_temp_dir(name: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(&format!("animsmith-cookbook-{name}-"))
        .tempdir()
        .expect("creates temp dir")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Run the CLI with `args` and return (exit code, stdout).
fn run(args: &[&str]) -> (Option<i32>, String) {
    let output = animsmith().args(args).output().expect("runs animsmith");
    (output.status.code(), stdout(&output))
}

// --- 1. Committed assets track the generator -------------------------

#[test]
fn example_assets_match_generator_output() {
    // The generator (crates/animsmith/examples/gen_example_assets.rs) and
    // this test both write the committed assets through the same
    // animsmith-testkit `write_example_assets` wiring, so a wrong
    // filename, dropped asset, or swapped clean/dirty document fails here
    // — not just when a human reruns the generator. (#117 replaced an
    // earlier `cargo run --example` subprocess with this in-process build.)
    let tmp = unique_temp_dir("gen");
    animsmith_testkit::write_example_assets(tmp.path(), |doc, path| {
        animsmith_gltf::write::write(doc, path)
    })
    .expect("writes example assets");

    for name in ["clip.glb", "clip-dirty.glb"] {
        let committed = std::fs::read(asset(name)).expect("reads committed asset");
        let regenerated = std::fs::read(tmp.path().join(name))
            .unwrap_or_else(|e| panic!("generator did not write {name}: {e}"));
        if committed != regenerated {
            // Report sizes/offset rather than dumping two 896-byte vectors.
            // A pure length change (identical prefix) has no differing
            // byte, so fall back to the point where the shorter file ends.
            let offset = committed
                .iter()
                .zip(&regenerated)
                .position(|(a, b)| a != b)
                .unwrap_or(committed.len().min(regenerated.len()));
            panic!(
                "examples/assets/{name} is stale ({} committed bytes vs {} regenerated, \
                 first difference at byte {offset}) — regenerate with \
                 `cargo run -p animsmith --example gen_example_assets`",
                committed.len(),
                regenerated.len(),
            );
        }
    }
}

// --- 2. Documented commands still behave as the cookbook shows -------
//
// Covers every command in docs/examples.md that runs against the
// committed assets. The cookbook's remaining commands target placeholder
// or FBX assets this repo does not ship (the `--config … character.glb`
// line, and the convert/report/embed sections), so they are not
// smoke-tested here; the worked config's parse is covered separately by
// `example_config_parses_verbatim` in cli_contract.rs.

#[test]
fn cookbook_first_gate() {
    let clean = asset("clip.glb");
    let clean = clean.to_str().unwrap();
    let dirty = asset("clip-dirty.glb");
    let dirty = dirty.to_str().unwrap();

    let (code, out) = run(&["inspect", clean]);
    assert_eq!(code, Some(0), "inspect clean");
    assert!(out.contains("swing"), "inspect names the clip: {out}");

    let (code, out) = run(&["measure", "--format", "json", clean]);
    assert_eq!(code, Some(0), "measure clean exits 0");
    let doc: Value = serde_json::from_str(&out).expect("measure --format json is valid JSON");
    assert!(
        doc["files"][0]["measurements"].get("swing").is_some(),
        "measure reports the clip's metrics: {out}"
    );

    let (code, out) = run(&["lint", clean]);
    assert_eq!(code, Some(0), "lint clean exits 0");
    assert!(out.contains("clean"), "lint reports clean: {out}");

    let (code, out) = run(&["lint", dirty]);
    assert_eq!(code, Some(1), "lint dirty exits 1");
    assert!(
        out.contains("quat-norm") && out.contains("quat-flip"),
        "lint dirty names both checks: {out}"
    );

    // The documented `--deny-warnings` command exits 1 on the dirty
    // asset and still prints both findings.
    let (code, out) = run(&["lint", "--deny-warnings", dirty]);
    assert_eq!(code, Some(1), "--deny-warnings dirty exits 1");
    assert!(
        out.contains("quat-norm") && out.contains("quat-flip"),
        "--deny-warnings still reports the findings: {out}"
    );

    // Prove the promotion itself: --select isolates the warning (exit 0
    // confirms the quat-norm error was dropped), then --deny-warnings
    // flips that warning-only run to 1.
    let (code, out) = run(&["lint", "--select", "quat-flip", dirty]);
    assert_eq!(code, Some(0), "warning-only run exits 0");
    assert!(
        out.contains("quat-flip") && !out.contains("quat-norm"),
        "--select isolates the warning: {out}"
    );
    let (code, _) = run(&["lint", "--deny-warnings", "--select", "quat-flip", dirty]);
    assert_eq!(code, Some(1), "--deny-warnings promotes the warning");

    let (code, out) = run(&["lint", "--format", "json", dirty]);
    assert_eq!(code, Some(1), "json lint dirty exits 1");
    let doc: Value = serde_json::from_str(&out).expect("lint --format json is valid JSON");
    let ids: Vec<&str> = doc["files"][0]["findings"]
        .as_array()
        .expect("findings array")
        .iter()
        .filter_map(|f| f["check_id"].as_str())
        .collect();
    assert!(
        ids.contains(&"quat-norm") && ids.contains(&"quat-flip"),
        "json findings name both checks: {ids:?}"
    );
}

#[test]
fn cookbook_repair_roundtrip() {
    let dirty = asset("clip-dirty.glb");
    let dirty = dirty.to_str().unwrap();
    let tmp = unique_temp_dir("repair");
    let fixed = tmp.path().join("fixed.glb");
    let fixed = fixed.to_str().unwrap();

    let (code, out) = run(&["fix", "--dry-run", dirty]);
    assert_eq!(code, Some(1), "dry-run with pending repairs exits 1");
    assert!(out.contains("would fix"), "dry-run reports repairs: {out}");

    let (code, _) = run(&["fix", dirty, "-o", fixed]);
    assert_eq!(code, Some(0), "fix -o exits 0");

    let (code, out) = run(&["lint", fixed]);
    assert_eq!(code, Some(0), "repaired asset lints clean");
    assert!(out.contains("clean"), "repaired asset is clean: {out}");

    let (code, out) = run(&["diff", dirty, fixed]);
    assert_eq!(code, Some(0), "lossless repair diffs clean");
    assert!(
        out.contains("no significant movement"),
        "diff reports no movement: {out}"
    );
}

#[test]
fn cookbook_transform() {
    let clean = asset("clip.glb");
    let clean = clean.to_str().unwrap();
    let tmp = unique_temp_dir("transform");
    let sliced = tmp.path().join("sliced.glb");
    let sliced = sliced.to_str().unwrap();
    let held = tmp.path().join("held.glb");
    let held = held.to_str().unwrap();

    let (code, out) = run(&["transform", clean, "-o", sliced, "--slice", "0.5:1.0"]);
    assert_eq!(code, Some(0), "slice exits 0");
    assert!(out.contains("sliced"), "reports the slice: {out}");

    let (code, out) = run(&["diff", clean, sliced]);
    assert_eq!(code, Some(1), "slice moves measurements, diff exits 1");
    // "moved" is a per-metric change line, so it distinguishes a moved
    // diff from the clean `0 significant change(s)` output (which also
    // contains the substring "significant change").
    assert!(out.contains("moved"), "diff lists the moved metrics: {out}");

    let (code, out) = run(&["transform", clean, "-o", held, "--hold-extend", "0.5"]);
    assert_eq!(code, Some(0), "hold-extend exits 0");
    assert!(out.contains("hold-extended"), "reports the hold: {out}");

    // Reuse the written file: the hold extends the clip's duration, so a
    // diff against the source reports movement — guards a no-write success.
    let (code, out) = run(&["diff", clean, held]);
    assert_eq!(code, Some(1), "hold-extend changes the clip, diff exits 1");
    assert!(out.contains("moved"), "diff lists the moved metrics: {out}");
}

#[test]
fn cookbook_config_steering() {
    let dirty = asset("clip-dirty.glb");
    let dirty = dirty.to_str().unwrap();

    // --select runs only the named check.
    let (code, out) = run(&["lint", "--select", "quat-norm", dirty]);
    assert_eq!(code, Some(1), "select quat-norm still errors");
    assert!(out.contains("quat-norm"), "select keeps quat-norm: {out}");
    assert!(!out.contains("quat-flip"), "select drops quat-flip: {out}");

    // --allow suppresses the named check's findings; the quat-norm
    // error still fails the run.
    let (code, out) = run(&["lint", "--allow", "quat-flip", dirty]);
    assert_eq!(code, Some(1), "allow keeps the quat-norm error");
    assert!(
        out.contains("quat-norm") && !out.contains("quat-flip"),
        "allow hides only quat-flip, keeping quat-norm: {out}"
    );

    // A severity override demotes the warning to a note.
    let tmp = unique_temp_dir("config");
    let cfg = tmp.path().join("demote.toml");
    std::fs::write(&cfg, "[checks.quat-flip]\nseverity = \"note\"\n").expect("writes config");
    let (code, out) = run(&["lint", "--config", cfg.to_str().unwrap(), dirty]);
    assert_eq!(code, Some(1), "quat-norm error keeps exit 1");
    assert!(
        out.contains("note[quat-flip]"),
        "override demotes quat-flip to a note: {out}"
    );
}
