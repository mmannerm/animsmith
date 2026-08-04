//! End-to-end contracts for opt-in constant-track pruning.
//!
//! The fixtures are authored through the public model/writer boundary so this
//! remains a glTF-only test in both feature configurations.

use animsmith_core::glam::{Quat, Vec3};
use animsmith_core::model::*;
use animsmith_core::sample::{default_frame_count, sample_clip};
use std::path::Path;
use std::process::{Command, Output};

const HOSTILE_CLIP: &str = "walk\nclip\u{1b}[31m";
const HOSTILE_BONE: &str = "hand\nnode\u{1b}[31m";
#[derive(Debug, PartialEq, Eq)]
enum ValuesSnapshot {
    Vec3s(Vec<[u32; 3]>),
    Quats(Vec<[u32; 4]>),
}

type TrackSnapshot = (
    BoneId,
    &'static str,
    Interpolation,
    Vec<u32>,
    ValuesSnapshot,
);
type ClipTrackSnapshot = (String, Vec<TrackSnapshot>);

fn run(args: &[&std::ffi::OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .args(args)
        .output()
        .expect("runs animsmith")
}

fn output_text(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout is UTF-8")
}

fn error_text(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr is UTF-8")
}

fn track(bone: BoneId, property: Property, values: TrackValues) -> Track {
    Track {
        bone,
        property,
        interpolation: Interpolation::Linear,
        times: vec![0.0, 0.5, 1.0],
        values,
    }
}

fn dynamic_rotation(bone: BoneId) -> Track {
    Track {
        bone,
        property: Property::Rotation,
        interpolation: Interpolation::Linear,
        times: vec![0.0, 1.0],
        values: TrackValues::Quats(vec![Quat::IDENTITY, Quat::from_rotation_y(0.5)]),
    }
}

/// The fixture deliberately combines a removable rest-valued channel with
/// protected, non-rest, and sole-writable channels.
fn fixture_document() -> Document {
    Document {
        skeleton: Skeleton {
            bones: vec![
                Bone {
                    name: "root".into(),
                    parent: None,
                    rest: Transform::IDENTITY,
                    inverse_bind: None,
                },
                Bone {
                    name: HOSTILE_BONE.into(),
                    parent: Some(0),
                    rest: Transform::IDENTITY,
                    inverse_bind: None,
                },
                Bone {
                    name: "protected".into(),
                    parent: Some(0),
                    rest: Transform::IDENTITY,
                    inverse_bind: None,
                },
                Bone {
                    name: "non-rest".into(),
                    parent: Some(0),
                    rest: Transform {
                        translation: Vec3::new(3.0, 0.0, 0.0),
                        ..Transform::IDENTITY
                    },
                    inverse_bind: None,
                },
                Bone {
                    name: "solo".into(),
                    parent: Some(0),
                    rest: Transform::IDENTITY,
                    inverse_bind: None,
                },
            ],
        },
        clips: vec![
            Clip {
                name: HOSTILE_CLIP.into(),
                duration_s: 1.0,
                tracks: vec![
                    dynamic_rotation(0),
                    track(
                        1,
                        Property::Translation,
                        TrackValues::Vec3s(vec![Vec3::ZERO; 3]),
                    ),
                    track(
                        1,
                        Property::Rotation,
                        TrackValues::Quats(vec![Quat::IDENTITY; 3]),
                    ),
                    track(
                        3,
                        Property::Translation,
                        TrackValues::Vec3s(vec![Vec3::ZERO; 3]),
                    ),
                ],
            },
            Clip {
                name: "other".into(),
                duration_s: 1.0,
                tracks: vec![
                    dynamic_rotation(0),
                    track(1, Property::Scale, TrackValues::Vec3s(vec![Vec3::ONE; 3])),
                ],
            },
            Clip {
                name: "protected-clip".into(),
                duration_s: 1.0,
                tracks: vec![
                    dynamic_rotation(0),
                    track(
                        2,
                        Property::Translation,
                        TrackValues::Vec3s(vec![Vec3::ZERO; 3]),
                    ),
                ],
            },
            Clip {
                name: "solo-clip".into(),
                duration_s: 1.0,
                tracks: vec![track(
                    4,
                    Property::Scale,
                    TrackValues::Vec3s(vec![Vec3::ONE; 3]),
                )],
            },
        ],
        assets: SceneAssets {
            meshes: vec![MeshAsset {
                name: "triangle".into(),
                source_mesh_index: 0,
                primitives: vec![Primitive {
                    positions: vec![Vec3::ZERO, Vec3::X, Vec3::Y],
                    ..Primitive::default()
                }],
            }],
            instances: vec![MeshInstance {
                source_node_index: 0,
                node: 0,
                mesh: 0,
                ..MeshInstance::default()
            }],
            ..SceneAssets::default()
        },
        source: SourceInfo::default(),
    }
}

fn write_fixture(path: &Path) {
    animsmith_gltf::write::write(&fixture_document(), path).expect("writes procedural GLB");
}

fn find_clip<'a>(doc: &'a Document, name: &str) -> &'a Clip {
    doc.clips
        .iter()
        .find(|clip| clip.name == name)
        .unwrap_or_else(|| panic!("fixture has clip {name:?}"))
}

