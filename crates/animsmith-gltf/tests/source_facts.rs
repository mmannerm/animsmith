//! Analytic coverage for the immutable glTF raw-source-facts projection.

use animsmith_core::scale::{ScaleError, ScaleOperation};
use animsmith_core::{
    DependencyClosureCoverageReasonV1, DependencyClosureCoverageV1, DependencyReferenceTargetV1,
    DependencyResourceRefusalReasonV1, DependencyResourceUnavailableReasonV1, InputIdentity,
    RAW_SOURCE_V1_MAX_TEXT_BYTES, SourceAxisV1, SourceChannelPropertyV1, SourceFormatV1,
    SourceHandednessV1, SourceInterpolationV1, SourceLoaderDispositionV1, SourceObservationStateV1,
    SourceProvenanceKindV1, SourceResourceKindV1, SourceResourceLocatorV1,
    SourceSetCoverageStateV1, SourceUnavailableReasonV1,
};
use base64::Engine as _;
use serde_json::{Value, json};
use std::path::Path;

fn json_bytes(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).expect("serialize analytic glTF")
}

fn data_uri(bytes: &[u8]) -> String {
    format!(
        "data:application/octet-stream;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

fn push_f32s(blob: &mut Vec<u8>, values: &[f32]) -> (usize, usize) {
    while !blob.len().is_multiple_of(4) {
        blob.push(0);
    }
    let offset = blob.len();
    blob.extend(values.iter().flat_map(|value| value.to_le_bytes()));
    (offset, blob.len() - offset)
}

fn channel_fixture() -> Vec<u8> {
    let mut blob = Vec::new();
    let input = push_f32s(&mut blob, &[1.0, 3.0]);
    let translation = push_f32s(&mut blob, &[0.0; 6]);
    let rotation = push_f32s(&mut blob, &[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]);
    let scale_cubic = push_f32s(&mut blob, &[1.0; 18]);
    let weights = push_f32s(&mut blob, &[0.25, 0.75]);
    let views = [input, translation, rotation, scale_cubic, weights]
        .into_iter()
        .map(|(offset, length)| json!({ "buffer": 0, "byteOffset": offset, "byteLength": length }))
        .collect::<Vec<_>>();
    json_bytes(&json!({
        "asset": { "version": "2.0" },
        "buffers": [{ "uri": data_uri(&blob), "byteLength": blob.len() }],
        "bufferViews": views,
        "accessors": [
            { "bufferView": 0, "componentType": 5126, "count": 2, "type": "SCALAR", "min": [1.0], "max": [3.0] },
            { "bufferView": 1, "componentType": 5126, "count": 2, "type": "VEC3" },
            { "bufferView": 2, "componentType": 5126, "count": 2, "type": "VEC4" },
            { "bufferView": 3, "componentType": 5126, "count": 6, "type": "VEC3" },
            { "bufferView": 4, "componentType": 5126, "count": 2, "type": "SCALAR" }
        ],
        "nodes": [{ "name": "root" }],
        "animations": [{
            "name": "raw",
            "samplers": [
                { "input": 0, "output": 1 },
                { "input": 0, "output": 2, "interpolation": "STEP" },
                { "input": 0, "output": 3, "interpolation": "CUBICSPLINE" },
                { "input": 0, "output": 4, "interpolation": "LINEAR" }
            ],
            "channels": [
                { "sampler": 0, "target": { "node": 0, "path": "translation" } },
                { "sampler": 1, "target": { "node": 0, "path": "rotation" } },
                { "sampler": 2, "target": { "node": 0, "path": "scale" } },
                { "sampler": 3, "target": { "node": 0, "path": "weights" } }
            ]
        }]
    }))
}

fn glb(mut value: Value, bin: &[u8]) -> Vec<u8> {
    if value.get("buffers").is_none() {
        value["buffers"] = json!([{ "byteLength": bin.len() }]);
    }
    let mut json = json_bytes(&value);
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    let mut binary = bin.to_vec();
    while !binary.len().is_multiple_of(4) {
        binary.push(0);
    }
    let total = 12 + 8 + json.len() + 8 + binary.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend_from_slice(&0x4e4f_534au32.to_le_bytes());
    out.extend_from_slice(&json);
    out.extend_from_slice(&(binary.len() as u32).to_le_bytes());
    out.extend_from_slice(&0x004e_4942u32.to_le_bytes());
    out.extend_from_slice(&binary);
    out
}

fn assert_same_facts(left: &animsmith_core::LoadedSource, right: &animsmith_core::LoadedSource) {
    let left = left.source_facts();
    let right = right.source_facts();
    assert_eq!(left.format(), right.format());
    assert_eq!(left.primary_identity(), right.primary_identity());
    assert_eq!(left.linear_unit(), right.linear_unit());
    assert_eq!(left.coordinate_basis(), right.coordinate_basis());
    assert_eq!(left.frames_per_second(), right.frames_per_second());
    assert_eq!(left.clips(), right.clips());
    assert_eq!(left.constructs(), right.constructs());
    assert_eq!(left.resources(), right.resources());
    assert_eq!(
        left.source_skeleton().nodes.len(),
        right.source_skeleton().nodes.len()
    );
    assert_eq!(
        left.source_skeleton().skins.len(),
        right.source_skeleton().skins.len()
    );
    assert_eq!(left.work(), right.work());
}

#[test]
fn path_and_bytes_bind_exact_identity_and_container_kind() {
    let bytes = json_bytes(&json!({ "asset": { "version": "2.0" } }));
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("misleading.glb");
    std::fs::write(&path, &bytes).expect("write analytic source");
    let from_path = animsmith_gltf::load_source(&path).expect("path load");
    let from_bytes = animsmith_gltf::load_source_bytes(&path, &bytes).expect("byte load");

    assert_eq!(from_path.source_facts().format(), SourceFormatV1::GltfJson);
    assert_eq!(
        from_path.source_facts().primary_identity(),
        &InputIdentity::from_bytes(&bytes)
    );
    assert_eq!(
        from_path.source_facts().primary_identity(),
        from_bytes.source_facts().primary_identity()
    );
    assert_same_facts(&from_path, &from_bytes);
    let scale = animsmith_gltf::preflight_scale_source_bytes(&path, &bytes)
        .expect("minimal source passes scale preflight");
    assert_eq!(
        scale.source_facts().primary_identity(),
        &InputIdentity::from_bytes(&bytes)
    );

    let glb = glb(json!({ "asset": { "version": "2.0" } }), &[0, 0, 0, 0]);
    let glb_path = dir.path().join("misleading.gltf");
    std::fs::write(&glb_path, &glb).expect("write analytic GLB");
    let loaded = animsmith_gltf::load_source_bytes(&glb_path, &glb).expect("GLB bytes load");
    let loaded_from_path = animsmith_gltf::load_source(&glb_path).expect("GLB path load");
    assert_eq!(loaded.source_facts().format(), SourceFormatV1::Glb);
    assert_eq!(
        loaded.source_facts().primary_identity(),
        &InputIdentity::from_bytes(&glb)
    );
    assert_same_facts(&loaded, &loaded_from_path);
}

#[test]
fn gltf_units_basis_and_time_basis_are_format_defined() {
    let bytes = json_bytes(&json!({ "asset": { "version": "2.0" } }));
    let loaded = animsmith_gltf::load_source_bytes(Path::new("facts.gltf"), &bytes)
        .expect("minimal glTF loads");
    let facts = loaded.source_facts();
    let SourceObservationStateV1::Observed(unit) = facts.linear_unit().state() else {
        panic!("glTF unit must be observed")
    };
    assert_eq!(unit.meters_per_source_unit(), 1.0);
    let SourceObservationStateV1::Observed(basis) = facts.coordinate_basis().state() else {
        panic!("glTF basis must be observed")
    };
    assert_eq!(basis.right(), SourceAxisV1::PositiveX);
    assert_eq!(basis.up(), SourceAxisV1::PositiveY);
    assert_eq!(basis.forward(), SourceAxisV1::PositiveZ);
    assert_eq!(basis.handedness(), SourceHandednessV1::Right);
    assert!(matches!(
        facts.frames_per_second().state(),
        SourceObservationStateV1::ProvenAbsent
    ));
    assert!(facts.clips().proves_absence());
    assert!(facts.constructs().proves_absence());
    assert!(facts.resources().proves_absence());
}

#[test]
fn clips_keep_names_ranges_and_every_raw_channel_identity() {
    let bytes = channel_fixture();
    let loaded = animsmith_gltf::load_source_bytes(Path::new("channels.gltf"), &bytes)
        .expect("analytic channels load");
    let clips = loaded.source_facts().clips();
    assert_eq!(clips.coverage().state(), SourceSetCoverageStateV1::Complete);
    assert_eq!(clips.rows().len(), 1);
    let clip = &clips.rows()[0];
    assert_eq!(clip.source_clip_index(), 0);
    assert!(matches!(
        clip.source_name().state(),
        SourceObservationStateV1::Observed(name) if name.as_str() == "raw"
    ));
    assert!(matches!(
        clip.normalized_clip_index().state(),
        SourceObservationStateV1::Observed(0)
    ));
    assert!(matches!(
        clip.source_range().state(),
        SourceObservationStateV1::ProvenAbsent
    ));
    let SourceObservationStateV1::Observed(range) = clip.sampler_range().state() else {
        panic!("authored sampler extent must be observed")
    };
    assert_eq!((range.begin_s(), range.end_s()), (1.0, 3.0));

    let rows = clip.channels().rows();
    assert_eq!(rows.len(), 4);
    assert_eq!(
        rows.iter().map(|row| row.property()).collect::<Vec<_>>(),
        [
            SourceChannelPropertyV1::Translation,
            SourceChannelPropertyV1::Rotation,
            SourceChannelPropertyV1::Scale,
            SourceChannelPropertyV1::Weights,
        ]
    );
    assert_eq!(
        rows.iter().map(|row| row.disposition()).collect::<Vec<_>>(),
        [
            SourceLoaderDispositionV1::Preserved,
            SourceLoaderDispositionV1::Preserved,
            SourceLoaderDispositionV1::Preserved,
            SourceLoaderDispositionV1::Discarded,
        ]
    );
    assert_eq!(
        rows.iter()
            .map(|row| (row.input_accessor_index(), row.output_accessor_index()))
            .collect::<Vec<_>>(),
        [
            (Some(0), Some(1)),
            (Some(0), Some(2)),
            (Some(0), Some(3)),
            (Some(0), Some(4))
        ]
    );
    assert_eq!(
        rows.iter()
            .map(|row| match row.interpolation().state() {
                SourceObservationStateV1::Observed(value) => *value,
                state => panic!("missing interpolation: {state:?}"),
            })
            .collect::<Vec<_>>(),
        [
            SourceInterpolationV1::Linear,
            SourceInterpolationV1::Step,
            SourceInterpolationV1::CubicSpline,
            SourceInterpolationV1::Linear,
        ]
    );
    assert_eq!(rows[3].target().index(), 0);
    assert!(!rows[3].components().x());
    assert_eq!(
        rows[0]
            .interpolation()
            .provenance()
            .expect("observed interpolation provenance")
            .kind(),
        SourceProvenanceKindV1::ParserProjected
    );
    assert_eq!(loaded.document().clips[0].tracks.len(), 3);
}

#[test]
fn unnamed_and_empty_animation_proves_no_sampler_range() {
    let bytes = json_bytes(&json!({
        "asset": { "version": "2.0" },
        "animations": [{ "channels": [], "samplers": [] }]
    }));
    let loaded = animsmith_gltf::load_source_bytes(Path::new("empty.gltf"), &bytes)
        .expect("empty animation loads");
    let clip = &loaded.source_facts().clips().rows()[0];
    assert!(matches!(
        clip.source_name().state(),
        SourceObservationStateV1::ProvenAbsent
    ));
    assert!(matches!(
        clip.sampler_range().state(),
        SourceObservationStateV1::ProvenAbsent
    ));
    assert!(clip.channels().proves_absence());
    assert_eq!(loaded.document().clips[0].name, "animation0");
}

#[test]
fn oversized_optional_name_does_not_turn_legacy_success_into_failure() {
    let bytes = json_bytes(&json!({
        "asset": { "version": "2.0" },
        "animations": [{
            "name": "x".repeat(RAW_SOURCE_V1_MAX_TEXT_BYTES + 1),
            "channels": [],
            "samplers": []
        }]
    }));
    let legacy = animsmith_gltf::load_bytes(Path::new("oversized-name.gltf"), &bytes)
        .expect("document-only load remains successful");
    assert_eq!(legacy.clips.len(), 1);

    let loaded = animsmith_gltf::load_source_bytes(Path::new("oversized-name.gltf"), &bytes)
        .expect("source-facts load remains successful");
    assert!(matches!(
        loaded.source_facts().clips().rows()[0]
            .source_name()
            .state(),
        SourceObservationStateV1::Unavailable(SourceUnavailableReasonV1::ProjectionBudgetExceeded)
    ));
}

#[test]
fn used_extensions_keep_declaration_order_and_loader_unsupported() {
    let bytes = json_bytes(&json!({
        "asset": { "version": "2.0" },
        "extensionsUsed": ["Z_vendor", "A_vendor"],
        "extensions": {
            "Z_vendor": { "uri": "unmodeled-vendor-sidecar.bin" }
        }
    }));
    let loaded = animsmith_gltf::load_source_bytes(Path::new("extensions.gltf"), &bytes)
        .expect("extension declarations load");
    let rows = loaded.source_facts().constructs().rows();
    assert_eq!(
        rows.iter()
            .map(|row| row.name().as_str())
            .collect::<Vec<_>>(),
        ["Z_vendor", "A_vendor"]
    );
    assert!(!rows[0].required());
    assert!(!rows[1].required());
    assert_eq!(
        rows.iter().map(|row| row.count()).collect::<Vec<_>>(),
        [1, 1]
    );
    assert_eq!(
        rows.iter()
            .map(|row| row.source_order_index())
            .collect::<Vec<_>>(),
        [0, 1]
    );
    assert!(
        rows.iter()
            .all(|row| row.disposition() == SourceLoaderDispositionV1::Unsupported)
    );
    let closure = loaded.dependency_closure();
    assert!(!closure.coverage().is_complete());
    assert!(closure.identity().is_none());
    assert_eq!(closure.work().external_open_attempts(), 0);
    assert!(
        closure
            .coverage()
            .reasons()
            .contains(&DependencyClosureCoverageReasonV1::UnmodeledResourceDomain)
    );
    assert!(!format!("{closure:?}").contains("unmodeled-vendor-sidecar.bin"));
}

#[test]
fn required_unsupported_extensions_preserve_strict_legacy_rejection() {
    let bytes = json_bytes(&json!({
        "asset": { "version": "2.0" },
        "extensionsUsed": ["Z_vendor"],
        "extensionsRequired": ["Z_vendor"]
    }));
    let path = Path::new("required-extension.gltf");
    assert!(animsmith_gltf::load_bytes(path, &bytes).is_err());
    assert!(animsmith_gltf::load_source_bytes(path, &bytes).is_err());
}

#[test]
fn rejected_resource_locators_are_redacted_from_load_errors() {
    for uri in [
        "../TOP_SECRET_LOCATOR.bin",
        "data:TOP_SECRET_LOCATOR,not-base64",
    ] {
        let bytes = json_bytes(&json!({
            "asset": { "version": "2.0" },
            "buffers": [{ "uri": uri, "byteLength": 4 }]
        }));
        let error = animsmith_gltf::load_source_bytes(Path::new("redaction.gltf"), &bytes)
            .expect_err("unsafe resource must be rejected")
            .to_string();
        assert!(!error.contains("TOP_SECRET_LOCATOR"), "{error}");
    }
}

#[test]
fn control_bearing_resource_locator_is_redacted_from_facts_and_debug() {
    let bytes = json_bytes(&json!({
        "asset": { "version": "2.0" },
        "images": [{ "uri": "textures/TOP_SECRET\nname.png" }]
    }));
    let loaded = animsmith_gltf::load_source_bytes(Path::new("control-uri.gltf"), &bytes)
        .expect("unreferenced image declaration loads");
    assert!(matches!(
        loaded.source_facts().resources().rows()[0].locator(),
        SourceResourceLocatorV1::Malformed
    ));
    let debug = format!("{:?}", loaded.source_facts());
    assert!(!debug.contains("TOP_SECRET"), "{debug}");
}

#[test]
fn resources_cover_bin_data_relative_and_redacted_unsafe_locators() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join("external.bin"), [0u8; 4]).expect("external buffer");
    let bytes = json_bytes(&json!({
        "asset": { "version": "2.0" },
        "buffers": [
            { "uri": data_uri(&[0u8; 4]), "byteLength": 4 },
            { "uri": "external.bin", "byteLength": 4 }
        ],
        "bufferViews": [{ "buffer": 0, "byteOffset": 0, "byteLength": 4 }],
        "images": [
            { "bufferView": 0, "mimeType": "image/png" },
            { "uri": "data:image/png;base64," },
            { "uri": "textures/albedo.png" },
            { "uri": "../secret.png" },
            { "uri": "/absolute.png" },
            { "uri": "https://example.invalid/image.png" }
        ]
    }));
    let path = dir.path().join("resources.gltf");
    let loaded = animsmith_gltf::load_source_bytes_with_resource_root(&path, &bytes, dir.path())
        .expect("resources load");
    let rows = loaded.source_facts().resources().rows();
    assert_eq!(rows.len(), 8);
    assert_eq!(
        rows.iter()
            .map(|row| row.source_order_index())
            .collect::<Vec<_>>(),
        (0..8).collect::<Vec<_>>()
    );
    assert_eq!(rows[0].kind(), SourceResourceKindV1::Buffer);
    assert!(matches!(
        rows[0].locator(),
        SourceResourceLocatorV1::DataUri
    ));
    assert!(matches!(
        rows[1].locator(),
        SourceResourceLocatorV1::Relative(path) if path.as_str() == "external.bin"
    ));
    assert_eq!(rows[2].kind(), SourceResourceKindV1::Image);
    assert!(matches!(
        rows[2].locator(),
        SourceResourceLocatorV1::Embedded
    ));
    assert!(matches!(
        rows[3].locator(),
        SourceResourceLocatorV1::DataUri
    ));
    assert!(matches!(
        rows[4].locator(),
        SourceResourceLocatorV1::Relative(path) if path.as_str() == "textures/albedo.png"
    ));
    assert!(matches!(
        rows[5].locator(),
        SourceResourceLocatorV1::Escaping
    ));
    assert!(matches!(
        rows[6].locator(),
        SourceResourceLocatorV1::Absolute
    ));
    assert!(matches!(rows[7].locator(), SourceResourceLocatorV1::Remote));

    let glb = glb(json!({ "asset": { "version": "2.0" } }), &[0u8; 4]);
    let loaded =
        animsmith_gltf::load_source_bytes(Path::new("embedded.glb"), &glb).expect("BIN GLB loads");
    assert!(matches!(
        loaded.source_facts().resources().rows()[0].locator(),
        SourceResourceLocatorV1::Embedded
    ));
}

