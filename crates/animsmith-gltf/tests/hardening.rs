//! Hostile-input hardening: structurally malformed containers must
//! produce `LoadError` (operator error), never a panic — and `fix`
//! must never report success for bytes it did not write.

use animsmith_core::glam::Quat;
use animsmith_core::measure::measure_document;
use animsmith_core::profile::ResolvedRoles;
use animsmith_core::{
    CheckCtx, CheckSelection, Config, MetricGrids, Severity, evaluate_checks, mechanical_checks,
};
use animsmith_gltf::LoadError;
use animsmith_gltf::fix::{FixSession, Repair};
use std::path::{Path, PathBuf};

fn unique_temp_dir(name: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(&format!("animsmith-harden-{name}-"))
        .tempdir()
        .unwrap()
}

/// 3 keyframe times but only 2 VEC4 output values, via a data-URI
/// buffer (validates within the container; the cross-accessor count
/// mismatch is ours to catch).
const COUNT_MISMATCH_GLTF: &str = r#"{
  "asset": { "version": "2.0" },
  "buffers": [{ "uri": "data:application/octet-stream;base64,AAAAAAAAAD8AAIA/AAAAAAAAAAAAAAAAAACAPwAAAAAAAAAAAAAAAAAAgD8=", "byteLength": 44 }],
  "bufferViews": [
    { "buffer": 0, "byteOffset": 0, "byteLength": 12 },
    { "buffer": 0, "byteOffset": 12, "byteLength": 32 }
  ],
  "accessors": [
    { "bufferView": 0, "componentType": 5126, "count": 3, "type": "SCALAR", "min": [0], "max": [1] },
    { "bufferView": 1, "componentType": 5126, "count": 2, "type": "VEC4" }
  ],
  "nodes": [{ "name": "root" }],
  "animations": [{
    "name": "bad",
    "samplers": [{ "input": 0, "output": 1, "interpolation": "LINEAR" }],
    "channels": [{ "sampler": 0, "target": { "node": 0, "path": "rotation" } }]
  }],
  "scenes": [{ "nodes": [0] }],
  "scene": 0
}"#;

/// First keyframe time is NaN (0x7FC00000); 3 valid identity quats.
/// Accessor min/max lie, as a hostile file would.
const NAN_TIME_GLTF: &str = r#"{
  "asset": { "version": "2.0" },
  "buffers": [{ "uri": "data:application/octet-stream;base64,AADAfwAAAD8AAIA/AAAAAAAAAAAAAAAAAACAPwAAAAAAAAAAAAAAAAAAgD8AAAAAAAAAAAAAAAAAAIA/", "byteLength": 60 }],
  "bufferViews": [
    { "buffer": 0, "byteOffset": 0, "byteLength": 12 },
    { "buffer": 0, "byteOffset": 12, "byteLength": 48 }
  ],
  "accessors": [
    { "bufferView": 0, "componentType": 5126, "count": 3, "type": "SCALAR", "min": [0], "max": [1] },
    { "bufferView": 1, "componentType": 5126, "count": 3, "type": "VEC4" }
  ],
  "nodes": [{ "name": "root" }],
  "animations": [{
    "name": "poisoned",
    "samplers": [{ "input": 0, "output": 1, "interpolation": "LINEAR" }],
    "channels": [{ "sampler": 0, "target": { "node": 0, "path": "rotation" } }]
  }],
  "scenes": [{ "nodes": [0] }],
  "scene": 0
}"#;

/// CUBICSPLINE needs 3 output values per key (in-tangent, value,
/// out-tangent). 2 keys with 3 values (not 6) is malformed — this
/// exercises the `per_key = 3` branch of the validator.
const CUBIC_COUNT_MISMATCH_GLTF: &str = r#"{
  "asset": { "version": "2.0" },
  "buffers": [{ "uri": "data:application/octet-stream;base64,AAAAAAAAgD8AAAAAAAAAAAAAAAAAAIA/AAAAAAAAAAAAAAAAAACAPwAAAAAAAAAAAAAAAAAAgD8=", "byteLength": 56 }],
  "bufferViews": [
    { "buffer": 0, "byteOffset": 0, "byteLength": 8 },
    { "buffer": 0, "byteOffset": 8, "byteLength": 48 }
  ],
  "accessors": [
    { "bufferView": 0, "componentType": 5126, "count": 2, "type": "SCALAR", "min": [0], "max": [1] },
    { "bufferView": 1, "componentType": 5126, "count": 3, "type": "VEC4" }
  ],
  "nodes": [{ "name": "root" }],
  "animations": [{
    "name": "bad-cubic",
    "samplers": [{ "input": 0, "output": 1, "interpolation": "CUBICSPLINE" }],
    "channels": [{ "sampler": 0, "target": { "node": 0, "path": "rotation" } }]
  }],
  "scenes": [{ "nodes": [0] }],
  "scene": 0
}"#;

