use animsmith_core::glam::{Quat, Vec3};
use animsmith_core::transform::{
    CONSTANT_TRACK_PRUNE_QUAT_TOLERANCE_RAD, CONSTANT_TRACK_PRUNE_VEC3_TOLERANCE,
    ConstantTrackRetentionReason, prune_constant_tracks,
};
use animsmith_core::{
    Bone, Clip, Interpolation, Property, Skeleton, Track, TrackValues, Transform,
    default_frame_count, sample_clip,
};

#[derive(Debug, PartialEq, Eq)]
enum ValuesSnapshot {
    Vec3s(Vec<[u32; 3]>),
    Quats(Vec<[u32; 4]>),
}

type TrackSnapshot = (usize, Property, Interpolation, Vec<u32>, ValuesSnapshot);

fn clip_snapshot(clip: &Clip) -> (u64, Vec<TrackSnapshot>) {
    (
        clip.duration_s.to_bits(),
        clip.tracks
            .iter()
            .map(|track| {
                (
                    track.bone,
                    track.property,
                    track.interpolation,
                    track.times.iter().map(|time| time.to_bits()).collect(),
                    match &track.values {
                        TrackValues::Vec3s(values) => ValuesSnapshot::Vec3s(
                            values
                                .iter()
                                .map(|value| {
                                    [value.x.to_bits(), value.y.to_bits(), value.z.to_bits()]
                                })
                                .collect(),
                        ),
                        TrackValues::Quats(values) => ValuesSnapshot::Quats(
                            values
                                .iter()
                                .map(|value| {
                                    [
                                        value.x.to_bits(),
                                        value.y.to_bits(),
                                        value.z.to_bits(),
                                        value.w.to_bits(),
                                    ]
                                })
                                .collect(),
                        ),
                    },
                )
            })
            .collect(),
    )
}

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
        Track {
            bone: 0,
            property: Property::Rotation,
            interpolation: Interpolation::Linear,
            times: vec![0.0, 0.5, 1.0],
            values: TrackValues::Quats(vec![Quat::IDENTITY, small_rotation, Quat::IDENTITY]),
        },
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
    assert_eq!(default_frame_count(&original), 3);
    let outcome = prune_constant_tracks(&skeleton, &mut clip, &[]);
    assert!(outcome.removed.is_empty());
    assert_eq!(
        outcome.retained[0].reason,
        ConstantTrackRetentionReason::PoseChanged
    );
    assert_eq!(clip.tracks.len(), original.tracks.len());
}

#[test]
fn cumulative_trials_are_compared_with_the_untouched_original() {
    let skeleton = Skeleton {
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
                rest: Transform::IDENTITY,
                inverse_bind: None,
            },
            Bone {
                name: "leaf".into(),
                parent: Some(1),
                rest: Transform::IDENTITY,
                inverse_bind: None,
            },
        ],
    };
    let delta = Quat::from_rotation_z(CONSTANT_TRACK_PRUNE_QUAT_TOLERANCE_RAD * 0.6);
    let mut clip = clip(vec![
        quat_track(vec![delta, delta]),
        Track {
            bone: 1,
            ..quat_track(vec![delta, delta])
        },
        Track {
            bone: 2,
            ..quat_track(vec![Quat::IDENTITY, Quat::from_rotation_y(0.2)])
        },
    ]);

    let outcome = prune_constant_tracks(&skeleton, &mut clip, &[]);
    assert_eq!(
        outcome
            .removed
            .iter()
            .map(|record| record.original_track_index)
            .collect::<Vec<_>>(),
        [0]
    );
    assert_eq!(outcome.retained.len(), 1);
    assert_eq!(outcome.retained[0].record.original_track_index, 1);
    assert_eq!(
        outcome.retained[0].reason,
        ConstantTrackRetentionReason::PoseChanged
    );
}

