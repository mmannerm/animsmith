//! Measurements: the raw per-clip metric map that `measure` emits and
//! `lint` judges. Kept separate from findings so pipelines (e.g. a
//! bake's measured sidecar) can pin their own contracts to the numbers.

use crate::checks::exceeds_f32_cap;
use crate::checks::fps::GRID_TOLERANCE_FRAMES;
use crate::checks::loop_closure::effective_caps;
use crate::config::Config;
use crate::metrics::{
    MetricGrids, foot_cycle_metrics, loop_continuity_metrics, root_motion_speed_mps,
    rotation_range_deg,
};
use crate::model::{
    AffineGeometryFacts, DecodedImageColorType, Document, ImageContainerFormat, ImageSourceKind,
    ImageUnavailableReason, MaterialResourceCoverage, MaterialTextureSlot, MeshAsset,
    SourceImageInspection, SourceInverseBindAccessorStatus, SourceNodeLocalRest,
    SourceSkeletonCoverage, tolerant_world_rest_matrices, values_equal_to_mean,
};
use crate::profile::ResolvedRoles;
use crate::sample::PoseGrid;
use crate::transform::analyze_duplicate_loop_endpoint;
use glam::{Mat3, Mat4, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Rotation ranges below this are not recorded (matches the incubating
/// pipeline's convention).
pub const MIN_RECORDED_ROTATION_DEG: f64 = 0.1;

/// Relative tolerance used for orthogonality and equal-axis classification.
pub const LINEAR_CLASSIFICATION_RELATIVE_TOLERANCE: f64 = 1.0e-5;
/// Scale-relative determinant threshold used to classify singular matrices.
pub const LINEAR_CLASSIFICATION_SINGULAR_TOLERANCE: f64 = 1.0e-6;

/// Axis-aligned bounding box whose coordinates use the owning measurement's
/// documented definition-local, node-world, or scene-world domain.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Aabb {
    /// Minimum XYZ corner.
    pub min: [f32; 3],
    /// Maximum XYZ corner.
    pub max: [f32; 3],
}

/// Static base-geometry measurements of one source mesh definition.
///
/// Vertex data is measured from loader-decoded base geometry: indexed meshes
/// count each stored position once, while unindexed meshes count every stored
/// triangle corner. The geometry AABB and centroid are in the definition's
/// primitive coordinate system and exclude node transforms, morph targets,
/// skinning, animation, and runtime placement.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MeshDefinitionMeasurements {
    /// Stable index of the mesh definition in the source format.
    pub mesh_index: usize,
    /// Mesh name.
    pub name: String,
    /// Total position count across the mesh's primitives.
    pub vertex_count: u32,
    /// Bounding box over every finite base `POSITION`; `None` when none exist.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry_aabb: Option<Aabb>,
    /// Arithmetic mean of every finite base `POSITION`; `None` when none
    /// exist. Like [`Self::geometry_aabb`], this is in mesh-definition local
    /// coordinates and excludes node transforms, morph targets, skinning,
    /// animation, and runtime placement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry_centroid: Option<[f32; 3]>,
    /// Highest number of non-zero skin influences on any single vertex
    /// (`0` for an unskinned mesh).
    pub max_joints_per_vertex: u32,
    /// Min/max of the per-vertex skin-weight sums (≈1.0 for a
    /// well-formed skin); `None` for an unskinned mesh.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight_sum_min: Option<f64>,
    /// Maximum finite per-vertex skin-weight sum; `None` for an
    /// unskinned mesh.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight_sum_max: Option<f64>,
    /// Declared non-primary skin-influence sets, merged across every
    /// primitive in this mesh definition and sorted by set number.
    pub additional_influence_sets: Vec<AdditionalInfluenceSetMeasurements>,
}

/// Presence of one non-primary skin-influence attribute set in a mesh definition.
///
/// The two sides are reported independently so malformed or partial source
/// declarations remain observable without assigning them skinning semantics.
/// The mismatch flags preserve that evidence when independent primitive
/// declarations make the aggregate sides appear paired.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AdditionalInfluenceSetMeasurements {
    /// glTF attribute-set number (`n >= 1`).
    pub set_index: u32,
    /// Whether any primitive declares `JOINTS_n`.
    pub joints_present: bool,
    /// Whether any primitive declares `WEIGHTS_n`.
    pub weights_present: bool,
    /// Whether any primitive declares `JOINTS_n` without `WEIGHTS_n` on that
    /// same primitive.
    pub joints_without_weights_present: bool,
    /// Whether any primitive declares `WEIGHTS_n` without `JOINTS_n` on that
    /// same primitive.
    pub weights_without_joints_present: bool,
}

/// Why a static node-instance AABB could not be emitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum StaticNodeAabbUnavailableReason {
    /// The referenced definition has no finite base positions.
    NoFinitePositions,
    /// The instance is skinned, whose node transform is not a static rendered
    /// bound under glTF semantics; bind-pose skinning is a separate domain.
    SkinnedDeformationExcluded,
    /// The default/rest world transform or a transformed point was non-finite.
    NonFiniteTransform,
}

/// Static base-geometry bounds for one mesh-bearing source node.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct NodeInstanceMeasurements {
    /// Stable index of the mesh-bearing node in the source format.
    pub node_index: usize,
    /// Node name for display; identity comes from [`Self::node_index`].
    pub node_name: String,
    /// Stable source mesh-definition index referenced by this node.
    pub mesh_index: usize,
    /// Tight AABB after applying the node's default/rest world transform to
    /// every finite base position. Deformation and runtime placement are
    /// excluded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub static_node_world_aabb: Option<Aabb>,
    /// Present exactly when [`Self::static_node_world_aabb`] is unavailable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub static_node_world_aabb_unavailable_reason: Option<StaticNodeAabbUnavailableReason>,
}

/// Static aggregate bounds for one declared source scene.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SceneMeasurements {
    /// Stable index of the scene in the source format.
    pub scene_index: usize,
    /// Authored scene name, when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Number of mesh-bearing node instances reachable from the scene roots.
    pub instance_count: usize,
    /// Union of every available static node-instance AABB in this scene.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub static_scene_world_aabb: Option<Aabb>,
    /// Reachable instances excluded because their static node AABB was
    /// unavailable. A non-zero value means the scene aggregate is partial.
    pub excluded_instance_count: usize,
}

/// One material-to-texture binding in the source resource table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MaterialTextureBindingMeasurements {
    /// Stable material slot.
    pub slot: MaterialTextureSlot,
    /// Stable source texture index.
    pub texture_index: usize,
}

/// One material definition from the source resource table.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MaterialDefinitionMeasurements {
    /// Stable source material index.
    pub material_index: usize,
    /// Authored name, when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Texture bindings in fixed slot order.
    pub texture_bindings: Vec<MaterialTextureBindingMeasurements>,
}

/// One texture definition from the source resource table.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TextureMeasurements {
    /// Stable source texture index.
    pub texture_index: usize,
    /// Authored name, when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Stable source image index referenced by this texture.
    pub image_index: usize,
}

/// Flat source-image measurement. Available image metadata and an unavailable
/// reason are mutually exclusive by contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ImageMeasurements {
    /// Stable source image index.
    pub image_index: usize,
    /// Authored name, when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Source declaration kind.
    pub source_kind: ImageSourceKind,
    /// MIME type declared by the source, when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared_mime_type: Option<String>,
    /// Container recognized during inspection, when any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detected_container: Option<ImageContainerFormat>,
    /// Pixel width when decoded metadata is available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    /// Pixel height when decoded metadata is available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    /// Decoded channel count when metadata is available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_count: Option<u8>,
    /// Decoded color representation when metadata is available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decoded_color_type: Option<DecodedImageColorType>,
    /// Why decoded metadata could not be provided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unavailable_reason: Option<ImageUnavailableReason>,
}

/// The source-preservation coverage for skeleton and skin measurements.
///
/// A source loader that cannot retain stable node and skin identities reports
/// [`SourceSkeletonCoverage::Unavailable`] with empty node and skin arrays,
/// rather than treating normalized skeleton ordinals as source indices.
pub type SkeletonSourceCoverage = SourceSkeletonCoverage;

/// One finite authored node-local rest representation.
///
/// The tagged representation preserves a source matrix as a matrix instead of
/// presenting a decomposition as authored TRS evidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum SkeletonNodeLocalRestMeasurements {
    /// Authored local translation, rotation, and scale.
    Trs {
        /// Translation in metres expressed in the direct parent's coordinate
        /// frame. It is not directly comparable with scene- or mesh-space
        /// measurements until ancestor transforms are composed.
        translation_parent_space_m: [f32; 3],
        /// Quaternion components in XYZW order.
        rotation_xyzw: [f32; 4],
        /// Non-uniform local scale.
        scale: [f32; 3],
    },
    /// Authored 4×4 local matrix in column-major order.
    Matrix {
        /// Column-major matrix components.
        matrix: [f32; 16],
    },
    /// The authored transform could not be represented in JSON safely.
    Unavailable {
        /// Stable reason for the unavailable local transform.
        reason: SkeletonNodeLocalRestUnavailableReason,
    },
}

/// Stable classification of the linear 3x3 part of an affine transform.
///
/// Reflection is classified before shear or scale shape; the axis lengths and
/// orientation retain the remaining facts for reflected transforms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum LinearTransformClassification {
    /// Orthogonal unit-length axes with positive orientation.
    UnitOrthonormal,
    /// Orthogonal equal-length axes with a non-unit factor.
    UniformScaled,
    /// Orthogonal axes with unequal lengths.
    NonUniform,
    /// At least two axes are not orthogonal.
    Sheared,
    /// The determinant is negative; axis and uniform-factor facts remain
    /// available to describe the reflected transform further.
    Reflected,
    /// The linear transform is singular or near-singular relative to its axis
    /// lengths.
    Singular,
    /// At least one matrix component, derived axis length, or determinant is
    /// non-finite.
    NonFinite,
}

/// Orientation sign of a finite linear transform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum LinearTransformOrientation {
    /// Positive determinant.
    Positive,
    /// Negative determinant (a reflection).
    Negative,
    /// Zero or near-zero relative determinant.
    Zero,
}

/// Deterministic facts derived from the linear 3x3 part of an affine matrix.
///
/// Numeric fields are present for every finite observation and absent only for
/// [`LinearTransformClassification::NonFinite`]. `uniform_scale` is present
/// when the columns are mutually orthogonal and have one common length,
/// including reflected uniform transforms. Derived numeric fields use `f64`
/// so determinant products remain representable across finite `f32` matrices.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct LinearTransformMeasurements {
    /// Stable transform class.
    pub classification: LinearTransformClassification,
    /// Lengths of the X, Y, and Z matrix columns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub axis_lengths: Option<[f64; 3]>,
    /// Determinant of the linear 3x3 matrix.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub determinant: Option<f64>,
    /// Determinant sign, using the same relative singularity tolerance as the
    /// classification.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orientation: Option<LinearTransformOrientation>,
    /// Common orthogonal axis length when one is well-defined.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uniform_scale: Option<f64>,
}

/// Why a node-local rest representation is unavailable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SkeletonNodeLocalRestUnavailableReason {
    /// The source transform has a non-finite component.
    NonFiniteTransform,
}

/// Why a node's accumulated rest-world matrix is unavailable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SkeletonRestWorldMatrixUnavailableReason {
    /// The node-local rest transform is unavailable.
    NonFiniteLocalRest,
    /// The parent rest-world matrix is unavailable.
    ParentRestWorldUnavailable,
    /// Matrix composition produced a non-finite result.
    NonFiniteWorldMatrix,
}

/// One source node in stable source-node order.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SkeletonNodeMeasurements {
    /// Stable source node-array index.
    pub node_index: usize,
    /// Authored node name, when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Direct source parent, when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_node_index: Option<usize>,
    /// Source scenes that explicitly name this node as a scene root, in
    /// ascending source-scene order. Scene membership does not alter the
    /// node's local or accumulated rest transform.
    pub scene_root_indices: Vec<usize>,
    /// Authored node-local rest representation.
    pub local_rest: SkeletonNodeLocalRestMeasurements,
    /// Accumulated rest transform from this source root to this node, in
    /// column-major order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rest_world_matrix: Option<[f32; 16]>,
    /// Translation column of [`Self::rest_world_matrix`], in metres in the
    /// accumulated source rest-world coordinate domain.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rest_world_translation_m: Option<[f32; 3]>,
    /// Linear-transform facts for the accumulated rest-world matrix. A
    /// non-finite classification accompanies an unavailable matrix.
    pub rest_world_linear: LinearTransformMeasurements,
    /// Present exactly when [`Self::rest_world_matrix`] is unavailable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rest_world_matrix_unavailable_reason: Option<SkeletonRestWorldMatrixUnavailableReason>,
}

/// Source-level read state for one inverse-bind accessor.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SkinInverseBindAccessorMeasurements {
    /// Whether the source accessor was absent, readable, empty, short, or
    /// unreadable.
    pub status: SourceInverseBindAccessorStatus,
    /// Declared source accessor count, when an accessor was declared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared_count: Option<usize>,
    /// Every readable finite raw matrix in accessor order, including entries
    /// beyond the skin's joint count.
    pub matrices: Vec<[f32; 16]>,
}

/// One joint slot in a source skin.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SkinJointMeasurements {
    /// Zero-based source skin joint slot.
    pub joint_index: usize,
    /// Stable source node index for this joint.
    pub node_index: usize,
    /// Inverse of the usable raw IBM: joint bind space to mesh bind space.
    pub joint_bind_to_mesh: SkinDerivedMatrixMeasurements,
    /// `joint_rest_world * raw_inverse_bind` for this joint slot. This is a
    /// per-joint observation of mesh bind world, not a policy judgment that
    /// all slots must agree.
    pub mesh_bind_world: SkinDerivedMatrixMeasurements,
}

/// One node that attaches a source skin.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SkinAttachmentMeasurements {
    /// Stable source node index of the attachment.
    pub node_index: usize,
    /// Stable source mesh-definition index when declared on the node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mesh_index: Option<usize>,
}

