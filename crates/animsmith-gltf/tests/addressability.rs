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
        overflow.scene_coverage(),
        RawGltfAddressabilityCoverageV1::budget_exceeded()
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