#[test]
fn sampling_unavailable_candidates_are_reported_without_mutation() {
    let skeleton = skeleton(Transform::IDENTITY);
    let mut zero_duration = clip(vec![
        vec_track(
            Property::Translation,
            Interpolation::Linear,
            vec![Vec3::ZERO, Vec3::ZERO],
        ),
        quat_track(vec![Quat::IDENTITY, Quat::from_rotation_y(0.2)]),
    ]);
    zero_duration.duration_s = 0.0;
    let before = clip_snapshot(&zero_duration);
    let outcome = prune_constant_tracks(&skeleton, &mut zero_duration, &[]);
    assert!(outcome.removed.is_empty());
    assert_eq!(outcome.retained.len(), 1);
    assert_eq!(
        outcome.retained[0].reason,
        ConstantTrackRetentionReason::SamplingUnavailable
    );
    assert_eq!(clip_snapshot(&zero_duration), before);

    let overflow_skeleton = Skeleton {
        bones: vec![
            Bone {
                name: "root".into(),
                parent: None,
                rest: Transform {
                    scale: Vec3::splat(f32::MAX),
                    ..Transform::IDENTITY
                },
                inverse_bind: None,
            },
            Bone {
                name: "child".into(),
                parent: Some(0),
                rest: Transform {
                    translation: Vec3::splat(f32::MAX),
                    ..Transform::IDENTITY
                },
                inverse_bind: None,
            },
        ],
    };
    let mut trial_overflow = clip(vec![
        vec_track(
            Property::Scale,
            Interpolation::Linear,
            vec![Vec3::ZERO, Vec3::ZERO],
        ),
        quat_track(vec![Quat::IDENTITY, Quat::from_rotation_y(0.2)]),
    ]);
    let before = clip_snapshot(&trial_overflow);
    let outcome = prune_constant_tracks(&overflow_skeleton, &mut trial_overflow, &[]);
    assert!(outcome.removed.is_empty());
    assert_eq!(outcome.retained.len(), 1);
    assert_eq!(
        outcome.retained[0].reason,
        ConstantTrackRetentionReason::SamplingUnavailable
    );
    assert_eq!(clip_snapshot(&trial_overflow), before);
}

#[test]
fn single_key_pins_and_rotation_above_tolerance_are_not_candidates() {
    let skeleton = skeleton(Transform::IDENTITY);
    let mut single_key = clip(vec![Track {
        bone: 0,
        property: Property::Translation,
        interpolation: Interpolation::Linear,
        times: vec![0.0],
        values: TrackValues::Vec3s(vec![Vec3::ZERO]),
    }]);
    let outcome = prune_constant_tracks(&skeleton, &mut single_key, &[]);
    assert!(outcome.removed.is_empty() && outcome.retained.is_empty());
    assert_eq!(single_key.tracks.len(), 1);

    let mut changing_rotation = clip(vec![
        quat_track(vec![
            Quat::IDENTITY,
            Quat::from_rotation_z(CONSTANT_TRACK_PRUNE_QUAT_TOLERANCE_RAD * 2.0),
        ]),
        vec_track(
            Property::Translation,
            Interpolation::Linear,
            vec![Vec3::ZERO, Vec3::X],
        ),
    ]);
    let outcome = prune_constant_tracks(&skeleton, &mut changing_rotation, &[]);
    assert!(outcome.removed.is_empty() && outcome.retained.is_empty());
    assert_eq!(changing_rotation.tracks.len(), 2);
}

#[test]
fn exact_rest_vector_channels_coexist_with_cubic_rotation_and_slow_candidates() {
    let skeleton = Skeleton {
        bones: (0..3)
            .map(|bone| Bone {
                name: format!("bone-{bone}"),
                parent: None,
                rest: Transform::IDENTITY,
                inverse_bind: None,
            })
            .collect(),
    };
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
        Track {
            bone: 1,
            ..vec_track(
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
            )
        },
        Track {
            bone: 1,
            interpolation: Interpolation::Linear,
            ..quat_track(vec![Quat::IDENTITY, -Quat::IDENTITY])
        },
        Track {
            bone: 2,
            ..quat_track(vec![Quat::IDENTITY, Quat::from_rotation_y(0.2)])
        },
    ]);
    let original = clip.clone();

    let outcome = prune_constant_tracks(&skeleton, &mut clip, &[]);

    assert_eq!(
        outcome
            .removed
            .iter()
            .map(|record| record.original_track_index)
            .collect::<Vec<_>>(),
        [0, 1, 2, 3]
    );
    assert!(outcome.retained.is_empty());
    assert_original_grid_equal(&skeleton, &original, &clip);
}

