//! Drift guards for the Mixamo tutorial (docs/mixamo-tutorial.md) and
//! its committed contract config (examples/mixamo.animsmith.toml).
//!
//! The repo commits no third-party bytes (examples/README.md, "Asset
//! policy"), so the tutorial's claims are exercised against a
//! procedurally generated stand-in: the shared analytic walk rig with
//! Mixamo's `mixamorig:*` bone names and its `mixamo.com` take name.
//! What must hold: the built-in `mixamo` profile resolves the rig
//! through the real CLI, the tutorial's analytic measure numbers stay
//! true, and the committed contract passes a clean in-place walk while
//! failing both mutations of it — a popped loop and a traveling clip —
//! proving the semantic checks fire on a Mixamo-shaped rig even though
//! the profile has no Root role.

use animsmith_core::fixtures::{WALK_STRIDE, WalkBones, walk_doc};
use animsmith_core::glam::Vec3;
use animsmith_core::model::{Document, Interpolation, Property, Track, TrackValues};
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

fn write_doc(dir: &Path, name: &str, doc: &Document) -> String {
    let path = dir.join(name);
    animsmith_gltf::write::write(doc, &path).expect("writes stand-in rig");
    path.to_str().expect("utf-8 path").to_owned()
}

/// Write a mixamorig-named walk covering `periods` cycles into `dir`.
/// 1.0 closes the loop exactly; 0.75 pops the seam.
fn write_walk(dir: &Path, name: &str, periods: f64) -> String {
    let doc = walk_doc(&MIXAMO_BONES, MIXAMO_TAKE, periods, WALK_STRIDE, f64::sin);
    write_doc(dir, name, &doc)
}

/// Write a closed-loop walk whose hips travel `travel_m` forward over
/// the clip — root motion baked into the Hips track, Mixamo-style (the
/// rig has no root bone for it to live on).
fn write_traveling_walk(dir: &Path, name: &str, travel_m: f32) -> String {
    let mut doc = walk_doc(&MIXAMO_BONES, MIXAMO_TAKE, 1.0, WALK_STRIDE, f64::sin);
    let rest = doc.skeleton.bones[0].rest.translation;
    doc.clips[0].tracks.push(Track {
        bone: 0,
        property: Property::Translation,
        interpolation: Interpolation::Linear,
        times: vec![0.0, 1.0],
        values: TrackValues::Vec3s(vec![rest, rest + Vec3::new(0.0, 0.0, travel_m)]),
    });
    write_doc(dir, name, &doc)
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

/// `(check_id, severity)` pairs from a `lint --format json` run.
fn finding_ids(json: &str) -> Vec<(String, String)> {
    let doc: Value = serde_json::from_str(json).expect("lint emits valid JSON");
    doc["files"][0]["findings"]
        .as_array()
        .expect("findings array")
        .iter()
        .map(|f| {
            (
                f["check_id"].as_str().unwrap_or_default().to_owned(),
                f["severity"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect()
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
    // documents — Hips bound by name, no Root role in the map — and
    // the analytic numbers its step 6 reads the contract off of.
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
    let m = &doc["files"][0]["measurements"][MIXAMO_TAKE];
    assert_eq!(
        m["speed_mps"].as_f64(),
        Some(0.0),
        "the in-place stand-in has zero hip travel: {m}"
    );
    assert!(
        m["loop_seam_ratio"].as_f64().expect("seam ratio") < 1e-9,
        "the full cycle closes exactly: {m}"
    );
    let phase = m["gait"]["phase"].as_f64().expect("gait phase");
    assert!(
        (phase - 0.75).abs() < 1e-6,
        "analytic gait phase of the walk fixture: {m}"
    );
}

#[test]
fn tutorial_contract_gates_the_walk() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let clean = write_walk(tmp.path(), "walking.glb", 1.0);
    let popped = write_walk(tmp.path(), "walking-popped.glb", 0.75);
    let traveling = write_traveling_walk(tmp.path(), "walking-traveling.glb", 1.0);
    let config = config_path();

    // The committed contract passes the clean in-place walk: the clip
    // pattern matches the mixamo.com take, the pinned profile resolves
    // the rig, and loop-seam / in-place both judge and pass.
    let (code, out) = run(&["lint", "--config", &config, &clean]);
    assert_eq!(code, Some(0), "clean walk passes the tutorial contract");
    assert!(out.contains("clean"), "clean walk lints clean: {out}");

    // The same contract fails the popped loop, and loop-seam is the
    // *only* finding — the clean rig differs by exactly this (a stale
    // clip name or broken profile pin would skip it and pass both).
    let (code, json) = run(&["lint", "--config", &config, "--format", "json", &popped]);
    assert_eq!(code, Some(1), "popped loop fails the contract");
    let ids = finding_ids(&json);
    assert_eq!(
        ids,
        vec![("loop-seam".to_owned(), "error".to_owned())],
        "the popped seam is the only finding: {ids:?}"
    );

    // And the in-place declaration is judged, not just parsed: a walk
    // whose hips travel fails in-place through the same contract —
    // measured on the Hips track, since the rig has no root bone.
    let (code, json) = run(&["lint", "--config", &config, "--format", "json", &traveling]);
    assert_eq!(code, Some(1), "traveling walk violates the in-place pin");
    let ids = finding_ids(&json);
    assert!(
        ids.contains(&("in-place".to_owned(), "error".to_owned())),
        "in-place fires from the Hips fallback: {ids:?}"
    );
}

#[test]
fn tutorial_mechanical_steps_are_noops_on_the_clean_walk() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let clean = write_walk(tmp.path(), "walking.glb", 1.0);

    // Step 4: a bare lint (mechanical checks only) reports clean.
    let (code, out) = run(&["lint", &clean]);
    assert_eq!(code, Some(0), "bare lint exits 0");
    assert!(out.contains("clean"), "mechanical checks are clean: {out}");

    // Step 5: `fix --dry-run` on a clean file is a no-op that exits 0 —
    // the tutorial's "safe to run unconditionally" claim.
    let (code, out) = run(&["fix", "--dry-run", &clean]);
    assert_eq!(code, Some(0), "no pending repairs exits 0");
    assert!(
        out.contains("0 key(s) would be fixed") && out.contains("no output written"),
        "dry-run reports the no-op without writing: {out}"
    );
}
