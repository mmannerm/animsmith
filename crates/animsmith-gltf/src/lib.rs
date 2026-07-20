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
//!     Ok(results.into_iter().flat_map(|check| check.findings).collect())
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
    Bone, Clip, Document, Interpolation, MaterialAsset, MeshAsset, Primitive, Property,
    SceneAssets, Skeleton, SourceInfo, TextureAsset, Track, TrackValues, Transform,
};
use base64::Engine as _;
use glam::{Mat4, Quat, Vec3};
use std::collections::BTreeMap;
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
/// scene assets (meshes, skins, materials, and embedded base-color textures)
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
    let mut doc = build_document(&gltf, &buffers, path, &topo)?;
    doc.assets = extract_assets(&gltf.document, &buffers, path.parent(), &topo.bone_of_node);
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

    // Inverse bind matrices from skins (last skin wins on conflict).
    for skin in doc.skins() {
        // Skip a count-0 IBM accessor: gltf 1.4's reader underflows and
        // panics iterating one (the same guard the asset path uses).
        if skin.inverse_bind_matrices().is_none_or(|a| a.count() == 0) {
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

/// Parse meshes (indexed or unindexed), skins (joints + inverse bind
/// matrices), and materials (PBR factors + embedded base-color texture)
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

    assets
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
