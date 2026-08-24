//! M3 checks: in-place, fps, bind-pose, foot-slide. The foot-slide
//! fixture is an analytic treadmill walk: flat stance at constant
//! sweep speed, sinusoidal swing — so the expected stance speed is
//! exact.

use animsmith_core::model::*;
use animsmith_core::profile::{ResolvedRoles, Role};
use animsmith_core::{
    CheckCtx, CheckEvaluation, CheckSelection, Config, CoverageGapCode, EvaluationScopeCode,
    EvaluationState, MetricGrids, Severity, Value, all_checks, evaluate_checks,
};
use glam::{Quat, Vec3};

const KEYS: usize = 33; // 32 intervals over 1 s
const STANCE_SWEEP_M: f32 = 0.5; // stance covers ±0.25 m in 0.5 s → 1 m/s

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

/// Treadmill foot: stance (first half of the cycle, phase-offset per
/// foot) sweeps z linearly at constant speed with y = 0; swing lifts
/// the foot and returns it.
fn treadmill_track(bone: BoneId, rest: Vec3, phase_offset: f64, sweep: f32) -> Track {
    let times: Vec<f32> = (0..KEYS).map(|k| k as f32 / (KEYS - 1) as f32).collect();
    let values: Vec<Vec3> = (0..KEYS)
        .map(|k| {
            let u = ((k as f64 / (KEYS - 1) as f64) + phase_offset).rem_euclid(1.0);
            let (dy, dz) = if u < 0.5 {
                // Stance: z from +sweep/2 to −sweep/2, grounded.
                let s = u / 0.5;
                (0.0, (0.5 - s as f32) * sweep)
            } else {
                // Swing: return, lifted.
                let s = (u - 0.5) / 0.5;
                (
                    0.08 * (std::f64::consts::PI * s).sin() as f32,
                    (s as f32 - 0.5) * sweep,
                )
            };
            rest + Vec3::new(0.0, dy, dz)
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

fn treadmill_doc(sweep: f32) -> Document {
    let skel = skeleton();
    Document {
        skeleton: skel.clone(),
        clips: vec![Clip {
            name: "walk".into(),
            duration_s: 1.0,
            tracks: vec![
                treadmill_track(1, skel.bones[1].rest.translation, 0.0, sweep),
                treadmill_track(2, skel.bones[2].rest.translation, 0.5, sweep),
            ],
        }],
        assets: Default::default(),
        source: SourceInfo::default(),
    }
}

fn travelling_doc(root_travel_m: f32) -> Document {
    let mut doc = treadmill_doc(STANCE_SWEEP_M);
    doc.clips[0].tracks.push(Track {
        bone: 0,
        property: Property::Translation,
        interpolation: Interpolation::Linear,
        times: vec![0.0, 1.0],
        values: TrackValues::Vec3s(vec![
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 1.0, root_travel_m),
        ]),
    });
    doc
}

fn evaluate_with(doc: &Document, config: &Config) -> Vec<CheckEvaluation> {
    let roles = roles(&doc.skeleton);
    let grids = MetricGrids::new(doc);
    let ctx = CheckCtx::new(&grids, &roles, config);
    evaluate_checks(&ctx, &all_checks(), CheckSelection::All).expect("valid built-in catalog")
}

fn lint_with(doc: &Document, config: &Config) -> Vec<animsmith_core::Finding> {
    evaluate_with(doc, config)
        .into_iter()
        .flat_map(|check| check.findings().to_vec())
        .collect()
}

fn json_config(json: serde_json::Value) -> Config {
    serde_json::from_value(json).expect("config parses")
}

fn of<'a>(findings: &'a [animsmith_core::Finding], id: &str) -> Vec<&'a animsmith_core::Finding> {
    findings.iter().filter(|f| f.check_id == id).collect()
}

// ---- foot-slide -------------------------------------------------------

#[test]
fn clean_treadmill_passes_foot_slide() {
    let doc = treadmill_doc(STANCE_SWEEP_M);
    let config = json_config(serde_json::json!({
        "clips": { "walk": {
            "in_place": true,
            "speed_mps": { "value": 1.0, "tolerance": 0.25 }
        }}
    }));
    let findings = lint_with(&doc, &config);
    assert!(
        of(&findings, "foot-slide").is_empty(),
        "clean treadmill flagged: {findings:#?}"
    );
    // And the in-place treadmill exemption holds: no stray-pin error.
    assert!(of(&findings, "root-motion-speed").is_empty());
}