/// An animation channel that resolves to zero keyframes.
const ZERO_KEY_GLTF: &str = r#"{
  "asset": { "version": "2.0" },
  "buffers": [{ "uri": "data:application/octet-stream;base64,", "byteLength": 0 }],
  "bufferViews": [
    { "buffer": 0, "byteOffset": 0, "byteLength": 0 },
    { "buffer": 0, "byteOffset": 0, "byteLength": 0 }
  ],
  "accessors": [
    { "bufferView": 0, "componentType": 5126, "count": 0, "type": "SCALAR" },
    { "bufferView": 1, "componentType": 5126, "count": 0, "type": "VEC4" }
  ],
  "nodes": [{ "name": "root" }],
  "animations": [{
    "name": "empty",
    "samplers": [{ "input": 0, "output": 1, "interpolation": "LINEAR" }],
    "channels": [{ "sampler": 0, "target": { "node": 0, "path": "rotation" } }]
  }],
  "scenes": [{ "nodes": [0] }],
  "scene": 0
}"#;

#[test]
fn loader_rejects_times_values_count_mismatch() {
    let dir = unique_temp_dir("count-mismatch");
    let path = dir.path().join("bad.gltf");
    std::fs::write(&path, COUNT_MISMATCH_GLTF).unwrap();

    let err = animsmith_gltf::load(&path).expect_err("mismatch must be rejected");
    assert!(matches!(err, LoadError::Malformed(_)), "{err}");
    assert!(
        err.to_string()
            .contains("3 keyframe times but 2 output values"),
        "{err}"
    );
}

#[test]
fn loader_rejects_cubic_triplet_count_mismatch() {
    let dir = unique_temp_dir("cubic-count-mismatch");
    let path = dir.path().join("bad-cubic.gltf");
    std::fs::write(&path, CUBIC_COUNT_MISMATCH_GLTF).unwrap();

    let err = animsmith_gltf::load(&path).expect_err("cubic mismatch must be rejected");
    assert!(matches!(err, LoadError::Malformed(_)), "{err}");
    // 2 keys x 3 = 6 expected, 3 present.
    assert!(
        err.to_string()
            .contains("2 keyframe times but 3 output values (expected 6)"),
        "{err}"
    );
}

#[test]
fn loader_rejects_zero_key_channel() {
    let dir = unique_temp_dir("zero-key");
    let path = dir.path().join("empty.gltf");
    std::fs::write(&path, ZERO_KEY_GLTF).unwrap();

    let err = animsmith_gltf::load(&path).expect_err("zero-key channel must be rejected");
    assert!(matches!(err, LoadError::Malformed(_)), "{err}");
    assert!(err.to_string().contains("zero keyframes"), "{err}");
}

/// A well-formed container whose only defect is the animation channel
/// `target` — `{path}` for the property, `{node}` for the node index.
/// `gltf`'s validator never inspects channel targets, so both slip past
/// `from_slice` and would panic in the high-level getters.
fn gltf_with_channel_target(path: &str, node: usize) -> String {
    format!(
        r#"{{
  "asset": {{ "version": "2.0" }},
  "buffers": [{{ "uri": "data:application/octet-stream;base64,AAAAAAAAAD8AAIA/AAAAAAAAAAAAAAAAAACAPwAAAAAAAAAAAAAAAAAAgD8AAAAAAAAAAAAAAAAAAIA/", "byteLength": 60 }}],
  "bufferViews": [
    {{ "buffer": 0, "byteOffset": 0, "byteLength": 12 }},
    {{ "buffer": 0, "byteOffset": 12, "byteLength": 48 }}
  ],
  "accessors": [
    {{ "bufferView": 0, "componentType": 5126, "count": 3, "type": "SCALAR", "min": [0], "max": [1] }},
    {{ "bufferView": 1, "componentType": 5126, "count": 3, "type": "VEC4" }}
  ],
  "nodes": [{{ "name": "root" }}],
  "animations": [{{
    "name": "bad",
    "samplers": [{{ "input": 0, "output": 1, "interpolation": "LINEAR" }}],
    "channels": [{{ "sampler": 0, "target": {{ "node": {node}, "path": "{path}" }} }}]
  }}],
  "scenes": [{{ "nodes": [0] }}],
  "scene": 0
}}"#
    )
}

