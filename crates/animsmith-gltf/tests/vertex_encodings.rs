//! A primitive accessor its `gltf` reader cannot decode must produce a
//! `LoadError`, never a panic (invariant-1) — whether it is typed for a
//! different element than the reader's, or addressed in a buffer layout the
//! reader cannot walk. The other half is over-rejection: every encoding and
//! every layout glTF legitimately permits for a slot the loader reads must
//! still load, with the same values its plainest equivalent produces.

use animsmith_gltf::LoadError;
use base64::Engine as _;
use serde_json::{Value, json};

const BYTE: u32 = 5120;
const UNSIGNED_BYTE: u32 = 5121;
const SHORT: u32 = 5122;
const UNSIGNED_SHORT: u32 = 5123;
const UNSIGNED_INT: u32 = 5125;
const FLOAT: u32 = 5126;

/// One accessor plus the bytes its buffer view holds.
struct Accessor {
    accessor_type: &'static str,
    component_type: u32,
    count: usize,
    bytes: Vec<u8>,
    bounds: Option<(Vec<f32>, Vec<f32>)>,
    /// Emitted as `"normalized": true`. glTF requires it of an integer
    /// `TEXCOORD_n` or `WEIGHTS_n`, so a fixture claiming to be one has to
    /// carry it — see `a_normalized_attribute_really_declares_the_flag`.
    normalized: bool,
    layout: Layout,
}

impl Default for Accessor {
    fn default() -> Self {
        Self {
            accessor_type: "SCALAR",
            component_type: FLOAT,
            count: 0,
            bytes: Vec::new(),
            bounds: None,
            normalized: false,
            layout: Layout::default(),
        }
    }
}

/// How an accessor addresses its bytes, as distinct from what element it
/// declares. Everything here is a way to make `gltf`'s reader walk memory
/// the accessor's `type`/`componentType` alone do not describe, which is
/// the second half of "an encoding the reader can decode".
#[derive(Default, Clone)]
struct Layout {
    /// `byteStride` on the buffer view this accessor owns.
    stride: Option<u64>,
    /// Written to the accessor's `count` verbatim, replacing the element
    /// count the fixture's bytes actually hold.
    declared_count: Option<u64>,
    /// Read another accessor's buffer view at this byte offset instead of
    /// owning one — what interleaving looks like in the file.
    shared_view: Option<(usize, u64)>,
    /// Emit no `bufferView`, which glTF permits only for a sparse accessor.
    omit_view: bool,
    sparse: Option<Sparse>,
}

/// A `sparse` block, always with `UNSIGNED_SHORT` indices.
#[derive(Clone)]
struct Sparse {
    /// Written to `sparse.count` verbatim.
    declared_count: u64,
    indices: Vec<u16>,
    /// Replacement elements, in the accessor's own encoding.
    values: Vec<u8>,
    /// `byteStride` on the view holding those replacement elements.
    values_stride: Option<u64>,
}

impl Sparse {
    /// A well-formed block replacing element 0 with `values`.
    fn replacing_first(values: Vec<u8>) -> Self {
        Self {
            declared_count: 1,
            indices: vec![0],
            values,
            values_stride: None,
        }
    }

    fn declaring_count(mut self, count: u64) -> Self {
        self.declared_count = count;
        self
    }

    fn values_stride(mut self, stride: u64) -> Self {
        self.values_stride = Some(stride);
        self
    }
}

