//! Canonical baking of static mesh node transforms into vertex data.
//!
//! The operation is deliberately narrow: it produces a static-only scene
//! with identity node transforms. Animations, skins, reflections, malformed
//! primitive data, and ill-conditioned transforms are rejected rather than
//! being silently rewritten with different semantics.

use crate::model::{
    Bone, Document, MeshAsset, MeshInstance, Primitive, SceneAsset, SceneAssets, Skeleton,
    Transform,
};
use glam::{Mat3, Mat4};
use serde::Serialize;
use std::collections::HashSet;

/// Relative determinant threshold below which a linear transform is too
/// ill-conditioned to invert reliably for normal transformation.
const MIN_RELATIVE_DETERMINANT: f32 = 1.0e-6;

/// Result of [`bake_static_mesh_transforms`].
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct StaticMeshBake {
    /// Canonical static document with one identity root and one identity mesh
    /// child per input mesh instance.
    pub document: Document,
    /// Deterministic evidence describing every baked input instance.
    pub evidence: StaticMeshBakeEvidence,
}

/// Machine-readable evidence emitted by a static mesh bake.
#[derive(Debug, Clone, Serialize)]
#[non_exhaustive]
pub struct StaticMeshBakeEvidence {
    /// Baked instances in input instance order.
    pub entries: Vec<StaticMeshBakeInstanceEvidence>,
}

/// Evidence for one input mesh-node attachment.
#[derive(Debug, Clone, Serialize)]
#[non_exhaustive]
pub struct StaticMeshBakeInstanceEvidence {
    /// Source-format node index of the mesh attachment.
    pub source_node_index: usize,
    /// Source node name from the input skeleton.
    pub source_node_name: String,
    /// Ordinal of the input mesh definition in [`SceneAssets::meshes`].
    pub source_mesh_ordinal: usize,
    /// Stable mesh-definition identity supplied by the source loader.
    pub source_mesh_index: usize,
    /// Source mesh-definition name.
    pub source_mesh_name: String,
    /// Node ordinal in the canonical output document.
    pub output_node_index: usize,
    /// Mesh ordinal in the canonical output document.
    pub output_mesh_index: usize,
    /// Accumulated rest/world matrix in column-major order.
    pub world_transform: [f32; 16],
    /// Determinant of the baked linear transform.
    pub linear_determinant: f32,
    /// Number of primitives copied into the output mesh.
    pub primitive_count: usize,
    /// Number of positions copied across all output primitives.
    pub position_count: usize,
    /// Number of normals copied across all output primitives.
    pub normal_count: usize,
}

