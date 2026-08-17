//! Pipeline-mechanical clip transforms, ported from the incubating
//! bake's Python: frame-window slicing, hold-extension, duplicate-endpoint
//! removal, and gait-anchor rotation. Scope rule (DESIGN.md §1): animsmith may rewrite a clip
//! only in ways whose correctness its own checks can verify.

use crate::checks::constant_track::{is_constant_track, quaternion_angular_delta};
use crate::metrics::foot_cycle_metrics;
use crate::model::{BoneId, Clip, Interpolation, Property, Skeleton, Track, TrackValues};
use crate::profile::{ResolvedRoles, Role};
use crate::sample::{PoseGrid, TrackSample, default_frame_count, sample_clip, sample_track};
use glam::{Quat, Vec3};
use std::collections::BTreeSet;
use std::fmt;
use thiserror::Error;

/// Failure while analyzing a duplicate loop endpoint.
#[derive(Debug, Clone, PartialEq, Error)]
#[non_exhaustive]
pub enum DuplicateLoopEndpointError {
    /// The clip has no tracks.
    #[error("clip has no tracks")]
    NoTracks,
    /// Stored value count does not exactly match the interpolation mode.
    #[error(
        "track {track} has {value_count} values for {key_count} keys with {interpolation:?} interpolation"
    )]
    InvalidValueCount {
        /// Index of the malformed track.
        track: usize,
        /// Number of authored keys.
        key_count: usize,
        /// Number of stored values.
        value_count: usize,
        /// Interpolation mode that determines values per key.
        interpolation: Interpolation,
    },
    /// A property uses incompatible value storage.
    #[error("track {track} has invalid value storage")]
    InvalidValueStorage {
        /// Index of the malformed track.
        track: usize,
    },
    /// A duration, key time, or stored value is non-finite.
    #[error("track {track:?} contains a non-finite authored value")]
    NonFinite {
        /// Index of the malformed track, or `None` for clip duration.
        track: Option<usize>,
    },
    /// A timeline is not strictly increasing.
    #[error("track {track} timeline is not strictly increasing")]
    NonIncreasingTime {
        /// Index of the malformed track.
        track: usize,
    },
    /// A track differs from the exact common authored timeline.
    #[error("track {track} does not share the exact authored timeline")]
    TimelineMismatch {
        /// Index of the mismatching track.
        track: usize,
    },
    /// A final key time does not equal the declared duration.
    #[error("track {track} does not end at the declared duration")]
    DurationMismatch {
        /// Index of the mismatching track.
        track: usize,
    },
}

/// The lossless change made by [`drop_duplicate_loop_endpoint`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct DuplicateLoopEndpointOutcome {
    /// Number of consecutive closing keys removed from every track.
    pub removed_keys_per_track: usize,
    /// Declared duration before removal.
    pub duration_before_s: f64,
    /// Duration re-pinned to the final retained key.
    pub duration_after_s: f64,
    /// Largest closing translation-component delta, in metres.
    pub max_translation_endpoint_delta_m: Option<f32>,
    /// Largest sign-invariant closing rotation delta, in radians.
    pub max_rotation_endpoint_delta_rad: Option<f32>,
    /// Largest closing scale-component delta.
    pub max_scale_endpoint_delta: Option<f32>,
}

/// Component-wise tolerance for duplicate translation and scale endpoints.
pub const DUPLICATE_ENDPOINT_VEC3_TOLERANCE: f32 = 1.0e-5;
/// Sign-invariant shortest-path angular tolerance for duplicate rotations.
pub const DUPLICATE_ENDPOINT_QUATERNION_TOLERANCE_RAD: f32 = 1.0e-4;

/// Maximum component-wise local translation/scale change accepted when
/// pruning a constant track. This aliases the `constant-track` check's
/// classification tolerance.
pub const CONSTANT_TRACK_PRUNE_VEC3_TOLERANCE: f32 = crate::checks::constant_track::VEC3_TOLERANCE;
/// Maximum sign-invariant local rotation change accepted when pruning a
/// constant track. This aliases the `constant-track` check's tolerance.
pub const CONSTANT_TRACK_PRUNE_QUAT_TOLERANCE_RAD: f32 =
    crate::checks::constant_track::QUAT_TOLERANCE_RAD;

/// Outcome of [`prune_constant_tracks`].
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct PruneConstantTracksOutcome {
    /// Candidate tracks removed, in original authored order.
    pub removed: Vec<ConstantTrackPruneRecord>,
    /// Candidate tracks retained, in original authored order.
    pub retained: Vec<ConstantTrackRetainedRecord>,
}

/// One constant-track candidate considered by [`prune_constant_tracks`].
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct ConstantTrackPruneRecord {
    /// Original index in [`Clip::tracks`].
    pub original_track_index: usize,
    /// Target bone.
    pub bone: BoneId,
    /// Target local TRS property.
    pub property: Property,
    /// Authored interpolation mode.
    pub interpolation: Interpolation,
    /// Number of authored keyframes.
    pub key_count: usize,
}

/// A candidate that [`prune_constant_tracks`] conservatively retained.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct ConstantTrackRetainedRecord {
    /// The candidate's immutable authored evidence.
    pub record: ConstantTrackPruneRecord,
    /// Why it was retained.
    pub reason: ConstantTrackRetentionReason,
}

/// Reason a constant-track candidate was not removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConstantTrackRetentionReason {
    /// The caller identifies this bone as a required authored channel.
    ProtectedBone,
    /// The track targets no bone in the supplied skeleton.
    InvalidTarget,
    /// The original or a trial clip cannot be safely sampled.
    SamplingUnavailable,
    /// Removing the track changes sampled local TRS or model-space pose data.
    PoseChanged,
    /// Removing the track would leave no writable track in the clip.
    LastWritableTrack,
}

impl fmt::Display for ConstantTrackRetentionReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ProtectedBone => "target bone is protected",
            Self::InvalidTarget => "track target is not present in the skeleton",
            Self::SamplingUnavailable => "the original or trial clip cannot be sampled safely",
            Self::PoseChanged => {
                "removal changes sampled local TRS or model-space position/rotation"
            }
            Self::LastWritableTrack => "removal would leave no writable track",
        })
    }
}

/// Remove constant multi-key tracks only when doing so preserves every local
/// TRS and model-space position/rotation on the original clip's default sample
/// grid.
///
/// This is deliberately more conservative than the `constant-track` check:
/// an all-zero translation track, for example, only disappears if the rest
/// pose and any other channel reproduce it. Candidate classification shares
/// that check's interpolation-aware tolerances; accepted removals are then
/// validated cumulatively against the untouched original. Invalid hand-built
/// inputs are retained instead of panicking. The final edit is atomic.
pub fn prune_constant_tracks(
    skeleton: &Skeleton,
    clip: &mut Clip,
    protected_bones: &[BoneId],
) -> PruneConstantTracksOutcome {
    prune_constant_tracks_impl(skeleton, clip, protected_bones, || {})
}

