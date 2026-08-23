//! Versioned semantic basis projected from an accepted scale plan.

use super::{
    ScaleError, ScaleOperation, ScalePlan, ScaleProjectedRole, ScaleSourceNodeKind,
    ScaleTolerancePolicy,
};
use crate::model::{
    Document, Property, SourceNodeLocalRest, SourceSkeletonCoverage, TrackValues,
    validate_document_shape,
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

/// Stable semantic version of [`AssemblyScaleBasis`].
pub const ASSEMBLY_SCALE_BASIS_VERSION: u32 = 1;

/// One named normalized node and its exact authored rest basis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssemblyScaleNamedNode {
    /// Stable node name used by assembly remapping.
    pub name: String,
    /// Parent node name, when any.
    pub parent: Option<String>,
    /// Translation component bits.
    pub translation_bits: [u32; 3],
    /// Rotation component bits in `[x, y, z, w]` order.
    pub rotation_bits: [u32; 4],
    /// Scale component bits.
    pub scale_bits: [u32; 3],
}

/// One raw source node, including projected/helper layout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssemblyScaleSourceNode {
    /// Raw source node identity.
    pub source_node_index: usize,
    /// Raw parent identity.
    pub parent_source_node_index: Option<usize>,
    /// Authored name, when present.
    pub name: Option<String>,
    /// Stable projected/helper role.
    pub role: String,
    /// Authored local-rest representation and exact component bits.
    pub local_rest: AssemblyScaleSourceRest,
}

/// Authored raw source-node rest representation retained by a basis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AssemblyScaleSourceRest {
    /// Decomposed translation, quaternion, and scale.
    Trs {
        /// Translation component bits.
        translation_bits: [u32; 3],
        /// Quaternion component bits in `[x, y, z, w]` order.
        rotation_bits: [u32; 4],
        /// Scale component bits.
        scale_bits: [u32; 3],
    },
    /// Exact authored column-major local matrix bits.
    Matrix {
        /// Matrix component bits.
        matrix_bits: [u32; 16],
    },
}

/// One animation channel target and the plan-owned effective multiplier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssemblyScaleTargetPath {
    /// Clip position in the input document.
    pub clip_index: usize,
    /// Track position inside the clip.
    pub track_index: usize,
    /// Named target used by assembly remapping.
    pub bone: String,
    /// Stable property name.
    pub property: &'static str,
    /// Effective multiplier encoded without float spelling ambiguity.
    pub factor_bits: u64,
}

/// Complete versioned semantic basis for one assembly input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssemblyScaleBasis {
    /// Basis schema version.
    pub version: u32,
    /// Fixed model coordinate convention.
    pub coordinate_convention: &'static str,
    /// Tolerance identity used for semantic compatibility.
    pub tolerance_policy_id: &'static str,
    /// Selected source skin.
    pub source_skin_index: usize,
    /// Selected source root node.
    pub source_root_node_index: usize,
    /// Declared factor bits.
    pub expected_factor_bits: u64,
    /// Normalized named topology and rest/orientation basis.
    pub named_nodes: Vec<AssemblyScaleNamedNode>,
    /// Raw projected/helper topology and local-rest basis.
    pub source_nodes: Vec<AssemblyScaleSourceNode>,
    /// Animation target paths and effective factors.
    pub target_paths: Vec<AssemblyScaleTargetPath>,
}

/// Versioned semantic basis for a meshless clip whose source has no skin.
///
/// Such a clip has animation targets to rebase, but no inverse-bind domain.
/// This basis deliberately cannot be mistaken for [`AssemblyScaleBasis`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssemblyScaleSkinlessClipBasis {
    /// Basis schema version.
    pub version: u32,
    /// Fixed model coordinate convention.
    pub coordinate_convention: &'static str,
    /// Tolerance identity used for semantic compatibility.
    pub tolerance_policy_id: &'static str,
    /// Exact named selector resolved in this input.
    pub root_node_name: String,
    /// Format-local source root selected by that name.
    pub source_root_node_index: usize,
    /// Declared factor bits inherited from the proved base plan.
    pub expected_factor_bits: u64,
    /// Normalized named topology and rest/orientation basis.
    pub named_nodes: Vec<AssemblyScaleNamedNode>,
    /// Animation target paths and base-plan-owned effective factors.
    pub target_paths: Vec<AssemblyScaleTargetPath>,
}

/// Why two independently supplied assembly inputs do not share one basis.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("assembly scale basis mismatch ({reason})")]
pub struct AssemblyScaleCompatibilityError {
    /// Stable machine-readable mismatch reason.
    pub reason: &'static str,
}

