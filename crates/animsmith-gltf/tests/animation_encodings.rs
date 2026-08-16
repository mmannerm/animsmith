//! Property-aware animation accessor encoding regressions.
//!
//! `gltf` selects its output iterator from the channel target property, not
//! from the accessor's declared element. These synthetic files therefore
//! prove both sides of the boundary: every encoding the selected reader can
//! legitimately decode still loads, while wrong `type` and
//! `componentType` declarations fail before a reader is constructed.

use animsmith_core::model::{Property, TrackValues};
use animsmith_gltf::fix::FixSession;
use animsmith_gltf::{FixError, GltfScalePreflightError, LoadError, preflight_scale_source_bytes};
use base64::Engine as _;
use serde_json::{Value, json};

const BYTE: u32 = 5120;
const UNSIGNED_BYTE: u32 = 5121;
const SHORT: u32 = 5122;
const UNSIGNED_SHORT: u32 = 5123;
const UNSIGNED_INT: u32 = 5125;
const FLOAT: u32 = 5126;

#[derive(Clone, Copy)]
struct Encoding {
    accessor_type: &'static str,
    component_type: u32,
    normalized: bool,
}

impl Encoding {
    const fn new(accessor_type: &'static str, component_type: u32) -> Self {
        Self {
            accessor_type,
            component_type,
            normalized: false,
        }
    }

    const fn normalized(mut self) -> Self {
        self.normalized = true;
        self
    }

    fn byte_len(self, count: usize) -> usize {
        let components = match self.accessor_type {
            "SCALAR" => 1,
            "VEC2" => 2,
            "VEC3" => 3,
            "VEC4" => 4,
            other => panic!("unsupported synthetic accessor type {other}"),
        };
        let width = match self.component_type {
            BYTE | UNSIGNED_BYTE => 1,
            SHORT | UNSIGNED_SHORT => 2,
            UNSIGNED_INT | FLOAT => 4,
            other => panic!("unsupported synthetic component type {other}"),
        };
        count * components * width
    }
}

struct Clip {
    input: Encoding,
    output: Encoding,
    property: &'static str,
    output_bytes: Option<Vec<u8>>,
}

impl Clip {
    fn rotation(output: Encoding) -> Self {
        Self {
            input: Encoding::new("SCALAR", FLOAT),
            output,
            property: "rotation",
            output_bytes: Some(rotation_bytes(output)),
        }
    }

    fn with_encodings(input: Encoding, output: Encoding, property: &'static str) -> Self {
        Self {
            input,
            output,
            property,
            output_bytes: None,
        }
    }

    fn to_json(&self) -> Vec<u8> {
        let count = 2;
        let input_bytes =
            if self.input.component_type == FLOAT && self.input.accessor_type == "SCALAR" {
                [0.0f32, 1.0]
                    .into_iter()
                    .flat_map(f32::to_le_bytes)
                    .collect()
            } else {
                vec![0; self.input.byte_len(count)]
            };
        let output_bytes = self
            .output_bytes
            .clone()
            .unwrap_or_else(|| vec![0; self.output.byte_len(count)]);

        let mut blob = Vec::new();
        let input_view = push_view(&mut blob, &input_bytes);
        let output_view = push_view(&mut blob, &output_bytes);
        let mut input = accessor(0, self.input, count);
        let component_count = match self.input.accessor_type {
            "SCALAR" => 1,
            "VEC2" => 2,
            "VEC3" => 3,
            "VEC4" => 4,
            _ => unreachable!(),
        };
        input["min"] = json!(vec![0; component_count]);
        input["max"] = json!(vec![1; component_count]);
        let output = accessor(1, self.output, count);

        serde_json::to_vec(&json!({
            "asset": { "version": "2.0" },
            "buffers": [{
                "uri": format!(
                    "data:application/octet-stream;base64,{}",
                    base64::engine::general_purpose::STANDARD.encode(&blob)
                ),
                "byteLength": blob.len()
            }],
            "bufferViews": [input_view, output_view],
            "accessors": [input, output],
            "nodes": [{ "name": "root" }],
            "animations": [{
                "name": "encoding",
                "samplers": [{ "input": 0, "output": 1, "interpolation": "LINEAR" }],
                "channels": [{
                    "sampler": 0,
                    "target": { "node": 0, "path": self.property }
                }]
            }],
            "scenes": [{ "nodes": [0] }],
            "scene": 0
        }))
        .expect("serializes synthetic glTF")
    }

