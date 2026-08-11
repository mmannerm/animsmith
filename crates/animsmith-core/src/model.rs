//! The loader-facing layer: clips, tracks, and the skeleton before metric
//! resampling or repair. The glTF loader preserves authored animation
//! values; the FBX loader normalizes scene coordinates and bakes takes to
//! linear TRS tracks. Mechanical checks (NaN, quaternion flips, key
//! density, …) read this layer; semantic checks read the sampled layer
//! built from it (see [`crate::sample`]).

use glam::{Mat3, Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Stable machine-readable reason a positive-uniform affine linear part failed
/// classification.
///
/// Core consumers map these typed facts into their own domain-specific
/// diagnostics rather than sharing a broad operation error enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AffineDomainViolation {
    /// The linear part's axis lengths are not equal (non-uniform scale).
    NonUniformScale,
    /// The linear part's axes are not mutually orthogonal (shear).
    Sheared,
    /// The linear part has a negative determinant (reflection).
    Reflected,
    /// The linear part is singular or near-singular.
    Singular,
    /// The linear part contains a non-finite component.
    NonFinite,
}

/// Tolerances supplied by one caller of
/// [`classify_positive_uniform_affine`].
///
/// The classifier deliberately owns no global policy: Appendix D scale
/// planning uses its versioned `f64` policy, while skinned bind-pose
/// canonicalization retains its established `1e-4` acceptance band.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PositiveUniformAffineTolerance {
    pub(crate) equal_axis: f64,
    pub(crate) relative_orthogonality: f64,
    pub(crate) singular_determinant_relative: f64,
}

/// Classify an affine linear part as an orientation-preserving positive
/// uniform scale and return its common factor.
///
/// Inputs widen to `f64` before every derived calculation. This preserves the
/// Appendix D scale classifier's boundary behaviour; callers choose the
/// tolerance policy appropriate to their separate contract.
pub(crate) fn classify_positive_uniform_affine(
    linear: Mat3,
    tolerance: PositiveUniformAffineTolerance,
) -> Result<f64, AffineDomainViolation> {
    if !linear.is_finite() {
        return Err(AffineDomainViolation::NonFinite);
    }
    let columns = [
        linear.x_axis.as_dvec3(),
        linear.y_axis.as_dvec3(),
        linear.z_axis.as_dvec3(),
    ];
    let lengths = affine_axis_lengths(linear);
    if lengths.iter().any(|length| !length.is_finite()) {
        return Err(AffineDomainViolation::NonFinite);
    }
    let average = average_affine_axis_length(lengths);
    if average <= 0.0 {
        return Err(AffineDomainViolation::Singular);
    }

    // Check singularity before the shape facts. A degenerate basis that is
    // also non-uniform or sheared is still singular, which keeps the
    // independently named rejection classes deterministic.
    // Expand the scalar triple product from the widened columns. Calling
    // `Mat3::determinant` here would perform the derived arithmetic in f32
    // before widening and moves the singular boundary.
    let determinant = columns[2].dot(columns[0].cross(columns[1]));
    if !determinant.is_finite() {
        return Err(AffineDomainViolation::NonFinite);
    }
    let axis_product = lengths[0] * lengths[1] * lengths[2];
    if determinant.abs() <= tolerance.singular_determinant_relative * axis_product {
        return Err(AffineDomainViolation::Singular);
    }
    // This is a relative band with no unit floor: a floor would become an
    // absolute tolerance for sub-unit transforms. The longer-operand base
    // keeps the comparison symmetric, and `>` makes the boundary inclusive.
    if lengths
        .iter()
        .any(|&length| (length - average).abs() > tolerance.equal_axis * average.max(length))
    {
        return Err(AffineDomainViolation::NonUniformScale);
    }

    // Scale the dot-product band by the square of the common factor. As with
    // the axis band, equality is accepted and only a value beyond it rejects.
    let orthogonality_tolerance = tolerance.relative_orthogonality * average * average;
    if columns[0].dot(columns[1]).abs() > orthogonality_tolerance
        || columns[0].dot(columns[2]).abs() > orthogonality_tolerance
        || columns[1].dot(columns[2]).abs() > orthogonality_tolerance
    {
        return Err(AffineDomainViolation::Sheared);
    }
    if determinant < 0.0 {
        return Err(AffineDomainViolation::Reflected);
    }
    Ok(average)
}

/// The three column lengths of a linear part, widened to `f64` first.
///
/// The positive-uniform classifier and the scale proof's observed-factor
/// witness both use this helper so that their factor is one shared quantity.
pub(crate) fn affine_axis_lengths(linear: Mat3) -> [f64; 3] {
    [
        linear.x_axis.as_dvec3().length(),
        linear.y_axis.as_dvec3().length(),
        linear.z_axis.as_dvec3().length(),
    ]
}

/// The arithmetic-mean common factor represented by three affine axis
/// lengths.
///
/// The finite widened inputs are summed in ascending order so this shared
/// factor does not depend on an affine matrix's authored column order.
pub(crate) fn average_affine_axis_length(lengths: [f64; 3]) -> f64 {
    let mut ascending = lengths;
    ascending.sort_by(f64::total_cmp);
    (ascending[0] + ascending[1] + ascending[2]) / 3.0
}

#[cfg(test)]
pub(crate) mod affine_test_fixtures {
    use super::{Mat3, Vec3};

    /// A finite diagonal basis intentionally between the two callers' equal
    /// axis bands: Appendix D rejects it, while skinned canonicalization's
    /// established `1e-4` policy accepts it.
    pub(crate) fn tolerance_divergence_basis() -> Mat3 {
        Mat3::from_diagonal(Vec3::new(1.0, 1.000_05, 1.0))
    }

    /// A nearly orthogonal basis whose only non-zero cross-axis dot product
    /// lies between the two callers' orthogonality bands.
    pub(crate) fn orthogonality_tolerance_divergence_basis() -> Mat3 {
        Mat3::from_cols(Vec3::X, Vec3::new(5.0e-5, 1.0, 0.0), Vec3::Z)
    }

    /// All signed column orders of the exact Appendix D v6 mean fixture.
    ///
    /// Odd permutations negate their first column, preserving orientation
    /// without changing any axis length.
    pub(crate) fn appendix_d_v6_mean_permutations() -> [Mat3; 6] {
        let columns = [
            Vec3::new(
                f32::from_bits(0x3f0e_8cbb),
                f32::from_bits(0x3f26_fbbe),
                f32::from_bits(0x3f21_9bc7),
            ),
            Vec3::new(
                f32::from_bits(0x3d9c_b415),
                f32::from_bits(0x3e92_d82b),
                f32::from_bits(0x3f82_e85d),
            ),
            Vec3::new(
                f32::from_bits(0x3f14_5226),
                f32::from_bits(0x3e9e_e50d),
                f32::from_bits(0x3f56_817c),
            ),
        ];
        [
            Mat3::from_cols(columns[0], columns[1], columns[2]),
            Mat3::from_cols(-columns[0], columns[2], columns[1]),
            Mat3::from_cols(-columns[1], columns[0], columns[2]),
            Mat3::from_cols(columns[1], columns[2], columns[0]),
            Mat3::from_cols(columns[2], columns[0], columns[1]),
            Mat3::from_cols(-columns[2], columns[1], columns[0]),
        ]
    }
}

/// Index into [`Skeleton::bones`].
pub type BoneId = usize;

/// Node-local TRS transform.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    /// Translation in scene units.
    pub translation: Vec3,
    /// Orientation relative to the parent node.
    pub rotation: Quat,
    /// Non-uniform local scale.
    pub scale: Vec3,
}

impl Transform {
    /// The identity transform: zero translation, identity rotation, and
    /// unit scale.
    pub const IDENTITY: Self = Self {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    };

    /// Convert this TRS transform to a matrix using glam's
    /// scale-rotation-translation order.
    pub fn to_mat4(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// One skeleton node/bone in parent-before-child order.
#[derive(Debug, Clone)]
pub struct Bone {
    /// Bone/node name as authored or normalized by the loader.
    pub name: String,
    /// Parent bone index; `None` means this is a root bone.
    pub parent: Option<BoneId>,
    /// Rest pose, node-local. Whether this or the inverse-bind-derived
    /// rest is authoritative is a `bind-pose` check concern.
    pub rest: Transform,
    /// Inverse bind matrix from a skin, when one references this bone.
    pub inverse_bind: Option<Mat4>,
}

/// Bones in topological order: a bone's parent always precedes it.
/// Loaders are responsible for establishing this invariant.
#[derive(Debug, Clone, Default)]
pub struct Skeleton {
    /// Bones in topological order.
    pub bones: Vec<Bone>,
}

impl Skeleton {
    /// Name of the bone at `id`.
    ///
    /// # Panics
    ///
    /// Panics if `id` is not a valid index into [`Skeleton::bones`].
    pub fn bone_name(&self, id: BoneId) -> &str {
        &self.bones[id].name
    }
}

/// Structural failure composing [`Skeleton`] rest-local transforms into
/// world matrices, returned by [`world_rest_matrices`].
///
/// This is deliberately generic over the eventual caller-facing error: every
/// caller of [`world_rest_matrices`] maps one of these two structural facts
/// into its own typed error variant rather than sharing an error enum across
/// module boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorldMatrixError {
    /// The node's local or accumulated world transform has a non-finite
    /// component.
    NonFiniteTransform {
        /// The node with the non-finite transform.
        node: BoneId,
    },
    /// The node's parent is not earlier in [`Skeleton::bones`], so the
    /// skeleton is not in the required parent-before-child order.
    InvalidParent {
        /// The node with an invalid parent.
        node: BoneId,
        /// The invalid parent index.
        parent: BoneId,
    },
}

/// Compose every [`Bone::rest`] local transform in `skeleton` into a
/// parent-before-child world matrix, shared by every module that needs plain
/// rest-world FK (skin bind-pose canonicalization, static mesh baking, and
/// scale planning/proof).
///
/// `skeleton.bones` order is trusted as parent-before-child, matching
/// [`Skeleton`]'s documented invariant; a parent index that is not strictly
/// less than its child's is reported as [`WorldMatrixError::InvalidParent`]
/// rather than assumed.
pub(crate) fn world_rest_matrices(skeleton: &Skeleton) -> Result<Vec<Mat4>, WorldMatrixError> {
    let mut worlds = Vec::with_capacity(skeleton.bones.len());
    for (node, bone) in skeleton.bones.iter().enumerate() {
        let local = bone.rest.to_mat4();
        if !mat4_is_finite(local) {
            return Err(WorldMatrixError::NonFiniteTransform { node });
        }
        let world = match bone.parent {
            Some(parent) if parent < node => worlds[parent] * local,
            Some(parent) => return Err(WorldMatrixError::InvalidParent { node, parent }),
            None => local,
        };
        if !mat4_is_finite(world) {
            return Err(WorldMatrixError::NonFiniteTransform { node });
        }
        worlds.push(world);
    }
    Ok(worlds)
}

/// Compose rest-world matrices for a partial-evidence consumer.
///
/// Unlike [`world_rest_matrices`], this deliberately preserves a slot for
/// every bone and makes only the malformed chain unavailable. Measurement
/// uses that behaviour to retain finite evidence from unrelated roots.
pub(crate) fn tolerant_world_rest_matrices(skeleton: &Skeleton) -> Vec<Option<Mat4>> {
    let mut worlds = Vec::with_capacity(skeleton.bones.len());
    for bone in &skeleton.bones {
        let local = bone.rest.to_mat4();
        let world = match bone.parent {
            Some(parent) => worlds
                .get(parent)
                .copied()
                .flatten()
                .map(|parent_world| parent_world * local),
            None => Some(local),
        }
        .filter(|matrix| mat4_is_finite(*matrix));
        worlds.push(world);
    }
    worlds
}

pub(crate) fn mat4_is_finite(matrix: Mat4) -> bool {
    matrix.to_cols_array().into_iter().all(f32::is_finite)
}

/// Animated property targeted by a [`Track`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Property {
    /// Local translation channel.
    Translation,
    /// Local rotation channel.
    Rotation,
    /// Local scale channel.
    Scale,
}

impl Property {
    /// Stable snake-case name used in diagnostics and serialized
    /// metadata.
    pub fn as_str(self) -> &'static str {
        match self {
            Property::Translation => "translation",
            Property::Rotation => "rotation",
            Property::Scale => "scale",
        }
    }
}

