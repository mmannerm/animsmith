//! Generic, name-based character-assembly operations.

use animsmith_core::assembly::{
    AssemblyError, RestPoseTrackOptions, complete_rest_pose_tracks,
    normalize_quaternion_hemispheres, remap_clip_to_base, remove_final_keys,
    strip_named_bone_tracks,
};
use animsmith_core::model::{
    Bone, Clip, Interpolation, Property, Skeleton, Track, TrackValues, Transform,
};
use glam::{Quat, Vec3};

fn bone(name: &str, x: f32) -> Bone {
    Bone {
        name: name.into(),
        parent: None,
        rest: Transform {
            translation: Vec3::new(x, 1.0, 2.0),
            rotation: Quat::from_rotation_x(x),
            scale: Vec3::splat(x + 1.0),
        },
        inverse_bind: None,
    }
}

fn track(bone: usize, property: Property) -> Track {
    let values = match property {
        Property::Rotation => TrackValues::Quats(vec![Quat::IDENTITY, Quat::IDENTITY]),
        _ => TrackValues::Vec3s(vec![Vec3::ZERO, Vec3::ONE]),
    };
    Track {
        bone,
        property,
        interpolation: Interpolation::Linear,
        times: vec![0.0, 1.0],
        values,
    }
}

#[test]
fn remap_uses_exact_names_and_preserves_authored_tracks() {
    let source_skeleton = Skeleton {
        bones: vec![bone("hips", 0.0), bone("hand", 1.0)],
    };
    let base = Skeleton {
        bones: vec![bone("hand", 4.0), bone("hips", 3.0), bone("extra", 2.0)],
    };
    let source = Clip {
        name: "wave".into(),
        duration_s: 1.0,
        tracks: vec![
            track(0, Property::Translation),
            track(1, Property::Rotation),
        ],
    };

    let remapped = remap_clip_to_base(&source, &source_skeleton, &base).expect("exact names map");
    assert_eq!(remapped.tracks[0].bone, 1);
    assert_eq!(remapped.tracks[1].bone, 0);
    assert_eq!(remapped.tracks[0].times, source.tracks[0].times);
    assert_eq!(remapped.tracks[1].key_quat(1), source.tracks[1].key_quat(1));
}

#[test]
fn remap_rejects_duplicate_and_missing_referenced_names() {
    let clip = Clip {
        name: "clip".into(),
        duration_s: 1.0,
        tracks: vec![track(0, Property::Translation)],
    };
    let duplicate_source = Skeleton {
        bones: vec![bone("hips", 0.0), bone("hips", 1.0)],
    };
    let duplicate_base = Skeleton {
        bones: vec![bone("hips", 0.0), bone("hips", 1.0)],
    };
    let one_hips = Skeleton {
        bones: vec![bone("hips", 0.0)],
    };

    assert!(matches!(
        remap_clip_to_base(&clip, &duplicate_source, &one_hips),
        Err(AssemblyError::AmbiguousSourceName { .. })
    ));
    assert!(matches!(
        remap_clip_to_base(&clip, &one_hips, &duplicate_base),
        Err(AssemblyError::AmbiguousBaseName { .. })
    ));
    assert!(matches!(
        remap_clip_to_base(&clip, &one_hips, &Skeleton::default()),
        Err(AssemblyError::MissingBaseBone { .. })
    ));
}

#[test]
fn named_strip_is_exact_and_refuses_ambiguous_selection() {
    let skeleton = Skeleton {
        bones: vec![bone("root", 0.0), bone("weapon", 1.0), bone("weapon", 2.0)],
    };
    let mut clip = Clip {
        name: "attack".into(),
        duration_s: 1.0,
        tracks: vec![
            track(0, Property::Translation),
            track(1, Property::Rotation),
            track(2, Property::Scale),
        ],
    };
    let original = clip.clone();
    assert!(matches!(
        strip_named_bone_tracks(&mut clip, &skeleton, ["weapon"]),
        Err(AssemblyError::AmbiguousSelectedName { .. })
    ));
    assert_eq!(
        clip.tracks.len(),
        original.tracks.len(),
        "failure is atomic"
    );

    let unique = Skeleton {
        bones: vec![bone("root", 0.0), bone("weapon", 1.0)],
    };
    clip.tracks.truncate(2);
    assert_eq!(
        strip_named_bone_tracks(&mut clip, &unique, ["weapon"]).unwrap(),
        1
    );
    assert_eq!(clip.tracks[0].bone, 0);
    assert_eq!(
        strip_named_bone_tracks(&mut clip, &unique, ["Weapon"]).unwrap(),
        0
    );
}