#[test]
fn dependency_closure_captures_one_external_key_and_its_aliases_once() {
    let dir = tempfile::tempdir().expect("temp dir");
    let shared = [1_u8, 2, 3, 4];
    std::fs::write(dir.path().join("shared.bin"), shared).expect("external resource");
    let bytes = json_bytes(&json!({
        "asset": { "version": "2.0" },
        "buffers": [{ "uri": "shared.bin", "byteLength": shared.len() }],
        "images": [{ "uri": "shared.bin" }]
    }));
    let loaded = animsmith_gltf::load_source_bytes_with_resource_root(
        &dir.path().join("captured.gltf"),
        &bytes,
        dir.path(),
    )
    .expect("rooted byte load");

    let closure = loaded.dependency_closure();
    assert!(matches!(
        closure.coverage(),
        DependencyClosureCoverageV1::Complete
    ));
    assert!(closure.identity().is_some());
    assert_eq!(closure.external_resources().len(), 1);
    assert_eq!(
        closure.external_resources()[0].identity().bytes(),
        shared.len() as u64
    );
    assert_eq!(closure.work().external_open_attempts(), 1);
    assert_eq!(closure.work().captured_external_resources(), 1);
    assert_eq!(
        closure.work().external_bytes_read_hashed(),
        shared.len() as u64
    );
    assert!(matches!(
        closure.references()[0].target(),
        DependencyReferenceTargetV1::External { key } if key.as_str() == "shared.bin"
    ));
    assert!(matches!(
        closure.references()[1].target(),
        DependencyReferenceTargetV1::External { key } if key.as_str() == "shared.bin"
    ));
    assert!(!format!("{closure:?}").contains(&dir.path().display().to_string()));
}

