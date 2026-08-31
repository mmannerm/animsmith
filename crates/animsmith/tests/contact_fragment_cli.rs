use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn animsmith() -> Command {
    Command::new(env!("CARGO_BIN_EXE_animsmith"))
}

fn write_contact_fixture(directory: &Path) -> (PathBuf, PathBuf) {
    let input = directory.join("contact.gltf");
    let config = directory.join("contact.toml");
    let mut bytes = Vec::new();
    for value in [0.0f32, 0.5, 1.0] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    // Left Foot has only singleton support; Right Toe is its independent
    // fallback and has one maximal three-sample support run.
    for value in [0.0f32, 0.1, 0.0] {
        for component in [0.0, value, 0.0] {
            bytes.extend_from_slice(&component.to_le_bytes());
        }
    }
    for value in [0.0f32, 0.0, 0.0] {
        for component in [0.0, value, 0.0] {
            bytes.extend_from_slice(&component.to_le_bytes());
        }
    }
    fs::write(directory.join("contact.bin"), bytes).unwrap();
    fs::write(
        &input,
        serde_json::to_vec(&json!({
            "asset": {"version": "2.0"},
            "buffers": [{"uri": "contact.bin", "byteLength": 84}],
            "bufferViews": [
                {"buffer": 0, "byteOffset": 0, "byteLength": 12},
                {"buffer": 0, "byteOffset": 12, "byteLength": 36},
                {"buffer": 0, "byteOffset": 48, "byteLength": 36}
            ],
            "accessors": [
                {"bufferView": 0, "componentType": 5126, "count": 3, "type": "SCALAR"},
                {"bufferView": 1, "componentType": 5126, "count": 3, "type": "VEC3"},
                {"bufferView": 2, "componentType": 5126, "count": 3, "type": "VEC3"}
            ],
            "nodes": [
                {"name": "root", "children": [1, 2], "skin": 0},
                {"name": "humanoid_ L Foot"},
                {"name": "humanoid_ R Toe0"}
            ],
            "skins": [{"joints": [1, 2]}],
            "scenes": [{"nodes": [0]}], "scene": 0,
            "animations": [{
                "name": "walk", "samplers": [
                    {"input": 0, "output": 1, "interpolation": "LINEAR"},
                    {"input": 0, "output": 2, "interpolation": "LINEAR"}
                ],
                "channels": [
                    {"sampler": 0, "target": {"node": 1, "path": "translation"}},
                    {"sampler": 1, "target": {"node": 2, "path": "translation"}}
                ]
            }]
        }))
        .unwrap(),
    )
    .unwrap();
    fs::write(&config, "[rig]\nprofile = \"humanoid\"\n").unwrap();
    (input, config)
}

