//! Public-contract tests for static mesh transform baking.
//!
//! The expected vertices and normals below are calculated from the source
//! fixture matrices, not by the baking implementation.  This deliberately
//! exercises the result through the normal glTF writer and loader too.

use animsmith_core::bake_static_mesh_transforms;
use animsmith_core::model::*;
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
        Vec3::ONE,
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

    assert!(
        baked.evidence.output_root_is_identity,
        "machine-readable evidence declares the normalized output root"
    );
    assert_eq!(
        baked.evidence.entries.len(),
        source.assets.instances.len(),
        "evidence has one ordered entry per baked source instance"
    );
    let worlds = world_matrices(&source);
    for (entry, source_instance) in baked.evidence.entries.iter().zip(&source.assets.instances) {
        assert_eq!(entry.source_node_index, source_instance.source_node_index);
        assert_eq!(entry.source_mesh_ordinal, source_instance.mesh);
        assert_eq!(
            entry.source_mesh_index,
            source.assets.meshes[source_instance.mesh].source_mesh_index
        );
        assert_eq!(
            entry.primitive_count,
            source.assets.meshes[source_instance.mesh].primitives.len()
        );
        assert_eq!(
            entry.position_count,
            source.assets.meshes[source_instance.mesh]
                .primitives
                .iter()
                .map(|primitive| primitive.positions.len())
                .sum::<usize>()
        );
        assert_eq!(
            entry.world_transform,
            worlds[source_instance.node].to_cols_array(),
            "evidence records the accumulated, not merely local, matrix"
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
        for (actual, expected) in primitive.positions.iter().zip(&expected[index].0) {
            close_vec3(
                *actual,
                *expected,
                "baked position must equal source world position",
            );
        }
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
    animsmith_gltf::write::write(output, &b).expect("writes same baked GLB again");
    assert_eq!(
        std::fs::read(&a).unwrap(),
        std::fs::read(&b).unwrap(),
        "same-platform bakes are byte-identical"
    );

    let reloaded = animsmith_gltf::load(&a).expect("baked GLB reloads");
    for bone in &reloaded.skeleton.bones {
        assert_eq!(
            bone.rest,
            Transform::IDENTITY,
            "reload has no helper transform"
        );
    }
    for (index, instance) in reloaded.assets.instances.iter().enumerate() {
        let primitive = &reloaded.assets.meshes[instance.mesh].primitives[0];
        for (actual, expected) in primitive.positions.iter().zip(&expected[index].0) {
            close_vec3(
                *actual,
                *expected,
                "reloaded baked position remains world equivalent",
            );
        }
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

    let mut shared = source.clone();
    shared.assets.instances.push(MeshInstance {
        source_node_index: 99,
        node: 3,
        mesh: 0,
        ..MeshInstance::default()
    });
    assert!(
        bake_static_mesh_transforms(&shared).is_err(),
        "shared mesh definitions are ambiguous to bake"
    );

    let mut unattached = source.clone();
    unattached.assets.meshes.push(MeshAsset::default());
    assert!(
        bake_static_mesh_transforms(&unattached).is_err(),
        "uninstanced definitions are rejected rather than silently dropped"
    );

    let mut skinned = source.clone();
    skinned.assets.instances[0].skin_joints.push(0);
    assert!(
        bake_static_mesh_transforms(&skinned).is_err(),
        "skinned mesh is outside static bake contract"
    );

    let mut animated = source.clone();
    animated.clips.push(Clip {
        name: "moves-an-ancestor".into(),
        duration_s: 1.0,
        tracks: vec![Track {
            bone: 0,
            property: Property::Translation,
            interpolation: Interpolation::Linear,
            times: vec![0.0, 1.0],
            values: TrackValues::Vec3s(vec![Vec3::ZERO, Vec3::X]),
        }],
    });
    assert!(
        bake_static_mesh_transforms(&animated).is_err(),
        "animation of a mesh ancestor is not static"
    );

    let mut singular = source.clone();
    singular.skeleton.bones[1].rest.scale.z = 0.0;
    assert!(
        bake_static_mesh_transforms(&singular).is_err(),
        "singular ancestor transform fails closed"
    );

    let mut reflected = source;
    reflected.skeleton.bones[2].rest.scale.x = -1.0;
    assert!(
        bake_static_mesh_transforms(&reflected).is_err(),
        "reflection is rejected until winding semantics are explicit"
    );
}
