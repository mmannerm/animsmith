//! [`load_source`] and [`load_source_bytes`] read `.gltf`/`.glb` input into
//! an immutable normalized-document plus raw-source-facts owner. [`load`] and
//! [`load_bytes`] retain the legacy document-only surface, and [`write::write`]
//! emits a document as
//! glTF/GLB, and the [`fix`] module provides byte-surgical quaternion
//! repairs. [`preflight_scale_source`] inventories the original raw source and
//! fails closed on domains that current scale producers cannot preserve, while
//! [`preflight_clip_track_source`] captures the narrower role-specific
//! animation projection used by clip-track consumers.
//! Malformed inputs report [`LoadError`]; output failures
//! report [`WriteError`].
//! [`write::preflight_glb_bytes`] plus [`write::write_glb_bytes`] are the
//! explicit count-then-retain boundary for a bounded in-memory GLB candidate;
//! callers select a projection policy and byte limits before any output bytes
//! are retained.
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

mod capability;
pub mod fix;
mod scale;
pub mod write;

pub use capability::{
    GltfAccessorCapability, GltfAnimationChannelCapability, GltfAttributeCapability,
    GltfBufferCapability, GltfBufferSourceKind, GltfBufferViewCapability, GltfCapabilityManifest,
    GltfCapabilityViolation, GltfCapabilityViolationKind, GltfContainerKind,
    GltfInstancingCapability, GltfNodeCapability, GltfNodeRestKind, GltfPrimitiveCapability,
    GltfScalePreflightError, GltfScaleSource, GltfSkinCapability, preflight_clip_track_source,
    preflight_clip_track_source_bytes, preflight_scale_source, preflight_scale_source_bytes,
};
pub use scale::{
    GltfRawJsonDifference, GltfRawJsonDifferenceKind, GltfRawJsonDifferenceSummary,
    GltfScaleArtifact, GltfScaleArtifactProof, GltfScaleRewriteError, capability_facts,
    capability_facts_for_source, operation_capability_facts, operation_capability_facts_for_source,
    prove_rewritten_artifact, prove_rewritten_rest_bind, rewrite_linear_units, rewrite_rest_bind,
    rewrite_scale_plan,
};

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
use animsmith_core::{
    DependencyClosureBuilderV1, DependencyClosureError, DependencyClosureV1,
    DependencyResourceKeyV1, DependencyResourceRefusalReasonV1,
    DependencyResourceUnavailableReasonV1, InputIdentity, LoadedSource,
    RAW_SOURCE_V1_MAX_TEXT_BYTES, RawSourceFactsBuilderV1, ResourceKeySyntaxV1, SourceAxisV1,
    SourceChannelFactV1, SourceChannelPropertyV1, SourceClipFactV1, SourceComponentMaskV1,
    SourceConstructFactV1, SourceConstructKindV1, SourceCoordinateBasisV1, SourceFactDomainV1,
    SourceFactSetV1, SourceFactsError, SourceFormatV1, SourceInterpolationV1, SourceLinearUnitV1,
    SourceLoaderDispositionV1, SourceLogicalLocatorV1, SourceObservationV1, SourceProvenanceKindV1,
    SourceProvenanceV1, SourceResourceKindV1, SourceResourceLocatorV1, SourceResourceReferenceV1,
    SourceTargetKindV1, SourceTargetV1, SourceTextV1, SourceTimeRangeV1, SourceUnavailableReasonV1,
};
use base64::Engine as _;
use glam::{Mat4, Quat, Vec3};
use gltf::accessor::{DataType as ComponentType, Dimensions as AccessorType};
use image::{ColorType, ImageError, ImageFormat, ImageReader, Limits};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

mod addressability;

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
    /// A rooted external dependency required to construct the document failed.
    #[error("external resource load failed: {0}")]
    ExternalResource(ExternalResourceFailure),
    /// The `gltf` parser rejected the container.
    #[error("glTF parse error: {0}")]
    Gltf(#[from] gltf::Error),
    /// The format-neutral source-facts projection contradicted a core invariant.
    #[error("invalid raw-source facts: {0}")]
    SourceFacts(#[from] SourceFactsError),
    /// Buffer resolution or GLB framing failed.
    #[error("buffer resolution failed: {0}")]
    Buffer(String),
    /// Animation data is structurally malformed.
    #[error("malformed animation data: {0}")]
    Malformed(String),
    /// The node graph is not a forest that can become a skeleton.
    #[error("malformed node graph: {0}")]
    Topology(String),
    /// A primitive's vertex-attribute or index accessor declares an element
    /// encoding the loader cannot read as authored. Either the file
    /// contradicts the glTF vertex-attribute rules (a `VEC3` `TEXCOORD_0`, or
    /// an integer `TEXCOORD_0`/`WEIGHTS_0` without `normalized: true`), or it
    /// uses a spec-permitted encoding the `gltf` crate's reader has no decoder
    /// for (a `KHR_mesh_quantization` `POSITION`). The message reports the
    /// encodings that slot accepts.
    #[error(
        "mesh {mesh} primitive {primitive} {attribute}: accessor {accessor} is {found}, but the loader reads {expected}"
    )]
    PrimitiveEncoding {
        /// glTF index of the mesh holding the primitive.
        mesh: usize,
        /// Index of the primitive within that mesh.
        primitive: usize,
        /// Attribute semantic or slot name, such as `TEXCOORD_0`.
        attribute: String,
        /// Index of the offending accessor.
        accessor: usize,
        /// Declared encoding, such as `VEC3 of FLOAT`.
        found: String,
        /// Encodings the reader accepts, such as
        /// `VEC2 of normalized UNSIGNED_BYTE, normalized UNSIGNED_SHORT, or
        /// FLOAT`.
        expected: String,
    },
    /// A primitive's vertex-attribute or index accessor declares an element
    /// the loader does read, but addresses its bytes in a way the reader
    /// cannot walk: a buffer view whose own extent overflows, exceeds its
    /// buffer declaration, or exceeds the bytes that actually resolved; a
    /// `byteStride` shorter than the element it strides over; a `sparse`
    /// block of count 0; or a `count`/`byteOffset` whose required extent
    /// overflows or exceeds its buffer view. `gltf`'s `Validate`
    /// relates an accessor to none of these. The first shapes can panic in
    /// the reader; a merely short extent instead makes the reader return
    /// `None`, which is equally unsafe to substitute with empty geometry.
    #[error("mesh {mesh} primitive {primitive} {attribute}: accessor {accessor} {problem}")]
    PrimitiveAccessorLayout {
        /// glTF index of the mesh holding the primitive.
        mesh: usize,
        /// Index of the primitive within that mesh.
        primitive: usize,
        /// Attribute semantic or slot name, such as `TEXCOORD_0`.
        attribute: String,
        /// Index of the offending accessor.
        accessor: usize,
        /// What the reader could not walk, such as `reads its elements from
        /// buffer view 0 at byteStride 4, shorter than the 12-byte element
        /// it strides over`.
        problem: String,
    },
    /// An animation sampler's `input` or `output` accessor addresses its
    /// bytes in a way the reader cannot walk — the same layout shapes
    /// [`Self::PrimitiveAccessorLayout`] names, reached through
    /// `read_inputs`/`read_outputs` instead of through a primitive.
    ///
    /// This is the sampler's *layout*, not its *encoding*: an accessor
    /// typed for a different element than the sampler's reader decodes is a
    /// separate class and is not judged here.
    #[error("clip '{clip}' node {node} sampler {slot}: accessor {accessor} {problem}")]
    AnimationAccessorLayout {
        /// Name the clip loads under, as reported by every other animation
        /// diagnostic.
        clip: String,
        /// glTF index of the node the offending channel targets.
        node: usize,
        /// Sampler slot the accessor fills: `input` or `output`.
        slot: &'static str,
        /// Index of the offending accessor.
        accessor: usize,
        /// What the reader could not walk, such as `reads its sparse indices
        /// from buffer view 2, whose byteOffset 18446744073709551615 plus
        /// byteLength 12 is a byte extent that overflows`.
        problem: String,
    },
    /// An animation sampler's `input` or `output` accessor declares an
    /// element encoding the property-specific reader cannot decode. The
    /// reader always expects scalar `FLOAT` key times; outputs are `VEC3` of
    /// `FLOAT` for translation/scale, or one of glTF's five component
    /// encodings as `VEC4` rotation and scalar morph weights.
    #[error(
        "animation {animation} sampler {sampler} {slot} for node {node} {property}: accessor {accessor} is {found}, but the loader reads {expected}"
    )]
    AnimationEncoding {
        /// glTF index of the animation holding the sampler.
        animation: usize,
        /// Index of the sampler within that animation.
        sampler: usize,
        /// Sampler slot the accessor fills: `input` or `output`.
        slot: &'static str,
        /// glTF index of the node the channel targets.
        node: usize,
        /// Target property selecting the output reader.
        property: &'static str,
        /// Index of the offending accessor.
        accessor: usize,
        /// Declared encoding, such as `VEC3 of FLOAT`.
        found: String,
        /// Encodings the selected reader accepts.
        expected: String,
    },
}

/// Sanitized failure classes for external resources required by the loader.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ExternalResourceFailure {
    /// Captured bytes did not authorize a filesystem root.
    #[error("external resource requires an explicit trusted root")]
    ResourceRootRequired,
    /// The source-controlled locator or resolved path crossed the refusal boundary.
    #[error("unsafe external buffer resource")]
    Refused,
    /// The resource exceeded the bounded closure-capture limits.
    #[error("external buffer resource exceeds capture limits")]
    CaptureLimitExceeded,
    /// The accepted resource was missing, unreadable, or changed before capture.
    #[error("external buffer resource is unavailable")]
    Unavailable,
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
    /// The bounded GLB projection cannot represent this document without
    /// silently omitting or changing authored model data.
    #[error("GLB projection refused: {0}")]
    Refused(String),
    /// A checked byte calculation or allocation reservation failed before
    /// output bytes were constructed.
    #[error("GLB output cannot reserve {bytes} bytes for {field}")]
    Allocation {
        /// The output component whose allocation could not be reserved.
        field: &'static str,
        /// Requested byte count.
        bytes: usize,
    },
    /// The caller supplied a preflight receipt for a different document or
    /// limit set.
    #[error("GLB preflight receipt no longer matches the document")]
    ReceiptMismatch,
}

/// Convert an external-resource URI to the shared safe relative key.
///
/// This legacy helper is retained for writer and repair paths. Loader-side
/// resource capture additionally validates a trusted root and rejects
/// symlinks before opening the key.
pub(crate) fn safe_external_buffer_path(uri: &str) -> Result<PathBuf, LoadError> {
    DependencyResourceKeyV1::from_source_str(uri, ResourceKeySyntaxV1::GltfUri)
        .map(|key| PathBuf::from(key.as_str()))
        .map_err(|_| unsafe_external_uri())
}

