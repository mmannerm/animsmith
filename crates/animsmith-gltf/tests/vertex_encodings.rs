//! A primitive accessor typed for a different element than its `gltf`
//! reader decodes must produce a `LoadError`, never a panic (invariant-1) —
//! and every encoding glTF legitimately permits for a slot the loader reads
//! must still load, with the same values its `FLOAT` equivalent produces.

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
        };
        let positions = primitive.push(Accessor {
            accessor_type: "VEC3",
            component_type: FLOAT,
            count: 3,
            bytes: f32s(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0]),
            bounds: Some((vec![0.0, 0.0, 0.0], vec![1.0, 1.0, 0.0])),
        });
        primitive.attributes.push(("POSITION".into(), positions));
        primitive
    }

    fn push(&mut self, accessor: Accessor) -> usize {
        self.accessors.push(accessor);
        self.accessors.len() - 1
    }

    /// Declare `semantic` with the given encoding, replacing any existing
    /// declaration of it so the default `POSITION` can be re-encoded.
    fn attribute(
        mut self,
        semantic: &str,
        accessor_type: &'static str,
        component_type: u32,
        count: usize,
        bytes: Vec<u8>,
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
        });
        self.inverse_bind = Some(index);
        self
    }

    fn mode(mut self, mode: u32) -> Self {
        self.mode = mode;
        self
    }

    fn to_json(&self) -> Vec<u8> {
        let mut blob = Vec::new();
        let mut views = Vec::new();
        let mut accessors = Vec::new();
        for accessor in &self.accessors {
            while blob.len() % 4 != 0 {
                blob.push(0);
            }
            views.push(json!({
                "buffer": 0,
                "byteOffset": blob.len(),
                "byteLength": accessor.bytes.len()
            }));
            blob.extend_from_slice(&accessor.bytes);
            let mut json = json!({
                "bufferView": views.len() - 1,
                "componentType": accessor.component_type,
                "count": accessor.count,
                "type": accessor.accessor_type
            });
            if let Some((min, max)) = &accessor.bounds {
                json["min"] = json!(min);
                json["max"] = json!(max);
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
        let mut node = json!({ "name": "mesh-node", "mesh": 0 });
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
            "meshes": [{ "primitives": [primitive] }],
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

    fn expect_primitive(&self) -> animsmith_core::model::Primitive {
        let document = self.load().expect("valid encoding must load");
        document.assets.meshes[0].primitives[0].clone()
    }
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
        .attribute(
            "TEXCOORD_0",
            "VEC2",
            UNSIGNED_BYTE,
            3,
            u8s(&[0, 0, 255, 0, 0, 255]),
        )
        .expect_primitive();
    assert_eq!(primitive.uvs, EXPECTED_UVS);
}

#[test]
fn normalized_unsigned_short_tex_coords_load_as_the_float_equivalent() {
    let primitive = Primitive::new()
        .attribute(
            "TEXCOORD_0",
            "VEC2",
            UNSIGNED_SHORT,
            3,
            u16s(&[0, 0, 65535, 0, 0, 65535]),
        )
        .expect_primitive();
    assert_eq!(primitive.uvs, EXPECTED_UVS);
}

/// Joint indices and weights the loader must read identically from either
/// integer joint encoding.
const EXPECTED_JOINTS: [[u16; 4]; 3] = [[0, 1, 2, 3], [1, 2, 3, 0], [2, 3, 0, 1]];
const EXPECTED_WEIGHTS: [[f32; 4]; 3] = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
];

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

const JOINT_VALUES: [u16; 12] = [0, 1, 2, 3, 1, 2, 3, 0, 2, 3, 0, 1];
const WEIGHT_FLOATS: [f32; 12] = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];

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
    let weights = u8s(&WEIGHT_FLOATS.map(|weight| (weight * 255.0) as u8));
    let primitive =
        skinned(UNSIGNED_SHORT, u16s(&JOINT_VALUES), UNSIGNED_BYTE, weights).expect_primitive();
    assert_eq!(primitive.joints, EXPECTED_JOINTS);
    assert_eq!(primitive.weights, EXPECTED_WEIGHTS);
}

#[test]
fn normalized_unsigned_short_weights_load_as_the_float_equivalent() {
    let weights = u16s(&WEIGHT_FLOATS.map(|weight| (weight * 65535.0) as u16));
    let primitive =
        skinned(UNSIGNED_SHORT, u16s(&JOINT_VALUES), UNSIGNED_SHORT, weights).expect_primitive();
    assert_eq!(primitive.joints, EXPECTED_JOINTS);
    assert_eq!(primitive.weights, EXPECTED_WEIGHTS);
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
