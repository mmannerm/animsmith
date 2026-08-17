//! Private validation, topology, and format-neutral structural readers.
//!
//! This module owns scale-input validation, candidate structural parity,
//! canonical source-domain derivation, world-pose readers, and inverse-bind
//! resolution. It carries no operation planner, compiled-ledger construction,
//! candidate rewrite, or proof expectation.

use super::numeric::translation_composition_rounding_base;
use super::{
    ScaleError, ScalePayloadShapeRow, ScaleProjectedRole, ScaleSourceNodeKind,
    ScaleSourceTopologyRow,
};
use crate::model::{
    BoneId, Document, MeshInstance, Skeleton, SourceInverseBindAccessorStatus, SourceNodeAsset,
    SourceNodeLocalRest, SourceSkeletonCoverage, SourceSkinAsset, mat4_is_finite,
    validate_document_shape as validate_model_document_shape, world_rest_matrices,
};
use glam::{DMat4, Mat4};
#[cfg(test)]
use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};

/// Validate every public scale-input snapshot before reading or rewriting it.
/// Shared structure is delegated to [`crate::model::validate_document_shape`];
/// this adapter owns only scale's finite base-position and nonnegative primary
/// skin-weight requirements. [`super::plan_scale`], reference candidate construction,
/// and [`super::prove_scale`] all call it, including for generated and externally
/// loaded candidates, because a mutable [`Document`] cannot carry a durable
/// validation guarantee.
///
/// The nonnegative sign scan for every stored primary weight happens here.
/// Per-vertex primitive skinning shape (`joints`/`weights` parallel to
/// `positions`) is a separate check: it is validated directly where those
/// arrays are walked, in [`super::proof::accumulate_skinned_bounds`], since
/// only affected instances' geometry needs shape-parity validation.
pub(in crate::scale) fn validate_scale_input(document: &Document) -> Result<(), ScaleError> {
    validate_model_document_shape(document)?;
    for (mesh_index, mesh) in document.assets.meshes.iter().enumerate() {
        for (primitive_index, primitive) in mesh.primitives.iter().enumerate() {
            if primitive
                .positions
                .iter()
                .any(|position| !position.is_finite())
            {
                return Err(ScaleError::InvalidMeshPrimitive {
                    mesh_index,
                    primitive_index,
                    reason: "non_finite_position",
                });
            }
            for (vertex_index, weights) in primitive.weights.iter().enumerate() {
                for (influence_index, &weight) in weights.iter().enumerate() {
                    if weight.is_finite() && weight < 0.0 {
                        return Err(ScaleError::NegativeSkinWeight {
                            mesh_index,
                            primitive_index,
                            vertex_index,
                            influence_index,
                        });
                    }
                }
            }
        }
    }
    Ok(())
}

