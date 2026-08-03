//! Bind-pose canonicalization for skinned character geometry.
//!
//! This module deliberately works on an unanimated base document. It turns
//! every skinned mesh into world-space bind geometry, moves the skeleton by the
//! same declared coordinate transform, and regenerates inverse bind matrices.

use crate::model::{
    Bone, Document, MeshAsset, MeshInstance, SceneAsset, SceneAssets, Skeleton,
    SourceSkeletonAssets, Transform,
};
use glam::{Mat3, Mat4, Vec3};
use std::collections::BTreeSet;

const MATRIX_EPSILON: f32 = 1.0e-4;
const MIN_RELATIVE_DETERMINANT: f32 = 1.0e-6;

/// Deterministic placement applied after the declared coordinate conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SkinnedBindPosePlacement {
    /// Retain converted bind-pose coordinates.
    #[default]
    Preserve,
    /// Place the bind-pose bounds on `Y = 0` and centre their X/Z midpoint.
    GroundAndCenter,
}

/// Input for [`canonicalize_skinned_bind_pose`].
#[derive(Debug, Clone, Copy)]
pub struct SkinnedBindPoseCanonicalizationOptions {
    /// Affine, orientation-preserving uniform-scale transform from input space
    /// into right-handed, Y-up metres. Identity explicitly declares that the
    /// document is already in that space.
    pub source_to_meters_y_up: Mat4,
    /// Optional deterministic placement in converted bind-pose space.
    pub placement: SkinnedBindPosePlacement,
}

impl Default for SkinnedBindPoseCanonicalizationOptions {
    fn default() -> Self {
        Self {
            source_to_meters_y_up: Mat4::IDENTITY,
            placement: SkinnedBindPosePlacement::Preserve,
        }
    }
}

/// Result of [`canonicalize_skinned_bind_pose`].
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct SkinnedBindPoseCanonicalization {
    /// Canonical document with one identity scene root and explicit skin IBMs.
    pub document: Document,
    /// Complete transform applied to source-world bind geometry.
    pub source_world_to_canonical: Mat4,
    /// Converted bind-pose bounds before optional placement.
    pub converted_bounds_min: Vec3,
    /// Converted bind-pose bounds before optional placement.
    pub converted_bounds_max: Vec3,
}

