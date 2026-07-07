//! Round-trip: a synthetic document written as .glb and .gltf must
//! reload with identical structure and values.

use animsmith_core::model::*;
use glam::{Quat, Vec3};

fn synthetic_doc() -> Document {
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
                    name: "spine".into(),
                    parent: Some(0),
                    rest: Transform {
                        translation: Vec3::new(0.0, 0.5, 0.0),
                        rotation: Quat::from_rotation_y(0.3),
                        scale: Vec3::ONE,
                    },
                    inverse_bind: None,
                },
            ],
        },
        clips: vec![Clip {
            name: "sway".into(),
            duration_s: 1.0,
            tracks: vec![
                Track {
                    bone: 1,
                    property: Property::Rotation,
                    interpolation: Interpolation::Linear,
                    times: vec![0.0, 0.5, 1.0],
                    values: TrackValues::Quats(vec![
                        Quat::IDENTITY,
                        Quat::from_rotation_z(0.4),
                        Quat::IDENTITY,
                    ]),
                },
                Track {
                    bone: 0,
                    property: Property::Translation,
                    interpolation: Interpolation::Step,
                    times: vec![0.0, 1.0],
                    values: TrackValues::Vec3s(vec![Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0)]),
                },
            ],
        }],
        assets: Default::default(),
        source: SourceInfo::default(),
    }
}

fn assert_round_trip(extension: &str) {
    let doc = synthetic_doc();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(format!("roundtrip.{extension}"));
    animsmith_gltf::write::write(&doc, &path).expect("writes");
    let loaded = animsmith_gltf::load(&path).expect("reloads");

    assert_eq!(loaded.skeleton.bones.len(), 2);
    assert_eq!(loaded.skeleton.bones[1].name, "spine");
    assert_eq!(loaded.skeleton.bones[1].parent, Some(0));
    let rest = loaded.skeleton.bones[1].rest;
    assert!((rest.translation - Vec3::new(0.0, 0.5, 0.0)).length() < 1e-6);
    assert!(rest.rotation.angle_between(Quat::from_rotation_y(0.3)) < 1e-5);

    assert_eq!(loaded.clips.len(), 1);
    let clip = &loaded.clips[0];
    assert_eq!(clip.name, "sway");
    assert!((clip.duration_s - 1.0).abs() < 1e-6);
    assert_eq!(clip.tracks.len(), 2);
    let rotation = clip
        .tracks
        .iter()
        .find(|t| t.property == Property::Rotation)
        .unwrap();
    assert_eq!(rotation.interpolation, Interpolation::Linear);
    assert_eq!(rotation.times, vec![0.0, 0.5, 1.0]);
    assert!(
        rotation
            .key_quat(1)
            .unwrap()
            .angle_between(Quat::from_rotation_z(0.4))
            < 1e-5
    );
    let translation = clip
        .tracks
        .iter()
        .find(|t| t.property == Property::Translation)
        .unwrap();
    assert_eq!(translation.interpolation, Interpolation::Step);
}

#[test]
fn glb_round_trip() {
    assert_round_trip("glb");
}

#[test]
fn gltf_round_trip() {
    assert_round_trip("gltf");
}

/// Collect the 4-byte chunk-type tags of a GLB, skipping the 12-byte
/// header. Test helper — assumes well-formed chunk framing.
fn glb_chunk_types(bytes: &[u8]) -> Vec<[u8; 4]> {
    let mut types = Vec::new();
    let mut off = 12;
    while off + 8 <= bytes.len() {
        let len = u32::from_le_bytes(bytes[off..off + 4].try_into().unwrap()) as usize;
        types.push(bytes[off + 4..off + 8].try_into().unwrap());
        off += 8 + len;
    }
    types
}

/// A skeleton-only document has no animation or mesh bytes, so its buffer
/// is empty. The writer must not emit a zero-length BIN chunk (Khronos
/// GLB_EMPTY_CHUNK) or present-but-empty buffers/bufferViews/accessors
/// arrays (each invalid glTF), and both containers must still reload.
#[test]
fn empty_document_omits_buffer_and_bin_chunk() {
    let doc = Document {
        skeleton: Skeleton {
            bones: vec![Bone {
                name: "root".into(),
                parent: None,
                rest: Transform::IDENTITY,
                inverse_bind: None,
            }],
        },
        clips: vec![],
        assets: Default::default(),
        source: SourceInfo::default(),
    };
    let dir = tempfile::tempdir().unwrap();

    let glb = dir.path().join("empty.glb");
    animsmith_gltf::write::write(&doc, &glb).expect("writes glb");
    let bytes = std::fs::read(&glb).unwrap();
    assert_eq!(
        glb_chunk_types(&bytes),
        vec![*b"JSON"],
        "empty doc must emit only a JSON chunk, no BIN chunk"
    );

    // The JSON must not carry empty accessor arrays or a zero-length buffer.
    let json_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let json: serde_json::Value = serde_json::from_slice(&bytes[20..20 + json_len]).unwrap();
    for key in ["buffers", "bufferViews", "accessors"] {
        assert!(
            json.get(key).is_none(),
            "{key} must be absent for an empty doc"
        );
    }

    for ext in ["glb", "gltf"] {
        let path = dir.path().join(format!("empty.{ext}"));
        animsmith_gltf::write::write(&doc, &path).expect("writes");
        let loaded = animsmith_gltf::load(&path).expect("reloads");
        assert_eq!(loaded.skeleton.bones.len(), 1, "{ext} skeleton preserved");
        assert!(loaded.clips.is_empty(), "{ext} has no clips");
    }
}
