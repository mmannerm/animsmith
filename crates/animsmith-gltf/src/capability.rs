//! Read-only raw glTF capability inventory for future scale producers.
//!
//! A normalized [`animsmith_core::Document`] cannot prove that a source file
//! lacked data the loader does not model. This module therefore inventories
//! the original glTF JSON and resolved buffers before any scale plan or
//! candidate document exists.

use crate::{
    LoadError, load_bytes, resolve_buffers, topology, validate_animation_channels,
    validate_glb_framing,
};
use animsmith_core::Document;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

const GLB_MAGIC: &[u8; 4] = b"glTF";
const GLB_JSON_CHUNK: u32 = 0x4e4f_534a;

/// Whether the captured top-level source is JSON glTF or a binary GLB.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GltfContainerKind {
    /// A plain JSON `.gltf` document.
    Gltf,
    /// A binary `.glb` container.
    Glb,
}

/// How one source buffer was declared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GltfBufferSourceKind {
    /// The GLB BIN chunk.
    BinaryChunk,
    /// A base64 data URI.
    DataUri,
    /// An external relative URI.
    External,
}

/// One source buffer recorded before normalized loading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GltfBufferCapability {
    /// Stable source buffer index.
    pub buffer_index: usize,
    /// Source declaration kind.
    pub source_kind: GltfBufferSourceKind,
    /// Declared byte length.
    pub declared_byte_length: u64,
}

/// Whether a node authored decomposed TRS or a matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GltfNodeRestKind {
    /// No matrix was declared, so the node uses glTF TRS properties/defaults.
    Trs,
    /// The node declared a local matrix.
    Matrix,
}

/// One source node identity and authored rest representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GltfNodeCapability {
    /// Stable source node index.
    pub node_index: usize,
    /// Authored rest representation.
    pub rest_kind: GltfNodeRestKind,
    /// Referenced mesh index, when present.
    pub mesh_index: Option<usize>,
    /// Referenced skin index, when present.
    pub skin_index: Option<usize>,
}

/// One animation channel and its exact accessor identities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GltfAnimationChannelCapability {
    /// Stable source animation index.
    pub animation_index: usize,
    /// Channel index inside the animation.
    pub channel_index: usize,
    /// Source target node index.
    pub target_node_index: usize,
    /// glTF target path (`translation`, `rotation`, `scale`, or `weights`).
    pub target_path: String,
    /// glTF interpolation spelling.
    pub interpolation: String,
    /// Input time accessor index.
    pub input_accessor_index: usize,
    /// Output value accessor index.
    pub output_accessor_index: usize,
}

/// One vertex attribute declaration and its source accessor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GltfAttributeCapability {
    /// glTF attribute semantic such as `POSITION` or `JOINTS_0`.
    pub semantic: String,
    /// Stable source accessor index.
    pub accessor_index: usize,
}

/// One source primitive and every declared attribute semantic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GltfPrimitiveCapability {
    /// Stable source mesh index.
    pub mesh_index: usize,
    /// Primitive index inside the mesh.
    pub primitive_index: usize,
    /// Raw glTF primitive mode value (default `4`, triangles).
    pub mode: u64,
    /// Attributes in lexical semantic order with exact accessor identities.
    pub attributes: Vec<GltfAttributeCapability>,
    /// Number of declared morph targets.
    pub morph_target_count: usize,
    /// `POSITION` accessor indices from morph targets in target order.
    pub morph_position_accessors: Vec<usize>,
}

/// One raw `EXT_mesh_gpu_instancing` declaration and its accessor identities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GltfInstancingCapability {
    /// Stable source node index carrying the instancing payload.
    pub node_index: usize,
    /// Instancing attributes in lexical semantic order.
    pub attributes: Vec<GltfAttributeCapability>,
}

/// One raw accessor layout required by a future exact-source rewrite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GltfAccessorCapability {
    /// Stable source accessor index.
    pub accessor_index: usize,
    /// Referenced buffer-view index, when present.
    pub buffer_view_index: Option<usize>,
    /// Byte offset relative to the buffer view.
    pub byte_offset: u64,
    /// Raw glTF component-type value.
    pub component_type: u64,
    /// Raw glTF accessor type such as `VEC3` or `MAT4`.
    pub accessor_type: String,
    /// Declared element count.
    pub count: u64,
    /// Whether normalized integer interpretation was requested.
    pub normalized: bool,
    /// Whether the accessor declares sparse replacement data.
    pub sparse: bool,
}

/// One raw buffer-view layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GltfBufferViewCapability {
    /// Stable source buffer-view index.
    pub buffer_view_index: usize,
    /// Stable source buffer index.
    pub buffer_index: usize,
    /// Byte offset relative to the buffer.
    pub byte_offset: u64,
    /// Declared byte length.
    pub byte_length: u64,
    /// Optional element stride.
    pub byte_stride: Option<u64>,
}

/// Read-side inverse-bind declaration for one source skin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GltfSkinCapability {
    /// Stable source skin index.
    pub skin_index: usize,
    /// Number of declared joints.
    pub joint_count: usize,
    /// Declared inverse-bind accessor index, when present.
    pub inverse_bind_accessor_index: Option<usize>,
    /// Declared inverse-bind accessor count, when readable from raw JSON.
    pub inverse_bind_count: Option<u64>,
}

/// Deterministic facts captured from the original glTF/GLB source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GltfCapabilityManifest {
    /// Top-level container kind.
    pub container: GltfContainerKind,
    /// Source buffers in source order.
    pub buffers: Vec<GltfBufferCapability>,
    /// Source buffer views in source order.
    pub buffer_views: Vec<GltfBufferViewCapability>,
    /// Source accessors in source order.
    pub accessors: Vec<GltfAccessorCapability>,
    /// Source nodes in source order.
    pub nodes: Vec<GltfNodeCapability>,
    /// Animation channels in animation/channel order.
    pub animation_channels: Vec<GltfAnimationChannelCapability>,
    /// Mesh primitives in mesh/primitive order.
    pub primitives: Vec<GltfPrimitiveCapability>,
    /// GPU-instancing declarations in source node order.
    pub instancing: Vec<GltfInstancingCapability>,
    /// Source skins in source order.
    pub skins: Vec<GltfSkinCapability>,
    /// Number of declared cameras.
    pub camera_count: usize,
    /// Declared extension names in lexical order.
    pub extensions: Vec<String>,
    /// JSON pointers of extension payloads in lexical order.
    pub extension_locations: Vec<String>,
    /// JSON pointers of external buffer/image declarations in lexical order.
    pub external_resource_locations: Vec<String>,
    /// JSON pointers of every non-null `extras` value in lexical order.
    pub extras_locations: Vec<String>,
    /// JSON pointers of unknown members in lexical order.
    pub unknown_member_locations: Vec<String>,
}

/// Stable machine identity for one fail-closed capability violation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum GltfCapabilityViolationKind {
    /// A source buffer or image uses an external URI.
    ExternalResource,
    /// A morph target is present.
    MorphTarget,
    /// Static or animated morph weights are present.
    MorphWeights,
    /// A camera definition or reference is present.
    Camera,
    /// A punctual-light declaration or payload is present.
    Light,
    /// An `EXT_mesh_gpu_instancing` declaration or payload is present.
    Instancing,
    /// An extension declaration is not covered by a registered handler.
    ExtensionDeclaration,
    /// An extension payload is not covered by a registered handler.
    ExtensionPayload,
    /// Non-null application-specific extras are present.
    Extras,
    /// A JSON member outside the glTF 2.0 schema was ignored by the typed parser.
    UnknownJsonMember,
    /// A primitive mode other than triangle lists is present.
    NonTrianglePrimitive,
    /// A vertex attribute is outside the normalized writer subset.
    UnsupportedVertexAttribute,
    /// A secondary `JOINTS_n` or `WEIGHTS_n` set is present.
    SecondarySkinInfluences,
    /// A skin omitted its inverse-bind accessor.
    MissingInverseBinds,
    /// A skin declared an empty inverse-bind accessor.
    EmptyInverseBindAccessor,
    /// A skin's inverse-bind count does not equal its joint count.
    InverseBindCountMismatch,
    /// A declared inverse-bind accessor is not a dense f32 MAT4 source.
    UnreadableInverseBinds,
    /// A used accessor cannot be safely bounded, or a rewrite accessor is not dense f32.
    UnsafeAccessorLayout,
    /// One accessor is shared between scale-bearing and dimensionless semantics.
    ConflictingAccessorUse,
    /// A scale-bearing accessor overlaps another used byte range in a source
    /// buffer. The other range is an accessor, or an `image` payload reported
    /// alongside it as [`GltfCapabilityViolationKind::ImagePayloadOverlap`].
    OverlappingAccessorRanges,
    /// A node declares `matrix` alongside `translation`, `rotation` or
    /// `scale`, which glTF 2.0 §3.5 forbids.
    ConflictingNodeTransform,
    /// A node `matrix` is not TRS-decomposable: its last row is not
    /// `(0, 0, 0, 1)`.
    NonAffineNodeMatrix,
    /// An `image` reads a buffer view overlapping a scale-bearing accessor.
    ImagePayloadOverlap,
}