/// Why one derived per-joint bind-domain matrix is unavailable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SkinDerivedMatrixUnavailableReason {
    /// The skin did not declare an inverse-bind accessor.
    InverseBindAccessorAbsent,
    /// The skin declared a count-zero inverse-bind accessor.
    InverseBindAccessorEmpty,
    /// The readable accessor has fewer matrices than declared joint slots.
    InverseBindAccessorCountMismatch,
    /// The declared accessor could not be read safely, including non-finite
    /// raw matrix data that JSON cannot represent.
    InverseBindAccessorUnreadable,
    /// The joint's accumulated rest-world matrix is unavailable.
    JointRestWorldUnavailable,
    /// The raw inverse-bind matrix cannot itself be inverted.
    InverseBindMatrixNonInvertible,
    /// Multiplication produced a non-finite derived matrix.
    NonFiniteDerivedMatrix,
}

/// One per-joint derived bind-domain matrix.
///
/// A matrix and its unavailable reason are mutually exclusive.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SkinDerivedMatrixMeasurements {
    /// Matrix in the documented coordinate domain, in column-major order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matrix: Option<[f32; 16]>,
    /// Linear-transform facts derived from [`Self::matrix`]. Present exactly
    /// when the matrix is available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linear: Option<LinearTransformMeasurements>,
    /// Present exactly when [`Self::matrix`] is unavailable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unavailable_reason: Option<SkinDerivedMatrixUnavailableReason>,
}

/// Stable aggregate class for one skin's joint-bind-to-mesh linear facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SkinBindLinearSummaryClassification {
    /// The skin declares no joint slots.
    NoJoints,
    /// No joint-bind-to-mesh matrix is available.
    Unavailable,
    /// Some, but not all, joint-bind-to-mesh matrices are available.
    PartiallyUnavailable,
    /// Every joint is an unreflected orthogonal uniform transform with the
    /// same factor.
    ConsistentUniform,
    /// Every joint is unreflected, orthogonal, and uniform, but factors differ.
    MixedUniform,
    /// Every joint is non-uniform or sheared.
    NonUniformOrSheared,
    /// Every joint is reflected or singular.
    ReflectedOrSingular,
    /// Available joints span more than one of the preceding transform groups.
    Mixed,
}

/// Summary of one skin's joint-bind-to-mesh linear-transform evidence.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SkinBindLinearSummaryMeasurements {
    /// Stable aggregate class.
    pub classification: SkinBindLinearSummaryClassification,
    /// Number of declared skin joint slots.
    pub joint_count: usize,
    /// Number of slots with an available joint-bind-to-mesh matrix.
    pub available_joint_count: usize,
    /// Number of slots without an available joint-bind-to-mesh matrix.
    pub unavailable_joint_count: usize,
    /// Canonical arithmetic mean factor for
    /// [`SkinBindLinearSummaryClassification::ConsistentUniform`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consistent_uniform_scale: Option<f64>,
}

fn unavailable_linear_transform() -> LinearTransformMeasurements {
    LinearTransformMeasurements {
        classification: LinearTransformClassification::NonFinite,
        axis_lengths: None,
        determinant: None,
        orientation: None,
        uniform_scale: None,
    }
}

/// One source skin in stable source-skin order.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SkinMeasurements {
    /// Stable source skin-array index.
    pub skin_index: usize,
    /// Authored skin name, when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Explicitly declared source skeleton root, when any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skeleton_root_node_index: Option<usize>,
    /// Source joint slots in their declared order.
    pub joints: Vec<SkinJointMeasurements>,
    /// Aggregate of the joints' bind-to-mesh linear-transform facts.
    pub joint_bind_linear_summary: SkinBindLinearSummaryMeasurements,
    /// Exact source accessor state and finite readable raw matrices.
    pub inverse_bind_accessor: SkinInverseBindAccessorMeasurements,
    /// Source nodes that reference this skin, in source-node order.
    pub attachments: Vec<SkinAttachmentMeasurements>,
}

/// Static scene-asset evidence nested beside clip measurements.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AssetMeasurements {
    /// Whether material, texture, and image resource tables are complete.
    pub material_resource_coverage: MaterialResourceCoverage,
    /// Source material definitions in stable source order.
    pub material_definitions: Vec<MaterialDefinitionMeasurements>,
    /// Source texture definitions in stable source order.
    pub textures: Vec<TextureMeasurements>,
    /// Source image definitions in stable source order.
    pub images: Vec<ImageMeasurements>,
    /// Whether source skeleton identity facts are available.
    #[serde(default)]
    pub skeleton_source_coverage: SkeletonSourceCoverage,
    /// Source nodes in source-node order when skeleton coverage is complete.
    #[serde(default)]
    pub skeleton_nodes: Vec<SkeletonNodeMeasurements>,
    /// Source skins in source-skin order when skeleton coverage is complete.
    #[serde(default)]
    pub skins: Vec<SkinMeasurements>,
    /// Source mesh definitions, including definitions with no node instance.
    pub mesh_definitions: Vec<MeshDefinitionMeasurements>,
    /// Mesh-bearing source nodes, including nodes outside every scene.
    pub node_instances: Vec<NodeInstanceMeasurements>,
    /// Every declared source scene in source order.
    pub scenes: Vec<SceneMeasurements>,
    /// Stable source scene index selected as the default, when declared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_scene_index: Option<usize>,
}

#[derive(Debug, Clone, Copy)]
struct Bounds {
    min: [f32; 3],
    max: [f32; 3],
    any: bool,
}

impl Default for Bounds {
    fn default() -> Self {
        Self {
            min: [f32::INFINITY; 3],
            max: [f32::NEG_INFINITY; 3],
            any: false,
        }
    }
}

impl Bounds {
    fn include(&mut self, point: Vec3) -> bool {
        let point = point.to_array();
        if !point.iter().all(|value| value.is_finite()) {
            return false;
        }
        self.any = true;
        for ((min, max), value) in self.min.iter_mut().zip(&mut self.max).zip(point) {
            *min = min.min(value);
            *max = max.max(value);
        }
        true
    }

    fn include_aabb(&mut self, aabb: Aabb) {
        self.any = true;
        for ((min, max), (aabb_min, aabb_max)) in self
            .min
            .iter_mut()
            .zip(&mut self.max)
            .zip(aabb.min.into_iter().zip(aabb.max))
        {
            *min = min.min(aabb_min);
            *max = max.max(aabb_max);
        }
    }

    fn finish(self) -> Option<Aabb> {
        self.any.then_some(Aabb {
            min: self.min,
            max: self.max,
        })
    }
}

/// Arithmetic-mean reducer for finite mesh-definition positions.
///
/// Keep the sum in f64 so a large finite f32 mesh cannot overflow before its
/// final mean is narrowed back to the wire format's f32 coordinate type.
#[derive(Default)]
struct Centroid {
    sum: [f64; 3],
    count: u64,
}

impl Centroid {
    fn include(&mut self, point: Vec3) {
        let point = point.to_array();
        for (sum, value) in self.sum.iter_mut().zip(point) {
            *sum += f64::from(value);
        }
        self.count += 1;
    }

    fn finish(self) -> Option<[f32; 3]> {
        (self.count != 0).then(|| {
            let count = self.count as f64;
            self.sum.map(|sum| (sum / count) as f32)
        })
    }
}

fn measure_mesh_definition(mesh: &MeshAsset) -> MeshDefinitionMeasurements {
    let mut vertex_count = 0u32;
    let mut bounds = Bounds::default();
    let mut centroid = Centroid::default();
    let mut max_joints_per_vertex = 0u32;
    let mut weight_sum_min = f64::INFINITY;
    let mut weight_sum_max = f64::NEG_INFINITY;
    let mut any_finite_weight = false;
    let mut additional_influence_sets: BTreeMap<u32, AdditionalInfluenceSetMeasurements> =
        BTreeMap::new();

    for primitive in &mesh.primitives {
        vertex_count = vertex_count.saturating_add(primitive.positions.len() as u32);
        for &position in &primitive.positions {
            // Non-finite geometry remains visible to the `nan` check but must
            // never leak a JSON-invalid bound.
            if bounds.include(position) {
                centroid.include(position);
            }
        }
        for weights in &primitive.weights {
            let influences = weights.iter().filter(|&&weight| weight > 0.0).count() as u32;
            max_joints_per_vertex = max_joints_per_vertex.max(influences);
            let sum: f64 = weights.iter().map(|&weight| f64::from(weight)).sum();
            if sum.is_finite() {
                any_finite_weight = true;
                weight_sum_min = weight_sum_min.min(sum);
                weight_sum_max = weight_sum_max.max(sum);
            }
        }
        for set in &primitive.additional_influence_sets {
            additional_influence_sets
                .entry(set.set_index)
                .and_modify(|entry| {
                    entry.joints_present |= set.joints_present;
                    entry.weights_present |= set.weights_present;
                    entry.joints_without_weights_present |=
                        set.joints_present && !set.weights_present;
                    entry.weights_without_joints_present |=
                        set.weights_present && !set.joints_present;
                })
                .or_insert(AdditionalInfluenceSetMeasurements {
                    set_index: set.set_index,
                    joints_present: set.joints_present,
                    weights_present: set.weights_present,
                    joints_without_weights_present: set.joints_present && !set.weights_present,
                    weights_without_joints_present: set.weights_present && !set.joints_present,
                });
        }
    }

    MeshDefinitionMeasurements {
        mesh_index: mesh.source_mesh_index,
        name: mesh.name.clone(),
        vertex_count,
        geometry_aabb: bounds.finish(),
        geometry_centroid: centroid.finish(),
        max_joints_per_vertex,
        weight_sum_min: any_finite_weight.then_some(weight_sum_min),
        weight_sum_max: any_finite_weight.then_some(weight_sum_max),
        additional_influence_sets: additional_influence_sets.into_values().collect(),
    }
}

fn matrix_is_finite(matrix: Mat4) -> bool {
    matrix
        .to_cols_array()
        .into_iter()
        .all(|component| component.is_finite())
}

fn matrix_to_columns(matrix: Mat4) -> [f32; 16] {
    matrix.to_cols_array()
}

fn vec3_is_finite(value: Vec3) -> bool {
    value.to_array().into_iter().all(f32::is_finite)
}

fn quat_is_finite(value: glam::Quat) -> bool {
    value.to_array().into_iter().all(f32::is_finite)
}

