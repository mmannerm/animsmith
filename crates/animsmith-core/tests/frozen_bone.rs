//! Synthetic cubic curves with independently known motion, through both
//! public evaluation runners. Sampling a curve never proves it is frozen.

use animsmith_core::glam::Quat;
use animsmith_core::model::*;
use animsmith_core::{
    CheckCtx, CheckSelection, Config, EvaluationError, MetricGrids, ResolvedRoles, all_checks,
    evaluate_checks, evaluate_checks_v2,
};

fn cubic(outgoing_z: f32, incoming_z: f32) -> Track {
    let zero = Quat::from_xyzw(0.0, 0.0, 0.0, 0.0);
    Track {
        bone: 0,
        property: Property::Rotation,
        interpolation: Interpolation::CubicSpline,
        times: vec![0.0, 1.0],
        values: TrackValues::Quats(vec![
            zero,
            Quat::IDENTITY,
            Quat::from_xyzw(0.0, 0.0, outgoing_z, 0.0),
            Quat::from_xyzw(0.0, 0.0, incoming_z, 0.0),
            Quat::IDENTITY,
            zero,
        ]),
    }
}

fn document(track: Track) -> Document {
    Document {
        skeleton: Skeleton {
            bones: vec![Bone {
                name: "joint".into(),
                parent: None,
                rest: Transform::IDENTITY,
                inverse_bind: None,
            }],
        },
        clips: vec![Clip {
            name: "curve".into(),
            duration_s: track.end_time() as f64,
            tracks: vec![track],
        }],
        ..Document::default()
    }
}

fn evaluate(
    track: Track,
    floor: f64,
    current: bool,
) -> Result<animsmith_core::CheckEvaluation, EvaluationError> {
    evaluate_document(document(track), floor, current)
}

fn evaluate_document(
    document: Document,
    floor: f64,
    current: bool,
) -> Result<animsmith_core::CheckEvaluation, EvaluationError> {
    let config: Config = serde_json::from_value(serde_json::json!({
        "clips": {"curve": {"animates_bones": ["joint"]}},
        "checks": {"frozen-bone": {"min_rotation_deg": floor}}
    }))
    .unwrap();
    let roles = ResolvedRoles::default();
    let grids = MetricGrids::new(&document);
    let ctx = CheckCtx::new(&grids, &roles, &config);
    let selected = ["frozen-bone".to_owned()].into();
    let checks = all_checks();
    let results = if current {
        evaluate_checks_v2(&ctx, &checks, CheckSelection::Only(&selected))
    } else {
        evaluate_checks(&ctx, &checks, CheckSelection::Only(&selected))
    }?;
    Ok(results
        .into_iter()
        .find(|check| check.check_id() == "frozen-bone")
        .unwrap())
}

#[test]
fn cubic_midpoint_proves_ninety_degrees_without_changing_keyed_metric() {
    // Hermite z(u)=4u(1-u), w(u)=1. At u=1/2 the orientation is
    // normalize([0,0,1,1]), exactly 90 degrees from identity.
    let track = cubic(4.0, -4.0);
    assert_eq!(
        animsmith_core::metrics::rotation_range_deg(&track),
        Some(0.0)
    );
    for current in [false, true] {
        let result = evaluate(track.clone(), 89.0, current).unwrap();
        assert_eq!(
            result.evaluation(),
            animsmith_core::EvaluationState::Complete
        );
        assert!(result.findings().is_empty());
        // Tangents are derivatives: doubling interval duration and halving
        // them leaves the independently known 90-degree midpoint intact.
        let mut stretched = cubic(2.0, -2.0);
        stretched.times[1] = 2.0;
        assert!(
            evaluate(stretched, 89.0, current)
                .unwrap()
                .findings()
                .is_empty()
        );
        // 91 degrees cannot be established by this evidence; never emit a
        // false frozen finding containing the zero-degree keyed range.
        assert!(evaluate(track.clone(), 91.0, current).is_err());
    }
}

