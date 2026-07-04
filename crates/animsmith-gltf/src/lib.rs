//! glTF/GLB ingestion into the animsmith core model.
//!
//! Every node in the file becomes a core `Bone` (animations may target
//! any node; role resolution decides later which ones matter), in
//! topological order. Values are kept exactly as authored — no
//! quaternion renormalization, no resampling — so the mechanical checks
//! see the real data.
//!
//! Buffers are resolved without the `gltf` crate's `import` feature to
//! keep image decoding out of the dependency tree: GLB BIN chunks,
//! `data:` URIs, and sibling files are supported.

pub mod fix;
pub mod write;

use animsmith_core::model::{
    Bone, Clip, Document, Interpolation, Property, Skeleton, SourceInfo, Track, TrackValues,
    Transform,
};
use base64::Engine as _;
use glam::{Mat4, Quat, Vec3};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum LoadError {
    #[error("failed to read {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("glTF parse error: {0}")]
    Gltf(#[from] gltf::Error),
    #[error("buffer resolution failed: {0}")]
    Buffer(String),
}

/// `fix` = load + patch + write, and its error type says so: the load
/// phase (reading, parsing, buffer resolution) fails as [`LoadError`],
/// the write phase as [`WriteError`].
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FixError {
    #[error(transparent)]
    Load(#[from] LoadError),
    #[error(transparent)]
    Write(#[from] WriteError),
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum WriteError {
    #[error("failed to write {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to serialize glTF JSON: {0}")]
    Serialize(#[from] serde_json::Error),
}

/// Load a `.glb` or `.gltf` file into a core [`Document`].
pub fn load(path: &Path) -> Result<Document, LoadError> {
    let gltf = gltf::Gltf::open(path)?;
    let buffers = resolve_buffers(&gltf, path.parent())?;
    build_document(&gltf, &buffers, path)
}

fn resolve_buffers(gltf: &gltf::Gltf, base: Option<&Path>) -> Result<Vec<Vec<u8>>, LoadError> {
    let mut buffers = Vec::new();
    for buffer in gltf.buffers() {
        let data = match buffer.source() {
            gltf::buffer::Source::Bin => gltf
                .blob
                .clone()
                .ok_or_else(|| LoadError::Buffer("GLB has no BIN chunk".into()))?,
            gltf::buffer::Source::Uri(uri) => {
                if let Some(encoded) = uri.strip_prefix("data:") {
                    let payload =
                        encoded
                            .split_once("base64,")
                            .map(|(_, p)| p)
                            .ok_or_else(|| {
                                LoadError::Buffer(format!(
                                    "unsupported data URI in buffer: {uri:.40}"
                                ))
                            })?;
                    base64::engine::general_purpose::STANDARD
                        .decode(payload)
                        .map_err(|e| LoadError::Buffer(format!("bad base64 data URI: {e}")))?
                } else {
                    let path = base.unwrap_or(Path::new(".")).join(uri);
                    std::fs::read(&path).map_err(|source| LoadError::Io {
                        path: path.display().to_string(),
                        source,
                    })?
                }
            }
        };
        buffers.push(data);
    }
    Ok(buffers)
}

fn build_document(
    gltf: &gltf::Gltf,
    buffers: &[Vec<u8>],
    path: &Path,
) -> Result<Document, LoadError> {
    let doc = &gltf.document;

    // Parent map over ALL nodes (scene membership doesn't matter:
    // animations may target unreferenced subtrees).
    let node_count = doc.nodes().count();
    let mut parent: Vec<Option<usize>> = vec![None; node_count];
    for node in doc.nodes() {
        for child in node.children() {
            parent[child.index()] = Some(node.index());
        }
    }

    // Topological order: DFS from roots.
    let mut order: Vec<usize> = Vec::with_capacity(node_count);
    let mut stack: Vec<usize> = doc
        .nodes()
        .filter(|n| parent[n.index()].is_none())
        .map(|n| n.index())
        .collect();
    stack.reverse(); // keep file order among roots
    let nodes: Vec<gltf::Node> = doc.nodes().collect();
    while let Some(i) = stack.pop() {
        order.push(i);
        let children: Vec<usize> = nodes[i].children().map(|c| c.index()).collect();
        for &c in children.iter().rev() {
            stack.push(c);
        }
    }

    let mut bone_of_node: Vec<Option<usize>> = vec![None; node_count];
    for (bone_id, &node_index) in order.iter().enumerate() {
        bone_of_node[node_index] = Some(bone_id);
    }

    let mut bones: Vec<Bone> = Vec::with_capacity(node_count);
    for &node_index in &order {
        let node = &nodes[node_index];
        let (t, r, s) = node.transform().decomposed();
        bones.push(Bone {
            name: node
                .name()
                .map(str::to_owned)
                .unwrap_or_else(|| format!("node{node_index}")),
            parent: parent[node_index].and_then(|p| bone_of_node[p]),
            rest: Transform {
                translation: Vec3::from_array(t),
                rotation: Quat::from_array(r),
                scale: Vec3::from_array(s),
            },
            inverse_bind: None,
        });
    }

    // Inverse bind matrices from skins (last skin wins on conflict).
    for skin in doc.skins() {
        let reader = skin.reader(|buffer| buffers.get(buffer.index()).map(Vec::as_slice));
        if let Some(ibms) = reader.read_inverse_bind_matrices() {
            for (joint, ibm) in skin.joints().zip(ibms) {
                if let Some(bone_id) = bone_of_node[joint.index()] {
                    bones[bone_id].inverse_bind = Some(Mat4::from_cols_array_2d(&ibm));
                }
            }
        }
    }

    // Animations → clips. Unnamed clips get stable positional names.
    let mut clips = Vec::new();
    let mut name_uses: BTreeMap<String, usize> = BTreeMap::new();
    for animation in doc.animations() {
        let base_name = animation
            .name()
            .map(str::to_owned)
            .unwrap_or_else(|| format!("animation{}", animation.index()));
        let uses = name_uses.entry(base_name.clone()).or_insert(0);
        let name = if *uses == 0 {
            base_name.clone()
        } else {
            format!("{base_name}#{uses}")
        };
        *uses += 1;

        let mut tracks = Vec::new();
        let mut duration = 0.0f64;
        for channel in animation.channels() {
            let Some(bone) = bone_of_node[channel.target().node().index()] else {
                continue;
            };
            let reader = channel.reader(|buffer| buffers.get(buffer.index()).map(Vec::as_slice));
            let Some(times) = reader.read_inputs().map(|it| it.collect::<Vec<f32>>()) else {
                continue;
            };
            let (property, values) = match reader.read_outputs() {
                Some(gltf::animation::util::ReadOutputs::Translations(it)) => (
                    Property::Translation,
                    TrackValues::Vec3s(it.map(Vec3::from_array).collect()),
                ),
                Some(gltf::animation::util::ReadOutputs::Rotations(r)) => (
                    Property::Rotation,
                    TrackValues::Quats(r.into_f32().map(Quat::from_array).collect()),
                ),
                Some(gltf::animation::util::ReadOutputs::Scales(it)) => (
                    Property::Scale,
                    TrackValues::Vec3s(it.map(Vec3::from_array).collect()),
                ),
                // Morph-target weights are out of scope for the
                // skeletal check catalog (P2 revisits them).
                Some(gltf::animation::util::ReadOutputs::MorphTargetWeights(_)) | None => continue,
            };
            duration = duration.max(times.last().copied().unwrap_or(0.0) as f64);
            tracks.push(Track {
                bone,
                property,
                interpolation: match channel.sampler().interpolation() {
                    gltf::animation::Interpolation::Linear => Interpolation::Linear,
                    gltf::animation::Interpolation::Step => Interpolation::Step,
                    gltf::animation::Interpolation::CubicSpline => Interpolation::CubicSpline,
                },
                times,
                values,
            });
        }
        clips.push(Clip {
            name,
            duration_s: duration,
            tracks,
        });
    }

    Ok(Document {
        skeleton: Skeleton { bones },
        clips,
        source: SourceInfo {
            path: Some(path.display().to_string()),
            format: Some("gltf".into()),
        },
    })
}
