use animsmith_core::glam::{Quat, Vec3};
use animsmith_core::transform::{
    CONSTANT_TRACK_PRUNE_QUAT_TOLERANCE_RAD, CONSTANT_TRACK_PRUNE_VEC3_TOLERANCE,
    ConstantTrackRetentionReason, prune_constant_tracks,
};
use animsmith_core::{
    Bone, Clip, Interpolation, Property, Skeleton, Track, TrackValues, Transform,
    default_frame_count, sample_clip,
};

fn skeleton(rest: Transform) -> Skeleton {
    Skeleton {
        bones: vec![Bone {
            name: "root".into(),
            parent: None,
            rest,
            inverse_bind: None,
        }],
    }
}

fn vec_track(property: Property, interpolation: Interpolation, values: Vec<Vec3>) -> Track {
    Track {
        bone: 0,
        property,
        interpolation,
        times: vec![0.0, 1.0],
        values: TrackValues::Vec3s(values),
    }
}

fn quat_track(values: Vec<Quat>) -> Track {
    Track {
        bone: 0,
        property: Property::Rotation,
        interpolation: Interpolation::Linear,
        times: vec![0.0, 1.0],
        values: TrackValues::Quats(values),
    }
}

fn clip(tracks: Vec<Track>) -> Clip {
    Clip {
        name: "clip".into(),
        duration_s: 1.0,
        tracks,
    }
}

fn assert_original_grid_equal(skeleton: &Skeleton, original: &Clip, final_clip: &Clip) {
    let frames = default_frame_count(original);
    let a = sample_clip(skeleton, original, frames);
    let b = sample_clip(skeleton, final_clip, frames);
    assert_eq!(a.frame_count(), b.frame_count());
    for frame in 0..a.frame_count() {
        for bone in 0..a.bone_count() {
            let left = a.local(frame, bone);
            let right = b.local(frame, bone);
            assert!((left.translation - right.translation).abs().max_element() <= 1e-4);
            assert!((left.scale - right.scale).abs().max_element() <= 1e-4);
            assert!(
                left.rotation
                    .normalize()
                    .angle_between(right.rotation.normalize())
                    <= 1e-3
            );
            assert!(
                (a.model_position(frame, bone) - b.model_position(frame, bone))
                    .abs()
                    .max_element()
                    <= 1e-4
            );
            assert!(
                a.model_rotation(frame, bone)
                    .normalize()
                    .angle_between(b.model_rotation(frame, bone).normalize())
                    <= 1e-3
            );
        }
    }
}

#[test]
fn prunes_rest_equivalent_linear_step_and_flat_cubic_tracks() {
    let skeleton = skeleton(Transform::IDENTITY);
    let mut clip = clip(vec![
        vec_track(
            Property::Translation,
            Interpolation::Linear,
            vec![Vec3::ZERO, Vec3::ZERO],
        ),
        vec_track(
            Property::Scale,
            Interpolation::Step,
            vec![Vec3::ONE, Vec3::ONE],
        ),
        vec_track(
            Property::Translation,
            Interpolation::CubicSpline,
            vec![
                Vec3::ZERO,
                Vec3::ZERO,
                Vec3::ZERO,
                Vec3::ZERO,
                Vec3::ZERO,
                Vec3::ZERO,
            ],
        ),
        quat_track(vec![Quat::IDENTITY, Quat::from_rotation_y(0.2)]),
    ]);
    let original = clip.clone();
    let outcome = prune_constant_tracks(&skeleton, &mut clip, &[]);
    assert_eq!(outcome.removed.len(), 3);
    assert_eq!(
        outcome
            .removed
            .iter()
            .map(|r| r.original_track_index)
            .collect::<Vec<_>>(),
        [0, 1, 2]
    );
    assert!(outcome.retained.is_empty());
    assert_original_grid_equal(&skeleton, &original, &clip);
}

#[test]
fn nonzero_cubic_tangents_are_not_candidates() {
    let skeleton = skeleton(Transform::IDENTITY);
    let mut clip = clip(vec![vec_track(
        Property::Translation,
        Interpolation::CubicSpline,
        vec![
            Vec3::ZERO,
            Vec3::ZERO,
            Vec3::X,
            Vec3::X,
            Vec3::ZERO,
            Vec3::ZERO,
        ],
    )]);
    let original = clip.clone();
    let outcome = prune_constant_tracks(&skeleton, &mut clip, &[]);
    assert!(outcome.removed.is_empty() && outcome.retained.is_empty());
    assert_eq!(clip.tracks.len(), 1);
    assert_original_grid_equal(&skeleton, &original, &clip);
}

