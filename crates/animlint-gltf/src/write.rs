//! Minimal glTF 2.0 writer for `convert`: emits the skeleton (node
//! hierarchy + rest TRS) and every clip's animation tracks. This is a
//! *pipeline* conversion — meshes, skins, and materials are not
//! carried; the output exists so animation data can enter glTF-based
//! tooling (including animlint itself) straight from a DCC export.
//!
//! Values are written exactly as held in the core model — lint first;
//! conversion does not repair.

use crate::LoadError;
use animlint_core::model::{Document, Interpolation, Property, TrackValues};
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
            "VEC3" => 3,
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

    let mut buffer = json!({ "byteLength": buffer_len });
    if let Some(uri) = buffer_uri {
        buffer["uri"] = json!(uri);
    }

    json!({
        "asset": {
            "version": "2.0",
            "generator": format!("animlint {}", env!("CARGO_PKG_VERSION")),
        },
        "scene": 0,
        "scenes": [{ "nodes": roots }],
        "nodes": nodes,
        "buffers": [buffer],
    })
}

/// Serialize `doc` to `path` (`.glb` for binary, anything else as
/// `.gltf` JSON with an embedded data-URI buffer).
pub fn write(doc: &Document, path: &Path) -> Result<(), LoadError> {
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
            let sampler = samplers.len();
            samplers.push(json!({
                "input": input,
                "output": output,
                "interpolation": match track.interpolation {
                    Interpolation::Linear => "LINEAR",
                    Interpolation::Step => "STEP",
                    Interpolation::CubicSpline => "CUBICSPLINE",
                },
            }));
            channels.push(json!({
                "sampler": sampler,
                "target": {
                    "node": track.bone,
                    "path": match track.property {
                        Property::Translation => "translation",
                        Property::Rotation => "rotation",
                        Property::Scale => "scale",
                    },
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
    root["bufferViews"] = Value::Array(buffers.views);
    root["accessors"] = Value::Array(buffers.accessors);
    if !animations.is_empty() {
        root["animations"] = Value::Array(animations);
    }

    let io_err = |e: std::io::Error| LoadError::Io {
        path: path.display().to_string(),
        source: e,
    };
    if binary {
        let mut json_bytes = serde_json::to_vec(&root).expect("glTF JSON serializes");
        while !json_bytes.len().is_multiple_of(4) {
            json_bytes.push(b' ');
        }
        let mut bin = buffers.bytes;
        while !bin.len().is_multiple_of(4) {
            bin.push(0);
        }
        let total = 12 + 8 + json_bytes.len() + 8 + bin.len();
        let mut out = Vec::with_capacity(total);
        out.extend_from_slice(b"glTF");
        out.extend_from_slice(&2u32.to_le_bytes());
        out.extend_from_slice(&(total as u32).to_le_bytes());
        out.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(b"JSON");
        out.extend_from_slice(&json_bytes);
        out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
        out.extend_from_slice(b"BIN\0");
        out.extend_from_slice(&bin);
        std::fs::write(path, out).map_err(io_err)
    } else {
        let text = serde_json::to_string_pretty(&root).expect("glTF JSON serializes");
        std::fs::write(path, text).map_err(io_err)
    }
}
