//! Clip transforms: slice, hold-extend, gait-anchor rotation. The
//! gait test uses an OPEN cyclic loop (no duplicated endpoint key),
//! the shape the rotation semantics are defined for: the wrap step is
//! a real frame and the cycle period is `duration + 1/fps`.

use animsmith_core::model::*;
use animsmith_core::profile::{ResolvedRoles, Role};
use animsmith_core::transform::{
    GAIT_ANCHOR_MAX_HORIZONTAL_ACCUMULATION_M, GAIT_ANCHOR_MAX_TRAJECTORY_POSE_SAMPLES,
    GAIT_ANCHOR_MAX_YAW_ACCUMULATION_DEG, GaitTrajectoryPolicy, align_gait_anchor, hold_extend,
    slice,
};
use glam::{Quat, Vec3};
use std::f64::consts::TAU;

/// Extract a Vec3 track's values, panicking otherwise (test helper).
fn vec3_values(track: &Track) -> Vec<Vec3> {
    match &track.values {
        TrackValues::Vec3s(v) => v.clone(),
        _ => panic!("expected a Vec3 track"),
    }
}

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

fn open_loop_foot_track_at_fps(bone: BoneId, rest: Vec3, sign: f32, fps: f64) -> Track {
    let times: Vec<f32> = (0..KEYS).map(|k| (k as f64 / fps) as f32).collect();
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
    open_walk_at_fps(FPS)
}