/// One deterministic, source-indexed preflight rejection.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct GltfCapabilityViolation {
    /// JSON pointer or stable source identity for the rejected domain.
    pub location: String,
    /// Stable violation kind.
    pub kind: GltfCapabilityViolationKind,
}

/// A captured, immutable source that passed the common scale preflight.
///
/// This type deliberately has no mutation or write method. Later operation
/// slices consume its manifest and captured bytes without reopening the input.
#[derive(Debug)]
pub struct GltfScaleSource {
    document: Document,
    manifest: GltfCapabilityManifest,
    source_bytes: Vec<u8>,
    raw_json: Value,
    resolved_buffers: Vec<Vec<u8>>,
}

impl GltfScaleSource {
    /// The normalized read-only document built from the captured bytes.
    pub fn document(&self) -> &Document {
        &self.document
    }

    /// The deterministic raw capability manifest.
    pub fn manifest(&self) -> &GltfCapabilityManifest {
        &self.manifest
    }

    /// The exact captured top-level input bytes.
    pub fn source_bytes(&self) -> &[u8] {
        &self.source_bytes
    }

    /// The original top-level JSON tree.
    pub fn raw_json(&self) -> &Value {
        &self.raw_json
    }

    /// Resolved source buffers in buffer-index order.
    pub fn resolved_buffers(&self) -> &[Vec<u8>] {
        &self.resolved_buffers
    }
}

/// Failure to load or safely preflight a captured scale source.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum GltfScalePreflightError {
    /// The source was malformed or unreadable.
    #[error(transparent)]
    Load(#[from] LoadError),
    /// The source was parseable but contains unsupported raw domains.
    #[error("glTF scale preflight rejected {count} unsupported source domain(s)")]
    Unsupported {
        /// Complete inventory gathered before rejection.
        manifest: Box<GltfCapabilityManifest>,
        /// Deterministically ordered typed violations.
        violations: Vec<GltfCapabilityViolation>,
        /// Number of violations, repeated for stable error rendering.
        count: usize,
    },
}

/// Read and preflight a glTF/GLB file without creating a candidate or output.
///
/// # Errors
///
/// Returns [`GltfScalePreflightError::Load`] for unreadable or malformed input
/// and [`GltfScalePreflightError::Unsupported`] for a parseable source whose
/// complete raw domain is not covered by the initial scale boundary.
pub fn preflight_scale_source(path: &Path) -> Result<GltfScaleSource, GltfScalePreflightError> {
    let bytes = std::fs::read(path).map_err(|source| LoadError::Io {
        path: path.display().to_string(),
        source,
    })?;
    preflight_scale_source_bytes(path, &bytes)
}

/// Preflight captured glTF/GLB bytes without creating a candidate or output.
///
/// `path` is used only for source provenance and resolving resources. The
/// initial accepted subset rejects external resources before resolving them,
/// so a successful value is fully captured in memory.
///
/// # Errors
///
/// Returns [`GltfScalePreflightError::Load`] for malformed input and
/// [`GltfScalePreflightError::Unsupported`] for unsupported raw domains.
pub fn preflight_scale_source_bytes(
    path: &Path,
    bytes: &[u8],
) -> Result<GltfScaleSource, GltfScalePreflightError> {
    capture_scale_source(path, bytes, GatePolicy::Enforce)
}

/// Whether a captured source must clear the preflight's violation gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GatePolicy {
    /// A source with any violation is refused. The only policy in a
    /// non-test build.
    Enforce,
    /// Violations are inventoried and then ignored, so a
    /// [`GltfScaleSource`] is built for a source the gate would refuse.
    ///
    /// Every operation below the gate keeps its own guard for the source
    /// facts the gate decides — [`crate::scale::rewrite_linear_units`]
    /// re-checks out-of-contract node transforms and image payloads aliasing
    /// a converted accessor. Those guards are what must hold if the gate is
    /// ever relaxed, which is exactly the property no test can observe while
    /// the gate refuses every source that would reach them: deleting the
    /// guard's call site leaves the public API's behaviour unchanged. This
    /// policy is the synthetic relaxation those tests need, and it exists
    /// only under `cfg(test)` so no release path can select it.
    #[cfg(test)]
    Bypass,
}

/// Capture a scale source, applying `policy` to the preflight's violations.
fn capture_scale_source(
    path: &Path,
    bytes: &[u8],
    policy: GatePolicy,
) -> Result<GltfScaleSource, GltfScalePreflightError> {
    validate_glb_framing(bytes)?;
    let (container, json_bytes) = raw_json_bytes(bytes)?;
    let raw_json: Value = serde_json::from_slice(json_bytes)
        .map_err(|error| LoadError::Malformed(format!("invalid top-level JSON: {error}")))?;
    if !raw_json.is_object() {
        return Err(LoadError::Malformed("top-level glTF JSON is not an object".into()).into());
    }
    let gltf = gltf::Gltf::from_slice_without_validation(bytes).map_err(LoadError::Gltf)?;

    let mut violations = Vec::new();
    let manifest = inventory(&raw_json, container, &mut violations);
    let accessor_uses = inspect_accessor_uses(&raw_json, &mut violations);
    match validate_document(&gltf.document) {
        Ok(()) => {}
        Err(error) => return Err(LoadError::Gltf(error).into()),
    }
    validate_animation_channels(gltf.document.as_json())?;
    topology(&gltf.document)?;

    let can_resolve_buffers = !manifest
        .buffers
        .iter()
        .any(|buffer| buffer.source_kind == GltfBufferSourceKind::External);
    let resolved_buffers = if can_resolve_buffers {
        resolve_buffers(&gltf, path.parent())?
    } else {
        Vec::new()
    };
    if can_resolve_buffers {
        inspect_accessor_layouts(
            &raw_json,
            &resolved_buffers,
            &accessor_uses,
            &mut violations,
        );
    }
    violations.sort();
    violations.dedup();
    let refuse = match policy {
        GatePolicy::Enforce => !violations.is_empty(),
        #[cfg(test)]
        GatePolicy::Bypass => false,
    };
    if refuse {
        let count = violations.len();
        return Err(GltfScalePreflightError::Unsupported {
            manifest: Box::new(manifest),
            violations,
            count,
        });
    }

    let document = load_bytes(path, bytes)?;
    Ok(GltfScaleSource {
        document,
        manifest,
        source_bytes: bytes.to_vec(),
        raw_json,
        resolved_buffers,
    })
}

/// Capture a [`GltfScaleSource`] from bytes the preflight gate would refuse.
///
/// See [`GatePolicy::Bypass`] for why this exists. It is not a public API and
/// not reachable from an integration test: the gate is the only way to build a
/// [`GltfScaleSource`] outside this crate, and that stays true.
///
/// # Errors
///
/// Returns [`GltfScalePreflightError::Load`] for input that is malformed
/// rather than merely out of contract. `Unsupported` is never returned.
#[cfg(test)]
pub(crate) fn scale_source_past_the_gate(
    path: &Path,
    bytes: &[u8],
) -> Result<GltfScaleSource, GltfScalePreflightError> {
    capture_scale_source(path, bytes, GatePolicy::Bypass)
}

