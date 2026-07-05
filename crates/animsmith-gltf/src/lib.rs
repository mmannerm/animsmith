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

#![doc = "\n\n"]
#![doc = include_str!("../README.md")]

pub mod fix;
pub mod write;

use animsmith_core::model::{
    Bone, Clip, Document, Interpolation, MaterialAsset, MeshAsset, Primitive, Property,
    SceneAssets, Skeleton, SourceInfo, TextureAsset, Track, TrackValues, Transform,
};
use base64::Engine as _;
use glam::{Mat4, Quat, Vec3};
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

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
    #[error("malformed animation data: {0}")]
    Malformed(String),
}

/// `fix` errors are classified by defect, not by phase: [`LoadError`]
/// means the *input* was unreadable or malformed (even when detected
/// while assembling the output, e.g. re-deriving GLB chunk bounds or
/// validating an input-supplied buffer URI); [`WriteError`] means
/// emitting the output failed.
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

/// Contain an external-buffer URI to a relative child path: absolute
/// paths, `..`, backslashes, and non-normal components are rejected.
/// URIs are used verbatim (no percent-decoding), so encoded traversal
/// sequences stay literal path characters and cannot escape either.
pub(crate) fn safe_external_buffer_path(uri: &str) -> Result<PathBuf, LoadError> {
    if uri.is_empty() || uri.contains('\\') {
        return Err(LoadError::Buffer(format!(
            "unsafe external buffer URI {uri:?}: expected a relative child path"
        )));
    }
    let path = Path::new(uri);
    if path.is_absolute() {
        return Err(LoadError::Buffer(format!(
            "unsafe external buffer URI {uri:?}: absolute paths are not supported"
        )));
    }
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => out.push(part),
            _ => {
                return Err(LoadError::Buffer(format!(
                    "unsafe external buffer URI {uri:?}: expected a relative child path"
                )));
            }
        }
    }
    if out.as_os_str().is_empty() {
        return Err(LoadError::Buffer(format!(
            "unsafe external buffer URI {uri:?}: expected a relative child path"
        )));
    }
    Ok(out)
}

/// Structural validation for one animation channel: key/value counts
/// must agree (x3 for CUBICSPLINE's [in-tangent, value, out-tangent]
/// triplets) and a track must have at least one key. Violations are
/// container-level malformation -> [`LoadError::Malformed`], exit 2 at
/// the CLI; semantic problems (NaN, flips, seams) stay findings.
fn validate_track_lengths(
    clip: &str,
    node: usize,
    interpolation: Interpolation,
    times: &[f32],
    values: &TrackValues,
) -> Result<(), LoadError> {
    if times.is_empty() {
        return Err(LoadError::Malformed(format!(
            "clip '{clip}' node {node}: animation channel with zero keyframes"
        )));
    }
    let per_key = match interpolation {
        Interpolation::CubicSpline => 3,
        _ => 1,
    };
    let expected = times.len() * per_key;
    let actual = match values {
        TrackValues::Vec3s(v) => v.len(),
        TrackValues::Quats(v) => v.len(),
    };
    if actual != expected {
        return Err(LoadError::Malformed(format!(
            "clip '{clip}' node {node}: {} keyframe times but {actual} output values (expected {expected})",
            times.len()
        )));
    }
    Ok(())
}

/// Load a `.glb` or `.gltf` file into a core [`Document`], including the
/// scene assets (meshes, skins, materials) its geometry describes — the
/// symmetric read side of [`write::write`], and the same one-call shape
/// `animsmith_fbx::load` uses. Consumers that judge only animation
/// (`lint`, `inspect`) simply ignore [`Document::assets`].
pub fn load(path: &Path) -> Result<Document, LoadError> {
    let gltf = gltf::Gltf::open(path)?;
    let buffers = resolve_buffers(&gltf, path.parent())?;
    let mut doc = build_document(&gltf, &buffers, path)?;
    doc.assets = extract_assets(&gltf.document, &buffers, path.parent())?;
    Ok(doc)
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
                    let path = base
                        .unwrap_or(Path::new("."))
                        .join(safe_external_buffer_path(uri)?);
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

    let nodes: Vec<gltf::Node> = doc.nodes().collect();
    let (order, parent, bone_of_node) = topology(doc);

    let mut bones: Vec<Bone> = Vec::with_capacity(nodes.len());
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
            // Reject zero-count sampler accessors before reading: the
            // `gltf` reader underflows on a count-0 accessor (panics in
            // its accessor iterator), so this guard is what keeps a
            // hostile file from crashing the loader.
            let sampler = channel.sampler();
            if sampler.input().count() == 0 || sampler.output().count() == 0 {
                return Err(LoadError::Malformed(format!(
                    "clip '{name}' node {}: animation channel with zero keyframes",
                    channel.target().node().index()
                )));
            }
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
            let interpolation = match channel.sampler().interpolation() {
                gltf::animation::Interpolation::Linear => Interpolation::Linear,
                gltf::animation::Interpolation::Step => Interpolation::Step,
                gltf::animation::Interpolation::CubicSpline => Interpolation::CubicSpline,
            };
            validate_track_lengths(
                &name,
                channel.target().node().index(),
                interpolation,
                &times,
                &values,
            )?;
            duration = duration.max(times.last().copied().unwrap_or(0.0) as f64);
            tracks.push(Track {
                bone,
                property,
                interpolation,
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
        // `build_document` covers skeleton + animation; `load` fills
        // `assets` from `extract_assets` before returning.
        assets: SceneAssets::default(),
        source: SourceInfo {
            path: Some(path.display().to_string()),
            format: Some("gltf".into()),
        },
    })
}

