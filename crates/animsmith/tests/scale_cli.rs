//! CLI contract for the `scale` producer.
//!
//! Every test here drives the real binary, because the properties under test
//! are the producer's: which files exist afterwards, what the exit code is,
//! which stream carried the record, and whether two identical invocations
//! produce identical bytes. None of those is observable from inside the
//! library.
//!
//! This file must keep building and passing under `--no-default-features`:
//! `scale` is the first evidence-emitting producer in the minimal binary, and
//! a feature-gated import here would silently drop that coverage.

use animsmith_core::sha256_hex;
#[cfg(feature = "fbx")]
use animsmith_core::{DEPENDENCY_CLOSURE_V1_MAX_EXTERNAL_RESOURCES, model::Property};
use animsmith_testkit::{
    clipless_mesh_scale_rig_glb, nodes_only_scale_rig_glb, oversized_proof_scale_rig_glb,
    rest_bind_scale_rig_glb, rest_bind_scale_rig_gltf, rotation_only_meshless_scale_rig_glb,
    unaffected_bind_scale_rig_glb,
};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const SCALE_EVIDENCE_SCHEMA: &str =
    include_str!("../../../docs/schemas/scale-evidence-v4.schema.json");
const SCALE_EVIDENCE_SCHEMA_ID: &str = "urn:animsmith:schema:scale-evidence:4";
#[cfg(feature = "fbx")]
const FBX_SCALE_EVIDENCE_SCHEMA: &str =
    include_str!("../../../docs/schemas/scale-evidence-v5.schema.json");
#[cfg(feature = "fbx")]
const FBX_SCALE_EVIDENCE_SCHEMA_ID: &str = "urn:animsmith:schema:scale-evidence:5";
#[cfg(feature = "fbx")]
const TINY_PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0xF8, 0xFF, 0xFF, 0x3F,
    0x00, 0x05, 0xFE, 0x02, 0xFE, 0xA7, 0x35, 0x81, 0x84, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E,
    0x44, 0xAE, 0x42, 0x60, 0x82,
];

fn animsmith() -> Command {
    Command::new(env!("CARGO_BIN_EXE_animsmith"))
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// A temporary directory holding the §D.3 case 2 rig as `rig.glb`.
struct Fixture {
    dir: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        Self::with_asset(rest_bind_scale_rig_glb())
    }

    fn with_asset(asset: Vec<u8>) -> Self {
        let dir = tempfile::tempdir().expect("temporary directory");
        std::fs::write(dir.path().join("rig.glb"), asset).expect("writes the rig fixture");
        Self { dir }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    /// `scale rest-bind` on the rig at the declared factor, into `out.glb`
    /// and `out.json`.
    ///
    /// The factor is passed as `--expected-factor=<value>` rather than as two
    /// arguments so that a negative one is not read as a bundle of short
    /// flags: a factor the operation must reject has to reach the operation.
    fn rest_bind_command(&self, expected_factor: &str, format: &str) -> Command {
        let mut command = animsmith();
        command
            .current_dir(self.dir.path())
            .args([
                "scale",
                "rest-bind",
                "rig.glb",
                "-o",
                "out.glb",
                "--source-skin-index",
                "0",
                "--source-root-node-index",
                "0",
            ])
            .arg(format!("--expected-factor={expected_factor}"))
            .args(["--evidence", "out.json", "--format", format]);
        command
    }

    fn rest_bind(&self, expected_factor: &str, format: &str) -> Output {
        self.rest_bind_command(expected_factor, format)
            .output()
            .expect("runs animsmith")
    }

    /// `scale rest-bind` with a stdout nobody is reading.
    ///
    /// The pipe's read end is dropped **before** the child is spawned, so its
    /// stdout has no reader from the moment it exists: the write failure is a
    /// property of the setup rather than a race against how quickly the child
    /// reaches its write.
    fn rest_bind_into_closed_stdout(&self, expected_factor: &str, format: &str) -> Output {
        let (reader, writer) = std::io::pipe().expect("creates a pipe");
        drop(reader);
        self.rest_bind_command(expected_factor, format)
            .stdout(Stdio::from(writer))
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawns animsmith")
            .wait_with_output()
            .expect("waits for animsmith")
    }

    fn whole_document(&self, factor: &str, format: &str) -> Output {
        animsmith()
            .current_dir(self.dir.path())
            .args(["scale", "whole-document", "rig.glb", "-o", "out.glb"])
            .arg(format!("--factor={factor}"))
            .args(["--evidence", "out.json", "--format", format])
            .output()
            .expect("runs animsmith")
    }
}

fn validator() -> jsonschema::Validator {
    let schema: Value = serde_json::from_str(SCALE_EVIDENCE_SCHEMA).expect("valid schema JSON");
    jsonschema::options()
        .build(&schema)
        .expect("scale evidence schema compiles")
}

fn assert_schema_valid(instance: &Value) {
    let validator = validator();
    let errors: Vec<String> = validator
        .iter_errors(instance)
        .map(|error| format!("{}: {error}", error.instance_path()))
        .collect();
    assert!(errors.is_empty(), "schema errors: {}", errors.join("; "));
}

#[cfg(feature = "fbx")]
fn assert_fbx_schema_valid(instance: &Value) {
    let fbx: Value =
        serde_json::from_str(FBX_SCALE_EVIDENCE_SCHEMA).expect("valid FBX schema JSON");
    let gltf: Value = serde_json::from_str(SCALE_EVIDENCE_SCHEMA).expect("valid glTF schema JSON");
    let registry = jsonschema::Registry::new()
        .add(SCALE_EVIDENCE_SCHEMA_ID, &gltf)
        .expect("adds glTF scale schema")
        .prepare()
        .expect("prepares schema registry");
    let validator = jsonschema::options()
        .with_registry(&registry)
        .build(&fbx)
        .expect("FBX scale evidence schema compiles");
    let errors: Vec<String> = validator
        .iter_errors(instance)
        .map(|error| format!("{}: {error}", error.instance_path()))
        .collect();
    assert!(
        errors.is_empty(),
        "FBX schema errors: {}",
        errors.join("; ")
    );
}

fn assert_published_pair_is_bound(fixture: &Fixture) {
    let artifact_path = fixture.path("out.glb");
    let evidence_path = fixture.path("out.json");
    let artifact = std::fs::read(&artifact_path).expect("reads published scale artifact");
    animsmith_gltf::load(&artifact_path).expect("published scale artifact reloads");
    let evidence = read_json(&evidence_path);
    assert_schema_valid(&evidence);
    assert_eq!(evidence["outcome"], "published");
    assert_eq!(
        evidence["result"]["artifact"]["sha256"],
        sha256_hex(&artifact),
        "scale evidence binds the artifact that landed"
    );
    assert_eq!(
        evidence["result"]["artifact"]["bytes"],
        artifact.len() as u64
    );
    assert_eq!(
        evidence["result"]["proof"]["read_back_digest_matches"],
        true
    );
}

fn glb(value: &Value, binary: &[u8]) -> Vec<u8> {
    let mut json = serde_json::to_vec(value).expect("fixture JSON");
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    let mut padded_binary = binary.to_vec();
    while !padded_binary.len().is_multiple_of(4) {
        padded_binary.push(0);
    }
    let total_length = 12 + 8 + json.len() + 8 + padded_binary.len();
    let mut asset = Vec::with_capacity(total_length);
    asset.extend_from_slice(b"glTF");
    asset.extend_from_slice(&2u32.to_le_bytes());
    asset.extend_from_slice(&(total_length as u32).to_le_bytes());
    asset.extend_from_slice(&(json.len() as u32).to_le_bytes());
    asset.extend_from_slice(&0x4e4f_534au32.to_le_bytes());
    asset.extend_from_slice(&json);
    asset.extend_from_slice(&(padded_binary.len() as u32).to_le_bytes());
    asset.extend_from_slice(&0x004e_4942u32.to_le_bytes());
    asset.extend_from_slice(&padded_binary);
    asset
}

fn map_glb_json(asset: Vec<u8>, mutate: impl FnOnce(&mut Value)) -> Vec<u8> {
    let json_length = u32::from_le_bytes(asset[12..16].try_into().unwrap()) as usize;
    let mut value: Value =
        serde_json::from_slice(&asset[20..20 + json_length]).expect("the fixture has a JSON chunk");
    let binary_header = 20 + json_length;
    let binary_length =
        u32::from_le_bytes(asset[binary_header..binary_header + 4].try_into().unwrap()) as usize;
    let binary = &asset[binary_header + 8..binary_header + 8 + binary_length];
    mutate(&mut value);
    glb(&value, binary)
}

fn morph_scale_rig_glb() -> Vec<u8> {
    let floats: [f32; 28] = [
        1.0, 2.0, 3.0, -1.0, 0.5, 4.0, 2.0, -2.0, 0.0, // base POSITION
        0.25, 0.0, -0.5, 1.0, -1.0, 2.0, 0.0, 0.5, 0.25, // morph POSITION
        0.0, 1.0, // key times
        0.25, 0.75, // animated weights
        1.0, 1.0, 1.0, 2.0, 2.0, 2.0, // scale animation
    ];
    let binary: Vec<u8> = floats
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect();
    let value = serde_json::json!({
        "asset": { "version": "2.0" },
        "buffers": [{ "byteLength": binary.len() }],
        "bufferViews": [
            { "buffer": 0, "byteOffset": 0, "byteLength": 36 },
            { "buffer": 0, "byteOffset": 36, "byteLength": 36 },
            { "buffer": 0, "byteOffset": 72, "byteLength": 8 },
            { "buffer": 0, "byteOffset": 80, "byteLength": 8 },
            { "buffer": 0, "byteOffset": 88, "byteLength": 24 }
        ],
        "accessors": [
            { "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3",
              "min": [-1.0, -2.0, 0.0], "max": [2.0, 2.0, 4.0] },
            { "bufferView": 1, "componentType": 5126, "count": 3, "type": "VEC3" },
            { "bufferView": 2, "componentType": 5126, "count": 2, "type": "SCALAR",
              "min": [0.0], "max": [1.0] },
            { "bufferView": 3, "componentType": 5126, "count": 2, "type": "SCALAR" },
            { "bufferView": 4, "componentType": 5126, "count": 2, "type": "VEC3" }
        ],
        "meshes": [
            { "primitives": [{ "attributes": { "POSITION": 0 } }] },
            {
                "weights": [0.25],
                "primitives": [{
                    "attributes": { "POSITION": 0 },
                    "targets": [{ "POSITION": 1 }]
                }]
            }
        ],
        "nodes": [{ "mesh": 0 }, { "mesh": 1, "weights": [0.5] }],
        "scenes": [{ "nodes": [0, 1] }],
        "scene": 0,
        "animations": [
            {
                "samplers": [{ "input": 2, "output": 4, "interpolation": "LINEAR" }],
                "channels": [{ "sampler": 0, "target": { "node": 0, "path": "scale" } }]
            },
            {
                "samplers": [
                    { "input": 2, "output": 4, "interpolation": "LINEAR" },
                    { "input": 2, "output": 3, "interpolation": "LINEAR" }
                ],
                "channels": [
                    { "sampler": 0, "target": { "node": 0, "path": "scale" } },
                    { "sampler": 1, "target": { "node": 1, "path": "weights" } }
                ]
            }
        ]
    });
    glb(&value, &binary)
}

fn rest_bind_scale_rig_with_morphs_glb() -> Vec<u8> {
    map_glb_json(rest_bind_scale_rig_glb(), |value| {
        value["meshes"]
            .as_array_mut()
            .expect("meshes")
            .push(serde_json::json!({
                "weights": [0.25],
                "primitives": [
                    { "attributes": { "POSITION": 0 } },
                    {
                        "attributes": { "POSITION": 0 },
                        "targets": [{ "POSITION": 0 }]
                    }
                ]
            }));
        value["nodes"]
            .as_array_mut()
            .expect("nodes")
            .push(serde_json::json!({ "name": "morph-holder", "mesh": 1, "weights": [0.5] }));
        value["scenes"][0]["nodes"]
            .as_array_mut()
            .expect("scene nodes")
            .push(serde_json::json!(4));
        value["animations"]
            .as_array_mut()
            .expect("animations")
            .push(serde_json::json!({
                "name": "morph-clip",
                "samplers": [
                    { "input": 4, "interpolation": "LINEAR", "output": 5 },
                    { "input": 4, "interpolation": "LINEAR", "output": 4 }
                ],
                "channels": [
                    { "sampler": 0, "target": { "node": 1, "path": "translation" } },
                    { "sampler": 1, "target": { "node": 4, "path": "weights" } }
                ]
            }));
    })
}

fn rest_bind_scale_rig_with_negative_weight_glb(
    vertex_index: usize,
    influence_index: usize,
    weight: f32,
) -> Vec<u8> {
    let mut asset = rest_bind_scale_rig_glb();
    let json_length = u32::from_le_bytes(asset[12..16].try_into().unwrap()) as usize;
    let json: Value = serde_json::from_slice(&asset[20..20 + json_length])
        .expect("the synthetic GLB has a JSON chunk");
    let accessor_index = json["meshes"][0]["primitives"][0]["attributes"]["WEIGHTS_0"]
        .as_u64()
        .expect("the rig has WEIGHTS_0") as usize;
    let accessor = &json["accessors"][accessor_index];
    assert_eq!(accessor["componentType"], 5126, "WEIGHTS_0 is f32");
    assert_eq!(accessor["type"], "VEC4", "WEIGHTS_0 has four influences");
    assert!(
        vertex_index < accessor["count"].as_u64().expect("an accessor count") as usize,
        "requested vertex is in WEIGHTS_0"
    );
    assert!(influence_index < 4, "requested influence is in a VEC4");

    let weight_offset = f32_accessor_component_offset(
        &json,
        json_length,
        accessor_index,
        vertex_index,
        influence_index,
        4,
    );
    asset[weight_offset..weight_offset + size_of::<f32>()].copy_from_slice(&weight.to_le_bytes());
    asset
}

/// Locate one `f32` component through the GLB's accessor and buffer-view
/// metadata. Tests that corrupt a semantic input field should survive fixture
/// repacking rather than depending on a literal BIN offset.
fn f32_accessor_component_offset(
    json: &Value,
    json_length: usize,
    accessor_index: usize,
    element_index: usize,
    component_index: usize,
    component_count: usize,
) -> usize {
    let accessor = &json["accessors"][accessor_index];
    assert_eq!(accessor["componentType"], 5126, "the accessor is f32");
    assert!(
        element_index < accessor["count"].as_u64().expect("an accessor count") as usize,
        "requested element is in the accessor"
    );
    assert!(
        component_index < component_count,
        "requested component is in the accessor element"
    );
    let view_index = accessor["bufferView"]
        .as_u64()
        .expect("the accessor has a buffer view") as usize;
    let view = &json["bufferViews"][view_index];
    assert_eq!(view["buffer"].as_u64(), Some(0), "the GLB has one buffer");
    let view_offset = view["byteOffset"].as_u64().unwrap_or(0) as usize;
    let accessor_offset = accessor["byteOffset"].as_u64().unwrap_or(0) as usize;
    let packed_stride = component_count * size_of::<f32>();
    let stride = view["byteStride"].as_u64().unwrap_or(packed_stride as u64) as usize;
    let bin_payload = 20 + json_length + 8;
    bin_payload
        + view_offset
        + accessor_offset
        + element_index * stride
        + component_index * size_of::<f32>()
}

fn rest_bind_scale_rig_with_non_finite_inverse_bind_glb() -> Vec<u8> {
    let mut asset = rest_bind_scale_rig_glb();
    let json_length = u32::from_le_bytes(asset[12..16].try_into().unwrap()) as usize;
    let json: Value = serde_json::from_slice(&asset[20..20 + json_length])
        .expect("the synthetic GLB has a JSON chunk");
    let accessor_index = json["skins"][0]["inverseBindMatrices"]
        .as_u64()
        .expect("the rig skin has inverse-bind evidence") as usize;
    let accessor = &json["accessors"][accessor_index];
    assert_eq!(accessor["type"], "MAT4", "inverse binds are matrices");
    let offset = f32_accessor_component_offset(&json, json_length, accessor_index, 0, 0, 16);
    asset[offset..offset + size_of::<f32>()].copy_from_slice(&f32::NAN.to_le_bytes());
    asset
}

fn rest_bind_scale_rig_gltf_with_near_zero_joint_rotation() -> Vec<u8> {
    const AUTHORED_ROTATION: &str =
        "[1.7028635168614414e-9,7.312918715030037e-9,-0.10423537343740463,0.9945526719093323]";
    let mut value: Value = serde_json::from_slice(&rest_bind_scale_rig_gltf())
        .expect("the synthetic glTF has a JSON document");
    value["nodes"][1]["rotation"] = Value::String("__AUTHORED_ROTATION__".into());
    String::from_utf8(serde_json::to_vec(&value).expect("fixture JSON serializes"))
        .expect("fixture JSON is UTF-8")
        .replace("\"__AUTHORED_ROTATION__\"", AUTHORED_ROTATION)
        .into_bytes()
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).expect("reads JSON")).expect("valid JSON")
}