fn validate_document(document: &gltf::Document) -> Result<(), gltf::Error> {
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

/// Split a captured container into its kind and its top-level JSON bytes.
///
/// Shared with [`crate::scale`], whose artifact proof must re-read the
/// emitted container through exactly the same framing the preflight used.
pub(crate) fn raw_json_bytes(bytes: &[u8]) -> Result<(GltfContainerKind, &[u8]), LoadError> {
    if !bytes.starts_with(GLB_MAGIC) {
        return Ok((GltfContainerKind::Gltf, bytes));
    }
    let chunk_length = bytes
        .get(12..16)
        .and_then(|slice| slice.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| LoadError::Buffer("malformed GLB JSON chunk header".into()))?
        as usize;
    let chunk_type = bytes
        .get(16..20)
        .and_then(|slice| slice.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| LoadError::Buffer("malformed GLB JSON chunk header".into()))?;
    if chunk_type != GLB_JSON_CHUNK {
        return Err(LoadError::Buffer(
            "GLB first chunk is not a JSON chunk".into(),
        ));
    }
    let end = 20usize
        .checked_add(chunk_length)
        .ok_or_else(|| LoadError::Buffer("GLB JSON chunk range overflow".into()))?;
    let json = bytes
        .get(20..end)
        .ok_or_else(|| LoadError::Buffer("malformed GLB JSON chunk length".into()))?;
    Ok((GltfContainerKind::Glb, json))
}

fn violation(
    violations: &mut Vec<GltfCapabilityViolation>,
    kind: GltfCapabilityViolationKind,
    location: impl Into<String>,
) {
    violations.push(GltfCapabilityViolation {
        kind,
        location: location.into(),
    });
}

fn as_index(value: Option<&Value>) -> Option<usize> {
    value?.as_u64()?.try_into().ok()
}

fn inventory(
    root: &Value,
    container: GltfContainerKind,
    violations: &mut Vec<GltfCapabilityViolation>,
) -> GltfCapabilityManifest {
    let Some(object) = root.as_object() else {
        return GltfCapabilityManifest {
            container,
            buffers: Vec::new(),
            buffer_views: Vec::new(),
            accessors: Vec::new(),
            nodes: Vec::new(),
            animation_channels: Vec::new(),
            primitives: Vec::new(),
            instancing: Vec::new(),
            skins: Vec::new(),
            camera_count: 0,
            extensions: Vec::new(),
            extension_locations: Vec::new(),
            external_resource_locations: Vec::new(),
            extras_locations: Vec::new(),
            unknown_member_locations: Vec::new(),
        };
    };
    let mut manifest = GltfCapabilityManifest {
        container,
        buffers: Vec::new(),
        buffer_views: Vec::new(),
        accessors: Vec::new(),
        nodes: Vec::new(),
        animation_channels: Vec::new(),
        primitives: Vec::new(),
        instancing: Vec::new(),
        skins: Vec::new(),
        camera_count: object
            .get("cameras")
            .and_then(Value::as_array)
            .map_or(0, Vec::len),
        extensions: Vec::new(),
        extension_locations: Vec::new(),
        external_resource_locations: Vec::new(),
        extras_locations: Vec::new(),
        unknown_member_locations: Vec::new(),
    };

    inspect_schema_members(root, "", &mut manifest, violations);
    inventory_extensions(object, &mut manifest, violations);
    inventory_buffers(object, container, &mut manifest, violations);
    inventory_buffer_views_and_accessors(object, &mut manifest);
    inventory_nodes(object, &mut manifest, violations);
    inventory_animations(object, &mut manifest, violations);
    inventory_meshes(object, &mut manifest, violations);
    inventory_skins(object, &mut manifest, violations);

    if manifest.camera_count > 0 {
        violation(violations, GltfCapabilityViolationKind::Camera, "/cameras");
    }
    manifest.extensions.sort();
    manifest.extensions.dedup();
    manifest.extension_locations.sort();
    manifest.extension_locations.dedup();
    manifest.external_resource_locations.sort();
    manifest.external_resource_locations.dedup();
    manifest.extras_locations.sort();
    manifest.extras_locations.dedup();
    manifest.unknown_member_locations.sort();
    manifest.unknown_member_locations.dedup();
    manifest
}

fn inventory_extensions(
    root: &Map<String, Value>,
    manifest: &mut GltfCapabilityManifest,
    violations: &mut Vec<GltfCapabilityViolation>,
) {
    for key in ["extensionsUsed", "extensionsRequired"] {
        let Some(values) = root.get(key).and_then(Value::as_array) else {
            continue;
        };
        for (index, value) in values.iter().enumerate() {
            let Some(name) = value.as_str() else { continue };
            manifest.extensions.push(name.to_owned());
            let kind = match name {
                "KHR_lights_punctual" => GltfCapabilityViolationKind::Light,
                "EXT_mesh_gpu_instancing" => GltfCapabilityViolationKind::Instancing,
                _ => GltfCapabilityViolationKind::ExtensionDeclaration,
            };
            violation(violations, kind, format!("/{key}/{index}"));
        }
    }
}

fn inventory_buffers(
    root: &Map<String, Value>,
    container: GltfContainerKind,
    manifest: &mut GltfCapabilityManifest,
    violations: &mut Vec<GltfCapabilityViolation>,
) {
    let Some(buffers) = root.get("buffers").and_then(Value::as_array) else {
        return;
    };
    for (buffer_index, buffer) in buffers.iter().enumerate() {
        let Some(buffer) = buffer.as_object() else {
            continue;
        };
        let uri = buffer.get("uri").and_then(Value::as_str);
        let source_kind = match uri {
            Some(uri) if uri.starts_with("data:") => GltfBufferSourceKind::DataUri,
            Some(_) => GltfBufferSourceKind::External,
            None if container == GltfContainerKind::Glb => GltfBufferSourceKind::BinaryChunk,
            None => GltfBufferSourceKind::External,
        };
        if source_kind == GltfBufferSourceKind::External {
            manifest
                .external_resource_locations
                .push(format!("/buffers/{buffer_index}/uri"));
            violation(
                violations,
                GltfCapabilityViolationKind::ExternalResource,
                format!("/buffers/{buffer_index}/uri"),
            );
        }
        manifest.buffers.push(GltfBufferCapability {
            buffer_index,
            source_kind,
            declared_byte_length: buffer
                .get("byteLength")
                .and_then(Value::as_u64)
                .unwrap_or(0),
        });
    }
    if let Some(images) = root.get("images").and_then(Value::as_array) {
        for (image_index, image) in images.iter().enumerate() {
            if image
                .get("uri")
                .and_then(Value::as_str)
                .is_some_and(|uri| !uri.starts_with("data:"))
            {
                manifest
                    .external_resource_locations
                    .push(format!("/images/{image_index}/uri"));
                violation(
                    violations,
                    GltfCapabilityViolationKind::ExternalResource,
                    format!("/images/{image_index}/uri"),
                );
            }
        }
    }
}

fn inventory_buffer_views_and_accessors(
    root: &Map<String, Value>,
    manifest: &mut GltfCapabilityManifest,
) {
    if let Some(buffer_views) = root.get("bufferViews").and_then(Value::as_array) {
        for (buffer_view_index, view) in buffer_views.iter().enumerate() {
            let Some(view) = view.as_object() else {
                continue;
            };
            manifest.buffer_views.push(GltfBufferViewCapability {
                buffer_view_index,
                buffer_index: as_index(view.get("buffer")).unwrap_or(usize::MAX),
                byte_offset: view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0),
                byte_length: view.get("byteLength").and_then(Value::as_u64).unwrap_or(0),
                byte_stride: view.get("byteStride").and_then(Value::as_u64),
            });
        }
    }
    if let Some(accessors) = root.get("accessors").and_then(Value::as_array) {
        for (accessor_index, accessor) in accessors.iter().enumerate() {
            let Some(accessor) = accessor.as_object() else {
                continue;
            };
            manifest.accessors.push(GltfAccessorCapability {
                accessor_index,
                buffer_view_index: as_index(accessor.get("bufferView")),
                byte_offset: accessor
                    .get("byteOffset")
                    .and_then(Value::as_u64)
                    .unwrap_or(0),
                component_type: accessor
                    .get("componentType")
                    .and_then(Value::as_u64)
                    .unwrap_or(0),
                accessor_type: accessor
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                count: accessor.get("count").and_then(Value::as_u64).unwrap_or(0),
                normalized: accessor
                    .get("normalized")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                sparse: accessor.contains_key("sparse"),
            });
        }
    }
}