#[test]
fn duplicate_channels_retain_the_existing_sampled_trial_semantics() {
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
    let mut clip = clip(vec![
        vec_track(
            Property::Scale,
            Interpolation::Linear,
            vec![Vec3::ONE, Vec3::ONE],
        ),
        vec_track(
            Property::Scale,
            Interpolation::Linear,
            vec![
                Vec3::ONE + Vec3::splat(CONSTANT_TRACK_PRUNE_VEC3_TOLERANCE * 0.5),
                Vec3::ONE + Vec3::splat(CONSTANT_TRACK_PRUNE_VEC3_TOLERANCE * 0.5),
            ],
        ),
        Track {
            bone: 1,
            ..quat_track(vec![Quat::IDENTITY, Quat::from_rotation_x(0.2)])
        },
    ]);
    let original = clip.clone();

    let outcome = prune_constant_tracks(&skeleton, &mut clip, &[]);

    assert_eq!(
        outcome
            .removed
            .iter()
            .map(|record| record.original_track_index)
            .collect::<Vec<_>>(),
        [0]
    );
    assert_eq!(outcome.retained.len(), 1);
    assert_eq!(outcome.retained[0].record.original_track_index, 1);
    assert_eq!(
        outcome.retained[0].reason,
        ConstantTrackRetentionReason::PoseChanged
    );
    assert_original_grid_equal(&skeleton, &original, &clip);
}

#[test]
fn fast_candidates_preserve_authored_order_and_last_writable_reason() {
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
    ]);

    let outcome = prune_constant_tracks(&skeleton, &mut clip, &[]);

    assert_eq!(
        outcome
            .removed
            .iter()
            .map(|record| record.original_track_index)
            .collect::<Vec<_>>(),
        [0]
    );
    assert_eq!(outcome.retained.len(), 1);
    assert_eq!(outcome.retained[0].record.original_track_index, 1);
    assert_eq!(
        outcome.retained[0].reason,
        ConstantTrackRetentionReason::LastWritableTrack
    );
}

#[test]
fn thousands_of_unique_exact_rest_channels_are_pruned_in_authored_order() {
    const CANDIDATE_COUNT: usize = 2_048;
    let skeleton = Skeleton {
        bones: (0..CANDIDATE_COUNT)
            .map(|bone| Bone {
                name: format!("bone-{bone}"),
                parent: None,
                rest: Transform::IDENTITY,
                inverse_bind: None,
            })
            .collect(),
    };
    let mut tracks: Vec<_> = (0..CANDIDATE_COUNT)
        .map(|bone| Track {
            bone,
            ..vec_track(
                Property::Translation,
                Interpolation::Linear,
                vec![Vec3::ZERO, Vec3::ZERO],
            )
        })
        .collect();
    tracks.push(quat_track(vec![Quat::IDENTITY, Quat::from_rotation_z(0.2)]));
    let mut clip = clip(tracks);
    let original = clip.clone();

    let outcome = prune_constant_tracks(&skeleton, &mut clip, &[]);

    assert_eq!(outcome.removed.len(), CANDIDATE_COUNT);
    assert_eq!(
        outcome
            .removed
            .iter()
            .map(|record| record.original_track_index)
            .collect::<Vec<_>>(),
        (0..CANDIDATE_COUNT).collect::<Vec<_>>()
    );
    assert!(outcome.retained.is_empty());
    assert_eq!(clip.tracks.len(), 1);
    assert_original_grid_equal(&skeleton, &original, &clip);
}
