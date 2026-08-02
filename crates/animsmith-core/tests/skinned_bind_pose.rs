//! Analytic proof for skinned bind-pose canonicalization. The fixture uses a
//! centimetre, Z-up source transform and a mesh node distinct from its joints,
//! so the test exercises the full `joint^-1 * geometry` IBM relationship.

use animsmith_core::model::{MeshAsset, MeshInstance, Primitive, SceneAssets};
use animsmith_core::{
    Bone, Document, Skeleton, SkinnedBindPoseCanonicalizationError,
    SkinnedBindPoseCanonicalizationOptions, SkinnedBindPosePlacement, Transform,
    canonicalize_skinned_bind_pose,
};
use glam::{Mat3, Mat4, Quat, Vec3};

const EPSILON: f32 = 2.0e-5;

fn assert_vec3_close(actual: Vec3, expected: Vec3) {
    assert!(
        actual.abs_diff_eq(expected, EPSILON),
        "expected {expected:?}, got {actual:?}"
    );
}

fn assert_mat4_close(actual: Mat4, expected: Mat4) {
    for (actual, expected) in actual
        .to_cols_array()
        .into_iter()
        .zip(expected.to_cols_array())
    {
        assert!(
            (actual - expected).abs() <= EPSILON,
            "expected {expected}, got {actual}"
        );
    }
}

fn worlds(skeleton: &Skeleton) -> Vec<Mat4> {
    let mut worlds = Vec::new();
    for bone in &skeleton.bones {
        let local = bone.rest.to_mat4();
        worlds.push(match bone.parent {
            Some(parent) => worlds[parent] * local,
            None => local,
        });
    }
    worlds
}

fn source_document() -> Document {
    let skeleton = Skeleton {
        bones: vec![
            Bone {
                name: "hips".into(),
                parent: None,
                rest: Transform {
                    translation: Vec3::new(100.0, 200.0, -100.0),
                    rotation: Quat::from_rotation_y(0.3),
                    scale: Vec3::ONE,
                },
                inverse_bind: None,
            },
            Bone {
                name: "spine".into(),
                parent: Some(0),
                rest: Transform {
                    translation: Vec3::new(0.0, 50.0, 0.0),
                    ..Transform::IDENTITY
                },
                inverse_bind: None,
            },
            // The mesh node is separate from the skeleton joints, as it is in
            // many DCC exports. Its bind-world transform must be carried into
            // both position baking and source IBM validation.
            Bone {
                name: "mesh-node".into(),
                parent: None,
                rest: Transform {
                    translation: Vec3::new(-80.0, -150.0, 40.0),
                    rotation: Quat::from_rotation_z(-0.2),
                    scale: Vec3::ONE,
                },
                inverse_bind: None,
            },
        ],
    };
    let source_worlds = worlds(&skeleton);
    let geometry_world = source_worlds[2];
    let skin_ibms = [0, 1]
        .into_iter()
        .map(|joint| source_worlds[joint].inverse() * geometry_world)
        .collect();
    Document {
        skeleton,
        clips: vec![],
        assets: SceneAssets {
            meshes: vec![MeshAsset {
                name: "character".into(),
                source_mesh_index: 7,
                primitives: vec![Primitive {
                    positions: vec![
                        Vec3::new(-50.0, 0.0, 0.0),
                        Vec3::new(150.0, 0.0, 0.0),
                        Vec3::new(-50.0, 0.0, 100.0),
                    ],
                    normals: vec![Vec3::Y; 3],
                    joints: vec![[0, 1, 0, 0]; 3],
                    weights: vec![[0.75, 0.25, 0.0, 0.0]; 3],
                    ..Primitive::default()
                }],
            }],
            instances: vec![MeshInstance {
                source_node_index: 23,
                node: 2,
                mesh: 0,
                skin_joints: vec![0, 1],
                skin_ibms,
            }],
            ..SceneAssets::default()
        },
        ..Document::default()
    }
}

fn centimetres_z_up_to_meters_y_up() -> Mat4 {
    Mat4::from_scale_rotation_translation(
        Vec3::splat(0.01),
        Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2),
        Vec3::ZERO,
    )
}