// --- Published pairs --------------------------------------------------------

#[test]
fn rest_bind_publishes_a_pair_whose_evidence_names_the_appendix_d6_policy() {
    let fixture = Fixture::new();
    let output = fixture.rest_bind("0.01", "text");

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    assert!(
        stdout(&output).starts_with("wrote out.glb and out.json (rest-bind factor 0.01,"),
        "stdout:\n{}",
        stdout(&output)
    );

    let record = read_json(&fixture.path("out.json"));
    assert_schema_valid(&record);
    assert_eq!(record["schema_version"], 4);
    assert_eq!(record["schema"], SCALE_EVIDENCE_SCHEMA_ID);
    assert_eq!(record["command"], "scale");
    assert_eq!(record["outcome"], "published");
    assert_eq!(record["rejection"], Value::Null);
    assert_eq!(record["operation"]["kind"], "rest-bind");
    assert_eq!(record["operation"]["source_skin_index"], 0);
    assert_eq!(record["operation"]["source_root_node_index"], 0);

    // Declared paths, verbatim: the canonical forms the distinctness check
    // computes are never serialized.
    assert_eq!(record["paths"]["input"], "rig.glb");
    assert_eq!(record["paths"]["output"], "out.glb");
    assert_eq!(record["paths"]["evidence"], "out.json");

    // The affected closure in raw source-node space: the scaled root, its
    // joint, and the transform-only attachment. Node 3, the mesh holder, is
    // outside it. Skin 0's only joint is node 1, which is inside.
    let result = &record["result"];
    assert_eq!(
        result["affected"]["source_nodes"],
        serde_json::json!([0, 1, 2])
    );
    assert_eq!(result["affected"]["source_skins"], serde_json::json!([0]));
    assert_eq!(result["affected"]["transform_only_attachment_count"], 1);

    // Exactly the §D.2 node members this operation rewrites: the root's
    // absorbed local scale, and the two descendant local translations.
    assert_eq!(
        result["artifact"]["rewritten_json_pointers"],
        serde_json::json!([
            "/nodes/0/scale",
            "/nodes/1/translation",
            "/nodes/2/translation"
        ])
    );
    // The inverse-bind accessor and the affected translation output.
    assert_eq!(
        result["artifact"]["rewritten_accessors"],
        serde_json::json!([3, 5])
    );
    // The artifact proof's own count of that same set. The two halves are
    // derived independently — the list is what the artifact reports it
    // rewrote, the count is what `prove_rewritten_rest_bind` re-derived from
    // the source — and a record whose count disagrees with its own list is
    // false whichever half is wrong, so they are cross-checked against each
    // other as well as against the hand-known two.
    assert_eq!(result["proof"]["artifact"]["rewritten_accessor_count"], 2);
    assert_eq!(
        result["proof"]["artifact"]["rewritten_accessor_count"]
            .as_u64()
            .expect("a count"),
        result["artifact"]["rewritten_accessors"]
            .as_array()
            .expect("a list")
            .len() as u64,
        "the count and the list must describe one set"
    );
    // The maximal buffer ranges *outside* the rewritten accessors that were
    // re-read and found byte-identical. The rig's single 412-byte buffer
    // lays its accessors out from zero, in accessor order:
    //
    //   0..36    POSITION         accessor 0   not rewritten by rest/bind
    //   36..60   JOINTS_0         accessor 1   not rewritten
    //   60..108  WEIGHTS_0        accessor 2   not rewritten
    //   108..172 inverse bind     accessor 3   REWRITTEN
    //   172..180 key times        accessor 4   not rewritten
    //   180..204 translation out  accessor 5   REWRITTEN
    //   204..252 unreferenced
    //   252..284 rotation out     accessor 6   not rewritten
    //   284..412 unreferenced
    //
    // Deleting the two rewritten spans from `0..412` leaves exactly three
    // maximal gaps — `0..108`, `172..180` and `204..412`. Non-zero for a
    // structural reason and not by luck: a rewrite that spilled outside its
    // two accessors would merge or lose gaps, and one that claimed the whole
    // buffer would report zero.
    assert_eq!(result["proof"]["artifact"]["preserved_byte_ranges"], 3);
    assert_eq!(result["artifact"]["container"], "glb");
    assert_eq!(result["proof"]["read_back_digest_matches"], true);

    // Publication is only reachable through a preflight that inventoried the
    // source, so a published record always carries the raw #280 manifest. A
    // `null` here would describe a run that cannot have happened.
    assert_eq!(record["capability"]["container"], "glb");
    assert_eq!(record["capability"]["camera_count"], 0);

    // §D.6's proof coverage. The rig's clip declares two authored key times
    // and both its samplers are `LINEAR`, so there are no cubic-segment
    // interior times to add: two sample times, counted once each.
    assert_eq!(result["proof"]["sample_time_count"], 2);

    // The same count reaches the human-readable summary. The record above is
    // the frozen contract, but the summary is what an operator reads without
    // parsing anything, and it is rendered from a separate argument — an
    // audit mutated that argument and the record's assertion did not see it.
    let summary = stdout(&output);
    assert!(
        summary.contains("2 proof sample time(s)"),
        "summary should carry the sample-time count, got:\n{summary}"
    );

    // The published digest and byte count describe the file that landed.
    let published = std::fs::read(fixture.path("out.glb")).expect("reads the artifact");
    assert_eq!(
        result["artifact"]["bytes"].as_u64(),
        Some(published.len() as u64)
    );

    // §D.6 wants both observed-factor witnesses. The declared factor is the
    // exact `f64` decimal `0.01`; both witnesses are the `f32` the loader
    // read for the root's authored `0.01`, which is a different number.
    assert_eq!(result["factors"]["declared"], 0.01);
    let observed = f64::from(0.01f32);
    assert_eq!(result["factors"]["planned_observed"], observed);
    assert_eq!(result["factors"]["proved_observed"], observed);
    assert_eq!(
        result["factors"]["divergence_ceiling"], 7.103515625e-5,
        "the `appendix-d-v6` common-factor band plus its unit-scale postcondition"
    );

    assert_eq!(result["tolerance"]["policy_id"], "appendix-d-v6");
    // Published rather than implied under the v6 identity: a
    // consumer reading two records with the same policy id must be able to
    // see that they allowed the same `f32` rounding.
    assert_eq!(result["tolerance"]["f32_rounding_ulps"], 4);

    // Two of the three residuals `prove_scale` evaluates unconditionally,
    // whose `evaluated` is the proof's own comparison count rather than any
    // predicate re-derived here. The rig has animation tracks, so the
    // per-element comparison ran; its only skinned instance has its joint
    // *inside* the closure, so the outside-the-closure bind comparison had
    // nothing to walk and reports an absence rather than a zero.
    let residuals = &result["proof"]["residuals"];
    assert_eq!(residuals["track_value"]["evaluated"], true);
    assert_eq!(residuals["track_value"]["max"], 0.0);
    assert_eq!(residuals["unaffected_inverse_bind"]["evaluated"], false);
    assert_eq!(residuals["unaffected_inverse_bind"]["max"], Value::Null);

    assert_eq!(
        result["domain_rewrites"],
        serde_json::json!({
            "rest_hierarchy": true,
            "translation_animation": true,
            "scale_animation": true,
            "inverse_binds": true,
            "base_mesh_positions": false
        }),
        "the reparameterization leaves base mesh POSITION alone: the vertices \
         are already authored in the correct world space"
    );
}

#[test]
fn whole_document_v4_evidence_publishes_every_morph_weight_location() {
    let fixture = Fixture::with_asset(morph_scale_rig_glb());
    let output = fixture.whole_document("2", "json");

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    let record = read_json(&fixture.path("out.json"));
    assert_schema_valid(&record);
    assert_eq!(record["schema_version"], 4);
    assert_eq!(record["schema"], SCALE_EVIDENCE_SCHEMA_ID);
    assert_eq!(
        record["capability"]["morph_weight_locations"],
        serde_json::json!([
            "/animations/1/channels/1/target/path",
            "/meshes/1/weights",
            "/nodes/1/weights"
        ])
    );
    assert_eq!(
        record["capability"]["primitives"][1]["morph_position_accessors"],
        serde_json::json!([1])
    );
}

#[test]
fn factor_one_morph_conversion_is_a_byte_exact_no_write() {
    let source = morph_scale_rig_glb();
    let fixture = Fixture::with_asset(source.clone());
    let output = fixture.whole_document("1", "json");

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    assert_eq!(
        std::fs::read(fixture.path("out.glb")).unwrap(),
        source,
        "q = 1 must not re-encode even a morph-bearing source"
    );
    let record = read_json(&fixture.path("out.json"));
    assert_schema_valid(&record);
    assert_eq!(
        record["result"]["artifact"]["rewritten_accessors"],
        serde_json::json!([])
    );
    assert_eq!(
        record["result"]["artifact"]["rewritten_json_pointers"],
        serde_json::json!([])
    );
    assert_eq!(
        record["result"]["artifact"]["reencoded_buffers"],
        serde_json::json!([])
    );
    assert_eq!(
        record["result"]["proof"]["artifact"]["rewritten_accessor_count"],
        0
    );
}

#[test]
fn a_morph_position_count_mismatch_is_a_public_atomic_refusal() {
    let source = map_glb_json(morph_scale_rig_glb(), |value| {
        value["accessors"][1]["count"] = serde_json::json!(2);
    });
    let fixture = Fixture::with_asset(source);
    let output = fixture.whole_document("2", "json");

    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    assert!(!fixture.path("out.glb").exists());
    assert!(!fixture.path("out.json").exists());
    let record: Value = serde_json::from_str(&stdout(&output)).expect("one refusal record");
    assert_schema_valid(&record);
    assert_eq!(record["rejection"]["stage"], "rewrite");
    assert_eq!(record["rejection"]["kind"], "plan-document-mismatch");
    assert!(
        record["rejection"]["detail"]
            .as_str()
            .expect("detail")
            .contains("morph_position_count_mismatch")
    );
}

#[test]
fn whole_document_publishes_the_exact_binary32_narrowing_residuals_the_factor_costs() {
    // Hand-computed, not observed. The conversion narrows each product to
    // `f32` exactly once, so the residual against the `f64` expectation is
    // the narrowing error of that one rounding:
    //
    //   f32(0.01)                    = 0.00999999977648258209228515625
    //   f64(0.01)                    = 0.01000000000000000020816681711721685...
    //   |f32(0.01) - 0.01|           = 2.2351741811588166e-10
    //
    // That is the largest per-element error any rewritten element in this rig
    // takes, because `1.0` is the largest source magnitude whose product with
    // `0.01` is not exactly representable: the rig's other scale-bearing
    // values are `100`, `300` and `-100` (whose products `1`, `3`, `-1` are
    // exact) and the mesh's own `0.5`, `1.25`, `0.75` and `0.25` (whose
    // products carry strictly smaller absolute error, the largest of them
    // being 1.862645142292063e-10 at `1.25`).
    const NARROWING_AT_ONE: f64 = 2.2351741811588166e-10;
    // `mesh_position` is a per-vertex L2 norm rather than a per-component
    // error, so its maximum is at the vertex with three inexact components:
    //   (-0.5, 0.75, 0.5) -> sqrt(1.1175870905794083e-10^2
    //                           + 1.6763806315323038e-10^2
    //                           + 1.1175870905794083e-10^2)
    const MESH_POSITION_MAX: f64 = 2.3039648069873242e-10;

    let fixture = Fixture::new();
    let output = fixture.whole_document("0.01", "json");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&output)
    );

    let record: Value = serde_json::from_str(&stdout(&output)).expect("stdout is one JSON record");
    assert_schema_valid(&record);
    let result = &record["result"];

    assert_eq!(
        result["proof"]["artifact"]["length_factor_residual"]
            .as_f64()
            .expect("a finite residual"),
        NARROWING_AT_ONE
    );
    assert_eq!(
        result["proof"]["residuals"]["track_value"]["max"]
            .as_f64()
            .expect("a finite residual"),
        0.0,
        "this rig's translation values scale to exact binary32 integers and its rotation stays exact"
    );
    assert_eq!(
        result["proof"]["residuals"]["mesh_position"]["max"]
            .as_f64()
            .expect("a finite residual"),
        MESH_POSITION_MAX
    );
    // Both literals above are non-zero, so this fixture shows the residual
    // pipeline reporting a genuine measurement rather than only the zeros an
    // exactly-representable factor produces. `evaluated` is the other half of
    // that claim: a `max` of `null` here would mean nothing was checked.
    assert_eq!(
        result["proof"]["residuals"]["mesh_position"]["evaluated"],
        true
    );

    // The other three artifact-proof fields, all hand-derived from the rig.
    //
    // A whole-document conversion rewrites exactly the accessors §D.4's
    // domain table names as scale-bearing: the mesh's `POSITION` (accessor
    // 0), the skin's `inverseBindMatrices` translation column (accessor 3),
    // and the output of the one sampler whose channel targets `translation`
    // (accessor 5). `JOINTS_0`, `WEIGHTS_0`, the key times and the rotation
    // output carry no length. That is three, and it must equal the length of
    // the list the artifact reports on the other side of the record.
    assert_eq!(
        result["artifact"]["rewritten_accessors"],
        serde_json::json!([0, 3, 5])
    );
    assert_eq!(result["proof"]["artifact"]["rewritten_accessor_count"], 3);
    assert_eq!(
        result["proof"]["artifact"]["rewritten_accessor_count"]
            .as_u64()
            .expect("a count"),
        result["artifact"]["rewritten_accessors"]
            .as_array()
            .expect("a list")
            .len() as u64,
        "the count and the list must describe one set"
    );
    // Those three occupy `0..36`, `108..172` and `180..204` of the rig's
    // single 412-byte buffer, so deleting them leaves three maximal gaps —
    // `36..108`, `172..180` and `204..412` — each re-read and found
    // byte-identical. Three rather than the rest/bind run's three by a
    // different partition: this operation also rewrites `POSITION` at the
    // buffer's head, which closes the gap `0..108` and opens `36..108`.
    assert_eq!(result["proof"]["artifact"]["preserved_byte_ranges"], 3);
    // `dimensionless_residual` is the maximum drift across values that live
    // *inside* a rewritten range and must nevertheless come through
    // unchanged. This rig declares none: it has no `matrix` node (whose 3x3
    // and homogeneous row would be dimensionless under a unit conversion),
    // and the only rewritten accessor carrying `min`/`max` is `POSITION`,
    // every component of which scales. So the maximum is taken over an empty
    // set and the published value is an exact `0.0` — an absence, not a
    // measurement, and the reason no fixture in this file can make it
    // non-zero is that each such element is separately required to be
    // bit-identical rather than merely close.
    assert_eq!(
        result["proof"]["artifact"]["dimensionless_residual"]
            .as_f64()
            .expect("a finite residual"),
        0.0
    );

    // Whole-document conversion's closure is every node and every skin.
    assert_eq!(
        result["affected"]["source_nodes"],
        serde_json::json!([0, 1, 2, 3])
    );
    assert_eq!(result["affected"]["source_skins"], serde_json::json!([0]));
    assert_eq!(
        result["domain_rewrites"],
        serde_json::json!({
            "rest_hierarchy": true,
            "translation_animation": true,
            "scale_animation": false,
            "inverse_binds": true,
            "base_mesh_positions": true
        }),
        "whole-document conversion rewrites every length domain while preserving \
         dimensionless scale channels"
    );
    // §D.7: this operation's factor has no measurable source counterpart, so
    // both witnesses are the declared factor and they cannot diverge.
    assert_eq!(result["factors"]["planned_observed"], 0.01);
    assert_eq!(result["factors"]["proved_observed"], 0.01);
    assert_eq!(result["factors"]["divergence"], 0.0);
}