/// Interpolation mode for a [`Track`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interpolation {
    /// Linear interpolation between key values.
    Linear,
    /// Hold the previous key until the next key.
    Step,
    /// glTF cubic spline: `values` holds `[in-tangent, value, out-tangent]`
    /// triplets per keyframe. Use [`Track::value_index`] to address the
    /// value elements.
    CubicSpline,
}

/// Storage for a track's key values.
#[derive(Debug, Clone)]
pub enum TrackValues {
    /// Translation or scale values.
    Vec3s(Vec<Vec3>),
    /// Rotation values.
    Quats(Vec<Quat>),
}

impl TrackValues {
    /// Number of stored values, including tangents for cubic-spline
    /// tracks.
    pub fn len(&self) -> usize {
        match self {
            TrackValues::Vec3s(v) => v.len(),
            TrackValues::Quats(v) => v.len(),
        }
    }

    /// Whether there are no stored values.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// One animated property of one bone.
#[derive(Debug, Clone)]
pub struct Track {
    /// Bone index targeted by this track.
    pub bone: BoneId,
    /// Property animated on the target bone.
    pub property: Property,
    /// Interpolation mode used between keys.
    pub interpolation: Interpolation,
    /// Keyframe times in seconds. Same length as the keyframe count
    /// (tangent elements in cubic tracks do not add times).
    pub times: Vec<f32>,
    /// Key values, with cubic-spline tracks storing tangent triplets.
    pub values: TrackValues,
}

impl Track {
    /// Number of keyframes.
    pub fn key_count(&self) -> usize {
        self.times.len()
    }

    /// Index into `values` of keyframe `k`'s value element (skips
    /// tangents for cubic tracks).
    pub fn value_index(&self, k: usize) -> usize {
        match self.interpolation {
            Interpolation::CubicSpline => 3 * k + 1,
            _ => k,
        }
    }

    /// Keyframe `k`'s value, for Vec3 tracks.
    pub fn key_vec3(&self, k: usize) -> Option<Vec3> {
        match &self.values {
            TrackValues::Vec3s(v) => v.get(self.value_index(k)).copied(),
            TrackValues::Quats(_) => None,
        }
    }

    /// Keyframe `k`'s value, for rotation tracks.
    pub fn key_quat(&self, k: usize) -> Option<Quat> {
        match &self.values {
            TrackValues::Quats(v) => v.get(self.value_index(k)).copied(),
            TrackValues::Vec3s(_) => None,
        }
    }

    /// First key time, or `0.0` for an empty track.
    pub fn start_time(&self) -> f32 {
        self.times.first().copied().unwrap_or(0.0)
    }

    /// Last key time, or `0.0` for an empty track.
    pub fn end_time(&self) -> f32 {
        self.times.last().copied().unwrap_or(0.0)
    }
}

/// One animation clip targeting the document skeleton.
#[derive(Debug, Clone)]
pub struct Clip {
    /// Clip name, used as the key in measurement maps and config
    /// expectations.
    pub name: String,
    /// Clip length in seconds (max sampler end time across tracks).
    pub duration_s: f64,
    /// Animated tracks belonging to this clip.
    pub tracks: Vec<Track>,
}

/// Loader-provided provenance for a [`Document`].
#[derive(Debug, Clone, Default)]
pub struct SourceInfo {
    /// Source path, when the loader was given one.
    pub path: Option<String>,
    /// Source format label such as `"glb"` or `"fbx"`.
    pub format: Option<String>,
}

/// A loaded file: one skeleton, any number of clips targeting it, and
/// the scene assets (meshes, materials, and textures) that rode in alongside
/// them.
/// `assets` is default-empty: the check catalog judges animation and
/// ignores it, but the load/write round-trip carries it so `transform`
/// and `convert` preserve geometry instead of silently dropping it.
#[derive(Debug, Clone, Default)]
pub struct Document {
    /// Skeleton shared by every clip.
    pub skeleton: Skeleton,
    /// Animation clips targeting [`Document::skeleton`].
    pub clips: Vec<Clip>,
    /// Meshes, materials, and textures carried by the loaded scene.
    pub assets: SceneAssets,
    /// Optional source provenance.
    pub source: SourceInfo,
}

/// A structural invariant violated by [`validate_document_shape`].
///
/// Validation is a snapshot, not a durable guarantee: [`Document`] and its
/// nested fields are publicly mutable. Strict operations that rely on this
/// full shape must validate again at each public boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum DocumentShapeError {
    /// A bone's local rest transform or its composed rest-world transform is
    /// non-finite.
    #[error("node {node} has a non-finite rest transform")]
    NonFiniteSkeletonRest {
        /// The affected skeleton node.
        node: BoneId,
    },
    /// A bone's parent does not precede it in parent-before-child order.
    #[error("node {node} has invalid parent {parent}")]
    InvalidSkeletonParent {
        /// The affected skeleton node.
        node: BoneId,
        /// The invalid parent index.
        parent: BoneId,
    },
    /// The source-node projection declares one source node identity twice.
    #[error("source skeleton declares duplicate source node index {source_node_index}")]
    DuplicateSourceNodeIndex {
        /// The duplicated source-node index.
        source_node_index: usize,
    },
    /// The source-skin projection declares one source skin identity twice.
    #[error("source skeleton declares duplicate source skin index {source_skin_index}")]
    DuplicateSourceSkinIndex {
        /// The duplicated source-skin index.
        source_skin_index: usize,
    },
    /// A complete source-node projection contradicts the normalized skeleton.
    #[error(
        "source node {source_node_index} contradicts the document skeleton's parent chain ({violation})"
    )]
    SourceProjection {
        /// The source node whose projection failed.
        source_node_index: usize,
        /// The typed projection failure.
        violation: SourceProjectionViolation,
    },
    /// A clip declares the same target `(node, property)` more than once.
    #[error("clip {clip_index} declares duplicate {property:?} tracks for node {node}")]
    DuplicateClipTrack {
        /// Index into [`Document::clips`].
        clip_index: usize,
        /// The duplicated target node.
        node: BoneId,
        /// The duplicated animated property.
        property: Property,
    },
    /// A track is malformed for its target, interpolation, or value storage.
    #[error("clip {clip_index} track for node {node} has an invalid shape ({violation})")]
    TrackShape {
        /// Index into [`Document::clips`].
        clip_index: usize,
        /// The track's target node.
        node: BoneId,
        /// The typed track-shape failure.
        violation: TrackShapeViolation,
    },
    /// A mesh instance has an invalid reference or inverse-bind payload.
    #[error("mesh instance {instance_index} is invalid ({violation})")]
    MeshInstanceShape {
        /// Index into [`SceneAssets::instances`].
        instance_index: usize,
        /// The typed mesh-instance failure.
        violation: MeshInstanceShapeViolation,
    },
    /// A bone-level inverse-bind matrix is non-finite.
    #[error("node {node} has a non-finite inverse-bind matrix")]
    NonFiniteBoneInverseBind {
        /// The affected skeleton node.
        node: BoneId,
    },
}

/// The way a complete source-node projection contradicts the skeleton.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum SourceProjectionViolation {
    /// A projected bone index is outside [`Skeleton::bones`].
    #[error("projected_bone_out_of_range")]
    ProjectedBoneOutOfRange,
    /// Two source-node rows project to the same normalized bone.
    #[error("two_source_nodes_project_to_one_bone")]
    TwoSourceNodesProjectToOneBone,
    /// An ancestor walk names a source node absent from the projection table.
    #[error("parent_source_node_is_missing")]
    ParentSourceNodeMissing,
    /// An ancestor walk through unprojected rows does not terminate.
    #[error("cyclic_unprojected_source_parent_chain")]
    CyclicUnprojectedSourceParentChain,
    /// The nearest projected source ancestor differs from the bone parent.
    #[error("projection_and_skeleton_parents_differ")]
    NearestProjectedParentMismatch,
    /// A projected bone has an unprojected direct skeleton child.
    #[error("projected_bone_has_an_unprojected_skeleton_child")]
    ProjectedBoneHasUnprojectedSkeletonChild,
}

/// The way a clip track is malformed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum TrackShapeViolation {
    /// The target bone index is outside [`Skeleton::bones`].
    #[error("bone_index_out_of_range")]
    BoneIndexOutOfRange,
    /// The track has no keyframe times.
    #[error("empty_times")]
    EmptyTimes,
    /// At least one keyframe time is not finite.
    #[error("non_finite_time")]
    NonFiniteTime,
    /// Keyframe times are not strictly increasing.
    #[error("times_not_strictly_increasing")]
    TimesNotStrictlyIncreasing,
    /// Stored value count disagrees with the interpolation's key count.
    #[error("value_count_mismatch")]
    ValueCountMismatch,
    /// The value storage does not match the targeted property.
    #[error("value_type_mismatches_property")]
    ValueTypeMismatchesProperty,
    /// At least one stored value is not finite.
    #[error("non_finite_value")]
    NonFiniteValue,
}

/// The way a mesh instance is malformed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum MeshInstanceShapeViolation {
    /// The instance node index is outside [`Skeleton::bones`].
    #[error("node_index_out_of_range")]
    NodeIndexOutOfRange,
    /// The mesh index is outside [`SceneAssets::meshes`].
    #[error("mesh_index_out_of_range")]
    MeshIndexOutOfRange,
    /// A skin joint index is outside [`Skeleton::bones`].
    #[error("skin_joint_out_of_range")]
    SkinJointOutOfRange,
    /// A non-empty inverse-bind array has a different length from skin joints.
    #[error("skin_ibm_count_mismatch")]
    SkinInverseBindCountMismatch,
    /// An instance inverse-bind matrix is not finite.
    #[error("non_finite_inverse_bind")]
    NonFiniteSkinInverseBind,
}

// --- Scene assets (meshes/materials) -----------------------------------
//
// The geometry half of a [`Document`]. Populated by both format loaders and
// emitted by the writer, so a full conversion preserves geometry. Primitives
// may be indexed already; [`Primitive::weld`] can index unindexed exact
// duplicates without collapsing authored seams.

/// One triangle-list primitive sharing a material. Attribute arrays may be
/// indexed already; [`Primitive::weld`] dedupes an unindexed primitive into
/// indexed form.
///
/// Additional glTF skin-influence attribute sets are intentionally metadata
/// only. The primary four influences remain in [`Self::joints`] and
/// [`Self::weights`]; consumers can use this metadata to apply their own
/// policy without the core assuming how additional influences are evaluated.
#[derive(Debug, Clone, Default)]
pub struct Primitive {
    /// Index into [`SceneAssets::materials`].
    pub material: Option<usize>,
    /// Triangle indices into the attribute arrays; empty = unindexed.
    pub indices: Vec<u32>,
    /// Vertex positions in scene units.
    pub positions: Vec<Vec3>,
    /// Same length as `positions`, or empty.
    pub normals: Vec<Vec3>,
    /// Same length as `positions`, or empty.
    pub uvs: Vec<[f32; 2]>,
    /// Indices into an owning instance's skin-joint list; empty if unskinned.
    pub joints: Vec<[u16; 4]>,
    /// Skinning weights parallel to [`Primitive::joints`].
    pub weights: Vec<[f32; 4]>,
    /// Declared non-primary skin-influence attribute sets.
    ///
    /// Each entry records whether the glTF primitive had `JOINTS_n` and/or
    /// `WEIGHTS_n` for `n >= 1`. Entries are sorted by
    /// [`AdditionalInfluenceSet::set_index`].
    pub additional_influence_sets: Vec<AdditionalInfluenceSet>,
}

/// Presence metadata for one non-primary glTF skin-influence attribute set.
///
/// A set may contain only one side because source assets can declare
/// `JOINTS_n` and `WEIGHTS_n` independently. This type deliberately does not
/// retain the corresponding per-vertex values: the core model's skinning
/// semantics remain the primary `JOINTS_0` / `WEIGHTS_0` set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdditionalInfluenceSet {
    /// glTF attribute-set number (`n >= 1`).
    pub set_index: u32,
    /// Whether `JOINTS_n` was declared.
    pub joints_present: bool,
    /// Whether `WEIGHTS_n` was declared.
    pub weights_present: bool,
}