fn write_metric_grid_work_fixture(
    directory: &Path,
    stem: &str,
    frames: usize,
    bones: usize,
    tracks: usize,
) -> (PathBuf, PathBuf) {
    let input = directory.join(format!("{stem}.gltf"));
    let config = directory.join(format!("{stem}.toml"));
    let buffer_name = format!("{stem}.bin");
    let mut bytes = Vec::with_capacity(frames * 32);
    for frame in 0..frames {
        bytes.extend_from_slice(&(frame as f32 / (frames - 1) as f32).to_le_bytes());
    }
    let translation_offset = bytes.len();
    for _ in 0..frames {
        for component in [0.0f32; 3] {
            bytes.extend_from_slice(&component.to_le_bytes());
        }
    }
    let rotation_offset = bytes.len();
    for _ in 0..frames {
        for component in [0.0f32, 0.0, 0.0, 1.0] {
            bytes.extend_from_slice(&component.to_le_bytes());
        }
    }
    fs::write(directory.join(&buffer_name), &bytes).unwrap();
    let nodes = (0..bones)
        .map(|index| json!({"name": format!("N{index}")}))
        .collect::<Vec<_>>();
    let roots = (0..bones).collect::<Vec<_>>();
    let channels = (0..tracks)
        .map(|index| {
            let sampler = index % 2;
            let path = if sampler == 0 {
                "translation"
            } else {
                "rotation"
            };
            json!({"sampler": sampler, "target": {"node": index / 2, "path": path}})
        })
        .collect::<Vec<_>>();
    fs::write(
        &input,
        serde_json::to_vec(&json!({
            "asset": {"version": "2.0"},
            "buffers": [{"uri": buffer_name, "byteLength": bytes.len()}],
            "bufferViews": [
                {"buffer": 0, "byteOffset": 0, "byteLength": translation_offset},
                {"buffer": 0, "byteOffset": translation_offset, "byteLength": frames * 12},
                {"buffer": 0, "byteOffset": rotation_offset, "byteLength": frames * 16}
            ],
            "accessors": [
                {"bufferView": 0, "componentType": 5126, "count": frames, "type": "SCALAR", "min": [0.0], "max": [1.0]},
                {"bufferView": 1, "componentType": 5126, "count": frames, "type": "VEC3"},
                {"bufferView": 2, "componentType": 5126, "count": frames, "type": "VEC4"}
            ],
            "nodes": nodes,
            "scenes": [{"nodes": roots}], "scene": 0,
            "animations": [{
                "name": "walk",
                "samplers": [
                    {"input": 0, "output": 1, "interpolation": "LINEAR"},
                    {"input": 0, "output": 2, "interpolation": "LINEAR"}
                ],
                "channels": channels
            }]
        }))
        .unwrap(),
    )
    .unwrap();
    fs::write(
        &config,
        "[rig]\nprofile = \"auto\"\nroles = { root = \"N0\", left_foot = \"N1\", right_foot = \"N2\" }\n",
    )
    .unwrap();
    (input, config)
}

