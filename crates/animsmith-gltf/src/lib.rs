//! [`load`] reads `.gltf`/`.glb` files into an
//! [`animsmith_core::Document`], [`write::write`] emits a document as
//! glTF/GLB, and the [`fix`] module provides byte-surgical quaternion
//! repairs. Malformed inputs report [`LoadError`]; output failures
//! report [`WriteError`].
//!
//! This crate is the glTF/GLB format edge around `animsmith-core`.
//! Loading preserves authored animation values for checks and also carries
//! meshes, skins, materials, and embedded textures into
//! [`Document::assets`](animsmith_core::model::Document::assets).
//! Writing is a model round-trip for `convert` and `transform`; use
//! [`fix::FixSession`] when a repair must preserve every non-animation byte
//! of the original container.
//!
//! # Quick start
//!
//! Load a document and run the shared core checks:
//!
//! ```no_run
//! fn lint_clip(
//!     path: &std::path::Path,
//! ) -> Result<Vec<animsmith_core::Finding>, Box<dyn std::error::Error>> {
//!     let doc = animsmith_gltf::load(path)?;
//!     let roles = animsmith_core::detect_profile(&doc.skeleton).unwrap_or_default();
//!     let config = animsmith_core::Config::default();
//!     let grids = animsmith_core::MetricGrids::new(&doc);
//!     let ctx = animsmith_core::CheckCtx::new(&grids, &roles, &config);
//!     let results = animsmith_core::evaluate_checks(
//!         &ctx,
//!         &animsmith_core::all_checks(),
//!         animsmith_core::CheckSelection::All,
//!     )?;
//!     Ok(results
//!         .into_iter()
//!         .flat_map(|check| check.findings().to_vec())
//!         .collect())
//! }
//! ```
//!
//! Compose byte-surgical repairs through one session:
//!
//! ```no_run
//! fn repair_quaternions(
//!     input: &std::path::Path,
//!     output: &std::path::Path,
//! ) -> Result<(), Box<dyn std::error::Error>> {
//!     use animsmith_gltf::fix::{FixSession, Repair};
//!
//!     let mut session = FixSession::read(input)?;
//!     session.apply(Repair::QuatNorm);
//!     session.apply(Repair::QuatFlip);
//!     session.write(input, output)?;
//!     Ok(())
//! }
//! ```
//!
//! # Build and API status
//!
//! This crate has no public feature flags and supports the workspace MSRV,
//! Rust 1.88. Its Rust API is pre-1.0; see `animsmith-core`'s crate-level API
//! status for the shared stability boundary.
//!
//! See the GitHub [embedding guide] for crate selection and the [pipeline
//! scenario guide] for raw-to-game-ready workflows.
//!
//! [embedding guide]: https://github.com/mmannerm/animsmith/blob/main/docs/embedding.md
//! [pipeline scenario guide]: https://github.com/mmannerm/animsmith/blob/main/docs/pipeline-scenarios.md
//!
#![warn(missing_docs)]

pub mod fix;
pub mod write;

use animsmith_core::model::{
    AdditionalInfluenceSet, Bone, Clip, DecodedImageColorType, Document, ImageContainerFormat,
    ImageSourceKind, ImageUnavailableReason, Interpolation, MaterialAsset, MaterialResourceAssets,
    MaterialResourceCoverage, MaterialTextureSlot, MeshAsset, MeshInstance, NormalTextureAsset,
    OcclusionTextureAsset, Primitive, Property, SceneAsset, SceneAssets, Skeleton,
    SourceImageAsset, SourceImageInspection, SourceInfo, SourceInverseBindAccessor,
    SourceInverseBindAccessorStatus, SourceMaterialAsset, SourceMaterialTextureBinding,
    SourceNodeAsset, SourceNodeLocalRest, SourceSkeletonAssets, SourceSkeletonCoverage,
    SourceSkinAsset, SourceSkinAttachment, SourceTextureAsset, TextureAsset, Track, TrackValues,
    Transform,
};
use base64::Engine as _;
use glam::{Mat4, Quat, Vec3};
use image::{ColorType, ImageError, ImageFormat, ImageReader, Limits};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Cursor, Read};
use std::path::{Component, Path, PathBuf};

/// Errors returned while loading `.gltf` or `.glb` input.
///
/// These are structural or operator errors. Semantic animation defects,
/// such as non-unit quaternions or seam pops, load successfully and are
/// reported by `animsmith-core` checks.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum LoadError {
    /// The source file or one of its external buffers could not be read.
    #[error("failed to read {path}: {source}")]
    Io {
        /// Path that failed to read.
        path: String,
        /// Underlying filesystem error.
        source: std::io::Error,
    },
    /// The `gltf` parser rejected the container.
    #[error("glTF parse error: {0}")]
    Gltf(#[from] gltf::Error),
    /// Buffer resolution or GLB framing failed.
    #[error("buffer resolution failed: {0}")]
    Buffer(String),
    /// Animation data is structurally malformed.
    #[error("malformed animation data: {0}")]
    Malformed(String),
    /// The node graph is not a forest that can become a skeleton.
    #[error("malformed node graph: {0}")]
    Topology(String),
}

