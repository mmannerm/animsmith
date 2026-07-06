//! Clip transforms: slice, hold-extend, gait-anchor rotation. The
//! gait test uses an OPEN cyclic loop (no duplicated endpoint key),
//! the shape the rotation semantics are defined for: the wrap step is
//! a real frame and the cycle period is `duration + 1/fps`.

use animsmith_core::model::*;
use animsmith_core::profile::{ResolvedRoles, Role};
use animsmith_core::sample::{TrackSample, sample_track};
use animsmith_core::transform::{align_gait_anchor, hold_extend, slice};
use glam::Vec3;
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

    let outcome = align_gait_anchor(&skel, &mut clip, &roles, FPS).expect("aligns");
    // The synthetic L−R foot-height signal is 2A·sin(2πk/32): its
    // fundamental trough sits at key 24 (a quarter cycle before the
    // wrap) — phase 0.75.
    assert!(
        (outcome.phase_before - 0.75).abs() < 0.05,
        "before: {}",
        outcome.phase_before
    );
    // The anchor must land within one frame of clip time 0. The bound is
    // deliberately below one frame (1/32 ≈ 0.031), so a rotation off by a
    // whole frame cannot satisfy it — the old 0.06 bound (≈ two frames)
    // let an off-by-one rotation pass.
    assert!(
        circular_delta(outcome.phase_after, 0.0) < 0.75 / KEYS as f64,
        "after: {} (>= one frame off — off-by-one rotation?)",
        outcome.phase_after
    );
    // The shift is pinned ANALYTICALLY, not read back from the outcome's
    // own `phase_before`: moving the key-24 trough to time 0 is a pure
    // 24-frame rotation, and a clean symmetric loop needs no wrap nudge
    // (frame_offset 0). A rotation off by one frame would shift by 23 or
    // 25 and fail the per-key equality below — the failure the previous
    // oracle (deriving its expected shift from the impl's outputs) could
    // not see. Quantized shifts land on existing keys, so *every* key
    // must match exactly, not all-but-one.
    assert_eq!(outcome.frame_offset, 0, "clean loop needs no wrap nudge");
    const SHIFT: usize = 24;
    let rotated = &clip.tracks[0];
    let orig = &original.tracks[0];
    for k in 0..KEYS {
        let want = orig.key_vec3((k + SHIFT) % KEYS).unwrap();
        let got = rotated.key_vec3(k).unwrap();
        assert!(
            (got - want).length() < 1e-6,
            "key {k}: rotated {got:?} != original key {} {want:?} — not a pure {SHIFT}-frame rotation",
            (k + SHIFT) % KEYS
        );
    }
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
    let values: Vec<Vec3> = [
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(0.5, 0.0, 0.0),
        Vec3::new(1.0, 0.0, 0.0),
    ]
    .into_iter()
    .flat_map(flat)
    .collect();
    Track {
        bone,
        property: Property::Translation,
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

    let err = align_gait_anchor(&skel, &mut clip, &roles, FPS).unwrap_err();
    assert!(err.contains("cannot gait-anchor"), "got: {err}");
    assert!(err.contains("bone 0"), "error should name the track: {err}");
    // Refusal is total: the clip is left untouched, not partially rotated.
    assert_eq!(clip.tracks.len(), original.tracks.len());
    for (a, b) in clip.tracks.iter().zip(&original.tracks) {
        assert_eq!(a.key_vec3(0), b.key_vec3(0));
    }
}

/// #27: a non-constant two-key LINEAR track (too short for the old
/// `< 3 keys` skip) must be rotated, not silently left in place.
#[test]
fn gait_anchor_rotates_short_non_constant_tracks() {
    let (skel, mut clip) = open_walk();
    let roles = roles(&skel);
    let dur = clip.duration_s as f32;
    clip.tracks.push(Track {
        bone: 0,
        property: Property::Translation,
        interpolation: Interpolation::Linear,
        times: vec![0.0, dur],
        values: TrackValues::Vec3s(vec![Vec3::new(0.0, 1.0, 0.0), Vec3::new(2.0, 1.0, 0.0)]),
    });
    let ramp_before = clip.tracks.last().unwrap().clone();

    let outcome = align_gait_anchor(&skel, &mut clip, &roles, FPS).expect("aligns");
    let ramp_after = clip.tracks.last().unwrap();

    let TrackValues::Vec3s(after) = &ramp_after.values else {
        unreachable!()
    };
    // Independently recompute the shift the rotation applied and sample
    // the ORIGINAL ramp there; the rotated values must match exactly
    // (this is the same lossless-resample contract the foot-track test
    // pins, but on a 2-key track — so a "resample at 0 → constant" or a
    // value-corrupting bug can't hide behind a bare `before != after`).
    let period = dur as f64 + 1.0 / FPS;
    let shift = (((outcome.phase_before * period * FPS).round() + outcome.frame_offset as f64)
        / FPS)
        .rem_euclid(period);
    for (k, &t) in [0.0f32, dur].iter().enumerate() {
        let TrackSample::Vec3(want) =
            sample_track(&ramp_before, ((t as f64 + shift) % period) as f32)
        else {
            unreachable!()
        };
        assert!(
            (after[k] - want).length() < 1e-6,
            "ramp key {k}: rotated {:?} != resampled {want:?}",
            after[k]
        );
    }
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

    align_gait_anchor(&skel, &mut clip, &roles, FPS).expect("aligns, does not refuse");
    let constant_after = clip.tracks.last().unwrap();
    assert_eq!(
        vec3_values(&constant_before),
        vec3_values(constant_after),
        "a constant cubic track must be left untouched"
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

    let err = align_gait_anchor(&skel, &mut clip, &roles, FPS).unwrap_err();
    assert!(err.contains("cannot gait-anchor"), "got: {err}");
    assert!(err.contains("bone 0"), "error should name the track: {err}");
    // Refusal is total — nothing rotated.
    for (a, b) in clip.tracks.iter().zip(&before.tracks) {
        assert_eq!(vec3_values(a), vec3_values(b));
    }
}
