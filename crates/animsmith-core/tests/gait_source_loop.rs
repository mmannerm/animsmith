//! Analytic source-boundary admission before a phase shift can hide its seam.

use animsmith_core::model::{
    Bone, Clip, Document, Interpolation, Property, Skeleton, Track, TrackValues, Transform,
};
use animsmith_core::transform::{
    GaitAlignOutcome, GaitLoopPolicy, GaitTrajectoryPolicy, align_gait_anchor,
};
use animsmith_core::{
    CheckCtx, CheckSelection, Config, MetricGrids, ResolvedRoles, Role, all_checks, evaluate_checks,
};
use glam::{Quat, Vec3};

const FPS: f64 = 30.0;
const KEYS: usize = 31;

fn fixture() -> (Document, ResolvedRoles) {
    let skeleton = Skeleton {
        bones: vec![
            Bone {
                name: "hips".into(),
                parent: None,
                rest: Transform::IDENTITY,
                inverse_bind: None,
            },
            Bone {
                name: "left".into(),
                parent: Some(0),
                rest: Transform::IDENTITY,
                inverse_bind: None,
            },
            Bone {
                name: "right".into(),
                parent: Some(0),
                rest: Transform::IDENTITY,
                inverse_bind: None,
            },
            Bone {
                name: "head".into(),
                parent: Some(0),
                rest: Transform::IDENTITY,
                inverse_bind: None,
            },
        ],
    };
    let roles = ResolvedRoles::from_names(
        &skeleton,
        [
            (Role::Hips, "hips".into()),
            (Role::LeftFoot, "left".into()),
            (Role::RightFoot, "right".into()),
        ],
    );
    let tracks = [(1, 1.0), (2, -1.0)]
        .into_iter()
        .map(|(bone, sign)| Track {
            bone,
            property: Property::Translation,
            interpolation: Interpolation::Linear,
            times: (0..KEYS).map(|i| (i as f64 / FPS) as f32).collect(),
            values: TrackValues::Vec3s(
                (0..KEYS)
                    .map(|i| {
                        Vec3::new(
                            sign * 0.1,
                            sign * 0.025
                                * (std::f32::consts::TAU * i as f32 / (KEYS - 1) as f32).sin(),
                            0.0,
                        )
                    })
                    .collect(),
            ),
        })
        .collect();
    (
        Document {
            skeleton,
            clips: vec![Clip {
                name: "walk".into(),
                duration_s: 1.0,
                tracks,
            }],
            ..Document::default()
        },
        roles,
    )
}

fn head_track(values: TrackValues) -> Track {
    Track {
        bone: 3,
        property: if matches!(&values, TrackValues::Quats(_)) {
            Property::Rotation
        } else {
            Property::Translation
        },
        interpolation: Interpolation::Linear,
        times: (0..KEYS).map(|i| (i as f64 / FPS) as f32).collect(),
        values,
    }
}

fn align(
    doc: &mut Document,
    roles: &ResolvedRoles,
    config: &Config,
) -> Result<GaitAlignOutcome, String> {
    let policy = GaitLoopPolicy::from_config(config, &doc.clips[0].name)?;
    align_gait_anchor(
        &doc.skeleton,
        &mut doc.clips[0],
        roles,
        FPS,
        GaitTrajectoryPolicy::InPlace,
        &policy,
    )
}

#[test]
fn source_pose_and_velocity_defects_refuse_without_mutation() {
    let mut position_step = vec![Vec3::ZERO; KEYS];
    position_step[1] = Vec3::X * 0.01; // .01 m / (1/30 s) = .3 m/s outgoing, zero incoming.
    let mut rotation_step = vec![Quat::IDENTITY; KEYS];
    rotation_step[1] = Quat::from_rotation_y(1_f32.to_radians()); // 30 deg/s outgoing, zero incoming.
    let cases = [
        (
            TrackValues::Quats(
                (0..KEYS)
                    .map(|i| Quat::from_rotation_y((5.0 * i as f32 / 30.0).to_radians()))
                    .collect(),
            ),
            "loop-closure",
            "rotation delta (deg)",
            5.0,
        ),
        (
            TrackValues::Vec3s(
                (0..KEYS)
                    .map(|i| Vec3::X * (0.05 * i as f32 / 30.0))
                    .collect(),
            ),
            "loop-closure",
            "position delta (m)",
            0.05,
        ),
        (
            TrackValues::Vec3s(position_step),
            "loop-seam-vel",
            "velocity delta (m/s)",
            0.3,
        ),
        (
            TrackValues::Quats(rotation_step),
            "loop-seam-rot",
            "angular velocity delta (deg/s)",
            30.0,
        ),
    ];
    for (values, check, fact, expected_measurement) in cases {
        let (mut doc, roles) = fixture();
        doc.clips[0].tracks.push(head_track(values));
        let before = format!("{:?}", doc.clips[0]);
        let error = align(&mut doc, &roles, &Config::default()).unwrap_err();
        for expected in [
            "clip \"walk\"",
            "bone \"head\"",
            "source wrap at 1.000000 s",
            check,
            fact,
        ] {
            assert!(error.contains(expected), "missing {expected:?}: {error}");
        }
        let measured: f64 = error
            .split_once(fact)
            .unwrap()
            .1
            .split_whitespace()
            .next()
            .unwrap()
            .parse()
            .unwrap();
        assert!((measured - expected_measurement).abs() < 0.00001, "{error}");
        assert_eq!(format!("{:?}", doc.clips[0]), before);
    }
}

