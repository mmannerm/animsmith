//! Format-neutral correspondence for a private rest/bind staging document.
//!
//! Both `scale` and `assemble` serialize an admitted normalized document to a
//! private GLB before applying the existing rest/bind writer. Raw source
//! indices cannot cross that serialization boundary. This module maps the
//! selected skin and root by stable normalized identities, then proves the
//! source-parent ancestry of every consumed node is unchanged. It deliberately
//! owns no input-role, recipe, evidence, or publication policy.

use animsmith_core::model::{Document, Skeleton, SourceNodeAsset, SourceSkinAsset};
use animsmith_core::scale::ScaleOperation;
use std::collections::{BTreeMap, BTreeSet};

const CANONICAL_WRITER_ROOT: &str = "animsmith-canonical-root";

/// Map a rest/bind selector across a private normalized-document stage.
///
/// The returned selectors address only `staged`. Callers retain their own
/// format admission, operation policy, evidence, and publication boundaries.
pub(crate) fn map_rest_bind_operation(
    original: &Document,
    staged: &Document,
    operation: ScaleOperation,
    context: &str,
) -> Result<ScaleOperation, String> {
    let ScaleOperation::RestBindUniformScale {
        source_skin_index,
        source_root_node_index,
        expected_factor,
    } = operation
    else {
        return Err(format!("{context} only maps rest/bind operations"));
    };
    let original_nodes = indexed_source_nodes(&original.assets.source_skeleton.nodes, "base")?;
    let staged_nodes = indexed_source_nodes(&staged.assets.source_skeleton.nodes, "staged")?;
    let original_skins = indexed_source_skins(&original.assets.source_skeleton.skins, "base")?;
    let staged_skins = indexed_source_skins(&staged.assets.source_skeleton.skins, "staged")?;
    let correspondence =
        StagedSelectorBoneCorrespondence::new(&original.skeleton, &staged.skeleton);
    let original_root = original_nodes
        .get(&source_root_node_index)
        .ok_or_else(|| format!("base source root id {source_root_node_index} is absent"))?;
    let original_root_bone = original_root
        .bone
        .and_then(|bone| original.skeleton.bones.get(bone))
        .ok_or_else(|| {
            format!(
                "source_root_node_index {source_root_node_index} has no named normalized base node"
            )
        })?;
    let staged_root_bone = correspondence.map_name(
        &original_root_bone.name,
        "staged selector root correspondence",
    )?;
    let staged_root_matches = staged_nodes
        .values()
        .filter(|node| node.bone == Some(staged_root_bone))
        .map(|node| node.source_node_index)
        .collect::<Vec<_>>();
    let [staged_root_node_index] = staged_root_matches.as_slice() else {
        return Err(format!(
            "staged artifact does not map root {:?} to exactly one raw node",
            original_root_bone.name
        ));
    };
    let staged_root = staged_nodes
        .get(staged_root_node_index)
        .ok_or_else(|| format!("staged source root id {staged_root_node_index} is absent"))?;
    let original_skin = original_skins
        .get(&source_skin_index)
        .ok_or_else(|| format!("base source skin id {source_skin_index} is absent"))?;
    let joint_names = original_skin
        .joint_source_node_indices
        .iter()
        .map(|source_index| {
            original_nodes
                .get(source_index)
                .ok_or_else(|| format!("base skin joint source id {source_index} is absent"))
                .and_then(|node| {
                    node.bone
                        .and_then(|bone| original.skeleton.bones.get(bone))
                        .ok_or_else(|| {
                            format!("selected base skin joint {source_index} is not named")
                        })
                })
                .map(|bone| bone.name.clone())
        })
        .collect::<Result<Vec<_>, _>>()?;
    if joint_names.is_empty() {
        return Err("selected base skin has no stable joint topology".into());
    }
    if joint_names.iter().collect::<BTreeSet<_>>().len() != joint_names.len() {
        return Err("selected base skin has duplicate named joint identities".into());
    }
    let mut staged_skin_matches = Vec::new();
    for skin in staged_skins.values() {
        let names = skin
            .joint_source_node_indices
            .iter()
            .map(|source_index| {
                staged_nodes
                    .get(source_index)
                    .and_then(|node| node.bone)
                    .and_then(|bone| staged.skeleton.bones.get(bone))
                    .map(|bone| bone.name.clone())
            })
            .collect::<Option<Vec<_>>>();
        if names.as_deref() == Some(&joint_names) {
            staged_skin_matches.push(skin.source_skin_index);
        }
    }
    let [staged_skin_index] = staged_skin_matches.as_slice() else {
        return Err(
            "staged artifact does not contain exactly one skin with the selected named joint topology"
                .into(),
        );
    };
    let selected_staged_skin = staged_skins
        .get(staged_skin_index)
        .ok_or_else(|| format!("staged source skin id {staged_skin_index} is absent"))?;
    let original_consumed = std::iter::once(original_root.source_node_index)
        .chain(original_skin.joint_source_node_indices.iter().copied())
        .collect::<Vec<_>>();
    let staged_consumed = std::iter::once(staged_root.source_node_index)
        .chain(
            selected_staged_skin
                .joint_source_node_indices
                .iter()
                .copied(),
        )
        .collect::<Vec<_>>();
    require_unique_consumed_raw_source_identities(
        &original_nodes,
        &original.skeleton,
        &original_consumed,
        "base",
    )?;
    require_unique_consumed_raw_source_identities(
        &staged_nodes,
        &staged.skeleton,
        &staged_consumed,
        "staged",
    )?;
    require_source_parent_correspondence(
        original_root,
        staged_root,
        &original_nodes,
        &staged_nodes,
        &original.skeleton,
        &staged.skeleton,
        "staged selector root correspondence",
    )?;
    for (original_joint, staged_joint) in original_skin
        .joint_source_node_indices
        .iter()
        .zip(&selected_staged_skin.joint_source_node_indices)
    {
        let original_node = original_nodes
            .get(original_joint)
            .ok_or_else(|| format!("base skin joint source id {original_joint} is absent"))?;
        let staged_node = staged_nodes
            .get(staged_joint)
            .ok_or_else(|| format!("staged skin joint source id {staged_joint} is absent"))?;
        let original_bone = original_node
            .bone
            .and_then(|bone| original.skeleton.bones.get(bone))
            .ok_or_else(|| format!("base skin joint source id {original_joint} is not named"))?;
        correspondence.map_name(&original_bone.name, "staged skin joint correspondence")?;
        require_source_parent_correspondence(
            original_node,
            staged_node,
            &original_nodes,
            &staged_nodes,
            &original.skeleton,
            &staged.skeleton,
            "staged skin joint correspondence",
        )?;
    }
    Ok(ScaleOperation::RestBindUniformScale {
        source_skin_index: *staged_skin_index,
        source_root_node_index: *staged_root_node_index,
        expected_factor,
    })
}