fn prune_constant_tracks_impl(
    skeleton: &Skeleton,
    clip: &mut Clip,
    protected_bones: &[BoneId],
    mut record_sampled_trial: impl FnMut(),
) -> PruneConstantTracksOutcome {
    let candidates: Vec<ConstantTrackPruneRecord> = clip
        .tracks
        .iter()
        .enumerate()
        .filter(|(_, track)| is_constant_track(track))
        .map(|(original_track_index, track)| ConstantTrackPruneRecord {
            original_track_index,
            bone: track.bone,
            property: track.property,
            interpolation: track.interpolation,
            key_count: track.key_count(),
        })
        .collect();
    if candidates.is_empty() {
        return PruneConstantTracksOutcome {
            removed: Vec::new(),
            retained: Vec::new(),
        };
    }

    if !valid_sampling_target(skeleton, clip) {
        return PruneConstantTracksOutcome {
            removed: Vec::new(),
            retained: candidates
                .into_iter()
                .map(|record| ConstantTrackRetainedRecord {
                    reason: if record.bone >= skeleton.bones.len() {
                        ConstantTrackRetentionReason::InvalidTarget
                    } else {
                        ConstantTrackRetentionReason::SamplingUnavailable
                    },
                    record,
                })
                .collect(),
        };
    }

    let frames = default_frame_count(clip);
    let original = sample_clip(skeleton, clip, frames);
    if !finite_grid(&original) {
        return PruneConstantTracksOutcome {
            removed: Vec::new(),
            retained: candidates
                .into_iter()
                .map(|record| ConstantTrackRetainedRecord {
                    record,
                    reason: ConstantTrackRetentionReason::SamplingUnavailable,
                })
                .collect(),
        };
    }

    let source = clip.clone();
    let duplicate_channels = duplicate_track_channels(&source);
    let protected_bones: BTreeSet<_> = protected_bones.iter().copied().collect();
    let mut accepted = BTreeSet::new();
    let mut removed_records = Vec::new();
    let mut retained = Vec::new();
    for record in candidates {
        if protected_bones.contains(&record.bone) {
            retained.push(ConstantTrackRetainedRecord {
                record,
                reason: ConstantTrackRetentionReason::ProtectedBone,
            });
            continue;
        }
        if source.tracks.len() <= accepted.len() + 1 {
            retained.push(ConstantTrackRetainedRecord {
                record,
                reason: ConstantTrackRetentionReason::LastWritableTrack,
            });
            continue;
        }
        let exact_rest_channel = source
            .tracks
            .get(record.original_track_index)
            .zip(skeleton.bones.get(record.bone))
            .is_some_and(|(track, bone)| {
                !duplicate_channels.contains(&track_channel_key(track))
                    && authored_track_is_exact_rest_equivalent(track, &bone.rest, &original)
            });
        if exact_rest_channel {
            accepted.insert(record.original_track_index);
            removed_records.push(record);
            continue;
        }
        record_sampled_trial();
        let mut trial = source.clone();
        trial.tracks = source
            .tracks
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != record.original_track_index && !accepted.contains(index))
            .map(|(_, track)| track.clone())
            .collect();
        let trial_grid = sample_clip(skeleton, &trial, frames);
        if !finite_grid(&trial_grid) {
            retained.push(ConstantTrackRetainedRecord {
                record,
                reason: ConstantTrackRetentionReason::SamplingUnavailable,
            });
        } else if !sampled_poses_match(&original, &trial_grid) {
            retained.push(ConstantTrackRetainedRecord {
                record,
                reason: ConstantTrackRetentionReason::PoseChanged,
            });
        } else {
            accepted.insert(record.original_track_index);
            removed_records.push(record);
        }
    }
    if !accepted.is_empty() {
        clip.tracks = source
            .tracks
            .into_iter()
            .enumerate()
            .filter(|(index, _)| !accepted.contains(index))
            .map(|(_, track)| track)
            .collect();
    }
    PruneConstantTracksOutcome {
        removed: removed_records,
        retained,
    }
}

#[cfg(test)]
mod constant_track_fast_path_tests {
    use super::*;
    use crate::model::{Bone, Transform};

    #[test]
    fn thousands_of_unique_exact_rest_channels_require_no_sampled_trials() {
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
                property: Property::Translation,
                interpolation: Interpolation::Linear,
                times: vec![0.0, 1.0],
                values: TrackValues::Vec3s(vec![Vec3::ZERO, Vec3::ZERO]),
            })
            .collect();
        tracks.push(Track {
            bone: 0,
            property: Property::Rotation,
            interpolation: Interpolation::Linear,
            times: vec![0.0, 1.0],
            values: TrackValues::Quats(vec![Quat::IDENTITY, Quat::from_rotation_z(0.2)]),
        });
        let mut clip = Clip {
            name: "large-exact-rest".into(),
            duration_s: 1.0,
            tracks,
        };
        let mut sampled_trials = 0;

        let outcome = prune_constant_tracks_impl(&skeleton, &mut clip, &[], || {
            sampled_trials += 1;
        });

        assert_eq!(outcome.removed.len(), CANDIDATE_COUNT);
        assert!(outcome.retained.is_empty());
        assert_eq!(sampled_trials, 0);
        assert_eq!(clip.tracks.len(), 1);
        assert_eq!(clip.tracks[0].property, Property::Rotation);
    }

    fn sampled_trials_for(tracks: Vec<Track>) -> usize {
        let skeleton = Skeleton {
            bones: vec![Bone {
                name: "root".into(),
                parent: None,
                rest: Transform::IDENTITY,
                inverse_bind: None,
            }],
        };
        let mut clip = Clip {
            name: "route".into(),
            duration_s: 1.0,
            tracks,
        };
        let mut sampled_trials = 0;
        let _ = prune_constant_tracks_impl(&skeleton, &mut clip, &[], || sampled_trials += 1);
        sampled_trials
    }

    fn vector_track(property: Property, interpolation: Interpolation, value: Vec3) -> Track {
        Track {
            bone: 0,
            property,
            interpolation,
            times: vec![0.0, 1.0],
            values: TrackValues::Vec3s(vec![value, value]),
        }
    }

    fn moving_rotation() -> Track {
        Track {
            bone: 0,
            property: Property::Rotation,
            interpolation: Interpolation::Linear,
            times: vec![0.0, 1.0],
            values: TrackValues::Quats(vec![Quat::IDENTITY, Quat::from_rotation_z(0.2)]),
        }
    }

    fn moving_scale() -> Track {
        Track {
            bone: 0,
            property: Property::Scale,
            interpolation: Interpolation::Linear,
            times: vec![0.0, 1.0],
            values: TrackValues::Vec3s(vec![Vec3::ONE, Vec3::splat(2.0)]),
        }
    }

    #[test]
    fn exact_route_and_sampled_fallback_domains_are_independently_pinned() {
        for (property, interpolation, value) in [
            (Property::Translation, Interpolation::Linear, Vec3::ZERO),
            (Property::Translation, Interpolation::Step, Vec3::ZERO),
            (Property::Scale, Interpolation::Linear, Vec3::ONE),
            (Property::Scale, Interpolation::Step, Vec3::ONE),
        ] {
            assert_eq!(
                sampled_trials_for(vec![
                    vector_track(property, interpolation, value),
                    moving_rotation(),
                ]),
                0,
                "{property:?}/{interpolation:?} exact-rest channels use the bounded route"
            );
        }

        let zero = Quat::from_xyzw(0.0, 0.0, 0.0, 0.0);
        let sampled_cases = [
            (
                2,
                vec![
                    Track {
                        bone: 0,
                        property: Property::Rotation,
                        interpolation: Interpolation::Linear,
                        times: vec![0.0, 1.0],
                        values: TrackValues::Quats(vec![Quat::IDENTITY, -Quat::IDENTITY]),
                    },
                    vector_track(Property::Translation, Interpolation::Linear, Vec3::X),
                    moving_scale(),
                ],
            ),
            (
                1,
                vec![
                    Track {
                        bone: 0,
                        property: Property::Translation,
                        interpolation: Interpolation::CubicSpline,
                        times: vec![0.0, 1.0],
                        values: TrackValues::Vec3s(vec![
                            Vec3::ZERO,
                            Vec3::ZERO,
                            Vec3::ZERO,
                            Vec3::ZERO,
                            Vec3::ZERO,
                            Vec3::ZERO,
                        ]),
                    },
                    moving_rotation(),
                ],
            ),
            (
                2,
                vec![
                    vector_track(Property::Scale, Interpolation::Linear, Vec3::ONE),
                    vector_track(Property::Scale, Interpolation::Linear, Vec3::ONE),
                    moving_rotation(),
                ],
            ),
            (
                1,
                vec![
                    vector_track(Property::Translation, Interpolation::Linear, Vec3::X),
                    moving_rotation(),
                ],
            ),
            (
                1,
                vec![
                    vector_track(
                        Property::Translation,
                        Interpolation::Linear,
                        Vec3::splat(CONSTANT_TRACK_PRUNE_VEC3_TOLERANCE * 0.5),
                    ),
                    moving_rotation(),
                ],
            ),
            (
                2,
                vec![
                    Track {
                        bone: 0,
                        property: Property::Rotation,
                        interpolation: Interpolation::CubicSpline,
                        times: vec![0.0, 1.0],
                        values: TrackValues::Quats(vec![
                            zero,
                            Quat::IDENTITY,
                            zero,
                            zero,
                            Quat::IDENTITY,
                            zero,
                        ]),
                    },
                    vector_track(Property::Translation, Interpolation::Linear, Vec3::X),
                    moving_scale(),
                ],
            ),
            (
                1,
                vec![
                    vector_track(
                        Property::Translation,
                        Interpolation::Linear,
                        Vec3::new(-0.0, 0.0, 0.0),
                    ),
                    moving_rotation(),
                ],
            ),
        ];
        for (expected_trials, tracks) in sampled_cases {
            assert_eq!(
                sampled_trials_for(tracks),
                expected_trials,
                "each rotation, cubic, duplicate, non-rest, tolerance, and bit-distinct case retains its own sampled proof"
            );
        }
    }

    #[test]
    fn linear_endpoints_that_round_on_the_grid_retain_the_sampled_proof() {
        let rest_value = Vec3::splat(12_000.0);
        let skeleton = Skeleton {
            bones: vec![Bone {
                name: "large".into(),
                parent: None,
                rest: Transform {
                    translation: rest_value,
                    ..Transform::IDENTITY
                },
                inverse_bind: None,
            }],
        };
        let rotation_times = (0..=200).map(|key| key as f32 / 200.0).collect::<Vec<_>>();
        let rotation_values = rotation_times
            .iter()
            .map(|time| Quat::from_rotation_z(*time * 0.2))
            .collect::<Vec<_>>();
        let mut clip = Clip {
            name: "linear-rounding".into(),
            duration_s: 1.0,
            tracks: vec![
                vector_track(Property::Translation, Interpolation::Linear, rest_value),
                Track {
                    bone: 0,
                    property: Property::Rotation,
                    interpolation: Interpolation::Linear,
                    times: rotation_times,
                    values: TrackValues::Quats(rotation_values),
                },
            ],
        };
        let mut sampled_trials = 0;

        let outcome = prune_constant_tracks_impl(&skeleton, &mut clip, &[], || {
            sampled_trials += 1;
        });

        assert_eq!(sampled_trials, 1);
        assert!(outcome.removed.is_empty());
        assert_eq!(clip.tracks.len(), 2);
        assert!((0..=200).any(|frame| {
            matches!(
                sample_track(&clip.tracks[0], frame as f32 / 200.0),
                TrackSample::Vec3(value) if !vec3_bits_eq(value, rest_value)
            )
        }));
    }
}