fn unsafe_external_uri() -> LoadError {
    LoadError::Buffer("unsafe external buffer URI: expected a relative child path".to_owned())
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

/// Detect an `extensions` object key anywhere in the exact JSON payload.
///
/// `gltf-json` discards unknown extension payloads when its optional
/// `extensions` feature is disabled, and it does not require payload names to
/// appear in `extensionsUsed`. This allocation-free, nonrecursive scan keeps
/// dependency closure coverage conservative even for that undeclared shape.
/// It runs only after the ordinary glTF parser has accepted the JSON.
fn has_extension_object(primary_bytes: &[u8]) -> bool {
    let Some(json) = source_json_payload(primary_bytes) else {
        return true;
    };
    json_has_object_key(json, b"extensions")
}

fn source_json_payload(primary_bytes: &[u8]) -> Option<&[u8]> {
    if !primary_bytes.starts_with(b"glTF") {
        return Some(primary_bytes);
    }
    const GLB_JSON_OFFSET: usize = 20;
    let length = u32::from_le_bytes(primary_bytes.get(12..16)?.try_into().ok()?) as usize;
    primary_bytes.get(GLB_JSON_OFFSET..GLB_JSON_OFFSET.checked_add(length)?)
}

fn json_has_object_key(json: &[u8], target: &[u8]) -> bool {
    let mut cursor = 0;
    while cursor < json.len() {
        if json[cursor] != b'"' {
            cursor += 1;
            continue;
        }
        cursor += 1;
        let mut target_index = 0;
        let mut candidate = true;
        loop {
            let Some(&byte) = json.get(cursor) else {
                return true;
            };
            if byte == b'"' {
                cursor += 1;
                break;
            }
            let decoded = if byte == b'\\' {
                cursor += 1;
                let Some(&escape) = json.get(cursor) else {
                    return true;
                };
                match escape {
                    b'u' => {
                        let Some(hex) = json.get(cursor + 1..cursor + 5) else {
                            return true;
                        };
                        cursor += 5;
                        decode_json_hex_quad(hex).and_then(|value| u8::try_from(value).ok())
                    }
                    b'"' | b'\\' | b'/' => {
                        cursor += 1;
                        Some(escape)
                    }
                    b'b' | b'f' | b'n' | b'r' | b't' => {
                        cursor += 1;
                        None
                    }
                    _ => return true,
                }
            } else {
                cursor += 1;
                Some(byte)
            };
            if candidate {
                match decoded {
                    Some(decoded) if target.get(target_index) == Some(&decoded) => {
                        target_index += 1;
                    }
                    _ => candidate = false,
                }
            }
        }
        let mut delimiter = cursor;
        while matches!(json.get(delimiter), Some(b' ' | b'\t' | b'\r' | b'\n')) {
            delimiter += 1;
        }
        if candidate && target_index == target.len() && json.get(delimiter).copied() == Some(b':') {
            return true;
        }
    }
    false
}

fn decode_json_hex_quad(hex: &[u8]) -> Option<u16> {
    hex.iter().try_fold(0_u16, |value, byte| {
        let digit = match byte {
            b'0'..=b'9' => u16::from(*byte - b'0'),
            b'a'..=b'f' => u16::from(*byte - b'a' + 10),
            b'A'..=b'F' => u16::from(*byte - b'A' + 10),
            _ => return None,
        };
        Some((value << 4) | digit)
    })
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
///
/// Element encodings are judged separately after this raw channel validation
/// makes the high-level target accessors safe to call; see
/// [`validate_animation_accessor_encodings`].
pub(crate) fn validate_animation_channels(root: &gltf::json::Root) -> Result<(), LoadError> {
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
    }
    Ok(())
}

/// Validate every animation reader boundary after raw channel indices and
/// target paths are known to be safe to project through `gltf`'s high-level
/// API.
pub(crate) fn validate_animations(doc: &gltf::Document) -> Result<(), LoadError> {
    validate_animation_channels(doc.as_json())?;
    validate_animation_accessor_encodings(doc)
}

/// Apply the typed glTF validation boundary while admitting declarations for
/// extensions this loader inventories but does not implement. The `gltf`
/// crate reports those required-extension declarations as `Unsupported`; all
/// structural validation failures remain load errors.
pub(crate) fn validate_document(document: &gltf::Document) -> Result<(), gltf::Error> {
    use gltf::json::validation::{Error, Validate};

    let root = document.as_json();
    let mut errors = Vec::new();
    root.validate(root, gltf::json::Path::new, &mut |path, error| {
        errors.push((path(), error));
    });
    if errors.iter().all(|(_, error)| *error == Error::Unsupported) {
        Ok(())
    } else {
        Err(gltf::Error::Validation(errors))
    }
}

/// The element encoding one `gltf` reader can decode: the single accessor
/// `type` its element matches, and the `componentType`s it dispatches on.
struct ReaderEncoding {
    /// Accessor `type` whose element size the reader's element type equals.
    accessor_type: AccessorType,
    /// Accepted `componentType`s, in glTF enum order.
    component_types: &'static [ComponentType],
    /// Whether accepted integer components must declare `normalized: true`.
    ///
    /// `gltf`'s float conversion rescales integer texture coordinates and
    /// weights even when this flag is absent, so the loader must enforce the
    /// declaration before it lets those readers reinterpret the values.
    normalized_integers: bool,
}

/// `read_positions` is `Iter<[f32; 3]>` with no dispatch on the accessor.
/// `KHR_mesh_quantization` also permits normalized `BYTE`/`SHORT` here; the
/// `gltf` reader has no decoder for those, so they are refused rather than
/// misread.
const POSITION_ENCODING: ReaderEncoding = ReaderEncoding {
    accessor_type: AccessorType::Vec3,
    component_types: &[ComponentType::F32],
    normalized_integers: false,
};
/// `read_normals` is `Iter<[f32; 3]>`, with the same quantization caveat as
/// [`POSITION_ENCODING`].
const NORMAL_ENCODING: ReaderEncoding = ReaderEncoding {
    accessor_type: AccessorType::Vec3,
    component_types: &[ComponentType::F32],
    normalized_integers: false,
};
/// `read_tex_coords` dispatches over the three encodings glTF permits for
/// `TEXCOORD_n`: `FLOAT`, or normalized `UNSIGNED_BYTE`/`UNSIGNED_SHORT`.
/// `gltf`'s `into_f32()` rescales either integer width even when the accessor
/// omits `normalized: true`, so admission must consult the flag before that
/// reader is built. Otherwise measurements would report values the document
/// never authorized the loader to derive.
const TEX_COORD_ENCODING: ReaderEncoding = ReaderEncoding {
    accessor_type: AccessorType::Vec2,
    component_types: &[ComponentType::U8, ComponentType::U16, ComponentType::F32],
    normalized_integers: true,
};
/// `read_joints` dispatches over both encodings glTF permits for `JOINTS_n`.
/// Joint indices are never `FLOAT` and never normalized.
const JOINTS_ENCODING: ReaderEncoding = ReaderEncoding {
    accessor_type: AccessorType::Vec4,
    component_types: &[ComponentType::U8, ComponentType::U16],
    normalized_integers: false,
};
/// `read_weights` dispatches over the three encodings glTF permits for
/// `WEIGHTS_n`: `FLOAT`, or normalized `UNSIGNED_BYTE`/`UNSIGNED_SHORT`,
/// with the same admission requirement as [`TEX_COORD_ENCODING`].
const WEIGHTS_ENCODING: ReaderEncoding = ReaderEncoding {
    accessor_type: AccessorType::Vec4,
    component_types: &[ComponentType::U8, ComponentType::U16, ComponentType::F32],
    normalized_integers: true,
};
/// `read_indices` dispatches over the three index encodings glTF permits.
const INDEX_ENCODING: ReaderEncoding = ReaderEncoding {
    accessor_type: AccessorType::Scalar,
    component_types: &[ComponentType::U8, ComponentType::U16, ComponentType::U32],
    normalized_integers: false,
};
/// `read_inverse_bind_matrices` is `Iter<[[f32; 4]; 4]>`; glTF permits only
/// `MAT4` of `FLOAT` there.
const INVERSE_BIND_ENCODING: ReaderEncoding = ReaderEncoding {
    accessor_type: AccessorType::Mat4,
    component_types: &[ComponentType::F32],
    normalized_integers: false,
};
/// `read_inputs` is an un-dispatched `Iter<f32>`.
const ANIMATION_INPUT_ENCODING: ReaderEncoding = ReaderEncoding {
    accessor_type: AccessorType::Scalar,
    component_types: &[ComponentType::F32],
    normalized_integers: false,
};
/// Translation and scale outputs are un-dispatched `Iter<[f32; 3]>` values.
const ANIMATION_VEC3_OUTPUT_ENCODING: ReaderEncoding = ReaderEncoding {
    accessor_type: AccessorType::Vec3,
    component_types: &[ComponentType::F32],
    normalized_integers: false,
};
/// Rotation outputs dispatch over every quaternion encoding glTF permits and
/// the `gltf` reader decodes.
const ANIMATION_ROTATION_OUTPUT_ENCODING: ReaderEncoding = ReaderEncoding {
    accessor_type: AccessorType::Vec4,
    component_types: &[
        ComponentType::I8,
        ComponentType::U8,
        ComponentType::I16,
        ComponentType::U16,
        ComponentType::F32,
    ],
    normalized_integers: false,
};
/// Morph-weight outputs dispatch over the same five component encodings as
/// rotations, but use scalar elements because one key contains one scalar per
/// morph target. The loader does not retain them, but it still constructs the
/// property-selected reader before skipping them, so their reader boundary
/// must remain both safe and spec-complete.
const ANIMATION_WEIGHT_OUTPUT_ENCODING: ReaderEncoding = ReaderEncoding {
    accessor_type: AccessorType::Scalar,
    component_types: &[
        ComponentType::I8,
        ComponentType::U8,
        ComponentType::I16,
        ComponentType::U16,
        ComponentType::F32,
    ],
    normalized_integers: false,
};

/// Reject sampler accessors whose declared element disagrees with the exact
/// `gltf` reader selected by their slot and target property.
fn validate_animation_accessor_encodings(doc: &gltf::Document) -> Result<(), LoadError> {
    for animation in doc.animations() {
        for channel in animation.channels() {
            let sampler = channel.sampler();
            let target = channel.target();
            let node = target.node().index();
            let property = target.property();
            check_animation_accessor_encoding(
                animation.index(),
                sampler.index(),
                node,
                animation_property_name(property),
                "input",
                &sampler.input(),
                &ANIMATION_INPUT_ENCODING,
            )?;
            let output_encoding = match property {
                gltf::animation::Property::Translation | gltf::animation::Property::Scale => {
                    &ANIMATION_VEC3_OUTPUT_ENCODING
                }
                gltf::animation::Property::Rotation => &ANIMATION_ROTATION_OUTPUT_ENCODING,
                gltf::animation::Property::MorphTargetWeights => &ANIMATION_WEIGHT_OUTPUT_ENCODING,
            };
            check_animation_accessor_encoding(
                animation.index(),
                sampler.index(),
                node,
                animation_property_name(property),
                "output",
                &sampler.output(),
                output_encoding,
            )?;
        }
    }
    Ok(())
}

fn check_animation_accessor_encoding(
    animation: usize,
    sampler: usize,
    node: usize,
    property: &'static str,
    slot: &'static str,
    accessor: &gltf::Accessor<'_>,
    required: &ReaderEncoding,
) -> Result<(), LoadError> {
    if encoding_matches(accessor, required) {
        return Ok(());
    }
    Err(LoadError::AnimationEncoding {
        animation,
        sampler,
        slot,
        node,
        property,
        accessor: accessor.index(),
        found: format!(
            "{} of {}",
            accessor_type_name(accessor.dimensions()),
            component_type_name(accessor.data_type())
        ),
        expected: describe_encoding(required),
    })
}

fn animation_property_name(property: gltf::animation::Property) -> &'static str {
    match property {
        gltf::animation::Property::Translation => "translation",
        gltf::animation::Property::Rotation => "rotation",
        gltf::animation::Property::Scale => "scale",
        gltf::animation::Property::MorphTargetWeights => "weights",
    }
}

/// The encoding the loader requires of one attribute semantic, or `None`
/// when no reader is ever built for it.
///
/// The match is exhaustive over `gltf::Semantic` so that a **new variant in
/// `gltf`** has to be answered here before this crate compiles. That is the
/// whole of what the compiler enforces, and it is worth stating what it
/// does *not*: `gltf::Semantic` is a plain enum, so an exhaustive match
/// cannot notice a new *read site*. Adding `reader.read_tangents()` or
/// `reader.read_tex_coords(1)` to `extract_assets` compiles unchanged
/// against the `None` arms below and reinstates the panic this module
/// exists to prevent — a `TANGENT` declared `VEC3` of `FLOAT` would go
/// straight into `Iter::<[f32; 4]>::new`.
///
/// So each `None` is a claim about `extract_assets` as written — "no reader
/// is ever built for this semantic" — that only this comment and the reason
/// recorded on each arm keep true. Turning one into a read means giving it
/// an encoding here first.
fn required_attribute_encoding(semantic: &gltf::Semantic) -> Option<&'static ReaderEncoding> {
    match semantic {
        gltf::Semantic::Positions => Some(&POSITION_ENCODING),
        gltf::Semantic::Normals => Some(&NORMAL_ENCODING),
        // Only set 0 of each of these reaches a reader; the core model
        // carries one UV channel and one influence set.
        gltf::Semantic::TexCoords(0) => Some(&TEX_COORD_ENCODING),
        gltf::Semantic::Joints(0) => Some(&JOINTS_ENCODING),
        gltf::Semantic::Weights(0) => Some(&WEIGHTS_ENCODING),
        // Not read. `TANGENT` and `COLOR_n` have no core-model slot, and
        // `TEXCOORD_n`/`JOINTS_n`/`WEIGHTS_n` above set 0 are recorded from
        // `Accessor::count()` alone (see `additional_influence_sets`), which
        // never builds a reader and so cannot panic on a mistyped accessor.
        gltf::Semantic::Tangents
        | gltf::Semantic::Colors(_)
        | gltf::Semantic::TexCoords(_)
        | gltf::Semantic::Joints(_)
        | gltf::Semantic::Weights(_) => None,
    }
}