/// The last row of a column-major glTF node `matrix`, and the only values
/// glTF 2.0 permits there.
///
/// Shared with [`crate::scale`], whose rewriter keeps its own guard as
/// defence in depth: re-deriving the row there would let two definitions of
/// "affine" drift apart, which is exactly how this workspace's two affine
/// classifiers once came to disagree.
pub(crate) const AFFINE_LAST_ROW: [(usize, f64); 4] = [(3, 0.0), (7, 0.0), (11, 0.0), (15, 1.0)];

/// One way a source node's transform is outside the glTF 2.0 contract.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum NodeTransformFault {
    /// A TRS member is declared alongside `matrix`.
    TrsBesideMatrix {
        /// Stable source node index.
        node_index: usize,
        /// The offending member's glTF spelling.
        member: &'static str,
    },
    /// A last-row `matrix` entry is a number other than the affine one.
    ProjectiveMatrixEntry {
        /// Stable source node index.
        node_index: usize,
        /// Component index inside the column-major `matrix`.
        component: usize,
        /// The authored value.
        value: f64,
        /// The only value glTF 2.0 permits there.
        expected: f64,
    },
    /// A last-row `matrix` entry is not a JSON number at all, so it cannot be
    /// shown to be the affine value.
    UnreadableMatrixEntry {
        /// Stable source node index.
        node_index: usize,
        /// Component index inside the column-major `matrix`.
        component: usize,
    },
}

impl NodeTransformFault {
    /// JSON pointer of the offending member or `matrix` entry.
    pub(crate) fn location(self) -> String {
        match self {
            Self::TrsBesideMatrix { node_index, member } => format!("/nodes/{node_index}/{member}"),
            Self::ProjectiveMatrixEntry {
                node_index,
                component,
                ..
            }
            | Self::UnreadableMatrixEntry {
                node_index,
                component,
            } => format!("/nodes/{node_index}/matrix/{component}"),
        }
    }

    /// The preflight violation kind this fault is reported as.
    fn kind(self) -> GltfCapabilityViolationKind {
        match self {
            Self::TrsBesideMatrix { .. } => GltfCapabilityViolationKind::ConflictingNodeTransform,
            // An entry that is not a readable number is not the affine value
            // either, so it fails closed as the same kind.
            Self::ProjectiveMatrixEntry { .. } | Self::UnreadableMatrixEntry { .. } => {
                GltfCapabilityViolationKind::NonAffineNodeMatrix
            }
        }
    }
}

/// The value `object` declares for `member`, treating an explicit JSON `null`
/// as no declaration at all.
///
/// `serde_json` reports `"matrix": null` as `Some(Value::Null)`, while the
/// typed glTF parse deserializes the same member into `Option<[f32; 16]>` as
/// `None`. A raw-JSON walker asking only whether the key is *present*
/// therefore disagrees with the typed parse about what the node declared: it
/// reads `{"matrix": null, "translation": [...]}` as a node declaring both,
/// and refuses a document the typed parse reads as a plain TRS node — naming
/// the innocent `translation` as the offender.
///
/// Every walker deciding whether a node authored a transform goes through
/// here, so the gate, the rewriter's guard and [`crate::scale`]'s rewrite
/// selection cannot disagree about it. Presence checks whose only outcome is
/// a fail-closed refusal — `/nodes/*/camera` and `/nodes/*/weights` — are
/// deliberately left key-based: over-refusing a `null` there costs a source
/// nothing that could have converted, while over-refusing a transform member
/// costs a source that converts correctly.
pub(crate) fn declared<'a>(object: &'a Value, member: &str) -> Option<&'a Value> {
    object.get(member).filter(|value| !value.is_null())
}

/// Every glTF 2.0 node-transform contract violation in `nodes`, in node order
/// and, within a node, TRS members before `matrix` entries.
///
/// The `gltf` crate parses both shapes, so neither is refused by the typed
/// parse and neither is a wrong answer on schema-valid input:
///
/// * A node declaring `matrix` **and** a TRS member. glTF 2.0 §3.5 makes the
///   two mutually exclusive, and the typed parse silently honours `matrix`
///   while ignoring the TRS members, so a consumer cannot know which the
///   author meant.
/// * A node `matrix` whose last row is not `(0, 0, 0, 1)`. glTF 2.0 requires
///   `matrix` to be decomposable to translation, rotation and scale. The
///   whole-document conversion's `M' = U M U^-1` identity leaves entries 3, 7,
///   11 and 15 alone, which is only correct when they are the affine row: a
///   projective row transforms as `1/q`, so treating it as invariant would
///   emit a matrix that is not the converted transform.
///
/// A `matrix` of the wrong arity fails the typed glTF parse — which
/// deserializes it as `[f32; 16]` — before either caller runs, so shape
/// errors keep their existing owner rather than gaining a second report here.
///
/// A member authored as JSON `null` is not a declaration: see [`declared`].
pub(crate) fn node_transform_faults(nodes: &[Value]) -> Vec<NodeTransformFault> {
    let mut faults = Vec::new();
    for (node_index, node) in nodes.iter().enumerate() {
        let Some(matrix) = declared(node, "matrix") else {
            continue;
        };
        for member in ["translation", "rotation", "scale"] {
            if declared(node, member).is_some() {
                faults.push(NodeTransformFault::TrsBesideMatrix { node_index, member });
            }
        }
        let Some(values) = matrix.as_array().filter(|values| values.len() == 16) else {
            continue;
        };
        for (component, expected) in AFFINE_LAST_ROW {
            match values[component].as_f64() {
                None => faults.push(NodeTransformFault::UnreadableMatrixEntry {
                    node_index,
                    component,
                }),
                Some(value) if value != expected => {
                    faults.push(NodeTransformFault::ProjectiveMatrixEntry {
                        node_index,
                        component,
                        value,
                        expected,
                    });
                }
                Some(_) => {}
            }
        }
    }
    faults
}

fn inventory_nodes(
    root: &Map<String, Value>,
    manifest: &mut GltfCapabilityManifest,
    violations: &mut Vec<GltfCapabilityViolation>,
) {
    let Some(nodes) = root.get("nodes").and_then(Value::as_array) else {
        return;
    };
    for fault in node_transform_faults(nodes) {
        violation(violations, fault.kind(), fault.location());
    }
    for (node_index, node) in nodes.iter().enumerate() {
        if !node.is_object() {
            continue;
        }
        // `weights` and `camera` stay key-based: a `null` there is a domain
        // the conversion cannot preserve either way, so refusing it is the
        // safe direction. `matrix` below cannot, because there a false
        // positive refuses a source that converts correctly.
        if node.get("weights").is_some() {
            violation(
                violations,
                GltfCapabilityViolationKind::MorphWeights,
                format!("/nodes/{node_index}/weights"),
            );
        }
        if node.get("camera").is_some() {
            violation(
                violations,
                GltfCapabilityViolationKind::Camera,
                format!("/nodes/{node_index}/camera"),
            );
        }
        if let Some(attributes) = node
            .get("extensions")
            .and_then(|extensions| extensions.get("EXT_mesh_gpu_instancing"))
            .and_then(|extension| extension.get("attributes"))
            .and_then(Value::as_object)
        {
            let mut attributes = attributes
                .iter()
                .map(|(semantic, accessor)| GltfAttributeCapability {
                    semantic: semantic.clone(),
                    accessor_index: as_index(Some(accessor)).unwrap_or(usize::MAX),
                })
                .collect::<Vec<_>>();
            attributes.sort_by(|left, right| left.semantic.cmp(&right.semantic));
            manifest.instancing.push(GltfInstancingCapability {
                node_index,
                attributes,
            });
        }
        manifest.nodes.push(GltfNodeCapability {
            node_index,
            // A key-based check would report a `"matrix": null` node as
            // `Matrix` while the typed parse reads it as `Trs`.
            rest_kind: if declared(node, "matrix").is_some() {
                GltfNodeRestKind::Matrix
            } else {
                GltfNodeRestKind::Trs
            },
            mesh_index: as_index(node.get("mesh")),
            skin_index: as_index(node.get("skin")),
        });
    }
}

