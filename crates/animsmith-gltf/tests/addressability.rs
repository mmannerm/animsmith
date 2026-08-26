//! Analytic coverage for immutable same-load raw glTF addressability evidence.

use animsmith_core::{
    InputIdentity, RAW_GLTF_ADDRESSABILITY_V1_MAX_PATH_BYTES,
    RAW_GLTF_ADDRESSABILITY_V1_MAX_PATH_SEGMENTS, RawGltfAddressabilityCoverageV1,
    RawGltfDefaultSceneObservationV1, RawGltfInverseBindMatricesObservationV1,
};
use base64::Engine as _;
use serde_json::{Value, json};
use std::path::Path;

fn load(value: Value) -> animsmith_core::LoadedSource {
    let bytes = serde_json::to_vec(&value).expect("serialize analytic glTF");
    animsmith_gltf::load_source_bytes(Path::new("addressability.gltf"), &bytes)
        .expect("analytic glTF loads")
}

fn chain(names: impl IntoIterator<Item = String>) -> Value {
    let names = names.into_iter().collect::<Vec<_>>();
    let node_count = names.len();
    let nodes = names
        .into_iter()
        .enumerate()
        .map(|(index, name)| {
            let mut node = json!({ "name": name });
            if index + 1 < node_count {
                node["children"] = json!([index + 1]);
            }
            node
        })
        .collect::<Vec<_>>();
    json!({
        "asset": { "version": "2.0" },
        "nodes": nodes,
        "scenes": [{ "nodes": [0] }],
        "scene": 0
    })
}

#[test]
fn zero_scenes_and_absent_default_scene_are_complete_observations() {
    let source = load(json!({ "asset": { "version": "2.0" } }));
    let inventory = source
        .raw_gltf_addressability_inventory()
        .expect("glTF loader attaches inventory");

    assert_eq!(
        inventory.primary_input(),
        source.source_facts().primary_identity()
    );
    assert_eq!(inventory.dependency_closure(), source.dependency_closure());
    assert_eq!(
        inventory.scene_coverage(),
        RawGltfAddressabilityCoverageV1::Complete
    );
    assert!(inventory.scenes().is_empty());
    assert_eq!(
        inventory.default_scene(),
        RawGltfDefaultSceneObservationV1::Absent
    );
    assert!(inventory.path_candidates().is_empty());
}