/// Reject primitive accessors the reader that will decode them cannot
/// decode. `gltf`'s `Validate` checks each accessor in isolation, never
/// cross-checks it against the slot that references it, and never relates
/// it to the buffer view it reads — so a `VEC3` `TEXCOORD_0` and a
/// `POSITION` on a stride-4 view both parse cleanly and then trip a
/// `debug_assert`, an `unreachable!()`, or an arithmetic overflow inside
/// the reader: a panic on arbitrary input (invariant-1).
///
/// Two shapes of *element encoding* leak through, both fatal:
///
/// - **Wrong `type`.** `Iter::<T>::new` asserts `size_of::<T>() ==
///   accessor.size()`. A `VEC3` `TEXCOORD_0` is 12 bytes against
///   `[f32; 2]`'s 8, so `read_tex_coords` panics.
/// - **Wrong `componentType`.** The dispatching readers (`read_tex_coords`,
///   `read_joints`, `read_weights`, `read_indices`) have an `unreachable!()`
///   arm for the component types they cannot decode, so a `BYTE`
///   `TEXCOORD_0` panics there instead.
///
/// A third shape is worse than a panic: when the wrong `componentType`
/// happens to preserve the element size and the reader does not dispatch on
/// it, nothing fires and the bytes are silently reinterpreted. A `VEC3` of
/// `UNSIGNED_INT` `NORMAL` is 12 bytes just like `[f32; 3]`, so every normal
/// would load as the float reading of an integer's bits. Refusing that too
/// is what keeps invariant-9 honest for these slots: what the loader reads
/// is decoded as the encoding it declares, or the file is refused — never
/// reinterpreted as a different element.
///
/// [`unreadable_primitive_layout`] covers the rest: an accessor whose element
/// the loader does read, addressed in a way the reader cannot walk against
/// either its declarations or its resolved bytes.
///
/// ## What this does not promise
///
/// "Never reinterpreted" is narrower than "every authored value survives",
/// and deliberately so, in two directions.
///
/// *Values are rescaled where the accessor authorizes scaling.*
/// `into_f32()` rescales a normalized `UNSIGNED_BYTE`/`UNSIGNED_SHORT`
/// `TEXCOORD_0` or `WEIGHTS_0` from full scale (see
/// [`TEX_COORD_ENCODING`]). Checks therefore see those slots as floats, not
/// as the integers on disk. An integer accessor missing `normalized: true`
/// is refused before this reader boundary because `gltf` would otherwise
/// perform the same rescaling without the document declaring it.
///
/// *Unreadable authored values are not absence.*
/// [`unreadable_primitive_layout`] relates every dense and sparse walk to the
/// buffer view and resolved buffer bytes that must satisfy it. A short
/// `POSITION` or index read is refused rather than mapped to an empty vector,
/// so checks can distinguish an authored empty slot from authored values the
/// loader could not read. Inverse binds deliberately use the other established
/// treatment: their shortfall remains explicit source evidence through
/// [`inverse_bind_is_readable`].
///
/// ## Scope
///
/// Only what the loader actually reads is checked. Non-triangle primitives
/// are skipped whole by `extract_assets`, so their accessors are never
/// decoded and are not judged here. Within an ingested primitive the check
/// is deliberately independent of the count-zero and `JOINTS_0`/`WEIGHTS_0`
/// pairing guards that decide whether a particular read happens: those
/// guards are free to move without opening a hole.
///
/// A skin's `inverseBindMatrices` accessor has the same panics, but not the
/// same answer — see [`inverse_bind_is_readable`].
fn validate_primitive_accessors(
    doc: &gltf::Document,
    buffers: &[Vec<u8>],
) -> Result<(), LoadError> {
    for mesh in doc.meshes() {
        for primitive in mesh.primitives() {
            if primitive.mode() != gltf::mesh::Mode::Triangles {
                continue;
            }
            for (semantic, accessor) in primitive.attributes() {
                let Some(required) = required_attribute_encoding(&semantic) else {
                    continue;
                };
                check_primitive_accessor(
                    &mesh,
                    &primitive,
                    &semantic.to_string(),
                    &accessor,
                    required,
                    buffers,
                )?;
            }
            if let Some(accessor) = primitive.indices() {
                check_primitive_accessor(
                    &mesh,
                    &primitive,
                    "indices",
                    &accessor,
                    &INDEX_ENCODING,
                    buffers,
                )?;
            }
        }
    }
    Ok(())
}

/// One slot of one ingested primitive: the element must be one the reader
/// decodes, and its bytes must be laid out so the reader can walk them.
fn check_primitive_accessor(
    mesh: &gltf::Mesh<'_>,
    primitive: &gltf::Primitive<'_>,
    attribute: &str,
    accessor: &gltf::Accessor<'_>,
    required: &ReaderEncoding,
    buffers: &[Vec<u8>],
) -> Result<(), LoadError> {
    if !encoding_matches(accessor, required) {
        return Err(LoadError::PrimitiveEncoding {
            mesh: mesh.index(),
            primitive: primitive.index(),
            attribute: attribute.to_owned(),
            accessor: accessor.index(),
            found: format!(
                "{} of {}",
                accessor_type_name(accessor.dimensions()),
                component_type_name(accessor.data_type())
            ),
            expected: describe_encoding(required),
        });
    }
    if let Some(problem) = unreadable_primitive_layout(accessor, buffers) {
        return Err(LoadError::PrimitiveAccessorLayout {
            mesh: mesh.index(),
            primitive: primitive.index(),
            attribute: attribute.to_owned(),
            accessor: accessor.index(),
            problem,
        });
    }
    Ok(())
}

/// Why a modeled primitive accessor cannot be walked against both the JSON
/// declarations and the bytes that buffer resolution actually returned.
fn unreadable_primitive_layout(
    accessor: &gltf::Accessor<'_>,
    buffers: &[Vec<u8>],
) -> Option<String> {
    unreadable_layout(accessor).or_else(|| {
        if let Some(view) = accessor.view()
            && let Some(problem) = loaded_buffer_shortfall("elements", &view, buffers)
        {
            return Some(problem);
        }
        let sparse = accessor.sparse()?;
        loaded_buffer_shortfall("sparse indices", &sparse.indices().view(), buffers)
            .or_else(|| loaded_buffer_shortfall("sparse values", &sparse.values().view(), buffers))
    })
}

/// A resolved external file, data URI, or GLB BIN chunk can be shorter than
/// the buffer's declared `byteLength`. `gltf` slices the complete view before
/// walking an accessor, so even a smaller accessor extent cannot be decoded
/// when that declared view extends beyond the bytes that actually loaded.
fn loaded_buffer_shortfall(
    subject: &str,
    view: &gltf::buffer::View<'_>,
    buffers: &[Vec<u8>],
) -> Option<String> {
    let view_end = view_end(view)?;
    let buffer_index = view.buffer().index();
    let loaded_length = buffers.get(buffer_index).map_or(0, Vec::len);
    (view_end > loaded_length).then(|| {
        format!(
            "reads its {subject} from buffer view {}, whose byte extent ends at {view_end} \
             beyond loaded buffer {buffer_index}'s {loaded_length} bytes",
            view.index()
        )
    })
}

/// One sampler accessor of one animation channel the loader reads: its
/// bytes must be laid out so the channel's reader can walk them.
///
/// `read_inputs` and `read_outputs` each build their own `Iter` over their
/// own accessor, so an `input` and an `output` are judged separately and
/// both before either reader exists. The layout shapes are exactly the ones
/// [`unreadable_layout`] names for a primitive slot — the panic is in the
/// shared accessor iterator, not in anything primitive-specific.
///
/// **Layout only.** This judges *how* a sampler accessor addresses its
/// bytes: its view's extent, its `byteStride`, its `sparse` count, and the
/// extent its own `count` walks. [`validate_animation_accessor_encodings`]
/// has already judged *what* element the accessor declares against its
/// property-selected reader; keeping the checks separate preserves their
/// distinct public error classifications.
fn check_sampler_accessor(
    clip: &str,
    node: usize,
    slot: &'static str,
    accessor: &gltf::Accessor<'_>,
) -> Result<(), LoadError> {
    match unreadable_layout(accessor) {
        Some(problem) => Err(LoadError::AnimationAccessorLayout {
            clip: clip.to_owned(),
            node,
            slot,
            accessor: accessor.index(),
            problem,
        }),
        None => Ok(()),
    }
}

/// Why `gltf`'s reader cannot walk an accessor's bytes, independent of the
/// element it declares — or `None` when it can.
///
/// `Iter::new` first slices a buffer view as `view.byteOffset +
/// view.byteLength`, then steps the accessor inside it as `byteOffset +
/// byteStride * (count - 1) + size` — over the accessor's own buffer view
/// and, for a sparse accessor, over its index and value views too. Six
/// layout failures are unsafe here, and `gltf-json`'s `Validate` catches
/// none of them:
///
/// - **A buffer view whose own extent overflows.** `USize64`'s validator
///   only rejects a value past `usize`, and `View`'s derived `Validate`
///   relates `byteOffset` neither to `byteLength` nor to the buffer, so
///   `buffer_view_slice` adds the two unchecked before any accessor
///   arithmetic runs (see [`view_end`]).
/// - **A buffer view whose extent exceeds its buffer.** The same missing
///   relationship lets a view point beyond the buffer's declared bytes;
///   the reader then has no slice to walk and answers `None`.
/// - **A `byteStride` shorter than the element.** `Stride`'s validator
///   accepts any `4..=252`, so a `VEC3` of `FLOAT` `POSITION` on a stride-4
///   view validates and then trips `debug_assert!(stride >=
///   size_of::<T>())`. Losing that assertion in a release build does not
///   make the shortfall survivable: the truncated slice reaches
///   `Item::from_slice`, whose `assert!(slice.len() >= N *
///   size_of::<T>())` is a *hard* assert, and it fires on the ordinary
///   dense path just as it does on the sparse one — which compiles no
///   stride assertion at all, in either profile. Deleting this branch and
///   running the suite under `--release` panics a dense `POSITION` at
///   `util.rs:266`. This is the one shape here that still panics with
///   `overflow-checks` off; the arithmetic overflow/underflow shapes go
///   quiet instead, which is worse.
/// - **`sparse.count` of 0.** `sparse_count - 1` underflows, with or
///   without a base `bufferView`. Every count-zero guard in this loader
///   reads `Accessor::count()`; none of them sees this one.
/// - **An extent that overflows `usize`.** Nothing bounds `count` or
///   `sparse.count`, so `stride * (count - 1)` overflows on a large enough
///   declaration.
/// - **An extent that exceeds its buffer view.** A merely large `count` or
///   `byteOffset` does not overflow, but `Iter::new` answers `None` when the
///   declared view cannot satisfy the walk. Treating that as an empty vector
///   would silently replace authored geometry with absence. Sparse index and
///   value views are judged on the same boundary.
///
/// "`gltf-json`'s `Validate` catches none of them" is a claim about this
/// crate's own JSON validation, which is all that stands between
/// `from_slice` and the reader — not about the Khronos glTF-Validator,
/// which does name several of these (`ACCESSOR_SMALL_BYTESTRIDE`, and a
/// `sparse.count` of 0 against the schema's `minimum: 1`). That separate
/// tool is what `docs/cli.md` points authors at; this loader never runs it.
///
/// One near neighbour is deliberately *not* refused. `Accessor::count()` of
/// 0 stays loadable: every read site already treats a count-zero accessor as
/// absent, so no reader is built and the arithmetic is never reached.
fn unreadable_layout(accessor: &gltf::Accessor<'_>) -> Option<String> {
    let size = accessor.size();
    if let Some(view) = accessor.view()
        && let Some(problem) =
            unwalkable("elements", &view, accessor.offset(), accessor.count(), size)
    {
        return Some(problem);
    }
    let sparse = accessor.sparse()?;
    if sparse.count() == 0 {
        return Some("declares a sparse block of count 0, which its reader cannot walk".to_owned());
    }
    let indices = sparse.indices();
    let values = sparse.values();
    // A sparse index is 1, 2, or 4 bytes and `byteStride` is validated into
    // `4..=252`, so an index view can never stride shorter than the element
    // it strides over; only its extent can overflow.
    unwalkable(
        "sparse indices",
        &indices.view(),
        indices.offset(),
        sparse.count(),
        indices.index_type().size(),
    )
    .or_else(|| {
        unwalkable(
            "sparse values",
            &values.view(),
            values.offset(),
            sparse.count(),
            size,
        )
    })
}

/// Where a buffer view's declared extent ends, or `None` when it has no
/// end: `byteOffset` and `byteLength` are both file-derived, and nothing in
/// `gltf-json` relates them to each other or to the buffer, so their sum
/// overflows on arbitrary input — a panic in a debug build, and a wrong
/// extent in a release one.
///
/// No read of a view's declared extent in this loader escapes that answer.
/// The image path slices through this function directly. Every reader-driven
/// read is gated instead: the reader would perform the same unchecked add
/// itself, so [`unwalkable`] asks this before the reader is ever built — for a
/// primitive slot in [`check_primitive_accessor`], for a skin's
/// `inverseBindMatrices` in [`inverse_bind_is_readable`], and for an animation
/// sampler's `input` and `output` in [`check_sampler_accessor`]. Primitive
/// slots additionally compare the result with the resolved buffer length in
/// [`loaded_buffer_shortfall`].
fn view_end(view: &gltf::buffer::View<'_>) -> Option<usize> {
    view.offset().checked_add(view.length())
}

