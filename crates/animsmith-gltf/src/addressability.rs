//! Same-load projection of immutable raw glTF addressability evidence.

use super::Topology;
use animsmith_core::{
    DependencyClosureV1, InputIdentity, RAW_GLTF_ADDRESSABILITY_V1_MAX_NAME_BYTES,
    RAW_GLTF_ADDRESSABILITY_V1_MAX_PATH_BYTES, RAW_GLTF_ADDRESSABILITY_V1_MAX_PATH_SEGMENTS,
    RAW_GLTF_ADDRESSABILITY_V1_MAX_ROWS_PER_DOMAIN,
    RAW_GLTF_ADDRESSABILITY_V1_MAX_STRUCTURAL_REFERENCES,
    RAW_GLTF_ADDRESSABILITY_V1_MAX_TEXT_BYTES, RawGltfAddressabilityCoverageV1,
    RawGltfAddressabilityInventoryErrorV1, RawGltfAddressabilityInventoryInputV1,
    RawGltfAddressabilityInventoryV1, RawGltfDefaultSceneObservationV1,
    RawGltfInverseBindMatricesObservationV1, RawGltfNodeRowV1, RawGltfScenePathCandidateRowV1,
    RawGltfSceneRowV1, RawGltfSkinAttachmentRowV1, RawGltfSkinRowV1,
};

#[derive(Default)]
struct ProjectionBudget {
    structural_references: usize,
    text_bytes: usize,
}

impl ProjectionBudget {
    fn admit_references(&mut self, count: usize) -> bool {
        let Some(next) = self.structural_references.checked_add(count) else {
            return false;
        };
        if next > RAW_GLTF_ADDRESSABILITY_V1_MAX_STRUCTURAL_REFERENCES {
            return false;
        }
        self.structural_references = next;
        true
    }

    fn admit_row(&mut self, name: Option<&str>, references: usize) -> bool {
        let name_bytes = name.map_or(0, str::len);
        let Some(next_text) = self.text_bytes.checked_add(name_bytes) else {
            return false;
        };
        let Some(next_references) = self.structural_references.checked_add(references) else {
            return false;
        };
        if name_bytes > RAW_GLTF_ADDRESSABILITY_V1_MAX_NAME_BYTES
            || next_text > RAW_GLTF_ADDRESSABILITY_V1_MAX_TEXT_BYTES
            || next_references > RAW_GLTF_ADDRESSABILITY_V1_MAX_STRUCTURAL_REFERENCES
        {
            return false;
        }
        self.text_bytes = next_text;
        self.structural_references = next_references;
        true
    }
}

fn complete_unless_stopped(stopped: bool) -> RawGltfAddressabilityCoverageV1 {
    if stopped {
        RawGltfAddressabilityCoverageV1::budget_exceeded()
    } else {
        RawGltfAddressabilityCoverageV1::Complete
    }
}