fn inventory_animations(
    root: &Map<String, Value>,
    manifest: &mut GltfCapabilityManifest,
    violations: &mut Vec<GltfCapabilityViolation>,
) {
    let Some(animations) = root.get("animations").and_then(Value::as_array) else {
        return;
    };
    for (animation_index, animation) in animations.iter().enumerate() {
        let Some(animation) = animation.as_object() else {
            continue;
        };
        let samplers = animation
            .get("samplers")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default();
        let channels = animation
            .get("channels")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default();
        for (channel_index, channel) in channels.iter().enumerate() {
            let Some(channel) = channel.as_object() else {
                continue;
            };
            let sampler_index = as_index(channel.get("sampler")).unwrap_or(usize::MAX);
            let Some(sampler) = samplers.get(sampler_index).and_then(Value::as_object) else {
                continue;
            };
            let Some(target) = channel.get("target").and_then(Value::as_object) else {
                continue;
            };
            let target_path = target
                .get("path")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            if target_path == "weights" {
                violation(
                    violations,
                    GltfCapabilityViolationKind::MorphWeights,
                    format!("/animations/{animation_index}/channels/{channel_index}/target/path"),
                );
            }
            manifest
                .animation_channels
                .push(GltfAnimationChannelCapability {
                    animation_index,
                    channel_index,
                    target_node_index: as_index(target.get("node")).unwrap_or(usize::MAX),
                    target_path,
                    interpolation: sampler
                        .get("interpolation")
                        .and_then(Value::as_str)
                        .unwrap_or("LINEAR")
                        .to_owned(),
                    input_accessor_index: as_index(sampler.get("input")).unwrap_or(usize::MAX),
                    output_accessor_index: as_index(sampler.get("output")).unwrap_or(usize::MAX),
                });
        }
    }
}