/// One source mesh definition, independent of any node that instances it.
#[derive(Debug, Clone, Default)]
pub struct MeshAsset {
    /// Mesh name.
    pub name: String,
    /// Stable index of this definition in the source format.
    ///
    /// glTF permits several nodes to instance one mesh definition. Loaders
    /// preserve that distinction through [`SceneAssets::instances`].
    pub source_mesh_index: usize,
    /// Triangle-list primitives belonging to this mesh.
    pub primitives: Vec<Primitive>,
}

/// One node instance of a source [`MeshAsset`] definition.
#[derive(Debug, Clone, Default)]
pub struct MeshInstance {
    /// Index of the source-format node that owns this mesh instance.
    pub source_node_index: usize,
    /// The node this mesh hangs off in the core skeleton.
    pub node: BoneId,
    /// Index into [`SceneAssets::meshes`] of the instanced definition.
    pub mesh: usize,
    /// Skin joints in cluster order. Empty = unskinned.
    pub skin_joints: Vec<BoneId>,
    /// Per-joint inverse bind matrices, parallel to `skin_joints`
    /// (glTF convention: joint-bind-world⁻¹ × geometry-to-world, all
    /// in the converted scene space). Falls back to the bones'
    /// `inverse_bind` when empty.
    pub skin_ibms: Vec<Mat4>,
}

/// One declared source scene and its root nodes.
#[derive(Debug, Clone, Default)]
pub struct SceneAsset {
    /// Index of this scene in the source-format scene array.
    pub source_scene_index: usize,
    /// Authored scene name, when the source format provides one.
    pub name: Option<String>,
    /// Root nodes belonging to this scene, represented as core bone ids.
    pub roots: Vec<BoneId>,
}

/// Whether a loader supplied source-node and source-skin identity evidence.
///
/// The skeleton used by sampling is deliberately format-neutral and is ordered
/// for parent-before-child FK. Source formats can use a different stable node
/// order, so this coverage flag keeps an empty source table from being
/// mistaken for a source file with no nodes or skins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceSkeletonCoverage {
    /// The loader cannot provide source-node and source-skin identity facts.
    #[default]
    Unavailable,
    /// The source-node and source-skin tables describe the loaded input.
    /// Source nodes with [`SourceNodeAsset::bone`] form the downward-closed
    /// projection consumed by strict operations.
    Complete,
}

/// The authored local-rest representation of one source node.
///
/// glTF permits either decomposed TRS properties or a matrix. Keeping this
/// representation separate from [`Bone::rest`] avoids presenting a lossy
/// matrix decomposition as though it were authored TRS evidence.
#[derive(Debug, Clone)]
pub enum SourceNodeLocalRest {
    /// Source node declared translation, rotation, and scale properties.
    Trs {
        /// Local translation in scene units.
        translation: Vec3,
        /// Local orientation relative to the parent node.
        rotation: Quat,
        /// Local non-uniform scale.
        scale: Vec3,
    },
    /// Source node declared a column-major 4×4 local transform matrix.
    Matrix(Mat4),
}

/// One source-format node with source-native identity facts.
///
/// Marked `#[non_exhaustive]` because this projection grows as loaders learn
/// to carry more source-native identity (`bone` was the most recent
/// addition): out-of-crate embedders construct it through
/// [`SourceNodeAsset::new`] and assign the optional facts they have, so a
/// later field cannot break their build. The sibling source-asset structs in
/// this module are not yet marked; they are stable in a way this one has
/// already demonstrated it is not.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct SourceNodeAsset {
    /// Stable node-array index in the source format.
    pub source_node_index: usize,
    /// Authored node name, when present.
    pub name: Option<String>,
    /// Source node-array index of the authored parent, when any.
    pub parent_source_node_index: Option<usize>,
    /// Declared source scenes that name this node as a root, in source-scene
    /// index order.
    pub scene_root_indices: Vec<usize>,
    /// Authored local-rest representation.
    pub local_rest: SourceNodeLocalRest,
    /// The core [`BoneId`] this source node normalized to, when the loader's
    /// topology derivation kept it reachable from a scene root.
    ///
    /// `None` means the loader dropped this source node during
    /// normalization (for example, it was unreachable from any root and so
    /// never became a [`Skeleton`] bone). Format-neutral consumers that need
    /// to resolve a raw source-node selector (for example
    /// [`crate::scale::ScaleOperation::RestBindUniformScale`]'s
    /// `source_root_node_index`/skin joints) into the normalized
    /// [`Skeleton`] must use this field rather than assuming source-node
    /// order equals bone order.
    ///
    /// With [`SourceSkeletonCoverage::Complete`] coverage, the `Some` rows
    /// must form a downward-closed, parent-preserving projection into the
    /// normalized skeleton; [`validate_document_shape`] verifies that fact.
    pub bone: Option<BoneId>,
}

impl SourceNodeAsset {
    /// One source node identified by its stable source-array index and its
    /// authored local rest — the two facts every loader necessarily has.
    ///
    /// Every remaining fact ([`Self::name`], [`Self::parent_source_node_index`],
    /// [`Self::scene_root_indices`], [`Self::bone`]) starts absent and is
    /// assigned through the public fields. This is the only way to build the
    /// value outside `animsmith-core`, since the type is `#[non_exhaustive]`.
    pub fn new(source_node_index: usize, local_rest: SourceNodeLocalRest) -> Self {
        Self {
            source_node_index,
            name: None,
            parent_source_node_index: None,
            scene_root_indices: Vec::new(),
            local_rest,
            bone: None,
        }
    }
}

/// Read status for a source skin's inverse-bind accessor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceInverseBindAccessorStatus {
    /// The skin did not declare an inverse-bind accessor.
    #[default]
    Absent,
    /// The accessor was readable and had at least one matrix per declared joint.
    Available,
    /// The source declared a count-zero inverse-bind accessor.
    EmptyAccessor,
    /// The accessor was readable but has fewer matrices than declared joints.
    CountMismatch,
    /// The source declared an accessor that the loader could not read.
    Unreadable,
}

/// Read-side evidence for one source skin inverse-bind accessor.
#[derive(Debug, Clone, Default)]
pub struct SourceInverseBindAccessor {
    /// Whether the accessor was absent, complete, or malformed.
    pub status: SourceInverseBindAccessorStatus,
    /// Declared source accessor count, or `None` when no accessor was declared.
    pub declared_count: Option<usize>,
    /// Raw matrices in accessor order when they were readable.
    ///
    /// This may contain non-finite values from a parseable binary accessor.
    /// Measurement serialization must classify those values rather than emit
    /// non-finite JSON numbers.
    pub matrices: Vec<Mat4>,
}

/// One source node that declares use of a source skin.
#[derive(Debug, Clone)]
pub struct SourceSkinAttachment {
    /// Stable node-array index of the attachment node.
    pub source_node_index: usize,
    /// Stable source mesh-definition index, when the node declares a mesh.
    ///
    /// This remains present even when the current core mesh importer skips the
    /// definition (for example, because it has no triangle-list primitive).
    pub source_mesh_index: Option<usize>,
}

/// One source skin definition, kept separate from bone-level convenience data.
#[derive(Debug, Clone, Default)]
pub struct SourceSkinAsset {
    /// Stable skin-array index in the source format.
    pub source_skin_index: usize,
    /// Authored skin name, when present.
    pub name: Option<String>,
    /// Explicitly declared skeleton root, when present; never inferred.
    pub skeleton_root_source_node_index: Option<usize>,
    /// Source joints in declared skin-slot order.
    pub joint_source_node_indices: Vec<usize>,
    /// Exact inverse-bind accessor evidence for this skin.
    pub inverse_bind_accessor: SourceInverseBindAccessor,
    /// Source nodes that reference this skin, in source-node order.
    pub attachments: Vec<SourceSkinAttachment>,
}

/// Source-node and source-skin evidence carried beside normalized scene assets.
#[derive(Debug, Clone, Default)]
pub struct SourceSkeletonAssets {
    /// Whether these source tables are complete for the loaded input.
    pub coverage: SourceSkeletonCoverage,
    /// Source nodes in stable source-node order.
    pub nodes: Vec<SourceNodeAsset>,
    /// Source skins in stable source-skin order.
    pub skins: Vec<SourceSkinAsset>,
}

/// An embedded texture: raw encoded image bytes (glTF embeds the file
/// as-is, no decoding).
#[derive(Debug, Clone)]
pub struct TextureAsset {
    /// Encoded image bytes.
    pub bytes: Vec<u8>,
    /// "image/png" or "image/jpeg".
    pub mime: String,
}

/// A normal-map texture and the scalar applied to its X/Y components.
///
/// Keeping the scale beside the texture makes the glTF normal-texture state
/// atomic: a scale cannot accidentally survive after its texture is removed.
#[derive(Debug, Clone)]
pub struct NormalTextureAsset {
    /// Embedded encoded normal-map image.
    pub texture: TextureAsset,
    /// Scalar multiplier for the decoded tangent-space X/Y components.
    pub scale: f32,
}

/// An occlusion texture and the scalar applied to its sampled value.
///
/// Keeping the strength beside the texture makes the glTF occlusion-texture
/// state atomic: a strength cannot accidentally survive after its texture is
/// removed.
#[derive(Debug, Clone)]
pub struct OcclusionTextureAsset {
    /// Embedded encoded occlusion texture.
    pub texture: TextureAsset,
    /// Scalar multiplier for the sampled occlusion value.
    pub strength: f32,
}

/// PBR material factors plus optional embedded glTF texture slots.
#[derive(Debug, Clone)]
pub struct MaterialAsset {
    /// Material name.
    pub name: String,
    /// Multiplied with the texture when one is present (set to white
    /// by the FBX loader in that case, matching exporter convention).
    pub base_color: [f32; 4],
    /// Metallic factor.
    pub metallic: f32,
    /// Roughness factor.
    pub roughness: f32,
    /// Embedded base-color texture, if one was loaded.
    pub base_color_texture: Option<TextureAsset>,
    /// Embedded tangent-space normal texture, if one was loaded.
    pub normal_texture: Option<NormalTextureAsset>,
    /// Embedded metallic-roughness texture, if one was loaded.
    ///
    /// glTF stores roughness in green and metallic in blue.
    pub metallic_roughness_texture: Option<TextureAsset>,
    /// Embedded occlusion texture, if one was loaded.
    pub occlusion_texture: Option<OcclusionTextureAsset>,
}

/// Whether source material-resource inspection covers the whole input.
///
/// This sidecar is deliberately separate from writer-facing [`MaterialAsset`]
/// values. A loader may preserve materials for writing while declining to
/// inspect resource provenance or decode image metadata.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaterialResourceCoverage {
    /// The loader inspected its complete documented source material-resource
    /// domain. Format-specific documentation defines which binding slots that
    /// domain includes.
    Complete,
    /// The loader cannot provide source resource evidence.
    #[default]
    Unavailable,
}

/// A material texture slot with stable source-format meaning.
///
/// Declaration order is the stable wire order used by material-resource
/// measurements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaterialTextureSlot {
    /// Base-color texture.
    BaseColor,
    /// Tangent-space normal texture.
    Normal,
    /// Combined metallic-roughness texture.
    MetallicRoughness,
    /// Occlusion texture.
    Occlusion,
    /// Emissive texture.
    Emissive,
}

/// One source material-to-texture binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceMaterialTextureBinding {
    /// Material slot in stable semantic order.
    pub slot: MaterialTextureSlot,
    /// Stable source texture index.
    pub texture_index: usize,
}

/// One source material definition, independent of writer-facing material data.
#[derive(Debug, Clone, Default)]
pub struct SourceMaterialAsset {
    /// Stable source material index.
    pub material_index: usize,
    /// Authored name, when present.
    pub name: Option<String>,
    /// Source texture bindings, sorted by [`SourceMaterialTextureBinding::slot`].
    pub texture_bindings: Vec<SourceMaterialTextureBinding>,
}

/// One source texture definition.
#[derive(Debug, Clone, Default)]
pub struct SourceTextureAsset {
    /// Stable source texture index.
    pub texture_index: usize,
    /// Authored name, when present.
    pub name: Option<String>,
    /// Stable source image index referenced by this texture.
    pub image_index: usize,
}