#[test]
fn slippery_stance_is_flagged() {
    // Stance sweeps at half the declared speed: 0.5 m/s deviation.
    let doc = treadmill_doc(STANCE_SWEEP_M * 0.5);
    let config = json_config(serde_json::json!({
        "clips": { "walk": {
            "in_place": true,
            "speed_mps": { "value": 1.0, "tolerance": 0.25 }
        }}
    }));
    let findings = lint_with(&doc, &config);
    let slides = of(&findings, "foot-slide");
    assert!(!slides.is_empty(), "slippery stance not flagged");
    assert_eq!(slides[0].severity, Severity::Warning);
}

/// The shared classifier is an internal refactor: this analytic treadmill
/// fixture pins the complete observable foot-slide findings, including their
/// order, coordinates, numeric evidence, selected bones, and prose.
#[test]
fn slippery_stance_keeps_legacy_foot_slide_findings() {
    let doc = treadmill_doc(STANCE_SWEEP_M * 0.5);
    let config = json_config(serde_json::json!({
        "clips": { "walk": {
            "in_place": true,
            "speed_mps": { "value": 1.0, "tolerance": 0.25 }
        }}
    }));
    let findings = lint_with(&doc, &config);
    let slides = of(&findings, "foot-slide");
    let evidence: Vec<_> = slides
        .iter()
        .map(|finding| {
            let measured = match finding.measured.as_ref() {
                Some(Value::Number(value)) => *value,
                value => panic!("expected numeric slide measurement, got {value:?}"),
            };
            let expected = match finding.expected.as_ref() {
                Some(Value::Number(value)) => *value,
                value => panic!("expected numeric slide cap, got {value:?}"),
            };
            (
                finding.bone.as_deref(),
                finding.time_s,
                measured,
                expected,
                finding.message.as_str(),
            )
        })
        .collect();
    assert_eq!(
        evidence,
        [
            (
                Some("l_foot"),
                Some(0.03125),
                0.5,
                0.3,
                "left foot skates during stance: speed deviates 0.50 m/s from the expected 1.00 m/s (cap 0.30) — foot plants will slip at runtime",
            ),
            (
                Some("r_foot"),
                Some(0.03125),
                0.5,
                0.3,
                "right foot skates during stance: speed deviates 0.50 m/s from the expected 1.00 m/s (cap 0.30) — foot plants will slip at runtime",
            ),
        ]
    );
}

/// A rootless foot retains finite model-space X/Z when only its Y is NaN. Its
/// first pair is uniquely fast, so FootSlide exposes the frozen classifier's
/// inclusion of that NaN-touching stance pair as an observable finding.
#[test]
fn foot_slide_keeps_nan_height_as_legacy_stance() {
    let skel = Skeleton {
        bones: vec![
            Bone {
                name: "pelvis".into(),
                parent: None,
                rest: Transform::IDENTITY,
                inverse_bind: None,
            },
            Bone {
                name: "l_foot".into(),
                parent: None,
                rest: Transform::IDENTITY,
                inverse_bind: None,
            },
            Bone {
                name: "r_foot".into(),
                parent: None,
                rest: Transform::IDENTITY,
                inverse_bind: None,
            },
        ],
    };
    let doc = Document {
        skeleton: skel,
        clips: vec![Clip {
            name: "walk".into(),
            duration_s: 1.0,
            tracks: vec![
                Track {
                    bone: 0,
                    property: Property::Translation,
                    interpolation: Interpolation::Linear,
                    times: vec![0.0, 0.5, 1.0],
                    values: TrackValues::Vec3s(vec![
                        Vec3::ZERO,
                        Vec3::new(0.0, 0.0, 0.5),
                        Vec3::new(0.0, 0.0, 1.0),
                    ]),
                },
                Track {
                    bone: 1,
                    property: Property::Translation,
                    interpolation: Interpolation::Linear,
                    times: vec![0.0, 0.5, 1.0],
                    values: TrackValues::Vec3s(vec![
                        Vec3::new(0.0, f32::NAN, 0.0),
                        Vec3::new(0.0, 0.0, 1.0),
                        Vec3::new(0.0, 0.0, 1.0),
                    ]),
                },
                Track {
                    bone: 2,
                    property: Property::Translation,
                    interpolation: Interpolation::Linear,
                    times: vec![0.0, 0.5, 1.0],
                    values: TrackValues::Vec3s(vec![Vec3::ZERO, Vec3::ZERO, Vec3::ZERO]),
                },
            ],
        }],
        assets: Default::default(),
        source: SourceInfo::default(),
    };
    let config = json_config(serde_json::json!({
        "clips": { "walk": {
            "in_place": false,
            "speed_mps": { "value": 1.0, "tolerance": 0.25 }
        }}
    }));

    let findings = lint_with(&doc, &config);
    let slides = of(&findings, "foot-slide");
    assert_eq!(slides.len(), 1);
    let left = slides[0];
    assert_eq!(left.bone.as_deref(), Some("l_foot"));
    assert_eq!(left.time_s, Some(0.5));
    assert!(matches!(left.measured, Some(Value::Number(value)) if value == 2.0));
    assert!(matches!(left.expected, Some(Value::Number(value)) if value == 0.3));
}

