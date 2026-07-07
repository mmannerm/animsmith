//! Minimal glTF 2.0 writer for `convert`/`transform`: emits the
//! skeleton (node hierarchy + rest TRS), every clip's animation tracks,
//! and whatever scene assets the [`Document`] carries ([`Document::assets`]
//! — triangulated meshes, skins, and factor-only materials). A document
//! with default-empty assets writes animation + skeleton only, so
//! animation data can still enter glTF-based tooling (including animsmith
//! itself) straight from a DCC export.
//!
//! Values are written exactly as held in the core model — lint first;
//! conversion does not repair.

use crate::WriteError;
use animsmith_core::model::{Document, Interpolation, Property, TrackValues};
use base64::Engine as _;
use serde_json::{Value, json};
use std::path::Path;

struct BufferBuilder {
    bytes: Vec<u8>,
    views: Vec<Value>,
    accessors: Vec<Value>,
}

impl BufferBuilder {
    fn new() -> Self {
        Self {
            bytes: Vec::new(),
            views: Vec::new(),
            accessors: Vec::new(),
        }
    }

    /// Append `data` as a buffer view + accessor; returns the accessor
    /// index. `kind` is "SCALAR" | "VEC3" | "VEC4"; floats only.
    fn push(&mut self, data: &[f32], kind: &str, with_min_max: bool) -> usize {
        let components = match kind {
            "SCALAR" => 1,
            "VEC2" => 2,
            "VEC3" => 3,
            "MAT4" => 16,
            _ => 4,
        };
        let offset = self.bytes.len();
        for v in data {
            self.bytes.extend_from_slice(&v.to_le_bytes());
        }
        let view = self.views.len();
        self.views.push(json!({
            "buffer": 0,
            "byteOffset": offset,
            "byteLength": data.len() * 4,
        }));
        let mut accessor = json!({
            "bufferView": view,
            "componentType": 5126,
            "count": data.len() / components,
            "type": kind,
        });
        if with_min_max && !data.is_empty() {
            // Required on animation inputs; componentwise for vectors.
            let mut min = vec![f32::MAX; components];
            let mut max = vec![f32::MIN; components];
            for (i, v) in data.iter().enumerate() {
                let c = i % components;
                min[c] = min[c].min(*v);
                max[c] = max[c].max(*v);
            }
            accessor["min"] = json!(min);
            accessor["max"] = json!(max);
        }
        let index = self.accessors.len();
        self.accessors.push(accessor);
        index
    }
}

impl BufferBuilder {
    /// Append u32 triangle indices as a buffer view + accessor.
    fn push_indices(&mut self, data: &[u32]) -> usize {
        let offset = self.bytes.len();
        for v in data {
            self.bytes.extend_from_slice(&v.to_le_bytes());
        }
        let view = self.views.len();
        self.views.push(json!({
            "buffer": 0,
            "byteOffset": offset,
            "byteLength": data.len() * 4,
        }));
        let index = self.accessors.len();
        self.accessors.push(json!({
            "bufferView": view,
            "componentType": 5125,
            "count": data.len(),
            "type": "SCALAR",
        }));
        index
    }

    /// Append raw bytes (an encoded image) as a bare buffer view.
    fn push_view(&mut self, data: &[u8]) -> usize {
        while !self.bytes.len().is_multiple_of(4) {
            self.bytes.push(0);
        }
        let offset = self.bytes.len();
        self.bytes.extend_from_slice(data);
        let view = self.views.len();
        self.views.push(json!({
            "buffer": 0,
            "byteOffset": offset,
            "byteLength": data.len(),
        }));
        view
    }

    /// Append u16 data (JOINTS_0) as a buffer view + accessor.
    fn push_u16(&mut self, data: &[u16], kind: &str) -> usize {
        let components = if kind == "VEC4" { 4 } else { 1 };
        let offset = self.bytes.len();
        for v in data {
            self.bytes.extend_from_slice(&v.to_le_bytes());
        }
        while !self.bytes.len().is_multiple_of(4) {
            self.bytes.push(0);
        }
        let view = self.views.len();
        self.views.push(json!({
            "buffer": 0,
            "byteOffset": offset,
            "byteLength": data.len() * 2,
        }));
        let index = self.accessors.len();
        self.accessors.push(json!({
            "bufferView": view,
            "componentType": 5123,
            "count": data.len() / components,
            "type": kind,
        }));
        index
    }
}