#[test]
fn loader_rejects_unknown_animation_target_path() {
    let dir = unique_temp_dir("bad-target-path");
    let path = dir.path().join("bad-path.gltf");
    std::fs::write(&path, gltf_with_channel_target("wobble", 0)).unwrap();

    // Would otherwise panic in gltf's `Target::property().unwrap()`.
    let err = animsmith_gltf::load(&path).expect_err("unknown target path must be rejected");
    assert!(matches!(err, LoadError::Malformed(_)), "{err}");
    assert!(err.to_string().contains("unknown target path"), "{err}");

    // The fix path shares the guard.
    let out = dir.path().join("out.gltf");
    let err = FixSession::apply_to_path(&path, &out, Repair::QuatFlip)
        .expect_err("fix must reject it too");
    assert!(
        matches!(err, animsmith_gltf::FixError::Load(LoadError::Malformed(_))),
        "{err}"
    );
}

/// A rotation channel whose sampler output accessor is UNSIGNED_INT
/// (componentType 5125) — a spec-valid component type that is nonsensical
/// for animation and hits `unreachable!()` in gltf's `read_outputs`.
const U32_ROTATION_OUTPUT_GLTF: &str = r#"{
  "asset": { "version": "2.0" },
  "buffers": [{ "uri": "data:application/octet-stream;base64,AAAAAAAAgD8AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", "byteLength": 60 }],
  "bufferViews": [
    { "buffer": 0, "byteOffset": 0, "byteLength": 12 },
    { "buffer": 0, "byteOffset": 12, "byteLength": 48 }
  ],
  "accessors": [
    { "bufferView": 0, "componentType": 5126, "count": 3, "type": "SCALAR", "min": [0], "max": [1] },
    { "bufferView": 1, "componentType": 5125, "count": 3, "type": "VEC4" }
  ],
  "nodes": [{ "name": "root" }],
  "animations": [{
    "name": "bad",
    "samplers": [{ "input": 0, "output": 1, "interpolation": "LINEAR" }],
    "channels": [{ "sampler": 0, "target": { "node": 0, "path": "rotation" } }]
  }],
  "scenes": [{ "nodes": [0] }],
  "scene": 0
}"#;

#[test]
fn loader_rejects_unsigned_int_animation_output() {
    let dir = unique_temp_dir("u32-output");
    let path = dir.path().join("u32.gltf");
    std::fs::write(&path, U32_ROTATION_OUTPUT_GLTF).unwrap();

    // Would otherwise hit gltf's `read_outputs` `unreachable!()`.
    let err = animsmith_gltf::load(&path).expect_err("U32 animation output must be rejected");
    assert!(matches!(err, LoadError::Malformed(_)), "{err}");
    assert!(err.to_string().contains("UNSIGNED_INT"), "{err}");
}

#[test]
fn loader_rejects_out_of_range_animation_target_node() {
    let dir = unique_temp_dir("bad-target-node");
    let path = dir.path().join("bad-node.gltf");
    // Only node 0 exists; target node 9 is out of range.
    std::fs::write(&path, gltf_with_channel_target("rotation", 9)).unwrap();

    // Would otherwise panic in gltf's `Target::node().unwrap()`.
    let err = animsmith_gltf::load(&path).expect_err("out-of-range target node must be rejected");
    assert!(matches!(err, LoadError::Malformed(_)), "{err}");
    assert!(err.to_string().contains("out of range"), "{err}");
}

/// A pure node cycle with no root: node 0 lists node 1 as its child and
/// node 1 lists node 0 back. Neither node is a root, so the DFS never
/// descends into the cycle at all — both nodes stay unreached and the
/// post-DFS reachability check rejects the graph (`LoadError::Topology`)
/// rather than dropping the unreachable subtree and silently loading a
/// bone-less skeleton.
const NODE_CYCLE_GLTF: &str = r#"{
  "asset": { "version": "2.0" },
  "nodes": [
    { "name": "a", "children": [1] },
    { "name": "b", "children": [0] }
  ],
  "scenes": [{ "nodes": [] }],
  "scene": 0
}"#;