#[test]
fn factor_one_evidence_reports_the_empty_compiled_write_set() {
    let fixture = Fixture::new();
    let output = fixture.whole_document("1", "json");

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&output)
    );
    let record = read_json(&fixture.path("out.json"));
    assert_schema_valid(&record);
    let result = &record["result"];
    assert_eq!(
        result["artifact"]["rewritten_accessors"],
        serde_json::json!([])
    );
    assert_eq!(
        result["artifact"]["rewritten_json_pointers"],
        serde_json::json!([])
    );
    assert_eq!(
        result["artifact"]["reencoded_buffers"],
        serde_json::json!([])
    );
    assert_eq!(result["proof"]["artifact"]["rewritten_accessor_count"], 0);
}

#[test]
fn the_json_gltf_container_publishes_the_same_rewrite_and_records_its_container() {
    // The same rig payload, in the other container. `.gltf` is the case where
    // the single buffer is a base64 data URI, so the artifact re-encodes it —
    // which a GLB, whose payload is the BIN chunk, never does.
    let fixture = Fixture::new();
    std::fs::write(fixture.path("rig.gltf"), rest_bind_scale_rig_gltf()).unwrap();
    let output = animsmith()
        .current_dir(fixture.dir.path())
        .args([
            "scale",
            "rest-bind",
            "rig.gltf",
            "-o",
            "out.gltf",
            "--source-skin-index",
            "0",
            "--source-root-node-index",
            "0",
            "--expected-factor",
            "0.01",
            "--evidence",
            "out.json",
        ])
        .output()
        .expect("runs animsmith");

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&output)
    );
    let result = read_json(&fixture.path("out.json"))["result"].clone();
    assert_eq!(result["artifact"]["container"], "gltf");
    assert_eq!(
        result["artifact"]["reencoded_buffers"],
        serde_json::json!([0]),
        "the data-URI buffer is re-encoded; a GLB BIN chunk is not"
    );
    // The same §D.2 node members as the GLB run: the operation is a property
    // of the document, not of how it was packaged.
    assert_eq!(
        result["artifact"]["rewritten_json_pointers"],
        serde_json::json!([
            "/nodes/0/scale",
            "/nodes/1/translation",
            "/nodes/2/translation"
        ])
    );
    // The published artifact is JSON glTF, not a GLB.
    let published = std::fs::read(fixture.path("out.gltf")).unwrap();
    assert!(
        !published.starts_with(b"glTF"),
        "the container is preserved"
    );
    serde_json::from_slice::<Value>(&published).expect("the artifact is JSON glTF");
}

#[test]
fn rest_bind_cli_preserves_near_zero_joint_rotation_components_exactly() {
    const AUTHORED_X: f64 = 1.7028635168614414e-9;
    let fixture = Fixture::new();
    std::fs::write(
        fixture.path("near-zero.gltf"),
        rest_bind_scale_rig_gltf_with_near_zero_joint_rotation(),
    )
    .expect("writes the synthetic raw JSON fixture");

    let output = rest_bind_paths(
        &fixture,
        "near-zero.gltf",
        "near-zero-out.gltf",
        "near-zero-evidence.json",
    );

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&output)
    );
    let artifact = read_json(&fixture.path("near-zero-out.gltf"));
    assert_eq!(
        artifact["nodes"][0]["scale"],
        serde_json::json!([1.0, 1.0, 1.0]),
        "the published artifact contains the rest/bind root-scale rewrite"
    );
    assert_eq!(
        artifact["nodes"][1]["translation"],
        serde_json::json!([0.0, 1.0, 0.0]),
        "the published artifact contains the rest/bind joint-translation rewrite"
    );
    assert_eq!(
        artifact["nodes"][1]["rotation"][0]
            .as_f64()
            .expect("rotation component is numeric")
            .to_bits(),
        AUTHORED_X.to_bits()
    );
    let evidence = read_json(&fixture.path("near-zero-evidence.json"));
    assert_eq!(
        evidence["result"]["artifact"]["rewritten_json_pointers"],
        serde_json::json!([
            "/nodes/0/scale",
            "/nodes/1/translation",
            "/nodes/2/translation"
        ]),
        "the rest/bind conversion runs while rotation remains outside its write set"
    );
    assert_eq!(
        evidence["result"]["artifact"]["reencoded_buffers"],
        serde_json::json!([0]),
        "the scale-bearing data-URI buffer is re-encoded"
    );
}

#[test]
fn an_unevaluated_obligation_publishes_null_rather_than_a_zero_residual() {
    let fixture = Fixture::new();
    assert_eq!(
        fixture.whole_document("0.01", "text").status.code(),
        Some(0)
    );
    let residuals = read_json(&fixture.path("out.json"))["result"]["proof"]["residuals"].clone();

    // A whole-document conversion declares no rest/bind postcondition and no
    // transform-only attachment, and the rig's only animation is `LINEAR`, so
    // it has no cubic-segment interior time. Reporting `0.0` for any of the
    // three would be a record of a claim nothing checked.
    for claim in ["unit_scale", "transform_only_affine", "cubic_interior"] {
        assert_eq!(residuals[claim]["evaluated"], false, "{claim}");
        assert_eq!(residuals[claim]["max"], Value::Null, "{claim}");
    }
    // And the counterpart: a claim that *was* evaluated reports a number,
    // including when that number is a checked zero.
    assert_eq!(residuals["skin_matrix"]["evaluated"], true);
    assert_eq!(residuals["skin_matrix"]["max"], 0.0);
    assert_eq!(residuals["key_translation"]["evaluated"], true);
    assert_eq!(residuals["key_translation"]["max"], 0.0);
}

#[test]
fn clipless_mesh_evidence_keeps_each_residual_with_its_own_evaluation() {
    let fixture = Fixture::with_asset(clipless_mesh_scale_rig_glb());
    let run = fixture.rest_bind("0.01", "text");
    assert_eq!(run.status.code(), Some(0), "stderr:\n{}", stderr(&run));

    let record = read_json(&fixture.path("out.json"));
    assert_schema_valid(&record);
    let result = &record["result"];
    let residuals = &result["proof"]["residuals"];
    for claim in ["mesh_position", "bounds", "unit_scale"] {
        assert_eq!(residuals[claim]["evaluated"], true, "{claim}");
        assert_eq!(residuals[claim]["max"], 0.0, "{claim}");
    }
    for claim in ["track_value", "trajectory", "transform_only_affine"] {
        assert_eq!(residuals[claim]["evaluated"], false, "{claim}");
        assert_eq!(residuals[claim]["max"], Value::Null, "{claim}");
    }
    assert_eq!(result["proof"]["sample_time_count"], 0);
    assert_eq!(result["affected"]["transform_only_attachment_count"], 0);
}

#[test]
fn rotation_only_meshless_evidence_keeps_each_residual_with_its_own_evaluation() {
    let fixture = Fixture::with_asset(rotation_only_meshless_scale_rig_glb());
    let run = fixture.whole_document("0.01", "text");
    assert_eq!(run.status.code(), Some(0), "stderr:\n{}", stderr(&run));

    let record = read_json(&fixture.path("out.json"));
    assert_schema_valid(&record);
    let result = &record["result"];
    let residuals = &result["proof"]["residuals"];
    for claim in ["track_value", "trajectory"] {
        assert_eq!(residuals[claim]["evaluated"], true, "{claim}");
        assert!(
            residuals[claim]["max"].as_f64().is_some(),
            "{claim} must publish its measured maximum"
        );
    }
    for claim in ["mesh_position", "key_translation", "bounds"] {
        assert_eq!(residuals[claim]["evaluated"], false, "{claim}");
        assert_eq!(residuals[claim]["max"], Value::Null, "{claim}");
    }
    assert_eq!(result["proof"]["sample_time_count"], 2);
}

#[test]
fn a_rig_with_no_meshes_and_no_clips_reports_absences_for_what_it_cannot_evaluate() {
    // Three of the twelve residuals are `evaluated` on every other fixture
    // here, so nothing distinguishes them from a constant `true`:
    // `track_value` and `mesh_position`, which `prove_scale` compares
    // unconditionally, and the rest obligation behind `rest_translation` and
    // `rest_rotation`. This rig has no mesh and no clip, which separates
    // them: the first two have nothing to walk — and their published
    // comparison counts are zero — while the rest obligation is still
    // declared and still measured.
    let fixture = Fixture::new();
    std::fs::write(fixture.path("nodes.glb"), nodes_only_scale_rig_glb()).unwrap();
    let run = rest_bind_paths(&fixture, "nodes.glb", "out.glb", "out.json");
    assert_eq!(run.status.code(), Some(0), "stderr:\n{}", stderr(&run));

    let record = read_json(&fixture.path("out.json"));
    assert_schema_valid(&record);
    let residuals = &record["result"]["proof"]["residuals"];
    for claim in ["track_value", "mesh_position"] {
        assert_eq!(residuals[claim]["evaluated"], false, "{claim}");
        assert_eq!(residuals[claim]["max"], Value::Null, "{claim}");
    }
    // The rest obligation is a property of the *operation*, not of the
    // payloads the other two read, so removing the clip must not silence it.
    for claim in ["rest_translation", "rest_rotation"] {
        assert_eq!(residuals[claim]["evaluated"], true, "{claim}");
        // An exact zero, not an approximation: both sides of the comparison
        // reach each world rest position through the same `f32` products in
        // the same order — the source multiplies the parent's authored
        // `0.01` in, the candidate multiplies the same narrowed factor into
        // the child's own translation — so the difference is bit-for-bit
        // nothing.
        assert_eq!(residuals[claim]["max"], 0.0, "{claim}");
    }
    // With no clip there is nothing to sample, and the count says so rather
    // than being absent.
    assert_eq!(record["result"]["proof"]["sample_time_count"], 0);
    // The immutable evidence booleans describe the operation's domain
    // contract, not which payloads this particular document happened to
    // carry. In particular, removing every clip must not turn the operation's
    // translation/scale-animation dispositions into `false`.
    assert_eq!(
        record["result"]["domain_rewrites"],
        serde_json::json!({
            "rest_hierarchy": true,
            "translation_animation": true,
            "scale_animation": true,
            "inverse_binds": true,
            "base_mesh_positions": false
        })
    );
}

#[test]
fn a_skin_outside_the_closure_publishes_a_measured_unaffected_inverse_bind() {
    // The `unaffected_inverse_bind` claim is the one residual that asserts
    // what the rewrite did *not* touch, and on the §D.3 case 2 rig it reports
    // an absence: the only skinned instance's joint is inside the affected
    // closure. This rig adds a second instance of the same mesh under its own
    // skin, whose joint is outside, which is the only shape where the claim
    // has something to walk — and therefore the only shape that can tell a
    // measured `0.0` from the fabricated one §D.6 forbids.
    let fixture = Fixture::new();
    std::fs::write(fixture.path("two.glb"), unaffected_bind_scale_rig_glb()).unwrap();
    let run = rest_bind_paths(&fixture, "two.glb", "out.glb", "out.json");
    assert_eq!(run.status.code(), Some(0), "stderr:\n{}", stderr(&run));

    let record = read_json(&fixture.path("out.json"));
    assert_schema_valid(&record);
    let result = &record["result"];
    assert_eq!(
        result["proof"]["residuals"]["unaffected_inverse_bind"]["evaluated"],
        true
    );
    // Skin 1's bind is the identity and node 4 carries no transform, so
    // `W(rest) * B = I` on both sides of the rewrite. The rewrite must leave
    // both exactly as it found them, which makes the checked residual an
    // exact zero rather than a narrowing error.
    assert_eq!(
        result["proof"]["residuals"]["unaffected_inverse_bind"]["max"],
        0.0
    );
    // And the second skin really is outside the closure: the operation
    // rewrote the same three §D.2 node members and the same two accessors as
    // it does without it.
    assert_eq!(
        result["affected"]["source_nodes"],
        serde_json::json!([0, 1, 2])
    );
    assert_eq!(result["affected"]["source_skins"], serde_json::json!([0]));
    assert_eq!(
        result["artifact"]["rewritten_accessors"],
        serde_json::json!([3, 5]),
        "accessor 7 is skin 1's own inverse bind and must not be rewritten"
    );
}

#[test]
fn identical_inputs_and_arguments_produce_byte_identical_artifact_and_evidence() {
    let first = Fixture::new();
    let second = Fixture::new();
    assert_eq!(first.rest_bind("0.01", "text").status.code(), Some(0));
    assert_eq!(second.rest_bind("0.01", "text").status.code(), Some(0));

    // Different temporary directories, so any absolute host path or
    // canonicalized form leaking into the record would show up here — as
    // would a timestamp.
    assert_ne!(first.dir.path(), second.dir.path());
    assert_eq!(
        std::fs::read(first.path("out.glb")).unwrap(),
        std::fs::read(second.path("out.glb")).unwrap(),
    );
    assert_eq!(
        std::fs::read(first.path("out.json")).unwrap(),
        std::fs::read(second.path("out.json")).unwrap(),
    );
}

/// Under `--format json` the published evidence and stdout are the same
/// bytes, because the record is serialized once and both destinations
/// receive that `Vec<u8>`.
///
/// This guards **drift**, not the single serialization itself: routing stdout
/// back through a second serializer would still pass here, because two
/// serializers over one record agree today. The property is the construction;
/// this test is what notices if the two destinations ever stop matching.
#[test]
fn the_published_evidence_and_the_json_stream_are_the_same_bytes() {
    let fixture = Fixture::new();
    let output = fixture.rest_bind("0.01", "json");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&output)
    );
    assert_eq!(
        output.stdout,
        std::fs::read(fixture.path("out.json")).unwrap(),
    );
}

// --- Refusals ---------------------------------------------------------------