/// `fix` errors are classified by defect, not by phase: [`LoadError`]
/// means the *input* was unreadable or malformed (even when detected
/// while assembling the output, e.g. re-deriving GLB chunk bounds or
/// validating an input-supplied buffer URI); [`WriteError`] means
/// emitting the output failed.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FixError {
    /// The input container could not be read, parsed, or safely framed.
    #[error(transparent)]
    Load(#[from] LoadError),
    /// The patched output container could not be emitted.
    #[error(transparent)]
    Write(#[from] WriteError),
}

/// Errors returned while writing a core document as glTF/GLB.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum WriteError {
    /// The output file could not be written.
    #[error("failed to write {path}: {source}")]
    Io {
        /// Path that failed to write.
        path: String,
        /// Underlying filesystem error.
        source: std::io::Error,
    },
    /// glTF JSON serialization failed.
    #[error("failed to serialize glTF JSON: {0}")]
    Serialize(#[from] serde_json::Error),
    /// A GLB length field would exceed the format's `u32` byte limit.
    #[error(
        "GLB too large: {field} is {bytes} bytes, exceeding the 4 GiB limit of a GLB u32 length field"
    )]
    TooLarge {
        /// Name of the GLB length field or chunk that overflowed.
        field: &'static str,
        /// Actual byte count that could not fit in the GLB field.
        bytes: usize,
    },
}

