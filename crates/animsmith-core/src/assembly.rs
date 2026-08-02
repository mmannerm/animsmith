//! Lossless, name-based clip operations for character-assembly pipelines.
//!
//! These helpers deliberately do not retarget motion.  They only move a clip
//! between skeletons with the same exact bone names, remove selected channels,
//! fill omitted channels from an authoritative rest pose, and make quaternion
//! signs consistent.

use std::collections::{BTreeMap, BTreeSet};

use glam::{Quat, Vec3};
use thiserror::Error;

use crate::model::{BoneId, Clip, Interpolation, Property, Skeleton, Track, TrackValues};

/// Failure while resolving an exact bone name for an assembly operation.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum AssemblyError {
    /// A track references a source bone outside its skeleton.
    #[error("track references source bone {bone}, but the source skeleton has {bone_count} bones")]
    SourceBoneOutOfBounds {
        /// Referenced source bone index.
        bone: BoneId,
        /// Number of source bones.
        bone_count: usize,
    },
    /// More than one source bone has a referenced exact name.
    #[error("source bone name {name:?} is ambiguous (bones {first} and {second})")]
    AmbiguousSourceName {
        /// Ambiguous name.
        name: String,
        /// First source bone with this name.
        first: BoneId,
        /// Another source bone with this name.
        second: BoneId,
    },
    /// More than one destination bone has a referenced exact name.
    #[error("base bone name {name:?} is ambiguous (bones {first} and {second})")]
    AmbiguousBaseName {
        /// Ambiguous name.
        name: String,
        /// First base bone with this name.
        first: BoneId,
        /// Another base bone with this name.
        second: BoneId,
    },
    /// A referenced source-bone name is absent from the base skeleton.
    #[error("source bone {source_bone} named {name:?} is missing from the base skeleton")]
    MissingBaseBone {
        /// Referenced source bone index.
        source_bone: BoneId,
        /// Missing exact name.
        name: String,
    },
    /// A supplied named-bone selection is ambiguous in the skeleton.
    #[error("bone name {name:?} is ambiguous (bones {first} and {second})")]
    AmbiguousSelectedName {
        /// Ambiguous name.
        name: String,
        /// First matching bone.
        first: BoneId,
        /// Another matching bone.
        second: BoneId,
    },
    /// A caller-selected completion target is outside the base skeleton.
    #[error("selected base bone {bone} is outside the base skeleton ({bone_count} bones)")]
    SelectedBoneOutOfBounds {
        /// Invalid selected bone.
        bone: BoneId,
        /// Number of base bones.
        bone_count: usize,
    },
}

/// Selects which absent local-transform channels [`complete_rest_pose_tracks`]
/// creates.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RestPoseTrackOptions {
    /// Complete absent translation channels.
    pub translation: bool,
    /// Complete absent rotation channels.
    pub rotation: bool,
    /// Complete absent scale channels.
    pub scale: bool,
}

impl RestPoseTrackOptions {
    /// Select all three local-transform channels.
    pub const ALL: Self = Self {
        translation: true,
        rotation: true,
        scale: true,
    };
}