fn source_local_rest_measurement(
    local_rest: &SourceNodeLocalRest,
) -> (SkeletonNodeLocalRestMeasurements, Option<Mat4>) {
    match local_rest {
        SourceNodeLocalRest::Trs {
            translation,
            rotation,
            scale,
        } if vec3_is_finite(*translation)
            && quat_is_finite(*rotation)
            && vec3_is_finite(*scale) =>
        {
            let matrix = Mat4::from_scale_rotation_translation(*scale, *rotation, *translation);
            if matrix_is_finite(matrix) {
                (
                    SkeletonNodeLocalRestMeasurements::Trs {
                        translation_parent_space_m: translation.to_array(),
                        rotation_xyzw: rotation.to_array(),
                        scale: scale.to_array(),
                    },
                    Some(matrix),
                )
            } else {
                (
                    SkeletonNodeLocalRestMeasurements::Unavailable {
                        reason: SkeletonNodeLocalRestUnavailableReason::NonFiniteTransform,
                    },
                    None,
                )
            }
        }
        SourceNodeLocalRest::Matrix(matrix) if matrix_is_finite(*matrix) => (
            SkeletonNodeLocalRestMeasurements::Matrix {
                matrix: matrix_to_columns(*matrix),
            },
            Some(*matrix),
        ),
        _ => (
            SkeletonNodeLocalRestMeasurements::Unavailable {
                reason: SkeletonNodeLocalRestUnavailableReason::NonFiniteTransform,
            },
            None,
        ),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RestWorldVisit {
    Visiting,
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceRestWorldError {
    NonFiniteLocalRest,
    MissingParentNode,
    ParentRestWorldUnavailable,
    ParentCycle,
    NonFiniteWorldMatrix,
}

fn source_rest_world(
    node_index: usize,
    source_nodes: &BTreeMap<usize, (&crate::model::SourceNodeAsset, Option<Mat4>)>,
    visits: &mut BTreeMap<usize, RestWorldVisit>,
    worlds: &mut BTreeMap<usize, Result<Mat4, SourceRestWorldError>>,
) -> Result<Mat4, SourceRestWorldError> {
    if let Some(result) = worlds.get(&node_index) {
        return *result;
    }
    let mut path = Vec::new();
    let mut current = node_index;
    let mut parent_result = loop {
        if let Some(result) = worlds.get(&current) {
            break *result;
        }
        if visits.get(&current) == Some(&RestWorldVisit::Visiting) {
            break Err(SourceRestWorldError::ParentCycle);
        }
        let Some((node, local)) = source_nodes.get(&current) else {
            if path.is_empty() {
                return Err(SourceRestWorldError::MissingParentNode);
            }
            break Err(SourceRestWorldError::MissingParentNode);
        };
        let Some(local) = *local else {
            let result = Err(SourceRestWorldError::NonFiniteLocalRest);
            worlds.insert(current, result);
            visits.insert(current, RestWorldVisit::Done);
            break result;
        };
        visits.insert(current, RestWorldVisit::Visiting);
        path.push((current, local));
        match node.parent_source_node_index {
            Some(parent) => current = parent,
            None => {
                let result = Ok(local);
                worlds.insert(current, result);
                visits.insert(current, RestWorldVisit::Done);
                path.pop();
                break result;
            }
        }
    };

    for (current, local) in path.into_iter().rev() {
        parent_result = match parent_result {
            Err(
                error @ (SourceRestWorldError::MissingParentNode
                | SourceRestWorldError::ParentCycle),
            ) => Err(error),
            Err(_) => Err(SourceRestWorldError::ParentRestWorldUnavailable),
            Ok(parent_world) => {
                let world = parent_world * local;
                matrix_is_finite(world)
                    .then_some(world)
                    .ok_or(SourceRestWorldError::NonFiniteWorldMatrix)
            }
        };
        visits.insert(current, RestWorldVisit::Done);
        worlds.insert(current, parent_result);
    }
    parent_result
}

fn derived_accessor_global_unavailable_reason(
    status: SourceInverseBindAccessorStatus,
) -> Option<SkinDerivedMatrixUnavailableReason> {
    match status {
        SourceInverseBindAccessorStatus::Absent => {
            Some(SkinDerivedMatrixUnavailableReason::InverseBindAccessorAbsent)
        }
        SourceInverseBindAccessorStatus::EmptyAccessor => {
            Some(SkinDerivedMatrixUnavailableReason::InverseBindAccessorEmpty)
        }
        SourceInverseBindAccessorStatus::Unreadable => {
            Some(SkinDerivedMatrixUnavailableReason::InverseBindAccessorUnreadable)
        }
        SourceInverseBindAccessorStatus::Available
        | SourceInverseBindAccessorStatus::CountMismatch => None,
    }
}

fn invertible_matrix(matrix: Mat4) -> Option<Mat4> {
    let determinant = matrix.determinant();
    if !determinant.is_finite() || determinant == 0.0 {
        return None;
    }
    let inverse = matrix.inverse();
    matrix_is_finite(inverse).then_some(inverse)
}

/// Derive deterministic scale, orientation, and shape facts from an affine
/// matrix's linear 3x3 part.
///
/// Singularity is tested relative to the product of the three axis lengths,
/// so the classification does not depend on whether the transform happens to
/// use metre, centimetre, or another uniformly scaled coordinate system.
/// Orthogonality remains pair-normalized rather than using the positive-uniform
/// operation classifier's common-factor band. Public precedence is singular,
/// reflected, sheared, unit/uniform, then non-uniform; the other numeric facts
/// remain available on reflected and singular observations.
pub fn measure_linear_transform(matrix: Mat4) -> LinearTransformMeasurements {
    if !matrix_is_finite(matrix) {
        return LinearTransformMeasurements {
            classification: LinearTransformClassification::NonFinite,
            axis_lengths: None,
            determinant: None,
            orientation: None,
            uniform_scale: None,
        };
    }

    // Matrices are stored as f32, but scale-cubed determinants can overflow or
    // underflow f32 even when every source component is finite and the matrix
    // is well-conditioned. The model-owned fact seam widens every input before
    // deriving lengths, determinant, product, mean, and dot products.
    let facts = match AffineGeometryFacts::from_linear(Mat3::from_mat4(matrix)) {
        Ok(facts) => facts,
        Err(_) => return unavailable_linear_transform(),
    };

    let singular = facts.axis_length_product == 0.0
        || facts.determinant.abs()
            <= LINEAR_CLASSIFICATION_SINGULAR_TOLERANCE * facts.axis_length_product;
    let orientation = if singular {
        LinearTransformOrientation::Zero
    } else if facts.determinant < 0.0 {
        LinearTransformOrientation::Negative
    } else {
        LinearTransformOrientation::Positive
    };
    let orthogonal = [(0usize, 1usize), (0, 2), (1, 2)]
        .into_iter()
        .zip(facts.cross_axis_dots)
        .all(|((left, right), dot)| {
            let length_product = facts.axis_lengths[left] * facts.axis_lengths[right];
            length_product == 0.0
                || dot.abs() <= LINEAR_CLASSIFICATION_RELATIVE_TOLERANCE * length_product
        });
    let uniform = facts.has_equal_axis_lengths(LINEAR_CLASSIFICATION_RELATIVE_TOLERANCE);
    let uniform_scale = (orthogonal && uniform).then_some(facts.mean_axis_length);
    let unit = uniform_scale
        .is_some_and(|scale| (scale - 1.0).abs() <= LINEAR_CLASSIFICATION_RELATIVE_TOLERANCE);
    let classification = if singular {
        LinearTransformClassification::Singular
    } else if orientation == LinearTransformOrientation::Negative {
        LinearTransformClassification::Reflected
    } else if !orthogonal {
        LinearTransformClassification::Sheared
    } else if unit {
        LinearTransformClassification::UnitOrthonormal
    } else if uniform {
        LinearTransformClassification::UniformScaled
    } else {
        LinearTransformClassification::NonUniform
    };

    LinearTransformMeasurements {
        classification,
        axis_lengths: Some(facts.axis_lengths),
        determinant: Some(facts.determinant),
        orientation: Some(orientation),
        uniform_scale,
    }
}

pub(crate) fn summarize_skin_bind_linear(
    joints: &[SkinJointMeasurements],
) -> SkinBindLinearSummaryMeasurements {
    let joint_count = joints.len();
    let available: Vec<_> = joints
        .iter()
        .filter_map(|joint| joint.joint_bind_to_mesh.linear)
        .collect();
    let available_joint_count = available.len();
    let unavailable_joint_count = joint_count.saturating_sub(available_joint_count);
    let (classification, consistent_uniform_scale) = if joint_count == 0 {
        (SkinBindLinearSummaryClassification::NoJoints, None)
    } else if available_joint_count == 0 {
        (SkinBindLinearSummaryClassification::Unavailable, None)
    } else if unavailable_joint_count > 0 {
        (
            SkinBindLinearSummaryClassification::PartiallyUnavailable,
            None,
        )
    } else if available.iter().all(|linear| {
        matches!(
            linear.classification,
            LinearTransformClassification::UnitOrthonormal
                | LinearTransformClassification::UniformScaled
        )
    }) {
        let mut factors = available
            .iter()
            .map(|linear| {
                linear
                    .uniform_scale
                    .expect("uniform classifications carry a scale")
            })
            .collect::<Vec<_>>();
        // Joint order is source metadata, not geometry. Canonicalize the sum
        // order, compare every factor symmetrically with the mean, and report
        // that mean so neither classification nor evidence privileges joint 0.
        factors.sort_by(f64::total_cmp);
        let mean = factors.iter().sum::<f64>() / factors.len() as f64;
        if values_equal_to_mean(&factors, mean, LINEAR_CLASSIFICATION_RELATIVE_TOLERANCE) {
            (
                SkinBindLinearSummaryClassification::ConsistentUniform,
                Some(mean),
            )
        } else {
            (SkinBindLinearSummaryClassification::MixedUniform, None)
        }
    } else if available.iter().all(|linear| {
        matches!(
            linear.classification,
            LinearTransformClassification::NonUniform | LinearTransformClassification::Sheared
        )
    }) {
        (
            SkinBindLinearSummaryClassification::NonUniformOrSheared,
            None,
        )
    } else if available.iter().all(|linear| {
        matches!(
            linear.classification,
            LinearTransformClassification::Reflected | LinearTransformClassification::Singular
        )
    }) {
        (
            SkinBindLinearSummaryClassification::ReflectedOrSingular,
            None,
        )
    } else {
        (SkinBindLinearSummaryClassification::Mixed, None)
    };
    SkinBindLinearSummaryMeasurements {
        classification,
        joint_count,
        available_joint_count,
        unavailable_joint_count,
        consistent_uniform_scale,
    }
}

fn unavailable_derived_matrix(
    reason: SkinDerivedMatrixUnavailableReason,
) -> SkinDerivedMatrixMeasurements {
    SkinDerivedMatrixMeasurements {
        matrix: None,
        linear: None,
        unavailable_reason: Some(reason),
    }
}

fn available_derived_matrix(matrix: Mat4) -> SkinDerivedMatrixMeasurements {
    SkinDerivedMatrixMeasurements {
        matrix: Some(matrix_to_columns(matrix)),
        linear: Some(measure_linear_transform(matrix)),
        unavailable_reason: None,
    }
}

pub(crate) fn measure_source_skeleton(
    doc: &Document,
) -> (
    SkeletonSourceCoverage,
    Vec<SkeletonNodeMeasurements>,
    Vec<SkinMeasurements>,
) {
    let source = &doc.assets.source_skeleton;
    if source.coverage == SourceSkeletonCoverage::Unavailable {
        return (SourceSkeletonCoverage::Unavailable, Vec::new(), Vec::new());
    }

    let mut source_nodes = BTreeMap::new();
    for node in &source.nodes {
        let (_, local) = source_local_rest_measurement(&node.local_rest);
        if source_nodes
            .insert(node.source_node_index, (node, local))
            .is_some()
        {
            return (SourceSkeletonCoverage::Unavailable, Vec::new(), Vec::new());
        }
    }
    for skin in &source.skins {
        if skin
            .joint_source_node_indices
            .iter()
            .any(|joint| !source_nodes.contains_key(joint))
            || skin
                .skeleton_root_source_node_index
                .is_some_and(|root| !source_nodes.contains_key(&root))
            || skin
                .attachments
                .iter()
                .any(|attachment| !source_nodes.contains_key(&attachment.source_node_index))
        {
            return (SourceSkeletonCoverage::Unavailable, Vec::new(), Vec::new());
        }
    }

    let mut visits = BTreeMap::new();
    let mut worlds = BTreeMap::new();
    for node in &source.nodes {
        let _ = source_rest_world(
            node.source_node_index,
            &source_nodes,
            &mut visits,
            &mut worlds,
        );
    }
    let mut skeleton_nodes = Vec::with_capacity(source.nodes.len());
    for node in &source.nodes {
        let (local_rest, _) = source_local_rest_measurement(&node.local_rest);
        let Some(world) = worlds.get(&node.source_node_index).copied() else {
            return (SourceSkeletonCoverage::Unavailable, Vec::new(), Vec::new());
        };
        let (
            rest_world_matrix,
            rest_world_translation_m,
            rest_world_linear,
            rest_world_matrix_unavailable_reason,
        ) = match world {
            Ok(matrix) => (
                Some(matrix_to_columns(matrix)),
                Some(matrix.w_axis.truncate().to_array()),
                measure_linear_transform(matrix),
                None,
            ),
            Err(SourceRestWorldError::NonFiniteLocalRest) => (
                None,
                None,
                unavailable_linear_transform(),
                Some(SkeletonRestWorldMatrixUnavailableReason::NonFiniteLocalRest),
            ),
            Err(SourceRestWorldError::ParentRestWorldUnavailable) => (
                None,
                None,
                unavailable_linear_transform(),
                Some(SkeletonRestWorldMatrixUnavailableReason::ParentRestWorldUnavailable),
            ),
            Err(SourceRestWorldError::NonFiniteWorldMatrix) => (
                None,
                None,
                unavailable_linear_transform(),
                Some(SkeletonRestWorldMatrixUnavailableReason::NonFiniteWorldMatrix),
            ),
            Err(SourceRestWorldError::MissingParentNode | SourceRestWorldError::ParentCycle) => {
                return (SourceSkeletonCoverage::Unavailable, Vec::new(), Vec::new());
            }
        };
        skeleton_nodes.push(SkeletonNodeMeasurements {
            node_index: node.source_node_index,
            name: node.name.clone(),
            parent_node_index: node.parent_source_node_index,
            scene_root_indices: node.scene_root_indices.clone(),
            local_rest,
            rest_world_matrix,
            rest_world_translation_m,
            rest_world_linear,
            rest_world_matrix_unavailable_reason,
        });
    }

    let skins = source
        .skins
        .iter()
        .map(|skin| {
            let all_raw_finite = skin
                .inverse_bind_accessor
                .matrices
                .iter()
                .all(|matrix| matrix_is_finite(*matrix));
            let status = if all_raw_finite {
                skin.inverse_bind_accessor.status
            } else {
                SourceInverseBindAccessorStatus::Unreadable
            };
            let raw_matrices = if all_raw_finite {
                skin.inverse_bind_accessor
                    .matrices
                    .iter()
                    .copied()
                    .map(matrix_to_columns)
                    .collect()
            } else {
                Vec::new()
            };
            let inverse_bind_accessor = SkinInverseBindAccessorMeasurements {
                status,
                declared_count: skin.inverse_bind_accessor.declared_count,
                matrices: raw_matrices,
            };
            let joints: Vec<_> = skin
                .joint_source_node_indices
                .iter()
                .enumerate()
                .map(|(joint_index, &node_index)| {
                    let unavailable_joint = |reason| SkinJointMeasurements {
                        joint_index,
                        node_index,
                        joint_bind_to_mesh: unavailable_derived_matrix(reason),
                        mesh_bind_world: unavailable_derived_matrix(reason),
                    };
                    let raw = match derived_accessor_global_unavailable_reason(status) {
                        Some(reason) => return unavailable_joint(reason),
                        None => match skin.inverse_bind_accessor.matrices.get(joint_index).copied() {
                            Some(raw) => raw,
                            None => {
                                let reason =
                                    SkinDerivedMatrixUnavailableReason::InverseBindAccessorCountMismatch;
                                return unavailable_joint(reason);
                            }
                        },
                    };
                    let joint_bind_to_mesh = invertible_matrix(raw).map_or_else(
                        || {
                            unavailable_derived_matrix(
                                SkinDerivedMatrixUnavailableReason::InverseBindMatrixNonInvertible,
                            )
                        },
                        available_derived_matrix,
                    );
                    let world = worlds
                        .get(&node_index)
                        .copied()
                        .unwrap_or(Err(SourceRestWorldError::ParentRestWorldUnavailable));
                    let mesh_bind_world = match world {
                        Ok(world) => {
                            let matrix = world * raw;
                            matrix_is_finite(matrix).then_some(()).map_or_else(
                                || {
                                    unavailable_derived_matrix(
                                        SkinDerivedMatrixUnavailableReason::NonFiniteDerivedMatrix,
                                    )
                                },
                                |_| available_derived_matrix(matrix),
                            )
                        }
                        Err(_) => unavailable_derived_matrix(
                            SkinDerivedMatrixUnavailableReason::JointRestWorldUnavailable,
                        ),
                    };
                    SkinJointMeasurements {
                        joint_index,
                        node_index,
                        joint_bind_to_mesh,
                        mesh_bind_world,
                    }
                })
                .collect();
            let joint_bind_linear_summary = summarize_skin_bind_linear(&joints);
            SkinMeasurements {
                skin_index: skin.source_skin_index,
                name: skin.name.clone(),
                skeleton_root_node_index: skin.skeleton_root_source_node_index,
                joints,
                joint_bind_linear_summary,
                inverse_bind_accessor,
                attachments: skin
                    .attachments
                    .iter()
                    .map(|attachment| SkinAttachmentMeasurements {
                        node_index: attachment.source_node_index,
                        mesh_index: attachment.source_mesh_index,
                    })
                    .collect(),
            }
        })
        .collect();
    (SourceSkeletonCoverage::Complete, skeleton_nodes, skins)
}

fn transformed_definition_aabb(
    mesh: &MeshAsset,
    world: Mat4,
) -> Result<Aabb, StaticNodeAabbUnavailableReason> {
    let mut bounds = Bounds::default();
    let mut any_finite_source = false;
    for primitive in &mesh.primitives {
        for &position in &primitive.positions {
            if !position.is_finite() {
                continue;
            }
            any_finite_source = true;
            if !bounds.include(world.transform_point3(position)) {
                return Err(StaticNodeAabbUnavailableReason::NonFiniteTransform);
            }
        }
    }
    if !any_finite_source {
        return Err(StaticNodeAabbUnavailableReason::NoFinitePositions);
    }
    bounds
        .finish()
        .ok_or(StaticNodeAabbUnavailableReason::NonFiniteTransform)
}

#[derive(Debug, Clone, Copy, Default)]
struct NodeAggregate {
    bounds: Bounds,
    instance_count: usize,
    excluded_instance_count: usize,
}

impl NodeAggregate {
    fn include(&mut self, other: Self) {
        if let Some(aabb) = other.bounds.finish() {
            self.bounds.include_aabb(aabb);
        }
        self.instance_count = self.instance_count.saturating_add(other.instance_count);
        self.excluded_instance_count = self
            .excluded_instance_count
            .saturating_add(other.excluded_instance_count);
    }
}

/// Measure source mesh definitions, their default/rest node instances, and
/// every declared scene without sampling animation or deformation.
///
/// World transforms are composed once in parent-before-child order. Scene
/// aggregates are then derived from reverse-order subtree summaries, so a file
/// with many scenes cannot force work proportional to scenes × all nodes.
/// Non-finite geometry never reaches JSON; non-finite effective transforms and
/// skinned instances are represented by typed unavailability reasons.
pub fn measure_assets(doc: &Document) -> AssetMeasurements {
    let (skeleton_source_coverage, skeleton_nodes, skins) = measure_source_skeleton(doc);
    let material_resource_coverage = doc.assets.material_resources.coverage;
    let material_definitions = doc
        .assets
        .material_resources
        .materials
        .iter()
        .map(|material| MaterialDefinitionMeasurements {
            material_index: material.material_index,
            name: material.name.clone(),
            texture_bindings: material
                .texture_bindings
                .iter()
                .map(|binding| MaterialTextureBindingMeasurements {
                    slot: binding.slot,
                    texture_index: binding.texture_index,
                })
                .collect(),
        })
        .collect();
    let textures = doc
        .assets
        .material_resources
        .textures
        .iter()
        .map(|texture| TextureMeasurements {
            texture_index: texture.texture_index,
            name: texture.name.clone(),
            image_index: texture.image_index,
        })
        .collect();
    let images = doc
        .assets
        .material_resources
        .images
        .iter()
        .map(|image| {
            let (width, height, channel_count, decoded_color_type, unavailable_reason) =
                match image.inspection {
                    SourceImageInspection::Available {
                        width,
                        height,
                        channel_count,
                        color_type,
                    } => (
                        Some(width),
                        Some(height),
                        Some(channel_count),
                        Some(color_type),
                        None,
                    ),
                    SourceImageInspection::Unavailable { reason } => {
                        (None, None, None, None, Some(reason))
                    }
                };
            ImageMeasurements {
                image_index: image.image_index,
                name: image.name.clone(),
                source_kind: image.source_kind,
                declared_mime_type: image.declared_mime_type.clone(),
                detected_container: image.detected_container,
                width,
                height,
                channel_count,
                decoded_color_type,
                unavailable_reason,
            }
        })
        .collect();
    let mesh_definitions = doc
        .assets
        .meshes
        .iter()
        .map(measure_mesh_definition)
        .collect::<Vec<_>>();
    let worlds = tolerant_world_rest_matrices(&doc.skeleton);
    let mut node_aggregates = vec![NodeAggregate::default(); doc.skeleton.bones.len()];
    let mut node_instances = Vec::with_capacity(doc.assets.instances.len());

    for instance in &doc.assets.instances {
        let Some(mesh) = doc.assets.meshes.get(instance.mesh) else {
            continue;
        };
        let bounds = if !instance.skin_joints.is_empty() {
            Err(StaticNodeAabbUnavailableReason::SkinnedDeformationExcluded)
        } else {
            match worlds.get(instance.node).copied().flatten() {
                Some(world) => transformed_definition_aabb(mesh, world),
                None => Err(StaticNodeAabbUnavailableReason::NonFiniteTransform),
            }
        };
        let (static_node_world_aabb, unavailable) = match bounds {
            Ok(aabb) => (Some(aabb), None),
            Err(reason) => (None, Some(reason)),
        };
        let node_name = doc
            .skeleton
            .bones
            .get(instance.node)
            .map(|bone| bone.name.clone())
            .unwrap_or_else(|| format!("node-{}", instance.source_node_index));
        let measurement = NodeInstanceMeasurements {
            node_index: instance.source_node_index,
            node_name,
            mesh_index: mesh.source_mesh_index,
            static_node_world_aabb,
            static_node_world_aabb_unavailable_reason: unavailable,
        };
        if let Some(aggregate) = node_aggregates.get_mut(instance.node) {
            aggregate.instance_count = aggregate.instance_count.saturating_add(1);
            match measurement.static_node_world_aabb {
                Some(aabb) => aggregate.bounds.include_aabb(aabb),
                None => {
                    aggregate.excluded_instance_count =
                        aggregate.excluded_instance_count.saturating_add(1);
                }
            }
        }
        node_instances.push(measurement);
    }

    // Parent-before-child is a loader invariant. Walking in reverse lets each
    // subtree contribute once to its parent and then to any scene that names
    // the root, avoiding a scenes × nodes traversal.
    for node in (0..doc.skeleton.bones.len()).rev() {
        let Some(parent) = doc.skeleton.bones[node].parent else {
            continue;
        };
        let child = node_aggregates[node];
        if let Some(parent_aggregate) = node_aggregates.get_mut(parent) {
            parent_aggregate.include(child);
        }
    }

    let scenes = doc
        .assets
        .scenes
        .iter()
        .map(|scene| {
            let mut aggregate = NodeAggregate::default();
            for &root in &scene.roots {
                if let Some(root_aggregate) = node_aggregates.get(root).copied() {
                    aggregate.include(root_aggregate);
                }
            }
            SceneMeasurements {
                scene_index: scene.source_scene_index,
                name: scene.name.clone(),
                instance_count: aggregate.instance_count,
                static_scene_world_aabb: aggregate.bounds.finish(),
                excluded_instance_count: aggregate.excluded_instance_count,
            }
        })
        .collect();

    AssetMeasurements {
        material_resource_coverage,
        material_definitions,
        textures,
        images,
        skeleton_source_coverage,
        skeleton_nodes,
        skins,
        mesh_definitions,
        node_instances,
        scenes,
        default_scene_index: doc.assets.default_scene,
    }
}

/// Role-dependent gait metrics for one clip.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct GaitMeasurement {
    /// Stride-anchor phase in `[0,1)`; see
    /// [`crate::metrics::FootCycleMetrics::gait_phase`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<f64>,
    /// Peak-to-peak L−R foot-height swing (metres).
    pub lr_amplitude_m: f64,
}

/// Model-space loop-continuity measurements for one skeleton bone.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct BoneLoopContinuityMeasurement {
    /// Stable zero-based bone index in skeleton order.
    pub bone_index: u32,
    /// Human-readable bone name. Consumers should use `bone_index` as the
    /// identity because display names are not required to be unique.
    pub bone_name: String,
    /// Last-sample to first-sample model-space position distance (metres).
    pub position_delta_m: f64,
    /// Shortest-path model-space rotation difference (degrees).
    pub rotation_delta_deg: f64,
    /// Difference between the model-space linear velocities immediately
    /// before and after the wrap (metres per second).
    pub seam_velocity_delta_mps: f64,
    /// Difference between the model-space angular velocities immediately
    /// before and after the wrap (degrees per second).
    pub seam_angular_velocity_delta_degps: f64,
}

/// Per-bone C0 pose closure plus C1 linear- and angular-velocity continuity
/// for one clip.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct LoopContinuityMeasurement {
    /// Measurements in skeleton order.
    pub bones: Vec<BoneLoopContinuityMeasurement>,
}

