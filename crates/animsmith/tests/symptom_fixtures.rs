//! Drift guards for the committed symptom fixtures under
//! `examples/assets/` and the contract configs that arm them: one fixture
//! per runtime symptom family, each proved through the real CLI.
//!
//! Every fixture is the clean `walk.glb` motion plus exactly the authored
//! defect its symptom needs, so each test pins that mutation from both
//! sides. The fixture under its config must report exactly the intended
//! findings — check id, severity, and clip/bone subject — and the control
//! must report none of them: the clean walk under the same config (the
//! config alone invents nothing) and, for the contract-aware families, the
//! same fixture with no config (the defect alone judges nothing, because
//! these checks enforce a declared expectation rather than a guess).
//!
//! Expected values are the ones the fixtures are authored with — a 0.25 s
//! channel-end spread, 1.2 m of hip travel, a quarter-cycle phase shift —
//! not numbers read back out of the tool.

use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

/// A path inside the repository, as the CLI receives it.
fn repo_path(relative: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
        .to_str()
        .expect("utf-8 path")
        .to_owned()
}

/// A committed fixture under `examples/assets/`.
fn asset(name: &str) -> String {
    repo_path(&format!("examples/assets/{name}"))
}

/// A committed contract config under `examples/`.
fn config(name: &str) -> String {
    repo_path(&format!("examples/{name}.animsmith.toml"))
}

/// Run `lint --format json` over `args` and return the exit code with the
/// parsed envelope.
fn lint(args: &[&str]) -> (Option<i32>, Value) {
    let output = Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .args(["lint", "--format", "json"])
        .args(args)
        .output()
        .expect("runs animsmith lint");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let json = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("lint --format json is valid JSON ({e}): {stdout}"));
    (output.status.code(), json)
}

/// Every finding in report order as `(check_id, severity, clip, bone)`.
/// An absent subject reads as `""`, so an expectation is a plain literal.
fn findings(json: &Value) -> Vec<(&str, &str, &str, &str)> {
    finding_rows(json)
        .map(|finding| {
            (
                finding["check_id"].as_str().unwrap_or_default(),
                finding["severity"].as_str().unwrap_or_default(),
                finding["clip"].as_str().unwrap_or_default(),
                finding["bone"].as_str().unwrap_or_default(),
            )
        })
        .collect()
}

/// The measured value of the single finding produced by `check_id`.
fn measured(json: &Value, check_id: &str) -> f64 {
    let mut matching = finding_rows(json).filter(|f| f["check_id"] == check_id);
    let finding = matching
        .next()
        .unwrap_or_else(|| panic!("no {check_id} finding in {json}"));
    assert!(
        matching.next().is_none(),
        "expected exactly one {check_id} finding in {json}"
    );
    finding["measured"]
        .as_f64()
        .unwrap_or_else(|| panic!("{check_id} reports a numeric measurement: {finding}"))
}

/// Every finding message in report order.
fn messages(json: &Value) -> Vec<&str> {
    finding_rows(json)
        .map(|finding| finding["message"].as_str().unwrap_or_default())
        .collect()
}

fn finding_rows(json: &Value) -> impl Iterator<Item = &Value> {
    json["files"][0]["checks"]
        .as_array()
        .expect("checks array")
        .iter()
        .flat_map(|check| check["findings"].as_array().expect("findings array"))
}

// --- A limb freezes while the rest keeps moving ----------------------

#[test]
fn short_channel_reports_the_channel_end_spread() {
    // The left foot's rotation channel stops at 0.75 s while both
    // translation channels run to 1.0 s: the engine clamp-holds the short
    // one, so the ankle freezes a quarter cycle before the walk ends.
    let (code, json) = lint(&[&asset("walk-short-channel.glb")]);
    assert_eq!(code, Some(0), "a channel-end spread is a warning");
    assert_eq!(
        findings(&json),
        vec![("duration-sanity", "warning", "walk_short_channel", "")],
    );
    assert!(
        (measured(&json, "duration-sanity") - 0.25).abs() < 1e-6,
        "the authored 0.25 s spread is what is measured: {json}"
    );

    // Control: the clean walk under the same (config-free) command. This
    // check is mechanical, so the fixture's mutation is the only variable.
    let (code, json) = lint(&[&asset("walk.glb")]);
    assert_eq!(code, Some(0));
    assert!(findings(&json).is_empty(), "clean walk has no findings");
}

// --- The character glides or runs in place ---------------------------

