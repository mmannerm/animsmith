//! Public raw-capability preflight contract tests.

use animsmith_core::RAW_SOURCE_V1_MAX_CLIPS;
use animsmith_gltf::{
    GltfBufferSourceKind, GltfCapabilityViolation, GltfCapabilityViolationKind, GltfContainerKind,
    GltfNodeRestKind, GltfScalePreflightError, preflight_clip_track_source_bytes,
    preflight_scale_source, preflight_scale_source_bytes,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use std::path::Path;

const ZERO_F32X9: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
const ZERO_F32X12: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
const ZERO_F32X16: &str =
    "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA==";

fn base_json() -> Value {
    json!({
        "asset": { "version": "2.0" },
        "buffers": [{
            "uri": format!("data:application/octet-stream;base64,{ZERO_F32X9}"),
            "byteLength": 36
        }],
        "bufferViews": [{ "buffer": 0, "byteLength": 36 }],
        "accessors": [{
            "bufferView": 0,
            "componentType": 5126,
            "count": 3,
            "type": "VEC3",
            "min": [0, 0, 0],
            "max": [0, 0, 0]
        }],
        "meshes": [{ "primitives": [{ "attributes": { "POSITION": 0 } }] }]
    })
}

fn bytes(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).expect("literal JSON serializes")
}

fn data_uri(buffer: &[u8]) -> String {
    format!(
        "data:application/octet-stream;base64,{}",
        STANDARD.encode(buffer)
    )
}