/// Selector mode requested while deriving an assembly compatibility basis.
///
/// The request is not itself compatibility identity. The constructor checks
/// it against the accepted basis and source document before producing the
/// validated identity consumed by the comparator.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum AssemblyScaleSelectorRequest<'a> {
    /// Preserve exact format-local source skin and root indices.
    Indexed,
    /// Derive identity from one exact normalized root name.
    Named {
        /// Exact, case-sensitive root name already selected by the caller.
        root_node_name: &'a str,
    },
}

/// Format-neutral source indices resolved from one exact named assembly root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssemblyScaleResolvedNamedSelector {
    /// The unique non-empty source skin fully governed by the named root.
    pub source_skin_index: usize,
    /// The unique source node projected from the named normalized root.
    pub source_root_node_index: usize,
}

/// Why a named assembly scale selector did not resolve exactly once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum AssemblyScaleNamedSelectorResolutionError {
    /// The exact normalized name matched zero or multiple source nodes.
    #[error("named assembly scale root resolves to {matches} source nodes; expected exactly one")]
    RootNotUnique {
        /// Number of matching projected source nodes.
        matches: usize,
    },
    /// The resolved root fully governed zero or multiple source skins.
    #[error("named assembly scale root fully governs {matches} source skins; expected exactly one")]
    SkinNotUnique {
        /// Number of non-empty fully governed source skins.
        matches: usize,
    },
}

#[derive(Debug, Clone)]
enum AssemblyScaleSelectorIdentity {
    Indexed,
    Named {
        root_node_name: String,
        skin_joint_names: Vec<String>,
    },
}

/// One assembly basis paired with core-validated selector identity.
///
/// Named identity cannot be assembled directly by a caller: it is derived
/// from the source document's unique root and fully governed skin facts.
#[derive(Debug, Clone)]
pub struct AssemblyScaleCompatibilityBasis {
    basis: AssemblyScaleBasis,
    selector: AssemblyScaleSelectorIdentity,
    animation_target_factors: BTreeMap<(String, Property), f64>,
    affected_node_names: BTreeSet<String>,
}

fn named_nodes(document: &Document) -> Result<Vec<AssemblyScaleNamedNode>, ScaleError> {
    let mut names = BTreeSet::new();
    let mut named_nodes = Vec::with_capacity(document.skeleton.bones.len());
    for (index, bone) in document.skeleton.bones.iter().enumerate() {
        if bone.name.is_empty() || !names.insert(bone.name.as_str()) {
            return Err(ScaleError::PlanDocumentMismatch {
                reason: "assembly_basis_requires_unique_named_nodes",
            });
        }
        named_nodes.push(AssemblyScaleNamedNode {
            name: bone.name.clone(),
            parent: bone
                .parent
                .and_then(|parent| document.skeleton.bones.get(parent))
                .map(|parent| parent.name.clone()),
            translation_bits: bone.rest.translation.to_array().map(f32::to_bits),
            rotation_bits: bone.rest.rotation.to_array().map(f32::to_bits),
            scale_bits: bone.rest.scale.to_array().map(f32::to_bits),
        });
        if bone.parent.is_some_and(|parent| parent >= index) {
            return Err(ScaleError::PlanDocumentMismatch {
                reason: "assembly_basis_parent_order",
            });
        }
    }
    Ok(named_nodes)
}

fn governed_source_node_indices(
    parent_by_index: &BTreeMap<usize, Option<usize>>,
    source_root_node_index: usize,
) -> BTreeSet<usize> {
    if !parent_by_index.contains_key(&source_root_node_index) {
        return BTreeSet::new();
    }

    let mut children_by_index = BTreeMap::<usize, Vec<usize>>::new();
    for (&source_node_index, parent_source_node_index) in parent_by_index {
        if let Some(parent_source_node_index) = parent_source_node_index {
            children_by_index
                .entry(*parent_source_node_index)
                .or_default()
                .push(source_node_index);
        }
    }

    let mut governed = BTreeSet::new();
    let mut pending = vec![source_root_node_index];
    while let Some(source_node_index) = pending.pop() {
        if governed.insert(source_node_index)
            && let Some(children) = children_by_index.get(&source_node_index)
        {
            pending.extend(children.iter().copied());
        }
    }
    governed
}

fn source_skin_is_fully_governed(
    governed_source_node_indices: &BTreeSet<usize>,
    joint_source_node_indices: &[usize],
) -> bool {
    !joint_source_node_indices.is_empty()
        && joint_source_node_indices
            .iter()
            .all(|joint| governed_source_node_indices.contains(joint))
}