#[test]
fn rest_completion_only_adds_requested_absent_channels() {
    let base = Skeleton {
        bones: vec![bone("root", 0.0), bone("hand", 2.0)],
    };
    let existing_translation = track(0, Property::Translation);
    let mut clip = Clip {
        name: "pose".into(),
        duration_s: 2.0,
        tracks: vec![existing_translation.clone()],
    };
    let added = complete_rest_pose_tracks(&mut clip, &base, RestPoseTrackOptions::ALL);
    assert_eq!(
        added, 5,
        "root rotation/scale plus hand translation/rotation/scale"
    );
    assert_eq!(
        clip.tracks[0].times, existing_translation.times,
        "authored channel stays untouched"
    );
    let hand_translation = clip
        .tracks
        .iter()
        .find(|t| t.bone == 1 && t.property == Property::Translation)
        .unwrap();
    assert_eq!(
        hand_translation.key_vec3(0),
        Some(base.bones[1].rest.translation)
    );
    let hand_rotation = clip
        .tracks
        .iter()
        .find(|t| t.bone == 1 && t.property == Property::Rotation)
        .unwrap();
    assert_eq!(hand_rotation.key_quat(0), Some(base.bones[1].rest.rotation));
    let hand_scale = clip
        .tracks
        .iter()
        .find(|t| t.bone == 1 && t.property == Property::Scale)
        .unwrap();
    assert_eq!(hand_scale.key_vec3(0), Some(base.bones[1].rest.scale));
    assert!(
        clip.tracks
            .iter()
            .any(|t| t.bone == 0 && t.property == Property::Scale),
        "root scale is completed too"
    );
}

#[test]
fn hemisphere_normalization_flips_cubic_triplets_but_keeps_zero_dot_ties() {
    let q = Quat::from_rotation_y(0.5);
    let qx = Quat::from_xyzw(1.0, 0.0, 0.0, 0.0);
    let qy = Quat::from_xyzw(0.0, 1.0, 0.0, 0.0);
    let qz = Quat::from_xyzw(0.0, 0.0, 1.0, 0.0);
    let mut clip = Clip {
        name: "turn".into(),
        duration_s: 1.0,
        tracks: vec![Track {
            bone: 0,
            property: Property::Rotation,
            interpolation: Interpolation::CubicSpline,
            times: vec![0.0, 0.5, 1.0],
            values: TrackValues::Quats(vec![qx, q, qy, -qx, -q, -qy, qy, qz, qx]),
        }],
    };
    assert_eq!(normalize_quaternion_hemispheres(&mut clip), 1);
    let TrackValues::Quats(values) = &clip.tracks[0].values else {
        unreachable!()
    };
    assert_eq!(values[3], qx);
    assert_eq!(values[4], q);
    assert_eq!(values[5], qy);
    assert_eq!(values[7], qz, "zero-dot tie retains its supplied sign");
}

#[test]
fn removing_final_keys_keeps_cubic_alignment_and_retimes_duration() {
    let mut cubic = track(0, Property::Translation);
    cubic.interpolation = Interpolation::CubicSpline;
    cubic.times = vec![0.0, 0.5, 1.0];
    cubic.values = TrackValues::Vec3s(vec![
        Vec3::ZERO,
        Vec3::X,
        Vec3::ZERO,
        Vec3::ZERO,
        Vec3::Y,
        Vec3::ZERO,
        Vec3::ZERO,
        Vec3::Z,
        Vec3::ZERO,
    ]);
    let mut clip = Clip {
        name: "loop".into(),
        duration_s: 1.0,
        tracks: vec![
            cubic,
            track(1, Property::Scale),
            track(2, Property::Rotation),
        ],
    };
    assert_eq!(remove_final_keys(&mut clip), 3);
    assert_eq!(clip.duration_s, 0.5);
    assert_eq!(clip.tracks[0].key_count(), 2);
    let TrackValues::Vec3s(values) = &clip.tracks[0].values else {
        unreachable!()
    };
    assert_eq!(values.len(), 6);
    assert_eq!(clip.tracks[0].key_vec3(1), Some(Vec3::Y));
    assert_eq!(clip.tracks[1].key_count(), 1);
    assert_eq!(clip.tracks[2].key_count(), 1);
}