pub(in crate::scale) fn validate_candidate_structure(
    source: &Document,
    candidate: &Document,
) -> Result<(), ScaleError> {
    // Bone-count parity is checked first, and it is the clause the sampling
    // budget rests on. [`sample_time_obligations`] poses *both* skeletons at
    // every sample time, but [`per_sample_work_units`] can only measure the
    // source — it is called before any candidate is walked, and
    // [`ScaleCandidate::from_document`] is public, so the candidate's bone
    // count is caller-supplied. Charging `PROOF_SIDES * bones(source)` is
    // therefore only an honest charge once the two counts are known equal:
    // without this clause a two-bone source paired with a candidate padded to
    // 60_000 identity bones was charged 36_000 units — 0.009% of the budget —
    // and proved in 3.71s release, more than twice the wall time of the
    // vertex-dominated document the budget scores at 100%. Parity is the fix
    // rather than charging both
    // sides because it is the same class of claim as the mesh and skin-joint
    // clauses below ("unchanged skeleton/mesh/skin identity", DESIGN.md
    // Appendix D §D.6), and it makes the single-side charge correct by
    // construction rather than merely conservative.
    if source.skeleton.bones.len() != candidate.skeleton.bones.len() {
        return Err(ScaleError::CandidateStructureMismatch {
            reason: "bone_count_mismatch",
        });
    }
    // Topology is outside both operations' write sets. Compare it globally,
    // not only outside a rest/bind closure: whole-document conversion affects
    // every bone but still never re-parents one, and an affected leaf can be
    // coherently moved between parents whose worlds happen to agree.
    //
    // The raw source-node rows are keyed by source identity rather than
    // zipped or indexed as BoneIds. A loader is free to normalize a wide
    // source hierarchy into parent-before-child DFS order, so raw node-array
    // order and normalized bone order are not interchangeable. Requiring the
    // coverage and, when coverage is Complete, the `(raw parent, projected
    // bone)` map to agree also prevents a candidate from making a coherent
    // skeleton re-parent look valid by rewriting its own projection to match.
    // Rows under Unavailable coverage are deliberately ignored: the model
    // does not claim that they are identity evidence.
    let complete_projection_differs = source.assets.source_skeleton.coverage
        == SourceSkeletonCoverage::Complete
        && candidate.assets.source_skeleton.coverage == SourceSkeletonCoverage::Complete
        && source_node_projection(source) != source_node_projection(candidate);
    if source
        .skeleton
        .bones
        .iter()
        .map(|bone| bone.parent)
        .ne(candidate.skeleton.bones.iter().map(|bone| bone.parent))
        || source.assets.source_skeleton.coverage != candidate.assets.source_skeleton.coverage
        || complete_projection_differs
    {
        return Err(ScaleError::CandidateStructureMismatch {
            reason: "skeleton_topology_mismatch",
        });
    }
    // Source skins are raw identity/evidence sidecars that neither operation
    // rewrites. Compare their numeric-free semantic rows in the documented
    // stable source-skin order: a frontend may not add, remove, reorder,
    // retarget, or reshape one and still satisfy the exact-payload claim.
    if source_skin_payload_shapes(source) != source_skin_payload_shapes(candidate) {
        return Err(ScaleError::CandidateStructureMismatch {
            reason: "source_skin_payload_mismatch",
        });
    }
    if source.clips.len() != candidate.clips.len() {
        return Err(ScaleError::CandidateStructureMismatch {
            reason: "clip_count_mismatch",
        });
    }
    for (source_clip, candidate_clip) in source.clips.iter().zip(candidate.clips.iter()) {
        if source_clip.tracks.len() != candidate_clip.tracks.len() {
            return Err(ScaleError::CandidateStructureMismatch {
                reason: "track_count_mismatch",
            });
        }
        for (source_track, candidate_track) in
            source_clip.tracks.iter().zip(candidate_clip.tracks.iter())
        {
            if source_track.bone != candidate_track.bone
                || source_track.property != candidate_track.property
                || source_track.interpolation != candidate_track.interpolation
                || source_track.times != candidate_track.times
                || source_track.values.len() != candidate_track.values.len()
            {
                return Err(ScaleError::CandidateStructureMismatch {
                    reason: "track_shape_mismatch",
                });
            }
        }
    }
    if source.assets.instances.len() != candidate.assets.instances.len() {
        return Err(ScaleError::CandidateStructureMismatch {
            reason: "instance_count_mismatch",
        });
    }
    // Mesh assignment and skin-joint identity are what DESIGN.md Appendix D
    // §D.6 calls "unchanged mesh/material/skin identity": neither operation
    // rewrites which mesh an instance draws or which joints it is bound to.
    // Checking it here rather than inferring it from a residual is also what
    // lets the skin and bounds obligations share one instance/vertex walk —
    // the two sides are then known to have the same slots, the same mesh, and
    // therefore the same per-primitive vertex counts.
    //
    // `node` and `source_node_index` are the *placement* half of that same
    // identity, and neither is reachable from any residual. `node` is what
    // attaches the instance to a bone — holding the skeleton fixed, it is what
    // positions an unskinned prop, so moving one from a bone outside the
    // affected closure onto a rebased one relocates it in world space while
    // every residual this module measures stays exactly zero.
    // `source_node_index` is what [`instance_source_skin`] matches attachments
    // against, so re-pointing it changes whether a missing bind resolves to
    // glTF's format-defined identity default or is refused as missing
    // evidence.
    //
    // The topology comparison above now supplies that fixed skeleton, and
    // `prove_scale` compares the complete derived world rest of every bone
    // outside a rest/bind closure exactly. Together they prove placement as
    // well as instance identity: neither a coherent re-parent nor a direct
    // rest mutation of an independent sibling/leaf can relocate the prop.
    // Comparing instances positionally also fixes instance *order*: two
    // instances identical in every payload but attached to different nodes
    // cannot be swapped without one of these two clauses firing.
    for (instance, candidate_instance) in source
        .assets
        .instances
        .iter()
        .zip(candidate.assets.instances.iter())
    {
        if instance.node != candidate_instance.node {
            return Err(ScaleError::CandidateStructureMismatch {
                reason: "instance_node_mismatch",
            });
        }
        if instance.source_node_index != candidate_instance.source_node_index {
            return Err(ScaleError::CandidateStructureMismatch {
                reason: "instance_source_node_index_mismatch",
            });
        }
        if instance.mesh != candidate_instance.mesh {
            return Err(ScaleError::CandidateStructureMismatch {
                reason: "instance_mesh_mismatch",
            });
        }
        if instance.skin_joints != candidate_instance.skin_joints {
            return Err(ScaleError::CandidateStructureMismatch {
                reason: "instance_skin_joints_mismatch",
            });
        }
    }
    if source.assets.meshes.len() != candidate.assets.meshes.len() {
        return Err(ScaleError::CandidateStructureMismatch {
            reason: "mesh_count_mismatch",
        });
    }
    for (source_mesh, candidate_mesh) in source
        .assets
        .meshes
        .iter()
        .zip(candidate.assets.meshes.iter())
    {
        if source_mesh.source_mesh_index != candidate_mesh.source_mesh_index {
            return Err(ScaleError::CandidateStructureMismatch {
                reason: "mesh_source_identity_mismatch",
            });
        }
        if source_mesh.primitives.len() != candidate_mesh.primitives.len() {
            return Err(ScaleError::CandidateStructureMismatch {
                reason: "primitive_count_mismatch",
            });
        }
        for (source_primitive, candidate_primitive) in source_mesh
            .primitives
            .iter()
            .zip(candidate_mesh.primitives.iter())
        {
            if source_primitive.positions.len() != candidate_primitive.positions.len() {
                return Err(ScaleError::CandidateStructureMismatch {
                    reason: "primitive_vertex_count_mismatch",
                });
            }
            if source_primitive.normals.len() != candidate_primitive.normals.len() {
                return Err(ScaleError::CandidateStructureMismatch {
                    reason: "primitive_normal_count_mismatch",
                });
            }
            if source_primitive.joints.len() != candidate_primitive.joints.len() {
                return Err(ScaleError::CandidateStructureMismatch {
                    reason: "primitive_joint_count_mismatch",
                });
            }
            if source_primitive.weights.len() != candidate_primitive.weights.len() {
                return Err(ScaleError::CandidateStructureMismatch {
                    reason: "primitive_weight_count_mismatch",
                });
            }
        }
    }
    Ok(())
}

