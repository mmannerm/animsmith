//! Private analytic scale-candidate construction.
//!
//! Production format frontends rewrite exact source representations, reload
//! their emitted artifacts, and wrap those documents with
//! [`ScaleCandidate::from_document`]. This module owns only the independent
//! analytic reference writer used by fixtures and calibration. Its numeric
//! derivations deliberately remain separate from proof-owned expectations.

use super::numeric::{scale_rows, scale_translation_only};
use super::planning::{check_factor_narrows, validate_plan_document_inventory};
use super::validation::{
    instance_bind, local_rest_matrix, source_node_index_map, validate_scale_input,
};
use super::{
    ScaleBoneRestField, ScaleError, ScaleFieldDisposition, ScaleFieldTarget, ScaleOperation,
    ScalePlan, ScaleRewriteRule, ScaleSourceRestField,
};
use crate::model::{
    BoneId, Document, Property, SourceNodeAsset, SourceNodeLocalRest, SourceSkeletonCoverage,
    TrackValues, mat4_is_finite,
};
use glam::{DMat3, DMat4, DVec3, Mat4, Vec4};
use std::collections::{BTreeMap, BTreeSet};

/// A candidate document supplied to [`super::prove_scale`].
///
/// This type deliberately has no mutation method. Its only public constructor,
/// [`ScaleCandidate::from_document`], wraps the document a format frontend
/// reloaded from its exact emitted artifact bytes. Reference and calibration
/// tests use the non-default `fixtures` feature instead of a production
/// candidate-building API.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ScaleCandidate {
    pub(in crate::scale) document: Document,
}

impl ScaleCandidate {
    /// Wrap a candidate `document` that a format frontend reloaded from the
    /// exact artifact bytes it emitted, so it can be handed to
    /// [`super::prove_scale`].
    ///
    /// DESIGN.md Appendix D §D.8 assigns "exact source rewriting" to the
    /// format frontend, which necessarily produces candidates this module did
    /// not build: `animsmith_gltf`'s whole-document linear-unit rewrite
    /// operates on raw glTF JSON and buffer bytes and then reloads the
    /// artifact. Without this constructor that reloaded [`Document`] could
    /// never reach [`super::prove_scale`], and the artifact-level proof D.6
    /// requires would have no in-memory layer to sit on top of.
    ///
    /// This constructor asserts nothing about `document`. It does not need
    /// to: [`super::prove_scale`] already re-validates both documents it is
    /// given and re-derives every claim from them, so this type carries no
    /// safety obligation that [`super::prove_scale`] does not independently
    /// redo.
    pub fn from_document(document: Document) -> Self {
        Self { document }
    }

    /// The candidate document.
    pub fn document(&self) -> &Document {
        &self.document
    }

    /// Consume this candidate, taking ownership of the document.
    pub fn into_document(self) -> Document {
        self.document
    }
}

#[cfg(doctest)]
mod candidate_api_contract {
    /// Compile-fail coverage for the removed production-looking reference
    /// builder. Each former path stays in its own compilation unit so restoring
    /// one cannot be masked by the other remaining unavailable.
    ///
    /// ```compile_fail
    /// use animsmith_core::build_scale_candidate;
    /// ```
    ///
    /// ```compile_fail
    /// use animsmith_core::scale::build_scale_candidate;
    /// ```
    ///
    /// The reloaded-artifact wrapper remains opaque; external callers use its
    /// explicit constructor rather than a struct literal.
    ///
    /// ```compile_fail
    /// use animsmith_core::{Document, ScaleCandidate};
    ///
    /// let _ = ScaleCandidate {
    ///     document: Document::default(),
    /// };
    /// ```
    ///
    /// Field privacy is pinned independently from the non-exhaustive struct
    /// construction boundary above.
    ///
    /// ```compile_fail
    /// use animsmith_core::ScaleCandidate;
    ///
    /// fn read_private_document(candidate: ScaleCandidate) {
    ///     let _ = candidate.document;
    /// }
    /// ```
    struct RemovedPublicBuilder;
}