#[test]
fn multiple_scenes_shared_roots_hierarchy_skins_and_paths_preserve_source_order() {
    let inverse_bind_bytes = base64::engine::general_purpose::STANDARD.encode([0u8; 64]);
    let source = load(json!({
        "asset": { "version": "2.0" },
        "buffers": [{
            "byteLength": 64,
            "uri": format!("data:application/octet-stream;base64,{inverse_bind_bytes}")
        }],
        "bufferViews": [{ "buffer": 0, "byteOffset": 0, "byteLength": 64 }],
        "accessors": [{
            "bufferView": 0,
            "componentType": 5126,
            "count": 1,
            "type": "MAT4"
        }],
        "nodes": [
            { "name": "shared-root", "children": [1, 2] },
            { "name": "duplicate", "skin": 0 },
            { "skin": 1 },
            { "name": "duplicate" }
        ],
        "skins": [
            { "name": "skin", "joints": [1], "skeleton": 0, "inverseBindMatrices": 0 },
            { "name": "skin", "joints": [2] },
            { "name": "unreferenced", "joints": [3] }
        ],
        "scenes": [
            { "name": "scene", "nodes": [0] },
            { "name": "scene", "nodes": [0] }
        ],
        "scene": 1
    }));
    let inventory = source.raw_gltf_addressability_inventory().unwrap();

    assert_eq!(inventory.default_scene().selected_scene_index(), Some(1));
    assert_eq!(inventory.scenes().len(), 2);
    assert_eq!(inventory.scenes()[0].name(), Some("scene"));
    assert_eq!(inventory.scenes()[0].root_node_indices(), &[0]);
    assert_eq!(inventory.scenes()[1].root_node_indices(), &[0]);

    assert_eq!(inventory.nodes().len(), 4);
    assert_eq!(inventory.nodes()[0].name(), Some("shared-root"));
    assert_eq!(inventory.nodes()[0].parent_node_index(), None);
    assert_eq!(inventory.nodes()[0].child_node_indices(), &[1, 2]);
    assert_eq!(inventory.nodes()[1].parent_node_index(), Some(0));
    assert_eq!(inventory.nodes()[2].name(), None);
    assert_eq!(inventory.nodes()[3].parent_node_index(), None);

    assert_eq!(inventory.skins().len(), 3);
    assert_eq!(inventory.skins()[0].joint_node_indices(), &[1]);
    assert_eq!(inventory.skins()[0].skeleton_root_node_index(), Some(0));
    assert_eq!(
        inventory.skins()[0].inverse_bind_matrices(),
        RawGltfInverseBindMatricesObservationV1::Declared {
            source_accessor_index: 0
        }
    );
    assert_eq!(inventory.skins()[1].skeleton_root_node_index(), None);
    assert_eq!(
        inventory.skins()[1].inverse_bind_matrices(),
        RawGltfInverseBindMatricesObservationV1::Absent
    );
    assert_eq!(inventory.skins()[2].name(), Some("unreferenced"));
    assert_eq!(
        inventory
            .attachments()
            .iter()
            .map(|row| (row.source_node_index(), row.source_skin_index()))
            .collect::<Vec<_>>(),
        vec![(1, 0), (2, 1)]
    );

    assert_eq!(inventory.path_candidates().len(), 6);
    assert_eq!(inventory.path_candidates()[0].source_scene_index(), 0);
    assert_eq!(inventory.path_candidates()[0].source_node_indices(), &[0]);
    assert_eq!(
        inventory.path_candidates()[1].source_node_indices(),
        &[0, 1]
    );
    assert_eq!(
        inventory.path_candidates()[2].source_node_indices(),
        &[0, 2]
    );
    assert_eq!(inventory.path_candidates()[3].source_scene_index(), 1);
    assert_eq!(inventory.path_candidates()[4].target_node_index(), Some(1));
    assert!(
        inventory
            .path_candidates()
            .iter()
            .all(|row| row.target_node_index() != Some(3)),
        "unreachable source node is not given a scene path"
    );
}

#[test]
fn exact_primary_bytes_and_closure_bind_identity_without_reopening_path() {
    let first = json!({
        "asset": { "version": "2.0" },
        "nodes": [{ "name": "first" }]
    });
    let second = json!({
        "asset": { "version": "2.0" },
        "nodes": [{ "name": "second" }]
    });
    let first_bytes = serde_json::to_vec(&first).unwrap();
    let second_bytes = serde_json::to_vec(&second).unwrap();
    let missing = Path::new("path-does-not-exist.gltf");
    let first = animsmith_gltf::load_source_bytes(missing, &first_bytes).unwrap();
    let second = animsmith_gltf::load_source_bytes(missing, &second_bytes).unwrap();
    let first_inventory = first.raw_gltf_addressability_inventory().unwrap();
    let second_inventory = second.raw_gltf_addressability_inventory().unwrap();
    assert_eq!(
        first_inventory.identity().sha256(),
        "73a8c4453b39851f83dc5c88aeb68cdd0e64bea17cd72e4617aee89065805e9e"
    );
    assert_eq!(first_inventory.identity().bytes(), 1_454);

    assert_eq!(
        first_inventory.primary_input(),
        &InputIdentity::from_bytes(&first_bytes)
    );
    assert_eq!(
        first_inventory.dependency_closure(),
        first.dependency_closure()
    );
    assert_eq!(first_inventory.nodes()[0].name(), Some("first"));
    assert_eq!(second_inventory.nodes()[0].name(), Some("second"));
    assert_ne!(first_inventory.identity(), second_inventory.identity());
}