#[test]
fn dependency_closure_refuses_key_syntax_before_any_open() {
    let dir = tempfile::tempdir().expect("temp dir");
    let bytes = json_bytes(&json!({
        "asset": { "version": "2.0" },
        "images": [{ "uri": "image.png?untrusted-query" }]
    }));
    let loaded = animsmith_gltf::load_source_bytes_with_resource_root(
        &dir.path().join("captured.gltf"),
        &bytes,
        dir.path(),
    )
    .expect("optional image remains a source row");

    let closure = loaded.dependency_closure();
    assert_eq!(closure.work().external_open_attempts(), 0);
    assert!(matches!(
        closure.references()[0].target(),
        DependencyReferenceTargetV1::Refused {
            reason: DependencyResourceRefusalReasonV1::Malformed,
            ..
        }
    ));
}

#[test]
fn dependency_closure_rejects_non_regular_external_targets_before_any_open() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::create_dir(dir.path().join("directory-target")).expect("directory target");
    let bytes = json_bytes(&json!({
        "asset": { "version": "2.0" },
        "images": [{ "uri": "directory-target" }]
    }));
    let loaded = animsmith_gltf::load_source_bytes_with_resource_root(
        &dir.path().join("captured.gltf"),
        &bytes,
        dir.path(),
    )
    .expect("optional directory target remains observable");

    let closure = loaded.dependency_closure();
    assert_eq!(closure.work().external_open_attempts(), 0);
    assert!(matches!(
        closure.references()[0].target(),
        DependencyReferenceTargetV1::Unavailable {
            reason: DependencyResourceUnavailableReasonV1::Unreadable,
            ..
        }
    ));
}