fn track_channel_key(track: &Track) -> (BoneId, u8) {
    let property = match track.property {
        Property::Translation => 0,
        Property::Rotation => 1,
        Property::Scale => 2,
    };
    (track.bone, property)
}

fn duplicate_track_channels(clip: &Clip) -> BTreeSet<(BoneId, u8)> {
    let mut seen = BTreeSet::new();
    let mut duplicates = BTreeSet::new();
    for track in &clip.tracks {
        if !seen.insert(track_channel_key(track)) {
            duplicates.insert(track_channel_key(track));
        }
    }
    duplicates
}

/// Whether deleting this sole authored vector channel produces its rest
/// component exactly, without relying on the sampled tolerance check. This is
/// only a stronger acceptance route for tracks that `is_constant_track`
/// already classified as candidates; rotation and cubic candidates retain the
/// sampled trial path.
fn authored_track_is_exact_rest_equivalent(
    track: &Track,
    rest: &crate::model::Transform,
    original: &PoseGrid,
) -> bool {
    let rest_value = match track.property {
        Property::Translation => rest.translation,
        Property::Scale => rest.scale,
        Property::Rotation => return false,
    };
    let TrackValues::Vec3s(values) = &track.values else {
        return false;
    };
    if !values.iter().all(|value| vec3_bits_eq(*value, rest_value)) {
        return false;
    }
    match track.interpolation {
        Interpolation::Step => true,
        Interpolation::Linear => (0..original.frame_count()).all(|frame| {
            let local = original.local(frame, track.bone);
            let value = match track.property {
                Property::Translation => local.translation,
                Property::Scale => local.scale,
                Property::Rotation => unreachable!("rotation was excluded above"),
            };
            vec3_bits_eq(value, rest_value)
        }),
        _ => false,
    }
}

fn vec3_bits_eq(a: Vec3, b: Vec3) -> bool {
    a.to_array()
        .into_iter()
        .zip(b.to_array())
        .all(|(a, b)| a.to_bits() == b.to_bits())
}

fn valid_sampling_target(skeleton: &Skeleton, clip: &Clip) -> bool {
    clip.duration_s.is_finite()
        && clip.duration_s > 0.0
        && skeleton.bones.iter().enumerate().all(|(index, bone)| {
            bone.parent.is_none_or(|parent| parent < index)
                && bone.rest.translation.is_finite()
                && bone.rest.scale.is_finite()
                && bone.rest.rotation.is_finite()
                && bone.rest.rotation.length_squared() > 0.0
        })
        && clip.tracks.iter().all(|track| {
            let Some(expected) = track.key_count().checked_mul(
                if track.interpolation == Interpolation::CubicSpline {
                    3
                } else {
                    1
                },
            ) else {
                return false;
            };
            track.bone < skeleton.bones.len()
                && track.key_count() > 0
                && track.values.len() == expected
                && track.times.iter().all(|time| time.is_finite())
                && track.times.windows(2).all(|pair| pair[0] < pair[1])
                && matches!(
                    (track.property, &track.values),
                    (Property::Rotation, TrackValues::Quats(_))
                        | (
                            Property::Translation | Property::Scale,
                            TrackValues::Vec3s(_)
                        )
                )
                && match &track.values {
                    TrackValues::Vec3s(values) => values.iter().all(|value| value.is_finite()),
                    TrackValues::Quats(values) => {
                        values.iter().enumerate().all(|(index, value)| {
                            value.is_finite()
                                && (track.interpolation == Interpolation::CubicSpline
                                    && index % 3 != 1
                                    || value.length_squared() > 0.0)
                        })
                    }
                }
        })
}

fn finite_grid(grid: &crate::sample::PoseGrid) -> bool {
    (0..grid.frame_count()).all(|frame| {
        (0..grid.bone_count()).all(|bone| {
            let pose = grid.local(frame, bone);
            let model_position = grid.model_position(frame, bone);
            let model_rotation = grid.model_rotation(frame, bone);
            pose.translation.is_finite()
                && pose.scale.is_finite()
                && pose.rotation.is_finite()
                && pose.rotation.length_squared() > 0.0
                && model_position.is_finite()
                && model_rotation.is_finite()
                && model_rotation.length_squared() > 0.0
        })
    })
}

fn sampled_poses_match(
    original: &crate::sample::PoseGrid,
    trial: &crate::sample::PoseGrid,
) -> bool {
    original.frame_count() == trial.frame_count()
        && original.bone_count() == trial.bone_count()
        && (0..original.frame_count()).all(|frame| {
            (0..original.bone_count()).all(|bone| {
                let a = original.local(frame, bone);
                let b = trial.local(frame, bone);
                vec3_within(a.translation, b.translation)
                    && vec3_within(a.scale, b.scale)
                    && quaternion_within(a.rotation, b.rotation)
                    && vec3_within(
                        original.model_position(frame, bone),
                        trial.model_position(frame, bone),
                    )
                    && quaternion_within(
                        original.model_rotation(frame, bone),
                        trial.model_rotation(frame, bone),
                    )
            })
        })
}