/// Rejection reason returned by [`canonicalize_skinned_bind_pose`].
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SkinnedBindPoseCanonicalizationError {
    /// The document has no mesh attachments to canonicalize.
    #[error("cannot canonicalize skinned bind pose: document has no mesh instances")]
    NoInstances,
    /// The operation accepts an unanimated base scene only.
    #[error(
        "cannot canonicalize skinned bind pose: clip {clip:?} animates node {node} ({property})"
    )]
    AnimationTrack {
        /// Clip name.
        clip: String,
        /// Target node.
        node: usize,
        /// Animated property name.
        property: &'static str,
    },
    /// The caller supplied an unsafe coordinate conversion.
    #[error("cannot canonicalize skinned bind pose: source_to_meters_y_up is invalid ({reason})")]
    InvalidCoordinateTransform {
        /// Stable machine-readable rejection reason.
        reason: &'static str,
    },
    /// The skeleton is not in parent-before-child order.
    #[error(
        "cannot canonicalize skinned bind pose: skeleton node {node} has invalid parent {parent}"
    )]
    InvalidParent {
        /// Node ordinal.
        node: usize,
        /// Invalid parent ordinal.
        parent: usize,
    },
    /// A skeleton rest transform or accumulated world matrix is non-finite.
    #[error(
        "cannot canonicalize skinned bind pose: skeleton node {node} has a non-finite transform"
    )]
    NonFiniteTransform {
        /// Node ordinal.
        node: usize,
    },
    /// A converted skeleton root cannot be represented as node-local TRS.
    #[error(
        "cannot canonicalize skinned bind pose: skeleton root {node} cannot be represented as TRS"
    )]
    NonTrsRoot {
        /// Node ordinal.
        node: usize,
    },
    /// A mesh attachment references missing model data.
    #[error(
        "cannot canonicalize skinned bind pose: source node {source_node_index} references missing {kind} {index}"
    )]
    MissingReference {
        /// Source node identity supplied by the loader.
        source_node_index: usize,
        /// Referenced model collection.
        kind: &'static str,
        /// Missing ordinal.
        index: usize,
    },
    /// A mesh attachment is not skinned. Select/remove props first.
    #[error(
        "cannot canonicalize skinned bind pose: source node {source_node_index} is not skinned"
    )]
    UnskinnedInstance {
        /// Source node identity supplied by the loader.
        source_node_index: usize,
    },
    /// An explicit IBM array does not parallel its joint list.
    #[error(
        "cannot canonicalize skinned bind pose: source node {source_node_index} has {ibms} inverse bind matrices for {joints} joints"
    )]
    InverseBindCount {
        /// Source node identity supplied by the loader.
        source_node_index: usize,
        /// Number of inverse bind matrices.
        ibms: usize,
        /// Number of skin joints.
        joints: usize,
    },
    /// A skin relies on a bone-level inverse bind that is absent.
    #[error(
        "cannot canonicalize skinned bind pose: source node {source_node_index} joint {joint} has no inverse bind matrix"
    )]
    MissingInverseBind {
        /// Source node identity supplied by the loader.
        source_node_index: usize,
        /// Joint node ordinal.
        joint: usize,
    },
    /// An IBM disagrees with `joint_bind_world^-1 * geometry_bind_world`.
    #[error(
        "cannot canonicalize skinned bind pose: source node {source_node_index} joint {joint} has an inconsistent inverse bind matrix"
    )]
    InconsistentInverseBind {
        /// Source node identity supplied by the loader.
        source_node_index: usize,
        /// Joint node ordinal.
        joint: usize,
    },
    /// A joint's rest-world matrix cannot be inverted reliably.
    #[error(
        "cannot canonicalize skinned bind pose: joint {joint} has a singular or near-singular rest transform"
    )]
    SingularJoint {
        /// Joint node ordinal.
        joint: usize,
    },
    /// A mesh bind-world transform cannot safely be baked while preserving
    /// triangle indices and winding.
    #[error(
        "cannot canonicalize skinned bind pose: source node {source_node_index} has {reason} geometry transform"
    )]
    InvalidGeometryTransform {
        /// Source node identity supplied by the loader.
        source_node_index: usize,
        /// Stable machine-readable rejection reason.
        reason: &'static str,
    },
    /// A primitive is malformed for a skinned triangle mesh.
    #[error(
        "cannot canonicalize skinned bind pose: mesh {mesh} primitive {primitive} is invalid ({reason})"
    )]
    InvalidPrimitive {
        /// Mesh ordinal.
        mesh: usize,
        /// Primitive ordinal within the mesh.
        primitive: usize,
        /// Stable machine-readable rejection reason.
        reason: &'static str,
    },
    /// A vertex attribute is non-finite.
    #[error(
        "cannot canonicalize skinned bind pose: mesh {mesh} primitive {primitive} has non-finite {attribute} at vertex {vertex}"
    )]
    NonFiniteAttribute {
        /// Mesh ordinal.
        mesh: usize,
        /// Primitive ordinal.
        primitive: usize,
        /// Attribute name.
        attribute: &'static str,
        /// Vertex ordinal.
        vertex: usize,
    },
    /// A normal cannot be normalized after the geometry transform.
    #[error(
        "cannot canonicalize skinned bind pose: mesh {mesh} primitive {primitive} has zero normal at vertex {vertex}"
    )]
    ZeroNormal {
        /// Mesh ordinal.
        mesh: usize,
        /// Primitive ordinal.
        primitive: usize,
        /// Vertex ordinal.
        vertex: usize,
    },
}

#[derive(Debug, Clone, Copy)]
struct Plan {
    instance: usize,
    geometry_world: Mat4,
}

