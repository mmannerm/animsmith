//! Minimal glTF 2.0 writer for `convert`/`transform`: emits the
//! skeleton (node hierarchy + rest TRS), each clip's writable animation tracks,
//! and whatever scene assets the [`Document`] carries ([`Document::assets`]
//! — triangulated meshes, skins, factor-only materials, and embedded
//! base-color and normal textures). A document with default-empty assets writes
//! animation + skeleton only, so
//! animation data can still enter glTF-based tooling (including animsmith
//! itself) straight from a DCC export.
//!
//! Values are written exactly as held in the core model — lint first;
//! conversion does not repair.

use crate::WriteError;
use animsmith_core::model::{
    Document, Interpolation, MaterialResourceCoverage, Property, SourceInverseBindAccessorStatus,
    SourceNodeLocalRest, SourceSkeletonCoverage, TrackValues, validate_document_shape,
};
use base64::Engine as _;
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use std::path::Path;

/// Inclusive byte ceilings for the strict in-memory GLB writer.
///
/// Foot-cycle V1 callers should select [`Self::FOOT_CYCLE_V1`] explicitly. A
/// caller preparing a larger artifact must make that authority explicit and
/// still remain within the glTF container's `u32` length fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlbWriteLimits {
    /// Maximum padded JSON chunk bytes.
    pub max_json_bytes: usize,
    /// Maximum padded binary chunk bytes.
    pub max_bin_bytes: usize,
    /// Maximum complete GLB bytes, including header and chunk framing.
    pub max_total_bytes: usize,
    /// Maximum normalized JSON rows and bounded validation rows considered by
    /// strict preflight before it constructs a writer-owned JSON value.
    pub max_structural_rows: usize,
    /// Maximum UTF-8 bytes retained in strict generated JSON names.
    pub max_name_bytes: usize,
    /// Maximum aggregate strict traversal work across the admitted model's
    /// source/output rows, binary components, sidecar lookups, and copied
    /// texture bytes.
    pub max_work: usize,
}

impl GlbWriteLimits {
    /// Conservative 256 MiB inclusive ceiling for each candidate component
    /// and for the framed in-memory GLB.
    pub const FOOT_CYCLE_V1: Self = Self {
        max_json_bytes: 256 * 1024 * 1024,
        max_bin_bytes: 256 * 1024 * 1024,
        max_total_bytes: 256 * 1024 * 1024,
        max_structural_rows: 1_000_000,
        max_name_bytes: 1_048_576,
        max_work: 16_000_000,
    };
}

/// Immutable, exact result of [`preflight_glb_bytes`].
///
/// A receipt binds the document's strict projection and its byte counts to one
/// limit set. [`write_glb_bytes`] repeats the projection and refuses if any
/// count changed, so a mutable core [`Document`] cannot use a stale approval.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlbWritePreflight {
    summary: WriteSummary,
    json_bytes: usize,
    bin_bytes: usize,
    total_bytes: usize,
    limits: GlbWriteLimits,
    policy: GlbProjectionPolicyV1,
    json_digest: [u8; 32],
    bin_digest: [u8; 32],
}

impl GlbWritePreflight {
    /// Generated glTF summary for the strict projection.
    pub fn summary(&self) -> WriteSummary {
        self.summary
    }

    /// Exact padded JSON chunk byte count.
    pub fn json_bytes(&self) -> usize {
        self.json_bytes
    }

    /// Exact padded binary chunk byte count.
    pub fn bin_bytes(&self) -> usize {
        self.bin_bytes
    }

    /// Exact complete GLB byte count, including framing.
    pub fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    /// Fixed GLB header size included in [`Self::total_bytes`].
    pub const fn header_bytes(&self) -> usize {
        12
    }

    /// Limits against which this receipt was admitted.
    pub fn limits(&self) -> GlbWriteLimits {
        self.limits
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BinaryMode {
    Count,
    Retain,
}

/// Counts of the scene data emitted by [`write()`].
///
/// The first five fields describe the generated glTF, which can differ from the
/// input [`Document`] when an animation clip has no writable channels, a legacy
/// skinned mesh requires an additional holder node, or materials are omitted
/// because the document has no meshes. [`Self::clips_without_writable_tracks`]
/// is intentionally an input-to-output delta rather than an artifact count so
/// callers can explain omitted source clips.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct WriteSummary {
    /// Number of nodes emitted in the glTF skeleton/scene graph.
    pub nodes: usize,
    /// Number of animations emitted.
    pub animations: usize,
    /// Number of meshes emitted.
    pub meshes: usize,
    /// Number of primitive positions emitted.
    pub primitive_positions: usize,
    /// Number of materials emitted.
    pub materials: usize,
    /// Number of input clips omitted because none of their tracks were writable.
    pub clips_without_writable_tracks: usize,
}

struct BufferBuilder {
    bytes: Option<Vec<u8>>,
    byte_len: usize,
    digest: Sha256,
    views: Vec<Value>,
    accessors: Vec<Value>,
}

impl BufferBuilder {
    fn new(mode: BinaryMode, reserve: usize) -> Result<Self, WriteError> {
        let bytes = match mode {
            BinaryMode::Count => None,
            BinaryMode::Retain => {
                let mut bytes = Vec::new();
                bytes
                    .try_reserve_exact(reserve)
                    .map_err(|_| WriteError::Allocation {
                        field: "BIN chunk",
                        bytes: reserve,
                    })?;
                Some(bytes)
            }
        };
        Ok(Self {
            bytes,
            byte_len: 0,
            digest: Sha256::new(),
            views: Vec::new(),
            accessors: Vec::new(),
        })
    }

    fn len(&self) -> usize {
        self.byte_len
    }

    fn append(&mut self, bytes: &[u8]) -> Result<(), WriteError> {
        self.byte_len = self
            .byte_len
            .checked_add(bytes.len())
            .ok_or(WriteError::TooLarge {
                field: "BIN chunk",
                bytes: usize::MAX,
            })?;
        if let Some(retained) = &mut self.bytes {
            retained.extend_from_slice(bytes);
        }
        self.digest.update(bytes);
        Ok(())
    }

    fn push_padding(&mut self) -> Result<(), WriteError> {
        while !self.byte_len.is_multiple_of(4) {
            self.append(&[0])?;
        }
        Ok(())
    }

    fn into_bytes(self) -> Result<Vec<u8>, WriteError> {
        self.bytes.ok_or(WriteError::Refused(
            "internal count projection cannot emit bytes".to_owned(),
        ))
    }

    fn digest(&self) -> [u8; 32] {
        self.digest.clone().finalize().into()
    }

