//! Semantic-check tests on a synthetic walk cycle with analytically
//! known metrics: L−R foot-height signal is a pure sine (fundamental
//! trough at phase 0.75, peak-to-peak 4·A), the loop closes exactly
//! (seam 0), and root travel is exact.

use animlint_core::model::*;
use animlint_core::profile::{ResolvedRoles, Role};
use animlint_core::{CheckCtx, Config, Severity, all_checks, run_checks};
use glam::Vec3;
use std::f64::consts::TAU;

const KEYS: usize = 33; // 32 intervals over 1 s
const FOOT_AMPLITUDE: f32 = 0.05; // vertical swing per foot
const STRIDE: f32 = 0.15; // fore/aft swing per foot

fn skeleton() -> Skeleton {
    Skeleton {
        bones: vec![
            Bone {
                name: "pelvis".into(),
                parent: None,
                rest: Transform {
                    translation: Vec3::new(0.0, 1.0, 0.0),
                    ..Transform::IDENTITY
                },
                inverse_bind: None,
            },
            Bone {
                name: "l_foot".into(),
                parent: Some(0),
                rest: Transform {
                    translation: Vec3::new(0.1, -1.0, 0.0),
                    ..Transform::IDENTITY
                },
                inverse_bind: None,
            },
            Bone {
                name: "r_foot".into(),
                parent: Some(0),
                rest: Transform {
                    translation: Vec3::new(-0.1, -1.0, 0.0),
                    ..Transform::IDENTITY
                },
                inverse_bind: None,
            },
        ],
    }
}

fn roles(skel: &Skeleton) -> ResolvedRoles {
    ResolvedRoles::from_names(
        skel,
        [
            (Role::Hips, "pelvis".to_string()),
            (Role::LeftFoot, "l_foot".to_string()),
            (Role::RightFoot, "r_foot".to_string()),
        ],
    )
}

fn foot_track(bone: BoneId, rest: Vec3, sign: f32, periods: f64) -> Track {
    let times: Vec<f32> = (0..KEYS).map(|k| k as f32 / (KEYS - 1) as f32).collect();
    let values: Vec<Vec3> = (0..KEYS)
        .map(|k| {
            let theta = (periods * TAU * k as f64 / (KEYS - 1) as f64) as f32;
            rest + Vec3::new(
                0.0,
                sign * FOOT_AMPLITUDE * theta.sin(),
                sign * STRIDE * theta.sin(),
            )
        })
        .collect();
    Track {
        bone,
        property: Property::Translation,
        interpolation: Interpolation::Linear,
        times,
        values: TrackValues::Vec3s(values),
    }
}

fn doc_with_periods(periods: f64) -> Document {
    let skel = skeleton();
    let tracks = vec![
        foot_track(1, skel.bones[1].rest.translation, 1.0, periods),
        foot_track(2, skel.bones[2].rest.translation, -1.0, periods),
    ];
    Document {
        skeleton: skel,
        clips: vec![Clip {
            name: "walk".into(),
            duration_s: 1.0,
            tracks,
        }],
        source: SourceInfo::default(),
    }
}

/// A 1 s walk cycle that closes exactly: left foot up when right is
/// down, both returning to their first-frame pose at t = 1.
fn walk_doc() -> Document {
    doc_with_periods(1.0)
}

/// A clip cut mid-cycle (¾ of a period): smooth internally, but the
/// wrap does not return to the first frame — the classic seam pop.
fn popped_doc() -> Document {
    doc_with_periods(0.75)
}

fn lint_with(doc: &Document, config: &Config) -> Vec<animlint_core::Finding> {
    let roles = roles(&doc.skeleton);
    let ctx = CheckCtx::new(doc, &roles, config);
    run_checks(&ctx, &all_checks())
}

fn json_config(json: serde_json::Value) -> Config {
    serde_json::from_value(json).expect("config parses")
}