/// Whether one strided walk of `count` elements of `size` bytes is one
/// `Iter::new` can perform, phrased for the refusal message when it is not.
fn unwalkable(
    subject: &str,
    view: &gltf::buffer::View<'_>,
    offset: usize,
    count: usize,
    size: usize,
) -> Option<String> {
    // `Iter::new` slices the view before it strides over anything, so the
    // view's own extent is judged before the accessor's.
    let Some(view_end) = view_end(view) else {
        return Some(format!(
            "reads its {subject} from buffer view {}, whose byteOffset {} plus byteLength {} \
             is a byte extent that overflows",
            view.index(),
            view.offset(),
            view.length()
        ));
    };
    if view_end > view.buffer().length() {
        return Some(format!(
            "reads its {subject} from buffer view {}, whose byte extent ends at {view_end} \
             beyond buffer {}'s byteLength {}",
            view.index(),
            view.buffer().index(),
            view.buffer().length()
        ));
    }
    let stride = view.stride().unwrap_or(size);
    if stride < size {
        return Some(format!(
            "reads its {subject} from buffer view {} at byteStride {stride}, \
             shorter than the {size}-byte element it strides over",
            view.index()
        ));
    }
    let required_end = count
        .checked_sub(1)
        .and_then(|last| stride.checked_mul(last))
        .and_then(|span| span.checked_add(offset))
        .and_then(|end| end.checked_add(size));
    // `count` 0 never reaches that arithmetic: no read site builds a reader
    // for a count-zero accessor, so its underflow is unreachable rather than
    // refused.
    if count == 0 {
        return None;
    }
    let Some(required_end) = required_end else {
        return Some(format!(
            "walks {count} {subject} of {size} bytes at byteStride {stride} \
             from byteOffset {offset}, a byte extent that overflows"
        ));
    };
    (required_end > view.length()).then(|| {
        format!(
            "walks {count} {subject} of {size} bytes at byteStride {stride} from byteOffset \
             {offset}, requiring byte extent {required_end} beyond buffer view {}'s byteLength {}",
            view.index(),
            view.length()
        )
    })
}

/// Whether a skin's `inverseBindMatrices` accessor is one
/// `read_inverse_bind_matrices` can decode — both the element it declares
/// and the layout it declares it in.
///
/// An unreadable one panics exactly like an unreadable vertex attribute,
/// but it is not a load error: the loader's contract is that an unusable
/// inverse-bind declaration is *source evidence* — the skin's accessor
/// state is reported as [`SourceInverseBindAccessorStatus::Unreadable`] and
/// the rest of the file still measures. Refusing the document would replace
/// that evidence with an exit code. So the three inverse-bind read sites
/// gate on this instead, and never build the reader when it is false.
fn inverse_bind_is_readable(accessor: &gltf::Accessor<'_>) -> bool {
    encoding_matches(accessor, &INVERSE_BIND_ENCODING) && unreadable_layout(accessor).is_none()
}

fn encoding_matches(accessor: &gltf::Accessor<'_>, required: &ReaderEncoding) -> bool {
    accessor.dimensions() == required.accessor_type
        && required.component_types.contains(&accessor.data_type())
        && (!required.normalized_integers
            || accessor.data_type() == ComponentType::F32
            || accessor.normalized())
}

/// Render an accepted encoding in the file's own vocabulary, so the refusal
/// reads as the glTF the author would have to write.
fn describe_encoding(required: &ReaderEncoding) -> String {
    let names: Vec<String> = required
        .component_types
        .iter()
        .copied()
        .map(|component| {
            let name = component_type_name(component);
            if required.normalized_integers && component != ComponentType::F32 {
                format!("normalized {name}")
            } else {
                name.to_owned()
            }
        })
        .collect();
    let components = match names.as_slice() {
        [] => String::new(),
        [only] => only.clone(),
        [first, last] => format!("{first} or {last}"),
        [rest @ .., last] => format!("{}, or {last}", rest.join(", ")),
    };
    format!(
        "{} of {components}",
        accessor_type_name(required.accessor_type)
    )
}

fn accessor_type_name(accessor_type: AccessorType) -> &'static str {
    match accessor_type {
        AccessorType::Scalar => "SCALAR",
        AccessorType::Vec2 => "VEC2",
        AccessorType::Vec3 => "VEC3",
        AccessorType::Vec4 => "VEC4",
        AccessorType::Mat2 => "MAT2",
        AccessorType::Mat3 => "MAT3",
        AccessorType::Mat4 => "MAT4",
    }
}

fn component_type_name(component_type: ComponentType) -> &'static str {
    match component_type {
        ComponentType::I8 => "BYTE",
        ComponentType::U8 => "UNSIGNED_BYTE",
        ComponentType::I16 => "SHORT",
        ComponentType::U16 => "UNSIGNED_SHORT",
        ComponentType::U32 => "UNSIGNED_INT",
        ComponentType::F32 => "FLOAT",
    }
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
/// animation channels, geometry or animation accessors typed for an element
/// the selected reader cannot decode or laid out so it cannot walk them, or
/// node graphs that cannot be represented as a skeleton forest.
pub fn load(path: &Path) -> Result<Document, LoadError> {
    load_source(path).map(LoadedSource::into_document)
}

/// Load a `.gltf` or `.glb` file with immutable importer-sensitive source facts.
///
/// The returned owner binds the normalized document and raw facts to the exact
/// primary bytes read here. It intentionally exposes no mutable document
/// access; consume it with [`LoadedSource::into_document`] to discard the
/// sidecar and recover the legacy document-only value.
///
/// # Errors
///
/// Returns [`LoadError`] under the same conditions as [`load`]. Projection
/// budget exhaustion is represented as partial fact coverage and is never a
/// load failure.
pub fn load_source(path: &Path) -> Result<LoadedSource, LoadError> {
    let bytes = std::fs::read(path).map_err(|source| LoadError::Io {
        path: path.display().to_string(),
        source,
    })?;
    let root = path.parent().unwrap_or_else(|| Path::new("."));
    load_source_bytes_with_resource_root(path, &bytes, root)
}

/// Load a `.glb` or `.gltf` byte slice into a core [`Document`].
///
/// `bytes` supplies the top-level container exactly as captured by the
/// caller. `path` is retained for source provenance and diagnostics only;
/// captured-byte inputs need an explicit root API before any external
/// resource may be resolved.
///
/// # Errors
///
/// Returns [`LoadError`] for unsafe or missing external buffers, malformed
/// GLB framing, parser rejection, structurally invalid animation channels,
/// geometry or animation accessors typed for an element the selected reader
/// cannot decode or laid out so it cannot walk them, or node graphs that
/// cannot be represented as a skeleton forest.
pub fn load_bytes(path: &Path, bytes: &[u8]) -> Result<Document, LoadError> {
    load_source_bytes(path, bytes).map(LoadedSource::into_document)
}

/// Load captured bytes with an explicit trusted local root for external resources.
///
/// The root is capability input from the caller; it is never retained in raw
/// facts, the dependency closure, diagnostics for source-controlled locators,
/// or its digest.
///
/// # Errors
///
/// Returns [`LoadError`] under the same conditions as [`load_bytes`], while
/// allowing safe external resources to resolve under `resource_root`.
pub fn load_bytes_with_resource_root(
    path: &Path,
    bytes: &[u8],
    resource_root: &Path,
) -> Result<Document, LoadError> {
    load_source_bytes_with_resource_root(path, bytes, resource_root)
        .map(LoadedSource::into_document)
}

/// Load captured `.gltf` or `.glb` bytes with immutable raw-source facts.
///
/// `bytes` is both the parser input and the authority for
/// [`animsmith_core::InputIdentity`]. This self-contained entry point refuses
/// safe relative external declarations because captured bytes alone do not
/// authorize a local filesystem root; use
/// [`load_source_bytes_with_resource_root`] for such inputs.
///
/// # Errors
///
/// Returns [`LoadError`] under the same conditions as [`load_bytes`]. A
/// source-facts projection limit produces a successful value with partial
/// coverage.
pub fn load_source_bytes(path: &Path, bytes: &[u8]) -> Result<LoadedSource, LoadError> {
    load_source_bytes_inner(path, bytes, None)
}

/// Load captured bytes with an explicit trusted local root for external resources.
///
/// `resource_root` is an authority supplied by the caller, not a source fact.
/// Resource keys are normalized and opened only beneath that root. The final
/// root and every locator-derived symlink component are refused; ancestors of
/// the explicitly supplied root are part of the caller's capability path. The
/// root itself never enters public evidence.
///
/// # Errors
///
/// Returns [`LoadError`] for a malformed source, an essential unavailable
/// external buffer, or an unsafe resource declaration. Missing/unreadable
/// external images remain typed unavailable source evidence.
pub fn load_source_bytes_with_resource_root(
    path: &Path,
    bytes: &[u8],
    resource_root: &Path,
) -> Result<LoadedSource, LoadError> {
    load_source_bytes_inner(path, bytes, Some(resource_root))
}

fn load_source_bytes_inner(
    path: &Path,
    bytes: &[u8],
    resource_root: Option<&Path>,
) -> Result<LoadedSource, LoadError> {
    load_source_bytes_inner_with_reader(path, bytes, resource_root, read_external_file)
}

fn load_source_bytes_inner_with_reader<F>(
    path: &Path,
    bytes: &[u8],
    resource_root: Option<&Path>,
    mut read_external: F,
) -> Result<LoadedSource, LoadError>
where
    F: FnMut(&Path, u64) -> CapturedResource,
{
    // Parse from the supplied slice rather than via `Gltf::open`: the reader
    // path (`Glb::from_reader`) trusts the GLB header's declared length and
    // pre-allocates `vec![0; declared_len]` before reading a byte, so a
    // spoofed length OOMs on tiny input. The slice path validates the declared
    // length against the bytes actually present, keeping malformed containers
    // within invariant-1 (LoadError, never an unbounded allocation). This
    // mirrors what `fix` already does.
    validate_glb_framing(bytes)?;
    // Keep the legacy loader's strict validation boundary: required
    // extensions the parser cannot implement remain load errors. The
    // permissive validator is reserved for scale preflight, which must
    // inventory unsupported declarations before refusing an operation.
    let gltf = gltf::Gltf::from_slice(bytes)?;
    validate_animations(&gltf.document)?;
    let mut facts = source_facts_builder(bytes)?;
    project_extension_facts(&gltf.document, &mut facts);
    project_resource_facts(&gltf.document, &mut facts);
    let has_unmodeled_extension_domain = has_extension_object(bytes)
        || gltf.document.extensions_used().next().is_some()
        || gltf.document.extensions_required().next().is_some();
    let (dependency_closure, mut resources) = capture_dependency_closure(
        &facts,
        resource_root,
        has_unmodeled_extension_domain,
        &mut read_external,
    )?;
    let buffers = resolve_captured_buffers(&gltf, &mut resources)?;
    validate_primitive_accessors(&gltf.document, &buffers)?;
    // Derive the node topology once and share it: the skeleton build and
    // asset extraction must agree on which bone each node became, and it is
    // also where malformed graphs are rejected (so that runs once too).
    let topo = topology(&gltf.document)?;
    let source_skeleton = extract_source_skeleton(&gltf.document, &buffers, &topo);
    let mut doc = build_document(&gltf, &buffers, path, &topo, &mut facts)?;
    doc.assets = extract_assets(&gltf.document, &buffers, &mut resources, &topo.bone_of_node);
    doc.assets.scenes = extract_scenes(&gltf.document, &topo.bone_of_node);
    doc.assets.default_scene = gltf.document.default_scene().map(|scene| scene.index());
    doc.assets.source_skeleton = source_skeleton;
    let source = facts
        .finish_with_dependency_closure(doc, dependency_closure)
        .map_err(LoadError::from)?;
    let addressability_inventory = addressability::project(
        &gltf.document,
        &topo,
        source.source_facts().primary_identity().clone(),
        source.dependency_closure().clone(),
    )
    .map_err(|error| {
        LoadError::Malformed(format!("raw glTF addressability projection: {error}"))
    })?;
    let inventory = capability::raw_scene_attachment_inventory_from_bytes(bytes, &source)?;
    source
        .with_raw_gltf_addressability_inventory(addressability_inventory)
        .map_err(|error| LoadError::Malformed(format!("raw glTF addressability binding: {error}")))?
        .with_raw_scene_attachment_inventory(inventory)
        .map_err(|error| LoadError::Malformed(format!("raw scene/attachment binding: {error}")))
}

fn source_facts_builder(primary_bytes: &[u8]) -> Result<RawSourceFactsBuilderV1, SourceFactsError> {
    let format = if primary_bytes.starts_with(b"glTF") {
        SourceFormatV1::Glb
    } else {
        SourceFormatV1::GltfJson
    };
    let mut facts = RawSourceFactsBuilderV1::new(format, InputIdentity::from_bytes(primary_bytes));
    facts.set_linear_unit(SourceObservationV1::observed(
        SourceLinearUnitV1::new(1.0)?,
        SourceProvenanceV1::format_defined(),
        SourceLoaderDispositionV1::Preserved,
    ));
    facts.set_coordinate_basis(SourceObservationV1::observed(
        SourceCoordinateBasisV1::new(
            SourceAxisV1::PositiveX,
            SourceAxisV1::PositiveY,
            SourceAxisV1::PositiveZ,
        )?,
        SourceProvenanceV1::format_defined(),
        SourceLoaderDispositionV1::Preserved,
    ));
    facts.set_frames_per_second(SourceObservationV1::proven_absent(
        SourceProvenanceV1::format_defined(),
    ));

    Ok(facts)
}