/// Canonicalize an unanimated skinned base document into right-handed Y-up metres.
///
/// The caller supplies a finite affine, positive-determinant, uniform-scale
/// `source_to_meters_y_up` matrix. Input skins must satisfy
/// `IBM = joint_bind_world^-1 * geometry_bind_world`. The output has one
/// identity scene root; each private output mesh holds canonical bind-world
/// positions and inverse-transpose normalized normals, while its IBMs are
/// regenerated as `canonical_joint_bind_world^-1`. This makes vertex data,
/// joints, rest pose, and inverse binds mutually consistent.
///
/// Animation tracks, unskinned instances, malformed skin attributes,
/// non-finite data, reflections or singular geometry transforms, non-TRS
/// coordinate conversions, singular joints, and inconsistent inverse bind
/// matrices are rejected. The function does not infer source axes: identity
/// explicitly asserts the source is already right-handed Y-up metres.
pub fn canonicalize_skinned_bind_pose(
    doc: &Document,
    options: SkinnedBindPoseCanonicalizationOptions,
) -> Result<SkinnedBindPoseCanonicalization, SkinnedBindPoseCanonicalizationError> {
    validate_coordinate_transform(options.source_to_meters_y_up)?;
    let source_worlds = world_matrices(&doc.skeleton)?;
    let plans = validate(doc, &source_worlds)?;
    let (bounds_min, bounds_max) = converted_bounds(doc, &plans, options.source_to_meters_y_up)?;
    let placement = placement_transform(options.placement, bounds_min, bounds_max);
    let source_world_to_canonical = placement * options.source_to_meters_y_up;
    let mut skeleton = canonical_skeleton(&doc.skeleton, source_world_to_canonical)?;
    let canonical_worlds = world_matrices(&skeleton)?;
    let mut meshes = Vec::with_capacity(plans.len());
    let mut instances = Vec::with_capacity(plans.len());
    let mut joints_with_skins = BTreeSet::new();

    for plan in plans {
        let input = &doc.assets.instances[plan.instance];
        let output_mesh = meshes.len();
        let geometry_transform = source_world_to_canonical * plan.geometry_world;
        meshes.push(transform_mesh(
            &doc.assets.meshes[input.mesh],
            input.mesh,
            geometry_transform,
        )?);
        let skin_joints: Vec<usize> = input.skin_joints.iter().map(|joint| joint + 1).collect();
        let skin_ibms = skin_joints
            .iter()
            .map(|&joint| {
                let inverse = inverse_joint_matrix(canonical_worlds[joint], joint)?;
                joints_with_skins.insert(joint);
                Ok(inverse)
            })
            .collect::<Result<Vec<_>, _>>()?;
        instances.push(MeshInstance {
            source_node_index: input.source_node_index,
            node: 0,
            mesh: output_mesh,
            skin_joints,
            skin_ibms,
        });
    }
    for joint in joints_with_skins {
        skeleton.bones[joint].inverse_bind =
            Some(inverse_joint_matrix(canonical_worlds[joint], joint)?);
    }
    Ok(SkinnedBindPoseCanonicalization {
        document: Document {
            skeleton,
            clips: doc.clips.clone(),
            assets: SceneAssets {
                meshes,
                instances,
                materials: doc.assets.materials.clone(),
                material_resources: doc.assets.material_resources.clone(),
                scenes: vec![SceneAsset {
                    source_scene_index: 0,
                    name: None,
                    roots: vec![0],
                }],
                default_scene: Some(0),
                source_skeleton: SourceSkeletonAssets::default(),
            },
            source: doc.source.clone(),
        },
        source_world_to_canonical,
        converted_bounds_min: bounds_min,
        converted_bounds_max: bounds_max,
    })
}