fn f32s(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}
fn u8s(values: &[u8]) -> Vec<u8> {
    values.to_vec()
}
fn u16s(values: &[u16]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}
fn u32s(values: &[u32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// A single-triangle mesh whose slots are individually replaceable, so one
/// test changes exactly one accessor encoding and nothing else.
struct Primitive {
    mode: u32,
    accessors: Vec<Accessor>,
    attributes: Vec<(String, usize)>,
    indices: Option<usize>,
    inverse_bind: Option<usize>,
    /// Filler meshes emitted ahead of this one, so the mesh index a refusal
    /// reports is something other than 0.
    preceding_meshes: usize,
    /// Filler primitives emitted ahead of this one inside its own mesh.
    preceding_primitives: usize,
}

impl Primitive {
    /// A minimal valid triangle: `POSITION` only.
    fn new() -> Self {
        let mut primitive = Self {
            mode: 4,
            accessors: Vec::new(),
            attributes: Vec::new(),
            indices: None,
            inverse_bind: None,
            preceding_meshes: 0,
            preceding_primitives: 0,
        };
        let positions = primitive.push(Accessor {
            accessor_type: "VEC3",
            component_type: FLOAT,
            count: 3,
            bytes: f32s(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0]),
            bounds: Some((vec![0.0, 0.0, 0.0], vec![1.0, 1.0, 0.0])),
            normalized: false,
            ..Accessor::default()
        });
        primitive.attributes.push(("POSITION".into(), positions));
        primitive
    }

    fn push(&mut self, accessor: Accessor) -> usize {
        self.accessors.push(accessor);
        self.accessors.len() - 1
    }

    /// Declare `semantic` with the given encoding and a bare accessor,
    /// replacing any existing declaration of it so the default `POSITION`
    /// can be re-encoded.
    fn attribute(
        self,
        semantic: &str,
        accessor_type: &'static str,
        component_type: u32,
        count: usize,
        bytes: Vec<u8>,
    ) -> Self {
        self.declare(semantic, accessor_type, component_type, count, bytes, false)
    }

    /// Declare `semantic` on an accessor that also carries
    /// `"normalized": true` — the only spec-legal way to give an integer
    /// `TEXCOORD_n` or `WEIGHTS_n`.
    fn normalized_attribute(
        self,
        semantic: &str,
        accessor_type: &'static str,
        component_type: u32,
        count: usize,
        bytes: Vec<u8>,
    ) -> Self {
        self.declare(semantic, accessor_type, component_type, count, bytes, true)
    }

    fn declare(
        mut self,
        semantic: &str,
        accessor_type: &'static str,
        component_type: u32,
        count: usize,
        bytes: Vec<u8>,
        normalized: bool,
    ) -> Self {
        // `gltf`'s own validation requires a three-element POSITION
        // min/max whatever the accessor `type` says, which is why no test
        // below retypes POSITION: the container rejects that before the
        // loader sees it. Only its `componentType` is ours to catch.
        let bounds = (semantic == "POSITION").then(|| (vec![0.0, 0.0, 0.0], vec![1.0, 1.0, 0.0]));
        let index = self.push(Accessor {
            accessor_type,
            component_type,
            count,
            bytes,
            bounds,
            normalized,
            ..Accessor::default()
        });
        self.attributes.retain(|(name, _)| name != semantic);
        self.attributes.push((semantic.to_owned(), index));
        self
    }

    fn indices(
        mut self,
        accessor_type: &'static str,
        component_type: u32,
        count: usize,
        bytes: Vec<u8>,
    ) -> Self {
        let index = self.push(Accessor {
            accessor_type,
            component_type,
            count,
            bytes,
            bounds: None,
            normalized: false,
            ..Accessor::default()
        });
        self.indices = Some(index);
        self
    }

    fn inverse_bind(
        mut self,
        accessor_type: &'static str,
        component_type: u32,
        count: usize,
        bytes: Vec<u8>,
    ) -> Self {
        let index = self.push(Accessor {
            accessor_type,
            component_type,
            count,
            bytes,
            bounds: None,
            normalized: false,
            ..Accessor::default()
        });
        self.inverse_bind = Some(index);
        self
    }

    fn mode(mut self, mode: u32) -> Self {
        self.mode = mode;
        self
    }

    /// Emit this primitive as mesh `mesh`, primitive `primitive`, padding
    /// with copies of the default `POSITION`-only triangle. Every other
    /// fixture is mesh 0 primitive 0, which is exactly what lets a refusal
    /// report a constant and go unnoticed.
    fn at(mut self, mesh: usize, primitive: usize) -> Self {
        self.preceding_meshes = mesh;
        self.preceding_primitives = primitive;
        self
    }

    /// Apply a layout override to the accessor most recently declared, so
    /// it attaches to exactly the slot the test just named.
    fn layout(mut self, edit: impl FnOnce(&mut Layout)) -> Self {
        let accessor = self.accessors.last_mut().expect("an accessor is declared");
        edit(&mut accessor.layout);
        self
    }

    /// Put `byteStride` on the buffer view that accessor owns.
    fn stride(self, stride: u64) -> Self {
        self.layout(|layout| layout.stride = Some(stride))
    }

    /// Write `count` into the accessor verbatim, whatever its bytes hold.
    fn declared_count(self, count: u64) -> Self {
        self.layout(|layout| layout.declared_count = Some(count))
    }

    fn sparse(self, sparse: Sparse) -> Self {
        self.layout(|layout| layout.sparse = Some(sparse))
    }

    /// Drop the accessor's `bufferView`, leaving its `sparse` block the only
    /// source of values.
    fn without_buffer_view(self) -> Self {
        self.layout(|layout| layout.omit_view = true)
    }

    /// Read `semantic`'s buffer view at `byte_offset` instead of owning one.
    fn interleaved_with(self, semantic: &str, byte_offset: u64) -> Self {
        let owner = self
            .attributes
            .iter()
            .find(|(name, _)| name == semantic)
            .map(|(_, index)| *index)
            .expect("the interleaved-with semantic is declared");
        self.layout(|layout| layout.shared_view = Some((owner, byte_offset)))
    }

    fn to_json(&self) -> Vec<u8> {
        let mut blob = Vec::new();
        let mut views = Vec::new();
        let mut accessors = Vec::new();
        // Which buffer view each accessor reads, so an interleaved accessor
        // can point at the view an earlier one owns.
        let mut view_of: Vec<Option<usize>> = Vec::new();
        for accessor in &self.accessors {
            let view = if accessor.layout.omit_view {
                None
            } else if let Some((owner, _)) = accessor.layout.shared_view {
                view_of[owner]
            } else {
                Some(push_view(
                    &mut blob,
                    &mut views,
                    &accessor.bytes,
                    accessor.layout.stride,
                ))
            };
            view_of.push(view);
            let mut json = json!({
                "componentType": accessor.component_type,
                "count": accessor.layout.declared_count.unwrap_or(accessor.count as u64),
                "type": accessor.accessor_type
            });
            if let Some(view) = view {
                json["bufferView"] = json!(view);
            }
            if let Some((_, byte_offset)) = accessor.layout.shared_view {
                json["byteOffset"] = json!(byte_offset);
            }
            if let Some((min, max)) = &accessor.bounds {
                json["min"] = json!(min);
                json["max"] = json!(max);
            }
            if accessor.normalized {
                json["normalized"] = json!(true);
            }
            if let Some(sparse) = &accessor.layout.sparse {
                let indices = push_view(&mut blob, &mut views, &u16s(&sparse.indices), None);
                let values = push_view(&mut blob, &mut views, &sparse.values, sparse.values_stride);
                json["sparse"] = json!({
                    "count": sparse.declared_count,
                    "indices": {
                        "bufferView": indices,
                        "byteOffset": 0,
                        "componentType": UNSIGNED_SHORT
                    },
                    "values": { "bufferView": values, "byteOffset": 0 }
                });
            }
            accessors.push(json);
        }
        let attributes: serde_json::Map<String, Value> = self
            .attributes
            .iter()
            .map(|(name, index)| (name.clone(), json!(index)))
            .collect();
        let mut primitive = json!({ "mode": self.mode, "attributes": attributes });
        if let Some(indices) = self.indices {
            primitive["indices"] = json!(indices);
        }
        // The filler carries the same POSITION accessor this fixture's own
        // default does, so padding adds meshes and primitives without adding
        // anything else a check could trip over.
        let filler = json!({ "mode": 4, "attributes": { "POSITION": 0 } });
        let mut siblings = vec![filler.clone(); self.preceding_primitives];
        siblings.push(primitive);
        let mut meshes = vec![json!({ "primitives": [filler] }); self.preceding_meshes];
        meshes.push(json!({ "primitives": siblings }));
        let mut node = json!({ "name": "mesh-node", "mesh": self.preceding_meshes });
        let mut document = json!({
            "asset": { "version": "2.0" },
            "buffers": [{
                "uri": format!(
                    "data:application/octet-stream;base64,{}",
                    base64::engine::general_purpose::STANDARD.encode(&blob)
                ),
                "byteLength": blob.len().max(1)
            }],
            "bufferViews": views,
            "accessors": accessors,
            "meshes": meshes,
            "scenes": [{ "nodes": [0] }],
            "scene": 0
        });
        if let Some(inverse_bind) = self.inverse_bind {
            node["skin"] = json!(0);
            document["skins"] = json!([{ "joints": [0], "inverseBindMatrices": inverse_bind }]);
        }
        document["nodes"] = json!([node]);
        serde_json::to_vec(&document).expect("serializes synthetic glTF")
    }

    fn load(&self) -> Result<animsmith_core::model::Document, LoadError> {
        animsmith_gltf::load_bytes(std::path::Path::new("synthetic.gltf"), &self.to_json())
    }

    fn expect_refusal(&self, expected: &str) {
        let error = self.load().expect_err("mistyped accessor must be refused");
        assert!(
            matches!(error, LoadError::PrimitiveEncoding { .. }),
            "expected a PrimitiveEncoding refusal, got {error:?}"
        );
        assert_eq!(error.to_string(), expected);
    }

    fn expect_layout_refusal(&self, expected: &str) {
        let error = self
            .load()
            .expect_err("an accessor the reader cannot walk must be refused");
        assert!(
            matches!(error, LoadError::PrimitiveAccessorLayout { .. }),
            "expected a PrimitiveAccessorLayout refusal, got {error:?}"
        );
        assert_eq!(error.to_string(), expected);
    }

    fn expect_primitive(&self) -> animsmith_core::model::Primitive {
        let document = self.load().expect("valid encoding must load");
        document.assets.meshes[self.preceding_meshes].primitives[self.preceding_primitives].clone()
    }
}

/// Append `bytes` to the buffer as a fresh 4-byte-aligned buffer view.
/// glTF forbids a zero-length view, so an empty one still gets a byte.
fn push_view(
    blob: &mut Vec<u8>,
    views: &mut Vec<Value>,
    bytes: &[u8],
    stride: Option<u64>,
) -> usize {
    while !blob.len().is_multiple_of(4) {
        blob.push(0);
    }
    let mut view = json!({
        "buffer": 0,
        "byteOffset": blob.len(),
        "byteLength": bytes.len().max(1)
    });
    if let Some(stride) = stride {
        view["byteStride"] = json!(stride);
    }
    blob.extend_from_slice(bytes);
    if bytes.is_empty() {
        blob.push(0);
    }
    views.push(view);
    views.len() - 1
}

// --- Refusals: wrong `type` -------------------------------------------

#[test]
fn loader_refuses_vec3_tex_coords() {
    // The issue's reproducer: `size_of::<[f32; 2]>()` is 8 against a VEC3's
    // 12, which trips the `debug_assert_eq!` inside `Iter::<[f32; 2]>::new`.
    Primitive::new()
        .attribute("TEXCOORD_0", "VEC3", FLOAT, 3, f32s(&[0.0; 9]))
        .expect_refusal(
            "mesh 0 primitive 0 TEXCOORD_0: accessor 1 is VEC3 of FLOAT, \
             but the loader reads VEC2 of UNSIGNED_BYTE, UNSIGNED_SHORT, or FLOAT",
        );
}

#[test]
fn loader_refuses_vec4_normals() {
    Primitive::new()
        .attribute("NORMAL", "VEC4", FLOAT, 3, f32s(&[0.0; 12]))
        .expect_refusal(
            "mesh 0 primitive 0 NORMAL: accessor 1 is VEC4 of FLOAT, \
             but the loader reads VEC3 of FLOAT",
        );
}

#[test]
fn loader_refuses_vec3_joints() {
    Primitive::new()
        .attribute("JOINTS_0", "VEC3", UNSIGNED_SHORT, 3, u16s(&[0; 9]))
        .attribute("WEIGHTS_0", "VEC4", FLOAT, 3, f32s(&[0.0; 12]))
        .expect_refusal(
            "mesh 0 primitive 0 JOINTS_0: accessor 1 is VEC3 of UNSIGNED_SHORT, \
             but the loader reads VEC4 of UNSIGNED_BYTE or UNSIGNED_SHORT",
        );
}

#[test]
fn loader_refuses_vec3_weights() {
    Primitive::new()
        .attribute("JOINTS_0", "VEC4", UNSIGNED_SHORT, 3, u16s(&[0; 12]))
        .attribute("WEIGHTS_0", "VEC3", FLOAT, 3, f32s(&[0.0; 9]))
        .expect_refusal(
            "mesh 0 primitive 0 WEIGHTS_0: accessor 2 is VEC3 of FLOAT, \
             but the loader reads VEC4 of UNSIGNED_BYTE, UNSIGNED_SHORT, or FLOAT",
        );
}

#[test]
fn loader_refuses_vec2_indices() {
    Primitive::new()
        .indices("VEC2", UNSIGNED_SHORT, 3, u16s(&[0; 6]))
        .expect_refusal(
            "mesh 0 primitive 0 indices: accessor 1 is VEC2 of UNSIGNED_SHORT, \
             but the loader reads SCALAR of UNSIGNED_BYTE, UNSIGNED_SHORT, or UNSIGNED_INT",
        );
}

// --- Refusals: wrong `componentType` ----------------------------------

#[test]
fn loader_refuses_signed_byte_tex_coords() {
    // Right element size, wrong component type: `read_tex_coords` has no
    // arm for BYTE and hits its `unreachable!()`.
    Primitive::new()
        .attribute("TEXCOORD_0", "VEC2", BYTE, 3, u8s(&[0; 6]))
        .expect_refusal(
            "mesh 0 primitive 0 TEXCOORD_0: accessor 1 is VEC2 of BYTE, \
             but the loader reads VEC2 of UNSIGNED_BYTE, UNSIGNED_SHORT, or FLOAT",
        );
}

#[test]
fn loader_refuses_float_joints() {
    // `read_joints` decodes only the two integer encodings glTF permits.
    Primitive::new()
        .attribute("JOINTS_0", "VEC4", FLOAT, 3, f32s(&[0.0; 12]))
        .attribute("WEIGHTS_0", "VEC4", FLOAT, 3, f32s(&[0.0; 12]))
        .expect_refusal(
            "mesh 0 primitive 0 JOINTS_0: accessor 1 is VEC4 of FLOAT, \
             but the loader reads VEC4 of UNSIGNED_BYTE or UNSIGNED_SHORT",
        );
}

#[test]
fn loader_refuses_signed_short_weights() {
    Primitive::new()
        .attribute("JOINTS_0", "VEC4", UNSIGNED_SHORT, 3, u16s(&[0; 12]))
        .attribute("WEIGHTS_0", "VEC4", SHORT, 3, u16s(&[0; 12]))
        .expect_refusal(
            "mesh 0 primitive 0 WEIGHTS_0: accessor 2 is VEC4 of SHORT, \
             but the loader reads VEC4 of UNSIGNED_BYTE, UNSIGNED_SHORT, or FLOAT",
        );
}

#[test]
fn loader_refuses_float_indices() {
    Primitive::new()
        .indices("SCALAR", FLOAT, 3, f32s(&[0.0, 1.0, 2.0]))
        .expect_refusal(
            "mesh 0 primitive 0 indices: accessor 1 is SCALAR of FLOAT, \
             but the loader reads SCALAR of UNSIGNED_BYTE, UNSIGNED_SHORT, or UNSIGNED_INT",
        );
}

#[test]
fn loader_refuses_unsigned_int_positions_that_would_be_reinterpreted() {
    // A VEC3 of UNSIGNED_INT is 12 bytes exactly like `[f32; 3]`, so no
    // assertion fires: without the check every position would silently load
    // as the float reading of an integer's bits (invariant-9).
    Primitive::new()
        .attribute("POSITION", "VEC3", UNSIGNED_INT, 3, u32s(&[1; 9]))
        .expect_refusal(
            "mesh 0 primitive 0 POSITION: accessor 1 is VEC3 of UNSIGNED_INT, \
             but the loader reads VEC3 of FLOAT",
        );
}

#[test]
fn loader_refuses_unsigned_int_normals_that_would_be_reinterpreted() {
    Primitive::new()
        .attribute("NORMAL", "VEC3", UNSIGNED_INT, 3, u32s(&[1; 9]))
        .expect_refusal(
            "mesh 0 primitive 0 NORMAL: accessor 1 is VEC3 of UNSIGNED_INT, \
             but the loader reads VEC3 of FLOAT",
        );
}

// --- The accepted set: no over-rejection ------------------------------

/// UVs a `FLOAT` accessor and its normalized-integer equivalents all encode
/// exactly: 255/255 and 65535/65535 are both 1.0.
const EXPECTED_UVS: [[f32; 2]; 3] = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]];