#[test]
fn canonicalization_keeps_bind_geometry_joints_rest_and_ibms_consistent() {
    let source = source_document();
    let source_worlds = worlds(&source.skeleton);
    let source_geometry_world = source_worlds[2];
    let result = canonicalize_skinned_bind_pose(
        &source,
        SkinnedBindPoseCanonicalizationOptions {
            source_to_meters_y_up: centimetres_z_up_to_meters_y_up(),
            placement: SkinnedBindPosePlacement::GroundAndCenter,
        },
    )
    .expect("valid skinned bind pose canonicalizes");
    let output = &result.document;

    assert_eq!(output.skeleton.bones[0].name, "animsmith-canonical-root");
    assert_eq!(output.skeleton.bones[0].parent, None);
    assert_eq!(output.skeleton.bones[0].rest, Transform::IDENTITY);
    assert_eq!(output.assets.scenes.len(), 1);
    assert_eq!(output.assets.scenes[0].roots, vec![0]);
    assert_eq!(output.assets.default_scene, Some(0));
    assert_eq!(output.assets.instances.len(), 1);
    let instance = &output.assets.instances[0];
    assert_eq!(
        instance.node, 0,
        "canonical geometry is attached to identity"
    );
    assert_eq!(
        instance.skin_joints,
        vec![1, 2],
        "root insertion remaps joints"
    );

    let positions = &output.assets.meshes[0].primitives[0].positions;
    let min = positions.iter().copied().reduce(Vec3::min).unwrap();
    let max = positions.iter().copied().reduce(Vec3::max).unwrap();
    assert!((min.y - 0.0).abs() < EPSILON, "grounded minimum: {min:?}");
    assert!(
        ((min.x + max.x) * 0.5).abs() < EPSILON,
        "centred X: {min:?} {max:?}"
    );
    assert!(
        ((min.z + max.z) * 0.5).abs() < EPSILON,
        "centred Z: {min:?} {max:?}"
    );

    let expected_position = (result.source_world_to_canonical * source_geometry_world)
        .transform_point3(Vec3::new(-50.0, 0.0, 0.0));
    assert_vec3_close(positions[0], expected_position);
    let expected_normal =
        (Mat3::from_mat4(result.source_world_to_canonical * source_geometry_world)
            .inverse()
            .transpose()
            * Vec3::Y)
            .normalize();
    assert_vec3_close(
        output.assets.meshes[0].primitives[0].normals[0],
        expected_normal,
    );

    let output_worlds = worlds(&output.skeleton);
    assert_mat4_close(
        output_worlds[1],
        result.source_world_to_canonical * source_worlds[0],
    );
    assert_mat4_close(
        output_worlds[2],
        result.source_world_to_canonical * source_worlds[1],
    );
    for (slot, &joint) in instance.skin_joints.iter().enumerate() {
        assert_mat4_close(
            output_worlds[joint] * instance.skin_ibms[slot],
            Mat4::IDENTITY,
        );
        assert_mat4_close(
            output.skeleton.bones[joint]
                .inverse_bind
                .expect("joint fallback IBM"),
            instance.skin_ibms[slot],
        );
    }
    // At bind pose, every weighted joint palette entry is identity. Therefore
    // the skinned output position exactly equals the canonical mesh position.
    let weights = output.assets.meshes[0].primitives[0].weights[0];
    let joints = output.assets.meshes[0].primitives[0].joints[0];
    let bind_deformed = joints
        .into_iter()
        .zip(weights)
        .fold(Vec3::ZERO, |sum, (slot, weight)| {
            sum + weight
                * (output_worlds[instance.skin_joints[slot as usize]]
                    * instance.skin_ibms[slot as usize])
                    .transform_point3(positions[0])
        });
    assert_vec3_close(bind_deformed, positions[0]);
}

#[test]
fn canonicalization_is_deterministic_and_rejects_inconsistent_ibms() {
    let source = source_document();
    let options = SkinnedBindPoseCanonicalizationOptions {
        source_to_meters_y_up: centimetres_z_up_to_meters_y_up(),
        placement: SkinnedBindPosePlacement::GroundAndCenter,
    };
    let first = canonicalize_skinned_bind_pose(&source, options).unwrap();
    let second = canonicalize_skinned_bind_pose(&source, options).unwrap();
    assert_mat4_close(
        first.source_world_to_canonical,
        second.source_world_to_canonical,
    );
    assert_eq!(
        first.document.assets.meshes[0].primitives[0].positions,
        second.document.assets.meshes[0].primitives[0].positions,
    );
    assert_eq!(
        first.document.assets.instances[0].skin_joints,
        second.document.assets.instances[0].skin_joints,
    );
    assert_eq!(
        first.document.assets.instances[0].skin_ibms,
        second.document.assets.instances[0].skin_ibms,
    );

    let mut inconsistent = source;
    inconsistent.assets.instances[0].skin_ibms[1] *= Mat4::from_translation(Vec3::X);
    let error = canonicalize_skinned_bind_pose(&inconsistent, options).unwrap_err();
    assert!(matches!(
        error,
        SkinnedBindPoseCanonicalizationError::InconsistentInverseBind {
            source_node_index: 23,
            joint: 1,
        }
    ));
}

#[test]
fn canonicalization_rejects_non_uniform_coordinate_conversion() {
    let error = canonicalize_skinned_bind_pose(
        &source_document(),
        SkinnedBindPoseCanonicalizationOptions {
            source_to_meters_y_up: Mat4::from_scale(Vec3::new(0.01, 0.02, 0.01)),
            placement: SkinnedBindPosePlacement::Preserve,
        },
    )
    .unwrap_err();
    assert!(matches!(
        error,
        SkinnedBindPoseCanonicalizationError::InvalidCoordinateTransform {
            reason: "non_uniform_or_sheared"
        }
    ));
}
