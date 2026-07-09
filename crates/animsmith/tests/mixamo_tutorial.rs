//! Drift guards for the Mixamo tutorial (docs/mixamo-tutorial.md) and
//! its committed contract config (examples/mixamo.animsmith.toml).
//!
//! The repo commits no third-party bytes (examples/README.md, "Asset
//! policy"), so the tutorial's claims are exercised against a
//! procedurally generated stand-in: the shared analytic walk rig with
//! Mixamo's `mixamorig:*` bone names and its `mixamo.com` take name.
//! What must hold: the built-in `mixamo` profile resolves the rig
//! through the real CLI, and the committed contract passes a clean
//! in-place walk while failing a popped loop — proving the semantic
//! checks fire on a Mixamo-shaped rig even though the profile has no
//! Root role.

use animsmith_core::fixtures::{WALK_STRIDE, WalkBones, walk_doc};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Mixamo's bone names: the built-in `mixamo` profile matches them
/// exactly; the take name below is what Mixamo calls every export.
const MIXAMO_BONES: WalkBones = WalkBones {
    hips: "mixamorig:Hips",
    left_foot: "mixamorig:LeftFoot",
    right_foot: "mixamorig:RightFoot",
};
const MIXAMO_TAKE: &str = "mixamo.com";

fn config_path() -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/mixamo.animsmith.toml")
        .to_str()
        .expect("utf-8 path")
        .to_owned()
}

/// Write a mixamorig-named walk covering `periods` cycles into `dir`.
/// 1.0 closes the loop exactly; 0.75 pops the seam.
fn write_walk(dir: &Path, name: &str, periods: f64) -> String {
    let doc = walk_doc(&MIXAMO_BONES, MIXAMO_TAKE, periods, WALK_STRIDE, f64::sin);
    let path = dir.join(name);
    animsmith_gltf::write::write(&doc, &path).expect("writes stand-in rig");
    path.to_str().expect("utf-8 path").to_owned()
}

/// Run the CLI with `args` and return (exit code, stdout).
fn run(args: &[&str]) -> (Option<i32>, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .args(args)
        .output()
        .expect("runs animsmith");
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
    )
}

#[test]
fn mixamo_profile_resolves_mixamorig_names() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let walk = write_walk(tmp.path(), "walking.glb", 1.0);

    // `inspect` with no config auto-detects the mixamo profile from the
    // mixamorig:* names, as the tutorial's step 3 shows.
    let (code, out) = run(&["inspect", &walk]);
    assert_eq!(code, Some(0), "inspect exits 0");
    assert!(
        out.contains("rig profile: mixamo"),
        "inspect detects the mixamo profile: {out}"
    );

    // `measure --format json` reports the resolved roles the tutorial
    // documents — Hips bound by name, no Root role in the map.
    let (code, out) = run(&["measure", "--format", "json", &walk]);
    assert_eq!(code, Some(0), "measure exits 0");
    let doc: Value = serde_json::from_str(&out).expect("measure emits valid JSON");
    let roles = &doc["files"][0]["rig"]["resolved_roles"];
    assert_eq!(
        roles["hips"], "mixamorig:Hips",
        "hips resolves by exact name: {roles}"
    );
    assert!(
        roles.get("root").is_none(),
        "the mixamo profile has no Root role: {roles}"
    );
}

#[test]
fn tutorial_contract_gates_the_walk() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let clean = write_walk(tmp.path(), "walking.glb", 1.0);
    let popped = write_walk(tmp.path(), "walking-popped.glb", 0.75);
    let config = config_path();

    // The committed contract passes the clean in-place walk: the clip
    // pattern matches the mixamo.com take, the pinned profile resolves
    // the rig, and loop-seam / in-place both judge and pass.
    let (code, out) = run(&["lint", "--config", &config, &clean]);
    assert_eq!(code, Some(0), "clean walk passes the tutorial contract");
    assert!(out.contains("clean"), "clean walk lints clean: {out}");

    // The same contract fails the popped loop, proving the semantic
    // checks actually fire on a Mixamo-shaped rig (a stale clip name or
    // broken profile pin would skip them and pass both files).
    let (code, out) = run(&["lint", "--config", &config, &popped]);
    assert_eq!(code, Some(1), "popped loop fails the contract");
    assert!(out.contains("loop-seam"), "names loop-seam: {out}");
}