#[cfg(unix)]
#[test]
fn dependency_closure_refuses_resource_and_root_symlinks_before_any_open() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join("target.png"), b"not an image").expect("target");
    symlink(dir.path().join("target.png"), dir.path().join("linked.png")).expect("symlink");
    let bytes = json_bytes(&json!({
        "asset": { "version": "2.0" },
        "images": [{ "uri": "linked.png" }]
    }));

    let loaded = animsmith_gltf::load_source_bytes_with_resource_root(
        &dir.path().join("captured.gltf"),
        &bytes,
        dir.path(),
    )
    .expect("optional symlinked image remains observable");
    let closure = loaded.dependency_closure();
    assert_eq!(closure.work().external_open_attempts(), 0);
    assert!(matches!(
        closure.references()[0].target(),
        DependencyReferenceTargetV1::Refused {
            reason: DependencyResourceRefusalReasonV1::Symlink,
            ..
        }
    ));

    let root_link = dir.path().with_extension("root-link");
    symlink(dir.path(), &root_link).expect("root symlink");
    let loaded = animsmith_gltf::load_source_bytes_with_resource_root(
        &dir.path().join("captured.gltf"),
        &bytes,
        &root_link,
    )
    .expect("optional image remains observable");
    let closure = loaded.dependency_closure();
    assert_eq!(closure.work().external_open_attempts(), 0);
    assert!(matches!(
        closure.references()[0].target(),
        DependencyReferenceTargetV1::Refused {
            reason: DependencyResourceRefusalReasonV1::Symlink,
            ..
        }
    ));
}