#[test]
fn foot_slide_records_partial_evidence_when_one_side_is_unresolved() {
    let doc = treadmill_doc(STANCE_SWEEP_M);
    let roles = ResolvedRoles::from_names(
        &doc.skeleton,
        [
            (Role::Hips, "pelvis".to_string()),
            (Role::LeftFoot, "l_foot".to_string()),
        ],
    );
    let config = json_config(serde_json::json!({
        "clips": { "walk": {
            "in_place": true,
            "speed_mps": { "value": 1.0, "tolerance": 0.25 }
        }}
    }));
    let grids = MetricGrids::new(&doc);
    let ctx = CheckCtx::new(&grids, &roles, &config);
    let records =
        evaluate_checks(&ctx, &all_checks(), CheckSelection::All).expect("valid built-in catalog");
    let foot_slide = records
        .iter()
        .find(|record| record.check_id() == "foot-slide")
        .expect("foot-slide record");

    assert_eq!(foot_slide.evaluation(), EvaluationState::Partial);
    assert!(foot_slide.findings().is_empty());
    assert!(
        foot_slide
            .evaluated_scopes()
            .iter()
            .any(|scope| scope.code.as_str() == "left_foot_stance")
    );
    assert!(
        !foot_slide
            .evaluated_scopes()
            .iter()
            .any(|scope| scope.code.as_str() == "right_foot_stance")
    );
    let right_gap = foot_slide
        .gaps()
        .iter()
        .find(|gap| {
            gap.scope
                .as_ref()
                .is_some_and(|scope| scope.code.as_str() == "right_foot_stance")
        })
        .expect("right-foot coverage gap");
    assert_eq!(right_gap.code, CoverageGapCode::ROLES_UNRESOLVED);
}