#[test]
fn analytic_walk_metrics() {
    let doc = walk_doc();
    let roles = roles(&doc.skeleton);
    let measurements = animlint_core::measure::measure_document(&doc, &roles);
    let walk = &measurements["walk"];

    // L−R foot height = 2A·sin(θ): fundamental trough at 0.75.
    let gait = walk.gait.as_ref().expect("gait measured");
    let phase = gait.phase.expect("phase measured");
    assert!(
        (phase - 0.75).abs() < 1e-3,
        "expected phase 0.75, got {phase}"
    );
    // Peak-to-peak of 2A·sin over a sampled period ≈ 4A.
    assert!(
        (gait.lr_amplitude_m - 4.0 * FOOT_AMPLITUDE as f64).abs() < 0.01,
        "expected ~{}, got {}",
        4.0 * FOOT_AMPLITUDE as f64,
        gait.lr_amplitude_m
    );
    // The cycle closes exactly: seam ≈ 0 against a real stride step.
    let seam = walk.loop_seam_ratio.expect("seam measured");
    assert!(seam < 0.1, "expected ~0 seam, got {seam}");
}

#[test]
fn clean_loop_passes_loop_seam() {
    let doc = walk_doc();
    let config = json_config(serde_json::json!({
        "clips": { "walk": { "loop": true } }
    }));
    let findings = lint_with(&doc, &config);
    assert!(
        !findings.iter().any(|f| f.check_id == "loop-seam"),
        "clean loop flagged: {findings:#?}"
    );
}

#[test]
fn seam_pop_is_flagged_on_declared_loop() {
    let doc = popped_doc();
    let config = json_config(serde_json::json!({
        "clips": { "walk": { "loop": true } }
    }));
    let findings = lint_with(&doc, &config);
    let seam = findings
        .iter()
        .find(|f| f.check_id == "loop-seam")
        .expect("seam finding");
    assert_eq!(seam.severity, Severity::Error);
    assert_eq!(seam.clip.as_deref(), Some("walk"));
}

#[test]
fn seam_pop_is_ignored_without_loop_declaration() {
    let doc = popped_doc();
    let findings = lint_with(&doc, &Config::default());
    assert!(
        !findings.iter().any(|f| f.check_id == "loop-seam"),
        "undeclared clip judged: {findings:#?}"
    );
}

#[test]
fn root_motion_speed_matches_declared() {
    let mut doc = walk_doc();
    // Pelvis travels 3 m in Z over the 1 s clip.
    doc.clips[0].tracks.push(Track {
        bone: 0,
        property: Property::Translation,
        interpolation: Interpolation::Linear,
        times: vec![0.0, 1.0],
        values: TrackValues::Vec3s(vec![Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 1.0, 3.0)]),
    });
    let good = json_config(serde_json::json!({
        "clips": { "walk": { "speed_mps": { "value": 3.0, "tolerance": 0.25 } } }
    }));
    let findings = lint_with(&doc, &good);
    assert!(
        !findings.iter().any(|f| f.check_id == "root-motion-speed"),
        "correct pin flagged: {findings:#?}"
    );

    let stale = json_config(serde_json::json!({
        "clips": { "walk": { "speed_mps": { "value": 4.0, "tolerance": 0.25 } } }
    }));
    let findings = lint_with(&doc, &stale);
    let f = findings
        .iter()
        .find(|f| f.check_id == "root-motion-speed")
        .expect("speed finding");
    assert_eq!(f.severity, Severity::Error);
}

#[test]
fn stray_speed_pin_on_stationary_clip_is_flagged() {
    let doc = walk_doc(); // pelvis never travels
    let config = json_config(serde_json::json!({
        "clips": { "walk": { "speed_mps": { "value": 3.0, "tolerance": 0.25 } } }
    }));
    let findings = lint_with(&doc, &config);
    let f = findings
        .iter()
        .find(|f| f.check_id == "root-motion-speed")
        .expect("stray-pin finding");
    assert!(f.message.contains("stray"), "got: {}", f.message);
}

#[test]
fn gait_group_spread_is_flagged() {
    // Second clip with L/R swapped: phase shifts by half a cycle.
    let mut doc = walk_doc();
    let skel = skeleton();
    doc.clips.push(Clip {
        name: "walk_swapped".into(),
        duration_s: 1.0,
        tracks: vec![
            foot_track(1, skel.bones[1].rest.translation, -1.0, 1.0),
            foot_track(2, skel.bones[2].rest.translation, 1.0, 1.0),
        ],
    });
    let config = json_config(serde_json::json!({
        "groups": { "ring": {
            "clips": ["walk", "walk_swapped"],
            "max_gait_phase_spread": 0.1,
            "min_lr_amplitude_m": 0.05
        }}
    }));
    let findings = lint_with(&doc, &config);
    let f = findings
        .iter()
        .find(|f| f.check_id == "gait-group")
        .expect("gait-group finding");
    assert_eq!(f.severity, Severity::Error);
}