/// The observed endpoint convention of a declared looping clip.
///
/// This is emitted only when enough authored or sampled evidence exists. In
/// particular, it intentionally does not infer a mode for clips not declared
/// as loops, malformed authored tracks, or clips without usable continuity
/// samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum LoopEndpointMode {
    /// A non-duplicate declared loop whose inclusive pose closure is within
    /// the effective loop-closure caps.
    UniqueCycle,
    /// The strict, mechanically removable duplicate-endpoint predicate from
    /// `duplicate-loop-endpoint` succeeded.
    DuplicateEndpoint,
    /// A declared loop whose inclusive pose closure exceeds an effective
    /// loop-closure cap.
    NonClosing,
}

impl LoopEndpointMode {
    /// Stable machine-readable spelling used in serialized endpoint evidence.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UniqueCycle => "unique_cycle",
            Self::DuplicateEndpoint => "duplicate_endpoint",
            Self::NonClosing => "non_closing",
        }
    }
}

/// Valid declared frame-grid evidence for a clip.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct FrameGridMeasurement {
    /// Declared frames per second used to validate the authored grid.
    pub fps: f64,
    /// Rounded number of duration intervals at [`Self::fps`].
    pub frame_intervals: u32,
}

/// Measurements for one clip in the `measure` output map.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ClipMeasurements {
    /// Clip duration in seconds.
    pub duration_s: f64,
    /// Keyframe count of the longest channel. This also selects the uniform
    /// metric-grid resolution, but it is not an authored frame-rate value.
    pub frame_count: u32,
    /// Bones with at least one keyframed channel, sorted.
    pub animated_bones: Vec<String>,
    /// Max rotation deviation (degrees) of each bone from its first
    /// keyed rotation. Bones under [`MIN_RECORDED_ROTATION_DEG`] are
    /// omitted.
    pub bone_rotation_range_deg: BTreeMap<String, f64>,
    /// Model-space pose closure plus seam-adjacent linear- and angular-velocity
    /// continuity for every skeleton bone. Available without rig-role
    /// resolution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loop_continuity: Option<LoopContinuityMeasurement>,
    /// Endpoint convention measured for a declared looping clip when enough
    /// authored or model-space continuity evidence is available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loop_endpoint_mode: Option<LoopEndpointMode>,
    /// Declared FPS-grid evidence when the duration and every authored key
    /// land within the established frame-grid tolerance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_grid: Option<FrameGridMeasurement>,
    /// Loop wrap discontinuity ratio; needs hips + foot roles and a
    /// real stride. See [`crate::metrics::FootCycleMetrics`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loop_seam_ratio: Option<f64>,
    /// Gait stride anchor; needs a left and a right foot role.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gait: Option<GaitMeasurement>,
    /// Horizontal root displacement ÷ duration (m/s); needs the Root
    /// (or Hips) role.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed_mps: Option<f64>,
}

/// Measure every clip using shared metric pose grids. Role-dependent
/// metrics (loop seam, gait, root-motion speed) are present only where
/// the roles resolve; pass an empty [`ResolvedRoles`] to skip them.
///
/// This returns clip measurements only. Call [`measure_assets`] separately
/// when the pipeline also needs static scene measurements. Clip names are map
/// keys and therefore must be unique; a later duplicate replaces an earlier
/// entry.
pub fn measure_document(
    grids: &MetricGrids<'_>,
    roles: &ResolvedRoles,
    config: &Config,
) -> BTreeMap<String, ClipMeasurements> {
    let doc = grids.document();
    let min_stride_step_m = config.loop_seam_min_stride_step_m();
    doc.clips
        .iter()
        .enumerate()
        .map(|(clip_index, clip)| {
            let mut animated: BTreeSet<String> = BTreeSet::new();
            let mut rotation_range: BTreeMap<String, f64> = BTreeMap::new();
            let mut frame_count = 0usize;

            for track in &clip.tracks {
                let Some(bone) = doc.skeleton.bones.get(track.bone) else {
                    continue;
                };
                if track.key_count() == 0 {
                    continue;
                }
                animated.insert(bone.name.clone());
                frame_count = frame_count.max(track.key_count());

                if let Some(max_deg) = rotation_range_deg(track)
                    && max_deg >= MIN_RECORDED_ROTATION_DEG
                {
                    let entry = rotation_range.entry(bone.name.clone()).or_insert(0.0);
                    *entry = entry.max(max_deg);
                }
            }

            let grid = grids.grid(clip_index);
            let cycle = grid
                .as_ref()
                .and_then(|g| foot_cycle_metrics(g, roles, min_stride_step_m));
            let loop_continuity = grid.as_ref().and_then(|grid| {
                let metrics = loop_continuity_metrics(grid)?;
                Some(LoopContinuityMeasurement {
                    bones: metrics
                        .into_iter()
                        .enumerate()
                        .map(|(bone_index, metrics)| BoneLoopContinuityMeasurement {
                            bone_index: bone_index as u32,
                            bone_name: doc.skeleton.bones[bone_index].name.clone(),
                            position_delta_m: metrics.position_delta_m,
                            rotation_delta_deg: metrics.rotation_delta_deg,
                            seam_velocity_delta_mps: metrics.seam_velocity_delta_mps,
                            seam_angular_velocity_delta_degps: metrics
                                .seam_angular_velocity_delta_degps,
                        })
                        .collect(),
                })
            });
            let expectations = config.expectations_for(&clip.name);
            let (position_cap, rotation_cap) = effective_caps(config, &expectations);
            let loop_endpoint_mode = (expectations.looping == Some(true))
                .then(|| {
                    measure_loop_endpoint_mode(clip, grid.as_deref(), position_cap, rotation_cap)
                })
                .flatten();
            let frame_grid = measure_frame_grid(clip, expectations.fps);
            let speed_mps = grid.as_ref().and_then(|g| root_motion_speed_mps(g, roles));
            let duration_s = if clip.duration_s.is_finite() {
                clip.duration_s
            } else {
                clip.tracks
                    .iter()
                    .flat_map(|track| track.times.iter().copied())
                    .filter(|time| time.is_finite())
                    .map(f64::from)
                    .fold(0.0, f64::max)
            };

            (
                clip.name.clone(),
                ClipMeasurements {
                    duration_s,
                    frame_count: frame_count as u32,
                    animated_bones: animated.into_iter().collect(),
                    bone_rotation_range_deg: rotation_range,
                    loop_continuity,
                    loop_endpoint_mode,
                    frame_grid,
                    loop_seam_ratio: cycle.as_ref().and_then(|c| c.loop_seam_ratio),
                    gait: cycle.map(|c| GaitMeasurement {
                        phase: c.gait_phase,
                        lr_amplitude_m: c.lr_amplitude_m,
                    }),
                    speed_mps,
                },
            )
        })
        .collect()
}