pub(super) fn project(
    document: &gltf::Document,
    topology: &Topology,
    primary_input: InputIdentity,
    dependency_closure: DependencyClosureV1,
) -> Result<RawGltfAddressabilityInventoryV1, RawGltfAddressabilityInventoryErrorV1> {
    let mut budget = ProjectionBudget::default();
    let default_scene = match document.default_scene() {
        Some(scene) if budget.admit_references(1) => RawGltfDefaultSceneObservationV1::Selected {
            source_scene_index: scene.index() as u64,
        },
        Some(_) => RawGltfDefaultSceneObservationV1::Unavailable {
            reason: animsmith_core::RawGltfAddressabilityCoverageReasonV1::ProjectionBudgetExceeded,
        },
        None => RawGltfDefaultSceneObservationV1::Absent,
    };

    let mut scenes = Vec::new();
    let mut scenes_stopped = false;
    for scene in document.scenes() {
        if scenes.len() == RAW_GLTF_ADDRESSABILITY_V1_MAX_ROWS_PER_DOMAIN {
            scenes_stopped = true;
            break;
        }
        let root_count = scene.nodes().count();
        if !budget.admit_row(scene.name(), root_count) {
            scenes_stopped = true;
            break;
        }
        scenes.push(RawGltfSceneRowV1::new(
            scene.index() as u64,
            scene.name().map(str::to_owned),
            scene.nodes().map(|node| node.index() as u64).collect(),
        ));
    }

    let mut nodes = Vec::new();
    let mut nodes_stopped = false;
    for node in document.nodes() {
        if nodes.len() == RAW_GLTF_ADDRESSABILITY_V1_MAX_ROWS_PER_DOMAIN {
            nodes_stopped = true;
            break;
        }
        let child_count = node.children().count();
        let reference_count = child_count + usize::from(topology.parent[node.index()].is_some());
        if !budget.admit_row(node.name(), reference_count) {
            nodes_stopped = true;
            break;
        }
        nodes.push(RawGltfNodeRowV1::new(
            node.index() as u64,
            node.name().map(str::to_owned),
            topology.parent[node.index()].map(|index| index as u64),
            node.children().map(|child| child.index() as u64).collect(),
        ));
    }

    let mut skins = Vec::new();
    let mut skins_stopped = false;
    for skin in document.skins() {
        if skins.len() == RAW_GLTF_ADDRESSABILITY_V1_MAX_ROWS_PER_DOMAIN {
            skins_stopped = true;
            break;
        }
        let joint_count = skin.joints().count();
        let reference_count = joint_count
            + usize::from(skin.skeleton().is_some())
            + usize::from(skin.inverse_bind_matrices().is_some());
        if !budget.admit_row(skin.name(), reference_count) {
            skins_stopped = true;
            break;
        }
        skins.push(RawGltfSkinRowV1::new(
            skin.index() as u64,
            skin.name().map(str::to_owned),
            skin.joints().map(|joint| joint.index() as u64).collect(),
            skin.skeleton().map(|node| node.index() as u64),
            match skin.inverse_bind_matrices() {
                Some(accessor) => RawGltfInverseBindMatricesObservationV1::Declared {
                    source_accessor_index: accessor.index() as u64,
                },
                None => RawGltfInverseBindMatricesObservationV1::Absent,
            },
        ));
    }

    let mut attachments = Vec::new();
    let mut attachments_stopped = false;
    for node in document.nodes() {
        let Some(skin) = node.skin() else {
            continue;
        };
        if attachments.len() == RAW_GLTF_ADDRESSABILITY_V1_MAX_ROWS_PER_DOMAIN
            || !budget.admit_references(2)
        {
            attachments_stopped = true;
            break;
        }
        attachments.push(RawGltfSkinAttachmentRowV1::new(
            node.index() as u64,
            skin.index() as u64,
        ));
    }

    let mut path_candidates = Vec::new();
    let mut paths_stopped = false;
    let source_nodes = document.nodes().collect::<Vec<_>>();
    'scenes: for scene in document.scenes() {
        let mut stack = scene
            .nodes()
            .map(|node| (node.index(), node.index()))
            .collect::<Vec<_>>();
        stack.reverse();
        while let Some((root_index, node_index)) = stack.pop() {
            if path_candidates.len() == RAW_GLTF_ADDRESSABILITY_V1_MAX_ROWS_PER_DOMAIN {
                paths_stopped = true;
                break 'scenes;
            }
            let Some(path) = path_from_scene_root(root_index, node_index, topology) else {
                paths_stopped = true;
                break 'scenes;
            };
            if !projected_path_within_bounds(&path, &source_nodes)
                || !budget.admit_references(1 + path.len())
            {
                paths_stopped = true;
                break 'scenes;
            }
            path_candidates.push(RawGltfScenePathCandidateRowV1::new(
                path_candidates.len() as u64,
                scene.index() as u64,
                path.into_iter().map(|index| index as u64).collect(),
            ));
            let mut children = source_nodes[node_index]
                .children()
                .map(|child| (root_index, child.index()))
                .collect::<Vec<_>>();
            children.reverse();
            stack.extend(children);
        }
    }

    RawGltfAddressabilityInventoryV1::new(
        primary_input,
        dependency_closure,
        RawGltfAddressabilityInventoryInputV1 {
            default_scene,
            scene_coverage: complete_unless_stopped(scenes_stopped),
            scenes,
            node_coverage: complete_unless_stopped(nodes_stopped),
            nodes,
            skin_coverage: complete_unless_stopped(skins_stopped),
            skins,
            attachment_coverage: complete_unless_stopped(attachments_stopped),
            attachments,
            path_candidate_coverage: complete_unless_stopped(paths_stopped),
            path_candidates,
        },
    )
}

fn projected_path_within_bounds(path: &[usize], nodes: &[gltf::Node<'_>]) -> bool {
    if path.len() > RAW_GLTF_ADDRESSABILITY_V1_MAX_PATH_SEGMENTS {
        return false;
    }
    let mut bytes = 0usize;
    for (position, &node_index) in path.iter().enumerate() {
        let Some(node) = nodes.get(node_index) else {
            return false;
        };
        let segment_bytes = node
            .name()
            .map_or_else(|| format!("GltfNode{node_index}").len(), str::len);
        let Some(next) = bytes
            .checked_add(usize::from(position > 0))
            .and_then(|value| value.checked_add(segment_bytes))
        else {
            return false;
        };
        if segment_bytes > RAW_GLTF_ADDRESSABILITY_V1_MAX_NAME_BYTES
            || next > RAW_GLTF_ADDRESSABILITY_V1_MAX_PATH_BYTES
        {
            return false;
        }
        bytes = next;
    }
    true
}

fn path_from_scene_root(
    root_index: usize,
    node_index: usize,
    topology: &Topology,
) -> Option<Vec<usize>> {
    let mut path = Vec::new();
    let mut current = Some(node_index);
    while let Some(node) = current {
        if path.len() >= topology.parent.len() {
            return None;
        }
        path.push(node);
        if node == root_index {
            path.reverse();
            return Some(path);
        }
        current = *topology.parent.get(node)?;
    }
    None
}