fn source_node_projection(document: &Document) -> BTreeMap<usize, (Option<usize>, Option<BoneId>)> {
    document
        .assets
        .source_skeleton
        .nodes
        .iter()
        .map(|node| {
            (
                node.source_node_index,
                (node.parent_source_node_index, node.bone),
            )
        })
        .collect()
}

pub(in crate::scale) fn source_skin_payload_shapes(
    document: &Document,
) -> Vec<ScalePayloadShapeRow> {
    if document.assets.source_skeleton.coverage != SourceSkeletonCoverage::Complete {
        return Vec::new();
    }
    let mut rows = Vec::new();
    for skin in &document.assets.source_skeleton.skins {
        rows.push(ScalePayloadShapeRow::SourceSkin {
            source_skin_index: skin.source_skin_index,
            skeleton_root_source_node_index: skin.skeleton_root_source_node_index,
            joint_count: skin.joint_source_node_indices.len(),
            attachment_count: skin.attachments.len(),
            inverse_bind_status: skin.inverse_bind_accessor.status,
            inverse_bind_declared_count: skin.inverse_bind_accessor.declared_count,
            inverse_bind_matrix_count: skin.inverse_bind_accessor.matrices.len(),
        });
        rows.extend(skin.joint_source_node_indices.iter().enumerate().map(
            |(slot, &source_node_index)| ScalePayloadShapeRow::SourceSkinJoint {
                source_skin_index: skin.source_skin_index,
                slot,
                source_node_index,
            },
        ));
        rows.extend(
            skin.attachments
                .iter()
                .enumerate()
                .map(
                    |(attachment_index, attachment)| ScalePayloadShapeRow::SourceSkinAttachment {
                        source_skin_index: skin.source_skin_index,
                        attachment_index,
                        source_node_index: attachment.source_node_index,
                        source_mesh_index: attachment.source_mesh_index,
                    },
                ),
        );
    }
    rows
}

pub(in crate::scale) fn whole_document_source_topology(
    document: &Document,
) -> Vec<ScaleSourceTopologyRow> {
    if document.assets.source_skeleton.coverage != SourceSkeletonCoverage::Complete {
        return Vec::new();
    }
    let mut rows: Vec<_> = document
        .assets
        .source_skeleton
        .nodes
        .iter()
        .map(|node| ScaleSourceTopologyRow {
            source_node_index: node.source_node_index,
            parent_source_node_index: node.parent_source_node_index,
            kind: ScaleSourceNodeKind::OutsideDomain { bone: node.bone },
        })
        .collect();
    rows.sort_unstable_by_key(|row| row.source_node_index);
    rows
}

pub(in crate::scale) struct RestBindTopology {
    pub(in crate::scale) source_rows: Vec<ScaleSourceTopologyRow>,
    pub(in crate::scale) scaled_root_bone: BoneId,
    #[cfg(test)]
    pub(in crate::scale) ancestry_steps: usize,
}

impl RestBindTopology {
    fn projected_rows(
        &self,
    ) -> impl Iterator<
        Item = (
            &ScaleSourceTopologyRow,
            BoneId,
            ScaleProjectedRole,
            Option<usize>,
        ),
    > {
        self.source_rows.iter().filter_map(|row| match row.kind {
            ScaleSourceNodeKind::Projected {
                bone,
                role,
                incoming_connector_tail,
                ..
            } => Some((row, bone, role, incoming_connector_tail)),
            ScaleSourceNodeKind::Connector | ScaleSourceNodeKind::OutsideDomain { .. } => None,
        })
    }

    pub(in crate::scale) fn bone_of_source(&self) -> BTreeMap<usize, BoneId> {
        self.projected_rows()
            .map(|(row, bone, _, _)| (row.source_node_index, bone))
            .collect()
    }

    pub(in crate::scale) fn affected_nodes(&self) -> Vec<BoneId> {
        let mut nodes: Vec<_> = self.projected_rows().map(|(_, bone, _, _)| bone).collect();
        nodes.sort_unstable();
        nodes.dedup();
        nodes
    }

    pub(in crate::scale) fn transform_only_attachments(&self) -> Vec<BoneId> {
        let mut nodes: Vec<_> = self
            .projected_rows()
            .filter_map(|(_, bone, role, _)| {
                (role == ScaleProjectedRole::TransformOnly).then_some(bone)
            })
            .collect();
        nodes.sort_unstable();
        nodes
    }

    pub(in crate::scale) fn connector_sources(&self) -> BTreeSet<usize> {
        self.source_rows
            .iter()
            .filter_map(|row| {
                matches!(row.kind, ScaleSourceNodeKind::Connector).then_some(row.source_node_index)
            })
            .collect()
    }
}