#[test]
fn travel_fails_a_gameplay_owned_horizontal_contract() {
    let walk_travel = asset("walk-travel.glb");
    let walk = asset("walk.glb");
    let contract = config("walk-travel-in-place");

    let (code, json) = lint(&["--config", &contract, &walk_travel]);
    assert_eq!(code, Some(1), "a glide is a failing finding");
    assert_eq!(
        findings(&json),
        vec![("in-place", "error", "walk_travel", "")],
    );
    assert!(
        (measured(&json, "in-place") - 1.2).abs() < 1e-3,
        "the authored 1.2 m of travel over a 1 s cycle: {json}"
    );

    // The declaration is what arms the check, not the travel.
    let (code, json) = lint(&[&walk_travel]);
    assert_eq!(code, Some(0), "no declared XZ owner, nothing to judge");
    assert!(findings(&json).is_empty());

    // And the config invents nothing on its own: it declares the fixture's
    // clip, which the clean walk does not carry, so the same run over the
    // unmutated rig is silent.
    let (code, json) = lint(&["--config", &contract, &walk]);
    assert_eq!(code, Some(0), "nothing declared about the clean walk");
    assert!(findings(&json).is_empty());
}

#[test]
fn travel_fails_a_stale_root_motion_speed_pin() {
    let walk_travel = asset("walk-travel.glb");
    let walk = asset("walk.glb");
    let contract = config("walk-travel-root-motion");

    let (code, json) = lint(&["--config", &contract, &walk_travel]);
    assert_eq!(code, Some(1), "a stale speed pin is a failing finding");
    assert_eq!(
        findings(&json),
        vec![("root-motion-speed", "error", "walk_travel", "")],
        "the same travel declared animation-owned misses the 1.0 m/s pin \
         instead of failing in-place"
    );
    assert!(
        (measured(&json, "root-motion-speed") - 1.2).abs() < 1e-3,
        "1.2 m/s measured against the declared 1.0 ± 0.1: {json}"
    );

    let (code, json) = lint(&[&walk_travel]);
    assert_eq!(code, Some(0), "no declared speed, nothing to judge");
    assert!(findings(&json).is_empty());

    // The pin is scoped to the fixture's clip, so it reaches nothing in the
    // clean walk. (Declared over an in-place clip it would fail the other
    // way, as a stray pin — that is a contract error, not this symptom.)
    let (code, json) = lint(&["--config", &contract, &walk]);
    assert_eq!(code, Some(0));
    assert!(findings(&json).is_empty());
}

// --- Directional blend members travel at different speeds -------------

#[test]
fn run_ring_reports_the_phase_shifted_member() {
    let ring = asset("run-ring.glb");
    let contract = config("run-ring");

    let (code, json) = lint(&["--config", &contract, &ring]);
    assert_eq!(code, Some(1), "a spread ring is a failing finding");
    assert_eq!(
        findings(&json),
        vec![("gait-group", "error", "", "")],
        "the phase spread is a group-level finding, not a per-clip one"
    );
    let message = messages(&json)[0];
    assert!(
        message.contains("gait group 'run-ring'"),
        "the finding names the declared ring: {message}"
    );
    assert!(
        message.contains("run_left=0.50"),
        "and the member a quarter cycle away from the other three's 0.75: \
         {message}"
    );
    for coherent in ["run_forward=0.75", "run_backward=0.75", "run_right=0.75"] {
        assert!(
            message.contains(coherent),
            "the three coherent members share a stride anchor: {message}"
        );
    }
    // The rendered spread and cap are what examples/assets/README.md
    // quotes for this fixture, so pin the rendering, not just the phases
    // it was derived from.
    assert!(
        message.contains("spread by 0.20 cycle (cap 0.15)"),
        "the finding prints the spread against the declared cap: {message}"
    );
    // And pin the value analytically. The spread is the largest deviation
    // from the members' circular mean: three unit vectors at phase 0.75
    // (angle -TAU/4) plus one at 0.50 (angle TAU/2) sum to (-1, -3) in
    // (cos, sin), so the mean lies atan(3) radians from the shifted
    // member — arctan(3) / TAU of a cycle, which is 0.1988 and over the
    // declared 0.15 cap.
    let expected_spread = 3.0f64.atan() / std::f64::consts::TAU;
    let measured_spread = measured(&json, "gait-group");
    assert!(
        (measured_spread - expected_spread).abs() < 1e-9,
        "the quarter-cycle shift spreads the ring by {expected_spread}, \
         measured {measured_spread}"
    );

    // The declaration is what arms the check: a blend ring is a runtime
    // intent, not something the four clips reveal on their own.
    let (code, json) = lint(&[&ring]);
    assert_eq!(code, Some(0), "no declared ring, nothing to judge");
    assert!(findings(&json).is_empty());

    // The shift is what fails it: the same document, the same cap, the
    // three members that were not shifted.
    let tmp = tempfile::tempdir().expect("temp dir");
    let coherent = tmp.path().join("coherent-ring.toml");
    std::fs::write(
        &coherent,
        "[gait_groups.run-ring]\n\
         clips = [\"run_forward\", \"run_backward\", \"run_right\"]\n\
         max_gait_phase_spread = 0.15\n\
         min_lr_amplitude_m = 0.03\n",
    )
    .expect("writes control contract");
    let (code, json) = lint(&["--config", coherent.to_str().unwrap(), &ring]);
    assert_eq!(code, Some(0), "the unshifted members agree");
    assert!(findings(&json).is_empty());

    // `walk.glb` is not a ring document, so the committed contract reports
    // its members as absent rather than as spread — the group's presence
    // contract, which is a different finding from the symptom.
    let (code, json) = lint(&["--config", &contract, &asset("walk.glb")]);
    assert_eq!(code, Some(1), "a ring declared over absent clips fails");
    assert_eq!(
        findings(&json),
        vec![
            ("gait-group", "error", "run_forward", ""),
            ("gait-group", "error", "run_backward", ""),
            ("gait-group", "error", "run_left", ""),
            ("gait-group", "error", "run_right", ""),
        ],
        "one per configured member, in the order the ring declares them"
    );
    assert!(
        messages(&json)
            .iter()
            .all(|message| message.contains("member not found in file")),
        "no phase-spread finding without the ring's clips: {json}"
    );
}