fn open_walk_at_fps(fps: f64) -> (Skeleton, Clip) {
    let skel = skeleton();
    let clip = Clip {
        name: "walk".into(),
        duration_s: (KEYS - 1) as f64 / fps,
        tracks: vec![
            open_loop_foot_track_at_fps(1, skel.bones[1].rest.translation, 1.0, fps),
            open_loop_foot_track_at_fps(2, skel.bones[2].rest.translation, -1.0, fps),
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
    let orig_track = &original.tracks[0];

    // The window [0.25, 0.75] with a half-frame epsilon at 32 fps keeps
    // exactly original keys 8..=24 (times 8/32 = 0.25 through 24/32 =
    // 0.75) — 17 keys — retimed so the first lands at 0 and the last at
    // the new 0.5 s duration. Both counts are analytic, not re-derived
    // from the epsilon rule the way the old oracle was.
    const FIRST: usize = 8; // 8/32 = 0.25
    const KEPT: usize = 17; // keys 8..=24 inclusive
    assert_eq!(
        track.key_count(),
        KEPT,
        "kept {} keys, want {KEPT}: {:?}",
        track.key_count(),
        track.times
    );
    assert_eq!(track.times[0], 0.0);
    assert!(
        (track.end_time() - 0.5).abs() < 1e-6,
        "end {}",
        track.end_time()
    );

    // Slice retimes; it never resamples — so every kept key carries its
    // original value verbatim across the WHOLE window (not just key 0),
    // at the fps-grid time (FIRST+i)/32 − 0.25.
    for i in 0..KEPT {
        assert_eq!(
            track.key_vec3(i),
            orig_track.key_vec3(FIRST + i),
            "key {i} value must equal original key {}",
            FIRST + i
        );
        let want_t = ((FIRST + i) as f32 / FPS as f32 - 0.25).clamp(0.0, 0.5);
        assert!(
            (track.times[i] - want_t).abs() < 1e-6,
            "key {i} time {} != {want_t}",
            track.times[i]
        );
    }
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

    let outcome = align_gait_anchor(&skel, &mut clip, &roles, FPS, GaitTrajectoryPolicy::InPlace)
        .expect("aligns");
    // The synthetic L−R foot-height signal is 2A·sin(2πk/32): its
    // fundamental trough sits at key 24 (a quarter cycle before the
    // wrap) — phase 0.75.
    assert!(
        (outcome.phase_before - 0.75).abs() < 0.05,
        "before: {}",
        outcome.phase_before
    );
    // `align_gait_anchor` may apply a ±1-frame wrap nudge: it tries
    // offsets 0/−1/+1 and keeps the cleanest seam. On this symmetric loop
    // the three seams tie, so *which* offset wins is an internal tie-break
    // (candidate order + comparison), not an observable contract — so the
    // test must not pin frame_offset to a specific value. Instead fold the
    // chosen nudge into the expected shift: the ANCHOR itself stays
    // analytically pinned (the trough of sin(2πk/32) is key 24), while the
    // nudge is read back as the one legitimate degree of freedom.
    assert!(
        outcome.frame_offset.abs() <= 1,
        "wrap nudge out of range: {}",
        outcome.frame_offset
    );
    const ANCHOR: i32 = 24; // trough of the L−R sine, analytic
    let shift = (ANCHOR + outcome.frame_offset).rem_euclid(KEYS as i32) as usize;
    // The anchor lands within one frame of its nudge-adjusted target. The
    // bound is deliberately below one frame (0.75/32 < 1/32 ≈ 0.031), so
    // an off-by-one *anchor* rounding cannot satisfy it — unlike the old
    // 0.06 bound (≈ two frames), which let an off-by-one rotation pass.
    let target_phase = ((-outcome.frame_offset) as f64 / KEYS as f64).rem_euclid(1.0);
    assert!(
        circular_delta(outcome.phase_after, target_phase) < 0.75 / KEYS as f64,
        "after: {} (target {target_phase}) — off-by-one anchor rounding?",
        outcome.phase_after
    );
    // Lossless: every rotated key equals the original key `shift` later
    // (the quantized shift lands exactly on an existing key), for EVERY
    // key — not all-but-one. The shift is pinned analytically (ANCHOR 24,
    // not read back from `phase_before`); an off-by-one anchor would shift
    // by 23 or 25 instead of 24 and fail here — the failure the previous
    // oracle (deriving its shift from the impl's own output) could not see.
    let rotated = &clip.tracks[0];
    let orig = &original.tracks[0];
    for k in 0..KEYS {
        let want = orig.key_vec3((k + shift) % KEYS).unwrap();
        let got = rotated.key_vec3(k).unwrap();
        assert!(
            (got - want).length() < 1e-6,
            "key {k}: rotated {got:?} != original key {} {want:?} — not a pure {shift}-frame rotation",
            (k + shift) % KEYS
        );
    }
}

#[test]
fn gait_anchor_permutes_authored_values_and_samples_authored_times_at_30_fps() {
    const NON_BINARY_FPS: f64 = 30.0;
    let (skel, mut clip) = open_walk_at_fps(NON_BINARY_FPS);
    let roles = roles(&skel);
    let original = clip.clone();

    let outcome = align_gait_anchor(
        &skel,
        &mut clip,
        &roles,
        NON_BINARY_FPS,
        GaitTrajectoryPolicy::InPlace,
    )
    .expect("30 fps authored grid remains lossless");
    let shift = ((outcome.phase_before * KEYS as f64).round() as i64
        + i64::from(outcome.frame_offset))
    .rem_euclid(KEYS as i64) as usize;
    for (track_index, (before, after)) in original.tracks.iter().zip(&clip.tracks).enumerate() {
        for key in 0..KEYS {
            assert_eq!(
                after.key_vec3(key),
                before.key_vec3((key + shift) % KEYS),
                "track {track_index} key {key} must be an exact authored-value permutation"
            );
        }
    }

    // A selected trajectory rotation with no horizontal forward direction at
    // one exact authored key must be observed at that key. Reconstructing the
    // nominal uniform time through duration*i/N can miss the exact f32 time.
    let (_, mut undefined_forward) = open_walk_at_fps(NON_BINARY_FPS);
    let mut rotations = vec![Quat::IDENTITY; KEYS];
    rotations[5] = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
    undefined_forward.tracks.push(Track {
        bone: 0,
        property: Property::Rotation,
        interpolation: Interpolation::Linear,
        times: (0..KEYS)
            .map(|key| (key as f64 / NON_BINARY_FPS) as f32)
            .collect(),
        values: TrackValues::Quats(rotations),
    });
    let error = align_gait_anchor(
        &skel,
        &mut undefined_forward,
        &roles,
        NON_BINARY_FPS,
        GaitTrajectoryPolicy::InPlace,
    )
    .unwrap_err();
    assert!(
        error.contains(
            "no finite horizontal projection for its selected local +Z heading basis at sample 5"
        ),
        "got: {error}"
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
    let err = align_gait_anchor(&skel, &mut clip, &roles, FPS, GaitTrajectoryPolicy::InPlace)
        .unwrap_err();
    assert!(err.contains("stride anchor"), "got: {err}");
}

fn root_translation(values: impl IntoIterator<Item = Vec3>) -> Track {
    Track {
        bone: 0,
        property: Property::Translation,
        interpolation: Interpolation::Linear,
        times: (0..KEYS).map(|key| key as f32 / FPS as f32).collect(),
        values: TrackValues::Vec3s(values.into_iter().collect()),
    }
}

fn root_yaw(values: impl IntoIterator<Item = f32>) -> Track {
    Track {
        bone: 0,
        property: Property::Rotation,
        interpolation: Interpolation::Linear,
        times: (0..KEYS).map(|key| key as f32 / FPS as f32).collect(),
        values: TrackValues::Quats(values.into_iter().map(Quat::from_rotation_y).collect()),
    }
}

fn vertical_local_forward_basis() -> Quat {
    Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)
}

fn vertical_basis_root_yaw(values: impl IntoIterator<Item = f32>) -> Track {
    let basis = vertical_local_forward_basis();
    Track {
        bone: 0,
        property: Property::Rotation,
        interpolation: Interpolation::Linear,
        times: (0..KEYS).map(|key| key as f32 / FPS as f32).collect(),
        values: TrackValues::Quats(
            values
                .into_iter()
                .map(|yaw| Quat::from_rotation_y(yaw) * basis)
                .collect(),
        ),
    }
}

#[test]
fn gait_anchor_accepts_in_place_motion_when_local_z_is_vertical() {
    let (mut skel, mut clip) = open_walk();
    skel.bones[0].rest.rotation = vertical_local_forward_basis();
    let roles = roles(&skel);

    align_gait_anchor(&skel, &mut clip, &roles, FPS, GaitTrajectoryPolicy::InPlace)
        .expect("a fixed horizontal alternative-axis witness admits the vertical-+Z source basis");
}

#[test]
fn gait_anchor_vertical_z_tie_selects_y_once_and_never_falls_back_to_x() {
    let (skel, mut clip) = open_walk();
    // This exact quaternion maps local +Z vertically and gives local +Y and
    // +X exactly equal horizontal projections. Policy must choose +Y.
    let basis = Quat::from_xyzw(-0.5, -0.5, -0.5, 0.5);
    let mut rotations = vec![basis; KEYS];
    rotations[5] = Quat::IDENTITY;
    clip.tracks.push(Track {
        bone: 0,
        property: Property::Rotation,
        interpolation: Interpolation::Linear,
        times: (0..KEYS).map(|key| key as f32 / FPS as f32).collect(),
        values: TrackValues::Quats(rotations),
    });
    let roles = roles(&skel);
    let before = format!("{clip:?}");

    let error = align_gait_anchor(&skel, &mut clip, &roles, FPS, GaitTrajectoryPolicy::InPlace)
        .unwrap_err();

    assert!(
        error.contains(
            "no finite horizontal projection for its selected local +Y heading basis at sample 5"
        ),
        "got: {error}"
    );
    assert_eq!(format!("{clip:?}"), before);
}

#[test]
fn gait_anchor_vertical_basis_still_refuses_translation_and_yaw() {
    let (mut skel, base) = open_walk();
    skel.bones[0].rest.rotation = vertical_local_forward_basis();
    let roles = roles(&skel);

    let mut translation = base.clone();
    translation.tracks.push(root_translation(
        (0..KEYS).map(|key| Vec3::new(key as f32 * 0.1, 1.0, 0.0)),
    ));
    let translation_before = format!("{translation:?}");
    let translation_error = align_gait_anchor(
        &skel,
        &mut translation,
        &roles,
        FPS,
        GaitTrajectoryPolicy::InPlace,
    )
    .unwrap_err();
    assert_gait_refusal_is_located_and_atomic(
        &translation,
        &translation_before,
        &translation_error,
    );
    assert!(
        translation_error.contains("horizontal translation 3.1000 m"),
        "got: {translation_error}"
    );

    let mut yaw = base.clone();
    yaw.tracks
        .push(vertical_basis_root_yaw((0..KEYS).map(|key| {
            key as f32 * std::f32::consts::FRAC_PI_2 / (KEYS - 1) as f32
        })));
    let yaw_before = format!("{yaw:?}");
    let yaw_error =
        align_gait_anchor(&skel, &mut yaw, &roles, FPS, GaitTrajectoryPolicy::InPlace).unwrap_err();
    assert_gait_refusal_is_located_and_atomic(&yaw, &yaw_before, &yaw_error);
    assert!(yaw_error.contains("yaw 90.000 deg"), "got: {yaw_error}");

    let mut mixed = base;
    mixed.tracks.push(root_translation(
        (0..KEYS).map(|key| Vec3::new(0.0, 1.0, key as f32 * 0.05)),
    ));
    mixed.tracks.push(vertical_basis_root_yaw(
        (0..KEYS).map(|key| key as f32 * std::f32::consts::FRAC_PI_2 / (KEYS - 1) as f32),
    ));
    let mixed_before = format!("{mixed:?}");
    let mixed_error = align_gait_anchor(
        &skel,
        &mut mixed,
        &roles,
        FPS,
        GaitTrajectoryPolicy::InPlace,
    )
    .unwrap_err();
    assert_gait_refusal_is_located_and_atomic(&mixed, &mixed_before, &mixed_error);
    assert!(mixed_error.contains("1.5500 m"), "got: {mixed_error}");
    assert!(mixed_error.contains("yaw 90.000 deg"), "got: {mixed_error}");
}

fn assert_gait_refusal_is_located_and_atomic(clip: &Clip, before: &str, error: &str) {
    assert!(
        error.contains(&format!("clip {:?}", clip.name)),
        "got: {error}"
    );
    assert!(error.contains("Hips fallback"), "got: {error}");
    assert!(error.contains("pelvis"), "got: {error}");
    assert!(error.contains("horizontal translation"), "got: {error}");
    assert!(error.contains("yaw"), "got: {error}");
    assert!(error.contains("cap 0.0100 m"), "got: {error}");
    assert!(error.contains("cap 1.000 deg"), "got: {error}");
    assert!(error.contains("retain source root motion"), "got: {error}");
    assert!(error.contains("runtime phase offsets"), "got: {error}");
    assert!(
        error.contains("trajectory-preserving operation"),
        "got: {error}"
    );
    assert_eq!(
        format!("{clip:?}"),
        before,
        "refusal must not rewrite the clip"
    );
}

#[test]
fn gait_anchor_refuses_accumulating_root_translation() {
    let (skel, mut clip) = open_walk();
    let roles = roles(&skel);
    clip.name = "walk_root_motion".into();
    clip.tracks.push(root_translation(
        (0..KEYS).map(|key| Vec3::new(key as f32 * 0.1, 1.0, 0.0)),
    ));
    let before = format!("{clip:?}");

    let error = align_gait_anchor(&skel, &mut clip, &roles, FPS, GaitTrajectoryPolicy::InPlace)
        .unwrap_err();

    assert_gait_refusal_is_located_and_atomic(&clip, &before, &error);
    assert!(error.contains("3.1000 m"), "got: {error}");
    assert!(error.contains("yaw 0.000 deg"), "got: {error}");
}

#[test]
fn gait_anchor_refuses_accumulating_root_yaw_without_horizontal_speed() {
    let (skel, mut clip) = open_walk();
    let roles = roles(&skel);
    clip.name = "turn_root_motion".into();
    clip.tracks.push(root_yaw(
        (0..KEYS).map(|key| key as f32 * std::f32::consts::FRAC_PI_2 / (KEYS - 1) as f32),
    ));
    let before = format!("{clip:?}");

    let error = align_gait_anchor(&skel, &mut clip, &roles, FPS, GaitTrajectoryPolicy::InPlace)
        .unwrap_err();

    assert_gait_refusal_is_located_and_atomic(&clip, &before, &error);
    assert!(error.contains("translation 0.0000 m"), "got: {error}");
    assert!(error.contains("yaw 90.000 deg"), "got: {error}");
}

#[test]
fn gait_anchor_refuses_a_full_turn_even_when_endpoint_heading_aliases_start() {
    const WINDING_FRAMES: usize = 5;
    const WINDING_FPS: f64 = 1.0;
    let (skel, mut clip) = open_walk();
    let roles = roles(&skel);
    clip.name = "full_turn_root_motion".into();
    clip.duration_s = (WINDING_FRAMES - 1) as f64;
    for track in &mut clip.tracks {
        track.times = (0..WINDING_FRAMES).map(|key| key as f32).collect();
        let TrackValues::Vec3s(values) = &mut track.values else {
            unreachable!()
        };
        values.truncate(WINDING_FRAMES);
    }
    clip.tracks.push(Track {
        bone: 0,
        property: Property::Rotation,
        interpolation: Interpolation::Linear,
        times: (0..WINDING_FRAMES).map(|key| key as f32).collect(),
        values: TrackValues::Quats(
            (0..WINDING_FRAMES)
                .map(|key| Quat::from_rotation_y(key as f32 * std::f32::consts::FRAC_PI_2))
                .collect(),
        ),
    });
    let before = format!("{clip:?}");

    let error = align_gait_anchor(
        &skel,
        &mut clip,
        &roles,
        WINDING_FPS,
        GaitTrajectoryPolicy::InPlace,
    )
    .unwrap_err();

    assert_gait_refusal_is_located_and_atomic(&clip, &before, &error);
    assert!(error.contains("translation 0.0000 m"), "got: {error}");
    assert!(error.contains("yaw 360.000 deg"), "got: {error}");
}

#[test]
fn gait_anchor_refuses_mixed_root_translation_and_yaw() {
    let (skel, mut clip) = open_walk();
    let roles = roles(&skel);
    clip.name = "arc_root_motion".into();
    clip.tracks.push(root_translation(
        (0..KEYS).map(|key| Vec3::new(0.0, 1.0, key as f32 * 0.05)),
    ));
    clip.tracks.push(root_yaw(
        (0..KEYS).map(|key| key as f32 * std::f32::consts::FRAC_PI_2 / (KEYS - 1) as f32),
    ));
    let before = format!("{clip:?}");

    let error = align_gait_anchor(&skel, &mut clip, &roles, FPS, GaitTrajectoryPolicy::InPlace)
        .unwrap_err();

    assert_gait_refusal_is_located_and_atomic(&clip, &before, &error);
    assert!(error.contains("1.5500 m"), "got: {error}");
    assert!(error.contains("yaw 90.000 deg"), "got: {error}");
}

#[test]
fn gait_anchor_refuses_abrupt_boundary_translation_and_yaw() {
    let (skel, mut clip) = open_walk();
    let roles = roles(&skel);
    clip.name = "boundary_jump".into();
    clip.tracks.push(root_translation((0..KEYS).map(|key| {
        if key == 0 {
            Vec3::ZERO
        } else {
            Vec3::new(3.0, 1.0, 0.0)
        }
    })));
    clip.tracks.push(root_yaw(
        (0..KEYS).map(|key| if key == 0 { 0.0 } else { std::f32::consts::PI }),
    ));
    let before = format!("{clip:?}");

    let error = align_gait_anchor(&skel, &mut clip, &roles, FPS, GaitTrajectoryPolicy::InPlace)
        .unwrap_err();

    assert_gait_refusal_is_located_and_atomic(&clip, &before, &error);
    assert!(error.contains("translation 3.0000 m"), "got: {error}");
    assert!(error.contains("yaw 180.000 deg"), "got: {error}");
}

#[test]
fn gait_anchor_refuses_abrupt_final_boundary_translation_and_yaw() {
    let (skel, mut clip) = open_walk();
    let roles = roles(&skel);
    clip.name = "final_boundary_jump".into();
    clip.tracks.push(root_translation((0..KEYS).map(|key| {
        if key + 1 == KEYS {
            Vec3::new(3.0, 1.0, 0.0)
        } else {
            Vec3::new(0.0, 1.0, 0.0)
        }
    })));
    clip.tracks.push(root_yaw((0..KEYS).map(|key| {
        if key + 1 == KEYS {
            std::f32::consts::PI
        } else {
            0.0
        }
    })));
    let before = format!("{clip:?}");

    let error = align_gait_anchor(&skel, &mut clip, &roles, FPS, GaitTrajectoryPolicy::InPlace)
        .unwrap_err();

    assert_gait_refusal_is_located_and_atomic(&clip, &before, &error);
    assert!(error.contains("translation 3.0000 m"), "got: {error}");
    assert!(error.contains("yaw 180.000 deg"), "got: {error}");
}

#[test]
fn gait_anchor_accepts_cyclic_root_translation_and_yaw() {
    let (skel, mut clip) = open_walk();
    let roles = roles(&skel);
    clip.tracks.push(root_translation((0..KEYS).map(|key| {
        let theta = std::f32::consts::TAU * key as f32 / KEYS as f32;
        Vec3::new(0.05 * theta.sin(), 1.0, 0.05 * theta.cos())
    })));
    clip.tracks.push(root_yaw((0..KEYS).map(|key| {
        let theta = std::f32::consts::TAU * key as f32 / KEYS as f32;
        0.08 * theta.sin()
    })));

    align_gait_anchor(&skel, &mut clip, &roles, FPS, GaitTrajectoryPolicy::InPlace)
        .expect("cyclic root/pelvis motion is safe to rotate");
}

fn linear_root_translation_with_accumulation(accumulation_m: f64) -> Track {
    let step = accumulation_m as f32 / (KEYS - 1) as f32;
    root_translation((0..KEYS).map(|key| Vec3::new(step * key as f32, 1.0, 0.0)))
}

fn linear_root_yaw_with_accumulation(accumulation_deg: f64) -> Track {
    let step_rad = (accumulation_deg as f32 / (KEYS - 1) as f32).to_radians();
    root_yaw((0..KEYS).map(|key| step_rad * key as f32))
}

#[test]
fn gait_anchor_policy_pins_translation_and_yaw_caps() {
    let (skel, base) = open_walk();
    let roles = roles(&skel);

    let mut inside = base.clone();
    inside
        .tracks
        .push(linear_root_translation_with_accumulation(
            GAIT_ANCHOR_MAX_HORIZONTAL_ACCUMULATION_M * 0.99,
        ));
    inside.tracks.push(linear_root_yaw_with_accumulation(
        GAIT_ANCHOR_MAX_YAW_ACCUMULATION_DEG * 0.99,
    ));
    align_gait_anchor(
        &skel,
        &mut inside,
        &roles,
        FPS,
        GaitTrajectoryPolicy::InPlace,
    )
    .expect("facts immediately inside both fixed caps are admitted");

    let mut at_cap = base.clone();
    at_cap.tracks.push(root_translation((0..KEYS).map(|key| {
        Vec3::new(
            if key + 1 == KEYS {
                GAIT_ANCHOR_MAX_HORIZONTAL_ACCUMULATION_M as f32
            } else {
                0.0
            },
            1.0,
            0.0,
        )
    })));
    at_cap.tracks.push(root_yaw((0..KEYS).map(|key| {
        if key + 1 == KEYS {
            (GAIT_ANCHOR_MAX_YAW_ACCUMULATION_DEG as f32).to_radians()
        } else {
            0.0
        }
    })));
    align_gait_anchor(
        &skel,
        &mut at_cap,
        &roles,
        FPS,
        GaitTrajectoryPolicy::InPlace,
    )
    .expect("facts exactly at both inclusive fixed caps are admitted");

    let mut translation_outside = base.clone();
    translation_outside
        .tracks
        .push(linear_root_translation_with_accumulation(
            GAIT_ANCHOR_MAX_HORIZONTAL_ACCUMULATION_M * 1.01,
        ));
    let translation_error = align_gait_anchor(
        &skel,
        &mut translation_outside,
        &roles,
        FPS,
        GaitTrajectoryPolicy::InPlace,
    )
    .unwrap_err();
    assert!(
        translation_error.contains("horizontal translation 0.0101 m"),
        "got: {translation_error}"
    );
    assert!(
        translation_error.contains("cap 0.0100 m"),
        "got: {translation_error}"
    );
    assert!(
        translation_error.contains("cap 1.000 deg"),
        "got: {translation_error}"
    );

    let mut yaw_outside = base;
    yaw_outside.tracks.push(linear_root_yaw_with_accumulation(
        GAIT_ANCHOR_MAX_YAW_ACCUMULATION_DEG * 1.000_01,
    ));
    let yaw_error = align_gait_anchor(
        &skel,
        &mut yaw_outside,
        &roles,
        FPS,
        GaitTrajectoryPolicy::InPlace,
    )
    .unwrap_err();
    assert!(yaw_error.contains("and yaw "), "got: {yaw_error}");
    assert!(yaw_error.contains("cap 0.0100 m"), "got: {yaw_error}");
    assert!(yaw_error.contains("cap 1.000 deg"), "got: {yaw_error}");
}

fn long_walk_with_distributed_yaw(accumulation_deg: f64) -> (Skeleton, Clip) {
    const LONG_FRAMES: usize = 100_001;
    const LONG_FPS: f64 = 100_000.0;
    let skel = skeleton();
    let times: Vec<f32> = (0..LONG_FRAMES)
        .map(|key| (key as f64 / LONG_FPS) as f32)
        .collect();
    let rest_translations: Vec<Vec3> = skel
        .bones
        .iter()
        .map(|bone| bone.rest.translation)
        .collect();
    let foot = |bone: usize, sign: f32| Track {
        bone,
        property: Property::Translation,
        interpolation: Interpolation::Linear,
        times: times.clone(),
        values: TrackValues::Vec3s(
            (0..LONG_FRAMES)
                .map(|key| {
                    let theta = (TAU * key as f64 / LONG_FRAMES as f64) as f32;
                    rest_translations[bone]
                        + Vec3::new(0.0, sign * 0.05 * theta.sin(), sign * 0.15 * theta.sin())
                })
                .collect(),
        ),
    };
    let yaw = Track {
        bone: 0,
        property: Property::Rotation,
        interpolation: Interpolation::Linear,
        times: times.clone(),
        values: TrackValues::Quats(
            (0..LONG_FRAMES)
                .map(|key| {
                    let degrees = accumulation_deg * key as f64 / (LONG_FRAMES - 1) as f64;
                    Quat::from_rotation_y((degrees as f32).to_radians())
                })
                .collect(),
        ),
    };
    (
        skel,
        Clip {
            name: "long_distributed_yaw".into(),
            duration_s: (LONG_FRAMES - 1) as f64 / LONG_FPS,
            tracks: vec![foot(1, 1.0), foot(2, -1.0), yaw],
        },
    )
}

#[test]
fn gait_anchor_long_grid_yaw_cap_is_stable_and_minimally_above_refuses() {
    const LONG_FPS: f64 = 100_000.0;
    let (skel, mut at_cap) = long_walk_with_distributed_yaw(GAIT_ANCHOR_MAX_YAW_ACCUMULATION_DEG);
    let resolved_roles = roles(&skel);
    align_gait_anchor(
        &skel,
        &mut at_cap,
        &resolved_roles,
        LONG_FPS,
        GaitTrajectoryPolicy::InPlace,
    )
    .expect("100,000 distributed yaw steps at the exact inclusive cap are admitted");

    let (skel, mut above) =
        long_walk_with_distributed_yaw(GAIT_ANCHOR_MAX_YAW_ACCUMULATION_DEG * 1.000_01);
    let resolved_roles = roles(&skel);
    let last_before = above.tracks[2].key_quat(100_000);
    let error = align_gait_anchor(
        &skel,
        &mut above,
        &resolved_roles,
        LONG_FPS,
        GaitTrajectoryPolicy::InPlace,
    )
    .unwrap_err();
    assert!(error.contains("and yaw "), "got: {error}");
    assert!(error.contains("cap 1.000 deg"), "got: {error}");
    assert_eq!(above.tracks[2].key_quat(100_000), last_before);
}

#[test]
fn gait_anchor_fails_closed_without_root_or_hips_evidence() {
    let (skel, mut clip) = open_walk();
    let roles = ResolvedRoles::from_names(
        &skel,
        [
            (Role::LeftFoot, "l_foot".to_string()),
            (Role::RightFoot, "r_foot".to_string()),
        ],
    );
    let before = format!("{clip:?}");

    let error = align_gait_anchor(&skel, &mut clip, &roles, FPS, GaitTrajectoryPolicy::InPlace)
        .unwrap_err();

    assert!(error.contains("walk"), "got: {error}");
    assert!(error.contains("evidence is missing"), "got: {error}");
    assert!(error.contains("retain source root motion"), "got: {error}");
    assert_eq!(format!("{clip:?}"), before);
}

#[test]
fn gait_anchor_fails_closed_on_out_of_range_selected_role() {
    let (mut skel, mut clip) = open_walk();
    let roles = roles(&skel);
    skel.bones.clear();
    let before = format!("{clip:?}");

    let error = align_gait_anchor(&skel, &mut clip, &roles, FPS, GaitTrajectoryPolicy::InPlace)
        .unwrap_err();

    assert!(error.contains("Hips fallback"), "got: {error}");
    assert!(
        error.contains("index 0 is outside the skeleton"),
        "got: {error}"
    );
    assert!(error.contains("evidence is missing"), "got: {error}");
    assert_eq!(format!("{clip:?}"), before);
}

#[test]
fn gait_anchor_fails_closed_on_out_of_range_ancestor() {
    let (mut skel, mut clip) = open_walk();
    skel.bones[0].parent = Some(99);
    let roles = roles(&skel);
    let before = format!("{clip:?}");

    let error = align_gait_anchor(&skel, &mut clip, &roles, FPS, GaitTrajectoryPolicy::InPlace)
        .unwrap_err();

    assert!(error.contains("pelvis"), "got: {error}");
    assert!(
        error.contains("out-of-range ancestor index 99"),
        "got: {error}"
    );
    assert!(error.contains("evidence is missing"), "got: {error}");
    assert_eq!(format!("{clip:?}"), before);
}

#[test]
fn gait_anchor_fails_closed_on_cyclic_ancestor_chain() {
    let (mut skel, mut clip) = open_walk();
    skel.bones[0].parent = Some(0);
    let roles = roles(&skel);
    let before = format!("{clip:?}");

    let error = align_gait_anchor(&skel, &mut clip, &roles, FPS, GaitTrajectoryPolicy::InPlace)
        .unwrap_err();

    assert!(error.contains("pelvis"), "got: {error}");
    assert!(error.contains("cyclic ancestor chain"), "got: {error}");
    assert!(error.contains("evidence is missing"), "got: {error}");
    assert_eq!(format!("{clip:?}"), before);
}

#[test]
fn gait_anchor_fails_closed_on_non_finite_root_evidence() {
    let (skel, mut clip) = open_walk();
    let roles = roles(&skel);
    clip.tracks.push(root_translation((0..KEYS).map(|key| {
        if key == KEYS / 2 {
            Vec3::new(f32::NAN, 1.0, 0.0)
        } else {
            Vec3::new(0.0, 1.0, 0.0)
        }
    })));
    let before = format!("{clip:?}");

    let error = align_gait_anchor(&skel, &mut clip, &roles, FPS, GaitTrajectoryPolicy::InPlace)
        .unwrap_err();

    assert!(error.contains("walk"), "got: {error}");
    assert!(
        error.contains("non-finite authored trajectory evidence"),
        "got: {error}"
    );
    assert_eq!(format!("{clip:?}"), before);
}

#[test]
fn gait_anchor_fails_closed_on_non_finite_authored_interval_between_samples() {
    let (skel, mut clip) = open_walk();
    let roles = roles(&skel);
    clip.name = "hidden_nan".into();
    clip.tracks.push(Track {
        bone: 0,
        property: Property::Translation,
        interpolation: Interpolation::Step,
        times: vec![0.0, 0.0001, 0.0002, clip.duration_s as f32],
        values: TrackValues::Vec3s(vec![
            Vec3::ZERO,
            Vec3::splat(f32::NAN),
            Vec3::ZERO,
            Vec3::ZERO,
        ]),
    });
    let before = format!("{clip:?}");

    let error = align_gait_anchor(&skel, &mut clip, &roles, FPS, GaitTrajectoryPolicy::InPlace)
        .unwrap_err();

    assert!(
        error.contains("non-finite authored trajectory evidence in track 2"),
        "got: {error}"
    );
    assert!(error.contains("hidden_nan"), "got: {error}");
    assert!(error.contains("retain source root motion"), "got: {error}");
    assert_eq!(format!("{clip:?}"), before);
}

#[test]
fn gait_anchor_fails_closed_on_finite_step_turn_between_uniform_samples() {
    let (skel, mut clip) = open_walk();
    let roles = roles(&skel);
    clip.name = "hidden_full_turn".into();
    let mut times: Vec<f32> = (0..KEYS).map(|key| key as f32 / FPS as f32).collect();
    times[1..5].copy_from_slice(&[0.001, 0.002, 0.003, 0.004]);
    let mut values = vec![Quat::IDENTITY; KEYS];
    for (slot, degrees) in values[..5]
        .iter_mut()
        .zip([0.0f32, 90.0, 180.0, 270.0, 360.0])
    {
        *slot = Quat::from_rotation_y(degrees.to_radians());
    }
    values[5..].fill(Quat::from_rotation_y(std::f32::consts::TAU));
    clip.tracks.push(Track {
        bone: 0,
        property: Property::Rotation,
        interpolation: Interpolation::Step,
        times,
        values: TrackValues::Quats(values),
    });
    let before = format!("{clip:?}");

    let error = align_gait_anchor(&skel, &mut clip, &roles, FPS, GaitTrajectoryPolicy::InPlace)
        .unwrap_err();

    assert!(error.contains("hidden_full_turn"), "got: {error}");
    assert!(error.contains("Hips fallback"), "got: {error}");
    assert!(error.contains("pelvis"), "got: {error}");
    assert!(
        error.contains("duplicate/non-frame-aligned whole-frame trajectory evidence"),
        "got: {error}"
    );
    assert!(error.contains("track 2, key 1"), "got: {error}");
    assert!(error.contains("retain source root motion"), "got: {error}");
    assert_eq!(format!("{clip:?}"), before);
}

#[test]
fn gait_anchor_fails_closed_on_non_frame_aligned_trajectory_key() {
    let (skel, mut clip) = open_walk();
    let roles = roles(&skel);
    let mut yaw = root_yaw((0..KEYS).map(|key| {
        let theta = std::f32::consts::TAU * key as f32 / KEYS as f32;
        0.1 * theta.sin()
    }));
    yaw.times[5] += 0.001;
    clip.tracks.push(yaw);
    let before = format!("{clip:?}");

    let error = align_gait_anchor(&skel, &mut clip, &roles, FPS, GaitTrajectoryPolicy::InPlace)
        .unwrap_err();

    assert!(
        error.contains("duplicate/non-frame-aligned whole-frame trajectory evidence"),
        "got: {error}"
    );
    assert!(error.contains("track 2, key 5"), "got: {error}");
    assert!(error.contains("32 fps"), "got: {error}");
    assert_eq!(format!("{clip:?}"), before);
}

#[test]
fn gait_anchor_fails_closed_on_duplicate_declared_frame_key() {
    let (skel, mut clip) = open_walk();
    let roles = roles(&skel);
    let mut yaw = root_yaw((0..KEYS).map(|key| {
        let theta = std::f32::consts::TAU * key as f32 / KEYS as f32;
        0.1 * theta.sin()
    }));
    yaw.times[5] = yaw.times[4];
    clip.tracks.push(yaw);
    let before = format!("{clip:?}");

    let error = align_gait_anchor(&skel, &mut clip, &roles, FPS, GaitTrajectoryPolicy::InPlace)
        .unwrap_err();

    assert!(
        error.contains("duplicate/non-frame-aligned whole-frame trajectory evidence"),
        "got: {error}"
    );
    assert!(error.contains("track 2, key 5"), "got: {error}");
    assert_eq!(format!("{clip:?}"), before);
}

#[test]
fn gait_anchor_fails_closed_on_sparse_frame_aligned_selected_trajectory() {
    let (skel, mut clip) = open_walk();
    let roles = roles(&skel);
    // This in-place bump is keyed on declared frames but omits the frames a
    // gait phase shift can target. Resampling it at the unchanged three key
    // times would synthesize new values instead of permuting authored keys.
    clip.tracks.push(Track {
        bone: 0,
        property: Property::Translation,
        interpolation: Interpolation::Linear,
        times: vec![0.0, 1.0 / FPS as f32, clip.duration_s as f32],
        values: TrackValues::Vec3s(vec![
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.005, 1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        ]),
    });
    let before = format!("{clip:?}");

    let error = align_gait_anchor(&skel, &mut clip, &roles, FPS, GaitTrajectoryPolicy::InPlace)
        .unwrap_err();

    assert!(
        error.contains("incomplete whole-frame rotation evidence in track 2"),
        "got: {error}"
    );
    assert!(
        error.contains("3 keys instead of exactly 32"),
        "got: {error}"
    );
    assert!(error.contains("retain source root motion"), "got: {error}");
    assert_eq!(format!("{clip:?}"), before);
}

#[test]
fn gait_anchor_fails_closed_before_oversized_trajectory_grid_allocation() {
    let (skel, mut clip) = open_walk();
    let roles = roles(&skel);
    let frames = GAIT_ANCHOR_MAX_TRAJECTORY_POSE_SAMPLES / skel.bones.len() + 1;
    clip.name = "oversized_grid".into();
    clip.duration_s = (frames - 1) as f64;
    let before = format!("{clip:?}");

    let error = align_gait_anchor(&skel, &mut clip, &roles, 1.0, GaitTrajectoryPolicy::InPlace)
        .unwrap_err();

    assert!(error.contains("oversized_grid"), "got: {error}");
    assert!(error.contains("trajectory pose samples"), "got: {error}");
    assert!(error.contains("333334 frames x 3 bones"), "got: {error}");
    assert!(
        error.contains("1000000 sample safety budget"),
        "got: {error}"
    );
    assert_eq!(format!("{clip:?}"), before);
}

fn extend_skeleton_to(skeleton: &mut Skeleton, count: usize) {
    while skeleton.bones.len() < count {
        skeleton.bones.push(Bone {
            name: format!("unused_{}", skeleton.bones.len()),
            parent: None,
            rest: Transform::IDENTITY,
            inverse_bind: None,
        });
    }
}

#[test]
fn gait_anchor_bounds_unrelated_authored_key_work_before_sampling() {
    let (mut skel, mut clip) = open_walk();
    extend_skeleton_to(&mut skel, 100);
    let roles = roles(&skel);
    clip.name = "unrelated_key_bomb".into();
    clip.tracks.push(Track {
        bone: 99,
        property: Property::Scale,
        interpolation: Interpolation::Linear,
        times: (0..20_001).map(|key| key as f32).collect(),
        values: TrackValues::Vec3s(vec![Vec3::ONE; 20_001]),
    });
    let before = format!("{clip:?}");

    let error = align_gait_anchor(&skel, &mut clip, &roles, FPS, GaitTrajectoryPolicy::InPlace)
        .unwrap_err();
    assert!(error.contains("2000100 pose samples"), "got: {error}");
    assert!(error.contains("sample safety budget"), "got: {error}");
    assert_eq!(format!("{clip:?}"), before);
}

#[test]
fn gait_anchor_rejects_duplicate_channels_before_sampling_or_mutation() {
    let (skel, mut clip) = open_walk();
    let roles = roles(&skel);
    for _ in 0..2_001 {
        clip.tracks.push(Track {
            bone: 0,
            property: Property::Scale,
            interpolation: Interpolation::Linear,
            times: vec![0.0],
            values: TrackValues::Vec3s(vec![Vec3::ONE]),
        });
    }
    let track_count = clip.tracks.len();
    let first = clip.tracks[2].key_vec3(0);
    let last = clip.tracks.last().and_then(|track| track.key_vec3(0));

    let error = align_gait_anchor(&skel, &mut clip, &roles, FPS, GaitTrajectoryPolicy::InPlace)
        .unwrap_err();
    assert!(
        error.contains("track 3 duplicates the scale channel"),
        "got: {error}"
    );
    assert_eq!(clip.tracks.len(), track_count);
    assert_eq!(clip.tracks[2].key_vec3(0), first);
    assert_eq!(clip.tracks.last().and_then(|track| track.key_vec3(0)), last);
}

#[test]
fn gait_anchor_bounds_frame_by_track_sampling_work() {
    let (mut skel, mut clip) = open_walk();
    extend_skeleton_to(&mut skel, 10_500);
    let roles = roles(&skel);
    for bone in 0..skel.bones.len() {
        for property in [Property::Rotation, Property::Scale] {
            let values = if property == Property::Rotation {
                TrackValues::Quats(vec![Quat::IDENTITY])
            } else {
                TrackValues::Vec3s(vec![Vec3::ONE])
            };
            clip.tracks.push(Track {
                bone,
                property,
                interpolation: Interpolation::Linear,
                times: vec![0.0],
                values,
            });
        }
        if bone != 1 && bone != 2 {
            clip.tracks.push(Track {
                bone,
                property: Property::Translation,
                interpolation: Interpolation::Linear,
                times: vec![0.0],
                values: TrackValues::Vec3s(vec![skel.bones[bone].rest.translation]),
            });
        }
    }
    let track_count = clip.tracks.len();

    let error = align_gait_anchor(&skel, &mut clip, &roles, FPS, GaitTrajectoryPolicy::InPlace)
        .unwrap_err();
    assert!(error.contains("1008000 channel samples"), "got: {error}");
    assert!(error.contains("32 frames x 31500 tracks"), "got: {error}");
    assert_eq!(clip.tracks.len(), track_count);
}

#[test]
fn gait_anchor_validates_the_whole_skeleton_and_every_metric_role() {
    let (mut child_first, mut clip) = open_walk();
    child_first.bones[0].parent = Some(2);
    let child_roles = roles(&child_first);
    let before = format!("{clip:?}");
    let error = align_gait_anchor(
        &child_first,
        &mut clip,
        &child_roles,
        FPS,
        GaitTrajectoryPolicy::InPlace,
    )
    .unwrap_err();
    assert!(error.contains("parents-before-children"), "got: {error}");
    assert_eq!(format!("{clip:?}"), before);

    let (mut unrelated_bad, mut clip) = open_walk();
    unrelated_bad.bones.push(Bone {
        name: "unrelated".into(),
        parent: Some(99),
        rest: Transform::IDENTITY,
        inverse_bind: None,
    });
    let unrelated_roles = roles(&unrelated_bad);
    let error = align_gait_anchor(
        &unrelated_bad,
        &mut clip,
        &unrelated_roles,
        FPS,
        GaitTrajectoryPolicy::InPlace,
    )
    .unwrap_err();
    assert!(error.contains("unrelated"), "got: {error}");
    assert!(
        error.contains("out-of-range ancestor index 99"),
        "got: {error}"
    );

    let (mut mismatched_role, mut clip) = open_walk();
    let mismatched_roles = roles(&mismatched_role);
    mismatched_role.bones.pop();
    let error = align_gait_anchor(
        &mismatched_role,
        &mut clip,
        &mismatched_roles,
        FPS,
        GaitTrajectoryPolicy::InPlace,
    )
    .unwrap_err();
    assert!(error.contains("right_foot bone index 2"), "got: {error}");
}

#[test]
fn gait_anchor_requires_a_dense_grid_for_unrelated_motion_and_exact_duration() {
    let (mut skel, mut sparse) = open_walk();
    extend_skeleton_to(&mut skel, 4);
    let sparse_roles = roles(&skel);
    sparse.tracks.push(Track {
        bone: 3,
        property: Property::Scale,
        interpolation: Interpolation::Linear,
        times: vec![0.0, sparse.duration_s as f32],
        values: TrackValues::Vec3s(vec![Vec3::ONE, Vec3::splat(2.0)]),
    });
    let before = format!("{sparse:?}");
    let error = align_gait_anchor(
        &skel,
        &mut sparse,
        &sparse_roles,
        FPS,
        GaitTrajectoryPolicy::InPlace,
    )
    .unwrap_err();
    assert!(error.contains("track 2"), "got: {error}");
    assert!(
        error.contains("2 keys instead of exactly 32"),
        "got: {error}"
    );
    assert_eq!(format!("{sparse:?}"), before);

    let (skel, mut off_frame) = open_walk();
    let off_frame_roles = roles(&skel);
    let duration = off_frame.duration_s as f32;
    off_frame.duration_s = f64::from(f32::from_bits(duration.to_bits() + 1));
    let error = align_gait_anchor(
        &skel,
        &mut off_frame,
        &off_frame_roles,
        FPS,
        GaitTrajectoryPolicy::InPlace,
    )
    .unwrap_err();
    assert!(error.contains("key 31"), "got: {error}");
    assert!(error.contains("required frame time"), "got: {error}");
}

#[test]
fn gait_anchor_rejects_vec3_quaternion_and_cubic_cardinality_mismatches() {
    let (skel, base) = open_walk();
    let roles = roles(&skel);
    for (label, values, interpolation) in [
        (
            "vec3",
            TrackValues::Vec3s(vec![Vec3::ZERO; KEYS - 1]),
            Interpolation::Linear,
        ),
        (
            "quaternion",
            TrackValues::Quats(vec![Quat::IDENTITY; KEYS - 1]),
            Interpolation::Linear,
        ),
        (
            "cubic",
            TrackValues::Vec3s(vec![Vec3::ZERO; KEYS * 3 - 1]),
            Interpolation::CubicSpline,
        ),
    ] {
        let mut clip = base.clone();
        clip.tracks.push(Track {
            bone: 0,
            property: if label == "quaternion" {
                Property::Rotation
            } else {
                Property::Translation
            },
            interpolation,
            times: (0..KEYS).map(|key| key as f32 / FPS as f32).collect(),
            values,
        });
        let before = format!("{clip:?}");
        let error = align_gait_anchor(&skel, &mut clip, &roles, FPS, GaitTrajectoryPolicy::InPlace)
            .unwrap_err();
        assert!(error.contains("track 2"), "{label}: {error}");
        assert!(error.contains("expected exactly"), "{label}: {error}");
        assert_eq!(format!("{clip:?}"), before);
    }
}

#[test]
fn interior_outliers_never_mask_endpoint_translation_or_yaw() {
    let (skel, mut clip) = open_walk();
    let roles = roles(&skel);
    clip.name = "outlier_mask".into();
    let mut translations = vec![Vec3::ZERO; KEYS];
    translations[10].x = 100.0;
    translations[11].x = -100.0;
    translations[KEYS - 1].x = 3.0;
    clip.tracks.push(root_translation(translations));
    let mut yaws = vec![0.0; KEYS];
    yaws[10] = 170.0f32.to_radians();
    yaws[11] = (-170.0f32).to_radians();
    yaws[KEYS - 1] = 90.0f32.to_radians();
    clip.tracks.push(root_yaw(yaws));

    let error = align_gait_anchor(&skel, &mut clip, &roles, FPS, GaitTrajectoryPolicy::InPlace)
        .unwrap_err();
    assert!(error.contains("translation 3.0000 m"), "got: {error}");
    assert!(error.contains("and yaw "), "got: {error}");
    assert!(error.contains("cap 1.000 deg"), "got: {error}");
}

#[test]
fn gait_anchor_pose_budget_is_inclusive_at_the_exact_cap() {
    let (mut skel, mut at_cap) = open_walk();
    extend_skeleton_to(&mut skel, 100);
    let roles = roles(&skel);
    at_cap.duration_s = 9_999.0;
    let at_cap_error = align_gait_anchor(
        &skel,
        &mut at_cap,
        &roles,
        1.0,
        GaitTrajectoryPolicy::InPlace,
    )
    .unwrap_err();
    assert!(
        !at_cap_error.contains("above the 1000000 sample safety budget"),
        "exactly 1,000,000 samples must pass the budget gate: {at_cap_error}"
    );
    assert!(at_cap_error.contains("incomplete whole-frame"));

    let (_, mut above) = open_walk();
    above.duration_s = 10_000.0;
    let error = align_gait_anchor(
        &skel,
        &mut above,
        &roles,
        1.0,
        GaitTrajectoryPolicy::InPlace,
    )
    .unwrap_err();
    assert!(
        error.contains("1000100 trajectory pose samples"),
        "got: {error}"
    );
    assert!(error.contains("above the 1000000 sample safety budget"));
}

#[test]
fn gait_anchor_fails_closed_on_unrepresentable_trajectory_grid() {
    let (skel, mut clip) = open_walk();
    let roles = roles(&skel);
    clip.name = "overflowing_grid".into();
    let before = format!("{clip:?}");

    let error = align_gait_anchor(
        &skel,
        &mut clip,
        &roles,
        f64::MAX,
        GaitTrajectoryPolicy::InPlace,
    )
    .unwrap_err();

    assert!(error.contains("overflowing_grid"), "got: {error}");
    assert!(
        error.contains("sample count that cannot be represented on this platform"),
        "got: {error}"
    );
    assert_eq!(format!("{clip:?}"), before);
}

fn open_walk_with_motion_ancestor() -> (Skeleton, Clip) {
    let (old_skel, mut clip) = open_walk();
    let mut bones = vec![Bone {
        name: "motion_root".into(),
        parent: None,
        rest: Transform::IDENTITY,
        inverse_bind: None,
    }];
    bones.extend(old_skel.bones.into_iter().map(|mut bone| {
        bone.parent = bone.parent.map(|parent| parent + 1).or(Some(0));
        bone
    }));
    for track in &mut clip.tracks {
        track.bone += 1;
    }
    (Skeleton { bones }, clip)
}

#[test]
fn gait_anchor_measures_ancestor_driven_model_space_trajectory() {
    let (skel, mut clip) = open_walk_with_motion_ancestor();
    clip.name = "ancestor_driven".into();
    clip.tracks.push(Track {
        bone: 0,
        property: Property::Translation,
        interpolation: Interpolation::Linear,
        times: (0..KEYS).map(|key| key as f32 / FPS as f32).collect(),
        values: TrackValues::Vec3s(
            (0..KEYS)
                .map(|key| Vec3::new(key as f32 * 0.1, 0.0, 0.0))
                .collect(),
        ),
    });
    clip.tracks.push(Track {
        bone: 0,
        property: Property::Rotation,
        interpolation: Interpolation::Linear,
        times: (0..KEYS).map(|key| key as f32 / FPS as f32).collect(),
        values: TrackValues::Quats(
            (0..KEYS)
                .map(|key| {
                    Quat::from_rotation_y(
                        key as f32 * std::f32::consts::FRAC_PI_2 / (KEYS - 1) as f32,
                    )
                })
                .collect(),
        ),
    });
    let roles = roles(&skel);
    let before = format!("{clip:?}");

    let error = align_gait_anchor(&skel, &mut clip, &roles, FPS, GaitTrajectoryPolicy::InPlace)
        .unwrap_err();

    assert_gait_refusal_is_located_and_atomic(&clip, &before, &error);
    assert!(error.contains("3.1000 m"), "got: {error}");
    assert!(error.contains("yaw 90.000 deg"), "got: {error}");
}

#[test]
fn gait_anchor_measures_ancestor_scale_in_model_space_trajectory() {
    let (mut skel, mut clip) = open_walk_with_motion_ancestor();
    // Hips is one metre to the side of its ancestor, so ancestor scale moves
    // the selected Hips origin horizontally even without a translation track.
    skel.bones[1].rest.translation.x = 1.0;
    clip.name = "ancestor_scale".into();
    clip.tracks.push(Track {
        bone: 0,
        property: Property::Scale,
        interpolation: Interpolation::Linear,
        times: (0..KEYS).map(|key| key as f32 / FPS as f32).collect(),
        values: TrackValues::Vec3s(
            (0..KEYS)
                .map(|key| Vec3::splat(1.0 + key as f32 / (KEYS - 1) as f32))
                .collect(),
        ),
    });
    let roles = roles(&skel);
    let before = format!("{clip:?}");

    let error = align_gait_anchor(&skel, &mut clip, &roles, FPS, GaitTrajectoryPolicy::InPlace)
        .unwrap_err();

    assert_gait_refusal_is_located_and_atomic(&clip, &before, &error);
    assert!(error.contains("1.0000 m"), "got: {error}");
    assert!(error.contains("yaw 0.000 deg"), "got: {error}");
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

/// #26: keys denser than the fps within the start epsilon must not all
/// collapse onto t=0, and a key just past the end must clamp into the
/// window rather than exceed the declared duration.
#[test]
fn slice_dedupes_start_boundary_and_clamps_end() {
    // fps=30 → eps = 1/60 ≈ 0.0167. Three keys fall within [start-eps,
    // start]; one falls within (end, end+eps].
    let times: Vec<f32> = vec![0.24, 0.245, 0.25, 0.40, 0.60, 0.75, 0.7575];
    let values: Vec<Vec3> = (0..times.len())
        .map(|i| Vec3::new(i as f32, 0.0, 0.0))
        .collect();
    let mut clip = Clip {
        name: "dense".into(),
        duration_s: 1.0,
        tracks: vec![Track {
            bone: 0,
            property: Property::Translation,
            interpolation: Interpolation::Linear,
            times,
            values: TrackValues::Vec3s(values),
        }],
    };

    slice(&mut clip, 0.25, 0.75, 30.0);
    let t = &clip.tracks[0];

    assert_eq!(
        t.times.iter().filter(|&&x| x == 0.0).count(),
        1,
        "at most one key at t=0: {:?}",
        t.times
    );
    for w in t.times.windows(2) {
        assert!(
            w[1] > w[0],
            "times must be strictly increasing: {:?}",
            t.times
        );
    }
    assert!(
        t.end_time() <= 0.5 + 1e-6,
        "last key {} exceeds duration 0.5",
        t.end_time()
    );
    assert!((clip.duration_s - 0.5).abs() < 1e-9);
    // Every surviving key keeps its original value (losslessness): the
    // boundary keys are the ones closest to the window — 0.25 (value 2)
    // and 0.75 (value 5) — and the interior keys 0.40/0.60 carry values
    // 3 and 4 verbatim.
    assert_eq!(
        (0..t.key_count())
            .map(|k| t.key_vec3(k).unwrap().x)
            .collect::<Vec<_>>(),
        vec![2.0, 3.0, 4.0, 5.0],
    );
}

/// #26 for CUBICSPLINE: dedup keeps whole tangent triplets aligned with
/// their (retimed) keys — a per-key stride of 1 would shred them.
#[test]
fn slice_dedupes_cubic_keeps_triplets_aligned() {
    // Two keys inside the start epsilon (0.24, 0.25); values are
    // triplets [in, value, out] with the value carrying the key index.
    let times: Vec<f32> = vec![0.24, 0.25, 0.40, 0.60, 0.75];
    let values: Vec<Vec3> = (0..times.len())
        .flat_map(|i| {
            [
                Vec3::new(i as f32, -1.0, 0.0), // in-tangent
                Vec3::new(i as f32, 0.0, 0.0),  // value
                Vec3::new(i as f32, 1.0, 0.0),  // out-tangent
            ]
        })
        .collect();
    let mut clip = Clip {
        name: "cubic".into(),
        duration_s: 1.0,
        tracks: vec![Track {
            bone: 0,
            property: Property::Translation,
            interpolation: Interpolation::CubicSpline,
            times,
            values: TrackValues::Vec3s(values),
        }],
    };

    slice(&mut clip, 0.25, 0.75, 30.0);
    let t = &clip.tracks[0];
    let TrackValues::Vec3s(v) = &t.values else {
        unreachable!()
    };
    assert_eq!(t.key_count(), 4, "0.24 dropped as a start duplicate");
    assert_eq!(v.len(), 3 * t.key_count(), "triplets intact");
    // Surviving original key indices are 1,2,3,4; their triplets must
    // land verbatim (in/value/out), proving cubic per_key=3 alignment.
    for (out_key, orig_i) in [1usize, 2, 3, 4].into_iter().enumerate() {
        assert_eq!(v[out_key * 3], Vec3::new(orig_i as f32, -1.0, 0.0));
        assert_eq!(v[out_key * 3 + 1], Vec3::new(orig_i as f32, 0.0, 0.0));
        assert_eq!(v[out_key * 3 + 2], Vec3::new(orig_i as f32, 1.0, 0.0));
    }
}

/// #26: the end clamp is load-bearing on its own — a single key just
/// past `end` (no key exactly at `end`, so the dedup never fires) must
/// still be pulled back into the window.
#[test]
fn slice_clamps_lone_past_end_key() {
    let times: Vec<f32> = vec![0.30, 0.50, 0.7575];
    let values: Vec<Vec3> = (0..times.len())
        .map(|i| Vec3::new(i as f32, 0.0, 0.0))
        .collect();
    let mut clip = Clip {
        name: "past-end".into(),
        duration_s: 1.0,
        tracks: vec![Track {
            bone: 0,
            property: Property::Translation,
            interpolation: Interpolation::Linear,
            times,
            values: TrackValues::Vec3s(values),
        }],
    };

    slice(&mut clip, 0.25, 0.75, 30.0);
    let t = &clip.tracks[0];
    assert!(
        t.end_time() <= 0.5 + 1e-6,
        "past-end key {} not clamped into the window",
        t.end_time()
    );
}

fn cubic_ramp_track(bone: BoneId) -> Track {
    // 3 keys, distinct values, zero tangents → non-constant CUBICSPLINE.
    let flat = |v: Vec3| [Vec3::ZERO, v, Vec3::ZERO];
    let values: Vec<Vec3> = [Vec3::ONE, Vec3::splat(1.5), Vec3::splat(2.0)]
        .into_iter()
        .flat_map(flat)
        .collect();
    Track {
        bone,
        property: Property::Scale,
        interpolation: Interpolation::CubicSpline,
        times: vec![0.0, 0.5, 1.0],
        values: TrackValues::Vec3s(values),
    }
}

/// #27: a non-constant CUBICSPLINE track cannot be rotated coherently;
/// align must refuse (naming it) rather than shift the linear tracks
/// and leave the cubic one behind.
#[test]
fn gait_anchor_refuses_mixed_interpolation_clips() {
    let (skel, mut clip) = open_walk();
    let roles = roles(&skel);
    clip.tracks.push(cubic_ramp_track(0));
    let original = clip.clone();

    let err = align_gait_anchor(&skel, &mut clip, &roles, FPS, GaitTrajectoryPolicy::InPlace)
        .unwrap_err();
    assert!(err.contains("cannot gait-anchor"), "got: {err}");
    assert!(
        err.contains("track 2"),
        "error should name the track: {err}"
    );
    // Refusal is total: the clip is left untouched, not partially rotated.
    assert_eq!(clip.tracks.len(), original.tracks.len());
    for (a, b) in clip.tracks.iter().zip(&original.tracks) {
        assert_eq!(a.key_vec3(0), b.key_vec3(0));
    }
}

/// A sparse nonconstant track cannot be resampled losslessly by a whole-frame
/// phase shift and must refuse before any sibling track changes.
#[test]
fn gait_anchor_refuses_short_non_constant_tracks() {
    let (skel, mut clip) = open_walk();
    let roles = roles(&skel);
    let dur = clip.duration_s as f32;
    clip.tracks.push(Track {
        bone: 0,
        property: Property::Scale,
        interpolation: Interpolation::Linear,
        times: vec![0.0, dur],
        values: TrackValues::Vec3s(vec![Vec3::ONE, Vec3::splat(2.0)]),
    });
    let before = format!("{clip:?}");
    let error = align_gait_anchor(&skel, &mut clip, &roles, FPS, GaitTrajectoryPolicy::InPlace)
        .unwrap_err();
    assert!(error.contains("track 2"), "got: {error}");
    assert!(
        error.contains("2 keys instead of exactly 32"),
        "got: {error}"
    );
    assert_eq!(format!("{clip:?}"), before);
}

/// #27: a *constant* CUBICSPLINE track is rotation-invariant, so
/// alignment must skip it (not refuse the whole clip) and leave it
/// byte-identical.
#[test]
fn gait_anchor_skips_constant_cubic_tracks() {
    let (skel, mut clip) = open_walk();
    let roles = roles(&skel);
    // Constant cubic: same value at every key, zero tangents.
    let held = Vec3::new(0.0, 2.0, 0.0);
    let values: Vec<Vec3> = (0..3)
        .flat_map(|_| [Vec3::ZERO, held, Vec3::ZERO])
        .collect();
    clip.tracks.push(Track {
        bone: 0,
        property: Property::Translation,
        interpolation: Interpolation::CubicSpline,
        times: vec![0.0, 0.5, 1.0],
        values: TrackValues::Vec3s(values),
    });
    let constant_before = clip.tracks.last().unwrap().clone();

    align_gait_anchor(&skel, &mut clip, &roles, FPS, GaitTrajectoryPolicy::InPlace)
        .expect("aligns, does not refuse");
    let constant_after = clip.tracks.last().unwrap();
    assert_eq!(
        vec3_values(&constant_before),
        vec3_values(constant_after),
        "a constant cubic track must be left untouched"
    );
}

#[test]
fn gait_anchor_constant_track_past_duration_cannot_change_the_rotation_period() {
    let (skel, mut control) = open_walk();
    let roles = roles(&skel);
    let mut with_constant = control.clone();
    let held = Vec3::new(0.0, 2.0, 0.0);
    with_constant.tracks.push(Track {
        bone: 0,
        property: Property::Translation,
        interpolation: Interpolation::CubicSpline,
        times: vec![0.0, 1.0, 2.0],
        values: TrackValues::Vec3s(
            (0..3)
                .flat_map(|_| [Vec3::ZERO, held, Vec3::ZERO])
                .collect(),
        ),
    });
    let constant_before = with_constant.tracks[2].clone();

    let control_outcome = align_gait_anchor(
        &skel,
        &mut control,
        &roles,
        FPS,
        GaitTrajectoryPolicy::InPlace,
    )
    .expect("control aligns");
    let constant_outcome = align_gait_anchor(
        &skel,
        &mut with_constant,
        &roles,
        FPS,
        GaitTrajectoryPolicy::InPlace,
    )
    .expect("an exempt constant track cannot alter the declared period");

    assert_eq!(constant_outcome.frame_offset, control_outcome.frame_offset);
    for track in 0..2 {
        assert_eq!(
            vec3_values(&with_constant.tracks[track]),
            vec3_values(&control.tracks[track]),
            "constant track endpoint must not affect nonconstant track {track}"
        );
    }
    assert_eq!(
        vec3_values(&with_constant.tracks[2]),
        vec3_values(&constant_before),
        "the exempt constant track itself remains untouched"
    );
}

/// #27: a CUBICSPLINE track whose keyed values are equal but whose
/// tangents are non-zero is an *animated* Hermite curve — the sampler
/// interpolates through the tangents. It must be refused (naming it),
/// not mistaken for a constant hold and silently left behind while the
/// rest of the rig rotates.
#[test]
fn gait_anchor_refuses_cubic_with_nonzero_tangents() {
    let (skel, mut clip) = open_walk();
    let roles = roles(&skel);
    let held = Vec3::new(0.0, 2.0, 0.0);
    let tangent = Vec3::new(1.0, 0.0, 0.0); // non-zero → curved segment
    let values: Vec<Vec3> = (0..3).flat_map(|_| [tangent, held, tangent]).collect();
    clip.tracks.push(Track {
        bone: 0,
        property: Property::Translation,
        interpolation: Interpolation::CubicSpline,
        times: vec![0.0, 0.5, 1.0],
        values: TrackValues::Vec3s(values),
    });
    let before = clip.clone();

    let err = align_gait_anchor(&skel, &mut clip, &roles, FPS, GaitTrajectoryPolicy::InPlace)
        .unwrap_err();
    assert!(err.contains("cannot gait-anchor"), "got: {err}");
    assert!(err.contains("pelvis"), "error should name the bone: {err}");
    assert!(
        err.contains("track 2"),
        "error should name the track: {err}"
    );
    // Refusal is total — nothing rotated.
    for (a, b) in clip.tracks.iter().zip(&before.tracks) {
        assert_eq!(vec3_values(a), vec3_values(b));
    }
}