#[test]
fn declared_motion_checks_report_unmeasurable_non_finite_root_motion() {
    let mut doc = treadmill_doc(STANCE_SWEEP_M);
    let times = vec![0.0, 0.5, 1.0];
    doc.clips[0].tracks.push(Track {
        bone: 0,
        property: Property::Translation,
        interpolation: Interpolation::Linear,
        times,
        values: TrackValues::Vec3s(vec![
            Vec3::new(-f32::MAX, 1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(f32::MAX, 1.0, 0.0),
        ]),
    });
    let config = json_config(serde_json::json!({
        "clips": { "walk": {
            "in_place": false,
            "speed_mps": { "value": 1.0, "tolerance": 0.25 }
        }}
    }));
    let resolved = roles(&doc.skeleton);
    let grids = MetricGrids::new(&doc);
    let ctx = CheckCtx::new(&grids, &resolved, &config);
    let records =
        evaluate_checks(&ctx, &all_checks(), CheckSelection::All).expect("built-in catalog");

    for id in ["in-place", "root-motion-speed", "foot-slide"] {
        let record = records
            .iter()
            .find(|record| record.check_id() == id)
            .expect("declared-motion check record");
        let gap = record
            .gaps()
            .iter()
            .find(|gap| gap.code == CoverageGapCode::MEASUREMENT_UNAVAILABLE)
            .unwrap_or_else(|| panic!("{id} did not report missing measurement: {record:#?}"));
        let scope = gap.scope.as_ref().unwrap();
        let expected_scope = match id {
            "in-place" => "travel_mode",
            "root-motion-speed" => "root_motion_speed",
            "foot-slide" => "foot_stance",
            _ => unreachable!(),
        };
        assert_eq!(
            (gap.code, scope.code.as_str(), scope.subject.as_deref()),
            (
                CoverageGapCode::MEASUREMENT_UNAVAILABLE,
                expected_scope,
                Some("walk")
            )
        );
        assert!(gap.message.contains("root-motion speed"));
    }
}

#[test]
fn foot_slide_reports_whole_clip_gap_when_metric_grid_is_too_short() {
    let mut doc = treadmill_doc(STANCE_SWEEP_M);
    for track in &mut doc.clips[0].tracks {
        track.times.truncate(2);
        match &mut track.values {
            TrackValues::Vec3s(values) => values.truncate(2),
            _ => panic!("treadmill fixture uses translation tracks"),
        }
    }
    let config = json_config(serde_json::json!({
        "clips": { "walk": {
            "in_place": true,
            "speed_mps": { "value": 1.0, "tolerance": 0.25 }
        }}
    }));
    let resolved = roles(&doc.skeleton);
    let grids = MetricGrids::new(&doc);
    let ctx = CheckCtx::new(&grids, &resolved, &config);
    let records =
        evaluate_checks(&ctx, &all_checks(), CheckSelection::All).expect("built-in catalog");
    let foot_slide = records
        .iter()
        .find(|record| record.check_id() == "foot-slide")
        .expect("foot-slide record");
    let gap = foot_slide
        .gaps()
        .iter()
        .find(|gap| gap.code == CoverageGapCode::MEASUREMENT_UNAVAILABLE)
        .expect("too-short clip gap");
    let scope = gap.scope.as_ref().unwrap();
    assert_eq!(
        (gap.code, scope.code.as_str(), scope.subject.as_deref()),
        (
            CoverageGapCode::MEASUREMENT_UNAVAILABLE,
            "foot_stance",
            Some("walk")
        )
    );
    assert!(gap.message.contains("too short"));
}

/// #57: a rig whose feet resolve only as toe roles (no foot roles) must
/// still be judged — the per-foot loop falls back to the toe, matching
/// `foot_cycle_metrics`. Before the fix the loop skipped both feet and
/// produced silent nothing (readiness said Ready via root/hips).
#[test]
fn toe_only_rig_is_evaluated_for_foot_slide() {
    let doc = treadmill_doc(STANCE_SWEEP_M * 0.5); // slippery: 0.5 m/s deviation
    let roles = ResolvedRoles::from_names(
        &doc.skeleton,
        [
            (Role::Hips, "pelvis".to_string()),
            (Role::LeftToe, "l_foot".to_string()),
            (Role::RightToe, "r_foot".to_string()),
        ],
    );
    let config = json_config(serde_json::json!({
        "clips": { "walk": {
            "in_place": true,
            "speed_mps": { "value": 1.0, "tolerance": 0.25 }
        }}
    }));
    let grids = MetricGrids::new(&doc);
    let ctx = CheckCtx::new(&grids, &roles, &config);
    let findings: Vec<_> = evaluate_checks(&ctx, &all_checks(), CheckSelection::All)
        .expect("valid built-in catalog")
        .into_iter()
        .flat_map(|check| check.findings().to_vec())
        .collect();
    let slides = of(&findings, "foot-slide");
    assert!(
        !slides.is_empty(),
        "toe-only rig produced no foot-slide finding: {findings:#?}"
    );
    assert_eq!(slides[0].severity, Severity::Warning);
}

/// #100: when a side resolves *both* a foot and a toe role, foot-slide
/// must measure (and name) the foot — the `[foot, toe]` preference. The
/// foot bones are slippery (so they flag) while the toe bones are planted
/// cleanly; a foot-first loop names the foot, a toe-first regression
/// would measure the clean toe and either drop the finding or name the
/// toe. Locks the ordering the toe-only test (#57) can't see.
#[test]
fn foot_slide_prefers_foot_over_toe_when_both_resolve() {
    let bone = |name: &str, x: f32, z: f32| Bone {
        name: name.into(),
        parent: Some(0),
        rest: Transform {
            translation: Vec3::new(x, -1.0, z),
            ..Transform::IDENTITY
        },
        inverse_bind: None,
    };
    let skel = Skeleton {
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
            bone("l_foot", 0.1, 0.0),
            bone("r_foot", -0.1, 0.0),
            bone("l_toe", 0.1, 0.1),
            bone("r_toe", -0.1, 0.1),
        ],
    };
    let doc = Document {
        skeleton: skel.clone(),
        clips: vec![Clip {
            name: "walk".into(),
            duration_s: 1.0,
            tracks: vec![
                // Slippery feet (half sweep → 0.5 m/s vs declared 1.0).
                treadmill_track(1, skel.bones[1].rest.translation, 0.0, STANCE_SWEEP_M * 0.5),
                treadmill_track(2, skel.bones[2].rest.translation, 0.5, STANCE_SWEEP_M * 0.5),
                // Clean toes (full sweep → exactly the declared 1.0 m/s).
                treadmill_track(3, skel.bones[3].rest.translation, 0.0, STANCE_SWEEP_M),
                treadmill_track(4, skel.bones[4].rest.translation, 0.5, STANCE_SWEEP_M),
            ],
        }],
        assets: Default::default(),
        source: SourceInfo::default(),
    };
    let roles = ResolvedRoles::from_names(
        &skel,
        [
            (Role::Hips, "pelvis".to_string()),
            (Role::LeftFoot, "l_foot".to_string()),
            (Role::RightFoot, "r_foot".to_string()),
            (Role::LeftToe, "l_toe".to_string()),
            (Role::RightToe, "r_toe".to_string()),
        ],
    );
    let config = json_config(serde_json::json!({
        "clips": { "walk": {
            "in_place": true,
            "speed_mps": { "value": 1.0, "tolerance": 0.25 }
        }}
    }));
    let grids = MetricGrids::new(&doc);
    let ctx = CheckCtx::new(&grids, &roles, &config);
    let findings: Vec<_> = evaluate_checks(&ctx, &all_checks(), CheckSelection::All)
        .expect("valid built-in catalog")
        .into_iter()
        .flat_map(|check| check.findings().to_vec())
        .collect();
    let slides = of(&findings, "foot-slide");
    // Assert the exact set of named bones is BOTH feet — not just "some
    // finding names a foot". A one-sided regression (only the right side
    // reordered to toe-first) still flags the left foot, so a weaker
    // "non-empty and every finding is a foot" oracle would pass it; pin
    // both sides so dropping either fails.
    let mut named: Vec<&str> = slides
        .iter()
        .map(|f| f.bone.as_deref().expect("finding names a bone"))
        .collect();
    named.sort_unstable();
    assert_eq!(
        named,
        ["l_foot", "r_foot"],
        "expected a foot-slide finding on BOTH feet naming the foot bone; a one-sided \
         toe-first (or skipped-side) regression drops one: {findings:#?}"
    );
}