fn project_extension_facts(document: &gltf::Document, facts: &mut RawSourceFactsBuilderV1) {
    let mut source_order_index = 0;
    for name in document.extensions_used() {
        if !project_extension_declaration(name, false, "/extensionsUsed", source_order_index, facts)
        {
            return;
        }
        source_order_index += 1;
    }
    for name in document.extensions_required() {
        if !project_extension_declaration(
            name,
            true,
            "/extensionsRequired",
            source_order_index,
            facts,
        ) {
            return;
        }
        source_order_index += 1;
    }
    facts.mark_complete(SourceFactDomainV1::Constructs);
}

fn project_extension_declaration(
    name: &str,
    required: bool,
    provenance: &'static str,
    source_order_index: usize,
    facts: &mut RawSourceFactsBuilderV1,
) -> bool {
    if facts.remaining_observation_rows() == 0 {
        facts.mark_budget_exceeded(SourceFactDomainV1::Constructs);
        return false;
    }
    if name.len().saturating_add(provenance.len()) > facts.remaining_text_bytes()
        || name.len() > RAW_SOURCE_V1_MAX_TEXT_BYTES
    {
        facts.mark_budget_exceeded(SourceFactDomainV1::Constructs);
        return false;
    }
    let row = SourceConstructFactV1::new(
        source_order_index,
        SourceConstructKindV1::Extension,
        SourceTextV1::new(name).expect("extension name was checked against the text bound"),
        required,
        1,
        SourceLoaderDispositionV1::Unsupported,
        located_provenance(
            SourceProvenanceKindV1::SourceDeclared,
            provenance.to_owned(),
        ),
    )
    .expect("an extension declaration has a positive count");
    facts.push_construct(row)
}

fn project_resource_facts(document: &gltf::Document, facts: &mut RawSourceFactsBuilderV1) {
    let mut source_order_index = 0;
    for buffer in document.buffers() {
        if facts.remaining_resource_rows() == 0 || facts.remaining_observation_rows() == 0 {
            facts.mark_budget_exceeded(SourceFactDomainV1::Resources);
            return;
        }
        let (possible_locator_bytes, pointer_len) = match buffer.source() {
            gltf::buffer::Source::Bin => (0, "/buffers/".len() + decimal_len(buffer.index())),
            gltf::buffer::Source::Uri(uri) => (
                SourceResourceLocatorV1::retained_relative_bytes(uri),
                "/buffers/".len() + decimal_len(buffer.index()) + "/uri".len(),
            ),
        };
        if possible_locator_bytes.saturating_add(pointer_len) > facts.remaining_text_bytes() {
            facts.mark_budget_exceeded(SourceFactDomainV1::Resources);
            return;
        }
        let (locator, pointer) = match buffer.source() {
            gltf::buffer::Source::Bin => (
                SourceResourceLocatorV1::Embedded,
                format!("/buffers/{}", buffer.index()),
            ),
            gltf::buffer::Source::Uri(uri) => (
                SourceResourceLocatorV1::classify(uri),
                format!("/buffers/{}/uri", buffer.index()),
            ),
        };
        if !facts.push_resource(SourceResourceReferenceV1::new(
            source_order_index,
            SourceResourceKindV1::Buffer,
            buffer.index() as u64,
            locator,
            SourceLoaderDispositionV1::Preserved,
            located_provenance(SourceProvenanceKindV1::SourceDeclared, pointer),
        )) {
            return;
        }
        source_order_index += 1;
    }
    for image in document.images() {
        if facts.remaining_resource_rows() == 0 || facts.remaining_observation_rows() == 0 {
            facts.mark_budget_exceeded(SourceFactDomainV1::Resources);
            return;
        }
        let (possible_locator_bytes, pointer_len) = match image.source() {
            gltf::image::Source::View { .. } => (0, "/images/".len() + decimal_len(image.index())),
            gltf::image::Source::Uri { uri, .. } => (
                SourceResourceLocatorV1::retained_relative_bytes(uri),
                "/images/".len() + decimal_len(image.index()) + "/uri".len(),
            ),
        };
        if possible_locator_bytes.saturating_add(pointer_len) > facts.remaining_text_bytes() {
            facts.mark_budget_exceeded(SourceFactDomainV1::Resources);
            return;
        }
        let (locator, pointer) = match image.source() {
            gltf::image::Source::View { .. } => (
                SourceResourceLocatorV1::Embedded,
                format!("/images/{}", image.index()),
            ),
            gltf::image::Source::Uri { uri, .. } => (
                SourceResourceLocatorV1::classify(uri),
                format!("/images/{}/uri", image.index()),
            ),
        };
        if !facts.push_resource(SourceResourceReferenceV1::new(
            source_order_index,
            SourceResourceKindV1::Image,
            image.index() as u64,
            locator,
            SourceLoaderDispositionV1::Preserved,
            located_provenance(SourceProvenanceKindV1::SourceDeclared, pointer),
        )) {
            return;
        }
        source_order_index += 1;
    }
    facts.mark_complete(SourceFactDomainV1::Resources);
}

fn located_provenance(kind: SourceProvenanceKindV1, locator: String) -> SourceProvenanceV1 {
    let locator = SourceLogicalLocatorV1::gltf_json_pointer(locator)
        .expect("generated glTF locator is valid and bounded");
    match kind {
        SourceProvenanceKindV1::SourceDeclared => SourceProvenanceV1::source_declared(locator),
        SourceProvenanceKindV1::ParserProjected => SourceProvenanceV1::parser_projected(locator),
        SourceProvenanceKindV1::DerivedFromSource => {
            SourceProvenanceV1::derived_from_source(locator)
        }
        SourceProvenanceKindV1::FormatDefined => {
            unreachable!("located glTF provenance is never format-defined")
        }
    }
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
                                LoadError::Buffer("unsupported data URI in buffer".to_owned())
                            })?;
                    base64::engine::general_purpose::STANDARD
                        .decode(payload)
                        .map_err(|e| LoadError::Buffer(format!("bad base64 data URI: {e}")))?
                } else {
                    let root = base.ok_or_else(resource_root_required)?;
                    let path = root.join(safe_external_buffer_path(uri)?);
                    std::fs::read(&path).map_err(|source| LoadError::Io {
                        // Do not reproduce a source-controlled resource
                        // locator or its resolved host path through the new
                        // evidence-aware API's error surface.
                        path: "<external glTF buffer>".to_owned(),
                        source,
                    })?
                }
            }
        };
        buffers.push(data);
    }
    Ok(buffers)
}

/// Loader-private cap on duplicate external `Vec` slots. Closure I/O records
/// unique opened/hashed bytes separately, so aliases cannot multiply memory.
const MAX_EXTERNAL_MATERIALIZED_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Clone, Copy)]
enum CapturedResourceFailure {
    Refused(DependencyResourceRefusalReasonV1),
    Unavailable(DependencyResourceUnavailableReasonV1),
}

enum CapturedResource {
    Bytes(Vec<u8>),
    Failure(CapturedResourceFailure),
}

#[derive(Clone)]
enum CapturedReference {
    Primary,
    External(DependencyResourceKeyV1),
    Failure(CapturedResourceFailure),
}

/// One bounded local-file resource capture tied to a single loader invocation.
///
/// The map keeps exact bytes only until all document consumers have reused
/// them. The resulting core closure receives identities and safe logical keys,
/// never this root or any resolved host path.
struct ResourceCaptureSession {
    root: TrustedResourceRoot,
    resources: BTreeMap<DependencyResourceKeyV1, CapturedResource>,
    references: BTreeMap<(SourceResourceKindV1, u64), CapturedReference>,
    materialized_external_bytes: u64,
    materialized_external_limit: u64,
}

enum TrustedResourceRoot {
    Absent,
    Available(PathBuf),
    Failure(CapturedResourceFailure),
}

impl ResourceCaptureSession {
    fn new(root: Option<&Path>) -> Self {
        Self {
            root: trusted_resource_root(root),
            resources: BTreeMap::new(),
            references: BTreeMap::new(),
            materialized_external_bytes: 0,
            materialized_external_limit: MAX_EXTERNAL_MATERIALIZED_BYTES,
        }
    }

    fn insert_reference(
        &mut self,
        kind: SourceResourceKindV1,
        source_index: u64,
        reference: CapturedReference,
    ) {
        self.references.insert((kind, source_index), reference);
    }

    fn reference(&self, kind: SourceResourceKindV1, source_index: u64) -> CapturedReference {
        self.references
            .get(&(kind, source_index))
            .cloned()
            .unwrap_or(CapturedReference::Failure(
                CapturedResourceFailure::Unavailable(
                    DependencyResourceUnavailableReasonV1::ResourceBudgetExceeded,
                ),
            ))
    }

    /// Validate every resource component before an external open. This makes a
    /// symlink a pure refusal: no source-controlled file is opened first.
    fn preflight_external(
        &self,
        key: &DependencyResourceKeyV1,
    ) -> Result<PathBuf, CapturedResourceFailure> {
        let root = match &self.root {
            TrustedResourceRoot::Available(root) => root,
            TrustedResourceRoot::Absent => {
                return Err(CapturedResourceFailure::Unavailable(
                    DependencyResourceUnavailableReasonV1::ResourceRootUnavailable,
                ));
            }
            TrustedResourceRoot::Failure(failure) => return Err(*failure),
        };
        let mut path = root.clone();
        let mut components = key.as_str().split('/').peekable();
        while let Some(component) = components.next() {
            let is_final = components.peek().is_none();
            path.push(component);
            match std::fs::symlink_metadata(&path) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(CapturedResourceFailure::Refused(
                        DependencyResourceRefusalReasonV1::Symlink,
                    ));
                }
                Ok(metadata)
                    if (!is_final && metadata.is_dir()) || (is_final && metadata.is_file()) => {}
                Ok(_) => {
                    return Err(CapturedResourceFailure::Unavailable(
                        DependencyResourceUnavailableReasonV1::Unreadable,
                    ));
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    return Err(CapturedResourceFailure::Unavailable(
                        DependencyResourceUnavailableReasonV1::Missing,
                    ));
                }
                Err(_) => {
                    return Err(CapturedResourceFailure::Unavailable(
                        DependencyResourceUnavailableReasonV1::Unreadable,
                    ));
                }
            }
        }
        Ok(path)
    }

    fn materialize_external(
        &mut self,
        kind: SourceResourceKindV1,
        source_index: u64,
    ) -> Result<Vec<u8>, CapturedResourceFailure> {
        let CapturedReference::External(key) = self.reference(kind, source_index) else {
            return Err(reference_failure(self.reference(kind, source_index)));
        };
        let length = match self.resources.get(&key) {
            Some(CapturedResource::Bytes(bytes)) => bytes.len() as u64,
            Some(CapturedResource::Failure(failure)) => return Err(*failure),
            None => {
                return Err(CapturedResourceFailure::Unavailable(
                    DependencyResourceUnavailableReasonV1::ResourceBudgetExceeded,
                ));
            }
        };
        let Some(next) = self.materialized_external_bytes.checked_add(length) else {
            return Err(CapturedResourceFailure::Unavailable(
                DependencyResourceUnavailableReasonV1::ResourceBudgetExceeded,
            ));
        };
        if next > self.materialized_external_limit {
            return Err(CapturedResourceFailure::Unavailable(
                DependencyResourceUnavailableReasonV1::ResourceBudgetExceeded,
            ));
        }
        let bytes = match self.resources.get(&key) {
            Some(CapturedResource::Bytes(bytes)) => bytes.clone(),
            _ => unreachable!("captured resource state changed without mutation"),
        };
        self.materialized_external_bytes = next;
        Ok(bytes)
    }

    fn external_image_payload(
        &self,
        image_index: usize,
    ) -> (Option<&[u8]>, ImageUnavailableReason) {
        let CapturedReference::External(key) =
            self.reference(SourceResourceKindV1::Image, image_index as u64)
        else {
            return (None, ImageUnavailableReason::SourceUnavailable);
        };
        match self.resources.get(&key) {
            Some(CapturedResource::Bytes(bytes)) => (
                Some(bytes.as_slice()),
                ImageUnavailableReason::SourceUnavailable,
            ),
            _ => (None, ImageUnavailableReason::SourceUnavailable),
        }
    }

    fn external_image_is_available(&self, image_index: usize) -> bool {
        self.external_image_payload(image_index).0.is_some()
    }

    fn clone_image_for_material(
        &mut self,
        image_index: usize,
        texture: &TextureAsset,
    ) -> Option<TextureAsset> {
        if matches!(
            self.reference(SourceResourceKindV1::Image, image_index as u64),
            CapturedReference::External(_)
        ) {
            let length = texture.bytes.len() as u64;
            let next = self.materialized_external_bytes.checked_add(length)?;
            if next > self.materialized_external_limit {
                return None;
            }
            self.materialized_external_bytes = next;
        }
        Some(texture.clone())
    }
}