#[test]
fn retains_non_rest_and_protected_tracks_with_reasons() {
    let skeleton = skeleton(Transform {
        translation: Vec3::X,
        ..Transform::IDENTITY
    });
    let mut clip = clip(vec![
        vec_track(
            Property::Translation,
            Interpolation::Linear,
            vec![Vec3::ZERO, Vec3::ZERO],
        ),
        vec_track(
            Property::Scale,
            Interpolation::Linear,
            vec![Vec3::ONE, Vec3::ONE],
        ),
    ]);
    let original = clip.clone();
    let outcome = prune_constant_tracks(&skeleton, &mut clip, &[0]);
    assert_eq!(
        outcome.retained[0].reason,
        ConstantTrackRetentionReason::ProtectedBone
    );
    assert_eq!(outcome.retained[0].record.original_track_index, 0);
    assert!(outcome.removed.is_empty());
    assert_eq!(
        outcome.retained.len(),
        2,
        "protection applies to every channel on the bone"
    );
    assert_original_grid_equal(&skeleton, &original, &clip);

    let mut non_rest = Clip {
        name: "clip".into(),
        duration_s: 1.0,
        tracks: vec![
            vec_track(
                Property::Translation,
                Interpolation::Linear,
                vec![Vec3::ZERO, Vec3::ZERO],
            ),
            vec_track(
                Property::Scale,
                Interpolation::Linear,
                vec![Vec3::ONE, Vec3::ONE],
            ),
        ],
    };
    let outcome = prune_constant_tracks(&skeleton, &mut non_rest, &[]);
    assert_eq!(
        outcome.retained[0].reason,
        ConstantTrackRetentionReason::PoseChanged
    );
}

#[test]
fn sign_invariant_quaternions_and_cumulative_duplicates_are_pruned_in_authored_order() {
    let skeleton = skeleton(Transform::IDENTITY);
    let mut clip = clip(vec![
        quat_track(vec![Quat::IDENTITY, -Quat::IDENTITY]),
        vec_track(
            Property::Translation,
            Interpolation::Linear,
            vec![Vec3::ZERO, Vec3::ZERO],
        ),
        vec_track(
            Property::Translation,
            Interpolation::Step,
            vec![Vec3::ZERO, Vec3::ZERO],
        ),
        quat_track(vec![Quat::IDENTITY, Quat::from_rotation_x(0.2)]),
    ]);
    let original = clip.clone();
    let outcome = prune_constant_tracks(&skeleton, &mut clip, &[]);
    assert_eq!(
        outcome
            .removed
            .iter()
            .map(|r| r.original_track_index)
            .collect::<Vec<_>>(),
        [0, 1, 2]
    );
    assert!(outcome.retained.is_empty());
    assert_original_grid_equal(&skeleton, &original, &clip);
    let again = prune_constant_tracks(&skeleton, &mut clip, &[]);
    assert!(again.removed.is_empty() && again.retained.is_empty());
}

#[test]
fn invalid_malformed_and_nonfinite_inputs_fail_closed_without_mutation() {
    let skeleton = skeleton(Transform::IDENTITY);
    let mut invalid_target = clip(vec![Track {
        bone: 9,
        ..vec_track(
            Property::Translation,
            Interpolation::Linear,
            vec![Vec3::ZERO, Vec3::ZERO],
        )
    }]);
    let before = invalid_target.clone();
    let outcome = prune_constant_tracks(&skeleton, &mut invalid_target, &[]);
    assert_eq!(
        outcome.retained[0].reason,
        ConstantTrackRetentionReason::InvalidTarget
    );
    assert_eq!(invalid_target.tracks.len(), before.tracks.len());

    let mut malformed = clip(vec![vec_track(
        Property::Translation,
        Interpolation::Linear,
        vec![Vec3::ZERO],
    )]);
    let before = malformed.clone();
    assert!(
        prune_constant_tracks(&skeleton, &mut malformed, &[])
            .removed
            .is_empty()
    );
    assert_eq!(
        malformed.tracks[0].values.len(),
        before.tracks[0].values.len()
    );

    let mut nonfinite = clip(vec![vec_track(
        Property::Translation,
        Interpolation::Linear,
        vec![Vec3::ZERO, Vec3::splat(f32::NAN)],
    )]);
    let before = nonfinite.clone();
    assert!(
        prune_constant_tracks(&skeleton, &mut nonfinite, &[])
            .removed
            .is_empty()
    );
    assert_eq!(
        nonfinite.tracks[0].values.len(),
        before.tracks[0].values.len()
    );
}

