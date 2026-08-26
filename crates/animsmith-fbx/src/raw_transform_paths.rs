//! Raw-preserving FBX transform hierarchy projection.

use animsmith_core::{
    InputIdentity, RawTransformPathInventoryV1, RawTransformPathNodeInputV1,
    RawTransformPathNodeKindV1, SourceFormatV1,
};
use std::collections::{BTreeMap, BTreeSet};

/// Parse the exact captured bytes without the production scene normalization.
///
/// A failure is evidence unavailability, not a reason to reject bytes which
/// the established normalized loader already accepted.
pub(crate) fn project(
    bytes: &[u8],
    filename: &str,
    normalized: &ufbx::Scene,
    primary_input: InputIdentity,
) -> RawTransformPathInventoryV1 {
    let raw = ufbx::load_memory(
        bytes,
        ufbx::LoadOpts {
            // Preserve the FBX transform hierarchy and names instead of the
            // production helper and compensation projection.
            space_conversion: ufbx::SpaceConversion::TransformRoot,
            geometry_transform_handling: ufbx::GeometryTransformHandling::Preserve,
            inherit_mode_handling: ufbx::InheritModeHandling::Preserve,
            filename: filename.into(),
            // Match the production loader's external-I/O boundary exactly.
            load_external_files: false,
            ignore_missing_external_files: false,
            ..Default::default()
        },
    );
    let Ok(raw) = raw else {
        return RawTransformPathInventoryV1::unavailable(
            primary_input,
            SourceFormatV1::Fbx,
            normalized.nodes.len() as u64,
        );
    };

    let normalized_originals = unique_original_node_map(normalized);
    let mut normalized_to_inventory = BTreeMap::new();
    for raw_node in &raw.nodes {
        if let Some(&normalized_index) = normalized_originals.get(&raw_node.element.element_id) {
            normalized_to_inventory.insert(normalized_index, raw_node.element.typed_id as u64);
        }
    }

    // ufbx adds normalization helpers after original typed nodes. Give them
    // stable appended inventory identities while keeping every original row's
    // raw typed_id and raw parent chain unchanged.
    let mut next_helper_index = raw.nodes.len() as u64;
    for node in normalized.nodes.iter().filter(|node| is_helper(node)) {
        normalized_to_inventory.insert(node.element.typed_id, next_helper_index);
        next_helper_index = next_helper_index.saturating_add(1);
    }

    let mut inputs = Vec::with_capacity(next_helper_index as usize);
    for node in &raw.nodes {
        let kind = node_kind(node);
        inputs.push(RawTransformPathNodeInputV1 {
            source_node_index: node.element.typed_id as u64,
            parent_source_node_index: node
                .parent
                .as_ref()
                .map(|parent| parent.element.typed_id as u64),
            source_name: (!node.is_root).then_some(node.element.name.as_ref()),
            projected_bone_index: (!node.is_root)
                .then(|| normalized_originals.get(&node.element.element_id).copied())
                .flatten()
                .map(u64::from),
            kind,
        });
    }
    for node in normalized.nodes.iter().filter(|node| is_helper(node)) {
        let source_node_index = normalized_to_inventory[&node.element.typed_id];
        let parent_source_node_index = node
            .parent
            .as_ref()
            .and_then(|parent| normalized_to_inventory.get(&parent.element.typed_id))
            .copied();
        inputs.push(RawTransformPathNodeInputV1 {
            source_node_index,
            parent_source_node_index,
            source_name: Some(node.element.name.as_ref()),
            projected_bone_index: Some(node.element.typed_id as u64),
            kind: node_kind(node),
        });
    }

    RawTransformPathInventoryV1::from_nodes(
        primary_input.clone(),
        SourceFormatV1::Fbx,
        normalized.nodes.len() as u64,
        inputs,
    )
    .unwrap_or_else(|_| {
        RawTransformPathInventoryV1::unavailable(
            primary_input,
            SourceFormatV1::Fbx,
            normalized.nodes.len() as u64,
        )
    })
}

/// Map raw-stable element identity to one normalized document bone index.
///
/// ufbx documents `element_id` as consistent across loads of the same file.
/// Duplicate element identities are removed rather than guessed.
fn unique_original_node_map(scene: &ufbx::Scene) -> BTreeMap<u32, u32> {
    let mut map = BTreeMap::new();
    let mut duplicates = BTreeSet::new();
    for node in scene.nodes.iter().filter(|node| !is_helper(node)) {
        let element_id = node.element.element_id;
        if map.insert(element_id, node.element.typed_id).is_some() {
            duplicates.insert(element_id);
        }
    }
    for duplicate in duplicates {
        map.remove(&duplicate);
    }
    map
}

fn is_helper(node: &ufbx::Node) -> bool {
    node.is_geometry_transform_helper || node.is_scale_helper
}

fn node_kind(node: &ufbx::Node) -> RawTransformPathNodeKindV1 {
    match (
        node.is_root,
        node.is_geometry_transform_helper,
        node.is_scale_helper,
    ) {
        (true, _, _) => RawTransformPathNodeKindV1::ImplicitUfbxRoot,
        (_, true, true) => RawTransformPathNodeKindV1::GeometryAndScaleHelper,
        (_, true, false) => RawTransformPathNodeKindV1::GeometryTransformHelper,
        (_, false, true) => RawTransformPathNodeKindV1::ScaleCompensationHelper,
        (_, false, false) => RawTransformPathNodeKindV1::Source,
    }
}