/// Memoized raw-source ancestry needed by the rest/bind topology table.
///
/// `projected_parent_by_source` names the nearest projected ancestor of each
/// projected domain row. `connector_tail_by_source` names an immediate
/// unprojected parent when that edge enters a connector span, and
/// `used_connectors` is the union of those spans. All ancestor walks share a
/// path-compressed cache, so a long connector chain shared by many projected
/// successors is traversed once rather than once per successor.
struct RestBindSourceAncestry {
    projected_parent_by_source: BTreeMap<usize, Option<usize>>,
    connector_tail_by_source: BTreeMap<usize, usize>,
    used_connectors: BTreeSet<usize>,
    #[cfg(test)]
    ancestry_steps: usize,
}

fn derive_rest_bind_source_ancestry(
    by_source_index: &BTreeMap<usize, &SourceNodeAsset>,
    source_nodes: &BTreeSet<usize>,
    projected_sources: impl Iterator<Item = usize>,
    source_root_node_index: usize,
) -> Result<RestBindSourceAncestry, ScaleError> {
    let projected_sources: Vec<_> = projected_sources.collect();
    let mut nearest_cache: BTreeMap<usize, Option<usize>> = BTreeMap::new();
    let mut projected_parent_by_source = BTreeMap::new();
    let mut connector_tail_by_source = BTreeMap::new();
    #[cfg(test)]
    let mut ancestry_steps = 0;

    for source in projected_sources {
        let asset = by_source_index
            .get(&source)
            .ok_or(ScaleError::IncompleteClosure {
                reason: "dangling_source_parent_node_index",
            })?;
        if source != source_root_node_index
            && let Some(parent) = asset.parent_source_node_index.filter(|parent| {
                source_nodes.contains(parent)
                    && by_source_index
                        .get(parent)
                        .is_some_and(|parent| parent.bone.is_none())
            })
        {
            connector_tail_by_source.insert(source, parent);
        }

        let mut cursor = asset.parent_source_node_index;
        let mut path = Vec::new();
        let mut visiting = BTreeSet::new();
        let projected_parent = loop {
            let Some(parent) = cursor else {
                break None;
            };
            if let Some(&cached) = nearest_cache.get(&parent) {
                break cached;
            }
            if !visiting.insert(parent) || visiting.len() > by_source_index.len() {
                return Err(ScaleError::IncompleteClosure {
                    reason: "cyclic_or_unbounded_source_parent_chain",
                });
            }
            let parent_asset =
                by_source_index
                    .get(&parent)
                    .ok_or(ScaleError::IncompleteClosure {
                        reason: "dangling_source_parent_node_index",
                    })?;
            #[cfg(test)]
            {
                ancestry_steps += 1;
            }
            if parent_asset.bone.is_some() {
                break Some(parent);
            }
            path.push(parent);
            cursor = parent_asset.parent_source_node_index;
        };
        for connector in path {
            nearest_cache.insert(connector, projected_parent);
        }
        projected_parent_by_source.insert(source, projected_parent);
    }

    let mut used_connectors = BTreeSet::new();
    let mut pending: Vec<usize> = connector_tail_by_source.values().copied().collect();
    while let Some(source) = pending.pop() {
        if !used_connectors.insert(source) {
            continue;
        }
        let asset = by_source_index
            .get(&source)
            .ok_or(ScaleError::IncompleteClosure {
                reason: "dangling_source_parent_node_index",
            })?;
        if let Some(parent) = asset.parent_source_node_index.filter(|parent| {
            source_nodes.contains(parent)
                && by_source_index
                    .get(parent)
                    .is_some_and(|parent| parent.bone.is_none())
        }) {
            pending.push(parent);
        }
    }

    Ok(RestBindSourceAncestry {
        projected_parent_by_source,
        connector_tail_by_source,
        used_connectors,
        #[cfg(test)]
        ancestry_steps,
    })
}