/// Build the analytic reference candidate from an accepted [`ScalePlan`],
/// without mutating `document`.
///
/// This is private implementation support. Cross-crate analytic tests opt in
/// to `animsmith_core::fixtures::build_scale_reference_candidate`; production
/// format frontends rewrite exact source bytes and use
/// [`ScaleCandidate::from_document`] on the emitted reload.
///
/// `document` need not be numerically identical to the document `plan` was
/// computed against. Re-deriving its structural planning inventory must,
/// however, produce the same affected domain and proof inventory. This permits
/// intentional numerical replay while rejecting a structurally stale plan
/// before one of its proof walks can omit newly introduced payload.
///
/// # Errors
///
/// Returns [`ScaleError::PlanDocumentMismatch`] if `document` derives a
/// different plan inventory, [`ScaleError::BoneIndexOutOfRange`] if an
/// affected node in `plan` is out of range for `document`,
/// [`ScaleError::MissingInverseBind`] if an affected skin slot has no
/// inverse-bind evidence to conjugate, or any document-shape error — checked
/// on the *candidate* as well as the input, so a build can never hand back a
/// structurally invalid or non-finite document as `Ok`.
#[cfg_attr(not(any(test, feature = "fixtures")), allow(dead_code))]
pub(crate) fn build_scale_candidate(
    document: &Document,
    plan: &ScalePlan,
) -> Result<ScaleCandidate, ScaleError> {
    validate_plan_document_inventory(document, plan)?;
    let candidate = match plan.operation() {
        ScaleOperation::WholeDocumentLinearUnits { .. } => build_whole_document(document, plan)?,
        ScaleOperation::RestBindUniformScale { .. } => build_rest_bind(document, plan)?,
    };
    // The same fail-closed shape check the input had to pass, re-run on the
    // output: a builder is the one place in this module that writes numbers,
    // so it must not be the one place that returns unvalidated ones. Without
    // this, an overflowing or annihilating factor produces a candidate whose
    // only remaining defence is `prove_scale`, which a caller is free not to
    // run.
    validate_scale_input(&candidate)?;
    Ok(ScaleCandidate {
        document: candidate,
    })
}

pub(in crate::scale) fn build_whole_document(
    document: &Document,
    plan: &ScalePlan,
) -> Result<Document, ScaleError> {
    let q = check_factor_narrows(plan.common_factor(), plan.common_factor())?;
    let mut candidate = document.clone();
    let source_positions: BTreeMap<_, _> = candidate
        .assets
        .source_skeleton
        .nodes
        .iter()
        .enumerate()
        .map(|(position, node)| (node.source_node_index, position))
        .collect();
    for row in plan.field_rows() {
        let ScaleFieldDisposition::Rewrite(rule) = row.disposition else {
            continue;
        };
        if rule != ScaleRewriteRule::WholeDocumentLength {
            return Err(ScaleError::PlanDocumentMismatch {
                reason: "invalid_whole_document_rewrite_rule",
            });
        }
        match row.target {
            ScaleFieldTarget::BoneRest {
                bone,
                field: ScaleBoneRestField::Translation,
            } => candidate.skeleton.bones[bone].rest.translation *= q,
            ScaleFieldTarget::BoneInverseBind { bone } => {
                let inverse_bind = candidate.skeleton.bones[bone].inverse_bind.as_mut().ok_or(
                    ScaleError::PlanDocumentMismatch {
                        reason: "compiled_bone_inverse_bind_missing",
                    },
                )?;
                *inverse_bind = scale_translation_only(*inverse_bind, q);
            }
            ScaleFieldTarget::SourceNodeRest {
                source_node_index,
                field: ScaleSourceRestField::Translation | ScaleSourceRestField::MatrixTranslation,
            } => {
                let position = *source_positions.get(&source_node_index).ok_or(
                    ScaleError::PlanDocumentMismatch {
                        reason: "compiled_source_node_missing",
                    },
                )?;
                let node = &mut candidate.assets.source_skeleton.nodes[position];
                node.local_rest = match &node.local_rest {
                    SourceNodeLocalRest::Trs {
                        translation,
                        rotation,
                        scale,
                    } => SourceNodeLocalRest::Trs {
                        translation: *translation * q,
                        rotation: *rotation,
                        scale: *scale,
                    },
                    SourceNodeLocalRest::Matrix(matrix) => {
                        SourceNodeLocalRest::Matrix(scale_translation_only(*matrix, q))
                    }
                };
            }
            ScaleFieldTarget::AnimationValues {
                clip_index,
                track_index,
                property: Property::Translation,
                ..
            } => {
                if let TrackValues::Vec3s(values) =
                    &mut candidate.clips[clip_index].tracks[track_index].values
                {
                    for value in values {
                        *value *= q;
                    }
                }
            }
            ScaleFieldTarget::MeshPositions {
                mesh_index,
                primitive_index,
            } => {
                for position in
                    &mut candidate.assets.meshes[mesh_index].primitives[primitive_index].positions
                {
                    *position *= q;
                }
            }
            ScaleFieldTarget::InstanceInverseBind {
                instance_index,
                slot,
                ..
            } => {
                let inverse_bind = &mut candidate.assets.instances[instance_index].skin_ibms[slot];
                *inverse_bind = scale_translation_only(*inverse_bind, q);
            }
            _ => {
                return Err(ScaleError::PlanDocumentMismatch {
                    reason: "invalid_whole_document_write_target",
                });
            }
        }
    }
    // Unavailable source coverage is not identity evidence and therefore has
    // no public/replay ledger rows. Preserve the released whole-document
    // behavior for any best-effort raw locals a frontend nevertheless kept:
    // they still receive the unit conversion, but their identities cannot
    // make an otherwise compatible plan stale or become proof authority.
    if candidate.assets.source_skeleton.coverage != SourceSkeletonCoverage::Complete {
        for node in &mut candidate.assets.source_skeleton.nodes {
            node.local_rest = match &node.local_rest {
                SourceNodeLocalRest::Trs {
                    translation,
                    rotation,
                    scale,
                } => SourceNodeLocalRest::Trs {
                    translation: *translation * q,
                    rotation: *rotation,
                    scale: *scale,
                },
                SourceNodeLocalRest::Matrix(matrix) => {
                    SourceNodeLocalRest::Matrix(scale_translation_only(*matrix, q))
                }
            };
        }
    }
    Ok(candidate)
}