fn vec3_within(a: Vec3, b: Vec3) -> bool {
    (a - b).abs().max_element() <= CONSTANT_TRACK_PRUNE_VEC3_TOLERANCE
}

fn quaternion_within(a: Quat, b: Quat) -> bool {
    quaternion_angular_delta(a, b)
        .is_some_and(|delta| delta <= CONSTANT_TRACK_PRUNE_QUAT_TOLERANCE_RAD)
}

/// Analyze whether a clip has a safe, duplicated loop endpoint.
///
/// The authored timeline must be finite, strictly increasing, and exactly
/// shared by every track; each track must have exact key/value cardinality,
/// at least three keys, and a final time exactly equal to clip duration.
/// Closing vectors compare component-wise within `1e-5`; quaternions compare
/// with sign-invariant shortest-path angular distance within `1e-4` radians.
/// The predicate is the mechanically removable subset of #22's future
/// `duplicate_endpoint` mode, not a parallel endpoint-mode classifier.
/// `Ok(None)` is a valid non-candidate, including two-key clips and stationary
/// holds.
pub fn analyze_duplicate_loop_endpoint(
    clip: &Clip,
) -> Result<Option<DuplicateLoopEndpointOutcome>, DuplicateLoopEndpointError> {
    let Some(reference) = clip.tracks.first() else {
        return Err(DuplicateLoopEndpointError::NoTracks);
    };
    if !clip.duration_s.is_finite() {
        return Err(DuplicateLoopEndpointError::NonFinite { track: None });
    }
    let mut moving_terminal_count = None;
    let mut terminal_counts = Vec::with_capacity(clip.tracks.len());
    let mut max_translation_endpoint_delta_m: Option<f32> = None;
    let mut max_rotation_endpoint_delta_rad: Option<f32> = None;
    let mut max_scale_endpoint_delta: Option<f32> = None;
    for (index, track) in clip.tracks.iter().enumerate() {
        validate_duplicate_endpoint_track(index, track)?;
        if track.times != reference.times {
            return Err(DuplicateLoopEndpointError::TimelineMismatch { track: index });
        }
        // Authored key times are f32 even though the model carries duration as
        // f64. Compare in the authored time domain so a preceding transform
        // such as `slice` is not rejected only for f64 representation dust.
        if track.end_time() != clip.duration_s as f32 {
            return Err(DuplicateLoopEndpointError::DurationMismatch { track: index });
        }
        if track.key_count() < 3 {
            return Ok(None);
        }
        let Some(count) = terminal_duplicate_count(track) else {
            return Ok(None);
        };
        let final_key = track.key_count() - 1;
        match track.property {
            Property::Translation => {
                let delta = vec3_key_delta(track, 0, final_key);
                max_translation_endpoint_delta_m = Some(
                    max_translation_endpoint_delta_m.map_or(delta, |current| current.max(delta)),
                );
            }
            Property::Rotation => {
                let Some(delta) = quaternion_key_delta(track, 0, final_key) else {
                    return Ok(None);
                };
                max_rotation_endpoint_delta_rad = Some(
                    max_rotation_endpoint_delta_rad.map_or(delta, |current| current.max(delta)),
                );
            }
            Property::Scale => {
                let delta = vec3_key_delta(track, 0, final_key);
                max_scale_endpoint_delta =
                    Some(max_scale_endpoint_delta.map_or(delta, |current| current.max(delta)));
            }
        }
        let moves = track_has_motion(track);
        if moves {
            if moving_terminal_count.is_some_and(|expected| expected != count) {
                return Ok(None);
            }
            moving_terminal_count = Some(count);
        }
        terminal_counts.push(count);
    }
    let Some(removed_keys_per_track) = moving_terminal_count else {
        return Ok(None);
    };
    if terminal_counts
        .into_iter()
        .any(|available| available < removed_keys_per_track)
    {
        return Ok(None);
    }
    Ok(Some(DuplicateLoopEndpointOutcome {
        removed_keys_per_track,
        duration_before_s: clip.duration_s,
        duration_after_s: reference.times[reference.key_count() - removed_keys_per_track - 1]
            as f64,
        max_translation_endpoint_delta_m,
        max_rotation_endpoint_delta_rad,
        max_scale_endpoint_delta,
    }))
}

/// Atomically remove all consecutive duplicate closing keys from every track.
///
/// Retained times, values, and cubic tangent/value/tangent triplets are
/// unchanged. Errors and non-candidates leave `clip` untouched.
pub fn drop_duplicate_loop_endpoint(
    clip: &mut Clip,
) -> Result<Option<DuplicateLoopEndpointOutcome>, DuplicateLoopEndpointError> {
    let Some(outcome) = analyze_duplicate_loop_endpoint(clip)? else {
        return Ok(None);
    };
    for track in &mut clip.tracks {
        let values = outcome.removed_keys_per_track
            * if track.interpolation == Interpolation::CubicSpline {
                3
            } else {
                1
            };
        track
            .times
            .truncate(track.key_count() - outcome.removed_keys_per_track);
        match &mut track.values {
            TrackValues::Vec3s(stored) => stored.truncate(stored.len() - values),
            TrackValues::Quats(stored) => stored.truncate(stored.len() - values),
        }
    }
    clip.duration_s = outcome.duration_after_s;
    debug_assert!(matches!(analyze_duplicate_loop_endpoint(clip), Ok(None)));
    Ok(Some(outcome))
}

fn validate_duplicate_endpoint_track(
    index: usize,
    track: &Track,
) -> Result<(), DuplicateLoopEndpointError> {
    let keys = track.key_count();
    let expected = keys
        * if track.interpolation == Interpolation::CubicSpline {
            3
        } else {
            1
        };
    if track.values.len() != expected {
        return Err(DuplicateLoopEndpointError::InvalidValueCount {
            track: index,
            key_count: keys,
            value_count: track.values.len(),
            interpolation: track.interpolation,
        });
    }
    if !matches!(
        (track.property, &track.values),
        (Property::Rotation, TrackValues::Quats(_))
            | (
                Property::Translation | Property::Scale,
                TrackValues::Vec3s(_)
            )
    ) {
        return Err(DuplicateLoopEndpointError::InvalidValueStorage { track: index });
    }
    if track.times.iter().any(|time| !time.is_finite())
        || match &track.values {
            TrackValues::Vec3s(values) => values.iter().any(|value| !value.is_finite()),
            TrackValues::Quats(values) => values.iter().any(|value| !value.is_finite()),
        }
    {
        return Err(DuplicateLoopEndpointError::NonFinite { track: Some(index) });
    }
    if track.times.windows(2).any(|window| window[1] <= window[0]) {
        return Err(DuplicateLoopEndpointError::NonIncreasingTime { track: index });
    }
    Ok(())
}

fn terminal_duplicate_count(track: &Track) -> Option<usize> {
    let mut count = 0;
    while count < track.key_count() - 2
        && keyed_values_match(track, 0, track.key_count() - count - 1)
    {
        count += 1;
    }
    (count > 0).then_some(count)
}

fn track_has_motion(track: &Track) -> bool {
    (1..track.key_count()).any(|key| !keyed_values_match(track, 0, key))
        || (track.interpolation == Interpolation::CubicSpline
            && match &track.values {
                TrackValues::Vec3s(values) => values
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| index % 3 != 1)
                    .any(|(_, value)| {
                        value.abs().max_element() > DUPLICATE_ENDPOINT_VEC3_TOLERANCE
                    }),
                TrackValues::Quats(values) => values
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| index % 3 != 1)
                    .any(|(_, value)| {
                        value
                            .to_array()
                            .into_iter()
                            .any(|component| component.abs() > DUPLICATE_ENDPOINT_VEC3_TOLERANCE)
                    }),
            })
}

