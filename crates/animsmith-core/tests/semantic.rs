//! Semantic-check tests on a synthetic walk cycle with analytically
//! known metrics: L−R foot-height signal is a pure sine (fundamental
//! trough at phase 0.75, peak-to-peak 4·A), the loop closes exactly
//! (seam 0), and root travel is exact.

use animsmith_core::fixtures::{
    self, WALK_FOOT_AMPLITUDE as FOOT_AMPLITUDE, WALK_KEYS as KEYS, WALK_STRIDE as STRIDE,
    WalkBones,
};
use animsmith_core::metrics::MIN_STRIDE_STEP_M;
use animsmith_core::model::*;
use animsmith_core::profile::{ResolvedRoles, Role};
use animsmith_core::{
    CheckCtx, CheckEvaluation, CheckSelection, Config, ConfigurationState, CoverageGapCode,
    EvaluationState, MetricGrids, Severity, all_checks, evaluate_checks,
};
use glam::Vec3;
use std::f64::consts::TAU;

/// The walk rig with explicit-role bone names: `l_foot`/`r_foot` +
/// [`roles`] drive the checks directly, bypassing profile detection
/// (unlike testkit's committed asset, which resolves `ue-mannequin`).
const BONES: WalkBones = WalkBones {
    hips: "pelvis",
    left_foot: "l_foot",
    right_foot: "r_foot",
};

fn skeleton() -> Skeleton {
    BONES.skeleton()
}

/// Explicit hips/left-foot/right-foot role map — the profile-bypass path
/// that drives the checks directly, without relying on profile detection.
fn roles(skel: &Skeleton) -> ResolvedRoles {
    ResolvedRoles::from_names(
        skel,
        [
            (Role::Hips, BONES.hips.to_string()),
            (Role::LeftFoot, BONES.left_foot.to_string()),
            (Role::RightFoot, BONES.right_foot.to_string()),
        ],
    )
}

fn foot_track(bone: BoneId, rest: Vec3, sign: f32, periods: f64) -> Track {
    // Analytic assertions have tolerances, so the platform sine is fine
    // here — no committed bytes depend on it (unlike testkit's `libm::sin`).
    fixtures::foot_track(bone, rest, sign, periods, STRIDE, f64::sin)
}

fn doc_with_periods(periods: f64) -> Document {
    doc_with_periods_and_stride(periods, STRIDE)
}

fn doc_with_periods_and_stride(periods: f64, stride: f32) -> Document {
    fixtures::walk_doc(&BONES, "walk", periods, stride, f64::sin)
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
    let grids = MetricGrids::new(doc);
    let ctx = CheckCtx::new(&grids, &roles, config);
    evaluate_checks(&ctx, &all_checks(), CheckSelection::All)
        .expect("valid built-in catalog")
        .into_iter()
        .flat_map(|check| check.findings)
        .collect()
}

/// Evaluate with an empty role map, preserving typed coverage evidence.
fn evaluate_unresolved(doc: &Document, config: &Config) -> Vec<CheckEvaluation> {
    let roles = ResolvedRoles::default();
    let grids = MetricGrids::new(doc);
    let ctx = CheckCtx::new(&grids, &roles, config);
    evaluate_checks(&ctx, &all_checks(), CheckSelection::All).expect("valid built-in catalog")
}

fn check<'a>(records: &'a [CheckEvaluation], id: &str) -> &'a CheckEvaluation {
    records
        .iter()
        .find(|record| record.check_id == id)
        .unwrap_or_else(|| panic!("missing {id} record"))
}