    /// Append `data` as a buffer view + accessor; returns the accessor
    /// index. `kind` is "SCALAR" | "VEC3" | "VEC4"; floats only.
    fn push_f32<I>(
        &mut self,
        len: usize,
        data: I,
        kind: &str,
        with_min_max: bool,
    ) -> Result<usize, WriteError>
    where
        I: IntoIterator<Item = f32>,
    {
        let components = match kind {
            "SCALAR" => 1,
            "VEC2" => 2,
            "VEC3" => 3,
            "MAT4" => 16,
            _ => 4,
        };
        let offset = self.len();
        let byte_len = len.checked_mul(4).ok_or(WriteError::TooLarge {
            field: "BIN chunk",
            bytes: usize::MAX,
        })?;
        let mut min = vec![f32::MAX; components];
        let mut max = vec![f32::MIN; components];
        let mut count = 0usize;
        for value in data {
            let component = count % components;
            if with_min_max {
                min[component] = min[component].min(value);
                max[component] = max[component].max(value);
            }
            self.append(&value.to_le_bytes())?;
            count = count.checked_add(1).ok_or(WriteError::TooLarge {
                field: "BIN chunk",
                bytes: usize::MAX,
            })?;
        }
        debug_assert_eq!(count, len);
        let view = self.views.len();
        self.views.push(json!({
            "buffer": 0,
            "byteOffset": offset,
            "byteLength": byte_len,
        }));
        let mut accessor = json!({
            "bufferView": view,
            "componentType": 5126,
            "count": len / components,
            "type": kind,
        });
        if with_min_max && len > 0 {
            accessor["min"] = json!(min);
            accessor["max"] = json!(max);
        }
        let index = self.accessors.len();
        self.accessors.push(accessor);
        Ok(index)
    }
}

impl BufferBuilder {
    /// Append u32 triangle indices as a buffer view + accessor.
    fn push_indices(&mut self, data: &[u32]) -> Result<usize, WriteError> {
        let offset = self.len();
        for v in data {
            self.append(&v.to_le_bytes())?;
        }
        let view = self.views.len();
        self.views.push(json!({
            "buffer": 0,
            "byteOffset": offset,
            "byteLength": data.len().checked_mul(4).ok_or(WriteError::TooLarge { field: "BIN chunk", bytes: usize::MAX })?,
        }));
        let index = self.accessors.len();
        self.accessors.push(json!({
            "bufferView": view,
            "componentType": 5125,
            "count": data.len(),
            "type": "SCALAR",
        }));
        Ok(index)
    }

    /// Append raw bytes (an encoded image) as a bare buffer view.
    fn push_view(&mut self, data: &[u8]) -> Result<usize, WriteError> {
        self.push_padding()?;
        let offset = self.len();
        self.append(data)?;
        let view = self.views.len();
        self.views.push(json!({
            "buffer": 0,
            "byteOffset": offset,
            "byteLength": data.len(),
        }));
        Ok(view)
    }