#[test]
fn per_domain_row_bound_retains_exact_n_and_marks_n_plus_one_partial() {
    let exact_scenes = (0..animsmith_core::RAW_GLTF_ADDRESSABILITY_V1_MAX_ROWS_PER_DOMAIN)
        .map(|_| json!({ "nodes": [] }))
        .collect::<Vec<_>>();
    let exact = load(json!({
        "asset": { "version": "2.0" },
        "scenes": exact_scenes
    }));
    let exact = exact.raw_gltf_addressability_inventory().unwrap();
    assert_eq!(exact.scenes().len(), 4_096);
    assert_eq!(
        exact.scene_coverage(),
        RawGltfAddressabilityCoverageV1::Complete
    );

    let overflow_scenes = (0..=animsmith_core::RAW_GLTF_ADDRESSABILITY_V1_MAX_ROWS_PER_DOMAIN)
        .map(|_| json!({ "nodes": [] }))
        .collect::<Vec<_>>();
    let overflow = load(json!({
        "asset": { "version": "2.0" },
        "scenes": overflow_scenes
    }));
    let overflow = overflow.raw_gltf_addressability_inventory().unwrap();
    assert_eq!(overflow.scenes().len(), 4_096);
    assert_eq!(
        overflow.scenes(),
        exact.scenes(),
        "scene overflow retains the exact canonical source prefix"
    );
    assert_eq!(
        overflow.scene_coverage(),
        RawGltfAddressabilityCoverageV1::budget_exceeded()
    );
}

#[test]
fn node_skin_and_attachment_row_bounds_retain_independent_canonical_prefixes() {
    let row_limit = animsmith_core::RAW_GLTF_ADDRESSABILITY_V1_MAX_ROWS_PER_DOMAIN;

    let exact_nodes = load(json!({
        "asset": { "version": "2.0" },
        "nodes": (0..row_limit).map(|_| json!({})).collect::<Vec<_>>()
    }));
    let exact_nodes = exact_nodes.raw_gltf_addressability_inventory().unwrap();
    assert_eq!(exact_nodes.nodes().len(), row_limit);
    assert_eq!(
        exact_nodes.node_coverage(),
        RawGltfAddressabilityCoverageV1::Complete
    );
    let overflow_nodes = load(json!({
        "asset": { "version": "2.0" },
        "nodes": (0..=row_limit).map(|_| json!({})).collect::<Vec<_>>()
    }));
    let overflow_nodes = overflow_nodes.raw_gltf_addressability_inventory().unwrap();
    assert_eq!(overflow_nodes.nodes(), exact_nodes.nodes());
    assert_eq!(
        overflow_nodes.node_coverage(),
        RawGltfAddressabilityCoverageV1::budget_exceeded()
    );
    assert_eq!(
        overflow_nodes.skin_coverage(),
        RawGltfAddressabilityCoverageV1::Complete,
        "node overflow does not contaminate independent skin coverage"
    );

    let skins = |count| {
        (0..count)
            .map(|_| json!({ "joints": [0] }))
            .collect::<Vec<_>>()
    };
    let exact_skins = load(json!({
        "asset": { "version": "2.0" },
        "nodes": [{}],
        "skins": skins(row_limit)
    }));
    let exact_skins = exact_skins.raw_gltf_addressability_inventory().unwrap();
    assert_eq!(exact_skins.skins().len(), row_limit);
    assert_eq!(
        exact_skins.skin_coverage(),
        RawGltfAddressabilityCoverageV1::Complete
    );
    let overflow_skins = load(json!({
        "asset": { "version": "2.0" },
        "nodes": [{}],
        "skins": skins(row_limit + 1)
    }));
    let overflow_skins = overflow_skins.raw_gltf_addressability_inventory().unwrap();
    assert_eq!(overflow_skins.skins(), exact_skins.skins());
    assert_eq!(
        overflow_skins.skin_coverage(),
        RawGltfAddressabilityCoverageV1::budget_exceeded()
    );
    assert_eq!(
        overflow_skins.node_coverage(),
        RawGltfAddressabilityCoverageV1::Complete,
        "skin overflow does not contaminate independent node coverage"
    );

    let attached_nodes = |count| (0..count).map(|_| json!({ "skin": 0 })).collect::<Vec<_>>();
    let exact_attachments = load(json!({
        "asset": { "version": "2.0" },
        "nodes": attached_nodes(row_limit),
        "skins": [{ "joints": [0] }]
    }));
    let exact_attachments = exact_attachments
        .raw_gltf_addressability_inventory()
        .unwrap();
    assert_eq!(exact_attachments.attachments().len(), row_limit);
    assert_eq!(
        exact_attachments.attachment_coverage(),
        RawGltfAddressabilityCoverageV1::Complete
    );
    let overflow_attachments = load(json!({
        "asset": { "version": "2.0" },
        "nodes": attached_nodes(row_limit + 1),
        "skins": [{ "joints": [0] }]
    }));
    let overflow_attachments = overflow_attachments
        .raw_gltf_addressability_inventory()
        .unwrap();
    assert_eq!(
        overflow_attachments.attachments(),
        exact_attachments.attachments()
    );
    assert_eq!(
        overflow_attachments.attachment_coverage(),
        RawGltfAddressabilityCoverageV1::budget_exceeded()
    );
    assert_eq!(
        overflow_attachments.skin_coverage(),
        RawGltfAddressabilityCoverageV1::Complete,
        "attachment overflow does not contaminate independent skin coverage"
    );
}