/// Rejection reason returned by [`bake_static_mesh_transforms`].
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum StaticMeshBakeError {
    /// The input carries no mesh-node attachments to bake.
    #[error("cannot bake static transforms: document has no mesh instances")]
    NoInstances,
    /// An input mesh definition is not attached to exactly one mesh node.
    #[error(
        "cannot bake static transforms: mesh {mesh} (source mesh {source_mesh_index}) has {instances} instances; each mesh definition must have exactly one"
    )]
    MeshInstanceCount {
        /// Ordinal in [`SceneAssets::meshes`].
        mesh: usize,
        /// Source mesh identity.
        source_mesh_index: usize,
        /// Number of input instances that reference the definition.
        instances: usize,
    },
    /// A mesh instance references a mesh definition that does not exist.
    #[error(
        "cannot bake static transforms: source node {source_node_index} references missing mesh {mesh}"
    )]
    MissingMesh {
        /// Source node identity from the instance.
        source_node_index: usize,
        /// Missing mesh ordinal.
        mesh: usize,
    },
    /// A mesh instance references a skeleton node that does not exist.
    #[error(
        "cannot bake static transforms: source node {source_node_index} maps to missing skeleton node {node}"
    )]
    MissingNode {
        /// Source node identity from the instance.
        source_node_index: usize,
        /// Missing skeleton node ordinal.
        node: usize,
    },
    /// More than one model instance claims one source mesh node.
    #[error(
        "cannot bake static transforms: source node {source_node_index} has duplicate mesh attachments"
    )]
    DuplicateNodeAttachment {
        /// Source node identity claimed more than once.
        source_node_index: usize,
    },
    /// More than one model instance attaches a mesh to one internal skeleton node.
    #[error("cannot bake static transforms: skeleton node {node} has duplicate mesh attachments")]
    DuplicateSkeletonNodeAttachment {
        /// Internal skeleton node claimed more than once.
        node: usize,
    },
    /// The input contains animation data, which a static-only bake cannot retain.
    #[error(
        "cannot bake static transforms: clip {clip:?} animates skeleton node {node} ({property})"
    )]
    AnimationTrack {
        /// Clip name.
        clip: String,
        /// Target skeleton node.
        node: usize,
        /// Animated property name.
        property: &'static str,
    },
    /// A skeleton carries inverse-bind data, indicating a skin domain.
    #[error("cannot bake static transforms: skeleton node {node} carries an inverse bind matrix")]
    SkeletonSkin {
        /// Skeleton node carrying inverse-bind data.
        node: usize,
    },
    /// A mesh instance carries skin joints or inverse bind matrices.
    #[error("cannot bake static transforms: source node {source_node_index} is skinned")]
    SkinnedInstance {
        /// Source node identity from the instance.
        source_node_index: usize,
    },
    /// A primitive carries skin vertex attributes.
    #[error(
        "cannot bake static transforms: mesh {mesh} primitive {primitive} carries skin attributes"
    )]
    SkinnedPrimitive {
        /// Input mesh ordinal.
        mesh: usize,
        /// Primitive ordinal within the mesh.
        primitive: usize,
    },
    /// A skeleton parent is absent or is not earlier than its child.
    #[error("cannot bake static transforms: skeleton node {node} has invalid parent {parent}")]
    InvalidParent {
        /// Skeleton node ordinal.
        node: usize,
        /// Invalid parent ordinal.
        parent: usize,
    },
    /// A local or accumulated matrix has a non-finite component.
    #[error("cannot bake static transforms: skeleton node {node} has a non-finite transform")]
    NonFiniteTransform {
        /// Skeleton node ordinal.
        node: usize,
    },
    /// A mesh primitive has malformed attributes, indices, or material data.
    #[error(
        "cannot bake static transforms: mesh {mesh} primitive {primitive} is invalid: {reason}"
    )]
    InvalidPrimitive {
        /// Input mesh ordinal.
        mesh: usize,
        /// Primitive ordinal within the mesh.
        primitive: usize,
        /// Stable machine-readable reason.
        reason: &'static str,
    },
    /// A material factor contains a non-finite value.
    #[error("cannot bake static transforms: material {material} has non-finite {factor}")]
    NonFiniteMaterial {
        /// Material ordinal.
        material: usize,
        /// Material factor name.
        factor: &'static str,
    },
    /// A position, normal, or UV contains a non-finite component.
    #[error(
        "cannot bake static transforms: mesh {mesh} primitive {primitive} has non-finite {attribute} at vertex {vertex}"
    )]
    NonFiniteAttribute {
        /// Input mesh ordinal.
        mesh: usize,
        /// Primitive ordinal within the mesh.
        primitive: usize,
        /// Attribute name.
        attribute: &'static str,
        /// Vertex ordinal.
        vertex: usize,
    },
    /// A normal cannot be normalized after applying the normal matrix.
    #[error(
        "cannot bake static transforms: mesh {mesh} primitive {primitive} has zero normal at vertex {vertex}"
    )]
    ZeroNormal {
        /// Input mesh ordinal.
        mesh: usize,
        /// Primitive ordinal within the mesh.
        primitive: usize,
        /// Vertex ordinal.
        vertex: usize,
    },
    /// A linear transform is singular or too close to singular for a stable inverse transpose.
    #[error(
        "cannot bake static transforms: source node {source_node_index} has singular or near-singular transform (determinant {determinant})"
    )]
    SingularTransform {
        /// Source node identity from the instance.
        source_node_index: usize,
        /// Linear determinant.
        determinant: f32,
    },
    /// A linear transform reverses winding, which would require changing preserved indices.
    #[error(
        "cannot bake static transforms: source node {source_node_index} has reflection transform (determinant {determinant})"
    )]
    ReflectionTransform {
        /// Source node identity from the instance.
        source_node_index: usize,
        /// Linear determinant.
        determinant: f32,
    },
}