fn keyed_values_match(track: &Track, first: usize, other: usize) -> bool {
    match &track.values {
        TrackValues::Vec3s(_) => {
            vec3_key_delta(track, first, other) <= DUPLICATE_ENDPOINT_VEC3_TOLERANCE
        }
        TrackValues::Quats(_) => quaternion_key_delta(track, first, other)
            .is_some_and(|delta| delta <= DUPLICATE_ENDPOINT_QUATERNION_TOLERANCE_RAD),
    }
}

fn vec3_key_delta(track: &Track, first: usize, other: usize) -> f32 {
    let TrackValues::Vec3s(values) = &track.values else {
        unreachable!("validated vector track")
    };
    (values[track.value_index(first)] - values[track.value_index(other)])
        .abs()
        .max_element()
}

fn quaternion_key_delta(track: &Track, first: usize, other: usize) -> Option<f32> {
    let TrackValues::Quats(values) = &track.values else {
        unreachable!("validated quaternion track")
    };
    let first = values[track.value_index(first)];
    let other = values[track.value_index(other)];
    let first_length_squared = first.length_squared();
    let other_length_squared = other.length_squared();
    if first_length_squared == 0.0 || other_length_squared == 0.0 {
        return None;
    }
    let delta = first.normalize().conjugate() * other.normalize();
    let [x, y, z, w] = delta.to_array();
    let sin_half_angle = glam::Vec3::new(x, y, z).length();
    Some(2.0 * sin_half_angle.atan2(w.abs()))
}

/// Keep only the keys inside `[start, end]` seconds (with a half-frame
/// epsilon at `fps` absorbing float drift from earlier retimings) and
/// retime them so the window starts at 0. Cubic tangent triplets move
/// with their keys. The clip duration becomes `end - start`.
///
/// Boundary keys are snapped to the window, not carried past it: keys
/// within the epsilon of `start` clamp to 0 and keys within it of `end`
/// clamp to the new duration. When several keys land on a boundary, the
/// one closest to the original boundary is kept and the rest dropped —
/// so the output has at most one key at 0 and one at the end, stays
/// time-monotonic, and round-trips its declared duration.
///
/// # Panics
///
/// Panics if a hand-built track violates the loader invariant that
/// `values` contains one value per key for linear/step tracks, or one
/// tangent-value-tangent triplet per key for cubic-spline tracks.
pub fn slice(clip: &mut Clip, start_s: f64, end_s: f64, fps: f64) {
    let eps = (0.5 / fps) as f32;
    let (start, end) = (start_s as f32, end_s as f32);
    let duration = (end - start).max(0.0);
    for track in &mut clip.tracks {
        // (key index, retimed+clamped time), in original key order.
        let mut kept: Vec<(usize, f32)> = (0..track.key_count())
            .filter(|&k| track.times[k] >= start - eps && track.times[k] <= end + eps)
            .map(|k| (k, (track.times[k] - start).clamp(0.0, duration)))
            .collect();

        // Drop boundary duplicates: at t=0 keep the last (closest to
        // `start`); at t=duration keep the first (closest to `end`).
        // Interior times are already distinct and monotonic.
        kept.retain({
            let times: Vec<f32> = kept.iter().map(|&(_, t)| t).collect();
            let mut i = 0;
            move |_| {
                let t = times[i];
                let keep = if t <= 0.0 {
                    times.get(i + 1).is_none_or(|&next| next > 0.0)
                } else if t >= duration {
                    i == 0 || times[i - 1] < duration
                } else {
                    true
                };
                i += 1;
                keep
            }
        });

        track.times = kept.iter().map(|&(_, t)| t).collect();
        let per_key = match track.interpolation {
            Interpolation::CubicSpline => 3,
            _ => 1,
        };
        match &mut track.values {
            TrackValues::Vec3s(v) => {
                let old = std::mem::take(v);
                *v = kept
                    .iter()
                    .flat_map(|&(k, _)| old[k * per_key..(k + 1) * per_key].to_vec())
                    .collect();
            }
            TrackValues::Quats(v) => {
                let old = std::mem::take(v);
                *v = kept
                    .iter()
                    .flat_map(|&(k, _)| old[k * per_key..(k + 1) * per_key].to_vec())
                    .collect();
            }
        }
    }
    clip.duration_s = (end_s - start_s).max(0.0);
    clip.tracks.retain(|t| t.key_count() > 0);
}

/// Append one key per track duplicating its final value `hold_s`
/// seconds after its last key (a linear hold — charge/block poses).
/// The clip duration extends to the longest held end.
///
/// # Panics
///
/// Panics if a hand-built track violates the loader invariant that each
/// key has a corresponding stored value (or cubic-spline triplet).
pub fn hold_extend(clip: &mut Clip, hold_s: f64) {
    for track in &mut clip.tracks {
        let Some(&last) = track.times.last() else {
            continue;
        };
        let key = track.key_count() - 1;
        track.times.push(last + hold_s as f32);
        let value_index = track.value_index(key);
        match &mut track.values {
            TrackValues::Vec3s(v) => {
                let value = v[value_index];
                match track.interpolation {
                    Interpolation::CubicSpline => {
                        // Zero tangents: a flat Hermite hold. Also zero
                        // the previous key's out-tangent so the hold
                        // segment stays flat.
                        v[key * 3 + 2] = glam::Vec3::ZERO;
                        v.extend_from_slice(&[glam::Vec3::ZERO, value, glam::Vec3::ZERO]);
                    }
                    _ => v.push(value),
                }
            }
            TrackValues::Quats(v) => {
                let value = v[value_index];
                match track.interpolation {
                    Interpolation::CubicSpline => {
                        v[key * 3 + 2] = glam::Quat::from_xyzw(0.0, 0.0, 0.0, 0.0);
                        v.extend_from_slice(&[
                            glam::Quat::from_xyzw(0.0, 0.0, 0.0, 0.0),
                            value,
                            glam::Quat::from_xyzw(0.0, 0.0, 0.0, 0.0),
                        ]);
                    }
                    _ => v.push(value),
                }
            }
        }
        clip.duration_s = clip.duration_s.max((last + hold_s as f32) as f64);
    }
}

/// Outcome of [`align_gait_anchor`].
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct GaitAlignOutcome {
    /// The measured stride-anchor phase before rotation.
    pub phase_before: f64,
    /// The phase after rotation (should sit near 0).
    pub phase_after: f64,
    /// Loop-seam ratio after rotation (the chosen candidate's wrap).
    pub seam_after: Option<f64>,
    /// The whole-frame offset (−1/0/+1) that produced the cleanest wrap.
    pub frame_offset: i32,
}

/// Declared movement contract under which gait-anchor rotation may run.
///
/// The policy is an explicit caller obligation rather than an inference from
/// clip names or measured speed. Gait anchoring cyclically reorders every
/// animated channel, so it is only safe when the selected root trajectory is
/// itself cyclic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum GaitTrajectoryPolicy {
    /// The caller declares that gameplay, not the clip, owns locomotion travel.
    /// The transform verifies that declaration before rewriting any channel.
    InPlace,
}

/// Maximum horizontal root-trajectory endpoint displacement admitted by the
/// in-place gait-anchor policy.
pub const GAIT_ANCHOR_MAX_HORIZONTAL_ACCUMULATION_M: f64 = 0.01;

/// Maximum accumulated root yaw admitted by the in-place gait-anchor policy.
pub const GAIT_ANCHOR_MAX_YAW_ACCUMULATION_DEG: f64 = 1.0;

/// Maximum `declared frame samples × skeleton bones` the in-place gait
/// trajectory verifier will allocate and evaluate.
pub const GAIT_ANCHOR_MAX_TRAJECTORY_POSE_SAMPLES: usize = 1_000_000;

const GAIT_TRAJECTORY_ALTERNATIVES: &str = "retain source root motion, use runtime phase offsets, or use a separately designed \
     trajectory-preserving operation";