#[cfg(unix)]
#[test]
fn explicit_root_capability_may_traverse_a_symlinked_ancestor() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().expect("temp dir");
    let real_parent = dir.path().join("real-parent");
    let real_root = real_parent.join("resources");
    std::fs::create_dir_all(&real_root).expect("real resource root");
    std::fs::write(real_root.join("image.bin"), b"captured").expect("external image");
    let linked_parent = dir.path().join("linked-parent");
    symlink(&real_parent, &linked_parent).expect("ancestor symlink");
    let explicit_root = linked_parent.join("resources");
    let bytes = json_bytes(&json!({
        "asset": { "version": "2.0" },
        "images": [{ "uri": "image.bin" }]
    }));

    let loaded = animsmith_gltf::load_source_bytes_with_resource_root(
        &dir.path().join("captured.gltf"),
        &bytes,
        &explicit_root,
    )
    .expect("the caller explicitly authorized the root path");
    let closure = loaded.dependency_closure();
    assert!(closure.coverage().is_complete());
    assert_eq!(closure.work().external_open_attempts(), 1);
    assert!(matches!(
        closure.references()[0].target(),
        DependencyReferenceTargetV1::External { key } if key.as_str() == "image.bin"
    ));
}

#[test]
fn path_loading_preserves_a_caller_supplied_lexical_parent_in_the_root() {
    let dir = tempfile::tempdir().expect("temp dir");
    let capability = dir.path().join("capability");
    let unused = capability.join("unused");
    let resources = capability.join("resources");
    std::fs::create_dir_all(&unused).expect("lexical parent component exists");
    std::fs::create_dir_all(&resources).expect("resource root");
    std::fs::write(resources.join("image.bin"), b"captured").expect("external image");
    let bytes = json_bytes(&json!({
        "asset": { "version": "2.0" },
        "images": [{ "uri": "image.bin" }]
    }));
    let source = resources.join("source.gltf");
    std::fs::write(&source, bytes).expect("primary source");
    let supplied_path = unused.join("..").join("resources").join("source.gltf");

    let loaded = animsmith_gltf::load_source(&supplied_path)
        .expect("the input parent is the caller-authorized resource root");
    let closure = loaded.dependency_closure();
    assert!(closure.coverage().is_complete());
    assert_eq!(closure.work().external_open_attempts(), 1);
    assert!(matches!(
        closure.references()[0].target(),
        DependencyReferenceTargetV1::External { key } if key.as_str() == "image.bin"
    ));
}