#[test]
fn proven_frozen_cubic_and_linear_step_controls_retain_findings() {
    for current in [false, true] {
        let frozen = evaluate(cubic(0.0, 0.0), 1.0, current).unwrap();
        assert_eq!(frozen.findings().len(), 1);
        assert!(matches!(
            frozen.findings()[0].measured,
            Some(animsmith_core::Value::Number(0.0))
        ));
        for interpolation in [Interpolation::Linear, Interpolation::Step] {
            let mut track = cubic(0.0, 0.0);
            track.interpolation = interpolation;
            track.values = TrackValues::Quats(vec![Quat::IDENTITY, -Quat::IDENTITY]);
            assert_eq!(
                evaluate(track.clone(), 1.0, current)
                    .unwrap()
                    .findings()
                    .len(),
                1
            );
            track.values =
                TrackValues::Quats(vec![Quat::IDENTITY, Quat::from_xyzw(0.0, 0.0, 1.0, 1.0)]);
            assert!(
                evaluate(track, 89.0, current)
                    .unwrap()
                    .findings()
                    .is_empty()
            );
        }
    }
}

#[test]
fn midpoint_cancellation_and_hemisphere_crossing_are_explicit_errors() {
    // Equal tangents give z(u)=4u(1-u)(1-2u): endpoints and midpoint
    // are identity, but u=1/4 has z=3/8,w=1 (about 41 degrees).
    // A midpoint-only maximum would falsely call this frozen.
    let off_midpoint = cubic(4.0, 4.0);
    let mut crossing = cubic(0.0, 0.0);
    if let TrackValues::Quats(values) = &mut crossing.values {
        values[4] = -Quat::IDENTITY; // zero raw quaternion at u=1/2
    }
    for current in [false, true] {
        for track in [&off_midpoint, &crossing] {
            let error = evaluate(track.clone(), 1.0, current).unwrap_err();
            assert!(matches!(
                error,
                EvaluationError::Inconclusive {
                    check_id: "frozen-bone",
                    ..
                }
            ));
            let message = error.to_string();
            assert!(
                message.contains("curve") && message.contains("joint") && message.contains("cubic")
            );
        }
    }
}

#[test]
fn malformed_cubic_cannot_be_proven_flat_and_inactive_check_does_not_refuse() {
    let mut track = cubic(0.0, 0.0);
    track.times[1] = 0.0;
    assert!(evaluate(track, 1.0, true).is_err());
    let document = document(cubic(4.0, 4.0));
    let roles = ResolvedRoles::default();
    let grids = MetricGrids::new(&document);
    let config: Config = serde_json::from_value(serde_json::json!({
        "clips": {"curve": {"animates_bones": ["joint"]}},
        "checks": {"frozen-bone": {"severity": "off"}}
    }))
    .unwrap();
    let ctx = CheckCtx::new(&grids, &roles, &config);
    assert!(evaluate_checks_v2(&ctx, &all_checks(), CheckSelection::All).is_ok());
}

#[test]
fn conclusive_keys_survive_a_separate_nonfinite_tangent() {
    let mut track = cubic(0.0, 0.0);
    if let TrackValues::Quats(values) = &mut track.values {
        values[4] = Quat::from_xyzw(0.0, 0.0, 1.0, 1.0); // exactly 90 degrees
        values[0].z = f32::NAN; // unused incoming tangent, owned by nan
    }
    for current in [false, true] {
        let result = evaluate(track.clone(), 1.0, current).unwrap();
        assert_eq!(
            result.evaluation(),
            animsmith_core::EvaluationState::Complete
        );
        assert!(result.findings().is_empty());
    }
}

#[test]
fn another_tracks_motion_witness_prevents_an_inconclusive_bone_decision() {
    let unknown = cubic(4.0, 4.0);
    let moving = Track {
        interpolation: Interpolation::Linear,
        values: TrackValues::Quats(vec![Quat::IDENTITY, Quat::from_xyzw(0.0, 0.0, 1.0, 1.0)]),
        ..unknown.clone()
    };
    for current in [false, true] {
        for tracks in [
            vec![unknown.clone(), moving.clone()],
            vec![moving.clone(), unknown.clone()],
        ] {
            let mut document = document(unknown.clone());
            document.clips[0].tracks = tracks;
            let result = evaluate_document(document, 1.0, current).unwrap();
            assert_eq!(
                result.evaluation(),
                animsmith_core::EvaluationState::Complete
            );
            assert!(result.findings().is_empty());
        }
    }
}