#[test]
fn a_refused_run_publishes_nothing_and_leaves_a_prior_pair_byte_identical() {
    let fixture = Fixture::new();
    std::fs::write(fixture.path("out.glb"), b"previous artifact").unwrap();
    std::fs::write(fixture.path("out.json"), b"previous evidence").unwrap();

    // The rig's root is authored at `0.01`; declaring `0.02` is a fact about
    // the source, not a typo the tool can repair.
    let output = fixture.rest_bind("0.02", "json");

    assert_eq!(
        output.status.code(),
        Some(1),
        "stderr:\n{}",
        stderr(&output)
    );
    assert_eq!(
        std::fs::read(fixture.path("out.glb")).unwrap(),
        b"previous artifact"
    );
    assert_eq!(
        std::fs::read(fixture.path("out.json")).unwrap(),
        b"previous evidence"
    );

    let record: Value = serde_json::from_str(&stdout(&output)).expect("stdout is one JSON record");
    assert_schema_valid(&record);
    assert_eq!(record["outcome"], "rejected");
    assert_eq!(record["result"], Value::Null);
    assert_eq!(record["rejection"]["stage"], "plan");
    assert_eq!(record["rejection"]["kind"], "factor-mismatch");
    assert_eq!(record["rejection"]["violations"], serde_json::json!([]));
    assert_eq!(
        record["rejection"]["artifact_proof_differences"],
        Value::Null
    );
    // A refusal still identifies the input it refused and inventories it.
    assert!(record["input"]["sha256"].is_string());
    assert_eq!(record["capability"]["container"], "glb");
}

#[test]
fn scale_evidence_v4_schema_pins_artifact_proof_difference_shape() {
    // Start with a real refusal so the fixture remains a full producer
    // record, then replace only the nullable diagnostic payload.
    let fixture = Fixture::new();
    let output = fixture.rest_bind("0.02", "json");
    assert_eq!(
        output.status.code(),
        Some(1),
        "stderr:\n{}",
        stderr(&output)
    );
    let mut record: Value =
        serde_json::from_str(&stdout(&output)).expect("stdout is one JSON record");

    record["rejection"]["artifact_proof_differences"] = serde_json::json!({
        "omitted": 1,
        "items": (0..16)
            .map(|index| serde_json::json!({
                "location": format!("/nodes/{index}"),
                "kind": match index % 3 {
                    0 => "value_changed",
                    1 => "artifact_added",
                    _ => "artifact_removed",
                }
            }))
            .collect::<Vec<_>>()
    });
    record["rejection"]["kind"] = serde_json::json!("artifact-proof-failed");
    assert_schema_valid(&record);

    let mut untruncated = record.clone();
    untruncated["rejection"]["artifact_proof_differences"] = serde_json::json!({
        "omitted": 0,
        "items": [
            { "location": "", "kind": "value_changed" },
            { "location": "/a~0~1b~0", "kind": "artifact_added" }
        ]
    });
    assert_schema_valid(&untruncated);

    for invalid in [
        serde_json::json!({
            "omitted": 0,
            "items": [{ "location": "/nodes/1", "kind": "value_changed" }],
            "total": 1
        }),
        serde_json::json!({ "omitted": 0, "items": [] }),
        serde_json::json!({
            "omitted": 0,
            "items": [{ "location": "/nodes/1", "kind": "unsupported" }]
        }),
        serde_json::json!({
            "omitted": 0,
            "items": [{ "location": "nodes/1", "kind": "value_changed" }]
        }),
        serde_json::json!({
            "omitted": 0,
            "items": [{ "location": "/bad~2escape", "kind": "value_changed" }]
        }),
        serde_json::json!({
            "omitted": 1,
            "items": (0..15)
                .map(|index| serde_json::json!({
                    "location": format!("/nodes/{index}"),
                    "kind": "artifact_removed"
                }))
                .collect::<Vec<_>>()
        }),
        serde_json::json!({
            "omitted": 1,
            "items": (0..17)
                .map(|index| serde_json::json!({
                    "location": format!("/nodes/{index}"),
                    "kind": "artifact_removed"
                }))
                .collect::<Vec<_>>()
        }),
    ] {
        let mut malformed = record.clone();
        malformed["rejection"]["artifact_proof_differences"] = invalid;
        let schema = validator();
        let errors: Vec<_> = schema.iter_errors(&malformed).collect();
        assert!(!errors.is_empty(), "{malformed}");
    }

    let mut wrong_kind = record;
    wrong_kind["rejection"]["kind"] = serde_json::json!("factor-mismatch");
    let schema = validator();
    let errors: Vec<_> = schema.iter_errors(&wrong_kind).collect();
    assert!(
        !errors.is_empty(),
        "only artifact-proof-failed may carry raw JSON differences"
    );
}

#[test]
fn a_negative_primary_skin_weight_is_a_typed_plan_refusal_and_publishes_nothing() {
    // The smallest finite value below zero catches a thresholded check such
    // as `weight <= -0.5` while the last vertex/influence catches a partial
    // scan of the VEC4 accessor.
    let weight = -f32::from_bits(1);
    for operation in ["rest-bind", "whole-document"] {
        let fixture =
            Fixture::with_asset(rest_bind_scale_rig_with_negative_weight_glb(2, 3, weight));
        let output = match operation {
            "rest-bind" => fixture.rest_bind("0.01", "json"),
            "whole-document" => fixture.whole_document("100", "json"),
            _ => unreachable!(),
        };

        assert_eq!(
            output.status.code(),
            Some(1),
            "{operation} stderr:\n{}",
            stderr(&output)
        );
        assert!(!fixture.path("out.glb").exists(), "{operation}");
        assert!(!fixture.path("out.json").exists(), "{operation}");

        let record: Value =
            serde_json::from_str(&stdout(&output)).expect("stdout is one JSON record");
        assert_schema_valid(&record);
        assert_eq!(record["outcome"], "rejected", "{operation}");
        assert_eq!(record["result"], Value::Null, "{operation}");
        assert_eq!(record["rejection"]["stage"], "plan", "{operation}");
        assert_eq!(
            record["rejection"]["kind"], "negative-skin-weight",
            "{operation}"
        );
        assert_eq!(
            record["rejection"]["violations"],
            serde_json::json!([]),
            "{operation}"
        );
        assert!(
            record["rejection"]["detail"]
                .as_str()
                .expect("a rejection detail")
                .contains("vertex 2 primary skin influence 3"),
            "{operation} record: {record}"
        );
    }
}

#[test]
fn a_shared_document_shape_failure_keeps_its_cli_kind_and_publishes_nothing() {
    let fixture = Fixture::with_asset(rest_bind_scale_rig_with_non_finite_inverse_bind_glb());
    let output = fixture.rest_bind("0.01", "json");

    assert_eq!(
        output.status.code(),
        Some(1),
        "stderr:\n{}",
        stderr(&output)
    );
    assert!(!fixture.path("out.glb").exists());
    assert!(!fixture.path("out.json").exists());

    let record: Value = serde_json::from_str(&stdout(&output)).expect("stdout is one JSON record");
    assert_schema_valid(&record);
    assert_eq!(record["outcome"], "rejected");
    assert_eq!(record["result"], Value::Null);
    assert_eq!(record["rejection"]["stage"], "plan");
    assert_eq!(record["rejection"]["kind"], "invalid-mesh-instance");
    assert_eq!(record["rejection"]["violations"], serde_json::json!([]));
    assert_eq!(
        record["rejection"]["artifact_proof_differences"],
        Value::Null
    );
    assert!(
        record["rejection"]["detail"]
            .as_str()
            .expect("a rejection detail")
            .contains("mesh instance 0 is invalid (non_finite_inverse_bind)"),
        "record: {record}"
    );
}

#[test]
fn a_refusal_in_text_mode_writes_prose_to_stderr_and_nothing_to_stdout() {
    let fixture = Fixture::new();
    let output = fixture.rest_bind("0.02", "text");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout(&output), "");
    assert!(
        stderr(&output)
            .starts_with("animsmith: scale rest-bind refused rig.glb: [factor-mismatch]"),
        "stderr:\n{}",
        stderr(&output)
    );
    assert!(!fixture.path("out.glb").exists());
    assert!(!fixture.path("out.json").exists());
}

// --- A stdout that cannot take the record ----------------------------------

/// A stdout nobody is reading is a failure to **report** the run, not to
/// perform it: the pair is already on disk. Reporting exit `2` would say the
/// invocation was wrong when it was not, and would contradict what exit `2`
/// means everywhere else in this CLI.
#[test]
fn a_published_run_whose_stdout_is_closed_keeps_exit_0_and_diagnoses_on_stderr() {
    let fixture = Fixture::new();
    let output = fixture.rest_bind_into_closed_stdout("0.01", "json");

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&output)
    );
    // Ours, not the OS's: the platform's wording for a reader-less pipe is
    // not this contract.
    assert_eq!(
        stderr(&output)
            .matches("animsmith: cannot write JSON output to stdout")
            .count(),
        1,
        "stderr:\n{}",
        stderr(&output)
    );
    // And the run really did publish a self-consistent pair, which is why it
    // remains a success even though its summary could not be delivered.
    assert_published_pair_is_bound(&fixture);
}

/// The minimal-build producer's default text summary uses the same checked
/// boundary as JSON. Publication remains the outcome even when the summary's
/// consumer has already gone away.
#[test]
fn a_published_text_summary_with_closed_stdout_keeps_exit_0() {
    let fixture = Fixture::new();
    let output = fixture.rest_bind_into_closed_stdout("0.01", "text");

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&output)
    );
    assert_eq!(
        stderr(&output)
            .matches("animsmith: cannot write text output to stdout")
            .count(),
        1,
        "stderr:\n{}",
        stderr(&output)
    );
    assert_published_pair_is_bound(&fixture);
}

/// The same for a refusal, where raising the write failure is an actual
/// inversion rather than merely a wrong number: `scale … --format json | head`
/// on a refused asset would report an operator error (`2`) for something that
/// is a property of the asset (`1`) — the split this crate's exit codes exist
/// to make legible.
#[test]
fn a_refused_run_whose_stdout_is_closed_keeps_exit_1_and_diagnoses_on_stderr() {
    let fixture = Fixture::new();
    let output = fixture.rest_bind_into_closed_stdout("0.02", "json");

    assert_eq!(
        output.status.code(),
        Some(1),
        "a refusal whose record could not be printed is still a refusal; stderr:\n{}",
        stderr(&output)
    );
    assert_eq!(
        stderr(&output)
            .matches("animsmith: cannot write JSON output to stdout")
            .count(),
        1,
        "stderr:\n{}",
        stderr(&output)
    );
    // Nothing published, exactly as when the record is read successfully.
    assert!(!fixture.path("out.glb").exists());
    assert!(!fixture.path("out.json").exists());
}

#[test]
fn a_factor_the_plan_accepts_but_the_bytes_cannot_hold_is_refused_at_the_rewrite() {
    // `1e37` narrows to a usable `f32`, so planning accepts it; the rig's
    // largest length is the `300` translation key, and `300 * 1e37 = 3e39`
    // overflows binary32, whose maximum is about `3.4e38`. The refusal is
    // therefore raised where the bytes are written, not where the factor is
    // validated — which is what distinguishes the two stages.
    let fixture = Fixture::new();
    let output = fixture.whole_document("1e37", "json");

    assert_eq!(
        output.status.code(),
        Some(1),
        "stderr:\n{}",
        stderr(&output)
    );
    assert!(!fixture.path("out.glb").exists());
    assert!(!fixture.path("out.json").exists());

    let record: Value = serde_json::from_str(&stdout(&output)).expect("stdout is one JSON record");
    assert_schema_valid(&record);
    assert_eq!(record["rejection"]["stage"], "rewrite");
    assert_eq!(record["rejection"]["kind"], "value-not-representable");
}

#[test]
fn an_unsupported_source_domain_is_refused_with_its_typed_violations() {
    let fixture = Fixture::new();
    // A camera is one of the raw domains the #280 preflight refuses outright:
    // the scale operations cannot preserve or convert it.
    std::fs::write(
        fixture.path("camera.gltf"),
        br#"{"asset":{"version":"2.0"},
             "cameras":[{"type":"orthographic",
                         "orthographic":{"xmag":1,"ymag":1,"zfar":10,"znear":1}}],
             "nodes":[{"name":"only"}],
             "scenes":[{"nodes":[0]}],"scene":0}"#,
    )
    .unwrap();
    let output = animsmith()
        .current_dir(fixture.dir.path())
        .args([
            "scale",
            "whole-document",
            "camera.gltf",
            "-o",
            "out.gltf",
            "--factor",
            "0.01",
            "--evidence",
            "out.json",
            "--format",
            "json",
        ])
        .output()
        .expect("runs animsmith");

    assert_eq!(
        output.status.code(),
        Some(1),
        "stderr:\n{}",
        stderr(&output)
    );
    assert!(!fixture.path("out.gltf").exists());
    assert!(!fixture.path("out.json").exists());

    let record: Value = serde_json::from_str(&stdout(&output)).expect("stdout is one JSON record");
    assert_schema_valid(&record);
    assert_eq!(record["rejection"]["stage"], "preflight");
    assert_eq!(record["rejection"]["kind"], "unsupported-source-domain");
    assert_eq!(
        record["rejection"]["artifact_proof_differences"],
        Value::Null,
        "capability violations retain their existing, separate contract"
    );
    let kinds: Vec<&str> = record["rejection"]["violations"]
        .as_array()
        .expect("violations array")
        .iter()
        .map(|violation| violation["kind"].as_str().expect("a violation kind"))
        .collect();
    assert!(kinds.contains(&"camera"), "{kinds:?}");
    // The refusal carries the complete inventory gathered before rejection,
    // so a consumer can see what else the source declared.
    assert_eq!(record["capability"]["camera_count"], 1);
    assert_eq!(record["capability"]["container"], "gltf");
}

#[test]
fn rest_bind_morph_refusal_preserves_the_complete_located_inventory_and_prior_pair() {
    let fixture = Fixture::with_asset(rest_bind_scale_rig_with_morphs_glb());
    let prior_artifact = b"prior artifact";
    let prior_evidence = b"prior evidence";
    std::fs::write(fixture.path("out.glb"), prior_artifact).unwrap();
    std::fs::write(fixture.path("out.json"), prior_evidence).unwrap();

    let output = fixture.rest_bind("0.01", "json");
    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    assert_eq!(
        std::fs::read(fixture.path("out.glb")).unwrap(),
        prior_artifact
    );
    assert_eq!(
        std::fs::read(fixture.path("out.json")).unwrap(),
        prior_evidence
    );

    let record: Value = serde_json::from_str(&stdout(&output)).expect("one refusal record");
    assert_schema_valid(&record);
    assert_eq!(record["rejection"]["stage"], "preflight");
    assert_eq!(record["rejection"]["kind"], "unsupported-source-domain");
    assert_eq!(
        record["rejection"]["violations"]
            .as_array()
            .expect("violations")
            .len(),
        4
    );
    assert_eq!(
        record["rejection"]["violations"],
        serde_json::json!([
            { "kind": "morph_target", "location": "/meshes/1/primitives/1/targets" },
            { "kind": "morph_weights", "location": "/animations/1/channels/1/target/path" },
            { "kind": "morph_weights", "location": "/meshes/1/weights" },
            { "kind": "morph_weights", "location": "/nodes/4/weights" }
        ])
    );
    assert_eq!(
        record["capability"]["morph_weight_locations"],
        serde_json::json!([
            "/animations/1/channels/1/target/path",
            "/meshes/1/weights",
            "/nodes/4/weights"
        ])
    );
}