/// Two roots both claim node 2 as a child — a node with two parents,
/// which glTF forbids (the graph is no longer a forest). Recovering would
/// force an arbitrary winner whose bone id could sort *after* the child,
/// silently corrupting FK output, so the loader rejects it outright.
const NODE_MULTIPARENT_GLTF: &str = r#"{
  "asset": { "version": "2.0" },
  "nodes": [
    { "name": "A", "children": [2] },
    { "name": "B", "children": [2] },
    { "name": "C" }
  ],
  "scenes": [{ "nodes": [0, 1] }],
  "scene": 0
}"#;

#[test]
fn loader_rejects_cyclic_node_graph() {
    let dir = unique_temp_dir("node-cycle");
    let path = dir.path().join("cycle.gltf");
    std::fs::write(&path, NODE_CYCLE_GLTF).unwrap();

    // A rootless cycle is never descended into; it is caught as unreached
    // by the post-DFS reachability check and rejected, not loaded as a
    // partial skeleton (invariant-1).
    let err = animsmith_gltf::load(&path).expect_err("cyclic node graph must be rejected");
    assert!(matches!(err, LoadError::Topology(_)), "{err}");
    assert!(err.to_string().contains("cycle"), "{err}");
}

#[test]
fn loader_rejects_multiparent_node_graph() {
    let dir = unique_temp_dir("multiparent");
    let path = dir.path().join("dag.gltf");
    std::fs::write(&path, NODE_MULTIPARENT_GLTF).unwrap();

    // A node with two parents is not a forest; rejecting it avoids an
    // arbitrary parent choice that would silently corrupt FK output.
    let err = animsmith_gltf::load(&path).expect_err("multi-parent node graph must be rejected");
    assert!(matches!(err, LoadError::Topology(_)), "{err}");
    assert!(err.to_string().contains("one parent per node"), "{err}");
}

/// A cycle *entered* from a root: root(0) → A(1) → B(2), and B lists A as
/// its child (a back-edge). Unlike the rootless cycle, the entry node A is
/// reachable — but the back-edge makes A a child of both the root and B,
/// so this is caught as a *duplicate parent*, not as an unreachable cycle.
/// This pins the loader's documented claim that a root-entered cycle trips
/// the duplicate-parent check first; without a fixture that distinction is
/// asserted only in a comment.
const NODE_ROOT_ENTERED_CYCLE_GLTF: &str = r#"{
  "asset": { "version": "2.0" },
  "nodes": [
    { "name": "root", "children": [1] },
    { "name": "a", "children": [2] },
    { "name": "b", "children": [1] }
  ],
  "scenes": [{ "nodes": [0] }],
  "scene": 0
}"#;

/// The degenerate cycle: a single node listing itself as its own child. It
/// has a parent (itself) so it is not a root, and no root reaches it, so it
/// is caught by the reachability check like any other cycle.
const NODE_SELF_LOOP_GLTF: &str = r#"{
  "asset": { "version": "2.0" },
  "nodes": [{ "name": "ouroboros", "children": [0] }],
  "scenes": [{ "nodes": [] }],
  "scene": 0
}"#;

#[test]
fn loader_rejects_root_entered_cycle_as_duplicate_parent() {
    let dir = unique_temp_dir("root-entered-cycle");
    let path = dir.path().join("back-edge.gltf");
    std::fs::write(&path, NODE_ROOT_ENTERED_CYCLE_GLTF).unwrap();

    // The back-edge gives the entry node a second parent, so the loader
    // diagnoses it as a duplicate parent (not a bare cycle) — and either
    // way must reject, never loop or load a corrupt skeleton (invariant-1).
    let err = animsmith_gltf::load(&path).expect_err("root-entered cycle must be rejected");
    assert!(matches!(err, LoadError::Topology(_)), "{err}");
    assert!(err.to_string().contains("one parent per node"), "{err}");
}

#[test]
fn loader_rejects_self_loop_node() {
    let dir = unique_temp_dir("self-loop");
    let path = dir.path().join("selfloop.gltf");
    std::fs::write(&path, NODE_SELF_LOOP_GLTF).unwrap();

    let err = animsmith_gltf::load(&path).expect_err("self-loop node must be rejected");
    assert!(matches!(err, LoadError::Topology(_)), "{err}");
    assert!(err.to_string().contains("cycle"), "{err}");
}