#[test]
fn dependency_closure_stops_after_bounded_external_read_witness() {
    let dir = tempfile::tempdir().expect("temp dir");
    let oversized = dir.path().join("oversized.png");
    let file = std::fs::File::create(&oversized).expect("create sparse resource");
    file.set_len(64 * 1024 * 1024 + 1)
        .expect("set bounded-overflow length");
    let bytes = json_bytes(&json!({
        "asset": { "version": "2.0" },
        "images": [
            { "uri": "oversized.png" },
            { "uri": "must-not-be-inspected.png" }
        ]
    }));
    let loaded = animsmith_gltf::load_source_bytes_with_resource_root(
        &dir.path().join("captured.gltf"),
        &bytes,
        dir.path(),
    )
    .expect("optional resource limit is not a load error");

    let closure = loaded.dependency_closure();
    assert_eq!(closure.references().len(), 1);
    assert_eq!(closure.work().external_open_attempts(), 1);
    assert_eq!(
        closure.work().external_bytes_read_hashed(),
        64 * 1024 * 1024 + 1
    );
    assert!(matches!(
        closure.references()[0].target(),
        DependencyReferenceTargetV1::Unavailable {
            reason: DependencyResourceUnavailableReasonV1::ResourceBudgetExceeded,
            ..
        }
    ));
}

#[test]
fn dependency_closure_deduplicates_four_thousand_alias_rows() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join("shared.bin"), b"x").expect("shared resource");
    let images = (0..4_096)
        .map(|_| json!({ "uri": "shared.bin" }))
        .collect::<Vec<_>>();
    let bytes = json_bytes(&json!({
        "asset": { "version": "2.0" },
        "images": images
    }));
    let loaded = animsmith_gltf::load_source_bytes_with_resource_root(
        &dir.path().join("captured.gltf"),
        &bytes,
        dir.path(),
    )
    .expect("all aliases are bounded and reusable");

    let closure = loaded.dependency_closure();
    assert_eq!(closure.references().len(), 4_096);
    assert_eq!(closure.work().dedup_probes(), 4_096);
    assert_eq!(closure.work().external_open_attempts(), 1);
    assert_eq!(closure.work().distinct_external_keys(), 1);
    assert_eq!(closure.external_resources().len(), 1);
}

#[test]
fn dependency_closure_maps_json_data_glb_primary_and_mixed_glb_external_rows() {
    let json_data = json_bytes(&json!({
        "asset": { "version": "2.0" },
        "buffers": [{ "uri": data_uri(&[1, 2, 3, 4]), "byteLength": 4 }]
    }));
    let loaded = animsmith_gltf::load_source_bytes(Path::new("data.gltf"), &json_data)
        .expect("data URI is primary-backed");
    assert!(matches!(
        loaded.dependency_closure().references()[0].target(),
        DependencyReferenceTargetV1::Primary
    ));

    let embedded_glb = glb(
        json!({
            "asset": { "version": "2.0" },
            "buffers": [{ "byteLength": 4 }],
            "bufferViews": [{ "buffer": 0, "byteOffset": 0, "byteLength": 4 }],
            "images": [{ "bufferView": 0, "mimeType": "image/png" }]
        }),
        &[0, 0, 0, 0],
    );
    let loaded = animsmith_gltf::load_source_bytes(Path::new("embedded.glb"), &embedded_glb)
        .expect("GLB BIN and image view are primary-backed");
    assert!(
        loaded
            .dependency_closure()
            .references()
            .iter()
            .all(|reference| matches!(reference.target(), DependencyReferenceTargetV1::Primary))
    );

    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join("external.bin"), [9_u8; 4]).expect("external buffer");
    let mixed = glb(
        json!({
            "asset": { "version": "2.0" },
            "buffers": [
                { "byteLength": 4 },
                { "uri": "external.bin", "byteLength": 4 }
            ]
        }),
        &[0, 0, 0, 0],
    );
    let loaded = animsmith_gltf::load_source_bytes_with_resource_root(
        &dir.path().join("mixed.glb"),
        &mixed,
        dir.path(),
    )
    .expect("mixed GLB uses the primary and one rooted external capture");
    assert!(matches!(
        loaded.dependency_closure().references()[0].target(),
        DependencyReferenceTargetV1::Primary
    ));
    assert!(matches!(
        loaded.dependency_closure().references()[1].target(),
        DependencyReferenceTargetV1::External { key } if key.as_str() == "external.bin"
    ));
}

#[test]
fn dependency_closure_identity_changes_for_each_independent_external_input() {
    let dir = tempfile::tempdir().expect("temp dir");
    let bytes = json_bytes(&json!({
        "asset": { "version": "2.0" },
        "buffers": [{ "uri": "buffer.bin", "byteLength": 4 }],
        "images": [{ "uri": "image.bin" }]
    }));
    let path = dir.path().join("captured.gltf");
    let load = || {
        animsmith_gltf::load_source_bytes_with_resource_root(&path, &bytes, dir.path())
            .expect("rooted source")
    };

    std::fs::write(dir.path().join("buffer.bin"), [1_u8; 4]).expect("buffer bytes");
    std::fs::write(dir.path().join("image.bin"), [2_u8; 4]).expect("image bytes");
    let baseline = load();
    std::fs::write(dir.path().join("buffer.bin"), [3_u8; 4]).expect("mutated buffer");
    let changed_buffer = load();
    std::fs::write(dir.path().join("buffer.bin"), [1_u8; 4]).expect("restored buffer");
    std::fs::write(dir.path().join("image.bin"), [4_u8; 4]).expect("mutated image");
    let changed_image = load();

    assert_eq!(
        baseline.dependency_closure().primary_input(),
        changed_buffer.dependency_closure().primary_input()
    );
    assert_eq!(
        baseline.dependency_closure().primary_input(),
        changed_image.dependency_closure().primary_input()
    );
    assert_ne!(
        baseline.dependency_closure().identity(),
        changed_buffer.dependency_closure().identity()
    );
    assert_ne!(
        baseline.dependency_closure().identity(),
        changed_image.dependency_closure().identity()
    );
}