    /// Append u16 data (JOINTS_0) as a buffer view + accessor.
    fn push_u16<I>(&mut self, len: usize, data: I, kind: &str) -> Result<usize, WriteError>
    where
        I: IntoIterator<Item = u16>,
    {
        let components = if kind == "VEC4" { 4 } else { 1 };
        let offset = self.len();
        let mut seen = 0usize;
        for v in data {
            self.append(&v.to_le_bytes())?;
            seen = seen.checked_add(1).ok_or(WriteError::TooLarge {
                field: "BIN chunk",
                bytes: usize::MAX,
            })?;
        }
        debug_assert_eq!(seen, len);
        self.push_padding()?;
        let view = self.views.len();
        self.views.push(json!({
            "buffer": 0,
            "byteOffset": offset,
            "byteLength": len.checked_mul(2).ok_or(WriteError::TooLarge { field: "BIN chunk", bytes: usize::MAX })?,
        }));
        let index = self.accessors.len();
        self.accessors.push(json!({
            "bufferView": view,
            "componentType": 5123,
            "count": len / components,
            "type": kind,
        }));
        Ok(index)
    }
}

fn document_to_json(
    doc: &Document,
    buffer_uri: Option<String>,
    buffer_len: usize,
) -> Result<Value, WriteError> {
    let mut children = Vec::<Vec<usize>>::new();
    children
        .try_reserve_exact(doc.skeleton.bones.len())
        .map_err(|_| WriteError::Allocation {
            field: "skeleton adjacency",
            bytes: doc.skeleton.bones.len(),
        })?;
    children.resize_with(doc.skeleton.bones.len(), Vec::new);
    for (child, bone) in doc.skeleton.bones.iter().enumerate() {
        if let Some(parent) = bone.parent
            && let Some(children) = children.get_mut(parent)
        {
            children.push(child);
        }
    }
    let mut nodes = Vec::<Value>::new();
    nodes
        .try_reserve_exact(doc.skeleton.bones.len())
        .map_err(|_| WriteError::Allocation {
            field: "node array",
            bytes: doc.skeleton.bones.len(),
        })?;
    for (id, bone) in doc.skeleton.bones.iter().enumerate() {
        let mut node = json!({
            "name": bone.name,
            "translation": bone.rest.translation.to_array(),
            "rotation": bone.rest.rotation.to_array(),
            "scale": bone.rest.scale.to_array(),
        });
        if !children[id].is_empty() {
            node["children"] = json!(children[id]);
        }
        nodes.push(node);
    }
    let roots: Vec<usize> = doc
        .skeleton
        .bones
        .iter()
        .enumerate()
        .filter(|(_, b)| b.parent.is_none())
        .map(|(i, _)| i)
        .collect();

    let mut root = json!({
        "asset": {
            "version": "2.0",
            "generator": format!("animsmith {}", env!("CARGO_PKG_VERSION")),
        },
        "scene": 0,
        "scenes": [{ "nodes": roots }],
        "nodes": nodes,
    });
    // A glTF buffer must have byteLength ≥ 1. An empty document (no
    // animation, no mesh bytes) has nothing to reference it — and no
    // bufferViews or accessors either — so omit the buffer rather than
    // emit a zero-length one, which in GLB would force an empty BIN chunk
    // the Khronos validator rejects (GLB_EMPTY_CHUNK). The caller
    // likewise omits the (empty) bufferViews/accessors arrays.
    if buffer_len > 0 {
        let mut buffer = json!({ "byteLength": buffer_len });
        if let Some(uri) = buffer_uri {
            buffer["uri"] = json!(uri);
        }
        root["buffers"] = json!([buffer]);
    }
    Ok(root)
}

/// Narrow a GLB byte length to the `u32` its header/chunk field requires,
/// failing closed above the 4 GiB GLB limit rather than truncating (which
/// would emit a length field disagreeing with the bytes on disk).
pub(crate) fn glb_len_u32(field: &'static str, len: usize) -> Result<u32, WriteError> {
    u32::try_from(len).map_err(|_| WriteError::TooLarge { field, bytes: len })
}

/// The `u32` length fields of a GLB container.
#[derive(Debug)]
pub(crate) struct GlbLengths {
    /// Total file length (12-byte header + JSON chunk + optional BIN chunk).
    pub(crate) total: u32,
    /// JSON chunk payload length.
    pub(crate) json: u32,
    /// BIN chunk payload length, or `None` when the payload is empty (the
    /// BIN chunk is then omitted — an empty chunk is GLB_EMPTY_CHUNK).
    pub(crate) bin: Option<u32>,
}

/// Plan a GLB's chunk framing from its (already 4-byte-padded) JSON and
/// BIN payload lengths, narrowing every `u32` length field and failing
/// closed above the 4 GiB GLB limit. The parts are checked *before* the
/// total so an oversized JSON or BIN chunk is attributed to itself rather
/// than masked as a total overflow (each part is `<= total`, so a
/// total-first check could only ever report `total`).
pub(crate) fn plan_glb_lengths(json_len: usize, bin_len: usize) -> Result<GlbLengths, WriteError> {
    let json = glb_len_u32("JSON chunk", json_len)?;
    let (bin, bin_bytes) = if bin_len > 0 {
        (
            Some(glb_len_u32("BIN chunk", bin_len)?),
            8usize.checked_add(bin_len).ok_or(WriteError::TooLarge {
                field: "total GLB length",
                bytes: usize::MAX,
            })?,
        )
    } else {
        (None, 0)
    };
    let total_bytes = 12usize
        .checked_add(8)
        .and_then(|total| total.checked_add(json_len))
        .and_then(|total| total.checked_add(bin_bytes))
        .ok_or(WriteError::TooLarge {
            field: "total GLB length",
            bytes: usize::MAX,
        })?;
    let total = glb_len_u32("total GLB length", total_bytes)?;
    Ok(GlbLengths { total, json, bin })
}

struct Projection {
    root: Value,
    bin: Option<Vec<u8>>,
    bin_bytes: usize,
    summary: WriteSummary,
    bin_digest: [u8; 32],
}

/// Build the one normalized writer projection in either count or retain mode.
/// Count mode intentionally builds the exact JSON tree but retains no binary
/// payload; retain mode emits the same tree and bytes after a successful count
/// preflight has reserved the exact binary capacity.
fn build_projection(
    doc: &Document,
    mode: BinaryMode,
    reserve: usize,
    policy: GlbProjectionPolicyV1,
) -> Result<Projection, WriteError> {
    let assets = &doc.assets;
    let mut buffers = BufferBuilder::new(mode, reserve)?;
    let mut animations: Vec<Value> = Vec::new();
    let mut clips_without_writable_tracks = 0usize;

    for clip in &doc.clips {
        let mut samplers: Vec<Value> = Vec::new();
        let mut channels: Vec<Value> = Vec::new();
        for track in &clip.tracks {
            if track.times.is_empty() || track.bone >= doc.skeleton.bones.len() {
                continue;
            }
            let input = buffers.push_f32(
                track.times.len(),
                track.times.iter().copied(),
                "SCALAR",
                true,
            )?;
            let output = match &track.values {
                TrackValues::Vec3s(v) => buffers.push_f32(
                    v.len().checked_mul(3).ok_or(WriteError::TooLarge {
                        field: "BIN chunk",
                        bytes: usize::MAX,
                    })?,
                    v.iter().flat_map(|x| x.to_array()),
                    "VEC3",
                    false,
                )?,
                TrackValues::Quats(v) => buffers.push_f32(
                    v.len().checked_mul(4).ok_or(WriteError::TooLarge {
                        field: "BIN chunk",
                        bytes: usize::MAX,
                    })?,
                    v.iter().flat_map(|q| q.to_array()),
                    "VEC4",
                    false,
                )?,
            };
            let interpolation = match track.interpolation {
                Interpolation::Linear => "LINEAR",
                Interpolation::Step => "STEP",
                Interpolation::CubicSpline => "CUBICSPLINE",
            };
            let target_path = match track.property {
                Property::Translation => "translation",
                Property::Rotation => "rotation",
                Property::Scale => "scale",
            };
            let sampler = samplers.len();
            samplers.push(json!({
                "input": input,
                "output": output,
                "interpolation": interpolation,
            }));
            channels.push(json!({
                "sampler": sampler,
                "target": {
                    "node": track.bone,
                    "path": target_path,
                },
            }));
        }
        if channels.is_empty() {
            clips_without_writable_tracks += 1;
        } else {
            animations.push(json!({
                "name": clip.name,
                "samplers": samplers,
                "channels": channels,
            }));
        }
    }

    let mut meshes_json: Vec<Value> = Vec::new();
    let mut skins_json: Vec<Value> = Vec::new();
    // node index -> (mesh index, Option<skin index>)
    let mut node_attach: Vec<(usize, usize, Option<usize>)> = Vec::new();
    for mesh in &assets.meshes {
        let mut prims: Vec<Value> = Vec::new();
        for prim in &mesh.primitives {
            let mut attributes = json!({
                // POSITION min/max is required by the spec.
                "POSITION": buffers.push_f32(prim.positions.len().checked_mul(3).ok_or(WriteError::TooLarge { field: "BIN chunk", bytes: usize::MAX })?, prim.positions.iter().flat_map(|v| v.to_array()), "VEC3", true)?,
            });
            if !prim.normals.is_empty() {
                attributes["NORMAL"] = json!(
                    buffers.push_f32(
                        prim.normals
                            .len()
                            .checked_mul(3)
                            .ok_or(WriteError::TooLarge {
                                field: "BIN chunk",
                                bytes: usize::MAX
                            })?,
                        prim.normals.iter().flat_map(|v| v.to_array()),
                        "VEC3",
                        false
                    )?
                );
            }
            if !prim.uvs.is_empty() {
                attributes["TEXCOORD_0"] = json!(buffers.push_f32(
                    prim.uvs.len().checked_mul(2).ok_or(WriteError::TooLarge {
                        field: "BIN chunk",
                        bytes: usize::MAX
                    })?,
                    prim.uvs.iter().flatten().copied(),
                    "VEC2",
                    false
                )?);
            }
            if !prim.joints.is_empty() {
                attributes["JOINTS_0"] = json!(
                    buffers.push_u16(
                        prim.joints
                            .len()
                            .checked_mul(4)
                            .ok_or(WriteError::TooLarge {
                                field: "BIN chunk",
                                bytes: usize::MAX
                            })?,
                        prim.joints.iter().flatten().copied(),
                        "VEC4"
                    )?
                );
                attributes["WEIGHTS_0"] = json!(
                    buffers.push_f32(
                        prim.weights
                            .len()
                            .checked_mul(4)
                            .ok_or(WriteError::TooLarge {
                                field: "BIN chunk",
                                bytes: usize::MAX
                            })?,
                        prim.weights.iter().flatten().copied(),
                        "VEC4",
                        false
                    )?
                );
            }
            let mut value = json!({ "attributes": attributes });
            if !prim.indices.is_empty() {
                value["indices"] = json!(buffers.push_indices(&prim.indices)?);
            }
            if let Some(material) = prim.material {
                value["material"] = json!(material);
            }
            prims.push(value);
        }
        meshes_json.push(json!({ "name": mesh.name, "primitives": prims }));
    }

    for instance in &assets.instances {
        if instance.mesh >= assets.meshes.len() {
            continue;
        }
        let skin_index = if instance.skin_joints.is_empty() {
            None
        } else {
            let values = instance
                .skin_joints
                .iter()
                .enumerate()
                .flat_map(|(slot, &joint)| {
                    instance
                        .skin_ibms
                        .get(slot)
                        .copied()
                        .or_else(|| doc.skeleton.bones.get(joint).and_then(|b| b.inverse_bind))
                        .unwrap_or(glam::Mat4::IDENTITY)
                        .to_cols_array()
                });
            let accessor = buffers.push_f32(
                instance
                    .skin_joints
                    .len()
                    .checked_mul(16)
                    .ok_or(WriteError::TooLarge {
                        field: "BIN chunk",
                        bytes: usize::MAX,
                    })?,
                values,
                "MAT4",
                false,
            )?;
            let index = skins_json.len();
            skins_json.push(json!({
                "joints": instance.skin_joints,
                "inverseBindMatrices": accessor,
            }));
            Some(index)
        };
        // Skinned meshes hang off a fresh identity node at scene root:
        // the spec ignores a skinned mesh's node transform, but several
        // loaders (notably three.js) fold it into the bind matrix, so a
        // transform-carrying node yields inconsistent rendering. The
        // joints + IBMs fully place the vertices. Unskinned meshes keep
        // their original node, whose transform is meaningful.
        node_attach.push((instance.node, instance.mesh, skin_index));
    }

    // Embedded material textures: raw encoded bytes as buffer views
    // (glTF never decodes; PNG/JPEG pass through untouched).
    let mut images_json: Vec<Value> = Vec::new();
    let mut textures_json: Vec<Value> = Vec::new();
    let mut material_texture_index: Vec<Option<usize>> = vec![None; assets.materials.len()];
    let mut material_normal_texture_index: Vec<Option<usize>> = vec![None; assets.materials.len()];
    let mut material_metallic_roughness_texture_index: Vec<Option<usize>> =
        vec![None; assets.materials.len()];
    let mut material_occlusion_texture_index: Vec<Option<usize>> =
        vec![None; assets.materials.len()];
    for (mi, material) in assets.materials.iter().enumerate() {
        if let Some(texture) = &material.base_color_texture {
            let view = buffers.push_view(&texture.bytes)?;
            let image = images_json.len();
            images_json.push(json!({ "bufferView": view, "mimeType": texture.mime }));
            material_texture_index[mi] = Some(textures_json.len());
            textures_json.push(json!({ "source": image }));
        }
        if let Some(normal) = &material.normal_texture {
            let view = buffers.push_view(&normal.texture.bytes)?;
            let image = images_json.len();
            images_json.push(json!({
                "bufferView": view,
                "mimeType": normal.texture.mime,
            }));
            material_normal_texture_index[mi] = Some(textures_json.len());
            textures_json.push(json!({ "source": image }));
        }
        if let Some(texture) = &material.metallic_roughness_texture {
            let view = buffers.push_view(&texture.bytes)?;
            let image = images_json.len();
            images_json.push(json!({ "bufferView": view, "mimeType": texture.mime }));
            material_metallic_roughness_texture_index[mi] = Some(textures_json.len());
            textures_json.push(json!({ "source": image }));
        }
        if let Some(occlusion) = &material.occlusion_texture {
            let view = buffers.push_view(&occlusion.texture.bytes)?;
            let image = images_json.len();
            images_json.push(json!({ "bufferView": view, "mimeType": occlusion.texture.mime }));
            material_occlusion_texture_index[mi] = Some(textures_json.len());
            textures_json.push(json!({ "source": image }));
        }
    }

    let mut root = document_to_json(doc, None, buffers.len())?;
    // Present-but-empty accessor arrays are invalid glTF (minItems 1); an
    // empty document has none, so emit them only when populated.
    if !buffers.views.is_empty() {
        root["bufferViews"] = Value::Array(std::mem::take(&mut buffers.views));
    }
    if !buffers.accessors.is_empty() {
        root["accessors"] = Value::Array(std::mem::take(&mut buffers.accessors));
    }
    if !animations.is_empty() {
        root["animations"] = Value::Array(animations);
    }
    if !meshes_json.is_empty() {
        for (node, mesh_index, skin_index) in &node_attach {
            match skin_index {
                Some(skin) if policy == GlbProjectionPolicyV1::Legacy => {
                    let nodes = root["nodes"].as_array_mut().expect("nodes array");
                    let holder = nodes.len();
                    nodes.push(json!({
                        "name": format!("{}_skinned", assets.meshes[*mesh_index].name),
                        "mesh": mesh_index,
                        "skin": skin,
                    }));
                    root["scenes"][0]["nodes"]
                        .as_array_mut()
                        .expect("scene roots")
                        .push(json!(holder));
                }
                Some(skin) => {
                    let node_value = &mut root["nodes"][*node];
                    node_value["mesh"] = json!(mesh_index);
                    node_value["skin"] = json!(skin);
                }
                None => {
                    let node_value = &mut root["nodes"][*node];
                    node_value["mesh"] = json!(mesh_index);
                }
            }
        }
        root["meshes"] = Value::Array(meshes_json);
        if !skins_json.is_empty() {
            root["skins"] = Value::Array(skins_json);
        }
        if !assets.materials.is_empty() {
            root["materials"] = Value::Array(
                assets
                    .materials
                    .iter()
                    .enumerate()
                    .map(|(mi, m)| {
                        let mut pbr = json!({
                            "baseColorFactor": m.base_color,
                            "metallicFactor": m.metallic,
                            "roughnessFactor": m.roughness,
                        });
                        if let Some(slot) = material_texture_index[mi] {
                            pbr["baseColorTexture"] = json!({ "index": slot });
                        }
                        if let Some(slot) = material_metallic_roughness_texture_index[mi] {
                            pbr["metallicRoughnessTexture"] = json!({ "index": slot });
                        }
                        let mut material = json!({ "name": m.name, "pbrMetallicRoughness": pbr });
                        if let (Some(slot), Some(normal)) =
                            (material_normal_texture_index[mi], &m.normal_texture)
                        {
                            material["normalTexture"] = json!({
                                "index": slot,
                                "scale": normal.scale,
                            });
                        }
                        if let (Some(slot), Some(occlusion)) =
                            (material_occlusion_texture_index[mi], &m.occlusion_texture)
                        {
                            material["occlusionTexture"] = json!({
                                "index": slot,
                                "strength": occlusion.strength,
                            });
                        }
                        material
                    })
                    .collect(),
            );
            if !images_json.is_empty() {
                root["images"] = Value::Array(images_json);
                root["textures"] = Value::Array(textures_json);
            }
        }
    }

    let array_len = |key: &str| root.get(key).and_then(Value::as_array).map_or(0, Vec::len);
    let primitive_positions = root
        .get("meshes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|mesh| mesh.get("primitives").and_then(Value::as_array))
        .flatten()
        .filter_map(|primitive| {
            primitive
                .pointer("/attributes/POSITION")
                .and_then(Value::as_u64)
                .and_then(|index| usize::try_from(index).ok())
        })
        .filter_map(|index| {
            root.get("accessors")
                .and_then(Value::as_array)
                .and_then(|accessors| accessors.get(index))
                .and_then(|accessor| accessor.get("count"))
                .and_then(Value::as_u64)
                .and_then(|count| usize::try_from(count).ok())
        })
        .sum();
    let animations = array_len("animations");
    let summary = WriteSummary {
        nodes: array_len("nodes"),
        animations,
        meshes: array_len("meshes"),
        primitive_positions,
        materials: array_len("materials"),
        clips_without_writable_tracks,
    };

    let bin_bytes = buffers.len();
    let bin_digest = buffers.digest();
    let bin = match mode {
        BinaryMode::Count => None,
        BinaryMode::Retain => Some(buffers.into_bytes()?),
    };
    Ok(Projection {
        root,
        bin,
        bin_bytes,
        summary,
        bin_digest,
    })
}

/// Projection contract selected by a GLB caller.
///
/// [`Self::Legacy`] retains `write()`'s established normalizing writer
/// behaviour. [`Self::StrictFootCycleV1`] is for an #18 candidate: it rejects
/// the modeled document shapes whose legacy projection would omit, merge, or
/// otherwise fail to represent enough for a foot-cycle candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlbProjectionPolicyV1 {
    /// Existing conversion/transform writer behaviour.
    Legacy,
    /// Fail-closed candidate projection for foot-cycle V1.
    StrictFootCycleV1,
}

fn padded_len(len: usize, field: &'static str) -> Result<usize, WriteError> {
    len.checked_add(3)
        .map(|value| value & !3)
        .ok_or(WriteError::TooLarge {
            field,
            bytes: usize::MAX,
        })
}

struct CountingWriter {
    bytes: usize,
    digest: Sha256,
}

impl std::io::Write for CountingWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.bytes = self
            .bytes
            .checked_add(bytes.len())
            .ok_or_else(|| std::io::Error::other("JSON byte count overflow"))?;
        self.digest.update(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn count_json(root: &Value) -> Result<(usize, [u8; 32]), WriteError> {
    let mut sink = CountingWriter {
        bytes: 0,
        digest: Sha256::new(),
    };
    serde_json::to_writer(&mut sink, root)?;
    Ok((
        padded_len(sink.bytes, "JSON chunk")?,
        sink.digest.finalize().into(),
    ))
}

fn check_limits(
    json_bytes: usize,
    bin_bytes: usize,
    limits: GlbWriteLimits,
) -> Result<GlbLengths, WriteError> {
    if json_bytes > limits.max_json_bytes {
        return Err(WriteError::TooLarge {
            field: "configured JSON chunk limit",
            bytes: json_bytes,
        });
    }
    if bin_bytes > limits.max_bin_bytes {
        return Err(WriteError::TooLarge {
            field: "configured BIN chunk limit",
            bytes: bin_bytes,
        });
    }
    let lengths = plan_glb_lengths(json_bytes, bin_bytes)?;
    if lengths.total as usize > limits.max_total_bytes {
        return Err(WriteError::TooLarge {
            field: "configured total GLB limit",
            bytes: lengths.total as usize,
        });
    }
    Ok(lengths)
}

fn strict_foot_cycle_projectable(doc: &Document, limits: GlbWriteLimits) -> Result<(), WriteError> {
    strict_structure_projectable(doc, limits)?;
    validate_document_shape(doc).map_err(|error| WriteError::Refused(error.to_string()))?;
    for (clip_index, clip) in doc.clips.iter().enumerate() {
        if clip.tracks.is_empty() {
            return Err(WriteError::Refused(format!(
                "clip {clip_index} has no writable tracks"
            )));
        }
        let end = clip
            .tracks
            .iter()
            .filter_map(|track| track.times.last().copied())
            .fold(0.0_f64, |end, time| end.max(f64::from(time)));
        if !clip.duration_s.is_finite() || clip.duration_s != end {
            return Err(WriteError::Refused(format!(
                "clip {clip_index} duration is not represented by its track times"
            )));
        }
    }
    if doc.assets.meshes.is_empty() && !doc.assets.materials.is_empty() {
        return Err(WriteError::Refused(
            "materials without a mesh would be omitted".to_owned(),
        ));
    }
    for (mesh_index, mesh) in doc.assets.meshes.iter().enumerate() {
        if mesh.primitives.is_empty() {
            return Err(WriteError::Refused(format!(
                "mesh {mesh_index} has no primitives"
            )));
        }
        for (primitive_index, primitive) in mesh.primitives.iter().enumerate() {
            if primitive.positions.is_empty()
                || primitive
                    .positions
                    .iter()
                    .any(|position| !position.is_finite())
                || (!primitive.normals.is_empty()
                    && (primitive.normals.len() != primitive.positions.len()
                        || primitive.normals.iter().any(|normal| !normal.is_finite())))
                || (!primitive.uvs.is_empty()
                    && (primitive.uvs.len() != primitive.positions.len()
                        || primitive
                            .uvs
                            .iter()
                            .flatten()
                            .any(|value| !value.is_finite())))
                || primitive.joints.len() != primitive.weights.len()
                || (!primitive.joints.is_empty()
                    && (primitive.joints.len() != primitive.positions.len()
                        || primitive
                            .weights
                            .iter()
                            .flatten()
                            .any(|weight| !weight.is_finite())))
                || primitive
                    .indices
                    .iter()
                    .any(|&index| index as usize >= primitive.positions.len())
                || (primitive.indices.is_empty() && !primitive.positions.len().is_multiple_of(3))
                || (!primitive.indices.is_empty() && !primitive.indices.len().is_multiple_of(3))
                || !primitive.additional_influence_sets.is_empty()
            {
                return Err(WriteError::Refused(format!(
                    "mesh {mesh_index} primitive {primitive_index} is not exactly writer-representable"
                )));
            }
            if primitive
                .material
                .is_some_and(|material| material >= doc.assets.materials.len())
            {
                return Err(WriteError::Refused(format!(
                    "mesh {mesh_index} primitive {primitive_index} references an unknown material"
                )));
            }
        }
    }
    strict_skin_projectable(doc)?;
    strict_source_sidecars_projectable(doc)?;
    for (material_index, material) in doc.assets.materials.iter().enumerate() {
        if !material.base_color.into_iter().all(f32::is_finite)
            || !material.metallic.is_finite()
            || !material.roughness.is_finite()
        {
            return Err(WriteError::Refused(format!(
                "material {material_index} has non-finite factors"
            )));
        }
        for texture in [
            material.base_color_texture.as_ref(),
            material.normal_texture.as_ref().map(|slot| &slot.texture),
            material.metallic_roughness_texture.as_ref(),
            material
                .occlusion_texture
                .as_ref()
                .map(|slot| &slot.texture),
        ] {
            if let Some(texture) = texture
                && (texture.bytes.is_empty()
                    || !matches!(texture.mime.as_str(), "image/png" | "image/jpeg"))
            {
                return Err(WriteError::Refused(format!(
                    "material {material_index} has an unsupported embedded texture"
                )));
            }
        }
        if material
            .normal_texture
            .as_ref()
            .is_some_and(|normal| !normal.scale.is_finite())
            || material
                .occlusion_texture
                .as_ref()
                .is_some_and(|occlusion| !occlusion.strength.is_finite())
        {
            return Err(WriteError::Refused(format!(
                "material {material_index} has a non-finite texture factor"
            )));
        }
    }
    let mut unskinned_nodes = std::collections::BTreeSet::new();
    for instance in &doc.assets.instances {
        if instance.skin_joints.is_empty() && !unskinned_nodes.insert(instance.node) {
            return Err(WriteError::Refused(
                "multiple unskinned mesh instances target one node".to_owned(),
            ));
        }
    }
    strict_scene_projectable(doc)
}

fn strict_structure_projectable(doc: &Document, limits: GlbWriteLimits) -> Result<(), WriteError> {
    let mut rows = doc.skeleton.bones.len();
    let mut name_bytes = 0usize;
    let mut work = 0usize;
    let add = |total: &mut usize, next: usize, field: &'static str, cap: usize| {
        *total = total
            .checked_add(next)
            .filter(|value| *value <= cap)
            .ok_or_else(|| {
                WriteError::Refused(format!(
                    "strict foot-cycle {field} exceeds its V1 structural limit"
                ))
            })?;
        Ok::<(), WriteError>(())
    };
    if rows > limits.max_structural_rows {
        return Err(WriteError::Refused(
            "strict foot-cycle JSON rows exceed its V1 structural limit".to_owned(),
        ));
    }
    // The writer's first proportional allocations are skeleton adjacency,
    // node/root arrays, and the JSON vectors below. Charge every source-side
    // validation table too: `validate_document_shape` and strict admission
    // build ordered lookup sets after this boundary.
    add(
        &mut rows,
        doc.skeleton.bones.len().saturating_mul(2),
        "JSON rows",
        limits.max_structural_rows,
    )?;
    // parent walk plus emitted TRS (3 + 4 + 3 scalar components) per node.
    add(
        &mut work,
        doc.skeleton.bones.len().saturating_mul(11),
        "work",
        limits.max_work,
    )?;
    for bone in &doc.skeleton.bones {
        add(
            &mut name_bytes,
            bone.name.len(),
            "name bytes",
            limits.max_name_bytes,
        )?;
    }
    for clip in &doc.clips {
        add(&mut rows, 1, "JSON rows", limits.max_structural_rows)?;
        add(
            &mut name_bytes,
            clip.name.len(),
            "name bytes",
            limits.max_name_bytes,
        )?;
        for track in &clip.tracks {
            // sampler/channel plus two buffer views and two accessors.
            add(&mut rows, 6, "JSON rows", limits.max_structural_rows)?;
            add(&mut work, track.times.len(), "work", limits.max_work)?;
            let value_components = match &track.values {
                TrackValues::Vec3s(values) => values.len().saturating_mul(3),
                TrackValues::Quats(values) => values.len().saturating_mul(4),
            };
            add(&mut work, value_components, "work", limits.max_work)?;
        }
    }
    for mesh in &doc.assets.meshes {
        add(&mut rows, 1, "JSON rows", limits.max_structural_rows)?;
        add(
            &mut name_bytes,
            mesh.name.len(),
            "name bytes",
            limits.max_name_bytes,
        )?;
        for primitive in &mesh.primitives {
            // primitive plus up to six emitted buffer-view/accessor pairs.
            add(&mut rows, 13, "JSON rows", limits.max_structural_rows)?;
            for count in [
                primitive.positions.len().saturating_mul(3),
                primitive.normals.len().saturating_mul(3),
                primitive.uvs.len().saturating_mul(2),
                primitive.joints.len().saturating_mul(4),
                primitive.weights.len().saturating_mul(4),
                primitive.indices.len(),
                primitive.additional_influence_sets.len(),
            ] {
                add(&mut work, count, "work", limits.max_work)?;
            }
        }
    }
    for material in &doc.assets.materials {
        // material plus up to four image/texture rows and their buffer views.
        add(&mut rows, 13, "JSON rows", limits.max_structural_rows)?;
        add(
            &mut name_bytes,
            material.name.len(),
            "name bytes",
            limits.max_name_bytes,
        )?;
        for texture in [
            material.base_color_texture.as_ref(),
            material.normal_texture.as_ref().map(|slot| &slot.texture),
            material.metallic_roughness_texture.as_ref(),
            material
                .occlusion_texture
                .as_ref()
                .map(|slot| &slot.texture),
        ]
        .into_iter()
        .flatten()
        {
            add(&mut work, texture.bytes.len(), "work", limits.max_work)?;
        }
    }
    add(
        &mut rows,
        doc.assets.instances.len().saturating_mul(3),
        "JSON rows",
        limits.max_structural_rows,
    )?;
    for instance in &doc.assets.instances {
        add(
            &mut work,
            instance.skin_joints.len(),
            "work",
            limits.max_work,
        )?;
        add(
            &mut work,
            instance.skin_ibms.len().saturating_mul(16),
            "work",
            limits.max_work,
        )?;
    }
    for scene in &doc.assets.scenes {
        add(&mut rows, 1, "JSON rows", limits.max_structural_rows)?;
        add(&mut work, scene.roots.len(), "work", limits.max_work)?;
    }
    let source = &doc.assets.source_skeleton;
    add(
        &mut rows,
        source.nodes.len(),
        "JSON rows",
        limits.max_structural_rows,
    )?;
    add(
        &mut rows,
        source.skins.len(),
        "JSON rows",
        limits.max_structural_rows,
    )?;
    for node in &source.nodes {
        add(
            &mut work,
            node.scene_root_indices.len().saturating_add(1),
            "work",
            limits.max_work,
        )?;
    }
    for skin in &source.skins {
        add(
            &mut work,
            skin.joint_source_node_indices.len(),
            "work",
            limits.max_work,
        )?;
        add(
            &mut work,
            skin.inverse_bind_accessor.matrices.len().saturating_mul(16),
            "work",
            limits.max_work,
        )?;
        add(&mut work, skin.attachments.len(), "work", limits.max_work)?;
    }
    let resources = &doc.assets.material_resources;
    add(
        &mut rows,
        resources.materials.len(),
        "JSON rows",
        limits.max_structural_rows,
    )?;
    add(
        &mut rows,
        resources.textures.len(),
        "JSON rows",
        limits.max_structural_rows,
    )?;
    add(
        &mut rows,
        resources.images.len(),
        "JSON rows",
        limits.max_structural_rows,
    )?;
    for material in &resources.materials {
        add(
            &mut work,
            material.texture_bindings.len().saturating_add(1),
            "work",
            limits.max_work,
        )?;
    }
    add(
        &mut work,
        resources
            .textures
            .len()
            .saturating_add(resources.images.len()),
        "work",
        limits.max_work,
    )?;
    Ok(())
}

fn strict_skin_projectable(doc: &Document) -> Result<(), WriteError> {
    let mut mesh_kind = std::collections::BTreeMap::<usize, bool>::new();
    let mut occupied_nodes = std::collections::BTreeSet::new();
    for (instance_index, instance) in doc.assets.instances.iter().enumerate() {
        if !occupied_nodes.insert(instance.node) {
            return Err(WriteError::Refused(
                "multiple mesh instances target one normalized node".to_owned(),
            ));
        }
        let Some(mesh) = doc.assets.meshes.get(instance.mesh) else {
            return Err(WriteError::Refused(format!(
                "instance {instance_index} references an unknown mesh"
            )));
        };
        let skinned = !instance.skin_joints.is_empty();
        if mesh_kind
            .insert(instance.mesh, skinned)
            .is_some_and(|prior| prior != skinned)
        {
            return Err(WriteError::Refused(
                "a mesh cannot be reused by both skinned and unskinned instances".to_owned(),
            ));
        }
        if !skinned {
            if mesh
                .primitives
                .iter()
                .any(|primitive| !primitive.joints.is_empty())
            {
                return Err(WriteError::Refused(
                    "an unskinned instance would reinterpret JOINTS_0".to_owned(),
                ));
            }
            continue;
        }
        if instance.skin_ibms.len() != instance.skin_joints.len() {
            return Err(WriteError::Refused(
                "skinned instance has no explicit inverse-bind matrices to preserve".to_owned(),
            ));
        }
        for (primitive_index, primitive) in mesh.primitives.iter().enumerate() {
            if primitive.joints.is_empty()
                || primitive
                    .joints
                    .iter()
                    .flatten()
                    .any(|&slot| slot as usize >= instance.skin_joints.len())
                || primitive
                    .weights
                    .iter()
                    .flatten()
                    .any(|weight| !weight.is_finite())
            {
                return Err(WriteError::Refused(format!(
                    "mesh {} primitive {primitive_index} has unrepresentable primary skin influences",
                    instance.mesh
                )));
            }
        }
    }
    Ok(())
}

fn strict_source_sidecars_projectable(doc: &Document) -> Result<(), WriteError> {
    strict_complete_material_resources_projectable(doc)?;
    if doc.assets.source_skeleton.coverage != SourceSkeletonCoverage::Complete {
        return Ok(());
    }
    let mut bone_of_source = std::collections::BTreeMap::new();
    for node in &doc.assets.source_skeleton.nodes {
        let Some(bone) = node.bone else {
            return Err(WriteError::Refused(
                "source-node projection drops a scene node".to_owned(),
            ));
        };
        let Some(normalized) = doc.skeleton.bones.get(bone) else {
            return Err(WriteError::Refused(
                "source-node bone is outside the normalized skeleton".to_owned(),
            ));
        };
        match &node.local_rest {
            SourceNodeLocalRest::Trs {
                translation,
                rotation,
                scale,
            } if *translation == normalized.rest.translation
                && *rotation == normalized.rest.rotation
                && *scale == normalized.rest.scale => {}
            _ => {
                return Err(WriteError::Refused(
                    "source-node local rest cannot be represented by the normalized writer"
                        .to_owned(),
                ));
            }
        }
        bone_of_source.insert(node.source_node_index, bone);
    }
    let mut instance_of_source = std::collections::BTreeMap::new();
    for (index, instance) in doc.assets.instances.iter().enumerate() {
        if instance_of_source
            .insert(instance.source_node_index, index)
            .is_some()
        {
            return Err(WriteError::Refused(
                "source skin attachment does not identify exactly one normalized instance"
                    .to_owned(),
            ));
        }
    }
    let mut seen_attachments = std::collections::BTreeSet::new();
    for skin in &doc.assets.source_skeleton.skins {
        if skin.skeleton_root_source_node_index.is_some() {
            return Err(WriteError::Refused(
                "source skin skeleton-root declaration is not emitted by the writer".to_owned(),
            ));
        }
        if skin.attachments.is_empty() {
            return Err(WriteError::Refused(
                "unattached complete source skin would be omitted by the writer".to_owned(),
            ));
        }
        if skin.inverse_bind_accessor.status != SourceInverseBindAccessorStatus::Available
            || skin.inverse_bind_accessor.declared_count
                != Some(skin.joint_source_node_indices.len())
            || skin.inverse_bind_accessor.matrices.len() != skin.joint_source_node_indices.len()
        {
            return Err(WriteError::Refused(
                "complete source skin inverse-bind facts cannot be represented exactly".to_owned(),
            ));
        }
        for attachment in &skin.attachments {
            if !seen_attachments.insert(attachment.source_node_index) {
                return Err(WriteError::Refused(
                    "one source node cannot be attached to multiple strict source skins".to_owned(),
                ));
            }
            let Some(&instance_index) = instance_of_source.get(&attachment.source_node_index)
            else {
                return Err(WriteError::Refused(
                    "source skin attachment does not identify exactly one normalized instance"
                        .to_owned(),
                ));
            };
            let instance = &doc.assets.instances[instance_index];
            if bone_of_source.get(&attachment.source_node_index) != Some(&instance.node)
                || attachment.source_mesh_index
                    != doc
                        .assets
                        .meshes
                        .get(instance.mesh)
                        .map(|mesh| mesh.source_mesh_index)
                || instance.skin_joints.is_empty()
                || instance.skin_joints.len() != skin.joint_source_node_indices.len()
                || instance.skin_ibms != skin.inverse_bind_accessor.matrices
            {
                return Err(WriteError::Refused(
                    "source skin attachment, joints, or inverse binds would change".to_owned(),
                ));
            }
            for (&source_joint, &joint) in skin
                .joint_source_node_indices
                .iter()
                .zip(&instance.skin_joints)
            {
                if bone_of_source.get(&source_joint) != Some(&joint) {
                    return Err(WriteError::Refused(
                        "source skin joint order cannot be represented by the normalized instance"
                            .to_owned(),
                    ));
                }
            }
        }
    }
    Ok(())
}

fn strict_complete_material_resources_projectable(doc: &Document) -> Result<(), WriteError> {
    let resources = &doc.assets.material_resources;
    if resources.coverage != MaterialResourceCoverage::Complete {
        return Ok(());
    }
    // An exact empty source graph is preserved: the emitted GLB has no
    // materials, textures, or images and the same loader redetects complete
    // empty lists. Non-empty complete graphs still carry source-only ids,
    // declaration kinds, and inspection facts that this writer cannot emit.
    if resources.materials.is_empty()
        && resources.textures.is_empty()
        && resources.images.is_empty()
        && doc.assets.materials.is_empty()
    {
        return Ok(());
    }
    // The normalized writer has texture bytes and MIME types, but no stable
    // source texture/image ids, declaration kinds, or inspection facts. A
    // non-empty complete resource graph would be regenerated rather than preserved.
    Err(WriteError::Refused(
        "complete source material-resource evidence is not preserved by the writer".to_owned(),
    ))
}

fn strict_scene_projectable(doc: &Document) -> Result<(), WriteError> {
    let scenes = &doc.assets.scenes;
    if scenes.is_empty() {
        return Err(WriteError::Refused(
            "strict projection requires the one emitted source scene and default".to_owned(),
        ));
    }
    if scenes.len() != 1 || doc.assets.default_scene != Some(0) {
        return Err(WriteError::Refused(
            "multiple or non-canonical source scenes cannot be preserved".to_owned(),
        ));
    }
    let expected_roots = doc
        .skeleton
        .bones
        .iter()
        .filter(|bone| bone.parent.is_none())
        .count();
    let actual_roots = &scenes[0].roots;
    if actual_roots.len() != expected_roots
        || actual_roots.iter().enumerate().any(|(index, &root)| {
            doc.skeleton
                .bones
                .get(root)
                .is_none_or(|bone| bone.parent.is_some())
                || actual_roots[..index].contains(&root)
        })
    {
        return Err(WriteError::Refused(
            "source scene roots differ from the canonical emitted scene".to_owned(),
        ));
    }
    if doc.assets.source_skeleton.coverage == SourceSkeletonCoverage::Complete {
        for node in &doc.assets.source_skeleton.nodes {
            let Some(bone) = node.bone else {
                return Err(WriteError::Refused(
                    "source scene membership has no normalized bone".to_owned(),
                ));
            };
            let Some(bone) = doc.skeleton.bones.get(bone) else {
                return Err(WriteError::Refused(
                    "source scene membership names an invalid normalized bone".to_owned(),
                ));
            };
            let should_be_root = bone.parent.is_none();
            let expected: &[usize] = if should_be_root { &[0] } else { &[] };
            if node.scene_root_indices.as_slice() != expected {
                return Err(WriteError::Refused(
                    "source scene membership cannot be projected exactly".to_owned(),
                ));
            }
        }
    }
    Ok(())
}

/// Count an exact in-memory GLB candidate without retaining its binary bytes.
///
/// The receipt may be passed to [`write_glb_bytes`] only with the same
/// [`GlbProjectionPolicyV1`] and unchanged emitted JSON/BIN projection.
///
/// # Errors
///
/// Refuses a strict foot-cycle candidate whose normalized model would lose
/// data, or whose exact padded JSON, BIN, or total framing exceeds `limits`.
pub fn preflight_glb_bytes(
    doc: &Document,
    policy: GlbProjectionPolicyV1,
    limits: GlbWriteLimits,
) -> Result<GlbWritePreflight, WriteError> {
    if policy == GlbProjectionPolicyV1::StrictFootCycleV1 {
        strict_foot_cycle_projectable(doc, limits)?;
    }
    let projection = build_projection(doc, BinaryMode::Count, 0, policy)?;
    let (json_bytes, json_digest) = count_json(&projection.root)?;
    let bin_bytes = padded_len(projection.bin_bytes, "BIN chunk")?;
    let lengths = check_limits(json_bytes, bin_bytes, limits)?;
    Ok(GlbWritePreflight {
        summary: projection.summary,
        json_bytes,
        bin_bytes,
        total_bytes: lengths.total as usize,
        limits,
        policy,
        json_digest,
        bin_digest: projection.bin_digest,
    })
}

/// Construct an in-memory GLB from an exact preflight receipt.
///
/// This reruns the same projection in retaining mode and refuses a changed
/// document or limit set before allocating the complete GLB vector.
///
/// # Errors
///
/// Returns [`WriteError::ReceiptMismatch`] when the document no longer has the
/// approved exact projection, or [`WriteError::Allocation`] when the checked
/// output vector cannot reserve its exact total capacity.
pub fn write_glb_bytes(
    doc: &Document,
    policy: GlbProjectionPolicyV1,
    receipt: &GlbWritePreflight,
) -> Result<Vec<u8>, WriteError> {
    if receipt.policy != policy {
        return Err(WriteError::ReceiptMismatch);
    }
    let counted = preflight_glb_bytes(doc, policy, receipt.limits)?;
    if &counted != receipt {
        return Err(WriteError::ReceiptMismatch);
    }
    let projection = build_projection(doc, BinaryMode::Retain, receipt.bin_bytes, policy)?;
    let (json_bytes, json_digest) = count_json(&projection.root)?;
    let mut bin = projection
        .bin
        .expect("retaining projection has binary bytes");
    while !bin.len().is_multiple_of(4) {
        bin.push(0);
    }
    let lengths = check_limits(json_bytes, bin.len(), receipt.limits)?;
    if projection.summary != receipt.summary
        || json_bytes != receipt.json_bytes
        || bin.len() != receipt.bin_bytes
        || lengths.total as usize != receipt.total_bytes
        || json_digest != receipt.json_digest
        || projection.bin_digest != receipt.bin_digest
    {
        return Err(WriteError::ReceiptMismatch);
    }
    let mut json = Vec::new();
    json.try_reserve_exact(json_bytes)
        .map_err(|_| WriteError::Allocation {
            field: "JSON chunk",
            bytes: json_bytes,
        })?;
    serde_json::to_writer(&mut json, &projection.root)?;
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    let mut out = Vec::new();
    out.try_reserve_exact(receipt.total_bytes)
        .map_err(|_| WriteError::Allocation {
            field: "GLB output",
            bytes: receipt.total_bytes,
        })?;
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&lengths.total.to_le_bytes());
    out.extend_from_slice(&lengths.json.to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&json);
    if let Some(bin_len) = lengths.bin {
        out.extend_from_slice(&bin_len.to_le_bytes());
        out.extend_from_slice(b"BIN\0");
        out.extend_from_slice(&bin);
    }
    debug_assert_eq!(out.len(), receipt.total_bytes);
    Ok(out)
}