#[test]
fn path_candidate_row_bound_retains_the_exact_scene_dfs_prefix() {
    let row_limit = animsmith_core::RAW_GLTF_ADDRESSABILITY_V1_MAX_ROWS_PER_DOMAIN;
    let flat_scene = |count| {
        json!({
            "asset": { "version": "2.0" },
            "nodes": (0..count).map(|_| json!({})).collect::<Vec<_>>(),
            "scenes": [{ "nodes": (0..count).collect::<Vec<_>>() }]
        })
    };
    let exact = load(flat_scene(row_limit));
    let exact = exact.raw_gltf_addressability_inventory().unwrap();
    assert_eq!(exact.path_candidates().len(), row_limit);
    assert_eq!(
        exact.path_candidate_coverage(),
        RawGltfAddressabilityCoverageV1::Complete
    );

    let overflow = load(flat_scene(row_limit + 1));
    let overflow = overflow.raw_gltf_addressability_inventory().unwrap();
    assert_eq!(overflow.path_candidates(), exact.path_candidates());
    assert_eq!(
        overflow.path_candidate_coverage(),
        RawGltfAddressabilityCoverageV1::budget_exceeded()
    );
}

#[test]
fn per_name_bound_is_independent_for_scenes_nodes_and_skins() {
    let exact_name = "x".repeat(animsmith_core::RAW_GLTF_ADDRESSABILITY_V1_MAX_NAME_BYTES);
    let oversized_name = format!("{exact_name}x");
    let exact = load(json!({
        "asset": { "version": "2.0" },
        "nodes": [{ "name": exact_name }, { "name": "joint" }],
        "skins": [{ "name": "skin-prefix", "joints": [1] }, { "name": exact_name, "joints": [1] }],
        "scenes": [{ "name": "scene-prefix", "nodes": [] }, { "name": exact_name, "nodes": [] }]
    }));
    let exact = exact.raw_gltf_addressability_inventory().unwrap();
    assert_eq!(
        exact.scene_coverage(),
        RawGltfAddressabilityCoverageV1::Complete
    );
    assert_eq!(
        exact.node_coverage(),
        RawGltfAddressabilityCoverageV1::Complete
    );
    assert_eq!(
        exact.skin_coverage(),
        RawGltfAddressabilityCoverageV1::Complete
    );

    let scene_overflow = load(json!({
        "asset": { "version": "2.0" },
        "scenes": [{ "name": "scene-prefix", "nodes": [] }, { "name": oversized_name, "nodes": [] }]
    }));
    let scene_overflow = scene_overflow.raw_gltf_addressability_inventory().unwrap();
    assert_eq!(scene_overflow.scenes().len(), 1);
    assert_eq!(scene_overflow.scenes()[0].name(), Some("scene-prefix"));
    assert_eq!(
        scene_overflow.scene_coverage(),
        RawGltfAddressabilityCoverageV1::budget_exceeded()
    );
    assert_eq!(
        scene_overflow.node_coverage(),
        RawGltfAddressabilityCoverageV1::Complete
    );

    let node_overflow = load(json!({
        "asset": { "version": "2.0" },
        "nodes": [{ "name": "node-prefix" }, { "name": oversized_name }]
    }));
    let node_overflow = node_overflow.raw_gltf_addressability_inventory().unwrap();
    assert_eq!(node_overflow.nodes().len(), 1);
    assert_eq!(node_overflow.nodes()[0].name(), Some("node-prefix"));
    assert_eq!(
        node_overflow.node_coverage(),
        RawGltfAddressabilityCoverageV1::budget_exceeded()
    );
    assert_eq!(
        node_overflow.scene_coverage(),
        RawGltfAddressabilityCoverageV1::Complete
    );

    let skin_overflow = load(json!({
        "asset": { "version": "2.0" },
        "nodes": [{}],
        "skins": [
            { "name": "skin-prefix", "joints": [0] },
            { "name": oversized_name, "joints": [0] }
        ]
    }));
    let skin_overflow = skin_overflow.raw_gltf_addressability_inventory().unwrap();
    assert_eq!(skin_overflow.skins().len(), 1);
    assert_eq!(skin_overflow.skins()[0].name(), Some("skin-prefix"));
    assert_eq!(
        skin_overflow.skin_coverage(),
        RawGltfAddressabilityCoverageV1::budget_exceeded()
    );
    assert_eq!(
        skin_overflow.node_coverage(),
        RawGltfAddressabilityCoverageV1::Complete
    );
}