#[test]
fn dependency_closure_normalizes_percent_aliases_but_keeps_equal_byte_keys_distinct() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join("shared name.bin"), [7_u8; 4]).expect("shared resource");
    let aliases = json_bytes(&json!({
        "asset": { "version": "2.0" },
        "buffers": [{ "uri": "shared%20name.bin", "byteLength": 4 }],
        "images": [{ "uri": "shared name.bin" }]
    }));
    let loaded = animsmith_gltf::load_source_bytes_with_resource_root(
        &dir.path().join("aliases.gltf"),
        &aliases,
        dir.path(),
    )
    .expect("percent-equivalent aliases load once");
    assert_eq!(loaded.dependency_closure().external_resources().len(), 1);
    assert_eq!(
        loaded.dependency_closure().work().external_open_attempts(),
        1
    );

    std::fs::write(dir.path().join("left.bin"), [8_u8; 4]).expect("left bytes");
    std::fs::write(dir.path().join("right.bin"), [8_u8; 4]).expect("right bytes");
    let distinct = json_bytes(&json!({
        "asset": { "version": "2.0" },
        "buffers": [{ "uri": "left.bin", "byteLength": 4 }],
        "images": [{ "uri": "right.bin" }]
    }));
    let loaded = animsmith_gltf::load_source_bytes_with_resource_root(
        &dir.path().join("distinct.gltf"),
        &distinct,
        dir.path(),
    )
    .expect("equal content under distinct keys remains distinct");
    let closure = loaded.dependency_closure();
    assert_eq!(closure.external_resources().len(), 2);
    assert_ne!(
        closure.external_resources()[0].key(),
        closure.external_resources()[1].key()
    );
    assert_eq!(
        closure.external_resources()[0].identity(),
        closure.external_resources()[1].identity()
    );
}

#[test]
fn dependency_closure_keeps_optional_missing_rows_but_fails_essential_buffers() {
    let dir = tempfile::tempdir().expect("temp dir");
    let optional = json_bytes(&json!({
        "asset": { "version": "2.0" },
        "images": [{ "uri": "missing.png" }]
    }));
    let loaded = animsmith_gltf::load_source_bytes_with_resource_root(
        &dir.path().join("optional.gltf"),
        &optional,
        dir.path(),
    )
    .expect("optional missing image remains typed evidence");
    assert!(matches!(
        loaded.dependency_closure().coverage(),
        DependencyClosureCoverageV1::Partial { .. }
    ));
    assert!(matches!(
        loaded.dependency_closure().references()[0].target(),
        DependencyReferenceTargetV1::Unavailable {
            reason: DependencyResourceUnavailableReasonV1::Missing,
            ..
        }
    ));

    let essential = json_bytes(&json!({
        "asset": { "version": "2.0" },
        "buffers": [{ "uri": "missing.bin", "byteLength": 4 }]
    }));
    let error = animsmith_gltf::load_source_bytes_with_resource_root(
        &dir.path().join("essential.gltf"),
        &essential,
        dir.path(),
    )
    .expect_err("missing essential buffer is a load error");
    assert!(
        error
            .to_string()
            .contains("external buffer resource is unavailable"),
        "{error}"
    );
    assert!(!error.to_string().contains("missing.bin"), "{error}");
}

#[test]
fn dependency_closure_identity_includes_source_declaration_order() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join("first.bin"), [1_u8]).expect("first bytes");
    std::fs::write(dir.path().join("second.bin"), [2_u8]).expect("second bytes");
    let load = |images: Value| {
        let bytes = json_bytes(&json!({
            "asset": { "version": "2.0" },
            "images": images
        }));
        animsmith_gltf::load_source_bytes_with_resource_root(
            &dir.path().join("ordered.gltf"),
            &bytes,
            dir.path(),
        )
        .expect("ordered resources")
    };
    let first_then_second = load(json!([
        { "uri": "first.bin" },
        { "uri": "second.bin" }
    ]));
    let second_then_first = load(json!([
        { "uri": "second.bin" },
        { "uri": "first.bin" }
    ]));
    assert!(matches!(
        first_then_second.dependency_closure().references()[0].target(),
        DependencyReferenceTargetV1::External { key } if key.as_str() == "first.bin"
    ));
    assert!(matches!(
        second_then_first.dependency_closure().references()[0].target(),
        DependencyReferenceTargetV1::External { key } if key.as_str() == "second.bin"
    ));
    assert_ne!(
        first_then_second.dependency_closure().identity(),
        second_then_first.dependency_closure().identity()
    );
}