/// Read one already-preflighted resource exactly once. `limit + 1` is a
/// bounded witness for the core closure's terminal resource-budget row.
fn read_external_file(path: &Path, limit: u64) -> CapturedResource {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return CapturedResource::Failure(CapturedResourceFailure::Unavailable(
                DependencyResourceUnavailableReasonV1::Missing,
            ));
        }
        Err(_) => {
            return CapturedResource::Failure(CapturedResourceFailure::Unavailable(
                DependencyResourceUnavailableReasonV1::Unreadable,
            ));
        }
    };
    let max_read = limit.saturating_add(1);
    let mut bytes = Vec::new();
    let read = file.take(max_read).read_to_end(&mut bytes);
    if read.is_err() {
        return CapturedResource::Failure(CapturedResourceFailure::Unavailable(
            DependencyResourceUnavailableReasonV1::Unreadable,
        ));
    }
    CapturedResource::Bytes(bytes)
}

/// Validate the caller-supplied root itself without inspecting its ancestors.
///
/// Ancestors are part of the capability path the caller explicitly supplied;
/// only the final root and locator-derived children belong to this loader's
/// symlink-refusal boundary. A relative root is made absolute once here, never
/// inferred from a source locator.
fn trusted_resource_root(root: Option<&Path>) -> TrustedResourceRoot {
    let Some(root) = root else {
        return TrustedResourceRoot::Absent;
    };
    let root = if root.is_absolute() {
        root.to_path_buf()
    } else {
        match std::env::current_dir() {
            Ok(current) => current.join(root),
            Err(_) => {
                return TrustedResourceRoot::Failure(CapturedResourceFailure::Unavailable(
                    DependencyResourceUnavailableReasonV1::ResourceRootUnavailable,
                ));
            }
        }
    };
    match std::fs::symlink_metadata(&root) {
        Ok(metadata) if metadata.is_dir() => TrustedResourceRoot::Available(root),
        Ok(metadata) if metadata.file_type().is_symlink() => TrustedResourceRoot::Failure(
            CapturedResourceFailure::Refused(DependencyResourceRefusalReasonV1::Symlink),
        ),
        Ok(_) | Err(_) => TrustedResourceRoot::Failure(CapturedResourceFailure::Unavailable(
            DependencyResourceUnavailableReasonV1::ResourceRootUnavailable,
        )),
    }
}

fn reference_failure(reference: CapturedReference) -> CapturedResourceFailure {
    match reference {
        CapturedReference::Failure(failure) => failure,
        CapturedReference::Primary | CapturedReference::External(_) => {
            CapturedResourceFailure::Unavailable(
                DependencyResourceUnavailableReasonV1::ResourceBudgetExceeded,
            )
        }
    }
}

fn capture_dependency_closure<F>(
    facts: &RawSourceFactsBuilderV1,
    root: Option<&Path>,
    has_unmodeled_resource_domain: bool,
    read_external: &mut F,
) -> Result<(DependencyClosureV1, ResourceCaptureSession), LoadError>
where
    F: FnMut(&Path, u64) -> CapturedResource,
{
    let mut closure = DependencyClosureBuilderV1::new(
        facts.primary_identity().clone(),
        facts.resource_coverage(),
        facts.resource_rows().len(),
    );
    if has_unmodeled_resource_domain {
        closure.mark_unmodeled_resource_domain();
    }
    let mut session = ResourceCaptureSession::new(root);
    for row in facts.resource_rows() {
        let (locator_bytes, components) = match row.locator() {
            SourceResourceLocatorV1::Relative(locator) => (
                locator.as_str().len(),
                DependencyResourceKeyV1::source_component_count(locator),
            ),
            _ => (0, 0),
        };
        if !closure.begin_reference(locator_bytes, components) {
            break;
        }
        let kind = row.kind();
        let source_index = row.source_index();
        let source_order_index = row.source_order_index();
        match row.locator() {
            SourceResourceLocatorV1::Embedded | SourceResourceLocatorV1::DataUri => {
                closure
                    .push_primary(source_order_index, kind, source_index)
                    .map_err(SourceFactsError::from)?;
                session.insert_reference(kind, source_index, CapturedReference::Primary);
            }
            SourceResourceLocatorV1::Absolute => {
                record_refused(
                    &mut closure,
                    &mut session,
                    source_order_index,
                    kind,
                    source_index,
                    DependencyResourceRefusalReasonV1::Absolute,
                )?;
            }
            SourceResourceLocatorV1::Escaping => {
                record_refused(
                    &mut closure,
                    &mut session,
                    source_order_index,
                    kind,
                    source_index,
                    DependencyResourceRefusalReasonV1::Escaping,
                )?;
            }
            SourceResourceLocatorV1::Remote => {
                record_refused(
                    &mut closure,
                    &mut session,
                    source_order_index,
                    kind,
                    source_index,
                    DependencyResourceRefusalReasonV1::Remote,
                )?;
            }
            SourceResourceLocatorV1::Malformed => {
                record_refused(
                    &mut closure,
                    &mut session,
                    source_order_index,
                    kind,
                    source_index,
                    DependencyResourceRefusalReasonV1::Malformed,
                )?;
            }
            SourceResourceLocatorV1::Oversized => {
                record_refused(
                    &mut closure,
                    &mut session,
                    source_order_index,
                    kind,
                    source_index,
                    DependencyResourceRefusalReasonV1::Oversized,
                )?;
            }
            SourceResourceLocatorV1::Missing => {
                closure
                    .push_unavailable(
                        source_order_index,
                        kind,
                        source_index,
                        None,
                        DependencyResourceUnavailableReasonV1::Missing,
                    )
                    .map_err(SourceFactsError::from)?;
                session.insert_reference(
                    kind,
                    source_index,
                    CapturedReference::Failure(CapturedResourceFailure::Unavailable(
                        DependencyResourceUnavailableReasonV1::Missing,
                    )),
                );
            }
            SourceResourceLocatorV1::Relative(locator) => {
                // Captured bytes carry no ambient filesystem authority. This
                // applies to optional images as well as essential buffers.
                if root.is_none() {
                    return Err(resource_root_required());
                }
                let key = match DependencyResourceKeyV1::from_relative(
                    locator,
                    ResourceKeySyntaxV1::GltfUri,
                ) {
                    Ok(key) => key,
                    Err(error) => {
                        let reason = match error {
                            DependencyClosureError::ResourceKeyTooLong { .. }
                            | DependencyClosureError::TooManyPathComponents { .. } => {
                                DependencyResourceRefusalReasonV1::Oversized
                            }
                            _ => DependencyResourceRefusalReasonV1::Malformed,
                        };
                        record_refused(
                            &mut closure,
                            &mut session,
                            source_order_index,
                            kind,
                            source_index,
                            reason,
                        )?;
                        continue;
                    }
                };
                match closure
                    .prepare_external_key(&key)
                    .map_err(SourceFactsError::from)?
                {
                    None => break,
                    Some(false) => {
                        let reference = session
                            .resources
                            .get(&key)
                            .map(|resource| match resource {
                                CapturedResource::Bytes(_) => {
                                    CapturedReference::External(key.clone())
                                }
                                CapturedResource::Failure(failure) => {
                                    CapturedReference::Failure(*failure)
                                }
                            })
                            .unwrap_or(CapturedReference::Failure(
                                CapturedResourceFailure::Unavailable(
                                    DependencyResourceUnavailableReasonV1::ResourceBudgetExceeded,
                                ),
                            ));
                        record_cached_reference(
                            &mut closure,
                            &mut session,
                            source_order_index,
                            kind,
                            source_index,
                            key,
                            reference,
                        )?;
                    }
                    Some(true) => {
                        let limit = closure
                            .max_resource_bytes()
                            .min(closure.remaining_external_bytes());
                        let resource = match session.preflight_external(&key) {
                            Ok(path) => {
                                // Count the attempt immediately before the
                                // only File::open, after all refusal checks.
                                closure
                                    .record_external_open_attempt(&key)
                                    .map_err(SourceFactsError::from)?;
                                read_external(&path, limit)
                            }
                            Err(failure) => CapturedResource::Failure(failure),
                        };
                        let reference = match &resource {
                            CapturedResource::Bytes(bytes) => {
                                let identity = InputIdentity::from_bytes(bytes);
                                let captured = closure
                                    .push_captured_external(
                                        source_order_index,
                                        kind,
                                        source_index,
                                        key.clone(),
                                        identity,
                                    )
                                    .map_err(SourceFactsError::from)?;
                                if captured {
                                    CapturedReference::External(key.clone())
                                } else {
                                    CapturedReference::Failure(
                                        CapturedResourceFailure::Unavailable(
                                            DependencyResourceUnavailableReasonV1::ResourceBudgetExceeded,
                                        ),
                                    )
                                }
                            }
                            CapturedResource::Failure(failure) => {
                                record_failure(
                                    &mut closure,
                                    source_order_index,
                                    kind,
                                    source_index,
                                    Some(key.clone()),
                                    *failure,
                                )?;
                                CapturedReference::Failure(*failure)
                            }
                        };
                        let resource = match reference {
                            CapturedReference::Failure(failure)
                                if matches!(&resource, CapturedResource::Bytes(_)) =>
                            {
                                CapturedResource::Failure(failure)
                            }
                            _ => resource,
                        };
                        session.resources.insert(key, resource);
                        session.insert_reference(kind, source_index, reference);
                    }
                }
            }
        }
    }
    let closure = closure.finish().map_err(SourceFactsError::from)?;
    Ok((closure, session))
}

fn record_refused(
    closure: &mut DependencyClosureBuilderV1,
    session: &mut ResourceCaptureSession,
    source_order_index: usize,
    kind: SourceResourceKindV1,
    source_index: u64,
    reason: DependencyResourceRefusalReasonV1,
) -> Result<(), LoadError> {
    closure
        .push_refused(source_order_index, kind, source_index, reason)
        .map_err(SourceFactsError::from)?;
    session.insert_reference(
        kind,
        source_index,
        CapturedReference::Failure(CapturedResourceFailure::Refused(reason)),
    );
    Ok(())
}

fn record_failure(
    closure: &mut DependencyClosureBuilderV1,
    source_order_index: usize,
    kind: SourceResourceKindV1,
    source_index: u64,
    key: Option<DependencyResourceKeyV1>,
    failure: CapturedResourceFailure,
) -> Result<(), LoadError> {
    match failure {
        CapturedResourceFailure::Refused(reason) => closure
            .push_refused(source_order_index, kind, source_index, reason)
            .map_err(SourceFactsError::from)?,
        CapturedResourceFailure::Unavailable(reason) => closure
            .push_unavailable(source_order_index, kind, source_index, key, reason)
            .map_err(SourceFactsError::from)?,
    }
    Ok(())
}

fn record_cached_reference(
    closure: &mut DependencyClosureBuilderV1,
    session: &mut ResourceCaptureSession,
    source_order_index: usize,
    kind: SourceResourceKindV1,
    source_index: u64,
    key: DependencyResourceKeyV1,
    reference: CapturedReference,
) -> Result<(), LoadError> {
    match &reference {
        CapturedReference::External(_) => closure
            .push_external_alias(source_order_index, kind, source_index, key)
            .map_err(SourceFactsError::from)?,
        CapturedReference::Failure(failure) => record_failure(
            closure,
            source_order_index,
            kind,
            source_index,
            Some(key),
            *failure,
        )?,
        CapturedReference::Primary => unreachable!("external aliases never map to primary"),
    }
    session.insert_reference(kind, source_index, reference);
    Ok(())
}

fn resolve_captured_buffers(
    gltf: &gltf::Gltf,
    resources: &mut ResourceCaptureSession,
) -> Result<Vec<Vec<u8>>, LoadError> {
    let mut buffers = Vec::new();
    for buffer in gltf.buffers() {
        let data = match buffer.source() {
            gltf::buffer::Source::Bin => gltf
                .blob
                .clone()
                .ok_or_else(|| LoadError::Buffer("GLB has no BIN chunk".into()))?,
            gltf::buffer::Source::Uri(uri) if uri.starts_with("data:") => {
                let payload = uri
                    .strip_prefix("data:")
                    .and_then(|encoded| encoded.split_once("base64,").map(|(_, payload)| payload))
                    .ok_or_else(|| {
                        LoadError::Buffer("unsupported data URI in buffer".to_owned())
                    })?;
                base64::engine::general_purpose::STANDARD
                    .decode(payload)
                    .map_err(|_| LoadError::Buffer("invalid data URI in buffer".to_owned()))?
            }
            gltf::buffer::Source::Uri(_) => resources
                .materialize_external(SourceResourceKindV1::Buffer, buffer.index() as u64)
                .map_err(buffer_capture_error)?,
        };
        buffers.push(data);
    }
    Ok(buffers)
}