#[test]
fn early_generic_preflight_refusal_unions_the_selected_operations_morph_inventory() {
    let source = map_glb_json(rest_bind_scale_rig_with_morphs_glb(), |value| {
        value["cameras"] = serde_json::json!([{
            "type": "orthographic",
            "orthographic": { "xmag": 1, "ymag": 1, "zfar": 10, "znear": 1 }
        }]);
    });
    let fixture = Fixture::with_asset(source);
    let prior_artifact = b"prior artifact";
    let prior_evidence = b"prior evidence";
    std::fs::write(fixture.path("out.glb"), prior_artifact).unwrap();
    std::fs::write(fixture.path("out.json"), prior_evidence).unwrap();

    let output = fixture.rest_bind("0.01", "json");
    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    assert_eq!(
        std::fs::read(fixture.path("out.glb")).unwrap(),
        prior_artifact
    );
    assert_eq!(
        std::fs::read(fixture.path("out.json")).unwrap(),
        prior_evidence
    );

    let record: Value = serde_json::from_str(&stdout(&output)).expect("one refusal record");
    assert_schema_valid(&record);
    assert_eq!(record["rejection"]["stage"], "preflight");
    assert_eq!(record["rejection"]["kind"], "unsupported-source-domain");
    assert_eq!(
        record["rejection"]["detail"],
        "glTF scale preflight rejected 5 unsupported source domain(s)"
    );
    assert_eq!(
        record["rejection"]["violations"]
            .as_array()
            .expect("violations")
            .len(),
        5
    );
    assert_eq!(
        record["rejection"]["violations"],
        serde_json::json!([
            { "kind": "morph_target", "location": "/meshes/1/primitives/1/targets" },
            { "kind": "morph_weights", "location": "/animations/1/channels/1/target/path" },
            { "kind": "morph_weights", "location": "/meshes/1/weights" },
            { "kind": "morph_weights", "location": "/nodes/4/weights" },
            { "kind": "camera", "location": "/cameras" }
        ])
    );
    assert_eq!(
        record["capability"]["morph_weight_locations"],
        serde_json::json!([
            "/animations/1/channels/1/target/path",
            "/meshes/1/weights",
            "/nodes/4/weights"
        ])
    );
}

#[test]
fn an_animation_targeting_a_matrix_node_is_a_typed_preflight_refusal() {
    // glTF requires an animated node to use TRS. The raw guard must enforce
    // that source-contract rule before operation planning: the typed parser
    // can otherwise decompose the matrix and lose the authoring distinction.
    // This is a complete, self-contained two-key scale sampler so an invalid
    // accessor cannot accidentally be the reason the source was refused.
    let fixture = Fixture::new();
    std::fs::write(
        fixture.path("matrix-animation.gltf"),
        br#"{
          "asset":{"version":"2.0"},
          "buffers":[{"uri":"data:application/octet-stream;base64,AAAAAACAPwAAgD8AAIA/AACAPwAAgD8AAIA/AACAPwA=","byteLength":32}],
          "bufferViews":[
            {"buffer":0,"byteOffset":0,"byteLength":8},
            {"buffer":0,"byteOffset":8,"byteLength":24}
          ],
          "accessors":[
            {"bufferView":0,"componentType":5126,"count":2,"type":"SCALAR"},
            {"bufferView":1,"componentType":5126,"count":2,"type":"VEC3"}
          ],
          "nodes":[{"matrix":[1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1]}],
          "animations":[{
            "samplers":[{"input":0,"output":1,"interpolation":"LINEAR"}],
            "channels":[{"sampler":0,"target":{"node":0,"path":"scale"}}]
          }],
          "scenes":[{"nodes":[0]}],"scene":0
        }"#,
    )
    .expect("writes a matrix-animation fixture");

    let output = animsmith()
        .current_dir(fixture.dir.path())
        .args([
            "scale",
            "whole-document",
            "matrix-animation.gltf",
            "-o",
            "out.gltf",
            "--factor",
            "0.01",
            "--evidence",
            "out.json",
            "--format",
            "json",
        ])
        .output()
        .expect("runs animsmith");

    assert_eq!(
        output.status.code(),
        Some(1),
        "stderr:\n{}",
        stderr(&output)
    );
    assert!(!fixture.path("out.gltf").exists());
    assert!(!fixture.path("out.json").exists());

    let record: Value = serde_json::from_str(&stdout(&output)).expect("stdout is one JSON record");
    assert_schema_valid(&record);
    assert_eq!(record["outcome"], "rejected");
    assert_eq!(record["result"], Value::Null);
    assert_eq!(record["rejection"]["stage"], "preflight");
    assert_eq!(record["rejection"]["kind"], "unsupported-source-domain");
    assert_eq!(
        record["rejection"]["violations"],
        serde_json::json!([{
            "location": "/animations/0/channels/0/target",
            "kind": "animated_matrix_node"
        }]),
        "the violation identifies the raw channel target, not a normalized bone"
    );
    assert_eq!(
        record["capability"]["animation_channels"],
        serde_json::json!([{
            "animation_index": 0,
            "channel_index": 0,
            "target_node_index": 0,
            "target_path": "scale",
            "interpolation": "LINEAR",
            "input_accessor_index": 0,
            "output_accessor_index": 1
        }]),
        "the refused record retains the raw manifest identities"
    );
    assert_eq!(record["capability"]["nodes"][0]["node_index"], 0);
    assert_eq!(record["capability"]["nodes"][0]["rest_kind"], "matrix");
}

#[test]
fn a_document_whose_size_alone_exceeds_the_proof_sampling_budget_is_refused_at_the_proof() {
    // The proof stage is otherwise unreachable from the CLI: every residual
    // refusal needs a rewrite that produces one, which a correct rewriter
    // does not. The sampling budget is the exception, because it is a pure
    // size property of the input — and it is checked before the first sample
    // time is evaluated, so this 1.9 MB rig refuses in milliseconds.
    //
    // Hand-computed against the `appendix-d-v6` budget of 400,000,000:
    //
    //   per-sample cost = 2 sides * 3 bones                    =       6
    //                   + (2 sides + 1 skin residual) * 1 slot =       3
    //                   + 2 sides * 50,000 skinned vertices    = 100,000
    //                                                            -------
    //                                                            100,009
    //   sample times    = 5,000 LINEAR keys, no cubic segments
    //   work            = 5,000 * 100,009 = 500,045,000
    let fixture = Fixture::new();
    std::fs::write(fixture.path("big.glb"), oversized_proof_scale_rig_glb()).unwrap();
    let output = animsmith()
        .current_dir(fixture.dir.path())
        .args([
            "scale",
            "whole-document",
            "big.glb",
            "-o",
            "out.glb",
            "--factor",
            "0.01",
            "--evidence",
            "out.json",
            "--format",
            "json",
        ])
        .output()
        .expect("runs animsmith");

    assert_eq!(
        output.status.code(),
        Some(1),
        "a refusal that is a property of the input asset exits 1, whatever \
         stage raised it\nstderr:\n{}",
        stderr(&output)
    );
    assert!(!fixture.path("out.glb").exists());
    assert!(!fixture.path("out.json").exists());

    let record: Value = serde_json::from_str(&stdout(&output)).expect("stdout is one JSON record");
    assert_schema_valid(&record);
    assert_eq!(record["rejection"]["stage"], "proof");
    assert_eq!(
        record["rejection"]["kind"],
        "proof-sampling-budget-exceeded"
    );
    let detail = record["rejection"]["detail"]
        .as_str()
        .expect("a rejection detail");
    for expected in ["500045000", "5000 sample times", "100009 work units"] {
        assert!(detail.contains(expected), "{expected} is not in {detail}");
    }
}

#[test]
fn a_source_whose_bytes_do_not_parse_is_a_refusal_and_not_an_operator_error() {
    // The bytes were read: `fs::read` succeeded, and what failed is the
    // *asset*. Reporting that as exit 2 tells an operator their command line
    // was wrong when it was not — the exact inversion the 1-vs-2 split
    // exists to prevent.
    let fixture = Fixture::new();
    let mut truncated = rest_bind_scale_rig_glb();
    truncated.truncate(64);
    std::fs::write(fixture.path("junk.glb"), &truncated).unwrap();
    let output = animsmith()
        .current_dir(fixture.dir.path())
        .args([
            "scale",
            "whole-document",
            "junk.glb",
            "-o",
            "out.glb",
            "--factor",
            "0.01",
            "--evidence",
            "out.json",
            "--format",
            "json",
        ])
        .output()
        .expect("runs animsmith");

    assert_eq!(
        output.status.code(),
        Some(1),
        "stderr:\n{}",
        stderr(&output)
    );
    assert!(!fixture.path("out.glb").exists());
    assert!(!fixture.path("out.json").exists());

    let record: Value = serde_json::from_str(&stdout(&output)).expect("stdout is one JSON record");
    assert_schema_valid(&record);
    assert_eq!(record["outcome"], "rejected");
    assert_eq!(record["rejection"]["stage"], "preflight");
    assert_eq!(record["rejection"]["kind"], "unreadable-source");
    // Nothing loaded, so there is no inventory. `null` is that absence, and
    // the schema permits it only for a refusal.
    assert_eq!(record["capability"], Value::Null);
    // The record still identifies the bytes it refused.
    assert_eq!(record["input"]["bytes"], 64);
}

// --- Operator errors --------------------------------------------------------

/// Run `scale rest-bind` with substituted paths and report the exit code and
/// stderr.
fn rest_bind_paths(fixture: &Fixture, input: &str, output: &str, evidence: &str) -> Output {
    animsmith()
        .current_dir(fixture.dir.path())
        .args([
            "scale",
            "rest-bind",
            input,
            "-o",
            output,
            "--source-skin-index",
            "0",
            "--source-root-node-index",
            "0",
            "--expected-factor",
            "0.01",
            "--evidence",
            evidence,
        ])
        .output()
        .expect("runs animsmith")
}

#[test]
fn two_arguments_naming_one_file_are_an_operator_error() {
    let fixture = Fixture::new();
    for (input, output, evidence, expected) in [
        ("rig.glb", "rig.glb", "out.json", "input and output"),
        ("rig.glb", "out.glb", "rig.glb", "input and evidence"),
        // Lexically different, one file: the check resolves each parent
        // directory before comparing.
        ("rig.glb", "out.glb", "./out.glb", "output and evidence"),
    ] {
        let run = rest_bind_paths(&fixture, input, output, evidence);
        assert_eq!(
            run.status.code(),
            Some(2),
            "{input} {output} {evidence}\nstderr:\n{}",
            stderr(&run)
        );
        assert!(
            stderr(&run).contains(expected),
            "{input} {output} {evidence}\nstderr:\n{}",
            stderr(&run)
        );
    }
    assert!(!fixture.path("out.glb").exists());
    assert!(!fixture.path("out.json").exists());
}

#[cfg(unix)]
#[test]
fn a_symlinked_input_aliasing_a_destination_is_refused_before_the_asset_is_destroyed() {
    // The input and the two destinations are reached by different syscalls:
    // `fs::read` follows a symlinked final component, `fs::rename` replaces
    // one. Resolving the input the destination way therefore makes
    // `latest.glb -> store/rig.glb` compare as a *different* file from
    // `store/rig.glb`, publication renames over the source asset the run just
    // read, and the command reports success over a destroyed input.
    let fixture = Fixture::new();
    std::fs::create_dir(fixture.path("store")).unwrap();
    std::fs::rename(fixture.path("rig.glb"), fixture.path("store/rig.glb")).unwrap();
    std::os::unix::fs::symlink("store/rig.glb", fixture.path("latest.glb")).unwrap();
    let asset = rest_bind_scale_rig_glb();
    assert_eq!(std::fs::read(fixture.path("store/rig.glb")).unwrap(), asset);

    for (output, evidence, expected) in [
        ("store/rig.glb", "out.json", "input and output"),
        // `--evidence` has no extension check at all, so this direction
        // replaces the GLB with a JSON record.
        ("out.glb", "store/rig.glb", "input and evidence"),
    ] {
        let run = rest_bind_paths(&fixture, "latest.glb", output, evidence);
        assert_eq!(
            run.status.code(),
            Some(2),
            "{output} {evidence}\nstderr:\n{}",
            stderr(&run)
        );
        assert!(
            stderr(&run).contains(expected),
            "{output} {evidence}\nstderr:\n{}",
            stderr(&run)
        );
        assert_eq!(
            std::fs::read(fixture.path("store/rig.glb")).unwrap(),
            asset,
            "{output} {evidence}: the source asset must survive byte-identical"
        );
    }
    assert!(!fixture.path("out.glb").exists());
    assert!(!fixture.path("out.json").exists());
}

#[cfg(unix)]
#[test]
fn a_symlinked_input_that_aliases_nothing_publishes_normally() {
    // The guard above is distinctness, not a ban on symbolic links. `scale`
    // deliberately does not adopt `assemble`'s `reject_symlink_path`: that
    // rule keeps a recipe's declared inputs inside the recipe's own
    // directory, a containment property this command does not have, and its
    // three paths are the operator's own.
    let fixture = Fixture::new();
    std::fs::create_dir(fixture.path("store")).unwrap();
    std::fs::rename(fixture.path("rig.glb"), fixture.path("store/rig.glb")).unwrap();
    std::os::unix::fs::symlink("store/rig.glb", fixture.path("latest.glb")).unwrap();

    let run = rest_bind_paths(&fixture, "latest.glb", "out.glb", "out.json");
    assert_eq!(run.status.code(), Some(0), "stderr:\n{}", stderr(&run));
    assert_schema_valid(&read_json(&fixture.path("out.json")));
    // The declared path is recorded verbatim: the canonical form the
    // distinctness check computes never reaches the record.
    assert_eq!(
        read_json(&fixture.path("out.json"))["paths"]["input"],
        "latest.glb"
    );
    assert_eq!(
        std::fs::read(fixture.path("store/rig.glb")).unwrap(),
        rest_bind_scale_rig_glb(),
        "the linked-to asset is read, not written"
    );
}

#[test]
fn a_hardlinked_input_and_output_publish_with_the_source_surviving() {
    // A hard link is not an alias the distinctness check has to refuse, and
    // this test pins that reading so a future change does not "fix" it into
    // one. `rig.glb` and `alias.glb` are two directory entries for one inode;
    // `fs::canonicalize` resolves each to its own name, so they compare as
    // different paths — and they genuinely are different destinations,
    // because publication reaches the output through `fs::rename`, which
    // replaces `alias.glb`'s entry with the artifact's and leaves `rig.glb`
    // pointing at the original inode with its original bytes.
    //
    // That is exactly the distinctness §D.7 asks for: the run's source
    // survives the run. The symlink case above is different only because
    // `fs::read` follows a symlinked final component, so there the input and
    // the output are one entry rather than two.
    let fixture = Fixture::new();
    std::fs::hard_link(fixture.path("rig.glb"), fixture.path("alias.glb"))
        .expect("hard links the rig");
    let source = rest_bind_scale_rig_glb();
    assert_eq!(std::fs::read(fixture.path("alias.glb")).unwrap(), source);

    let run = rest_bind_paths(&fixture, "rig.glb", "alias.glb", "out.json");
    assert_eq!(run.status.code(), Some(0), "stderr:\n{}", stderr(&run));
    assert_schema_valid(&read_json(&fixture.path("out.json")));
    assert_eq!(
        std::fs::read(fixture.path("rig.glb")).unwrap(),
        source,
        "the input entry must survive byte-identical"
    );
    // And the publication really did land on the other entry, so the two
    // have parted company rather than the run having been a no-op.
    assert_ne!(
        std::fs::read(fixture.path("alias.glb")).unwrap(),
        source,
        "the output entry carries the rewritten artifact"
    );
}