pub(in crate::scale) fn build_rest_bind(
    document: &Document,
    plan: &ScalePlan,
) -> Result<Document, ScaleError> {
    let affected = plan.affected_set();
    let s = check_factor_narrows(plan.common_factor(), plan.common_factor())?;
    let by_source_index = source_node_index_map(document);
    let mut connector_product_by_tail = BTreeMap::new();
    let parent_factor = |node: BoneId| -> Result<f32, ScaleError> {
        let bone = document
            .skeleton
            .bones
            .get(node)
            .ok_or(ScaleError::BoneIndexOutOfRange { index: node })?;
        Ok(match bone.parent {
            Some(parent) if affected.contains(&parent) => s,
            _ => 1.0,
        })
    };
    let node_factor = |node: BoneId| -> f32 { if affected.contains(&node) { s } else { 1.0 } };

    let mut candidate = document.clone();
    let source_positions: BTreeMap<_, _> = candidate
        .assets
        .source_skeleton
        .nodes
        .iter()
        .enumerate()
        .map(|(position, node)| (node.source_node_index, position))
        .collect();
    let mut materialized_binds: BTreeMap<usize, Vec<Mat4>> = BTreeMap::new();
    for row in plan.field_rows() {
        let ScaleFieldDisposition::Rewrite(rule) = row.disposition else {
            continue;
        };
        match (row.target, rule) {
            (
                ScaleFieldTarget::BoneRest {
                    bone,
                    field: ScaleBoneRestField::Translation,
                },
                ScaleRewriteRule::RestBindParentBasis,
            ) => candidate.skeleton.bones[bone].rest.translation *= parent_factor(bone)?,
            (
                ScaleFieldTarget::BoneRest {
                    bone,
                    field: ScaleBoneRestField::Scale,
                },
                ScaleRewriteRule::RestBindLocalScale,
            ) => {
                candidate.skeleton.bones[bone].rest.scale *=
                    parent_factor(bone)? / node_factor(bone);
            }
            (ScaleFieldTarget::BoneInverseBind { bone }, ScaleRewriteRule::RestBindNodeBasis) => {
                let inverse_bind = candidate.skeleton.bones[bone].inverse_bind.as_mut().ok_or(
                    ScaleError::PlanDocumentMismatch {
                        reason: "compiled_bone_inverse_bind_missing",
                    },
                )?;
                *inverse_bind = scale_rows(*inverse_bind, node_factor(bone));
            }
            (
                ScaleFieldTarget::SourceNodeRest {
                    source_node_index,
                    field,
                },
                ScaleRewriteRule::RestBindSourceLocal { connector_tail },
            ) => {
                let position = *source_positions.get(&source_node_index).ok_or(
                    ScaleError::PlanDocumentMismatch {
                        reason: "compiled_source_node_missing",
                    },
                )?;
                let bone = candidate.assets.source_skeleton.nodes[position]
                    .bone
                    .ok_or(ScaleError::SourceNodeNotNormalized { source_node_index })?;
                let local_rest = &candidate.assets.source_skeleton.nodes[position].local_rest;
                let s_parent = parent_factor(bone)?;
                let s_node = node_factor(bone);
                let rebased = if let Some(connector_tail) = connector_tail {
                    rebase_source_local_through_connector_bridge(
                        local_rest,
                        connector_tail,
                        &by_source_index,
                        &mut connector_product_by_tail,
                        s_parent,
                        s_node,
                        bone,
                    )?
                } else {
                    rebase_source_local_rest(local_rest, s_parent, s_node, None)
                };
                candidate.assets.source_skeleton.nodes[position].local_rest =
                    source_rest_with_rewritten_field(local_rest, &rebased, field)?;
            }
            (
                ScaleFieldTarget::AnimationValues {
                    clip_index,
                    track_index,
                    ..
                },
                rule @ (ScaleRewriteRule::RestBindParentBasis
                | ScaleRewriteRule::RestBindLocalScale),
            ) => match rule {
                ScaleRewriteRule::RestBindParentBasis => {
                    let property = document.clips[clip_index].tracks[track_index].property;
                    let bone = document.clips[clip_index].tracks[track_index].bone;
                    let s_parent =
                        plan.animation_target_factor_unchecked(document, bone, property)? as f32;
                    if let TrackValues::Vec3s(values) =
                        &mut candidate.clips[clip_index].tracks[track_index].values
                    {
                        for value in values.iter_mut() {
                            *value *= s_parent;
                        }
                    }
                }
                ScaleRewriteRule::RestBindLocalScale => {
                    let property = document.clips[clip_index].tracks[track_index].property;
                    let bone = document.clips[clip_index].tracks[track_index].bone;
                    let multiplier =
                        plan.animation_target_factor_unchecked(document, bone, property)?;
                    if let TrackValues::Vec3s(values) =
                        &mut candidate.clips[clip_index].tracks[track_index].values
                    {
                        for value in values {
                            // `Vec3` is the storage boundary. Form the
                            // product in f64 and narrow each component once.
                            *value = (value.as_dvec3() * multiplier).as_vec3();
                        }
                    }
                }
                ScaleRewriteRule::WholeDocumentLength
                | ScaleRewriteRule::RestBindNodeBasis
                | ScaleRewriteRule::RestBindSourceLocal { .. } => {
                    unreachable!("outer pattern limits animation rules")
                }
            },
            (
                ScaleFieldTarget::InstanceInverseBind {
                    instance_index,
                    slot,
                    joint,
                },
                ScaleRewriteRule::RestBindNodeBasis,
            ) => {
                let source_instance = &document.assets.instances[instance_index];
                let binds = materialized_binds
                    .entry(instance_index)
                    .or_insert_with(|| Vec::with_capacity(source_instance.skin_joints.len()));
                if slot != binds.len() {
                    return Err(ScaleError::PlanDocumentMismatch {
                        reason: "compiled_inverse_bind_slot_order_mismatch",
                    });
                }
                let before = instance_bind(document, source_instance, slot, joint)?;
                binds.push(scale_rows(before, node_factor(joint)));
            }
            _ => {
                return Err(ScaleError::PlanDocumentMismatch {
                    reason: "invalid_rest_bind_write_target",
                });
            }
        }
    }
    for (instance_index, binds) in materialized_binds {
        candidate.assets.instances[instance_index].skin_ibms = binds;
    }
    Ok(candidate)
}