/// How an image payload was declared by its source format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageSourceKind {
    /// Bytes embedded directly in a container record.
    Embedded,
    /// Bytes encoded in a data URI.
    DataUri,
    /// A relative or otherwise external resource reference.
    External,
}

/// Image container format recognized by inspection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageContainerFormat {
    /// PNG image data.
    Png,
    /// JPEG image data.
    Jpeg,
}

/// Decoded image color representation reported by inspection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecodedImageColorType {
    /// Single-channel, 8-bit luminance.
    L8,
    /// Luminance plus alpha, 8-bit channels.
    La8,
    /// RGB, 8-bit channels.
    Rgb8,
    /// RGBA, 8-bit channels.
    Rgba8,
    /// Single-channel, 16-bit luminance.
    L16,
    /// Luminance plus alpha, 16-bit channels.
    La16,
    /// RGB, 16-bit channels.
    Rgb16,
    /// RGBA, 16-bit channels.
    Rgba16,
}

/// Why source-image inspection could not produce decoded metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageUnavailableReason {
    /// The source does not make the image payload available to the loader.
    SourceUnavailable,
    /// A data URI could not be parsed or decoded.
    InvalidDataUri,
    /// The image container is not supported for inspection.
    UnsupportedContainer,
    /// Supported image bytes could not be decoded.
    DecodeFailed,
    /// Inspection declined the resource because it exceeded a resource limit.
    ResourceLimit,
}

/// Result of bounded source-image inspection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceImageInspection {
    /// Decoded metadata was available without retaining decoded pixels.
    Available {
        /// Pixel width.
        width: u32,
        /// Pixel height.
        height: u32,
        /// Number of decoded channels.
        channel_count: u8,
        /// Decoded color representation.
        color_type: DecodedImageColorType,
    },
    /// Inspection could not provide decoded metadata.
    Unavailable {
        /// Stable unavailability reason.
        reason: ImageUnavailableReason,
    },
}

/// One source image definition and bounded inspection result.
#[derive(Debug, Clone)]
pub struct SourceImageAsset {
    /// Stable source image index.
    pub image_index: usize,
    /// Authored name, when present.
    pub name: Option<String>,
    /// Source declaration kind.
    pub source_kind: ImageSourceKind,
    /// MIME type declared by the source, when present.
    pub declared_mime_type: Option<String>,
    /// Detected container format, when recognisable.
    pub detected_container: Option<ImageContainerFormat>,
    /// Bounded image-inspection result.
    pub inspection: SourceImageInspection,
}

/// Read-only source material-resource evidence carried beside scene assets.
#[derive(Debug, Clone, Default)]
pub struct MaterialResourceAssets {
    /// Whether the source resource lists are complete.
    pub coverage: MaterialResourceCoverage,
    /// Source materials in source order.
    pub materials: Vec<SourceMaterialAsset>,
    /// Source textures in source order.
    pub textures: Vec<SourceTextureAsset>,
    /// Source images in source order.
    pub images: Vec<SourceImageAsset>,
}

impl Primitive {
    /// Dedupe identical corners into indexed triangles. Exact
    /// bit-equality only — no tolerance welding, so seams authored via
    /// split normals/UVs are preserved.
    pub fn weld(&mut self) {
        if !self.indices.is_empty() || self.positions.is_empty() {
            return;
        }
        let corner_key = |i: usize| -> Vec<u8> {
            let mut key = Vec::with_capacity(64);
            let mut push_f32s = |vals: &[f32]| {
                for v in vals {
                    key.extend_from_slice(&v.to_le_bytes());
                }
            };
            push_f32s(&self.positions[i].to_array());
            if let Some(n) = self.normals.get(i) {
                push_f32s(&n.to_array());
            }
            if let Some(uv) = self.uvs.get(i) {
                push_f32s(uv);
            }
            if let Some(w) = self.weights.get(i) {
                push_f32s(w);
            }
            if let Some(j) = self.joints.get(i) {
                for v in j {
                    key.extend_from_slice(&v.to_le_bytes());
                }
            }
            key
        };
        let mut seen: std::collections::HashMap<Vec<u8>, u32> = std::collections::HashMap::new();
        let mut indices = Vec::with_capacity(self.positions.len());
        let mut positions = Vec::new();
        let mut normals = Vec::new();
        let mut uvs = Vec::new();
        let mut joints = Vec::new();
        let mut weights = Vec::new();
        for i in 0..self.positions.len() {
            let index = *seen.entry(corner_key(i)).or_insert_with(|| {
                positions.push(self.positions[i]);
                if let Some(n) = self.normals.get(i) {
                    normals.push(*n);
                }
                if let Some(uv) = self.uvs.get(i) {
                    uvs.push(*uv);
                }
                if let Some(j) = self.joints.get(i) {
                    joints.push(*j);
                }
                if let Some(w) = self.weights.get(i) {
                    weights.push(*w);
                }
                (positions.len() - 1) as u32
            });
            indices.push(index);
        }
        self.indices = indices;
        self.positions = positions;
        self.normals = normals;
        self.uvs = uvs;
        self.joints = joints;
        self.weights = weights;
    }
}

/// Mesh definitions, their node instances, scenes, and materials carried
/// alongside animation data.
#[derive(Debug, Clone, Default)]
pub struct SceneAssets {
    /// Mesh definitions in source order, including definitions without a node
    /// instance.
    pub meshes: Vec<MeshAsset>,
    /// Node instances of the mesh definitions, in source node order.
    pub instances: Vec<MeshInstance>,
    /// Materials referenced by mesh primitives.
    pub materials: Vec<MaterialAsset>,
    /// Read-only source material, texture, and image evidence for measurement.
    /// Writer-facing material slots remain in [`Self::materials`].
    pub material_resources: MaterialResourceAssets,
    /// Declared source scenes in source order.
    pub scenes: Vec<SceneAsset>,
    /// Source scene index selected by default, when one was declared.
    pub default_scene: Option<usize>,
    /// Source-node and source-skin identity evidence for skeleton measurements.
    ///
    /// This is intentionally separate from the normalized [`Skeleton`] and
    /// from [`MeshInstance::skin_ibms`]: a source node order need not match
    /// FK order, and one joint can have different inverse binds in different
    /// source skins.
    pub source_skeleton: SourceSkeletonAssets,
}

/// Validate the enumerated structural snapshot strict document operations
/// rely on.
///
/// This is a snapshot only: [`Document`] is publicly mutable, so a successful
/// call does not certify a document against later mutation. Any strict
/// operation that relies on this full shape must rerun validation at its own
/// public boundary.
/// Tolerant analysis APIs may intentionally accept documents this rejects and
/// preserve the valid evidence they can read. This function does not validate
/// operation-specific capability, affine, closure, proof, or payload
/// invariants such as primitive skinning shape and base positions.
///
/// # Errors
///
/// Returns a typed [`DocumentShapeError`] for the first violation in stable
/// validation order: skeleton rest/topology, source identity/projection,
/// tracks, instances, then bone-level inverse binds.
pub fn validate_document_shape(document: &Document) -> Result<(), DocumentShapeError> {
    validate_skeleton_rest(&document.skeleton)?;
    validate_source_skeleton_identity(&document.assets.source_skeleton)?;
    validate_source_projection(document)?;
    validate_clip_tracks(document)?;
    validate_mesh_instances(document)?;
    validate_bone_inverse_binds(&document.skeleton)
}

fn validate_skeleton_rest(skeleton: &Skeleton) -> Result<(), DocumentShapeError> {
    world_rest_matrices(skeleton)
        .map(|_| ())
        .map_err(|error| match error {
            WorldMatrixError::NonFiniteTransform { node } => {
                DocumentShapeError::NonFiniteSkeletonRest { node }
            }
            WorldMatrixError::InvalidParent { node, parent } => {
                DocumentShapeError::InvalidSkeletonParent { node, parent }
            }
        })
}

fn validate_source_skeleton_identity(
    source_skeleton: &SourceSkeletonAssets,
) -> Result<(), DocumentShapeError> {
    let mut seen_nodes = BTreeSet::new();
    for node in &source_skeleton.nodes {
        if !seen_nodes.insert(node.source_node_index) {
            return Err(DocumentShapeError::DuplicateSourceNodeIndex {
                source_node_index: node.source_node_index,
            });
        }
    }
    let mut seen_skins = BTreeSet::new();
    for skin in &source_skeleton.skins {
        if !seen_skins.insert(skin.source_skin_index) {
            return Err(DocumentShapeError::DuplicateSourceSkinIndex {
                source_skin_index: skin.source_skin_index,
            });
        }
    }
    Ok(())
}

/// Validate the identity relation a `Complete` source projection claims.
///
/// Projected rows must be injective, preserve each bone's nearest projected
/// ancestor, and be downward-closed in the normalized skeleton. Unprojected
/// source rows may remain between projected ancestors, and unrelated
/// unprojected roots remain legal; totality is not required. Non-`Complete`
/// rows are not identity evidence and are deliberately ignored.
///
/// The rule is load-bearing for consumers such as scale that select a rewrite
/// domain through source-node ancestry but apply and prove it through
/// [`Skeleton::bones`]. Without agreement, a normalized child can sit outside
/// the selected source closure while its parent moves, leaving its displaced
/// world rest outside every declared proof walk.
fn validate_source_projection(document: &Document) -> Result<(), DocumentShapeError> {
    let source_skeleton = &document.assets.source_skeleton;
    if source_skeleton.coverage != SourceSkeletonCoverage::Complete {
        return Ok(());
    }

    let bones = &document.skeleton.bones;
    let mut bone_of_source = BTreeMap::new();
    let mut source_of_bone = BTreeMap::new();
    let mut skeleton_parents = Vec::with_capacity(source_skeleton.nodes.len());
    for node in &source_skeleton.nodes {
        let Some(bone) = node.bone else {
            continue;
        };
        let skeleton_parent = bones
            .get(bone)
            .ok_or(DocumentShapeError::SourceProjection {
                source_node_index: node.source_node_index,
                violation: SourceProjectionViolation::ProjectedBoneOutOfRange,
            })?
            .parent;
        if source_of_bone
            .insert(bone, node.source_node_index)
            .is_some()
        {
            return Err(DocumentShapeError::SourceProjection {
                source_node_index: node.source_node_index,
                violation: SourceProjectionViolation::TwoSourceNodesProjectToOneBone,
            });
        }
        bone_of_source.insert(node.source_node_index, bone);
        skeleton_parents.push((node, skeleton_parent));
    }

    let by_source_index: BTreeMap<_, _> = source_skeleton
        .nodes
        .iter()
        .map(|node| (node.source_node_index, node))
        .collect();
    let unprojected_rows = source_skeleton.nodes.len() - bone_of_source.len();
    for (node, skeleton_parent) in skeleton_parents {
        let mut cursor = node.parent_source_node_index;
        let mut steps = 0usize;
        let projected_parent = loop {
            let Some(parent_source_node_index) = cursor else {
                break None;
            };
            if let Some(&bone) = bone_of_source.get(&parent_source_node_index) {
                break Some(bone);
            }
            let parent = by_source_index.get(&parent_source_node_index).ok_or(
                DocumentShapeError::SourceProjection {
                    source_node_index: node.source_node_index,
                    violation: SourceProjectionViolation::ParentSourceNodeMissing,
                },
            )?;
            steps += 1;
            if steps > unprojected_rows {
                return Err(DocumentShapeError::SourceProjection {
                    source_node_index: node.source_node_index,
                    violation: SourceProjectionViolation::CyclicUnprojectedSourceParentChain,
                });
            }
            cursor = parent.parent_source_node_index;
        };
        if projected_parent != skeleton_parent {
            return Err(DocumentShapeError::SourceProjection {
                source_node_index: node.source_node_index,
                violation: SourceProjectionViolation::NearestProjectedParentMismatch,
            });
        }
    }

    for (bone, child) in bones.iter().enumerate() {
        if source_of_bone.contains_key(&bone) {
            continue;
        }
        if let Some(parent) = child.parent
            && let Some(&source_node_index) = source_of_bone.get(&parent)
        {
            return Err(DocumentShapeError::SourceProjection {
                source_node_index,
                violation: SourceProjectionViolation::ProjectedBoneHasUnprojectedSkeletonChild,
            });
        }
    }
    Ok(())
}