/// Resolve the selector-derived rest/bind domain without classifying its
/// numeric affine factors.
///
/// Planning uses both halves. Candidate construction and proof reuse only
/// this half to bind a replayed plan to the supplied source's current write
/// and evidence inventory while keeping proof's observed-factor witness
/// independent of planning's numeric acceptance band.
pub(in crate::scale) fn derive_rest_bind_plan_domain(
    document: &Document,
    source_skin_index: usize,
    source_root_node_index: usize,
) -> Result<RestBindTopology, ScaleError> {
    if document.assets.source_skeleton.coverage != SourceSkeletonCoverage::Complete {
        return Err(ScaleError::IncompleteSourceSkeleton);
    }
    let skin = resolve_rest_bind_skin(document, source_skin_index)?;
    let by_source_index = source_node_index_map(document);
    if !by_source_index.contains_key(&source_root_node_index) {
        return Err(ScaleError::InvalidRootSelector {
            source_root_node_index,
        });
    }

    let source_nodes =
        rest_bind_affected_closure(document, &by_source_index, skin, source_root_node_index)?;

    let scaled_root_bone = by_source_index[&source_root_node_index].bone.ok_or(
        ScaleError::SourceNodeNotNormalized {
            source_node_index: source_root_node_index,
        },
    )?;
    for &joint in &skin.joint_source_node_indices {
        if by_source_index[&joint].bone.is_none() {
            return Err(ScaleError::SourceNodeNotNormalized {
                source_node_index: joint,
            });
        }
    }

    let bone_of_source: BTreeMap<usize, BoneId> = source_nodes
        .iter()
        .filter_map(|source| by_source_index[source].bone.map(|bone| (*source, bone)))
        .collect();

    // A source row without a normalized bone is admitted only as a strict
    // connector between two projected rows. Record only each projected
    // successor's immediate connector tail. Candidate construction memoizes
    // the ordered ancestor product per connector row, keeping both storage and
    // work linear when many projected successors share a long connector chain.
    let ancestry = derive_rest_bind_source_ancestry(
        &by_source_index,
        &source_nodes,
        bone_of_source.keys().copied(),
        source_root_node_index,
    )?;
    if ancestry.used_connectors.iter().any(|source| {
        let SourceNodeLocalRest::Matrix(matrix) = &by_source_index[source].local_rest else {
            return false;
        };
        matrix.x_axis.w != 0.0
            || matrix.y_axis.w != 0.0
            || matrix.z_axis.w != 0.0
            || matrix.w_axis.w != 1.0
    }) {
        return Err(ScaleError::IncompleteClosure {
            reason: "non_affine_connector_source_transform",
        });
    }
    if let Some(&terminal) = source_nodes.iter().find(|source| {
        by_source_index[source].bone.is_none() && !ancestry.used_connectors.contains(source)
    }) {
        return Err(ScaleError::SourceNodeNotNormalized {
            source_node_index: terminal,
        });
    }

    let joint_sources: BTreeSet<usize> = skin.joint_source_node_indices.iter().copied().collect();
    let mut source_rows = Vec::with_capacity(document.assets.source_skeleton.nodes.len());
    for asset in &document.assets.source_skeleton.nodes {
        let source_node_index = asset.source_node_index;
        let kind = if !source_nodes.contains(&source_node_index) {
            ScaleSourceNodeKind::OutsideDomain { bone: asset.bone }
        } else {
            match asset.bone {
                Some(bone) => {
                    let role = if source_node_index == source_root_node_index {
                        ScaleProjectedRole::Root
                    } else if joint_sources.contains(&source_node_index) {
                        ScaleProjectedRole::Joint
                    } else {
                        ScaleProjectedRole::TransformOnly
                    };
                    let projected_parent = *ancestry
                        .projected_parent_by_source
                        .get(&source_node_index)
                        .ok_or(ScaleError::IncompleteClosure {
                            reason: "projected_source_ancestry_missing",
                        })?;
                    ScaleSourceNodeKind::Projected {
                        bone,
                        role,
                        projected_parent,
                        incoming_connector_tail: ancestry
                            .connector_tail_by_source
                            .get(&source_node_index)
                            .copied(),
                    }
                }
                None => ScaleSourceNodeKind::Connector,
            }
        };
        source_rows.push(ScaleSourceTopologyRow {
            source_node_index,
            parent_source_node_index: asset.parent_source_node_index,
            kind,
        });
    }
    source_rows.sort_unstable_by_key(|row| row.source_node_index);

    Ok(RestBindTopology {
        source_rows,
        scaled_root_bone,
        #[cfg(test)]
        ancestry_steps: ancestry.ancestry_steps,
    })
}

/// Resolve `source_skin_index` against
/// `document.assets.source_skeleton.skins` by
/// [`SourceSkinAsset::source_skin_index`] — never by raw array position,
/// since a loader's source-skin indices need not be dense or contiguous.
pub(in crate::scale) fn resolve_rest_bind_skin(
    document: &Document,
    source_skin_index: usize,
) -> Result<&SourceSkinAsset, ScaleError> {
    let skin = document
        .assets
        .source_skeleton
        .skins
        .iter()
        .find(|skin| skin.source_skin_index == source_skin_index)
        .ok_or(ScaleError::InvalidSkinSelector { source_skin_index })?;
    if skin.joint_source_node_indices.is_empty() {
        return Err(ScaleError::InvalidSkinSelector { source_skin_index });
    }
    Ok(skin)
}

pub(in crate::scale) fn source_node_index_map(
    document: &Document,
) -> BTreeMap<usize, &SourceNodeAsset> {
    document
        .assets
        .source_skeleton
        .nodes
        .iter()
        .map(|node| (node.source_node_index, node))
        .collect()
}