#[test]
fn a_factor_that_is_not_finite_and_positive_is_an_operator_error() {
    // These are properties of the *invocation*: the factor is rejected before
    // a single member of the document is consulted, so a rejected evidence
    // record would claim the asset was examined and found wanting.
    let fixture = Fixture::new();
    for factor in ["nan", "inf", "-inf", "0", "-1"] {
        let run = fixture.whole_document(factor, "json");
        assert_eq!(
            run.status.code(),
            Some(2),
            "--factor {factor}\nstderr:\n{}",
            stderr(&run)
        );
        assert_eq!(
            stdout(&run),
            "",
            "--factor {factor} must print no evidence record"
        );
        assert!(
            stderr(&run).contains("scale factor must be finite and positive"),
            "--factor {factor}\nstderr:\n{}",
            stderr(&run)
        );
    }

    // The rest/bind operation validates its own declared factor, and says so
    // in its own words.
    let rest_bind = fixture.rest_bind("0", "json");
    assert_eq!(
        rest_bind.status.code(),
        Some(2),
        "stderr:\n{}",
        stderr(&rest_bind)
    );
    assert!(
        stderr(&rest_bind).contains("rest/bind expected factor must be finite and positive"),
        "stderr:\n{}",
        stderr(&rest_bind)
    );

    assert!(!fixture.path("out.glb").exists());
    assert!(!fixture.path("out.json").exists());
}

#[test]
fn a_non_gltf_extension_and_a_container_swap_are_operator_errors() {
    let fixture = Fixture::new();
    std::fs::write(fixture.path("rig.dae"), b"not read").unwrap();
    // Extension the producer does not read at all.
    let wrong_input = rest_bind_paths(&fixture, "rig.dae", "out.glb", "out.json");
    assert_eq!(wrong_input.status.code(), Some(2));
    assert!(
        stderr(&wrong_input).contains("self-contained glTF/GLB only"),
        "stderr:\n{}",
        stderr(&wrong_input)
    );

    // The rewrite operates on the source's own bytes and preserves its
    // container, so it cannot honour a `.gltf` destination for a GLB source.
    let swapped = rest_bind_paths(&fixture, "rig.glb", "out.gltf", "out.json");
    assert_eq!(swapped.status.code(), Some(2));
    assert!(
        stderr(&swapped).contains("must keep the source container"),
        "stderr:\n{}",
        stderr(&swapped)
    );

    // A GLB whose extension claims JSON glTF: the extension is checked
    // against the container the preflight actually found.
    std::fs::copy(fixture.path("rig.glb"), fixture.path("mislabelled.gltf")).unwrap();
    let mislabelled = rest_bind_paths(&fixture, "mislabelled.gltf", "out.gltf", "out.json");
    assert_eq!(
        mislabelled.status.code(),
        Some(2),
        "stderr:\n{}",
        stderr(&mislabelled)
    );
    assert!(
        stderr(&mislabelled).contains("its extension declares"),
        "stderr:\n{}",
        stderr(&mislabelled)
    );

    assert!(!fixture.path("out.glb").exists());
    assert!(!fixture.path("out.gltf").exists());
    assert!(!fixture.path("out.json").exists());
}

#[test]
fn an_unusable_input_or_destination_is_an_operator_error() {
    let fixture = Fixture::new();
    let missing_input = rest_bind_paths(&fixture, "absent.glb", "out.glb", "out.json");
    assert_eq!(missing_input.status.code(), Some(2));
    assert!(
        stderr(&missing_input).contains("cannot read"),
        "stderr:\n{}",
        stderr(&missing_input)
    );

    let missing_dir = rest_bind_paths(&fixture, "rig.glb", "absent/out.glb", "out.json");
    assert_eq!(missing_dir.status.code(), Some(2));
    assert!(
        stderr(&missing_dir).contains("output directory"),
        "stderr:\n{}",
        stderr(&missing_dir)
    );

    // A destination that exists but is not a regular file. This is the case
    // the destination check owns by itself: a missing *directory* is also
    // caught by resolving the path identity for the distinctness check, so
    // that case alone cannot show this guard is wired.
    std::fs::create_dir(fixture.path("dir.glb")).unwrap();
    let directory_artifact = rest_bind_paths(&fixture, "rig.glb", "dir.glb", "out.json");
    assert_eq!(
        directory_artifact.status.code(),
        Some(2),
        "stderr:\n{}",
        stderr(&directory_artifact)
    );
    assert!(
        stderr(&directory_artifact).contains("dir.glb is not a regular file"),
        "stderr:\n{}",
        stderr(&directory_artifact)
    );

    std::fs::create_dir(fixture.path("dir.json")).unwrap();
    let directory_evidence = rest_bind_paths(&fixture, "rig.glb", "out.glb", "dir.json");
    assert_eq!(directory_evidence.status.code(), Some(2));
    assert!(
        stderr(&directory_evidence).contains("dir.json is not a regular file"),
        "stderr:\n{}",
        stderr(&directory_evidence)
    );

    assert!(!fixture.path("out.glb").exists());
    assert!(!fixture.path("out.json").exists());
}

#[cfg(feature = "fbx")]
fn fbx_rest_bind_command(
    dir: &Path,
    input: &str,
    output: &str,
    evidence: &str,
    factor: &str,
    format: &str,
) -> Command {
    fbx_rest_bind_command_for_selectors(dir, input, output, evidence, factor, format, ("0", "1"))
}

#[cfg(feature = "fbx")]
fn fbx_rest_bind_command_for_selectors(
    dir: &Path,
    input: &str,
    output: &str,
    evidence: &str,
    factor: &str,
    format: &str,
    selectors: (&str, &str),
) -> Command {
    let mut command = animsmith();
    command.current_dir(dir).args([
        "scale",
        "rest-bind",
        input,
        "-o",
        output,
        "--source-skin-index",
        selectors.0,
        "--source-root-node-index",
        selectors.1,
        "--expected-factor",
        factor,
        "--evidence",
        evidence,
        "--format",
        format,
    ]);
    command
}

#[cfg(feature = "fbx")]
fn two_skin_fbx_fixture() -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../animsmith-fbx/testdata/rigged_triangle.fbx"),
    )
    .expect("reads analytic FBX fixture")
    .replacen("Count: 8", "Count: 14", 1)
    .replacen(
        "ObjectType: \"Model\" { Count: 2 }",
        "ObjectType: \"Model\" { Count: 4 }",
        1,
    )
    .replacen(
        "ObjectType: \"Geometry\" { Count: 1 }",
        "ObjectType: \"Geometry\" { Count: 2 }",
        1,
    )
    .replacen(
        "ObjectType: \"Deformer\" { Count: 2 }",
        "ObjectType: \"Deformer\" { Count: 4 }",
        1,
    )
    .replacen(
        "\tAnimationStack: 3001",
        r#"
	Model: 1101, "Model::root-two", "Null" {
		Version: 232
		Properties70: {
			P: "Lcl Translation", "Lcl Translation", "", "A",0,0,0
			P: "Lcl Rotation", "Lcl Rotation", "", "A",0,0,0
			P: "Lcl Scaling", "Lcl Scaling", "", "A",1,1,1
		}
	}
	Geometry: 2101, "Geometry::tri-two", "Mesh" {
		Vertices: *9 { a: 0,0,0,100,0,0,0,100,0 }
		PolygonVertexIndex: *3 { a: 0,1,-3 }
	}
	Model: 1102, "Model::tri-two", "Mesh" {
		Version: 232
		Properties70: {
			P: "Lcl Translation", "Lcl Translation", "", "A",0,0,0
			P: "Lcl Rotation", "Lcl Rotation", "", "A",0,0,0
			P: "Lcl Scaling", "Lcl Scaling", "", "A",1,1,1
		}
	}
	Deformer: 4101, "Deformer::skin-two", "Skin" {
		Version: 101
		Link_DeformAccuracy: 50
	}
	Deformer: 4102, "SubDeformer::root-two-cluster", "Cluster" {
		Version: 100
		Indexes: *3 { a: 0,1,2 }
		Weights: *3 { a: 1,1,1 }
		Transform: *16 { a: 1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1 }
		TransformLink: *16 { a: 1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1 }
	}
	AnimationStack: 3001"#,
        1,
    )
    .replacen(
        "\tC: \"OO\",3002,3001",
        "\tC: \"OO\",1101,0\n\tC: \"OO\",1102,1101\n\tC: \"OO\",2101,1102\n\tC: \"OO\",4101,2101\n\tC: \"OO\",4102,4101\n\tC: \"OO\",1101,4102\n\tC: \"OO\",3002,3001",
        1,
    )
    .replace(r"\t", "\t")
    // FBX's historical field spelling is intentionally emitted without
    // placing that misspelling in Rust source, where the typo gate owns it.
    .replace("Link_DeformAccuracy", &["Link_DeformA", "curacy"].concat())
}

#[cfg(feature = "fbx")]
fn external_normal_texture_fbx_fixture() -> String {
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../animsmith-fbx/testdata/rigged_triangle.fbx"),
    )
    .expect("reads analytic FBX fixture")
    .replace("\r\n", "\n");
    let source = source.replacen(
        "\tObjectType: \"Deformer\" { Count: 2 }\n}",
        "\tObjectType: \"Deformer\" { Count: 2 }\n\tObjectType: \"Material\" { Count: 1 }\n\tObjectType: \"Texture\" { Count: 1 }\n\tObjectType: \"Video\" { Count: 1 }\n}",
        1,
    );
    let objects = r#"	Material: 5001, "Material::normal_mat", "" {
		Version: 102
		ShadingModel: "phong"
		MultiLayer: 0
	}
	Texture: 5002, "Texture::normal", "" {
		Type: "TextureVideoClip"
		Version: 202
		TextureName: "Texture::normal"
		Media: "Video::normal"
		FileName: "normal.png"
		RelativeFilename: "normal.png"
		ModelUVTranslation: 0,0
		ModelUVScaling: 1,1
		Texture_Alpha_Source: "None"
		Cropping: 0,0,0,0
	}
	Video: 5003, "Video::normal", "Clip" {
		Type: "Clip"
		Properties70: {
			P: "Path", "KString", "XRefUrl", "", "normal.png"
		}
		FileName: "normal.png"
		RelativeFilename: "normal.png"
	}
}
Connections: {"#;
    source
        .replacen("}\nConnections: {", objects, 1)
        .replacen(
            "Connections: {",
            "Connections: {\n\tC: \"OO\",5001,1002\n\tC: \"OP\",5002,5001,\"NormalMap\"\n\tC: \"OO\",5003,5002",
            1,
        )
}

#[cfg(feature = "fbx")]
fn external_resource_closure_budget_fbx_fixture(last_locator: &str) -> String {
    let mut videos = String::new();
    for index in 0..=DEPENDENCY_CLOSURE_V1_MAX_EXTERNAL_RESOURCES {
        let locator = if index == DEPENDENCY_CLOSURE_V1_MAX_EXTERNAL_RESOURCES {
            last_locator.to_owned()
        } else {
            format!("resource{index}.bin")
        };
        videos.push_str(&format!(
            concat!(
                "\tVideo: {}, \"Video::resource{}\", \"Clip\" {{\n",
                "\t\tType: \"Clip\"\n",
                "\t\tFileName: {:?}\n",
                "\t\tRelativeFilename: {:?}\n",
                "\t}}\n"
            ),
            10_000 + index,
            index,
            locator,
            locator
        ));
    }
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../animsmith-fbx/testdata/rigged_triangle.fbx"),
    )
    .expect("reads analytic FBX fixture")
    .replace("\r\n", "\n")
    .replacen(
        "\tObjectType: \"Deformer\" { Count: 2 }\n}",
        &format!(
            "\tObjectType: \"Deformer\" {{ Count: 2 }}\n\tObjectType: \"Video\" {{ Count: {} }}\n}}",
            DEPENDENCY_CLOSURE_V1_MAX_EXTERNAL_RESOURCES + 1
        ),
        1,
    )
    .replacen("}\nConnections: {", &format!("{videos}}}\nConnections: {{"), 1)
}

/// Two separate skins whose ordered named joint topologies deliberately
/// collide after staging, while their selected parent root remains unique.
#[cfg(feature = "fbx")]
fn ambiguous_skin_fbx_fixture() -> String {
    two_skin_fbx_fixture()
        .replacen("Count: 14", "Count: 15", 1)
        .replacen(
            "ObjectType: \"Model\" { Count: 4 }",
            "ObjectType: \"Model\" { Count: 5 }",
            1,
        )
        .replacen(
            "\tModel: 1001, \"Model::root\", \"Null\" {",
            "\tModel: 9001, \"Model::selector-root\", \"Null\" {\n\t\tVersion: 232\n\t\tProperties70: {\n\t\t\tP: \"Lcl Translation\", \"Lcl Translation\", \"\", \"A\",0,0,0\n\t\t\tP: \"Lcl Rotation\", \"Lcl Rotation\", \"\", \"A\",0,0,0\n\t\t\tP: \"Lcl Scaling\", \"Lcl Scaling\", \"\", \"A\",1,1,1\n\t\t}\n\t}\n\tModel: 1001, \"Model::root\", \"Null\" {",
            1,
        )
        .replacen("Model::root-two", "Model::root", 1)
        .replacen(
            "\tC: \"OO\",1001,0",
            "\tC: \"OO\",9001,0\n\tC: \"OO\",1001,9001",
            1,
        )
        .replacen("\tC: \"OO\",1101,0", "\tC: \"OO\",1101,9001", 1)
}

/// One skin whose two distinct joint nodes deliberately share a normalized
/// name; its selected ancestor root is unique, so this reaches the selected
/// skin's repeated-name bridge rather than either root or skin cardinality.
#[cfg(feature = "fbx")]
fn repeated_selected_joint_name_fbx_fixture() -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../animsmith-fbx/testdata/rigged_triangle.fbx"),
    )
    .expect("reads analytic FBX fixture")
    // The ordered connection splice below deliberately matches its analytic
    // LF fixture spelling. Normalize a Windows checkout first so it builds
    // the same selected-skin topology on every CI platform.
    .replace("\r\n", "\n")
    .replacen("Count: 8", "Count: 12", 1)
    .replacen(
        "Weights: *3 { a: 1,1,1 }",
        "Weights: *3 { a: 0.5,0.5,0.5 }",
        1,
    )
    .replacen(
        "ObjectType: \"Model\" { Count: 2 }",
        "ObjectType: \"Model\" { Count: 4 }",
        1,
    )
    .replacen(
        "ObjectType: \"Deformer\" { Count: 2 }",
        "ObjectType: \"Deformer\" { Count: 4 }",
        1,
    )
    .replacen(
        "\tAnimationStack: 3001",
        r#"
	Model: 1101, "Model::spine", "Null" {
		Version: 232
		Properties70: {
			P: "Lcl Translation", "Lcl Translation", "", "A",0,0,0
			P: "Lcl Rotation", "Lcl Rotation", "", "A",0,0,0
			P: "Lcl Scaling", "Lcl Scaling", "", "A",1,1,1
		}
	}
	Model: 1102, "Model::spine", "Null" {
		Version: 232
		Properties70: {
			P: "Lcl Translation", "Lcl Translation", "", "A",0,0,0
			P: "Lcl Rotation", "Lcl Rotation", "", "A",0,0,0
			P: "Lcl Scaling", "Lcl Scaling", "", "A",1,1,1
		}
	}
	Deformer: 4101, "SubDeformer::spine-one-cluster", "Cluster" {
		Version: 100
		Indexes: *3 { a: 0,1,2 }
		Weights: *3 { a: 0.25,0.25,0.25 }
		Transform: *16 { a: 1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1 }
		TransformLink: *16 { a: 1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1 }
	}
	Deformer: 4102, "SubDeformer::spine-two-cluster", "Cluster" {
		Version: 100
		Indexes: *3 { a: 0,1,2 }
		Weights: *3 { a: 0.25,0.25,0.25 }
		Transform: *16 { a: 1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1 }
		TransformLink: *16 { a: 1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1 }
	}
	AnimationStack: 3001"#,
        1,
    )
    .replacen(
        "\tC: \"OO\",4001,2001\n\tC: \"OO\",4002,4001\n\tC: \"OO\",1001,4002",
        "\tC: \"OO\",4001,2001\n\tC: \"OO\",4002,4001\n\tC: \"OO\",1001,4002\n\tC: \"OO\",1101,1001\n\tC: \"OO\",1102,1001\n\tC: \"OO\",4101,4001\n\tC: \"OO\",4102,4001\n\tC: \"OO\",1101,4101\n\tC: \"OO\",1102,4102",
        1,
    )
}