#[test]
fn last_writable_track_is_retained() {
    let skeleton = skeleton(Transform::IDENTITY);
    let mut clip = clip(vec![vec_track(
        Property::Translation,
        Interpolation::Linear,
        vec![Vec3::ZERO, Vec3::ZERO],
    )]);
    let outcome = prune_constant_tracks(&skeleton, &mut clip, &[]);
    assert!(outcome.removed.is_empty());
    assert_eq!(
        outcome.retained[0].reason,
        ConstantTrackRetentionReason::LastWritableTrack
    );
}

#[test]
fn tolerance_boundary_is_inclusive_and_larger_motion_is_not_a_candidate() {
    let skeleton = skeleton(Transform::IDENTITY);
    let dynamic = quat_track(vec![Quat::IDENTITY, Quat::from_rotation_y(0.2)]);

    let mut at_boundary = clip(vec![
        vec_track(
            Property::Translation,
            Interpolation::Linear,
            vec![Vec3::ZERO, Vec3::splat(CONSTANT_TRACK_PRUNE_VEC3_TOLERANCE)],
        ),
        dynamic.clone(),
    ]);
    let outcome = prune_constant_tracks(&skeleton, &mut at_boundary, &[]);
    assert_eq!(
        outcome
            .removed
            .iter()
            .map(|record| record.original_track_index)
            .collect::<Vec<_>>(),
        [0]
    );

    let mut above_boundary = clip(vec![
        vec_track(
            Property::Translation,
            Interpolation::Linear,
            vec![
                Vec3::ZERO,
                Vec3::splat(CONSTANT_TRACK_PRUNE_VEC3_TOLERANCE * 1.01),
            ],
        ),
        dynamic,
    ]);
    let outcome = prune_constant_tracks(&skeleton, &mut above_boundary, &[]);
    assert!(outcome.removed.is_empty() && outcome.retained.is_empty());

    let mut rotation = clip(vec![
        quat_track(vec![
            Quat::IDENTITY,
            Quat::from_rotation_z(CONSTANT_TRACK_PRUNE_QUAT_TOLERANCE_RAD * 0.5),
        ]),
        vec_track(
            Property::Translation,
            Interpolation::Linear,
            vec![Vec3::ZERO, Vec3::X],
        ),
    ]);
    assert_eq!(
        prune_constant_tracks(&skeleton, &mut rotation, &[]).removed[0].original_track_index,
        0
    );
}

#[test]
fn model_space_hierarchy_amplification_refuses_an_otherwise_local_match() {
    let skeleton = Skeleton {
        bones: vec![
            Bone {
                name: "root".into(),
                parent: None,
                rest: Transform::IDENTITY,
                inverse_bind: None,
            },
            Bone {
                name: "tip".into(),
                parent: Some(0),
                rest: Transform {
                    translation: Vec3::new(100.0, 0.0, 0.0),
                    ..Transform::IDENTITY
                },
                inverse_bind: None,
            },
        ],
    };
    let small_rotation = Quat::from_rotation_z(CONSTANT_TRACK_PRUNE_QUAT_TOLERANCE_RAD * 0.5);
    let mut clip = clip(vec![
        quat_track(vec![small_rotation, small_rotation]),
        Track {
            bone: 1,
            ..vec_track(
                Property::Translation,
                Interpolation::Linear,
                vec![Vec3::new(100.0, 0.0, 0.0), Vec3::new(101.0, 0.0, 0.0)],
            )
        },
    ]);
    let original = clip.clone();
    let outcome = prune_constant_tracks(&skeleton, &mut clip, &[]);
    assert!(outcome.removed.is_empty());
    assert_eq!(
        outcome.retained[0].reason,
        ConstantTrackRetentionReason::PoseChanged
    );
    assert_eq!(clip.tracks.len(), original.tracks.len());
}