#[test]
fn coherent_gait_group_is_clean() {
    let mut doc = walk_doc();
    let mut second = doc.clips[0].clone();
    second.name = "walk_b".into();
    doc.clips.push(second);
    let config = json_config(serde_json::json!({
        "groups": { "ring": {
            "clips": ["walk", "walk_b"],
            "max_gait_phase_spread": 0.1,
            "min_lr_amplitude_m": 0.05
        }}
    }));
    let findings = lint_with(&doc, &config);
    assert!(
        !findings.iter().any(|f| f.check_id == "gait-group"),
        "coherent group flagged: {findings:#?}"
    );
}

#[test]
fn missing_group_member_is_flagged() {
    let doc = walk_doc();
    let config = json_config(serde_json::json!({
        "groups": { "ring": {
            "clips": ["walk", "no_such_clip"],
            "max_gait_phase_spread": 0.1
        }}
    }));
    let findings = lint_with(&doc, &config);
    assert!(
        findings
            .iter()
            .any(|f| f.check_id == "gait-group" && f.clip.as_deref() == Some("no_such_clip")),
        "got: {findings:#?}"
    );
}

#[test]
fn frozen_required_bone_is_flagged() {
    let doc = walk_doc(); // feet have translation tracks, zero rotation
    let config = json_config(serde_json::json!({
        "clips": { "walk": { "animates_bones": ["l_foot"] } }
    }));
    let findings = lint_with(&doc, &config);
    let f = findings
        .iter()
        .find(|f| f.check_id == "frozen-bone")
        .expect("frozen finding");
    assert_eq!(f.bone.as_deref(), Some("l_foot"));
}

#[test]
fn missing_required_bone_is_flagged() {
    let doc = walk_doc();
    let config = json_config(serde_json::json!({
        "clips": { "walk": { "animates_bones": ["no_such_bone"] } }
    }));
    let findings = lint_with(&doc, &config);
    assert!(
        findings
            .iter()
            .any(|f| f.check_id == "missing-bones" && f.bone.as_deref() == Some("no_such_bone")),
        "got: {findings:#?}"
    );
}

#[test]
fn severity_override_and_off() {
    let doc = popped_doc();
    let demoted = json_config(serde_json::json!({
        "clips": { "walk": { "loop": true } },
        "checks": { "loop-seam": { "severity": "note" } }
    }));
    let findings = lint_with(&doc, &demoted);
    let f = findings
        .iter()
        .find(|f| f.check_id == "loop-seam")
        .expect("demoted finding");
    assert_eq!(f.severity, Severity::Note);

    let off = json_config(serde_json::json!({
        "clips": { "walk": { "loop": true } },
        "checks": { "loop-seam": { "severity": "off" } }
    }));
    let findings = lint_with(&doc, &off);
    assert!(!findings.iter().any(|f| f.check_id == "loop-seam"));
}

#[test]
fn glob_expectations_merge_with_exact_winning() {
    let config = json_config(serde_json::json!({
        "clips": {
            "walk_*": { "loop": true, "fps": 30.0 },
            "walk_fast": { "fps": 60.0 }
        }
    }));
    let exp = config.expectations_for("walk_fast");
    assert_eq!(exp.looping, Some(true)); // from the glob
    assert_eq!(exp.fps, Some(60.0)); // exact wins
    let other = config.expectations_for("run_fast");
    assert_eq!(other.looping, None);
}

#[test]
fn circular_spread_handles_wrap() {
    use animlint_core::metrics::circular_phase_spread;
    let spread = circular_phase_spread(&[0.98, 0.02]);
    assert!(spread < 0.03, "wrap over-reported: {spread}");
    let spread = circular_phase_spread(&[0.25, 0.75]);
    assert!((spread - 0.25).abs() < 1e-6, "got {spread}");
}