fn glb(value: &Value, bin: &[u8]) -> Vec<u8> {
    let mut json = bytes(value);
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

fn unsupported(
    value: &Value,
) -> (
    Vec<GltfCapabilityViolation>,
    animsmith_gltf::GltfCapabilityManifest,
) {
    match preflight_scale_source_bytes(Path::new("synthetic.gltf"), &bytes(value)) {
        Err(GltfScalePreflightError::Unsupported {
            manifest,
            violations,
            count,
        }) => {
            assert_eq!(count, violations.len());
            (violations, *manifest)
        }
        other => panic!("expected typed capability rejection, got {other:?}"),
    }
}

fn clip_track_unsupported(
    value: &Value,
) -> (
    Vec<GltfCapabilityViolation>,
    animsmith_gltf::GltfCapabilityManifest,
) {
    match preflight_clip_track_source_bytes(Path::new("synthetic.gltf"), &bytes(value)) {
        Err(GltfScalePreflightError::Unsupported {
            manifest,
            violations,
            count,
        }) => {
            assert_eq!(count, violations.len());
            (violations, *manifest)
        }
        other => panic!("expected typed clip-track rejection, got {other:?}"),
    }
}

fn assert_has(violations: &[GltfCapabilityViolation], kind: GltfCapabilityViolationKind) {
    assert!(
        violations.iter().any(|violation| violation.kind == kind),
        "missing {kind:?} in {violations:#?}"
    );
}

/// Every location reported for `kind`, in the preflight's own sorted order.
fn locations(
    violations: &[GltfCapabilityViolation],
    kind: GltfCapabilityViolationKind,
) -> Vec<&str> {
    violations
        .iter()
        .filter(|violation| violation.kind == kind)
        .map(|violation| violation.location.as_str())
        .collect()
}

#[test]
fn accepts_self_contained_gltf_and_equivalent_glb_without_writing() {
    let mut gltf = base_json();
    gltf["nodes"] = json!([{ "translation": [2, 0, 0] }]);
    let gltf_bytes = bytes(&gltf);
    let directory = tempfile::tempdir().expect("temporary provenance directory");
    let input = directory.path().join("input.gltf");

    let source = preflight_scale_source_bytes(&input, &gltf_bytes).expect("data-URI glTF accepted");
    assert_eq!(source.source_bytes(), gltf_bytes);
    assert_eq!(source.raw_json(), &gltf);
    assert_eq!(source.document().skeleton.bones[0].rest.translation.x, 2.0);
    assert_eq!(source.manifest().container, GltfContainerKind::Gltf);
    assert_eq!(
        source.manifest().buffers[0].source_kind,
        GltfBufferSourceKind::DataUri
    );
    assert_eq!(source.resolved_buffers(), &[vec![0; 36]]);
    assert_eq!(source.manifest().buffers[0].buffer_index, 0);
    assert_eq!(source.manifest().buffers[0].declared_byte_length, 36);
    assert_eq!(source.manifest().buffer_views[0].buffer_index, 0);
    assert_eq!(source.manifest().buffer_views[0].byte_offset, 0);
    assert_eq!(source.manifest().buffer_views[0].byte_length, 36);
    assert_eq!(source.manifest().buffer_views[0].byte_stride, None);
    assert_eq!(source.manifest().accessors[0].buffer_view_index, Some(0));
    assert_eq!(source.manifest().accessors[0].byte_offset, 0);
    assert_eq!(source.manifest().accessors[0].component_type, 5126);
    assert_eq!(source.manifest().accessors[0].accessor_type, "VEC3");
    assert_eq!(source.manifest().accessors[0].count, 3);
    assert!(!source.manifest().accessors[0].normalized);
    assert!(!source.manifest().accessors[0].sparse);
    assert_eq!(
        source.manifest().primitives[0].attributes[0].semantic,
        "POSITION"
    );
    assert_eq!(
        source.manifest().primitives[0].attributes[0].accessor_index,
        0
    );
    assert!(
        std::fs::read_dir(directory.path())
            .unwrap()
            .next()
            .is_none(),
        "preflight creates no output"
    );

    let mut glb_json = base_json();
    glb_json["buffers"][0] = json!({ "byteLength": 36 });
    let glb_bytes = glb(&glb_json, &[0; 36]);
    let glb_source = preflight_scale_source_bytes(Path::new("synthetic.glb"), &glb_bytes)
        .expect("equivalent GLB accepted");
    assert_eq!(glb_source.manifest().container, GltfContainerKind::Glb);
    assert_eq!(glb_source.source_bytes(), glb_bytes);
    assert_eq!(
        glb_source.manifest().buffers[0].source_kind,
        GltfBufferSourceKind::BinaryChunk
    );
    assert_eq!(glb_source.resolved_buffers(), &[vec![0; 36]]);
}

#[test]
fn accepts_every_primary_attribute_with_distinct_valid_accessor_storage() {
    let mut value = base_json();
    let mut buffer = vec![0; 176];
    buffer[168..174].copy_from_slice(&[0, 0, 1, 0, 2, 0]);
    value["buffers"][0] = json!({ "uri": data_uri(&buffer), "byteLength": 176 });
    value["bufferViews"] = json!([
        { "buffer": 0, "byteOffset": 0, "byteLength": 36 },
        { "buffer": 0, "byteOffset": 36, "byteLength": 36 },
        { "buffer": 0, "byteOffset": 72, "byteLength": 24 },
        { "buffer": 0, "byteOffset": 96, "byteLength": 24 },
        { "buffer": 0, "byteOffset": 120, "byteLength": 48 },
        { "buffer": 0, "byteOffset": 168, "byteLength": 6 }
    ]);
    value["accessors"] = json!([
        { "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3", "min": [0, 0, 0], "max": [0, 0, 0] },
        { "bufferView": 1, "componentType": 5126, "count": 3, "type": "VEC3" },
        { "bufferView": 2, "componentType": 5126, "count": 3, "type": "VEC2" },
        { "bufferView": 3, "componentType": 5123, "count": 3, "type": "VEC4" },
        { "bufferView": 4, "componentType": 5126, "count": 3, "type": "VEC4" },
        { "bufferView": 5, "componentType": 5123, "count": 3, "type": "SCALAR" }
    ]);
    value["meshes"][0]["primitives"][0] = json!({
        "attributes": {
            "WEIGHTS_0": 4,
            "POSITION": 0,
            "TEXCOORD_0": 2,
            "NORMAL": 1,
            "JOINTS_0": 3
        },
        "indices": 5
    });

    let source = preflight_scale_source_bytes(Path::new("primary.gltf"), &bytes(&value))
        .expect("the complete primary attribute subset is accepted");

    assert_eq!(
        source.manifest().primitives[0]
            .attributes
            .iter()
            .map(|attribute| (attribute.semantic.as_str(), attribute.accessor_index))
            .collect::<Vec<_>>(),
        vec![
            ("JOINTS_0", 3),
            ("NORMAL", 1),
            ("POSITION", 0),
            ("TEXCOORD_0", 2),
            ("WEIGHTS_0", 4),
        ]
    );
    assert_eq!(source.resolved_buffers(), &[buffer]);
}

#[test]
fn accepts_dense_inverse_binds_and_ordinary_translation_animation() {
    let mut value = base_json();
    let mut buffer = vec![0; 132];
    for matrix_component in [0usize, 5, 10, 15] {
        let offset = 36 + matrix_component * 4;
        buffer[offset..offset + 4].copy_from_slice(&1.0f32.to_le_bytes());
    }
    buffer[104..108].copy_from_slice(&1.0f32.to_le_bytes());
    value["buffers"][0] = json!({ "uri": data_uri(&buffer), "byteLength": 132 });
    value["bufferViews"] = json!([
        { "buffer": 0, "byteOffset": 0, "byteLength": 36 },
        { "buffer": 0, "byteOffset": 36, "byteLength": 64 },
        { "buffer": 0, "byteOffset": 100, "byteLength": 8 },
        { "buffer": 0, "byteOffset": 108, "byteLength": 24 }
    ]);
    value["accessors"] = json!([
        { "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3", "min": [0, 0, 0], "max": [0, 0, 0] },
        { "bufferView": 1, "componentType": 5126, "count": 1, "type": "MAT4" },
        { "bufferView": 2, "componentType": 5126, "count": 2, "type": "SCALAR", "min": [0], "max": [1] },
        { "bufferView": 3, "componentType": 5126, "count": 2, "type": "VEC3" }
    ]);
    value["nodes"] = json!([{}, { "mesh": 0, "skin": 0 }]);
    value["skins"] = json!([{
        "joints": [0],
        "skeleton": 0,
        "inverseBindMatrices": 1
    }]);
    value["animations"] = json!([{
        "samplers": [{ "input": 2, "interpolation": "LINEAR", "output": 3 }],
        "channels": [{ "sampler": 0, "target": { "node": 0, "path": "translation" } }]
    }]);

    let source = preflight_scale_source_bytes(Path::new("skinned.gltf"), &bytes(&value))
        .expect("ordinary dense skin and translation animation are supported");

    assert_eq!(source.manifest().skins[0].skin_index, 0);
    assert_eq!(source.manifest().skins[0].joint_count, 1);
    assert_eq!(
        source.manifest().skins[0].inverse_bind_accessor_index,
        Some(1)
    );
    assert_eq!(source.manifest().skins[0].inverse_bind_count, Some(1));
    assert_eq!(source.manifest().nodes[1].skin_index, Some(0));
    assert_eq!(source.manifest().animation_channels[0].target_node_index, 0);
    assert_eq!(
        source.manifest().animation_channels[0].target_path,
        "translation"
    );
    assert_eq!(
        source.manifest().animation_channels[0].interpolation,
        "LINEAR"
    );
    assert_eq!(
        source.manifest().animation_channels[0].input_accessor_index,
        2
    );
    assert_eq!(
        source.manifest().animation_channels[0].output_accessor_index,
        3
    );
    assert!(!source.requires_clip_track_projection());

    let clip_source = preflight_clip_track_source_bytes(Path::new("skinned.gltf"), &bytes(&value))
        .expect("a clean skinned source also passes clip-track preflight");
    assert!(
        !clip_source.requires_clip_track_projection(),
        "a clean skinned source keeps its established rest/bind application"
    );
    assert_eq!(clip_source.source_bytes(), source.source_bytes());
    assert_eq!(
        clip_source.document().assets.meshes.len(),
        source.document().assets.meshes.len()
    );
    assert_eq!(
        clip_source.document().assets.instances.len(),
        source.document().assets.instances.len()
    );
    assert_eq!(
        clip_source.document().assets.materials.len(),
        source.document().assets.materials.len()
    );
    assert_eq!(
        clip_source.document().assets.scenes.len(),
        source.document().assets.scenes.len()
    );
    assert_eq!(
        clip_source.document().assets.source_skeleton.nodes.len(),
        source.document().assets.source_skeleton.nodes.len()
    );
    assert_eq!(
        clip_source.document().assets.source_skeleton.skins.len(),
        source.document().assets.source_skeleton.skins.len()
    );
}

/// A valid clip source with one deliberately unreadable bind-only accessor.
///
/// The bad `byteStride` is unsafe for inverse-bind reading, but all animation
/// accessors are dense and valid. The role-specific projection must retain the
/// source manifest while omitting the mesh/bind payload from its document.
fn skinned_animation_with_unreadable_bind_layout() -> Value {
    let mut value = base_json();
    let mut buffer = vec![0; 132];
    buffer[100..104].copy_from_slice(&1.0f32.to_le_bytes());
    value["buffers"][0] = json!({ "uri": data_uri(&buffer), "byteLength": 132 });
    value["bufferViews"] = json!([
        { "buffer": 0, "byteOffset": 0, "byteLength": 36 },
        { "buffer": 0, "byteOffset": 36, "byteLength": 64, "byteStride": 4 },
        { "buffer": 0, "byteOffset": 100, "byteLength": 8 },
        { "buffer": 0, "byteOffset": 108, "byteLength": 24 }
    ]);
    value["accessors"] = json!([
        { "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3", "min": [0, 0, 0], "max": [0, 0, 0] },
        { "bufferView": 1, "componentType": 5126, "count": 1, "type": "MAT4" },
        { "bufferView": 2, "componentType": 5126, "count": 2, "type": "SCALAR", "min": [0], "max": [1] },
        { "bufferView": 3, "componentType": 5126, "count": 2, "type": "VEC3" }
    ]);
    value["nodes"] = json!([{}, { "mesh": 0, "skin": 0 }]);
    value["skins"] = json!([{
        "joints": [0],
        "skeleton": 0,
        "inverseBindMatrices": 1
    }]);
    value["animations"] = json!([{
        "samplers": [{ "input": 2, "interpolation": "LINEAR", "output": 3 }],
        "channels": [{ "sampler": 0, "target": { "node": 0, "path": "translation" } }]
    }]);
    value
}

#[test]
fn clip_track_preflight_accepts_bind_only_layout_fault_and_retains_identity_manifest() {
    let value = skinned_animation_with_unreadable_bind_layout();
    let raw = bytes(&value);
    let (violations, _) = unsupported(&value);
    assert_has(
        &violations,
        GltfCapabilityViolationKind::UnsafeAccessorLayout,
    );

    let source = preflight_clip_track_source_bytes(Path::new("bind-only.gltf"), &raw)
        .expect("clip-track projection ignores only the unreadable bind payload");
    assert_eq!(source.source_bytes(), raw);
    assert_eq!(
        source.source_facts().primary_identity().bytes(),
        raw.len() as u64
    );
    assert_eq!(
        source.manifest().skins[0].inverse_bind_accessor_index,
        Some(1)
    );
    assert_eq!(source.manifest().accessors[1].buffer_view_index, Some(1));
    assert_eq!(source.document().clips.len(), 1);
    assert_eq!(source.document().clips[0].tracks.len(), 1);
    assert!(source.document().assets.instances.is_empty());
    assert_eq!(source.document().assets.source_skeleton.skins.len(), 1);
    assert!(source.requires_clip_track_projection());
}

#[test]
fn clip_track_preflight_refuses_unsafe_animation_accessor_layout() {
    let mut value = skinned_animation_with_unreadable_bind_layout();
    value["bufferViews"][2]["byteLength"] = json!(4);

    let (violations, _) = clip_track_unsupported(&value);
    assert!(
        locations(
            &violations,
            GltfCapabilityViolationKind::UnsafeAccessorLayout
        )
        .contains(&"/accessors/2")
    );
}

#[test]
fn clip_track_preflight_refuses_bind_alias_on_an_animation_accessor() {
    let mut value = skinned_animation_with_unreadable_bind_layout();
    value["skins"][0]["inverseBindMatrices"] = json!(2);

    let (violations, _) = clip_track_unsupported(&value);
    assert!(
        locations(
            &violations,
            GltfCapabilityViolationKind::ConflictingAccessorUse
        )
        .contains(&"/accessors/2")
    );
}

#[test]
fn clip_track_preflight_refuses_external_dependency() {
    let mut value = base_json();
    value["buffers"][0]["uri"] = json!("clip.bin");

    let (violations, manifest) = clip_track_unsupported(&value);
    assert_has(&violations, GltfCapabilityViolationKind::ExternalResource);
    assert_eq!(manifest.external_resource_locations, vec!["/buffers/0/uri"]);
}

#[test]
fn clip_track_preflight_refuses_clean_source_with_incomplete_clip_facts() {
    let mut value = base_json();
    value["animations"] = Value::Array(
        (0..=RAW_SOURCE_V1_MAX_CLIPS)
            .map(|index| {
                json!({
                    "name": format!("clip-{index}"),
                    "samplers": [],
                    "channels": []
                })
            })
            .collect(),
    );

    let error = preflight_clip_track_source_bytes(Path::new("many-clips.gltf"), &bytes(&value))
        .expect_err("clip-track consumers require complete raw clip coverage");
    assert!(
        error
            .to_string()
            .contains("clip-track raw source facts coverage is incomplete"),
        "unexpected error: {error}"
    );
}

#[test]
fn clip_track_preflight_retains_animated_matrix_node() {
    let mut value = skinned_animation_with_unreadable_bind_layout();
    value["bufferViews"][1]
        .as_object_mut()
        .expect("literal buffer view object")
        .remove("byteStride");
    value["nodes"][0] = json!({
        "matrix": [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]
    });

    let (violations, _) = clip_track_unsupported(&value);
    assert_eq!(
        locations(&violations, GltfCapabilityViolationKind::AnimatedMatrixNode),
        vec!["/animations/0/channels/0/target"]
    );
}

#[test]
fn filesystem_api_captures_nonzero_bytes_without_reopening_external_resources() {
    let mut value = base_json();
    let mut buffer = vec![0; 36];
    buffer[0..4].copy_from_slice(&1.0f32.to_le_bytes());
    value["buffers"][0]["uri"] = json!(data_uri(&buffer));
    let source_bytes = bytes(&value);
    let directory = tempfile::tempdir().expect("temporary source directory");
    let input = directory.path().join("source.gltf");
    std::fs::write(&input, &source_bytes).expect("write synthetic source");

    let source = preflight_scale_source(&input).expect("filesystem preflight accepts source");

    assert_eq!(source.source_bytes(), source_bytes);
    assert_eq!(source.resolved_buffers(), &[buffer]);
    assert_eq!(std::fs::read(&input).unwrap(), source_bytes);
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[test]
fn inventories_trs_and_matrix_nodes_from_raw_json() {
    let mut value = base_json();
    value["nodes"] = json!([
        { "translation": [1, 2, 3], "mesh": 0 },
        { "matrix": [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1] }
    ]);
    let source = preflight_scale_source_bytes(Path::new("nodes.gltf"), &bytes(&value))
        .expect("TRS and matrix are both inventoried");
    assert_eq!(
        source
            .manifest()
            .nodes
            .iter()
            .map(|node| node.rest_kind)
            .collect::<Vec<_>>(),
        vec![GltfNodeRestKind::Trs, GltfNodeRestKind::Matrix]
    );
    assert_eq!(source.manifest().nodes[0].node_index, 0);
    assert_eq!(source.manifest().nodes[0].mesh_index, Some(0));
    assert_eq!(source.manifest().nodes[0].skin_index, None);
    assert_eq!(source.manifest().nodes[1].node_index, 1);
}

// --- Out-of-contract node transforms (#301) ---------------------------------

/// The identity node `matrix`, column-major.
const IDENTITY_MATRIX: [f64; 16] = [
    1.0, 0.0, 0.0, 0.0, //
    0.0, 1.0, 0.0, 0.0, //
    0.0, 0.0, 1.0, 0.0, //
    0.0, 0.0, 0.0, 1.0,
];

/// `base_json` carrying one node with `matrix` plus the given extra members.
fn node_matrix_document(matrix: [f64; 16], extras: &[(&str, Value)]) -> Value {
    let mut node = json!({ "matrix": Vec::from(matrix) });
    for (member, value) in extras {
        node[*member] = value.clone();
    }
    let mut value = base_json();
    value["nodes"] = json!([node]);
    value
}

#[test]
fn rejects_a_node_declaring_matrix_alongside_any_trs_member() {
    // glTF 2.0 §3.5 makes `matrix` and TRS mutually exclusive, but the `gltf`
    // crate parses the combination and silently honours `matrix`, so nothing
    // below the gate can tell which the author meant.
    for (member, member_value) in [
        ("translation", json!([1.5, -2.0, 0.25])),
        ("rotation", json!([0.0, 0.0, 0.0, 1.0])),
        ("scale", json!([2.0, 2.0, 2.0])),
    ] {
        let value = node_matrix_document(IDENTITY_MATRIX, &[(member, member_value)]);
        let (violations, manifest) = unsupported(&value);
        assert_eq!(
            locations(
                &violations,
                GltfCapabilityViolationKind::ConflictingNodeTransform
            ),
            vec![format!("/nodes/0/{member}")],
            "matrix + {member}"
        );
        assert!(
            locations(
                &violations,
                GltfCapabilityViolationKind::NonAffineNodeMatrix
            )
            .is_empty(),
            "matrix + {member}: the identity matrix is affine"
        );
        assert_eq!(manifest.nodes[0].rest_kind, GltfNodeRestKind::Matrix);
    }

    // All three at once report all three members, sorted by location.
    let value = node_matrix_document(
        IDENTITY_MATRIX,
        &[
            ("translation", json!([1.5, -2.0, 0.25])),
            ("rotation", json!([0.0, 0.0, 0.0, 1.0])),
            ("scale", json!([2.0, 2.0, 2.0])),
        ],
    );
    let (violations, _) = unsupported(&value);
    assert_eq!(
        locations(
            &violations,
            GltfCapabilityViolationKind::ConflictingNodeTransform
        ),
        vec![
            "/nodes/0/rotation",
            "/nodes/0/scale",
            "/nodes/0/translation"
        ]
    );
}

#[test]
fn rejects_a_node_matrix_whose_last_row_is_not_affine() {
    // The whole-document conversion is `M' = U M U^-1` for a uniform
    // `U = scale(q)`, which leaves entries 3, 7, 11 and 15 alone. That is the
    // converted transform only when they are `(0, 0, 0, 1)`: a projective
    // entry transforms as `1/q`, so the gate refuses rather than answer wrong.
    for (component, authored) in [(3usize, 0.5), (7, -1.0), (11, 2.0), (15, 2.0)] {
        let mut matrix = IDENTITY_MATRIX;
        matrix[component] = authored;
        let value = node_matrix_document(matrix, &[]);
        let (violations, _) = unsupported(&value);
        assert_eq!(
            locations(
                &violations,
                GltfCapabilityViolationKind::NonAffineNodeMatrix
            ),
            vec![format!("/nodes/0/matrix/{component}")],
            "matrix[{component}] = {authored}"
        );
        assert!(
            locations(
                &violations,
                GltfCapabilityViolationKind::ConflictingNodeTransform
            )
            .is_empty(),
            "matrix[{component}] = {authored}: no TRS member is declared"
        );
    }

    // The comparison is exact, so the near-valid band is refused too: each of
    // these is inside every tolerance this workspace declares — `1e-5` for
    // `equal_axis` and `common_factor` — and none of them reaches one. This is
    // the claim DESIGN.md Appendix D §D.3 case 4 makes in prose, pinned here
    // so loosening the gate to a tolerance comparison fails a test rather than
    // silently publishing an unconverted `3, 7, 11` or a projective `15`.
    for (component, authored) in [
        (15usize, 1.000_000_1_f64),
        (15, 1.0 + f64::EPSILON),
        (3, 1e-12),
    ] {
        let mut matrix = IDENTITY_MATRIX;
        matrix[component] = authored;
        let value = node_matrix_document(matrix, &[]);
        let (violations, _) = unsupported(&value);
        assert_eq!(
            locations(
                &violations,
                GltfCapabilityViolationKind::NonAffineNodeMatrix
            ),
            vec![format!("/nodes/0/matrix/{component}")],
            "matrix[{component}] = {authored:?}"
        );
        assert_eq!(
            violations.len(),
            1,
            "matrix[{component}] = {authored:?}: the near-valid entry is the \
             only thing wrong with the document"
        );
    }

    // One matrix breaking all four entries reports all four, and the sorted
    // order is lexical over the JSON pointers rather than numeric.
    let mut matrix = IDENTITY_MATRIX;
    for component in [3usize, 7, 11, 15] {
        matrix[component] = 0.5;
    }
    let value = node_matrix_document(matrix, &[]);
    let (violations, _) = unsupported(&value);
    assert_eq!(
        locations(
            &violations,
            GltfCapabilityViolationKind::NonAffineNodeMatrix
        ),
        vec![
            "/nodes/0/matrix/11",
            "/nodes/0/matrix/15",
            "/nodes/0/matrix/3",
            "/nodes/0/matrix/7"
        ]
    );
}

#[test]
fn accepts_an_affine_node_matrix_and_a_matrixless_trs_node() {
    // The guard must not reject the ordinary cases. A `matrix` carrying a
    // translation column is affine — entries 3, 7 and 11 are zero and entry
    // 15 is one — and a node declaring TRS *without* `matrix` declares no
    // conflict at all.
    let mut matrix = IDENTITY_MATRIX;
    matrix[12] = 1.5;
    matrix[13] = -2.0;
    matrix[14] = 0.25;
    let value = node_matrix_document(matrix, &[]);
    let source = preflight_scale_source_bytes(Path::new("affine-matrix.gltf"), &bytes(&value))
        .expect("an affine matrix with a translation column preflights");
    assert_eq!(
        source.manifest().nodes[0].rest_kind,
        GltfNodeRestKind::Matrix
    );

    let mut trs = base_json();
    trs["nodes"] = json!([{
        "translation": [1.5, -2.0, 0.25],
        "rotation": [0.0, 0.0, 0.0, 1.0],
        "scale": [2.0, 2.0, 2.0]
    }]);
    let source = preflight_scale_source_bytes(Path::new("trs-node.gltf"), &bytes(&trs))
        .expect("a node declaring all three TRS members and no matrix preflights");
    assert_eq!(source.manifest().nodes[0].rest_kind, GltfNodeRestKind::Trs);
}

#[test]
fn a_transform_member_authored_as_json_null_is_not_a_declaration() {
    // `serde_json` reports `"matrix": null` as `Some(Value::Null)`, but the
    // typed glTF parse deserializes the same member into `Option<[f32; 16]>`
    // as `None`. A gate asking only whether the *key* is present therefore
    // disagrees with the loader about what the node declared: it reads this
    // as a node carrying both transforms, refuses a document the loader reads
    // as plain TRS, and names `/nodes/0/translation` — the one member that is
    // genuinely declared and genuinely innocent — as the offender.
    let mut null_matrix = base_json();
    null_matrix["nodes"] = json!([{ "matrix": null, "translation": [1.5, -2.0, 0.25] }]);
    let source = preflight_scale_source_bytes(Path::new("null-matrix.gltf"), &bytes(&null_matrix))
        .expect("a null `matrix` declares no matrix, so this is a plain TRS node");
    assert_eq!(source.manifest().nodes[0].rest_kind, GltfNodeRestKind::Trs);

    // The mirror. An affine `matrix` beside a null `translation` is a plain
    // matrix node, and `/nodes/0/matrix` is not the offender either.
    let mut matrix = IDENTITY_MATRIX;
    matrix[12] = 1.5;
    matrix[13] = -2.0;
    matrix[14] = 0.25;
    let value = node_matrix_document(matrix, &[("translation", Value::Null)]);
    let source = preflight_scale_source_bytes(Path::new("null-translation.gltf"), &bytes(&value))
        .expect("a null `translation` declares no translation");
    assert_eq!(
        source.manifest().nodes[0].rest_kind,
        GltfNodeRestKind::Matrix
    );

    // A null `matrix` alone is a TRS node at glTF's identity defaults.
    let mut only_null_matrix = base_json();
    only_null_matrix["nodes"] = json!([{ "matrix": null }]);
    let source = preflight_scale_source_bytes(
        Path::new("only-null-matrix.gltf"),
        &bytes(&only_null_matrix),
    )
    .expect("a null `matrix` alone declares no matrix");
    assert_eq!(source.manifest().nodes[0].rest_kind, GltfNodeRestKind::Trs);
}

// --- Image payloads sharing bytes with a scale-bearing accessor (#300) ------

/// A 96-byte buffer holding a `POSITION` accessor, an optional `NORMAL`
/// accessor, and one image payload, at caller-chosen offsets.
///
/// `POSITION` is on `bufferView 1` and occupies
/// `position_offset .. position_offset + 36`; the image is on `bufferView 2`
/// and occupies `image_offset .. image_offset + image_length`.
///
/// `bufferView 0` is an unreferenced filler at `[84, 96)`, which no accessor
/// and no image ever reaches. It is there so the image is never the *first*
/// view: a sweep reading a fixed view rather than the one the image names
/// would otherwise answer from the image's own range by coincidence, and
/// every case below would still pass.
fn image_and_positions(
    image_offset: usize,
    image_length: usize,
    position_offset: usize,
    normal_offset: Option<usize>,
) -> Value {
    let mut buffer_views = vec![
        json!({ "buffer": 0, "byteOffset": 84, "byteLength": 12 }),
        json!({ "buffer": 0, "byteOffset": position_offset, "byteLength": 36 }),
        json!({ "buffer": 0, "byteOffset": image_offset, "byteLength": image_length }),
    ];
    let mut accessors = vec![json!({
        "bufferView": 1, "componentType": 5126, "count": 3, "type": "VEC3",
        "min": [0, 0, 0], "max": [0, 0, 0]
    })];
    let mut attributes = json!({ "POSITION": 0 });
    if let Some(normal_offset) = normal_offset {
        buffer_views.push(json!({ "buffer": 0, "byteOffset": normal_offset, "byteLength": 36 }));
        accessors.push(json!({
            "bufferView": 3, "componentType": 5126, "count": 3, "type": "VEC3"
        }));
        attributes["NORMAL"] = json!(1);
    }
    json!({
        "asset": { "version": "2.0" },
        "buffers": [{ "uri": data_uri(&[0u8; 96]), "byteLength": 96 }],
        "bufferViews": buffer_views,
        "accessors": accessors,
        "images": [{ "bufferView": 2, "mimeType": "image/png" }],
        "meshes": [{ "primitives": [{ "attributes": attributes }] }]
    })
}

#[test]
fn rejects_an_image_payload_overlapping_a_scale_bearing_accessor() {
    // An `image` reads its buffer view directly and never becomes an
    // accessor, so a check built from accessor ranges alone never compares
    // the two. Both ends of the overlap are located: the image that would be
    // corrupted, and the accessor whose rewrite would corrupt it.
    for (name, image_offset, image_length, position_offset) in [
        (
            "image runs one byte into the accessor",
            0usize,
            13usize,
            12usize,
        ),
        ("accessor runs one byte into the image", 35, 13, 0),
        ("image fully contains the accessor", 0, 48, 4),
    ] {
        let value = image_and_positions(image_offset, image_length, position_offset, None);
        let (violations, _) = unsupported(&value);
        assert_eq!(
            locations(
                &violations,
                GltfCapabilityViolationKind::ImagePayloadOverlap
            ),
            vec!["/images/0/bufferView"],
            "{name}"
        );
        assert_eq!(
            locations(
                &violations,
                GltfCapabilityViolationKind::OverlappingAccessorRanges
            ),
            vec!["/accessors/0"],
            "{name}"
        );
    }
}

#[test]
fn accepts_an_image_payload_adjacent_to_or_aliasing_only_dimensionless_bytes() {
    // Both ranges are half-open, so touching endpoints share no byte, and
    // refusing that would reject the tightly packed layout every exporter
    // emits. Both orders are pinned because the two sweeps that decide it are
    // independent. An image aliasing a `NORMAL` accessor is likewise accepted:
    // the conversion never writes those bytes.
    for (name, image_offset, image_length, position_offset, normal_offset) in [
        (
            "image ends where the accessor begins",
            0usize,
            12usize,
            12usize,
            None,
        ),
        ("image begins where the accessor ends", 36, 12, 0, None),
        ("image aliases NORMAL only", 48, 12, 0, Some(36usize)),
    ] {
        let value = image_and_positions(image_offset, image_length, position_offset, normal_offset);
        preflight_scale_source_bytes(Path::new("image-adjacent.gltf"), &bytes(&value))
            .unwrap_or_else(|error| panic!("{name}: must preflight cleanly, got {error:?}"));
    }
}

#[test]
fn an_empty_image_view_is_ranged_by_neither_the_gate_nor_the_rewriter() {
    // A `byteLength: 0` view covers no byte, so under the half-open
    // comparison every range here uses it aliases nothing — not even a
    // converted accessor it sits inside. The gate drops it before comparing
    // and `scale::reject_image_payload_overlap` skips the same shape; this
    // pins that they agree. Without the skip on the rewriter's side its
    // predicate degenerates for `start == end` into "the offset lies strictly
    // inside the span", and this source would be accepted here and refused
    // there. `byteLength: 0` is schema-invalid (glTF 2.0 gives
    // `bufferView.byteLength` `minimum: 1`), so this pins an unreachable case
    // — the point is that the two walkers must not drift, not that the shape
    // is expected.
    for (name, image_offset) in [
        ("empty view inside POSITION", 12usize),
        ("empty view at POSITION's start", 0),
        ("empty view at POSITION's end", 36),
    ] {
        let value = image_and_positions(image_offset, 0, 0, None);
        preflight_scale_source_bytes(Path::new("empty-image-view.gltf"), &bytes(&value))
            .unwrap_or_else(|error| panic!("{name}: an empty view aliases nothing: {error:?}"));
    }
}

#[test]
fn rejects_raw_extras_extensions_and_unknown_members() {
    let mut value = base_json();
    value["extras"] = json!({ "opaque": true });
    value["extensionsUsed"] = json!(["ACME_opaque"]);
    value["extensions"] = json!({ "ACME_opaque": { "preserve": "me" } });
    value["unmodeledTopLevel"] = json!(true);
    value["a/b~c"] = json!(true);
    value["nodes"] = json!([{ "extras": { "nested": true }, "unmodeledNode": 7 }]);
    value["materials"] = json!([{ "extras": { "nested": true } }]);
    let (violations, manifest) = unsupported(&value);
    assert_has(&violations, GltfCapabilityViolationKind::Extras);
    assert_has(
        &violations,
        GltfCapabilityViolationKind::ExtensionDeclaration,
    );
    assert_has(&violations, GltfCapabilityViolationKind::ExtensionPayload);
    assert_has(&violations, GltfCapabilityViolationKind::UnknownJsonMember);
    assert_eq!(manifest.extensions, vec!["ACME_opaque"]);
    assert_eq!(
        manifest.extras_locations,
        vec!["/extras", "/materials/0/extras", "/nodes/0/extras"]
    );
    assert_eq!(
        manifest.unknown_member_locations,
        vec!["/a~1b~0c", "/nodes/0/unmodeledNode", "/unmodeledTopLevel"]
    );
    assert_eq!(
        violations
            .iter()
            .filter(|violation| violation.kind == GltfCapabilityViolationKind::Extras)
            .map(|violation| violation.location.as_str())
            .collect::<Vec<_>>(),
        vec!["/extras", "/materials/0/extras", "/nodes/0/extras"]
    );

    let mut required_conflict = base_json();
    required_conflict["extensionsRequired"] = json!(["ACME_required"]);
    required_conflict["buffers"][0] = json!({
        "uri": format!("data:application/octet-stream;base64,{ZERO_F32X12}"),
        "byteLength": 48
    });
    required_conflict["bufferViews"][0] =
        json!({ "buffer": 0, "byteLength": 48, "byteStride": 16 });
    required_conflict["meshes"][0]["primitives"][0]["attributes"] =
        json!({ "POSITION": 0, "NORMAL": 0 });
    let (violations, _) = unsupported(&required_conflict);
    assert_has(
        &violations,
        GltfCapabilityViolationKind::ExtensionDeclaration,
    );
    assert_has(
        &violations,
        GltfCapabilityViolationKind::ConflictingAccessorUse,
    );
    assert_has(
        &violations,
        GltfCapabilityViolationKind::UnsafeAccessorLayout,
    );
}

#[test]
fn inventories_and_rejects_gpu_instancing_with_stable_accessor_identities() {
    let mut value = base_json();
    value["extensionsUsed"] = json!(["EXT_mesh_gpu_instancing"]);
    value["nodes"] = json!([{
        "mesh": 0,
        "extensions": {
            "EXT_mesh_gpu_instancing": {
                "attributes": { "TRANSLATION": 0, "SCALE": 0 }
            }
        }
    }]);

    let (violations, manifest) = unsupported(&value);

    assert_eq!(
        manifest.instancing[0]
            .attributes
            .iter()
            .map(|attribute| (attribute.semantic.as_str(), attribute.accessor_index))
            .collect::<Vec<_>>(),
        vec![("SCALE", 0), ("TRANSLATION", 0)]
    );
    assert_eq!(manifest.instancing[0].node_index, 0);
    assert_eq!(
        violations
            .iter()
            .filter(|violation| violation.kind == GltfCapabilityViolationKind::Instancing)
            .map(|violation| violation.location.as_str())
            .collect::<Vec<_>>(),
        vec![
            "/extensionsUsed/0",
            "/nodes/0/extensions/EXT_mesh_gpu_instancing"
        ]
    );
}

#[test]
fn rejects_external_resources_and_punctual_lights_before_resolution() {
    let mut value = base_json();
    value["buffers"][0]["uri"] = json!("missing.bin");
    value["images"] = json!([{ "uri": "missing.png" }]);
    value["extensionsUsed"] = json!(["KHR_lights_punctual"]);
    value["extensions"] = json!({
        "KHR_lights_punctual": { "lights": [{ "type": "point", "range": 4.0 }] }
    });
    let (violations, manifest) = unsupported(&value);
    assert_has(&violations, GltfCapabilityViolationKind::ExternalResource);
    assert_has(&violations, GltfCapabilityViolationKind::Light);
    assert_eq!(
        manifest.external_resource_locations,
        vec!["/buffers/0/uri", "/images/0/uri"]
    );
    assert_eq!(
        manifest.extension_locations,
        vec!["/extensions/KHR_lights_punctual"]
    );
    assert_eq!(
        violations
            .iter()
            .filter(|violation| violation.kind == GltfCapabilityViolationKind::Light)
            .map(|violation| violation.location.as_str())
            .collect::<Vec<_>>(),
        vec!["/extensions/KHR_lights_punctual", "/extensionsUsed/0"]
    );

    let mut declaration_only = base_json();
    declaration_only["extensionsRequired"] = json!(["KHR_lights_punctual"]);
    let (violations, _) = unsupported(&declaration_only);
    assert_eq!(
        violations
            .iter()
            .filter(|violation| violation.kind == GltfCapabilityViolationKind::Light)
            .map(|violation| violation.location.as_str())
            .collect::<Vec<_>>(),
        vec!["/extensionsRequired/0"]
    );

    let mut payload_only = base_json();
    payload_only["extensions"] = json!({ "KHR_lights_punctual": { "lights": [] } });
    let (violations, _) = unsupported(&payload_only);
    assert_eq!(
        violations
            .iter()
            .filter(|violation| violation.kind == GltfCapabilityViolationKind::Light)
            .map(|violation| violation.location.as_str())
            .collect::<Vec<_>>(),
        vec!["/extensions/KHR_lights_punctual"]
    );

    let mut external_image = base_json();
    external_image["images"] = json!([{ "uri": "missing.png" }]);
    external_image["buffers"][0] = json!({
        "uri": format!("data:application/octet-stream;base64,{ZERO_F32X12}"),
        "byteLength": 48
    });
    external_image["bufferViews"][0] = json!({ "buffer": 0, "byteLength": 48, "byteStride": 16 });
    let (violations, _) = unsupported(&external_image);
    assert_has(&violations, GltfCapabilityViolationKind::ExternalResource);
    assert_has(
        &violations,
        GltfCapabilityViolationKind::UnsafeAccessorLayout,
    );
}

#[test]
fn rejects_unsupported_morph_semantics_cameras_modes_attributes_and_secondary_influences() {
    let mut value = base_json();
    value["meshes"] = json!([
        {
            "weights": [0.5],
            "primitives": [{
                "mode": 1,
                "attributes": { "POSITION": 0, "COLOR_0": 0, "JOINTS_1": 0, "WEIGHTS_1": 0 },
                "targets": [{ "POSITION": 0 }, { "NORMAL": 0 }]
            }]
        },
        {
            "primitives": [{
                "mode": 0,
                "attributes": { "POSITION": 0, "TANGENT": 0, "JOINTS_2": 0, "WEIGHTS_2": 0 }
            }]
        }
    ]);
    value["nodes"] = json!([{ "mesh": 0, "weights": [0.25], "camera": 0 }]);
    value["cameras"] =
        json!([{ "type": "perspective", "perspective": { "yfov": 1.0, "znear": 0.1 } }]);
    let (violations, manifest) = unsupported(&value);
    for kind in [
        GltfCapabilityViolationKind::MorphTarget,
        GltfCapabilityViolationKind::Camera,
        GltfCapabilityViolationKind::NonTrianglePrimitive,
        GltfCapabilityViolationKind::UnsupportedVertexAttribute,
        GltfCapabilityViolationKind::SecondarySkinInfluences,
    ] {
        assert_has(&violations, kind);
    }
    assert_eq!(manifest.primitives[0].mode, 1);
    assert_eq!(manifest.primitives[0].morph_target_count, 2);
    assert_eq!(manifest.primitives[0].morph_position_accessors, vec![0]);
    assert_eq!(
        manifest.primitives[0].unsupported_morph_locations,
        ["/meshes/0/primitives/0/targets/1/NORMAL"]
    );
    assert_eq!(
        manifest.morph_weight_locations,
        ["/meshes/0/weights", "/nodes/0/weights"]
    );
    assert_eq!(manifest.primitives[1].mesh_index, 1);
    assert_eq!(manifest.primitives[1].primitive_index, 0);
    assert_eq!(manifest.primitives[1].mode, 0);
    assert_eq!(manifest.camera_count, 1);
    assert_eq!(
        violations
            .iter()
            .filter(|violation| violation.kind == GltfCapabilityViolationKind::Camera)
            .map(|violation| violation.location.as_str())
            .collect::<Vec<_>>(),
        vec!["/cameras", "/nodes/0/camera"]
    );
    assert_eq!(
        violations
            .iter()
            .filter(|violation| {
                violation.kind == GltfCapabilityViolationKind::NonTrianglePrimitive
            })
            .map(|violation| violation.location.as_str())
            .collect::<Vec<_>>(),
        vec!["/meshes/0/primitives/0/mode", "/meshes/1/primitives/0/mode"]
    );
    assert_eq!(
        violations
            .iter()
            .filter(|violation| {
                violation.kind == GltfCapabilityViolationKind::SecondarySkinInfluences
            })
            .map(|violation| violation.location.as_str())
            .collect::<Vec<_>>(),
        vec![
            "/meshes/0/primitives/0/attributes/JOINTS_1",
            "/meshes/0/primitives/0/attributes/WEIGHTS_1",
            "/meshes/1/primitives/0/attributes/JOINTS_2",
            "/meshes/1/primitives/0/attributes/WEIGHTS_2"
        ]
    );
}

#[test]
fn rejects_missing_empty_and_count_mismatched_inverse_binds() {
    let mut missing = base_json();
    missing["meshes"] = json!([]);
    missing["nodes"] = json!([{}, {}]);
    missing["skins"] = json!([{ "joints": [0, 1] }]);
    let (violations, manifest) = unsupported(&missing);
    assert_has(
        &violations,
        GltfCapabilityViolationKind::MissingInverseBinds,
    );
    assert_eq!(manifest.skins[0].skin_index, 0);
    assert_eq!(manifest.skins[0].joint_count, 2);
    assert_eq!(manifest.skins[0].inverse_bind_accessor_index, None);

    let mut empty = base_json();
    empty["meshes"] = json!([]);
    empty["nodes"] = json!([{}]);
    empty["bufferViews"] = json!([{ "buffer": 0, "byteLength": 0 }]);
    empty["accessors"] =
        json!([{ "bufferView": 0, "componentType": 5126, "count": 0, "type": "MAT4" }]);
    empty["skins"] = json!([{ "joints": [0], "inverseBindMatrices": 0 }]);
    let (violations, manifest) = unsupported(&empty);
    assert_has(
        &violations,
        GltfCapabilityViolationKind::EmptyInverseBindAccessor,
    );
    assert_eq!(manifest.skins[0].inverse_bind_count, Some(0));

    let mut mismatch = base_json();
    mismatch["meshes"] = json!([]);
    mismatch["buffers"][0]["uri"] = json!(format!(
        "data:application/octet-stream;base64,{ZERO_F32X16}"
    ));
    mismatch["buffers"][0]["byteLength"] = json!(64);
    mismatch["bufferViews"][0]["byteLength"] = json!(64);
    mismatch["nodes"] = json!([{}, {}]);
    mismatch["accessors"] =
        json!([{ "bufferView": 0, "componentType": 5126, "count": 1, "type": "MAT4" }]);
    mismatch["skins"] = json!([{ "joints": [0, 1], "inverseBindMatrices": 0 }]);
    let (violations, manifest) = unsupported(&mismatch);
    assert_has(
        &violations,
        GltfCapabilityViolationKind::InverseBindCountMismatch,
    );
    assert_eq!(manifest.skins[0].inverse_bind_accessor_index, Some(0));
    assert_eq!(manifest.skins[0].inverse_bind_count, Some(1));

    let mut oversized = base_json();
    oversized["meshes"] = json!([]);
    oversized["buffers"][0] = json!({ "uri": data_uri(&[0; 128]), "byteLength": 128 });
    oversized["bufferViews"][0]["byteLength"] = json!(128);
    oversized["nodes"] = json!([{}]);
    oversized["accessors"] = json!([
        { "bufferView": 0, "componentType": 5126, "count": 2, "type": "MAT4" }
    ]);
    oversized["skins"] = json!([{ "joints": [0], "inverseBindMatrices": 0 }]);
    let (violations, manifest) = unsupported(&oversized);
    assert_has(
        &violations,
        GltfCapabilityViolationKind::InverseBindCountMismatch,
    );
    assert_eq!(manifest.skins[0].inverse_bind_count, Some(2));
}

#[test]
fn rejects_unsafe_shared_and_overlapping_scale_accessor_layouts() {
    let mut unsafe_layout = base_json();
    unsafe_layout["buffers"][0]["uri"] = json!(format!(
        "data:application/octet-stream;base64,{ZERO_F32X12}"
    ));
    unsafe_layout["buffers"][0]["byteLength"] = json!(48);
    unsafe_layout["bufferViews"][0] = json!({ "buffer": 0, "byteLength": 48, "byteStride": 16 });
    let (violations, _) = unsupported(&unsafe_layout);
    assert_has(
        &violations,
        GltfCapabilityViolationKind::UnsafeAccessorLayout,
    );

    let mut shared = base_json();
    shared["meshes"][0]["primitives"][0]["attributes"] = json!({ "POSITION": 0, "NORMAL": 0 });
    let (violations, _) = unsupported(&shared);
    assert_has(
        &violations,
        GltfCapabilityViolationKind::ConflictingAccessorUse,
    );

    let mut overlapping = base_json();
    overlapping["accessors"] = json!([
        { "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3", "min": [0, 0, 0], "max": [0, 0, 0] },
        { "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3", "min": [0, 0, 0], "max": [0, 0, 0] }
    ]);
    overlapping["meshes"] = json!([
        { "primitives": [{ "attributes": { "POSITION": 0 } }] },
        { "primitives": [{ "attributes": { "POSITION": 1 } }] }
    ]);
    let (violations, _) = unsupported(&overlapping);
    assert_has(
        &violations,
        GltfCapabilityViolationKind::OverlappingAccessorRanges,
    );

    let mut cross_domain_overlap = base_json();
    cross_domain_overlap["accessors"] = json!([
        { "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3", "min": [0, 0, 0], "max": [0, 0, 0] },
        { "bufferView": 0, "componentType": 5123, "count": 3, "type": "SCALAR" }
    ]);
    cross_domain_overlap["meshes"][0]["primitives"][0]["indices"] = json!(1);
    let (violations, _) = unsupported(&cross_domain_overlap);
    assert_has(
        &violations,
        GltfCapabilityViolationKind::OverlappingAccessorRanges,
    );

    let mut masked_overlap = base_json();
    masked_overlap["buffers"][0] = json!({ "uri": data_uri(&[0; 1204]), "byteLength": 1204 });
    masked_overlap["bufferViews"] = json!([
        { "buffer": 0, "byteOffset": 0, "byteLength": 900 },
        { "buffer": 0, "byteOffset": 4, "byteLength": 1200 },
        { "buffer": 0, "byteOffset": 800, "byteLength": 24 }
    ]);
    masked_overlap["accessors"] = json!([
        { "bufferView": 0, "componentType": 5126, "count": 75, "type": "VEC3", "min": [0, 0, 0], "max": [0, 0, 0] },
        { "bufferView": 1, "componentType": 5126, "count": 100, "type": "VEC3" },
        { "bufferView": 2, "componentType": 5126, "count": 3, "type": "VEC2" }
    ]);
    masked_overlap["meshes"][0]["primitives"][0]["attributes"] =
        json!({ "POSITION": 0, "NORMAL": 1, "TEXCOORD_0": 2 });
    let (violations, _) = unsupported(&masked_overlap);
    assert_eq!(
        violations
            .iter()
            .filter(|violation| {
                violation.kind == GltfCapabilityViolationKind::OverlappingAccessorRanges
            })
            .map(|violation| violation.location.as_str())
            .collect::<Vec<_>>(),
        vec!["/accessors/0", "/accessors/1", "/accessors/2"]
    );
}

/// One converted `POSITION` accessor plus one accessor no source object
/// references. The unreferenced accessor may be dense or sparse, but its raw
/// payload remains source data that the byte-surgical scale writer owes.
fn positions_and_unreferenced_accessor(accessor: Value, buffer_views: Vec<Value>) -> Value {
    json!({
        "asset": { "version": "2.0" },
        "buffers": [{ "uri": data_uri(&[0u8; 128]), "byteLength": 128 }],
        "bufferViews": buffer_views,
        "accessors": [
            {
                "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3",
                "min": [0, 0, 0], "max": [0, 0, 0]
            },
            accessor
        ],
        "meshes": [{ "primitives": [{ "attributes": { "POSITION": 0 } }] }]
    })
}

#[test]
fn rejects_an_unreferenced_dense_accessor_overlapping_a_rewritten_accessor() {
    let value = positions_and_unreferenced_accessor(
        json!({ "bufferView": 1, "componentType": 5121, "count": 1, "type": "SCALAR" }),
        vec![
            json!({ "buffer": 0, "byteOffset": 0, "byteLength": 36 }),
            json!({ "buffer": 0, "byteOffset": 35, "byteLength": 1 }),
        ],
    );
    let (violations, _) = unsupported(&value);
    assert_eq!(
        locations(
            &violations,
            GltfCapabilityViolationKind::OverlappingAccessorRanges
        ),
        vec!["/accessors/0", "/accessors/1"]
    );
}

#[test]
fn rejects_unreferenced_sparse_payloads_overlapping_a_rewritten_accessor() {
    for (name, indices_offset, values_offset, sparse_location) in [
        (
            "sparse index enters POSITION",
            35usize,
            40usize,
            "/accessors/1/sparse/indices/bufferView",
        ),
        (
            "sparse values enter POSITION",
            36,
            24,
            "/accessors/1/sparse/values/bufferView",
        ),
    ] {
        let value = positions_and_unreferenced_accessor(
            json!({
                "componentType": 5126,
                "count": 3,
                "type": "VEC3",
                "sparse": {
                    "count": 1,
                    "indices": { "bufferView": 1, "componentType": 5121 },
                    "values": { "bufferView": 2 }
                }
            }),
            vec![
                json!({ "buffer": 0, "byteOffset": 0, "byteLength": 36 }),
                json!({ "buffer": 0, "byteOffset": indices_offset, "byteLength": 1 }),
                json!({ "buffer": 0, "byteOffset": values_offset, "byteLength": 12 }),
            ],
        );
        let (violations, _) = unsupported(&value);
        assert_eq!(
            locations(
                &violations,
                GltfCapabilityViolationKind::OverlappingAccessorRanges
            ),
            vec!["/accessors/0", sparse_location],
            "{name}"
        );
    }
}

#[test]
fn rejects_an_unreferenced_sparse_accessor_dense_base_overlapping_a_rewrite() {
    let value = positions_and_unreferenced_accessor(
        json!({
            "bufferView": 1,
            "componentType": 5126,
            "count": 1,
            "type": "VEC3",
            "sparse": {
                "count": 1,
                "indices": { "bufferView": 2, "componentType": 5121 },
                "values": { "bufferView": 3 }
            }
        }),
        vec![
            json!({ "buffer": 0, "byteOffset": 0, "byteLength": 36 }),
            json!({ "buffer": 0, "byteOffset": 24, "byteLength": 12 }),
            json!({ "buffer": 0, "byteOffset": 36, "byteLength": 1 }),
            json!({ "buffer": 0, "byteOffset": 40, "byteLength": 12 }),
        ],
    );
    let (violations, _) = unsupported(&value);
    assert_eq!(
        locations(
            &violations,
            GltfCapabilityViolationKind::OverlappingAccessorRanges
        ),
        vec!["/accessors/0", "/accessors/1"]
    );
}

#[test]
fn accepts_harmless_unreferenced_dense_and_sparse_accessors() {
    let dense = positions_and_unreferenced_accessor(
        json!({ "bufferView": 1, "componentType": 5121, "count": 4, "type": "SCALAR" }),
        vec![
            json!({ "buffer": 0, "byteOffset": 0, "byteLength": 36 }),
            json!({ "buffer": 0, "byteOffset": 36, "byteLength": 4 }),
        ],
    );
    preflight_scale_source_bytes(Path::new("unreferenced-dense.gltf"), &bytes(&dense))
        .expect("a disjoint unreferenced dense accessor remains supported");

    let sparse = positions_and_unreferenced_accessor(
        json!({
            "componentType": 5126,
            "count": 3,
            "type": "VEC3",
            "sparse": {
                "count": 1,
                "indices": { "bufferView": 1, "componentType": 5121 },
                "values": { "bufferView": 2 }
            }
        }),
        vec![
            json!({ "buffer": 0, "byteOffset": 0, "byteLength": 36 }),
            json!({ "buffer": 0, "byteOffset": 36, "byteLength": 1 }),
            json!({ "buffer": 0, "byteOffset": 40, "byteLength": 12 }),
        ],
    );
    preflight_scale_source_bytes(Path::new("unreferenced-sparse.gltf"), &bytes(&sparse))
        .expect("disjoint unreferenced sparse payloads remain supported");
}

#[test]
fn accepts_a_compact_unreferenced_dense_integer_matrix() {
    let value = positions_and_unreferenced_accessor(
        json!({ "bufferView": 1, "componentType": 5121, "count": 1, "type": "MAT2" }),
        vec![
            json!({ "buffer": 0, "byteOffset": 0, "byteLength": 36 }),
            // The columns begin at offsets 0 and 4. The last component ends
            // at byte 6, and glTF permits the final two padding bytes to be
            // omitted when no further element follows.
            json!({ "buffer": 0, "byteOffset": 36, "byteLength": 6 }),
        ],
    );

    preflight_scale_source_bytes(Path::new("compact-dense-mat2-u8.gltf"), &bytes(&value))
        .expect("a compact final integer-matrix column remains supported");
}

#[test]
fn accepts_compact_unreferenced_sparse_integer_matrix_values() {
    let value = positions_and_unreferenced_accessor(
        json!({
            "componentType": 5121,
            "count": 1,
            "type": "MAT2",
            "sparse": {
                "count": 1,
                "indices": { "bufferView": 1, "componentType": 5121 },
                "values": { "bufferView": 2 }
            }
        }),
        vec![
            json!({ "buffer": 0, "byteOffset": 0, "byteLength": 36 }),
            json!({ "buffer": 0, "byteOffset": 36, "byteLength": 1 }),
            json!({ "buffer": 0, "byteOffset": 40, "byteLength": 6 }),
        ],
    );

    preflight_scale_source_bytes(Path::new("compact-sparse-mat2-u8.gltf"), &bytes(&value))
        .expect("compact sparse integer-matrix values remain supported");
}

#[test]
fn keeps_integer_matrix_stride_between_elements() {
    let matrix = |byte_length| {
        positions_and_unreferenced_accessor(
            json!({ "bufferView": 1, "componentType": 5121, "count": 2, "type": "MAT2" }),
            vec![
                json!({ "buffer": 0, "byteOffset": 0, "byteLength": 36 }),
                json!({ "buffer": 0, "byteOffset": 36, "byteLength": byte_length }),
            ],
        )
    };

    preflight_scale_source_bytes(Path::new("two-compact-mat2-u8.gltf"), &bytes(&matrix(14)))
        .expect("two MAT2 U8 elements use an eight-byte stride and a six-byte final extent");

    let (violations, _) = unsupported(&matrix(13));
    assert_eq!(
        locations(
            &violations,
            GltfCapabilityViolationKind::UnsafeAccessorLayout
        ),
        vec!["/accessors/1"],
        "the second element still begins at byte 8; compact trailing storage must not collapse its stride"
    );
}

#[test]
fn rejects_unreadable_inverse_binds_while_inventorying_animated_morph_weights() {
    let mut value = base_json();
    value["meshes"] = json!([]);
    value["nodes"] = json!([{}]);
    value["accessors"] = json!([
        { "bufferView": 0, "componentType": 5126, "count": 1, "type": "VEC4" },
        { "bufferView": 0, "componentType": 5126, "count": 1, "type": "SCALAR" }
    ]);
    value["skins"] = json!([{ "joints": [0], "inverseBindMatrices": 0 }]);
    value["animations"] = json!([{
        "samplers": [{ "input": 1, "interpolation": "CUBICSPLINE", "output": 1 }],
        "channels": [{ "sampler": 0, "target": { "node": 0, "path": "weights" } }]
    }]);
    let (violations, manifest) = unsupported(&value);
    assert_has(
        &violations,
        GltfCapabilityViolationKind::UnreadableInverseBinds,
    );
    assert!(
        !violations
            .iter()
            .any(|violation| violation.kind == GltfCapabilityViolationKind::MorphWeights)
    );
    assert_eq!(
        manifest.morph_weight_locations,
        ["/animations/0/channels/0/target/path"]
    );
    assert_eq!(manifest.animation_channels[0].animation_index, 0);
    assert_eq!(manifest.animation_channels[0].channel_index, 0);
    assert_eq!(manifest.animation_channels[0].target_node_index, 0);
    assert_eq!(manifest.animation_channels[0].target_path, "weights");
    assert_eq!(manifest.animation_channels[0].interpolation, "CUBICSPLINE");
    assert_eq!(manifest.animation_channels[0].input_accessor_index, 1);
    assert_eq!(manifest.animation_channels[0].output_accessor_index, 1);
    assert_eq!(manifest.accessors[0].accessor_type, "VEC4");
}

#[test]
fn multiple_violations_are_typed_sorted_and_repeatable() {
    let mut value = base_json();
    value["extras"] = json!({ "unhandled": true });
    value["meshes"][0]["primitives"][0]["mode"] = json!(5);
    value["nodes"] = json!([{ "camera": 0 }]);
    value["cameras"] = json!([{ "type": "orthographic", "orthographic": { "xmag": 1, "ymag": 1, "zfar": 1, "znear": 0 } }]);
    let (first, first_manifest) = unsupported(&value);
    let (second, second_manifest) = unsupported(&value);
    assert_eq!(first, second);
    assert_eq!(first_manifest, second_manifest);
    assert!(
        first.windows(2).all(|pair| pair[0] <= pair[1]),
        "violations are stable: {first:#?}"
    );
    for kind in [
        GltfCapabilityViolationKind::Camera,
        GltfCapabilityViolationKind::Extras,
        GltfCapabilityViolationKind::NonTrianglePrimitive,
    ] {
        assert_has(&first, kind);
    }
}

#[test]
fn reordered_source_arrays_retain_deterministic_source_order_and_identity() {
    let mut value = base_json();
    value["nodes"] = json!([
        { "translation": [1, 0, 0], "extras": { "source": "first" } },
        { "matrix": [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1], "extras": { "source": "second" } }
    ]);
    value["nodes"].as_array_mut().unwrap().reverse();

    let (first_violations, first_manifest) = unsupported(&value);
    let (second_violations, second_manifest) = unsupported(&value);

    assert_eq!(first_violations, second_violations);
    assert_eq!(first_manifest, second_manifest);
    assert_eq!(
        first_manifest
            .nodes
            .iter()
            .map(|node| (node.node_index, node.rest_kind))
            .collect::<Vec<_>>(),
        vec![(0, GltfNodeRestKind::Matrix), (1, GltfNodeRestKind::Trs)]
    );
    assert_eq!(
        first_manifest.extras_locations,
        vec!["/nodes/0/extras", "/nodes/1/extras"]
    );
}

#[test]
fn malformed_topology_wins_over_unsupported_domains_before_inventory_returns() {
    let mut value = base_json();
    value["extras"] = json!({ "unsupported": true });
    value["extensionsRequired"] = json!(["ACME_required"]);
    value["nodes"] = json!([
        { "children": [2] },
        { "children": [2] },
        {}
    ]);

    let error = preflight_scale_source_bytes(Path::new("malformed.gltf"), &bytes(&value))
        .expect_err("duplicate-parent topology is malformed");

    assert!(matches!(error, GltfScalePreflightError::Load(_)));
}

#[test]
fn hostile_glb_length_is_a_load_error_without_allocating_declared_size() {
    let mut glb_bytes = Vec::new();
    glb_bytes.extend_from_slice(b"glTF");
    glb_bytes.extend_from_slice(&2u32.to_le_bytes());
    glb_bytes.extend_from_slice(&u32::MAX.to_le_bytes());
    let error = preflight_scale_source_bytes(Path::new("hostile.glb"), &glb_bytes)
        .expect_err("spoofed GLB length is rejected");
    assert!(matches!(error, GltfScalePreflightError::Load(_)));

    let mut hostile_accessor = base_json();
    hostile_accessor["accessors"][0]["count"] = json!(u64::MAX);
    let result = preflight_scale_source_bytes(
        Path::new("hostile-accessor.gltf"),
        &bytes(&hostile_accessor),
    );
    assert!(result.is_err(), "hostile accessor count must fail closed");
}