/// Rotate a cyclic clip in time so its measured stride anchor (the
/// trough of the L−R foot-height fundamental) lands at clip time 0.
///
/// Semantics ported from the reference bake: the cycle period is
/// `duration + 1/fps` (an open loop's wrap step is a real frame of the
/// stride); the shift is quantized to whole frames so every resample
/// lands on an existing key; each animated channel keeps its times and
/// gets its output values replaced by the channel sampled at
/// `(t + shift) mod period`. Constant channels are rotation-invariant
/// and left alone; a non-constant CUBICSPLINE channel cannot be
/// resampled losslessly, so alignment refuses (naming it) rather than
/// rotate the rest of the rig around it. Because a ±1-frame shift stays
/// inside phase tolerance but moves *where the wrap lands*, all three
/// candidates are tried and the one with the cleanest wrap (lowest seam
/// ratio) wins.
///
/// # Errors
///
/// Returns an error when the clip has no measurable stride anchor, the
/// left-right foot amplitude is too small to define a stable phase, a
/// non-constant cubic-spline track would need lossy resampling, or no
/// tested rotation candidate remains measurable. Under
/// [`GaitTrajectoryPolicy::InPlace`], missing/non-finite selected-root
/// evidence, any nonconstant channel without a complete declared whole-frame
/// key grid, malformed track cardinality or skeleton/role topology, pose-sample work above
/// [`GAIT_ANCHOR_MAX_TRAJECTORY_POSE_SAMPLES`], and material horizontal
/// translation or yaw accumulation are also errors. All errors are returned
/// before `clip` is changed.
///
pub fn align_gait_anchor(
    skeleton: &Skeleton,
    clip: &mut Clip,
    roles: &ResolvedRoles,
    fps: f64,
    trajectory_policy: GaitTrajectoryPolicy,
) -> Result<GaitAlignOutcome, String> {
    let sampling_frames = match trajectory_policy {
        GaitTrajectoryPolicy::InPlace => {
            verify_in_place_gait_trajectory(skeleton, clip, roles, fps)?
        }
    };

    let measure = |c: &Clip| -> Option<(f64, Option<f64>, f64)> {
        let grid = sample_clip(skeleton, c, sampling_frames);
        let m = foot_cycle_metrics(&grid, roles, crate::metrics::MIN_STRIDE_STEP_M)?;
        Some((m.gait_phase?, m.loop_seam_ratio, m.lr_amplitude_m))
    };
    let Some((phase_before, _, amplitude)) = measure(clip) else {
        return Err(
            "no usable stride anchor (hips/foot roles unresolved or clip too short)".into(),
        );
    };
    if amplitude < 0.03 {
        return Err(format!(
            "no usable stride anchor (L−R amplitude {amplitude:.4} m) — a ring clip must \
             alternate its feet for anchor alignment to mean anything"
        ));
    }

    // Refuse rather than rotate part of a clip: a channel we cannot
    // resample coherently (a non-constant CUBICSPLINE track) would be
    // left in place while its siblings shift, desynchronizing the rig.
    // Constant tracks are rotation-invariant and safely skipped.
    let unrotatable: Vec<String> = clip
        .tracks
        .iter()
        .filter(|t| {
            t.interpolation == Interpolation::CubicSpline && !is_rotation_invariant_track(t)
        })
        .map(|t| format!("{} bone {}", t.property.as_str(), t.bone))
        .collect();
    if !unrotatable.is_empty() {
        return Err(format!(
            "cannot gait-anchor: these animated tracks need lossless resampling that is \
             not yet supported ({}); retime them to LINEAR first",
            unrotatable.join(", ")
        ));
    }

    let original = clip.clone();
    let mut best: Option<(f64, GaitAlignOutcome, Clip)> = None;
    for frame_offset in [0i32, -1, 1] {
        let mut candidate = original.clone();
        rotate_values(&mut candidate, phase_before, fps, frame_offset);
        let Some((phase_after, seam_after, _)) = measure(&candidate) else {
            continue;
        };
        // Rank by wrap cleanliness; a missing seam (no stride at the
        // wrap) should not happen on a ring clip — rank it last.
        let rank = seam_after.unwrap_or(f64::MAX);
        if best.as_ref().is_none_or(|(r, _, _)| rank < *r) {
            best = Some((
                rank,
                GaitAlignOutcome {
                    phase_before,
                    phase_after,
                    seam_after,
                    frame_offset,
                },
                candidate,
            ));
        }
    }
    let Some((_, outcome, rotated)) = best else {
        return Err("no rotation candidate was measurable".into());
    };
    *clip = rotated;
    Ok(outcome)
}

