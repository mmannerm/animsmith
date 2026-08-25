//! Black-box CLI coverage for selected source-node rest-world scale checks.
//!
//! The synthetic glTF deliberately keeps metre-sized geometry and a skin whose
//! inverse bind cancels the hierarchy's centimetre conversion.  That proves
//! the lint judges an attachment's inherited source transform rather than
//! mistaking compensated mesh deformation for a safe attachment transform.

use serde_json::{Value, json};
use std::path::Path;
use std::process::{Command, Output};

const OUTPUT_SCHEMA: &str = include_str!("../../../docs/schemas/output-v13.schema.json");
const OUTPUT_V10_SCHEMA: &str = include_str!("../../../docs/schemas/output-v10.schema.json");
const MEASUREMENTS_V15_SCHEMA: &str =
    include_str!("../../../docs/schemas/measurements-v15.schema.json");
const MEASUREMENTS_SCHEMA: &str =
    include_str!("../../../docs/schemas/measurements-v16.schema.json");

fn output_validator() -> jsonschema::Validator {
    let output: Value = serde_json::from_str(OUTPUT_SCHEMA).expect("valid output schema");
    let output_v10: Value =
        serde_json::from_str(OUTPUT_V10_SCHEMA).expect("valid historical output schema");
    let measurements: Value =
        serde_json::from_str(MEASUREMENTS_SCHEMA).expect("valid measurements schema");
    let measurements_v15: Value = serde_json::from_str(MEASUREMENTS_V15_SCHEMA)
        .expect("valid historical measurements schema");
    let registry = jsonschema::Registry::new()
        .add("urn:animsmith:schema:output:10", output_v10)
        .expect("registers historical output schema")
        .add("urn:animsmith:schema:measurements:15", measurements_v15)
        .expect("registers historical measurements schema")
        .add("urn:animsmith:schema:measurements:16", measurements)
        .expect("registers measurements schema")
        .prepare()
        .expect("prepares measurements schema registry");
    jsonschema::options()
        .with_registry(&registry)
        .build(&output)
        .expect("compiles output schema")
}

fn append_f32s(bytes: &mut Vec<u8>, values: impl IntoIterator<Item = f32>) -> (usize, usize) {
    while !bytes.len().is_multiple_of(4) {
        bytes.push(0);
    }
    let offset = bytes.len();
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    (offset, bytes.len() - offset)
}

fn write_compensated_attachment_gltf(path: &Path, geometry_extent_m: f32) {
    let mut bytes = Vec::new();
    let (positions_offset, positions_length) = append_f32s(
        &mut bytes,
        [
            0.0_f32,
            0.0,
            0.0,
            geometry_extent_m,
            0.0,
            0.0,
            0.0,
            geometry_extent_m,
            0.0,
        ],
    );
    let joints_offset = bytes.len();
    bytes.extend_from_slice(&[0_u8, 0, 0, 0].repeat(3));
    let joints_length = bytes.len() - joints_offset;
    let (weights_offset, weights_length) = append_f32s(
        &mut bytes,
        [
            1.0_f32, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0,
        ],
    );
    // The root's 0.01 rest-world scale is intentionally cancelled for the
    // skinned mesh by this inverse bind.  The selected socket has no such
    // compensation and must therefore still be reported at 0.01.
    let (inverse_bind_offset, inverse_bind_length) = append_f32s(
        &mut bytes,
        [
            100.0_f32, 0.0, 0.0, 0.0, 0.0, 100.0, 0.0, 0.0, 0.0, 0.0, 100.0, 0.0, 0.0, 0.0, 0.0,
            1.0,
        ],
    );
    let buffer = path.with_extension("bin");
    std::fs::write(&buffer, &bytes).expect("writes synthetic buffer");

    let document = json!({
        "asset": { "version": "2.0" },
        "buffers": [{
            "uri": buffer.file_name().and_then(|name| name.to_str()).expect("UTF-8 buffer name"),
            "byteLength": bytes.len()
        }],
        "bufferViews": [
            { "buffer": 0, "byteOffset": positions_offset, "byteLength": positions_length },
            { "buffer": 0, "byteOffset": joints_offset, "byteLength": joints_length },
            { "buffer": 0, "byteOffset": weights_offset, "byteLength": weights_length },
            { "buffer": 0, "byteOffset": inverse_bind_offset, "byteLength": inverse_bind_length }
        ],
        "accessors": [
            { "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3", "min": [0.0, 0.0, 0.0], "max": [geometry_extent_m, geometry_extent_m, 0.0] },
            { "bufferView": 1, "componentType": 5121, "count": 3, "type": "VEC4" },
            { "bufferView": 2, "componentType": 5126, "count": 3, "type": "VEC4" },
            { "bufferView": 3, "componentType": 5126, "count": 1, "type": "MAT4" }
        ],
        "meshes": [{ "name": "metre-triangle", "primitives": [{
            "attributes": { "POSITION": 0, "JOINTS_0": 1, "WEIGHTS_0": 2 }
        }] }],
        "nodes": [
            { "name": "centimetre-source-root", "scale": [0.01, 0.01, 0.01], "children": [1, 2, 3] },
            { "name": "attachment-socket" },
            { "name": "joint-root" },
            { "name": "metre-sized-skinned-mesh", "mesh": 0, "skin": 0 }
        ],
        "skins": [{ "name": "compensated-skin", "joints": [2], "inverseBindMatrices": 3 }],
        "scenes": [{ "nodes": [0] }],
        "scene": 0
    });
    std::fs::write(
        path,
        serde_json::to_vec_pretty(&document).expect("serializes synthetic glTF"),
    )
    .expect("writes synthetic glTF");
}