/// The `UNSIGNED_BYTE` spelling of [`EXPECTED_UVS`], full-scale.
const UV_BYTES: [u8; 6] = [0, 0, 255, 0, 0, 255];
/// The `UNSIGNED_SHORT` spelling of [`EXPECTED_UVS`], full-scale.
const UV_SHORTS: [u16; 6] = [0, 0, 65535, 0, 0, 65535];

#[test]
fn float_tex_coords_load() {
    let primitive = Primitive::new()
        .attribute(
            "TEXCOORD_0",
            "VEC2",
            FLOAT,
            3,
            f32s(&[0.0, 0.0, 1.0, 0.0, 0.0, 1.0]),
        )
        .expect_primitive();
    assert_eq!(primitive.uvs, EXPECTED_UVS);
}

#[test]
fn normalized_unsigned_byte_tex_coords_load_as_the_float_equivalent() {
    let primitive = Primitive::new()
        .normalized_attribute("TEXCOORD_0", "VEC2", UNSIGNED_BYTE, 3, u8s(&UV_BYTES))
        .expect_primitive();
    assert_eq!(primitive.uvs, EXPECTED_UVS);
}

#[test]
fn normalized_unsigned_short_tex_coords_load_as_the_float_equivalent() {
    let primitive = Primitive::new()
        .normalized_attribute("TEXCOORD_0", "VEC2", UNSIGNED_SHORT, 3, u16s(&UV_SHORTS))
        .expect_primitive();
    assert_eq!(primitive.uvs, EXPECTED_UVS);
}