fn buffer_capture_error(failure: CapturedResourceFailure) -> LoadError {
    match failure {
        CapturedResourceFailure::Refused(_) => {
            LoadError::ExternalResource(ExternalResourceFailure::Refused)
        }
        CapturedResourceFailure::Unavailable(
            DependencyResourceUnavailableReasonV1::ResourceRootUnavailable,
        ) => resource_root_required(),
        CapturedResourceFailure::Unavailable(
            DependencyResourceUnavailableReasonV1::ResourceBudgetExceeded,
        ) => LoadError::ExternalResource(ExternalResourceFailure::CaptureLimitExceeded),
        CapturedResourceFailure::Unavailable(_) => {
            LoadError::ExternalResource(ExternalResourceFailure::Unavailable)
        }
    }
}

fn resource_root_required() -> LoadError {
    LoadError::ExternalResource(ExternalResourceFailure::ResourceRootRequired)
}

struct PendingGltfClipFacts {
    animation_index: usize,
    source_name: SourceObservationV1<SourceTextV1>,
    normalized_clip_provenance: SourceProvenanceV1,
    range_provenance: SourceProvenanceV1,
    channels: Vec<SourceChannelFactV1>,
    channel_limit: usize,
    remaining_text: usize,
    minimum: f64,
    maximum: f64,
    saw_input: bool,
    sampler_inputs_available: bool,
    sampler_inputs_finite: bool,
    truncated: bool,
}

impl PendingGltfClipFacts {
    fn begin(
        animation: &gltf::Animation<'_>,
        builder: &mut RawSourceFactsBuilderV1,
    ) -> Option<Self> {
        if builder.remaining_clip_rows() == 0 || builder.remaining_observation_rows() == 0 {
            builder.mark_budget_exceeded(SourceFactDomainV1::Clips);
            return None;
        }
        let animation_index = animation.index();
        let prefix_len = "/animations/".len() + decimal_len(animation_index);
        let name_locator_len = prefix_len + "/name".len();
        let normalized_locator_len = prefix_len;
        let range_locator_len = prefix_len + "/samplers/*/input".len();
        let retained_name_len = animation.name().map_or(0, |name| {
            if name.len() <= RAW_SOURCE_V1_MAX_TEXT_BYTES {
                name.len()
            } else {
                0
            }
        });
        let fixed_text_len = name_locator_len
            .saturating_add(retained_name_len)
            .saturating_add(normalized_locator_len)
            .saturating_add(range_locator_len);
        if fixed_text_len > builder.remaining_text_bytes() {
            builder.mark_budget_exceeded(SourceFactDomainV1::Clips);
            return None;
        }

        let name_locator = format!("/animations/{animation_index}/name");
        let name_provenance =
            located_provenance(SourceProvenanceKindV1::SourceDeclared, name_locator);
        let source_name = match animation.name() {
            Some(name) if name.len() <= RAW_SOURCE_V1_MAX_TEXT_BYTES => {
                SourceObservationV1::observed(
                    SourceTextV1::new(name).expect("source name length checked before cloning"),
                    name_provenance,
                    SourceLoaderDispositionV1::Preserved,
                )
            }
            Some(_) => SourceObservationV1::unavailable(
                SourceUnavailableReasonV1::ProjectionBudgetExceeded,
                Some(name_provenance),
                SourceLoaderDispositionV1::Preserved,
            ),
            None => SourceObservationV1::proven_absent(name_provenance),
        };

        Some(Self {
            animation_index,
            source_name,
            normalized_clip_provenance: located_provenance(
                SourceProvenanceKindV1::ParserProjected,
                format!("/animations/{animation_index}"),
            ),
            range_provenance: located_provenance(
                SourceProvenanceKindV1::DerivedFromSource,
                format!("/animations/{animation_index}/samplers/*/input"),
            ),
            channels: Vec::new(),
            channel_limit: builder.remaining_observation_rows().saturating_sub(1),
            remaining_text: builder.remaining_text_bytes() - fixed_text_len,
            minimum: f64::INFINITY,
            maximum: f64::NEG_INFINITY,
            saw_input: false,
            sampler_inputs_available: true,
            sampler_inputs_finite: true,
            truncated: false,
        })
    }

    fn record_channel(&mut self, channel: &gltf::animation::Channel<'_>, times: Option<&[f32]>) {
        if self.channels.len() >= self.channel_limit {
            self.truncated = true;
            return;
        }
        let sampler = channel.sampler();
        let channel_index = channel.index();
        let interpolation_locator_len = "/animations/".len()
            + decimal_len(self.animation_index)
            + "/samplers/".len()
            + decimal_len(sampler.index())
            + "/interpolation".len();
        let channel_locator_len = "/animations/".len()
            + decimal_len(self.animation_index)
            + "/channels/".len()
            + decimal_len(channel_index);
        let row_text_len = interpolation_locator_len.saturating_add(channel_locator_len);
        if row_text_len > self.remaining_text {
            self.truncated = true;
            return;
        }

        let (property, components, disposition) = match channel.target().property() {
            gltf::animation::Property::Translation => (
                SourceChannelPropertyV1::Translation,
                SourceComponentMaskV1::new(true, true, true),
                SourceLoaderDispositionV1::Preserved,
            ),
            gltf::animation::Property::Rotation => (
                SourceChannelPropertyV1::Rotation,
                SourceComponentMaskV1::new(true, true, true),
                SourceLoaderDispositionV1::Preserved,
            ),
            gltf::animation::Property::Scale => (
                SourceChannelPropertyV1::Scale,
                SourceComponentMaskV1::new(true, true, true),
                SourceLoaderDispositionV1::Preserved,
            ),
            gltf::animation::Property::MorphTargetWeights => (
                SourceChannelPropertyV1::Weights,
                SourceComponentMaskV1::new(false, false, false),
                SourceLoaderDispositionV1::Discarded,
            ),
        };
        let interpolation = match sampler.interpolation() {
            gltf::animation::Interpolation::Linear => SourceInterpolationV1::Linear,
            gltf::animation::Interpolation::Step => SourceInterpolationV1::Step,
            gltf::animation::Interpolation::CubicSpline => SourceInterpolationV1::CubicSpline,
        };
        let interpolation_provenance = located_provenance(
            // `gltf` projects an omitted interpolation member to LINEAR, so
            // this typed value is parser-effective rather than proof that the
            // JSON member was explicitly authored.
            SourceProvenanceKindV1::ParserProjected,
            format!(
                "/animations/{}/samplers/{}/interpolation",
                self.animation_index,
                sampler.index()
            ),
        );
        let channel_provenance = located_provenance(
            SourceProvenanceKindV1::SourceDeclared,
            format!(
                "/animations/{}/channels/{channel_index}",
                self.animation_index
            ),
        );
        self.channels.push(
            SourceChannelFactV1::new(
                channel_index,
                SourceTargetV1::new(
                    SourceTargetKindV1::Node,
                    channel.target().node().index() as u64,
                ),
                property,
                components,
                SourceObservationV1::observed(interpolation, interpolation_provenance, disposition),
                disposition,
                channel_provenance,
            )
            .with_accessors(sampler.input().index(), sampler.output().index()),
        );
        self.remaining_text -= row_text_len;

        match times {
            Some(times) => {
                for &time in times {
                    self.saw_input = true;
                    if time.is_finite() {
                        self.minimum = self.minimum.min(f64::from(time));
                        self.maximum = self.maximum.max(f64::from(time));
                    } else {
                        self.sampler_inputs_finite = false;
                    }
                }
            }
            None => self.sampler_inputs_available = false,
        }
    }

    fn finish(self) -> Result<SourceClipFactV1, SourceFactsError> {
        let sampler_range = if self.truncated {
            SourceObservationV1::unavailable(
                SourceUnavailableReasonV1::ProjectionBudgetExceeded,
                Some(self.range_provenance),
                SourceLoaderDispositionV1::Preserved,
            )
        } else if !self.sampler_inputs_available {
            SourceObservationV1::unavailable(
                SourceUnavailableReasonV1::ParserUnavailable,
                Some(self.range_provenance),
                SourceLoaderDispositionV1::Unknown,
            )
        } else if !self.sampler_inputs_finite {
            SourceObservationV1::unavailable(
                SourceUnavailableReasonV1::Malformed,
                Some(self.range_provenance),
                SourceLoaderDispositionV1::Preserved,
            )
        } else if self.saw_input {
            SourceObservationV1::observed(
                SourceTimeRangeV1::new(self.minimum, self.maximum)?,
                self.range_provenance,
                SourceLoaderDispositionV1::Preserved,
            )
        } else {
            SourceObservationV1::proven_absent(self.range_provenance)
        };
        let channels = if self.truncated {
            SourceFactSetV1::partial(
                self.channels,
                SourceUnavailableReasonV1::ProjectionBudgetExceeded,
            )
        } else {
            SourceFactSetV1::complete(self.channels)
        };
        Ok(SourceClipFactV1::new(
            self.animation_index,
            self.source_name,
            SourceObservationV1::observed(
                self.animation_index,
                self.normalized_clip_provenance,
                SourceLoaderDispositionV1::Preserved,
            ),
            SourceObservationV1::proven_absent(SourceProvenanceV1::format_defined()),
            sampler_range,
            channels,
        ))
    }
}

fn decimal_len(mut value: usize) -> usize {
    let mut len = 1;
    while value >= 10 {
        value /= 10;
        len += 1;
    }
    len
}

