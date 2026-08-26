//! Raw-preserving FBX transform hierarchy projection.

use animsmith_core::{
    InputIdentity, RAW_TRANSFORM_PATH_INVENTORY_V1_MAX_ROWS, RawTransformPathInventoryV1,
    RawTransformPathNodeInputV1, RawTransformPathNodeKindV1, SourceFormatV1,
};
use std::collections::BTreeMap;

/// Retain the core's bounded prefix and one row which proves that more input
/// existed. The terminal witness is intentionally not itself retained by the
/// core inventory.
fn retained_prefix_with_terminal_witness<T>(
    nodes: impl Iterator<Item = T>,
) -> impl Iterator<Item = T> {
    nodes.take(RAW_TRANSFORM_PATH_INVENTORY_V1_MAX_ROWS.saturating_add(1))
}

#[derive(Clone, Copy)]
enum NormalizedCorrelation {
    Unmatched,
    Unique(u64),
    Duplicate,
}

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
            // This second parse is evidence-only. `ignore_all_content` is
            // ufbx's coupled geometry/animation/embedded-content switch;
            // node identities, parents, names, and synthetic transform
            // helpers remain available (covered by the raw-path fixtures).
            ignore_all_content: true,
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

    // ufbx exposes nodes in typed-id order. Only identity state for the
    // retained raw prefix is needed: the core consumes one extra node as the
    // overflow witness and then stops. Do not build source-wide maps before
    // that bound is applied.
    let mut raw_id_by_element = BTreeMap::new();
    let mut normalized_by_raw_id = BTreeMap::new();
    for node in raw
        .nodes
        .iter()
        .take(RAW_TRANSFORM_PATH_INVENTORY_V1_MAX_ROWS)
    {
        let raw_id = node.element.typed_id as u64;
        if raw_id_by_element
            .insert(node.element.element_id, raw_id)
            .is_some()
        {
            return RawTransformPathInventoryV1::unavailable(
                primary_input,
                SourceFormatV1::Fbx,
                normalized.nodes.len() as u64,
            );
        }
        normalized_by_raw_id.insert(raw_id, NormalizedCorrelation::Unmatched);
    }

    // The production scene is already materialized. Scan it without retaining
    // unrelated identities, and retain only exact same-load correlations for
    // the raw prefix. Repeated normalized element identities deliberately
    // erase the correlation instead of choosing one.
    for node in normalized.nodes.iter().filter(|node| !is_helper(node)) {
        let Some(&raw_id) = raw_id_by_element.get(&node.element.element_id) else {
            continue;
        };
        let Some(correlation) = normalized_by_raw_id.get_mut(&raw_id) else {
            return RawTransformPathInventoryV1::unavailable(
                primary_input,
                SourceFormatV1::Fbx,
                normalized.nodes.len() as u64,
            );
        };
        *correlation = match *correlation {
            NormalizedCorrelation::Unmatched => {
                NormalizedCorrelation::Unique(node.element.typed_id as u64)
            }
            NormalizedCorrelation::Unique(_) | NormalizedCorrelation::Duplicate => {
                NormalizedCorrelation::Duplicate
            }
        };
    }

    let mut normalized_to_inventory = BTreeMap::new();
    for (&raw_id, correlation) in &normalized_by_raw_id {
        if let NormalizedCorrelation::Unique(normalized_id) = correlation {
            normalized_to_inventory.insert(*normalized_id as u32, raw_id);
        }
    }

    let raw_count = raw.nodes.len();
    let helper_capacity = RAW_TRANSFORM_PATH_INVENTORY_V1_MAX_ROWS.saturating_sub(raw_count);
    // Helpers are appended after every original raw typed-id. Retain only the
    // available helper slots and one terminal witness; their map intentionally
    // excludes that witness because no retained row may point to it.
    let helpers: Vec<&ufbx::Node> = if raw_count <= RAW_TRANSFORM_PATH_INVENTORY_V1_MAX_ROWS {
        retained_prefix_with_terminal_witness(
            normalized
                .nodes
                .iter()
                .filter(|node| is_helper(node))
                .map(|node| &**node),
        )
        .take(helper_capacity.saturating_add(1))
        .collect()
    } else {
        Vec::new()
    };
    for (offset, node) in helpers.iter().take(helper_capacity).enumerate() {
        normalized_to_inventory.insert(
            node.element.typed_id,
            raw_count.saturating_add(offset) as u64,
        );
    }

    let raw_inputs = retained_prefix_with_terminal_witness(raw.nodes.iter()).map(|node| {
        let projected_bone_index = (!node.is_root)
            .then(|| {
                normalized_by_raw_id
                    .get(&(node.element.typed_id as u64))
                    .and_then(|correlation| match correlation {
                        NormalizedCorrelation::Unique(index) => Some(*index),
                        NormalizedCorrelation::Unmatched | NormalizedCorrelation::Duplicate => None,
                    })
            })
            .flatten();
        RawTransformPathNodeInputV1 {
            source_node_index: node.element.typed_id as u64,
            parent_source_node_index: node
                .parent
                .as_ref()
                .map(|parent| parent.element.typed_id as u64),
            source_name: (!node.is_root).then_some(node.element.name.as_ref()),
            projected_bone_index,
            kind: node_kind(node),
        }
    });
    let helper_inputs = helpers.into_iter().enumerate().map(|(offset, node)| {
        let source_node_index = raw_count.saturating_add(offset) as u64;
        let parent_source_node_index = node
            .parent
            .as_ref()
            .and_then(|parent| normalized_to_inventory.get(&parent.element.typed_id))
            .copied();
        RawTransformPathNodeInputV1 {
            source_node_index,
            parent_source_node_index,
            source_name: Some(node.element.name.as_ref()),
            projected_bone_index: Some(node.element.typed_id as u64),
            kind: node_kind(node),
        }
    });

    RawTransformPathInventoryV1::from_nodes(
        primary_input.clone(),
        SourceFormatV1::Fbx,
        normalized.nodes.len() as u64,
        raw_inputs.chain(helper_inputs),
    )
    .unwrap_or_else(|_| {
        RawTransformPathInventoryV1::unavailable(
            primary_input,
            SourceFormatV1::Fbx,
            normalized.nodes.len() as u64,
        )
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use animsmith_core::{
        RAW_TRANSFORM_PATH_INVENTORY_V1_MAX_ROWS, RawTransformPathCoverageReasonV1,
        RawTransformPathCoverageV1,
    };
    use std::cell::Cell;

    #[test]
    fn raw_prefix_reads_only_the_terminal_overflow_witness() {
        let visits = Cell::new(0usize);
        let nodes: Vec<_> = retained_prefix_with_terminal_witness(
            (0..RAW_TRANSFORM_PATH_INVENTORY_V1_MAX_ROWS + 2).inspect(|_| {
                visits.set(visits.get().saturating_add(1));
            }),
        )
        .map(|index| RawTransformPathNodeInputV1 {
            source_node_index: index as u64,
            parent_source_node_index: (index != 0).then_some(0),
            source_name: (index != 0).then_some("node"),
            projected_bone_index: (index != 0).then_some(index as u64),
            kind: if index == 0 {
                RawTransformPathNodeKindV1::ImplicitUfbxRoot
            } else {
                RawTransformPathNodeKindV1::Source
            },
        })
        .collect();

        assert_eq!(visits.get(), RAW_TRANSFORM_PATH_INVENTORY_V1_MAX_ROWS + 1);
        assert_eq!(nodes.len(), RAW_TRANSFORM_PATH_INVENTORY_V1_MAX_ROWS + 1);

        let inventory = RawTransformPathInventoryV1::from_nodes(
            InputIdentity::from_bytes(b"bounded-raw-projection"),
            SourceFormatV1::Fbx,
            (RAW_TRANSFORM_PATH_INVENTORY_V1_MAX_ROWS + 2) as u64,
            nodes,
        )
        .expect("bounded terminal witness is valid inventory input");
        assert_eq!(
            inventory.rows().len(),
            RAW_TRANSFORM_PATH_INVENTORY_V1_MAX_ROWS
        );
        assert_eq!(
            inventory.coverage(),
            RawTransformPathCoverageV1::Partial(
                RawTransformPathCoverageReasonV1::ProjectionBudgetExceeded
            )
        );
    }
}