#[test]
fn integer_tex_coords_without_the_normalized_flag_still_load() {
    // glTF requires `"normalized": true` on an integer `TEXCOORD_n`, so
    // these two documents are invalid — but they are invalid in a way the
    // reader decodes perfectly well, and this loader is not a spec
    // validator (see `attributes_no_reader_touches_are_not_refused`).
    // Refusing them would reject files that measure fine, so they load.
    //
    // They decode to the same values as the flagged fixtures above because
    // `gltf`'s `into_f32()` rescales `UNSIGNED_BYTE`/`UNSIGNED_SHORT` from
    // full scale whatever the flag says — the flag is behaviourally inert
    // on the load path, and only there: the `scale` preflight reads it to
    // raise `UnsafeAccessorLayout`. That is `gltf`'s extraction behaviour,
    // pinned here as what this loader currently hands checks, not a claim
    // that it is the authored value.
    for primitive in [
        Primitive::new().attribute("TEXCOORD_0", "VEC2", UNSIGNED_BYTE, 3, u8s(&UV_BYTES)),
        Primitive::new().attribute("TEXCOORD_0", "VEC2", UNSIGNED_SHORT, 3, u16s(&UV_SHORTS)),
    ] {
        assert_eq!(primitive.expect_primitive().uvs, EXPECTED_UVS);
    }
}

/// Joint indices and weights the loader must read identically from either
/// integer joint encoding.
const EXPECTED_JOINTS: [[u16; 4]; 3] = [[0, 1, 2, 3], [1, 2, 3, 0], [2, 3, 0, 1]];
const EXPECTED_WEIGHTS: [[f32; 4]; 3] = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
];

/// A skinned triangle whose `WEIGHTS_0` accessor carries no `normalized`
/// flag — correct only when the weights are `FLOAT`.
fn skinned(
    joint_type: u32,
    joint_bytes: Vec<u8>,
    weight_type: u32,
    weight_bytes: Vec<u8>,
) -> Primitive {
    Primitive::new()
        .attribute("JOINTS_0", "VEC4", joint_type, 3, joint_bytes)
        .attribute("WEIGHTS_0", "VEC4", weight_type, 3, weight_bytes)
}