fn validate_coordinate_transform(
    transform: Mat4,
) -> Result<(), SkinnedBindPoseCanonicalizationError> {
    if !matrix4_is_finite(transform) {
        return Err(
            SkinnedBindPoseCanonicalizationError::InvalidCoordinateTransform {
                reason: "non_finite",
            },
        );
    }
    if !transform.w_axis.abs_diff_eq(glam::Vec4::W, MATRIX_EPSILON) {
        return Err(
            SkinnedBindPoseCanonicalizationError::InvalidCoordinateTransform {
                reason: "non_affine",
            },
        );
    }
    let linear = Mat3::from_mat4(transform);
    let columns = [linear.x_axis, linear.y_axis, linear.z_axis];
    let scale = columns[0].length();
    if !scale.is_finite() || scale <= MATRIX_EPSILON {
        return Err(
            SkinnedBindPoseCanonicalizationError::InvalidCoordinateTransform {
                reason: "zero_scale",
            },
        );
    }
    let tolerance = MATRIX_EPSILON * scale * scale;
    if columns
        .iter()
        .any(|column| (column.length() - scale).abs() > MATRIX_EPSILON * scale)
        || columns[0].dot(columns[1]).abs() > tolerance
        || columns[0].dot(columns[2]).abs() > tolerance
        || columns[1].dot(columns[2]).abs() > tolerance
    {
        return Err(
            SkinnedBindPoseCanonicalizationError::InvalidCoordinateTransform {
                reason: "non_uniform_or_sheared",
            },
        );
    }
    if linear.determinant() <= 0.0 {
        return Err(
            SkinnedBindPoseCanonicalizationError::InvalidCoordinateTransform {
                reason: "reflection_or_singular",
            },
        );
    }
    Ok(())
}

fn validate(
    doc: &Document,
    worlds: &[Mat4],
) -> Result<Vec<Plan>, SkinnedBindPoseCanonicalizationError> {
    if doc.assets.instances.is_empty() {
        return Err(SkinnedBindPoseCanonicalizationError::NoInstances);
    }
    for clip in &doc.clips {
        if let Some(track) = clip.tracks.first() {
            return Err(SkinnedBindPoseCanonicalizationError::AnimationTrack {
                clip: clip.name.clone(),
                node: track.bone,
                property: track.property.as_str(),
            });
        }
    }
    let mut plans = Vec::with_capacity(doc.assets.instances.len());
    for (instance_ordinal, instance) in doc.assets.instances.iter().enumerate() {
        let mesh = doc.assets.meshes.get(instance.mesh).ok_or(
            SkinnedBindPoseCanonicalizationError::MissingReference {
                source_node_index: instance.source_node_index,
                kind: "mesh",
                index: instance.mesh,
            },
        )?;
        // Keep the attachment-node reference structurally valid, but derive
        // geometry bind space from the skin equation itself. FBX geometric
        // transform helper nodes can make the attachment's ordinary rest
        // world differ from the geometry-to-world matrix encoded by every
        // inverse bind.
        worlds.get(instance.node).ok_or(
            SkinnedBindPoseCanonicalizationError::MissingReference {
                source_node_index: instance.source_node_index,
                kind: "node",
                index: instance.node,
            },
        )?;
        if instance.skin_joints.is_empty() {
            return Err(SkinnedBindPoseCanonicalizationError::UnskinnedInstance {
                source_node_index: instance.source_node_index,
            });
        }
        if !instance.skin_ibms.is_empty() && instance.skin_ibms.len() != instance.skin_joints.len()
        {
            return Err(SkinnedBindPoseCanonicalizationError::InverseBindCount {
                source_node_index: instance.source_node_index,
                ibms: instance.skin_ibms.len(),
                joints: instance.skin_joints.len(),
            });
        }
        let mut geometry_world = None;
        for (slot, &joint) in instance.skin_joints.iter().enumerate() {
            let joint_world = *worlds.get(joint).ok_or(
                SkinnedBindPoseCanonicalizationError::MissingReference {
                    source_node_index: instance.source_node_index,
                    kind: "joint",
                    index: joint,
                },
            )?;
            let actual = if instance.skin_ibms.is_empty() {
                doc.skeleton.bones[joint].inverse_bind.ok_or(
                    SkinnedBindPoseCanonicalizationError::MissingInverseBind {
                        source_node_index: instance.source_node_index,
                        joint,
                    },
                )?
            } else {
                instance.skin_ibms[slot]
            };
            let candidate = joint_world * actual;
            if !matrix4_is_finite(actual)
                || !matrix4_is_finite(candidate)
                || geometry_world
                    .is_some_and(|geometry| !matrix_approximately_equal(candidate, geometry))
            {
                return Err(
                    SkinnedBindPoseCanonicalizationError::InconsistentInverseBind {
                        source_node_index: instance.source_node_index,
                        joint,
                    },
                );
            }
            geometry_world.get_or_insert(candidate);
        }
        let Some(geometry_world) = geometry_world else {
            return Err(SkinnedBindPoseCanonicalizationError::UnskinnedInstance {
                source_node_index: instance.source_node_index,
            });
        };
        validate_geometry_transform(geometry_world, instance.source_node_index)?;
        validate_mesh(mesh, instance.mesh, instance)?;
        plans.push(Plan {
            instance: instance_ordinal,
            geometry_world,
        });
    }
    plans.sort_unstable_by_key(|plan| {
        let instance = &doc.assets.instances[plan.instance];
        (instance.source_node_index, plan.instance)
    });
    Ok(plans)
}