/// Measure endpoint evidence for a looping clip. Callers own the declaration
/// policy; [`measure_document`] invokes this only for `loop = true` clips.
pub(crate) fn measure_loop_endpoint_mode(
    clip: &crate::model::Clip,
    grid: Option<&PoseGrid>,
    max_position_delta_m: f64,
    max_rotation_delta_deg: f64,
) -> Option<LoopEndpointMode> {
    match analyze_duplicate_loop_endpoint(clip) {
        Ok(Some(_)) => return Some(LoopEndpointMode::DuplicateEndpoint),
        Ok(None) => {}
        Err(_) => return None,
    }
    let continuity = loop_continuity_metrics(grid?)?;
    let closes = continuity.iter().all(|bone| {
        !exceeds_f32_cap(bone.position_delta_m, max_position_delta_m)
            && !exceeds_f32_cap(bone.rotation_delta_deg, max_rotation_delta_deg)
    });
    Some(if closes {
        LoopEndpointMode::UniqueCycle
    } else {
        LoopEndpointMode::NonClosing
    })
}

/// Measure valid declared FPS-grid evidence for one clip.
pub(crate) fn measure_frame_grid(
    clip: &crate::model::Clip,
    declared_fps: Option<f64>,
) -> Option<FrameGridMeasurement> {
    let fps = declared_fps?;
    if !fps.is_finite() || fps <= 0.0 || !clip.duration_s.is_finite() || clip.duration_s <= 0.0 {
        return None;
    }
    let intervals = clip.duration_s * fps;
    if !intervals.is_finite() || (intervals - intervals.round()).abs() > GRID_TOLERANCE_FRAMES {
        return None;
    }
    let rounded = intervals.round();
    if !(0.0..=f64::from(u32::MAX)).contains(&rounded) {
        return None;
    }
    if clip
        .tracks
        .iter()
        .flat_map(|track| &track.times)
        .any(|&time| {
            let frames = f64::from(time) * fps;
            !frames.is_finite() || (frames - frames.round()).abs() > GRID_TOLERANCE_FRAMES
        })
    {
        return None;
    }
    Some(FrameGridMeasurement {
        fps,
        frame_intervals: rounded as u32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        AdditionalInfluenceSet, AffineDomainViolation, Bone, Clip, Document, Interpolation,
        MeshAsset, PositiveUniformAffineTolerance, Primitive, Property, SceneAsset, SceneAssets,
        Skeleton, SourceInverseBindAccessor, SourceInverseBindAccessorStatus, SourceNodeAsset,
        SourceNodeLocalRest, SourceSkeletonAssets, SourceSkeletonCoverage, SourceSkinAsset,
        SourceSkinAttachment, Track, TrackValues, Transform, classify_positive_uniform_affine,
    };
    use crate::profile::Role;
    use glam::{Mat4, Quat, Vec3};

    fn mesh(name: &str, primitives: Vec<Primitive>) -> MeshDefinitionMeasurements {
        let doc = Document {
            assets: SceneAssets {
                meshes: vec![MeshAsset {
                    name: name.into(),
                    source_mesh_index: 0,
                    primitives,
                }],
                ..SceneAssets::default()
            },
            ..Document::default()
        };
        measure_assets(&doc).mesh_definitions.remove(0)
    }

    #[test]
    fn only_globally_unavailable_inverse_bind_accessors_have_a_derived_reason() {
        assert_eq!(
            derived_accessor_global_unavailable_reason(SourceInverseBindAccessorStatus::Absent),
            Some(SkinDerivedMatrixUnavailableReason::InverseBindAccessorAbsent)
        );
        assert_eq!(
            derived_accessor_global_unavailable_reason(
                SourceInverseBindAccessorStatus::EmptyAccessor
            ),
            Some(SkinDerivedMatrixUnavailableReason::InverseBindAccessorEmpty)
        );
        assert_eq!(
            derived_accessor_global_unavailable_reason(SourceInverseBindAccessorStatus::Unreadable),
            Some(SkinDerivedMatrixUnavailableReason::InverseBindAccessorUnreadable)
        );
        assert_eq!(
            derived_accessor_global_unavailable_reason(SourceInverseBindAccessorStatus::Available),
            None
        );
        assert_eq!(
            derived_accessor_global_unavailable_reason(
                SourceInverseBindAccessorStatus::CountMismatch
            ),
            None,
            "a readable count-mismatched accessor can still supply earlier slots"
        );
    }

    #[test]
    fn linear_transform_measurements_classify_affine_shape_and_orientation() {
        let cases = [
            (
                Mat4::IDENTITY,
                LinearTransformClassification::UnitOrthonormal,
                Some(LinearTransformOrientation::Positive),
                Some(1.0),
            ),
            (
                Mat4::from_scale(Vec3::splat(0.01)),
                LinearTransformClassification::UniformScaled,
                Some(LinearTransformOrientation::Positive),
                Some(f64::from(0.01f32)),
            ),
            (
                Mat4::from_scale(Vec3::new(2.0, 3.0, 4.0)),
                LinearTransformClassification::NonUniform,
                Some(LinearTransformOrientation::Positive),
                None,
            ),
            (
                Mat4::from_cols_array(&[
                    1.0, 0.0, 0.0, 0.0, 0.5, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
                ]),
                LinearTransformClassification::Sheared,
                Some(LinearTransformOrientation::Positive),
                None,
            ),
            (
                Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0)),
                LinearTransformClassification::Reflected,
                Some(LinearTransformOrientation::Negative),
                Some(1.0),
            ),
            (
                Mat4::from_scale(Vec3::new(1.0, 0.0, 1.0)),
                LinearTransformClassification::Singular,
                Some(LinearTransformOrientation::Zero),
                None,
            ),
        ];
        for (matrix, classification, orientation, uniform_scale) in cases {
            let measured = measure_linear_transform(matrix);
            assert_eq!(measured.classification, classification);
            assert_eq!(measured.orientation, orientation);
            assert_eq!(measured.uniform_scale, uniform_scale);
            assert!(measured.axis_lengths.is_some());
            assert!(measured.determinant.is_some());
        }

        let non_finite = measure_linear_transform(Mat4::from_cols_array(&[f32::NAN; 16]));
        assert_eq!(
            non_finite,
            LinearTransformMeasurements {
                classification: LinearTransformClassification::NonFinite,
                axis_lengths: None,
                determinant: None,
                orientation: None,
                uniform_scale: None,
            }
        );

        for scale in [1.0e-30f32, 1.0e-16, 1.0e13, 1.0e30] {
            let measured = measure_linear_transform(Mat4::from_scale(Vec3::splat(scale)));
            assert_eq!(
                measured.classification,
                LinearTransformClassification::UniformScaled,
                "finite uniform scale {scale:e}"
            );
            assert_eq!(measured.uniform_scale, Some(f64::from(scale)));
            assert!(measured.determinant.is_some_and(f64::is_finite));
            assert_ne!(measured.determinant, Some(0.0));
        }
    }

    #[test]
    fn linear_measurement_reconciles_equal_axis_fixtures_in_every_axis_order() {
        let permutations = |[x, y, z]: [f32; 3]| {
            [
                Vec3::new(x, y, z),
                Vec3::new(x, z, y),
                Vec3::new(y, x, z),
                Vec3::new(y, z, x),
                Vec3::new(z, x, y),
                Vec3::new(z, y, x),
            ]
        };
        let policy = PositiveUniformAffineTolerance {
            equal_axis: LINEAR_CLASSIFICATION_RELATIVE_TOLERANCE,
            relative_orthogonality: LINEAR_CLASSIFICATION_RELATIVE_TOLERANCE,
            singular_determinant_relative: LINEAR_CLASSIFICATION_SINGULAR_TOLERANCE,
        };

        for diagonal in permutations([1.0, 1.0, 1.000_012]) {
            let measured = measure_linear_transform(Mat4::from_scale(diagonal));
            assert_eq!(
                measured.classification,
                LinearTransformClassification::UnitOrthonormal,
                "issue fixture {diagonal:?}"
            );
            assert_eq!(
                classify_positive_uniform_affine(Mat3::from_diagonal(diagonal), policy),
                measured
                    .uniform_scale
                    .ok_or(AffineDomainViolation::NonFinite),
                "measurement and Appendix D share the equal-axis decision"
            );
        }

        // The old measurement compared only X-Y and X-Z, so this exact shape
        // changed class when either extreme occupied X. Mean-relative
        // comparison gives every column permutation the same class.
        let high = f32::from_bits(0x3f80_004b);
        let low = f32::from_bits(0x3f7f_ff69);
        for diagonal in permutations([1.0, high, low]) {
            assert_eq!(
                measure_linear_transform(Mat4::from_scale(diagonal)).classification,
                LinearTransformClassification::UnitOrthonormal,
                "axis-order counterexample {diagonal:?}"
            );
        }
    }

    #[test]
    fn linear_measurement_pins_equal_axis_boundaries_and_extreme_finite_scales() {
        let on_long_edge = Vec3::new(99_998.5, 99_998.5, 100_000.0);
        let measured = measure_linear_transform(Mat4::from_scale(on_long_edge));
        assert_eq!(
            measured.classification,
            LinearTransformClassification::UniformScaled
        );
        assert_eq!(measured.uniform_scale, Some(99_999.0));

        let short = 99_998.5;
        let outside = 100_000.0 + 0.007_812_5;
        for diagonal in [
            Vec3::new(outside, short, short),
            Vec3::new(short, outside, short),
            Vec3::new(short, short, outside),
        ] {
            assert_eq!(
                measure_linear_transform(Mat4::from_scale(diagonal)).classification,
                LinearTransformClassification::NonUniform
            );
        }

        for scale in [f32::from_bits(1), f32::MIN_POSITIVE, f32::MAX] {
            let measured = measure_linear_transform(Mat4::from_scale(Vec3::splat(scale)));
            assert_eq!(
                measured.classification,
                LinearTransformClassification::UniformScaled,
                "complete finite f32 scale range at {scale:e}"
            );
            assert_eq!(measured.uniform_scale, Some(f64::from(scale)));
            assert!(measured.determinant.is_some_and(f64::is_finite));
        }
    }

    #[test]
    fn linear_measurement_pins_pair_normalization_and_public_precedence() {
        let pair_normalized_shear = Mat3::from_cols(
            Vec3::X,
            Vec3::new(3.0e-5, 2.0, 0.0),
            Vec3::new(0.0, 0.0, 3.0),
        );
        let facts = AffineGeometryFacts::from_linear(pair_normalized_shear).unwrap();
        assert!(
            facts.cross_axis_dots[0].abs()
                > LINEAR_CLASSIFICATION_RELATIVE_TOLERANCE
                    * facts.axis_lengths[0]
                    * facts.axis_lengths[1]
        );
        assert!(
            facts.cross_axis_dots[0].abs()
                <= LINEAR_CLASSIFICATION_RELATIVE_TOLERANCE
                    * facts.mean_axis_length
                    * facts.mean_axis_length,
            "measurement intentionally does not use the operation classifier's common-factor band"
        );
        for shear in [3.0e-5, -3.0e-5] {
            let signed_shear = Mat3::from_cols(Vec3::X, Vec3::new(shear, 2.0, 0.0), Vec3::Z);
            assert_eq!(
                measure_linear_transform(Mat4::from_mat3(signed_shear)).classification,
                LinearTransformClassification::Sheared,
                "orthogonality is independent of the dot-product sign"
            );
        }
        for (pair, linear) in [
            (
                "XZ",
                Mat3::from_cols(
                    Vec3::X,
                    Vec3::new(0.0, 100.0, 0.0),
                    Vec3::new(1.5e-5, 0.0, 1.0),
                ),
            ),
            (
                "YZ",
                Mat3::from_cols(
                    Vec3::new(100.0, 0.0, 0.0),
                    Vec3::Y,
                    Vec3::new(0.0, 1.5e-5, 1.0),
                ),
            ),
        ] {
            assert_eq!(
                measure_linear_transform(Mat4::from_mat3(linear)).classification,
                LinearTransformClassification::Sheared,
                "{pair} dot must use that pair's own length product"
            );
        }
        assert_eq!(
            classify_positive_uniform_affine(
                pair_normalized_shear,
                PositiveUniformAffineTolerance {
                    equal_axis: LINEAR_CLASSIFICATION_RELATIVE_TOLERANCE,
                    relative_orthogonality: LINEAR_CLASSIFICATION_RELATIVE_TOLERANCE,
                    singular_determinant_relative: LINEAR_CLASSIFICATION_SINGULAR_TOLERANCE,
                },
            ),
            Err(AffineDomainViolation::NonUniformScale),
            "the positive-uniform operation classifier intentionally rejects shape before shear"
        );

        let singular_reflected_shear = Mat4::from_cols(
            (-Vec3::X).extend(0.0),
            Vec3::new(0.5, 1.0e-8, 0.0).extend(0.0),
            Vec3::Z.extend(0.0),
            glam::Vec4::W,
        );
        let singular = measure_linear_transform(singular_reflected_shear);
        assert_eq!(
            singular.classification,
            LinearTransformClassification::Singular
        );
        assert_eq!(
            singular.orientation,
            Some(LinearTransformOrientation::Zero),
            "singularity owns the public orientation before determinant sign"
        );
        assert!(singular.determinant.is_some_and(|value| value < 0.0));

        let reflected_shear = Mat4::from_cols(
            (-Vec3::X).extend(0.0),
            Vec3::new(0.5, 1.0, 0.0).extend(0.0),
            Vec3::Z.extend(0.0),
            glam::Vec4::W,
        );
        assert_eq!(
            measure_linear_transform(reflected_shear).classification,
            LinearTransformClassification::Reflected
        );
    }

    #[test]
    fn linear_measurement_is_atomic_for_non_finite_mat4_components() {
        for index in 0..16 {
            let mut columns = Mat4::IDENTITY.to_cols_array();
            columns[index] = f32::NAN;
            assert_eq!(
                measure_linear_transform(Mat4::from_cols_array(&columns)),
                unavailable_linear_transform(),
                "component {index} must make every numeric fact unavailable"
            );
        }
    }

    #[test]
    fn linear_measurement_reports_the_canonical_widened_determinant() {
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
        let measured = measure_linear_transform(Mat4::from_mat3(linear));
        assert_eq!(
            measured.determinant.map(f64::to_bits),
            Some(0x3eb4_b98f_a000_0000)
        );
        assert_ne!(measured.determinant, Some(f64::from(linear.determinant())));
    }

    #[test]
    fn skin_bind_summary_covers_every_stable_aggregate_class() {
        let available_joint = |joint_index, matrix| SkinJointMeasurements {
            joint_index,
            node_index: joint_index,
            joint_bind_to_mesh: available_derived_matrix(matrix),
            mesh_bind_world: available_derived_matrix(Mat4::IDENTITY),
        };
        let unavailable_joint = |joint_index| SkinJointMeasurements {
            joint_index,
            node_index: joint_index,
            joint_bind_to_mesh: unavailable_derived_matrix(
                SkinDerivedMatrixUnavailableReason::InverseBindAccessorAbsent,
            ),
            mesh_bind_world: unavailable_derived_matrix(
                SkinDerivedMatrixUnavailableReason::InverseBindAccessorAbsent,
            ),
        };
        let assert_summary = |joints: &[SkinJointMeasurements],
                              classification,
                              available_joint_count,
                              unavailable_joint_count,
                              consistent_uniform_scale| {
            assert_eq!(
                summarize_skin_bind_linear(joints),
                SkinBindLinearSummaryMeasurements {
                    classification,
                    joint_count: joints.len(),
                    available_joint_count,
                    unavailable_joint_count,
                    consistent_uniform_scale,
                }
            );
        };

        assert_summary(
            &[],
            SkinBindLinearSummaryClassification::NoJoints,
            0,
            0,
            None,
        );
        assert_summary(
            &[unavailable_joint(0)],
            SkinBindLinearSummaryClassification::Unavailable,
            0,
            1,
            None,
        );
        assert_summary(
            &[available_joint(0, Mat4::IDENTITY), unavailable_joint(1)],
            SkinBindLinearSummaryClassification::PartiallyUnavailable,
            1,
            1,
            None,
        );
        assert_summary(
            &[
                available_joint(0, Mat4::IDENTITY),
                available_joint(1, Mat4::IDENTITY),
            ],
            SkinBindLinearSummaryClassification::ConsistentUniform,
            2,
            0,
            Some(1.0),
        );
        assert_summary(
            &[
                available_joint(0, Mat4::IDENTITY),
                available_joint(1, Mat4::from_scale(Vec3::splat(2.0))),
            ],
            SkinBindLinearSummaryClassification::MixedUniform,
            2,
            0,
            None,
        );
        assert_summary(
            &[
                available_joint(0, Mat4::from_scale(Vec3::new(1.0, 2.0, 3.0))),
                available_joint(
                    1,
                    Mat4::from_cols_array(&[
                        1.0, 0.0, 0.0, 0.0, 0.5, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0,
                        1.0,
                    ]),
                ),
            ],
            SkinBindLinearSummaryClassification::NonUniformOrSheared,
            2,
            0,
            None,
        );
        assert_summary(
            &[
                available_joint(0, Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0))),
                available_joint(
                    1,
                    Mat4::from_cols_array(&[
                        1.0, 0.0, 0.0, 0.0, 1.0, 1.0e-8, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0,
                        0.0, 1.0,
                    ]),
                ),
            ],
            SkinBindLinearSummaryClassification::ReflectedOrSingular,
            2,
            0,
            None,
        );
        assert_summary(
            &[
                available_joint(0, Mat4::IDENTITY),
                available_joint(1, Mat4::from_scale(Vec3::new(1.0, 2.0, 3.0))),
            ],
            SkinBindLinearSummaryClassification::Mixed,
            2,
            0,
            None,
        );
    }

    #[test]
    fn skin_bind_summary_is_joint_order_invariant_and_reports_the_mean() {
        let matrix_from_bits = |columns: [[u32; 4]; 4]| {
            Mat4::from_cols(
                glam::Vec4::from_array(columns[0].map(f32::from_bits)),
                glam::Vec4::from_array(columns[1].map(f32::from_bits)),
                glam::Vec4::from_array(columns[2].map(f32::from_bits)),
                glam::Vec4::from_array(columns[3].map(f32::from_bits)),
            )
        };
        let raw_inverse_binds = [
            matrix_from_bits([
                [0xbcde_4500, 0xbd7b_2918, 0x3f7f_6c80, 0],
                [0x3f40_907c, 0xbf28_9ba8, 0xbca4_0480, 0],
                [0x3f28_8afa, 0x3f3f_fdef, 0x3d83_0f78, 0],
                [0, 0, 0, 0x3f80_0000],
            ]),
            matrix_from_bits([
                [0x3da5_7c20, 0xbf7e_c9a2, 0xbd5d_55e0, 0],
                [0x3e48_71f6, 0xbd18_d560, 0x3f7a_dda0, 0],
                [0xbf7a_31a0, 0xbdb7_d42c, 0x3e44_6898, 0],
                [0, 0, 0, 0x3f80_0000],
            ]),
            matrix_from_bits([
                [0xbee1_b0e8, 0xbd50_c238, 0xbf65_6a79, 0],
                [0xbf62_2552, 0xbe1c_0be8, 0x3ee2_e94f, 0],
                [0xbe22_f8bc, 0x3f7c_ac66, 0x3cb5_7540, 0],
                [0, 0, 0, 0x3f80_0000],
            ]),
        ];
        let expected_factor_bits = [
            0x3ff0_0000_110e_4203,
            0x3ff0_0000_2d55_0083,
            0x3fef_ffff_b3bb_b2b7,
        ];
        let expected_mean = f64::from_bits(0x3ff0_0000_0815_b3f5);
        let permutations = [
            [0usize, 1usize, 2usize],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ];

        for order in permutations {
            let doc = Document {
                assets: SceneAssets {
                    source_skeleton: SourceSkeletonAssets {
                        coverage: SourceSkeletonCoverage::Complete,
                        nodes: (0..3)
                            .map(|source_node_index| SourceNodeAsset {
                                source_node_index,
                                name: Some(format!("joint_{source_node_index}")),
                                parent_source_node_index: None,
                                scene_root_indices: vec![0],
                                local_rest: SourceNodeLocalRest::Matrix(Mat4::IDENTITY),
                                bone: None,
                            })
                            .collect(),
                        skins: vec![SourceSkinAsset {
                            source_skin_index: 0,
                            name: Some("order_invariant_uniform_bind_scale".into()),
                            skeleton_root_source_node_index: Some(0),
                            joint_source_node_indices: order.to_vec(),
                            inverse_bind_accessor: SourceInverseBindAccessor {
                                status: SourceInverseBindAccessorStatus::Available,
                                declared_count: Some(3),
                                matrices: order.map(|index| raw_inverse_binds[index]).to_vec(),
                            },
                            attachments: Vec::new(),
                        }],
                    },
                    ..SceneAssets::default()
                },
                ..Document::default()
            };

            let measured = measure_assets(&doc);
            let skin = &measured.skins[0];
            assert_eq!(
                skin.joints
                    .iter()
                    .map(|joint| {
                        let linear = joint
                            .joint_bind_to_mesh
                            .linear
                            .expect("finite invertible raw inverse binds are measurable");
                        assert_eq!(
                            linear.classification,
                            LinearTransformClassification::UnitOrthonormal
                        );
                        linear
                            .uniform_scale
                            .expect("uniform joint binds carry their factor")
                            .to_bits()
                    })
                    .collect::<Vec<_>>(),
                order.map(|index| expected_factor_bits[index]).to_vec(),
                "source joint order {order:?}"
            );
            assert_eq!(
                skin.joint_bind_linear_summary,
                SkinBindLinearSummaryMeasurements {
                    classification: SkinBindLinearSummaryClassification::ConsistentUniform,
                    joint_count: 3,
                    available_joint_count: 3,
                    unavailable_joint_count: 0,
                    consistent_uniform_scale: Some(expected_mean),
                },
                "source joint order {order:?}"
            );
        }
        assert_ne!(
            expected_mean, 1.0,
            "the summary reports its mean, not joint 0"
        );
    }

    #[test]
    fn skin_bind_summary_classification_is_mean_relative_in_every_joint_order() {
        let factors = [
            1.0_f32,
            f32::from_bits(0x3f80_004b),
            f32::from_bits(0x3f7f_ff69),
        ];
        let mut sorted_factors = factors.map(f64::from);
        sorted_factors.sort_by(f64::total_cmp);
        let expected_mean = sorted_factors.into_iter().sum::<f64>() / factors.len() as f64;
        let permutations = [
            [0usize, 1usize, 2usize],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ];

        for order in permutations {
            let joints = order.map(|index| SkinJointMeasurements {
                joint_index: index,
                node_index: index,
                joint_bind_to_mesh: available_derived_matrix(Mat4::from_scale(Vec3::splat(
                    factors[index],
                ))),
                mesh_bind_world: available_derived_matrix(Mat4::IDENTITY),
            });
            assert_eq!(
                summarize_skin_bind_linear(&joints),
                SkinBindLinearSummaryMeasurements {
                    classification: SkinBindLinearSummaryClassification::ConsistentUniform,
                    joint_count: 3,
                    available_joint_count: 3,
                    unavailable_joint_count: 0,
                    consistent_uniform_scale: Some(expected_mean),
                },
                "high/low factors straddle the first-joint band in order {order:?}"
            );
        }
    }

    #[test]
    fn source_measurement_reports_disagreeing_uniform_joint_bind_scales() {
        let doc = Document {
            assets: SceneAssets {
                source_skeleton: SourceSkeletonAssets {
                    coverage: SourceSkeletonCoverage::Complete,
                    nodes: (0..2)
                        .map(|source_node_index| SourceNodeAsset {
                            source_node_index,
                            name: Some(format!("joint_{source_node_index}")),
                            parent_source_node_index: None,
                            scene_root_indices: vec![0],
                            local_rest: SourceNodeLocalRest::Matrix(Mat4::IDENTITY),
                            bone: None,
                        })
                        .collect(),
                    skins: vec![SourceSkinAsset {
                        source_skin_index: 0,
                        name: Some("mixed_uniform_bind_scale".into()),
                        skeleton_root_source_node_index: Some(0),
                        joint_source_node_indices: vec![0, 1],
                        inverse_bind_accessor: SourceInverseBindAccessor {
                            status: SourceInverseBindAccessorStatus::Available,
                            declared_count: Some(2),
                            matrices: vec![Mat4::IDENTITY, Mat4::from_scale(Vec3::splat(0.5))],
                        },
                        attachments: Vec::new(),
                    }],
                },
                ..SceneAssets::default()
            },
            ..Document::default()
        };

        let measured = measure_assets(&doc);
        let skin = &measured.skins[0];
        assert_eq!(
            skin.joints
                .iter()
                .map(|joint| {
                    let linear = joint
                        .joint_bind_to_mesh
                        .linear
                        .expect("finite invertible raw inverse binds are measurable");
                    (linear.classification, linear.uniform_scale)
                })
                .collect::<Vec<_>>(),
            vec![
                (LinearTransformClassification::UnitOrthonormal, Some(1.0)),
                (LinearTransformClassification::UniformScaled, Some(2.0)),
            ]
        );
        assert_eq!(
            skin.joint_bind_linear_summary,
            SkinBindLinearSummaryMeasurements {
                classification: SkinBindLinearSummaryClassification::MixedUniform,
                joint_count: 2,
                available_joint_count: 2,
                unavailable_joint_count: 0,
                consistent_uniform_scale: None,
            }
        );
    }

    #[test]
    fn non_finite_source_rest_is_explicit_in_matrix_and_linear_domains() {
        let doc = Document {
            assets: SceneAssets {
                source_skeleton: SourceSkeletonAssets {
                    coverage: SourceSkeletonCoverage::Complete,
                    nodes: vec![SourceNodeAsset {
                        source_node_index: 0,
                        name: None,
                        parent_source_node_index: None,
                        scene_root_indices: Vec::new(),
                        local_rest: SourceNodeLocalRest::Matrix(Mat4::from_cols_array(
                            &[f32::NAN; 16],
                        )),
                        bone: None,
                    }],
                    skins: Vec::new(),
                },
                ..SceneAssets::default()
            },
            ..Document::default()
        };
        let node = &measure_assets(&doc).skeleton_nodes[0];
        assert!(node.rest_world_matrix.is_none());
        assert!(node.rest_world_translation_m.is_none());
        assert_eq!(
            node.rest_world_matrix_unavailable_reason,
            Some(SkeletonRestWorldMatrixUnavailableReason::NonFiniteLocalRest)
        );
        assert_eq!(
            node.rest_world_linear.classification,
            LinearTransformClassification::NonFinite
        );
        assert!(node.rest_world_linear.axis_lengths.is_none());
    }

    #[test]
    fn source_skeleton_measurement_preserves_source_order_and_bind_domains() {
        // Source order deliberately puts the child before its parent. Core FK
        // order remains parent-before-child, so rest-world composition must
        // follow source parent identities rather than array position.
        let skeleton = Skeleton {
            bones: vec![
                Bone {
                    name: "root".into(),
                    parent: None,
                    rest: Transform {
                        translation: Vec3::new(10.0, 0.0, 0.0),
                        ..Transform::IDENTITY
                    },
                    inverse_bind: None,
                },
                Bone {
                    name: "joint".into(),
                    parent: Some(0),
                    rest: Transform {
                        translation: Vec3::new(2.0, 0.0, 0.0),
                        ..Transform::IDENTITY
                    },
                    inverse_bind: None,
                },
                Bone {
                    name: "mesh".into(),
                    parent: Some(0),
                    rest: Transform::IDENTITY,
                    inverse_bind: None,
                },
            ],
        };
        let doc = Document {
            skeleton,
            assets: SceneAssets {
                scenes: vec![SceneAsset {
                    source_scene_index: 4,
                    name: None,
                    roots: vec![0],
                }],
                source_skeleton: SourceSkeletonAssets {
                    coverage: SourceSkeletonCoverage::Complete,
                    nodes: vec![
                        SourceNodeAsset {
                            source_node_index: 0,
                            name: Some("joint".into()),
                            parent_source_node_index: Some(1),
                            scene_root_indices: vec![],
                            local_rest: SourceNodeLocalRest::Trs {
                                translation: Vec3::new(2.0, 0.0, 0.0),
                                rotation: Quat::IDENTITY,
                                scale: Vec3::ONE,
                            },
                            bone: None,
                        },
                        SourceNodeAsset {
                            source_node_index: 1,
                            name: Some("root".into()),
                            parent_source_node_index: None,
                            scene_root_indices: vec![4],
                            local_rest: SourceNodeLocalRest::Trs {
                                translation: Vec3::new(10.0, 0.0, 0.0),
                                rotation: Quat::IDENTITY,
                                scale: Vec3::ONE,
                            },
                            bone: None,
                        },
                        SourceNodeAsset {
                            source_node_index: 2,
                            name: Some("mesh".into()),
                            parent_source_node_index: Some(1),
                            scene_root_indices: vec![],
                            local_rest: SourceNodeLocalRest::Matrix(Mat4::IDENTITY),
                            bone: None,
                        },
                    ],
                    skins: vec![SourceSkinAsset {
                        source_skin_index: 0,
                        name: Some("skin".into()),
                        skeleton_root_source_node_index: Some(1),
                        joint_source_node_indices: vec![0],
                        inverse_bind_accessor: SourceInverseBindAccessor {
                            status: SourceInverseBindAccessorStatus::Available,
                            declared_count: Some(2),
                            matrices: vec![
                                Mat4::from_translation(Vec3::new(-12.0, 0.0, 0.0)),
                                Mat4::IDENTITY,
                            ],
                        },
                        attachments: vec![SourceSkinAttachment {
                            source_node_index: 2,
                            source_mesh_index: Some(7),
                        }],
                    }],
                },
                ..SceneAssets::default()
            },
            ..Document::default()
        };

        let measured = measure_assets(&doc);
        assert_eq!(
            measured.skeleton_source_coverage,
            SourceSkeletonCoverage::Complete
        );
        assert_eq!(
            measured
                .skeleton_nodes
                .iter()
                .map(|node| node.node_index)
                .collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        assert_eq!(measured.skeleton_nodes[0].parent_node_index, Some(1));
        assert_eq!(measured.skeleton_nodes[1].scene_root_indices, vec![4]);
        assert_eq!(
            measured.skeleton_nodes[0]
                .rest_world_matrix
                .expect("finite child rest world")[12],
            12.0
        );
        let skin = &measured.skins[0];
        assert_eq!(skin.skeleton_root_node_index, Some(1));
        assert_eq!(
            skin.inverse_bind_accessor.matrices.len(),
            2,
            "extra raw IBM survives"
        );
        assert_eq!(skin.attachments[0].node_index, 2);
        assert_eq!(skin.attachments[0].mesh_index, Some(7));
        assert_eq!(skin.joints[0].joint_bind_to_mesh.matrix.unwrap()[12], 12.0);
        assert_eq!(
            skin.joints[0].mesh_bind_world.matrix.unwrap(),
            Mat4::IDENTITY.to_cols_array()
        );
    }

    #[test]
    fn count_mismatched_inverse_bind_accessor_keeps_present_slots_and_marks_missing_ones() {
        let doc = Document {
            assets: SceneAssets {
                source_skeleton: SourceSkeletonAssets {
                    coverage: SourceSkeletonCoverage::Complete,
                    nodes: vec![SourceNodeAsset {
                        source_node_index: 0,
                        name: None,
                        parent_source_node_index: None,
                        scene_root_indices: vec![],
                        local_rest: SourceNodeLocalRest::Matrix(Mat4::IDENTITY),
                        bone: None,
                    }],
                    skins: vec![SourceSkinAsset {
                        source_skin_index: 0,
                        name: None,
                        skeleton_root_source_node_index: None,
                        joint_source_node_indices: vec![0, 0],
                        inverse_bind_accessor: SourceInverseBindAccessor {
                            status: SourceInverseBindAccessorStatus::CountMismatch,
                            declared_count: Some(1),
                            matrices: vec![Mat4::IDENTITY],
                        },
                        attachments: vec![],
                    }],
                },
                ..SceneAssets::default()
            },
            ..Document::default()
        };

        let skin = &measure_assets(&doc).skins[0];
        assert_eq!(
            skin.joints[0].joint_bind_to_mesh.matrix,
            Some(Mat4::IDENTITY.to_cols_array())
        );
        assert_eq!(
            skin.joints[1].joint_bind_to_mesh.unavailable_reason,
            Some(SkinDerivedMatrixUnavailableReason::InverseBindAccessorCountMismatch)
        );
        assert_eq!(
            skin.joints[1].mesh_bind_world.unavailable_reason,
            Some(SkinDerivedMatrixUnavailableReason::InverseBindAccessorCountMismatch)
        );
    }

    #[test]
    fn source_skeleton_measurement_preserves_full_matrix_domains() {
        // Literal column-major matrices make this an independent analytic
        // oracle for all diagonal and translation components.
        let doc = Document {
            assets: SceneAssets {
                source_skeleton: SourceSkeletonAssets {
                    coverage: SourceSkeletonCoverage::Complete,
                    nodes: vec![SourceNodeAsset {
                        source_node_index: 0,
                        name: None,
                        parent_source_node_index: None,
                        scene_root_indices: vec![],
                        local_rest: SourceNodeLocalRest::Matrix(Mat4::from_cols_array(&[
                            2.0, 0.0, 0.0, 0.0, 0.0, 3.0, 0.0, 0.0, 0.0, 0.0, 4.0, 0.0, 10.0, 20.0,
                            30.0, 1.0,
                        ])),
                        bone: None,
                    }],
                    skins: vec![SourceSkinAsset {
                        source_skin_index: 0,
                        name: None,
                        skeleton_root_source_node_index: Some(0),
                        joint_source_node_indices: vec![0],
                        inverse_bind_accessor: SourceInverseBindAccessor {
                            status: SourceInverseBindAccessorStatus::Available,
                            declared_count: Some(1),
                            matrices: vec![Mat4::from_cols_array(&[
                                0.5, 0.0, 0.0, 0.0, 0.0, 0.25, 0.0, 0.0, 0.0, 0.0, 2.0, 0.0, 1.0,
                                2.0, 3.0, 1.0,
                            ])],
                        },
                        attachments: vec![],
                    }],
                },
                ..SceneAssets::default()
            },
            ..Document::default()
        };

        let joint = &measure_assets(&doc).skins[0].joints[0];
        assert_eq!(
            joint.joint_bind_to_mesh.matrix,
            Some([
                2.0, 0.0, 0.0, 0.0, 0.0, 4.0, 0.0, 0.0, 0.0, 0.0, 0.5, 0.0, -2.0, -8.0, -1.5, 1.0,
            ])
        );
        assert_eq!(
            joint.mesh_bind_world.matrix,
            Some([
                1.0, 0.0, 0.0, 0.0, 0.0, 0.75, 0.0, 0.0, 0.0, 0.0, 8.0, 0.0, 12.0, 26.0, 42.0, 1.0,
            ])
        );
    }

    #[test]
    fn source_skeleton_measurement_handles_a_deep_leaf_first_hierarchy() {
        const NODE_COUNT: usize = 16_384;
        let nodes = (0..NODE_COUNT)
            .map(|node_index| SourceNodeAsset {
                source_node_index: node_index,
                name: None,
                parent_source_node_index: (node_index + 1 < NODE_COUNT).then_some(node_index + 1),
                scene_root_indices: Vec::new(),
                local_rest: SourceNodeLocalRest::Matrix(if node_index + 1 == NODE_COUNT {
                    Mat4::from_translation(Vec3::X)
                } else {
                    Mat4::IDENTITY
                }),
                bone: None,
            })
            .collect();
        let doc = Document {
            assets: SceneAssets {
                source_skeleton: SourceSkeletonAssets {
                    coverage: SourceSkeletonCoverage::Complete,
                    nodes,
                    skins: Vec::new(),
                },
                ..SceneAssets::default()
            },
            ..Document::default()
        };

        let measured = measure_assets(&doc);
        assert_eq!(measured.skeleton_nodes.len(), NODE_COUNT);
        assert_eq!(
            measured.skeleton_nodes[0]
                .rest_world_matrix
                .expect("deep leaf rest world")[12],
            1.0
        );
    }

    #[test]
    fn malformed_source_parent_graph_downgrades_source_coverage() {
        for parent_source_node_index in [Some(7), Some(0)] {
            let doc = Document {
                assets: SceneAssets {
                    source_skeleton: SourceSkeletonAssets {
                        coverage: SourceSkeletonCoverage::Complete,
                        nodes: vec![SourceNodeAsset {
                            source_node_index: 0,
                            name: None,
                            parent_source_node_index,
                            scene_root_indices: Vec::new(),
                            local_rest: SourceNodeLocalRest::Matrix(Mat4::IDENTITY),
                            bone: None,
                        }],
                        skins: Vec::new(),
                    },
                    ..SceneAssets::default()
                },
                ..Document::default()
            };

            let measured = measure_assets(&doc);
            assert_eq!(
                measured.skeleton_source_coverage,
                SourceSkeletonCoverage::Unavailable
            );
            assert!(measured.skeleton_nodes.is_empty());
            assert!(measured.skins.is_empty());
        }
    }

    #[test]
    fn skinned_mesh_measures_bbox_joints_and_weight_sums() {
        // Four positions with an analytic AABB of (0,0,0)..(2,3,4).
        let prim = Primitive {
            positions: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(2.0, 0.0, 0.0),
                Vec3::new(0.0, 3.0, 0.0),
                Vec3::new(0.0, 0.0, 4.0),
            ],
            // Influence counts 1, 2, 3, 3 → max 3; weight sums 1.0, 1.0,
            // 1.0, 0.9 → min 0.9, max 1.0.
            weights: vec![
                [1.0, 0.0, 0.0, 0.0],
                [0.5, 0.5, 0.0, 0.0],
                [0.4, 0.3, 0.3, 0.0],
                [0.3, 0.3, 0.3, 0.0],
            ],
            joints: vec![[0, 0, 0, 0]; 4],
            ..Primitive::default()
        };
        let m = mesh("body", vec![prim]);

        assert_eq!(m.name, "body");
        assert_eq!(m.vertex_count, 4);
        let aabb = m.geometry_aabb.as_ref().expect("positions present");
        assert_eq!(aabb.min, [0.0, 0.0, 0.0]);
        assert_eq!(aabb.max, [2.0, 3.0, 4.0]);
        assert_eq!(m.geometry_centroid, Some([0.5, 0.75, 1.0]));
        assert_eq!(m.max_joints_per_vertex, 3);
        // f32 weights summed in f64 carry rounding; compare with tolerance.
        assert!((m.weight_sum_min.unwrap() - 0.9).abs() < 1e-6);
        assert!((m.weight_sum_max.unwrap() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn mesh_measurements_preserve_secondary_influence_set_mismatches_without_affecting_primary_stats()
     {
        let primary = Primitive {
            positions: vec![Vec3::ZERO],
            joints: vec![[0, 1, 0, 0]],
            weights: vec![[0.75, 0.25, 0.0, 0.0]],
            additional_influence_sets: vec![AdditionalInfluenceSet {
                set_index: 2,
                joints_present: true,
                weights_present: false,
            }],
            ..Primitive::default()
        };
        let secondary = Primitive {
            positions: vec![Vec3::ONE],
            additional_influence_sets: vec![
                AdditionalInfluenceSet {
                    set_index: 1,
                    joints_present: false,
                    weights_present: true,
                },
                AdditionalInfluenceSet {
                    set_index: 2,
                    joints_present: false,
                    weights_present: true,
                },
            ],
            ..Primitive::default()
        };

        let measured = mesh("body", vec![primary, secondary]);

        assert_eq!(measured.max_joints_per_vertex, 2);
        assert_eq!(measured.weight_sum_min, Some(1.0));
        assert_eq!(measured.weight_sum_max, Some(1.0));
        assert_eq!(
            measured.additional_influence_sets,
            vec![
                AdditionalInfluenceSetMeasurements {
                    set_index: 1,
                    joints_present: false,
                    weights_present: true,
                    joints_without_weights_present: false,
                    weights_without_joints_present: true,
                },
                AdditionalInfluenceSetMeasurements {
                    set_index: 2,
                    joints_present: true,
                    weights_present: true,
                    joints_without_weights_present: true,
                    weights_without_joints_present: true,
                },
            ]
        );
    }

    #[test]
    fn unskinned_mesh_has_bbox_but_no_weight_stats() {
        let prim = Primitive {
            positions: vec![Vec3::new(-1.0, -2.0, -3.0), Vec3::new(1.0, 2.0, 3.0)],
            ..Primitive::default()
        };
        let m = mesh("prop", vec![prim]);

        assert_eq!(m.vertex_count, 2);
        assert_eq!(m.geometry_aabb.as_ref().unwrap().min, [-1.0, -2.0, -3.0]);
        assert_eq!(m.geometry_centroid, Some([0.0, 0.0, 0.0]));
        assert_eq!(m.max_joints_per_vertex, 0);
        assert_eq!(m.weight_sum_min, None, "no skin ⇒ no weight-sum");
        assert_eq!(m.weight_sum_max, None);
    }

    #[test]
    fn empty_mesh_reports_no_bbox() {
        let m = mesh("hollow", vec![Primitive::default()]);
        assert_eq!(m.vertex_count, 0);
        assert!(m.geometry_aabb.is_none(), "no positions ⇒ no bounding box");
        assert!(m.geometry_centroid.is_none(), "no positions ⇒ no centroid");
    }

    #[test]
    fn non_finite_position_is_dropped_from_the_bbox() {
        // A vertex with any non-finite coordinate is garbage geometry:
        // it is dropped whole (not folded per-axis), so the box stays
        // the finite extent — and never emits a non-finite bound.
        let prim = Primitive {
            positions: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(f32::NAN, 5.0, 0.0),
                Vec3::new(f32::INFINITY, 9.0, 0.0),
                Vec3::new(2.0, 3.0, 0.0),
            ],
            ..Primitive::default()
        };
        let m = mesh("nan", vec![prim]);
        let aabb = m.geometry_aabb.as_ref().unwrap();
        // Only the two finite vertices contribute; the NaN/Inf rows drop
        // out, so their 5.0 / 9.0 do NOT reach the box.
        assert_eq!(aabb.min, [0.0, 0.0, 0.0]);
        assert_eq!(aabb.max, [2.0, 3.0, 0.0]);
        assert_eq!(m.geometry_centroid, Some([1.0, 1.5, 0.0]));
        assert!(
            aabb.min.iter().chain(&aabb.max).all(|c| c.is_finite()),
            "no non-finite bound is ever emitted"
        );
    }

    #[test]
    fn all_non_finite_positions_yield_no_bbox() {
        // Every vertex non-finite ⇒ no finite contribution ⇒ `aabb` is
        // omitted, not an inf/-inf box that serializes to JSON `null`.
        let prim = Primitive {
            positions: vec![Vec3::splat(f32::NAN), Vec3::splat(f32::INFINITY)],
            ..Primitive::default()
        };
        let m = mesh("allnan", vec![prim]);
        assert_eq!(m.vertex_count, 2, "count still reflects the vertices");
        assert!(
            m.geometry_aabb.is_none(),
            "no finite vertex ⇒ no box (never null bounds)"
        );
        assert!(
            m.geometry_centroid.is_none(),
            "no finite vertex ⇒ no centroid"
        );
    }

    #[test]
    fn non_finite_weight_sum_is_omitted() {
        // A NaN weight makes its sum non-finite; it must not surface as a
        // JSON-null weight-sum bound.
        let prim = Primitive {
            positions: vec![Vec3::ZERO, Vec3::ONE],
            weights: vec![[0.5, 0.5, 0.0, 0.0], [f32::NAN, 0.0, 0.0, 0.0]],
            ..Primitive::default()
        };
        let m = mesh("nanw", vec![prim]);
        // The one finite sum (1.0) is kept; the NaN sum is skipped.
        assert_eq!(m.weight_sum_min, Some(1.0));
        assert_eq!(m.weight_sum_max, Some(1.0));
    }

    #[test]
    fn all_non_finite_weight_sums_yield_no_weight_stats() {
        // Every weight sum non-finite ⇒ no finite contribution ⇒ both
        // bounds omitted, not an inf/-inf pair that serializes to `null`.
        let prim = Primitive {
            positions: vec![Vec3::ZERO, Vec3::ONE],
            weights: vec![[f32::NAN, 0.0, 0.0, 0.0], [f32::INFINITY, 0.0, 0.0, 0.0]],
            ..Primitive::default()
        };
        let m = mesh("allnanw", vec![prim]);
        assert_eq!(m.weight_sum_min, None, "no finite weight sum ⇒ omitted");
        assert_eq!(m.weight_sum_max, None);
        // max_joints_per_vertex still counts the non-zero influences.
        assert_eq!(m.max_joints_per_vertex, 1);
    }

    #[test]
    fn vertex_count_sums_across_primitives() {
        let a = Primitive {
            positions: vec![Vec3::ZERO; 3],
            ..Primitive::default()
        };
        let b = Primitive {
            positions: vec![Vec3::ONE; 5],
            ..Primitive::default()
        };
        let m = mesh("multi", vec![a, b]);
        assert_eq!(m.vertex_count, 8, "3 + 5 corners across two primitives");
    }

    #[test]
    fn geometry_centroid_is_the_finite_position_mean_across_primitives() {
        // The centroid is intentionally not the centre of the AABB: the third
        // finite vertex is duplicated in a separate primitive. Positions, not
        // triangle indices, are the existing vertex_count/AABB domain.
        let indexed = Primitive {
            positions: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(6.0, 0.0, 0.0),
                Vec3::new(0.0, 3.0, 0.0),
            ],
            indices: vec![0, 1, 2, 0, 1, 2],
            ..Primitive::default()
        };
        let unindexed = Primitive {
            positions: vec![Vec3::new(0.0, 3.0, 0.0), Vec3::splat(f32::NAN)],
            ..Primitive::default()
        };
        let m = mesh("asymmetric", vec![indexed, unindexed]);

        assert_eq!(m.vertex_count, 5, "all authored position rows count");
        assert_eq!(m.geometry_aabb.unwrap().max, [6.0, 3.0, 0.0]);
        assert_eq!(
            m.geometry_centroid,
            Some([1.5, 1.5, 0.0]),
            "four finite position rows, independent of six index references"
        );
    }

    #[test]
    fn non_finite_instance_transform_makes_scene_coverage_partial() {
        let doc = Document {
            skeleton: Skeleton {
                bones: vec![
                    Bone {
                        name: "finite".into(),
                        parent: None,
                        rest: Transform::IDENTITY,
                        inverse_bind: None,
                    },
                    Bone {
                        name: "overflow".into(),
                        parent: Some(0),
                        rest: Transform {
                            scale: Vec3::splat(f32::MAX),
                            ..Transform::IDENTITY
                        },
                        inverse_bind: None,
                    },
                ],
            },
            assets: SceneAssets {
                meshes: vec![MeshAsset {
                    name: "point".into(),
                    source_mesh_index: 4,
                    primitives: vec![Primitive {
                        positions: vec![Vec3::new(2.0, 0.0, 0.0)],
                        ..Primitive::default()
                    }],
                }],
                instances: vec![
                    crate::model::MeshInstance {
                        source_node_index: 10,
                        node: 0,
                        mesh: 0,
                        ..crate::model::MeshInstance::default()
                    },
                    crate::model::MeshInstance {
                        source_node_index: 11,
                        node: 1,
                        mesh: 0,
                        ..crate::model::MeshInstance::default()
                    },
                ],
                scenes: vec![crate::model::SceneAsset {
                    source_scene_index: 3,
                    name: Some("partial".into()),
                    roots: vec![0],
                }],
                default_scene: None,
                ..SceneAssets::default()
            },
            ..Document::default()
        };

        let measured = measure_assets(&doc);
        assert_eq!(measured.default_scene_index, None, "no implicit scene zero");
        assert_eq!(measured.node_instances.len(), 2);
        assert_eq!(
            measured.node_instances[0].static_node_world_aabb,
            Some(Aabb {
                min: [2.0, 0.0, 0.0],
                max: [2.0, 0.0, 0.0],
            })
        );
        assert_eq!(
            measured.node_instances[1].static_node_world_aabb_unavailable_reason,
            Some(StaticNodeAabbUnavailableReason::NonFiniteTransform)
        );
        assert_eq!(measured.scenes[0].instance_count, 2);
        assert_eq!(measured.scenes[0].excluded_instance_count, 1);
        assert_eq!(
            measured.scenes[0].static_scene_world_aabb,
            measured.node_instances[0].static_node_world_aabb,
            "partial aggregate retains the finite instance"
        );
    }

    #[test]
    fn malformed_skeleton_chain_does_not_hide_an_unrelated_instance() {
        let doc = Document {
            skeleton: Skeleton {
                bones: vec![
                    Bone {
                        name: "malformed".into(),
                        parent: Some(1),
                        rest: Transform::IDENTITY,
                        inverse_bind: None,
                    },
                    Bone {
                        name: "malformed_child".into(),
                        parent: Some(0),
                        rest: Transform::IDENTITY,
                        inverse_bind: None,
                    },
                    Bone {
                        name: "valid_root".into(),
                        parent: None,
                        rest: Transform {
                            translation: Vec3::X,
                            ..Transform::IDENTITY
                        },
                        inverse_bind: None,
                    },
                    Bone {
                        name: "valid_instance".into(),
                        parent: Some(2),
                        rest: Transform {
                            translation: Vec3::Y,
                            ..Transform::IDENTITY
                        },
                        inverse_bind: None,
                    },
                ],
            },
            assets: SceneAssets {
                meshes: vec![MeshAsset {
                    name: "point".into(),
                    source_mesh_index: 0,
                    primitives: vec![Primitive {
                        positions: vec![Vec3::X],
                        ..Primitive::default()
                    }],
                }],
                instances: vec![
                    crate::model::MeshInstance {
                        source_node_index: 10,
                        node: 0,
                        mesh: 0,
                        ..crate::model::MeshInstance::default()
                    },
                    crate::model::MeshInstance {
                        source_node_index: 11,
                        node: 3,
                        mesh: 0,
                        ..crate::model::MeshInstance::default()
                    },
                ],
                scenes: vec![SceneAsset {
                    source_scene_index: 0,
                    name: None,
                    roots: vec![0, 2],
                }],
                ..SceneAssets::default()
            },
            ..Document::default()
        };

        let measured = measure_assets(&doc);
        assert_eq!(
            measured.node_instances[0].static_node_world_aabb_unavailable_reason,
            Some(StaticNodeAabbUnavailableReason::NonFiniteTransform)
        );
        assert_eq!(
            measured.node_instances[1].static_node_world_aabb,
            Some(Aabb {
                min: [2.0, 1.0, 0.0],
                max: [2.0, 1.0, 0.0],
            })
        );
        assert_eq!(measured.scenes[0].excluded_instance_count, 1);
        assert_eq!(
            measured.scenes[0].static_scene_world_aabb,
            measured.node_instances[1].static_node_world_aabb
        );
    }

    #[test]
    fn later_duplicate_clip_name_replaces_earlier_measurement() {
        let earlier = Clip {
            name: "duplicate".into(),
            duration_s: 1.0,
            tracks: vec![
                Track {
                    bone: 0,
                    property: Property::Rotation,
                    interpolation: Interpolation::Linear,
                    times: vec![0.0, 0.5, 1.0],
                    values: TrackValues::Quats(vec![
                        Quat::IDENTITY,
                        Quat::from_rotation_x(0.25),
                        Quat::from_rotation_x(0.5),
                    ]),
                },
                Track {
                    bone: 0,
                    property: Property::Translation,
                    interpolation: Interpolation::Linear,
                    times: vec![0.0, 0.5, 1.0],
                    values: TrackValues::Vec3s(vec![Vec3::ZERO, Vec3::Z * 0.5, Vec3::Z]),
                },
                Track {
                    bone: 1,
                    property: Property::Translation,
                    interpolation: Interpolation::Linear,
                    times: vec![0.0, 0.5, 1.0],
                    values: TrackValues::Vec3s(vec![
                        Vec3::new(-0.1, -1.0, 0.0),
                        Vec3::new(-0.1, -0.9, 0.15),
                        Vec3::new(-0.1, -1.0, 0.0),
                    ]),
                },
                Track {
                    bone: 2,
                    property: Property::Translation,
                    interpolation: Interpolation::Linear,
                    times: vec![0.0, 0.5, 1.0],
                    values: TrackValues::Vec3s(vec![
                        Vec3::new(0.1, -1.0, 0.0),
                        Vec3::new(0.1, -1.1, -0.15),
                        Vec3::new(0.1, -1.0, 0.0),
                    ]),
                },
            ],
        };
        let later = Clip {
            name: "duplicate".into(),
            duration_s: 2.0,
            tracks: vec![Track {
                bone: 0,
                property: Property::Translation,
                interpolation: Interpolation::Linear,
                times: vec![0.0, 2.0],
                values: TrackValues::Vec3s(vec![Vec3::ZERO, Vec3::X]),
            }],
        };
        let skeleton = Skeleton {
            bones: vec![
                Bone {
                    name: "hips".into(),
                    parent: None,
                    rest: Transform::IDENTITY,
                    inverse_bind: None,
                },
                Bone {
                    name: "left_foot".into(),
                    parent: Some(0),
                    rest: Transform::IDENTITY,
                    inverse_bind: None,
                },
                Bone {
                    name: "right_foot".into(),
                    parent: Some(0),
                    rest: Transform::IDENTITY,
                    inverse_bind: None,
                },
            ],
        };
        let roles = ResolvedRoles::from_names(
            &skeleton,
            [
                (Role::Hips, "hips".into()),
                (Role::LeftFoot, "left_foot".into()),
                (Role::RightFoot, "right_foot".into()),
            ],
        );
        let earlier_doc = Document {
            skeleton: skeleton.clone(),
            clips: vec![earlier.clone()],
            ..Document::default()
        };
        let earlier_grids = MetricGrids::new(&earlier_doc);
        let earlier_measurement =
            &measure_document(&earlier_grids, &roles, &Config::default())["duplicate"];
        assert!(earlier_measurement.loop_seam_ratio.is_some());
        assert!(earlier_measurement.gait.is_some());
        assert!(earlier_measurement.speed_mps.is_some());

        let doc = Document {
            skeleton,
            clips: vec![earlier, later],
            ..Document::default()
        };
        let grids = MetricGrids::new(&doc);
        let measurements = measure_document(&grids, &roles, &Config::default());

        assert_eq!(
            serde_json::to_value(measurements).expect("duplicate measurements serialize"),
            serde_json::json!({
                "duplicate": {
                    "duration_s": 2.0,
                    "frame_count": 2,
                    "animated_bones": ["hips"],
                    "bone_rotation_range_deg": {},
                }
            })
        );
    }
}