#[test]
fn source_lint_still_reports_the_head_and_clean_cycle_still_aligns() {
    let (mut doc, roles) = fixture();
    let config: Config =
        serde_json::from_value(serde_json::json!({"clips":{"walk":{"loop":true}}})).unwrap();
    doc.clips[0].tracks.push(head_track(TrackValues::Quats(
        (0..KEYS)
            .map(|i| Quat::from_rotation_y((5.0 * i as f32 / 30.0).to_radians()))
            .collect(),
    )));
    let grids = MetricGrids::new(&doc);
    let ctx = CheckCtx::new(&grids, &roles, &config);
    let checks = evaluate_checks(&ctx, &all_checks(), CheckSelection::All).unwrap();
    let findings = checks
        .iter()
        .find(|c| c.check_id() == "loop-closure")
        .unwrap()
        .findings();
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].bone.as_deref(), Some("head"));
    assert_eq!(findings[0].clip.as_deref(), Some("walk"));
    drop(grids);
    doc.clips[0].tracks.pop();
    let outcome = align(&mut doc, &roles, &config).unwrap();
    assert!((outcome.phase_before - 0.75).abs() < 0.05);
    assert!(outcome.phase_after.min(1.0 - outcome.phase_after) < 0.05);
}

#[test]
fn smooth_open_cycle_uses_resolved_per_clip_tolerance_without_requiring_equal_endpoints() {
    let (mut doc, roles) = fixture();
    // 31 samples from a 31-frame open sine cycle: its normal wrap frame is
    // about two degrees. The default 1-degree admission intentionally refuses;
    // the explicit 3-degree policy admits it without closing the pose.
    doc.clips[0].tracks.push(head_track(TrackValues::Quats(
        (0..KEYS)
            .map(|i| {
                Quat::from_rotation_y(
                    (10.0 * (std::f32::consts::TAU * i as f32 / KEYS as f32).sin()).to_radians(),
                )
            })
            .collect(),
    )));
    let error = align(&mut doc, &roles, &Config::default()).unwrap_err();
    assert!(error.contains("rotation delta"), "{error}");
    let config = serde_json::from_value(serde_json::json!({
        "checks":{"loop-closure":{"max_rotation_delta_deg":1.5}},
        "clips":{"*":{"max_loop_rotation_delta_deg":1.75},"walk":{"loop":false,"max_loop_rotation_delta_deg":3.0}}
    })).unwrap();
    align(&mut doc, &roles, &config).unwrap();
}

#[test]
fn missing_non_root_boundary_evidence_refuses() {
    let (mut doc, roles) = fixture();
    doc.clips[0].tracks.push(head_track(TrackValues::Quats(vec![
        Quat::from_xyzw(
            0.0, 0.0, 0.0, 0.0
        );
        KEYS
    ])));
    let before = format!("{:?}", doc.clips[0]);
    let error = align(&mut doc, &roles, &Config::default()).unwrap_err();
    assert!(error.contains("bone \"head\" is unavailable"), "{error}");
    assert_eq!(format!("{:?}", doc.clips[0]), before);
}

#[test]
fn invalid_direct_policy_is_rejected_and_large_loop_caps_do_not_relax_root_safety() {
    for cap in [-1.0, f64::NAN, f64::INFINITY] {
        let mut config = Config::default();
        config
            .clips
            .entry("walk".into())
            .or_default()
            .max_loop_rotation_delta_deg = Some(cap);
        assert!(GaitLoopPolicy::from_config(&config, "walk").is_err());
    }
    let (mut doc, roles) = fixture();
    let mut root = head_track(TrackValues::Vec3s(
        (0..KEYS)
            .map(|i| Vec3::X * (0.02 * i as f32 / 30.0))
            .collect(),
    ));
    root.bone = 0;
    doc.clips[0].tracks.push(root);
    let config = serde_json::from_value(serde_json::json!({"clips":{"walk":{"max_loop_position_delta_m":1000.0,"max_loop_rotation_delta_deg":1000.0,"max_loop_velocity_delta_mps":1000.0,"max_loop_angular_velocity_delta_degps":1000.0}}})).unwrap();
    let error = align(&mut doc, &roles, &config).unwrap_err();
    assert!(error.contains("horizontal translation 0.0200 m"), "{error}");
}

#[test]
fn source_foot_ratio_is_checked_even_when_per_bone_open_pose_caps_admit_the_wrap() {
    let (mut doc, roles) = fixture();
    for track in &mut doc.clips[0].tracks {
        let TrackValues::Vec3s(values) = &mut track.values else {
            unreachable!()
        };
        for (i, value) in values.iter_mut().enumerate() {
            value.z = i as f32 * 0.03;
        }
    }
    // .9 m wrap / about .03 m adjacent step = about 30: C0 is deliberately
    // admitted and C1 is continuous, but the existing foot ratio cap is 1.5.
    let config = serde_json::from_value(
        serde_json::json!({"clips":{"walk":{"max_loop_position_delta_m":1.0}}}),
    )
    .unwrap();
    let before = format!("{:?}", doc.clips[0]);
    let error = align(&mut doc, &roles, &config).unwrap_err();
    assert!(
        error.contains("source wrap at 1.000000 s fails loop-seam: foot ratio"),
        "{error}"
    );
    assert_eq!(format!("{:?}", doc.clips[0]), before);
}
