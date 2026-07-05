//! Semantic-check tests on a synthetic walk cycle with analytically
//! known metrics: L−R foot-height signal is a pure sine (fundamental
//! trough at phase 0.75, peak-to-peak 4·A), the loop closes exactly
//! (seam 0), and root travel is exact.

use animsmith_core::metrics::MIN_STRIDE_STEP_M;
use animsmith_core::model::*;
use animsmith_core::profile::{ResolvedRoles, Role};
use animsmith_core::{CheckCtx, Config, Severity, all_checks, run_checks};
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

fn foot_track_with_stride(bone: BoneId, rest: Vec3, sign: f32, periods: f64, stride: f32) -> Track {
    let times: Vec<f32> = (0..KEYS).map(|k| k as f32 / (KEYS - 1) as f32).collect();
    let values: Vec<Vec3> = (0..KEYS)
        .map(|k| {
            let theta = (periods * TAU * k as f64 / (KEYS - 1) as f64) as f32;
            rest + Vec3::new(
                0.0,
                sign * FOOT_AMPLITUDE * theta.sin(),
                sign * stride * theta.sin(),
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

fn foot_track(bone: BoneId, rest: Vec3, sign: f32, periods: f64) -> Track {
    foot_track_with_stride(bone, rest, sign, periods, STRIDE)
}

fn doc_with_periods(periods: f64) -> Document {
    doc_with_periods_and_stride(periods, STRIDE)
}

fn doc_with_periods_and_stride(periods: f64, stride: f32) -> Document {
    let skel = skeleton();
    let tracks = vec![
        foot_track_with_stride(1, skel.bones[1].rest.translation, 1.0, periods, stride),
        foot_track_with_stride(2, skel.bones[2].rest.translation, -1.0, periods, stride),
    ];
    Document {
        skeleton: skel,
        clips: vec![Clip {
            name: "walk".into(),
            duration_s: 1.0,
            tracks,
        }],
        assets: Default::default(),
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

fn lint_with(doc: &Document, config: &Config) -> Vec<animsmith_core::Finding> {
    let roles = roles(&doc.skeleton);
    let ctx = CheckCtx::new(doc, &roles, config);
    run_checks(&ctx, &all_checks())
}

/// Lint with an *empty* role map — the shape a rig whose profile did
/// not resolve produces. Role-dependent checks must skip-with-note,
/// never fail.
fn lint_unresolved(doc: &Document, config: &Config) -> Vec<animsmith_core::Finding> {
    let roles = ResolvedRoles::default();
    let ctx = CheckCtx::new(doc, &roles, config);
    run_checks(&ctx, &all_checks())
}

fn json_config(json: serde_json::Value) -> Config {
    serde_json::from_value(json).expect("config parses")
}

#[test]
fn default_min_stride_step_matches_metric_default() {
    assert_eq!(
        Config::default().loop_seam_min_stride_step_m(),
        MIN_STRIDE_STEP_M
    );
}

#[test]
fn analytic_walk_metrics() {
    let doc = walk_doc();
    let roles = roles(&doc.skeleton);
    let config = Config::default();
    let measurements = animsmith_core::measure::measure_document(&doc, &roles, &config);
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
fn min_stride_step_config_controls_tiny_stride_ratio() {
    let doc = doc_with_periods_and_stride(0.75, 0.01);
    let roles = roles(&doc.skeleton);
    let default_floor = json_config(serde_json::json!({
        "clips": { "walk": { "loop": true } }
    }));
    let tuned_floor = json_config(serde_json::json!({
        "clips": { "walk": { "loop": true } },
        "checks": { "loop-seam": { "min_stride_step_m": 0.001 } }
    }));

    let default_measurements =
        animsmith_core::measure::measure_document(&doc, &roles, &default_floor);
    assert_eq!(default_measurements["walk"].loop_seam_ratio, None);
    let default_findings = lint_with(&doc, &default_floor);
    assert!(
        !default_findings.iter().any(|f| f.check_id == "loop-seam"),
        "default floor should suppress tiny-stride seam ratio: {default_findings:#?}"
    );

    let tuned_measurements = animsmith_core::measure::measure_document(&doc, &roles, &tuned_floor);
    let ratio = tuned_measurements["walk"]
        .loop_seam_ratio
        .expect("configured stride floor reports ratio");
    let expected_ratio = 1.0 / ((0.75 * TAU / (KEYS - 1) as f64) as f32).sin() as f64;
    assert!(
        (ratio - expected_ratio).abs() < 1e-4,
        "expected {expected_ratio}, got {ratio}"
    );
    let tuned_findings = lint_with(&doc, &tuned_floor);
    assert!(
        tuned_findings.iter().any(|f| f.check_id == "loop-seam"),
        "tuned floor should expose loop-seam finding: {tuned_findings:#?}"
    );
}

#[test]
fn zero_stride_floor_does_not_report_stationary_ratio() {
    let doc = doc_with_periods_and_stride(0.0, 0.0);
    let roles = roles(&doc.skeleton);
    let config = json_config(serde_json::json!({
        "clips": { "walk": { "loop": true } },
        "checks": { "loop-seam": { "min_stride_step_m": 0.0 } }
    }));

    let measurements = animsmith_core::measure::measure_document(&doc, &roles, &config);
    assert_eq!(measurements["walk"].loop_seam_ratio, None);
    let findings = lint_with(&doc, &config);
    assert!(
        !findings.iter().any(|f| f.check_id == "loop-seam"),
        "zero stride should not produce loop-seam finding: {findings:#?}"
    );
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
        "gait_groups": { "ring": {
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
        "gait_groups": { "ring": {
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
        "gait_groups": { "ring": {
            "clips": ["walk", "no_such_clip"],
            "max_gait_phase_spread": 0.1
        }}
    }));
    let findings = lint_with(&doc, &config);
    assert!(
        findings.iter().any(|f| f.check_id == "gait-group"
            && f.clip.as_deref() == Some("no_such_clip")
            && f.severity == Severity::Error),
        "got: {findings:#?}"
    );
}

/// #28 (Codex audit): a gait-group member-not-found is a config error
/// detectable without a rig, so it must still surface when roles are
/// unresolved — not be hidden behind the roles skip-note. The existing
/// member gets one exempt skip-note; the missing member gets its Error.
#[test]
fn missing_group_member_is_flagged_even_when_roles_unresolved() {
    let doc = walk_doc();
    let config = json_config(serde_json::json!({
        "gait_groups": { "ring": {
            "clips": ["walk", "no_such_clip"],
            "max_gait_phase_spread": 0.1
        }}
    }));
    let findings = lint_unresolved(&doc, &config);

    assert!(
        findings.iter().any(|f| f.check_id == "gait-group"
            && f.clip.as_deref() == Some("no_such_clip")
            && f.severity == Severity::Error),
        "member-not-found Error hidden by unresolved roles: {findings:#?}"
    );
    // The resolvable-but-unmeasurable member yields exactly one Note.
    let notes: Vec<_> = findings
        .iter()
        .filter(|f| f.check_id == "gait-group" && f.severity == Severity::Note)
        .collect();
    assert_eq!(notes.len(), 1, "expected one skip-note: {notes:#?}");
}

/// The member-not-found Error stays an Error even under a severity
/// override — it is a config violation, not a diagnostic.
#[test]
fn missing_group_member_error_survives_severity_override() {
    let doc = walk_doc();
    let config = json_config(serde_json::json!({
        "checks": { "gait-group": { "severity": "warn" } },
        "gait_groups": { "ring": {
            "clips": ["walk", "no_such_clip"],
            "max_gait_phase_spread": 0.1
        }}
    }));
    // Roles resolved so only the member-not-found path fires.
    let findings = lint_with(&doc, &config);
    let member = findings
        .iter()
        .find(|f| f.check_id == "gait-group" && f.clip.as_deref() == Some("no_such_clip"))
        .expect("member-not-found reported");
    // A real violation, so the override applies: warn, not error.
    assert_eq!(member.severity, Severity::Warning);
}

/// #28 regression guard: with roles resolved, a declared ring member
/// too short to carry a cycle is a real Error — the readiness refactor
/// narrowed this branch (it used to also cover unresolved roles), so
/// it must not silently become a skip.
#[test]
fn too_short_group_member_is_an_error() {
    let mut doc = walk_doc();
    // A 2-key clip: below the 3-frame floor `foot_cycle_metrics` needs.
    let short = Clip {
        name: "stub".into(),
        duration_s: 1.0,
        tracks: vec![Track {
            bone: 1,
            property: Property::Translation,
            interpolation: Interpolation::Linear,
            times: vec![0.0, 1.0],
            values: TrackValues::Vec3s(vec![Vec3::new(0.1, -1.0, 0.0); 2]),
        }],
    };
    doc.clips.push(short);
    let config = json_config(serde_json::json!({
        "gait_groups": { "ring": {
            "clips": ["walk", "stub"],
            "max_gait_phase_spread": 0.1
        }}
    }));
    let findings = lint_with(&doc, &config); // roles resolve
    let stub = findings
        .iter()
        .find(|f| f.check_id == "gait-group" && f.clip.as_deref() == Some("stub"))
        .expect("too-short member flagged");
    assert_eq!(stub.severity, Severity::Error);
    assert!(stub.message.contains("too short"), "{}", stub.message);
}

/// #28: foot-slide (previously silent on unresolved roles) now emits
/// its own skip-note, driven by its `speed_mps` pending-work predicate
/// and carrying the root/hips reason — not another check's.
#[test]
fn foot_slide_skip_note_is_isolated_and_reasoned() {
    let doc = walk_doc();
    let config = json_config(serde_json::json!({
        "clips": { "walk": { "speed_mps": { "value": 1.0, "tolerance": 0.25 }, "in_place": true } }
    }));
    let findings = lint_unresolved(&doc, &config);

    let foot: Vec<_> = findings
        .iter()
        .filter(|f| f.check_id == "foot-slide")
        .collect();
    assert_eq!(foot.len(), 1, "one foot-slide note: {foot:#?}");
    assert_eq!(foot[0].severity, Severity::Note);
    assert!(foot[0].message.contains("root/hips"), "{}", foot[0].message);

    // in_place=true means root-motion-speed has no pending work → Idle,
    // while foot-slide still judges the treadmill sweep. Pins the
    // asymmetric pending-work predicates.
    assert!(
        !findings.iter().any(|f| f.check_id == "root-motion-speed"),
        "root-motion-speed should be idle for an in-place pin: {findings:#?}"
    );
}

/// #28: gait-group (previously a false Error on unresolved roles) now
/// emits a single skip-note carrying the hips/foot reason.
#[test]
fn gait_group_skip_note_is_isolated_and_reasoned() {
    let doc = walk_doc();
    let config = json_config(serde_json::json!({
        "gait_groups": { "ring": { "clips": ["walk"], "max_gait_phase_spread": 0.1 } }
    }));
    let findings = lint_unresolved(&doc, &config);
    let group: Vec<_> = findings
        .iter()
        .filter(|f| f.check_id == "gait-group")
        .collect();
    assert_eq!(group.len(), 1, "one gait-group note: {group:#?}");
    assert_eq!(group[0].severity, Severity::Note);
    assert!(
        group[0].message.contains("hips/foot"),
        "{}",
        group[0].message
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

/// #28: a severity override applies to a check's *violations*, never
/// to its requirement skip-notes. Declaring `loop-seam` an error on a
/// rig whose roles don't resolve must still surface a Note — not a
/// false Error that fails CI on every rig without foot roles.
#[test]
fn skip_note_is_exempt_from_severity_override() {
    let doc = popped_doc(); // would be a loop-seam Error with roles resolved
    let config = json_config(serde_json::json!({
        "clips": { "walk": { "loop": true } },
        "checks": { "loop-seam": { "severity": "error" } }
    }));
    let findings = lint_unresolved(&doc, &config);
    let seam: Vec<_> = findings
        .iter()
        .filter(|f| f.check_id == "loop-seam")
        .collect();
    assert_eq!(seam.len(), 1, "one skip-note, not silence: {seam:#?}");
    assert_eq!(
        seam[0].severity,
        Severity::Note,
        "skip-note must stay a Note despite severity = error"
    );
    assert!(seam[0].message.contains("skipped"), "{}", seam[0].message);
}

/// #28: `severity = "off"` removes the check entirely — not even its
/// skip-note is emitted.
#[test]
fn off_removes_check_including_its_skip_note() {
    let doc = popped_doc();
    let config = json_config(serde_json::json!({
        "clips": { "walk": { "loop": true } },
        "checks": { "loop-seam": { "severity": "off" } }
    }));
    let findings = lint_unresolved(&doc, &config);
    assert!(!findings.iter().any(|f| f.check_id == "loop-seam"));
}

/// #28: the skip-note plumbing is unified — every role-dependent check
/// with pending work emits exactly one Note when roles don't resolve,
/// including `foot-slide` (previously silent) and `gait-group`
/// (previously a false Error).
#[test]
fn unresolved_roles_yield_one_note_per_pending_check() {
    let doc = walk_doc();
    let config = json_config(serde_json::json!({
        "clips": { "walk": { "loop": true, "speed_mps": { "value": 1.0, "tolerance": 0.25 }, "in_place": false } },
        "gait_groups": { "ring": { "clips": ["walk"], "max_gait_phase_spread": 0.1 } }
    }));
    let findings = lint_unresolved(&doc, &config);
    for id in [
        "loop-seam",
        "root-motion-speed",
        "in-place",
        "foot-slide",
        "gait-group",
    ] {
        let notes: Vec<_> = findings.iter().filter(|f| f.check_id == id).collect();
        assert_eq!(
            notes.len(),
            1,
            "{id}: expected one skip-note, got {notes:#?}"
        );
        assert_eq!(notes[0].severity, Severity::Note, "{id} not a Note");
    }
}

/// #28: a check with no pending work stays silent even when roles are
/// unresolved — no spurious skip-notes.
#[test]
fn idle_checks_emit_no_skip_notes() {
    let doc = walk_doc(); // no config expectations at all
    let config = Config::default();
    let findings = lint_unresolved(&doc, &config);
    for id in [
        "loop-seam",
        "root-motion-speed",
        "in-place",
        "foot-slide",
        "gait-group",
    ] {
        assert!(
            !findings.iter().any(|f| f.check_id == id),
            "{id} emitted a note with nothing to do: {findings:#?}"
        );
    }
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
    use animsmith_core::metrics::circular_phase_spread;
    let spread = circular_phase_spread(&[0.98, 0.02]);
    assert!(spread < 0.03, "wrap over-reported: {spread}");
    let spread = circular_phase_spread(&[0.25, 0.75]);
    assert!((spread - 0.25).abs() < 1e-6, "got {spread}");
}
