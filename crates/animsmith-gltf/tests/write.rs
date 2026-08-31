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
    let summary = animsmith_gltf::write::write(&doc, &path).expect("writes");
    let loaded = animsmith_gltf::load(&path).expect("reloads");

    assert_eq!(summary.nodes, loaded.skeleton.bones.len());
    assert_eq!(summary.animations, loaded.clips.len());
    assert_eq!(summary.meshes, loaded.assets.meshes.len());
    assert_eq!(
        summary.primitive_positions,
        loaded
            .assets
            .meshes
            .iter()
            .flat_map(|mesh| mesh.primitives.iter())
            .map(|primitive| primitive.positions.len())
            .sum::<usize>()
    );
    assert_eq!(summary.materials, loaded.assets.materials.len());
    assert_eq!(summary.clips_without_writable_tracks, 0);

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
fn strict_in_memory_glb_preflight_counts_exact_bytes_and_matches_legacy_path() {
    use animsmith_gltf::write::{
        GlbProjectionPolicyV1, GlbWriteLimits, preflight_glb_bytes, write_glb_bytes,
    };

    let doc = synthetic_doc();
    let receipt = preflight_glb_bytes(
        &doc,
        GlbProjectionPolicyV1::StrictFootCycleV1,
        GlbWriteLimits::FOOT_CYCLE_V1,
    )
    .expect("strict candidate is representable");
    let bytes = write_glb_bytes(&doc, GlbProjectionPolicyV1::StrictFootCycleV1, &receipt)
        .expect("writes exact preflight candidate");
    assert_eq!(bytes.len(), receipt.total_bytes());
    assert_eq!(
        u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize,
        receipt.total_bytes(),
        "GLB header uses the admitted total byte count"
    );

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("same.glb");
    animsmith_gltf::write::write(&doc, &path).expect("legacy path writes");
    assert_eq!(
        bytes,
        std::fs::read(path).unwrap(),
        "strict direct candidate and legacy GLB path share the projection bytes"
    );
}

#[test]
fn strict_receipt_binds_content_not_just_byte_counts() {
    use animsmith_gltf::{
        WriteError,
        write::{GlbProjectionPolicyV1, GlbWriteLimits, preflight_glb_bytes, write_glb_bytes},
    };

    let mut doc = synthetic_doc();
    let receipt = preflight_glb_bytes(
        &doc,
        GlbProjectionPolicyV1::StrictFootCycleV1,
        GlbWriteLimits::FOOT_CYCLE_V1,
    )
    .expect("preflight");
    let TrackValues::Quats(values) = &mut doc.clips[0].tracks[0].values else {
        panic!("fixture rotation values")
    };
    values[1] = Quat::from_rotation_z(0.8);
    assert!(matches!(
        write_glb_bytes(&doc, GlbProjectionPolicyV1::StrictFootCycleV1, &receipt),
        Err(WriteError::ReceiptMismatch)
    ));
}

#[test]
fn strict_preflight_refuses_legacy_omission_before_candidate_bytes_exist() {
    use animsmith_gltf::{
        WriteError,
        write::{GlbProjectionPolicyV1, GlbWriteLimits, preflight_glb_bytes},
    };

    let mut doc = synthetic_doc();
    doc.clips.push(Clip {
        name: "empty".into(),
        duration_s: 0.0,
        tracks: vec![],
    });
    let error = preflight_glb_bytes(
        &doc,
        GlbProjectionPolicyV1::StrictFootCycleV1,
        GlbWriteLimits::FOOT_CYCLE_V1,
    )
    .expect_err("strict candidate must not omit the clip");
    assert!(
        matches!(error, WriteError::Refused(message) if message.contains("no writable tracks"))
    );
}

#[test]
fn strict_preflight_accepts_the_exact_total_limit_and_refuses_its_first_byte_over() {
    use animsmith_gltf::{
        WriteError,
        write::{GlbProjectionPolicyV1, GlbWriteLimits, preflight_glb_bytes},
    };

    let doc = synthetic_doc();
    let generous = preflight_glb_bytes(
        &doc,
        GlbProjectionPolicyV1::StrictFootCycleV1,
        GlbWriteLimits::FOOT_CYCLE_V1,
    )
    .expect("discover exact candidate count");
    let exact = GlbWriteLimits {
        max_json_bytes: generous.json_bytes(),
        max_bin_bytes: generous.bin_bytes(),
        max_total_bytes: generous.total_bytes(),
    };
    assert!(preflight_glb_bytes(&doc, GlbProjectionPolicyV1::StrictFootCycleV1, exact).is_ok());
    let first_over = GlbWriteLimits {
        max_total_bytes: exact.max_total_bytes - 1,
        ..exact
    };
    assert!(matches!(
        preflight_glb_bytes(&doc, GlbProjectionPolicyV1::StrictFootCycleV1, first_over),
        Err(WriteError::TooLarge { field: "configured total GLB limit", bytes }) if bytes == exact.max_total_bytes
    ));
}