struct StableBoneIndex {
    by_name: BTreeMap<String, usize>,
    names_by_index: Vec<String>,
    parent_by_index: Vec<Option<usize>>,
    ambiguous: BTreeSet<String>,
}

impl StableBoneIndex {
    fn new(skeleton: &Skeleton) -> Self {
        let mut by_name = BTreeMap::new();
        let mut names_by_index = Vec::with_capacity(skeleton.bones.len());
        let parent_by_index = skeleton.bones.iter().map(|bone| bone.parent).collect();
        let mut ambiguous = BTreeSet::new();
        for (index, bone) in skeleton.bones.iter().enumerate() {
            if by_name.insert(bone.name.clone(), index).is_some() {
                ambiguous.insert(bone.name.clone());
            }
            names_by_index.push(bone.name.clone());
        }
        Self {
            by_name,
            names_by_index,
            parent_by_index,
            ambiguous,
        }
    }

    fn resolve(&self, name: &str, context: &str) -> Result<usize, String> {
        if name.is_empty() {
            return Err(format!("{context} contains an empty stable bone identity"));
        }
        if self.ambiguous.contains(name) {
            return Err(format!(
                "{context} found ambiguous stable bone identity {name:?}"
            ));
        }
        self.by_name
            .get(name)
            .copied()
            .ok_or_else(|| format!("{context} cannot resolve stable bone identity {name:?}"))
    }

