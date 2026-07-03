//! Clip transforms: slice, hold-extend, gait-anchor rotation. The
//! gait test uses an OPEN cyclic loop (no duplicated endpoint key),
//! the shape the rotation semantics are defined for: the wrap step is
//! a real frame and the cycle period is `duration + 1/fps`.

use animsmith_core::model::*;
use animsmith_core::profile::{ResolvedRoles, Role};
use animsmith_core::transform::{align_gait_anchor, hold_extend, slice};
use glam::Vec3;
use std::f64::consts::TAU;

const KEYS: usize = 32; // open loop: one full cycle across KEYS frames
const FPS: f64 = 32.0;

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

fn open_loop_foot_track(bone: BoneId, rest: Vec3, sign: f32) -> Track {
    let times: Vec<f32> = (0..KEYS).map(|k| k as f32 / FPS as f32).collect();
    let values: Vec<Vec3> = (0..KEYS)
        .map(|k| {
            let theta = (TAU * k as f64 / KEYS as f64) as f32;
            rest + Vec3::new(0.0, sign * 0.05 * theta.sin(), sign * 0.15 * theta.sin())
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

fn open_walk() -> (Skeleton, Clip) {
    let skel = skeleton();
    let clip = Clip {
        name: "walk".into(),
        duration_s: (KEYS - 1) as f64 / FPS,
        tracks: vec![
            open_loop_foot_track(1, skel.bones[1].rest.translation, 1.0),
            open_loop_foot_track(2, skel.bones[2].rest.translation, -1.0),
        ],
    };
    (skel, clip)
}

fn circular_delta(a: f64, b: f64) -> f64 {
    let d = (a - b).rem_euclid(1.0);
    d.min(1.0 - d)
}

#[test]
fn slice_keeps_window_and_retimes() {
    let (_, mut clip) = open_walk();
    let original = clip.clone();
    slice(&mut clip, 0.25, 0.75, FPS);
    assert!((clip.duration_s - 0.5).abs() < 1e-9);
    let track = &clip.tracks[0];
    assert_eq!(track.times[0], 0.0);
    assert!(track.end_time() <= 0.5 + 0.5 / FPS as f32);
    // Values are the original window's values, untouched.
    let orig_track = &original.tracks[0];
    let first_kept = orig_track
        .times
        .iter()
        .position(|&t| t >= 0.25 - 0.5 / FPS as f32)
        .unwrap();
    assert_eq!(track.key_vec3(0), orig_track.key_vec3(first_kept));
}

#[test]
fn hold_extend_appends_final_pose() {
    let (_, mut clip) = open_walk();
    let before_end = clip.tracks[0].end_time();
    let last = clip.tracks[0].key_vec3(clip.tracks[0].key_count() - 1);
    hold_extend(&mut clip, 1.0);
    let track = &clip.tracks[0];
    assert!((track.end_time() - (before_end + 1.0)).abs() < 1e-5);
    assert_eq!(track.key_vec3(track.key_count() - 1), last);
    assert!((clip.duration_s - (before_end as f64 + 1.0)).abs() < 1e-5);
}

#[test]
fn gait_anchor_rotation_moves_phase_to_zero_losslessly() {
    let (skel, mut clip) = open_walk();
    let roles = roles(&skel);
    let original = clip.clone();

    let outcome = align_gait_anchor(&skel, &mut clip, &roles, FPS).expect("aligns");
    // The synthetic diff signal 2A·sin has its fundamental trough at
    // 0.75 of the cycle.
    assert!(
        (outcome.phase_before - 0.75).abs() < 0.05,
        "before: {}",
        outcome.phase_before
    );
    assert!(
        circular_delta(outcome.phase_after, 0.0) < 0.06,
        "after: {}",
        outcome.phase_after
    );
    // The rotation is lossless: every rotated key equals the original
    // key `shift` frames later (mod the cycle), because quantized
    // shifts land on existing keys.
    let base_shift = (outcome.phase_before * KEYS as f64).round() as i64;
    let shift_keys = (base_shift + outcome.frame_offset as i64).rem_euclid(KEYS as i64) as usize;
    let rotated = &clip.tracks[0];
    let orig = &original.tracks[0];
    let mut matched = 0;
    for k in 0..KEYS {
        let want = orig.key_vec3((k + shift_keys) % KEYS).unwrap();
        let got = rotated.key_vec3(k).unwrap();
        if (got - want).length() < 1e-5 {
            matched += 1;
        }
    }
    assert!(
        matched >= KEYS - 1,
        "only {matched}/{KEYS} keys match a pure {shift_keys}-frame rotation"
    );
}

#[test]
fn gait_anchor_refuses_stationary_clips() {
    let skel = skeleton();
    let roles = roles(&skel);
    let mut clip = Clip {
        name: "idle".into(),
        duration_s: 1.0,
        tracks: vec![Track {
            bone: 1,
            property: Property::Translation,
            interpolation: Interpolation::Linear,
            times: (0..8).map(|k| k as f32 / 8.0).collect(),
            values: TrackValues::Vec3s(vec![Vec3::new(0.1, -1.0, 0.0); 8]),
        }],
    };
    let err = align_gait_anchor(&skel, &mut clip, &roles, FPS).unwrap_err();
    assert!(err.contains("stride anchor"), "got: {err}");
}

#[test]
fn hold_extend_handles_cubic_tracks() {
    let (_, mut clip) = open_walk();
    // Rebuild track 0 as CUBICSPLINE with zero tangents.
    let orig = clip.tracks[0].clone();
    let TrackValues::Vec3s(vals) = &orig.values else {
        unreachable!()
    };
    let mut cubic_vals = Vec::new();
    for v in vals {
        cubic_vals.push(Vec3::ZERO);
        cubic_vals.push(*v);
        cubic_vals.push(Vec3::ZERO);
    }
    clip.tracks[0] = Track {
        interpolation: Interpolation::CubicSpline,
        values: TrackValues::Vec3s(cubic_vals),
        ..orig.clone()
    };
    let last_value = orig.key_vec3(orig.key_count() - 1).unwrap();
    hold_extend(&mut clip, 0.5);
    let track = &clip.tracks[0];
    assert_eq!(track.key_count(), orig.key_count() + 1);
    assert_eq!(track.key_vec3(track.key_count() - 1), Some(last_value));
    // The appended triplet has zero tangents (flat hold).
    let TrackValues::Vec3s(v) = &track.values else {
        unreachable!()
    };
    assert_eq!(v[v.len() - 3], Vec3::ZERO);
    assert_eq!(v[v.len() - 1], Vec3::ZERO);
}