// --- A limb is T-posed, or a bone never moves -------------------------

#[test]
fn frozen_arm_reports_the_static_arm_and_the_missing_one() {
    let frozen = asset("walk-frozen-arm.glb");
    let contract = config("walk-frozen-arm");

    let (code, json) = lint(&["--config", &contract, &frozen]);
    assert_eq!(code, Some(1), "declared motion that never happens fails");
    assert_eq!(
        findings(&json),
        vec![
            ("constant-track", "note", "walk_frozen_arm", "arm_l"),
            ("missing-bones", "error", "walk_frozen_arm", "arm_r"),
            ("frozen-bone", "error", "walk_frozen_arm", "arm_l"),
        ],
        "in catalog order: the mechanical note under the static channel, \
         the arm that never reached the file, and the arm that is keyed \
         but never rotates"
    );
    assert_eq!(
        measured(&json, "frozen-bone"),
        0.0,
        "identical rotation keys rotate nowhere: {json}"
    );

    // Without the declaration only the mechanical note survives: nothing
    // in the file says the arms were supposed to move.
    let (code, json) = lint(&[&frozen]);
    assert_eq!(code, Some(0));
    assert_eq!(
        findings(&json),
        vec![("constant-track", "note", "walk_frozen_arm", "arm_l")],
    );

    // Control: the contract declares the fixture's clip, which the clean
    // walk does not carry, so it asserts nothing about the unmutated rig.
    let (code, json) = lint(&["--config", &contract, &asset("walk.glb")]);
    assert_eq!(code, Some(0));
    assert!(findings(&json).is_empty());
}

// --- The file is bloated, or the retargeter chokes --------------------

#[test]
fn scaled_reports_animated_non_uniform_scale_and_the_constant_channel() {
    let scaled = asset("walk-scaled.glb");

    let (code, json) = lint(&[&scaled]);
    assert_eq!(code, Some(0), "scale and bloat are warnings and notes");
    assert_eq!(
        findings(&json),
        vec![
            ("scale-keys", "warning", "walk_scaled", "pelvis"),
            ("non-uniform-scale", "warning", "walk_scaled", "pelvis"),
            ("constant-track", "note", "walk_scaled", "weapon_socket"),
        ],
        "the animated pelvis scale, its unequal axes, and the socket \
         channel that is keyed but never moves"
    );

    // Control: the clean walk under the same (config-free) command. These
    // checks are mechanical, so the mutation is the only variable.
    let (code, json) = lint(&[&asset("walk.glb")]);
    assert_eq!(code, Some(0));
    assert!(findings(&json).is_empty());
}

#[test]
fn pruning_removes_the_constant_socket_channel_and_keeps_the_scale() {
    let tmp = tempfile::Builder::new()
        .prefix("animsmith-symptom-prune-")
        .tempdir()
        .expect("creates temp dir");
    let compact = tmp.path().join("compact.glb");
    let compact = compact.to_str().expect("utf-8 path");

    let output = Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .args([
            "transform",
            &asset("walk-scaled.glb"),
            "-o",
            compact,
            "--prune-constant-tracks",
        ])
        .output()
        .expect("runs animsmith transform");
    assert_eq!(output.status.code(), Some(0), "the prune succeeds");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!(
            "  constant-track removed 'walk_scaled': track index 3 bone \
             'weapon_socket' translation Linear 5 key(s)\nwrote {compact} \
             (4 node(s), 1 clip(s), 0 mesh(es) / 0 position(s), 0 material(s))\n"
        ),
        "the transform names the track it removed"
    );

    // The removal is exactly the note's subject: the two scale warnings
    // survive, because animated scale is authored content, not bloat.
    let (code, json) = lint(&[compact]);
    assert_eq!(code, Some(0));
    assert_eq!(
        findings(&json),
        vec![
            ("scale-keys", "warning", "walk_scaled", "pelvis"),
            ("non-uniform-scale", "warning", "walk_scaled", "pelvis"),
        ],
    );
}