/// The same triangle with `"normalized": true` on `WEIGHTS_0`, which glTF
/// requires of an integer weight accessor. `JOINTS_0` never carries it:
/// joint indices are indices, not a normalized range.
fn skinned_normalized_weights(
    joint_type: u32,
    joint_bytes: Vec<u8>,
    weight_type: u32,
    weight_bytes: Vec<u8>,
) -> Primitive {
    Primitive::new()
        .attribute("JOINTS_0", "VEC4", joint_type, 3, joint_bytes)
        .normalized_attribute("WEIGHTS_0", "VEC4", weight_type, 3, weight_bytes)
}

const JOINT_VALUES: [u16; 12] = [0, 1, 2, 3, 1, 2, 3, 0, 2, 3, 0, 1];
const WEIGHT_FLOATS: [f32; 12] = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];

/// The `UNSIGNED_BYTE` spelling of [`WEIGHT_FLOATS`], full-scale.
fn weight_bytes() -> Vec<u8> {
    u8s(&WEIGHT_FLOATS.map(|weight| (weight * 255.0) as u8))
}
/// The `UNSIGNED_SHORT` spelling of [`WEIGHT_FLOATS`], full-scale.
fn weight_shorts() -> Vec<u8> {
    u16s(&WEIGHT_FLOATS.map(|weight| (weight * 65535.0) as u16))
}

#[test]
fn unsigned_byte_joints_load() {
    let bytes = u8s(&JOINT_VALUES.map(|value| value as u8));
    let primitive = skinned(UNSIGNED_BYTE, bytes, FLOAT, f32s(&WEIGHT_FLOATS)).expect_primitive();
    assert_eq!(primitive.joints, EXPECTED_JOINTS);
    assert_eq!(primitive.weights, EXPECTED_WEIGHTS);
}

#[test]
fn unsigned_short_joints_load_as_the_byte_equivalent() {
    let primitive = skinned(
        UNSIGNED_SHORT,
        u16s(&JOINT_VALUES),
        FLOAT,
        f32s(&WEIGHT_FLOATS),
    )
    .expect_primitive();
    assert_eq!(primitive.joints, EXPECTED_JOINTS);
    assert_eq!(primitive.weights, EXPECTED_WEIGHTS);
}

#[test]
fn normalized_unsigned_byte_weights_load_as_the_float_equivalent() {
    let primitive = skinned_normalized_weights(
        UNSIGNED_SHORT,
        u16s(&JOINT_VALUES),
        UNSIGNED_BYTE,
        weight_bytes(),
    )
    .expect_primitive();
    assert_eq!(primitive.joints, EXPECTED_JOINTS);
    assert_eq!(primitive.weights, EXPECTED_WEIGHTS);
}

#[test]
fn normalized_unsigned_short_weights_load_as_the_float_equivalent() {
    let primitive = skinned_normalized_weights(
        UNSIGNED_SHORT,
        u16s(&JOINT_VALUES),
        UNSIGNED_SHORT,
        weight_shorts(),
    )
    .expect_primitive();
    assert_eq!(primitive.joints, EXPECTED_JOINTS);
    assert_eq!(primitive.weights, EXPECTED_WEIGHTS);
}

#[test]
fn integer_weights_without_the_normalized_flag_still_load() {
    // The `WEIGHTS_0` counterpart of
    // `integer_tex_coords_without_the_normalized_flag_still_load`: invalid
    // glTF that the reader decodes, so the loader does not refuse it, and
    // `into_f32()` rescales it from full scale regardless of the flag.
    for primitive in [
        skinned(
            UNSIGNED_SHORT,
            u16s(&JOINT_VALUES),
            UNSIGNED_BYTE,
            weight_bytes(),
        ),
        skinned(
            UNSIGNED_SHORT,
            u16s(&JOINT_VALUES),
            UNSIGNED_SHORT,
            weight_shorts(),
        ),
    ] {
        let primitive = primitive.expect_primitive();
        assert_eq!(primitive.joints, EXPECTED_JOINTS);
        assert_eq!(primitive.weights, EXPECTED_WEIGHTS);
    }
}

/// The fixtures above are the only witness that a *normalized* integer
/// accessor loads, so they have to really be normalized. No loaded value
/// can prove it: `gltf`'s `into_f32()` rescales `UNSIGNED_BYTE`/
/// `UNSIGNED_SHORT` irrespective of the `normalized` metadata, so the
/// flagged and unflagged fixtures decode identically. Only the document
/// distinguishes them — so assert on the document, and the "normalized"
/// tests above cannot silently decay into duplicates of the unflagged ones.
///
/// Every normalized fixture is listed here by name, not a representative
/// one: the flag is emitted per accessor, so a serializer that emitted it
/// for `UNSIGNED_BYTE` only would leave both `UNSIGNED_SHORT` fixtures
/// silently unflagged — duplicates of the "without the flag" tests, with
/// nothing left proving a normalized accessor loads at all.
#[test]
fn a_normalized_attribute_really_declares_the_flag() {
    let flag_of = |primitive: &Primitive, semantic: &str| -> Option<Value> {
        let document: Value =
            serde_json::from_slice(&primitive.to_json()).expect("fixture is JSON");
        let index = document["meshes"][0]["primitives"][0]["attributes"][semantic]
            .as_u64()
            .expect("semantic is declared");
        document["accessors"][index as usize]
            .get("normalized")
            .cloned()
    };

    // One row per fixture the "normalized ... load as the float equivalent"
    // tests build, across both integer widths and both slots that take one.
    let flagged: [(&str, &str, Primitive); 4] = [
        (
            "UNSIGNED_BYTE TEXCOORD_0",
            "TEXCOORD_0",
            Primitive::new().normalized_attribute(
                "TEXCOORD_0",
                "VEC2",
                UNSIGNED_BYTE,
                3,
                u8s(&UV_BYTES),
            ),
        ),
        (
            "UNSIGNED_SHORT TEXCOORD_0",
            "TEXCOORD_0",
            Primitive::new().normalized_attribute(
                "TEXCOORD_0",
                "VEC2",
                UNSIGNED_SHORT,
                3,
                u16s(&UV_SHORTS),
            ),
        ),
        (
            "UNSIGNED_BYTE WEIGHTS_0",
            "WEIGHTS_0",
            skinned_normalized_weights(
                UNSIGNED_SHORT,
                u16s(&JOINT_VALUES),
                UNSIGNED_BYTE,
                weight_bytes(),
            ),
        ),
        (
            "UNSIGNED_SHORT WEIGHTS_0",
            "WEIGHTS_0",
            skinned_normalized_weights(
                UNSIGNED_SHORT,
                u16s(&JOINT_VALUES),
                UNSIGNED_SHORT,
                weight_shorts(),
            ),
        ),
    ];
    for (name, semantic, primitive) in &flagged {
        assert_eq!(
            flag_of(primitive, semantic),
            Some(json!(true)),
            "the {name} fixture must really carry the flag"
        );
    }

    // Joint indices and the bare builder must stay unflagged, or the
    // "without the flag" tests would not be testing what they claim.
    let unflagged: [(&str, &str, Primitive); 3] = [
        (
            "JOINTS_0 alongside a normalized WEIGHTS_0",
            "JOINTS_0",
            skinned_normalized_weights(
                UNSIGNED_SHORT,
                u16s(&JOINT_VALUES),
                UNSIGNED_BYTE,
                weight_bytes(),
            ),
        ),
        (
            "UNSIGNED_BYTE TEXCOORD_0 declared bare",
            "TEXCOORD_0",
            Primitive::new().attribute("TEXCOORD_0", "VEC2", UNSIGNED_BYTE, 3, u8s(&UV_BYTES)),
        ),
        (
            "UNSIGNED_SHORT TEXCOORD_0 declared bare",
            "TEXCOORD_0",
            Primitive::new().attribute("TEXCOORD_0", "VEC2", UNSIGNED_SHORT, 3, u16s(&UV_SHORTS)),
        ),
    ];
    for (name, semantic, primitive) in &unflagged {
        assert_eq!(
            flag_of(primitive, semantic),
            None,
            "the {name} fixture must stay unflagged"
        );
    }
}

