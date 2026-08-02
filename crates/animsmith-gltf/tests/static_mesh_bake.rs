//! Public-contract tests for static mesh transform baking.
//!
//! The expected vertices and normals below are calculated from the source
//! fixture matrices, not by the baking implementation.  This deliberately
//! exercises the result through the normal glTF writer and loader too.

use animsmith_core::model::*;
use animsmith_core::{StaticMeshBakeError, bake_static_mesh_transforms};
use glam::{Mat3, Mat4, Quat, Vec3};

const EPSILON: f32 = 1.0e-5;
const TINY_JPEG: &[u8] = &[
    0xff, 0xd8, 0xff, 0xe0, 0x00, 0x10, b'J', b'F', b'I', b'F', 0xff, 0xd9,
];

fn close_vec3(actual: Vec3, expected: Vec3, what: &str) {
    assert!(
        actual.abs_diff_eq(expected, EPSILON),
        "{what}: got {actual:?}, expected {expected:?} (epsilon {EPSILON})"
    );
}

fn close_f32(actual: f32, expected: f32, what: &str) {
    assert!(
        (actual - expected).abs() <= EPSILON,
        "{what}: got {actual}, expected {expected} (epsilon {EPSILON})"
    );
}

fn aabb(points: &[Vec3]) -> (Vec3, Vec3) {
    assert!(
        !points.is_empty(),
        "AABB oracle requires at least one point"
    );
    points.iter().copied().fold(
        (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
        |(min, max), point| (min.min(point), max.max(point)),
    )
}

fn assert_bake_error(
    doc: &Document,
    expected: &str,
    matches_expected: impl FnOnce(&StaticMeshBakeError) -> bool,
) {
    let error = bake_static_mesh_transforms(doc).expect_err(expected);
    assert!(
        matches_expected(&error),
        "expected {expected}, got {error:?}: {error}"
    );
}

fn transform(translation: Vec3, rotation: Quat, scale: Vec3) -> Transform {
    Transform {
        translation,
        rotation,
        scale,
    }
}

/// A deliberately asymmetric hierarchy.  Neither the position nor normal
/// oracle can accidentally pass if it uses only the mesh node's local TRS.
fn supported_document() -> Document {
    let root = transform(
        Vec3::new(3.0, -2.0, 5.0),
        Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
        Vec3::new(2.0, 3.0, 5.0),
    );
    let carrier = transform(
        Vec3::new(1.0, 4.0, -1.0),
        Quat::from_rotation_x(std::f32::consts::FRAC_PI_3),
        Vec3::splat(1.25),
    );
    let mesh_a = transform(
        Vec3::new(-2.0, 1.0, 3.0),
        Quat::from_rotation_y(-0.37),
        Vec3::new(1.0, 2.0, 1.0),
    );
    let mesh_b = transform(
        Vec3::new(2.0, -1.0, 0.5),
        Quat::from_rotation_z(0.23),
        Vec3::new(0.5, 1.5, 2.0),
    );
    let positions = vec![
        Vec3::new(-1.0, 0.25, 2.0),
        Vec3::new(2.0, -0.5, 0.75),
        Vec3::new(0.4, 1.5, -1.0),
        Vec3::new(-0.25, 2.0, 0.5),
    ];
    let normal = Vec3::new(1.0, 2.0, 3.0).normalize();
    Document {
        skeleton: Skeleton {
            bones: vec![
                Bone {
                    name: "axis-unit-helper".into(),
                    parent: None,
                    rest: root,
                    inverse_bind: None,
                },
                Bone {
                    name: "carrier".into(),
                    parent: Some(0),
                    rest: carrier,
                    inverse_bind: None,
                },
                Bone {
                    name: "first-mesh".into(),
                    parent: Some(1),
                    rest: mesh_a,
                    inverse_bind: None,
                },
                Bone {
                    name: "second-mesh".into(),
                    parent: Some(0),
                    rest: mesh_b,
                    inverse_bind: None,
                },
            ],
        },
        clips: vec![],
        assets: SceneAssets {
            meshes: vec![
                MeshAsset {
                    name: "indexed-and-textured".into(),
                    source_mesh_index: 11,
                    primitives: vec![Primitive {
                        material: Some(0),
                        indices: vec![0, 1, 2, 0, 2, 3],
                        positions: positions.clone(),
                        normals: vec![normal; positions.len()],
                        uvs: vec![[0.0, 0.0], [1.0, 0.0], [0.75, 1.0], [0.0, 1.0]],
                        ..Primitive::default()
                    }],
                },
                MeshAsset {
                    name: "second-mesh".into(),
                    source_mesh_index: 12,
                    primitives: vec![Primitive {
                        material: Some(1),
                        indices: vec![0, 2, 1],
                        positions: positions.iter().map(|p| p * 0.25).collect(),
                        normals: vec![normal; positions.len()],
                        uvs: vec![[0.1, 0.2], [0.8, 0.2], [0.3, 0.9], [0.2, 0.7]],
                        ..Primitive::default()
                    }],
                },
            ],
            instances: vec![
                MeshInstance {
                    source_node_index: 7,
                    node: 2,
                    mesh: 0,
                    ..MeshInstance::default()
                },
                MeshInstance {
                    source_node_index: 8,
                    node: 3,
                    mesh: 1,
                    ..MeshInstance::default()
                },
            ],
            materials: vec![
                MaterialAsset {
                    name: "painted".into(),
                    base_color: [0.3, 0.4, 0.5, 0.6],
                    metallic: 0.7,
                    roughness: 0.2,
                    base_color_texture: Some(TextureAsset {
                        bytes: TINY_JPEG.to_vec(),
                        mime: "image/jpeg".into(),
                    }),
                },
                MaterialAsset {
                    name: "plain".into(),
                    base_color: [0.9, 0.8, 0.7, 1.0],
                    metallic: 0.1,
                    roughness: 0.9,
                    base_color_texture: None,
                },
            ],
            scenes: vec![SceneAsset {
                source_scene_index: 0,
                name: Some("source".into()),
                roots: vec![0],
            }],
            default_scene: Some(0),
        },
        source: SourceInfo::default(),
    }
}

fn world_matrices(doc: &Document) -> Vec<Mat4> {
    let mut worlds = Vec::with_capacity(doc.skeleton.bones.len());
    for bone in &doc.skeleton.bones {
        let local = bone.rest.to_mat4();
        worlds.push(bone.parent.map_or(local, |parent| worlds[parent] * local));
    }
    worlds
}

fn expected_world_geometry(doc: &Document, instance: &MeshInstance) -> (Vec<Vec3>, Vec<Vec3>) {
    let world = world_matrices(doc)[instance.node];
    let normal_matrix = Mat3::from_mat4(world).inverse().transpose();
    let primitive = &doc.assets.meshes[instance.mesh].primitives[0];
    (
        primitive
            .positions
            .iter()
            .map(|p| world.transform_point3(*p))
            .collect(),
        primitive
            .normals
            .iter()
            .map(|n| (normal_matrix * *n).normalize())
            .collect(),
    )
}

#[test]
fn static_mesh_bake_flattens_geometry_without_losing_scene_asset_semantics() {
    let source = supported_document();
    let expected: Vec<_> = source
        .assets
        .instances
        .iter()
        .map(|i| expected_world_geometry(&source, i))
        .collect();

    let baked = bake_static_mesh_transforms(&source).expect("supported static scene bakes");
    let output = &baked.document;
    let repeated = bake_static_mesh_transforms(&source).expect("repeated supported bake succeeds");

    assert_eq!(
        serde_json::to_value(&baked.evidence).unwrap(),
        serde_json::to_value(&repeated.evidence).unwrap(),
        "repeated bakes produce identical machine-readable evidence"
    );
    let worlds = world_matrices(&source);
    assert_eq!(
        baked.evidence.entries.len(),
        source.assets.instances.len(),
        "evidence/source cardinality is explicit before pairwise comparison"
    );
    for (ordinal, (entry, source_instance)) in baked
        .evidence
        .entries
        .iter()
        .zip(&source.assets.instances)
        .enumerate()
    {
        let source_node = &source.skeleton.bones[source_instance.node];
        let source_mesh = &source.assets.meshes[source_instance.mesh];
        let world = worlds[source_instance.node];
        assert_eq!(entry.source_node_index, source_instance.source_node_index);
        assert_eq!(entry.source_node_name, source_node.name);
        assert_eq!(entry.source_mesh_ordinal, source_instance.mesh);
        assert_eq!(entry.source_mesh_index, source_mesh.source_mesh_index);
        assert_eq!(entry.source_mesh_name, source_mesh.name);
        assert_eq!(entry.output_node_index, ordinal + 1);
        assert_eq!(entry.output_mesh_index, ordinal);
        assert_eq!(entry.primitive_count, source_mesh.primitives.len());
        assert_eq!(
            entry.position_count,
            source_mesh
                .primitives
                .iter()
                .map(|primitive| primitive.positions.len())
                .sum::<usize>()
        );
        assert_eq!(
            entry.normal_count,
            source_mesh
                .primitives
                .iter()
                .map(|primitive| primitive.normals.len())
                .sum::<usize>()
        );
        assert_eq!(
            entry.world_transform,
            world.to_cols_array(),
            "evidence records the accumulated, not merely local, matrix"
        );
        close_f32(
            entry.linear_determinant,
            Mat3::from_mat4(world).determinant(),
            "evidence records the independently calculated linear determinant",
        );
    }

    assert_eq!(
        output.assets.meshes.len(),
        2,
        "both independent mesh definitions survive"
    );
    assert_eq!(
        output.assets.instances.len(),
        2,
        "both mesh instances survive"
    );
    assert_eq!(
        output.skeleton.bones.len(),
        output.assets.instances.len() + 1,
        "canonical topology has exactly one root plus one node per mesh instance"
    );
    assert_eq!(
        output
            .skeleton
            .bones
            .iter()
            .filter(|bone| bone.parent.is_none())
            .count(),
        1,
        "canonical topology has exactly one root"
    );
    let root = &output.skeleton.bones[0];
    assert_eq!(root.name, "animsmith-static-root");
    assert_eq!(root.parent, None);
    assert_eq!(root.rest, Transform::IDENTITY);
    assert_eq!(root.inverse_bind, None);
    assert_eq!(output.assets.scenes.len(), 1);
    assert_eq!(output.assets.scenes[0].roots, vec![0]);
    assert_eq!(output.assets.default_scene, Some(0));
    for (ordinal, instance) in output.assets.instances.iter().enumerate() {
        assert_eq!(instance.node, ordinal + 1);
        assert_eq!(instance.mesh, ordinal);
        assert!(instance.skin_joints.is_empty());
        assert!(instance.skin_ibms.is_empty());
        let node = &output.skeleton.bones[instance.node];
        assert_eq!(node.name, format!("animsmith-static-mesh-{ordinal}"));
        assert_eq!(node.parent, Some(0));
        assert_eq!(node.rest, Transform::IDENTITY);
        assert_eq!(node.inverse_bind, None);
    }
    assert_eq!(output.assets.materials.len(), 2, "materials survive");
    for (actual, expected) in output.assets.materials.iter().zip(&source.assets.materials) {
        assert_eq!(actual.name, expected.name);
        assert_eq!(actual.base_color, expected.base_color);
        assert_eq!(actual.metallic, expected.metallic);
        assert_eq!(actual.roughness, expected.roughness);
    }
    assert_eq!(
        output.assets.materials[0]
            .base_color_texture
            .as_ref()
            .unwrap()
            .bytes,
        TINY_JPEG
    );
    assert_eq!(
        output.assets.materials[0]
            .base_color_texture
            .as_ref()
            .unwrap()
            .mime,
        "image/jpeg"
    );

    for (index, instance) in output.assets.instances.iter().enumerate() {
        let primitive = &output.assets.meshes[instance.mesh].primitives[0];
        let source_primitive =
            &source.assets.meshes[source.assets.instances[index].mesh].primitives[0];
        assert_eq!(
            primitive.indices, source_primitive.indices,
            "mesh {index} preserves indices"
        );
        assert_eq!(
            primitive.uvs, source_primitive.uvs,
            "mesh {index} preserves UVs"
        );
        assert_eq!(
            primitive.material, source_primitive.material,
            "mesh {index} preserves material binding"
        );
        assert_eq!(
            primitive.positions.len(),
            expected[index].0.len(),
            "mesh {index} position cardinality matches before pairwise comparison"
        );
        for (actual, expected) in primitive.positions.iter().zip(&expected[index].0) {
            close_vec3(
                *actual,
                *expected,
                "baked position must equal source world position",
            );
        }
        let (before_min, before_max) = aabb(&expected[index].0);
        let (after_min, after_max) = aabb(&primitive.positions);
        close_vec3(
            after_min,
            before_min,
            "baked AABB min must equal source world-space AABB min",
        );
        close_vec3(
            after_max,
            before_max,
            "baked AABB max must equal source world-space AABB max",
        );
        assert_eq!(
            primitive.normals.len(),
            expected[index].1.len(),
            "mesh {index} normal cardinality matches before pairwise comparison"
        );
        for (actual, expected) in primitive.normals.iter().zip(&expected[index].1) {
            close_vec3(
                *actual,
                *expected,
                "baked normal must equal normalized inverse-transpose normal",
            );
            assert!(
                (actual.length() - 1.0).abs() <= EPSILON,
                "baked normals are unit length"
            );
        }
    }

    // A normalized static result has no transform-bearing helper remaining on
    // any mesh path.  Checking every node is stricter and makes reloading
    // unable to reintroduce a hidden axis/unit conversion.
    for bone in &output.skeleton.bones {
        assert_eq!(
            bone.rest,
            Transform::IDENTITY,
            "baked output node {:?} is identity",
            bone.name
        );
    }
    assert!(
        output.clips.is_empty(),
        "the supported fixture has no animation to carry"
    );

    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.glb");
    let b = dir.path().join("b.glb");
    animsmith_gltf::write::write(output, &a).expect("writes baked GLB");
    animsmith_gltf::write::write(&repeated.document, &b).expect("writes repeated baked GLB");
    assert_eq!(
        std::fs::read(&a).unwrap(),
        std::fs::read(&b).unwrap(),
        "same-platform bakes are byte-identical"
    );

    let reloaded = animsmith_gltf::load(&a).expect("baked GLB reloads");
    assert_eq!(
        reloaded.skeleton.bones.len(),
        output.skeleton.bones.len(),
        "reload preserves canonical node cardinality"
    );
    assert_eq!(
        reloaded
            .skeleton
            .bones
            .iter()
            .filter(|bone| bone.parent.is_none())
            .count(),
        1,
        "reload preserves the single canonical root"
    );
    assert_eq!(reloaded.skeleton.bones[0].parent, None);
    assert_eq!(reloaded.assets.scenes.len(), 1);
    assert_eq!(reloaded.assets.scenes[0].roots, vec![0]);
    assert_eq!(reloaded.assets.default_scene, Some(0));
    assert_eq!(
        reloaded.assets.materials.len(),
        source.assets.materials.len()
    );
    let reloaded_texture = reloaded.assets.materials[0]
        .base_color_texture
        .as_ref()
        .expect("embedded texture survives write and reload");
    assert_eq!(reloaded_texture.bytes, TINY_JPEG);
    assert_eq!(reloaded_texture.mime, "image/jpeg");
    for bone in &reloaded.skeleton.bones {
        assert_eq!(
            bone.rest,
            Transform::IDENTITY,
            "reload has no helper transform"
        );
    }
    for (index, instance) in reloaded.assets.instances.iter().enumerate() {
        assert_eq!(instance.node, index + 1);
        assert_eq!(instance.mesh, index);
        assert_eq!(reloaded.skeleton.bones[instance.node].parent, Some(0));
        assert_eq!(
            reloaded.skeleton.bones[instance.node].rest,
            Transform::IDENTITY
        );
        let primitive = &reloaded.assets.meshes[instance.mesh].primitives[0];
        let source_primitive =
            &source.assets.meshes[source.assets.instances[index].mesh].primitives[0];
        assert_eq!(primitive.indices, source_primitive.indices);
        assert_eq!(primitive.uvs, source_primitive.uvs);
        assert_eq!(primitive.material, source_primitive.material);
        assert_eq!(
            primitive.positions.len(),
            expected[index].0.len(),
            "reloaded mesh {index} position cardinality matches before pairwise comparison"
        );
        for (actual, expected) in primitive.positions.iter().zip(&expected[index].0) {
            close_vec3(
                *actual,
                *expected,
                "reloaded baked position remains world equivalent",
            );
        }
        let (before_min, before_max) = aabb(&expected[index].0);
        let (after_min, after_max) = aabb(&primitive.positions);
        close_vec3(after_min, before_min, "reloaded baked AABB min");
        close_vec3(after_max, before_max, "reloaded baked AABB max");
        assert_eq!(
            primitive.normals.len(),
            expected[index].1.len(),
            "reloaded mesh {index} normal cardinality matches before pairwise comparison"
        );
        for (actual, expected) in primitive.normals.iter().zip(&expected[index].1) {
            close_vec3(
                *actual,
                *expected,
                "reloaded baked normal remains world equivalent",
            );
        }
    }
}

#[test]
fn static_mesh_bake_rejects_ambiguous_or_nonstatic_inputs() {
    let source = supported_document();

    let mut no_instances = source.clone();
    no_instances.assets.instances.clear();
    assert_bake_error(&no_instances, "NoInstances", |error| {
        matches!(error, StaticMeshBakeError::NoInstances)
    });

    let mut shared = source.clone();
    shared.skeleton.bones.push(Bone {
        name: "another-instance".into(),
        parent: Some(0),
        rest: Transform::IDENTITY,
        inverse_bind: None,
    });
    shared.assets.instances.push(MeshInstance {
        source_node_index: 99,
        node: 4,
        mesh: 0,
        ..MeshInstance::default()
    });
    assert_bake_error(
        &shared,
        "MeshInstanceCount for shared definition",
        |error| {
            matches!(
                error,
                StaticMeshBakeError::MeshInstanceCount {
                    mesh: 0,
                    source_mesh_index: 11,
                    instances: 2
                }
            )
        },
    );

    let mut unattached = source.clone();
    unattached.assets.meshes.push(MeshAsset {
        source_mesh_index: 777,
        ..MeshAsset::default()
    });
    assert_bake_error(
        &unattached,
        "MeshInstanceCount for uninstanced definition",
        |error| {
            matches!(
                error,
                StaticMeshBakeError::MeshInstanceCount {
                    mesh: 2,
                    source_mesh_index: 777,
                    instances: 0
                }
            )
        },
    );

    let mut duplicate_source_attachment = source.clone();
    duplicate_source_attachment.assets.instances[1].source_node_index = 7;
    assert_bake_error(
        &duplicate_source_attachment,
        "DuplicateNodeAttachment for duplicate source node",
        |error| {
            matches!(
                error,
                StaticMeshBakeError::DuplicateNodeAttachment {
                    source_node_index: 7
                }
            )
        },
    );

    let mut duplicate_internal_node = source.clone();
    duplicate_internal_node.assets.instances[1].node = 2;
    assert_bake_error(
        &duplicate_internal_node,
        "DuplicateSkeletonNodeAttachment for duplicate internal node",
        |error| {
            matches!(
                error,
                StaticMeshBakeError::DuplicateSkeletonNodeAttachment { node: 2 }
            )
        },
    );

    let mut missing_mesh = source.clone();
    missing_mesh.assets.instances[0].mesh = 99;
    assert_bake_error(&missing_mesh, "MissingMesh", |error| {
        matches!(
            error,
            StaticMeshBakeError::MissingMesh {
                source_node_index: 7,
                mesh: 99
            }
        )
    });

    let mut missing_node = source.clone();
    missing_node.assets.instances[0].node = 99;
    assert_bake_error(&missing_node, "MissingNode", |error| {
        matches!(
            error,
            StaticMeshBakeError::MissingNode {
                source_node_index: 7,
                node: 99
            }
        )
    });

    let mut invalid_parent = source.clone();
    invalid_parent.skeleton.bones[1].parent = Some(1);
    assert_bake_error(&invalid_parent, "InvalidParent", |error| {
        matches!(
            error,
            StaticMeshBakeError::InvalidParent { node: 1, parent: 1 }
        )
    });

    let mut bad_material_index = source.clone();
    bad_material_index.assets.meshes[0].primitives[0].material = Some(99);
    assert_bake_error(
        &bad_material_index,
        "InvalidPrimitive material_index",
        |error| {
            matches!(
                error,
                StaticMeshBakeError::InvalidPrimitive {
                    mesh: 0,
                    primitive: 0,
                    reason: "material_index"
                }
            )
        },
    );

    let mut bad_triangle_index = source.clone();
    bad_triangle_index.assets.meshes[0].primitives[0].indices[2] = 99;
    assert_bake_error(
        &bad_triangle_index,
        "InvalidPrimitive triangle_index",
        |error| {
            matches!(
                error,
                StaticMeshBakeError::InvalidPrimitive {
                    mesh: 0,
                    primitive: 0,
                    reason: "triangle_index"
                }
            )
        },
    );

    let mut bad_index_count = source.clone();
    bad_index_count.assets.meshes[0].primitives[0].indices.pop();
    assert_bake_error(
        &bad_index_count,
        "InvalidPrimitive indexed_triangle_count",
        |error| {
            matches!(
                error,
                StaticMeshBakeError::InvalidPrimitive {
                    mesh: 0,
                    primitive: 0,
                    reason: "indexed_triangle_count"
                }
            )
        },
    );

    let mut bad_normal_count = source.clone();
    bad_normal_count.assets.meshes[0].primitives[0]
        .normals
        .pop();
    assert_bake_error(
        &bad_normal_count,
        "InvalidPrimitive normal_count",
        |error| {
            matches!(
                error,
                StaticMeshBakeError::InvalidPrimitive {
                    mesh: 0,
                    primitive: 0,
                    reason: "normal_count"
                }
            )
        },
    );

    let mut bad_uv_count = source.clone();
    bad_uv_count.assets.meshes[0].primitives[0].uvs.pop();
    assert_bake_error(&bad_uv_count, "InvalidPrimitive uv_count", |error| {
        matches!(
            error,
            StaticMeshBakeError::InvalidPrimitive {
                mesh: 0,
                primitive: 0,
                reason: "uv_count"
            }
        )
    });

    let mut empty_positions = source.clone();
    empty_positions.assets.meshes[0].primitives[0]
        .positions
        .clear();
    assert_bake_error(
        &empty_positions,
        "InvalidPrimitive empty_positions",
        |error| {
            matches!(
                error,
                StaticMeshBakeError::InvalidPrimitive {
                    mesh: 0,
                    primitive: 0,
                    reason: "empty_positions"
                }
            )
        },
    );

    let mut empty_primitives = source.clone();
    empty_primitives.assets.meshes[1].primitives.clear();
    assert_bake_error(
        &empty_primitives,
        "InvalidPrimitive empty_primitives",
        |error| {
            matches!(
                error,
                StaticMeshBakeError::InvalidPrimitive {
                    mesh: 1,
                    primitive: 0,
                    reason: "empty_primitives"
                }
            )
        },
    );

    let mut nonfinite_transform = source.clone();
    nonfinite_transform.skeleton.bones[1].rest.translation.x = f32::INFINITY;
    assert_bake_error(&nonfinite_transform, "NonFiniteTransform", |error| {
        matches!(error, StaticMeshBakeError::NonFiniteTransform { node: 1 })
    });

    let mut nonfinite_position = source.clone();
    nonfinite_position.assets.meshes[0].primitives[0].positions[2].x = f32::NAN;
    assert_bake_error(
        &nonfinite_position,
        "NonFiniteAttribute position",
        |error| {
            matches!(
                error,
                StaticMeshBakeError::NonFiniteAttribute {
                    mesh: 0,
                    primitive: 0,
                    attribute: "position",
                    vertex: 2
                }
            )
        },
    );

    let mut nonfinite_normal = source.clone();
    nonfinite_normal.assets.meshes[0].primitives[0].normals[1].y = f32::INFINITY;
    assert_bake_error(&nonfinite_normal, "NonFiniteAttribute normal", |error| {
        matches!(
            error,
            StaticMeshBakeError::NonFiniteAttribute {
                mesh: 0,
                primitive: 0,
                attribute: "normal",
                vertex: 1
            }
        )
    });

    let mut nonfinite_uv = source.clone();
    nonfinite_uv.assets.meshes[0].primitives[0].uvs[3][1] = f32::NAN;
    assert_bake_error(&nonfinite_uv, "NonFiniteAttribute uv", |error| {
        matches!(
            error,
            StaticMeshBakeError::NonFiniteAttribute {
                mesh: 0,
                primitive: 0,
                attribute: "uv",
                vertex: 3
            }
        )
    });

    let mut nonfinite_base_color = source.clone();
    nonfinite_base_color.assets.materials[0].base_color[2] = f32::NAN;
    assert_bake_error(
        &nonfinite_base_color,
        "NonFiniteMaterial base_color",
        |error| {
            matches!(
                error,
                StaticMeshBakeError::NonFiniteMaterial {
                    material: 0,
                    factor: "base_color"
                }
            )
        },
    );

    let mut nonfinite_metallic = source.clone();
    nonfinite_metallic.assets.materials[0].metallic = f32::INFINITY;
    assert_bake_error(&nonfinite_metallic, "NonFiniteMaterial metallic", |error| {
        matches!(
            error,
            StaticMeshBakeError::NonFiniteMaterial {
                material: 0,
                factor: "metallic"
            }
        )
    });

    let mut nonfinite_roughness = source.clone();
    nonfinite_roughness.assets.materials[1].roughness = f32::NAN;
    assert_bake_error(
        &nonfinite_roughness,
        "NonFiniteMaterial roughness",
        |error| {
            matches!(
                error,
                StaticMeshBakeError::NonFiniteMaterial {
                    material: 1,
                    factor: "roughness"
                }
            )
        },
    );

    let mut zero_normal = source.clone();
    zero_normal.assets.meshes[0].primitives[0].normals[0] = Vec3::ZERO;
    assert_bake_error(&zero_normal, "ZeroNormal", |error| {
        matches!(
            error,
            StaticMeshBakeError::ZeroNormal {
                mesh: 0,
                primitive: 0,
                vertex: 0
            }
        )
    });

    let mut unstable_finite_normal = source.clone();
    unstable_finite_normal.assets.meshes[0].primitives[0].normals[2] = Vec3::splat(f32::MAX / 16.0);
    assert_bake_error(
        &unstable_finite_normal,
        "finite normal that cannot be stably normalized",
        |error| {
            matches!(
                error,
                StaticMeshBakeError::ZeroNormal {
                    mesh: 0,
                    primitive: 0,
                    vertex: 2
                }
            )
        },
    );

    let mut skeleton_skin = source.clone();
    skeleton_skin.skeleton.bones[3].inverse_bind = Some(Mat4::IDENTITY);
    assert_bake_error(&skeleton_skin, "SkeletonSkin", |error| {
        matches!(error, StaticMeshBakeError::SkeletonSkin { node: 3 })
    });

    let mut instance_joints = source.clone();
    instance_joints.assets.instances[0].skin_joints.push(0);
    assert_bake_error(&instance_joints, "SkinnedInstance joints", |error| {
        matches!(
            error,
            StaticMeshBakeError::SkinnedInstance {
                source_node_index: 7
            }
        )
    });

    let mut instance_ibms = source.clone();
    instance_ibms.assets.instances[1]
        .skin_ibms
        .push(Mat4::IDENTITY);
    assert_bake_error(&instance_ibms, "SkinnedInstance IBMs", |error| {
        matches!(
            error,
            StaticMeshBakeError::SkinnedInstance {
                source_node_index: 8
            }
        )
    });

    let mut primitive_joints = source.clone();
    primitive_joints.assets.meshes[0].primitives[0]
        .joints
        .push([0; 4]);
    assert_bake_error(&primitive_joints, "SkinnedPrimitive joints", |error| {
        matches!(
            error,
            StaticMeshBakeError::SkinnedPrimitive {
                mesh: 0,
                primitive: 0
            }
        )
    });

    let mut primitive_weights = source.clone();
    primitive_weights.assets.meshes[1].primitives[0]
        .weights
        .push([0.25; 4]);
    assert_bake_error(&primitive_weights, "SkinnedPrimitive weights", |error| {
        matches!(
            error,
            StaticMeshBakeError::SkinnedPrimitive {
                mesh: 1,
                primitive: 0
            }
        )
    });

    let mut animated = source.clone();
    animated.clips.push(Clip {
        name: "earlier-empty-clip".into(),
        duration_s: 0.0,
        tracks: vec![],
    });
    animated.clips.push(Clip {
        name: "later-animated-clip".into(),
        duration_s: 1.0,
        tracks: vec![Track {
            bone: 3,
            property: Property::Scale,
            interpolation: Interpolation::Step,
            times: vec![0.0, 1.0],
            values: TrackValues::Vec3s(vec![Vec3::ONE, Vec3::splat(2.0)]),
        }],
    });
    assert_bake_error(&animated, "AnimationTrack in later clip", |error| {
        matches!(
            error,
            StaticMeshBakeError::AnimationTrack {
                clip,
                node: 3,
                property: "scale"
            } if clip == "later-animated-clip"
        )
    });

    let mut animated_ancestor = source.clone();
    animated_ancestor.clips.push(Clip {
        name: "animated-ancestor".into(),
        duration_s: 1.0,
        tracks: vec![Track {
            bone: 0,
            property: Property::Translation,
            interpolation: Interpolation::Linear,
            times: vec![0.0, 1.0],
            values: TrackValues::Vec3s(vec![Vec3::ZERO, Vec3::X]),
        }],
    });
    assert_bake_error(&animated_ancestor, "animated mesh ancestor", |error| {
        matches!(
            error,
            StaticMeshBakeError::AnimationTrack {
                clip,
                node: 0,
                property: "translation"
            } if clip == "animated-ancestor"
        )
    });

    let mut singular = source.clone();
    singular.skeleton.bones[1].rest.scale.z = 0.0;
    assert_bake_error(&singular, "SingularTransform", |error| {
        matches!(
            error,
            StaticMeshBakeError::SingularTransform {
                source_node_index: 7,
                ..
            }
        )
    });

    let mut near_singular = source.clone();
    near_singular.skeleton.bones[2].rest.scale.x = 1.0e-9;
    assert_bake_error(&near_singular, "near SingularTransform", |error| {
        matches!(
            error,
            StaticMeshBakeError::SingularTransform {
                source_node_index: 7,
                ..
            }
        )
    });

    let mut reflected = source;
    reflected.skeleton.bones[2].rest.scale.x = -1.0;
    assert_bake_error(&reflected, "ReflectionTransform", |error| {
        matches!(
            error,
            StaticMeshBakeError::ReflectionTransform {
                source_node_index: 7,
                ..
            }
        )
    });
}