/// Verify that cyclic time rotation cannot move an authored world trajectory
/// wrap into the middle of the clip.
///
/// The fixed caps apply directly to endpoint displacement and accumulated yaw.
/// No sampled step is subtracted as an allowance: an interior outlier must
/// never authorize unrelated endpoint drift.
fn verify_in_place_gait_trajectory(
    skeleton: &Skeleton,
    clip: &Clip,
    roles: &ResolvedRoles,
    fps: f64,
) -> Result<usize, String> {
    validate_gait_sampling_domain(skeleton, clip, roles)?;
    let (role, bone) = roles
        .get(Role::Root)
        .map(|bone| ("Root", bone))
        .or_else(|| roles.get(Role::Hips).map(|bone| ("Hips fallback", bone)))
        .ok_or_else(|| {
            format!(
                "cannot gait-anchor clip {:?} under the in-place policy: selected Root/\
                 Hips trajectory evidence is missing; {GAIT_TRAJECTORY_ALTERNATIVES}",
                clip.name
            )
        })?;
    let Some(bone_name) = skeleton.bones.get(bone).map(|entry| entry.name.as_str()) else {
        return Err(format!(
            "cannot gait-anchor clip {:?} under the in-place policy: selected {role} bone \
             index {bone} is outside the skeleton, so trajectory evidence is missing; \
             {GAIT_TRAJECTORY_ALTERNATIVES}",
            clip.name
        ));
    };

    // Sampling alone is insufficient evidence for irregular or STEP tracks:
    // a non-finite authored interval can fall entirely between uniform grid
    // samples. Inspect every authored value and time on the selected bone and
    // its ancestors, because all of those channels contribute to the selected
    // model-space trajectory.
    let mut trajectory_bones = vec![false; skeleton.bones.len()];
    let mut cursor = Some(bone);
    let mut ancestor_count = 0usize;
    while let Some(index) = cursor {
        let Some(entry) = skeleton.bones.get(index) else {
            return Err(format!(
                "cannot gait-anchor clip {:?} under the in-place policy: selected {role} bone \
                 {:?} (index {bone}) has an out-of-range ancestor index {index}, so trajectory \
                 evidence is missing; {GAIT_TRAJECTORY_ALTERNATIVES}",
                clip.name, bone_name
            ));
        };
        if trajectory_bones[index] || ancestor_count >= skeleton.bones.len() {
            return Err(format!(
                "cannot gait-anchor clip {:?} under the in-place policy: selected {role} bone \
                 {:?} (index {bone}) has a cyclic ancestor chain, so trajectory evidence is \
                 missing; {GAIT_TRAJECTORY_ALTERNATIVES}",
                clip.name, bone_name
            ));
        }
        trajectory_bones[index] = true;
        ancestor_count += 1;
        cursor = entry.parent;
    }
    let frames =
        verify_trajectory_frame_grid(clip, role, bone, bone_name, skeleton.bones.len(), fps)?;
    let grid = sample_clip(skeleton, clip, frames);
    if grid.frame_count() < 3 {
        return Err(format!(
            "cannot gait-anchor clip {:?} under the in-place policy: selected {role} bone \
             {:?} (index {bone}) has fewer than three trajectory samples; \
             {GAIT_TRAJECTORY_ALTERNATIVES}",
            clip.name, bone_name
        ));
    }

    let mut horizontal = Vec::with_capacity(grid.frame_count());
    let mut yaw_steps_deg = Vec::with_capacity(grid.frame_count() - 1);
    let mut previous_forward: Option<Vec3> = None;
    for frame in 0..grid.frame_count() {
        let position = grid.model_position(frame, bone);
        let rotation = grid.model_rotation(frame, bone);
        if !position.is_finite()
            || !rotation.is_finite()
            || !rotation.length_squared().is_finite()
            || rotation.length_squared() == 0.0
        {
            return Err(format!(
                "cannot gait-anchor clip {:?} under the in-place policy: selected {role} bone \
                 {:?} (index {bone}) has non-finite trajectory evidence at sample {frame}; \
                 {GAIT_TRAJECTORY_ALTERNATIVES}",
                clip.name, bone_name
            ));
        }
        horizontal.push(Vec3::new(position.x, 0.0, position.z));

        let forward_3d = rotation.normalize() * Vec3::Z;
        let forward_xz = Vec3::new(forward_3d.x, 0.0, forward_3d.z);
        let length = forward_xz.length();
        if !length.is_finite() || length <= f32::EPSILON {
            return Err(format!(
                "cannot gait-anchor clip {:?} under the in-place policy: selected {role} bone \
                 {:?} (index {bone}) has no finite horizontal forward axis at sample {frame}; \
                 {GAIT_TRAJECTORY_ALTERNATIVES}",
                clip.name, bone_name
            ));
        }
        let forward = forward_xz / length;
        if let Some(previous) = previous_forward {
            let cross_y = previous.cross(forward).y;
            let dot = previous.dot(forward).clamp(-1.0, 1.0);
            let step_deg = f64::from(cross_y.atan2(dot).to_degrees());
            if !step_deg.is_finite() {
                return Err(format!(
                    "cannot gait-anchor clip {:?} under the in-place policy: selected {role} \
                     bone {:?} (index {bone}) has non-finite yaw evidence at sample {frame}; \
                     {GAIT_TRAJECTORY_ALTERNATIVES}",
                    clip.name, bone_name
                ));
            }
            yaw_steps_deg.push(step_deg);
        }
        previous_forward = Some(forward);
    }

    let last = horizontal.len() - 1;
    let horizontal_endpoint_m = f64::from((horizontal[last] - horizontal[0]).length());
    let horizontal_accumulation_m = horizontal_endpoint_m;
    let accumulated_yaw_deg = yaw_steps_deg.iter().sum::<f64>().abs();
    let yaw_accumulation_deg = accumulated_yaw_deg;

    if !horizontal_accumulation_m.is_finite()
        || !yaw_accumulation_deg.is_finite()
        || crate::checks::exceeds_f32_cap(
            horizontal_accumulation_m,
            GAIT_ANCHOR_MAX_HORIZONTAL_ACCUMULATION_M,
        )
        || crate::checks::exceeds_f32_cap(
            yaw_accumulation_deg,
            GAIT_ANCHOR_MAX_YAW_ACCUMULATION_DEG,
        )
    {
        return Err(format!(
            "cannot gait-anchor clip {:?} under the in-place policy: selected {role} bone \
             {:?} (index {bone}) accumulates horizontal translation \
             {horizontal_accumulation_m:.4} m (endpoint {horizontal_endpoint_m:.4} m, cap \
             {GAIT_ANCHOR_MAX_HORIZONTAL_ACCUMULATION_M:.4} m) and yaw \
             {yaw_accumulation_deg:.3} deg (sampled total {accumulated_yaw_deg:.3} deg, cap \
             {GAIT_ANCHOR_MAX_YAW_ACCUMULATION_DEG:.3} deg); \
             {GAIT_TRAJECTORY_ALTERNATIVES}",
            clip.name, bone_name
        ));
    }
    Ok(frames)
}

/// Validate every hand-built input fact on which whole-skeleton sampling and
/// value rotation rely. This runs before any allocation or mutation.
fn validate_gait_sampling_domain(
    skeleton: &Skeleton,
    clip: &Clip,
    roles: &ResolvedRoles,
) -> Result<(), String> {
    for (bone, entry) in skeleton.bones.iter().enumerate() {
        if let Some(parent) = entry.parent {
            if parent >= skeleton.bones.len() {
                return Err(format!(
                    "cannot gait-anchor clip {:?}: skeleton bone {:?} (index {bone}) has \
                     out-of-range ancestor index {parent} (its parent), so trajectory evidence \
                     is missing",
                    clip.name, entry.name
                ));
            }
            if parent >= bone {
                return Err(format!(
                    "cannot gait-anchor clip {:?}: skeleton bone {:?} (index {bone}) has parent \
                     index {parent}, creating a cyclic ancestor chain or child-before-parent \
                     order; whole-skeleton sampling requires an acyclic parents-before-children \
                     order and trajectory evidence is missing",
                    clip.name, entry.name
                ));
            }
        }
    }
    for (role, bone) in roles.iter() {
        if bone >= skeleton.bones.len() {
            let role = if role == Role::Hips {
                "Hips fallback"
            } else {
                role.as_str()
            };
            return Err(format!(
                "cannot gait-anchor clip {:?}: selected {role} bone index {bone} is outside the \
                 skeleton, so trajectory evidence is missing ({} bones)",
                clip.name,
                skeleton.bones.len()
            ));
        }
    }
    for (track_index, track) in clip.tracks.iter().enumerate() {
        if track.bone >= skeleton.bones.len() {
            return Err(format!(
                "cannot gait-anchor clip {:?}: track {track_index} targets out-of-range bone \
                 index {}",
                clip.name, track.bone
            ));
        }
        let key_count = track.times.len();
        let expected_values = if track.interpolation == Interpolation::CubicSpline {
            key_count.checked_mul(3)
        } else {
            Some(key_count)
        }
        .ok_or_else(|| {
            format!(
                "cannot gait-anchor clip {:?}: track {track_index} value cardinality overflows",
                clip.name
            )
        })?;
        let (value_count, storage_matches) = match &track.values {
            TrackValues::Vec3s(values) => (values.len(), track.property != Property::Rotation),
            TrackValues::Quats(values) => (values.len(), track.property == Property::Rotation),
        };
        if value_count != expected_values || !storage_matches {
            return Err(format!(
                "cannot gait-anchor clip {:?}: track {track_index} has {key_count} times and \
                 {value_count} values for {:?} {:?}; expected exactly {expected_values} values \
                 with property-compatible storage",
                clip.name, track.property, track.interpolation
            ));
        }
        let finite_values = match &track.values {
            TrackValues::Vec3s(values) => values.iter().all(|value| value.is_finite()),
            TrackValues::Quats(values) => values.iter().all(|value| value.is_finite()),
        };
        if track.times.iter().any(|time| !time.is_finite()) || !finite_values {
            return Err(format!(
                "cannot gait-anchor clip {:?}: non-finite authored trajectory evidence in \
                 track {track_index}; {GAIT_TRAJECTORY_ALTERNATIVES}",
                clip.name
            ));
        }
    }
    Ok(())
}