#[cfg(feature = "fbx")]
fn two_active_take_fbx_fixture() -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../animsmith-fbx/testdata/rigged_triangle_empty_take.fbx"),
    )
    .expect("reads analytic two-take FBX fixture")
    .replacen(
        "\tAnimationStack: 3011, \"AnimStack::empty\", \"\" {}",
        "\tAnimationStack: 3011, \"AnimStack::second\", \"\" {\n\t\tProperties70: {\n\t\t\tP: \"LocalStart\", \"KTime\", \"Time\", \"\",23093079000\n\t\t\tP: \"LocalStop\", \"KTime\", \"Time\", \"\",46186158000\n\t\t\tP: \"ReferenceStart\", \"KTime\", \"Time\", \"\",23093079000\n\t\t\tP: \"ReferenceStop\", \"KTime\", \"Time\", \"\",46186158000\n\t\t}\n\t}",
        1,
    )
    .replacen(
        "\tC: \"OO\",3002,3001",
        "\tC: \"OO\",3002,3001\n\tC: \"OO\",3002,3011",
        1,
    )
}

/// The conservative #286 boundary accepts a complete normalized FBX inventory
/// only for rest/bind scaling, stages a private GLB, proves the exact emitted
/// GLB once, and atomically publishes that artifact/evidence pair.
#[cfg(feature = "fbx")]
#[test]
fn fbx_rest_bind_reencodes_and_proves_a_complete_inventory() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let input = dir.path().join("rig.fbx");
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../animsmith-fbx/testdata/rigged_triangle.fbx"),
        &input,
    )
    .expect("copies self-authored FBX fixture");
    let output =
        fbx_rest_bind_command(dir.path(), "rig.fbx", "out.glb", "out.json", "0.01", "json")
            .output()
            .expect("runs FBX rest-bind scale");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&output)
    );
    assert!(output.stderr.is_empty());
    let record: Value = serde_json::from_slice(&output.stdout).expect("JSON evidence");
    assert_fbx_schema_valid(&record);
    assert_eq!(record["schema"], FBX_SCALE_EVIDENCE_SCHEMA_ID);
    assert_eq!(record["input_format"], "fbx");
    assert_eq!(record["outcome"], "published");
    assert_eq!(
        record["capability"]["domains"]["translation_animation"],
        "baked"
    );
    assert_eq!(record["capability"]["animation_takes_baked"], true);
    assert_eq!(record["capability"]["authored_curve_keys_preserved"], false);
    assert_eq!(
        record["capability"]["domains"]["shared_raw_accessor_payloads"],
        "unverifiable"
    );
    assert_eq!(
        record["result"]["scale"]["proof"]["read_back_digest_matches"],
        true
    );
    assert!(record["result"]["staged_source"]["bytes"].as_u64().unwrap() > 0);
    let artifact = std::fs::read(dir.path().join("out.glb")).expect("published GLB");
    animsmith_gltf::load(&dir.path().join("out.glb")).expect("published GLB reloads");
    assert_eq!(
        record["result"]["scale"]["artifact"]["sha256"],
        sha256_hex(&artifact)
    );
    assert_eq!(
        record["result"]["scale"]["artifact"]["bytes"],
        artifact.len() as u64
    );
    // The staging boundary is deliberately private. It is bound to the
    // evidence by its own identity, while the public output contract is the
    // reloaded/proved artifact rather than byte identity with `convert`'s
    // implementation detail.
    assert!(
        record["result"]["staged_source"]["sha256"]
            .as_str()
            .is_some_and(|digest| digest.len() == 64),
        "the private stage is identified without making convert byte layout public"
    );
    assert_eq!(
        std::fs::read(dir.path().join("out.json")).unwrap(),
        output.stdout
    );
    let first_artifact = artifact;
    let first_evidence = output.stdout;
    let rerun = fbx_rest_bind_command(dir.path(), "rig.fbx", "out.glb", "out.json", "0.01", "json")
        .output()
        .expect("reruns deterministic FBX rest-bind scale");
    assert_eq!(rerun.status.code(), Some(0), "stderr:\n{}", stderr(&rerun));
    assert_eq!(
        std::fs::read(dir.path().join("out.glb")).unwrap(),
        first_artifact
    );
    assert_eq!(
        std::fs::read(dir.path().join("out.json")).unwrap(),
        first_evidence
    );
    assert_eq!(rerun.stdout, first_evidence);
}

#[cfg(feature = "fbx")]
#[test]
fn fbx_rest_bind_captures_an_admitted_external_texture_before_staging() {
    let dir = tempfile::tempdir().expect("temporary directory");
    std::fs::create_dir(dir.path().join("nested")).expect("creates nested input directory");
    std::fs::write(
        dir.path().join("nested/rig.fbx"),
        external_normal_texture_fbx_fixture(),
    )
    .expect("writes analytic external-texture FBX");
    std::fs::write(dir.path().join("nested/normal.png"), TINY_PNG)
        .expect("writes analytic linked texture");
    std::fs::write(dir.path().join("normal.png"), b"wrong-root decoy")
        .expect("writes broader-root decoy");

    let output = fbx_rest_bind_command(
        dir.path(),
        "nested/rig.fbx",
        "out.glb",
        "out.json",
        "0.01",
        "json",
    )
    .output()
    .expect("runs FBX rest-bind scale");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&output)
    );
    let record: Value = serde_json::from_slice(&output.stdout).expect("JSON evidence");
    assert_fbx_schema_valid(&record);
    assert_eq!(record["capability"]["external_resource_count"], 2);

    let artifact = animsmith_gltf::load(&dir.path().join("out.glb"))
        .expect("published external-texture GLB reloads");
    assert_eq!(
        artifact.assets.materials[0]
            .normal_texture
            .as_ref()
            .expect("published material retains its captured normal texture")
            .texture
            .bytes,
        TINY_PNG,
        "admitting a scale-irrelevant declaration must not silently remove its supported payload"
    );
}

#[cfg(feature = "fbx")]
#[test]
fn fbx_rest_bind_refuses_destinations_that_alias_captured_dependencies() {
    for (destination, locator, output, evidence) in [
        ("evidence", "normal.png", "out.glb", "nested/normal.png"),
        ("output", "artifact.glb", "nested/artifact.glb", "out.json"),
    ] {
        let dir = tempfile::tempdir().expect("temporary directory");
        std::fs::create_dir(dir.path().join("nested")).unwrap();
        let source = external_normal_texture_fbx_fixture().replace("normal.png", locator);
        std::fs::write(dir.path().join("nested/rig.fbx"), source).unwrap();
        let dependency = dir.path().join("nested").join(locator);
        std::fs::write(&dependency, TINY_PNG).unwrap();
        let (peer, prior_peer) = if destination == "evidence" {
            (dir.path().join(output), b"prior artifact".as_slice())
        } else {
            (dir.path().join(evidence), b"prior evidence".as_slice())
        };
        std::fs::write(&peer, prior_peer).unwrap();

        let result = fbx_rest_bind_command(
            dir.path(),
            "nested/rig.fbx",
            output,
            evidence,
            "0.01",
            "json",
        )
        .output()
        .expect("runs destructive-alias preflight");

        assert_eq!(
            result.status.code(),
            Some(2),
            "stderr:\n{}",
            stderr(&result)
        );
        assert!(result.stdout.is_empty());
        assert!(
            stderr(&result).contains(&format!(
                "scale external dependency {locator:?} and {destination} must be different paths"
            )),
            "stderr:\n{}",
            stderr(&result)
        );
        assert_eq!(
            std::fs::read(dependency).unwrap(),
            TINY_PNG,
            "publication must not replace the captured source dependency"
        );
        assert_eq!(
            std::fs::read(peer).unwrap(),
            prior_peer,
            "refusal must leave the non-aliased publication peer byte-identical"
        );
    }
}

#[cfg(feature = "fbx")]
#[test]
fn fbx_rest_bind_protects_a_keyed_dependency_that_exceeds_the_capture_budget() {
    let dir = tempfile::tempdir().expect("temporary directory");
    std::fs::create_dir(dir.path().join("nested")).unwrap();
    let source = external_normal_texture_fbx_fixture().replace("normal.png", "oversized.png");
    std::fs::write(dir.path().join("nested/rig.fbx"), source).unwrap();
    let dependency = dir.path().join("nested/oversized.png");
    std::fs::write(&dependency, TINY_PNG).unwrap();
    let oversized_len = animsmith_core::ResourceClosureBudgetV1::VALUE
        .max_resource_bytes()
        .saturating_add(1);
    std::fs::OpenOptions::new()
        .write(true)
        .open(&dependency)
        .unwrap()
        .set_len(oversized_len)
        .unwrap();
    let peer = dir.path().join("out.glb");
    let prior_peer = b"prior artifact";
    std::fs::write(&peer, prior_peer).unwrap();

    let result = fbx_rest_bind_command(
        dir.path(),
        "nested/rig.fbx",
        "out.glb",
        "nested/oversized.png",
        "0.01",
        "json",
    )
    .output()
    .expect("runs keyed unavailable-dependency preflight");

    assert_eq!(
        result.status.code(),
        Some(2),
        "stderr:\n{}",
        stderr(&result)
    );
    assert!(result.stdout.is_empty());
    assert_eq!(
        stderr(&result),
        "animsmith: scale dependency closure exceeded its resource budget, so publication cannot prove every source sidecar distinct\n"
    );
    assert_eq!(std::fs::metadata(&dependency).unwrap().len(), oversized_len);
    let mut prefix = [0u8; 8];
    let mut dependency_file = std::fs::File::open(&dependency).unwrap();
    std::io::Read::read_exact(&mut dependency_file, &mut prefix).unwrap();
    assert_eq!(prefix, TINY_PNG[..8]);
    assert_eq!(std::fs::read(peer).unwrap(), prior_peer);
}

#[cfg(feature = "fbx")]
#[test]
fn fbx_rest_bind_rejects_a_truncated_dependency_closure_before_publication() {
    for last_locator in ["out.json", "tail.png"] {
        let dir = tempfile::tempdir().expect("temporary directory");
        std::fs::write(
            dir.path().join("rig.fbx"),
            external_resource_closure_budget_fbx_fixture(last_locator),
        )
        .unwrap();
        let artifact = dir.path().join("out.glb");
        let evidence = dir.path().join("out.json");
        let tail = dir.path().join("tail.png");
        let prior_artifact = b"prior artifact";
        let prior_evidence = b"prior evidence";
        let prior_tail = b"source-sidecar sentinel";
        std::fs::write(&artifact, prior_artifact).unwrap();
        std::fs::write(&evidence, prior_evidence).unwrap();
        if last_locator == "tail.png" {
            std::fs::write(&tail, prior_tail).unwrap();
        }

        let result =
            fbx_rest_bind_command(dir.path(), "rig.fbx", "out.glb", "out.json", "0.01", "json")
                .output()
                .expect("runs dependency-closure budget preflight");

        assert_eq!(
            result.status.code(),
            Some(2),
            "{last_locator} stderr:\n{}",
            stderr(&result)
        );
        assert!(result.stdout.is_empty());
        assert_eq!(
            stderr(&result),
            "animsmith: scale dependency closure exceeded its resource budget, so publication cannot prove every source sidecar distinct\n"
        );
        assert_eq!(std::fs::read(artifact).unwrap(), prior_artifact);
        assert_eq!(std::fs::read(evidence).unwrap(), prior_evidence);
        if last_locator == "tail.png" {
            assert_eq!(std::fs::read(tail).unwrap(), prior_tail);
        }
    }
}

#[cfg(feature = "fbx")]
#[test]
fn fbx_rest_bind_protects_a_missing_retained_key_without_requiring_its_parent() {
    let alias_dir = tempfile::tempdir().expect("temporary directory");
    std::fs::create_dir(alias_dir.path().join("nested")).unwrap();
    let source = external_normal_texture_fbx_fixture().replace("normal.png", "missing.png");
    std::fs::write(alias_dir.path().join("nested/rig.fbx"), source).unwrap();
    let peer = alias_dir.path().join("out.glb");
    let prior_peer = b"prior artifact";
    std::fs::write(&peer, prior_peer).unwrap();

    let alias = fbx_rest_bind_command(
        alias_dir.path(),
        "nested/rig.fbx",
        "out.glb",
        "nested/missing.png",
        "0.01",
        "json",
    )
    .output()
    .expect("runs missing-key alias preflight");
    assert_eq!(alias.status.code(), Some(2), "stderr:\n{}", stderr(&alias));
    assert!(alias.stdout.is_empty());
    assert!(
        stderr(&alias).contains(
            "scale external dependency \"missing.png\" and evidence must be different paths"
        ),
        "stderr:\n{}",
        stderr(&alias)
    );
    assert!(!alias_dir.path().join("nested/missing.png").exists());
    assert_eq!(std::fs::read(peer).unwrap(), prior_peer);

    let unrelated_dir = tempfile::tempdir().expect("temporary directory");
    let source =
        external_normal_texture_fbx_fixture().replace("normal.png", "textures/missing.png");
    std::fs::write(unrelated_dir.path().join("rig.fbx"), source).unwrap();
    let unrelated = fbx_rest_bind_command(
        unrelated_dir.path(),
        "rig.fbx",
        "out.glb",
        "out.json",
        "0.01",
        "json",
    )
    .output()
    .expect("runs unrelated missing-intermediate dependency");
    assert_eq!(
        unrelated.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&unrelated)
    );
    assert!(unrelated.stderr.is_empty());
    assert!(!unrelated.stdout.is_empty());
    assert!(unrelated_dir.path().join("out.glb").is_file());
    assert!(unrelated_dir.path().join("out.json").is_file());
    assert!(!unrelated_dir.path().join("textures").exists());
}