#[test]
fn every_index_encoding_loads_the_same_triangle() {
    for primitive in [
        Primitive::new().indices("SCALAR", UNSIGNED_BYTE, 3, u8s(&[0, 1, 2])),
        Primitive::new().indices("SCALAR", UNSIGNED_SHORT, 3, u16s(&[0, 1, 2])),
        Primitive::new().indices("SCALAR", UNSIGNED_INT, 3, u32s(&[0, 1, 2])),
    ] {
        assert_eq!(primitive.expect_primitive().indices, [0, 1, 2]);
    }
}

// --- Slots the loader does not read stay unjudged ---------------------

#[test]
fn attributes_no_reader_touches_are_not_refused() {
    // None of these reaches a reader, so a nonsense encoding on one cannot
    // panic — and refusing it would turn the loader into a spec validator
    // that rejects files it reads perfectly well.
    for primitive in [
        Primitive::new().attribute("COLOR_0", "VEC3", UNSIGNED_BYTE, 3, u8s(&[0; 9])),
        Primitive::new().attribute("COLOR_0", "MAT4", FLOAT, 3, f32s(&[0.0; 48])),
        Primitive::new().attribute("TANGENT", "SCALAR", BYTE, 3, u8s(&[0; 3])),
        Primitive::new().attribute("TEXCOORD_1", "VEC3", UNSIGNED_INT, 3, u32s(&[0; 9])),
        Primitive::new()
            .attribute("JOINTS_0", "VEC4", UNSIGNED_SHORT, 3, u16s(&[0; 12]))
            .attribute("WEIGHTS_0", "VEC4", FLOAT, 3, f32s(&[0.0; 12]))
            .attribute("JOINTS_1", "SCALAR", FLOAT, 3, f32s(&[0.0; 3])),
    ] {
        assert_eq!(primitive.expect_primitive().positions.len(), 3);
    }
}

#[test]
fn a_skipped_primitive_mode_is_not_judged() {
    // `extract_assets` skips non-triangle primitives whole, so no reader is
    // built and nothing can panic. The document loads with no mesh.
    let document = Primitive::new()
        .mode(0)
        .attribute("TEXCOORD_0", "VEC3", FLOAT, 3, f32s(&[0.0; 9]))
        .load()
        .expect("a skipped primitive is not judged");
    assert!(document.assets.meshes.is_empty());
}

#[test]
fn an_unreadable_inverse_bind_accessor_stays_source_evidence() {
    // A mistyped inverse-bind accessor panics in its reader just like a
    // vertex attribute, but the loader's contract reports it as unreadable
    // source evidence rather than refusing the file.
    use animsmith_core::model::SourceInverseBindAccessorStatus;
    let document = Primitive::new()
        .inverse_bind("VEC4", FLOAT, 1, f32s(&[0.0; 4]))
        .load()
        .expect("an unreadable inverse-bind accessor is not a load error");
    let skin = &document.assets.source_skeleton.skins[0];
    assert_eq!(
        skin.inverse_bind_accessor.status,
        SourceInverseBindAccessorStatus::Unreadable
    );
    assert_eq!(skin.inverse_bind_accessor.declared_count, Some(1));
    assert!(skin.inverse_bind_accessor.matrices.is_empty());
    assert!(document.assets.instances[0].skin_ibms.is_empty());
}

#[test]
fn a_well_formed_inverse_bind_accessor_still_loads() {
    let identity: Vec<f32> = vec![
        1.0, 0.0, 0.0, 0.0, //
        0.0, 1.0, 0.0, 0.0, //
        0.0, 0.0, 1.0, 0.0, //
        0.0, 0.0, 0.0, 1.0,
    ];
    let document = Primitive::new()
        .inverse_bind("MAT4", FLOAT, 1, f32s(&identity))
        .load()
        .expect("a MAT4 of FLOAT inverse-bind accessor loads");
    assert_eq!(document.assets.instances[0].skin_ibms.len(), 1);
}

// --- Refusals: a layout the reader cannot walk ------------------------

/// The default triangle's positions, for fixtures that re-encode
/// `POSITION` rather than take the builder's own.
const TRIANGLE_POSITIONS: [f32; 9] = [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0];

fn identity_matrix() -> Vec<u8> {
    f32s(&[
        1.0, 0.0, 0.0, 0.0, //
        0.0, 1.0, 0.0, 0.0, //
        0.0, 0.0, 1.0, 0.0, //
        0.0, 0.0, 0.0, 1.0,
    ])
}

/// A `POSITION` on the builder's own buffer view, so a `byteStride` can be
/// put on it.
fn positions_with_own_view() -> Primitive {
    Primitive::new().attribute("POSITION", "VEC3", FLOAT, 3, f32s(&TRIANGLE_POSITIONS))
}