/// Remap a source clip onto `base` using exact, unique bone names.
///
/// The returned clip has the same keys, values, interpolation, name, and
/// duration as `source`; only each track's bone index changes.  Every source
/// bone referenced by a track and its base counterpart must have a unique,
/// exact name.  Names outside the referenced set are intentionally irrelevant.
///
/// # Errors
///
/// Returns [`AssemblyError`] if a track references an invalid source bone, or
/// if a referenced source name is ambiguous or missing in `base`.
pub fn remap_clip_to_base(
    source: &Clip,
    source_skeleton: &Skeleton,
    base: &Skeleton,
) -> Result<Clip, AssemblyError> {
    let referenced: BTreeSet<BoneId> = source.tracks.iter().map(|track| track.bone).collect();
    let mut referenced_names = BTreeSet::new();
    for &bone in &referenced {
        let Some(source_bone) = source_skeleton.bones.get(bone) else {
            return Err(AssemblyError::SourceBoneOutOfBounds {
                bone,
                bone_count: source_skeleton.bones.len(),
            });
        };
        referenced_names.insert(source_bone.name.as_str());
    }
    let mut source_names = BTreeMap::new();
    for (bone, source_bone) in source_skeleton.bones.iter().enumerate() {
        if !referenced_names.contains(source_bone.name.as_str()) {
            continue;
        }
        if let Some(first) = source_names.insert(source_bone.name.as_str(), bone) {
            return Err(AssemblyError::AmbiguousSourceName {
                name: source_bone.name.clone(),
                first,
                second: bone,
            });
        }
    }

    let mut base_names = BTreeMap::new();
    for (bone, base_bone) in base.bones.iter().enumerate() {
        if !referenced_names.contains(base_bone.name.as_str()) {
            continue;
        }
        if let Some(first) = base_names.insert(base_bone.name.as_str(), bone) {
            return Err(AssemblyError::AmbiguousBaseName {
                name: base_bone.name.clone(),
                first,
                second: bone,
            });
        }
    }

    let remapped: BTreeMap<BoneId, BoneId> = referenced
        .iter()
        .map(|&source_bone| {
            let name = source_skeleton.bones[source_bone].name.as_str();
            base_names
                .get(name)
                .copied()
                .map(|base_bone| (source_bone, base_bone))
                .ok_or_else(|| AssemblyError::MissingBaseBone {
                    source_bone,
                    name: name.to_owned(),
                })
        })
        .collect::<Result<_, _>>()?;

    let mut output = source.clone();
    for track in &mut output.tracks {
        track.bone = remapped[&track.bone];
    }
    Ok(output)
}

/// Remove every track targeting any exact, uniquely named `bones` entry.
///
/// Unknown names select no tracks.  A duplicated selected name is rejected so
/// a caller cannot silently strip motion from an unintended bone.
///
/// # Errors
///
/// Returns [`AssemblyError::AmbiguousSelectedName`] if a requested name occurs
/// more than once in `skeleton`.
pub fn strip_named_bone_tracks(
    clip: &mut Clip,
    skeleton: &Skeleton,
    bones: impl IntoIterator<Item = impl AsRef<str>>,
) -> Result<usize, AssemblyError> {
    let requested: BTreeSet<String> = bones
        .into_iter()
        .map(|name| name.as_ref().to_owned())
        .collect();
    let mut selected = BTreeSet::new();
    for name in requested {
        let matches: Vec<_> = skeleton
            .bones
            .iter()
            .enumerate()
            .filter_map(|(id, bone)| (bone.name == name).then_some(id))
            .collect();
        if let [bone] = matches.as_slice() {
            selected.insert(*bone);
        } else if let [first, second, ..] = matches.as_slice() {
            return Err(AssemblyError::AmbiguousSelectedName {
                name,
                first: *first,
                second: *second,
            });
        }
    }
    let before = clip.tracks.len();
    clip.tracks.retain(|track| !selected.contains(&track.bone));
    Ok(before - clip.tracks.len())
}

/// Add one rest-pose key at time zero for selected channels that a clip omits.
///
/// Existing tracks, including empty ones, are never altered.  The created
/// tracks use linear interpolation; one key is a constant hold under the core
/// sampler.  Tracks are appended in skeleton order, then translation,
/// rotation, scale order for deterministic output.
pub fn complete_rest_pose_tracks(
    clip: &mut Clip,
    base: &Skeleton,
    options: RestPoseTrackOptions,
) -> Result<usize, AssemblyError> {
    complete_rest_pose_tracks_for_bones(clip, base, 0..base.bones.len(), options)
}