/// Compute the closed connected hierarchy of DESIGN.md Appendix D §D.2 in
/// raw source-node identity space: the scaled ancestor, every selected skin
/// joint and the paths between them, and every descendant whose attachment
/// transform would otherwise inherit the common factor.
pub(in crate::scale) fn rest_bind_affected_closure(
    document: &Document,
    by_source_index: &BTreeMap<usize, &SourceNodeAsset>,
    skin: &SourceSkinAsset,
    source_root_node_index: usize,
) -> Result<BTreeSet<usize>, ScaleError> {
    // Built up front so every insertion below — root, joint, ancestor-path
    // helper, or later descendant — is checked against the same evidence,
    // rather than only the descendants a later BFS happens to visit.
    let unskinned_attachment: BTreeSet<usize> = document
        .assets
        .instances
        .iter()
        .filter(|instance| instance.skin_joints.is_empty())
        .map(|instance| instance.source_node_index)
        .collect();
    let reject_if_unskinned = |source_node_index: usize| -> Result<(), ScaleError> {
        if !unskinned_attachment.contains(&source_node_index) {
            return Ok(());
        }
        let bone = by_source_index
            .get(&source_node_index)
            .and_then(|asset| asset.bone)
            .ok_or(ScaleError::SourceNodeNotNormalized { source_node_index })?;
        Err(ScaleError::UnsupportedUnskinnedGeometry { node: bone })
    };

    let mut domain = BTreeSet::new();
    let mut reaches_root = BTreeSet::from([source_root_node_index]);
    reject_if_unskinned(source_root_node_index)?;
    domain.insert(source_root_node_index);
    for &joint in &skin.joint_source_node_indices {
        if !by_source_index.contains_key(&joint) {
            return Err(ScaleError::IncompleteClosure {
                reason: "skin_joint_source_node_missing",
            });
        }
        reject_if_unskinned(joint)?;
        let mut cursor = joint;
        let mut pending = Vec::new();
        let mut visiting = BTreeSet::new();
        loop {
            if reaches_root.contains(&cursor) {
                break;
            }
            if !visiting.insert(cursor) || visiting.len() > by_source_index.len() {
                return Err(ScaleError::IncompleteClosure {
                    reason: "cyclic_or_unbounded_source_parent_chain",
                });
            }
            // `cursor` was itself reached via a parent link from a node
            // already confirmed present, but the parent it names need not
            // be: a dangling `parent_source_node_index` must fail closed
            // with a typed error rather than panic on this index.
            let asset = *by_source_index
                .get(&cursor)
                .ok_or(ScaleError::IncompleteClosure {
                    reason: "dangling_source_parent_node_index",
                })?;
            pending.push(cursor);
            match asset.parent_source_node_index {
                Some(parent) => {
                    reject_if_unskinned(parent)?;
                    cursor = parent;
                }
                None => {
                    return Err(ScaleError::IncompleteClosure {
                        reason: "joint_not_descendant_of_scaled_root",
                    });
                }
            }
        }
        for source in pending {
            domain.insert(source);
            reaches_root.insert(source);
        }
    }

    let mut children: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for node in &document.assets.source_skeleton.nodes {
        if let Some(parent) = node.parent_source_node_index {
            children
                .entry(parent)
                .or_default()
                .push(node.source_node_index);
        }
    }
    // Joint ownership spans every declared skin, not just the selected
    // instance: a descendant claimed as a joint by a different skin closes
    // the domain rather than being silently absorbed.
    let joint_owner: BTreeMap<usize, usize> = document
        .assets
        .source_skeleton
        .skins
        .iter()
        .flat_map(|skin| {
            skin.joint_source_node_indices
                .iter()
                .map(move |&joint| (joint, skin.source_skin_index))
        })
        .collect();

    let mut queue: Vec<usize> = domain.iter().copied().collect();
    while let Some(node) = queue.pop() {
        for &child in children.get(&node).into_iter().flatten() {
            if domain.contains(&child) {
                continue;
            }
            if let Some(&owner) = joint_owner.get(&child)
                && owner != skin.source_skin_index
            {
                return Err(ScaleError::IncompleteClosure {
                    reason: "descendant_joint_of_another_skin",
                });
            }
            reject_if_unskinned(child)?;
            domain.insert(child);
            queue.push(child);
        }
    }
    Ok(domain)
}

/// Compose `start`'s raw rest-world matrix from
/// `document.assets.source_skeleton.nodes`, walking the full
/// `parent_source_node_index` ancestor chain (not stopping at any affected
/// closure boundary) so classification sees the node's true rest-world
/// linear part.
///
/// Iterative and cache/visited-guarded rather than naive recursion: a
/// malformed cyclic or self-referencing parent chain errors instead of
/// looping or overflowing the stack.
#[derive(Clone, Copy)]
pub(in crate::scale) enum SourceWorldAccumulator {
    Narrow(Mat4),
    Widened(DMat4),
}

pub(in crate::scale) fn source_world_matrix(
    start: usize,
    by_source_index: &BTreeMap<usize, &SourceNodeAsset>,
    connector_sources: &BTreeSet<usize>,
    cache: &mut BTreeMap<usize, SourceWorldAccumulator>,
) -> Result<Mat4, ScaleError> {
    if let Some(&world) = cache.get(&start) {
        return narrow_source_world(start, world);
    }
    let mut chain = Vec::new();
    let mut cursor = start;
    let mut visited = BTreeSet::new();
    let base_world = loop {
        if let Some(&world) = cache.get(&cursor) {
            break world;
        }
        if !visited.insert(cursor) {
            return Err(ScaleError::IncompleteClosure {
                reason: "cyclic_source_parent_chain",
            });
        }
        let asset = *by_source_index
            .get(&cursor)
            .ok_or(ScaleError::IncompleteClosure {
                reason: "missing_source_node",
            })?;
        chain.push((cursor, asset));
        match asset.parent_source_node_index {
            Some(parent) => cursor = parent,
            None => break SourceWorldAccumulator::Narrow(Mat4::IDENTITY),
        }
    };
    let mut world = base_world;
    for (node, asset) in chain.into_iter().rev() {
        let local = local_rest_matrix(&asset.local_rest);
        if !mat4_is_finite(local) {
            return Err(ScaleError::NonFiniteSourceTransform {
                source_node_index: node,
            });
        }
        world = if connector_sources.contains(&node) {
            let widened = match world {
                SourceWorldAccumulator::Narrow(world) => world.as_dmat4(),
                SourceWorldAccumulator::Widened(world) => world,
            } * local.as_dmat4();
            if !widened.is_finite() {
                return Err(ScaleError::NonFiniteSourceTransform {
                    source_node_index: node,
                });
            }
            SourceWorldAccumulator::Widened(widened)
        } else {
            let narrowed = match world {
                SourceWorldAccumulator::Narrow(world) => world * local,
                SourceWorldAccumulator::Widened(world) => (world * local.as_dmat4()).as_mat4(),
            };
            if !mat4_is_finite(narrowed) {
                return Err(ScaleError::NonFiniteSourceTransform {
                    source_node_index: node,
                });
            }
            SourceWorldAccumulator::Narrow(narrowed)
        };
        cache.insert(node, world);
    }
    narrow_source_world(start, world)
}

