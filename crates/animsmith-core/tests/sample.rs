//! Sampler and FK semantics: these pin the "samples like a game
//! runtime" contract from DESIGN.md §5.

use animsmith_core::model::*;
use animsmith_core::sample::sample_clip;
use glam::{Quat, Vec3};

fn two_bone_skeleton() -> Skeleton {
    Skeleton {
        bones: vec![
            Bone {
                name: "root".into(),
                parent: None,
                rest: Transform::IDENTITY,
                inverse_bind: None,
            },
            Bone {
                name: "child".into(),
                parent: Some(0),
                rest: Transform {
                    translation: Vec3::new(0.0, 1.0, 0.0),
                    ..Transform::IDENTITY
                },
                inverse_bind: None,
            },
        ],
    }
}

fn clip(tracks: Vec<Track>) -> Clip {
    let duration = tracks
        .iter()
        .map(|t| t.end_time() as f64)
        .fold(0.0, f64::max);
    Clip {
        name: "test".into(),
        duration_s: duration,
        tracks,
    }
}

#[test]
fn linear_translation_lerps() {
    let skel = two_bone_skeleton();
    let c = clip(vec![Track {
        bone: 0,
        property: Property::Translation,
        interpolation: Interpolation::Linear,
        times: vec![0.0, 1.0],
        values: TrackValues::Vec3s(vec![Vec3::ZERO, Vec3::new(2.0, 0.0, 0.0)]),
    }]);
    let grid = sample_clip(&skel, &c, 3);
    assert_eq!(grid.times, vec![0.0, 0.5, 1.0]);
    assert!(
        (grid.local(1, 0).translation - Vec3::new(1.0, 0.0, 0.0)).length() < 1e-6,
        "midpoint should lerp to (1,0,0), got {:?}",
        grid.local(1, 0).translation
    );
}

#[test]
fn step_holds_left_key() {
    let skel = two_bone_skeleton();
    let c = clip(vec![Track {
        bone: 0,
        property: Property::Translation,
        interpolation: Interpolation::Step,
        times: vec![0.0, 1.0],
        values: TrackValues::Vec3s(vec![Vec3::ZERO, Vec3::new(2.0, 0.0, 0.0)]),
    }]);
    let grid = sample_clip(&skel, &c, 3);
    assert_eq!(grid.local(1, 0).translation, Vec3::ZERO);
    assert_eq!(grid.local(2, 0).translation, Vec3::new(2.0, 0.0, 0.0));
}

#[test]
fn slerp_takes_shortest_path() {
    // 3.5 rad about Y has w = cos(1.75) < 0, so dot(identity, q1) < 0:
    // naive slerp would go the long way. Shortest path passes through
    // the (2π − 3.5)/2 ≈ 1.39 rad rotation in the *negative* direction.
    let q1 = Quat::from_rotation_y(3.5);
    assert!(
        Quat::IDENTITY.dot(q1) < 0.0,
        "fixture must cross hemispheres"
    );
    let skel = two_bone_skeleton();
    let c = clip(vec![Track {
        bone: 0,
        property: Property::Rotation,
        interpolation: Interpolation::Linear,
        times: vec![0.0, 1.0],
        values: TrackValues::Quats(vec![Quat::IDENTITY, q1]),
    }]);
    let grid = sample_clip(&skel, &c, 3);
    let mid = grid.local(1, 0).rotation;
    let expected_angle = (2.0 * std::f32::consts::PI - 3.5) / 2.0;
    let actual = Quat::IDENTITY.angle_between(mid);
    assert!(
        (actual - expected_angle).abs() < 1e-3,
        "expected shortest-path midpoint angle {expected_angle}, got {actual}"
    );
}

#[test]
fn cubic_spline_with_zero_tangents_matches_hermite() {
    let skel = two_bone_skeleton();
    let c = clip(vec![Track {
        bone: 0,
        property: Property::Translation,
        interpolation: Interpolation::CubicSpline,
        times: vec![0.0, 1.0],
        values: TrackValues::Vec3s(vec![
            Vec3::ZERO,               // in-tangent k0
            Vec3::ZERO,               // value k0
            Vec3::ZERO,               // out-tangent k0
            Vec3::ZERO,               // in-tangent k1
            Vec3::new(2.0, 0.0, 0.0), // value k1
            Vec3::ZERO,               // out-tangent k1
        ]),
    }]);
    let grid = sample_clip(&skel, &c, 3);
    // With zero tangents, h(0.5) = 0.5·p0 + 0.5·p1.
    assert!(
        (grid.local(1, 0).translation - Vec3::new(1.0, 0.0, 0.0)).length() < 1e-6,
        "got {:?}",
        grid.local(1, 0).translation
    );
}

#[test]
fn fk_accumulates_parent_chain() {
    let skel = two_bone_skeleton();
    // Rotate root 90° about Z: child rest offset (0,1,0) should land at (-1,0,0).
    let c = clip(vec![Track {
        bone: 0,
        property: Property::Rotation,
        interpolation: Interpolation::Linear,
        times: vec![0.0, 1.0],
        values: TrackValues::Quats(vec![
            Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
            Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
        ]),
    }]);
    let grid = sample_clip(&skel, &c, 2);
    let child_pos = grid.model_position(0, 1);
    assert!(
        (child_pos - Vec3::new(-1.0, 0.0, 0.0)).length() < 1e-5,
        "got {child_pos:?}"
    );
}

#[test]
fn sampling_clamps_outside_key_range() {
    let skel = two_bone_skeleton();
    // Track keys cover [0.25, 0.75] of a 1s clip: grid endpoints clamp.
    let mut c = clip(vec![Track {
        bone: 0,
        property: Property::Translation,
        interpolation: Interpolation::Linear,
        times: vec![0.25, 0.75],
        values: TrackValues::Vec3s(vec![Vec3::new(1.0, 0.0, 0.0), Vec3::new(3.0, 0.0, 0.0)]),
    }]);
    c.duration_s = 1.0;
    let grid = sample_clip(&skel, &c, 5);
    assert_eq!(grid.local(0, 0).translation, Vec3::new(1.0, 0.0, 0.0));
    assert_eq!(grid.local(4, 0).translation, Vec3::new(3.0, 0.0, 0.0));
    assert!((grid.local(2, 0).translation.x - 2.0).abs() < 1e-6);
}