#[test]
fn dependency_closure_refusal_table_never_opens_or_retains_unsafe_spellings() {
    let dir = tempfile::tempdir().expect("temp dir");
    for (uri, expected) in [
        ("../escape.png", DependencyResourceRefusalReasonV1::Escaping),
        ("/absolute.png", DependencyResourceRefusalReasonV1::Absolute),
        (
            "https://example.invalid/image.png",
            DependencyResourceRefusalReasonV1::Remote,
        ),
        ("malformed%2", DependencyResourceRefusalReasonV1::Malformed),
        (
            "query.png?value",
            DependencyResourceRefusalReasonV1::Malformed,
        ),
    ] {
        let bytes = json_bytes(&json!({
            "asset": { "version": "2.0" },
            "images": [{ "uri": uri }]
        }));
        let loaded = animsmith_gltf::load_source_bytes_with_resource_root(
            &dir.path().join("refusal.gltf"),
            &bytes,
            dir.path(),
        )
        .expect("optional unsafe image is typed evidence");
        let closure = loaded.dependency_closure();
        assert_eq!(closure.work().external_open_attempts(), 0, "{uri}");
        assert!(matches!(
            closure.references()[0].target(),
            DependencyReferenceTargetV1::Refused { reason, .. } if *reason == expected
        ));
        assert!(!format!("{closure:?}").contains(uri), "{uri}");
        assert!(!format!("{closure:?}").contains(&dir.path().display().to_string()));
    }
}

#[test]
fn unsafe_resource_uris_do_not_consume_retained_locator_budget() {
    // The combined raw URI bytes exceed the aggregate retained-text budget,
    // but escaping locators are redacted and retain none of those bytes.
    let escaping_uri = format!("../{}", "x".repeat(RAW_SOURCE_V1_MAX_TEXT_BYTES - 3));
    let images = (0..2_100)
        .map(|_| json!({ "uri": escaping_uri }))
        .collect::<Vec<_>>();
    let bytes = json_bytes(&json!({
        "asset": { "version": "2.0" },
        "images": images
    }));
    let loaded = animsmith_gltf::load_source_bytes(Path::new("redacted-budget.gltf"), &bytes)
        .expect("unsafe resource locators are redacted before text accounting");
    let resources = loaded.source_facts().resources();
    assert_eq!(
        resources.coverage().state(),
        SourceSetCoverageStateV1::Complete
    );
    assert_eq!(resources.rows().len(), 2_100);
    assert!(
        resources
            .rows()
            .iter()
            .all(|row| matches!(row.locator(), SourceResourceLocatorV1::Escaping))
    );
}

#[test]
fn oversized_data_uri_remains_embedded_for_source_aware_scale() {
    let payload = vec![0u8; RAW_SOURCE_V1_MAX_TEXT_BYTES];
    let bytes = json_bytes(&json!({
        "asset": { "version": "2.0" },
        "scene": 0,
        "scenes": [{ "nodes": [0] }],
        "nodes": [{ "translation": [1.0, 2.0, 3.0] }],
        "buffers": [{ "uri": data_uri(&payload), "byteLength": payload.len() }]
    }));
    let source =
        animsmith_gltf::preflight_scale_source_bytes(Path::new("oversized-data-uri.gltf"), &bytes)
            .expect("self-contained data URI source preflights");
    assert!(matches!(
        source.source_facts().resources().rows()[0].locator(),
        SourceResourceLocatorV1::DataUri
    ));
    let operation = ScaleOperation::WholeDocumentLinearUnits { factor: 2.0 };
    let facts = animsmith_gltf::operation_capability_facts_for_source(&source, operation)
        .expect("redacted data URI stays self-contained");
    assert!(!facts.external_resources_present);
    animsmith_gltf::rewrite_linear_units(&source, 2.0)
        .expect("oversized embedded payload does not regress scaling");
}

#[test]
fn source_aware_scale_fails_closed_on_partial_resource_projection() {
    let images = (0..=animsmith_core::RAW_SOURCE_V1_MAX_RESOURCE_REFERENCES)
        .map(|_| json!({ "uri": "data:image/png;base64," }))
        .collect::<Vec<_>>();
    let bytes = json_bytes(&json!({
        "asset": { "version": "2.0" },
        "images": images
    }));
    let source =
        animsmith_gltf::preflight_scale_source_bytes(Path::new("partial-resources.gltf"), &bytes)
            .expect("embedded declarations pass raw scale preflight");
    assert_eq!(
        source.source_facts().resources().coverage().state(),
        SourceSetCoverageStateV1::Partial
    );
    let error = animsmith_gltf::operation_capability_facts_for_source(
        &source,
        ScaleOperation::WholeDocumentLinearUnits { factor: 2.0 },
    )
    .expect_err("partial shared resource coverage must fail closed");
    assert!(matches!(
        error,
        animsmith_gltf::GltfScaleRewriteError::Plan(ScaleError::IncompleteCapability)
    ));
}

#[test]
fn clean_source_aware_scale_projection_matches_the_manifest_projection() {
    let bytes = json_bytes(&json!({ "asset": { "version": "2.0" } }));
    let source = animsmith_gltf::preflight_scale_source_bytes(Path::new("clean.gltf"), &bytes)
        .expect("clean source preflights");
    assert_eq!(
        animsmith_gltf::capability_facts_for_source(&source),
        animsmith_gltf::capability_facts(source.manifest())
    );
    let operation = ScaleOperation::WholeDocumentLinearUnits { factor: 2.0 };
    assert_eq!(
        animsmith_gltf::operation_capability_facts_for_source(&source, operation)
            .expect("clean source-aware projection"),
        animsmith_gltf::operation_capability_facts(source.manifest(), operation)
            .expect("clean manifest projection")
    );
}