// ---- in-place ---------------------------------------------------------

#[test]
fn travelling_clip_declared_in_place_is_flagged() {
    let doc = travelling_doc(2.0);
    let config = json_config(serde_json::json!({
        "clips": { "walk": { "in_place": true } }
    }));
    let findings = lint_with(&doc, &config);
    let hits = of(&findings, "in-place");
    assert_eq!(hits.len(), 1, "got: {findings:#?}");
    assert_eq!(hits[0].severity, Severity::Error);
}

#[test]
fn stationary_clip_declared_root_motion_is_flagged() {
    let doc = treadmill_doc(STANCE_SWEEP_M);
    let config = json_config(serde_json::json!({
        "clips": { "walk": { "in_place": false } }
    }));
    let findings = lint_with(&doc, &config);
    assert_eq!(of(&findings, "in-place").len(), 1);
}

#[test]
fn canonical_horizontal_owners_drive_the_in_place_check() {
    let travelling = travelling_doc(2.0);
    let stationary = treadmill_doc(STANCE_SWEEP_M);
    let gameplay = json_config(serde_json::json!({
        "clips": { "walk": { "movement_owner_xz": "gameplay" } }
    }));
    let gameplay_hits = lint_with(&travelling, &gameplay);
    let gameplay_hits = of(&gameplay_hits, "in-place");
    assert_eq!(gameplay_hits.len(), 1);
    let legacy_gameplay = json_config(serde_json::json!({
        "clips": { "walk": { "in_place": true } }
    }));
    let legacy_gameplay_hits = lint_with(&travelling, &legacy_gameplay);
    let legacy_gameplay_hits = of(&legacy_gameplay_hits, "in-place");
    assert_eq!(legacy_gameplay_hits.len(), 1);
    assert_eq!(gameplay_hits[0].message, legacy_gameplay_hits[0].message);
    assert!(
        of(&lint_with(&stationary, &gameplay), "in-place").is_empty(),
        "gameplay-owned XZ must accept a stationary clip"
    );

    let animation = json_config(serde_json::json!({
        "clips": { "walk": { "movement_owner_xz": "animation" } }
    }));
    let animation_hits = lint_with(&stationary, &animation);
    let animation_hits = of(&animation_hits, "in-place");
    assert_eq!(animation_hits.len(), 1);
    let legacy_animation = json_config(serde_json::json!({
        "clips": { "walk": { "in_place": false } }
    }));
    let legacy_animation_hits = lint_with(&stationary, &legacy_animation);
    let legacy_animation_hits = of(&legacy_animation_hits, "in-place");
    assert_eq!(legacy_animation_hits.len(), 1);
    assert_eq!(animation_hits[0].message, legacy_animation_hits[0].message);
    assert!(
        of(&lint_with(&travelling, &animation), "in-place").is_empty(),
        "animation-owned XZ must accept a travelling clip"
    );
}