/// Add absent rest-pose channels for an explicit base-bone selection.
///
/// Bone ids are sorted and deduplicated before tracks are appended, so caller
/// ordering cannot change the output. Existing tracks are never altered.
///
/// # Errors
///
/// Returns [`AssemblyError::SelectedBoneOutOfBounds`] when a selected id is not
/// present in `base`.
pub fn complete_rest_pose_tracks_for_bones(
    clip: &mut Clip,
    base: &Skeleton,
    bones: impl IntoIterator<Item = BoneId>,
    options: RestPoseTrackOptions,
) -> Result<usize, AssemblyError> {
    let properties = [
        (Property::Translation, options.translation),
        (Property::Rotation, options.rotation),
        (Property::Scale, options.scale),
    ];
    let mut added = 0;
    let bones = bones.into_iter().collect::<BTreeSet<_>>();
    for bone in bones {
        let Some(base_bone) = base.bones.get(bone) else {
            return Err(AssemblyError::SelectedBoneOutOfBounds {
                bone,
                bone_count: base.bones.len(),
            });
        };
        for (property, enabled) in properties {
            if !enabled
                || clip
                    .tracks
                    .iter()
                    .any(|track| track.bone == bone && track.property == property)
            {
                continue;
            }
            clip.tracks.push(rest_track(
                bone,
                property,
                base_bone.rest.translation,
                base_bone.rest.rotation,
                base_bone.rest.scale,
            ));
            added += 1;
        }
    }
    Ok(added)
}

fn rest_track(
    bone: BoneId,
    property: Property,
    translation: Vec3,
    rotation: Quat,
    scale: Vec3,
) -> Track {
    let values = match property {
        Property::Translation => TrackValues::Vec3s(vec![translation]),
        Property::Rotation => TrackValues::Quats(vec![rotation]),
        Property::Scale => TrackValues::Vec3s(vec![scale]),
    };
    Track {
        bone,
        property,
        interpolation: Interpolation::Linear,
        times: vec![0.0],
        values,
    }
}

/// Make rotation-track key quaternions hemisphere-consistent in key order.
///
/// Each finite key is compared to the preceding finite key.  If their dot
/// product is negative, the key is negated.  Cubic-spline keys negate their
/// whole tangent/value/tangent triplet, preserving the represented curve.
/// A zero dot product is deliberately left unchanged, making ties stable and
/// avoiding platform-dependent sign choices.  Returns the number of flipped
/// keys.
pub fn normalize_quaternion_hemispheres(clip: &mut Clip) -> usize {
    let mut flipped = 0;
    for track in &mut clip.tracks {
        if track.property != Property::Rotation {
            continue;
        }
        let key_count = track.key_count();
        let interpolation = track.interpolation;
        let TrackValues::Quats(values) = &mut track.values else {
            continue;
        };
        let mut previous = None;
        for key in 0..key_count {
            let value_index = match interpolation {
                Interpolation::CubicSpline => key * 3 + 1,
                _ => key,
            };
            let Some(value) = values.get(value_index).copied() else {
                break;
            };
            if !value.is_finite() {
                continue;
            }
            if previous.is_some_and(|previous: Quat| previous.dot(value) < 0.0) {
                let start = match interpolation {
                    Interpolation::CubicSpline => key * 3,
                    _ => key,
                };
                let count = match interpolation {
                    Interpolation::CubicSpline => 3,
                    _ => 1,
                };
                if start + count > values.len() {
                    break;
                }
                for quaternion in &mut values[start..start + count] {
                    *quaternion = -*quaternion;
                }
                previous = Some(-value);
                flipped += 1;
            } else {
                previous = Some(value);
            }
        }
    }
    flipped
}

/// Remove one final key from every nonempty track and shorten the clip to the
/// latest remaining key.
///
/// This is useful for converting a duplicate-endpoint loop to an open loop.
/// Empty tracks are removed.  If every track becomes empty, the duration is
/// zero.  Cubic-spline tangent/value/tangent triplets are removed together.
/// Returns the number of keys removed.
pub fn remove_final_keys(clip: &mut Clip) -> usize {
    let mut removed = 0;
    for track in &mut clip.tracks {
        if track.times.pop().is_none() {
            continue;
        }
        removed += 1;
        let count = match track.interpolation {
            Interpolation::CubicSpline => 3,
            _ => 1,
        };
        match &mut track.values {
            TrackValues::Vec3s(values) => values.truncate(values.len().saturating_sub(count)),
            TrackValues::Quats(values) => values.truncate(values.len().saturating_sub(count)),
        }
    }
    clip.tracks.retain(|track| !track.times.is_empty());
    clip.duration_s = clip
        .tracks
        .iter()
        .map(|track| track.end_time() as f64)
        .fold(0.0, f64::max);
    removed
}