#[derive(Debug, Clone, Copy)]
struct BakePlan {
    instance: usize,
    mesh: usize,
    node: usize,
    world: Mat4,
    normal: Mat3,
    determinant: f32,
}

/// Bake accumulated rest/world transforms into static mesh positions and normals.
///
/// The returned document is canonical: it contains one identity root, one
/// identity child per input mesh instance, one private mesh definition per
/// child, no clips, no skins, and one scene. Positions are transformed by the
/// accumulated rest/world matrix; normals use its inverse transpose and are
/// normalized. Indices, UVs, material assignment, material values, and
/// embedded texture bytes are copied unchanged.
///
/// # Errors
///
/// Returns [`StaticMeshBakeError`] when there are no mesh instances, any
/// animation track or skin signal is present, or a mesh definition is not
/// attached to exactly one distinct source node. Missing references, invalid
/// hierarchy or primitive layouts, non-finite transforms, attributes, or
/// material factors, zero or unstable normals, near-singular transforms, and
/// reflections are also rejected. Reflections would require rewriting the
/// preserved triangle winding. The full document, including the transformed
/// positions and normals, is validated before output is constructed.
pub fn bake_static_mesh_transforms(doc: &Document) -> Result<StaticMeshBake, StaticMeshBakeError> {
    let plans = validate(doc)?;
    let mut meshes = Vec::with_capacity(plans.len());
    let mut instances = Vec::with_capacity(plans.len());
    let mut nodes = Vec::with_capacity(plans.len() + 1);
    let mut evidence = Vec::with_capacity(plans.len());

    nodes.push(Bone {
        name: "animsmith-static-root".to_string(),
        parent: None,
        rest: Transform::IDENTITY,
        inverse_bind: None,
    });

    for (ordinal, plan) in plans.iter().enumerate() {
        let input_mesh = &doc.assets.meshes[plan.mesh];
        let input_instance = &doc.assets.instances[plan.instance];
        let output_node = ordinal + 1;
        let output_mesh = ordinal;
        let baked_mesh = bake_mesh(input_mesh, plan, plan.mesh)?;
        let position_count = baked_mesh
            .primitives
            .iter()
            .map(|primitive| primitive.positions.len())
            .sum();
        let normal_count = baked_mesh
            .primitives
            .iter()
            .map(|primitive| primitive.normals.len())
            .sum();

        nodes.push(Bone {
            name: format!("animsmith-static-mesh-{ordinal}"),
            parent: Some(0),
            rest: Transform::IDENTITY,
            inverse_bind: None,
        });
        meshes.push(baked_mesh);
        instances.push(MeshInstance {
            source_node_index: input_instance.source_node_index,
            node: output_node,
            mesh: output_mesh,
            skin_joints: Vec::new(),
            skin_ibms: Vec::new(),
        });
        evidence.push(StaticMeshBakeInstanceEvidence {
            source_node_index: input_instance.source_node_index,
            source_node_name: doc.skeleton.bones[plan.node].name.clone(),
            source_mesh_ordinal: plan.mesh,
            source_mesh_index: input_mesh.source_mesh_index,
            source_mesh_name: input_mesh.name.clone(),
            output_node_index: output_node,
            output_mesh_index: output_mesh,
            world_transform: plan.world.to_cols_array(),
            linear_determinant: plan.determinant,
            primitive_count: input_mesh.primitives.len(),
            position_count,
            normal_count,
        });
    }

    Ok(StaticMeshBake {
        document: Document {
            skeleton: Skeleton { bones: nodes },
            clips: Vec::new(),
            assets: SceneAssets {
                meshes,
                instances,
                materials: doc.assets.materials.clone(),
                scenes: vec![SceneAsset {
                    source_scene_index: 0,
                    name: None,
                    roots: vec![0],
                }],
                default_scene: Some(0),
            },
            source: doc.source.clone(),
        },
        evidence: StaticMeshBakeEvidence { entries: evidence },
    })
}