#[test]
fn loader_refuses_a_byte_stride_shorter_than_its_element() {
    // `gltf-json`'s `Stride` validator accepts any 4..=252, so a stride
    // below the element size passes `Validate` and then trips
    // `debug_assert!(stride >= size_of::<T>())` inside `Iter::new`.
    for stride in [4, 8] {
        positions_with_own_view()
            .stride(stride)
            .expect_layout_refusal(&format!(
                "mesh 0 primitive 0 POSITION: accessor 1 reads its elements from buffer view 1 \
             at byteStride {stride}, shorter than the 12-byte element it strides over"
            ));
    }
}

#[test]
fn loader_refuses_a_short_byte_stride_on_every_slot_it_reads() {
    // The same hole on the other read slots, so deleting the check from any
    // one of them is caught: NORMAL and TEXCOORD_0 assert in `Iter::new`,
    // and WEIGHTS_0 only reaches its reader because JOINTS_0 is present.
    Primitive::new()
        .attribute("NORMAL", "VEC3", FLOAT, 3, f32s(&[0.0; 9]))
        .stride(8)
        .expect_layout_refusal(
            "mesh 0 primitive 0 NORMAL: accessor 1 reads its elements from buffer view 1 \
             at byteStride 8, shorter than the 12-byte element it strides over",
        );
    Primitive::new()
        .attribute("TEXCOORD_0", "VEC2", FLOAT, 3, f32s(&[0.0; 6]))
        .stride(4)
        .expect_layout_refusal(
            "mesh 0 primitive 0 TEXCOORD_0: accessor 1 reads its elements from buffer view 1 \
             at byteStride 4, shorter than the 8-byte element it strides over",
        );
    Primitive::new()
        .attribute("JOINTS_0", "VEC4", UNSIGNED_SHORT, 3, u16s(&JOINT_VALUES))
        .attribute("WEIGHTS_0", "VEC4", FLOAT, 3, f32s(&WEIGHT_FLOATS))
        .stride(12)
        .expect_layout_refusal(
            "mesh 0 primitive 0 WEIGHTS_0: accessor 2 reads its elements from buffer view 2 \
             at byteStride 12, shorter than the 16-byte element it strides over",
        );
}

#[test]
fn loader_refuses_a_sparse_block_of_count_zero() {
    // `sparse_count - 1` underflows in `Iter::new` before either branch
    // reads a byte, so it panics with and without a base `bufferView`.
    // Every count-zero guard in the loader reads `Accessor::count()`, which
    // is 3 here and says nothing about the sparse block.
    positions_with_own_view()
        .sparse(Sparse::replacing_first(f32s(&[9.0, 9.0, 9.0])).declaring_count(0))
        .expect_layout_refusal(
            "mesh 0 primitive 0 POSITION: accessor 1 declares a sparse block of count 0, \
             which its reader cannot walk",
        );
    Primitive::new()
        .attribute("TEXCOORD_0", "VEC2", FLOAT, 3, Vec::new())
        .without_buffer_view()
        .sparse(Sparse::replacing_first(f32s(&[9.0, 9.0])).declaring_count(0))
        .expect_layout_refusal(
            "mesh 0 primitive 0 TEXCOORD_0: accessor 1 declares a sparse block of count 0, \
             which its reader cannot walk",
        );
}

#[test]
fn loader_refuses_a_count_whose_byte_extent_overflows() {
    // Nothing relates `count` to the view it reads, so `stride * (count - 1)`
    // overflows `usize` outright on a large enough declaration.
    positions_with_own_view()
        .declared_count(u64::MAX)
        .expect_layout_refusal(
            "mesh 0 primitive 0 POSITION: accessor 1 walks 18446744073709551615 elements \
             of 12 bytes at byteStride 12 from byteOffset 0, a byte extent that overflows",
        );
}

#[test]
fn loader_refuses_a_sparse_count_whose_byte_extent_overflows() {
    // `sparse.count` is unbounded in exactly the same way, and is walked
    // over the index view before the accessor's own is touched.
    positions_with_own_view()
        .sparse(Sparse::replacing_first(f32s(&[9.0, 9.0, 9.0])).declaring_count(u64::MAX))
        .expect_layout_refusal(
            "mesh 0 primitive 0 POSITION: accessor 1 walks 18446744073709551615 sparse indices \
             of 2 bytes at byteStride 2 from byteOffset 0, a byte extent that overflows",
        );
}

#[test]
fn loader_refuses_sparse_values_strided_shorter_than_their_element() {
    // The sparse branch of `Iter::new` compiles no stride assertion, so a
    // short-strided values view reaches `Item::from_slice`, which asserts
    // on the truncated slice instead. Same defect, different panic site.
    positions_with_own_view()
        .sparse(Sparse::replacing_first(f32s(&[9.0, 9.0, 9.0])).values_stride(4))
        .expect_layout_refusal(
            "mesh 0 primitive 0 POSITION: accessor 1 reads its sparse values from buffer view 3 \
             at byteStride 4, shorter than the 12-byte element it strides over",
        );
}

// --- Layouts that must keep loading -----------------------------------

#[test]
fn an_interleaved_buffer_view_still_decodes_every_attribute_on_it() {
    // The over-rejection risk of the stride check: a correctly-sized
    // interleaved view is what real exporters emit. POSITION and
    // TEXCOORD_0 share one stride-20 view at offsets 0 and 12.
    let mut bytes = Vec::new();
    for vertex in 0..3 {
        bytes.extend(f32s(&TRIANGLE_POSITIONS[vertex * 3..vertex * 3 + 3]));
        bytes.extend(f32s(&EXPECTED_UVS[vertex]));
    }
    let primitive = Primitive::new()
        .attribute("POSITION", "VEC3", FLOAT, 3, bytes)
        .stride(20)
        .attribute("TEXCOORD_0", "VEC2", FLOAT, 3, Vec::new())
        .interleaved_with("POSITION", 12)
        .expect_primitive();
    assert_eq!(primitive.positions.len(), 3);
    assert_eq!(primitive.positions[1].x, 1.0);
    assert_eq!(primitive.uvs, EXPECTED_UVS);
}