fn narrow_source_world(
    source_node_index: usize,
    world: SourceWorldAccumulator,
) -> Result<Mat4, ScaleError> {
    let world = match world {
        SourceWorldAccumulator::Narrow(world) => world,
        SourceWorldAccumulator::Widened(world) => world.as_mat4(),
    };
    if !mat4_is_finite(world) {
        return Err(ScaleError::NonFiniteSourceTransform { source_node_index });
    }
    Ok(world)
}

/// Compose a raw authored local-rest matrix, preserving shear: the `Trs`
/// variant round-trips through `Mat4::from_scale_rotation_translation`
/// (necessarily orthogonal/uniform-representable), while `Matrix` is used
/// as-is — the only representation that can carry a literal shear term.
pub(in crate::scale) fn local_rest_matrix(rest: &SourceNodeLocalRest) -> Mat4 {
    match rest {
        SourceNodeLocalRest::Trs {
            translation,
            rotation,
            scale,
        } => Mat4::from_scale_rotation_translation(*scale, *rotation, *translation),
        SourceNodeLocalRest::Matrix(matrix) => *matrix,
    }
}

/// Compose every [`Bone::rest`](crate::model::Bone::rest) local transform in
/// `skeleton` into a parent-before-child world matrix, delegating to the
/// shared helper [`crate::model::world_rest_matrices`] and mapping its
/// structural error into this module's [`ScaleError`].
pub(in crate::scale) fn world_rests(skeleton: &Skeleton) -> Result<Vec<Mat4>, ScaleError> {
    world_rest_matrices(skeleton).map_err(|error| match error {
        crate::model::WorldMatrixError::NonFiniteTransform { node } => {
            ScaleError::NonFiniteTransform { node }
        }
        crate::model::WorldMatrixError::InvalidParent { node, parent } => {
            ScaleError::InvalidParent { node, parent }
        }
    })
}

/// One document side's composed world matrices, paired with the magnitude
/// each bone's world *translation column* was actually summed from.
///
/// The two travel together for [`super::proof::SkinSlot`]'s reason, one composition
/// earlier: a parent chain whose translations cancel leaves a world
/// translation orders of magnitude smaller than the terms it was accumulated
/// from, and a tolerance for anything derived from that world has to be
/// stated against the terms. See [`translation_composition_rounding_base`].
#[derive(Debug, Clone, Copy)]
pub(in crate::scale) struct WorldBonePose {
    pub(in crate::scale) matrix: Mat4,
    /// Sum of the translation-column rounding bases on the path from the root
    /// to this bone.
    ///
    /// `0.0` for a root: its world matrix *is* its local matrix, copied, so
    /// no arithmetic ran and there is no rounding base to carry.
    pub(in crate::scale) translation_rounding_magnitude: f64,
}

pub(in crate::scale) struct WorldPose {
    /// Matrix and provenance are one record so a consumer cannot pair one
    /// bone's world with another bone's rounding magnitude by indexing two
    /// parallel vectors independently.
    pub(in crate::scale) bones: Vec<WorldBonePose>,
}

impl WorldPose {
    /// One bone's world matrix and the magnitude its translation column was
    /// accumulated from.
    ///
    /// The two are always read together, and reading them through one
    /// bounds-checked accessor is what keeps the chain lookup from being a
    /// raw index that is safe only because a `get` on the parallel matrix
    /// vector ran above it.
    pub(in crate::scale) fn bone(&self, node: BoneId) -> Result<WorldBonePose, ScaleError> {
        self.bones
            .get(node)
            .copied()
            .ok_or(ScaleError::BoneIndexOutOfRange { index: node })
    }
}

/// Add one child composition's translation-column rounding base to the
/// provenance its parent already carries.
///
/// The policy models coherent translation-column rounding as additive across
/// links instead of depth-flat `max` or RSS/depth heuristics. This is the
/// empirically calibrated Appendix D v5 recurrence, retained unchanged by
/// v6, not a universal componentwise forward-error proof for the inherited
/// linear block.
pub(in crate::scale) fn child_translation_rounding_magnitude(
    parent: WorldBonePose,
    local: Mat4,
) -> f64 {
    parent.translation_rounding_magnitude
        + translation_composition_rounding_base(parent.matrix, local)
}

/// [`world_rests`] plus the translation chain magnitudes those worlds were
/// composed through.
pub(in crate::scale) fn rest_world_pose(skeleton: &Skeleton) -> Result<WorldPose, ScaleError> {
    let matrices = world_rests(skeleton)?;
    let mut bones: Vec<WorldBonePose> = Vec::with_capacity(matrices.len());
    for (node, bone) in skeleton.bones.iter().enumerate() {
        // `world_rests` has already refused every non-root whose parent is
        // not strictly below it, so the `None` arm is reached only by a
        // genuine root.
        let matrix = matrices[node];
        let translation_rounding_magnitude = match bone.parent {
            Some(parent) if parent < node => {
                child_translation_rounding_magnitude(bones[parent], bone.rest.to_mat4())
            }
            _ => 0.0,
        };
        bones.push(WorldBonePose {
            matrix,
            translation_rounding_magnitude,
        });
    }
    Ok(WorldPose { bones })
}