#[test]
fn canonical_gameplay_owner_exempts_treadmill_speed_from_root_motion_check() {
    let doc = treadmill_doc(STANCE_SWEEP_M);
    let config = json_config(serde_json::json!({
        "clips": { "walk": {
            "movement_owner_xz": "gameplay",
            "speed_mps": { "value": 1.0, "tolerance": 0.25 }
        }}
    }));
    let records = evaluate_with(&doc, &config);
    let root_speed = records
        .iter()
        .find(|record| record.check_id() == "root-motion-speed")
        .expect("root-motion-speed record");
    assert_eq!(
        root_speed.applicability(),
        animsmith_core::Applicability::NotApplicable
    );
    assert!(root_speed.findings().is_empty());
    assert!(root_speed.gaps().is_empty());

    let legacy = json_config(serde_json::json!({
        "clips": { "walk": {
            "in_place": true,
            "speed_mps": { "value": 1.0, "tolerance": 0.25 }
        }}
    }));
    let legacy_records = evaluate_with(&doc, &legacy);
    let legacy_root_speed = legacy_records
        .iter()
        .find(|record| record.check_id() == "root-motion-speed")
        .expect("legacy root-motion-speed record");
    assert_eq!(
        root_speed.applicability(),
        legacy_root_speed.applicability()
    );
}

#[test]
fn canonical_animation_owner_runs_the_root_motion_speed_check() {
    let doc = travelling_doc(2.0);
    let matching = json_config(serde_json::json!({
        "clips": { "walk": {
            "movement_owner_xz": "animation",
            "speed_mps": { "value": 2.0, "tolerance": 0.01 }
        }}
    }));
    let matching_records = evaluate_with(&doc, &matching);
    let matching_speed = matching_records
        .iter()
        .find(|record| record.check_id() == "root-motion-speed")
        .expect("root-motion-speed record");
    assert_eq!(
        matching_speed.applicability(),
        animsmith_core::Applicability::Applicable
    );
    assert!(matching_speed.findings().is_empty());
    assert!(matching_speed.gaps().is_empty());
    assert_eq!(matching_speed.evaluated_scopes().len(), 1);
    assert_eq!(
        matching_speed.evaluated_scopes()[0].code,
        EvaluationScopeCode::ROOT_MOTION_SPEED
    );

    let stale = json_config(serde_json::json!({
        "clips": { "walk": {
            "movement_owner_xz": "animation",
            "speed_mps": { "value": 1.0, "tolerance": 0.01 }
        }}
    }));
    let stale_records = evaluate_with(&doc, &stale);
    let stale_speed = stale_records
        .iter()
        .find(|record| record.check_id() == "root-motion-speed")
        .expect("root-motion-speed record");
    assert_eq!(stale_speed.findings().len(), 1);
    assert_eq!(stale_speed.findings()[0].check_id, "root-motion-speed");
    assert!(matches!(
        stale_speed.findings()[0].measured,
        Some(animsmith_core::Value::Number(2.0))
    ));
    assert!(matches!(
        stale_speed.findings()[0].expected,
        Some(animsmith_core::Value::Number(1.0))
    ));

    let stationary_records = evaluate_with(&treadmill_doc(STANCE_SWEEP_M), &stale);
    let stationary_speed = stationary_records
        .iter()
        .find(|record| record.check_id() == "root-motion-speed")
        .expect("root-motion-speed record");
    assert_eq!(
        stationary_speed.applicability(),
        animsmith_core::Applicability::Applicable
    );
    assert_eq!(stationary_speed.findings().len(), 1);
    assert!(stationary_speed.findings()[0].message.contains("stray"));
}