fn world_matrices(skeleton: &Skeleton) -> Result<Vec<Mat4>, SkinnedBindPoseCanonicalizationError> {
    let mut worlds = Vec::with_capacity(skeleton.bones.len());
    for (node, bone) in skeleton.bones.iter().enumerate() {
        let local = bone.rest.to_mat4();
        if !matrix4_is_finite(local) {
            return Err(SkinnedBindPoseCanonicalizationError::NonFiniteTransform { node });
        }
        let world = match bone.parent {
            Some(parent) if parent < node => worlds[parent] * local,
            Some(parent) => {
                return Err(SkinnedBindPoseCanonicalizationError::InvalidParent { node, parent });
            }
            None => local,
        };
        if !matrix4_is_finite(world) {
            return Err(SkinnedBindPoseCanonicalizationError::NonFiniteTransform { node });
        }
        worlds.push(world);
    }
    Ok(worlds)
}

fn inverse_joint_matrix(
    world: Mat4,
    joint: usize,
) -> Result<Mat4, SkinnedBindPoseCanonicalizationError> {
    let linear = Mat3::from_mat4(world);
    let scale = linear
        .to_cols_array()
        .into_iter()
        .fold(0.0_f32, |largest, component| largest.max(component.abs()));
    let determinant = linear.determinant();
    if !determinant.is_finite()
        || scale == 0.0
        || determinant.abs() <= MIN_RELATIVE_DETERMINANT * scale * scale * scale
    {
        return Err(SkinnedBindPoseCanonicalizationError::SingularJoint { joint });
    }
    let inverse = world.inverse();
    if !matrix4_is_finite(inverse) {
        return Err(SkinnedBindPoseCanonicalizationError::SingularJoint { joint });
    }
    Ok(inverse)
}

fn validate_geometry_transform(
    world: Mat4,
    source_node_index: usize,
) -> Result<(), SkinnedBindPoseCanonicalizationError> {
    let linear = Mat3::from_mat4(world);
    let scale = linear
        .to_cols_array()
        .into_iter()
        .fold(0.0_f32, |largest, component| largest.max(component.abs()));
    let determinant = linear.determinant();
    if !determinant.is_finite()
        || scale == 0.0
        || determinant.abs() <= MIN_RELATIVE_DETERMINANT * scale * scale * scale
    {
        return Err(
            SkinnedBindPoseCanonicalizationError::InvalidGeometryTransform {
                source_node_index,
                reason: "singular_or_near_singular",
            },
        );
    }
    if determinant < 0.0 {
        return Err(
            SkinnedBindPoseCanonicalizationError::InvalidGeometryTransform {
                source_node_index,
                reason: "reflection",
            },
        );
    }
    Ok(())
}