fn inventory_meshes(
    root: &Map<String, Value>,
    manifest: &mut GltfCapabilityManifest,
    violations: &mut Vec<GltfCapabilityViolation>,
) {
    let Some(meshes) = root.get("meshes").and_then(Value::as_array) else {
        return;
    };
    for (mesh_index, mesh) in meshes.iter().enumerate() {
        let Some(mesh) = mesh.as_object() else {
            continue;
        };
        if mesh.contains_key("weights") {
            violation(
                violations,
                GltfCapabilityViolationKind::MorphWeights,
                format!("/meshes/{mesh_index}/weights"),
            );
        }
        let primitives = mesh
            .get("primitives")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default();
        for (primitive_index, primitive) in primitives.iter().enumerate() {
            let Some(primitive) = primitive.as_object() else {
                continue;
            };
            let mode = primitive.get("mode").and_then(Value::as_u64).unwrap_or(4);
            if mode != 4 {
                violation(
                    violations,
                    GltfCapabilityViolationKind::NonTrianglePrimitive,
                    format!("/meshes/{mesh_index}/primitives/{primitive_index}/mode"),
                );
            }
            let mut attributes = primitive
                .get("attributes")
                .and_then(Value::as_object)
                .map(|attributes| {
                    attributes
                        .iter()
                        .map(|(semantic, accessor)| GltfAttributeCapability {
                            semantic: semantic.clone(),
                            accessor_index: as_index(Some(accessor)).unwrap_or(usize::MAX),
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            attributes.sort_by(|left, right| left.semantic.cmp(&right.semantic));
            for attribute in &attributes {
                let semantic = &attribute.semantic;
                let semantic_pointer = json_pointer_token(semantic);
                let location = format!(
                    "/meshes/{mesh_index}/primitives/{primitive_index}/attributes/{semantic_pointer}"
                );
                if is_secondary_influence(semantic) {
                    violation(
                        violations,
                        GltfCapabilityViolationKind::SecondarySkinInfluences,
                        location,
                    );
                } else if !matches!(
                    semantic.as_str(),
                    "POSITION" | "NORMAL" | "TEXCOORD_0" | "JOINTS_0" | "WEIGHTS_0"
                ) {
                    violation(
                        violations,
                        GltfCapabilityViolationKind::UnsupportedVertexAttribute,
                        location,
                    );
                }
            }
            let morph_target_count = primitive
                .get("targets")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            let morph_position_accessors = primitive
                .get("targets")
                .and_then(Value::as_array)
                .map(|targets| {
                    targets
                        .iter()
                        .filter_map(|target| as_index(target.get("POSITION")))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if morph_target_count > 0 {
                violation(
                    violations,
                    GltfCapabilityViolationKind::MorphTarget,
                    format!("/meshes/{mesh_index}/primitives/{primitive_index}/targets"),
                );
            }
            manifest.primitives.push(GltfPrimitiveCapability {
                mesh_index,
                primitive_index,
                mode,
                attributes,
                morph_target_count,
                morph_position_accessors,
            });
        }
    }
}

fn is_secondary_influence(semantic: &str) -> bool {
    semantic
        .strip_prefix("JOINTS_")
        .or_else(|| semantic.strip_prefix("WEIGHTS_"))
        .and_then(|index| index.parse::<u32>().ok())
        .is_some_and(|index| index >= 1)
}

fn inventory_skins(
    root: &Map<String, Value>,
    manifest: &mut GltfCapabilityManifest,
    violations: &mut Vec<GltfCapabilityViolation>,
) {
    let accessors = root
        .get("accessors")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let Some(skins) = root.get("skins").and_then(Value::as_array) else {
        return;
    };
    for (skin_index, skin) in skins.iter().enumerate() {
        let Some(skin) = skin.as_object() else {
            continue;
        };
        let joint_count = skin
            .get("joints")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        let inverse_bind_accessor_index = as_index(skin.get("inverseBindMatrices"));
        let inverse_bind_count = inverse_bind_accessor_index
            .and_then(|index| accessors.get(index))
            .and_then(|accessor| accessor.get("count"))
            .and_then(Value::as_u64);
        let inverse_bind_readable = inverse_bind_accessor_index
            .and_then(|index| accessors.get(index))
            .and_then(Value::as_object)
            .is_some_and(|accessor| {
                accessor.get("bufferView").and_then(Value::as_u64).is_some()
                    && accessor.get("componentType").and_then(Value::as_u64) == Some(5126)
                    && accessor.get("type").and_then(Value::as_str) == Some("MAT4")
                    && !accessor.contains_key("sparse")
            });
        match (inverse_bind_accessor_index, inverse_bind_count) {
            (None, _) => violation(
                violations,
                GltfCapabilityViolationKind::MissingInverseBinds,
                format!("/skins/{skin_index}/inverseBindMatrices"),
            ),
            (Some(_), Some(0)) => violation(
                violations,
                GltfCapabilityViolationKind::EmptyInverseBindAccessor,
                format!("/skins/{skin_index}/inverseBindMatrices"),
            ),
            (Some(_), Some(count)) if count != joint_count as u64 => violation(
                violations,
                GltfCapabilityViolationKind::InverseBindCountMismatch,
                format!("/skins/{skin_index}/inverseBindMatrices"),
            ),
            (Some(_), _) if !inverse_bind_readable => violation(
                violations,
                GltfCapabilityViolationKind::UnreadableInverseBinds,
                format!("/skins/{skin_index}/inverseBindMatrices"),
            ),
            _ => {}
        }
        manifest.skins.push(GltfSkinCapability {
            skin_index,
            joint_count,
            inverse_bind_accessor_index,
            inverse_bind_count,
        });
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum AccessorUse {
    ScaleBearing,
    Dimensionless,
}

/// Which source object owns one byte range in the disjointness inspection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum RangeOwner {
    /// A used accessor's element range.
    Accessor(usize),
    /// An `image`'s complete buffer view.
    ImagePayload(usize),
}

impl RangeOwner {
    /// JSON pointer identifying the owner.
    fn location(self) -> String {
        match self {
            Self::Accessor(index) => format!("/accessors/{index}"),
            Self::ImagePayload(index) => format!("/images/{index}/bufferView"),
        }
    }

    /// The violation kind reported when this owner's range is not disjoint
    /// from a scale-bearing accessor's.
    fn overlap_kind(self) -> GltfCapabilityViolationKind {
        match self {
            Self::Accessor(_) => GltfCapabilityViolationKind::OverlappingAccessorRanges,
            Self::ImagePayload(_) => GltfCapabilityViolationKind::ImagePayloadOverlap,
        }
    }
}

/// One `(buffer, start, end, owner, scale_bearing)` range entry.
type OwnedRange = (usize, usize, usize, RangeOwner, bool);

fn inspect_accessor_layouts(
    root: &Value,
    buffers: &[Vec<u8>],
    uses: &BTreeMap<usize, BTreeSet<AccessorUse>>,
    violations: &mut Vec<GltfCapabilityViolation>,
) {
    let Some(root) = root.as_object() else { return };
    let mut ranges: Vec<OwnedRange> = Vec::new();
    for (&accessor_index, accessor_uses) in uses {
        let scale_bearing = accessor_uses.contains(&AccessorUse::ScaleBearing);
        let range = if scale_bearing {
            dense_f32_accessor_range(root, buffers, accessor_index)
        } else {
            accessor_range(root, buffers, accessor_index)
                .map(|range| (range.buffer, range.start, range.end))
        };
        match range {
            Some((buffer, start, end)) => {
                ranges.push((
                    buffer,
                    start,
                    end,
                    RangeOwner::Accessor(accessor_index),
                    scale_bearing,
                ));
            }
            None => violation(
                violations,
                GltfCapabilityViolationKind::UnsafeAccessorLayout,
                format!("/accessors/{accessor_index}"),
            ),
        }
    }
    ranges.extend(image_payload_ranges(root));
    ranges.sort_unstable();

    let mut overlapping = BTreeSet::new();
    let mut prior_scale: Option<(usize, usize, RangeOwner)> = None;
    for &(buffer, start, end, owner, scale_bearing) in &ranges {
        if let Some((left_buffer, left_end, left_owner)) = prior_scale
            && left_buffer == buffer
            && start < left_end
        {
            overlapping.insert(left_owner);
            overlapping.insert(owner);
        }
        if scale_bearing
            && prior_scale
                .is_none_or(|(left_buffer, left_end, _)| left_buffer != buffer || end > left_end)
        {
            prior_scale = Some((buffer, end, owner));
        }
    }
    let mut later_scale: Option<(usize, usize, RangeOwner)> = None;
    for &(buffer, start, end, owner, scale_bearing) in ranges.iter().rev() {
        if let Some((right_buffer, right_start, right_owner)) = later_scale
            && right_buffer == buffer
            && right_start < end
        {
            overlapping.insert(owner);
            overlapping.insert(right_owner);
        }
        if scale_bearing
            && later_scale.is_none_or(|(right_buffer, right_start, _)| {
                right_buffer != buffer || start < right_start
            })
        {
            later_scale = Some((buffer, start, owner));
        }
    }
    for owner in overlapping {
        violation(violations, owner.overlap_kind(), owner.location());
    }
}

/// The byte range every `image` reads directly from a buffer view.
///
/// # Why images, and why only images
///
/// An `image` is the one consumer in the supported subset that reads a
/// `bufferView` without ever becoming an accessor, so its bytes are invisible
/// to a disjointness proof built from accessor ranges alone. The complete
/// enumeration of `bufferView` consumers in glTF 2.0 core is:
///
/// | Consumer | Treatment |
/// |---|---|
/// | `/accessors/*/bufferView`, referenced by a mesh, skin or sampler | Ranged by [`inspect_accessor_layouts`] above. |
/// | `/accessors/*/sparse/indices/bufferView`, `/accessors/*/sparse/values/bufferView` | Out of range: [`accessor_range`] refuses every `sparse` accessor, so a *used* sparse accessor is already an `UnsafeAccessorLayout` violation and never reaches a rewrite. |
/// | `/images/*/bufferView` | Ranged here. |
/// | Extension payloads such as `EXT_meshopt_compression` or `KHR_draco_mesh_compression` | Out of range: this crate registers no extension handler, so every extension declaration *and* every extension payload is already an `ExtensionDeclaration`/`ExtensionPayload` violation. |
///
/// An accessor that **no** mesh, skin or animation sampler references is not
/// in `uses` and is therefore ranged by neither this function nor the
/// accessor sweep; nor are its `sparse` buffer views. That gap predates this
/// inspection and is not closed here.
///
/// # Bounds
///
/// The range is taken from the declared view, without requiring it to fit the
/// resolved buffer: a view running past the buffer still aliases whatever real
/// bytes it starts on. Where `usize` is narrower than `u64`, a declared value
/// past `usize::MAX` clamps, and neither clamp can hide a real overlap.
/// [`accessor_range`] admits a range only when its end is within the resolved
/// buffer's length, so every range compared against ends below `usize::MAX`: a
/// start large enough to clamp is already past all of them, and a clamped end
/// only widens the image range.
fn image_payload_ranges(root: &Map<String, Value>) -> Vec<OwnedRange> {
    let Some(images) = root.get("images").and_then(Value::as_array) else {
        return Vec::new();
    };
    let buffer_views = root
        .get("bufferViews")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let mut out = Vec::new();
    for (image_index, image) in images.iter().enumerate() {
        let Some(view_index) = as_index(image.get("bufferView")) else {
            continue;
        };
        // An out-of-range index is an `IndexOutOfBounds` validation error,
        // which `validate_document` raises before this inspection runs.
        let Some(view) = buffer_views.get(view_index).and_then(Value::as_object) else {
            continue;
        };
        let Some(buffer) = as_index(view.get("buffer")) else {
            continue;
        };
        let start = clamped_usize(view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0));
        let end = start.saturating_add(clamped_usize(
            view.get("byteLength").and_then(Value::as_u64).unwrap_or(0),
        ));
        // An empty view shares no byte with anything under the half-open
        // comparison every range here uses, so it is dropped rather than
        // ranged. [`crate::scale::reject_image_payload_overlap`] skips the
        // same shape, so the gate and the guard give one answer for it.
        if start < end {
            out.push((
                buffer,
                start,
                end,
                RangeOwner::ImagePayload(image_index),
                false,
            ));
        }
    }
    out
}

fn clamped_usize(value: u64) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

fn inspect_accessor_uses(
    root: &Value,
    violations: &mut Vec<GltfCapabilityViolation>,
) -> BTreeMap<usize, BTreeSet<AccessorUse>> {
    let Some(root) = root.as_object() else {
        return BTreeMap::new();
    };
    let uses = collect_accessor_uses(root);
    for (accessor_index, accessor_uses) in &uses {
        if accessor_uses.len() > 1 {
            violation(
                violations,
                GltfCapabilityViolationKind::ConflictingAccessorUse,
                format!("/accessors/{accessor_index}"),
            );
        }
    }
    uses
}

fn collect_accessor_uses(root: &Map<String, Value>) -> BTreeMap<usize, BTreeSet<AccessorUse>> {
    let mut uses: BTreeMap<usize, BTreeSet<AccessorUse>> = BTreeMap::new();
    let mut add = |index: Option<usize>, kind| {
        if let Some(index) = index {
            uses.entry(index).or_default().insert(kind);
        }
    };
    if let Some(meshes) = root.get("meshes").and_then(Value::as_array) {
        for mesh in meshes {
            let Some(primitives) = mesh.get("primitives").and_then(Value::as_array) else {
                continue;
            };
            for primitive in primitives {
                if let Some(attributes) = primitive.get("attributes").and_then(Value::as_object) {
                    for (semantic, index) in attributes {
                        add(
                            as_index(Some(index)),
                            if semantic == "POSITION" {
                                AccessorUse::ScaleBearing
                            } else {
                                AccessorUse::Dimensionless
                            },
                        );
                    }
                }
                add(
                    as_index(primitive.get("indices")),
                    AccessorUse::Dimensionless,
                );
                if let Some(targets) = primitive.get("targets").and_then(Value::as_array) {
                    for target in targets {
                        if let Some(target) = target.as_object() {
                            for (semantic, index) in target {
                                add(
                                    as_index(Some(index)),
                                    if semantic == "POSITION" {
                                        AccessorUse::ScaleBearing
                                    } else {
                                        AccessorUse::Dimensionless
                                    },
                                );
                            }
                        }
                    }
                }
            }
        }
    }
    if let Some(skins) = root.get("skins").and_then(Value::as_array) {
        for skin in skins {
            add(
                as_index(skin.get("inverseBindMatrices")),
                AccessorUse::ScaleBearing,
            );
        }
    }
    if let Some(animations) = root.get("animations").and_then(Value::as_array) {
        for animation in animations {
            let samplers = animation
                .get("samplers")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or_default();
            let channels = animation
                .get("channels")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or_default();
            for channel in channels {
                let sampler_index = as_index(channel.get("sampler")).unwrap_or(usize::MAX);
                let Some(sampler) = samplers.get(sampler_index) else {
                    continue;
                };
                add(as_index(sampler.get("input")), AccessorUse::Dimensionless);
                let path = channel
                    .get("target")
                    .and_then(|target| target.get("path"))
                    .and_then(Value::as_str);
                add(
                    as_index(sampler.get("output")),
                    if path == Some("translation") {
                        AccessorUse::ScaleBearing
                    } else {
                        AccessorUse::Dimensionless
                    },
                );
            }
        }
    }
    uses
}

/// The `(buffer, start, end)` byte range of a dense, non-normalized,
/// non-sparse, 4-byte-aligned `f32` accessor, or `None` when the accessor is
/// not in that shape.
///
/// Shared with [`crate::scale`]: the byte rewriter must resolve exactly the
/// same range this preflight vouched for, so re-deriving it there would let
/// the two definitions drift apart.
pub(crate) fn dense_f32_accessor_range(
    root: &Map<String, Value>,
    buffers: &[Vec<u8>],
    accessor_index: usize,
) -> Option<(usize, usize, usize)> {
    let accessors = root.get("accessors")?.as_array()?;
    let accessor = accessors.get(accessor_index)?.as_object()?;
    if accessor.get("componentType")?.as_u64()? != 5126
        || accessor.get("normalized").and_then(Value::as_bool) == Some(true)
        || accessor.contains_key("sparse")
    {
        return None;
    }
    let range = accessor_range(root, buffers, accessor_index)?;
    if range.stride != range.element_size || !range.start.is_multiple_of(4) {
        return None;
    }
    Some((range.buffer, range.start, range.end))
}

#[derive(Debug, Clone, Copy)]
struct AccessorRange {
    buffer: usize,
    start: usize,
    end: usize,
    stride: usize,
    element_size: usize,
}

fn accessor_range(
    root: &Map<String, Value>,
    buffers: &[Vec<u8>],
    accessor_index: usize,
) -> Option<AccessorRange> {
    let accessors = root.get("accessors")?.as_array()?;
    let buffer_views = root.get("bufferViews")?.as_array()?;
    let accessor = accessors.get(accessor_index)?.as_object()?;
    if accessor.contains_key("sparse") {
        return None;
    }
    let component_size = match accessor.get("componentType")?.as_u64()? {
        5120 | 5121 => 1usize,
        5122 | 5123 => 2,
        5125 | 5126 => 4,
        _ => return None,
    };
    let element_size = accessor_element_size(accessor.get("type")?.as_str()?, component_size)?;
    let count: usize = accessor.get("count")?.as_u64()?.try_into().ok()?;
    if count == 0 {
        return None;
    }
    let view_index = as_index(accessor.get("bufferView"))?;
    let view = buffer_views.get(view_index)?.as_object()?;
    let buffer_index = as_index(view.get("buffer"))?;
    let buffer = buffers.get(buffer_index)?;
    let view_offset: usize = view
        .get("byteOffset")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .try_into()
        .ok()?;
    let view_length: usize = view.get("byteLength")?.as_u64()?.try_into().ok()?;
    if view_offset.checked_add(view_length)? > buffer.len() {
        return None;
    }
    let accessor_offset: usize = accessor
        .get("byteOffset")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .try_into()
        .ok()?;
    let stride: usize = view
        .get("byteStride")
        .and_then(Value::as_u64)
        .unwrap_or(element_size as u64)
        .try_into()
        .ok()?;
    if stride < element_size {
        return None;
    }
    let relative_end = accessor_offset
        .checked_add(count.checked_sub(1)?.checked_mul(stride)?)?
        .checked_add(element_size)?;
    if relative_end > view_length {
        return None;
    }
    let start = view_offset.checked_add(accessor_offset)?;
    let end = view_offset.checked_add(relative_end)?;
    (end <= buffer.len()).then_some(AccessorRange {
        buffer: buffer_index,
        start,
        end,
        stride,
        element_size,
    })
}

fn accessor_element_size(accessor_type: &str, component_size: usize) -> Option<usize> {
    let (columns, rows, matrix) = match accessor_type {
        "SCALAR" => (1usize, 1usize, false),
        "VEC2" => (1, 2, false),
        "VEC3" => (1, 3, false),
        "VEC4" => (1, 4, false),
        "MAT2" => (2, 2, true),
        "MAT3" => (3, 3, true),
        "MAT4" => (4, 4, true),
        _ => return None,
    };
    let column_size = rows.checked_mul(component_size)?;
    let stored_column_size = if matrix {
        column_size.checked_add(3)? & !3
    } else {
        column_size
    };
    columns.checked_mul(stored_column_size)
}

fn inspect_schema_members(
    value: &Value,
    pointer: &str,
    manifest: &mut GltfCapabilityManifest,
    violations: &mut Vec<GltfCapabilityViolation>,
) {
    match value {
        Value::Object(object) => {
            if object.get("extras").is_some_and(|value| !value.is_null()) {
                let location = format!("{pointer}/extras");
                manifest.extras_locations.push(location.clone());
                violation(violations, GltfCapabilityViolationKind::Extras, location);
            }
            if let Some(extensions) = object.get("extensions").and_then(Value::as_object) {
                for name in extensions.keys() {
                    let location = json_pointer_child(&format!("{pointer}/extensions"), name);
                    manifest.extensions.push(name.clone());
                    manifest.extension_locations.push(location.clone());
                    violation(
                        violations,
                        match name.as_str() {
                            "KHR_lights_punctual" => GltfCapabilityViolationKind::Light,
                            "EXT_mesh_gpu_instancing" => GltfCapabilityViolationKind::Instancing,
                            _ => GltfCapabilityViolationKind::ExtensionPayload,
                        },
                        location,
                    );
                }
            }
            if let Some(allowed) = allowed_members(pointer) {
                for key in object.keys() {
                    if !allowed.contains(&key.as_str()) {
                        let location = json_pointer_child(pointer, key);
                        manifest.unknown_member_locations.push(location.clone());
                        violation(
                            violations,
                            GltfCapabilityViolationKind::UnknownJsonMember,
                            location,
                        );
                    }
                }
            }
            for (key, child) in object {
                if key == "extras" || key == "extensions" {
                    continue;
                }
                inspect_schema_members(
                    child,
                    &json_pointer_child(pointer, key),
                    manifest,
                    violations,
                );
            }
        }
        Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                inspect_schema_members(child, &format!("{pointer}/{index}"), manifest, violations);
            }
        }
        _ => {}
    }
}

fn json_pointer_child(pointer: &str, token: &str) -> String {
    format!("{pointer}/{}", json_pointer_token(token))
}

fn json_pointer_token(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

fn allowed_members(pointer: &str) -> Option<&'static [&'static str]> {
    const ROOT: &[&str] = &[
        "accessors",
        "animations",
        "asset",
        "buffers",
        "bufferViews",
        "cameras",
        "extensions",
        "extensionsRequired",
        "extensionsUsed",
        "extras",
        "images",
        "materials",
        "meshes",
        "nodes",
        "samplers",
        "scene",
        "scenes",
        "skins",
        "textures",
    ];
    const ASSET: &[&str] = &[
        "copyright",
        "extensions",
        "extras",
        "generator",
        "minVersion",
        "version",
    ];
    const ACCESSOR: &[&str] = &[
        "bufferView",
        "byteOffset",
        "componentType",
        "count",
        "extensions",
        "extras",
        "max",
        "min",
        "name",
        "normalized",
        "sparse",
        "type",
    ];
    const BUFFER: &[&str] = &["byteLength", "extensions", "extras", "name", "uri"];
    const VIEW: &[&str] = &[
        "buffer",
        "byteLength",
        "byteOffset",
        "byteStride",
        "extensions",
        "extras",
        "name",
        "target",
    ];
    const NODE: &[&str] = &[
        "camera",
        "children",
        "extensions",
        "extras",
        "matrix",
        "mesh",
        "name",
        "rotation",
        "scale",
        "skin",
        "translation",
        "weights",
    ];
    const MESH: &[&str] = &["extensions", "extras", "name", "primitives", "weights"];
    const PRIMITIVE: &[&str] = &[
        "attributes",
        "extensions",
        "extras",
        "indices",
        "material",
        "mode",
        "targets",
    ];
    const ANIMATION: &[&str] = &["channels", "extensions", "extras", "name", "samplers"];
    const CHANNEL: &[&str] = &["extensions", "extras", "sampler", "target"];
    const TARGET: &[&str] = &["extensions", "extras", "node", "path"];
    const ANIM_SAMPLER: &[&str] = &["extensions", "extras", "input", "interpolation", "output"];
    const SKIN: &[&str] = &[
        "extensions",
        "extras",
        "inverseBindMatrices",
        "joints",
        "name",
        "skeleton",
    ];
    const SCENE: &[&str] = &["extensions", "extras", "name", "nodes"];
    const IMAGE: &[&str] = &[
        "bufferView",
        "extensions",
        "extras",
        "mimeType",
        "name",
        "uri",
    ];
    const TEXTURE: &[&str] = &["extensions", "extras", "name", "sampler", "source"];
    const SAMPLER: &[&str] = &[
        "extensions",
        "extras",
        "magFilter",
        "minFilter",
        "name",
        "wrapS",
        "wrapT",
    ];
    const CAMERA: &[&str] = &[
        "extensions",
        "extras",
        "name",
        "orthographic",
        "perspective",
        "type",
    ];
    const MATERIAL: &[&str] = &[
        "alphaCutoff",
        "alphaMode",
        "doubleSided",
        "emissiveFactor",
        "emissiveTexture",
        "extensions",
        "extras",
        "name",
        "normalTexture",
        "occlusionTexture",
        "pbrMetallicRoughness",
    ];
    const PBR: &[&str] = &[
        "baseColorFactor",
        "baseColorTexture",
        "extensions",
        "extras",
        "metallicFactor",
        "metallicRoughnessTexture",
        "roughnessFactor",
    ];
    const TEXTURE_INFO: &[&str] = &["extensions", "extras", "index", "texCoord"];
    const NORMAL_TEXTURE_INFO: &[&str] = &["extensions", "extras", "index", "scale", "texCoord"];
    const OCCLUSION_TEXTURE_INFO: &[&str] =
        &["extensions", "extras", "index", "strength", "texCoord"];
    const PERSPECTIVE: &[&str] = &[
        "aspectRatio",
        "extensions",
        "extras",
        "yfov",
        "zfar",
        "znear",
    ];
    const ORTHOGRAPHIC: &[&str] = &["extensions", "extras", "xmag", "ymag", "zfar", "znear"];
    const SPARSE: &[&str] = &["count", "extensions", "extras", "indices", "values"];
    const SPARSE_INDICES: &[&str] = &[
        "bufferView",
        "byteOffset",
        "componentType",
        "extensions",
        "extras",
    ];
    const SPARSE_VALUES: &[&str] = &["bufferView", "byteOffset", "extensions", "extras"];
    if pointer.is_empty() {
        Some(ROOT)
    } else if pointer == "/asset" {
        Some(ASSET)
    } else if indexed_member(pointer, "/accessors/") {
        Some(ACCESSOR)
    } else if indexed_member(pointer, "/buffers/") {
        Some(BUFFER)
    } else if indexed_member(pointer, "/bufferViews/") {
        Some(VIEW)
    } else if indexed_member(pointer, "/nodes/") {
        Some(NODE)
    } else if indexed_member(pointer, "/meshes/") {
        Some(MESH)
    } else if indexed_nested_member(pointer, "/meshes/", "/primitives/") {
        Some(PRIMITIVE)
    } else if indexed_member(pointer, "/animations/") {
        Some(ANIMATION)
    } else if indexed_nested_member(pointer, "/animations/", "/channels/") {
        Some(CHANNEL)
    } else if pointer.contains("/animations/") && pointer.ends_with("/target") {
        Some(TARGET)
    } else if indexed_nested_member(pointer, "/animations/", "/samplers/") {
        Some(ANIM_SAMPLER)
    } else if indexed_member(pointer, "/skins/") {
        Some(SKIN)
    } else if indexed_member(pointer, "/scenes/") {
        Some(SCENE)
    } else if indexed_member(pointer, "/images/") {
        Some(IMAGE)
    } else if indexed_member(pointer, "/textures/") {
        Some(TEXTURE)
    } else if indexed_member(pointer, "/samplers/") {
        Some(SAMPLER)
    } else if indexed_member(pointer, "/cameras/") {
        Some(CAMERA)
    } else if indexed_member(pointer, "/materials/") {
        Some(MATERIAL)
    } else if pointer.contains("/materials/") && pointer.ends_with("/pbrMetallicRoughness") {
        Some(PBR)
    } else if pointer.contains("/materials/")
        && (pointer.ends_with("/baseColorTexture")
            || pointer.ends_with("/metallicRoughnessTexture")
            || pointer.ends_with("/emissiveTexture"))
    {
        Some(TEXTURE_INFO)
    } else if pointer.contains("/materials/") && pointer.ends_with("/normalTexture") {
        Some(NORMAL_TEXTURE_INFO)
    } else if pointer.contains("/materials/") && pointer.ends_with("/occlusionTexture") {
        Some(OCCLUSION_TEXTURE_INFO)
    } else if pointer.contains("/cameras/") && pointer.ends_with("/perspective") {
        Some(PERSPECTIVE)
    } else if pointer.contains("/cameras/") && pointer.ends_with("/orthographic") {
        Some(ORTHOGRAPHIC)
    } else if pointer.contains("/accessors/") && pointer.ends_with("/sparse") {
        Some(SPARSE)
    } else if pointer.contains("/accessors/") && pointer.ends_with("/sparse/indices") {
        Some(SPARSE_INDICES)
    } else if pointer.contains("/accessors/") && pointer.ends_with("/sparse/values") {
        Some(SPARSE_VALUES)
    } else {
        None
    }
}

fn indexed_member(pointer: &str, prefix: &str) -> bool {
    pointer
        .strip_prefix(prefix)
        .is_some_and(|suffix| !suffix.is_empty() && !suffix.contains('/'))
}

fn indexed_nested_member(pointer: &str, prefix: &str, nested: &str) -> bool {
    let Some(suffix) = pointer.strip_prefix(prefix) else {
        return false;
    };
    let Some((outer, inner)) = suffix.split_once(nested) else {
        return false;
    };
    !outer.is_empty() && !outer.contains('/') && !inner.is_empty() && !inner.contains('/')
}