#[test]
fn a_sparse_accessor_with_a_nonzero_count_still_loads() {
    // The over-rejection risk of the sparse checks. The block really is
    // applied: vertex 0 comes from the sparse values, not the base view.
    let primitive = positions_with_own_view()
        .sparse(Sparse::replacing_first(f32s(&[9.0, 9.0, 9.0])))
        .expect_primitive();
    assert_eq!(primitive.positions.len(), 3);
    assert_eq!(primitive.positions[0].x, 9.0);
    assert_eq!(primitive.positions[1].x, 1.0);
}

#[test]
fn a_count_that_overruns_its_buffer_view_still_loads_empty() {
    // Pins the documented non-guarantee: only extent arithmetic that
    // *overflows* is refused. A `count` that merely exceeds its 36-byte
    // view makes `Iter::new` answer `None`, and `read_positions` is
    // `unwrap_or_default()`, so the primitive is pushed with no geometry
    // rather than refused. Changing that is a behaviour change, not a
    // tightening of this check.
    let primitive = positions_with_own_view()
        .declared_count(100)
        .expect_primitive();
    assert!(primitive.positions.is_empty());
}

// --- The refusal names the slot it found ------------------------------

#[test]
fn a_refusal_names_the_mesh_and_primitive_it_found() {
    // Every other fixture is mesh 0 primitive 0, which is exactly what lets
    // a hard-coded 0 in either field go unnoticed. Naming the primitive is
    // an acceptance criterion of #312.
    Primitive::new()
        .at(1, 2)
        .attribute("TEXCOORD_0", "VEC3", FLOAT, 3, f32s(&[0.0; 9]))
        .expect_refusal(
            "mesh 1 primitive 2 TEXCOORD_0: accessor 1 is VEC3 of FLOAT, \
             but the loader reads VEC2 of UNSIGNED_BYTE, UNSIGNED_SHORT, or FLOAT",
        );
    positions_with_own_view()
        .at(2, 1)
        .stride(4)
        .expect_layout_refusal(
            "mesh 2 primitive 1 POSITION: accessor 1 reads its elements from buffer view 1 \
             at byteStride 4, shorter than the 12-byte element it strides over",
        );
}

#[test]
fn a_refusal_spells_the_matrix_accessor_types() {
    // `MAT2` and `MAT3` reach a message only through a slot that refuses
    // them, so nothing else in this file would notice the two names being
    // swapped.
    Primitive::new()
        .attribute("TEXCOORD_0", "MAT2", FLOAT, 3, f32s(&[0.0; 12]))
        .expect_refusal(
            "mesh 0 primitive 0 TEXCOORD_0: accessor 1 is MAT2 of FLOAT, \
             but the loader reads VEC2 of UNSIGNED_BYTE, UNSIGNED_SHORT, or FLOAT",
        );
    Primitive::new()
        .attribute("TEXCOORD_0", "MAT3", FLOAT, 3, f32s(&[0.0; 27]))
        .expect_refusal(
            "mesh 0 primitive 0 TEXCOORD_0: accessor 1 is MAT3 of FLOAT, \
             but the loader reads VEC2 of UNSIGNED_BYTE, UNSIGNED_SHORT, or FLOAT",
        );
}

#[test]
fn a_count_zero_accessor_is_still_judged() {
    // The check is deliberately independent of the count-zero guards that
    // decide whether a read happens, so that moving one of those guards
    // cannot open a hole. A count-0 `TEXCOORD_0` is treated as absent by
    // the loader and would never reach a reader — and is refused anyway.
    Primitive::new()
        .attribute("TEXCOORD_0", "VEC3", FLOAT, 0, f32s(&[0.0; 9]))
        .expect_refusal(
            "mesh 0 primitive 0 TEXCOORD_0: accessor 1 is VEC3 of FLOAT, \
             but the loader reads VEC2 of UNSIGNED_BYTE, UNSIGNED_SHORT, or FLOAT",
        );
}

// --- Inverse binds: the same judgement, a different answer ------------

/// `inverseBindMatrices` is unreadable for `reason`, and stays source
/// evidence rather than refusing the document.
fn expect_unreadable_inverse_bind(primitive: Primitive, reason: &str) {
    use animsmith_core::model::SourceInverseBindAccessorStatus;
    let document = primitive
        .load()
        .unwrap_or_else(|error| panic!("{reason} must not refuse the document: {error}"));
    let skin = &document.assets.source_skeleton.skins[0];
    assert_eq!(
        skin.inverse_bind_accessor.status,
        SourceInverseBindAccessorStatus::Unreadable,
        "{reason}"
    );
    assert!(skin.inverse_bind_accessor.matrices.is_empty(), "{reason}");
    assert!(
        document.assets.instances[0].skin_ibms.is_empty(),
        "{reason}"
    );
    assert!(
        document.skeleton.bones[0].inverse_bind.is_none(),
        "{reason}"
    );
}

#[test]
fn an_unsigned_int_inverse_bind_stays_unreadable() {
    // The silent-misread class for this slot: a MAT4 of UNSIGNED_INT is 64
    // bytes exactly like `[[f32; 4]; 4]`, so nothing asserts and every
    // inverse bind would load as the float reading of an integer's bits.
    // The other inverse-bind refusal test uses a VEC4, which exercises only
    // the dimension half of the check.
    expect_unreadable_inverse_bind(
        Primitive::new().inverse_bind("MAT4", UNSIGNED_INT, 1, u32s(&[1; 16])),
        "a MAT4 of UNSIGNED_INT inverse bind",
    );
}

#[test]
fn an_inverse_bind_the_reader_cannot_walk_stays_unreadable() {
    // Layout is judged for this slot too, and with the same answer: a
    // stride-4 view under a 64-byte element panics in `Iter::new` exactly
    // as a vertex attribute would.
    expect_unreadable_inverse_bind(
        Primitive::new()
            .inverse_bind("MAT4", FLOAT, 1, identity_matrix())
            .stride(4),
        "an inverse bind on a stride-4 view",
    );
    expect_unreadable_inverse_bind(
        Primitive::new()
            .inverse_bind("MAT4", FLOAT, 1, identity_matrix())
            .declared_count(u64::MAX),
        "an inverse bind whose byte extent overflows",
    );
    expect_unreadable_inverse_bind(
        Primitive::new()
            .inverse_bind("MAT4", FLOAT, 1, identity_matrix())
            .sparse(Sparse::replacing_first(identity_matrix()).declaring_count(0)),
        "an inverse bind with a sparse block of count 0",
    );
}