fn document_to_json(doc: &Document, buffer_uri: Option<String>, buffer_len: usize) -> Value {
    let mut nodes: Vec<Value> = Vec::with_capacity(doc.skeleton.bones.len());
    for (id, bone) in doc.skeleton.bones.iter().enumerate() {
        let children: Vec<usize> = doc
            .skeleton
            .bones
            .iter()
            .enumerate()
            .filter(|(_, b)| b.parent == Some(id))
            .map(|(i, _)| i)
            .collect();
        let mut node = json!({
            "name": bone.name,
            "translation": bone.rest.translation.to_array(),
            "rotation": bone.rest.rotation.to_array(),
            "scale": bone.rest.scale.to_array(),
        });
        if !children.is_empty() {
            node["children"] = json!(children);
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
    root
}

/// Serialize `doc` to `path` (`.glb` for binary, anything else as
/// `.gltf` JSON with an embedded data-URI buffer): skeleton, animation,
/// and any scene assets it carries ([`Document::assets`] — triangulated
/// meshes, skins, factor-only materials; textures are downstream
/// pipelines' job). A `Document` with default-empty assets writes
/// animation + skeleton only.
pub fn write(doc: &Document, path: &Path) -> Result<(), WriteError> {
    let assets = &doc.assets;
    let mut buffers = BufferBuilder::new();
    let mut animations: Vec<Value> = Vec::new();

    for clip in &doc.clips {
        let mut samplers: Vec<Value> = Vec::new();
        let mut channels: Vec<Value> = Vec::new();
        for track in &clip.tracks {
            if track.times.is_empty() || track.bone >= doc.skeleton.bones.len() {
                continue;
            }
            let input = buffers.push(&track.times, "SCALAR", true);
            let output = match &track.values {
                TrackValues::Vec3s(v) => {
                    let flat: Vec<f32> = v.iter().flat_map(|x| x.to_array()).collect();
                    buffers.push(&flat, "VEC3", false)
                }
                TrackValues::Quats(v) => {
                    let flat: Vec<f32> = v.iter().flat_map(|q| q.to_array()).collect();
                    buffers.push(&flat, "VEC4", false)
                }
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
        if !channels.is_empty() {
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
            let flat: Vec<f32> = prim.positions.iter().flat_map(|v| v.to_array()).collect();
            let mut attributes = json!({
                // POSITION min/max is required by the spec.
                "POSITION": buffers.push(&flat, "VEC3", true),
            });
            if !prim.normals.is_empty() {
                let flat: Vec<f32> = prim.normals.iter().flat_map(|v| v.to_array()).collect();
                attributes["NORMAL"] = json!(buffers.push(&flat, "VEC3", false));
            }
            if !prim.uvs.is_empty() {
                let flat: Vec<f32> = prim.uvs.iter().flatten().copied().collect();
                attributes["TEXCOORD_0"] = json!(buffers.push(&flat, "VEC2", false));
            }
            if !prim.joints.is_empty() {
                let flat_j: Vec<u16> = prim.joints.iter().flatten().copied().collect();
                attributes["JOINTS_0"] = json!(buffers.push_u16(&flat_j, "VEC4"));
                let flat_w: Vec<f32> = prim.weights.iter().flatten().copied().collect();
                attributes["WEIGHTS_0"] = json!(buffers.push(&flat_w, "VEC4", false));
            }
            let mut value = json!({ "attributes": attributes });
            if !prim.indices.is_empty() {
                value["indices"] = json!(buffers.push_indices(&prim.indices));
            }
            if let Some(material) = prim.material {
                value["material"] = json!(material);
            }
            prims.push(value);
        }
        let mesh_index = meshes_json.len();
        meshes_json.push(json!({ "name": mesh.name, "primitives": prims }));

        let skin_index = if mesh.skin_joints.is_empty() {
            None
        } else {
            let mut ibms: Vec<f32> = Vec::with_capacity(mesh.skin_joints.len() * 16);
            for (slot, &joint) in mesh.skin_joints.iter().enumerate() {
                let m = mesh
                    .skin_ibms
                    .get(slot)
                    .copied()
                    .or_else(|| doc.skeleton.bones.get(joint).and_then(|b| b.inverse_bind))
                    .unwrap_or(glam::Mat4::IDENTITY);
                ibms.extend_from_slice(&m.to_cols_array());
            }
            let accessor = buffers.push(&ibms, "MAT4", false);
            let index = skins_json.len();
            skins_json.push(json!({
                "joints": mesh.skin_joints,
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
        node_attach.push((mesh.node, mesh_index, skin_index));
    }

    // Embedded base-color textures: raw encoded bytes as buffer views
    // (glTF never decodes; PNG/JPEG pass through untouched).
    let mut images_json: Vec<Value> = Vec::new();
    let mut textures_json: Vec<Value> = Vec::new();
    let mut material_texture_index: Vec<Option<usize>> = vec![None; assets.materials.len()];
    for (mi, material) in assets.materials.iter().enumerate() {
        if let Some(texture) = &material.base_color_texture {
            let view = buffers.push_view(&texture.bytes);
            let image = images_json.len();
            images_json.push(json!({ "bufferView": view, "mimeType": texture.mime }));
            material_texture_index[mi] = Some(textures_json.len());
            textures_json.push(json!({ "source": image }));
        }
    }

    let binary = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("glb"));

    let uri = if binary {
        None
    } else {
        Some(format!(
            "data:application/octet-stream;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(&buffers.bytes)
        ))
    };
    let mut root = document_to_json(doc, uri, buffers.bytes.len());
    // Present-but-empty accessor arrays are invalid glTF (minItems 1); an
    // empty document has none, so emit them only when populated.
    if !buffers.views.is_empty() {
        root["bufferViews"] = Value::Array(buffers.views);
    }
    if !buffers.accessors.is_empty() {
        root["accessors"] = Value::Array(buffers.accessors);
    }
    if !animations.is_empty() {
        root["animations"] = Value::Array(animations);
    }
    if !meshes_json.is_empty() {
        for (node, mesh_index, skin_index) in &node_attach {
            match skin_index {
                Some(skin) => {
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
                        json!({ "name": m.name, "pbrMetallicRoughness": pbr })
                    })
                    .collect(),
            );
            if !images_json.is_empty() {
                root["images"] = Value::Array(images_json);
                root["textures"] = Value::Array(textures_json);
            }
        }
    }

    let io_err = |e: std::io::Error| WriteError::Io {
        path: path.display().to_string(),
        source: e,
    };
    if binary {
        let mut json_bytes = serde_json::to_vec(&root)?;
        while !json_bytes.len().is_multiple_of(4) {
            json_bytes.push(b' ');
        }
        let mut bin = buffers.bytes;
        while !bin.len().is_multiple_of(4) {
            bin.push(0);
        }
        // Omit the BIN chunk entirely when there is no binary payload: a
        // zero-length chunk is GLB_EMPTY_CHUNK to the Khronos validator.
        let has_bin = !bin.is_empty();
        let bin_bytes = if has_bin { 8 + bin.len() } else { 0 };
        let total = 12 + 8 + json_bytes.len() + bin_bytes;
        let mut out = Vec::with_capacity(total);
        out.extend_from_slice(b"glTF");
        out.extend_from_slice(&2u32.to_le_bytes());
        out.extend_from_slice(&(total as u32).to_le_bytes());
        out.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(b"JSON");
        out.extend_from_slice(&json_bytes);
        if has_bin {
            out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
            out.extend_from_slice(b"BIN\0");
            out.extend_from_slice(&bin);
        }
        std::fs::write(path, out).map_err(io_err)
    } else {
        let text = serde_json::to_string_pretty(&root)?;
        std::fs::write(path, text).map_err(io_err)
    }
}