/// Apply the established projected-local rebase, optionally combining its
/// translation with a widened connector-conjugation offset.
///
/// `None` deliberately avoids adding a zero: direct-edge f32 association and
/// signed-zero behavior are part of the calibrated Appendix D contract. A
/// bridged translation stays widened until the complete sum is narrowed, so
/// compensating terms above the f32 range can still produce a finite local.
fn rebase_source_local_rest(
    local_rest: &SourceNodeLocalRest,
    s_parent: f32,
    s_node: f32,
    bridge_offset: Option<DVec3>,
) -> SourceNodeLocalRest {
    match local_rest {
        SourceNodeLocalRest::Trs {
            translation,
            rotation,
            scale,
        } => {
            let translation = match bridge_offset {
                Some(offset) => (translation.as_dvec3() * f64::from(s_parent) + offset).as_vec3(),
                None => *translation * s_parent,
            };
            let scale = match bridge_offset {
                Some(_) => {
                    let ratio = f64::from(s_parent) / f64::from(s_node);
                    (scale.as_dvec3() * ratio).as_vec3()
                }
                None => *scale * (s_parent / s_node),
            };
            SourceNodeLocalRest::Trs {
                translation,
                rotation: *rotation,
                scale,
            }
        }
        SourceNodeLocalRest::Matrix(matrix) => {
            let rebased = if let Some(offset) = bridge_offset {
                let ratio = f64::from(s_parent) / f64::from(s_node);
                let rebase_linear_column = |column: Vec4| {
                    (column.truncate().as_dvec3() * ratio)
                        .as_vec3()
                        .extend(column.w)
                };
                let translation =
                    (matrix.w_axis.truncate().as_dvec3() * f64::from(s_parent) + offset).as_vec3();
                Mat4::from_cols(
                    rebase_linear_column(matrix.x_axis),
                    rebase_linear_column(matrix.y_axis),
                    rebase_linear_column(matrix.z_axis),
                    translation.extend(matrix.w_axis.w),
                )
            } else {
                rebase_matrix(*matrix, s_parent, s_node)
            };
            SourceNodeLocalRest::Matrix(rebased)
        }
    }
}