fn build_document(
    gltf: &gltf::Gltf,
    buffers: &[Vec<u8>],
    path: &Path,
    topo: &Topology,
    source_facts: &mut RawSourceFactsBuilderV1,
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
        // panics iterating one. An accessor that is not `MAT4` of `FLOAT`
        // panics in that reader too; both are skipped here and recorded as
        // source evidence by `extract_source_skeleton`.
        if skin
            .inverse_bind_matrices()
            .is_none_or(|accessor| accessor.count() == 0 || !inverse_bind_is_readable(&accessor))
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
    let mut facts_complete = true;
    for animation in doc.animations() {
        let mut pending_facts = if facts_complete {
            PendingGltfClipFacts::begin(&animation, source_facts)
        } else {
            None
        };
        if pending_facts.is_none() {
            facts_complete = false;
        }
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
            // Nothing below this point may build a channel reader that has
            // not been judged first: `read_inputs` and `read_outputs` each
            // hand their own accessor to `Iter::new`, which panics on
            // arbitrary input in two independent ways.
            //
            // A count-0 accessor underflows in that iterator, and is
            // malformed animation rather than an unwalkable layout, so it
            // keeps its own message.
            let sampler = channel.sampler();
            let node = channel.target().node().index();
            if sampler.input().count() == 0 || sampler.output().count() == 0 {
                return Err(LoadError::Malformed(format!(
                    "clip '{name}' node {node}: animation channel with zero keyframes"
                )));
            }
            // The layout the accessor declares is the other way, and each
            // half of the reader has to be judged on its own accessor.
            check_sampler_accessor(&name, node, "input", &sampler.input())?;
            check_sampler_accessor(&name, node, "output", &sampler.output())?;
            let reader = channel.reader(|buffer| buffers.get(buffer.index()).map(Vec::as_slice));
            let times = reader.read_inputs().map(|it| it.collect::<Vec<f32>>());
            if let Some(pending) = pending_facts.as_mut()
                && !pending.truncated
            {
                pending.record_channel(&channel, times.as_deref());
            }
            let Some(times) = times else {
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
            validate_track_lengths(&name, node, interpolation, &times, &values)?;
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
        if let Some(pending) = pending_facts {
            let truncated = pending.truncated;
            if !source_facts.push_clip(pending.finish()?) || truncated {
                facts_complete = false;
            }
        }
    }
    if facts_complete {
        source_facts.mark_complete(SourceFactDomainV1::Clips);
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
    let mut scene_root_indices = vec![Vec::new(); doc.nodes().count()];
    for scene in doc.scenes() {
        for root in scene.nodes() {
            if let Some(indices) = scene_root_indices.get_mut(root.index()) {
                indices.push(scene.index());
            }
        }
    }
    for indices in &mut scene_root_indices {
        indices.sort_unstable();
        indices.dedup();
    }
    let mut attachments = vec![Vec::new(); doc.skins().count()];
    for node in doc.nodes() {
        let Some(skin) = node.skin() else {
            continue;
        };
        let Some(for_skin) = attachments.get_mut(skin.index()) else {
            return SourceSkeletonAssets::default();
        };
        for_skin.push(SourceSkinAttachment {
            source_node_index: node.index(),
            source_mesh_index: node.mesh().map(|mesh| mesh.index()),
        });
    }

    let mut nodes = Vec::with_capacity(doc.nodes().count());
    for node in doc.nodes() {
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
        let mut source_node = SourceNodeAsset::new(node.index(), local_rest);
        source_node.name = node.name().map(str::to_owned);
        source_node.parent_source_node_index = topo.parent[node.index()];
        source_node.scene_root_indices = std::mem::take(&mut scene_root_indices[node.index()]);
        source_node.bone = topo.bone_of_node[node.index()];
        nodes.push(source_node);
    }

    let mut skins = Vec::with_capacity(doc.skins().count());
    for skin in doc.skins() {
        let joints = skin.joints().map(|joint| joint.index()).collect::<Vec<_>>();
        let skeleton_root = skin.skeleton().map(|node| node.index());
        let inverse_bind_accessor = match skin.inverse_bind_matrices() {
            None => SourceInverseBindAccessor::default(),
            Some(accessor) if accessor.count() == 0 => SourceInverseBindAccessor {
                status: SourceInverseBindAccessorStatus::EmptyAccessor,
                declared_count: Some(0),
                matrices: Vec::new(),
            },
            // An accessor the matrix reader cannot decode is unreadable
            // evidence, not a load error; building the reader for it would
            // panic (see `inverse_bind_is_readable`).
            Some(accessor) if !inverse_bind_is_readable(&accessor) => SourceInverseBindAccessor {
                status: SourceInverseBindAccessorStatus::Unreadable,
                declared_count: Some(accessor.count()),
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
            skeleton_root_source_node_index: skeleton_root,
            joint_source_node_indices: joints,
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
    resources: &mut ResourceCaptureSession,
    bone_of_node: &[Option<usize>],
) -> SceneAssets {
    let mut assets = SceneAssets::default();

    let source_images = extract_source_images(doc, buffers, resources);
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
            material_texture(&raw_images, info.texture().source().index(), resources)
        });
        let normal_texture = material.normal_texture().and_then(|info| {
            material_texture(&raw_images, info.texture().source().index(), resources).map(
                |texture| NormalTextureAsset {
                    texture,
                    scale: info.scale(),
                },
            )
        });
        let metallic_roughness_texture = pbr.metallic_roughness_texture().and_then(|info| {
            material_texture(&raw_images, info.texture().source().index(), resources)
        });
        let occlusion_texture = material.occlusion_texture().and_then(|info| {
            material_texture(&raw_images, info.texture().source().index(), resources).map(
                |texture| OcclusionTextureAsset {
                    texture,
                    strength: info.strength(),
                },
            )
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
            // Every accessor read below has already had its `type`,
            // `componentType`, and buffer layout checked against what its
            // reader decodes; see `validate_primitive_accessors`, which
            // `load_bytes` runs before any reader exists. Adding a read here
            // for a semantic `required_attribute_encoding` answers `None` —
            // `read_tangents`, or any set index above 0 — compiles fine and
            // reopens the panic, so give it an encoding there first.
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
                source_primitive_index: Some(prim.index()),
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
                skin.inverse_bind_matrices().is_some_and(|accessor| {
                    accessor.count() > 0 && inverse_bind_is_readable(&accessor)
                })
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

fn material_texture(
    raw_images: &[Option<TextureAsset>],
    image_index: usize,
    resources: &mut ResourceCaptureSession,
) -> Option<TextureAsset> {
    let texture = raw_images.get(image_index)?.as_ref()?;
    resources.clone_image_for_material(image_index, texture)
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
    resources: &mut ResourceCaptureSession,
) -> Vec<LoadedSourceImage> {
    let writer_images = writer_image_indices(doc);
    doc.images()
        .map(|image| {
            let image_index = image.index();
            let retain_raw = writer_images.contains(&image_index);
            let name = image.name().map(str::to_owned);
            let (source_kind, declared_mime_type, raw, unavailable_reason, inspected) =
                match image.source() {
                    gltf::image::Source::View { view, mime_type } => {
                        let bytes = buffers.get(view.buffer().index()).and_then(|buffer| {
                            // A view with no `view_end` has no bytes here: an
                            // image is source evidence, so a failed range is an
                            // explicit source gap rather than a refusal.
                            view_end(&view).and_then(|end| buffer.get(view.offset()..end))
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
                            None,
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
                                None,
                            )
                        } else {
                            let (detected_container, leading_magic_hex, inspection) = {
                                let (bytes, reason) = resources.external_image_payload(image_index);
                                inspect_source_image(bytes, reason)
                            };
                            let raw = retain_raw.then(|| {
                                resources
                                    .materialize_external(
                                        SourceResourceKindV1::Image,
                                        image_index as u64,
                                    )
                                    .ok()
                                    .map(|bytes| TextureAsset {
                                        bytes,
                                        mime: mime_type.unwrap_or_default().to_owned(),
                                    })
                            });
                            let raw = raw.flatten();
                            let materialization_limited = retain_raw
                                && raw.is_none()
                                && resources.external_image_is_available(image_index);
                            (
                                ImageSourceKind::External,
                                mime_type.map(str::to_owned),
                                raw,
                                if materialization_limited {
                                    ImageUnavailableReason::ResourceLimit
                                } else {
                                    ImageUnavailableReason::SourceUnavailable
                                },
                                Some(if materialization_limited {
                                    (
                                        None,
                                        None,
                                        SourceImageInspection::Unavailable {
                                            reason: ImageUnavailableReason::ResourceLimit,
                                        },
                                    )
                                } else {
                                    (detected_container, leading_magic_hex, inspection)
                                }),
                            )
                        }
                    }
                };
            let (detected_container, leading_magic_hex, inspection) =
                inspected.unwrap_or_else(|| {
                    inspect_source_image(
                        raw.as_ref().map(|texture| texture.bytes.as_slice()),
                        unavailable_reason,
                    )
                });
            LoadedSourceImage {
                record: SourceImageAsset {
                    image_index,
                    name,
                    source_kind,
                    declared_mime_type,
                    detected_container,
                    leading_magic_hex,
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

/// Extract one material's bindings in fixed semantic order. This is called
/// from the single source-material walk that also keeps writer-facing slots.
fn source_material_texture_bindings(
    material: &gltf::Material<'_>,
) -> Vec<SourceMaterialTextureBinding> {
    let pbr = material.pbr_metallic_roughness();
    let mut texture_bindings = Vec::with_capacity(5);
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
    push(
        MaterialTextureSlot::Emissive,
        material.emissive_texture().map(|info| info.texture()),
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
    bytes: Option<&[u8]>,
    unavailable_reason: ImageUnavailableReason,
) -> (
    Option<ImageContainerFormat>,
    Option<String>,
    SourceImageInspection,
) {
    let Some(bytes) = bytes else {
        return (
            None,
            None,
            SourceImageInspection::Unavailable {
                reason: unavailable_reason,
            },
        );
    };
    if bytes.len() > MAX_IMAGE_ENCODED_BYTES {
        return (
            detect_container(bytes),
            None,
            SourceImageInspection::Unavailable {
                reason: ImageUnavailableReason::ResourceLimit,
            },
        );
    }
    let Some((format, detected_container)) = image_format(bytes) else {
        return (
            None,
            leading_magic_hex(bytes),
            SourceImageInspection::Unavailable {
                reason: ImageUnavailableReason::UnsupportedContainer,
            },
        );
    };
    let mut reader = ImageReader::new(Cursor::new(bytes));
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
                    None,
                    SourceImageInspection::Available {
                        width,
                        height,
                        channel_count: decoded.color().channel_count(),
                        color_type,
                    },
                ),
                None => (
                    Some(detected_container),
                    None,
                    SourceImageInspection::Unavailable {
                        reason: ImageUnavailableReason::DecodeFailed,
                    },
                ),
            }
        }
        Err(ImageError::Limits(_)) => (
            Some(detected_container),
            None,
            SourceImageInspection::Unavailable {
                reason: ImageUnavailableReason::ResourceLimit,
            },
        ),
        Err(_) => (
            Some(detected_container),
            None,
            SourceImageInspection::Unavailable {
                reason: ImageUnavailableReason::DecodeFailed,
            },
        ),
    }
}

/// Capture at most the first 16 bytes of an unsupported nonempty payload as
/// lowercase even-length hexadecimal evidence.
fn leading_magic_hex(bytes: &[u8]) -> Option<String> {
    if bytes.is_empty() {
        return None;
    }
    Some(
        bytes
            .iter()
            .take(16)
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    )
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

#[cfg(test)]
mod dependency_capture_tests {
    use super::*;

    fn key() -> DependencyResourceKeyV1 {
        DependencyResourceKeyV1::from_source_str("shared.bin", ResourceKeySyntaxV1::GltfUri)
            .expect("safe test key")
    }

    fn session_with_aliases(limit: u64) -> ResourceCaptureSession {
        let key = key();
        let mut session = ResourceCaptureSession::new(None);
        session.materialized_external_limit = limit;
        session
            .resources
            .insert(key.clone(), CapturedResource::Bytes(vec![1, 2]));
        for (kind, index) in [
            (SourceResourceKindV1::Buffer, 0),
            (SourceResourceKindV1::Buffer, 1),
            (SourceResourceKindV1::Image, 0),
            (SourceResourceKindV1::Image, 1),
        ] {
            session.insert_reference(kind, index, CapturedReference::External(key.clone()));
        }
        session
    }

    #[test]
    fn recording_reader_binds_one_capture_to_the_digest_and_document() {
        let dir = tempfile::tempdir().expect("temp dir");
        let external_path = dir.path().join("shared.bin");
        std::fs::write(&external_path, [0_u8; 36]).expect("decoy external bytes");

        let mut captured = Vec::new();
        for value in [0.0_f32, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 3.0, 0.0] {
            captured.extend_from_slice(&value.to_le_bytes());
        }
        let primary = serde_json::to_vec(&serde_json::json!({
            "asset": { "version": "2.0" },
            "buffers": [
                { "uri": "shared.bin", "byteLength": captured.len() },
                { "uri": "shared.bin", "byteLength": captured.len() }
            ],
            "bufferViews": [{ "buffer": 0, "byteOffset": 0, "byteLength": captured.len() }],
            "accessors": [{
                "bufferView": 0,
                "componentType": 5126,
                "count": 3,
                "type": "VEC3",
                "min": [0.0, 0.0, 0.0],
                "max": [2.0, 3.0, 0.0]
            }],
            "meshes": [{ "primitives": [{ "attributes": { "POSITION": 0 } }] }],
            "nodes": [{ "mesh": 0 }],
            "scenes": [{ "nodes": [0] }],
            "scene": 0
        }))
        .expect("analytic glTF JSON");
        let mut opens = 0;
        let loaded = load_source_bytes_inner_with_reader(
            &dir.path().join("recorded.gltf"),
            &primary,
            Some(dir.path()),
            |path, limit| {
                opens += 1;
                assert_eq!(path, external_path);
                assert!(limit >= captured.len() as u64);
                CapturedResource::Bytes(captured.clone())
            },
        )
        .expect("recorded external capture loads");

        assert_eq!(opens, 1, "two aliases cause one resolver open");
        let closure = loaded.dependency_closure();
        assert!(closure.coverage().is_complete());
        assert_eq!(closure.references().len(), 2);
        assert_eq!(closure.external_resources().len(), 1);
        assert_eq!(
            closure.external_resources()[0].identity(),
            &InputIdentity::from_bytes(&captured),
            "the closure hashes the resolver-returned capture"
        );
        assert_eq!(
            loaded.document().assets.meshes[0].primitives[0].positions,
            [
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(2.0, 0.0, 0.0),
                Vec3::new(0.0, 3.0, 0.0),
            ],
            "the Document consumes the recorded capture, not the on-disk decoy"
        );
    }

    #[test]
    fn raw_extension_key_scan_handles_nesting_escaping_and_non_key_text() {
        assert!(json_has_object_key(
            br#"{"meshes":[{"primitives":[{"extensio\u006es":{"X":{}}}]}]}"#,
            b"extensions"
        ));
        assert!(!json_has_object_key(
            br#"{"extras":{"label":"extensions","note":"\"extensions\":"}}"#,
            b"extensions"
        ));
    }

    #[test]
    fn materialization_cap_refuses_essential_buffer_alias_without_leaking_a_path() {
        let gltf = gltf::Gltf::from_slice(
            br#"{
                "asset":{"version":"2.0"},
                "buffers":[
                    {"uri":"shared.bin","byteLength":2},
                    {"uri":"shared.bin","byteLength":2}
                ]
            }"#,
        )
        .expect("test glTF");
        let mut session = session_with_aliases(3);
        let error = resolve_captured_buffers(&gltf, &mut session)
            .expect_err("second essential clone exceeds the internal cap");
        assert!(
            error
                .to_string()
                .contains("external buffer resource exceeds capture limits"),
            "{error}"
        );
    }

    #[test]
    fn materialization_cap_omits_optional_image_alias_while_its_capture_stays_reusable() {
        let gltf = gltf::Gltf::from_slice(
            br#"{
                "asset":{"version":"2.0"},
                "images":[
                    {"uri":"shared.bin"},
                    {"uri":"shared.bin"}
                ],
                "textures":[{"source":0},{"source":1}],
                "materials":[
                    {"pbrMetallicRoughness":{"baseColorTexture":{"index":0}}},
                    {"pbrMetallicRoughness":{"baseColorTexture":{"index":1}}}
                ]
            }"#,
        )
        .expect("test glTF");
        let mut session = session_with_aliases(3);
        let images = extract_source_images(&gltf.document, &[], &mut session);
        assert!(images[0].texture.is_some());
        assert!(images[1].texture.is_none());
        assert!(matches!(
            images[1].record.inspection,
            SourceImageInspection::Unavailable {
                reason: ImageUnavailableReason::ResourceLimit
            }
        ));
        assert!(session.external_image_is_available(1));
    }
}