fn direct(input: &Path, config: &Path, output: &Path, clip: &str) -> Output {
    animsmith()
        .args([
            "--config",
            config.to_str().unwrap(),
            "generate",
            "contact-fragment",
            input.to_str().unwrap(),
            "--clip",
            clip,
            "--output",
            output.to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .unwrap()
}

fn write_collection_manifest(directory: &Path, input: &Path, config: &Path) -> PathBuf {
    let manifest = directory.join("collection.toml");
    fs::write(
        &manifest,
        format!(
            "schema = \"urn:animsmith:schema:collection-manifest:1\"\nschema_version = 1\ncollection_id = \"example.contacts\"\n[[sources]]\nkey = \"walk\"\npath = \"{}\"\nconfig = \"{}\"\n[[clips]]\nid = \"example.contacts/walk\"\nsource = \"walk\"\ntake_index = 0\ntake_name = \"walk\"\n",
            input.file_name().unwrap().to_str().unwrap(),
            config.file_name().unwrap().to_str().unwrap(),
        ),
    )
    .unwrap();
    manifest
}

fn collection(manifest: &Path, output: &Path, clip: &str, format: &str) -> Output {
    animsmith()
        .args([
            "collection",
            "generate-contact-fragment",
            manifest.to_str().unwrap(),
            "--clip",
            clip,
            "--output",
            output.to_str().unwrap(),
            "--format",
            format,
        ])
        .output()
        .unwrap()
}

#[test]
fn direct_contact_fragment_is_source_bound_and_uses_independent_toe_fallback() {
    let directory = tempfile::tempdir().unwrap();
    let (input, config) = write_contact_fixture(directory.path());
    let output = directory.path().join("fragment.json");
    let run = direct(&input, &config, &output, "walk");
    assert_eq!(
        run.status.code(),
        Some(0),
        "stderr: {}\nstdout: {}",
        String::from_utf8_lossy(&run.stderr),
        String::from_utf8_lossy(&run.stdout)
    );
    assert_eq!(
        run.stdout,
        fs::read(&output).unwrap(),
        "JSON stdout is the published bytes"
    );
    let fragment: Value = serde_json::from_slice(&run.stdout).unwrap();
    assert_eq!(
        fragment["clip"],
        json!({"scope": "document", "clip_name": "walk"})
    );
    assert_eq!(fragment["events"].as_array().unwrap().len(), 2);
    assert_eq!(fragment["events"][0]["event_id"], "marker/right_toe/0");
    assert_eq!(fragment["events"][1]["event_id"], "support/right_toe/0-2");
    assert_eq!(
        fragment["events"][1]["window"],
        json!({"start": 0, "end": 1})
    );
    assert_eq!(fragment["events"][0]["role"], "right_toe");
    assert_eq!(fragment["events"][1]["role"], "right_toe");
    assert_eq!(fragment["extensions"].as_array().unwrap().len(), 1);
    assert_eq!(
        fragment["extensions"][0],
        json!({
            "schema": "urn:animsmith:contact-support-detector:1",
            "schema_version": 1,
            "payload": {
                "algorithm": "stance-support-v1",
                "sampling": "metric-grid-longest-authored-channel",
                "max_frames": 1_000_000,
                "contact_height_m": 0.03,
                "roles": {"left": "left_foot", "right": "right_toe"},
            },
        })
    );
    let fragment_keys = fragment
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        fragment_keys,
        BTreeSet::from([
            "artifact",
            "clip",
            "dependency_closure_identity",
            "duration_s",
            "events",
            "extensions",
            "producer",
            "schema",
            "schema_version",
        ])
    );
    for event in fragment["events"].as_array().unwrap() {
        let keys = event
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let expected = if event.get("time").is_some() {
            BTreeSet::from(["event_id", "phase", "role", "time"])
        } else {
            BTreeSet::from(["event_id", "phase", "role", "window"])
        };
        assert_eq!(keys, expected, "{event}");
    }
    assert_eq!(
        fragment["artifact"]["bytes"],
        fs::metadata(&input).unwrap().len()
    );
}

#[test]
fn asymmetric_side_minima_emit_each_window_earliest_minimum_marker() {
    let directory = tempfile::tempdir().unwrap();
    let (input, config) = write_contact_fixture(directory.path());
    let bin = directory.path().join("contact.bin");
    let mut bytes = fs::read(&bin).unwrap();
    // Left Foot has a two-sample support run with its unique minimum at frame
    // 1. Right Toe remains a separate all-support run whose minimum is frame
    // 0, proving each side computes its own minimum.
    for (frame, y) in [0.02f32, 0.01, 0.1].into_iter().enumerate() {
        let offset = 12 + frame * 12 + 4;
        bytes[offset..offset + 4].copy_from_slice(&y.to_le_bytes());
    }
    fs::write(&bin, bytes).unwrap();
    let output = directory.path().join("fragment.json");
    let run = direct(&input, &config, &output, "walk");
    assert_eq!(run.status.code(), Some(0));
    let fragment: Value = serde_json::from_slice(&run.stdout).unwrap();
    let events = fragment["events"].as_array().unwrap();
    assert!(
        events
            .iter()
            .any(|event| { event["event_id"] == "marker/left_foot/1" && event["time"] == 0.5 })
    );
    assert!(
        events.iter().any(|event| {
            event["event_id"] == "support/left_foot/0-1"
                && event["window"] == json!({"start": 0, "end": 0.5})
        }),
        "{fragment}"
    );
    assert!(
        events
            .iter()
            .any(|event| { event["event_id"] == "marker/right_toe/0" && event["time"] == 0 })
    );
    assert!(events.iter().any(|event| event["role"] == "left_foot"));
    assert!(events.iter().any(|event| event["role"] == "right_toe"));
}

#[test]
fn collection_contact_fragment_reloads_its_manifest_witness() {
    let directory = tempfile::tempdir().unwrap();
    let (input, config) = write_contact_fixture(directory.path());
    let manifest = write_collection_manifest(directory.path(), &input, &config);
    let output = directory.path().join("collection-fragment.json");
    let run = collection(&manifest, &output, "example.contacts/walk", "json");
    assert_eq!(
        run.status.code(),
        Some(0),
        "stderr: {}\nstdout: {}",
        String::from_utf8_lossy(&run.stderr),
        String::from_utf8_lossy(&run.stdout)
    );
    let fragment: Value = serde_json::from_slice(&run.stdout).unwrap();
    assert_eq!(fragment["clip"]["scope"], "collection");
    assert_eq!(fragment["clip"]["logical_id"], "example.contacts/walk");
    assert_eq!(fragment["clip"]["take_index"], 0);
}

#[test]
fn collection_take_index_resolves_through_its_raw_source_witness() {
    let directory = tempfile::tempdir().unwrap();
    let (input, config) = write_contact_fixture(directory.path());
    let mut source: Value = serde_json::from_slice(&fs::read(&input).unwrap()).unwrap();
    // The second raw `walk` is normalized internally as `walk#1`; a direct
    // `Document::clips[raw_take_index]` name comparison would reject it.
    let duplicate = source["animations"][0].clone();
    source["animations"].as_array_mut().unwrap().push(duplicate);
    fs::write(&input, serde_json::to_vec(&source).unwrap()).unwrap();
    let manifest = write_collection_manifest(directory.path(), &input, &config);
    let manifest_text = String::from_utf8(fs::read(&manifest).unwrap())
        .unwrap()
        .replace("take_index = 0", "take_index = 1");
    fs::write(&manifest, manifest_text).unwrap();
    let output = directory.path().join("collection-fragment.json");
    let run = collection(&manifest, &output, "example.contacts/walk", "json");
    assert_eq!(run.status.code(), Some(0));
    let fragment: Value = serde_json::from_slice(&run.stdout).unwrap();
    assert_eq!(fragment["clip"]["take_index"], 1);
    assert_eq!(fragment["clip"]["take_name"], "walk");
}

#[test]
fn complete_clip_with_no_retained_support_runs_publishes_empty_events() {
    let directory = tempfile::tempdir().unwrap();
    let (input, config) = write_contact_fixture(directory.path());
    let bin = directory.path().join("contact.bin");
    let mut bytes = fs::read(&bin).unwrap();
    // Both selected roles have only isolated support samples at frames 0 and
    // 2, so complete evidence exists but the >=2-sample run filter retains
    // no events.
    for (frame, y) in [0.0f32, 0.1, 0.0].into_iter().enumerate() {
        let offset = 48 + frame * 12 + 4;
        bytes[offset..offset + 4].copy_from_slice(&y.to_le_bytes());
    }
    fs::write(&bin, bytes).unwrap();
    let output = directory.path().join("fragment.json");
    let run = direct(&input, &config, &output, "walk");
    assert_eq!(run.status.code(), Some(0));
    let fragment: Value = serde_json::from_slice(&run.stdout).unwrap();
    assert_eq!(fragment["events"], json!([]));
}

#[test]
fn duplicate_collection_logical_id_is_a_control_error_before_selection() {
    let directory = tempfile::tempdir().unwrap();
    let (input, config) = write_contact_fixture(directory.path());
    let manifest = write_collection_manifest(directory.path(), &input, &config);
    let duplicate = "\n[[clips]]\nid = \"example.contacts/walk\"\nsource = \"walk\"\ntake_index = 0\ntake_name = \"walk\"\n";
    let mut text = String::from_utf8(fs::read(&manifest).unwrap()).unwrap();
    text.push_str(duplicate);
    fs::write(&manifest, text).unwrap();
    let output = directory.path().join("fragment.json");
    fs::write(&output, b"sentinel").unwrap();
    let run = collection(&manifest, &output, "example.contacts/walk", "json");
    assert_eq!(run.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&run.stderr).contains("collection control error"));
    assert_eq!(fs::read(&output).unwrap(), b"sentinel");
}