/// Merge one compiled raw-source write into its local container.
///
/// A [`super::ScaleFieldPlan`] is the builder's write authority. Even when
/// deriving one field naturally produces a complete local transform, sibling
/// fields must retain their original bits unless their own row also says
/// Rewrite.
fn source_rest_with_rewritten_field(
    original: &SourceNodeLocalRest,
    rewritten: &SourceNodeLocalRest,
    field: ScaleSourceRestField,
) -> Result<SourceNodeLocalRest, ScaleError> {
    match (original, rewritten, field) {
        (
            SourceNodeLocalRest::Trs {
                translation: _,
                rotation,
                scale,
            },
            SourceNodeLocalRest::Trs {
                translation: rewritten,
                ..
            },
            ScaleSourceRestField::Translation,
        ) => Ok(SourceNodeLocalRest::Trs {
            translation: *rewritten,
            rotation: *rotation,
            scale: *scale,
        }),
        (
            SourceNodeLocalRest::Trs {
                translation,
                rotation,
                scale: _,
            },
            SourceNodeLocalRest::Trs {
                scale: rewritten, ..
            },
            ScaleSourceRestField::Scale,
        ) => Ok(SourceNodeLocalRest::Trs {
            translation: *translation,
            rotation: *rotation,
            scale: *rewritten,
        }),
        (
            SourceNodeLocalRest::Matrix(original),
            SourceNodeLocalRest::Matrix(rewritten),
            ScaleSourceRestField::MatrixLinear,
        ) => Ok(SourceNodeLocalRest::Matrix(Mat4::from_cols(
            rewritten.x_axis.truncate().extend(original.x_axis.w),
            rewritten.y_axis.truncate().extend(original.y_axis.w),
            rewritten.z_axis.truncate().extend(original.z_axis.w),
            original.w_axis,
        ))),
        (
            SourceNodeLocalRest::Matrix(original),
            SourceNodeLocalRest::Matrix(rewritten),
            ScaleSourceRestField::MatrixTranslation,
        ) => Ok(SourceNodeLocalRest::Matrix(Mat4::from_cols(
            original.x_axis,
            original.y_axis,
            original.z_axis,
            rewritten.w_axis.truncate().extend(original.w_axis.w),
        ))),
        _ => Err(ScaleError::PlanDocumentMismatch {
            reason: "source_local_field_variant_mismatch",
        }),
    }
}