fn assert_loop_seam_has_unresolved_roles_gap(records: &[CheckEvaluation]) {
    let loop_seam = check(records, "loop-seam");
    assert_eq!(loop_seam.evaluation(), EvaluationState::NotEvaluated);
    assert!(loop_seam.findings.is_empty());
    assert_eq!(loop_seam.gaps[0].code.as_str(), "roles_unresolved");
    assert!(loop_seam.gaps[0].message.contains("hips/foot"));
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
    let grids = MetricGrids::new(&doc);
    let measurements = animsmith_core::measure::measure_document(&grids, &roles, &config);
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
fn check_ctx_does_not_resolve_declarative_rig_config() {
    let doc = walk_doc();
    let inline_config = json_config(serde_json::json!({
        "rig": {
            "profile": "auto",
            "roles": {
                "hips": "pelvis",
                "left_foot": "l_foot",
                "right_foot": "r_foot"
            }
        },
        "clips": { "walk": { "loop": true } }
    }));
    assert_loop_seam_has_unresolved_roles_gap(&evaluate_unresolved(&doc, &inline_config));

    // `ue-mannequin` resolves this renamed walk rig when a frontend applies
    // `Config::rig`; `CheckCtx` must still preserve the supplied empty map.
    let mut profile_doc = doc;
    profile_doc.skeleton.bones[1].name = "foot_l".into();
    profile_doc.skeleton.bones[2].name = "foot_r".into();
    let profile_config = json_config(serde_json::json!({
        "rig": { "profile": "ue-mannequin" },
        "clips": { "walk": { "loop": true } }
    }));
    assert_loop_seam_has_unresolved_roles_gap(&evaluate_unresolved(&profile_doc, &profile_config));
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

    let default_grids = MetricGrids::new(&doc);
    let default_measurements =
        animsmith_core::measure::measure_document(&default_grids, &roles, &default_floor);
    assert_eq!(default_measurements["walk"].loop_seam_ratio, None);
    let default_ctx = CheckCtx::new(&default_grids, &roles, &default_floor);
    let default_records = evaluate_checks(&default_ctx, &all_checks(), CheckSelection::All)
        .expect("valid built-in catalog");
    let seam = check(&default_records, "loop-seam");
    assert_eq!(seam.evaluation(), EvaluationState::NotEvaluated);
    assert!(seam.findings.is_empty());
    assert_eq!(seam.gaps[0].code.as_str(), "measurement_unavailable");
    assert_eq!(
        seam.gaps[0].scope.as_ref().unwrap().subject.as_deref(),
        Some("walk")
    );

    let tuned_grids = MetricGrids::new(&doc);
    let tuned_measurements =
        animsmith_core::measure::measure_document(&tuned_grids, &roles, &tuned_floor);
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

    let grids = MetricGrids::new(&doc);
    let measurements = animsmith_core::measure::measure_document(&grids, &roles, &config);
    assert_eq!(measurements["walk"].loop_seam_ratio, None);
    let ctx = CheckCtx::new(&grids, &roles, &config);
    let records =
        evaluate_checks(&ctx, &all_checks(), CheckSelection::All).expect("valid built-in catalog");
    let seam = check(&records, "loop-seam");
    assert_eq!(seam.evaluation(), EvaluationState::NotEvaluated);
    assert!(seam.findings.is_empty());
    assert_eq!(seam.gaps[0].code.as_str(), "measurement_unavailable");
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

#[test]
fn unresolved_phase_coherence_is_a_typed_gap_not_completed_work() {
    let doc = walk_doc();
    let config = json_config(serde_json::json!({
        "gait_groups": { "ring": {
            "clips": ["walk", "no_such_clip"],
            "max_gait_phase_spread": 0.1
        }}
    }));
    let roles = roles(&doc.skeleton);
    let grids = MetricGrids::new(&doc);
    let ctx = CheckCtx::new(&grids, &roles, &config);
    let checks = all_checks();
    let records = evaluate_checks(&ctx, &checks, CheckSelection::All).unwrap();
    let gait = records
        .iter()
        .find(|record| record.check_id == "gait-group")
        .expect("gait-group record");

    assert_eq!(gait.evaluation(), EvaluationState::Partial);
    assert!(
        gait.evaluated_scopes
            .iter()
            .any(|scope| scope.code == "member_existence")
    );
    assert!(
        !gait
            .evaluated_scopes
            .iter()
            .any(|scope| scope.code == "phase_coherence")
    );
    assert_eq!(
        gait.gaps[0].code.as_str(),
        "insufficient_measurable_members"
    );
    assert_eq!(
        gait.gaps[0].scope.as_ref().unwrap().subject.as_deref(),
        Some("ring")
    );
}

#[test]
fn gait_phase_coverage_distinguishes_complete_and_partial_groups() {
    let mut doc = walk_doc();
    let mut second = doc.clips[0].clone();
    second.name = "walk_b".into();
    doc.clips.push(second);

    for (clips, expected_evaluation, expected_gap) in [
        (vec!["walk", "walk_b"], EvaluationState::Complete, None),
        (
            vec!["walk", "walk_b", "missing"],
            EvaluationState::Partial,
            Some("members_not_evaluated"),
        ),
    ] {
        let config = json_config(serde_json::json!({
            "gait_groups": { "ring": {
                "clips": clips,
                "max_gait_phase_spread": 0.1,
                "min_lr_amplitude_m": 0.05
            }}
        }));
        let roles = roles(&doc.skeleton);
        let grids = MetricGrids::new(&doc);
        let ctx = CheckCtx::new(&grids, &roles, &config);
        let checks = all_checks();
        let records = evaluate_checks(&ctx, &checks, CheckSelection::All).unwrap();
        let gait = records
            .iter()
            .find(|record| record.check_id == "gait-group")
            .expect("gait-group record");

        assert_eq!(gait.evaluation(), expected_evaluation);
        assert!(gait.evaluated_scopes.iter().any(|scope| {
            scope.code == "phase_coherence" && scope.subject.as_deref() == Some("ring")
        }));
        assert_eq!(gait.gaps.first().map(|gap| gap.code.as_str()), expected_gap);
    }
}

/// #28 (Codex audit): a gait-group member-not-found is a config error
/// detectable without a rig, so it must still surface when roles are
/// unresolved. The existing member gets a typed role gap; the missing member
/// remains a content Error.
#[test]
fn missing_group_member_is_flagged_even_when_roles_unresolved() {
    let doc = walk_doc();
    let config = json_config(serde_json::json!({
        "gait_groups": { "ring": {
            "clips": ["walk", "no_such_clip"],
            "max_gait_phase_spread": 0.1
        }}
    }));
    let records = evaluate_unresolved(&doc, &config);
    let gait = check(&records, "gait-group");

    assert!(
        gait.findings
            .iter()
            .any(|finding| finding.clip.as_deref() == Some("no_such_clip")
                && finding.severity == Severity::Error),
        "member-not-found Error hidden by unresolved roles: {gait:#?}"
    );
    assert_eq!(gait.evaluation(), EvaluationState::Partial);
    assert_eq!(gait.gaps[0].code.as_str(), "roles_unresolved");
}

#[test]
fn all_missing_group_members_remain_content_errors_without_a_role_gap() {
    let doc = walk_doc();
    let config = json_config(serde_json::json!({
        "gait_groups": { "ring": {
            "clips": ["missing_a", "missing_b"],
            "max_gait_phase_spread": 0.1
        }}
    }));
    let records = evaluate_unresolved(&doc, &config);
    let gait = check(&records, "gait-group");

    assert_eq!(gait.findings.len(), 2, "missing-member errors: {gait:#?}");
    assert!(
        gait.findings
            .iter()
            .all(|finding| finding.severity == Severity::Error),
        "missing members remain content errors: {gait:#?}"
    );
    assert!(
        gait.gaps
            .iter()
            .all(|gap| gap.code.as_str() != "roles_unresolved")
    );
}

/// A member-not-found result remains a content finding, so the ordinary
/// per-check severity override can demote its default error to a warning.
#[test]
fn missing_group_member_content_error_honors_severity_override() {
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

/// A declared ring member too short to carry a cycle is explicit missing
/// coverage, not a content failure.
#[test]
fn too_short_group_member_is_a_typed_gap() {
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
    let roles = roles(&doc.skeleton);
    let grids = MetricGrids::new(&doc);
    let ctx = CheckCtx::new(&grids, &roles, &config);
    let records = evaluate_checks(&ctx, &all_checks(), CheckSelection::All).unwrap();
    let gait = check(&records, "gait-group");
    assert!(
        gait.findings.is_empty(),
        "measurement gaps do not block: {gait:#?}"
    );
    let gap = gait
        .gaps
        .iter()
        .find(|gap| gap.code.as_str() == "insufficient_measurable_members")
        .expect("too-short member leaves group coverage incomplete");
    assert_eq!(gap.scope.as_ref().unwrap().subject.as_deref(), Some("ring"));
    let member_gap = gait
        .gaps
        .iter()
        .find(|gap| {
            gap.code == CoverageGapCode::MEASUREMENT_UNAVAILABLE
                && gap.scope.as_ref().is_some_and(|scope| {
                    scope.code == "phase_measurement" && scope.subject.as_deref() == Some("stub")
                })
        })
        .expect("unmeasurable member retains clip attribution");
    assert!(member_gap.message.contains("could not be measured"));
}

#[test]
fn every_below_amplitude_group_member_retains_clip_attribution() {
    let mut doc = walk_doc();
    let mut second = doc.clips[0].clone();
    second.name = "walk_b".into();
    doc.clips.push(second);
    let config = json_config(serde_json::json!({
        "gait_groups": { "ring": {
            "clips": ["walk", "walk_b"],
            "max_gait_phase_spread": 0.1,
            "min_lr_amplitude_m": 1.0
        }}
    }));
    let roles = roles(&doc.skeleton);
    let grids = MetricGrids::new(&doc);
    let ctx = CheckCtx::new(&grids, &roles, &config);
    let records = evaluate_checks(&ctx, &all_checks(), CheckSelection::All).unwrap();
    let gait = check(&records, "gait-group");

    let member_subjects: Vec<_> = gait
        .gaps
        .iter()
        .filter(|gap| {
            gap.code == CoverageGapCode::MEASUREMENT_UNAVAILABLE
                && gap
                    .scope
                    .as_ref()
                    .is_some_and(|scope| scope.code == "phase_measurement")
        })
        .map(|gap| {
            assert!(gap.message.contains("below the 1.000 m evidence floor"));
            gap.scope.as_ref().unwrap().subject.as_deref().unwrap()
        })
        .collect();
    assert_eq!(member_subjects, ["walk", "walk_b"]);
}

/// Foot-slide reports its own role gap from its `speed_mps` applicability.
#[test]
fn foot_slide_role_gap_is_isolated_and_reasoned() {
    let doc = walk_doc();
    let config = json_config(serde_json::json!({
        "clips": { "walk": { "speed_mps": { "value": 1.0, "tolerance": 0.25 }, "in_place": true } }
    }));
    let records = evaluate_unresolved(&doc, &config);
    let foot = check(&records, "foot-slide");
    assert_eq!(foot.evaluation(), EvaluationState::NotEvaluated);
    assert!(foot.findings.is_empty());
    assert_eq!(foot.gaps[0].code.as_str(), "roles_unresolved");
    assert!(foot.gaps[0].message.contains("root/hips"));

    // in_place=true means root-motion-speed has no pending work → Idle,
    // while foot-slide still judges the treadmill sweep. Pins the
    // asymmetric pending-work predicates.
    assert!(
        check(&records, "root-motion-speed").applicability
            == animsmith_core::Applicability::NotApplicable,
        "root-motion-speed should be inapplicable for an in-place pin: {records:#?}"
    );
}

/// Gait-group completes member existence and reports phase as a role gap.
#[test]
fn gait_group_role_gap_is_isolated_and_reasoned() {
    let doc = walk_doc();
    let config = json_config(serde_json::json!({
        "gait_groups": { "ring": { "clips": ["walk"], "max_gait_phase_spread": 0.1 } }
    }));
    let records = evaluate_unresolved(&doc, &config);
    let gait = check(&records, "gait-group");
    assert_eq!(gait.evaluation(), EvaluationState::Partial);
    assert!(gait.findings.is_empty());
    assert_eq!(gait.gaps[0].code.as_str(), "roles_unresolved");
    assert!(gait.gaps[0].message.contains("hips/foot"));
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

/// Severity overrides apply to content findings, never coverage gaps.
#[test]
fn coverage_gap_is_exempt_from_severity_override() {
    let doc = popped_doc(); // would be a loop-seam Error with roles resolved
    let config = json_config(serde_json::json!({
        "clips": { "walk": { "loop": true } },
        "checks": { "loop-seam": { "severity": "error" } }
    }));
    let records = evaluate_unresolved(&doc, &config);
    let seam = check(&records, "loop-seam");
    assert_eq!(seam.evaluation(), EvaluationState::NotEvaluated);
    assert!(seam.findings.is_empty());
    assert_eq!(seam.gaps[0].code.as_str(), "roles_unresolved");
}

/// `severity = "off"` records a disabled check without evaluating it.
#[test]
fn off_disables_check_without_emitting_a_gap() {
    let doc = popped_doc();
    let config = json_config(serde_json::json!({
        "clips": { "walk": { "loop": true } },
        "checks": { "loop-seam": { "severity": "off" } }
    }));
    let records = evaluate_unresolved(&doc, &config);
    let seam = check(&records, "loop-seam");
    assert_eq!(seam.configuration, ConfigurationState::Disabled);
    assert_eq!(seam.evaluation(), EvaluationState::NotEvaluated);
    assert!(seam.gaps.is_empty());
}

/// Every applicable role-dependent check reports typed missing coverage.
#[test]
fn unresolved_roles_yield_gaps_for_every_applicable_check() {
    let doc = walk_doc();
    let config = json_config(serde_json::json!({
        "clips": { "walk": { "loop": true, "speed_mps": { "value": 1.0, "tolerance": 0.25 }, "in_place": false } },
        "gait_groups": { "ring": { "clips": ["walk"], "max_gait_phase_spread": 0.1 } }
    }));
    let records = evaluate_unresolved(&doc, &config);
    for id in [
        "loop-seam",
        "root-motion-speed",
        "in-place",
        "foot-slide",
        "gait-group",
    ] {
        let record = check(&records, id);
        assert!(record.findings.is_empty(), "{id}: gaps are not findings");
        assert!(!record.gaps.is_empty(), "{id}: expected a coverage gap");
        assert_eq!(record.gaps[0].code.as_str(), "roles_unresolved");
    }
}

/// A check with no declared work is explicitly not applicable.
#[test]
fn undeclared_checks_are_not_applicable_without_gaps() {
    let doc = walk_doc(); // no config expectations at all
    let config = Config::default();
    let records = evaluate_unresolved(&doc, &config);
    for id in [
        "loop-seam",
        "root-motion-speed",
        "in-place",
        "foot-slide",
        "gait-group",
    ] {
        let record = check(&records, id);
        assert_eq!(
            record.applicability,
            animsmith_core::Applicability::NotApplicable
        );
        assert!(record.findings.is_empty());
        assert!(record.gaps.is_empty());
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
