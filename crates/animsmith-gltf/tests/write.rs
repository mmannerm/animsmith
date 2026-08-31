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
        assets: SceneAssets {
            scenes: vec![SceneAsset {
                source_scene_index: 0,
                name: None,
                roots: vec![0],
            }],
            default_scene: Some(0),
            ..Default::default()
        },
        source: SourceInfo::default(),
    }
}

fn plain_material() -> MaterialAsset {
    MaterialAsset {
        name: "material".into(),
        base_color: [1.0; 4],
        metallic: 0.0,
        roughness: 1.0,
        base_color_texture: None,
        normal_texture: None,
        metallic_roughness_texture: None,
        occlusion_texture: None,
    }
}

fn complete_strict_asset_doc() -> Document {
    let mut doc = synthetic_doc();
    let texture = |byte| TextureAsset {
        bytes: vec![byte],
        mime: "image/png".into(),
    };
    doc.assets.materials.push(MaterialAsset {
        name: "complete".into(),
        base_color: [1.0; 4],
        metallic: 0.0,
        roughness: 1.0,
        base_color_texture: Some(texture(1)),
        normal_texture: Some(NormalTextureAsset {
            texture: texture(2),
            scale: 1.0,
        }),
        metallic_roughness_texture: Some(texture(3)),
        occlusion_texture: Some(OcclusionTextureAsset {
            texture: texture(4),
            strength: 1.0,
        }),
    });
    doc.assets.meshes.push(MeshAsset {
        name: "complete_mesh".into(),
        source_mesh_index: 0,
        primitives: vec![Primitive {
            material: Some(0),
            positions: vec![Vec3::ZERO, Vec3::X, Vec3::Y],
            joints: vec![[0, 0, 0, 0]; 3],
            weights: vec![[1.0, 0.0, 0.0, 0.0]; 3],
            ..Default::default()
        }],
    });
    doc.assets.instances.push(MeshInstance {
        source_node_index: 0,
        node: 0,
        mesh: 0,
        skin_joints: vec![0, 1],
        skin_ibms: vec![glam::Mat4::IDENTITY; 2],
    });
    doc.assets.scenes = vec![SceneAsset {
        source_scene_index: 0,
        name: None,
        roots: vec![0],
    }];
    doc.assets.default_scene = Some(0);
    doc.assets.source_skeleton.coverage = SourceSkeletonCoverage::Complete;
    let mut root = SourceNodeAsset::new(
        0,
        SourceNodeLocalRest::Trs {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        },
    );
    root.bone = Some(0);
    root.scene_root_indices = vec![0];
    doc.assets.source_skeleton.nodes.push(root);
    let rest = doc.skeleton.bones[1].rest;
    let mut child = SourceNodeAsset::new(
        1,
        SourceNodeLocalRest::Trs {
            translation: rest.translation,
            rotation: rest.rotation,
            scale: rest.scale,
        },
    );
    child.parent_source_node_index = Some(0);
    child.bone = Some(1);
    doc.assets.source_skeleton.nodes.push(child);
    doc.assets.source_skeleton.skins.push(SourceSkinAsset {
        source_skin_index: 0,
        joint_source_node_indices: vec![0, 1],
        inverse_bind_accessor: SourceInverseBindAccessor {
            status: SourceInverseBindAccessorStatus::Available,
            declared_count: Some(2),
            matrices: vec![glam::Mat4::IDENTITY; 2],
        },
        attachments: vec![SourceSkinAttachment {
            source_node_index: 0,
            source_mesh_index: Some(0),
        }],
        ..Default::default()
    });
    doc
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
fn strict_complete_asset_keeps_skin_node_scene_and_supported_material_slots() {
    use animsmith_gltf::write::{
        GlbProjectionPolicyV1, GlbWriteLimits, preflight_glb_bytes, write_glb_bytes,
    };

    let doc = complete_strict_asset_doc();
    let receipt = preflight_glb_bytes(
        &doc,
        GlbProjectionPolicyV1::StrictFootCycleV1,
        GlbWriteLimits::FOOT_CYCLE_V1,
    )
    .expect("complete strict projection");
    let bytes = write_glb_bytes(&doc, GlbProjectionPolicyV1::StrictFootCycleV1, &receipt)
        .expect("strict bytes");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("complete.glb");
    std::fs::write(&path, &bytes).unwrap();
    assert_eq!(
        std::fs::read(&path).unwrap(),
        bytes,
        "path retains the exact candidate bytes"
    );
    let loaded = animsmith_gltf::load(&path).expect("reload strict candidate");
    assert_eq!(
        loaded.skeleton.bones.len(),
        doc.skeleton.bones.len(),
        "strict adds no holder bone"
    );
    assert_eq!(loaded.assets.instances.len(), 1);
    assert_eq!(
        loaded.assets.instances[0].node, 0,
        "strict retains the skinned attachment node"
    );
    assert_eq!(loaded.assets.instances[0].skin_joints, vec![0, 1]);
    assert_eq!(loaded.assets.scenes.len(), 1);
    assert_eq!(loaded.assets.default_scene, Some(0));
    let material = &loaded.assets.materials[0];
    assert_eq!(material.base_color_texture.as_ref().unwrap().bytes, vec![1]);
    assert_eq!(
        material.normal_texture.as_ref().unwrap().texture.bytes,
        vec![2]
    );
    assert_eq!(
        material.metallic_roughness_texture.as_ref().unwrap().bytes,
        vec![3]
    );
    assert_eq!(
        material.occlusion_texture.as_ref().unwrap().texture.bytes,
        vec![4]
    );

    // Every view is padded to four bytes. The final one-byte occlusion image
    // would otherwise make the BIN chunk unaligned.
    let mut offset = 12;
    let mut saw_bin = false;
    while offset < bytes.len() {
        let len = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        let kind: [u8; 4] = bytes[offset + 4..offset + 8].try_into().unwrap();
        assert_eq!(len % 4, 0, "every GLB chunk is four-byte aligned");
        if kind == *b"BIN\0" {
            saw_bin = true;
            assert_eq!(
                &bytes[offset + 8 + len - 4..offset + 8 + len],
                &[4, 0, 0, 0]
            );
        }
        offset += 8 + len;
    }
    assert!(saw_bin, "textured candidate has a BIN chunk");
}

#[test]
fn strict_refuses_node_overwrite_and_mismatched_source_attachment() {
    use animsmith_gltf::{
        WriteError,
        write::{GlbProjectionPolicyV1, GlbWriteLimits, preflight_glb_bytes},
    };
    let strict = |doc: &Document| {
        preflight_glb_bytes(
            doc,
            GlbProjectionPolicyV1::StrictFootCycleV1,
            GlbWriteLimits::FOOT_CYCLE_V1,
        )
    };
    let mut doc = complete_strict_asset_doc();
    doc.assets.instances.push(doc.assets.instances[0].clone());
    assert!(matches!(
        strict(&doc),
        Err(WriteError::Refused(message)) if message.contains("multiple mesh instances target one normalized node")
    ));
    let mut doc = complete_strict_asset_doc();
    let mut other_mesh = doc.assets.meshes[0].clone();
    other_mesh.source_mesh_index = 1;
    doc.assets.meshes.push(other_mesh);
    doc.assets.instances.push(MeshInstance {
        source_node_index: 99,
        node: 0,
        mesh: 1,
        ..Default::default()
    });
    assert!(matches!(
        strict(&doc),
        Err(WriteError::Refused(message)) if message.contains("multiple mesh instances target one normalized node")
    ));
    let mut doc = complete_strict_asset_doc();
    doc.assets.instances[0].node = 1;
    assert!(matches!(
        strict(&doc),
        Err(WriteError::Refused(message)) if message.contains("attachment, joints, or inverse binds")
    ));
}

#[test]
fn strict_receipt_ignores_unavailable_source_only_sidecar_mutation() {
    use animsmith_gltf::write::{
        GlbProjectionPolicyV1, GlbWriteLimits, preflight_glb_bytes, write_glb_bytes,
    };
    let mut doc = synthetic_doc();
    let mut sidecar = SourceNodeAsset::new(99, SourceNodeLocalRest::Matrix(glam::Mat4::IDENTITY));
    sidecar.name = Some("before".into());
    doc.assets.source_skeleton.nodes.push(sidecar);
    let receipt = preflight_glb_bytes(
        &doc,
        GlbProjectionPolicyV1::StrictFootCycleV1,
        GlbWriteLimits::FOOT_CYCLE_V1,
    )
    .unwrap();
    doc.assets.source_skeleton.nodes[0].name = Some("after".into());
    assert!(write_glb_bytes(&doc, GlbProjectionPolicyV1::StrictFootCycleV1, &receipt).is_ok());
}

#[test]
fn strict_complete_sidecars_refuse_resource_graph_and_stale_skin_count() {
    use animsmith_gltf::{
        WriteError,
        write::{GlbProjectionPolicyV1, GlbWriteLimits, preflight_glb_bytes},
    };
    let strict = |doc: &Document| {
        preflight_glb_bytes(
            doc,
            GlbProjectionPolicyV1::StrictFootCycleV1,
            GlbWriteLimits::FOOT_CYCLE_V1,
        )
    };
    let mut doc = complete_strict_asset_doc();
    doc.assets.material_resources.coverage = MaterialResourceCoverage::Complete;
    assert!(
        matches!(strict(&doc), Err(WriteError::Refused(message)) if message.contains("material-resource")),
        "an empty source sidecar cannot excuse writer-facing materials"
    );
    doc.assets.materials.clear();
    for mesh in &mut doc.assets.meshes {
        for primitive in &mut mesh.primitives {
            primitive.material = None;
        }
    }
    assert!(
        strict(&doc).is_ok(),
        "complete empty resource facts round-trip exactly"
    );
    doc.assets
        .material_resources
        .materials
        .push(SourceMaterialAsset::default());
    assert!(
        matches!(strict(&doc), Err(WriteError::Refused(message)) if message.contains("material-resource"))
    );
    let mut doc = complete_strict_asset_doc();
    doc.assets.source_skeleton.skins[0]
        .inverse_bind_accessor
        .declared_count = Some(1);
    assert!(
        matches!(strict(&doc), Err(WriteError::Refused(message)) if message.contains("inverse-bind"))
    );
    let mut doc = complete_strict_asset_doc();
    doc.assets.default_scene = None;
    assert!(
        matches!(strict(&doc), Err(WriteError::Refused(message)) if message.contains("scenes"))
    );
}

#[test]
fn strict_structural_limits_refuse_before_shape_validation_or_projection() {
    use animsmith_gltf::{
        WriteError,
        write::{GlbProjectionPolicyV1, GlbWriteLimits, preflight_glb_bytes},
    };

    let mut doc = synthetic_doc();
    doc.skeleton.bones[0].name = "ab".into();
    doc.skeleton.bones[1].name = "cd".into();
    doc.clips[0].name.clear();
    let limits = GlbWriteLimits {
        max_name_bytes: 4,
        ..GlbWriteLimits::FOOT_CYCLE_V1
    };
    assert!(
        preflight_glb_bytes(&doc, GlbProjectionPolicyV1::StrictFootCycleV1, limits).is_ok(),
        "exact name cap admits"
    );
    let exact_work = GlbWriteLimits {
        max_work: 46,
        ..GlbWriteLimits::FOOT_CYCLE_V1
    };
    assert!(
        preflight_glb_bytes(&doc, GlbProjectionPolicyV1::StrictFootCycleV1, exact_work).is_ok(),
        "scalar component charge admits its exact budget"
    );
    assert!(matches!(
        preflight_glb_bytes(
            &doc,
            GlbProjectionPolicyV1::StrictFootCycleV1,
            GlbWriteLimits { max_work: 45, ..exact_work }
        ),
        Err(WriteError::Refused(message)) if message.contains("work")
    ));
    // Deliberately invalid after the structural boundary: a limit refusal must
    // win, demonstrating no shape-validation/projection path was entered.
    doc.clips[0].tracks[0].bone = usize::MAX;
    let limits = GlbWriteLimits {
        max_name_bytes: 3,
        ..limits
    };
    assert!(matches!(
        preflight_glb_bytes(&doc, GlbProjectionPolicyV1::StrictFootCycleV1, limits),
        Err(WriteError::Refused(message)) if message.contains("name bytes")
    ));
    for (limits, expected) in [
        (
            GlbWriteLimits {
                max_structural_rows: 1,
                ..GlbWriteLimits::FOOT_CYCLE_V1
            },
            "JSON rows",
        ),
        (
            GlbWriteLimits {
                max_work: 1,
                ..GlbWriteLimits::FOOT_CYCLE_V1
            },
            "work",
        ),
    ] {
        assert!(matches!(
            preflight_glb_bytes(&doc, GlbProjectionPolicyV1::StrictFootCycleV1, limits),
            Err(WriteError::Refused(message)) if message.contains(expected)
        ));
    }
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
fn strict_receipt_refuses_same_length_json_and_bin_changes() {
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
    .unwrap();
    doc.clips[0].name = "wave".into(); // same UTF-8 byte length as "sway"
    assert!(matches!(
        write_glb_bytes(&doc, GlbProjectionPolicyV1::StrictFootCycleV1, &receipt),
        Err(WriteError::ReceiptMismatch)
    ));

    let mut doc = synthetic_doc();
    let receipt = preflight_glb_bytes(
        &doc,
        GlbProjectionPolicyV1::StrictFootCycleV1,
        GlbWriteLimits::FOOT_CYCLE_V1,
    )
    .unwrap();
    let TrackValues::Vec3s(values) = &mut doc.clips[0].tracks[1].values else {
        panic!("fixture translation values")
    };
    values[1].x = 2.0; // same BIN byte count, different exact bytes
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
        ..GlbWriteLimits::FOOT_CYCLE_V1
    };
    assert_eq!(generous.header_bytes(), 12);
    assert!(preflight_glb_bytes(&doc, GlbProjectionPolicyV1::StrictFootCycleV1, exact).is_ok());
    for (limits, field) in [
        (
            GlbWriteLimits {
                max_json_bytes: exact.max_json_bytes - 1,
                ..exact
            },
            "configured JSON chunk limit",
        ),
        (
            GlbWriteLimits {
                max_bin_bytes: exact.max_bin_bytes - 1,
                ..exact
            },
            "configured BIN chunk limit",
        ),
    ] {
        assert!(matches!(
            preflight_glb_bytes(&doc, GlbProjectionPolicyV1::StrictFootCycleV1, limits),
            Err(WriteError::TooLarge { field: got, .. }) if got == field
        ));
    }
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
fn strict_preflight_refuses_unrepresentable_skin_and_complete_sidecar_facts() {
    use animsmith_gltf::{
        WriteError,
        write::{GlbProjectionPolicyV1, GlbWriteLimits, preflight_glb_bytes},
    };

    let skinned_mesh = || MeshAsset {
        name: "skin".into(),
        source_mesh_index: 0,
        primitives: vec![Primitive {
            positions: vec![Vec3::ZERO, Vec3::X, Vec3::Y],
            joints: vec![[2, 0, 0, 0]; 3],
            weights: vec![[1.0, 0.0, 0.0, 0.0]; 3],
            ..Default::default()
        }],
    };
    let mut doc = synthetic_doc();
    doc.assets.meshes.push(skinned_mesh());
    doc.assets.instances.push(MeshInstance {
        source_node_index: 0,
        node: 0,
        mesh: 0,
        skin_joints: vec![0, 1],
        skin_ibms: vec![glam::Mat4::IDENTITY; 2],
    });
    assert!(matches!(
        preflight_glb_bytes(&doc, GlbProjectionPolicyV1::StrictFootCycleV1, GlbWriteLimits::FOOT_CYCLE_V1),
        Err(WriteError::Refused(message)) if message.contains("primary skin influences")
    ));

    let mut doc = synthetic_doc();
    doc.assets.material_resources.coverage = MaterialResourceCoverage::Complete;
    doc.assets
        .material_resources
        .materials
        .push(SourceMaterialAsset {
            material_index: 0,
            name: None,
            texture_bindings: vec![SourceMaterialTextureBinding {
                slot: MaterialTextureSlot::Emissive,
                texture_index: 0,
            }],
        });
    assert!(matches!(
        preflight_glb_bytes(&doc, GlbProjectionPolicyV1::StrictFootCycleV1, GlbWriteLimits::FOOT_CYCLE_V1),
        Err(WriteError::Refused(message)) if message.contains("material-resource")
    ));

    let mut doc = synthetic_doc();
    doc.assets.source_skeleton.coverage = SourceSkeletonCoverage::Complete;
    let mut node = SourceNodeAsset::new(0, SourceNodeLocalRest::Matrix(glam::Mat4::IDENTITY));
    node.bone = Some(0);
    doc.assets.source_skeleton.nodes.push(node);
    let rest = doc.skeleton.bones[1].rest;
    let mut child = SourceNodeAsset::new(
        1,
        SourceNodeLocalRest::Trs {
            translation: rest.translation,
            rotation: rest.rotation,
            scale: rest.scale,
        },
    );
    child.parent_source_node_index = Some(0);
    child.bone = Some(1);
    doc.assets.source_skeleton.nodes.push(child);
    let result = preflight_glb_bytes(
        &doc,
        GlbProjectionPolicyV1::StrictFootCycleV1,
        GlbWriteLimits::FOOT_CYCLE_V1,
    );
    assert!(
        matches!(
            result,
            Err(WriteError::Refused(ref message)) if message.contains("local rest")
        ),
        "{result:?}"
    );

    let mut doc = synthetic_doc();
    doc.assets.meshes.push(MeshAsset {
        name: "mesh".into(),
        source_mesh_index: 0,
        primitives: vec![Primitive {
            positions: vec![Vec3::ZERO, Vec3::X, Vec3::Y],
            ..Default::default()
        }],
    });
    doc.assets.instances.push(MeshInstance {
        source_node_index: 0,
        node: 0,
        mesh: 0,
        ..Default::default()
    });
    doc.assets.source_skeleton.coverage = SourceSkeletonCoverage::Complete;
    let mut root = SourceNodeAsset::new(
        0,
        SourceNodeLocalRest::Trs {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        },
    );
    root.bone = Some(0);
    doc.assets.source_skeleton.nodes.push(root);
    let rest = doc.skeleton.bones[1].rest;
    let mut child = SourceNodeAsset::new(
        1,
        SourceNodeLocalRest::Trs {
            translation: rest.translation,
            rotation: rest.rotation,
            scale: rest.scale,
        },
    );
    child.parent_source_node_index = Some(0);
    child.bone = Some(1);
    doc.assets.source_skeleton.nodes.push(child);
    doc.assets.source_skeleton.skins.push(SourceSkinAsset {
        source_skin_index: 0,
        joint_source_node_indices: vec![0, 1],
        inverse_bind_accessor: SourceInverseBindAccessor {
            status: SourceInverseBindAccessorStatus::Available,
            declared_count: Some(2),
            matrices: vec![glam::Mat4::IDENTITY; 2],
        },
        attachments: vec![SourceSkinAttachment {
            source_node_index: 0,
            source_mesh_index: Some(0),
        }],
        ..Default::default()
    });
    let result = preflight_glb_bytes(
        &doc,
        GlbProjectionPolicyV1::StrictFootCycleV1,
        GlbWriteLimits::FOOT_CYCLE_V1,
    );
    assert!(
        matches!(
            result,
            Err(WriteError::Refused(ref message)) if message.contains("attachment, joints, or inverse binds")
        ),
        "{result:?}"
    );
}

#[test]
fn strict_scene_check_does_not_index_unavailable_source_sidecars() {
    use animsmith_gltf::write::{GlbProjectionPolicyV1, GlbWriteLimits, preflight_glb_bytes};

    let mut doc = synthetic_doc();
    doc.assets.scenes = vec![SceneAsset {
        source_scene_index: 0,
        name: None,
        roots: vec![0],
    }];
    doc.assets.default_scene = Some(0);
    let mut stale = SourceNodeAsset::new(99, SourceNodeLocalRest::Matrix(glam::Mat4::IDENTITY));
    stale.bone = Some(usize::MAX);
    doc.assets.source_skeleton.nodes.push(stale);
    assert!(
        preflight_glb_bytes(
            &doc,
            GlbProjectionPolicyV1::StrictFootCycleV1,
            GlbWriteLimits::FOOT_CYCLE_V1,
        )
        .is_ok()
    );
}

#[test]
fn strict_refusal_domains_are_fail_closed() {
    use animsmith_gltf::{
        WriteError,
        write::{GlbProjectionPolicyV1, GlbWriteLimits, preflight_glb_bytes},
    };
    let strict = |doc: &Document| {
        preflight_glb_bytes(
            doc,
            GlbProjectionPolicyV1::StrictFootCycleV1,
            GlbWriteLimits::FOOT_CYCLE_V1,
        )
    };

    let mut doc = synthetic_doc();
    doc.assets.materials.push(plain_material());
    assert!(
        matches!(strict(&doc), Err(WriteError::Refused(message)) if message.contains("materials without a mesh"))
    );

    let mut doc = synthetic_doc();
    doc.assets.meshes.push(MeshAsset::default());
    assert!(
        matches!(strict(&doc), Err(WriteError::Refused(message)) if message.contains("no primitives"))
    );

    let mut doc = synthetic_doc();
    doc.assets.meshes.push(MeshAsset {
        name: "mesh".into(),
        source_mesh_index: 0,
        primitives: vec![Primitive {
            positions: vec![Vec3::ZERO, Vec3::X, Vec3::Y],
            material: Some(0),
            ..Default::default()
        }],
    });
    let mut material = plain_material();
    material.base_color_texture = Some(TextureAsset {
        bytes: Vec::new(),
        mime: "image/png".into(),
    });
    doc.assets.materials.push(material);
    assert!(
        matches!(strict(&doc), Err(WriteError::Refused(message)) if message.contains("unsupported embedded texture"))
    );

    let mut doc = synthetic_doc();
    doc.assets.meshes.push(MeshAsset {
        name: "mesh".into(),
        source_mesh_index: 0,
        primitives: vec![Primitive {
            positions: vec![Vec3::ZERO, Vec3::X, Vec3::Y],
            joints: vec![[0, 0, 0, 0]; 3],
            weights: vec![[1.0, 0.0, 0.0, 0.0]; 3],
            ..Default::default()
        }],
    });
    doc.assets.instances.extend([
        MeshInstance {
            source_node_index: 0,
            node: 0,
            mesh: 0,
            skin_joints: vec![0, 1],
            skin_ibms: vec![glam::Mat4::IDENTITY; 2],
        },
        MeshInstance {
            source_node_index: 1,
            node: 1,
            mesh: 0,
            ..Default::default()
        },
    ]);
    assert!(
        matches!(strict(&doc), Err(WriteError::Refused(message)) if message.contains("both skinned and unskinned"))
    );
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
