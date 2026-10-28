//! Independent binary32 boundaries for slice/hold admission and atomicity.

use animsmith_core::model::{Clip, Interpolation, Property, Track, TrackValues};
use animsmith_core::transform::{TransformTimingError, hold_extend, slice};
use glam::Vec3;

fn linear_clip(times: &[f32]) -> Clip {
    Clip {
        name: "timing".into(),
        duration_s: f64::from(times.last().copied().unwrap_or(0.0)),
        tracks: vec![Track {
            bone: 0,
            property: Property::Translation,
            interpolation: Interpolation::Linear,
            times: times.to_vec(),
            values: TrackValues::Vec3s(
                (0..times.len())
                    .map(|i| Vec3::splat(i as f32 + 1.0))
                    .collect(),
            ),
        }],
    }
}

fn mixed_timing_clip() -> Clip {
    let mut clip = linear_clip(&[0.0, 1.0]);
    clip.tracks[0].bone = 1;
    clip.tracks.insert(
        0,
        Track {
            bone: 0,
            property: Property::Translation,
            interpolation: Interpolation::CubicSpline,
            times: vec![0.0],
            values: TrackValues::Vec3s(vec![Vec3::X, Vec3::splat(2.0), Vec3::Z]),
        },
    );
    clip
}

#[test]
fn tiny_hold_refusal_preserves_earlier_cubic_values_tangents_and_duration() {
    let mut clip = mixed_timing_clip();
    let before = format!("{clip:?}");
    // 1e-20 advances zero, but is far below the 2^-23 spacing at one.
    assert_eq!(
        hold_extend(&mut clip, 1e-20),
        Err(TransformTimingError::NonIncreasingTime { track: 1, key: 2 })
    );
    assert_eq!(format!("{clip:?}"), before);
}

#[test]
fn hold_accepts_the_next_f32_and_a_zero_duration_pose() {
    let mut clip = linear_clip(&[0.0, 1.0]);
    hold_extend(&mut clip, 2.0f64.powi(-23)).unwrap();
    let next = f32::from_bits(1.0f32.to_bits() + 1);
    assert_eq!(clip.tracks[0].times, [0.0, 1.0, next]);
    assert_eq!(clip.tracks[0].key_vec3(2), Some(Vec3::splat(2.0)));
    assert_eq!(clip.duration_s, f64::from(next));

    let mut pose = linear_clip(&[0.0]);
    let mut empty = pose.tracks[0].clone();
    empty.times.clear();
    empty.values = TrackValues::Vec3s(vec![]);
    pose.tracks.push(empty);
    hold_extend(&mut pose, 0.25).unwrap();
    assert_eq!(pose.tracks[0].times, [0.0, 0.25]);
    assert_eq!(pose.tracks[0].key_vec3(1), Some(Vec3::ONE));
    assert!(pose.tracks[1].times.is_empty());
    assert_eq!(pose.duration_s, 0.25);
}

#[test]
fn invalid_controls_refuse_without_changes() {
    for hold in [0.0, -0.25, f64::NAN, f64::INFINITY] {
        let mut clip = linear_clip(&[0.0, 1.0]);
        let before = format!("{clip:?}");
        assert!(matches!(
            hold_extend(&mut clip, hold),
            Err(TransformTimingError::InvalidArgument {
                argument: "hold_s",
                ..
            })
        ));
        assert_eq!(format!("{clip:?}"), before);
    }
    for (start, end, fps) in [
        (0.0, 1.0, 0.0),
        (0.0, 1.0, f64::NAN),
        (f64::NAN, 1.0, 30.0),
        (0.0, f64::INFINITY, 30.0),
        (1.0, 0.0, 30.0),
    ] {
        let mut clip = linear_clip(&[0.0, 1.0]);
        let before = format!("{clip:?}");
        assert!(slice(&mut clip, start, end, fps).is_err());
        assert_eq!(format!("{clip:?}"), before);
    }
}