/// Move a projected successor's rest/bind correction through an ordered
/// chain of unchanged, unprojected source transforms.
///
/// If `H` is the parent-to-child product of the connector locals and `L` is
/// the projected successor's authored local, preserving every connector
/// exactly requires `L' = H^-1 S_parent H L S_node^-1`. A multiplier-only
/// rewrite of `L` is wrong whenever `H` has a nonzero translation.
fn rebase_source_local_through_connector_bridge(
    local_rest: &SourceNodeLocalRest,
    connector_tail: usize,
    by_source_index: &BTreeMap<usize, &SourceNodeAsset>,
    connector_product_by_tail: &mut BTreeMap<usize, DMat4>,
    s_parent: f32,
    s_node: f32,
    bone: BoneId,
) -> Result<SourceNodeLocalRest, ScaleError> {
    if s_parent == 1.0 && s_node == 1.0 {
        return Ok(local_rest.clone());
    }
    let connector =
        memoized_connector_product(connector_tail, by_source_index, connector_product_by_tail)?;
    let connector_linear_inverse = DMat3::from_cols(
        connector.x_axis.truncate(),
        connector.y_axis.truncate(),
        connector.z_axis.truncate(),
    )
    .inverse();
    let bridge_offset =
        connector_linear_inverse * (connector.w_axis.truncate() * (f64::from(s_parent) - 1.0));
    if !connector_linear_inverse.x_axis.is_finite()
        || !connector_linear_inverse.y_axis.is_finite()
        || !connector_linear_inverse.z_axis.is_finite()
        || !bridge_offset.is_finite()
    {
        return Err(ScaleError::NonFiniteTransform { node: bone });
    }
    // For affine H=[A,t], H^-1*S_parent*H contributes only the translation
    // A^-1*((s_parent-1)*t) beyond the established direct-edge rewrite. Keep
    // every complete bridged expression widened through its single model
    // boundary, while direct projected edges retain their established f32
    // association and signed-zero behavior.
    let rebased = rebase_source_local_rest(local_rest, s_parent, s_node, Some(bridge_offset));
    if !mat4_is_finite(local_rest_matrix(&rebased)) {
        return Err(ScaleError::NonFiniteTransform { node: bone });
    }
    Ok(rebased)
}

/// Return the ordered connector product from its nearest projected ancestor
/// through `connector_tail`, caching every traversed prefix once.
fn memoized_connector_product(
    connector_tail: usize,
    by_source_index: &BTreeMap<usize, &SourceNodeAsset>,
    connector_product_by_tail: &mut BTreeMap<usize, DMat4>,
) -> Result<DMat4, ScaleError> {
    let mut pending = Vec::new();
    let mut visited = BTreeSet::new();
    let mut cursor = connector_tail;
    let mut product = loop {
        if let Some(&cached) = connector_product_by_tail.get(&cursor) {
            break cached;
        }
        if !visited.insert(cursor) {
            return Err(ScaleError::IncompleteClosure {
                reason: "cyclic_connector_source_parent_chain",
            });
        }
        let asset = by_source_index
            .get(&cursor)
            .ok_or(ScaleError::IncompleteClosure {
                reason: "dangling_connector_source_node_index",
            })?;
        if asset.bone.is_some() {
            break DMat4::IDENTITY;
        }
        pending.push(cursor);
        cursor = asset
            .parent_source_node_index
            .ok_or(ScaleError::IncompleteClosure {
                reason: "connector_without_projected_ancestor",
            })?;
    };
    while let Some(source) = pending.pop() {
        let asset = by_source_index
            .get(&source)
            .ok_or(ScaleError::IncompleteClosure {
                reason: "dangling_connector_source_node_index",
            })?;
        product *= local_rest_matrix(&asset.local_rest).as_dmat4();
        connector_product_by_tail.insert(source, product);
    }
    connector_product_by_tail
        .get(&connector_tail)
        .copied()
        .ok_or(ScaleError::IncompleteClosure {
            reason: "empty_connector_bridge",
        })
}

/// `L' = scale(s_parent) * L * scale(1 / s_node)`: the rest/bind local
/// rebase of DESIGN.md Appendix D §D.2, applied to a raw authored matrix
/// that may carry terms a TRS decomposition cannot represent.
///
/// Left-multiplying by a uniform scale scales the output rows (that is
/// [`scale_rows`]); right-multiplying by `scale(1 / s_node)` scales the three
/// linear columns in full, translation column untouched.
fn rebase_matrix(matrix: Mat4, s_parent: f32, s_node: f32) -> Mat4 {
    let scaled = scale_rows(matrix, s_parent);
    let inverse_node = 1.0 / s_node;
    Mat4::from_cols(
        scaled.x_axis * inverse_node,
        scaled.y_axis * inverse_node,
        scaled.z_axis * inverse_node,
        scaled.w_axis,
    )
}