#[test]
fn refusal_never_replaces_an_existing_output() {
    let directory = tempfile::tempdir().unwrap();
    let (input, config) = write_contact_fixture(directory.path());
    let output = directory.path().join("fragment.json");
    fs::write(&output, b"sentinel").unwrap();
    let run = direct(&input, &config, &output, "missing");
    assert_eq!(run.status.code(), Some(1));
    assert_eq!(fs::read(&output).unwrap(), b"sentinel");
    let refusal: Value = serde_json::from_slice(&run.stdout).unwrap();
    assert_eq!(refusal["rejection"]["kind"], "selection-mismatch");
}

#[test]
fn metric_grid_work_excess_refuses_direct_and_collection_without_replacing_output() {
    let directory = tempfile::tempdir().unwrap();
    let (input, config) =
        write_metric_grid_work_fixture(directory.path(), "pose-excess", 1_001, 1_000, 1);
    let manifest = write_collection_manifest(directory.path(), &input, &config);
    let output = directory.path().join("fragment.json");
    for collection_scoped in [false, true] {
        fs::write(&output, b"sentinel").unwrap();
        let run = if collection_scoped {
            collection(&manifest, &output, "example.contacts/walk", "json")
        } else {
            direct(&input, &config, &output, "walk")
        };
        assert_eq!(run.status.code(), Some(1));
        assert_eq!(fs::read(&output).unwrap(), b"sentinel");
        let refusal: Value = serde_json::from_slice(&run.stdout).unwrap();
        assert_eq!(refusal["rejection"]["kind"], "incomplete-evidence");
    }
}