    fn ancestry(&self, index: usize, context: &str) -> Result<Vec<&str>, String> {
        let mut ancestry = Vec::new();
        let mut seen = BTreeSet::from([index]);
        let mut parent = *self
            .parent_by_index
            .get(index)
            .ok_or_else(|| format!("{context} references missing bone index {index}"))?;
        while let Some(parent_index) = parent {
            if !seen.insert(parent_index) {
                return Err(format!(
                    "{context} contains a cyclic parent chain at bone index {parent_index}"
                ));
            }
            let parent_name = self
                .names_by_index
                .get(parent_index)
                .map(String::as_str)
                .ok_or_else(|| format!("{context} references missing bone index {parent_index}"))?;
            self.resolve(parent_name, context)?;
            ancestry.push(parent_name);
            parent = *self.parent_by_index.get(parent_index).ok_or_else(|| {
                format!("{context} references missing parent bone index {parent_index}")
            })?;
        }
        Ok(ancestry)
    }
}

struct StagedSelectorBoneCorrespondence {
    original: StableBoneIndex,
    staged: StableBoneIndex,
}

impl StagedSelectorBoneCorrespondence {
    fn new(original: &Skeleton, staged: &Skeleton) -> Self {
        Self {
            original: StableBoneIndex::new(original),
            staged: StableBoneIndex::new(staged),
        }
    }

    fn map_name(&self, name: &str, context: &str) -> Result<usize, String> {
        let original = self.original.resolve(name, context)?;
        let mut original_ancestry = self.original.ancestry(original, context)?;
        let staged = self.staged.resolve(name, context)?;
        let mut staged_ancestry = self.staged.ancestry(staged, context)?;
        original_ancestry.retain(|ancestor| *ancestor != CANONICAL_WRITER_ROOT);
        staged_ancestry.retain(|ancestor| *ancestor != CANONICAL_WRITER_ROOT);
        if original_ancestry != staged_ancestry {
            return Err(format!(
                "{context} ancestor identity for bone {name:?} differs between selector spaces"
            ));
        }
        Ok(staged)
    }
}

fn require_source_parent_correspondence(
    left: &SourceNodeAsset,
    right: &SourceNodeAsset,
    left_nodes: &BTreeMap<usize, &SourceNodeAsset>,
    right_nodes: &BTreeMap<usize, &SourceNodeAsset>,
    left_skeleton: &Skeleton,
    right_skeleton: &Skeleton,
    context: &str,
) -> Result<(), String> {
    let ancestry = |node: &SourceNodeAsset,
                    nodes: &BTreeMap<usize, &SourceNodeAsset>,
                    skeleton: &Skeleton|
     -> Result<Vec<(bool, String)>, String> {
        let mut ancestry = Vec::new();
        let mut seen = BTreeSet::from([node.source_node_index]);
        let mut parent = node.parent_source_node_index;
        while let Some(parent_index) = parent {
            if !seen.insert(parent_index) {
                return Err(format!(
                    "{context} source node {} has a cyclic parent chain at source id {parent_index}",
                    node.source_node_index
                ));
            }
            let parent_node = nodes.get(&parent_index).ok_or_else(|| {
                format!(
                    "{context} source node {} has stale parent id {parent_index}",
                    node.source_node_index
                )
            })?;
            let identity = if let Some(bone) = parent_node.bone {
                let bone = skeleton.bones.get(bone).ok_or_else(|| {
                    format!(
                        "{context} source parent {parent_index} references missing normalized bone {bone}"
                    )
                })?;
                if bone.name.is_empty() {
                    return Err(format!(
                        "{context} source parent {parent_index} has an empty normalized identity"
                    ));
                }
                if bone.name != CANONICAL_WRITER_ROOT {
                    Some((true, bone.name.clone()))
                } else {
                    None
                }
            } else {
                let name = parent_node.name.as_deref().ok_or_else(|| {
                    format!("{context} source parent {parent_index} has no stable source identity")
                })?;
                if name.is_empty() {
                    return Err(format!(
                        "{context} source parent {parent_index} has an empty source identity"
                    ));
                }
                Some((false, name.to_owned()))
            };
            if let Some(identity) = identity {
                if ancestry.contains(&identity) {
                    return Err(format!(
                        "{context} source node {} has duplicate ancestor identity {:?}",
                        node.source_node_index, identity.1
                    ));
                }
                ancestry.push(identity);
            }
            parent = parent_node.parent_source_node_index;
        }
        Ok(ancestry)
    };
    let left_ancestry = ancestry(left, left_nodes, left_skeleton)?;
    let right_ancestry = ancestry(right, right_nodes, right_skeleton)?;
    if left_ancestry != right_ancestry {
        return Err(format!(
            "{context} ancestor identity differs for consumed source node {}",
            left.source_node_index
        ));
    }
    Ok(())
}

