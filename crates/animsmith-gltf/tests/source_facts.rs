//! Analytic coverage for the immutable glTF raw-source-facts projection.

use animsmith_core::scale::{ScaleError, ScaleOperation};
use animsmith_core::{
    InputIdentity, RAW_SOURCE_V1_MAX_TEXT_BYTES, SourceAxisV1, SourceChannelPropertyV1,
    SourceFormatV1, SourceHandednessV1, SourceInterpolationV1, SourceLoaderDispositionV1,
    SourceObservationStateV1, SourceProvenanceKindV1, SourceResourceKindV1,
    SourceResourceLocatorV1, SourceSetCoverageStateV1, SourceUnavailableReasonV1,
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
    value["buffers"] = json!([{ "byteLength": bin.len() }]);
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
        "extensionsUsed": ["Z_vendor", "A_vendor"]
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
    let loaded = animsmith_gltf::load_source_bytes(&path, &bytes).expect("resources load");
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