#[test]
fn strict_preflight_refuses_source_scene_membership_that_the_canonical_scene_would_collapse() {
    use animsmith_gltf::{
        WriteError,
        write::{GlbProjectionPolicyV1, GlbWriteLimits, preflight_glb_bytes},
    };

    let mut doc = synthetic_doc();
    doc.assets.scenes = vec![
        SceneAsset {
            source_scene_index: 0,
            name: Some("first".into()),
            roots: vec![0],
        },
        SceneAsset {
            source_scene_index: 1,
            name: Some("second".into()),
            roots: vec![0],
        },
    ];
    doc.assets.default_scene = Some(0);
    assert!(matches!(
        preflight_glb_bytes(
            &doc,
            GlbProjectionPolicyV1::StrictFootCycleV1,
            GlbWriteLimits::FOOT_CYCLE_V1,
        ),
        Err(WriteError::Refused(message)) if message.contains("source scenes")
    ));
}

#[test]
fn gltf_round_trip() {
    assert_round_trip("gltf");
}

#[test]
fn write_summary_counts_each_clip_without_writable_tracks() {
    let mut doc = synthetic_doc();
    doc.clips.extend(["empty-a", "empty-b"].map(|name| Clip {
        name: name.into(),
        duration_s: 0.0,
        tracks: vec![],
    }));
    doc.clips.push(Clip {
        name: "empty-track".into(),
        duration_s: 0.0,
        tracks: vec![Track {
            bone: 0,
            property: Property::Translation,
            interpolation: Interpolation::Linear,
            times: vec![],
            values: TrackValues::Vec3s(vec![]),
        }],
    });
    let mut mixed = doc.clips[0].clone();
    mixed.name = "mixed".into();
    mixed.tracks.push(Track {
        bone: 0,
        property: Property::Translation,
        interpolation: Interpolation::Linear,
        times: vec![],
        values: TrackValues::Vec3s(vec![]),
    });
    doc.clips.push(mixed);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("omitted-empty-clips.glb");

    let summary = animsmith_gltf::write::write(&doc, &path).expect("writes");
    let loaded = animsmith_gltf::load(&path).expect("reloads");

    assert_eq!(
        (
            summary.animations,
            summary.clips_without_writable_tracks,
            loaded
                .clips
                .iter()
                .map(|clip| clip.name.as_str())
                .collect::<Vec<_>>(),
        ),
        (2, 3, vec!["sway", "mixed"]),
        "empty clips are omitted while a mixed writable/non-writable clip is preserved"
    );
}

#[test]
fn write_summary_counts_a_clip_whose_only_track_targets_an_unknown_bone() {
    let mut doc = synthetic_doc();
    let mut invalid_track = doc.clips[0].tracks[0].clone();
    invalid_track.bone = doc.skeleton.bones.len();
    doc.clips = vec![Clip {
        name: "unknown-bone".into(),
        duration_s: 1.0,
        tracks: vec![invalid_track],
    }];
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("unknown-bone.glb");

    let summary = animsmith_gltf::write::write(&doc, &path).expect("writes");
    let loaded = animsmith_gltf::load(&path).expect("reloads");

    assert_eq!(summary.animations, 0);
    assert_eq!(summary.clips_without_writable_tracks, 1);
    assert!(
        loaded.clips.is_empty(),
        "the unknown-bone clip is absent from the artifact"
    );
}

#[test]
fn write_summary_omits_materials_when_document_has_no_meshes() {
    let mut doc = synthetic_doc();
    doc.assets.materials.push(MaterialAsset {
        name: "unused".into(),
        base_color: [1.0; 4],
        metallic: 0.0,
        roughness: 1.0,
        base_color_texture: None,
        normal_texture: None,
        metallic_roughness_texture: None,
        occlusion_texture: None,
    });
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("meshless-material.glb");

    let summary = animsmith_gltf::write::write(&doc, &path).expect("writes");
    let loaded = animsmith_gltf::load(&path).expect("reloads");

    assert_eq!(
        (
            doc.assets.materials.len(),
            summary.materials,
            loaded.assets.materials.len(),
        ),
        (1, 0, 0),
        "a source material is absent from a meshless artifact and its summary"
    );
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