fn validate_clip_tracks(document: &Document) -> Result<(), DocumentShapeError> {
    let bone_count = document.skeleton.bones.len();
    for (clip_index, clip) in document.clips.iter().enumerate() {
        let mut seen = Vec::with_capacity(clip.tracks.len());
        for track in &clip.tracks {
            if track.bone >= bone_count {
                return Err(DocumentShapeError::TrackShape {
                    clip_index,
                    node: track.bone,
                    violation: TrackShapeViolation::BoneIndexOutOfRange,
                });
            }
            if seen.contains(&(track.bone, track.property)) {
                return Err(DocumentShapeError::DuplicateClipTrack {
                    clip_index,
                    node: track.bone,
                    property: track.property,
                });
            }
            seen.push((track.bone, track.property));
            validate_track_shape(clip_index, track)?;
        }
    }
    Ok(())
}

fn validate_track_shape(clip_index: usize, track: &Track) -> Result<(), DocumentShapeError> {
    let violation = if track.times.is_empty() {
        Some(TrackShapeViolation::EmptyTimes)
    } else if track.times.iter().any(|time| !time.is_finite()) {
        Some(TrackShapeViolation::NonFiniteTime)
    } else if track.times.windows(2).any(|times| times[0] >= times[1]) {
        Some(TrackShapeViolation::TimesNotStrictlyIncreasing)
    } else {
        let expected_values = match track.interpolation {
            Interpolation::CubicSpline => track.times.len().checked_mul(3),
            Interpolation::Linear | Interpolation::Step => Some(track.times.len()),
        };
        if expected_values != Some(track.values.len()) {
            Some(TrackShapeViolation::ValueCountMismatch)
        } else if !matches!(
            (&track.values, track.property),
            (
                TrackValues::Vec3s(_),
                Property::Translation | Property::Scale
            ) | (TrackValues::Quats(_), Property::Rotation)
        ) {
            Some(TrackShapeViolation::ValueTypeMismatchesProperty)
        } else if match &track.values {
            TrackValues::Vec3s(values) => values.iter().any(|value| !value.is_finite()),
            TrackValues::Quats(values) => values.iter().any(|value| !value.is_finite()),
        } {
            Some(TrackShapeViolation::NonFiniteValue)
        } else {
            None
        }
    };
    violation.map_or(Ok(()), |violation| {
        Err(DocumentShapeError::TrackShape {
            clip_index,
            node: track.bone,
            violation,
        })
    })
}

fn validate_mesh_instances(document: &Document) -> Result<(), DocumentShapeError> {
    let bone_count = document.skeleton.bones.len();
    let mesh_count = document.assets.meshes.len();
    for (instance_index, instance) in document.assets.instances.iter().enumerate() {
        let violation = if instance.node >= bone_count {
            Some(MeshInstanceShapeViolation::NodeIndexOutOfRange)
        } else if instance.mesh >= mesh_count {
            Some(MeshInstanceShapeViolation::MeshIndexOutOfRange)
        } else if instance
            .skin_joints
            .iter()
            .any(|&joint| joint >= bone_count)
        {
            Some(MeshInstanceShapeViolation::SkinJointOutOfRange)
        } else if !instance.skin_ibms.is_empty()
            && instance.skin_ibms.len() != instance.skin_joints.len()
        {
            Some(MeshInstanceShapeViolation::SkinInverseBindCountMismatch)
        } else if instance.skin_ibms.iter().any(|ibm| !mat4_is_finite(*ibm)) {
            Some(MeshInstanceShapeViolation::NonFiniteSkinInverseBind)
        } else {
            None
        };
        if let Some(violation) = violation {
            return Err(DocumentShapeError::MeshInstanceShape {
                instance_index,
                violation,
            });
        }
    }
    Ok(())
}