fn has_track(clip: &Clip, bone: BoneId, property: Property) -> bool {
    clip.tracks
        .iter()
        .any(|track| track.bone == bone && track.property == property)
}

fn track_snapshot(doc: &Document) -> Vec<ClipTrackSnapshot> {
    doc.clips
        .iter()
        .map(|clip| {
            (
                clip.name.clone(),
                clip.tracks
                    .iter()
                    .map(|track| {
                        (
                            track.bone,
                            track.property.as_str(),
                            track.interpolation,
                            track.times.iter().map(|time| time.to_bits()).collect(),
                            match &track.values {
                                TrackValues::Vec3s(values) => ValuesSnapshot::Vec3s(
                                    values
                                        .iter()
                                        .map(|value| {
                                            [
                                                value.x.to_bits(),
                                                value.y.to_bits(),
                                                value.z.to_bits(),
                                            ]
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
        })
        .collect()
}

fn assert_pose_grid_equal(before: &Document, after: &Document, clip_name: &str) {
    let before_clip = find_clip(before, clip_name);
    let after_clip = find_clip(after, clip_name);
    let frames = default_frame_count(before_clip);
    let before_grid = sample_clip(&before.skeleton, before_clip, frames);
    let after_grid = sample_clip(&after.skeleton, after_clip, frames);
    assert_eq!(
        before_grid.times, after_grid.times,
        "same original sample grid"
    );
    for frame in 0..frames {
        for bone in 0..before.skeleton.bones.len() {
            let a = before_grid.local(frame, bone);
            let b = after_grid.local(frame, bone);
            assert!(a.translation.abs_diff_eq(b.translation, 1e-6));
            assert!(a.scale.abs_diff_eq(b.scale, 1e-6));
            assert!(a.rotation.abs_diff_eq(b.rotation, 1e-6));
        }
    }
}

#[test]
fn transform_prune_constant_tracks_is_opt_in_and_preserves_assets() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let input = dir.path().join("input.glb");
    let unchanged = dir.path().join("unchanged.glb");
    write_fixture(&input);
    let before = animsmith_gltf::load(&input).expect("loads fixture");

    let output = run(&[
        "transform".as_ref(),
        input.as_os_str(),
        "--output".as_ref(),
        unchanged.as_os_str(),
    ]);
    assert!(output.status.success(), "{}", error_text(&output));
    let after = animsmith_gltf::load(&unchanged).expect("reloads default transform output");
    assert_eq!(
        track_snapshot(&after),
        track_snapshot(&before),
        "no flag is a no-op"
    );
    assert_eq!(after.assets.meshes.len(), 1, "mesh survives transform");
    assert_eq!(
        after.assets.meshes[0].primitives[0].positions,
        before.assets.meshes[0].primitives[0].positions,
        "geometry survives unchanged"
    );
}

#[test]
fn transform_prunes_only_safe_selected_unprotected_tracks_and_is_idempotent() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let input = dir.path().join("input.glb");
    let output = dir.path().join("pruned.glb");
    let all_pruned = dir.path().join("all-pruned.glb");
    let twice = dir.path().join("twice.glb");
    let config = dir.path().join("animsmith.toml");
    write_fixture(&input);
    std::fs::write(
        &config,
        "[rig]\nrequired_bones = [\"hand\\nnode\\u001B[31m\"]\n\
[clips.protected-clip]\nanimates_bones = [\"protected\"]\n\
[clips.other]\nanimates_bones = [\"hand\"]\n",
    )
    .expect("writes config");
    let before = animsmith_gltf::load(&input).expect("loads fixture");

    let output_result = run(&[
        "--config".as_ref(),
        config.as_os_str(),
        "transform".as_ref(),
        input.as_os_str(),
        "--output".as_ref(),
        output.as_os_str(),
        "--prune-constant-tracks".as_ref(),
        "--clip".as_ref(),
        HOSTILE_CLIP.as_ref(),
    ]);
    assert!(
        output_result.status.success(),
        "{}",
        error_text(&output_result)
    );
    let text = output_text(&output_result);
    assert_eq!(
        text.lines().take(3).collect::<Vec<_>>(),
        vec![
            "  constant-track removed 'walk\\nclip\\u{1b}[31m': track index 1 bone 'hand\\nnode\\u{1b}[31m' translation Linear 3 key(s)",
            "  constant-track removed 'walk\\nclip\\u{1b}[31m': track index 2 bone 'hand\\nnode\\u{1b}[31m' rotation Linear 3 key(s)",
            "  constant-track retained 'walk\\nclip\\u{1b}[31m': track index 3 bone 'non-rest' translation Linear 3 key(s): removal changes sampled local TRS or model-space position/rotation",
        ],
        "exact authored-order evidence"
    );
    assert!(
        text.contains("walk\\nclip"),
        "hostile clip is escaped: {text}"
    );
    assert!(
        text.contains("hand\\nnode"),
        "hostile bone is escaped: {text}"
    );
    assert!(
        !text.contains(HOSTILE_CLIP),
        "raw control text must not render: {text}"
    );
    assert!(
        !text.contains(HOSTILE_BONE),
        "raw control text must not render: {text}"
    );

    let after = animsmith_gltf::load(&output).expect("reloads pruned output");
    let selected = find_clip(&after, HOSTILE_CLIP);
    assert!(!has_track(selected, 1, Property::Translation));
    assert!(!has_track(selected, 1, Property::Rotation));
    assert!(
        has_track(selected, 3, Property::Translation),
        "non-rest track remains"
    );
    assert!(
        has_track(selected, 0, Property::Rotation),
        "dynamic track remains"
    );
    assert!(
        has_track(find_clip(&after, "other"), 1, Property::Scale),
        "--clip leaves other clips untouched"
    );
    assert!(
        has_track(
            find_clip(&after, "protected-clip"),
            2,
            Property::Translation
        ),
        "--clip leaves configured protected clip untouched"
    );
    assert!(
        has_track(find_clip(&after, "solo-clip"), 4, Property::Scale),
        "the last writable track remains"
    );
    assert_pose_grid_equal(&before, &after, HOSTILE_CLIP);

    // A separate unscoped invocation proves the profile guard is functional,
    // rather than merely unvisited because the preceding run used --clip.
    let all_result = run(&[
        "--config".as_ref(),
        config.as_os_str(),
        "transform".as_ref(),
        input.as_os_str(),
        "--output".as_ref(),
        all_pruned.as_os_str(),
        "--prune-constant-tracks".as_ref(),
    ]);
    assert!(all_result.status.success(), "{}", error_text(&all_result));
    let all_text = output_text(&all_result);
    assert!(
        all_text.contains(
            "  constant-track retained 'protected-clip': track index 1 bone 'protected' translation Linear 3 key(s): target bone is protected\n"
        ),
        "protected evidence: {all_text}"
    );
    assert!(
        all_text.contains(
            "  constant-track retained 'solo-clip': track index 0 bone 'solo' scale Linear 3 key(s): removal would leave no writable track\n"
        ),
        "last-track evidence: {all_text}"
    );
    assert!(
        all_text.contains(
            "  constant-track removed 'other': track index 1 bone 'hand\\nnode\\u{1b}[31m' scale Linear 3 key(s)\n"
        ),
        "neither a required_bones declaration nor a substring animates_bones name protects the track: {all_text}"
    );
    let all = animsmith_gltf::load(&all_pruned).expect("reloads unscoped output");
    assert!(
        has_track(find_clip(&all, "protected-clip"), 2, Property::Translation),
        "animates_bones prevents pruning even when the clip is selected"
    );
    assert!(
        !has_track(find_clip(&all, "other"), 1, Property::Scale),
        "unprotected tracks in other clips are pruned without --clip"
    );

    let rerun = run(&[
        "--config".as_ref(),
        config.as_os_str(),
        "transform".as_ref(),
        output.as_os_str(),
        "--output".as_ref(),
        twice.as_os_str(),
        "--prune-constant-tracks".as_ref(),
        "--clip".as_ref(),
        HOSTILE_CLIP.as_ref(),
    ]);
    assert!(rerun.status.success(), "{}", error_text(&rerun));
    let twice_doc = animsmith_gltf::load(&twice).expect("reloads second output");
    assert_eq!(
        track_snapshot(&twice_doc),
        track_snapshot(&after),
        "a second prune removes no further tracks"
    );
}