/// Node-index → bone-id map (plus the raw parent array and DFS order),
/// in the topological order `build_document` assigns bones: DFS from
/// roots, file order among siblings, over ALL nodes (scene membership
/// doesn't matter — animations may target unreferenced subtrees). Shared
/// by the skeleton build and asset extraction so both agree on which
/// bone a node became.
fn topology(doc: &gltf::Document) -> (Vec<usize>, Vec<Option<usize>>, Vec<Option<usize>>) {
    let node_count = doc.nodes().count();
    let mut parent: Vec<Option<usize>> = vec![None; node_count];
    for node in doc.nodes() {
        for child in node.children() {
            parent[child.index()] = Some(node.index());
        }
    }

    let nodes: Vec<gltf::Node> = doc.nodes().collect();
    let mut order: Vec<usize> = Vec::with_capacity(node_count);
    let mut stack: Vec<usize> = doc
        .nodes()
        .filter(|n| parent[n.index()].is_none())
        .map(|n| n.index())
        .collect();
    stack.reverse(); // keep file order among roots
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
    (order, parent, bone_of_node)
}

/// Parse meshes (indexed or unindexed), skins (joints + inverse bind
/// matrices), and materials (PBR factors + embedded base-color texture)
/// into the core [`SceneAssets`] model — the symmetric read side of
/// [`write::write`], mirroring `animsmith-fbx`'s `extract_assets`.
///
/// Vertex data is kept exactly as authored: glTF is already triangulated
/// and Y-up, so unlike the FBX path there is no triangulation, unit
/// conversion, or UV flip — `measure` sees the real bytes. Materials
/// keep their glTF array index so a primitive's `material()` index maps
/// straight into `assets.materials`.
fn extract_assets(
    doc: &gltf::Document,
    buffers: &[Vec<u8>],
    base: Option<&Path>,
) -> Result<SceneAssets, LoadError> {
    let mut assets = SceneAssets::default();

    // `doc.materials()` yields defined materials in index order (the
    // synthesized default material has no index and is skipped), so
    // pushing in iteration order keeps `assets.materials[i]` aligned
    // with glTF material index `i`.
    for material in doc.materials() {
        if material.index().is_none() {
            continue;
        }
        let pbr = material.pbr_metallic_roughness();
        let base_color_texture = pbr
            .base_color_texture()
            .and_then(|info| read_image(info.texture().source().source(), buffers, base));
        assets.materials.push(MaterialAsset {
            name: material.name().unwrap_or("material").to_string(),
            base_color: pbr.base_color_factor(),
            metallic: pbr.metallic_factor(),
            roughness: pbr.roughness_factor(),
            base_color_texture,
        });
    }

    let (_order, _parent, bone_of_node) = topology(doc);

    for node in doc.nodes() {
        let Some(mesh) = node.mesh() else { continue };
        let node_bone = bone_of_node[node.index()].unwrap_or(0);

        let skin = node.skin();
        // Skin joints are node indices in the file; map them into bone
        // ids so they index the core skeleton, matching the writer,
        // which emits joints in bone order.
        let skin_joints: Vec<usize> = skin
            .as_ref()
            .map(|s| {
                s.joints()
                    .map(|j| bone_of_node[j.index()].unwrap_or(0))
                    .collect()
            })
            .unwrap_or_default();
        // gltf 1.4's accessor iterator underflows (panics) on a count-0
        // accessor — the same bug the animation path guards before
        // reading. Only read an inverse-bind accessor that has entries.
        let skin_ibms: Vec<Mat4> = skin
            .as_ref()
            .filter(|s| s.inverse_bind_matrices().is_some_and(|a| a.count() > 0))
            .map(|s| {
                let reader = s.reader(|b| buffers.get(b.index()).map(Vec::as_slice));
                reader
                    .read_inverse_bind_matrices()
                    .map(|it| it.map(|m| Mat4::from_cols_array_2d(&m)).collect())
                    .unwrap_or_default()
            })
            .unwrap_or_default();

        let mut primitives = Vec::new();
        for prim in mesh.primitives() {
            // Only triangle lists are ingested. The core model and the
            // writer are triangle-only (no primitive `mode` field), and
            // measure/checks assume triangulated geometry; a points/
            // lines/strip/fan primitive read as a triangle list would be
            // silently corrupted, so skip it rather than misinterpret it.
            // Skinned rigs — the target inputs — are triangle lists.
            if prim.mode() != gltf::mesh::Mode::Triangles {
                continue;
            }
            let reader = prim.reader(|b| buffers.get(b.index()).map(Vec::as_slice));
            // Never iterate a count-0 accessor: gltf 1.4's reader
            // underflows and panics on one (invariant: hostile input must
            // not crash the loader). Treat a zero-count attribute as
            // absent, and skip a primitive whose POSITION is missing or
            // empty — a primitive without positions carries no geometry.
            let has = |sem: gltf::Semantic| prim.get(&sem).is_some_and(|a| a.count() > 0);
            if !has(gltf::Semantic::Positions) {
                continue;
            }
            let positions: Vec<Vec3> = reader
                .read_positions()
                .map(|it| it.map(Vec3::from_array).collect())
                .unwrap_or_default();
            let normals = if has(gltf::Semantic::Normals) {
                reader
                    .read_normals()
                    .map(|it| it.map(Vec3::from_array).collect())
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            let uvs = if has(gltf::Semantic::TexCoords(0)) {
                reader
                    .read_tex_coords(0)
                    .map(|tc| tc.into_f32().collect())
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            // JOINTS_0/WEIGHTS_0 come as a pair; keep them parallel.
            let (joints, weights) =
                if has(gltf::Semantic::Joints(0)) && has(gltf::Semantic::Weights(0)) {
                    match (reader.read_joints(0), reader.read_weights(0)) {
                        (Some(j), Some(w)) => (j.into_u16().collect(), w.into_f32().collect()),
                        _ => (Vec::new(), Vec::new()),
                    }
                } else {
                    (Vec::new(), Vec::new())
                };
            let indices = if prim.indices().is_some_and(|a| a.count() > 0) {
                reader
                    .read_indices()
                    .map(|it| it.into_u32().collect())
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            primitives.push(Primitive {
                material: prim.material().index(),
                indices,
                positions,
                normals,
                uvs,
                joints,
                weights,
            });
        }
        if primitives.is_empty() {
            continue;
        }

        assets.meshes.push(MeshAsset {
            name: mesh.name().unwrap_or("mesh").to_string(),
            node: node_bone,
            primitives,
            skin_joints,
            skin_ibms,
        });
    }

    Ok(assets)
}

/// Read an embedded glTF image into a [`TextureAsset`] (raw encoded
/// bytes + MIME; glTF never decodes, so PNG/JPEG pass through
/// untouched). Buffer-view and `data:` URI sources are supported (what
/// the writer and typical GLB exports use); an external-file source is
/// read relative to `base`. A texture whose bytes can't be resolved
/// yields `None` — an absent texture is missing measurement data, not a
/// load failure.
fn read_image(
    source: gltf::image::Source,
    buffers: &[Vec<u8>],
    base: Option<&Path>,
) -> Option<TextureAsset> {
    match source {
        gltf::image::Source::View { view, mime_type } => {
            let buffer = buffers.get(view.buffer().index())?;
            // `offset`/`length` are attacker-controlled `byteOffset`/
            // `byteLength` JSON fields; add with a checked op so a
            // near-`usize::MAX` offset fails closed instead of panicking
            // on overflow in debug builds (invariant: loaders never
            // panic on hostile input).
            let end = view.offset().checked_add(view.length())?;
            let bytes = buffer.get(view.offset()..end)?.to_vec();
            Some(TextureAsset {
                bytes,
                mime: mime_type.to_string(),
            })
        }
        gltf::image::Source::Uri { uri, mime_type } => {
            if let Some(encoded) = uri.strip_prefix("data:") {
                let (meta, payload) = encoded.split_once("base64,")?;
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(payload)
                    .ok()?;
                let mime = mime_type
                    .map(str::to_string)
                    .unwrap_or_else(|| meta.trim_end_matches(';').to_string());
                Some(TextureAsset { bytes, mime })
            } else {
                let path = base?.join(safe_external_buffer_path(uri).ok()?);
                let bytes = std::fs::read(path).ok()?;
                Some(TextureAsset {
                    bytes,
                    mime: mime_type.unwrap_or_default().to_string(),
                })
            }
        }
    }
}