/// Require every nonconstant channel to carry the complete
/// whole-frame grid that [`rotate_values`] permutes. Sampling a sparse channel
/// at a shifted omitted frame would synthesize and store a new value at an
/// unchanged key time rather than bijectively reordering authored values.
/// Bounding the grid before [`sample_clip`] also keeps the public core boundary
/// from allocating attacker-controlled `frames × bones` pose arrays.
fn verify_trajectory_frame_grid(
    clip: &Clip,
    role: &str,
    bone: BoneId,
    bone_name: &str,
    skeleton_bones: usize,
    fps: f64,
) -> Result<usize, String> {
    let intervals = clip.duration_s * fps;
    let interval_tolerance = f64::from(f32::EPSILON) * intervals.abs().max(1.0) * 4.0;
    let rounded_intervals = intervals.round();
    if !fps.is_finite()
        || fps <= 0.0
        || !clip.duration_s.is_finite()
        || clip.duration_s <= 0.0
        || !intervals.is_finite()
        || (intervals - rounded_intervals).abs() > interval_tolerance
        || rounded_intervals < 1.0
    {
        return Err(format!(
            "cannot gait-anchor clip {:?} under the in-place policy: selected {role} bone \
             {:?} (index {bone}) has no finite whole-frame trajectory grid at {fps} fps over \
             {:.6} s; {GAIT_TRAJECTORY_ALTERNATIVES}",
            clip.name, bone_name, clip.duration_s
        ));
    }
    // `usize::MAX as f64` rounds upward on 64-bit targets. Rejecting the
    // boundary itself is the conservative checked conversion: every value
    // admitted below it converts and still has room for the closing `+ 1`.
    if rounded_intervals >= usize::MAX as f64 {
        return Err(format!(
            "cannot gait-anchor clip {:?} under the in-place policy: selected {role} bone \
             {:?} (index {bone}) has a whole-frame trajectory sample count that cannot be \
             represented on this platform; {GAIT_TRAJECTORY_ALTERNATIVES}",
            clip.name, bone_name
        ));
    }
    let expected_keys = (rounded_intervals as usize).checked_add(1).ok_or_else(|| {
        format!(
            "cannot gait-anchor clip {:?} under the in-place policy: selected {role} bone \
                 {:?} (index {bone}) has a whole-frame trajectory grid whose sample count \
                 overflows this platform; {GAIT_TRAJECTORY_ALTERNATIVES}",
            clip.name, bone_name
        )
    })?;
    let pose_samples = expected_keys.checked_mul(skeleton_bones).ok_or_else(|| {
        format!(
            "cannot gait-anchor clip {:?} under the in-place policy: selected {role} bone \
             {:?} (index {bone}) has a whole-frame trajectory grid whose frame-by-bone work \
             overflows this platform; {GAIT_TRAJECTORY_ALTERNATIVES}",
            clip.name, bone_name
        )
    })?;
    if pose_samples > GAIT_ANCHOR_MAX_TRAJECTORY_POSE_SAMPLES {
        return Err(format!(
            "cannot gait-anchor clip {:?} under the in-place policy: selected {role} bone \
             {:?} (index {bone}) requires {pose_samples} trajectory pose samples \
             ({expected_keys} frames x {skeleton_bones} bones), above the \
             {GAIT_ANCHOR_MAX_TRAJECTORY_POSE_SAMPLES} sample safety budget; \
             {GAIT_TRAJECTORY_ALTERNATIVES}",
            clip.name, bone_name
        ));
    }
    let authored_frames = default_frame_count(clip);
    let authored_pose_samples = authored_frames.checked_mul(skeleton_bones).ok_or_else(|| {
        format!(
            "cannot gait-anchor clip {:?} under the in-place policy: authored sampling work \
             overflows this platform; {GAIT_TRAJECTORY_ALTERNATIVES}",
            clip.name
        )
    })?;
    if authored_pose_samples > GAIT_ANCHOR_MAX_TRAJECTORY_POSE_SAMPLES {
        return Err(format!(
            "cannot gait-anchor clip {:?} under the in-place policy: authored tracks require \
             {authored_pose_samples} pose samples ({authored_frames} maximum keys x \
             {skeleton_bones} bones), above the {GAIT_ANCHOR_MAX_TRAJECTORY_POSE_SAMPLES} \
             sample safety budget; {GAIT_TRAJECTORY_ALTERNATIVES}",
            clip.name
        ));
    }

    for (track_index, track) in clip.tracks.iter().enumerate() {
        if is_rotation_invariant_track(track) {
            continue;
        }
        if track.key_count() != expected_keys {
            return Err(format!(
                "cannot gait-anchor clip {:?} under the in-place policy: selected {role} bone \
                 {:?} (index {bone}) has incomplete whole-frame rotation evidence in track \
                 {track_index}: {} keys instead of exactly {expected_keys} at {fps} fps; \
                 {GAIT_TRAJECTORY_ALTERNATIVES}",
                clip.name,
                bone_name,
                track.key_count()
            ));
        }
        for (key, &time) in track.times.iter().enumerate() {
            let expected = if key + 1 == expected_keys {
                clip.duration_s as f32
            } else {
                (key as f64 / fps) as f32
            };
            let grid_endpoint = ((expected_keys - 1) as f64 / fps) as f32;
            if time != expected || (key + 1 == expected_keys && time != grid_endpoint) {
                return Err(format!(
                    "cannot gait-anchor clip {:?} under the in-place policy: selected {role} \
                     bone {:?} (index {bone}) has duplicate/non-frame-aligned whole-frame \
                     trajectory evidence in track {track_index}, key {key}: authored time \
                     {time:.9} s, required frame time {expected:.9} s at {fps} fps; \
                     {GAIT_TRAJECTORY_ALTERNATIVES}",
                    clip.name, bone_name
                ));
            }
        }
    }
    Ok(expected_keys)
}

/// Exact representation-level predicate used by gait-anchor rotation. It is
/// intentionally stricter than the lint/prune tolerance classifier: changing
/// this would change which tracks gait rotation leaves untouched.
fn is_rotation_invariant_track(track: &Track) -> bool {
    let n = track.key_count();
    if n <= 1 {
        return true;
    }
    let cubic = track.interpolation == Interpolation::CubicSpline;
    fn constant<T: Copy + PartialEq>(values: &[T], n: usize, cubic: bool, zero: T) -> bool {
        let value = |key: usize| if cubic { 3 * key + 1 } else { key };
        let Some(&first) = values.get(value(0)) else {
            return false;
        };
        (0..n).all(|key| {
            values.get(value(key)) == Some(&first)
                && (!cubic
                    || (values.get(3 * key) == Some(&zero)
                        && values.get(3 * key + 2) == Some(&zero)))
        })
    }
    match &track.values {
        TrackValues::Vec3s(values) => constant(values, n, cubic, glam::Vec3::ZERO),
        TrackValues::Quats(values) => {
            constant(values, n, cubic, glam::Quat::from_xyzw(0.0, 0.0, 0.0, 0.0))
        }
    }
}

/// Replace each animated channel's output values with the channel
/// sampled at `(t + shift) mod period`; times untouched. Constant
/// tracks (rotation-invariant) are skipped; non-constant CUBICSPLINE
/// tracks are refused upstream in [`align_gait_anchor`].
///
/// The in-place preflight proves this uniform-framing condition before this
/// function runs: every nonconstant track has one key at each exact f32
/// `key / fps` time and the exact period endpoint. The sampled values therefore
/// select an authored key bijectively; irregular, sparse, or differently
/// framed channels never reach this mutation boundary.
fn rotate_values(clip: &mut Clip, phase: f64, fps: f64, frame_offset: i32) {
    let duration = clip
        .tracks
        .iter()
        .map(Track::end_time)
        .fold(0.0f32, f32::max) as f64;
    if duration <= 0.0 {
        return;
    }
    let period = duration + 1.0 / fps;
    let mut shift = ((phase * period * fps).round() + frame_offset as f64) / fps;
    shift = shift.rem_euclid(period);

    for track in &mut clip.tracks {
        // Constant tracks (any key count) are invariant; cubic tracks
        // reaching here are constant, so the zip below only touches
        // LINEAR/STEP values. Non-constant short tracks (e.g. a 2-key
        // root ramp) are now rotated instead of silently left behind.
        if is_rotation_invariant_track(track) {
            continue;
        }
        let sampled: Vec<TrackSample> = track
            .times
            .iter()
            .map(|&t| sample_track(track, ((t as f64 + shift) % period) as f32))
            .collect();
        match &mut track.values {
            TrackValues::Vec3s(v) => {
                for (slot, s) in v.iter_mut().zip(&sampled) {
                    if let TrackSample::Vec3(x) = s {
                        *slot = *x;
                    }
                }
            }
            TrackValues::Quats(v) => {
                for (slot, s) in v.iter_mut().zip(&sampled) {
                    if let TrackSample::Quat(q) = s {
                        *slot = *q;
                    }
                }
            }
        }
    }
}
