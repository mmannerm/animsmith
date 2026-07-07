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
    /// rest is authoritative is a check's concern (`bind-pose`, P1).
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
/// the scene assets (meshes/materials) that rode in alongside them.
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

// --- Scene assets (meshes/materials) -----------------------------------
//
// The geometry half of a [`Document`]. Populated by loaders that ingest
// meshes (FBX today; glTF is #16) and emitted by the writer, so a full
// conversion preserves geometry. Vertex data is unindexed (one entry
// per triangle corner); a welding/indexing pass is a future size
// optimization.

/// One glTF-primitive-to-be: triangles sharing a material. Attributes
/// are per corner until [`Primitive::weld`] dedupes them into indexed
/// form.
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
    /// Indices into the owning mesh's `skin_joints`; empty if unskinned.
    pub joints: Vec<[u16; 4]>,
    /// Skinning weights parallel to [`Primitive::joints`].
    pub weights: Vec<[f32; 4]>,
}

/// Mesh data attached to a scene node.
#[derive(Debug, Clone, Default)]
pub struct MeshAsset {
    /// Mesh name.
    pub name: String,
    /// The node this mesh hangs off.
    pub node: BoneId,
    /// Triangle-list primitives belonging to this mesh.
    pub primitives: Vec<Primitive>,
    /// Skin joints in cluster order. Empty = unskinned.
    pub skin_joints: Vec<BoneId>,
    /// Per-joint inverse bind matrices, parallel to `skin_joints`
    /// (glTF convention: joint-bind-world⁻¹ × geometry-to-world, all
    /// in the converted scene space). Falls back to the bones'
    /// `inverse_bind` when empty.
    pub skin_ibms: Vec<Mat4>,
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

/// Factor-only material plus an optional embedded base-color texture.
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

/// Mesh and material assets carried alongside animation data.
#[derive(Debug, Clone, Default)]
pub struct SceneAssets {
    /// Meshes in document order.
    pub meshes: Vec<MeshAsset>,
    /// Materials referenced by mesh primitives.
    pub materials: Vec<MaterialAsset>,
}