/// Resolve one exact normalized root name to its unique fully governed skin.
///
/// A skin matches only when it is non-empty and every joint is the resolved
/// root or one of its descendants in the authoritative source-node hierarchy.
/// Callers remain responsible for choosing named versus indexed selector mode;
/// this helper only centralizes the format-neutral named-resolution rule.
///
/// # Errors
///
/// Returns [`AssemblyScaleNamedSelectorResolutionError::RootNotUnique`] when
/// the name does not resolve to exactly one projected source node, or
/// [`AssemblyScaleNamedSelectorResolutionError::SkinNotUnique`] when that node
/// does not fully govern exactly one non-empty source skin.
pub fn resolve_assembly_scale_named_selector(
    document: &Document,
    root_node_name: &str,
) -> Result<AssemblyScaleResolvedNamedSelector, AssemblyScaleNamedSelectorResolutionError> {
    let root_matches = document
        .assets
        .source_skeleton
        .nodes
        .iter()
        .filter_map(|node| {
            node.bone
                .and_then(|bone| document.skeleton.bones.get(bone))
                .filter(|bone| bone.name == root_node_name)
                .map(|_| node.source_node_index)
        })
        .collect::<Vec<_>>();
    let [source_root_node_index] = root_matches.as_slice() else {
        return Err(AssemblyScaleNamedSelectorResolutionError::RootNotUnique {
            matches: root_matches.len(),
        });
    };
    let parent_by_index = document
        .assets
        .source_skeleton
        .nodes
        .iter()
        .map(|node| (node.source_node_index, node.parent_source_node_index))
        .collect::<BTreeMap<_, _>>();
    let governed_source_node_indices =
        governed_source_node_indices(&parent_by_index, *source_root_node_index);
    let skin_matches = document
        .assets
        .source_skeleton
        .skins
        .iter()
        .filter(|skin| {
            source_skin_is_fully_governed(
                &governed_source_node_indices,
                &skin.joint_source_node_indices,
            )
        })
        .collect::<Vec<_>>();
    let [skin] = skin_matches.as_slice() else {
        return Err(AssemblyScaleNamedSelectorResolutionError::SkinNotUnique {
            matches: skin_matches.len(),
        });
    };
    Ok(AssemblyScaleResolvedNamedSelector {
        source_skin_index: skin.source_skin_index,
        source_root_node_index: *source_root_node_index,
    })
}

impl AssemblyScaleCompatibilityBasis {
    /// The immutable evidence basis governed by this validated selector.
    #[must_use]
    pub fn basis(&self) -> &AssemblyScaleBasis {
        &self.basis
    }
}

/// Project an accepted rest/bind plan into the versioned assembly basis.
///
/// # Errors
///
/// Returns the plan's validation error, a selector-operation mismatch, or a
/// duplicate/empty named-node error that would make name remapping ambiguous.
pub fn assembly_scale_basis(
    document: &Document,
    plan: &ScalePlan,
) -> Result<AssemblyScaleBasis, ScaleError> {
    plan.validate_document_inventory(document)?;
    let ScaleOperation::RestBindUniformScale {
        source_skin_index,
        source_root_node_index,
        expected_factor,
    } = plan.operation()
    else {
        return Err(ScaleError::PlanDocumentMismatch {
            reason: "assembly_basis_requires_rest_bind",
        });
    };
    let named_nodes = named_nodes(document)?;
    let source_by_index = document
        .assets
        .source_skeleton
        .nodes
        .iter()
        .map(|node| (node.source_node_index, node))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut source_nodes = Vec::new();
    for row in plan.ledger().source_topology() {
        let source = source_by_index.get(&row.source_node_index()).ok_or(
            ScaleError::PlanDocumentMismatch {
                reason: "assembly_basis_source_node_missing",
            },
        )?;
        let local_rest = match source.local_rest {
            SourceNodeLocalRest::Trs {
                translation,
                rotation,
                scale,
            } => AssemblyScaleSourceRest::Trs {
                translation_bits: translation.to_array().map(f32::to_bits),
                rotation_bits: rotation.to_array().map(f32::to_bits),
                scale_bits: scale.to_array().map(f32::to_bits),
            },
            SourceNodeLocalRest::Matrix(matrix) => AssemblyScaleSourceRest::Matrix {
                matrix_bits: matrix.to_cols_array().map(f32::to_bits),
            },
        };
        let role = match row.kind() {
            ScaleSourceNodeKind::Projected { role, .. } => match role {
                ScaleProjectedRole::Root => "projected-root",
                ScaleProjectedRole::Joint => "projected-joint",
                ScaleProjectedRole::TransformOnly => "projected-transform-only",
            },
            ScaleSourceNodeKind::Connector => "connector",
            ScaleSourceNodeKind::OutsideDomain { bone: Some(_) } => "outside-projected",
            ScaleSourceNodeKind::OutsideDomain { bone: None } => "outside-helper",
        };
        source_nodes.push(AssemblyScaleSourceNode {
            source_node_index: row.source_node_index(),
            parent_source_node_index: row.parent_source_node_index(),
            name: source.name.clone(),
            role: role.to_owned(),
            local_rest,
        });
    }
    let mut target_paths = Vec::new();
    for (clip_index, clip) in document.clips.iter().enumerate() {
        for (track_index, track) in clip.tracks.iter().enumerate() {
            let bone = document
                .skeleton
                .bones
                .get(track.bone)
                .ok_or(ScaleError::BoneIndexOutOfRange { index: track.bone })?;
            target_paths.push(AssemblyScaleTargetPath {
                clip_index,
                track_index,
                bone: bone.name.clone(),
                property: track.property.as_str(),
                factor_bits: plan
                    .animation_target_factor_unchecked(document, track.bone, track.property)?
                    .to_bits(),
            });
        }
    }
    Ok(AssemblyScaleBasis {
        version: ASSEMBLY_SCALE_BASIS_VERSION,
        coordinate_convention: "right-handed-y-up-metres",
        tolerance_policy_id: plan.tolerance_policy().id,
        source_skin_index,
        source_root_node_index,
        expected_factor_bits: expected_factor.to_bits(),
        named_nodes,
        source_nodes,
        target_paths,
    })
}