fn require_unique_consumed_raw_source_identities(
    nodes: &BTreeMap<usize, &SourceNodeAsset>,
    skeleton: &Skeleton,
    consumed_source_node_indices: &[usize],
    context: &str,
) -> Result<(), String> {
    let mut raw_identities = BTreeMap::<String, usize>::new();
    for source_node_index in consumed_source_node_indices {
        let mut seen = BTreeSet::new();
        let mut current = Some(*source_node_index);
        while let Some(source_node_index) = current {
            if !seen.insert(source_node_index) {
                return Err(format!(
                    "{context} consumed source node {source_node_index} has a cyclic parent chain"
                ));
            }
            let node = nodes.get(&source_node_index).ok_or_else(|| {
                format!(
                    "{context} consumed source node {source_node_index} is absent from the source projection"
                )
            })?;
            if let Some(bone) = node.bone {
                skeleton.bones.get(bone).ok_or_else(|| {
                    format!(
                        "{context} consumed source node {source_node_index} references missing normalized bone {bone}"
                    )
                })?;
            } else {
                let name = node.name.as_deref().ok_or_else(|| {
                    format!(
                        "{context} consumed raw source node {source_node_index} has no stable identity"
                    )
                })?;
                if name.is_empty() {
                    return Err(format!(
                        "{context} consumed raw source node {source_node_index} has an empty stable identity"
                    ));
                }
                if let Some(previous) = raw_identities.insert(name.to_owned(), source_node_index)
                    && previous != source_node_index
                {
                    return Err(format!(
                        "{context} consumed raw source nodes {previous} and {source_node_index} share stable identity {name:?}"
                    ));
                }
            }
            if let Some(parent) = node.parent_source_node_index
                && !nodes.contains_key(&parent)
            {
                return Err(format!(
                    "{context} consumed source node {source_node_index} has stale parent id {parent}"
                ));
            }
            current = node.parent_source_node_index;
        }
    }
    Ok(())
}

fn indexed_source_nodes<'a>(
    nodes: &'a [SourceNodeAsset],
    context: &str,
) -> Result<BTreeMap<usize, &'a SourceNodeAsset>, String> {
    let mut indexed = BTreeMap::new();
    for node in nodes {
        if indexed.insert(node.source_node_index, node).is_some() {
            return Err(format!(
                "{context} source node id {} is duplicated",
                node.source_node_index
            ));
        }
    }
    Ok(indexed)
}

fn indexed_source_skins<'a>(
    skins: &'a [SourceSkinAsset],
    context: &str,
) -> Result<BTreeMap<usize, &'a SourceSkinAsset>, String> {
    let mut indexed = BTreeMap::new();
    for skin in skins {
        if indexed.insert(skin.source_skin_index, skin).is_some() {
            return Err(format!(
                "{context} source skin id {} is duplicated",
                skin.source_skin_index
            ));
        }
    }
    Ok(indexed)
}