#[test]
fn aggregate_structural_reference_bound_is_atomic_and_prefix_preserving() {
    let reference_limit = animsmith_core::RAW_GLTF_ADDRESSABILITY_V1_MAX_STRUCTURAL_REFERENCES;
    let joints = |count| std::iter::repeat_n(0, count).collect::<Vec<_>>();
    let exact = load(json!({
        "asset": { "version": "2.0" },
        "nodes": [{}],
        "skins": [
            { "name": "prefix", "joints": joints(reference_limit / 2) },
            { "name": "tail", "joints": joints(reference_limit / 2) }
        ]
    }));
    let exact = exact.raw_gltf_addressability_inventory().unwrap();
    assert_eq!(exact.skins().len(), 2);
    assert_eq!(
        exact.skin_coverage(),
        RawGltfAddressabilityCoverageV1::Complete
    );

    let overflow = load(json!({
        "asset": { "version": "2.0" },
        "nodes": [{}],
        "skins": [
            { "name": "prefix", "joints": joints(reference_limit / 2) },
            { "name": "tail", "joints": joints(reference_limit / 2 + 1) }
        ]
    }));
    let overflow = overflow.raw_gltf_addressability_inventory().unwrap();
    assert_eq!(overflow.skins(), &exact.skins()[..1]);
    assert_eq!(
        overflow.skin_coverage(),
        RawGltfAddressabilityCoverageV1::budget_exceeded()
    );
    assert_eq!(
        overflow.node_coverage(),
        RawGltfAddressabilityCoverageV1::Complete,
        "the structural budget refusal is isolated to the consuming domain"
    );
}