fn validate(doc: &Document) -> Result<Vec<BakePlan>, StaticMeshBakeError> {
    if doc.assets.instances.is_empty() {
        return Err(StaticMeshBakeError::NoInstances);
    }
    for clip in &doc.clips {
        if let Some(track) = clip.tracks.first() {
            return Err(StaticMeshBakeError::AnimationTrack {
                clip: clip.name.clone(),
                node: track.bone,
                property: track.property.as_str(),
            });
        }
    }
    for (node, bone) in doc.skeleton.bones.iter().enumerate() {
        if bone.inverse_bind.is_some() {
            return Err(StaticMeshBakeError::SkeletonSkin { node });
        }
    }

    let worlds = world_matrices(&doc.skeleton)?;
    validate_materials(&doc.assets)?;
    let mut mesh_counts = vec![0usize; doc.assets.meshes.len()];
    let mut source_nodes = HashSet::with_capacity(doc.assets.instances.len());
    let mut skeleton_nodes = vec![false; doc.skeleton.bones.len()];
    for instance in &doc.assets.instances {
        let Some(count) = mesh_counts.get_mut(instance.mesh) else {
            return Err(StaticMeshBakeError::MissingMesh {
                source_node_index: instance.source_node_index,
                mesh: instance.mesh,
            });
        };
        *count += 1;
        if instance.node >= doc.skeleton.bones.len() {
            return Err(StaticMeshBakeError::MissingNode {
                source_node_index: instance.source_node_index,
                node: instance.node,
            });
        }
        if !source_nodes.insert(instance.source_node_index) {
            return Err(StaticMeshBakeError::DuplicateNodeAttachment {
                source_node_index: instance.source_node_index,
            });
        }
        if skeleton_nodes[instance.node] {
            return Err(StaticMeshBakeError::DuplicateSkeletonNodeAttachment {
                node: instance.node,
            });
        }
        skeleton_nodes[instance.node] = true;
        if !instance.skin_joints.is_empty() || !instance.skin_ibms.is_empty() {
            return Err(StaticMeshBakeError::SkinnedInstance {
                source_node_index: instance.source_node_index,
            });
        }
    }
    for (mesh, &instances) in mesh_counts.iter().enumerate() {
        if instances != 1 {
            return Err(StaticMeshBakeError::MeshInstanceCount {
                mesh,
                source_mesh_index: doc.assets.meshes[mesh].source_mesh_index,
                instances,
            });
        }
    }

    let mut plans = Vec::with_capacity(doc.assets.instances.len());
    for (instance_ordinal, instance) in doc.assets.instances.iter().enumerate() {
        let mesh = &doc.assets.meshes[instance.mesh];
        validate_mesh(mesh, instance.mesh, doc.assets.materials.len())?;
        let world = worlds[instance.node];
        let linear = Mat3::from_mat4(world);
        let determinant = linear.determinant();
        let scale = linear
            .to_cols_array()
            .into_iter()
            .fold(0.0_f32, |largest, component| largest.max(component.abs()));
        let threshold = MIN_RELATIVE_DETERMINANT * scale * scale * scale;
        if !determinant.is_finite() || scale == 0.0 || determinant.abs() <= threshold {
            return Err(StaticMeshBakeError::SingularTransform {
                source_node_index: instance.source_node_index,
                determinant,
            });
        }
        if determinant < 0.0 {
            return Err(StaticMeshBakeError::ReflectionTransform {
                source_node_index: instance.source_node_index,
                determinant,
            });
        }
        let normal = linear.inverse().transpose();
        if !matrix3_is_finite(normal) {
            return Err(StaticMeshBakeError::SingularTransform {
                source_node_index: instance.source_node_index,
                determinant,
            });
        }
        validate_baked_mesh(mesh, instance.mesh, world, normal)?;
        plans.push(BakePlan {
            instance: instance_ordinal,
            mesh: instance.mesh,
            node: instance.node,
            world,
            normal,
            determinant,
        });
    }
    Ok(plans)
}