#[cfg(all(feature = "fbx", unix))]
#[test]
fn fbx_rest_bind_refuses_a_symlink_retained_key_before_any_publication() {
    use std::os::unix::fs::symlink;

    for destination_kind in ["entry", "target", "unrelated"] {
        let dir = tempfile::tempdir().expect("temporary directory");
        std::fs::create_dir(dir.path().join("nested")).unwrap();
        std::fs::write(
            dir.path().join("nested/rig.fbx"),
            external_normal_texture_fbx_fixture(),
        )
        .unwrap();
        let dependency = dir.path().join("nested/normal.png");
        let target = dir.path().join("nested/source.png");
        std::fs::write(&target, TINY_PNG).unwrap();
        symlink("source.png", &dependency).unwrap();
        let case_variant = dir.path().join("nested/NORMAL.PNG");
        let entry_destination = if std::fs::symlink_metadata(&case_variant).is_ok() {
            "nested/NORMAL.PNG"
        } else {
            "nested/normal.png"
        };
        let evidence = match destination_kind {
            "entry" => entry_destination,
            "target" => "nested/source.png",
            "unrelated" => "out.json",
            _ => unreachable!(),
        };
        let peer = dir.path().join("out.glb");
        let unrelated_evidence = dir.path().join("out.json");
        let prior_peer = b"prior artifact";
        let prior_evidence = b"prior evidence";
        std::fs::write(&peer, prior_peer).unwrap();
        std::fs::write(&unrelated_evidence, prior_evidence).unwrap();

        let result = fbx_rest_bind_command(
            dir.path(),
            "nested/rig.fbx",
            "out.glb",
            evidence,
            "0.01",
            "json",
        )
        .output()
        .expect("runs symlink-refused dependency preflight");

        assert_eq!(
            result.status.code(),
            Some(2),
            "{destination_kind} stderr:\n{}",
            stderr(&result)
        );
        assert!(result.stdout.is_empty());
        assert!(
            stderr(&result).contains(
                "scale external dependency \"normal.png\" is a symlink and cannot be published safely"
            ),
            "{destination_kind} stderr:\n{}",
            stderr(&result)
        );
        assert!(
            std::fs::symlink_metadata(&dependency)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(
            std::fs::read_link(&dependency).unwrap(),
            Path::new("source.png")
        );
        assert_eq!(std::fs::read(target).unwrap(), TINY_PNG);
        assert_eq!(std::fs::read(peer).unwrap(), prior_peer);
        assert_eq!(std::fs::read(unrelated_evidence).unwrap(), prior_evidence);
    }
}

#[cfg(feature = "fbx")]
#[test]
fn fbx_rest_bind_uses_the_filesystems_case_semantics_for_destination_identity() {
    let dir = tempfile::tempdir().expect("temporary directory");
    std::fs::create_dir(dir.path().join("nested")).unwrap();
    std::fs::write(
        dir.path().join("nested/rig.fbx"),
        external_normal_texture_fbx_fixture(),
    )
    .unwrap();
    let dependency = dir.path().join("nested/normal.png");
    std::fs::write(&dependency, TINY_PNG).unwrap();
    let case_variant = dir.path().join("nested/NORMAL.PNG");
    let case_insensitive = std::fs::canonicalize(&case_variant).is_ok();
    let peer = dir.path().join("out.glb");
    let prior_peer = b"prior artifact";
    std::fs::write(&peer, prior_peer).unwrap();

    let result = fbx_rest_bind_command(
        dir.path(),
        "nested/rig.fbx",
        "out.glb",
        case_variant
            .strip_prefix(dir.path())
            .unwrap()
            .to_str()
            .unwrap(),
        "0.01",
        "json",
    )
    .output()
    .expect("runs case-variant destructive-alias preflight");

    if case_insensitive {
        assert_eq!(
            result.status.code(),
            Some(2),
            "stderr:\n{}",
            stderr(&result)
        );
        assert!(result.stdout.is_empty());
        assert!(
            stderr(&result).contains(
                "scale external dependency \"normal.png\" and evidence must be different paths"
            ),
            "stderr:\n{}",
            stderr(&result)
        );
        assert_eq!(std::fs::read(peer).unwrap(), prior_peer);
    } else {
        assert_eq!(
            result.status.code(),
            Some(0),
            "stderr:\n{}",
            stderr(&result)
        );
        assert!(result.stderr.is_empty());
        assert!(!result.stdout.is_empty());
        assert_ne!(std::fs::read(peer).unwrap(), prior_peer);
        assert!(case_variant.is_file());
    }
    assert_eq!(std::fs::read(dependency).unwrap(), TINY_PNG);
}

#[cfg(feature = "fbx")]
#[test]
fn fbx_rest_bind_maps_a_second_skin_and_preserves_every_baked_take() {
    let dir = tempfile::tempdir().expect("temporary directory");
    std::fs::write(dir.path().join("two-skins.fbx"), two_skin_fbx_fixture())
        .expect("writes analytic two-skin FBX fixture");
    let output = fbx_rest_bind_command_for_selectors(
        dir.path(),
        "two-skins.fbx",
        "out.glb",
        "out.json",
        "0.01",
        "json",
        ("1", "2"),
    )
    .output()
    .expect("runs second-skin FBX rest-bind scale");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&output)
    );
    let record: Value = serde_json::from_slice(&output.stdout).expect("JSON evidence");
    assert_fbx_schema_valid(&record);
    assert_eq!(record["operation"]["source_skin_index"], 1);
    assert_eq!(record["operation"]["source_root_node_index"], 2);
    assert_eq!(record["capability"]["skin_deformer_count"], 2);
    let document =
        animsmith_gltf::load(&dir.path().join("out.glb")).expect("published GLB reloads");
    assert_eq!(document.assets.source_skeleton.skins.len(), 2);
    assert_eq!(
        document.clips.len(),
        1,
        "the source take is retained after staging"
    );
}

#[cfg(feature = "fbx")]
#[test]
fn fbx_rest_bind_preserves_multiple_baked_takes() {
    let dir = tempfile::tempdir().expect("temporary directory");
    std::fs::write(
        dir.path().join("two-takes.fbx"),
        two_active_take_fbx_fixture(),
    )
    .expect("writes active two-take FBX fixture");
    let output = fbx_rest_bind_command(
        dir.path(),
        "two-takes.fbx",
        "out.glb",
        "out.json",
        "0.01",
        "json",
    )
    .output()
    .expect("runs multi-take FBX rest-bind scale");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&output)
    );
    let record: Value = serde_json::from_slice(&output.stdout).expect("JSON evidence");
    assert_fbx_schema_valid(&record);
    assert_eq!(record["capability"]["animation_take_count"], 2);
    assert_eq!(record["capability"]["animation_takes_baked"], true);
    assert_eq!(record["capability"]["authored_curve_keys_preserved"], false);
    let document =
        animsmith_gltf::load(&dir.path().join("out.glb")).expect("published GLB reloads");
    assert_eq!(
        document.clips.len(),
        2,
        "every baked take survives the staged proof path"
    );
    let take = document
        .clips
        .iter()
        .find(|clip| clip.name == "take")
        .expect("first take is retained");
    let second = document
        .clips
        .iter()
        .find(|clip| clip.name == "second")
        .expect("second take is retained");
    let take_track = take
        .tracks
        .iter()
        .find(|track| track.property == Property::Translation)
        .expect("first take has translation");
    let second_track = second
        .tracks
        .iter()
        .find(|track| track.property == Property::Translation)
        .expect("second take has translation");
    assert_eq!(take_track.times.first(), Some(&0.0));
    assert_eq!(second_track.times.first(), Some(&-0.5));
    assert_ne!(
        take_track.times, second_track.times,
        "each active source take must retain its distinct baked time domain"
    );
}

/// The private FBX-to-GLB stage maps public source selectors by exact named
/// normalized identity. Ambiguous roots or joint topologies must refuse before
/// either public member is replaced.
#[cfg(feature = "fbx")]
#[test]
fn fbx_rest_bind_refuses_ambiguous_staged_selector_mappings_atomically() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let prior_artifact = b"prior artifact";
    let prior_evidence = b"prior evidence";
    std::fs::write(dir.path().join("out.glb"), prior_artifact).unwrap();
    std::fs::write(dir.path().join("out.json"), prior_evidence).unwrap();

    let duplicate_root = two_skin_fbx_fixture().replacen("Model::root-two", "Model::root", 1);
    std::fs::write(dir.path().join("duplicate-root.fbx"), duplicate_root)
        .expect("writes duplicate-root fixture");
    let root_refusal = fbx_rest_bind_command_for_selectors(
        dir.path(),
        "duplicate-root.fbx",
        "out.glb",
        "out.json",
        "0.01",
        "json",
        ("1", "2"),
    )
    .output()
    .expect("runs ambiguous root mapping");
    assert_eq!(root_refusal.status.code(), Some(1));
    let root_record: Value = serde_json::from_slice(&root_refusal.stdout).unwrap();
    assert_fbx_schema_valid(&root_record);
    assert_eq!(root_record["rejection"]["stage"], "rewrite");
    assert_eq!(root_record["rejection"]["kind"], "staged-selector-mismatch");
    assert_eq!(
        std::fs::read(dir.path().join("out.glb")).unwrap(),
        prior_artifact
    );
    assert_eq!(
        std::fs::read(dir.path().join("out.json")).unwrap(),
        prior_evidence
    );

    std::fs::write(
        dir.path().join("repeated-joint.fbx"),
        repeated_selected_joint_name_fbx_fixture(),
    )
    .expect("writes repeated selected-joint fixture");
    let repeated_refusal = fbx_rest_bind_command(
        dir.path(),
        "repeated-joint.fbx",
        "out.glb",
        "out.json",
        "0.01",
        "json",
    )
    .output()
    .expect("runs repeated selected-joint mapping");
    assert_eq!(repeated_refusal.status.code(), Some(1));
    assert!(repeated_refusal.stderr.is_empty());
    let repeated_record: Value = serde_json::from_slice(&repeated_refusal.stdout).unwrap();
    assert_fbx_schema_valid(&repeated_record);
    assert_eq!(repeated_record["rejection"]["stage"], "rewrite");
    assert_eq!(
        repeated_record["rejection"]["kind"],
        "staged-selector-mismatch"
    );
    assert_eq!(
        std::fs::read(dir.path().join("out.glb")).unwrap(),
        prior_artifact
    );
    assert_eq!(
        std::fs::read(dir.path().join("out.json")).unwrap(),
        prior_evidence
    );

    std::fs::write(
        dir.path().join("duplicate-skin.fbx"),
        ambiguous_skin_fbx_fixture(),
    )
    .expect("writes ambiguous skin fixture");
    let skin_refusal = fbx_rest_bind_command_for_selectors(
        dir.path(),
        "duplicate-skin.fbx",
        "out.glb",
        "out.json",
        "0.01",
        "json",
        ("1", "1"),
    )
    .output()
    .expect("runs ambiguous skin mapping");
    assert_eq!(skin_refusal.status.code(), Some(1));
    let skin_record: Value = serde_json::from_slice(&skin_refusal.stdout).unwrap();
    assert_fbx_schema_valid(&skin_record);
    assert_eq!(skin_record["rejection"]["stage"], "rewrite");
    assert_eq!(skin_record["rejection"]["kind"], "staged-selector-mismatch");
    assert_eq!(
        std::fs::read(dir.path().join("out.glb")).unwrap(),
        prior_artifact
    );
    assert_eq!(
        std::fs::read(dir.path().join("out.json")).unwrap(),
        prior_evidence
    );
}

#[cfg(feature = "fbx")]
#[test]
fn fbx_whole_document_and_invalid_rest_bind_refuse_without_publication() {
    let dir = tempfile::tempdir().expect("temporary directory");
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../animsmith-fbx/testdata/rigged_triangle.fbx"),
        dir.path().join("rig.fbx"),
    )
    .expect("copies self-authored FBX fixture");
    let whole = animsmith()
        .current_dir(dir.path())
        .args([
            "scale",
            "whole-document",
            "rig.fbx",
            "-o",
            "out.glb",
            "--factor",
            "0.01",
            "--evidence",
            "out.json",
            "--format",
            "json",
        ])
        .output()
        .expect("runs unsupported FBX whole-document scale");
    assert_eq!(whole.status.code(), Some(2));
    assert!(whole.stdout.is_empty());
    assert!(stderr(&whole).contains("limited to rest-bind"));

    let prior_artifact = b"prior artifact";
    let prior_evidence = b"prior evidence";
    std::fs::write(dir.path().join("out.glb"), prior_artifact).unwrap();
    std::fs::write(dir.path().join("out.json"), prior_evidence).unwrap();
    let refused =
        fbx_rest_bind_command(dir.path(), "rig.fbx", "out.glb", "out.json", "1.0", "json")
            .output()
            .expect("runs factor-mismatched FBX rest-bind scale");
    assert_eq!(refused.status.code(), Some(1));
    assert!(refused.stderr.is_empty());
    let refusal: Value = serde_json::from_slice(&refused.stdout).expect("JSON refusal");
    assert_fbx_schema_valid(&refusal);
    assert_eq!(refusal["rejection"]["kind"], "factor-mismatch");
    assert_eq!(
        std::fs::read(dir.path().join("out.glb")).unwrap(),
        prior_artifact
    );
    assert_eq!(
        std::fs::read(dir.path().join("out.json")).unwrap(),
        prior_evidence
    );

    let text = fbx_rest_bind_command(
        dir.path(),
        "rig.fbx",
        "text.glb",
        "text.json",
        "0.01",
        "text",
    )
    .output()
    .expect("runs FBX rest-bind text success");
    assert_eq!(text.status.code(), Some(0), "stderr:\n{}", stderr(&text));
    assert!(text.stderr.is_empty());
    assert!(stdout(&text).contains("rest-bind factor 0.01"));

    let text_refusal = fbx_rest_bind_command(
        dir.path(),
        "rig.fbx",
        "text-refusal.glb",
        "text-refusal.json",
        "1.0",
        "text",
    )
    .output()
    .expect("runs FBX rest-bind text refusal");
    assert_eq!(text_refusal.status.code(), Some(1));
    assert!(text_refusal.stdout.is_empty());
    assert!(stderr(&text_refusal).contains("factor-mismatch"));

    for output_name in ["wrong.gltf", "wrong.fbx", "wrong"] {
        let wrong_extension = fbx_rest_bind_command(
            dir.path(),
            "rig.fbx",
            output_name,
            "wrong.json",
            "0.01",
            "json",
        )
        .output()
        .expect("runs invalid FBX output extension");
        assert_eq!(wrong_extension.status.code(), Some(2));
        assert!(wrong_extension.stdout.is_empty());
        assert!(stderr(&wrong_extension).contains("must be .glb"));
        assert!(!dir.path().join(output_name).exists());
    }

    let unsupported = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../animsmith-fbx/testdata/rigged_triangle.fbx"),
    )
    .expect("reads analytic FBX fixture")
    .replacen(
        "ObjectType: \"Geometry\" { Count: 1 }",
        "ObjectType: \"Geometry\" { Count: 2 }",
        1,
    )
    .replacen(
        "\tAnimationStack: 3001",
        "\tGeometry: 5001, \"Geometry::detached\", \"Mesh\" {\n\t\tVertices: *9 { a: 0,0,0,100,0,0,0,100,0 }\n\t\tPolygonVertexIndex: *3 { a: 0,1,-3 }\n\t}\n\tAnimationStack: 3001",
        1,
    );
    std::fs::write(dir.path().join("unsupported.fbx"), unsupported)
        .expect("writes unsupported FBX fixture");
    let inventory_refusal = fbx_rest_bind_command(
        dir.path(),
        "unsupported.fbx",
        "out.glb",
        "out.json",
        "0.01",
        "json",
    )
    .output()
    .expect("runs inventory-gated FBX refusal");
    assert_eq!(inventory_refusal.status.code(), Some(1));
    assert!(inventory_refusal.stderr.is_empty());
    let record: Value = serde_json::from_slice(&inventory_refusal.stdout).expect("v5 refusal");
    assert_fbx_schema_valid(&record);
    assert_eq!(record["rejection"]["kind"], "unsupported-source-domain");
    assert_eq!(record["capability"]["uninstanced_mesh_definition_count"], 1);
    assert_eq!(
        std::fs::read(dir.path().join("out.glb")).unwrap(),
        prior_artifact
    );
    assert_eq!(
        std::fs::read(dir.path().join("out.json")).unwrap(),
        prior_evidence
    );
}
