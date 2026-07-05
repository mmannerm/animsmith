//! Hostile-input hardening: structurally malformed containers must
//! produce `LoadError` (operator error), never a panic — and `fix`
//! must never report success for bytes it did not write.

use animsmith_core::glam::Quat;
use animsmith_core::measure::measure_document;
use animsmith_core::profile::ResolvedRoles;
use animsmith_core::{CheckCtx, Config, MetricGrids, Severity, mechanical_checks, run_checks};
use animsmith_gltf::LoadError;
use std::path::{Path, PathBuf};

fn unique_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("animsmith-harden-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
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
    let path = dir.join("bad.gltf");
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
    let path = dir.join("bad-cubic.gltf");
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
    let path = dir.join("empty.gltf");
    std::fs::write(&path, ZERO_KEY_GLTF).unwrap();

    let err = animsmith_gltf::load(&path).expect_err("zero-key channel must be rejected");
    assert!(matches!(err, LoadError::Malformed(_)), "{err}");
    assert!(err.to_string().contains("zero keyframes"), "{err}");
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
        let path = dir.join(format!("{label}.gltf"));
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
    let path = dir.join("nan.gltf");
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
    let findings = run_checks(&ctx, &mechanical_checks());
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
    let glb = write_glb_with_external_buffer(&dir);
    assert_eq!(
        animsmith_gltf::fix::inspect_quat_hemisphere(&glb)
            .expect("inspects")
            .total_fixed(),
        2
    );

    // Fix into a DIFFERENT directory: the repaired ext.bin must land
    // next to the output, or the "fixed" report is a lie.
    let out_dir = dir.join("out");
    std::fs::create_dir_all(&out_dir).unwrap();
    let out_glb = out_dir.join("fixed.glb");
    let report = animsmith_gltf::fix::fix_quat_hemisphere(&glb, &out_glb).expect("fixes");
    assert_eq!(report.total_fixed(), 2);

    assert!(out_dir.join("ext.bin").exists(), "patched buffer written");
    assert_eq!(
        animsmith_gltf::fix::inspect_quat_hemisphere(&out_glb)
            .expect("re-inspects output")
            .total_fixed(),
        0,
        "output must actually be repaired"
    );
    // The input pair is untouched.
    assert_eq!(
        animsmith_gltf::fix::inspect_quat_hemisphere(&glb)
            .expect("re-inspects input")
            .total_fixed(),
        2
    );
}
