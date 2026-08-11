//! Analytic proof for skinned bind-pose canonicalization. The fixture uses a
//! centimetre, Z-up source transform and a mesh node distinct from its joints,
//! so the test exercises the full `joint^-1 * geometry` IBM relationship.

use animsmith_core::model::{
    Clip, Interpolation, MeshAsset, MeshInstance, Primitive, Property, SceneAssets, Track,
    TrackValues,
};
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

#[test]
fn canonicalization_pins_the_shared_symmetric_axis_band() {
    canonicalize_skinned_bind_pose(
        &source_document(),
        SkinnedBindPoseCanonicalizationOptions {
            source_to_meters_y_up: Mat4::from_scale(Vec3::new(1.0, 1.000_15, 1.0)),
            placement: SkinnedBindPosePlacement::Preserve,
        },
    )
    .expect("the shared average-relative 1e-4 band accepts this single-axis edge");

    let error = canonicalize_skinned_bind_pose(
        &source_document(),
        SkinnedBindPoseCanonicalizationOptions {
            source_to_meters_y_up: Mat4::from_scale(Vec3::new(1.0, 1.000_16, 1.0)),
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

#[test]
fn canonicalization_preserves_preclassifier_reason_precedence() {
    let mut non_affine = Mat4::IDENTITY;
    non_affine.w_axis.x = 1.0;
    for (transform, expected_reason) in [
        (Mat4::from_scale(Vec3::new(1.0e-4, 1.0, 1.0)), "zero_scale"),
        (non_affine, "non_affine"),
    ] {
        let error = canonicalize_skinned_bind_pose(
            &source_document(),
            SkinnedBindPoseCanonicalizationOptions {
                source_to_meters_y_up: transform,
                placement: SkinnedBindPosePlacement::Preserve,
            },
        )
        .unwrap_err();
        assert!(matches!(
            error,
            SkinnedBindPoseCanonicalizationError::InvalidCoordinateTransform { reason }
                if reason == expected_reason
        ));
    }
}

#[test]
fn canonicalization_does_not_overflow_its_zero_scale_witness() {
    let mut source = source_document();
    for bone in &mut source.skeleton.bones {
        bone.rest = Transform::IDENTITY;
    }
    source.assets.instances[0].skin_ibms = vec![Mat4::IDENTITY; 2];
    // The coordinate basis itself is valid and reaches the later output-TRS
    // representability check. The former f32 length mislabeled it as
    // `InvalidCoordinateTransform { reason: "zero_scale" }` before then.
    let error = canonicalize_skinned_bind_pose(
        &source,
        SkinnedBindPoseCanonicalizationOptions {
            source_to_meters_y_up: Mat4::from_scale(Vec3::splat(2.0e19)),
            placement: SkinnedBindPosePlacement::Preserve,
        },
    )
    .unwrap_err();
    assert!(matches!(
        error,
        SkinnedBindPoseCanonicalizationError::NonTrsRoot { node: 0 }
    ));
}

#[test]
fn canonicalization_pins_large_finite_positive_and_reflected_determinants() {
    let mut source = source_document();
    for bone in &mut source.skeleton.bones {
        bone.rest = Transform::IDENTITY;
    }
    source.assets.instances[0].skin_ibms = vec![Mat4::IDENTITY; 2];

    // This is a reflected, nearly orthogonal uniform basis. Its stored f32
    // components and its exact f64 determinant are finite, but the old f32
    // determinant evaluates `-inf + inf` and becomes NaN. A `det <= 0`
    // predicate therefore admitted it. The shared widened classifier must
    // retain the negative orientation fact and its stable public reason.
    let scale = 1.0e18_f32;
    let shear = 5.0e-5_f32;
    let reflected = Mat3::from_cols(
        scale * Vec3::new(1.0, shear, 0.0),
        scale * Vec3::new(shear, 0.0, 1.0),
        scale * Vec3::new(0.0, 1.0, -shear),
    );
    assert!(reflected.to_cols_array().into_iter().all(f32::is_finite));
    assert!(reflected.determinant().is_nan());
    let widened_columns = [
        reflected.x_axis.as_dvec3(),
        reflected.y_axis.as_dvec3(),
        reflected.z_axis.as_dvec3(),
    ];
    let widened_lengths = widened_columns.map(|column| column.length());
    let widened_average = widened_lengths.into_iter().sum::<f64>() / 3.0;
    for length in widened_lengths {
        assert!(
            (length - widened_average).abs() <= f64::from(1.0e-4_f32) * widened_average.max(length)
        );
    }
    let widened_orthogonality_tolerance = f64::from(1.0e-4_f32) * widened_average * widened_average;
    for (left, right) in [(0, 1), (0, 2), (1, 2)] {
        assert!(
            widened_columns[left].dot(widened_columns[right]).abs()
                <= widened_orthogonality_tolerance
        );
    }
    let widened_determinant = widened_columns[2].dot(widened_columns[0].cross(widened_columns[1]));
    assert!(widened_determinant.is_finite());
    assert!(widened_determinant < 0.0);

    // Flipping one column preserves every magnitude and orthogonality fact
    // while making the widened orientation positive. Its f32 determinant is
    // still NaN, so the public result distinguishes the determinant's actual
    // sign from a blanket rejection of every binary32 NaN.
    let positive = Mat3::from_cols(reflected.x_axis, reflected.y_axis, -reflected.z_axis);
    assert!(positive.to_cols_array().into_iter().all(f32::is_finite));
    assert!(positive.determinant().is_nan());
    let positive_columns = [
        positive.x_axis.as_dvec3(),
        positive.y_axis.as_dvec3(),
        positive.z_axis.as_dvec3(),
    ];
    let positive_widened_determinant =
        positive_columns[2].dot(positive_columns[0].cross(positive_columns[1]));
    assert!(positive_widened_determinant.is_finite());
    assert!(positive_widened_determinant > 0.0);

    let error = canonicalize_skinned_bind_pose(
        &source,
        SkinnedBindPoseCanonicalizationOptions {
            source_to_meters_y_up: Mat4::from_mat3(positive),
            placement: SkinnedBindPosePlacement::Preserve,
        },
    )
    .unwrap_err();
    assert!(
        matches!(
            &error,
            SkinnedBindPoseCanonicalizationError::NonTrsRoot { node: 0 }
        ),
        "unexpected positive-basis result: {error:?}"
    );

    let error = canonicalize_skinned_bind_pose(
        &source,
        SkinnedBindPoseCanonicalizationOptions {
            source_to_meters_y_up: Mat4::from_mat3(reflected),
            placement: SkinnedBindPosePlacement::Preserve,
        },
    )
    .unwrap_err();
    assert!(matches!(
        error,
        SkinnedBindPoseCanonicalizationError::InvalidCoordinateTransform {
            reason: "reflection_or_singular"
        }
    ));
}

#[test]
fn canonicalization_pins_its_production_orthogonality_and_singularity_parameters() {
    let accepted_shear = Mat3::from_cols(Vec3::X, Vec3::new(5.0e-5, 1.0, 0.0), Vec3::Z);
    canonicalize_skinned_bind_pose(
        &source_document(),
        SkinnedBindPoseCanonicalizationOptions {
            source_to_meters_y_up: Mat4::from_mat3(accepted_shear),
            placement: SkinnedBindPosePlacement::Preserve,
        },
    )
    .expect("the canonical coordinate policy keeps its declared 1e-4 shear band");

    let refused_shear = Mat3::from_cols(Vec3::X, Vec3::new(1.1e-4, 1.0, 0.0), Vec3::Z);
    let error = canonicalize_skinned_bind_pose(
        &source_document(),
        SkinnedBindPoseCanonicalizationOptions {
            source_to_meters_y_up: Mat4::from_mat3(refused_shear),
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

    // This positive-determinant basis is below Appendix D's relative
    // singularity threshold, but canonicalization declares only an exact
    // zero determinant singular. Its much larger shear must therefore own
    // the established grouped rejection reason.
    let near_singular_shear = Mat3::from_cols(Vec3::X, Vec3::new(1.0, 5.0e-7, 0.0), Vec3::Z);
    let error = canonicalize_skinned_bind_pose(
        &source_document(),
        SkinnedBindPoseCanonicalizationOptions {
            source_to_meters_y_up: Mat4::from_mat3(near_singular_shear),
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

#[test]
fn canonicalization_pins_its_exact_zero_singularity_threshold() {
    // The smallest positive binary32 determinant must reach the later shear
    // check rather than being absorbed by an implicit positive singularity
    // floor.
    let positive_minimum_determinant =
        Mat3::from_cols(Vec3::X, Vec3::new(1.0, f32::from_bits(1), 0.0), Vec3::Z);
    let error = canonicalize_skinned_bind_pose(
        &source_document(),
        SkinnedBindPoseCanonicalizationOptions {
            source_to_meters_y_up: Mat4::from_mat3(positive_minimum_determinant),
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

#[test]
fn canonicalization_pins_the_shared_symmetric_shear_base() {
    let linear = Mat3::from_cols(
        Vec3::X,
        Vec3::new(9.999_319e-5, 0.999_923_9, 0.0),
        Vec3::new(0.0, 0.0, 0.999_923_9),
    );
    let columns = [
        linear.x_axis.as_dvec3(),
        linear.y_axis.as_dvec3(),
        linear.z_axis.as_dvec3(),
    ];
    let average = columns.iter().map(|column| column.length()).sum::<f64>() / 3.0;
    let dot = columns[0].dot(columns[1]).abs();
    assert!(dot <= f64::from(1.0e-4_f32));
    assert!(dot > f64::from(1.0e-4_f32) * average * average);

    let error = canonicalize_skinned_bind_pose(
        &source_document(),
        SkinnedBindPoseCanonicalizationOptions {
            source_to_meters_y_up: Mat4::from_mat3(linear),
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

#[test]
fn canonicalization_pins_f64_shear_arithmetic_at_its_public_boundary() {
    let linear = Mat3::from_cols(
        Vec3::new(1.0, f32::from_bits(0xb3b5_dd6f), 0.0),
        Vec3::new(f32::from_bits(0x38d1_e80c), 1.0, 0.0),
        Vec3::new(0.0, 0.0, f32::from_bits(0x3f80_0332)),
    );
    let f32_average =
        (linear.x_axis.length() + linear.y_axis.length() + linear.z_axis.length()) / 3.0;
    let f32_dot = linear.x_axis.dot(linear.y_axis).abs();
    let f32_tolerance = 1.0e-4_f32 * f32_average * f32_average;
    assert_eq!(f32_dot, f32_tolerance);

    let columns = [
        linear.x_axis.as_dvec3(),
        linear.y_axis.as_dvec3(),
        linear.z_axis.as_dvec3(),
    ];
    let f64_average = columns.iter().map(|column| column.length()).sum::<f64>() / 3.0;
    let f64_dot = columns[0].dot(columns[1]).abs();
    let f64_tolerance = f64::from(1.0e-4_f32) * f64_average * f64_average;
    assert!(f64_dot > f64_tolerance);

    let error = canonicalize_skinned_bind_pose(
        &source_document(),
        SkinnedBindPoseCanonicalizationOptions {
            source_to_meters_y_up: Mat4::from_mat3(linear),
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

#[test]
fn canonicalization_prioritizes_singular_coordinate_conversion_over_shear() {
    // This basis is both sheared (its first two columns are parallel) and
    // singular. The shared classifier deliberately replaces the legacy
    // canonicalization order with its singular-before-shape precedence while
    // preserving the existing grouped reason vocabulary.
    let error = canonicalize_skinned_bind_pose(
        &source_document(),
        SkinnedBindPoseCanonicalizationOptions {
            source_to_meters_y_up: Mat4::from_cols_array(&[
                1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
            ]),
            placement: SkinnedBindPosePlacement::Preserve,
        },
    )
    .unwrap_err();

    assert!(matches!(
        error,
        SkinnedBindPoseCanonicalizationError::InvalidCoordinateTransform {
            reason: "reflection_or_singular"
        }
    ));
}

#[test]
fn canonicalization_preserves_non_finite_and_reflection_reason_strings() {
    for (basis, expected_reason) in [
        (
            Mat4::from_scale(Vec3::new(f32::NAN, 1.0, 1.0)),
            "non_finite",
        ),
        (
            Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0)),
            "reflection_or_singular",
        ),
    ] {
        let error = canonicalize_skinned_bind_pose(
            &source_document(),
            SkinnedBindPoseCanonicalizationOptions {
                source_to_meters_y_up: basis,
                placement: SkinnedBindPosePlacement::Preserve,
            },
        )
        .unwrap_err();
        assert!(matches!(
            error,
            SkinnedBindPoseCanonicalizationError::InvalidCoordinateTransform { reason }
                if reason == expected_reason
        ));
    }
}

#[test]
fn canonicalization_rejects_animated_base_scenes() {
    let mut source = source_document();
    source.clips.push(Clip {
        name: "animated-base".into(),
        duration_s: 1.0,
        tracks: vec![Track {
            bone: 0,
            property: Property::Translation,
            interpolation: Interpolation::Linear,
            times: vec![0.0, 1.0],
            values: TrackValues::Vec3s(vec![Vec3::ZERO, Vec3::X]),
        }],
    });

    let error =
        canonicalize_skinned_bind_pose(&source, SkinnedBindPoseCanonicalizationOptions::default())
            .unwrap_err();
    assert!(matches!(
        error,
        SkinnedBindPoseCanonicalizationError::AnimationTrack {
            ref clip,
            node: 0,
            property: "translation",
        } if clip == "animated-base"
    ));
}

#[test]
fn canonicalization_accepts_bone_level_inverse_bind_fallback() {
    let mut source = source_document();
    let explicit_ibms = std::mem::take(&mut source.assets.instances[0].skin_ibms);
    for (&joint, inverse_bind) in source.assets.instances[0]
        .skin_joints
        .iter()
        .zip(&explicit_ibms)
    {
        source.skeleton.bones[joint].inverse_bind = Some(*inverse_bind);
    }

    let result = canonicalize_skinned_bind_pose(
        &source,
        SkinnedBindPoseCanonicalizationOptions {
            source_to_meters_y_up: centimetres_z_up_to_meters_y_up(),
            placement: SkinnedBindPosePlacement::Preserve,
        },
    )
    .expect("bone-level inverse binds are a supported input representation");
    let output = &result.document;
    let instance = &output.assets.instances[0];
    assert_eq!(instance.skin_ibms.len(), instance.skin_joints.len());
    let output_worlds = worlds(&output.skeleton);
    for (slot, &joint) in instance.skin_joints.iter().enumerate() {
        assert_mat4_close(
            output_worlds[joint] * instance.skin_ibms[slot],
            Mat4::IDENTITY,
        );
        assert_eq!(
            output.skeleton.bones[joint].inverse_bind,
            Some(instance.skin_ibms[slot])
        );
    }
}

#[test]
fn canonicalization_rejects_malformed_skin_inputs() {
    let options = SkinnedBindPoseCanonicalizationOptions::default();

    let mut unskinned = source_document();
    unskinned.assets.instances[0].skin_joints.clear();
    unskinned.assets.instances[0].skin_ibms.clear();
    assert!(matches!(
        canonicalize_skinned_bind_pose(&unskinned, options),
        Err(SkinnedBindPoseCanonicalizationError::UnskinnedInstance {
            source_node_index: 23
        })
    ));

    let mut missing_inverse_bind = source_document();
    missing_inverse_bind.assets.instances[0].skin_ibms.clear();
    assert!(matches!(
        canonicalize_skinned_bind_pose(&missing_inverse_bind, options),
        Err(SkinnedBindPoseCanonicalizationError::MissingInverseBind {
            source_node_index: 23,
            joint: 0,
        })
    ));

    let mut invalid_weights = source_document();
    invalid_weights.assets.meshes[0].primitives[0].weights[0] = [1.0, 1.0, 0.0, 0.0];
    assert!(matches!(
        canonicalize_skinned_bind_pose(&invalid_weights, options),
        Err(SkinnedBindPoseCanonicalizationError::InvalidPrimitive {
            mesh: 0,
            primitive: 0,
            reason: "skin_weights",
        })
    ));
}
