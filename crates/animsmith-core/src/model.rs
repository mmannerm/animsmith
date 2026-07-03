//! The raw layer: clips, tracks, and the skeleton exactly as the source
//! file authored them. Mechanical checks (NaN, quaternion flips, key
//! density, …) read this; semantic checks read the sampled layer built
//! from it (see [`crate::sample`]).

use glam::{Mat4, Quat, Vec3};

/// Index into [`Skeleton::bones`].
pub type BoneId = usize;

/// Node-local TRS transform.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Transform {
    pub const IDENTITY: Self = Self {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    };

    pub fn to_mat4(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

#[derive(Debug, Clone)]
pub struct Bone {
    pub name: String,
    pub parent: Option<BoneId>,
    /// Rest pose, node-local. Whether this or the inverse-bind-derived
    /// rest is authoritative is a check's concern (`bind-pose`, P1).
    pub rest: Transform,
    /// Inverse bind matrix from a skin, when one references this bone.
    pub inverse_bind: Option<Mat4>,
}

/// Bones in topological order: a bone's parent always precedes it.
/// Loaders are responsible for establishing this invariant.
#[derive(Debug, Clone, Default)]
pub struct Skeleton {
    pub bones: Vec<Bone>,
}

impl Skeleton {
    pub fn bone_name(&self, id: BoneId) -> &str {
        &self.bones[id].name
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Property {
    Translation,
    Rotation,
    Scale,
}

impl Property {
    pub fn as_str(self) -> &'static str {
        match self {
            Property::Translation => "translation",
            Property::Rotation => "rotation",
            Property::Scale => "scale",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interpolation {
    Linear,
    Step,
    /// glTF cubic spline: `values` holds `[in-tangent, value, out-tangent]`
    /// triplets per keyframe. Use [`Track::value_index`] to address the
    /// value elements.
    CubicSpline,
}

#[derive(Debug, Clone)]
pub enum TrackValues {
    Vec3s(Vec<Vec3>),
    Quats(Vec<Quat>),
}

impl TrackValues {
    pub fn len(&self) -> usize {
        match self {
            TrackValues::Vec3s(v) => v.len(),
            TrackValues::Quats(v) => v.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// One animated property of one bone.
#[derive(Debug, Clone)]
pub struct Track {
    pub bone: BoneId,
    pub property: Property,
    pub interpolation: Interpolation,
    /// Keyframe times in seconds. Same length as the keyframe count
    /// (tangent elements in cubic tracks do not add times).
    pub times: Vec<f32>,
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

    pub fn start_time(&self) -> f32 {
        self.times.first().copied().unwrap_or(0.0)
    }

    pub fn end_time(&self) -> f32 {
        self.times.last().copied().unwrap_or(0.0)
    }
}

#[derive(Debug, Clone)]
pub struct Clip {
    pub name: String,
    /// Clip length in seconds (max sampler end time across tracks).
    pub duration_s: f64,
    pub tracks: Vec<Track>,
}

#[derive(Debug, Clone, Default)]
pub struct SourceInfo {
    pub path: Option<String>,
    pub format: Option<String>,
}

/// A loaded file: one skeleton, any number of clips targeting it.
#[derive(Debug, Clone, Default)]
pub struct Document {
    pub skeleton: Skeleton,
    pub clips: Vec<Clip>,
    pub source: SourceInfo,
}

// --- Scene assets (meshes/materials) -----------------------------------
//
// Carried alongside a Document by `convert` so a full FBX→glTF
// conversion can preserve geometry. Deliberately NOT part of Document:
// the check catalog judges animation, and every existing consumer stays
// untouched. Vertex data is unindexed (one entry per triangle corner);
// a welding/indexing pass is a future size optimization.

/// One glTF-primitive-to-be: unindexed triangles sharing a material.
#[derive(Debug, Clone, Default)]
pub struct Primitive {
    /// Index into [`SceneAssets::materials`].
    pub material: Option<usize>,
    pub positions: Vec<Vec3>,
    /// Same length as `positions`, or empty.
    pub normals: Vec<Vec3>,
    /// Same length as `positions`, or empty.
    pub uvs: Vec<[f32; 2]>,
    /// Indices into the owning mesh's `skin_joints`; empty if unskinned.
    pub joints: Vec<[u16; 4]>,
    pub weights: Vec<[f32; 4]>,
}

#[derive(Debug, Clone, Default)]
pub struct MeshAsset {
    pub name: String,
    /// The node this mesh hangs off.
    pub node: BoneId,
    pub primitives: Vec<Primitive>,
    /// Skin joints in cluster order. Empty = unskinned.
    pub skin_joints: Vec<BoneId>,
    /// Per-joint inverse bind matrices, parallel to `skin_joints`
    /// (glTF convention: joint-bind-world⁻¹ × geometry-to-world, all
    /// in the converted scene space). Falls back to the bones'
    /// `inverse_bind` when empty.
    pub skin_ibms: Vec<Mat4>,
}

/// Factor-only material (textures are wired by downstream pipelines).
#[derive(Debug, Clone)]
pub struct MaterialAsset {
    pub name: String,
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
}

#[derive(Debug, Clone, Default)]
pub struct SceneAssets {
    pub meshes: Vec<MeshAsset>,
    pub materials: Vec<MaterialAsset>,
}