#[test]
fn vertical_and_yaw_intent_do_not_activate_the_horizontal_check() {
    let doc = treadmill_doc(STANCE_SWEEP_M);
    let config = json_config(serde_json::json!({
        "clips": { "walk": {
            "movement_owner_y": "animation",
            "movement_owner_yaw": "gameplay"
        }}
    }));
    let records = evaluate_with(&doc, &config);
    for check_id in ["in-place", "root-motion-speed"] {
        let record = records
            .iter()
            .find(|record| record.check_id() == check_id)
            .expect("check record");
        assert_eq!(
            record.applicability(),
            animsmith_core::Applicability::NotApplicable,
            "{check_id} was activated by Y/yaw intent"
        );
    }
}

#[test]
fn absent_horizontal_owner_does_not_exempt_a_root_motion_speed_pin() {
    let doc = treadmill_doc(STANCE_SWEEP_M);
    let config = json_config(serde_json::json!({
        "clips": { "walk": {
            "movement_owner_y": "animation",
            "movement_owner_yaw": "gameplay",
            "speed_mps": { "value": 1.0, "tolerance": 0.25 }
        }}
    }));
    let records = evaluate_with(&doc, &config);
    let root_speed = records
        .iter()
        .find(|record| record.check_id() == "root-motion-speed")
        .expect("root-motion-speed record");
    assert_eq!(
        root_speed.applicability(),
        animsmith_core::Applicability::Applicable
    );
    assert_eq!(root_speed.findings().len(), 1);
}

#[test]
fn matching_in_place_declaration_is_clean() {
    let doc = treadmill_doc(STANCE_SWEEP_M);
    let config = json_config(serde_json::json!({
        "clips": { "walk": { "in_place": true } }
    }));
    let findings = lint_with(&doc, &config);
    assert!(of(&findings, "in-place").is_empty(), "got: {findings:#?}");
}

// ---- fps --------------------------------------------------------------

#[test]
fn on_grid_keys_pass_fps() {
    let doc = treadmill_doc(STANCE_SWEEP_M); // keys at k/32 over 1 s
    let config = json_config(serde_json::json!({
        "clips": { "walk": { "fps": 32.0 } }
    }));
    let records = evaluate_with(&doc, &config);
    let fps = records
        .iter()
        .find(|record| record.check_id() == "fps")
        .expect("fps record");
    assert!(fps.findings().is_empty(), "got: {:#?}", fps.findings());
    assert_eq!(fps.evaluated_scopes().len(), 1);
    assert_eq!(
        fps.evaluated_scopes()[0].code,
        EvaluationScopeCode::FRAME_GRID
    );
    assert_eq!(fps.evaluated_scopes()[0].subject.as_deref(), Some("walk"));
}

#[test]
fn off_grid_key_and_fractional_duration_are_flagged() {
    let mut doc = treadmill_doc(STANCE_SWEEP_M);
    doc.clips[0].tracks[0].times[5] += 0.011; // ~0.35 frames off at 32 fps
    doc.clips[0].duration_s = 1.013; // 32.4 frames
    let config = json_config(serde_json::json!({
        "clips": { "walk": { "fps": 32.0 } }
    }));
    let findings = lint_with(&doc, &config);
    assert_eq!(of(&findings, "fps").len(), 2, "got: {findings:#?}");
}