#[test]
fn metric_grid_sampling_work_accepts_exact_and_refuses_n_plus_one() {
    let exact_directory = tempfile::tempdir().unwrap();
    let (exact_input, exact_config) =
        write_metric_grid_work_fixture(exact_directory.path(), "sampling-exact", 1_000, 500, 1_000);
    let exact_output = exact_directory.path().join("fragment.json");
    assert_eq!(
        direct(&exact_input, &exact_config, &exact_output, "walk")
            .status
            .code(),
        Some(0)
    );
    assert!(exact_output.exists());

    let excess_directory = tempfile::tempdir().unwrap();
    let (excess_input, excess_config) = write_metric_grid_work_fixture(
        excess_directory.path(),
        "sampling-excess",
        1_000,
        501,
        1_001,
    );
    let excess_output = excess_directory.path().join("fragment.json");
    fs::write(&excess_output, b"sentinel").unwrap();
    let run = direct(&excess_input, &excess_config, &excess_output, "walk");
    assert_eq!(run.status.code(), Some(1));
    assert_eq!(fs::read(&excess_output).unwrap(), b"sentinel");
    let refusal: Value = serde_json::from_slice(&run.stdout).unwrap();
    assert_eq!(refusal["rejection"]["kind"], "incomplete-evidence");
}

#[test]
fn output_never_replaces_consumed_primary_config_manifest_or_external_source() {
    let directory = tempfile::tempdir().unwrap();
    let (input, config) = write_contact_fixture(directory.path());
    let manifest = write_collection_manifest(directory.path(), &input, &config);
    let external = directory.path().join("contact.bin");
    for target in [&input, &config, &external] {
        let before = fs::read(target).unwrap();
        let run = direct(&input, &config, target, "walk");
        assert_eq!(run.status.code(), Some(2));
        assert_eq!(fs::read(target).unwrap(), before);
    }
    let before = fs::read(&manifest).unwrap();
    let run = collection(&manifest, &manifest, "example.contacts/walk", "json");
    assert_eq!(run.status.code(), Some(2));
    assert_eq!(fs::read(&manifest).unwrap(), before);
    let before = fs::read(&config).unwrap();
    let run = collection(&manifest, &config, "example.contacts/walk", "json");
    assert_eq!(run.status.code(), Some(2));
    assert_eq!(fs::read(&config).unwrap(), before);
    let before = fs::read(&input).unwrap();
    let run = collection(&manifest, &input, "example.contacts/walk", "json");
    assert_eq!(run.status.code(), Some(2));
    assert_eq!(fs::read(&input).unwrap(), before);
}

#[test]
fn text_presentations_preserve_exit_taxonomy_and_output_on_refusal() {
    let directory = tempfile::tempdir().unwrap();
    let (input, config) = write_contact_fixture(directory.path());
    let output = directory.path().join("fragment.json");
    let success = animsmith()
        .args([
            "--config",
            config.to_str().unwrap(),
            "generate",
            "contact-fragment",
            input.to_str().unwrap(),
            "--clip",
            "walk",
            "--output",
            output.to_str().unwrap(),
            "--format",
            "text",
        ])
        .output()
        .unwrap();
    assert_eq!(success.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&success.stdout).contains("published contact fragment"));

    fs::write(&output, b"sentinel").unwrap();
    let refusal = animsmith()
        .args([
            "--config",
            config.to_str().unwrap(),
            "generate",
            "contact-fragment",
            input.to_str().unwrap(),
            "--clip",
            "missing",
            "--output",
            output.to_str().unwrap(),
            "--format",
            "text",
        ])
        .output()
        .unwrap();
    assert_eq!(refusal.status.code(), Some(1));
    assert_eq!(fs::read(&output).unwrap(), b"sentinel");
    assert!(String::from_utf8_lossy(&refusal.stderr).contains("selection-mismatch"));

    let manifest = write_collection_manifest(directory.path(), &input, &config);
    let collection_success = collection(
        &manifest,
        &directory.path().join("collection-fragment.json"),
        "example.contacts/walk",
        "text",
    );
    assert_eq!(collection_success.status.code(), Some(0));
    assert!(
        String::from_utf8_lossy(&collection_success.stdout).contains("published contact fragment")
    );
    let collection_refusal = collection(&manifest, &output, "missing", "text");
    assert_eq!(collection_refusal.status.code(), Some(1));
    assert_eq!(fs::read(&output).unwrap(), b"sentinel");
}