fn validate_materials(assets: &SceneAssets) -> Result<(), StaticMeshBakeError> {
    for (material, value) in assets.materials.iter().enumerate() {
        if !value
            .base_color
            .iter()
            .all(|component| component.is_finite())
        {
            return Err(StaticMeshBakeError::NonFiniteMaterial {
                material,
                factor: "base_color",
            });
        }
        if !value.metallic.is_finite() {
            return Err(StaticMeshBakeError::NonFiniteMaterial {
                material,
                factor: "metallic",
            });
        }
        if !value.roughness.is_finite() {
            return Err(StaticMeshBakeError::NonFiniteMaterial {
                material,
                factor: "roughness",
            });
        }
    }
    Ok(())
}

fn world_matrices(skeleton: &Skeleton) -> Result<Vec<Mat4>, StaticMeshBakeError> {
    let mut worlds = Vec::with_capacity(skeleton.bones.len());
    for (node, bone) in skeleton.bones.iter().enumerate() {
        let local = bone.rest.to_mat4();
        if !matrix4_is_finite(local) {
            return Err(StaticMeshBakeError::NonFiniteTransform { node });
        }
        let world = match bone.parent {
            Some(parent) if parent < node => worlds[parent] * local,
            Some(parent) => return Err(StaticMeshBakeError::InvalidParent { node, parent }),
            None => local,
        };
        if !matrix4_is_finite(world) {
            return Err(StaticMeshBakeError::NonFiniteTransform { node });
        }
        worlds.push(world);
    }
    Ok(worlds)
}

fn validate_mesh(
    mesh: &MeshAsset,
    mesh_ordinal: usize,
    materials: usize,
) -> Result<(), StaticMeshBakeError> {
    if mesh.primitives.is_empty() {
        return Err(StaticMeshBakeError::InvalidPrimitive {
            mesh: mesh_ordinal,
            primitive: 0,
            reason: "empty_primitives",
        });
    }
    for (primitive_ordinal, primitive) in mesh.primitives.iter().enumerate() {
        validate_primitive(primitive, mesh_ordinal, primitive_ordinal, materials)?;
    }
    Ok(())
}