#[cfg(test)]
std::thread_local! {
    static AFFECTED_SKIN_CLASSIFICATION_STEPS: Cell<usize> = const { Cell::new(0) };
}

fn skin_palette_intersects_affected(instance: &MeshInstance, affected: &BTreeSet<BoneId>) -> bool {
    instance.skin_joints.iter().any(|joint| {
        #[cfg(test)]
        AFFECTED_SKIN_CLASSIFICATION_STEPS
            .set(AFFECTED_SKIN_CLASSIFICATION_STEPS.get().saturating_add(1));
        affected.contains(joint)
    })
}

/// Resolve the affected skin working set once for the rest and sampled proof
/// walks. Classification scans every source skin palette, so repeating it at
/// every sample time would perform file-controlled work that the sampling
/// budget does not charge.
pub(in crate::scale) fn affected_skin_instance_indices(
    document: &Document,
    affected: &BTreeSet<BoneId>,
) -> Vec<usize> {
    document
        .assets
        .instances
        .iter()
        .enumerate()
        .filter_map(|(instance_index, instance)| {
            skin_palette_intersects_affected(instance, affected).then_some(instance_index)
        })
        .collect()
}

#[cfg(test)]
pub(in crate::scale) fn reset_affected_skin_classification_steps() {
    AFFECTED_SKIN_CLASSIFICATION_STEPS.set(0);
}

#[cfg(test)]
pub(in crate::scale) fn affected_skin_classification_steps() -> usize {
    AFFECTED_SKIN_CLASSIFICATION_STEPS.get()
}

/// The inverse bind `document` *stores* for one skin slot, in this module's
/// precedence order — the instance's own per-slot array first, then the
/// bone convenience value — or `None` when it stores neither.
///
/// This is the stored-evidence prefix of [`instance_bind`]'s effective
/// fallback chain. Callers that need the bind a slot *has*, including proof,
/// must use [`instance_bind`] so a complete attached source skin with an
/// [`SourceInverseBindAccessorStatus::Absent`] accessor can license the
/// format-defined identity default.
///
/// The out-of-range branch is unreachable for a document that passed
/// [`validate_scale_input`], which requires a non-empty `skin_ibms` to be
/// exactly as long as `skin_joints`; it is kept because this function is
/// total over its arguments rather than over its current call sites.
pub(in crate::scale) fn stored_instance_bind(
    document: &Document,
    instance: &MeshInstance,
    slot: usize,
    joint: BoneId,
) -> Result<Option<Mat4>, ScaleError> {
    if !instance.skin_ibms.is_empty() {
        return instance
            .skin_ibms
            .get(slot)
            .copied()
            .map(Some)
            .ok_or(ScaleError::BoneIndexOutOfRange { index: joint });
    }
    let bone = document
        .skeleton
        .bones
        .get(joint)
        .ok_or(ScaleError::BoneIndexOutOfRange { index: joint })?;
    Ok(bone.inverse_bind)
}

/// Resolve one skin joint's inverse-bind matrix per the documented
/// [`MeshInstance::skin_ibms`] contract: use the instance's own matrix when
/// present, else fall back to the bone's [`crate::model::Bone::inverse_bind`]
/// — the only fallback this model contract genuinely represents
/// ([`stored_instance_bind`]).
///
/// A bone with neither is missing evidence in general, and rejects with
/// [`ScaleError::MissingInverseBind`] rather than substituting
/// [`Mat4::IDENTITY`] to mask a partial or malformed bind array — *unless*
/// the source's own complete-coverage evidence proves this is the
/// format-defined identity default rather than an unavailable or malformed
/// accessor (for example, glTF permits a skin to omit
/// `inverseBindMatrices` entirely, in which case every joint's inverse-bind
/// matrix is defined to be identity). [`instance_source_skin`] only returns
/// that skin evidence when `document.assets.source_skeleton.coverage` is
/// complete, so an incomplete or absent source-skeleton projection still
/// rejects here rather than silently defaulting to identity.
pub(in crate::scale) fn instance_bind(
    document: &Document,
    instance: &MeshInstance,
    slot: usize,
    joint: BoneId,
) -> Result<Mat4, ScaleError> {
    if let Some(stored) = stored_instance_bind(document, instance, slot, joint)? {
        return Ok(stored);
    }
    if instance_source_skin(document, instance).is_some_and(|skin| {
        skin.inverse_bind_accessor.status == SourceInverseBindAccessorStatus::Absent
    }) {
        return Ok(Mat4::IDENTITY);
    }
    Err(ScaleError::MissingInverseBind { node: joint })
}

/// Resolve the source skin that attaches at `instance`'s source node,
/// per [`crate::model::SourceSkinAttachment::source_node_index`].
///
/// Returns `None` when `document.assets.source_skeleton.coverage` is not
/// [`SourceSkeletonCoverage::Complete`]: an incomplete or unprojected source
/// table cannot vouch for the accessor evidence it does not carry, so an
/// absent flag there must not be read as proof of a format-defined default.
fn instance_source_skin<'a>(
    document: &'a Document,
    instance: &MeshInstance,
) -> Option<&'a SourceSkinAsset> {
    if document.assets.source_skeleton.coverage != SourceSkeletonCoverage::Complete {
        return None;
    }
    document.assets.source_skeleton.skins.iter().find(|skin| {
        skin.attachments
            .iter()
            .any(|attachment| attachment.source_node_index == instance.source_node_index)
    })
}
