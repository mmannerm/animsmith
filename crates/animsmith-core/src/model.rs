//! The loader-facing layer: clips, tracks, and the skeleton before metric
//! resampling or repair. The glTF loader preserves authored animation
//! values; the FBX loader normalizes scene coordinates and bakes takes to
//! linear TRS tracks. Mechanical checks (NaN, quaternion flips, key
//! density, …) read this layer; semantic checks read the sampled layer
//! built from it (see [`crate::sample`]).

use glam::{Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};

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
    /// Every source material, texture, and image was inspected.
    Complete,
    /// The loader cannot provide source resource evidence.
    #[default]
    Unavailable,
}

/// A material texture slot with stable source-format meaning.
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