fn validate_primitive(
    primitive: &Primitive,
    mesh: usize,
    primitive_ordinal: usize,
    materials: usize,
) -> Result<(), StaticMeshBakeError> {
    let invalid = |reason| StaticMeshBakeError::InvalidPrimitive {
        mesh,
        primitive: primitive_ordinal,
        reason,
    };
    if primitive
        .material
        .is_some_and(|material| material >= materials)
    {
        return Err(invalid("material_index"));
    }
    if primitive.positions.is_empty() {
        return Err(invalid("empty_positions"));
    }
    if !primitive.normals.is_empty() && primitive.normals.len() != primitive.positions.len() {
        return Err(invalid("normal_count"));
    }
    if !primitive.uvs.is_empty() && primitive.uvs.len() != primitive.positions.len() {
        return Err(invalid("uv_count"));
    }
    if !primitive.joints.is_empty() || !primitive.weights.is_empty() {
        return Err(StaticMeshBakeError::SkinnedPrimitive {
            mesh,
            primitive: primitive_ordinal,
        });
    }
    if primitive.indices.is_empty() {
        if !primitive.positions.len().is_multiple_of(3) {
            return Err(invalid("unindexed_triangle_count"));
        }
    } else {
        if !primitive.indices.len().is_multiple_of(3) {
            return Err(invalid("indexed_triangle_count"));
        }
        if primitive.indices.iter().any(|&index| {
            usize::try_from(index).map_or(true, |index| index >= primitive.positions.len())
        }) {
            return Err(invalid("triangle_index"));
        }
    }
    for (vertex, position) in primitive.positions.iter().enumerate() {
        if !position.is_finite() {
            return Err(StaticMeshBakeError::NonFiniteAttribute {
                mesh,
                primitive: primitive_ordinal,
                attribute: "position",
                vertex,
            });
        }
    }
    for (vertex, normal) in primitive.normals.iter().enumerate() {
        if !normal.is_finite() {
            return Err(StaticMeshBakeError::NonFiniteAttribute {
                mesh,
                primitive: primitive_ordinal,
                attribute: "normal",
                vertex,
            });
        }
        if normal.length_squared() == 0.0 {
            return Err(StaticMeshBakeError::ZeroNormal {
                mesh,
                primitive: primitive_ordinal,
                vertex,
            });
        }
    }
    for (vertex, uv) in primitive.uvs.iter().enumerate() {
        if !uv.iter().all(|component| component.is_finite()) {
            return Err(StaticMeshBakeError::NonFiniteAttribute {
                mesh,
                primitive: primitive_ordinal,
                attribute: "uv",
                vertex,
            });
        }
    }
    Ok(())
}

fn validate_baked_mesh(
    mesh: &MeshAsset,
    mesh_ordinal: usize,
    world: Mat4,
    normal_matrix: Mat3,
) -> Result<(), StaticMeshBakeError> {
    for (primitive_ordinal, primitive) in mesh.primitives.iter().enumerate() {
        for (vertex, position) in primitive.positions.iter().enumerate() {
            if !world.transform_point3(*position).is_finite() {
                return Err(StaticMeshBakeError::NonFiniteAttribute {
                    mesh: mesh_ordinal,
                    primitive: primitive_ordinal,
                    attribute: "baked_position",
                    vertex,
                });
            }
        }
        for (vertex, normal) in primitive.normals.iter().enumerate() {
            let transformed = normal_matrix * *normal;
            if !transformed.is_finite() {
                return Err(StaticMeshBakeError::NonFiniteAttribute {
                    mesh: mesh_ordinal,
                    primitive: primitive_ordinal,
                    attribute: "baked_normal",
                    vertex,
                });
            }
            if transformed.try_normalize().is_none() {
                return Err(StaticMeshBakeError::ZeroNormal {
                    mesh: mesh_ordinal,
                    primitive: primitive_ordinal,
                    vertex,
                });
            }
        }
    }
    Ok(())
}

fn bake_mesh(
    mesh: &MeshAsset,
    plan: &BakePlan,
    mesh_ordinal: usize,
) -> Result<MeshAsset, StaticMeshBakeError> {
    let mut baked = mesh.clone();
    for (primitive_ordinal, primitive) in baked.primitives.iter_mut().enumerate() {
        for position in &mut primitive.positions {
            *position = plan.world.transform_point3(*position);
        }
        for (vertex, normal) in primitive.normals.iter_mut().enumerate() {
            // Leave canonical unit normals under an exact identity matrix
            // untouched so baking an already baked artifact stays byte-stable.
            if plan.world == Mat4::IDENTITY && normal.is_normalized() {
                continue;
            }
            let transformed = plan.normal * *normal;
            let Some(normalized) = transformed.try_normalize() else {
                return Err(StaticMeshBakeError::ZeroNormal {
                    mesh: mesh_ordinal,
                    primitive: primitive_ordinal,
                    vertex,
                });
            };
            *normal = normalized;
        }
    }
    Ok(baked)
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