#[test]
fn non_finite_declared_fps_is_a_typed_gap() {
    let doc = treadmill_doc(STANCE_SWEEP_M);
    let mut config = Config::default();
    config.clips.insert(
        "walk".into(),
        animsmith_core::config::ClipExpectations {
            fps: Some(f64::NAN),
            ..Default::default()
        },
    );
    let roles = roles(&doc.skeleton);
    let grids = MetricGrids::new(&doc);
    let ctx = CheckCtx::new(&grids, &roles, &config);
    let records =
        evaluate_checks(&ctx, &all_checks(), CheckSelection::All).expect("valid built-in catalog");
    let fps = records
        .iter()
        .find(|record| record.check_id() == "fps")
        .expect("fps record");
    assert_eq!(fps.evaluation(), EvaluationState::NotEvaluated);
    assert_eq!(fps.gaps()[0].code, CoverageGapCode::INVALID_DECLARED_FPS);
    let scope = fps.gaps()[0]
        .scope
        .as_ref()
        .expect("typed frame-grid scope");
    assert_eq!(scope.code, EvaluationScopeCode::FRAME_GRID);
    assert_eq!(scope.subject.as_deref(), Some("walk"));
}

// ---- bind-pose --------------------------------------------------------

fn rotated_first_frame_doc(angle: f32) -> Document {
    let skel = skeleton();
    let tracks = (0..3)
        .map(|bone| Track {
            bone,
            property: Property::Rotation,
            interpolation: Interpolation::Linear,
            times: vec![0.0, 1.0],
            values: TrackValues::Quats(vec![
                Quat::from_rotation_y(angle),
                Quat::from_rotation_y(angle + 0.1),
            ]),
        })
        .collect();
    Document {
        skeleton: skel,
        clips: vec![Clip {
            name: "pose".into(),
            duration_s: 1.0,
            tracks,
        }],
        assets: Default::default(),
        source: SourceInfo::default(),
    }
}

#[test]
fn wrong_bind_is_flagged() {
    // Every bone starts 90° from rest: not a plausible start pose.
    let doc = rotated_first_frame_doc(std::f32::consts::FRAC_PI_2);
    let records = evaluate_with(&doc, &Config::default());
    let bind_pose = records
        .iter()
        .find(|record| record.check_id() == "bind-pose")
        .expect("bind-pose record");
    assert_eq!(bind_pose.findings().len(), 1, "got: {bind_pose:#?}");
    assert_eq!(bind_pose.evaluated_scopes().len(), 1);
    assert_eq!(
        bind_pose.evaluated_scopes()[0].code,
        EvaluationScopeCode::FIRST_FRAME_REST_DELTA
    );
    assert_eq!(
        bind_pose.evaluated_scopes()[0].subject.as_deref(),
        Some("pose")
    );
}

#[test]
fn near_rest_start_is_clean() {
    let findings = lint_with(&rotated_first_frame_doc(0.15), &Config::default());
    assert!(of(&findings, "bind-pose").is_empty(), "got: {findings:#?}");
}

#[test]
fn invalid_rest_rotation_is_insufficient_evidence_not_complete() {
    let mut doc = rotated_first_frame_doc(0.15);
    doc.skeleton.bones[0].rest.rotation = Quat::from_xyzw(0.0, 0.0, 0.0, 0.0);
    let roles = roles(&doc.skeleton);
    let grids = MetricGrids::new(&doc);
    let config = Config::default();
    let ctx = CheckCtx::new(&grids, &roles, &config);
    let records =
        evaluate_checks(&ctx, &all_checks(), CheckSelection::All).expect("valid built-in catalog");
    let bind_pose = records
        .iter()
        .find(|record| record.check_id() == "bind-pose")
        .expect("bind-pose record");
    assert_eq!(bind_pose.evaluation(), EvaluationState::NotEvaluated);
    assert_eq!(
        bind_pose.gaps()[0].code,
        CoverageGapCode::INSUFFICIENT_ROTATION_EVIDENCE
    );
    let scope = bind_pose.gaps()[0]
        .scope
        .as_ref()
        .expect("typed first-frame/rest-delta scope");
    assert_eq!(scope.code, EvaluationScopeCode::FIRST_FRAME_REST_DELTA);
    assert_eq!(scope.subject.as_deref(), Some("pose"));
}