    fn load(&self) -> Result<animsmith_core::model::Document, LoadError> {
        animsmith_gltf::load_bytes(std::path::Path::new("encoding.gltf"), &self.to_json())
    }
}

fn push_view(blob: &mut Vec<u8>, bytes: &[u8]) -> Value {
    while !blob.len().is_multiple_of(4) {
        blob.push(0);
    }
    let offset = blob.len();
    blob.extend_from_slice(bytes);
    json!({
        "buffer": 0,
        "byteOffset": offset,
        "byteLength": bytes.len()
    })
}

fn accessor(view: usize, encoding: Encoding, count: usize) -> Value {
    let mut accessor = json!({
        "bufferView": view,
        "componentType": encoding.component_type,
        "count": count,
        "type": encoding.accessor_type
    });
    if encoding.normalized {
        accessor["normalized"] = json!(true);
    }
    accessor
}

fn rotation_bytes(encoding: Encoding) -> Vec<u8> {
    match encoding.component_type {
        BYTE => [0i8, 0, 0, 127, 0, 0, 0, 127]
            .into_iter()
            .map(|value| value as u8)
            .collect(),
        UNSIGNED_BYTE => vec![0, 0, 0, 255, 0, 0, 0, 255],
        SHORT => [0i16, 0, 0, i16::MAX, 0, 0, 0, i16::MAX]
            .into_iter()
            .flat_map(i16::to_le_bytes)
            .collect(),
        UNSIGNED_SHORT => [0u16, 0, 0, u16::MAX, 0, 0, 0, u16::MAX]
            .into_iter()
            .flat_map(u16::to_le_bytes)
            .collect(),
        FLOAT => [0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect(),
        other => vec![0; Encoding::new("VEC4", other).byte_len(2)],
    }
}

#[test]
fn every_decodable_rotation_output_encoding_still_loads() {
    for (name, encoding) in [
        ("BYTE", Encoding::new("VEC4", BYTE).normalized()),
        (
            "UNSIGNED_BYTE",
            Encoding::new("VEC4", UNSIGNED_BYTE).normalized(),
        ),
        ("SHORT", Encoding::new("VEC4", SHORT).normalized()),
        (
            "UNSIGNED_SHORT",
            Encoding::new("VEC4", UNSIGNED_SHORT).normalized(),
        ),
        ("FLOAT", Encoding::new("VEC4", FLOAT)),
    ] {
        let document = Clip::rotation(encoding)
            .load()
            .unwrap_or_else(|error| panic!("{name} rotation output must load: {error}"));
        let track = &document.clips[0].tracks[0];
        assert_eq!(track.property, Property::Rotation, "{name}");
        assert!(matches!(&track.values, TrackValues::Quats(values) if values.len() == 2));
    }
}

#[test]
fn every_property_selected_float_output_shape_still_loads() {
    for (property, output, expected_property) in [
        (
            "translation",
            Encoding::new("VEC3", FLOAT),
            Some(Property::Translation),
        ),
        ("scale", Encoding::new("VEC3", FLOAT), Some(Property::Scale)),
    ] {
        let document = Clip::with_encodings(Encoding::new("SCALAR", FLOAT), output, property)
            .load()
            .unwrap_or_else(|error| panic!("valid {property} output must load: {error}"));
        match expected_property {
            Some(expected) => assert_eq!(document.clips[0].tracks[0].property, expected),
            None => assert!(
                document.clips[0].tracks.is_empty(),
                "morph weights remain intentionally unmodeled"
            ),
        }
    }
}

#[test]
fn every_decodable_weight_output_encoding_still_loads() {
    for (name, output) in [
        ("BYTE", Encoding::new("SCALAR", BYTE).normalized()),
        (
            "UNSIGNED_BYTE",
            Encoding::new("SCALAR", UNSIGNED_BYTE).normalized(),
        ),
        ("SHORT", Encoding::new("SCALAR", SHORT).normalized()),
        (
            "UNSIGNED_SHORT",
            Encoding::new("SCALAR", UNSIGNED_SHORT).normalized(),
        ),
        ("FLOAT", Encoding::new("SCALAR", FLOAT)),
    ] {
        let document = Clip::with_encodings(Encoding::new("SCALAR", FLOAT), output, "weights")
            .load()
            .unwrap_or_else(|error| panic!("{name} weight output must load: {error}"));
        assert!(
            document.clips[0].tracks.is_empty(),
            "morph weights remain intentionally unmodeled ({name})"
        );
    }
}

#[test]
fn every_sampler_slot_refuses_wrong_type_and_component_type() {
    let scalar_f32 = Encoding::new("SCALAR", FLOAT);
    let vec3_f32 = Encoding::new("VEC3", FLOAT);
    let vec4_f32 = Encoding::new("VEC4", FLOAT);
    let cases = [
        (
            "input wrong type",
            Clip::with_encodings(Encoding::new("VEC3", FLOAT), vec4_f32, "rotation"),
            "rotation",
            "input",
            0,
            "VEC3 of FLOAT",
            "SCALAR of FLOAT",
        ),
        (
            "input wrong component",
            Clip::with_encodings(Encoding::new("SCALAR", UNSIGNED_INT), vec4_f32, "rotation"),
            "rotation",
            "input",
            0,
            "SCALAR of UNSIGNED_INT",
            "SCALAR of FLOAT",
        ),
        (
            "translation wrong type",
            Clip::with_encodings(scalar_f32, Encoding::new("VEC4", FLOAT), "translation"),
            "translation",
            "output",
            1,
            "VEC4 of FLOAT",
            "VEC3 of FLOAT",
        ),
        (
            "translation wrong component",
            Clip::with_encodings(
                scalar_f32,
                Encoding::new("VEC3", UNSIGNED_SHORT).normalized(),
                "translation",
            ),
            "translation",
            "output",
            1,
            "VEC3 of UNSIGNED_SHORT",
            "VEC3 of FLOAT",
        ),
        (
            "rotation wrong type",
            Clip::with_encodings(scalar_f32, vec3_f32, "rotation"),
            "rotation",
            "output",
            1,
            "VEC3 of FLOAT",
            "VEC4 of BYTE, UNSIGNED_BYTE, SHORT, UNSIGNED_SHORT, or FLOAT",
        ),
        (
            "rotation wrong component",
            Clip::with_encodings(scalar_f32, Encoding::new("VEC4", UNSIGNED_INT), "rotation"),
            "rotation",
            "output",
            1,
            "VEC4 of UNSIGNED_INT",
            "VEC4 of BYTE, UNSIGNED_BYTE, SHORT, UNSIGNED_SHORT, or FLOAT",
        ),
        (
            "scale wrong type",
            Clip::with_encodings(scalar_f32, Encoding::new("VEC2", FLOAT), "scale"),
            "scale",
            "output",
            1,
            "VEC2 of FLOAT",
            "VEC3 of FLOAT",
        ),
        (
            "weights wrong component",
            Clip::with_encodings(scalar_f32, Encoding::new("SCALAR", UNSIGNED_INT), "weights"),
            "weights",
            "output",
            1,
            "SCALAR of UNSIGNED_INT",
            "SCALAR of BYTE, UNSIGNED_BYTE, SHORT, UNSIGNED_SHORT, or FLOAT",
        ),
    ];

    for (
        name,
        clip,
        expected_property,
        expected_slot,
        expected_accessor,
        expected_found,
        expected_reader,
    ) in cases
    {
        let error = match clip.load() {
            Ok(_) => panic!("{name} must be refused"),
            Err(error) => error,
        };
        match &error {
            LoadError::AnimationEncoding {
                animation,
                sampler,
                slot,
                node,
                property,
                accessor,
                found,
                expected,
            } => {
                assert_eq!(*animation, 0, "{name}");
                assert_eq!(*sampler, 0, "{name}");
                assert_eq!(*slot, expected_slot, "{name}");
                assert_eq!(*node, 0, "{name}");
                assert_eq!(*property, expected_property, "{name}");
                assert_eq!(*accessor, expected_accessor, "{name}");
                assert_eq!(found, expected_found, "{name}");
                assert_eq!(expected, expected_reader, "{name}");
            }
            other => panic!("{name}: expected AnimationEncoding, got {other:?}"),
        }
    }
}

#[test]
fn a_refusal_names_nonzero_animation_sampler_and_node_indices() {
    let clip = Clip::with_encodings(
        Encoding::new("SCALAR", FLOAT),
        Encoding::new("VEC4", UNSIGNED_INT),
        "rotation",
    );
    let mut document: Value =
        serde_json::from_slice(&clip.to_json()).expect("parses synthetic glTF");

    document["nodes"] = json!([{ "name": "decoy" }, { "name": "target" }]);
    document["scenes"][0]["nodes"] = json!([0, 1]);

    let mut valid_output = document["accessors"][1].clone();
    valid_output["componentType"] = json!(FLOAT);
    document["accessors"]
        .as_array_mut()
        .expect("accessors are an array")
        .push(valid_output);

    document["animations"] = json!([
        {
            "name": "decoy",
            "samplers": [{ "input": 0, "output": 2, "interpolation": "LINEAR" }],
            "channels": [{
                "sampler": 0,
                "target": { "node": 0, "path": "rotation" }
            }]
        },
        {
            "name": "poisoned",
            "samplers": [
                { "input": 0, "output": 2, "interpolation": "LINEAR" },
                { "input": 0, "output": 1, "interpolation": "LINEAR" }
            ],
            "channels": [{
                "sampler": 1,
                "target": { "node": 1, "path": "rotation" }
            }]
        }
    ]);

    let bytes = serde_json::to_vec(&document).expect("serializes synthetic glTF");
    let error = animsmith_gltf::load_bytes(std::path::Path::new("encoding.gltf"), &bytes)
        .expect_err("the nonzero-index sampler must be refused");
    assert!(matches!(
        error,
        LoadError::AnimationEncoding {
            animation: 1,
            sampler: 1,
            slot: "output",
            node: 1,
            property: "rotation",
            accessor: 1,
            ..
        }
    ));
}

#[test]
fn every_public_ingestion_boundary_refuses_the_same_mistyped_sampler() {
    let clip = Clip::with_encodings(
        Encoding::new("SCALAR", FLOAT),
        Encoding::new("VEC4", UNSIGNED_INT),
        "rotation",
    );
    let bytes = clip.to_json();

    let load_error = animsmith_gltf::load_bytes(std::path::Path::new("encoding.gltf"), &bytes)
        .expect_err("the loader must reject the mistyped sampler");
    assert_rotation_output_error(&load_error);

    let directory = tempfile::tempdir().expect("creates temporary directory");
    let input = directory.path().join("encoding.gltf");
    std::fs::write(&input, &bytes).expect("writes synthetic glTF");
    let fix_error = match FixSession::read(&input) {
        Ok(_) => panic!("fix must reject the mistyped sampler"),
        Err(error) => error,
    };
    match fix_error {
        FixError::Load(error) => assert_rotation_output_error(&error),
        other => panic!("expected fix Load error, got {other:?}"),
    }

    let preflight_error = preflight_scale_source_bytes(&input, &bytes)
        .expect_err("scale preflight must reject the mistyped sampler");
    match preflight_error {
        GltfScalePreflightError::Load(error) => assert_rotation_output_error(&error),
        other => panic!("expected preflight Load error, got {other:?}"),
    }
}

fn assert_rotation_output_error(error: &LoadError) {
    assert!(matches!(
        error,
        LoadError::AnimationEncoding {
            animation: 0,
            sampler: 0,
            slot: "output",
            node: 0,
            property: "rotation",
            accessor: 1,
            found,
            expected,
        } if found == "VEC4 of UNSIGNED_INT"
            && expected == "VEC4 of BYTE, UNSIGNED_BYTE, SHORT, UNSIGNED_SHORT, or FLOAT"
    ));
}