#[test]
fn aggregate_text_bound_accepts_one_mib_and_rejects_the_next_byte_atomically() {
    let name_limit = animsmith_core::RAW_GLTF_ADDRESSABILITY_V1_MAX_NAME_BYTES;
    let text_limit = animsmith_core::RAW_GLTF_ADDRESSABILITY_V1_MAX_TEXT_BYTES;
    let exact_row_count = text_limit / name_limit;
    let name = "x".repeat(name_limit);
    let scenes = |extra: bool| {
        let mut rows = (0..exact_row_count)
            .map(|_| json!({ "name": name, "nodes": [] }))
            .collect::<Vec<_>>();
        if extra {
            rows.push(json!({ "name": "x", "nodes": [] }));
        }
        rows
    };
    let exact = load(json!({
        "asset": { "version": "2.0" },
        "scenes": scenes(false)
    }));
    let exact = exact.raw_gltf_addressability_inventory().unwrap();
    assert_eq!(exact.scenes().len(), exact_row_count);
    assert_eq!(
        exact.scene_coverage(),
        RawGltfAddressabilityCoverageV1::Complete
    );

    let overflow = load(json!({
        "asset": { "version": "2.0" },
        "scenes": scenes(true)
    }));
    let overflow = overflow.raw_gltf_addressability_inventory().unwrap();
    assert_eq!(overflow.scenes(), exact.scenes());
    assert_eq!(
        overflow.scene_coverage(),
        RawGltfAddressabilityCoverageV1::budget_exceeded()
    );
    assert_eq!(
        overflow.node_coverage(),
        RawGltfAddressabilityCoverageV1::Complete,
        "the aggregate text refusal is isolated to scene projection"
    );
}

#[test]
fn path_segment_ceiling_accepts_256_and_retains_the_canonical_prefix_at_257() {
    let exact = load(chain(
        (0..RAW_GLTF_ADDRESSABILITY_V1_MAX_PATH_SEGMENTS).map(|index| format!("n{index}")),
    ));
    let exact = exact.raw_gltf_addressability_inventory().unwrap();
    assert_eq!(
        exact.path_candidates().len(),
        RAW_GLTF_ADDRESSABILITY_V1_MAX_PATH_SEGMENTS
    );
    assert_eq!(
        exact.path_candidate_coverage(),
        RawGltfAddressabilityCoverageV1::Complete
    );
    assert_eq!(
        exact
            .path_candidates()
            .last()
            .unwrap()
            .source_node_indices(),
        &(0..RAW_GLTF_ADDRESSABILITY_V1_MAX_PATH_SEGMENTS as u64).collect::<Vec<_>>()
    );

    let overflow = load(chain(
        (0..=RAW_GLTF_ADDRESSABILITY_V1_MAX_PATH_SEGMENTS).map(|index| format!("n{index}")),
    ));
    let overflow = overflow.raw_gltf_addressability_inventory().unwrap();
    assert_eq!(
        overflow.path_candidates(),
        exact.path_candidates(),
        "overflow retains the exact deterministic DFS prefix"
    );
    assert_eq!(
        overflow.path_candidate_coverage(),
        RawGltfAddressabilityCoverageV1::budget_exceeded()
    );
}

#[test]
fn projected_path_byte_ceiling_accepts_4096_and_stops_before_4097() {
    let exact_names = vec![
        "a".repeat(1_023),
        "b".repeat(1_023),
        "c".repeat(1_023),
        "d".repeat(1_024),
    ];
    assert_eq!(
        exact_names.iter().map(String::len).sum::<usize>() + exact_names.len() - 1,
        RAW_GLTF_ADDRESSABILITY_V1_MAX_PATH_BYTES
    );
    let exact = load(chain(exact_names.clone()));
    let exact = exact.raw_gltf_addressability_inventory().unwrap();
    assert_eq!(exact.path_candidates().len(), exact_names.len());
    assert_eq!(
        exact.path_candidate_coverage(),
        RawGltfAddressabilityCoverageV1::Complete
    );

    let mut overflow_names = exact_names;
    overflow_names.push(String::new());
    let overflow = load(chain(overflow_names));
    let overflow = overflow.raw_gltf_addressability_inventory().unwrap();
    assert_eq!(
        overflow.path_candidates(),
        exact.path_candidates(),
        "the first over-byte path is excluded without disturbing its canonical prefix"
    );
    assert_eq!(
        overflow.path_candidate_coverage(),
        RawGltfAddressabilityCoverageV1::budget_exceeded()
    );
}