fn validate_mesh(
    mesh: &MeshAsset,
    mesh_ordinal: usize,
    instance: &MeshInstance,
) -> Result<(), SkinnedBindPoseCanonicalizationError> {
    if mesh.primitives.is_empty() {
        return Err(invalid_primitive(mesh_ordinal, 0, "empty_primitives"));
    }
    for (primitive_ordinal, primitive) in mesh.primitives.iter().enumerate() {
        let invalid = |reason| invalid_primitive(mesh_ordinal, primitive_ordinal, reason);
        if primitive.positions.is_empty() {
            return Err(invalid("empty_positions"));
        }
        if !primitive.normals.is_empty() && primitive.normals.len() != primitive.positions.len() {
            return Err(invalid("normal_count"));
        }
        if primitive.joints.len() != primitive.positions.len()
            || primitive.weights.len() != primitive.positions.len()
        {
            return Err(invalid("skin_attribute_count"));
        }
        if primitive.indices.is_empty() {
            if !primitive.positions.len().is_multiple_of(3) {
                return Err(invalid("unindexed_triangle_count"));
            }
        } else if !primitive.indices.len().is_multiple_of(3)
            || primitive
                .indices
                .iter()
                .any(|&index| index as usize >= primitive.positions.len())
        {
            return Err(invalid("triangle_indices"));
        }
        for (vertex, position) in primitive.positions.iter().enumerate() {
            if !position.is_finite() {
                return Err(non_finite(
                    mesh_ordinal,
                    primitive_ordinal,
                    "position",
                    vertex,
                ));
            }
        }
        for (vertex, normal) in primitive.normals.iter().enumerate() {
            if !normal.is_finite() {
                return Err(non_finite(
                    mesh_ordinal,
                    primitive_ordinal,
                    "normal",
                    vertex,
                ));
            }
            if normal.length_squared() == 0.0 {
                return Err(SkinnedBindPoseCanonicalizationError::ZeroNormal {
                    mesh: mesh_ordinal,
                    primitive: primitive_ordinal,
                    vertex,
                });
            }
        }
        for (vertex, weights) in primitive.weights.iter().enumerate() {
            if weights.iter().any(|weight| !weight.is_finite()) {
                return Err(non_finite(
                    mesh_ordinal,
                    primitive_ordinal,
                    "weight",
                    vertex,
                ));
            }
            if weights.iter().any(|weight| *weight < 0.0)
                || (weights.iter().sum::<f32>() - 1.0).abs() > MATRIX_EPSILON
            {
                return Err(invalid("skin_weights"));
            }
            if primitive.joints[vertex]
                .iter()
                .zip(weights)
                .any(|(&joint_slot, &weight)| {
                    weight != 0.0 && joint_slot as usize >= instance.skin_joints.len()
                })
            {
                return Err(invalid("joint_index"));
            }
        }
    }
    Ok(())
}

fn converted_bounds(
    doc: &Document,
    plans: &[Plan],
    source_to_meters_y_up: Mat4,
) -> Result<(Vec3, Vec3), SkinnedBindPoseCanonicalizationError> {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for plan in plans {
        let instance = &doc.assets.instances[plan.instance];
        let transform = source_to_meters_y_up * plan.geometry_world;
        for (primitive_ordinal, primitive) in doc.assets.meshes[instance.mesh]
            .primitives
            .iter()
            .enumerate()
        {
            for (vertex, position) in primitive.positions.iter().enumerate() {
                let converted = transform.transform_point3(*position);
                if !converted.is_finite() {
                    return Err(non_finite(
                        instance.mesh,
                        primitive_ordinal,
                        "converted_position",
                        vertex,
                    ));
                }
                min = min.min(converted);
                max = max.max(converted);
            }
        }
    }
    Ok((min, max))
}

fn placement_transform(placement: SkinnedBindPosePlacement, min: Vec3, max: Vec3) -> Mat4 {
    match placement {
        SkinnedBindPosePlacement::Preserve => Mat4::IDENTITY,
        SkinnedBindPosePlacement::GroundAndCenter => Mat4::from_translation(Vec3::new(
            -(min.x + max.x) * 0.5,
            -min.y,
            -(min.z + max.z) * 0.5,
        )),
    }
}