/// Serialize `doc` to `path` (`.glb` for binary, anything else as `.gltf`
/// JSON with an embedded data-URI buffer). This legacy entry point retains its
/// historical permissive projection; #18 candidates use the strict receipt API.
///
/// # Errors
///
/// Returns a write, serialization, checked-size, or allocation error.
pub fn write(doc: &Document, path: &Path) -> Result<WriteSummary, WriteError> {
    let binary = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("glb"));
    if binary {
        let receipt = preflight_glb_bytes(
            doc,
            GlbProjectionPolicyV1::Legacy,
            GlbWriteLimits {
                max_json_bytes: u32::MAX as usize,
                max_bin_bytes: u32::MAX as usize,
                max_total_bytes: u32::MAX as usize,
                ..GlbWriteLimits::FOOT_CYCLE_V1
            },
        )?;
        let summary = receipt.summary();
        let bytes = write_glb_bytes(doc, GlbProjectionPolicyV1::Legacy, &receipt)?;
        std::fs::write(path, bytes).map_err(|source| WriteError::Io {
            path: path.display().to_string(),
            source,
        })?;
        return Ok(summary);
    }
    let mut projection =
        build_projection(doc, BinaryMode::Retain, 0, GlbProjectionPolicyV1::Legacy)?;
    if projection.bin_bytes > 0 {
        projection.root["buffers"][0]["uri"] = json!(format!(
            "data:application/octet-stream;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(
                projection
                    .bin
                    .as_deref()
                    .expect("retaining projection has binary bytes")
            )
        ));
    }
    let summary = projection.summary;
    let text = serde_json::to_string_pretty(&projection.root)?;
    std::fs::write(path, text).map_err(|source| WriteError::Io {
        path: path.display().to_string(),
        source,
    })?;
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::{GlbLengths, glb_len_u32, plan_glb_lengths};
    use crate::WriteError;

    #[test]
    fn glb_len_u32_accepts_up_to_the_u32_limit() {
        assert_eq!(glb_len_u32("x", 0).unwrap(), 0);
        assert_eq!(glb_len_u32("x", 1234).unwrap(), 1234);
        assert_eq!(glb_len_u32("x", u32::MAX as usize).unwrap(), u32::MAX);
    }

    // A length past the u32 limit is only representable where usize is
    // wider than u32; on a 32-bit target the value can't be constructed.
    #[test]
    #[cfg(target_pointer_width = "64")]
    fn glb_len_u32_rejects_over_4gib() {
        let too_big = u32::MAX as usize + 1;
        let err = glb_len_u32("total GLB length", too_big).unwrap_err();
        let msg = err.to_string();
        assert!(
            matches!(err, WriteError::TooLarge { field: "total GLB length", bytes } if bytes == too_big),
            "expected TooLarge naming the field and size"
        );
        assert!(
            msg.contains("4 GiB") && msg.contains("total GLB length"),
            "message must name the limit and field: {msg}"
        );
    }

    // The seam `write()` actually uses: from JSON/BIN payload lengths it
    // derives the three u32 fields. An 8-byte JSON + 16-byte BIN gives a
    // total of 12 (header) + 8+8 (JSON chunk) + 8+16 (BIN chunk) = 52.
    #[test]
    fn plan_glb_lengths_derives_the_three_fields() {
        let GlbLengths { total, json, bin } = plan_glb_lengths(8, 16).unwrap();
        assert_eq!((total, json, bin), (12 + 8 + 8 + 8 + 16, 8, Some(16)));
        // Empty BIN payload → no BIN chunk, total drops the 8+bin bytes.
        let GlbLengths { total, json, bin } = plan_glb_lengths(8, 0).unwrap();
        assert_eq!((total, json, bin), (12 + 8 + 8, 8, None));
    }

    // Pins the writer's length-field wiring without allocating a >4 GiB
    // document: each oversized field is attributed to *itself*, not
    // masked as a total overflow. (Regression guard for the wiring — a
    // `write()` that skipped `plan_glb_lengths` would drop this coverage.)
    #[test]
    #[cfg(target_pointer_width = "64")]
    fn plan_glb_lengths_attributes_each_overflowing_field() {
        let over = u32::MAX as usize + 1;
        let ok = 8usize;
        let field = |r: Result<GlbLengths, WriteError>| match r.unwrap_err() {
            WriteError::TooLarge { field, .. } => field,
            other => panic!("expected TooLarge, got {other:?}"),
        };
        assert_eq!(field(plan_glb_lengths(over, ok)), "JSON chunk");
        assert_eq!(field(plan_glb_lengths(ok, over)), "BIN chunk");
        // Both parts fit in u32 but their sum overflows the total.
        let half = u32::MAX as usize / 2 + 1;
        assert_eq!(field(plan_glb_lengths(half, half)), "total GLB length");
    }
}