fn validate_bone_inverse_binds(skeleton: &Skeleton) -> Result<(), DocumentShapeError> {
    for (node, bone) in skeleton.bones.iter().enumerate() {
        if let Some(inverse_bind) = bone.inverse_bind
            && !mat4_is_finite(inverse_bind)
        {
            return Err(DocumentShapeError::NonFiniteBoneInverseBind { node });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bone(parent: Option<BoneId>) -> Bone {
        Bone {
            name: "bone".into(),
            parent,
            rest: Transform::IDENTITY,
            inverse_bind: None,
        }
    }

    fn one_bone_document() -> Document {
        Document {
            skeleton: Skeleton {
                bones: vec![bone(None)],
            },
            ..Document::default()
        }
    }

    fn source_node(
        source_node_index: usize,
        parent_source_node_index: Option<usize>,
        bone: Option<BoneId>,
    ) -> SourceNodeAsset {
        SourceNodeAsset {
            source_node_index,
            name: None,
            parent_source_node_index,
            scene_root_indices: Vec::new(),
            local_rest: SourceNodeLocalRest::Trs {
                translation: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
            },
            bone,
        }
    }

    fn valid_track() -> Track {
        Track {
            bone: 0,
            property: Property::Translation,
            interpolation: Interpolation::Linear,
            times: vec![0.0],
            values: TrackValues::Vec3s(vec![Vec3::ZERO]),
        }
    }

    fn track_document(track: Track) -> Document {
        let mut document = one_bone_document();
        document.clips.push(Clip {
            name: "clip".into(),
            duration_s: 0.0,
            tracks: vec![track],
        });
        document
    }

    fn instance_document() -> Document {
        let mut document = one_bone_document();
        document.assets.meshes.push(MeshAsset::default());
        document.assets.instances.push(MeshInstance {
            node: 0,
            mesh: 0,
            ..MeshInstance::default()
        });
        document
    }

    #[test]
    fn document_shape_validation_accepts_a_complete_projection_with_an_unprojected_intermediate() {
        let mut document = Document {
            skeleton: Skeleton {
                bones: vec![bone(None), bone(Some(0))],
            },
            assets: SceneAssets {
                source_skeleton: SourceSkeletonAssets {
                    coverage: SourceSkeletonCoverage::Complete,
                    nodes: vec![
                        source_node(10, None, Some(0)),
                        source_node(11, Some(10), None),
                        source_node(12, Some(11), Some(1)),
                    ],
                    ..SourceSkeletonAssets::default()
                },
                meshes: vec![MeshAsset::default()],
                instances: vec![MeshInstance {
                    node: 1,
                    mesh: 0,
                    skin_joints: vec![0, 1],
                    skin_ibms: vec![Mat4::IDENTITY, Mat4::IDENTITY],
                    ..MeshInstance::default()
                }],
                ..SceneAssets::default()
            },
            ..Document::default()
        };
        document.clips.push(Clip {
            name: "clip".into(),
            duration_s: 0.0,
            tracks: vec![valid_track()],
        });

        assert_eq!(validate_document_shape(&document), Ok(()));
    }

    #[test]
    fn document_shape_validation_has_an_analytic_error_for_every_variant() {
        let projection_error =
            |source_node_index, violation| DocumentShapeError::SourceProjection {
                source_node_index,
                violation,
            };
        let track_error = |node, violation| DocumentShapeError::TrackShape {
            clip_index: 0,
            node,
            violation,
        };
        let instance_error = |violation| DocumentShapeError::MeshInstanceShape {
            instance_index: 0,
            violation,
        };

        let mut non_finite_rest = one_bone_document();
        non_finite_rest.skeleton.bones[0].rest.translation.x = f32::NAN;
        let overflowed_rest_world = Document {
            skeleton: Skeleton {
                bones: vec![
                    Bone {
                        rest: Transform {
                            scale: Vec3::splat(f32::MAX),
                            ..Transform::IDENTITY
                        },
                        ..bone(None)
                    },
                    Bone {
                        rest: Transform {
                            translation: Vec3::splat(2.0),
                            ..Transform::IDENTITY
                        },
                        ..bone(Some(0))
                    },
                ],
            },
            ..Document::default()
        };
        let self_parent = Document {
            skeleton: Skeleton {
                bones: vec![bone(Some(0))],
            },
            ..Document::default()
        };
        let forward_parent = Document {
            skeleton: Skeleton {
                bones: vec![bone(Some(1)), bone(None)],
            },
            ..Document::default()
        };
        let far_parent = Document {
            skeleton: Skeleton {
                bones: vec![bone(Some(99))],
            },
            ..Document::default()
        };
        let duplicate_node = Document {
            assets: SceneAssets {
                source_skeleton: SourceSkeletonAssets {
                    nodes: vec![
                        source_node(9, None, None),
                        source_node(10, None, None),
                        source_node(9, None, None),
                    ],
                    ..SourceSkeletonAssets::default()
                },
                ..SceneAssets::default()
            },
            ..Document::default()
        };
        let duplicate_skin = Document {
            assets: SceneAssets {
                source_skeleton: SourceSkeletonAssets {
                    skins: vec![
                        SourceSkinAsset {
                            source_skin_index: 4,
                            ..SourceSkinAsset::default()
                        },
                        SourceSkinAsset {
                            source_skin_index: 5,
                            ..SourceSkinAsset::default()
                        },
                        SourceSkinAsset {
                            source_skin_index: 4,
                            ..SourceSkinAsset::default()
                        },
                    ],
                    ..SourceSkeletonAssets::default()
                },
                ..SceneAssets::default()
            },
            ..Document::default()
        };
        let complete_projection = |nodes| SceneAssets {
            source_skeleton: SourceSkeletonAssets {
                coverage: SourceSkeletonCoverage::Complete,
                nodes,
                ..SourceSkeletonAssets::default()
            },
            ..SceneAssets::default()
        };
        let out_of_range_projection = Document {
            skeleton: Skeleton {
                bones: vec![bone(None)],
            },
            assets: complete_projection(vec![source_node(10, None, Some(1))]),
            ..Document::default()
        };
        let non_injective_projection = Document {
            skeleton: Skeleton {
                bones: vec![bone(None)],
            },
            assets: complete_projection(vec![
                source_node(10, None, Some(0)),
                source_node(11, None, Some(0)),
            ]),
            ..Document::default()
        };
        let missing_projection_parent = Document {
            skeleton: Skeleton {
                bones: vec![bone(None), bone(Some(0))],
            },
            assets: complete_projection(vec![source_node(11, Some(99), Some(1))]),
            ..Document::default()
        };
        let cyclic_unprojected_parent = Document {
            skeleton: Skeleton {
                bones: vec![bone(None), bone(Some(0))],
            },
            assets: complete_projection(vec![
                source_node(11, Some(12), Some(1)),
                source_node(12, Some(12), None),
            ]),
            ..Document::default()
        };
        let cyclic_unprojected_parent_pair = Document {
            skeleton: Skeleton {
                bones: vec![bone(None), bone(Some(0))],
            },
            assets: complete_projection(vec![
                source_node(11, Some(12), Some(1)),
                source_node(12, Some(13), None),
                source_node(13, Some(12), None),
            ]),
            ..Document::default()
        };
        let mismatched_nearest_parent = Document {
            skeleton: Skeleton {
                bones: vec![bone(None), bone(Some(0))],
            },
            assets: complete_projection(vec![
                source_node(10, None, Some(0)),
                source_node(11, None, Some(1)),
            ]),
            ..Document::default()
        };
        let unprojected_child = Document {
            skeleton: Skeleton {
                bones: vec![bone(None), bone(Some(0))],
            },
            assets: complete_projection(vec![source_node(10, None, Some(0))]),
            ..Document::default()
        };

        let duplicate_track = {
            let track = valid_track();
            let mut document = track_document(track.clone());
            document.clips[0].tracks.push(Track {
                property: Property::Scale,
                ..valid_track()
            });
            document.clips[0].tracks.push(track);
            document
        };
        let mut boundary_out_of_range_track = valid_track();
        boundary_out_of_range_track.bone = 1;
        let mut far_out_of_range_track = valid_track();
        far_out_of_range_track.bone = 99;
        let empty_track = Track {
            times: Vec::new(),
            values: TrackValues::Vec3s(Vec::new()),
            ..valid_track()
        };
        let non_finite_later_time = Track {
            times: vec![0.0, f32::NAN],
            values: TrackValues::Vec3s(vec![Vec3::ZERO, Vec3::ZERO]),
            ..valid_track()
        };
        let unordered_times = Track {
            times: vec![1.0, 0.0],
            values: TrackValues::Vec3s(vec![Vec3::ZERO, Vec3::ZERO]),
            ..valid_track()
        };
        let equal_times = Track {
            times: vec![0.0, 0.0],
            values: TrackValues::Vec3s(vec![Vec3::ZERO, Vec3::ZERO]),
            ..valid_track()
        };
        let wrong_linear_value_count = Track {
            values: TrackValues::Vec3s(Vec::new()),
            ..valid_track()
        };
        let excess_linear_value_count = Track {
            values: TrackValues::Vec3s(vec![Vec3::ZERO, Vec3::ZERO]),
            ..valid_track()
        };
        let wrong_step_value_count = Track {
            interpolation: Interpolation::Step,
            times: vec![0.0, 1.0],
            values: TrackValues::Vec3s(vec![Vec3::ZERO]),
            ..valid_track()
        };
        let excess_step_value_count = Track {
            interpolation: Interpolation::Step,
            values: TrackValues::Vec3s(vec![Vec3::ZERO, Vec3::ZERO]),
            ..valid_track()
        };
        let wrong_cubic_value_count = Track {
            interpolation: Interpolation::CubicSpline,
            times: vec![0.0, 1.0],
            values: TrackValues::Vec3s(vec![Vec3::ZERO; 4]),
            ..valid_track()
        };
        let excess_cubic_value_count = Track {
            interpolation: Interpolation::CubicSpline,
            values: TrackValues::Vec3s(vec![Vec3::ZERO; 4]),
            ..valid_track()
        };
        let wrong_translation_value_type = Track {
            values: TrackValues::Quats(vec![Quat::IDENTITY]),
            ..valid_track()
        };
        let wrong_scale_value_type = Track {
            property: Property::Scale,
            values: TrackValues::Quats(vec![Quat::IDENTITY]),
            ..valid_track()
        };
        let wrong_rotation_value_type = Track {
            property: Property::Rotation,
            values: TrackValues::Vec3s(vec![Vec3::ZERO]),
            ..valid_track()
        };
        let non_finite_value = Track {
            values: TrackValues::Vec3s(vec![Vec3::splat(f32::NAN)]),
            ..valid_track()
        };

        let mut bad_instance_node = instance_document();
        bad_instance_node.assets.instances[0].node = 1;
        let mut far_instance_node = instance_document();
        far_instance_node.assets.instances[0].node = 99;
        let mut bad_instance_mesh = instance_document();
        bad_instance_mesh.assets.instances[0].mesh = 1;
        let mut far_instance_mesh = instance_document();
        far_instance_mesh.assets.instances[0].mesh = 99;
        let mut bad_instance_joint = instance_document();
        bad_instance_joint.assets.instances[0].skin_joints = vec![1];
        let mut far_instance_joint = instance_document();
        far_instance_joint.assets.instances[0].skin_joints = vec![99];
        let mut bad_instance_count = instance_document();
        bad_instance_count.assets.instances[0].skin_joints = vec![0];
        bad_instance_count.assets.instances[0].skin_ibms = vec![Mat4::IDENTITY, Mat4::IDENTITY];
        let mut short_instance_count = instance_document();
        short_instance_count.skeleton.bones.push(bone(Some(0)));
        short_instance_count.assets.instances[0].skin_joints = vec![0, 1];
        short_instance_count.assets.instances[0].skin_ibms = vec![Mat4::IDENTITY];
        let mut bad_instance_ibm = instance_document();
        bad_instance_ibm.assets.instances[0].skin_joints = vec![0];
        bad_instance_ibm.assets.instances[0].skin_ibms =
            vec![Mat4::from_cols_array(&[f32::NAN; 16])];
        let mut bad_bone_ibm = one_bone_document();
        bad_bone_ibm.skeleton.bones[0].inverse_bind = Some(Mat4::from_cols_array(&[f32::NAN; 16]));

        let cases = vec![
            (
                "non-finite rest",
                non_finite_rest,
                DocumentShapeError::NonFiniteSkeletonRest { node: 0 },
            ),
            (
                "non-finite composed rest world",
                overflowed_rest_world,
                DocumentShapeError::NonFiniteSkeletonRest { node: 1 },
            ),
            (
                "self parent",
                self_parent,
                DocumentShapeError::InvalidSkeletonParent { node: 0, parent: 0 },
            ),
            (
                "forward parent",
                forward_parent,
                DocumentShapeError::InvalidSkeletonParent { node: 0, parent: 1 },
            ),
            (
                "far parent",
                far_parent,
                DocumentShapeError::InvalidSkeletonParent {
                    node: 0,
                    parent: 99,
                },
            ),
            (
                "duplicate source node",
                duplicate_node,
                DocumentShapeError::DuplicateSourceNodeIndex {
                    source_node_index: 9,
                },
            ),
            (
                "duplicate source skin",
                duplicate_skin,
                DocumentShapeError::DuplicateSourceSkinIndex {
                    source_skin_index: 4,
                },
            ),
            (
                "projected bone range",
                out_of_range_projection,
                projection_error(10, SourceProjectionViolation::ProjectedBoneOutOfRange),
            ),
            (
                "projection injectivity",
                non_injective_projection,
                projection_error(
                    11,
                    SourceProjectionViolation::TwoSourceNodesProjectToOneBone,
                ),
            ),
            (
                "missing projection parent",
                missing_projection_parent,
                projection_error(11, SourceProjectionViolation::ParentSourceNodeMissing),
            ),
            (
                "cyclic projection parent",
                cyclic_unprojected_parent,
                projection_error(
                    11,
                    SourceProjectionViolation::CyclicUnprojectedSourceParentChain,
                ),
            ),
            (
                "cyclic projection parent pair",
                cyclic_unprojected_parent_pair,
                projection_error(
                    11,
                    SourceProjectionViolation::CyclicUnprojectedSourceParentChain,
                ),
            ),
            (
                "nearest projection parent",
                mismatched_nearest_parent,
                projection_error(
                    11,
                    SourceProjectionViolation::NearestProjectedParentMismatch,
                ),
            ),
            (
                "projection downward closure",
                unprojected_child,
                projection_error(
                    10,
                    SourceProjectionViolation::ProjectedBoneHasUnprojectedSkeletonChild,
                ),
            ),
            (
                "duplicate track",
                duplicate_track,
                DocumentShapeError::DuplicateClipTrack {
                    clip_index: 0,
                    node: 0,
                    property: Property::Translation,
                },
            ),
            (
                "track bone range boundary",
                track_document(boundary_out_of_range_track),
                track_error(1, TrackShapeViolation::BoneIndexOutOfRange),
            ),
            (
                "track bone range far",
                track_document(far_out_of_range_track),
                track_error(99, TrackShapeViolation::BoneIndexOutOfRange),
            ),
            (
                "empty track",
                track_document(empty_track),
                track_error(0, TrackShapeViolation::EmptyTimes),
            ),
            (
                "non-finite time",
                track_document(non_finite_later_time),
                track_error(0, TrackShapeViolation::NonFiniteTime),
            ),
            (
                "unordered times",
                track_document(unordered_times),
                track_error(0, TrackShapeViolation::TimesNotStrictlyIncreasing),
            ),
            (
                "equal times",
                track_document(equal_times),
                track_error(0, TrackShapeViolation::TimesNotStrictlyIncreasing),
            ),
            (
                "linear value count",
                track_document(wrong_linear_value_count),
                track_error(0, TrackShapeViolation::ValueCountMismatch),
            ),
            (
                "linear excess value count",
                track_document(excess_linear_value_count),
                track_error(0, TrackShapeViolation::ValueCountMismatch),
            ),
            (
                "step value count",
                track_document(wrong_step_value_count),
                track_error(0, TrackShapeViolation::ValueCountMismatch),
            ),
            (
                "step excess value count",
                track_document(excess_step_value_count),
                track_error(0, TrackShapeViolation::ValueCountMismatch),
            ),
            (
                "cubic value count",
                track_document(wrong_cubic_value_count),
                track_error(0, TrackShapeViolation::ValueCountMismatch),
            ),
            (
                "cubic excess value count",
                track_document(excess_cubic_value_count),
                track_error(0, TrackShapeViolation::ValueCountMismatch),
            ),
            (
                "translation value type",
                track_document(wrong_translation_value_type),
                track_error(0, TrackShapeViolation::ValueTypeMismatchesProperty),
            ),
            (
                "scale value type",
                track_document(wrong_scale_value_type),
                track_error(0, TrackShapeViolation::ValueTypeMismatchesProperty),
            ),
            (
                "rotation value type",
                track_document(wrong_rotation_value_type),
                track_error(0, TrackShapeViolation::ValueTypeMismatchesProperty),
            ),
            (
                "non-finite value",
                track_document(non_finite_value),
                track_error(0, TrackShapeViolation::NonFiniteValue),
            ),
            (
                "instance node boundary",
                bad_instance_node,
                instance_error(MeshInstanceShapeViolation::NodeIndexOutOfRange),
            ),
            (
                "instance node far",
                far_instance_node,
                instance_error(MeshInstanceShapeViolation::NodeIndexOutOfRange),
            ),
            (
                "instance mesh boundary",
                bad_instance_mesh,
                instance_error(MeshInstanceShapeViolation::MeshIndexOutOfRange),
            ),
            (
                "instance mesh far",
                far_instance_mesh,
                instance_error(MeshInstanceShapeViolation::MeshIndexOutOfRange),
            ),
            (
                "instance joint boundary",
                bad_instance_joint,
                instance_error(MeshInstanceShapeViolation::SkinJointOutOfRange),
            ),
            (
                "instance joint far",
                far_instance_joint,
                instance_error(MeshInstanceShapeViolation::SkinJointOutOfRange),
            ),
            (
                "instance ibm count excess",
                bad_instance_count,
                instance_error(MeshInstanceShapeViolation::SkinInverseBindCountMismatch),
            ),
            (
                "instance ibm count short",
                short_instance_count,
                instance_error(MeshInstanceShapeViolation::SkinInverseBindCountMismatch),
            ),
            (
                "instance ibm finite",
                bad_instance_ibm,
                instance_error(MeshInstanceShapeViolation::NonFiniteSkinInverseBind),
            ),
            (
                "bone ibm finite",
                bad_bone_ibm,
                DocumentShapeError::NonFiniteBoneInverseBind { node: 0 },
            ),
        ];
        for (name, document, expected) in cases {
            assert_eq!(validate_document_shape(&document), Err(expected), "{name}");
        }
    }

    #[test]
    fn document_shape_finiteness_checks_every_stored_component() {
        for component in 0..3 {
            let mut translation = Vec3::ZERO.to_array();
            translation[component] = f32::NAN;
            let mut document = one_bone_document();
            document.skeleton.bones[0].rest.translation = Vec3::from_array(translation);
            assert_eq!(
                validate_document_shape(&document),
                Err(DocumentShapeError::NonFiniteSkeletonRest { node: 0 }),
                "rest translation component {component}"
            );

            let mut scale = Vec3::ONE.to_array();
            scale[component] = f32::NAN;
            let mut document = one_bone_document();
            document.skeleton.bones[0].rest.scale = Vec3::from_array(scale);
            assert_eq!(
                validate_document_shape(&document),
                Err(DocumentShapeError::NonFiniteSkeletonRest { node: 0 }),
                "rest scale component {component}"
            );

            let mut value = Vec3::ZERO.to_array();
            value[component] = f32::NAN;
            let document = track_document(Track {
                values: TrackValues::Vec3s(vec![Vec3::from_array(value)]),
                ..valid_track()
            });
            assert_eq!(
                validate_document_shape(&document),
                Err(DocumentShapeError::TrackShape {
                    clip_index: 0,
                    node: 0,
                    violation: TrackShapeViolation::NonFiniteValue,
                }),
                "track Vec3 component {component}"
            );
        }

        for component in 0..4 {
            let mut rotation = Quat::IDENTITY.to_array();
            rotation[component] = f32::NAN;
            let mut document = one_bone_document();
            document.skeleton.bones[0].rest.rotation = Quat::from_array(rotation);
            assert_eq!(
                validate_document_shape(&document),
                Err(DocumentShapeError::NonFiniteSkeletonRest { node: 0 }),
                "rest rotation component {component}"
            );

            let document = track_document(Track {
                property: Property::Rotation,
                values: TrackValues::Quats(vec![Quat::from_array(rotation)]),
                ..valid_track()
            });
            assert_eq!(
                validate_document_shape(&document),
                Err(DocumentShapeError::TrackShape {
                    clip_index: 0,
                    node: 0,
                    violation: TrackShapeViolation::NonFiniteValue,
                }),
                "track quaternion component {component}"
            );
        }

        for key in 0..3 {
            let mut times = vec![0.0, 1.0, 2.0];
            times[key] = f32::NAN;
            let document = track_document(Track {
                times,
                values: TrackValues::Vec3s(vec![Vec3::ZERO; 3]),
                ..valid_track()
            });
            assert_eq!(
                validate_document_shape(&document),
                Err(DocumentShapeError::TrackShape {
                    clip_index: 0,
                    node: 0,
                    violation: TrackShapeViolation::NonFiniteTime,
                }),
                "track time {key}"
            );
        }

        for component in 0..16 {
            let mut columns = Mat4::IDENTITY.to_cols_array();
            columns[component] = f32::NAN;
            let inverse_bind = Mat4::from_cols_array(&columns);

            let mut instance_document = instance_document();
            instance_document.assets.instances[0].skin_joints = vec![0];
            instance_document.assets.instances[0].skin_ibms = vec![inverse_bind];
            assert_eq!(
                validate_document_shape(&instance_document),
                Err(DocumentShapeError::MeshInstanceShape {
                    instance_index: 0,
                    violation: MeshInstanceShapeViolation::NonFiniteSkinInverseBind,
                }),
                "instance inverse-bind component {component}"
            );

            let mut bone_document = one_bone_document();
            bone_document.skeleton.bones[0].inverse_bind = Some(inverse_bind);
            assert_eq!(
                validate_document_shape(&bone_document),
                Err(DocumentShapeError::NonFiniteBoneInverseBind { node: 0 }),
                "bone inverse-bind component {component}"
            );
        }
    }

    #[test]
    fn document_shape_rejects_duplicate_tracks_for_every_property() {
        let tracks = [
            (Property::Translation, TrackValues::Vec3s(vec![Vec3::ZERO])),
            (Property::Scale, TrackValues::Vec3s(vec![Vec3::ONE])),
            (Property::Rotation, TrackValues::Quats(vec![Quat::IDENTITY])),
        ];

        for (property, values) in tracks {
            let track = Track {
                property,
                values,
                ..valid_track()
            };
            let mut document = track_document(track.clone());
            document.clips[0].tracks.push(track);

            assert_eq!(
                validate_document_shape(&document),
                Err(DocumentShapeError::DuplicateClipTrack {
                    clip_index: 0,
                    node: 0,
                    property,
                }),
                "duplicate {property:?} track"
            );
        }
    }

    #[test]
    fn document_shape_rejects_infinite_times_quaternions_and_inverse_binds() {
        for non_finite in [f32::INFINITY, f32::NEG_INFINITY] {
            let document = track_document(Track {
                times: vec![non_finite],
                values: TrackValues::Vec3s(vec![Vec3::ZERO]),
                ..valid_track()
            });
            assert_eq!(
                validate_document_shape(&document),
                Err(DocumentShapeError::TrackShape {
                    clip_index: 0,
                    node: 0,
                    violation: TrackShapeViolation::NonFiniteTime,
                }),
                "track time {non_finite}"
            );

            let document = track_document(Track {
                property: Property::Rotation,
                values: TrackValues::Quats(vec![Quat::from_xyzw(non_finite, 0.0, 0.0, 1.0)]),
                ..valid_track()
            });
            assert_eq!(
                validate_document_shape(&document),
                Err(DocumentShapeError::TrackShape {
                    clip_index: 0,
                    node: 0,
                    violation: TrackShapeViolation::NonFiniteValue,
                }),
                "track quaternion {non_finite}"
            );

            let mut columns = Mat4::IDENTITY.to_cols_array();
            columns[0] = non_finite;
            let inverse_bind = Mat4::from_cols_array(&columns);
            let mut document = instance_document();
            document.assets.instances[0].skin_joints = vec![0];
            document.assets.instances[0].skin_ibms = vec![inverse_bind];
            assert_eq!(
                validate_document_shape(&document),
                Err(DocumentShapeError::MeshInstanceShape {
                    instance_index: 0,
                    violation: MeshInstanceShapeViolation::NonFiniteSkinInverseBind,
                }),
                "instance inverse bind {non_finite}"
            );

            let mut document = one_bone_document();
            document.skeleton.bones[0].inverse_bind = Some(inverse_bind);
            assert_eq!(
                validate_document_shape(&document),
                Err(DocumentShapeError::NonFiniteBoneInverseBind { node: 0 }),
                "bone inverse bind {non_finite}"
            );
        }
    }

    #[test]
    fn document_shape_checks_mesh_and_joint_references_on_later_instances() {
        let later_instance = MeshInstance {
            node: 0,
            mesh: 0,
            ..MeshInstance::default()
        };

        let mut document = instance_document();
        document.assets.instances.push(later_instance.clone());
        document.assets.instances[1].mesh = 1;
        assert_eq!(
            validate_document_shape(&document),
            Err(DocumentShapeError::MeshInstanceShape {
                instance_index: 1,
                violation: MeshInstanceShapeViolation::MeshIndexOutOfRange,
            })
        );

        let mut document = instance_document();
        document.assets.instances.push(later_instance);
        document.assets.instances[1].skin_joints = vec![1];
        assert_eq!(
            validate_document_shape(&document),
            Err(DocumentShapeError::MeshInstanceShape {
                instance_index: 1,
                violation: MeshInstanceShapeViolation::SkinJointOutOfRange,
            })
        );
    }

    #[test]
    fn document_shape_finds_duplicates_that_do_not_involve_the_first_item() {
        let mut document = Document::default();
        document.assets.source_skeleton.skins = [4, 5, 5]
            .into_iter()
            .map(|source_skin_index| SourceSkinAsset {
                source_skin_index,
                ..SourceSkinAsset::default()
            })
            .collect();
        assert_eq!(
            validate_document_shape(&document),
            Err(DocumentShapeError::DuplicateSourceSkinIndex {
                source_skin_index: 5,
            })
        );

        let scale_track = Track {
            property: Property::Scale,
            values: TrackValues::Vec3s(vec![Vec3::ONE]),
            ..valid_track()
        };
        let mut document = track_document(valid_track());
        document.clips[0].tracks.push(scale_track.clone());
        document.clips[0].tracks.push(scale_track);
        assert_eq!(
            validate_document_shape(&document),
            Err(DocumentShapeError::DuplicateClipTrack {
                clip_index: 0,
                node: 0,
                property: Property::Scale,
            })
        );
    }

    #[test]
    fn document_shape_checks_later_tracks_and_inverse_binds() {
        let mut document = track_document(valid_track());
        document.clips[0].tracks.push(Track {
            property: Property::Scale,
            times: Vec::new(),
            values: TrackValues::Vec3s(Vec::new()),
            ..valid_track()
        });
        assert_eq!(
            validate_document_shape(&document),
            Err(DocumentShapeError::TrackShape {
                clip_index: 0,
                node: 0,
                violation: TrackShapeViolation::EmptyTimes,
            })
        );

        let scale_track = Track {
            property: Property::Scale,
            values: TrackValues::Vec3s(vec![Vec3::ONE]),
            ..valid_track()
        };
        let mut document = track_document(valid_track());
        document.clips.push(Clip {
            name: "later".into(),
            duration_s: 0.0,
            tracks: vec![scale_track.clone(), scale_track],
        });
        assert_eq!(
            validate_document_shape(&document),
            Err(DocumentShapeError::DuplicateClipTrack {
                clip_index: 1,
                node: 0,
                property: Property::Scale,
            })
        );

        let mut document = track_document(valid_track());
        document.clips.push(Clip {
            name: "later".into(),
            duration_s: 0.0,
            tracks: vec![Track {
                property: Property::Scale,
                times: Vec::new(),
                values: TrackValues::Vec3s(Vec::new()),
                ..valid_track()
            }],
        });
        assert_eq!(
            validate_document_shape(&document),
            Err(DocumentShapeError::TrackShape {
                clip_index: 1,
                node: 0,
                violation: TrackShapeViolation::EmptyTimes,
            })
        );

        let mut columns = Mat4::IDENTITY.to_cols_array();
        columns[15] = f32::NAN;
        let non_finite_inverse_bind = Mat4::from_cols_array(&columns);

        let mut document = instance_document();
        document.skeleton.bones.push(bone(Some(0)));
        document.assets.instances[0].skin_joints = vec![0, 1];
        document.assets.instances[0].skin_ibms = vec![Mat4::IDENTITY, non_finite_inverse_bind];
        assert_eq!(
            validate_document_shape(&document),
            Err(DocumentShapeError::MeshInstanceShape {
                instance_index: 0,
                violation: MeshInstanceShapeViolation::NonFiniteSkinInverseBind,
            })
        );

        let mut document = instance_document();
        document.assets.instances.push(MeshInstance {
            node: 0,
            mesh: 0,
            skin_joints: vec![0],
            skin_ibms: vec![non_finite_inverse_bind],
            ..MeshInstance::default()
        });
        assert_eq!(
            validate_document_shape(&document),
            Err(DocumentShapeError::MeshInstanceShape {
                instance_index: 1,
                violation: MeshInstanceShapeViolation::NonFiniteSkinInverseBind,
            })
        );

        let mut document = instance_document();
        document.assets.instances.push(MeshInstance {
            node: 0,
            mesh: 0,
            skin_joints: vec![0],
            skin_ibms: vec![Mat4::IDENTITY, Mat4::IDENTITY],
            ..MeshInstance::default()
        });
        assert_eq!(
            validate_document_shape(&document),
            Err(DocumentShapeError::MeshInstanceShape {
                instance_index: 1,
                violation: MeshInstanceShapeViolation::SkinInverseBindCountMismatch,
            })
        );

        let mut document = one_bone_document();
        document.skeleton.bones.push(Bone {
            inverse_bind: Some(non_finite_inverse_bind),
            ..bone(Some(0))
        });
        assert_eq!(
            validate_document_shape(&document),
            Err(DocumentShapeError::NonFiniteBoneInverseBind { node: 1 })
        );
    }

    #[test]
    fn document_shape_violation_names_remain_machine_stable() {
        let source_projection = [
            (
                SourceProjectionViolation::ProjectedBoneOutOfRange,
                "projected_bone_out_of_range",
            ),
            (
                SourceProjectionViolation::TwoSourceNodesProjectToOneBone,
                "two_source_nodes_project_to_one_bone",
            ),
            (
                SourceProjectionViolation::ParentSourceNodeMissing,
                "parent_source_node_is_missing",
            ),
            (
                SourceProjectionViolation::CyclicUnprojectedSourceParentChain,
                "cyclic_unprojected_source_parent_chain",
            ),
            (
                SourceProjectionViolation::NearestProjectedParentMismatch,
                "projection_and_skeleton_parents_differ",
            ),
            (
                SourceProjectionViolation::ProjectedBoneHasUnprojectedSkeletonChild,
                "projected_bone_has_an_unprojected_skeleton_child",
            ),
        ];
        for (violation, expected) in source_projection {
            assert_eq!(violation.to_string(), expected);
        }

        let track = [
            (
                TrackShapeViolation::BoneIndexOutOfRange,
                "bone_index_out_of_range",
            ),
            (TrackShapeViolation::EmptyTimes, "empty_times"),
            (TrackShapeViolation::NonFiniteTime, "non_finite_time"),
            (
                TrackShapeViolation::TimesNotStrictlyIncreasing,
                "times_not_strictly_increasing",
            ),
            (
                TrackShapeViolation::ValueCountMismatch,
                "value_count_mismatch",
            ),
            (
                TrackShapeViolation::ValueTypeMismatchesProperty,
                "value_type_mismatches_property",
            ),
            (TrackShapeViolation::NonFiniteValue, "non_finite_value"),
        ];
        for (violation, expected) in track {
            assert_eq!(violation.to_string(), expected);
        }

        let instance = [
            (
                MeshInstanceShapeViolation::NodeIndexOutOfRange,
                "node_index_out_of_range",
            ),
            (
                MeshInstanceShapeViolation::MeshIndexOutOfRange,
                "mesh_index_out_of_range",
            ),
            (
                MeshInstanceShapeViolation::SkinJointOutOfRange,
                "skin_joint_out_of_range",
            ),
            (
                MeshInstanceShapeViolation::SkinInverseBindCountMismatch,
                "skin_ibm_count_mismatch",
            ),
            (
                MeshInstanceShapeViolation::NonFiniteSkinInverseBind,
                "non_finite_inverse_bind",
            ),
        ];
        for (violation, expected) in instance {
            assert_eq!(violation.to_string(), expected);
        }
    }

    #[test]
    fn tolerant_world_rests_keep_unrelated_partial_evidence() {
        let skeleton = Skeleton {
            bones: vec![
                bone(None),
                bone(Some(99)),
                Bone {
                    rest: Transform {
                        translation: Vec3::X,
                        ..Transform::IDENTITY
                    },
                    ..bone(None)
                },
                Bone {
                    rest: Transform {
                        translation: Vec3::Y,
                        ..Transform::IDENTITY
                    },
                    ..bone(Some(2))
                },
                bone(Some(1)),
            ],
        };

        let worlds = tolerant_world_rest_matrices(&skeleton);
        assert_eq!(worlds.len(), 5);
        assert_eq!(worlds[0], Some(Mat4::IDENTITY));
        assert_eq!(worlds[1], None, "the malformed parent is unavailable");
        assert_eq!(worlds[2], Some(Mat4::from_translation(Vec3::X)));
        assert_eq!(
            worlds[3],
            Some(Mat4::from_translation(Vec3::new(1.0, 1.0, 0.0))),
            "a finite independent chain remains measurable"
        );
        assert_eq!(
            worlds[4], None,
            "a child of unavailable evidence is unavailable"
        );
    }

    #[test]
    fn shared_affine_classifier_respects_distinct_caller_tolerances() {
        let equal_axis_basis = affine_test_fixtures::tolerance_divergence_basis();
        let strict = PositiveUniformAffineTolerance {
            equal_axis: 1.0e-5,
            relative_orthogonality: 1.0e-5,
            singular_determinant_relative: 1.0e-6,
        };
        let loose = PositiveUniformAffineTolerance {
            equal_axis: 1.0e-4,
            relative_orthogonality: 1.0e-4,
            singular_determinant_relative: 0.0,
        };

        assert_eq!(
            classify_positive_uniform_affine(equal_axis_basis, strict),
            Err(AffineDomainViolation::NonUniformScale),
            "the stricter caller rejects this equal-axis difference"
        );
        assert!(
            classify_positive_uniform_affine(equal_axis_basis, loose).is_ok(),
            "the looser caller accepts this equal-axis difference"
        );

        let orthogonality_basis = affine_test_fixtures::orthogonality_tolerance_divergence_basis();
        assert_eq!(
            classify_positive_uniform_affine(orthogonality_basis, strict),
            Err(AffineDomainViolation::Sheared),
            "the stricter caller rejects this cross-axis dot product"
        );
        assert!(
            classify_positive_uniform_affine(orthogonality_basis, loose).is_ok(),
            "the looser caller accepts this cross-axis dot product"
        );
    }

    #[test]
    fn shared_affine_classifier_pins_its_symmetric_f64_formula() {
        let policy = PositiveUniformAffineTolerance {
            equal_axis: 1.0e-5,
            relative_orthogonality: 1.0e-5,
            singular_determinant_relative: 1.0e-6,
        };

        // Exact binary32 lengths whose mean is exactly 99_999 in binary64.
        // The longest-axis deviation is exactly 1: accepted only when the
        // relative base is max(mean, axis), then refused one binary32 ulp
        // farther out.
        let on_long_edge = Mat3::from_diagonal(Vec3::new(99_998.5, 99_998.5, 100_000.0));
        assert_eq!(
            classify_positive_uniform_affine(on_long_edge, policy),
            Ok(99_999.0)
        );
        let short = 99_998.5;
        let long = 100_000.0 + 0.007_812_5;
        for diagonal in [
            Vec3::new(long, short, short),
            Vec3::new(short, long, short),
            Vec3::new(short, short, long),
        ] {
            assert_eq!(
                classify_positive_uniform_affine(Mat3::from_diagonal(diagonal), policy),
                Err(AffineDomainViolation::NonUniformScale)
            );
        }

        // A one-sided comparison would miss the uniquely short axis. At this
        // exact binary32 step the short-axis deviation is outside the band,
        // while each longer axis remains inside it.
        let short = 1.0 - 2.0_f32.powi(-16);
        for diagonal in [
            Vec3::new(short, 1.0, 1.0),
            Vec3::new(1.0, short, 1.0),
            Vec3::new(1.0, 1.0, short),
        ] {
            assert_eq!(
                classify_positive_uniform_affine(Mat3::from_diagonal(diagonal), policy),
                Err(AffineDomainViolation::NonUniformScale)
            );
        }

        // Only binary64 dot-product arithmetic places this basis outside the
        // orthogonality band; binary32 rounds the deciding dot back inside.
        let c0 = Vec3::new(0.12792248, -0.99066633, -0.047073245);
        let c1 = Vec3::new(-0.34637994, -0.00016034879, -0.93809813);
        let c2 = Vec3::new(0.92933476, 0.13630849, -0.3431568);
        assert!((c1.dot(c2) as f64).abs() < 1.0e-5);
        assert!(c1.as_dvec3().dot(c2.as_dvec3()).abs() > 1.0e-5);
        assert_eq!(
            classify_positive_uniform_affine(Mat3::from_cols(c0, c1, c2), policy),
            Err(AffineDomainViolation::Sheared)
        );

        // Orthogonality is sign-independent; dropping abs() accepts the
        // negative case while leaving the positive fixture green.
        for shear in [2.0_f32.powi(-15), -2.0_f32.powi(-15)] {
            let basis = Mat3::from_cols(Vec3::X, Vec3::new(shear, 1.0, 0.0), Vec3::Z);
            assert_eq!(
                classify_positive_uniform_affine(basis, policy),
                Err(AffineDomainViolation::Sheared)
            );
        }
    }

    #[test]
    fn affine_axis_mean_is_ascending_and_column_order_invariant() {
        // This is the audited counterexample. These are the widened lengths
        // of three exact binary32 columns; their authored-order sum changes
        // by one binary64 ulp when the columns are cycled. The canonical
        // ascending association is the lower result.
        let lengths = [
            f64::from_bits(0x3ff1_09e7_e000_022c),
            f64::from_bits(0x3ff1_09ec_6000_0eb5),
            f64::from_bits(0x3ff1_09fa_e000_3cde),
        ];
        let expected = f64::from_bits(0x3ff1_09ef_b555_6f3f);
        let ascending = (lengths[0] + lengths[1] + lengths[2]) / 3.0;
        let descending = (lengths[2] + lengths[1] + lengths[0]) / 3.0;
        assert_eq!(expected.to_bits(), 0x3ff1_09ef_b555_6f3f);
        assert_eq!(ascending.to_bits(), expected.to_bits());
        assert_eq!(descending.to_bits(), 0x3ff1_09ef_b555_6f40);
        for order in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            assert_eq!(
                average_affine_axis_length(order.map(|index| lengths[index])),
                expected,
                "axis order {order:?}"
            );
        }

        // The association is observable even for exactly representable
        // dyadic inputs: adding the two small terms first retains them,
        // while adding either to `2^53` loses both. This catches replacing
        // the canonical sort with a different fixed input order.
        let dyadic = [2.0_f64.powi(53), 1.0, 1.0];
        let ascending = (dyadic[1] + dyadic[2] + dyadic[0]) / 3.0;
        let descending = (dyadic[0] + dyadic[1] + dyadic[2]) / 3.0;
        assert_ne!(ascending, descending);
        assert_eq!(average_affine_axis_length(dyadic), ascending);

        // The same exact binary32 columns exercise the classifier. Every
        // permutation below preserves orientation: odd column permutations
        // negate one column, which leaves lengths unchanged and restores the
        // determinant's sign. The equal-axis boundary must consequently
        // reject the same geometry in all six authored axis orders.
        let permutations = affine_test_fixtures::appendix_d_v6_mean_permutations();
        let expected_length_bits = lengths.map(f64::to_bits);
        assert_eq!(
            affine_axis_lengths(permutations[0]).map(f64::to_bits),
            expected_length_bits
        );
        let tolerance = PositiveUniformAffineTolerance {
            equal_axis: 1.0e-5,
            relative_orthogonality: 1.0e-5,
            singular_determinant_relative: 1.0e-6,
        };
        for (permutation, linear) in permutations.into_iter().enumerate() {
            assert!(
                linear
                    .x_axis
                    .as_dvec3()
                    .cross(linear.y_axis.as_dvec3())
                    .dot(linear.z_axis.as_dvec3())
                    > 0.0,
                "orientation for permutation {permutation}"
            );
            assert_eq!(
                average_affine_axis_length(affine_axis_lengths(linear)).to_bits(),
                expected.to_bits(),
                "mean for permutation {permutation}"
            );
            assert_eq!(
                classify_positive_uniform_affine(linear, tolerance),
                Err(AffineDomainViolation::NonUniformScale),
                "classification for permutation {permutation}"
            );
        }
    }

    #[test]
    fn shared_affine_classifier_pins_f64_determinant_arithmetic() {
        // These exact binary32 columns make the f32 scalar triple product
        // land above the same threshold that the product of widened columns
        // lands below. The remaining bands are deliberately loose so only
        // singularity arithmetic decides the result.
        let linear = Mat3::from_cols(
            Vec3::new(
                f32::from_bits(0x3ff3_5574),
                f32::from_bits(0x3f0e_fa3c),
                0.0,
            ),
            Vec3::new(
                f32::from_bits(0x3ff5_5e17),
                f32::from_bits(0x3f10_2c31),
                0.0,
            ),
            Vec3::Z,
        );
        let columns = [
            linear.x_axis.as_dvec3(),
            linear.y_axis.as_dvec3(),
            linear.z_axis.as_dvec3(),
        ];
        let determinant_f64 = columns[2].dot(columns[0].cross(columns[1]));
        let determinant_f32 = f64::from(linear.determinant());
        let lengths = affine_axis_lengths(linear);
        let threshold = (determinant_f64 + determinant_f32) / 2.0;
        assert!(determinant_f64 < threshold);
        assert!(determinant_f32 > threshold);

        assert_eq!(
            classify_positive_uniform_affine(
                linear,
                PositiveUniformAffineTolerance {
                    equal_axis: 10.0,
                    relative_orthogonality: 10.0,
                    singular_determinant_relative: threshold
                        / (lengths[0] * lengths[1] * lengths[2]),
                },
            ),
            Err(AffineDomainViolation::Singular)
        );

        // Every derived determinant operand must stay widened as well. A
        // binary32 axis-product intermediate overflows on this otherwise
        // finite, positive, exactly uniform basis and falsely calls it
        // singular under Appendix D's non-zero relative threshold.
        let large_uniform = 2.0e19_f32;
        assert_eq!(
            classify_positive_uniform_affine(
                Mat3::from_diagonal(Vec3::splat(large_uniform)),
                PositiveUniformAffineTolerance {
                    equal_axis: 1.0e-5,
                    relative_orthogonality: 1.0e-5,
                    singular_determinant_relative: 1.0e-6,
                },
            ),
            Ok(f64::from(large_uniform))
        );
    }

    #[test]
    fn weld_preserves_uv_seams_at_shared_positions() {
        let mut primitive = Primitive {
            positions: vec![Vec3::ZERO, Vec3::ZERO, Vec3::ZERO],
            uvs: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 0.0]],
            ..Primitive::default()
        };

        primitive.weld();

        assert_eq!(primitive.positions.len(), 2);
        let reconstructed_corners = primitive
            .indices
            .iter()
            .map(|&index| {
                let index = index as usize;
                (primitive.positions[index], primitive.uvs[index])
            })
            .collect::<Vec<_>>();
        assert_eq!(
            reconstructed_corners,
            vec![
                (Vec3::ZERO, [0.0, 0.0]),
                (Vec3::ZERO, [1.0, 0.0]),
                (Vec3::ZERO, [0.0, 0.0]),
            ]
        );
    }
}
