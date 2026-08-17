//! Versioned semantic basis projected from an accepted scale plan.

use super::{ScaleError, ScaleOperation, ScalePlan, ScaleProjectedRole, ScaleSourceNodeKind};
use crate::model::{Document, SourceNodeLocalRest};
use serde::Serialize;
use std::collections::BTreeSet;

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
    /// Exact column-major authored local matrix bits.
    pub local_matrix_bits: [u32; 16],
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

/// Why two independently supplied assembly inputs do not share one basis.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("assembly scale basis mismatch ({reason})")]
pub struct AssemblyScaleCompatibilityError {
    /// Stable machine-readable mismatch reason.
    pub reason: &'static str,
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
        let matrix = match source.local_rest {
            SourceNodeLocalRest::Trs {
                translation,
                rotation,
                scale,
            } => glam::Mat4::from_scale_rotation_translation(scale, rotation, translation),
            SourceNodeLocalRest::Matrix(matrix) => matrix,
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
            local_matrix_bits: matrix.to_cols_array().map(f32::to_bits),
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
        source_skin_index,
        source_root_node_index,
        expected_factor_bits: expected_factor.to_bits(),
        named_nodes,
        source_nodes,
        target_paths,
    })
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
    let mismatch = if base.version != input.version {
        Some("basis-version")
    } else if base.coordinate_convention != input.coordinate_convention {
        Some("coordinate-convention")
    } else if base.source_skin_index != input.source_skin_index {
        Some("source-skin-selector")
    } else if base.source_root_node_index != input.source_root_node_index {
        Some("source-root-selector")
    } else if base.expected_factor_bits != input.expected_factor_bits {
        Some("expected-factor")
    } else if base.named_nodes != input.named_nodes {
        Some("named-topology-rest-orientation")
    } else if base.source_nodes != input.source_nodes {
        Some("source-helper-layout")
    } else {
        None
    };
    mismatch.map_or(Ok(()), |reason| {
        Err(AssemblyScaleCompatibilityError { reason })
    })
}