fn glb_with_declared_length(total_len: u32) -> Vec<u8> {
    let mut glb = Vec::new();
    glb.extend_from_slice(b"glTF");
    glb.extend_from_slice(&2u32.to_le_bytes()); // version
    glb.extend_from_slice(&total_len.to_le_bytes()); // total length (the lie)
    glb.extend_from_slice(&0x0004_c000u32.to_le_bytes()); // JSON chunk length
    glb.extend_from_slice(b"JSON");
    glb.extend_from_slice(br#"{ "asset": { "version": "2.0" } }"#);
    glb
}

/// A GLB header whose declared total length dwarfs the file must not
/// drive a length-field allocation (invariant-1). The reader path in the
/// `gltf` crate pre-allocates `vec![0; declared_len]` before reading a
/// byte; `load` reads the file, validates the framing, and parses from
/// the slice instead. Found by the `gltf_load` fuzz target (see `fuzz/`).
#[test]
fn loader_rejects_glb_length_field_far_past_eof() {
    // Declared ~2.5 GiB against a 54-byte file.
    let glb = glb_with_declared_length(0x9800_0538);
    let dir = unique_temp_dir("glb-length-oom");
    let path = dir.path().join("huge-length.glb");
    std::fs::write(&path, &glb).unwrap();

    // No panic, no multi-gigabyte allocation — just a LoadError. The
    // message pins that our framing guard rejected it, not a downstream
    // parse (which is where the allocation would have happened).
    let err = animsmith_gltf::load(&path).expect_err("bogus GLB length must be rejected");
    assert!(matches!(err, LoadError::Buffer(_)), "{err}");
    assert!(err.to_string().contains("GLB header declares"), "{err}");
}

/// A declared length *below* the 12-byte header underflows the `gltf`
/// crate's `declared - HEADER_LEN` subtraction — benign wrapping in
/// release, but a panic under the overflow checks every debug build and
/// `cargo test` run with. `load` rejects it first. Found by the
/// `gltf_fix_quat_hemisphere` fuzz target (see `fuzz/`).
#[test]
fn loader_rejects_glb_length_field_below_header_size() {
    let glb = glb_with_declared_length(0); // 0 - 12 would underflow
    let dir = unique_temp_dir("glb-length-underflow");
    let path = dir.path().join("tiny-length.glb");
    std::fs::write(&path, &glb).unwrap();

    let err = animsmith_gltf::load(&path).expect_err("sub-header GLB length must be rejected");
    assert!(matches!(err, LoadError::Buffer(_)), "{err}");
    assert!(err.to_string().contains("GLB header declares"), "{err}");

    // The fix path shares the same guard.
    let out = dir.path().join("out.glb");
    let err = FixSession::apply_to_path(&path, &out, Repair::QuatFlip)
        .expect_err("fix must reject it too");
    assert!(
        matches!(err, animsmith_gltf::FixError::Load(LoadError::Buffer(_))),
        "{err}"
    );
}

fn gltf_with_buffer_uri(uri: &str) -> String {
    format!(
        r#"{{
  "asset": {{ "version": "2.0" }},
  "buffers": [{{ "uri": "{uri}", "byteLength": 1 }}],
  "nodes": [{{ "name": "root" }}],
  "scenes": [{{ "nodes": [0] }}],
  "scene": 0
}}"#
    )
}

#[test]
fn loader_rejects_unsafe_external_buffer_uris() {
    let dir = unique_temp_dir("load-unsafe-uri");
    // Every containment branch: parent traversal, absolute path,
    // backslash, and a bare `..`. All are LoadError::Buffer, never a
    // read outside the input's directory.
    for (label, uri) in [
        ("parent", "../escape.bin"),
        ("absolute", "/etc/passwd"),
        ("backslash", "..\\\\escape.bin"),
        ("bare-dotdot", ".."),
    ] {
        let path = dir.path().join(format!("{label}.gltf"));
        std::fs::write(&path, gltf_with_buffer_uri(uri)).unwrap();
        let err = animsmith_gltf::load(&path).expect_err("unsafe URI must be rejected");
        assert!(
            matches!(err, LoadError::Buffer(_)),
            "{label}: wrong variant: {err}"
        );
        assert!(
            err.to_string().contains("unsafe external buffer URI"),
            "{label}: {err}"
        );
    }
}