#[test]
fn finite_hold_overflow_and_invalid_retained_times_refuse() {
    let mut clip = linear_clip(&[0.0, 1.0]);
    let before = format!("{clip:?}");
    assert_eq!(
        hold_extend(&mut clip, f64::MAX),
        Err(TransformTimingError::InvalidKeyTime { track: 0, key: 2 })
    );
    assert_eq!(format!("{clip:?}"), before);

    let mut backwards = linear_clip(&[1.0, 0.0]);
    let before = format!("{backwards:?}");
    assert_eq!(
        hold_extend(&mut backwards, 0.5),
        Err(TransformTimingError::NonIncreasingTime { track: 0, key: 1 })
    );
    assert_eq!(format!("{backwards:?}"), before);
}

#[test]
fn slice_refuses_interior_rounding_collision_before_changing_any_track() {
    let mut clip = mixed_timing_clip();
    let before = format!("{clip:?}");
    // Both 0 + 2^24 and 1 + 2^24 round to 2^24 in binary32. This
    // collision is inside the 2^25 window, so boundary dedup cannot fix it.
    assert_eq!(
        slice(&mut clip, -16_777_216.0, 16_777_216.0, 30.0),
        Err(TransformTimingError::NonIncreasingTime { track: 1, key: 1 })
    );
    assert_eq!(format!("{clip:?}"), before);
}

#[test]
fn slice_refuses_an_interior_key_rounded_onto_end_before_changing_any_track() {
    let mut clip = mixed_timing_clip();
    clip.duration_s = 4.0;
    clip.tracks[1].times = vec![3.5, 4.0];
    clip.tracks[1].values = TrackValues::Vec3s(vec![Vec3::splat(3.5), Vec3::splat(4.0)]);
    let before = format!("{clip:?}");
    // Both 3.5 + 2^24 and 4 + 2^24 round to 16777220. Keeping the first
    // boundary key would replace the actual endpoint's value 4 with 3.5.
    assert_eq!(
        slice(&mut clip, -16_777_216.0, 4.0, 30.0),
        Err(TransformTimingError::RoundedSliceEnd { track: 1, key: 0 })
    );
    assert_eq!(format!("{clip:?}"), before);
}

#[test]
fn slice_preserves_single_frame_negative_start_and_zero_epsilon_uses() {
    let mut pose = linear_clip(&[0.0, 1.0]);
    slice(&mut pose, 1.0, 1.0, 30.0).unwrap();
    assert_eq!(pose.duration_s, 0.0);
    assert_eq!(pose.tracks[0].times, [0.0]);
    assert_eq!(pose.tracks[0].key_vec3(0), Some(Vec3::splat(2.0)));

    let mut negative_start = linear_clip(&[0.0, 0.25]);
    slice(&mut negative_start, -0.25, 0.25, 30.0).unwrap();
    assert_eq!(negative_start.tracks[0].times, [0.25, 0.5]);
    assert_eq!(negative_start.duration_s, 0.5);

    let mut precise = linear_clip(&[0.0, 1.0]);
    slice(&mut precise, 0.0, 1.0, f64::MAX).unwrap();
    assert_eq!(precise.tracks[0].times, [0.0, 1.0]);
}

#[test]
fn finite_slice_intermediates_must_be_representable() {
    for (start, end, fps) in [
        (1.0, 1.0 + 2.0f64.powi(-24), 30.0),
        (-f64::MAX, f64::MAX, 30.0),
        (0.0, 1.0, f64::MIN_POSITIVE),
    ] {
        let mut clip = linear_clip(&[0.0, 1.0]);
        let before = format!("{clip:?}");
        assert_eq!(
            slice(&mut clip, start, end, fps),
            Err(TransformTimingError::UnrepresentableWindow)
        );
        assert_eq!(format!("{clip:?}"), before);
    }
}