#[test]
fn incomplete_source_evidence_refuses_without_replacing_existing_output() {
    let directory = tempfile::tempdir().unwrap();
    let (input, config) = write_contact_fixture(directory.path());
    let output = directory.path().join("fragment.json");
    fs::write(&output, b"sentinel").unwrap();
    fs::remove_file(directory.path().join("contact.bin")).unwrap();
    let run = direct(&input, &config, &output, "walk");
    assert_eq!(run.status.code(), Some(1));
    assert_eq!(fs::read(&output).unwrap(), b"sentinel");
    let refusal: Value = serde_json::from_slice(&run.stdout).unwrap();
    assert_eq!(refusal["rejection"]["kind"], "incomplete-evidence");
}

#[test]
fn strict_preflight_refuses_bad_duration_or_missing_bilateral_role() {
    let directory = tempfile::tempdir().unwrap();
    let (input, config) = write_contact_fixture(directory.path());
    let output = directory.path().join("fragment.json");
    fs::write(&output, b"sentinel").unwrap();
    let mut bytes = fs::read(directory.path().join("contact.bin")).unwrap();
    bytes[..12].fill(0);
    fs::write(directory.path().join("contact.bin"), bytes).unwrap();
    let duration = direct(&input, &config, &output, "walk");
    assert_eq!(duration.status.code(), Some(1));
    assert_eq!(fs::read(&output).unwrap(), b"sentinel");

    let directory = tempfile::tempdir().unwrap();
    let (input, config) = write_contact_fixture(directory.path());
    let output = directory.path().join("fragment.json");
    fs::write(&output, b"sentinel").unwrap();
    let source = String::from_utf8(fs::read(&input).unwrap())
        .unwrap()
        .replace("humanoid_ R Toe0", "unresolved_right");
    fs::write(&input, source).unwrap();
    let missing_side = direct(&input, &config, &output, "walk");
    assert_eq!(missing_side.status.code(), Some(1));
    assert_eq!(fs::read(&output).unwrap(), b"sentinel");
}

#[test]
fn duplicate_direct_clip_names_refuse_without_replacing_output() {
    let directory = tempfile::tempdir().unwrap();
    let (input, config) = write_contact_fixture(directory.path());
    let mut source: Value = serde_json::from_slice(&fs::read(&input).unwrap()).unwrap();
    let duplicate = source["animations"][0].clone();
    source["animations"].as_array_mut().unwrap().push(duplicate);
    fs::write(&input, serde_json::to_vec(&source).unwrap()).unwrap();
    let output = directory.path().join("fragment.json");
    fs::write(&output, b"sentinel").unwrap();
    let run = direct(&input, &config, &output, "walk");
    assert_eq!(run.status.code(), Some(1));
    assert_eq!(fs::read(&output).unwrap(), b"sentinel");
    let refusal: Value = serde_json::from_slice(&run.stdout).unwrap();
    assert_eq!(refusal["rejection"]["kind"], "selection-mismatch");
}