/// Contain an external-resource URI to a relative child path: absolute
/// paths, `..`, backslashes, and non-normal components are rejected after
/// percent-decoding. Encoded path separators are rejected rather than treated
/// as hierarchy, so every accepted separator was present in the source URI.
pub(crate) fn safe_external_buffer_path(uri: &str) -> Result<PathBuf, LoadError> {
    let decoded = decode_external_uri_path(uri)?;
    if decoded.is_empty() || decoded.contains('\\') {
        return Err(LoadError::Buffer(format!(
            "unsafe external buffer URI {uri:?}: expected a relative child path"
        )));
    }
    let path = Path::new(&decoded);
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

/// Decode percent-encoded bytes in a URI path without permitting an escape to
/// introduce a path separator. glTF URIs are Unicode strings, so decoded bytes
/// must form UTF-8 before they become an OS path.
fn decode_external_uri_path(uri: &str) -> Result<String, LoadError> {
    let bytes = uri.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'%' {
            decoded.push(bytes[index]);
            index += 1;
            continue;
        }
        let Some((&high, &low)) = bytes.get(index + 1).zip(bytes.get(index + 2)) else {
            return Err(unsafe_external_uri(uri));
        };
        let Some(value) = hex_value(high)
            .zip(hex_value(low))
            .map(|(high, low)| high << 4 | low)
        else {
            return Err(unsafe_external_uri(uri));
        };
        if matches!(value, b'/' | b'\\' | 0) {
            return Err(unsafe_external_uri(uri));
        }
        decoded.push(value);
        index += 3;
    }
    String::from_utf8(decoded).map_err(|_| unsafe_external_uri(uri))
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn unsafe_external_uri(uri: &str) -> LoadError {
    LoadError::Buffer(format!(
        "unsafe external buffer URI {uri:?}: expected a relative child path"
    ))
}

/// Reject a GLB whose 12-byte header declares a total length the file
/// can't back, *before* handing the bytes to the `gltf` container parser.
/// That parser computes `declared_len - HEADER_LEN`: a length below the
/// header size underflows (panics under overflow checks, e.g. every debug
/// build and `cargo test`), and a length past EOF drives a length-field
/// allocation — both invariant-1 violations on arbitrary input. Plain
/// glTF JSON (no `glTF` magic) passes through untouched. Found by the
/// `gltf_load` / `gltf_fix_quat_hemisphere` fuzz targets (see `fuzz/`).
pub(crate) fn validate_glb_framing(bytes: &[u8]) -> Result<(), LoadError> {
    const GLB_MAGIC: &[u8; 4] = b"glTF";
    const GLB_HEADER_LEN: usize = 12;
    if !bytes.starts_with(GLB_MAGIC) {
        return Ok(());
    }
    if bytes.len() < GLB_HEADER_LEN {
        return Err(LoadError::Buffer(
            "truncated GLB: file ends before the 12-byte header".into(),
        ));
    }
    let declared =
        u32::from_le_bytes(bytes[8..12].try_into().expect("slice has four bytes")) as usize;
    if declared < GLB_HEADER_LEN || declared > bytes.len() {
        return Err(LoadError::Buffer(format!(
            "GLB header declares {declared} bytes but the file is {}",
            bytes.len()
        )));
    }
    Ok(())
}

/// Reject animation data the `gltf` crate leaves un-validated but then
/// panics on. Its hand-written `Animation::validate` checks samplers and
/// the sampler *index*, but not the pieces below — each slips past
/// `Gltf::from_slice`'s validation and crashes a high-level getter on
/// arbitrary input (invariant-1). Found by the `gltf_load` /
/// `gltf_fix_quat_hemisphere` fuzz targets (see `fuzz/`).
///
/// - An unknown `target.path` (`Checked::Invalid`) or out-of-range
///   `target.node`: `Target::property()` / `Target::node()` both
///   `.unwrap()`.
/// - A sampler `output` accessor typed `UNSIGNED_INT`: no valid animation
///   output is ever U32, and `read_outputs` has no arm for it — it hits
///   an `unreachable!()`. (Truly invalid component types are already
///   rejected by the derived accessor validation `from_slice` runs; only
///   this spec-valid-but-nonsensical one leaks through.)
pub(crate) fn validate_animation_channels(root: &gltf::json::Root) -> Result<(), LoadError> {
    use gltf::json::accessor::ComponentType;
    use gltf::json::validation::Checked;
    let node_count = root.nodes.len();
    for (ai, anim) in root.animations.iter().enumerate() {
        for (ci, channel) in anim.channels.iter().enumerate() {
            if matches!(channel.target.path, Checked::Invalid) {
                return Err(LoadError::Malformed(format!(
                    "animation {ai} channel {ci}: unknown target path"
                )));
            }
            if channel.target.node.value() >= node_count {
                return Err(LoadError::Malformed(format!(
                    "animation {ai} channel {ci}: target node index {} out of range ({node_count} nodes)",
                    channel.target.node.value()
                )));
            }
        }
        for (si, sampler) in anim.samplers.iter().enumerate() {
            if let Some(accessor) = root.accessors.get(sampler.output.value())
                && matches!(
                    accessor.component_type,
                    Checked::Valid(ct) if ct.0 == ComponentType::U32
                )
            {
                return Err(LoadError::Malformed(format!(
                    "animation {ai} sampler {si}: output accessor has an unsupported UNSIGNED_INT component type"
                )));
            }
        }
    }
    Ok(())
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
/// scene assets (meshes, skins, materials, and embedded base-color and normal textures)
/// its geometry describes — the
/// symmetric read side of [`write::write`], and the same one-call shape
/// `animsmith_fbx::load` uses. Consumers that judge only animation
/// (`lint`, `inspect`) simply ignore [`Document::assets`].
/// Non-triangle primitives are skipped rather than reinterpreted.
///
/// # Errors
///
/// Returns [`LoadError`] for unreadable files, unsafe or missing external
/// buffers, malformed GLB framing, parser rejection, structurally invalid
/// animation channels, or node graphs that cannot be represented as a
/// skeleton forest.
pub fn load(path: &Path) -> Result<Document, LoadError> {
    // Read the whole file, then parse from the slice rather than via
    // `Gltf::open`: the reader path (`Glb::from_reader`) trusts the GLB
    // header's declared length and pre-allocates `vec![0; declared_len]`
    // before reading a byte, so a spoofed length OOMs on tiny input. The
    // slice path validates the declared length against the bytes actually
    // present, keeping malformed containers within invariant-1 (LoadError,
    // never an unbounded allocation). This mirrors what `fix` already does.
    let bytes = std::fs::read(path).map_err(|source| LoadError::Io {
        path: path.display().to_string(),
        source,
    })?;
    validate_glb_framing(&bytes)?;
    let gltf = gltf::Gltf::from_slice(&bytes)?;
    validate_animation_channels(gltf.document.as_json())?;
    let buffers = resolve_buffers(&gltf, path.parent())?;
    // Derive the node topology once and share it: the skeleton build and
    // asset extraction must agree on which bone each node became, and it is
    // also where malformed graphs are rejected (so that runs once too).
    let topo = topology(&gltf.document)?;
    let source_skeleton = extract_source_skeleton(&gltf.document, &buffers, &topo);
    let mut doc = build_document(&gltf, &buffers, path, &topo)?;
    doc.assets = extract_assets(&gltf.document, &buffers, path.parent(), &topo.bone_of_node);
    doc.assets.scenes = extract_scenes(&gltf.document, &topo.bone_of_node);
    doc.assets.default_scene = gltf.document.default_scene().map(|scene| scene.index());
    doc.assets.source_skeleton = source_skeleton;
    Ok(doc)
}

pub(crate) fn resolve_buffers(
    gltf: &gltf::Gltf,
    base: Option<&Path>,
) -> Result<Vec<Vec<u8>>, LoadError> {
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
    topo: &Topology,
) -> Result<Document, LoadError> {
    let doc = &gltf.document;

    let nodes: Vec<gltf::Node> = doc.nodes().collect();
    let Topology {
        order,
        parent,
        bone_of_node,
    } = topo;

    let mut bones: Vec<Bone> = Vec::with_capacity(nodes.len());
    for &node_index in order {
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

    // Existing compatibility representation: one bone can carry only one
    // inverse bind, so the last source skin wins here. `source_skeleton`
    // retains the complete per-skin source evidence for measurements.
    for skin in doc.skins() {
        // Skip a count-0 IBM accessor: gltf 1.4's reader underflows and
        // panics iterating one (the same guard the asset path uses).
        if skin
            .inverse_bind_matrices()
            .is_none_or(|accessor| accessor.count() == 0)
        {
            continue;
        }
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
            duration = times
                .iter()
                .copied()
                .filter(|time| time.is_finite())
                .map(f64::from)
                .fold(duration, f64::max);
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

/// The node-graph derivation [`topology`] produces once per load, shared
/// by the skeleton build and asset extraction so both agree on which bone
/// a node became. All three arrays are indexed by glTF node index.
struct Topology {
    /// Node indices in bone order: DFS from roots, file order among
    /// siblings — the order `build_document` assigns bone ids in.
    order: Vec<usize>,
    /// Each node's parent node index (`None` for roots), as reached by the
    /// DFS — always pushed to `order` before the child.
    parent: Vec<Option<usize>>,
    /// Each node's assigned bone id. `Some` for every node after a
    /// successful `topology` (all nodes are reached); the `Option` keeps
    /// index alignment and lets consumers skip gracefully.
    bone_of_node: Vec<Option<usize>>,
}

/// Derives the bone [`Topology`] from the glTF node graph: a DFS from the
/// roots, file order among siblings, over ALL nodes (scene membership
/// doesn't matter — animations may target unreferenced subtrees). This is
/// the order `build_document` assigns bone ids in.
///
/// glTF requires the node graph to be a forest. A malformed file can
/// break that two ways, and both are rejected as [`LoadError::Topology`]
/// rather than silently repaired — recovering would force an arbitrary
/// choice (which of two parents a node inherits, or dropping a cyclic
/// subtree) that quietly corrupts every downstream world transform:
///
/// - **Duplicate parent** — a node claimed as a child by more than one
///   node. Caught by the reference count below, before any traversal.
/// - **Cycle** — a closed loop. A cycle *reachable* from a root gives its
///   entry node a second parent, so it is caught by the duplicate-parent
///   check above. A *rootless* cycle has no root to descend from, so the
///   DFS never enters it and its nodes stay unreached — caught by the
///   post-DFS reachability check. Either way the DFS never actually walks
///   a cycle.
///
/// Both checks are O(nodes + edges). Because duplicate parents are
/// rejected first, every surviving node has at most one parent, so the
/// DFS reaches each node at most once and cannot loop — the walk is
/// bounded without relying on cycle detection mid-traversal, keeping
/// hostile input within invariant-1 (a `LoadError`, never a panic or
/// OOM). The `gltf_load` fuzz target (cycle → OOM under the old
/// best-effort recovery) and the audit (multi-parent → bad FK) motivated
/// the hardening.
fn topology(doc: &gltf::Document) -> Result<Topology, LoadError> {
    let node_count = doc.nodes().count();
    // Count parent claims per node. A forest allows at most one; two or
    // more is a duplicate-parent malformation. Also drives root detection:
    // a node with zero claims is a root.
    // `child.index()` is in range: `Gltf::from_slice` validates node child
    // indices. `saturating_add` keeps the count panic-free even on a
    // pathological file-derived edge multiplicity (invariant-1); any value
    // above 1 is a duplicate parent regardless.
    let mut parent_refs: Vec<u32> = vec![0; node_count];
    for node in doc.nodes() {
        for child in node.children() {
            let refs = &mut parent_refs[child.index()];
            *refs = refs.saturating_add(1);
        }
    }
    if let Some(dup) = parent_refs.iter().position(|&refs| refs > 1) {
        return Err(LoadError::Topology(format!(
            "node {dup} is a child of {} nodes; glTF requires a forest (one parent per node)",
            parent_refs[dup]
        )));
    }

    let nodes: Vec<gltf::Node> = doc.nodes().collect();
    let mut order: Vec<usize> = Vec::with_capacity(node_count);
    let mut parent: Vec<Option<usize>> = vec![None; node_count];
    let mut stack: Vec<usize> = doc
        .nodes()
        .filter(|n| parent_refs[n.index()] == 0)
        .map(|n| n.index())
        .collect();
    stack.reverse(); // keep file order among roots
    // DFS records `parent` as the node it reached the child *through*,
    // which was pushed to `order` before the child — keeping every
    // parent's bone id below its children's, the ordering `sample_clip`'s
    // single ascending FK pass relies on. With duplicate parents already
    // rejected, each child has exactly one parent, so this is unambiguous.
    // The `visited` re-entry guard is defensive: that same one-parent
    // property means each node is pushed at most once, so the guard is not
    // normally hit — it keeps the walk self-bounding if that upstream
    // guarantee is ever weakened.
    let mut visited: Vec<bool> = vec![false; node_count];
    while let Some(i) = stack.pop() {
        if visited[i] {
            continue;
        }
        visited[i] = true;
        order.push(i);
        let children: Vec<usize> = nodes[i].children().map(|c| c.index()).collect();
        for &c in children.iter().rev() {
            parent[c] = Some(i);
            stack.push(c);
        }
    }

    // Any node the DFS never reached has a parent (it is not a root) yet no
    // root-anchored path — it is trapped in a rootless cycle. (A cycle
    // reachable from a root can't reach here: its entry node has two
    // parents and was rejected above.) Reject rather than load a partial
    // skeleton silently missing those bones.
    if order.len() != node_count {
        let orphan = (0..node_count).find(|&n| !visited[n]).unwrap();
        return Err(LoadError::Topology(format!(
            "node {orphan} is unreachable from any root; the node graph contains a cycle"
        )));
    }

    let mut bone_of_node: Vec<Option<usize>> = vec![None; node_count];
    for (bone_id, &node_index) in order.iter().enumerate() {
        bone_of_node[node_index] = Some(bone_id);
    }
    Ok(Topology {
        order,
        parent,
        bone_of_node,
    })
}

/// Extract source-order skeleton evidence without conflating it with the
/// parent-before-child core skeleton used for sampling.
///
/// This reads every source node and skin, including skin attachments whose
/// mesh definition is later skipped by the triangle-only asset importer.
/// Inverse-bind accessor failures are source evidence rather than load errors:
/// callers can measure a parseable file's incomplete or malformed binding
/// declaration without silently falling back to a bone-level matrix.
fn extract_source_skeleton(
    doc: &gltf::Document,
    buffers: &[Vec<u8>],
    topo: &Topology,
) -> SourceSkeletonAssets {
    let mut attachments = vec![Vec::new(); doc.skins().count()];
    for node in doc.nodes() {
        let Some(skin) = node.skin() else {
            continue;
        };
        let Some(bone) = topo.bone_of_node.get(node.index()).copied().flatten() else {
            return SourceSkeletonAssets::default();
        };
        let Some(for_skin) = attachments.get_mut(skin.index()) else {
            return SourceSkeletonAssets::default();
        };
        for_skin.push(SourceSkinAttachment {
            source_node_index: node.index(),
            bone,
            source_mesh_index: node.mesh().map(|mesh| mesh.index()),
        });
    }

    let mut nodes = Vec::with_capacity(doc.nodes().count());
    for node in doc.nodes() {
        let Some(bone) = topo.bone_of_node.get(node.index()).copied().flatten() else {
            return SourceSkeletonAssets::default();
        };
        let local_rest = match node.transform() {
            gltf::scene::Transform::Decomposed {
                translation,
                rotation,
                scale,
            } => SourceNodeLocalRest::Trs {
                translation: Vec3::from_array(translation),
                rotation: Quat::from_array(rotation),
                scale: Vec3::from_array(scale),
            },
            gltf::scene::Transform::Matrix { matrix } => {
                SourceNodeLocalRest::Matrix(Mat4::from_cols_array_2d(&matrix))
            }
        };
        nodes.push(SourceNodeAsset {
            source_node_index: node.index(),
            name: node.name().map(str::to_owned),
            parent_source_node_index: topo.parent[node.index()],
            local_rest,
            bone,
        });
    }

    let mut skins = Vec::with_capacity(doc.skins().count());
    for skin in doc.skins() {
        let mut joints = Vec::new();
        for joint in skin.joints() {
            let Some(bone) = topo.bone_of_node.get(joint.index()).copied().flatten() else {
                return SourceSkeletonAssets::default();
            };
            joints.push(bone);
        }
        let skeleton_root = match skin.skeleton() {
            Some(node) => match topo.bone_of_node.get(node.index()).copied().flatten() {
                Some(bone) => Some(bone),
                None => return SourceSkeletonAssets::default(),
            },
            None => None,
        };
        let inverse_bind_accessor = match skin.inverse_bind_matrices() {
            None => SourceInverseBindAccessor::default(),
            Some(accessor) if accessor.count() == 0 => SourceInverseBindAccessor {
                status: SourceInverseBindAccessorStatus::EmptyAccessor,
                declared_count: Some(0),
                matrices: Vec::new(),
            },
            Some(accessor) => {
                let declared_count = accessor.count();
                let reader = skin.reader(|buffer| buffers.get(buffer.index()).map(Vec::as_slice));
                match reader.read_inverse_bind_matrices() {
                    Some(matrices) => {
                        let matrices = matrices
                            .map(|matrix| Mat4::from_cols_array_2d(&matrix))
                            .collect::<Vec<_>>();
                        SourceInverseBindAccessor {
                            status: if matrices.len() >= joints.len() {
                                SourceInverseBindAccessorStatus::Available
                            } else {
                                SourceInverseBindAccessorStatus::CountMismatch
                            },
                            declared_count: Some(declared_count),
                            matrices,
                        }
                    }
                    None => SourceInverseBindAccessor {
                        status: SourceInverseBindAccessorStatus::Unreadable,
                        declared_count: Some(declared_count),
                        matrices: Vec::new(),
                    },
                }
            }
        };
        skins.push(SourceSkinAsset {
            source_skin_index: skin.index(),
            name: skin.name().map(str::to_owned),
            skeleton_root,
            joints,
            inverse_bind_accessor,
            attachments: std::mem::take(&mut attachments[skin.index()]),
        });
    }

    SourceSkeletonAssets {
        coverage: SourceSkeletonCoverage::Complete,
        nodes,
        skins,
    }
}

/// Parse meshes (indexed or unindexed), skins (joints + inverse bind
/// matrices), and materials (PBR factors + embedded base-color and normal textures)
/// into the core [`SceneAssets`] model — the symmetric read side of
/// [`write::write`], mirroring `animsmith-fbx`'s `extract_assets`.
///
/// Triangle-list vertex data is kept in glTF coordinates without unit
/// conversion or UV flipping; other primitive modes are skipped. Materials
/// keep their glTF array index so a primitive's `material()` index maps
/// straight into `assets.materials`.
fn extract_assets(
    doc: &gltf::Document,
    buffers: &[Vec<u8>],
    base: Option<&Path>,
    bone_of_node: &[Option<usize>],
) -> SceneAssets {
    let mut assets = SceneAssets::default();

    let source_images = extract_source_images(doc, buffers, base);
    let (raw_images, source_image_records): (Vec<_>, Vec<_>) = source_images
        .into_iter()
        .map(|image| (image.texture, image.record))
        .unzip();
    assets.material_resources = MaterialResourceAssets {
        coverage: MaterialResourceCoverage::Complete,
        materials: Vec::new(),
        textures: extract_source_textures(doc),
        images: source_image_records,
    };

    // `doc.materials()` yields defined materials in index order (the
    // synthesized default material has no index and is skipped), so
    // pushing in iteration order keeps `assets.materials[i]` aligned
    // with glTF material index `i`.
    for material in doc.materials() {
        let Some(material_index) = material.index() else {
            continue;
        };
        let pbr = material.pbr_metallic_roughness();
        assets
            .material_resources
            .materials
            .push(SourceMaterialAsset {
                material_index,
                name: material.name().map(str::to_owned),
                texture_bindings: source_material_texture_bindings(&material),
            });
        let base_color_texture = pbr.base_color_texture().and_then(|info| {
            raw_images
                .get(info.texture().source().index())
                .and_then(|image| image.clone())
        });
        let normal_texture = material.normal_texture().and_then(|info| {
            raw_images
                .get(info.texture().source().index())
                .and_then(|image| image.clone())
                .map(|texture| NormalTextureAsset {
                    texture,
                    scale: info.scale(),
                })
        });
        let metallic_roughness_texture = pbr.metallic_roughness_texture().and_then(|info| {
            raw_images
                .get(info.texture().source().index())
                .and_then(|image| image.clone())
        });
        let occlusion_texture = material.occlusion_texture().and_then(|info| {
            raw_images
                .get(info.texture().source().index())
                .and_then(|image| image.clone())
                .map(|texture| OcclusionTextureAsset {
                    texture,
                    strength: info.strength(),
                })
        });
        assets.materials.push(MaterialAsset {
            name: material.name().unwrap_or("material").to_string(),
            base_color: pbr.base_color_factor(),
            metallic: pbr.metallic_factor(),
            roughness: pbr.roughness_factor(),
            base_color_texture,
            normal_texture,
            metallic_roughness_texture,
            occlusion_texture,
        });
    }

    // Keep definitions apart from their node instances. In particular, a
    // valid definition that no node references is still observable to later
    // definition-domain measurement work.
    let mut core_mesh_of_source = vec![None; doc.meshes().count()];
    for mesh in doc.meshes() {
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
            // Secondary influence attributes do not change the core's
            // primary-set semantics, but their independent presence matters
            // to consumers that must reject or report unsupported influence
            // sets. Only retain nonzero accessors, matching the loader's
            // count-zero-as-absent hardening policy above.
            let mut additional_influence_sets: BTreeMap<u32, AdditionalInfluenceSet> =
                BTreeMap::new();
            for (semantic, accessor) in prim.attributes() {
                if accessor.count() == 0 {
                    continue;
                }
                match semantic {
                    gltf::Semantic::Joints(set) if set >= 1 => {
                        additional_influence_sets
                            .entry(set)
                            .and_modify(|entry| entry.joints_present = true)
                            .or_insert(AdditionalInfluenceSet {
                                set_index: set,
                                joints_present: true,
                                weights_present: false,
                            });
                    }
                    gltf::Semantic::Weights(set) if set >= 1 => {
                        additional_influence_sets
                            .entry(set)
                            .and_modify(|entry| entry.weights_present = true)
                            .or_insert(AdditionalInfluenceSet {
                                set_index: set,
                                joints_present: false,
                                weights_present: true,
                            });
                    }
                    _ => {}
                }
            }
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
                additional_influence_sets: additional_influence_sets.into_values().collect(),
            });
        }
        if primitives.is_empty() {
            continue;
        }
        let core_mesh = assets.meshes.len();
        core_mesh_of_source[mesh.index()] = Some(core_mesh);
        assets.meshes.push(MeshAsset {
            name: mesh.name().unwrap_or("mesh").to_string(),
            source_mesh_index: mesh.index(),
            primitives,
        });
    }

    for node in doc.nodes() {
        let Some(source_mesh) = node.mesh() else {
            continue;
        };
        let Some(mesh) = core_mesh_of_source[source_mesh.index()] else {
            continue;
        };
        let skin = node.skin();
        let skin_joints = skin
            .as_ref()
            .map(|skin| {
                skin.joints()
                    .map(|joint| bone_of_node[joint.index()].unwrap_or(0))
                    .collect()
            })
            .unwrap_or_default();
        let skin_ibms = skin
            .as_ref()
            .filter(|skin| {
                skin.inverse_bind_matrices()
                    .is_some_and(|accessor| accessor.count() > 0)
            })
            .map(|skin| {
                let reader = skin.reader(|buffer| buffers.get(buffer.index()).map(Vec::as_slice));
                reader
                    .read_inverse_bind_matrices()
                    .map(|matrices| {
                        matrices
                            .map(|matrix| Mat4::from_cols_array_2d(&matrix))
                            .collect()
                    })
                    .unwrap_or_default()
            })
            .unwrap_or_default();
        assets.instances.push(MeshInstance {
            source_node_index: node.index(),
            node: bone_of_node[node.index()].unwrap_or(0),
            mesh,
            skin_joints,
            skin_ibms,
        });
    }

    assets
}

/// Preserve declared glTF scene membership separately from the all-node
/// skeleton topology. A node can be reachable from the forest yet absent from
/// any declared scene, so membership must not be inferred from `Bone::parent`.
fn extract_scenes(doc: &gltf::Document, bone_of_node: &[Option<usize>]) -> Vec<SceneAsset> {
    doc.scenes()
        .map(|scene| SceneAsset {
            source_scene_index: scene.index(),
            name: scene.name().map(str::to_owned),
            roots: scene
                .nodes()
                .filter_map(|node| bone_of_node[node.index()])
                .collect(),
        })
        .collect()
}

/// Maximum encoded source image size considered for metadata inspection.
const MAX_IMAGE_ENCODED_BYTES: usize = 64 * 1024 * 1024;
/// Maximum allocation the image decoder may request during inspection.
const MAX_IMAGE_DECODE_ALLOC_BYTES: u64 = 192 * 1024 * 1024;

/// One image record plus raw bytes for the legacy writer-facing material
/// slots. The sidecar reports bounded inspection facts; it never changes
/// whether a resolvable source image remains writable.
struct LoadedSourceImage {
    record: SourceImageAsset,
    texture: Option<TextureAsset>,
}

/// Read source-image definitions once, in glTF source order. This retains
/// independent image rows (including unreferenced images), while later
/// texture and material records refer to them by their glTF array indices.
fn extract_source_images(
    doc: &gltf::Document,
    buffers: &[Vec<u8>],
    base: Option<&Path>,
) -> Vec<LoadedSourceImage> {
    let writer_images = writer_image_indices(doc);
    doc.images()
        .map(|image| {
            let image_index = image.index();
            let retain_raw = writer_images.contains(&image_index);
            let name = image.name().map(str::to_owned);
            let (source_kind, declared_mime_type, raw, unavailable_reason) = match image.source() {
                gltf::image::Source::View { view, mime_type } => {
                    let bytes = buffers.get(view.buffer().index()).and_then(|buffer| {
                        // `offset`/`length` are file-derived. A checked add
                        // keeps malformed views from panicking in debug
                        // builds; failed ranges are explicit source gaps.
                        view.offset()
                            .checked_add(view.length())
                            .and_then(|end| buffer.get(view.offset()..end))
                    });
                    let (raw, reason) = match bytes {
                        Some(bytes) if !retain_raw && bytes.len() > MAX_IMAGE_ENCODED_BYTES => {
                            (None, ImageUnavailableReason::ResourceLimit)
                        }
                        Some(bytes) => (
                            Some(TextureAsset {
                                bytes: bytes.to_vec(),
                                mime: mime_type.to_string(),
                            }),
                            ImageUnavailableReason::SourceUnavailable,
                        ),
                        None => (None, ImageUnavailableReason::SourceUnavailable),
                    };
                    (
                        ImageSourceKind::Embedded,
                        Some(mime_type.to_string()),
                        raw,
                        reason,
                    )
                }
                gltf::image::Source::Uri { uri, mime_type } => {
                    if let Some(encoded) = uri.strip_prefix("data:") {
                        let (mime_from_uri, raw, reason) =
                            read_data_uri_image(encoded, mime_type, retain_raw);
                        (
                            ImageSourceKind::DataUri,
                            mime_type.map(str::to_owned).or(mime_from_uri),
                            raw,
                            reason,
                        )
                    } else {
                        let path = base.and_then(|base| {
                            safe_external_buffer_path(uri)
                                .ok()
                                .map(|path| base.join(path))
                        });
                        let (raw, reason) = path
                            .map_or((None, ImageUnavailableReason::SourceUnavailable), |path| {
                                read_external_image(&path, mime_type, retain_raw)
                            });
                        (
                            ImageSourceKind::External,
                            mime_type.map(str::to_owned),
                            raw,
                            reason,
                        )
                    }
                }
            };
            let (detected_container, inspection) =
                inspect_source_image(raw.as_ref(), unavailable_reason);
            LoadedSourceImage {
                record: SourceImageAsset {
                    image_index,
                    name,
                    source_kind,
                    declared_mime_type,
                    detected_container,
                    inspection,
                },
                texture: if retain_raw { raw } else { None },
            }
        })
        .collect()
}

/// Images used by writer-facing material slots retain their full encoded
/// payload, preserving the loader's established round-trip behavior. Other
/// source rows need only a bounded payload long enough for inspection.
fn writer_image_indices(doc: &gltf::Document) -> BTreeSet<usize> {
    let mut images = BTreeSet::new();
    for material in doc.materials() {
        let pbr = material.pbr_metallic_roughness();
        for texture in [
            pbr.base_color_texture().map(|info| info.texture()),
            material.normal_texture().map(|info| info.texture()),
            pbr.metallic_roughness_texture().map(|info| info.texture()),
            material.occlusion_texture().map(|info| info.texture()),
        ]
        .into_iter()
        .flatten()
        {
            images.insert(texture.source().index());
        }
    }
    images
}

/// Decode a glTF `data:` image URI without treating malformed URI input as a
/// load error. The sidecar preserves the stable reason while the legacy
/// writer slot remains absent, as it was before material-resource evidence.
fn read_data_uri_image(
    encoded: &str,
    mime_type: Option<&str>,
    retain_raw: bool,
) -> (Option<String>, Option<TextureAsset>, ImageUnavailableReason) {
    let Some((metadata, payload)) = encoded.split_once(',') else {
        return (None, None, ImageUnavailableReason::InvalidDataUri);
    };
    if !metadata.ends_with(";base64") {
        return (None, None, ImageUnavailableReason::InvalidDataUri);
    }
    let mime_from_uri = metadata
        .strip_suffix(";base64")
        .filter(|mime| !mime.is_empty())
        .map(str::to_owned);
    if !retain_raw && estimated_base64_decoded_len(payload.len()) > MAX_IMAGE_ENCODED_BYTES {
        return (mime_from_uri, None, ImageUnavailableReason::ResourceLimit);
    }
    let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(payload) else {
        return (mime_from_uri, None, ImageUnavailableReason::InvalidDataUri);
    };
    if !retain_raw && bytes.len() > MAX_IMAGE_ENCODED_BYTES {
        return (mime_from_uri, None, ImageUnavailableReason::ResourceLimit);
    }
    let mime = mime_type
        .map(str::to_owned)
        .or_else(|| mime_from_uri.clone())
        .unwrap_or_default();
    (
        mime_from_uri,
        Some(TextureAsset { bytes, mime }),
        ImageUnavailableReason::InvalidDataUri,
    )
}

fn estimated_base64_decoded_len(encoded_len: usize) -> usize {
    encoded_len.saturating_add(3) / 4 * 3
}

fn read_external_image(
    path: &Path,
    mime_type: Option<&str>,
    retain_raw: bool,
) -> (Option<TextureAsset>, ImageUnavailableReason) {
    let bytes = if retain_raw {
        std::fs::read(path).ok()
    } else {
        let file = std::fs::File::open(path).ok();
        file.and_then(|file| {
            let mut bytes = Vec::new();
            file.take((MAX_IMAGE_ENCODED_BYTES + 1) as u64)
                .read_to_end(&mut bytes)
                .ok()
                .map(|_| bytes)
        })
    };
    let Some(bytes) = bytes else {
        return (None, ImageUnavailableReason::SourceUnavailable);
    };
    if !retain_raw && bytes.len() > MAX_IMAGE_ENCODED_BYTES {
        return (None, ImageUnavailableReason::ResourceLimit);
    }
    (
        Some(TextureAsset {
            bytes,
            mime: mime_type.unwrap_or_default().to_owned(),
        }),
        ImageUnavailableReason::SourceUnavailable,
    )
}

/// Extract one material's bindings in fixed semantic order. This is called
/// from the single source-material walk that also keeps writer-facing slots.
fn source_material_texture_bindings(
    material: &gltf::Material<'_>,
) -> Vec<SourceMaterialTextureBinding> {
    let pbr = material.pbr_metallic_roughness();
    let mut texture_bindings = Vec::with_capacity(4);
    let mut push = |slot, texture: Option<gltf::Texture>| {
        if let Some(texture) = texture {
            texture_bindings.push(SourceMaterialTextureBinding {
                slot,
                texture_index: texture.index(),
            });
        }
    };
    push(
        MaterialTextureSlot::BaseColor,
        pbr.base_color_texture().map(|info| info.texture()),
    );
    push(
        MaterialTextureSlot::Normal,
        material.normal_texture().map(|info| info.texture()),
    );
    push(
        MaterialTextureSlot::MetallicRoughness,
        pbr.metallic_roughness_texture().map(|info| info.texture()),
    );
    push(
        MaterialTextureSlot::Occlusion,
        material.occlusion_texture().map(|info| info.texture()),
    );
    texture_bindings
}

/// Project texture definitions in source order, retaining unreferenced rows
/// and their source image identity.
fn extract_source_textures(doc: &gltf::Document) -> Vec<SourceTextureAsset> {
    doc.textures()
        .map(|texture| SourceTextureAsset {
            texture_index: texture.index(),
            name: texture.name().map(str::to_owned),
            image_index: texture.source().index(),
        })
        .collect()
}

/// Inspect an image payload under strict encoded-size and decoder-allocation
/// bounds. Inspection decodes only long enough to obtain metadata; the decoded
/// image is immediately dropped and never becomes part of the core model.
fn inspect_source_image(
    texture: Option<&TextureAsset>,
    unavailable_reason: ImageUnavailableReason,
) -> (Option<ImageContainerFormat>, SourceImageInspection) {
    let Some(texture) = texture else {
        return (
            None,
            SourceImageInspection::Unavailable {
                reason: unavailable_reason,
            },
        );
    };
    if texture.bytes.len() > MAX_IMAGE_ENCODED_BYTES {
        return (
            detect_container(&texture.bytes),
            SourceImageInspection::Unavailable {
                reason: ImageUnavailableReason::ResourceLimit,
            },
        );
    }
    let Some((format, detected_container)) = image_format(&texture.bytes) else {
        return (
            None,
            SourceImageInspection::Unavailable {
                reason: ImageUnavailableReason::UnsupportedContainer,
            },
        );
    };
    let mut reader = ImageReader::new(Cursor::new(&texture.bytes));
    reader.set_format(format);
    let mut limits = Limits::default();
    limits.max_alloc = Some(MAX_IMAGE_DECODE_ALLOC_BYTES);
    reader.limits(limits);
    match reader.decode() {
        Ok(decoded) => {
            let color_type = match decoded.color() {
                ColorType::L8 => Some(DecodedImageColorType::L8),
                ColorType::La8 => Some(DecodedImageColorType::La8),
                ColorType::Rgb8 => Some(DecodedImageColorType::Rgb8),
                ColorType::Rgba8 => Some(DecodedImageColorType::Rgba8),
                ColorType::L16 => Some(DecodedImageColorType::L16),
                ColorType::La16 => Some(DecodedImageColorType::La16),
                ColorType::Rgb16 => Some(DecodedImageColorType::Rgb16),
                ColorType::Rgba16 => Some(DecodedImageColorType::Rgba16),
                _ => None,
            };
            let (width, height) = (decoded.width(), decoded.height());
            match color_type {
                Some(color_type) => (
                    Some(detected_container),
                    SourceImageInspection::Available {
                        width,
                        height,
                        channel_count: decoded.color().channel_count(),
                        color_type,
                    },
                ),
                None => (
                    Some(detected_container),
                    SourceImageInspection::Unavailable {
                        reason: ImageUnavailableReason::DecodeFailed,
                    },
                ),
            }
        }
        Err(ImageError::Limits(_)) => (
            Some(detected_container),
            SourceImageInspection::Unavailable {
                reason: ImageUnavailableReason::ResourceLimit,
            },
        ),
        Err(_) => (
            Some(detected_container),
            SourceImageInspection::Unavailable {
                reason: ImageUnavailableReason::DecodeFailed,
            },
        ),
    }
}

/// Return the supported container format and its core vocabulary variant.
fn image_format(bytes: &[u8]) -> Option<(ImageFormat, ImageContainerFormat)> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some((ImageFormat::Png, ImageContainerFormat::Png))
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some((ImageFormat::Jpeg, ImageContainerFormat::Jpeg))
    } else {
        None
    }
}

/// Detect only the container formats the bounded inspector supports.
fn detect_container(bytes: &[u8]) -> Option<ImageContainerFormat> {
    image_format(bytes).map(|(_, container)| container)
}