fn run_lint(input: &Path, config: &Path, format: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .arg("--config")
        .arg(config)
        .arg("lint")
        .arg(input)
        .args(["--select", "rest-world-scale", "--format", format])
        .output()
        .expect("runs animsmith lint")
}

fn run_measure(input: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .arg("measure")
        .arg(input)
        .args(["--format", "json"])
        .output()
        .expect("runs animsmith measure")
}

#[test]
fn lint_selected_rest_world_scale_reports_compensated_attachment_in_every_output_format() {
    let dir = tempfile::tempdir().expect("creates temp directory");
    let input = dir.path().join("compensated-attachment.gltf");
    let config = dir.path().join("animsmith.toml");
    write_compensated_attachment_gltf(&input, 1.0);
    std::fs::write(
        &config,
        concat!(
            "[checks.rest-world-scale]\n",
            "node_selectors = [\"attachment-socket\"]\n",
            "expected_uniform_scale = 1.25\n",
            "uniform_scale_tolerance = 0.2\n",
        ),
    )
    .expect("writes lint config");

    let measure_output = run_measure(&input);
    assert!(
        measure_output.status.success(),
        "measure failed: {}",
        String::from_utf8_lossy(&measure_output.stderr)
    );
    let measure_report: Value =
        serde_json::from_slice(&measure_output.stdout).expect("measure emits JSON");
    assert!(
        output_validator().is_valid(&measure_report),
        "measure output satisfies the published schema: {measure_report:#}"
    );
    let measurements = &measure_report["files"][0]["measurements"];
    let mesh = &measurements["mesh_definitions"][0];
    assert_eq!(mesh["geometry_aabb"]["min"], json!([0.0, 0.0, 0.0]));
    assert_eq!(mesh["geometry_aabb"]["max"], json!([1.0, 1.0, 0.0]));
    let joint = &measurements["skeleton_nodes"][2];
    assert_eq!(joint["name"], "joint-root");
    assert_eq!(
        joint["rest_world_linear"]["classification"],
        "uniform_scaled"
    );
    assert!(
        (joint["rest_world_linear"]["uniform_scale"]
            .as_f64()
            .expect("joint rest-world uniform factor")
            - 0.01)
            .abs()
            < 1.0e-8
    );
    let skin = &measurements["skins"][0];
    assert_eq!(skin["inverse_bind_accessor"]["status"], "available");
    assert_eq!(skin["inverse_bind_accessor"]["matrices"][0][0], 100.0);
    assert_eq!(
        skin["joints"][0]["joint_bind_to_mesh"]["linear"]["classification"],
        "uniform_scaled"
    );
    assert_eq!(
        skin["joints"][0]["mesh_bind_world"]["linear"]["classification"],
        "unit_orthonormal"
    );
    assert_eq!(
        skin["joints"][0]["mesh_bind_world"]["linear"]["uniform_scale"],
        1.0
    );
    assert_eq!(
        skin["joints"][0]["mesh_bind_world"]["matrix"],
        json!([
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0
        ])
    );

    let json_output = run_lint(&input, &config, "json");
    assert!(
        json_output.status.success(),
        "lint failed: {}",
        String::from_utf8_lossy(&json_output.stderr)
    );
    let report: Value = serde_json::from_slice(&json_output.stdout).expect("lint emits JSON");
    let validator = output_validator();
    assert!(
        validator.is_valid(&report),
        "lint output satisfies the published schema: {report:#}"
    );
    let check = report["files"][0]["checks"]
        .as_array()
        .expect("check records")
        .iter()
        .find(|check| check["check_id"] == "rest-world-scale")
        .expect("selected rest-world-scale record");
    assert_eq!(check["selection"], "selected");
    assert_eq!(check["applicability"], "applicable");
    assert_eq!(check["evaluation"], "complete");
    let finding = check["findings"]
        .as_array()
        .expect("findings")
        .first()
        .expect("finding");
    assert_eq!(
        finding["node"],
        "#0(centimetre-source-root)/#1(attachment-socket)"
    );
    assert!(
        (finding["measured"]
            .as_f64()
            .expect("numeric measured factor")
            - 0.01)
            .abs()
            < 1.0e-8,
        "finding retains the inherited centimetre factor: {finding:#}"
    );
    assert_eq!(finding["expected"], 1.25);

    let text_output = run_lint(&input, &config, "text");
    assert!(text_output.status.success(), "text lint exits successfully");
    let text = String::from_utf8(text_output.stdout).expect("text output is UTF-8");
    assert!(
        text.contains("warning[rest-world-scale]"),
        "stdout:\n{text}"
    );
    assert!(
        text.contains("#0(centimetre-source-root)/#1(attachment-socket)"),
        "stdout:\n{text}"
    );
    assert!(text.contains("measured 0.0100"), "stdout:\n{text}");

    let markdown_output = run_lint(&input, &config, "markdown");
    assert!(
        markdown_output.status.success(),
        "Markdown lint exits successfully"
    );
    let markdown = String::from_utf8(markdown_output.stdout).expect("Markdown output is UTF-8");
    assert!(
        markdown.contains("`rest-world-scale`"),
        "stdout:\n{markdown}"
    );
    assert!(
        markdown.contains("node `#0(centimetre-source-root)/#1(attachment-socket)`"),
        "stdout:\n{markdown}"
    );
    assert!(markdown.contains("`0.0100`"), "stdout:\n{markdown}");

    for (name, policy) in [
        (
            "expected-override.toml",
            concat!(
                "[checks.rest-world-scale]\n",
                "node_selectors = [\"attachment-socket\"]\n",
                "expected_uniform_scale = 0.01\n",
                "uniform_scale_tolerance = 0.000001\n",
            ),
        ),
        (
            "wide-tolerance.toml",
            concat!(
                "[checks.rest-world-scale]\n",
                "node_selectors = [\"attachment-socket\"]\n",
                "expected_uniform_scale = 1.0\n",
                "uniform_scale_tolerance = 1.0\n",
            ),
        ),
    ] {
        let policy_path = dir.path().join(name);
        std::fs::write(&policy_path, policy).expect("writes override config");
        let output = run_lint(&input, &policy_path, "json");
        assert!(output.status.success(), "override lint succeeds");
        let report: Value = serde_json::from_slice(&output.stdout).expect("valid override JSON");
        let check = report["files"][0]["checks"]
            .as_array()
            .expect("checks")
            .iter()
            .find(|check| check["check_id"] == "rest-world-scale")
            .expect("rest-world-scale record");
        assert_eq!(check["evaluation"], "complete");
        assert_eq!(check["findings"], json!([]), "{name} must affect lint");
    }

    let tiny_input = dir.path().join("tiny-compensated-attachment.gltf");
    write_compensated_attachment_gltf(&tiny_input, 0.01);
    let tiny_output = run_lint(&tiny_input, &config, "json");
    assert!(tiny_output.status.success(), "tiny-geometry lint succeeds");
    let tiny_report: Value = serde_json::from_slice(&tiny_output.stdout).expect("valid tiny JSON");
    assert_eq!(
        tiny_report["files"][0]["measurements"]["mesh_definitions"][0]["geometry_aabb"]["max"],
        json!([0.01, 0.01, 0.0])
    );
    let tiny_check = tiny_report["files"][0]["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|check| check["check_id"] == "rest-world-scale")
        .expect("rest-world-scale record");
    assert_eq!(
        tiny_check["findings"].as_array().expect("findings").len(),
        1
    );
    assert!(
        (tiny_check["findings"][0]["measured"]
            .as_f64()
            .expect("tiny-geometry selected-node factor")
            - 0.01)
            .abs()
            < 1.0e-8,
        "mesh magnitude must not influence the selected-node scale policy"
    );
}