#[test]
fn nan_key_time_measures_without_panicking_and_lints_as_error() {
    let dir = unique_temp_dir("nan-time");
    let path = dir.path().join("nan.gltf");
    std::fs::write(&path, NAN_TIME_GLTF).unwrap();

    // The NaN is semantic, not structural: the file loads, sampling
    // survives it, and the `nan` check reports it as an error.
    let doc = animsmith_gltf::load(&path).expect("NaN times load; checks judge them");
    let roles = ResolvedRoles::default();
    let config = Config::default();
    let grids = MetricGrids::new(&doc);
    let measurements = measure_document(&grids, &roles, &config);
    assert!(measurements.contains_key("poisoned"));

    let ctx = CheckCtx::new(&grids, &roles, &config);
    let findings: Vec<_> = evaluate_checks(&ctx, &mechanical_checks(), CheckSelection::All)
        .expect("valid built-in catalog")
        .into_iter()
        .flat_map(|check| check.findings().to_vec())
        .collect();
    assert!(
        findings.iter().any(|f| f.check_id == "nan"
            && f.severity == Severity::Error
            && f.message.contains("non-finite key time")),
        "findings: {findings:?}"
    );
}

/// A BIN-less GLB whose only buffer is an external `ext.bin` holding
/// hemisphere-flipped rotation keys.
fn write_glb_with_external_buffer(dir: &Path) -> PathBuf {
    let json = r#"{
  "asset": { "version": "2.0" },
  "buffers": [{ "uri": "ext.bin", "byteLength": 100 }],
  "bufferViews": [
    { "buffer": 0, "byteOffset": 0, "byteLength": 20 },
    { "buffer": 0, "byteOffset": 20, "byteLength": 80 }
  ],
  "accessors": [
    { "bufferView": 0, "componentType": 5126, "count": 5, "type": "SCALAR", "min": [0], "max": [1] },
    { "bufferView": 1, "componentType": 5126, "count": 5, "type": "VEC4" }
  ],
  "nodes": [{ "name": "root" }],
  "animations": [{
    "name": "sway",
    "samplers": [{ "input": 0, "output": 1, "interpolation": "LINEAR" }],
    "channels": [{ "sampler": 0, "target": { "node": 0, "path": "rotation" } }]
  }],
  "scenes": [{ "nodes": [0] }],
  "scene": 0
}"#;
    let mut json_bytes = json.as_bytes().to_vec();
    while !json_bytes.len().is_multiple_of(4) {
        json_bytes.push(b' ');
    }
    let total = 12 + 8 + json_bytes.len();
    let mut glb = Vec::new();
    glb.extend_from_slice(b"glTF");
    glb.extend_from_slice(&2u32.to_le_bytes());
    glb.extend_from_slice(&(total as u32).to_le_bytes());
    glb.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    glb.extend_from_slice(b"JSON");
    glb.extend_from_slice(&json_bytes);

    let mut ext = Vec::new();
    for t in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
        ext.extend_from_slice(&t.to_le_bytes());
    }
    let mut quats: Vec<Quat> = [0.0f32, 0.4, 0.8, 1.2, 1.6]
        .iter()
        .map(|&a| Quat::from_rotation_y(a))
        .collect();
    quats[1] = -quats[1];
    quats[3] = -quats[3];
    for q in &quats {
        for c in q.to_array() {
            ext.extend_from_slice(&c.to_le_bytes());
        }
    }
    assert_eq!(ext.len(), 100);

    let glb_path = dir.join("sway.glb");
    std::fs::write(&glb_path, glb).unwrap();
    std::fs::write(dir.join("ext.bin"), ext).unwrap();
    glb_path
}

#[test]
fn glb_external_buffer_fix_writes_the_buffer_not_a_false_success() {
    let dir = unique_temp_dir("glb-ext-fix");
    let glb = write_glb_with_external_buffer(dir.path());
    assert_eq!(
        FixSession::inspect(&glb, Repair::QuatFlip)
            .expect("inspects")
            .total_fixed(),
        2
    );

    // Fix into a DIFFERENT directory: the repaired ext.bin must land
    // next to the output, or the "fixed" report is a lie.
    let out_dir = dir.path().join("out");
    std::fs::create_dir_all(&out_dir).unwrap();
    let out_glb = out_dir.join("fixed.glb");
    let report = FixSession::apply_to_path(&glb, &out_glb, Repair::QuatFlip).expect("fixes");
    assert_eq!(report.total_fixed(), 2);

    assert!(out_dir.join("ext.bin").exists(), "patched buffer written");
    assert_eq!(
        FixSession::inspect(&out_glb, Repair::QuatFlip)
            .expect("re-inspects output")
            .total_fixed(),
        0,
        "output must actually be repaired"
    );
    // The input pair is untouched.
    assert_eq!(
        FixSession::inspect(&glb, Repair::QuatFlip)
            .expect("re-inspects input")
            .total_fixed(),
        2
    );
}