/// Project one accepted plan and pair it with selector identity derived from
/// the same document.
///
/// # Errors
///
/// Returns the plan's validation error, or a plan/document mismatch if a named
/// request does not resolve to exactly the planned root and skin, or if any
/// selected skin joint lacks one normalized name.
pub fn assembly_scale_compatibility_basis(
    document: &Document,
    plan: &ScalePlan,
    selector: AssemblyScaleSelectorRequest<'_>,
) -> Result<AssemblyScaleCompatibilityBasis, ScaleError> {
    // Derive both halves from one document/plan pair so callers cannot seal a
    // selector identity from one document around another document's basis.
    let basis = assembly_scale_basis(document, plan)?;
    let selector = match selector {
        AssemblyScaleSelectorRequest::Indexed => AssemblyScaleSelectorIdentity::Indexed,
        AssemblyScaleSelectorRequest::Named { root_node_name } => {
            let resolved = resolve_assembly_scale_named_selector(document, root_node_name)
                .map_err(|error| ScaleError::PlanDocumentMismatch {
                    reason: match error {
                        AssemblyScaleNamedSelectorResolutionError::RootNotUnique { .. } => {
                            "assembly_basis_named_selector_root_not_unique"
                        }
                        AssemblyScaleNamedSelectorResolutionError::SkinNotUnique { .. } => {
                            "assembly_basis_named_selector_skin_not_unique"
                        }
                    },
                })?;
            if resolved.source_root_node_index != basis.source_root_node_index {
                return Err(ScaleError::PlanDocumentMismatch {
                    reason: "assembly_basis_named_selector_root_disagrees_with_plan",
                });
            }
            let skin = document
                .assets
                .source_skeleton
                .skins
                .iter()
                .find(|skin| skin.source_skin_index == resolved.source_skin_index)
                .ok_or(ScaleError::PlanDocumentMismatch {
                    reason: "assembly_basis_named_selector_skin_not_unique",
                })?;
            if resolved.source_skin_index != basis.source_skin_index {
                return Err(ScaleError::PlanDocumentMismatch {
                    reason: "assembly_basis_named_selector_skin_disagrees_with_plan",
                });
            }
            let source_nodes = document
                .assets
                .source_skeleton
                .nodes
                .iter()
                .map(|node| (node.source_node_index, node))
                .collect::<std::collections::BTreeMap<_, _>>();
            let skin_joint_names = skin
                .joint_source_node_indices
                .iter()
                .map(|source_index| {
                    source_nodes
                        .get(source_index)
                        .and_then(|node| node.bone)
                        .and_then(|bone| document.skeleton.bones.get(bone))
                        .map(|bone| bone.name.clone())
                        .ok_or(ScaleError::PlanDocumentMismatch {
                            reason: "assembly_basis_named_selector_joint_has_no_name",
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            AssemblyScaleSelectorIdentity::Named {
                root_node_name: root_node_name.to_owned(),
                skin_joint_names,
            }
        }
    };
    let mut animation_target_factors = BTreeMap::new();
    for (bone, named) in document.skeleton.bones.iter().enumerate() {
        for property in [Property::Translation, Property::Rotation, Property::Scale] {
            animation_target_factors.insert(
                (named.name.clone(), property),
                plan.animation_target_factor_unchecked(document, bone, property)?,
            );
        }
    }
    let affected_node_names = plan
        .affected_nodes()
        .iter()
        .filter_map(|&bone| document.skeleton.bones.get(bone))
        .map(|bone| bone.name.clone())
        .collect();
    Ok(AssemblyScaleCompatibilityBasis {
        basis,
        selector,
        animation_target_factors,
        affected_node_names,
    })
}

/// Rebase animation targets in one meshless, skinless clip against a proved
/// named base plan.
///
/// The base plan remains the sole owner of rest/bind selection and proof. The
/// clip must have the same normalized named topology and rest basis for the
/// selected skin's joints, the root, each actual animation target, and their
/// named ancestry. Targets must remain inside the base plan's affected closure,
/// but unreferenced base-only descendants are not clip requirements. The clip
/// must also have exactly one source node for `root_node_name`, no source skins,
/// and no mesh instances. Only animation values are changed; cubic-spline
/// tangent triplets are stored in the same value array and therefore receive
/// the same factor.
///
/// # Errors
///
/// Returns a stable compatibility mismatch when the input is not the narrow
/// meshless animation-clip domain or its normalized skeleton/track targets are
/// malformed.
pub fn rebase_assembly_scale_skinless_clip(
    base: &AssemblyScaleCompatibilityBasis,
    document: &Document,
    root_node_name: &str,
) -> Result<(Document, AssemblyScaleSkinlessClipBasis), AssemblyScaleCompatibilityError> {
    let AssemblyScaleSelectorIdentity::Named {
        root_node_name: base_root,
        skin_joint_names,
    } = &base.selector
    else {
        return Err(AssemblyScaleCompatibilityError {
            reason: "source-selector-mode",
        });
    };
    if base_root != root_node_name {
        return Err(AssemblyScaleCompatibilityError {
            reason: "source-name-selector",
        });
    }
    validate_document_shape(document).map_err(|_| AssemblyScaleCompatibilityError {
        reason: "skinless-clip-invalid-document",
    })?;
    if document.assets.source_skeleton.coverage != SourceSkeletonCoverage::Complete {
        return Err(AssemblyScaleCompatibilityError {
            reason: "skinless-clip-source-coverage",
        });
    }
    if !document.assets.source_skeleton.skins.is_empty() {
        return Err(AssemblyScaleCompatibilityError {
            reason: "skinless-clip-has-source-skins",
        });
    }
    if !document.assets.instances.is_empty() {
        return Err(AssemblyScaleCompatibilityError {
            reason: "skinless-clip-has-mesh-instances",
        });
    }
    let root_matches = document
        .assets
        .source_skeleton
        .nodes
        .iter()
        .filter_map(|node| {
            node.bone
                .and_then(|bone| document.skeleton.bones.get(bone))
                .filter(|bone| bone.name == root_node_name)
                .map(|_| node.source_node_index)
        })
        .collect::<Vec<_>>();
    let [source_root_node_index] = root_matches.as_slice() else {
        return Err(AssemblyScaleCompatibilityError {
            reason: "source-root-name-not-unique",
        });
    };
    let input_named_nodes = named_nodes(document).map_err(|_| AssemblyScaleCompatibilityError {
        reason: "named-topology",
    })?;
    // A meshless clip must carry the complete selected rig, not geometry-only
    // descendants that happen to lie in the base plan's broader rewrite
    // closure. Actual animation targets widen that rig domain when the base
    // plan also governs them, so animated helpers remain proved rather than
    // being admitted by a permissive name intersection.
    let mut relevant_names = skin_joint_names.iter().cloned().collect::<BTreeSet<_>>();
    relevant_names.insert(base_root.clone());
    for clip in &document.clips {
        for track in &clip.tracks {
            let bone =
                document
                    .skeleton
                    .bones
                    .get(track.bone)
                    .ok_or(AssemblyScaleCompatibilityError {
                        reason: "animation-target-bone",
                    })?;
            if !base
                .animation_target_factors
                .contains_key(&(bone.name.clone(), track.property))
            {
                return Err(AssemblyScaleCompatibilityError {
                    reason: "animation-target-bone",
                });
            }
            if !base.affected_node_names.contains(&bone.name) {
                return Err(AssemblyScaleCompatibilityError {
                    reason: "animation-target-outside-scale-domain",
                });
            }
            relevant_names.insert(bone.name.clone());
        }
    }
    loop {
        let before = relevant_names.len();
        for node in &base.basis.named_nodes {
            if relevant_names.contains(&node.name)
                && let Some(parent) = &node.parent
            {
                relevant_names.insert(parent.clone());
            }
        }
        if relevant_names.len() == before {
            break;
        }
    }
    let base_named_nodes = base
        .basis
        .named_nodes
        .iter()
        .filter(|node| relevant_names.contains(&node.name))
        .cloned()
        .collect::<Vec<_>>();
    let input_named_nodes = input_named_nodes
        .into_iter()
        .filter(|node| relevant_names.contains(&node.name))
        .collect::<Vec<_>>();
    let tolerance = ScaleTolerancePolicy::APPENDIX_D_V6;
    if !same_named_topology(&base_named_nodes, &input_named_nodes) {
        return Err(AssemblyScaleCompatibilityError {
            reason: "named-topology",
        });
    }
    if !same_named_rest(&base_named_nodes, &input_named_nodes, &tolerance) {
        return Err(AssemblyScaleCompatibilityError {
            reason: "named-rest-basis",
        });
    }
    if !same_named_orientations(&base_named_nodes, &input_named_nodes, &tolerance) {
        return Err(AssemblyScaleCompatibilityError {
            reason: "named-orientation",
        });
    }

    let mut rebased = document.clone();
    let mut target_paths = Vec::new();
    for (clip_index, clip) in rebased.clips.iter_mut().enumerate() {
        for (track_index, track) in clip.tracks.iter_mut().enumerate() {
            let bone =
                document
                    .skeleton
                    .bones
                    .get(track.bone)
                    .ok_or(AssemblyScaleCompatibilityError {
                        reason: "animation-target-bone",
                    })?;
            let factor = *base
                .animation_target_factors
                .get(&(bone.name.clone(), track.property))
                .ok_or(AssemblyScaleCompatibilityError {
                    reason: "animation-target-bone",
                })?;
            match (&mut track.values, track.property) {
                (TrackValues::Vec3s(values), Property::Translation) => {
                    let factor = factor as f32;
                    for value in values {
                        *value *= factor;
                    }
                }
                (TrackValues::Vec3s(values), Property::Scale) => {
                    for value in values {
                        *value = (value.as_dvec3() * factor).as_vec3();
                    }
                }
                (TrackValues::Quats(_), Property::Rotation) => {}
                _ => {
                    return Err(AssemblyScaleCompatibilityError {
                        reason: "animation-value-kind",
                    });
                }
            }
            target_paths.push(AssemblyScaleTargetPath {
                clip_index,
                track_index,
                bone: bone.name.clone(),
                property: track.property.as_str(),
                factor_bits: factor.to_bits(),
            });
        }
    }
    validate_document_shape(&rebased).map_err(|_| AssemblyScaleCompatibilityError {
        reason: "skinless-clip-rebase-invalid-document",
    })?;
    Ok((
        rebased,
        AssemblyScaleSkinlessClipBasis {
            version: ASSEMBLY_SCALE_BASIS_VERSION,
            coordinate_convention: base.basis.coordinate_convention,
            tolerance_policy_id: base.basis.tolerance_policy_id,
            root_node_name: root_node_name.to_owned(),
            source_root_node_index: *source_root_node_index,
            expected_factor_bits: base.basis.expected_factor_bits,
            named_nodes: input_named_nodes,
            target_paths,
        },
    ))
}

/// Require two bases to agree on every static semantic field.
///
/// Target paths intentionally remain per-input fingerprint material: clip
/// files may contain different takes. Each target's named node and factor are
/// checked against its own accepted plan when the basis is built.
///
/// # Errors
///
/// Returns the first stable mismatch category.
pub fn require_assembly_scale_compatibility(
    base: &AssemblyScaleBasis,
    input: &AssemblyScaleBasis,
) -> Result<(), AssemblyScaleCompatibilityError> {
    require_assembly_scale_compatibility_inner(
        base,
        &AssemblyScaleSelectorIdentity::Indexed,
        input,
        &AssemblyScaleSelectorIdentity::Indexed,
    )
}

/// Require two core-validated compatibility bases to agree.
///
/// # Errors
///
/// Returns the first stable mismatch category, including selector-mode or
/// exact named-selector disagreement.
pub fn require_assembly_scale_compatibility_with_selectors(
    base: &AssemblyScaleCompatibilityBasis,
    input: &AssemblyScaleCompatibilityBasis,
) -> Result<(), AssemblyScaleCompatibilityError> {
    require_assembly_scale_compatibility_inner(
        &base.basis,
        &base.selector,
        &input.basis,
        &input.selector,
    )
}

fn require_assembly_scale_compatibility_inner(
    base: &AssemblyScaleBasis,
    base_selector: &AssemblyScaleSelectorIdentity,
    input: &AssemblyScaleBasis,
    input_selector: &AssemblyScaleSelectorIdentity,
) -> Result<(), AssemblyScaleCompatibilityError> {
    let tolerance = ScaleTolerancePolicy::APPENDIX_D_V6;
    let named_selectors = match (base_selector, input_selector) {
        (AssemblyScaleSelectorIdentity::Indexed, AssemblyScaleSelectorIdentity::Indexed) => None,
        (
            AssemblyScaleSelectorIdentity::Named {
                root_node_name: base_root,
                skin_joint_names: base_joints,
            },
            AssemblyScaleSelectorIdentity::Named {
                root_node_name: input_root,
                skin_joint_names: input_joints,
            },
        ) => Some((base_root, base_joints, input_root, input_joints)),
        _ => {
            return Err(AssemblyScaleCompatibilityError {
                reason: "source-selector-mode",
            });
        }
    };
    let mismatch = if base.version != input.version {
        Some("basis-version")
    } else if base.coordinate_convention != input.coordinate_convention {
        Some("coordinate-convention")
    } else if base.tolerance_policy_id != input.tolerance_policy_id
        || base.tolerance_policy_id != tolerance.id
    {
        Some("tolerance-policy")
    } else if named_selectors.is_none() && base.source_skin_index != input.source_skin_index {
        Some("source-skin-selector")
    } else if named_selectors.is_none()
        && base.source_root_node_index != input.source_root_node_index
    {
        Some("source-root-selector")
    } else if named_selectors.is_some_and(|(base_root, base_joints, input_root, input_joints)| {
        base_root != input_root || base_joints != input_joints
    }) {
        Some("source-name-selector")
    } else if base.expected_factor_bits != input.expected_factor_bits {
        Some("expected-factor")
    } else if !same_named_topology(&base.named_nodes, &input.named_nodes) {
        Some("named-topology")
    } else if !same_named_rest(&base.named_nodes, &input.named_nodes, &tolerance) {
        Some("named-rest-basis")
    } else if !same_named_orientations(&base.named_nodes, &input.named_nodes, &tolerance) {
        Some("named-orientation")
    } else if (named_selectors.is_some()
        && !same_named_source_layout(&base.source_nodes, &input.source_nodes))
        || (named_selectors.is_none()
            && !same_source_layout(&base.source_nodes, &input.source_nodes))
    {
        Some("source-helper-layout")
    } else if (named_selectors.is_some()
        && !same_named_source_rest(&base.source_nodes, &input.source_nodes, &tolerance))
        || (named_selectors.is_none()
            && !same_source_rest(&base.source_nodes, &input.source_nodes, &tolerance))
    {
        Some("source-helper-rest-basis")
    } else {
        None
    };
    mismatch.map_or(Ok(()), |reason| {
        Err(AssemblyScaleCompatibilityError { reason })
    })
}

fn same_named_topology(base: &[AssemblyScaleNamedNode], input: &[AssemblyScaleNamedNode]) -> bool {
    base.len() == input.len()
        && base
            .iter()
            .zip(input)
            .all(|(base, input)| base.name == input.name && base.parent == input.parent)
}

fn same_named_rest(
    base: &[AssemblyScaleNamedNode],
    input: &[AssemblyScaleNamedNode],
    tolerance: &ScaleTolerancePolicy,
) -> bool {
    base.iter().zip(input).all(|(base, input)| {
        close_f32_bits(&base.translation_bits, &input.translation_bits, tolerance)
            && close_f32_bits(&base.scale_bits, &input.scale_bits, tolerance)
    })
}

fn same_named_orientations(
    base: &[AssemblyScaleNamedNode],
    input: &[AssemblyScaleNamedNode],
    tolerance: &ScaleTolerancePolicy,
) -> bool {
    base.iter()
        .zip(input)
        .all(|(base, input)| same_quaternion(&base.rotation_bits, &input.rotation_bits, tolerance))
}

fn same_source_layout(base: &[AssemblyScaleSourceNode], input: &[AssemblyScaleSourceNode]) -> bool {
    base.len() == input.len()
        && base.iter().zip(input).all(|(base, input)| {
            base.source_node_index == input.source_node_index
                && base.parent_source_node_index == input.parent_source_node_index
                && base.name == input.name
                && base.role == input.role
                && std::mem::discriminant(&base.local_rest)
                    == std::mem::discriminant(&input.local_rest)
        })
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct NamedSourcePath(Vec<(Option<String>, String, bool)>);

fn named_source_paths(nodes: &[AssemblyScaleSourceNode]) -> Option<Vec<NamedSourcePath>> {
    let by_index = nodes
        .iter()
        .enumerate()
        .map(|(position, node)| (node.source_node_index, position))
        .collect::<std::collections::BTreeMap<_, _>>();
    nodes
        .iter()
        .map(|node| {
            let mut path = Vec::new();
            let mut current = Some(node.source_node_index);
            for _ in 0..=nodes.len() {
                let Some(index) = current else {
                    path.reverse();
                    return Some(NamedSourcePath(path));
                };
                let row = nodes.get(*by_index.get(&index)?)?;
                path.push((
                    row.name.clone(),
                    row.role.clone(),
                    matches!(&row.local_rest, AssemblyScaleSourceRest::Matrix { .. }),
                ));
                current = row.parent_source_node_index;
            }
            None
        })
        .collect()
}

fn same_named_source_layout(
    base: &[AssemblyScaleSourceNode],
    input: &[AssemblyScaleSourceNode],
) -> bool {
    let (Some(mut base), Some(mut input)) = (named_source_paths(base), named_source_paths(input))
    else {
        return false;
    };
    base.sort();
    input.sort();
    base == input
}

fn same_named_source_rest(
    base: &[AssemblyScaleSourceNode],
    input: &[AssemblyScaleSourceNode],
    tolerance: &ScaleTolerancePolicy,
) -> bool {
    let (Some(base_paths), Some(input_paths)) =
        (named_source_paths(base), named_source_paths(input))
    else {
        return false;
    };
    let mut matched = vec![false; input.len()];
    base.iter().zip(base_paths).all(|(base_node, base_path)| {
        input
            .iter()
            .zip(&input_paths)
            .enumerate()
            .find(|(index, (input_node, input_path))| {
                !matched[*index]
                    && **input_path == base_path
                    && same_source_rest_node(base_node, input_node, tolerance)
            })
            .is_some_and(|(index, _)| {
                matched[index] = true;
                true
            })
    })
}

fn same_source_rest_node(
    base: &AssemblyScaleSourceNode,
    input: &AssemblyScaleSourceNode,
    tolerance: &ScaleTolerancePolicy,
) -> bool {
    same_source_rest(
        std::slice::from_ref(base),
        std::slice::from_ref(input),
        tolerance,
    )
}

fn same_source_rest(
    base: &[AssemblyScaleSourceNode],
    input: &[AssemblyScaleSourceNode],
    tolerance: &ScaleTolerancePolicy,
) -> bool {
    base.iter().zip(input).all(
        |(base, input)| match (&base.local_rest, &input.local_rest) {
            (
                AssemblyScaleSourceRest::Trs {
                    translation_bits: base_translation,
                    rotation_bits: base_rotation,
                    scale_bits: base_scale,
                },
                AssemblyScaleSourceRest::Trs {
                    translation_bits: input_translation,
                    rotation_bits: input_rotation,
                    scale_bits: input_scale,
                },
            ) => {
                close_f32_bits(base_translation, input_translation, tolerance)
                    && close_f32_bits(base_scale, input_scale, tolerance)
                    && same_quaternion(base_rotation, input_rotation, tolerance)
            }
            (
                AssemblyScaleSourceRest::Matrix {
                    matrix_bits: base_matrix,
                },
                AssemblyScaleSourceRest::Matrix {
                    matrix_bits: input_matrix,
                },
            ) => close_f32_bits(base_matrix, input_matrix, tolerance),
            _ => false,
        },
    )
}

fn close_f32_bits<const N: usize>(
    base: &[u32; N],
    input: &[u32; N],
    tolerance: &ScaleTolerancePolicy,
) -> bool {
    base.iter().zip(input).all(|(&base, &input)| {
        close_f64(
            f32::from_bits(base) as f64,
            f32::from_bits(input) as f64,
            tolerance,
        )
    })
}

fn close_f64(base: f64, input: f64, tolerance: &ScaleTolerancePolicy) -> bool {
    base.is_finite()
        && input.is_finite()
        && (base - input).abs()
            <= tolerance.scalar_absolute + tolerance.scalar_relative * base.abs().max(input.abs())
}

fn same_quaternion(base: &[u32; 4], input: &[u32; 4], tolerance: &ScaleTolerancePolicy) -> bool {
    let base = base.map(|bits| f32::from_bits(bits) as f64);
    let input = input.map(|bits| f32::from_bits(bits) as f64);
    if !base
        .iter()
        .chain(input.iter())
        .all(|value| value.is_finite())
    {
        return false;
    }
    let base_norm = base.iter().map(|value| value * value).sum::<f64>().sqrt();
    let input_norm = input.iter().map(|value| value * value).sum::<f64>().sqrt();
    if base_norm == 0.0 || input_norm == 0.0 {
        return false;
    }
    let dot = base
        .iter()
        .zip(input)
        .map(|(base, input)| base * input)
        .sum::<f64>()
        / (base_norm * input_norm);
    2.0 * dot.abs().clamp(-1.0, 1.0).acos() <= tolerance.rotation_residual_radians
}