#[test]
fn direct_selection_samples_the_exact_raw_name_witness_not_a_synthetic_collision() {
    let directory = tempfile::tempdir().unwrap();
    let (input, config) = write_contact_fixture(directory.path());
    let bin = directory.path().join("contact.bin");
    let mut bytes = fs::read(&bin).unwrap();
    // Third authored take: duration 2s, Left Foot support only, unlike the
    // preceding duplicated `walk` takes (duration 1s, Right Toe support).
    for value in [0.0f32, 1.0, 2.0] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    for values in [[0.0f32, 0.0, 0.0], [0.0f32, 0.1, 0.0]] {
        for value in values {
            for component in [0.0, value, 0.0] {
                bytes.extend_from_slice(&component.to_le_bytes());
            }
        }
    }
    fs::write(&bin, bytes).unwrap();

    let mut source: Value = serde_json::from_slice(&fs::read(&input).unwrap()).unwrap();
    source["buffers"][0]["byteLength"] = json!(168);
    let duplicate = source["animations"][0].clone();
    let mut authored_walk_hash_one = duplicate.clone();
    authored_walk_hash_one["name"] = json!("walk#1");
    for (sampler, input_accessor, output_accessor) in [(0, 3, 4), (1, 3, 5)] {
        authored_walk_hash_one["samplers"][sampler]["input"] = json!(input_accessor);
        authored_walk_hash_one["samplers"][sampler]["output"] = json!(output_accessor);
    }
    source["animations"].as_array_mut().unwrap().push(duplicate);
    source["animations"]
        .as_array_mut()
        .unwrap()
        .push(authored_walk_hash_one);
    for (offset, length) in [(84, 12), (96, 36), (132, 36)] {
        source["bufferViews"].as_array_mut().unwrap().push(json!({
            "buffer": 0,
            "byteOffset": offset,
            "byteLength": length,
        }));
    }
    for view in [3, 4, 5] {
        let kind = if view == 3 { "SCALAR" } else { "VEC3" };
        source["accessors"].as_array_mut().unwrap().push(json!({
            "bufferView": view,
            "componentType": 5126,
            "count": 3,
            "type": kind,
        }));
    }
    fs::write(&input, serde_json::to_vec(&source).unwrap()).unwrap();

    let output = directory.path().join("fragment.json");
    let run = direct(&input, &config, &output, "walk#1");
    assert_eq!(run.status.code(), Some(0));
    let fragment: Value = serde_json::from_slice(&run.stdout).unwrap();
    assert_eq!(
        fragment["clip"],
        json!({"scope": "document", "clip_name": "walk#1"})
    );
    assert_eq!(fragment["duration_s"], 2);
    let ids = fragment["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|event| event["event_id"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(ids, ["marker/left_foot/0", "support/left_foot/0-2"]);
}

#[test]
fn non_finite_stance_samples_refuse_without_replacing_output() {
    for non_finite in [f32::NAN, f32::INFINITY] {
        let directory = tempfile::tempdir().unwrap();
        let (input, config) = write_contact_fixture(directory.path());
        let bin = directory.path().join("contact.bin");
        let mut bytes = fs::read(&bin).unwrap();
        // First Left Foot translation's Y component.
        bytes[16..20].copy_from_slice(&non_finite.to_le_bytes());
        fs::write(&bin, bytes).unwrap();
        let output = directory.path().join("fragment.json");
        fs::write(&output, b"sentinel").unwrap();
        let run = direct(&input, &config, &output, "walk");
        assert_eq!(run.status.code(), Some(1));
        assert_eq!(fs::read(&output).unwrap(), b"sentinel");
        let refusal: Value = serde_json::from_slice(&run.stdout).unwrap();
        assert_eq!(refusal["rejection"]["kind"], "incomplete-evidence");
    }
}

#[test]
fn collection_take_name_mismatch_refuses_without_replacing_output() {
    let directory = tempfile::tempdir().unwrap();
    let (input, config) = write_contact_fixture(directory.path());
    let manifest = write_collection_manifest(directory.path(), &input, &config);
    let rewritten = String::from_utf8(fs::read(&manifest).unwrap())
        .unwrap()
        .replace("take_name = \"walk\"", "take_name = \"other\"");
    fs::write(&manifest, rewritten).unwrap();
    let output = directory.path().join("fragment.json");
    fs::write(&output, b"sentinel").unwrap();
    let run = collection(&manifest, &output, "example.contacts/walk", "json");
    assert_eq!(run.status.code(), Some(1));
    assert_eq!(fs::read(&output).unwrap(), b"sentinel");
    let refusal: Value = serde_json::from_slice(&run.stdout).unwrap();
    assert_eq!(refusal["rejection"]["kind"], "selection-mismatch");
}