fn canonical_skeleton(
    source: &Skeleton,
    transform: Mat4,
) -> Result<Skeleton, SkinnedBindPoseCanonicalizationError> {
    let mut bones = Vec::with_capacity(source.bones.len() + 1);
    bones.push(Bone {
        name: "animsmith-canonical-root".to_string(),
        parent: None,
        rest: Transform::IDENTITY,
        inverse_bind: None,
    });
    for (node, source_bone) in source.bones.iter().enumerate() {
        let (parent, rest) = match source_bone.parent {
            Some(parent) => (Some(parent + 1), source_bone.rest),
            None => (
                Some(0),
                transform_to_trs(transform * source_bone.rest.to_mat4())
                    .ok_or(SkinnedBindPoseCanonicalizationError::NonTrsRoot { node })?,
            ),
        };
        bones.push(Bone {
            name: source_bone.name.clone(),
            parent,
            rest,
            inverse_bind: None,
        });
    }
    Ok(Skeleton { bones })
}

fn transform_mesh(
    mesh: &MeshAsset,
    mesh_ordinal: usize,
    transform: Mat4,
) -> Result<MeshAsset, SkinnedBindPoseCanonicalizationError> {
    let normal_transform = Mat3::from_mat4(transform).inverse().transpose();
    if !matrix3_is_finite(normal_transform) {
        return Err(
            SkinnedBindPoseCanonicalizationError::InvalidCoordinateTransform {
                reason: "normal_matrix",
            },
        );
    }
    let mut output = mesh.clone();
    for (primitive_ordinal, primitive) in output.primitives.iter_mut().enumerate() {
        for (vertex, position) in primitive.positions.iter_mut().enumerate() {
            *position = transform.transform_point3(*position);
            if !position.is_finite() {
                return Err(non_finite(
                    mesh_ordinal,
                    primitive_ordinal,
                    "canonical_position",
                    vertex,
                ));
            }
        }
        for (vertex, normal) in primitive.normals.iter_mut().enumerate() {
            let Some(normalized) = (normal_transform * *normal).try_normalize() else {
                return Err(SkinnedBindPoseCanonicalizationError::ZeroNormal {
                    mesh: mesh_ordinal,
                    primitive: primitive_ordinal,
                    vertex,
                });
            };
            *normal = normalized;
        }
    }
    Ok(output)
}

fn transform_to_trs(matrix: Mat4) -> Option<Transform> {
    if !matrix4_is_finite(matrix) {
        return None;
    }
    let (scale, rotation, translation) = matrix.to_scale_rotation_translation();
    if !scale.is_finite() || !rotation.is_finite() || !translation.is_finite() {
        return None;
    }
    let transform = Transform {
        translation,
        rotation,
        scale,
    };
    matrix_approximately_equal(matrix, transform.to_mat4()).then_some(transform)
}

fn invalid_primitive(
    mesh: usize,
    primitive: usize,
    reason: &'static str,
) -> SkinnedBindPoseCanonicalizationError {
    SkinnedBindPoseCanonicalizationError::InvalidPrimitive {
        mesh,
        primitive,
        reason,
    }
}

fn non_finite(
    mesh: usize,
    primitive: usize,
    attribute: &'static str,
    vertex: usize,
) -> SkinnedBindPoseCanonicalizationError {
    SkinnedBindPoseCanonicalizationError::NonFiniteAttribute {
        mesh,
        primitive,
        attribute,
        vertex,
    }
}

fn matrix_approximately_equal(left: Mat4, right: Mat4) -> bool {
    left.to_cols_array()
        .into_iter()
        .zip(right.to_cols_array())
        .all(|(left, right)| {
            (left - right).abs() <= MATRIX_EPSILON * left.abs().max(right.abs()).max(1.0)
        })
}

fn matrix4_is_finite(matrix: Mat4) -> bool {
    matrix
        .to_cols_array()
        .into_iter()
        .all(|component| component.is_finite())
}

fn matrix3_is_finite(matrix: Mat3) -> bool {
    matrix
        .to_cols_array()
        .into_iter()
        .all(|component| component.is_finite())
}
